// SPDX-License-Identifier: AGPL-3.0-or-later

//! Focused production smoke test for the single native HEADS entry point.

use std::path::PathBuf;

use audiveris_omr::{
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads,
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::recognize_native_stem_seeds,
    recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds},
};

#[test]
fn full_entry_owns_every_chula_heads_boundary() {
    let grid = recognize_grid_lines(repo_path("data/examples/chula.png")).expect("GRID");
    let headers = recognize_native_headers(&grid).expect("HEADERS");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS");
    let ledgers = recognize_native_ledgers(&grid, &beams).expect("LEDGERS");

    let heads = recognize_native_heads(&grid, &headers, &stem_seeds, &beams, &ledgers)
        .expect("complete HEADS");

    let expected_systems = vec![1, 2, 3];
    assert_eq!(
        heads
            .obstacles
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect::<Vec<_>>(),
        expected_systems
    );
    assert_eq!(
        heads
            .scanners
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect::<Vec<_>>(),
        expected_systems
    );
    assert_eq!(
        heads
            .scanner_pools
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect::<Vec<_>>(),
        expected_systems
    );
    assert_eq!(
        heads
            .competitors
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect::<Vec<_>>(),
        expected_systems
    );
    assert_eq!(
        heads.seed_glyphs.head_count + heads.range_glyphs.head_count,
        332
    );
    assert_eq!(heads.epilog.staff_epilog.retained_head_count, 327);
    assert_eq!(heads.epilog.head_beam_removal_count, 1);
    assert_eq!(heads.epilog.final_head_count, 326);
    assert_eq!(
        heads
            .epilog
            .systems
            .iter()
            .map(|system| system.final_heads.len())
            .sum::<usize>(),
        heads.epilog.final_head_count
    );
}

fn repo_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(path)
}
