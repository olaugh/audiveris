// SPDX-License-Identifier: AGPL-3.0-or-later

//! Concrete headless SIG ownership for GRID bar and bracket interpretations.
//!
//! This is the graph-neutral equivalent of `BarsRetriever.createInters` and
//! `createConnectionInters`: stable glyph/inter identities, peak backlinks,
//! endpoint relations, connection support, and good-bar freezing are retained
//! without pulling UI or Java object cycles into the recognition core.

use std::collections::BTreeMap;

use crate::{
    bar_alignment::BarAlignment,
    bars_logic::{
        BarsLogicError, ConnectionInterPlan, VerticalInterPlan, extended_connection_peak_keys,
    },
    peak_graph::{PeakEdgeId, PeakGraph},
    staff_peak::StaffPeakKey,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GridGlyphId(usize);

impl GridGlyphId {
    #[must_use]
    pub const fn value(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GridInterId(usize);

impl GridInterId {
    #[must_use]
    pub const fn value(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridSigNode {
    Vertical {
        plan: VerticalInterPlan,
        glyph: GridGlyphId,
        frozen: bool,
    },
    Connector {
        plan: ConnectionInterPlan,
        frozen: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridSigRelation {
    NoExclusion,
    BarConnectionSupport { grade: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridSigEdge {
    pub source: GridInterId,
    pub target: GridInterId,
    pub relation: GridSigRelation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionPromotionFailure {
    MissingTopInter,
    MissingBottomInter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionPromotionWarning {
    pub edge: PeakEdgeId,
    pub failure: ConnectionPromotionFailure,
}

#[derive(Clone, Debug, Default)]
pub struct GridSig {
    next_glyph_id: usize,
    next_inter_id: usize,
    nodes: BTreeMap<GridInterId, GridSigNode>,
    edges: Vec<GridSigEdge>,
    peak_inters: BTreeMap<StaffPeakKey, GridInterId>,
}

impl GridSig {
    #[must_use]
    pub fn node(&self, id: GridInterId) -> Option<&GridSigNode> {
        self.nodes.get(&id)
    }

    #[must_use]
    pub fn edges(&self) -> &[GridSigEdge] {
        &self.edges
    }

    #[must_use]
    pub fn inter_of(&self, peak: StaffPeakKey) -> Option<GridInterId> {
        self.peak_inters.get(&peak).copied()
    }

    pub fn nodes_in_order(&self) -> impl Iterator<Item = (GridInterId, &GridSigNode)> {
        self.nodes.iter().map(|(&id, node)| (id, node))
    }

    /// Materialize Java `createInters` plans in projector traversal order.
    pub fn promote_vertical_inters(&mut self, plans: &[VerticalInterPlan]) -> Vec<GridInterId> {
        let mut promoted = Vec::with_capacity(plans.len());
        for &plan in plans {
            self.next_glyph_id += 1;
            let glyph = GridGlyphId(self.next_glyph_id);
            self.next_inter_id += 1;
            let inter = GridInterId(self.next_inter_id);
            self.nodes.insert(
                inter,
                GridSigNode::Vertical {
                    plan,
                    glyph,
                    frozen: false,
                },
            );
            self.peak_inters.insert(plan.peak, inter);
            promoted.push(inter);
        }
        promoted
    }

    /// Materialize Java `createConnectionInters` plans.
    ///
    /// Java inserts the connector vertex before it discovers a missing endpoint
    /// and catches each connection failure independently. The orphan connector
    /// and any already-added top relation therefore remain visible.
    pub fn promote_connection_inters(
        &mut self,
        peak_graph: &PeakGraph<BarAlignment>,
        plans: &[ConnectionInterPlan],
    ) -> Result<Vec<ConnectionPromotionWarning>, BarsLogicError> {
        let mut warnings = Vec::new();
        for &plan in plans {
            self.next_inter_id += 1;
            let connector = GridInterId(self.next_inter_id);
            self.nodes.insert(
                connector,
                GridSigNode::Connector {
                    plan,
                    frozen: false,
                },
            );

            let Some(top) = self.inter_of(plan.top) else {
                warnings.push(ConnectionPromotionWarning {
                    edge: plan.edge,
                    failure: ConnectionPromotionFailure::MissingTopInter,
                });
                continue;
            };
            self.edges.push(GridSigEdge {
                source: top,
                target: connector,
                relation: GridSigRelation::NoExclusion,
            });

            let Some(bottom) = self.inter_of(plan.bottom) else {
                warnings.push(ConnectionPromotionWarning {
                    edge: plan.edge,
                    failure: ConnectionPromotionFailure::MissingBottomInter,
                });
                continue;
            };
            self.edges.push(GridSigEdge {
                source: connector,
                target: bottom,
                relation: GridSigRelation::NoExclusion,
            });
            self.edges.push(GridSigEdge {
                source: top,
                target: bottom,
                relation: GridSigRelation::BarConnectionSupport { grade: plan.grade },
            });

            if plan.freeze_and_extend_bar_column {
                if let Some(GridSigNode::Connector { frozen, .. }) = self.nodes.get_mut(&connector)
                {
                    *frozen = true;
                }
                for peak in extended_connection_peak_keys(peak_graph, plan.edge)? {
                    if let Some(inter) = self.inter_of(peak)
                        && let Some(GridSigNode::Vertical { frozen, .. }) =
                            self.nodes.get_mut(&inter)
                    {
                        *frozen = true;
                    }
                }
            }
        }
        Ok(warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bar_alignment::{AlignmentPeak, BarAlignment, BarImpacts},
        bar_column::{PeakId, StaffId},
        bars_logic::{
            ConnectorInterKind, PeakWidthClass, VerticalInterKind, VerticalMedian,
            plan_connection_inters,
        },
        staff_peak::StaffPeak,
    };

    fn peak(staff: usize, x: i32) -> StaffPeak {
        StaffPeak::new(StaffId::new(staff), 10, 30, x, x).unwrap()
    }

    fn vertical_plan(peak: &StaffPeak) -> VerticalInterPlan {
        VerticalInterPlan {
            peak: peak.key(),
            median: VerticalMedian {
                x: f64::from(peak.start()) + 0.5,
                top: 9.5,
                bottom: 31.5,
            },
            width: 1.0,
            impacts: None,
            kind: VerticalInterKind::Barline {
                width_class: PeakWidthClass::Thin,
                left_staff_end: false,
                right_staff_end: false,
            },
        }
    }

    fn connection_graph(top: StaffPeak, bottom: StaffPeak, grade: f64) -> PeakGraph<BarAlignment> {
        let top_key = top.key();
        let bottom_key = bottom.key();
        let mut graph = PeakGraph::new();
        graph.add_vertex(top);
        graph.add_vertex(bottom);
        let top_alignment =
            AlignmentPeak::new(PeakId::new(1), top_key.staff_id(), 10, 1.0).unwrap();
        let bottom_alignment =
            AlignmentPeak::new(PeakId::new(2), bottom_key.staff_id(), 10, 1.0).unwrap();
        let alignment = BarAlignment::new(
            top_alignment,
            bottom_alignment,
            0.0,
            0.0,
            BarImpacts::alignment(1.0, 1.0).unwrap(),
        )
        .unwrap();
        let connection = BarAlignment::connection(&alignment, grade, grade).unwrap();
        graph.add_edge(top_key, bottom_key, connection).unwrap();
        graph
    }

    #[test]
    fn vertical_promotion_assigns_glyphs_in_plan_order_and_backlinks_peaks() {
        let top = peak(1, 10);
        let bottom = peak(2, 10);
        let plans = [vertical_plan(&top), vertical_plan(&bottom)];
        let mut sig = GridSig::default();
        let ids = sig.promote_vertical_inters(&plans);
        assert_eq!(ids.iter().map(|id| id.value()).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(sig.inter_of(top.key()), Some(ids[0]));
        assert_eq!(sig.inter_of(bottom.key()), Some(ids[1]));
        assert!(matches!(
            sig.node(ids[1]),
            Some(GridSigNode::Vertical { glyph, .. }) if glyph.value() == 2
        ));
    }

    #[test]
    fn complete_connection_adds_three_relations_and_freezes_good_bar_component() {
        let top = peak(1, 10);
        let bottom = peak(2, 10);
        let graph = connection_graph(top.clone(), bottom.clone(), 1.0);
        let mut sig = GridSig::default();
        let verticals = sig.promote_vertical_inters(&[vertical_plan(&top), vertical_plan(&bottom)]);
        let plans = plan_connection_inters(&graph, |_| true);
        assert_eq!(
            plans[0].kind,
            ConnectorInterKind::Barline(PeakWidthClass::Thin)
        );
        assert!(
            sig.promote_connection_inters(&graph, &plans)
                .unwrap()
                .is_empty()
        );
        assert_eq!(sig.edges().len(), 3);
        assert!(matches!(
            sig.node(verticals[0]),
            Some(GridSigNode::Vertical { frozen: true, .. })
        ));
        assert!(matches!(
            sig.nodes_in_order().last().map(|(_, node)| node),
            Some(GridSigNode::Connector { frozen: true, .. })
        ));
    }

    #[test]
    fn missing_bottom_keeps_connector_and_top_edge_then_continues() {
        let top = peak(1, 10);
        let bottom = peak(2, 10);
        let graph = connection_graph(top.clone(), bottom.clone(), 1.0);
        let mut sig = GridSig::default();
        sig.promote_vertical_inters(&[vertical_plan(&top)]);
        let plans = plan_connection_inters(&graph, |key| key == top.key());
        let warnings = sig.promote_connection_inters(&graph, &plans).unwrap();
        assert_eq!(
            warnings,
            [ConnectionPromotionWarning {
                edge: plans[0].edge,
                failure: ConnectionPromotionFailure::MissingBottomInter,
            }]
        );
        assert_eq!(sig.edges().len(), 1);
        assert_eq!(sig.nodes_in_order().count(), 2);
    }
}
