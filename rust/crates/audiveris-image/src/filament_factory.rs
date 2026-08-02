// SPDX-License-Identifier: AGPL-3.0-or-later

//! Neutral core-section grouping for horizontal staff filaments.
//!
//! This ports the deterministic core filtering, reverse-length ordering, and
//! real-gap and overlapping-filament compatibility branches of Java
//! `FilamentFactory`. Expansion orchestration, glyph ownership, and indexes
//! remain outside this dependency-light slice.

use crate::filament::{FilamentError, StaffFilament};
use crate::run_table::Orientation;
use crate::section::Section;

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

        // Rust's slice sort is stable, as is Collections.sort used by Java.
        filaments.sort_by(|one, two| {
            let one_length = one.bounds().map_or(0, |bounds| bounds.width);
            let two_length = two.bounds().map_or(0, |bounds| bounds.width);
            two_length.cmp(&one_length)
        });

        let mut live: Vec<Option<StaffFilament>> = filaments.into_iter().map(Some).collect();
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

        Ok(live.into_iter().flatten().collect())
    }
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
}
