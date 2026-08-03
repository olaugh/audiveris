// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light port of Java `GlyphFactory.buildGlyphs(RunTable, Point)`.
//!
//! Runs connect only when their inclusive coordinate ranges overlap in two
//! consecutive sequences. Diagonal-only contact is deliberately disconnected.

use std::collections::BTreeMap;

use crate::run_table::{Orientation, Run, RunTable};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphComponentRun {
    pub sequence: usize,
    pub run: Run,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GlyphComponent {
    /// Smallest Java discovery mark in this connected component.
    pub ancestor_mark: usize,
    pub left: i32,
    pub top: i32,
    pub width: usize,
    pub height: usize,
    pub weight: usize,
    pub centroid_x: f64,
    pub centroid_y: f64,
    pub rounded_centroid_x: i32,
    pub rounded_centroid_y: i32,
    /// Source orientation and traversal order, with source-relative runs.
    pub orientation: Orientation,
    pub runs: Vec<GlyphComponentRun>,
}

#[derive(Clone, Copy, Debug)]
struct MarkedRun {
    run: Run,
    mark: usize,
}

/// Build glyph components in Java ancestor-mark order.
#[must_use]
pub fn build_glyph_components(
    table: &RunTable,
    offset_x: i32,
    offset_y: i32,
) -> Vec<GlyphComponent> {
    let mut marked = vec![Vec::<MarkedRun>::new(); table.sequence_count()];
    let mut parents = vec![0_usize];
    let mut global_mark = 0_usize;

    for sequence in 0..table.sequence_count() {
        let previous = sequence.checked_sub(1).map(|index| marked[index].clone());
        let mut previous_active = 0_usize;
        for run in table.sequence(sequence).unwrap_or_default() {
            let mut mark = 0_usize;
            if let Some(previous) = &previous {
                for (index, candidate) in previous.iter().enumerate().skip(previous_active) {
                    if candidate.run.start > run.stop() {
                        break;
                    }
                    if candidate.run.stop() >= run.start {
                        if mark == 0 {
                            mark = candidate.mark;
                        } else {
                            union_min(&mut parents, mark, candidate.mark);
                        }
                        previous_active = index;
                    }
                }
            }
            if mark == 0 {
                global_mark += 1;
                parents.push(global_mark);
                mark = global_mark;
            }
            marked[sequence].push(MarkedRun { run: *run, mark });
        }
    }

    for mark in 1..=global_mark {
        let root = find_root(&parents, mark);
        parents[mark] = root;
    }
    let mut grouped = BTreeMap::<usize, Vec<GlyphComponentRun>>::new();
    for (sequence, runs) in marked.iter().enumerate() {
        for marked_run in runs {
            grouped
                .entry(parents[marked_run.mark])
                .or_default()
                .push(GlyphComponentRun {
                    sequence,
                    run: marked_run.run,
                });
        }
    }
    grouped
        .into_iter()
        .map(|(ancestor_mark, runs)| {
            component_from_runs(table.orientation(), ancestor_mark, runs, offset_x, offset_y)
        })
        .collect()
}

fn find_root(parents: &[usize], mut mark: usize) -> usize {
    while parents[mark] != mark {
        mark = parents[mark];
    }
    mark
}

fn union_min(parents: &mut [usize], one: usize, two: usize) {
    let one = find_root(parents, one);
    let two = find_root(parents, two);
    if one != two {
        let (parent, child) = if one < two { (one, two) } else { (two, one) };
        parents[child] = parent;
    }
}

fn component_from_runs(
    orientation: Orientation,
    ancestor_mark: usize,
    runs: Vec<GlyphComponentRun>,
    offset_x: i32,
    offset_y: i32,
) -> GlyphComponent {
    let min_sequence = runs.iter().map(|entry| entry.sequence).min().unwrap();
    let max_sequence = runs.iter().map(|entry| entry.sequence).max().unwrap();
    let min_coordinate = runs.iter().map(|entry| entry.run.start).min().unwrap();
    let max_coordinate = runs.iter().map(|entry| entry.run.stop()).max().unwrap();
    let (relative_left, relative_top, width, height) = match orientation {
        Orientation::Horizontal => (
            min_coordinate,
            min_sequence,
            max_coordinate - min_coordinate + 1,
            max_sequence - min_sequence + 1,
        ),
        Orientation::Vertical => (
            min_sequence,
            min_coordinate,
            max_sequence - min_sequence + 1,
            max_coordinate - min_coordinate + 1,
        ),
    };
    let left = offset_x.wrapping_add(
        i32::try_from(relative_left).expect("RunTable dimensions originate as Java int values"),
    );
    let top = offset_y.wrapping_add(
        i32::try_from(relative_top).expect("RunTable dimensions originate as Java int values"),
    );
    let mut weight = 0_usize;
    let mut sum_x = 0_f64;
    let mut sum_y = 0_f64;
    for entry in &runs {
        let length = entry.run.length;
        weight += length;
        let coordinate_sum = arithmetic_sum(entry.run.start, entry.run.stop());
        match orientation {
            Orientation::Horizontal => {
                sum_x += coordinate_sum + (f64::from(offset_x) * length as f64);
                sum_y += (entry.sequence as f64 + f64::from(offset_y)) * length as f64;
            }
            Orientation::Vertical => {
                sum_x += (entry.sequence as f64 + f64::from(offset_x)) * length as f64;
                sum_y += coordinate_sum + (f64::from(offset_y) * length as f64);
            }
        }
    }
    let centroid_x = sum_x / weight as f64;
    let centroid_y = sum_y / weight as f64;
    GlyphComponent {
        ancestor_mark,
        left,
        top,
        width,
        height,
        weight,
        centroid_x,
        centroid_y,
        rounded_centroid_x: centroid_x.round_ties_even() as i32,
        rounded_centroid_y: centroid_y.round_ties_even() as i32,
        orientation,
        runs,
    }
}

fn arithmetic_sum(start: usize, stop: usize) -> f64 {
    ((start as f64 + stop as f64) * (stop - start + 1) as f64) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_table::{BACKGROUND, FOREGROUND};

    #[test]
    fn overlap_merges_marks_but_diagonal_contact_stays_separate() {
        let pixels = [
            FOREGROUND, BACKGROUND, FOREGROUND, BACKGROUND, BACKGROUND, FOREGROUND, FOREGROUND,
            FOREGROUND, BACKGROUND, BACKGROUND, BACKGROUND, BACKGROUND, BACKGROUND, FOREGROUND,
            BACKGROUND,
        ];
        let table = RunTable::from_pixels(Orientation::Horizontal, 5, 3, &pixels).unwrap();
        let components = build_glyph_components(&table, 0, 0);
        assert_eq!(components.len(), 2);
        assert_eq!(components[0].ancestor_mark, 1);
        assert_eq!((components[0].left, components[0].top), (0, 0));
        assert_eq!((components[0].width, components[0].height), (3, 2));
        assert_eq!(components[0].weight, 5);
        assert_eq!(components[1].ancestor_mark, 3);
        assert_eq!((components[1].left, components[1].top), (3, 2));
    }

    #[test]
    fn root_order_bounds_weight_and_ties_even_centroid_match_java() {
        let pixels = [
            BACKGROUND, FOREGROUND, FOREGROUND, BACKGROUND, BACKGROUND, FOREGROUND, FOREGROUND,
            BACKGROUND,
        ];
        let table = RunTable::from_pixels(Orientation::Horizontal, 4, 2, &pixels).unwrap();
        let components = build_glyph_components(&table, 10, 20);
        assert_eq!(components.len(), 1);
        let glyph = &components[0];
        assert_eq!(
            (glyph.left, glyph.top, glyph.width, glyph.height),
            (11, 20, 2, 2)
        );
        assert_eq!(glyph.weight, 4);
        assert_eq!((glyph.centroid_x, glyph.centroid_y), (11.5, 20.5));
        assert_eq!(
            (glyph.rounded_centroid_x, glyph.rounded_centroid_y),
            (12, 20)
        );
    }

    #[test]
    fn vertical_orientation_transposes_sequence_geometry() {
        let pixels = [FOREGROUND, FOREGROUND, BACKGROUND, FOREGROUND];
        let table = RunTable::from_pixels(Orientation::Vertical, 2, 2, &pixels).unwrap();
        let components = build_glyph_components(&table, 3, 4);
        assert_eq!(components.len(), 1);
        assert_eq!((components[0].left, components[0].top), (3, 4));
        assert_eq!((components[0].width, components[0].height), (2, 2));
        assert_eq!(components[0].weight, 3);
    }
}
