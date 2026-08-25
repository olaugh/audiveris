// SPDX-License-Identifier: AGPL-3.0-or-later

//! Focused production smoke test for the single native HEADS entry point.

use std::path::PathBuf;

use audiveris_omr::{
    head_template::HeadTemplateShape,
    native_headers::recognize_native_headers,
    native_heads::{recognize_native_heads, recognize_native_heads_with_small_heads},
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::recognize_native_stem_seeds,
    native_stems_head_corners::materialize_native_stems_head_corners,
    native_stems_head_seeds::materialize_native_stems_head_seeds,
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

    let stem_corners = materialize_native_stems_head_corners(&heads, &stem_seeds)
        .expect("STEMS head-corner prolog");
    assert_eq!(stem_corners.head_count, 326);
    assert_eq!(stem_corners.corner_count, 1_304);
    assert_eq!(
        stem_corners
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect::<Vec<_>>(),
        expected_systems
    );

    let stem_head_seeds = materialize_native_stems_head_seeds(&grid, &stem_seeds, &stem_corners)
        .expect("STEMS head seed selection");
    assert_eq!(stem_head_seeds.input_seed_count, 190);
    assert_eq!(stem_head_seeds.kept_seed_count, 164);
    assert_eq!(stem_head_seeds.purged_seed_count, 26);
    assert_eq!(stem_head_seeds.corner_count, 1_304);
    assert_eq!(stem_head_seeds.selected_seed_count, 346);
    assert_eq!(stem_head_seeds.section_fallback_count, 958);
    assert_eq!(stem_head_seeds.visited_candidate_count, 649);
    assert_eq!(
        stem_head_seeds.selected_seed_count + stem_head_seeds.section_fallback_count,
        stem_head_seeds.corner_count
    );
}

#[test]
fn small_heads_switch_matches_batuque_live_java_shape_census() {
    let grid = recognize_grid_lines(repo_path("data/examples/batuque.png")).expect("GRID");
    let headers = recognize_native_headers(&grid).expect("HEADERS");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS");
    let ledgers = recognize_native_ledgers(&grid, &beams).expect("LEDGERS");
    let heads = recognize_native_heads_with_small_heads(
        &grid,
        &headers,
        &stem_seeds,
        &beams,
        &ledgers,
        true,
    )
    .expect("small-head HEADS");

    let mut counts = [0_usize; 8];
    for system in &heads.epilog.systems {
        let staff_system = &heads.epilog.staff_epilog.systems[system.system_id - 1];
        for reference in &system.final_heads {
            let head = &staff_system.staffs[reference.staff_index].heads[reference.head_index];
            counts[head.shape.factory_ordinal()] += 1;
        }
    }
    assert_eq!(
        counts,
        [155, 170, 0, 0, 221, 150, 0, 0],
        "exact Java live HEADS shape census"
    );
    assert_eq!(heads.epilog.final_head_count, 696);
    assert!(heads.scanners.systems.iter().all(|system| {
        system
            .staffs
            .iter()
            .filter(|staff| !staff.tablature)
            .flat_map(|staff| &staff.geometries)
            .all(|geometry| {
                geometry
                    .all_shapes
                    .contains(&HeadTemplateShape::NoteheadBlackSmall)
            })
    }));

    let stem_corners = materialize_native_stems_head_corners(&heads, &stem_seeds)
        .expect("small-head STEMS corner prolog");
    assert_eq!(stem_corners.head_count, 696);
    assert_eq!(stem_corners.corner_count, 2_784);
}

fn repo_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(path)
}
