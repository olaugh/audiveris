// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light decisions from Java `BarsRetriever`.

use crate::{
    bar_alignment::VerticalSide,
    staff_peak::{StaffPeak, StaffPeakAttribute, StaffPeakKey},
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bar_column::StaffId;

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
}
