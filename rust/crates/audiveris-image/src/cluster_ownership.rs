// SPDX-License-Identifier: AGPL-3.0-or-later

//! Stable-ID ownership boundary for Java's recursive staff-cluster graph.
//!
//! `StaffFilament`, `FilamentComb`, and `LineCluster` use mutable object back-links
//! in Java. This registry keeps those links beside the neutral geometry instead:
//! filament and cluster parent forests, cluster membership, reverse comb lookup,
//! and section ownership. It deliberately does not choose merge candidates or own
//! `LineCluster` geometry; those remain coordinator policy and value state.

use std::{collections::BTreeMap, error::Error, fmt};

use crate::{filament::StaffFilament, filament_comb::FilamentComb, line_cluster::FilamentId};

/// Stable caller-provided identity for a comb object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CombId(u64);

impl CombId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Stable identity of a cluster, derived from its seed filament as in Java.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClusterId(u64);

impl ClusterId {
    #[must_use]
    pub const fn from_seed(seed: FilamentId) -> Self {
        Self(seed.value())
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Current relative line position of a filament ancestor in a cluster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClusterMembership {
    cluster: ClusterId,
    position: i32,
}

impl ClusterMembership {
    #[must_use]
    pub const fn cluster(self) -> ClusterId {
        self.cluster
    }

    #[must_use]
    pub const fn position(self) -> i32 {
        self.position
    }
}

/// Neutral replacement for the ownership fields spread across Java objects.
#[derive(Clone, Debug, Default)]
pub struct ClusterOwnership {
    filament_parents: BTreeMap<FilamentId, FilamentId>,
    cluster_parents: BTreeMap<ClusterId, ClusterId>,
    memberships: BTreeMap<FilamentId, ClusterMembership>,
    reverse_combs: BTreeMap<FilamentId, BTreeMap<i32, CombId>>,
    comb_columns: BTreeMap<CombId, i32>,
    section_owners: BTreeMap<usize, FilamentId>,
}

impl ClusterOwnership {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            filament_parents: BTreeMap::new(),
            cluster_parents: BTreeMap::new(),
            memberships: BTreeMap::new(),
            reverse_combs: BTreeMap::new(),
            comb_columns: BTreeMap::new(),
            section_owners: BTreeMap::new(),
        }
    }

    /// Register one root filament and claim each of its source sections.
    ///
    /// Registration is transactional: no state changes if either the filament
    /// identity or any section identity is already present.
    pub fn register_filament(
        &mut self,
        id: FilamentId,
        filament: &StaffFilament,
    ) -> Result<(), ClusterOwnershipError> {
        if id.value() == 0 {
            return Err(ClusterOwnershipError::InvalidFilamentId);
        }
        if self.filament_parents.contains_key(&id) {
            return Err(ClusterOwnershipError::DuplicateFilament(id));
        }
        for section in filament.sections() {
            if let Some(owner) = self.section_owners.get(&section.id()) {
                return Err(ClusterOwnershipError::SectionAlreadyOwned {
                    section: section.id(),
                    owner: *owner,
                });
            }
        }

        self.filament_parents.insert(id, id);
        self.reverse_combs.insert(id, BTreeMap::new());
        for section in filament.sections() {
            self.section_owners.insert(section.id(), id);
        }
        Ok(())
    }

    /// Register one comb and reproduce `StaffFilament.addComb` reverse links.
    ///
    /// Java keys a filament's reverse map by column, so a later comb at the same
    /// column replaces the earlier entry for that filament.
    pub fn register_comb(
        &mut self,
        id: CombId,
        comb: &FilamentComb,
    ) -> Result<(), ClusterOwnershipError> {
        if id.value() == 0 {
            return Err(ClusterOwnershipError::InvalidCombId);
        }
        if self.comb_columns.contains_key(&id) {
            return Err(ClusterOwnershipError::DuplicateComb(id));
        }

        let members = comb
            .filament_ids()
            .iter()
            .map(|&value| FilamentId::new(value as u64))
            .collect::<Vec<_>>();
        for member in &members {
            if !self.filament_parents.contains_key(member) {
                return Err(ClusterOwnershipError::UnknownFilament(*member));
            }
        }

        self.comb_columns.insert(id, comb.column());
        for member in members {
            self.reverse_combs
                .get_mut(&member)
                .expect("registered filament has a reverse-comb map")
                .insert(comb.column(), id);
        }
        Ok(())
    }

    /// Create Java's seed cluster and assign the root filament at position zero.
    pub fn register_cluster(
        &mut self,
        seed: FilamentId,
    ) -> Result<ClusterId, ClusterOwnershipError> {
        let ancestor = self.filament_ancestor(seed)?;
        if ancestor != seed {
            return Err(ClusterOwnershipError::NonRootFilament(seed));
        }
        if self.memberships.contains_key(&seed) {
            return Err(ClusterOwnershipError::FilamentAlreadyClustered(seed));
        }
        let id = ClusterId::from_seed(seed);
        if self.cluster_parents.contains_key(&id) {
            return Err(ClusterOwnershipError::DuplicateCluster(id));
        }
        self.cluster_parents.insert(id, id);
        self.memberships.insert(
            seed,
            ClusterMembership {
                cluster: id,
                position: 0,
            },
        );
        Ok(id)
    }

    /// Assign an unclustered filament ancestor to an existing cluster line.
    pub fn assign_filament(
        &mut self,
        filament: FilamentId,
        cluster: ClusterId,
        position: i32,
    ) -> Result<(), ClusterOwnershipError> {
        let filament = self.filament_ancestor(filament)?;
        let cluster = self.cluster_ancestor(cluster)?;
        if self.memberships.contains_key(&filament) {
            return Err(ClusterOwnershipError::FilamentAlreadyClustered(filament));
        }
        self.memberships
            .insert(filament, ClusterMembership { cluster, position });
        Ok(())
    }

    /// Record Java `StaffFilament.include` after the caller chooses the winner.
    ///
    /// Sections and reverse combs move to the winning ancestor. `putAll` behavior
    /// means the swallowed filament's comb wins if both maps contain a column.
    /// The winner's cluster assignment is retained, including an absent assignment.
    pub fn merge_filaments(
        &mut self,
        winner: FilamentId,
        swallowed: FilamentId,
    ) -> Result<bool, ClusterOwnershipError> {
        let winner = self.filament_ancestor(winner)?;
        let swallowed = self.filament_ancestor(swallowed)?;
        if winner == swallowed {
            return Ok(false);
        }

        let swallowed_combs = self
            .reverse_combs
            .remove(&swallowed)
            .expect("registered root has a reverse-comb map");
        self.reverse_combs
            .get_mut(&winner)
            .expect("registered root has a reverse-comb map")
            .extend(swallowed_combs);
        for owner in self.section_owners.values_mut() {
            if *owner == swallowed {
                *owner = winner;
            }
        }
        self.memberships.remove(&swallowed);
        self.filament_parents.insert(swallowed, winner);
        Ok(true)
    }

    /// Record `LineCluster.include` after its line-position shift is known.
    ///
    /// All still-distinct filament ancestors assigned to the swallowed cluster
    /// are moved to the destination and shifted atomically. Previously swallowed
    /// cluster IDs continue to resolve through the parent forest.
    pub fn merge_clusters(
        &mut self,
        destination: ClusterId,
        swallowed: ClusterId,
        position_shift: i32,
    ) -> Result<bool, ClusterOwnershipError> {
        let destination = self.cluster_ancestor(destination)?;
        let swallowed = self.cluster_ancestor(swallowed)?;
        if destination == swallowed {
            return Ok(false);
        }

        let updates = self
            .memberships
            .iter()
            .filter(|(_, membership)| membership.cluster == swallowed)
            .map(|(&filament, membership)| {
                membership
                    .position
                    .checked_add(position_shift)
                    .map(|position| (filament, position))
                    .ok_or(ClusterOwnershipError::PositionOverflow)
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (filament, position) in updates {
            self.memberships.insert(
                filament,
                ClusterMembership {
                    cluster: destination,
                    position,
                },
            );
        }
        self.cluster_parents.insert(swallowed, destination);
        Ok(true)
    }

    /// Reproduce `LineCluster.destroy` for a current cluster root.
    ///
    /// Java clears each member filament's cluster backlink and reverse comb
    /// map, but leaves the cluster and filament objects themselves alive. The
    /// neutral registry therefore retains both parent forests and section
    /// ownership while releasing active memberships and comb links.
    pub fn destroy_cluster(
        &mut self,
        cluster: ClusterId,
    ) -> Result<Vec<FilamentId>, ClusterOwnershipError> {
        let cluster = self.cluster_ancestor(cluster)?;
        let members = self
            .memberships
            .iter()
            .filter_map(|(&filament, membership)| {
                (membership.cluster == cluster).then_some(filament)
            })
            .collect::<Vec<_>>();
        for filament in &members {
            self.memberships.remove(filament);
            self.reverse_combs
                .get_mut(filament)
                .expect("registered filament has a reverse-comb map")
                .clear();
        }
        Ok(members)
    }

    /// Apply Java `LineCluster.trim` housekeeping after the value has selected
    /// and renumbered its surviving lines.
    pub fn synchronize_trimmed_cluster(
        &mut self,
        cluster: ClusterId,
        remaining: &[(FilamentId, i32)],
        removed: &[FilamentId],
    ) -> Result<(), ClusterOwnershipError> {
        let cluster = self.cluster_ancestor(cluster)?;
        let mut supplied = std::collections::BTreeSet::new();
        for &(filament, _) in remaining {
            let root = self.filament_ancestor(filament)?;
            if root != filament || !supplied.insert(root) {
                return Err(ClusterOwnershipError::InvalidClusterMembers);
            }
        }
        for &filament in removed {
            let root = self.filament_ancestor(filament)?;
            if root != filament || !supplied.insert(root) {
                return Err(ClusterOwnershipError::InvalidClusterMembers);
            }
        }
        let current = self
            .memberships
            .iter()
            .filter_map(|(&filament, membership)| {
                (membership.cluster == cluster).then_some(filament)
            })
            .collect::<std::collections::BTreeSet<_>>();
        if supplied != current {
            return Err(ClusterOwnershipError::InvalidClusterMembers);
        }

        for &filament in removed {
            self.memberships.remove(&filament);
            self.reverse_combs
                .get_mut(&filament)
                .expect("registered filament has a reverse-comb map")
                .clear();
        }
        for &(filament, position) in remaining {
            self.memberships
                .insert(filament, ClusterMembership { cluster, position });
        }
        Ok(())
    }

    pub fn filament_ancestor(
        &self,
        filament: FilamentId,
    ) -> Result<FilamentId, ClusterOwnershipError> {
        let mut current = filament;
        loop {
            let parent = self
                .filament_parents
                .get(&current)
                .copied()
                .ok_or(ClusterOwnershipError::UnknownFilament(filament))?;
            if parent == current {
                return Ok(current);
            }
            current = parent;
        }
    }

    /// Direct Java `partOf` parent, or `None` for a root filament.
    pub fn filament_parent(
        &self,
        filament: FilamentId,
    ) -> Result<Option<FilamentId>, ClusterOwnershipError> {
        let parent = self
            .filament_parents
            .get(&filament)
            .copied()
            .ok_or(ClusterOwnershipError::UnknownFilament(filament))?;
        Ok((parent != filament).then_some(parent))
    }

    pub fn cluster_ancestor(&self, cluster: ClusterId) -> Result<ClusterId, ClusterOwnershipError> {
        let mut current = cluster;
        loop {
            let parent = self
                .cluster_parents
                .get(&current)
                .copied()
                .ok_or(ClusterOwnershipError::UnknownCluster(cluster))?;
            if parent == current {
                return Ok(current);
            }
            current = parent;
        }
    }

    /// Direct Java `LineCluster.parent`, or `None` for a root cluster.
    pub fn cluster_parent(
        &self,
        cluster: ClusterId,
    ) -> Result<Option<ClusterId>, ClusterOwnershipError> {
        let parent = self
            .cluster_parents
            .get(&cluster)
            .copied()
            .ok_or(ClusterOwnershipError::UnknownCluster(cluster))?;
        Ok((parent != cluster).then_some(parent))
    }

    /// Resolve both filament and cluster ancestry at lookup time.
    pub fn membership_of(
        &self,
        filament: FilamentId,
    ) -> Result<Option<ClusterMembership>, ClusterOwnershipError> {
        let filament = self.filament_ancestor(filament)?;
        self.memberships
            .get(&filament)
            .copied()
            .map(|membership| {
                Ok(ClusterMembership {
                    cluster: self.cluster_ancestor(membership.cluster)?,
                    position: membership.position,
                })
            })
            .transpose()
    }

    /// Current reverse comb map, ordered by sample column.
    pub fn combs_of(
        &self,
        filament: FilamentId,
    ) -> Result<&BTreeMap<i32, CombId>, ClusterOwnershipError> {
        let filament = self.filament_ancestor(filament)?;
        Ok(self
            .reverse_combs
            .get(&filament)
            .expect("registered root has a reverse-comb map"))
    }

    /// Current owning filament ancestor of a section.
    #[must_use]
    pub fn section_owner(&self, section: usize) -> Option<FilamentId> {
        self.section_owners.get(&section).copied()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClusterOwnershipError {
    InvalidFilamentId,
    InvalidCombId,
    DuplicateFilament(FilamentId),
    DuplicateComb(CombId),
    DuplicateCluster(ClusterId),
    UnknownFilament(FilamentId),
    UnknownCluster(ClusterId),
    NonRootFilament(FilamentId),
    FilamentAlreadyClustered(FilamentId),
    SectionAlreadyOwned { section: usize, owner: FilamentId },
    PositionOverflow,
    InvalidClusterMembers,
}

impl fmt::Display for ClusterOwnershipError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFilamentId => formatter.write_str("filament ID must be positive"),
            Self::InvalidCombId => formatter.write_str("comb ID must be positive"),
            Self::DuplicateFilament(id) => write!(formatter, "duplicate filament {}", id.value()),
            Self::DuplicateComb(id) => write!(formatter, "duplicate comb {}", id.value()),
            Self::DuplicateCluster(id) => write!(formatter, "duplicate cluster {}", id.value()),
            Self::UnknownFilament(id) => write!(formatter, "unknown filament {}", id.value()),
            Self::UnknownCluster(id) => write!(formatter, "unknown cluster {}", id.value()),
            Self::NonRootFilament(id) => {
                write!(formatter, "filament {} is not an ancestor", id.value())
            }
            Self::FilamentAlreadyClustered(id) => {
                write!(formatter, "filament {} is already clustered", id.value())
            }
            Self::SectionAlreadyOwned { section, owner } => write!(
                formatter,
                "section {section} is already owned by filament {}",
                owner.value()
            ),
            Self::PositionOverflow => formatter.write_str("cluster line position overflow"),
            Self::InvalidClusterMembers => {
                formatter.write_str("trimmed cluster members do not match ownership")
            }
        }
    }
}

impl Error for ClusterOwnershipError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections},
    };

    fn three_filaments() -> [StaffFilament; 3] {
        let mut table = RunTable::new(Orientation::Horizontal, 80, 9).unwrap();
        table.add_run(1, Run::new(0, 20)).unwrap();
        table.add_run(4, Run::new(20, 20)).unwrap();
        table.add_run(7, Run::new(40, 20)).unwrap();
        let mut sections = build_sections(&table, JunctionPolicy::All);
        assert_eq!(sections.len(), 3);
        std::array::from_fn(|_| {
            let mut filament = StaffFilament::new(10).unwrap();
            filament.add_section(sections.remove(0)).unwrap();
            filament
        })
    }

    fn register_three() -> (ClusterOwnership, [FilamentId; 3]) {
        let filaments = three_filaments();
        let ids = [FilamentId::new(1), FilamentId::new(2), FilamentId::new(3)];
        let mut ownership = ClusterOwnership::new();
        for (id, filament) in ids.into_iter().zip(&filaments) {
            ownership.register_filament(id, filament).unwrap();
        }
        (ownership, ids)
    }

    #[test]
    fn registration_claims_sections_transactionally() {
        let filaments = three_filaments();
        let mut ownership = ClusterOwnership::new();
        ownership
            .register_filament(FilamentId::new(10), &filaments[0])
            .unwrap();
        let section = filaments[0].sections()[0].id();
        assert_eq!(ownership.section_owner(section), Some(FilamentId::new(10)));
        assert_eq!(
            ownership.register_filament(FilamentId::new(20), &filaments[0]),
            Err(ClusterOwnershipError::SectionAlreadyOwned {
                section,
                owner: FilamentId::new(10),
            })
        );
        assert_eq!(
            ownership.filament_ancestor(FilamentId::new(20)),
            Err(ClusterOwnershipError::UnknownFilament(FilamentId::new(20)))
        );
    }

    #[test]
    fn filament_merge_moves_sections_and_java_style_comb_maps() {
        let (mut ownership, ids) = register_three();
        let mut first = FilamentComb::new(4);
        first.append_root(1, 10.0).unwrap();
        first.append_root(2, 20.0).unwrap();
        ownership.register_comb(CombId::new(10), &first).unwrap();

        let mut replacement = FilamentComb::new(4);
        replacement.append_root(2, 20.0).unwrap();
        replacement.append_root(3, 30.0).unwrap();
        ownership
            .register_comb(CombId::new(11), &replacement)
            .unwrap();
        let swallowed_section = ownership
            .section_owners
            .iter()
            .find_map(|(&section, &owner)| (owner == ids[1]).then_some(section))
            .unwrap();

        assert!(ownership.merge_filaments(ids[0], ids[1]).unwrap());
        assert_eq!(ownership.filament_parent(ids[1]).unwrap(), Some(ids[0]));
        assert_eq!(ownership.filament_ancestor(ids[1]).unwrap(), ids[0]);
        assert_eq!(ownership.section_owner(swallowed_section), Some(ids[0]));
        assert_eq!(
            ownership.combs_of(ids[0]).unwrap().get(&4),
            Some(&CombId::new(11))
        );
        assert_eq!(
            ownership.combs_of(ids[1]).unwrap(),
            ownership.combs_of(ids[0]).unwrap()
        );
        assert!(!ownership.merge_filaments(ids[1], ids[0]).unwrap());
    }

    #[test]
    fn cluster_merge_tracks_parent_and_shifted_membership() {
        let (mut ownership, ids) = register_three();
        let upper = ownership.register_cluster(ids[0]).unwrap();
        let lower = ownership.register_cluster(ids[1]).unwrap();
        ownership.assign_filament(ids[2], lower, 2).unwrap();

        assert!(ownership.merge_clusters(upper, lower, -1).unwrap());
        assert_eq!(ownership.cluster_parent(lower).unwrap(), Some(upper));
        assert_eq!(ownership.cluster_ancestor(lower).unwrap(), upper);
        assert_eq!(
            ownership.membership_of(ids[1]).unwrap(),
            Some(ClusterMembership {
                cluster: upper,
                position: -1,
            })
        );
        assert_eq!(
            ownership.membership_of(ids[2]).unwrap(),
            Some(ClusterMembership {
                cluster: upper,
                position: 1,
            })
        );
        assert!(!ownership.merge_clusters(lower, upper, 99).unwrap());
    }

    #[test]
    fn cluster_destroy_releases_memberships_and_combs_but_keeps_objects() {
        let (mut ownership, ids) = register_three();
        let mut comb = FilamentComb::new(4);
        comb.append_root(1, 10.0).unwrap();
        comb.append_root(2, 20.0).unwrap();
        ownership.register_comb(CombId::new(10), &comb).unwrap();
        let retained_section = ownership
            .section_owners
            .iter()
            .find_map(|(&section, &owner)| (owner == ids[0]).then_some(section))
            .unwrap();
        let cluster = ownership.register_cluster(ids[0]).unwrap();
        ownership.assign_filament(ids[1], cluster, 1).unwrap();

        assert_eq!(ownership.destroy_cluster(cluster).unwrap(), &ids[..2]);
        assert_eq!(ownership.membership_of(ids[0]).unwrap(), None);
        assert_eq!(ownership.membership_of(ids[1]).unwrap(), None);
        assert!(ownership.combs_of(ids[0]).unwrap().is_empty());
        assert!(ownership.combs_of(ids[1]).unwrap().is_empty());
        assert_eq!(ownership.cluster_ancestor(cluster).unwrap(), cluster);
        assert_eq!(ownership.section_owner(retained_section), Some(ids[0]));
    }

    #[test]
    fn swallowed_filament_uses_winner_cluster_membership() {
        let (mut ownership, ids) = register_three();
        let cluster = ownership.register_cluster(ids[0]).unwrap();
        let other = ownership.register_cluster(ids[1]).unwrap();
        assert_ne!(cluster, other);

        ownership.merge_filaments(ids[0], ids[1]).unwrap();
        assert_eq!(
            ownership.membership_of(ids[1]),
            ownership.membership_of(ids[0])
        );
        assert_eq!(
            ownership.membership_of(ids[1]).unwrap(),
            Some(ClusterMembership {
                cluster,
                position: 0,
            })
        );
    }
}
