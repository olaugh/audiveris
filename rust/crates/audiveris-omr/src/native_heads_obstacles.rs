// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact frozen GRID bar/connector obstacles consumed by HEADS.
//!
//! Java `NoteHeadsBuilder.getSystemBarAreas` starts from SIG insertion order,
//! keeps only frozen `BarlineInter` and `BarConnectorInter` nodes, then applies
//! a stable sort on the integer area-bounds ordinate. This adapter retains the
//! pre-filter candidate order as well as that final obstacle-pool order.

use std::{error::Error, fmt};

use audiveris_image::{
    bar_alignment::VerticalSide,
    bars_logic::{ConnectorInterKind, PeakWidthClass, VerticalInterKind},
    beam_structure::Segment,
    grid_sig::{GridInterId, GridSigNode},
    staff_peak::{StaffPeak, StaffPeakKey},
};

use crate::{
    grid_executor::HeadlessSystemSigState,
    head_scanner_slices::{JavaRectangle, VerticalRibbonArea},
    recognize::GridLinesRecognition,
};

/// Java class represented by one HEADS bar obstacle candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHeadsBarClass {
    BarlineInter,
    BarConnectorInter,
}

impl NativeHeadsBarClass {
    #[must_use]
    pub const fn java_name(self) -> &'static str {
        match self {
            Self::BarlineInter => "BarlineInter",
            Self::BarConnectorInter => "BarConnectorInter",
        }
    }
}

/// Java shape represented by one HEADS bar obstacle candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHeadsBarShape {
    ThinBarline,
    ThickBarline,
    ThinConnector,
    ThickConnector,
}

impl NativeHeadsBarShape {
    #[must_use]
    pub const fn java_name(self) -> &'static str {
        match self {
            Self::ThinBarline => "THIN_BARLINE",
            Self::ThickBarline => "THICK_BARLINE",
            Self::ThinConnector => "THIN_CONNECTOR",
            Self::ThickConnector => "THICK_CONNECTOR",
        }
    }
}

/// One GRID SIG barline/connector considered by HEADS.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHeadsBarObstacle {
    /// System-local GRID inter identity, which is also SIG insertion order.
    pub source_inter_id: usize,
    /// Zero-based ordinal after filtering SIG nodes to the two Java classes.
    pub source_ordinal: usize,
    pub class: NativeHeadsBarClass,
    pub shape: NativeHeadsBarShape,
    /// Barline staff owner; Java connectors have no staff owner.
    pub staff_id: Option<usize>,
    pub frozen: bool,
    pub median: Segment,
    pub thickness: f64,
    /// Java `AreaUtil.verticalRibbon(...).getBounds()`.
    pub area_bounds: JavaRectangle,
}

impl NativeHeadsBarObstacle {
    #[must_use]
    pub const fn area(self) -> VerticalRibbonArea {
        VerticalRibbonArea::new(self.median, self.thickness)
    }
}

/// Candidate and frozen obstacle pools for one system.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsBarObstacleSystem {
    pub system_id: usize,
    /// `SIGraph.vertexSet()` order after class filtering, before frozen filtering.
    pub candidates_in_sig_order: Vec<NativeHeadsBarObstacle>,
    /// Frozen candidates, stable-sorted only by integer `area_bounds.y`.
    pub frozen_by_ordinate: Vec<NativeHeadsBarObstacle>,
}

/// Sheet-wide GRID obstacle input for `NoteHeadsBuilder`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsBarObstaclePool {
    pub systems: Vec<NativeHeadsBarObstacleSystem>,
}

/// Invalid or incomplete GRID SIG state at the HEADS boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHeadsBarObstacleError {
    MissingConnectorEndpoint {
        system_id: usize,
        connector_inter_id: usize,
        side: VerticalSide,
        endpoint: StaffPeakKey,
    },
    NonBarlineConnectorEndpoint {
        system_id: usize,
        connector_inter_id: usize,
        side: VerticalSide,
        endpoint: StaffPeakKey,
    },
}

impl fmt::Display for NativeHeadsBarObstacleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingConnectorEndpoint {
                system_id,
                connector_inter_id,
                side,
                endpoint,
            } => write!(
                formatter,
                "system {system_id} connector {connector_inter_id} has no {side:?} endpoint \
                 barline for staff {} peak {}..{}",
                endpoint.staff_id().value(),
                endpoint.start(),
                endpoint.stop()
            ),
            Self::NonBarlineConnectorEndpoint {
                system_id,
                connector_inter_id,
                side,
                endpoint,
            } => write!(
                formatter,
                "system {system_id} connector {connector_inter_id} {side:?} endpoint on staff {} \
                 peak {}..{} is not a barline",
                endpoint.staff_id().value(),
                endpoint.start(),
                endpoint.stop()
            ),
        }
    }
}

impl Error for NativeHeadsBarObstacleError {}

/// Materialize Java's GRID-owned bar/connector pool at the HEADS boundary.
pub fn materialize_native_heads_bar_obstacles(
    grid: &GridLinesRecognition,
) -> Result<NativeHeadsBarObstaclePool, NativeHeadsBarObstacleError> {
    let systems = grid
        .peak_graph
        .sig
        .systems
        .iter()
        .map(|system| materialize_system(system, grid.scale.scale.line.main))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(NativeHeadsBarObstaclePool { systems })
}

fn materialize_system(
    system: &HeadlessSystemSigState,
    foreground_thickness: i32,
) -> Result<NativeHeadsBarObstacleSystem, NativeHeadsBarObstacleError> {
    let mut candidates = Vec::new();
    for (inter_id, node) in system.sig.nodes_in_order() {
        let source_ordinal = candidates.len();
        let candidate = match node {
            GridSigNode::Vertical { plan, frozen, .. } => {
                let VerticalInterKind::Barline { width_class, .. } = plan.kind else {
                    continue;
                };
                obstacle(
                    ObstacleIdentity {
                        inter_id,
                        source_ordinal,
                        class: NativeHeadsBarClass::BarlineInter,
                        shape: barline_shape(width_class),
                        staff_id: Some(plan.peak.staff_id().value()),
                    },
                    *frozen,
                    Segment {
                        x1: plan.median.x,
                        y1: plan.median.top,
                        x2: plan.median.x,
                        y2: plan.median.bottom,
                    },
                    plan.width,
                )
            }
            GridSigNode::Connector { plan, frozen, .. } => {
                let ConnectorInterKind::Barline(width_class) = plan.kind else {
                    continue;
                };
                // Java constructs BarConnectorInter geometry from the
                // BarConnection's two StaffPeaks before it checks whether both
                // peaks own published BarlineInters. A brace replacement can
                // therefore leave a valid orphan connector in the SIG; HEADS
                // must still use that connector's own geometry.
                let top = connector_endpoint_peak(system, inter_id, VerticalSide::Top, plan.top)?;
                let bottom =
                    connector_endpoint_peak(system, inter_id, VerticalSide::Bottom, plan.bottom)?;
                let (median, thickness) = connector_geometry(top, bottom, foreground_thickness);
                obstacle(
                    ObstacleIdentity {
                        inter_id,
                        source_ordinal,
                        class: NativeHeadsBarClass::BarConnectorInter,
                        shape: connector_shape(width_class),
                        staff_id: None,
                    },
                    *frozen,
                    median,
                    thickness,
                )
            }
        };
        candidates.push(candidate);
    }

    let mut frozen = candidates
        .iter()
        .copied()
        .filter(|candidate| candidate.frozen)
        .collect::<Vec<_>>();
    // Rust's slice sort is stable. Java compares only bounds.y, so equal-y
    // candidates retain the filtered SIG insertion order above.
    frozen.sort_by_key(|candidate| candidate.area_bounds.y);

    Ok(NativeHeadsBarObstacleSystem {
        system_id: system.system_id,
        candidates_in_sig_order: candidates,
        frozen_by_ordinate: frozen,
    })
}

fn connector_geometry(
    top: &StaffPeak,
    bottom: &StaffPeak,
    foreground_thickness: i32,
) -> (Segment, f64) {
    let top_width = f64::from(top.width());
    let bottom_width = f64::from(bottom.width());
    let half_line = f64::from(foreground_thickness) / 2.0;
    (
        Segment {
            x1: f64::from(top.start()) + (top_width / 2.0),
            y1: f64::from(top.bottom()) + half_line + 0.5,
            x2: f64::from(bottom.start()) + (bottom_width / 2.0),
            y2: f64::from(bottom.top()) - half_line + 0.5,
        },
        (top_width + bottom_width) / 2.0,
    )
}

#[derive(Clone, Copy)]
struct ObstacleIdentity {
    inter_id: GridInterId,
    source_ordinal: usize,
    class: NativeHeadsBarClass,
    shape: NativeHeadsBarShape,
    staff_id: Option<usize>,
}

fn obstacle(
    identity: ObstacleIdentity,
    frozen: bool,
    median: Segment,
    thickness: f64,
) -> NativeHeadsBarObstacle {
    let area = VerticalRibbonArea::new(median, thickness);
    NativeHeadsBarObstacle {
        source_inter_id: identity.inter_id.value(),
        source_ordinal: identity.source_ordinal,
        class: identity.class,
        shape: identity.shape,
        staff_id: identity.staff_id,
        frozen,
        median,
        thickness,
        area_bounds: area.integer_bounds(),
    }
}

fn connector_endpoint_peak(
    system: &HeadlessSystemSigState,
    connector: GridInterId,
    side: VerticalSide,
    endpoint: StaffPeakKey,
) -> Result<&StaffPeak, NativeHeadsBarObstacleError> {
    system
        .staff_peaks
        .iter()
        .flatten()
        .find(|peak| peak.key() == endpoint)
        .ok_or(NativeHeadsBarObstacleError::MissingConnectorEndpoint {
            system_id: system.system_id,
            connector_inter_id: connector.value(),
            side,
            endpoint,
        })
}

const fn barline_shape(width: PeakWidthClass) -> NativeHeadsBarShape {
    match width {
        PeakWidthClass::Thin => NativeHeadsBarShape::ThinBarline,
        PeakWidthClass::Thick => NativeHeadsBarShape::ThickBarline,
    }
}

const fn connector_shape(width: PeakWidthClass) -> NativeHeadsBarShape {
    match width {
        PeakWidthClass::Thin => NativeHeadsBarShape::ThinConnector,
        PeakWidthClass::Thick => NativeHeadsBarShape::ThickConnector,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connector_uses_connection_peak_geometry_even_without_endpoint_inters() {
        let top = StaffPeak::new(
            audiveris_image::bar_column::StaffId::new(1),
            349,
            437,
            200,
            202,
        )
        .expect("top peak");
        let bottom = StaffPeak::new(
            audiveris_image::bar_column::StaffId::new(2),
            591,
            679,
            199,
            201,
        )
        .expect("bottom peak");
        let (median, thickness) = connector_geometry(&top, &bottom, 1);
        let area = VerticalRibbonArea::new(median, thickness);
        assert_eq!(
            median,
            Segment {
                x1: 201.5,
                y1: 438.0,
                x2: 200.5,
                y2: 591.0
            }
        );
        assert_eq!(thickness, 3.0);
        assert_eq!(area.integer_bounds(), JavaRectangle::new(199, 438, 4, 153));
    }
}
