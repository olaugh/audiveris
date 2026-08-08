// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact area-versus-slim-box overlap used by Java `NoteHeadsBuilder.overlap`.
//!
//! The current HEADS competitor pool contains only straight
//! `AreaUtil.verticalRibbon` and `AreaUtil.horizontalParallelogram` areas. Both
//! are convex polygons, so Java2D `Area.intersects(Rectangle2D)` reduces to a
//! strict separating-axis test on the vertices Java puts in each path. Strict
//! interval overlap is important: a shared edge or corner has no area and does
//! not intersect. This module deliberately does not model scanner ordering or
//! the bounds-only branch used for area-less `Inter` implementations.

use audiveris_image::beam_structure::Segment;

use crate::head_scanner_slices::JavaRectangle;

/// Java2D `Area.intersects(Rectangle2D)` for a straight
/// `AreaUtil.verticalRibbon`.
///
/// Negative widths reverse the path but describe the same filled region.
/// Zero-width, horizontal, rounding-collapsed, or non-finite ribbons contain
/// no usable positive area.
#[must_use]
pub fn vertical_ribbon_intersects_rectangle(
    median: Segment,
    width: f64,
    rectangle: JavaRectangle,
) -> bool {
    // Match AreaUtil.verticalRibbon: compute dx first, negate the translation
    // used by the forward copy, and then add each translation to the source x.
    let dx = width / 2.0;
    let negative_dx = -dx;
    let polygon = [
        (median.x1 + negative_dx, median.y1),
        (median.x2 + negative_dx, median.y2),
        (median.x2 + dx, median.y2),
        (median.x1 + dx, median.y1),
    ];

    let positive_area =
        median.y1 != median.y2 && (polygon[0].0 != polygon[3].0 || polygon[1].0 != polygon[2].0);
    positive_area && convex_polygon_intersects_rectangle(&polygon, rectangle)
}

/// Java2D `Area.intersects(Rectangle2D)` for
/// `AreaUtil.horizontalParallelogram`.
///
/// Negative heights reverse the path but describe the same filled region.
/// Zero-height, vertical, rounding-collapsed, or non-finite parallelograms
/// contain no usable positive area.
#[must_use]
pub fn horizontal_parallelogram_intersects_rectangle(
    median: Segment,
    height: f64,
    rectangle: JavaRectangle,
) -> bool {
    // Preserve AreaUtil.horizontalParallelogramPath's operation order.
    let dy = height / 2.0;
    let polygon = [
        (median.x1, median.y1 - dy),
        (median.x2, median.y2 - dy),
        (median.x2, median.y2 + dy),
        (median.x1, median.y1 + dy),
    ];

    let positive_area =
        median.x1 != median.x2 && (polygon[0].1 != polygon[3].1 || polygon[1].1 != polygon[2].1);
    positive_area && convex_polygon_intersects_rectangle(&polygon, rectangle)
}

/// Positive-area intersection of a convex quadrilateral and an integer Java
/// rectangle. Degenerate polygon edges are skipped so a rounding-collapsed end
/// can still leave the triangle Java2D constructs at the other end.
fn convex_polygon_intersects_rectangle(
    polygon: &[(f64, f64); 4],
    rectangle: JavaRectangle,
) -> bool {
    if rectangle.is_empty()
        || !polygon
            .iter()
            .flat_map(|&(x, y)| [x, y])
            .all(f64::is_finite)
    {
        return false;
    }

    // Rectangle2D receives doubles from java.awt.Rectangle. Its maximum
    // coordinates are double additions, not wrapping int additions.
    let left = f64::from(rectangle.x);
    let top = f64::from(rectangle.y);
    let right = left + f64::from(rectangle.width);
    let bottom = top + f64::from(rectangle.height);
    let rectangle_points = [(left, top), (right, top), (right, bottom), (left, bottom)];

    if has_separating_axis(polygon, &rectangle_points, (1.0, 0.0))
        || has_separating_axis(polygon, &rectangle_points, (0.0, 1.0))
    {
        return false;
    }

    for index in 0..polygon.len() {
        let start = polygon[index];
        let stop = polygon[(index + 1) % polygon.len()];
        let axis = (-(stop.1 - start.1), stop.0 - start.0);

        // Area drops zero-length path segments. Their zero normal is not a
        // separating axis, particularly when one end collapsed in rounding.
        if axis == (0.0, 0.0) {
            continue;
        }
        if has_separating_axis(polygon, &rectangle_points, axis) {
            return false;
        }
    }

    true
}

fn has_separating_axis(
    polygon: &[(f64, f64); 4],
    rectangle: &[(f64, f64); 4],
    axis: (f64, f64),
) -> bool {
    let Some((polygon_minimum, polygon_maximum)) = projection_range(polygon, axis) else {
        return true;
    };
    let Some((rectangle_minimum, rectangle_maximum)) = projection_range(rectangle, axis) else {
        return true;
    };

    // Equality is separation: Java Area.intersects requires positive area.
    polygon_maximum <= rectangle_minimum || rectangle_maximum <= polygon_minimum
}

fn projection_range(points: &[(f64, f64); 4], axis: (f64, f64)) -> Option<(f64, f64)> {
    let project = |point: (f64, f64)| (point.0 * axis.0) + (point.1 * axis.1);
    let first = project(points[0]);
    if first.is_nan() {
        return None;
    }

    let mut minimum = first;
    let mut maximum = first;
    for &point in &points[1..] {
        let projection = project(point);
        if projection.is_nan() {
            return None;
        }
        minimum = minimum.min(projection);
        maximum = maximum.max(projection);
    }
    Some((minimum, maximum))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(x1: f64, y1: f64, x2: f64, y2: f64) -> Segment {
        Segment { x1, y1, x2, y2 }
    }

    #[test]
    fn horizontal_parallelogram_and_rectangle_containment_both_intersect() {
        let area = segment(2.0, 5.0, 8.0, 5.0);
        assert!(horizontal_parallelogram_intersects_rectangle(
            area,
            4.0,
            JavaRectangle::new(0, 0, 10, 10),
        ));

        let area = segment(-10.0, 5.0, 20.0, 5.0);
        assert!(horizontal_parallelogram_intersects_rectangle(
            area,
            20.0,
            JavaRectangle::new(2, 3, 4, 4),
        ));
    }

    #[test]
    fn vertical_ribbon_and_rectangle_containment_both_intersect() {
        let area = segment(5.0, 2.0, 5.0, 8.0);
        assert!(vertical_ribbon_intersects_rectangle(
            area,
            4.0,
            JavaRectangle::new(0, 0, 10, 10),
        ));

        let area = segment(5.0, -10.0, 5.0, 20.0);
        assert!(vertical_ribbon_intersects_rectangle(
            area,
            20.0,
            JavaRectangle::new(2, 3, 4, 4),
        ));
    }

    #[test]
    fn shared_edges_and_corners_do_not_have_positive_area() {
        let horizontal = segment(0.0, 0.0, 10.0, 0.0);
        assert!(!horizontal_parallelogram_intersects_rectangle(
            horizontal,
            2.0,
            JavaRectangle::new(10, -1, 4, 2),
        ));
        assert!(!horizontal_parallelogram_intersects_rectangle(
            horizontal,
            2.0,
            JavaRectangle::new(10, 1, 4, 2),
        ));

        let vertical = segment(0.0, 0.0, 0.0, 10.0);
        assert!(!vertical_ribbon_intersects_rectangle(
            vertical,
            2.0,
            JavaRectangle::new(1, 10, 3, 3),
        ));
        assert!(!vertical_ribbon_intersects_rectangle(
            vertical,
            2.0,
            JavaRectangle::new(1, 4, 3, 2),
        ));
    }

    #[test]
    fn sloped_areas_cross_rectangle_without_containing_its_corners() {
        let rising = segment(-10.0, -10.0, 10.0, 10.0);
        assert!(horizontal_parallelogram_intersects_rectangle(
            rising,
            0.5,
            JavaRectangle::new(-1, -1, 2, 2),
        ));

        let falling = segment(-10.0, 10.0, 10.0, -10.0);
        assert!(horizontal_parallelogram_intersects_rectangle(
            falling,
            0.5,
            JavaRectangle::new(-1, -1, 2, 2),
        ));

        let diagonal_ribbon = segment(-2.0, -10.0, 2.0, 10.0);
        assert!(vertical_ribbon_intersects_rectangle(
            diagonal_ribbon,
            0.5,
            JavaRectangle::new(-1, -1, 2, 2),
        ));
    }

    #[test]
    fn area_that_only_reaches_a_sloped_corner_does_not_intersect() {
        // The parallelogram's lower-right corner is exactly the rectangle's
        // upper-left corner; all remaining area lies above or to the left.
        assert!(!horizontal_parallelogram_intersects_rectangle(
            segment(-2.0, -2.0, 0.0, -1.0),
            2.0,
            JavaRectangle::new(0, 0, 2, 2),
        ));

        // The ribbon's lower-right corner is exactly the same rectangle corner.
        assert!(!vertical_ribbon_intersects_rectangle(
            segment(-2.0, -2.0, -1.0, 0.0),
            2.0,
            JavaRectangle::new(0, 0, 2, 2),
        ));
    }

    #[test]
    fn negative_geometry_extent_reverses_path_but_retains_area() {
        let rectangle = JavaRectangle::new(-1, -1, 2, 2);
        let vertical = segment(0.0, -2.0, 0.0, 2.0);
        let horizontal = segment(-2.0, 0.0, 2.0, 0.0);

        assert_eq!(
            vertical_ribbon_intersects_rectangle(vertical, -2.0, rectangle),
            vertical_ribbon_intersects_rectangle(vertical, 2.0, rectangle),
        );
        assert_eq!(
            horizontal_parallelogram_intersects_rectangle(horizontal, -2.0, rectangle),
            horizontal_parallelogram_intersects_rectangle(horizontal, 2.0, rectangle),
        );
    }

    #[test]
    fn empty_rectangles_and_degenerate_areas_never_intersect() {
        let rectangle = JavaRectangle::new(-1, -1, 2, 2);
        let vertical = segment(0.0, -2.0, 0.0, 2.0);
        let horizontal = segment(-2.0, 0.0, 2.0, 0.0);

        for empty in [
            JavaRectangle::new(0, 0, 0, 2),
            JavaRectangle::new(0, 0, 2, 0),
            JavaRectangle::new(0, 0, -1, 2),
            JavaRectangle::new(0, 0, 2, -1),
        ] {
            assert!(!vertical_ribbon_intersects_rectangle(vertical, 2.0, empty));
            assert!(!horizontal_parallelogram_intersects_rectangle(
                horizontal, 2.0, empty,
            ));
        }

        assert!(!vertical_ribbon_intersects_rectangle(
            vertical, 0.0, rectangle,
        ));
        assert!(!vertical_ribbon_intersects_rectangle(
            segment(-2.0, 0.0, 2.0, 0.0),
            2.0,
            rectangle,
        ));
        assert!(!horizontal_parallelogram_intersects_rectangle(
            horizontal, 0.0, rectangle,
        ));
        assert!(!horizontal_parallelogram_intersects_rectangle(
            segment(0.0, -2.0, 0.0, 2.0),
            2.0,
            rectangle,
        ));
    }

    #[test]
    fn non_finite_or_rounding_collapsed_paths_are_empty() {
        let rectangle = JavaRectangle::new(-1, -1, 2, 2);
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(!vertical_ribbon_intersects_rectangle(
                segment(value, -2.0, 0.0, 2.0),
                2.0,
                rectangle,
            ));
            assert!(!vertical_ribbon_intersects_rectangle(
                segment(0.0, -2.0, 0.0, 2.0),
                value,
                rectangle,
            ));
            assert!(!horizontal_parallelogram_intersects_rectangle(
                segment(-2.0, value, 2.0, 0.0),
                2.0,
                rectangle,
            ));
            assert!(!horizontal_parallelogram_intersects_rectangle(
                segment(-2.0, 0.0, 2.0, 0.0),
                value,
                rectangle,
            ));
        }

        let huge = 1_u64 << 53;
        assert!(!vertical_ribbon_intersects_rectangle(
            segment(huge as f64, -2.0, huge as f64, 2.0),
            1.0,
            rectangle,
        ));
        assert!(!horizontal_parallelogram_intersects_rectangle(
            segment(-2.0, huge as f64, 2.0, huge as f64),
            1.0,
            rectangle,
        ));
    }

    #[test]
    fn integer_rectangle_maximum_uses_double_addition_without_wrapping() {
        let rectangle = JavaRectangle::new(i32::MAX - 1, 0, 4, 2);
        assert!(horizontal_parallelogram_intersects_rectangle(
            segment(f64::from(i32::MAX), 1.0, f64::from(i32::MAX) + 2.0, 1.0),
            1.0,
            rectangle,
        ));

        assert!(!horizontal_parallelogram_intersects_rectangle(
            segment(f64::from(i32::MIN), 1.0, f64::from(i32::MIN) + 1.0, 1.0),
            1.0,
            rectangle,
        ));
    }
}
