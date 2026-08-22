// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production composition gate for Java `MultipleRestsBuilder`.

use std::path::{Path, PathBuf};

use audiveris_omr::{
    native_headers::recognize_native_headers,
    native_stem_seeds::recognize_native_stem_seeds,
    recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds},
};

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}

#[test]
fn bach_system_six_produces_one_identity_free_multiple_rest_replacement() {
    let grid = recognize_grid_lines(repo_path("data/examples/BachInvention5.jpg")).expect("GRID");
    let headers = recognize_native_headers(&grid).expect("HEADERS");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS");

    assert_eq!(beams.multiple_rests.len(), 1, "{:#?}", beams.multiple_rests);
    let rest = &beams.multiple_rests[0];
    assert_eq!(rest.source_beam_ordinal, 182);
    assert_eq!(rest.system_id, 6);
    assert_eq!(rest.staff_id, 12);
    assert_eq!(rest.grade.to_bits(), 0x3fe3_dacf_882d_0517);
    assert_eq!(rest.height.to_bits(), 0x4021_5a5a_5a5a_5a00);
    assert_eq!(rest.source_median.x1.to_bits(), 0x4092_7c00_0000_0000);
    assert_eq!(rest.source_median.y1.to_bits(), 0x40a2_9df1_bc82_362c);
    assert_eq!(rest.source_median.x2.to_bits(), 0x4094_1c00_0000_0000);
    assert_eq!(rest.source_median.y2.to_bits(), 0x40a2_9c88_473d_134b);
    assert_eq!(
        rest.source_bounds,
        audiveris_image::staff_peak::PeakBounds {
            x: 1183,
            y: 2377,
            width: 104,
            height: 11,
        }
    );
    // These two are port-pinned intermediates, not Java-verified values: Java's
    // oracle publishes this rest's grade and bounds (both asserted above, both
    // unchanged) but never its pitch. Both derive from `Staff.pitchPositionOf`,
    // which reads `getFirstLine().yAt(x)` / `getLastLine().yAt(x)`, so they moved
    // when `y_at_x_ext` was corrected to evaluate the spline as `LineInfo.yAt`
    // does. `stop_pitch` shifted 3987 ulp -- a pitch this close to zero comes out
    // of a near-cancellation that amplifies a one-ulp ordinate -- while
    // `start_pitch` was unaffected. Pitch is only read through
    // `abs() > MULTIPLE_REST_MAX_ABSOLUTE_PITCH`, so neither shift can change the
    // decision, and the rest is still detected with identical grade and bounds.
    assert_eq!(rest.start_pitch.to_bits(), 0x3fb9_12cb_541e_8116);
    assert_eq!(rest.stop_pitch.to_bits(), 0x3fac_76cd_f933_b240);
    assert_eq!(rest.serif_search.added_chunk, 5);
    assert_eq!(
        (
            rest.left_peak.top(),
            rest.left_peak.bottom(),
            rest.left_peak.start(),
            rest.left_peak.stop(),
        ),
        (2365, 2399, 1183, 1185)
    );
    assert_eq!(
        (
            rest.right_peak.top(),
            rest.right_peak.bottom(),
            rest.right_peak.start(),
            rest.right_peak.stop(),
        ),
        (2365, 2398, 1283, 1287)
    );
    assert_eq!(rest.left_peak, rest.serif_search.left.clone().unwrap());
    assert_eq!(rest.right_peak, rest.serif_search.right.clone().unwrap());

    let (source_system, source_beam) = beams.raw_beams[rest.source_beam_ordinal];
    assert_eq!(source_system, rest.system_id);
    assert_eq!(source_beam.item.median, rest.source_median);
    assert_eq!(source_beam.item.height, rest.height);
    assert_eq!(source_beam.grade, rest.grade);
    assert_eq!(
        beams.beams_after_multiple_rests.len() + 1,
        beams.raw_beams.len()
    );
    let expected = beams
        .raw_beams
        .iter()
        .enumerate()
        .filter(|(ordinal, _)| *ordinal != rest.source_beam_ordinal)
        .map(|(_, beam)| *beam)
        .collect::<Vec<_>>();
    assert_eq!(beams.beams_after_multiple_rests, expected);

    // `BeamsBuilder.buildBeams()` creates `BeamGroupInter` containment before
    // `BeamsStep` invokes `MultipleRestsBuilder`. Replacing the rest-like beam
    // removes that vertex and its containment edge, but does not geometrically
    // regroup the survivors. This preserved identity is observable in the
    // Java-backed HEADS and STEMS corpus fixtures.
    let post_rest_grouping_input = beams
        .beams_after_multiple_rests
        .iter()
        .chain(&beams.hooks)
        .filter(|(system_id, _)| *system_id == rest.system_id)
        .map(|(_, beam)| *beam)
        .collect::<Vec<_>>();
    let pre_rest_grouping_input = beams
        .raw_beams
        .iter()
        .chain(&beams.hooks)
        .filter(|(system_id, _)| *system_id == rest.system_id)
        .map(|(_, beam)| *beam)
        .collect::<Vec<_>>();
    let retained = beams
        .group_memberships
        .iter()
        .find(|membership| membership.system_id == rest.system_id)
        .expect("Bach system 6 beam groups");
    let post_rest_groups = audiveris_omr::beam_inters::group_beams(
        &post_rest_grouping_input,
        grid.scale.scale.interline.main,
    );
    let pre_rest_groups = audiveris_omr::beam_inters::group_beams(
        &pre_rest_grouping_input,
        grid.scale.scale.interline.main,
    );
    assert_eq!(retained.groups, pre_rest_groups.groups);
    assert_ne!(retained.groups, post_rest_groups.groups);
}
