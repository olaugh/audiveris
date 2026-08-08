// SPDX-License-Identifier: AGPL-3.0-or-later

#[path = "../src/head_small_beam_purge.rs"]
mod head_small_beam_purge;

mod head_scanner_slices {
    pub use audiveris_omr::head_scanner_slices::JavaRectangle;
}

mod head_template_overlap {
    pub use audiveris_omr::head_template_overlap::horizontal_parallelogram_intersects_rectangle;
}

use audiveris_image::beam_structure::Segment;
use head_scanner_slices::JavaRectangle;
use head_small_beam_purge::{
    HeadBeamShape, SmallBeamPurgeBeam, SmallBeamPurgeHead, SmallBeamPurgeTarget, purge_small_beams,
    purge_small_beams_with_contextual_grades,
};

fn rect(x: i32, y: i32, width: i32, height: i32) -> JavaRectangle {
    JavaRectangle::new(x, y, width, height)
}

fn head(x: i32, y: i32, width: i32, height: i32, grade: f64) -> SmallBeamPurgeHead {
    SmallBeamPurgeHead {
        bounds: rect(x, y, width, height),
        grade,
        removed: false,
    }
}

fn beam(
    shape: HeadBeamShape,
    bounds: JavaRectangle,
    median: Segment,
    height: f64,
    contextual_grade: f64,
) -> SmallBeamPurgeBeam {
    SmallBeamPurgeBeam {
        shape,
        bounds,
        median,
        height,
        contextual_grade,
        removed: false,
    }
}

fn segment(x1: f64, y1: f64, x2: f64, y2: f64) -> Segment {
    Segment { x1, y1, x2, y2 }
}

#[test]
fn filters_all_four_beam_shapes_with_strict_width_and_keeps_sig_order() {
    let beams = [
        beam(
            HeadBeamShape::BeamHookSmall,
            rect(0, 0, 9, 2),
            segment(0.0, 1.0, 9.0, 1.0),
            2.0,
            2.0,
        ),
        beam(
            HeadBeamShape::Other,
            rect(0, 0, 1, 2),
            segment(0.0, 1.0, 1.0, 1.0),
            2.0,
            2.0,
        ),
        beam(
            HeadBeamShape::Beam,
            rect(0, 0, 10, 2),
            segment(0.0, 1.0, 10.0, 1.0),
            2.0,
            2.0,
        ),
        beam(
            HeadBeamShape::BeamSmall,
            rect(20, 0, 8, 2),
            segment(20.0, 1.0, 28.0, 1.0),
            2.0,
            2.0,
        ),
        beam(
            HeadBeamShape::BeamHook,
            rect(40, 0, -1, 2),
            segment(40.0, 1.0, 41.0, 1.0),
            2.0,
            2.0,
        ),
    ];

    let result = purge_small_beams(&[], &beams, 10);

    assert_eq!(result.candidate_beam_indices, [0, 3, 4]);
    assert_eq!(result.remaining_beam_indices, [0, 3, 4]);
}

#[test]
fn stable_ordinate_order_breaks_ties_in_original_sig_order() {
    let heads = [
        head(20, 5, 2, 2, 0.1),
        head(20, -3, 2, 2, 0.1),
        head(20, 5, 2, 2, 0.1),
        head(20, 1, 2, 2, 0.1),
    ];

    let result = purge_small_beams(&heads, &[], 10);

    assert_eq!(result.ordered_head_indices, [1, 3, 0, 2]);
    assert_eq!(result.remaining_head_indices, [1, 3, 0, 2]);
}

#[test]
fn stronger_head_removes_beam_and_stops_its_scan() {
    let heads = [head(0, 0, 2, 2, 0.7), head(3, 0, 2, 2, 0.9)];
    let beams = [beam(
        HeadBeamShape::BeamHook,
        rect(0, 0, 5, 2),
        segment(0.0, 1.0, 5.0, 1.0),
        2.0,
        0.6,
    )];

    let result = purge_small_beams(&heads, &beams, 10);

    assert_eq!(result.checks.len(), 1);
    assert_eq!(result.decisions.len(), 1);
    assert_eq!(result.decisions[0].target, SmallBeamPurgeTarget::Beam);
    assert_eq!(result.beam_removed, [true]);
    assert_eq!(result.head_removed, [false, false]);
    assert!(result.remaining_beam_indices.is_empty());
}

#[test]
fn equal_and_unordered_grades_take_java_else_branch_and_remove_head() {
    for (head_grade, beam_grade) in [
        (0.5, 0.5),
        (f64::NAN, 0.5),
        (0.5, f64::NAN),
        (f64::NAN, f64::NAN),
    ] {
        let heads = [head(0, 0, 2, 2, head_grade)];
        let beams = [beam(
            HeadBeamShape::BeamSmall,
            rect(0, 0, 4, 2),
            segment(0.0, 1.0, 4.0, 1.0),
            2.0,
            beam_grade,
        )];

        let result = purge_small_beams(&heads, &beams, 10);

        assert_eq!(result.decisions[0].target, SmallBeamPurgeTarget::Head);
        assert_eq!(result.beam_removed, [false]);
        assert_eq!(result.head_removed, [true]);
    }
}

#[test]
fn one_beam_removes_weak_heads_until_a_stronger_head_removes_the_beam() {
    let heads = [
        head(0, 0, 2, 2, 0.1),
        head(2, 0, 2, 2, 0.4),
        head(4, 0, 2, 2, 0.9),
        head(6, 0, 2, 2, 0.1),
    ];
    let beams = [beam(
        HeadBeamShape::Beam,
        rect(0, 0, 8, 2),
        segment(0.0, 1.0, 8.0, 1.0),
        2.0,
        0.5,
    )];

    let result = purge_small_beams(&heads, &beams, 10);

    assert_eq!(
        result
            .decisions
            .iter()
            .map(|decision| decision.target)
            .collect::<Vec<_>>(),
        [
            SmallBeamPurgeTarget::Head,
            SmallBeamPurgeTarget::Head,
            SmallBeamPurgeTarget::Beam,
        ],
    );
    assert_eq!(result.head_removed, [true, true, false, false]);
    assert_eq!(result.remaining_head_indices, [2, 3]);
    assert_eq!(result.beam_removed, [true]);
}

#[test]
fn heads_removed_by_an_earlier_beam_are_invisible_to_later_beams() {
    let heads = [head(0, 0, 2, 2, 0.1)];
    let beams = [
        beam(
            HeadBeamShape::Beam,
            rect(0, 0, 4, 2),
            segment(0.0, 1.0, 4.0, 1.0),
            2.0,
            0.5,
        ),
        beam(
            HeadBeamShape::BeamSmall,
            rect(0, 0, 4, 2),
            segment(0.0, 1.0, 4.0, 1.0),
            2.0,
            0.5,
        ),
    ];

    let result = purge_small_beams(&heads, &beams, 10);

    assert_eq!(result.checks.len(), 1);
    assert_eq!(result.decisions.len(), 1);
    assert_eq!(result.remaining_beam_indices, [0, 1]);
    assert_eq!(result.head_removed, [true]);
}

#[test]
fn uses_filled_parallelogram_not_just_overlapping_bounds() {
    // The parallelogram's lower-right corner only touches the rectangle's
    // upper-left corner. Its integer bounds overlap, but Java Area does not.
    let heads = [head(0, 0, 2, 2, 1.0)];
    let beams = [beam(
        HeadBeamShape::BeamHook,
        rect(-2, -3, 2, 3),
        segment(-2.0, -2.0, 0.0, -1.0),
        2.0,
        0.0,
    )];

    let result = purge_small_beams(&heads, &beams, 10);

    assert_eq!(result.checks.len(), 1);
    assert!(!result.checks[0].intersects);
    assert!(result.checks[0].breaks);
    assert!(result.decisions.is_empty());
}

#[test]
fn vertical_break_uses_inclusive_bottom_with_java_integer_wrapping() {
    let heads = [head(100, i32::MIN, 2, 2, 1.0), head(100, 0, 2, 2, 1.0)];
    let beams = [beam(
        HeadBeamShape::Beam,
        rect(0, i32::MAX, 5, 2),
        segment(0.0, f64::from(i32::MAX), 5.0, f64::from(i32::MAX)),
        2.0,
        0.0,
    )];

    let result = purge_small_beams(&heads, &beams, 10);

    // MAX + 2 - 1 wraps to MIN, so y == MIN does not break, then y == 0 does.
    assert_eq!(result.checks.len(), 2);
    assert!(!result.checks[0].breaks);
    assert!(result.checks[1].breaks);
}

#[test]
fn initially_removed_vertices_are_omitted_from_both_live_queries() {
    let mut heads = [head(0, 0, 2, 2, 1.0), head(0, 0, 2, 2, 1.0)];
    heads[0].removed = true;
    let mut beams = [beam(
        HeadBeamShape::Beam,
        rect(0, 0, 4, 2),
        segment(0.0, 1.0, 4.0, 1.0),
        2.0,
        0.0,
    )];
    beams[0].removed = true;

    let result = purge_small_beams(&heads, &beams, 10);

    assert_eq!(result.ordered_head_indices, [1]);
    assert!(result.candidate_beam_indices.is_empty());
    assert_eq!(result.head_removed, [true, false]);
    assert_eq!(result.beam_removed, [true]);
    assert!(result.checks.is_empty());
}

#[test]
fn dynamic_context_sees_beams_removed_by_an_earlier_scan() {
    let heads = [head(0, 0, 2, 2, 0.9), head(10, 0, 2, 2, 0.6)];
    let beams = [
        beam(
            HeadBeamShape::Beam,
            rect(0, 0, 4, 2),
            segment(0.0, 1.0, 4.0, 1.0),
            2.0,
            0.0,
        ),
        beam(
            HeadBeamShape::Beam,
            rect(10, 0, 4, 2),
            segment(10.0, 1.0, 14.0, 1.0),
            2.0,
            0.0,
        ),
    ];

    let result =
        purge_small_beams_with_contextual_grades(&heads, &beams, 10, |beam_index, removed, _| {
            match beam_index {
                0 => 0.5,
                1 if removed[0] => 0.4,
                1 => 0.8,
                _ => unreachable!(),
            }
        });

    // Beam 0 loses to head 0. Beam 1 then loses its support from beam 0 and
    // drops below head 1; a precomputed 0.8 would instead purge that head.
    assert_eq!(result.beam_removed, [true, true]);
    assert_eq!(result.head_removed, [false, false]);
    assert_eq!(result.computed_contextual_grades, [Some(0.5), Some(0.4)]);
}
