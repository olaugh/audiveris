// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light cluster finalization from Java `ClustersRetriever`.
//!
//! This module starts with the stage immediately after `mergeClusterPairs`:
//! rejecting clusters whose raw member-line widths differ too much. Sheet
//! constants are resolved by the caller and Java object back-links are handled
//! by the neutral ownership registry.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use crate::{
    cluster_ownership::{ClusterId, ClusterOwnership, ClusterOwnershipError},
    filament::FilamentError,
    line_cluster::{LineCluster, LineClusterError},
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClusterFinalizeError {
    InvalidParameters,
    MissingClusterValue(ClusterId),
    DuplicateClusterOrder(ClusterId),
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
}
