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
