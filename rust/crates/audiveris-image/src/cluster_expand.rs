// SPDX-License-Identifier: AGPL-3.0-or-later

//! Neutral orchestration for Java `ClustersRetriever.expandClusters`.

use std::{
    cmp::{Ordering, Reverse},
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use crate::{
    cluster_ownership::{ClusterId, ClusterOwnership, ClusterOwnershipError},
    filament::{FilamentError, StaffFilament},
    line_cluster::{FilamentId, LineCluster, LineClusterError},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClusterExpansionParameters {
    global_slope: f64,
    maximum_cluster_x_margin: i64,
    cluster_y_margin: i64,
    maximum_extrapolation: usize,
    maximum_ordinate_distance: f64,
    probe_width: usize,
    maximum_foreground: usize,
}

impl ClusterExpansionParameters {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        global_slope: f64,
        maximum_cluster_x_margin: i64,
        cluster_y_margin: i64,
        maximum_extrapolation: usize,
        maximum_ordinate_distance: f64,
        probe_width: usize,
        maximum_foreground: usize,
    ) -> Result<Self, ClusterExpansionError> {
        if !global_slope.is_finite()
            || maximum_cluster_x_margin < 0
            || cluster_y_margin < 0
            || !maximum_ordinate_distance.is_finite()
            || maximum_ordinate_distance < 0.0
        {
            return Err(ClusterExpansionError::InvalidParameters);
        }
        Ok(Self {
            global_slope,
            maximum_cluster_x_margin,
            cluster_y_margin,
            maximum_extrapolation,
            maximum_ordinate_distance,
            probe_width,
            maximum_foreground,
        })
    }
}

/// Java `expandClusters`, using current root-filament values and explicit list
/// order instead of Sheet-owned mutable objects.
///
/// Clusters are stably sorted by decreasing true length. Candidate filaments
/// are stably sorted first by start abscissa and then, from that ordering, by
/// stop abscissa. Each cluster scans the stop ordering and then the start
/// ordering. Accepted growth invalidates its rough bounds but does not restart
/// either scan. The complete operation is transactional.
pub fn expand_clusters_in_order(
    ownership: &mut ClusterOwnership,
    clusters: &mut BTreeMap<ClusterId, LineCluster>,
    cluster_order: &[ClusterId],
    filament_values: &BTreeMap<FilamentId, StaffFilament>,
    filament_order: &[FilamentId],
    parameters: ClusterExpansionParameters,
) -> Result<Vec<ClusterId>, ClusterExpansionError> {
    let mut seen_clusters = BTreeSet::new();
    let mut ordered_clusters = Vec::with_capacity(cluster_order.len());
    for &id in cluster_order {
        if !seen_clusters.insert(id) {
            return Err(ClusterExpansionError::DuplicateClusterOrder(id));
        }
        if !clusters.contains_key(&id) {
            return Err(ClusterExpansionError::MissingClusterValue(id));
        }
        ordered_clusters.push((id, clusters[&id].true_length()?));
    }

    let mut seen_filaments = BTreeSet::new();
    let mut starts = Vec::with_capacity(filament_order.len());
    for &id in filament_order {
        if !seen_filaments.insert(id) {
            return Err(ClusterExpansionError::DuplicateFilamentOrder(id));
        }
        ownership.filament_ancestor(id)?;
        let filament = filament_values
            .get(&id)
            .ok_or(ClusterExpansionError::MissingFilamentValue(id))?;
        let geometry = filament.geometry()?;
        starts.push((id, geometry.start().0, geometry.stop().0));
    }
    starts.sort_by(|one, two| java_double_compare(one.1, two.1));
    let mut stops = starts.clone();
    stops.sort_by(|one, two| java_double_compare(one.2, two.2));
    ordered_clusters.sort_by_key(|&(_, length)| Reverse(length));

    let mut next_ownership = ownership.clone();
    let mut next_clusters = clusters.clone();
    for &(cluster, _) in &ordered_clusters {
        expand_one(
            cluster,
            &stops,
            &mut next_ownership,
            &mut next_clusters,
            filament_values,
            parameters,
        )?;
        expand_one(
            cluster,
            &starts,
            &mut next_ownership,
            &mut next_clusters,
            filament_values,
            parameters,
        )?;
    }

    *ownership = next_ownership;
    *clusters = next_clusters;
    Ok(ordered_clusters.into_iter().map(|(id, _)| id).collect())
}

fn expand_one(
    cluster_id: ClusterId,
    candidates: &[(FilamentId, f64, f64)],
    ownership: &mut ClusterOwnership,
    clusters: &mut BTreeMap<ClusterId, LineCluster>,
    filament_values: &BTreeMap<FilamentId, StaffFilament>,
    parameters: ClusterExpansionParameters,
) -> Result<(), ClusterExpansionError> {
    let mut grown_bounds = None;
    for &(candidate_id, _, _) in candidates {
        let candidate_id = ownership.filament_ancestor(candidate_id)?;
        if ownership.membership_of(candidate_id)?.is_some() {
            continue;
        }
        let candidate = filament_values
            .get(&candidate_id)
            .ok_or(ClusterExpansionError::MissingFilamentValue(candidate_id))?;
        let bounds = candidate.bounds()?;
        let middle_x = bounds.x + (bounds.width / 2);
        let middle_y = candidate
            .geometry()?
            .position_at(middle_x as f64)?
            .round_ties_even() as i64;

        let rough = if let Some(bounds) = grown_bounds {
            bounds
        } else {
            let bounds = clusters
                .get(&cluster_id)
                .ok_or(ClusterExpansionError::MissingClusterValue(cluster_id))?
                .bounds()?;
            let bounds = GrownBounds::new(
                bounds,
                parameters.maximum_cluster_x_margin,
                parameters.cluster_y_margin,
            );
            grown_bounds = Some(bounds);
            bounds
        };
        if !rough.contains(middle_x as i64, middle_y) {
            continue;
        }

        let points = clusters
            .get(&cluster_id)
            .ok_or(ClusterExpansionError::MissingClusterValue(cluster_id))?
            .points_at(
                middle_x as f64,
                parameters.maximum_extrapolation,
                parameters.global_slope,
            )?;
        for point in points.iter().flatten() {
            if (middle_y as f64 - point.1).abs() > parameters.maximum_ordinate_distance {
                continue;
            }
            // Java calls `points.indexOf(point)`, so equal points address the
            // first matching index rather than the loop's current index.
            let index = points
                .iter()
                .position(|value| value.as_ref() == Some(point))
                .expect("iterated point remains present");
            let resident = clusters[&cluster_id]
                .line_by_index(index)
                .expect("point index addresses a cluster line")
                .primary_id();
            if clusters
                .get_mut(&cluster_id)
                .ok_or(ClusterExpansionError::MissingClusterValue(cluster_id))?
                .include_filament_by_index(
                    candidate_id,
                    candidate.clone(),
                    index,
                    parameters.probe_width,
                    parameters.maximum_foreground,
                )?
            {
                ownership.merge_filaments(resident, candidate_id)?;
                grown_bounds = None;
                break;
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct GrownBounds {
    left: i64,
    right: i64,
    top: i64,
    bottom: i64,
}

impl GrownBounds {
    fn new(bounds: crate::section::Bounds, x_margin: i64, y_margin: i64) -> Self {
        Self {
            left: bounds.x as i64 - x_margin,
            right: (bounds.x + bounds.width) as i64 + x_margin,
            top: bounds.y as i64 - y_margin,
            bottom: (bounds.y + bounds.height) as i64 + y_margin,
        }
    }

    fn contains(self, x: i64, y: i64) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

fn java_double_compare(one: f64, two: f64) -> Ordering {
    one.partial_cmp(&two).unwrap_or_else(|| {
        if one.is_nan() == two.is_nan() {
            Ordering::Equal
        } else if one.is_nan() {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClusterExpansionError {
    InvalidParameters,
    MissingClusterValue(ClusterId),
    MissingFilamentValue(FilamentId),
    DuplicateClusterOrder(ClusterId),
    DuplicateFilamentOrder(FilamentId),
    Cluster(LineClusterError),
    Filament(FilamentError),
    Ownership(ClusterOwnershipError),
}

impl From<LineClusterError> for ClusterExpansionError {
    fn from(value: LineClusterError) -> Self {
        Self::Cluster(value)
    }
}

impl From<FilamentError> for ClusterExpansionError {
    fn from(value: FilamentError) -> Self {
        Self::Filament(value)
    }
}

impl From<ClusterOwnershipError> for ClusterExpansionError {
    fn from(value: ClusterOwnershipError) -> Self {
        Self::Ownership(value)
    }
}

impl fmt::Display for ClusterExpansionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParameters => {
                formatter.write_str("cluster expansion parameters are invalid")
            }
            Self::MissingClusterValue(id) => {
                write!(formatter, "missing cluster value {}", id.value())
            }
            Self::MissingFilamentValue(id) => {
                write!(formatter, "missing filament value {}", id.value())
            }
            Self::DuplicateClusterOrder(id) => write!(
                formatter,
                "duplicate cluster {} in expansion order",
                id.value()
            ),
            Self::DuplicateFilamentOrder(id) => write!(
                formatter,
                "duplicate filament {} in expansion order",
                id.value()
            ),
            Self::Cluster(error) => write!(formatter, "cluster expansion geometry error: {error}"),
            Self::Filament(error) => write!(formatter, "cluster expansion filament error: {error}"),
            Self::Ownership(error) => {
                write!(formatter, "cluster expansion ownership error: {error}")
            }
        }
    }
}

impl Error for ClusterExpansionError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections},
    };

    fn state(
        specs: &[(u64, usize, usize, usize)],
        clustered: &[u64],
    ) -> (
        ClusterOwnership,
        BTreeMap<ClusterId, LineCluster>,
        BTreeMap<FilamentId, StaffFilament>,
        Vec<FilamentId>,
    ) {
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
        let mut ownership = ClusterOwnership::new();
        let mut values = BTreeMap::new();
        let mut order = Vec::new();
        for &(value, x, y, length) in specs {
            let index = sections
                .iter()
                .position(|section| {
                    let bounds = section.bounds();
                    bounds.x == x && bounds.y == y && bounds.width == length
                })
                .unwrap();
            let mut filament = StaffFilament::new(10).unwrap();
            filament.add_section(sections.remove(index)).unwrap();
            let id = FilamentId::new(value);
            ownership.register_filament(id, &filament).unwrap();
            values.insert(id, filament);
            order.push(id);
        }
        let mut clusters = BTreeMap::new();
        for &value in clustered {
            let id = FilamentId::new(value);
            let cluster = ownership.register_cluster(id).unwrap();
            clusters.insert(
                cluster,
                LineCluster::new(10, id, values[&id].clone()).unwrap(),
            );
        }
        (ownership, clusters, values, order)
    }

    fn parameters(x_margin: i64, y_margin: i64, dy: f64) -> ClusterExpansionParameters {
        ClusterExpansionParameters::new(0.0, x_margin, y_margin, 50, dy, 4, 3).unwrap()
    }

    #[test]
    fn two_directional_scans_revisit_candidate_after_growth() {
        let (mut ownership, mut clusters, values, filament_order) =
            state(&[(1, 150, 10, 30), (2, 100, 10, 30), (3, 50, 10, 30)], &[1]);
        let cluster = ClusterId::from_seed(FilamentId::new(1));

        let order = expand_clusters_in_order(
            &mut ownership,
            &mut clusters,
            &[cluster],
            &values,
            &filament_order,
            parameters(35, 1, 0.0),
        )
        .unwrap();

        assert_eq!(order, [cluster]);
        assert_eq!(
            clusters[&cluster].line_at(0).unwrap().absorbed_ids(),
            &[FilamentId::new(2), FilamentId::new(3)]
        );
        assert_eq!(
            ownership.filament_ancestor(FilamentId::new(3)).unwrap(),
            FilamentId::new(1)
        );
    }

    #[test]
    fn reverse_true_length_order_gives_long_cluster_first_choice() {
        let (mut ownership, mut clusters, values, filament_order) = state(
            &[(1, 40, 12, 60), (2, 0, 10, 100), (3, 100, 11, 30)],
            &[1, 2],
        );
        let short = ClusterId::from_seed(FilamentId::new(1));
        let long = ClusterId::from_seed(FilamentId::new(2));

        let order = expand_clusters_in_order(
            &mut ownership,
            &mut clusters,
            &[short, long],
            &values,
            &filament_order,
            parameters(20, 2, 1.0),
        )
        .unwrap();

        assert_eq!(order, [long, short]);
        assert_eq!(
            ownership
                .membership_of(FilamentId::new(3))
                .unwrap()
                .unwrap()
                .cluster(),
            long
        );
        assert_eq!(
            clusters[&long].line_at(0).unwrap().absorbed_ids(),
            &[FilamentId::new(3)]
        );
        assert!(
            clusters[&short]
                .line_at(0)
                .unwrap()
                .absorbed_ids()
                .is_empty()
        );
    }

    #[test]
    fn grown_rectangle_contains_is_half_open_at_right_edge() {
        let (mut ownership, mut clusters, values, filament_order) =
            state(&[(1, 0, 10, 30), (2, 30, 12, 30)], &[1]);
        let cluster = ClusterId::from_seed(FilamentId::new(1));
        expand_clusters_in_order(
            &mut ownership,
            &mut clusters,
            &[cluster],
            &values,
            &filament_order,
            parameters(15, 3, 2.0),
        )
        .unwrap();
        assert!(
            clusters[&cluster]
                .line_at(0)
                .unwrap()
                .absorbed_ids()
                .is_empty()
        );

        let (mut ownership, mut clusters, values, filament_order) =
            state(&[(1, 0, 10, 30), (2, 29, 12, 30)], &[1]);
        expand_clusters_in_order(
            &mut ownership,
            &mut clusters,
            &[cluster],
            &values,
            &filament_order,
            parameters(15, 3, 2.0),
        )
        .unwrap();
        assert_eq!(
            clusters[&cluster].line_at(0).unwrap().absorbed_ids(),
            &[FilamentId::new(2)]
        );
    }

    #[test]
    fn duplicate_order_fails_before_mutation() {
        let (mut ownership, mut clusters, values, filament_order) =
            state(&[(1, 0, 10, 30), (2, 40, 10, 30)], &[1]);
        let cluster = ClusterId::from_seed(FilamentId::new(1));
        assert_eq!(
            expand_clusters_in_order(
                &mut ownership,
                &mut clusters,
                &[cluster],
                &values,
                &[filament_order[0], filament_order[0]],
                parameters(10, 1, 0.0),
            ),
            Err(ClusterExpansionError::DuplicateFilamentOrder(
                filament_order[0]
            ))
        );
        assert!(
            clusters[&cluster]
                .line_at(0)
                .unwrap()
                .absorbed_ids()
                .is_empty()
        );
    }
}
