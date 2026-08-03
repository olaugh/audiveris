// SPDX-License-Identifier: AGPL-3.0-or-later

//! Concrete Java `LinesRetriever.includeStickers` over prepared staff lines.

use std::error::Error;
use std::fmt;

use crate::{
    filament::FilamentError,
    grid_lifecycle::GridStageFailure,
    line_completion::LineCompletionStage,
    line_sections::{LineSectionsError, adjacent_stickers, all_stickers},
    prepared_completion::{PreparedCompletionState, RemainingLineCompletionStages},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IncludeStickersParameters {
    pub maximum_connection_length: usize,
}

/// One non-empty Java per-line sticker mutation boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedStickerInclusionBatch {
    pub staff_id: usize,
    pub line_id: usize,
    pub section_ids: Vec<usize>,
    pub preserved_start: (f64, f64),
    pub preserved_stop: (f64, f64),
    /// False if endpoint recomputation failed after retaining added sections.
    pub endpoints_restored: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum IncludeStickersError {
    MissingBinaryBuffer,
    StickerSelection(LineSectionsError),
    AdjacentDiscovery {
        staff_id: usize,
        line_id: usize,
        source: LineSectionsError,
    },
    Filament {
        staff_id: usize,
        line_id: usize,
        source: FilamentError,
    },
}

impl fmt::Display for IncludeStickersError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBinaryBuffer => {
                formatter.write_str("includeStickers requires the loaded binary buffer")
            }
            Self::StickerSelection(error) => {
                write!(
                    formatter,
                    "includeStickers candidate selection failed: {error}"
                )
            }
            Self::AdjacentDiscovery {
                staff_id,
                line_id,
                source,
            } => write!(
                formatter,
                "staff {staff_id} line {line_id} sticker discovery failed: {source}"
            ),
            Self::Filament {
                staff_id,
                line_id,
                source,
            } => write!(
                formatter,
                "staff {staff_id} line {line_id} sticker inclusion failed: {source}"
            ),
        }
    }
}

impl Error for IncludeStickersError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::StickerSelection(error) | Self::AdjacentDiscovery { source: error, .. } => {
                Some(error)
            }
            Self::Filament { source, .. } => Some(source),
            Self::MissingBinaryBuffer => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum IncludeStickersCompletionError<NextError> {
    IncludeStickers(IncludeStickersError),
    Next(NextError),
}

impl<NextError: fmt::Display> fmt::Display for IncludeStickersCompletionError<NextError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncludeStickers(error) => error.fmt(formatter),
            Self::Next(error) => error.fmt(formatter),
        }
    }
}

impl<NextError: Error + 'static> Error for IncludeStickersCompletionError<NextError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::IncludeStickers(error) => Some(error),
            Self::Next(error) => Some(error),
        }
    }
}

pub struct IncludeStickersCompletion<Next> {
    parameters: IncludeStickersParameters,
    next: Next,
}

impl<Next> IncludeStickersCompletion<Next> {
    #[must_use]
    pub const fn new(parameters: IncludeStickersParameters, next: Next) -> Self {
        Self { parameters, next }
    }

    #[must_use]
    pub const fn next(&self) -> &Next {
        &self.next
    }
}

impl<Next> RemainingLineCompletionStages for IncludeStickersCompletion<Next>
where
    Next: RemainingLineCompletionStages,
{
    type StepError = Next::StepError;
    type OtherError = IncludeStickersCompletionError<Next::OtherError>;

    fn run_stage(
        &mut self,
        stage: LineCompletionStage,
        state: &mut PreparedCompletionState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        if stage == LineCompletionStage::IncludeStickers {
            include_stickers(state, self.parameters).map_err(|error| {
                GridStageFailure::Other(IncludeStickersCompletionError::IncludeStickers(error))
            })
        } else {
            self.next
                .run_stage(stage, state)
                .map_err(|failure| match failure {
                    GridStageFailure::Step(error) => GridStageFailure::Step(error),
                    GridStageFailure::Other(error) => {
                        GridStageFailure::Other(IncludeStickersCompletionError::Next(error))
                    }
                })
        }
    }

    fn log_swallowed_error(&mut self, error: &Self::OtherError) {
        if let IncludeStickersCompletionError::Next(error) = error {
            self.next.log_swallowed_error(error);
        }
    }

    fn finish(&mut self) {
        self.next.finish();
    }
}

pub fn include_stickers(
    state: &mut PreparedCompletionState,
    parameters: IncludeStickersParameters,
) -> Result<(), IncludeStickersError> {
    state.sticker_inclusion_batches.clear();
    state.sticker_section_ids.clear();
    let sheet_height = state
        .binary_buffer
        .as_ref()
        .ok_or(IncludeStickersError::MissingBinaryBuffer)?
        .height();
    let excluded_ids = state
        .staffs
        .iter()
        .flat_map(|staff| &staff.lines)
        .flat_map(|line| line.filament.sections())
        .map(crate::section::Section::id)
        .collect::<Vec<_>>();
    let stickers = all_stickers(
        &state.horizontal_sections,
        &excluded_ids,
        sheet_height,
        parameters.maximum_connection_length,
    )
    .map_err(IncludeStickersError::StickerSelection)?;
    state.sticker_section_ids = stickers.iter().map(crate::section::Section::id).collect();

    let (staffs, batches) = (&mut state.staffs, &mut state.sticker_inclusion_batches);
    for staff in staffs {
        for line in &mut staff.lines {
            // Java discovers the complete LinkedHashSet before invalidating
            // the filament with the first member addition.
            let to_add = adjacent_stickers(line.filament.sections(), &stickers, sheet_height)
                .map_err(|source| IncludeStickersError::AdjacentDiscovery {
                    staff_id: staff.id,
                    line_id: line.id,
                    source,
                })?;
            if to_add.is_empty() {
                continue;
            }

            let geometry =
                line.filament
                    .geometry()
                    .map_err(|source| IncludeStickersError::Filament {
                        staff_id: staff.id,
                        line_id: line.id,
                        source,
                    })?;
            let preserved_start = geometry.start();
            let preserved_stop = geometry.stop();
            let batch_index = batches.len();
            batches.push(PreparedStickerInclusionBatch {
                staff_id: staff.id,
                line_id: line.id,
                section_ids: Vec::new(),
                preserved_start,
                preserved_stop,
                endpoints_restored: false,
            });

            // Each successful addition is immediately observable. If a later
            // addition or endpoint recomputation fails, Java retains it.
            for sticker in to_add {
                let section_id = sticker.id();
                line.filament.add_section(sticker).map_err(|source| {
                    IncludeStickersError::Filament {
                        staff_id: staff.id,
                        line_id: line.id,
                        source,
                    }
                })?;
                batches[batch_index].section_ids.push(section_id);
            }

            // Unlike includeSections, includeStickers preserves both endpoint
            // ordinates as well as their abscissae.
            line.filament
                .set_ending_points(preserved_start, preserved_stop)
                .map_err(|source| IncludeStickersError::Filament {
                    staff_id: staff.id,
                    line_id: line.id,
                    source,
                })?;
            batches[batch_index].endpoints_restored = true;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        filament::StaffFilament,
        lines_coordinator::StaffCandidateKind,
        prepared_lines::{PreparedStaff, PreparedStaffLine},
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, Section, build_sections_from_id},
    };
    use std::convert::Infallible;

    fn section(id: usize, y: usize, start: usize, length: usize) -> Section {
        let mut table = RunTable::new(Orientation::Horizontal, 30, 8).unwrap();
        table.add_run(y, Run::new(start, length)).unwrap();
        build_sections_from_id(&table, JunctionPolicy::All, id).remove(0)
    }

    fn line(id: usize, member: Section) -> PreparedStaffLine {
        let bounds = member.bounds();
        let y = bounds.y as f64;
        let mut filament = StaffFilament::new(2).unwrap();
        filament.add_section(member).unwrap();
        filament
            .set_ending_points(
                (bounds.x as f64 + 0.25, y + 0.25),
                ((bounds.x + bounds.width - 1) as f64 - 0.25, y + 0.25),
            )
            .unwrap();
        PreparedStaffLine {
            id,
            cluster_position: 0,
            filament,
        }
    }

    fn state(
        lines: Vec<PreparedStaffLine>,
        horizontal_sections: Vec<Section>,
    ) -> PreparedCompletionState {
        PreparedCompletionState {
            staffs: vec![PreparedStaff {
                id: 9,
                kind: StaffCandidateKind::Standard,
                left: 0.0,
                right: 29.0,
                interline: 2,
                small: false,
                short: false,
                lines,
            }],
            completion_systems: None,
            discarded_filaments: Vec::new(),
            horizontal_sections,
            binary_buffer: Some(RunTable::new(Orientation::Horizontal, 30, 8).unwrap()),
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

    fn parameters() -> IncludeStickersParameters {
        IncludeStickersParameters {
            maximum_connection_length: 3,
        }
    }

    #[test]
    fn includes_in_member_side_coordinate_order_and_restores_exact_endpoints() {
        let member = section(100, 2, 4, 7);
        let top_left = section(1, 1, 3, 3);
        let top_right = section(2, 1, 8, 2);
        let bottom = section(3, 3, 5, 2);
        let unrelated = section(4, 3, 20, 2);
        let mut state = state(
            vec![line(7, member.clone())],
            vec![bottom, member, unrelated, top_right, top_left],
        );
        let preserved = state.staffs[0].lines[0].filament.ending_points().unwrap();
        state.staffs[0].lines[0]
            .filament
            .replace_defining_points(vec![preserved.0, (7.0, 5.0), preserved.1])
            .unwrap();

        include_stickers(&mut state, parameters()).unwrap();

        let line = &state.staffs[0].lines[0];
        assert_eq!(line.filament.ending_points(), Some(preserved));
        assert_eq!(state.sticker_section_ids, [1, 2, 3, 4]);
        assert_eq!(
            state.sticker_inclusion_batches,
            [PreparedStickerInclusionBatch {
                staff_id: 9,
                line_id: 7,
                section_ids: vec![1, 2, 3],
                preserved_start: preserved.0,
                preserved_stop: preserved.1,
                endpoints_restored: true,
            }]
        );
        assert_eq!(
            line.filament
                .sections()
                .iter()
                .map(Section::id)
                .collect::<Vec<_>>(),
            [1, 100, 3, 2]
        );
        assert_eq!(line.filament.defining_points().unwrap().len(), 2);
        assert!(
            !line
                .filament
                .defining_points()
                .unwrap()
                .contains(&(7.0, 5.0))
        );
    }

    #[test]
    fn empty_selection_leaves_custom_defining_points_and_endpoints_untouched() {
        let member = section(100, 6, 4, 7);
        let remote = section(1, 1, 5, 2);
        let mut state = state(vec![line(7, member.clone())], vec![remote, member]);
        let preserved = state.staffs[0].lines[0].filament.ending_points().unwrap();
        let defining_points = vec![preserved.0, (7.0, 4.0), preserved.1];
        state.staffs[0].lines[0]
            .filament
            .replace_defining_points(defining_points.clone())
            .unwrap();

        include_stickers(&mut state, parameters()).unwrap();

        let filament = &state.staffs[0].lines[0].filament;
        assert_eq!(state.sticker_section_ids, [1]);
        assert!(state.sticker_inclusion_batches.is_empty());
        assert_eq!(filament.ending_points(), Some(preserved));
        assert_eq!(filament.defining_points().unwrap(), defining_points);
    }

    #[test]
    fn static_candidate_tally_allows_the_same_sticker_on_two_lines() {
        let upper = section(100, 2, 4, 7);
        let lower = section(200, 4, 4, 7);
        let shared = section(1, 3, 5, 2);
        let mut state = state(
            vec![line(7, upper.clone()), line(8, lower.clone())],
            vec![lower, shared, upper],
        );

        include_stickers(&mut state, parameters()).unwrap();

        assert_eq!(state.sticker_inclusion_batches.len(), 2);
        assert_eq!(state.sticker_inclusion_batches[0].section_ids, [1]);
        assert_eq!(state.sticker_inclusion_batches[1].section_ids, [1]);
        assert!(
            state.staffs[0].lines[0]
                .filament
                .sections()
                .iter()
                .any(|section| section.id() == 1)
        );
        assert!(
            state.staffs[0].lines[1]
                .filament
                .sections()
                .iter()
                .any(|section| section.id() == 1)
        );
    }

    #[test]
    fn later_discovery_failure_retains_prior_line_mutation_and_audit() {
        let first = section(100, 2, 4, 7);
        let invalid = section(200, 0, 4, 7);
        let sticker = section(1, 1, 5, 2);
        let mut state = state(
            vec![line(7, first.clone()), line(8, invalid.clone())],
            vec![invalid, sticker, first],
        );

        assert_eq!(
            include_stickers(&mut state, parameters()),
            Err(IncludeStickersError::AdjacentDiscovery {
                staff_id: 9,
                line_id: 8,
                source: LineSectionsError::AdjacentPositionOutOfRange {
                    source_id: 200,
                    top: true,
                },
            })
        );
        assert_eq!(state.sticker_inclusion_batches.len(), 1);
        assert_eq!(state.sticker_inclusion_batches[0].section_ids, [1]);
        assert!(state.sticker_inclusion_batches[0].endpoints_restored);
        assert!(
            state.staffs[0].lines[0]
                .filament
                .sections()
                .iter()
                .any(|section| section.id() == 1)
        );
        assert_eq!(state.staffs[0].lines[1].filament.sections().len(), 1);
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
    fn wrapper_owns_only_include_stickers() {
        let member = section(100, 2, 4, 7);
        let sticker = section(1, 1, 5, 2);
        let mut state = state(vec![line(7, member.clone())], vec![sticker, member]);
        let mut completion = IncludeStickersCompletion::new(parameters(), Tail::default());

        completion
            .run_stage(LineCompletionStage::IncludeStickers, &mut state)
            .unwrap();
        completion
            .run_stage(LineCompletionStage::FillHolesFinal, &mut state)
            .unwrap();

        assert_eq!(completion.next().0, [LineCompletionStage::FillHolesFinal]);
        assert_eq!(state.sticker_inclusion_batches[0].section_ids, [1]);
    }
}
