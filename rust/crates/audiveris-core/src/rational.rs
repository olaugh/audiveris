// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{cmp::Ordering, error::Error, fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rational {
    pub num: i32,
    pub den: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RationalError {
    ZeroDenominator,
    Invalid(String),
    Overflow,
}

impl fmt::Display for RationalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDenominator => f.write_str("denominator is zero"),
            Self::Invalid(value) => write!(f, "invalid rational: {value}"),
            Self::Overflow => f.write_str("rational arithmetic overflow"),
        }
    }
}

impl Error for RationalError {}

const fn gcd_i64(mut a: i64, mut b: i64) -> i64 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    if a == 0 { 1 } else { a }
}

impl Rational {
    pub const ZERO: Self = Self { num: 0, den: 1 };
    pub const QUARTER: Self = Self { num: 1, den: 4 };
    pub const HALF: Self = Self { num: 1, den: 2 };
    pub const TWO_OVER_THREE: Self = Self { num: 2, den: 3 };
    pub const ONE: Self = Self { num: 1, den: 1 };
    pub const THREE_OVER_TWO: Self = Self { num: 3, den: 2 };
    pub const TWO: Self = Self { num: 2, den: 1 };
    pub const MAX_VALUE: Self = Self {
        num: i32::MAX,
        den: 1,
    };

    pub fn new(num: i32, den: i32) -> Result<Self, RationalError> {
        Self::from_i64(i64::from(num), i64::from(den))
    }

    fn from_i64(num: i64, den: i64) -> Result<Self, RationalError> {
        if den == 0 {
            return Err(RationalError::ZeroDenominator);
        }
        let gcd = gcd_i64(num, den);
        let mut num = num / gcd;
        let mut den = den / gcd;
        if den < 0 {
            num = -num;
            den = -den;
        }
        Ok(Self {
            num: num.try_into().map_err(|_| RationalError::Overflow)?,
            den: den.try_into().map_err(|_| RationalError::Overflow)?,
        })
    }

    pub fn abs(self) -> Result<Self, RationalError> {
        Self::from_i64(i64::from(self.num).abs(), i64::from(self.den))
    }

    pub fn inverse(self) -> Result<Self, RationalError> {
        Self::new(self.den, self.num)
    }

    pub fn opposite(self) -> Result<Self, RationalError> {
        Self::from_i64(-i64::from(self.num), i64::from(self.den))
    }

    pub fn plus(self, that: Self) -> Result<Self, RationalError> {
        Self::from_i64(
            i64::from(self.num) * i64::from(that.den) + i64::from(self.den) * i64::from(that.num),
            i64::from(self.den) * i64::from(that.den),
        )
    }

    pub fn minus(self, that: Self) -> Result<Self, RationalError> {
        self.plus(that.opposite()?)
    }

    pub fn times(self, that: Self) -> Result<Self, RationalError> {
        Self::from_i64(
            i64::from(self.num) * i64::from(that.num),
            i64::from(self.den) * i64::from(that.den),
        )
    }

    pub fn divides(self, that: Self) -> Result<Self, RationalError> {
        self.times(that.inverse()?)
    }

    #[must_use]
    pub fn as_f64(self) -> f64 {
        f64::from(self.num) / f64::from(self.den)
    }

    pub fn gcd(values: &[Self]) -> Result<Self, RationalError> {
        values.iter().copied().try_fold(Self::ZERO, Self::gcd_pair)
    }

    pub fn gcd_pair(a: Self, b: Self) -> Result<Self, RationalError> {
        if a.num == 0 {
            return Ok(b);
        }
        let left = i64::from(a.den);
        let right = i64::from(b.den);
        let lcm = left / gcd_i64(left, right) * right;
        Self::from_i64(1, lcm)
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Self) -> Ordering {
        (i64::from(self.num) * i64::from(other.den))
            .cmp(&(i64::from(self.den) * i64::from(other.num)))
    }
}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == 1 {
            write!(f, "{}", self.num)
        } else {
            write!(f, "{}/{}", self.num, self.den)
        }
    }
}

impl FromStr for Rational {
    type Err = RationalError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let parts: Vec<_> = value.split('/').map(str::trim).collect();
        match parts.as_slice() {
            [num] => Self::new(
                num.parse()
                    .map_err(|_| RationalError::Invalid(value.to_owned()))?,
                1,
            ),
            [num, den] => Self::new(
                num.parse()
                    .map_err(|_| RationalError::Invalid(value.to_owned()))?,
                den.parse()
                    .map_err(|_| RationalError::Invalid(value.to_owned()))?,
            ),
            _ => Err(RationalError::Invalid(value.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(num: i32, den: i32) -> Rational {
        Rational::new(num, den).unwrap()
    }

    #[test]
    fn reduces_and_normalizes_sign() {
        assert_eq!(r(-4, -6), r(2, 3));
        assert_eq!(r(2, -3).to_string(), "-2/3");
        assert_eq!(r(2, 1).to_string(), "2");
        assert_eq!(Rational::new(1, 0), Err(RationalError::ZeroDenominator));
    }

    #[test]
    fn ports_java_arithmetic_cases() {
        assert_eq!(r(-2, 3).abs().unwrap(), r(2, 3));
        assert_eq!(r(2, 3).divides(r(4, 5)).unwrap(), r(10, 12));
        assert_eq!(r(2, 3).divides(r(2, 1)).unwrap(), r(1, 3));
        assert_eq!(r(2, 3).minus(r(1, 2)).unwrap(), r(1, 6));
        assert_eq!(r(2, 3).plus(r(1, 2)).unwrap(), r(7, 6));
        assert_eq!(r(2, 3).times(r(4, 5)).unwrap(), r(8, 15));
        assert_eq!(r(2, 3).inverse().unwrap(), r(3, 2));
        assert_eq!(r(2, 3).opposite().unwrap(), r(-2, 3));
    }

    #[test]
    fn ports_java_order_and_gcd_cases() {
        assert!(r(4, 9) < r(2, 3));
        assert_eq!(r(4, 9), r(-8, -18));
        assert!(r(3, 16) < Rational::MAX_VALUE);
        assert_eq!(
            Rational::gcd(&[r(2, 3), r(1, 4), r(5, 6)]).unwrap(),
            r(1, 12)
        );
    }

    #[test]
    fn parses_java_forms() {
        assert_eq!("3/4".parse::<Rational>().unwrap(), r(3, 4));
        assert_eq!(" 5 ".parse::<Rational>().unwrap(), r(5, 1));
        assert!((r(2, 4).as_f64() - 0.5).abs() < f64::EPSILON);
    }
}
