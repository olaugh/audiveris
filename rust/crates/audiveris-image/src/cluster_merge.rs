// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light merge compatibility from Java `ClustersRetriever.canMerge`.
//!
//! Sheet-owned scale fractions and skew are resolved into numeric parameters.
//! The kernel returns Java's directional delta (`one` line index to `two` line
//! index); the adapter transactionally merges `one` into `two` through the
//! neutral ownership coordinator.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

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

    #[must_use]
    pub const fn global_slope(self) -> f64 {
        self.global_slope
    }

    #[must_use]
    pub const fn maximum_coordinate_gap(self) -> i64 {
        self.maximum_coordinate_gap
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClusterMergePassParameters {
    compatibility: ClusterMergeParameters,
    single_candidate_margin: i64,
    candidate_vertical_margin: i64,
}

impl ClusterMergePassParameters {
    pub fn new(
        compatibility: ClusterMergeParameters,
        single_candidate_margin: i64,
        candidate_vertical_margin: i64,
    ) -> Result<Self, ClusterMergeError> {
        if single_candidate_margin < 0 || candidate_vertical_margin < 0 {
            return Err(ClusterMergeError::InvalidPassParameters);
        }
        Ok(Self {
            compatibility,
            single_candidate_margin,
            candidate_vertical_margin,
        })
    }
}

/// Resolved, sheet-independent inputs to Java `mergeClusterPairs`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClusterPairPassParameters {
    global_slope: f64,
    maximum_center_distance: i64,
    maximum_horizontal_gap: i64,
    minimum_length: f64,
}

impl ClusterPairPassParameters {
    pub fn new(
        global_slope: f64,
        maximum_center_distance: i64,
        maximum_horizontal_gap: i64,
        minimum_length: f64,
    ) -> Result<Self, ClusterMergeError> {
        if !global_slope.is_finite()
            || maximum_center_distance < 0
            || maximum_horizontal_gap < 0
            || !minimum_length.is_finite()
            || minimum_length < 0.0
        {
            return Err(ClusterMergeError::InvalidPairPassParameters);
        }
        Ok(Self {
            global_slope,
            maximum_center_distance,
            maximum_horizontal_gap,
            minimum_length,
        })
    }
}

/// Active and short-isolated cluster IDs after `mergeClusterPairs`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterPairPassResult {
    survivors: Vec<ClusterId>,
    discarded: Vec<ClusterId>,
}

impl ClusterPairPassResult {
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

/// Java `mergeClusters`: sort once, then let each candidate repeatedly absorb
/// compatible earlier heads until a complete rescan finds no merge.
///
/// `cluster_order` is the caller's current list order and therefore controls
/// stable ordinate ties. This pass deliberately does not require equal cluster
/// sizes; Java applies that restriction only in the later `mergeClusterPairs`.
/// The full pass commits atomically.
pub fn merge_clusters_in_order(
    ownership: &mut ClusterOwnership,
    clusters: &mut BTreeMap<ClusterId, LineCluster>,
    cluster_order: &[ClusterId],
    parameters: ClusterMergePassParameters,
) -> Result<Vec<ClusterId>, ClusterMergeError> {
    let mut seen = BTreeSet::new();
    let mut ordered = cluster_order
        .iter()
        .copied()
        .map(|id| {
            if !seen.insert(id) {
                return Err(ClusterMergeError::DuplicateClusterOrder(id));
            }
            let cluster = clusters
                .get(&id)
                .ok_or(ClusterMergeError::MissingClusterValue(id))?;
            Ok((
                id,
                cluster_deskewed_ordinate(cluster, parameters.compatibility.global_slope)?,
            ))
        })
        .collect::<Result<Vec<_>, ClusterMergeError>>()?;
    // Stable sort exactly preserves caller order when Java's comparator returns
    // zero. A non-finite ordinate would compare equal in the source comparator.
    ordered.sort_by(|one, two| one.1.partial_cmp(&two.1).unwrap_or(Ordering::Equal));

    let mut next_ownership = ownership.clone();
    let mut next_clusters = clusters.clone();
    for current_index in 0..ordered.len() {
        let current = ordered[current_index].0;
        let Some(initial) = next_clusters.get(&current) else {
            continue;
        };
        let horizontal_margin = if initial.is_one_line() {
            parameters.single_candidate_margin
        } else {
            parameters.compatibility.maximum_coordinate_gap
        };

        loop {
            let candidate = next_clusters
                .get(&current)
                .ok_or(ClusterMergeError::MissingClusterValue(current))?;
            let candidate_bounds = candidate.bounds()?;
            let mut merged = false;
            for &(head, _) in &ordered[..current_index] {
                if !next_clusters.contains_key(&head)
                    || next_ownership.cluster_parent(head)?.is_some()
                {
                    continue;
                }
                let head_bounds = next_clusters[&head].bounds()?;
                if !intersects_grown(
                    candidate_bounds,
                    head_bounds,
                    horizontal_margin,
                    parameters.candidate_vertical_margin,
                ) {
                    continue;
                }
                if merge_cluster_pair_if_compatible(
                    &mut next_ownership,
                    &mut next_clusters,
                    head,
                    current,
                    parameters.compatibility,
                )?
                .is_some()
                {
                    merged = true;
                    break;
                }
            }
            if !merged {
                break;
            }
        }
    }

    let mut survivors = Vec::new();
    for (id, _) in ordered {
        if next_clusters.contains_key(&id) && next_ownership.cluster_parent(id)?.is_none() {
            survivors.push(id);
        }
    }
    *ownership = next_ownership;
    *clusters = next_clusters;
    Ok(survivors)
}

/// Java `mergeClusterPairs`: merge nearby clusters of exactly the same size,
/// then remove short isolated clusters from the active value set.
///
/// The list is stably sorted once by deskewed ordinate. A successful merge
/// restarts at the same index with fresh bounds and center limits, allowing a
/// horizontal chain to become reachable as the destination grows. As in Java,
/// unequal-size entries are skipped before the vertical-limit check. The first
/// cluster absorbs the later cluster with line-index delta zero.
///
/// Java removes a short cluster from the active list without calling
/// `LineCluster.destroy`; correspondingly, its neutral ownership records remain
/// registered while its value is removed from `clusters`. The whole pass is
/// transactional if validation or a merge fails.
pub fn merge_cluster_pairs_in_order(
    ownership: &mut ClusterOwnership,
    clusters: &mut BTreeMap<ClusterId, LineCluster>,
    cluster_order: &[ClusterId],
    parameters: ClusterPairPassParameters,
) -> Result<ClusterPairPassResult, ClusterMergeError> {
    let mut seen = BTreeSet::new();
    let mut ordered = cluster_order
        .iter()
        .copied()
        .map(|id| {
            if !seen.insert(id) {
                return Err(ClusterMergeError::DuplicateClusterOrder(id));
            }
            let cluster = clusters
                .get(&id)
                .ok_or(ClusterMergeError::MissingClusterValue(id))?;
            Ok((
                id,
                cluster_deskewed_ordinate(cluster, parameters.global_slope)?,
            ))
        })
        .collect::<Result<Vec<_>, ClusterMergeError>>()?;
    ordered.sort_by(|one, two| one.1.partial_cmp(&two.1).unwrap_or(Ordering::Equal));

    let mut next_ownership = ownership.clone();
    let mut next_clusters = clusters.clone();
    let mut discarded = Vec::new();
    let mut index = 0;
    while index < ordered.len() {
        let current = ordered[index].0;
        let cluster = next_clusters
            .get(&current)
            .ok_or(ClusterMergeError::MissingClusterValue(current))?;
        let cluster_bounds = cluster.bounds()?;
        let maximum_ordinate = cluster_deskewed_ordinate(cluster, parameters.global_slope)?
            + parameters.maximum_center_distance as f64;
        let cluster_size = cluster.size();

        let mut merged = false;
        let mut candidate_index = index + 1;
        while candidate_index < ordered.len() {
            let candidate_id = ordered[candidate_index].0;
            let candidate = next_clusters
                .get(&candidate_id)
                .ok_or(ClusterMergeError::MissingClusterValue(candidate_id))?;
            if candidate.size() != cluster_size {
                candidate_index += 1;
                continue;
            }
            if cluster_deskewed_ordinate(candidate, parameters.global_slope)? > maximum_ordinate {
                break;
            }
            if horizontal_gap(cluster_bounds, candidate.bounds()?)
                > parameters.maximum_horizontal_gap
            {
                candidate_index += 1;
                continue;
            }

            let raw_shift = cluster
                .first_position()
                .checked_sub(candidate.first_position())
                .ok_or(ClusterMergeError::PositionOverflow)?;
            merge_cluster_values(
                &mut next_ownership,
                &mut next_clusters,
                current,
                candidate_id,
                raw_shift,
            )?;
            ordered.remove(candidate_index);
            merged = true;
            break;
        }
        if merged {
            continue;
        }

        let cluster = next_clusters
            .get(&current)
            .ok_or(ClusterMergeError::MissingClusterValue(current))?;
        if (cluster.true_length()? as f64) < parameters.minimum_length {
            next_clusters.remove(&current);
            ordered.remove(index);
            discarded.push(current);
        } else {
            index += 1;
        }
    }

    let survivors = ordered.into_iter().map(|(id, _)| id).collect();
    *ownership = next_ownership;
    *clusters = next_clusters;
    Ok(ClusterPairPassResult {
        survivors,
        discarded,
    })
}

pub fn cluster_deskewed_ordinate(
    cluster: &LineCluster,
    global_slope: f64,
) -> Result<f64, ClusterMergeError> {
    if !global_slope.is_finite() {
        return Err(ClusterMergeError::InvalidParameters);
    }
    let bounds = cluster.bounds()?;
    // Java LineCluster.getCenter uses integer division before deskewing.
    let x = bounds.x + (bounds.width / 2);
    let y = bounds.y + (bounds.height / 2);
    Ok(deskew_ordinate(x as f64, y as f64, global_slope))
}

fn intersects_grown(
    candidate: crate::section::Bounds,
    head: crate::section::Bounds,
    horizontal_margin: i64,
    vertical_margin: i64,
) -> bool {
    let candidate_left = candidate.x as i64 - horizontal_margin;
    let candidate_right = (candidate.x + candidate.width) as i64 + horizontal_margin;
    let candidate_top = candidate.y as i64 - vertical_margin;
    let candidate_bottom = (candidate.y + candidate.height) as i64 + vertical_margin;
    let head_left = head.x as i64;
    let head_right = (head.x + head.width) as i64;
    let head_top = head.y as i64;
    let head_bottom = (head.y + head.height) as i64;
    candidate_left < head_right
        && head_left < candidate_right
        && candidate_top < head_bottom
        && head_top < candidate_bottom
}

fn horizontal_gap(one: crate::section::Bounds, two: crate::section::Bounds) -> i64 {
    let common_left = one.x.max(two.x) as i64;
    let common_right = (one.x + one.width).min(two.x + two.width) as i64;
    common_left - common_right
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
    InvalidPassParameters,
    InvalidPairPassParameters,
    PositionOverflow,
    MissingClusterValue(ClusterId),
    DuplicateClusterOrder(ClusterId),
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
            Self::InvalidPassParameters => {
                formatter.write_str("cluster merge pass parameters are invalid")
            }
            Self::InvalidPairPassParameters => {
                formatter.write_str("cluster pair pass parameters are invalid")
            }
            Self::PositionOverflow => formatter.write_str("cluster merge position overflow"),
            Self::MissingClusterValue(id) => {
                write!(formatter, "missing cluster value {}", id.value())
            }
            Self::DuplicateClusterOrder(id) => {
                write!(formatter, "duplicate cluster {} in merge order", id.value())
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

    fn cluster_state(
        specs: &[(u64, usize, usize)],
    ) -> (
        ClusterOwnership,
        BTreeMap<ClusterId, LineCluster>,
        Vec<ClusterId>,
    ) {
        let specs = specs
            .iter()
            .map(|&(id, x, y)| (id, x, y, 60))
            .collect::<Vec<_>>();
        cluster_state_with_lengths(&specs)
    }

    fn cluster_state_with_lengths(
        specs: &[(u64, usize, usize, usize)],
    ) -> (
        ClusterOwnership,
        BTreeMap<ClusterId, LineCluster>,
        Vec<ClusterId>,
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
        let mut clusters = BTreeMap::new();
        let mut order = Vec::new();
        for &(value, x, y, length) in specs {
            let section_index = sections
                .iter()
                .position(|section| {
                    let bounds = section.bounds();
                    bounds.x == x && bounds.y == y && bounds.width == length
                })
                .unwrap();
            let mut filament = StaffFilament::new(10).unwrap();
            filament
                .add_section(sections.remove(section_index))
                .unwrap();
            let filament_id = FilamentId::new(value);
            ownership.register_filament(filament_id, &filament).unwrap();
            let cluster_id = ownership.register_cluster(filament_id).unwrap();
            clusters.insert(
                cluster_id,
                LineCluster::new(10, filament_id, filament).unwrap(),
            );
            order.push(cluster_id);
        }
        (ownership, clusters, order)
    }

    fn pass_parameters(
        regular_margin: i64,
        single_margin: i64,
        maximum_distance: f64,
    ) -> ClusterMergePassParameters {
        ClusterMergePassParameters::new(
            parameters(regular_margin, maximum_distance, 1),
            single_margin,
            0,
        )
        .unwrap()
    }

    fn pair_parameters(
        maximum_center_distance: i64,
        maximum_horizontal_gap: i64,
        minimum_length: f64,
    ) -> ClusterPairPassParameters {
        ClusterPairPassParameters::new(
            0.0,
            maximum_center_distance,
            maximum_horizontal_gap,
            minimum_length,
        )
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

    #[test]
    fn merge_pass_restarts_after_growth_and_preserves_stable_tie_order() {
        let (mut ownership, mut clusters, order) =
            cluster_state(&[(1, 0, 10), (2, 150, 10), (3, 75, 10)]);
        let survivor = ClusterId::from_seed(FilamentId::new(3));

        let survivors = merge_clusters_in_order(
            &mut ownership,
            &mut clusters,
            &order,
            pass_parameters(0, 20, 0.0),
        )
        .unwrap();

        assert_eq!(survivors, [survivor]);
        assert_eq!(clusters.len(), 1);
        assert_eq!(
            clusters[&survivor].line_at(0).unwrap().absorbed_ids(),
            &[FilamentId::new(1), FilamentId::new(2)]
        );
        assert_eq!(ownership.cluster_parent(order[0]).unwrap(), Some(survivor));
        assert_eq!(ownership.cluster_parent(order[1]).unwrap(), Some(survivor));
    }

    #[test]
    fn merge_pass_candidate_growth_requires_positive_rectangle_intersection() {
        let (mut ownership, mut clusters, order) = cluster_state(&[(1, 0, 10), (2, 80, 10)]);
        let survivors = merge_clusters_in_order(
            &mut ownership,
            &mut clusters,
            &order,
            pass_parameters(0, 20, 0.0),
        )
        .unwrap();
        assert_eq!(survivors, order);
        assert_eq!(clusters.len(), 2);

        let (mut ownership, mut clusters, order) = cluster_state(&[(1, 0, 10), (2, 80, 10)]);
        let survivors = merge_clusters_in_order(
            &mut ownership,
            &mut clusters,
            &order,
            pass_parameters(0, 21, 0.0),
        )
        .unwrap();
        assert_eq!(survivors, [order[1]]);
        assert_eq!(clusters.len(), 1);
    }

    #[test]
    fn merge_pass_allows_different_sizes_and_sorts_by_deskewed_ordinate() {
        let (mut ownership, mut clusters, mut order) =
            cluster_state(&[(2, 80, 10), (1, 0, 10), (3, 80, 20)]);
        let large = ClusterId::from_seed(FilamentId::new(2));
        let extra = ClusterId::from_seed(FilamentId::new(3));
        let extra_value = clusters.remove(&extra).unwrap();
        clusters
            .get_mut(&large)
            .unwrap()
            .merge_with_shift(extra_value, 1)
            .unwrap();
        ownership.merge_clusters(large, extra, 1).unwrap();
        order.retain(|id| *id != extra);

        let survivors = merge_clusters_in_order(
            &mut ownership,
            &mut clusters,
            &order,
            pass_parameters(30, 0, 0.0),
        )
        .unwrap();

        assert_eq!(survivors, [large]);
        assert_eq!(clusters[&large].size(), 2);
        assert_eq!(ownership.cluster_parent(order[1]).unwrap(), Some(large));
    }

    #[test]
    fn pair_pass_restarts_after_growth_and_keeps_first_stable_tie() {
        let (mut ownership, mut clusters, order) =
            cluster_state(&[(1, 0, 10), (2, 150, 10), (3, 75, 10)]);

        let result = merge_cluster_pairs_in_order(
            &mut ownership,
            &mut clusters,
            &order,
            pair_parameters(0, 20, 0.0),
        )
        .unwrap();

        assert_eq!(result.survivors(), [order[0]]);
        assert!(result.discarded().is_empty());
        assert_eq!(clusters.len(), 1);
        assert_eq!(
            clusters[&order[0]].line_at(0).unwrap().absorbed_ids(),
            &[FilamentId::new(3), FilamentId::new(2)]
        );
        assert_eq!(ownership.cluster_parent(order[1]).unwrap(), Some(order[0]));
        assert_eq!(ownership.cluster_parent(order[2]).unwrap(), Some(order[0]));
    }

    #[test]
    fn pair_pass_uses_inclusive_gap_and_center_limits() {
        let (mut ownership, mut clusters, order) =
            cluster_state(&[(1, 0, 10), (2, 80, 12)]);
        let result = merge_cluster_pairs_in_order(
            &mut ownership,
            &mut clusters,
            &order,
            pair_parameters(2, 20, 0.0),
        )
        .unwrap();
        assert_eq!(result.survivors(), [order[0]]);

        let (mut ownership, mut clusters, order) =
            cluster_state(&[(1, 0, 10), (2, 80, 13)]);
        let result = merge_cluster_pairs_in_order(
            &mut ownership,
            &mut clusters,
            &order,
            pair_parameters(2, 20, 0.0),
        )
        .unwrap();
        assert_eq!(result.survivors(), order);
    }

    #[test]
    fn pair_pass_skips_other_sizes_and_breaks_at_first_far_equal_size() {
        let (mut ownership, mut clusters, mut order) =
            cluster_state(&[(1, 0, 10), (2, 0, 12), (3, 0, 20), (4, 0, 14)]);
        let different = order[1];
        let extra = order[3];
        let extra_value = clusters.remove(&extra).unwrap();
        clusters
            .get_mut(&different)
            .unwrap()
            .merge_with_shift(extra_value, 1)
            .unwrap();
        ownership.merge_clusters(different, extra, 1).unwrap();
        order.retain(|id| *id != extra);

        let result = merge_cluster_pairs_in_order(
            &mut ownership,
            &mut clusters,
            &order,
            pair_parameters(5, 100, 0.0),
        )
        .unwrap();

        // C1 first sees the far, equal-size C3 and breaks. The two-line C2 is
        // skipped by the size test even though its box overlaps horizontally.
        assert_eq!(result.survivors().len(), 3);
        assert_eq!(clusters.len(), 3);
        assert_eq!(clusters[&different].size(), 2);
    }

    #[test]
    fn pair_pass_discards_only_unmerged_clusters_strictly_below_length() {
        let (mut ownership, mut clusters, order) = cluster_state_with_lengths(&[
            (1, 0, 10, 19),
            (2, 100, 20, 20),
            (3, 121, 20, 10),
        ]);

        let result = merge_cluster_pairs_in_order(
            &mut ownership,
            &mut clusters,
            &order,
            pair_parameters(0, 1, 20.0),
        )
        .unwrap();

        assert_eq!(result.discarded(), [order[0]]);
        assert_eq!(result.survivors(), [order[1]]);
        assert!(!clusters.contains_key(&order[0]));
        assert_eq!(clusters[&order[1]].true_length().unwrap(), 29);
        // Java's list-only short-cluster removal leaves its ownership backlink.
        assert_eq!(
            ownership
                .membership_of(FilamentId::new(1))
                .unwrap()
                .unwrap()
                .cluster(),
            order[0]
        );
    }
}
