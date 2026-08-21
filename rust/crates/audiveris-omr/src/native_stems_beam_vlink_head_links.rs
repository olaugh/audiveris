// SPDX-License-Identifier: AGPL-3.0-or-later

//! Serial head links at the end of a beam-origin `VLinker.link` call.
//!
//! This boundary independently replays the complete boundary-16 transaction,
//! executes Java's insertion-ordered head-relation loop, evaluates the
//! deliberately inert remainder comparison, and stops after `VLinker.link`
//! returns `true` but before the caller assigns the outer B-linker flag again.
//! The compact v1 envelope accepts only live standard endpoints, the sole
//! standard `SigListener`, pre-populated relation metadata, and a non-manual
//! callback path. Public inputs cannot inject a listener or graph fault.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use crate::{
    head_template::HeadTemplateShape,
    native_heads_staff_epilog::NativeHeadStaffEpilogRef,
    native_sig::{
        NativeSigEdge, NativeSigEdgeId, NativeSigHeadStemPayload, NativeSigInterKind,
        NativeSigRelationKind, NativeSigRelationOrigin, NativeSigSupport, NativeSigSystem,
        NativeSigSystemBindings, NativeSigVertexId,
    },
    native_stems_beam_builders::NativeStemsBeamBuilderSystem,
    native_stems_beam_link_plans::{
        NativeStemsBeamHeadRelation, NativeStemsBeamLinkPlanAttempt, NativeStemsBeamLinkPlanSystem,
    },
    native_stems_beam_reachability::{
        NativeStemsBeamHeadCornerAction, NativeStemsBeamHeadCornerRef,
        NativeStemsBeamHeadDecisionAction, NativeStemsBeamReachabilitySystem,
        NativeStemsBeamVInspection,
    },
    native_stems_beam_scheduler::{
        NativeStemsBeamPlanRef, NativeStemsBeamSchedulerStatus, NativeStemsBeamSchedulerSystem,
    },
    native_stems_beam_stumps::NativeStemsBeamStumpSystem,
    native_stems_beam_vlink_b_linker_flag::NativeStemsBeamVLinkBLinkerFlagTransaction,
    native_stems_beam_vlink_base_apply::{
        NativeStemsBeamIncidentDirection, NativeStemsBeamSheetEditState,
        NativeStemsBeamSigListenerTopology, NativeStemsBeamVLinkBaseApplyKey,
        NativeStemsBeamVLinkBaseApplyTransaction,
    },
    native_stems_beam_vlink_reuse_check::{
        NativeStemsBeamRelationParameters, NativeStemsBeamVLinkReuseCheck,
        NativeStemsBeamVLinkReuseLiveState,
    },
    native_stems_beam_vlink_sibling_links::{
        NativeStemsBeamNativeBLinkerCell, NativeStemsBeamNativeSiblingTransaction,
        NativeStemsBeamSiblingRelationObjectIdentity, NativeStemsBeamVLinkSiblingLinksState,
        NativeStemsBeamVLinkSiblingLinksTransaction,
        apply_native_stems_beam_vlink_sibling_links_transaction,
        initialize_native_stems_beam_b_linker_cells,
    },
    native_stems_beam_vlink_transaction::NativeStemsBeamVLinkTransaction,
    native_stems_beam_vlinkers::NativeStemsBeamVLinkerSystem,
    native_stems_head_corners::{NativeStemsHeadCornerHead, NativeStemsHeadCornerSystem},
    stems_step::{NativeStemHeadSide, NativeStemPoint, NativeStemVerticalSide},
};

const HEAD_STEM_CLASS: &str = "org.audiveris.omr.sig.relation.HeadStemRelation";
const NEUTRAL_STEM_LENGTH: f64 = 2.8;

/// Stable identity of one live Java `HeadInter` participating in the plan.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct NativeStemsBeamHeadLinkHeadRef {
    pub reference: NativeHeadStaffEpilogRef,
    pub sig_ordinal: usize,
    pub x_ordinal: usize,
}

impl NativeStemsBeamHeadLinkHeadRef {
    fn from_corner(corner: NativeStemsBeamHeadCornerRef) -> Self {
        Self {
            reference: corner.head,
            sig_ordinal: corner.sig_ordinal,
            x_ordinal: corner.x_ordinal,
        }
    }
}

/// Exact live head object/registry/SIG state at this seam.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamHeadLinkHeadState {
    pub reference: NativeStemsBeamHeadLinkHeadRef,
    pub alias: String,
    pub runtime_class: String,
    pub inter_id: i32,
    pub inter_index_ordinal: usize,
    pub inter_index_object_matches: usize,
    pub inter_index_id_matches: usize,
    pub sig_vertex_identity: usize,
    pub sig_object_matches: usize,
    pub sig_system_id: usize,
    pub removed: bool,
    pub vip: bool,
    pub manual: bool,
    pub abnormal: bool,
    pub center: (i32, i32),
    /// Exact Java `Shape.name()` token. Compact v1 derives both predicates
    /// below from this value instead of trusting independent booleans.
    pub shape: String,
    pub is_small: bool,
    /// Exact result of `ShapeSet.StemHeads.contains(head.getShape())`.
    pub is_stem_head: bool,
}

/// The shared `HeadLinker.SLinker` field observed by both vertical C children.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct NativeStemsBeamHeadSLinkerRef {
    pub head: NativeStemsBeamHeadLinkHeadRef,
    pub horizontal: NativeStemHeadSide,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamHeadSLinkerCell {
    pub reference: NativeStemsBeamHeadSLinkerRef,
    pub linked: bool,
    pub closed: bool,
    /// Java `EnumMap<VerticalSide,CLinker>` order: TOP, then BOTTOM.
    pub ordered_observer_corners: Vec<NativeStemsBeamHeadCornerRef>,
}

/// Persistent native S-linker cell. The topology comes from the complete
/// head-corner constructor product, never from a B17 fixture row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamNativeSLinkerCell {
    pub reference: NativeStemsBeamHeadSLinkerRef,
    pub linked: bool,
    pub closed: bool,
    /// Exact `EnumMap<VerticalSide,CLinker>` order: TOP then BOTTOM.
    pub ordered_observer_corners: Vec<NativeStemsBeamHeadCornerRef>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamNativeHeadStep {
    pub map_ordinal: usize,
    pub corner: NativeStemsBeamHeadCornerRef,
    pub head_vertex: NativeSigVertexId,
    pub stem_vertex: NativeSigVertexId,
    pub s_linker: NativeStemsBeamHeadSLinkerRef,
    pub linked_before: bool,
    pub linked_after: bool,
    pub closed_before: bool,
    pub closed_after: bool,
    pub directed_pair_before: Vec<NativeSigEdgeId>,
    pub consistency: Option<f64>,
    pub appended_edge: Option<NativeSigEdgeId>,
    pub head_incident_after: Option<Vec<NativeSigEdgeId>>,
    pub stem_incident_after: Option<Vec<NativeSigEdgeId>>,
    pub head_abnormal_before: bool,
    pub head_abnormal_after: bool,
    pub stem_abnormal_before: bool,
    pub stem_abnormal_after: bool,
    pub branch: NativeStemsBeamHeadLinkBranch,
}

/// Native graph/S-cell delta for the first carried B17 transaction. Sheet and
/// Book dirty assignments are counted, but their external state is not owned
/// by this bounded carrier yet.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamNativeHeadTransaction {
    pub system_id: usize,
    pub plan_ordinal: usize,
    pub stem_identity: usize,
    pub steps: Vec<NativeStemsBeamNativeHeadStep>,
    pub assigned_s_linkers: Vec<NativeStemsBeamHeadSLinkerRef>,
    pub appended_edges: Vec<NativeSigEdgeId>,
    pub s_linker_write_count: usize,
    pub s_linker_value_change_count: usize,
    pub head_abnormal_value_change_count: usize,
    pub stem_abnormal_value_change_count: usize,
    pub dirty_cascade_assignment_count: usize,
    pub last_index: i32,
    pub max_index: i32,
    pub remainder_less_than: bool,
    pub returned_true: bool,
}

/// Build the exhaustive native S-cell catalogue: two cells per live head,
/// each observing its TOP then BOTTOM C child.
pub fn initialize_native_stems_beam_s_linker_cells(
    corners: &NativeStemsHeadCornerSystem,
) -> Result<Vec<NativeStemsBeamNativeSLinkerCell>, NativeStemsBeamVLinkHeadLinksError> {
    let mut x_ordinals = vec![None; corners.heads_in_sig_order.len()];
    for (x_ordinal, &sig_ordinal) in corners.heads_by_abscissa.iter().enumerate() {
        let slot = x_ordinals.get_mut(sig_ordinal).ok_or(
            NativeStemsBeamVLinkHeadLinksError::Predecessor {
                phase: "native S-cell x order",
            },
        )?;
        if slot.replace(x_ordinal).is_some() {
            return Err(NativeStemsBeamVLinkHeadLinksError::Predecessor {
                phase: "duplicate native S-cell x order",
            });
        }
    }
    let mut result = Vec::with_capacity(corners.heads_in_sig_order.len() * 2);
    for (sig_ordinal, head) in corners.heads_in_sig_order.iter().enumerate() {
        let x_ordinal =
            x_ordinals[sig_ordinal].ok_or(NativeStemsBeamVLinkHeadLinksError::Predecessor {
                phase: "missing native S-cell x order",
            })?;
        if head.corners_in_constructor_order.len() != 4 {
            return Err(NativeStemsBeamVLinkHeadLinksError::Predecessor {
                phase: "native S-cell corner count",
            });
        }
        for (side, start) in [
            (NativeStemHeadSide::Left, 0),
            (NativeStemHeadSide::Right, 2),
        ] {
            let observer = |offset: usize| NativeStemsBeamHeadCornerRef {
                head: head.reference,
                sig_ordinal,
                x_ordinal,
                horizontal: side,
                vertical: head.corners_in_constructor_order[start + offset].vertical,
            };
            let top = observer(0);
            let bottom = observer(1);
            if top.vertical != NativeStemVerticalSide::Top
                || bottom.vertical != NativeStemVerticalSide::Bottom
                || head.corners_in_constructor_order[start].horizontal != side
                || head.corners_in_constructor_order[start + 1].horizontal != side
            {
                return Err(NativeStemsBeamVLinkHeadLinksError::Predecessor {
                    phase: "native S-cell constructor order",
                });
            }
            result.push(NativeStemsBeamNativeSLinkerCell {
                reference: NativeStemsBeamHeadSLinkerRef {
                    head: NativeStemsBeamHeadLinkHeadRef {
                        reference: head.reference,
                        sig_ordinal,
                        x_ordinal,
                    },
                    horizontal: side,
                },
                linked: false,
                closed: false,
                ordered_observer_corners: vec![top, bottom],
            });
        }
    }
    Ok(result)
}

/// Resume from the owned post-B16 graph and atomically commit Java's ordered
/// B17 head loop to that graph and the persistent native S-cell catalogue.
/// Legacy Java object IDs, aliases, hashes, and certificate rows are not part
/// of this API.
#[allow(clippy::too_many_arguments)]
pub fn apply_native_stems_beam_vlink_head_transaction_to_native_sig(
    sig: &mut NativeSigSystem,
    bindings: &NativeSigSystemBindings,
    scheduler: &NativeStemsBeamSchedulerSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    builders: &NativeStemsBeamBuilderSystem,
    corners: &NativeStemsHeadCornerSystem,
    reachability: &NativeStemsBeamReachabilitySystem,
    b15: &NativeStemsBeamVLinkBLinkerFlagTransaction,
    b16: &NativeStemsBeamNativeSiblingTransaction,
    b_cells: &[NativeStemsBeamNativeBLinkerCell],
    s_cells: &mut Vec<NativeStemsBeamNativeSLinkerCell>,
) -> Result<NativeStemsBeamNativeHeadTransaction, NativeStemsBeamVLinkHeadLinksError> {
    let frontier = match &scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => return Err(NativeStemsBeamVLinkHeadLinksError::PredecessorNotReady),
    };
    let system_id = scheduler.system_id;
    if system_id != sig.system_id
        || system_id != bindings.system_id
        || system_id != plans.system_id
        || system_id != builders.system_id
        || system_id != corners.system_id
        || system_id != reachability.system_id
        || plans.interline != reachability.interline
        || plans.interline != corners.interline
        || frontier.plan != b15.key.plan
        || b16.system_id != system_id
        || b16.plan_ordinal != frontier.plan.plan_ordinal
        || b16.stem_identity != b15.stem_after.stem_identity
        || !b15.linked_after
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::PredecessorMismatch);
    }
    sig.validate_integrity()
        .map_err(|_| NativeStemsBeamVLinkHeadLinksError::InvalidState {
            phase: "native B17 SIG integrity",
        })?;
    bindings.validate_against(sig).map_err(|_| {
        NativeStemsBeamVLinkHeadLinksError::InvalidState {
            phase: "native B17 bindings",
        }
    })?;
    let expected_b_cells =
        initialize_native_stems_beam_b_linker_cells(reachability).map_err(|_| {
            NativeStemsBeamVLinkHeadLinksError::InvalidState {
                phase: "native B17 B-cell topology",
            }
        })?;
    if expected_b_cells.len() != b_cells.len()
        || expected_b_cells
            .iter()
            .map(|cell| cell.reference)
            .ne(b_cells.iter().map(|cell| cell.reference))
        || !b_cells
            .iter()
            .any(|cell| cell.reference == b15.target_b_linker && cell.linked)
        || b16.assigned_b_linkers.iter().any(|reference| {
            !b_cells
                .iter()
                .any(|cell| cell.reference == *reference && cell.linked)
        })
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
            phase: "native B17 B-cell continuity",
        });
    }
    let expected_s_cells = initialize_native_stems_beam_s_linker_cells(corners)?;
    if expected_s_cells.len() != s_cells.len()
        || expected_s_cells
            .iter()
            .zip(s_cells.iter())
            .any(|(expected, actual)| {
                expected.reference != actual.reference
                    || expected.ordered_observer_corners != actual.ordered_observer_corners
            })
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
            phase: "native B17 S-cell topology",
        });
    }
    let plan = resolve_plan(plans, frontier.plan)?;
    let builder = builders.builders.get(frontier.plan.builder_ordinal).ok_or(
        NativeStemsBeamVLinkHeadLinksError::Predecessor {
            phase: "native B17 builder",
        },
    )?;
    let inspection = resolve_reachability_inspection(
        reachability,
        b15.triggering_v_linker,
        system_id,
        plans.interline,
    )?;
    if plan.relations.is_empty()
        || plan.relations.len() != plan.head_target_count
        || plan
            .relations
            .iter()
            .enumerate()
            .any(|(ordinal, relation)| {
                relation.map_ordinal != ordinal
                    || !relation.check.accepted
                    || relation.check.extension_point.is_none()
                    || !inspection.head_targets.contains(&relation.corner)
            })
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::Predecessor {
            phase: "native B17 relation map",
        });
    }
    let stem_vertex = bindings
        .stem_vertices
        .get(&b16.stem_identity)
        .copied()
        .ok_or(NativeStemsBeamVLinkHeadLinksError::InvalidState {
            phase: "native B17 stem binding",
        })?;
    let base_edges = sig
        .directed_edges(b16.graph.base_vertex.0, stem_vertex.0)
        .map_err(|_| NativeStemsBeamVLinkHeadLinksError::InvalidState {
            phase: "native B17 base edge query",
        })?;
    if !base_edges.iter().any(|edge| {
        edge.kind == NativeSigRelationKind::BeamStem
            && edge.origin
                == (NativeSigRelationOrigin::BeamVBaseDraft {
                    plan_ordinal: frontier.plan.plan_ordinal,
                })
    }) || b16.graph.appended_edges.iter().any(|id| {
        sig.edges.get(id.0).is_none_or(|edge| {
            !edge.active
                || edge.kind != NativeSigRelationKind::BeamStem
                || edge.target != stem_vertex.0
                || !matches!(
                    edge.origin,
                    NativeSigRelationOrigin::BeamVSiblingDraft { plan_ordinal, .. }
                        if plan_ordinal == frontier.plan.plan_ordinal
                )
        })
    }) {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
            phase: "native B17 post-B16 graph continuity",
        });
    }
    let stem =
        sig.vertex(stem_vertex.0)
            .ok_or(NativeStemsBeamVLinkHeadLinksError::InvalidState {
                phase: "native B17 stem vertex",
            })?;
    // A freshly created stem is abnormal before its first head callback, but
    // a B13-selected existing stem can already be normal. B17 owns the live
    // callback transition below, so only the runtime class is a predecessor
    // invariant here.
    if stem.kind != NativeSigInterKind::Stem {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
            phase: "native B17 post-B16 stem",
        });
    }

    let mut shadow_sig = sig.clone();
    let mut shadow_cells = s_cells.clone();
    let mut steps = Vec::with_capacity(plan.relations.len());
    let mut assigned_s_linkers = Vec::with_capacity(plan.relations.len());
    let mut appended_edges = Vec::new();
    let mut s_linker_value_change_count = 0;
    let mut head_abnormal_value_change_count = 0;
    let mut stem_abnormal_value_change_count = 0;
    let mut dirty_cascade_assignment_count = 0;
    for relation in &plan.relations {
        let head_vertex = bindings
            .head_vertices
            .get(&relation.corner.head)
            .copied()
            .ok_or(NativeStemsBeamVLinkHeadLinksError::InvalidState {
                phase: "native B17 head binding",
            })?;
        let head = native_corner_head(corners, relation.corner)?;
        let graph_head = shadow_sig.vertex(head_vertex.0).ok_or(
            NativeStemsBeamVLinkHeadLinksError::InvalidState {
                phase: "native B17 head vertex",
            },
        )?;
        if graph_head.kind != NativeSigInterKind::Head
            || graph_head.shape.as_deref() != Some(native_head_shape(head.shape))
        {
            return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
                phase: "native B17 head shape",
            });
        }
        let s_ref = NativeStemsBeamHeadSLinkerRef {
            head: NativeStemsBeamHeadLinkHeadRef::from_corner(relation.corner),
            horizontal: relation.check.derived_horizontal,
        };
        let cell = unique_native_s_cell_mut(&mut shadow_cells, s_ref)?;
        let linked_before = cell.linked;
        let closed_before = cell.closed;
        cell.linked = true;
        s_linker_value_change_count += usize::from(!linked_before);
        assigned_s_linkers.push(s_ref);

        let directed_pair_before = shadow_sig
            .directed_edges(head_vertex.0, stem_vertex.0)
            .map_err(|_| NativeStemsBeamVLinkHeadLinksError::InvalidState {
                phase: "native B17 pair query",
            })?
            .iter()
            .map(|edge| NativeSigEdgeId(edge.ordinal))
            .collect::<Vec<_>>();
        let existing = directed_pair_before
            .iter()
            .any(|id| shadow_sig.edges[id.0].kind == NativeSigRelationKind::HeadStem);
        let head_abnormal_before = shadow_sig.vertices[head_vertex.0].abnormal;
        let stem_abnormal_before = shadow_sig.vertices[stem_vertex.0].abnormal;
        let mut consistency = None;
        let mut appended_edge = None;
        let mut head_incident_after = None;
        let mut stem_incident_after = None;
        let branch = if existing {
            NativeStemsBeamHeadLinkBranch::ExistingHeadStem
        } else {
            let value = head_stem_consistency(
                false,
                b15.stem_after.geometry.median.start.y,
                b15.stem_after.geometry.median.stop.y,
                plans.interline,
            )?;
            let extension = relation.check.extension_point.ok_or(
                NativeStemsBeamVLinkHeadLinksError::Predecessor {
                    phase: "native B17 extension",
                },
            )?;
            let edge_id = NativeSigEdgeId(shadow_sig.edges.len());
            shadow_sig
                .append_edge(NativeSigEdge {
                    ordinal: edge_id.0,
                    active: true,
                    source: head_vertex.0,
                    target: stem_vertex.0,
                    kind: NativeSigRelationKind::HeadStem,
                    origin: NativeSigRelationOrigin::BeamVHeadDraft {
                        plan_ordinal: frontier.plan.plan_ordinal,
                        map_ordinal: relation.map_ordinal,
                    },
                    support: Some(NativeSigSupport {
                        grade: relation.check.grade,
                        bar_connection_impacts: None,
                    }),
                    beam_portion: None,
                    stem_extension: None,
                    head_stem: Some(NativeSigHeadStemPayload {
                        dx: relation.check.dx,
                        dy: relation.check.dy,
                        head_side: relation.check.derived_horizontal,
                        extension_point: extension,
                        consistency: value,
                        manual: false,
                    }),
                })
                .map_err(|_| NativeStemsBeamVLinkHeadLinksError::InvalidState {
                    phase: "native B17 edge append",
                })?;
            let head_scan = shadow_sig
                .incident_edges(head_vertex.0)
                .map_err(|_| NativeStemsBeamVLinkHeadLinksError::InvalidState {
                    phase: "native B17 head incident query",
                })?
                .iter()
                .map(|edge| NativeSigEdgeId(edge.ordinal))
                .collect::<Vec<_>>();
            let stem_scan = shadow_sig
                .incident_edges(stem_vertex.0)
                .map_err(|_| NativeStemsBeamVLinkHeadLinksError::InvalidState {
                    phase: "native B17 stem incident query",
                })?
                .iter()
                .map(|edge| NativeSigEdgeId(edge.ordinal))
                .collect::<Vec<_>>();
            let head_after = if native_head_reads_head_stem(head.shape) {
                !head_scan
                    .iter()
                    .any(|id| shadow_sig.edges[id.0].kind == NativeSigRelationKind::HeadStem)
            } else {
                head_abnormal_before
            };
            let stem_after = !stem_scan
                .iter()
                .any(|id| shadow_sig.edges[id.0].kind == NativeSigRelationKind::HeadStem);
            shadow_sig
                .set_abnormal(head_vertex, head_after)
                .and_then(|()| shadow_sig.set_abnormal(stem_vertex, stem_after))
                .map_err(|_| NativeStemsBeamVLinkHeadLinksError::InvalidState {
                    phase: "native B17 abnormal callback",
                })?;
            if head_after != head_abnormal_before {
                head_abnormal_value_change_count += 1;
                dirty_cascade_assignment_count += 1;
            }
            if stem_after != stem_abnormal_before {
                stem_abnormal_value_change_count += 1;
                dirty_cascade_assignment_count += 1;
            }
            consistency = Some(value);
            appended_edge = Some(edge_id);
            appended_edges.push(edge_id);
            head_incident_after = Some(head_scan);
            stem_incident_after = Some(stem_scan);
            NativeStemsBeamHeadLinkBranch::Linked
        };
        let cell_after = unique_native_s_cell(&shadow_cells, s_ref)?;
        steps.push(NativeStemsBeamNativeHeadStep {
            map_ordinal: relation.map_ordinal,
            corner: relation.corner,
            head_vertex,
            stem_vertex,
            s_linker: s_ref,
            linked_before,
            linked_after: cell_after.linked,
            closed_before,
            closed_after: cell_after.closed,
            directed_pair_before,
            consistency,
            appended_edge,
            head_incident_after,
            stem_incident_after,
            head_abnormal_before,
            head_abnormal_after: shadow_sig.vertices[head_vertex.0].abnormal,
            stem_abnormal_before,
            stem_abnormal_after: shadow_sig.vertices[stem_vertex.0].abnormal,
            branch,
        });
    }
    shadow_sig.validate_integrity().map_err(|_| {
        NativeStemsBeamVLinkHeadLinksError::InvalidState {
            phase: "native B17 committed SIG",
        }
    })?;
    let last_index = plan.expand_last_index.unwrap_or(-1);
    let max_index = i32::try_from(builder.items.len())
        .ok()
        .and_then(|count| count.checked_sub(1))
        .ok_or(NativeStemsBeamVLinkHeadLinksError::UnsupportedV1 {
            phase: "native B17 builder max index",
        })?;
    let result = NativeStemsBeamNativeHeadTransaction {
        system_id,
        plan_ordinal: frontier.plan.plan_ordinal,
        stem_identity: b16.stem_identity,
        steps,
        assigned_s_linkers,
        appended_edges,
        s_linker_write_count: plan.relations.len(),
        s_linker_value_change_count,
        head_abnormal_value_change_count,
        stem_abnormal_value_change_count,
        dirty_cascade_assignment_count,
        last_index,
        max_index,
        remainder_less_than: last_index < max_index,
        returned_true: true,
    };
    *sig = shadow_sig;
    *s_cells = shadow_cells;
    Ok(result)
}

fn native_corner_head(
    corners: &NativeStemsHeadCornerSystem,
    corner: NativeStemsBeamHeadCornerRef,
) -> Result<&NativeStemsHeadCornerHead, NativeStemsBeamVLinkHeadLinksError> {
    let head = corners.heads_in_sig_order.get(corner.sig_ordinal).ok_or(
        NativeStemsBeamVLinkHeadLinksError::Predecessor {
            phase: "native B17 corner SIG ordinal",
        },
    )?;
    let x_ordinal = corners
        .heads_by_abscissa
        .iter()
        .position(|&ordinal| ordinal == corner.sig_ordinal);
    if head.reference != corner.head
        || x_ordinal != Some(corner.x_ordinal)
        || !head.corners_in_constructor_order.iter().any(|candidate| {
            candidate.horizontal == corner.horizontal && candidate.vertical == corner.vertical
        })
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::Predecessor {
            phase: "native B17 corner topology",
        });
    }
    Ok(head)
}

fn native_head_shape(shape: HeadTemplateShape) -> &'static str {
    match shape {
        HeadTemplateShape::NoteheadBlack => "NOTEHEAD_BLACK",
        HeadTemplateShape::NoteheadVoid => "NOTEHEAD_VOID",
        HeadTemplateShape::WholeNote => "WHOLE_NOTE",
        HeadTemplateShape::Breve => "BREVE",
    }
}

fn native_head_reads_head_stem(shape: HeadTemplateShape) -> bool {
    matches!(
        shape,
        HeadTemplateShape::NoteheadBlack | HeadTemplateShape::NoteheadVoid
    )
}

fn unique_native_s_cell_mut(
    cells: &mut [NativeStemsBeamNativeSLinkerCell],
    reference: NativeStemsBeamHeadSLinkerRef,
) -> Result<&mut NativeStemsBeamNativeSLinkerCell, NativeStemsBeamVLinkHeadLinksError> {
    let mut matches = cells.iter_mut().filter(|cell| cell.reference == reference);
    let cell = matches
        .next()
        .ok_or(NativeStemsBeamVLinkHeadLinksError::InvalidState {
            phase: "missing native B17 S cell",
        })?;
    if matches.next().is_some() {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
            phase: "duplicate native B17 S cell",
        });
    }
    Ok(cell)
}

fn unique_native_s_cell(
    cells: &[NativeStemsBeamNativeSLinkerCell],
    reference: NativeStemsBeamHeadSLinkerRef,
) -> Result<&NativeStemsBeamNativeSLinkerCell, NativeStemsBeamVLinkHeadLinksError> {
    let mut matches = cells.iter().filter(|cell| cell.reference == reference);
    let cell = matches
        .next()
        .ok_or(NativeStemsBeamVLinkHeadLinksError::InvalidState {
            phase: "missing committed native B17 S cell",
        })?;
    if matches.next().is_some() {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
            phase: "duplicate committed native B17 S cell",
        });
    }
    Ok(cell)
}

/// Java relation-object identity. Dense graph identity is a separate domain.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum NativeStemsBeamHeadRelationObjectIdentity {
    GraphObject(usize),
    BaseDraft(usize),
    SiblingDraft {
        plan_ordinal: usize,
        sibling_ordinal: usize,
    },
    HeadDraft {
        plan_ordinal: usize,
        map_ordinal: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamHeadQueryRelationKind {
    HeadStem,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamHeadPairClassRead {
    ExaminedContinue,
    ExaminedMatchBreak,
    UnreadAfterBreak,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamHeadSourceOutgoingRelation {
    pub source_outgoing_ordinal: usize,
    pub graph_relation_identity: usize,
    pub relation_object_identity: NativeStemsBeamHeadRelationObjectIdentity,
    pub relation_class: String,
    pub target_vertex_identity: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamHeadPairRelation {
    pub pair_ordinal: usize,
    pub source_outgoing_ordinal: usize,
    pub graph_relation_identity: usize,
    pub relation_object_identity: NativeStemsBeamHeadRelationObjectIdentity,
    pub relation_class: String,
    pub kind: NativeStemsBeamHeadQueryRelationKind,
    pub class_read: NativeStemsBeamHeadPairClassRead,
}

/// Complete `getAllEdges(head,stem)` query and the complete source-outgoing
/// order from which Java derived it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamHeadDirectedPairScan {
    pub source_outgoing_scanned: usize,
    pub source_outgoing_provenance_sha256: String,
    pub source_outgoing_relations: Vec<NativeStemsBeamHeadSourceOutgoingRelation>,
    pub query_relation_count: usize,
    pub pair_provenance_sha256: String,
    pub relations: Vec<NativeStemsBeamHeadPairRelation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamHeadIncidentOpposite {
    Head(NativeStemsBeamHeadLinkHeadRef),
    Stem,
    OtherInter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamHeadIncidentClassRead {
    ExaminedContinue,
    ExaminedMatchBreak,
    UnreadAfterBreak,
}

/// One row of `edgesOf(inter)`, which is incoming insertion order followed by
/// outgoing insertion order in the pinned graph implementation.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamHeadIncidentRelation {
    pub incident_ordinal: usize,
    pub direction: NativeStemsBeamIncidentDirection,
    pub direction_ordinal: usize,
    pub graph_relation_identity: usize,
    pub relation_object_identity: NativeStemsBeamHeadRelationObjectIdentity,
    pub relation_class: String,
    pub kind: NativeStemsBeamHeadQueryRelationKind,
    pub opposite: NativeStemsBeamHeadIncidentOpposite,
    pub opposite_alias: String,
    pub opposite_inter_id: i32,
    pub opposite_vertex_identity: usize,
    pub class_read: NativeStemsBeamHeadIncidentClassRead,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamHeadIncidentScan {
    pub query_relation_count: usize,
    pub query_provenance_sha256: String,
    pub relations: Vec<NativeStemsBeamHeadIncidentRelation>,
}

/// `HeadInter.checkAbnormal` performs no graph read for a non-stem-head
/// shape. This explicit state prevents an exhaustive certificate from
/// overclaiming a Java read that never happened.
#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamHeadAbnormalScan {
    NotReadNonStemHead,
    Read(NativeStemsBeamHeadIncidentScan),
}

/// Exact pre-loop state of the already-created `HeadStemRelation` map value.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamHeadPlanDraft {
    pub relation_object_identity: NativeStemsBeamHeadRelationObjectIdentity,
    pub relation_class: String,
    pub relation: NativeStemsBeamHeadRelation,
    pub manual: bool,
    /// Already populated by `HeadStemRelation.checkRelation`; callback default
    /// selection must remain unread in compact v1.
    pub head_side_before: Option<NativeStemHeadSide>,
    /// Already populated by `HeadStemRelation.checkRelation`; callback anchor
    /// fallback must remain unread in compact v1.
    pub extension_point_before: Option<NativeStemPoint>,
    /// Java's private nullable field. Real plans enter this seam with `null`.
    pub consistency_before: Option<f64>,
    pub graph_matches_before: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamHeadLinkStepCertificate {
    pub map_ordinal: usize,
    pub corner: NativeStemsBeamHeadCornerRef,
    pub s_linker: NativeStemsBeamHeadSLinkerRef,
    pub plan_draft: NativeStemsBeamHeadPlanDraft,
    pub directed_pair: NativeStemsBeamHeadDirectedPairScan,
    /// Present only when the directed pair contains no HeadStem relation.
    pub expected_consistency_after: Option<f64>,
    /// Compact v1 excludes observable debug-log emission from
    /// `setConsistency`; this field is read only on the missing branch.
    pub consistency_debug_enabled: Option<bool>,
    pub add_edge_returned: Option<bool>,
    pub head_incident_after: Option<NativeStemsBeamHeadAbnormalScan>,
    pub stem_incident_after: Option<NativeStemsBeamHeadIncidentScan>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkHeadLinksCertificate {
    pub system_id: usize,
    pub headless: bool,
    pub listener_topology: NativeStemsBeamSigListenerTopology,
    pub interline: i32,
    pub neutral_stem_length: f64,
    pub steps: Vec<NativeStemsBeamHeadLinkStepCertificate>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamHeadAppendedRelation {
    pub graph_relation_identity: usize,
    pub relation_object_identity: NativeStemsBeamHeadRelationObjectIdentity,
    pub source_head: NativeStemsBeamHeadLinkHeadRef,
    pub source_vertex_identity: usize,
    pub target_stem_identity: usize,
    pub target_vertex_identity: usize,
    pub relation: NativeStemsBeamHeadRelation,
    pub consistency: f64,
}

/// Mutable compact state for one complete head loop.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkHeadLinksState {
    /// Exact state from which boundary 16 is independently replayed.
    pub sibling_links_state_before: NativeStemsBeamVLinkSiblingLinksState,
    /// Exact committed boundary-16 state, retained unchanged by this seam.
    pub sibling_links_state_after: NativeStemsBeamVLinkSiblingLinksState,
    /// First-seen union of the plan's live head objects.
    pub live_heads: Vec<NativeStemsBeamHeadLinkHeadState>,
    /// First-seen union of the plan's shared `(head,hSide)` cells.
    pub s_linker_cells: Vec<NativeStemsBeamHeadSLinkerCell>,
    pub stem_manual: bool,
    pub stem_abnormal: bool,
    pub appended_relations: Vec<NativeStemsBeamHeadAppendedRelation>,
    pub sheet_edit: NativeStemsBeamSheetEditState,
    pub certificate: Option<NativeStemsBeamVLinkHeadLinksCertificate>,
    pub committed: Option<NativeStemsBeamVLinkBaseApplyKey>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamHeadLinkBranch {
    ExistingHeadStem,
    Linked,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamVLinkHeadLinksOperation {
    SLinkerLinkedAssigned {
        map_ordinal: usize,
        target: NativeStemsBeamHeadSLinkerRef,
        before: bool,
        after: bool,
        closed_before: bool,
        closed_after: bool,
    },
    DirectedPairLookupCompleted {
        map_ordinal: usize,
        relations_read: usize,
        matched: bool,
    },
    HeadStemConsistencyAssigned {
        map_ordinal: usize,
        value: f64,
    },
    SigGlobalRelationInserted {
        map_ordinal: usize,
        graph_relation_identity: usize,
    },
    HeadOutgoingRelationInserted {
        map_ordinal: usize,
        graph_relation_identity: usize,
    },
    StemIncomingRelationInserted {
        map_ordinal: usize,
        graph_relation_identity: usize,
    },
    SigEdgeEventDispatched {
        map_ordinal: usize,
        graph_relation_identity: usize,
    },
    StandardSigListenerEdgeCallbackStarted {
        map_ordinal: usize,
    },
    HeadStemRelationCallbackStarted {
        map_ordinal: usize,
    },
    AutomaticManualFlagsRead {
        map_ordinal: usize,
        relation_manual_read: bool,
        relation_manual: bool,
        head_manual_read: bool,
        head_manual: bool,
        stem_manual_read: bool,
        stem_manual: bool,
        chord_branch_read: bool,
    },
    HeadAbnormalScanNotReadNonStemHead {
        map_ordinal: usize,
    },
    HeadAbnormalScanCompleted {
        map_ordinal: usize,
        relations_read: usize,
    },
    HeadAbnormalAssigned {
        map_ordinal: usize,
        before: bool,
        after: bool,
    },
    StemAbnormalScanCompleted {
        map_ordinal: usize,
        relations_read: usize,
    },
    StemAbnormalAssigned {
        map_ordinal: usize,
        before: bool,
        after: bool,
    },
    SheetStubModifiedSetTrue {
        map_ordinal: usize,
        subject: NativeStemsBeamHeadDirtySubject,
    },
    BookModifiedSetTrue {
        map_ordinal: usize,
        subject: NativeStemsBeamHeadDirtySubject,
    },
    BookDirtySetTrue {
        map_ordinal: usize,
        subject: NativeStemsBeamHeadDirtySubject,
    },
    HeadStemRelationCallbackCompleted {
        map_ordinal: usize,
    },
    StandardSigListenerEdgeCallbackCompleted {
        map_ordinal: usize,
    },
    RemainderCompared {
        last_index: i32,
        max_index: i32,
        less_than: bool,
        split_mutation_count: usize,
    },
    VLinkerReturnedTrue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamHeadDirtySubject {
    Head,
    Stem,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkHeadTrace {
    pub map_ordinal: usize,
    pub s_write_event_ordinal: usize,
    pub consistency_event_ordinal: Option<usize>,
    pub edge_event_ordinal: Option<usize>,
    pub callback_event_ordinal: Option<usize>,
    pub corner: NativeStemsBeamHeadCornerRef,
    pub head: NativeStemsBeamHeadLinkHeadRef,
    pub s_linker: NativeStemsBeamHeadSLinkerRef,
    pub s_linked_before: bool,
    pub s_linked_after: bool,
    pub s_closed_before: bool,
    pub s_closed_after: bool,
    pub branch: NativeStemsBeamHeadLinkBranch,
    pub directed_pair_relations_read: usize,
    pub consistency_before: Option<f64>,
    pub consistency_after: Option<f64>,
    pub default_head_side_branch_read: Option<bool>,
    pub default_extension_branch_read: Option<bool>,
    pub manual_chord_branch_read: Option<bool>,
    pub appended_relation: Option<NativeStemsBeamHeadAppendedRelation>,
    pub add_edge_returned: Option<bool>,
    pub callback_completed: bool,
    pub head_abnormal_before: Option<bool>,
    pub head_abnormal_after: Option<bool>,
    pub stem_abnormal_before: Option<bool>,
    pub stem_abnormal_after: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamVLinkHeadLinksOutcome {
    ReturnedTrueBeforeOuterBLinkerAssignment {
        stem_identity: usize,
        continuation_support_grade: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkHeadLinksTransaction {
    pub key: NativeStemsBeamVLinkBaseApplyKey,
    pub continuation_support_grade: f64,
    pub consumed_certificate: NativeStemsBeamVLinkHeadLinksCertificate,
    pub steps: Vec<NativeStemsBeamVLinkHeadTrace>,
    pub operations: Vec<NativeStemsBeamVLinkHeadLinksOperation>,
    pub appended_graph_relation_identities: Vec<usize>,
    pub assigned_s_linkers: Vec<NativeStemsBeamHeadSLinkerRef>,
    pub s_linker_write_count: usize,
    pub s_linker_value_change_count: usize,
    pub consistency_mutation_count: usize,
    pub sig_relation_mutation_count: usize,
    pub head_abnormal_assignment_count: usize,
    pub head_abnormal_mutation_count: usize,
    pub stem_abnormal_assignment_count: usize,
    pub stem_abnormal_mutation_count: usize,
    pub dirty_cascade_count: usize,
    pub sheet_edit_mutation_count: usize,
    pub event_count: usize,
    pub sibling_link_mutation_count: usize,
    pub head_link_mutation_count: usize,
    pub last_index: i32,
    pub max_index: i32,
    pub remainder_less_than: bool,
    pub split_mutation_count: usize,
    pub returned_true: bool,
    pub outcome: NativeStemsBeamVLinkHeadLinksOutcome,
    pub state_after: Box<NativeStemsBeamVLinkHeadLinksState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamVLinkHeadLinksError {
    Predecessor { phase: &'static str },
    PredecessorMismatch,
    PredecessorNotReady,
    InvalidState { phase: &'static str },
    InvalidEvidence { phase: &'static str },
    UnsupportedV1 { phase: &'static str },
    DefensiveCommitInvariant { phase: &'static str },
}

impl fmt::Display for NativeStemsBeamVLinkHeadLinksError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid beam VLink head-links boundary: {self:?}"
        )
    }
}

impl Error for NativeStemsBeamVLinkHeadLinksError {}

/// Execute the complete serial head-relation loop and stop immediately after
/// Java returns `true`, before the outer B-linker assignment. Evidence is
/// evaluated on a clone, so every public error preserves the caller's state.
#[allow(clippy::too_many_arguments)]
pub fn apply_native_stems_beam_vlink_head_links_transaction(
    scheduler_system: &NativeStemsBeamSchedulerSystem,
    plan_system: &NativeStemsBeamLinkPlanSystem,
    stump_system: &NativeStemsBeamStumpSystem,
    vlinker_system: &NativeStemsBeamVLinkerSystem,
    reachability_system: &NativeStemsBeamReachabilitySystem,
    builder_system: &NativeStemsBeamBuilderSystem,
    create_transaction: &NativeStemsBeamVLinkTransaction,
    reuse_live_state: &NativeStemsBeamVLinkReuseLiveState,
    relation_parameters: NativeStemsBeamRelationParameters,
    reuse_check: &NativeStemsBeamVLinkReuseCheck,
    base_apply_transaction: &NativeStemsBeamVLinkBaseApplyTransaction,
    b_linker_flag_transaction: &NativeStemsBeamVLinkBLinkerFlagTransaction,
    sibling_links_transaction: &NativeStemsBeamVLinkSiblingLinksTransaction,
    state: &mut NativeStemsBeamVLinkHeadLinksState,
) -> Result<NativeStemsBeamVLinkHeadLinksTransaction, NativeStemsBeamVLinkHeadLinksError> {
    if state.committed.is_some() {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
            phase: "one-shot transaction already committed",
        });
    }

    let mut sibling_replay_state = state.sibling_links_state_before.clone();
    let replayed = apply_native_stems_beam_vlink_sibling_links_transaction(
        scheduler_system,
        plan_system,
        stump_system,
        vlinker_system,
        reachability_system,
        builder_system,
        create_transaction,
        reuse_live_state,
        relation_parameters,
        reuse_check,
        base_apply_transaction,
        b_linker_flag_transaction,
        &mut sibling_replay_state,
    )
    .map_err(|_| NativeStemsBeamVLinkHeadLinksError::Predecessor {
        phase: "boundary-16 exact replay",
    })?;
    if &replayed != sibling_links_transaction
        || &sibling_replay_state != sibling_links_transaction.state_after.as_ref()
        || replayed.state_after.as_ref() != &sibling_replay_state
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::PredecessorMismatch);
    }
    if state.sibling_links_state_after != sibling_replay_state {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
            phase: "boundary-16 committed state join",
        });
    }

    let (stem_identity, support_grade) = match replayed.outcome {
        crate::native_stems_beam_vlink_sibling_links::NativeStemsBeamVLinkSiblingLinksOutcome::ReadyBeforeHeadRelationLoop {
            stem_identity,
            continuation_support_grade,
        } => (stem_identity, continuation_support_grade),
    };
    if !support_grade.is_finite()
        || support_grade.to_bits() != replayed.continuation_support_grade.to_bits()
        || replayed.stem_after.stem_identity != stem_identity
        || replayed.head_link_mutation_count != 0
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::PredecessorNotReady);
    }

    let frontier = match &scheduler_system.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => return Err(NativeStemsBeamVLinkHeadLinksError::PredecessorNotReady),
    };
    if frontier.plan != replayed.key.plan
        || scheduler_system.system_id != plan_system.system_id
        || scheduler_system.system_id != builder_system.system_id
        || scheduler_system.system_id != vlinker_system.system_id
        || scheduler_system.system_id != reachability_system.system_id
        || scheduler_system.system_id != stump_system.system_id
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::Predecessor {
            phase: "system/frontier products",
        });
    }

    let mut shadow = state.clone();
    let certificate =
        shadow
            .certificate
            .clone()
            .ok_or(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                phase: "missing one-shot certificate",
            })?;
    let prepared = prepare_and_commit_supported(
        HeadLinksPrepareInputs {
            plan_ref: frontier.plan,
            plan_system,
            reachability_system,
            builder_system,
            triggering_v_linker: b_linker_flag_transaction.triggering_v_linker,
            base_apply_transaction,
            predecessor: &replayed,
            reuse_live_state,
        },
        &certificate,
        &mut shadow,
    )?;
    shadow.certificate = None;
    shadow.committed = Some(replayed.key);

    let transaction = NativeStemsBeamVLinkHeadLinksTransaction {
        key: replayed.key,
        continuation_support_grade: support_grade,
        consumed_certificate: certificate,
        steps: prepared.steps,
        operations: prepared.operations,
        appended_graph_relation_identities: prepared.appended_graph_relation_identities,
        assigned_s_linkers: prepared.assigned_s_linkers,
        s_linker_write_count: prepared.s_linker_write_count,
        s_linker_value_change_count: prepared.s_linker_value_change_count,
        consistency_mutation_count: prepared.consistency_mutation_count,
        sig_relation_mutation_count: prepared.sig_relation_mutation_count,
        head_abnormal_assignment_count: prepared.head_abnormal_assignment_count,
        head_abnormal_mutation_count: prepared.head_abnormal_mutation_count,
        stem_abnormal_assignment_count: prepared.stem_abnormal_assignment_count,
        stem_abnormal_mutation_count: prepared.stem_abnormal_mutation_count,
        dirty_cascade_count: prepared.dirty_cascade_count,
        sheet_edit_mutation_count: prepared.sheet_edit_mutation_count,
        event_count: prepared.event_count,
        sibling_link_mutation_count: 0,
        head_link_mutation_count: prepared.sig_relation_mutation_count,
        last_index: prepared.last_index,
        max_index: prepared.max_index,
        remainder_less_than: prepared.remainder_less_than,
        split_mutation_count: 0,
        returned_true: true,
        outcome: NativeStemsBeamVLinkHeadLinksOutcome::ReturnedTrueBeforeOuterBLinkerAssignment {
            stem_identity,
            continuation_support_grade: support_grade,
        },
        state_after: Box::new(shadow.clone()),
    };
    *state = shadow;
    Ok(transaction)
}

struct PreparedHeadLinks {
    steps: Vec<NativeStemsBeamVLinkHeadTrace>,
    operations: Vec<NativeStemsBeamVLinkHeadLinksOperation>,
    appended_graph_relation_identities: Vec<usize>,
    assigned_s_linkers: Vec<NativeStemsBeamHeadSLinkerRef>,
    s_linker_write_count: usize,
    s_linker_value_change_count: usize,
    consistency_mutation_count: usize,
    sig_relation_mutation_count: usize,
    head_abnormal_assignment_count: usize,
    head_abnormal_mutation_count: usize,
    stem_abnormal_assignment_count: usize,
    stem_abnormal_mutation_count: usize,
    dirty_cascade_count: usize,
    sheet_edit_mutation_count: usize,
    event_count: usize,
    last_index: i32,
    max_index: i32,
    remainder_less_than: bool,
}

struct HeadLinksPrepareInputs<'a> {
    plan_ref: NativeStemsBeamPlanRef,
    plan_system: &'a NativeStemsBeamLinkPlanSystem,
    reachability_system: &'a NativeStemsBeamReachabilitySystem,
    builder_system: &'a NativeStemsBeamBuilderSystem,
    triggering_v_linker: crate::native_stems_beam_vlinkers::NativeStemsBeamVLinkerRef,
    base_apply_transaction: &'a NativeStemsBeamVLinkBaseApplyTransaction,
    predecessor: &'a NativeStemsBeamVLinkSiblingLinksTransaction,
    reuse_live_state: &'a NativeStemsBeamVLinkReuseLiveState,
}

fn prepare_and_commit_supported(
    inputs: HeadLinksPrepareInputs<'_>,
    certificate: &NativeStemsBeamVLinkHeadLinksCertificate,
    state: &mut NativeStemsBeamVLinkHeadLinksState,
) -> Result<PreparedHeadLinks, NativeStemsBeamVLinkHeadLinksError> {
    let HeadLinksPrepareInputs {
        plan_ref,
        plan_system,
        reachability_system,
        builder_system,
        triggering_v_linker,
        base_apply_transaction,
        predecessor,
        reuse_live_state,
    } = inputs;
    let system_id = plan_ref.system_id;
    let b16_state = state.sibling_links_state_after.clone();
    let base_state = &b16_state.base_apply_state_after;
    let stem = &predecessor.stem_after;
    let stem_vertex = base_state.sig.stem.sig_vertex_identity.ok_or(
        NativeStemsBeamVLinkHeadLinksError::UnsupportedV1 {
            phase: "head-link target stem absent from live SIG",
        },
    )?;
    let stem_inter_id = stem.inter_id.filter(|value| *value > 0).ok_or(
        NativeStemsBeamVLinkHeadLinksError::UnsupportedV1 {
            phase: "head-link target stem lacks positive Java ID",
        },
    )?;
    if base_state.sig.stem.removed
        || !base_state.sig.stem.inter_indexed
        || base_state.sig.stem.sig_system_id != Some(system_id)
        || certificate.system_id != system_id
        || !certificate.headless
        || certificate.listener_topology
            != NativeStemsBeamSigListenerTopology::SoleStandardSigListener
        || base_state.sig.listener_topology != certificate.listener_topology
        || certificate.interline != plan_system.interline
        || certificate.interline <= 0
        || certificate.neutral_stem_length.to_bits() != NEUTRAL_STEM_LENGTH.to_bits()
        || state.stem_manual
        || state.stem_abnormal != base_state.sig.stem.abnormal
        || state.sheet_edit != b16_state.sheet_edit
        || !state.appended_relations.is_empty()
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::UnsupportedV1 {
            phase: "compact listener/endpoint/manual/state envelope",
        });
    }

    let attempt = resolve_plan(plan_system, plan_ref)?;
    let builder = builder_system
        .builders
        .get(plan_ref.builder_ordinal)
        .ok_or(NativeStemsBeamVLinkHeadLinksError::Predecessor {
            phase: "builder ordinal",
        })?;
    if builder.builder_ordinal != plan_ref.builder_ordinal
        || attempt.stem_profile != plan_ref.stem_profile
        || attempt.relations.is_empty()
        || certificate.steps.len() != attempt.relations.len()
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::Predecessor {
            phase: "plan/builder head loop",
        });
    }
    let last_index =
        attempt
            .expand_last_index
            .ok_or(NativeStemsBeamVLinkHeadLinksError::Predecessor {
                phase: "missing successful expand lastIndex",
            })?;
    let max_index = i32::try_from(builder.items.len().checked_sub(1).ok_or(
        NativeStemsBeamVLinkHeadLinksError::Predecessor {
            phase: "empty StemBuilder items",
        },
    )?)
    .map_err(|_| NativeStemsBeamVLinkHeadLinksError::UnsupportedV1 {
        phase: "StemBuilder maxIndex overflow",
    })?;

    let vertex_limit = base_state
        .sig
        .baseline_vertex_count
        .checked_add(base_state.sig.appended_vertices.len())
        .ok_or(NativeStemsBeamVLinkHeadLinksError::UnsupportedV1 {
            phase: "SIG vertex identity overflow",
        })?;
    let inter_index_limit = base_state
        .inter_index
        .baseline_entry_count
        .checked_add(base_state.inter_index.appended_entries.len())
        .ok_or(NativeStemsBeamVLinkHeadLinksError::UnsupportedV1 {
            phase: "InterIndex ordinal overflow",
        })?;
    let reachability_inspection = resolve_reachability_inspection(
        reachability_system,
        triggering_v_linker,
        system_id,
        certificate.interline,
    )?;
    validate_live_heads_and_cells(
        &attempt.relations,
        &state.live_heads,
        &state.s_linker_cells,
        LiveStateValidationContext {
            system_id,
            stem_inter_id,
            stem_vertex,
            vertex_limit,
            inter_index_limit,
            b16_state: &b16_state,
            reachability_inspection,
            reuse_live_state,
        },
    )?;
    validate_step_headers(plan_ref, &attempt.relations, certificate)?;
    let mut relation_objects = initial_relation_object_map(&b16_state)?;
    if relation_objects.values().any(|object| {
        matches!(
            object,
            NativeStemsBeamHeadRelationObjectIdentity::HeadDraft { .. }
        )
    }) {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
            phase: "unattached head draft present in predecessor graph",
        });
    }
    let mut relation_payloads = BTreeMap::new();
    let mut relation_endpoints = initial_relation_endpoint_map(&b16_state)?;
    let mut relation_sources = relation_endpoints
        .iter()
        .map(|(identity, (source, _))| (*identity, *source))
        .collect::<BTreeMap<_, _>>();
    let mut endpoint_by_vertex =
        initial_endpoint_map(&state.live_heads, stem_inter_id, stem_vertex)?;
    let mut vertex_by_inter_id = endpoint_by_vertex
        .iter()
        .map(|(vertex, endpoint)| (endpoint.inter_id, *vertex))
        .collect::<BTreeMap<_, _>>();
    let mut prior_source_scans = BTreeMap::<
        NativeStemsBeamHeadLinkHeadRef,
        Vec<NativeStemsBeamHeadSourceOutgoingRelation>,
    >::new();
    let predecessor_stem_scan =
        predecessor_stem_incident_rows(predecessor, base_apply_transaction, &state.live_heads)?;
    seed_predecessor_stem_snapshot(
        &predecessor_stem_scan,
        stem_vertex,
        &mut relation_objects,
        &mut relation_payloads,
        &mut relation_endpoints,
        &mut relation_sources,
    )?;
    let mut prior_stem_scan: Option<NativeStemsBeamHeadIncidentScan> = None;
    let mut traces = Vec::with_capacity(attempt.relations.len());
    let mut operations = Vec::new();
    let mut appended_ids = Vec::new();
    let mut assigned_s_linkers = Vec::with_capacity(attempt.relations.len());
    let mut s_value_changes = 0;
    let mut head_abnormal_assignments = 0;
    let mut head_abnormal_mutations = 0;
    let mut stem_abnormal_assignments = 0;
    let mut stem_abnormal_mutations = 0;
    let mut dirty_cascades = 0;
    let mut event_count = 0;
    let starting_sheet = state.sheet_edit;

    for (map_ordinal, relation) in attempt.relations.iter().enumerate() {
        let step = &certificate.steps[map_ordinal];
        let head_ref = NativeStemsBeamHeadLinkHeadRef::from_corner(relation.corner);
        let head_index = state
            .live_heads
            .iter()
            .position(|head| head.reference == head_ref)
            .ok_or(NativeStemsBeamVLinkHeadLinksError::InvalidState {
                phase: "plan head missing from live catalogue",
            })?;
        let cell_ref = NativeStemsBeamHeadSLinkerRef {
            head: head_ref,
            horizontal: relation.corner.horizontal,
        };
        let cell_index = state
            .s_linker_cells
            .iter()
            .position(|cell| cell.reference == cell_ref)
            .ok_or(NativeStemsBeamVLinkHeadLinksError::InvalidState {
                phase: "plan S-linker cell missing",
            })?;
        let s_before = state.s_linker_cells[cell_index].linked;
        let closed_before = state.s_linker_cells[cell_index].closed;
        state.s_linker_cells[cell_index].linked = true;
        if !s_before {
            s_value_changes += 1;
        }
        assigned_s_linkers.push(cell_ref);
        let s_write_event_ordinal = event_count;
        event_count += 1;
        operations.push(
            NativeStemsBeamVLinkHeadLinksOperation::SLinkerLinkedAssigned {
                map_ordinal,
                target: cell_ref,
                before: s_before,
                after: true,
                closed_before,
                closed_after: closed_before,
            },
        );

        let next_graph_identity = next_relation_identity(&b16_state, &state.appended_relations)?;
        validate_pair_draft_visibility(&step.directed_pair, &state.appended_relations)?;
        let pair = validate_pair_scan(
            &step.directed_pair,
            PairValidationContext {
                graph_limit: next_graph_identity,
                source_head_vertex: state.live_heads[head_index].sig_vertex_identity,
                target_stem_vertex: stem_vertex,
                vertex_limit,
            },
            RelationValidationMaps {
                objects: &mut relation_objects,
                payloads: &mut relation_payloads,
                endpoints: &mut relation_endpoints,
                sources: &mut relation_sources,
            },
        )?;
        validate_source_scan_evolution(
            head_ref,
            state.live_heads[head_index].sig_vertex_identity,
            &step.directed_pair.source_outgoing_relations,
            &step.directed_pair.relations,
            &prior_source_scans,
            &state.appended_relations,
            &relation_endpoints,
        )?;
        prior_source_scans.insert(
            head_ref,
            step.directed_pair.source_outgoing_relations.clone(),
        );
        operations.push(
            NativeStemsBeamVLinkHeadLinksOperation::DirectedPairLookupCompleted {
                map_ordinal,
                relations_read: pair.relations_read,
                matched: pair.first_match.is_some(),
            },
        );

        let mut trace = NativeStemsBeamVLinkHeadTrace {
            map_ordinal,
            s_write_event_ordinal,
            consistency_event_ordinal: None,
            edge_event_ordinal: None,
            callback_event_ordinal: None,
            corner: relation.corner,
            head: head_ref,
            s_linker: cell_ref,
            s_linked_before: s_before,
            s_linked_after: true,
            s_closed_before: closed_before,
            s_closed_after: closed_before,
            branch: NativeStemsBeamHeadLinkBranch::ExistingHeadStem,
            directed_pair_relations_read: pair.relations_read,
            consistency_before: None,
            consistency_after: None,
            default_head_side_branch_read: None,
            default_extension_branch_read: None,
            manual_chord_branch_read: None,
            appended_relation: None,
            add_edge_returned: None,
            callback_completed: false,
            head_abnormal_before: None,
            head_abnormal_after: None,
            stem_abnormal_before: None,
            stem_abnormal_after: None,
        };

        if pair.first_match.is_some() {
            validate_existing_branch_unread(step)?;
            traces.push(trace);
            continue;
        }

        let head = state.live_heads[head_index].clone();
        if head.manual || state.stem_manual || step.plan_draft.manual {
            return Err(NativeStemsBeamVLinkHeadLinksError::UnsupportedV1 {
                phase: "manual HeadStem callback",
            });
        }
        let consistency = head_stem_consistency(
            head.is_small,
            stem.geometry.median.start.y,
            stem.geometry.median.stop.y,
            certificate.interline,
        )?;
        if step
            .expected_consistency_after
            .is_none_or(|value| value.to_bits() != consistency.to_bits())
            || step.consistency_debug_enabled != Some(false)
            || step.add_edge_returned != Some(true)
        {
            return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                phase: "consistency/addEdge result",
            });
        }
        let consistency_event_ordinal = event_count;
        event_count += 1;
        operations.push(
            NativeStemsBeamVLinkHeadLinksOperation::HeadStemConsistencyAssigned {
                map_ordinal,
                value: consistency,
            },
        );

        let object_identity = NativeStemsBeamHeadRelationObjectIdentity::HeadDraft {
            plan_ordinal: plan_ref.plan_ordinal,
            map_ordinal,
        };
        if relation_objects
            .values()
            .any(|value| *value == object_identity)
        {
            return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                phase: "plan draft object already attached",
            });
        }
        let appended = NativeStemsBeamHeadAppendedRelation {
            graph_relation_identity: next_graph_identity,
            relation_object_identity: object_identity,
            source_head: head_ref,
            source_vertex_identity: head.sig_vertex_identity,
            target_stem_identity: stem.stem_identity,
            target_vertex_identity: stem_vertex,
            relation: relation.clone(),
            consistency,
        };
        state.appended_relations.push(appended.clone());
        relation_objects.insert(next_graph_identity, object_identity);
        relation_payloads.insert(
            next_graph_identity,
            RelationPayload {
                object: object_identity,
                class: HEAD_STEM_CLASS.to_owned(),
                kind: Some(NativeStemsBeamHeadQueryRelationKind::HeadStem),
            },
        );
        join_relation_endpoints(
            &mut relation_endpoints,
            next_graph_identity,
            head.sig_vertex_identity,
            stem_vertex,
        )?;
        join_relation_source(
            &mut relation_sources,
            next_graph_identity,
            head.sig_vertex_identity,
        )?;
        let edge_event_ordinal = event_count;
        event_count += 1;
        append_edge_operations(map_ordinal, next_graph_identity, &mut operations);

        let head_scan = step.head_incident_after.as_ref().ok_or(
            NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                phase: "missing head callback scan",
            },
        )?;
        let stem_scan = step.stem_incident_after.as_ref().ok_or(
            NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                phase: "missing stem callback scan",
            },
        )?;
        if let NativeStemsBeamHeadAbnormalScan::Read(scan) = head_scan {
            validate_incident_draft_visibility(scan, &state.appended_relations)?;
        }
        validate_incident_draft_visibility(stem_scan, &state.appended_relations)?;
        let callback_event_ordinal = event_count;
        event_count += 1;
        operations.push(
            NativeStemsBeamVLinkHeadLinksOperation::StandardSigListenerEdgeCallbackStarted {
                map_ordinal,
            },
        );
        operations.push(
            NativeStemsBeamVLinkHeadLinksOperation::HeadStemRelationCallbackStarted { map_ordinal },
        );
        operations.push(
            NativeStemsBeamVLinkHeadLinksOperation::AutomaticManualFlagsRead {
                map_ordinal,
                relation_manual_read: true,
                relation_manual: false,
                head_manual_read: true,
                head_manual: false,
                stem_manual_read: true,
                stem_manual: false,
                chord_branch_read: false,
            },
        );

        let head_before = state.live_heads[head_index].abnormal;
        let head_after = if head.is_stem_head {
            let scan = match head_scan {
                NativeStemsBeamHeadAbnormalScan::Read(scan) => scan,
                NativeStemsBeamHeadAbnormalScan::NotReadNonStemHead => {
                    return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                        phase: "stem-head callback scan marked unread",
                    });
                }
            };
            validate_head_callback_outgoing_partition(
                &step.directed_pair.source_outgoing_relations,
                scan,
                next_graph_identity,
                object_identity,
                stem_vertex,
            )?;
            let read = validate_incident_scan(
                scan,
                IncidentValidationContext {
                    queried: IncidentQueried::Head(head_ref),
                    current_graph_identity: next_graph_identity,
                    current_object_identity: object_identity,
                    current_head: head_ref,
                    current_head_alias: &head.alias,
                    stem_alias: &b16_state.stem_alias,
                    stem_inter_id,
                    stem_vertex,
                    vertex_limit,
                    live_heads: &state.live_heads,
                },
                &mut relation_objects,
                &mut relation_payloads,
                &mut relation_endpoints,
                &mut relation_sources,
                &mut endpoint_by_vertex,
                &mut vertex_by_inter_id,
            )?;
            operations.push(
                NativeStemsBeamVLinkHeadLinksOperation::HeadAbnormalScanCompleted {
                    map_ordinal,
                    relations_read: read,
                },
            );
            head_abnormal_assignments += 1;
            operations.push(
                NativeStemsBeamVLinkHeadLinksOperation::HeadAbnormalAssigned {
                    map_ordinal,
                    before: head_before,
                    after: false,
                },
            );
            if head_before {
                state.live_heads[head_index].abnormal = false;
                head_abnormal_mutations += 1;
                dirty_cascades += 1;
                dirty_cascade(
                    map_ordinal,
                    NativeStemsBeamHeadDirtySubject::Head,
                    state,
                    &mut operations,
                );
            }
            false
        } else {
            if head_scan != &NativeStemsBeamHeadAbnormalScan::NotReadNonStemHead {
                return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                    phase: "non-stem head callback fabricated graph read",
                });
            }
            operations.push(
                NativeStemsBeamVLinkHeadLinksOperation::HeadAbnormalScanNotReadNonStemHead {
                    map_ordinal,
                },
            );
            head_before
        };

        let stem_before = state.stem_abnormal;
        let stem_read = validate_incident_scan(
            stem_scan,
            IncidentValidationContext {
                queried: IncidentQueried::Stem,
                current_graph_identity: next_graph_identity,
                current_object_identity: object_identity,
                current_head: head_ref,
                current_head_alias: &head.alias,
                stem_alias: &b16_state.stem_alias,
                stem_inter_id,
                stem_vertex,
                vertex_limit,
                live_heads: &state.live_heads,
            },
            &mut relation_objects,
            &mut relation_payloads,
            &mut relation_endpoints,
            &mut relation_sources,
            &mut endpoint_by_vertex,
            &mut vertex_by_inter_id,
        )?;
        validate_stem_scan_evolution(
            prior_stem_scan.as_ref(),
            stem_scan,
            next_graph_identity,
            &predecessor_stem_scan,
        )?;
        prior_stem_scan = Some(stem_scan.clone());
        operations.push(
            NativeStemsBeamVLinkHeadLinksOperation::StemAbnormalScanCompleted {
                map_ordinal,
                relations_read: stem_read,
            },
        );
        stem_abnormal_assignments += 1;
        operations.push(
            NativeStemsBeamVLinkHeadLinksOperation::StemAbnormalAssigned {
                map_ordinal,
                before: stem_before,
                after: false,
            },
        );
        if stem_before {
            state.stem_abnormal = false;
            stem_abnormal_mutations += 1;
            dirty_cascades += 1;
            dirty_cascade(
                map_ordinal,
                NativeStemsBeamHeadDirtySubject::Stem,
                state,
                &mut operations,
            );
        }
        operations.push(
            NativeStemsBeamVLinkHeadLinksOperation::HeadStemRelationCallbackCompleted {
                map_ordinal,
            },
        );
        operations.push(
            NativeStemsBeamVLinkHeadLinksOperation::StandardSigListenerEdgeCallbackCompleted {
                map_ordinal,
            },
        );

        trace.branch = NativeStemsBeamHeadLinkBranch::Linked;
        trace.consistency_event_ordinal = Some(consistency_event_ordinal);
        trace.edge_event_ordinal = Some(edge_event_ordinal);
        trace.callback_event_ordinal = Some(callback_event_ordinal);
        trace.consistency_after = Some(consistency);
        trace.default_head_side_branch_read = Some(false);
        trace.default_extension_branch_read = Some(false);
        trace.manual_chord_branch_read = Some(false);
        trace.appended_relation = Some(appended);
        trace.add_edge_returned = Some(true);
        trace.callback_completed = true;
        trace.head_abnormal_before = Some(head_before);
        trace.head_abnormal_after = Some(head_after);
        trace.stem_abnormal_before = Some(stem_before);
        trace.stem_abnormal_after = Some(false);
        appended_ids.push(next_graph_identity);
        traces.push(trace);
    }

    let remainder_less_than = last_index < max_index;
    operations.push(NativeStemsBeamVLinkHeadLinksOperation::RemainderCompared {
        last_index,
        max_index,
        less_than: remainder_less_than,
        split_mutation_count: 0,
    });
    operations.push(NativeStemsBeamVLinkHeadLinksOperation::VLinkerReturnedTrue);

    Ok(PreparedHeadLinks {
        steps: traces,
        operations,
        appended_graph_relation_identities: appended_ids,
        assigned_s_linkers,
        s_linker_write_count: attempt.relations.len(),
        s_linker_value_change_count: s_value_changes,
        consistency_mutation_count: state.appended_relations.len(),
        sig_relation_mutation_count: state.appended_relations.len(),
        head_abnormal_assignment_count: head_abnormal_assignments,
        head_abnormal_mutation_count: head_abnormal_mutations,
        stem_abnormal_assignment_count: stem_abnormal_assignments,
        stem_abnormal_mutation_count: stem_abnormal_mutations,
        dirty_cascade_count: dirty_cascades,
        sheet_edit_mutation_count: sheet_edit_delta(starting_sheet, state.sheet_edit),
        event_count,
        last_index,
        max_index,
        remainder_less_than,
    })
}

fn resolve_plan(
    plan_system: &NativeStemsBeamLinkPlanSystem,
    reference: NativeStemsBeamPlanRef,
) -> Result<&NativeStemsBeamLinkPlanAttempt, NativeStemsBeamVLinkHeadLinksError> {
    let mut plan_ordinal = 0_usize;
    for (builder_ordinal, builder) in plan_system.builders.iter().enumerate() {
        if builder.builder_ordinal != builder_ordinal {
            return Err(NativeStemsBeamVLinkHeadLinksError::Predecessor {
                phase: "plan builder order",
            });
        }
        for attempt in &builder.attempts {
            if plan_ordinal == reference.plan_ordinal {
                if builder_ordinal != reference.builder_ordinal {
                    return Err(NativeStemsBeamVLinkHeadLinksError::Predecessor {
                        phase: "flattened plan ordinal",
                    });
                }
                return Ok(attempt);
            }
            plan_ordinal = plan_ordinal.checked_add(1).ok_or(
                NativeStemsBeamVLinkHeadLinksError::UnsupportedV1 {
                    phase: "plan ordinal overflow",
                },
            )?;
        }
    }
    Err(NativeStemsBeamVLinkHeadLinksError::Predecessor {
        phase: "missing flattened plan",
    })
}

struct LiveStateValidationContext<'a> {
    system_id: usize,
    stem_inter_id: i32,
    stem_vertex: usize,
    vertex_limit: usize,
    inter_index_limit: usize,
    b16_state: &'a NativeStemsBeamVLinkSiblingLinksState,
    reachability_inspection: &'a NativeStemsBeamVInspection,
    reuse_live_state: &'a NativeStemsBeamVLinkReuseLiveState,
}

fn resolve_reachability_inspection(
    reachability: &NativeStemsBeamReachabilitySystem,
    triggering: crate::native_stems_beam_vlinkers::NativeStemsBeamVLinkerRef,
    system_id: usize,
    interline: i32,
) -> Result<&NativeStemsBeamVInspection, NativeStemsBeamVLinkHeadLinksError> {
    if reachability.system_id != system_id || reachability.interline != interline {
        return Err(NativeStemsBeamVLinkHeadLinksError::Predecessor {
            phase: "reachability system/interline join",
        });
    }
    let mut matches = reachability
        .beam_inspections
        .iter()
        .flat_map(|beam| &beam.b_visits)
        .flat_map(|visit| &visit.v_inspections)
        .filter(|inspection| inspection.reference == triggering);
    let inspection = matches
        .next()
        .ok_or(NativeStemsBeamVLinkHeadLinksError::Predecessor {
            phase: "missing reachability V inspection",
        })?;
    if matches.next().is_some() {
        return Err(NativeStemsBeamVLinkHeadLinksError::Predecessor {
            phase: "duplicate reachability V inspection",
        });
    }
    Ok(inspection)
}

fn validate_live_heads_and_cells(
    relations: &[NativeStemsBeamHeadRelation],
    heads: &[NativeStemsBeamHeadLinkHeadState],
    cells: &[NativeStemsBeamHeadSLinkerCell],
    context: LiveStateValidationContext<'_>,
) -> Result<(), NativeStemsBeamVLinkHeadLinksError> {
    let LiveStateValidationContext {
        system_id,
        stem_inter_id,
        stem_vertex,
        vertex_limit,
        inter_index_limit,
        b16_state,
        reachability_inspection,
        reuse_live_state,
    } = context;
    let mut expected_heads = Vec::new();
    let mut expected_cells = Vec::new();
    for relation in relations {
        let head = NativeStemsBeamHeadLinkHeadRef::from_corner(relation.corner);
        if !expected_heads.contains(&head) {
            expected_heads.push(head);
        }
        let cell = NativeStemsBeamHeadSLinkerRef {
            head,
            horizontal: relation.corner.horizontal,
        };
        if !expected_cells.contains(&cell) {
            expected_cells.push(cell);
        }
    }
    if heads.iter().map(|head| head.reference).collect::<Vec<_>>() != expected_heads
        || cells.iter().map(|cell| cell.reference).collect::<Vec<_>>() != expected_cells
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
            phase: "first-seen head/S-linker catalogue",
        });
    }

    let mut inter_ids = BTreeSet::new();
    let mut vertices = BTreeSet::new();
    let mut index_ordinals = BTreeSet::new();
    let mut aliases = BTreeSet::new();
    let transaction_state = &b16_state.base_apply_state_after.transaction_state;
    let allocator_last = transaction_state.glyph_index.persistent_ids.sheet_last_id;
    let stem_index_ordinal = match b16_state.base_apply_state_after.inter_index.stem_lookup {
        crate::native_stems_beam_vlink_base_apply::NativeStemsBeamInterIndexLookup::PresentSameObject {
            index_ordinal,
            inter_id,
            ..
        } if inter_id == stem_inter_id => index_ordinal,
        _ => {
            return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
                phase: "selected stem InterIndex lookup",
            });
        }
    };
    for head in heads {
        let candidate = crate::native_stems_beam_reachability::NativeStemsBeamHeadCandidateRef {
            head: head.reference.reference,
            sig_ordinal: head.reference.sig_ordinal,
            x_ordinal: head.reference.x_ordinal,
        };
        let mut decisions = reachability_inspection
            .head_search
            .decisions
            .iter()
            .filter(|decision| decision.candidate == candidate);
        let decision = decisions
            .next()
            .ok_or(NativeStemsBeamVLinkHeadLinksError::Predecessor {
                phase: "live head missing from reachability decisions",
            })?;
        if decisions.next().is_some()
            || decision.action != NativeStemsBeamHeadDecisionAction::CornersChecked
            || !decision.size_accepted
            || decision_shape_token(decision.shape) != head.shape
            || decision.is_small_head != head.is_small
            || relations
                .iter()
                .filter(|relation| {
                    NativeStemsBeamHeadLinkHeadRef::from_corner(relation.corner) == head.reference
                })
                .any(|relation| {
                    decision.corner_checks.iter().all(|check| {
                        check.reference != relation.corner
                            || !check.accepted
                            || check.action != NativeStemsBeamHeadCornerAction::Accepted
                    })
                })
        {
            return Err(NativeStemsBeamVLinkHeadLinksError::Predecessor {
                phase: "live head shape/corner reachability join",
            });
        }
        let (shape_is_small, shape_is_stem_head) = supported_head_shape_flags(&head.shape).ok_or(
            NativeStemsBeamVLinkHeadLinksError::UnsupportedV1 {
                phase: "unsupported live head shape",
            },
        )?;
        if head.alias != format!("head:{}", head.reference.x_ordinal)
            || head.runtime_class != "org.audiveris.omr.sig.inter.HeadInter"
            || head.inter_id <= 0
            || head.inter_id > allocator_last
            || head.inter_id == stem_inter_id
            || transaction_state
                .glyph_index
                .known_canonical_glyphs
                .iter()
                .any(|glyph| glyph.glyph_id == head.inter_id)
            || transaction_state
                .selected_glyph_bindings
                .iter()
                .any(|glyph| glyph.glyph_id == head.inter_id)
            || transaction_state
                .system_stems
                .known_stems
                .iter()
                .any(|known| {
                    known.glyph_id == head.inter_id || known.inter_id == Some(head.inter_id)
                })
            || head.inter_index_object_matches != 1
            || head.inter_index_id_matches != 1
            || head.sig_object_matches != 1
            || head.sig_system_id != system_id
            || head.sig_vertex_identity == stem_vertex
            || head.sig_vertex_identity >= vertex_limit
            || head.inter_index_ordinal == stem_index_ordinal
            || head.inter_index_ordinal >= inter_index_limit
            || head.removed
            || head.manual
            || head.is_small != shape_is_small
            || head.is_stem_head != shape_is_stem_head
            || head.inter_id == b16_state.group_runtime.inter_id
            || head.sig_vertex_identity == b16_state.group_runtime.sig_vertex_identity
            || b16_state.live_group_members.iter().any(|beam| {
                head.inter_id == beam.runtime.inter_id
                    || Some(head.sig_vertex_identity) == beam.runtime.sig_vertex_identity
                    || head.inter_index_ordinal == beam.inter_index_ordinal
                    || head.alias == beam.alias
            })
            || head.alias == b16_state.stem_alias
            || head.alias == b16_state.group_runtime.alias
            || !inter_ids.insert(head.inter_id)
            || !vertices.insert(head.sig_vertex_identity)
            || !index_ordinals.insert(head.inter_index_ordinal)
            || !aliases.insert(head.alias.as_str())
        {
            return Err(NativeStemsBeamVLinkHeadLinksError::UnsupportedV1 {
                phase: "live head registry/SIG/manual identity",
            });
        }
        let mut centers = relations
            .iter()
            .filter(|relation| {
                NativeStemsBeamHeadLinkHeadRef::from_corner(relation.corner) == head.reference
            })
            .map(|relation| relation.check.head_center);
        if centers.any(|center| center != head.center) {
            return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
                phase: "live head center/plan join",
            });
        }
    }

    for cell in cells {
        let observers = &cell.ordered_observer_corners;
        if !canonical_s_observers(cell)
            || relations.iter().any(|relation| {
                let reference = NativeStemsBeamHeadSLinkerRef {
                    head: NativeStemsBeamHeadLinkHeadRef::from_corner(relation.corner),
                    horizontal: relation.corner.horizontal,
                };
                reference == cell.reference && !observers.contains(&relation.corner)
            })
        {
            return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
                phase: "shared S-linker observer topology/order",
            });
        }
    }
    let entries = match &reuse_live_state.evaluation {
        crate::native_stems_beam_vlink_reuse_check::NativeStemsBeamVLinkReuseLiveEvaluation::Entries(
            entries,
        ) if reuse_live_state.system_id == system_id && entries.len() == relations.len() => entries,
        _ => {
            return Err(NativeStemsBeamVLinkHeadLinksError::Predecessor {
                phase: "boundary-13 S-linker evaluation",
            });
        }
    };
    for (map_ordinal, (relation, entry)) in relations.iter().zip(entries).enumerate() {
        if entry.map_ordinal != map_ordinal || entry.corner != relation.corner {
            return Err(NativeStemsBeamVLinkHeadLinksError::Predecessor {
                phase: "boundary-13 S-linker map join",
            });
        }
        if let crate::native_stems_beam_vlink_reuse_check::NativeStemsBeamReuseEntryObservation::Examined {
            s_linker_linked,
            ..
        } = &entry.observation
        {
            let reference = NativeStemsBeamHeadSLinkerRef {
                head: NativeStemsBeamHeadLinkHeadRef::from_corner(relation.corner),
                horizontal: relation.corner.horizontal,
            };
            if cells
                .iter()
                .find(|cell| cell.reference == reference)
                .is_none_or(|cell| cell.linked != *s_linker_linked)
            {
                return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
                    phase: "boundary-13/B17 shared S-linker flag join",
                });
            }
        }
    }
    Ok(())
}

fn decision_shape_token(shape: HeadTemplateShape) -> &'static str {
    match shape {
        HeadTemplateShape::NoteheadBlack => "NOTEHEAD_BLACK",
        HeadTemplateShape::NoteheadVoid => "NOTEHEAD_VOID",
        HeadTemplateShape::WholeNote => "WHOLE_NOTE",
        HeadTemplateShape::Breve => "BREVE",
    }
}

fn canonical_s_observers(cell: &NativeStemsBeamHeadSLinkerCell) -> bool {
    let observers = &cell.ordered_observer_corners;
    observers.len() == 2
        && observers[0].vertical == NativeStemVerticalSide::Top
        && observers[1].vertical == NativeStemVerticalSide::Bottom
        && observers.iter().all(|corner| {
            NativeStemsBeamHeadLinkHeadRef::from_corner(*corner) == cell.reference.head
                && corner.horizontal == cell.reference.horizontal
        })
}

fn supported_head_shape_flags(shape: &str) -> Option<(bool, bool)> {
    let small = matches!(
        shape,
        "NOTEHEAD_BLACK_SMALL" | "NOTEHEAD_VOID_SMALL" | "WHOLE_NOTE_SMALL" | "BREVE_SMALL"
    );
    let stem = matches!(
        shape,
        "NOTEHEAD_BLACK"
            | "NOTEHEAD_VOID"
            | "NOTEHEAD_BLACK_SMALL"
            | "NOTEHEAD_VOID_SMALL"
            | "NOTEHEAD_CROSS"
            | "NOTEHEAD_CROSS_VOID"
            | "NOTEHEAD_DIAMOND_FILLED"
            | "NOTEHEAD_DIAMOND_VOID"
            | "NOTEHEAD_TRIANGLE_DOWN_FILLED"
            | "NOTEHEAD_TRIANGLE_DOWN_VOID"
            | "NOTEHEAD_CIRCLE_X"
            | "NOTEHEAD_CIRCLE_X_VOID"
    );
    let recognized = stem
        || matches!(
            shape,
            "WHOLE_NOTE"
                | "BREVE"
                | "WHOLE_NOTE_SMALL"
                | "BREVE_SMALL"
                | "WHOLE_NOTE_CROSS"
                | "BREVE_CROSS"
                | "WHOLE_NOTE_DIAMOND"
                | "BREVE_DIAMOND"
                | "WHOLE_NOTE_TRIANGLE_DOWN"
                | "BREVE_TRIANGLE_DOWN"
                | "WHOLE_NOTE_CIRCLE_X"
                | "BREVE_CIRCLE_X"
        );
    recognized.then_some((small, stem))
}

fn validate_step_headers(
    plan_ref: NativeStemsBeamPlanRef,
    relations: &[NativeStemsBeamHeadRelation],
    certificate: &NativeStemsBeamVLinkHeadLinksCertificate,
) -> Result<(), NativeStemsBeamVLinkHeadLinksError> {
    for (map_ordinal, (relation, step)) in relations.iter().zip(&certificate.steps).enumerate() {
        let expected_object = NativeStemsBeamHeadRelationObjectIdentity::HeadDraft {
            plan_ordinal: plan_ref.plan_ordinal,
            map_ordinal,
        };
        if relation.map_ordinal != map_ordinal
            || step.map_ordinal != map_ordinal
            || step.corner != relation.corner
            || step.s_linker
                != (NativeStemsBeamHeadSLinkerRef {
                    head: NativeStemsBeamHeadLinkHeadRef::from_corner(relation.corner),
                    horizontal: relation.corner.horizontal,
                })
            || step.plan_draft.relation_object_identity != expected_object
            || step.plan_draft.relation_class != HEAD_STEM_CLASS
            || step.plan_draft.manual
            || step.plan_draft.head_side_before != Some(relation.check.derived_horizontal)
            || !option_point_bits_equal(
                step.plan_draft.extension_point_before,
                relation.check.extension_point,
            )
            || step.plan_draft.consistency_before.is_some()
            || step.plan_draft.graph_matches_before != 0
            || !head_relation_exact(&step.plan_draft.relation, relation)
            || !valid_plan_relation(relation)
            || relation.check.derived_horizontal
                != step.plan_draft.relation.check.derived_horizontal
        {
            return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                phase: "ordered relation-map plan draft",
            });
        }
    }
    Ok(())
}

fn valid_plan_relation(relation: &NativeStemsBeamHeadRelation) -> bool {
    let check = &relation.check;
    let finite = [
        check.reference_point.x,
        check.reference_point.y,
        check.x_stem,
        check.x_gap_pixels,
        check.y_gap_pixels,
        check.dx,
        check.dy,
        check.x_maximum,
        check.y_maximum,
        check.x_weight,
        check.y_weight,
        check.raw_x_impact,
        check.raw_y_impact,
        check.x_impact,
        check.y_impact,
        check.grade,
    ]
    .into_iter()
    .all(f64::is_finite);
    finite
        && check.encountered_corner == relation.corner
        && check.encountered_horizontal == relation.corner.horizontal
        && check.vertical == relation.corner.vertical
        && check.horizontal_side_diverges
            == (check.derived_horizontal != check.encountered_horizontal)
        && check.x_maximum > 0.0
        && check.y_maximum > 0.0
        && check.x_weight > 0.0
        && check.y_weight > 0.0
        && (0.0..=1.0).contains(&check.x_impact)
        && (0.0..=1.0).contains(&check.y_impact)
        && (0.1..=1.0).contains(&check.grade)
        && check.accepted
        && check
            .extension_point
            .is_some_and(|point| point.x.is_finite() && point.y.is_finite())
}

fn head_relation_exact(
    left: &NativeStemsBeamHeadRelation,
    right: &NativeStemsBeamHeadRelation,
) -> bool {
    left == right
        && point_bits_equal(left.check.reference_point, right.check.reference_point)
        && left.check.x_stem.to_bits() == right.check.x_stem.to_bits()
        && left.check.x_gap_pixels.to_bits() == right.check.x_gap_pixels.to_bits()
        && left.check.y_gap_pixels.to_bits() == right.check.y_gap_pixels.to_bits()
        && left.check.dx.to_bits() == right.check.dx.to_bits()
        && left.check.dy.to_bits() == right.check.dy.to_bits()
        && left.check.x_maximum.to_bits() == right.check.x_maximum.to_bits()
        && left.check.y_maximum.to_bits() == right.check.y_maximum.to_bits()
        && left.check.x_weight.to_bits() == right.check.x_weight.to_bits()
        && left.check.y_weight.to_bits() == right.check.y_weight.to_bits()
        && left.check.raw_x_impact.to_bits() == right.check.raw_x_impact.to_bits()
        && left.check.raw_y_impact.to_bits() == right.check.raw_y_impact.to_bits()
        && left.check.x_impact.to_bits() == right.check.x_impact.to_bits()
        && left.check.y_impact.to_bits() == right.check.y_impact.to_bits()
        && left.check.grade.to_bits() == right.check.grade.to_bits()
        && option_point_bits_equal(left.check.extension_point, right.check.extension_point)
}

fn point_bits_equal(left: NativeStemPoint, right: NativeStemPoint) -> bool {
    left.x.to_bits() == right.x.to_bits() && left.y.to_bits() == right.y.to_bits()
}

fn option_point_bits_equal(left: Option<NativeStemPoint>, right: Option<NativeStemPoint>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => point_bits_equal(left, right),
        (None, None) => true,
        _ => false,
    }
}

fn head_stem_consistency(
    is_small: bool,
    y1: f64,
    y2: f64,
    interline: i32,
) -> Result<f64, NativeStemsBeamVLinkHeadLinksError> {
    if !y1.is_finite() || !y2.is_finite() || interline <= 0 {
        return Err(NativeStemsBeamVLinkHeadLinksError::UnsupportedV1 {
            phase: "HeadStem consistency geometry",
        });
    }
    let scaled_length = (y2 - y1) / f64::from(interline);
    let ratio = scaled_length / NEUTRAL_STEM_LENGTH;
    let consistency = if is_small { 1.0 / ratio } else { ratio };
    if !consistency.is_finite() {
        return Err(NativeStemsBeamVLinkHeadLinksError::UnsupportedV1 {
            phase: "non-finite HeadStem consistency",
        });
    }
    Ok(consistency)
}

fn validate_existing_branch_unread(
    step: &NativeStemsBeamHeadLinkStepCertificate,
) -> Result<(), NativeStemsBeamVLinkHeadLinksError> {
    if step.expected_consistency_after.is_some()
        || step.consistency_debug_enabled.is_some()
        || step.add_edge_returned.is_some()
        || step.head_incident_after.is_some()
        || step.stem_incident_after.is_some()
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
            phase: "existing HeadStem branch read forbidden fields",
        });
    }
    Ok(())
}

fn validate_pair_draft_visibility(
    scan: &NativeStemsBeamHeadDirectedPairScan,
    appended: &[NativeStemsBeamHeadAppendedRelation],
) -> Result<(), NativeStemsBeamVLinkHeadLinksError> {
    let rows = scan
        .source_outgoing_relations
        .iter()
        .map(|row| (row.graph_relation_identity, row.relation_object_identity))
        .chain(
            scan.relations
                .iter()
                .map(|row| (row.graph_relation_identity, row.relation_object_identity)),
        );
    if rows.into_iter().any(|(graph_identity, object)| {
        matches!(
            object,
            NativeStemsBeamHeadRelationObjectIdentity::HeadDraft { .. }
        ) && !appended.iter().any(|relation| {
            relation.graph_relation_identity == graph_identity
                && relation.relation_object_identity == object
        })
    }) {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
            phase: "unattached head draft visible before insertion",
        });
    }
    Ok(())
}

fn validate_incident_draft_visibility(
    scan: &NativeStemsBeamHeadIncidentScan,
    appended: &[NativeStemsBeamHeadAppendedRelation],
) -> Result<(), NativeStemsBeamVLinkHeadLinksError> {
    if scan.relations.iter().any(|row| {
        matches!(
            row.relation_object_identity,
            NativeStemsBeamHeadRelationObjectIdentity::HeadDraft { .. }
        ) && !appended.iter().any(|relation| {
            relation.graph_relation_identity == row.graph_relation_identity
                && relation.relation_object_identity == row.relation_object_identity
        })
    }) {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
            phase: "unattached head draft visible in callback",
        });
    }
    Ok(())
}

struct PairReadResult {
    relations_read: usize,
    first_match: Option<usize>,
}

#[derive(Clone)]
struct RelationPayload {
    object: NativeStemsBeamHeadRelationObjectIdentity,
    class: String,
    kind: Option<NativeStemsBeamHeadQueryRelationKind>,
}

#[derive(Clone, Copy)]
struct PairValidationContext {
    graph_limit: usize,
    source_head_vertex: usize,
    target_stem_vertex: usize,
    vertex_limit: usize,
}

struct RelationValidationMaps<'a> {
    objects: &'a mut BTreeMap<usize, NativeStemsBeamHeadRelationObjectIdentity>,
    payloads: &'a mut BTreeMap<usize, RelationPayload>,
    endpoints: &'a mut BTreeMap<usize, (usize, usize)>,
    sources: &'a mut BTreeMap<usize, usize>,
}

fn validate_pair_scan(
    scan: &NativeStemsBeamHeadDirectedPairScan,
    context: PairValidationContext,
    maps: RelationValidationMaps<'_>,
) -> Result<PairReadResult, NativeStemsBeamVLinkHeadLinksError> {
    let PairValidationContext {
        graph_limit,
        source_head_vertex,
        target_stem_vertex,
        vertex_limit,
    } = context;
    let RelationValidationMaps {
        objects: relation_objects,
        payloads: relation_payloads,
        endpoints: relation_endpoints,
        sources: relation_sources,
    } = maps;
    if scan.source_outgoing_scanned != scan.source_outgoing_relations.len()
        || scan.query_relation_count != scan.relations.len()
        || scan.source_outgoing_provenance_sha256
            != source_outgoing_query_sha256(&scan.source_outgoing_relations)
        || scan.pair_provenance_sha256 != pair_query_sha256(&scan.relations)
        || !valid_sha256(&scan.source_outgoing_provenance_sha256)
        || !valid_sha256(&scan.pair_provenance_sha256)
        || scan.source_outgoing_scanned < scan.query_relation_count
        || scan
            .source_outgoing_relations
            .iter()
            .enumerate()
            .any(|(ordinal, row)| {
                row.source_outgoing_ordinal != ordinal
                    || row.graph_relation_identity >= graph_limit
                    || row.relation_class.is_empty()
                    || row.target_vertex_identity >= vertex_limit
            })
        || scan
            .source_outgoing_relations
            .windows(2)
            .any(|rows| rows[0].graph_relation_identity >= rows[1].graph_relation_identity)
        || scan.relations.iter().enumerate().any(|(ordinal, row)| {
            row.pair_ordinal != ordinal
                || row.graph_relation_identity >= graph_limit
                || row.relation_class.is_empty()
        })
        || scan
            .relations
            .windows(2)
            .any(|rows| rows[0].graph_relation_identity >= rows[1].graph_relation_identity)
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
            phase: "directed pair shape/provenance/order",
        });
    }

    let expected_pair = scan
        .source_outgoing_relations
        .iter()
        .filter(|row| row.target_vertex_identity == target_stem_vertex)
        .collect::<Vec<_>>();
    let known_pair_identities = relation_endpoints
        .iter()
        .filter_map(|(identity, endpoints)| {
            (*endpoints == (source_head_vertex, target_stem_vertex)).then_some(*identity)
        })
        .collect::<Vec<_>>();
    let actual_pair_identities = expected_pair
        .iter()
        .map(|row| row.graph_relation_identity)
        .collect::<Vec<_>>();
    if expected_pair.len() != scan.relations.len()
        || actual_pair_identities != known_pair_identities
        || scan
            .relations
            .iter()
            .zip(expected_pair)
            .any(|(pair, outgoing)| {
                pair.source_outgoing_ordinal != outgoing.source_outgoing_ordinal
                    || pair.graph_relation_identity != outgoing.graph_relation_identity
                    || pair.relation_object_identity != outgoing.relation_object_identity
                    || pair.relation_class != outgoing.relation_class
            })
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
            phase: "directed pair exact live target filter",
        });
    }
    if relation_endpoints.iter().any(|(identity, endpoints)| {
        *endpoints == (source_head_vertex, target_stem_vertex)
            && relation_payloads.get(identity).is_some_and(|payload| {
                !scan.source_outgoing_relations.iter().any(|row| {
                    row.graph_relation_identity == *identity
                        && row.relation_object_identity == payload.object
                        && row.relation_class == payload.class
                        && row.target_vertex_identity == target_stem_vertex
                }) || !scan.relations.iter().any(|row| {
                    row.graph_relation_identity == *identity
                        && row.relation_object_identity == payload.object
                        && row.relation_class == payload.class
                })
            })
    }) {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
            phase: "known relation omitted from directed pair",
        });
    }

    for row in &scan.source_outgoing_relations {
        join_relation_source(
            relation_sources,
            row.graph_relation_identity,
            source_head_vertex,
        )?;
        join_relation_endpoints(
            relation_endpoints,
            row.graph_relation_identity,
            source_head_vertex,
            row.target_vertex_identity,
        )?;
        join_relation_object(
            relation_objects,
            row.graph_relation_identity,
            row.relation_object_identity,
        )?;
        join_relation_payload(
            relation_payloads,
            row.graph_relation_identity,
            row.relation_object_identity,
            &row.relation_class,
            None,
        )?;
    }

    let mut first_match = None;
    for row in &scan.relations {
        let outgoing = scan
            .source_outgoing_relations
            .get(row.source_outgoing_ordinal)
            .ok_or(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                phase: "pair source-outgoing ordinal",
            })?;
        if outgoing.graph_relation_identity != row.graph_relation_identity
            || outgoing.relation_object_identity != row.relation_object_identity
            || outgoing.relation_class != row.relation_class
        {
            return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                phase: "pair/source-outgoing exact join",
            });
        }
        if (row.kind == NativeStemsBeamHeadQueryRelationKind::HeadStem)
            != (row.relation_class == HEAD_STEM_CLASS)
        {
            return Err(NativeStemsBeamVLinkHeadLinksError::UnsupportedV1 {
                phase: "directed pair runtime relation class",
            });
        }
        let expected_read = if first_match.is_some() {
            NativeStemsBeamHeadPairClassRead::UnreadAfterBreak
        } else if row.kind == NativeStemsBeamHeadQueryRelationKind::HeadStem {
            first_match = Some(row.pair_ordinal);
            NativeStemsBeamHeadPairClassRead::ExaminedMatchBreak
        } else {
            NativeStemsBeamHeadPairClassRead::ExaminedContinue
        };
        if row.class_read != expected_read {
            return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                phase: "directed first-isInstance class reads",
            });
        }
        join_relation_object(
            relation_objects,
            row.graph_relation_identity,
            row.relation_object_identity,
        )?;
        join_relation_payload(
            relation_payloads,
            row.graph_relation_identity,
            row.relation_object_identity,
            &row.relation_class,
            Some(row.kind),
        )?;
        join_relation_endpoints(
            relation_endpoints,
            row.graph_relation_identity,
            source_head_vertex,
            target_stem_vertex,
        )?;
    }
    Ok(PairReadResult {
        relations_read: first_match.map_or(scan.relations.len(), |index| index + 1),
        first_match,
    })
}

fn validate_source_scan_evolution(
    head: NativeStemsBeamHeadLinkHeadRef,
    head_vertex: usize,
    current: &[NativeStemsBeamHeadSourceOutgoingRelation],
    pair: &[NativeStemsBeamHeadPairRelation],
    prior_scans: &BTreeMap<
        NativeStemsBeamHeadLinkHeadRef,
        Vec<NativeStemsBeamHeadSourceOutgoingRelation>,
    >,
    appended: &[NativeStemsBeamHeadAppendedRelation],
    relation_endpoints: &BTreeMap<usize, (usize, usize)>,
) -> Result<(), NativeStemsBeamVLinkHeadLinksError> {
    if current.iter().any(|row| {
        relation_endpoints
            .get(&row.graph_relation_identity)
            .is_some_and(|(source, _)| *source != head_vertex)
    }) {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
            phase: "source-outgoing relation endpoint mismatch",
        });
    }
    let Some(prior) = prior_scans.get(&head) else {
        if appended.iter().any(|relation| relation.source_head == head) {
            return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                phase: "first head query omits prior local append",
            });
        }
        return Ok(());
    };
    if current.len() < prior.len()
        || current[..prior.len()] != *prior
        || current[prior.len()..]
            .iter()
            .zip(appended.iter().filter(|relation| {
                relation.source_head == head
                    && !prior
                        .iter()
                        .any(|row| row.graph_relation_identity == relation.graph_relation_identity)
            }))
            .any(|(row, relation)| {
                row.graph_relation_identity != relation.graph_relation_identity
                    || row.relation_object_identity != relation.relation_object_identity
                    || row.relation_class != HEAD_STEM_CLASS
            })
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
            phase: "serial source-outgoing scan evolution",
        });
    }
    let expected_new = appended
        .iter()
        .filter(|relation| {
            relation.source_head == head
                && !prior
                    .iter()
                    .any(|row| row.graph_relation_identity == relation.graph_relation_identity)
        })
        .count();
    if current.len() != prior.len() + expected_new {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
            phase: "serial source-outgoing append cardinality",
        });
    }
    // Every boundary-17 edge appended from this head targets the one selected
    // stem. Java's next getAllEdges(head, stem) query must therefore return it;
    // merely retaining it in the broader head-outgoing scan is not enough.
    if appended
        .iter()
        .filter(|relation| relation.source_head == head)
        .any(|relation| {
            pair.iter().all(|row| {
                row.graph_relation_identity != relation.graph_relation_identity
                    || row.relation_object_identity != relation.relation_object_identity
                    || row.relation_class != HEAD_STEM_CLASS
                    || row.kind != NativeStemsBeamHeadQueryRelationKind::HeadStem
            })
        })
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
            phase: "serial local HeadStem omitted from directed pair",
        });
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EndpointEvidence {
    inter_id: i32,
    alias: String,
    role: EndpointRole,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum EndpointRole {
    Head(NativeStemsBeamHeadLinkHeadRef),
    Stem,
    OtherInter,
}

fn initial_endpoint_map(
    heads: &[NativeStemsBeamHeadLinkHeadState],
    stem_inter_id: i32,
    stem_vertex: usize,
) -> Result<BTreeMap<usize, EndpointEvidence>, NativeStemsBeamVLinkHeadLinksError> {
    let mut result = BTreeMap::new();
    result.insert(
        stem_vertex,
        EndpointEvidence {
            inter_id: stem_inter_id,
            alias: "__selected_stem__".to_owned(),
            role: EndpointRole::Stem,
        },
    );
    for head in heads {
        if result
            .insert(
                head.sig_vertex_identity,
                EndpointEvidence {
                    inter_id: head.inter_id,
                    alias: head.alias.clone(),
                    role: EndpointRole::Head(head.reference),
                },
            )
            .is_some()
        {
            return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
                phase: "represented endpoint vertex collision",
            });
        }
    }
    Ok(result)
}

#[derive(Clone, Copy)]
enum IncidentQueried {
    Head(NativeStemsBeamHeadLinkHeadRef),
    Stem,
}

struct IncidentValidationContext<'a> {
    queried: IncidentQueried,
    current_graph_identity: usize,
    current_object_identity: NativeStemsBeamHeadRelationObjectIdentity,
    current_head: NativeStemsBeamHeadLinkHeadRef,
    current_head_alias: &'a str,
    stem_alias: &'a str,
    stem_inter_id: i32,
    stem_vertex: usize,
    vertex_limit: usize,
    live_heads: &'a [NativeStemsBeamHeadLinkHeadState],
}

#[allow(clippy::too_many_arguments)]
fn validate_incident_scan(
    scan: &NativeStemsBeamHeadIncidentScan,
    context: IncidentValidationContext<'_>,
    relation_objects: &mut BTreeMap<usize, NativeStemsBeamHeadRelationObjectIdentity>,
    relation_payloads: &mut BTreeMap<usize, RelationPayload>,
    relation_endpoints: &mut BTreeMap<usize, (usize, usize)>,
    relation_sources: &mut BTreeMap<usize, usize>,
    endpoint_by_vertex: &mut BTreeMap<usize, EndpointEvidence>,
    vertex_by_inter_id: &mut BTreeMap<i32, usize>,
) -> Result<usize, NativeStemsBeamVLinkHeadLinksError> {
    if scan.query_relation_count != scan.relations.len()
        || !valid_sha256(&scan.query_provenance_sha256)
        || scan.query_provenance_sha256 != incident_query_sha256(&scan.relations)
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
            phase: "incident query count/provenance",
        });
    }
    validate_incident_order(&scan.relations)?;

    let mut first_match = None;
    let mut current_matches = 0;
    let mut seen_graph_identities = BTreeSet::new();
    for row in &scan.relations {
        if row.graph_relation_identity > context.current_graph_identity
            || row.relation_class.is_empty()
            || row.opposite_alias.is_empty()
            || row.opposite_inter_id <= 0
            || row.opposite_vertex_identity >= context.vertex_limit
            || !seen_graph_identities.insert(row.graph_relation_identity)
        {
            return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                phase: "incident relation/endpoint domain",
            });
        }
        if (row.kind == NativeStemsBeamHeadQueryRelationKind::HeadStem)
            != (row.relation_class == HEAD_STEM_CLASS)
        {
            return Err(NativeStemsBeamVLinkHeadLinksError::UnsupportedV1 {
                phase: "incident runtime relation class",
            });
        }
        let expected_read = if first_match.is_some() {
            NativeStemsBeamHeadIncidentClassRead::UnreadAfterBreak
        } else if row.kind == NativeStemsBeamHeadQueryRelationKind::HeadStem {
            first_match = Some(row.incident_ordinal);
            NativeStemsBeamHeadIncidentClassRead::ExaminedMatchBreak
        } else {
            NativeStemsBeamHeadIncidentClassRead::ExaminedContinue
        };
        if row.class_read != expected_read {
            return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                phase: "incident first-isInstance class reads",
            });
        }
        validate_incident_opposite(row, &context, endpoint_by_vertex, vertex_by_inter_id)?;
        let queried_vertex = match context.queried {
            IncidentQueried::Head(reference) => context
                .live_heads
                .iter()
                .find(|head| head.reference == reference)
                .map(|head| head.sig_vertex_identity)
                .ok_or(NativeStemsBeamVLinkHeadLinksError::InvalidState {
                    phase: "queried incident head vertex",
                })?,
            IncidentQueried::Stem => context.stem_vertex,
        };
        let (source_vertex, target_vertex) = match row.direction {
            NativeStemsBeamIncidentDirection::Incoming => {
                (row.opposite_vertex_identity, queried_vertex)
            }
            NativeStemsBeamIncidentDirection::Outgoing => {
                (queried_vertex, row.opposite_vertex_identity)
            }
        };
        join_relation_endpoints(
            relation_endpoints,
            row.graph_relation_identity,
            source_vertex,
            target_vertex,
        )?;
        join_relation_source(relation_sources, row.graph_relation_identity, source_vertex)?;
        join_relation_object(
            relation_objects,
            row.graph_relation_identity,
            row.relation_object_identity,
        )?;
        join_relation_payload(
            relation_payloads,
            row.graph_relation_identity,
            row.relation_object_identity,
            &row.relation_class,
            Some(row.kind),
        )?;

        if row.graph_relation_identity == context.current_graph_identity {
            current_matches += 1;
            let correct = match context.queried {
                IncidentQueried::Head(head) => {
                    row.direction == NativeStemsBeamIncidentDirection::Outgoing
                        && head == context.current_head
                        && row.opposite == NativeStemsBeamHeadIncidentOpposite::Stem
                        && row.opposite_alias == context.stem_alias
                        && row.opposite_inter_id == context.stem_inter_id
                        && row.opposite_vertex_identity == context.stem_vertex
                }
                IncidentQueried::Stem => {
                    row.direction == NativeStemsBeamIncidentDirection::Incoming
                        && row.opposite
                            == NativeStemsBeamHeadIncidentOpposite::Head(context.current_head)
                        && row.opposite_alias == context.current_head_alias
                        && context
                            .live_heads
                            .iter()
                            .find(|head| head.reference == context.current_head)
                            .is_some_and(|head| {
                                row.opposite_inter_id == head.inter_id
                                    && row.opposite_vertex_identity == head.sig_vertex_identity
                            })
                }
            };
            if !correct
                || row.relation_object_identity != context.current_object_identity
                || row.kind != NativeStemsBeamHeadQueryRelationKind::HeadStem
                || row.relation_class != HEAD_STEM_CLASS
            {
                return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                    phase: "fresh HeadStem callback edge",
                });
            }
        }
    }
    if current_matches != 1 || first_match.is_none() {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
            phase: "callback HeadStem visibility/cardinality",
        });
    }
    Ok(first_match.expect("checked above") + 1)
}

fn validate_incident_opposite(
    row: &NativeStemsBeamHeadIncidentRelation,
    context: &IncidentValidationContext<'_>,
    endpoint_by_vertex: &mut BTreeMap<usize, EndpointEvidence>,
    vertex_by_inter_id: &mut BTreeMap<i32, usize>,
) -> Result<(), NativeStemsBeamVLinkHeadLinksError> {
    let evidence = match row.opposite {
        NativeStemsBeamHeadIncidentOpposite::Stem => {
            if row.opposite_alias != context.stem_alias
                || row.opposite_inter_id != context.stem_inter_id
                || row.opposite_vertex_identity != context.stem_vertex
                || matches!(context.queried, IncidentQueried::Stem)
            {
                return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                    phase: "incident stem role/identity",
                });
            }
            EndpointEvidence {
                inter_id: row.opposite_inter_id,
                alias: "__selected_stem__".to_owned(),
                role: EndpointRole::Stem,
            }
        }
        NativeStemsBeamHeadIncidentOpposite::Head(reference) => {
            let head = context
                .live_heads
                .iter()
                .find(|head| head.reference == reference)
                .ok_or(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                    phase: "incident unknown head role",
                })?;
            if row.opposite_alias != head.alias
                || row.opposite_inter_id != head.inter_id
                || row.opposite_vertex_identity != head.sig_vertex_identity
                || matches!(context.queried, IncidentQueried::Head(queried) if queried == reference)
            {
                return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                    phase: "incident head role/identity",
                });
            }
            EndpointEvidence {
                inter_id: row.opposite_inter_id,
                alias: row.opposite_alias.clone(),
                role: EndpointRole::Head(reference),
            }
        }
        NativeStemsBeamHeadIncidentOpposite::OtherInter => {
            if row.opposite_inter_id == context.stem_inter_id
                || row.opposite_vertex_identity == context.stem_vertex
                || context.live_heads.iter().any(|head| {
                    head.inter_id == row.opposite_inter_id
                        || head.sig_vertex_identity == row.opposite_vertex_identity
                })
            {
                return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                    phase: "incident OtherInter endpoint collision",
                });
            }
            EndpointEvidence {
                inter_id: row.opposite_inter_id,
                alias: row.opposite_alias.clone(),
                role: EndpointRole::OtherInter,
            }
        }
    };
    if endpoint_by_vertex
        .get(&row.opposite_vertex_identity)
        .is_some_and(|prior| prior != &evidence)
        || vertex_by_inter_id
            .get(&row.opposite_inter_id)
            .is_some_and(|prior| *prior != row.opposite_vertex_identity)
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
            phase: "incident endpoint vertex/Inter identity consistency",
        });
    }
    endpoint_by_vertex.insert(row.opposite_vertex_identity, evidence);
    vertex_by_inter_id.insert(row.opposite_inter_id, row.opposite_vertex_identity);
    Ok(())
}

fn validate_incident_order(
    rows: &[NativeStemsBeamHeadIncidentRelation],
) -> Result<(), NativeStemsBeamVLinkHeadLinksError> {
    let mut outgoing = false;
    let mut incoming_ordinal = 0;
    let mut outgoing_ordinal = 0;
    let mut last_incoming = None;
    let mut last_outgoing = None;
    for (incident_ordinal, row) in rows.iter().enumerate() {
        if row.incident_ordinal != incident_ordinal {
            return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                phase: "incident ordinal",
            });
        }
        match row.direction {
            NativeStemsBeamIncidentDirection::Incoming if outgoing => {
                return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                    phase: "incident incoming-after-outgoing order",
                });
            }
            NativeStemsBeamIncidentDirection::Incoming => {
                if row.direction_ordinal != incoming_ordinal
                    || last_incoming.is_some_and(|prior| prior >= row.graph_relation_identity)
                {
                    return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                        phase: "incoming insertion chronology",
                    });
                }
                incoming_ordinal += 1;
                last_incoming = Some(row.graph_relation_identity);
            }
            NativeStemsBeamIncidentDirection::Outgoing => {
                outgoing = true;
                if row.direction_ordinal != outgoing_ordinal
                    || last_outgoing.is_some_and(|prior| prior >= row.graph_relation_identity)
                {
                    return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                        phase: "outgoing insertion chronology",
                    });
                }
                outgoing_ordinal += 1;
                last_outgoing = Some(row.graph_relation_identity);
            }
        }
    }
    Ok(())
}

fn validate_head_callback_outgoing_partition(
    source_before: &[NativeStemsBeamHeadSourceOutgoingRelation],
    incident_after: &NativeStemsBeamHeadIncidentScan,
    fresh_graph_identity: usize,
    fresh_object_identity: NativeStemsBeamHeadRelationObjectIdentity,
    stem_vertex: usize,
) -> Result<(), NativeStemsBeamVLinkHeadLinksError> {
    let outgoing = incident_after
        .relations
        .iter()
        .filter(|row| row.direction == NativeStemsBeamIncidentDirection::Outgoing)
        .collect::<Vec<_>>();
    if outgoing.len() != source_before.len() + 1
        || source_before
            .iter()
            .zip(&outgoing)
            .any(|(source, incident)| {
                incident.direction_ordinal != source.source_outgoing_ordinal
                    || incident.graph_relation_identity != source.graph_relation_identity
                    || incident.relation_object_identity != source.relation_object_identity
                    || incident.relation_class != source.relation_class
                    || incident.opposite_vertex_identity != source.target_vertex_identity
            })
        || outgoing.last().is_none_or(|fresh| {
            fresh.direction_ordinal != source_before.len()
                || fresh.graph_relation_identity != fresh_graph_identity
                || fresh.relation_object_identity != fresh_object_identity
                || fresh.relation_class != HEAD_STEM_CLASS
                || fresh.opposite != NativeStemsBeamHeadIncidentOpposite::Stem
                || fresh.opposite_vertex_identity != stem_vertex
        })
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
            phase: "head callback incident/source-outgoing exact evolution",
        });
    }
    Ok(())
}

fn validate_stem_scan_evolution(
    prior: Option<&NativeStemsBeamHeadIncidentScan>,
    current: &NativeStemsBeamHeadIncidentScan,
    current_graph_identity: usize,
    predecessor: &[PredecessorStemIncidentRow],
) -> Result<(), NativeStemsBeamVLinkHeadLinksError> {
    if let Some(prior) = prior {
        if current.relations.len() != prior.relations.len() + 1
            || prior.relations.iter().any(|old| {
                current
                    .relations
                    .iter()
                    .find(|row| row.graph_relation_identity == old.graph_relation_identity)
                    .is_none_or(|row| !incident_payload_equal(old, row))
            })
            || current
                .relations
                .iter()
                .filter(|row| row.graph_relation_identity == current_graph_identity)
                .count()
                != 1
        {
            return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                phase: "serial stem incident append evolution",
            });
        }
    } else if current.relations.len() != predecessor.len() + 1
        || predecessor.iter().any(|old| {
            current
                .relations
                .iter()
                .find(|row| row.graph_relation_identity == old.graph_relation_identity)
                .is_none_or(|row| !old.matches(row))
        })
        || current
            .relations
            .iter()
            .filter(|row| row.graph_relation_identity == current_graph_identity)
            .count()
            != 1
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
            phase: "first stem scan does not extend exact boundary-16 snapshot",
        });
    }
    Ok(())
}

#[derive(Clone)]
struct PredecessorStemIncidentRow {
    direction: NativeStemsBeamIncidentDirection,
    direction_ordinal: usize,
    graph_relation_identity: usize,
    relation_object_identity: NativeStemsBeamHeadRelationObjectIdentity,
    relation_class: String,
    opposite_inter_id: i32,
    opposite_vertex_identity: usize,
    opposite: NativeStemsBeamHeadIncidentOpposite,
    opposite_alias: Option<String>,
}

impl PredecessorStemIncidentRow {
    fn matches(&self, row: &NativeStemsBeamHeadIncidentRelation) -> bool {
        self.direction == row.direction
            && self.direction_ordinal == row.direction_ordinal
            && self.graph_relation_identity == row.graph_relation_identity
            && self.relation_object_identity == row.relation_object_identity
            && self.relation_class == row.relation_class
            && self.opposite_inter_id == row.opposite_inter_id
            && self.opposite_vertex_identity == row.opposite_vertex_identity
            && self.opposite == row.opposite
            && self
                .opposite_alias
                .as_ref()
                .is_none_or(|alias| alias == &row.opposite_alias)
    }
}

fn predecessor_stem_incident_rows(
    predecessor: &NativeStemsBeamVLinkSiblingLinksTransaction,
    base_apply_transaction: &NativeStemsBeamVLinkBaseApplyTransaction,
    live_heads: &[NativeStemsBeamHeadLinkHeadState],
) -> Result<Vec<PredecessorStemIncidentRow>, NativeStemsBeamVLinkHeadLinksError> {
    if let Some(scan) = predecessor
        .consumed_certificate
        .steps
        .iter()
        .rev()
        .find_map(|step| step.stem_incident_after.as_ref())
    {
        return scan
            .relations
            .iter()
            .map(|row| {
                if row.opposite
                    == crate::native_stems_beam_vlink_base_apply::NativeStemsBeamIncidentOpposite::Stem
                {
                    return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
                        phase: "boundary-16 stem incident self endpoint",
                    });
                }
                let (opposite, opposite_alias) = predecessor_opposite(
                    row.opposite_inter_id,
                    row.opposite_vertex_identity,
                    Some(&row.opposite_alias),
                    live_heads,
                )?;
                Ok(PredecessorStemIncidentRow {
                    direction: row.direction,
                    direction_ordinal: row.direction_ordinal,
                    graph_relation_identity: row.graph_relation_identity,
                    relation_object_identity: sibling_relation_object(row.relation_object_identity),
                    relation_class: row.relation_class.clone(),
                    opposite_inter_id: row.opposite_inter_id,
                    opposite_vertex_identity: row.opposite_vertex_identity,
                    opposite,
                    opposite_alias,
                })
            })
            .collect();
    }

    // When no sibling edge was added, boundary 14 supplied the final exact
    // stem incident snapshot. The public entry point already included this
    // transaction in the independently replayed boundary-16 chain.
    let base = if base_apply_transaction.callback.called {
        &base_apply_transaction
            .consumed_certificate
            .stem_incident_after
    } else {
        &base_apply_transaction
            .consumed_certificate
            .stem_incident_before
    };
    base.relations
        .iter()
        .map(|row| {
            if row.opposite
                == crate::native_stems_beam_vlink_base_apply::NativeStemsBeamIncidentOpposite::Stem
            {
                return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
                    phase: "boundary-14 stem incident self endpoint",
                });
            }
            let (opposite, opposite_alias) = predecessor_opposite(
                row.opposite_inter_id,
                row.opposite_vertex_ordinal,
                None,
                live_heads,
            )?;
            Ok(PredecessorStemIncidentRow {
                direction: row.direction,
                direction_ordinal: row.direction_ordinal,
                graph_relation_identity: row.graph_relation_identity,
                relation_object_identity: base_relation_object(row.relation_object_identity),
                relation_class: row.relation_class.clone(),
                opposite_inter_id: row.opposite_inter_id,
                opposite_vertex_identity: row.opposite_vertex_ordinal,
                opposite,
                opposite_alias,
            })
        })
        .collect()
}

fn predecessor_opposite(
    inter_id: i32,
    vertex: usize,
    prior_alias: Option<&str>,
    live_heads: &[NativeStemsBeamHeadLinkHeadState],
) -> Result<(NativeStemsBeamHeadIncidentOpposite, Option<String>), NativeStemsBeamVLinkHeadLinksError>
{
    let by_id = live_heads.iter().find(|head| head.inter_id == inter_id);
    let by_vertex = live_heads
        .iter()
        .find(|head| head.sig_vertex_identity == vertex);
    match (by_id, by_vertex) {
        (Some(left), Some(right)) if left.reference == right.reference => {
            if prior_alias.is_some_and(|alias| alias != left.alias) {
                return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
                    phase: "predecessor/live-head alias mismatch",
                });
            }
            return Ok((
                NativeStemsBeamHeadIncidentOpposite::Head(left.reference),
                prior_alias.map(ToOwned::to_owned),
            ));
        }
        (Some(_), _) | (_, Some(_)) => {
            return Err(NativeStemsBeamVLinkHeadLinksError::InvalidState {
                phase: "predecessor/live-head endpoint coordinate collision",
            });
        }
        (None, None) => {}
    }
    Ok((
        NativeStemsBeamHeadIncidentOpposite::OtherInter,
        prior_alias.map(ToOwned::to_owned),
    ))
}

fn seed_predecessor_stem_snapshot(
    rows: &[PredecessorStemIncidentRow],
    stem_vertex: usize,
    relation_objects: &mut BTreeMap<usize, NativeStemsBeamHeadRelationObjectIdentity>,
    relation_payloads: &mut BTreeMap<usize, RelationPayload>,
    relation_endpoints: &mut BTreeMap<usize, (usize, usize)>,
    relation_sources: &mut BTreeMap<usize, usize>,
) -> Result<(), NativeStemsBeamVLinkHeadLinksError> {
    for row in rows {
        let (source, target) = match row.direction {
            NativeStemsBeamIncidentDirection::Incoming => {
                (row.opposite_vertex_identity, stem_vertex)
            }
            NativeStemsBeamIncidentDirection::Outgoing => {
                (stem_vertex, row.opposite_vertex_identity)
            }
        };
        join_relation_object(
            relation_objects,
            row.graph_relation_identity,
            row.relation_object_identity,
        )?;
        let kind = if row.relation_class == HEAD_STEM_CLASS {
            NativeStemsBeamHeadQueryRelationKind::HeadStem
        } else {
            NativeStemsBeamHeadQueryRelationKind::Other
        };
        join_relation_payload(
            relation_payloads,
            row.graph_relation_identity,
            row.relation_object_identity,
            &row.relation_class,
            Some(kind),
        )?;
        join_relation_endpoints(
            relation_endpoints,
            row.graph_relation_identity,
            source,
            target,
        )?;
        join_relation_source(relation_sources, row.graph_relation_identity, source)?;
    }
    Ok(())
}

fn incident_payload_equal(
    left: &NativeStemsBeamHeadIncidentRelation,
    right: &NativeStemsBeamHeadIncidentRelation,
) -> bool {
    left.direction == right.direction
        && left.direction_ordinal == right.direction_ordinal
        && left.graph_relation_identity == right.graph_relation_identity
        && left.relation_object_identity == right.relation_object_identity
        && left.relation_class == right.relation_class
        && left.kind == right.kind
        && left.opposite == right.opposite
        && left.opposite_alias == right.opposite_alias
        && left.opposite_inter_id == right.opposite_inter_id
        && left.opposite_vertex_identity == right.opposite_vertex_identity
        && left.class_read == right.class_read
}

fn initial_relation_object_map(
    state: &NativeStemsBeamVLinkSiblingLinksState,
) -> Result<
    BTreeMap<usize, NativeStemsBeamHeadRelationObjectIdentity>,
    NativeStemsBeamVLinkHeadLinksError,
> {
    let mut result = BTreeMap::new();
    for relation in &state.base_apply_state_after.sig.appended_relations {
        let object = match relation.relation_object_identity {
            crate::native_stems_beam_vlink_base_apply::NativeStemsBeamRelationObjectIdentity::GraphObject(
                identity,
            ) => NativeStemsBeamHeadRelationObjectIdentity::GraphObject(identity),
            crate::native_stems_beam_vlink_base_apply::NativeStemsBeamRelationObjectIdentity::FreshDraft(
                plan,
            ) => NativeStemsBeamHeadRelationObjectIdentity::BaseDraft(plan),
        };
        join_relation_object(&mut result, relation.graph_relation_identity, object)?;
    }
    for relation in &state.appended_relations {
        let object = match relation.relation_object_identity {
            NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(identity) => {
                NativeStemsBeamHeadRelationObjectIdentity::GraphObject(identity)
            }
            NativeStemsBeamSiblingRelationObjectIdentity::BaseDraft(plan) => {
                NativeStemsBeamHeadRelationObjectIdentity::BaseDraft(plan)
            }
            NativeStemsBeamSiblingRelationObjectIdentity::SiblingDraft {
                plan_ordinal,
                sibling_ordinal,
            } => NativeStemsBeamHeadRelationObjectIdentity::SiblingDraft {
                plan_ordinal,
                sibling_ordinal,
            },
        };
        join_relation_object(&mut result, relation.graph_relation_identity, object)?;
    }
    Ok(result)
}

fn base_relation_object(
    object: crate::native_stems_beam_vlink_base_apply::NativeStemsBeamRelationObjectIdentity,
) -> NativeStemsBeamHeadRelationObjectIdentity {
    match object {
        crate::native_stems_beam_vlink_base_apply::NativeStemsBeamRelationObjectIdentity::GraphObject(
            identity,
        ) => NativeStemsBeamHeadRelationObjectIdentity::GraphObject(identity),
        crate::native_stems_beam_vlink_base_apply::NativeStemsBeamRelationObjectIdentity::FreshDraft(
            plan,
        ) => NativeStemsBeamHeadRelationObjectIdentity::BaseDraft(plan),
    }
}

fn sibling_relation_object(
    object: NativeStemsBeamSiblingRelationObjectIdentity,
) -> NativeStemsBeamHeadRelationObjectIdentity {
    match object {
        NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(identity) => {
            NativeStemsBeamHeadRelationObjectIdentity::GraphObject(identity)
        }
        NativeStemsBeamSiblingRelationObjectIdentity::BaseDraft(plan) => {
            NativeStemsBeamHeadRelationObjectIdentity::BaseDraft(plan)
        }
        NativeStemsBeamSiblingRelationObjectIdentity::SiblingDraft {
            plan_ordinal,
            sibling_ordinal,
        } => NativeStemsBeamHeadRelationObjectIdentity::SiblingDraft {
            plan_ordinal,
            sibling_ordinal,
        },
    }
}

fn initial_relation_endpoint_map(
    state: &NativeStemsBeamVLinkSiblingLinksState,
) -> Result<BTreeMap<usize, (usize, usize)>, NativeStemsBeamVLinkHeadLinksError> {
    let mut result = BTreeMap::new();
    for relation in &state.base_apply_state_after.sig.appended_relations {
        join_relation_endpoints(
            &mut result,
            relation.graph_relation_identity,
            relation.source_vertex_identity,
            relation.target_vertex_identity,
        )?;
    }
    for relation in &state.appended_relations {
        join_relation_endpoints(
            &mut result,
            relation.graph_relation_identity,
            relation.source_vertex_identity,
            relation.target_vertex_identity,
        )?;
    }
    Ok(result)
}

fn join_relation_endpoints(
    map: &mut BTreeMap<usize, (usize, usize)>,
    graph_identity: usize,
    source_vertex: usize,
    target_vertex: usize,
) -> Result<(), NativeStemsBeamVLinkHeadLinksError> {
    let endpoints = (source_vertex, target_vertex);
    if map
        .get(&graph_identity)
        .is_some_and(|prior| *prior != endpoints)
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
            phase: "cross-query relation endpoint consistency",
        });
    }
    map.insert(graph_identity, endpoints);
    Ok(())
}

fn join_relation_source(
    map: &mut BTreeMap<usize, usize>,
    graph_identity: usize,
    source_vertex: usize,
) -> Result<(), NativeStemsBeamVLinkHeadLinksError> {
    if map
        .get(&graph_identity)
        .is_some_and(|prior| *prior != source_vertex)
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
            phase: "cross-query relation source consistency",
        });
    }
    map.insert(graph_identity, source_vertex);
    Ok(())
}

fn join_relation_object(
    map: &mut BTreeMap<usize, NativeStemsBeamHeadRelationObjectIdentity>,
    graph_identity: usize,
    object: NativeStemsBeamHeadRelationObjectIdentity,
) -> Result<(), NativeStemsBeamVLinkHeadLinksError> {
    if map
        .get(&graph_identity)
        .is_some_and(|prior| *prior != object)
        || map
            .iter()
            .any(|(identity, prior)| *identity != graph_identity && *prior == object)
    {
        return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
            phase: "graph/relation-object identity consistency",
        });
    }
    map.insert(graph_identity, object);
    Ok(())
}

fn join_relation_payload(
    map: &mut BTreeMap<usize, RelationPayload>,
    graph_identity: usize,
    object: NativeStemsBeamHeadRelationObjectIdentity,
    class: &str,
    kind: Option<NativeStemsBeamHeadQueryRelationKind>,
) -> Result<(), NativeStemsBeamVLinkHeadLinksError> {
    if let Some(prior) = map.get_mut(&graph_identity) {
        if prior.object != object
            || prior.class != class
            || prior
                .kind
                .zip(kind)
                .is_some_and(|(left, right)| left != right)
        {
            return Err(NativeStemsBeamVLinkHeadLinksError::InvalidEvidence {
                phase: "cross-query relation payload consistency",
            });
        }
        if prior.kind.is_none() {
            prior.kind = kind;
        }
    } else {
        map.insert(
            graph_identity,
            RelationPayload {
                object,
                class: class.to_owned(),
                kind,
            },
        );
    }
    Ok(())
}

fn next_relation_identity(
    state: &NativeStemsBeamVLinkSiblingLinksState,
    head_appends: &[NativeStemsBeamHeadAppendedRelation],
) -> Result<usize, NativeStemsBeamVLinkHeadLinksError> {
    state
        .base_apply_state_after
        .sig
        .baseline_relation_count
        .checked_add(state.base_apply_state_after.sig.appended_relations.len())
        .and_then(|value| value.checked_add(state.appended_relations.len()))
        .and_then(|value| value.checked_add(head_appends.len()))
        .ok_or(NativeStemsBeamVLinkHeadLinksError::UnsupportedV1 {
            phase: "SIG relation identity overflow",
        })
}

fn append_edge_operations(
    map_ordinal: usize,
    graph_relation_identity: usize,
    operations: &mut Vec<NativeStemsBeamVLinkHeadLinksOperation>,
) {
    operations.push(
        NativeStemsBeamVLinkHeadLinksOperation::SigGlobalRelationInserted {
            map_ordinal,
            graph_relation_identity,
        },
    );
    operations.push(
        NativeStemsBeamVLinkHeadLinksOperation::HeadOutgoingRelationInserted {
            map_ordinal,
            graph_relation_identity,
        },
    );
    operations.push(
        NativeStemsBeamVLinkHeadLinksOperation::StemIncomingRelationInserted {
            map_ordinal,
            graph_relation_identity,
        },
    );
    operations.push(
        NativeStemsBeamVLinkHeadLinksOperation::SigEdgeEventDispatched {
            map_ordinal,
            graph_relation_identity,
        },
    );
}

fn dirty_cascade(
    map_ordinal: usize,
    subject: NativeStemsBeamHeadDirtySubject,
    state: &mut NativeStemsBeamVLinkHeadLinksState,
    operations: &mut Vec<NativeStemsBeamVLinkHeadLinksOperation>,
) {
    state.sheet_edit.stub_modified = true;
    operations.push(
        NativeStemsBeamVLinkHeadLinksOperation::SheetStubModifiedSetTrue {
            map_ordinal,
            subject,
        },
    );
    state.sheet_edit.book_modified = true;
    operations.push(
        NativeStemsBeamVLinkHeadLinksOperation::BookModifiedSetTrue {
            map_ordinal,
            subject,
        },
    );
    state.sheet_edit.book_dirty = true;
    operations.push(NativeStemsBeamVLinkHeadLinksOperation::BookDirtySetTrue {
        map_ordinal,
        subject,
    });
}

fn sheet_edit_delta(
    before: NativeStemsBeamSheetEditState,
    after: NativeStemsBeamSheetEditState,
) -> usize {
    usize::from(before.stub_modified != after.stub_modified)
        + usize::from(before.book_modified != after.book_modified)
        + usize::from(before.book_dirty != after.book_dirty)
}

fn query_rows_sha256(rows: impl IntoIterator<Item = String>) -> String {
    let mut bytes = Vec::new();
    for row in rows {
        bytes.extend_from_slice(row.as_bytes());
        bytes.push(b'\n');
    }
    sha256_hex(&bytes)
}

fn graph_relation_token(identity: usize) -> String {
    format!("sig-edge:{identity}")
}

fn relation_object_token(identity: NativeStemsBeamHeadRelationObjectIdentity) -> String {
    match identity {
        NativeStemsBeamHeadRelationObjectIdentity::GraphObject(identity) => {
            format!("sig-relation-object:{identity}")
        }
        NativeStemsBeamHeadRelationObjectIdentity::BaseDraft(plan) => {
            format!("base-draft:{plan}")
        }
        NativeStemsBeamHeadRelationObjectIdentity::SiblingDraft {
            plan_ordinal,
            sibling_ordinal,
        } => format!("sibling-draft:{plan_ordinal}:{sibling_ordinal}"),
        NativeStemsBeamHeadRelationObjectIdentity::HeadDraft {
            plan_ordinal,
            map_ordinal,
        } => format!("head-draft:{plan_ordinal}:{map_ordinal}"),
    }
}

fn source_outgoing_query_sha256(rows: &[NativeStemsBeamHeadSourceOutgoingRelation]) -> String {
    query_rows_sha256(rows.iter().map(|row| {
        format!(
            "{}:{}:{}:{}:{}",
            row.source_outgoing_ordinal,
            graph_relation_token(row.graph_relation_identity),
            relation_object_token(row.relation_object_identity),
            row.relation_class,
            row.target_vertex_identity,
        )
    }))
}

fn pair_query_sha256(rows: &[NativeStemsBeamHeadPairRelation]) -> String {
    query_rows_sha256(rows.iter().map(|row| {
        format!(
            "{}:{}:{}:{}:{}",
            row.pair_ordinal,
            row.source_outgoing_ordinal,
            graph_relation_token(row.graph_relation_identity),
            relation_object_token(row.relation_object_identity),
            row.relation_class,
        )
    }))
}

fn incident_direction_token(direction: NativeStemsBeamIncidentDirection) -> &'static str {
    match direction {
        NativeStemsBeamIncidentDirection::Incoming => "Incoming",
        NativeStemsBeamIncidentDirection::Outgoing => "Outgoing",
    }
}

fn incident_query_sha256(rows: &[NativeStemsBeamHeadIncidentRelation]) -> String {
    query_rows_sha256(rows.iter().map(|row| {
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}",
            row.incident_ordinal,
            incident_direction_token(row.direction),
            row.direction_ordinal,
            graph_relation_token(row.graph_relation_identity),
            relation_object_token(row.relation_object_identity),
            row.relation_class,
            row.opposite_alias,
            row.opposite_inter_id,
            row.opposite_vertex_identity,
        )
    }))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_hex(input: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut padded = input.to_vec();
    let bit_len = u64::try_from(input.len()).expect("query certificate length fits u64") * 8;
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    let mut digest = [
        0x6a09e667_u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in padded.chunks_exact(64) {
        let mut words = [0_u32; 64];
        for (index, bytes) in chunk.chunks_exact(4).enumerate() {
            words[index] = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = digest;
        for index in 0..64 {
            let upper_e = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(upper_e)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let upper_a = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = upper_a.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (slot, value) in digest.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
    digest.iter().map(|word| format!("{word:08x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(identity: usize) -> NativeStemsBeamHeadRelationObjectIdentity {
        NativeStemsBeamHeadRelationObjectIdentity::GraphObject(identity)
    }

    fn outgoing(
        source_outgoing_ordinal: usize,
        graph_relation_identity: usize,
        relation_class: &str,
        target_vertex_identity: usize,
    ) -> NativeStemsBeamHeadSourceOutgoingRelation {
        NativeStemsBeamHeadSourceOutgoingRelation {
            source_outgoing_ordinal,
            graph_relation_identity,
            relation_object_identity: object(graph_relation_identity),
            relation_class: relation_class.to_owned(),
            target_vertex_identity,
        }
    }

    fn pair(
        pair_ordinal: usize,
        graph_relation_identity: usize,
        relation_class: &str,
        kind: NativeStemsBeamHeadQueryRelationKind,
        class_read: NativeStemsBeamHeadPairClassRead,
    ) -> NativeStemsBeamHeadPairRelation {
        NativeStemsBeamHeadPairRelation {
            pair_ordinal,
            source_outgoing_ordinal: pair_ordinal,
            graph_relation_identity,
            relation_object_identity: object(graph_relation_identity),
            relation_class: relation_class.to_owned(),
            kind,
            class_read,
        }
    }

    fn exact_pair_scan() -> NativeStemsBeamHeadDirectedPairScan {
        let source_outgoing_relations = vec![
            outgoing(0, 2, "example.OtherRelation", 6),
            outgoing(1, 4, HEAD_STEM_CLASS, 6),
            outgoing(2, 7, "example.OtherRelation", 6),
        ];
        let relations = vec![
            pair(
                0,
                2,
                "example.OtherRelation",
                NativeStemsBeamHeadQueryRelationKind::Other,
                NativeStemsBeamHeadPairClassRead::ExaminedContinue,
            ),
            pair(
                1,
                4,
                HEAD_STEM_CLASS,
                NativeStemsBeamHeadQueryRelationKind::HeadStem,
                NativeStemsBeamHeadPairClassRead::ExaminedMatchBreak,
            ),
            pair(
                2,
                7,
                "example.OtherRelation",
                NativeStemsBeamHeadQueryRelationKind::Other,
                NativeStemsBeamHeadPairClassRead::UnreadAfterBreak,
            ),
        ];
        NativeStemsBeamHeadDirectedPairScan {
            source_outgoing_scanned: source_outgoing_relations.len(),
            source_outgoing_provenance_sha256: source_outgoing_query_sha256(
                &source_outgoing_relations,
            ),
            source_outgoing_relations,
            query_relation_count: relations.len(),
            pair_provenance_sha256: pair_query_sha256(&relations),
            relations,
        }
    }

    fn check_pair(
        scan: &NativeStemsBeamHeadDirectedPairScan,
    ) -> Result<PairReadResult, NativeStemsBeamVLinkHeadLinksError> {
        let mut objects = BTreeMap::new();
        let mut payloads = BTreeMap::new();
        let mut endpoints = scan
            .source_outgoing_relations
            .iter()
            .filter(|row| row.target_vertex_identity == 6)
            .map(|row| (row.graph_relation_identity, (5, 6)))
            .collect::<BTreeMap<_, _>>();
        let mut sources = BTreeMap::new();
        validate_pair_scan(
            scan,
            PairValidationContext {
                graph_limit: 8,
                source_head_vertex: 5,
                target_stem_vertex: 6,
                vertex_limit: 20,
            },
            RelationValidationMaps {
                objects: &mut objects,
                payloads: &mut payloads,
                endpoints: &mut endpoints,
                sources: &mut sources,
            },
        )
    }

    fn incident(
        incident_ordinal: usize,
        direction: NativeStemsBeamIncidentDirection,
        direction_ordinal: usize,
        graph_relation_identity: usize,
    ) -> NativeStemsBeamHeadIncidentRelation {
        NativeStemsBeamHeadIncidentRelation {
            incident_ordinal,
            direction,
            direction_ordinal,
            graph_relation_identity,
            relation_object_identity: object(graph_relation_identity),
            relation_class: "example.OtherRelation".to_owned(),
            kind: NativeStemsBeamHeadQueryRelationKind::Other,
            opposite: NativeStemsBeamHeadIncidentOpposite::OtherInter,
            opposite_alias: format!("inter:{}", 100 + graph_relation_identity),
            opposite_inter_id: i32::try_from(100 + graph_relation_identity).unwrap(),
            opposite_vertex_identity: graph_relation_identity,
            class_read: NativeStemsBeamHeadIncidentClassRead::ExaminedContinue,
        }
    }

    fn corner(vertical: NativeStemVerticalSide) -> NativeStemsBeamHeadCornerRef {
        NativeStemsBeamHeadCornerRef {
            head: NativeHeadStaffEpilogRef {
                staff_index: 0,
                head_index: 2,
            },
            sig_ordinal: 7,
            x_ordinal: 3,
            horizontal: NativeStemHeadSide::Left,
            vertical,
        }
    }

    fn plan_relation() -> NativeStemsBeamHeadRelation {
        let corner = corner(NativeStemVerticalSide::Top);
        NativeStemsBeamHeadRelation {
            corner,
            map_ordinal: 0,
            first_item_index: 1,
            latest_item_index: 1,
            replaced_existing_payload: false,
            check: crate::native_stems_beam_link_plans::NativeStemsBeamHeadRelationCheck {
                encountered_corner: corner,
                encountered_horizontal: NativeStemHeadSide::Left,
                derived_horizontal: NativeStemHeadSide::Left,
                horizontal_side_diverges: false,
                vertical: NativeStemVerticalSide::Top,
                head_center: (40, 50),
                reference_point: NativeStemPoint { x: 41.0, y: 48.0 },
                stump_bounds: None,
                x_stem: 42.0,
                x_gap_pixels: 1.0,
                y_gap_pixels: 0.0,
                dx: 0.05,
                dy: 0.0,
                horizontal_gap_kind:
                    crate::native_stems_beam_link_plans::NativeStemsBeamHorizontalGapKind::Out,
                x_maximum: 0.3,
                y_maximum: 0.8,
                x_weight: 2.0,
                y_weight: 1.0,
                raw_x_impact: 0.8,
                raw_y_impact: 1.0,
                x_impact: 0.8,
                y_impact: 1.0,
                grade: 0.85,
                extension_point: Some(NativeStemPoint { x: 42.0, y: 46.0 }),
                accepted: true,
            },
        }
    }

    fn existing_step() -> NativeStemsBeamHeadLinkStepCertificate {
        let relation = plan_relation();
        let head = NativeStemsBeamHeadLinkHeadRef::from_corner(relation.corner);
        let mut directed_pair = exact_pair_scan();
        // Replace the generic fixture draft domain with an attached graph
        // object: the current plan draft must remain absent from this query.
        directed_pair.source_outgoing_relations[1].relation_object_identity = object(4);
        directed_pair.relations[1].relation_object_identity = object(4);
        directed_pair.source_outgoing_provenance_sha256 =
            source_outgoing_query_sha256(&directed_pair.source_outgoing_relations);
        directed_pair.pair_provenance_sha256 = pair_query_sha256(&directed_pair.relations);
        NativeStemsBeamHeadLinkStepCertificate {
            map_ordinal: 0,
            corner: relation.corner,
            s_linker: NativeStemsBeamHeadSLinkerRef {
                head,
                horizontal: NativeStemHeadSide::Left,
            },
            plan_draft: NativeStemsBeamHeadPlanDraft {
                relation_object_identity: NativeStemsBeamHeadRelationObjectIdentity::HeadDraft {
                    plan_ordinal: 3,
                    map_ordinal: 0,
                },
                relation_class: HEAD_STEM_CLASS.to_owned(),
                relation: relation.clone(),
                manual: false,
                head_side_before: Some(NativeStemHeadSide::Left),
                extension_point_before: relation.check.extension_point,
                consistency_before: None,
                graph_matches_before: 0,
            },
            directed_pair,
            expected_consistency_after: None,
            consistency_debug_enabled: None,
            add_edge_returned: None,
            head_incident_after: None,
            stem_incident_after: None,
        }
    }

    #[test]
    fn sha256_vectors_are_standard_and_lowercase() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(valid_sha256(&"a".repeat(64)));
        assert!(!valid_sha256(&"A".repeat(64)));
    }

    #[test]
    fn standard_head_consistency_uses_signed_unclamped_ratio() {
        let value = head_stem_consistency(false, 10.0, 66.0, 20).unwrap();
        assert_eq!(value.to_bits(), 1.0_f64.to_bits());
        let negative = head_stem_consistency(false, 66.0, 10.0, 20).unwrap();
        assert_eq!(negative.to_bits(), (-1.0_f64).to_bits());
    }

    #[test]
    fn small_head_consistency_uses_exact_reciprocal() {
        let value = head_stem_consistency(true, 10.0, 38.0, 20).unwrap();
        assert_eq!(value.to_bits(), 2.0_f64.to_bits());
    }

    #[test]
    fn head_shape_derives_small_and_stem_predicates() {
        assert_eq!(
            supported_head_shape_flags("NOTEHEAD_BLACK"),
            Some((false, true))
        );
        assert_eq!(
            supported_head_shape_flags("NOTEHEAD_VOID_SMALL"),
            Some((true, true))
        );
        assert_eq!(
            supported_head_shape_flags("WHOLE_NOTE_SMALL"),
            Some((true, false))
        );
        assert_eq!(supported_head_shape_flags("ACCIDENTAL_SHARP"), None);
    }

    #[test]
    fn accepted_plan_relation_payload_is_finite_and_complete() {
        let relation = plan_relation();
        assert!(valid_plan_relation(&relation));
        let mut malformed = relation.clone();
        malformed.check.grade = f64::NAN;
        assert!(!valid_plan_relation(&malformed));
        let mut malformed = relation;
        malformed.check.extension_point = None;
        assert!(!valid_plan_relation(&malformed));
    }

    #[test]
    fn plan_draft_requires_prepopulated_side_extension_and_null_consistency() {
        let plan = NativeStemsBeamPlanRef {
            system_id: 1,
            plan_ordinal: 3,
            builder_ordinal: 0,
            stem_profile: 3,
        };
        let step = existing_step();
        let certificate = NativeStemsBeamVLinkHeadLinksCertificate {
            system_id: 1,
            headless: true,
            listener_topology: NativeStemsBeamSigListenerTopology::SoleStandardSigListener,
            interline: 20,
            neutral_stem_length: NEUTRAL_STEM_LENGTH,
            steps: vec![step.clone()],
        };
        validate_step_headers(plan, &[plan_relation()], &certificate).unwrap();
        let mut drifted = certificate.clone();
        drifted.steps[0].plan_draft.head_side_before = None;
        assert!(validate_step_headers(plan, &[plan_relation()], &drifted).is_err());
        let mut drifted = certificate;
        drifted.steps[0].plan_draft.consistency_before = Some(1.0);
        assert!(validate_step_headers(plan, &[plan_relation()], &drifted).is_err());
    }

    #[test]
    fn existing_branch_forbids_consistency_edge_and_callback_reads() {
        let step = existing_step();
        validate_existing_branch_unread(&step).unwrap();
        let mut drifted = step;
        drifted.consistency_debug_enabled = Some(false);
        assert!(validate_existing_branch_unread(&drifted).is_err());
        drifted.consistency_debug_enabled = None;
        drifted.add_edge_returned = Some(true);
        assert!(validate_existing_branch_unread(&drifted).is_err());
    }

    #[test]
    fn zero_or_nonfinite_consistency_fails_closed() {
        assert!(head_stem_consistency(false, 10.0, 10.0, 20).is_ok());
        assert!(head_stem_consistency(true, 10.0, 10.0, 20).is_err());
        assert!(head_stem_consistency(false, f64::NAN, 20.0, 20).is_err());
        assert!(head_stem_consistency(false, 0.0, 20.0, 0).is_err());
    }

    #[test]
    fn directed_pair_stops_on_first_assignable_head_stem() {
        let scan = exact_pair_scan();
        let result = check_pair(&scan).unwrap();
        assert_eq!(result.relations_read, 2);
        assert_eq!(result.first_match, Some(1));
    }

    #[test]
    fn empty_directed_pair_is_the_missing_relation_branch() {
        let mut scan = NativeStemsBeamHeadDirectedPairScan {
            source_outgoing_scanned: 0,
            source_outgoing_provenance_sha256: String::new(),
            source_outgoing_relations: Vec::new(),
            query_relation_count: 0,
            pair_provenance_sha256: String::new(),
            relations: Vec::new(),
        };
        scan.source_outgoing_provenance_sha256 =
            source_outgoing_query_sha256(&scan.source_outgoing_relations);
        scan.pair_provenance_sha256 = pair_query_sha256(&scan.relations);
        let result = check_pair(&scan).unwrap();
        assert_eq!(result.relations_read, 0);
        assert_eq!(result.first_match, None);
    }

    #[test]
    fn directed_pair_rejects_unread_or_hash_drift() {
        let mut scan = exact_pair_scan();
        scan.relations[2].class_read = NativeStemsBeamHeadPairClassRead::ExaminedContinue;
        scan.pair_provenance_sha256 = pair_query_sha256(&scan.relations);
        assert!(check_pair(&scan).is_err());
        let mut scan = exact_pair_scan();
        scan.source_outgoing_provenance_sha256
            .replace_range(0..1, "A");
        assert!(check_pair(&scan).is_err());
    }

    #[test]
    fn directed_pair_accepts_only_exactly_attached_prior_draft() {
        let mut scan = exact_pair_scan();
        let draft = NativeStemsBeamHeadRelationObjectIdentity::HeadDraft {
            plan_ordinal: 3,
            map_ordinal: 1,
        };
        scan.source_outgoing_relations[0].relation_object_identity = draft;
        scan.relations[0].relation_object_identity = draft;
        scan.source_outgoing_provenance_sha256 =
            source_outgoing_query_sha256(&scan.source_outgoing_relations);
        scan.pair_provenance_sha256 = pair_query_sha256(&scan.relations);
        assert!(validate_pair_draft_visibility(&scan, &[]).is_err());
        let attached = NativeStemsBeamHeadAppendedRelation {
            graph_relation_identity: 2,
            relation_object_identity: draft,
            source_head: NativeStemsBeamHeadLinkHeadRef::from_corner(corner(
                NativeStemVerticalSide::Top,
            )),
            source_vertex_identity: 4,
            target_stem_identity: 5,
            target_vertex_identity: 6,
            relation: plan_relation(),
            consistency: 1.0,
        };
        assert!(validate_pair_draft_visibility(&scan, &[attached]).is_ok());
    }

    #[test]
    fn exact_class_and_kind_cannot_disagree() {
        let mut scan = exact_pair_scan();
        scan.relations[1].kind = NativeStemsBeamHeadQueryRelationKind::Other;
        scan.relations[1].class_read = NativeStemsBeamHeadPairClassRead::ExaminedContinue;
        scan.relations[2].class_read = NativeStemsBeamHeadPairClassRead::ExaminedContinue;
        scan.pair_provenance_sha256 = pair_query_sha256(&scan.relations);
        assert!(matches!(
            check_pair(&scan),
            Err(NativeStemsBeamVLinkHeadLinksError::UnsupportedV1 { .. })
        ));
    }

    #[test]
    fn directed_pair_is_exact_source_target_filter() {
        let mut scan = exact_pair_scan();
        scan.source_outgoing_relations[2].target_vertex_identity = 7;
        scan.source_outgoing_provenance_sha256 =
            source_outgoing_query_sha256(&scan.source_outgoing_relations);
        assert!(check_pair(&scan).is_err());
    }

    #[test]
    fn directed_pair_rejects_relation_absent_from_predecessor_snapshot() {
        let scan = exact_pair_scan();
        assert!(
            validate_pair_scan(
                &scan,
                PairValidationContext {
                    graph_limit: 8,
                    source_head_vertex: 5,
                    target_stem_vertex: 6,
                    vertex_limit: 20,
                },
                RelationValidationMaps {
                    objects: &mut BTreeMap::new(),
                    payloads: &mut BTreeMap::new(),
                    endpoints: &mut BTreeMap::new(),
                    sources: &mut BTreeMap::new(),
                },
            )
            .is_err()
        );
    }

    #[test]
    fn source_identity_is_stable_across_queries() {
        let source_outgoing_relations = vec![outgoing(0, 2, "example.OtherRelation", 7)];
        let scan = NativeStemsBeamHeadDirectedPairScan {
            source_outgoing_scanned: 1,
            source_outgoing_provenance_sha256: source_outgoing_query_sha256(
                &source_outgoing_relations,
            ),
            source_outgoing_relations,
            query_relation_count: 0,
            pair_provenance_sha256: pair_query_sha256(&[]),
            relations: Vec::new(),
        };
        let mut objects = BTreeMap::new();
        let mut payloads = BTreeMap::new();
        let mut endpoints = BTreeMap::new();
        let mut sources = BTreeMap::new();
        validate_pair_scan(
            &scan,
            PairValidationContext {
                graph_limit: 8,
                source_head_vertex: 5,
                target_stem_vertex: 6,
                vertex_limit: 20,
            },
            RelationValidationMaps {
                objects: &mut objects,
                payloads: &mut payloads,
                endpoints: &mut endpoints,
                sources: &mut sources,
            },
        )
        .unwrap();
        assert!(
            validate_pair_scan(
                &scan,
                PairValidationContext {
                    graph_limit: 8,
                    source_head_vertex: 8,
                    target_stem_vertex: 6,
                    vertex_limit: 20,
                },
                RelationValidationMaps {
                    objects: &mut objects,
                    payloads: &mut payloads,
                    endpoints: &mut endpoints,
                    sources: &mut sources,
                },
            )
            .is_err()
        );
    }

    #[test]
    fn incident_order_is_incoming_then_outgoing_and_source_ordered() {
        let rows = vec![
            incident(0, NativeStemsBeamIncidentDirection::Incoming, 0, 1),
            incident(1, NativeStemsBeamIncidentDirection::Incoming, 1, 3),
            incident(2, NativeStemsBeamIncidentDirection::Outgoing, 0, 2),
            incident(3, NativeStemsBeamIncidentDirection::Outgoing, 1, 6),
        ];
        validate_incident_order(&rows).unwrap();
        let mut reordered = rows.clone();
        reordered.swap(0, 1);
        assert!(validate_incident_order(&reordered).is_err());
        let mut reversed_phase = rows;
        reversed_phase[3].direction = NativeStemsBeamIncidentDirection::Incoming;
        assert!(validate_incident_order(&reversed_phase).is_err());
    }

    #[test]
    fn shared_s_observers_are_exact_top_then_bottom() {
        let reference = NativeStemsBeamHeadSLinkerRef {
            head: NativeStemsBeamHeadLinkHeadRef::from_corner(corner(NativeStemVerticalSide::Top)),
            horizontal: NativeStemHeadSide::Left,
        };
        let mut cell = NativeStemsBeamHeadSLinkerCell {
            reference,
            linked: false,
            closed: true,
            ordered_observer_corners: vec![
                corner(NativeStemVerticalSide::Top),
                corner(NativeStemVerticalSide::Bottom),
            ],
        };
        assert!(canonical_s_observers(&cell));
        cell.ordered_observer_corners.swap(0, 1);
        assert!(!canonical_s_observers(&cell));
        cell.ordered_observer_corners[0].horizontal = NativeStemHeadSide::Right;
        assert!(!canonical_s_observers(&cell));
    }

    #[test]
    fn first_callback_extends_exact_predecessor_stem_snapshot() {
        let predecessor = vec![PredecessorStemIncidentRow {
            direction: NativeStemsBeamIncidentDirection::Incoming,
            direction_ordinal: 0,
            graph_relation_identity: 2,
            relation_object_identity: object(2),
            relation_class: "example.OtherRelation".to_owned(),
            opposite_inter_id: 102,
            opposite_vertex_identity: 2,
            opposite: NativeStemsBeamHeadIncidentOpposite::OtherInter,
            opposite_alias: Some("inter:102".to_owned()),
        }];
        let relations = vec![
            incident(0, NativeStemsBeamIncidentDirection::Incoming, 0, 2),
            incident(1, NativeStemsBeamIncidentDirection::Incoming, 1, 9),
        ];
        let current = NativeStemsBeamHeadIncidentScan {
            query_relation_count: relations.len(),
            query_provenance_sha256: incident_query_sha256(&relations),
            relations,
        };
        validate_stem_scan_evolution(None, &current, 9, &predecessor).unwrap();
        let mut incomplete = current;
        incomplete.relations.remove(0);
        incomplete.query_relation_count = incomplete.relations.len();
        incomplete.query_provenance_sha256 = incident_query_sha256(&incomplete.relations);
        assert!(validate_stem_scan_evolution(None, &incomplete, 9, &predecessor).is_err());
    }

    #[test]
    fn head_callback_retains_complete_source_outgoing_partition() {
        let source = vec![outgoing(0, 2, "example.OtherRelation", 7)];
        let mut old = incident(0, NativeStemsBeamIncidentDirection::Outgoing, 0, 2);
        old.opposite_vertex_identity = 7;
        let draft = NativeStemsBeamHeadRelationObjectIdentity::HeadDraft {
            plan_ordinal: 3,
            map_ordinal: 0,
        };
        let mut fresh = incident(1, NativeStemsBeamIncidentDirection::Outgoing, 1, 9);
        fresh.relation_object_identity = draft;
        fresh.relation_class = HEAD_STEM_CLASS.to_owned();
        fresh.kind = NativeStemsBeamHeadQueryRelationKind::HeadStem;
        fresh.opposite = NativeStemsBeamHeadIncidentOpposite::Stem;
        fresh.opposite_vertex_identity = 6;
        let relations = vec![old, fresh];
        let scan = NativeStemsBeamHeadIncidentScan {
            query_relation_count: relations.len(),
            query_provenance_sha256: incident_query_sha256(&relations),
            relations,
        };
        validate_head_callback_outgoing_partition(&source, &scan, 9, draft, 6).unwrap();
        let mut incomplete = scan;
        incomplete.relations.remove(0);
        assert!(
            validate_head_callback_outgoing_partition(&source, &incomplete, 9, draft, 6).is_err()
        );
    }

    #[test]
    fn predecessor_head_endpoint_requires_exact_id_vertex_and_alias() {
        let head = NativeStemsBeamHeadLinkHeadState {
            reference: NativeStemsBeamHeadLinkHeadRef::from_corner(corner(
                NativeStemVerticalSide::Top,
            )),
            alias: "head:17".to_owned(),
            runtime_class: "org.audiveris.omr.sig.inter.HeadInter".to_owned(),
            inter_id: 17,
            inter_index_ordinal: 3,
            inter_index_object_matches: 1,
            inter_index_id_matches: 1,
            sig_vertex_identity: 8,
            sig_object_matches: 1,
            sig_system_id: 1,
            removed: false,
            vip: false,
            manual: false,
            abnormal: true,
            center: (40, 50),
            shape: "NOTEHEAD_BLACK".to_owned(),
            is_small: false,
            is_stem_head: true,
        };
        assert!(matches!(
            predecessor_opposite(17, 8, Some("head:17"), std::slice::from_ref(&head)),
            Ok((NativeStemsBeamHeadIncidentOpposite::Head(_), Some(_)))
        ));
        assert!(predecessor_opposite(17, 9, None, std::slice::from_ref(&head)).is_err());
        assert!(predecessor_opposite(18, 8, None, std::slice::from_ref(&head)).is_err());
        assert!(
            predecessor_opposite(17, 8, Some("inter:17"), std::slice::from_ref(&head)).is_err()
        );
    }

    #[test]
    fn graph_and_relation_object_domains_are_one_to_one() {
        let mut objects = BTreeMap::new();
        join_relation_object(&mut objects, 1, object(10)).unwrap();
        join_relation_object(&mut objects, 1, object(10)).unwrap();
        assert!(join_relation_object(&mut objects, 1, object(11)).is_err());
        assert!(join_relation_object(&mut objects, 2, object(10)).is_err());
    }

    #[test]
    fn later_same_head_pair_cannot_omit_prior_local_append() {
        let head = NativeStemsBeamHeadLinkHeadRef::from_corner(corner(NativeStemVerticalSide::Top));
        let draft = NativeStemsBeamHeadRelationObjectIdentity::HeadDraft {
            plan_ordinal: 3,
            map_ordinal: 0,
        };
        let graph_relation_identity = 9;
        let current = vec![NativeStemsBeamHeadSourceOutgoingRelation {
            source_outgoing_ordinal: 0,
            graph_relation_identity,
            relation_object_identity: draft,
            relation_class: HEAD_STEM_CLASS.to_owned(),
            target_vertex_identity: 6,
        }];
        let appended = vec![NativeStemsBeamHeadAppendedRelation {
            graph_relation_identity,
            relation_object_identity: draft,
            source_head: head,
            source_vertex_identity: 4,
            target_stem_identity: 5,
            target_vertex_identity: 6,
            relation: plan_relation(),
            consistency: 1.0,
        }];
        let prior = BTreeMap::from([(head, Vec::new())]);
        assert!(
            validate_source_scan_evolution(
                head,
                4,
                &current,
                &[],
                &prior,
                &appended,
                &BTreeMap::new(),
            )
            .is_err()
        );

        let pair = vec![NativeStemsBeamHeadPairRelation {
            pair_ordinal: 0,
            source_outgoing_ordinal: 0,
            graph_relation_identity,
            relation_object_identity: draft,
            relation_class: HEAD_STEM_CLASS.to_owned(),
            kind: NativeStemsBeamHeadQueryRelationKind::HeadStem,
            class_read: NativeStemsBeamHeadPairClassRead::ExaminedMatchBreak,
        }];
        validate_source_scan_evolution(
            head,
            4,
            &current,
            &pair,
            &prior,
            &appended,
            &BTreeMap::new(),
        )
        .unwrap();
    }

    #[test]
    fn query_hashes_bind_order_and_endpoint_payload() {
        let mut rows = vec![
            incident(0, NativeStemsBeamIncidentDirection::Incoming, 0, 1),
            incident(1, NativeStemsBeamIncidentDirection::Outgoing, 0, 2),
        ];
        let original = incident_query_sha256(&rows);
        rows[1].opposite_alias.push_str("-drift");
        assert_ne!(original, incident_query_sha256(&rows));
        rows.swap(0, 1);
        assert_ne!(original, incident_query_sha256(&rows));
    }
}
