// SPDX-License-Identifier: AGPL-3.0-or-later

//! Concrete headless SIG ownership for GRID bar and bracket interpretations.
//!
//! This is the graph-neutral equivalent of `BarsRetriever.createInters` and
//! `createConnectionInters`: stable glyph/inter identities, peak backlinks,
//! endpoint relations, connection support, and good-bar freezing are retained
//! without pulling UI or Java object cycles into the recognition core.

use std::collections::BTreeMap;

use audiveris_core::grade::contextual_from_partners;

use crate::{
    bar_alignment::BarAlignment,
    bar_alignment::{BarAlignmentKind, VerticalSide},
    bar_column::StaffId,
    bars_logic::{
        BarsLogicError, BraceGroupDecision, ConnectionInterPlan, GroupPeakFacts, GroupStaffFacts,
        PlannedPart, VerticalInterKind, VerticalInterPlan, create_part_groups,
        extended_connection_peak_keys, is_true_brace_group, part_connection_edge_ids, plan_parts,
    },
    part_group::PartGroup,
    peak_graph::{PeakEdgeId, PeakGraph},
    staff_peak::{StaffPeak, StaffPeakKey},
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
        contextual_grade: Option<f64>,
    },
    Connector {
        plan: ConnectionInterPlan,
        frozen: bool,
        contextual_grade: Option<f64>,
    },
}

impl GridSigNode {
    /// Java `Inter.getGrade()`: the immutable intrinsic grade supplied by the
    /// recognition plan, never a previously computed contextual value.
    #[must_use]
    pub fn intrinsic_grade(&self) -> f64 {
        match self {
            Self::Vertical { plan, .. } => plan.impacts.map_or(0.0, |impacts| impacts.grade()),
            Self::Connector { plan, .. } => plan.grade,
        }
    }

    #[must_use]
    pub const fn contextual_grade(&self) -> Option<f64> {
        match self {
            Self::Vertical {
                contextual_grade, ..
            }
            | Self::Connector {
                contextual_grade, ..
            } => *contextual_grade,
        }
    }

    fn set_contextual_grade(&mut self, grade: Option<f64>) {
        match self {
            Self::Vertical {
                contextual_grade, ..
            }
            | Self::Connector {
                contextual_grade, ..
            } => *contextual_grade = grade,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridSigRelation {
    NoExclusion,
    BarConnectionSupport { grade: f64 },
    BarGroup { gap_fraction: f64 },
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
pub enum BarGroupPromotionError {
    MissingInter(StaffPeakKey),
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
                    contextual_grade: None,
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
                    contextual_grade: None,
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

    /// Reproduce `BarsRetriever.groupBarlines` in staff/projector order.
    ///
    /// Braces and brackets are skipped without clearing the preceding ordinary
    /// peak. Java has no per-staff catch here, so a missing promoted inter stops
    /// immediately while previously inserted relations remain.
    pub fn group_barlines(
        &mut self,
        staff_peaks: &[Vec<StaffPeak>],
        maximum_gap: i32,
        mut pixels_to_fraction: impl FnMut(i32) -> f64,
    ) -> Result<(), BarGroupPromotionError> {
        for peaks in staff_peaks {
            let mut previous: Option<&StaffPeak> = None;
            for peak in peaks {
                if peak.is_brace() || peak.is_bracket() {
                    continue;
                }
                if let Some(previous_peak) = previous {
                    let gap = peak
                        .start()
                        .wrapping_sub(previous_peak.stop())
                        .wrapping_sub(1);
                    if gap <= maximum_gap {
                        let source = self
                            .inter_of(previous_peak.key())
                            .ok_or(BarGroupPromotionError::MissingInter(previous_peak.key()))?;
                        let target = self
                            .inter_of(peak.key())
                            .ok_or(BarGroupPromotionError::MissingInter(peak.key()))?;
                        self.edges.push(GridSigEdge {
                            source,
                            target,
                            relation: GridSigRelation::BarGroup {
                                gap_fraction: pixels_to_fraction(gap),
                            },
                        });
                    }
                }
                previous = Some(peak);
            }
        }
        Ok(())
    }

    /// Recompute the bounded GRID contextual grades produced by Java
    /// `SIGraph.contextualize()` at the end of `BarsRetriever.process()`.
    ///
    /// At this seam only `BarConnectionRelation` is a support relation. Its
    /// source and target ratio is `1 + 5 * relation_grade`. Partner grades are
    /// always intrinsic snapshots, so repeated calls are idempotent. Stable
    /// decreasing partner-grade order fixes floating-point accumulation order.
    pub fn contextualize(&mut self) {
        let intrinsic = self
            .nodes
            .iter()
            .map(|(&id, node)| (id, node.intrinsic_grade()))
            .collect::<BTreeMap<_, _>>();
        let mut computed = BTreeMap::new();

        for (&id, node) in &self.nodes {
            let grade = node.intrinsic_grade();
            let contextual = {
                let mut supports = self
                    .edges
                    .iter()
                    .filter_map(|edge| {
                        let GridSigRelation::BarConnectionSupport {
                            grade: relation_grade,
                        } = edge.relation
                        else {
                            return None;
                        };
                        let partner = if edge.source == id {
                            edge.target
                        } else if edge.target == id {
                            edge.source
                        } else {
                            return None;
                        };
                        intrinsic
                            .get(&partner)
                            .copied()
                            .map(|partner_grade| (partner_grade, 1.0 + (5.0 * relation_grade)))
                    })
                    .collect::<Vec<_>>();
                supports.sort_by(|left, right| right.0.total_cmp(&left.0));
                let (partners, ratios): (Vec<_>, Vec<_>) = supports.into_iter().unzip();
                contextual_from_partners(grade, &partners, &ratios)
                    .expect("partner grades and support ratios are built together")
            };
            computed.insert(id, Some(contextual));
        }

        for (id, grade) in computed {
            self.nodes
                .get_mut(&id)
                .expect("contextual snapshot contains existing nodes")
                .set_contextual_grade(grade);
        }
    }

    /// Execute the concrete post-`groupBarlines` tail currently available in
    /// Java order: `recordBars`, `createGroups`, then `createParts`.
    ///
    /// `brace_peaks` carries Java's separately held
    /// `StaffProjector.getBracePeak()` candidate per staff. It participates in
    /// grouping but never in ordinary peak traversal or SIG promotion.
    pub fn build_bar_tail(
        &self,
        result: &mut BarTailResult,
        staff_peaks: &[Vec<StaffPeak>],
        brace_peaks: &[Option<StaffPeak>],
        peak_graph: &PeakGraph<BarAlignment>,
        parameters: BarTailParameters,
    ) -> Result<(), BarTailError> {
        self.record_bars(result, staff_peaks)?;
        if brace_peaks.len() != staff_peaks.len() {
            return Err(BarTailError::BracePeakCount {
                staffs: staff_peaks.len(),
                braces: brace_peaks.len(),
            });
        }

        let facts = staff_peaks
            .iter()
            .map(|peaks| {
                peaks
                    .iter()
                    .map(|peak| group_peak_facts(peak, peak_graph))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut staff_facts = Vec::new();
        for ((peaks, facts), detached_brace) in staff_peaks.iter().zip(&facts).zip(brace_peaks) {
            if let Some(first) = peaks.first() {
                let start_peak_index = peaks
                    .iter()
                    .position(|peak| peak.is_staff_end(crate::staff_peak::HorizontalSide::Left));
                let start_peak = start_peak_index.map(|index| &peaks[index]);
                let brace_peak = if let Some(brace) = detached_brace {
                    if brace.staff_id() != first.staff_id() {
                        return Err(BarTailError::BracePeakStaffMismatch {
                            expected: first.staff_id().value(),
                            actual: brace.staff_id().value(),
                        });
                    }
                    if !brace.is_brace() {
                        return Err(BarTailError::InvalidBracePeak(brace.key()));
                    }
                    Some(group_peak_facts(brace, peak_graph))
                } else {
                    peaks
                        .iter()
                        .position(StaffPeak::is_brace)
                        .map(|index| facts[index])
                };
                staff_facts.push(GroupStaffFacts {
                    staff_id: i32::try_from(first.staff_id().value())
                        .map_err(|_| BarTailError::StaffIdOverflow(first.staff_id().value()))?,
                    peaks: facts,
                    start_peak_index,
                    brace_peak,
                    part_connected_below: !part_connection_edge_ids(
                        peak_graph,
                        first.staff_id(),
                        VerticalSide::Bottom,
                        start_peak,
                    )
                    .is_empty(),
                });
            }
        }
        let groups = create_part_groups(&staff_facts)?;
        result.part_groups = groups.clone();

        let first_staff = staff_peaks
            .iter()
            .find_map(|peaks| peaks.first())
            .ok_or(BarTailError::MissingStaffRange)?
            .staff_id()
            .value();
        let first_staff =
            i32::try_from(first_staff).map_err(|_| BarTailError::StaffIdOverflow(first_staff))?;
        let last_staff = staff_peaks
            .iter()
            .rev()
            .find_map(|peaks| peaks.first())
            .ok_or(BarTailError::MissingStaffRange)?
            .staff_id()
            .value();
        let last_staff =
            i32::try_from(last_staff).map_err(|_| BarTailError::StaffIdOverflow(last_staff))?;
        let decisions = groups
            .iter()
            .enumerate()
            .map(|(group_index, group)| {
                let first = StaffId::new(group.first_staff_id() as usize);
                let last = StaffId::new(group.last_staff_id() as usize);
                let first_start = staff_start_peak(staff_peaks, first);
                let last_start = staff_start_peak(staff_peaks, last);
                BraceGroupDecision {
                    group_index,
                    group,
                    is_true_brace: is_true_brace_group(
                        group,
                        !part_connection_edge_ids(
                            peak_graph,
                            first,
                            VerticalSide::Top,
                            first_start,
                        )
                        .is_empty(),
                        !part_connection_edge_ids(
                            peak_graph,
                            last,
                            VerticalSide::Bottom,
                            last_start,
                        )
                        .is_empty(),
                        !part_connection_edge_ids(
                            peak_graph,
                            first,
                            VerticalSide::Bottom,
                            first_start,
                        )
                        .is_empty(),
                        parameters.allow_disconnected_braced_parts,
                    ),
                }
            })
            .collect::<Vec<_>>();
        let plan = plan_parts(
            first_staff,
            last_staff,
            &decisions,
            parameters.force_separate_parts,
            parameters.force_single_part,
        )?;
        for &index in plan.removed_group_indices.iter().rev() {
            result.part_groups.remove(index);
        }
        result.parts = plan.parts;
        result.contextualized = false;
        Ok(())
    }

    /// Java `recordBars` with explicit system-owned staff identities, including
    /// staffs whose projector has no surviving peak.
    pub fn record_bars_for_staffs(
        &self,
        result: &mut BarTailResult,
        staff_ids: &[StaffId],
        staff_peaks: &[Vec<StaffPeak>],
    ) -> Result<(), BarTailError> {
        if staff_ids.len() != staff_peaks.len() {
            return Err(BarTailError::StaffPeakCount {
                staffs: staff_ids.len(),
                peak_sets: staff_peaks.len(),
            });
        }
        for (&staff_id, peaks) in staff_ids.iter().zip(staff_peaks) {
            if let Some(peak) = peaks.iter().find(|peak| peak.staff_id() != staff_id) {
                return Err(BarTailError::PeakStaffMismatch {
                    expected: staff_id.value(),
                    actual: peak.staff_id().value(),
                });
            }
            let bars = peaks
                .iter()
                .filter_map(|peak| self.inter_of(peak.key()))
                .filter(|&inter| {
                    matches!(
                        self.node(inter),
                        Some(GridSigNode::Vertical {
                            plan: VerticalInterPlan {
                                kind: VerticalInterKind::Barline { .. },
                                ..
                            },
                            ..
                        })
                    )
                })
                .collect::<Vec<_>>();
            // Java calls Staff.setBarlines after the whole projector scan.
            result.staff_barlines.insert(staff_id.value(), bars);
        }
        Ok(())
    }

    /// Java `createGroups` with explicit staff ownership and detached brace
    /// candidates. Previously completed `recordBars` mutations remain if this
    /// traversal encounters invalid later staff evidence.
    pub fn create_groups_for_staffs(
        &self,
        result: &mut BarTailResult,
        staff_ids: &[StaffId],
        staff_peaks: &[Vec<StaffPeak>],
        brace_peaks: &[Option<StaffPeak>],
        peak_graph: &PeakGraph<BarAlignment>,
    ) -> Result<(), BarTailError> {
        if staff_ids.len() != staff_peaks.len() {
            return Err(BarTailError::StaffPeakCount {
                staffs: staff_ids.len(),
                peak_sets: staff_peaks.len(),
            });
        }
        if brace_peaks.len() != staff_peaks.len() {
            return Err(BarTailError::BracePeakCount {
                staffs: staff_peaks.len(),
                braces: brace_peaks.len(),
            });
        }

        let facts = staff_peaks
            .iter()
            .map(|peaks| {
                peaks
                    .iter()
                    .map(|peak| group_peak_facts(peak, peak_graph))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut staff_facts = Vec::with_capacity(staff_ids.len());
        for (((&staff_id, peaks), facts), detached_brace) in staff_ids
            .iter()
            .zip(staff_peaks)
            .zip(&facts)
            .zip(brace_peaks)
        {
            if let Some(peak) = peaks.iter().find(|peak| peak.staff_id() != staff_id) {
                return Err(BarTailError::PeakStaffMismatch {
                    expected: staff_id.value(),
                    actual: peak.staff_id().value(),
                });
            }
            let start_peak_index = peaks
                .iter()
                .position(|peak| peak.is_staff_end(crate::staff_peak::HorizontalSide::Left));
            let start_peak = start_peak_index.map(|index| &peaks[index]);
            let brace_peak = if let Some(brace) = detached_brace {
                if brace.staff_id() != staff_id {
                    return Err(BarTailError::BracePeakStaffMismatch {
                        expected: staff_id.value(),
                        actual: brace.staff_id().value(),
                    });
                }
                if !brace.is_brace() {
                    return Err(BarTailError::InvalidBracePeak(brace.key()));
                }
                Some(group_peak_facts(brace, peak_graph))
            } else {
                peaks
                    .iter()
                    .position(StaffPeak::is_brace)
                    .map(|index| facts[index])
            };
            staff_facts.push(GroupStaffFacts {
                staff_id: i32::try_from(staff_id.value())
                    .map_err(|_| BarTailError::StaffIdOverflow(staff_id.value()))?,
                peaks: facts,
                start_peak_index,
                brace_peak,
                part_connected_below: !part_connection_edge_ids(
                    peak_graph,
                    staff_id,
                    VerticalSide::Bottom,
                    start_peak,
                )
                .is_empty(),
            });
        }
        result.part_groups = create_part_groups(&staff_facts)?;
        Ok(())
    }

    fn record_bars(
        &self,
        result: &mut BarTailResult,
        staff_peaks: &[Vec<StaffPeak>],
    ) -> Result<(), BarTailError> {
        for peaks in staff_peaks {
            let Some(first) = peaks.first() else {
                continue;
            };
            let staff_id = first.staff_id();
            let mut bars = Vec::new();
            for peak in peaks {
                let Some(inter) = self.inter_of(peak.key()) else {
                    continue;
                };
                if matches!(
                    self.node(inter),
                    Some(GridSigNode::Vertical {
                        plan: VerticalInterPlan {
                            kind: VerticalInterKind::Barline { .. },
                            ..
                        },
                        ..
                    })
                ) {
                    bars.push(inter);
                }
            }
            // Java calls Staff.setBarlines after the whole peak scan.
            result.staff_barlines.insert(staff_id.value(), bars);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BarTailResult {
    pub staff_barlines: BTreeMap<usize, Vec<GridInterId>>,
    pub part_groups: Vec<PartGroup>,
    pub parts: Vec<PlannedPart>,
    /// Set after the separate final system contextualization traversal.
    pub contextualized: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BarTailParameters {
    pub allow_disconnected_braced_parts: bool,
    pub force_separate_parts: bool,
    pub force_single_part: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarTailError {
    MissingStaffRange,
    StaffIdOverflow(usize),
    StaffPeakCount { staffs: usize, peak_sets: usize },
    PeakStaffMismatch { expected: usize, actual: usize },
    BracePeakCount { staffs: usize, braces: usize },
    BracePeakStaffMismatch { expected: usize, actual: usize },
    InvalidBracePeak(crate::staff_peak::StaffPeakKey),
    Logic(BarsLogicError),
}

impl From<BarsLogicError> for BarTailError {
    fn from(value: BarsLogicError) -> Self {
        Self::Logic(value)
    }
}

fn group_peak_facts(peak: &StaffPeak, graph: &PeakGraph<BarAlignment>) -> GroupPeakFacts {
    use crate::staff_peak::StaffPeakAttribute;
    GroupPeakFacts {
        brace: peak.is_brace(),
        bracket: peak.is_bracket(),
        bracket_top: peak.is_set(StaffPeakAttribute::BracketTop),
        bracket_bottom: peak.is_set(StaffPeakAttribute::BracketBottom),
        brace_top: peak.is_set(StaffPeakAttribute::BraceTop),
        brace_bottom: peak.is_set(StaffPeakAttribute::BraceBottom),
        connected_top: graph.edges().iter().any(|edge| {
            edge.relation().kind() == BarAlignmentKind::Connection && edge.target() == peak.key()
        }),
        connected_bottom: graph.edges().iter().any(|edge| {
            edge.relation().kind() == BarAlignmentKind::Connection && edge.source() == peak.key()
        }),
    }
}

fn staff_start_peak(staff_peaks: &[Vec<StaffPeak>], staff_id: StaffId) -> Option<&StaffPeak> {
    staff_peaks
        .iter()
        .flat_map(|peaks| peaks.iter())
        .find(|peak| {
            peak.staff_id() == staff_id
                && peak.is_staff_end(crate::staff_peak::HorizontalSide::Left)
        })
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
        staff_peak::{StaffPeak, StaffVerticalImpacts},
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

    fn graded_vertical_plan(peak: &StaffPeak, normalized_impact: f64) -> VerticalInterPlan {
        let mut plan = vertical_plan(peak);
        plan.impacts = Some(StaffVerticalImpacts::new(
            normalized_impact,
            normalized_impact,
            normalized_impact,
            normalized_impact,
            normalized_impact,
            normalized_impact,
        ));
        plan
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

    #[test]
    fn bar_groups_skip_structural_peaks_without_reset_and_use_inclusive_gap() {
        let first = peak(1, 10);
        let mut bracket = peak(1, 12);
        bracket.set(crate::staff_peak::StaffPeakAttribute::BracketMiddle);
        let at_limit = peak(1, 14);
        let beyond = peak(1, 19);
        let mut sig = GridSig::default();
        sig.promote_vertical_inters(&[
            vertical_plan(&first),
            vertical_plan(&bracket),
            vertical_plan(&at_limit),
            vertical_plan(&beyond),
        ]);
        sig.group_barlines(&[vec![first, bracket, at_limit, beyond]], 3, |gap| {
            f64::from(gap) / 10.0
        })
        .unwrap();
        let groups = sig
            .edges()
            .iter()
            .filter_map(|edge| match edge.relation {
                GridSigRelation::BarGroup { gap_fraction } => Some(gap_fraction),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(groups, [0.3]);
    }

    #[test]
    fn missing_group_inter_stops_after_retaining_prior_group_edges() {
        let first = peak(1, 10);
        let second = peak(1, 12);
        let missing = peak(1, 14);
        let mut sig = GridSig::default();
        sig.promote_vertical_inters(&[vertical_plan(&first), vertical_plan(&second)]);
        assert_eq!(
            sig.group_barlines(&[vec![first, second, missing.clone()]], 3, f64::from),
            Err(BarGroupPromotionError::MissingInter(missing.key()))
        );
        assert_eq!(
            sig.edges()
                .iter()
                .filter(|edge| matches!(edge.relation, GridSigRelation::BarGroup { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn bar_tail_records_staff_order_and_plans_default_parts() {
        let mut top = peak(1, 10);
        let mut bottom = peak(2, 10);
        top.set(crate::staff_peak::StaffPeakAttribute::StaffLeftEnd);
        bottom.set(crate::staff_peak::StaffPeakAttribute::StaffLeftEnd);
        let graph = connection_graph(top.clone(), bottom.clone(), 1.0);
        let mut sig = GridSig::default();
        let ids = sig.promote_vertical_inters(&[vertical_plan(&top), vertical_plan(&bottom)]);
        let mut tail = BarTailResult::default();

        sig.build_bar_tail(
            &mut tail,
            &[vec![top], vec![bottom]],
            &[None, None],
            &graph,
            BarTailParameters::default(),
        )
        .unwrap();

        assert_eq!(tail.staff_barlines[&1], [ids[0]]);
        assert_eq!(tail.staff_barlines[&2], [ids[1]]);
        assert!(tail.part_groups.is_empty());
        assert_eq!(
            tail.parts,
            [
                PlannedPart {
                    first_staff_id: 1,
                    last_staff_id: 1,
                },
                PlannedPart {
                    first_staff_id: 2,
                    last_staff_id: 2,
                },
            ]
        );
        assert!(!tail.contextualized);
    }

    #[test]
    fn disconnected_two_staff_brace_creates_one_part_and_removes_true_group() {
        use crate::staff_peak::StaffPeakAttribute;

        let mut top_brace = peak(1, 5);
        top_brace.set(StaffPeakAttribute::BraceTop);
        let mut top_bar = peak(1, 10);
        top_bar.set(StaffPeakAttribute::StaffLeftEnd);
        let mut bottom_brace = peak(2, 5);
        bottom_brace.set(StaffPeakAttribute::BraceBottom);
        let mut bottom_bar = peak(2, 10);
        bottom_bar.set(StaffPeakAttribute::StaffLeftEnd);
        let mut sig = GridSig::default();
        sig.promote_vertical_inters(&[vertical_plan(&top_bar), vertical_plan(&bottom_bar)]);
        let mut tail = BarTailResult::default();

        sig.build_bar_tail(
            &mut tail,
            &[vec![top_brace, top_bar], vec![bottom_brace, bottom_bar]],
            &[None, None],
            &PeakGraph::new(),
            BarTailParameters {
                allow_disconnected_braced_parts: true,
                ..BarTailParameters::default()
            },
        )
        .unwrap();

        assert!(tail.part_groups.is_empty());
        assert_eq!(
            tail.parts,
            [PlannedPart {
                first_staff_id: 1,
                last_staff_id: 2,
            }]
        );
    }

    #[test]
    fn detached_brace_matches_in_list_grouping_and_builds_one_part() {
        use crate::staff_peak::StaffPeakAttribute;

        let mut top_brace = peak(1, 5);
        top_brace.set(StaffPeakAttribute::BraceTop);
        let mut bottom_brace = peak(2, 5);
        bottom_brace.set(StaffPeakAttribute::BraceBottom);
        let mut top_bar = peak(1, 10);
        top_bar.set(StaffPeakAttribute::StaffLeftEnd);
        let mut bottom_bar = peak(2, 10);
        bottom_bar.set(StaffPeakAttribute::StaffLeftEnd);
        let mut sig = GridSig::default();
        sig.promote_vertical_inters(&[vertical_plan(&top_bar), vertical_plan(&bottom_bar)]);

        let mut in_list = BarTailResult::default();
        sig.build_bar_tail(
            &mut in_list,
            &[
                vec![top_brace.clone(), top_bar.clone()],
                vec![bottom_brace.clone(), bottom_bar.clone()],
            ],
            &[None, None],
            &PeakGraph::new(),
            BarTailParameters::default(),
        )
        .unwrap();
        let mut detached = BarTailResult::default();
        sig.build_bar_tail(
            &mut detached,
            &[vec![top_bar.clone()], vec![bottom_bar.clone()]],
            &[Some(top_brace.clone()), Some(bottom_brace.clone())],
            &PeakGraph::new(),
            BarTailParameters::default(),
        )
        .unwrap();

        assert_eq!(detached, in_list);
        assert_eq!(detached.part_groups.len(), 1);
        assert_eq!(detached.part_groups[0].first_staff_id(), 1);
        assert_eq!(detached.part_groups[0].last_staff_id(), 2);

        let mut merged = BarTailResult::default();
        sig.build_bar_tail(
            &mut merged,
            &[vec![top_bar], vec![bottom_bar]],
            &[Some(top_brace), Some(bottom_brace)],
            &PeakGraph::new(),
            BarTailParameters {
                allow_disconnected_braced_parts: true,
                ..BarTailParameters::default()
            },
        )
        .unwrap();
        assert!(merged.part_groups.is_empty());
        assert_eq!(
            merged.parts,
            [PlannedPart {
                first_staff_id: 1,
                last_staff_id: 2,
            }]
        );
    }

    #[test]
    fn record_bars_skips_missing_inters_and_non_barline_nodes() {
        let first = peak(1, 10);
        let missing = peak(1, 20);
        let mut bracket = peak(2, 10);
        bracket.set(crate::staff_peak::StaffPeakAttribute::BracketTop);
        let mut sig = GridSig::default();
        let mut bracket_plan = vertical_plan(&bracket);
        bracket_plan.kind = VerticalInterKind::Bracket(crate::bars_logic::BracketKind::Top);
        let ids = sig.promote_vertical_inters(&[vertical_plan(&first), bracket_plan]);
        let mut tail = BarTailResult::default();

        sig.build_bar_tail(
            &mut tail,
            &[vec![first, missing], vec![bracket]],
            &[None, None],
            &PeakGraph::new(),
            BarTailParameters::default(),
        )
        .unwrap();
        assert_eq!(tail.staff_barlines[&1], [ids[0]]);
        assert!(tail.staff_barlines[&2].is_empty());
        assert_eq!(tail.parts.len(), 2);
    }

    #[test]
    fn contextualize_sets_isolated_and_missing_impact_grades() {
        let missing = peak(1, 10);
        let graded = peak(2, 20);
        let mut sig = GridSig::default();
        let ids = sig.promote_vertical_inters(&[
            vertical_plan(&missing),
            graded_vertical_plan(&graded, 0.5),
        ]);
        let graded_intrinsic = sig.node(ids[1]).unwrap().intrinsic_grade();

        sig.contextualize();

        assert_eq!(sig.node(ids[0]).unwrap().intrinsic_grade(), 0.0);
        assert_eq!(sig.node(ids[0]).unwrap().contextual_grade(), Some(0.0));
        assert_eq!(
            sig.node(ids[1]).unwrap().contextual_grade(),
            Some(graded_intrinsic)
        );
    }

    #[test]
    fn contextualize_uses_only_bar_support_with_unequal_intrinsic_grades() {
        let low = peak(1, 10);
        let high = peak(2, 20);
        let mut sig = GridSig::default();
        let ids = sig.promote_vertical_inters(&[
            graded_vertical_plan(&low, 0.25),
            graded_vertical_plan(&high, 0.75),
        ]);
        sig.edges.push(GridSigEdge {
            source: ids[0],
            target: ids[1],
            relation: GridSigRelation::NoExclusion,
        });
        sig.edges.push(GridSigEdge {
            source: ids[0],
            target: ids[1],
            relation: GridSigRelation::BarGroup { gap_fraction: 0.1 },
        });
        sig.edges.push(GridSigEdge {
            source: ids[0],
            target: ids[1],
            relation: GridSigRelation::BarConnectionSupport { grade: 0.4 },
        });
        let low_grade = sig.node(ids[0]).unwrap().intrinsic_grade();
        let high_grade = sig.node(ids[1]).unwrap().intrinsic_grade();

        sig.contextualize();

        assert_eq!(
            sig.node(ids[0]).unwrap().contextual_grade(),
            Some(contextual_from_partners(low_grade, &[high_grade], &[3.0]).unwrap())
        );
        assert_eq!(
            sig.node(ids[1]).unwrap().contextual_grade(),
            Some(contextual_from_partners(high_grade, &[low_grade], &[3.0]).unwrap())
        );
    }

    #[test]
    fn contextualize_three_chain_sorts_partners_by_decreasing_intrinsic_grade() {
        let low = peak(1, 10);
        let center = peak(2, 20);
        let high = peak(3, 30);
        let mut sig = GridSig::default();
        let ids = sig.promote_vertical_inters(&[
            graded_vertical_plan(&low, 0.2),
            graded_vertical_plan(&center, 0.5),
            graded_vertical_plan(&high, 0.8),
        ]);
        // Deliberately insert the lower-grade partner first.
        sig.edges.push(GridSigEdge {
            source: ids[0],
            target: ids[1],
            relation: GridSigRelation::BarConnectionSupport { grade: 0.2 },
        });
        sig.edges.push(GridSigEdge {
            source: ids[1],
            target: ids[2],
            relation: GridSigRelation::BarConnectionSupport { grade: 0.7 },
        });
        let grades = ids
            .iter()
            .map(|&id| sig.node(id).unwrap().intrinsic_grade())
            .collect::<Vec<_>>();

        sig.contextualize();

        let expected =
            contextual_from_partners(grades[1], &[grades[2], grades[0]], &[4.5, 2.0]).unwrap();
        assert_eq!(sig.node(ids[1]).unwrap().contextual_grade(), Some(expected));
    }

    #[test]
    fn contextualize_preserves_connectors_orphans_frozen_state_and_is_idempotent() {
        let top = peak(1, 10);
        let bottom = peak(2, 10);
        let graph = connection_graph(top.clone(), bottom.clone(), 1.0);
        let plans = plan_connection_inters(&graph, |_| true);
        let mut sig = GridSig::default();
        let verticals = sig.promote_vertical_inters(&[
            graded_vertical_plan(&top, 0.5),
            graded_vertical_plan(&bottom, 0.6),
        ]);
        sig.promote_connection_inters(&graph, &plans).unwrap();
        let connector = sig.nodes_in_order().last().unwrap().0;
        let edges_before = sig.edges.clone();
        let intrinsic_before = sig
            .nodes_in_order()
            .map(|(id, node)| (id, node.intrinsic_grade()))
            .collect::<Vec<_>>();

        sig.contextualize();
        let first = sig
            .nodes_in_order()
            .map(|(id, node)| (id, node.contextual_grade()))
            .collect::<Vec<_>>();
        sig.contextualize();

        assert_eq!(sig.edges, edges_before);
        assert_eq!(
            sig.nodes_in_order()
                .map(|(id, node)| (id, node.intrinsic_grade()))
                .collect::<Vec<_>>(),
            intrinsic_before
        );
        assert_eq!(
            sig.nodes_in_order()
                .map(|(id, node)| (id, node.contextual_grade()))
                .collect::<Vec<_>>(),
            first
        );
        assert!(matches!(
            sig.node(connector),
            Some(GridSigNode::Connector {
                frozen: true,
                contextual_grade: Some(_),
                ..
            })
        ));
        assert!(verticals.iter().all(|id| matches!(
            sig.node(*id),
            Some(GridSigNode::Vertical { frozen: true, .. })
        )));

        let mut orphan = GridSig::default();
        orphan.promote_vertical_inters(&[graded_vertical_plan(&bottom, 0.6)]);
        let warnings = orphan.promote_connection_inters(&graph, &plans).unwrap();
        assert_eq!(
            warnings[0].failure,
            ConnectionPromotionFailure::MissingTopInter
        );
        let orphan_connector = orphan.nodes_in_order().last().unwrap().0;
        orphan.contextualize();
        assert_eq!(
            orphan.node(orphan_connector).unwrap().contextual_grade(),
            Some(plans[0].grade)
        );
    }

    #[test]
    fn explicit_staff_ownership_records_empty_staff_and_preserves_error_prefix() {
        let first = peak(1, 10);
        let foreign = peak(1, 20);
        let mut sig = GridSig::default();
        let ids = sig.promote_vertical_inters(&[vertical_plan(&first), vertical_plan(&foreign)]);
        let mut result = BarTailResult::default();

        sig.record_bars_for_staffs(
            &mut result,
            &[StaffId::new(1), StaffId::new(2)],
            &[vec![first.clone()], Vec::new()],
        )
        .unwrap();
        assert_eq!(result.staff_barlines[&1], [ids[0]]);
        assert!(result.staff_barlines[&2].is_empty());

        let mut partial = BarTailResult::default();
        assert_eq!(
            sig.record_bars_for_staffs(
                &mut partial,
                &[StaffId::new(1), StaffId::new(2)],
                &[vec![first], vec![foreign]],
            ),
            Err(BarTailError::PeakStaffMismatch {
                expected: 2,
                actual: 1,
            })
        );
        assert_eq!(partial.staff_barlines[&1], [ids[0]]);
        assert!(!partial.staff_barlines.contains_key(&2));
    }
}
