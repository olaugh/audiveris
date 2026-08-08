// SPDX-License-Identifier: AGPL-3.0-or-later

//! Eight-page production differential for the native HEADS epilog.
//!
//! This test owns the expensive GRID -> range-head page loader once. The
//! staff-purge and small-beam assertions are composed below as their production
//! surfaces land; the current boundary already proves that every compact-oracle
//! input ordinal and retained attachment row resolves to the exact native
//! seed/range product rather than a fixture-only reconstruction.

use std::{cmp::Ordering, path::PathBuf};

use audiveris_omr::{
    head_scanner_slices::JavaRectangle,
    head_seed_tally_analysis::HeadSeedTallySide,
    head_small_beam_purge::SmallBeamPurgeTarget,
    head_template::{HeadTemplateBounds, HeadTemplateShape},
    heads_post_range_corpus::{
        PostRangeBeamPurgeTarget, PostRangeBounds, PostRangeInputRef, PostRangeInter,
        PostRangeOrigin, PostRangePage, PostRangeShape, PostRangeStaffHead, PostRangeSystem,
        parse_heads_post_range_corpus,
    },
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads_prolog,
    native_heads_bar_slices::materialize_native_head_scanner_bar_slices,
    native_heads_competitor_slices::materialize_native_head_scanner_competitor_slices,
    native_heads_competitors::{
        NativeHeadsCompetitorPool, NativeHeadsCompetitorShape, materialize_native_heads_competitors,
    },
    native_heads_epilog::{
        JavaHeadSeedShape, NativeHeadsEpilogInput, NativeHeadsEpilogRecognition,
        compose_native_heads_epilog,
    },
    native_heads_obstacles::materialize_native_heads_bar_obstacles,
    native_heads_range_glyphs::{
        NativeHeadRangeGlyph, NativeHeadsRangeGlyphRecognition, NativeHeadsRangeGlyphsInput,
        retrieve_native_heads_range_glyphs,
    },
    native_heads_range_lookup::{NativeHeadsRangeLookupInput, recognize_native_heads_range_lookup},
    native_heads_scanner::recognize_native_heads_scanner_context,
    native_heads_scanner_pools::materialize_native_head_scanner_pools,
    native_heads_seed_glyphs::{
        NativeHeadSeedGlyph, NativeHeadsSeedGlyphRecognition, NativeHeadsSeedGlyphsInput,
        retrieve_native_heads_seed_glyphs,
    },
    native_heads_seed_lookup::{NativeHeadsSeedLookupInput, recognize_native_heads_seed_lookup},
    native_heads_small_beams::NativeHeadsSmallBeamSystemResult,
    native_heads_staff_epilog::{
        NativeHeadStaffEpilogOrigin, NativeHeadStaffEpilogRef, NativeHeadsStaffEpilogRecognition,
        NativeHeadsStaffEpilogStaff, NativeHeadsStaffEpilogSystem,
    },
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::recognize_native_stem_seeds,
    recognize::{
        GridLinesRecognition, NativeBeamRecognition, recognize_grid_lines,
        recognize_native_beams_with_stem_seeds,
    },
};

const ORACLE: &str = include_str!("../../../oracle/heads-post-range.txt");

struct NativeHeadsPageProducts {
    grid: GridLinesRecognition,
    beams: NativeBeamRecognition,
    competitors: NativeHeadsCompetitorPool,
    seed_heads: NativeHeadsSeedGlyphRecognition,
    range_heads: NativeHeadsRangeGlyphRecognition,
}

#[derive(Clone, Copy)]
enum NativeStaffHead<'a> {
    Seed(&'a NativeHeadSeedGlyph),
    Range(&'a NativeHeadRangeGlyph),
}

impl<'a> NativeStaffHead<'a> {
    fn origin(self) -> PostRangeOrigin {
        match self {
            Self::Seed(head) => PostRangeOrigin::Seed(head.ordinal),
            Self::Range(head) => PostRangeOrigin::Range(head.ordinal),
        }
    }

    fn relative_java_id(self, seed_count: usize) -> usize {
        match self {
            Self::Seed(head) => head.ordinal,
            Self::Range(head) => seed_count + head.ordinal,
        }
    }

    fn shape(self) -> HeadTemplateShape {
        match self {
            Self::Seed(head) => head.shape,
            Self::Range(head) => head.shape,
        }
    }

    fn bounds(self) -> HeadTemplateBounds {
        match self {
            Self::Seed(head) => head.bounds,
            Self::Range(head) => head.bounds,
        }
    }

    fn grade_bits(self) -> u64 {
        match self {
            Self::Seed(head) => head.grade_bits,
            Self::Range(head) => head.grade_bits,
        }
    }

    fn distance_impact_bits(self) -> u64 {
        match self {
            Self::Seed(head) => head.distance_impact_bits,
            Self::Range(head) => head.distance_impact_bits,
        }
    }

    fn good(self) -> bool {
        match self {
            Self::Seed(head) => head.good,
            Self::Range(head) => head.good,
        }
    }

    fn glyph(self) -> &'a audiveris_omr::head_glyph_retrieval::RetrievedHeadGlyph {
        match self {
            Self::Seed(head) => &head.glyph,
            Self::Range(head) => &head.glyph,
        }
    }
}

#[test]
fn native_heads_epilog_matches_java_corpus() {
    let expected = parse_heads_post_range_corpus(ORACLE).expect("valid HEADS post-range oracle");
    let mut totals = [0_usize; 3];
    let mut epilog_totals = [0_usize; 5];
    for page in &expected.pages {
        let native = load_native_page(page);
        assert_pre_epilog_staff_contract(page, &native, &mut totals);
        let epilog = compose_native_heads_epilog(NativeHeadsEpilogInput {
            seed_glyphs: &native.seed_heads,
            range_glyphs: &native.range_heads,
            competitors: &native.competitors,
            beams: &native.beams,
        })
        .unwrap_or_else(|error| panic!("{}: complete epilog failed: {error}", page.page));
        assert_staff_epilog_contract(page, &epilog.staff_epilog);
        assert_small_beam_contract(page, &native, &epilog);
        assert_scale_contract(page, &epilog);
        assert_eq!(
            epilog.beam_removal_count, page.summary.purged_beams,
            "{}",
            page.page
        );
        assert_eq!(
            epilog.head_beam_removal_count, page.summary.purged_heads,
            "{}",
            page.page
        );
        assert_eq!(
            epilog.final_head_count, page.summary.final_heads,
            "{}",
            page.page
        );
        epilog_totals[0] += epilog.staff_epilog.retained_head_count;
        epilog_totals[1] += epilog.head_beam_removal_count;
        epilog_totals[2] += epilog.final_head_count;
        epilog_totals[3] += epilog
            .systems
            .iter()
            .flat_map(|system| system.tally_left.iter().chain(&system.tally_right))
            .filter(|entry| !entry.removed)
            .count();
        epilog_totals[4] += epilog.scale_analysis.len();

        // Keep the products alive together: staff epilog consumes seed/range
        // heads, while small-beam arbitration also consumes GRID/BEAMS.
        assert_eq!(
            native.grid.staves.len(),
            page.declared_staves,
            "{}",
            page.page
        );
        assert!(
            native.beams.raw_beams.len() + native.beams.hooks.len()
                >= page
                    .systems
                    .iter()
                    .map(|system| system.summary.beam_inputs)
                    .sum::<usize>(),
            "{} short-beam inputs cannot exceed all native beams/hooks",
            page.page
        );
        assert_eq!(native.competitors.systems.len(), page.systems.len());
    }
    assert_eq!(totals, [3_435, 174, 3_609]);
    assert_eq!(epilog_totals, [3_547, 26, 3_521, 1_451, 18]);
    assert_eq!(expected.summary.inputs, totals[2]);
}

fn load_native_page(page: &PostRangePage) -> NativeHeadsPageProducts {
    let image = page.page.strip_suffix("#1").expect("page sheet suffix");
    let grid = recognize_grid_lines(repo_path(&format!("data/examples/{image}")))
        .unwrap_or_else(|error| panic!("{}: GRID failed: {error}", page.page));
    let obstacles = materialize_native_heads_bar_obstacles(&grid)
        .unwrap_or_else(|error| panic!("{}: obstacles failed: {error}", page.page));
    let headers = recognize_native_headers(&grid)
        .unwrap_or_else(|error| panic!("{}: HEADERS failed: {error}", page.page));
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers)
        .unwrap_or_else(|error| panic!("{}: STEM_SEEDS failed: {error}", page.page));
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .unwrap_or_else(|error| panic!("{}: BEAMS failed: {error}", page.page));
    let competitors = materialize_native_heads_competitors(&grid, &beams)
        .unwrap_or_else(|error| panic!("{}: competitors failed: {error}", page.page));
    let ledgers = recognize_native_ledgers(&grid, &beams)
        .unwrap_or_else(|error| panic!("{}: LEDGERS failed: {error}", page.page));
    let heads = recognize_native_heads_prolog(&grid, &beams, &ledgers, &stem_seeds)
        .unwrap_or_else(|error| panic!("{}: HEADS prolog failed: {error}", page.page));
    let scanners =
        recognize_native_heads_scanner_context(&grid, &headers, &ledgers, &stem_seeds, &heads)
            .unwrap_or_else(|error| panic!("{}: scanner context failed: {error}", page.page));
    let pools = materialize_native_head_scanner_pools(&stem_seeds, &heads, &scanners)
        .unwrap_or_else(|error| panic!("{}: scanner pools failed: {error}", page.page));
    let bar_slices = materialize_native_head_scanner_bar_slices(&pools, &obstacles)
        .unwrap_or_else(|error| panic!("{}: bar slices failed: {error}", page.page));
    let competitor_slices = materialize_native_head_scanner_competitor_slices(&pools, &competitors)
        .unwrap_or_else(|error| panic!("{}: competitor slices failed: {error}", page.page));
    let seed_lookup = recognize_native_heads_seed_lookup(NativeHeadsSeedLookupInput {
        heads: &heads,
        scanners: &scanners,
        pools: &pools,
        obstacles: &obstacles,
        bar_slices: &bar_slices,
        competitors: &competitors,
        competitor_slices: &competitor_slices,
        stem_seeds: &stem_seeds,
    })
    .unwrap_or_else(|error| panic!("{}: seed lookup failed: {error}", page.page));
    let seed_heads = retrieve_native_heads_seed_glyphs(NativeHeadsSeedGlyphsInput {
        grid: &grid,
        heads: &heads,
        lookup: &seed_lookup,
    })
    .unwrap_or_else(|error| panic!("{}: seed glyphs failed: {error}", page.page));
    let range_lookup = recognize_native_heads_range_lookup(NativeHeadsRangeLookupInput {
        heads: &heads,
        scanners: &scanners,
        pools: &pools,
        obstacles: &obstacles,
        bar_slices: &bar_slices,
        competitors: &competitors,
        competitor_slices: &competitor_slices,
    })
    .unwrap_or_else(|error| panic!("{}: range lookup failed: {error}", page.page));
    let range_heads = retrieve_native_heads_range_glyphs(NativeHeadsRangeGlyphsInput {
        grid: &grid,
        heads: &heads,
        scanners: &scanners,
        pools: &pools,
        competitors: &competitors,
        range_lookup: &range_lookup,
        seed_glyphs: &seed_heads,
    })
    .unwrap_or_else(|error| panic!("{}: range glyphs failed: {error}", page.page));
    NativeHeadsPageProducts {
        grid,
        beams,
        competitors,
        seed_heads,
        range_heads,
    }
}

fn assert_pre_epilog_staff_contract(
    page: &PostRangePage,
    native: &NativeHeadsPageProducts,
    totals: &mut [usize; 3],
) {
    for expected in &page.staffs {
        let label = format!(
            "{} system {} staff {}",
            page.page, expected.system_id, expected.staff_id
        );
        let seed_staff = native
            .seed_heads
            .systems
            .iter()
            .find(|system| system.system_id == expected.system_id)
            .and_then(|system| {
                system
                    .staffs
                    .iter()
                    .find(|staff| staff.staff_id == expected.staff_id)
            })
            .unwrap_or_else(|| panic!("{label}: missing native seed staff"));
        let range_staff = native
            .range_heads
            .systems
            .iter()
            .find(|system| system.system_id == expected.system_id)
            .and_then(|system| {
                system
                    .staffs
                    .iter()
                    .find(|staff| staff.staff_id == expected.staff_id)
            })
            .unwrap_or_else(|| panic!("{label}: missing native range staff"));
        assert_eq!(
            seed_staff.heads.len(),
            expected.summary.seed_inputs,
            "{label}"
        );
        assert_eq!(
            range_staff.heads.len(),
            expected.summary.range_inputs,
            "{label}"
        );
        let inputs = sorted_staff_inputs(&seed_staff.heads, &range_staff.heads);
        assert_eq!(inputs.len(), expected.summary.inputs, "{label}");

        for decision in expected
            .duplicate_decisions
            .iter()
            .chain(&expected.overlap_decisions)
        {
            assert_input_ref(&inputs, decision.left, &label);
            assert_input_ref(&inputs, decision.right, &label);
            assert!(
                decision.purged_input < inputs.len() && decision.kept_input < inputs.len(),
                "{label}: purge output reference"
            );
        }
        for head in &expected.attachment_heads {
            let actual = inputs
                .get(head.input_ordinal)
                .copied()
                .unwrap_or_else(|| panic!("{label}: missing input {}", head.input_ordinal));
            assert_eq!(actual.origin(), head.origin, "{label}: attachment origin");
            assert_head(actual, head, &label);
        }

        totals[0] += seed_staff.heads.len();
        totals[1] += range_staff.heads.len();
        totals[2] += inputs.len();
    }
}

fn assert_staff_epilog_contract(page: &PostRangePage, actual: &NativeHeadsStaffEpilogRecognition) {
    assert_eq!(
        actual.input_head_count, page.summary.inputs,
        "{}",
        page.page
    );
    assert_eq!(
        actual.duplicate_removal_count, page.summary.duplicates,
        "{}",
        page.page
    );
    assert_eq!(
        actual.overlap_exclusion_count, page.summary.overlaps,
        "{}",
        page.page
    );
    assert_eq!(
        actual.retained_head_count, page.summary.staff_heads,
        "{}",
        page.page
    );
    for expected in &page.staffs {
        let label = format!(
            "{} system {} staff {}",
            page.page, expected.system_id, expected.staff_id
        );
        let staff = actual
            .systems
            .iter()
            .find(|system| system.system_id == expected.system_id)
            .and_then(|system| {
                system
                    .staffs
                    .iter()
                    .find(|staff| staff.staff_id == expected.staff_id)
            })
            .unwrap_or_else(|| panic!("{label}: missing native staff epilog"));
        assert_eq!(staff.heads.len(), expected.summary.inputs, "{label}");
        assert_eq!(
            staff.purge.duplicate.decisions.len(),
            expected.duplicate_decisions.len(),
            "{label}: duplicate decisions"
        );
        assert_eq!(
            staff.purge.overlap.decisions.len(),
            expected.overlap_decisions.len(),
            "{label}: overlap decisions"
        );
        assert_purge_decisions(
            staff,
            &staff.purge.duplicate.decisions,
            &expected.duplicate_decisions,
            &format!("{label} duplicate"),
        );
        assert_purge_decisions(
            staff,
            &staff.purge.overlap.decisions,
            &expected.overlap_decisions,
            &format!("{label} overlap"),
        );
        assert_eq!(
            staff.retained_attachment_indices.len(),
            expected.attachment_heads.len(),
            "{label}: attachment count"
        );
        for (&head_index, expected) in staff
            .retained_attachment_indices
            .iter()
            .zip(&expected.attachment_heads)
        {
            let head = &staff.heads[head_index];
            assert_eq!(head.input_ordinal, expected.input_ordinal, "{label}");
            assert_eq!(origin(head.origin), expected.origin, "{label}");
            assert_epilog_head(head, expected, &label);
        }
    }
}

fn assert_purge_decisions(
    staff: &NativeHeadsStaffEpilogStaff,
    actual: &[audiveris_omr::head_purge::HeadPurgeDecision],
    expected: &[audiveris_omr::heads_post_range_corpus::PostRangePurgeDecision],
    label: &str,
) {
    for (actual, expected) in actual.iter().zip(expected) {
        let left = &staff.heads[actual.left_index];
        let right = &staff.heads[actual.right_index];
        let purged = &staff.heads[actual.purged_index];
        let kept = &staff.heads[actual.kept_index];
        assert_eq!(left.input_ordinal, expected.left.input_ordinal, "{label}");
        assert_eq!(right.input_ordinal, expected.right.input_ordinal, "{label}");
        assert_eq!(origin(left.origin), expected.left.origin, "{label}");
        assert_eq!(origin(right.origin), expected.right.origin, "{label}");
        assert_eq!(purged.input_ordinal, expected.purged_input, "{label}");
        assert_eq!(kept.input_ordinal, expected.kept_input, "{label}");
        assert_eq!(
            actual.grade_difference.to_bits(),
            expected.grade_difference.bits,
            "{label}"
        );
        assert_eq!(
            purge_reason(actual.reason),
            expected.choice,
            "{label}: reason"
        );
    }
}

fn assert_epilog_head(
    actual: &audiveris_omr::native_heads_staff_epilog::NativeHeadStaffEpilogHead,
    expected: &PostRangeStaffHead,
    label: &str,
) {
    assert_eq!(actual.shape, shape(expected.inter.shape), "{label}");
    assert_eq!(
        actual.bounds,
        java_bounds(bounds(expected.inter.bounds)),
        "{label}"
    );
    assert_eq!(actual.grade_bits, expected.inter.grade.bits, "{label}");
    assert_eq!(
        actual.distance_impact_bits, expected.inter.impacts[0].value.bits,
        "{label}: epilog distance impact"
    );
    assert_eq!(actual.good, expected.inter.good, "{label}");
    let glyph = expected.inter.glyph.expect("staff-head glyph");
    assert_eq!(
        actual.glyph_bounds,
        java_bounds(bounds(glyph.bounds)),
        "{label}"
    );
    assert_eq!(actual.glyph_weight, glyph.weight, "{label}");
    assert_eq!(actual.glyph_run_digest, glyph.run_hash, "{label}");
    assert_eq!(
        actual.tally_after_replication.left_dx.map(f64::to_bits),
        expected.tally.left.as_ref().map(|value| value.bits),
        "{label}: left tally"
    );
    assert_eq!(
        actual.tally_after_replication.right_dx.map(f64::to_bits),
        expected.tally.right.as_ref().map(|value| value.bits),
        "{label}: right tally"
    );
    assert_eq!(
        actual.tally_retained_after_cleanup,
        expected.tally.left.is_some() || expected.tally.right.is_some(),
        "{label}: tally cleanup"
    );
    assert!(!actual.duplicate_removed, "{label}");
}

fn origin(origin: NativeHeadStaffEpilogOrigin) -> PostRangeOrigin {
    match origin {
        NativeHeadStaffEpilogOrigin::Seed(ordinal) => PostRangeOrigin::Seed(ordinal),
        NativeHeadStaffEpilogOrigin::Range(ordinal) => PostRangeOrigin::Range(ordinal),
    }
}

fn purge_reason(
    reason: audiveris_omr::head_purge::HeadPurgeReason,
) -> audiveris_omr::heads_post_range_corpus::PostRangePurgeChoice {
    use audiveris_omr::{
        head_purge::HeadPurgeReason as Actual,
        heads_post_range_corpus::PostRangePurgeChoice as Expected,
    };
    match reason {
        Actual::LowerGradeLeft => Expected::LowerGradeLeft,
        Actual::LowerGradeRight => Expected::LowerGradeRight,
        Actual::EqualLeftNoSeed => Expected::EqualLeftNoSeed,
        Actual::EqualRightNoSeed => Expected::EqualRightNoSeed,
        Actual::EqualDifferentShape => Expected::EqualShape,
        Actual::EqualDifferentBounds => Expected::EqualBounds,
        Actual::EqualPreserveLeft => Expected::EqualPreserveLeft,
    }
}

fn assert_small_beam_contract(
    page: &PostRangePage,
    native: &NativeHeadsPageProducts,
    epilog: &NativeHeadsEpilogRecognition,
) {
    assert_eq!(epilog.systems.len(), page.systems.len(), "{}", page.page);
    for expected in &page.systems {
        let actual = epilog
            .systems
            .iter()
            .find(|system| system.system_id == expected.system_id)
            .unwrap_or_else(|| {
                panic!(
                    "{} system {}: missing small-beam result",
                    page.page, expected.system_id
                )
            });
        let staff_system = epilog
            .staff_epilog
            .systems
            .iter()
            .find(|system| system.system_id == expected.system_id)
            .expect("staff-epilog system");
        assert_eq!(
            actual.heads_in_sig_order, staff_system.retained_creation_order,
            "{} system {} SIG order",
            page.page, expected.system_id
        );
        assert_eq!(
            actual.heads_before_beam_by_ordinate,
            actual
                .small_beams
                .arbitration
                .ordered_head_indices
                .iter()
                .map(|&index| actual.small_beams.head_provenance[index])
                .collect::<Vec<_>>(),
            "{} system {} top-level ordinate order",
            page.page,
            expected.system_id
        );
        assert_eq!(
            actual.beam_removed_heads,
            actual
                .small_beams
                .arbitration
                .decisions
                .iter()
                .filter_map(|decision| {
                    (decision.target == SmallBeamPurgeTarget::Head)
                        .then_some(actual.small_beams.head_provenance[decision.head_index])
                })
                .collect::<Vec<_>>(),
            "{} system {} top-level removals",
            page.page,
            expected.system_id
        );
        assert_eq!(
            actual.final_heads,
            actual
                .small_beams
                .arbitration
                .remaining_head_indices
                .iter()
                .map(|&index| actual.small_beams.head_provenance[index])
                .collect::<Vec<_>>(),
            "{} system {} top-level finals",
            page.page,
            expected.system_id
        );
        assert_small_beam_system(page, native, staff_system, &actual.small_beams, expected);
        assert_eq!(
            actual.final_heads.len(),
            expected.final_heads.len(),
            "{} system {} top-level finals",
            page.page,
            expected.system_id
        );
    }
}

fn assert_small_beam_system(
    page: &PostRangePage,
    native: &NativeHeadsPageProducts,
    staff_system: &NativeHeadsStaffEpilogSystem,
    actual: &NativeHeadsSmallBeamSystemResult<NativeHeadStaffEpilogRef>,
    expected: &PostRangeSystem,
) {
    let label = format!("{} system {}", page.page, expected.system_id);
    assert_eq!(
        actual.arbitration.candidate_beam_indices.len(),
        expected.beam_inputs.len(),
        "{label}: narrow beam count"
    );
    assert_eq!(
        actual.arbitration.decisions.len(),
        expected.beam_decisions.len(),
        "{label}: beam-decision count"
    );

    let competitor_system = native
        .competitors
        .systems
        .iter()
        .find(|system| system.system_id == expected.system_id)
        .expect("competitor system");
    for (&beam_index, expected) in actual
        .arbitration
        .candidate_beam_indices
        .iter()
        .zip(&expected.beam_inputs)
    {
        assert_eq!(
            beam_ordinal(actual, beam_index),
            expected.ordinal,
            "{label}"
        );
        let input = actual.beam_inputs[beam_index];
        let provenance = actual.beam_provenance[beam_index];
        let competitor = competitor_system
            .candidates_in_sig_order
            .iter()
            .find(|candidate| {
                candidate.source == provenance.source
                    && candidate.source_ordinal == provenance.source_ordinal
            })
            .unwrap_or_else(|| panic!("{label}: missing beam provenance {provenance:?}"));
        let inter = &expected.inter;
        assert_eq!(inter.class_name, beam_class(competitor.shape), "{label}");
        assert_eq!(inter.shape, beam_shape(competitor.shape), "{label}");
        assert_eq!(input.bounds, java_bounds(bounds(inter.bounds)), "{label}");
        assert_eq!(competitor.grade.to_bits(), inter.grade.bits, "{label}");
        assert_eq!(
            competitor.best_grade.to_bits(),
            inter.best_grade.bits,
            "{label}"
        );
        assert_eq!(competitor.good, inter.good, "{label}");
        assert!(!inter.removed, "{label}");
        assert!(inter.contextual_grade.is_none(), "{label}");
        assert!(inter.impacts.is_empty(), "{label}");
        assert_eq!(inter.beam_width, Some(input.bounds.width), "{label}");
        assert_eq!(
            inter.mean_height.as_ref().map(|height| height.bits),
            Some(input.height.to_bits()),
            "{label}"
        );
        // The competitor materializer does not yet retain the beam source
        // glyph's run-table digest and weight, so these fixture fields remain
        // the one ungraded part of each otherwise exact beam-input row.
        assert!(inter.glyph.is_some(), "{label}: missing oracle beam glyph");
    }

    for (decision, expected) in actual
        .arbitration
        .decisions
        .iter()
        .zip(&expected.beam_decisions)
    {
        assert_eq!(
            beam_ordinal(actual, decision.beam_index),
            expected.beam_ordinal,
            "{label}"
        );
        assert_eq!(
            head_ordinal(actual, decision.head_index),
            expected.head_ordinal,
            "{label}"
        );
        let target = match decision.target {
            SmallBeamPurgeTarget::Beam => PostRangeBeamPurgeTarget::Beam,
            SmallBeamPurgeTarget::Head => PostRangeBeamPurgeTarget::Head,
        };
        assert_eq!(target, expected.purged, "{label}");
    }

    let removed = actual
        .arbitration
        .ordered_head_indices
        .iter()
        .copied()
        .filter(|&index| actual.arbitration.head_removed[index])
        .collect::<Vec<_>>();
    assert_eq!(removed.len(), expected.purged_heads.len(), "{label}");
    for (&head_index, expected) in removed.iter().zip(&expected.purged_heads) {
        assert_eq!(
            head_ordinal(actual, head_index),
            expected.head_ordinal,
            "{label}"
        );
        let head = epilog_head(staff_system, actual.head_provenance[head_index]);
        assert_system_head(head, &expected.inter, true, &label);
    }
    assert_eq!(
        actual.arbitration.remaining_head_indices.len(),
        expected.final_heads.len(),
        "{label}"
    );
    for (ordinal, (&head_index, expected)) in actual
        .arbitration
        .remaining_head_indices
        .iter()
        .zip(&expected.final_heads)
        .enumerate()
    {
        assert_eq!(ordinal, expected.ordinal, "{label}");
        let head = epilog_head(staff_system, actual.head_provenance[head_index]);
        assert_system_head(head, &expected.inter, false, &label);
    }
    assert_eq!(
        actual.arbitration.checks.len(),
        expected.summary.beam_checks.count,
        "{label}: beam-check count"
    );
    assert_eq!(
        fnv_rows(actual_beam_check_rows(page, actual)),
        expected.summary.beam_checks.hash,
        "{label}: beam-check hash"
    );
}

fn assert_scale_contract(page: &PostRangePage, epilog: &NativeHeadsEpilogRecognition) {
    let mut ordinal = 0_usize;
    for system in &epilog.systems {
        for entries in [&system.tally_left, &system.tally_right] {
            for (entry_ordinal, entry) in entries.iter().enumerate() {
                if !entry.removed {
                    let expected = page
                        .scale_inputs
                        .get(ordinal)
                        .unwrap_or_else(|| panic!("{}: missing scale input {ordinal}", page.page));
                    assert_eq!(expected.system_id, system.system_id, "{}", page.page);
                    assert_eq!(expected.ordinal, ordinal, "{}", page.page);
                    assert_eq!(expected.entry_ordinal, entry_ordinal, "{}", page.page);
                    assert_eq!(expected.side, post_range_side(entry.side), "{}", page.page);
                    assert_eq!(
                        expected.shape,
                        post_range_seed_shape(entry.shape),
                        "{}",
                        page.page
                    );
                    assert_eq!(expected.dx.bits, entry.dx.to_bits(), "{}", page.page);
                    ordinal += 1;
                }
            }
        }
    }
    assert_eq!(ordinal, page.scale_inputs.len(), "{}", page.page);
    assert_eq!(
        epilog.scale_analysis.len(),
        page.scales.len(),
        "{}",
        page.page
    );
    for (actual, expected) in epilog.scale_analysis.iter().zip(&page.scales) {
        assert_eq!(
            post_range_seed_shape(actual.shape),
            expected.shape,
            "{}",
            page.page
        );
        assert_eq!(actual.side, tally_side(expected.side), "{}", page.page);
        assert_eq!(actual.dx.to_bits(), expected.dx.bits, "{}", page.page);
    }
}

fn post_range_side(
    side: HeadSeedTallySide,
) -> audiveris_omr::heads_post_range_corpus::PostRangeSide {
    use audiveris_omr::heads_post_range_corpus::PostRangeSide;
    match side {
        HeadSeedTallySide::Left => PostRangeSide::Left,
        HeadSeedTallySide::Right => PostRangeSide::Right,
    }
}

fn tally_side(side: audiveris_omr::heads_post_range_corpus::PostRangeSide) -> HeadSeedTallySide {
    use audiveris_omr::heads_post_range_corpus::PostRangeSide;
    match side {
        PostRangeSide::Left => HeadSeedTallySide::Left,
        PostRangeSide::Right => HeadSeedTallySide::Right,
    }
}

fn post_range_seed_shape(shape: JavaHeadSeedShape) -> PostRangeShape {
    match shape {
        JavaHeadSeedShape::NoteheadVoid => PostRangeShape::NoteheadVoid,
        JavaHeadSeedShape::NoteheadBlack => PostRangeShape::NoteheadBlack,
        other => panic!("unexpected tally head shape {other:?}"),
    }
}

fn actual_beam_check_rows(
    page: &PostRangePage,
    system: &NativeHeadsSmallBeamSystemResult<NativeHeadStaffEpilogRef>,
) -> Vec<String> {
    system
        .arbitration
        .checks
        .iter()
        .enumerate()
        .map(|(ordinal, check)| {
            format!(
                "headpostbeamcheck {} system {} ordinal {} beam {} head {} intersects {} breaks {} headGrade {} beamGrade {}",
                page.page,
                system.system_id,
                ordinal,
                beam_ordinal(system, check.beam_index),
                head_ordinal(system, check.head_index),
                check.intersects,
                check.breaks,
                java_hex_double(system.head_inputs[check.head_index].grade),
                java_hex_double(system.beam_inputs[check.beam_index].contextual_grade),
            )
        })
        .collect()
}

fn beam_ordinal(
    system: &NativeHeadsSmallBeamSystemResult<NativeHeadStaffEpilogRef>,
    beam_index: usize,
) -> usize {
    system
        .arbitration
        .candidate_beam_indices
        .iter()
        .position(|&index| index == beam_index)
        .expect("candidate beam ordinal")
}

fn head_ordinal(
    system: &NativeHeadsSmallBeamSystemResult<NativeHeadStaffEpilogRef>,
    head_index: usize,
) -> usize {
    system
        .arbitration
        .ordered_head_indices
        .iter()
        .position(|&index| index == head_index)
        .expect("head ordinate ordinal")
}

fn epilog_head(
    system: &NativeHeadsStaffEpilogSystem,
    reference: NativeHeadStaffEpilogRef,
) -> &audiveris_omr::native_heads_staff_epilog::NativeHeadStaffEpilogHead {
    &system.staffs[reference.staff_index].heads[reference.head_index]
}

fn assert_system_head(
    actual: &audiveris_omr::native_heads_staff_epilog::NativeHeadStaffEpilogHead,
    expected: &PostRangeInter,
    removed: bool,
    label: &str,
) {
    assert_eq!(actual.shape, shape(expected.shape), "{label}");
    assert_eq!(
        actual.bounds,
        java_bounds(bounds(expected.bounds)),
        "{label}"
    );
    assert_eq!(actual.grade_bits, expected.grade.bits, "{label}");
    assert_eq!(expected.removed, removed, "{label}");
    let glyph = expected.glyph.expect("expected system-head glyph");
    assert_eq!(
        actual.glyph_bounds,
        java_bounds(bounds(glyph.bounds)),
        "{label}"
    );
    assert_eq!(actual.glyph_weight, glyph.weight, "{label}");
    assert_eq!(actual.glyph_run_digest, glyph.run_hash, "{label}");
}

fn beam_class(shape: NativeHeadsCompetitorShape) -> &'static str {
    match shape {
        NativeHeadsCompetitorShape::Beam => "BeamInter",
        NativeHeadsCompetitorShape::BeamHook => "BeamHookInter",
        NativeHeadsCompetitorShape::BeamSmall => "SmallBeamInter",
        other => panic!("non-beam competitor {other:?}"),
    }
}

fn beam_shape(shape: NativeHeadsCompetitorShape) -> PostRangeShape {
    match shape {
        NativeHeadsCompetitorShape::Beam => PostRangeShape::Beam,
        NativeHeadsCompetitorShape::BeamHook => PostRangeShape::BeamHook,
        other => panic!("unsupported oracle beam shape {other:?}"),
    }
}

fn fnv_rows(rows: Vec<String>) -> u64 {
    rows.iter()
        .flat_map(|row| row.bytes().chain([b'\n']))
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
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

fn sorted_staff_inputs<'a>(
    seeds: &'a [NativeHeadSeedGlyph],
    ranges: &'a [NativeHeadRangeGlyph],
) -> Vec<NativeStaffHead<'a>> {
    let seed_count = seeds.len();
    let mut inputs = seeds
        .iter()
        .map(NativeStaffHead::Seed)
        .chain(ranges.iter().map(NativeStaffHead::Range))
        .collect::<Vec<_>>();
    inputs.sort_by(|left, right| {
        let left_bounds = left.bounds();
        let right_bounds = right.bounds();
        java_i32_compare(left_bounds.x, right_bounds.x)
            .then_with(|| java_i32_compare(left_bounds.y, right_bounds.y))
            .then_with(|| {
                left.relative_java_id(seed_count)
                    .cmp(&right.relative_java_id(seed_count))
            })
    });
    inputs
}

fn java_i32_compare(left: i32, right: i32) -> Ordering {
    left.wrapping_sub(right).cmp(&0)
}

fn assert_input_ref(inputs: &[NativeStaffHead<'_>], expected: PostRangeInputRef, label: &str) {
    let actual = inputs
        .get(expected.input_ordinal)
        .copied()
        .unwrap_or_else(|| panic!("{label}: missing decision input {}", expected.input_ordinal));
    assert_eq!(actual.origin(), expected.origin, "{label}: decision origin");
}

fn assert_head(actual: NativeStaffHead<'_>, expected: &PostRangeStaffHead, label: &str) {
    let inter = &expected.inter;
    assert_eq!(inter.class_name, "HeadInter", "{label}: class");
    assert_eq!(actual.shape(), shape(inter.shape), "{label}: shape");
    assert_eq!(actual.bounds(), bounds(inter.bounds), "{label}: bounds");
    assert_eq!(
        actual.grade_bits(),
        expected.grade_before.bits,
        "{label}: grade before"
    );
    assert_eq!(
        actual.grade_bits(),
        expected.grade_after.bits,
        "{label}: grade after"
    );
    assert_eq!(actual.grade_bits(), inter.grade.bits, "{label}: grade");
    assert_eq!(
        actual.grade_bits(),
        inter.best_grade.bits,
        "{label}: best grade"
    );
    assert_eq!(actual.good(), inter.good, "{label}: good");
    assert!(!inter.removed, "{label}: pre-small-beam removed state");
    assert!(
        inter.contextual_grade.is_none(),
        "{label}: contextual grade"
    );
    assert_eq!(inter.impacts.len(), 1, "{label}: impact cardinality");
    assert_eq!(inter.impacts[0].name, "dist", "{label}: impact name");
    assert_eq!(
        actual.distance_impact_bits(),
        inter.impacts[0].value.bits,
        "{label}: distance impact"
    );
    assert_eq!(
        inter.impacts[0].weight.bits,
        1.0_f64.to_bits(),
        "{label}: impact weight"
    );
    let expected_glyph = inter.glyph.expect("post-range head glyph");
    let glyph = actual.glyph();
    assert_eq!(
        glyph.glyph_bounds,
        bounds(expected_glyph.bounds),
        "{label}: glyph bounds"
    );
    assert_eq!(glyph.weight, expected_glyph.weight, "{label}: glyph weight");
    assert_eq!(
        glyph.run_digest, expected_glyph.run_hash,
        "{label}: glyph runs"
    );
}

fn shape(shape: PostRangeShape) -> HeadTemplateShape {
    match shape {
        PostRangeShape::NoteheadBlack => HeadTemplateShape::NoteheadBlack,
        PostRangeShape::NoteheadVoid => HeadTemplateShape::NoteheadVoid,
        PostRangeShape::Beam | PostRangeShape::BeamHook => {
            panic!("beam shape in head-only assertion")
        }
    }
}

fn bounds(bounds: PostRangeBounds) -> HeadTemplateBounds {
    HeadTemplateBounds {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: bounds.height,
    }
}

fn java_bounds(bounds: HeadTemplateBounds) -> JavaRectangle {
    JavaRectangle::new(bounds.x, bounds.y, bounds.width, bounds.height)
}

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}
