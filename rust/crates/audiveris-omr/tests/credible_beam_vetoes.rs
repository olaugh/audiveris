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
