// SPDX-License-Identifier: AGPL-3.0-or-later

//! Grades the native clef stage against Java, staff by staff, on the whole corpus.
//!
//! The oracle is `rust/oracle/clef-headers.txt`, produced by `:app:clefProbe` driving each sheet
//! to HEADERS in a live JVM.
//!
//! **What this isolates, and why.** Java's header start comes from `HeaderBuilder`, which is a
//! separate stage with its own barline dependencies. This test *supplies* Java's header start and
//! grades only what the clef stage does with it — the lookup rectangles, the part extraction and
//! purge, the classifier, and the chosen shape and its box. That keeps a clef disagreement from
//! being blamed on a header-start disagreement, in the same way the spot-dispatch test was graded
//! as a pure function of the centroid. The header start itself needs its own oracle; grading it
//! here would be circular, since it is an input.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use audiveris_omr::clef_classifier::BundledClefClassifier;
use audiveris_omr::clef_column::{
    ClefLifecycleRecognizer, ClefLookupParameters, ClefLookupStaffGeometry, HeadlessClefColumn,
    NativeClefParameters, NativeClefProposalRecognizer, StaffLineOrdinates,
    build_clef_lookup_contexts_at,
};
use audiveris_omr::clef_parameters::{
    SheetClefParameters, StaffClefParameters, max_eval_rank, max_part_count,
};
use audiveris_omr::headers_step::{HeadlessHeaderStaff, HeadlessHeaderSystem};
use audiveris_omr::recognize::{GridLinesRecognition, StaffLineGeometry, recognize_grid_lines};
use audiveris_omr::staff_header::{HeaderBounds, StaffHeader};

/// Java `Grades.intrinsicRatio`.
const INTRINSIC_RATIO: f64 = 0.8;

/// Java `Grades.clefMinGrade / Grades.intrinsicRatio`, the classifier's minimum.
const MINIMUM_CLASSIFIER_GRADE: f64 = 0.03 / 0.8;

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}

/// One `staff` row of the oracle.
#[derive(Debug, Clone, PartialEq)]
struct OracleStaff {
    id: usize,
    specific_interline: i32,
    header_start: i32,
    clef_stop: Option<i32>,
    shape: String,
    symbol: (i32, i32, i32, i32),
}

fn parse_oracle() -> Vec<(String, i32, Vec<OracleStaff>)> {
    let text = std::fs::read_to_string(repo_path("rust/oracle/clef-headers.txt"))
        .expect("the clef oracle is checked in");
    let mut pages: Vec<(String, i32, Vec<OracleStaff>)> = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        match fields.first().copied() {
            Some("page") => pages.push((
                fields[1].to_owned(),
                fields[2].parse().expect("sheet interline"),
                Vec::new(),
            )),
            Some("staff") => {
                let page = pages.last_mut().expect("a staff row follows a page row");
                page.2.push(OracleStaff {
                    id: fields[1].parse().expect("staff id"),
                    specific_interline: fields[2].parse().expect("specific interline"),
                    header_start: fields[3].parse().expect("header start"),
                    clef_stop: fields[5].parse().ok(),
                    shape: fields[6].to_owned(),
                    symbol: (
                        fields[8].parse().expect("symbol x"),
                        fields[9].parse().expect("symbol y"),
                        fields[10].parse().expect("symbol w"),
                        fields[11].parse().expect("symbol h"),
                    ),
                });
            }
            _ => {}
        }
    }
    pages
}

/// Staff-line ordinates backed by the splines GRID published.
struct GridOrdinates<'a>(&'a [StaffLineGeometry]);

impl StaffLineOrdinates for GridOrdinates<'_> {
    fn ordinates_at(&self, staff_id: usize, x: i32) -> Option<(i32, i32)> {
        let staff = self.0.iter().find(|staff| staff.staff_id == staff_id)?;
        Some((staff.first_line_y_at(x)?, staff.last_line_y_at(x)?))
    }
}

/// Java `Staff.getClefStop()`: the inclusive right edge of the header clef's own bounds.
///
/// This is not the value `setClefStop` stored, and the difference matters. `registerClefs`
/// computes an end from `glyph.getBounds().intersection(clefBox)` and stores it on the clef
/// *range*; but `getClefStop` never reads that once a header clef exists — it recomputes from
/// `header.clef.getBounds()` and ignores the glyph entirely. The stored value survives only as a
/// fallback for a staff whose clef was never selected.
///
/// Getting this wrong is quiet rather than loud: the intersection form agrees with the box form
/// whenever the glyph is at least as wide as its symbol, which was 56 of these 65 staves. The
/// nine that disagreed were all bass clefs, whose ink is narrower than the F_CLEF symbol.
fn clef_stop(symbol: HeaderBounds) -> i32 {
    symbol.x + symbol.width - 1
}

#[test]
fn native_clefs_match_java_on_every_corpus_staff() {
    let pages = parse_oracle();
    assert_eq!(pages.len(), 9, "the oracle covers nine pages");

    let mut checked = 0;
    let mut mismatches: Vec<String> = Vec::new();

    for (name, sheet_interline, oracle_staves) in &pages {
        let recognition: GridLinesRecognition =
            recognize_grid_lines(repo_path(&format!("data/examples/{name}")))
                .unwrap_or_else(|error| panic!("{name}: GRID failed: {error}"));

        assert_eq!(
            recognition.scale.scale.interline.main, *sheet_interline,
            "{name}: sheet interline disagrees before any clef work"
        );

        let sheet = SheetClefParameters::new(*sheet_interline);
        let lookup_parameters = ClefLookupParameters {
            sheet_height: i32::try_from(recognition.scale.height).unwrap_or(i32::MAX),
            // `aboveStaff`/`belowStaff` are staff-scaled; on this corpus every staff shares the
            // sheet interline, which the per-staff assertion below re-checks rather than assumes.
            above_staff: StaffClefParameters::new(*sheet_interline).above_staff,
            below_staff: StaffClefParameters::new(*sheet_interline).below_staff,
            belt_margin: sheet.belt_margin,
            x_core_margin: sheet.x_core_margin,
            y_core_margin: sheet.y_core_margin,
        };

        let geometries: Vec<ClefLookupStaffGeometry> = oracle_staves
            .iter()
            .filter_map(|oracle| {
                let lines = recognition
                    .staff_lines
                    .iter()
                    .find(|lines| lines.staff_id == oracle.id)?;
                let browse_start = oracle.header_start;
                let browse_stop = browse_start + sheet.max_clef_end;
                let x_mid = (browse_start + browse_stop) / 2;
                Some(ClefLookupStaffGeometry {
                    staff_id: oracle.id,
                    browse_start,
                    browse_stop,
                    left_abscissa: lines.left,
                    right_abscissa: lines.right,
                    first_line_y: lines.first_line_y_at(x_mid).unwrap_or_default(),
                    last_line_y: lines.last_line_y_at(x_mid).unwrap_or_default(),
                    percussion_only: false,
                })
            })
            .collect();
        assert_eq!(
            geometries.len(),
            oracle_staves.len(),
            "{name}: GRID produced fewer staves than Java's HEADERS reported"
        );

        let contexts = build_clef_lookup_contexts_at(
            &geometries,
            lookup_parameters,
            &GridOrdinates(&recognition.staff_lines),
        );

        let mut parameters = BTreeMap::new();
        for oracle in oracle_staves {
            let staff = StaffClefParameters::new(oracle.specific_interline);
            let lines = recognition
                .staff_lines
                .iter()
                .find(|lines| lines.staff_id == oracle.id)
                .expect("checked above");
            assert_eq!(
                lines.interline, oracle.specific_interline,
                "{name} staff {}: specific interline disagrees",
                oracle.id
            );
            let browse_start = oracle.header_start;
            let x_mid = (browse_start + browse_start + sheet.max_clef_end) / 2;
            parameters.insert(
                oracle.id,
                NativeClefParameters {
                    staff_interline: oracle.specific_interline,
                    first_line_y: f64::from(lines.first_line_y_at(x_mid).unwrap_or_default()),
                    last_line_y: f64::from(lines.last_line_y_at(x_mid).unwrap_or_default()),
                    min_part_weight: usize::try_from(staff.min_part_weight).unwrap_or(0),
                    max_part_count: max_part_count(),
                    max_part_gap: staff.max_part_gap,
                    max_glyph_height: staff.max_glyph_height,
                    min_glyph_weight: usize::try_from(staff.min_glyph_weight).unwrap_or(0),
                    max_eval_rank: max_eval_rank(),
                    minimum_classifier_grade: MINIMUM_CLASSIFIER_GRADE,
                    f_area_pitch_offset: 0.0,
                },
            );
        }

        // One NO_STAFF source per system; `classify_clefs` crops it per lookup rectangle in
        // absolute coordinates, so every system reads the same sheet raster.
        let system_count = recognition.system_bounds.len().max(1);
        let mut sources = BTreeMap::new();
        for system_id in 1..=system_count {
            sources.insert(system_id, recognition.no_staff.clone());
        }

        let classifier = BundledClefClassifier::bundled().expect("bundled classifier");
        let visual = NativeClefProposalRecognizer::new(
            classifier,
            sources,
            contexts.clone(),
            parameters,
            1,
            1,
        );
        let lifecycle = ClefLifecycleRecognizer::new(visual, contexts, INTRINSIC_RATIO, 0.0);
        let mut column = HeadlessClefColumn::new(lifecycle);

        let staffs: Vec<HeadlessHeaderStaff> = oracle_staves
            .iter()
            .map(|oracle| {
                let mut staff = HeadlessHeaderStaff::new(oracle.id);
                staff.maximum_clef_end = sheet.max_clef_end;
                // Java's header start, supplied rather than computed. See the module comment.
                staff.header = Some(StaffHeader::new(oracle.header_start));
                staff
            })
            .collect();
        let mut system = HeadlessHeaderSystem::new(1, staffs);

        if let Err(error) = column.retrieve_clefs(&mut system) {
            mismatches.push(format!("{name}: retrieve_clefs failed: {error:?}"));
            continue;
        }

        for oracle in oracle_staves {
            checked += 1;
            let produced = column
                .builders()
                .get(&oracle.id)
                .and_then(|builder| {
                    builder
                        .candidates
                        .iter()
                        .max_by(|one, two| one.grade.total_cmp(&two.grade))
                })
                .map(|candidate| {
                    (
                        format!("{:?}", candidate.kind),
                        candidate.bounds,
                        candidate.glyph_bounds,
                    )
                });
            match produced {
                None => mismatches.push(format!(
                    "{name} staff {}: no clef, Java found {}",
                    oracle.id, oracle.shape
                )),
                Some((kind, bounds, glyph_bounds)) => {
                    let symbol = (bounds.x, bounds.y, bounds.width, bounds.height);
                    if symbol != oracle.symbol {
                        mismatches.push(format!(
                            "{name} staff {}: {kind} box {symbol:?}, Java {} box {:?}",
                            oracle.id, oracle.shape, oracle.symbol
                        ));
                    }
                    let stop = clef_stop(bounds);
                    if Some(stop) != oracle.clef_stop {
                        mismatches.push(format!(
                            "{name} staff {}: clefStop {stop}, Java {:?}",
                            oracle.id, oracle.clef_stop
                        ));
                    }
                    let _ = glyph_bounds;
                }
            }
        }
    }

    assert_eq!(checked, 65, "every oracle staff was compared");
    assert!(
        mismatches.is_empty(),
        "{} of {checked} staves disagree with Java:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}
