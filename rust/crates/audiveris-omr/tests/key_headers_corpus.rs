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

#[test]
fn native_keys_match_java_on_every_corpus_staff() {
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

        let mut clef_column = HeadlessClefColumn::new(ClefLifecycleRecognizer::new(
            NativeClefProposalRecognizer::new(
                BundledClefClassifier::bundled().expect("bundled clef classifier"),
                sources.clone(),
                clef_contexts.clone(),
                clef_parameters,
                1,
                1,
            ),
            clef_contexts,
            INTRINSIC_RATIO,
            0.0,
        ));

        let staffs: Vec<HeadlessHeaderStaff> = oracle_staves
            .iter()
            .map(|oracle| {
                let mut staff = HeadlessHeaderStaff::new(oracle.id);
                staff.maximum_clef_end = sheet.max_clef_end;
                staff.header = Some(StaffHeader::new(oracle.header_start));
                staff
            })
            .collect();
        let mut system = HeadlessHeaderSystem::new(1, staffs);

        if let Err(error) = clef_column.retrieve_clefs(&mut system) {
            mismatches.push(format!("{name}: retrieve_clefs failed: {error:?}"));
            continue;
        }
        if let Err(error) = clef_column.select_clefs(&mut system) {
            mismatches.push(format!("{name}: select_clefs failed: {error:?}"));
            continue;
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
                    sources,
                    key_contexts,
                    key_parameters,
                    10_000,
                    10_000,
                )
                .with_clef_supports(clef_supports),
                lifecycle_contexts,
            ),
            max_slice_distance(*sheet_interline),
        );

        if let Err(error) =
            key_column.retrieve_keys(&mut system, max_header_width(*sheet_interline))
        {
            mismatches.push(format!("{name}: retrieve_keys failed: {error:?}"));
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
    }

    assert_eq!(checked, 65, "every oracle staff was compared");
    assert_eq!(with_key, 34, "the 34 key-bearing staves were reached");
    assert!(
        mismatches.is_empty(),
        "{} of {checked} staves disagree with Java:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}
