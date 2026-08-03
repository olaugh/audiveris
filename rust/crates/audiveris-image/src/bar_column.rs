// SPDX-License-Identifier: AGPL-3.0-or-later

//! Stable-ID, graph-neutral semantic core of Java `grid.BarColumn`.
//!
//! Staff slots are fixed at construction in caller order. Peak graph edges and
//! back-references are represented as immutable scalar peak facts and
//! caller-supplied relation records, avoiding PeakGraph/SIG/UI object cycles.

use std::collections::HashSet;
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StaffId(usize);

impl StaffId {
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PeakId(usize);

impl PeakId {
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> usize {
        self.0
    }
}

/// Scalar peak state consumed by Java `BarColumn`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarPeak {
    id: PeakId,
    staff_id: StaffId,
    width: f64,
    deskewed_x: f64,
    brace: bool,
    left_staff_end: bool,
}

impl BarPeak {
    pub fn new(
        id: PeakId,
        staff_id: StaffId,
        width: f64,
        deskewed_x: f64,
        brace: bool,
        left_staff_end: bool,
    ) -> Result<Self, BarColumnError> {
        if id.value() == 0 {
            return Err(BarColumnError::InvalidPeakId);
        }
        if staff_id.value() == 0 {
            return Err(BarColumnError::InvalidStaffId);
        }
        if !width.is_finite() || width <= 0.0 || !deskewed_x.is_finite() {
            return Err(BarColumnError::InvalidPeakGeometry(id));
        }
        Ok(Self {
            id,
            staff_id,
            width,
            deskewed_x,
            brace,
            left_staff_end,
        })
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
    pub const fn width(self) -> f64 {
        self.width
    }

    #[must_use]
    pub const fn deskewed_x(self) -> f64 {
        self.deskewed_x
    }

    #[must_use]
    pub const fn is_brace(self) -> bool {
        self.brace
    }

    #[must_use]
    pub const fn is_left_staff_end(self) -> bool {
        self.left_staff_end
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeakRelationKind {
    Alignment,
    Connection,
}

/// Neutral replacement for a PeakGraph edge between two peak IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeakRelation {
    pub first: PeakId,
    pub second: PeakId,
    pub kind: PeakRelationKind,
}

impl PeakRelation {
    #[must_use]
    pub const fn connection(first: PeakId, second: PeakId) -> Self {
        Self {
            first,
            second,
            kind: PeakRelationKind::Connection,
        }
    }

    #[must_use]
    pub const fn alignment(first: PeakId, second: PeakId) -> Self {
        Self {
            first,
            second,
            kind: PeakRelationKind::Alignment,
        }
    }

    fn is_connection_between(self, one: PeakId, two: PeakId) -> bool {
        self.kind == PeakRelationKind::Connection
            && ((self.first == one && self.second == two)
                || (self.first == two && self.second == one))
    }
}

#[derive(Clone, Debug)]
pub struct BarColumn {
    staff_ids: Vec<StaffId>,
    peaks: Vec<Option<BarPeak>>,
    deskewed_x: Option<f64>,
    width: Option<f64>,
}

impl BarColumn {
    pub fn new(staff_ids: Vec<StaffId>) -> Result<Self, BarColumnError> {
        if staff_ids.is_empty() {
            return Err(BarColumnError::NoStaffSlots);
        }
        let mut seen = HashSet::with_capacity(staff_ids.len());
        for staff_id in &staff_ids {
            if staff_id.value() == 0 {
                return Err(BarColumnError::InvalidStaffId);
            }
            if !seen.insert(*staff_id) {
                return Err(BarColumnError::DuplicateStaffId(*staff_id));
            }
        }
        let peaks = vec![None; staff_ids.len()];
        Ok(Self {
            staff_ids,
            peaks,
            deskewed_x: None,
            width: None,
        })
    }

    #[must_use]
    pub fn staff_ids(&self) -> &[StaffId] {
        &self.staff_ids
    }

    /// Fixed slots in staff order, matching Java's peak array order.
    #[must_use]
    pub fn peaks(&self) -> &[Option<BarPeak>] {
        &self.peaks
    }

    pub fn add_peak(&mut self, peak: BarPeak) -> Result<Option<BarPeak>, BarColumnError> {
        let slot = self.slot_of(peak.staff_id)?;
        if self
            .peaks
            .iter()
            .enumerate()
            .any(|(index, candidate)| index != slot && candidate.is_some_and(|p| p.id == peak.id))
        {
            return Err(BarColumnError::DuplicatePeakId(peak.id));
        }
        let previous = self.peaks[slot].replace(peak);
        // Java invalidates both lazy values even when replacing a slot.
        self.deskewed_x = None;
        self.width = None;
        Ok(previous)
    }

    /// Add peaks in supplied chain order, with Java's per-peak overwrite behavior.
    pub fn add_chain(&mut self, chain: &[BarPeak]) -> Result<(), BarColumnError> {
        validate_chain(chain)?;
        for peak in chain {
            self.add_peak(*peak)?;
        }
        Ok(())
    }

    /// Whether every chain staff slot is currently empty.
    pub fn can_include(&self, chain: &[BarPeak]) -> Result<bool, BarColumnError> {
        validate_chain(chain)?;
        for peak in chain {
            if self.peaks[self.slot_of(peak.staff_id)?].is_some() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Lazy mean width. An empty column returns and caches NaN, like Java 0/0.
    pub fn mean_width(&mut self) -> f64 {
        if let Some(width) = self.width {
            return width;
        }
        let mut count = 0;
        let mut sum = 0.0;
        for peak in self.peaks.iter().flatten() {
            count += 1;
            sum += peak.width;
        }
        let width = sum / count as f64;
        self.width = Some(width);
        width
    }

    /// Lazy mean deskewed abscissa. An empty column returns and caches NaN.
    pub fn deskewed_x(&mut self) -> f64 {
        if let Some(x) = self.deskewed_x {
            return x;
        }
        let mut count = 0;
        let mut sum = 0.0;
        for peak in self.peaks.iter().flatten() {
            count += 1;
            sum += peak.deskewed_x;
        }
        let x = sum / count as f64;
        self.deskewed_x = Some(x);
        x
    }

    /// Java status: all slots populated and none of the peaks is a brace.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.peaks
            .iter()
            .all(|peak| peak.is_some_and(|peak| !peak.brace))
    }

    #[must_use]
    pub fn has_brace(&self) -> bool {
        self.peaks.iter().flatten().any(|peak| peak.brace)
    }

    /// Whether the top staff peak marks the system's left end.
    #[must_use]
    pub fn is_start(&self) -> bool {
        self.peaks[0].is_some_and(|peak| peak.left_staff_end)
    }

    /// Full column with a concrete connection for every adjacent staff pair.
    #[must_use]
    pub fn is_fully_connected(&self, relations: &[PeakRelation]) -> bool {
        if !self.is_full() {
            return false;
        }
        self.peaks.windows(2).all(|pair| {
            let one = pair[0].expect("full column").id;
            let two = pair[1].expect("full column").id;
            relations
                .iter()
                .any(|relation| relation.is_connection_between(one, two))
        })
    }

    fn slot_of(&self, staff_id: StaffId) -> Result<usize, BarColumnError> {
        self.staff_ids
            .iter()
            .position(|candidate| *candidate == staff_id)
            .ok_or(BarColumnError::UnknownStaffId(staff_id))
    }
}

fn validate_chain(chain: &[BarPeak]) -> Result<(), BarColumnError> {
    let mut staff_ids = HashSet::with_capacity(chain.len());
    let mut peak_ids = HashSet::with_capacity(chain.len());
    for peak in chain {
        if !staff_ids.insert(peak.staff_id) {
            return Err(BarColumnError::DuplicateChainStaff(peak.staff_id));
        }
        if !peak_ids.insert(peak.id) {
            return Err(BarColumnError::DuplicatePeakId(peak.id));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarColumnError {
    NoStaffSlots,
    InvalidStaffId,
    InvalidPeakId,
    InvalidPeakGeometry(PeakId),
    DuplicateStaffId(StaffId),
    UnknownStaffId(StaffId),
    DuplicatePeakId(PeakId),
    DuplicateChainStaff(StaffId),
}

impl fmt::Display for BarColumnError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoStaffSlots => formatter.write_str("bar column needs at least one staff slot"),
            Self::InvalidStaffId => formatter.write_str("staff ID must be positive"),
            Self::InvalidPeakId => formatter.write_str("peak ID must be positive"),
            Self::InvalidPeakGeometry(id) => {
                write!(formatter, "invalid geometry for peak {}", id.value())
            }
            Self::DuplicateStaffId(id) => write!(formatter, "duplicate staff ID {}", id.value()),
            Self::UnknownStaffId(id) => write!(formatter, "unknown staff ID {}", id.value()),
            Self::DuplicatePeakId(id) => write!(formatter, "duplicate peak ID {}", id.value()),
            Self::DuplicateChainStaff(id) => {
                write!(formatter, "chain repeats staff ID {}", id.value())
            }
        }
    }
}

impl Error for BarColumnError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn peak(id: usize, staff: usize, width: f64, x: f64) -> BarPeak {
        BarPeak::new(PeakId::new(id), StaffId::new(staff), width, x, false, false).unwrap()
    }

    #[test]
    fn fixed_slots_preserve_staff_order_and_room_behavior() {
        let mut column =
            BarColumn::new(vec![StaffId::new(30), StaffId::new(10), StaffId::new(20)]).unwrap();
        column.add_peak(peak(3, 20, 3.0, 30.0)).unwrap();
        column.add_peak(peak(1, 30, 1.0, 10.0)).unwrap();

        assert_eq!(
            column
                .peaks()
                .iter()
                .map(|peak| peak.map(BarPeak::id))
                .collect::<Vec<_>>(),
            [Some(PeakId::new(1)), None, Some(PeakId::new(3))]
        );
        let chain = [peak(2, 10, 2.0, 20.0)];
        assert!(column.can_include(&chain).unwrap());
        column.add_chain(&chain).unwrap();
        assert!(!column.can_include(&chain).unwrap());
    }

    #[test]
    fn overwrite_invalidates_cached_means_and_empty_mean_is_nan() {
        let mut empty = BarColumn::new(vec![StaffId::new(1)]).unwrap();
        assert!(empty.mean_width().is_nan());
        assert!(empty.deskewed_x().is_nan());

        let mut column = BarColumn::new(vec![StaffId::new(1), StaffId::new(2)]).unwrap();
        column.add_peak(peak(1, 1, 2.0, 10.0)).unwrap();
        column.add_peak(peak(2, 2, 4.0, 20.0)).unwrap();
        assert_eq!(column.mean_width(), 3.0);
        assert_eq!(column.deskewed_x(), 15.0);

        let previous = column.add_peak(peak(3, 2, 8.0, 40.0)).unwrap();
        assert_eq!(previous.map(BarPeak::id), Some(PeakId::new(2)));
        assert_eq!(column.mean_width(), 5.0);
        assert_eq!(column.deskewed_x(), 25.0);
    }

    #[test]
    fn full_start_brace_and_connection_status_match_java() {
        let staff_ids = vec![StaffId::new(1), StaffId::new(2), StaffId::new(3)];
        let mut column = BarColumn::new(staff_ids).unwrap();
        let top = BarPeak::new(PeakId::new(1), StaffId::new(1), 2.0, 10.0, false, true).unwrap();
        let brace = BarPeak::new(PeakId::new(2), StaffId::new(2), 2.0, 10.0, true, false).unwrap();
        column.add_peak(top).unwrap();
        column.add_peak(brace).unwrap();
        column.add_peak(peak(3, 3, 2.0, 10.0)).unwrap();
        assert!(column.is_start());
        assert!(column.has_brace());
        assert!(!column.is_full());
        assert!(!column.is_fully_connected(&[]));

        column.add_peak(peak(4, 2, 2.0, 10.0)).unwrap();
        assert!(column.is_full());
        assert!(!column.has_brace());
        let alignments = [
            PeakRelation::alignment(PeakId::new(1), PeakId::new(4)),
            PeakRelation::alignment(PeakId::new(4), PeakId::new(3)),
        ];
        assert!(!column.is_fully_connected(&alignments));
        let connections = [
            PeakRelation::connection(PeakId::new(4), PeakId::new(1)),
            PeakRelation::connection(PeakId::new(3), PeakId::new(4)),
        ];
        assert!(column.is_fully_connected(&connections));

        // Java's zero-length adjacency array makes a full one-staff column
        // vacuously fully connected.
        let mut one_staff = BarColumn::new(vec![StaffId::new(9)]).unwrap();
        one_staff.add_peak(peak(9, 9, 2.0, 10.0)).unwrap();
        assert!(one_staff.is_fully_connected(&[]));
    }

    #[test]
    fn rejects_invalid_slots_peaks_and_chain_state() {
        assert!(matches!(
            BarColumn::new(vec![]),
            Err(BarColumnError::NoStaffSlots)
        ));
        assert!(matches!(
            BarColumn::new(vec![StaffId::new(1), StaffId::new(1)]),
            Err(BarColumnError::DuplicateStaffId(id)) if id == StaffId::new(1)
        ));
        assert_eq!(
            BarPeak::new(PeakId::new(1), StaffId::new(1), f64::NAN, 2.0, false, false,),
            Err(BarColumnError::InvalidPeakGeometry(PeakId::new(1)))
        );

        let mut column = BarColumn::new(vec![StaffId::new(1)]).unwrap();
        assert_eq!(
            column.add_peak(peak(2, 9, 2.0, 2.0)),
            Err(BarColumnError::UnknownStaffId(StaffId::new(9)))
        );
        let duplicate_staff = [peak(1, 1, 2.0, 2.0), peak(2, 1, 2.0, 3.0)];
        assert_eq!(
            column.can_include(&duplicate_staff),
            Err(BarColumnError::DuplicateChainStaff(StaffId::new(1)))
        );
    }
}
