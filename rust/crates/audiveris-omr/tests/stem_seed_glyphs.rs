// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact corpus gate for `VerticalsBuilder.checkVerticals(Collection)`.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use audiveris_omr::{
    native_headers::recognize_native_headers,
    native_stem_seeds::{
        NativeStemSeedDecision, NativeStemSeedSkipReason, STEM_SEEDS_MINIMUM_GRADE,
        recognize_native_stem_seeds,
    },
    recognize::recognize_grid_lines,
    stem_seeds_step::NativeStemImpacts,
};

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}

#[derive(Debug)]
struct OracleCheck {
    values: [f64; 7],
    weights: [f64; 7],
    impacts: [f64; 7],
    gap: i32,
    black: i32,
    white: i32,
    grade: f64,
    accepted: bool,
}

#[derive(Debug)]
struct OracleDecision {
    ordinal: usize,
    center: (f64, f64),
    staff_id: Option<usize>,
    header_stop: Option<i32>,
    tablature: bool,
    skip: Option<NativeStemSeedSkipReason>,
    threshold: Option<f64>,
    check: Option<OracleCheck>,
}

#[derive(Debug)]
struct OracleGlyph {
    glyph_index: usize,
    ordinal: usize,
    bounds: (usize, usize, usize, usize),
    weight: usize,
    start: (f64, f64),
    stop: (f64, f64),
    thickness: f64,
    distance: f64,
    run_count: usize,
    run_digest: u64,
}

#[derive(Debug)]
struct OracleSystem {
    profile: i32,
    threshold: f64,
    candidates: usize,
    checked: usize,
    accepted: usize,
    decisions: Vec<OracleDecision>,
    glyphs: Vec<OracleGlyph>,
}

fn parse_oracle() -> Vec<(String, BTreeMap<usize, OracleSystem>)> {
    let text = std::fs::read_to_string(repo_path("rust/oracle/stem-seed-glyphs.txt"))
        .expect("the accepted STEM_SEEDS oracle is checked in");
    let mut pages = Vec::<(String, BTreeMap<usize, OracleSystem>)>::new();
    for line in text.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        match fields.first().copied() {
            Some("stemseedacceptsheet") => {
                pages.push((fields[1].to_owned(), BTreeMap::new()));
            }
            Some("stemseedacceptinput") => {
                let page = pages.last_mut().expect("input follows sheet");
                assert_eq!(page.0, fields[1]);
                page.1.insert(
                    fields[3].parse().unwrap(),
                    OracleSystem {
                        profile: fields[5].parse().unwrap(),
                        threshold: parse_hex_f64(fields[7]),
                        candidates: fields[9].parse().unwrap(),
                        checked: fields[11].parse().unwrap(),
                        accepted: fields[13].parse().unwrap(),
                        decisions: Vec::new(),
                        glyphs: Vec::new(),
                    },
                );
            }
            Some("stemseedskip") | Some("stemseedcheck") => {
                let page = pages.last_mut().expect("decision follows sheet");
                assert_eq!(page.0, fields[1]);
                let system = page
                    .1
                    .get_mut(&fields[3].parse().unwrap())
                    .expect("decision follows input");
                let ordinal = fields[5].parse().unwrap();
                assert_eq!(system.decisions.len(), ordinal);
                let staff: i32 = fields[10].parse().unwrap();
                let header_stop: i32 = fields[12].parse().unwrap();
                let checked = fields[0] == "stemseedcheck";
                let (skip, threshold, check) = if checked {
                    let (value_names, values) = parse_named_values(fields[18]);
                    let (impact_names, weights, impacts) = parse_named_impacts(fields[20]);
                    assert_eq!(value_names, impact_names);
                    assert_eq!(value_names, CHECK_NAMES);
                    (
                        None,
                        Some(parse_hex_f64(fields[16])),
                        Some(OracleCheck {
                            values,
                            weights,
                            impacts,
                            gap: fields[22].parse().unwrap(),
                            black: fields[24].parse().unwrap(),
                            white: fields[26].parse().unwrap(),
                            grade: parse_hex_f64(fields[28]),
                            accepted: fields[30].parse().unwrap(),
                        }),
                    )
                } else {
                    let reason = match fields[16] {
                        "no-staff" => NativeStemSeedSkipReason::NoStaff,
                        "tablature" => NativeStemSeedSkipReason::Tablature,
                        "header" => NativeStemSeedSkipReason::Header,
                        other => panic!("unknown skip reason {other}"),
                    };
                    (Some(reason), None, None)
                };
                system.decisions.push(OracleDecision {
                    ordinal,
                    center: (parse_hex_f64(fields[7]), parse_hex_f64(fields[8])),
                    staff_id: usize::try_from(staff).ok(),
                    header_stop: (header_stop >= 0).then_some(header_stop),
                    tablature: fields[14].parse().unwrap(),
                    skip,
                    threshold,
                    check,
                });
            }
            Some("stemseedglyph") => {
                let page = pages.last_mut().expect("glyph follows sheet");
                assert_eq!(page.0, fields[1]);
                assert_eq!(fields[9], "VERTICAL_SEED");
                assert_eq!(fields[11], "true");
                let system = page
                    .1
                    .get_mut(&fields[3].parse().unwrap())
                    .expect("glyph follows input");
                let glyph_index = fields[5].parse().unwrap();
                assert_eq!(system.glyphs.len(), glyph_index);
                system.glyphs.push(OracleGlyph {
                    glyph_index,
                    ordinal: fields[7].parse().unwrap(),
                    bounds: (
                        fields[13].parse().unwrap(),
                        fields[14].parse().unwrap(),
                        fields[15].parse().unwrap(),
                        fields[16].parse().unwrap(),
                    ),
                    weight: fields[18].parse().unwrap(),
                    start: (parse_hex_f64(fields[20]), parse_hex_f64(fields[21])),
                    stop: (parse_hex_f64(fields[23]), parse_hex_f64(fields[24])),
                    thickness: parse_hex_f64(fields[26]),
                    distance: parse_hex_f64(fields[28]),
                    run_count: fields[30].parse().unwrap(),
                    run_digest: u64::from_str_radix(fields[31], 16).unwrap(),
                });
            }
            _ => {}
        }
    }
    pages
}

const CHECK_NAMES: [&str; 7] = [
    "Slope",
    "Straight",
    "Length",
    "Clean",
    "Black",
    "BlackRatio",
    "Gap",
];

fn parse_named_values(text: &str) -> ([&str; 7], [f64; 7]) {
    let entries = text.split(',').collect::<Vec<_>>();
    assert_eq!(entries.len(), 7);
    let names = std::array::from_fn(|index| entries[index].split_once(':').unwrap().0);
    let values =
        std::array::from_fn(|index| parse_hex_f64(entries[index].split_once(':').unwrap().1));
    (names, values)
}

fn parse_named_impacts(text: &str) -> ([&str; 7], [f64; 7], [f64; 7]) {
    let entries = text.split(',').collect::<Vec<_>>();
    assert_eq!(entries.len(), 7);
    let parts = entries
        .iter()
        .map(|entry| entry.split(':').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let names = std::array::from_fn(|index| parts[index][0]);
    let weights = std::array::from_fn(|index| parse_hex_f64(parts[index][1]));
    let impacts = std::array::from_fn(|index| parse_hex_f64(parts[index][2]));
    (names, weights, impacts)
}

fn parse_hex_f64(text: &str) -> f64 {
    let (negative, text) = text
        .strip_prefix('-')
        .map_or((false, text), |rest| (true, rest));
    let (mantissa, exponent) = text.split_once('p').expect("Java hex float has exponent");
    let exponent: i32 = exponent.parse().unwrap();
    let mantissa = mantissa.strip_prefix("0x").expect("Java hex float prefix");
    let (whole, fraction) = mantissa.split_once('.').expect("Java hex float point");
    let digits = format!("{whole}{fraction}");
    let integer = u64::from_str_radix(&digits, 16).unwrap();
    let value = integer as f64 * 2.0_f64.powi(exponent - (4 * fraction.len() as i32));
    if negative { -value } else { value }
}

fn impacts(value: NativeStemImpacts) -> [f64; 7] {
    [
        value.slope,
        value.straight,
        value.length,
        value.clean,
        value.black,
        value.black_ratio,
        value.gap,
    ]
}

fn assert_same_f64(context: &str, actual: f64, expected: f64) {
    assert_eq!(
        actual.to_bits(),
        expected.to_bits(),
        "{context}: actual {actual:?}, expected {expected:?}"
    );
}

fn assert_same_f64s(context: &str, actual: [f64; 7], expected: [f64; 7]) {
    for (index, (actual, expected)) in actual.into_iter().zip(expected).enumerate() {
        assert_same_f64(
            &format!("{context} {}", CHECK_NAMES[index]),
            actual,
            expected,
        );
    }
}

#[test]
fn native_stem_seed_acceptance_matches_java_corpus_exactly() {
    let pages = parse_oracle();
    let mut total_systems = 0;
    let mut total_candidates = 0;
    let mut total_checked = 0;
    let mut total_accepted = 0;
    let mut total_skipped = 0;
    let mut total_rejected = 0;

    for (page, oracle_systems) in pages {
        let (file, sheet) = page.split_once('#').expect("page#sheet");
        assert_eq!(sheet, "1");
        let grid = recognize_grid_lines(repo_path(&format!("data/examples/{file}")))
            .unwrap_or_else(|error| panic!("{page}: GRID failed: {error}"));
        let headers = recognize_native_headers(&grid)
            .unwrap_or_else(|error| panic!("{page}: HEADERS failed: {error}"));
        let actual = recognize_native_stem_seeds(&grid, &headers)
            .unwrap_or_else(|error| panic!("{page}: STEM_SEEDS failed: {error}"));
        assert_eq!(actual.systems.len(), oracle_systems.len(), "{page}");

        for system in &actual.systems {
            let expected = &oracle_systems[&system.raw.system_id];
            let context = format!("{page} system {}", system.raw.system_id);
            assert_eq!(system.raw.profile, expected.profile, "{context}");
            assert_same_f64(
                &format!("{context} threshold"),
                STEM_SEEDS_MINIMUM_GRADE,
                expected.threshold,
            );
            assert_eq!(
                system.raw.candidates.len(),
                expected.candidates,
                "{context}"
            );
            assert_eq!(
                system.decisions.len(),
                expected.decisions.len(),
                "{context}"
            );
            assert_eq!(
                system.registered_glyphs.len(),
                expected.checked,
                "{context}"
            );
            assert_eq!(system.free_glyphs.len(), expected.accepted, "{context}");
            assert_eq!(system.free_glyphs.len(), expected.glyphs.len(), "{context}");

            for (actual, expected) in system.decisions.iter().zip(&expected.decisions) {
                let gate = match actual {
                    NativeStemSeedDecision::Skipped { gate, .. }
                    | NativeStemSeedDecision::Checked { gate, .. } => gate,
                };
                let decision_context = format!("{context} candidate {}", expected.ordinal);
                assert_eq!(gate.ordinal, expected.ordinal, "{decision_context}");
                assert_same_f64(
                    &format!("{decision_context} center x"),
                    gate.center.0,
                    expected.center.0,
                );
                assert_same_f64(
                    &format!("{decision_context} center y"),
                    gate.center.1,
                    expected.center.1,
                );
                assert_eq!(gate.staff_id, expected.staff_id, "{decision_context}");
                assert_eq!(gate.header_stop, expected.header_stop, "{decision_context}");
                assert_eq!(gate.tablature, expected.tablature, "{decision_context}");

                match (actual, expected.skip, expected.check.as_ref()) {
                    (
                        NativeStemSeedDecision::Skipped { reason, .. },
                        Some(expected_reason),
                        None,
                    ) => assert_eq!(*reason, expected_reason, "{decision_context}"),
                    (
                        NativeStemSeedDecision::Checked {
                            threshold,
                            weights,
                            check,
                            registered_glyph_index,
                            accepted,
                            free_glyph_index,
                            ..
                        },
                        None,
                        Some(expected_check),
                    ) => {
                        assert_same_f64(
                            &format!("{decision_context} threshold"),
                            *threshold,
                            expected.threshold.unwrap(),
                        );
                        assert_same_f64s(
                            &format!("{decision_context} values"),
                            impacts(check.values),
                            expected_check.values,
                        );
                        assert_same_f64s(
                            &format!("{decision_context} weights"),
                            impacts(*weights),
                            expected_check.weights,
                        );
                        assert_same_f64s(
                            &format!("{decision_context} impacts"),
                            impacts(check.impacts),
                            expected_check.impacts,
                        );
                        assert_eq!(check.counts.largest_gap, expected_check.gap);
                        assert_eq!(check.counts.black, expected_check.black);
                        assert_eq!(check.counts.white, expected_check.white);
                        assert_same_f64(
                            &format!("{decision_context} grade"),
                            check.grade,
                            expected_check.grade,
                        );
                        assert_eq!(*accepted, expected_check.accepted, "{decision_context}");
                        assert!(*registered_glyph_index < system.registered_glyphs.len());
                        assert_eq!(free_glyph_index.is_some(), *accepted, "{decision_context}");
                        if *accepted {
                            total_accepted += 1;
                        } else {
                            total_rejected += 1;
                        }
                        total_checked += 1;
                    }
                    other => panic!("{decision_context}: decision shape differs: {other:?}"),
                }
            }

            for (actual, expected) in system.free_glyphs.iter().zip(&expected.glyphs) {
                let glyph_context = format!("{context} glyph {}", expected.glyph_index);
                assert_eq!(actual.source_ordinal, expected.ordinal, "{glyph_context}");
                assert!(actual.vertical_seed_group, "{glyph_context}");
                assert!(actual.free, "{glyph_context}");
                assert_eq!(
                    (
                        actual.bounds.x,
                        actual.bounds.y,
                        actual.bounds.width,
                        actual.bounds.height,
                    ),
                    expected.bounds,
                    "{glyph_context}"
                );
                assert_eq!(actual.weight, expected.weight, "{glyph_context}");
                assert_same_f64(
                    &format!("{glyph_context} start x"),
                    actual.start.0,
                    expected.start.0,
                );
                assert_same_f64(
                    &format!("{glyph_context} start y"),
                    actual.start.1,
                    expected.start.1,
                );
                assert_same_f64(
                    &format!("{glyph_context} stop x"),
                    actual.stop.0,
                    expected.stop.0,
                );
                assert_same_f64(
                    &format!("{glyph_context} stop y"),
                    actual.stop.1,
                    expected.stop.1,
                );
                assert_same_f64(
                    &format!("{glyph_context} thickness"),
                    actual.mean_thickness,
                    expected.thickness,
                );
                assert_same_f64(
                    &format!("{glyph_context} distance"),
                    actual.mean_distance,
                    expected.distance,
                );
                assert_eq!(actual.run_count(), expected.run_count, "{glyph_context}");
                assert_eq!(actual.run_digest(), expected.run_digest, "{glyph_context}");
            }

            total_systems += 1;
            total_candidates += expected.candidates;
            total_skipped += expected.candidates - expected.checked;
        }
    }

    assert_eq!(total_systems, 30);
    assert_eq!(total_candidates, 2_425);
    assert_eq!(total_checked, 2_003);
    assert_eq!(total_skipped, 422);
    assert_eq!(total_rejected, 97);
    assert_eq!(total_accepted, 1_906);
}
