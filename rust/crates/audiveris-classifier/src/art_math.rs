// SPDX-License-Identifier: AGPL-3.0-or-later

//! Java-exact math used while constructing Audiveris' ART lookup tables.
//!
//! `BasicARTExtractor` builds its static tables with `Math.hypot`, `Math.atan2`,
//! `Math.cos`, and `Math.sin`. On the frozen Temurin 25 AArch64 oracle, the
//! first two follow OpenJDK's fdlibm implementation while sine and cosine use
//! HotSpot's AArch64 intrinsic. Platform libm differs by a few ulps. The
//! functions below are narrow ports of those exact source paths for the ART
//! domain: finite `atan2` inputs in `[-1, 1]`, and finite trig inputs below 64.

const ATAN_HIGH: [f64; 4] = [
    f64::from_bits(0x3fdd_ac67_0561_bb4f),
    f64::from_bits(0x3fe9_21fb_5444_2d18),
    f64::from_bits(0x3fef_730b_d281_f69b),
    f64::from_bits(0x3ff9_21fb_5444_2d18),
];
const ATAN_LOW: [f64; 4] = [
    f64::from_bits(0x3c7a_2b7f_222f_65e2),
    f64::from_bits(0x3c81_a626_3314_5c07),
    f64::from_bits(0x3c70_0788_7af0_cbbd),
    f64::from_bits(0x3c91_a626_3314_5c07),
];
const ATAN_COEFFICIENTS: [f64; 11] = [
    f64::from_bits(0x3fd5_5555_5555_550d),
    f64::from_bits(0xbfc9_9999_9998_ebc4),
    f64::from_bits(0x3fc2_4924_9200_83ff),
    f64::from_bits(0xbfbc_71c6_fe23_1671),
    f64::from_bits(0x3fb7_45cd_c54c_206e),
    f64::from_bits(0xbfb3_b0f2_af74_9a6d),
    f64::from_bits(0x3fb1_0d66_a0d0_3d51),
    f64::from_bits(0xbfad_de2d_52de_fd9a),
    f64::from_bits(0x3fa9_7b4b_2476_0deb),
    f64::from_bits(0xbfa2_b444_2c6a_6c2f),
    f64::from_bits(0x3f90_ad3a_e322_da11),
];

const SIN_COEFFICIENTS: [f64; 6] = [
    f64::from_bits(0xbfc5_5555_5555_5549),
    f64::from_bits(0x3f81_1111_1110_f8a6),
    f64::from_bits(0xbf2a_01a0_19c1_61d5),
    f64::from_bits(0x3ec7_1de3_57b1_fe7d),
    f64::from_bits(0xbe5a_e5e6_8a2b_9ceb),
    f64::from_bits(0x3de5_d93a_5acf_d57c),
];
const COS_COEFFICIENTS: [f64; 6] = [
    f64::from_bits(0x3fa5_5555_5555_554c),
    f64::from_bits(0xbf56_c16c_16c1_5177),
    f64::from_bits(0x3efa_01a0_19cb_1590),
    f64::from_bits(0xbe92_7e4f_809c_52ad),
    f64::from_bits(0x3e21_ee9e_bdb4_b1c4),
    f64::from_bits(0xbda8_fae9_be88_38d4),
];

const NPIO2_HIGH_WORDS: [u32; 32] = [
    0x3ff9_21fb,
    0x4009_21fb,
    0x4012_d97c,
    0x4019_21fb,
    0x401f_6a7a,
    0x4022_d97c,
    0x4025_fdbb,
    0x4029_21fb,
    0x402c_463a,
    0x402f_6a7a,
    0x4031_475c,
    0x4032_d97c,
    0x4034_6b9c,
    0x4035_fdbb,
    0x4037_8fdb,
    0x4039_21fb,
    0x403a_b41b,
    0x403c_463a,
    0x403d_d85a,
    0x403f_6a7a,
    0x4040_7e4c,
    0x4041_475c,
    0x4042_106c,
    0x4042_d97c,
    0x4043_a28c,
    0x4044_6b9c,
    0x4045_34ac,
    0x4045_fdbb,
    0x4046_c6cb,
    0x4047_8fdb,
    0x4048_58eb,
    0x4049_21fb,
];

fn high_word(value: f64) -> i32 {
    (value.to_bits() >> 32) as u32 as i32
}

fn low_word(value: f64) -> u32 {
    value.to_bits() as u32
}

fn java_atan(mut value: f64) -> f64 {
    let signed_high = high_word(value);
    let absolute_high = signed_high & 0x7fff_ffff;
    let reduction;
    if absolute_high < 0x3fdc_0000 {
        if absolute_high < 0x3e20_0000 {
            return value;
        }
        reduction = -1;
    } else {
        value = value.abs();
        if absolute_high < 0x3ff3_0000 {
            if absolute_high < 0x3fe6_0000 {
                reduction = 0;
                value = ((2.0 * value) - 1.0) / (2.0 + value);
            } else {
                reduction = 1;
                value = (value - 1.0) / (value + 1.0);
            }
        } else if absolute_high < 0x4003_8000 {
            reduction = 2;
            value = (value - 1.5) / (1.0 + (1.5 * value));
        } else {
            reduction = 3;
            value = -1.0 / value;
        }
    }

    let square = value * value;
    let fourth = square * square;
    let odd = square
        * (ATAN_COEFFICIENTS[0]
            + fourth
                * (ATAN_COEFFICIENTS[2]
                    + fourth
                        * (ATAN_COEFFICIENTS[4]
                            + fourth
                                * (ATAN_COEFFICIENTS[6]
                                    + fourth
                                        * (ATAN_COEFFICIENTS[8]
                                            + fourth * ATAN_COEFFICIENTS[10])))));
    let even = fourth
        * (ATAN_COEFFICIENTS[1]
            + fourth
                * (ATAN_COEFFICIENTS[3]
                    + fourth
                        * (ATAN_COEFFICIENTS[5]
                            + fourth * (ATAN_COEFFICIENTS[7] + fourth * ATAN_COEFFICIENTS[9]))));
    if reduction < 0 {
        value - (value * (odd + even))
    } else {
        let index = reduction as usize;
        let result = ATAN_HIGH[index] - (((value * (odd + even)) - ATAN_LOW[index]) - value);
        if signed_high < 0 { -result } else { result }
    }
}

pub(super) fn java_atan2(y: f64, x: f64) -> f64 {
    let x_high = high_word(x);
    let x_absolute = x_high & 0x7fff_ffff;
    let x_low = low_word(x);
    let y_high = high_word(y);
    let y_absolute = y_high & 0x7fff_ffff;
    let y_low = low_word(y);

    if ((x_high.wrapping_sub(0x3ff0_0000)) as u32 | x_low) == 0 {
        return java_atan(y);
    }
    let quadrant = ((y_high >> 31) & 1) | ((x_high >> 30) & 2);
    if (y_absolute as u32 | y_low) == 0 {
        return match quadrant {
            0 | 1 => y,
            2 => std::f64::consts::PI,
            _ => -std::f64::consts::PI,
        };
    }
    const HALF_PI: f64 = f64::from_bits(0x3ff9_21fb_5444_2d18);
    const PI_LOW: f64 = f64::from_bits(0x3ca1_a626_3314_5c07);
    if (x_absolute as u32 | x_low) == 0 {
        return if y_high < 0 { -HALF_PI } else { HALF_PI };
    }
    let exponent_delta = (y_absolute - x_absolute) >> 20;
    let angle = if exponent_delta > 60 {
        HALF_PI + (0.5 * PI_LOW)
    } else if x_high < 0 && exponent_delta < -60 {
        0.0
    } else {
        java_atan((y / x).abs())
    };
    match quadrant {
        0 => angle,
        1 => -angle,
        2 => std::f64::consts::PI - (angle - PI_LOW),
        _ => (angle - PI_LOW) - std::f64::consts::PI,
    }
}

fn reduce_pi_over_two(value: f64) -> (i32, f64, f64) {
    const INVERSE_HALF_PI: f64 = f64::from_bits(0x3fe4_5f30_6dc9_c883);
    const HALF_PI_1: f64 = f64::from_bits(0x3ff9_21fb_5440_0000);
    const HALF_PI_1_TAIL: f64 = f64::from_bits(0x3dd0_b461_1a62_6331);
    const HALF_PI_2: f64 = f64::from_bits(0x3dd0_b461_1a60_0000);
    const HALF_PI_2_TAIL: f64 = f64::from_bits(0x3ba3_198a_2e03_7073);
    const HALF_PI_3: f64 = f64::from_bits(0x3ba3_198a_2e00_0000);
    const HALF_PI_3_TAIL: f64 = f64::from_bits(0x397b_839a_2520_49c1);

    let signed_high = high_word(value);
    let absolute_high = signed_high as u32 & 0x7fff_ffff;
    if absolute_high < 0x4002_d97c {
        let (quadrant, remainder, sign) = if signed_high > 0 {
            (1, value - HALF_PI_1, 1.0)
        } else {
            (-1, value + HALF_PI_1, -1.0)
        };
        if absolute_high != 0x3ff9_21fb {
            let high = remainder - (sign * HALF_PI_1_TAIL);
            return (quadrant, high, (remainder - high) - (sign * HALF_PI_1_TAIL));
        }
        let remainder = remainder - (sign * HALF_PI_2);
        let high = remainder - (sign * HALF_PI_2_TAIL);
        return (quadrant, high, (remainder - high) - (sign * HALF_PI_2_TAIL));
    }

    let absolute = value.abs();
    let rounded = absolute.mul_add(INVERSE_HALF_PI, 0.5);
    let mut quadrant = rounded.trunc() as i32;
    let quadrant_float = f64::from(quadrant);
    let mut remainder = (-quadrant_float).mul_add(HALF_PI_1, absolute);
    let mut tail = quadrant_float * HALF_PI_1_TAIL;
    let mut high = remainder - tail;
    if !(quadrant < 32 && absolute_high != NPIO2_HIGH_WORDS[quadrant as usize - 1]) {
        let exponent = (absolute_high >> 20) as i32;
        let mut cancellation = exponent - (((high_word(high) as u32 >> 20) & 0x7ff) as i32);
        if cancellation > 16 {
            let previous = remainder;
            let product = quadrant_float * HALF_PI_2;
            remainder = previous - product;
            tail = quadrant_float.mul_add(HALF_PI_2_TAIL, product - (previous - remainder));
            high = remainder - tail;
            cancellation = exponent - (((high_word(high) as u32 >> 20) & 0x7ff) as i32);
            if cancellation > 49 {
                let previous = remainder;
                let product = quadrant_float * HALF_PI_3;
                remainder = previous - product;
                tail = quadrant_float.mul_add(HALF_PI_3_TAIL, product - (previous - remainder));
                high = remainder - tail;
            }
        }
    }
    let mut low = (remainder - high) - tail;
    if signed_high < 0 {
        quadrant = -quadrant;
        high = -high;
        low = -low;
    }
    (quadrant, high, low)
}

fn kernel_sin(value: f64, tail: f64, has_tail: bool) -> f64 {
    let square = value * value;
    let cube = square * value;
    let mut polynomial = square.mul_add(SIN_COEFFICIENTS[5], SIN_COEFFICIENTS[4]);
    polynomial = square.mul_add(polynomial, SIN_COEFFICIENTS[3]);
    polynomial = square.mul_add(polynomial, SIN_COEFFICIENTS[2]);
    polynomial = square.mul_add(polynomial, SIN_COEFFICIENTS[1]);
    if !has_tail {
        return cube.mul_add(square.mul_add(polynomial, SIN_COEFFICIENTS[0]), value);
    }
    let corrected = (-cube).mul_add(polynomial, 0.5 * tail);
    let corrected = (-square).mul_add(corrected, tail);
    value + cube.mul_add(SIN_COEFFICIENTS[0], corrected)
}

fn kernel_cos(value: f64, tail: f64, original_high: u32) -> f64 {
    let square = value * value;
    let fourth = square * square;
    let value_tail = value * tail;
    let mut polynomial = square.mul_add(COS_COEFFICIENTS[5], COS_COEFFICIENTS[4]);
    polynomial = square.mul_add(polynomial, COS_COEFFICIENTS[3]);
    polynomial = square.mul_add(polynomial, COS_COEFFICIENTS[2]);
    polynomial = square.mul_add(polynomial, COS_COEFFICIENTS[1]);
    polynomial = square.mul_add(polynomial, COS_COEFFICIENTS[0]);
    if original_high < 0x3fd3_3333 {
        let correction = (-fourth).mul_add(polynomial, value_tail);
        return 1.0 - 0.5_f64.mul_add(square, correction);
    }
    // HotSpot deliberately retains the original argument's high word after
    // reduction. Recomputing it from `value` changes the final ulp.
    let quarter = if original_high > 0x3fe9_0000 {
        0.28125
    } else {
        f64::from_bits(u64::from(original_high - 0x0020_0000) << 32)
    };
    let correction = fourth.mul_add(polynomial, -value_tail);
    let half_square = (0.5 * square) - quarter;
    (1.0 - quarter) - (half_square - correction)
}

pub(super) fn java_cos(value: f64) -> f64 {
    let original_high = high_word(value) as u32 & 0x7fff_ffff;
    if original_high < 0x3e40_0000 {
        return 1.0;
    }
    if original_high <= 0x3fe9_21fb {
        return kernel_cos(value, 0.0, original_high);
    }
    let (quadrant, high, low) = reduce_pi_over_two(value);
    match quadrant & 3 {
        0 => kernel_cos(high, low, original_high),
        1 => -kernel_sin(high, low, true),
        2 => -kernel_cos(high, low, original_high),
        _ => kernel_sin(high, low, true),
    }
}

pub(super) fn java_sin(value: f64) -> f64 {
    let original_high = high_word(value) as u32 & 0x7fff_ffff;
    if original_high < 0x3e40_0000 {
        return value;
    }
    if original_high <= 0x3fe9_21fb {
        return kernel_sin(value, 0.0, false);
    }
    let (quadrant, high, low) = reduce_pi_over_two(value);
    match quadrant & 3 {
        0 => kernel_sin(high, low, true),
        1 => kernel_cos(high, low, original_high),
        2 => -kernel_sin(high, low, true),
        _ => -kernel_cos(high, low, original_high),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiveris_core::basic_line::java_hypot;

    fn hash(values: impl IntoIterator<Item = f64>) -> u64 {
        values
            .into_iter()
            .fold(0xcbf2_9ce4_8422_2325, |hash, value| {
                (hash ^ value.to_bits()).wrapping_mul(0x100_0000_01b3)
            })
    }

    #[test]
    fn hotspot_trig_matches_java_math_across_art_range() {
        let values = (0..=1_000).map(|index| (f64::from(index) - 500.0) / 37.0);
        assert_eq!(hash(values.clone().map(java_cos)), 0x9512_8b69_422b_7e4f);
        assert_eq!(hash(values.map(java_sin)), 0xb6cc_744c_5b1c_af67);
    }

    #[test]
    fn art_lut_operations_match_temurin_25_bits() {
        let mut radial = Vec::new();
        let mut angle = Vec::new();
        let mut basis = Vec::new();
        let mut angular_cos = Vec::new();
        let mut angular_sin = Vec::new();
        for x in 0..101 {
            let normalized_x = (f64::from(x) - 50.0) / 50.0;
            for y in 0..101 {
                let normalized_y = (f64::from(y) - 50.0) / 50.0;
                let radius = java_hypot(normalized_x, normalized_y);
                if radius < 1.0 {
                    let theta = java_atan2(normalized_y, normalized_x);
                    radial.push(radius);
                    angle.push(theta);
                    for angular in 0..20 {
                        for radial_order in 0..5 {
                            basis.push(java_cos(
                                radius * std::f64::consts::PI * f64::from(radial_order),
                            ));
                            angular_cos.push(java_cos(theta * f64::from(angular)));
                            angular_sin.push(java_sin(theta * f64::from(angular)));
                        }
                    }
                }
            }
        }
        assert_eq!(radial.len(), 7_825);
        assert_eq!(hash(radial), 0xfbb0_9ebb_1fb4_26c7);
        assert_eq!(hash(angle), 0xa60a_ba16_9d56_ee8f);
        assert_eq!(hash(basis), 0x82e7_7be3_1d98_d245);
        assert_eq!(hash(angular_cos), 0xe8e0_d273_7835_d14d);
        assert_eq!(hash(angular_sin), 0xbf00_032c_0f5b_58f6);
    }
}
