// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light value semantics of Java `grid.StaffPeak`.
//!
//! This layer preserves peak geometry, ordering, equality, deskewed-center
//! calculation, and attribute transitions. Filament, SIG `Inter`, serif, and
//! `BarColumn` ownership are intentionally left to later stable-ID adapters.

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::{error::Error, fmt};

use audiveris_core::grade::clamp;

use crate::bar_alignment::VerticalSide;
use crate::bar_column::StaffId;

const INTRINSIC_RATIO: f64 = 0.8;

/// Java `AbstractStaffVerticalInter.Impacts`, retained on accepted projection
/// peaks before any SIG `Inter` exists.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaffVerticalImpacts {
    core: f64,
    gap: f64,
    start: f64,
    stop: f64,
    left: f64,
    right: f64,
    grade: f64,
}

impl StaffVerticalImpacts {
    #[must_use]
    pub fn new(core: f64, gap: f64, start: f64, stop: f64, left: f64, right: f64) -> Self {
        let impacts = [
            clamp(core),
            clamp(gap),
            clamp(start),
            clamp(stop),
            clamp(left),
            clamp(right),
        ];
        let grade = weighted_grade(&impacts, &[1.0, 1.0, 1.0, 1.0, 0.5, 0.5]);
        Self {
            core: impacts[0],
            gap: impacts[1],
            start: impacts[2],
            stop: impacts[3],
            left: impacts[4],
            right: impacts[5],
            grade,
        }
    }

    #[must_use]
    pub const fn core(self) -> f64 {
        self.core
    }

    #[must_use]
    pub const fn gap(self) -> f64 {
        self.gap
    }

    #[must_use]
    pub const fn start(self) -> f64 {
        self.start
    }

    #[must_use]
    pub const fn stop(self) -> f64 {
        self.stop
    }

    #[must_use]
    pub const fn left(self) -> f64 {
        self.left
    }

    #[must_use]
    pub const fn right(self) -> f64 {
        self.right
    }

    #[must_use]
    pub const fn grade(self) -> f64 {
        self.grade
    }
}

fn weighted_grade(impacts: &[f64], weights: &[f64]) -> f64 {
    let mut global = 1.0;
    let mut total_weight = 0.0;
    for (&impact, &weight) in impacts.iter().zip(weights) {
        total_weight += weight;
        if impact == 0.0 {
            global = 0.0;
        } else if weight != 0.0 {
            global *= impact.powf(weight);
        }
    }
    INTRINSIC_RATIO * global.powf(1.0 / total_weight)
}

/// Java-compatible integer rectangle, including inclusive peak dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeakBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PeakPoint {
    pub x: f64,
    pub y: f64,
}

impl PeakPoint {
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HorizontalSide {
    Left,
    Right,
}

/// Immutable key used by Java `StaffPeak.compareTo` and `equals`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StaffPeakKey {
    staff_id: StaffId,
    start: i32,
    stop: i32,
}

impl PartialOrd for StaffPeakKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for StaffPeakKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.staff_id
            .value()
            .cmp(&other.staff_id.value())
            .then_with(|| self.start.cmp(&other.start))
            .then_with(|| self.stop.cmp(&other.stop))
    }
}

impl StaffPeakKey {
    #[must_use]
    pub const fn staff_id(self) -> StaffId {
        self.staff_id
    }

    #[must_use]
    pub const fn start(self) -> i32 {
        self.start
    }

    #[must_use]
    pub const fn stop(self) -> i32 {
        self.stop
    }
}

/// Declaration order matches Java `StaffPeak.Attribute` and therefore
/// `EnumSet` iteration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StaffPeakAttribute {
    Thin,
    Thick,
    StaffLeftEnd,
    StaffRightEnd,
    BracketTop,
    BracketMiddle,
    BracketBottom,
    CclefOne,
    CclefTwo,
    CclefTail,
    Brace,
    BraceTop,
    BraceMiddle,
    BraceBottom,
}

impl StaffPeakAttribute {
    const ALL: [Self; 14] = [
        Self::Thin,
        Self::Thick,
        Self::StaffLeftEnd,
        Self::StaffRightEnd,
        Self::BracketTop,
        Self::BracketMiddle,
        Self::BracketBottom,
        Self::CclefOne,
        Self::CclefTwo,
        Self::CclefTail,
        Self::Brace,
        Self::BraceTop,
        Self::BraceMiddle,
        Self::BraceBottom,
    ];

    const fn bit(self) -> u16 {
        1_u16 << self as u8
    }
}

#[derive(Clone, Debug)]
pub struct StaffPeak {
    staff_id: StaffId,
    top: i32,
    bottom: i32,
    start: i32,
    stop: i32,
    deskewed_center: Option<PeakPoint>,
    attributes: u16,
    impacts: Option<StaffVerticalImpacts>,
}

impl StaffPeak {
    pub fn new(
        staff_id: StaffId,
        top: i32,
        bottom: i32,
        start: i32,
        stop: i32,
    ) -> Result<Self, StaffPeakError> {
        if staff_id.value() == 0 {
            return Err(StaffPeakError::InvalidStaffId);
        }
        Ok(Self {
            staff_id,
            top,
            bottom,
            start,
            stop,
            deskewed_center: None,
            attributes: 0,
            impacts: None,
        })
    }

    pub fn with_impacts(
        staff_id: StaffId,
        top: i32,
        bottom: i32,
        start: i32,
        stop: i32,
        impacts: StaffVerticalImpacts,
    ) -> Result<Self, StaffPeakError> {
        let mut peak = Self::new(staff_id, top, bottom, start, stop)?;
        peak.impacts = Some(impacts);
        Ok(peak)
    }

    #[must_use]
    pub const fn staff_id(&self) -> StaffId {
        self.staff_id
    }

    #[must_use]
    pub const fn top(&self) -> i32 {
        self.top
    }

    #[must_use]
    pub const fn bottom(&self) -> i32 {
        self.bottom
    }

    #[must_use]
    pub const fn start(&self) -> i32 {
        self.start
    }

    #[must_use]
    pub const fn stop(&self) -> i32 {
        self.stop
    }

    /// Java's inclusive `stop - start + 1`, including `int` wrapping.
    #[must_use]
    pub const fn width(&self) -> i32 {
        self.stop.wrapping_sub(self.start).wrapping_add(1)
    }

    /// Java `Rectangle` construction, including `int` wrapping.
    #[must_use]
    pub const fn bounds(&self) -> PeakBounds {
        PeakBounds {
            x: self.start,
            y: self.top,
            width: self.width(),
            height: self.bottom.wrapping_sub(self.top).wrapping_add(1),
        }
    }

    #[must_use]
    pub const fn ordinate(&self, side: VerticalSide) -> i32 {
        match side {
            VerticalSide::Top => self.top,
            VerticalSide::Bottom => self.bottom,
        }
    }

    /// Compute Java's integer-sum midpoint and apply the sheet deskew transform.
    ///
    /// The callback keeps affine/skew ownership outside this graph-neutral type.
    /// Invalid transform output fails atomically instead of storing unusable state.
    pub fn compute_deskewed_center(
        &mut self,
        transform: impl FnOnce(PeakPoint) -> PeakPoint,
    ) -> Result<PeakPoint, StaffPeakError> {
        let midpoint = PeakPoint {
            x: f64::from(self.start.wrapping_add(self.stop)) / 2.0,
            y: f64::from(self.top.wrapping_add(self.bottom)) / 2.0,
        };
        let transformed = transform(midpoint);
        if !transformed.x.is_finite() || !transformed.y.is_finite() {
            return Err(StaffPeakError::NonFiniteDeskewedCenter);
        }
        self.deskewed_center = Some(transformed);
        Ok(transformed)
    }

    #[must_use]
    pub const fn deskewed_center(&self) -> Option<PeakPoint> {
        self.deskewed_center
    }

    #[must_use]
    pub const fn impacts(&self) -> Option<StaffVerticalImpacts> {
        self.impacts
    }

    #[must_use]
    pub fn deskewed_abscissa(&self) -> Option<f64> {
        self.deskewed_center.map(|point| point.x)
    }

    pub fn set(&mut self, attribute: StaffPeakAttribute) {
        self.attributes |= attribute.bit();
    }

    pub fn unset(&mut self, attribute: StaffPeakAttribute) {
        self.attributes &= !attribute.bit();
    }

    #[must_use]
    pub const fn is_set(&self, attribute: StaffPeakAttribute) -> bool {
        self.attributes & attribute.bit() != 0
    }

    /// Attributes in Java `EnumSet` declaration order.
    pub fn attributes(&self) -> impl Iterator<Item = StaffPeakAttribute> + '_ {
        StaffPeakAttribute::ALL
            .into_iter()
            .filter(|attribute| self.is_set(*attribute))
    }

    #[must_use]
    pub const fn is_brace(&self) -> bool {
        self.is_set(StaffPeakAttribute::Brace)
            || self.is_set(StaffPeakAttribute::BraceTop)
            || self.is_set(StaffPeakAttribute::BraceMiddle)
            || self.is_set(StaffPeakAttribute::BraceBottom)
    }

    #[must_use]
    pub const fn is_brace_end(&self, side: VerticalSide) -> bool {
        self.is_set(match side {
            VerticalSide::Top => StaffPeakAttribute::BraceTop,
            VerticalSide::Bottom => StaffPeakAttribute::BraceBottom,
        })
    }

    #[must_use]
    pub const fn is_bracket(&self) -> bool {
        self.is_set(StaffPeakAttribute::BracketTop)
            || self.is_set(StaffPeakAttribute::BracketMiddle)
            || self.is_set(StaffPeakAttribute::BracketBottom)
    }

    #[must_use]
    pub const fn is_bracket_end(&self, side: VerticalSide) -> bool {
        self.is_set(match side {
            VerticalSide::Top => StaffPeakAttribute::BracketTop,
            VerticalSide::Bottom => StaffPeakAttribute::BracketBottom,
        })
    }

    pub fn set_bracket_end(&mut self, side: VerticalSide) {
        self.set(match side {
            VerticalSide::Top => StaffPeakAttribute::BracketTop,
            VerticalSide::Bottom => StaffPeakAttribute::BracketBottom,
        });
    }

    #[must_use]
    pub const fn is_staff_end(&self, side: HorizontalSide) -> bool {
        self.is_set(match side {
            HorizontalSide::Left => StaffPeakAttribute::StaffLeftEnd,
            HorizontalSide::Right => StaffPeakAttribute::StaffRightEnd,
        })
    }

    pub fn set_staff_end(&mut self, side: HorizontalSide) {
        self.set(match side {
            HorizontalSide::Left => StaffPeakAttribute::StaffLeftEnd,
            HorizontalSide::Right => StaffPeakAttribute::StaffRightEnd,
        });
    }

    #[must_use]
    pub const fn key(&self) -> StaffPeakKey {
        StaffPeakKey {
            staff_id: self.staff_id,
            start: self.start,
            stop: self.stop,
        }
    }
}

impl PartialEq for StaffPeak {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl Eq for StaffPeak {}

impl PartialOrd for StaffPeak {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for StaffPeak {
    fn cmp(&self, other: &Self) -> Ordering {
        self.staff_id
            .value()
            .cmp(&other.staff_id.value())
            .then_with(|| self.start.cmp(&other.start))
            .then_with(|| self.stop.cmp(&other.stop))
    }
}

impl Hash for StaffPeak {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Java hashes staff and start only; stop differences intentionally
        // collide even though compareTo uses stop as its final tie-breaker.
        self.staff_id.hash(state);
        self.start.hash(state);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaffPeakError {
    InvalidStaffId,
    NonFiniteDeskewedCenter,
}

impl fmt::Display for StaffPeakError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStaffId => formatter.write_str("staff ID must be positive"),
            Self::NonFiniteDeskewedCenter => {
                formatter.write_str("deskewed peak center must be finite")
            }
        }
    }
}

impl Error for StaffPeakError {}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, hash_map::DefaultHasher};

    use super::*;

    fn peak(staff: usize, top: i32, bottom: i32, start: i32, stop: i32) -> StaffPeak {
        StaffPeak::new(StaffId::new(staff), top, bottom, start, stop).unwrap()
    }

    fn hash(peak: &StaffPeak) -> u64 {
        let mut state = DefaultHasher::new();
        peak.hash(&mut state);
        state.finish()
    }

    #[test]
    fn inclusive_geometry_and_side_access_match_java() {
        let value = peak(2, 20, 28, 10, 12);
        assert_eq!(value.width(), 3);
        assert_eq!(
            value.bounds(),
            PeakBounds {
                x: 10,
                y: 20,
                width: 3,
                height: 9,
            }
        );
        assert_eq!(value.ordinate(VerticalSide::Top), 20);
        assert_eq!(value.ordinate(VerticalSide::Bottom), 28);

        let singleton = peak(2, 5, 5, 7, 7);
        assert_eq!(singleton.width(), 1);
        assert_eq!(singleton.bounds().height, 1);
    }

    #[test]
    fn staff_vertical_impacts_clamp_and_use_java_weights() {
        let impacts = StaffVerticalImpacts::new(0.5, 1.2, 1.0, 1.0, 0.25, -0.5);
        assert_eq!(impacts.core(), 0.5);
        assert_eq!(impacts.gap(), 1.0);
        assert_eq!(impacts.start(), 1.0);
        assert_eq!(impacts.stop(), 1.0);
        assert_eq!(impacts.left(), 0.25);
        assert_eq!(impacts.right(), 0.0);
        assert_eq!(impacts.grade(), 0.0);

        let impacts = StaffVerticalImpacts::new(0.5, 1.0, 1.0, 1.0, 0.25, 1.0);
        let expected = 0.8 * (0.5_f64 * 0.25_f64.sqrt()).powf(1.0 / 5.0);
        assert!((impacts.grade() - expected).abs() < 1.0e-14);

        let peak = StaffPeak::with_impacts(StaffId::new(2), 10, 20, 4, 5, impacts).unwrap();
        assert_eq!(peak.impacts(), Some(impacts));
        assert!(peak.attributes().next().is_none());
    }

    #[test]
    fn raw_reversed_and_overflowing_geometry_preserves_java_int_arithmetic() {
        let reversed = peak(1, 9, 3, 8, 4);
        assert_eq!(reversed.width(), -3);
        assert_eq!(reversed.bounds().height, -5);

        let overflow = peak(1, i32::MIN, i32::MAX, i32::MIN, i32::MAX);
        assert_eq!(overflow.width(), 0);
        assert_eq!(overflow.bounds().height, 0);
    }

    #[test]
    fn ordering_and_identity_use_only_staff_start_and_stop() {
        let mut changed_geometry = peak(1, 100, 200, 10, 12);
        changed_geometry.set(StaffPeakAttribute::Brace);
        let same_key = peak(1, 1, 2, 10, 12);
        assert_eq!(changed_geometry, same_key);
        assert_eq!(hash(&changed_geometry), hash(&same_key));

        let same_hash_different_stop = peak(1, 1, 2, 10, 13);
        assert_ne!(same_key, same_hash_different_stop);
        assert_eq!(hash(&same_key), hash(&same_hash_different_stop));

        let values = BTreeSet::from([
            peak(2, 0, 1, -5, -4),
            peak(1, 0, 1, 10, 13),
            peak(1, 0, 1, 9, 20),
            peak(1, 0, 1, 10, 12),
        ]);
        assert_eq!(
            values
                .iter()
                .map(|peak| (peak.staff_id().value(), peak.start(), peak.stop()))
                .collect::<Vec<_>>(),
            [(1, 9, 20), (1, 10, 12), (1, 10, 13), (2, -5, -4)]
        );
    }

    #[test]
    fn deskew_transform_uses_java_integer_sum_midpoint_and_is_atomic() {
        let mut peak = peak(1, 20, 25, 10, 13);
        let transformed = peak
            .compute_deskewed_center(|point| PeakPoint::new(point.x + 2.0, point.y - 3.0))
            .unwrap();
        assert_eq!(transformed, PeakPoint::new(13.5, 19.5));
        assert_eq!(peak.deskewed_center(), Some(transformed));
        assert_eq!(peak.deskewed_abscissa(), Some(13.5));

        assert_eq!(
            peak.compute_deskewed_center(|_| PeakPoint::new(f64::NAN, 0.0)),
            Err(StaffPeakError::NonFiniteDeskewedCenter)
        );
        assert_eq!(peak.deskewed_center(), Some(transformed));
    }

    #[test]
    fn attribute_groups_and_directional_helpers_match_java() {
        let mut peak = peak(1, 0, 4, 10, 10);
        peak.set(StaffPeakAttribute::Thick);
        peak.set(StaffPeakAttribute::BraceMiddle);
        peak.set_bracket_end(VerticalSide::Top);
        peak.set_staff_end(HorizontalSide::Right);

        assert!(peak.is_brace());
        assert!(!peak.is_brace_end(VerticalSide::Top));
        assert!(peak.is_bracket());
        assert!(peak.is_bracket_end(VerticalSide::Top));
        assert!(!peak.is_bracket_end(VerticalSide::Bottom));
        assert!(!peak.is_staff_end(HorizontalSide::Left));
        assert!(peak.is_staff_end(HorizontalSide::Right));
        assert_eq!(
            peak.attributes().collect::<Vec<_>>(),
            [
                StaffPeakAttribute::Thick,
                StaffPeakAttribute::StaffRightEnd,
                StaffPeakAttribute::BracketTop,
                StaffPeakAttribute::BraceMiddle,
            ]
        );

        peak.unset(StaffPeakAttribute::BraceMiddle);
        peak.unset(StaffPeakAttribute::BracketTop);
        assert!(!peak.is_brace());
        assert!(!peak.is_bracket());
    }

    #[test]
    fn rejects_zero_staff_identity() {
        assert_eq!(
            StaffPeak::new(StaffId::new(0), 0, 1, 0, 1),
            Err(StaffPeakError::InvalidStaffId)
        );
    }
}
