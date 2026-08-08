// SPDX-License-Identifier: AGPL-3.0-or-later

//! Stable-ID selection semantics from Java `BarAlignment` and `BarConnection`.
//!
//! Peak geometry discovery, connection areas, PeakGraph, SIG, and UI ownership
//! are intentionally outside this neutral value layer.

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::{error::Error, fmt};

use audiveris_core::{grade::clamp, java_math::java_positive_pow};

use crate::bar_column::{PeakId, StaffId};

const INTRINSIC_RATIO: f64 = 0.8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AlignmentPeak {
    id: PeakId,
    staff_id: StaffId,
    start: i32,
    grade: f64,
    geometry: Option<PeakGeometry>,
}

/// Integer extrema consumed by Java `BarConnection.getMedian/getWidth`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeakGeometry {
    width: usize,
    top: i32,
    bottom: i32,
}

impl PeakGeometry {
    #[must_use]
    pub const fn width(self) -> usize {
        self.width
    }

    #[must_use]
    pub const fn top(self) -> i32 {
        self.top
    }

    #[must_use]
    pub const fn bottom(self) -> i32 {
        self.bottom
    }
}

/// Rather-vertical connector median between the two peak extrema.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConnectionMedian {
    pub top: (f64, f64),
    pub bottom: (f64, f64),
}

impl AlignmentPeak {
    pub fn new(
        id: PeakId,
        staff_id: StaffId,
        start: i32,
        grade: f64,
    ) -> Result<Self, BarAlignmentError> {
        if id.value() == 0 {
            return Err(BarAlignmentError::InvalidPeakId);
        }
        if staff_id.value() == 0 {
            return Err(BarAlignmentError::InvalidStaffId);
        }
        if !grade.is_finite() || !(0.0..=1.0).contains(&grade) {
            return Err(BarAlignmentError::InvalidPeakGrade(id));
        }
        Ok(Self {
            id,
            staff_id,
            start,
            grade,
            geometry: None,
        })
    }

    /// Construct a peak with geometry needed by connection width and median.
    pub fn with_geometry(
        id: PeakId,
        staff_id: StaffId,
        start: i32,
        width: usize,
        top: i32,
        bottom: i32,
        grade: f64,
    ) -> Result<Self, BarAlignmentError> {
        let mut peak = Self::new(id, staff_id, start, grade)?;
        if width == 0 || top > bottom {
            return Err(BarAlignmentError::InvalidPeakGeometry(id));
        }
        peak.geometry = Some(PeakGeometry { width, top, bottom });
        Ok(peak)
    }

    #[must_use]
    pub const fn id(self) -> PeakId {
        self.id
    }

    #[must_use]
    pub const fn staff_id(self) -> StaffId {
        self.staff_id
    }

    #[must_use]
    pub const fn start(self) -> i32 {
        self.start
    }

    #[must_use]
    pub const fn grade(self) -> f64 {
        self.grade
    }

    #[must_use]
    pub const fn geometry(self) -> Option<PeakGeometry> {
        self.geometry
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BarAlignmentKind {
    Alignment,
    Connection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerticalSide {
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BarImpacts {
    Alignment {
        align: f64,
        width: f64,
    },
    Connection {
        align: f64,
        width: f64,
        gap: f64,
        white: f64,
    },
}

impl BarImpacts {
    pub fn alignment(align: f64, width: f64) -> Result<Self, BarAlignmentError> {
        validate_impacts(&[align, width])?;
        Ok(Self::Alignment {
            align: clamp(align),
            width: clamp(width),
        })
    }

    pub fn connection(
        align: f64,
        width: f64,
        gap: f64,
        white: f64,
    ) -> Result<Self, BarAlignmentError> {
        validate_impacts(&[align, width, gap, white])?;
        Ok(Self::Connection {
            align: clamp(align),
            width: clamp(width),
            gap: clamp(gap),
            white: clamp(white),
        })
    }

    #[must_use]
    pub fn grade(self) -> f64 {
        match self {
            Self::Alignment { align, width } => weighted_grade(&[align, width], &[2.0, 1.0]),
            Self::Connection {
                align,
                width,
                gap,
                white,
            } => weighted_grade(&[align, width, gap, white], &[2.0, 1.0, 2.0, 2.0]),
        }
    }

    #[must_use]
    pub const fn align(self) -> f64 {
        match self {
            Self::Alignment { align, .. } | Self::Connection { align, .. } => align,
        }
    }

    #[must_use]
    pub const fn width(self) -> f64 {
        match self {
            Self::Alignment { width, .. } | Self::Connection { width, .. } => width,
        }
    }
}

/// Alignment identity is its typed pair of stable peak IDs.
#[derive(Clone, Copy, Debug)]
pub struct BarAlignment {
    top: AlignmentPeak,
    bottom: AlignmentPeak,
    slope: f64,
    delta_width: f64,
    impacts: BarImpacts,
    grade: f64,
    kind: BarAlignmentKind,
}

impl BarAlignment {
    pub fn new(
        top: AlignmentPeak,
        bottom: AlignmentPeak,
        slope: f64,
        delta_width: f64,
        impacts: BarImpacts,
    ) -> Result<Self, BarAlignmentError> {
        if top.id == bottom.id || top.staff_id == bottom.staff_id {
            return Err(BarAlignmentError::InvalidPeakPair);
        }
        if !slope.is_finite() || !delta_width.is_finite() {
            return Err(BarAlignmentError::InvalidGeometry);
        }
        if !matches!(impacts, BarImpacts::Alignment { .. }) {
            return Err(BarAlignmentError::WrongImpactType);
        }
        Ok(Self {
            top,
            bottom,
            slope,
            delta_width,
            grade: impacts.grade(),
            impacts,
            kind: BarAlignmentKind::Alignment,
        })
    }

    /// Java `BarConnection` construction from an underlying alignment.
    pub fn connection(
        alignment: &Self,
        gap_impact: f64,
        white_impact: f64,
    ) -> Result<Self, BarAlignmentError> {
        if alignment.kind != BarAlignmentKind::Alignment {
            return Err(BarAlignmentError::WrongRelationType);
        }
        let impacts = BarImpacts::connection(
            alignment.impacts.align(),
            alignment.impacts.width(),
            gap_impact,
            white_impact,
        )?;
        Ok(Self {
            top: alignment.top,
            bottom: alignment.bottom,
            slope: alignment.slope,
            delta_width: alignment.delta_width,
            grade: impacts.grade(),
            impacts,
            kind: BarAlignmentKind::Connection,
        })
    }

    #[must_use]
    pub const fn top(&self) -> AlignmentPeak {
        self.top
    }

    #[must_use]
    pub const fn bottom(&self) -> AlignmentPeak {
        self.bottom
    }

    #[must_use]
    pub const fn peak(&self, side: VerticalSide) -> AlignmentPeak {
        match side {
            VerticalSide::Top => self.top,
            VerticalSide::Bottom => self.bottom,
        }
    }

    #[must_use]
    pub const fn slope(&self) -> f64 {
        self.slope
    }

    #[must_use]
    pub const fn delta_width(&self) -> f64 {
        self.delta_width
    }

    #[must_use]
    pub const fn impacts(&self) -> BarImpacts {
        self.impacts
    }

    #[must_use]
    pub const fn grade(&self) -> f64 {
        self.grade
    }

    #[must_use]
    pub const fn kind(&self) -> BarAlignmentKind {
        self.kind
    }

    /// Java `BarConnection.getWidth`.
    pub fn connection_width(&self) -> Result<f64, BarAlignmentError> {
        if self.kind != BarAlignmentKind::Connection {
            return Err(BarAlignmentError::WrongRelationType);
        }
        let top = self
            .top
            .geometry
            .ok_or(BarAlignmentError::MissingPeakGeometry(self.top.id))?;
        let bottom = self
            .bottom
            .geometry
            .ok_or(BarAlignmentError::MissingPeakGeometry(self.bottom.id))?;
        Ok((top.width as f64 + bottom.width as f64) / 2.0)
    }

    /// Java `BarConnection.getMedian` using resolved foreground thickness.
    ///
    /// The `+0.5` terms preserve Java's inside-extrema to pixel-center
    /// adjustment.
    pub fn connection_median(
        &self,
        foreground_thickness: usize,
    ) -> Result<ConnectionMedian, BarAlignmentError> {
        if self.kind != BarAlignmentKind::Connection {
            return Err(BarAlignmentError::WrongRelationType);
        }
        if foreground_thickness == 0 {
            return Err(BarAlignmentError::InvalidForegroundThickness);
        }
        let top = self
            .top
            .geometry
            .ok_or(BarAlignmentError::MissingPeakGeometry(self.top.id))?;
        let bottom = self
            .bottom
            .geometry
            .ok_or(BarAlignmentError::MissingPeakGeometry(self.bottom.id))?;
        let half_line = foreground_thickness as f64 / 2.0;
        Ok(ConnectionMedian {
            top: (
                self.top.start as f64 + top.width as f64 / 2.0,
                top.bottom as f64 + half_line + 0.5,
            ),
            bottom: (
                self.bottom.start as f64 + bottom.width as f64 / 2.0,
                bottom.top as f64 - half_line + 0.5,
            ),
        })
    }

    #[must_use]
    pub fn same_peaks(&self, other: &Self) -> bool {
        self.top.id == other.top.id && self.bottom.id == other.bottom.id
    }

    /// Java `compareTo`: top staff ID, then top peak start only.
    ///
    /// Distinct relations can compare equal, so this intentionally is not a
    /// Rust `Ord` implementation.
    #[must_use]
    pub fn source_cmp(&self, other: &Self) -> Ordering {
        self.top
            .staff_id
            .value()
            .cmp(&other.top.staff_id.value())
            .then_with(|| self.top.start.cmp(&other.top.start))
    }

    /// Java `bestOf`, preserving iteration order and strict tie behavior.
    #[must_use]
    pub fn best_of(alignments: &[Self], side: VerticalSide) -> Option<&Self> {
        let mut best = None;
        let mut best_contextual_grade = 0.0;

        for alignment in alignments {
            let contextual_grade = alignment.grade * alignment.peak(side).grade;
            match best {
                None => {
                    best = Some(alignment);
                    best_contextual_grade = contextual_grade;
                }
                Some(current) if current.kind == BarAlignmentKind::Connection => {
                    if alignment.kind == BarAlignmentKind::Connection
                        && contextual_grade > best_contextual_grade
                    {
                        best = Some(alignment);
                        best_contextual_grade = contextual_grade;
                    }
                }
                Some(_) => {
                    if alignment.kind == BarAlignmentKind::Connection
                        || contextual_grade > best_contextual_grade
                    {
                        best = Some(alignment);
                        best_contextual_grade = contextual_grade;
                    }
                }
            }
        }
        best
    }
}

impl PartialEq for BarAlignment {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.same_peaks(other)
    }
}

impl Eq for BarAlignment {}

impl Hash for BarAlignment {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
        self.top.id.hash(state);
        self.bottom.id.hash(state);
    }
}

fn validate_impacts(impacts: &[f64]) -> Result<(), BarAlignmentError> {
    if impacts.iter().any(|impact| !impact.is_finite()) {
        return Err(BarAlignmentError::NonFiniteImpact);
    }
    Ok(())
}

fn weighted_grade(impacts: &[f64], weights: &[f64]) -> f64 {
    let mut global = 1.0;
    let mut total_weight = 0.0;
    for (&impact, &weight) in impacts.iter().zip(weights) {
        total_weight += weight;
        if impact == 0.0 {
            global = 0.0;
        } else if weight != 0.0 {
            global *= java_positive_pow(impact, weight);
        }
    }
    INTRINSIC_RATIO * java_positive_pow(global, 1.0 / total_weight)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarAlignmentError {
    InvalidPeakId,
    InvalidStaffId,
    InvalidPeakGrade(PeakId),
    InvalidPeakGeometry(PeakId),
    MissingPeakGeometry(PeakId),
    InvalidForegroundThickness,
    InvalidPeakPair,
    InvalidGeometry,
    NonFiniteImpact,
    WrongImpactType,
    WrongRelationType,
}

impl fmt::Display for BarAlignmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPeakId => formatter.write_str("peak ID must be positive"),
            Self::InvalidStaffId => formatter.write_str("staff ID must be positive"),
            Self::InvalidPeakGrade(id) => {
                write!(
                    formatter,
                    "invalid contextual grade for peak {}",
                    id.value()
                )
            }
            Self::InvalidPeakGeometry(id) => {
                write!(formatter, "invalid geometry for peak {}", id.value())
            }
            Self::MissingPeakGeometry(id) => {
                write!(formatter, "missing geometry for peak {}", id.value())
            }
            Self::InvalidForegroundThickness => {
                formatter.write_str("foreground thickness must be positive")
            }
            Self::InvalidPeakPair => formatter.write_str("alignment peak pair is invalid"),
            Self::InvalidGeometry => formatter.write_str("alignment geometry is not finite"),
            Self::NonFiniteImpact => formatter.write_str("alignment impact is not finite"),
            Self::WrongImpactType => formatter.write_str("alignment impacts have the wrong type"),
            Self::WrongRelationType => {
                formatter.write_str("a connection requires an underlying alignment")
            }
        }
    }
}

impl Error for BarAlignmentError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn peak(id: usize, staff: usize, start: i32, grade: f64) -> AlignmentPeak {
        AlignmentPeak::new(PeakId::new(id), StaffId::new(staff), start, grade).unwrap()
    }

    fn geometric_peak(
        id: usize,
        staff: usize,
        start: i32,
        width: usize,
        top: i32,
        bottom: i32,
        grade: f64,
    ) -> AlignmentPeak {
        AlignmentPeak::with_geometry(
            PeakId::new(id),
            StaffId::new(staff),
            start,
            width,
            top,
            bottom,
            grade,
        )
        .unwrap()
    }

    fn alignment(
        id: usize,
        top_staff: usize,
        start: i32,
        impact: f64,
        top_grade: f64,
        bottom_grade: f64,
    ) -> BarAlignment {
        BarAlignment::new(
            peak(id, top_staff, start, top_grade),
            peak(id + 100, top_staff + 1, start, bottom_grade),
            0.0,
            0.0,
            BarImpacts::alignment(impact, impact).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn weighted_impacts_clamp_and_preserve_java_weights() {
        let impacts = BarImpacts::alignment(0.5, 1.0).unwrap();
        let expected = 0.8 * java_positive_pow(0.25, 1.0 / 3.0);
        assert_eq!(impacts.grade().to_bits(), expected.to_bits());

        let clamped = BarImpacts::connection(2.0, 1.0, -1.0, 1.0).unwrap();
        assert_eq!(clamped.grade(), 0.0);
        assert_eq!(
            BarImpacts::alignment(f64::NAN, 1.0),
            Err(BarAlignmentError::NonFiniteImpact)
        );
    }

    #[test]
    fn ordering_endpoints_and_typed_id_equality_match_source_contract() {
        let first = alignment(1, 2, 50, 1.0, 1.0, 1.0);
        let second = alignment(2, 1, 100, 1.0, 1.0, 1.0);
        let third = alignment(3, 1, 20, 1.0, 1.0, 1.0);
        let tied = alignment(4, 1, 20, 0.5, 1.0, 1.0);
        let mut values = [first, second, tied, third];
        values.sort_by(BarAlignment::source_cmp);
        assert_eq!(
            values.iter().map(|value| value.top.id).collect::<Vec<_>>(),
            [
                PeakId::new(4),
                PeakId::new(3),
                PeakId::new(2),
                PeakId::new(1)
            ]
        );
        assert_eq!(values[0].source_cmp(&values[1]), Ordering::Equal);

        let same_ids = BarAlignment::new(
            first.top,
            first.bottom,
            0.2,
            3.0,
            BarImpacts::alignment(0.2, 0.4).unwrap(),
        )
        .unwrap();
        assert_eq!(first, same_ids);
        let connection = BarAlignment::connection(&first, 1.0, 1.0).unwrap();
        assert!(first.same_peaks(&connection));
        assert_ne!(first, connection);
        assert_eq!(first.peak(VerticalSide::Top), first.top());
        assert_eq!(first.peak(VerticalSide::Bottom), first.bottom());
    }

    #[test]
    fn best_of_prefers_connections_then_strictly_higher_contextual_grade() {
        let strong_alignment = alignment(1, 1, 10, 1.0, 1.0, 1.0);
        let weak_base = alignment(2, 1, 20, 0.1, 1.0, 1.0);
        let weak_connection = BarAlignment::connection(&weak_base, 0.1, 0.1).unwrap();
        let later_alignment = alignment(3, 1, 30, 1.0, 1.0, 1.0);
        let stronger_base = alignment(4, 1, 40, 0.5, 1.0, 1.0);
        let stronger_connection = BarAlignment::connection(&stronger_base, 0.5, 0.5).unwrap();
        let equal_base = alignment(5, 1, 50, 0.5, 1.0, 1.0);
        let equal_connection = BarAlignment::connection(&equal_base, 0.5, 0.5).unwrap();
        let values = [
            strong_alignment,
            weak_connection,
            later_alignment,
            stronger_connection,
            equal_connection,
        ];

        assert_eq!(
            BarAlignment::best_of(&values, VerticalSide::Top),
            Some(&values[3])
        );
        assert_eq!(BarAlignment::best_of(&[], VerticalSide::Top), None);
    }

    #[test]
    fn best_of_uses_the_requested_partner_grade() {
        let top_favored = alignment(1, 1, 10, 1.0, 0.9, 0.1);
        let bottom_favored = alignment(2, 1, 20, 1.0, 0.2, 0.8);
        let values = [top_favored, bottom_favored];
        assert_eq!(
            BarAlignment::best_of(&values, VerticalSide::Top),
            Some(&values[0])
        );
        assert_eq!(
            BarAlignment::best_of(&values, VerticalSide::Bottom),
            Some(&values[1])
        );
    }

    #[test]
    fn rejects_invalid_peak_relation_and_geometry_state() {
        assert_eq!(
            AlignmentPeak::new(PeakId::new(1), StaffId::new(1), 0, 1.1),
            Err(BarAlignmentError::InvalidPeakGrade(PeakId::new(1)))
        );
        let same = peak(1, 1, 0, 0.5);
        assert_eq!(
            BarAlignment::new(
                same,
                same,
                0.0,
                0.0,
                BarImpacts::alignment(1.0, 1.0).unwrap(),
            ),
            Err(BarAlignmentError::InvalidPeakPair)
        );
        assert_eq!(
            BarAlignment::new(
                peak(1, 1, 0, 0.5),
                peak(2, 2, 0, 0.5),
                f64::INFINITY,
                0.0,
                BarImpacts::alignment(1.0, 1.0).unwrap(),
            ),
            Err(BarAlignmentError::InvalidGeometry)
        );
    }

    #[test]
    fn connection_width_and_median_match_java_pixel_center_adjustment() {
        let alignment = BarAlignment::new(
            geometric_peak(1, 1, 10, 3, 12, 20, 1.0),
            geometric_peak(2, 2, 12, 4, 30, 38, 1.0),
            0.0,
            1.0,
            BarImpacts::alignment(1.0, 1.0).unwrap(),
        )
        .unwrap();
        let connection = BarAlignment::connection(&alignment, 1.0, 1.0).unwrap();

        assert_eq!(connection.connection_width().unwrap(), 3.5);
        assert_eq!(
            connection.connection_median(3).unwrap(),
            ConnectionMedian {
                top: (11.5, 22.0),
                bottom: (14.0, 29.0),
            }
        );
        assert_eq!(
            connection.connection_median(2).unwrap(),
            ConnectionMedian {
                top: (11.5, 21.5),
                bottom: (14.0, 29.5),
            }
        );
    }

    #[test]
    fn connection_geometry_fails_closed_for_incomplete_state() {
        let plain = alignment(1, 1, 10, 1.0, 1.0, 1.0);
        assert_eq!(
            plain.connection_width(),
            Err(BarAlignmentError::WrongRelationType)
        );

        let connection = BarAlignment::connection(&plain, 1.0, 1.0).unwrap();
        assert_eq!(
            connection.connection_width(),
            Err(BarAlignmentError::MissingPeakGeometry(PeakId::new(1)))
        );
        assert_eq!(
            connection.connection_median(0),
            Err(BarAlignmentError::InvalidForegroundThickness)
        );
        assert_eq!(
            connection.connection_median(1),
            Err(BarAlignmentError::MissingPeakGeometry(PeakId::new(1)))
        );
        assert_eq!(
            AlignmentPeak::with_geometry(PeakId::new(3), StaffId::new(1), 0, 0, 10, 20, 1.0,),
            Err(BarAlignmentError::InvalidPeakGeometry(PeakId::new(3)))
        );
        assert_eq!(
            AlignmentPeak::with_geometry(PeakId::new(4), StaffId::new(1), 0, 2, 20, 10, 1.0,),
            Err(BarAlignmentError::InvalidPeakGeometry(PeakId::new(4)))
        );
    }
}
