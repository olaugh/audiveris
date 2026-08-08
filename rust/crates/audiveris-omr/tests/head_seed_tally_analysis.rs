// SPDX-License-Identifier: AGPL-3.0-or-later

//! Compact Java differential for `HeadSeedTally.analyze`.

use std::collections::BTreeMap;

// This pure module is intentionally testable before its crate-root export is
// wired into the concurrently maintained port surface.
#[path = "../src/head_seed_tally_analysis.rs"]
mod head_seed_tally_analysis;

use head_seed_tally_analysis::{
    HeadSeedScaleEntry, HeadSeedTallySample, HeadSeedTallySide, analyze_head_seed_tallies,
};

const ORACLE: &str = include_str!("../../../oracle/heads-post-range.txt");

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Shape {
    // Relative Java `Shape` declaration order for the two corpus shapes.
    NoteheadVoid,
    NoteheadBlack,
}

#[derive(Debug, Default)]
struct Page {
    samples: Vec<HeadSeedTallySample<Shape>>,
    expected: Vec<ExpectedScale>,
    next_ordinal: usize,
}

#[derive(Clone, Copy, Debug)]
struct ExpectedScale {
    shape: Shape,
    side: HeadSeedTallySide,
    dx_bits: u64,
}

#[test]
fn java_post_range_tallies_reproduce_all_head_seed_scale_bits() {
    let mut pages = BTreeMap::<String, Page>::new();
    let mut page_declarations = 0;
    let mut corpus_summary = None;

    for line in ORACLE.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        match fields.first().copied() {
            Some("headpostpage") => {
                assert_eq!(fields.len(), 8, "bad page row: {line}");
                page_declarations += 1;
                assert!(
                    pages
                        .insert(fields[1].to_owned(), Page::default())
                        .is_none()
                );
            }
            Some("headpostscaleinput") => {
                assert_eq!(fields.len(), 14, "bad scale-input row: {line}");
                assert_eq!(fields[2], "system");
                parse_usize(fields[3]);
                assert_eq!(fields[4], "ordinal");
                let ordinal = parse_usize(fields[5]);
                assert_eq!(fields[6], "side");
                let side = parse_side(fields[7]);
                assert_eq!(fields[8], "entry");
                parse_usize(fields[9]);
                assert_eq!(fields[10], "shape");
                let shape = parse_shape(fields[11]);
                assert_eq!(fields[12], "dx");
                let dx_bits = parse_double_bits(fields[13]);
                let page = pages
                    .get_mut(fields[1])
                    .unwrap_or_else(|| panic!("input before page declaration: {line}"));
                assert_eq!(ordinal, page.next_ordinal, "non-sequential input: {line}");
                page.next_ordinal += 1;
                page.samples.push(HeadSeedTallySample {
                    shape,
                    side,
                    dx: f64::from_bits(dx_bits),
                    removed: false,
                });
            }
            Some("headpostscale") => {
                assert_eq!(fields.len(), 8, "bad scale row: {line}");
                assert_eq!(fields[2], "shape");
                assert_eq!(fields[4], "side");
                assert_eq!(fields[6], "dx");
                let expected = ExpectedScale {
                    shape: parse_shape(fields[3]),
                    side: parse_side(fields[5]),
                    dx_bits: parse_double_bits(fields[7]),
                };
                pages
                    .get_mut(fields[1])
                    .unwrap_or_else(|| panic!("scale before page declaration: {line}"))
                    .expected
                    .push(expected);
            }
            Some("headpostcorpussummary") => corpus_summary = Some(line),
            _ => {}
        }
    }

    assert_eq!(page_declarations, 8);
    assert_eq!(pages.len(), 8);
    assert_eq!(
        pages.values().map(|page| page.samples.len()).sum::<usize>(),
        1_451
    );
    assert_eq!(
        pages
            .values()
            .map(|page| page.expected.len())
            .sum::<usize>(),
        18
    );
    let summary = corpus_summary.expect("missing corpus summary");
    assert!(summary.contains(" scaleInputRows 1451 scaleRows 18 "));

    for (page_name, page) in pages {
        let actual = analyze_head_seed_tallies(&page.samples);
        assert_eq!(actual.len(), page.expected.len(), "{page_name}");
        for (actual, expected) in actual.iter().zip(&page.expected) {
            assert_scale(page_name.as_str(), actual, *expected);
        }
    }
}

fn assert_scale(page: &str, actual: &HeadSeedScaleEntry<Shape>, expected: ExpectedScale) {
    assert_eq!(actual.shape, expected.shape, "shape on {page}");
    assert_eq!(actual.side, expected.side, "side on {page}");
    assert!(actual.cardinality >= 10, "quorum on {page}");
    assert_eq!(actual.dx.to_bits(), expected.dx_bits, "dx bits on {page}");
}

fn parse_shape(value: &str) -> Shape {
    match value {
        "NOTEHEAD_VOID" => Shape::NoteheadVoid,
        "NOTEHEAD_BLACK" => Shape::NoteheadBlack,
        _ => panic!("unexpected tally shape {value}"),
    }
}

fn parse_side(value: &str) -> HeadSeedTallySide {
    match value {
        "LEFT" => HeadSeedTallySide::Left,
        "RIGHT" => HeadSeedTallySide::Right,
        _ => panic!("unexpected horizontal side {value}"),
    }
}

fn parse_usize(value: &str) -> usize {
    value
        .parse()
        .unwrap_or_else(|_| panic!("invalid usize {value}"))
}

fn parse_double_bits(value: &str) -> u64 {
    let (java_hex, raw_bits) = value
        .split_once('/')
        .unwrap_or_else(|| panic!("invalid oracle double {value}"));
    assert!(
        java_hex.contains("0x") || java_hex == "NaN" || java_hex.ends_with("Infinity"),
        "invalid Java double rendering {value}"
    );
    u64::from_str_radix(raw_bits, 16).unwrap_or_else(|_| panic!("invalid double bits {value}"))
}
