// SPDX-License-Identifier: AGPL-3.0-or-later

//! Neutral core-section grouping for horizontal staff filaments.
//!
//! This ports the deterministic core filtering, reverse-length ordering, and
//! real-gap branch of Java `FilamentFactory`. Overlapping-filament thickness
//! probes, expansion by leftover short sections, glyph ownership, and indexes
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

/// Why two candidates were not grouped by the supported compatibility branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Incompatibility {
    Geometry,
    OverlapRequiresThicknessProbe,
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
            return Err(Incompatibility::OverlapRequiresThicknessProbe);
        }

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

    /// Build one filament per core section and merge compatible real-gap candidates.
    ///
    /// Input order is retained for equal lengths, matching Java's stable sort.
    pub fn retrieve_core_filaments(
        &self,
        sections: &[Section],
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
                        .gap_compatibility(
                            live[head].as_ref().expect("live head"),
                            live[candidate].as_ref().expect("live candidate"),
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
    fn overlap_is_deferred_to_the_unported_thickness_probe() {
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
}
