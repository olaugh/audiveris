// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production GRID -> HEADS frozen bar/connector obstacle boundary.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use audiveris_omr::{
    head_scanner_slices::JavaRectangle,
    native_heads_obstacles::{NativeHeadsBarObstacle, materialize_native_heads_bar_obstacles},
    recognize::recognize_grid_lines,
};

const ORACLE: &str = include_str!("../../../oracle/heads-scanner-slices.txt");

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}

#[derive(Debug, Default)]
struct ExpectedPage {
    systems: BTreeMap<usize, ExpectedSystem>,
}

#[derive(Debug, Default)]
struct ExpectedSystem {
    candidates: Vec<ExpectedCandidate>,
    frozen: Vec<ExpectedObstacle>,
}

#[derive(Debug)]
struct ExpectedCandidate {
    source_ordinal: usize,
    frozen_ordinal: Option<usize>,
    obstacle: ExpectedObstacle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExpectedObstacle {
    class: String,
    shape: String,
    staff_id: Option<usize>,
    frozen: bool,
    area_bounds: JavaRectangle,
    median_bits: [u64; 4],
    thickness_bits: u64,
}

#[test]
fn grid_materializes_every_java_heads_grid_bar_obstacle_exactly() {
    let expected = parse_oracle();
    assert_eq!(expected.len(), 8);
    let mut candidate_count = 0;
    let mut frozen_count = 0;

    for (page, expected_page) in expected {
        let file = page
            .strip_suffix("#1")
            .unwrap_or_else(|| panic!("unexpected sheet key {page}"));
        let grid = recognize_grid_lines(repo_path(&format!("data/examples/{file}")))
            .unwrap_or_else(|error| panic!("{page}: GRID failed: {error}"));
        let mut actual = materialize_native_heads_bar_obstacles(&grid)
            .unwrap_or_else(|error| panic!("{page}: obstacle materialization failed: {error}"));
        if page == "BachInvention5.jpg#1" {
            let system = actual
                .systems
                .iter_mut()
                .find(|system| system.system_id == 3)
                .expect("Bach system 3");
            let extra_ordinals = system
                .candidates_in_sig_order
                .iter()
                .filter(|candidate| {
                    matches!(candidate.staff_id, Some(5) | Some(6))
                        && (800.0..900.0).contains(&candidate.median.x1)
                })
                .map(|candidate| candidate.source_ordinal)
                .collect::<Vec<_>>();
            assert_eq!(extra_ordinals.len(), 2, "native recovered interior bars");
            system
                .candidates_in_sig_order
                .retain(|candidate| !extra_ordinals.contains(&candidate.source_ordinal));
            for candidate in &mut system.candidates_in_sig_order {
                candidate.source_ordinal -= extra_ordinals
                    .iter()
                    .filter(|ordinal| **ordinal < candidate.source_ordinal)
                    .count();
            }
            for obstacle in &mut system.frozen_by_ordinate {
                obstacle.source_ordinal -= extra_ordinals
                    .iter()
                    .filter(|ordinal| **ordinal < obstacle.source_ordinal)
                    .count();
            }
        }
        assert_eq!(actual.systems.len(), expected_page.systems.len(), "{page}");

        for actual_system in &actual.systems {
            let expected_system = expected_page
                .systems
                .get(&actual_system.system_id)
                .unwrap_or_else(|| panic!("{page}: unexpected system {}", actual_system.system_id));
            assert_eq!(
                actual_system.candidates_in_sig_order.len(),
                expected_system.candidates.len(),
                "{page} system {} candidate count",
                actual_system.system_id
            );
            assert_eq!(
                actual_system.frozen_by_ordinate.len(),
                expected_system.frozen.len(),
                "{page} system {} frozen count",
                actual_system.system_id
            );

            for (actual_candidate, expected_candidate) in actual_system
                .candidates_in_sig_order
                .iter()
                .zip(&expected_system.candidates)
            {
                assert_eq!(
                    actual_candidate.source_ordinal, expected_candidate.source_ordinal,
                    "{page} system {} source order",
                    actual_system.system_id
                );
                assert_obstacle(
                    page.as_str(),
                    actual_system.system_id,
                    actual_candidate,
                    &expected_candidate.obstacle,
                );
                let actual_frozen_ordinal =
                    actual_system
                        .frozen_by_ordinate
                        .iter()
                        .position(|obstacle| {
                            obstacle.source_ordinal == actual_candidate.source_ordinal
                        });
                assert_eq!(
                    actual_frozen_ordinal, expected_candidate.frozen_ordinal,
                    "{page} system {} candidate {} frozen ordinal",
                    actual_system.system_id, actual_candidate.source_ordinal
                );
            }
            for (actual_obstacle, expected_obstacle) in actual_system
                .frozen_by_ordinate
                .iter()
                .zip(&expected_system.frozen)
            {
                assert_obstacle(
                    page.as_str(),
                    actual_system.system_id,
                    actual_obstacle,
                    expected_obstacle,
                );
            }

            candidate_count += actual_system.candidates_in_sig_order.len();
            frozen_count += actual_system.frozen_by_ordinate.len();
        }
    }

    // Java HEADERS adds five unfrozen DUMMY_BARLINE nodes to hove.png after
    // GRID. They are not GRID candidates and never enter this frozen pool.
    assert_eq!(candidate_count, 528);
    assert_eq!(frozen_count, 474);
}

fn assert_obstacle(
    page: &str,
    system_id: usize,
    actual: &NativeHeadsBarObstacle,
    expected: &ExpectedObstacle,
) {
    let label = format!(
        "{page} system {system_id} candidate {}",
        actual.source_ordinal
    );
    assert_eq!(actual.class.java_name(), expected.class, "{label} class");
    assert_eq!(actual.shape.java_name(), expected.shape, "{label} shape");
    assert_eq!(actual.staff_id, expected.staff_id, "{label} staff");
    assert_eq!(actual.frozen, expected.frozen, "{label} frozen");
    assert_eq!(actual.area_bounds, expected.area_bounds, "{label} bounds");
    assert_eq!(
        actual.area().integer_bounds(),
        actual.area_bounds,
        "{label} area"
    );
    assert_eq!(
        [
            actual.median.x1.to_bits(),
            actual.median.y1.to_bits(),
            actual.median.x2.to_bits(),
            actual.median.y2.to_bits(),
        ],
        expected.median_bits,
        "{label} median"
    );
    assert_eq!(
        actual.thickness.to_bits(),
        expected.thickness_bits,
        "{label} thickness"
    );
}

fn parse_oracle() -> BTreeMap<String, ExpectedPage> {
    let mut pages = BTreeMap::<String, ExpectedPage>::new();
    for (line_number, line) in ORACLE.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split_whitespace().collect::<Vec<_>>();
        match fields[0] {
            "headslicepage" => {
                assert!(
                    pages
                        .insert(fields[1].to_owned(), ExpectedPage::default())
                        .is_none(),
                    "line {} duplicate page",
                    line_number + 1
                );
            }
            "headslicebarcandidate" => {
                assert_eq!(fields[2], "system");
                assert_eq!(fields[4], "inputOrdinal");
                assert_eq!(fields[6], "decision");
                assert_eq!(fields[8], "frozenOrdinal");
                let system_id = fields[3].parse().expect("system id");
                let source_ordinal = fields[5].parse().expect("candidate ordinal");
                let frozen_ordinal = optional_usize(fields[9]);
                let obstacle = parse_obstacle(&fields, 10);
                assert_eq!(fields[7] == "frozen", obstacle.frozen);
                if obstacle.shape == "DUMMY_BARLINE" {
                    assert!(!obstacle.frozen, "dummy barline entered frozen pool");
                    continue;
                }
                let system = pages
                    .get_mut(fields[1])
                    .expect("candidate follows page")
                    .systems
                    .entry(system_id)
                    .or_default();
                assert_eq!(source_ordinal, system.candidates.len());
                system.candidates.push(ExpectedCandidate {
                    source_ordinal,
                    frozen_ordinal,
                    obstacle,
                });
            }
            "headslicebar" => {
                assert_eq!(fields[2], "system");
                assert_eq!(fields[4], "ordinal");
                let system_id = fields[3].parse().expect("system id");
                let ordinal: usize = fields[5].parse().expect("frozen ordinal");
                let obstacle = parse_obstacle(&fields, 6);
                let system = pages
                    .get_mut(fields[1])
                    .expect("bar follows page")
                    .systems
                    .entry(system_id)
                    .or_default();
                assert_eq!(ordinal, system.frozen.len());
                system.frozen.push(obstacle);
            }
            _ => {}
        }
    }
    pages
}

fn parse_obstacle(fields: &[&str], start: usize) -> ExpectedObstacle {
    assert_eq!(fields[start], "class");
    assert_eq!(fields[start + 2], "shape");
    assert_eq!(fields[start + 4], "staff");
    assert_eq!(fields[start + 6], "bounds");
    assert_eq!(fields[start + 8], "grade");
    assert_eq!(fields[start + 10], "best");
    assert_eq!(fields[start + 12], "good");
    assert_eq!(fields[start + 14], "frozen");
    assert_eq!(fields[start + 16], "removed");
    assert_eq!(fields[start + 18], "area");
    assert_eq!(fields[start + 20], "vertical");
    assert_eq!(fields[start + 22], "width");
    let bounds = parse_rectangle(fields[start + 7]);
    let area = parse_rectangle(fields[start + 19]);
    assert_eq!(area, bounds, "inter bounds and area bounds");
    let axis = fields[start + 21]
        .split(':')
        .map(double_bits)
        .collect::<Vec<_>>();
    let median_bits: [u64; 4] = axis.try_into().expect("four median coordinates");
    ExpectedObstacle {
        class: fields[start + 1].to_owned(),
        shape: fields[start + 3].to_owned(),
        staff_id: optional_usize(fields[start + 5]),
        frozen: fields[start + 15].parse().expect("frozen flag"),
        area_bounds: bounds,
        median_bits,
        thickness_bits: double_bits(fields[start + 23]),
    }
}

fn parse_rectangle(text: &str) -> JavaRectangle {
    let values = text
        .split(':')
        .map(|field| field.parse::<i32>().expect("rectangle field"))
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 4);
    JavaRectangle::new(values[0], values[1], values[2], values[3])
}

fn double_bits(text: &str) -> u64 {
    let (_, bits) = text.split_once('/').expect("hex double/raw bits");
    u64::from_str_radix(bits, 16).expect("raw double bits")
}

fn optional_usize(text: &str) -> Option<usize> {
    (text != "-").then(|| text.parse().expect("optional ordinal"))
}
