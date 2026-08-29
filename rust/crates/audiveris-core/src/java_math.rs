// SPDX-License-Identifier: AGPL-3.0-or-later

//! Floating-point operations whose specified Java implementation matters.

const EXP_SIGNIF_BITS: i32 = 0x7fff_ffff;

fn high(value: f64) -> i32 {
    (value.to_bits() >> 32) as u32 as i32
}

fn low(value: f64) -> i32 {
    value.to_bits() as u32 as i32
}

fn with_high(value: f64, high: i32) -> f64 {
    f64::from_bits((value.to_bits() & 0xffff_ffff) | ((high as u32 as u64) << 32))
}

fn zero_low(value: f64) -> f64 {
    f64::from_bits(value.to_bits() & 0xffff_ffff_0000_0000)
}

const fn bits(value: u64) -> f64 {
    f64::from_bits(value)
}

/// OpenJDK fdlibm `pow` for a non-negative base and finite exponent.
///
/// Java `Math.pow` and `StrictMath.pow` take this path for the weighted
/// geometric means used by Audiveris. Platform `pow` differs by one ulp often
/// enough to change frozen grades, so this is a direct translation of
/// `java.lang.FdLibm.Pow`, narrowed only by the public preconditions.
#[must_use]
pub fn java_positive_pow(x: f64, y: f64) -> f64 {
    assert!((x.is_nan() || x >= 0.0) && y.is_finite());
    if y == 0.0 {
        return 1.0;
    }
    if x.is_nan() {
        return x;
    }
    if y == 2.0 {
        return x * x;
    }
    if y == 0.5 {
        return x.sqrt();
    }
    if y.abs() == 1.0 {
        return if y == 1.0 { x } else { 1.0 / x };
    }
    if x == 0.0 || x == 1.0 || x.is_infinite() {
        return if y < 0.0 { 1.0 / x } else { x };
    }

    let cp = bits(0x3fee_c709_dc3a_03fd);
    let cp_h = bits(0x3fee_c709_e000_0000);
    let cp_l = bits(0xbe3e_2fe0_145b_01f5);
    let dp_h = [0.0, bits(0x3fe2_b803_4000_0000)];
    let dp_l = [0.0, bits(0x3e4c_fdeb_43cf_d006)];
    let l1 = bits(0x3fe3_3333_3333_3303);
    let l2 = bits(0x3fdb_6db6_db6f_abff);
    let l3 = bits(0x3fd5_5555_518f_264d);
    let l4 = bits(0x3fd1_7460_a91d_4101);
    let l5 = bits(0x3fcd_864a_93c9_db65);
    let l6 = bits(0x3fca_7e28_4a45_4eef);

    let mut x_abs = x;
    let mut ix = high(x_abs) & EXP_SIGNIF_BITS;
    let mut n = 0_i32;
    if ix < 0x0010_0000 {
        x_abs *= 9_007_199_254_740_992.0; // 2^53
        n -= 53;
        ix = high(x_abs);
    }
    n += (ix >> 20) - 0x3ff;
    let j = ix & 0x000f_ffff;
    ix = j | 0x3ff0_0000;
    let k;
    if j <= 0x3988e {
        k = 0;
    } else if j < 0xbb67a {
        k = 1;
    } else {
        k = 0;
        n += 1;
        ix -= 0x0010_0000;
    }
    x_abs = with_high(x_abs, ix);

    let bp = [1.0, 1.5];
    let u = x_abs - bp[k];
    let reciprocal = 1.0 / (x_abs + bp[k]);
    let ss = u * reciprocal;
    let s_h = zero_low(ss);
    let t_h = with_high(
        0.0,
        ((ix >> 1) | 0x2000_0000) + 0x0008_0000 + ((k as i32) << 18),
    );
    let t_l = x_abs - (t_h - bp[k]);
    let s_l = reciprocal * ((u - s_h * t_h) - s_h * t_l);
    let mut s2 = ss * ss;
    let mut r = s2 * s2 * (l1 + s2 * (l2 + s2 * (l3 + s2 * (l4 + s2 * (l5 + s2 * l6)))));
    r += s_l * (s_h + ss);
    s2 = s_h * s_h;
    let log_t_h = zero_low(3.0 + s2 + r);
    let log_t_l = r - ((log_t_h - 3.0) - s2);
    let u = s_h * log_t_h;
    let v = s_l * log_t_h + log_t_l * ss;
    let p_h = zero_low(u + v);
    let p_l = v - (p_h - u);
    let z_h = cp_h * p_h;
    let z_l = cp_l * p_h + p_l * cp + dp_l[k];
    let t = f64::from(n);
    let t1 = zero_low(((z_h + z_l) + dp_h[k]) + t);
    let t2 = z_l - (((t1 - t) - dp_h[k]) - z_h);

    let y1 = zero_low(y);
    let p_l = (y - y1) * t1 + y * t2;
    let mut p_h = y1 * t1;
    let mut z = p_l + p_h;
    let mut j = high(z);
    let i = low(z);
    if j >= 0x4090_0000 {
        if ((j - 0x4090_0000) | i) != 0 {
            return f64::INFINITY;
        }
        let ovt = 8.008_566_259_537_294e-17;
        if p_l + ovt > z - p_h {
            return f64::INFINITY;
        }
    } else if (j & EXP_SIGNIF_BITS) >= 0x4090_cc00
        && (((j.wrapping_sub(0xc090_cc00_u32 as i32)) | i) != 0 || p_l <= z - p_h)
    {
        return 0.0;
    }

    let p1 = bits(0x3fc5_5555_5555_553e);
    let p2 = bits(0xbf66_c16c_16be_bd93);
    let p3 = bits(0x3f11_566a_af25_de2c);
    let p4 = bits(0xbebb_bd41_c5d2_6bf1);
    let p5 = bits(0x3e66_3769_72be_a4d0);
    let lg2 = bits(0x3fe6_2e42_fefa_39ef);
    let lg2_h = bits(0x3fe6_2e43_0000_0000);
    let lg2_l = bits(0xbe20_5c61_0ca8_6c39);

    let magnitude = j & EXP_SIGNIF_BITS;
    let mut k = (magnitude >> 20) - 0x3ff;
    n = 0;
    if magnitude > 0x3fe0_0000 {
        let rounded = j.wrapping_add(0x0010_0000_i32.wrapping_shr((k + 1) as u32));
        k = ((rounded & EXP_SIGNIF_BITS) >> 20) - 0x3ff;
        let mask = 0x000f_ffff_i32.wrapping_shr(k as u32);
        let t = with_high(0.0, rounded & !mask);
        n = ((rounded & 0x000f_ffff) | 0x0010_0000).wrapping_shr((20 - k) as u32);
        if j < 0 {
            n = -n;
        }
        p_h -= t;
    }
    let t = zero_low(p_l + p_h);
    let u = t * lg2_h;
    let v = (p_l - (t - p_h)) * lg2 + t * lg2_l;
    z = u + v;
    let w = v - (z - u);
    let t = z * z;
    let t1 = z - t * (p1 + t * (p2 + t * (p3 + t * (p4 + t * p5))));
    r = (z * t1) / (t1 - 2.0) - (w + z * w);
    z = 1.0 - (r - z);
    j = high(z).wrapping_add(n << 20);
    if (j >> 20) <= 0 {
        // The grade use-case has a positive fractional exponent, so a
        // non-zero input's root is normal. Retain fdlibm's scaling behavior
        // for completeness at the remote underflow edge.
        z * 2.0_f64.powi(n)
    } else {
        with_high(z, high(z).wrapping_add(n << 20))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_pow_matches_openjdk_grade_residual() {
        let product = f64::from_bits(0x3fb5_df43_e2da_3d16);
        assert_eq!(
            java_positive_pow(product, 1.0 / 13.0).to_bits(),
            0x3fea_7bae_55ad_bec6
        );
        let close_to_one = f64::from_bits(0x3fec_4c4c_9529_7916);
        assert_eq!(
            java_positive_pow(close_to_one, 1.0 / 13.0).to_bits(),
            0x3fef_b2e4_5c6d_eb6f
        );
        assert_eq!(java_positive_pow(0.25, 2.0), 0.0625);
        assert_eq!(java_positive_pow(0.0, 1.0 / 13.0), 0.0);
        assert!(java_positive_pow(f64::NAN, 1.0 / 13.0).is_nan());
    }
}
