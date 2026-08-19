// SPDX-License-Identifier: AGPL-3.0-or-later

//! Neutral core-section grouping for horizontal staff filaments.
//!
//! This ports the deterministic core filtering, reverse-length ordering, and
//! real-gap and overlapping-filament compatibility branches of Java
//! `FilamentFactory`, including the horizontal leftover-section expansion
//! phase. Glyph ownership, indexes, and vertical filaments remain outside this
//! dependency-light slice.

use std::{error::Error, fmt};

use crate::filament::{FilamentError, StaffFilament};
use crate::run_table::Orientation;
use crate::section::{Bounds, Section};

/// Pixel-domain parameters needed by the neutral grouping subset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilamentFactoryParams {
    pub interline: usize,
    pub min_core_section_length: usize,
    pub min_section_aspect: f64,
    pub max_coord_gap: f64,
    pub max_pos_gap: f64,
    pub max_pos_gap_for_slope: f64,
    pub max_gap_slope: f64,
    pub min_length_for_delta_slope: f64,
    pub max_delta_slope: f64,
}

/// Resolved pixel parameters for Java's overlapping-filament probes.
///
/// Audiveris derives these values from both interline and mean line thickness,
/// so they are kept separate from [`FilamentFactoryParams`] rather than
/// guessing a line scale in this dependency-light crate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlapParams {
    pub probe_width: usize,
    pub max_overlap_delta_pos: f64,
    pub max_thickness: f64,
    pub max_overlap_space: f64,
    pub max_expansion_space: f64,
    pub max_involving_length: f64,
    pub max_consistent_ratio: f64,
}

/// Why two candidates were not grouped by the supported compatibility branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Incompatibility {
    Geometry,
    OverlapRequiresThicknessProbe,
    OverlapPosition,
    Thickness,
    InconsistentThickness,
    OverlapSpace,
    NoContact,
    CoordinateGap,
    PositionGap,
    GapSlope,
    DeltaSlope,
}

/// Dependency-light factory for the initial, long-section filament skeletons.
#[derive(Clone, Copy, Debug)]
pub struct FilamentFactory {
    params: FilamentFactoryParams,
}

/// One final filament with the ID assigned by Java `FilamentIndex.register`
/// when its initial core section was created.
#[derive(Clone, Debug)]
pub struct IdentifiedFilament {
    id: u64,
    filament: StaffFilament,
}

impl IdentifiedFilament {
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub const fn filament(&self) -> &StaffFilament {
        &self.filament
    }

    #[must_use]
    pub fn into_filament(self) -> StaffFilament {
        self.filament
    }
}

/// Identity-bearing factory result. `creation_ids` includes swallowed initial
/// filaments and every temporary expansion filament, matching registrations
/// that remain visible in Java's sheet-global `FilamentIndex`.
#[derive(Clone, Debug)]
pub struct IdentifiedFilamentResult {
    survivors: Vec<IdentifiedFilament>,
    creation_ids: Vec<u64>,
    next_creation_id: u64,
}

impl IdentifiedFilamentResult {
    #[must_use]
    pub fn survivors(&self) -> &[IdentifiedFilament] {
        &self.survivors
    }

    #[must_use]
    pub fn creation_ids(&self) -> &[u64] {
        &self.creation_ids
    }

    /// ID to pass to a later factory sharing the same Java `FilamentIndex`.
    #[must_use]
    pub const fn next_creation_id(&self) -> u64 {
        self.next_creation_id
    }

    #[must_use]
    pub fn into_survivors(self) -> Vec<IdentifiedFilament> {
        self.survivors
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FilamentFactoryIdentityError {
    Filament(FilamentError),
    InvalidFirstId,
    IdentityOverflow,
}

impl From<FilamentError> for FilamentFactoryIdentityError {
    fn from(value: FilamentError) -> Self {
        Self::Filament(value)
    }
}

impl fmt::Display for FilamentFactoryIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Filament(error) => write!(formatter, "filament construction failed: {error}"),
            Self::InvalidFirstId => formatter.write_str("first filament ID must be positive"),
            Self::IdentityOverflow => formatter.write_str("filament identity overflow"),
        }
    }
}

impl Error for FilamentFactoryIdentityError {}

impl FilamentFactory {
    #[must_use]
    pub const fn new(params: FilamentFactoryParams) -> Self {
        Self { params }
    }

    /// Whether a section passes Java's orientation, length, and global-slimness gates.
    #[must_use]
    pub fn is_core_candidate(&self, section: &Section) -> bool {
        if section.orientation() != Orientation::Horizontal
            || section.length(Orientation::Horizontal) < self.params.min_core_section_length
        {
            return false;
        }

        let mean_thickness = section.mean_thickness(Orientation::Horizontal);
        mean_thickness <= 1.0
            || (section.length(Orientation::Horizontal) as f64 / mean_thickness)
                >= self.params.min_section_aspect
    }

    /// Java `isSectionFat` for a horizontal section.
    ///
    /// `overlap.probe_width` and `overlap.max_thickness` must be the resolved
    /// integer pixel values used by Java's scale-dependent parameters.
    #[must_use]
    pub fn is_section_fat(&self, section: &Section, overlap: OverlapParams) -> bool {
        if section.orientation() != Orientation::Horizontal {
            return true;
        }
        let mean_thickness = section.mean_thickness(Orientation::Horizontal);
        if mean_thickness <= 1.0 {
            return false;
        }
        let mean_aspect = section.length(Orientation::Horizontal) as f64 / mean_thickness;
        if mean_aspect < self.params.min_section_aspect {
            return true;
        }

        self.local_section_fatness(section, overlap).unwrap_or(true)
    }

    fn local_section_fatness(&self, section: &Section, overlap: OverlapParams) -> Option<bool> {
        let bounds = section.bounds();
        let line = fitted_section_line(section)?;
        if line.slope().abs() >= std::f64::consts::FRAC_PI_4 {
            return Some(bounds.height as f64 > overlap.max_thickness);
        }

        let start_coordinate = bounds.x + (bounds.width / 4);
        let stop_coordinate = bounds.x + ((3 * bounds.width) / 4);
        let start_position = line.y_at_x(start_coordinate as f64)?.round_ties_even() as isize;
        let stop_position = line.y_at_x(stop_coordinate as f64)?.round_ties_even() as isize;
        let half_width = (overlap.probe_width / 2).min(bounds.width / 4) as isize;
        let max_thickness = overlap.max_thickness as isize;
        let probe_width = 2 * half_width;

        let estimate = |coordinate: usize, position: isize| {
            if probe_width == 0 {
                return 0.0;
            }
            let count = section_pixel_count_in_signed(
                section,
                coordinate as isize - half_width,
                coordinate as isize + half_width,
                position - max_thickness,
                position + max_thickness,
            );
            (count as f64 / probe_width as f64).round_ties_even()
        };
        Some(
            estimate(start_coordinate, start_position) > overlap.max_thickness
                || estimate(stop_coordinate, stop_position) > overlap.max_thickness,
        )
    }

    /// Check the source-compatible, non-overlapping (`coordGap >= 0`) merge branch.
    pub fn gap_compatibility(
        &self,
        one: &StaffFilament,
        two: &StaffFilament,
    ) -> Result<(), Incompatibility> {
        self.compatibility(one, two, None, false)
    }

    /// Check both Java compatibility branches using resolved overlap parameters.
    ///
    /// Set `expanding` when `two` represents a leftover section being attached
    /// during Java's expansion phase. This changes the permitted internal space
    /// and, when that space is zero, enables Java's real-contact requirement.
    pub fn overlap_compatibility(
        &self,
        one: &StaffFilament,
        two: &StaffFilament,
        overlap: OverlapParams,
        expanding: bool,
    ) -> Result<(), Incompatibility> {
        self.compatibility(one, two, Some(overlap), expanding)
    }

    fn compatibility(
        &self,
        one: &StaffFilament,
        two: &StaffFilament,
        overlap: Option<OverlapParams>,
        expanding: bool,
    ) -> Result<(), Incompatibility> {
        let one_geometry = one.geometry().map_err(|_| Incompatibility::Geometry)?;
        let two_geometry = two.geometry().map_err(|_| Incompatibility::Geometry)?;
        let one_start = one_geometry.start();
        let one_stop = one_geometry.stop();
        let two_start = two_geometry.start();
        let two_stop = two_geometry.stop();

        let overlap_start = one_start.0.max(two_start.0);
        let overlap_stop = one_stop.0.min(two_stop.0);
        let coordinate_gap = (overlap_start - overlap_stop) - 1.0;
        if coordinate_gap > self.params.max_coord_gap {
            return Err(Incompatibility::CoordinateGap);
        }
        if coordinate_gap < 0.0 {
            let overlap = overlap.ok_or(Incompatibility::OverlapRequiresThicknessProbe)?;
            let max_consistent_thickness = self.max_consistent_thickness(one, overlap)?;
            let max_space = if expanding {
                overlap.max_expansion_space
            } else {
                overlap.max_overlap_space
            };
            // Java truncates this positive value when casting to int.
            let value_count = (3.0_f64.min(1.0 - (coordinate_gap / 10.0))) as usize;

            for index in 1..=value_count {
                let coordinate =
                    overlap_start - ((index as f64 * coordinate_gap) / (value_count + 1) as f64);
                let one_position = one_geometry
                    .position_at(coordinate)
                    .map_err(|_| Incompatibility::Geometry)?;
                let two_position = two_geometry
                    .position_at(coordinate)
                    .map_err(|_| Incompatibility::Geometry)?;
                if (one_position - two_position).abs() > overlap.max_overlap_delta_pos {
                    return Err(Incompatibility::OverlapPosition);
                }

                let thickness = thickness_at(coordinate, overlap.probe_width, &[one, two])?;
                if thickness > overlap.max_thickness {
                    return Err(Incompatibility::Thickness);
                }
                if -coordinate_gap <= overlap.max_involving_length
                    && thickness > max_consistent_thickness
                {
                    return Err(Incompatibility::InconsistentThickness);
                }

                let space = thickness
                    - thickness_at(coordinate, overlap.probe_width, &[one])?
                    - thickness_at(coordinate, overlap.probe_width, &[two])?;
                if space > max_space {
                    return Err(Incompatibility::OverlapSpace);
                }
                if expanding
                    && max_space == 0.0
                    && !filament_touches_section(one, &two.sections()[0])
                {
                    return Err(Incompatibility::NoContact);
                }
            }
        } else {
            let (gap_start, gap_stop) = if one_start.0 < two_start.0 {
                (one_stop, two_start)
            } else {
                (two_stop, one_start)
            };
            let one_bounds = one.bounds().map_err(|_| Incompatibility::Geometry)?;
            let two_bounds = two.bounds().map_err(|_| Incompatibility::Geometry)?;
            let one_thickness = one.weight() as f64 / one_bounds.width as f64;
            let two_thickness = two.weight() as f64 / two_bounds.width as f64;
            let position_margin = (one_thickness.max(two_thickness) / 2.0).round_ties_even();
            let position_gap = (gap_stop.1 - gap_start.1).abs() - position_margin;
            if position_gap > self.params.max_pos_gap {
                return Err(Incompatibility::PositionGap);
            }
            if position_gap > self.params.max_pos_gap_for_slope
                && position_gap / coordinate_gap > self.params.max_gap_slope
            {
                return Err(Incompatibility::GapSlope);
            }
        }

        let one_length = one_stop.0 - one_start.0 + 1.0;
        let two_length = two_stop.0 - two_start.0 + 1.0;
        if one_length >= self.params.min_length_for_delta_slope
            && two_length >= self.params.min_length_for_delta_slope
        {
            let one_slope = (one_stop.1 - one_start.1) / (one_stop.0 - one_start.0);
            let two_slope = (two_stop.1 - two_start.1) / (two_stop.0 - two_start.0);
            if (two_slope - one_slope).abs() > self.params.max_delta_slope {
                return Err(Incompatibility::DeltaSlope);
            }
        }

        Ok(())
    }

    fn max_consistent_thickness(
        &self,
        filament: &StaffFilament,
        overlap: OverlapParams,
    ) -> Result<f64, Incompatibility> {
        let mean = filament.weight() as f64
            / filament
                .bounds()
                .map_err(|_| Incompatibility::Geometry)?
                .width as f64;
        Ok(if mean < 2.0 {
            2.0 * overlap.max_consistent_ratio * mean
        } else {
            overlap.max_consistent_ratio * mean
        })
    }

    /// Build one filament per core section and merge compatible real-gap candidates.
    ///
    /// Input order is retained for equal lengths, matching Java's stable sort.
    pub fn retrieve_core_filaments(
        &self,
        sections: &[Section],
    ) -> Result<Vec<StaffFilament>, FilamentError> {
        self.retrieve_core_filaments_internal(sections, None)
    }

    /// Build and merge core filaments with Java's overlap probes enabled.
    pub fn retrieve_core_filaments_with_overlap(
        &self,
        sections: &[Section],
        overlap: OverlapParams,
    ) -> Result<Vec<StaffFilament>, FilamentError> {
        self.retrieve_core_filaments_internal(sections, Some(overlap))
    }

    /// Run Java's two merge passes around horizontal leftover-section expansion.
    pub fn retrieve_filaments(
        &self,
        sections: &[Section],
        overlap: OverlapParams,
    ) -> Result<Vec<StaffFilament>, FilamentError> {
        let mut filaments = Vec::new();
        for section in sections.iter().filter(|section| {
            section.orientation() == Orientation::Horizontal
                && section.length(Orientation::Horizontal) >= self.params.min_core_section_length
                && !self.is_section_fat(section, overlap)
        }) {
            let mut filament = StaffFilament::new(self.params.interline)?;
            filament.add_section(section.clone())?;
            filaments.push(filament);
        }
        self.merge_filaments(&mut filaments, Some(overlap))?;
        self.expand_filaments(&mut filaments, sections, overlap)?;
        self.merge_filaments(&mut filaments, Some(overlap))?;
        Ok(filaments)
    }

    /// Java `retrieveFilaments` with sheet-global creation identities.
    ///
    /// `first_creation_id` is one greater than the current
    /// `FilamentIndex.getLastId()`. Every accepted initial core and every
    /// temporary leftover candidate consumes an ID immediately, before merge
    /// or expansion can swallow it. Final survivors therefore retain gaps.
    /// The existing value-only [`Self::retrieve_filaments`] API remains
    /// available for callers that do not own sheet index state.
    pub fn retrieve_filaments_with_ids(
        &self,
        sections: &[Section],
        overlap: OverlapParams,
        first_creation_id: u64,
    ) -> Result<IdentifiedFilamentResult, FilamentFactoryIdentityError> {
        if first_creation_id == 0 {
            return Err(FilamentFactoryIdentityError::InvalidFirstId);
        }
        let mut next_creation_id = first_creation_id;
        let mut creation_ids = Vec::new();
        let mut filaments = Vec::new();
        for section in sections.iter().filter(|section| {
            section.orientation() == Orientation::Horizontal
                && section.length(Orientation::Horizontal) >= self.params.min_core_section_length
                && !self.is_section_fat(section, overlap)
        }) {
            let id = allocate_filament_id(&mut next_creation_id, &mut creation_ids)?;
            let mut filament = StaffFilament::new(self.params.interline)?;
            filament.add_section(section.clone())?;
            filaments.push(IdentifiedFilament { id, filament });
        }
        self.merge_identified_filaments(&mut filaments, Some(overlap))?;
        self.expand_identified_filaments(
            &mut filaments,
            sections,
            overlap,
            &mut next_creation_id,
            &mut creation_ids,
        )?;
        self.merge_identified_filaments(&mut filaments, Some(overlap))?;
        Ok(IdentifiedFilamentResult {
            survivors: filaments,
            creation_ids,
            next_creation_id,
        })
    }

    fn expand_identified_filaments(
        &self,
        filaments: &mut [IdentifiedFilament],
        source: &[Section],
        overlap: OverlapParams,
        next_creation_id: &mut u64,
        creation_ids: &mut Vec<u64>,
    ) -> Result<(), FilamentFactoryIdentityError> {
        let mut leftovers = source
            .iter()
            .filter(|section| {
                section.orientation() == Orientation::Horizontal
                    && !filaments
                        .iter()
                        .any(|entry| entry.filament.sections().contains(section))
                    && !self.is_section_fat(section, overlap)
            })
            .cloned()
            .collect::<Vec<_>>();
        leftovers.sort_by_key(Section::first_pos);

        let mut section_filaments = Vec::with_capacity(leftovers.len());
        for section in leftovers {
            let id = allocate_filament_id(next_creation_id, creation_ids)?;
            let mut filament = StaffFilament::new(self.params.interline)?;
            filament.add_section(section)?;
            section_filaments.push(IdentifiedFilament { id, filament });
        }

        filaments.sort_by_key(|entry| {
            std::cmp::Reverse(entry.filament.bounds().map_or(0, |bounds| bounds.width))
        });
        for entry in filaments {
            let fixed_bounds = entry.filament.bounds()?;
            loop {
                let mut compatible_index = None;
                for (index, candidate) in section_filaments.iter().enumerate() {
                    if grown_bounds_intersect(
                        fixed_bounds,
                        candidate.filament.bounds()?,
                        self.params.max_coord_gap,
                        self.params.max_pos_gap,
                    ) && self
                        .compatibility(&entry.filament, &candidate.filament, Some(overlap), true)
                        .is_ok()
                    {
                        compatible_index = Some(index);
                        break;
                    }
                }
                let Some(index) = compatible_index else {
                    break;
                };
                let candidate = section_filaments.remove(index);
                for section in candidate.filament.sections().iter().cloned() {
                    entry.filament.add_section(section)?;
                }
            }
        }
        Ok(())
    }

    fn merge_identified_filaments(
        &self,
        filaments: &mut Vec<IdentifiedFilament>,
        overlap: Option<OverlapParams>,
    ) -> Result<(), FilamentError> {
        filaments.sort_by_key(|entry| {
            std::cmp::Reverse(entry.filament.bounds().map_or(0, |bounds| bounds.width))
        });

        let mut live = filaments.drain(..).map(Some).collect::<Vec<_>>();
        let mut live_bounds = live
            .iter()
            .map(|entry| {
                entry
                    .as_ref()
                    .map(|entry| entry.filament.bounds())
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>()?;
        // At high scan resolutions, testing every earlier filament makes this
        // stage quadratic in thousands of short horizontal sections. Index
        // live heads by vertical band, then retain the original ascending head
        // order within the small set whose bounds can actually intersect.
        // Stale bucket entries after a merge are harmless: current bounds and
        // liveness are still checked before compatibility.
        let bucket_height = self.params.max_pos_gap.ceil().max(1.0) as usize;
        let mut heads_by_y = Vec::<Vec<usize>>::new();
        for current in 0..live.len() {
            if live[current].is_none() {
                continue;
            }
            let mut candidate = current;
            loop {
                let mut merged_into = None;
                let candidate_bounds = live_bounds[candidate].expect("live candidate bounds");
                let mut possible_heads = indexed_vertical_candidates(
                    &heads_by_y,
                    candidate_bounds,
                    self.params.max_pos_gap,
                    bucket_height,
                );
                possible_heads.sort_unstable();
                possible_heads.dedup();
                for head in possible_heads {
                    if head >= current {
                        continue;
                    }
                    if head == candidate || live[head].is_none() {
                        continue;
                    }
                    let head_bounds = live_bounds[head].expect("live head bounds");
                    if !grown_bounds_intersect(
                        candidate_bounds,
                        head_bounds,
                        self.params.max_coord_gap,
                        self.params.max_pos_gap,
                    ) {
                        continue;
                    }
                    let compatible = self
                        .compatibility(
                            &live[head].as_ref().expect("live head").filament,
                            &live[candidate].as_ref().expect("live candidate").filament,
                            overlap,
                            false,
                        )
                        .is_ok();
                    if compatible {
                        let sections = live[candidate]
                            .as_ref()
                            .expect("live candidate")
                            .filament
                            .sections()
                            .to_vec();
                        for section in sections {
                            live[head]
                                .as_mut()
                                .expect("live head")
                                .filament
                                .add_section(section)?;
                        }
                        live[candidate] = None;
                        live_bounds[candidate] = None;
                        let updated = live[head]
                            .as_ref()
                            .expect("merged live head")
                            .filament
                            .bounds()?;
                        live_bounds[head] = Some(updated);
                        index_vertical_head(&mut heads_by_y, head, updated, bucket_height);
                        merged_into = Some(head);
                        break;
                    }
                }
                let Some(head) = merged_into else { break };
                candidate = head;
            }
            if live[current].is_some() {
                index_vertical_head(
                    &mut heads_by_y,
                    current,
                    live_bounds[current].expect("surviving filament bounds"),
                    bucket_height,
                );
            }
        }
        filaments.extend(live.into_iter().flatten());
        Ok(())
    }

    /// Attach unprocessed, non-fat horizontal source sections to existing filaments.
    ///
    /// The returned count is the number of attached leftovers. This ports
    /// Java's stable `Section.byPosition` ordering, stable reverse-length
    /// filament ordering, fixed grown-box prefilter, and repeated first-match
    /// scan. The grown box intentionally is not recomputed after attachment.
    /// "Processed" means present in the supplied filaments; unlike Java's
    /// stateful factory, this value type does not retain processed/fat caches
    /// across separate calls.
    pub fn expand_filaments(
        &self,
        filaments: &mut [StaffFilament],
        source: &[Section],
        overlap: OverlapParams,
    ) -> Result<usize, FilamentError> {
        let mut leftovers: Vec<Section> = source
            .iter()
            .filter(|section| {
                section.orientation() == Orientation::Horizontal
                    && !filaments
                        .iter()
                        .any(|filament| filament.sections().contains(section))
                    && !self.is_section_fat(section, overlap)
            })
            .cloned()
            .collect();
        // Stable, and deliberately no coordinate tie-breaker: Java's
        // Section.byPosition returns equality for equal first positions.
        leftovers.sort_by_key(Section::first_pos);

        let mut section_filaments = Vec::with_capacity(leftovers.len());
        for section in leftovers {
            let mut filament = StaffFilament::new(self.params.interline)?;
            filament.add_section(section)?;
            section_filaments.push(filament);
        }

        filaments.sort_by(|one, two| {
            let one_length = one.bounds().map_or(0, |bounds| bounds.width);
            let two_length = two.bounds().map_or(0, |bounds| bounds.width);
            two_length.cmp(&one_length)
        });

        let mut attached = 0;
        for filament in filaments {
            // Java computes this once, before the repeated attachment loop.
            let fixed_bounds = filament.bounds()?;
            loop {
                let mut compatible_index = None;
                for (index, candidate) in section_filaments.iter().enumerate() {
                    if grown_bounds_intersect(
                        fixed_bounds,
                        candidate.bounds()?,
                        self.params.max_coord_gap,
                        self.params.max_pos_gap,
                    ) && self
                        .compatibility(filament, candidate, Some(overlap), true)
                        .is_ok()
                    {
                        compatible_index = Some(index);
                        break;
                    }
                }
                let Some(index) = compatible_index else {
                    break;
                };
                let candidate = section_filaments.remove(index);
                for section in candidate.sections().iter().cloned() {
                    filament.add_section(section)?;
                }
                attached += 1;
            }
        }
        Ok(attached)
    }

    fn retrieve_core_filaments_internal(
        &self,
        sections: &[Section],
        overlap: Option<OverlapParams>,
    ) -> Result<Vec<StaffFilament>, FilamentError> {
        let mut filaments = Vec::new();
        for section in sections
            .iter()
            .filter(|section| self.is_core_candidate(section))
        {
            let mut filament = StaffFilament::new(self.params.interline)?;
            filament.add_section(section.clone())?;
            filaments.push(filament);
        }

        self.merge_filaments(&mut filaments, overlap)?;
        Ok(filaments)
    }

    fn merge_filaments(
        &self,
        filaments: &mut Vec<StaffFilament>,
        overlap: Option<OverlapParams>,
    ) -> Result<(), FilamentError> {
        // Rust's slice sort is stable, as is Collections.sort used by Java.
        filaments.sort_by(|one, two| {
            let one_length = one.bounds().map_or(0, |bounds| bounds.width);
            let two_length = two.bounds().map_or(0, |bounds| bounds.width);
            two_length.cmp(&one_length)
        });

        let mut live: Vec<Option<StaffFilament>> = filaments.drain(..).map(Some).collect();
        for current in 0..live.len() {
            if live[current].is_none() {
                continue;
            }
            let mut candidate = current;
            loop {
                let mut merged_into = None;
                for head in 0..current {
                    if head == candidate || live[head].is_none() {
                        continue;
                    }
                    let head_bounds = live[head].as_ref().expect("live head").bounds()?;
                    let candidate_bounds =
                        live[candidate].as_ref().expect("live candidate").bounds()?;
                    if !grown_bounds_intersect(
                        candidate_bounds,
                        head_bounds,
                        self.params.max_coord_gap,
                        self.params.max_pos_gap,
                    ) {
                        continue;
                    }
                    let compatible = self
                        .compatibility(
                            live[head].as_ref().expect("live head"),
                            live[candidate].as_ref().expect("live candidate"),
                            overlap,
                            false,
                        )
                        .is_ok();
                    if compatible {
                        let sections = live[candidate]
                            .as_ref()
                            .expect("live candidate")
                            .sections()
                            .to_vec();
                        for section in sections {
                            live[head]
                                .as_mut()
                                .expect("live head")
                                .add_section(section)?;
                        }
                        live[candidate] = None;
                        merged_into = Some(head);
                        break;
                    }
                }
                let Some(head) = merged_into else { break };
                candidate = head;
            }
        }

        filaments.extend(live.into_iter().flatten());
        Ok(())
    }
}

fn allocate_filament_id(
    next_creation_id: &mut u64,
    creation_ids: &mut Vec<u64>,
) -> Result<u64, FilamentFactoryIdentityError> {
    let id = *next_creation_id;
    *next_creation_id = id
        .checked_add(1)
        .ok_or(FilamentFactoryIdentityError::IdentityOverflow)?;
    creation_ids.push(id);
    Ok(id)
}

#[derive(Clone, Copy)]
struct FittedLine {
    slope: f64,
    intercept: Option<f64>,
    vertical_b: Option<(f64, f64)>,
}

impl FittedLine {
    fn slope(self) -> f64 {
        self.slope
    }

    fn y_at_x(self, x: f64) -> Option<f64> {
        if let Some(intercept) = self.intercept {
            Some((self.slope * x) + intercept)
        } else {
            let (b, c) = self.vertical_b?;
            (b != 0.0).then(|| (x - c) / b)
        }
    }
}

/// Pixel regression used by Java `BasicSection.getAbsoluteLine`.
fn fitted_section_line(section: &Section) -> Option<FittedLine> {
    let mut count = 0.0;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_x2 = 0.0;
    let mut sum_y2 = 0.0;
    let mut sum_xy = 0.0;
    for (offset, run) in section.runs().iter().enumerate() {
        let y = (section.first_pos() + offset) as f64;
        for x in run.start..=run.stop() {
            let x = x as f64;
            count += 1.0;
            sum_x += x;
            sum_y += y;
            sum_x2 += x * x;
            sum_y2 += y * y;
            sum_xy += x * y;
        }
    }
    if count < 2.0 {
        return None;
    }
    let horizontal_denominator = (count * sum_x2) - (sum_x * sum_x);
    let vertical_denominator = (count * sum_y2) - (sum_y * sum_y);
    let numerator = (count * sum_xy) - (sum_x * sum_y);
    if horizontal_denominator.abs() >= vertical_denominator.abs() {
        if horizontal_denominator == 0.0 {
            return None;
        }
        let slope = numerator / horizontal_denominator;
        let intercept = ((sum_y * sum_x2) - (sum_x * sum_xy)) / horizontal_denominator;
        Some(FittedLine {
            slope,
            intercept: Some(intercept),
            vertical_b: None,
        })
    } else {
        if vertical_denominator == 0.0 {
            return None;
        }
        // Java's vertical fit is -x + b*y + c = 0.
        let b = numerator / vertical_denominator;
        let c = ((sum_x * sum_y2) - (sum_y * sum_xy)) / vertical_denominator;
        Some(FittedLine {
            slope: 1.0 / b,
            intercept: None,
            vertical_b: Some((b, c)),
        })
    }
}

fn section_pixel_count_in_signed(
    section: &Section,
    x_start: isize,
    x_stop: isize,
    y_start: isize,
    y_stop: isize,
) -> usize {
    let mut count = 0;
    for (offset, run) in section.runs().iter().enumerate() {
        let y = (section.first_pos() + offset) as isize;
        if y < y_start || y >= y_stop {
            continue;
        }
        let start = (run.start as isize).max(x_start);
        let stop = (run.stop() as isize + 1).min(x_stop);
        if start < stop {
            count += (stop - start) as usize;
        }
    }
    count
}

/// Java `Rectangle.grow(...); Rectangle.intersects(...)` in half-open form.
fn grown_bounds_intersect(
    grown: Bounds,
    other: Bounds,
    coordinate_margin: f64,
    position_margin: f64,
) -> bool {
    let left = grown.x as f64 - coordinate_margin;
    let right = (grown.x + grown.width) as f64 + coordinate_margin;
    let top = grown.y as f64 - position_margin;
    let bottom = (grown.y + grown.height) as f64 + position_margin;
    let other_right = (other.x + other.width) as f64;
    let other_bottom = (other.y + other.height) as f64;
    right > other.x as f64 && other_right > left && bottom > other.y as f64 && other_bottom > top
}

fn vertical_bucket_span(bounds: Bounds, margin: f64, bucket_height: usize) -> (usize, usize) {
    let start = ((bounds.y as f64 - margin).floor().max(0.0) as usize) / bucket_height;
    // Deliberately over-include the half-open lower edge. Exact intersection
    // is still decided by `grown_bounds_intersect`; the index must only avoid
    // false negatives.
    let stop = (((bounds.y + bounds.height) as f64 + margin).ceil() as usize) / bucket_height;
    (start, stop)
}

fn index_vertical_head(
    heads_by_y: &mut Vec<Vec<usize>>,
    head: usize,
    bounds: Bounds,
    bucket_height: usize,
) {
    let (start, stop) = vertical_bucket_span(bounds, 0.0, bucket_height);
    if heads_by_y.len() <= stop {
        heads_by_y.resize_with(stop + 1, Vec::new);
    }
    for bucket in &mut heads_by_y[start..=stop] {
        if let Err(position) = bucket.binary_search(&head) {
            bucket.insert(position, head);
        }
    }
}

fn indexed_vertical_candidates(
    heads_by_y: &[Vec<usize>],
    bounds: Bounds,
    margin: f64,
    bucket_height: usize,
) -> Vec<usize> {
    if heads_by_y.is_empty() {
        return Vec::new();
    }
    let (start, stop) = vertical_bucket_span(bounds, margin, bucket_height);
    if start >= heads_by_y.len() {
        return Vec::new();
    }
    heads_by_y[start..=stop.min(heads_by_y.len() - 1)]
        .iter()
        .flatten()
        .copied()
        .collect()
}

/// Java `Compounds.getThicknessAt` for horizontal staff filaments.
fn thickness_at(
    coordinate: f64,
    probe_width: usize,
    filaments: &[&StaffFilament],
) -> Result<f64, Incompatibility> {
    let mut min_x = usize::MAX;
    let mut max_x = 0_usize;
    for filament in filaments {
        let bounds = filament.bounds().map_err(|_| Incompatibility::Geometry)?;
        min_x = min_x.min(bounds.x);
        max_x = max_x.max(bounds.x + bounds.width - 1);
    }

    let integer_coordinate = coordinate.floor() as isize;
    if integer_coordinate < min_x as isize || integer_coordinate > max_x as isize {
        return Ok(0.0);
    }

    // Java creates a zero-width Rectangle then grows it by probeWidth / 2.
    // Rectangle's right edge is exclusive, so the resulting integer window is
    // [coord-half, coord+half), and a zero half-width collects no pixels.
    let half_width = (probe_width / 2) as isize;
    if half_width == 0 {
        return Ok(0.0);
    }
    let probe_start = integer_coordinate - half_width;
    let probe_stop = integer_coordinate + half_width;
    let mut min_position = usize::MAX;
    let mut max_position = 0_usize;
    let mut found = false;

    for filament in filaments {
        for section in filament.sections() {
            for (offset, run) in section.runs().iter().enumerate() {
                let run_start = run.start as isize;
                let run_stop = run.stop() as isize + 1;
                if run_start < probe_stop && run_stop > probe_start {
                    let position = section.first_pos() + offset;
                    min_position = min_position.min(position);
                    max_position = max_position.max(position);
                    found = true;
                }
            }
        }
    }

    Ok(if found {
        (max_position - min_position + 1) as f64
    } else {
        0.0
    })
}

fn filament_touches_section(filament: &StaffFilament, candidate: &Section) -> bool {
    filament
        .sections()
        .iter()
        .any(|member| member.touches(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_table::{Run, RunTable};
    use crate::section::{JunctionPolicy, build_sections};

    fn params() -> FilamentFactoryParams {
        FilamentFactoryParams {
            interline: 2,
            min_core_section_length: 6,
            min_section_aspect: 3.0,
            max_coord_gap: 3.0,
            max_pos_gap: 2.0,
            max_pos_gap_for_slope: 0.5,
            max_gap_slope: 0.5,
            min_length_for_delta_slope: 100.0,
            max_delta_slope: 0.01,
        }
    }

    fn section(x: usize, y: usize, length: usize, thickness: usize) -> Section {
        let mut table =
            RunTable::new(Orientation::Horizontal, x + length + 1, y + thickness + 1).unwrap();
        for row in y..(y + thickness) {
            table.add_run(row, Run::new(x, length)).unwrap();
        }
        build_sections(&table, JunctionPolicy::All).remove(0)
    }

    fn filament(sections: impl IntoIterator<Item = Section>) -> StaffFilament {
        let mut filament = StaffFilament::new(2).unwrap();
        for section in sections {
            filament.add_section(section).unwrap();
        }
        filament
    }

    fn overlap_params() -> OverlapParams {
        OverlapParams {
            probe_width: 2,
            max_overlap_delta_pos: 2.0,
            max_thickness: 4.0,
            max_overlap_space: 1.0,
            max_expansion_space: 0.0,
            max_involving_length: 10.0,
            max_consistent_ratio: 1.7,
        }
    }

    #[test]
    fn core_filter_matches_length_and_global_aspect_boundaries() {
        let factory = FilamentFactory::new(params());
        assert!(factory.is_core_candidate(&section(0, 2, 6, 1)));
        assert!(!factory.is_core_candidate(&section(0, 2, 5, 1)));
        assert!(factory.is_core_candidate(&section(0, 2, 6, 2)));
        assert!(!factory.is_core_candidate(&section(0, 2, 6, 3)));
    }

    #[test]
    fn gap_compatibility_includes_exact_coordinate_limit() {
        let factory = FilamentFactory::new(params());
        let make = |section| {
            let mut filament = StaffFilament::new(2).unwrap();
            filament.add_section(section).unwrap();
            filament
        };
        let left = make(section(0, 2, 8, 1));
        let at_limit = make(section(11, 2, 8, 1));
        let beyond = make(section(12, 2, 8, 1));
        assert_eq!(factory.gap_compatibility(&left, &at_limit), Ok(()));
        assert_eq!(
            factory.gap_compatibility(&left, &beyond),
            Err(Incompatibility::CoordinateGap)
        );
    }

    #[test]
    fn grouping_is_stable_for_equal_length_candidates() {
        let factory = FilamentFactory::new(params());
        let sections = [
            section(0, 2, 8, 1),
            section(10, 2, 8, 1),
            section(30, 8, 8, 1),
        ];
        let filaments = factory.retrieve_core_filaments(&sections).unwrap();

        assert_eq!(filaments.len(), 2);
        assert_eq!(
            filaments[0]
                .sections()
                .iter()
                .map(|section| section.bounds().x)
                .collect::<Vec<_>>(),
            [0, 10]
        );
        assert_eq!(filaments[1].sections()[0].bounds().x, 30);
    }

    #[test]
    fn overlap_is_deferred_without_resolved_thickness_parameters() {
        let factory = FilamentFactory::new(params());
        let make = |section| {
            let mut filament = StaffFilament::new(2).unwrap();
            filament.add_section(section).unwrap();
            filament
        };
        assert_eq!(
            factory.gap_compatibility(&make(section(0, 2, 8, 1)), &make(section(6, 2, 8, 1)),),
            Err(Incompatibility::OverlapRequiresThicknessProbe)
        );
    }

    #[test]
    fn overlap_probe_accepts_coincident_thin_filaments() {
        let factory = FilamentFactory::new(params());
        assert_eq!(
            factory.overlap_compatibility(
                &filament([section(0, 2, 8, 1)]),
                &filament([section(6, 2, 8, 1)]),
                overlap_params(),
                false,
            ),
            Ok(())
        );
    }

    #[test]
    fn overlap_probe_reports_java_checks_in_order() {
        let factory = FilamentFactory::new(params());
        let baseline = || filament([section(0, 2, 8, 1)]);

        let mut overlap = overlap_params();
        overlap.max_overlap_delta_pos = 0.5;
        assert_eq!(
            factory.overlap_compatibility(
                &baseline(),
                &filament([section(6, 3, 8, 1)]),
                overlap,
                false,
            ),
            Err(Incompatibility::OverlapPosition)
        );

        overlap.max_overlap_delta_pos = 2.0;
        overlap.max_thickness = 1.0;
        assert_eq!(
            factory.overlap_compatibility(
                &baseline(),
                &filament([section(6, 3, 8, 1)]),
                overlap,
                false,
            ),
            Err(Incompatibility::Thickness)
        );

        overlap.max_thickness = 4.0;
        overlap.max_consistent_ratio = 0.4;
        assert_eq!(
            factory.overlap_compatibility(
                &baseline(),
                &filament([section(6, 3, 8, 1)]),
                overlap,
                false,
            ),
            Err(Incompatibility::InconsistentThickness)
        );

        overlap.max_consistent_ratio = 1.7;
        overlap.max_involving_length = 0.0;
        overlap.max_overlap_delta_pos = 3.0;
        overlap.max_overlap_space = 0.0;
        assert_eq!(
            factory.overlap_compatibility(
                &baseline(),
                &filament([section(6, 4, 8, 1)]),
                overlap,
                false,
            ),
            Err(Incompatibility::OverlapSpace)
        );
    }

    #[test]
    fn zero_space_expansion_requires_contact_with_first_section() {
        let factory = FilamentFactory::new(params());
        let one = filament([section(0, 2, 21, 1)]);
        let two = filament([section(0, 10, 2, 1), section(2, 2, 19, 1)]);
        let mut overlap = overlap_params();
        overlap.max_overlap_delta_pos = 20.0;
        overlap.max_involving_length = 0.0;
        overlap.max_thickness = 20.0;

        assert_eq!(
            factory.overlap_compatibility(&one, &two, overlap, true),
            Err(Incompatibility::NoContact)
        );
    }

    #[test]
    fn overlap_enabled_grouping_merges_sections_that_the_legacy_entry_defers() {
        let factory = FilamentFactory::new(params());
        let sections = [section(0, 2, 8, 1), section(6, 2, 8, 1)];
        assert_eq!(factory.retrieve_core_filaments(&sections).unwrap().len(), 2);
        let merged = factory
            .retrieve_core_filaments_with_overlap(&sections, overlap_params())
            .unwrap();
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].sections().len(), 2);
    }

    #[test]
    fn local_half_probes_reject_a_uniformly_thick_section() {
        let factory = FilamentFactory::new(params());
        let thick = section(0, 2, 12, 3);
        let mut overlap = overlap_params();
        overlap.max_thickness = 1.0;
        assert!(factory.is_section_fat(&thick, overlap));

        overlap.max_thickness = 3.0;
        assert!(!factory.is_section_fat(&thick, overlap));
        assert!(!factory.is_section_fat(&section(0, 2, 8, 1), overlap));
    }

    #[test]
    fn expansion_prefers_longer_filament_and_repeats_first_match() {
        let factory = FilamentFactory::new(params());
        let long = section(0, 2, 10, 1);
        let short = section(0, 2, 8, 1);
        let first = section(10, 2, 5, 1);
        let second = section(11, 2, 5, 1);
        let source = [long.clone(), short.clone(), first, second];
        let mut filaments = vec![filament([short]), filament([long])];

        assert_eq!(
            factory
                .expand_filaments(&mut filaments, &source, overlap_params())
                .unwrap(),
            2
        );
        assert_eq!(filaments[0].sections().len(), 3);
        assert_eq!(filaments[1].sections().len(), 1);
    }

    #[test]
    fn expansion_keeps_the_original_grown_box_after_attachment() {
        let factory = FilamentFactory::new(params());
        let core = section(10, 2, 8, 1);
        let reachable = section(5, 2, 5, 1);
        let chained_only = section(0, 2, 5, 1);
        let source = [core.clone(), chained_only, reachable];
        let mut filaments = vec![filament([core])];

        assert_eq!(
            factory
                .expand_filaments(&mut filaments, &source, overlap_params())
                .unwrap(),
            1
        );
        assert_eq!(
            filaments[0]
                .sections()
                .iter()
                .map(|section| section.bounds().x)
                .collect::<Vec<_>>(),
            [5, 10]
        );
    }

    #[test]
    fn complete_retrieval_merges_again_after_expansion() {
        let factory = FilamentFactory::new(params());
        let source = [
            section(0, 2, 8, 1),
            section(18, 2, 8, 1),
            section(8, 2, 5, 1),
            section(13, 2, 5, 1),
        ];
        let filaments = factory
            .retrieve_filaments(&source, overlap_params())
            .unwrap();

        assert_eq!(filaments.len(), 1);
        assert_eq!(filaments[0].sections().len(), 4);
        assert_eq!(filaments[0].bounds().unwrap().width, 26);
    }

    #[test]
    fn identity_result_keeps_swallowed_gap_and_temporary_creation_ids() {
        let factory = FilamentFactory::new(params());
        let source = [
            section(0, 2, 8, 1),
            section(18, 2, 8, 1),
            section(50, 8, 8, 1),
            section(8, 2, 5, 1),
            section(13, 2, 5, 1),
        ];
        let result = factory
            .retrieve_filaments_with_ids(&source, overlap_params(), 41)
            .unwrap();
        let value_only = factory
            .retrieve_filaments(&source, overlap_params())
            .unwrap();

        // Initial cores consume 41,42,43. Expansion candidates consume 44,45.
        assert_eq!(result.creation_ids(), [41, 42, 43, 44, 45]);
        assert_eq!(result.next_creation_id(), 46);
        assert_eq!(
            result
                .survivors()
                .iter()
                .map(IdentifiedFilament::id)
                .collect::<Vec<_>>(),
            [41, 43]
        );
        assert_eq!(result.survivors()[0].filament().sections().len(), 4);
        assert_eq!(result.survivors()[0].filament().bounds().unwrap().width, 26);
        assert_eq!(result.survivors()[1].filament().sections().len(), 1);
        assert_eq!(
            result
                .survivors()
                .iter()
                .map(|entry| entry.filament().sections().to_vec())
                .collect::<Vec<_>>(),
            value_only
                .iter()
                .map(|filament| filament.sections().to_vec())
                .collect::<Vec<_>>()
        );
    }
}
