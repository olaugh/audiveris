// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineCoefficients {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub rather_vertical: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineError {
    NotEnoughPoints,
    Singular,
    Horizontal,
    Vertical,
    DifferentLengths,
}

impl fmt::Display for LineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotEnoughPoints => f.write_str("not enough defining points"),
            Self::Singular => f.write_str("line parameters are not properly defined"),
            Self::Horizontal => f.write_str("line is horizontal"),
            Self::Vertical => f.write_str("line is vertical"),
            Self::DifferentLengths => f.write_str("coordinate arrays have different lengths"),
        }
    }
}

impl Error for LineError {}

#[derive(Debug, Clone, PartialEq)]
pub struct BasicLine {
    sx: f64,
    sx2: f64,
    sxy: f64,
    sy: f64,
    sy2: f64,
    count: usize,
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
}

impl Default for BasicLine {
    fn default() -> Self {
        Self {
            sx: 0.0,
            sx2: 0.0,
            sxy: 0.0,
            sy: 0.0,
            sy2: 0.0,
            count: 0,
            x_min: f64::INFINITY,
            x_max: f64::NEG_INFINITY,
            y_min: f64::INFINITY,
            y_max: f64::NEG_INFINITY,
        }
    }
}

impl BasicLine {
    pub fn from_coordinates(x: &[f64], y: &[f64]) -> Result<Self, LineError> {
        if x.len() != y.len() {
            return Err(LineError::DifferentLengths);
        }
        if x.len() < 2 {
            return Err(LineError::NotEnoughPoints);
        }
        let mut line = Self::default();
        // Match the Java constructor's reverse inclusion order.
        for index in (0..x.len()).rev() {
            line.include_point(x[index], y[index]);
        }
        line.coefficients()?;
        Ok(line)
    }

    pub fn from_points(points: &[(f64, f64)]) -> Result<Self, LineError> {
        if points.len() < 2 {
            return Err(LineError::NotEnoughPoints);
        }
        let mut line = Self::default();
        for &(x, y) in points {
            line.include_point(x, y);
        }
        line.coefficients()?;
        Ok(line)
    }

    pub fn include_point(&mut self, x: f64, y: f64) {
        self.count += 1;
        self.sx += x;
        self.sy += y;
        self.sx2 += x * x;
        self.sy2 += y * y;
        self.sxy += x * y;
        self.x_min = self.x_min.min(x);
        self.x_max = self.x_max.max(x);
        self.y_min = self.y_min.min(y);
        self.y_max = self.y_max.max(y);
    }

    pub fn include_line(&mut self, other: &Self) {
        self.count += other.count;
        self.sx += other.sx;
        self.sy += other.sy;
        self.sx2 += other.sx2;
        self.sy2 += other.sy2;
        self.sxy += other.sxy;
        self.x_min = self.x_min.min(other.x_min);
        self.x_max = self.x_max.max(other.x_max);
        self.y_min = self.y_min.min(other.y_min);
        self.y_max = self.y_max.max(other.y_max);
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.count
    }

    pub fn coefficients(&self) -> Result<LineCoefficients, LineError> {
        if self.count < 2 {
            return Err(LineError::NotEnoughPoints);
        }
        let n = self.count as f64;
        let horizontal_denominator = n * self.sx2 - self.sx * self.sx;
        let vertical_denominator = n * self.sy2 - self.sy * self.sy;
        let (mut a, mut b, mut c, rather_vertical) =
            if horizontal_denominator.abs() >= vertical_denominator.abs() {
                (
                    (n * self.sxy - self.sx * self.sy) / horizontal_denominator,
                    -1.0,
                    (self.sy * self.sx2 - self.sx * self.sxy) / horizontal_denominator,
                    false,
                )
            } else {
                (
                    -1.0,
                    (n * self.sxy - self.sx * self.sy) / vertical_denominator,
                    (self.sx * self.sy2 - self.sy * self.sxy) / vertical_denominator,
                    true,
                )
            };
        let norm = java_hypot(a, b);
        a /= norm;
        b /= norm;
        c /= norm;
        if !a.is_finite() || !b.is_finite() || !c.is_finite() {
            return Err(LineError::Singular);
        }
        Ok(LineCoefficients {
            a,
            b,
            c,
            rather_vertical,
        })
    }

    pub fn distance_of(&self, x: f64, y: f64) -> Result<f64, LineError> {
        let line = self.coefficients()?;
        Ok(line.a * x + line.b * y + line.c)
    }

    pub fn mean_distance(&self) -> Result<f64, LineError> {
        let line = self.coefficients()?;
        let n = self.count as f64;
        let distance_squared = (line.a * line.a * self.sx2
            + line.b * line.b * self.sy2
            + line.c * line.c * n
            + 2.0 * line.a * line.b * self.sxy
            + 2.0 * line.a * line.c * self.sx
            + 2.0 * line.b * line.c * self.sy)
            / n;
        Ok(distance_squared.max(0.0).sqrt())
    }

    pub fn slope(&self) -> Result<f64, LineError> {
        let line = self.coefficients()?;
        Ok(-line.a / line.b)
    }

    pub fn inverted_slope(&self) -> Result<f64, LineError> {
        let line = self.coefficients()?;
        Ok(-line.b / line.a)
    }

    pub fn x_at_y(&self, y: f64) -> Result<f64, LineError> {
        if self.count == 1 {
            return Ok(self.sx);
        }
        let line = self.coefficients()?;
        if line.a == 0.0 {
            Err(LineError::Horizontal)
        } else {
            Ok(-(line.b * y + line.c) / line.a)
        }
    }

    pub fn y_at_x(&self, x: f64) -> Result<f64, LineError> {
        if self.count == 1 {
            return Ok(self.sy);
        }
        let line = self.coefficients()?;
        if line.b == 0.0 {
            Err(LineError::Vertical)
        } else {
            Ok((-line.a * x - line.c) / line.b)
        }
    }

    pub fn x_at_y_int(&self, y: i32) -> Result<i32, LineError> {
        Ok(self.x_at_y(f64::from(y))?.round_ties_even() as i32)
    }

    pub fn y_at_x_int(&self, x: i32) -> Result<i32, LineError> {
        Ok(self.y_at_x(f64::from(x))?.round_ties_even() as i32)
    }

    #[must_use]
    pub fn swapped_coordinates(&self) -> Self {
        Self {
            sx: self.sy,
            sx2: self.sy2,
            sxy: self.sxy,
            sy: self.sx,
            sy2: self.sx2,
            count: self.count,
            x_min: self.y_min,
            x_max: self.y_max,
            y_min: self.x_min,
            y_max: self.x_max,
        }
    }

    #[must_use]
    pub fn extents(&self) -> Option<(f64, f64, f64, f64)> {
        (self.count > 0).then_some((self.x_min, self.x_max, self.y_min, self.y_max))
    }
}

/// `Math.hypot`, as OpenJDK computes it.
///
/// Not `f64::hypot`. HotSpot does not intrinsify `hypot` the way it does `sin`,
/// `cos`, `log`, `exp` and `pow`, so `Math.hypot` is `StrictMath.hypot`, which
/// is fdlibm's `__ieee754_hypot` -- a specific algorithm, not merely a
/// correctly-rounded one. Rust's `f64::hypot` calls the platform libm and
/// differs from it by an ulp often enough to matter here: `BasicLine`
/// normalises its coefficients by this, and one ulp in the norm is one ulp in
/// every distance measured from the line. It was the last residual between the
/// port's beams and Java's, showing up as a beam whose jitter impact read
/// 0.847273069 against Java's 0.847273070.
///
/// Ported from fdlibm rather than reimplemented: the point is to be bit-equal
/// to Java, and any independently "better" formulation would not be.
#[must_use]
pub fn java_hypot(x: f64, y: f64) -> f64 {
    let mut ha = (x.to_bits() >> 32) as u32 & 0x7fff_ffff;
    let mut hb = (y.to_bits() >> 32) as u32 & 0x7fff_ffff;
    let (mut a, mut b) = if hb > ha {
        std::mem::swap(&mut ha, &mut hb);
        (y, x)
    } else {
        (x, y)
    };
    a = f64::from_bits((a.to_bits() & 0x0000_0000_ffff_ffff) | (u64::from(ha) << 32));
    b = f64::from_bits((b.to_bits() & 0x0000_0000_ffff_ffff) | (u64::from(hb) << 32));

    // x/y greater than 2^60: the smaller operand cannot affect the result.
    if ha.wrapping_sub(hb) > 0x03c0_0000 {
        return a + b;
    }

    let mut k = 0_i32;
    if ha > 0x5f30_0000 {
        if ha >= 0x7ff0_0000 {
            let mut w = a + b;
            if (ha & 0xf_ffff) | (a.to_bits() as u32) == 0 {
                w = a;
            }
            if (hb ^ 0x7ff0_0000) | (b.to_bits() as u32) == 0 {
                w = b;
            }
            return w;
        }
        ha -= 0x2580_0000;
        hb -= 0x2580_0000;
        k += 600;
        a = f64::from_bits((a.to_bits() & 0xffff_ffff) | (u64::from(ha) << 32));
        b = f64::from_bits((b.to_bits() & 0xffff_ffff) | (u64::from(hb) << 32));
    }
    if hb < 0x20b0_0000 {
        if hb <= 0x000f_ffff {
            if hb | (b.to_bits() as u32) == 0 {
                return a;
            }
            let tiny = f64::from_bits(0x7fd0_0000_u64 << 32);
            b *= tiny;
            a *= tiny;
            k -= 1022;
        } else {
            ha += 0x2580_0000;
            hb += 0x2580_0000;
            k -= 600;
            a = f64::from_bits((a.to_bits() & 0xffff_ffff) | (u64::from(ha) << 32));
            b = f64::from_bits((b.to_bits() & 0xffff_ffff) | (u64::from(hb) << 32));
        }
    }

    let mut w = a - b;
    if w > b {
        let t1 = f64::from_bits(u64::from(ha) << 32);
        let t2 = a - t1;
        w = (t1 * t1 - (b * (-b) - t2 * (a + t1))).sqrt();
    } else {
        a += a;
        let y1 = f64::from_bits(u64::from(hb) << 32);
        let y2 = b - y1;
        let t1 = f64::from_bits(u64::from(ha + 0x0010_0000) << 32);
        let t2 = a - t1;
        w = (t1 * y1 - (w * (-w) - (t1 * y2 + t2 * b))).sqrt();
    }
    if k != 0 {
        f64::from_bits(((1.0_f64).to_bits() as i64 + (i64::from(k) << 52)) as u64) * w
    } else {
        w
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const X: [f64; 5] = [1.0, 2.0, 3.0, 4.0, 5.0];
    const Y: [f64; 5] = [4.0, 9.0, 14.0, 19.0, 24.0];

    fn near(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
    }

    #[test]
    fn ports_java_regression_coefficients_and_distances() {
        let line = BasicLine::from_coordinates(&X, &Y).unwrap();
        let coefficients = line.coefficients().unwrap();
        near(coefficients.a, -0.980_580_675_690_920_2);
        near(coefficients.b, 0.196_116_135_138_183_77);
        near(coefficients.c, 0.196_116_135_138_187_7);
        near(
            line.distance_of(0.0, 0.0).unwrap(),
            0.196_116_135_138_184_04,
        );
        near(
            line.distance_of(1.0, 0.0).unwrap(),
            -0.784_464_540_552_736_1,
        );
        near(line.distance_of(1.0, 10.0).unwrap(), 1.176_696_810_829_104);
        // Match the Java test's near-equality tolerance; cancellation leaves a tiny residue.
        assert!(line.mean_distance().unwrap() < 1e-7);
        assert_eq!(line.count(), 5);
    }

    #[test]
    fn ports_java_include_and_intercepts() {
        let mut line = BasicLine::default();
        line.include_point(1.0, 3.0);
        line.include_point(-1.0, 1.0);
        let coefficients = line.coefficients().unwrap();
        near(coefficients.a, 0.5_f64.sqrt());
        near(coefficients.b, -0.5_f64.sqrt());
        near(coefficients.c, 2.0_f64.sqrt());
        near(line.x_at_y(0.0).unwrap(), -2.0);
        near(line.x_at_y(2.0).unwrap(), 0.0);
        near(line.y_at_x(-2.0).unwrap(), 0.0);
        near(line.y_at_x(0.0).unwrap(), 2.0);
    }

    #[test]
    fn ports_java_horizontal_vertical_and_single_point_behavior() {
        let mut horizontal = BasicLine::default();
        horizontal.include_point(0.0, 0.0);
        horizontal.include_point(1.0, 0.0);
        assert_eq!(horizontal.x_at_y(0.0), Err(LineError::Horizontal));
        near(horizontal.y_at_x(5.0).unwrap(), 0.0);

        let mut vertical = BasicLine::default();
        vertical.include_point(0.0, 0.0);
        vertical.include_point(0.0, 1.0);
        near(vertical.x_at_y(5.0).unwrap(), 0.0);
        assert_eq!(vertical.y_at_x(0.0), Err(LineError::Vertical));

        let mut singleton = BasicLine::default();
        singleton.include_point(7.0, 9.0);
        near(singleton.x_at_y(100.0).unwrap(), 7.0);
        near(singleton.y_at_x(100.0).unwrap(), 9.0);
        assert_eq!(
            singleton.distance_of(0.0, 0.0),
            Err(LineError::NotEnoughPoints)
        );
    }

    #[test]
    fn ports_java_tangent_and_rounding_cases() {
        let mut line = BasicLine::default();
        for point in [
            (214.0, 624.0),
            (215.0, 625.0),
            (215.0, 626.0),
            (215.0, 627.0),
        ] {
            line.include_point(point.0, point.1);
        }
        near(line.x_at_y(624.0).unwrap(), 214.3);
        near(line.x_at_y(625.0).unwrap(), 214.6);
        near(line.x_at_y(626.0).unwrap(), 214.9);
        near(line.x_at_y(627.0).unwrap(), 215.2);

        let mut x_round = BasicLine::default();
        x_round.include_point(0.0, 0.0);
        x_round.include_point(2.0, 5.0);
        assert_eq!(x_round.x_at_y_int(3).unwrap(), 1);
        let mut y_round = BasicLine::default();
        y_round.include_point(0.0, 0.0);
        y_round.include_point(5.0, 2.0);
        assert_eq!(y_round.y_at_x_int(4).unwrap(), 2);
    }

    #[test]
    fn ports_java_line_merge_reset_swap_and_validation() {
        let mut first = BasicLine::default();
        let mut second = BasicLine::default();
        for offset in -1..=1 {
            first.include_point(-100.0 + f64::from(offset), -1.0);
            second.include_point(100.0 + f64::from(offset), 1.0);
        }
        first.include_line(&second);
        let coefficients = first.coefficients().unwrap();
        assert!((coefficients.a - 0.01).abs() < 1e-3);
        assert!((coefficients.b + 1.0).abs() < 1e-3);
        assert!(coefficients.c.abs() < 1e-3);

        let swapped = first.swapped_coordinates();
        near(swapped.x_at_y(0.0).unwrap(), first.y_at_x(0.0).unwrap());
        first.reset();
        assert_eq!(first.count(), 0);
        assert_eq!(first.coefficients(), Err(LineError::NotEnoughPoints));
        assert_eq!(
            BasicLine::from_coordinates(&[1.0], &[2.0]),
            Err(LineError::NotEnoughPoints)
        );
        assert_eq!(
            BasicLine::from_coordinates(&[1.0], &[2.0, 3.0]),
            Err(LineError::DifferentLengths)
        );
        assert_eq!(
            BasicLine::from_coordinates(&[1.0, 1.0, 1.0], &[2.0, 2.0, 2.0]),
            Err(LineError::Singular)
        );
    }
}
