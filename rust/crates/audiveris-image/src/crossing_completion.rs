// SPDX-License-Identifier: AGPL-3.0-or-later

//! Concrete prepared-staff adapter for Java `LinesInspector.process`.

use std::error::Error;
use std::fmt;

use crate::{
    filament::FilamentError,
    grid_lifecycle::GridStageFailure,
    line_completion::{
        CrossingChunkRemoval, InspectedLineFilament, InspectedStaff, LineCompletionStage,
        LinesInspectorError, LinesInspectorReport, inspect_crossing_chunks_detailed,
    },
    prepared_completion::{PreparedCompletionState, RemainingLineCompletionStages},
    prepared_lines::PreparedStaff,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InspectCrossingChunksParameters {
    pub minimum_offset: usize,
    pub global_slope: f64,
    pub minimum_chunk_slope: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedCrossingLineInspection {
    pub staff_id: usize,
    pub line_id: usize,
    pub chunks: Vec<CrossingChunkRemoval>,
    /// Exact successful `removeSection` prefix in Java traversal order.
    pub removed_section_ids: Vec<usize>,
    /// False when `computeLine` failed after retaining all prior removals.
    pub recomputed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PreparedCrossingMutationError {
    MissingStaff {
        staff_id: usize,
    },
    MissingLine {
        staff_id: usize,
        line_id: usize,
    },
    MissingSection {
        staff_id: usize,
        line_id: usize,
        section_id: usize,
    },
    Filament {
        staff_id: usize,
        line_id: usize,
        source: FilamentError,
    },
}

impl fmt::Display for PreparedCrossingMutationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingStaff { staff_id } => {
                write!(formatter, "crossing inspection lost staff {staff_id}")
            }
            Self::MissingLine { staff_id, line_id } => write!(
                formatter,
                "crossing inspection lost staff {staff_id} line {line_id}"
            ),
            Self::MissingSection {
                staff_id,
                line_id,
                section_id,
            } => write!(
                formatter,
                "crossing inspection lost section {section_id} from staff {staff_id} line {line_id}"
            ),
            Self::Filament {
                staff_id,
                line_id,
                source,
            } => write!(
                formatter,
                "staff {staff_id} line {line_id} recomputation failed: {source}"
            ),
        }
    }
}

impl Error for PreparedCrossingMutationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Filament { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PreparedCrossingInspectionError {
    InvalidStaffLeft { staff_id: usize, left: f64 },
    Inspector(LinesInspectorError<PreparedCrossingMutationError>),
}

impl fmt::Display for PreparedCrossingInspectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStaffLeft { staff_id, left } => {
                write!(
                    formatter,
                    "staff {staff_id} has invalid left abscissa {left}"
                )
            }
            Self::Inspector(error) => error.fmt(formatter),
        }
    }
}

impl Error for PreparedCrossingInspectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidStaffLeft { .. } => None,
            Self::Inspector(error) => Some(error),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum InspectCrossingChunksCompletionError<NextError> {
    InspectCrossingChunks(PreparedCrossingInspectionError),
    Next(NextError),
}

impl<NextError: fmt::Display> fmt::Display for InspectCrossingChunksCompletionError<NextError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InspectCrossingChunks(error) => error.fmt(formatter),
            Self::Next(error) => error.fmt(formatter),
        }
    }
}

impl<NextError: Error + 'static> Error for InspectCrossingChunksCompletionError<NextError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InspectCrossingChunks(error) => Some(error),
            Self::Next(error) => Some(error),
        }
    }
}

pub struct InspectCrossingChunksCompletion<Next> {
    parameters: InspectCrossingChunksParameters,
    next: Next,
}

impl<Next> InspectCrossingChunksCompletion<Next> {
    #[must_use]
    pub const fn new(parameters: InspectCrossingChunksParameters, next: Next) -> Self {
        Self { parameters, next }
    }

    #[must_use]
    pub const fn next(&self) -> &Next {
        &self.next
    }
}

impl<Next> RemainingLineCompletionStages for InspectCrossingChunksCompletion<Next>
where
    Next: RemainingLineCompletionStages,
{
    type StepError = Next::StepError;
    type OtherError = InspectCrossingChunksCompletionError<Next::OtherError>;

    fn run_stage(
        &mut self,
        stage: LineCompletionStage,
        state: &mut PreparedCompletionState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        if stage == LineCompletionStage::InspectCrossingChunks {
            // Raw retrieval measures the authoritative slope only after this
            // wrapper has been constructed. Prepared callers without raw
            // metadata retain the explicit parameter as their bounded seam.
            let global_slope = state.global_slope.unwrap_or(self.parameters.global_slope);
            inspect_prepared_crossing_chunks(
                &mut state.staffs,
                self.parameters.minimum_offset,
                global_slope,
                self.parameters.minimum_chunk_slope,
                &mut state.crossing_line_inspections,
            )
            .map(|_| ())
            .map_err(|error| {
                GridStageFailure::Other(
                    InspectCrossingChunksCompletionError::InspectCrossingChunks(error),
                )
            })
        } else {
            self.next
                .run_stage(stage, state)
                .map_err(|failure| match failure {
                    GridStageFailure::Step(error) => GridStageFailure::Step(error),
                    GridStageFailure::Other(error) => {
                        GridStageFailure::Other(InspectCrossingChunksCompletionError::Next(error))
                    }
                })
        }
    }

    fn log_swallowed_error(&mut self, error: &Self::OtherError) {
        if let InspectCrossingChunksCompletionError::Next(error) = error {
            self.next.log_swallowed_error(error);
        }
    }

    fn finish(&mut self) {
        self.next.finish();
    }
}

/// Inspect and mutate prepared staff filaments in Java staff/line/member order.
///
/// `minimum_offset` is already scale-resolved. Staff left edges use Java
/// `Math.rint` semantics before the header exclusion boundary is formed.
pub fn inspect_prepared_crossing_chunks(
    staffs: &mut [PreparedStaff],
    minimum_offset: usize,
    global_slope: f64,
    minimum_chunk_slope: f64,
    audits: &mut Vec<PreparedCrossingLineInspection>,
) -> Result<LinesInspectorReport, PreparedCrossingInspectionError> {
    let mut inspected = staffs
        .iter()
        .map(|staff| {
            if !staff.left.is_finite() || staff.left < 0.0 {
                return Err(PreparedCrossingInspectionError::InvalidStaffLeft {
                    staff_id: staff.id,
                    left: staff.left,
                });
            }
            Ok(InspectedStaff {
                id: staff.id,
                left_abscissa: staff.left.round_ties_even() as usize,
                lines: staff
                    .lines
                    .iter()
                    .map(|line| InspectedLineFilament {
                        id: line.id,
                        members: line.filament.sections().to_vec(),
                    })
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    inspect_crossing_chunks_detailed(
        &mut inspected,
        minimum_offset,
        global_slope,
        minimum_chunk_slope,
        |staff_id, line_id, chunks, _| {
            let staff = staffs
                .iter_mut()
                .find(|staff| staff.id == staff_id)
                .ok_or(PreparedCrossingMutationError::MissingStaff { staff_id })?;
            let line = staff
                .lines
                .iter_mut()
                .find(|line| line.id == line_id)
                .ok_or(PreparedCrossingMutationError::MissingLine { staff_id, line_id })?;
            // Java `removeSection` invalidates compound bounds but retains
            // CurvedFilament endpoints for the subsequent `computeLine`.
            let geometry = line.filament.geometry().map_err(|source| {
                PreparedCrossingMutationError::Filament {
                    staff_id,
                    line_id,
                    source,
                }
            })?;
            let old_start = geometry.start();
            let old_stop = geometry.stop();
            audits.push(PreparedCrossingLineInspection {
                staff_id,
                line_id,
                chunks: chunks.to_vec(),
                removed_section_ids: Vec::new(),
                recomputed: false,
            });
            let audit = audits.last_mut().expect("inspection audit just appended");
            for chunk in chunks {
                for &section_id in &chunk.section_ids {
                    if !line.filament.remove_section_by_id(section_id) {
                        return Err(PreparedCrossingMutationError::MissingSection {
                            staff_id,
                            line_id,
                            section_id,
                        });
                    }
                    audit.removed_section_ids.push(section_id);
                }
            }
            line.filament
                .set_ending_points(old_start, old_stop)
                .map_err(|source| PreparedCrossingMutationError::Filament {
                    staff_id,
                    line_id,
                    source,
                })?;
            audit.recomputed = true;
            Ok(())
        },
    )
    .map_err(PreparedCrossingInspectionError::Inspector)
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use super::*;
    use crate::{
        filament::StaffFilament,
        lines_coordinator::StaffCandidateKind,
        prepared_lines::PreparedStaffLine,
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, Section, build_sections_from_id},
    };

    fn section(id: usize, rows: &[(usize, usize, usize)]) -> Section {
        let width = rows
            .iter()
            .map(|(_, start, length)| start + length)
            .max()
            .unwrap_or(1)
            + 1;
        let height = rows.iter().map(|(y, _, _)| y + 1).max().unwrap_or(1) + 1;
        let mut table = RunTable::new(Orientation::Horizontal, width, height).unwrap();
        for &(y, start, length) in rows {
            table.add_run(y, Run::new(start, length)).unwrap();
        }
        build_sections_from_id(&table, JunctionPolicy::All, id).remove(0)
    }

    fn crossing(first_id: usize, x: usize, y: usize) -> [Section; 2] {
        [
            section(first_id, &[(y, x, 2)]),
            section(first_id + 1, &[(y + 1, x + 1, 2)]),
        ]
    }

    fn line(id: usize, members: Vec<Section>) -> PreparedStaffLine {
        let mut filament = StaffFilament::new(10).unwrap();
        for member in members {
            filament.add_section(member).unwrap();
        }
        PreparedStaffLine {
            id,
            cluster_position: 0,
            filament,
        }
    }

    fn staff(lines: Vec<PreparedStaffLine>) -> PreparedStaff {
        PreparedStaff {
            id: 7,
            kind: StaffCandidateKind::Standard,
            left: 0.0,
            right: 100.0,
            interline: 10,
            small: false,
            short: false,
            tentative: false,
            lines,
        }
    }

    fn completion_state(staffs: Vec<PreparedStaff>) -> PreparedCompletionState {
        PreparedCompletionState {
            staffs,
            global_slope: None,
            completion_systems: None,
            discarded_filaments: Vec::new(),
            horizontal_sections: Vec::new(),
            binary_buffer: None,
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

    fn parameters() -> InspectCrossingChunksParameters {
        InspectCrossingChunksParameters {
            minimum_offset: 10,
            global_slope: 0.0,
            minimum_chunk_slope: 0.04,
        }
    }

    #[test]
    fn removes_in_sorted_chunk_order_then_recomputes_once() {
        let [one, two] = crossing(10, 10, 10);
        let survivor = section(30, &[(12, 30, 20)]);
        let mut staffs = vec![staff(vec![line(3, vec![two, survivor, one])])];
        let mut audits = Vec::new();
        let original = staffs[0].lines[0].filament.geometry().unwrap();

        let report =
            inspect_prepared_crossing_chunks(&mut staffs, 10, 0.0, 0.04, &mut audits).unwrap();

        assert_eq!(report.updates.len(), 1);
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].removed_section_ids, [10, 11]);
        assert!(audits[0].recomputed);
        let recomputed = staffs[0].lines[0].filament.geometry().unwrap();
        assert_eq!(recomputed.start(), original.start());
        assert_eq!(recomputed.stop(), original.stop());
        assert_eq!(
            staffs[0].lines[0]
                .filament
                .sections()
                .iter()
                .map(Section::id)
                .collect::<Vec<_>>(),
            [30]
        );
    }

    #[test]
    fn rounded_staff_left_excludes_header_members() {
        let header = crossing(1, 10, 10);
        let body = crossing(10, 12, 20);
        let survivor = section(30, &[(22, 30, 20)]);
        let mut prepared = staff(vec![line(
            3,
            vec![
                header[0].clone(),
                header[1].clone(),
                body[0].clone(),
                body[1].clone(),
                survivor,
            ],
        )]);
        prepared.left = 1.5; // Java Math.rint -> 2, xMin -> 12.
        let mut staffs = vec![prepared];
        let mut audits = Vec::new();

        inspect_prepared_crossing_chunks(&mut staffs, 10, 0.0, 0.04, &mut audits).unwrap();

        assert_eq!(audits[0].removed_section_ids, [10, 11]);
        assert_eq!(
            staffs[0].lines[0]
                .filament
                .sections()
                .iter()
                .map(Section::id)
                .collect::<Vec<_>>(),
            [1, 2, 30]
        );
    }

    #[test]
    fn recompute_failure_retains_removals_and_incomplete_audit() {
        let first = crossing(1, 10, 10);
        let survivor = section(9, &[(12, 30, 20)]);
        let second = crossing(20, 40, 20);
        let mut staffs = vec![staff(vec![
            line(3, vec![first[0].clone(), first[1].clone(), survivor]),
            line(4, second.to_vec()),
        ])];
        let mut audits = Vec::new();

        let result = inspect_prepared_crossing_chunks(&mut staffs, 10, 0.0, 0.04, &mut audits);

        assert!(matches!(
            result,
            Err(PreparedCrossingInspectionError::Inspector(
                LinesInspectorError::Recompute(PreparedCrossingMutationError::Filament {
                    staff_id: 7,
                    line_id: 4,
                    source: FilamentError::Empty,
                })
            ))
        ));
        assert_eq!(audits.len(), 2);
        assert!(audits[0].recomputed);
        assert_eq!(audits[0].removed_section_ids, [1, 2]);
        assert!(!audits[1].recomputed);
        assert_eq!(audits[1].removed_section_ids, [20, 21]);
        assert!(staffs[1 - 1].lines[1].filament.sections().is_empty());
    }

    #[test]
    fn wrapper_owns_only_crossing_inspection_stage() {
        let [one, two] = crossing(10, 10, 10);
        let survivor = section(30, &[(12, 30, 20)]);
        let mut state = completion_state(vec![staff(vec![line(3, vec![two, survivor, one])])]);
        let mut completion = InspectCrossingChunksCompletion::new(parameters(), Tail::default());

        completion
            .run_stage(LineCompletionStage::InspectCrossingChunks, &mut state)
            .unwrap();
        completion
            .run_stage(LineCompletionStage::FillHolesFinal, &mut state)
            .unwrap();

        assert_eq!(state.crossing_line_inspections.len(), 1);
        assert_eq!(
            completion.next().0,
            vec![LineCompletionStage::FillHolesFinal]
        );
    }

    #[test]
    fn wrapper_prefers_late_bound_raw_slope_over_constructor_placeholder() {
        let [one, two] = crossing(10, 10, 10);
        let survivor = section(30, &[(12, 30, 20)]);
        let mut state = completion_state(vec![staff(vec![line(3, vec![one, two, survivor])])]);
        // The crossing pixels have slope 1/3. The static placeholder (0.0)
        // would remove them; the measured raw slope must retain them.
        state.global_slope = Some(1.0 / 3.0);
        let mut completion = InspectCrossingChunksCompletion::new(parameters(), Tail::default());

        completion
            .run_stage(LineCompletionStage::InspectCrossingChunks, &mut state)
            .unwrap();

        assert!(state.crossing_line_inspections.is_empty());
        assert_eq!(
            state.staffs[0].lines[0]
                .filament
                .sections()
                .iter()
                .map(Section::id)
                .collect::<Vec<_>>(),
            [10, 11, 30]
        );
    }
}
