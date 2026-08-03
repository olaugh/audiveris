// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light merge compatibility from Java `ClustersRetriever.canMerge`.
//!
//! Sheet-owned scale fractions and skew are resolved into numeric parameters.
//! The kernel returns Java's directional delta (`one` line index to `two` line
//! index); the adapter transactionally merges `one` into `two` through the
//! neutral ownership coordinator.

use std::{collections::BTreeMap, error::Error, fmt};

use crate::{
    cluster_coordinator::{RecursiveIncludeError, merge_cluster_values},
    cluster_ownership::{ClusterId, ClusterOwnership, ClusterOwnershipError},
    filament::FilamentError,
    line_cluster::{LineCluster, LineClusterError, combined_thickness_at},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClusterMergeParameters {
    global_slope: f64,
    maximum_coordinate_gap: i64,
    maximum_ordinate_distance: f64,
    maximum_extrapolation: usize,
    probe_width: usize,
    maximum_foreground: usize,
}

impl ClusterMergeParameters {
    pub fn new(
        global_slope: f64,
        maximum_coordinate_gap: i64,
        maximum_ordinate_distance: f64,
        maximum_extrapolation: usize,
        probe_width: usize,
        maximum_foreground: usize,
    ) -> Result<Self, ClusterMergeError> {
        if !global_slope.is_finite()
            || maximum_coordinate_gap < 0
            || !maximum_ordinate_distance.is_finite()
            || maximum_ordinate_distance < 0.0
        {
            return Err(ClusterMergeError::InvalidParameters);
        }
        Ok(Self {
            global_slope,
            maximum_coordinate_gap,
            maximum_ordinate_distance,
            maximum_extrapolation,
            probe_width,
            maximum_foreground,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeMeasurement {
    Overlap,
    Endpoints,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClusterMergeDecision {
    delta_position: i32,
    mean_distance: f64,
    measurement: MergeMeasurement,
}

impl ClusterMergeDecision {
    #[must_use]
    pub const fn delta_position(self) -> i32 {
        self.delta_position
    }

    #[must_use]
    pub const fn mean_distance(self) -> f64 {
        self.mean_distance
    }

    #[must_use]
    pub const fn measurement(self) -> MergeMeasurement {
        self.measurement
    }
}

/// Java `bestMatch`: scan deltas from most negative to most positive and keep
/// the first strict minimum. Missing ordinates do not contribute.
#[must_use]
pub fn best_ordinate_match(one: &[Option<f64>], two: &[Option<f64>]) -> Option<(i32, f64)> {
    let maximum_len = one.len().max(two.len());
    if maximum_len == 0 || maximum_len > i32::MAX as usize {
        return None;
    }
    let delta_max = maximum_len as i32 - 1;
    let mut best = None;
    for delta in -delta_max..=delta_max {
        let mut sum = 0.0;
        let mut count = 0_usize;
        for (one_index, one_value) in one.iter().enumerate() {
            let two_index = one_index as i64 + i64::from(delta);
            if two_index < 0 || two_index >= two.len() as i64 {
                continue;
            }
            if let (Some(one_value), Some(two_value)) = (*one_value, two[two_index as usize]) {
                count += 1;
                sum += (two_value - one_value).abs();
            }
        }
        if count != 0 {
            let distance = sum / count as f64;
            // Java starts at Double.MAX_VALUE and uses strict `<`: NaN and
            // infinity never become a winner, and the first finite tie stays.
            if best.map_or(distance < f64::MAX, |(_, best_distance)| {
                distance < best_distance
            }) {
                best = Some((delta, distance));
            }
        }
    }
    best
}

/// Decide whether Java would merge `one` into `two` and report its line delta.
pub fn cluster_merge_compatibility(
    one: &LineCluster,
    two: &LineCluster,
    parameters: ClusterMergeParameters,
) -> Result<Option<ClusterMergeDecision>, ClusterMergeError> {
    let one_bounds = one.bounds()?;
    let two_bounds = two.bounds()?;
    let one_left = one_bounds.x as i64;
    let one_right = (one_bounds.x + one_bounds.width - 1) as i64;
    let two_left = two_bounds.x as i64;
    let two_right = (two_bounds.x + two_bounds.width - 1) as i64;

    if one.size() > 1 || two.size() > 1 {
        let minimum_right = one_right.min(two_right);
        let maximum_left = one_left.max(two_left);
        let gap = maximum_left - minimum_right;
        if gap > parameters.maximum_coordinate_gap {
            return Ok(None);
        }
        if gap <= 0 {
            let middle = (maximum_left + minimum_right) / 2;
            let one_values = deskew_points(
                one.points_at(
                    middle as f64,
                    parameters.maximum_extrapolation,
                    parameters.global_slope,
                )?,
                parameters.global_slope,
            );
            let two_values = deskew_points(
                two.points_at(
                    middle as f64,
                    parameters.maximum_extrapolation,
                    parameters.global_slope,
                )?,
                parameters.global_slope,
            );
            let Some((delta, distance)) = best_ordinate_match(&one_values, &two_values) else {
                return Ok(None);
            };
            if distance > parameters.maximum_ordinate_distance
                || !collision_is_clear(one, two, delta, parameters)?
            {
                return Ok(None);
            }
            return Ok(Some(ClusterMergeDecision {
                delta_position: delta,
                mean_distance: distance,
                measurement: MergeMeasurement::Overlap,
            }));
        }
    }

    let (one_points, two_points) = if one_left < two_left {
        (one.stops()?, two.starts()?)
    } else {
        (one.starts()?, two.stops()?)
    };
    let one_values = deskew_complete_points(&one_points, parameters.global_slope);
    let two_values = deskew_complete_points(&two_points, parameters.global_slope);
    let Some((delta, distance)) = best_ordinate_match(&one_values, &two_values) else {
        return Ok(None);
    };
    Ok(
        (distance <= parameters.maximum_ordinate_distance).then_some(ClusterMergeDecision {
            delta_position: delta,
            mean_distance: distance,
            measurement: MergeMeasurement::Endpoints,
        }),
    )
}

/// Decide and, if accepted, atomically merge `one` into `two`.
pub fn merge_cluster_pair_if_compatible(
    ownership: &mut ClusterOwnership,
    clusters: &mut BTreeMap<ClusterId, LineCluster>,
    one: ClusterId,
    two: ClusterId,
    parameters: ClusterMergeParameters,
) -> Result<Option<ClusterMergeDecision>, ClusterMergeError> {
    let one = ownership.cluster_ancestor(one)?;
    let two = ownership.cluster_ancestor(two)?;
    if one == two {
        return Ok(None);
    }
    let one_value = clusters
        .get(&one)
        .ok_or(ClusterMergeError::MissingClusterValue(one))?;
    let two_value = clusters
        .get(&two)
        .ok_or(ClusterMergeError::MissingClusterValue(two))?;
    let Some(decision) = cluster_merge_compatibility(one_value, two_value, parameters)? else {
        return Ok(None);
    };
    let raw_shift = decision
        .delta_position
        .checked_add(two_value.first_position())
        .and_then(|value| value.checked_sub(one_value.first_position()))
        .ok_or(ClusterMergeError::PositionOverflow)?;

    let mut next_ownership = ownership.clone();
    let mut next_clusters = clusters.clone();
    merge_cluster_values(&mut next_ownership, &mut next_clusters, two, one, raw_shift)?;
    *ownership = next_ownership;
    *clusters = next_clusters;
    Ok(Some(decision))
}

fn deskew_points(points: Vec<Option<(f64, f64)>>, slope: f64) -> Vec<Option<f64>> {
    points
        .into_iter()
        .map(|point| point.map(|(x, y)| deskew_ordinate(x, y, slope)))
        .collect()
}

fn deskew_complete_points(points: &[(f64, f64)], slope: f64) -> Vec<Option<f64>> {
    points
        .iter()
        .map(|&(x, y)| Some(deskew_ordinate(x, y, slope)))
        .collect()
}

fn deskew_ordinate(x: f64, y: f64, slope: f64) -> f64 {
    // Follow Java Skew's atan -> rotation construction rather than replacing
    // it with an algebraic identity that can round differently by one ULP.
    let deskew_angle = -slope.atan();
    (x * deskew_angle.sin()) + (y * deskew_angle.cos())
}

fn collision_is_clear(
    one: &LineCluster,
    two: &LineCluster,
    delta: i32,
    parameters: ClusterMergeParameters,
) -> Result<bool, ClusterMergeError> {
    let one_lines = one.lines().map(|(_, line)| line).collect::<Vec<_>>();
    let two_lines = two.lines().map(|(_, line)| line).collect::<Vec<_>>();
    for (one_index, one_line) in one_lines.into_iter().enumerate() {
        let two_index = one_index as i64 + i64::from(delta);
        if two_index < 0 || two_index >= two_lines.len() as i64 {
            continue;
        }
        let two_line = two_lines[two_index as usize];
        let one_bounds = one_line.filament().bounds()?;
        let two_bounds = two_line.filament().bounds()?;
        let left = one_bounds.x.max(two_bounds.x);
        let right = (one_bounds.x + one_bounds.width - 1).min(two_bounds.x + two_bounds.width - 1);
        if left <= right {
            let middle = left + ((right - left) / 2);
            if combined_thickness_at(
                middle,
                parameters.probe_width,
                [one_line.filament(), two_line.filament()],
            )? > parameters.maximum_foreground as f64
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClusterMergeError {
    InvalidParameters,
    PositionOverflow,
    MissingClusterValue(ClusterId),
    Cluster(LineClusterError),
    Filament(FilamentError),
    Ownership(ClusterOwnershipError),
    Coordinator(RecursiveIncludeError),
}

impl From<LineClusterError> for ClusterMergeError {
    fn from(value: LineClusterError) -> Self {
        Self::Cluster(value)
    }
}

impl From<ClusterOwnershipError> for ClusterMergeError {
    fn from(value: ClusterOwnershipError) -> Self {
        Self::Ownership(value)
    }
}

impl From<FilamentError> for ClusterMergeError {
    fn from(value: FilamentError) -> Self {
        Self::Filament(value)
    }
}

impl From<RecursiveIncludeError> for ClusterMergeError {
    fn from(value: RecursiveIncludeError) -> Self {
        Self::Coordinator(value)
    }
}

impl fmt::Display for ClusterMergeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParameters => formatter.write_str("cluster merge parameters are invalid"),
            Self::PositionOverflow => formatter.write_str("cluster merge position overflow"),
            Self::MissingClusterValue(id) => {
                write!(formatter, "missing cluster value {}", id.value())
            }
            Self::Cluster(error) => write!(formatter, "cluster merge geometry error: {error}"),
            Self::Filament(error) => write!(formatter, "cluster merge filament error: {error}"),
            Self::Ownership(error) => write!(formatter, "cluster merge ownership error: {error}"),
            Self::Coordinator(error) => {
                write!(formatter, "cluster merge coordinator error: {error}")
            }
        }
    }
}

impl Error for ClusterMergeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        filament::StaffFilament,
        line_cluster::FilamentId,
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections},
    };

    fn filament(x: usize, y: usize, length: usize) -> StaffFilament {
        let mut table = RunTable::new(Orientation::Horizontal, x + length + 1, y + 2).unwrap();
        table.add_run(y, Run::new(x, length)).unwrap();
        let mut filament = StaffFilament::new(10).unwrap();
        filament
            .add_section(build_sections(&table, JunctionPolicy::All).remove(0))
            .unwrap();
        filament
    }

    fn parameters(
        maximum_gap: i64,
        maximum_distance: f64,
        maximum_foreground: usize,
    ) -> ClusterMergeParameters {
        ClusterMergeParameters::new(0.0, maximum_gap, maximum_distance, 3, 4, maximum_foreground)
            .unwrap()
    }

    #[test]
    fn best_match_keeps_first_negative_delta_on_distance_tie() {
        assert_eq!(
            best_ordinate_match(&[Some(0.0), Some(2.0)], &[Some(1.0)]),
            Some((-1, 1.0))
        );
        assert_eq!(best_ordinate_match(&[], &[Some(1.0)]), None);
        assert_eq!(best_ordinate_match(&[None], &[Some(1.0)]), None);
    }

    #[test]
    fn endpoint_branch_includes_distance_edge_and_one_line_gap_exception() {
        let one = LineCluster::new(10, FilamentId::new(1), filament(0, 10, 60)).unwrap();
        let two = LineCluster::new(10, FilamentId::new(2), filament(100, 12, 60)).unwrap();
        let decision = cluster_merge_compatibility(&one, &two, parameters(0, 2.0, 1))
            .unwrap()
            .unwrap();
        assert_eq!(decision.delta_position(), 0);
        assert_eq!(decision.mean_distance(), 2.0);
        assert_eq!(decision.measurement(), MergeMeasurement::Endpoints);
        assert!(
            cluster_merge_compatibility(&one, &two, parameters(0, 1.99, 1))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn multiline_gap_limit_is_inclusive_and_rejects_next_pixel() {
        let mut one = LineCluster::new(10, FilamentId::new(1), filament(0, 10, 60)).unwrap();
        one.include_line(1, FilamentId::new(2), filament(0, 20, 60))
            .unwrap();
        let at_limit = LineCluster::new(10, FilamentId::new(3), filament(62, 10, 60)).unwrap();
        let beyond = LineCluster::new(10, FilamentId::new(4), filament(63, 10, 60)).unwrap();
        assert!(
            cluster_merge_compatibility(&one, &at_limit, parameters(3, 0.0, 1))
                .unwrap()
                .is_some()
        );
        assert!(
            cluster_merge_compatibility(&one, &beyond, parameters(3, 0.0, 1))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn overlap_collision_accepts_foreground_edge_and_rejects_above_it() {
        let mut one = LineCluster::new(10, FilamentId::new(1), filament(0, 10, 60)).unwrap();
        one.include_line(1, FilamentId::new(2), filament(0, 20, 60))
            .unwrap();
        let mut two = LineCluster::new(10, FilamentId::new(3), filament(0, 11, 60)).unwrap();
        two.include_line(1, FilamentId::new(4), filament(0, 21, 60))
            .unwrap();

        let accepted = cluster_merge_compatibility(&one, &two, parameters(0, 1.0, 2))
            .unwrap()
            .unwrap();
        assert_eq!(accepted.measurement(), MergeMeasurement::Overlap);
        assert_eq!(accepted.mean_distance(), 1.0);
        assert!(
            cluster_merge_compatibility(&one, &two, parameters(0, 1.0, 1))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn accepted_pair_merges_one_into_two_with_registry_transfer() {
        let mut table = RunTable::new(Orientation::Horizontal, 150, 14).unwrap();
        table.add_run(10, Run::new(0, 60)).unwrap();
        table.add_run(12, Run::new(80, 60)).unwrap();
        let mut sections = build_sections(&table, JunctionPolicy::All);
        let one_id = FilamentId::new(1);
        let two_id = FilamentId::new(2);
        let mut one_filament = StaffFilament::new(10).unwrap();
        one_filament.add_section(sections.remove(0)).unwrap();
        let swallowed_section = one_filament.sections()[0].id();
        let mut two_filament = StaffFilament::new(10).unwrap();
        two_filament.add_section(sections.remove(0)).unwrap();

        let mut ownership = ClusterOwnership::new();
        ownership.register_filament(one_id, &one_filament).unwrap();
        ownership.register_filament(two_id, &two_filament).unwrap();
        let one = ownership.register_cluster(one_id).unwrap();
        let two = ownership.register_cluster(two_id).unwrap();
        let mut clusters = BTreeMap::from([
            (one, LineCluster::new(10, one_id, one_filament).unwrap()),
            (two, LineCluster::new(10, two_id, two_filament).unwrap()),
        ]);

        let decision = merge_cluster_pair_if_compatible(
            &mut ownership,
            &mut clusters,
            one,
            two,
            parameters(0, 2.0, 1),
        )
        .unwrap()
        .unwrap();
        assert_eq!(decision.delta_position(), 0);
        assert_eq!(clusters.len(), 1);
        assert_eq!(ownership.cluster_parent(one).unwrap(), Some(two));
        assert_eq!(ownership.filament_parent(one_id).unwrap(), Some(two_id));
        assert_eq!(ownership.section_owner(swallowed_section), Some(two_id));
        assert_eq!(clusters[&two].line_at(0).unwrap().absorbed_ids(), &[one_id]);
    }
}
