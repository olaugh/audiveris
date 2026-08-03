// SPDX-License-Identifier: AGPL-3.0-or-later

//! Transactional recursive comb inclusion from Java `LineCluster.include`.
//!
//! The caller supplies explicit comb snapshots and current unclustered filament
//! values. `ClusterOwnership` supplies all mutable back-references; `LineCluster`
//! remains the geometric value. Sheet ordering, skew-based candidate selection,
//! cluster consistency policy, glyph ownership, and SIG integration stay outside.

use std::{collections::BTreeMap, error::Error, fmt};

use crate::{
    cluster_ownership::{ClusterId, ClusterOwnership, ClusterOwnershipError, CombId},
    filament::StaffFilament,
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

fn merge_cluster_values(
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
    MissingCombSnapshot(CombId),
    MissingFilamentValue(FilamentId),
    MissingClusterValue(ClusterId),
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

impl fmt::Display for RecursiveIncludeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ownership(error) => write!(formatter, "recursive ownership error: {error}"),
            Self::Cluster(error) => write!(formatter, "recursive cluster error: {error}"),
            Self::MissingCombSnapshot(id) => {
                write!(formatter, "missing comb snapshot {}", id.value())
            }
            Self::MissingFilamentValue(id) => {
                write!(formatter, "missing filament value {}", id.value())
            }
            Self::MissingClusterValue(id) => {
                write!(formatter, "missing cluster value {}", id.value())
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
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections},
    };

    fn fixture(count: usize) -> (ClusterOwnership, BTreeMap<FilamentId, StaffFilament>) {
        let mut table = RunTable::new(Orientation::Horizontal, 160, (count * 3) + 2).unwrap();
        for index in 0..count {
            table
                .add_run((index * 3) + 1, Run::new(index * 20, 18))
                .unwrap();
        }
        let sections = build_sections(&table, JunctionPolicy::All);
        assert_eq!(sections.len(), count);

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
