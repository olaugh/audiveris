// SPDX-License-Identifier: AGPL-3.0-or-later

//! Adapter from a live horizontal section lag to the primary clustering pass.

use std::{collections::BTreeMap, error::Error, fmt};

use crate::{
    cluster_coordinator::{FollowCombsNetworkError, RecursiveCombSnapshot, follow_combs_network},
    cluster_ownership::{ClusterOwnership, ClusterOwnershipError, CombId},
    cluster_pipeline::ClusterRetrievalParameters,
    comb_builder::{CombBuilderError, CombFilament, popular_comb_size, retrieve_combs},
    filament::FilamentError,
    filament_factory::{
        FilamentFactory, FilamentFactoryIdentityError, FilamentFactoryParams, OverlapParams,
    },
    line_cluster::FilamentId,
    line_rejection::{
        LineCandidate, LinePoint, LineRejectionError, LineRejectionParameters,
        reject_line_candidates, reject_line_candidates_projective,
    },
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
    /// Opt-in local staff-line slope model for projective page captures.
    pub projective_slope: bool,
}

/// Sampling and cluster parameters for Java's optional small-interline pass.
///
/// The filaments themselves remain the objects created by the main-interline
/// factory. Only comb sampling and cluster interpretation use these values.
#[derive(Clone, Debug)]
pub struct RawSecondaryPassParameters {
    pub sampling_dx: usize,
    pub minimum_delta_y: isize,
    pub maximum_delta_y: isize,
    pub retrieval: ClusterRetrievalParameters,
}

/// Constructed pass plus the observable post-network root order.
#[derive(Clone, Debug)]
pub struct RawPrimaryPassBuild {
    state: ClusterPassState,
    survivor_order: Vec<FilamentId>,
    root_order: Vec<FilamentId>,
    popular_comb_size: Option<usize>,
    global_slope: f64,
    curved_ids: Vec<FilamentId>,
    sloped_ids: Vec<FilamentId>,
    sloped_filaments: BTreeMap<FilamentId, crate::filament::StaffFilament>,
    factory_creation_ids: Vec<FilamentId>,
    next_filament_id: FilamentId,
}

impl RawPrimaryPassBuild {
    #[must_use]
    pub const fn state(&self) -> &ClusterPassState {
        &self.state
    }

    /// Rejection survivors in Java's stable reverse-length order, before the
    /// comb network is allowed to merge roots.
    #[must_use]
    pub fn survivor_order(&self) -> &[FilamentId] {
        &self.survivor_order
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
    pub const fn global_slope(&self) -> f64 {
        self.global_slope
    }

    /// Java `FilamentIndex` IDs rejected for curvature, in pre-sort order.
    #[must_use]
    pub fn curved_ids(&self) -> &[FilamentId] {
        &self.curved_ids
    }

    /// Java `FilamentIndex` IDs rejected for slope, in post-length-sort order.
    #[must_use]
    pub fn sloped_ids(&self) -> &[FilamentId] {
        &self.sloped_ids
    }

    /// Slope rejects retained for Java's later discarded-filament fallback.
    /// Curvature rejects are intentionally unavailable here.
    #[must_use]
    pub const fn sloped_filaments(&self) -> &BTreeMap<FilamentId, crate::filament::StaffFilament> {
        &self.sloped_filaments
    }

    /// Every ID registered by the factory, including swallowed cores and
    /// temporary expansion candidates.
    #[must_use]
    pub fn factory_creation_ids(&self) -> &[FilamentId] {
        &self.factory_creation_ids
    }

    /// Next ID for another factory sharing the same Java `FilamentIndex`.
    #[must_use]
    pub const fn next_filament_id(&self) -> FilamentId {
        self.next_filament_id
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
    build_primary_cluster_pass_with_first_filament_id(lag, parameters, FilamentId::new(1))
}

/// Identity-aware entry point for a sheet whose Java `FilamentIndex` may
/// already contain entities from an earlier factory.
pub fn build_primary_cluster_pass_with_first_filament_id(
    lag: &HorizontalSectionLag,
    parameters: RawPrimaryPassParameters,
    first_filament_id: FilamentId,
) -> Result<RawPrimaryPassBuild, RawLineAdapterError> {
    let factory = FilamentFactory::new(parameters.factory);
    let identified = factory.retrieve_filaments_with_ids(
        lag.sections(),
        parameters.overlap,
        first_filament_id.value(),
    )?;
    let factory_creation_ids = identified
        .creation_ids()
        .iter()
        .copied()
        .map(FilamentId::new)
        .collect::<Vec<_>>();
    let next_filament_id = FilamentId::new(identified.next_creation_id());
    let values = identified
        .into_survivors()
        .into_iter()
        .map(|entry| (entry.id(), entry.into_filament()))
        .collect();
    let rejection_parameters =
        LineRejectionParameters::java_defaults(parameters.factory.interline as f64);
    let FilteredRawFilaments {
        mut ownership,
        mut filaments,
        mut filament_order,
        global_slope,
        curved_ids,
        sloped_ids,
        sloped_filaments,
    } = filter_raw_filaments(values, rejection_parameters, parameters.projective_slope)?;

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
    let survivor_order = filament_order.clone();

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
        survivor_order,
        root_order,
        popular_comb_size,
        global_slope,
        curved_ids,
        sloped_ids,
        sloped_filaments,
        factory_creation_ids,
        next_filament_id,
    })
}

/// Lazily construct Java's secondary/small-interline clustering state.
///
/// `primary_discarded` comes from a transactional preview of the primary
/// cluster pass. Java does not feed slope rejects into this pass; it retains
/// them separately for `includeDiscardedFilaments`. Every value is cloned from
/// the original main-interline [`StaffFilament`], preserving its ID, sections
/// and geometry; no filament is rebuilt with the small interline.
pub fn build_secondary_cluster_pass(
    picture_width: usize,
    primary: &ClusterPassState,
    primary_discarded: &[FilamentId],
    parameters: RawSecondaryPassParameters,
) -> Result<ClusterPassState, RawLineAdapterError> {
    let mut filaments = BTreeMap::new();
    for &id in primary_discarded {
        let filament = primary
            .filaments()
            .get(&id)
            .ok_or(RawLineAdapterError::MissingFilament(id))?;
        if filaments.insert(id, filament.clone()).is_some() {
            return Err(RawLineAdapterError::DuplicateSecondaryFilament(id));
        }
    }
    // Java sorts the second-pass input by entity ID before constructing its
    // comb network. BTreeMap iteration gives that exact deterministic order.
    let mut filament_order = filaments.keys().copied().collect::<Vec<_>>();
    let mut ownership = ClusterOwnership::new();
    for (&id, filament) in &filaments {
        ownership.register_filament(id, filament)?;
    }
    let samples = filament_order
        .iter()
        .map(|id| {
            let filament = filaments
                .get(id)
                .ok_or(RawLineAdapterError::MissingFilament(*id))?;
            let raw_id =
                usize::try_from(id.value()).map_err(|_| RawLineAdapterError::IdentityOverflow)?;
            Ok(CombFilament::new(raw_id, raw_id, filament.geometry()?)?)
        })
        .collect::<Result<Vec<_>, RawLineAdapterError>>()?;
    let columns = retrieve_combs(
        picture_width,
        parameters.sampling_dx,
        parameters.minimum_delta_y,
        parameters.maximum_delta_y,
        &samples,
    )?;
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

    Ok(ClusterPassState::new(
        ownership,
        filaments,
        combs,
        filament_order,
        parameters.retrieval,
    ))
}

#[derive(Clone, Debug)]
struct FilteredRawFilaments {
    ownership: ClusterOwnership,
    filaments: BTreeMap<FilamentId, crate::filament::StaffFilament>,
    filament_order: Vec<FilamentId>,
    global_slope: f64,
    curved_ids: Vec<FilamentId>,
    sloped_ids: Vec<FilamentId>,
    sloped_filaments: BTreeMap<FilamentId, crate::filament::StaffFilament>,
}

fn filter_raw_filaments(
    values: Vec<(u64, crate::filament::StaffFilament)>,
    parameters: LineRejectionParameters,
    projective_slope: bool,
) -> Result<FilteredRawFilaments, RawLineAdapterError> {
    let mut all = BTreeMap::new();
    let mut candidates = Vec::with_capacity(values.len());
    for (factory_id, filament) in values {
        let raw_id =
            usize::try_from(factory_id).map_err(|_| RawLineAdapterError::IdentityOverflow)?;
        let id = FilamentId::new(factory_id);
        let geometry = filament.geometry()?;
        let start = geometry.start();
        let stop = geometry.stop();
        let x_mid = (start.0 + stop.0) / 2.0;
        candidates.push(LineCandidate::new(
            raw_id,
            LinePoint::new(start.0, start.1),
            LinePoint::new(stop.0, stop.1),
            geometry.position_at(x_mid)?,
            filament.bounds()?.width,
        )?);
        all.insert(id, filament);
    }

    let report = if projective_slope {
        reject_line_candidates_projective(candidates, parameters)?
    } else {
        reject_line_candidates(candidates, parameters)?
    };
    let curved_ids = report
        .curved
        .iter()
        .map(|rejected| FilamentId::new(rejected.candidate.id() as u64))
        .collect::<Vec<_>>();
    let sloped_ids = report
        .sloped
        .iter()
        .map(|rejected| FilamentId::new(rejected.candidate.id() as u64))
        .collect::<Vec<_>>();
    let filament_order = report
        .survivors
        .iter()
        .map(|candidate| FilamentId::new(candidate.id() as u64))
        .collect::<Vec<_>>();

    let mut ownership = ClusterOwnership::new();
    let mut filaments = BTreeMap::new();
    for id in &filament_order {
        let filament = all
            .remove(id)
            .ok_or(RawLineAdapterError::MissingFilament(*id))?;
        ownership.register_filament(*id, &filament)?;
        filaments.insert(*id, filament);
    }
    let mut sloped_filaments = BTreeMap::new();
    for id in &sloped_ids {
        let filament = all
            .remove(id)
            .ok_or(RawLineAdapterError::MissingFilament(*id))?;
        sloped_filaments.insert(*id, filament);
    }
    // Curvature rejects deliberately remain unregistered and are dropped.

    Ok(FilteredRawFilaments {
        ownership,
        filaments,
        filament_order,
        global_slope: report.global_slope,
        curved_ids,
        sloped_ids,
        sloped_filaments,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub enum RawLineAdapterError {
    Filament(FilamentError),
    FactoryIdentity(FilamentFactoryIdentityError),
    Rejection(LineRejectionError),
    Comb(CombBuilderError),
    Ownership(ClusterOwnershipError),
    Follow(FollowCombsNetworkError),
    MissingFilament(FilamentId),
    DuplicateSecondaryFilament(FilamentId),
    IdentityOverflow,
}

impl From<FilamentError> for RawLineAdapterError {
    fn from(value: FilamentError) -> Self {
        Self::Filament(value)
    }
}

impl From<FilamentFactoryIdentityError> for RawLineAdapterError {
    fn from(value: FilamentFactoryIdentityError) -> Self {
        Self::FactoryIdentity(value)
    }
}

impl From<LineRejectionError> for RawLineAdapterError {
    fn from(value: LineRejectionError) -> Self {
        Self::Rejection(value)
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
            Self::FactoryIdentity(error) => {
                write!(formatter, "raw filament identity failed: {error}")
            }
            Self::Rejection(error) => write!(formatter, "raw filament rejection failed: {error}"),
            Self::Comb(error) => write!(formatter, "raw comb sampling failed: {error}"),
            Self::Ownership(error) => write!(formatter, "raw ownership failed: {error}"),
            Self::Follow(error) => write!(formatter, "raw comb network failed: {error}"),
            Self::MissingFilament(id) => write!(formatter, "missing raw filament {}", id.value()),
            Self::DuplicateSecondaryFilament(id) => {
                write!(formatter, "duplicate secondary raw filament {}", id.value())
            }
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
        filament::StaffFilament,
        lines_coordinator::{LinesCoordinatorParameters, retrieve_staff_candidates},
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections},
    };

    fn retrieval_parameters() -> ClusterRetrievalParameters {
        retrieval_parameters_for(10, BTreeSet::from([5]))
    }

    fn retrieval_parameters_for(
        interline: usize,
        desired_sizes: BTreeSet<usize>,
    ) -> ClusterRetrievalParameters {
        let expansion = ClusterExpansionParameters::new(0.0, 0, 1, 0, 0.1, 1, 10).unwrap();
        let compatibility = ClusterMergeParameters::new(0.0, 0, 0.1, 0, 1, 10).unwrap();
        let merge = ClusterMergePassParameters::new(compatibility, 0, 1).unwrap();
        ClusterRetrievalParameters::new(
            interline,
            desired_sizes,
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

    fn one_line_filament(interline: usize, y: usize, x: usize, length: usize) -> StaffFilament {
        let mut table = RunTable::new(Orientation::Horizontal, x + length + 1, y + 2).unwrap();
        table.add_run(y, Run::new(x, length)).unwrap();
        let section = build_sections(&table, JunctionPolicy::All).remove(0);
        let mut filament = StaffFilament::new(interline).unwrap();
        filament.add_section(section).unwrap();
        filament
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
            projective_slope: false,
        }
    }

    fn identity_parameters() -> RawPrimaryPassParameters {
        let mut parameters = parameters();
        parameters.factory = FilamentFactoryParams {
            interline: 2,
            min_core_section_length: 6,
            min_section_aspect: 3.0,
            max_coord_gap: 10.0,
            max_pos_gap: 2.0,
            max_pos_gap_for_slope: 0.5,
            max_gap_slope: 0.5,
            min_length_for_delta_slope: 100.0,
            max_delta_slope: 0.01,
        };
        parameters.overlap = OverlapParams {
            probe_width: 2,
            max_overlap_delta_pos: 2.0,
            max_thickness: 4.0,
            max_overlap_space: 1.0,
            max_expansion_space: 0.0,
            max_involving_length: 10.0,
            max_consistent_ratio: 1.7,
        };
        parameters.sampling_dx = 10;
        parameters.minimum_delta_y = 1;
        parameters.maximum_delta_y = 2;
        parameters
    }

    fn rejection_fixture_filaments() -> Vec<StaffFilament> {
        let mut runs = RunTable::new(Orientation::Horizontal, 130, 200).unwrap();
        // ID 1: short horizontal, accepted by the sloped-page tolerance.
        runs.add_run(10, Run::new(0, 30)).unwrap();
        // ID 2: long bowed candidate, removed before slope estimation.
        for (y, x) in [(40, 0), (50, 40), (50, 80), (40, 115)] {
            runs.add_run(y, Run::new(x, 5)).unwrap();
        }
        // ID 3: longest line establishes a positive global slope.
        for (y, x) in [(90, 0), (92, 40), (94, 80), (96, 115)] {
            runs.add_run(y, Run::new(x, 5)).unwrap();
        }
        // ID 4: ordinary-length positive slope outlier.
        for (y, x) in [(130, 0), (135, 40), (139, 75)] {
            runs.add_run(y, Run::new(x, 5)).unwrap();
        }
        // ID 5: shorter line consistent with the global slope.
        for (y, x) in [(170, 0), (172, 50), (175, 95)] {
            runs.add_run(y, Run::new(x, 5)).unwrap();
        }

        let sections = build_sections(&runs, JunctionPolicy::All);
        let mut filaments = (0..5)
            .map(|_| StaffFilament::new(10).unwrap())
            .collect::<Vec<_>>();
        for section in sections {
            let index = match section.bounds().y {
                0..=29 => 0,
                30..=69 => 1,
                70..=119 => 2,
                120..=159 => 3,
                _ => 4,
            };
            filaments[index].add_section(section).unwrap();
        }
        filaments
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
        assert_eq!(
            built.survivor_order(),
            [
                FilamentId::new(1),
                FilamentId::new(2),
                FilamentId::new(5),
                FilamentId::new(6),
                FilamentId::new(4),
                FilamentId::new(3),
            ]
        );
        assert_eq!(
            built.factory_creation_ids(),
            [
                FilamentId::new(1),
                FilamentId::new(2),
                FilamentId::new(3),
                FilamentId::new(4),
                FilamentId::new(5),
                FilamentId::new(6),
            ]
        );
        assert_eq!(built.next_filament_id(), FilamentId::new(7));
        assert_eq!(built.root_order().len(), 5);
        assert_eq!(built.popular_comb_size(), Some(5));
        assert_eq!(built.global_slope(), 0.0);
        assert!(built.curved_ids().is_empty());
        assert!(built.sloped_ids().is_empty());
        assert!(built.sloped_filaments().is_empty());
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

    #[test]
    fn secondary_pass_preserves_main_geometry_ids_and_sorted_discard_provenance() {
        let discarded_id = FilamentId::new(5);
        let other_discarded_id = FilamentId::new(2);
        let mut table = RunTable::new(Orientation::Horizontal, 41, 42).unwrap();
        table.add_run(20, Run::new(0, 40)).unwrap();
        table.add_run(40, Run::new(0, 40)).unwrap();
        let mut source_sections = build_sections(&table, JunctionPolicy::All);
        let discarded_section = source_sections.remove(0);
        let other_discarded_section = source_sections.remove(0);
        let mut discarded = StaffFilament::new(10).unwrap();
        discarded.add_section(discarded_section).unwrap();
        let mut other_discarded = StaffFilament::new(10).unwrap();
        other_discarded
            .add_section(other_discarded_section)
            .unwrap();
        let discarded_sections = discarded.sections().to_vec();
        let other_discarded_sections = other_discarded.sections().to_vec();

        let mut primary_ownership = ClusterOwnership::new();
        primary_ownership
            .register_filament(discarded_id, &discarded)
            .unwrap();
        primary_ownership
            .register_filament(other_discarded_id, &other_discarded)
            .unwrap();
        let primary = ClusterPassState::new(
            primary_ownership,
            BTreeMap::from([
                (discarded_id, discarded),
                (other_discarded_id, other_discarded),
            ]),
            BTreeMap::new(),
            vec![discarded_id],
            retrieval_parameters_for(10, BTreeSet::from([1])),
        );
        let secondary = build_secondary_cluster_pass(
            100,
            &primary,
            &[other_discarded_id, discarded_id],
            RawSecondaryPassParameters {
                sampling_dx: 20,
                minimum_delta_y: 4,
                maximum_delta_y: 6,
                retrieval: retrieval_parameters_for(5, BTreeSet::from([1])),
            },
        )
        .unwrap();

        assert_eq!(
            secondary.filament_order(),
            [other_discarded_id, discarded_id]
        );
        assert_eq!(secondary.parameters().interline(), 5);
        assert_eq!(secondary.filaments()[&discarded_id].interline(), 10);
        assert_eq!(secondary.filaments()[&other_discarded_id].interline(), 10);
        assert_eq!(
            secondary.filaments()[&discarded_id].sections(),
            discarded_sections
        );
        assert_eq!(
            secondary.filaments()[&other_discarded_id].sections(),
            other_discarded_sections
        );
        assert_eq!(
            secondary
                .ownership()
                .section_owner(secondary.filaments()[&discarded_id].sections()[0].id()),
            Some(discarded_id)
        );
    }

    #[test]
    fn duplicate_secondary_provenance_fails_without_mutating_sources() {
        let id = FilamentId::new(7);
        let filament = one_line_filament(10, 20, 0, 40);
        let mut ownership = ClusterOwnership::new();
        ownership.register_filament(id, &filament).unwrap();
        let primary = ClusterPassState::new(
            ownership,
            BTreeMap::from([(id, filament.clone())]),
            BTreeMap::new(),
            vec![id],
            retrieval_parameters_for(10, BTreeSet::from([1])),
        );
        let error = build_secondary_cluster_pass(
            100,
            &primary,
            &[id, id],
            RawSecondaryPassParameters {
                sampling_dx: 20,
                minimum_delta_y: 4,
                maximum_delta_y: 6,
                retrieval: retrieval_parameters_for(5, BTreeSet::from([1])),
            },
        )
        .unwrap_err();

        assert_eq!(error, RawLineAdapterError::DuplicateSecondaryFilament(id));
        assert_eq!(primary.filaments()[&id].interline(), filament.interline());
        assert_eq!(primary.filaments()[&id].sections(), filament.sections());
    }

    #[test]
    fn rejection_precedes_combs_and_preserves_ids_ownership_and_discard_classes() {
        let values = rejection_fixture_filaments();
        let survivor_sections = values
            .iter()
            .enumerate()
            .map(|(index, filament)| {
                (
                    FilamentId::new((index + 1) as u64),
                    filament
                        .sections()
                        .iter()
                        .map(|section| section.id())
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let identified = values
            .into_iter()
            .enumerate()
            .map(|(index, filament)| ((index + 1) as u64, filament))
            .collect();
        let filtered = filter_raw_filaments(
            identified,
            LineRejectionParameters::java_defaults(10.0),
            false,
        )
        .unwrap();

        assert!((filtered.global_slope - (6.0 / 119.0)).abs() < 1e-12);
        assert_eq!(
            filtered.filament_order,
            [FilamentId::new(3), FilamentId::new(5), FilamentId::new(1)]
        );
        assert_eq!(filtered.curved_ids, [FilamentId::new(2)]);
        assert_eq!(filtered.sloped_ids, [FilamentId::new(4)]);
        assert_eq!(
            filtered.filaments.keys().copied().collect::<Vec<_>>(),
            [FilamentId::new(1), FilamentId::new(3), FilamentId::new(5)]
        );
        assert_eq!(
            filtered
                .sloped_filaments
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            [FilamentId::new(4)]
        );

        for id in [FilamentId::new(1), FilamentId::new(3), FilamentId::new(5)] {
            for section in &survivor_sections[&id] {
                assert_eq!(filtered.ownership.section_owner(*section), Some(id));
            }
        }
        for id in [FilamentId::new(2), FilamentId::new(4)] {
            for section in &survivor_sections[&id] {
                assert_eq!(filtered.ownership.section_owner(*section), None);
            }
        }
        assert_eq!(
            filtered.sloped_filaments[&FilamentId::new(4)]
                .sections()
                .iter()
                .map(|section| section.id())
                .collect::<Vec<_>>(),
            survivor_sections[&FilamentId::new(4)]
        );
        assert!(!filtered.sloped_filaments.contains_key(&FilamentId::new(2)));
    }

    #[test]
    fn adapter_uses_factory_creation_ids_across_swallow_and_expansion() {
        let mut runs = RunTable::new(Orientation::Horizontal, 320, 8).unwrap();
        for (x, length) in [(0, 80), (91, 80), (220, 80)] {
            runs.add_run(2, Run::new(x, length)).unwrap();
        }
        for (x, length) in [(80, 5), (86, 5)] {
            runs.add_run(3, Run::new(x, length)).unwrap();
        }
        let lag = HorizontalSectionLag::from_long_runs(runs).unwrap();
        let built = build_primary_cluster_pass_with_first_filament_id(
            &lag,
            identity_parameters(),
            FilamentId::new(41),
        )
        .unwrap();

        assert_eq!(
            built.factory_creation_ids(),
            [
                FilamentId::new(41),
                FilamentId::new(42),
                FilamentId::new(43),
                FilamentId::new(44),
                FilamentId::new(45),
            ]
        );
        assert_eq!(built.next_filament_id(), FilamentId::new(46));
        assert_eq!(
            built.survivor_order(),
            [FilamentId::new(41), FilamentId::new(43)]
        );
        assert_eq!(
            built
                .state()
                .filaments()
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            [FilamentId::new(41), FilamentId::new(43)]
        );
        assert!(!built.state().filaments().contains_key(&FilamentId::new(42)));
        for section in lag.sections() {
            let expected = if section.bounds().x < 200 {
                FilamentId::new(41)
            } else {
                FilamentId::new(43)
            };
            assert_eq!(
                built.state().ownership().section_owner(section.id()),
                Some(expected)
            );
        }
    }
}
