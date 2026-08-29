// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;

use audiveris_omr::{
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads,
    native_ledgers::recognize_native_ledgers,
    native_reduction::recognize_native_reduction,
    native_stem_seeds::recognize_native_stem_seeds,
    native_stems::recognize_native_stems,
    recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds},
};

const SOURCE_MEI: &[u8] = include_bytes!(
    "../../../oracle/clean-reduction-no-builders/piano-control-notation-coverage.mei"
);

#[test]
fn clean_system_without_beam_builders_reaches_terminal_reduction() {
    assert!(SOURCE_MEI.starts_with(b"<?xml"), "tracked MEI fixture");
    let grid = recognize_grid_lines(fixture_path("coverage-2x.png")).expect("coverage GRID");
    let headers = recognize_native_headers(&grid).expect("coverage HEADERS");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("coverage STEM_SEEDS");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("coverage BEAMS");
    let ledgers = recognize_native_ledgers(&grid, &beams).expect("coverage LEDGERS");
    let heads = recognize_native_heads(&grid, &headers, &stem_seeds, &beams, &ledgers)
        .expect("coverage HEADS");
    assert_eq!(heads.epilog.final_head_count, 58, "Java-exact raw HEADS");

    let stems = recognize_native_stems(&grid, &headers, &stem_seeds, &beams, &ledgers, &heads, 1)
        .expect("a builder-free system must still reach terminal STEMS");
    assert_eq!(stems.systems.len(), 2);
    assert!(
        stems.components.beam_builders.systems[0]
            .builders
            .is_empty(),
        "system 1 deliberately exercises the zero-builder SIDES/STUMPS path"
    );

    let reduction = recognize_native_reduction(&grid, stems)
        .expect("a builder-free system must still reach terminal REDUCTION");
    assert_eq!(reduction.foundations.len(), 2);
    assert_eq!(reduction.head_end_refinements.len(), 2);
    assert_eq!(reduction.beam_groups.len(), 2);
}

fn fixture_path(page: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../rust/oracle/clean-reduction-no-builders")
        .join(page)
}
