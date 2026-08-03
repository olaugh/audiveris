// SPDX-License-Identifier: AGPL-3.0-or-later

//! Transactional headless boundary for Java `GridBuilder.buildInfo`.
//!
//! Lag creation, short-section insertion, final line completion, and Sheet/SIG
//! writes remain in their raster or application adapters. This module composes
//! the dependency-complete core in Java order: line/staff retrieval first, then
//! bar/system processing.

use std::{collections::HashSet, error::Error, fmt};

use crate::{
    bar_column::StaffId,
    bars_coordinator::{
        BarsCoordinatorError, BarsCoordinatorParameters, BarsCoordinatorResult, BarsSystemState,
        process_bars_system,
    },
    lines_coordinator::{
        ClusterPassState, LinesCoordinatorError, LinesCoordinatorParameters, LinesRetrievalResult,
        StaffCandidateKind, retrieve_staff_candidates,
    },
};

#[derive(Clone, Debug)]
pub struct HeadlessGridState {
    primary_lines: ClusterPassState,
    secondary_lines: Option<ClusterPassState>,
    bar_systems: Vec<BarsSystemState>,
}

impl HeadlessGridState {
    pub fn new(
        primary_lines: ClusterPassState,
        secondary_lines: Option<ClusterPassState>,
        bar_systems: Vec<BarsSystemState>,
    ) -> Result<Self, GridCoordinatorError> {
        if bar_systems
            .windows(2)
            .any(|pair| pair[0].system_id() >= pair[1].system_id())
        {
            return Err(GridCoordinatorError::InvalidSystemOrder);
        }
        Ok(Self {
            primary_lines,
            secondary_lines,
            bar_systems,
        })
    }

    #[must_use]
    pub const fn primary_lines(&self) -> &ClusterPassState {
        &self.primary_lines
    }

    #[must_use]
    pub const fn secondary_lines(&self) -> Option<&ClusterPassState> {
        self.secondary_lines.as_ref()
    }

    #[must_use]
    pub fn bar_systems(&self) -> &[BarsSystemState] {
        &self.bar_systems
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeadlessGridResult {
    lines: LinesRetrievalResult,
    bars: Vec<BarsCoordinatorResult>,
    discarded_one_line_staffs: Vec<StaffId>,
}

impl HeadlessGridResult {
    #[must_use]
    pub const fn lines(&self) -> &LinesRetrievalResult {
        &self.lines
    }

    #[must_use]
    pub fn bars(&self) -> &[BarsCoordinatorResult] {
        &self.bars
    }

    #[must_use]
    pub fn discarded_one_line_staffs(&self) -> &[StaffId] {
        &self.discarded_one_line_staffs
    }
}

/// Compose neutral `LinesRetriever.retrieveLines` and
/// `BarsRetriever.process`, committing only after all systems succeed.
///
/// Bar-system states are prepared by the projection/system-discovery adapter.
/// Every bar staff must refer to the sequential ID and one-line kind allocated
/// by line retrieval. An omitted staff is accepted only when it is a one-line
/// staff, matching Java `PeakGraph.findBarPeaks`' removal rule.
pub fn process_headless_grid(
    state: &mut HeadlessGridState,
    lines_parameters: LinesCoordinatorParameters,
    bars_parameters: BarsCoordinatorParameters,
) -> Result<HeadlessGridResult, GridCoordinatorError> {
    let mut next = state.clone();
    let lines = if let Some(secondary) = next.secondary_lines.as_mut() {
        retrieve_staff_candidates(&mut next.primary_lines, Some(secondary), lines_parameters)?
    } else {
        retrieve_staff_candidates(&mut next.primary_lines, None, lines_parameters)?
    };

    let discarded_one_line_staffs = validate_staff_system_join(&lines, &next.bar_systems)?;
    let mut bars = Vec::with_capacity(next.bar_systems.len());
    for system in &mut next.bar_systems {
        bars.push(process_bars_system(system, bars_parameters)?);
    }

    *state = next;
    Ok(HeadlessGridResult {
        lines,
        bars,
        discarded_one_line_staffs,
    })
}

fn validate_staff_system_join(
    lines: &LinesRetrievalResult,
    systems: &[BarsSystemState],
) -> Result<Vec<StaffId>, GridCoordinatorError> {
    let candidates = lines.staffs();
    let mut seen = HashSet::new();
    let mut previous_staff = None;
    for system in systems {
        for staff in system.staffs() {
            let id = staff.staff_id();
            if previous_staff.is_some_and(|previous: StaffId| previous.value() >= id.value()) {
                return Err(GridCoordinatorError::InvalidStaffOrder(id));
            }
            previous_staff = Some(id);
            if !seen.insert(id) {
                return Err(GridCoordinatorError::DuplicateBarsStaff(id));
            }
            let candidate = candidates
                .iter()
                .find(|candidate| candidate.id() == id.value())
                .ok_or(GridCoordinatorError::UnknownBarsStaff(id))?;
            let candidate_one_line = candidate.kind() == StaffCandidateKind::OneLine;
            if candidate_one_line != staff.is_one_line() {
                return Err(GridCoordinatorError::StaffKindMismatch(id));
            }
        }
    }

    let mut discarded = Vec::new();
    for candidate in candidates {
        let id = StaffId::new(candidate.id());
        if seen.contains(&id) {
            continue;
        }
        if candidate.kind() != StaffCandidateKind::OneLine {
            return Err(GridCoordinatorError::MissingBarsStaff(id));
        }
        discarded.push(id);
    }
    Ok(discarded)
}

#[derive(Clone, Debug, PartialEq)]
pub enum GridCoordinatorError {
    InvalidSystemOrder,
    InvalidStaffOrder(StaffId),
    DuplicateBarsStaff(StaffId),
    UnknownBarsStaff(StaffId),
    MissingBarsStaff(StaffId),
    StaffKindMismatch(StaffId),
    Lines(LinesCoordinatorError),
    Bars(BarsCoordinatorError),
}

impl From<LinesCoordinatorError> for GridCoordinatorError {
    fn from(value: LinesCoordinatorError) -> Self {
        Self::Lines(value)
    }
}

impl From<BarsCoordinatorError> for GridCoordinatorError {
    fn from(value: BarsCoordinatorError) -> Self {
        Self::Bars(value)
    }
}

impl fmt::Display for GridCoordinatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSystemOrder => {
                formatter.write_str("bar systems are not in Java system order")
            }
            Self::InvalidStaffOrder(id) => {
                write!(formatter, "bar staff {} is out of sheet order", id.value())
            }
            Self::DuplicateBarsStaff(id) => write!(
                formatter,
                "bar staff {} occurs in multiple systems",
                id.value()
            ),
            Self::UnknownBarsStaff(id) => write!(
                formatter,
                "bar staff {} was not produced by line retrieval",
                id.value()
            ),
            Self::MissingBarsStaff(id) => write!(
                formatter,
                "non-one-line staff {} is missing from bar systems",
                id.value()
            ),
            Self::StaffKindMismatch(id) => write!(
                formatter,
                "bar staff {} has a mismatched line-count kind",
                id.value()
            ),
            Self::Lines(error) => write!(formatter, "line retrieval failed: {error}"),
            Self::Bars(error) => write!(formatter, "bar retrieval failed: {error}"),
        }
    }
}

impl Error for GridCoordinatorError {}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::{
        bar_alignment::{AlignmentPeak, BarAlignment, BarImpacts},
        bar_column::PeakId,
        bars_coordinator::BarsStaffState,
        cluster_coordinator::RecursiveCombSnapshot,
        cluster_expand::ClusterExpansionParameters,
        cluster_merge::{ClusterMergeParameters, ClusterMergePassParameters},
        cluster_ownership::{ClusterOwnership, CombId},
        cluster_pipeline::ClusterRetrievalParameters,
        filament::StaffFilament,
        filament_comb::FilamentComb,
        line_cluster::FilamentId,
        peak_graph::PeakGraph,
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, Section, build_sections},
        staff_peak::{StaffPeak, StaffPeakKey},
    };

    fn sections() -> Vec<Section> {
        let mut table = RunTable::new(Orientation::Horizontal, 42, 62).unwrap();
        for y in [10, 50, 60] {
            table.add_run(y, Run::new(0, 40)).unwrap();
        }
        build_sections(&table, JunctionPolicy::All)
    }

    fn filament(interline: usize, section: Section) -> StaffFilament {
        let mut filament = StaffFilament::new(interline).unwrap();
        filament.add_section(section).unwrap();
        filament
    }

    fn retrieval_parameters(
        interline: usize,
        desired_sizes: BTreeSet<usize>,
    ) -> ClusterRetrievalParameters {
        let expansion = ClusterExpansionParameters::new(0.0, 0, 1, 0, 0.1, 1, 10).unwrap();
        let compatibility = ClusterMergeParameters::new(0.0, 0, 0.1, 0, 1, 10).unwrap();
        ClusterRetrievalParameters::new(
            interline,
            desired_sizes,
            expansion,
            ClusterMergePassParameters::new(compatibility, 0, 1).unwrap(),
            0.0,
            0.0,
            0.0,
            1,
            0,
            None,
            1,
        )
        .unwrap()
    }

    fn line_states() -> (ClusterPassState, ClusterPassState) {
        let source = sections();
        let mut primary_ownership = ClusterOwnership::new();
        let primary_filaments = [
            (FilamentId::new(1), filament(10, source[0].clone())),
            (FilamentId::new(2), filament(10, source[1].clone())),
            (FilamentId::new(3), filament(10, source[2].clone())),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        for (&id, value) in &primary_filaments {
            primary_ownership.register_filament(id, value).unwrap();
        }
        let mut comb = FilamentComb::new(0);
        comb.append_root(2, 50.0).unwrap();
        comb.append_root(3, 60.0).unwrap();
        let comb_id = CombId::new(1);
        primary_ownership.register_comb(comb_id, &comb).unwrap();
        let primary = ClusterPassState::new(
            primary_ownership,
            primary_filaments,
            BTreeMap::from([(comb_id, RecursiveCombSnapshot::from_comb(&comb))]),
            vec![FilamentId::new(1), FilamentId::new(2), FilamentId::new(3)],
            retrieval_parameters(10, BTreeSet::from([2])),
        );

        let mut secondary_ownership = ClusterOwnership::new();
        let secondary_value = filament(5, source[0].clone());
        secondary_ownership
            .register_filament(FilamentId::new(1), &secondary_value)
            .unwrap();
        let secondary = ClusterPassState::new(
            secondary_ownership,
            BTreeMap::from([(FilamentId::new(1), secondary_value)]),
            BTreeMap::new(),
            Vec::new(),
            retrieval_parameters(5, BTreeSet::from([1])),
        );
        (primary, secondary)
    }

    fn peak(staff: usize) -> StaffPeak {
        let mut peak = StaffPeak::new(StaffId::new(staff), 10, 20, 10, 11).unwrap();
        peak.compute_deskewed_center(|point| point).unwrap();
        peak
    }

    fn connection(top: StaffPeakKey, bottom: StaffPeakKey) -> BarAlignment {
        let alignment = BarAlignment::new(
            AlignmentPeak::new(PeakId::new(1), top.staff_id(), top.start(), 1.0).unwrap(),
            AlignmentPeak::new(PeakId::new(2), bottom.staff_id(), bottom.start(), 1.0).unwrap(),
            0.0,
            0.0,
            BarImpacts::alignment(1.0, 1.0).unwrap(),
        )
        .unwrap();
        BarAlignment::connection(&alignment, 1.0, 1.0).unwrap()
    }

    fn bars_system(blank_evidence: bool) -> BarsSystemState {
        let top = peak(1);
        let bottom = peak(2);
        let mut graph = PeakGraph::new();
        graph.add_vertex(top.clone());
        graph.add_vertex(bottom.clone());
        graph
            .add_edge(top.key(), bottom.key(), connection(top.key(), bottom.key()))
            .unwrap();
        let blanks = |peak: &StaffPeak| {
            if blank_evidence {
                BTreeMap::from([(peak.key(), false)])
            } else {
                BTreeMap::new()
            }
        };
        BarsSystemState::new(
            1,
            vec![
                BarsStaffState::new(StaffId::new(1), 0, true, vec![top.clone()], blanks(&top))
                    .unwrap(),
                BarsStaffState::new(
                    StaffId::new(2),
                    0,
                    false,
                    vec![bottom.clone()],
                    blanks(&bottom),
                )
                .unwrap(),
            ],
            graph,
        )
        .unwrap()
    }

    fn line_parameters() -> LinesCoordinatorParameters {
        LinesCoordinatorParameters::new(0.0, 1, None, 1.0, 0.0, 100.0).unwrap()
    }

    fn bar_parameters() -> BarsCoordinatorParameters {
        BarsCoordinatorParameters::new(2, 12, 6, 20, 2, 10, 0.2, None).unwrap()
    }

    #[test]
    fn grid_runs_lines_before_bars_and_commits_both() {
        let (primary, secondary) = line_states();
        let mut state =
            HeadlessGridState::new(primary, Some(secondary), vec![bars_system(true)]).unwrap();

        let result =
            process_headless_grid(&mut state, line_parameters(), bar_parameters()).unwrap();

        assert_eq!(result.lines().staffs().len(), 2);
        assert_eq!(result.bars().len(), 1);
        assert_eq!(result.bars()[0].start_column_index(), Some(0));
        assert!(result.discarded_one_line_staffs().is_empty());
        assert_eq!(state.primary_lines().clusters().len(), 1);
        assert_eq!(state.secondary_lines().unwrap().clusters().len(), 1);
        assert_eq!(state.bar_systems()[0].columns().len(), 1);
    }

    #[test]
    fn downstream_bar_failure_rolls_back_completed_line_work() {
        let (primary, secondary) = line_states();
        let mut state =
            HeadlessGridState::new(primary, Some(secondary), vec![bars_system(false)]).unwrap();

        let error =
            process_headless_grid(&mut state, line_parameters(), bar_parameters()).unwrap_err();

        assert!(matches!(
            error,
            GridCoordinatorError::Bars(BarsCoordinatorError::MissingBlankEvidence(_))
        ));
        assert!(state.primary_lines().clusters().is_empty());
        assert!(state.secondary_lines().unwrap().clusters().is_empty());
        assert!(state.bar_systems()[0].columns().is_empty());
    }
}
