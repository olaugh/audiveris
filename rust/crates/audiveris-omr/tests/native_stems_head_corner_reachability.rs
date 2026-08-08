// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact differential gate for head-corner reachability.
//!
//! The frozen Java boundary first performs beam-origin reachability, retaining
//! its append-only B-linker arenas, and then visits every live head/corner in
//! stable source order.  It stops immediately before constructing a C-linker
//! `StemBuilder`.  Dense rejected scans are represented by count plus a SHA-256
//! over an explicit semantic stream; the native projector must reconstruct
//! those streams from typed branch evidence rather than trusting stored hashes.
//!
//! The gate freezes and cross-checks the oracle contract, reconstructs every
//! semantic row from the live native pipeline, and compares all eight pages
//! byte-for-byte. Missing identities or decision evidence fail closed.

use std::{collections::BTreeMap, path::PathBuf};

use audiveris_image::section::Bounds;
use audiveris_omr::{
    head_scanner_slices::JavaRectangle,
    head_template::HeadTemplateShape,
    native_headers::recognize_native_headers,
    native_heads::{NativeHeadsRecognition, recognize_native_heads},
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::{NativeStemSeedRecognition, recognize_native_stem_seeds},
    native_stems_beam_reachability::{
        NativeStemsBeamReachabilityRecognition, materialize_native_stems_beam_reachability,
    },
    native_stems_beam_stumps::{
        NativeStemsBeamSource, NativeStemsBeamStumpRecognition, NativeStemsBeamStumpSystem,
        materialize_native_stems_beam_stumps,
    },
    native_stems_beam_vlinkers::{
        NativeStemsBeamBLinkerRef, NativeStemsBeamVLinkerRecognition,
        materialize_native_stems_beam_vlinkers,
    },
    native_stems_head_corner_reachability::{
        NativeStemsHeadBeamAction, NativeStemsHeadBeamArena, NativeStemsHeadBeamArenaOrigin,
        NativeStemsHeadBeamScanAction, NativeStemsHeadCornerReachabilityRecognition,
        NativeStemsHeadCornerReachabilitySystem, NativeStemsHeadCornerRef, NativeStemsHeadDuration,
        NativeStemsHeadFindLinker, NativeStemsHeadFindResult, NativeStemsHeadLookupGeometry,
        NativeStemsHeadReachabilityCorner, NativeStemsHeadReachabilityTarget,
        NativeStemsHeadScanAction, NativeStemsHeadSeedOverlapAction, NativeStemsHeadSeedScanAction,
        materialize_native_stems_head_corner_reachability,
    },
    native_stems_head_corners::{
        NativeStemsHeadCornerRecognition, materialize_native_stems_head_corners,
    },
    native_stems_head_seeds::{
        NativeStemsHeadSeedRecognition, materialize_native_stems_head_seeds,
    },
    native_stems_head_stumps::{
        NativeStemsHeadStumpRecognition, materialize_native_stems_head_stumps,
    },
    recognize::{
        GridLinesRecognition, NativeBeamRecognition, recognize_grid_lines,
        recognize_native_beams_with_stem_seeds,
    },
    stems_step::{NativeStemHeadSide, NativeStemLine, NativeStemPoint, NativeStemVerticalSide},
};

const PAGES: &[&str] = &[
    "chula.png",
    "allegretto.png",
    "batuque.png",
    "carmen.png",
    "cucaracha.png",
    "hove.png",
    "zizi.png",
    "BachInvention5.jpg",
];

const ORACLE_PATH: &str = "rust/oracle/stems-head-corner-reachability.txt";
const PROBE_PATH: &str = "rust/oracle/java/StemsHeadCornerReachabilityProbe.java";
const RUNNER_PATH: &str = "rust/oracle/java/run-stems-head-corner-reachability.sh";

const EXPECTED_FIXTURE_SHA256: &str =
    "537cae86c19de20af35a246e03b6edd7f324d0f08c5768b319ed0557a7e28921";
const EXPECTED_PROBE_SHA256: &str =
    "7bac85a2e878d67ccecab9866428a8068b83d1453c2249f49b0c18ae6a17b39f";
const EXPECTED_RUNNER_SHA256: &str =
    "e9016abb44a500e242b81364531b775fe6b724cddf697cfc0bd4cfe21af0f75d";
const EXPECTED_BODY_SHA256: &str =
    "b3f10b53346adac1309d12fa2d245840a88b02c17e399e88d7e5e36f0358889b";
const EXPECTED_FIXTURE_BYTES: usize = 37_478_914;
const EXPECTED_FIXTURE_LINES: usize = 79_216;

const EXPECTED_CORPUS_FIELDS: &str = concat!(
    "systems 30 heads 3521 corners 14084 seedsScanned 36736 seedsKept 1340 ",
    "headScans 1007081 headTargets 4566 siblingScans 9015 beamTargets 8120 ",
    "anchorsCreated 1687 cSeedWrites 14084 builderChecks 16501",
);

const EXPECTED_PREFIX_COUNTS: &[(&str, usize)] = &[
    ("stemsheadreachpage", 8),
    ("stemsheadreachsystem", 30),
    ("stemsheadreachbarena", 60),
    ("stemsheadreachhead", 3_521),
    ("stemsheadreachcorner", 14_084),
    ("stemsheadreachseedscan", 14_084),
    ("stemsheadreachseed", 1_340),
    ("stemsheadreachheadscan", 14_084),
    ("stemsheadreachheadtarget", 4_566),
    ("stemsheadreachsiblings", 5_180),
    ("stemsheadreachbeamtarget", 8_120),
    ("stemsheadreachresult", 14_084),
    ("stemsheadreachsystemsummary", 30),
    ("stemsheadreachpagesummary", 8),
    ("stemsheadreachcorpussummary", 1),
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Totals {
    systems: usize,
    heads: usize,
    corners: usize,
    seeds_scanned: usize,
    seeds_kept: usize,
    head_scans: usize,
    head_targets: usize,
    sibling_scans: usize,
    beam_targets: usize,
    anchors_created: usize,
    c_seed_writes: usize,
    builder_checks: usize,
}

#[allow(dead_code)]
struct NativePage {
    grid: GridLinesRecognition,
    stem_seeds: NativeStemSeedRecognition,
    beams: NativeBeamRecognition,
    heads: NativeHeadsRecognition,
    corners: NativeStemsHeadCornerRecognition,
    head_seeds: NativeStemsHeadSeedRecognition,
    beam_stumps: NativeStemsBeamStumpRecognition,
    beam_vlinkers: NativeStemsBeamVLinkerRecognition,
    beam_reachability: NativeStemsBeamReachabilityRecognition,
    head_stumps: NativeStemsHeadStumpRecognition,
    reachability: NativeStemsHeadCornerReachabilityRecognition,
}

impl Totals {
    fn from_summary(row: &str) -> Self {
        Self {
            systems: optional_usize(row, "systems"),
            heads: required_usize(row, "heads"),
            corners: required_usize(row, "corners"),
            seeds_scanned: required_usize(row, "seedsScanned"),
            seeds_kept: required_usize(row, "seedsKept"),
            head_scans: required_usize(row, "headScans"),
            head_targets: required_usize(row, "headTargets"),
            sibling_scans: required_usize(row, "siblingScans"),
            beam_targets: required_usize(row, "beamTargets"),
            anchors_created: required_usize(row, "anchorsCreated"),
            c_seed_writes: required_usize(row, "cSeedWrites"),
            builder_checks: required_usize(row, "builderChecks"),
        }
    }

    fn include(&mut self, other: Self) {
        self.systems += other.systems;
        self.heads += other.heads;
        self.corners += other.corners;
        self.seeds_scanned += other.seeds_scanned;
        self.seeds_kept += other.seeds_kept;
        self.head_scans += other.head_scans;
        self.head_targets += other.head_targets;
        self.sibling_scans += other.sibling_scans;
        self.beam_targets += other.beam_targets;
        self.anchors_created += other.anchors_created;
        self.c_seed_writes += other.c_seed_writes;
        self.builder_checks += other.builder_checks;
    }
}

#[derive(Clone, Copy)]
struct RowHasher(u64);

impl Default for RowHasher {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl RowHasher {
    fn add(&mut self, row: &str) {
        for byte in row.bytes().chain([b'\n']) {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

#[test]
fn native_head_corner_reachability_matches_java_corpus_exactly() {
    let oracle =
        std::fs::read_to_string(repo_path(ORACLE_PATH)).expect("head reachability fixture");
    let mut corpus = Totals::default();
    for image in PAGES {
        let page = format!("{image}#1");
        let native = native_page(image);
        let (actual, totals) = native_page_rows(&page, &native);
        let expected = oracle_page_rows(&oracle, &page);
        if actual != expected {
            report_first_mismatches(&page, &actual, &expected);
            panic!("{page}: native head-corner reachability differs from Java");
        }
        corpus.include(totals);
    }
    assert_eq!(
        corpus,
        Totals::from_summary(
            oracle
                .lines()
                .find(|row| row.starts_with("stemsheadreachcorpussummary "))
                .expect("corpus summary"),
        )
    );
}

fn native_page(image: &str) -> NativePage {
    let grid = recognize_grid_lines(repo_path(&format!("data/examples/{image}")))
        .unwrap_or_else(|error| panic!("{image}: GRID failed: {error}"));
    let headers = recognize_native_headers(&grid)
        .unwrap_or_else(|error| panic!("{image}: HEADERS failed: {error}"));
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers)
        .unwrap_or_else(|error| panic!("{image}: STEM_SEEDS failed: {error}"));
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .unwrap_or_else(|error| panic!("{image}: BEAMS failed: {error}"));
    let ledgers = recognize_native_ledgers(&grid, &beams)
        .unwrap_or_else(|error| panic!("{image}: LEDGERS failed: {error}"));
    let heads = recognize_native_heads(&grid, &headers, &stem_seeds, &beams, &ledgers)
        .unwrap_or_else(|error| panic!("{image}: HEADS failed: {error}"));
    let corners = materialize_native_stems_head_corners(&heads, &stem_seeds)
        .unwrap_or_else(|error| panic!("{image}: STEMS corners failed: {error}"));
    let head_seeds = materialize_native_stems_head_seeds(&grid, &stem_seeds, &corners)
        .unwrap_or_else(|error| panic!("{image}: STEMS head seeds failed: {error}"));
    let beam_stumps =
        materialize_native_stems_beam_stumps(&grid, &beams, &heads, &stem_seeds, &head_seeds)
            .unwrap_or_else(|error| panic!("{image}: STEMS beam stumps failed: {error}"));
    let beam_vlinkers =
        materialize_native_stems_beam_vlinkers(&grid, &beams, &stem_seeds, &beam_stumps)
            .unwrap_or_else(|error| panic!("{image}: STEMS beam VLinkers failed: {error}"));
    let beam_reachability =
        materialize_native_stems_beam_reachability(&beams, &beam_stumps, &beam_vlinkers, &corners)
            .unwrap_or_else(|error| panic!("{image}: STEMS beam reachability failed: {error}"));
    let head_stumps =
        materialize_native_stems_head_stumps(&grid, &stem_seeds, &corners, &head_seeds)
            .unwrap_or_else(|error| panic!("{image}: STEMS head stumps failed: {error}"));
    let reachability = materialize_native_stems_head_corner_reachability(
        &grid,
        &stem_seeds,
        &heads,
        &corners,
        &head_seeds,
        &head_stumps,
        &beam_stumps,
        &beam_vlinkers,
        &beam_reachability,
    )
    .unwrap_or_else(|error| panic!("{image}: STEMS head reachability failed: {error}"));
    NativePage {
        grid,
        stem_seeds,
        beams,
        heads,
        corners,
        head_seeds,
        beam_stumps,
        beam_vlinkers,
        beam_reachability,
        head_stumps,
        reachability,
    }
}

fn native_page_rows(page: &str, native: &NativePage) -> (Vec<String>, Totals) {
    let mut rows = vec![format!(
        "stemsheadreachpage {page} systems {} staves {} family Bravura",
        native.reachability.systems.len(),
        native.grid.staves.len(),
    )];
    let mut page_hash = RowHasher::default();
    let mut page_totals = Totals::default();
    for system in &native.reachability.systems {
        let stumps = system_by_id(&native.beam_stumps.systems, system.system_id);
        let beam_reachability = native
            .beam_reachability
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system.system_id)
            .expect("beam reachability system");
        let mut system_rows = Vec::new();
        let totals = append_system_rows(page, system, stumps, beam_reachability, &mut system_rows);
        let mut system_hash = RowHasher::default();
        for row in &system_rows {
            system_hash.add(row);
            page_hash.add(row);
        }
        let summary = summary_row(
            "stemsheadreachsystemsummary",
            page,
            Some(system.system_id),
            1,
            totals,
            system_hash.0,
        );
        page_hash.add(&summary);
        system_rows.push(summary);
        rows.extend(system_rows);
        page_totals.include(totals);
    }
    rows.push(summary_row(
        "stemsheadreachpagesummary",
        page,
        None,
        native.reachability.systems.len(),
        page_totals,
        page_hash.0,
    ));
    assert_eq!(native.reachability.head_count, page_totals.heads);
    assert_eq!(native.reachability.corner_count, page_totals.corners);
    assert_eq!(
        native.reachability.seed_scan_count,
        page_totals.seeds_scanned
    );
    assert_eq!(native.reachability.kept_seed_count, page_totals.seeds_kept);
    assert_eq!(native.reachability.head_scan_count, page_totals.head_scans);
    assert_eq!(
        native.reachability.head_target_count,
        page_totals.head_targets
    );
    assert_eq!(
        native.reachability.sibling_scan_count,
        page_totals.sibling_scans
    );
    assert_eq!(
        native.reachability.beam_target_count,
        page_totals.beam_targets
    );
    assert_eq!(
        native.reachability.created_anchor_count,
        page_totals.anchors_created
    );
    assert_eq!(
        native.reachability.c_seed_assignment_count,
        page_totals.c_seed_writes
    );
    assert_eq!(
        native
            .reachability
            .page_persistent_registration_mutation_count,
        0
    );
    (rows, page_totals)
}

fn append_system_rows(
    page: &str,
    system: &NativeStemsHeadCornerReachabilitySystem,
    stumps: &NativeStemsBeamStumpSystem,
    beam_reachability: &audiveris_omr::native_stems_beam_reachability::NativeStemsBeamReachabilitySystem,
    rows: &mut Vec<String>,
) -> Totals {
    let initial_bs = system
        .after_beam_arenas
        .iter()
        .map(|arena| arena.initial_b_count)
        .sum::<usize>();
    rows.push(format!(
        "stemsheadreachsystem {page} system {} profile {} interline {} bounds {} \
         beamSigOrder {} beamXOrder {} headSigOrder {} headXOrder {} keptSeeds {} initialBs {} \
         maxBeamLinkerDx {} maxBeamSideDx {} minBeamHeadDy {} minHeadHeadDy {} \
         minSeedContrib {} maxLineSeedDx {} maxHeadInDx {} maxHeadOutDx {} slopeMargin {}",
        system.system_id,
        system.profile,
        system.interline,
        rectangle(system.system_bounds),
        usize_list(&(0..system.beam_sources_in_sig_order.len()).collect::<Vec<_>>()),
        usize_list(
            &system
                .beam_sources_by_abscissa
                .iter()
                .map(|&source| beam_sig_ordinal(stumps, source))
                .collect::<Vec<_>>(),
        ),
        usize_list(&system.head_sig_ordinals),
        usize_list(&system.head_sig_ordinals_by_abscissa),
        system.kept_seed_ordinals.len(),
        initial_bs,
        system.max_beam_linker_dx,
        system.max_beam_side_dx,
        system.min_beam_head_dy,
        system.min_head_head_dy,
        system.min_seed_contribution,
        java_hex_double(system.max_line_seed_dx),
        system.max_head_in_dx,
        system.max_head_out_dx,
        java_hex_double(system.slope_margin),
    ));
    rows.push(arena_row(
        page,
        system.system_id,
        "afterBeamPrefix",
        system,
        stumps,
    ));

    let mut totals = Totals {
        systems: 1,
        heads: system.heads.len(),
        ..Totals::default()
    };
    for head in &system.heads {
        rows.push(format!(
            "stemsheadreachhead {page} system {} xOrdinal {} sigOrdinal {} staff {} shape {} \
             bounds {} center {}:{} grade {} duration {}",
            system.system_id,
            head.x_ordinal,
            head.sig_ordinal,
            head.staff_id,
            head_shape(head.shape),
            rectangle(head.bounds),
            head.center.0,
            head.center.1,
            java_hex_double(head.grade),
            duration(head.duration),
        ));
        for corner in &head.corners {
            append_corner_rows(page, system, stumps, corner, rows, &mut totals);
        }
    }
    rows.push(arena_row(page, system.system_id, "final", system, stumps));
    totals.c_seed_writes = system.c_seed_assignment_count;
    totals.builder_checks = beam_reachability
        .beam_inspections
        .iter()
        .flat_map(|beam| &beam.b_visits)
        .map(|visit| visit.v_inspections.len())
        .sum::<usize>()
        + totals.corners;
    assert_eq!(system.c_builder_assignment_count, 0);
    assert_eq!(system.forbidden_sig_mutation_count, 0);
    assert_eq!(system.forbidden_link_state_mutation_count, 0);
    totals
}

fn append_corner_rows(
    page: &str,
    system: &NativeStemsHeadCornerReachabilitySystem,
    stumps: &NativeStemsBeamStumpSystem,
    corner: &NativeStemsHeadReachabilityCorner,
    rows: &mut Vec<String>,
    totals: &mut Totals,
) {
    let beam_rejects = beam_reject_stream(corner);
    let target_rejects = target_reject_stream(corner);
    rows.push(format!(
        "stemsheadreachcorner {page} system {} head {} inspectOrdinal {} corner {} alias {} \
         hSide {} vSide {} xDir {} yDir {} ref {} out {} in {} stump {} partLimit {} \
         initialYLimit {} initialQuad {} beamScans {} groups {} beamRejects {} beamRejectSha {} \
         targetScans {} targetRejects {} targetRejectSha {} targetBeam {} target {} \
         finalYLimit {} finalQuad {} lookupBounds {} lookupBounds2d {} theo {} yRange {}",
        system.system_id,
        corner.reference.x_ordinal,
        corner.inspection_ordinal,
        compact_corner(corner.reference),
        c_alias(corner.reference),
        head_side(corner.reference.horizontal),
        vertical_side(corner.reference.vertical),
        corner.x_direction,
        corner.y_direction,
        point(corner.reference_point),
        point(corner.out_point),
        point(corner.in_point),
        corner.stump.map_or_else(
            || "-".to_owned(),
            |stump| format!("{}:{}", bounds(stump.bounds), stump.weight)
        ),
        java_hex_double(corner.part_y_limit),
        java_hex_double(corner.initial_lookup.y_limit),
        quadrilateral(corner.initial_lookup),
        corner.beam_scans.len(),
        beam_groups(system, stumps, &corner.beam_groups),
        beam_rejects.len(),
        semantic_sha(&beam_rejects),
        corner.target_beam_scans.len(),
        target_rejects.len(),
        semantic_sha(&target_rejects),
        corner.target_beam.map_or_else(
            || "-".to_owned(),
            |source| beam_sig_ordinal(stumps, source).to_string()
        ),
        point(corner.target_point),
        java_hex_double(corner.final_lookup.y_limit),
        quadrilateral(corner.final_lookup),
        rectangle(corner.final_lookup.bounds),
        double_bounds(corner.final_lookup),
        line(corner.theoretical_line),
        rectangle(corner.y_range),
    ));

    let seed_rejects = seed_reject_stream(system, corner);
    let preliminary = corner
        .seed_scans
        .iter()
        .filter(|scan| scan.action == NativeStemsHeadSeedScanAction::Preliminary)
        .count();
    rows.push(format!(
        "stemsheadreachseedscan {page} system {} head {} inspectOrdinal {} neighborSeeds {} \
         scans {} prelim {} kept {} rejects {} rejectSha {}",
        system.system_id,
        corner.reference.x_ordinal,
        corner.inspection_ordinal,
        corner.seed_scans.len(),
        corner.seed_scans.len(),
        preliminary,
        corner.assigned_seed_ordinals.len(),
        seed_rejects.len(),
        semantic_sha(&seed_rejects),
    ));
    for (ordinal, &free_glyph_ordinal) in corner.assigned_seed_ordinals.iter().enumerate() {
        let scan = corner
            .seed_scans
            .iter()
            .find(|scan| scan.free_glyph_ordinal == free_glyph_ordinal)
            .expect("assigned seed scan");
        rows.push(format!(
            "stemsheadreachseed {page} system {} head {} inspectOrdinal {} ordinal {} keptSeed {} \
             bounds {} centroid {}:{} contrib {} distance {}",
            system.system_id,
            corner.reference.x_ordinal,
            corner.inspection_ordinal,
            ordinal,
            seed_ordinal(system, free_glyph_ordinal),
            bounds(scan.bounds),
            scan.centroid.0,
            scan.centroid.1,
            scan.contribution.expect("assigned seed contribution"),
            java_hex_double(scan.line_distance.expect("assigned seed distance")),
        ));
    }

    let head_rejects = head_reject_stream(corner);
    rows.push(format!(
        "stemsheadreachheadscan {page} system {} head {} inspectOrdinal {} scans {} candidates {} \
         targets {} rejects {} rejectSha {}",
        system.system_id,
        corner.reference.x_ordinal,
        corner.inspection_ordinal,
        corner.head_scans.len(),
        corner
            .head_scans
            .iter()
            .filter(|scan| scan.candidate_after_area)
            .count(),
        corner.head_targets.len(),
        head_rejects.len(),
        semantic_sha(&head_rejects),
    ));
    for (ordinal, target) in corner.head_targets.iter().enumerate() {
        rows.push(format!(
            "stemsheadreachheadtarget {page} system {} head {} inspectOrdinal {} ordinal {} target {}",
            system.system_id,
            corner.reference.x_ordinal,
            corner.inspection_ordinal,
            ordinal,
            c_alias(*target),
        ));
    }

    if corner.beam_action == NativeStemsHeadBeamAction::Inspected {
        let target_beam = corner.target_beam.expect("inspected target beam");
        let sibling_rejects = sibling_reject_stream(stumps, corner);
        rows.push(format!(
            "stemsheadreachsiblings {page} system {} head {} inspectOrdinal {} targetBeam {} \
             cross {} scans {} accepted {} rejects {} rejectSha {}",
            system.system_id,
            corner.reference.x_ordinal,
            corner.inspection_ordinal,
            beam_sig_ordinal(stumps, target_beam),
            point(corner.sibling_lookup_cross.expect("sibling lookup cross")),
            corner.sibling_scans.len(),
            corner
                .sibling_scans
                .iter()
                .filter(|scan| scan.accepted)
                .count(),
            sibling_rejects.len(),
            semantic_sha(&sibling_rejects),
        ));
        for find in &corner.find_linkers {
            append_find_row(page, system.system_id, stumps, corner, find, rows);
        }
    }

    let seed_tokens = corner
        .assigned_seed_ordinals
        .iter()
        .map(|&ordinal| seed_ordinal(system, ordinal).to_string())
        .collect::<Vec<_>>();
    let head_tokens = corner
        .head_targets
        .iter()
        .copied()
        .map(c_alias)
        .collect::<Vec<_>>();
    let beam_tokens = corner
        .beam_targets
        .iter()
        .map(|&reference| b_alias(stumps, reference))
        .collect::<Vec<_>>();
    let target_tokens = corner
        .ordered_targets
        .iter()
        .map(|target| match *target {
            NativeStemsHeadReachabilityTarget::Head(reference) => c_alias(reference),
            NativeStemsHeadReachabilityTarget::Beam(reference) => b_alias(stumps, reference),
        })
        .collect::<Vec<_>>();
    rows.push(format!(
        "stemsheadreachresult {page} system {} head {} inspectOrdinal {} corner {} alias {} \
         seeds {} headTargets {} beamAction {} beamTargets {} targets {} builder null",
        system.system_id,
        corner.reference.x_ordinal,
        corner.inspection_ordinal,
        result_corner(corner.reference),
        c_alias(corner.reference),
        string_list(&seed_tokens),
        string_list(&head_tokens),
        beam_action(corner.beam_action),
        string_list(&beam_tokens),
        string_list(&target_tokens),
    ));
    assert!(corner.c_seeds_assigned);
    assert!(!corner.c_builder_assigned);

    totals.corners += 1;
    totals.seeds_scanned += corner.seed_scans.len();
    totals.seeds_kept += corner.assigned_seed_ordinals.len();
    totals.head_scans += corner.head_scans.len();
    totals.head_targets += corner.head_targets.len();
    totals.sibling_scans += corner.sibling_scans.len();
    totals.beam_targets += corner.beam_targets.len();
    totals.anchors_created += corner
        .find_linkers
        .iter()
        .filter(|find| matches!(find.result, NativeStemsHeadFindResult::CreatedAnchor(_)))
        .count();
}

fn append_find_row(
    page: &str,
    system_id: usize,
    stumps: &NativeStemsBeamStumpSystem,
    corner: &NativeStemsHeadReachabilityCorner,
    find: &NativeStemsHeadFindLinker,
    rows: &mut Vec<String>,
) {
    let candidates = find
        .candidates
        .iter()
        .map(|candidate| {
            format!(
                "{}:{}:{}:{}",
                candidate.insertion_ordinal,
                point(candidate.reference_point),
                java_hex_double(candidate.dx),
                if candidate.replaces_best {
                    "replace"
                } else {
                    "keep"
                },
            )
        })
        .collect::<Vec<_>>();
    rows.push(format!(
        "stemsheadreachbeamtarget {page} system {system_id} head {} inspectOrdinal {} ordinal {} \
         findOrdinal {} beamSig {} before {} cross {} candidateScans {} candidateSha {} best {} \
         bestDx {} action {} result {} after {}",
        corner.reference.x_ordinal,
        corner.inspection_ordinal,
        find.sibling_ordinal,
        find.find_ordinal,
        beam_sig_ordinal(stumps, find.target_beam),
        find.arena_before.len(),
        point(find.cross),
        find.candidates.len(),
        semantic_sha(&candidates),
        find.selected_before_threshold
            .map_or_else(|| "-".to_owned(), |reference| b_alias(stumps, reference)),
        java_hex_double(find.best_dx),
        if matches!(find.result, NativeStemsHeadFindResult::Reused(_)) {
            "reuse"
        } else {
            "createAnchor"
        },
        b_alias(stumps, find.result.reference()),
        find.arena_after.len(),
    ));
}

fn beam_reject_stream(corner: &NativeStemsHeadReachabilityCorner) -> Vec<String> {
    let mut values = Vec::new();
    for scan in &corner.beam_scans {
        match scan.action {
            NativeStemsHeadBeamScanAction::Removed => {
                values.push(format!("removed:{}", scan.neighbor_ordinal));
            }
            NativeStemsHeadBeamScanAction::AcceptedArea => {}
            NativeStemsHeadBeamScanAction::Outside => values.push(format!(
                "outside:{}:{}",
                scan.neighbor_ordinal,
                rectangle(scan.bounds)
            )),
            NativeStemsHeadBeamScanAction::BreakX => values.push(format!(
                "breakX:{}:{}",
                scan.neighbor_ordinal,
                rectangle(scan.bounds)
            )),
        }
    }
    for decision in &corner.beam_direction_decisions {
        if !decision.accepted {
            values.push(format!(
                "direction:{}:{}:{}",
                decision.candidate_ordinal,
                point(decision.target),
                java_hex_double(decision.directed_distance),
            ));
        }
    }
    for decision in &corner.beam_group_decisions {
        if decision.near_gate_evaluated && !decision.accepted {
            values.push(format!(
                "near:{}:{}:{}",
                decision.sorted_ordinal,
                point(decision.near_target.expect("near target")),
                java_hex_double(decision.directed_distance.expect("near distance")),
            ));
        }
    }
    values
}

fn target_reject_stream(corner: &NativeStemsHeadReachabilityCorner) -> Vec<String> {
    corner
        .target_beam_scans
        .iter()
        .filter(|scan| !scan.intersects_initial_theoretical_line)
        .map(|scan| {
            format!(
                "noMedianIntersection:{}:{}:{}",
                scan.group_list_ordinal,
                scan.member_ordinal,
                segment(scan.median),
            )
        })
        .collect()
}

fn seed_reject_stream(
    system: &NativeStemsHeadCornerReachabilitySystem,
    corner: &NativeStemsHeadReachabilityCorner,
) -> Vec<String> {
    let mut values = Vec::new();
    for scan in &corner.seed_scans {
        let ordinal = seed_ordinal(system, scan.free_glyph_ordinal);
        match scan.action {
            NativeStemsHeadSeedScanAction::OutsideLookup => {
                values.push(format!("outside:{ordinal}:{}", bounds(scan.bounds)));
            }
            NativeStemsHeadSeedScanAction::OverlapsStump => {
                values.push(format!("stump:{ordinal}:{}", bounds(scan.bounds)));
            }
            NativeStemsHeadSeedScanAction::InsufficientContribution => values.push(format!(
                "contrib:{ordinal}:{}:{}",
                scan.contribution.expect("insufficient contribution"),
                bounds(scan.bounds),
            )),
            NativeStemsHeadSeedScanAction::TooFarFromLine => values.push(format!(
                "distance:{ordinal}:{}",
                java_hex_double(scan.line_distance.expect("line distance")),
            )),
            NativeStemsHeadSeedScanAction::Preliminary => {}
        }
    }
    for decision in &corner.seed_overlap_decisions {
        if decision.action == NativeStemsHeadSeedOverlapAction::RejectedOverlap {
            values.push(format!(
                "overlap:{}:{}",
                seed_ordinal(system, decision.free_glyph_ordinal),
                seed_ordinal(
                    system,
                    decision
                        .first_overlapping_kept_seed
                        .expect("overlapping kept seed"),
                ),
            ));
        }
    }
    values
}

fn head_reject_stream(corner: &NativeStemsHeadReachabilityCorner) -> Vec<String> {
    let mut values = Vec::new();
    for scan in &corner.head_scans {
        match scan.action {
            NativeStemsHeadScanAction::Removed => {
                values.push(format!("removed:{}", scan.x_ordinal));
            }
            NativeStemsHeadScanAction::OutsideLookup => {
                values.push(format!("outside:{}", scan.x_ordinal));
            }
            NativeStemsHeadScanAction::BreakX => {
                values.push(format!("breakX:{}", scan.x_ordinal));
            }
            NativeStemsHeadScanAction::SelfHead => {
                values.push(format!("self:{}", scan.x_ordinal));
            }
            NativeStemsHeadScanAction::CompetingHead => {
                values.push(format!("competing:{}", scan.x_ordinal));
            }
            NativeStemsHeadScanAction::DurationMismatch => values.push(format!(
                "duration:{}:{}",
                scan.x_ordinal,
                duration(scan.duration),
            )),
            NativeStemsHeadScanAction::TooNear => values.push(format!(
                "near:{}:{}",
                scan.x_ordinal,
                java_hex_double(scan.directed_distance.expect("near head distance")),
            )),
            NativeStemsHeadScanAction::CornersChecked => {
                for check in &scan.corner_checks {
                    if !check.contained {
                        values.push(format!(
                            "cornerOutside:{}:{}:{}",
                            scan.x_ordinal,
                            head_side(check.horizontal),
                            point(check.reference_point),
                        ));
                    }
                }
            }
        }
    }
    values
}

fn sibling_reject_stream(
    stumps: &NativeStemsBeamStumpSystem,
    corner: &NativeStemsHeadReachabilityCorner,
) -> Vec<String> {
    corner
        .sibling_scans
        .iter()
        .filter(|scan| !scan.accepted)
        .map(|scan| {
            format!(
                "outside:{}:{}",
                beam_sig_ordinal(stumps, scan.beam),
                point(scan.cross),
            )
        })
        .collect()
}

fn semantic_sha(values: &[String]) -> String {
    let mut bytes = Vec::new();
    for value in values {
        bytes.extend_from_slice(value.as_bytes());
        bytes.push(b'\n');
    }
    sha256_hex(&bytes)
}

fn arena_row(
    page: &str,
    system_id: usize,
    phase: &str,
    system: &NativeStemsHeadCornerReachabilitySystem,
    stumps: &NativeStemsBeamStumpSystem,
) -> String {
    let (arenas, count_field, added) = if phase == "afterBeamPrefix" {
        let total = system
            .after_beam_arenas
            .iter()
            .map(|arena| arena.entries.len())
            .sum::<usize>();
        let initial = system
            .after_beam_arenas
            .iter()
            .map(|arena| arena.initial_b_count)
            .sum::<usize>();
        (&system.after_beam_arenas, "added", total - initial)
    } else {
        let initial = system
            .after_beam_arenas
            .iter()
            .map(|arena| arena.entries.len())
            .sum::<usize>();
        let total = system
            .final_beam_arenas
            .iter()
            .map(|arena| arena.entries.len())
            .sum::<usize>();
        (&system.final_beam_arenas, "addedFromHeads", total - initial)
    };
    let total = arenas
        .iter()
        .map(|arena| arena.entries.len())
        .sum::<usize>();
    format!(
        "stemsheadreachbarena {page} system {system_id} phase {phase} total {total} \
         {count_field} {added} arenas {}",
        arena_tokens(arenas, stumps),
    )
}

fn arena_tokens(
    arenas: &[NativeStemsHeadBeamArena],
    stumps: &NativeStemsBeamStumpSystem,
) -> String {
    string_list(
        &arenas
            .iter()
            .map(|arena| {
                let members = arena
                    .entries
                    .iter()
                    .map(|entry| {
                        format!(
                            "{}:{}",
                            b_alias(stumps, entry.reference),
                            match entry.origin {
                                NativeStemsHeadBeamArenaOrigin::Initial => "I",
                                NativeStemsHeadBeamArenaOrigin::BeamReachabilityAnchor(_)
                                | NativeStemsHeadBeamArenaOrigin::HeadReachabilityAnchor {
                                    ..
                                } => {
                                    "A"
                                }
                            },
                        )
                    })
                    .collect::<Vec<_>>();
                format!("{}=[{}]", arena.sig_ordinal, string_list(&members))
            })
            .collect::<Vec<_>>(),
    )
}

fn summary_row(
    prefix: &str,
    page: &str,
    system_id: Option<usize>,
    systems: usize,
    totals: Totals,
    hash: u64,
) -> String {
    let system = system_id.map_or_else(String::new, |id| format!(" system {id}"));
    let systems = system_id.map_or_else(|| format!(" systems {systems}"), |_| String::new());
    format!(
        "{prefix} {page}{system}{systems} heads {} corners {} seedsScanned {} seedsKept {} \
         headScans {} headTargets {} siblingScans {} beamTargets {} anchorsCreated {} \
         cSeedWrites {} builderChecks {} hash {hash:016x}",
        totals.heads,
        totals.corners,
        totals.seeds_scanned,
        totals.seeds_kept,
        totals.head_scans,
        totals.head_targets,
        totals.sibling_scans,
        totals.beam_targets,
        totals.anchors_created,
        totals.c_seed_writes,
        totals.builder_checks,
    )
}

fn system_by_id(
    systems: &[NativeStemsBeamStumpSystem],
    system_id: usize,
) -> &NativeStemsBeamStumpSystem {
    systems
        .iter()
        .find(|system| system.system_id == system_id)
        .expect("beam stump system")
}

fn beam_sig_ordinal(stumps: &NativeStemsBeamStumpSystem, source: NativeStemsBeamSource) -> usize {
    stumps
        .beams_by_abscissa
        .iter()
        .find(|beam| beam.source == source)
        .expect("beam source")
        .sig_ordinal
}

fn b_alias(stumps: &NativeStemsBeamStumpSystem, reference: NativeStemsBeamBLinkerRef) -> String {
    format!(
        "b:{}:{}",
        beam_sig_ordinal(stumps, reference.beam),
        reference.id.checked_sub(1).expect("one-based B id"),
    )
}

fn c_alias(reference: NativeStemsHeadCornerRef) -> String {
    format!(
        "h:{}:{}{}",
        reference.x_ordinal,
        vertical_initial(reference.vertical),
        horizontal_initial(reference.horizontal),
    )
}

fn compact_corner(reference: NativeStemsHeadCornerRef) -> String {
    format!(
        "{}{}",
        vertical_initial(reference.vertical),
        horizontal_initial(reference.horizontal),
    )
}

fn result_corner(reference: NativeStemsHeadCornerRef) -> String {
    format!(
        "{}-{}",
        vertical_initial(reference.vertical),
        horizontal_initial(reference.horizontal),
    )
}

fn horizontal_initial(side: NativeStemHeadSide) -> &'static str {
    match side {
        NativeStemHeadSide::Left => "L",
        NativeStemHeadSide::Right => "R",
    }
}

fn vertical_initial(side: NativeStemVerticalSide) -> &'static str {
    match side {
        NativeStemVerticalSide::Top => "T",
        NativeStemVerticalSide::Bottom => "B",
    }
}

fn head_side(side: NativeStemHeadSide) -> &'static str {
    match side {
        NativeStemHeadSide::Left => "LEFT",
        NativeStemHeadSide::Right => "RIGHT",
    }
}

fn vertical_side(side: NativeStemVerticalSide) -> &'static str {
    match side {
        NativeStemVerticalSide::Top => "TOP",
        NativeStemVerticalSide::Bottom => "BOTTOM",
    }
}

fn head_shape(shape: HeadTemplateShape) -> &'static str {
    match shape {
        HeadTemplateShape::NoteheadBlack => "NOTEHEAD_BLACK",
        HeadTemplateShape::NoteheadVoid => "NOTEHEAD_VOID",
        _ => panic!("unsupported frozen head shape {shape:?}"),
    }
}

fn duration(value: NativeStemsHeadDuration) -> &'static str {
    match value {
        NativeStemsHeadDuration::Quarter => "1/4",
        NativeStemsHeadDuration::Half => "1/2",
    }
}

fn beam_action(value: NativeStemsHeadBeamAction) -> &'static str {
    match value {
        NativeStemsHeadBeamAction::NoTargetBeam => "none",
        NativeStemsHeadBeamAction::VoidSideSkipped => "voidSideSkip",
        NativeStemsHeadBeamAction::Inspected => "inspect",
    }
}

fn rectangle(value: JavaRectangle) -> String {
    format!("{}:{}:{}:{}", value.x, value.y, value.width, value.height)
}

fn bounds(value: Bounds) -> String {
    format!("{}:{}:{}:{}", value.x, value.y, value.width, value.height)
}

fn point(value: NativeStemPoint) -> String {
    format!("{}:{}", java_hex_double(value.x), java_hex_double(value.y))
}

fn line(value: NativeStemLine) -> String {
    format!("{}:{}", point(value.start), point(value.stop))
}

fn segment(value: audiveris_image::beam_structure::Segment) -> String {
    format!(
        "{}:{}:{}:{}",
        java_hex_double(value.x1),
        java_hex_double(value.y1),
        java_hex_double(value.x2),
        java_hex_double(value.y2),
    )
}

fn quadrilateral(value: NativeStemsHeadLookupGeometry) -> String {
    value.quadrilateral.map(point).join(":")
}

fn double_bounds(value: NativeStemsHeadLookupGeometry) -> String {
    format!(
        "{}:{}:{}:{}",
        java_hex_double(value.double_bounds.x),
        java_hex_double(value.double_bounds.y),
        java_hex_double(value.double_bounds.width),
        java_hex_double(value.double_bounds.height),
    )
}

fn beam_groups(
    system: &NativeStemsHeadCornerReachabilitySystem,
    stumps: &NativeStemsBeamStumpSystem,
    groups: &[usize],
) -> String {
    string_list(
        &groups
            .iter()
            .map(|&group| {
                let members = system
                    .beam_groups_in_source_order
                    .get(group)
                    .expect("beam group")
                    .iter()
                    .map(|&source| beam_sig_ordinal(stumps, source).to_string())
                    .collect::<Vec<_>>();
                format!("[{}]", string_list(&members))
            })
            .collect::<Vec<_>>(),
    )
}

fn seed_ordinal(system: &NativeStemsHeadCornerReachabilitySystem, free_ordinal: usize) -> usize {
    system
        .kept_seed_ordinals
        .iter()
        .position(|&candidate| candidate == free_ordinal)
        .expect("seed in current-system kept pool")
}

fn string_list(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_owned()
    } else {
        values.join(",")
    }
}

fn usize_list(values: &[usize]) -> String {
    string_list(&values.iter().map(usize::to_string).collect::<Vec<_>>())
}

fn oracle_page_rows(oracle: &str, page: &str) -> Vec<String> {
    oracle
        .lines()
        .filter(|row| row.split_whitespace().nth(1) == Some(page))
        .filter(|row| {
            row.split_whitespace()
                .next()
                .is_some_and(|prefix| prefix.starts_with("stemsheadreach"))
        })
        .map(str::to_owned)
        .collect()
}

fn report_first_mismatches(page: &str, actual: &[String], expected: &[String]) {
    let mut shown = 0;
    for index in 0..actual.len().max(expected.len()) {
        let actual = actual.get(index).map(String::as_str).unwrap_or("<missing>");
        let expected = expected
            .get(index)
            .map(String::as_str)
            .unwrap_or("<missing>");
        if actual != expected {
            eprintln!("{page} row {index}\n  actual:   {actual}\n  expected: {expected}");
            shown += 1;
            if shown == 8 {
                break;
            }
        }
    }
}

fn java_hex_double(value: f64) -> String {
    let raw_bits = value.to_bits();
    let canonical_bits = if value.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        raw_bits
    };
    let java = if value.is_nan() {
        "NaN".to_owned()
    } else if value == f64::INFINITY {
        "Infinity".to_owned()
    } else if value == f64::NEG_INFINITY {
        "-Infinity".to_owned()
    } else {
        let sign = if (raw_bits >> 63) != 0 { "-" } else { "" };
        let exponent_bits = ((raw_bits >> 52) & 0x7ff) as i32;
        let fraction_bits = raw_bits & 0x000f_ffff_ffff_ffff;
        if exponent_bits == 0 && fraction_bits == 0 {
            format!("{sign}0x0.0p0")
        } else {
            let mut fraction = format!("{fraction_bits:013x}");
            while fraction.len() > 1 && fraction.ends_with('0') {
                fraction.pop();
            }
            if exponent_bits == 0 {
                format!("{sign}0x0.{fraction}p-1022")
            } else {
                format!("{sign}0x1.{fraction}p{}", exponent_bits - 1023)
            }
        }
    };
    format!("{java}/{canonical_bits:016x}")
}

#[test]
fn head_corner_reachability_oracle_contract_is_frozen() {
    let oracle_bytes = std::fs::read(repo_path(ORACLE_PATH)).expect("head reachability fixture");
    let oracle = std::str::from_utf8(&oracle_bytes).expect("UTF-8 head reachability fixture");
    let probe = std::fs::read(repo_path(PROBE_PATH)).expect("head reachability probe");
    let runner = std::fs::read(repo_path(RUNNER_PATH)).expect("head reachability runner");

    assert_eq!(oracle_bytes.len(), EXPECTED_FIXTURE_BYTES, "fixture bytes");
    assert_eq!(
        oracle.lines().count(),
        EXPECTED_FIXTURE_LINES,
        "fixture lines"
    );
    assert_eq!(sha256_hex(&oracle_bytes), EXPECTED_FIXTURE_SHA256);
    assert_eq!(sha256_hex(&probe), EXPECTED_PROBE_SHA256);
    assert_eq!(sha256_hex(&runner), EXPECTED_RUNNER_SHA256);

    let (body, corpus_summary) = oracle
        .rsplit_once('\n')
        .and_then(|(without_trailing_newline, empty)| {
            assert!(empty.is_empty(), "fixture must end in a newline");
            without_trailing_newline.rsplit_once('\n')
        })
        .expect("fixture body and corpus summary");
    let body = format!("{body}\n");
    assert_eq!(sha256_hex(body.as_bytes()), EXPECTED_BODY_SHA256);
    assert!(corpus_summary.starts_with(&format!(
        "stemsheadreachcorpussummary pages {} {EXPECTED_CORPUS_FIELDS} ",
        PAGES.len()
    )));
    assert_eq!(
        field_value(corpus_summary, "probeSourceSha256"),
        EXPECTED_PROBE_SHA256
    );
    assert_eq!(
        field_value(corpus_summary, "runnerSourceSha256"),
        EXPECTED_RUNNER_SHA256
    );
    assert_eq!(
        field_value(corpus_summary, "emittedBodySha256"),
        EXPECTED_BODY_SHA256
    );

    let mut prefix_counts = BTreeMap::<&str, usize>::new();
    for row in oracle.lines().filter(|row| !row.starts_with('#')) {
        let prefix = row.split_whitespace().next().expect("semantic row prefix");
        assert!(
            EXPECTED_PREFIX_COUNTS
                .iter()
                .any(|(expected, _)| *expected == prefix),
            "unknown semantic row family {prefix}: {row}"
        );
        *prefix_counts.entry(prefix).or_default() += 1;
    }
    for (prefix, expected) in EXPECTED_PREFIX_COUNTS {
        assert_eq!(
            prefix_counts.get(prefix).copied(),
            Some(*expected),
            "{prefix} rows"
        );
    }
    assert_eq!(
        prefix_counts.values().sum::<usize>(),
        79_200,
        "semantic rows"
    );

    let mut page_totals = Totals::default();
    let mut system_totals = Totals::default();
    for row in oracle.lines() {
        if row.starts_with("stemsheadreachpagesummary ") {
            page_totals.include(Totals::from_summary(row));
        } else if row.starts_with("stemsheadreachsystemsummary ") {
            let mut totals = Totals::from_summary(row);
            totals.systems = 1;
            assert_eq!(totals.corners, 4 * totals.heads, "system corner algebra");
            assert_eq!(totals.c_seed_writes, totals.corners, "system seed writes");
            assert!(
                totals.builder_checks >= totals.corners,
                "system builder checks"
            );
            system_totals.include(totals);
        }
    }
    let corpus_totals = Totals::from_summary(corpus_summary);
    assert_eq!(page_totals, corpus_totals, "page summary aggregation");
    assert_eq!(system_totals, corpus_totals, "system summary aggregation");
    assert_eq!(corpus_totals.corners, 4 * corpus_totals.heads);
    assert_eq!(corpus_totals.c_seed_writes, corpus_totals.corners);
    assert_eq!(corpus_totals.builder_checks - corpus_totals.corners, 2_417);

    assert_detail_algebra(oracle, corpus_totals);
    assert_identity_chronology(oracle);
}

fn assert_identity_chronology(oracle: &str) {
    const INSPECT_CORNERS: [&str; 4] = ["TR", "BL", "TL", "BR"];
    const RESULT_CORNERS: [&str; 4] = ["T-R", "B-L", "T-L", "B-R"];
    let mut page_ordinal = 0;
    let mut current_head = None;
    let mut next_head = 0;
    let mut next_inspect = 0;
    let mut next_find = 0;
    let mut system_heads = 0;

    for row in oracle.lines().filter(|row| !row.starts_with('#')) {
        if row.starts_with("stemsheadreachpage ") {
            let expected_page = format!("{}#1", PAGES[page_ordinal]);
            assert_eq!(
                row.split_whitespace().nth(1),
                Some(expected_page.as_str()),
                "page source order"
            );
            page_ordinal += 1;
        } else if row.starts_with("stemsheadreachsystem ") {
            current_head = None;
            next_head = 0;
            next_inspect = 0;
            next_find = 0;
            system_heads = 0;
            let beam_sig_order = token_list(field_value(row, "beamSigOrder"));
            let beam_x_order = token_list(field_value(row, "beamXOrder"));
            let head_sig_order = token_list(field_value(row, "headSigOrder"));
            let head_x_order = token_list(field_value(row, "headXOrder"));
            assert_eq!(
                beam_sig_order,
                (0..beam_sig_order.len())
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>(),
                "beam SIG ordinals"
            );
            assert_eq!(
                head_sig_order,
                (0..head_sig_order.len())
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>(),
                "head SIG ordinals"
            );
            assert_eq!(
                beam_x_order.len(),
                beam_sig_order.len(),
                "beam x permutation"
            );
            assert_eq!(
                head_x_order.len(),
                head_sig_order.len(),
                "head x permutation"
            );
        } else if row.starts_with("stemsheadreachhead ") {
            assert_eq!(required_usize(row, "xOrdinal"), next_head, "head x order");
            current_head = Some(next_head);
            next_head += 1;
            system_heads += 1;
            match field_value(row, "shape") {
                "NOTEHEAD_BLACK" => assert_eq!(field_value(row, "duration"), "1/4"),
                "NOTEHEAD_VOID" => assert_eq!(field_value(row, "duration"), "1/2"),
                shape => panic!("unfrozen head shape {shape}"),
            }
        } else if row.starts_with("stemsheadreachcorner ") {
            let head = current_head.expect("corner after head row");
            assert_eq!(required_usize(row, "head"), head, "corner owner");
            assert_eq!(required_usize(row, "inspectOrdinal"), next_inspect);
            let corner = INSPECT_CORNERS[next_inspect % 4];
            assert_eq!(
                field_value(row, "corner"),
                corner,
                "HeadCorner.values order"
            );
            assert_eq!(
                field_value(row, "alias"),
                format!("h:{head}:{corner}"),
                "stable C alias"
            );
            next_inspect += 1;
        } else if row.starts_with("stemsheadreachbeamtarget ") {
            assert_eq!(
                required_usize(row, "findOrdinal"),
                next_find,
                "find chronology"
            );
            next_find += 1;
        } else if row.starts_with("stemsheadreachresult ") {
            let inspect = required_usize(row, "inspectOrdinal");
            assert_eq!(field_value(row, "corner"), RESULT_CORNERS[inspect % 4]);
        } else if row.starts_with("stemsheadreachsystemsummary ") {
            assert_eq!(next_head, required_usize(row, "heads"));
            assert_eq!(next_inspect, 4 * system_heads);
        }
    }
    assert_eq!(page_ordinal, PAGES.len());
}

fn assert_detail_algebra(oracle: &str, totals: Totals) {
    let semantic = oracle.lines().filter(|row| !row.starts_with('#'));
    let sum = |prefix: &str, field: &str| -> usize {
        semantic
            .clone()
            .filter(|row| row.starts_with(prefix))
            .map(|row| required_usize(row, field))
            .sum()
    };

    assert_eq!(
        sum("stemsheadreachseedscan ", "scans"),
        totals.seeds_scanned
    );
    assert_eq!(sum("stemsheadreachseedscan ", "kept"), totals.seeds_kept);
    assert_eq!(sum("stemsheadreachheadscan ", "scans"), totals.head_scans);
    assert_eq!(
        sum("stemsheadreachheadscan ", "targets"),
        totals.head_targets
    );
    assert_eq!(
        sum("stemsheadreachsiblings ", "scans"),
        totals.sibling_scans
    );
    assert_eq!(
        semantic
            .clone()
            .filter(|row| row.starts_with("stemsheadreachbeamtarget "))
            .count(),
        totals.beam_targets
    );
    assert_eq!(
        semantic
            .clone()
            .filter(|row| row.starts_with("stemsheadreachbeamtarget "))
            .filter(|row| field_value(row, "action") == "createAnchor")
            .count(),
        totals.anchors_created
    );

    let initial_bs = sum("stemsheadreachsystem ", "initialBs");
    let kept_seed_pool = sum("stemsheadreachsystem ", "keptSeeds");
    let after_beam_total: usize = semantic
        .clone()
        .filter(|row| {
            row.starts_with("stemsheadreachbarena ")
                && field_value(row, "phase") == "afterBeamPrefix"
        })
        .map(|row| required_usize(row, "total"))
        .sum();
    let beam_anchors = semantic
        .clone()
        .filter(|row| {
            row.starts_with("stemsheadreachbarena ")
                && field_value(row, "phase") == "afterBeamPrefix"
        })
        .map(|row| required_usize(row, "added"))
        .sum::<usize>();
    let final_bs: usize = semantic
        .clone()
        .filter(|row| {
            row.starts_with("stemsheadreachbarena ") && field_value(row, "phase") == "final"
        })
        .map(|row| required_usize(row, "total"))
        .sum();
    let head_anchors = semantic
        .clone()
        .filter(|row| {
            row.starts_with("stemsheadreachbarena ") && field_value(row, "phase") == "final"
        })
        .map(|row| required_usize(row, "addedFromHeads"))
        .sum::<usize>();
    assert_eq!(initial_bs, 2_116);
    assert_eq!(kept_seed_pool, 1_749);
    assert_eq!(beam_anchors, 145);
    assert_eq!(after_beam_total, initial_bs + beam_anchors);
    assert_eq!(head_anchors, totals.anchors_created);
    assert_eq!(final_bs, after_beam_total + head_anchors);
    assert_eq!(final_bs, 3_948);

    let mut beam_actions = BTreeMap::<&str, usize>::new();
    for row in semantic
        .clone()
        .filter(|row| row.starts_with("stemsheadreachresult "))
    {
        *beam_actions
            .entry(field_value(row, "beamAction"))
            .or_default() += 1;
        let heads = field_value(row, "headTargets");
        let beams = field_value(row, "beamTargets");
        let expected_targets = match (heads, beams) {
            ("-", "-") => "-".to_owned(),
            ("-", beams) => beams.to_owned(),
            (heads, "-") => heads.to_owned(),
            (heads, beams) => format!("{heads},{beams}"),
        };
        assert_eq!(
            field_value(row, "targets"),
            expected_targets,
            "C-before-B targets"
        );
        assert_eq!(field_value(row, "builder"), "null", "C builder seam");
    }
    assert_eq!(beam_actions.get("inspect"), Some(&5_180));
    assert_eq!(beam_actions.get("voidSideSkip"), Some(&1_709));
    assert_eq!(beam_actions.get("none"), Some(&7_195));
}

fn optional_usize(row: &str, field: &str) -> usize {
    row.split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .find_map(|pair| (pair[0] == field).then(|| pair[1].parse().expect("usize field")))
        .unwrap_or(0)
}

fn required_usize(row: &str, field: &str) -> usize {
    field_value(row, field).parse().expect("usize field")
}

fn token_list(value: &str) -> Vec<String> {
    if value == "-" {
        Vec::new()
    } else {
        value.split(',').map(str::to_owned).collect()
    }
}

fn field_value<'a>(row: &'a str, field: &str) -> &'a str {
    let mut words = row.split_whitespace();
    while let Some(word) = words.next() {
        if word == field {
            return words
                .next()
                .unwrap_or_else(|| panic!("missing value for {field}"));
        }
    }
    panic!("missing {field} in {row}")
}

fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut padded = bytes.to_vec();
    let bit_len = u64::try_from(bytes.len()).expect("input length fits u64") * 8;
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    let mut state = [
        0x6a09e667_u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in padded.chunks_exact(64) {
        let mut words = [0_u32; 64];
        for (index, word) in words[..16].iter_mut().enumerate() {
            let offset = index * 4;
            *word = u32::from_be_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let sigma1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temporary1 = h
                .wrapping_add(sigma1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let sigma0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temporary2 = sigma0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temporary1);
            d = c;
            c = b;
            b = a;
            a = temporary1.wrapping_add(temporary2);
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
    state.iter().map(|word| format!("{word:08x}")).collect()
}

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}
