// SPDX-License-Identifier: AGPL-3.0-or-later

//! Executable, sheet-neutral orchestration of Java `retrieveClusters`.

use std::{
    cmp::{Ordering, Reverse},
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use crate::{
    cluster_coordinator::{RecursiveCombSnapshot, RecursiveIncludeError, form_clusters_from_combs},
    cluster_expand::{ClusterExpansionError, ClusterExpansionParameters, expand_clusters_in_order},
    cluster_finalize::{
        ClusterConsistencyParameters, ClusterFinalizeError, compute_acceptable_cluster_length,
        destroy_inconsistent_clusters_in_order, destroy_non_desired_clusters_in_order,
        partition_non_clustered_filaments_in_order, trim_clusters_in_ordinate_order,
    },
    cluster_merge::{
        ClusterMergeError, ClusterMergePassParameters, ClusterPairPassParameters,
        cluster_deskewed_ordinate, merge_cluster_pairs_in_order, merge_clusters_in_order,
    },
    cluster_ownership::{ClusterId, ClusterOwnership, ClusterOwnershipError, CombId},
    filament::{FilamentError, StaffFilament},
    line_cluster::{FilamentId, LineCluster, LineClusterError},
    section::Bounds,
};

/// Sheet constants resolved into the values consumed by `retrieveClusters`.
#[derive(Clone, Debug, PartialEq)]
pub struct ClusterRetrievalParameters {
    interline: usize,
    desired_sizes: BTreeSet<usize>,
    expansion: ClusterExpansionParameters,
    merge: ClusterMergePassParameters,
    global_slope: f64,
    minimum_tablature_length_ratio: f64,
    minimum_cluster_length_ratio: f64,
    maximum_pair_center_distance: i64,
    maximum_pair_horizontal_gap: i64,
    consistency: Option<ClusterConsistencyParameters>,
    one_line_vertical_margin: i64,
}

impl ClusterRetrievalParameters {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        interline: usize,
        desired_sizes: BTreeSet<usize>,
        expansion: ClusterExpansionParameters,
        merge: ClusterMergePassParameters,
        global_slope: f64,
        minimum_tablature_length_ratio: f64,
        minimum_cluster_length_ratio: f64,
        maximum_pair_center_distance: i64,
        maximum_pair_horizontal_gap: i64,
        consistency: Option<ClusterConsistencyParameters>,
        one_line_vertical_margin: i64,
    ) -> Result<Self, ClusterPipelineError> {
        if interline == 0
            || desired_sizes.is_empty()
            || !global_slope.is_finite()
            || !minimum_tablature_length_ratio.is_finite()
            || minimum_tablature_length_ratio < 0.0
            || !minimum_cluster_length_ratio.is_finite()
            || minimum_cluster_length_ratio < 0.0
            || maximum_pair_center_distance < 0
            || maximum_pair_horizontal_gap < 0
            || one_line_vertical_margin < 0
        {
            return Err(ClusterPipelineError::InvalidParameters);
        }
        Ok(Self {
            interline,
            desired_sizes,
            expansion,
            merge,
            global_slope,
            minimum_tablature_length_ratio,
            minimum_cluster_length_ratio,
            maximum_pair_center_distance,
            maximum_pair_horizontal_gap,
            consistency,
            one_line_vertical_margin,
        })
    }

    #[must_use]
    pub const fn interline(&self) -> usize {
        self.interline
    }

    /// Override Java's page-relative minimum cluster length ratio.
    ///
    /// This is intentionally explicit: retaining short five-line clusters is
    /// useful for abbreviated final systems, but production parity remains at
    /// the constructor's 0.2 default unless an opt-in caller changes it.
    pub fn with_minimum_cluster_length_ratio(
        mut self,
        ratio: f64,
    ) -> Result<Self, ClusterPipelineError> {
        if !ratio.is_finite() || ratio < 0.0 {
            return Err(ClusterPipelineError::InvalidParameters);
        }
        self.minimum_cluster_length_ratio = ratio;
        Ok(self)
    }

    #[must_use]
    pub const fn minimum_cluster_length_ratio(&self) -> f64 {
        self.minimum_cluster_length_ratio
    }
}

/// Stable observable partitions produced by the complete neutral pipeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterRetrievalResult {
    cluster_ids: Vec<ClusterId>,
    one_line_cluster_ids: Vec<ClusterId>,
    active_filaments: Vec<FilamentId>,
    discarded_filaments: Vec<FilamentId>,
    merged_filaments: Vec<FilamentId>,
    discarded_clusters: Vec<ClusterId>,
    removed_by_trim: Vec<FilamentId>,
    popular_comb_size: Option<usize>,
}

impl ClusterRetrievalResult {
    #[must_use]
    pub fn cluster_ids(&self) -> &[ClusterId] {
        &self.cluster_ids
    }

    #[must_use]
    pub fn one_line_cluster_ids(&self) -> &[ClusterId] {
        &self.one_line_cluster_ids
    }

    #[must_use]
    pub fn active_filaments(&self) -> &[FilamentId] {
        &self.active_filaments
    }

    #[must_use]
    pub fn discarded_filaments(&self) -> &[FilamentId] {
        &self.discarded_filaments
    }

    #[must_use]
    pub fn merged_filaments(&self) -> &[FilamentId] {
        &self.merged_filaments
    }

    #[must_use]
    pub fn discarded_clusters(&self) -> &[ClusterId] {
        &self.discarded_clusters
    }

    #[must_use]
    pub fn removed_by_trim(&self) -> &[FilamentId] {
        &self.removed_by_trim
    }

    #[must_use]
    pub const fn popular_comb_size(&self) -> Option<usize> {
        self.popular_comb_size
    }
}

/// Run Java's cluster-retrieval stage order against explicit neutral state.
///
/// The incoming filament list is Java's mutable `allFilaments` order. The
/// complete pipeline is transactional: state is committed only if every stage,
/// including optional one-line recovery, succeeds.
pub fn retrieve_clusters(
    ownership: &mut ClusterOwnership,
    clusters: &mut BTreeMap<ClusterId, LineCluster>,
    filaments: &BTreeMap<FilamentId, StaffFilament>,
    combs: &mut BTreeMap<CombId, RecursiveCombSnapshot>,
    filament_order: &[FilamentId],
    parameters: &ClusterRetrievalParameters,
) -> Result<ClusterRetrievalResult, ClusterPipelineError> {
    let mut next_ownership = ownership.clone();
    let mut next_clusters = clusters.clone();
    let mut next_combs = combs.clone();
    let mut working_filaments = reverse_length_order(filaments, filament_order)?;
    let all_filaments = working_filaments.clone();
    let seeded_cluster_order = next_clusters.keys().copied().collect::<Vec<_>>();
    let formation = form_clusters_from_combs(
        &mut next_ownership,
        &mut next_clusters,
        filaments,
        &mut next_combs,
        &working_filaments,
        parameters.interline,
        false,
    )?;
    let mut cluster_order = Vec::new();
    let mut seen_clusters = BTreeSet::new();
    for id in seeded_cluster_order
        .into_iter()
        .chain(formation.cluster_ids().iter().copied())
    {
        let root = next_ownership.cluster_ancestor(id)?;
        if next_clusters.contains_key(&root) && seen_clusters.insert(root) {
            cluster_order.push(root);
        }
    }
    let popular_comb_size = formation.popular_comb_size();

    cluster_order = expand_clusters_in_order(
        &mut next_ownership,
        &mut next_clusters,
        &cluster_order,
        filaments,
        &working_filaments,
        parameters.expansion,
    )?;
    cluster_order = merge_clusters_in_order(
        &mut next_ownership,
        &mut next_clusters,
        &cluster_order,
        parameters.merge,
    )?;
    retain_roots(&next_ownership, &mut working_filaments)?;

    let trim = trim_clusters_in_ordinate_order(
        &mut next_ownership,
        &mut next_clusters,
        &cluster_order,
        parameters.global_slope,
        &parameters.desired_sizes,
        parameters.minimum_tablature_length_ratio,
    )?;
    cluster_order = trim.cluster_order().to_vec();
    let removed_by_trim = trim.removed_filaments().to_vec();
    let desired = destroy_non_desired_clusters_in_order(
        &mut next_ownership,
        &mut next_clusters,
        &cluster_order,
        &parameters.desired_sizes,
    )?;
    let (mut cluster_order, mut discarded_clusters) = desired.into_parts();

    if !cluster_order.is_empty() {
        let acceptable = compute_acceptable_cluster_length(
            &next_clusters,
            &cluster_order,
            parameters.minimum_cluster_length_ratio,
        )?;
        let pair_parameters = ClusterPairPassParameters::new(
            parameters.global_slope,
            parameters.maximum_pair_center_distance,
            parameters.maximum_pair_horizontal_gap,
            acceptable.minimum(),
        )?;
        let pairs = merge_cluster_pairs_in_order(
            &mut next_ownership,
            &mut next_clusters,
            &cluster_order,
            pair_parameters,
        )?;
        let (survivors, discarded) = pairs.into_parts();
        cluster_order = survivors;
        discarded_clusters.extend(discarded);
        retain_roots(&next_ownership, &mut working_filaments)?;
    }

    if let Some(consistency) = parameters.consistency {
        let result = destroy_inconsistent_clusters_in_order(
            &mut next_ownership,
            &mut next_clusters,
            &cluster_order,
            consistency,
        )?;
        let (survivors, discarded) = result.into_parts();
        cluster_order = survivors;
        discarded_clusters.extend(discarded);
    }

    cluster_order = expand_clusters_in_order(
        &mut next_ownership,
        &mut next_clusters,
        &cluster_order,
        filaments,
        &working_filaments,
        parameters.expansion,
    )?;

    let first_partition =
        partition_non_clustered_filaments_in_order(&next_ownership, &working_filaments)?;
    let mut discarded = first_partition.discarded().to_vec();
    let standard_clusters = cluster_order.clone();
    let mut one_line_cluster_ids = Vec::new();
    if parameters.desired_sizes.contains(&1) && !discarded.is_empty() {
        discarded = reverse_length_order(filaments, &discarded)?;
        let singles = form_clusters_from_combs(
            &mut next_ownership,
            &mut next_clusters,
            filaments,
            &mut next_combs,
            &discarded,
            parameters.interline,
            true,
        )?;
        one_line_cluster_ids = singles.cluster_ids().to_vec();
        one_line_cluster_ids = expand_clusters_in_order(
            &mut next_ownership,
            &mut next_clusters,
            &one_line_cluster_ids,
            filaments,
            &discarded,
            parameters.expansion,
        )?;
        one_line_cluster_ids = merge_clusters_in_order(
            &mut next_ownership,
            &mut next_clusters,
            &one_line_cluster_ids,
            parameters.merge,
        )?;
        retain_roots(&next_ownership, &mut discarded)?;
        let rejected = reject_singles_near_standard_clusters(
            &mut next_ownership,
            &mut next_clusters,
            &one_line_cluster_ids,
            &standard_clusters,
            parameters.global_slope,
            parameters.one_line_vertical_margin,
        )?;
        discarded_clusters.extend(rejected.iter().copied());
        one_line_cluster_ids.retain(|id| !rejected.contains(id));
        cluster_order.extend(one_line_cluster_ids.iter().copied());
        sort_clusters_by_ordinate(&next_clusters, &mut cluster_order, parameters.global_slope)?;
    }

    let final_partition =
        partition_non_clustered_filaments_in_order(&next_ownership, &all_filaments)?;
    let merged_filaments = final_partition.merged().to_vec();
    let active_filaments = final_partition
        .remaining()
        .iter()
        .copied()
        .filter(|id| !merged_filaments.contains(id))
        .collect();

    *ownership = next_ownership;
    *clusters = next_clusters;
    *combs = next_combs;
    Ok(ClusterRetrievalResult {
        cluster_ids: cluster_order,
        one_line_cluster_ids,
        active_filaments,
        discarded_filaments: final_partition.discarded().to_vec(),
        merged_filaments,
        discarded_clusters,
        removed_by_trim,
        popular_comb_size,
    })
}

fn reverse_length_order(
    filaments: &BTreeMap<FilamentId, StaffFilament>,
    order: &[FilamentId],
) -> Result<Vec<FilamentId>, ClusterPipelineError> {
    let mut seen = BTreeSet::new();
    let mut ordered = order
        .iter()
        .copied()
        .map(|id| {
            if !seen.insert(id) {
                return Err(ClusterPipelineError::DuplicateFilamentOrder(id));
            }
            let width = filaments
                .get(&id)
                .ok_or(ClusterPipelineError::MissingFilamentValue(id))?
                .bounds()?
                .width;
            Ok((id, width))
        })
        .collect::<Result<Vec<_>, ClusterPipelineError>>()?;
    ordered.sort_by_key(|&(_, width)| Reverse(width));
    Ok(ordered.into_iter().map(|(id, _)| id).collect())
}

fn retain_roots(
    ownership: &ClusterOwnership,
    filaments: &mut Vec<FilamentId>,
) -> Result<(), ClusterPipelineError> {
    let mut roots = Vec::with_capacity(filaments.len());
    for &id in filaments.iter() {
        if ownership.filament_parent(id)?.is_none() {
            roots.push(id);
        }
    }
    *filaments = roots;
    Ok(())
}

fn reject_singles_near_standard_clusters(
    ownership: &mut ClusterOwnership,
    clusters: &mut BTreeMap<ClusterId, LineCluster>,
    singles: &[ClusterId],
    standards: &[ClusterId],
    global_slope: f64,
    vertical_margin: i64,
) -> Result<Vec<ClusterId>, ClusterPipelineError> {
    let mut standards = standards.to_vec();
    sort_clusters_by_ordinate(clusters, &mut standards, global_slope)?;
    let mut rejected = Vec::new();
    for &single_id in singles {
        let single = clusters
            .get(&single_id)
            .ok_or(ClusterPipelineError::MissingClusterValue(single_id))?;
        let bounds = single.bounds()?;
        let center_x = (bounds.x + (bounds.width / 2)) as f64;
        let center_y = (bounds.y + (bounds.height / 2)) as f64;
        let single_y = deskew_ordinate(center_x, center_y, global_slope);
        for &standard_id in &standards {
            let standard = &clusters[&standard_id];
            if !intersects_with_vertical_margin(bounds, standard.bounds()?, vertical_margin) {
                continue;
            }
            let first_y = standard
                .first_line()
                .filament()
                .geometry()?
                .position_at(center_x)?
                - vertical_margin as f64;
            if single_y < deskew_ordinate(center_x, first_y, global_slope) {
                break;
            }
            let last_y = standard
                .last_line()
                .filament()
                .geometry()?
                .position_at(center_x)?
                + vertical_margin as f64;
            if single_y > deskew_ordinate(center_x, last_y, global_slope) {
                continue;
            }
            ownership.destroy_cluster(single_id)?;
            clusters.remove(&single_id);
            rejected.push(single_id);
            break;
        }
    }
    Ok(rejected)
}

fn sort_clusters_by_ordinate(
    clusters: &BTreeMap<ClusterId, LineCluster>,
    order: &mut [ClusterId],
    global_slope: f64,
) -> Result<(), ClusterPipelineError> {
    let mut ordinates = BTreeMap::new();
    for &id in order.iter() {
        let cluster = clusters
            .get(&id)
            .ok_or(ClusterPipelineError::MissingClusterValue(id))?;
        ordinates.insert(id, cluster_deskewed_ordinate(cluster, global_slope)?);
    }
    order.sort_by(|one, two| {
        ordinates[one]
            .partial_cmp(&ordinates[two])
            .unwrap_or(Ordering::Equal)
    });
    Ok(())
}

fn intersects_with_vertical_margin(one: Bounds, two: Bounds, margin: i64) -> bool {
    let one_left = one.x as i128;
    let one_right = (one.x + one.width) as i128;
    let one_top = one.y as i128 - margin as i128;
    let one_bottom = (one.y + one.height) as i128 + margin as i128;
    let two_left = two.x as i128;
    let two_right = (two.x + two.width) as i128;
    let two_top = two.y as i128;
    let two_bottom = (two.y + two.height) as i128;
    one_left < two_right && two_left < one_right && one_top < two_bottom && two_top < one_bottom
}

fn deskew_ordinate(x: f64, y: f64, slope: f64) -> f64 {
    let angle = -slope.atan();
    (x * angle.sin()) + (y * angle.cos())
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClusterPipelineError {
    InvalidParameters,
    MissingFilamentValue(FilamentId),
    MissingClusterValue(ClusterId),
    DuplicateFilamentOrder(FilamentId),
    Coordinator(RecursiveIncludeError),
    Expansion(ClusterExpansionError),
    Merge(ClusterMergeError),
    Finalize(ClusterFinalizeError),
    Ownership(ClusterOwnershipError),
    Cluster(LineClusterError),
    Filament(FilamentError),
}

macro_rules! pipeline_from {
    ($source:ty, $variant:ident) => {
        impl From<$source> for ClusterPipelineError {
            fn from(value: $source) -> Self {
                Self::$variant(value)
            }
        }
    };
}

pipeline_from!(RecursiveIncludeError, Coordinator);
pipeline_from!(ClusterExpansionError, Expansion);
pipeline_from!(ClusterMergeError, Merge);
pipeline_from!(ClusterFinalizeError, Finalize);
pipeline_from!(ClusterOwnershipError, Ownership);
pipeline_from!(LineClusterError, Cluster);
pipeline_from!(FilamentError, Filament);

impl fmt::Display for ClusterPipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParameters => {
                formatter.write_str("cluster retrieval parameters are invalid")
            }
            Self::MissingFilamentValue(id) => {
                write!(formatter, "missing filament value {}", id.value())
            }
            Self::MissingClusterValue(id) => {
                write!(formatter, "missing cluster value {}", id.value())
            }
            Self::DuplicateFilamentOrder(id) => write!(
                formatter,
                "duplicate filament {} in retrieval order",
                id.value()
            ),
            Self::Coordinator(error) => write!(formatter, "cluster formation failed: {error}"),
            Self::Expansion(error) => write!(formatter, "cluster expansion failed: {error}"),
            Self::Merge(error) => write!(formatter, "cluster merge failed: {error}"),
            Self::Finalize(error) => write!(formatter, "cluster finalization failed: {error}"),
            Self::Ownership(error) => write!(formatter, "cluster ownership failed: {error}"),
            Self::Cluster(error) => write!(formatter, "cluster geometry failed: {error}"),
            Self::Filament(error) => write!(formatter, "filament geometry failed: {error}"),
        }
    }
}

impl Error for ClusterPipelineError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cluster_merge::ClusterMergeParameters,
        filament_comb::FilamentComb,
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections},
    };

    fn fixture() -> (ClusterOwnership, BTreeMap<FilamentId, StaffFilament>) {
        fixture_with_single_at(100)
    }

    fn fixture_with_single_at(
        single_y: usize,
    ) -> (ClusterOwnership, BTreeMap<FilamentId, StaffFilament>) {
        let mut table = RunTable::new(Orientation::Horizontal, 40, single_y + 2).unwrap();
        for y in [10, 20, single_y] {
            table.add_run(y, Run::new(0, 30)).unwrap();
        }
        let sections = build_sections(&table, JunctionPolicy::All);
        let mut ownership = ClusterOwnership::new();
        let filaments = sections
            .into_iter()
            .enumerate()
            .map(|(index, section)| {
                let id = FilamentId::new(index as u64 + 1);
                let mut filament = StaffFilament::new(10).unwrap();
                filament.add_section(section).unwrap();
                ownership.register_filament(id, &filament).unwrap();
                (id, filament)
            })
            .collect();
        (ownership, filaments)
    }

    fn parameters() -> ClusterRetrievalParameters {
        let expansion = ClusterExpansionParameters::new(0.0, 0, 1, 0, 0.1, 1, 10).unwrap();
        let compatibility = ClusterMergeParameters::new(0.0, 0, 0.1, 0, 1, 10).unwrap();
        let merge = ClusterMergePassParameters::new(compatibility, 0, 1).unwrap();
        ClusterRetrievalParameters::new(
            10,
            BTreeSet::from([1, 2]),
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

    #[test]
    fn full_pipeline_recovers_a_remote_one_line_cluster() {
        let (mut ownership, filaments) = fixture();
        let mut comb = FilamentComb::new(0);
        comb.append_root(1, 10.0).unwrap();
        comb.append_root(2, 20.0).unwrap();
        let comb_id = CombId::new(1);
        ownership.register_comb(comb_id, &comb).unwrap();
        let mut combs = BTreeMap::from([(comb_id, RecursiveCombSnapshot::from_comb(&comb))]);
        let mut clusters = BTreeMap::new();

        let result = retrieve_clusters(
            &mut ownership,
            &mut clusters,
            &filaments,
            &mut combs,
            &[FilamentId::new(1), FilamentId::new(2), FilamentId::new(3)],
            &parameters(),
        )
        .unwrap();

        assert_eq!(result.popular_comb_size(), Some(2));
        assert_eq!(result.cluster_ids().len(), 2);
        assert_eq!(
            result.one_line_cluster_ids(),
            [ClusterId::from_seed(FilamentId::new(3))]
        );
        assert_eq!(
            result.active_filaments(),
            [FilamentId::new(1), FilamentId::new(2), FilamentId::new(3)]
        );
        assert!(result.discarded_filaments().is_empty());
        assert_eq!(clusters[&result.cluster_ids()[0]].size(), 2);
        assert_eq!(clusters[&result.cluster_ids()[1]].size(), 1);
    }

    #[test]
    fn duplicate_input_is_transactional() {
        let (mut ownership, filaments) = fixture();
        let before_membership = ownership.membership_of(FilamentId::new(1)).unwrap();
        let mut clusters = BTreeMap::new();
        let mut combs = BTreeMap::new();
        let id = FilamentId::new(1);
        assert_eq!(
            retrieve_clusters(
                &mut ownership,
                &mut clusters,
                &filaments,
                &mut combs,
                &[id, id],
                &parameters(),
            ),
            Err(ClusterPipelineError::DuplicateFilamentOrder(id))
        );
        assert_eq!(
            ownership.membership_of(FilamentId::new(1)).unwrap(),
            before_membership
        );
        assert!(clusters.is_empty());
        assert!(combs.is_empty());
    }

    #[test]
    fn recovered_ledger_like_single_is_rejected_near_standard_cluster() {
        let (mut ownership, filaments) = fixture_with_single_at(22);
        let mut comb = FilamentComb::new(0);
        comb.append_root(1, 10.0).unwrap();
        comb.append_root(2, 20.0).unwrap();
        let comb_id = CombId::new(1);
        ownership.register_comb(comb_id, &comb).unwrap();
        let mut combs = BTreeMap::from([(comb_id, RecursiveCombSnapshot::from_comb(&comb))]);
        let mut clusters = BTreeMap::new();
        let mut parameters = parameters();
        parameters.one_line_vertical_margin = 2;

        let result = retrieve_clusters(
            &mut ownership,
            &mut clusters,
            &filaments,
            &mut combs,
            &[FilamentId::new(1), FilamentId::new(2), FilamentId::new(3)],
            &parameters,
        )
        .unwrap();

        let rejected = ClusterId::from_seed(FilamentId::new(3));
        assert_eq!(result.cluster_ids().len(), 1);
        assert!(result.one_line_cluster_ids().is_empty());
        assert_eq!(result.discarded_clusters(), [rejected]);
        assert_eq!(result.discarded_filaments(), [FilamentId::new(3)]);
        assert!(!clusters.contains_key(&rejected));
        assert_eq!(ownership.membership_of(FilamentId::new(3)).unwrap(), None);
    }
}
