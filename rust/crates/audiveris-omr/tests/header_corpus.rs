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
) -> Result<Option<audiveris_omr::header_time_column::NeutralTimeValue>, String> {
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
    time_column
        .retrieve_time(system)
        .map_err(|error| format!("{name}: retrieve_time failed: {error:?}"))?;
    Ok(time_column.time_value())
}

#[test]
fn native_headers_match_java_on_every_corpus_staff() {
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
    let mut checked = 0;
    let mut with_key = 0;
    let mut with_time = 0;
    let mut mismatches: Vec<String> = Vec::new();

    for (name, sheet_interline, oracle_staves) in &pages {
        let recognition = recognize_grid_lines(repo_path(&format!("data/examples/{name}")))
            .unwrap_or_else(|error| panic!("{name}: GRID failed: {error}"));
        let sheet = SheetClefParameters::new(*sheet_interline);

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

            let time_value = match run_time_stage(
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
        } // per-system
    }

    assert_eq!(checked, 65, "every oracle staff was compared");
    assert_eq!(with_key, 34, "the 34 key-bearing staves were reached");
    assert_eq!(with_time, 17, "the 17 time-bearing staves were reached");
    assert!(
        mismatches.is_empty(),
        "{} of {checked} staves disagree with Java:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}
