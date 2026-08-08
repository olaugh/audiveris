// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact corpus gate for raw Java `VerticalsBuilder.retrieveCandidates()`.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use audiveris_image::{run_table::Orientation, section::Section};
use audiveris_omr::{
    native_stem_seeds::recognize_raw_stem_seed_candidates, recognize::recognize_grid_lines,
};

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}

#[derive(Debug)]
struct OracleCandidate {
    members: Vec<String>,
    bounds: (usize, usize, usize, usize),
    weight: usize,
    start: (f64, f64),
    stop: (f64, f64),
    thickness: f64,
    distance: f64,
}

#[derive(Debug)]
struct OracleSystem {
    profile: i32,
    left: i32,
    right: i32,
    interline: i32,
    maximum_stem: i32,
    minimum_core: i32,
    minimum_side: f64,
    vertical_count: usize,
    vertical_hash: u64,
    sticker_count: usize,
    sticker_hash: u64,
    candidates: Vec<OracleCandidate>,
}

fn parse_oracle() -> Vec<(String, BTreeMap<usize, OracleSystem>)> {
    let text = std::fs::read_to_string(repo_path("rust/oracle/stem-seeds.txt"))
        .expect("the raw STEM_SEEDS oracle is checked in");
    let mut pages = Vec::<(String, BTreeMap<usize, OracleSystem>)>::new();
    for line in text.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        match fields.first().copied() {
            Some("stemseedsheet") => pages.push((fields[1].to_owned(), BTreeMap::new())),
            Some("stemseedinput") => {
                let page = pages.last_mut().expect("input follows sheet");
                assert_eq!(page.0, fields[1]);
                let system_id = fields[3].parse().unwrap();
                page.1.insert(
                    system_id,
                    OracleSystem {
                        profile: fields[5].parse().unwrap(),
                        left: fields[7].parse().unwrap(),
                        right: fields[9].parse().unwrap(),
                        interline: fields[11].parse().unwrap(),
                        maximum_stem: fields[13].parse().unwrap(),
                        minimum_core: fields[15].parse().unwrap(),
                        minimum_side: parse_hex_f64(fields[17]),
                        vertical_count: fields[19].parse().unwrap(),
                        vertical_hash: u64::from_str_radix(fields[20], 16).unwrap(),
                        sticker_count: fields[22].parse().unwrap(),
                        sticker_hash: u64::from_str_radix(fields[23], 16).unwrap(),
                        candidates: Vec::new(),
                    },
                );
            }
            Some("stemseedcandidate") => {
                let page = pages.last_mut().expect("candidate follows sheet");
                assert_eq!(page.0, fields[1]);
                let system_id = fields[3].parse().unwrap();
                let system = page.1.get_mut(&system_id).expect("candidate follows input");
                assert_eq!(system.candidates.len(), fields[5].parse().unwrap());
                system.candidates.push(OracleCandidate {
                    members: fields[7].split(',').map(str::to_owned).collect(),
                    bounds: (
                        fields[9].parse().unwrap(),
                        fields[10].parse().unwrap(),
                        fields[11].parse().unwrap(),
                        fields[12].parse().unwrap(),
                    ),
                    weight: fields[14].parse().unwrap(),
                    start: (parse_hex_f64(fields[16]), parse_hex_f64(fields[17])),
                    stop: (parse_hex_f64(fields[19]), parse_hex_f64(fields[20])),
                    thickness: parse_hex_f64(fields[22]),
                    distance: parse_hex_f64(fields[24]),
                });
            }
            _ => {}
        }
    }
    pages
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

fn section_hash(sections: &[Section]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for section in sections {
        let bounds = section.bounds();
        let tag = match section.orientation() {
            Orientation::Horizontal => 'h',
            Orientation::Vertical => 'v',
        };
        let mut row = format!(
            "{tag}{} {} {} {} {} {} {}",
            section.id(),
            section.first_pos(),
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            section.weight()
        );
        for run in section.runs() {
            row.push_str(&format!(" {}:{}", run.start, run.length));
        }
        row.push('\n');
        for byte in row.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
    }
    hash
}

fn member_name(section: &Section) -> String {
    let tag = match section.orientation() {
        Orientation::Horizontal => 'h',
        Orientation::Vertical => 'v',
    };
    format!("{tag}{}", section.id())
}

fn assert_same_f64(context: &str, actual: f64, expected: f64) {
    assert_eq!(
        actual.to_bits(),
        expected.to_bits(),
        "{context}: actual {actual:?}, expected {expected:?}"
    );
}

#[test]
fn raw_vertical_stick_factory_matches_java_corpus_exactly() {
    let pages = parse_oracle();
    let mut checked_systems = 0;
    let mut checked_candidates = 0;
    for (page, oracle_systems) in pages {
        let (file, sheet) = page.split_once('#').expect("page#sheet");
        assert_eq!(sheet, "1", "example fixture is a one-sheet image");
        let grid = recognize_grid_lines(repo_path(&format!("data/examples/{file}")))
            .unwrap_or_else(|error| panic!("{page}: GRID failed: {error}"));
        let actual = recognize_raw_stem_seed_candidates(&grid)
            .unwrap_or_else(|error| panic!("{page}: STEM_SEEDS candidates failed: {error}"));
        assert_eq!(actual.systems.len(), oracle_systems.len(), "{page}");
        for system in &actual.systems {
            let expected = &oracle_systems[&system.system_id];
            let context = format!("{page} system {}", system.system_id);
            assert_eq!(system.profile, expected.profile, "{context}");
            assert_eq!(system.left, expected.left, "{context}");
            assert_eq!(system.right, expected.right, "{context}");
            assert_eq!(system.interline, expected.interline, "{context}");
            assert_eq!(
                system.maximum_stem_thickness, expected.maximum_stem,
                "{context}"
            );
            assert_eq!(
                system.minimum_core_section_length, expected.minimum_core,
                "{context}"
            );
            assert_same_f64(
                &format!("{context} minimum side"),
                system.minimum_side_ratio,
                expected.minimum_side,
            );
            assert_eq!(
                system.vertical_sections.len(),
                expected.vertical_count,
                "{context} vertical count"
            );
            assert_eq!(
                section_hash(&system.vertical_sections),
                expected.vertical_hash,
                "{context} vertical digest"
            );
            assert_eq!(
                system.horizontal_stickers.len(),
                expected.sticker_count,
                "{context} sticker count"
            );
            assert_eq!(
                section_hash(&system.horizontal_stickers),
                expected.sticker_hash,
                "{context} sticker digest"
            );
            assert_eq!(
                system.candidates.len(),
                expected.candidates.len(),
                "{context} candidate count"
            );
            for (ordinal, (candidate, expected)) in system
                .candidates
                .iter()
                .zip(&expected.candidates)
                .enumerate()
            {
                let context = format!("{context} candidate {ordinal}");
                assert_eq!(
                    candidate
                        .sections()
                        .iter()
                        .map(member_name)
                        .collect::<Vec<_>>(),
                    expected.members,
                    "{context} members"
                );
                assert_eq!(candidate.weight(), expected.weight, "{context} weight");
                let geometry = candidate
                    .straight_geometry()
                    .unwrap_or_else(|error| panic!("{context}: {error}"));
                assert_eq!(
                    (
                        geometry.bounds.x,
                        geometry.bounds.y,
                        geometry.bounds.width,
                        geometry.bounds.height
                    ),
                    expected.bounds,
                    "{context} bounds"
                );
                assert_same_f64(
                    &format!("{context} start x"),
                    geometry.start.0,
                    expected.start.0,
                );
                assert_same_f64(
                    &format!("{context} start y"),
                    geometry.start.1,
                    expected.start.1,
                );
                assert_same_f64(
                    &format!("{context} stop x"),
                    geometry.stop.0,
                    expected.stop.0,
                );
                assert_same_f64(
                    &format!("{context} stop y"),
                    geometry.stop.1,
                    expected.stop.1,
                );
                assert_same_f64(
                    &format!("{context} thickness"),
                    geometry.mean_thickness,
                    expected.thickness,
                );
                assert_same_f64(
                    &format!("{context} distance"),
                    geometry.mean_distance,
                    expected.distance,
                );
                checked_candidates += 1;
            }
            checked_systems += 1;
        }
    }
    assert_eq!(checked_systems, 30);
    assert_eq!(checked_candidates, 2_425);
}
