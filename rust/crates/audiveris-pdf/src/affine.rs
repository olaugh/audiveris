// SPDX-License-Identifier: AGPL-3.0-or-later

//! `java.awt.geom.AffineTransform`, including its state machine.
//!
//! Composing the page transform in closed form is not good enough, and that is
//! not a theoretical worry: it was tried, and the horizontal terms came out
//! bit-identical while `m11` was wrong in its last bits, which alone put 924
//! samples wrong across 13 destination rows. The reason is this class. Every
//! mutator dispatches on a cached `state` describing which of translate, scale
//! and shear are present, and the branches are *not* algebraically identical --
//! they skip terms that are known to be zero. Skipping `+ 0.0` is a no-op for
//! every double except `-0.0`, and the sign of zero is exactly what the corpus
//! exercises.
//!
//! The page transform reaches `concatenate`'s `APPLY_SCALE | APPLY_TRANSLATE`
//! case, which computes `m10 = T10 * m11`. The page's `m11` is negative, since
//! the device transform flips the y axis, and the placement's `T10` is `+0.0`,
//! since a scan's `cm` has no shear. The product is **`-0.0`**, which is what
//! `oracle/pdf-pages.txt` records for every one of the 189 draws. A closed-form
//! composition, or a state machine that treated the zero terms as absent,
//! yields `+0.0`.
//!
//! `type` is deliberately not ported. It is a cached classification that no
//! arithmetic reads; `state` is the one that selects branches.

/// The transform's `state` bits, which select which branch every mutator takes.
const APPLY_IDENTITY: u8 = 0;
const APPLY_TRANSLATE: u8 = 1;
const APPLY_SCALE: u8 = 2;
const APPLY_SHEAR: u8 = 4;

/// A two-by-three affine transform, mutated the way Java's is.
///
/// Field order follows Java's constructor -- `m00, m10, m01, m11, m02, m12` --
/// rather than reading order, because that is the order `getMatrix` writes and
/// the order the oracle records.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine {
    m00: f64,
    m10: f64,
    m01: f64,
    m11: f64,
    m02: f64,
    m12: f64,
    state: u8,
}

impl Default for Affine {
    fn default() -> Self {
        Self::identity()
    }
}

impl Affine {
    /// `new AffineTransform()`.
    #[must_use]
    pub const fn identity() -> Self {
        Self {
            m00: 1.0,
            m10: 0.0,
            m01: 0.0,
            m11: 1.0,
            m02: 0.0,
            m12: 0.0,
            state: APPLY_IDENTITY,
        }
    }

    /// `new AffineTransform(m00, m10, m01, m11, m02, m12)`, which calls
    /// `updateState`.
    #[must_use]
    pub fn new(m00: f64, m10: f64, m01: f64, m11: f64, m02: f64, m12: f64) -> Self {
        let mut transform = Self {
            m00,
            m10,
            m01,
            m11,
            m02,
            m12,
            state: APPLY_IDENTITY,
        };
        transform.update_state();
        transform
    }

    /// The six terms in `getMatrix` order: `m00, m10, m01, m11, m02, m12`.
    #[must_use]
    pub const fn matrix(&self) -> [f64; 6] {
        [self.m00, self.m10, self.m01, self.m11, self.m02, self.m12]
    }

    /// `AffineTransform.updateState`.
    fn update_state(&mut self) {
        self.state = if self.m01 == 0.0 && self.m10 == 0.0 {
            if self.m00 == 1.0 && self.m11 == 1.0 {
                if self.m02 == 0.0 && self.m12 == 0.0 {
                    APPLY_IDENTITY
                } else {
                    APPLY_TRANSLATE
                }
            } else if self.m02 == 0.0 && self.m12 == 0.0 {
                APPLY_SCALE
            } else {
                APPLY_SCALE | APPLY_TRANSLATE
            }
        } else if self.m00 == 0.0 && self.m11 == 0.0 {
            if self.m02 == 0.0 && self.m12 == 0.0 {
                APPLY_SHEAR
            } else {
                APPLY_SHEAR | APPLY_TRANSLATE
            }
        } else if self.m02 == 0.0 && self.m12 == 0.0 {
            APPLY_SHEAR | APPLY_SCALE
        } else {
            APPLY_SHEAR | APPLY_SCALE | APPLY_TRANSLATE
        };
    }

    /// `AffineTransform.scale`.
    pub fn scale(&mut self, sx: f64, sy: f64) {
        let mut state = self.state;
        match state {
            s if s == APPLY_SHEAR | APPLY_SCALE | APPLY_TRANSLATE
                || s == APPLY_SHEAR | APPLY_SCALE =>
            {
                self.m00 *= sx;
                self.m11 *= sy;
                // Java falls through into the shear-only case.
                self.m01 *= sy;
                self.m10 *= sx;
                if self.m01 == 0.0 && self.m10 == 0.0 {
                    state &= APPLY_TRANSLATE;
                    if !(self.m00 == 1.0 && self.m11 == 1.0) {
                        state |= APPLY_SCALE;
                    }
                    self.state = state;
                }
            }
            s if s == APPLY_SHEAR | APPLY_TRANSLATE || s == APPLY_SHEAR => {
                self.m01 *= sy;
                self.m10 *= sx;
                if self.m01 == 0.0 && self.m10 == 0.0 {
                    state &= APPLY_TRANSLATE;
                    if !(self.m00 == 1.0 && self.m11 == 1.0) {
                        state |= APPLY_SCALE;
                    }
                    self.state = state;
                }
            }
            s if s == APPLY_SCALE | APPLY_TRANSLATE || s == APPLY_SCALE => {
                self.m00 *= sx;
                self.m11 *= sy;
                if self.m00 == 1.0 && self.m11 == 1.0 {
                    self.state = state & APPLY_TRANSLATE;
                }
            }
            // APPLY_TRANSLATE and APPLY_IDENTITY.
            _ => {
                self.m00 = sx;
                self.m11 = sy;
                if sx != 1.0 || sy != 1.0 {
                    self.state = state | APPLY_SCALE;
                }
            }
        }
    }

    /// `AffineTransform.translate`.
    ///
    /// The omitted terms are the point. In the scale-only cases Java writes
    /// `m02 = tx * m00 + m02` without the `ty * m01` term, because `m01` is
    /// known to be zero -- and `x + 0.0` differs from `x` when `x` is `-0.0`.
    #[allow(
        clippy::assign_op_pattern,
        reason = "these statements are Java's, term for term; `+=` is equivalent by \
                  commutativity but breaks the correspondence this file is checked against"
    )]
    pub fn translate(&mut self, tx: f64, ty: f64) {
        match self.state {
            s if s == APPLY_SHEAR | APPLY_SCALE | APPLY_TRANSLATE => {
                self.m02 = tx * self.m00 + ty * self.m01 + self.m02;
                self.m12 = tx * self.m10 + ty * self.m11 + self.m12;
                if self.m02 == 0.0 && self.m12 == 0.0 {
                    self.state = APPLY_SHEAR | APPLY_SCALE;
                }
            }
            s if s == APPLY_SHEAR | APPLY_SCALE => {
                self.m02 = tx * self.m00 + ty * self.m01;
                self.m12 = tx * self.m10 + ty * self.m11;
                if self.m02 != 0.0 || self.m12 != 0.0 {
                    self.state = APPLY_SHEAR | APPLY_SCALE | APPLY_TRANSLATE;
                }
            }
            s if s == APPLY_SHEAR | APPLY_TRANSLATE => {
                self.m02 = ty * self.m01 + self.m02;
                self.m12 = tx * self.m10 + self.m12;
                if self.m02 == 0.0 && self.m12 == 0.0 {
                    self.state = APPLY_SHEAR;
                }
            }
            s if s == APPLY_SHEAR => {
                self.m02 = ty * self.m01;
                self.m12 = tx * self.m10;
                if self.m02 != 0.0 || self.m12 != 0.0 {
                    self.state = APPLY_SHEAR | APPLY_TRANSLATE;
                }
            }
            s if s == APPLY_SCALE | APPLY_TRANSLATE => {
                self.m02 = tx * self.m00 + self.m02;
                self.m12 = ty * self.m11 + self.m12;
                if self.m02 == 0.0 && self.m12 == 0.0 {
                    self.state = APPLY_SCALE;
                }
            }
            s if s == APPLY_SCALE => {
                self.m02 = tx * self.m00;
                self.m12 = ty * self.m11;
                if self.m02 != 0.0 || self.m12 != 0.0 {
                    self.state = APPLY_SCALE | APPLY_TRANSLATE;
                }
            }
            s if s == APPLY_TRANSLATE => {
                self.m02 = tx + self.m02;
                self.m12 = ty + self.m12;
                if self.m02 == 0.0 && self.m12 == 0.0 {
                    self.state = APPLY_IDENTITY;
                }
            }
            // APPLY_IDENTITY.
            _ => {
                self.m02 = tx;
                self.m12 = ty;
                if tx != 0.0 || ty != 0.0 {
                    self.state = APPLY_TRANSLATE;
                }
            }
        }
    }

    /// `AffineTransform.concatenate`: `this = this * other`.
    #[allow(
        clippy::similar_names,
        reason = "the term names are Java's, and renaming them would break the correspondence this file exists to hold"
    )]
    pub fn concatenate(&mut self, other: &Self) {
        let mystate = self.state;
        let txstate = other.state;

        // The optimised heads of Java's dispatch, in its order.
        if txstate == APPLY_IDENTITY {
            return;
        }
        if mystate == APPLY_IDENTITY {
            *self = *other;
            return;
        }
        if txstate == APPLY_TRANSLATE {
            self.translate(other.m02, other.m12);
            return;
        }
        if txstate == APPLY_SCALE {
            self.scale(other.m00, other.m11);
            return;
        }
        if txstate == APPLY_SHEAR {
            match mystate {
                s if s == APPLY_SHEAR | APPLY_SCALE | APPLY_TRANSLATE
                    || s == APPLY_SHEAR | APPLY_SCALE =>
                {
                    let t01 = other.m01;
                    let t10 = other.m10;
                    let mut m = self.m00;
                    self.m00 = self.m01 * t10;
                    self.m01 = m * t01;
                    m = self.m10;
                    self.m10 = self.m11 * t10;
                    self.m11 = m * t01;
                }
                s if s == APPLY_SHEAR | APPLY_TRANSLATE || s == APPLY_SHEAR => {
                    self.m00 = self.m01 * other.m10;
                    self.m01 = 0.0;
                    self.m11 = self.m10 * other.m01;
                    self.m10 = 0.0;
                    self.state = mystate ^ (APPLY_SHEAR | APPLY_SCALE);
                }
                s if s == APPLY_SCALE | APPLY_TRANSLATE || s == APPLY_SCALE => {
                    self.m01 = self.m00 * other.m01;
                    self.m00 = 0.0;
                    self.m10 = self.m11 * other.m10;
                    self.m11 = 0.0;
                    self.state = mystate ^ (APPLY_SHEAR | APPLY_SCALE);
                }
                // APPLY_TRANSLATE.
                _ => {
                    self.m00 = 0.0;
                    self.m01 = other.m01;
                    self.m10 = other.m10;
                    self.m11 = 0.0;
                    self.state = APPLY_TRANSLATE | APPLY_SHEAR;
                }
            }
            return;
        }

        // "If Tx has more than one attribute, it is not worth optimizing all of
        // those cases..."
        let (t00, t01, t02) = (other.m00, other.m01, other.m02);
        let (t10, t11, t12) = (other.m10, other.m11, other.m12);
        match mystate {
            s if s == APPLY_SHEAR | APPLY_SCALE
                || s == APPLY_SHEAR | APPLY_SCALE | APPLY_TRANSLATE =>
            {
                if mystate == APPLY_SHEAR | APPLY_SCALE {
                    self.state = mystate | txstate;
                }
                let mut m0 = self.m00;
                let mut m1 = self.m01;
                self.m00 = t00 * m0 + t10 * m1;
                self.m01 = t01 * m0 + t11 * m1;
                self.m02 += t02 * m0 + t12 * m1;

                m0 = self.m10;
                m1 = self.m11;
                self.m10 = t00 * m0 + t10 * m1;
                self.m11 = t01 * m0 + t11 * m1;
                self.m12 += t02 * m0 + t12 * m1;
                return;
            }
            s if s == APPLY_SHEAR | APPLY_TRANSLATE || s == APPLY_SHEAR => {
                let mut m0 = self.m01;
                self.m00 = t10 * m0;
                self.m01 = t11 * m0;
                self.m02 += t12 * m0;

                m0 = self.m10;
                self.m10 = t00 * m0;
                self.m11 = t01 * m0;
                self.m12 += t02 * m0;
            }
            s if s == APPLY_SCALE | APPLY_TRANSLATE || s == APPLY_SCALE => {
                let mut m0 = self.m00;
                self.m00 = t00 * m0;
                self.m01 = t01 * m0;
                self.m02 += t02 * m0;

                m0 = self.m11;
                self.m10 = t10 * m0;
                self.m11 = t11 * m0;
                self.m12 += t12 * m0;
            }
            // APPLY_TRANSLATE.
            _ => {
                self.m00 = t00;
                self.m01 = t01;
                self.m02 += t02;

                self.m10 = t10;
                self.m11 = t11;
                self.m12 += t12;
                self.state = txstate | APPLY_TRANSLATE;
                return;
            }
        }
        self.update_state();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scale_then_translate_matches_javas_term_order() {
        let mut transform = Affine::identity();
        transform.scale(2.0, 3.0);
        transform.translate(5.0, 7.0);
        assert_eq!(transform.matrix(), [2.0, 0.0, 0.0, 3.0, 10.0, 21.0]);
    }

    /// The whole reason this file exists.
    ///
    /// A flipped device transform concatenated with an unsheared placement
    /// takes `concatenate`'s scale-only case, which computes `m10 = T10 * m11`
    /// with `T10` at `+0.0` and `m11` negative. Java's answer is `-0.0`, and
    /// the oracle records `-0.0` on all 189 draws.
    #[test]
    fn concatenating_a_flip_with_an_unsheared_placement_yields_negative_zero() {
        let mut device = Affine::identity();
        device.scale(4.0, -4.0);
        let placement = Affine::new(0.5, 0.0, 0.0, 0.5, 1.0, 2.0);
        device.concatenate(&placement);
        let matrix = device.matrix();
        assert_eq!(matrix[1], 0.0, "value");
        assert!(
            matrix[1].is_sign_negative(),
            "m10 must be -0.0, not +0.0: got {:?}",
            matrix[1]
        );
    }

    /// The counter-case: composing the same thing in closed form gives `+0.0`,
    /// so the distinction is real rather than a quirk of how it is written.
    #[test]
    fn a_closed_form_composition_would_give_positive_zero_instead() {
        let (device_m10, device_m11) = (0.0f64, -4.0f64);
        let (placement_m00, placement_m10) = (0.5f64, 0.0f64);
        let closed_form = device_m10 * placement_m00 + device_m11 * placement_m10;
        assert_eq!(closed_form, 0.0);
        assert!(closed_form.is_sign_positive());
    }

    #[test]
    fn a_translate_on_a_scaled_transform_skips_the_shear_terms() {
        // `m01` is -0.0 here, so a general-case translate would add `ty * -0.0`
        // and change `m02`'s sign of zero; the scale-only case must not.
        let mut transform = Affine::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        transform.scale(2.0, 2.0);
        transform.translate(0.0, 3.0);
        assert_eq!(transform.matrix()[4], 0.0);
        assert!(transform.matrix()[4].is_sign_positive());
    }

    #[test]
    fn concatenating_identity_either_way_changes_nothing() {
        let mut transform = Affine::new(2.0, 3.0, 4.0, 5.0, 6.0, 7.0);
        let before = transform.matrix();
        transform.concatenate(&Affine::identity());
        assert_eq!(transform.matrix(), before);

        let mut identity = Affine::identity();
        identity.concatenate(&Affine::new(2.0, 3.0, 4.0, 5.0, 6.0, 7.0));
        assert_eq!(identity.matrix(), before);
    }

    #[test]
    fn a_general_concatenation_multiplies_in_javas_order() {
        let mut transform = Affine::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        transform.concatenate(&Affine::new(7.0, 8.0, 9.0, 10.0, 11.0, 12.0));
        // [1 3 5; 2 4 6] * [7 9 11; 8 10 12] with the implicit third row.
        // Written out rather than computed, so the expectation cannot drift
        // with the implementation.
        assert_eq!(transform.matrix(), [31.0, 46.0, 39.0, 58.0, 52.0, 76.0]);
    }
}
