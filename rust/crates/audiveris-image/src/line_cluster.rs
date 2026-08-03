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

    /// Java `mergeWith`: consume another cluster after aligning its first line.
    ///
    /// `delta_position` is expressed between the two clusters' zero-based line
    /// indices, rather than between their raw relative-position keys. Existing
    /// destination lines absorb the source line; empty positions retain the
    /// complete source identity history. The operation is transactional if a
    /// stable filament ID already occurs in both clusters.
    pub fn merge_with(&mut self, other: Self, delta_position: i32) -> Result<(), LineClusterError> {
        let self_first = *self.lines.first_key_value().expect("seeded cluster").0;
        let other_first = *other.lines.first_key_value().expect("seeded cluster").0;
        let shift = delta_position + self_first - other_first;

        for line in other.lines.values() {
            if self.contains_id(line.primary_id) {
                return Err(LineClusterError::DuplicateFilamentId(line.primary_id));
            }
            if let Some(id) = line
                .absorbed_ids
                .iter()
                .copied()
                .find(|&id| self.contains_id(id))
            {
                return Err(LineClusterError::DuplicateFilamentId(id));
            }
        }

        let mut merged_lines = self.lines.clone();
        for (position, incoming) in other.lines {
            let target_position = position + shift;
            if let Some(current) = merged_lines.get_mut(&target_position) {
                for section in incoming.filament.sections() {
                    current.filament.add_section(section.clone())?;
                }
                current.absorbed_ids.push(incoming.primary_id);
                current.absorbed_ids.extend(incoming.absorbed_ids);
            } else {
                merged_lines.insert(target_position, incoming);
            }
        }
        self.lines = merged_lines;
        Ok(())
    }

    /// Shift the first remaining relative line to position zero.
    pub fn renumber_lines(&mut self) {
        let first_position = *self.lines.first_key_value().expect("seeded cluster").0;
        if first_position == 0 {
            return;
        }
        self.lines = std::mem::take(&mut self.lines)
            .into_iter()
            .map(|(position, line)| (position - first_position, line))
            .collect();
    }

    /// Java `trim`: prune excess outer lines and reject a short sixth
    /// tablature line before renumbering the survivors from zero.
    ///
    /// When the two outer lines have equal true lengths Java removes the
    /// bottom one. Six-line clusters compare the shorter outer line with the
    /// ties-even rounded mean-length ratio of the four interior lines.
    pub fn trim(
        &mut self,
        allowed_comb_sizes: &std::collections::BTreeSet<usize>,
        minimum_tablature_length_ratio: f64,
    ) -> Result<Vec<ClusterLine>, LineClusterError> {
        let Some(&maximum_count) = allowed_comb_sizes.last() else {
            return Err(LineClusterError::EmptyCombSizes);
        };
        if maximum_count == 0 || !minimum_tablature_length_ratio.is_finite() {
            return Err(LineClusterError::InvalidTrimParameters);
        }

        let mut removed = Vec::new();
        while self.lines.len() > maximum_count {
            let (&top_position, top) = self.lines.first_key_value().expect("nonempty cluster");
            let (&bottom_position, bottom) = self.lines.last_key_value().expect("nonempty cluster");
            let remove_position = if top.filament.true_length()? < bottom.filament.true_length()? {
                top_position
            } else {
                bottom_position
            };
            removed.push(
                self.lines
                    .remove(&remove_position)
                    .expect("selected outer line"),
            );
        }

        if self.lines.len() == 6 {
            let positions = self.lines.keys().copied().collect::<Vec<_>>();
            if positions.windows(2).any(|pair| pair[1] != pair[0] + 1) {
                return Err(LineClusterError::NonContiguousTablatureLines);
            }
            let interior_sum = positions[1..5].iter().try_fold(0_usize, |sum, position| {
                Ok::<_, LineClusterError>(sum + self.lines[position].filament.true_length()?)
            })?;
            let minimum_length = (minimum_tablature_length_ratio * interior_sum as f64 / 4.0)
                .round_ties_even() as usize;
            let top_position = positions[0];
            let bottom_position = positions[5];
            let top_length = self.lines[&top_position].filament.true_length()?;
            let bottom_length = self.lines[&bottom_position].filament.true_length()?;
            let candidate = if top_length < bottom_length {
                (top_position, top_length)
            } else {
                (bottom_position, bottom_length)
            };
            if candidate.1 < minimum_length {
                removed.push(
                    self.lines
                        .remove(&candidate.0)
                        .expect("selected tablature outer line"),
                );
            }
        }

        self.renumber_lines();
        Ok(removed)
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

    /// Java `getStarts`, in top-to-bottom relative-position order.
    pub fn starts(&self) -> Result<Vec<(f64, f64)>, LineClusterError> {
        self.lines
            .values()
            .map(|line| Ok(line.filament.geometry()?.start()))
            .collect()
    }

    /// Java `getStops`, in top-to-bottom relative-position order.
    pub fn stops(&self) -> Result<Vec<(f64, f64)>, LineClusterError> {
        self.lines
            .values()
            .map(|line| Ok(line.filament.geometry()?.stop()))
            .collect()
    }

    /// Java `includeFilamentByIndex` with resolved scale pixel values.
    ///
    /// The zero-based index addresses lines in top-to-bottom relative-position
    /// order. `Ok(false)` means the index did not exist or an overlapping probe
    /// would exceed `max_fore`. Equality with `max_fore` is accepted. The
    /// filament is absorbed at the selected existing position; this method
    /// never inserts a new relative line.
    pub fn include_filament_by_index(
        &mut self,
        id: FilamentId,
        filament: StaffFilament,
        index: usize,
        probe_width: usize,
        max_fore: usize,
    ) -> Result<bool, LineClusterError> {
        filament.bounds()?;
        if self.contains_id(id) {
            return Err(LineClusterError::DuplicateFilamentId(id));
        }
        let Some(position) = self.lines.keys().nth(index).copied() else {
            return Ok(false);
        };

        let incoming_bounds = filament.bounds()?;
        let resident = &self.lines[&position].filament;
        for section in resident.sections() {
            let bounds = section.bounds();
            let common_left = incoming_bounds.x.max(bounds.x);
            let common_right =
                (incoming_bounds.x + incoming_bounds.width).min(bounds.x + bounds.width);
            let overlap = common_right.saturating_sub(common_left);
            if overlap > 0 {
                // Java integer division deliberately biases an odd overlap's
                // midpoint to the left.
                let coordinate = common_left + (overlap / 2);
                if combined_thickness_at(coordinate, probe_width, [&filament, resident])?
                    > max_fore as f64
                {
                    return Ok(false);
                }
            }
        }

        let current = self
            .lines
            .get_mut(&position)
            .expect("selected cluster line");
        let mut merged = current.filament.clone();
        for section in filament.sections() {
            merged.add_section(section.clone())?;
        }
        current.filament = merged;
        current.absorbed_ids.push(id);
        Ok(true)
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

/// Horizontal `Compounds.getThicknessAt` for two staff filaments.
fn combined_thickness_at(
    coordinate: usize,
    probe_width: usize,
    filaments: [&StaffFilament; 2],
) -> Result<f64, FilamentError> {
    let mut min_x = usize::MAX;
    let mut max_x = 0;
    for filament in filaments {
        let bounds = filament.bounds()?;
        min_x = min_x.min(bounds.x);
        max_x = max_x.max(bounds.x + bounds.width - 1);
    }
    if coordinate < min_x || coordinate > max_x {
        return Ok(0.0);
    }

    // Java grows a zero-width rectangle by integer probeWidth / 2.
    let half_width = probe_width / 2;
    if half_width == 0 {
        return Ok(0.0);
    }
    let probe_start = coordinate.saturating_sub(half_width);
    let probe_stop = coordinate + half_width; // exclusive
    let mut minimum_y = usize::MAX;
    let mut maximum_y = 0;
    let mut found = false;

    for filament in filaments {
        for section in filament.sections() {
            for (offset, run) in section.runs().iter().enumerate() {
                if run.start < probe_stop && run.stop() + 1 > probe_start {
                    let y = section.first_pos() + offset;
                    minimum_y = minimum_y.min(y);
                    maximum_y = maximum_y.max(y);
                    found = true;
                }
            }
        }
    }

    Ok(if found {
        (maximum_y - minimum_y + 1) as f64
    } else {
        0.0
    })
}

/// Failure in the supported neutral line-cluster surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineClusterError {
    Filament(FilamentError),
    DuplicateFilamentId(FilamentId),
    EmptyCombSizes,
    InvalidTrimParameters,
    NonContiguousTablatureLines,
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
            Self::EmptyCombSizes => formatter.write_str("line-cluster comb sizes are empty"),
            Self::InvalidTrimParameters => {
                formatter.write_str("line-cluster trim parameters are invalid")
            }
            Self::NonContiguousTablatureLines => {
                formatter.write_str("six-line tablature positions are not contiguous")
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

    #[test]
    fn starts_and_stops_follow_relative_position_order() {
        let mut cluster = LineCluster::new(10, FilamentId::new(1), filament(10, 10, 40)).unwrap();
        cluster
            .include_line(2, FilamentId::new(3), filament(5, 30, 45))
            .unwrap();
        cluster
            .include_line(-1, FilamentId::new(2), filament(2, 2, 42))
            .unwrap();

        assert_eq!(
            cluster.starts().unwrap(),
            [(2.0, 2.0), (10.0, 10.0), (5.0, 30.0)]
        );
        assert_eq!(
            cluster.stops().unwrap(),
            [(43.0, 2.0), (49.0, 10.0), (49.0, 30.0)]
        );
    }

    #[test]
    fn indexed_inclusion_uses_top_to_bottom_order_and_requires_existing_line() {
        let mut cluster = LineCluster::new(10, FilamentId::new(10), filament(0, 10, 40)).unwrap();
        cluster
            .include_line(-1, FilamentId::new(5), filament(0, 2, 40))
            .unwrap();
        cluster
            .include_line(2, FilamentId::new(30), filament(0, 30, 40))
            .unwrap();

        assert!(
            cluster
                .include_filament_by_index(FilamentId::new(11), filament(45, 10, 20), 1, 4, 1,)
                .unwrap()
        );
        assert_eq!(cluster.lines[&0].absorbed_ids(), &[FilamentId::new(11)]);
        assert_eq!(cluster.lines[&0].filament().sections().len(), 2);
        assert_eq!(cluster.lines[&-1].filament().sections().len(), 1);

        assert!(
            !cluster
                .include_filament_by_index(FilamentId::new(99), filament(0, 50, 20), 3, 4, 10,)
                .unwrap()
        );
        assert_eq!(cluster.size(), 3);
    }

    #[test]
    fn indexed_inclusion_rejects_only_thickness_strictly_above_max_fore() {
        let incoming = || filament(10, 12, 20);
        let mut rejected = LineCluster::new(10, FilamentId::new(1), filament(0, 10, 40)).unwrap();
        assert!(
            !rejected
                .include_filament_by_index(FilamentId::new(2), incoming(), 0, 4, 2)
                .unwrap()
        );
        assert_eq!(rejected.first_line().filament().sections().len(), 1);
        assert!(rejected.first_line().absorbed_ids().is_empty());

        let mut at_limit = LineCluster::new(10, FilamentId::new(1), filament(0, 10, 40)).unwrap();
        assert!(
            at_limit
                .include_filament_by_index(FilamentId::new(2), incoming(), 0, 4, 3)
                .unwrap()
        );
        assert_eq!(at_limit.first_line().filament().sections().len(), 2);
        assert_eq!(at_limit.first_line().absorbed_ids(), &[FilamentId::new(2)]);
        assert_eq!(
            at_limit.include_filament_by_index(FilamentId::new(2), filament(50, 12, 10), 0, 4, 3,),
            Err(LineClusterError::DuplicateFilamentId(FilamentId::new(2)))
        );
    }

    #[test]
    fn cluster_merge_aligns_first_lines_and_preserves_absorbed_ids() {
        let mut destination =
            LineCluster::new(10, FilamentId::new(10), filament(0, 10, 20)).unwrap();
        destination
            .include_line(2, FilamentId::new(30), filament(0, 30, 20))
            .unwrap();

        let mut source = LineCluster::new(10, FilamentId::new(40), filament(25, 10, 20)).unwrap();
        source
            .include_line(0, FilamentId::new(41), filament(50, 10, 20))
            .unwrap();
        source
            .include_line(1, FilamentId::new(50), filament(25, 20, 20))
            .unwrap();

        destination.merge_with(source, 1).unwrap();

        assert_eq!(
            destination
                .lines()
                .map(|(position, line)| (position, line.primary_id().value()))
                .collect::<Vec<_>>(),
            [(0, 10), (1, 40), (2, 30)]
        );
        assert_eq!(destination.lines[&1].absorbed_ids(), &[FilamentId::new(41)]);
        assert_eq!(destination.lines[&2].absorbed_ids(), &[FilamentId::new(50)]);
        assert_eq!(destination.lines[&2].filament().sections().len(), 2);
    }

    #[test]
    fn cluster_merge_rejects_duplicate_identity_without_mutation() {
        let mut destination =
            LineCluster::new(10, FilamentId::new(1), filament(0, 10, 20)).unwrap();
        let source = LineCluster::new(10, FilamentId::new(1), filament(30, 10, 20)).unwrap();

        assert_eq!(
            destination.merge_with(source, 0),
            Err(LineClusterError::DuplicateFilamentId(FilamentId::new(1)))
        );
        assert_eq!(destination.size(), 1);
        assert_eq!(destination.first_line().filament().sections().len(), 1);
    }

    #[test]
    fn renumber_lines_moves_first_position_to_zero_without_reordering() {
        let mut cluster = LineCluster::new(10, FilamentId::new(10), filament(0, 10, 20)).unwrap();
        cluster
            .include_line(-2, FilamentId::new(1), filament(0, 2, 20))
            .unwrap();
        cluster
            .include_line(3, FilamentId::new(40), filament(0, 40, 20))
            .unwrap();

        cluster.renumber_lines();

        assert_eq!(
            cluster
                .lines()
                .map(|(position, line)| (position, line.primary_id().value()))
                .collect::<Vec<_>>(),
            [(0, 1), (2, 10), (5, 40)]
        );
        cluster.renumber_lines();
        assert_eq!(
            cluster
                .lines()
                .map(|(position, _)| position)
                .collect::<Vec<_>>(),
            [0, 2, 5]
        );
    }

    #[test]
    fn trim_discards_shorter_outer_lines_and_breaks_ties_at_bottom() {
        let mut cluster = LineCluster::new(10, FilamentId::new(3), filament(0, 30, 30)).unwrap();
        for (position, id, length) in [
            (-2, 1, 10),
            (-1, 2, 20),
            (1, 4, 40),
            (2, 5, 20),
            (3, 6, 20),
            (4, 7, 20),
        ] {
            cluster
                .include_line(
                    position,
                    FilamentId::new(id),
                    filament(0, id as usize * 10, length),
                )
                .unwrap();
        }

        let removed = cluster.trim(&[5].into_iter().collect(), 0.5).unwrap();

        assert_eq!(
            removed
                .iter()
                .map(|line| line.primary_id().value())
                .collect::<Vec<_>>(),
            [1, 7]
        );
        assert_eq!(
            cluster
                .lines()
                .map(|(position, line)| (position, line.primary_id().value()))
                .collect::<Vec<_>>(),
            [(0, 2), (1, 3), (2, 4), (3, 5), (4, 6)]
        );
    }

    #[test]
    fn trim_short_sixth_line_uses_ties_even_interior_threshold() {
        let mut cluster = LineCluster::new(10, FilamentId::new(1), filament(0, 10, 3)).unwrap();
        for (position, id, length) in [(1, 2, 6), (2, 3, 6), (3, 4, 6), (4, 5, 6), (5, 6, 10)] {
            cluster
                .include_line(
                    position,
                    FilamentId::new(id),
                    filament(0, id as usize * 10, length),
                )
                .unwrap();
        }

        // Each interior true length is 5: 0.5 * 20 / 4 = 2.5, and
        // Java Math.rint rounds that tie to 2, equal to the short top line.
        assert!(
            cluster
                .trim(&[6].into_iter().collect(), 0.5)
                .unwrap()
                .is_empty()
        );
        assert_eq!(cluster.size(), 6);

        let removed = cluster.trim(&[6].into_iter().collect(), 0.6).unwrap();
        assert_eq!(
            removed
                .iter()
                .map(|line| line.primary_id().value())
                .collect::<Vec<_>>(),
            [1]
        );
        assert_eq!(cluster.size(), 5);
        assert_eq!(cluster.lines().next().unwrap().0, 0);
    }
}
