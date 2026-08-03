// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production-backed `RetrieveLines` for prepared cluster-pass inputs.
//!
//! The cluster coordinators are prepared upstream from the live horizontal
//! lag. This adapter verifies that provenance, executes
//! [`retrieve_staff_candidates`], and retains an owned staff/filament prefix
//! for the sheet-aware driver. The coordinator itself is transactional on
//! exceptional paths; this is deliberately not a claim of Java-compatible
//! failure-prefix semantics yet.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    filament::StaffFilament,
    grid_lifecycle::{GridBuildStage, GridStageFailure},
    line_cluster::FilamentId,
    lines_coordinator::{
        ClusterPass, ClusterPassState, LinesCoordinatorError, LinesCoordinatorParameters,
        StaffCandidateKind, retrieve_staff_candidates,
    },
    raster_grid_builder::{
        HeadlessRasterGridBuilder, RasterGridBuildState, RemainingRasterGridStages,
    },
    section::Section,
};

#[derive(Clone, Debug)]
pub struct PreparedStaffLine {
    pub id: usize,
    pub filament: StaffFilament,
}

#[derive(Clone, Debug)]
pub struct PreparedStaff {
    pub id: usize,
    pub kind: StaffCandidateKind,
    pub left: f64,
    pub right: f64,
    pub interline: usize,
    pub small: bool,
    pub short: bool,
    pub lines: Vec<PreparedStaffLine>,
}

#[derive(Clone, Debug)]
pub struct PreparedStaffHandoff {
    pub staffs: Vec<PreparedStaff>,
}

pub trait PreparedStaffHandoffSource {
    fn take_prepared_staff_handoff(&mut self) -> Option<PreparedStaffHandoff>;
}

pub trait PreparedStaffStage {
    fn prepared_staff_handoff(&self) -> Option<&PreparedStaffHandoff>;
    fn take_prepared_staff_handoff(&mut self) -> Option<PreparedStaffHandoff>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProductionRetrieveLinesError<DownstreamError> {
    MissingHorizontalLag,
    DuplicateLagSection(usize),
    Lines(LinesCoordinatorError),
    CandidateIdMismatch {
        expected: usize,
        actual: usize,
    },
    MissingCandidateCluster {
        candidate: usize,
    },
    CandidateLineIdsMismatch {
        candidate: usize,
    },
    DuplicateFilamentId(FilamentId),
    FilamentIdOverflow(FilamentId),
    UnknownLagSection {
        filament: FilamentId,
        section: usize,
    },
    LagSectionMismatch {
        filament: FilamentId,
        section: usize,
    },
    InterlineMismatch {
        candidate: usize,
        filament: FilamentId,
    },
    Downstream(DownstreamError),
}

/// Replace only the supplied `RetrieveLines` stage. Bars and completion remain
/// delegated until their production adapters are joined.
pub struct ProductionRetrieveLines<Downstream> {
    primary: ClusterPassState,
    secondary: Option<ClusterPassState>,
    parameters: LinesCoordinatorParameters,
    downstream: Downstream,
    handoff: Option<PreparedStaffHandoff>,
}

impl<Downstream> ProductionRetrieveLines<Downstream> {
    #[must_use]
    pub fn new(
        primary: ClusterPassState,
        secondary: Option<ClusterPassState>,
        parameters: LinesCoordinatorParameters,
        downstream: Downstream,
    ) -> Self {
        Self {
            primary,
            secondary,
            parameters,
            downstream,
            handoff: None,
        }
    }

    pub fn take_handoff(&mut self) -> Option<PreparedStaffHandoff> {
        self.handoff.take()
    }

    #[must_use]
    pub const fn downstream(&self) -> &Downstream {
        &self.downstream
    }
}

impl<Stages, Vip> PreparedStaffHandoffSource for HeadlessRasterGridBuilder<Stages, Vip>
where
    Stages: PreparedStaffStage,
{
    fn take_prepared_staff_handoff(&mut self) -> Option<PreparedStaffHandoff> {
        self.stages_mut().take_prepared_staff_handoff()
    }
}

impl<Downstream> PreparedStaffStage for ProductionRetrieveLines<Downstream> {
    fn prepared_staff_handoff(&self) -> Option<&PreparedStaffHandoff> {
        self.handoff.as_ref()
    }

    fn take_prepared_staff_handoff(&mut self) -> Option<PreparedStaffHandoff> {
        self.take_handoff()
    }
}

impl<Downstream> RemainingRasterGridStages for ProductionRetrieveLines<Downstream>
where
    Downstream: RemainingRasterGridStages,
{
    type StepError = Downstream::StepError;
    type OtherError = ProductionRetrieveLinesError<Downstream::OtherError>;

    fn retrieve_lines(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.handoff = None;
        let lag = state.horizontal_lag().ok_or_else(|| {
            GridStageFailure::Other(ProductionRetrieveLinesError::MissingHorizontalLag)
        })?;
        let lag_sections = index_lag_sections(lag.sections()).map_err(GridStageFailure::Other)?;
        let result = if let Some(secondary) = self.secondary.as_mut() {
            retrieve_staff_candidates(&mut self.primary, Some(secondary), self.parameters)
        } else {
            retrieve_staff_candidates(&mut self.primary, None, self.parameters)
        }
        .map_err(|error| GridStageFailure::Other(ProductionRetrieveLinesError::Lines(error)))?;
        self.handoff = Some(
            materialize_staffs(
                result.staffs(),
                &self.primary,
                self.secondary.as_ref(),
                &lag_sections,
            )
            .map_err(GridStageFailure::Other)?,
        );
        Ok(())
    }

    fn process_bars(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.downstream
            .process_bars(state)
            .map_err(map_downstream_failure)
    }

    fn complete_lines(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.downstream
            .complete_lines(state)
            .map_err(map_downstream_failure)
    }

    fn log_swallowed_error(&mut self, stage: GridBuildStage, error: &Self::OtherError) {
        if let ProductionRetrieveLinesError::Downstream(error) = error {
            self.downstream.log_swallowed_error(stage, error);
        }
    }

    fn finish(&mut self) {
        self.downstream.finish();
    }
}

fn map_downstream_failure<StepError, DownstreamError>(
    failure: GridStageFailure<StepError, DownstreamError>,
) -> GridStageFailure<StepError, ProductionRetrieveLinesError<DownstreamError>> {
    match failure {
        GridStageFailure::Step(error) => GridStageFailure::Step(error),
        GridStageFailure::Other(error) => {
            GridStageFailure::Other(ProductionRetrieveLinesError::Downstream(error))
        }
    }
}

fn index_lag_sections<DownstreamError>(
    sections: &[Section],
) -> Result<BTreeMap<usize, &Section>, ProductionRetrieveLinesError<DownstreamError>> {
    let mut indexed = BTreeMap::new();
    for section in sections {
        if indexed.insert(section.id(), section).is_some() {
            return Err(ProductionRetrieveLinesError::DuplicateLagSection(
                section.id(),
            ));
        }
    }
    Ok(indexed)
}

fn materialize_staffs<DownstreamError>(
    candidates: &[crate::lines_coordinator::StaffCandidate],
    primary: &ClusterPassState,
    secondary: Option<&ClusterPassState>,
    lag_sections: &BTreeMap<usize, &Section>,
) -> Result<PreparedStaffHandoff, ProductionRetrieveLinesError<DownstreamError>> {
    let mut seen_filaments = BTreeSet::new();
    let mut staffs = Vec::with_capacity(candidates.len());
    for (index, candidate) in candidates.iter().enumerate() {
        let expected = index + 1;
        if candidate.id() != expected {
            return Err(ProductionRetrieveLinesError::CandidateIdMismatch {
                expected,
                actual: candidate.id(),
            });
        }
        let pass = match candidate.source().pass() {
            ClusterPass::Main => primary,
            ClusterPass::Small => {
                secondary.ok_or(ProductionRetrieveLinesError::MissingCandidateCluster {
                    candidate: candidate.id(),
                })?
            }
        };
        let cluster = pass.clusters().get(&candidate.source().cluster()).ok_or(
            ProductionRetrieveLinesError::MissingCandidateCluster {
                candidate: candidate.id(),
            },
        )?;
        let cluster_ids = cluster
            .lines()
            .map(|(_, line)| line.primary_id())
            .collect::<Vec<_>>();
        if cluster_ids != candidate.line_ids() {
            return Err(ProductionRetrieveLinesError::CandidateLineIdsMismatch {
                candidate: candidate.id(),
            });
        }
        let mut lines = Vec::with_capacity(cluster_ids.len());
        for (_, line) in cluster.lines() {
            let id = line.primary_id();
            if !seen_filaments.insert(id) {
                return Err(ProductionRetrieveLinesError::DuplicateFilamentId(id));
            }
            if line.filament().interline() != candidate.interline() {
                return Err(ProductionRetrieveLinesError::InterlineMismatch {
                    candidate: candidate.id(),
                    filament: id,
                });
            }
            for section in line.filament().sections() {
                let registered = lag_sections.get(&section.id()).ok_or(
                    ProductionRetrieveLinesError::UnknownLagSection {
                        filament: id,
                        section: section.id(),
                    },
                )?;
                if *registered != section {
                    return Err(ProductionRetrieveLinesError::LagSectionMismatch {
                        filament: id,
                        section: section.id(),
                    });
                }
            }
            lines.push(PreparedStaffLine {
                id: usize::try_from(id.value())
                    .map_err(|_| ProductionRetrieveLinesError::FilamentIdOverflow(id))?,
                filament: line.filament().clone(),
            });
        }
        staffs.push(PreparedStaff {
            id: candidate.id(),
            kind: candidate.kind(),
            left: candidate.left(),
            right: candidate.right(),
            interline: candidate.interline(),
            small: candidate.is_small(),
            short: candidate.is_short(),
            lines,
        });
    }
    Ok(PreparedStaffHandoff { staffs })
}
