// SPDX-License-Identifier: AGPL-3.0-or-later

//! Transactional comb-network following and recursive cluster inclusion.
//!
//! The caller supplies explicit comb snapshots and current unclustered filament
//! values. `ClusterOwnership` supplies all mutable back-references; `LineCluster`
//! remains the geometric value. This module first supports Java
//! `ClustersRetriever.followCombsNetwork`, which joins fragments occupying the
//! same relative line before cluster creation, and then the recursive
//! `LineCluster.include` traversal. Sheet ordering, skew-based candidate
//! selection, cluster consistency policy, glyph ownership, and SIG integration
//! stay outside.

use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use audiveris_core::histogram::Histogram;

use crate::{
    cluster_ownership::{ClusterId, ClusterOwnership, ClusterOwnershipError, CombId},
    filament::{FilamentError, StaffFilament},
    filament_comb::FilamentComb,
    line_cluster::{FilamentId, LineCluster, LineClusterError},
};

/// Mutable processing state of one immutable comb membership snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecursiveCombSnapshot {
    members: Vec<FilamentId>,
    processed: bool,
}

impl RecursiveCombSnapshot {
    /// Capture Java comb append order and current processed state.
    #[must_use]
    pub fn from_comb(comb: &FilamentComb) -> Self {
        Self {
            members: comb
                .filament_ids()
                .iter()
                .map(|&id| FilamentId::new(id as u64))
                .collect(),
            processed: comb.is_processed(),
        }
    }

    #[must_use]
    pub fn members(&self) -> &[FilamentId] {
        &self.members
    }

    #[must_use]
    pub const fn is_processed(&self) -> bool {
        self.processed
    }
}

/// Follow Java's comb network and merge fragments that represent the same
/// relative staff line.
///
/// `filament_order` is Java's `allFilaments` list after factory merging. It is
/// traversed in caller order, while every filament's reverse comb map is
/// traversed by ascending column. Within a comb, append order is retained.
/// When two current ancestors occupy the same relative line, the longer one
/// wins; the first-seen ancestor wins an exact length tie (`>=` in Java).
///
/// The supplied states are committed together only after the whole traversal
/// succeeds. On success, swallowed values remain addressable in `filaments`, as
/// they do through Java's global filament index, while `filament_order` retains
/// only current roots, matching `removeMergedFilaments(allFilaments)`.
pub fn follow_combs_network(
    ownership: &mut ClusterOwnership,
    filaments: &mut BTreeMap<FilamentId, StaffFilament>,
    combs: &BTreeMap<CombId, RecursiveCombSnapshot>,
    filament_order: &mut Vec<FilamentId>,
) -> Result<(), FollowCombsNetworkError> {
    let mut next_ownership = ownership.clone();
    let mut next_filaments = filaments.clone();
    let mut next_order = filament_order.clone();
    follow_combs_network_inner(
        &mut next_ownership,
        &mut next_filaments,
        combs,
        &mut next_order,
    )?;
    *ownership = next_ownership;
    *filaments = next_filaments;
    *filament_order = next_order;
    Ok(())
}

fn follow_combs_network_inner(
    ownership: &mut ClusterOwnership,
    filaments: &mut BTreeMap<FilamentId, StaffFilament>,
    combs: &BTreeMap<CombId, RecursiveCombSnapshot>,
    filament_order: &mut Vec<FilamentId>,
) -> Result<(), FollowCombsNetworkError> {
    let mut seen = BTreeSet::new();
    let mut direct_combs = BTreeMap::new();
    for &filament in filament_order.iter() {
        if !seen.insert(filament) {
            return Err(FollowCombsNetworkError::DuplicateFilamentInput(filament));
        }
        let ancestor = ownership.filament_ancestor(filament)?;
        if ancestor != filament {
            return Err(FollowCombsNetworkError::NonRootFilamentInput { filament, ancestor });
        }
        if !filaments.contains_key(&filament) {
            return Err(FollowCombsNetworkError::MissingFilamentValue(filament));
        }
        direct_combs.insert(filament, ownership.combs_of(filament)?.clone());
    }

    // Keep direct per-object maps locally. Java leaves a swallowed object's
    // map intact, but a winning object receives putAll from every swallowed
    // root. ClusterOwnership intentionally exposes only the resolved root map.
    for &pivot in filament_order.iter() {
        let pivot_combs = direct_combs
            .get(&pivot)
            .ok_or(FollowCombsNetworkError::MissingFilamentInput(pivot))?
            .values()
            .copied()
            .collect::<Vec<_>>();
        let mut relative_lines = BTreeMap::<i32, FilamentId>::new();

        for comb_id in pivot_combs {
            let snapshot = combs
                .get(&comb_id)
                .ok_or(FollowCombsNetworkError::MissingCombSnapshot(comb_id))?;
            let pivot_ancestor = ownership.filament_ancestor(pivot)?;
            let mut pivot_index = None;
            for (index, &member) in snapshot.members.iter().enumerate() {
                if ownership.filament_ancestor(member)? == pivot_ancestor {
                    pivot_index = Some(index);
                    break;
                }
            }
            let pivot_index = pivot_index.ok_or(FollowCombsNetworkError::PivotMissingFromComb {
                pivot,
                comb: comb_id,
            })?;
            let pivot_index = i32::try_from(pivot_index)
                .map_err(|_| FollowCombsNetworkError::PositionOverflow)?;

            for (index, &member) in snapshot.members.iter().enumerate() {
                let index =
                    i32::try_from(index).map_err(|_| FollowCombsNetworkError::PositionOverflow)?;
                let relative = index
                    .checked_sub(pivot_index)
                    .ok_or(FollowCombsNetworkError::PositionOverflow)?;
                if relative == 0 {
                    continue;
                }

                if let Some(&incumbent) = relative_lines.get(&relative) {
                    connect_filament_ancestors(
                        ownership,
                        filaments,
                        &mut direct_combs,
                        incumbent,
                        member,
                    )?;
                } else {
                    relative_lines.insert(relative, member);
                }
            }
        }
    }

    let mut roots = Vec::with_capacity(filament_order.len());
    for &filament in filament_order.iter() {
        if ownership.filament_parent(filament)?.is_none() {
            roots.push(filament);
        }
    }
    *filament_order = roots;
    Ok(())
}

fn connect_filament_ancestors(
    ownership: &mut ClusterOwnership,
    filaments: &mut BTreeMap<FilamentId, StaffFilament>,
    direct_combs: &mut BTreeMap<FilamentId, BTreeMap<i32, CombId>>,
    one: FilamentId,
    two: FilamentId,
) -> Result<(), FollowCombsNetworkError> {
    let one = ownership.filament_ancestor(one)?;
    let two = ownership.filament_ancestor(two)?;
    if one == two {
        return Ok(());
    }

    let one_length = filaments
        .get(&one)
        .ok_or(FollowCombsNetworkError::MissingFilamentValue(one))?
        .bounds()?
        .width;
    let two_length = filaments
        .get(&two)
        .ok_or(FollowCombsNetworkError::MissingFilamentValue(two))?
        .bounds()?
        .width;
    // Java uses >=, so the incumbent (`one`) wins an exact tie.
    let (winner, swallowed) = if one_length >= two_length {
        (one, two)
    } else {
        (two, one)
    };

    let swallowed_value = filaments
        .get(&swallowed)
        .cloned()
        .ok_or(FollowCombsNetworkError::MissingFilamentValue(swallowed))?;
    let mut winner_value = filaments
        .get(&winner)
        .cloned()
        .ok_or(FollowCombsNetworkError::MissingFilamentValue(winner))?;
    for section in swallowed_value.sections().iter().cloned() {
        winner_value.add_section(section)?;
    }

    let swallowed_combs = direct_combs
        .get(&swallowed)
        .cloned()
        .ok_or(FollowCombsNetworkError::MissingFilamentInput(swallowed))?;
    direct_combs
        .get_mut(&winner)
        .ok_or(FollowCombsNetworkError::MissingFilamentInput(winner))?
        .extend(swallowed_combs);
    ownership.merge_filaments(winner, swallowed)?;
    filaments.insert(winner, winner_value);
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FollowCombsNetworkError {
    Ownership(ClusterOwnershipError),
    Filament(FilamentError),
    DuplicateFilamentInput(FilamentId),
    NonRootFilamentInput {
        filament: FilamentId,
        ancestor: FilamentId,
    },
    MissingFilamentInput(FilamentId),
    MissingFilamentValue(FilamentId),
    MissingCombSnapshot(CombId),
    PivotMissingFromComb {
        pivot: FilamentId,
        comb: CombId,
    },
    PositionOverflow,
}

impl From<ClusterOwnershipError> for FollowCombsNetworkError {
    fn from(value: ClusterOwnershipError) -> Self {
        Self::Ownership(value)
    }
}

impl From<FilamentError> for FollowCombsNetworkError {
    fn from(value: FilamentError) -> Self {
        Self::Filament(value)
    }
}

impl fmt::Display for FollowCombsNetworkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ownership(error) => write!(formatter, "comb-network ownership error: {error}"),
            Self::Filament(error) => write!(formatter, "comb-network filament error: {error}"),
            Self::DuplicateFilamentInput(id) => {
                write!(formatter, "duplicate input filament {}", id.value())
            }
            Self::NonRootFilamentInput { filament, ancestor } => write!(
                formatter,
                "input filament {} already belongs to ancestor {}",
                filament.value(),
                ancestor.value()
            ),
            Self::MissingFilamentInput(id) => {
                write!(
                    formatter,
                    "filament {} is absent from input order",
                    id.value()
                )
            }
            Self::MissingFilamentValue(id) => {
                write!(formatter, "missing filament value {}", id.value())
            }
            Self::MissingCombSnapshot(id) => {
                write!(formatter, "missing comb snapshot {}", id.value())
            }
            Self::PivotMissingFromComb { pivot, comb } => write!(
                formatter,
                "pivot filament {} is absent from comb {}",
                pivot.value(),
                comb.value()
            ),
            Self::PositionOverflow => formatter.write_str("comb relative position overflow"),
        }
    }
}

impl Error for FollowCombsNetworkError {}

/// Observable result of Java `createClusters`' neutral batch boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterFormation {
    cluster_ids: Vec<ClusterId>,
    popular_comb_size: Option<usize>,
}

impl ClusterFormation {
    /// Surviving clusters in seed-creation order.
    #[must_use]
    pub fn cluster_ids(&self) -> &[ClusterId] {
        &self.cluster_ids
    }

    /// Java histogram winner, where every comb votes with its member count.
    #[must_use]
    pub const fn popular_comb_size(&self) -> Option<usize> {
        self.popular_comb_size
    }
}

/// Form all eligible seed clusters in Java's stable reverse-length order.
///
/// `seed_candidates` is the original filament-list order. Each candidate is
/// sorted by its own horizontal bounds width before current ancestry is
/// resolved, matching Java's sort-then-`getAncestor` sequence. A root already
/// clustered by an earlier recursive traversal is skipped. Unless
/// `allow_single` is set, roots without reverse comb membership are skipped.
/// The complete batch is transactional.
pub fn form_clusters_from_combs(
    ownership: &mut ClusterOwnership,
    clusters: &mut BTreeMap<ClusterId, LineCluster>,
    filaments: &BTreeMap<FilamentId, StaffFilament>,
    combs: &mut BTreeMap<CombId, RecursiveCombSnapshot>,
    seed_candidates: &[FilamentId],
    interline: usize,
    allow_single: bool,
) -> Result<ClusterFormation, RecursiveIncludeError> {
    let popular_comb_size = popular_snapshot_size(combs);
    let mut ordered = seed_candidates
        .iter()
        .copied()
        .map(|id| {
            let width = filaments
                .get(&id)
                .ok_or(RecursiveIncludeError::MissingFilamentValue(id))?
                .bounds()?
                .width;
            Ok((id, width))
        })
        .collect::<Result<Vec<_>, RecursiveIncludeError>>()?;
    // Rust's slice sort is stable, like Collections.sort. The explicit key
    // avoids inventing an ID tie-breaker Java does not have.
    ordered.sort_by_key(|(_, width)| Reverse(*width));

    let mut next_ownership = ownership.clone();
    let mut next_clusters = clusters.clone();
    let mut next_combs = combs.clone();
    let mut created = Vec::new();
    for (candidate, _) in ordered {
        let seed = next_ownership.filament_ancestor(candidate)?;
        if next_ownership.membership_of(seed)?.is_some() {
            continue;
        }
        if !allow_single && next_ownership.combs_of(seed)?.is_empty() {
            continue;
        }

        let seed_value = filaments
            .get(&seed)
            .cloned()
            .ok_or(RecursiveIncludeError::MissingFilamentValue(seed))?;
        let cluster_id = next_ownership.register_cluster(seed)?;
        let cluster = LineCluster::new(interline, seed, seed_value)?;
        if next_clusters.insert(cluster_id, cluster).is_some() {
            return Err(RecursiveIncludeError::DuplicateClusterValue(cluster_id));
        }
        include_from_combs(
            &mut next_ownership,
            &mut next_clusters,
            filaments,
            &mut next_combs,
            cluster_id,
            seed,
            0,
        )?;
        created.push(cluster_id);
    }

    // Java removes cluster objects that acquired a parent during recursive
    // inclusion while preserving the creation order of survivors.
    let mut survivors = Vec::with_capacity(created.len());
    for id in created {
        if next_clusters.contains_key(&id) && next_ownership.cluster_parent(id)?.is_none() {
            survivors.push(id);
        }
    }
    *ownership = next_ownership;
    *clusters = next_clusters;
    *combs = next_combs;
    Ok(ClusterFormation {
        cluster_ids: survivors,
        popular_comb_size,
    })
}

/// Java `retrievePopularSize` over explicit snapshots.
#[must_use]
pub fn popular_snapshot_size(combs: &BTreeMap<CombId, RecursiveCombSnapshot>) -> Option<usize> {
    let mut histogram = Histogram::default();
    for comb in combs.values() {
        let count = comb.members.len();
        histogram.increase_count(count, i32::try_from(count).unwrap_or(i32::MAX));
    }
    histogram.max_bucket().copied()
}

/// Recursively dispatch the pivot's comb network into a destination cluster.
///
/// All mutable inputs are cloned first and committed together only after the
/// traversal succeeds. `filaments` must contain the current value of every
/// unclustered filament ancestor reachable from the snapshots. Clustered values
/// are always read from `clusters`, so later recursive steps observe earlier
/// same-transaction section absorption.
pub fn include_from_combs(
    ownership: &mut ClusterOwnership,
    clusters: &mut BTreeMap<ClusterId, LineCluster>,
    filaments: &BTreeMap<FilamentId, StaffFilament>,
    combs: &mut BTreeMap<CombId, RecursiveCombSnapshot>,
    destination: ClusterId,
    pivot: FilamentId,
    pivot_position: i32,
) -> Result<(), RecursiveIncludeError> {
    let mut next_ownership = ownership.clone();
    let mut next_clusters = clusters.clone();
    let mut next_combs = combs.clone();
    include_recursive(
        &mut next_ownership,
        &mut next_clusters,
        filaments,
        &mut next_combs,
        destination,
        pivot,
        pivot_position,
    )?;
    *ownership = next_ownership;
    *clusters = next_clusters;
    *combs = next_combs;
    Ok(())
}

fn include_recursive(
    ownership: &mut ClusterOwnership,
    clusters: &mut BTreeMap<ClusterId, LineCluster>,
    filaments: &BTreeMap<FilamentId, StaffFilament>,
    combs: &mut BTreeMap<CombId, RecursiveCombSnapshot>,
    destination: ClusterId,
    pivot: FilamentId,
    pivot_position: i32,
) -> Result<(), RecursiveIncludeError> {
    let destination = ownership.cluster_ancestor(destination)?;
    let pivot_ancestor = ownership.filament_ancestor(pivot)?;
    let pivot_combs = ownership
        .combs_of(pivot_ancestor)?
        .values()
        .copied()
        .collect::<Vec<_>>();

    for comb_id in pivot_combs {
        let snapshot = combs
            .get_mut(&comb_id)
            .ok_or(RecursiveIncludeError::MissingCombSnapshot(comb_id))?;
        if snapshot.processed {
            continue;
        }
        snapshot.processed = true;
        let members = snapshot.members.clone();
        let pivot_index = members
            .iter()
            .position(|member| ownership.filament_ancestor(*member).ok() == Some(pivot_ancestor))
            .ok_or(RecursiveIncludeError::PivotMissingFromComb {
                pivot: pivot_ancestor,
                comb: comb_id,
            })?;
        let pivot_index =
            i32::try_from(pivot_index).map_err(|_| RecursiveIncludeError::PositionOverflow)?;
        let delta = pivot_position
            .checked_sub(pivot_index)
            .ok_or(RecursiveIncludeError::PositionOverflow)?;

        for (index, member) in members.into_iter().enumerate() {
            let index =
                i32::try_from(index).map_err(|_| RecursiveIncludeError::PositionOverflow)?;
            let position = index
                .checked_add(delta)
                .ok_or(RecursiveIncludeError::PositionOverflow)?;
            let member = ownership.filament_ancestor(member)?;

            if let Some(membership) = ownership.membership_of(member)? {
                if membership.cluster() != destination {
                    let shift = position
                        .checked_sub(membership.position())
                        .ok_or(RecursiveIncludeError::PositionOverflow)?;
                    merge_cluster_values(
                        ownership,
                        clusters,
                        destination,
                        membership.cluster(),
                        shift,
                    )?;
                }
                continue;
            }

            let filament = filaments
                .get(&member)
                .cloned()
                .ok_or(RecursiveIncludeError::MissingFilamentValue(member))?;
            let resident = clusters
                .get(&destination)
                .ok_or(RecursiveIncludeError::MissingClusterValue(destination))?
                .line_at(position)
                .map(|line| line.primary_id());

            if let Some(resident) = resident {
                let resident = ownership.filament_ancestor(resident)?;
                clusters
                    .get_mut(&destination)
                    .expect("destination was checked above")
                    .include_line(position, member, filament)?;
                ownership.merge_filaments(resident, member)?;
            } else {
                clusters
                    .get_mut(&destination)
                    .expect("destination was checked above")
                    .include_line(position, member, filament)?;
                ownership.assign_filament(member, destination, position)?;
            }

            if member != pivot_ancestor {
                include_recursive(
                    ownership,
                    clusters,
                    filaments,
                    combs,
                    destination,
                    member,
                    position,
                )?;
            }
        }
    }
    Ok(())
}

pub(crate) fn merge_cluster_values(
    ownership: &mut ClusterOwnership,
    clusters: &mut BTreeMap<ClusterId, LineCluster>,
    destination: ClusterId,
    swallowed: ClusterId,
    shift: i32,
) -> Result<(), RecursiveIncludeError> {
    let destination = ownership.cluster_ancestor(destination)?;
    let swallowed = ownership.cluster_ancestor(swallowed)?;
    if destination == swallowed {
        return Ok(());
    }

    let mut destination_value = clusters
        .remove(&destination)
        .ok_or(RecursiveIncludeError::MissingClusterValue(destination))?;
    let swallowed_value = clusters
        .remove(&swallowed)
        .ok_or(RecursiveIncludeError::MissingClusterValue(swallowed))?;

    // Java visits the swallowed cluster in relative-position order. At an
    // occupied destination key, the existing line wins and steals sections,
    // comb links, and ancestry before the cluster parent is installed.
    for (position, incoming) in swallowed_value.lines() {
        let target = position
            .checked_add(shift)
            .ok_or(RecursiveIncludeError::PositionOverflow)?;
        let Some(resident) = destination_value.line_at(target) else {
            continue;
        };
        let resident = ownership.filament_ancestor(resident.primary_id())?;
        let incoming = ownership.filament_ancestor(incoming.primary_id())?;
        ownership.merge_filaments(resident, incoming)?;
    }

    destination_value.merge_with_shift(swallowed_value, shift)?;
    ownership.merge_clusters(destination, swallowed, shift)?;
    clusters.insert(destination, destination_value);
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RecursiveIncludeError {
    Ownership(ClusterOwnershipError),
    Cluster(LineClusterError),
    Filament(FilamentError),
    MissingCombSnapshot(CombId),
    MissingFilamentValue(FilamentId),
    MissingClusterValue(ClusterId),
    DuplicateClusterValue(ClusterId),
    PivotMissingFromComb { pivot: FilamentId, comb: CombId },
    PositionOverflow,
}

impl From<ClusterOwnershipError> for RecursiveIncludeError {
    fn from(value: ClusterOwnershipError) -> Self {
        Self::Ownership(value)
    }
}

impl From<LineClusterError> for RecursiveIncludeError {
    fn from(value: LineClusterError) -> Self {
        Self::Cluster(value)
    }
}

impl From<FilamentError> for RecursiveIncludeError {
    fn from(value: FilamentError) -> Self {
        Self::Filament(value)
    }
}

impl fmt::Display for RecursiveIncludeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ownership(error) => write!(formatter, "recursive ownership error: {error}"),
            Self::Cluster(error) => write!(formatter, "recursive cluster error: {error}"),
            Self::Filament(error) => write!(formatter, "recursive filament error: {error}"),
            Self::MissingCombSnapshot(id) => {
                write!(formatter, "missing comb snapshot {}", id.value())
            }
            Self::MissingFilamentValue(id) => {
                write!(formatter, "missing filament value {}", id.value())
            }
            Self::MissingClusterValue(id) => {
                write!(formatter, "missing cluster value {}", id.value())
            }
            Self::DuplicateClusterValue(id) => {
                write!(formatter, "duplicate cluster value {}", id.value())
            }
            Self::PivotMissingFromComb { pivot, comb } => write!(
                formatter,
                "pivot filament {} is absent from comb {}",
                pivot.value(),
                comb.value()
            ),
            Self::PositionOverflow => formatter.write_str("recursive line position overflow"),
        }
    }
}

impl Error for RecursiveIncludeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        comb_builder::{CombFilament, retrieve_combs},
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections},
    };

    fn fixture_with_lengths(
        lengths: &[usize],
    ) -> (ClusterOwnership, BTreeMap<FilamentId, StaffFilament>) {
        let width = lengths
            .iter()
            .enumerate()
            .map(|(index, length)| (index * 20) + length + 1)
            .max()
            .unwrap_or(1);
        let mut table =
            RunTable::new(Orientation::Horizontal, width, (lengths.len() * 3) + 2).unwrap();
        for (index, length) in lengths.iter().copied().enumerate() {
            table
                .add_run((index * 3) + 1, Run::new(index * 20, length))
                .unwrap();
        }
        let sections = build_sections(&table, JunctionPolicy::All);
        assert_eq!(sections.len(), lengths.len());

        let mut ownership = ClusterOwnership::new();
        let mut filaments = BTreeMap::new();
        for (index, section) in sections.into_iter().enumerate() {
            let id = FilamentId::new((index + 1) as u64);
            let mut filament = StaffFilament::new(10).unwrap();
            filament.add_section(section).unwrap();
            ownership.register_filament(id, &filament).unwrap();
            filaments.insert(id, filament);
        }
        (ownership, filaments)
    }

    fn fixture(count: usize) -> (ClusterOwnership, BTreeMap<FilamentId, StaffFilament>) {
        fixture_with_lengths(&vec![18; count])
    }

    fn add_comb(
        ownership: &mut ClusterOwnership,
        snapshots: &mut BTreeMap<CombId, RecursiveCombSnapshot>,
        id: u64,
        column: i32,
        members: &[u64],
    ) {
        let mut comb = FilamentComb::new(column);
        for (index, member) in members.iter().copied().enumerate() {
            comb.append_root(member as usize, (index * 10) as f64)
                .unwrap();
        }
        let id = CombId::new(id);
        ownership.register_comb(id, &comb).unwrap();
        snapshots.insert(id, RecursiveCombSnapshot::from_comb(&comb));
    }

    #[test]
    fn split_middle_line_follows_combs_to_longer_right_ancestor() {
        let mut table = RunTable::new(Orientation::Horizontal, 100, 61).unwrap();
        for y in [10, 20, 40, 50] {
            assert!(table.add_run(y, Run::new(0, 100)).unwrap());
        }
        assert!(table.add_run(30, Run::new(0, 21)).unwrap());
        assert!(table.add_run(30, Run::new(40, 60)).unwrap());
        let sections = build_sections(&table, JunctionPolicy::All);
        assert_eq!(sections.len(), 6);

        let mut ownership = ClusterOwnership::new();
        let mut filaments = BTreeMap::new();
        let mut order = Vec::new();
        let mut left = None;
        let mut right = None;
        for (index, section) in sections.into_iter().enumerate() {
            let bounds = section.bounds();
            let section_id = section.id();
            let id = FilamentId::new((index + 1) as u64);
            if bounds.y == 30 && bounds.x == 0 {
                left = Some((id, section_id));
            } else if bounds.y == 30 && bounds.x == 40 {
                right = Some((id, section_id));
            }
            let mut filament = StaffFilament::new(10).unwrap();
            filament.add_section(section).unwrap();
            ownership.register_filament(id, &filament).unwrap();
            filaments.insert(id, filament);
            order.push(id);
        }
        let (left, left_section) = left.unwrap();
        let (right, right_section) = right.unwrap();

        let comb_filaments = order
            .iter()
            .map(|id| {
                CombFilament::new(
                    id.value() as usize,
                    id.value() as usize,
                    filaments[id].geometry().unwrap(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let columns = retrieve_combs(100, 10, 10, 10, &comb_filaments).unwrap();
        let mut snapshots = BTreeMap::new();
        for (next_comb_id, comb) in (1_u64..).zip(columns.iter().flat_map(|column| column.combs()))
        {
            let id = CombId::new(next_comb_id);
            ownership.register_comb(id, comb).unwrap();
            snapshots.insert(id, RecursiveCombSnapshot::from_comb(comb));
        }
        assert_eq!(snapshots.len(), 10);

        follow_combs_network(&mut ownership, &mut filaments, &snapshots, &mut order).unwrap();

        assert_eq!(order.len(), 5);
        assert!(!order.contains(&left));
        assert!(order.contains(&right));
        assert_eq!(ownership.filament_parent(left).unwrap(), Some(right));
        assert_eq!(ownership.filament_ancestor(left).unwrap(), right);
        assert_eq!(ownership.section_owner(left_section), Some(right));
        assert_eq!(ownership.section_owner(right_section), Some(right));
        assert_eq!(filaments[&right].sections().len(), 2);
        assert_eq!(filaments[&right].bounds().unwrap().width, 100);
        assert_eq!(
            ownership
                .combs_of(right)
                .unwrap()
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            [1, 2, 4, 5, 6, 7, 8, 9]
        );

        let mut clusters = BTreeMap::new();
        let mut cluster_snapshots = snapshots.clone();
        let formation = form_clusters_from_combs(
            &mut ownership,
            &mut clusters,
            &filaments,
            &mut cluster_snapshots,
            &order,
            10,
            false,
        )
        .unwrap();
        assert_eq!(formation.cluster_ids().len(), 1);
        assert_eq!(clusters[&formation.cluster_ids()[0]].size(), 5);
    }

    #[test]
    fn follow_network_uses_column_order_and_incumbent_wins_length_tie() {
        let (mut ownership, mut filaments) = fixture_with_lengths(&[40, 20, 20]);
        let pivot = FilamentId::new(1);
        let lower_column = FilamentId::new(2);
        let higher_column = FilamentId::new(3);
        let higher_section = filaments[&higher_column].sections()[0].id();
        let mut snapshots = BTreeMap::new();
        // Register the higher column first. StaffFilament.getCombs is a
        // TreeMap, so Java still visits column 10 before column 20.
        add_comb(&mut ownership, &mut snapshots, 100, 20, &[1, 3]);
        add_comb(&mut ownership, &mut snapshots, 200, 10, &[1, 2]);
        let mut order = vec![pivot, lower_column, higher_column];

        follow_combs_network(&mut ownership, &mut filaments, &snapshots, &mut order).unwrap();

        assert_eq!(order, [pivot, lower_column]);
        assert_eq!(
            ownership.filament_ancestor(higher_column).unwrap(),
            lower_column
        );
        assert_eq!(ownership.section_owner(higher_section), Some(lower_column));
        assert_eq!(filaments[&lower_column].sections().len(), 2);
        assert_eq!(
            ownership
                .combs_of(lower_column)
                .unwrap()
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            [10, 20]
        );
    }

    #[test]
    fn swallowed_outer_pivot_traverses_its_frozen_direct_comb_map() {
        let (mut ownership, mut filaments) = fixture_with_lengths(&[3, 2, 5, 1]);
        let ids = [
            FilamentId::new(1),
            FilamentId::new(2),
            FilamentId::new(3),
            FilamentId::new(4),
        ];
        let mut snapshots = BTreeMap::new();
        add_comb(&mut ownership, &mut snapshots, 10, 1, &[3, 1]);
        add_comb(&mut ownership, &mut snapshots, 20, 2, &[1, 2, 3]);
        add_comb(&mut ownership, &mut snapshots, 30, 3, &[3, 4, 1]);
        let mut order = ids.to_vec();

        follow_combs_network(&mut ownership, &mut filaments, &snapshots, &mut order).unwrap();

        // Filament 4 is swallowed during filament 1's turn. Java still runs
        // its later outer turn over only its own original {column 3} map. If
        // that alias were resolved through ownership to filament 3's union,
        // filament 2 would be swallowed as well.
        assert_eq!(order, [ids[1], ids[2]]);
        assert_eq!(
            ids.map(|id| ownership.filament_ancestor(id).unwrap()),
            [ids[2], ids[1], ids[2], ids[2]]
        );
    }

    #[test]
    fn chained_collision_compares_the_winners_enlarged_bounds() {
        let (mut ownership, mut filaments) = fixture_with_lengths(&[10, 20, 30, 40]);
        let pivot = FilamentId::new(1);
        let first = FilamentId::new(2);
        let winner = FilamentId::new(3);
        let last = FilamentId::new(4);
        let mut snapshots = BTreeMap::new();
        add_comb(&mut ownership, &mut snapshots, 10, 10, &[1, 2]);
        add_comb(&mut ownership, &mut snapshots, 20, 20, &[1, 3]);
        add_comb(&mut ownership, &mut snapshots, 30, 30, &[1, 4]);
        let mut order = vec![pivot, first, winner, last];

        follow_combs_network(&mut ownership, &mut filaments, &snapshots, &mut order).unwrap();

        // Filament 3 first wins 2 by 30 > 20. Their current union spans 50
        // pixels, so it then wins 4 by 50 > 40; comparing the original width
        // of 3 would choose 4 incorrectly.
        assert_eq!(order, [pivot, winner]);
        assert_eq!(ownership.filament_ancestor(first).unwrap(), winner);
        assert_eq!(ownership.filament_ancestor(last).unwrap(), winner);
        assert_eq!(filaments[&winner].bounds().unwrap().width, 80);
        assert_eq!(filaments[&winner].sections().len(), 3);
    }

    #[test]
    fn follow_network_failure_rolls_back_prior_merges() {
        let (mut ownership, mut filaments) = fixture_with_lengths(&[40, 20, 20, 20]);
        let ids = [
            FilamentId::new(1),
            FilamentId::new(2),
            FilamentId::new(3),
            FilamentId::new(4),
        ];
        let mut snapshots = BTreeMap::new();
        add_comb(&mut ownership, &mut snapshots, 10, 10, &[1, 2]);
        add_comb(&mut ownership, &mut snapshots, 20, 20, &[1, 3]);
        add_comb(&mut ownership, &mut snapshots, 30, 30, &[1, 4]);
        snapshots.remove(&CombId::new(30));
        let mut order = ids.to_vec();
        let original_sections = ids.map(|id| filaments[&id].sections()[0].id());

        assert_eq!(
            follow_combs_network(&mut ownership, &mut filaments, &snapshots, &mut order),
            Err(FollowCombsNetworkError::MissingCombSnapshot(CombId::new(
                30
            )))
        );

        assert_eq!(order, ids);
        for (id, section) in ids.into_iter().zip(original_sections) {
            assert_eq!(ownership.filament_ancestor(id).unwrap(), id);
            assert_eq!(ownership.filament_parent(id).unwrap(), None);
            assert_eq!(ownership.section_owner(section), Some(id));
            assert_eq!(filaments[&id].sections().len(), 1);
        }
    }

    #[test]
    fn batch_formation_is_stable_filters_ancestors_and_reports_popular_size() {
        let (mut ownership, mut filaments) = fixture_with_lengths(&[40, 20, 30, 25, 25, 10]);
        let one = FilamentId::new(1);
        let two = FilamentId::new(2);
        // Model followCombsNetwork having selected filament 2 as ancestor. Java
        // still sorts the original filament 1 entry by its own longer width,
        // then resolves it to 2 before deciding whether to seed.
        let mut merged_two = filaments[&two].clone();
        for section in filaments[&one].sections() {
            merged_two.add_section(section.clone()).unwrap();
        }
        ownership.merge_filaments(two, one).unwrap();
        filaments.insert(two, merged_two);

        let mut snapshots = BTreeMap::new();
        add_comb(&mut ownership, &mut snapshots, 10, 1, &[2, 3]);
        add_comb(&mut ownership, &mut snapshots, 11, 2, &[4, 5]);
        let mut clusters = BTreeMap::new();
        let formation = form_clusters_from_combs(
            &mut ownership,
            &mut clusters,
            &filaments,
            &mut snapshots,
            &[
                FilamentId::new(3),
                two,
                one,
                FilamentId::new(5),
                FilamentId::new(4),
                FilamentId::new(6),
            ],
            10,
            false,
        )
        .unwrap();

        // Alias 1 is longest, so its current ancestor 2 seeds first. Equal-width
        // 5 and 4 retain input order, making 5 the second seed.
        assert_eq!(
            formation.cluster_ids(),
            [
                ClusterId::from_seed(two),
                ClusterId::from_seed(FilamentId::new(5))
            ]
        );
        assert_eq!(formation.popular_comb_size(), Some(2));
        assert_eq!(clusters.len(), 2);
        assert_eq!(ownership.filament_ancestor(one).unwrap(), two);
        assert_eq!(
            ownership
                .membership_of(FilamentId::new(3))
                .unwrap()
                .unwrap()
                .cluster(),
            ClusterId::from_seed(two)
        );
        assert!(
            ownership
                .membership_of(FilamentId::new(6))
                .unwrap()
                .is_none()
        );
        assert!(snapshots.values().all(RecursiveCombSnapshot::is_processed));
    }

    #[test]
    fn batch_single_flag_controls_combless_seed_eligibility() {
        let (mut ownership, filaments) = fixture(1);
        let seed = FilamentId::new(1);
        let mut clusters = BTreeMap::new();
        let mut snapshots = BTreeMap::new();

        let skipped = form_clusters_from_combs(
            &mut ownership,
            &mut clusters,
            &filaments,
            &mut snapshots,
            &[seed],
            10,
            false,
        )
        .unwrap();
        assert!(skipped.cluster_ids().is_empty());
        assert_eq!(skipped.popular_comb_size(), None);
        assert_eq!(ownership.membership_of(seed).unwrap(), None);

        let formed = form_clusters_from_combs(
            &mut ownership,
            &mut clusters,
            &filaments,
            &mut snapshots,
            &[seed],
            10,
            true,
        )
        .unwrap();
        assert_eq!(formed.cluster_ids(), [ClusterId::from_seed(seed)]);
        assert_eq!(clusters[&ClusterId::from_seed(seed)].size(), 1);
    }

    #[test]
    fn snapshot_popularity_weights_members_and_keeps_lower_tie() {
        let snapshot = |members: &[u64]| {
            let mut comb = FilamentComb::new(1);
            for (index, member) in members.iter().copied().enumerate() {
                comb.append_root(member as usize, index as f64).unwrap();
            }
            RecursiveCombSnapshot::from_comb(&comb)
        };
        let snapshots = BTreeMap::from([
            (CombId::new(1), snapshot(&[1, 2])),
            (CombId::new(2), snapshot(&[3, 4])),
            (CombId::new(3), snapshot(&[5, 6, 7, 8])),
        ]);

        // Two 2-member combs cast four votes; one 4-member comb also casts
        // four. Java Histogram keeps the lower bucket on a count tie.
        assert_eq!(popular_snapshot_size(&snapshots), Some(2));
    }

    #[test]
    fn recursive_cycle_processes_each_comb_once_in_column_order() {
        let (mut ownership, filaments) = fixture(3);
        let mut snapshots = BTreeMap::new();
        add_comb(&mut ownership, &mut snapshots, 10, 1, &[1, 2]);
        add_comb(&mut ownership, &mut snapshots, 11, 2, &[2, 3]);
        add_comb(&mut ownership, &mut snapshots, 12, 3, &[3, 1]);

        let seed = FilamentId::new(1);
        let cluster_id = ownership.register_cluster(seed).unwrap();
        let mut clusters = BTreeMap::from([(
            cluster_id,
            LineCluster::new(10, seed, filaments[&seed].clone()).unwrap(),
        )]);

        include_from_combs(
            &mut ownership,
            &mut clusters,
            &filaments,
            &mut snapshots,
            cluster_id,
            seed,
            0,
        )
        .unwrap();

        assert!(snapshots.values().all(RecursiveCombSnapshot::is_processed));
        assert_eq!(
            clusters[&cluster_id]
                .lines()
                .map(|(position, line)| (position, line.primary_id().value()))
                .collect::<Vec<_>>(),
            [(0, 1), (1, 2), (2, 3)]
        );
        for (position, id) in [1_u64, 2, 3].into_iter().enumerate() {
            let membership = ownership
                .membership_of(FilamentId::new(id))
                .unwrap()
                .unwrap();
            assert_eq!(membership.cluster(), cluster_id);
            assert_eq!(membership.position(), position as i32);
        }
    }

    #[test]
    fn cluster_merge_preserves_order_and_collision_overwrite_semantics() {
        let (mut ownership, filaments) = fixture(4);
        let mut snapshots = BTreeMap::new();
        add_comb(&mut ownership, &mut snapshots, 10, 1, &[1, 2]);
        // This later same-column link replaces comb 10 in filament 2's
        // reverse map and is transferred to resident filament 4 on collision.
        add_comb(&mut ownership, &mut snapshots, 11, 1, &[2, 3]);

        let one = FilamentId::new(1);
        let two = FilamentId::new(2);
        let three = FilamentId::new(3);
        let four = FilamentId::new(4);
        let destination = ownership.register_cluster(one).unwrap();
        ownership.assign_filament(four, destination, 1).unwrap();
        let mut destination_value = LineCluster::new(10, one, filaments[&one].clone()).unwrap();
        destination_value
            .include_line(1, four, filaments[&four].clone())
            .unwrap();

        let swallowed = ownership.register_cluster(two).unwrap();
        ownership.assign_filament(three, swallowed, 1).unwrap();
        let mut swallowed_value = LineCluster::new(10, two, filaments[&two].clone()).unwrap();
        swallowed_value
            .include_line(1, three, filaments[&three].clone())
            .unwrap();
        let swallowed_section = filaments[&two].sections()[0].id();
        let mut clusters = BTreeMap::from([
            (destination, destination_value),
            (swallowed, swallowed_value),
        ]);

        include_from_combs(
            &mut ownership,
            &mut clusters,
            &filaments,
            &mut snapshots,
            destination,
            one,
            0,
        )
        .unwrap();

        assert_eq!(
            ownership.cluster_parent(swallowed).unwrap(),
            Some(destination)
        );
        assert_eq!(ownership.filament_parent(two).unwrap(), Some(four));
        assert_eq!(ownership.section_owner(swallowed_section), Some(four));
        assert_eq!(
            ownership.combs_of(four).unwrap().get(&1),
            Some(&CombId::new(11))
        );
        assert_eq!(clusters.len(), 1);
        let merged = &clusters[&destination];
        assert_eq!(
            merged
                .lines()
                .map(|(position, line)| (position, line.primary_id().value()))
                .collect::<Vec<_>>(),
            [(0, 1), (1, 4), (2, 3)]
        );
        assert_eq!(merged.line_at(1).unwrap().absorbed_ids(), &[two]);
        assert!(snapshots[&CombId::new(10)].is_processed());
        assert!(!snapshots[&CombId::new(11)].is_processed());
    }

    #[test]
    fn reverse_comb_column_order_selects_the_first_resident_line() {
        let (mut ownership, filaments) = fixture(3);
        let mut snapshots = BTreeMap::new();
        // Both combs target position 1. Java walks the pivot's TreeMap by
        // column, so filament 2 becomes resident and then swallows filament 3.
        add_comb(&mut ownership, &mut snapshots, 20, 20, &[1, 3]);
        add_comb(&mut ownership, &mut snapshots, 10, 10, &[1, 2]);
        let seed = FilamentId::new(1);
        let two = FilamentId::new(2);
        let three = FilamentId::new(3);
        let cluster_id = ownership.register_cluster(seed).unwrap();
        let mut clusters = BTreeMap::from([(
            cluster_id,
            LineCluster::new(10, seed, filaments[&seed].clone()).unwrap(),
        )]);

        include_from_combs(
            &mut ownership,
            &mut clusters,
            &filaments,
            &mut snapshots,
            cluster_id,
            seed,
            0,
        )
        .unwrap();

        let line = clusters[&cluster_id].line_at(1).unwrap();
        assert_eq!(line.primary_id(), two);
        assert_eq!(line.absorbed_ids(), &[three]);
        assert_eq!(ownership.filament_parent(three).unwrap(), Some(two));
    }

    #[test]
    fn failure_rolls_back_processed_flags_membership_and_cluster_value() {
        let (mut ownership, filaments) = fixture(2);
        let mut snapshots = BTreeMap::new();
        add_comb(&mut ownership, &mut snapshots, 10, 1, &[2, 1]);
        let seed = FilamentId::new(1);
        let other = FilamentId::new(2);
        let cluster_id = ownership.register_cluster(seed).unwrap();
        let mut clusters = BTreeMap::from([(
            cluster_id,
            LineCluster::new(10, seed, filaments[&seed].clone()).unwrap(),
        )]);

        assert_eq!(
            include_from_combs(
                &mut ownership,
                &mut clusters,
                &filaments,
                &mut snapshots,
                cluster_id,
                seed,
                i32::MIN,
            ),
            Err(RecursiveIncludeError::PositionOverflow)
        );
        assert!(!snapshots[&CombId::new(10)].is_processed());
        assert_eq!(clusters[&cluster_id].size(), 1);
        assert_eq!(ownership.membership_of(other).unwrap(), None);
        assert_eq!(ownership.filament_parent(other).unwrap(), None);
    }
}
