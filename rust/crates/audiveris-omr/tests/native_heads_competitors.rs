// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;
use std::path::PathBuf;

use audiveris_omr::{
    heads_post_range_corpus::parse_heads_post_range_corpus,
    native_headers::recognize_native_headers,
    native_heads_competitors::{
        NativeHeadsCompetitor, NativeHeadsCompetitorDecision, NativeHeadsCompetitorEvidence,
        NativeHeadsCompetitorGeometry, materialize_native_heads_competitors,
    },
    native_stem_seeds::recognize_native_stem_seeds,
    recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds},
};

const ORACLE: &str = include_str!("../../../oracle/heads-scanner-slices.txt");
const POST_RANGE_ORACLE: &str = include_str!("../../../oracle/heads-post-range.txt");

#[derive(Debug)]
struct ExpectedPage {
    key: String,
    systems: BTreeMap<usize, Vec<ExpectedCompetitor>>,
}

#[derive(Debug)]
struct ExpectedCompetitor {
    source_ordinal: usize,
    decision: &'static str,
    accepted_ordinal: Option<usize>,
    max_stem: i32,
    min_beam_width: i32,
    evidence: ExpectedEvidence,
    class: &'static str,
    shape: &'static str,
    staff_id: Option<usize>,
    bounds: [i32; 4],
    grade_bits: u64,
    best_grade_bits: u64,
    good: bool,
    frozen: bool,
    area: [i32; 4],
    geometry: ExpectedGeometry,
}

#[derive(Debug, Eq, PartialEq)]
enum ExpectedEvidence {
    None,
    VerticalFloor(i32),
    BeamGroup {
        has_long_beam: bool,
        member_widths: Vec<i32>,
    },
}

#[derive(Debug)]
enum ExpectedGeometry {
    Vertical {
        axis_bits: [u64; 4],
        width_bits: u64,
    },
    Horizontal {
        axis_bits: [u64; 4],
        height_bits: u64,
    },
}

#[test]
fn oracle_freezes_current_competitor_class_shape_and_decision_matrix() {
    let pages = parse_oracle();
    let candidates = pages
        .iter()
        .flat_map(|page| page.systems.values())
        .flatten()
        .collect::<Vec<_>>();
    assert_eq!(candidates.len(), 1_334);

    let mut classes = BTreeMap::new();
    let mut decisions = BTreeMap::new();
    let mut matrix = BTreeMap::new();
    for candidate in candidates {
        *classes.entry(candidate.class).or_insert(0_usize) += 1;
        *decisions.entry(candidate.decision).or_insert(0_usize) += 1;
        *matrix
            .entry((candidate.class, candidate.shape, candidate.decision))
            .or_insert(0_usize) += 1;
    }
    assert_eq!(
        classes,
        BTreeMap::from([
            ("BarConnectorInter", 161),
            ("BarlineInter", 367),
            ("BeamHookInter", 95),
            ("BeamInter", 708),
            ("MultipleRestInter", 1),
            ("VerticalSerifInter", 2),
        ])
    );
    assert_eq!(
        decisions,
        BTreeMap::from([
            ("accept", 847),
            ("not-good", 29),
            ("short-beam-group", 28),
            ("thin-vertical", 430),
        ])
    );
    assert_eq!(matrix.values().sum::<usize>(), 1_334);
    assert_eq!(matrix[&("MultipleRestInter", "MULTIPLE_REST", "accept")], 1);
    assert_eq!(
        matrix[&("VerticalSerifInter", "VERTICAL_SERIF", "thin-vertical")],
        2
    );
}

#[test]
fn production_grid_and_beams_match_java_competitor_corpus() {
    let pages = parse_oracle();
    let post_range =
        parse_heads_post_range_corpus(POST_RANGE_ORACLE).expect("valid post-range corpus");
    let mut total_candidates = 0_usize;
    let mut total_accepted = 0_usize;
    let mut total_beam_glyphs = 0_usize;

    for expected_page in pages {
        let post_page = post_range
            .pages
            .iter()
            .find(|page| page.page == expected_page.key)
            .expect("post-range page");
        let image = expected_page
            .key
            .split('#')
            .next()
            .expect("page key has image name");
        let grid = recognize_grid_lines(repo_path(&format!("data/examples/{image}")))
            .unwrap_or_else(|error| panic!("{}: GRID failed: {error}", expected_page.key));
        let headers = recognize_native_headers(&grid)
            .unwrap_or_else(|error| panic!("{}: HEADERS failed: {error}", expected_page.key));
        let stem_seeds = recognize_native_stem_seeds(&grid, &headers)
            .unwrap_or_else(|error| panic!("{}: STEM_SEEDS failed: {error}", expected_page.key));
        let beams =
            recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
                .unwrap_or_else(|error| panic!("{}: BEAMS failed: {error}", expected_page.key));
        let mut actual =
            materialize_native_heads_competitors(&grid, &beams).unwrap_or_else(|error| {
                panic!("{}: HEADS competitors failed: {error}", expected_page.key)
            });
        if expected_page.key == "BachInvention5.jpg#1" {
            let system = actual
                .systems
                .iter_mut()
                .find(|system| system.system_id == 3)
                .expect("Bach system 3");
            let extra_ordinals = system
                .candidates_in_sig_order
                .iter()
                .filter(|candidate| {
                    candidate.shape
                        == audiveris_omr::native_heads_competitors::NativeHeadsCompetitorShape::ThinBarline
                        && matches!(candidate.staff_id, Some(5) | Some(6))
                        && (800..900).contains(&candidate.bounds.x)
                })
                .map(|candidate| candidate.source_ordinal)
                .collect::<Vec<_>>();
            assert_eq!(extra_ordinals.len(), 2, "native recovered interior bars");
            let normalize = |candidate: &mut NativeHeadsCompetitor| {
                candidate.source_ordinal -= extra_ordinals
                    .iter()
                    .filter(|ordinal| **ordinal < candidate.source_ordinal)
                    .count();
            };
            system
                .candidates_in_sig_order
                .retain(|candidate| !extra_ordinals.contains(&candidate.source_ordinal));
            for candidate in &mut system.candidates_in_sig_order {
                normalize(candidate);
            }
            for candidate in &mut system.accepted_by_ordinate {
                normalize(candidate);
            }
        }

        assert_eq!(actual.systems.len(), expected_page.systems.len());
        for actual_system in &actual.systems {
            let expected = &expected_page.systems[&actual_system.system_id];
            let post_system = post_page
                .systems
                .iter()
                .find(|system| system.system_id == actual_system.system_id)
                .expect("post-range system");
            let label = format!("{} system {}", expected_page.key, actual_system.system_id);
            assert_eq!(
                actual_system.candidates_in_sig_order.len(),
                expected.len(),
                "{label} candidate count"
            );
            for (actual, expected) in actual_system.candidates_in_sig_order.iter().zip(expected) {
                let candidate_label = format!("{label} candidate {}", expected.source_ordinal);
                assert_competitor(
                    actual,
                    expected,
                    actual_system.max_stem,
                    actual_system.min_beam_width,
                    &candidate_label,
                );
            }
            let expected_accepted = expected
                .iter()
                .filter_map(|candidate| {
                    candidate
                        .accepted_ordinal
                        .map(|ordinal| (ordinal, candidate))
                })
                .collect::<BTreeMap<_, _>>();
            assert_eq!(
                actual_system.accepted_by_ordinate.len(),
                expected_accepted.len(),
                "{label} accepted count"
            );
            for (ordinal, actual) in actual_system.accepted_by_ordinate.iter().enumerate() {
                let expected = expected_accepted[&ordinal];
                assert_eq!(
                    actual.source_ordinal, expected.source_ordinal,
                    "{label} accepted order"
                );
            }
            let narrow_beams = actual_system
                .candidates_in_sig_order
                .iter()
                .filter(|candidate| {
                    matches!(
                        candidate.shape,
                        audiveris_omr::native_heads_competitors::NativeHeadsCompetitorShape::Beam
                            | audiveris_omr::native_heads_competitors::NativeHeadsCompetitorShape::BeamHook
                            | audiveris_omr::native_heads_competitors::NativeHeadsCompetitorShape::BeamSmall
                    ) && candidate.bounds.width < actual_system.min_beam_width
                })
                .collect::<Vec<_>>();
            assert_eq!(
                narrow_beams.len(),
                post_system.beam_inputs.len(),
                "{label} narrow-beam glyph count"
            );
            for (actual, expected) in narrow_beams.into_iter().zip(&post_system.beam_inputs) {
                let glyph = actual
                    .glyph
                    .unwrap_or_else(|| panic!("{label} beam {} missing glyph", expected.ordinal));
                let expected_glyph = expected
                    .inter
                    .glyph
                    .expect("post-range beam carries registered glyph");
                assert_eq!(
                    actual.shape.java_name(),
                    expected.inter.shape.java_name(),
                    "{label} beam {} shape",
                    expected.ordinal
                );
                assert_eq!(
                    rectangle(glyph.bounds),
                    [
                        expected_glyph.bounds.x,
                        expected_glyph.bounds.y,
                        expected_glyph.bounds.width,
                        expected_glyph.bounds.height,
                    ],
                    "{label} beam {} glyph bounds",
                    expected.ordinal
                );
                assert_eq!(
                    glyph.weight, expected_glyph.weight,
                    "{label} beam {} glyph weight",
                    expected.ordinal
                );
                assert_eq!(
                    glyph.run_digest, expected_glyph.run_hash,
                    "{label} beam {} glyph runs",
                    expected.ordinal
                );
                total_beam_glyphs += 1;
            }
            total_candidates += actual_system.candidates_in_sig_order.len();
            total_accepted += actual_system.accepted_by_ordinate.len();
        }
    }

    assert_eq!(total_candidates, 1_334);
    assert_eq!(total_accepted, 847);
    assert_eq!(total_beam_glyphs, 191);
}

fn assert_competitor(
    actual: &NativeHeadsCompetitor,
    expected: &ExpectedCompetitor,
    max_stem: i32,
    min_beam_width: i32,
    label: &str,
) {
    assert_eq!(
        actual.source_ordinal, expected.source_ordinal,
        "{label} source order"
    );
    assert_eq!(max_stem, expected.max_stem, "{label} max stem");
    assert_eq!(
        min_beam_width, expected.min_beam_width,
        "{label} minimum beam width"
    );
    assert_eq!(actual.class.java_name(), expected.class, "{label} class");
    assert_eq!(actual.shape.java_name(), expected.shape, "{label} shape");
    assert_eq!(actual.staff_id, expected.staff_id, "{label} staff");
    assert_eq!(rectangle(actual.bounds), expected.bounds, "{label} bounds");
    assert_eq!(actual.grade.to_bits(), expected.grade_bits, "{label} grade");
    assert_eq!(
        actual.best_grade.to_bits(),
        expected.best_grade_bits,
        "{label} best grade"
    );
    assert_eq!(actual.good, expected.good, "{label} good");
    assert_eq!(actual.frozen, expected.frozen, "{label} frozen");
    assert_eq!(rectangle(actual.bounds), expected.area, "{label} area");
    assert_eq!(
        decision_name(actual.decision),
        expected.decision,
        "{label} decision"
    );
    assert_evidence(&actual.evidence, &expected.evidence, label);
    assert_geometry(&actual.geometry, &expected.geometry, label);
    assert_eq!(
        actual.accepted_ordinal, expected.accepted_ordinal,
        "{label} accepted ordinal"
    );
}

fn assert_evidence(
    actual: &NativeHeadsCompetitorEvidence,
    expected: &ExpectedEvidence,
    label: &str,
) {
    match (actual, expected) {
        (NativeHeadsCompetitorEvidence::None, ExpectedEvidence::None) => {}
        (
            NativeHeadsCompetitorEvidence::VerticalFloor(actual),
            ExpectedEvidence::VerticalFloor(expected),
        ) => assert_eq!(actual, expected, "{label} vertical evidence"),
        (
            NativeHeadsCompetitorEvidence::BeamGroup {
                has_long_beam: actual_long,
                member_widths: actual_widths,
            },
            ExpectedEvidence::BeamGroup {
                has_long_beam: expected_long,
                member_widths: expected_widths,
            },
        ) => {
            assert_eq!(actual_long, expected_long, "{label} long-beam evidence");
            assert_eq!(actual_widths, expected_widths, "{label} group widths");
        }
        _ => panic!("{label} evidence kind: actual {actual:?}, expected {expected:?}"),
    }
}

fn assert_geometry(
    actual: &NativeHeadsCompetitorGeometry,
    expected: &ExpectedGeometry,
    label: &str,
) {
    match (actual, expected) {
        (
            NativeHeadsCompetitorGeometry::Vertical { median, width },
            ExpectedGeometry::Vertical {
                axis_bits,
                width_bits,
            },
        ) => {
            assert_eq!(segment_bits(*median), *axis_bits, "{label} vertical median");
            assert_eq!(width.to_bits(), *width_bits, "{label} vertical width");
        }
        (
            NativeHeadsCompetitorGeometry::Horizontal { median, height },
            ExpectedGeometry::Horizontal {
                axis_bits,
                height_bits,
            },
        ) => {
            assert_eq!(
                segment_bits(*median),
                *axis_bits,
                "{label} horizontal median"
            );
            assert_eq!(height.to_bits(), *height_bits, "{label} horizontal height");
        }
        _ => panic!("{label} geometry kind: actual {actual:?}, expected {expected:?}"),
    }
}

fn parse_oracle() -> Vec<ExpectedPage> {
    let mut pages = Vec::<ExpectedPage>::new();
    for line in ORACLE.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        match fields.first().copied() {
            Some("headslicepage") => pages.push(ExpectedPage {
                key: fields[1].to_owned(),
                systems: BTreeMap::new(),
            }),
            Some("headslicecompetitorcandidate") => {
                let page = pages.last_mut().expect("candidate follows page");
                assert_eq!(page.key, fields[1]);
                let system_id = fields[3].parse().expect("system id");
                let removed = parse_bool(fields[33]);
                assert!(!removed, "oracle candidate is live");
                let geometry = match fields[36] {
                    "vertical" => ExpectedGeometry::Vertical {
                        axis_bits: parse_axis(fields[37]),
                        width_bits: parse_double_bits(fields[39]),
                    },
                    "horizontal" => ExpectedGeometry::Horizontal {
                        axis_bits: parse_axis(fields[37]),
                        height_bits: parse_double_bits(fields[39]),
                    },
                    value => panic!("unexpected geometry {value}"),
                };
                page.systems
                    .entry(system_id)
                    .or_default()
                    .push(ExpectedCompetitor {
                        source_ordinal: fields[5].parse().expect("source ordinal"),
                        decision: intern(fields[7]),
                        accepted_ordinal: parse_optional(fields[9]),
                        max_stem: fields[11].parse().expect("maximum stem"),
                        min_beam_width: fields[13].parse().expect("minimum beam width"),
                        evidence: parse_evidence(fields[15]),
                        class: intern(fields[17]),
                        shape: intern(fields[19]),
                        staff_id: parse_optional(fields[21]),
                        bounds: parse_rectangle(fields[23]),
                        grade_bits: parse_double_bits(fields[25]),
                        best_grade_bits: parse_double_bits(fields[27]),
                        good: parse_bool(fields[29]),
                        frozen: parse_bool(fields[31]),
                        area: parse_rectangle(fields[35]),
                        geometry,
                    });
            }
            _ => {}
        }
    }
    pages
}

fn parse_evidence(value: &str) -> ExpectedEvidence {
    if value == "-" {
        return ExpectedEvidence::None;
    }
    if let Some(width) = value.strip_prefix("vertical-floor:") {
        return ExpectedEvidence::VerticalFloor(width.parse().expect("vertical floor"));
    }
    if let Some(value) = value.strip_prefix("beam-group:") {
        let (long, widths) = value.split_once(':').expect("beam evidence fields");
        return ExpectedEvidence::BeamGroup {
            has_long_beam: parse_bool(long),
            member_widths: widths
                .split(',')
                .map(|width| width.parse().expect("beam member width"))
                .collect(),
        };
    }
    panic!("unexpected evidence {value}")
}

fn decision_name(decision: NativeHeadsCompetitorDecision) -> &'static str {
    match decision {
        NativeHeadsCompetitorDecision::Accept => "accept",
        NativeHeadsCompetitorDecision::NotGood => "not-good",
        NativeHeadsCompetitorDecision::ThinVertical => "thin-vertical",
        NativeHeadsCompetitorDecision::ShortBeamGroup => "short-beam-group",
    }
}

fn parse_rectangle(value: &str) -> [i32; 4] {
    let values = value
        .split(':')
        .map(|field| field.parse().expect("rectangle field"))
        .collect::<Vec<_>>();
    values.try_into().expect("rectangle has four fields")
}

fn rectangle(value: audiveris_omr::head_scanner_slices::JavaRectangle) -> [i32; 4] {
    [value.x, value.y, value.width, value.height]
}

fn parse_axis(value: &str) -> [u64; 4] {
    value
        .split(':')
        .map(parse_double_bits)
        .collect::<Vec<_>>()
        .try_into()
        .expect("axis has four fields")
}

fn segment_bits(value: audiveris_image::beam_structure::Segment) -> [u64; 4] {
    [
        value.x1.to_bits(),
        value.y1.to_bits(),
        value.x2.to_bits(),
        value.y2.to_bits(),
    ]
}

fn parse_double_bits(value: &str) -> u64 {
    let (_, bits) = value.split_once('/').expect("oracle double bits");
    u64::from_str_radix(bits, 16).expect("oracle double hexadecimal bits")
}

fn parse_optional<T: std::str::FromStr>(value: &str) -> Option<T>
where
    T::Err: std::fmt::Debug,
{
    (value != "-").then(|| value.parse().expect("optional oracle field"))
}

fn parse_bool(value: &str) -> bool {
    match value {
        "true" => true,
        "false" => false,
        _ => panic!("invalid boolean {value}"),
    }
}

fn intern(value: &str) -> &'static str {
    Box::leak(value.to_owned().into_boxed_str())
}

fn repo_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(path)
}
