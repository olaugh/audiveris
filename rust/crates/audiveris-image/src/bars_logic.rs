// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light decisions from Java `BarsRetriever`.

use crate::{
    bar_alignment::VerticalSide,
    bar_column::BarColumn,
    staff_peak::{HorizontalSide, StaffPeak, StaffPeakAttribute, StaffPeakKey},
};

/// Java `BracketKind`; `None` from [`bracket_kind`] remains distinct from
/// `Some(BracketKind::None)` for a middle-only bracket portion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BracketKind {
    None,
    Top,
    Bottom,
    Both,
}

/// Classify the end flags carried by a bracket peak.
#[must_use]
pub fn bracket_kind(peak: &StaffPeak) -> Option<BracketKind> {
    if peak.is_set(StaffPeakAttribute::BracketMiddle) {
        return Some(BracketKind::None);
    }
    match (
        peak.is_bracket_end(VerticalSide::Top),
        peak.is_bracket_end(VerticalSide::Bottom),
    ) {
        (true, true) => Some(BracketKind::Both),
        (true, false) => Some(BracketKind::Top),
        (false, true) => Some(BracketKind::Bottom),
        (false, false) => None,
    }
}

/// Java `getGroups`: collect maximal adjacent peak runs separated by at most
/// `maximum_double_bar_gap`, omitting singleton runs.
///
/// Input order is preserved exactly; Java does not sort or validate it here.
/// Gap arithmetic uses wrapping `int` operations before comparison.
#[must_use]
pub fn peak_groups(peaks: &[StaffPeak], maximum_double_bar_gap: i32) -> Vec<Vec<StaffPeakKey>> {
    let mut groups = Vec::new();
    let mut current = Vec::new();
    let mut previous_stop = None;

    for peak in peaks {
        if let Some(stop) = previous_stop {
            let gap = peak.start().wrapping_sub(stop).wrapping_sub(1);
            if gap > maximum_double_bar_gap {
                if current.len() > 1 {
                    groups.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
            }
        }
        current.push(peak.key());
        previous_stop = Some(peak.stop());
    }
    if current.len() > 1 {
        groups.push(current);
    }
    groups
}

/// Java `purgeLeftPeaks` selection, stopping at the first peak strictly to the
/// right of the staff limit because callers provide abscissa-ordered peaks.
#[must_use]
pub fn peaks_before_staff_start(peaks: &[StaffPeak], staff_left: i32) -> Vec<StaffPeakKey> {
    peaks
        .iter()
        .take_while(|peak| peak.start() <= staff_left)
        .filter(|peak| {
            !peak.is_staff_end(HorizontalSide::Left) && !peak.is_brace() && !peak.is_bracket()
        })
        .map(StaffPeak::key)
        .collect()
}

/// Java `purgeTooLeft` selection in its right-to-left insertion order.
///
/// The start peak anchors the chain. A distant peak is removed without moving
/// the anchor; a nearby peak becomes the next anchor. Java wrapping `int`
/// arithmetic is retained for the gap expression.
pub fn peaks_too_far_left(
    peaks: &[StaffPeak],
    start_index: usize,
    maximum_brace_bar_gap: i32,
) -> Result<Vec<StaffPeakKey>, BarsLogicError> {
    let Some(mut previous) = peaks.get(start_index) else {
        return Err(BarsLogicError::InvalidStartIndex(start_index));
    };
    let mut removed = Vec::new();
    for peak in peaks[..start_index].iter().rev() {
        let gap = previous.start().wrapping_sub(peak.stop()).wrapping_add(1);
        if gap > maximum_brace_bar_gap {
            removed.push(peak.key());
        } else {
            previous = peak;
        }
    }
    Ok(removed)
}

/// Java `purgeLeftOfBraces` selection in right-to-left insertion order.
#[must_use]
pub fn peaks_left_of_brace(
    peaks: &[StaffPeak],
    start_index: Option<usize>,
    brace_start: i32,
) -> Vec<StaffPeakKey> {
    let Some(start_index) = start_index.filter(|&index| index <= peaks.len()) else {
        return Vec::new();
    };
    peaks[..start_index]
        .iter()
        .rev()
        .filter(|peak| peak.stop() < brace_start)
        .map(StaffPeak::key)
        .collect()
}

/// First Java `BarColumn.isStart()` index, or `None` for Java's `-1`.
#[must_use]
pub fn start_column_index(columns: &[BarColumn]) -> Option<usize> {
    columns.iter().position(BarColumn::is_start)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarsLogicError {
    InvalidStartIndex(usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bar_column::{BarPeak, PeakId, StaffId};

    fn peak(staff: usize, start: i32, stop: i32) -> StaffPeak {
        StaffPeak::new(StaffId::new(staff), 0, 40, start, stop).unwrap()
    }

    #[test]
    fn bracket_kind_distinguishes_absence_middle_and_end_combinations() {
        let mut value = peak(1, 4, 5);
        assert_eq!(bracket_kind(&value), None);

        value.set(StaffPeakAttribute::BracketMiddle);
        value.set_bracket_end(VerticalSide::Top);
        assert_eq!(bracket_kind(&value), Some(BracketKind::None));

        value.unset(StaffPeakAttribute::BracketMiddle);
        assert_eq!(bracket_kind(&value), Some(BracketKind::Top));
        value.set_bracket_end(VerticalSide::Bottom);
        assert_eq!(bracket_kind(&value), Some(BracketKind::Both));
        value.unset(StaffPeakAttribute::BracketTop);
        assert_eq!(bracket_kind(&value), Some(BracketKind::Bottom));
    }

    #[test]
    fn peak_groups_preserve_order_include_exact_gap_and_omit_singletons() {
        let peaks = [
            peak(1, 0, 1),
            peak(1, 4, 5),   // gap 2: included at the boundary
            peak(1, 10, 10), // gap 4: closes the first group, singleton omitted
            peak(1, 20, 20),
            peak(1, 22, 23),
            peak(1, 25, 25),
        ];

        let groups = peak_groups(&peaks, 2);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], [peaks[0].key(), peaks[1].key()]);
        assert_eq!(groups[1], [peaks[3].key(), peaks[4].key(), peaks[5].key()]);
    }

    #[test]
    fn peak_groups_reproduce_java_wrapping_gap_arithmetic() {
        let peaks = [peak(1, 0, i32::MIN), peak(1, i32::MAX, i32::MAX)];
        // MAX - MIN - 1 wraps to -2, so Java keeps the pair even at maxGap -1.
        assert_eq!(peak_groups(&peaks, -1).len(), 1);
    }

    #[test]
    fn left_staff_purge_stops_in_order_and_preserves_structural_peaks() {
        let mut staff_end = peak(1, 2, 2);
        staff_end.set_staff_end(HorizontalSide::Left);
        let mut brace = peak(1, 3, 3);
        brace.set(StaffPeakAttribute::Brace);
        let mut bracket = peak(1, 4, 4);
        bracket.set(StaffPeakAttribute::BracketMiddle);
        let plain = peak(1, 5, 5);
        let later = peak(1, 10, 10);
        let out_of_order_left = peak(1, 1, 1);
        let peaks = [staff_end, brace, bracket, plain, later, out_of_order_left];

        assert_eq!(peaks_before_staff_start(&peaks, 5), [peaks[3].key()]);
    }

    #[test]
    fn too_left_purge_keeps_near_chain_anchor_and_returns_reverse_order() {
        let peaks = [peak(1, 0, 0), peak(1, 4, 4), peak(1, 8, 8), peak(1, 20, 20)];

        // From start x=20: x=8 is too far and does not replace the anchor;
        // x=4 and x=0 are therefore also too far, in right-to-left order.
        assert_eq!(
            peaks_too_far_left(&peaks, 3, 5).unwrap(),
            [peaks[2].key(), peaks[1].key(), peaks[0].key()]
        );

        let chained = [peak(1, 0, 0), peak(1, 4, 4), peak(1, 8, 8)];
        assert!(peaks_too_far_left(&chained, 2, 5).unwrap().is_empty());
        assert_eq!(
            peaks_too_far_left(&chained, 3, 5),
            Err(BarsLogicError::InvalidStartIndex(3))
        );
    }

    #[test]
    fn brace_purge_uses_start_prefix_and_returns_reverse_order() {
        let peaks = [peak(1, 0, 1), peak(1, 3, 4), peak(1, 7, 8), peak(1, 10, 11)];
        assert_eq!(
            peaks_left_of_brace(&peaks, Some(3), 7),
            [peaks[1].key(), peaks[0].key()]
        );
        assert!(peaks_left_of_brace(&peaks, None, 7).is_empty());
        assert!(peaks_left_of_brace(&peaks, Some(99), 7).is_empty());
    }

    #[test]
    fn start_column_lookup_returns_first_marked_column() {
        let staff_ids = vec![StaffId::new(1), StaffId::new(2)];
        let mut plain = BarColumn::new(staff_ids.clone()).unwrap();
        plain
            .add_peak(
                BarPeak::new(PeakId::new(1), StaffId::new(1), 2.0, 10.0, false, false).unwrap(),
            )
            .unwrap();
        let mut start = BarColumn::new(staff_ids).unwrap();
        start
            .add_peak(
                BarPeak::new(PeakId::new(2), StaffId::new(1), 2.0, 20.0, false, true).unwrap(),
            )
            .unwrap();

        assert_eq!(start_column_index(&[plain.clone(), start]), Some(1));
        assert_eq!(start_column_index(&[plain]), None);
    }
}
