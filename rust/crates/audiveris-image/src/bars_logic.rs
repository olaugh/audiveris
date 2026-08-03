// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light decisions from Java `BarsRetriever`.

use crate::{
    bar_alignment::VerticalSide,
    bar_column::BarColumn,
    run_table::Orientation,
    section::Section,
    staff_peak::{HorizontalSide, PeakBounds, StaffPeak, StaffPeakAttribute, StaffPeakKey},
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

/// Width class assigned by Java `BarsRetriever.partitionWidths`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeakWidthClass {
    Thin,
    Thick,
}

/// Stable peak-key result of Java `groupBarPeaks` plus `partitionWidths`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeakWidthAssignment {
    pub peak: StaffPeakKey,
    pub class: PeakWidthClass,
}

/// Dispatch structural peaks, isolated bars, and adjacent bar groups, then
/// reproduce Java's within-group thin/thick width partition.
///
/// Each inner slice is one staff's already abscissa-ordered peak sequence.
/// Braces and brackets terminate a group and are not classified. Isolated
/// bars are always thin. A heterogeneous group splits at the midpoint, with
/// exact midpoint ties classified thin, as in Java.
#[must_use]
pub fn classify_bar_peak_widths(
    staff_peaks: &[Vec<StaffPeak>],
    maximum_double_bar_gap: i32,
    interline: i32,
    minimum_normalized_width_delta: f64,
) -> Vec<PeakWidthAssignment> {
    let mut isolated = Vec::new();
    let mut groups: Vec<Vec<&StaffPeak>> = Vec::new();

    for peaks in staff_peaks {
        let mut group_index = None;
        let mut previous: Option<&StaffPeak> = None;

        for peak in peaks {
            if peak.is_brace() || peak.is_bracket() {
                if group_index.is_none() {
                    if let Some(previous) = previous {
                        isolated.push(previous);
                    }
                }
                group_index = None;
                previous = None;
                continue;
            }

            if let Some(previous_peak) = previous {
                let gap = peak
                    .start()
                    .wrapping_sub(previous_peak.stop())
                    .wrapping_sub(1);
                if gap <= maximum_double_bar_gap {
                    let index = *group_index.get_or_insert_with(|| {
                        groups.push(vec![previous_peak]);
                        groups.len() - 1
                    });
                    groups[index].push(peak);
                } else if group_index.is_some() {
                    group_index = None;
                } else {
                    isolated.push(previous_peak);
                }
            }
            previous = Some(peak);
        }

        if group_index.is_none() {
            if let Some(previous) = previous {
                isolated.push(previous);
            }
        }
    }

    let mut assignments = isolated
        .into_iter()
        .map(|peak| PeakWidthAssignment {
            peak: peak.key(),
            class: PeakWidthClass::Thin,
        })
        .collect::<Vec<_>>();

    for group in groups {
        let min_width = group
            .iter()
            .map(|peak| peak.width())
            .min()
            .unwrap_or(i32::MAX);
        let max_width = group
            .iter()
            .map(|peak| peak.width())
            .max()
            .unwrap_or(i32::MIN);
        let delta = max_width.wrapping_sub(min_width);
        let heterogeneous =
            f64::from(delta) / f64::from(interline) >= minimum_normalized_width_delta;

        assignments.extend(group.into_iter().map(|peak| {
            let class = if heterogeneous
                && peak.width().wrapping_sub(min_width) > max_width.wrapping_sub(peak.width())
            {
                PeakWidthClass::Thick
            } else {
                PeakWidthClass::Thin
            };
            PeakWidthAssignment {
                peak: peak.key(),
                class,
            }
        }));
    }
    assignments
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

/// Original column indices removed by Java `purgePartialColumns`.
///
/// Java represents a missing start column as `-1`, so in that case every
/// partial column is eligible. Columns through the first start column are
/// retained even when partial.
#[must_use]
pub fn partial_column_indices_after_start(columns: &[BarColumn]) -> Vec<usize> {
    let first_eligible = start_column_index(columns).map_or(0, |index| index + 1);
    columns
        .iter()
        .enumerate()
        .skip(first_eligible)
        .filter_map(|(index, column)| (!column.is_full()).then_some(index))
        .collect()
}

/// Java `BarFilamentBuilder.buildFilament` section preselection.
///
/// The caller supplies sections in nondecreasing x order. Intersection uses
/// Java `Rectangle`'s positive-area rule; a section touching the peak's right
/// edge triggers the same early break. Both horizontal and vertical sections
/// are accepted when their horizontal length does not exceed the peak width.
#[must_use]
pub fn bar_filament_section_ids(
    peak_bounds: PeakBounds,
    vertical_extension: i32,
    sections: &[Section],
) -> Vec<usize> {
    if peak_bounds.width <= 0 {
        return Vec::new();
    }
    let left = i128::from(peak_bounds.x);
    let right = left + i128::from(peak_bounds.width);
    let top = i128::from(peak_bounds.y) - i128::from(vertical_extension);
    let bottom =
        i128::from(peak_bounds.y) + i128::from(peak_bounds.height) + i128::from(vertical_extension);
    if right <= left || bottom <= top {
        return Vec::new();
    }

    let mut selected = Vec::new();
    for section in sections {
        let bounds = section.bounds();
        let section_left = bounds.x as i128;
        let section_right = section_left + bounds.width as i128;
        let section_top = bounds.y as i128;
        let section_bottom = section_top + bounds.height as i128;
        let intersects = section_left < right
            && section_right > left
            && section_top < bottom
            && section_bottom > top;
        if intersects {
            if section.length(Orientation::Horizontal) <= peak_bounds.width as usize {
                selected.push(section.id());
            }
        } else if section_left >= right {
            break;
        }
    }
    selected
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionLag {
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocatedSectionId {
    pub lag: SectionLag,
    pub id: usize,
}

/// Java `BarsRetriever.getSectionsByWidth`: filter VLAG then HLAG and perform
/// a stable x-only sort. Equal-x sections therefore retain lag and entity order.
#[must_use]
pub fn sections_by_width(
    vertical_sections: &[Section],
    horizontal_sections: &[Section],
    maximum_width: i32,
) -> Vec<LocatedSectionId> {
    if maximum_width < 0 {
        return Vec::new();
    }
    let maximum_width = maximum_width as usize;
    let mut selected = vertical_sections
        .iter()
        .filter(|section| section.length(Orientation::Horizontal) <= maximum_width)
        .map(|section| (section.bounds().x, SectionLag::Vertical, section.id()))
        .chain(
            horizontal_sections
                .iter()
                .filter(|section| section.length(Orientation::Horizontal) <= maximum_width)
                .map(|section| (section.bounds().x, SectionLag::Horizontal, section.id())),
        )
        .collect::<Vec<_>>();
    selected.sort_by_key(|(x, _, _)| *x);
    selected
        .into_iter()
        .map(|(_, lag, id)| LocatedSectionId { lag, id })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarsLogicError {
    InvalidStartIndex(usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bar_column::{BarPeak, PeakId, StaffId};
    use crate::{
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections},
    };

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
    fn bar_width_partition_matches_group_boundaries_and_midpoint_ties() {
        let isolated = peak(1, 0, 0);
        let narrow = peak(1, 10, 11); // width 2
        let midpoint = peak(1, 13, 16); // width 4, tied around [2, 6]
        let wide = peak(1, 18, 23); // width 6
        let after_group = peak(1, 40, 42);
        let mut brace = peak(1, 45, 45);
        brace.set(StaffPeakAttribute::Brace);
        let after_brace = peak(1, 47, 51);
        let staves = vec![vec![
            isolated.clone(),
            narrow.clone(),
            midpoint.clone(),
            wide.clone(),
            after_group.clone(),
            brace,
            after_brace.clone(),
        ]];

        let result = classify_bar_peak_widths(&staves, 2, 10, 0.3);
        assert_eq!(
            result,
            [
                PeakWidthAssignment {
                    peak: isolated.key(),
                    class: PeakWidthClass::Thin
                },
                PeakWidthAssignment {
                    peak: after_group.key(),
                    class: PeakWidthClass::Thin
                },
                PeakWidthAssignment {
                    peak: after_brace.key(),
                    class: PeakWidthClass::Thin
                },
                PeakWidthAssignment {
                    peak: narrow.key(),
                    class: PeakWidthClass::Thin
                },
                PeakWidthAssignment {
                    peak: midpoint.key(),
                    class: PeakWidthClass::Thin
                },
                PeakWidthAssignment {
                    peak: wide.key(),
                    class: PeakWidthClass::Thick
                },
            ]
        );
    }

    #[test]
    fn bar_width_partition_keeps_homogeneous_groups_thin() {
        let first = peak(1, 0, 1);
        let second = peak(1, 3, 5);
        let staves = vec![vec![first.clone(), second.clone()]];
        let result = classify_bar_peak_widths(&staves, 1, 10, 0.2);
        assert_eq!(
            result,
            [
                PeakWidthAssignment {
                    peak: first.key(),
                    class: PeakWidthClass::Thin
                },
                PeakWidthAssignment {
                    peak: second.key(),
                    class: PeakWidthClass::Thin
                },
            ]
        );
    }

    #[test]
    fn partial_column_purge_starts_strictly_after_first_start_column() {
        fn column(first: Option<BarPeak>, second: Option<BarPeak>) -> BarColumn {
            let mut column = BarColumn::new(vec![StaffId::new(1), StaffId::new(2)]).unwrap();
            if let Some(peak) = first {
                column.add_peak(peak).unwrap();
            }
            if let Some(peak) = second {
                column.add_peak(peak).unwrap();
            }
            column
        }
        fn bar(id: usize, staff: usize, start: bool) -> BarPeak {
            BarPeak::new(
                PeakId::new(id),
                StaffId::new(staff),
                2.0,
                id as f64,
                false,
                start,
            )
            .unwrap()
        }

        let columns = vec![
            column(Some(bar(1, 1, false)), None),
            column(Some(bar(2, 1, true)), None),
            column(Some(bar(3, 1, false)), Some(bar(4, 2, false))),
            column(None, Some(bar(5, 2, false))),
        ];
        assert_eq!(partial_column_indices_after_start(&columns), [3]);

        let no_start = vec![
            column(Some(bar(6, 1, false)), None),
            column(Some(bar(7, 1, false)), Some(bar(8, 2, false))),
        ];
        assert_eq!(partial_column_indices_after_start(&no_start), [0]);
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

    #[test]
    fn bar_section_selection_grows_vertically_filters_width_and_breaks_at_right_edge() {
        let mut table = RunTable::new(Orientation::Horizontal, 30, 20).unwrap();
        table.add_run(4, Run::new(9, 2)).unwrap(); // intersects after grow
        table.add_run(6, Run::new(10, 4)).unwrap(); // too wide
        table.add_run(10, Run::new(11, 2)).unwrap(); // intersects after grow
        table.add_run(5, Run::new(13, 1)).unwrap(); // touches right edge
        table.add_run(5, Run::new(15, 1)).unwrap(); // never visited after break
        let mut sections = build_sections(&table, JunctionPolicy::All);
        sections.sort_by_key(|section| section.bounds().x);

        assert_eq!(
            bar_filament_section_ids(
                PeakBounds {
                    x: 10,
                    y: 5,
                    width: 3,
                    height: 5,
                },
                2,
                &sections,
            ),
            sections
                .iter()
                .filter(|section| matches!(section.bounds().x, 9 | 11))
                .map(Section::id)
                .collect::<Vec<_>>()
        );
        assert!(
            bar_filament_section_ids(
                PeakBounds {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 1
                },
                0,
                &sections
            )
            .is_empty()
        );
    }

    #[test]
    fn section_width_filter_preserves_vertical_then_horizontal_order_on_x_ties() {
        let mut vertical_table = RunTable::new(Orientation::Vertical, 20, 20).unwrap();
        vertical_table.add_run(5, Run::new(2, 3)).unwrap();
        vertical_table.add_run(9, Run::new(1, 5)).unwrap(); // horizontal length 1
        let vertical = build_sections(&vertical_table, JunctionPolicy::All);

        let mut horizontal_table = RunTable::new(Orientation::Horizontal, 20, 20).unwrap();
        horizontal_table.add_run(3, Run::new(5, 2)).unwrap();
        horizontal_table.add_run(8, Run::new(12, 5)).unwrap(); // filtered at max 3
        let horizontal = build_sections(&horizontal_table, JunctionPolicy::All);

        let selected = sections_by_width(&vertical, &horizontal, 3);
        let vertical_at_five = vertical
            .iter()
            .find(|section| section.bounds().x == 5)
            .unwrap();
        let horizontal_at_five = horizontal
            .iter()
            .find(|section| section.bounds().x == 5)
            .unwrap();
        assert_eq!(
            &selected[..2],
            [
                LocatedSectionId {
                    lag: SectionLag::Vertical,
                    id: vertical_at_five.id(),
                },
                LocatedSectionId {
                    lag: SectionLag::Horizontal,
                    id: horizontal_at_five.id(),
                },
            ]
        );
        assert!(selected.iter().all(|entry| {
            !(entry.lag == SectionLag::Horizontal
                && entry.id
                    == horizontal
                        .iter()
                        .find(|section| section.bounds().x == 12)
                        .unwrap()
                        .id())
        }));
        assert!(sections_by_width(&vertical, &horizontal, -1).is_empty());
    }
}
