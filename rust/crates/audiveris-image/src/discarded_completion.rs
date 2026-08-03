// SPDX-License-Identifier: AGPL-3.0-or-later

//! Stateful Java `LinesRetriever.includeDiscardedFilaments` collaborator.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{
    filament::{
        FilamentError, FilamentInclusionDecision, FilamentInclusionEvidence,
        FilamentInclusionThresholds, FilamentPartOf, StaffFilament, can_include_filament,
    },
    grid_lifecycle::GridStageFailure,
    line_completion::LineCompletionStage,
    prepared_completion::{PreparedCompletionState, RemainingLineCompletionStages},
    section::Section,
};

/// Explicit Java `SystemInfo.getStaves()` membership and traversal order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedCompletionSystem {
    pub system_id: usize,
    pub staff_ids: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IncludeDiscardedFilamentsParameters {
    pub scale_interline: i32,
    pub foreground_thickness: i32,
    pub maximum_sticker_thickness: f64,
    pub maximum_sticker_gap: f64,
    pub maximum_sticker_extension: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedDiscardedFilamentSteal {
    pub candidate_line_id: usize,
    pub system_id: usize,
    pub staff_id: usize,
    pub line_id: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedFilamentRecomputation {
    pub system_id: usize,
    pub staff_id: usize,
    pub line_id: usize,
    pub preserved_start: (f64, f64),
    pub preserved_stop: (f64, f64),
}

#[derive(Clone, Debug, PartialEq)]
pub enum IncludeDiscardedFilamentsError {
    MissingSystemOwnership,
    InvalidParameters(IncludeDiscardedFilamentsParameters),
    DuplicateStaffId(usize),
    DuplicateCandidateId(usize),
    UnknownSystemStaff {
        system_id: usize,
        staff_id: usize,
    },
    StaffInMultipleSystems(usize),
    UnassignedStaff(usize),
    Filament {
        line_id: usize,
        source: FilamentError,
    },
}

impl fmt::Display for IncludeDiscardedFilamentsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSystemOwnership => formatter
                .write_str("includeDiscardedFilaments requires explicit system/staff membership"),
            Self::InvalidParameters(parameters) => write!(
                formatter,
                "invalid includeDiscardedFilaments parameters: {parameters:?}"
            ),
            Self::DuplicateStaffId(id) => write!(formatter, "duplicate prepared staff {id}"),
            Self::DuplicateCandidateId(id) => {
                write!(formatter, "duplicate discarded filament {id}")
            }
            Self::UnknownSystemStaff {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "system {system_id} refers to unknown staff {staff_id}"
            ),
            Self::StaffInMultipleSystems(id) => {
                write!(
                    formatter,
                    "staff {id} occurs in multiple completion systems"
                )
            }
            Self::UnassignedStaff(id) => {
                write!(formatter, "staff {id} has no completion system")
            }
            Self::Filament { line_id, source } => {
                write!(formatter, "filament {line_id} completion failed: {source}")
            }
        }
    }
}

impl Error for IncludeDiscardedFilamentsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Filament { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum IncludeDiscardedCompletionError<NextError> {
    IncludeDiscarded(IncludeDiscardedFilamentsError),
    Next(NextError),
}

impl<NextError: fmt::Display> fmt::Display for IncludeDiscardedCompletionError<NextError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncludeDiscarded(error) => error.fmt(formatter),
            Self::Next(error) => error.fmt(formatter),
        }
    }
}

impl<NextError: Error + 'static> Error for IncludeDiscardedCompletionError<NextError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::IncludeDiscarded(error) => Some(error),
            Self::Next(error) => Some(error),
        }
    }
}

pub struct IncludeDiscardedFilamentsCompletion<Next> {
    parameters: IncludeDiscardedFilamentsParameters,
    next: Next,
}

impl<Next> IncludeDiscardedFilamentsCompletion<Next> {
    #[must_use]
    pub const fn new(parameters: IncludeDiscardedFilamentsParameters, next: Next) -> Self {
        Self { parameters, next }
    }

    #[must_use]
    pub const fn next(&self) -> &Next {
        &self.next
    }
}

impl<Next> RemainingLineCompletionStages for IncludeDiscardedFilamentsCompletion<Next>
where
    Next: RemainingLineCompletionStages,
{
    type StepError = Next::StepError;
    type OtherError = IncludeDiscardedCompletionError<Next::OtherError>;

    fn run_stage(
        &mut self,
        stage: LineCompletionStage,
        state: &mut PreparedCompletionState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        if stage == LineCompletionStage::IncludeDiscardedFilaments {
            include_discarded_filaments(state, self.parameters).map_err(|error| {
                GridStageFailure::Other(IncludeDiscardedCompletionError::IncludeDiscarded(error))
            })
        } else {
            self.next
                .run_stage(stage, state)
                .map_err(|failure| match failure {
                    GridStageFailure::Step(error) => GridStageFailure::Step(error),
                    GridStageFailure::Other(error) => {
                        GridStageFailure::Other(IncludeDiscardedCompletionError::Next(error))
                    }
                })
        }
    }

    fn log_swallowed_error(&mut self, error: &Self::OtherError) {
        if let IncludeDiscardedCompletionError::Next(error) = error {
            self.next.log_swallowed_error(error);
        }
    }

    fn finish(&mut self) {
        self.next.finish();
    }
}

pub fn include_discarded_filaments(
    state: &mut PreparedCompletionState,
    parameters: IncludeDiscardedFilamentsParameters,
) -> Result<(), IncludeDiscardedFilamentsError> {
    validate_parameters(parameters)?;
    let systems = state
        .completion_systems
        .clone()
        .ok_or(IncludeDiscardedFilamentsError::MissingSystemOwnership)?;
    let staff_indices = validate_systems(&state.staffs, &systems)?;

    let mut candidate_ids = BTreeSet::new();
    let mut ordered_candidates = Vec::with_capacity(state.discarded_filaments.len());
    for (index, discarded) in state.discarded_filaments.iter().enumerate() {
        if !candidate_ids.insert(discarded.line.id) {
            return Err(IncludeDiscardedFilamentsError::DuplicateCandidateId(
                discarded.line.id,
            ));
        }
        let top = discarded
            .line
            .filament
            .bounds()
            .map_err(|source| IncludeDiscardedFilamentsError::Filament {
                line_id: discarded.line.id,
                source,
            })?
            .y;
        ordered_candidates.push((index, top));
    }
    // Java Collections.sort is stable and Filament.topComparator compares
    // only the top ordinate.
    ordered_candidates.sort_by_key(|(_, top)| *top);

    state.discarded_filament_steals.clear();
    state.discarded_filament_recomputations.clear();
    let (staffs, candidates, steals, recomputations) = (
        &mut state.staffs,
        &mut state.discarded_filaments,
        &mut state.discarded_filament_steals,
        &mut state.discarded_filament_recomputations,
    );

    for system in &systems {
        let mut i_min = 0_usize;
        for &staff_id in &system.staff_ids {
            let staff_index = staff_indices[&staff_id];
            for line in &mut staffs[staff_index].lines {
                let geometry = line.filament.geometry().map_err(|source| {
                    IncludeDiscardedFilamentsError::Filament {
                        line_id: line.id,
                        source,
                    }
                })?;
                let preserved_start = geometry.start();
                let preserved_stop = geometry.stop();
                let bounds = line.filament.bounds().map_err(|source| {
                    IncludeDiscardedFilamentsError::Filament {
                        line_id: line.id,
                        source,
                    }
                })?;
                let minimum_y = bounds.y as i64 - i64::from(parameters.foreground_thickness);
                let maximum_y = bounds.y as i64
                    + bounds.height as i64
                    + i64::from(parameters.foreground_thickness);
                let mut modified = false;

                for (ordered_index, &(candidate_index, top)) in
                    ordered_candidates.iter().enumerate().skip(i_min)
                {
                    let candidate = &mut candidates[candidate_index];
                    if candidate.line.filament.part_of().is_some() {
                        continue;
                    }
                    let top = top as i64;
                    if top < minimum_y {
                        i_min = ordered_index;
                        continue;
                    }
                    if top > maximum_y {
                        break;
                    }
                    let center_x =
                        rounded_centroid_x(&candidate.line.filament).map_err(|source| {
                            IncludeDiscardedFilamentsError::Filament {
                                line_id: candidate.line.id,
                                source,
                            }
                        })?;
                    if center_x < preserved_start.0 || center_x > preserved_stop.0 {
                        continue;
                    }
                    let evidence = inclusion_evidence(
                        line.id,
                        &line.filament,
                        candidate.line.id,
                        &candidate.line.filament,
                        parameters,
                    )?;
                    if can_include_filament(
                        evidence,
                        FilamentInclusionThresholds {
                            max_sticker_thickness: parameters.maximum_sticker_thickness,
                            max_sticker_gap: parameters.maximum_sticker_gap,
                            max_sticker_extension: parameters.maximum_sticker_extension,
                        },
                    ) == FilamentInclusionDecision::Include
                    {
                        line.filament
                            .steal_sections_from(
                                &mut candidate.line.filament,
                                FilamentPartOf {
                                    staff_id,
                                    line_id: line.id,
                                },
                            )
                            .map_err(|source| IncludeDiscardedFilamentsError::Filament {
                                line_id: line.id,
                                source,
                            })?;
                        steals.push(PreparedDiscardedFilamentSteal {
                            candidate_line_id: candidate.line.id,
                            system_id: system.system_id,
                            staff_id,
                            line_id: line.id,
                        });
                        modified = true;
                    }
                }

                if modified {
                    line.filament
                        .set_ending_points(preserved_start, preserved_stop)
                        .map_err(|source| IncludeDiscardedFilamentsError::Filament {
                            line_id: line.id,
                            source,
                        })?;
                    recomputations.push(PreparedFilamentRecomputation {
                        system_id: system.system_id,
                        staff_id,
                        line_id: line.id,
                        preserved_start,
                        preserved_stop,
                    });
                }
            }
        }
    }
    Ok(())
}

fn validate_parameters(
    parameters: IncludeDiscardedFilamentsParameters,
) -> Result<(), IncludeDiscardedFilamentsError> {
    if parameters.scale_interline <= 0
        || parameters.foreground_thickness < 0
        || !parameters.maximum_sticker_thickness.is_finite()
        || parameters.maximum_sticker_thickness < 0.0
        || !parameters.maximum_sticker_gap.is_finite()
        || parameters.maximum_sticker_gap < 0.0
        || parameters.maximum_sticker_extension < 0
    {
        return Err(IncludeDiscardedFilamentsError::InvalidParameters(
            parameters,
        ));
    }
    Ok(())
}

fn validate_systems(
    staffs: &[crate::prepared_lines::PreparedStaff],
    systems: &[PreparedCompletionSystem],
) -> Result<BTreeMap<usize, usize>, IncludeDiscardedFilamentsError> {
    let mut staff_indices = BTreeMap::new();
    for (index, staff) in staffs.iter().enumerate() {
        if staff_indices.insert(staff.id, index).is_some() {
            return Err(IncludeDiscardedFilamentsError::DuplicateStaffId(staff.id));
        }
    }
    let mut assigned = BTreeSet::new();
    for system in systems {
        for &staff_id in &system.staff_ids {
            if !staff_indices.contains_key(&staff_id) {
                return Err(IncludeDiscardedFilamentsError::UnknownSystemStaff {
                    system_id: system.system_id,
                    staff_id,
                });
            }
            if !assigned.insert(staff_id) {
                return Err(IncludeDiscardedFilamentsError::StaffInMultipleSystems(
                    staff_id,
                ));
            }
        }
    }
    if let Some(unassigned) = staff_indices.keys().find(|id| !assigned.contains(id)) {
        return Err(IncludeDiscardedFilamentsError::UnassignedStaff(*unassigned));
    }
    Ok(staff_indices)
}

fn rounded_centroid_x(filament: &StaffFilament) -> Result<f64, FilamentError> {
    let mut weight = 0_usize;
    let mut sum_x = 0.0;
    for section in filament.sections() {
        let section_weight = section.weight();
        let centroid = section
            .pixel_centroid_in(section.bounds())
            .ok_or(FilamentError::Empty)?;
        weight += section_weight;
        sum_x += centroid.0 * section_weight as f64;
    }
    if weight == 0 {
        return Err(FilamentError::Empty);
    }
    Ok((sum_x / weight as f64).round_ties_even())
}

fn inclusion_evidence(
    line_id: usize,
    line: &StaffFilament,
    candidate_id: usize,
    candidate: &StaffFilament,
    parameters: IncludeDiscardedFilamentsParameters,
) -> Result<FilamentInclusionEvidence, IncludeDiscardedFilamentsError> {
    let candidate_bounds =
        candidate
            .bounds()
            .map_err(|source| IncludeDiscardedFilamentsError::Filament {
                line_id: candidate_id,
                source,
            })?;
    let x_mid = candidate_bounds.x + (candidate_bounds.width / 2);
    let x_mid_f64 = x_mid as f64;
    let line_geometry = line
        .geometry()
        .map_err(|source| IncludeDiscardedFilamentsError::Filament { line_id, source })?;
    let candidate_geometry =
        candidate
            .geometry()
            .map_err(|source| IncludeDiscardedFilamentsError::Filament {
                line_id: candidate_id,
                source,
            })?;
    let line_y_at_mid = line_geometry
        .position_at(x_mid_f64)
        .map_err(|source| IncludeDiscardedFilamentsError::Filament { line_id, source })?;
    let candidate_y_at_mid = candidate_geometry
        .position_at(x_mid_f64)
        .map_err(|source| IncludeDiscardedFilamentsError::Filament {
            line_id: candidate_id,
            source,
        })?;
    let start = candidate_geometry.start();
    let stop = candidate_geometry.stop();
    let start_delta_y = start.1
        - line_geometry
            .position_at(start.0)
            .map_err(|source| IncludeDiscardedFilamentsError::Filament { line_id, source })?;
    let stop_delta_y = stop.1
        - line_geometry
            .position_at(stop.0)
            .map_err(|source| IncludeDiscardedFilamentsError::Filament { line_id, source })?;
    let probe_width = (f64::from(parameters.scale_interline) * 0.5).round_ties_even() as i32;
    let resulting_compound_thickness = compound_thickness_at(
        x_mid as i32,
        probe_width / 2,
        line.sections(),
        candidate.sections(),
    );

    Ok(FilamentInclusionEvidence {
        entity_mean_thickness: candidate.thickness().map_err(|source| {
            IncludeDiscardedFilamentsError::Filament {
                line_id: candidate_id,
                source,
            }
        })?,
        line_y_at_mid,
        candidate_y_at_mid,
        staff_foreground_thickness: f64::from(parameters.foreground_thickness),
        start_delta_y,
        stop_delta_y,
        resulting_compound_thickness,
    })
}

fn compound_thickness_at(
    x_mid: i32,
    probe_half_width: i32,
    line_sections: &[Section],
    candidate_sections: &[Section],
) -> f64 {
    let x_min = x_mid - probe_half_width;
    let x_max_exclusive = x_mid + probe_half_width;
    if x_min >= x_max_exclusive {
        return 0.0;
    }
    let mut minimum_y = None::<usize>;
    let mut maximum_y = None::<usize>;
    for section in line_sections.iter().chain(candidate_sections) {
        for (offset, run) in section.runs().iter().enumerate() {
            let overlaps = (run.stop() as i64) >= i64::from(x_min)
                && (run.start as i64) < i64::from(x_max_exclusive);
            if overlaps {
                let y = section.first_pos() + offset;
                minimum_y = Some(minimum_y.map_or(y, |current| current.min(y)));
                maximum_y = Some(maximum_y.map_or(y, |current| current.max(y)));
            }
        }
    }
    minimum_y
        .zip(maximum_y)
        .map_or(0.0, |(minimum, maximum)| (maximum - minimum + 1) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        line_endpoints::PreparedStaffEndPoints,
        lines_coordinator::StaffCandidateKind,
        prepared_lines::{
            PreparedStaff, PreparedStaffLine, RawDiscardedFilament, RawDiscardedFilamentProvenance,
        },
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections_from_id},
    };

    #[derive(Default)]
    struct Tail {
        calls: Vec<LineCompletionStage>,
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

        fn finish(&mut self) {}
    }

    fn line(id: usize, y: usize, start: usize, stop: usize) -> PreparedStaffLine {
        let mut table = RunTable::new(Orientation::Horizontal, 220, 40).unwrap();
        table.add_run(y, Run::new(start, stop - start + 1)).unwrap();
        let section = build_sections_from_id(&table, JunctionPolicy::All, id).remove(0);
        let mut filament = StaffFilament::new(10).unwrap();
        filament.add_section(section).unwrap();
        PreparedStaffLine {
            id,
            cluster_position: 0,
            filament,
        }
    }

    fn target_staff(id: usize, line_id: usize, start: usize, stop: usize) -> PreparedStaff {
        let mut line = line(line_id, 10, start, stop);
        line.filament
            .set_ending_points((start as f64, 10.0), (stop as f64, 10.0))
            .unwrap();
        PreparedStaff {
            id,
            kind: StaffCandidateKind::OneLine,
            left: start as f64,
            right: stop as f64,
            interline: 10,
            small: false,
            short: false,
            lines: vec![line],
        }
    }

    fn candidate(id: usize, start: usize, stop: usize) -> RawDiscardedFilament {
        RawDiscardedFilament {
            provenance: RawDiscardedFilamentProvenance::PrimaryDiscarded,
            line: line(id, 10, start, stop),
        }
    }

    fn parameters() -> IncludeDiscardedFilamentsParameters {
        IncludeDiscardedFilamentsParameters {
            scale_interline: 10,
            foreground_thickness: 1,
            maximum_sticker_thickness: 2.0,
            maximum_sticker_gap: 0.5,
            maximum_sticker_extension: 2,
        }
    }

    fn state() -> PreparedCompletionState {
        PreparedCompletionState {
            staffs: vec![target_staff(1, 11, 10, 100), target_staff(2, 12, 110, 200)],
            completion_systems: Some(vec![
                PreparedCompletionSystem {
                    system_id: 7,
                    staff_ids: vec![1],
                },
                PreparedCompletionSystem {
                    system_id: 8,
                    staff_ids: vec![2],
                },
            ]),
            discarded_filaments: vec![candidate(21, 20, 30), candidate(22, 120, 130)],
            horizontal_sections: Vec::new(),
            binary_buffer: None,
            thick_section_ids: Vec::new(),
            thin_section_ids: Vec::new(),
            defined_endpoints: Vec::<PreparedStaffEndPoints>::new(),
            fill_hole_insertions: Vec::new(),
            discarded_filament_steals: Vec::new(),
            discarded_filament_recomputations: Vec::new(),
            completed_stages: Vec::new(),
        }
    }

    #[test]
    fn concrete_stage_restarts_each_system_steals_and_restores_endpoints() {
        let mut state = state();
        let mut completion =
            IncludeDiscardedFilamentsCompletion::new(parameters(), Tail::default());

        completion
            .run_stage(LineCompletionStage::IncludeDiscardedFilaments, &mut state)
            .unwrap();

        assert!(completion.next().calls.is_empty());
        assert_eq!(
            state.discarded_filament_steals,
            [
                PreparedDiscardedFilamentSteal {
                    candidate_line_id: 21,
                    system_id: 7,
                    staff_id: 1,
                    line_id: 11,
                },
                PreparedDiscardedFilamentSteal {
                    candidate_line_id: 22,
                    system_id: 8,
                    staff_id: 2,
                    line_id: 12,
                },
            ]
        );
        assert_eq!(state.discarded_filament_recomputations.len(), 2);
        assert_eq!(state.staffs[0].lines[0].filament.sections().len(), 2);
        assert_eq!(
            state.staffs[0].lines[0].filament.ending_points(),
            Some(((10.0, 10.0), (100.0, 10.0)))
        );
        assert_eq!(
            state.discarded_filaments[0].line.filament.part_of(),
            Some(FilamentPartOf {
                staff_id: 1,
                line_id: 11,
            })
        );
        assert_eq!(
            state.discarded_filaments[1].line.filament.part_of(),
            Some(FilamentPartOf {
                staff_id: 2,
                line_id: 12,
            })
        );

        completion
            .run_stage(LineCompletionStage::FillHolesInitial, &mut state)
            .unwrap();
        assert_eq!(
            completion.next().calls,
            [LineCompletionStage::FillHolesInitial]
        );
    }

    #[test]
    fn absent_system_mapping_fails_before_mutation_instead_of_flattening_staffs() {
        let mut state = state();
        state.completion_systems = None;
        state
            .discarded_filament_steals
            .push(PreparedDiscardedFilamentSteal {
                candidate_line_id: 999,
                system_id: 9,
                staff_id: 9,
                line_id: 9,
            });

        assert_eq!(
            include_discarded_filaments(&mut state, parameters()),
            Err(IncludeDiscardedFilamentsError::MissingSystemOwnership)
        );
        assert_eq!(state.discarded_filament_steals[0].candidate_line_id, 999);
        assert!(
            state.discarded_filaments[0]
                .line
                .filament
                .part_of()
                .is_none()
        );
    }

    #[test]
    fn endpoint_recompute_failure_retains_steal_owner_and_mutated_line_prefix() {
        let mut state = state();
        let target = &mut state.staffs[0].lines[0].filament;
        assert_eq!(
            target.set_ending_points((-5.0, 10.0), (100.0, 10.0)),
            Err(FilamentError::InvalidEndingPoints)
        );

        let error = include_discarded_filaments(&mut state, parameters()).unwrap_err();

        assert_eq!(
            error,
            IncludeDiscardedFilamentsError::Filament {
                line_id: 11,
                source: FilamentError::InvalidEndingPoints,
            }
        );
        assert_eq!(state.discarded_filament_steals.len(), 1);
        assert!(state.discarded_filament_recomputations.is_empty());
        assert_eq!(state.staffs[0].lines[0].filament.sections().len(), 2);
        assert_eq!(
            state.discarded_filaments[0].line.filament.part_of(),
            Some(FilamentPartOf {
                staff_id: 1,
                line_id: 11,
            })
        );
        assert_eq!(
            state.staffs[0].lines[0].filament.ending_points(),
            Some(((-5.0, 10.0), (100.0, 10.0)))
        );
    }
}
