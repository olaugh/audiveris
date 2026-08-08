// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{collections::BTreeSet, path::PathBuf};

use audiveris_image::{
    glyph_factory::GlyphComponent,
    run_table::{FOREGROUND, Orientation, RunTable},
};
use audiveris_omr::{
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads_prolog,
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::recognize_native_stem_seeds,
    recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds},
};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const VALUE_UNKNOWN: i32 = -1;

#[derive(Debug)]
struct OracleHeadSpots {
    orientation: String,
    width: usize,
    height: usize,
    runs: usize,
    weight: usize,
    pixel_hash: u64,
}

#[derive(Debug)]
struct OracleBinary {
    width: usize,
    height: usize,
    black: usize,
    pixel_hash: u64,
}

#[derive(Debug)]
struct OracleDistances {
    width: usize,
    height: usize,
    normalizer: i32,
    unknown: usize,
    signed_i32_hash: u64,
}

#[derive(Debug)]
struct OracleComponent {
    ordinal: usize,
    left: i32,
    top: i32,
    width: usize,
    height: usize,
    weight: usize,
    centroid_x: i32,
    centroid_y: i32,
    runs: usize,
    run_hash: u64,
}

#[derive(Debug)]
struct OracleDispatch {
    system_id: usize,
    count: usize,
    ordinals: Vec<usize>,
}

#[derive(Debug)]
struct OraclePage {
    key: String,
    width: usize,
    height: usize,
    systems: usize,
    staves: Option<usize>,
    line_glyphs: Option<usize>,
    final_ledgers: Option<usize>,
    accepted_seeds: Option<usize>,
    head_spots: Option<OracleHeadSpots>,
    binary: Option<OracleBinary>,
    distances: Option<OracleDistances>,
    components: Vec<OracleComponent>,
    component_summary: Option<(usize, u64)>,
    dispatches: Vec<OracleDispatch>,
    dispatch_summary: Option<(usize, usize, usize, u64)>,
    summary: Option<(usize, usize, usize, usize, usize, usize)>,
}

impl OraclePage {
    fn new(fields: &[&str]) -> Self {
        assert_eq!(fields.len(), 8, "headsprologpage field count");
        assert_eq!(&fields[2..=2], &["width"]);
        assert_eq!(&fields[4..=4], &["height"]);
        assert_eq!(&fields[6..=6], &["systems"]);
        Self {
            key: fields[1].to_owned(),
            width: number(fields[3]),
            height: number(fields[5]),
            systems: number(fields[7]),
            staves: None,
            line_glyphs: None,
            final_ledgers: None,
            accepted_seeds: None,
            head_spots: None,
            binary: None,
            distances: None,
            components: Vec::new(),
            component_summary: None,
            dispatches: Vec::new(),
            dispatch_summary: None,
            summary: None,
        }
    }

    fn assert_complete(&self) {
        assert!(self.staves.is_some(), "{}: no staff input", self.key);
        assert!(self.line_glyphs.is_some(), "{}: no line input", self.key);
        assert!(
            self.final_ledgers.is_some(),
            "{}: no ledger input",
            self.key
        );
        assert!(self.accepted_seeds.is_some(), "{}: no seed input", self.key);
        assert!(self.head_spots.is_some(), "{}: no HEAD_SPOTS", self.key);
        assert!(self.binary.is_some(), "{}: no binary", self.key);
        assert!(self.distances.is_some(), "{}: no distances", self.key);
        assert!(
            self.component_summary.is_some(),
            "{}: no component summary",
            self.key
        );
        assert!(
            self.dispatch_summary.is_some(),
            "{}: no dispatch summary",
            self.key
        );
        assert!(self.summary.is_some(), "{}: no page summary", self.key);
    }
}

fn number<T: std::str::FromStr>(text: &str) -> T
where
    T::Err: std::fmt::Debug,
{
    text.parse().expect("a decimal oracle value")
}

fn hexadecimal(text: &str) -> u64 {
    u64::from_str_radix(text, 16).expect("a hexadecimal oracle digest")
}

fn current<'a>(pages: &'a mut [OraclePage], key: &str) -> &'a mut OraclePage {
    let page = pages.last_mut().expect("a page record before its rows");
    assert_eq!(page.key, key, "oracle rows remain page-contiguous");
    page
}

fn parse_oracle() -> Vec<OraclePage> {
    let mut pages = Vec::<OraclePage>::new();
    for line in include_str!("../../../oracle/heads-prolog.txt").lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split_whitespace().collect::<Vec<_>>();
        match fields[0] {
            "headsprologpage" => pages.push(OraclePage::new(&fields)),
            "headsprologstaffinput" => {
                assert_eq!(fields.len(), 7);
                assert_eq!(fields[2], "staves");
                assert_eq!(fields[4], "lineGlyphs");
                let page = current(&mut pages, fields[1]);
                page.staves = Some(number(fields[3]));
                page.line_glyphs = Some(number(fields[5]));
                let _ = hexadecimal(fields[6]);
            }
            "headsprologledgerinput" => {
                assert_eq!(fields.len(), 5);
                assert_eq!(fields[2], "finalLedgers");
                let page = current(&mut pages, fields[1]);
                page.final_ledgers = Some(number(fields[3]));
                let _ = hexadecimal(fields[4]);
            }
            "headsprologseedinput" => {
                assert_eq!(fields.len(), 5);
                assert_eq!(fields[2], "acceptedSeeds");
                let page = current(&mut pages, fields[1]);
                page.accepted_seeds = Some(number(fields[3]));
                let _ = hexadecimal(fields[4]);
            }
            "headsprologheadspots" => {
                assert_eq!(fields.len(), 13);
                assert_eq!(
                    [fields[2], fields[4], fields[6], fields[8], fields[10]],
                    ["orientation", "width", "height", "runs", "weight"]
                );
                current(&mut pages, fields[1]).head_spots = Some(OracleHeadSpots {
                    orientation: fields[3].to_owned(),
                    width: number(fields[5]),
                    height: number(fields[7]),
                    runs: number(fields[9]),
                    weight: number(fields[11]),
                    pixel_hash: hexadecimal(fields[12]),
                });
            }
            "headsprologbinary" => {
                assert_eq!(fields.len(), 9);
                assert_eq!(
                    [fields[2], fields[4], fields[6]],
                    ["width", "height", "black"]
                );
                current(&mut pages, fields[1]).binary = Some(OracleBinary {
                    width: number(fields[3]),
                    height: number(fields[5]),
                    black: number(fields[7]),
                    pixel_hash: hexadecimal(fields[8]),
                });
            }
            "headsprologdistances" => {
                assert_eq!(fields.len(), 11);
                assert_eq!(
                    [fields[2], fields[4], fields[6], fields[8]],
                    ["width", "height", "normalizer", "unknown"]
                );
                current(&mut pages, fields[1]).distances = Some(OracleDistances {
                    width: number(fields[3]),
                    height: number(fields[5]),
                    normalizer: number(fields[7]),
                    unknown: number(fields[9]),
                    signed_i32_hash: hexadecimal(fields[10]),
                });
            }
            "headsprologcomponent" => {
                assert_eq!(fields.len(), 17);
                assert_eq!(
                    [fields[2], fields[4], fields[9], fields[11], fields[14]],
                    ["ordinal", "bounds", "weight", "centroid", "runs"]
                );
                current(&mut pages, fields[1])
                    .components
                    .push(OracleComponent {
                        ordinal: number(fields[3]),
                        left: number(fields[5]),
                        top: number(fields[6]),
                        width: number(fields[7]),
                        height: number(fields[8]),
                        weight: number(fields[10]),
                        centroid_x: number(fields[12]),
                        centroid_y: number(fields[13]),
                        runs: number(fields[15]),
                        run_hash: hexadecimal(fields[16]),
                    });
            }
            "headsprologcomponentsummary" => {
                assert_eq!(fields.len(), 5);
                assert_eq!(fields[2], "components");
                current(&mut pages, fields[1]).component_summary =
                    Some((number(fields[3]), hexadecimal(fields[4])));
            }
            "headsprologdispatch" => {
                assert_eq!(fields.len(), 8);
                assert_eq!(
                    [fields[2], fields[4], fields[6]],
                    ["system", "count", "ordinals"]
                );
                let ordinals = if fields[7] == "-" {
                    Vec::new()
                } else {
                    fields[7].split(',').map(number).collect()
                };
                current(&mut pages, fields[1])
                    .dispatches
                    .push(OracleDispatch {
                        system_id: number(fields[3]),
                        count: number(fields[5]),
                        ordinals,
                    });
            }
            "headsprologdispatchsummary" => {
                assert_eq!(fields.len(), 9);
                assert_eq!(
                    [fields[2], fields[4], fields[6]],
                    ["systems", "references", "uniqueComponents"]
                );
                current(&mut pages, fields[1]).dispatch_summary = Some((
                    number(fields[3]),
                    number(fields[5]),
                    number(fields[7]),
                    hexadecimal(fields[8]),
                ));
            }
            "headsprologsummary" => {
                assert_eq!(fields.len(), 14);
                assert_eq!(
                    [
                        fields[2], fields[4], fields[6], fields[8], fields[10], fields[12]
                    ],
                    [
                        "staves",
                        "lineGlyphs",
                        "finalLedgers",
                        "acceptedSeeds",
                        "components",
                        "systems"
                    ]
                );
                current(&mut pages, fields[1]).summary = Some((
                    number(fields[3]),
                    number(fields[5]),
                    number(fields[7]),
                    number(fields[9]),
                    number(fields[11]),
                    number(fields[13]),
                ));
            }
            other => panic!("unknown HEADS prolog oracle record {other}"),
        }
    }

    assert_eq!(pages.len(), 8, "the full beam-sheet corpus is pinned");
    for page in &pages {
        page.assert_complete();
        assert_eq!(page.components.len(), page.component_summary.unwrap().0);
        assert_eq!(page.dispatches.len(), page.dispatch_summary.unwrap().0);
    }
    pages
}

fn repo_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(path)
}

fn fnv_bytes(bytes: impl IntoIterator<Item = u8>) -> u64 {
    bytes.into_iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    })
}

fn hash_rows<'a>(rows: impl IntoIterator<Item = &'a str>) -> u64 {
    let mut hash = FNV_OFFSET;
    for row in rows {
        for byte in row.bytes().chain(std::iter::once(b'\n')) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

fn signed_i32_hash(values: &[i32]) -> u64 {
    fnv_bytes(values.iter().flat_map(|value| value.to_le_bytes()))
}

fn orientation_name(orientation: Orientation) -> &'static str {
    match orientation {
        Orientation::Horizontal => "HORIZONTAL",
        Orientation::Vertical => "VERTICAL",
    }
}

fn component_run_hash(component: &GlyphComponent) -> u64 {
    let (sequence_origin, coordinate_origin, sequence_count) = match component.orientation {
        Orientation::Horizontal => (
            usize::try_from(component.top).expect("non-negative sheet coordinate"),
            usize::try_from(component.left).expect("non-negative sheet coordinate"),
            component.height,
        ),
        Orientation::Vertical => (
            usize::try_from(component.left).expect("non-negative sheet coordinate"),
            usize::try_from(component.top).expect("non-negative sheet coordinate"),
            component.width,
        ),
    };
    let mut rows = Vec::with_capacity(sequence_count + 1);
    rows.push(format!(
        "{} {} {}",
        orientation_name(component.orientation),
        component.width,
        component.height
    ));
    for local_sequence in 0..sequence_count {
        let source_sequence = sequence_origin + local_sequence;
        let mut row = local_sequence.to_string();
        for entry in component
            .runs
            .iter()
            .filter(|entry| entry.sequence == source_sequence)
        {
            let local_start = entry
                .run
                .start
                .checked_sub(coordinate_origin)
                .expect("component run starts within its bounds");
            row.push(' ');
            row.push_str(&format!("{local_start}:{}", entry.run.length));
        }
        rows.push(row);
    }
    hash_rows(rows.iter().map(String::as_str))
}

fn component_row(page: &str, ordinal: usize, component: &GlyphComponent) -> String {
    format!(
        "headsprologcomponent {page} ordinal {ordinal} bounds {} {} {} {} weight {} centroid {} {} runs {} {:016x}",
        component.left,
        component.top,
        component.width,
        component.height,
        component.weight,
        component.rounded_centroid_x,
        component.rounded_centroid_y,
        component.runs.len(),
        component_run_hash(component)
    )
}

fn dispatch_row(page: &str, system_id: usize, ordinals: &[usize]) -> String {
    let ordinals_text = if ordinals.is_empty() {
        "-".to_owned()
    } else {
        ordinals
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    format!(
        "headsprologdispatch {page} system {system_id} count {} ordinals {ordinals_text}",
        ordinals.len()
    )
}

fn run_table_pixel_hash(table: &RunTable) -> u64 {
    fnv_bytes(table.to_pixels())
}

#[test]
fn native_heads_prolog_matches_java_on_every_beam_sheet() {
    for oracle in parse_oracle() {
        let image = oracle.key.split('#').next().expect("a page file name");
        let grid = recognize_grid_lines(repo_path(&format!("data/examples/{image}")))
            .unwrap_or_else(|error| panic!("{}: GRID failed: {error}", oracle.key));
        let headers = recognize_native_headers(&grid)
            .unwrap_or_else(|error| panic!("{}: HEADERS failed: {error}", oracle.key));
        let stem_seeds = recognize_native_stem_seeds(&grid, &headers)
            .unwrap_or_else(|error| panic!("{}: STEM_SEEDS failed: {error}", oracle.key));
        let beams =
            recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
                .unwrap_or_else(|error| panic!("{}: BEAMS failed: {error}", oracle.key));
        let ledgers = recognize_native_ledgers(&grid, &beams)
            .unwrap_or_else(|error| panic!("{}: LEDGERS failed: {error}", oracle.key));
        let heads = recognize_native_heads_prolog(&grid, &beams, &ledgers, &stem_seeds)
            .unwrap_or_else(|error| panic!("{}: HEADS prolog failed: {error}", oracle.key));

        assert_eq!(
            (
                grid.scale.width,
                grid.scale.height,
                grid.peak_graph.systems.len()
            ),
            (oracle.width, oracle.height, oracle.systems),
            "{}: page boundary",
            oracle.key
        );
        let staff_ids = grid
            .peak_graph
            .systems
            .iter()
            .flatten()
            .copied()
            .collect::<BTreeSet<_>>();
        let line_glyphs = grid
            .peak_graph
            .sheet_staffs
            .iter()
            .filter(|staff| staff_ids.contains(&staff.id))
            .map(|staff| staff.lines.len())
            .sum::<usize>();
        let upstream = (
            staff_ids.len(),
            line_glyphs,
            ledgers.ledgers().len(),
            stem_seeds
                .systems
                .iter()
                .map(|system| system.free_glyphs.len())
                .sum::<usize>(),
        );
        assert_eq!(
            upstream,
            (
                oracle.staves.unwrap(),
                oracle.line_glyphs.unwrap(),
                oracle.final_ledgers.unwrap(),
                oracle.accepted_seeds.unwrap()
            ),
            "{}: upstream counts",
            oracle.key
        );

        let expected_spots = oracle.head_spots.as_ref().unwrap();
        assert_eq!(
            (
                orientation_name(beams.head_spot_runs.orientation()),
                beams.head_spot_runs.width(),
                beams.head_spot_runs.height(),
                beams.head_spot_runs.total_run_count(),
                beams.head_spot_runs.weight(),
                run_table_pixel_hash(&beams.head_spot_runs)
            ),
            (
                expected_spots.orientation.as_str(),
                expected_spots.width,
                expected_spots.height,
                expected_spots.runs,
                expected_spots.weight,
                expected_spots.pixel_hash
            ),
            "{}: HEAD_SPOTS",
            oracle.key
        );

        let expected_binary = oracle.binary.as_ref().unwrap();
        assert_eq!(
            (
                grid.scale.width,
                grid.scale.height,
                heads
                    .binary_without_lines
                    .iter()
                    .filter(|&&pixel| pixel == FOREGROUND)
                    .count(),
                fnv_bytes(heads.binary_without_lines.iter().copied())
            ),
            (
                expected_binary.width,
                expected_binary.height,
                expected_binary.black,
                expected_binary.pixel_hash
            ),
            "{}: neutralized binary",
            oracle.key
        );

        let expected_distances = oracle.distances.as_ref().unwrap();
        assert_eq!(
            (
                heads.distance_table.width,
                heads.distance_table.height,
                heads.distance_table.normalizer,
                heads
                    .distance_table
                    .values
                    .iter()
                    .filter(|&&value| value == VALUE_UNKNOWN)
                    .count(),
                signed_i32_hash(&heads.distance_table.values)
            ),
            (
                expected_distances.width,
                expected_distances.height,
                expected_distances.normalizer,
                expected_distances.unknown,
                expected_distances.signed_i32_hash
            ),
            "{}: distance table",
            oracle.key
        );

        assert_eq!(heads.spots.len(), oracle.components.len());
        let mut component_rows = Vec::with_capacity(heads.spots.len());
        for (ordinal, (spot, expected)) in heads.spots.iter().zip(&oracle.components).enumerate() {
            assert_eq!(expected.ordinal, ordinal, "{}: component order", oracle.key);
            let component = spot
                .component
                .as_ref()
                .expect("native HEAD_SPOT retains its component");
            assert_eq!(
                (
                    component.left,
                    component.top,
                    component.width,
                    component.height,
                    component.weight,
                    component.rounded_centroid_x,
                    component.rounded_centroid_y,
                    component.runs.len(),
                    component_run_hash(component)
                ),
                (
                    expected.left,
                    expected.top,
                    expected.width,
                    expected.height,
                    expected.weight,
                    expected.centroid_x,
                    expected.centroid_y,
                    expected.runs,
                    expected.run_hash
                ),
                "{}: component ordinal {}",
                oracle.key,
                expected.ordinal
            );
            component_rows.push(component_row(&oracle.key, expected.ordinal, component));
        }
        let expected_component_summary = oracle.component_summary.unwrap();
        assert_eq!(
            (
                component_rows.len(),
                hash_rows(component_rows.iter().map(String::as_str))
            ),
            expected_component_summary,
            "{}: component summary",
            oracle.key
        );

        assert_eq!(heads.system_spot_ordinals.len(), oracle.dispatches.len());
        let mut dispatch_rows = Vec::with_capacity(heads.system_spot_ordinals.len());
        for ((system_id, ordinals), expected) in
            heads.system_spot_ordinals.iter().zip(&oracle.dispatches)
        {
            let actual_set = ordinals.iter().copied().collect::<BTreeSet<_>>();
            let expected_set = expected.ordinals.iter().copied().collect::<BTreeSet<_>>();
            let missing_diagnostics = expected_set
                .difference(&actual_set)
                .map(|&ordinal| {
                    let spot = &heads.spots[ordinal];
                    let area = &grid.system_areas[*system_id - 1];
                    let bounds = &grid.system_bounds[*system_id - 1];
                    format!(
                        "ordinal {ordinal} center ({}, {}) relevant {:?}, area x [{}, {}), north {:?}, south {:?}, contains {}, system x [{}, staff-max {}, Java-right {}]",
                        spot.center_x,
                        spot.center_y,
                        spot.relevant_system_ids,
                        area.left,
                        area.right,
                        area.north().y_at_x(f64::from(spot.center_x)),
                        area.south().y_at_x(f64::from(spot.center_x)),
                        area.contains(f64::from(spot.center_x), f64::from(spot.center_y)),
                        bounds.left,
                        bounds.right,
                        bounds.java_right()
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(
                (*system_id, ordinals.len(), ordinals.as_slice()),
                (
                    expected.system_id,
                    expected.count,
                    expected.ordinals.as_slice()
                ),
                "{}: system {} dispatch; missing diagnostics: {missing_diagnostics:?}",
                oracle.key,
                expected.system_id
            );
            dispatch_rows.push(dispatch_row(&oracle.key, *system_id, ordinals));
        }
        let references = heads
            .system_spot_ordinals
            .iter()
            .map(|(_, ordinals)| ordinals.len())
            .sum::<usize>();
        let unique_components = heads
            .system_spot_ordinals
            .iter()
            .flat_map(|(_, ordinals)| ordinals)
            .copied()
            .collect::<BTreeSet<_>>()
            .len();
        assert_eq!(
            (
                dispatch_rows.len(),
                references,
                unique_components,
                hash_rows(dispatch_rows.iter().map(String::as_str))
            ),
            oracle.dispatch_summary.unwrap(),
            "{}: dispatch summary",
            oracle.key
        );
        assert_eq!(
            (
                upstream.0,
                upstream.1,
                upstream.2,
                upstream.3,
                heads.spots.len(),
                heads.system_spot_ordinals.len()
            ),
            oracle.summary.unwrap(),
            "{}: page summary",
            oracle.key
        );
    }
}
