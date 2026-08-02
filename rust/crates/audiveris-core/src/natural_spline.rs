// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-free numerical subset of Audiveris `NaturalSpline`.
//!
//! This ports only point interpolation plus `yAtX` and `yDerivativeAtX`, the
//! horizontal operations required by `CurvedFilament`. The Java class's shape
//! rendering, path iteration, integer rounding overload, x-at-y operations,
//! endpoint accessors, and explicit extrapolation helpers are intentionally
//! omitted. Segment lookup and parameterization preserve `GeoPath`: the first
//! segment whose end x is greater than or equal to the query is selected, and
//! its parameter is computed linearly from the two endpoint x coordinates.

use std::error::Error;
use std::fmt;

/// A natural spline represented by the same line/quadratic/cubic segments as Java.
#[derive(Clone, Debug, PartialEq)]
pub struct NaturalSpline {
    segments: Vec<Segment>,
}

impl NaturalSpline {
    /// Interpolate two or more `(x, y)` points.
    ///
    /// Two points produce a line, three produce one quadratic, and four or
    /// more produce the natural cubic sequence used by Java. As in the source,
    /// coordinates are not reordered or checked for finiteness/monotonicity.
    pub fn interpolate(points: &[(f64, f64)]) -> Result<Self, SplineError> {
        let segment_count = points
            .len()
            .checked_sub(1)
            .ok_or(SplineError::NotEnoughPoints)?;
        if segment_count < 1 {
            return Err(SplineError::NotEnoughPoints);
        }

        let segments = match segment_count {
            1 => vec![Segment::Line {
                start: points[0],
                end: points[1],
            }],
            2 => {
                let (x0, y0) = points[0];
                let (x1, y1) = points[1];
                let (x2, y2) = points[2];
                vec![Segment::Quadratic {
                    start: points[0],
                    control: (
                        (2.0 * x1) - ((x0 + x2) / 2.0),
                        (2.0 * y1) - ((y0 + y2) / 2.0),
                    ),
                    end: points[2],
                }]
            }
            _ => {
                let xx = points.iter().map(|point| point.0).collect::<Vec<_>>();
                let yy = points.iter().map(|point| point.1).collect::<Vec<_>>();
                let dx = cubic_derivatives(&xx);
                let dy = cubic_derivatives(&yy);
                let mut segments = Vec::with_capacity(segment_count);

                for index in 0..segment_count {
                    segments.push(Segment::Cubic {
                        start: points[index],
                        control1: (xx[index] + (dx[index] / 3.0), yy[index] + (dy[index] / 3.0)),
                        control2: (
                            xx[index + 1] - (dx[index + 1] / 3.0),
                            yy[index + 1] - (dy[index + 1] / 3.0),
                        ),
                        end: points[index + 1],
                    });
                }
                segments
            }
        };

        Ok(Self { segments })
    }

    /// Evaluate the Java-compatible y ordinate at `x`.
    ///
    /// A query below the first point is evaluated on the first segment with a
    /// negative parameter, matching `GeoPath`. A query above every segment end
    /// returns [`SplineError::AbscissaOutOfRange`].
    pub fn y_at_x(&self, x: f64) -> Result<f64, SplineError> {
        let segment = self.x_segment(x)?;
        let (x1, y1) = segment.start();
        let (x2, y2) = segment.end();
        let t = (x - x1) / (x2 - x1);
        let u = 1.0 - t;

        Ok(match *segment {
            Segment::Line { .. } => y1 + (t * (y2 - y1)),
            Segment::Quadratic { control, .. } => {
                (y1 * u * u) + (2.0 * control.1 * t * u) + (y2 * t * t)
            }
            Segment::Cubic {
                control1, control2, ..
            } => {
                (y1 * u * u * u)
                    + (3.0 * control1.1 * t * u * u)
                    + (3.0 * control2.1 * t * t * u)
                    + (y2 * t * t * t)
            }
        })
    }

    /// Evaluate the Java-compatible y derivative at `x`.
    pub fn y_derivative_at_x(&self, x: f64) -> Result<f64, SplineError> {
        let segment = self.x_segment(x)?;
        let (x1, y1) = segment.start();
        let (x2, y2) = segment.end();
        let delta_x = x2 - x1;
        let t = (x - x1) / delta_x;
        let u = 1.0 - t;

        Ok(match *segment {
            Segment::Line { .. } => (y2 - y1) / delta_x,
            Segment::Quadratic { control, .. } => {
                ((-2.0 * y1 * u) + (2.0 * control.1 * (1.0 - (2.0 * t))) + (2.0 * y2 * t)) / delta_x
            }
            Segment::Cubic {
                control1, control2, ..
            } => {
                ((-3.0 * y1 * u * u)
                    + (3.0 * control1.1 * ((u * u) - (2.0 * u * t)))
                    + (3.0 * control2.1 * ((2.0 * t * u) - (t * t)))
                    + (3.0 * y2 * t * t))
                    / delta_x
            }
        })
    }

    fn x_segment(&self, x: f64) -> Result<&Segment, SplineError> {
        self.segments
            .iter()
            .find(|segment| x.partial_cmp(&segment.end().0) != Some(std::cmp::Ordering::Greater))
            .ok_or(SplineError::AbscissaOutOfRange(x))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Segment {
    Line {
        start: (f64, f64),
        end: (f64, f64),
    },
    Quadratic {
        start: (f64, f64),
        control: (f64, f64),
        end: (f64, f64),
    },
    Cubic {
        start: (f64, f64),
        control1: (f64, f64),
        control2: (f64, f64),
        end: (f64, f64),
    },
}

impl Segment {
    const fn start(self) -> (f64, f64) {
        match self {
            Self::Line { start, .. }
            | Self::Quadratic { start, .. }
            | Self::Cubic { start, .. } => start,
        }
    }

    const fn end(self) -> (f64, f64) {
        match self {
            Self::Line { end, .. } | Self::Quadratic { end, .. } | Self::Cubic { end, .. } => end,
        }
    }
}

/// Failure to interpolate or evaluate the supported spline surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SplineError {
    /// Fewer than two points were supplied.
    NotEnoughPoints,
    /// No segment end has an x coordinate at or beyond this query.
    AbscissaOutOfRange(f64),
}

impl fmt::Display for SplineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotEnoughPoints => {
                formatter.write_str("natural spline needs at least two points")
            }
            Self::AbscissaOutOfRange(x) => write!(formatter, "abscissa not in spline range: {x}"),
        }
    }
}

impl Error for SplineError {}

fn cubic_derivatives(values: &[f64]) -> Vec<f64> {
    let n = values.len() - 1;
    let mut derivatives = vec![0.0; n + 1];
    let mut gamma = vec![0.0; n + 1];
    gamma[0] = 0.5;

    for index in 1..n {
        gamma[index] = 1.0 / (4.0 - gamma[index - 1]);
    }
    gamma[n] = 1.0 / (2.0 - gamma[n - 1]);

    let mut delta = vec![0.0; n + 1];
    delta[0] = 3.0 * (values[1] - values[0]) * gamma[0];
    for index in 1..n {
        delta[index] =
            ((3.0 * (values[index + 1] - values[index - 1])) - delta[index - 1]) * gamma[index];
    }
    delta[n] = ((3.0 * (values[n] - values[n - 1])) - delta[n - 1]) * gamma[n];

    derivatives[n] = delta[n];
    for index in (0..n).rev() {
        derivatives[index] = delta[index] - (gamma[index] * derivatives[index + 1]);
    }
    derivatives
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1.0e-12,
            "actual {actual:.17} differs from expected {expected:.17}"
        );
    }

    #[test]
    fn rejects_fewer_than_two_points() {
        assert_eq!(
            NaturalSpline::interpolate(&[]),
            Err(SplineError::NotEnoughPoints)
        );
        assert_eq!(
            NaturalSpline::interpolate(&[(1.0, 2.0)]),
            Err(SplineError::NotEnoughPoints)
        );
    }

    #[test]
    fn two_points_use_line_and_java_range_asymmetry() {
        let spline = NaturalSpline::interpolate(&[(0.0, 1.0), (10.0, 6.0)]).unwrap();
        near(spline.y_at_x(0.0).unwrap(), 1.0);
        near(spline.y_at_x(4.0).unwrap(), 3.0);
        near(spline.y_at_x(10.0).unwrap(), 6.0);
        near(spline.y_at_x(-2.0).unwrap(), 0.0);
        near(spline.y_derivative_at_x(5.0).unwrap(), 0.5);
        assert_eq!(
            spline.y_at_x(10.000_001),
            Err(SplineError::AbscissaOutOfRange(10.000_001))
        );
    }

    #[test]
    fn three_points_use_java_quadratic_control_and_linear_x_parameter() {
        let spline = NaturalSpline::interpolate(&[(0.0, 0.0), (20.0, 10.0), (30.0, 10.0)]).unwrap();

        // Java creates control (25, 15), but GeoPath computes t from endpoints,
        // rather than inverting the quadratic's x coordinate.
        near(spline.y_at_x(15.0).unwrap(), 10.0);
        near(spline.y_at_x(20.0).unwrap(), 100.0 / 9.0);
        near(spline.y_derivative_at_x(0.0).unwrap(), 1.0);
        near(spline.y_derivative_at_x(30.0).unwrap(), -1.0 / 3.0);
    }

    #[test]
    fn cubic_sequence_interpolates_knots_and_preserves_java_derivative_scaling() {
        let points = [(0.0, 0.0), (12.0, 1.0), (19.0, 2.0), (30.0, 3.0)];
        let spline = NaturalSpline::interpolate(&points).unwrap();

        for &(x, y) in &points {
            near(spline.y_at_x(x).unwrap(), y);
        }
        let at_left_segment = spline.y_derivative_at_x(12.0).unwrap();
        let just_right = spline
            .y_derivative_at_x(12.0 + f64::EPSILON * 12.0)
            .unwrap();
        // GeoPath divides the shared parametric derivative by each segment's
        // endpoint delta-x, so unequal segment widths produce this jump.
        near(at_left_segment, 1.0 / 12.0);
        near(just_right, 1.0 / 7.0);
        near(spline.y_at_x(24.5).unwrap(), 2.5);
    }

    #[test]
    fn knot_selection_uses_preceding_segment_and_skips_zero_width_segment_afterward() {
        let spline =
            NaturalSpline::interpolate(&[(0.0, 0.0), (10.0, 2.0), (10.0, 2.0), (20.0, 2.0)])
                .unwrap();

        near(spline.y_at_x(10.0).unwrap(), 2.0);
        assert!(spline.y_derivative_at_x(10.0).unwrap().is_finite());
        assert!(spline.y_at_x(10.5).unwrap().is_finite());
        assert!(spline.y_derivative_at_x(10.5).unwrap().is_finite());
    }

    #[test]
    fn nan_query_selects_first_segment_like_java_comparison() {
        let spline = NaturalSpline::interpolate(&[(0.0, 0.0), (1.0, 1.0)]).unwrap();
        assert!(spline.y_at_x(f64::NAN).unwrap().is_nan());
        // The line derivative does not use the NaN parameter after selection.
        near(spline.y_derivative_at_x(f64::NAN).unwrap(), 1.0);
    }
}
