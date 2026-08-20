// SPDX-License-Identifier: AGPL-3.0-or-later

//! Regression fixtures for the credible-beam veto gate, on the two Schenker
//! Sonata 1 page-1 misses it was built to recover.
//!
//! The pipeline options are passed as data, never via this process's
//! environment — but GRID itself needs the hard-scan env profile to find all
//! six systems on this page, so the test is `#[ignore]`d and must run in its
//! own invocation:
//!
//! ```text
//! AUDIVERIS_RECOVER_PIANO_SYSTEM_PAIRS=1 AUDIVERIS_RECOVER_PIANO_SYSTEM_BOUNDS=1 \
//! AUDIVERIS_RECOVER_STRONG_WIDE_PARTIAL_COLUMNS=1 AUDIVERIS_ADAPTIVE_BAR_VERTICAL_SLOPE=1 \
//! AUDIVERIS_SLOPE_AWARE_BAR_PROJECTION=1 AUDIVERIS_STAFF_EDGE_BAR_PROJECTION=1 \
//! AUDIVERIS_WEAK_BAR_MIN_GRADE=0.71 AUDIVERIS_MINIMUM_STAFF_PEAK_GRADE=0.0 \
//! AUDIVERIS_MINIMUM_CLUSTER_LENGTH_RATIO=0.15 AUDIVERIS_MINIMUM_STAFF_WIDTH_INTERLINES=27 \
//! AUDIVERIS_SCHENKER_PAGES=<dir with page-01.png> \
//! cargo test -p audiveris-omr --release --test credible_beam_vetoes -- --ignored --nocapture
//! ```

use audiveris_omr::beam_veto::BeamVetoScale;
use audiveris_omr::native_headers::recognize_native_headers;
use audiveris_omr::native_ledgers::recognize_native_ledgers_with_options;
use audiveris_omr::native_stem_seeds::recognize_native_stem_seeds;
use audiveris_omr::recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds};

#[test]
#[ignore = "manual regression; needs AUDIVERIS_SCHENKER_PAGES and the GRID hard-scan env profile"]
fn credible_beam_vetoes_recover_schenker_page1_misses() {
    let pages = std::env::var("AUDIVERIS_SCHENKER_PAGES").expect("AUDIVERIS_SCHENKER_PAGES");
    let page = std::path::Path::new(&pages).join("page-01.png");

    let grid = recognize_grid_lines(&page).expect("GRID");
    let headers = recognize_native_headers(&grid).expect("HEADERS");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS");
    let veto_scale = BeamVetoScale {
        interline: f64::from(grid.scale.scale.interline.main),
        beam_thickness: f64::from(grid.scale.scale.beam.main),
    };

    // The m30 high Fb sits on ledger rung -3 of staff 11 (system 6) around
    // x 683-696. A hallucinated 10x3 beam+hook on the -2 dash starves the
    // chain under Java rules; the credible gate must restore rungs -2 and -3.
    let ledgers_at = |option| {
        let ledgers = recognize_native_ledgers_with_options(&grid, &beams, option)
            .expect("LEDGERS recognition");
        let mut indexes = ledgers
            .ledgers()
            .iter()
            .filter(|inter| {
                inter.staff_id == 11
                    && inter.bounds.x >= 680.0
                    && inter.bounds.x <= 700.0
                    && inter.ledger_index < 0
            })
            .map(|inter| inter.ledger_index)
            .collect::<Vec<_>>();
        indexes.sort_unstable();
        indexes
    };
    assert_eq!(
        ledgers_at(None),
        vec![-1],
        "Java baseline: the false beam must still starve the m30 chain"
    );
    assert_eq!(
        ledgers_at(Some(veto_scale)),
        vec![-3, -2, -1],
        "credible gate: the full m30 ledger chain must materialize"
    );
}

/// Page 9's second-system 32nd runs: heads two ledgers above the staff whose
/// -2 ledger fused with the run and was never accepted. With
/// `AUDIVERIS_EXTENDED_LEDGER_PITCHES` (plus the credible-beam profile and
/// the GRID hard-scan env, set by the invocation), synthesized scan lines
/// from the neighboring accepted ledgers must propose them, and the range
/// scanner's rejected stemless matches must be retained.
#[test]
#[ignore = "manual regression; needs AUDIVERIS_SCHENKER_PAGES and the flag-on env profile"]
fn extended_ledger_pitches_recover_page9_run_peaks() {
    assert!(
        audiveris_omr::native_heads_scanner::extended_ledger_pitches_enabled(),
        "run with AUDIVERIS_EXTENDED_LEDGER_PITCHES=1"
    );
    let pages = std::env::var("AUDIVERIS_SCHENKER_PAGES").expect("AUDIVERIS_SCHENKER_PAGES");
    let page = std::path::Path::new(&pages).join("page-09.png");

    let grid = recognize_grid_lines(&page).expect("GRID");
    let headers = recognize_native_headers(&grid).expect("HEADERS");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS");
    let ledgers = audiveris_omr::native_ledgers::recognize_native_ledgers(&grid, &beams)
        .expect("LEDGERS");
    let heads =
        audiveris_omr::native_heads::recognize_native_heads(&grid, &headers, &stem_seeds, &beams, &ledgers)
            .expect("HEADS");

    let epilog = &heads.epilog;
    let mut recovered = 0;
    for system in &epilog.systems {
        if system.system_id != 2 {
            continue;
        }
        let staff_system = epilog
            .staff_epilog
            .systems
            .iter()
            .find(|candidate| candidate.system_id == 2)
            .expect("staff epilog system 2");
        for reference in &system.final_heads {
            let head = &staff_system.staffs[reference.staff_index].heads[reference.head_index];
            let bounds = head.bounds;
            if head.pitch() <= -8.0
                && (435..=455).contains(&bounds.x)
                && (218..=232).contains(&bounds.y)
                && head.grade() >= 0.5
            {
                recovered += 1;
            }
        }
    }
    assert!(
        recovered >= 2,
        "expected the m50 run peaks proposed and strongly matched, found {recovered}"
    );
    assert!(
        !epilog.subfloor_heads.is_empty(),
        "sub-floor retention must record the range scanner's rejected matches"
    );
}

/// Range-scan lattice probe: for one staff, print every geometry's scan
/// range, its spots' effective x-intervals, and its candidates inside an
/// x-window, so "no attempt" is distinguishable from "attempted and lost".
///
/// ```text
/// AUDIVERIS_SCHENKER_PAGES=... AUDIVERIS_PROBE_SPEC=page-09.png,3,325,372 \
///   cargo test -p audiveris-omr --release --test credible_beam_vetoes \
///   probe_range_scan_lattice -- --ignored --nocapture
/// ```
#[test]
#[ignore = "manual probe; needs AUDIVERIS_SCHENKER_PAGES and AUDIVERIS_PROBE_SPEC"]
fn probe_range_scan_lattice() {
    let pages = std::env::var("AUDIVERIS_SCHENKER_PAGES").expect("AUDIVERIS_SCHENKER_PAGES");
    let Ok(spec) = std::env::var("AUDIVERIS_PROBE_SPEC") else {
        eprintln!("skipping: AUDIVERIS_PROBE_SPEC unset (manual probe)");
        return;
    };
    let parts: Vec<&str> = spec.split(',').collect();
    let (page_name, staff_id, x_min, x_max) = (
        parts[0],
        parts[1].parse::<usize>().unwrap(),
        parts[2].parse::<i32>().unwrap(),
        parts[3].parse::<i32>().unwrap(),
    );
    let page = std::path::Path::new(&pages).join(page_name);

    let grid = recognize_grid_lines(&page).expect("GRID");
    let headers = recognize_native_headers(&grid).expect("HEADERS");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS");
    let ledgers = audiveris_omr::native_ledgers::recognize_native_ledgers(&grid, &beams)
        .expect("LEDGERS");
    let heads = audiveris_omr::native_heads::recognize_native_heads(
        &grid, &headers, &stem_seeds, &beams, &ledgers,
    )
    .expect("HEADS");

    for system in &heads.seed_lookup.systems {
        for staff in &system.staffs {
            if staff.staff_id != staff_id {
                continue;
            }
            for scan in &staff.scans {
                for search in &scan.searches {
                    let bounds = search.seed_bounds;
                    if bounds.x + bounds.width < x_min || bounds.x > x_max {
                        continue;
                    }
                    let mut evaluated = 0;
                    let mut blocked = 0;
                    let mut best_distance = f64::INFINITY;
                    for attempt in &search.attempts {
                        match attempt.outcome {
                            audiveris_omr::native_heads_seed_lookup::NativeHeadSeedAttemptOutcome::Evaluated { distance } => {
                                evaluated += 1;
                                if distance < best_distance { best_distance = distance; }
                            }
                            _ => blocked += 1,
                        }
                    }
                    println!(
                        "SEED pitch {:3} seed@({},{},{}x{}) side {:?} shape {:?}->{:?} grade {:?} outcome {:?} attempts eval {} blocked {} bestDist {:.2}",
                        scan.pitch,
                        bounds.x,
                        bounds.y,
                        bounds.width,
                        bounds.height,
                        search.side,
                        search.original_shape,
                        search.final_shape,
                        search.grade.map(|grade| (grade * 100.0).round() / 100.0),
                        search.outcome,
                        evaluated,
                        blocked,
                        best_distance,
                    );
                }
            }
        }
    }
    for system in &heads.range_lookup.systems {
        for staff in &system.staffs {
            if staff.staff_id != staff_id {
                continue;
            }
            for scan in &staff.scans {
                if scan.scan_right < x_min || scan.scan_left > x_max {
                    continue;
                }
                let spots: Vec<String> = scan
                    .spots
                    .iter()
                    .filter_map(|spot| spot.effective_x)
                    .filter(|(left, right)| *right >= x_min && *left <= x_max)
                    .map(|(left, right)| format!("{left}..{right}"))
                    .collect();
                let candidates: Vec<String> = scan
                    .candidates
                    .iter()
                    .filter(|candidate| candidate.x0 >= x_min && candidate.x0 <= x_max)
                    .map(|candidate| {
                        format!(
                            "{:?}@x{} g{:.2}",
                            candidate.shape, candidate.x0, candidate.grade
                        )
                    })
                    .collect();
                println!(
                    "pitch {:3} range {}..{} source {:?} spots [{}] candidates [{}]",
                    scan.pitch,
                    scan.scan_left,
                    scan.scan_right,
                    match &scan.source {
                        audiveris_omr::native_heads_scanner::NativeHeadScannerSource::StaffLine { .. } => "staff".to_owned(),
                        audiveris_omr::native_heads_scanner::NativeHeadScannerSource::Ledger { ledger_index, bounds, .. } =>
                            format!("ledger{} x{}w{}", ledger_index, bounds.x, bounds.width),
                    },
                    spots.join(" "),
                    candidates.join(" "),
                );
            }
        }
    }
}

/// Page 7 staff 3: dense 32nd-run ink broke staff-line stitching at x=468
/// while its system siblings reach 719, blinding head lookup to the whole
/// right half (the m25 run of 18 notes). With
/// `AUDIVERIS_EXTEND_SHORT_STAFF_LINES` (set by the invocation env) the lines
/// extend to the system edge and the run must resolve.
#[test]
#[ignore = "manual regression; needs AUDIVERIS_SCHENKER_PAGES and the flag-on env profile"]
fn extended_staff_lines_recover_page7_run() {
    let pages = std::env::var("AUDIVERIS_SCHENKER_PAGES").expect("AUDIVERIS_SCHENKER_PAGES");
    let page = std::path::Path::new(&pages).join("page-07.png");

    let grid = recognize_grid_lines(&page).expect("GRID");
    let staff3 = grid
        .peak_graph
        .sheet_staffs
        .iter()
        .find(|staff| staff.id == 3)
        .expect("staff 3");
    assert!(
        staff3.right > 700.0,
        "staff 3 lines must extend to the system edge, got {}",
        staff3.right
    );

    let headers = recognize_native_headers(&grid).expect("HEADERS");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS");
    let ledgers = audiveris_omr::native_ledgers::recognize_native_ledgers(&grid, &beams)
        .expect("LEDGERS");
    let heads = audiveris_omr::native_heads::recognize_native_heads(
        &grid, &headers, &stem_seeds, &beams, &ledgers,
    )
    .expect("HEADS");

    let epilog = &heads.epilog;
    let mut strong = 0;
    for system in &epilog.systems {
        if system.system_id != 2 {
            continue;
        }
        let staff_system = epilog
            .staff_epilog
            .systems
            .iter()
            .find(|candidate| candidate.system_id == 2)
            .expect("staff epilog system 2");
        for reference in &system.final_heads {
            let head = &staff_system.staffs[reference.staff_index].heads[reference.head_index];
            let bounds = head.bounds;
            if (465..=712).contains(&bounds.x)
                && (245..=285).contains(&bounds.y)
                && head.grade() >= 0.6
            {
                strong += 1;
            }
        }
    }
    assert!(
        strong >= 15,
        "the m25 32nd run must resolve strongly across the recovered half, found {strong}"
    );
}

/// Page 9's lone A at pitch -6 (x~340, y~231): a 52x3 sub-scale beam laid
/// over the fused ledger row entered the HEADS competitor pool, and every
/// one of the seed search's six template offsets was competitor-blocked, so
/// no location was ever evaluated. With the credible-beam gate extended to
/// the competitor pool, the A must resolve strongly.
#[test]
#[ignore = "manual regression; needs AUDIVERIS_SCHENKER_PAGES and the flag-on env profile"]
fn credible_competitors_recover_page9_lone_a() {
    assert!(
        audiveris_omr::beam_veto::credible_beam_vetoes_enabled(),
        "run with AUDIVERIS_CREDIBLE_BEAM_VETOES=1"
    );
    let pages = std::env::var("AUDIVERIS_SCHENKER_PAGES").expect("AUDIVERIS_SCHENKER_PAGES");
    let page = std::path::Path::new(&pages).join("page-09.png");

    let grid = recognize_grid_lines(&page).expect("GRID");
    let headers = recognize_native_headers(&grid).expect("HEADERS");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS");
    let ledgers = audiveris_omr::native_ledgers::recognize_native_ledgers(&grid, &beams)
        .expect("LEDGERS");
    let heads = audiveris_omr::native_heads::recognize_native_heads(
        &grid, &headers, &stem_seeds, &beams, &ledgers,
    )
    .expect("HEADS");

    let epilog = &heads.epilog;
    let mut best = 0.0_f64;
    for system in &epilog.systems {
        if system.system_id != 2 {
            continue;
        }
        let staff_system = epilog
            .staff_epilog
            .systems
            .iter()
            .find(|candidate| candidate.system_id == 2)
            .expect("staff epilog system 2");
        for reference in &system.final_heads {
            let head = &staff_system.staffs[reference.staff_index].heads[reference.head_index];
            let bounds = head.bounds;
            if (336..=346).contains(&bounds.x) && (228..=236).contains(&bounds.y) {
                best = best.max(head.grade());
            }
        }
    }
    assert!(
        best >= 0.6,
        "the lone A must resolve once sub-scale beams cannot block its template, best grade {best}"
    );
}

/// Schenker page 17: the triplet-eighth beams the closing fuses with their
/// noteheads must survive item grading under `AUDIVERIS_FUSED_BEAM_HEADROOM`.
///
/// Two symptoms of one disease. The fused ink drags a border fit outward, so
/// the item measures 5.0-5.7px against a typical height of 4 and dies on
/// Java's 1.4x ceiling; lifting the ceiling alone then exposed the second
/// symptom -- the same endpoints zero the jitter impact (137 of 138
/// below-floor items on this page died at `dist 0.00`). The gate lifts the
/// ceiling to 2.0x and anchors the jitter trim to the analysed border. Each
/// asserted region is one triplet beam the user pointed at as missing; the
/// probe asserts a raw beam at least two interlines wide in each.
///
/// AUDIVERIS_FUSED_BEAM_HEADROOM=1 + the GRID hard-scan profile required.
#[test]
#[ignore = "manual regression; needs AUDIVERIS_SCHENKER_PAGES and the flag-on env profile"]
fn fused_beam_headroom_recovers_page17_triplet_beams() {
    assert!(
        audiveris_omr::beam_veto::fused_beam_headroom_enabled(),
        "run with AUDIVERIS_FUSED_BEAM_HEADROOM=1"
    );
    let pages = std::env::var("AUDIVERIS_SCHENKER_PAGES").expect("AUDIVERIS_SCHENKER_PAGES");
    let page = std::path::Path::new(&pages).join("page-17.png");

    let grid = recognize_grid_lines(&page).expect("GRID");
    let headers = recognize_native_headers(&grid).expect("HEADERS");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS");

    // (x-range, y-range) of the triplet beams previously graded to death.
    let targets: [(std::ops::RangeInclusive<f64>, std::ops::RangeInclusive<f64>); 5] = [
        (105.0..=135.0, 50.0..=58.0),
        (260.0..=290.0, 529.0..=537.0),
        (410.0..=442.0, 527.0..=535.0),
        (490.0..=520.0, 526.0..=534.0),
        (525.0..=557.0, 525.0..=534.0),
    ];
    for (x_range, y_range) in targets {
        let found = beams.raw_beams.iter().any(|(_, beam)| {
            let median = beam.item.median;
            let wide_enough = median.x2 - median.x1 >= 18.0;
            x_range.contains(&median.x1) && y_range.contains(&median.y1) && wide_enough
        });
        assert!(
            found,
            "expected a recovered triplet beam with x1 in {x_range:?}, y1 in {y_range:?}"
        );
    }
}

/// Schenker page 1: stuck two-beam stacks must produce both beams under
/// `AUDIVERIS_STACKED_BEAM_SPLIT`.
///
/// Java's `splitLines` replaces a fused line with fresh `BeamLine`s whose
/// item lists are empty and nothing repopulates them, so `createBeamInters`
/// iterates nothing: the split is dead code, and every sixteenth-note beam
/// pair whose gutter the closing bridges silently creates zero inters
/// ("no item to grade", lines 2, items 0). The gate re-runs item retrieval
/// on the emptied lines. Each asserted region is one such stack; the probe
/// asserts at least two raw beams whose median y differ by at least half a
/// typical height.
///
/// AUDIVERIS_STACKED_BEAM_SPLIT=1 + the GRID hard-scan profile required.
#[test]
#[ignore = "manual regression; needs AUDIVERIS_SCHENKER_PAGES and the flag-on env profile"]
fn stacked_beam_split_recovers_page1_sixteenth_stacks() {
    assert!(
        audiveris_omr::beam_veto::stacked_beam_split_enabled(),
        "run with AUDIVERIS_STACKED_BEAM_SPLIT=1"
    );
    let pages = std::env::var("AUDIVERIS_SCHENKER_PAGES").expect("AUDIVERIS_SCHENKER_PAGES");
    let page = std::path::Path::new(&pages).join("page-01.png");

    let grid = recognize_grid_lines(&page).expect("GRID");
    let headers = recognize_native_headers(&grid).expect("HEADERS");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS");

    // (x-range, y-range) of fused sixteenth stacks previously dropped whole.
    let targets: [(std::ops::RangeInclusive<f64>, std::ops::RangeInclusive<f64>); 4] = [
        (335.0..=345.0, 188.0..=204.0),
        (615.0..=625.0, 184.0..=200.0),
        (503.0..=511.0, 434.0..=449.0),
        (138.0..=146.0, 512.0..=528.0),
    ];
    for (x_range, y_range) in targets {
        let mut ys: Vec<f64> = beams
            .raw_beams
            .iter()
            .filter(|(_, beam)| {
                let median = beam.item.median;
                x_range.contains(&median.x1)
                    && y_range.contains(&median.y1)
                    && median.x2 - median.x1 >= 12.0
            })
            .map(|(_, beam)| beam.item.median.y1)
            .collect();
        ys.sort_by(f64::total_cmp);
        assert!(
            ys.len() >= 2 && ys.last().unwrap() - ys.first().unwrap() >= 2.0,
            "expected a split pair in x {x_range:?} y {y_range:?}, got {ys:?}"
        );
    }
}

/// Schenker page 7: partially fused triple-beam stacks must build a structure
/// under `AUDIVERIS_CREATE_BEAM_BORDERS`.
///
/// A partially fused stack pairs its borders unevenly -- three top borders
/// where the gutters stay open, two bottoms where the closing bridged one --
/// and Java's `computeLines` refuses the whole structure ("This should never
/// happen!"). Java carries the repair, disabled: `allowBorderCreation`
/// synthesizes the missing opposite border one typical height away. The gate
/// enables that branch only for structures whose pairing failed, so evenly
/// paired structures never take the new path. Each asserted region is one
/// thirty-second-run stack from the user's page-7 screenshot; the probe
/// asserts at least three raw beams at distinct heights inside it.
///
/// AUDIVERIS_CREATE_BEAM_BORDERS=1 (+ the sibling beam flags) required.
#[test]
#[ignore = "manual regression; needs AUDIVERIS_SCHENKER_PAGES and the flag-on env profile"]
fn created_borders_recover_page7_triple_stacks() {
    assert!(
        audiveris_omr::beam_veto::create_beam_borders_enabled(),
        "run with AUDIVERIS_CREATE_BEAM_BORDERS=1"
    );
    let pages = std::env::var("AUDIVERIS_SCHENKER_PAGES").expect("AUDIVERIS_SCHENKER_PAGES");
    let page = std::path::Path::new(&pages).join("page-07.png");

    let grid = recognize_grid_lines(&page).expect("GRID");
    let headers = recognize_native_headers(&grid).expect("HEADERS");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS");

    let targets: [(std::ops::RangeInclusive<f64>, std::ops::RangeInclusive<f64>); 2] = [
        (120.0..=230.0, 650.0..=695.0),
        (115.0..=220.0, 805.0..=845.0),
    ];
    for (x_range, y_range) in targets {
        let mut ys: Vec<f64> = beams
            .raw_beams
            .iter()
            .filter(|(_, beam)| {
                let median = beam.item.median;
                x_range.contains(&median.x1)
                    && y_range.contains(&median.y1)
                    && median.x2 - median.x1 >= 8.0
            })
            .map(|(_, beam)| beam.item.median.y1)
            .collect();
        ys.sort_by(f64::total_cmp);
        ys.dedup_by(|a, b| (*a - *b).abs() < 2.0);
        assert!(
            ys.len() >= 3,
            "expected a recovered triple stack in x {x_range:?} y {y_range:?}, got {ys:?}"
        );
    }
}

/// Schenker page 7: a fused pair inside a multi-line structure must split
/// into two typical-height beams, not survive as one fat beam.
///
/// Java's `splitLines` handles only the fully fused single-line case (its
/// own TODO asks "what if beamCount = 2 and targetCount = 3 or more?"), and
/// the whole-glyph ink ratio cannot see a fused pair inside a wider blob --
/// the gaps dilute weight/width below the split threshold. Under
/// `AUDIVERIS_STACKED_BEAM_SPLIT` every line is additionally split by its own
/// border-fit height, with negative gutters allowed because a fully swallowed
/// gutter leaves the pair thinner than two typical heights. The asserted
/// regions are thirty-second-run fragments from the user's screenshots that
/// were accepted as single 6.3-7.7px beams; each must yield two beams of
/// typical thickness instead.
#[test]
#[ignore = "manual regression; needs AUDIVERIS_SCHENKER_PAGES and the flag-on env profile"]
fn per_line_split_replaces_page7_fat_singles() {
    assert!(
        audiveris_omr::beam_veto::stacked_beam_split_enabled(),
        "run with AUDIVERIS_STACKED_BEAM_SPLIT=1"
    );
    let pages = std::env::var("AUDIVERIS_SCHENKER_PAGES").expect("AUDIVERIS_SCHENKER_PAGES");
    let page = std::path::Path::new(&pages).join("page-07.png");

    let grid = recognize_grid_lines(&page).expect("GRID");
    let headers = recognize_native_headers(&grid).expect("HEADERS");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS");

    // Previously single fat beams (thickness 6.7-7.7) in the top-right run.
    let targets: [(std::ops::RangeInclusive<f64>, std::ops::RangeInclusive<f64>); 2] = [
        (660.0..=666.0, 158.0..=170.0),
        (686.0..=692.0, 164.0..=175.0),
    ];
    for (x_range, y_range) in targets {
        let split: Vec<(f64, f64)> = beams
            .raw_beams
            .iter()
            .filter(|(_, beam)| {
                let median = beam.item.median;
                x_range.contains(&median.x1) && y_range.contains(&median.y1)
            })
            .map(|(_, beam)| (beam.item.median.y1, beam.item.height))
            .collect();
        let distinct = {
            let mut ys: Vec<f64> = split.iter().map(|(y, _)| *y).collect();
            ys.sort_by(f64::total_cmp);
            ys.dedup_by(|a, b| (*a - *b).abs() < 1.5);
            ys.len()
        };
        assert!(
            distinct >= 2 && split.iter().all(|(_, h)| *h < 5.0),
            "expected the fused pair in x {x_range:?} y {y_range:?} split into \
             typical-height beams, got {split:?}"
        );
    }
}
