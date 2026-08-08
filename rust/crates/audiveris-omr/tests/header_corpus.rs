// SPDX-License-Identifier: AGPL-3.0-or-later

//! Grades the native key stage against Java on the whole corpus.
//!
//! **This one chains rather than isolates.** The clef test supplies Java's header start and grades
//! only what the clef stage does with it. Here the clef stage runs first and its `clefStop` feeds
//! the key stage's `browseStart`, exactly as `KeyColumn` does — so a key disagreement can be
//! caused by a clef disagreement, and that is deliberate. The clef stage is already graded 65/65
//! on its own, so chaining adds the *join* to what is under test rather than muddying it.
//!
//! Only the header start itself is still supplied from the oracle; computing it is
//! `HeaderBuilder`'s job and has its own unported stage.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use audiveris_omr::clef_classifier::BundledClefClassifier;
use audiveris_omr::clef_column::{
    ClefLifecycleRecognizer, ClefLookupParameters, ClefLookupStaffGeometry, HeadlessClefColumn,
    NativeClefParameters, NativeClefProposalRecognizer, StaffLineOrdinates,
    build_clef_lookup_contexts_at,
};
use audiveris_omr::clef_parameters::{SheetClefParameters, StaffClefParameters};
use audiveris_omr::header_time_builder::{
    HeaderTimeRasterContext, NativeHeaderTimeParameters, NativeHeaderTimeRecognizer,
};
use audiveris_omr::header_time_column::{
    HeaderTimeLifecycleContext, HeaderTimeLifecycleRecognizer, HeadlessHeaderTimeColumn,
    NeutralSpecificTimeShape,
};
use audiveris_omr::headers_step::{HeadlessHeaderStaff, HeadlessHeaderSystem};
use audiveris_omr::key_classifier::BundledKeyClassifier;
use audiveris_omr::key_column::{
    HeadlessKeyColumn, KeyLifecycleContext, KeyLifecycleRecognizer, NativeKeyParameters,
    NativeKeyProposalRecognizer, NativeKeyStaffContext, StaffPitchGeometry,
};
use audiveris_omr::key_parameters::{
    KeyExtractorParameters, KeyPipelineParameters, browse_envelope, max_eval_rank,
    max_header_width, max_part_count, max_slice_distance,
};
use audiveris_omr::recognize::{StaffLineGeometry, recognize_grid_lines};
use audiveris_omr::staff_header::StaffHeader;
use audiveris_omr::time_classifier::BundledTimeClassifier;

/// Java `Grades.intrinsicRatio`.
const INTRINSIC_RATIO: f64 = 0.8;

/// Java `Grades.clefMinGrade / Grades.intrinsicRatio`.
const MINIMUM_CLEF_GRADE: f64 = 0.03 / 0.8;

/// Java `Grades.keySigMinGrade / Grades.intrinsicRatio`.
const MINIMUM_KEY_GRADE: f64 = 0.01 / 0.8;

/// Java `KeyBuilder.constants.maxDeltaPitch_1` and `_4`.
const MAX_DELTA_PITCH_ONE: f64 = 0.5;
const MAX_DELTA_PITCH_FOUR: f64 = 2.0;

/// Java `ClefKeyRelation.clefSupportCoeff` and `KeyAltersRelation.sourceSupportCoeff`, both 5.
const CLEF_KEY_SOURCE_RATIO: f64 = 5.0;
const KEY_ALTERS_SOURCE_RATIO: f64 = 5.0;

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}

#[derive(Debug, Clone)]
struct OracleStaff {
    id: usize,
    specific_interline: i32,
    header_start: i32,
    /// The final `header.stop` after all three columns -- what `getHeaderStop()` returns and
    /// what the BEAMS/STEM_SEEDS header erase reads.
    header_stop: i32,
    /// `None` where Java found no key on this staff, which the port must also produce.
    key: Option<OracleKey>,
    /// `None` where Java found no time signature, which the port must also produce.
    time: Option<OracleTime>,
    /// Abscissae of good, connected barlines: what `Staff.getBrowseStop` cuts at.
    bars: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq)]
struct OracleTime {
    specific: Option<NeutralSpecificTimeShape>,
    numerator: i32,
    denominator: i32,
    bounds: (i32, i32, i32, i32),
    stop: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
struct OracleKey {
    fifths: i32,
    bounds: (i32, i32, i32, i32),
    stop: Option<i32>,
}

fn parse_oracle() -> Vec<(String, i32, Vec<OracleStaff>)> {
    let text = std::fs::read_to_string(repo_path("rust/oracle/clef-headers.txt"))
        .expect("the clef/key oracle is checked in");
    let mut pages: Vec<(String, i32, Vec<OracleStaff>)> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f.first().copied() {
            Some("page") => pages.push((f[1].to_owned(), f[2].parse().unwrap(), Vec::new())),
            Some("staff") => {
                let page = pages.last_mut().expect("a staff row follows a page row");
                page.2.push(OracleStaff {
                    id: f[1].parse().unwrap(),
                    specific_interline: f[2].parse().unwrap(),
                    header_start: f[3].parse().unwrap(),
                    header_stop: f[4].parse().unwrap(),
                    key: None,
                    time: None,
                    bars: Vec::new(),
                });
            }
            Some("key") => {
                let page = pages.last_mut().expect("a key row follows a page row");
                let id: usize = f[1].parse().unwrap();
                let staff = page
                    .2
                    .iter_mut()
                    .find(|staff| staff.id == id)
                    .expect("a key row names a staff already seen");
                if f[2] != "NONE" {
                    staff.key = Some(OracleKey {
                        fifths: f[2].parse().unwrap(),
                        bounds: (
                            f[4].parse().unwrap(),
                            f[5].parse().unwrap(),
                            f[6].parse().unwrap(),
                            f[7].parse().unwrap(),
                        ),
                        stop: f[8].parse().ok(),
                    });
                }
            }
            Some("bars") => {
                let page = pages.last_mut().expect("a bars row follows a page row");
                let id: usize = f[1].parse().unwrap();
                let staff = page
                    .2
                    .iter_mut()
                    .find(|staff| staff.id == id)
                    .expect("a bars row names a staff already seen");
                staff.bars = f[2..].iter().filter_map(|x| x.parse().ok()).collect();
            }
            Some("time") => {
                let page = pages.last_mut().expect("a time row follows a page row");
                let id: usize = f[1].parse().unwrap();
                let staff = page
                    .2
                    .iter_mut()
                    .find(|staff| staff.id == id)
                    .expect("a time row names a staff already seen");
                if f[2] != "NONE" {
                    let specific = match f[2] {
                        "COMMON_TIME" => Some(NeutralSpecificTimeShape::Common),
                        "CUT_TIME" => Some(NeutralSpecificTimeShape::Cut),
                        _ => None,
                    };
                    let (numerator, denominator) = f[3]
                        .split_once('/')
                        .map(|(n, d)| (n.parse().unwrap(), d.parse().unwrap()))
                        .expect("a rational like 2/4");
                    staff.time = Some(OracleTime {
                        specific,
                        numerator,
                        denominator,
                        bounds: (
                            f[5].parse().unwrap(),
                            f[6].parse().unwrap(),
                            f[7].parse().unwrap(),
                            f[8].parse().unwrap(),
                        ),
                        stop: f[9].parse().ok(),
                    });
                }
            }
            _ => {}
        }
    }
    pages
}

/// Staff-line ordinates for the clef stage: Java `yAt(int)`, so rounded.
struct GridOrdinates<'a>(&'a [StaffLineGeometry]);

impl StaffLineOrdinates for GridOrdinates<'_> {
    fn ordinates_at(&self, staff_id: usize, x: i32) -> Option<(i32, i32)> {
        let staff = self.0.iter().find(|staff| staff.staff_id == staff_id)?;
        Some((staff.first_line_y_at(x)?, staff.last_line_y_at(x)?))
    }
}

/// Staff-line ordinates for the key pitch: Java `yAt(double)`, so *unrounded*.
struct GridPitch(Vec<StaffLineGeometry>);

impl StaffPitchGeometry for GridPitch {
    fn line_span_at(&self, staff_id: usize, x: f64) -> Option<(f64, f64)> {
        let staff = self.0.iter().find(|staff| staff.staff_id == staff_id)?;
        Some((staff.first_line.y_at_x(x)?, staff.last_line.y_at_x(x)?))
    }
}

/// Java `Scale.Fraction` (`rint`) and `Scale.AreaFraction`, local to the time wiring.
fn fraction_int(interline: i32, value: f64) -> i32 {
    (f64::from(interline) * value).round_ties_even() as i32
}

fn area_int(interline: i32, value: f64) -> i32 {
    (f64::from(interline) * f64::from(interline) * value).round_ties_even() as i32
}

/// Runs the TIME stage against `system`, mirroring `HeaderTimeColumn.retrieveTime`.
///
/// `pair_ids` gates numerator/denominator pairing in the lifecycle, and the ids are allocated by
/// the recognizer, so the driver cannot know them upfront. Allocation is deterministic, so the
/// stage runs twice: a discovery pass replays the exact per-staff `classify_time_parts` sequence
/// the column will make and harvests the number-proposal ids, then the graded pass reruns with
/// the full numerator x denominator crossing pre-allocated.
fn run_time_stage(
    name: &str,
    sheet_interline: i32,
    oracle_staves: &[&OracleStaff],
    recognition: &audiveris_omr::recognize::GridLinesRecognition,
    time_sources: &BTreeMap<usize, audiveris_image::run_table::RunTable>,
    system: &mut HeadlessHeaderSystem,
) -> Result<
    (
        Option<audiveris_omr::header_time_column::NeutralTimeValue>,
        i32,
    ),
    String,
> {
    use audiveris_omr::header_time_column::VisualHeaderTimeProposalRecognizer;

    let build_recognizer = || {
        let mut raster_contexts = BTreeMap::new();
        let mut time_parameters = BTreeMap::new();
        for oracle in oracle_staves {
            let lines = recognition
                .staff_lines
                .iter()
                .find(|lines| lines.staff_id == oracle.id)
                .expect("staff present");
            let staff_interline = oracle.specific_interline;
            let browse_start = system
                .staffs
                .iter()
                .find(|staff| staff.id == oracle.id)
                .and_then(|staff| staff.header.as_ref())
                .map_or(oracle.header_start, |header| header.stop);
            // Java `HeaderTimeBuilder.getRoi`, quirk included: `bottom` compares
            // `lastLine.yAt(stop)` with itself -- the `yAt(start)` the symmetry suggests is
            // absent in the original, so it is absent here too.
            let roi_width = fraction_int(sheet_interline, 4.0);
            // Java `Staff.getBrowseStop(xMin, xMax)`: the first good connected barline inside the
            // window pulls the stop to just before itself. Without it the ROI runs past the
            // header into the first measure, whose notes classify as a plausible time signature
            // -- the port invented a 3/4 on batuque's second system exactly that way.
            let mut stop = browse_start + roi_width - 1;
            for &bar_x in &oracle.bars {
                if bar_x > stop {
                    break;
                }
                if bar_x > browse_start {
                    stop = bar_x - 1;
                    break;
                }
            }
            let roi_width = stop - browse_start + 1;
            let top = lines
                .first_line_y_at(browse_start)
                .unwrap_or_default()
                .min(lines.first_line_y_at(stop).unwrap_or_default());
            let bottom = lines.last_line_y_at(stop).unwrap_or_default();
            raster_contexts.insert(
                oracle.id,
                HeaderTimeRasterContext {
                    roi: audiveris_omr::staff_header::HeaderBounds {
                        x: browse_start,
                        y: top,
                        width: roi_width,
                        height: bottom - top + 1,
                    },
                },
            );
            time_parameters.insert(
                oracle.id,
                NativeHeaderTimeParameters {
                    staff_interline,
                    maximum_first_space_width: fraction_int(sheet_interline, 2.5),
                    maximum_space_cumul: usize::try_from(fraction_int(staff_interline, 0.4))
                        .unwrap_or(0),
                    minimum_time_width: fraction_int(staff_interline, 1.0),
                    vertical_margin: fraction_int(staff_interline, 0.10),
                    minimum_part_weight: usize::try_from(area_int(staff_interline, 0.01))
                        .unwrap_or(0),
                    maximum_part_gap: f64::from(fraction_int(staff_interline, 1.0)),
                    maximum_time_width: fraction_int(staff_interline, 2.0),
                    minimum_whole_weight: usize::try_from(area_int(staff_interline, 1.0))
                        .unwrap_or(0),
                    minimum_half_weight: usize::try_from(area_int(staff_interline, 0.75))
                        .unwrap_or(0),
                    maximum_eval_rank: 3,
                    minimum_classifier_grade: 0.1 / 0.8,
                    intrinsic_ratio: 0.8,
                },
            );
        }
        NativeHeaderTimeRecognizer::new(
            BundledTimeClassifier::bundled().expect("bundled time classifier"),
            time_sources.clone(),
            raster_contexts,
            time_parameters,
            20_000,
            20_000,
        )
    };

    // Discovery pass: replay the column's per-staff sequence and harvest number ids.
    let mut discovery = build_recognizer();
    let mut pair_ids_by_staff: BTreeMap<usize, BTreeMap<(usize, usize), usize>> = BTreeMap::new();
    let mut next_pair_id = 50_000usize;
    for staff in &system.staffs {
        if staff.tablature {
            continue;
        }
        let Some(header) = staff.header.as_ref() else {
            continue;
        };
        let input = audiveris_omr::header_time_column::HeaderTimeRecognitionInput {
            system_id: 1,
            staff_id: staff.id,
            browse_start: header.stop,
        };
        let mut range = audiveris_omr::staff_header::StaffHeaderRange::default();
        range.browse_start = header.stop;
        let proposals = discovery
            .classify_time_parts(input, &range)
            .map_err(|error| format!("{name}: time discovery failed: {error:?}"))?;
        let entry = pair_ids_by_staff.entry(staff.id).or_default();
        for numerator in &proposals.numerators {
            for denominator in &proposals.denominators {
                entry.insert((numerator.id, denominator.id), next_pair_id);
                next_pair_id += 1;
            }
        }
    }

    let mut lifecycle = BTreeMap::new();
    for oracle in oracle_staves {
        lifecycle.insert(
            oracle.id,
            HeaderTimeLifecycleContext {
                maximum_halves_dx: fraction_int(oracle.specific_interline, 1.0),
                top_bottom_source_ratio: 5.0,
                pair_ids: pair_ids_by_staff
                    .get(&oracle.id)
                    .cloned()
                    .unwrap_or_default(),
                // `AbstractTimeInter.defaultTimes` plus the configured `optionalTimes` (6/4 is in
                // both; the set semantics make the duplicate harmless).
                supported_values: vec![
                    (2, 2),
                    (3, 2),
                    (2, 4),
                    (3, 4),
                    (4, 4),
                    (5, 4),
                    (6, 4),
                    (3, 8),
                    (6, 8),
                    (9, 8),
                    (12, 8),
                    (7, 8),
                ],
            },
        );
    }
    let mut time_column = HeadlessHeaderTimeColumn::new(HeaderTimeLifecycleRecognizer::new(
        build_recognizer(),
        lifecycle,
    ));
    let time_offset = time_column
        .retrieve_time(system)
        .map_err(|error| format!("{name}: retrieve_time failed: {error:?}"))?;
    Ok((time_column.time_value(), time_offset))
}

/// One `erase` row: `(system, x, stop, first line y, last line y)`.
type EraseRow = (usize, i32, i32, i32, i32);

struct NativeHeaderPage {
    name: String,
    recognition: audiveris_omr::recognize::GridLinesRecognition,
    header_erases: Vec<audiveris_image::spots::HeaderErase>,
}

/// The `erase system <s> x <x> stop <x2> firstline <y1> lastline <y2>` rows of the beam oracle,
/// keyed by page name: what Java's `SpotsBuilder.eraseHeaderAreas` actually erased.
fn parse_beam_erases() -> BTreeMap<String, Vec<EraseRow>> {
    let text = std::fs::read_to_string(repo_path("rust/oracle/beam-spots.txt"))
        .expect("the beam-spots oracle is checked in");
    let mut map: BTreeMap<String, Vec<EraseRow>> = BTreeMap::new();
    let mut page = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("sheet ") {
            page = rest.split('#').next().unwrap_or("").trim().to_owned();
        } else if line.starts_with("erase ") {
            let f: Vec<&str> = line.split_whitespace().collect();
            map.entry(page.clone()).or_default().push((
                f[2].parse().unwrap(),
                f[4].parse().unwrap(),
                f[6].parse().unwrap(),
                f[8].parse().unwrap(),
                f[10].parse().unwrap(),
            ));
        }
    }
    map
}

fn run_native_headers() -> Vec<NativeHeaderPage> {
    // 65 of 65 staves exact -- shape presence/absence, fifths, union box and keyStop.
    //
    // The grade history is the record of what the port was missing, in order:
    //
    //   34/34 key-bearing failing  subset enumeration (GlyphCluster.decompose) absent
    //   -> 29                      flats' two-heuristic pitch (AlterInter.computePitch) absent
    //   -> 28                      candidate purge (purgeCandidates) absent
    //   -> 20                      the projection-peak pipeline itself absent: signature counted
    //                              from stem peaks before classification, slices allocated per
    //                              expected alteration, best-per-slice, neighbour-cropped second
    //                              pass (key_peaks.rs + the classify_key_shapes rewiring)
    //   -> 3                       the clef-guided third pass (fillMissingAlters) absent
    //   -> 1                       Java's `KeySlice.setPitchRect` truncates its pitch: an `int`
    //                              compound assignment narrows (int)(3 - 1.0559) to 1, placing
    //                              the hunt window 7 px higher than the fractional formula would
    //   -> 0
    //
    // Every step was diagnosed by this test's failure pattern plus targeted instrumentation, and
    // the last one by the per-alter boxes added to the oracle for exactly that purpose.
    let pages = parse_oracle();
    let beam_erases = parse_beam_erases();
    let mut checked = 0;
    let mut with_key = 0;
    let mut with_time = 0;
    let mut erases_checked = 0;
    let mut mismatches: Vec<String> = Vec::new();
    let mut native_pages = Vec::new();

    for (name, sheet_interline, oracle_staves) in &pages {
        let recognition = recognize_grid_lines(repo_path(&format!("data/examples/{name}")))
            .unwrap_or_else(|error| panic!("{name}: GRID failed: {error}"));
        let sheet = SheetClefParameters::new(*sheet_interline);
        let mut page_erases = Vec::new();

        // ---- clef stage, whose clefStop the key stage browses from ----
        let geometries: Vec<ClefLookupStaffGeometry> = oracle_staves
            .iter()
            .filter_map(|oracle| {
                let lines = recognition
                    .staff_lines
                    .iter()
                    .find(|lines| lines.staff_id == oracle.id)?;
                let browse_start = oracle.header_start;
                let x_mid = (browse_start + browse_start + sheet.max_clef_end) / 2;
                Some(ClefLookupStaffGeometry {
                    staff_id: oracle.id,
                    browse_start,
                    browse_stop: browse_start + sheet.max_clef_end,
                    left_abscissa: lines.left,
                    right_abscissa: lines.right,
                    first_line_y: lines.first_line_y_at(x_mid).unwrap_or_default(),
                    last_line_y: lines.last_line_y_at(x_mid).unwrap_or_default(),
                    percussion_only: false,
                })
            })
            .collect();
        let clef_contexts = build_clef_lookup_contexts_at(
            &geometries,
            ClefLookupParameters {
                sheet_height: i32::try_from(recognition.scale.height).unwrap_or(i32::MAX),
                above_staff: StaffClefParameters::new(*sheet_interline).above_staff,
                below_staff: StaffClefParameters::new(*sheet_interline).below_staff,
                belt_margin: sheet.belt_margin,
                x_core_margin: sheet.x_core_margin,
                y_core_margin: sheet.y_core_margin,
            },
            &GridOrdinates(&recognition.staff_lines),
        );

        let mut clef_parameters = BTreeMap::new();
        for oracle in oracle_staves {
            let staff = StaffClefParameters::new(oracle.specific_interline);
            let lines = recognition
                .staff_lines
                .iter()
                .find(|lines| lines.staff_id == oracle.id)
                .expect("staff present");
            let x_mid = (oracle.header_start + oracle.header_start + sheet.max_clef_end) / 2;
            clef_parameters.insert(
                oracle.id,
                NativeClefParameters {
                    staff_interline: oracle.specific_interline,
                    first_line_y: f64::from(lines.first_line_y_at(x_mid).unwrap_or_default()),
                    last_line_y: f64::from(lines.last_line_y_at(x_mid).unwrap_or_default()),
                    min_part_weight: usize::try_from(staff.min_part_weight).unwrap_or(0),
                    max_part_count: audiveris_omr::clef_parameters::max_part_count(),
                    max_part_gap: staff.max_part_gap,
                    max_glyph_height: staff.max_glyph_height,
                    min_glyph_weight: usize::try_from(staff.min_glyph_weight).unwrap_or(0),
                    max_eval_rank: audiveris_omr::clef_parameters::max_eval_rank(),
                    minimum_classifier_grade: MINIMUM_CLEF_GRADE,
                    f_area_pitch_offset: 0.0,
                },
            );
        }

        let system_count = recognition.system_bounds.len().max(1);
        let mut sources = BTreeMap::new();
        for system_id in 1..=system_count {
            sources.insert(system_id, recognition.no_staff.clone());
        }

        // Java runs the header per *system*: clef/key stop propagation is system-wide, and TIME
        // demands every staff of the system agree on a value. Modelling the page as one system
        // made staves without a time signature veto the ones with -- the uniform absence the
        // first time run produced. The membership comes from GRID's peak graph.
        for (system_index, member_ids) in recognition.peak_graph.systems.iter().enumerate() {
            let system_staves: Vec<&OracleStaff> = oracle_staves
                .iter()
                .filter(|oracle| member_ids.contains(&oracle.id))
                .collect();
            if system_staves.is_empty() {
                continue;
            }
            let oracle_staves = &system_staves;
            let staffs: Vec<HeadlessHeaderStaff> = oracle_staves
                .iter()
                .map(|oracle| {
                    let mut staff = HeadlessHeaderStaff::new(oracle.id);
                    staff.maximum_clef_end = sheet.max_clef_end;
                    staff.header = Some(StaffHeader::new(oracle.header_start));
                    staff
                })
                .collect();
            let mut system = HeadlessHeaderSystem::new(system_index + 1, staffs);

            let mut clef_column = HeadlessClefColumn::new(ClefLifecycleRecognizer::new(
                NativeClefProposalRecognizer::new(
                    BundledClefClassifier::bundled().expect("bundled clef classifier"),
                    sources.clone(),
                    clef_contexts.clone(),
                    clef_parameters.clone(),
                    1,
                    1,
                ),
                clef_contexts.clone(),
                INTRINSIC_RATIO,
                0.0,
            ));

            let clef_offset = match clef_column.retrieve_clefs(&mut system) {
                Ok(offset) => offset,
                Err(error) => {
                    mismatches.push(format!("{name}: retrieve_clefs failed: {error:?}"));
                    continue;
                }
            };
            // Java `HeaderBuilder.setSystemClefStop`: the system-wide largest offset advances every
            // staff's header stop. `selectClefs` runs only after TIME, so the key stage browses from
            // the *stored* clef range stop, exactly as `StaffHeader::clef_stop` falls back.
            if clef_offset > 0 {
                for staff in &mut system.staffs {
                    if let Some(header) = staff.header.as_mut() {
                        header.stop = header.start + clef_offset;
                    }
                }
            }

            // ---- key stage ----
            let mut key_contexts = BTreeMap::new();
            let mut key_parameters = BTreeMap::new();
            let mut lifecycle_contexts = BTreeMap::new();
            for oracle in oracle_staves {
                let lines = recognition
                    .staff_lines
                    .iter()
                    .find(|lines| lines.staff_id == oracle.id)
                    .expect("staff present");
                let interline = oracle.specific_interline;
                let extractor = KeyExtractorParameters::new(interline);
                let browse_start = system
                    .staffs
                    .iter()
                    .find(|staff| staff.id == oracle.id)
                    .and_then(|staff| staff.header.as_ref())
                    .and_then(StaffHeader::clef_stop)
                    .map_or(oracle.header_start, |stop| stop + 1);
                let browse_stop = oracle.header_start + max_header_width(*sheet_interline) - 1;
                // Java's `getBrowseRect` reads both lines at `xMin` only; see `browse_envelope`.
                let (envelope_top, envelope_bottom) = browse_envelope(
                    lines.first_line_y_at(browse_start).unwrap_or_default(),
                    lines.last_line_y_at(browse_start).unwrap_or_default(),
                    interline,
                );
                key_contexts.insert(
                    oracle.id,
                    NativeKeyStaffContext {
                        browse_stop,
                        envelope_top,
                        envelope_bottom,
                        staff_mid_y: f64::from(envelope_top + envelope_bottom) / 2.0,
                        // Java hands the classifier the *sheet* interline here, not the staff's.
                        classifier_interline: *sheet_interline,
                        line_count: 5,
                        staff_interline: oracle.specific_interline,
                    },
                );
                key_parameters.insert(
                    oracle.id,
                    NativeKeyParameters {
                        minimum_component_weight: usize::try_from(extractor.minimum_part_weight)
                            .unwrap_or(0),
                        maximum_component_gap: extractor.maximum_part_gap,
                        minimum_glyph_weight: usize::try_from(extractor.minimum_glyph_weight)
                            .unwrap_or(0),
                        maximum_glyph_weight: usize::try_from(extractor.maximum_glyph_weight)
                            .unwrap_or(usize::MAX),
                        minimum_alter_width: extractor.minimum_glyph_width,
                        minimum_alter_height: extractor.minimum_glyph_height,
                        maximum_alter_width: extractor.maximum_glyph_width,
                        maximum_alter_height: extractor.maximum_glyph_height,
                        maximum_alters: 7,
                        maximum_rank: max_eval_rank(),
                        minimum_classifier_grade: MINIMUM_KEY_GRADE,
                        pipeline: KeyPipelineParameters::new(
                            *sheet_interline,
                            oracle.specific_interline,
                        ),
                    },
                );
                let clefs = clef_column
                    .builders()
                    .get(&oracle.id)
                    .map(|builder| {
                        builder
                            .candidates
                            .iter()
                            .map(|candidate| audiveris_omr::key_column::KeyClefSupport {
                                id: candidate.id,
                                kind: candidate.kind,
                                grade: candidate.grade,
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                lifecycle_contexts.insert(
                    oracle.id,
                    KeyLifecycleContext {
                        clefs,
                        maximum_delta_pitch_one: MAX_DELTA_PITCH_ONE,
                        maximum_delta_pitch_four: MAX_DELTA_PITCH_FOUR,
                        clef_key_source_ratio: CLEF_KEY_SOURCE_RATIO,
                        key_alters_source_ratio: KEY_ALTERS_SOURCE_RATIO,
                    },
                );
            }

            let _ = max_part_count();

            let time_sources = sources.clone();
            let clef_supports: BTreeMap<usize, Vec<audiveris_omr::key_column::KeyClefSupport>> =
                lifecycle_contexts
                    .iter()
                    .map(|(staff_id, context)| (*staff_id, context.clefs.clone()))
                    .collect();
            let mut key_column = HeadlessKeyColumn::new(
                KeyLifecycleRecognizer::new(
                    NativeKeyProposalRecognizer::new(
                        BundledKeyClassifier::bundled().expect("bundled key classifier"),
                        GridPitch(recognition.staff_lines.clone()),
                        sources.clone(),
                        key_contexts.clone(),
                        key_parameters.clone(),
                        10_000,
                        10_000,
                    )
                    .with_clef_supports(clef_supports),
                    lifecycle_contexts,
                ),
                max_slice_distance(*sheet_interline),
            );

            let key_offset =
                match key_column.retrieve_keys(&mut system, max_header_width(*sheet_interline)) {
                    Ok(offset) => offset,
                    Err(error) => {
                        mismatches.push(format!("{name}: retrieve_keys failed: {error:?}"));
                        continue;
                    }
                };
            if key_offset > 0 {
                for staff in &mut system.staffs {
                    if let Some(header) = staff.header.as_mut() {
                        header.stop = header.start + key_offset;
                    }
                }
            }

            let (time_value, time_offset) = match run_time_stage(
                name,
                *sheet_interline,
                oracle_staves,
                &recognition,
                &time_sources,
                &mut system,
            ) {
                Ok(value) => value,
                Err(error) => {
                    mismatches.push(error);
                    continue;
                }
            };
            // Java `HeaderBuilder.setSystemTimeStop`: the final system-wide advance of header.stop.
            if time_offset > 0 {
                for staff in &mut system.staffs {
                    if let Some(header) = staff.header.as_mut() {
                        header.stop = header.start + time_offset;
                    }
                }
            }

            // Java selects clefs only now, after the whole header is browsed.
            if let Err(error) = clef_column.select_clefs(&mut system) {
                mismatches.push(format!("{name}: select_clefs failed: {error:?}"));
                continue;
            }

            for oracle in oracle_staves {
                checked += 1;
                let produced = system
                    .staffs
                    .iter()
                    .find(|staff| staff.id == oracle.id)
                    .and_then(|staff| staff.header.as_ref())
                    .and_then(|header| header.key.as_ref())
                    .map(|key| {
                        (
                            key.bounds.x,
                            key.bounds.y,
                            key.bounds.width,
                            key.bounds.height,
                        )
                    });
                match (&oracle.key, produced) {
                    (None, None) => {}
                    (None, Some(bounds)) => mismatches.push(format!(
                        "{name} staff {}: produced a key at {bounds:?}, Java found none",
                        oracle.id
                    )),
                    (Some(expected), None) => {
                        with_key += 1;
                        mismatches.push(format!(
                            "{name} staff {}: no key, Java found {} fifths at {:?}",
                            oracle.id, expected.fifths, expected.bounds
                        ));
                    }
                    (Some(expected), Some(bounds)) => {
                        with_key += 1;
                        if bounds != expected.bounds {
                            mismatches.push(format!(
                                "{name} staff {}: key box {bounds:?}, Java {:?}",
                                oracle.id, expected.bounds
                            ));
                        }
                        // Java `Staff.getKeyStop()` is the inclusive right edge of the key bounds.
                        let stop = bounds.0 + bounds.2 - 1;
                        if Some(stop) != expected.stop {
                            mismatches.push(format!(
                                "{name} staff {}: keyStop {stop}, Java {:?}",
                                oracle.id, expected.stop
                            ));
                        }
                        let fifths = key_column
                            .builders()
                            .get(&oracle.id)
                            .and_then(|builder| {
                                builder.candidates.iter().find(|candidate| candidate.frozen)
                            })
                            .map(|candidate| i32::from(candidate.fifths));
                        if fifths != Some(expected.fifths) {
                            mismatches.push(format!(
                                "{name} staff {}: fifths {fifths:?}, Java {}",
                                oracle.id, expected.fifths
                            ));
                        }
                    }
                }
            }

            // ---- time grading ----
            for oracle in oracle_staves {
                let produced = system
                    .staffs
                    .iter()
                    .find(|staff| staff.id == oracle.id)
                    .and_then(|staff| staff.header.as_ref())
                    .and_then(|header| header.time.as_ref())
                    .map(|time| {
                        (
                            time.bounds.x,
                            time.bounds.y,
                            time.bounds.width,
                            time.bounds.height,
                        )
                    });
                match (&oracle.time, produced) {
                    (None, None) => {}
                    (None, Some(bounds)) => mismatches.push(format!(
                        "{name} staff {}: produced a time at {bounds:?}, Java found none",
                        oracle.id
                    )),
                    (Some(expected), None) => {
                        with_time += 1;
                        mismatches.push(format!(
                            "{name} staff {}: no time, Java found {}/{} at {:?}",
                            oracle.id, expected.numerator, expected.denominator, expected.bounds
                        ));
                    }
                    (Some(expected), Some(bounds)) => {
                        with_time += 1;
                        if bounds != expected.bounds {
                            mismatches.push(format!(
                                "{name} staff {}: time box {bounds:?}, Java {:?}",
                                oracle.id, expected.bounds
                            ));
                        }
                        // Java `Staff.getTimeStop()` recomputes the inclusive right edge from
                        // `header.time`, shadowing the stored value -- the third getter of this kind.
                        let stop = bounds.0 + bounds.2 - 1;
                        if Some(stop) != expected.stop {
                            mismatches.push(format!(
                                "{name} staff {}: timeStop {stop}, Java {:?}",
                                oracle.id, expected.stop
                            ));
                        }
                        match time_value {
                        Some(value) => {
                            if value.specific_shape != expected.specific
                                || value.numerator != expected.numerator
                                || value.denominator != expected.denominator
                            {
                                mismatches.push(format!(
                                    "{name} staff {}: time value {value:?}, Java {:?}/{}/{}",
                                    oracle.id,
                                    expected.specific,
                                    expected.numerator,
                                    expected.denominator
                                ));
                            }
                        }
                        None => mismatches.push(format!(
                            "{name} staff {}: column agreed no time value, yet a header time exists",
                            oracle.id
                        )),
                    }
                    }
                }
            }

            // ---- native header erase vs Java's SpotsBuilder.eraseHeaderAreas ----
            //
            // This is what retires the beams caveat: the erase rectangle is now computed from the
            // *native* header stop and staff lines and must equal what Java erased. Java reads the
            // first non-tablature headered staff's stop, and the system's first/last staff lines at
            // that abscissa.
            if let Some(stop) = system
                .staffs
                .iter()
                .find_map(|staff| staff.header.as_ref().map(|header| header.stop))
            {
                let first_id = member_ids.first().copied().expect("non-empty system");
                let last_id = member_ids.last().copied().expect("non-empty system");
                let line_at = |staff_id: usize, first: bool| -> i32 {
                    recognition
                        .staff_lines
                        .iter()
                        .find(|lines| lines.staff_id == staff_id)
                        .and_then(|lines| {
                            if first {
                                lines.first_line_y_at(stop)
                            } else {
                                lines.last_line_y_at(stop)
                            }
                        })
                        .unwrap_or_default()
                };
                let first = line_at(first_id, true);
                let last = line_at(last_id, false);
                let x = recognition
                    .system_areas
                    .iter()
                    .find(|area| area.system_id == system_index + 1)
                    .map_or(0, |area| area.left);
                let margin = (2.0 * f64::from(*sheet_interline)).round_ties_even() as i32;
                page_erases.push(audiveris_image::spots::HeaderErase {
                    x,
                    stop,
                    top: first - margin,
                    bottom: last + margin,
                });

                if let Some(&(_, expected_x, expected_stop, expected_first, expected_last)) =
                    beam_erases.get(name.as_str()).and_then(|rows| {
                        rows.iter()
                            .find(|(system_id, ..)| *system_id == system_index + 1)
                    })
                {
                    let produced = (x, stop, first, last);
                    let expected = (expected_x, expected_stop, expected_first, expected_last);
                    erases_checked += 1;
                    if produced != expected {
                        mismatches.push(format!(
                            "{name} system {}: header erase {produced:?}, Java {expected:?}",
                            system_index + 1
                        ));
                    }
                }
            }

            // ---- header stop grading: the value getHeaderStop() serves to BEAMS/STEM_SEEDS ----
            for oracle in oracle_staves {
                let produced = system
                    .staffs
                    .iter()
                    .find(|staff| staff.id == oracle.id)
                    .and_then(|staff| staff.header.as_ref())
                    .map(|header| header.stop);
                if produced != Some(oracle.header_stop) {
                    mismatches.push(format!(
                        "{name} staff {}: header stop {produced:?}, Java {}",
                        oracle.id, oracle.header_stop
                    ));
                }
            }
        } // per-system
        native_pages.push(NativeHeaderPage {
            name: name.clone(),
            recognition,
            header_erases: page_erases,
        });
    }

    assert_eq!(checked, 65, "every oracle staff was compared");
    assert_eq!(with_key, 34, "the 34 key-bearing staves were reached");
    assert_eq!(with_time, 17, "the 17 time-bearing staves were reached");
    assert_eq!(
        erases_checked,
        beam_erases.values().map(Vec::len).sum::<usize>(),
        "every Java header-erase rectangle was compared against the native one"
    );
    assert!(
        mismatches.is_empty(),
        "{} of {checked} staves disagree with Java:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
    native_pages
}

#[test]
fn native_headers_match_java_on_every_corpus_staff() {
    assert_eq!(
        run_native_headers().len(),
        9,
        "every corpus page ran through GRID and HEADERS"
    );
}

/// Records grouped by the `sheet` row that starts each oracle segment.
fn beam_oracle_pages(text: &'static str) -> Vec<(String, Vec<Vec<&'static str>>)> {
    let mut pages: Vec<(String, Vec<Vec<&'static str>>)> = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields[0] == "sheet" {
            pages.push((fields[1].to_owned(), Vec::new()));
        }
        pages
            .last_mut()
            .expect("an oracle record follows a sheet row")
            .1
            .push(fields);
    }
    pages
}

fn raw_beam_key(system_id: usize, raw: audiveris_omr::beam_inters::RawBeam) -> String {
    format!(
        "{system_id} {} {} {:.9} {:.9} {:.9} {:.9} {:.9} grade {:.9} impacts \
         wdth {:.9} minH {:.9} maxH {:.9} core {:.9} belt {:.9} jit {:.9}",
        raw.kind.class_name(),
        raw.kind.shape(),
        raw.item.median.x1,
        raw.item.median.y1,
        raw.item.median.x2,
        raw.item.median.y2,
        raw.item.height,
        raw.grade,
        raw.impacts.width,
        raw.impacts.min_height,
        raw.impacts.max_height,
        raw.impacts.core,
        raw.impacts.belt,
        raw.impacts.distance,
    )
}

fn multiset_difference<T: Clone + PartialEq>(mine: &[T], theirs: &[T]) -> (Vec<T>, Vec<T>) {
    let mut spurious = mine.to_vec();
    for expected in theirs {
        if let Some(index) = spurious.iter().position(|actual| actual == expected) {
            spurious.remove(index);
        }
    }
    let mut missing = theirs.to_vec();
    for actual in mine {
        if let Some(index) = missing.iter().position(|expected| expected == actual) {
            missing.remove(index);
        }
    }
    (spurious, missing)
}

/// A final beam keyed by system, class, bounds, grade, and all six impacts.
fn expected_final_beams(records: &[Vec<&'static str>]) -> Vec<(usize, String, Vec<String>)> {
    records
        .iter()
        .filter(|fields| fields.first() == Some(&"inter"))
        .filter(|fields| matches!(fields[3], "BeamInter" | "BeamHookInter" | "SmallBeamInter"))
        .map(|fields| {
            let bounds = fields
                .iter()
                .position(|field| *field == "bounds")
                .expect("a beam has bounds");
            let grade = fields
                .iter()
                .position(|field| *field == "grade")
                .expect("a beam has a grade");
            let impacts = fields
                .iter()
                .position(|field| *field == "impacts")
                .expect("a beam has impacts");
            let mut evidence = fields[bounds + 1..bounds + 5]
                .iter()
                .map(|field| (*field).to_owned())
                .collect::<Vec<_>>();
            evidence.extend(["grade".to_owned(), fields[grade + 1].to_owned()]);
            evidence.extend(
                fields[impacts + 1..impacts + 13]
                    .iter()
                    .map(|field| (*field).to_owned()),
            );
            (
                fields[1].parse().expect("a system id"),
                fields[3].to_owned(),
                evidence,
            )
        })
        .collect()
}

fn produced_final_beams(
    recognition: &audiveris_omr::recognize::NativeBeamRecognition,
) -> Vec<(usize, String, Vec<String>)> {
    recognition
        .raw_beams
        .iter()
        .chain(&recognition.hooks)
        .map(|(system_id, raw)| {
            let bounds = audiveris_omr::beam_inters::beam_bounds(raw.item);
            (
                *system_id,
                raw.kind.class_name().to_owned(),
                vec![
                    bounds.x.to_string(),
                    bounds.y.to_string(),
                    bounds.width.to_string(),
                    bounds.height.to_string(),
                    "grade".to_owned(),
                    format!("{:.9}", raw.grade),
                    "wdth".to_owned(),
                    format!("{:.9}", raw.impacts.width),
                    "minH".to_owned(),
                    format!("{:.9}", raw.impacts.min_height),
                    "maxH".to_owned(),
                    format!("{:.9}", raw.impacts.max_height),
                    "core".to_owned(),
                    format!("{:.9}", raw.impacts.core),
                    "belt".to_owned(),
                    format!("{:.9}", raw.impacts.belt),
                    "jit".to_owned(),
                    format!("{:.9}", raw.impacts.distance),
                ],
            )
        })
        .collect()
}

fn ledger_oracle_summaries(input: &str, record_name: &str) -> BTreeMap<String, (usize, String)> {
    input
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            (fields.first() == Some(&record_name)).then(|| {
                (
                    fields[1].to_owned(),
                    (
                        fields[2].parse().expect("a ledger count"),
                        fields[3].to_owned(),
                    ),
                )
            })
        })
        .collect()
}

fn produced_final_ledgers(
    recognition: &audiveris_omr::native_ledgers::NativeLedgerRecognition,
) -> Vec<String> {
    recognition
        .ledgers()
        .iter()
        .map(|ledger| {
            format!(
                "ledger {} {} {} {:.9} {:.9} {:.9} {:.9} {:.9} {:.9} {}",
                ledger.system_id,
                ledger.staff_id,
                ledger.ledger_index,
                ledger.median.0.0,
                ledger.median.0.1,
                ledger.median.1.0,
                ledger.median.1.1,
                ledger.thickness,
                ledger.grade,
                ledger
                    .impacts
                    .iter()
                    .map(|impact| format!("{:.9}", impact.grade))
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        })
        .collect()
}

fn produced_final_ledger_lines(
    recognition: &audiveris_omr::native_ledgers::NativeLedgerRecognition,
) -> Vec<String> {
    recognition
        .ledger_lines
        .iter()
        .map(|line| {
            format!(
                "ledgerline {} {} {} {:.9}",
                line.system_id, line.staff_id, line.index, line.translation_y
            )
        })
        .collect()
}

#[test]
fn native_grid_headers_and_beams_match_java_on_every_beam_sheet() {
    let structures = beam_oracle_pages(include_str!("../../../oracle/beam-structures.txt"));
    let spots = beam_oracle_pages(include_str!("../../../oracle/beam-spots.txt"));
    let sig = beam_oracle_pages(include_str!("../../../oracle/beams-sig.txt"));
    let ledger_oracle = include_str!("../../../oracle/ledgers-corpus.txt");
    let ledger_summaries = ledger_oracle_summaries(ledger_oracle, "ledgersummary");
    let ledger_line_summaries = ledger_oracle_summaries(ledger_oracle, "ledgerlinesummary");
    let native_pages = run_native_headers();

    assert_eq!(structures.len(), 8, "eight beam sheets are pinned");
    assert_eq!(ledger_summaries.len(), 8, "eight ledger sheets are pinned");
    assert_eq!(
        ledger_line_summaries.len(),
        8,
        "eight ledger-line sheets are pinned"
    );
    let mut ungraded_small = native_pages[0].recognition.clone();
    ungraded_small.scale.scale.small_beam = Some(audiveris_image::scale_estimate::BeamScale {
        main: 7,
        extrapolated: false,
    });
    assert!(matches!(
        audiveris_omr::recognize::recognize_native_beams(
            &ungraded_small,
            native_pages[0].header_erases.clone(),
        ),
        Err(
            audiveris_omr::recognize::NativeBeamRecognitionError::UnsupportedSmallBeam {
                small: 7,
                ..
            }
        )
    ));
    let mut checked = 0;
    let mut raw_checked = 0;
    let mut spots_checked = 0;
    let mut erases_checked = 0;
    let mut ledgers_checked = 0;
    let mut ledger_lines_checked = 0;
    let mut failures = Vec::new();
    let mut ledger_failures = Vec::new();

    for (page, structure_records) in &structures {
        let file = page.split('#').next().expect("a file name");
        let native = native_pages
            .iter()
            .find(|native| native.name == file)
            .unwrap_or_else(|| panic!("no native HEADERS result for {page}"));
        let spot_records = &spots
            .iter()
            .find(|(name, _)| name == page)
            .unwrap_or_else(|| panic!("no spot oracle for {page}"))
            .1;
        let sig_records = &sig
            .iter()
            .find(|(name, _)| name == page)
            .unwrap_or_else(|| panic!("no SIG oracle for {page}"))
            .1;

        let produced = audiveris_omr::recognize::recognize_native_beams(
            &native.recognition,
            native.header_erases.clone(),
        )
        .unwrap_or_else(|error| panic!("{page}: native BEAMS failed: {error}"));

        let ledgers =
            audiveris_omr::native_ledgers::recognize_native_ledgers(&native.recognition, &produced)
                .unwrap_or_else(|error| panic!("{page}: native LEDGERS failed: {error}"));
        let mut actual_ledgers = produced_final_ledgers(&ledgers);
        actual_ledgers.sort();
        let canonical = actual_ledgers
            .iter()
            .map(|ledger| format!("{ledger}\n"))
            .collect::<String>();
        let actual_summary = (
            actual_ledgers.len(),
            format!(
                "{:016x}",
                audiveris_image::ingest::fnv1a64_bytes(canonical.as_bytes())
            ),
        );
        ledgers_checked += actual_ledgers.len();
        let expected_summary = ledger_summaries
            .get(page)
            .unwrap_or_else(|| panic!("no ledger summary oracle for {page}"));
        if &actual_summary != expected_summary {
            ledger_failures.push(format!(
                "{page}: native {actual_summary:?}, Java {expected_summary:?}; {} candidates, {} builder survivors, {} direct rejects, rebuilt {:?}",
                ledgers.candidates.len(),
                ledgers.builder_survivor_count,
                ledgers.discarded_filament_ids.len(),
                ledgers.rebuilt_system_ids,
            ));
        }

        let mut actual_lines = produced_final_ledger_lines(&ledgers);
        actual_lines.sort();
        let canonical_lines = actual_lines
            .iter()
            .map(|line| format!("{line}\n"))
            .collect::<String>();
        let actual_line_summary = (
            actual_lines.len(),
            format!(
                "{:016x}",
                audiveris_image::ingest::fnv1a64_bytes(canonical_lines.as_bytes())
            ),
        );
        ledger_lines_checked += actual_lines.len();
        let expected_line_summary = ledger_line_summaries
            .get(page)
            .unwrap_or_else(|| panic!("no ledger-line summary oracle for {page}"));
        if &actual_line_summary != expected_line_summary {
            ledger_failures.push(format!(
                "{page}: native ledger lines {actual_line_summary:?}, Java {expected_line_summary:?}"
            ));
        }

        if file == "chula.png" {
            assert_eq!(ledgers.filtered_run_count, 9_915);
            assert_eq!(ledgers.section_count, 4_052);
            assert_eq!(
                ledgers.system_section_counts,
                vec![(1, 2_039), (2, 577), (3, 961)]
            );
            assert_eq!(ledgers.registered_filament_count, 104);
            assert_eq!(ledgers.candidates.len(), 104);
            assert_eq!(ledgers.builder_survivor_count, 19);
            assert_eq!(ledgers.discarded_filament_ids.len(), 1);
            assert_eq!(ledgers.rebuilt_system_ids, vec![1]);

            let expected = include_str!("../../../oracle/ledgers-chula.txt")
                .lines()
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let missing = expected
                .iter()
                .filter(|ledger| !actual_ledgers.contains(ledger))
                .collect::<Vec<_>>();
            assert!(
                missing.is_empty(),
                "{page}: native builder missed exact Java ledgers {missing:?}"
            );
            assert_eq!(actual_ledgers.len(), expected.len());
        }

        let expected_spots = spot_records
            .iter()
            .find(|fields| fields.first() == Some(&"spotcount"))
            .expect("a spot count")[1]
            .parse::<usize>()
            .expect("a numeric spot count");
        assert_eq!(produced.spot_count, expected_spots, "{page}: spot count");
        spots_checked += produced.spot_count;
        erases_checked += native.header_erases.len();

        let mut expected_raw = structure_records
            .iter()
            .filter(|fields| fields.first() == Some(&"rawbeam"))
            .map(|fields| fields[1..].join(" "))
            .collect::<Vec<_>>();
        let mut actual_raw = produced
            .raw_beams
            .iter()
            .map(|(system_id, raw)| raw_beam_key(*system_id, *raw))
            .collect::<Vec<_>>();
        expected_raw.sort();
        actual_raw.sort();
        raw_checked += actual_raw.len();
        if actual_raw != expected_raw {
            let (spurious, missing) = multiset_difference(&actual_raw, &expected_raw);
            let differing = spurious.len().max(missing.len());
            failures.push(format!(
                "{page}: {differing} raw beams differ ({} vs Java {})",
                actual_raw.len(),
                expected_raw.len()
            ));
        }

        let expected = expected_final_beams(sig_records);
        let actual = produced_final_beams(&produced);
        for kind in ["BeamInter", "BeamHookInter", "SmallBeamInter"] {
            let mut theirs = expected
                .iter()
                .filter(|(_, class, _)| class == kind)
                .cloned()
                .collect::<Vec<_>>();
            let mut mine = actual
                .iter()
                .filter(|(_, class, _)| class == kind)
                .cloned()
                .collect::<Vec<_>>();
            theirs.sort();
            mine.sort();
            if mine.len() != theirs.len() {
                failures.push(format!(
                    "{page}: {kind} count {} vs Java {}",
                    mine.len(),
                    theirs.len()
                ));
            }
            if mine != theirs {
                let (spurious, missing) = multiset_difference(&mine, &theirs);
                let differing = spurious.len().max(missing.len());
                failures.push(format!(
                    "{page}: {kind} -- {differing} of {} differ",
                    theirs.len()
                ));
            }
        }

        let mut expected_groups = BTreeMap::new();
        for fields in sig_records
            .iter()
            .filter(|fields| fields.first() == Some(&"inter") && fields[3] == "BeamGroupInter")
        {
            *expected_groups
                .entry(fields[1].parse::<usize>().expect("a system id"))
                .or_insert(0usize) += 1;
        }
        let mut actual_groups = produced
            .group_counts
            .iter()
            .copied()
            .collect::<BTreeMap<_, _>>();
        for bounds in &native.recognition.system_bounds {
            expected_groups.entry(bounds.system_id).or_insert(0);
            actual_groups.entry(bounds.system_id).or_insert(0);
        }
        if actual_groups != expected_groups {
            failures.push(format!(
                "{page}: beam groups {actual_groups:?} vs Java {expected_groups:?}"
            ));
        }

        checked += 1;
    }

    assert_eq!(checked, 8, "every beam sheet was compared");
    assert_eq!(raw_checked, 787, "all raw beam records were compared");
    assert_eq!(
        spots_checked, 2_739,
        "all native spot components were counted"
    );
    assert_eq!(erases_checked, 30, "all native header erases reached BEAMS");
    assert_eq!(
        ledgers_checked, 581,
        "all final Java ledger inters were compared"
    );
    assert_eq!(
        ledger_lines_checked, 95,
        "all final Java inferred ledger lines were compared"
    );
    assert!(
        ledger_failures.is_empty(),
        "native end-to-end ledger divergences:\n{}",
        ledger_failures.join("\n")
    );

    // `MultipleRestsBuilder` runs after beam recognition and replaces one long
    // BachInvention5 beam with a MultipleRestInter. The native beam result is
    // therefore expected to retain exactly this one source beam until that
    // separate recognizer is wired.
    let known = [
        "BachInvention5.jpg#1: BeamInter count 193 vs Java 192",
        "BachInvention5.jpg#1: BeamInter -- 1 of 192 differ",
    ];
    let unexpected = failures
        .iter()
        .filter(|failure| !known.contains(&failure.as_str()))
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "new native end-to-end beam divergences:\n{}",
        unexpected
            .iter()
            .map(|failure| failure.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(
        failures.len(),
        known.len(),
        "a known divergence disappeared; remove it deliberately:\n{}",
        failures.join("\n")
    );
}
