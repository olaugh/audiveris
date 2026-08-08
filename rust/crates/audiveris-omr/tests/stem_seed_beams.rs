// SPDX-License-Identifier: AGPL-3.0-or-later

//! Focused composition gate from native STEM_SEEDS into BEAMS extension.

use std::path::{Path, PathBuf};

use audiveris_omr::{
    native_headers::recognize_native_headers,
    native_stem_seeds::recognize_native_stem_seeds,
    recognize::{
        NativeBeamRecognitionError, native_stem_seeds_for_beams, recognize_grid_lines,
        recognize_native_beams, recognize_native_beams_with_stem_seeds,
    },
};

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}

#[derive(Debug)]
struct FrozenBeam {
    system_id: usize,
    class_name: String,
    shape: String,
    median: [f64; 4],
    height: f64,
    grade: f64,
    impacts: [f64; 6],
}

fn parse_hex_f64(text: &str) -> f64 {
    let (mantissa, exponent) = text.split_once('p').expect("Java hex float has exponent");
    let exponent: i32 = exponent.parse().expect("Java hex float exponent");
    let mantissa = mantissa.strip_prefix("0x").expect("Java hex float prefix");
    let (whole, fraction) = mantissa.split_once('.').expect("Java hex float point");
    let whole = u64::from_str_radix(whole, 16).expect("Java hex float whole part") as f64;
    let fraction = fraction
        .bytes()
        .enumerate()
        .map(|(index, digit)| {
            let digit = char::from(digit)
                .to_digit(16)
                .expect("Java hex float fraction digit");
            f64::from(digit) / 16_f64.powi(i32::try_from(index + 1).expect("fraction index"))
        })
        .sum::<f64>();
    (whole + fraction) * 2_f64.powi(exponent)
}

fn parse_impact(text: &str, expected_name: &str) -> f64 {
    let fields = text.split(':').collect::<Vec<_>>();
    assert_eq!(
        fields.len(),
        3,
        "impact must contain name, weight, and value"
    );
    assert_eq!(fields[0], expected_name);
    let _weight = parse_hex_f64(fields[1]);
    parse_hex_f64(fields[2])
}

fn parse_frozen_beam(line: &str) -> FrozenBeam {
    let fields = line.split_whitespace().collect::<Vec<_>>();
    assert!(matches!(
        fields[0],
        "beamstemseedremoved" | "beamstemseedadded"
    ));
    assert_eq!(fields[2], "system");
    assert_eq!(fields[4], "beam");
    assert_eq!(fields[7], "median");
    assert_eq!(fields[12], "height");
    assert_eq!(fields[14], "grade");
    assert_eq!(fields[16], "impacts");
    FrozenBeam {
        system_id: fields[3].parse().expect("system id"),
        class_name: fields[5].to_owned(),
        shape: fields[6].to_owned(),
        median: std::array::from_fn(|index| parse_hex_f64(fields[8 + index])),
        height: parse_hex_f64(fields[13]),
        grade: parse_hex_f64(fields[15]),
        impacts: [
            parse_impact(fields[17], "wdth"),
            parse_impact(fields[18], "minH"),
            parse_impact(fields[19], "maxH"),
            parse_impact(fields[20], "core"),
            parse_impact(fields[21], "belt"),
            parse_impact(fields[22], "jit"),
        ],
    }
}

fn multiset_only_in(
    left: &[(usize, audiveris_omr::beam_inters::RawBeam)],
    right: &[(usize, audiveris_omr::beam_inters::RawBeam)],
) -> Vec<(usize, audiveris_omr::beam_inters::RawBeam)> {
    let mut only = left.to_vec();
    for candidate in right {
        if let Some(index) = only.iter().position(|beam| beam == candidate) {
            only.remove(index);
        }
    }
    only
}

fn assert_exact_f64(label: &str, actual: f64, expected: f64) {
    assert_eq!(
        actual.to_bits(),
        expected.to_bits(),
        "{label}: Rust {actual:?}, Java {expected:?}"
    );
}

fn assert_frozen_beam(
    actual: &(usize, audiveris_omr::beam_inters::RawBeam),
    expected: &FrozenBeam,
) {
    let (system_id, raw) = actual;
    assert_eq!(*system_id, expected.system_id);
    assert_eq!(raw.kind.class_name(), expected.class_name);
    assert_eq!(raw.kind.shape(), expected.shape);
    for (index, (actual, expected)) in [
        raw.item.median.x1,
        raw.item.median.y1,
        raw.item.median.x2,
        raw.item.median.y2,
    ]
    .into_iter()
    .zip(expected.median)
    .enumerate()
    {
        assert_exact_f64(&format!("median[{index}]"), actual, expected);
    }
    assert_exact_f64("height", raw.item.height, expected.height);
    assert_exact_f64("grade", raw.grade, expected.grade);
    for (label, actual, expected) in ["wdth", "minH", "maxH", "core", "belt", "jit"]
        .into_iter()
        .zip([
            raw.impacts.width,
            raw.impacts.min_height,
            raw.impacts.max_height,
            raw.impacts.core,
            raw.impacts.belt,
            raw.impacts.distance,
        ])
        .zip(expected.impacts)
        .map(|((label, actual), expected)| (label, actual, expected))
    {
        assert_exact_f64(label, actual, expected);
    }
}

#[test]
fn java_counterfactual_freezes_zero_observable_stem_extension_effect() {
    let fixture = std::fs::read_to_string(repo_path("rust/oracle/beam-stem-seeds.txt"))
        .expect("the production BEAMS/STEM_SEEDS counterfactual is checked in");
    assert_eq!(
        fixture
            .lines()
            .filter(|line| line.starts_with("beamstemseedstate "))
            .count(),
        30
    );
    assert_eq!(
        fixture
            .lines()
            .find(|line| line.starts_with("beamstemseedcorpussummary ")),
        Some(
            "beamstemseedcorpussummary sheets 8 systems 30 seeds 1906 beams 803 groups 493 rests 1 changedSystems 0 changedRecords 0"
        )
    );
}

#[test]
fn d039_natural_stem_extension_matches_java_exactly() {
    let fixture = include_str!("../../../oracle/beam-stem-seeds-d039.txt");
    assert_eq!(
        fixture
            .lines()
            .filter(|line| line.starts_with("beamstemseedcompare "))
            .count(),
        4
    );
    assert_eq!(
        fixture
            .lines()
            .filter(|line| line.starts_with("beamstemseedremoved "))
            .count(),
        2
    );
    assert_eq!(
        fixture
            .lines()
            .filter(|line| line.starts_with("beamstemseedadded "))
            .count(),
        2
    );
    assert_eq!(
        fixture
            .lines()
            .find(|line| line.starts_with("beamstemseedcorpussummary ")),
        Some(
            "beamstemseedcorpussummary sheets 1 systems 4 seeds 465 seededBeams 69 emptyBeams 69 seededGroups 56 emptyGroups 56 changedSystems 1 removed 2 added 2 5acbd8b3dd4d1405"
        )
    );
    let seeded_java = parse_frozen_beam(
        fixture
            .lines()
            .find(|line| {
                line.starts_with("beamstemseedremoved ") && line.contains(" system 2 beam ")
            })
            .expect("seeded-only Java beam"),
    );
    let empty_java = parse_frozen_beam(
        fixture
            .lines()
            .find(|line| line.starts_with("beamstemseedadded ") && line.contains(" system 2 beam "))
            .expect("hidden-seed Java beam"),
    );

    let grid = recognize_grid_lines(repo_path("data/examples/D0392410-1.256.png")).expect("GRID");
    let headers = recognize_native_headers(&grid).expect("HEADERS");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS");
    assert_eq!(
        stem_seeds
            .systems
            .iter()
            .map(|system| system.free_glyphs.len())
            .collect::<Vec<_>>(),
        [87, 76, 142, 160]
    );

    let seeded = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("seeded BEAMS");
    let empty = recognize_native_beams(&grid, headers.beam_erases()).expect("hidden-seed BEAMS");
    let seeded_only = multiset_only_in(&seeded.raw_beams, &empty.raw_beams);
    let empty_only = multiset_only_in(&empty.raw_beams, &seeded.raw_beams);

    assert_eq!(
        seeded_only.len(),
        1,
        "exactly one production beam is extended"
    );
    assert_eq!(
        empty_only.len(),
        1,
        "exactly one unextended beam is replaced"
    );
    assert_frozen_beam(&seeded_only[0], &seeded_java);
    assert_frozen_beam(&empty_only[0], &empty_java);

    // Java reports two canonical deltas in each direction because its one-member
    // BeamGroupInter record embeds the changed beam. Production Rust exposes the
    // beam itself and group counts; everything else remains byte-for-byte stable.
    assert_eq!(seeded.spot_count, empty.spot_count);
    assert_eq!(seeded.hooks, empty.hooks);
    assert_eq!(seeded.group_counts, empty.group_counts);
    assert_eq!(seeded.group_count, empty.group_count);
    assert_eq!(seeded.raw_beams.len(), empty.raw_beams.len());
}

#[test]
fn accepted_stems_feed_beam_extension_in_system_and_candidate_order() {
    let grid = recognize_grid_lines(repo_path("data/examples/chula.png")).expect("GRID");
    let headers = recognize_native_headers(&grid).expect("HEADERS");
    let mut stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS");

    let adapted = native_stem_seeds_for_beams(&grid, &stem_seeds).expect("stem adapter");
    assert_eq!(
        adapted
            .iter()
            .map(|system| (system.system_id, system.seeds.len()))
            .collect::<Vec<_>>(),
        [(1, 57), (2, 60), (3, 73)]
    );
    for (actual_system, source_system) in adapted.iter().zip(&stem_seeds.systems) {
        assert_eq!(actual_system.system_id, source_system.raw.system_id);
        for (actual, source) in actual_system.seeds.iter().zip(&source_system.free_glyphs) {
            assert_eq!(actual.id, source.source_ordinal);
            assert_eq!(actual.left, source.bounds.x as i32);
            assert_eq!(actual.top, source.bounds.y as i32);
            assert_eq!(actual.width, source.bounds.width);
            assert_eq!(actual.height, source.bounds.height);
            let median = actual.vertical_median.expect("a vertical seed median");
            assert_eq!(
                (median.x1, median.y1, median.x2, median.y2),
                (source.start.0, source.start.1, source.stop.0, source.stop.1)
            );
        }
    }

    stem_seeds.systems.swap(0, 1);
    assert!(matches!(
        native_stem_seeds_for_beams(&grid, &stem_seeds),
        Err(NativeBeamRecognitionError::StemSeedSystemOrder { .. })
    ));
    stem_seeds.systems.swap(0, 1);

    stem_seeds.systems[0].free_glyphs.swap(0, 1);
    assert!(matches!(
        native_stem_seeds_for_beams(&grid, &stem_seeds),
        Err(NativeBeamRecognitionError::InvalidStemSeed { system_id: 1, .. })
    ));
    stem_seeds.systems[0].free_glyphs.swap(0, 1);

    let duplicate = stem_seeds.systems[0].free_glyphs[0].clone();
    let duplicate_ordinal = duplicate.source_ordinal;
    stem_seeds.systems[0].free_glyphs.push(duplicate);
    assert!(matches!(
        native_stem_seeds_for_beams(&grid, &stem_seeds),
        Err(NativeBeamRecognitionError::DuplicateStemSeedOrdinal {
            system_id: 1,
            ordinal,
        }) if ordinal == duplicate_ordinal
    ));
    stem_seeds.systems[0].free_glyphs.pop();

    // Java does not successfully extend a chula beam to a stem, so composing
    // all 190 exact seeds must retain the already graded BEAMS result. This
    // exercises the real create -> extend(seeds) -> hooks -> groups entry point,
    // while the geometry assertions above prove the extension source is live.
    let compatibility =
        recognize_native_beams(&grid, headers.beam_erases()).expect("compatibility BEAMS");
    let composed =
        recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
            .expect("composed BEAMS");
    assert_eq!(composed.spot_count, compatibility.spot_count);
    assert_eq!(composed.raw_beams, compatibility.raw_beams);
    assert_eq!(composed.hooks, compatibility.hooks);
    assert_eq!(composed.group_counts, compatibility.group_counts);
    assert_eq!(composed.group_count, compatibility.group_count);
}

#[test]
fn composed_stem_extension_is_output_stable_on_all_graded_beam_pages() {
    let files = [
        "chula.png",
        "allegretto.png",
        "batuque.png",
        "carmen.png",
        "cucaracha.png",
        "hove.png",
        "zizi.png",
        "BachInvention5.jpg",
    ];
    let mut systems = 0;
    let mut seeds = 0;
    for file in files {
        let grid = recognize_grid_lines(repo_path(&format!("data/examples/{file}")))
            .unwrap_or_else(|error| panic!("{file}: GRID failed: {error}"));
        let headers = recognize_native_headers(&grid)
            .unwrap_or_else(|error| panic!("{file}: HEADERS failed: {error}"));
        let stem_seeds = recognize_native_stem_seeds(&grid, &headers)
            .unwrap_or_else(|error| panic!("{file}: STEM_SEEDS failed: {error}"));
        let adapted = native_stem_seeds_for_beams(&grid, &stem_seeds)
            .unwrap_or_else(|error| panic!("{file}: stem adapter failed: {error}"));
        systems += adapted.len();
        seeds += adapted
            .iter()
            .map(|system| system.seeds.len())
            .sum::<usize>();

        let compatibility = recognize_native_beams(&grid, headers.beam_erases())
            .unwrap_or_else(|error| panic!("{file}: compatibility BEAMS failed: {error}"));
        let composed =
            recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
                .unwrap_or_else(|error| panic!("{file}: composed BEAMS failed: {error}"));
        assert_eq!(composed.raw_beams, compatibility.raw_beams, "{file}");
        assert_eq!(composed.hooks, compatibility.hooks, "{file}");
        assert_eq!(composed.group_counts, compatibility.group_counts, "{file}");
    }
    assert_eq!(systems, 30);
    assert_eq!(seeds, 1_906);
}
