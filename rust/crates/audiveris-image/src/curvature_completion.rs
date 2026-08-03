// SPDX-License-Identifier: AGPL-3.0-or-later

//! Concrete `LinesRetriever.polishCurvatures` over prepared staff filaments.

use std::convert::Infallible;
use std::error::Error;
use std::fmt;

use crate::{
    filament::FilamentError,
    grid_lifecycle::GridStageFailure,
    line_completion::{
        CurvaturePoint, CurvaturePolishError, CurvatureSection, CurvedFilamentPolishState,
        LineCompletionStage, OrientedSectionBounds, curvature_removal_candidate,
    },
    prepared_completion::{PreparedCompletionState, RemainingLineCompletionStages},
    run_table::Orientation,
};

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedCurvatureRemoval {
    pub staff_id: usize,
    pub line_id: usize,
    pub section_id: usize,
    pub radius_point_index: usize,
    pub probe_point_index: usize,
    pub radius: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedCurvatureRecomputation {
    pub staff_id: usize,
    pub line_id: usize,
    pub preserved_start: (f64, f64),
    pub preserved_stop: (f64, f64),
}

#[derive(Clone, Debug, PartialEq)]
pub enum PolishCurvaturesError {
    InvalidMinimumRadius(i32),
    MissingSelectedSection {
        line_id: usize,
        section_id: usize,
    },
    InvalidPointCount {
        line_id: usize,
        count: usize,
    },
    Filament {
        staff_id: usize,
        line_id: usize,
        source: FilamentError,
    },
}

impl fmt::Display for PolishCurvaturesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMinimumRadius(radius) => {
                write!(
                    formatter,
                    "minimum curvature radius must be nonnegative, got {radius}"
                )
            }
            Self::MissingSelectedSection {
                line_id,
                section_id,
            } => write!(
                formatter,
                "line {line_id} lost selected curvature section {section_id}"
            ),
            Self::InvalidPointCount { line_id, count } => write!(
                formatter,
                "line {line_id} curvature geometry has only {count} defining points"
            ),
            Self::Filament {
                staff_id,
                line_id,
                source,
            } => write!(
                formatter,
                "staff {staff_id} line {line_id} curvature polishing failed: {source}"
            ),
        }
    }
}

impl Error for PolishCurvaturesError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Filament { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PolishCurvaturesCompletionError<NextError> {
    PolishCurvatures(PolishCurvaturesError),
    Next(NextError),
}

impl<NextError: fmt::Display> fmt::Display for PolishCurvaturesCompletionError<NextError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PolishCurvatures(error) => error.fmt(formatter),
            Self::Next(error) => error.fmt(formatter),
        }
    }
}

impl<NextError: Error + 'static> Error for PolishCurvaturesCompletionError<NextError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::PolishCurvatures(error) => Some(error),
            Self::Next(error) => Some(error),
        }
    }
}

pub struct PolishCurvaturesCompletion<Next> {
    minimum_radius: i32,
    next: Next,
}

impl<Next> PolishCurvaturesCompletion<Next> {
    #[must_use]
    pub const fn new(minimum_radius: i32, next: Next) -> Self {
        Self {
            minimum_radius,
            next,
        }
    }

    #[must_use]
    pub const fn next(&self) -> &Next {
        &self.next
    }
}

impl<Next> RemainingLineCompletionStages for PolishCurvaturesCompletion<Next>
where
    Next: RemainingLineCompletionStages,
{
    type StepError = Next::StepError;
    type OtherError = PolishCurvaturesCompletionError<Next::OtherError>;

    fn run_stage(
        &mut self,
        stage: LineCompletionStage,
        state: &mut PreparedCompletionState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        if stage == LineCompletionStage::PolishCurvatures {
            polish_prepared_curvatures(state, self.minimum_radius).map_err(|error| {
                GridStageFailure::Other(PolishCurvaturesCompletionError::PolishCurvatures(error))
            })
        } else {
            self.next
                .run_stage(stage, state)
                .map_err(|failure| match failure {
                    GridStageFailure::Step(error) => GridStageFailure::Step(error),
                    GridStageFailure::Other(error) => {
                        GridStageFailure::Other(PolishCurvaturesCompletionError::Next(error))
                    }
                })
        }
    }

    fn log_swallowed_error(&mut self, error: &Self::OtherError) {
        if let PolishCurvaturesCompletionError::Next(error) = error {
            self.next.log_swallowed_error(error);
        }
    }

    fn finish(&mut self) {
        self.next.finish();
    }
}

pub fn polish_prepared_curvatures(
    state: &mut PreparedCompletionState,
    minimum_radius: i32,
) -> Result<(), PolishCurvaturesError> {
    if minimum_radius < 0 {
        return Err(PolishCurvaturesError::InvalidMinimumRadius(minimum_radius));
    }
    state.curvature_removals.clear();
    state.curvature_recomputations.clear();
    let (staffs, removals, recomputations) = (
        &mut state.staffs,
        &mut state.curvature_removals,
        &mut state.curvature_recomputations,
    );

    for staff in staffs {
        for line in &mut staff.lines {
            let initial =
                line.filament
                    .geometry()
                    .map_err(|source| PolishCurvaturesError::Filament {
                        staff_id: staff.id,
                        line_id: line.id,
                        source,
                    })?;
            let old_start = initial.start();
            let old_stop = initial.stop();
            let mut needs_recomputation = false;

            loop {
                if needs_recomputation {
                    line.filament
                        .set_ending_points(old_start, old_stop)
                        .map_err(|source| PolishCurvaturesError::Filament {
                            staff_id: staff.id,
                            line_id: line.id,
                            source,
                        })?;
                    recomputations.push(PreparedCurvatureRecomputation {
                        staff_id: staff.id,
                        line_id: line.id,
                        preserved_start: old_start,
                        preserved_stop: old_stop,
                    });
                }

                let geometry =
                    line.filament
                        .geometry()
                        .map_err(|source| PolishCurvaturesError::Filament {
                            staff_id: staff.id,
                            line_id: line.id,
                            source,
                        })?;
                let kernel_state = CurvedFilamentPolishState {
                    orientation: Orientation::Horizontal,
                    interline: line.filament.interline() as i32,
                    start_point: as_curvature_point(geometry.start()),
                    stop_point: as_curvature_point(geometry.stop()),
                    points: Some(
                        geometry
                            .points()
                            .iter()
                            .copied()
                            .map(as_curvature_point)
                            .collect(),
                    ),
                    members: line
                        .filament
                        .sections()
                        .iter()
                        .map(|section| {
                            let bounds = section.bounds();
                            let centroid = section.centroid();
                            CurvatureSection {
                                id: section.id(),
                                oriented_bounds: OrientedSectionBounds {
                                    x: bounds.x as f64,
                                    y: bounds.y as f64,
                                    width: bounds.width as f64,
                                    height: bounds.height as f64,
                                },
                                centroid: CurvaturePoint {
                                    x: centroid.0 as f64,
                                    y: centroid.1 as f64,
                                },
                            }
                        })
                        .collect(),
                };
                let Some(removal) =
                    curvature_removal_candidate::<Infallible>(&kernel_state, minimum_radius)
                        .map_err(|error| match error {
                            CurvaturePolishError::InvalidPointCount { count } => {
                                PolishCurvaturesError::InvalidPointCount {
                                    line_id: line.id,
                                    count,
                                }
                            }
                            CurvaturePolishError::Recompute(never) => match never {},
                        })?
                else {
                    break;
                };

                if !line.filament.remove_section_by_id(removal.section_id) {
                    return Err(PolishCurvaturesError::MissingSelectedSection {
                        line_id: line.id,
                        section_id: removal.section_id,
                    });
                }
                removals.push(PreparedCurvatureRemoval {
                    staff_id: staff.id,
                    line_id: line.id,
                    section_id: removal.section_id,
                    radius_point_index: removal.radius_point_index,
                    probe_point_index: removal.probe_point_index,
                    radius: removal.radius,
                });
                needs_recomputation = true;
            }
        }
    }
    Ok(())
}

fn as_curvature_point(point: (f64, f64)) -> CurvaturePoint {
    CurvaturePoint {
        x: point.0,
        y: point.1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        discarded_completion::PreparedCompletionSystem,
        line_endpoints::PreparedStaffEndPoints,
        lines_coordinator::StaffCandidateKind,
        prepared_lines::{PreparedStaff, PreparedStaffLine},
        run_table::{Run, RunTable},
        section::{JunctionPolicy, build_sections_from_id},
    };

    fn state() -> PreparedCompletionState {
        let mut table = RunTable::new(Orientation::Horizontal, 101, 21).unwrap();
        table.add_run(10, Run::new(0, 81)).unwrap();
        table.add_run(10, Run::new(90, 11)).unwrap();
        let mut filament = crate::filament::StaffFilament::new(10).unwrap();
        for section in build_sections_from_id(&table, JunctionPolicy::All, 100) {
            filament.add_section(section).unwrap();
        }
        filament
            .set_ending_points((0.0, 10.0), (100.0, 10.0))
            .unwrap();
        filament
            .replace_defining_points(vec![(0.0, 10.0), (50.0, 0.0), (100.0, 10.0)])
            .unwrap();
        PreparedCompletionState {
            staffs: vec![PreparedStaff {
                id: 5,
                kind: StaffCandidateKind::OneLine,
                left: 0.0,
                right: 100.0,
                interline: 10,
                small: false,
                short: false,
                lines: vec![PreparedStaffLine {
                    id: 7,
                    cluster_position: 0,
                    filament,
                }],
            }],
            completion_systems: Some(vec![PreparedCompletionSystem {
                system_id: 1,
                staff_ids: vec![5],
            }]),
            discarded_filaments: Vec::new(),
            horizontal_sections: Vec::new(),
            binary_buffer: None,
            thick_section_ids: Vec::new(),
            thin_section_ids: Vec::new(),
            defined_endpoints: Vec::<PreparedStaffEndPoints>::new(),
            fill_hole_invocations: Vec::new(),
            discarded_filament_steals: Vec::new(),
            discarded_filament_recomputations: Vec::new(),
            section_inclusion_batches: Vec::new(),
            sticker_section_ids: Vec::new(),
            sticker_inclusion_batches: Vec::new(),
            curvature_removals: Vec::new(),
            curvature_recomputations: Vec::new(),
            completed_stages: Vec::new(),
        }
    }

    #[test]
    fn removes_endpoint_section_then_recomputes_once_with_old_endpoints() {
        let mut state = state();
        polish_prepared_curvatures(&mut state, 200).unwrap();
        assert_eq!(state.curvature_removals.len(), 1);
        // With three points Java's first-endpoint branch wins the else-if.
        assert_eq!(state.curvature_removals[0].section_id, 100);
        assert_eq!(state.curvature_removals[0].probe_point_index, 0);
        assert_eq!(state.curvature_recomputations.len(), 1);
        assert_eq!(
            state.staffs[0].lines[0].filament.ending_points(),
            Some(((0.0, 10.0), (100.0, 10.0)))
        );
    }

    #[test]
    fn recompute_failure_retains_removed_section_prefix_and_installed_endpoints() {
        let mut state = state();
        let filament = &mut state.staffs[0].lines[0].filament;
        assert!(filament.remove_section_by_id(101));
        filament
            .set_ending_points((0.0, 10.0), (100.0, 10.0))
            .unwrap();
        filament
            .replace_defining_points(vec![(0.0, 10.0), (50.0, 0.0), (100.0, 10.0)])
            .unwrap();

        let error = polish_prepared_curvatures(&mut state, 200).unwrap_err();
        assert!(matches!(
            error,
            PolishCurvaturesError::Filament {
                source: FilamentError::Empty,
                ..
            }
        ));
        assert_eq!(state.curvature_removals.len(), 1);
        assert_eq!(state.curvature_removals[0].section_id, 100);
        assert!(state.curvature_recomputations.is_empty());
        assert!(state.staffs[0].lines[0].filament.sections().is_empty());
        assert_eq!(
            state.staffs[0].lines[0].filament.ending_points(),
            Some(((0.0, 10.0), (100.0, 10.0)))
        );
    }

    #[derive(Default)]
    struct Tail(Vec<LineCompletionStage>);

    impl RemainingLineCompletionStages for Tail {
        type StepError = ();
        type OtherError = Infallible;

        fn run_stage(
            &mut self,
            stage: LineCompletionStage,
            _: &mut PreparedCompletionState,
        ) -> Result<(), GridStageFailure<(), Infallible>> {
            self.0.push(stage);
            Ok(())
        }

        fn log_swallowed_error(&mut self, _: &Infallible) {}
        fn finish(&mut self) {}
    }

    #[test]
    fn wrapper_owns_polish_but_delegates_following_fill_holes() {
        let mut completion = PolishCurvaturesCompletion::new(200, Tail::default());
        let mut state = state();
        completion
            .run_stage(LineCompletionStage::PolishCurvatures, &mut state)
            .unwrap();
        completion
            .run_stage(LineCompletionStage::FillHolesAfterPolish, &mut state)
            .unwrap();
        assert_eq!(
            completion.next().0,
            [LineCompletionStage::FillHolesAfterPolish]
        );
    }
}
