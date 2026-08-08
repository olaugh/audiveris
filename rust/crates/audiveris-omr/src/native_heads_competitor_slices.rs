// SPDX-License-Identifier: AGPL-3.0-or-later

//! Per-scanner competitor slices for HEADS.
//!
//! Java `NoteHeadsBuilder.getCompetitorsSlice` intersects the semantic scanner
//! `Area` with the bounds of the already accepted, ordinate-sorted competitor
//! pool. It then stably sorts the retained competitors by their left
//! abscissa. This module preserves the original accepted-pool indices while
//! reproducing that lookup order.

use std::{error::Error, fmt};

use crate::{
    native_heads_competitors::{NativeHeadsCompetitor, NativeHeadsCompetitorPool},
    native_heads_scanner_pools::NativeHeadScannerPoolsRecognition,
};

/// All per-scan competitor slices, in Java system/staff/scanner order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHeadScannerCompetitorSlicesRecognition {
    pub systems: Vec<NativeHeadScannerCompetitorSliceSystem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHeadScannerCompetitorSliceSystem {
    pub system_id: usize,
    pub staffs: Vec<NativeHeadScannerCompetitorSliceStaff>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHeadScannerCompetitorSliceStaff {
    pub staff_id: usize,
    pub scans: Vec<NativeHeadScannerCompetitorSlice>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHeadScannerCompetitorSlice {
    pub geometry_ordinal: usize,
    /// Indices into the corresponding system's `accepted_by_ordinate` pool,
    /// reordered by Java's stable `Inters.byAbscissa` comparison.
    pub competitor_indices: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeHeadScannerCompetitorSliceError {
    SystemOrder {
        scanner_system_ids: Vec<usize>,
        competitor_system_ids: Vec<usize>,
    },
    AcceptedOrdinal {
        system_id: usize,
        pool_index: usize,
        accepted_ordinal: Option<usize>,
    },
}

impl fmt::Display for NativeHeadScannerCompetitorSliceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemOrder {
                scanner_system_ids,
                competitor_system_ids,
            } => write!(
                formatter,
                "HEADS scanner system order {scanner_system_ids:?}, but competitor order is \
                 {competitor_system_ids:?}"
            ),
            Self::AcceptedOrdinal {
                system_id,
                pool_index,
                accepted_ordinal,
            } => write!(
                formatter,
                "HEADS system {system_id} competitor pool index {pool_index} reports accepted \
                 ordinal {accepted_ordinal:?}"
            ),
        }
    }
}

impl Error for NativeHeadScannerCompetitorSliceError {}

/// Apply every scanner's semantic competitor band to the accepted competitor
/// pool, retaining the pool identities through Java's stable x sort.
pub fn materialize_native_head_scanner_competitor_slices(
    scanners: &NativeHeadScannerPoolsRecognition,
    competitors: &NativeHeadsCompetitorPool,
) -> Result<NativeHeadScannerCompetitorSlicesRecognition, NativeHeadScannerCompetitorSliceError> {
    let scanner_system_ids = scanners
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    let competitor_system_ids = competitors
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    if scanner_system_ids != competitor_system_ids {
        return Err(NativeHeadScannerCompetitorSliceError::SystemOrder {
            scanner_system_ids,
            competitor_system_ids,
        });
    }

    let mut systems = Vec::with_capacity(scanners.systems.len());
    for (scanner_system, competitor_system) in scanners.systems.iter().zip(&competitors.systems) {
        for (pool_index, competitor) in competitor_system.accepted_by_ordinate.iter().enumerate() {
            if competitor.accepted_ordinal != Some(pool_index) {
                return Err(NativeHeadScannerCompetitorSliceError::AcceptedOrdinal {
                    system_id: scanner_system.system_id,
                    pool_index,
                    accepted_ordinal: competitor.accepted_ordinal,
                });
            }
        }

        systems.push(NativeHeadScannerCompetitorSliceSystem {
            system_id: scanner_system.system_id,
            staffs: scanner_system
                .staffs
                .iter()
                .map(|staff| NativeHeadScannerCompetitorSliceStaff {
                    staff_id: staff.staff_id,
                    scans: staff
                        .scans
                        .iter()
                        .map(|scan| NativeHeadScannerCompetitorSlice {
                            geometry_ordinal: scan.geometry_ordinal,
                            competitor_indices: competitor_indices(
                                &scan.competitor_band,
                                &competitor_system.accepted_by_ordinate,
                            ),
                        })
                        .collect(),
                })
                .collect(),
        });
    }

    Ok(NativeHeadScannerCompetitorSlicesRecognition { systems })
}

fn competitor_indices(
    band: &crate::head_scanner_slices::HeadScannerBand,
    competitors: &[NativeHeadsCompetitor],
) -> Vec<usize> {
    let mut indices = competitors
        .iter()
        .enumerate()
        .filter_map(|(index, competitor)| {
            band.intersects_rectangle(competitor.bounds)
                .then_some(index)
        })
        .collect::<Vec<_>>();
    // Rust's slice sort is stable, matching `Collections.sort` when two
    // competitors have the same left abscissa.
    indices.sort_by_key(|index| competitors[*index].bounds.x);
    indices
}

#[cfg(test)]
mod tests {
    use crate::{
        head_scanner_slices::{HeadScannerBand, JavaRectangle},
        native_heads_competitors::{
            NativeHeadsCompetitor, NativeHeadsCompetitorClass, NativeHeadsCompetitorDecision,
            NativeHeadsCompetitorEvidence, NativeHeadsCompetitorGeometry,
            NativeHeadsCompetitorShape, NativeHeadsCompetitorSource,
        },
    };
    use audiveris_image::{
        beam_structure::Segment,
        system_population::{BoundarySegment, StaffBoundary},
    };

    use super::*;

    #[test]
    fn semantic_intersection_then_stable_abscissa_sort_preserves_pool_indices() {
        let band = HeadScannerBand::from_staff_boundary(
            &StaffBoundary {
                segments: vec![BoundarySegment::Line {
                    start: (0.0, 10.0),
                    end: (40.0, 10.0),
                }],
            },
            -2.0,
            1.0,
        );
        let competitors = [
            competitor(20, 9, 0),
            competitor(5, 9, 1),
            competitor(5, 10, 2),
            competitor(15, 30, 3),
        ];
        assert_eq!(competitor_indices(&band, &competitors), vec![1, 2, 0]);
    }

    fn competitor(x: i32, y: i32, accepted_ordinal: usize) -> NativeHeadsCompetitor {
        NativeHeadsCompetitor {
            source: NativeHeadsCompetitorSource::GridInter(accepted_ordinal),
            source_ordinal: accepted_ordinal,
            accepted_ordinal: Some(accepted_ordinal),
            class: NativeHeadsCompetitorClass::BarlineInter,
            shape: NativeHeadsCompetitorShape::ThinBarline,
            staff_id: Some(1),
            bounds: JavaRectangle::new(x, y, 2, 2),
            grade: 1.0,
            best_grade: 1.0,
            good: true,
            frozen: true,
            geometry: NativeHeadsCompetitorGeometry::Vertical {
                median: Segment {
                    x1: f64::from(x + 1),
                    y1: f64::from(y),
                    x2: f64::from(x + 1),
                    y2: f64::from(y + 2),
                },
                width: 2.0,
            },
            glyph: None,
            evidence: NativeHeadsCompetitorEvidence::VerticalFloor(2),
            decision: NativeHeadsCompetitorDecision::Accept,
        }
    }
}
