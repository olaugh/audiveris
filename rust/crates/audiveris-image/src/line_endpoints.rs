// SPDX-License-Identifier: AGPL-3.0-or-later

//! Concrete Java `LinesRetriever.defineEndPoints` completion collaborator.
//!
//! The stage derives staff mean spacing and ending slopes from prepared
//! filaments, extrapolates nearby endpoints, and uses the live binary raster's
//! `StaffPattern` fit for missing endpoints. Results are committed one staff at
//! a time, preserving Java's completed-staff prefix when later geometry fails.

use std::error::Error;
use std::fmt;

use crate::{
    filament::{FilamentError, FilamentGeometry},
    grid_lifecycle::GridStageFailure,
    line_completion::LineCompletionStage,
    prepared_completion::{PreparedCompletionState, RemainingLineCompletionStages},
    prepared_lines::PreparedStaff,
    staff_pattern::{
        EndpointRetrieval, EndpointRetrievalError, EndpointRetrievalParams, LineEnding,
        StaffPattern, retrieve_line_endpoints,
    },
    staff_peak::HorizontalSide,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DefineEndPointsParameters {
    /// Java `scale.getInterline()`, also the horizontal sampling step.
    pub scale_interline: i32,
    /// Java `scale.getFore()` used by `StaffPattern`.
    pub foreground_thickness: i32,
    pub maximum_ending_dx: i32,
    pub pattern_width: i32,
    pub pattern_jitter: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedLineEndPoints {
    pub line_id: usize,
    pub left: LineEnding,
    pub right: LineEnding,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedStaffEndPoints {
    pub staff_id: usize,
    pub mean_interline: f64,
    pub left_ending_slope: f64,
    pub right_ending_slope: f64,
    pub left_pattern_fit: Option<(i32, f64, i32, f64)>,
    pub right_pattern_fit: Option<(i32, f64, i32, f64)>,
    pub lines: Vec<PreparedLineEndPoints>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DefineEndPointsError {
    MissingBinaryBuffer,
    InvalidParameters(DefineEndPointsParameters),
    EmptyStaff(usize),
    EmptyMeanInterlinePopulation(usize),
    Filament {
        staff_id: usize,
        line_id: usize,
        source: FilamentError,
    },
    Retrieval {
        staff_id: usize,
        side: HorizontalSide,
        source: EndpointRetrievalError,
    },
}

impl fmt::Display for DefineEndPointsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBinaryBuffer => {
                formatter.write_str("defineEndPoints requires the loaded binary buffer")
            }
            Self::InvalidParameters(parameters) => {
                write!(
                    formatter,
                    "invalid defineEndPoints parameters: {parameters:?}"
                )
            }
            Self::EmptyStaff(id) => write!(formatter, "staff {id} has no prepared lines"),
            Self::EmptyMeanInterlinePopulation(id) => write!(
                formatter,
                "staff {id} has no mean-interline samples between its abscissae"
            ),
            Self::Filament {
                staff_id,
                line_id,
                source,
            } => write!(
                formatter,
                "staff {staff_id} line {line_id} geometry failed: {source}"
            ),
            Self::Retrieval {
                staff_id,
                side,
                source,
            } => write!(
                formatter,
                "staff {staff_id} {side:?} endpoint retrieval failed: {source:?}"
            ),
        }
    }
}

impl Error for DefineEndPointsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Filament { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DefineEndPointsCompletionError<NextError> {
    DefineEndPoints(DefineEndPointsError),
    Next(NextError),
}

impl<NextError: fmt::Display> fmt::Display for DefineEndPointsCompletionError<NextError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DefineEndPoints(error) => error.fmt(formatter),
            Self::Next(error) => error.fmt(formatter),
        }
    }
}

impl<NextError: Error + 'static> Error for DefineEndPointsCompletionError<NextError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::DefineEndPoints(error) => Some(error),
            Self::Next(error) => Some(error),
        }
    }
}

/// Additive completion wrapper that owns only `DefineEndPoints` and delegates
/// every later stage unchanged.
pub struct DefineEndPointsCompletion<Next> {
    parameters: DefineEndPointsParameters,
    next: Next,
}

impl<Next> DefineEndPointsCompletion<Next> {
    #[must_use]
    pub const fn new(parameters: DefineEndPointsParameters, next: Next) -> Self {
        Self { parameters, next }
    }

    #[must_use]
    pub const fn next(&self) -> &Next {
        &self.next
    }
}

impl<Next> RemainingLineCompletionStages for DefineEndPointsCompletion<Next>
where
    Next: RemainingLineCompletionStages,
{
    type StepError = Next::StepError;
    type OtherError = DefineEndPointsCompletionError<Next::OtherError>;

    fn run_stage(
        &mut self,
        stage: LineCompletionStage,
        state: &mut PreparedCompletionState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        if stage == LineCompletionStage::DefineEndPoints {
            define_end_points(state, self.parameters).map_err(|error| {
                GridStageFailure::Other(DefineEndPointsCompletionError::DefineEndPoints(error))
            })
        } else {
            self.next
                .run_stage(stage, state)
                .map_err(|failure| match failure {
                    GridStageFailure::Step(error) => GridStageFailure::Step(error),
                    GridStageFailure::Other(error) => {
                        GridStageFailure::Other(DefineEndPointsCompletionError::Next(error))
                    }
                })
        }
    }

    fn log_swallowed_error(&mut self, error: &Self::OtherError) {
        if let DefineEndPointsCompletionError::Next(error) = error {
            self.next.log_swallowed_error(error);
        }
    }

    fn finish(&mut self) {
        self.next.finish();
    }
}

pub fn define_end_points(
    state: &mut PreparedCompletionState,
    parameters: DefineEndPointsParameters,
) -> Result<(), DefineEndPointsError> {
    if parameters.scale_interline <= 0
        || parameters.foreground_thickness < 0
        || parameters.maximum_ending_dx < 0
        || parameters.pattern_width < 0
        || parameters.pattern_jitter < 0
    {
        return Err(DefineEndPointsError::InvalidParameters(parameters));
    }
    let binary = state
        .binary_buffer
        .as_ref()
        .ok_or(DefineEndPointsError::MissingBinaryBuffer)?;
    let raster_width = binary.width();
    let raster_height = binary.height();
    let pixels = binary.to_pixels();
    state.defined_endpoints.clear();

    for staff in &mut state.staffs {
        let resolved =
            resolve_staff_endpoints(staff, parameters, raster_width, raster_height, &pixels)?;
        for (line, endpoints) in staff.lines.iter_mut().zip(&resolved.lines) {
            line.filament
                .set_ending_points(
                    (endpoints.left.x, endpoints.left.y),
                    (endpoints.right.x, endpoints.right.y),
                )
                .map_err(|source| DefineEndPointsError::Filament {
                    staff_id: staff.id,
                    line_id: line.id,
                    source,
                })?;
        }
        state.defined_endpoints.push(resolved);
    }
    Ok(())
}

fn resolve_staff_endpoints(
    staff: &PreparedStaff,
    parameters: DefineEndPointsParameters,
    raster_width: usize,
    raster_height: usize,
    pixels: &[u8],
) -> Result<PreparedStaffEndPoints, DefineEndPointsError> {
    if staff.lines.is_empty() {
        return Err(DefineEndPointsError::EmptyStaff(staff.id));
    }
    let geometries = staff
        .lines
        .iter()
        .map(|line| {
            line.filament
                .geometry()
                .map_err(|source| DefineEndPointsError::Filament {
                    staff_id: staff.id,
                    line_id: line.id,
                    source,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mean_interline = staff_mean_interline(staff, &geometries, parameters.scale_interline)?;
    let left_ending_slope = ending_slope(staff, &geometries, HorizontalSide::Left)?;
    let right_ending_slope = ending_slope(staff, &geometries, HorizontalSide::Right)?;
    let left = retrieve_side(
        staff,
        &geometries,
        parameters,
        mean_interline,
        left_ending_slope,
        HorizontalSide::Left,
        raster_width,
        raster_height,
        pixels,
    )?;
    let right = retrieve_side(
        staff,
        &geometries,
        parameters,
        mean_interline,
        right_ending_slope,
        HorizontalSide::Right,
        raster_width,
        raster_height,
        pixels,
    )?;
    let lines = staff
        .lines
        .iter()
        .zip(left.endings.iter().copied())
        .zip(right.endings.iter().copied())
        .map(|((line, left), right)| PreparedLineEndPoints {
            line_id: line.id,
            left,
            right,
        })
        .collect();

    Ok(PreparedStaffEndPoints {
        staff_id: staff.id,
        mean_interline,
        left_ending_slope,
        right_ending_slope,
        left_pattern_fit: left.pattern_fit,
        right_pattern_fit: right.pattern_fit,
        lines,
    })
}

fn staff_mean_interline(
    staff: &PreparedStaff,
    geometries: &[FilamentGeometry],
    sample_step: i32,
) -> Result<f64, DefineEndPointsError> {
    if geometries.len() == 1 {
        return Ok(0.0);
    }
    let left = staff.left.round_ties_even() as i32;
    let right = staff.right.round_ties_even() as i32;
    let mut sum = 0.0;
    let mut count = 0_usize;
    let mut x = left;
    while x <= right {
        let mut previous = geometries[0].position_at(f64::from(x)).map_err(|source| {
            DefineEndPointsError::Filament {
                staff_id: staff.id,
                line_id: staff.lines[0].id,
                source,
            }
        })?;
        for (line, geometry) in staff.lines[1..].iter().zip(&geometries[1..]) {
            let y = geometry.position_at(f64::from(x)).map_err(|source| {
                DefineEndPointsError::Filament {
                    staff_id: staff.id,
                    line_id: line.id,
                    source,
                }
            })?;
            sum += y - previous;
            count += 1;
            previous = y;
        }
        x = x.saturating_add(sample_step);
        if x == i32::MAX && right == i32::MAX {
            break;
        }
    }
    if count == 0 {
        return Err(DefineEndPointsError::EmptyMeanInterlinePopulation(staff.id));
    }
    Ok(sum / count as f64)
}

fn ending_slope(
    staff: &PreparedStaff,
    geometries: &[FilamentGeometry],
    side: HorizontalSide,
) -> Result<f64, DefineEndPointsError> {
    let mut slopes = staff
        .lines
        .iter()
        .zip(geometries)
        .map(|(line, geometry)| {
            let x = match side {
                HorizontalSide::Left => geometry.start().0,
                HorizontalSide::Right => geometry.stop().0,
            };
            geometry
                .slope_at(x)
                .map_err(|source| DefineEndPointsError::Filament {
                    staff_id: staff.id,
                    line_id: line.id,
                    source,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if slopes.len() < 3 {
        return Ok(slopes[0]);
    }
    slopes.sort_by(f64::total_cmp);
    Ok(slopes[1..slopes.len() - 1].iter().sum::<f64>() / (slopes.len() - 2) as f64)
}

#[allow(clippy::too_many_arguments)]
fn retrieve_side(
    staff: &PreparedStaff,
    geometries: &[FilamentGeometry],
    parameters: DefineEndPointsParameters,
    mean_interline: f64,
    ending_slope: f64,
    side: HorizontalSide,
    raster_width: usize,
    raster_height: usize,
    pixels: &[u8],
) -> Result<EndpointRetrieval, DefineEndPointsError> {
    let staff_x = match side {
        HorizontalSide::Left => staff.left,
        HorizontalSide::Right => staff.right,
    }
    .round_ties_even() as i32;
    let line_endings = geometries
        .iter()
        .map(|geometry| {
            let point = match side {
                HorizontalSide::Left => geometry.start(),
                HorizontalSide::Right => geometry.stop(),
            };
            LineEnding {
                x: point.0,
                y: point.1,
            }
        })
        .collect::<Vec<_>>();
    let pattern = StaffPattern::new(
        geometries.len(),
        parameters.pattern_width as usize,
        parameters.foreground_thickness as usize,
        f64::from(parameters.scale_interline),
    );
    retrieve_line_endpoints(
        EndpointRetrievalParams {
            side,
            staff_x,
            mean_interline,
            ending_slope,
            max_ending_dx: f64::from(parameters.maximum_ending_dx),
            pattern_width: parameters.pattern_width,
            pattern_jitter: parameters.pattern_jitter,
        },
        &line_endings,
        |x, y| pattern.evaluate((f64::from(x), y), raster_width, raster_height, pixels),
    )
    .map_err(|source| DefineEndPointsError::Retrieval {
        staff_id: staff.id,
        side,
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        filament::StaffFilament,
        lines_coordinator::StaffCandidateKind,
        prepared_lines::{PreparedStaffLine, RawDiscardedFilament},
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections},
    };

    #[derive(Default)]
    struct Tail {
        calls: Vec<LineCompletionStage>,
        finish_count: usize,
    }

    impl RemainingLineCompletionStages for Tail {
        type StepError = &'static str;
        type OtherError = &'static str;

        fn run_stage(
            &mut self,
            stage: LineCompletionStage,
            _state: &mut PreparedCompletionState,
        ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
            self.calls.push(stage);
            Ok(())
        }

        fn log_swallowed_error(&mut self, _error: &Self::OtherError) {}

        fn finish(&mut self) {
            self.finish_count += 1;
        }
    }

    fn prepared_staff(id: usize) -> PreparedStaff {
        let mut table = RunTable::new(Orientation::Horizontal, 120, 70).unwrap();
        for (index, y) in [10, 20, 30, 40, 50].into_iter().enumerate() {
            let stop = if index == 0 { 90 } else { 100 };
            table.add_run(y, Run::new(10, stop - 10 + 1)).unwrap();
        }
        let lines = build_sections(&table, JunctionPolicy::All)
            .into_iter()
            .enumerate()
            .map(|(index, section)| {
                let mut filament = StaffFilament::new(10).unwrap();
                filament.add_section(section).unwrap();
                PreparedStaffLine {
                    id: (id * 10) + index,
                    cluster_position: index as i32,
                    filament,
                }
            })
            .collect::<Vec<_>>();
        PreparedStaff {
            id,
            kind: StaffCandidateKind::Standard,
            left: 10.0,
            right: 100.0,
            interline: 10,
            small: false,
            short: false,
            lines,
        }
    }

    fn binary_pattern() -> RunTable {
        let mut table = RunTable::new(Orientation::Horizontal, 120, 70).unwrap();
        for y in [11, 21, 31, 41, 51] {
            table.add_run(y, Run::new(96, 4)).unwrap();
        }
        table
    }

    fn state(staffs: Vec<PreparedStaff>) -> PreparedCompletionState {
        PreparedCompletionState {
            staffs,
            global_slope: None,
            completion_systems: None,
            discarded_filaments: Vec::<RawDiscardedFilament>::new(),
            horizontal_sections: Vec::new(),
            binary_buffer: Some(binary_pattern()),
            thick_section_ids: Vec::new(),
            thin_section_ids: Vec::new(),
            defined_endpoints: Vec::new(),
            fill_hole_invocations: Vec::new(),
            discarded_filament_steals: Vec::new(),
            discarded_filament_recomputations: Vec::new(),
            section_inclusion_batches: Vec::new(),
            sticker_section_ids: Vec::new(),
            sticker_inclusion_batches: Vec::new(),
            crossing_line_inspections: Vec::new(),
            curvature_removals: Vec::new(),
            curvature_recomputations: Vec::new(),
            completed_stages: Vec::new(),
        }
    }

    fn parameters() -> DefineEndPointsParameters {
        DefineEndPointsParameters {
            scale_interline: 10,
            foreground_thickness: 1,
            maximum_ending_dx: 1,
            pattern_width: 4,
            pattern_jitter: 2,
        }
    }

    #[test]
    fn concrete_stage_uses_prepared_geometry_and_live_pattern_then_delegates() {
        let mut state = state(vec![prepared_staff(1)]);
        let mut completion = DefineEndPointsCompletion::new(parameters(), Tail::default());

        completion
            .run_stage(LineCompletionStage::DefineEndPoints, &mut state)
            .unwrap();

        assert!(completion.next().calls.is_empty());
        assert_eq!(state.defined_endpoints.len(), 1);
        let staff = &state.defined_endpoints[0];
        assert_eq!(staff.staff_id, 1);
        assert_eq!(staff.mean_interline, 10.0);
        assert_eq!(
            (staff.left_ending_slope, staff.right_ending_slope),
            (0.0, 0.0)
        );
        assert_eq!(staff.left_pattern_fit, None);
        assert_eq!(staff.right_pattern_fit, Some((96, 10.0, 1, 1.0)));
        assert_eq!(staff.lines[0].left, LineEnding { x: 10.0, y: 10.0 });
        assert_eq!(staff.lines[0].right, LineEnding { x: 100.0, y: 11.0 });
        assert_eq!(staff.lines[1].right, LineEnding { x: 100.0, y: 20.0 });
        let mutated = state.staffs[0].lines[0].filament.geometry().unwrap();
        assert_eq!(mutated.start(), (10.0, 10.0));
        assert_eq!(mutated.stop(), (100.0, 11.0));
        assert_eq!(
            state.staffs[0].lines[0].filament.ending_points(),
            Some(((10.0, 10.0), (100.0, 11.0)))
        );

        completion
            .run_stage(LineCompletionStage::IncludeDiscardedFilaments, &mut state)
            .unwrap();
        assert_eq!(
            completion.next().calls,
            [LineCompletionStage::IncludeDiscardedFilaments]
        );
    }

    #[test]
    fn later_staff_geometry_failure_retains_only_completed_staff_prefix() {
        let valid = prepared_staff(1);
        let invalid = PreparedStaff {
            id: 2,
            kind: StaffCandidateKind::OneLine,
            left: 10.0,
            right: 100.0,
            interline: 10,
            small: false,
            short: false,
            lines: vec![PreparedStaffLine {
                id: 99,
                cluster_position: 0,
                filament: StaffFilament::new(10).unwrap(),
            }],
        };
        let mut state = state(vec![valid, invalid]);

        let error = define_end_points(&mut state, parameters()).unwrap_err();

        assert!(matches!(
            error,
            DefineEndPointsError::Filament {
                staff_id: 2,
                line_id: 99,
                source: FilamentError::Empty,
            }
        ));
        assert_eq!(state.defined_endpoints.len(), 1);
        assert_eq!(state.defined_endpoints[0].staff_id, 1);
    }

    #[test]
    fn invalid_parameters_and_missing_buffer_fail_before_clearing_old_prefix() {
        let mut state = state(vec![prepared_staff(1)]);
        define_end_points(&mut state, parameters()).unwrap();
        let frozen = state.defined_endpoints.clone();

        let mut invalid = parameters();
        invalid.scale_interline = 0;
        assert_eq!(
            define_end_points(&mut state, invalid),
            Err(DefineEndPointsError::InvalidParameters(invalid))
        );
        assert_eq!(state.defined_endpoints, frozen);

        state.binary_buffer = None;
        assert_eq!(
            define_end_points(&mut state, parameters()),
            Err(DefineEndPointsError::MissingBinaryBuffer)
        );
        assert_eq!(state.defined_endpoints, frozen);
    }
}
