// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light core of Java `LineCluster`.
//!
//! The cluster owns horizontal staff filaments in relative-position order and
//! refers to their source identities with stable IDs. This avoids Java's
//! bidirectional filament/cluster object graph. Comb discovery, cluster-parent
//! merging, SIG integration, trimming, persistence, and UI/VIP behavior are
//! deliberately outside this slice.

use std::{collections::BTreeMap, error::Error, fmt};

use crate::{
    filament::{FilamentError, StaffFilament},
    section::Bounds,
};

/// Stable caller-provided identity for a source staff filament.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FilamentId(u64);

impl FilamentId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// One relative line in a cluster.
#[derive(Clone, Debug)]
pub struct ClusterLine {
    primary_id: FilamentId,
    absorbed_ids: Vec<FilamentId>,
    filament: StaffFilament,
}

impl ClusterLine {
    #[must_use]
    pub const fn primary_id(&self) -> FilamentId {
        self.primary_id
    }

    /// IDs merged into the primary filament at the same relative position.
    #[must_use]
    pub fn absorbed_ids(&self) -> &[FilamentId] {
        &self.absorbed_ids
    }

    #[must_use]
    pub const fn filament(&self) -> &StaffFilament {
        &self.filament
    }
}

/// Staff candidate whose lines are keyed by their relative vertical position.
#[derive(Clone, Debug)]
pub struct LineCluster {
    seed_id: FilamentId,
    interline: usize,
    lines: BTreeMap<i32, ClusterLine>,
}

impl LineCluster {
    /// Create Java's one-line seed cluster at relative position zero.
    pub fn new(
        interline: usize,
        seed_id: FilamentId,
        seed: StaffFilament,
    ) -> Result<Self, LineClusterError> {
        if interline == 0 {
            return Err(FilamentError::InvalidInterline.into());
        }
        seed.bounds()?;
        let mut lines = BTreeMap::new();
        lines.insert(
            0,
            ClusterLine {
                primary_id: seed_id,
                absorbed_ids: Vec::new(),
                filament: seed,
            },
        );
        Ok(Self {
            seed_id,
            interline,
            lines,
        })
    }

    /// Identity from which Java derives the cluster label (`C<seed id>`).
    #[must_use]
    pub const fn seed_id(&self) -> FilamentId {
        self.seed_id
    }

    #[must_use]
    pub const fn interline(&self) -> usize {
        self.interline
    }

    /// Include a filament at a relative line position.
    ///
    /// An empty position gains a new ordered member. At an occupied position,
    /// Java includes the incoming filament into the resident one; this neutral
    /// form does the same with its sections and records the absorbed stable ID.
    pub fn include_line(
        &mut self,
        position: i32,
        id: FilamentId,
        filament: StaffFilament,
    ) -> Result<(), LineClusterError> {
        filament.bounds()?;
        if self.contains_id(id) {
            return Err(LineClusterError::DuplicateFilamentId(id));
        }

        if let Some(current) = self.lines.get_mut(&position) {
            // Work on a clone so a rejected section cannot partially mutate the cluster.
            let mut merged = current.filament.clone();
            for section in filament.sections() {
                merged.add_section(section.clone())?;
            }
            current.filament = merged;
            current.absorbed_ids.push(id);
        } else {
            self.lines.insert(
                position,
                ClusterLine {
                    primary_id: id,
                    absorbed_ids: Vec::new(),
                    filament,
                },
            );
        }
        Ok(())
    }

    fn contains_id(&self, id: FilamentId) -> bool {
        self.lines
            .values()
            .any(|line| line.primary_id == id || line.absorbed_ids.contains(&id))
    }

    /// Members in stable top-to-bottom relative-position order.
    pub fn lines(&self) -> impl DoubleEndedIterator<Item = (i32, &ClusterLine)> {
        self.lines.iter().map(|(&position, line)| (position, line))
    }

    #[must_use]
    pub fn first_line(&self) -> &ClusterLine {
        self.lines.first_key_value().expect("seeded cluster").1
    }

    #[must_use]
    pub fn last_line(&self) -> &ClusterLine {
        self.lines.last_key_value().expect("seeded cluster").1
    }

    #[must_use]
    pub fn size(&self) -> usize {
        self.lines.len()
    }

    #[must_use]
    pub fn is_one_line(&self) -> bool {
        self.lines.len() == 1
    }

    /// Inclusive union of all member filament bounds.
    pub fn bounds(&self) -> Result<Bounds, LineClusterError> {
        let mut members = self.lines.values();
        let mut bounds = members.next().expect("seeded cluster").filament.bounds()?;
        let mut max_x = bounds.x + bounds.width - 1;
        let mut max_y = bounds.y + bounds.height - 1;
        for member in members {
            let current = member.filament.bounds()?;
            let min_x = bounds.x.min(current.x);
            let min_y = bounds.y.min(current.y);
            max_x = max_x.max(current.x + current.width - 1);
            max_y = max_y.max(current.y + current.height - 1);
            bounds.x = min_x;
            bounds.y = min_y;
        }
        bounds.width = max_x - bounds.x + 1;
        bounds.height = max_y - bounds.y + 1;
        Ok(bounds)
    }

    /// Java's integer-truncated mean of member `trueLength` values.
    pub fn true_length(&self) -> Result<usize, LineClusterError> {
        let sum = self.lines.values().try_fold(0_usize, |sum, line| {
            Ok::<_, LineClusterError>(sum + line.filament.true_length()?)
        })?;
        Ok(sum / self.lines.len())
    }

    /// Java-compatible points at `x`, ordered from top to bottom.
    ///
    /// Missing points first try vertical transfer from an immediately adjacent
    /// cluster line, then short horizontal extrapolation at `global_slope`.
    pub fn points_at(
        &self,
        x: f64,
        x_margin: usize,
        global_slope: f64,
    ) -> Result<Vec<Option<(f64, f64)>>, LineClusterError> {
        let geometries = self
            .lines
            .iter()
            .map(|(&position, line)| Ok((position, line.filament.geometry()?)))
            .collect::<Result<BTreeMap<_, _>, LineClusterError>>()?;
        let mut points = BTreeMap::new();
        let mut holes = Vec::new();

        for (&position, geometry) in &geometries {
            if geometry.is_within_range(x) {
                points.insert(position, Some((x, geometry.position_at(x)?)));
            } else {
                holes.push(position);
            }
        }

        for position in holes {
            let geometry = &geometries[&position];
            let end = if x <= geometry.start().0 {
                geometry.start()
            } else {
                geometry.stop()
            };
            let mut y = None;

            for other_position in [position.checked_sub(1), position.checked_add(1)]
                .into_iter()
                .flatten()
            {
                let Some(other) = geometries.get(&other_position) else {
                    continue;
                };
                if other.is_within_range(x) && other.is_within_range(end.0) {
                    y = Some(
                        other.position_at(x)?
                            + (geometry.position_at(end.0)? - other.position_at(end.0)?),
                    );
                    break;
                }
            }

            if y.is_none() {
                let delta_x = x - end.0;
                if delta_x.abs() <= x_margin as f64 {
                    y = Some(end.1 + (delta_x * global_slope));
                }
            }
            points.insert(position, y.map(|ordinate| (x, ordinate)));
        }

        Ok(points.into_values().collect())
    }
}

/// Failure in the supported neutral line-cluster surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineClusterError {
    Filament(FilamentError),
    DuplicateFilamentId(FilamentId),
}

impl From<FilamentError> for LineClusterError {
    fn from(value: FilamentError) -> Self {
        Self::Filament(value)
    }
}

impl fmt::Display for LineClusterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Filament(error) => write!(formatter, "line-cluster filament error: {error}"),
            Self::DuplicateFilamentId(id) => {
                write!(formatter, "duplicate filament id {}", id.value())
            }
        }
    }
}

impl Error for LineClusterError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
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

    #[test]
    fn seed_and_includes_remain_ordered_by_relative_position() {
        let mut cluster = LineCluster::new(10, FilamentId::new(10), filament(0, 10, 40)).unwrap();
        cluster
            .include_line(2, FilamentId::new(30), filament(5, 30, 45))
            .unwrap();
        cluster
            .include_line(-1, FilamentId::new(5), filament(2, 2, 42))
            .unwrap();

        assert_eq!(cluster.seed_id(), FilamentId::new(10));
        assert_eq!(cluster.interline(), 10);
        assert_eq!(cluster.size(), 3);
        assert!(!cluster.is_one_line());
        assert_eq!(
            cluster
                .lines()
                .map(|(position, line)| (position, line.primary_id().value()))
                .collect::<Vec<_>>(),
            [(-1, 5), (0, 10), (2, 30)]
        );
        assert_eq!(cluster.first_line().primary_id(), FilamentId::new(5));
        assert_eq!(cluster.last_line().primary_id(), FilamentId::new(30));
    }

    #[test]
    fn occupied_position_absorbs_sections_and_tracks_stable_ids() {
        let mut cluster = LineCluster::new(10, FilamentId::new(10), filament(0, 2, 40)).unwrap();
        cluster
            .include_line(0, FilamentId::new(11), filament(45, 2, 40))
            .unwrap();

        assert_eq!(cluster.size(), 1);
        assert!(cluster.is_one_line());
        assert_eq!(cluster.first_line().primary_id(), FilamentId::new(10));
        assert_eq!(cluster.first_line().absorbed_ids(), &[FilamentId::new(11)]);
        assert_eq!(cluster.first_line().filament().sections().len(), 2);
        assert_eq!(
            cluster.bounds().unwrap(),
            Bounds {
                x: 0,
                y: 2,
                width: 85,
                height: 1
            }
        );
        assert_eq!(cluster.true_length().unwrap(), 79);
        assert_eq!(
            cluster.include_line(1, FilamentId::new(11), filament(0, 5, 40)),
            Err(LineClusterError::DuplicateFilamentId(FilamentId::new(11)))
        );
    }

    #[test]
    fn bounds_union_and_true_length_use_all_relative_lines() {
        let mut cluster = LineCluster::new(10, FilamentId::new(1), filament(10, 10, 40)).unwrap();
        cluster
            .include_line(-1, FilamentId::new(2), filament(0, 2, 44))
            .unwrap();

        assert_eq!(
            cluster.bounds().unwrap(),
            Bounds {
                x: 0,
                y: 2,
                width: 50,
                height: 9,
            }
        );
        assert_eq!(cluster.true_length().unwrap(), 42);
    }

    #[test]
    fn points_use_adjacent_vertical_then_bounded_horizontal_extrapolation() {
        let mut cluster = LineCluster::new(10, FilamentId::new(1), filament(0, 2, 40)).unwrap();
        cluster
            .include_line(1, FilamentId::new(2), filament(10, 12, 40))
            .unwrap();

        assert_eq!(
            cluster.points_at(5.0, 3, 0.25).unwrap(),
            [Some((5.0, 2.0)), Some((5.0, 12.0))]
        );
        assert_eq!(
            cluster.points_at(-3.0, 3, 0.25).unwrap(),
            [Some((-3.0, 1.25)), None]
        );
        assert_eq!(cluster.points_at(100.0, 3, 0.25).unwrap(), [None, None]);
    }
}
