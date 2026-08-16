// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dense, phase-locked five-line tracing for warped staff geometry.
//!
//! GRID's Java-compatible filaments are intentionally sparse and their median
//! endpoint can stop before a curved terminal bar. This optional downstream
//! model keeps that product unchanged and derives a second, explicit
//! scan-coordinate representation suitable for measurement and rectification.

use crate::system_population::StaffBoundary;

#[derive(Clone, Debug, PartialEq)]
pub struct StaffGeometrySeed {
    pub staff_id: usize,
    pub left: i32,
    pub right: i32,
    pub interline: i32,
    pub lines: Vec<StaffBoundary>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaffTraceParameters {
    pub sample_step: i32,
    pub horizontal_probe_width: i32,
    pub vertical_search_radius: i32,
    pub spacing_search_radius: i32,
    pub maximum_extension: i32,
    pub minimum_pattern_score: f64,
    pub minimum_terminal_coverage: f64,
}

impl StaffTraceParameters {
    #[must_use]
    pub fn from_interline(interline: i32) -> Self {
        let interline = interline.max(1);
        Self {
            sample_step: (interline / 4).max(2),
            horizontal_probe_width: (interline / 3).max(2),
            vertical_search_radius: (2 * interline).max(4),
            spacing_search_radius: (interline / 5).max(1),
            maximum_extension: 15 * interline,
            minimum_pattern_score: 0.28,
            minimum_terminal_coverage: 0.60,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaffTraceColumn {
    pub x: f64,
    pub upper_y: f64,
    pub interline: f64,
    pub confidence: f64,
    pub terminal_coverage: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TracedStaffGeometry {
    pub staff_id: usize,
    pub seed_left: i32,
    pub seed_right: i32,
    pub left: i32,
    pub right: i32,
    pub mean_confidence: f64,
    pub terminal_bar_x: Option<i32>,
    pub columns: Vec<StaffTraceColumn>,
    pub lines: Vec<Vec<(f64, f64)>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct State {
    upper_y: i32,
    interline: i32,
}

#[derive(Clone, Debug)]
struct StateRow {
    states: Vec<State>,
    minimum_upper_y: i32,
    maximum_upper_y: i32,
    spacing_min: i32,
    spacing_max: i32,
}

impl StateRow {
    fn index_of(&self, upper_y: i32, interline: i32) -> Option<usize> {
        if upper_y < self.minimum_upper_y
            || upper_y > self.maximum_upper_y
            || interline < self.spacing_min
            || interline > self.spacing_max
        {
            return None;
        }
        let spacing_count = usize::try_from(self.spacing_max - self.spacing_min + 1).ok()?;
        let row = usize::try_from(upper_y - self.minimum_upper_y).ok()?;
        let column = usize::try_from(interline - self.spacing_min).ok()?;
        let index = row.checked_mul(spacing_count)?.checked_add(column)?;
        (self.states.get(index) == Some(&State { upper_y, interline })).then_some(index)
    }
}

#[derive(Clone, Copy, Debug)]
struct Cell {
    score: f64,
    previous: usize,
    emission: f64,
    coverage: f64,
}

/// Trace all five lines jointly in the original scan coordinate space.
///
/// The Viterbi state is upper-line y plus local interline. A seed-spline
/// penalty fixes the periodic phase while GRID has evidence; outside the seed
/// extent, local continuity carries that phase through curvature and occlusion.
#[must_use]
pub fn trace_staff_geometry(
    seed: &StaffGeometrySeed,
    parameters: StaffTraceParameters,
    width: usize,
    height: usize,
    pixels: &[u8],
) -> Option<TracedStaffGeometry> {
    if seed.lines.len() != 5
        || seed.interline <= 0
        || parameters.sample_step <= 0
        || parameters.horizontal_probe_width <= 0
        || width == 0
        || height == 0
        || pixels.len() != width.saturating_mul(height)
    {
        return None;
    }

    let raster_right = i32::try_from(width.saturating_sub(1)).ok()?;
    let start_x = seed
        .left
        .saturating_sub(parameters.maximum_extension)
        .max(0);
    let stop_x = seed
        .right
        .saturating_add(parameters.maximum_extension)
        .min(raster_right);
    let mut xs = Vec::new();
    let mut x = start_x;
    while x <= stop_x {
        xs.push(x);
        let next = x.saturating_add(parameters.sample_step);
        if next == x {
            break;
        }
        x = next;
    }
    if xs.last().copied() != Some(stop_x) {
        xs.push(stop_x);
    }
    if xs.len() < 2 {
        return None;
    }

    let spacing_min = (seed.interline - parameters.spacing_search_radius).max(2);
    let spacing_max = seed.interline + parameters.spacing_search_radius;
    let mut rows = Vec::<StateRow>::with_capacity(xs.len());
    for &column_x in &xs {
        let predicted = seed.lines[0].y_at_x_ext(f64::from(column_x));
        let center = predicted.round_ties_even() as i32;
        let minimum_upper_y = (center - parameters.vertical_search_radius).max(0);
        let maximum_upper_y =
            (center + parameters.vertical_search_radius).min(height as i32 - (4 * spacing_max) - 1);
        let mut states = Vec::new();
        for upper_y in minimum_upper_y..=maximum_upper_y {
            for interline in spacing_min..=spacing_max {
                states.push(State { upper_y, interline });
            }
        }
        if states.is_empty() {
            return None;
        }
        rows.push(StateRow {
            states,
            minimum_upper_y,
            maximum_upper_y,
            spacing_min,
            spacing_max,
        });
    }

    let mut lattice = Vec::<Vec<Cell>>::with_capacity(xs.len());
    for (column_index, (&column_x, row)) in xs.iter().zip(&rows).enumerate() {
        let seed_y = seed.lines[0].y_at_x_ext(f64::from(column_x));
        let inside_seed = seed.left <= column_x && column_x <= seed.right;
        let mut cells = Vec::with_capacity(row.states.len());
        for (state_index, &state) in row.states.iter().enumerate() {
            let emission = pattern_score(
                column_x,
                state,
                parameters.horizontal_probe_width,
                width,
                height,
                pixels,
            );
            let coverage = terminal_coverage(column_x, state, width, height, pixels);
            let phase_delta =
                (f64::from(state.upper_y) - seed_y) / f64::from(seed.interline.max(1));
            let spacing_delta =
                f64::from(state.interline - seed.interline) / f64::from(seed.interline);
            let seed_penalty = if inside_seed {
                (0.55 * phase_delta * phase_delta) + (0.25 * spacing_delta * spacing_delta)
            } else {
                0.04 * spacing_delta * spacing_delta
            };
            if column_index == 0 {
                cells.push(Cell {
                    score: (3.0 * emission) - seed_penalty,
                    previous: state_index,
                    emission,
                    coverage,
                });
                continue;
            }

            let previous_row = &rows[column_index - 1];
            let previous_cells = &lattice[column_index - 1];
            let previous_seed_y = seed.lines[0].y_at_x_ext(f64::from(xs[column_index - 1]));
            let seed_dy = seed_y - previous_seed_y;
            let mut best_score = f64::NEG_INFINITY;
            let mut best_previous = 0;
            let maximum_dy = parameters.sample_step + 2;
            for previous_y in state.upper_y - maximum_dy..=state.upper_y + maximum_dy {
                for previous_spacing in state.interline - 1..=state.interline + 1 {
                    let Some(previous_index) = previous_row.index_of(previous_y, previous_spacing)
                    else {
                        continue;
                    };
                    let dy = state.upper_y - previous_y;
                    let slope_penalty = 0.16 * (f64::from(dy) - seed_dy).abs();
                    let spacing_penalty =
                        0.18 * f64::from((state.interline - previous_spacing).abs());
                    let score =
                        previous_cells[previous_index].score - slope_penalty - spacing_penalty;
                    if score > best_score {
                        best_score = score;
                        best_previous = previous_index;
                    }
                }
            }
            if !best_score.is_finite() {
                best_score = lattice[column_index - 1]
                    .iter()
                    .map(|cell| cell.score)
                    .fold(f64::NEG_INFINITY, f64::max);
            }
            cells.push(Cell {
                score: best_score + (3.0 * emission) - seed_penalty,
                previous: best_previous,
                emission,
                coverage,
            });
        }
        lattice.push(cells);
    }

    let mut state_indices = vec![0_usize; xs.len()];
    state_indices[xs.len() - 1] = lattice[xs.len() - 1]
        .iter()
        .enumerate()
        .max_by(|(_, one), (_, two)| one.score.total_cmp(&two.score))
        .map(|(index, _)| index)?;
    for index in (1..xs.len()).rev() {
        state_indices[index - 1] = lattice[index][state_indices[index]].previous;
    }
    let extent_state_indices = state_indices.clone();

    let all_columns = xs
        .iter()
        .enumerate()
        .map(|(index, &column_x)| {
            let state = rows[index].states[state_indices[index]];
            let cell = lattice[index][state_indices[index]];
            StaffTraceColumn {
                x: f64::from(column_x),
                upper_y: f64::from(state.upper_y),
                interline: f64::from(state.interline),
                confidence: cell.emission,
                terminal_coverage: cell.coverage,
            }
        })
        .collect::<Vec<_>>();
    // Extent evidence comes from the globally optimal path; the anchored path
    // above is intentionally conservative about phase changes and can cross a
    // short low-contrast patch without implying that the physical staff ends.
    let extent_columns = xs
        .iter()
        .enumerate()
        .map(|(index, &column_x)| {
            let state = rows[index].states[extent_state_indices[index]];
            let cell = lattice[index][extent_state_indices[index]];
            StaffTraceColumn {
                x: f64::from(column_x),
                upper_y: f64::from(state.upper_y),
                interline: f64::from(state.interline),
                confidence: cell.emission,
                terminal_coverage: cell.coverage,
            }
        })
        .collect::<Vec<_>>();

    let seed_start = all_columns
        .iter()
        .position(|column| column.x >= f64::from(seed.left))?;
    let seed_stop = all_columns
        .iter()
        .rposition(|column| column.x <= f64::from(seed.right))?;
    let gap_budget = ((2 * seed.interline) / parameters.sample_step).max(2) as usize;
    let left_index = connected_extent(
        &extent_columns,
        seed_start,
        parameters.minimum_pattern_score,
        gap_budget,
        false,
    );
    let right_index = connected_extent(
        &extent_columns,
        seed_stop,
        parameters.minimum_pattern_score,
        gap_budget,
        true,
    );
    let terminal_index = all_columns[seed_stop..=right_index]
        .iter()
        .enumerate()
        .filter(|(_, column)| column.terminal_coverage >= parameters.minimum_terminal_coverage)
        .map(|(offset, _)| seed_stop + offset)
        .last()
        .filter(|index| right_index.saturating_sub(*index) <= 2);
    if left_index > right_index {
        return None;
    }
    let columns = all_columns[left_index..=right_index].to_vec();
    let mean_confidence =
        columns.iter().map(|column| column.confidence).sum::<f64>() / columns.len() as f64;
    let lines = (0..5)
        .map(|line_index| {
            columns
                .iter()
                .map(|column| {
                    (
                        column.x,
                        column.upper_y + (line_index as f64 * column.interline),
                    )
                })
                .collect()
        })
        .collect();

    Some(TracedStaffGeometry {
        staff_id: seed.staff_id,
        seed_left: seed.left,
        seed_right: seed.right,
        left: columns.first()?.x.round_ties_even() as i32,
        right: columns.last()?.x.round_ties_even() as i32,
        mean_confidence,
        terminal_bar_x: terminal_index.map(|index| all_columns[index].x.round_ties_even() as i32),
        columns,
        lines,
    })
}

fn connected_extent(
    columns: &[StaffTraceColumn],
    seed_index: usize,
    threshold: f64,
    gap_budget: usize,
    right: bool,
) -> usize {
    let mut boundary = seed_index;
    let mut misses = 0;
    if right {
        for (index, column) in columns.iter().enumerate().skip(seed_index + 1) {
            if column.confidence >= threshold {
                boundary = index;
                misses = 0;
            } else {
                misses += 1;
                if misses > gap_budget {
                    break;
                }
            }
        }
    } else {
        for index in (0..seed_index).rev() {
            if columns[index].confidence >= threshold {
                boundary = index;
                misses = 0;
            } else {
                misses += 1;
                if misses > gap_budget {
                    break;
                }
            }
        }
    }
    boundary
}

fn pattern_score(
    x: i32,
    state: State,
    probe_width: i32,
    width: usize,
    height: usize,
    pixels: &[u8],
) -> f64 {
    let mut matches = 0_usize;
    let mut trials = 0_usize;
    for line in 0..5 {
        let middle = state.upper_y + (line * state.interline);
        for y in middle - 1..=middle + 1 {
            for probe_x in x..x.saturating_add(probe_width) {
                trials += 1;
                if probe_x >= 0
                    && y >= 0
                    && (probe_x as usize) < width
                    && (y as usize) < height
                    && pixels[y as usize * width + probe_x as usize] == 0
                {
                    matches += 1;
                }
            }
        }
    }
    matches as f64 / trials.max(1) as f64
}

fn terminal_coverage(x: i32, state: State, width: usize, height: usize, pixels: &[u8]) -> f64 {
    if x < 0 || x as usize >= width {
        return 0.0;
    }
    let top = (state.upper_y - 2).max(0) as usize;
    let bottom =
        (state.upper_y + (4 * state.interline) + 2).min(height.saturating_sub(1) as i32) as usize;
    if bottom <= top {
        return 0.0;
    }
    let black = (top..=bottom)
        .filter(|&y| pixels[y * width + x as usize] == 0)
        .count();
    black as f64 / (bottom - top + 1) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_population::boundary_from_points;

    #[test]
    fn traces_a_curved_five_line_staff_through_a_terminal_bar() {
        let width = 260_usize;
        let height = 120_usize;
        let mut pixels = vec![255_u8; width * height];
        let curve = |x: usize| 20.0 + 10.0 * (x as f64 / 240.0).powi(3);
        for x in 12..=240 {
            for line in 0..5 {
                let y = (curve(x) + (line * 12) as f64).round() as usize;
                pixels[y * width + x] = 0;
                pixels[(y + 1) * width + x] = 0;
            }
        }
        for x in 238..=242 {
            for y in curve(240).round() as usize..=curve(240).round() as usize + 49 {
                pixels[y * width + x] = 0;
            }
        }
        let lines = (0..5)
            .map(|line| {
                boundary_from_points(&[
                    (20.0, curve(20) + f64::from(line * 12)),
                    (180.0, curve(180) + f64::from(line * 12)),
                ])
                .unwrap()
            })
            .collect();
        let seed = StaffGeometrySeed {
            staff_id: 1,
            left: 20,
            right: 180,
            interline: 12,
            lines,
        };
        let traced = trace_staff_geometry(
            &seed,
            StaffTraceParameters::from_interline(12),
            width,
            height,
            &pixels,
        )
        .unwrap();
        assert!(traced.right >= 238, "{traced:?}");
        assert_eq!(traced.terminal_bar_x, Some(240));
        let last = traced.lines[0].last().unwrap();
        assert!((last.1 - curve(last.0 as usize)).abs() <= 2.0, "{last:?}");
    }
}
