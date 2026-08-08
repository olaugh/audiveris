// SPDX-License-Identifier: AGPL-3.0-or-later

//! Eight-page Java differential for HEADS seed lookup and glyph materialization.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use audiveris_omr::{
    head_template::{HeadTemplateBounds, HeadTemplateShape, MIN_INTER_GRADE, impact_of},
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads_prolog,
    native_heads_bar_slices::materialize_native_head_scanner_bar_slices,
    native_heads_competitor_slices::materialize_native_head_scanner_competitor_slices,
    native_heads_competitors::materialize_native_heads_competitors,
    native_heads_obstacles::materialize_native_heads_bar_obstacles,
    native_heads_scanner::recognize_native_heads_scanner_context,
    native_heads_scanner_pools::materialize_native_head_scanner_pools,
    native_heads_seed_glyphs::{
        NativeHeadSeedGlyph, NativeHeadsSeedGlyphsInput, retrieve_native_heads_seed_glyphs,
    },
    native_heads_seed_lookup::{
        NativeHeadSeedCandidate, NativeHeadSeedLookupScan, NativeHeadSeedPerformance,
        NativeHeadSeedSearchOutcome, NativeHeadSeedShapeSearch, NativeHeadSeedSide,
        NativeHeadsSeedLookupInput, recognize_native_heads_seed_lookup,
    },
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::recognize_native_stem_seeds,
    recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds},
};

const ORACLE: &str = include_str!("../../../oracle/heads-seed-pass.txt");

#[derive(Debug)]
struct ExpectedCorpus {
    pages: BTreeMap<String, ExpectedPage>,
    summary: CorpusSummary,
}

#[derive(Debug, Default)]
struct ExpectedPage {
    declared_systems: usize,
    declared_staffs: usize,
    systems: BTreeMap<usize, ExpectedSystem>,
    summary: Option<PageSummary>,
}

#[derive(Debug, Default)]
struct ExpectedSystem {
    staffs: BTreeMap<usize, ExpectedStaff>,
    summary: Option<SystemSummary>,
}

#[derive(Debug, Default)]
struct ExpectedStaff {
    summary: Option<StaffSummary>,
    candidates: Vec<ExpectedCandidate>,
    heads: Vec<ExpectedHead>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ExpectedPerformance {
    bars: usize,
    competitors: usize,
    evaluations: usize,
    abandons: usize,
}

impl ExpectedPerformance {
    fn include(&mut self, other: Self) {
        self.bars += other.bars;
        self.competitors += other.competitors;
        self.evaluations += other.evaluations;
        self.abandons += other.abandons;
    }
}

#[derive(Debug)]
struct StaffSummary {
    scanners: usize,
    seeds: usize,
    searches: usize,
    candidates: usize,
    heads: usize,
    outcomes: [usize; 7],
    performance: ExpectedPerformance,
    kernel_hash: u64,
}

#[derive(Debug)]
struct SystemSummary {
    staffs: usize,
    searches: usize,
    candidates: usize,
    performance: ExpectedPerformance,
}

#[derive(Debug)]
struct PageSummary {
    staffs: usize,
    searches: usize,
    candidates: usize,
}

#[derive(Debug)]
struct CorpusSummary {
    pages: usize,
    systems: usize,
    staffs: usize,
    searches: usize,
    candidates: usize,
    heads: usize,
}

#[derive(Debug)]
struct ExpectedCandidate {
    ordinal: usize,
    search_ordinal: usize,
    scanner_ordinal: usize,
    seed_pool_index: usize,
    side: String,
    shape_ordinal: usize,
    original_shape: String,
    final_shape: String,
    selected: [i32; 6],
    distance_bits: u64,
    black_to_void: Option<bool>,
    pitch_bits: u64,
    bounds: [i32; 4],
    grade_bits: u64,
    distance_impact_bits: u64,
    impact_weight_bits: u64,
    outcome: String,
}

#[derive(Debug)]
struct ExpectedHead {
    ordinal: usize,
    candidate_ordinal: usize,
    search_ordinal: usize,
    scanner_ordinal: usize,
    seed_pool_index: usize,
    side: String,
    shape_ordinal: usize,
    original_shape: String,
    final_shape: String,
    selected: [i32; 6],
    distance_bits: u64,
    black_to_void: Option<bool>,
    provisional_bounds: [i32; 4],
    shape: String,
    pitch_bits: u64,
    bounds: [i32; 4],
    grade_bits: u64,
    distance_impact_bits: u64,
    impact_weight_bits: u64,
    good: bool,
    glyph_bounds: [i32; 4],
    glyph_weight: usize,
    glyph_run_digest: u64,
    tally_left_bits: Option<u64>,
    tally_right_bits: Option<u64>,
}

#[test]
fn native_heads_seed_lookup_and_final_glyphs_match_java_corpus() {
    let expected = parse_oracle();
    assert_eq!(expected.pages.len(), expected.summary.pages);

    let mut actual_systems = 0_usize;
    let mut actual_staffs = 0_usize;
    let mut actual_searches = 0_usize;
    let mut actual_candidates = 0_usize;
    let mut actual_heads = 0_usize;
    let mut actual_performance = ExpectedPerformance::default();
    let mut expected_performance = ExpectedPerformance::default();

    for (page_key, expected_page) in &expected.pages {
        let image = page_key
            .strip_suffix("#1")
            .unwrap_or_else(|| panic!("unexpected page key {page_key}"));
        let grid = recognize_grid_lines(repo_path(&format!("data/examples/{image}")))
            .unwrap_or_else(|error| panic!("{page_key}: GRID failed: {error}"));
        let obstacles = materialize_native_heads_bar_obstacles(&grid)
            .unwrap_or_else(|error| panic!("{page_key}: obstacles failed: {error}"));
        let headers = recognize_native_headers(&grid)
            .unwrap_or_else(|error| panic!("{page_key}: HEADERS failed: {error}"));
        let stem_seeds = recognize_native_stem_seeds(&grid, &headers)
            .unwrap_or_else(|error| panic!("{page_key}: STEM_SEEDS failed: {error}"));
        let beams =
            recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
                .unwrap_or_else(|error| panic!("{page_key}: BEAMS failed: {error}"));
        let competitors = materialize_native_heads_competitors(&grid, &beams)
            .unwrap_or_else(|error| panic!("{page_key}: competitors failed: {error}"));
        let ledgers = recognize_native_ledgers(&grid, &beams)
            .unwrap_or_else(|error| panic!("{page_key}: LEDGERS failed: {error}"));
        let heads = recognize_native_heads_prolog(&grid, &beams, &ledgers, &stem_seeds)
            .unwrap_or_else(|error| panic!("{page_key}: HEADS prolog failed: {error}"));
        let scanners =
            recognize_native_heads_scanner_context(&grid, &headers, &ledgers, &stem_seeds, &heads)
                .unwrap_or_else(|error| panic!("{page_key}: scanner context failed: {error}"));
        let pools = materialize_native_head_scanner_pools(&stem_seeds, &heads, &scanners)
            .unwrap_or_else(|error| panic!("{page_key}: scanner pools failed: {error}"));
        let bar_slices = materialize_native_head_scanner_bar_slices(&pools, &obstacles)
            .unwrap_or_else(|error| panic!("{page_key}: bar slices failed: {error}"));
        let competitor_slices =
            materialize_native_head_scanner_competitor_slices(&pools, &competitors)
                .unwrap_or_else(|error| panic!("{page_key}: competitor slices failed: {error}"));
        let actual = recognize_native_heads_seed_lookup(NativeHeadsSeedLookupInput {
            heads: &heads,
            scanners: &scanners,
            pools: &pools,
            obstacles: &obstacles,
            bar_slices: &bar_slices,
            competitors: &competitors,
            competitor_slices: &competitor_slices,
            stem_seeds: &stem_seeds,
        })
        .unwrap_or_else(|error| panic!("{page_key}: seed lookup failed: {error}"));
        let glyphs = retrieve_native_heads_seed_glyphs(NativeHeadsSeedGlyphsInput {
            grid: &grid,
            heads: &heads,
            lookup: &actual,
        })
        .unwrap_or_else(|error| panic!("{page_key}: seed glyph retrieval failed: {error}"));

        assert_eq!(
            actual.systems.len(),
            expected_page.declared_systems,
            "{page_key} system count"
        );
        assert_eq!(
            expected_page.systems.len(),
            expected_page.declared_systems,
            "{page_key} oracle system summaries"
        );
        assert_eq!(glyphs.systems.len(), actual.systems.len(), "{page_key}");

        let mut page_staffs = 0_usize;
        let mut page_searches = 0_usize;
        let mut page_candidates = 0_usize;
        let mut page_heads = 0_usize;
        for (system_ordinal, actual_system) in actual.systems.iter().enumerate() {
            let system_label = format!("{page_key} system {}", actual_system.system_id);
            let expected_system = expected_page
                .systems
                .get(&actual_system.system_id)
                .unwrap_or_else(|| panic!("{system_label} absent from oracle"));
            let summary = expected_system
                .summary
                .as_ref()
                .unwrap_or_else(|| panic!("{system_label} has no summary"));
            assert_eq!(actual_system.staffs.len(), summary.staffs, "{system_label}");
            let glyph_system = &glyphs.systems[system_ordinal];
            assert_eq!(
                glyph_system.system_id, actual_system.system_id,
                "{system_label}"
            );
            assert_eq!(
                glyph_system.staffs.len(),
                actual_system.staffs.len(),
                "{system_label}"
            );

            let pool_system = &pools.systems[system_ordinal];
            assert_eq!(
                pool_system.system_id, actual_system.system_id,
                "{system_label}"
            );
            let mut system_searches = 0_usize;
            let mut system_candidates = 0_usize;
            let mut system_heads = 0_usize;
            for (staff_ordinal, actual_staff) in actual_system.staffs.iter().enumerate() {
                let staff_label = format!("{system_label} staff {}", actual_staff.staff_id);
                let expected_staff = expected_system
                    .staffs
                    .get(&actual_staff.staff_id)
                    .unwrap_or_else(|| panic!("{staff_label} absent from oracle"));
                let staff_summary = expected_staff
                    .summary
                    .as_ref()
                    .unwrap_or_else(|| panic!("{staff_label} has no summary"));
                let glyph_staff = &glyph_system.staffs[staff_ordinal];
                assert_eq!(glyph_staff.staff_id, actual_staff.staff_id, "{staff_label}");
                let searches = actual_staff
                    .scans
                    .iter()
                    .map(|scan| scan.searches.len())
                    .sum::<usize>();
                let candidates = actual_staff
                    .scans
                    .iter()
                    .map(|scan| scan.candidates.len())
                    .sum::<usize>();

                assert_eq!(
                    actual_staff.scans.len(),
                    staff_summary.scanners,
                    "{staff_label}"
                );
                assert_eq!(
                    pool_system.seeds.len(),
                    staff_summary.seeds,
                    "{staff_label}"
                );
                assert_eq!(searches, staff_summary.searches, "{staff_label}");
                assert_eq!(candidates, staff_summary.candidates, "{staff_label}");
                assert_eq!(candidates, staff_summary.heads, "{staff_label}");
                assert_eq!(
                    candidates,
                    expected_staff.candidates.len(),
                    "{staff_label} candidate oracle rows"
                );
                assert_eq!(
                    outcome_counts(actual_staff),
                    staff_summary.outcomes,
                    "{staff_label} outcomes"
                );
                assert_performance(
                    actual_staff.performance,
                    staff_summary.performance,
                    &staff_label,
                );
                assert_eq!(
                    actual_staff.kernel_hash, staff_summary.kernel_hash,
                    "{staff_label} kernel hash"
                );
                assert_candidates(actual_staff, &expected_staff.candidates, &staff_label);
                assert_eq!(
                    glyph_staff.candidate_count, candidates,
                    "{staff_label} glyph candidate count"
                );
                assert_eq!(glyph_staff.dropped_count, 0, "{staff_label} dropped glyphs");
                assert_eq!(
                    glyph_staff.heads.len(),
                    expected_staff.heads.len(),
                    "{staff_label} final head rows"
                );
                assert_eq!(
                    glyph_staff.heads.len(),
                    candidates,
                    "{staff_label} survivors"
                );
                assert_final_heads(
                    &glyph_staff.heads,
                    &expected_staff.heads,
                    &expected_staff.candidates,
                    &staff_label,
                );
                expected_performance.include(staff_summary.performance);

                system_searches += searches;
                system_candidates += candidates;
                system_heads += glyph_staff.heads.len();
                page_staffs += 1;
            }
            assert_eq!(system_searches, summary.searches, "{system_label}");
            assert_eq!(system_candidates, summary.candidates, "{system_label}");
            assert_eq!(
                glyph_system.candidate_count, system_candidates,
                "{system_label}"
            );
            assert_eq!(glyph_system.dropped_count, 0, "{system_label}");
            assert_eq!(glyph_system.head_count, system_heads, "{system_label}");
            assert_eq!(system_heads, summary.candidates, "{system_label}");
            assert_performance(
                actual_system.performance,
                summary.performance,
                &system_label,
            );

            page_searches += system_searches;
            page_candidates += system_candidates;
            page_heads += system_heads;
            actual_systems += 1;
        }

        let page_summary = expected_page
            .summary
            .as_ref()
            .unwrap_or_else(|| panic!("{page_key} has no page summary"));
        assert_eq!(page_staffs, expected_page.declared_staffs, "{page_key}");
        assert_eq!(page_staffs, page_summary.staffs, "{page_key}");
        assert_eq!(page_searches, page_summary.searches, "{page_key}");
        assert_eq!(page_candidates, page_summary.candidates, "{page_key}");
        assert_eq!(page_heads, page_summary.candidates, "{page_key}");
        assert_eq!(glyphs.candidate_count, page_candidates, "{page_key}");
        assert_eq!(glyphs.dropped_count, 0, "{page_key}");
        assert_eq!(glyphs.head_count, page_heads, "{page_key}");
        actual_staffs += page_staffs;
        actual_searches += page_searches;
        actual_candidates += page_candidates;
        actual_heads += page_heads;
        actual_performance.include(performance(actual.performance));
    }

    assert_eq!(actual_systems, expected.summary.systems);
    assert_eq!(actual_staffs, expected.summary.staffs);
    assert_eq!(actual_searches, expected.summary.searches);
    assert_eq!(actual_candidates, expected.summary.candidates);
    assert_eq!(actual_candidates, expected.summary.heads);
    assert_eq!(actual_heads, expected.summary.heads);
    assert_eq!(actual_heads, actual_candidates);
    assert_eq!(actual_performance, expected_performance);
}

fn assert_candidates(
    staff: &audiveris_omr::native_heads_seed_lookup::NativeHeadSeedLookupStaff,
    expected: &[ExpectedCandidate],
    staff_label: &str,
) {
    let searches = staff
        .scans
        .iter()
        .flat_map(|scan| &scan.searches)
        .collect::<Vec<_>>();
    for (ordinal, search) in searches.iter().enumerate() {
        assert_eq!(search.ordinal, ordinal, "{staff_label} search order");
    }

    let mut ordinal = 0_usize;
    for scan in &staff.scans {
        for candidate in &scan.candidates {
            let expected = &expected[ordinal];
            let label = format!("{staff_label} candidate {ordinal}");
            assert_candidate(scan, candidate, &searches, expected, &label);
            ordinal += 1;
        }
    }
    assert_eq!(ordinal, expected.len(), "{staff_label} candidate count");
}

fn assert_candidate(
    scan: &NativeHeadSeedLookupScan,
    actual: &NativeHeadSeedCandidate,
    searches: &[&NativeHeadSeedShapeSearch],
    expected: &ExpectedCandidate,
    label: &str,
) {
    assert_eq!(actual.ordinal, expected.ordinal, "{label} ordinal");
    assert_eq!(
        actual.search_ordinal, expected.search_ordinal,
        "{label} search"
    );
    assert_eq!(
        scan.geometry_ordinal, expected.scanner_ordinal,
        "{label} scanner"
    );
    assert_eq!(
        actual.seed_pool_index, expected.seed_pool_index,
        "{label} seed"
    );
    assert_eq!(side_name(actual.side), expected.side, "{label} side");
    assert_eq!(actual.anchor, actual.side.anchor(), "{label} anchor");
    assert_eq!(
        shape_name(actual.original_shape),
        expected.original_shape,
        "{label} original shape"
    );
    assert_eq!(
        shape_name(actual.shape),
        expected.final_shape,
        "{label} final shape"
    );

    let search = searches
        .get(actual.search_ordinal)
        .copied()
        .unwrap_or_else(|| panic!("{label} missing search"));
    assert_eq!(
        search.ordinal, actual.search_ordinal,
        "{label} search ordinal"
    );
    assert_eq!(
        search.seed_pool_index, actual.seed_pool_index,
        "{label} search seed"
    );
    assert_eq!(
        search.seed_source_ordinal, actual.seed_source_ordinal,
        "{label} seed source"
    );
    assert_eq!(search.side, actual.side, "{label} search side");
    assert_eq!(search.anchor, actual.anchor, "{label} search anchor");
    assert_eq!(
        search.shape_ordinal, expected.shape_ordinal,
        "{label} shape ordinal"
    );
    assert_eq!(
        search.original_shape, actual.original_shape,
        "{label} search original"
    );
    assert_eq!(
        search.final_shape, actual.shape,
        "{label} search final shape"
    );

    let best = search
        .best
        .unwrap_or_else(|| panic!("{label} has no best location"));
    let best_attempt = search
        .attempts
        .get(best.attempt_ordinal)
        .unwrap_or_else(|| panic!("{label} missing selected attempt"));
    let selected = [
        best_attempt.y_offset_ordinal as i32,
        best_attempt.y_offset,
        best_attempt.x_offset_ordinal as i32,
        best_attempt.x_offset,
        best.x,
        best.y,
    ];
    assert_eq!(selected, expected.selected, "{label} selected location");
    assert!(best_attempt.selected_as_best, "{label} selected marker");
    assert_eq!((actual.x, actual.y), (best.x, best.y), "{label} pivot");
    assert_bits(
        best.distance,
        expected.distance_bits,
        label,
        "best distance",
    );
    assert_bits(actual.distance, expected.distance_bits, label, "distance");

    match expected.black_to_void {
        None => assert!(search.hole.is_none(), "{label} unexpected hole evaluation"),
        Some(converted) => {
            let hole = search
                .hole
                .unwrap_or_else(|| panic!("{label} missing hole evaluation"));
            assert_eq!(hole.converted_to_void, converted, "{label} black-to-void");
            assert_eq!(
                hole.threshold.to_bits(),
                0.2_f64.to_bits(),
                "{label} hole threshold"
            );
        }
    }

    assert_bits(actual.pitch, expected.pitch_bits, label, "pitch");
    assert_eq!(bounds(actual.bounds), expected.bounds, "{label} bounds");
    assert_eq!(
        search.slim_bounds,
        Some(actual.bounds),
        "{label} search bounds"
    );
    assert_bits(actual.grade, expected.grade_bits, label, "grade");
    assert_eq!(
        search.grade.map(f64::to_bits),
        Some(expected.grade_bits),
        "{label} search grade"
    );
    assert_bits(
        actual.distance_impact,
        expected.distance_impact_bits,
        label,
        "distance impact",
    );
    assert_eq!(
        expected.impact_weight_bits,
        1.0_f64.to_bits(),
        "{label} impact weight"
    );
    let raw_impact = impact_of(actual.distance);
    assert_bits(
        actual.raw_distance_impact,
        raw_impact.to_bits(),
        label,
        "raw distance impact",
    );
    assert_eq!(
        search.raw_distance_impact.map(f64::to_bits),
        Some(raw_impact.to_bits()),
        "{label} search raw impact"
    );
    assert_eq!(
        actual.minimum_grade.to_bits(),
        MIN_INTER_GRADE.to_bits(),
        "{label}"
    );
    let good = actual.grade >= 0.4;
    assert_eq!(actual.good_for_tally, good, "{label} good-for-tally");
    assert_eq!(
        actual.tally_dx.map(f64::to_bits),
        expected_tally_dx(actual, search).map(f64::to_bits),
        "{label} tally dx"
    );
    assert_eq!(expected.outcome, "final", "{label} Java glyph prediction");
    assert!(
        matches!(
            search.outcome,
            NativeHeadSeedSearchOutcome::ProvisionalCandidate { candidate_ordinal }
                if candidate_ordinal == actual.ordinal
        ),
        "{label} provisional outcome"
    );
}

fn assert_final_heads(
    actual: &[NativeHeadSeedGlyph],
    expected: &[ExpectedHead],
    candidates: &[ExpectedCandidate],
    staff_label: &str,
) {
    assert_eq!(actual.len(), expected.len(), "{staff_label} final heads");
    for (ordinal, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        let label = format!("{staff_label} final head {ordinal}");
        assert_eq!(actual.ordinal, ordinal, "{label} dense order");
        assert_eq!(actual.ordinal, expected.ordinal, "{label} oracle order");
        assert_eq!(
            actual.candidate_ordinal, expected.candidate_ordinal,
            "{label} candidate"
        );
        assert_eq!(actual.candidate_ordinal, ordinal, "{label} survivor order");
        assert_eq!(
            actual.search_ordinal, expected.search_ordinal,
            "{label} search"
        );
        assert_eq!(
            actual.geometry_ordinal, expected.scanner_ordinal,
            "{label} scanner"
        );
        assert_eq!(
            actual.seed_pool_index, expected.seed_pool_index,
            "{label} seed"
        );
        assert_eq!(side_name(actual.side), expected.side, "{label} side");
        assert_eq!(actual.anchor, actual.side.anchor(), "{label} anchor");
        assert_eq!(
            shape_name(actual.original_shape),
            expected.original_shape,
            "{label} original shape"
        );
        assert_eq!(
            shape_name(actual.shape),
            expected.final_shape,
            "{label} final shape"
        );
        assert_eq!(shape_name(actual.shape), expected.shape, "{label} shape");
        assert_eq!(
            [actual.x, actual.y],
            expected.selected[4..],
            "{label} pivot"
        );
        assert_eq!(
            actual.distance_bits, expected.distance_bits,
            "{label} distance"
        );
        assert_eq!(actual.pitch_bits, expected.pitch_bits, "{label} pitch");
        assert_eq!(
            bounds(actual.provisional_bounds),
            expected.provisional_bounds,
            "{label} provisional bounds"
        );
        assert_eq!(
            bounds(actual.bounds),
            expected.bounds,
            "{label} inter bounds"
        );
        assert_eq!(actual.grade_bits, expected.grade_bits, "{label} grade");
        assert_eq!(
            actual.distance_impact_bits, expected.distance_impact_bits,
            "{label} distance impact"
        );
        assert_eq!(
            actual.raw_distance_impact_bits,
            impact_of(f64::from_bits(expected.distance_bits)).to_bits(),
            "{label} raw distance impact"
        );
        assert_eq!(
            expected.impact_weight_bits,
            1.0_f64.to_bits(),
            "{label} impact weight"
        );
        assert_eq!(
            actual.minimum_grade_bits,
            MIN_INTER_GRADE.to_bits(),
            "{label} minimum grade"
        );
        assert_eq!(actual.good, expected.good, "{label} good");
        assert_eq!(
            bounds(actual.glyph.glyph_bounds),
            expected.glyph_bounds,
            "{label} glyph bounds"
        );
        assert_eq!(
            actual.glyph.new_inter_bounds, actual.bounds,
            "{label} updated inter bounds"
        );
        assert_eq!(
            actual.glyph.glyph_bounds, actual.bounds,
            "{label} positioned glyph bounds"
        );
        assert_eq!(
            actual.glyph.weight, expected.glyph_weight,
            "{label} glyph weight"
        );
        assert_eq!(
            actual.glyph.run_table.weight(),
            expected.glyph_weight,
            "{label} run-table weight"
        );
        assert_eq!(
            actual.glyph.run_digest, expected.glyph_run_digest,
            "{label} glyph run digest"
        );
        assert_eq!(
            actual.tally_left_bits, expected.tally_left_bits,
            "{label} left tally"
        );
        assert_eq!(
            actual.tally_right_bits, expected.tally_right_bits,
            "{label} right tally"
        );
        match (actual.good, actual.side) {
            (false, _) => {
                assert!(actual.tally_left_bits.is_none(), "{label} bad left tally");
                assert!(actual.tally_right_bits.is_none(), "{label} bad right tally");
            }
            (true, NativeHeadSeedSide::Left) => {
                assert!(actual.tally_left_bits.is_some(), "{label} left tally");
                assert!(actual.tally_right_bits.is_none(), "{label} right tally");
            }
            (true, NativeHeadSeedSide::Right) => {
                assert!(actual.tally_left_bits.is_none(), "{label} left tally");
                assert!(actual.tally_right_bits.is_some(), "{label} right tally");
            }
        }

        let candidate = candidates
            .get(actual.candidate_ordinal)
            .unwrap_or_else(|| panic!("{label} missing provisional candidate"));
        assert_eq!(candidate.search_ordinal, expected.search_ordinal, "{label}");
        assert_eq!(
            candidate.scanner_ordinal, expected.scanner_ordinal,
            "{label}"
        );
        assert_eq!(
            candidate.seed_pool_index, expected.seed_pool_index,
            "{label}"
        );
        assert_eq!(candidate.side, expected.side, "{label}");
        assert_eq!(candidate.shape_ordinal, expected.shape_ordinal, "{label}");
        assert_eq!(candidate.original_shape, expected.original_shape, "{label}");
        assert_eq!(candidate.final_shape, expected.final_shape, "{label}");
        assert_eq!(candidate.selected, expected.selected, "{label}");
        assert_eq!(candidate.distance_bits, expected.distance_bits, "{label}");
        assert_eq!(candidate.black_to_void, expected.black_to_void, "{label}");
        assert_eq!(candidate.bounds, expected.provisional_bounds, "{label}");
        assert_eq!(candidate.pitch_bits, expected.pitch_bits, "{label}");
        assert_eq!(candidate.grade_bits, expected.grade_bits, "{label}");
        assert_eq!(
            candidate.distance_impact_bits, expected.distance_impact_bits,
            "{label}"
        );
        assert_eq!(
            candidate.impact_weight_bits, expected.impact_weight_bits,
            "{label}"
        );
    }
}

fn expected_tally_dx(
    candidate: &NativeHeadSeedCandidate,
    search: &NativeHeadSeedShapeSearch,
) -> Option<f64> {
    candidate.good_for_tally.then(|| match candidate.side {
        NativeHeadSeedSide::Left => {
            f64::from(candidate.bounds.x.wrapping_sub(search.axis.precise_x)) + 0.5
        }
        NativeHeadSeedSide::Right => {
            let inclusive_right = candidate
                .bounds
                .x
                .wrapping_add(candidate.bounds.width)
                .wrapping_sub(1);
            f64::from(search.axis.precise_x) + 0.5 - f64::from(inclusive_right)
        }
    })
}

fn assert_bits(actual: f64, expected: u64, label: &str, field: &str) {
    assert_eq!(actual.to_bits(), expected, "{label} {field}");
}

fn bounds(bounds: HeadTemplateBounds) -> [i32; 4] {
    [bounds.x, bounds.y, bounds.width, bounds.height]
}

fn side_name(side: NativeHeadSeedSide) -> &'static str {
    match side {
        NativeHeadSeedSide::Left => "LEFT_STEM",
        NativeHeadSeedSide::Right => "RIGHT_STEM",
    }
}

fn shape_name(shape: HeadTemplateShape) -> &'static str {
    match shape {
        HeadTemplateShape::NoteheadBlack => "NOTEHEAD_BLACK",
        HeadTemplateShape::NoteheadVoid => "NOTEHEAD_VOID",
        HeadTemplateShape::WholeNote => "WHOLE_NOTE",
        HeadTemplateShape::Breve => "BREVE",
    }
}

fn outcome_counts(
    staff: &audiveris_omr::native_heads_seed_lookup::NativeHeadSeedLookupStaff,
) -> [usize; 7] {
    use audiveris_omr::native_heads_seed_lookup::NativeHeadSeedNominalAbandonReason;

    let mut counts = [0_usize; 7];
    for search in staff.scans.iter().flat_map(|scan| &scan.searches) {
        let index = match search.outcome {
            NativeHeadSeedSearchOutcome::AbandonedAtNominal {
                reason: NativeHeadSeedNominalAbandonReason::Bar { .. },
            } => 0,
            NativeHeadSeedSearchOutcome::AbandonedAtNominal {
                reason: NativeHeadSeedNominalAbandonReason::Competitor { .. },
            } => 1,
            NativeHeadSeedSearchOutcome::AbandonedAtNominal {
                reason: NativeHeadSeedNominalAbandonReason::Distance { .. },
            } => 2,
            NativeHeadSeedSearchOutcome::NoAcceptableLocation => 3,
            NativeHeadSeedSearchOutcome::BelowMinimumGrade => 4,
            NativeHeadSeedSearchOutcome::ProvisionalCandidate { .. } => 6,
        };
        counts[index] += 1;
    }
    counts
}

fn assert_performance(
    actual: NativeHeadSeedPerformance,
    expected: ExpectedPerformance,
    label: &str,
) {
    assert_eq!(performance(actual), expected, "{label} performance");
}

fn performance(actual: NativeHeadSeedPerformance) -> ExpectedPerformance {
    ExpectedPerformance {
        bars: actual.bar_blocks,
        competitors: actual.competitor_blocks,
        evaluations: actual.evaluations,
        abandons: actual.nominal_abandons,
    }
}

fn parse_oracle() -> ExpectedCorpus {
    let mut pages = BTreeMap::<String, ExpectedPage>::new();
    let mut corpus_summary = None;
    for (line_index, line) in ORACLE.lines().enumerate() {
        let line_number = line_index + 1;
        let fields = line.split_whitespace().collect::<Vec<_>>();
        match fields.first().copied() {
            Some("headseedpage") => {
                let key = fields[1].to_owned();
                let previous = pages.insert(
                    key,
                    ExpectedPage {
                        declared_systems: parse_field(&fields, "systems", line_number),
                        declared_staffs: parse_field(&fields, "staves", line_number),
                        ..ExpectedPage::default()
                    },
                );
                assert!(previous.is_none(), "duplicate page at line {line_number}");
            }
            Some("headseedcandidate") => {
                let page = page_mut(&mut pages, &fields, line_number);
                let system_id = parse_field(&fields, "system", line_number);
                let staff_id = parse_field(&fields, "staff", line_number);
                let (distance_impact_bits, impact_weight_bits) =
                    parse_impact(field(&fields, "impacts", line_number));
                let candidate = ExpectedCandidate {
                    ordinal: parse_field(&fields, "ordinal", line_number),
                    search_ordinal: parse_field(&fields, "attempt", line_number),
                    scanner_ordinal: parse_field(&fields, "scanner", line_number),
                    seed_pool_index: parse_field(&fields, "seed", line_number),
                    side: field(&fields, "side", line_number).to_owned(),
                    shape_ordinal: parse_field(&fields, "shapeOrdinal", line_number),
                    original_shape: field(&fields, "original", line_number).to_owned(),
                    final_shape: field(&fields, "finalShape", line_number).to_owned(),
                    selected: parse_i32_array(field(&fields, "selected", line_number)),
                    distance_bits: parse_double_bits(field(&fields, "distance", line_number)),
                    black_to_void: parse_optional_bool(field(&fields, "blackToVoid", line_number)),
                    pitch_bits: parse_double_bits(field(&fields, "pitch", line_number)),
                    bounds: parse_i32_array(field(&fields, "slim", line_number)),
                    grade_bits: parse_double_bits(field(&fields, "grade", line_number)),
                    distance_impact_bits,
                    impact_weight_bits,
                    outcome: field(&fields, "outcome", line_number).to_owned(),
                };
                let candidates = &mut page
                    .systems
                    .entry(system_id)
                    .or_default()
                    .staffs
                    .entry(staff_id)
                    .or_default()
                    .candidates;
                assert_eq!(
                    candidate.ordinal,
                    candidates.len(),
                    "candidate order at line {line_number}"
                );
                candidates.push(candidate);
            }
            Some("headseedhead") => {
                let page = page_mut(&mut pages, &fields, line_number);
                let system_id = parse_field(&fields, "system", line_number);
                let staff_id = parse_field(&fields, "staff", line_number);
                let (distance_impact_bits, impact_weight_bits) =
                    parse_impact(field(&fields, "impacts", line_number));
                let head = ExpectedHead {
                    ordinal: parse_field(&fields, "ordinal", line_number),
                    candidate_ordinal: parse_field(&fields, "candidate", line_number),
                    search_ordinal: parse_field(&fields, "attempt", line_number),
                    scanner_ordinal: parse_field(&fields, "scanner", line_number),
                    seed_pool_index: parse_field(&fields, "seed", line_number),
                    side: field(&fields, "side", line_number).to_owned(),
                    shape_ordinal: parse_field(&fields, "shapeOrdinal", line_number),
                    original_shape: field(&fields, "original", line_number).to_owned(),
                    final_shape: field(&fields, "finalShape", line_number).to_owned(),
                    selected: parse_i32_array(field(&fields, "selected", line_number)),
                    distance_bits: parse_double_bits(field(&fields, "distance", line_number)),
                    black_to_void: parse_optional_bool(field(&fields, "blackToVoid", line_number)),
                    provisional_bounds: parse_i32_array(field(&fields, "slim", line_number)),
                    shape: field(&fields, "shape", line_number).to_owned(),
                    pitch_bits: parse_double_bits(field(&fields, "pitch", line_number)),
                    bounds: parse_i32_array(field(&fields, "bounds", line_number)),
                    grade_bits: parse_double_bits(field(&fields, "grade", line_number)),
                    distance_impact_bits,
                    impact_weight_bits,
                    good: parse_field(&fields, "good", line_number),
                    glyph_bounds: parse_i32_array(field(&fields, "glyphBounds", line_number)),
                    glyph_weight: parse_field(&fields, "glyphWeight", line_number),
                    glyph_run_digest: parse_hex(field(&fields, "glyphRuns", line_number)),
                    tally_left_bits: parse_optional_double_bits(field(
                        &fields,
                        "tallyLeft",
                        line_number,
                    )),
                    tally_right_bits: parse_optional_double_bits(field(
                        &fields,
                        "tallyRight",
                        line_number,
                    )),
                };
                let heads = &mut page
                    .systems
                    .entry(system_id)
                    .or_default()
                    .staffs
                    .entry(staff_id)
                    .or_default()
                    .heads;
                assert_eq!(
                    head.ordinal,
                    heads.len(),
                    "head order at line {line_number}"
                );
                heads.push(head);
            }
            Some("headseedstaffsummary") => {
                let page = page_mut(&mut pages, &fields, line_number);
                let system_id = parse_field(&fields, "system", line_number);
                let staff_id = parse_field(&fields, "staff", line_number);
                let summary = StaffSummary {
                    scanners: parse_field(&fields, "scanners", line_number),
                    seeds: parse_field(&fields, "seeds", line_number),
                    searches: parse_field(&fields, "attempts", line_number),
                    candidates: parse_field(&fields, "candidates", line_number),
                    heads: parse_field(&fields, "heads", line_number),
                    outcomes: parse_colon_array(field(&fields, "outcomes", line_number)),
                    performance: parse_performance(field(&fields, "perf", line_number)),
                    kernel_hash: parse_hex(field(&fields, "kernelHash", line_number)),
                };
                let staff = page
                    .systems
                    .entry(system_id)
                    .or_default()
                    .staffs
                    .entry(staff_id)
                    .or_default();
                assert!(
                    staff.summary.replace(summary).is_none(),
                    "line {line_number}"
                );
            }
            Some("headseedsystemsummary") => {
                let page = page_mut(&mut pages, &fields, line_number);
                let system_id = parse_field(&fields, "system", line_number);
                let summary = SystemSummary {
                    staffs: parse_field(&fields, "staffs", line_number),
                    searches: parse_field(&fields, "attempts", line_number),
                    candidates: parse_field(&fields, "heads", line_number),
                    performance: parse_performance(field(&fields, "perf", line_number)),
                };
                let system = page.systems.entry(system_id).or_default();
                assert!(
                    system.summary.replace(summary).is_none(),
                    "line {line_number}"
                );
            }
            Some("headseedpagesummary") => {
                let page = page_mut(&mut pages, &fields, line_number);
                let summary = PageSummary {
                    staffs: parse_field(&fields, "staffs", line_number),
                    searches: parse_field(&fields, "attempts", line_number),
                    candidates: parse_field(&fields, "heads", line_number),
                };
                assert!(
                    page.summary.replace(summary).is_none(),
                    "line {line_number}"
                );
            }
            Some("headseedcorpussummary") => {
                let summary = CorpusSummary {
                    pages: parse_field(&fields, "pages", line_number),
                    systems: parse_field(&fields, "systems", line_number),
                    staffs: parse_field(&fields, "staffs", line_number),
                    searches: parse_field(&fields, "attempts", line_number),
                    candidates: parse_field(&fields, "candidates", line_number),
                    heads: parse_field(&fields, "heads", line_number),
                };
                assert!(
                    corpus_summary.replace(summary).is_none(),
                    "line {line_number}"
                );
            }
            _ => {}
        }
    }
    ExpectedCorpus {
        pages,
        summary: corpus_summary.expect("oracle corpus summary"),
    }
}

fn page_mut<'a>(
    pages: &'a mut BTreeMap<String, ExpectedPage>,
    fields: &[&str],
    line_number: usize,
) -> &'a mut ExpectedPage {
    pages
        .get_mut(fields[1])
        .unwrap_or_else(|| panic!("unknown page at line {line_number}"))
}

fn field<'a>(fields: &[&'a str], name: &str, line_number: usize) -> &'a str {
    let index = fields
        .iter()
        .position(|field| *field == name)
        .unwrap_or_else(|| panic!("line {line_number} has no {name} field"));
    fields
        .get(index + 1)
        .copied()
        .unwrap_or_else(|| panic!("line {line_number} has no {name} value"))
}

fn parse_field<T>(fields: &[&str], name: &str, line_number: usize) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Debug,
{
    field(fields, name, line_number)
        .parse()
        .unwrap_or_else(|error| panic!("line {line_number} invalid {name}: {error:?}"))
}

fn parse_colon_array<const N: usize>(text: &str) -> [usize; N] {
    let values = text
        .split(':')
        .map(|value| value.parse().expect("colon-delimited count"))
        .collect::<Vec<_>>();
    values
        .try_into()
        .unwrap_or_else(|_| panic!("expected {N} counts in {text}"))
}

fn parse_performance(text: &str) -> ExpectedPerformance {
    let [bars, competitors, evaluations, abandons] = parse_colon_array(text);
    ExpectedPerformance {
        bars,
        competitors,
        evaluations,
        abandons,
    }
}

fn parse_i32_array<const N: usize>(text: &str) -> [i32; N] {
    let values = text
        .split(':')
        .map(|value| value.parse().expect("colon-delimited integer"))
        .collect::<Vec<_>>();
    values
        .try_into()
        .unwrap_or_else(|_| panic!("expected {N} integers in {text}"))
}

fn parse_optional_bool(text: &str) -> Option<bool> {
    match text {
        "-" => None,
        "true" => Some(true),
        "false" => Some(false),
        _ => panic!("invalid optional boolean {text}"),
    }
}

fn parse_double_bits(text: &str) -> u64 {
    let (_, bits) = text
        .rsplit_once('/')
        .unwrap_or_else(|| panic!("double has no raw bits: {text}"));
    parse_hex(bits)
}

fn parse_optional_double_bits(text: &str) -> Option<u64> {
    (text != "-").then(|| parse_double_bits(text))
}

fn parse_impact(text: &str) -> (u64, u64) {
    let fields = text.split(':').collect::<Vec<_>>();
    assert_eq!(fields.len(), 3, "impact field {text}");
    assert_eq!(fields[0], "dist", "impact name {text}");
    (parse_double_bits(fields[1]), parse_double_bits(fields[2]))
}

fn parse_hex(text: &str) -> u64 {
    u64::from_str_radix(text, 16).unwrap_or_else(|error| panic!("invalid hex {text}: {error}"))
}

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}
