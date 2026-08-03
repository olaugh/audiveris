// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shared concrete implementation of Java `LinesRetriever.includeSections`.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{
    discarded_completion::PreparedCompletionSystem,
    filament::{FilamentError, StaffFilament},
    grid_lifecycle::GridStageFailure,
    line_completion::LineCompletionStage,
    line_section_dispatch::{
        SectionInclusionDecision, SectionInclusionEvidence, SectionInclusionThresholds,
        can_include_section,
    },
    prepared_completion::{PreparedCompletionState, RemainingLineCompletionStages},
    prepared_lines::PreparedStaff,
    section::{Bounds, Section},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IncludeSectionsParameters {
    pub scale_interline: i32,
    pub foreground_thickness: i32,
    pub maximum_sticker_thickness: f64,
    pub maximum_sticker_gap: f64,
    pub maximum_sticker_extension: i32,
}

/// Mutation ledger entry created at Java's per-line batch boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedSectionInclusionBatch {
    pub stage: LineCompletionStage,
    pub system_id: usize,
    pub staff_id: usize,
    pub line_id: usize,
    pub section_ids: Vec<usize>,
    pub preserved_start: (f64, f64),
    pub preserved_stop: (f64, f64),
    pub endpoints_recomputed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum IncludeSectionsError {
    MissingSystemOwnership,
    InvalidParameters(IncludeSectionsParameters),
    DuplicateStaffId(usize),
    DuplicateSectionId(usize),
    UnknownSectionId(usize),
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

impl fmt::Display for IncludeSectionsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSystemOwnership => {
                formatter.write_str("includeSections requires explicit system/staff membership")
            }
            Self::InvalidParameters(parameters) => {
                write!(
                    formatter,
                    "invalid includeSections parameters: {parameters:?}"
                )
            }
            Self::DuplicateStaffId(id) => write!(formatter, "duplicate prepared staff {id}"),
            Self::DuplicateSectionId(id) => write!(formatter, "duplicate horizontal section {id}"),
            Self::UnknownSectionId(id) => write!(formatter, "unknown horizontal section {id}"),
            Self::UnknownSystemStaff {
                system_id,
                staff_id,
            } => {
                write!(
                    formatter,
                    "system {system_id} refers to unknown staff {staff_id}"
                )
            }
            Self::StaffInMultipleSystems(id) => {
                write!(
                    formatter,
                    "staff {id} occurs in multiple completion systems"
                )
            }
            Self::UnassignedStaff(id) => write!(formatter, "staff {id} has no completion system"),
            Self::Filament { line_id, source } => {
                write!(
                    formatter,
                    "line {line_id} section inclusion failed: {source}"
                )
            }
        }
    }
}

impl Error for IncludeSectionsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Filament { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum IncludeSectionsCompletionError<NextError> {
    IncludeSections(IncludeSectionsError),
    Next(NextError),
}

impl<NextError: fmt::Display> fmt::Display for IncludeSectionsCompletionError<NextError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncludeSections(error) => error.fmt(formatter),
            Self::Next(error) => error.fmt(formatter),
        }
    }
}

impl<NextError: Error + 'static> Error for IncludeSectionsCompletionError<NextError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::IncludeSections(error) => Some(error),
            Self::Next(error) => Some(error),
        }
    }
}

/// One wrapper owns both calls to Java's same `includeSections` method.
pub struct IncludeSectionsCompletion<Next> {
    parameters: IncludeSectionsParameters,
    next: Next,
}

impl<Next> IncludeSectionsCompletion<Next> {
    #[must_use]
    pub const fn new(parameters: IncludeSectionsParameters, next: Next) -> Self {
        Self { parameters, next }
    }

    #[must_use]
    pub const fn next(&self) -> &Next {
        &self.next
    }
}

impl<Next> RemainingLineCompletionStages for IncludeSectionsCompletion<Next>
where
    Next: RemainingLineCompletionStages,
{
    type StepError = Next::StepError;
    type OtherError = IncludeSectionsCompletionError<Next::OtherError>;

    fn run_stage(
        &mut self,
        stage: LineCompletionStage,
        state: &mut PreparedCompletionState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        if matches!(
            stage,
            LineCompletionStage::IncludeThickSections | LineCompletionStage::IncludeThinSections
        ) {
            include_sections(state, stage, self.parameters).map_err(|error| {
                GridStageFailure::Other(IncludeSectionsCompletionError::IncludeSections(error))
            })
        } else {
            self.next
                .run_stage(stage, state)
                .map_err(|failure| match failure {
                    GridStageFailure::Step(error) => GridStageFailure::Step(error),
                    GridStageFailure::Other(error) => {
                        GridStageFailure::Other(IncludeSectionsCompletionError::Next(error))
                    }
                })
        }
    }

    fn log_swallowed_error(&mut self, error: &Self::OtherError) {
        if let IncludeSectionsCompletionError::Next(error) = error {
            self.next.log_swallowed_error(error);
        }
    }

    fn finish(&mut self) {
        self.next.finish();
    }
}

pub fn include_sections(
    state: &mut PreparedCompletionState,
    stage: LineCompletionStage,
    parameters: IncludeSectionsParameters,
) -> Result<(), IncludeSectionsError> {
    validate_parameters(stage, parameters)?;
    let systems = state
        .completion_systems
        .clone()
        .ok_or(IncludeSectionsError::MissingSystemOwnership)?;
    let staff_indices = validate_systems(&state.staffs, &systems)?;
    let candidates = selected_sections(state, stage)?;
    let maximum_id = state
        .horizontal_sections
        .iter()
        .map(Section::id)
        .max()
        .unwrap_or(0);
    // Java uses an id-indexed boolean vector, freshly allocated per call.
    let mut included = vec![false; maximum_id + 1];
    state
        .section_inclusion_batches
        .retain(|batch| batch.stage != stage);

    let (staffs, batches) = (&mut state.staffs, &mut state.section_inclusion_batches);
    for system in &systems {
        // Side-by-side systems restart the stable top-position scan.
        let mut i_min = 0_usize;
        for &staff_id in &system.staff_ids {
            let staff_index = staff_indices[&staff_id];
            for line in &mut staffs[staff_index].lines {
                let geometry =
                    line.filament
                        .geometry()
                        .map_err(|source| IncludeSectionsError::Filament {
                            line_id: line.id,
                            source,
                        })?;
                let preserved_start = geometry.start();
                let preserved_stop = geometry.stop();
                let bounds =
                    line.filament
                        .bounds()
                        .map_err(|source| IncludeSectionsError::Filament {
                            line_id: line.id,
                            source,
                        })?;
                let minimum_y = bounds.y as i64 - i64::from(parameters.foreground_thickness);
                let maximum_y =
                    (bounds.y + bounds.height) as i64 + i64::from(parameters.foreground_thickness);
                let batch_index = batches.len();
                batches.push(PreparedSectionInclusionBatch {
                    stage,
                    system_id: system.system_id,
                    staff_id,
                    line_id: line.id,
                    section_ids: Vec::new(),
                    preserved_start,
                    preserved_stop,
                    endpoints_recomputed: false,
                });
                let mut stickers = Vec::new();

                for (index, section) in candidates.iter().enumerate().skip(i_min) {
                    if included[section.id()] {
                        continue;
                    }
                    let first_position = section.first_pos() as i64;
                    if first_position < minimum_y {
                        // Java deliberately stores i rather than i + 1.
                        i_min = index;
                        continue;
                    }
                    if first_position > maximum_y {
                        break;
                    }
                    let center_x = section.centroid().0 as f64;
                    if center_x >= preserved_start.0
                        && center_x <= preserved_stop.0
                        && section_is_includable(&line.filament, section, parameters).map_err(
                            |source| IncludeSectionsError::Filament {
                                line_id: line.id,
                                source,
                            },
                        )?
                    {
                        stickers.push(section.clone());
                    }
                }

                // Selection is deferred, then each successful addition marks
                // its ID immediately. A later failure retains this prefix.
                for section in stickers {
                    let section_id = section.id();
                    line.filament.add_section(section).map_err(|source| {
                        IncludeSectionsError::Filament {
                            line_id: line.id,
                            source,
                        }
                    })?;
                    included[section_id] = true;
                    batches[batch_index].section_ids.push(section_id);
                }

                // Java performs this once per line even for an empty batch.
                let unbounded =
                    line.filament
                        .geometry()
                        .map_err(|source| IncludeSectionsError::Filament {
                            line_id: line.id,
                            source,
                        })?;
                let start_y = unbounded.position_at(preserved_start.0).map_err(|source| {
                    IncludeSectionsError::Filament {
                        line_id: line.id,
                        source,
                    }
                })?;
                let stop_y = unbounded.position_at(preserved_stop.0).map_err(|source| {
                    IncludeSectionsError::Filament {
                        line_id: line.id,
                        source,
                    }
                })?;
                line.filament
                    .set_ending_points((preserved_start.0, start_y), (preserved_stop.0, stop_y))
                    .map_err(|source| IncludeSectionsError::Filament {
                        line_id: line.id,
                        source,
                    })?;
                batches[batch_index].endpoints_recomputed = true;
            }
        }
    }
    Ok(())
}

fn validate_parameters(
    stage: LineCompletionStage,
    parameters: IncludeSectionsParameters,
) -> Result<(), IncludeSectionsError> {
    if !matches!(
        stage,
        LineCompletionStage::IncludeThickSections | LineCompletionStage::IncludeThinSections
    ) || parameters.scale_interline <= 0
        || parameters.foreground_thickness < 0
        || !parameters.maximum_sticker_thickness.is_finite()
        || parameters.maximum_sticker_thickness < 0.0
        || !parameters.maximum_sticker_gap.is_finite()
        || parameters.maximum_sticker_gap < 0.0
        || parameters.maximum_sticker_extension < 0
    {
        return Err(IncludeSectionsError::InvalidParameters(parameters));
    }
    Ok(())
}

fn selected_sections(
    state: &PreparedCompletionState,
    stage: LineCompletionStage,
) -> Result<Vec<Section>, IncludeSectionsError> {
    let mut by_id = BTreeMap::new();
    for section in &state.horizontal_sections {
        if by_id.insert(section.id(), section).is_some() {
            return Err(IncludeSectionsError::DuplicateSectionId(section.id()));
        }
    }
    let ids = if stage == LineCompletionStage::IncludeThickSections {
        &state.thick_section_ids
    } else {
        &state.thin_section_ids
    };
    let mut seen = BTreeSet::new();
    let mut selected = Vec::with_capacity(ids.len());
    for &id in ids {
        if !seen.insert(id) {
            return Err(IncludeSectionsError::DuplicateSectionId(id));
        }
        selected.push(
            (*by_id
                .get(&id)
                .ok_or(IncludeSectionsError::UnknownSectionId(id))?)
            .clone(),
        );
    }
    // `Collections.sort` is stable; source order resolves equal firstPos.
    selected.sort_by_key(Section::first_pos);
    Ok(selected)
}

fn validate_systems(
    staffs: &[PreparedStaff],
    systems: &[PreparedCompletionSystem],
) -> Result<BTreeMap<usize, usize>, IncludeSectionsError> {
    let mut staff_indices = BTreeMap::new();
    for (index, staff) in staffs.iter().enumerate() {
        if staff_indices.insert(staff.id, index).is_some() {
            return Err(IncludeSectionsError::DuplicateStaffId(staff.id));
        }
    }
    let mut assigned = BTreeSet::new();
    for system in systems {
        for &staff_id in &system.staff_ids {
            if !staff_indices.contains_key(&staff_id) {
                return Err(IncludeSectionsError::UnknownSystemStaff {
                    system_id: system.system_id,
                    staff_id,
                });
            }
            if !assigned.insert(staff_id) {
                return Err(IncludeSectionsError::StaffInMultipleSystems(staff_id));
            }
        }
    }
    if let Some(id) = staff_indices.keys().find(|id| !assigned.contains(id)) {
        return Err(IncludeSectionsError::UnassignedStaff(*id));
    }
    Ok(staff_indices)
}

fn section_is_includable(
    filament: &StaffFilament,
    section: &Section,
    parameters: IncludeSectionsParameters,
) -> Result<bool, FilamentError> {
    let bounds = section.bounds();
    let x_mid = bounds.x + (bounds.width / 2);
    let line_y = filament.geometry()?.position_at(x_mid as f64)?;
    let resulting_thickness = compound_thickness_at(
        x_mid,
        (f64::from(parameters.scale_interline) * 0.5).round_ties_even() as usize,
        section,
        filament,
    )?;
    Ok(can_include_section(
        SectionInclusionEvidence {
            entity_thickness: bounds.height as f64,
            section_top: bounds.y as f64,
            centroid_y: section.centroid_2d().1,
            line_y_at_mid: line_y,
            staff_foreground_thickness: f64::from(parameters.foreground_thickness),
            resulting_compound_thickness: resulting_thickness,
        },
        SectionInclusionThresholds {
            max_sticker_thickness: parameters.maximum_sticker_thickness,
            max_sticker_gap: parameters.maximum_sticker_gap,
            max_sticker_extension: parameters.maximum_sticker_extension,
        },
    ) == SectionInclusionDecision::Include)
}

fn compound_thickness_at(
    coordinate: usize,
    probe_width: usize,
    candidate: &Section,
    filament: &StaffFilament,
) -> Result<f64, FilamentError> {
    let union = union_bounds(candidate.bounds(), filament.bounds()?);
    if coordinate < union.x || coordinate >= union.x + union.width {
        return Ok(0.0);
    }
    let half_width = probe_width / 2;
    if half_width == 0 {
        return Ok(0.0);
    }
    let start = coordinate.saturating_sub(half_width);
    let stop = coordinate + half_width;
    let mut minimum_y = None::<usize>;
    let mut maximum_y = None::<usize>;
    for section in filament.sections().iter().chain(std::iter::once(candidate)) {
        for (offset, run) in section.runs().iter().enumerate() {
            if run.start < stop && run.stop() + 1 > start {
                let y = section.first_pos() + offset;
                minimum_y = Some(minimum_y.map_or(y, |current| current.min(y)));
                maximum_y = Some(maximum_y.map_or(y, |current| current.max(y)));
            }
        }
    }
    Ok(minimum_y
        .zip(maximum_y)
        .map_or(0.0, |(minimum, maximum)| (maximum - minimum + 1) as f64))
}

fn union_bounds(one: Bounds, two: Bounds) -> Bounds {
    let x = one.x.min(two.x);
    let y = one.y.min(two.y);
    let stop_x = (one.x + one.width).max(two.x + two.width);
    let stop_y = (one.y + one.height).max(two.y + two.height);
    Bounds {
        x,
        y,
        width: stop_x - x,
        height: stop_y - y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        line_endpoints::PreparedStaffEndPoints,
        lines_coordinator::StaffCandidateKind,
        prepared_lines::{PreparedStaff, PreparedStaffLine},
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections_from_id},
    };

    fn section(id: usize, y: usize) -> Section {
        let mut table = RunTable::new(Orientation::Horizontal, 101, 31).unwrap();
        table.add_run(y, Run::new(20, 5)).unwrap();
        build_sections_from_id(&table, JunctionPolicy::All, id).remove(0)
    }

    fn line(id: usize, y: usize) -> PreparedStaffLine {
        let mut table = RunTable::new(Orientation::Horizontal, 101, 31).unwrap();
        table.add_run(y, Run::new(0, 101)).unwrap();
        let mut filament = StaffFilament::new(10).unwrap();
        filament
            .add_section(build_sections_from_id(&table, JunctionPolicy::All, 100 + id).remove(0))
            .unwrap();
        filament
            .set_ending_points((0.0, y as f64), (100.0, y as f64))
            .unwrap();
        PreparedStaffLine {
            id,
            cluster_position: 0,
            filament,
        }
    }

    fn state() -> PreparedCompletionState {
        PreparedCompletionState {
            staffs: vec![PreparedStaff {
                id: 5,
                kind: StaffCandidateKind::OneLine,
                left: 0.0,
                right: 100.0,
                interline: 10,
                small: false,
                short: false,
                lines: vec![line(7, 10)],
            }],
            global_slope: None,
            completion_systems: Some(vec![PreparedCompletionSystem {
                system_id: 3,
                staff_ids: vec![5],
            }]),
            discarded_filaments: Vec::new(),
            horizontal_sections: vec![section(1, 11), section(2, 11)],
            binary_buffer: None,
            thick_section_ids: vec![1],
            thin_section_ids: vec![2],
            defined_endpoints: Vec::<PreparedStaffEndPoints>::new(),
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

    fn parameters() -> IncludeSectionsParameters {
        IncludeSectionsParameters {
            scale_interline: 10,
            foreground_thickness: 1,
            maximum_sticker_thickness: 4.0,
            maximum_sticker_gap: 2.0,
            maximum_sticker_extension: 4,
        }
    }

    #[test]
    fn thick_and_thin_calls_share_logic_but_reset_included_vector() {
        let mut state = state();
        include_sections(
            &mut state,
            LineCompletionStage::IncludeThickSections,
            parameters(),
        )
        .unwrap();
        include_sections(
            &mut state,
            LineCompletionStage::IncludeThinSections,
            parameters(),
        )
        .unwrap();

        assert_eq!(state.staffs[0].lines[0].filament.sections().len(), 3);
        assert_eq!(state.section_inclusion_batches.len(), 2);
        assert_eq!(state.section_inclusion_batches[0].section_ids, [1]);
        assert_eq!(state.section_inclusion_batches[1].section_ids, [2]);
        assert!(
            state
                .section_inclusion_batches
                .iter()
                .all(|batch| batch.endpoints_recomputed)
        );
    }

    #[test]
    fn absent_system_mapping_fails_before_any_line_mutation() {
        let mut state = state();
        state.completion_systems = None;
        let before = state.staffs[0].lines[0].filament.sections().to_vec();
        assert_eq!(
            include_sections(
                &mut state,
                LineCompletionStage::IncludeThickSections,
                parameters(),
            ),
            Err(IncludeSectionsError::MissingSystemOwnership)
        );
        assert_eq!(state.staffs[0].lines[0].filament.sections(), before);
        assert!(state.section_inclusion_batches.is_empty());
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
    fn wrapper_owns_exactly_both_include_section_stages() {
        let mut completion = IncludeSectionsCompletion::new(parameters(), Tail::default());
        let mut state = state();
        completion
            .run_stage(LineCompletionStage::IncludeThickSections, &mut state)
            .unwrap();
        completion
            .run_stage(LineCompletionStage::IncludeThinSections, &mut state)
            .unwrap();
        completion
            .run_stage(LineCompletionStage::PolishCurvatures, &mut state)
            .unwrap();
        assert_eq!(completion.next().0, [LineCompletionStage::PolishCurvatures]);
    }
}
