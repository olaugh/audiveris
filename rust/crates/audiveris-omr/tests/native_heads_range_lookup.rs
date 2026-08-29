// SPDX-License-Identifier: AGPL-3.0-or-later

//! Eight-page Java differential for the pre-aggregation HEADS range lookup.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use audiveris_omr::{
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads_prolog,
    native_heads_bar_slices::materialize_native_head_scanner_bar_slices,
    native_heads_competitor_slices::materialize_native_head_scanner_competitor_slices,
    native_heads_competitors::materialize_native_heads_competitors,
    native_heads_obstacles::materialize_native_heads_bar_obstacles,
    native_heads_range_lookup::{
        NativeHeadRangeKernelHashes, NativeHeadsRangeLookupInput, STEMLESS_BOOST,
        recognize_native_heads_range_lookup,
    },
    native_heads_scanner::recognize_native_heads_scanner_context,
    native_heads_scanner_pools::materialize_native_head_scanner_pools,
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::recognize_native_stem_seeds,
    recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds},
};

const ORACLE: &str = include_str!("../../../oracle/heads-range-pass.txt");

#[derive(Debug)]
struct ExpectedStaff {
    scanners: usize,
    counts: [usize; 5],
    scan_kinds: [usize; 6],
    performance: [usize; 4],
    outcomes: [usize; 7],
    hashes: NativeHeadRangeKernelHashes,
}

#[test]
fn native_heads_range_lookup_matches_java_corpus() {
    let expected = parse_staff_summaries();
    assert_eq!(expected.len(), 55);
    let pages = expected
        .keys()
        .map(|(page, _, _)| page.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(pages.len(), 8);

    let mut matched = 0;
    let mut totals = [0_usize; 4];
    for page in pages {
        let image_name = page.strip_suffix("#1").unwrap();
        let image = repo_path(&format!("data/examples/{image_name}"));
        let grid = recognize_grid_lines(image)
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
        let competitors = materialize_native_heads_competitors(&grid, &beams)
            .unwrap_or_else(|error| panic!("{page}: competitors failed: {error}"));
        let ledgers = recognize_native_ledgers(&grid, &beams)
            .unwrap_or_else(|error| panic!("{page}: LEDGERS failed: {error}"));
        let heads = recognize_native_heads_prolog(&grid, &beams, &ledgers, &stem_seeds)
            .unwrap_or_else(|error| panic!("{page}: HEADS prolog failed: {error}"));
        let scanners =
            recognize_native_heads_scanner_context(&grid, &headers, &ledgers, &stem_seeds, &heads)
                .unwrap_or_else(|error| panic!("{page}: scanner context failed: {error}"));
        let pools = materialize_native_head_scanner_pools(&stem_seeds, &heads, &scanners)
            .unwrap_or_else(|error| panic!("{page}: scanner pools failed: {error}"));
        let bar_slices = materialize_native_head_scanner_bar_slices(&pools, &obstacles)
            .unwrap_or_else(|error| panic!("{page}: bar slices failed: {error}"));
        let competitor_slices =
            materialize_native_head_scanner_competitor_slices(&pools, &competitors)
                .unwrap_or_else(|error| panic!("{page}: competitor slices failed: {error}"));
        let actual = recognize_native_heads_range_lookup(NativeHeadsRangeLookupInput {
            heads: &heads,
            scanners: &scanners,
            pools: &pools,
            obstacles: &obstacles,
            bar_slices: &bar_slices,
            competitors: &competitors,
            competitor_slices: &competitor_slices,
            stemless_boost: STEMLESS_BOOST,
        })
        .unwrap_or_else(|error| panic!("{page}: range lookup failed: {error}"));

        for system in &actual.systems {
            for staff in &system.staffs {
                let label = format!(
                    "{page} system {} staff {}",
                    system.system_id, staff.staff_id
                );
                let expected = expected
                    .get(&(page.clone(), system.system_id, staff.staff_id))
                    .unwrap_or_else(|| panic!("missing oracle {label}"));
                assert_eq!(staff.scans.len(), expected.scanners, "{label} scanners");
                assert_eq!(
                    [
                        staff.counts.spot_slices,
                        staff.counts.x_scans,
                        staff.counts.safety_skips,
                        staff.counts.shape_attempts,
                        staff.counts.raw_candidates,
                    ],
                    expected.counts,
                    "{label} counts"
                );
                assert_eq!(
                    [
                        staff.counts.safe_all,
                        staff.counts.safe_hollow,
                        staff.counts.right_limit,
                        staff.counts.above_image,
                        staff.counts.below_image,
                        staff.counts.foreground_gap,
                    ],
                    expected.scan_kinds,
                    "{label} scan kinds"
                );
                assert_eq!(
                    [
                        staff.performance.bar_blocks,
                        staff.performance.competitor_blocks,
                        staff.performance.evaluations,
                        staff.performance.nominal_abandons,
                    ],
                    expected.performance,
                    "{label} performance"
                );
                assert_eq!(
                    [
                        staff.counts.abandoned_bar,
                        staff.counts.abandoned_competitor,
                        staff.counts.abandoned_nominal,
                        staff.counts.no_best,
                        staff.counts.weak_stemless,
                        staff.counts.below_minimum_grade,
                        staff.counts.provisional,
                    ],
                    expected.outcomes,
                    "{label} outcomes"
                );
                assert_eq!(staff.hashes, expected.hashes, "{label} hashes");
                matched += 1;
            }
        }

        totals[0] += actual.counts.spot_slices;
        totals[1] += actual.counts.x_scans;
        totals[2] += actual.counts.shape_attempts;
        totals[3] += actual.counts.raw_candidates;
    }

    assert_eq!(matched, expected.len());
    assert_eq!(totals, [6_759, 921_558, 3_119_882, 34_101]);
}

fn parse_staff_summaries() -> BTreeMap<(String, usize, usize), ExpectedStaff> {
    let mut expected = BTreeMap::new();
    for line in ORACLE.lines() {
        if !line.starts_with("headrangestaffsummary ") {
            continue;
        }
        let fields = line.split_whitespace().collect::<Vec<_>>();
        let page = fields[1].to_owned();
        let system_id = field(&fields, "system").parse().unwrap();
        let staff_id = field(&fields, "staff").parse().unwrap();
        let item = ExpectedStaff {
            scanners: field(&fields, "scanners").parse().unwrap(),
            counts: [
                field(&fields, "spotSlices").parse().unwrap(),
                field(&fields, "scans").parse().unwrap(),
                field(&fields, "safetySkips").parse().unwrap(),
                field(&fields, "attempts").parse().unwrap(),
                field(&fields, "rawCandidates").parse().unwrap(),
            ],
            scan_kinds: colon_counts(field(&fields, "scanKinds")),
            performance: colon_counts(field(&fields, "perf")),
            outcomes: colon_counts(field(&fields, "outcomes")),
            hashes: NativeHeadRangeKernelHashes {
                spot: parse_hex(field(&fields, "spotKernelHash")),
                scan: parse_hex(field(&fields, "scanKernelHash")),
                attempt: parse_hex(field(&fields, "attemptKernelHash")),
                raw_candidate: parse_hex(field(&fields, "rawKernelHash")),
            },
        };
        assert!(expected.insert((page, system_id, staff_id), item).is_none());
    }
    expected
}

fn field<'a>(fields: &[&'a str], name: &str) -> &'a str {
    let index = fields
        .iter()
        .position(|field| *field == name)
        .unwrap_or_else(|| panic!("missing {name}"));
    fields[index + 1]
}

fn colon_counts<const N: usize>(text: &str) -> [usize; N] {
    text.split(':')
        .map(|item| item.parse().unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap_or_else(|_| panic!("expected {N} counts in {text}"))
}

fn parse_hex(text: &str) -> u64 {
    u64::from_str_radix(text, 16).unwrap()
}

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}
