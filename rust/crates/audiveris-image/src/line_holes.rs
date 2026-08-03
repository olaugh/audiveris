// SPDX-License-Identifier: AGPL-3.0-or-later

//! Concrete initial `StaffFilament.fillHoles` completion stage.

use std::error::Error;
use std::fmt;

use crate::{
    filament::{FilamentError, HoleNeighbor, interpolate_hole_point, plan_hole_fills},
    grid_lifecycle::GridStageFailure,
    line_completion::LineCompletionStage,
    prepared_completion::{PreparedCompletionState, RemainingLineCompletionStages},
    prepared_lines::PreparedStaffLine,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoleInsertionSource {
    NeighborInterpolation,
    CurrentSplineFallback,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedHoleInsertion {
    pub staff_id: usize,
    pub line_id: usize,
    pub preferred_x: i32,
    pub point: (f64, f64),
    pub source: HoleInsertionSource,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FillHolesError {
    Filament {
        staff_id: usize,
        line_id: usize,
        source: FilamentError,
    },
}

impl fmt::Display for FillHolesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Filament {
                staff_id,
                line_id,
                source,
            } => write!(
                formatter,
                "staff {staff_id} line {line_id} hole filling failed: {source}"
            ),
        }
    }
}

impl Error for FillHolesError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Filament { source, .. } => Some(source),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FillHolesCompletionError<NextError> {
    FillHoles(FillHolesError),
    Next(NextError),
}

impl<NextError: fmt::Display> fmt::Display for FillHolesCompletionError<NextError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FillHoles(error) => error.fmt(formatter),
            Self::Next(error) => error.fmt(formatter),
        }
    }
}

impl<NextError: Error + 'static> Error for FillHolesCompletionError<NextError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::FillHoles(error) => Some(error),
            Self::Next(error) => Some(error),
        }
    }
}

/// Additive collaborator that owns only the first Java `fillHoles` call.
pub struct FillHolesInitialCompletion<Next> {
    next: Next,
}

impl<Next> FillHolesInitialCompletion<Next> {
    #[must_use]
    pub const fn new(next: Next) -> Self {
        Self { next }
    }

    #[must_use]
    pub const fn next(&self) -> &Next {
        &self.next
    }
}

impl<Next> RemainingLineCompletionStages for FillHolesInitialCompletion<Next>
where
    Next: RemainingLineCompletionStages,
{
    type StepError = Next::StepError;
    type OtherError = FillHolesCompletionError<Next::OtherError>;

    fn run_stage(
        &mut self,
        stage: LineCompletionStage,
        state: &mut PreparedCompletionState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        if stage == LineCompletionStage::FillHolesInitial {
            fill_holes_initial(state).map_err(|error| {
                GridStageFailure::Other(FillHolesCompletionError::FillHoles(error))
            })
        } else {
            self.next
                .run_stage(stage, state)
                .map_err(|failure| match failure {
                    GridStageFailure::Step(error) => GridStageFailure::Step(error),
                    GridStageFailure::Other(error) => {
                        GridStageFailure::Other(FillHolesCompletionError::Next(error))
                    }
                })
        }
    }

    fn log_swallowed_error(&mut self, error: &Self::OtherError) {
        if let FillHolesCompletionError::Next(error) = error {
            self.next.log_swallowed_error(error);
        }
    }

    fn finish(&mut self) {
        self.next.finish();
    }
}

pub fn fill_holes_initial(state: &mut PreparedCompletionState) -> Result<(), FillHolesError> {
    state.fill_hole_insertions.clear();
    for staff in &mut state.staffs {
        for position in 0..staff.lines.len() {
            let (above, current_and_below) = staff.lines.split_at_mut(position);
            let (current, below) = current_and_below
                .split_first_mut()
                .expect("position is within staff lines");
            let staff_id = staff.id;
            let line_id = current.id;
            fill_line_holes(
                staff_id,
                current,
                above,
                below,
                &mut state.fill_hole_insertions,
            )
            .map_err(|source| FillHolesError::Filament {
                staff_id,
                line_id,
                source,
            })?;
        }
    }
    Ok(())
}

fn fill_line_holes(
    staff_id: usize,
    current: &mut PreparedStaffLine,
    above: &[PreparedStaffLine],
    below: &[PreparedStaffLine],
    audit: &mut Vec<PreparedHoleInsertion>,
) -> Result<(), FilamentError> {
    let max_hole_length = (current.filament.interline() as f64 * 6.0).round_ties_even() as i32;
    let virtual_length = (current.filament.interline() as f64 * 5.0).round_ties_even() as i32;
    let margin = virtual_length / 2;
    let geometry = current.filament.geometry()?;
    let plans = plan_hole_fills(
        &geometry
            .points()
            .iter()
            .map(|point| point.0)
            .collect::<Vec<_>>(),
        max_hole_length,
        virtual_length,
    )?;
    if plans.is_empty() {
        return Ok(());
    }

    let mut points = geometry.points().to_vec();
    let mut inserted = 0;
    for plan in plans {
        for preferred_x in plan.abscissae {
            let above_neighbors = neighbor_points(above, preferred_x, margin)?;
            let below_neighbors = neighbor_points(below, preferred_x, margin)?;
            let (point, source) = interpolate_hole_point(
                current.cluster_position,
                &above_neighbors,
                &below_neighbors,
            )
            .map_or_else(
                || {
                    geometry.position_at(f64::from(preferred_x)).map(|y| {
                        (
                            (f64::from(preferred_x), y),
                            HoleInsertionSource::CurrentSplineFallback,
                        )
                    })
                },
                |point| Ok((point, HoleInsertionSource::NeighborInterpolation)),
            )?;
            points.insert(plan.before_point_index + inserted, point);
            inserted += 1;
            // Java mutates the point list immediately; a later lookup or
            // spline failure must retain this exact insertion prefix.
            current.filament.mutate_defining_points(points.clone())?;
            audit.push(PreparedHoleInsertion {
                staff_id,
                line_id: current.id,
                preferred_x,
                point,
                source,
            });
        }
    }
    current.filament.regenerate_defining_spline()?;
    Ok(())
}

fn neighbor_points(
    lines: &[PreparedStaffLine],
    coordinate: i32,
    margin: i32,
) -> Result<Vec<Option<HoleNeighbor>>, FilamentError> {
    lines
        .iter()
        .map(|line| {
            line.filament
                .find_defining_point(coordinate, margin)
                .map(|point| {
                    point.map(|point| HoleNeighbor {
                        cluster_position: line.cluster_position,
                        point,
                    })
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        line_completion::LineCompletionStage,
        lines_coordinator::StaffCandidateKind,
        prepared_completion::PreparedCompletionState,
        prepared_lines::PreparedStaff,
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections},
    };

    fn line(id: usize, cluster_position: i32, y: usize) -> PreparedStaffLine {
        let mut table = RunTable::new(Orientation::Horizontal, 101, y + 2).unwrap();
        table.add_run(y, Run::new(0, 101)).unwrap();
        let mut filament = crate::filament::StaffFilament::new(10).unwrap();
        for section in build_sections(&table, JunctionPolicy::All) {
            filament.add_section(section).unwrap();
        }
        filament
            .set_ending_points((0.0, y as f64), (100.0, y as f64))
            .unwrap();
        filament
            .replace_defining_points(vec![(0.0, y as f64), (100.0, y as f64)])
            .unwrap();
        PreparedStaffLine {
            id,
            cluster_position,
            filament,
        }
    }

    fn state(lines: Vec<PreparedStaffLine>) -> PreparedCompletionState {
        PreparedCompletionState {
            staffs: vec![PreparedStaff {
                id: 9,
                kind: StaffCandidateKind::Standard,
                left: 0.0,
                right: 100.0,
                interline: 10,
                small: false,
                short: false,
                lines,
            }],
            completion_systems: None,
            discarded_filaments: Vec::new(),
            horizontal_sections: Vec::new(),
            binary_buffer: None,
            thick_section_ids: Vec::new(),
            thin_section_ids: Vec::new(),
            defined_endpoints: Vec::new(),
            fill_hole_insertions: Vec::new(),
            discarded_filament_steals: Vec::new(),
            discarded_filament_recomputations: Vec::new(),
            section_inclusion_batches: Vec::new(),
            curvature_removals: Vec::new(),
            curvature_recomputations: Vec::new(),
            completed_stages: Vec::new(),
        }
    }

    #[test]
    fn interpolates_from_nearest_lines_by_cluster_position() {
        let mut bottom = line(3, 5, 27);
        bottom
            .filament
            .replace_defining_points(vec![(0.0, 27.0), (50.0, 27.0), (100.0, 27.0)])
            .unwrap();
        let mut state = state(vec![line(1, 0, 2), line(2, 2, 12), bottom]);
        fill_holes_initial(&mut state).unwrap();

        let middle = &state.staffs[0].lines[1].filament;
        assert_eq!(
            middle.defining_points().unwrap(),
            vec![(0.0, 12.0), (50.0, 12.0), (100.0, 12.0)]
        );
        assert_eq!(
            state.fill_hole_insertions[1].source,
            HoleInsertionSource::NeighborInterpolation
        );
    }

    #[test]
    fn falls_back_to_current_spline_without_two_neighbors() {
        let mut state = state(vec![line(7, 0, 8)]);
        fill_holes_initial(&mut state).unwrap();
        assert_eq!(
            state.staffs[0].lines[0].filament.defining_points().unwrap(),
            vec![(0.0, 8.0), (50.0, 8.0), (100.0, 8.0)]
        );
        assert_eq!(
            state.fill_hole_insertions[0].source,
            HoleInsertionSource::CurrentSplineFallback
        );
    }

    #[derive(Default)]
    struct Tail(Vec<LineCompletionStage>);

    impl RemainingLineCompletionStages for Tail {
        type StepError = ();
        type OtherError = std::convert::Infallible;

        fn run_stage(
            &mut self,
            stage: LineCompletionStage,
            _: &mut PreparedCompletionState,
        ) -> Result<(), GridStageFailure<(), std::convert::Infallible>> {
            self.0.push(stage);
            Ok(())
        }

        fn log_swallowed_error(&mut self, _: &std::convert::Infallible) {}
        fn finish(&mut self) {}
    }

    #[test]
    fn wrapper_owns_only_initial_fill_holes_stage() {
        let mut completion = FillHolesInitialCompletion::new(Tail::default());
        let mut state = state(vec![line(7, 0, 8)]);
        completion
            .run_stage(LineCompletionStage::FillHolesInitial, &mut state)
            .unwrap();
        completion
            .run_stage(LineCompletionStage::FillHolesAfterPolish, &mut state)
            .unwrap();
        assert_eq!(
            completion.next().0,
            vec![LineCompletionStage::FillHolesAfterPolish]
        );
    }
}
