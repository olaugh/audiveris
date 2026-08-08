// SPDX-License-Identifier: AGPL-3.0-or-later

//! Per-scanner frozen bar/connector slices for HEADS.
//!
//! Java `NoteHeadsBuilder.Scanner.getBarAreas` traverses the system's frozen
//! bar areas in their existing stable-by-ordinate order, retains an area when
//! the scanner band intersects that area's integer bounds, and performs no
//! subsequent abscissa sort. This module composes the independently owned
//! scanner bands and GRID obstacle pool at that exact boundary.

use std::{error::Error, fmt};

use crate::{
    native_heads_obstacles::NativeHeadsBarObstaclePool,
    native_heads_scanner_pools::NativeHeadScannerPoolsRecognition,
};

/// All per-scan bar slices, in Java system/staff/scanner order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHeadScannerBarSlicesRecognition {
    pub systems: Vec<NativeHeadScannerBarSliceSystem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHeadScannerBarSliceSystem {
    pub system_id: usize,
    pub staffs: Vec<NativeHeadScannerBarSliceStaff>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHeadScannerBarSliceStaff {
    pub staff_id: usize,
    pub scans: Vec<NativeHeadScannerBarSlice>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHeadScannerBarSlice {
    pub geometry_ordinal: usize,
    /// Indexes into the corresponding system's `frozen_by_ordinate` pool.
    /// Filtering preserves that pool order exactly; Java does not x-sort it.
    pub frozen_bar_indices: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeHeadScannerBarSliceError {
    SystemOrder {
        scanner_system_ids: Vec<usize>,
        obstacle_system_ids: Vec<usize>,
    },
}

impl fmt::Display for NativeHeadScannerBarSliceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemOrder {
                scanner_system_ids,
                obstacle_system_ids,
            } => write!(
                formatter,
                "HEADS scanner system order {scanner_system_ids:?}, but frozen obstacle order is \
                 {obstacle_system_ids:?}"
            ),
        }
    }
}

impl Error for NativeHeadScannerBarSliceError {}

/// Apply every scanner's semantic bar band to the frozen obstacle pool.
pub fn materialize_native_head_scanner_bar_slices(
    scanners: &NativeHeadScannerPoolsRecognition,
    obstacles: &NativeHeadsBarObstaclePool,
) -> Result<NativeHeadScannerBarSlicesRecognition, NativeHeadScannerBarSliceError> {
    let scanner_system_ids = scanners
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    let obstacle_system_ids = obstacles
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    if scanner_system_ids != obstacle_system_ids {
        return Err(NativeHeadScannerBarSliceError::SystemOrder {
            scanner_system_ids,
            obstacle_system_ids,
        });
    }

    let systems = scanners
        .systems
        .iter()
        .zip(&obstacles.systems)
        .map(
            |(scanner_system, obstacle_system)| NativeHeadScannerBarSliceSystem {
                system_id: scanner_system.system_id,
                staffs: scanner_system
                    .staffs
                    .iter()
                    .map(|staff| NativeHeadScannerBarSliceStaff {
                        staff_id: staff.staff_id,
                        scans: staff
                            .scans
                            .iter()
                            .map(|scan| NativeHeadScannerBarSlice {
                                geometry_ordinal: scan.geometry_ordinal,
                                frozen_bar_indices: frozen_bar_indices(
                                    &scan.bar_band,
                                    &obstacle_system.frozen_by_ordinate,
                                ),
                            })
                            .collect(),
                    })
                    .collect(),
            },
        )
        .collect();
    Ok(NativeHeadScannerBarSlicesRecognition { systems })
}

fn frozen_bar_indices(
    band: &crate::head_scanner_slices::HeadScannerBand,
    obstacles: &[crate::native_heads_obstacles::NativeHeadsBarObstacle],
) -> Vec<usize> {
    obstacles
        .iter()
        .enumerate()
        .filter_map(|(index, obstacle)| {
            band.intersects_rectangle(obstacle.area_bounds)
                .then_some(index)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use audiveris_image::{
        beam_structure::Segment,
        system_population::{BoundarySegment, StaffBoundary},
    };

    use crate::{
        head_scanner_slices::{HeadScannerBand, JavaRectangle},
        native_heads_obstacles::{
            NativeHeadsBarClass, NativeHeadsBarObstacle, NativeHeadsBarShape,
        },
    };

    use super::*;

    #[test]
    fn filtering_preserves_ordinate_pool_order_without_an_abscissa_sort() {
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
        let obstacles = [obstacle(20, 9), obstacle(5, 9), obstacle(15, 30)];
        assert_eq!(frozen_bar_indices(&band, &obstacles), vec![0, 1]);
    }

    fn obstacle(x: i32, y: i32) -> NativeHeadsBarObstacle {
        NativeHeadsBarObstacle {
            source_inter_id: 1,
            source_ordinal: 0,
            class: NativeHeadsBarClass::BarlineInter,
            shape: NativeHeadsBarShape::ThinBarline,
            staff_id: Some(1),
            frozen: true,
            median: Segment {
                x1: f64::from(x) + 1.0,
                y1: f64::from(y),
                x2: f64::from(x) + 1.0,
                y2: f64::from(y + 2),
            },
            thickness: 2.0,
            area_bounds: JavaRectangle::new(x, y, 2, 2),
        }
    }
}
