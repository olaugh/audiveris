// SPDX-License-Identifier: AGPL-3.0-or-later

//! Eight-page differential gate for `BeamLinker.inspectVLinkers` reachability.

use std::path::PathBuf;

use audiveris_image::beam_structure::Segment;
use audiveris_omr::{
    beam_inters::BeamKind,
    head_scanner_slices::JavaRectangle,
    head_template::HeadTemplateShape,
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads,
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::{
        NativeStemSeedRecognition, NativeStemSeedSystemRecognition, recognize_native_stem_seeds,
    },
    native_stems_beam_reachability::{
        NativeStemsBeamArena, NativeStemsBeamArenaEntry, NativeStemsBeamArenaOrigin,
        NativeStemsBeamBVisit, NativeStemsBeamFindLinker, NativeStemsBeamFindResult,
        NativeStemsBeamHeadCornerAction, NativeStemsBeamHeadDecision,
        NativeStemsBeamHeadDecisionAction, NativeStemsBeamHeadSearch, NativeStemsBeamInspection,
        NativeStemsBeamReachabilityRecognition, NativeStemsBeamReachabilitySystem,
        NativeStemsBeamTargetAction, NativeStemsBeamVInspection,
        materialize_native_stems_beam_reachability,
    },
    native_stems_beam_stumps::{
        NativeStemsBeamGlyph, NativeStemsBeamSource, NativeStemsBeamStumpBeam,
        NativeStemsBeamStumpRecognition, NativeStemsBeamStumpSystem,
        materialize_native_stems_beam_stumps,
    },
    native_stems_beam_vlinkers::{
        NativeStemsBeamBLinker, NativeStemsBeamBLinkerOrigin, NativeStemsBeamBLinkerRef,
        NativeStemsBeamDoubleBounds, NativeStemsBeamVLinker, NativeStemsBeamVLinkerRecognition,
        NativeStemsBeamVLinkerRef, NativeStemsBeamVLinkerSystem,
        materialize_native_stems_beam_vlinkers,
    },
    native_stems_head_corners::{
        NativeStemsHeadCornerRecognition, NativeStemsHeadCornerSystem,
        materialize_native_stems_head_corners,
    },
    native_stems_head_seeds::{
        NativeStemsHeadSeedRecognition, NativeStemsHeadSeedSystem,
        materialize_native_stems_head_seeds,
    },
    recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds},
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
const ORACLE_PATH: &str = "rust/oracle/stems-beam-vlinker-inspect.txt";
const PROBE_PATH: &str = "rust/oracle/java/StemsBeamVLinkerInspectProbe.java";
const RUNNER_PATH: &str = "rust/oracle/java/run-stems-beam-vlinker-inspect.sh";

const EXPECTED_FIXTURE_SHA256: Option<&str> =
    Some("9c3f6d17fa6806cba9b01f3922aca34a220d21dc1a5269723e151a025c693221");
const EXPECTED_PROBE_SHA256: Option<&str> =
    Some("39ed0694f7c31593f157b5f250f8bfa4f006984e3b491a877903d64d810edd7b");
const EXPECTED_RUNNER_SHA256: Option<&str> =
    Some("61801362bc7328cfb3e90f7460016e333d776ee964d39cc296f60cf6edac33f1");
const EXPECTED_BODY_SHA256: Option<&str> =
    Some("470827ebc19065890c41c10016511e77eeefc851823bb8587f7537c7e7db23cf");
const EXPECTED_FIXTURE_BYTES: Option<usize> = Some(61_411_164);
const EXPECTED_FIXTURE_LINES: Option<usize> = Some(232_460);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Totals {
    beams: usize,
    small_beams: usize,
    heads: usize,
    small_heads: usize,
    corners: usize,
    initial_bs: usize,
    initial_vs: usize,
    beam_starts: usize,
    b_visits: usize,
    anchor_skips: usize,
    inspected_vs: usize,
    sibling_scans: usize,
    sibling_hits: usize,
    beam_candidates: usize,
    eligible_beams: usize,
    find_calls: usize,
    find_candidates: usize,
    b_reuses: usize,
    anchor_reuses: usize,
    anchors_created: usize,
    head_lookups: usize,
    head_scans: usize,
    head_area_hits: usize,
    head_competing_passes: usize,
    head_competing_removals: usize,
    head_competitor_drops: usize,
    small_head_candidates: usize,
    size_drops: usize,
    head_near_drops: usize,
    corner_checks: usize,
    corner_inside: usize,
    void_corner_drops: usize,
    c_accepted: usize,
    result_bs: usize,
    result_cs: usize,
    final_bs: usize,
    seed_snapshots: usize,
    builder_checks: usize,
}

impl Totals {
    fn include(&mut self, other: Self) {
        self.beams += other.beams;
        self.small_beams += other.small_beams;
        self.heads += other.heads;
        self.small_heads += other.small_heads;
        self.corners += other.corners;
        self.initial_bs += other.initial_bs;
        self.initial_vs += other.initial_vs;
        self.beam_starts += other.beam_starts;
        self.b_visits += other.b_visits;
        self.anchor_skips += other.anchor_skips;
        self.inspected_vs += other.inspected_vs;
        self.sibling_scans += other.sibling_scans;
        self.sibling_hits += other.sibling_hits;
        self.beam_candidates += other.beam_candidates;
        self.eligible_beams += other.eligible_beams;
        self.find_calls += other.find_calls;
        self.find_candidates += other.find_candidates;
        self.b_reuses += other.b_reuses;
        self.anchor_reuses += other.anchor_reuses;
        self.anchors_created += other.anchors_created;
        self.head_lookups += other.head_lookups;
        self.head_scans += other.head_scans;
        self.head_area_hits += other.head_area_hits;
        self.head_competing_passes += other.head_competing_passes;
        self.head_competing_removals += other.head_competing_removals;
        self.head_competitor_drops += other.head_competitor_drops;
        self.small_head_candidates += other.small_head_candidates;
        self.size_drops += other.size_drops;
        self.head_near_drops += other.head_near_drops;
        self.corner_checks += other.corner_checks;
        self.corner_inside += other.corner_inside;
        self.void_corner_drops += other.void_corner_drops;
        self.c_accepted += other.c_accepted;
        self.result_bs += other.result_bs;
        self.result_cs += other.result_cs;
        self.final_bs += other.final_bs;
        self.seed_snapshots += other.seed_snapshots;
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

#[derive(Default)]
struct Aliases {
    beam_glyphs: Vec<NativeStemsBeamGlyph>,
}

impl Aliases {
    fn new(system: &NativeStemsBeamStumpSystem) -> Self {
        let mut aliases = Self::default();
        for beam in &system.beams_by_abscissa {
            if !aliases.beam_glyphs.contains(&beam.beam_glyph) {
                aliases.beam_glyphs.push(beam.beam_glyph.clone());
            }
        }
        aliases
    }

    fn glyph(&self, glyph: &NativeStemsBeamGlyph) -> usize {
        self.beam_glyphs
            .iter()
            .position(|candidate| candidate == glyph)
            .expect("preassigned beam glyph")
    }
}

#[test]
fn native_stems_beam_reachability_matches_java_corpus_exactly() {
    let oracle = std::fs::read_to_string(repo_path(ORACLE_PATH))
        .expect("BeamVLinker-inspect oracle fixture");
    assert_corpus_summary(&oracle);

    let mut corpus_totals = Totals::default();
    for image in PAGES {
        let page = format!("{image}#1");
        let (actual, totals) = native_page_rows(image, &page);
        let expected = oracle_page_rows(&oracle, &page);
        if actual != expected {
            report_first_mismatches(&page, &actual, &expected);
            panic!("{page}: native BeamVLinker-inspect evidence differs from Java");
        }
        corpus_totals.include(totals);
    }
    assert_known_totals(corpus_totals);
}

fn native_page_rows(image: &str, page: &str) -> (Vec<String>, Totals) {
    let grid = recognize_grid_lines(repo_path(&format!("data/examples/{image}")))
        .unwrap_or_else(|error| panic!("{page}: GRID failed: {error}"));
    let headers = recognize_native_headers(&grid)
        .unwrap_or_else(|error| panic!("{page}: HEADERS failed: {error}"));
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers)
        .unwrap_or_else(|error| panic!("{page}: STEM_SEEDS failed: {error}"));
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .unwrap_or_else(|error| panic!("{page}: BEAMS failed: {error}"));
    let ledgers = recognize_native_ledgers(&grid, &beams)
        .unwrap_or_else(|error| panic!("{page}: LEDGERS failed: {error}"));
    let heads = recognize_native_heads(&grid, &headers, &stem_seeds, &beams, &ledgers)
        .unwrap_or_else(|error| panic!("{page}: HEADS failed: {error}"));
    let corners = materialize_native_stems_head_corners(&heads, &stem_seeds)
        .unwrap_or_else(|error| panic!("{page}: STEMS corners failed: {error}"));
    let head_seeds = materialize_native_stems_head_seeds(&grid, &stem_seeds, &corners)
        .unwrap_or_else(|error| panic!("{page}: STEMS head seeds failed: {error}"));
    let stumps =
        materialize_native_stems_beam_stumps(&grid, &beams, &heads, &stem_seeds, &head_seeds)
            .unwrap_or_else(|error| panic!("{page}: STEMS beam stumps failed: {error}"));
    let constructors = materialize_native_stems_beam_vlinkers(&grid, &beams, &stem_seeds, &stumps)
        .unwrap_or_else(|error| panic!("{page}: STEMS beam VLinkers failed: {error}"));
    let reachability =
        materialize_native_stems_beam_reachability(&beams, &stumps, &constructors, &corners)
            .unwrap_or_else(|error| panic!("{page}: beam reachability failed: {error}"));

    let mut rows = vec![format!(
        "stemsbeamvinspectpage {page} systems {} staves {} family Bravura",
        reachability.systems.len(),
        grid.staves.len(),
    )];
    let mut page_totals = Totals::default();
    let mut page_hash = RowHasher::default();
    for system in &reachability.systems {
        let stump_system = stump_system(&stumps, system.system_id);
        let constructor_system = constructor_system(&constructors, system.system_id);
        let head_system = head_system(&corners, system.system_id);
        let seed_system = seed_system(&stem_seeds, system.system_id);
        let kept_system = kept_system(&head_seeds, system.system_id);
        let mut system_rows = Vec::new();
        let totals = append_system_rows(
            page,
            system,
            stump_system,
            constructor_system,
            head_system,
            seed_system,
            kept_system,
            &mut system_rows,
        );
        let mut system_hash = RowHasher::default();
        for row in &system_rows {
            system_hash.add(row);
            page_hash.add(row);
        }
        let summary = summary_row(
            "stemsbeamvinspectsystemsummary",
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
        "stemsbeamvinspectpagesummary",
        page,
        None,
        reachability.systems.len(),
        page_totals,
        page_hash.0,
    ));
    assert_recognition_totals(&reachability, page_totals);
    (rows, page_totals)
}

#[allow(clippy::too_many_arguments)]
fn append_system_rows(
    page: &str,
    system: &NativeStemsBeamReachabilitySystem,
    stumps: &NativeStemsBeamStumpSystem,
    constructors: &NativeStemsBeamVLinkerSystem,
    heads: &NativeStemsHeadCornerSystem,
    seeds: &NativeStemSeedSystemRecognition,
    kept: &NativeStemsHeadSeedSystem,
    rows: &mut Vec<String>,
) -> Totals {
    let aliases = Aliases::new(stumps);
    let mut totals = Totals {
        beams: system.beam_sources_in_inspection_order.len(),
        small_beams: system
            .beam_inspections
            .iter()
            .filter(|beam| beam.is_small_beam)
            .count(),
        heads: heads.heads_in_sig_order.len(),
        small_heads: 0,
        corners: heads
            .heads_in_sig_order
            .iter()
            .map(|head| head.corners_in_constructor_order.len())
            .sum(),
        beam_starts: system.beam_inspections.len(),
        ..Totals::default()
    };
    let beam_sig_order =
        usize_list(&(0..system.beam_sources_in_inspection_order.len()).collect::<Vec<_>>());
    let beam_x_order = usize_list(
        &system
            .beam_sources_in_inspection_order
            .iter()
            .map(|&source| beam(stumps, source).sig_ordinal)
            .collect::<Vec<_>>(),
    );
    let head_pre_sort = usize_list(&(0..heads.heads_in_sig_order.len()).collect::<Vec<_>>());
    let head_x_order = usize_list(
        &system
            .heads_in_abscissa_order
            .iter()
            .map(|head| head.sig_ordinal)
            .collect::<Vec<_>>(),
    );
    rows.push(format!(
        "stemsbeamvinspectsystem {page} system {} profile {} interline {} bounds {} \
         beamSigOrder {} beamXOrder {} headPreSort {} headXOrder {} keptSeeds {} \
         maxBeamLinkerDx {} maxBeamSideDx {} maxBeamGroupDy {} minBeamHeadDy {} \
         vicinityMargin {} slopeMargin {} \
         halfBeamLuDx {} maxBeamSeedDyRatio {} allowSmallHeadOnStandardBeam {}",
        system.system_id,
        heads.profile,
        system.interline,
        rectangle(stumps.system_bounds),
        beam_sig_order,
        beam_x_order,
        head_pre_sort,
        head_x_order,
        kept.kept_seed_ordinals.len(),
        system.max_beam_linker_dx,
        system.max_beam_side_dx,
        constructors.max_beam_group_dy,
        system.min_beam_head_dy,
        constructors.vicinity_margin,
        java_hex_double(constructors.slope_margin),
        java_hex_double(constructors.half_beam_lookup_dx),
        java_hex_double(constructors.max_beam_seed_dy_ratio),
        system.allow_small_head_on_standard_beam,
    ));

    for (x_ordinal, &source) in system.beam_sources_in_inspection_order.iter().enumerate() {
        let item = beam(stumps, source);
        let group = &system.groups_in_source_order[item.group_ordinal];
        rows.push(format!(
            "stemsbeamvinspectbeamorder {page} system {} xOrdinal {x_ordinal} sigOrdinal {} \
             shape {} small {} bounds {} median {} glyph beamglyph:{} groupMembers {}",
            system.system_id,
            item.sig_ordinal,
            beam_shape(item.kind),
            item.kind == BeamKind::SmallBeam,
            rectangle(item.bounds),
            segment(item.median),
            aliases.glyph(&item.beam_glyph),
            source_sig_list(group, stumps),
        ));
    }

    for candidate in &system.heads_in_abscissa_order {
        let head = &heads.heads_in_sig_order[candidate.sig_ordinal];
        let staff_ordinal = constructors
            .staff_ids
            .iter()
            .position(|&staff| staff == head.staff_id)
            .expect("head staff belongs to system");
        rows.push(format!(
            "stemsbeamvinspecthead {page} system {} xOrdinal {} preSortOrdinal {} shape {} \
             bounds {} center {}:{} staffOrdinal {} grade {} small false half {}",
            system.system_id,
            candidate.x_ordinal,
            candidate.sig_ordinal,
            head_shape(head.shape),
            rectangle(head.bounds),
            head.bounds.x.wrapping_add(head.bounds.width / 2),
            head.bounds.y.wrapping_add(head.bounds.height / 2),
            staff_ordinal,
            java_hex_double(f64::from_bits(head.grade_bits)),
            head.shape == HeadTemplateShape::NoteheadVoid,
        ));
        for corner in &head.corners_in_constructor_order {
            rows.push(format!(
                "stemsbeamvinspectcorner {page} system {} head {} constructorOrdinal {} \
                 hSide {} vSide {} alias {} ref {} builder null",
                system.system_id,
                candidate.x_ordinal,
                corner.constructor_ordinal,
                head_side(corner.horizontal),
                vertical_side(corner.vertical),
                c_alias(candidate.x_ordinal, corner.horizontal, corner.vertical,),
                point(corner.reference),
            ));
        }
    }

    for &source in &system.beam_sources_in_inspection_order {
        let arena = arena(system, source);
        let constructor = constructor(constructors, source);
        totals.initial_bs += arena.initial_b_count;
        for entry in arena.all_b_linkers.iter().take(arena.initial_b_count) {
            let b = b_linker(constructor, entry.reference.id);
            totals.initial_vs += b.v_linkers.len();
            rows.push(b_state_row(
                "stemsbeamvinspectinitialb",
                page,
                system.system_id,
                stumps,
                entry,
                b.v_linkers.iter().map(|v| v.reference.side),
            ));
        }
    }

    for inspection in &system.beam_inspections {
        append_inspection_rows(
            page,
            system,
            stumps,
            constructors,
            heads,
            seeds,
            kept,
            &aliases,
            inspection,
            &mut totals,
            rows,
        );
    }

    for &source in &system.beam_sources_in_inspection_order {
        let arena = arena(system, source);
        let constructor = constructor(constructors, source);
        totals.final_bs += arena.all_b_linkers.len();
        for entry in &arena.all_b_linkers {
            let sides = if entry.is_anchor {
                Vec::new()
            } else {
                b_linker(constructor, entry.reference.id)
                    .v_linkers
                    .iter()
                    .map(|v| v.reference.side)
                    .collect()
            };
            rows.push(b_state_row(
                "stemsbeamvinspectfinalb",
                page,
                system.system_id,
                stumps,
                entry,
                sides,
            ));
        }
    }
    totals.seed_snapshots = totals.initial_vs;
    totals.builder_checks = (3 * totals.initial_vs) + (2 * totals.corners);
    assert_system_invariants(totals);
    totals
}

#[allow(clippy::too_many_arguments)]
fn append_inspection_rows(
    page: &str,
    system: &NativeStemsBeamReachabilitySystem,
    stumps: &NativeStemsBeamStumpSystem,
    constructors: &NativeStemsBeamVLinkerSystem,
    heads: &NativeStemsHeadCornerSystem,
    seeds: &NativeStemSeedSystemRecognition,
    kept: &NativeStemsHeadSeedSystem,
    aliases: &Aliases,
    inspection: &NativeStemsBeamInspection,
    totals: &mut Totals,
    rows: &mut Vec<String>,
) {
    let item = beam(stumps, inspection.beam);
    rows.push(format!(
        "stemsbeamvinspectbeamstart {page} system {} inspection {} xOrdinal {} sigOrdinal {} \
         allB {}",
        system.system_id,
        inspection.inspection_ordinal,
        item.x_ordinal,
        item.sig_ordinal,
        b_snapshot(system, stumps, &inspection.all_b_before),
    ));
    for visit in &inspection.b_visits {
        append_b_visit_rows(
            page,
            system,
            stumps,
            constructors,
            heads,
            seeds,
            kept,
            aliases,
            inspection,
            visit,
            totals,
            rows,
        );
    }
    rows.push(format!(
        "stemsbeamvinspectbeamend {page} system {} inspection {} sigOrdinal {} allB {}",
        system.system_id,
        inspection.inspection_ordinal,
        item.sig_ordinal,
        b_snapshot(system, stumps, &inspection.all_b_after),
    ));
}

#[allow(clippy::too_many_arguments)]
fn append_b_visit_rows(
    page: &str,
    system: &NativeStemsBeamReachabilitySystem,
    stumps: &NativeStemsBeamStumpSystem,
    constructors: &NativeStemsBeamVLinkerSystem,
    heads: &NativeStemsHeadCornerSystem,
    seeds: &NativeStemSeedSystemRecognition,
    kept: &NativeStemsHeadSeedSystem,
    aliases: &Aliases,
    inspection: &NativeStemsBeamInspection,
    visit: &NativeStemsBeamBVisit,
    totals: &mut Totals,
    rows: &mut Vec<String>,
) {
    totals.b_visits += 1;
    totals.anchor_skips += usize::from(visit.skipped_anchor);
    let entry = arena_entry(system, visit.reference);
    let v_sides = visit
        .v_inspections
        .iter()
        .map(|v| v.reference.side)
        .collect::<Vec<_>>();
    rows.push(format!(
        "stemsbeamvinspectbvisit {page} system {} inspection {} beamSig {} ordinal {} \
         alias {} isAnchor {} action {} hSide {} maxProfile {} vSides {}",
        system.system_id,
        inspection.inspection_ordinal,
        beam(stumps, inspection.beam).sig_ordinal,
        visit.visit_ordinal,
        b_alias(stumps, visit.reference),
        visit.is_anchor,
        if visit.skipped_anchor {
            "skipAnchor"
        } else {
            "visit"
        },
        if visit.skipped_anchor {
            "-"
        } else {
            optional_head_side(entry.horizontal_side)
        },
        visit
            .max_stem_profile
            .map_or_else(|| "-".to_owned(), |value| value.to_string()),
        vertical_side_list(&v_sides),
    ));
    for v in &visit.v_inspections {
        append_v_rows(
            page,
            system,
            stumps,
            constructors,
            heads,
            seeds,
            kept,
            aliases,
            inspection,
            entry,
            v,
            totals,
            rows,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn append_v_rows(
    page: &str,
    system: &NativeStemsBeamReachabilitySystem,
    stumps: &NativeStemsBeamStumpSystem,
    constructors: &NativeStemsBeamVLinkerSystem,
    heads: &NativeStemsHeadCornerSystem,
    _seeds: &NativeStemSeedSystemRecognition,
    kept: &NativeStemsHeadSeedSystem,
    aliases: &Aliases,
    inspection: &NativeStemsBeamInspection,
    entry: &NativeStemsBeamArenaEntry,
    v: &NativeStemsBeamVInspection,
    totals: &mut Totals,
    rows: &mut Vec<String>,
) {
    totals.inspected_vs += 1;
    let source_beam = beam(stumps, inspection.beam);
    let constructor_v = v_linker(constructors, v.reference);
    let seed_tokens = usize_list(
        &constructor_v
            .reachable_seed_ordinals
            .iter()
            .map(|free| {
                kept.kept_seed_ordinals
                    .iter()
                    .position(|candidate| candidate == free)
                    .expect("V seed remains in kept seed pool")
            })
            .collect::<Vec<_>>(),
    );
    rows.push(format!(
        "stemsbeamvinspectv {page} system {} inspectOrdinal {} beamSig {} beamX {} \
         bOrdinal {} bAlias {} beamShape {} beamSmall {} hSide {} ref {} vSide {} yDir {} \
         maxProfile {} theo {} luYLimit {} luQuad {} luBounds {} luBounds2d {} seeds {}",
        system.system_id,
        v.inspection_ordinal,
        source_beam.sig_ordinal,
        source_beam.x_ordinal,
        v.reference.b_linker.id - 1,
        b_alias(stumps, v.reference.b_linker),
        beam_shape(source_beam.kind),
        source_beam.kind == BeamKind::SmallBeam,
        optional_head_side(entry.horizontal_side),
        point(entry.reference_point),
        vertical_side(v.reference.side),
        v.y_direction,
        v.max_stem_profile,
        stem_line(v.theoretical_line),
        java_hex_double(constructor_v.final_geometry.y_limit),
        quadrilateral(v.lookup_quadrilateral),
        rectangle(v.lookup_bounds),
        double_bounds(constructor_v.final_geometry.double_bounds),
        seed_tokens,
    ));

    let vertical = Segment {
        x1: entry.reference_point.x,
        y1: entry.reference_point.y,
        x2: entry.reference_point.x - (1000.0 * system.global_slope),
        y2: entry.reference_point.y + 1000.0,
    };
    totals.sibling_scans += v.sibling_checks.len();
    totals.sibling_hits += v.siblings.len();
    for check in &v.sibling_checks {
        let sibling = beam(stumps, check.beam);
        let sorted_ordinal = v
            .siblings
            .iter()
            .position(|hit| hit.beam == check.beam)
            .map_or(-1, |ordinal| ordinal as i32);
        rows.push(format!(
            "stemsbeamvinspectsibling {page} system {} beamSig {} bOrdinal {} vSide {} \
             groupOrdinal {} siblingSig {} siblingX {} glyph beamglyph:{} median {} vertical {} \
             cross {} margin {} within {} sortedOrdinal {}",
            system.system_id,
            source_beam.sig_ordinal,
            v.reference.b_linker.id - 1,
            vertical_side(v.reference.side),
            check.input_ordinal,
            sibling.sig_ordinal,
            sibling.x_ordinal,
            aliases.glyph(&sibling.beam_glyph),
            segment(sibling.median),
            segment(vertical),
            point(check.cross),
            system.max_beam_side_dx,
            check.accepted,
            sorted_ordinal,
        ));
    }

    totals.beam_candidates += v.beam_decisions.len();
    for decision in &v.beam_decisions {
        let target = beam(stumps, decision.beam);
        let action = match &decision.action {
            NativeStemsBeamTargetAction::SelfBeam => "self",
            NativeStemsBeamTargetAction::MissingGlyph => "nullGlyph",
            NativeStemsBeamTargetAction::SameGlyph => "sameGlyph",
            NativeStemsBeamTargetAction::FindLinker(_) => "find",
        };
        rows.push(format!(
            "stemsbeamvinspectbeamcandidate {page} system {} sourceSig {} bOrdinal {} \
             vSide {} siblingOrdinal {} targetSig {} targetX {} sourceGlyph beamglyph:{} \
             targetGlyph beamglyph:{} action {}",
            system.system_id,
            source_beam.sig_ordinal,
            v.reference.b_linker.id - 1,
            vertical_side(v.reference.side),
            decision.sibling_ordinal,
            target.sig_ordinal,
            target.x_ordinal,
            aliases.glyph(&source_beam.beam_glyph),
            aliases.glyph(&target.beam_glyph),
            action,
        ));
        if let NativeStemsBeamTargetAction::FindLinker(find) = &decision.action {
            append_find_scan_rows(page, system, stumps, v, find, totals, rows);
        }
    }
    for decision in &v.beam_decisions {
        if let NativeStemsBeamTargetAction::FindLinker(find) = &decision.action {
            append_find_result_row(page, system, stumps, v, find, totals, rows);
        }
    }

    append_head_rows(
        page,
        system,
        stumps,
        heads,
        source_beam,
        constructor_v,
        v,
        totals,
        rows,
    );

    totals.result_bs += v.beam_targets.len();
    totals.result_cs += v.head_targets.len();
    let b_targets = list(
        &v.beam_targets
            .iter()
            .map(|&target| b_alias(stumps, target))
            .collect::<Vec<_>>(),
    );
    let c_targets = list(
        &v.head_targets
            .iter()
            .map(|target| c_alias(target.x_ordinal, target.horizontal, target.vertical))
            .collect::<Vec<_>>(),
    );
    let targets = list(
        &v.ordered_targets
            .iter()
            .map(|target| match target {
                audiveris_omr::native_stems_beam_reachability::NativeStemsBeamReachabilityTarget::Beam(
                    reference,
                ) => b_alias(stumps, *reference),
                audiveris_omr::native_stems_beam_reachability::NativeStemsBeamReachabilityTarget::Head(
                    reference,
                ) => c_alias(reference.x_ordinal, reference.horizontal, reference.vertical),
            })
            .collect::<Vec<_>>(),
    );
    rows.push(format!(
        "stemsbeamvinspectresult {page} system {} inspectOrdinal {} beamSig {} bOrdinal {} \
         vSide {} bTargets {} cTargets {} targets {} seeds {} maxProfile {} builder null",
        system.system_id,
        v.inspection_ordinal,
        source_beam.sig_ordinal,
        v.reference.b_linker.id - 1,
        vertical_side(v.reference.side),
        b_targets,
        c_targets,
        targets,
        seed_tokens,
        v.max_stem_profile,
    ));
}

#[allow(clippy::too_many_arguments)]
fn append_head_rows(
    page: &str,
    system: &NativeStemsBeamReachabilitySystem,
    stumps: &NativeStemsBeamStumpSystem,
    heads: &NativeStemsHeadCornerSystem,
    source_beam: &NativeStemsBeamStumpBeam,
    constructor_v: &NativeStemsBeamVLinker,
    v: &NativeStemsBeamVInspection,
    totals: &mut Totals,
    rows: &mut Vec<String>,
) {
    totals.head_lookups += 1;
    let search = &v.head_search;
    if search.skipped_for_empty_siblings {
        rows.push(format!(
            "stemsbeamvinspectheadlookup {page} system {} beamSig {} bOrdinal {} vSide {} \
             siblings - action empty luBounds {}",
            system.system_id,
            source_beam.sig_ordinal,
            v.reference.b_linker.id - 1,
            vertical_side(v.reference.side),
            rectangle(v.lookup_bounds),
        ));
        return;
    }

    let last_source = search.last_sibling.expect("nonempty head search sibling");
    let last_beam = beam(stumps, last_source);
    let last_side = search.last_border_side.expect("nonempty last border side");
    rows.push(format!(
        "stemsbeamvinspectheadlookup {page} system {} beamSig {} bOrdinal {} vSide {} \
         siblings {} lastBeamSig {} lastBorderSide {} lastBorder {} refX {} yLastBorder {} \
         luBounds {} luBounds2d {} systemHeads {}",
        system.system_id,
        source_beam.sig_ordinal,
        v.reference.b_linker.id - 1,
        vertical_side(v.reference.side),
        source_sig_list(
            &v.siblings.iter().map(|hit| hit.beam).collect::<Vec<_>>(),
            stumps,
        ),
        last_beam.sig_ordinal,
        vertical_side(last_side),
        segment(search.last_border.expect("nonempty last border")),
        java_hex_double(v.theoretical_line.start.x),
        java_hex_double(search.y_last_border.expect("nonempty last border y")),
        rectangle(v.lookup_bounds),
        double_bounds(constructor_v.final_geometry.double_bounds),
        usize_list(&(0..heads.heads_in_sig_order.len()).collect::<Vec<_>>()),
    ));

    totals.head_scans += search.scans.len();
    totals.head_area_hits += search.candidates_before_competing_removal.len();
    let area_max_x = f64::from(v.lookup_bounds.x.wrapping_add(v.lookup_bounds.width));
    let mut candidate_ordinal = 0_i32;
    for (input_ordinal, scan) in search.scans.iter().enumerate() {
        let ordinal = if scan.intersects_lookup_area {
            let result = candidate_ordinal;
            candidate_ordinal += 1;
            result
        } else {
            -1
        };
        rows.push(format!(
            "stemsbeamvinspectheadscan {page} system {} beamSig {} bOrdinal {} vSide {} \
             inputOrdinal {} headX {} bounds {} removed false intersects {} areaMaxX {} \
             candidateOrdinal {} breakAfter {}",
            system.system_id,
            source_beam.sig_ordinal,
            v.reference.b_linker.id - 1,
            vertical_side(v.reference.side),
            input_ordinal,
            scan.x_ordinal,
            rectangle(scan.bounds),
            scan.intersects_lookup_area,
            java_hex_double(area_max_x),
            ordinal,
            scan.breaks_after,
        ));
    }

    append_competing_rows(page, system, stumps, source_beam, v, search, totals, rows);
    append_head_candidate_rows(page, system, heads, source_beam, v, search, totals, rows);
}

#[allow(clippy::too_many_arguments)]
fn append_competing_rows(
    page: &str,
    system: &NativeStemsBeamReachabilitySystem,
    stumps: &NativeStemsBeamStumpSystem,
    source_beam: &NativeStemsBeamStumpBeam,
    v: &NativeStemsBeamVInspection,
    search: &NativeStemsBeamHeadSearch,
    totals: &mut Totals,
    rows: &mut Vec<String>,
) {
    let mut survivors = search.candidates_before_competing_removal.clone();
    totals.head_competing_passes += search.competing_removals.len();
    for removal in &search.competing_removals {
        let before = survivors.clone();
        survivors.retain(|candidate| !removal.removed_heads.contains(candidate));
        totals.head_competing_removals += removal.removed_heads.len();
        rows.push(format!(
            "stemsbeamvinspectheadcompeting {page} system {} beamSig {} bOrdinal {} vSide {} \
             siblingOrdinal {} siblingSig {} before {} removals {} after {}",
            system.system_id,
            source_beam.sig_ordinal,
            v.reference.b_linker.id - 1,
            vertical_side(v.reference.side),
            removal.sibling_ordinal,
            beam(stumps, removal.sibling).sig_ordinal,
            head_x_list(&before),
            head_x_list(&removal.removed_heads),
            head_x_list(&survivors),
        ));
    }
    assert_eq!(survivors, search.candidates_after_competing_removal);
}

#[allow(clippy::too_many_arguments)]
fn append_head_candidate_rows(
    page: &str,
    system: &NativeStemsBeamReachabilitySystem,
    heads: &NativeStemsHeadCornerSystem,
    source_beam: &NativeStemsBeamStumpBeam,
    v: &NativeStemsBeamVInspection,
    search: &NativeStemsBeamHeadSearch,
    totals: &mut Totals,
    rows: &mut Vec<String>,
) {
    let mut removed = Vec::new();
    for pass in &search.competing_removals {
        for candidate in &pass.removed_heads {
            if !removed.contains(candidate) {
                removed.push(*candidate);
            }
        }
    }
    totals.head_competitor_drops += removed.len();
    for candidate in &removed {
        let head = &heads.heads_in_sig_order[candidate.sig_ordinal];
        let siblings = search
            .competing_removals
            .iter()
            .filter(|pass| pass.removed_heads.contains(candidate))
            .map(|pass| pass.sibling_ordinal.to_string())
            .collect::<Vec<_>>();
        rows.push(format!(
            "stemsbeamvinspectheadcandidate {page} system {} beamSig {} bOrdinal {} vSide {} \
             areaOrdinal {} survivorOrdinal - headX {} shape {} headSmall false beamSmall {} \
             allowSmall {} sizeMismatch - centerY - yLastBorder - dy - minBeamHeadDy - \
             distanceAccepted - competingSiblings {} action competitor",
            system.system_id,
            source_beam.sig_ordinal,
            v.reference.b_linker.id - 1,
            vertical_side(v.reference.side),
            search
                .candidates_before_competing_removal
                .iter()
                .position(|value| value == candidate)
                .expect("removed head was an area candidate"),
            candidate.x_ordinal,
            head_shape(head.shape),
            source_beam.kind == BeamKind::SmallBeam,
            system.allow_small_head_on_standard_beam,
            list(&siblings),
        ));
    }

    for (survivor_ordinal, decision) in search.decisions.iter().enumerate() {
        append_head_decision_row(
            page,
            system,
            heads,
            source_beam,
            v,
            search,
            survivor_ordinal,
            decision,
            totals,
            rows,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn append_head_decision_row(
    page: &str,
    system: &NativeStemsBeamReachabilitySystem,
    _heads: &NativeStemsHeadCornerSystem,
    source_beam: &NativeStemsBeamStumpBeam,
    v: &NativeStemsBeamVInspection,
    search: &NativeStemsBeamHeadSearch,
    survivor_ordinal: usize,
    decision: &NativeStemsBeamHeadDecision,
    totals: &mut Totals,
    rows: &mut Vec<String>,
) {
    totals.small_head_candidates += usize::from(decision.is_small_head);
    let action = match decision.action {
        NativeStemsBeamHeadDecisionAction::SizeRejected => {
            totals.size_drops += 1;
            "size"
        }
        NativeStemsBeamHeadDecisionAction::DistanceRejected => {
            totals.head_near_drops += 1;
            "near"
        }
        NativeStemsBeamHeadDecisionAction::CornersChecked => "corners",
    };
    rows.push(format!(
        "stemsbeamvinspectheadcandidate {page} system {} beamSig {} bOrdinal {} vSide {} \
         areaOrdinal {} survivorOrdinal {} headX {} shape {} headSmall {} beamSmall {} \
         allowSmall {} sizeMismatch {} centerY {} yLastBorder {} dy {} minBeamHeadDy {} \
         distanceAccepted {} action {}",
        system.system_id,
        source_beam.sig_ordinal,
        v.reference.b_linker.id - 1,
        vertical_side(v.reference.side),
        search
            .candidates_before_competing_removal
            .iter()
            .position(|candidate| *candidate == decision.candidate)
            .expect("surviving head was an area candidate"),
        survivor_ordinal,
        decision.candidate.x_ordinal,
        head_shape(decision.shape),
        decision.is_small_head,
        decision.is_small_beam,
        decision.allow_small_head_on_standard_beam,
        !decision.size_accepted,
        optional_i32(decision.center_y),
        optional_double(decision.y_last_border),
        optional_double(decision.directed_dy),
        optional_i32(decision.min_beam_head_dy),
        optional_bool(decision.distance_accepted),
        action,
    ));
    for (corner_ordinal, corner) in decision.corner_checks.iter().enumerate() {
        totals.corner_checks += 1;
        totals.corner_inside += usize::from(corner.lookup_contains);
        totals.void_corner_drops +=
            usize::from(corner.action == NativeStemsBeamHeadCornerAction::VoidSideRejected);
        totals.c_accepted += usize::from(corner.accepted);
        let action = match corner.action {
            NativeStemsBeamHeadCornerAction::OutsideLookup => "outside",
            NativeStemsBeamHeadCornerAction::VoidSideRejected => "void",
            NativeStemsBeamHeadCornerAction::Accepted => "accept",
        };
        rows.push(format!(
            "stemsbeamvinspectcornercheck {page} system {} beamSig {} bOrdinal {} vSide {} \
             headX {} cornerOrdinal {} hSide {} targetVSide {} cAlias {} ref {} contains {} \
             half {} imposedVoidSide {} voidSideOk {} action {}",
            system.system_id,
            source_beam.sig_ordinal,
            v.reference.b_linker.id - 1,
            vertical_side(v.reference.side),
            decision.candidate.x_ordinal,
            corner_ordinal,
            head_side(corner.reference.horizontal),
            vertical_side(corner.reference.vertical),
            c_alias(
                corner.reference.x_ordinal,
                corner.reference.horizontal,
                corner.reference.vertical,
            ),
            point(corner.point),
            corner.lookup_contains,
            corner.is_half_head,
            head_side(corner.imposed_void_head_side),
            optional_bool(corner.half_head_side_accepted),
            action,
        ));
    }
}

fn b_state_row(
    prefix: &str,
    page: &str,
    system_id: usize,
    stumps: &NativeStemsBeamStumpSystem,
    entry: &NativeStemsBeamArenaEntry,
    sides: impl IntoIterator<Item = NativeStemVerticalSide>,
) -> String {
    let mode = match &entry.origin {
        NativeStemsBeamArenaOrigin::Constructor(NativeStemsBeamBLinkerOrigin::Stump { .. }) => {
            "stump"
        }
        NativeStemsBeamArenaOrigin::Constructor(NativeStemsBeamBLinkerOrigin::Orphan {
            ..
        }) => "orphan",
        NativeStemsBeamArenaOrigin::Anchor { .. } => "anchor",
    };
    let sides = sides.into_iter().collect::<Vec<_>>();
    format!(
        "{prefix} {page} system {system_id} beamSig {} ordinal {} id0 {} alias {} mode {} \
         hSide {} ref {} isAnchor {} vSides {}",
        beam(stumps, entry.reference.beam).sig_ordinal,
        entry.reference.id - 1,
        entry.reference.id - 1,
        b_alias(stumps, entry.reference),
        mode,
        optional_head_side(entry.horizontal_side),
        point(entry.reference_point),
        entry.is_anchor,
        vertical_side_list(&sides),
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
    format!(
        "{prefix} {page}{system} systems {systems} beams {} smallBeams {} heads {} smallHeads {} \
         corners {} initialBs {} initialVs {} beamStarts {} bVisits {} anchorSkips {} \
         inspectedVs {} siblingScans {} siblingHits {} beamCandidates {} eligibleBeams {} \
         findCalls {} findCandidates {} bReuses {} anchorReuses {} anchorsCreated {} \
         headLookups {} headScans {} headAreaHits {} headCompetingPasses {} \
         headCompetingRemovals {} headCompetitorDrops {} smallHeadCandidates {} sizeDrops {} \
         headNearDrops {} cornerChecks {} cornerInside {} voidCornerDrops {} cAccepted {} \
         resultBs {} resultCs {} finalBs {} seedSnapshots {} builderChecks {} hash {hash:016x}",
        totals.beams,
        totals.small_beams,
        totals.heads,
        totals.small_heads,
        totals.corners,
        totals.initial_bs,
        totals.initial_vs,
        totals.beam_starts,
        totals.b_visits,
        totals.anchor_skips,
        totals.inspected_vs,
        totals.sibling_scans,
        totals.sibling_hits,
        totals.beam_candidates,
        totals.eligible_beams,
        totals.find_calls,
        totals.find_candidates,
        totals.b_reuses,
        totals.anchor_reuses,
        totals.anchors_created,
        totals.head_lookups,
        totals.head_scans,
        totals.head_area_hits,
        totals.head_competing_passes,
        totals.head_competing_removals,
        totals.head_competitor_drops,
        totals.small_head_candidates,
        totals.size_drops,
        totals.head_near_drops,
        totals.corner_checks,
        totals.corner_inside,
        totals.void_corner_drops,
        totals.c_accepted,
        totals.result_bs,
        totals.result_cs,
        totals.final_bs,
        totals.seed_snapshots,
        totals.builder_checks,
    )
}

fn assert_system_invariants(totals: Totals) {
    assert_eq!(totals.corners, 4 * totals.heads);
    assert_eq!(totals.beam_starts, totals.beams);
    assert_eq!(totals.b_visits, totals.initial_bs + totals.anchor_skips);
    assert!(totals.anchor_skips <= totals.anchors_created);
    assert_eq!(totals.inspected_vs, totals.initial_vs);
    assert_eq!(totals.head_lookups, totals.inspected_vs);
    assert_eq!(totals.seed_snapshots, totals.inspected_vs);
    assert_eq!(totals.sibling_hits, totals.beam_candidates);
    assert_eq!(totals.sibling_hits, totals.head_competing_passes);
    assert_eq!(totals.eligible_beams, totals.find_calls);
    assert_eq!(totals.eligible_beams, totals.result_bs);
    assert_eq!(totals.find_calls, totals.b_reuses + totals.anchors_created);
    assert!(totals.anchor_reuses <= totals.b_reuses);
    assert_eq!(totals.final_bs, totals.initial_bs + totals.anchors_created);
    assert_eq!(totals.head_competing_removals, totals.head_competitor_drops);
    assert_eq!(
        totals.small_beams + totals.small_heads + totals.small_head_candidates + totals.size_drops,
        0
    );
    assert_eq!(
        totals.corner_checks,
        2 * (totals.head_area_hits
            - totals.head_competitor_drops
            - totals.size_drops
            - totals.head_near_drops)
    );
    assert_eq!(
        totals.corner_inside,
        totals.void_corner_drops + totals.c_accepted
    );
    assert_eq!(totals.c_accepted, totals.result_cs);
    assert_eq!(
        totals.builder_checks,
        (3 * totals.initial_vs) + (2 * totals.corners)
    );
}

fn assert_known_totals(totals: Totals) {
    assert_eq!(totals.beams, 803);
    assert_eq!(totals.small_beams, 0);
    assert_eq!(totals.heads, 3_521);
    assert_eq!(totals.small_heads, 0);
    assert_eq!(totals.corners, 14_084);
    assert_eq!(totals.initial_bs, 2_116);
    assert_eq!(totals.initial_vs, 2_417);
    assert_eq!(totals.beam_starts, 803);
    assert_eq!(totals.b_visits, 2_145);
    assert_eq!(totals.anchor_skips, 29);
    assert_eq!(totals.inspected_vs, 2_417);
    assert_eq!(totals.sibling_scans, 4_960);
    assert_eq!(totals.sibling_hits, 4_514);
    assert_eq!(totals.beam_candidates, 4_514);
    assert_eq!(totals.eligible_beams, 1_617);
    assert_eq!(totals.find_calls, 1_617);
    assert_eq!(totals.find_candidates, 5_354);
    assert_eq!(totals.b_reuses, 1_472);
    assert_eq!(totals.anchor_reuses, 215);
    assert_eq!(totals.anchors_created, 145);
    assert_eq!(totals.head_lookups, 2_417);
    assert_eq!(totals.head_scans, 158_886);
    assert_eq!(totals.head_area_hits, 5_739);
    assert_eq!(totals.head_competing_passes, 4_514);
    assert_eq!(totals.head_competing_removals, 0);
    assert_eq!(totals.head_competitor_drops, 0);
    assert_eq!(totals.small_head_candidates, 0);
    assert_eq!(totals.size_drops, 0);
    assert_eq!(totals.head_near_drops, 46);
    assert_eq!(totals.corner_checks, 11_386);
    assert_eq!(totals.corner_inside, 5_590);
    assert_eq!(totals.void_corner_drops, 531);
    assert_eq!(totals.c_accepted, 5_059);
    assert_eq!(totals.result_bs, 1_617);
    assert_eq!(totals.result_cs, 5_059);
    assert_eq!(totals.final_bs, 2_261);
    assert_eq!(totals.seed_snapshots, 2_417);
    assert_eq!(totals.builder_checks, 35_419);
}

fn assert_recognition_totals(recognition: &NativeStemsBeamReachabilityRecognition, totals: Totals) {
    assert_eq!(recognition.beam_inspection_count, totals.beams);
    assert_eq!(recognition.b_visit_count, totals.b_visits);
    assert_eq!(recognition.v_inspection_count, totals.inspected_vs);
    assert_eq!(recognition.sibling_check_count, totals.sibling_scans);
    assert_eq!(recognition.sibling_hit_count, totals.sibling_hits);
    assert_eq!(recognition.find_linker_request_count, totals.find_calls);
    assert_eq!(recognition.reused_b_linker_count, totals.b_reuses);
    assert_eq!(recognition.reused_anchor_count, totals.anchor_reuses);
    assert_eq!(recognition.created_anchor_count, totals.anchors_created);
    assert_eq!(recognition.head_scan_count, totals.head_scans);
    assert_eq!(recognition.head_candidate_count, totals.head_area_hits);
    assert_eq!(recognition.head_corner_check_count, totals.corner_checks);
    assert_eq!(recognition.accepted_head_corner_count, totals.c_accepted);
    assert_eq!(recognition.small_head_count, totals.small_head_candidates);
    assert_eq!(recognition.small_beam_count, totals.small_beams);
    assert_eq!(recognition.size_rejection_count, totals.size_drops);
    assert_eq!(recognition.distance_rejection_count, totals.head_near_drops);
    assert_eq!(
        recognition.competing_removal_count,
        totals.head_competing_removals
    );
    assert_eq!(
        recognition.lookup_contained_corner_count,
        totals.corner_inside
    );
    assert_eq!(
        recognition.void_side_rejection_count,
        totals.void_corner_drops
    );
    assert_eq!(
        recognition.target_count,
        totals.result_bs + totals.result_cs
    );
    assert_eq!(recognition.beam_target_count, totals.result_bs);
    assert_eq!(recognition.head_target_count, totals.result_cs);
}

fn stump_system(
    recognition: &NativeStemsBeamStumpRecognition,
    system_id: usize,
) -> &NativeStemsBeamStumpSystem {
    recognition
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .expect("beam-stump system")
}

fn constructor_system(
    recognition: &NativeStemsBeamVLinkerRecognition,
    system_id: usize,
) -> &NativeStemsBeamVLinkerSystem {
    recognition
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .expect("BeamVLinker constructor system")
}

fn head_system(
    recognition: &NativeStemsHeadCornerRecognition,
    system_id: usize,
) -> &NativeStemsHeadCornerSystem {
    recognition
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .expect("head-corner system")
}

fn seed_system(
    recognition: &NativeStemSeedRecognition,
    system_id: usize,
) -> &NativeStemSeedSystemRecognition {
    recognition
        .systems
        .iter()
        .find(|system| system.raw.system_id == system_id)
        .expect("stem-seed system")
}

fn kept_system(
    recognition: &NativeStemsHeadSeedRecognition,
    system_id: usize,
) -> &NativeStemsHeadSeedSystem {
    recognition
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .expect("kept-seed system")
}

fn beam(
    system: &NativeStemsBeamStumpSystem,
    source: NativeStemsBeamSource,
) -> &NativeStemsBeamStumpBeam {
    system
        .beams_by_abscissa
        .iter()
        .find(|beam| beam.source == source)
        .expect("live beam source")
}

fn constructor(
    system: &NativeStemsBeamVLinkerSystem,
    source: NativeStemsBeamSource,
) -> &audiveris_omr::native_stems_beam_vlinkers::NativeStemsBeamVLinkerConstructor {
    system
        .constructors
        .iter()
        .find(|constructor| constructor.source == source && constructor.survives_constructor_loop)
        .expect("live BeamLinker constructor")
}

fn b_linker(
    constructor: &audiveris_omr::native_stems_beam_vlinkers::NativeStemsBeamVLinkerConstructor,
    id: usize,
) -> &NativeStemsBeamBLinker {
    constructor
        .b_linkers
        .get(id - 1)
        .filter(|b| b.reference.id == id)
        .expect("initial B linker")
}

fn v_linker(
    system: &NativeStemsBeamVLinkerSystem,
    reference: NativeStemsBeamVLinkerRef,
) -> &NativeStemsBeamVLinker {
    b_linker(
        constructor(system, reference.b_linker.beam),
        reference.b_linker.id,
    )
    .v_linkers
    .iter()
    .find(|v| v.reference == reference)
    .expect("constructor V linker")
}

fn arena(
    system: &NativeStemsBeamReachabilitySystem,
    source: NativeStemsBeamSource,
) -> &NativeStemsBeamArena {
    system
        .final_beam_arenas
        .iter()
        .find(|arena| arena.beam == source)
        .expect("beam arena")
}

fn arena_entry(
    system: &NativeStemsBeamReachabilitySystem,
    reference: NativeStemsBeamBLinkerRef,
) -> &NativeStemsBeamArenaEntry {
    arena(system, reference.beam)
        .all_b_linkers
        .get(reference.id - 1)
        .filter(|entry| entry.reference == reference)
        .expect("B arena entry")
}

fn b_snapshot(
    system: &NativeStemsBeamReachabilitySystem,
    stumps: &NativeStemsBeamStumpSystem,
    references: &[NativeStemsBeamBLinkerRef],
) -> String {
    list(
        &references
            .iter()
            .map(|&reference| {
                format!(
                    "{}:{}",
                    b_alias(stumps, reference),
                    if arena_entry(system, reference).is_anchor {
                        "A"
                    } else {
                        "I"
                    }
                )
            })
            .collect::<Vec<_>>(),
    )
}

fn b_alias(stumps: &NativeStemsBeamStumpSystem, reference: NativeStemsBeamBLinkerRef) -> String {
    format!(
        "beam:{}:b:{}",
        beam(stumps, reference.beam).sig_ordinal,
        reference.id - 1
    )
}

fn c_alias(
    head_x: usize,
    horizontal: NativeStemHeadSide,
    vertical: NativeStemVerticalSide,
) -> String {
    format!(
        "h:{head_x}:{}:{}",
        head_side(horizontal),
        vertical_side(vertical)
    )
}

fn source_sig_list(
    sources: &[NativeStemsBeamSource],
    stumps: &NativeStemsBeamStumpSystem,
) -> String {
    usize_list(
        &sources
            .iter()
            .map(|&source| beam(stumps, source).sig_ordinal)
            .collect::<Vec<_>>(),
    )
}

fn head_x_list(
    heads: &[audiveris_omr::native_stems_beam_reachability::NativeStemsBeamHeadCandidateRef],
) -> String {
    usize_list(&heads.iter().map(|head| head.x_ordinal).collect::<Vec<_>>())
}

fn beam_shape(kind: BeamKind) -> &'static str {
    match kind {
        BeamKind::Beam => "BEAM",
        BeamKind::Hook => "BEAM_HOOK",
        BeamKind::SmallBeam => "BEAM_SMALL",
    }
}

fn head_shape(shape: HeadTemplateShape) -> &'static str {
    match shape {
        HeadTemplateShape::NoteheadBlack => "NOTEHEAD_BLACK",
        HeadTemplateShape::NoteheadVoid => "NOTEHEAD_VOID",
        _ => panic!("unsupported inspectVLinkers head shape {shape:?}"),
    }
}

fn head_side(side: NativeStemHeadSide) -> &'static str {
    match side {
        NativeStemHeadSide::Left => "LEFT",
        NativeStemHeadSide::Right => "RIGHT",
    }
}

fn optional_head_side(side: Option<NativeStemHeadSide>) -> &'static str {
    side.map_or("-", head_side)
}

fn vertical_side(side: NativeStemVerticalSide) -> &'static str {
    match side {
        NativeStemVerticalSide::Top => "TOP",
        NativeStemVerticalSide::Bottom => "BOTTOM",
    }
}

fn vertical_side_list(sides: &[NativeStemVerticalSide]) -> String {
    list(
        &sides
            .iter()
            .map(|&side| vertical_side(side).to_owned())
            .collect::<Vec<_>>(),
    )
}

fn rectangle(value: JavaRectangle) -> String {
    format!("{}:{}:{}:{}", value.x, value.y, value.width, value.height)
}

fn double_bounds(value: NativeStemsBeamDoubleBounds) -> String {
    format!(
        "{}:{}:{}:{}",
        java_hex_double(value.x),
        java_hex_double(value.y),
        java_hex_double(value.width),
        java_hex_double(value.height),
    )
}

fn point(value: NativeStemPoint) -> String {
    format!("{}:{}", java_hex_double(value.x), java_hex_double(value.y))
}

fn segment(value: Segment) -> String {
    format!(
        "{}:{}:{}:{}",
        java_hex_double(value.x1),
        java_hex_double(value.y1),
        java_hex_double(value.x2),
        java_hex_double(value.y2),
    )
}

fn stem_line(value: NativeStemLine) -> String {
    format!("{}:{}", point(value.start), point(value.stop))
}

fn quadrilateral(value: [NativeStemPoint; 4]) -> String {
    value.map(point).join(":")
}

fn optional_i32(value: Option<i32>) -> String {
    value.map_or_else(|| "-".to_owned(), |value| value.to_string())
}

fn optional_bool(value: Option<bool>) -> String {
    value.map_or_else(|| "-".to_owned(), |value| value.to_string())
}

fn optional_double(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_owned(), java_hex_double)
}

fn list(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_owned()
    } else {
        values.join(",")
    }
}

fn usize_list(values: &[usize]) -> String {
    list(&values.iter().map(usize::to_string).collect::<Vec<_>>())
}

fn assert_corpus_summary(oracle: &str) {
    let summary = oracle
        .lines()
        .find(|line| line.starts_with("stemsbeamvinspectcorpussummary "))
        .expect("BeamVLinker-inspect corpus summary");
    for (field, expected) in [
        ("pages", "8"),
        ("systems", "30"),
        ("beams", "803"),
        ("smallBeams", "0"),
        ("heads", "3521"),
        ("smallHeads", "0"),
        ("corners", "14084"),
        ("initialBs", "2116"),
        ("initialVs", "2417"),
        ("beamStarts", "803"),
        ("bVisits", "2145"),
        ("anchorSkips", "29"),
        ("inspectedVs", "2417"),
        ("siblingScans", "4960"),
        ("siblingHits", "4514"),
        ("beamCandidates", "4514"),
        ("eligibleBeams", "1617"),
        ("findCalls", "1617"),
        ("findCandidates", "5354"),
        ("bReuses", "1472"),
        ("anchorReuses", "215"),
        ("anchorsCreated", "145"),
        ("headLookups", "2417"),
        ("headScans", "158886"),
        ("headAreaHits", "5739"),
        ("headCompetingPasses", "4514"),
        ("headCompetingRemovals", "0"),
        ("headCompetitorDrops", "0"),
        ("smallHeadCandidates", "0"),
        ("sizeDrops", "0"),
        ("headNearDrops", "46"),
        ("cornerChecks", "11386"),
        ("cornerInside", "5590"),
        ("voidCornerDrops", "531"),
        ("cAccepted", "5059"),
        ("resultBs", "1617"),
        ("resultCs", "5059"),
        ("finalBs", "2261"),
        ("seedSnapshots", "2417"),
        ("builderChecks", "35419"),
    ] {
        assert_eq!(field_value(summary, field), expected, "summary {field}");
    }
    if let Some(expected) = EXPECTED_PROBE_SHA256 {
        assert_eq!(field_value(summary, "probeSourceSha256"), expected);
        assert_eq!(
            sha256_hex(&std::fs::read(repo_path(PROBE_PATH)).expect("probe source")),
            expected
        );
    }
    if let Some(expected) = EXPECTED_RUNNER_SHA256 {
        assert_eq!(field_value(summary, "runnerSourceSha256"), expected);
        assert_eq!(
            sha256_hex(&std::fs::read(repo_path(RUNNER_PATH)).expect("oracle runner")),
            expected
        );
    }
    if let Some(expected) = EXPECTED_BODY_SHA256 {
        assert_eq!(field_value(summary, "emittedBodySha256"), expected);
    }
    if let Some(expected) = EXPECTED_FIXTURE_SHA256 {
        assert_eq!(sha256_hex(oracle.as_bytes()), expected);
    }
    if let Some(expected) = EXPECTED_FIXTURE_BYTES {
        assert_eq!(oracle.len(), expected);
    }
    if let Some(expected) = EXPECTED_FIXTURE_LINES {
        assert_eq!(oracle.lines().count(), expected);
    }
}

fn oracle_page_rows(oracle: &str, page: &str) -> Vec<String> {
    oracle
        .lines()
        .filter(|line| line.split_whitespace().nth(1) == Some(page))
        .filter(|line| {
            matches!(
                line.split_whitespace().next(),
                Some(
                    "stemsbeamvinspectpage"
                        | "stemsbeamvinspectsystem"
                        | "stemsbeamvinspectbeamorder"
                        | "stemsbeamvinspecthead"
                        | "stemsbeamvinspectcorner"
                        | "stemsbeamvinspectinitialb"
                        | "stemsbeamvinspectbeamstart"
                        | "stemsbeamvinspectbvisit"
                        | "stemsbeamvinspectv"
                        | "stemsbeamvinspectsibling"
                        | "stemsbeamvinspectbeamcandidate"
                        | "stemsbeamvinspectfind"
                        | "stemsbeamvinspectfindcandidate"
                        | "stemsbeamvinspectfindresult"
                        | "stemsbeamvinspectheadlookup"
                        | "stemsbeamvinspectheadscan"
                        | "stemsbeamvinspectheadcompeting"
                        | "stemsbeamvinspectheadcandidate"
                        | "stemsbeamvinspectcornercheck"
                        | "stemsbeamvinspectresult"
                        | "stemsbeamvinspectbeamend"
                        | "stemsbeamvinspectfinalb"
                        | "stemsbeamvinspectsystemsummary"
                        | "stemsbeamvinspectpagesummary"
                )
            )
        })
        .map(str::to_owned)
        .collect()
}

fn field_value<'a>(row: &'a str, name: &str) -> &'a str {
    let mut words = row.split_whitespace();
    while let Some(word) = words.next() {
        if word == name {
            return words
                .next()
                .unwrap_or_else(|| panic!("missing value for {name}"));
        }
    }
    panic!("missing {name} in {row}")
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
    let bit_len = u64::try_from(bytes.len()).expect("fixture length fits u64") * 8;
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

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}

fn append_find_scan_rows(
    page: &str,
    system: &NativeStemsBeamReachabilitySystem,
    stumps: &NativeStemsBeamStumpSystem,
    v: &NativeStemsBeamVInspection,
    find: &NativeStemsBeamFindLinker,
    totals: &mut Totals,
    rows: &mut Vec<String>,
) {
    totals.eligible_beams += 1;
    totals.find_calls += 1;
    totals.find_candidates += find.candidates.len();
    let source = beam(stumps, v.reference.b_linker.beam);
    let target = beam(stumps, find.target_beam);
    rows.push(format!(
        "stemsbeamvinspectfind {page} system {} callOrdinal {} sourceSig {} bOrdinal {} \
         vSide {} targetSig {} targetX {} theo {} targetMedian {} cross {} x0 {} maxDx {} \
         beforeCount {} beforeBs {}",
        system.system_id,
        find.request_ordinal,
        source.sig_ordinal,
        v.reference.b_linker.id - 1,
        vertical_side(v.reference.side),
        target.sig_ordinal,
        target.x_ordinal,
        stem_line(find.theoretical_line),
        segment(target.median),
        point(find.cross),
        java_hex_double(find.cross.x),
        find.max_beam_linker_dx,
        find.arena_count_before,
        list(
            &find
                .candidates
                .iter()
                .map(|candidate| {
                    format!(
                        "{}:{}",
                        b_alias(stumps, candidate.reference),
                        if arena_entry(system, candidate.reference).is_anchor {
                            "A"
                        } else {
                            "I"
                        }
                    )
                })
                .collect::<Vec<_>>(),
        ),
    ));
    for candidate in &find.candidates {
        rows.push(format!(
            "stemsbeamvinspectfindcandidate {page} system {} callOrdinal {} ordinal {} alias {} \
             ref {} dx {} bestBefore {} strictReplace {} bestAfter {} bestAliasAfter {}",
            system.system_id,
            find.request_ordinal,
            candidate.insertion_ordinal,
            b_alias(stumps, candidate.reference),
            point(candidate.reference_point),
            java_hex_double(candidate.dx),
            java_hex_double(candidate.best_dx_before),
            candidate.replaces_best,
            java_hex_double(candidate.best_dx_after),
            candidate
                .best_after
                .map_or_else(|| "-".to_owned(), |best| b_alias(stumps, best)),
        ));
    }
}

fn append_find_result_row(
    page: &str,
    system: &NativeStemsBeamReachabilitySystem,
    stumps: &NativeStemsBeamStumpSystem,
    v: &NativeStemsBeamVInspection,
    find: &NativeStemsBeamFindLinker,
    totals: &mut Totals,
    rows: &mut Vec<String>,
) {
    let source = beam(stumps, v.reference.b_linker.beam);
    let target = beam(stumps, find.target_beam);
    let result = find.result.reference();
    let result_entry = arena_entry(system, result);
    let action = match find.result {
        NativeStemsBeamFindResult::CreatedAnchor(_) => {
            totals.anchors_created += 1;
            "createAnchor"
        }
        NativeStemsBeamFindResult::Reused(_) if result_entry.is_anchor => {
            totals.b_reuses += 1;
            totals.anchor_reuses += 1;
            "reuseAnchor"
        }
        NativeStemsBeamFindResult::Reused(_) => {
            totals.b_reuses += 1;
            "reuseInitial"
        }
    };
    rows.push(format!(
        "stemsbeamvinspectfindresult {page} system {} callOrdinal {} sourceSig {} sourceB {} \
         vSide {} targetSig {} bestDx {} thresholdInclusive {} action {} result {} id0 {} \
         ref {} isAnchor {} beforeCount {} afterCount {}",
        system.system_id,
        find.request_ordinal,
        source.sig_ordinal,
        v.reference.b_linker.id - 1,
        vertical_side(v.reference.side),
        target.sig_ordinal,
        java_hex_double(find.best_dx),
        find.max_beam_linker_dx,
        action,
        b_alias(stumps, result),
        result.id - 1,
        point(result_entry.reference_point),
        result_entry.is_anchor,
        find.arena_count_before,
        find.arena_count_after,
    ));
}
