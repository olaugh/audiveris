// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact bounded-area geometry used before Java `NoteHeadsBuilder` looks up
//! head templates.
//!
//! A scanner line creates a semantic band by appending its spline translated
//! by `above`, then the reversed spline translated by `below + 1`. Frozen
//! barlines and connectors are selected with
//! `scannerArea.intersects(verticalArea.getBounds())`. This module models that
//! deliberately narrow contract: an x-monotone staff/ledger band, the integer
//! bounds of a straight vertical ribbon, and Java's positive-area, half-open
//! rectangle intersections. It does not approximate a spline with a polyline
//! and is not a general polygon Boolean engine.

use audiveris_image::{
    beam_structure::Segment,
    system_population::{BoundarySegment, StaffBoundary},
};

/// Java `java.awt.Rectangle` coordinates and dimensions.
///
/// Width and height remain signed because Java permits empty rectangles with
/// zero or negative dimensions. Intersection requires positive area.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct JavaRectangle {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl JavaRectangle {
    #[must_use]
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Java `Rectangle.isEmpty()`.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    /// Java `Rectangle.intersects(Rectangle)`, including its overflow rules.
    ///
    /// The right and bottom edges are excluded: edge contact alone is not an
    /// intersection.
    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        if self.is_empty() || other.is_empty() {
            return false;
        }

        let self_right = self.x.wrapping_add(self.width);
        let self_bottom = self.y.wrapping_add(self.height);
        let other_right = other.x.wrapping_add(other.width);
        let other_bottom = other.y.wrapping_add(other.height);

        (other_right < other.x || other_right > self.x)
            && (other_bottom < other.y || other_bottom > self.y)
            && (self_right < self.x || self_right > other.x)
            && (self_bottom < self.y || self_bottom > other.y)
    }

    fn right_as_f64(self) -> f64 {
        f64::from(self.x) + f64::from(self.width)
    }

    fn bottom_as_f64(self) -> f64 {
        f64::from(self.y) + f64::from(self.height)
    }
}

/// The `Area` built around a scanner staff line or ledger center-line.
#[derive(Clone, Debug, PartialEq)]
pub struct HeadScannerBand {
    axis: Vec<BoundarySegment>,
    above: f64,
    effective_bottom: f64,
}

impl HeadScannerBand {
    /// Java `StaffLineAdapter.getArea(above, below)`.
    #[must_use]
    pub fn from_staff_boundary(boundary: &StaffBoundary, above: f64, below: f64) -> Self {
        Self {
            axis: boundary.segments.clone(),
            above,
            effective_bottom: below + 1.0,
        }
    }

    /// Java `LedgerAdapter.getArea(above, below)`.
    #[must_use]
    pub fn from_ledger_axis(axis: Segment, above: f64, below: f64) -> Self {
        Self {
            axis: vec![BoundarySegment::Line {
                start: (axis.x1, axis.y1),
                end: (axis.x2, axis.y2),
            }],
            above,
            effective_bottom: below + 1.0,
        }
    }

    #[must_use]
    pub const fn above(&self) -> f64 {
        self.above
    }

    /// The lower path translation, preserving Java's explicit `below + 1`.
    #[must_use]
    pub const fn effective_bottom(&self) -> f64 {
        self.effective_bottom
    }

    /// Whether the two translated paths enclose no positive area.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        if self.axis.is_empty() || self.above == self.effective_bottom {
            return true;
        }
        let Some((minimum_x, _, maximum_x, _)) = curve_chain_bounds(&self.axis) else {
            return true;
        };
        maximum_x <= minimum_x
    }

    /// Java `Area.getBounds()` for this semantic band.
    #[must_use]
    pub fn integer_bounds(&self) -> JavaRectangle {
        if self.is_empty() {
            return JavaRectangle::default();
        }
        let Some((min_x, min_y, max_x, max_y)) = curve_chain_bounds(&self.axis) else {
            return JavaRectangle::default();
        };
        let upper_offset = self.above.min(self.effective_bottom);
        let lower_offset = self.above.max(self.effective_bottom);
        enclosing_integer_bounds(min_x, min_y + upper_offset, max_x, max_y + lower_offset)
    }

    /// Java `Area.intersects(Rectangle)` for the supported x-monotone band.
    ///
    /// Both inputs are interpreted as areas. A shared boundary, including a
    /// tangent contact with the spline, is therefore not enough.
    #[must_use]
    pub fn intersects_rectangle(&self, rectangle: JavaRectangle) -> bool {
        if rectangle.is_empty() || self.is_empty() {
            return false;
        }

        let rectangle_left = f64::from(rectangle.x);
        let rectangle_right = rectangle.right_as_f64();
        let rectangle_top = f64::from(rectangle.y);
        let rectangle_bottom = rectangle.bottom_as_f64();
        let upper_offset = self.above.min(self.effective_bottom);
        let lower_offset = self.above.max(self.effective_bottom);

        // The strip and rectangle have overlapping interiors at x exactly when
        // the axis ordinate lies in this open interval.
        let required_y_min = rectangle_top - lower_offset;
        let required_y_max = rectangle_bottom - upper_offset;

        self.axis.iter().copied().any(|segment| {
            let (start, end) = segment_endpoints(segment);
            let segment_left = start.0.min(end.0);
            let segment_right = start.0.max(end.0);
            let clipped_left = segment_left.max(rectangle_left);
            let clipped_right = segment_right.min(rectangle_right);
            if clipped_right <= clipped_left {
                return false;
            }

            let mut start_t = parameter_at_x(segment, clipped_left);
            let mut end_t = parameter_at_x(segment, clipped_right);
            if start_t > end_t {
                std::mem::swap(&mut start_t, &mut end_t);
            }
            let (minimum_y, maximum_y) = segment_y_range(segment, start_t, end_t);
            minimum_y < required_y_max && maximum_y > required_y_min
        })
    }

    /// Java `Scanner.getBarAreas`: intersect this band with each vertical
    /// area's integer bounds and retain input/source order.
    #[must_use]
    pub fn intersecting_vertical_area_indices(&self, areas: &[VerticalRibbonArea]) -> Vec<usize> {
        areas
            .iter()
            .enumerate()
            .filter_map(|(index, area)| {
                self.intersects_rectangle(area.integer_bounds())
                    .then_some(index)
            })
            .collect()
    }
}

/// Java `AreaUtil.verticalRibbon` for a straight barline/connector median.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VerticalRibbonArea {
    pub median: Segment,
    pub thickness: f64,
}

impl VerticalRibbonArea {
    #[must_use]
    pub const fn new(median: Segment, thickness: f64) -> Self {
        Self { median, thickness }
    }

    /// Java `new Area(verticalRibbonPath).getBounds()`.
    #[must_use]
    pub fn integer_bounds(self) -> JavaRectangle {
        if ![
            self.median.x1,
            self.median.y1,
            self.median.x2,
            self.median.y2,
            self.thickness,
        ]
        .into_iter()
        .all(f64::is_finite)
            || self.thickness == 0.0
            || self.median.y1 == self.median.y2
        {
            return JavaRectangle::default();
        }

        let half_width = self.thickness.abs() / 2.0;
        enclosing_integer_bounds(
            self.median.x1.min(self.median.x2) - half_width,
            self.median.y1.min(self.median.y2),
            self.median.x1.max(self.median.x2) + half_width,
            self.median.y1.max(self.median.y2),
        )
    }
}

fn enclosing_integer_bounds(
    minimum_x: f64,
    minimum_y: f64,
    maximum_x: f64,
    maximum_y: f64,
) -> JavaRectangle {
    let x = minimum_x.floor();
    let y = minimum_y.floor();
    let right = maximum_x.ceil();
    let bottom = maximum_y.ceil();
    JavaRectangle::new(x as i32, y as i32, (right - x) as i32, (bottom - y) as i32)
}

fn curve_chain_bounds(axis: &[BoundarySegment]) -> Option<(f64, f64, f64, f64)> {
    let first = axis.first().copied()?;
    let (start, _) = segment_endpoints(first);
    let mut minimum_x = start.0;
    let mut maximum_x = start.0;
    let mut minimum_y = start.1;
    let mut maximum_y = start.1;
    for segment in axis.iter().copied() {
        include_segment_extrema(
            segment,
            &mut minimum_x,
            &mut minimum_y,
            &mut maximum_x,
            &mut maximum_y,
        );
    }
    Some((minimum_x, minimum_y, maximum_x, maximum_y))
}

fn include_segment_extrema(
    segment: BoundarySegment,
    minimum_x: &mut f64,
    minimum_y: &mut f64,
    maximum_x: &mut f64,
    maximum_y: &mut f64,
) {
    let mut include_t = |t: f64| {
        let point = point_at(segment, t);
        *minimum_x = minimum_x.min(point.0);
        *maximum_x = maximum_x.max(point.0);
        *minimum_y = minimum_y.min(point.1);
        *maximum_y = maximum_y.max(point.1);
    };
    include_t(0.0);
    include_t(1.0);
    for parameter in segment_extrema_parameters(segment, true)
        .into_iter()
        .chain(segment_extrema_parameters(segment, false))
    {
        if parameter > 0.0 && parameter < 1.0 {
            include_t(parameter);
        }
    }
}

fn segment_y_range(segment: BoundarySegment, start_t: f64, end_t: f64) -> (f64, f64) {
    let start_y = point_at(segment, start_t).1;
    let end_y = point_at(segment, end_t).1;
    let mut minimum_y = start_y.min(end_y);
    let mut maximum_y = start_y.max(end_y);
    for parameter in segment_extrema_parameters(segment, false) {
        if parameter > start_t && parameter < end_t {
            let y = point_at(segment, parameter).1;
            minimum_y = minimum_y.min(y);
            maximum_y = maximum_y.max(y);
        }
    }
    (minimum_y, maximum_y)
}

fn segment_extrema_parameters(segment: BoundarySegment, horizontal: bool) -> Vec<f64> {
    let coordinate = |point: (f64, f64)| if horizontal { point.0 } else { point.1 };
    match segment {
        BoundarySegment::Line { .. } => Vec::new(),
        BoundarySegment::Quadratic {
            start,
            control,
            end,
        } => {
            let start = coordinate(start);
            let control = coordinate(control);
            let end = coordinate(end);
            let denominator = start - control - control + end;
            if denominator == 0.0 {
                Vec::new()
            } else {
                vec![(start - control) / denominator]
            }
        }
        BoundarySegment::Cubic {
            start,
            control1,
            control2,
            end,
        } => {
            let start = coordinate(start);
            let control1 = coordinate(control1);
            let control2 = coordinate(control2);
            let end = coordinate(end);
            // B'(t)/3 = a*t^2 + b*t + c.
            let a = end - control2 + control1 - control2 + control1 - control2 + control1 - start;
            let b = 2.0 * (control2 - control1 - control1 + start);
            let c = control1 - start;
            solve_quadratic(c, b, a)
        }
    }
}

/// The stable quadratic formula used by Java `QuadCurve2D.solveQuadratic`.
fn solve_quadratic(c: f64, b: f64, a: f64) -> Vec<f64> {
    if a == 0.0 {
        return if b == 0.0 { Vec::new() } else { vec![-c / b] };
    }

    let mut discriminant = (b * b) - (4.0 * a * c);
    if discriminant < 0.0 {
        return Vec::new();
    }
    discriminant = discriminant.sqrt();
    if b < 0.0 {
        discriminant = -discriminant;
    }
    let q = (b + discriminant) / -2.0;
    let mut roots = vec![q / a];
    if q != 0.0 {
        roots.push(c / q);
    }
    roots
}

fn parameter_at_x(segment: BoundarySegment, x: f64) -> f64 {
    let (start, end) = segment_endpoints(segment);
    if x == start.0 {
        return 0.0;
    }
    if x == end.0 {
        return 1.0;
    }

    let increasing = start.0 < end.0;
    let mut low = 0.0;
    let mut high = 1.0;
    // A double has 52 explicit/significand fraction bits. This resolves the
    // parameter as far as repeated subdivision can distinguish it.
    for _ in 0..=52 {
        let middle = (low + high) / 2.0;
        let middle_x = point_at(segment, middle).0;
        if (middle_x < x) == increasing {
            low = middle;
        } else {
            high = middle;
        }
    }
    (low + high) / 2.0
}

fn point_at(segment: BoundarySegment, t: f64) -> (f64, f64) {
    let one_minus_t = 1.0 - t;
    match segment {
        BoundarySegment::Line { start, end } => (
            (one_minus_t * start.0) + (t * end.0),
            (one_minus_t * start.1) + (t * end.1),
        ),
        BoundarySegment::Quadratic {
            start,
            control,
            end,
        } => (
            (one_minus_t * one_minus_t * start.0)
                + (2.0 * one_minus_t * t * control.0)
                + (t * t * end.0),
            (one_minus_t * one_minus_t * start.1)
                + (2.0 * one_minus_t * t * control.1)
                + (t * t * end.1),
        ),
        BoundarySegment::Cubic {
            start,
            control1,
            control2,
            end,
        } => (
            (one_minus_t * one_minus_t * one_minus_t * start.0)
                + (3.0 * one_minus_t * one_minus_t * t * control1.0)
                + (3.0 * one_minus_t * t * t * control2.0)
                + (t * t * t * end.0),
            (one_minus_t * one_minus_t * one_minus_t * start.1)
                + (3.0 * one_minus_t * one_minus_t * t * control1.1)
                + (3.0 * one_minus_t * t * t * control2.1)
                + (t * t * t * end.1),
        ),
    }
}

const fn segment_endpoints(segment: BoundarySegment) -> ((f64, f64), (f64, f64)) {
    match segment {
        BoundarySegment::Line { start, end }
        | BoundarySegment::Quadratic { start, end, .. }
        | BoundarySegment::Cubic { start, end, .. } => (start, end),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(x1: f64, y1: f64, x2: f64, y2: f64) -> Segment {
        Segment { x1, y1, x2, y2 }
    }

    #[test]
    fn rectangle_intersection_is_positive_area_and_half_open() {
        let rectangle = JavaRectangle::new(10, 20, 5, 7);
        assert!(rectangle.intersects(JavaRectangle::new(14, 26, 3, 3)));
        assert!(!rectangle.intersects(JavaRectangle::new(15, 20, 3, 3)));
        assert!(!rectangle.intersects(JavaRectangle::new(10, 27, 3, 3)));
        assert!(!rectangle.intersects(JavaRectangle::new(12, 22, 0, 3)));
        assert!(!rectangle.intersects(JavaRectangle::new(12, 22, 3, -1)));
    }

    #[test]
    fn rectangle_intersection_preserves_java_integer_overflow_rule() {
        let overflowing = JavaRectangle::new(i32::MAX - 1, 10, 5, 5);
        assert!(overflowing.intersects(JavaRectangle::new(i32::MAX, 11, 1, 1)));
        assert!(!overflowing.intersects(JavaRectangle::new(i32::MIN, 11, 1, 1)));
    }

    #[test]
    fn vertical_ribbon_bounds_match_live_barline_and_connector_oracle() {
        let barline = VerticalRibbonArea::new(segment(201.5, 349.0, 201.5, 438.0), 3.0);
        assert_eq!(
            barline.integer_bounds(),
            JavaRectangle::new(200, 349, 3, 89)
        );

        let connector = VerticalRibbonArea::new(segment(201.5, 438.0, 200.5, 591.0), 3.0);
        assert_eq!(
            connector.integer_bounds(),
            JavaRectangle::new(199, 438, 4, 153)
        );
    }

    #[test]
    fn staff_band_bounds_match_live_semantic_slice_oracle() {
        let boundary = StaffBoundary {
            segments: vec![BoundarySegment::Line {
                start: (
                    f64::from_bits(0x4069_4000_0000_0000),
                    f64::from_bits(0x4075_efcd_be09_6c5e),
                ),
                end: (
                    f64::from_bits(0x40a2_2c00_0000_0000),
                    f64::from_bits(0x4076_c04b_62f1_dd73),
                ),
            }],
        };
        let band = HeadScannerBand::from_staff_boundary(&boundary, -15.75, -5.25);
        assert_eq!(band.effective_bottom().to_bits(), (-4.25_f64).to_bits());
        assert_eq!(
            band.integer_bounds(),
            JavaRectangle::new(202, 335, 2124, 25)
        );
    }

    #[test]
    fn band_bounds_include_curve_extrema_without_flattening() {
        let boundary = StaffBoundary {
            segments: vec![BoundarySegment::Quadratic {
                start: (0.0, 10.0),
                control: (5.0, 0.0),
                end: (10.0, 10.0),
            }],
        };
        let band = HeadScannerBand::from_staff_boundary(&boundary, -2.0, 2.0);
        assert_eq!(band.integer_bounds(), JavaRectangle::new(0, 3, 10, 10));
    }

    #[test]
    fn collapsed_paths_are_java_empty_areas_at_the_origin() {
        let zero_band = HeadScannerBand::from_ledger_axis(segment(7.0, 10.0, 20.0, 10.0), 2.0, 1.0);
        assert!(zero_band.is_empty());
        assert_eq!(zero_band.integer_bounds(), JavaRectangle::default());
        assert!(!zero_band.intersects_rectangle(JavaRectangle::new(7, 9, 13, 3)));

        let horizontal_ribbon = VerticalRibbonArea::new(segment(7.0, 10.0, 20.0, 10.0), 3.0);
        assert_eq!(horizontal_ribbon.integer_bounds(), JavaRectangle::default());
    }

    #[test]
    fn area_rectangle_intersection_rejects_edge_and_tangent_contact() {
        let band = HeadScannerBand::from_ledger_axis(segment(0.0, 10.0, 20.0, 10.0), -2.0, 1.0);
        assert!(band.intersects_rectangle(JavaRectangle::new(4, 9, 3, 2)));
        assert!(!band.intersects_rectangle(JavaRectangle::new(4, 4, 3, 4)));
        assert!(!band.intersects_rectangle(JavaRectangle::new(4, 12, 3, 2)));
        assert!(!band.intersects_rectangle(JavaRectangle::new(20, 9, 3, 2)));
    }

    #[test]
    fn curved_band_intersects_where_endpoint_chord_does_not() {
        let boundary = StaffBoundary {
            segments: vec![BoundarySegment::Quadratic {
                start: (0.0, 10.0),
                control: (5.0, 0.0),
                end: (10.0, 10.0),
            }],
        };
        let band = HeadScannerBand::from_staff_boundary(&boundary, -1.0, 0.0);
        assert!(band.intersects_rectangle(JavaRectangle::new(4, 4, 2, 2)));
        assert!(!band.intersects_rectangle(JavaRectangle::new(4, 2, 2, 2)));
    }

    #[test]
    fn frozen_area_slice_uses_integer_bounds_and_preserves_source_order() {
        let band = HeadScannerBand::from_ledger_axis(segment(0.0, 10.0, 20.0, 10.0), -2.0, 1.0);
        let areas = [
            VerticalRibbonArea::new(segment(2.5, 8.0, 2.5, 12.0), 1.0),
            VerticalRibbonArea::new(segment(7.5, 30.0, 7.5, 40.0), 1.0),
            VerticalRibbonArea::new(segment(12.5, 7.0, 12.5, 8.5), 1.0),
        ];
        assert_eq!(band.intersecting_vertical_area_indices(&areas), vec![0, 2]);
    }
}
