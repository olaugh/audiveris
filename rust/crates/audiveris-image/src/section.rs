// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light construction of Audiveris-style sections from run tables.
//!
//! A section contains one run per consecutive run-table sequence. A one-to-one
//! overlap continues a section when its [`JunctionPolicy`] permits it. Forks,
//! joins, incompatible runs, and empty sequences end the current section and
//! start one or more new sections. This deliberately mirrors the two-sided
//! traversal in Java's `SectionFactory`; it is not a generic connected-component
//! labeller.

use crate::run_table::{Orientation, Run, RunTable};

/// An absolute, integer pixel bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Bounds {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

/// Compatibility rule for extending a section with its sole overlapping run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JunctionPolicy {
    /// Accept every one-to-one junction.
    All,
    /// Limit the absolute difference from the previous run length.
    Delta { max_length_delta: usize },
    /// Limit the ratio to the section's integer-truncated mean run length.
    Ratio { max_length_ratio: f64 },
    /// Limit shifts of both run endpoints from the previous run.
    Shift { max_shift: usize },
}

impl JunctionPolicy {
    /// Audiveris' default ratio policy (`JunctionRatioPolicy.DEFAULT`).
    pub const DEFAULT_RATIO: Self = Self::Ratio {
        max_length_ratio: 1.25,
    };

    fn accepts(self, run: Run, section: &Section) -> bool {
        match self {
            Self::All => true,
            Self::Delta { max_length_delta } => {
                run.length.abs_diff(section.last_run().length) <= max_length_delta
            }
            Self::Ratio { max_length_ratio } => {
                // Java computes the minimum independently as 1 / maximum and
                // uses Section.getMeanRunLength(), whose division is integral.
                let ratio = run.length as f64 / section.mean_run_length() as f64;
                let min_length_ratio = 1.0 / max_length_ratio;
                min_length_ratio <= ratio && ratio <= max_length_ratio
            }
            Self::Shift { max_shift } => {
                let last = section.last_run();
                run.start.abs_diff(last.start) <= max_shift
                    && run.stop().abs_diff(last.stop()) <= max_shift
            }
        }
    }
}

/// An immutable section produced from a run table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    id: usize,
    orientation: Orientation,
    first_pos: usize,
    runs: Vec<Run>,
    weight: usize,
    min_coord: usize,
    max_coord: usize,
}

impl Section {
    fn new(id: usize, orientation: Orientation, first_pos: usize, first_run: Run) -> Self {
        Self {
            id,
            orientation,
            first_pos,
            runs: vec![first_run],
            weight: first_run.length,
            min_coord: first_run.start,
            max_coord: first_run.stop(),
        }
    }

    fn append(&mut self, run: Run) {
        self.weight += run.length;
        self.min_coord = self.min_coord.min(run.start);
        self.max_coord = self.max_coord.max(run.stop());
        self.runs.push(run);
    }

    /// Stable, one-based creation-order identifier.
    #[must_use]
    pub const fn id(&self) -> usize {
        self.id
    }

    #[must_use]
    pub const fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// Position of the first run (y for horizontal runs, x for vertical runs).
    #[must_use]
    pub const fn first_pos(&self) -> usize {
        self.first_pos
    }

    /// Position of the last run, inclusive.
    #[must_use]
    pub fn last_pos(&self) -> usize {
        self.first_pos + self.runs.len() - 1
    }

    #[must_use]
    pub fn runs(&self) -> &[Run] {
        &self.runs
    }

    #[must_use]
    pub fn first_run(&self) -> Run {
        self.runs[0]
    }

    #[must_use]
    pub fn last_run(&self) -> Run {
        self.runs[self.runs.len() - 1]
    }

    #[must_use]
    pub fn run_count(&self) -> usize {
        self.runs.len()
    }

    /// Number of foreground pixels in the section.
    #[must_use]
    pub const fn weight(&self) -> usize {
        self.weight
    }

    /// Smallest coordinate along the runs.
    #[must_use]
    pub const fn start_coord(&self) -> usize {
        self.min_coord
    }

    /// Largest coordinate along the runs, inclusive.
    #[must_use]
    pub const fn stop_coord(&self) -> usize {
        self.max_coord
    }

    /// Mean run length with Java-compatible integer truncation.
    #[must_use]
    pub fn mean_run_length(&self) -> usize {
        self.weight / self.runs.len()
    }

    /// Length of the longest constituent run.
    #[must_use]
    pub fn max_run_length(&self) -> usize {
        self.runs
            .iter()
            .map(|run| run.length)
            .max()
            .expect("a section always contains at least one run")
    }

    /// Absolute pixel bounds, swapping oriented axes for vertical runs.
    #[must_use]
    pub fn bounds(&self) -> Bounds {
        let coordinate_length = self.max_coord - self.min_coord + 1;
        let position_length = self.runs.len();

        match self.orientation {
            Orientation::Horizontal => Bounds {
                x: self.min_coord,
                y: self.first_pos,
                width: coordinate_length,
                height: position_length,
            },
            Orientation::Vertical => Bounds {
                x: self.first_pos,
                y: self.min_coord,
                width: position_length,
                height: coordinate_length,
            },
        }
    }

    /// Absolute length along the requested orientation.
    #[must_use]
    pub fn length(&self, orientation: Orientation) -> usize {
        let bounds = self.bounds();
        match orientation {
            Orientation::Horizontal => bounds.width,
            Orientation::Vertical => bounds.height,
        }
    }

    /// Absolute thickness across the requested orientation.
    #[must_use]
    pub fn thickness(&self, orientation: Orientation) -> usize {
        let bounds = self.bounds();
        match orientation {
            Orientation::Horizontal => bounds.height,
            Orientation::Vertical => bounds.width,
        }
    }

    /// Pixel weight divided by absolute length along the requested orientation.
    #[must_use]
    pub fn mean_thickness(&self, orientation: Orientation) -> f64 {
        self.weight as f64 / self.length(orientation) as f64
    }

    /// Number of section pixels inside an absolute, half-open pixel box.
    #[must_use]
    pub fn pixel_count_in(&self, roi: Bounds) -> usize {
        self.pixel_moments_in(roi).0
    }

    /// Centroid of section pixels inside an absolute, half-open pixel box.
    #[must_use]
    pub fn pixel_centroid_in(&self, roi: Bounds) -> Option<(f64, f64)> {
        let (count, sum_x, sum_y) = self.pixel_moments_in(roi);
        (count != 0).then(|| (sum_x / count as f64, sum_y / count as f64))
    }

    /// Whether this section intersects or shares a horizontal/vertical edge
    /// with another section. Corner-only diagonal contact is rejected, matching
    /// Java `BasicSection.touches` and `GeoUtil.touch`.
    #[must_use]
    pub fn touches(&self, other: &Self) -> bool {
        self.absolute_run_boxes().any(|one| {
            other
                .absolute_run_boxes()
                .any(|two| run_boxes_touch(one, two))
        })
    }

    fn pixel_moments_in(&self, roi: Bounds) -> (usize, f64, f64) {
        if roi.width == 0 || roi.height == 0 {
            return (0, 0.0, 0.0);
        }
        let roi_stop_x = roi.x + roi.width - 1;
        let roi_stop_y = roi.y + roi.height - 1;
        let mut count = 0;
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;

        for (offset, run) in self.runs.iter().enumerate() {
            let position = self.first_pos + offset;
            let (fixed, roi_fixed_min, roi_fixed_max, roi_coord_min, roi_coord_max) =
                match self.orientation {
                    Orientation::Horizontal => (position, roi.y, roi_stop_y, roi.x, roi_stop_x),
                    Orientation::Vertical => (position, roi.x, roi_stop_x, roi.y, roi_stop_y),
                };
            if fixed < roi_fixed_min || fixed > roi_fixed_max {
                continue;
            }
            let start = run.start.max(roi_coord_min);
            let stop = run.stop().min(roi_coord_max);
            if start > stop {
                continue;
            }
            for coordinate in start..=stop {
                let (x, y) = match self.orientation {
                    Orientation::Horizontal => (coordinate, fixed),
                    Orientation::Vertical => (fixed, coordinate),
                };
                count += 1;
                sum_x += x as f64;
                sum_y += y as f64;
            }
        }

        (count, sum_x, sum_y)
    }

    fn absolute_run_boxes(&self) -> impl Iterator<Item = Bounds> + '_ {
        self.runs.iter().enumerate().map(|(offset, run)| {
            let position = self.first_pos + offset;
            match self.orientation {
                Orientation::Horizontal => Bounds {
                    x: run.start,
                    y: position,
                    width: run.length,
                    height: 1,
                },
                Orientation::Vertical => Bounds {
                    x: position,
                    y: run.start,
                    width: 1,
                    height: run.length,
                },
            }
        })
    }
}

fn run_boxes_touch(one: Bounds, two: Bounds) -> bool {
    let x_overlap = one
        .x
        .saturating_add(one.width)
        .min(two.x.saturating_add(two.width)) as isize
        - one.x.max(two.x) as isize;
    if x_overlap < 0 {
        return false;
    }
    let y_overlap = one
        .y
        .saturating_add(one.height)
        .min(two.y.saturating_add(two.height)) as isize
        - one.y.max(two.y) as isize;
    y_overlap >= 0 && (x_overlap > 0 || y_overlap > 0)
}

/// Build sections in the same deterministic creation order as Java `SectionFactory`.
#[must_use]
pub fn build_sections(run_table: &RunTable, policy: JunctionPolicy) -> Vec<Section> {
    let orientation = run_table.orientation();
    let mut sections = Vec::new();
    let mut processed = Vec::new();
    let mut next_active = Vec::new();

    let create_section =
        |position: usize, run: Run, sections: &mut Vec<Section>, processed: &mut Vec<bool>| {
            let index = sections.len();
            sections.push(Section::new(index + 1, orientation, position, run));
            processed.push(false);
            index
        };

    for &run in run_table.sequence(0).unwrap_or_default() {
        next_active.push(create_section(0, run, &mut sections, &mut processed));
    }

    for position in 1..run_table.sequence_count() {
        let next_runs = run_table.sequence(position).unwrap_or_default();
        if next_runs.is_empty() {
            next_active.clear();
            continue;
        }

        let previous_active = std::mem::take(&mut next_active);

        // Previous-side pass: a fork or an incompatible sole successor marks
        // the old section as finished before any next-side assignment occurs.
        for &section_index in &previous_active {
            let previous = sections[section_index].last_run();
            let mut overlap_count = 0;
            let mut sole_overlap = None;

            for &run in next_runs {
                if run.start > previous.stop() {
                    break;
                }
                if run.stop() >= previous.start {
                    overlap_count += 1;
                    sole_overlap = Some(run);
                }
            }

            match overlap_count {
                0 => {}
                1 => {
                    if !policy.accepts(sole_overlap.expect("one overlap"), &sections[section_index])
                    {
                        processed[section_index] = true;
                    }
                }
                _ => processed[section_index] = true,
            }
        }

        // Next-side pass: only a unique, still-unprocessed predecessor can be
        // continued. Joins create a new section regardless of policy.
        for &run in next_runs {
            let mut overlaps = Vec::new();
            for &section_index in &previous_active {
                let previous = sections[section_index].last_run();
                if previous.start > run.stop() {
                    break;
                }
                if previous.stop() >= run.start {
                    overlaps.push(section_index);
                }
            }

            let section_index = if overlaps.len() == 1 && !processed[overlaps[0]] {
                let section_index = overlaps[0];
                sections[section_index].append(run);
                section_index
            } else {
                create_section(position, run, &mut sections, &mut processed)
            };
            next_active.push(section_index);
        }
    }

    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(
        orientation: Orientation,
        width: usize,
        height: usize,
        runs: &[(usize, Run)],
    ) -> RunTable {
        let mut table = RunTable::new(orientation, width, height).unwrap();
        for &(position, run) in runs {
            assert!(table.add_run(position, run).unwrap());
        }
        table
    }

    #[test]
    fn horizontal_section_metrics_match_java() {
        let table = table(
            Orientation::Horizontal,
            12,
            5,
            &[
                (1, Run::new(2, 4)),
                (2, Run::new(3, 5)),
                (3, Run::new(1, 3)),
            ],
        );
        let sections = build_sections(&table, JunctionPolicy::All);

        assert_eq!(sections.len(), 1);
        let section = &sections[0];
        assert_eq!(section.id(), 1);
        assert_eq!(section.orientation(), Orientation::Horizontal);
        assert_eq!((section.first_pos(), section.last_pos()), (1, 3));
        assert_eq!((section.start_coord(), section.stop_coord()), (1, 7));
        assert_eq!((section.run_count(), section.weight()), (3, 12));
        assert_eq!(section.mean_run_length(), 4);
        assert_eq!(section.max_run_length(), 5);
        assert_eq!(
            section.bounds(),
            Bounds {
                x: 1,
                y: 1,
                width: 7,
                height: 3
            }
        );
        assert_eq!(section.length(Orientation::Horizontal), 7);
        assert_eq!(section.thickness(Orientation::Horizontal), 3);
        assert_eq!(section.mean_thickness(Orientation::Horizontal), 12.0 / 7.0);
    }

    #[test]
    fn pixel_roi_moments_match_horizontal_and_vertical_coordinates() {
        let horizontal = build_sections(
            &table(
                Orientation::Horizontal,
                8,
                4,
                &[(1, Run::new(1, 4)), (2, Run::new(2, 4))],
            ),
            JunctionPolicy::All,
        )
        .remove(0);
        let roi = Bounds {
            x: 3,
            y: 1,
            width: 2,
            height: 2,
        };
        assert_eq!(horizontal.pixel_count_in(roi), 4);
        assert_eq!(horizontal.pixel_centroid_in(roi), Some((3.5, 1.5)));

        let vertical = build_sections(
            &table(
                Orientation::Vertical,
                4,
                8,
                &[(1, Run::new(1, 4)), (2, Run::new(2, 4))],
            ),
            JunctionPolicy::All,
        )
        .remove(0);
        let vertical_roi = Bounds {
            x: 1,
            y: 3,
            width: 2,
            height: 2,
        };
        assert_eq!(vertical.pixel_count_in(vertical_roi), 4);
        assert_eq!(vertical.pixel_centroid_in(vertical_roi), Some((1.5, 3.5)));
        assert_eq!(horizontal.pixel_count_in(Bounds { width: 0, ..roi }), 0);
    }

    #[test]
    fn touches_accepts_edge_contact_but_rejects_diagonal_contact() {
        let horizontal = build_sections(
            &table(Orientation::Horizontal, 8, 4, &[(1, Run::new(1, 3))]),
            JunctionPolicy::All,
        )
        .remove(0);
        let edge = build_sections(
            &table(Orientation::Vertical, 8, 4, &[(4, Run::new(1, 2))]),
            JunctionPolicy::All,
        )
        .remove(0);
        let diagonal = build_sections(
            &table(Orientation::Vertical, 8, 4, &[(4, Run::new(2, 2))]),
            JunctionPolicy::All,
        )
        .remove(0);

        assert!(horizontal.touches(&edge));
        assert!(!horizontal.touches(&diagonal));
        assert_eq!(horizontal.touches(&edge), edge.touches(&horizontal));
    }

    #[test]
    fn vertical_bounds_swap_oriented_axes() {
        let table = table(
            Orientation::Vertical,
            6,
            12,
            &[
                (2, Run::new(4, 3)),
                (3, Run::new(3, 5)),
                (4, Run::new(5, 2)),
            ],
        );
        let section = build_sections(&table, JunctionPolicy::All).remove(0);

        assert_eq!(
            section.bounds(),
            Bounds {
                x: 2,
                y: 3,
                width: 3,
                height: 5
            }
        );
        assert_eq!(section.length(Orientation::Vertical), 5);
        assert_eq!(section.thickness(Orientation::Vertical), 3);
        assert_eq!(section.mean_thickness(Orientation::Vertical), 2.0);
    }

    #[test]
    fn overlap_is_inclusive_but_diagonal_adjacency_is_not() {
        let touching = table(
            Orientation::Horizontal,
            8,
            2,
            &[(0, Run::new(1, 2)), (1, Run::new(2, 2))],
        );
        assert_eq!(build_sections(&touching, JunctionPolicy::All).len(), 1);

        let diagonal = table(
            Orientation::Horizontal,
            8,
            2,
            &[(0, Run::new(1, 2)), (1, Run::new(3, 2))],
        );
        let sections = build_sections(&diagonal, JunctionPolicy::All);
        assert_eq!(sections.iter().map(Section::id).collect::<Vec<_>>(), [1, 2]);
        assert!(sections.iter().all(|section| section.run_count() == 1));
    }

    #[test]
    fn fork_ends_source_and_preserves_next_run_order() {
        let table = table(
            Orientation::Horizontal,
            10,
            2,
            &[
                (0, Run::new(1, 6)),
                // Deliberately inserted in reverse coordinate order. RunTable
                // sorts them, and Java SectionFactory assigns IDs in that order.
                (1, Run::new(5, 1)),
                (1, Run::new(1, 1)),
            ],
        );
        let sections = build_sections(&table, JunctionPolicy::All);

        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].runs(), &[Run::new(1, 6)]);
        assert_eq!(sections[1].runs(), &[Run::new(1, 1)]);
        assert_eq!(sections[2].runs(), &[Run::new(5, 1)]);
        assert_eq!(
            sections.iter().map(Section::id).collect::<Vec<_>>(),
            [1, 2, 3]
        );
    }

    #[test]
    fn join_starts_new_section_after_both_sources() {
        let table = table(
            Orientation::Horizontal,
            10,
            2,
            &[
                (0, Run::new(1, 2)),
                (0, Run::new(5, 2)),
                (1, Run::new(2, 4)),
            ],
        );
        let sections = build_sections(&table, JunctionPolicy::All);

        assert_eq!(sections.len(), 3);
        assert_eq!(
            sections.iter().map(Section::id).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!((sections[0].last_pos(), sections[1].last_pos()), (0, 0));
        assert_eq!(sections[2].first_pos(), 1);
        assert_eq!(sections[2].runs(), &[Run::new(2, 4)]);
    }

    #[test]
    fn empty_sequence_breaks_a_section_and_ids_restart_per_build() {
        let table = table(
            Orientation::Horizontal,
            8,
            3,
            &[(0, Run::new(2, 2)), (2, Run::new(2, 2))],
        );
        let first = build_sections(&table, JunctionPolicy::All);
        let second = build_sections(&table, JunctionPolicy::All);

        assert_eq!(first.iter().map(Section::id).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(second.iter().map(Section::id).collect::<Vec<_>>(), [1, 2]);
        assert_eq!((first[0].last_pos(), first[1].first_pos()), (0, 2));
    }

    #[test]
    fn ratio_policy_uses_inclusive_java_boundary() {
        let accepted = table(
            Orientation::Horizontal,
            12,
            2,
            &[(0, Run::new(2, 4)), (1, Run::new(2, 5))],
        );
        assert_eq!(
            build_sections(
                &accepted,
                JunctionPolicy::Ratio {
                    max_length_ratio: 1.25,
                },
            )
            .len(),
            1
        );

        let rejected = table(
            Orientation::Horizontal,
            12,
            2,
            &[(0, Run::new(2, 4)), (1, Run::new(2, 6))],
        );
        assert_eq!(
            build_sections(&rejected, JunctionPolicy::DEFAULT_RATIO).len(),
            2
        );
    }

    #[test]
    fn delta_and_shift_policies_include_exact_limits() {
        let lengths = table(
            Orientation::Horizontal,
            12,
            2,
            &[(0, Run::new(2, 4)), (1, Run::new(2, 6))],
        );
        assert_eq!(
            build_sections(
                &lengths,
                JunctionPolicy::Delta {
                    max_length_delta: 2,
                },
            )
            .len(),
            1
        );

        let shifted = table(
            Orientation::Horizontal,
            12,
            2,
            &[(0, Run::new(2, 5)), (1, Run::new(3, 5))],
        );
        assert_eq!(
            build_sections(&shifted, JunctionPolicy::Shift { max_shift: 1 }).len(),
            1
        );
        assert_eq!(
            build_sections(&shifted, JunctionPolicy::Shift { max_shift: 0 }).len(),
            2
        );
    }

    #[test]
    fn integer_mean_run_length_matches_java_truncation() {
        let table = table(
            Orientation::Horizontal,
            10,
            2,
            &[(0, Run::new(1, 4)), (1, Run::new(1, 5))],
        );
        let section = build_sections(&table, JunctionPolicy::All).remove(0);

        assert_eq!(section.weight(), 9);
        assert_eq!(section.mean_run_length(), 4);
    }
}
