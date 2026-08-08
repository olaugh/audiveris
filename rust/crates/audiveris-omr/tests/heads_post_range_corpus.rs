// SPDX-License-Identifier: AGPL-3.0-or-later

//! Contract tests for the compact Java HEADS post-range oracle.

#[path = "../src/heads_post_range_corpus.rs"]
mod heads_post_range_corpus;

use heads_post_range_corpus::{
    PostRangeOrigin, PostRangePurgeChoice, PostRangeShape, parse_heads_post_range_corpus,
};

const ORACLE: &str = include_str!("../../../oracle/heads-post-range.txt");

#[test]
fn compact_post_range_fixture_has_the_exact_frozen_contract() {
    let corpus = parse_heads_post_range_corpus(ORACLE).expect("valid post-range corpus");
    assert_eq!(corpus.pages.len(), 8);
    assert_eq!(
        corpus
            .pages
            .iter()
            .map(|page| page.page.as_str())
            .collect::<Vec<_>>(),
        [
            "chula.png#1",
            "allegretto.png#1",
            "batuque.png#1",
            "carmen.png#1",
            "cucaracha.png#1",
            "hove.png#1",
            "zizi.png#1",
            "BachInvention5.jpg#1",
        ]
    );
    let summary = &corpus.summary;
    assert_eq!(summary.systems, 30);
    assert_eq!(summary.staffs, 55);
    assert_eq!(summary.inputs, 3_609);
    assert_eq!(summary.duplicates, 62);
    assert_eq!(summary.overlaps, 2_725);
    assert_eq!(summary.staff_heads, 3_547);
    assert_eq!(summary.beam_input_rows, 191);
    assert_eq!(summary.beam_decision_rows, 26);
    assert_eq!(summary.purged_beams, 0);
    assert_eq!(summary.purged_heads, 26);
    assert_eq!(summary.final_heads, 3_521);
    assert_eq!(summary.scale_input_rows, 1_451);
    assert_eq!(summary.scale_rows, 18);
    assert_eq!(
        corpus.fixture_sha256,
        [
            0xe8, 0x93, 0xc2, 0x32, 0x7a, 0x9a, 0xfa, 0x93, 0x70, 0x35, 0x55, 0x9f, 0x1a, 0x5b,
            0xe1, 0x70, 0xa2, 0x21, 0x48, 0xdd, 0x66, 0x55, 0xe8, 0xff, 0xb6, 0x29, 0x7c, 0x75,
            0xbf, 0xf5, 0xf6, 0xba,
        ]
    );
    let first_page = &corpus.pages[0];
    assert!(first_page.staff(1, 1).is_some());
    assert!(first_page.system(1).is_some());
}

#[test]
fn retained_rows_are_identity_free_compositor_and_attachment_inputs() {
    let corpus = parse_heads_post_range_corpus(ORACLE).expect("valid post-range corpus");
    let heads = corpus
        .pages
        .iter()
        .flat_map(|page| &page.staffs)
        .flat_map(|staff| &staff.attachment_heads)
        .collect::<Vec<_>>();
    assert_eq!(heads.len(), 3_547);
    assert_eq!(
        heads
            .iter()
            .filter(|head| matches!(head.origin, PostRangeOrigin::Seed(_)))
            .count(),
        3_373
    );
    assert_eq!(
        heads
            .iter()
            .filter(|head| matches!(head.origin, PostRangeOrigin::Range(_)))
            .count(),
        174
    );
    assert_eq!(
        heads
            .iter()
            .filter(|head| head.inter.shape == PostRangeShape::NoteheadBlack)
            .count(),
        1_594
    );
    assert_eq!(
        heads
            .iter()
            .filter(|head| head.inter.shape == PostRangeShape::NoteheadVoid)
            .count(),
        1_953
    );
    assert!(heads.iter().all(|head| !head.stemless));
    assert!(
        heads
            .iter()
            .all(|head| head.grade_before.bits == head.grade_after.bits)
    );

    let scale_inputs = corpus
        .pages
        .iter()
        .flat_map(|page| &page.scale_inputs)
        .collect::<Vec<_>>();
    assert_eq!(scale_inputs.len(), 1_451);
    assert_eq!(
        scale_inputs
            .iter()
            .filter(|input| input.shape == PostRangeShape::NoteheadBlack)
            .count(),
        1_407
    );
    assert_eq!(
        scale_inputs
            .iter()
            .filter(|input| input.shape == PostRangeShape::NoteheadVoid)
            .count(),
        44
    );
    assert_eq!(PostRangeShape::NoteheadBlack.java_name(), "NOTEHEAD_BLACK");
    assert_eq!(
        scale_inputs[0].dx.value().to_bits(),
        scale_inputs[0].dx.bits
    );
}

#[test]
fn purge_choice_distribution_and_boundary_gaps_stay_explicit() {
    let corpus = parse_heads_post_range_corpus(ORACLE).expect("valid post-range corpus");
    let decisions = corpus
        .pages
        .iter()
        .flat_map(|page| &page.staffs)
        .flat_map(|staff| {
            staff
                .duplicate_decisions
                .iter()
                .chain(&staff.overlap_decisions)
        })
        .collect::<Vec<_>>();
    let count = |choice| {
        decisions
            .iter()
            .filter(|decision| decision.choice == choice)
            .count()
    };
    assert_eq!(count(PostRangePurgeChoice::LowerGradeLeft), 2_038);
    assert_eq!(count(PostRangePurgeChoice::LowerGradeRight), 698);
    assert_eq!(count(PostRangePurgeChoice::EqualLeftNoSeed), 45);
    assert_eq!(count(PostRangePurgeChoice::EqualPreserveLeft), 6);
    assert_eq!(count(PostRangePurgeChoice::EqualRightNoSeed), 0);
    assert_eq!(count(PostRangePurgeChoice::EqualShape), 0);
    assert_eq!(count(PostRangePurgeChoice::EqualBounds), 0);

    // These streams are committed by summary counts/hashes, but deliberately
    // absent from the compact file and therefore cannot be reconstructed by a
    // later compositor without the native seed/range and beam products.
    let check_rows = corpus
        .pages
        .iter()
        .flat_map(|page| &page.staffs)
        .map(|staff| staff.summary.duplicate_checks.count + staff.summary.overlap_checks.count)
        .sum::<usize>();
    let beam_inputs = corpus
        .pages
        .iter()
        .flat_map(|page| &page.systems)
        .map(|system| system.summary.beam_inputs)
        .sum::<usize>();
    let beam_checks = corpus
        .pages
        .iter()
        .flat_map(|page| &page.systems)
        .map(|system| system.summary.beam_checks.count)
        .sum::<usize>();
    assert_eq!(check_rows, 15_336);
    assert_eq!(beam_inputs, 191);
    assert_eq!(beam_checks, 10_053);
}

#[test]
fn body_commitment_rejects_even_a_semantically_parseable_mutation() {
    let mutated = ORACLE.replacen("family Bravura", "family Petaluma", 1);
    let error = parse_heads_post_range_corpus(&mutated).expect_err("mutated fixture must fail");
    assert!(error.to_string().contains("SHA-256 mismatch"));
}
