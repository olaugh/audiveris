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
    bars_logic::{
        ConnectorInterKind, PeakWidthClass, VerticalInterKind, VerticalInterPlan, VerticalMedian,
    },
    beam_structure::Segment,
    grid_sig::{GridInterId, GridSigNode},
    staff_peak::StaffPeakKey,
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
        .map(materialize_system)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(NativeHeadsBarObstaclePool { systems })
}

fn materialize_system(
    system: &HeadlessSystemSigState,
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
                let top = connector_endpoint_plan(system, inter_id, VerticalSide::Top, plan.top)?;
                let bottom =
                    connector_endpoint_plan(system, inter_id, VerticalSide::Bottom, plan.bottom)?;
                let (median, thickness) =
                    connector_geometry(top.median, top.width, bottom.median, bottom.width);
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
    top: VerticalMedian,
    top_width: f64,
    bottom: VerticalMedian,
    bottom_width: f64,
) -> (Segment, f64) {
    (
        Segment {
            x1: top.x,
            y1: top.bottom,
            x2: bottom.x,
            y2: bottom.top,
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

fn connector_endpoint_plan(
    system: &HeadlessSystemSigState,
    connector: GridInterId,
    side: VerticalSide,
    endpoint: StaffPeakKey,
) -> Result<&VerticalInterPlan, NativeHeadsBarObstacleError> {
    let Some(endpoint_inter) = system.sig.inter_of(endpoint) else {
        return Err(NativeHeadsBarObstacleError::MissingConnectorEndpoint {
            system_id: system.system_id,
            connector_inter_id: connector.value(),
            side,
            endpoint,
        });
    };
    let Some(GridSigNode::Vertical { plan, .. }) = system.sig.node(endpoint_inter) else {
        return Err(NativeHeadsBarObstacleError::MissingConnectorEndpoint {
            system_id: system.system_id,
            connector_inter_id: connector.value(),
            side,
            endpoint,
        });
    };
    if !matches!(plan.kind, VerticalInterKind::Barline { .. }) {
        return Err(NativeHeadsBarObstacleError::NonBarlineConnectorEndpoint {
            system_id: system.system_id,
            connector_inter_id: connector.value(),
            side,
            endpoint,
        });
    }
    Ok(plan)
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
    fn connector_uses_endpoint_barline_geometry_and_average_width() {
        let top = VerticalMedian {
            x: 201.5,
            top: 349.0,
            bottom: 438.0,
        };
        let bottom = VerticalMedian {
            x: 200.5,
            top: 591.0,
            bottom: 679.0,
        };
        let (median, thickness) = connector_geometry(top, 3.0, bottom, 3.0);
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
