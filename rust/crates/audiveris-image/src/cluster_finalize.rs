// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light cluster finalization from Java `ClustersRetriever`.
//!
//! This module starts with the stage immediately after `mergeClusterPairs`:
//! rejecting clusters whose raw member-line widths differ too much. Sheet
//! constants are resolved by the caller and Java object back-links are handled
//! by the neutral ownership registry.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use crate::{
    cluster_ownership::{ClusterId, ClusterOwnership, ClusterOwnershipError},
    filament::FilamentError,
    line_cluster::{FilamentId, LineCluster, LineClusterError},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClusterConsistencyParameters {
    maximum_length_difference_ratio: f64,
}

impl ClusterConsistencyParameters {
    pub fn new(maximum_length_difference_ratio: f64) -> Result<Self, ClusterFinalizeError> {
        if !maximum_length_difference_ratio.is_finite() || maximum_length_difference_ratio < 0.0 {
            return Err(ClusterFinalizeError::InvalidParameters);
        }
        Ok(Self {
            maximum_length_difference_ratio,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterConsistencyResult {
    survivors: Vec<ClusterId>,
    discarded: Vec<ClusterId>,
}

impl ClusterConsistencyResult {
    #[must_use]
    pub fn survivors(&self) -> &[ClusterId] {
        &self.survivors
    }

    #[must_use]
    pub fn discarded(&self) -> &[ClusterId] {
        &self.discarded
    }

    #[must_use]
    pub fn into_parts(self) -> (Vec<ClusterId>, Vec<ClusterId>) {
        (self.survivors, self.discarded)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AcceptableClusterLength {
    median: usize,
    minimum: f64,
}

impl AcceptableClusterLength {
    #[must_use]
    pub const fn median(self) -> usize {
        self.median
    }

    #[must_use]
    pub const fn minimum(self) -> f64 {
        self.minimum
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilamentPartitionResult {
    remaining: Vec<FilamentId>,
    discarded: Vec<FilamentId>,
    merged: Vec<FilamentId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterTrimResult {
    cluster_order: Vec<ClusterId>,
    removed_filaments: Vec<FilamentId>,
}

impl ClusterTrimResult {
    #[must_use]
    pub fn cluster_order(&self) -> &[ClusterId] {
        &self.cluster_order
    }

    #[must_use]
    pub fn removed_filaments(&self) -> &[FilamentId] {
        &self.removed_filaments
    }
}

impl FilamentPartitionResult {
    #[must_use]
    pub fn remaining(&self) -> &[FilamentId] {
        &self.remaining
    }

    #[must_use]
    pub fn discarded(&self) -> &[FilamentId] {
        &self.discarded
    }

    #[must_use]
    pub fn merged(&self) -> &[FilamentId] {
        &self.merged
    }
}

/// Java `computeAcceptableLength`: upper median true length times a ratio.
pub fn compute_acceptable_cluster_length(
    clusters: &BTreeMap<ClusterId, LineCluster>,
    cluster_order: &[ClusterId],
    minimum_length_ratio: f64,
) -> Result<AcceptableClusterLength, ClusterFinalizeError> {
    if !minimum_length_ratio.is_finite() || minimum_length_ratio < 0.0 {
        return Err(ClusterFinalizeError::InvalidParameters);
    }
    if cluster_order.is_empty() {
        return Err(ClusterFinalizeError::EmptyClusterOrder);
    }
    let mut seen = BTreeSet::new();
    let mut lengths = Vec::with_capacity(cluster_order.len());
    for &id in cluster_order {
        if !seen.insert(id) {
            return Err(ClusterFinalizeError::DuplicateClusterOrder(id));
        }
        let cluster = clusters
            .get(&id)
            .ok_or(ClusterFinalizeError::MissingClusterValue(id))?;
        lengths.push(cluster.true_length()?);
    }
    lengths.sort_unstable();
    let median = lengths[lengths.len() / 2];
    Ok(AcceptableClusterLength {
        median,
        minimum: median as f64 * minimum_length_ratio,
    })
}

/// Java `isConsistent`: compare the shortest and longest raw line widths.
/// The maximum ratio is inclusive because Java rejects only strict `>`.
pub fn cluster_has_consistent_lengths(
    cluster: &LineCluster,
    parameters: ClusterConsistencyParameters,
) -> Result<bool, ClusterFinalizeError> {
    let mut minimum = usize::MAX;
    let mut maximum = usize::MIN;
    for (_, line) in cluster.lines() {
        let width = line.filament().bounds()?.width;
        minimum = minimum.min(width);
        maximum = maximum.max(width);
    }
    let mean = (minimum as f64 + maximum as f64) / 2.0;
    let difference_ratio = (maximum - minimum) as f64 / mean;
    Ok(difference_ratio <= parameters.maximum_length_difference_ratio)
}

/// Java `destroyInconsistentClusters`, preserving the caller's current order.
///
/// Rejected values are removed and their member cluster backlinks and reverse
/// comb links are cleared through `ClusterOwnership::destroy_cluster`. The full
/// pass is transactional.
pub fn destroy_inconsistent_clusters_in_order(
    ownership: &mut ClusterOwnership,
    clusters: &mut BTreeMap<ClusterId, LineCluster>,
    cluster_order: &[ClusterId],
    parameters: ClusterConsistencyParameters,
) -> Result<ClusterConsistencyResult, ClusterFinalizeError> {
    let mut seen = BTreeSet::new();
    for &id in cluster_order {
        if !seen.insert(id) {
            return Err(ClusterFinalizeError::DuplicateClusterOrder(id));
        }
        if !clusters.contains_key(&id) {
            return Err(ClusterFinalizeError::MissingClusterValue(id));
        }
    }

    let mut next_ownership = ownership.clone();
    let mut next_clusters = clusters.clone();
    let mut survivors = Vec::new();
    let mut discarded = Vec::new();
    for &id in cluster_order {
        if cluster_has_consistent_lengths(&next_clusters[&id], parameters)? {
            survivors.push(id);
        } else {
            next_ownership.destroy_cluster(id)?;
            next_clusters.remove(&id);
            discarded.push(id);
        }
    }

    *ownership = next_ownership;
    *clusters = next_clusters;
    Ok(ClusterConsistencyResult {
        survivors,
        discarded,
    })
}

/// Java `destroyNonDesiredClusters`: destroy and remove clusters whose exact
/// line count is absent from the requested comb sizes.
pub fn destroy_non_desired_clusters_in_order(
    ownership: &mut ClusterOwnership,
    clusters: &mut BTreeMap<ClusterId, LineCluster>,
    cluster_order: &[ClusterId],
    desired_sizes: &BTreeSet<usize>,
) -> Result<ClusterConsistencyResult, ClusterFinalizeError> {
    let mut seen = BTreeSet::new();
    for &id in cluster_order {
        if !seen.insert(id) {
            return Err(ClusterFinalizeError::DuplicateClusterOrder(id));
        }
        if !clusters.contains_key(&id) {
            return Err(ClusterFinalizeError::MissingClusterValue(id));
        }
    }
    let mut next_ownership = ownership.clone();
    let mut next_clusters = clusters.clone();
    let mut survivors = Vec::new();
    let mut discarded = Vec::new();
    for &id in cluster_order {
        if desired_sizes.contains(&next_clusters[&id].size()) {
            survivors.push(id);
        } else {
            next_ownership.destroy_cluster(id)?;
            next_clusters.remove(&id);
            discarded.push(id);
        }
    }
    *ownership = next_ownership;
    *clusters = next_clusters;
    Ok(ClusterConsistencyResult {
        survivors,
        discarded,
    })
}

/// Neutral `discardNonClusteredFilaments` partition in caller list order.
///
/// A merged child remains in Java's list at this stage and is reported
/// separately for the later `removeMergedFilaments` pass. Only unmerged roots
/// without cluster membership enter the discarded list.
pub fn partition_non_clustered_filaments_in_order(
    ownership: &ClusterOwnership,
    filament_order: &[FilamentId],
) -> Result<FilamentPartitionResult, ClusterFinalizeError> {
    let mut seen = BTreeSet::new();
    let mut remaining = Vec::new();
    let mut discarded = Vec::new();
    let mut merged = Vec::new();
    for &id in filament_order {
        if !seen.insert(id) {
            return Err(ClusterFinalizeError::DuplicateFilamentOrder(id));
        }
        if ownership.filament_parent(id)?.is_some() {
            remaining.push(id);
            merged.push(id);
        } else if ownership.membership_of(id)?.is_some() {
            remaining.push(id);
        } else {
            discarded.push(id);
        }
    }
    Ok(FilamentPartitionResult {
        remaining,
        discarded,
        merged,
    })
}

/// Java `trimClusters`: stably sort by deskewed ordinate, trim each cluster,
/// release removed line roots, and synchronize renumbered memberships.
pub fn trim_clusters_in_ordinate_order(
    ownership: &mut ClusterOwnership,
    clusters: &mut BTreeMap<ClusterId, LineCluster>,
    cluster_order: &[ClusterId],
    global_slope: f64,
    allowed_comb_sizes: &BTreeSet<usize>,
    minimum_tablature_length_ratio: f64,
) -> Result<ClusterTrimResult, ClusterFinalizeError> {
    if !global_slope.is_finite() {
        return Err(ClusterFinalizeError::InvalidParameters);
    }
    let mut seen = BTreeSet::new();
    let mut ordered = cluster_order
        .iter()
        .copied()
        .map(|id| {
            if !seen.insert(id) {
                return Err(ClusterFinalizeError::DuplicateClusterOrder(id));
            }
            let cluster = clusters
                .get(&id)
                .ok_or(ClusterFinalizeError::MissingClusterValue(id))?;
            let bounds = cluster.bounds()?;
            let x = bounds.x + (bounds.width / 2);
            let y = bounds.y + (bounds.height / 2);
            Ok((id, y as f64 - (global_slope * x as f64)))
        })
        .collect::<Result<Vec<_>, ClusterFinalizeError>>()?;
    ordered.sort_by(|one, two| one.1.partial_cmp(&two.1).unwrap_or(Ordering::Equal));

    let mut next_ownership = ownership.clone();
    let mut next_clusters = clusters.clone();
    let mut removed_filaments = Vec::new();
    for &(id, _) in &ordered {
        let removed = next_clusters
            .get_mut(&id)
            .ok_or(ClusterFinalizeError::MissingClusterValue(id))?
            .trim(allowed_comb_sizes, minimum_tablature_length_ratio)?;
        let remaining = next_clusters[&id]
            .lines()
            .map(|(position, line)| (line.primary_id(), position))
            .collect::<Vec<_>>();
        let removed = removed
            .into_iter()
            .map(|line| line.primary_id())
            .collect::<Vec<_>>();
        next_ownership.synchronize_trimmed_cluster(id, &remaining, &removed)?;
        removed_filaments.extend(removed);
    }
    let cluster_order = ordered.into_iter().map(|(id, _)| id).collect();
    *ownership = next_ownership;
    *clusters = next_clusters;
    Ok(ClusterTrimResult {
        cluster_order,
        removed_filaments,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClusterFinalizeError {
    InvalidParameters,
    EmptyClusterOrder,
    MissingClusterValue(ClusterId),
    DuplicateClusterOrder(ClusterId),
    DuplicateFilamentOrder(FilamentId),
    Cluster(LineClusterError),
    Filament(FilamentError),
    Ownership(ClusterOwnershipError),
}

impl From<LineClusterError> for ClusterFinalizeError {
    fn from(value: LineClusterError) -> Self {
        Self::Cluster(value)
    }
}

impl From<FilamentError> for ClusterFinalizeError {
    fn from(value: FilamentError) -> Self {
        Self::Filament(value)
    }
}

impl From<ClusterOwnershipError> for ClusterFinalizeError {
    fn from(value: ClusterOwnershipError) -> Self {
        Self::Ownership(value)
    }
}

impl fmt::Display for ClusterFinalizeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParameters => {
                formatter.write_str("cluster consistency parameters are invalid")
            }
            Self::EmptyClusterOrder => formatter.write_str("cluster order is empty"),
            Self::MissingClusterValue(id) => {
                write!(formatter, "missing cluster value {}", id.value())
            }
            Self::DuplicateClusterOrder(id) => {
                write!(
                    formatter,
                    "duplicate cluster {} in consistency order",
                    id.value()
                )
            }
            Self::DuplicateFilamentOrder(id) => {
                write!(
                    formatter,
                    "duplicate filament {} in finalization order",
                    id.value()
                )
            }
            Self::Cluster(error) => {
                write!(formatter, "cluster consistency geometry error: {error}")
            }
            Self::Filament(error) => {
                write!(formatter, "cluster consistency filament error: {error}")
            }
            Self::Ownership(error) => {
                write!(formatter, "cluster consistency ownership error: {error}")
            }
        }
    }
}

impl Error for ClusterFinalizeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        filament::StaffFilament,
        line_cluster::FilamentId,
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections},
    };

    fn filaments(specs: &[(u64, usize, usize, usize)]) -> Vec<(FilamentId, StaffFilament)> {
        let width = specs
            .iter()
            .map(|(_, x, _, length)| x + length + 1)
            .max()
            .unwrap_or(1);
        let height = specs.iter().map(|(_, _, y, _)| y + 2).max().unwrap_or(1);
        let mut table = RunTable::new(Orientation::Horizontal, width, height).unwrap();
        for &(_, x, y, length) in specs {
            table.add_run(y, Run::new(x, length)).unwrap();
        }
        let mut sections = build_sections(&table, JunctionPolicy::All);
        specs
            .iter()
            .map(|&(id, x, y, length)| {
                let index = sections
                    .iter()
                    .position(|section| {
                        let bounds = section.bounds();
                        bounds.x == x && bounds.y == y && bounds.width == length
                    })
                    .unwrap();
                let mut filament = StaffFilament::new(10).unwrap();
                filament.add_section(sections.remove(index)).unwrap();
                (FilamentId::new(id), filament)
            })
            .collect()
    }

    fn two_line_cluster(
        ownership: &mut ClusterOwnership,
        clusters: &mut BTreeMap<ClusterId, LineCluster>,
        first: (FilamentId, StaffFilament),
        second: (FilamentId, StaffFilament),
    ) -> ClusterId {
        ownership.register_filament(first.0, &first.1).unwrap();
        ownership.register_filament(second.0, &second.1).unwrap();
        let cluster = ownership.register_cluster(first.0).unwrap();
        ownership.assign_filament(second.0, cluster, 1).unwrap();
        let mut value = LineCluster::new(10, first.0, first.1).unwrap();
        value.include_line(1, second.0, second.1).unwrap();
        clusters.insert(cluster, value);
        cluster
    }

    fn cluster_from_lines(
        ownership: &mut ClusterOwnership,
        clusters: &mut BTreeMap<ClusterId, LineCluster>,
        lines: Vec<(FilamentId, StaffFilament)>,
    ) -> ClusterId {
        let mut lines = lines.into_iter();
        let (seed_id, seed) = lines.next().unwrap();
        ownership.register_filament(seed_id, &seed).unwrap();
        let cluster = ownership.register_cluster(seed_id).unwrap();
        let mut value = LineCluster::new(10, seed_id, seed).unwrap();
        for (index, (id, filament)) in lines.enumerate() {
            ownership.register_filament(id, &filament).unwrap();
            let position = (index + 1) as i32;
            ownership.assign_filament(id, cluster, position).unwrap();
            value.include_line(position, id, filament).unwrap();
        }
        clusters.insert(cluster, value);
        cluster
    }

    #[test]
    fn consistency_ratio_accepts_exact_limit_and_rejects_below_it() {
        let mut ownership = ClusterOwnership::new();
        let mut clusters = BTreeMap::new();
        let mut filaments = filaments(&[(1, 0, 10, 20), (2, 0, 20, 12)]).into_iter();
        let cluster = two_line_cluster(
            &mut ownership,
            &mut clusters,
            filaments.next().unwrap(),
            filaments.next().unwrap(),
        );
        assert!(
            cluster_has_consistent_lengths(
                &clusters[&cluster],
                ClusterConsistencyParameters::new(0.5).unwrap(),
            )
            .unwrap()
        );
        assert!(
            !cluster_has_consistent_lengths(
                &clusters[&cluster],
                ClusterConsistencyParameters::new(0.499).unwrap(),
            )
            .unwrap()
        );
    }

    #[test]
    fn inconsistent_pass_destroys_backlinks_and_preserves_order() {
        let mut ownership = ClusterOwnership::new();
        let mut clusters = BTreeMap::new();
        let mut filaments = filaments(&[
            (1, 0, 10, 20),
            (2, 0, 20, 12),
            (3, 50, 30, 20),
            (4, 50, 40, 8),
        ])
        .into_iter();
        let accepted = two_line_cluster(
            &mut ownership,
            &mut clusters,
            filaments.next().unwrap(),
            filaments.next().unwrap(),
        );
        let rejected = two_line_cluster(
            &mut ownership,
            &mut clusters,
            filaments.next().unwrap(),
            filaments.next().unwrap(),
        );

        let result = destroy_inconsistent_clusters_in_order(
            &mut ownership,
            &mut clusters,
            &[accepted, rejected],
            ClusterConsistencyParameters::new(0.5).unwrap(),
        )
        .unwrap();

        assert_eq!(result.survivors(), [accepted]);
        assert_eq!(result.discarded(), [rejected]);
        assert_eq!(clusters.keys().copied().collect::<Vec<_>>(), [accepted]);
        assert_eq!(
            ownership
                .membership_of(FilamentId::new(1))
                .unwrap()
                .unwrap()
                .cluster(),
            accepted
        );
        assert_eq!(ownership.membership_of(FilamentId::new(3)).unwrap(), None);
        assert_eq!(ownership.membership_of(FilamentId::new(4)).unwrap(), None);
    }

    #[test]
    fn duplicate_order_rejects_without_mutation() {
        let mut ownership = ClusterOwnership::new();
        let mut clusters = BTreeMap::new();
        let mut filaments = filaments(&[(1, 0, 10, 20), (2, 0, 20, 12)]).into_iter();
        let cluster = two_line_cluster(
            &mut ownership,
            &mut clusters,
            filaments.next().unwrap(),
            filaments.next().unwrap(),
        );
        let before = clusters[&cluster].size();

        assert_eq!(
            destroy_inconsistent_clusters_in_order(
                &mut ownership,
                &mut clusters,
                &[cluster, cluster],
                ClusterConsistencyParameters::new(0.0).unwrap(),
            ),
            Err(ClusterFinalizeError::DuplicateClusterOrder(cluster))
        );
        assert_eq!(clusters[&cluster].size(), before);
        assert!(
            ownership
                .membership_of(FilamentId::new(1))
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn acceptable_length_uses_upper_median_and_exact_ratio() {
        let mut ownership = ClusterOwnership::new();
        let mut clusters = BTreeMap::new();
        let mut order = Vec::new();
        for (id, filament) in filaments(&[
            (1, 0, 10, 10),
            (2, 0, 20, 20),
            (3, 0, 30, 30),
            (4, 0, 40, 40),
        ]) {
            ownership.register_filament(id, &filament).unwrap();
            let cluster = ownership.register_cluster(id).unwrap();
            clusters.insert(cluster, LineCluster::new(10, id, filament).unwrap());
            order.push(cluster);
        }

        let result = compute_acceptable_cluster_length(&clusters, &order, 0.2).unwrap();
        assert_eq!(result.median(), 30);
        assert_eq!(result.minimum(), 6.0);
    }

    #[test]
    fn non_desired_size_pass_calls_destroy_and_preserves_survivor_order() {
        let mut ownership = ClusterOwnership::new();
        let mut clusters = BTreeMap::new();
        let mut filaments =
            filaments(&[(1, 0, 10, 30), (2, 50, 20, 30), (3, 50, 30, 30)]).into_iter();
        let (single_id, single_value) = filaments.next().unwrap();
        ownership
            .register_filament(single_id, &single_value)
            .unwrap();
        let single = ownership.register_cluster(single_id).unwrap();
        clusters.insert(
            single,
            LineCluster::new(10, single_id, single_value).unwrap(),
        );
        let double = two_line_cluster(
            &mut ownership,
            &mut clusters,
            filaments.next().unwrap(),
            filaments.next().unwrap(),
        );

        let result = destroy_non_desired_clusters_in_order(
            &mut ownership,
            &mut clusters,
            &[single, double],
            &BTreeSet::from([2]),
        )
        .unwrap();

        assert_eq!(result.survivors(), [double]);
        assert_eq!(result.discarded(), [single]);
        assert_eq!(ownership.membership_of(single_id).unwrap(), None);
        assert!(clusters.contains_key(&double));
        assert!(!clusters.contains_key(&single));
    }

    #[test]
    fn filament_partition_keeps_merged_children_for_later_removal() {
        let mut ownership = ClusterOwnership::new();
        let values = filaments(&[(1, 0, 10, 30), (2, 50, 20, 30), (3, 100, 30, 30)]);
        for (id, filament) in &values {
            ownership.register_filament(*id, filament).unwrap();
        }
        let cluster = ownership.register_cluster(values[0].0).unwrap();
        ownership.merge_filaments(values[0].0, values[2].0).unwrap();

        let result = partition_non_clustered_filaments_in_order(
            &ownership,
            &[values[0].0, values[1].0, values[2].0],
        )
        .unwrap();

        assert_eq!(result.remaining(), [values[0].0, values[2].0]);
        assert_eq!(result.discarded(), [values[1].0]);
        assert_eq!(result.merged(), [values[2].0]);
        assert_eq!(
            ownership
                .membership_of(values[2].0)
                .unwrap()
                .unwrap()
                .cluster(),
            cluster
        );
    }

    #[test]
    fn trim_pass_releases_removed_top_and_renumbers_memberships() {
        let mut ownership = ClusterOwnership::new();
        let mut clusters = BTreeMap::new();
        let cluster = cluster_from_lines(
            &mut ownership,
            &mut clusters,
            filaments(&[(1, 0, 10, 10), (2, 0, 20, 30), (3, 0, 30, 20)]),
        );
        let result = trim_clusters_in_ordinate_order(
            &mut ownership,
            &mut clusters,
            &[cluster],
            0.0,
            &BTreeSet::from([2]),
            0.5,
        )
        .unwrap();

        assert_eq!(result.cluster_order(), [cluster]);
        assert_eq!(result.removed_filaments(), [FilamentId::new(1)]);
        assert_eq!(ownership.membership_of(FilamentId::new(1)).unwrap(), None);
        assert_eq!(
            ownership
                .membership_of(FilamentId::new(2))
                .unwrap()
                .unwrap()
                .position(),
            0
        );
        assert_eq!(
            ownership
                .membership_of(FilamentId::new(3))
                .unwrap()
                .unwrap()
                .position(),
            1
        );
        assert_eq!(clusters[&cluster].first_position(), 0);
    }

    #[test]
    fn trim_pass_stably_sorts_clusters_by_deskewed_ordinate() {
        let mut ownership = ClusterOwnership::new();
        let mut clusters = BTreeMap::new();
        let mut values = filaments(&[(1, 0, 10, 30), (2, 0, 30, 30)]).into_iter();
        let upper = cluster_from_lines(&mut ownership, &mut clusters, vec![values.next().unwrap()]);
        let lower = cluster_from_lines(&mut ownership, &mut clusters, vec![values.next().unwrap()]);
        let result = trim_clusters_in_ordinate_order(
            &mut ownership,
            &mut clusters,
            &[lower, upper],
            0.0,
            &BTreeSet::from([1]),
            0.5,
        )
        .unwrap();

        assert_eq!(result.cluster_order(), [upper, lower]);
        assert!(result.removed_filaments().is_empty());
    }
}
