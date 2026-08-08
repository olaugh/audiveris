// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production seed/head-spot pools and semantic bands for HEADS scanning.
//!
//! Java builds each system pool in insertion order, stably sorts it by top
//! ordinate, intersects glyph bounds with the scanner `Area`, then stably
//! sorts the selected glyphs by left abscissa. This module materializes that
//! boundary from the three native products immediately available before image
//! lookup. Competitor and frozen-bar pools deliberately remain separate work.

use std::{error::Error, fmt};

use audiveris_image::{beam_structure::Segment, section::Bounds};

use crate::{
    head_scanner_slices::{HeadScannerBand, JavaRectangle},
    native_heads::NativeHeadsPrologRecognition,
    native_heads_scanner::{
        NativeHeadScannerLine, NativeHeadsScannerRecognition, NativeHeadsScannerSystem,
    },
    native_stem_seeds::NativeStemSeedRecognition,
};

/// Java `NoteHeadsBuilder.Constants.pitchMargin`.
pub const HEAD_SEED_PITCH_MARGIN: f64 = 0.75;
/// Java `HeadInter.Constants.shrinkVertRatio`.
pub const HEAD_COMPETITOR_VERTICAL_RATIO: f64 = 0.5;

/// One system's pools and per-staff scan slices, in Java system order.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadScannerPoolsRecognition {
    pub systems: Vec<NativeHeadScannerPoolSystem>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadScannerPoolSystem {
    pub system_id: usize,
    /// Accepted `VERTICAL_SEED` glyphs, stably sorted by top ordinate.
    pub seeds: Vec<NativeHeadScannerPoolRectangle>,
    /// Dispatched `HEAD_SPOT` glyphs, stably sorted by top ordinate.
    pub spots: Vec<NativeHeadScannerPoolRectangle>,
    pub staffs: Vec<NativeHeadScannerPoolStaff>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadScannerPoolStaff {
    pub staff_id: usize,
    /// Same order as `NativeHeadsScannerStaff.geometries`.
    pub scans: Vec<NativeHeadScannerPoolScan>,
}

/// A rectangle in one ordinate-sorted system pool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeHeadScannerPoolRectangle {
    /// Stem raw-candidate ordinal for seeds; sheet-global spot ordinal for
    /// head spots.
    pub source_ordinal: usize,
    pub bounds: JavaRectangle,
}

/// Semantic `Area`s and the currently available pool slices for one scanner.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadScannerPoolScan {
    pub geometry_ordinal: usize,
    pub seed_band: HeadScannerBand,
    pub competitor_band: HeadScannerBand,
    pub bar_band: HeadScannerBand,
    /// Indexes into `NativeHeadScannerPoolSystem.seeds`, in stable abscissa
    /// order.
    pub seed_indices: Vec<usize>,
    /// Indexes into `NativeHeadScannerPoolSystem.spots`, in stable abscissa
    /// order. Java uses the competitor band for this lookup.
    pub spot_indices: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeHeadScannerPoolError {
    SystemOrder {
        source: &'static str,
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    StaffOwner {
        system_id: usize,
        staff_id: usize,
        actual_system_id: usize,
    },
    InvalidSeed {
        system_id: usize,
        source_ordinal: usize,
    },
    SpotOrdinal {
        system_id: usize,
        spot_ordinal: usize,
    },
    MissingSpotComponent {
        system_id: usize,
        spot_ordinal: usize,
    },
    DuplicateSpot {
        system_id: usize,
        spot_ordinal: usize,
    },
    CoordinateOutOfRange {
        kind: &'static str,
        system_id: usize,
        source_ordinal: usize,
        field: &'static str,
    },
    ArithmeticOverflow {
        system_id: usize,
        staff_id: usize,
        geometry_ordinal: usize,
    },
}

impl fmt::Display for NativeHeadScannerPoolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemOrder {
                source,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS {source} system order {actual:?}, expected {expected:?}"
            ),
            Self::StaffOwner {
                system_id,
                staff_id,
                actual_system_id,
            } => write!(
                formatter,
                "HEADS system {system_id} staff {staff_id} claims system {actual_system_id}"
            ),
            Self::InvalidSeed {
                system_id,
                source_ordinal,
            } => write!(
                formatter,
                "HEADS system {system_id} seed {source_ordinal} is not a free vertical seed"
            ),
            Self::SpotOrdinal {
                system_id,
                spot_ordinal,
            } => write!(
                formatter,
                "HEADS system {system_id} references missing spot {spot_ordinal}"
            ),
            Self::MissingSpotComponent {
                system_id,
                spot_ordinal,
            } => write!(
                formatter,
                "HEADS system {system_id} spot {spot_ordinal} has no native component"
            ),
            Self::DuplicateSpot {
                system_id,
                spot_ordinal,
            } => write!(
                formatter,
                "HEADS system {system_id} references spot {spot_ordinal} more than once"
            ),
            Self::CoordinateOutOfRange {
                kind,
                system_id,
                source_ordinal,
                field,
            } => write!(
                formatter,
                "HEADS system {system_id} {kind} {source_ordinal} {field} is outside Java int"
            ),
            Self::ArithmeticOverflow {
                system_id,
                staff_id,
                geometry_ordinal,
            } => write!(
                formatter,
                "HEADS system {system_id} staff {staff_id} scanner {geometry_ordinal} band arithmetic overflowed"
            ),
        }
    }
}

impl Error for NativeHeadScannerPoolError {}

/// Materialize the native rectangle pools and all semantic bands available
/// before HEADS image lookup.
pub fn materialize_native_head_scanner_pools(
    stem_seeds: &NativeStemSeedRecognition,
    heads: &NativeHeadsPrologRecognition,
    scanners: &NativeHeadsScannerRecognition,
) -> Result<NativeHeadScannerPoolsRecognition, NativeHeadScannerPoolError> {
    let expected_system_ids = scanners
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    validate_system_order(
        "STEM_SEEDS",
        &expected_system_ids,
        stem_seeds.systems.iter().map(|system| system.raw.system_id),
    )?;
    validate_system_order(
        "head-spot",
        &expected_system_ids,
        heads
            .system_spot_ordinals
            .iter()
            .map(|(system_id, _)| *system_id),
    )?;

    let mut systems = Vec::with_capacity(scanners.systems.len());
    for ((scanner_system, seed_system), (_, spot_ordinals)) in scanners
        .systems
        .iter()
        .zip(&stem_seeds.systems)
        .zip(&heads.system_spot_ordinals)
    {
        let system_id = scanner_system.system_id;
        let mut seeds = seed_system
            .free_glyphs
            .iter()
            .map(|glyph| {
                if !glyph.vertical_seed_group || !glyph.free {
                    return Err(NativeHeadScannerPoolError::InvalidSeed {
                        system_id,
                        source_ordinal: glyph.source_ordinal,
                    });
                }
                Ok(NativeHeadScannerPoolRectangle {
                    source_ordinal: glyph.source_ordinal,
                    bounds: rectangle_from_bounds(
                        glyph.bounds,
                        "seed",
                        system_id,
                        glyph.source_ordinal,
                    )?,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        // `Collections.sort(systemSeeds, Glyphs.byOrdinate)` is stable and
        // compares only the top coordinate.
        sort_pool_by_ordinate(&mut seeds);

        let mut seen_spots = vec![false; heads.spots.len()];
        let mut spots = Vec::with_capacity(spot_ordinals.len());
        for &spot_ordinal in spot_ordinals {
            let spot =
                heads
                    .spots
                    .get(spot_ordinal)
                    .ok_or(NativeHeadScannerPoolError::SpotOrdinal {
                        system_id,
                        spot_ordinal,
                    })?;
            if std::mem::replace(&mut seen_spots[spot_ordinal], true) {
                return Err(NativeHeadScannerPoolError::DuplicateSpot {
                    system_id,
                    spot_ordinal,
                });
            }
            let component = spot.component.as_ref().ok_or(
                NativeHeadScannerPoolError::MissingSpotComponent {
                    system_id,
                    spot_ordinal,
                },
            )?;
            spots.push(NativeHeadScannerPoolRectangle {
                source_ordinal: spot_ordinal,
                bounds: rectangle_from_i32_origin(
                    component.left,
                    component.top,
                    component.width,
                    component.height,
                    "spot",
                    system_id,
                    spot_ordinal,
                )?,
            });
        }
        // Java mutates the dispatched system list with the same stable sort.
        sort_pool_by_ordinate(&mut spots);

        let staffs = materialize_staff_scans(scanner_system, &seeds, &spots)?;
        systems.push(NativeHeadScannerPoolSystem {
            system_id,
            seeds,
            spots,
            staffs,
        });
    }

    Ok(NativeHeadScannerPoolsRecognition { systems })
}

fn validate_system_order(
    source: &'static str,
    expected: &[usize],
    actual: impl Iterator<Item = usize>,
) -> Result<(), NativeHeadScannerPoolError> {
    let actual = actual.collect::<Vec<_>>();
    if actual != expected {
        return Err(NativeHeadScannerPoolError::SystemOrder {
            source,
            expected: expected.to_vec(),
            actual,
        });
    }
    Ok(())
}

fn materialize_staff_scans(
    system: &NativeHeadsScannerSystem,
    seeds: &[NativeHeadScannerPoolRectangle],
    spots: &[NativeHeadScannerPoolRectangle],
) -> Result<Vec<NativeHeadScannerPoolStaff>, NativeHeadScannerPoolError> {
    let mut staffs = Vec::with_capacity(system.staffs.len());
    for staff in &system.staffs {
        if staff.system_id != system.system_id {
            return Err(NativeHeadScannerPoolError::StaffOwner {
                system_id: system.system_id,
                staff_id: staff.staff_id,
                actual_system_id: staff.system_id,
            });
        }
        let mut scans = Vec::with_capacity(staff.geometries.len());
        for (geometry_ordinal, geometry) in staff.geometries.iter().enumerate() {
            let interline = f64::from(geometry.interline);
            let direction = f64::from(geometry.direction);
            let seed_above = (interline * (direction - HEAD_SEED_PITCH_MARGIN)) / 2.0;
            let seed_below = (interline * (direction + HEAD_SEED_PITCH_MARGIN)) / 2.0;
            let competitor_above = (interline * (direction - HEAD_COMPETITOR_VERTICAL_RATIO)) / 2.0;
            let competitor_below = (interline * (direction + HEAD_COMPETITOR_VERTICAL_RATIO)) / 2.0;
            // Java multiplies these two ints before division by the double 2.0.
            let middle = geometry
                .interline
                .checked_mul(geometry.direction)
                .map(|value| f64::from(value) / 2.0)
                .ok_or(NativeHeadScannerPoolError::ArithmeticOverflow {
                    system_id: system.system_id,
                    staff_id: staff.staff_id,
                    geometry_ordinal,
                })?;
            let bar_above = middle - system.parameters.vertical_bar_margin;
            let bar_below = middle + system.parameters.vertical_bar_margin;

            let seed_band = band_for_line(&geometry.line, seed_above, seed_below);
            let competitor_band = band_for_line(&geometry.line, competitor_above, competitor_below);
            let bar_band = band_for_line(&geometry.line, bar_above, bar_below);
            let seed_indices = slice_indices(seeds, &seed_band);
            let spot_indices = slice_indices(spots, &competitor_band);
            scans.push(NativeHeadScannerPoolScan {
                geometry_ordinal,
                seed_band,
                competitor_band,
                bar_band,
                seed_indices,
                spot_indices,
            });
        }
        staffs.push(NativeHeadScannerPoolStaff {
            staff_id: staff.staff_id,
            scans,
        });
    }
    Ok(staffs)
}

fn band_for_line(line: &NativeHeadScannerLine, above: f64, below: f64) -> HeadScannerBand {
    match line {
        NativeHeadScannerLine::Staff(line) => {
            HeadScannerBand::from_staff_boundary(line.boundary(), above, below)
        }
        NativeHeadScannerLine::Ledger(line) => {
            let axis = line.axis();
            HeadScannerBand::from_ledger_axis(
                Segment {
                    x1: axis.x1,
                    y1: axis.y1,
                    x2: axis.x2,
                    y2: axis.y2,
                },
                above,
                below,
            )
        }
    }
}

fn sort_pool_by_ordinate(pool: &mut [NativeHeadScannerPoolRectangle]) {
    pool.sort_by_key(|glyph| glyph.bounds.y);
}

fn slice_indices(pool: &[NativeHeadScannerPoolRectangle], band: &HeadScannerBand) -> Vec<usize> {
    let mut indices = pool
        .iter()
        .enumerate()
        .filter_map(|(index, glyph)| band.intersects_rectangle(glyph.bounds).then_some(index))
        .collect::<Vec<_>>();
    // `Collections.sort(slice, Glyphs.byAbscissa)` is stable and compares
    // only the left coordinate.
    indices.sort_by_key(|&index| pool[index].bounds.x);
    indices
}

fn rectangle_from_bounds(
    bounds: Bounds,
    kind: &'static str,
    system_id: usize,
    source_ordinal: usize,
) -> Result<JavaRectangle, NativeHeadScannerPoolError> {
    let x = i32::try_from(bounds.x)
        .map_err(|_| coordinate_error(kind, system_id, source_ordinal, "x"))?;
    let y = i32::try_from(bounds.y)
        .map_err(|_| coordinate_error(kind, system_id, source_ordinal, "y"))?;
    rectangle_from_i32_origin(
        x,
        y,
        bounds.width,
        bounds.height,
        kind,
        system_id,
        source_ordinal,
    )
}

#[allow(clippy::too_many_arguments)]
fn rectangle_from_i32_origin(
    x: i32,
    y: i32,
    width: usize,
    height: usize,
    kind: &'static str,
    system_id: usize,
    source_ordinal: usize,
) -> Result<JavaRectangle, NativeHeadScannerPoolError> {
    let width = i32::try_from(width)
        .map_err(|_| coordinate_error(kind, system_id, source_ordinal, "width"))?;
    let height = i32::try_from(height)
        .map_err(|_| coordinate_error(kind, system_id, source_ordinal, "height"))?;
    Ok(JavaRectangle::new(x, y, width, height))
}

const fn coordinate_error(
    kind: &'static str,
    system_id: usize,
    source_ordinal: usize,
    field: &'static str,
) -> NativeHeadScannerPoolError {
    NativeHeadScannerPoolError::CoordinateOutOfRange {
        kind,
        system_id,
        source_ordinal,
        field,
    }
}

#[cfg(test)]
mod tests {
    use audiveris_image::system_population::{BoundarySegment, StaffBoundary};

    use super::*;

    #[test]
    fn checked_bounds_conversion_preserves_java_rectangle_fields() {
        assert_eq!(
            rectangle_from_bounds(
                Bounds {
                    x: 12,
                    y: 34,
                    width: 5,
                    height: 67,
                },
                "seed",
                1,
                2,
            ),
            Ok(JavaRectangle::new(12, 34, 5, 67))
        );
        assert!(matches!(
            rectangle_from_i32_origin(0, 0, i32::MAX as usize + 1, 1, "spot", 1, 2),
            Err(NativeHeadScannerPoolError::CoordinateOutOfRange { field: "width", .. })
        ));
    }

    #[test]
    fn slice_is_intersected_then_stably_sorted_by_abscissa() {
        let band = HeadScannerBand::from_staff_boundary(
            &StaffBoundary {
                segments: vec![BoundarySegment::Line {
                    start: (0.0, 10.0),
                    end: (30.0, 10.0),
                }],
            },
            -2.0,
            1.0,
        );
        let pool = vec![
            NativeHeadScannerPoolRectangle {
                source_ordinal: 8,
                bounds: JavaRectangle::new(20, 9, 2, 2),
            },
            NativeHeadScannerPoolRectangle {
                source_ordinal: 9,
                bounds: JavaRectangle::new(5, 30, 2, 2),
            },
            NativeHeadScannerPoolRectangle {
                source_ordinal: 10,
                bounds: JavaRectangle::new(5, 9, 2, 2),
            },
            NativeHeadScannerPoolRectangle {
                source_ordinal: 11,
                bounds: JavaRectangle::new(5, 10, 2, 2),
            },
        ];
        assert_eq!(slice_indices(&pool, &band), vec![2, 3, 0]);
    }

    #[test]
    fn pool_is_stably_sorted_by_ordinate() {
        let mut pool = vec![
            NativeHeadScannerPoolRectangle {
                source_ordinal: 8,
                bounds: JavaRectangle::new(20, 9, 2, 2),
            },
            NativeHeadScannerPoolRectangle {
                source_ordinal: 9,
                bounds: JavaRectangle::new(5, 3, 2, 2),
            },
            NativeHeadScannerPoolRectangle {
                source_ordinal: 10,
                bounds: JavaRectangle::new(1, 9, 2, 2),
            },
        ];
        sort_pool_by_ordinate(&mut pool);
        assert_eq!(
            pool.iter()
                .map(|glyph| glyph.source_ordinal)
                .collect::<Vec<_>>(),
            vec![9, 8, 10]
        );
    }

    #[test]
    fn band_offsets_preserve_java_operation_order() {
        let interline = 42.0;
        let direction = -1.0;
        let seed_above = (interline * (direction - HEAD_SEED_PITCH_MARGIN)) / 2.0;
        let seed_below = (interline * (direction + HEAD_SEED_PITCH_MARGIN)) / 2.0;
        let competitor_above = (interline * (direction - HEAD_COMPETITOR_VERTICAL_RATIO)) / 2.0;
        let competitor_below = (interline * (direction + HEAD_COMPETITOR_VERTICAL_RATIO)) / 2.0;
        assert_eq!(seed_above.to_bits(), (-36.75_f64).to_bits());
        assert_eq!(seed_below.to_bits(), (-5.25_f64).to_bits());
        assert_eq!(competitor_above.to_bits(), (-31.5_f64).to_bits());
        assert_eq!(competitor_below.to_bits(), (-10.5_f64).to_bits());
    }
}
