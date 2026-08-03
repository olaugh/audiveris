// SPDX-License-Identifier: AGPL-3.0-or-later

//! Adapter from a live horizontal section lag to the primary clustering pass.

use std::{collections::BTreeMap, error::Error, fmt};

use crate::{
    cluster_coordinator::{FollowCombsNetworkError, RecursiveCombSnapshot, follow_combs_network},
    cluster_ownership::{ClusterOwnership, ClusterOwnershipError, CombId},
    cluster_pipeline::ClusterRetrievalParameters,
    comb_builder::{CombBuilderError, CombFilament, popular_comb_size, retrieve_combs},
    filament::FilamentError,
    filament_factory::{FilamentFactory, FilamentFactoryParams, OverlapParams},
    line_cluster::FilamentId,
    line_short_sections::HorizontalSectionLag,
    lines_coordinator::ClusterPassState,
};

/// Pixel-resolved inputs needed before Java's primary `ClustersRetriever` pass.
#[derive(Clone, Debug)]
pub struct RawPrimaryPassParameters {
    pub factory: FilamentFactoryParams,
    pub overlap: OverlapParams,
    pub sampling_dx: usize,
    pub minimum_delta_y: isize,
    pub maximum_delta_y: isize,
    pub retrieval: ClusterRetrievalParameters,
}

/// Constructed pass plus the observable post-network root order.
#[derive(Clone, Debug)]
pub struct RawPrimaryPassBuild {
    state: ClusterPassState,
    root_order: Vec<FilamentId>,
    popular_comb_size: Option<usize>,
}

impl RawPrimaryPassBuild {
    #[must_use]
    pub const fn state(&self) -> &ClusterPassState {
        &self.state
    }

    #[must_use]
    pub fn root_order(&self) -> &[FilamentId] {
        &self.root_order
    }

    #[must_use]
    pub const fn popular_comb_size(&self) -> Option<usize> {
        self.popular_comb_size
    }

    #[must_use]
    pub fn into_state(self) -> ClusterPassState {
        self.state
    }
}

/// Build Java's primary long-line state without mutating the source lag.
pub fn build_primary_cluster_pass(
    lag: &HorizontalSectionLag,
    parameters: RawPrimaryPassParameters,
) -> Result<RawPrimaryPassBuild, RawLineAdapterError> {
    let factory = FilamentFactory::new(parameters.factory);
    let values = factory.retrieve_filaments(lag.sections(), parameters.overlap)?;

    let mut ownership = ClusterOwnership::new();
    let mut filaments = BTreeMap::new();
    let mut filament_order = Vec::with_capacity(values.len());
    for (index, filament) in values.into_iter().enumerate() {
        let id = FilamentId::new(
            u64::try_from(index + 1).map_err(|_| RawLineAdapterError::IdentityOverflow)?,
        );
        ownership.register_filament(id, &filament)?;
        filament_order.push(id);
        filaments.insert(id, filament);
    }

    let samples = filament_order
        .iter()
        .map(|id| {
            let filament = filaments
                .get(id)
                .ok_or(RawLineAdapterError::MissingFilament(*id))?;
            Ok(CombFilament::new(
                usize::try_from(id.value()).map_err(|_| RawLineAdapterError::IdentityOverflow)?,
                usize::try_from(id.value()).map_err(|_| RawLineAdapterError::IdentityOverflow)?,
                filament.geometry()?,
            )?)
        })
        .collect::<Result<Vec<_>, RawLineAdapterError>>()?;
    let columns = retrieve_combs(
        lag.run_table().width(),
        parameters.sampling_dx,
        parameters.minimum_delta_y,
        parameters.maximum_delta_y,
        &samples,
    )?;
    let popular_comb_size = popular_comb_size(&columns);

    let mut combs = BTreeMap::new();
    let mut next_comb_id = 1_u64;
    for comb in columns.iter().flat_map(|column| column.combs()) {
        let id = CombId::new(next_comb_id);
        next_comb_id = next_comb_id
            .checked_add(1)
            .ok_or(RawLineAdapterError::IdentityOverflow)?;
        ownership.register_comb(id, comb)?;
        combs.insert(id, RecursiveCombSnapshot::from_comb(comb));
    }

    follow_combs_network(&mut ownership, &mut filaments, &combs, &mut filament_order)?;
    let root_order = filament_order.clone();
    let state = ClusterPassState::new(
        ownership,
        filaments,
        combs,
        filament_order,
        parameters.retrieval,
    );
    Ok(RawPrimaryPassBuild {
        state,
        root_order,
        popular_comb_size,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub enum RawLineAdapterError {
    Filament(FilamentError),
    Comb(CombBuilderError),
    Ownership(ClusterOwnershipError),
    Follow(FollowCombsNetworkError),
    MissingFilament(FilamentId),
    IdentityOverflow,
}

impl From<FilamentError> for RawLineAdapterError {
    fn from(value: FilamentError) -> Self {
        Self::Filament(value)
    }
}

impl From<CombBuilderError> for RawLineAdapterError {
    fn from(value: CombBuilderError) -> Self {
        Self::Comb(value)
    }
}

impl From<ClusterOwnershipError> for RawLineAdapterError {
    fn from(value: ClusterOwnershipError) -> Self {
        Self::Ownership(value)
    }
}

impl From<FollowCombsNetworkError> for RawLineAdapterError {
    fn from(value: FollowCombsNetworkError) -> Self {
        Self::Follow(value)
    }
}

impl fmt::Display for RawLineAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Filament(error) => write!(formatter, "raw filament construction failed: {error}"),
            Self::Comb(error) => write!(formatter, "raw comb sampling failed: {error}"),
            Self::Ownership(error) => write!(formatter, "raw ownership failed: {error}"),
            Self::Follow(error) => write!(formatter, "raw comb network failed: {error}"),
            Self::MissingFilament(id) => write!(formatter, "missing raw filament {}", id.value()),
            Self::IdentityOverflow => formatter.write_str("raw adapter identity overflow"),
        }
    }
}

impl Error for RawLineAdapterError {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::{
        cluster_expand::ClusterExpansionParameters,
        cluster_merge::{ClusterMergeParameters, ClusterMergePassParameters},
        lines_coordinator::{LinesCoordinatorParameters, retrieve_staff_candidates},
        run_table::{Orientation, Run, RunTable},
    };

    fn retrieval_parameters() -> ClusterRetrievalParameters {
        let expansion = ClusterExpansionParameters::new(0.0, 0, 1, 0, 0.1, 1, 10).unwrap();
        let compatibility = ClusterMergeParameters::new(0.0, 0, 0.1, 0, 1, 10).unwrap();
        let merge = ClusterMergePassParameters::new(compatibility, 0, 1).unwrap();
        ClusterRetrievalParameters::new(
            10,
            BTreeSet::from([5]),
            expansion,
            merge,
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

    fn parameters() -> RawPrimaryPassParameters {
        RawPrimaryPassParameters {
            factory: FilamentFactoryParams {
                interline: 10,
                min_core_section_length: 1,
                min_section_aspect: 1.0,
                max_coord_gap: 5.0,
                max_pos_gap: 1.0,
                max_pos_gap_for_slope: 1.0,
                max_gap_slope: 0.1,
                min_length_for_delta_slope: 10.0,
                max_delta_slope: 0.1,
            },
            overlap: OverlapParams {
                probe_width: 2,
                max_overlap_delta_pos: 1.0,
                max_thickness: 2.0,
                max_overlap_space: 0.0,
                max_expansion_space: 0.0,
                max_involving_length: 10.0,
                max_consistent_ratio: 2.0,
            },
            sampling_dx: 20,
            minimum_delta_y: 9,
            maximum_delta_y: 11,
            retrieval: retrieval_parameters(),
        }
    }

    #[test]
    fn split_middle_raw_lag_forms_one_five_line_staff_and_preserves_sections() {
        let mut runs = RunTable::new(Orientation::Horizontal, 100, 61).unwrap();
        for y in [10, 20, 40, 50] {
            runs.add_run(y, Run::new(0, 100)).unwrap();
        }
        runs.add_run(30, Run::new(0, 21)).unwrap();
        runs.add_run(30, Run::new(40, 60)).unwrap();
        let lag = HorizontalSectionLag::from_long_runs(runs).unwrap();
        let middle_section_ids = lag
            .sections()
            .iter()
            .filter(|section| section.bounds().y == 30)
            .map(|section| section.id())
            .collect::<Vec<_>>();
        assert_eq!(middle_section_ids.len(), 2);

        let built = build_primary_cluster_pass(&lag, parameters()).unwrap();
        assert_eq!(built.root_order().len(), 5);
        assert_eq!(built.popular_comb_size(), Some(5));
        let first_owner = built
            .state()
            .ownership()
            .section_owner(middle_section_ids[0]);
        let second_owner = built
            .state()
            .ownership()
            .section_owner(middle_section_ids[1]);
        assert_eq!(first_owner, second_owner);
        let owner = first_owner.unwrap();
        assert_eq!(built.state().filaments()[&owner].sections().len(), 2);

        let mut state = built.into_state();
        let result = retrieve_staff_candidates(
            &mut state,
            None,
            LinesCoordinatorParameters::new(0.0, 1, None, 1.0, 0.0, 100.0).unwrap(),
        )
        .unwrap();
        assert_eq!(result.staffs().len(), 1);
        assert_eq!(result.staffs()[0].line_ids().len(), 5);
    }

    #[test]
    fn adapter_failure_does_not_mutate_live_lag() {
        let mut runs = RunTable::new(Orientation::Horizontal, 20, 12).unwrap();
        runs.add_run(10, Run::new(0, 20)).unwrap();
        let lag = HorizontalSectionLag::from_long_runs(runs).unwrap();
        let before = lag.clone();
        let mut invalid = parameters();
        invalid.sampling_dx = 0;

        assert!(build_primary_cluster_pass(&lag, invalid).is_err());
        assert_eq!(lag, before);
    }
}
