// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production scanner-band -> frozen GRID bar-slice boundary.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use audiveris_omr::{
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads_prolog,
    native_heads_bar_slices::materialize_native_head_scanner_bar_slices,
    native_heads_obstacles::materialize_native_heads_bar_obstacles,
    native_heads_scanner::recognize_native_heads_scanner_context,
    native_heads_scanner_pools::materialize_native_head_scanner_pools,
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::recognize_native_stem_seeds,
    recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds},
};

const ORACLE: &str = include_str!("../../../oracle/heads-scanner-slices.txt");

#[derive(Debug, Default)]
struct ExpectedPage {
    systems: BTreeMap<usize, ExpectedSystem>,
}

#[derive(Debug, Default)]
struct ExpectedSystem {
    staffs: BTreeMap<usize, Vec<ExpectedScan>>,
}

#[derive(Debug)]
struct ExpectedScan {
    geometry_ordinal: usize,
    frozen_bar_indices: Vec<usize>,
}

#[test]
fn native_bar_slices_match_java_on_every_corpus_scanner() {
    let expected = parse_oracle();
    assert_eq!(expected.len(), 8);
    let mut system_count = 0;
    let mut staff_count = 0;
    let mut scan_count = 0;
    let mut nonempty_slice_count = 0;
    let mut reference_count = 0;

    for (page, expected_page) in expected {
        let file = page
            .strip_suffix("#1")
            .unwrap_or_else(|| panic!("unexpected sheet key {page}"));
        let grid = recognize_grid_lines(repo_path(&format!("data/examples/{file}")))
            .unwrap_or_else(|error| panic!("{page}: GRID failed: {error}"));
        let obstacles = materialize_native_heads_bar_obstacles(&grid)
            .unwrap_or_else(|error| panic!("{page}: obstacles failed: {error}"));
        let headers = recognize_native_headers(&grid)
            .unwrap_or_else(|error| panic!("{page}: HEADERS failed: {error}"));
        let stem_seeds = recognize_native_stem_seeds(&grid, &headers)
            .unwrap_or_else(|error| panic!("{page}: STEM_SEEDS failed: {error}"));
        let beams =
            recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
                .unwrap_or_else(|error| panic!("{page}: BEAMS failed: {error}"));
        let ledgers = recognize_native_ledgers(&grid, &beams)
            .unwrap_or_else(|error| panic!("{page}: LEDGERS failed: {error}"));
        let heads = recognize_native_heads_prolog(&grid, &beams, &ledgers, &stem_seeds)
            .unwrap_or_else(|error| panic!("{page}: HEADS prolog failed: {error}"));
        let scanners =
            recognize_native_heads_scanner_context(&grid, &headers, &ledgers, &stem_seeds, &heads)
                .unwrap_or_else(|error| panic!("{page}: scanner context failed: {error}"));
        let pools = materialize_native_head_scanner_pools(&stem_seeds, &heads, &scanners)
            .unwrap_or_else(|error| panic!("{page}: scanner pools failed: {error}"));
        let actual = materialize_native_head_scanner_bar_slices(&pools, &obstacles)
            .unwrap_or_else(|error| panic!("{page}: bar slices failed: {error}"));

        assert_eq!(
            actual.systems.len(),
            expected_page.systems.len(),
            "{page} system count"
        );
        for actual_system in &actual.systems {
            let system_label = format!("{page} system {}", actual_system.system_id);
            let expected_system = expected_page
                .systems
                .get(&actual_system.system_id)
                .unwrap_or_else(|| panic!("{system_label} missing from oracle"));
            assert_eq!(
                actual_system.staffs.len(),
                expected_system.staffs.len(),
                "{system_label} staff count"
            );
            for actual_staff in &actual_system.staffs {
                let staff_label = format!("{system_label} staff {}", actual_staff.staff_id);
                let expected_scans = expected_system
                    .staffs
                    .get(&actual_staff.staff_id)
                    .unwrap_or_else(|| panic!("{staff_label} missing from oracle"));
                assert_eq!(
                    actual_staff.scans.len(),
                    expected_scans.len(),
                    "{staff_label} scan count"
                );
                for (actual_scan, expected_scan) in actual_staff.scans.iter().zip(expected_scans) {
                    let scan_label = format!("{staff_label} scan {}", actual_scan.geometry_ordinal);
                    assert_eq!(
                        actual_scan.geometry_ordinal, expected_scan.geometry_ordinal,
                        "{scan_label} ordinal"
                    );
                    assert_eq!(
                        actual_scan.frozen_bar_indices, expected_scan.frozen_bar_indices,
                        "{scan_label} frozen bar slice"
                    );
                    scan_count += 1;
                    nonempty_slice_count += usize::from(!actual_scan.frozen_bar_indices.is_empty());
                    reference_count += actual_scan.frozen_bar_indices.len();
                }
                staff_count += 1;
            }
            system_count += 1;
        }
    }

    assert_eq!(system_count, 30);
    assert_eq!(staff_count, 55);
    assert_eq!(scan_count, 1_767);
    assert_eq!(nonempty_slice_count, 552);
    assert_eq!(reference_count, 5_060);
}

fn parse_oracle() -> BTreeMap<String, ExpectedPage> {
    let mut pages = BTreeMap::<String, ExpectedPage>::new();
    for (line_number, line) in ORACLE.lines().enumerate() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.first() != Some(&"headslicescan") {
            continue;
        }
        assert_eq!(fields[2], "system", "line {}", line_number + 1);
        assert_eq!(fields[4], "staff", "line {}", line_number + 1);
        assert_eq!(fields[6], "ordinal", "line {}", line_number + 1);
        let system_id = fields[3].parse().expect("system id");
        let staff_id = fields[5].parse().expect("staff id");
        let geometry_ordinal = fields[7].parse().expect("geometry ordinal");
        let bars = fields
            .iter()
            .position(|field| *field == "bars")
            .unwrap_or_else(|| panic!("line {} has no bars field", line_number + 1));
        assert_eq!(bars + 2, fields.len(), "line {} tail", line_number + 1);
        let frozen_bar_indices = parse_indices(fields[bars + 1]);
        let scans = pages
            .entry(fields[1].to_owned())
            .or_default()
            .systems
            .entry(system_id)
            .or_default()
            .staffs
            .entry(staff_id)
            .or_default();
        assert_eq!(
            geometry_ordinal,
            scans.len(),
            "line {} order",
            line_number + 1
        );
        scans.push(ExpectedScan {
            geometry_ordinal,
            frozen_bar_indices,
        });
    }
    pages
}

fn parse_indices(text: &str) -> Vec<usize> {
    if text == "-" {
        Vec::new()
    } else {
        text.split(',')
            .map(|value| value.parse().expect("bar index"))
            .collect()
    }
}

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}
