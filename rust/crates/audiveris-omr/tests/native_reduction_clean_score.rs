// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;

use audiveris_omr::{
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads,
    native_ledgers::recognize_native_ledgers,
    native_reduction::recognize_native_reduction,
    native_sig::NativeSigInterKind,
    native_stem_seeds::recognize_native_stem_seeds,
    native_stems::recognize_native_stems,
    recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds},
};

const JAVA_ORACLE: &str =
    include_str!("../../../oracle/clean-reduction-disconnected/java-reduction-heads.txt");
const SOURCE_MEI: &[u8] =
    include_bytes!("../../../oracle/clean-reduction-disconnected/piano-disconnected-barlines.mei");

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct HeadGeometry {
    system_id: usize,
    shape: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[test]
fn clean_verovio_reduction_matches_java_heads_at_three_raster_scales() {
    assert!(
        SOURCE_MEI
            .windows(6)
            .filter(|window| *window == b"<note ")
            .count()
            >= 42,
        "the checked-in MEI source must remain the 42-note truth fixture"
    );

    let cases = [
        (
            "disconnected-1x.png",
            113,
            [(1113, 118, 17, 10), (1112, 236, 17, 10)],
        ),
        (
            "disconnected-1_5x.png",
            118,
            [(1669, 176, 26, 16), (1669, 354, 26, 16)],
        ),
        (
            "disconnected-2x.png",
            90,
            [(2224, 234, 37, 22), (2224, 472, 37, 22)],
        ),
    ];

    for (page, expected_raw_heads, expected_whole_bounds) in cases {
        let grid = recognize_grid_lines(fixture_path(page)).expect("clean-score GRID");
        let headers = recognize_native_headers(&grid).expect("clean-score HEADERS");
        let stem_seeds =
            recognize_native_stem_seeds(&grid, &headers).expect("clean-score STEM_SEEDS");
        let beams =
            recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
                .expect("clean-score BEAMS");
        let ledgers = recognize_native_ledgers(&grid, &beams).expect("clean-score LEDGERS");
        let heads = recognize_native_heads(&grid, &headers, &stem_seeds, &beams, &ledgers)
            .expect("clean-score HEADS");
        assert_eq!(
            heads.epilog.final_head_count, expected_raw_heads,
            "{page} raw HEADS parity changed"
        );
        let stems =
            recognize_native_stems(&grid, &headers, &stem_seeds, &beams, &ledgers, &heads, 1)
                .expect("clean-score STEMS");
        let reduction = recognize_native_reduction(&grid, stems).expect("clean-score REDUCTION");

        let mut actual = reduction
            .stems
            .systems
            .iter()
            .flat_map(|system| {
                system
                    .transaction
                    .state_after
                    .beam_state
                    .sig
                    .vertices
                    .iter()
                    .filter(|vertex| vertex.active && vertex.kind == NativeSigInterKind::Head)
                    .map(|vertex| HeadGeometry {
                        system_id: system.system_id,
                        shape: vertex.shape.clone().expect("live head shape"),
                        x: vertex.bounds.x,
                        y: vertex.bounds.y,
                        width: vertex.bounds.width,
                        height: vertex.bounds.height,
                    })
            })
            .collect::<Vec<_>>();
        actual.sort();

        let expected = java_heads(page);
        assert_eq!(actual.len(), 42, "{page} must retain all MEI noteheads");
        assert_eq!(
            actual, expected,
            "{page} reduced survivor geometry must match Java exactly"
        );

        let mut whole_bounds = actual
            .iter()
            .filter(|head| head.shape == "WHOLE_NOTE")
            .map(|head| (head.x, head.y, head.width, head.height))
            .collect::<Vec<_>>();
        whole_bounds.sort();
        let mut expected_whole_bounds = expected_whole_bounds.to_vec();
        expected_whole_bounds.sort();
        assert_eq!(
            whole_bounds, expected_whole_bounds,
            "{page} must retain both far-right stemless whole notes"
        );
    }
}

fn java_heads(page: &str) -> Vec<HeadGeometry> {
    let mut heads = JAVA_ORACLE
        .lines()
        .filter_map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            (fields.first() == Some(&"reductionhead") && fields.get(1) == Some(&page)).then(|| {
                HeadGeometry {
                    system_id: fields[3].parse().expect("Java system id"),
                    shape: fields[5].to_owned(),
                    x: fields[7].parse().expect("Java head x"),
                    y: fields[8].parse().expect("Java head y"),
                    width: fields[9].parse().expect("Java head width"),
                    height: fields[10].parse().expect("Java head height"),
                }
            })
        })
        .collect::<Vec<_>>();
    heads.sort();
    assert_eq!(heads.len(), 42, "Java oracle for {page}");
    heads
}

fn fixture_path(page: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../rust/oracle/clean-reduction-disconnected")
        .join(page)
}
