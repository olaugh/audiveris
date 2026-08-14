// SPDX-License-Identifier: AGPL-3.0-or-later

//! Serial sibling-beam links after the selected B-linker flag assignment.
//!
//! This boundary independently replays the complete boundary-15 transaction,
//! executes exactly Java's `linkSiblings(stem, supportGrade)` call, and stops
//! before the head-relation loop.  The compact v1 envelope admits the standard
//! sole `SigListener`, live endpoints, known beam classes, and exhaustive
//! zero-`ChordStemRelation` callback scans.  Listener/graph exception
//! synthetics remain oracle evidence: public inputs cannot inject faults.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use audiveris_image::{beam_structure::Segment, run_table::Orientation};

use crate::{
    beam_inters::BeamKind,
    native_sig::{
        NativeSigEdge, NativeSigEdgeId, NativeSigInterKind, NativeSigRelationKind,
        NativeSigRelationOrigin, NativeSigSupport, NativeSigSystem, NativeSigSystemBindings,
        NativeSigVertexId,
    },
    native_stems_beam_builders::{
        NativeStemsBeamBuilderItemKind, NativeStemsBeamBuilderSystem,
        NativeStemsBeamBuilderTargetRef,
    },
    native_stems_beam_link_plans::NativeStemsBeamLinkPlanSystem,
    native_stems_beam_reachability::NativeStemsBeamReachabilitySystem,
    native_stems_beam_scheduler::{NativeStemsBeamSchedulerStatus, NativeStemsBeamSchedulerSystem},
    native_stems_beam_stumps::{
        NativeStemsBeamGlyph, NativeStemsBeamSource, NativeStemsBeamStumpBeam,
        NativeStemsBeamStumpSystem,
    },
    native_stems_beam_vlink_b_linker_flag::{
        NativeStemsBeamVLinkBLinkerFlagOutcome, NativeStemsBeamVLinkBLinkerFlagState,
        NativeStemsBeamVLinkBLinkerFlagTransaction,
        apply_native_stems_beam_vlink_b_linker_flag_transaction,
    },
    native_stems_beam_vlink_base_apply::{
        NativeStemsBeamBeamIncidentRead, NativeStemsBeamBeamIncidentRule,
        NativeStemsBeamIncidentDirection, NativeStemsBeamIncidentOpposite,
        NativeStemsBeamInterIndexLookup, NativeStemsBeamQueryRelationKind,
        NativeStemsBeamSheetEditState, NativeStemsBeamSigListenerTopology,
        NativeStemsBeamSigRelationKind, NativeStemsBeamVLinkBaseApplyKey,
        NativeStemsBeamVLinkBaseApplyState, NativeStemsBeamVLinkBaseApplyTransaction,
        NativeStemsBeamVLinkBeamRuntimeState,
    },
    native_stems_beam_vlink_reuse_check::{
        NativeStemsBeamRelationParameters, NativeStemsBeamVLinkReuseCheck,
        NativeStemsBeamVLinkReuseLiveState,
    },
    native_stems_beam_vlink_transaction::{
        NativeStemsBeamKnownSystemStem, NativeStemsBeamVLinkTransaction,
    },
    native_stems_beam_vlinkers::{
        NativeStemsBeamBLinkerRef, NativeStemsBeamVLinkerSystem, generic_intersection,
    },
    stems_step::{NativeBeamPortion, NativeStemHeadSide, NativeStemPoint, NativeStemVerticalSide},
};

const MAX_SHORTER_RATIO: f64 = 0.8;
const X_IN_GAP_MAXIMUM_PROFILE_0: f64 = 0.5;
const CONTAINMENT_CLASS: &str = "org.audiveris.omr.sig.relation.Containment";
const BEAM_STEM_CLASS: &str = "org.audiveris.omr.sig.relation.BeamStemRelation";
const BEAM_REST_CLASS: &str = "org.audiveris.omr.sig.relation.BeamRestRelation";
const CHORD_STEM_CLASS: &str = "org.audiveris.omr.sig.relation.ChordStemRelation";
const BEAM_INTER_CLASS: &str = "org.audiveris.omr.sig.inter.BeamInter";
const BEAM_HOOK_INTER_CLASS: &str = "org.audiveris.omr.sig.inter.BeamHookInter";
const SMALL_BEAM_INTER_CLASS: &str = "org.audiveris.omr.sig.inter.SmallBeamInter";
const BEAM_GROUP_INTER_CLASS: &str = "org.audiveris.omr.sig.inter.BeamGroupInter";
const STEM_GLYPH_ITEM_CLASS: &str = "org.audiveris.omr.sheet.stem.StemItem$GlyphItem";
const STEM_GAP_ITEM_CLASS: &str = "org.audiveris.omr.sheet.stem.StemItem$GapItem";
const STEM_LINKER_ITEM_CLASS: &str = "org.audiveris.omr.sheet.stem.StemItem$LinkerItem";
const STEM_HALF_LINKER_ITEM_CLASS: &str = "org.audiveris.omr.sheet.stem.StemItem$HalfLinkerItem";
const BEAM_B_LINKER_CLASS: &str = "org.audiveris.omr.sheet.stem.BeamLinker$BLinker";
const BEAM_V_LINKER_CLASS: &str = "org.audiveris.omr.sheet.stem.BeamLinker$BLinker$VLinker";
const HEAD_C_LINKER_CLASS: &str = "org.audiveris.omr.sheet.stem.HeadLinker$SLinker$CLinker";

/// One relation as observed through the port-owned insertion-ordered SIG.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingGraphRelation {
    pub edge: NativeSigEdgeId,
    pub origin: NativeSigRelationOrigin,
    pub source: NativeSigVertexId,
    pub target: NativeSigVertexId,
    pub kind: NativeSigRelationKind,
    pub grade: Option<f64>,
    pub beam_portion: Option<NativeBeamPortion>,
    pub extension_point: Option<NativeStemPoint>,
}

/// Native inputs for one sibling BeamStem draft. Order is the already-proven
/// Java `linkSiblings` order; every graph query and mutation is derived here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingGraphDraft {
    /// Ordinal in Java's complete selected-sibling loop, including siblings
    /// which take an earlier no-mutation branch.
    pub sibling_ordinal: usize,
    pub source: NativeStemsBeamSource,
    pub grade: f64,
    pub beam_portion: NativeBeamPortion,
    pub extension_point: NativeStemPoint,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingGraphStep {
    pub sibling_ordinal: usize,
    pub source: NativeStemsBeamSource,
    pub sibling_vertex: NativeSigVertexId,
    pub source_outgoing_before: Vec<NativeStemsBeamSiblingGraphRelation>,
    pub directed_pair_before: Vec<NativeStemsBeamSiblingGraphRelation>,
    pub appended: Option<NativeStemsBeamSiblingGraphRelation>,
    pub stem_incident_after: Vec<NativeStemsBeamSiblingGraphRelation>,
    pub beam_incident_after: Vec<NativeStemsBeamSiblingGraphRelation>,
    pub beam_abnormal_before: bool,
    pub beam_abnormal_after: bool,
}

/// Graph-only B16 evidence. Java object aliases, persistent Inter IDs, and
/// opaque member hashes are intentionally absent from this production type.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingGraphProjection {
    pub system_id: usize,
    pub group_ordinal: usize,
    pub group_vertex: NativeSigVertexId,
    pub base_source: NativeStemsBeamSource,
    pub base_vertex: NativeSigVertexId,
    pub stem_identity: usize,
    pub stem_vertex: NativeSigVertexId,
    pub group_outgoing: Vec<NativeStemsBeamSiblingGraphRelation>,
    pub group_members: Vec<NativeStemsBeamSource>,
    pub steps: Vec<NativeStemsBeamSiblingGraphStep>,
    pub appended_edges: Vec<NativeSigEdgeId>,
}

fn graph_relation(edge: &NativeSigEdge) -> NativeStemsBeamSiblingGraphRelation {
    NativeStemsBeamSiblingGraphRelation {
        edge: NativeSigEdgeId(edge.ordinal),
        origin: edge.origin,
        source: NativeSigVertexId(edge.source),
        target: NativeSigVertexId(edge.target),
        kind: edge.kind,
        grade: edge.support.map(|support| support.grade),
        beam_portion: edge.beam_portion,
        extension_point: edge.stem_extension,
    }
}

fn source_for_vertex(
    bindings: &NativeSigSystemBindings,
    vertex: NativeSigVertexId,
) -> Option<NativeStemsBeamSource> {
    bindings
        .beam_vertices
        .iter()
        .find_map(|(&source, &bound)| (bound == vertex).then_some(source))
}

fn native_sibling_source_outgoing(
    sig: &NativeSigSystem,
    source: NativeSigVertexId,
) -> Result<Vec<&NativeSigEdge>, NativeStemsBeamVLinkSiblingLinksError> {
    if sig.vertex(source.0).is_none() {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native sibling source vertex",
        });
    }
    sig.outgoing_edges(source.0)
        .map_err(|_| NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native sibling source outgoing query",
        })
}

fn native_beam_abnormal(
    sig: &NativeSigSystem,
    beam: NativeSigVertexId,
) -> Result<bool, NativeStemsBeamVLinkSiblingLinksError> {
    let vertex = sig
        .vertex(beam.0)
        .ok_or(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native sibling beam vertex",
        })?;
    let portions = sig
        .incident_edges(beam.0)
        .map_err(|_| NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native sibling beam incident query",
        })?
        .into_iter()
        .filter(|edge| edge.kind == NativeSigRelationKind::BeamStem)
        .filter_map(|edge| edge.beam_portion)
        .collect::<Vec<_>>();
    Ok(match vertex.kind {
        NativeSigInterKind::BeamHook => portions.is_empty(),
        NativeSigInterKind::Beam | NativeSigInterKind::SmallBeam => {
            !portions.contains(&NativeBeamPortion::Left)
                || !portions.contains(&NativeBeamPortion::Right)
        }
        _ => {
            return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                phase: "native sibling beam kind",
            });
        }
    })
}

fn project_native_sibling_graph(
    sig: &NativeSigSystem,
    bindings: &NativeSigSystemBindings,
    group_ordinal: usize,
    base_source: NativeStemsBeamSource,
    stem_identity: usize,
    plan_ordinal: usize,
    drafts: &[NativeStemsBeamSiblingGraphDraft],
) -> Result<
    (NativeStemsBeamSiblingGraphProjection, NativeSigSystem),
    NativeStemsBeamVLinkSiblingLinksError,
> {
    sig.validate_integrity()
        .map_err(|_| NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native sibling SIG integrity",
        })?;
    bindings.validate_against(sig).map_err(|_| {
        NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native sibling binding integrity",
        }
    })?;
    let group_vertex = bindings
        .beam_group_vertices
        .get(&group_ordinal)
        .copied()
        .ok_or(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native sibling beam-group binding",
        })?;
    let base_vertex = bindings.beam_vertices.get(&base_source).copied().ok_or(
        NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native sibling base-beam binding",
        },
    )?;
    let stem_vertex = bindings.stem_vertices.get(&stem_identity).copied().ok_or(
        NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native sibling stem binding",
        },
    )?;
    let group_outgoing = sig
        .outgoing_edges(group_vertex.0)
        .map_err(|_| NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native sibling group outgoing query",
        })?
        .into_iter()
        .map(graph_relation)
        .collect::<Vec<_>>();
    let group_members = group_outgoing
        .iter()
        .filter(|edge| edge.kind == NativeSigRelationKind::Containment)
        .map(|edge| {
            source_for_vertex(bindings, edge.target).ok_or(
                NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                    phase: "native sibling group member binding",
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !group_members.contains(&base_source)
        || drafts.iter().any(|draft| draft.source == base_source)
        || drafts.iter().enumerate().any(|(index, draft)| {
            !draft.grade.is_finite()
                || !draft.extension_point.x.is_finite()
                || !draft.extension_point.y.is_finite()
                || !group_members.contains(&draft.source)
                || drafts[..index]
                    .iter()
                    .any(|prior| prior.source == draft.source)
        })
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native sibling ordered member partition",
        });
    }

    let mut shadow = sig.clone();
    let mut steps = Vec::with_capacity(drafts.len());
    let mut appended_edges = Vec::new();
    for (draft_ordinal, draft) in drafts.iter().enumerate() {
        let sibling_ordinal = draft.sibling_ordinal;
        if sibling_ordinal < draft_ordinal
            || drafts[..draft_ordinal]
                .iter()
                .any(|prior| prior.sibling_ordinal >= sibling_ordinal)
        {
            return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                phase: "native sibling draft ordinal order",
            });
        }
        let sibling_vertex = bindings.beam_vertices.get(&draft.source).copied().ok_or(
            NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                phase: "native sibling beam binding",
            },
        )?;
        let source_outgoing_before = native_sibling_source_outgoing(&shadow, sibling_vertex)?
            .into_iter()
            .map(graph_relation)
            .collect::<Vec<_>>();
        let directed_pair_before = shadow
            .directed_edges(sibling_vertex.0, stem_vertex.0)
            .map_err(|_| NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                phase: "native sibling directed-pair query",
            })?
            .into_iter()
            .map(graph_relation)
            .collect::<Vec<_>>();
        let beam_abnormal_before = shadow
            .vertex(sibling_vertex.0)
            .expect("validated live sibling")
            .abnormal;
        let appended = if directed_pair_before
            .iter()
            .any(|edge| edge.kind == NativeSigRelationKind::BeamStem)
        {
            None
        } else {
            let edge = NativeSigEdge {
                ordinal: shadow.edges.len(),
                active: true,
                source: sibling_vertex.0,
                target: stem_vertex.0,
                kind: NativeSigRelationKind::BeamStem,
                origin: NativeSigRelationOrigin::BeamVSiblingDraft {
                    plan_ordinal,
                    sibling_ordinal,
                },
                support: Some(NativeSigSupport {
                    grade: draft.grade,
                    bar_connection_impacts: None,
                }),
                beam_portion: Some(draft.beam_portion),
                stem_extension: Some(draft.extension_point),
            };
            let projected = graph_relation(&edge);
            shadow.append_edge(edge).map_err(|_| {
                NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                    phase: "native sibling shadow append",
                }
            })?;
            appended_edges.push(projected.edge);
            Some(projected)
        };
        let beam_abnormal_after = native_beam_abnormal(&shadow, sibling_vertex)?;
        shadow
            .set_abnormal(sibling_vertex, beam_abnormal_after)
            .map_err(|_| NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                phase: "native sibling shadow abnormal update",
            })?;
        let stem_incident_after = shadow
            .incident_edges(stem_vertex.0)
            .map_err(|_| NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                phase: "native sibling stem incident query",
            })?
            .into_iter()
            .map(graph_relation)
            .collect();
        let beam_incident_after = shadow
            .incident_edges(sibling_vertex.0)
            .map_err(|_| NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                phase: "native sibling beam incident query",
            })?
            .into_iter()
            .map(graph_relation)
            .collect();
        steps.push(NativeStemsBeamSiblingGraphStep {
            sibling_ordinal,
            source: draft.source,
            sibling_vertex,
            source_outgoing_before,
            directed_pair_before,
            appended,
            stem_incident_after,
            beam_incident_after,
            beam_abnormal_before,
            beam_abnormal_after,
        });
    }
    shadow.validate_integrity().map_err(|_| {
        NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native sibling projected SIG integrity",
        }
    })?;
    Ok((
        NativeStemsBeamSiblingGraphProjection {
            system_id: sig.system_id,
            group_ordinal,
            group_vertex,
            base_source,
            base_vertex,
            stem_identity,
            stem_vertex,
            group_outgoing,
            group_members,
            steps,
            appended_edges,
        },
        shadow,
    ))
}

/// Read-only graph projection for B16. All serial mutations occur on a clone.
#[allow(clippy::too_many_arguments)]
pub fn project_native_stems_beam_vlink_sibling_graph(
    sig: &NativeSigSystem,
    bindings: &NativeSigSystemBindings,
    group_ordinal: usize,
    base_source: NativeStemsBeamSource,
    stem_identity: usize,
    plan_ordinal: usize,
    drafts: &[NativeStemsBeamSiblingGraphDraft],
) -> Result<NativeStemsBeamSiblingGraphProjection, NativeStemsBeamVLinkSiblingLinksError> {
    project_native_sibling_graph(
        sig,
        bindings,
        group_ordinal,
        base_source,
        stem_identity,
        plan_ordinal,
        drafts,
    )
    .map(|(projection, _)| projection)
}

/// Atomically commit the graph portion of B16 after deriving it natively.
#[allow(clippy::too_many_arguments)]
pub fn apply_native_stems_beam_vlink_sibling_graph_to_native_sig(
    sig: &mut NativeSigSystem,
    bindings: &NativeSigSystemBindings,
    group_ordinal: usize,
    base_source: NativeStemsBeamSource,
    stem_identity: usize,
    plan_ordinal: usize,
    drafts: &[NativeStemsBeamSiblingGraphDraft],
) -> Result<NativeStemsBeamSiblingGraphProjection, NativeStemsBeamVLinkSiblingLinksError> {
    let (projection, shadow) = project_native_sibling_graph(
        sig,
        bindings,
        group_ordinal,
        base_source,
        stem_identity,
        plan_ordinal,
        drafts,
    )?;
    *sig = shadow;
    Ok(projection)
}

/// Persistent native shared cell for one beam B-linker.  This is the field
/// observed by every V child of the B-linker; it deliberately carries no Java
/// object identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamNativeBLinkerCell {
    pub reference: NativeStemsBeamBLinkerRef,
    pub linked: bool,
    pub closed: bool,
}

/// Typed member state used instead of Java's opaque BeamGroup token hash.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamNativeGroupMemberState {
    pub source: NativeStemsBeamSource,
    pub vertex: NativeSigVertexId,
    pub abnormal: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamNativeSiblingTrace {
    pub sibling_ordinal: usize,
    pub source: NativeStemsBeamSource,
    pub branch: NativeStemsBeamSiblingBranch,
    pub geometry: Option<NativeStemsBeamSiblingGeometryTrace>,
    pub selected_b_linker: Option<NativeStemsBeamBLinkerRef>,
    pub linked_before: Option<bool>,
    pub linked_after: Option<bool>,
}

/// Native B15+B16 carrier delta.  The SIG and shared-cell catalogue are the
/// authoritative post-state; this value is a deterministic trace of the
/// serial work which produced them.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamNativeSiblingTransaction {
    pub system_id: usize,
    pub plan_ordinal: usize,
    pub stem_identity: usize,
    pub group_ordinal: usize,
    pub group_members: Vec<NativeStemsBeamSiblingGroupMemberTrace>,
    pub group_state_before: Vec<NativeStemsBeamNativeGroupMemberState>,
    pub group_state_after: Vec<NativeStemsBeamNativeGroupMemberState>,
    pub siblings: Vec<NativeStemsBeamNativeSiblingTrace>,
    pub graph: NativeStemsBeamSiblingGraphProjection,
    pub base_b_linker: NativeStemsBeamBLinkerRef,
    pub base_linked_before: bool,
    pub base_linked_after: bool,
    pub assigned_b_linkers: Vec<NativeStemsBeamBLinkerRef>,
    pub b_linker_write_count: usize,
    pub b_linker_value_change_count: usize,
}

/// Construct the complete shared B-cell arena before the first SIDES
/// transaction.  Reachability owns the exhaustive arena topology; all cells
/// are initially false/open in this still-pre-link state.
pub fn initialize_native_stems_beam_b_linker_cells(
    reachability: &NativeStemsBeamReachabilitySystem,
) -> Result<Vec<NativeStemsBeamNativeBLinkerCell>, NativeStemsBeamVLinkSiblingLinksError> {
    let mut cells = Vec::new();
    let mut seen = Vec::new();
    for arena in &reachability.final_beam_arenas {
        for entry in &arena.all_b_linkers {
            if entry.reference.beam != arena.beam
                || entry.reference.id == 0
                || seen.contains(&entry.reference)
            {
                return Err(NativeStemsBeamVLinkSiblingLinksError::Predecessor {
                    phase: "native shared B-cell arena",
                });
            }
            seen.push(entry.reference);
            cells.push(NativeStemsBeamNativeBLinkerCell {
                reference: entry.reference,
                linked: false,
                closed: false,
            });
        }
    }
    Ok(cells)
}

/// Resume from the verified B15 result, derive every real B16 input from
/// native products, and atomically commit both the SIG and shared B cells.
/// No Java alias, persistent Inter ID, fixture row, or opaque group hash is an
/// input to this path.
#[allow(clippy::too_many_arguments)]
pub fn apply_native_stems_beam_vlink_sibling_transaction_to_native_sig(
    sig: &mut NativeSigSystem,
    bindings: &NativeSigSystemBindings,
    scheduler_system: &NativeStemsBeamSchedulerSystem,
    stump_system: &NativeStemsBeamStumpSystem,
    vlinker_system: &NativeStemsBeamVLinkerSystem,
    reachability_system: &NativeStemsBeamReachabilitySystem,
    builder_system: &NativeStemsBeamBuilderSystem,
    base_apply_transaction: &NativeStemsBeamVLinkBaseApplyTransaction,
    b_linker_flag_transaction: &NativeStemsBeamVLinkBLinkerFlagTransaction,
    cells: &mut Vec<NativeStemsBeamNativeBLinkerCell>,
) -> Result<NativeStemsBeamNativeSiblingTransaction, NativeStemsBeamVLinkSiblingLinksError> {
    let frontier = match &scheduler_system.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => return Err(NativeStemsBeamVLinkSiblingLinksError::PredecessorNotReady),
    };
    let system_id = scheduler_system.system_id;
    if system_id != sig.system_id
        || system_id != bindings.system_id
        || system_id != stump_system.system_id
        || system_id != vlinker_system.system_id
        || system_id != reachability_system.system_id
        || system_id != builder_system.system_id
        || stump_system.interline != vlinker_system.interline
        || stump_system.interline != reachability_system.interline
        || stump_system.max_beam_side_dx != vlinker_system.max_beam_side_dx
        || stump_system.max_beam_side_dx != reachability_system.max_beam_side_dx
        || frontier.plan != b_linker_flag_transaction.key.plan
        || frontier.b_linker != b_linker_flag_transaction.target_b_linker
        || frontier.v_linker != b_linker_flag_transaction.triggering_v_linker
        || base_apply_transaction.key != b_linker_flag_transaction.key
        || !b_linker_flag_transaction.linked_after
        || b_linker_flag_transaction.stem_after.stem_identity
            != base_apply_transaction.stem_after.stem_identity
        || b_linker_flag_transaction
            .continuation_support_grade
            .to_bits()
            != base_apply_transaction.fresh_relation.grade.to_bits()
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::PredecessorMismatch);
    }
    sig.validate_integrity()
        .map_err(|_| NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native B16 SIG integrity",
        })?;
    bindings.validate_against(sig).map_err(|_| {
        NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native B16 bindings",
        }
    })?;
    validate_native_cell_catalogue(reachability_system, cells)?;

    let mut shadow_sig = sig.clone();
    let mut shadow_cells = cells.clone();
    let base_cell = native_cell_mut(&mut shadow_cells, b_linker_flag_transaction.target_b_linker)?;
    if base_cell.linked != b_linker_flag_transaction.linked_before {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native B15 shared-cell before join",
        });
    }
    let base_linked_before = base_cell.linked;
    base_cell.linked = b_linker_flag_transaction.linked_after;

    let (base_b, v_linker) = resolve_base_vlinker(vlinker_system, frontier.v_linker)?;
    let builder = builder_system
        .builders
        .get(frontier.plan.builder_ordinal)
        .ok_or(NativeStemsBeamVLinkSiblingLinksError::Predecessor {
            phase: "native B16 builder ordinal",
        })?;
    if builder.start != frontier.v_linker || builder.y_direction != v_linker.y_direction {
        return Err(NativeStemsBeamVLinkSiblingLinksError::PredecessorMismatch);
    }
    let base_beam = find_stump_beam(stump_system, frontier.beam)?;
    let stem = &b_linker_flag_transaction.stem_after;
    let stem_identity = stem.stem_identity;
    let stem_vertex = bindings.stem_vertices.get(&stem_identity).copied().ok_or(
        NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native B16 stem binding",
        },
    )?;
    let group_vertex = bindings
        .beam_group_vertices
        .get(&base_beam.group_ordinal)
        .copied()
        .ok_or(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native B16 group binding",
        })?;
    let group_sources = shadow_sig
        .outgoing_edges(group_vertex.0)
        .map_err(|_| NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native B16 group query",
        })?
        .into_iter()
        .filter(|edge| edge.kind == NativeSigRelationKind::Containment)
        .map(|edge| {
            source_for_vertex(bindings, NativeSigVertexId(edge.target)).ok_or(
                NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                    phase: "native B16 group member binding",
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    if reachability_system
        .groups_in_source_order
        .get(base_beam.group_ordinal)
        != Some(&group_sources)
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::PredecessorMismatch);
    }

    let vertical = Segment {
        x1: base_b.reference_point.x,
        y1: base_b.reference_point.y,
        x2: base_b.reference_point.x - (1_000.0 * reachability_system.global_slope),
        y2: base_b.reference_point.y + 1_000.0,
    };
    let mut group_members = Vec::with_capacity(group_sources.len());
    for (member_ordinal, source) in group_sources.iter().copied().enumerate() {
        let beam = find_stump_beam(stump_system, source)?;
        let cross = generic_intersection(vertical, beam.median);
        let left_limit = beam.median.x1 - f64::from(vlinker_system.max_beam_side_dx);
        let right_limit = beam.median.x2 + f64::from(vlinker_system.max_beam_side_dx);
        group_members.push(NativeStemsBeamSiblingGroupMemberTrace {
            member_ordinal,
            source,
            cross,
            left_limit,
            right_limit,
            selected: left_limit <= cross.x && cross.x <= right_limit,
            sorted_ordinal: None,
            removed_as_base: false,
        });
    }
    let mut selected = group_members
        .iter()
        .enumerate()
        .filter_map(|(index, member)| member.selected.then_some(index))
        .collect::<Vec<_>>();
    selected.sort_by(|left, right| {
        group_members[*left]
            .cross
            .y
            .total_cmp(&group_members[*right].cross.y)
    });
    for (ordinal, index) in selected.iter().copied().enumerate() {
        group_members[index].sorted_ordinal = Some(ordinal);
    }
    let base_index = selected
        .iter()
        .position(|index| group_members[*index].source == frontier.beam)
        .ok_or(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native B16 selected base member",
        })?;
    group_members[selected[base_index]].removed_as_base = true;
    selected.remove(base_index);
    let sibling_sources = selected
        .iter()
        .map(|index| group_members[*index].source)
        .collect::<Vec<_>>();
    validate_native_reachability_siblings(reachability_system, frontier.v_linker, &group_members)?;

    let group_state_before = native_group_state(&shadow_sig, bindings, &group_sources)?;
    let stem_median = stem_segment(stem);
    let base_cross = generic_intersection(stem_median, base_beam.median);
    let portion_maximum_dx = java_rint_i32(f64::from(stump_system.interline) * 0.5);
    let mut traces = Vec::with_capacity(sibling_sources.len());
    let mut linked_work = Vec::new();
    let mut linked_cells = Vec::new();
    for (sibling_ordinal, source) in sibling_sources.iter().copied().enumerate() {
        let sibling_beam = find_stump_beam(stump_system, source)?;
        let sibling_vertex = bindings.beam_vertices.get(&source).copied().ok_or(
            NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                phase: "native B16 sibling binding",
            },
        )?;
        if sibling_beam.beam_glyph == base_beam.beam_glyph {
            traces.push(NativeStemsBeamNativeSiblingTrace {
                sibling_ordinal,
                source,
                branch: NativeStemsBeamSiblingBranch::SameGlyph,
                geometry: None,
                selected_b_linker: None,
                linked_before: None,
                linked_after: None,
            });
            continue;
        }
        let existing = shadow_sig
            .directed_edges(sibling_vertex.0, stem_vertex.0)
            .map_err(|_| NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                phase: "native B16 directed pair",
            })?
            .into_iter()
            .any(|edge| edge.kind == NativeSigRelationKind::BeamStem);
        if existing {
            traces.push(NativeStemsBeamNativeSiblingTrace {
                sibling_ordinal,
                source,
                branch: NativeStemsBeamSiblingBranch::ExistingBeamStem,
                geometry: None,
                selected_b_linker: None,
                linked_before: None,
                linked_after: None,
            });
            continue;
        }
        let geometry = sibling_geometry_values(
            stem_median,
            base_beam.median,
            sibling_beam.median,
            sibling_beam.height,
            base_cross,
            v_linker.y_direction,
            portion_maximum_dx,
            MAX_SHORTER_RATIO,
            b_linker_flag_transaction.continuation_support_grade,
        );
        if geometry.wrong_side == Some(true) {
            traces.push(NativeStemsBeamNativeSiblingTrace {
                sibling_ordinal,
                source,
                branch: NativeStemsBeamSiblingBranch::ShorterWrongSide,
                geometry: Some(geometry),
                selected_b_linker: None,
                linked_before: None,
                linked_after: None,
            });
            continue;
        }
        let draft = NativeStemsBeamSiblingGraphDraft {
            sibling_ordinal,
            source,
            grade: b_linker_flag_transaction.continuation_support_grade,
            beam_portion: geometry.beam_portion.ok_or(
                NativeStemsBeamVLinkSiblingLinksError::DefensiveCommitInvariant {
                    phase: "native B16 linked portion",
                },
            )?,
            extension_point: geometry.extension_point.ok_or(
                NativeStemsBeamVLinkSiblingLinksError::DefensiveCommitInvariant {
                    phase: "native B16 linked extension",
                },
            )?,
        };
        linked_work.push((traces.len(), draft));
        traces.push(NativeStemsBeamNativeSiblingTrace {
            sibling_ordinal,
            source,
            branch: NativeStemsBeamSiblingBranch::Linked,
            geometry: Some(geometry),
            selected_b_linker: None,
            linked_before: None,
            linked_after: None,
        });
    }

    // Start with the read-only group projection, then commit each Linked
    // sibling separately.  This preserves Java's edge/callback -> immutable
    // builder lookup -> shared-cell write order before the next sibling.
    let mut graph = apply_native_stems_beam_vlink_sibling_graph_to_native_sig(
        &mut shadow_sig,
        bindings,
        base_beam.group_ordinal,
        frontier.beam,
        stem_identity,
        frontier.plan.plan_ordinal,
        &[],
    )?;
    for (trace_index, draft) in linked_work {
        let projected = apply_native_stems_beam_vlink_sibling_graph_to_native_sig(
            &mut shadow_sig,
            bindings,
            base_beam.group_ordinal,
            frontier.beam,
            stem_identity,
            frontier.plan.plan_ordinal,
            std::slice::from_ref(&draft),
        )?;
        if projected.group_vertex != graph.group_vertex
            || projected.base_vertex != graph.base_vertex
            || projected.stem_vertex != graph.stem_vertex
            || projected.group_outgoing != graph.group_outgoing
            || projected.group_members != graph.group_members
            || projected.steps.len() != 1
            || projected.appended_edges.len() != 1
            || projected.steps[0].sibling_ordinal != draft.sibling_ordinal
            || projected.steps[0].source != draft.source
        {
            return Err(
                NativeStemsBeamVLinkSiblingLinksError::DefensiveCommitInvariant {
                    phase: "native B16 graph/branch join",
                },
            );
        }
        graph.steps.extend(projected.steps);
        graph.appended_edges.extend(projected.appended_edges);

        let selected_b_linker = native_builder_b_linker(builder, draft.source);
        if let Some(reference) = selected_b_linker {
            let cell = native_cell_mut(&mut shadow_cells, reference)?;
            let before = cell.linked;
            cell.linked = true;
            linked_cells.push((reference, before));
            traces[trace_index].linked_before = Some(before);
            traces[trace_index].linked_after = Some(true);
        }
        traces[trace_index].selected_b_linker = selected_b_linker;
    }
    let group_state_after = native_group_state(&shadow_sig, bindings, &group_sources)?;
    let assigned_b_linkers = linked_cells
        .iter()
        .map(|(reference, _)| *reference)
        .collect::<Vec<_>>();
    let b_linker_value_change_count = usize::from(!base_linked_before)
        + linked_cells.iter().filter(|(_, before)| !*before).count();
    let transaction = NativeStemsBeamNativeSiblingTransaction {
        system_id,
        plan_ordinal: frontier.plan.plan_ordinal,
        stem_identity,
        group_ordinal: base_beam.group_ordinal,
        group_members,
        group_state_before,
        group_state_after,
        siblings: traces,
        graph,
        base_b_linker: b_linker_flag_transaction.target_b_linker,
        base_linked_before,
        base_linked_after: true,
        assigned_b_linkers,
        b_linker_write_count: 1 + linked_cells.len(),
        b_linker_value_change_count,
    };
    *sig = shadow_sig;
    *cells = shadow_cells;
    Ok(transaction)
}

fn validate_native_cell_catalogue(
    reachability: &NativeStemsBeamReachabilitySystem,
    cells: &[NativeStemsBeamNativeBLinkerCell],
) -> Result<(), NativeStemsBeamVLinkSiblingLinksError> {
    let expected = reachability
        .final_beam_arenas
        .iter()
        .flat_map(|arena| arena.all_b_linkers.iter().map(|entry| entry.reference))
        .collect::<Vec<_>>();
    let actual = cells.iter().map(|cell| cell.reference).collect::<Vec<_>>();
    if actual != expected
        || actual
            .iter()
            .enumerate()
            .any(|(index, reference)| actual[..index].contains(reference))
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native shared B-cell catalogue",
        });
    }
    Ok(())
}

fn native_cell_mut(
    cells: &mut [NativeStemsBeamNativeBLinkerCell],
    reference: NativeStemsBeamBLinkerRef,
) -> Result<&mut NativeStemsBeamNativeBLinkerCell, NativeStemsBeamVLinkSiblingLinksError> {
    let mut matches = cells.iter_mut().filter(|cell| cell.reference == reference);
    let cell = matches
        .next()
        .ok_or(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native selected B-cell",
        })?;
    if matches.next().is_some() {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "native duplicate selected B-cell",
        });
    }
    Ok(cell)
}

fn native_group_state(
    sig: &NativeSigSystem,
    bindings: &NativeSigSystemBindings,
    sources: &[NativeStemsBeamSource],
) -> Result<Vec<NativeStemsBeamNativeGroupMemberState>, NativeStemsBeamVLinkSiblingLinksError> {
    sources
        .iter()
        .map(|source| {
            let vertex = bindings.beam_vertices.get(source).copied().ok_or(
                NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                    phase: "native group-state beam binding",
                },
            )?;
            let abnormal = sig.vertex(vertex.0).map(|beam| beam.abnormal).ok_or(
                NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                    phase: "native group-state live beam",
                },
            )?;
            Ok(NativeStemsBeamNativeGroupMemberState {
                source: *source,
                vertex,
                abnormal,
            })
        })
        .collect()
}

fn validate_native_reachability_siblings(
    reachability: &NativeStemsBeamReachabilitySystem,
    reference: crate::native_stems_beam_vlinkers::NativeStemsBeamVLinkerRef,
    members: &[NativeStemsBeamSiblingGroupMemberTrace],
) -> Result<(), NativeStemsBeamVLinkSiblingLinksError> {
    let inspection = reachability
        .beam_inspections
        .iter()
        .flat_map(|beam| &beam.b_visits)
        .flat_map(|visit| &visit.v_inspections)
        .find(|inspection| inspection.reference == reference)
        .ok_or(NativeStemsBeamVLinkSiblingLinksError::Predecessor {
            phase: "native B16 reachability inspection",
        })?;
    let mut selected = members
        .iter()
        .filter(|member| member.selected)
        .collect::<Vec<_>>();
    selected.sort_by_key(|member| member.sorted_ordinal);
    if inspection.siblings.len() != selected.len()
        || inspection
            .siblings
            .iter()
            .zip(selected)
            .any(|(prior, now)| {
                prior.beam != now.source || !point_bits_equal(prior.cross, now.cross)
            })
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::PredecessorMismatch);
    }
    Ok(())
}

fn native_builder_b_linker(
    builder: &crate::native_stems_beam_builders::NativeStemsBeamBuilder,
    source: NativeStemsBeamSource,
) -> Option<NativeStemsBeamBLinkerRef> {
    builder.items.iter().find_map(|item| match item.target {
        Some(NativeStemsBeamBuilderTargetRef::Beam(reference))
            if item.kind == NativeStemsBeamBuilderItemKind::BeamLinker
                && reference.beam == source =>
        {
            Some(reference)
        }
        _ => None,
    })
}

/// Stable Java glyph-object evidence. `None` means Java `null`; equal values
/// mean the exact same object, which is stronger than equal fixed content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamSiblingGlyphIdentity {
    pub object_identity: usize,
    pub token: String,
}

/// A live beam member reachable from the selected beam group's outgoing
/// containment scan.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingLiveBeam {
    pub source: NativeStemsBeamSource,
    pub alias: String,
    pub runtime: NativeStemsBeamVLinkBeamRuntimeState,
    /// Exact exhaustive `InterIndex` lookup for this Java beam object.
    pub inter_index_ordinal: usize,
    pub inter_index_object_matches: usize,
    pub inter_index_id_matches: usize,
    pub glyph: Option<NativeStemsBeamSiblingGlyphIdentity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamSiblingGroupTarget {
    Beam(NativeStemsBeamSource),
    OtherInter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamSiblingGroupTargetEvidence {
    GetMembersRead,
    GraphReconstruction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamSiblingGroupRelation {
    pub outgoing_ordinal: usize,
    pub graph_relation_identity: usize,
    pub relation_object_identity: NativeStemsBeamSiblingRelationObjectIdentity,
    pub relation_class: String,
    pub containment_match: bool,
    pub target: NativeStemsBeamSiblingGroupTarget,
    pub target_read_by_get_members: bool,
    pub target_evidence: NativeStemsBeamSiblingGroupTargetEvidence,
    pub target_alias: String,
    pub target_class: String,
    pub target_inter_id: i32,
    pub target_vertex_identity: usize,
    pub member_ordinal: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingGroupMemberTrace {
    pub member_ordinal: usize,
    pub source: NativeStemsBeamSource,
    pub cross: NativeStemPoint,
    pub left_limit: f64,
    pub right_limit: f64,
    pub selected: bool,
    pub sorted_ordinal: Option<usize>,
    pub removed_as_base: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingGroupScan {
    pub query_relation_count: usize,
    pub query_provenance_sha256: String,
    pub relations: Vec<NativeStemsBeamSiblingGroupRelation>,
    pub members: Vec<NativeStemsBeamSiblingGroupMemberTrace>,
}

/// Exact live `BeamGroupInter` object used by `beamGroup.getMembers()`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingGroupRuntimeState {
    pub alias: String,
    pub runtime_class: String,
    pub inter_id: i32,
    pub sig_vertex_identity: usize,
    pub removed: bool,
    pub vip: bool,
    pub abnormal: bool,
    /// Baseline hash over the group object plus its members. A sibling
    /// callback may change a member's abnormal flag, so this is deliberately
    /// retained as before-state evidence rather than mislabeled current state.
    pub member_state_sha256_before: String,
    /// Current member-inclusive group hash. It equals the before hash at
    /// entry and advances to the exact Java-observed after hash on commit.
    pub member_state_sha256: String,
    /// Hash of the group object alone. The sibling seam leaves it unchanged.
    pub object_state_sha256: String,
}

/// Relation-object identity is deliberately separate from dense/global graph
/// insertion identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamSiblingRelationObjectIdentity {
    GraphObject(usize),
    BaseDraft(usize),
    SiblingDraft {
        plan_ordinal: usize,
        sibling_ordinal: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamSiblingPairClassRead {
    ExaminedContinue,
    ExaminedMatchBreak,
    UnreadAfterBreak,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamSiblingPairRelation {
    pub pair_ordinal: usize,
    pub source_outgoing_ordinal: usize,
    pub graph_relation_identity: usize,
    pub relation_object_identity: NativeStemsBeamSiblingRelationObjectIdentity,
    pub relation_class: String,
    pub kind: NativeStemsBeamQueryRelationKind,
    pub class_read: NativeStemsBeamSiblingPairClassRead,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamSiblingSourceOutgoingRelation {
    pub source_outgoing_ordinal: usize,
    pub graph_relation_identity: usize,
    pub relation_object_identity: NativeStemsBeamSiblingRelationObjectIdentity,
    pub relation_class: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamSiblingQueryProvenance {
    NotRead,
    ExhaustiveSha256(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamSiblingPairScan {
    pub source_outgoing_scanned: usize,
    pub source_outgoing_provenance: NativeStemsBeamSiblingQueryProvenance,
    pub source_outgoing_relations: Vec<NativeStemsBeamSiblingSourceOutgoingRelation>,
    pub query_relation_count: usize,
    pub pair_provenance: NativeStemsBeamSiblingQueryProvenance,
    pub relations: Vec<NativeStemsBeamSiblingPairRelation>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingStemIncidentRelation {
    pub incident_ordinal: usize,
    pub direction: NativeStemsBeamIncidentDirection,
    pub direction_ordinal: usize,
    pub graph_relation_identity: usize,
    pub relation_object_identity: NativeStemsBeamSiblingRelationObjectIdentity,
    pub relation_class: String,
    /// Stem callback rows expose only runtime class / `ChordStem` matching;
    /// Beam portion is not read on this scan.
    pub kind: NativeStemsBeamQueryRelationKind,
    pub opposite_vertex_identity: usize,
    pub opposite: NativeStemsBeamIncidentOpposite,
    pub opposite_alias: String,
    pub opposite_inter_id: i32,
    pub chord_stem_match: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingStemIncidentScan {
    pub query_relation_count: usize,
    pub query_provenance_sha256: String,
    pub relations: Vec<NativeStemsBeamSiblingStemIncidentRelation>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingBeamIncidentRelation {
    pub incident_ordinal: usize,
    pub direction: NativeStemsBeamIncidentDirection,
    pub direction_ordinal: usize,
    pub graph_relation_identity: usize,
    pub relation_object_identity: NativeStemsBeamSiblingRelationObjectIdentity,
    pub relation_class: String,
    pub kind: NativeStemsBeamSigRelationKind,
    pub opposite_vertex_identity: usize,
    pub opposite: NativeStemsBeamIncidentOpposite,
    pub opposite_alias: String,
    pub opposite_inter_id: i32,
    pub read: NativeStemsBeamBeamIncidentRead,
    pub relevant: bool,
    pub beam_portion: Option<NativeBeamPortion>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingBeamIncidentScan {
    pub rule: NativeStemsBeamBeamIncidentRule,
    pub query_relation_count: usize,
    pub query_provenance_sha256: String,
    pub relations: Vec<NativeStemsBeamSiblingBeamIncidentRelation>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingStepCertificate {
    pub sibling_ordinal: usize,
    pub source: NativeStemsBeamSource,
    pub directed_pair: NativeStemsBeamSiblingPairScan,
    /// Read only after a fresh edge has been structurally installed.
    pub stem_incident_after: Option<NativeStemsBeamSiblingStemIncidentScan>,
    /// Read only by the synchronous BeamStem callback.
    pub beam_incident_after: Option<NativeStemsBeamSiblingBeamIncidentScan>,
    pub chord_stem_matches: usize,
    /// Java reads `StemBuilder.items` only after a fresh sibling edge and its
    /// synchronous callback have completed.
    pub builder_lookup: Option<NativeStemsBeamSiblingBuilderLookupScan>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkSiblingLinksCertificate {
    pub system_id: usize,
    pub headless: bool,
    pub listener_topology: NativeStemsBeamSigListenerTopology,
    pub interline: i32,
    pub x_in_gap_maximum_profile0: f64,
    pub portion_maximum_dx: i32,
    pub max_beam_side_dx: i32,
    pub max_shorter_ratio: f64,
    pub base_glyph: Option<NativeStemsBeamSiblingGlyphIdentity>,
    pub group_scan: NativeStemsBeamSiblingGroupScan,
    /// Opaque post-state digest emitted by the Java run. Typed member abnormal
    /// transitions are validated independently; the exact gate binds this
    /// digest because the compact state intentionally omits unrelated Inter
    /// structural fields needed to recompute Java's complete token locally.
    pub expected_group_member_state_sha256_after: String,
    pub steps: Vec<NativeStemsBeamSiblingStepCertificate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamSiblingBLinkerCell {
    pub reference: NativeStemsBeamBLinkerRef,
    pub linked: bool,
    pub closed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingAppendedRelation {
    pub graph_relation_identity: usize,
    pub relation_object_identity: NativeStemsBeamSiblingRelationObjectIdentity,
    pub source: NativeStemsBeamSource,
    pub source_vertex_identity: usize,
    pub target_stem_identity: usize,
    pub target_vertex_identity: usize,
    pub extension_point: NativeStemPoint,
    pub beam_portion: NativeBeamPortion,
    pub grade: f64,
}

/// Mutable state for exactly one serial `linkSiblings` call.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkSiblingLinksState {
    /// Exact state immediately before boundary 15. Boundary 16 reruns both
    /// boundary 14 and boundary 15 from this value before trusting inputs.
    pub b_linker_flag_state_before: NativeStemsBeamVLinkBLinkerFlagState,
    /// Exact committed boundary-15 state. It is independently reproduced
    /// from `b_linker_flag_state_before` before any sibling mutation and then
    /// retained unchanged in this boundary's `state_after`.
    pub b_linker_flag_state_after: NativeStemsBeamVLinkBLinkerFlagState,
    /// Exact boundary-14 state at entry; retained byte-for-byte while sibling
    /// mutations live in the compact overlays below.
    pub base_apply_state_after: NativeStemsBeamVLinkBaseApplyState,
    /// The outer `BeamLinker` cached this exact median object at construction.
    pub cached_base_median: Segment,
    pub cached_base_median_same_identity: bool,
    pub group_runtime: NativeStemsBeamSiblingGroupRuntimeState,
    pub base_glyph: Option<NativeStemsBeamSiblingGlyphIdentity>,
    pub stem_alias: String,
    pub live_group_members: Vec<NativeStemsBeamSiblingLiveBeam>,
    pub sibling_b_linker_cells: Vec<NativeStemsBeamSiblingBLinkerCell>,
    pub appended_relations: Vec<NativeStemsBeamSiblingAppendedRelation>,
    pub sheet_edit: NativeStemsBeamSheetEditState,
    pub certificate: Option<NativeStemsBeamVLinkSiblingLinksCertificate>,
    pub committed: Option<NativeStemsBeamVLinkBaseApplyKey>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamSiblingBranch {
    SameGlyph,
    ExistingBeamStem,
    ShorterWrongSide,
    Linked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamSiblingBuilderLinkerIdentity {
    StartVLinker,
    BeamBLinker(NativeStemsBeamBLinkerRef),
    HeadCLinker,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamSiblingBuilderItemRead {
    ExaminedContinue,
    ExaminedSelectBreak,
    UnreadAfterBreak,
    NotALinker,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamSiblingBuilderLinkerRead {
    NotRead,
    NotLinkerItem,
    ReadLinker,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamSiblingBuilderSourceRead {
    NotRead,
    ReadSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamSiblingBuilderAction {
    Continue,
    SelectBreak,
    UnreadAfterBreak,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamSiblingBuilderLookupState {
    FirstSourceIdentityMatch,
    ExhaustiveNoMatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamSiblingBuilderLookupTiming {
    ReconstructedFromImmutableItems,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingBuilderLookupRow {
    pub item_ordinal: usize,
    pub item_kind: NativeStemsBeamBuilderItemKind,
    pub linker: Option<NativeStemsBeamSiblingBuilderLinkerIdentity>,
    pub source_beam: Option<NativeStemsBeamSource>,
    pub read: NativeStemsBeamSiblingBuilderItemRead,
    pub runtime_class: Option<String>,
    pub linker_read: NativeStemsBeamSiblingBuilderLinkerRead,
    pub source_read: NativeStemsBeamSiblingBuilderSourceRead,
    pub linker_alias: Option<String>,
    pub linker_runtime_class: Option<String>,
    pub source_alias: Option<String>,
    pub source_inter_id: Option<i32>,
    pub identity_match: Option<bool>,
    pub action: NativeStemsBeamSiblingBuilderAction,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingBuilderLookupScan {
    pub state: NativeStemsBeamSiblingBuilderLookupState,
    pub timing: NativeStemsBeamSiblingBuilderLookupTiming,
    pub query_item_count: usize,
    pub query_provenance_sha256: String,
    pub rows: Vec<NativeStemsBeamSiblingBuilderLookupRow>,
    pub selected_b_linker: Option<NativeStemsBeamBLinkerRef>,
    pub selected_alias: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingGeometryTrace {
    pub base_cross: NativeStemPoint,
    pub sibling_cross: NativeStemPoint,
    pub base_length: f64,
    pub sibling_length: f64,
    pub length_ratio: f64,
    pub shorter_or_equal: bool,
    /// Java reads these only inside the inclusive `ratio <= 0.8` branch.
    pub delta_y: Option<f64>,
    pub directed_delta_y: Option<f64>,
    pub wrong_side: Option<bool>,
    /// Java reaches these only after the shorter/wrong-side rejection.
    pub extension_point: Option<NativeStemPoint>,
    pub portion_maximum_dx: Option<i32>,
    pub left_threshold: Option<f64>,
    pub right_threshold: Option<f64>,
    pub beam_portion: Option<NativeBeamPortion>,
    pub support_grade: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamSiblingBeamAbnormalTrace {
    NotRead,
    HookAnyBeamStem {
        incident_relation_count: usize,
        relations_read: usize,
        before: bool,
        after: bool,
    },
    RawBeamSides {
        incident_relation_count: usize,
        left_found: bool,
        right_found: bool,
        before: bool,
        after: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamVLinkSiblingLinksOperation {
    SigGlobalRelationInserted {
        sibling_ordinal: usize,
        graph_relation_identity: usize,
    },
    BeamOutgoingRelationInserted {
        sibling_ordinal: usize,
        graph_relation_identity: usize,
    },
    StemIncomingRelationInserted {
        sibling_ordinal: usize,
        graph_relation_identity: usize,
    },
    SigEdgeEventDispatched {
        sibling_ordinal: usize,
        graph_relation_identity: usize,
    },
    StandardSigListenerEdgeCallbackStarted {
        sibling_ordinal: usize,
    },
    BeamStemRelationCallbackStarted {
        sibling_ordinal: usize,
    },
    StemChordIncidentScanCompleted {
        sibling_ordinal: usize,
        incident_relation_count: usize,
        chord_stem_matches: usize,
    },
    BeamAbnormalSet {
        sibling_ordinal: usize,
        before: bool,
        after: bool,
    },
    SheetStubModifiedSetTrue {
        sibling_ordinal: usize,
    },
    BookModifiedSetTrue {
        sibling_ordinal: usize,
    },
    BookDirtySetTrue {
        sibling_ordinal: usize,
    },
    BeamStemRelationCallbackCompleted {
        sibling_ordinal: usize,
    },
    StandardSigListenerEdgeCallbackCompleted {
        sibling_ordinal: usize,
    },
    BLinkerLinkedAssigned {
        sibling_ordinal: usize,
        target: NativeStemsBeamBLinkerRef,
        /// Exact TOP then BOTTOM children observing the same parent field.
        /// Dynamic anchor B-linkers legitimately have no V children.
        ordered_observer_v_linkers:
            Vec<crate::native_stems_beam_vlinkers::NativeStemsBeamVLinkerRef>,
        before: bool,
        after: bool,
        closed_before: bool,
        closed_after: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkSiblingTrace {
    pub sibling_ordinal: usize,
    pub source: NativeStemsBeamSource,
    pub branch: NativeStemsBeamSiblingBranch,
    pub same_glyph_identity: bool,
    pub directed_pair_relations_read: usize,
    pub geometry: Option<NativeStemsBeamSiblingGeometryTrace>,
    pub relation: Option<NativeStemsBeamSiblingAppendedRelation>,
    pub stem_incident_graph_relation_identities: Vec<usize>,
    pub beam_abnormal: NativeStemsBeamSiblingBeamAbnormalTrace,
    pub builder_lookup: Option<NativeStemsBeamSiblingBuilderLookupScan>,
    pub selected_b_linker: Option<NativeStemsBeamBLinkerRef>,
    pub linked_before: Option<bool>,
    pub linked_after: Option<bool>,
    pub closed_before: Option<bool>,
    pub closed_after: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamVLinkSiblingLinksOutcome {
    ReadyBeforeHeadRelationLoop {
        stem_identity: usize,
        continuation_support_grade: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkSiblingLinksTransaction {
    pub key: NativeStemsBeamVLinkBaseApplyKey,
    pub stem_after: NativeStemsBeamKnownSystemStem,
    pub continuation_support_grade: f64,
    pub base_beam: NativeStemsBeamSource,
    pub cached_base_median: Segment,
    pub cached_base_median_same_identity: bool,
    pub group_runtime: NativeStemsBeamSiblingGroupRuntimeState,
    pub group_member_state_sha256_before: String,
    pub group_member_state_sha256_after: String,
    /// Exact one-shot evidence consumed by this commit. State clears its copy,
    /// but the returned trace retains every lazy query and callback row.
    pub consumed_certificate: NativeStemsBeamVLinkSiblingLinksCertificate,
    /// Java computes these once before entering the sibling loop, even when
    /// every sibling later takes an early branch.
    pub base_cross: NativeStemPoint,
    pub base_length: f64,
    pub group_members: Vec<NativeStemsBeamSiblingGroupMemberTrace>,
    pub sibling_sources: Vec<NativeStemsBeamSource>,
    pub siblings: Vec<NativeStemsBeamVLinkSiblingTrace>,
    pub operations: Vec<NativeStemsBeamVLinkSiblingLinksOperation>,
    pub appended_graph_relation_identities: Vec<usize>,
    pub assigned_b_linkers: Vec<NativeStemsBeamBLinkerRef>,
    pub sig_relation_mutation_count: usize,
    pub beam_abnormal_mutation_count: usize,
    pub sheet_edit_mutation_count: usize,
    pub b_linker_write_count: usize,
    pub b_linker_value_change_count: usize,
    pub sibling_link_mutation_count: usize,
    pub head_link_mutation_count: usize,
    pub outcome: NativeStemsBeamVLinkSiblingLinksOutcome,
    pub state_after: Box<NativeStemsBeamVLinkSiblingLinksState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamVLinkSiblingLinksError {
    Predecessor { phase: &'static str },
    PredecessorMismatch,
    PredecessorNotReady,
    InvalidState { phase: &'static str },
    InvalidEvidence { phase: &'static str },
    UnsupportedV1 { phase: &'static str },
    DefensiveCommitInvariant { phase: &'static str },
}

impl fmt::Display for NativeStemsBeamVLinkSiblingLinksError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid beam VLink sibling-links boundary: {self:?}"
        )
    }
}

impl Error for NativeStemsBeamVLinkSiblingLinksError {}

/// Execute the whole serial `linkSiblings` call and stop before Java reads the
/// first head relation. Every public certificate is consumed on a cloned
/// state; malformed evidence therefore cannot commit a partial prefix.
#[allow(clippy::too_many_arguments)]
pub fn apply_native_stems_beam_vlink_sibling_links_transaction(
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
    state: &mut NativeStemsBeamVLinkSiblingLinksState,
) -> Result<NativeStemsBeamVLinkSiblingLinksTransaction, NativeStemsBeamVLinkSiblingLinksError> {
    if state.committed.is_some() {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "one-shot transaction already committed",
        });
    }

    let mut flag_replay_state = state.b_linker_flag_state_before.clone();
    let replayed_flag = apply_native_stems_beam_vlink_b_linker_flag_transaction(
        scheduler_system,
        plan_system,
        stump_system,
        vlinker_system,
        create_transaction,
        reuse_live_state,
        relation_parameters,
        reuse_check,
        base_apply_transaction,
        &mut flag_replay_state,
    )
    .map_err(|_| NativeStemsBeamVLinkSiblingLinksError::Predecessor {
        phase: "boundary-15 exact replay",
    })?;
    if &replayed_flag != b_linker_flag_transaction
        || &flag_replay_state != b_linker_flag_transaction.state_after.as_ref()
        || replayed_flag.state_after.as_ref() != &flag_replay_state
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::PredecessorMismatch);
    }
    if state.b_linker_flag_state_after != flag_replay_state {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "boundary-15 committed state join",
        });
    }
    let (stem_identity, support_grade) = match replayed_flag.outcome {
        NativeStemsBeamVLinkBLinkerFlagOutcome::ReadyBeforeSiblingBeamLinks {
            stem_identity,
            continuation_support_grade,
        } => (stem_identity, continuation_support_grade),
    };
    if !support_grade.is_finite()
        || support_grade.to_bits() != replayed_flag.continuation_support_grade.to_bits()
        || support_grade.to_bits() != base_apply_transaction.fresh_relation.grade.to_bits()
        || replayed_flag.sibling_link_mutation_count != 0
        || replayed_flag.head_link_mutation_count != 0
        || replayed_flag.stem_after.stem_identity != stem_identity
        || !replayed_flag.linked_after
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::PredecessorNotReady);
    }

    let base_after = base_apply_transaction.state_after.as_ref();
    if &state.base_apply_state_after != base_after
        || state.sheet_edit != base_after.sheet_edit
        || state.base_apply_state_after.certificate.is_some()
        || state.base_apply_state_after.committed != Some(replayed_flag.key)
        || !state.appended_relations.is_empty()
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "boundary-14 state join",
        });
    }

    let frontier = match &scheduler_system.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => return Err(NativeStemsBeamVLinkSiblingLinksError::PredecessorNotReady),
    };
    if scheduler_system.system_id != stump_system.system_id
        || scheduler_system.system_id != vlinker_system.system_id
        || scheduler_system.system_id != reachability_system.system_id
        || scheduler_system.system_id != builder_system.system_id
        || frontier.plan != replayed_flag.key.plan
        || frontier.beam != replayed_flag.target_b_linker.beam
        || frontier.v_linker != replayed_flag.triggering_v_linker
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::Predecessor {
            phase: "system/frontier products",
        });
    }

    let mut shadow = state.clone();
    let certificate = shadow.certificate.clone().ok_or(
        NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "missing one-shot certificate",
        },
    )?;
    let prepared = prepare_and_commit_supported(
        frontier,
        stump_system,
        vlinker_system,
        reachability_system,
        builder_system,
        &replayed_flag,
        base_apply_transaction,
        &certificate,
        &mut shadow,
    )?;
    shadow.certificate = None;
    shadow.committed = Some(replayed_flag.key);

    let transaction = NativeStemsBeamVLinkSiblingLinksTransaction {
        key: replayed_flag.key,
        stem_after: replayed_flag.stem_after.clone(),
        continuation_support_grade: support_grade,
        base_beam: frontier.beam,
        cached_base_median: shadow.cached_base_median,
        cached_base_median_same_identity: shadow.cached_base_median_same_identity,
        group_runtime: shadow.group_runtime.clone(),
        group_member_state_sha256_before: prepared.group_member_state_sha256_before,
        group_member_state_sha256_after: prepared.group_member_state_sha256_after,
        consumed_certificate: certificate,
        base_cross: prepared.base_cross,
        base_length: prepared.base_length,
        group_members: prepared.group_members,
        sibling_sources: prepared.sibling_sources,
        siblings: prepared.siblings,
        operations: prepared.operations,
        appended_graph_relation_identities: prepared.appended_graph_relation_identities,
        assigned_b_linkers: prepared.assigned_b_linkers,
        sig_relation_mutation_count: prepared.sig_relation_mutation_count,
        beam_abnormal_mutation_count: prepared.beam_abnormal_mutation_count,
        sheet_edit_mutation_count: prepared.sheet_edit_mutation_count,
        b_linker_write_count: prepared.b_linker_write_count,
        b_linker_value_change_count: prepared.b_linker_value_change_count,
        sibling_link_mutation_count: prepared.sig_relation_mutation_count,
        head_link_mutation_count: 0,
        outcome: NativeStemsBeamVLinkSiblingLinksOutcome::ReadyBeforeHeadRelationLoop {
            stem_identity,
            continuation_support_grade: support_grade,
        },
        state_after: Box::new(shadow.clone()),
    };
    *state = shadow;
    Ok(transaction)
}

struct PreparedSiblingLinks {
    base_cross: NativeStemPoint,
    base_length: f64,
    group_members: Vec<NativeStemsBeamSiblingGroupMemberTrace>,
    sibling_sources: Vec<NativeStemsBeamSource>,
    siblings: Vec<NativeStemsBeamVLinkSiblingTrace>,
    operations: Vec<NativeStemsBeamVLinkSiblingLinksOperation>,
    appended_graph_relation_identities: Vec<usize>,
    assigned_b_linkers: Vec<NativeStemsBeamBLinkerRef>,
    sig_relation_mutation_count: usize,
    beam_abnormal_mutation_count: usize,
    sheet_edit_mutation_count: usize,
    b_linker_write_count: usize,
    b_linker_value_change_count: usize,
    group_member_state_sha256_before: String,
    group_member_state_sha256_after: String,
}

#[allow(clippy::too_many_arguments)]
fn prepare_and_commit_supported(
    frontier: &crate::native_stems_beam_scheduler::NativeStemsBeamAwaitingVLinkTransaction,
    stump_system: &NativeStemsBeamStumpSystem,
    vlinker_system: &NativeStemsBeamVLinkerSystem,
    reachability_system: &NativeStemsBeamReachabilitySystem,
    builder_system: &NativeStemsBeamBuilderSystem,
    predecessor: &NativeStemsBeamVLinkBLinkerFlagTransaction,
    base_apply_transaction: &NativeStemsBeamVLinkBaseApplyTransaction,
    certificate: &NativeStemsBeamVLinkSiblingLinksCertificate,
    state: &mut NativeStemsBeamVLinkSiblingLinksState,
) -> Result<PreparedSiblingLinks, NativeStemsBeamVLinkSiblingLinksError> {
    let system_id = frontier.plan.system_id;
    let base_state = state.base_apply_state_after.clone();
    let stem = &predecessor.stem_after;
    let stem_vertex = base_state.sig.stem.sig_vertex_identity.ok_or(
        NativeStemsBeamVLinkSiblingLinksError::UnsupportedV1 {
            phase: "sibling target stem absent from live SIG",
        },
    )?;
    let stem_inter_id = stem.inter_id.filter(|id| *id > 0).ok_or(
        NativeStemsBeamVLinkSiblingLinksError::UnsupportedV1 {
            phase: "sibling target stem lacks positive Java ID",
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
        || base_state.sig.beam.beam_group.is_none()
        || certificate.interline != vlinker_system.interline
        || certificate.x_in_gap_maximum_profile0.to_bits() != X_IN_GAP_MAXIMUM_PROFILE_0.to_bits()
        || certificate.max_beam_side_dx != vlinker_system.max_beam_side_dx
        || certificate.max_shorter_ratio.to_bits() != MAX_SHORTER_RATIO.to_bits()
        || certificate.portion_maximum_dx
            != java_rint_i32(f64::from(certificate.interline) * X_IN_GAP_MAXIMUM_PROFILE_0)
        || certificate.base_glyph != state.base_glyph
        || state.stem_alias.is_empty()
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::UnsupportedV1 {
            phase: "compact listener/endpoint/constant envelope",
        });
    }

    validate_unique_cells(&state.sibling_b_linker_cells)?;
    let (base_b, v_linker) = resolve_base_vlinker(vlinker_system, frontier.v_linker)?;
    let inspection = reachability_system
        .beam_inspections
        .iter()
        .flat_map(|beam| &beam.b_visits)
        .flat_map(|visit| &visit.v_inspections)
        .find(|inspection| inspection.reference == frontier.v_linker)
        .ok_or(NativeStemsBeamVLinkSiblingLinksError::Predecessor {
            phase: "reachability V inspection",
        })?;
    if inspection.y_direction != v_linker.y_direction
        || inspection.y_direction
            != builder_system
                .builders
                .get(frontier.plan.builder_ordinal)
                .ok_or(NativeStemsBeamVLinkSiblingLinksError::Predecessor {
                    phase: "builder ordinal",
                })?
                .y_direction
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::Predecessor {
            phase: "V/builder direction",
        });
    }
    let builder = &builder_system.builders[frontier.plan.builder_ordinal];
    if builder.start != frontier.v_linker
        || builder.builder_ordinal != frontier.plan.builder_ordinal
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::Predecessor {
            phase: "builder start",
        });
    }
    let base_beam = find_stump_beam(stump_system, frontier.beam)?;
    validate_cached_base_and_group(state, base_beam, stem_inter_id, stem_vertex)?;
    if state.live_group_members.iter().any(|beam| {
        beam.runtime.inter_id == state.group_runtime.inter_id
            || beam.runtime.sig_vertex_identity == Some(state.group_runtime.sig_vertex_identity)
    }) {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "BeamGroupInter/member endpoint collision",
        });
    }
    let (group_members, sibling_sources) = validate_group_scan(
        stump_system,
        reachability_system,
        &base_state,
        certificate,
        base_b.reference_point,
        frontier.beam,
        base_beam,
        stem_inter_id,
        stem_vertex,
        &state.live_group_members,
        inspection,
    )?;
    if certificate.steps.len() != sibling_sources.len()
        || certificate
            .steps
            .iter()
            .zip(&sibling_sources)
            .enumerate()
            .any(|(ordinal, (step, source))| {
                step.sibling_ordinal != ordinal || step.source != *source
            })
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "sibling step order/cardinality",
        });
    }
    validate_cross_query_relations(certificate, state, stem_inter_id, stem_vertex)?;

    let starting_sheet = state.sheet_edit;
    let group_member_state_sha256_before = state.group_runtime.member_state_sha256.clone();
    let mut relation_objects = initial_relation_object_map(&base_state, &state.appended_relations)?;
    for row in &certificate.group_scan.relations {
        join_relation_object(
            &mut relation_objects,
            row.graph_relation_identity,
            row.relation_object_identity,
        )?;
    }
    let mut previous_stem_scan = initial_stem_scan(
        base_apply_transaction,
        &state.live_group_members,
        &state.stem_alias,
    )?;
    let mut traces = Vec::with_capacity(sibling_sources.len());
    let mut operations = Vec::new();
    let mut appended_ids = Vec::new();
    let mut assigned_b_linkers = Vec::new();
    let mut beam_abnormal_mutations = 0;
    let mut b_linker_writes = 0;
    let mut b_linker_value_changes = 0;
    let mut referenced_cells = Vec::new();
    let base_cross = generic_intersection(stem_segment(stem), base_beam.median);
    let base_length = base_beam.median.x2 - base_beam.median.x1;

    for (sibling_ordinal, source) in sibling_sources.iter().copied().enumerate() {
        let step = &certificate.steps[sibling_ordinal];
        let live_index = state
            .live_group_members
            .iter()
            .position(|member| member.source == source)
            .ok_or(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                phase: "selected sibling live payload",
            })?;
        let sibling_beam = find_stump_beam(stump_system, source)?;
        let same_glyph = same_glyph_identity(
            state.live_group_members[live_index].glyph.as_ref(),
            state.base_glyph.as_ref(),
        );
        let mut trace = NativeStemsBeamVLinkSiblingTrace {
            sibling_ordinal,
            source,
            branch: NativeStemsBeamSiblingBranch::SameGlyph,
            same_glyph_identity: same_glyph,
            directed_pair_relations_read: 0,
            geometry: None,
            relation: None,
            stem_incident_graph_relation_identities: Vec::new(),
            beam_abnormal: NativeStemsBeamSiblingBeamAbnormalTrace::NotRead,
            builder_lookup: None,
            selected_b_linker: None,
            linked_before: None,
            linked_after: None,
            closed_before: None,
            closed_after: None,
        };
        if same_glyph {
            validate_unread_step(step, "same-glyph branch")?;
            traces.push(trace);
            continue;
        }

        let next_relation_identity =
            next_relation_identity(&base_state, &state.appended_relations)?;
        let pair = validate_pair_scan(step, next_relation_identity, &mut relation_objects)?;
        trace.directed_pair_relations_read = pair.relations_read;
        if pair.first_match.is_some() {
            validate_no_callback(step, "existing BeamStem branch")?;
            trace.branch = NativeStemsBeamSiblingBranch::ExistingBeamStem;
            traces.push(trace);
            continue;
        }

        let geometry = sibling_geometry(
            stem,
            base_beam,
            sibling_beam,
            base_cross,
            inspection.y_direction,
            certificate,
            predecessor.continuation_support_grade,
        );
        let wrong_side = geometry.wrong_side == Some(true);
        trace.geometry = Some(geometry.clone());
        if wrong_side {
            validate_no_callback(step, "shorter wrong-side branch")?;
            trace.branch = NativeStemsBeamSiblingBranch::ShorterWrongSide;
            traces.push(trace);
            continue;
        }

        let sibling_runtime = &state.live_group_members[live_index].runtime;
        let source_vertex = sibling_runtime.sig_vertex_identity.ok_or(
            NativeStemsBeamVLinkSiblingLinksError::UnsupportedV1 {
                phase: "sibling beam missing live SIG vertex",
            },
        )?;
        if sibling_runtime.removed
            || !sibling_runtime.inter_indexed
            || sibling_runtime.inter_id <= 0
            || sibling_runtime.inter_id == stem_inter_id
            || source_vertex == stem_vertex
            || sibling_runtime.sig_system_id != system_id
        {
            return Err(NativeStemsBeamVLinkSiblingLinksError::UnsupportedV1 {
                phase: "sibling beam endpoint state",
            });
        }
        let object_identity = NativeStemsBeamSiblingRelationObjectIdentity::SiblingDraft {
            plan_ordinal: frontier.plan.plan_ordinal,
            sibling_ordinal,
        };
        if relation_objects
            .values()
            .any(|value| *value == object_identity)
        {
            return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                phase: "fresh sibling relation object collision",
            });
        }
        let appended = NativeStemsBeamSiblingAppendedRelation {
            graph_relation_identity: next_relation_identity,
            relation_object_identity: object_identity,
            source,
            source_vertex_identity: source_vertex,
            target_stem_identity: stem.stem_identity,
            target_vertex_identity: stem_vertex,
            extension_point: geometry.extension_point.ok_or(
                NativeStemsBeamVLinkSiblingLinksError::DefensiveCommitInvariant {
                    phase: "linked sibling extension was not materialized",
                },
            )?,
            beam_portion: geometry.beam_portion.ok_or(
                NativeStemsBeamVLinkSiblingLinksError::DefensiveCommitInvariant {
                    phase: "linked sibling portion was not materialized",
                },
            )?,
            grade: predecessor.continuation_support_grade,
        };
        state.appended_relations.push(appended.clone());
        relation_objects.insert(next_relation_identity, object_identity);
        append_edge_operations(sibling_ordinal, next_relation_identity, &mut operations);

        let stem_scan = step.stem_incident_after.as_ref().ok_or(
            NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                phase: "missing post-edge stem callback scan",
            },
        )?;
        validate_stem_incident_scan(
            stem_scan,
            &appended,
            stem_inter_id,
            sibling_runtime.inter_id,
            &state.live_group_members[live_index].alias,
            next_relation_identity + 1,
            previous_stem_scan.as_deref(),
            &mut relation_objects,
        )?;
        previous_stem_scan = Some(stem_scan.relations.clone());
        if step.chord_stem_matches != 0
            || step.chord_stem_matches
                != stem_scan
                    .relations
                    .iter()
                    .filter(|row| row.chord_stem_match)
                    .count()
        {
            return Err(NativeStemsBeamVLinkSiblingLinksError::UnsupportedV1 {
                phase: "nonzero ChordStem callback envelope",
            });
        }
        operations.push(
            NativeStemsBeamVLinkSiblingLinksOperation::StemChordIncidentScanCompleted {
                sibling_ordinal,
                incident_relation_count: stem_scan.relations.len(),
                chord_stem_matches: 0,
            },
        );

        let beam_scan = step.beam_incident_after.as_ref().ok_or(
            NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                phase: "missing post-edge beam callback scan",
            },
        )?;
        let abnormal_before = state.live_group_members[live_index].runtime.abnormal;
        let (abnormal_after, abnormal_trace) = validate_beam_incident_scan(
            beam_scan,
            BeamIncidentValidationContext {
                beam_kind: sibling_beam.kind,
                fresh: &appended,
                source_outgoing_before: &step.directed_pair.source_outgoing_relations,
                stem_inter_id,
                sibling_inter_id: sibling_runtime.inter_id,
                stem_alias: &state.stem_alias,
                group_inter_id: state.group_runtime.inter_id,
                group_vertex: state.group_runtime.sig_vertex_identity,
                group_alias: &state.group_runtime.alias,
                live_group_members: &state.live_group_members,
                graph_limit: next_relation_identity + 1,
                abnormal_before,
            },
            &mut relation_objects,
        )?;
        if abnormal_before != abnormal_after {
            state.live_group_members[live_index].runtime.abnormal = abnormal_after;
            beam_abnormal_mutations += 1;
            operations.push(NativeStemsBeamVLinkSiblingLinksOperation::BeamAbnormalSet {
                sibling_ordinal,
                before: abnormal_before,
                after: abnormal_after,
            });
            dirty_cascade(sibling_ordinal, state, &mut operations);
        }
        operations.push(
            NativeStemsBeamVLinkSiblingLinksOperation::BeamStemRelationCallbackCompleted {
                sibling_ordinal,
            },
        );
        operations.push(
            NativeStemsBeamVLinkSiblingLinksOperation::StandardSigListenerEdgeCallbackCompleted {
                sibling_ordinal,
            },
        );

        let lookup = step.builder_lookup.as_ref().ok_or(
            NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                phase: "missing post-callback builder lookup",
            },
        )?;
        let selected_b =
            validate_builder_lookup(lookup, builder, source, &state.live_group_members)?;
        trace.builder_lookup = Some(lookup.clone());
        if let Some(reference) = selected_b {
            referenced_cells.push(reference);
            let ordered_observer_v_linkers =
                resolve_sibling_b_linker_observers(vlinker_system, reachability_system, reference)?;
            let cell = state
                .sibling_b_linker_cells
                .iter_mut()
                .find(|cell| cell.reference == reference)
                .ok_or(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                    phase: "selected sibling B-linker cell",
                })?;
            let before = cell.linked;
            let closed = cell.closed;
            cell.linked = true;
            b_linker_writes += 1;
            b_linker_value_changes += usize::from(!before);
            assigned_b_linkers.push(reference);
            operations.push(
                NativeStemsBeamVLinkSiblingLinksOperation::BLinkerLinkedAssigned {
                    sibling_ordinal,
                    target: reference,
                    ordered_observer_v_linkers,
                    before,
                    after: true,
                    closed_before: closed,
                    closed_after: closed,
                },
            );
            trace.selected_b_linker = Some(reference);
            trace.linked_before = Some(before);
            trace.linked_after = Some(true);
            trace.closed_before = Some(closed);
            trace.closed_after = Some(closed);
        }
        trace.branch = NativeStemsBeamSiblingBranch::Linked;
        trace.relation = Some(appended.clone());
        trace.stem_incident_graph_relation_identities = stem_scan
            .relations
            .iter()
            .map(|row| row.graph_relation_identity)
            .collect();
        trace.beam_abnormal = abnormal_trace;
        appended_ids.push(next_relation_identity);
        traces.push(trace);
    }

    referenced_cells.sort_by_key(|reference| (reference.beam, reference.id));
    referenced_cells.dedup();
    let mut actual_cells = state
        .sibling_b_linker_cells
        .iter()
        .map(|cell| cell.reference)
        .collect::<Vec<_>>();
    actual_cells.sort_by_key(|reference| (reference.beam, reference.id));
    if actual_cells != referenced_cells {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "compact referenced sibling B-cell catalogue",
        });
    }
    let group_member_state_sha256_after =
        certificate.expected_group_member_state_sha256_after.clone();
    validate_group_member_state_transition(
        &group_member_state_sha256_before,
        &group_member_state_sha256_after,
        beam_abnormal_mutations,
    )?;
    state.group_runtime.member_state_sha256 = group_member_state_sha256_after.clone();
    for member in &mut state.live_group_members {
        let group = member.runtime.beam_group.as_mut().ok_or(
            NativeStemsBeamVLinkSiblingLinksError::DefensiveCommitInvariant {
                phase: "live sibling lost shared BeamGroupInter state",
            },
        )?;
        group.state_sha256 = group_member_state_sha256_after.clone();
    }
    let final_sheet = state.sheet_edit;
    Ok(PreparedSiblingLinks {
        base_cross,
        base_length,
        group_members,
        sibling_sources,
        siblings: traces,
        operations,
        appended_graph_relation_identities: appended_ids.clone(),
        assigned_b_linkers,
        sig_relation_mutation_count: appended_ids.len(),
        beam_abnormal_mutation_count: beam_abnormal_mutations,
        sheet_edit_mutation_count: sheet_edit_delta(starting_sheet, final_sheet),
        b_linker_write_count: b_linker_writes,
        b_linker_value_change_count: b_linker_value_changes,
        group_member_state_sha256_before,
        group_member_state_sha256_after,
    })
}

fn validate_group_member_state_transition(
    before: &str,
    after: &str,
    abnormal_mutations: usize,
) -> Result<(), NativeStemsBeamVLinkSiblingLinksError> {
    if !valid_sha256(before)
        || !valid_sha256(after)
        || (abnormal_mutations == 0 && after != before)
        || (abnormal_mutations != 0 && after == before)
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "member-inclusive BeamGroupInter post-state hash",
        });
    }
    Ok(())
}

fn validate_cached_base_and_group(
    state: &NativeStemsBeamVLinkSiblingLinksState,
    base_beam: &NativeStemsBeamStumpBeam,
    stem_inter_id: i32,
    stem_vertex: usize,
) -> Result<(), NativeStemsBeamVLinkSiblingLinksError> {
    let base_runtime = &state.base_apply_state_after.sig.beam;
    let live_group = base_runtime.beam_group.as_ref().ok_or(
        NativeStemsBeamVLinkSiblingLinksError::UnsupportedV1 {
            phase: "base beam lacks live BeamGroupInter",
        },
    )?;
    let group = &state.group_runtime;
    let vertex_limit = state
        .base_apply_state_after
        .sig
        .baseline_vertex_count
        .checked_add(state.base_apply_state_after.sig.appended_vertices.len())
        .ok_or(NativeStemsBeamVLinkSiblingLinksError::UnsupportedV1 {
            phase: "SIG vertex count overflow",
        })?;
    if !state.cached_base_median_same_identity
        || !segment_bits_equal(state.cached_base_median, base_beam.median)
        || group.runtime_class != BEAM_GROUP_INTER_CLASS
        || group.inter_id <= 0
        || group.inter_id == base_runtime.inter_id
        || group.inter_id == stem_inter_id
        || group.sig_vertex_identity >= vertex_limit
        || group.sig_vertex_identity == base_runtime.sig_vertex_identity.unwrap_or(usize::MAX)
        || group.sig_vertex_identity == stem_vertex
        || group.alias != format!("beam-group:{}", group.sig_vertex_identity)
        || group.removed
        || !valid_sha256(&group.member_state_sha256_before)
        || group.member_state_sha256 != group.member_state_sha256_before
        || !valid_sha256(&group.object_state_sha256)
        || group.sig_vertex_identity != live_group.sig_vertex_ordinal
        || group.member_state_sha256_before != live_group.state_sha256
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "cached base median/live BeamGroupInter state",
        });
    }
    Ok(())
}

fn segment_bits_equal(left: Segment, right: Segment) -> bool {
    left.x1.to_bits() == right.x1.to_bits()
        && left.y1.to_bits() == right.y1.to_bits()
        && left.x2.to_bits() == right.x2.to_bits()
        && left.y2.to_bits() == right.y2.to_bits()
}

fn resolve_base_vlinker(
    system: &NativeStemsBeamVLinkerSystem,
    reference: crate::native_stems_beam_vlinkers::NativeStemsBeamVLinkerRef,
) -> Result<
    (
        &crate::native_stems_beam_vlinkers::NativeStemsBeamBLinker,
        &crate::native_stems_beam_vlinkers::NativeStemsBeamVLinker,
    ),
    NativeStemsBeamVLinkSiblingLinksError,
> {
    let matches = system
        .constructors
        .iter()
        .flat_map(|constructor| &constructor.b_linkers)
        .filter(|b_linker| b_linker.reference == reference.b_linker)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(NativeStemsBeamVLinkSiblingLinksError::Predecessor {
            phase: "selected B-linker cardinality",
        });
    }
    let b_linker = matches[0];
    let v_matches = b_linker
        .v_linkers
        .iter()
        .filter(|v_linker| v_linker.reference == reference)
        .collect::<Vec<_>>();
    if v_matches.len() != 1 {
        return Err(NativeStemsBeamVLinkSiblingLinksError::Predecessor {
            phase: "selected V-linker cardinality",
        });
    }
    Ok((b_linker, v_matches[0]))
}

fn resolve_sibling_b_linker_observers(
    vlinker_system: &NativeStemsBeamVLinkerSystem,
    reachability_system: &NativeStemsBeamReachabilitySystem,
    reference: NativeStemsBeamBLinkerRef,
) -> Result<
    Vec<crate::native_stems_beam_vlinkers::NativeStemsBeamVLinkerRef>,
    NativeStemsBeamVLinkSiblingLinksError,
> {
    let arena_matches = reachability_system
        .final_beam_arenas
        .iter()
        .filter(|arena| arena.beam == reference.beam)
        .collect::<Vec<_>>();
    if arena_matches.len() != 1 {
        return Err(NativeStemsBeamVLinkSiblingLinksError::Predecessor {
            phase: "sibling B-linker beam arena cardinality",
        });
    }
    let entry_matches = arena_matches[0]
        .all_b_linkers
        .iter()
        .filter(|entry| entry.reference == reference)
        .collect::<Vec<_>>();
    if entry_matches.len() != 1 {
        return Err(NativeStemsBeamVLinkSiblingLinksError::Predecessor {
            phase: "sibling B-linker live arena cardinality",
        });
    }
    if entry_matches[0].is_anchor {
        if !matches!(
            entry_matches[0].origin,
            crate::native_stems_beam_reachability::NativeStemsBeamArenaOrigin::Anchor { .. }
        ) {
            return Err(NativeStemsBeamVLinkSiblingLinksError::PredecessorMismatch);
        }
        return Ok(Vec::new());
    }
    if !matches!(
        entry_matches[0].origin,
        crate::native_stems_beam_reachability::NativeStemsBeamArenaOrigin::Constructor(_)
    ) {
        return Err(NativeStemsBeamVLinkSiblingLinksError::PredecessorMismatch);
    }
    let constructor_matches = vlinker_system
        .constructors
        .iter()
        .filter(|constructor| constructor.source == reference.beam)
        .flat_map(|constructor| &constructor.b_linkers)
        .filter(|b_linker| b_linker.reference == reference)
        .collect::<Vec<_>>();
    if constructor_matches.len() != 1 {
        return Err(NativeStemsBeamVLinkSiblingLinksError::Predecessor {
            phase: "sibling constructor B-linker cardinality",
        });
    }
    let observers = constructor_matches[0]
        .v_linkers
        .iter()
        .map(|v_linker| v_linker.reference)
        .collect::<Vec<_>>();
    if observers.len() > 2
        || observers
            .iter()
            .any(|observer| observer.b_linker != reference)
        || observers
            .windows(2)
            .any(|pair| pair[0].side >= pair[1].side)
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::Predecessor {
            phase: "sibling shared B-cell observer order",
        });
    }
    Ok(observers)
}

fn find_stump_beam(
    system: &NativeStemsBeamStumpSystem,
    source: NativeStemsBeamSource,
) -> Result<&NativeStemsBeamStumpBeam, NativeStemsBeamVLinkSiblingLinksError> {
    let matches = system
        .beams_by_abscissa
        .iter()
        .filter(|beam| beam.source == source)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(NativeStemsBeamVLinkSiblingLinksError::Predecessor {
            phase: "stump beam cardinality",
        });
    }
    Ok(matches[0])
}

#[allow(clippy::too_many_arguments)]
fn validate_group_scan(
    stump_system: &NativeStemsBeamStumpSystem,
    reachability_system: &NativeStemsBeamReachabilitySystem,
    base_state: &NativeStemsBeamVLinkBaseApplyState,
    certificate: &NativeStemsBeamVLinkSiblingLinksCertificate,
    reference_point: NativeStemPoint,
    base_source: NativeStemsBeamSource,
    base_beam: &NativeStemsBeamStumpBeam,
    stem_inter_id: i32,
    stem_vertex: usize,
    live_members: &[NativeStemsBeamSiblingLiveBeam],
    inspection: &crate::native_stems_beam_reachability::NativeStemsBeamVInspection,
) -> Result<
    (
        Vec<NativeStemsBeamSiblingGroupMemberTrace>,
        Vec<NativeStemsBeamSource>,
    ),
    NativeStemsBeamVLinkSiblingLinksError,
> {
    let scan = &certificate.group_scan;
    if scan.query_relation_count != scan.relations.len()
        || !valid_sha256(&scan.query_provenance_sha256)
        || scan.query_provenance_sha256 != group_query_sha256(&scan.relations)
        || scan
            .relations
            .iter()
            .enumerate()
            .any(|(ordinal, row)| row.outgoing_ordinal != ordinal)
        || scan
            .relations
            .windows(2)
            .any(|pair| pair[0].graph_relation_identity >= pair[1].graph_relation_identity)
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "beam-group outgoing query chronology",
        });
    }
    let graph_limit = next_relation_identity(base_state, &[])?;
    let vertex_limit = base_state
        .sig
        .baseline_vertex_count
        .checked_add(base_state.sig.appended_vertices.len())
        .ok_or(NativeStemsBeamVLinkSiblingLinksError::UnsupportedV1 {
            phase: "SIG vertex count overflow",
        })?;
    if scan.relations.iter().any(|row| {
        row.graph_relation_identity >= graph_limit
            || row.target_vertex_identity >= vertex_limit
            || row.target_inter_id <= 0
            || row.target_alias.is_empty()
            || row.target_class.is_empty()
            || row.relation_class.is_empty()
            || !matches!(
                row.relation_object_identity,
                NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(_)
            )
            || row.containment_match != (row.relation_class == CONTAINMENT_CLASS)
            || row.target_read_by_get_members != row.containment_match
            || row.target_evidence
                != if row.containment_match {
                    NativeStemsBeamSiblingGroupTargetEvidence::GetMembersRead
                } else {
                    NativeStemsBeamSiblingGroupTargetEvidence::GraphReconstruction
                }
    }) {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "beam-group relation domain",
        });
    }
    if scan.relations.iter().enumerate().any(|(index, row)| {
        scan.relations[..index].iter().any(|prior| {
            prior.graph_relation_identity == row.graph_relation_identity
                || prior.relation_object_identity == row.relation_object_identity
        })
    }) {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "beam-group graph/object identity uniqueness",
        });
    }

    let containment = scan
        .relations
        .iter()
        .filter(|row| row.containment_match)
        .collect::<Vec<_>>();
    if containment.len() != live_members.len()
        || containment.iter().enumerate().any(|(ordinal, row)| {
            row.member_ordinal != Some(ordinal)
                || !matches!(row.target, NativeStemsBeamSiblingGroupTarget::Beam(_))
        })
        || scan
            .relations
            .iter()
            .filter(|row| !row.containment_match)
            .any(|row| row.member_ordinal.is_some())
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "beam-group member projection",
        });
    }
    if live_members.iter().enumerate().any(|(index, member)| {
        live_members[..index]
            .iter()
            .any(|prior| prior.source == member.source)
    }) {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "duplicate live group beam object",
        });
    }
    validate_glyph_identities(certificate.base_glyph.as_ref(), live_members)?;

    let expected_group = reachability_system
        .groups_in_source_order
        .get(base_beam.group_ordinal)
        .ok_or(NativeStemsBeamVLinkSiblingLinksError::Predecessor {
            phase: "reachability beam group",
        })?;
    let actual_sources = live_members
        .iter()
        .map(|member| member.source)
        .collect::<Vec<_>>();
    if &actual_sources != expected_group {
        return Err(NativeStemsBeamVLinkSiblingLinksError::PredecessorMismatch);
    }
    let mut persistent_ids = BTreeMap::new();
    let mut inter_index_ordinals = BTreeMap::new();
    let mut vertices = BTreeMap::new();
    let mut aliases = BTreeMap::new();
    let mut target_endpoints = EndpointIdentityCatalogue::with_glyph_ids(
        base_state
            .transaction_state
            .glyph_index
            .known_canonical_glyphs
            .iter()
            .map(|glyph| glyph.glyph_id)
            .chain(
                base_state
                    .transaction_state
                    .selected_glyph_bindings
                    .iter()
                    .map(|glyph| glyph.glyph_id),
            ),
    );
    let mut target_classes = BTreeMap::new();
    for row in &scan.relations {
        target_endpoints.join(
            row.target_vertex_identity,
            row.target_inter_id,
            &row.target_alias,
            "beam-group target identity consistency",
        )?;
        if target_classes
            .insert(row.target_vertex_identity, row.target_class.clone())
            .is_some_and(|prior| prior != row.target_class)
        {
            return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                phase: "beam-group target class consistency",
            });
        }
    }
    for (ordinal, (row, member)) in containment.iter().zip(live_members).enumerate() {
        let NativeStemsBeamSiblingGroupTarget::Beam(source) = row.target else {
            unreachable!("containment member shape checked")
        };
        let beam = find_stump_beam(stump_system, source)?;
        let expected_class = match beam.kind {
            BeamKind::Beam => BEAM_INTER_CLASS,
            BeamKind::Hook => BEAM_HOOK_INTER_CLASS,
            BeamKind::SmallBeam => SMALL_BEAM_INTER_CLASS,
        };
        let expected_glyph_token = java_glyph_token(&beam.beam_glyph);
        let runtime = &member.runtime;
        let vertex = runtime.sig_vertex_identity.ok_or(
            NativeStemsBeamVLinkSiblingLinksError::UnsupportedV1 {
                phase: "group member absent from live SIG",
            },
        )?;
        if member.source != source
            || member.alias != row.target_alias
            || member.alias != beam_alias(beam.sig_ordinal)
            || row.target_inter_id != runtime.inter_id
            || row.target_vertex_identity != vertex
            || runtime.source != source
            || runtime.inter_id <= 0
            || runtime.inter_id == stem_inter_id
            || runtime.removed
            || !runtime.inter_indexed
            || runtime.sig_system_id != stump_system.system_id
            || runtime.stump_group_ordinal != base_beam.group_ordinal
            || vertex == stem_vertex
            || beam.group_ordinal != base_beam.group_ordinal
            || runtime.beam_group != base_state.sig.beam.beam_group
            || row.target_class != expected_class
            || member.glyph.as_ref().map(|glyph| glyph.token.as_str())
                != Some(expected_glyph_token.as_str())
        {
            return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                phase: "live group beam payload",
            });
        }
        validate_live_member_index(member, base_state.inter_index.baseline_entry_count)?;
        if persistent_ids.insert(runtime.inter_id, source).is_some()
            || inter_index_ordinals
                .insert(member.inter_index_ordinal, source)
                .is_some()
            || vertices.insert(vertex, source).is_some()
            || aliases.insert(member.alias.clone(), source).is_some()
        {
            return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                phase: "live group endpoint uniqueness",
            });
        }
        if row.member_ordinal != Some(ordinal) {
            return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                phase: "group member ordinal",
            });
        }
    }
    let base_member = live_members
        .iter()
        .find(|member| member.source == base_source)
        .ok_or(NativeStemsBeamVLinkSiblingLinksError::Predecessor {
            phase: "base beam absent from its live group",
        })?;
    if base_member.runtime != base_state.sig.beam
        || base_member.glyph != certificate.base_glyph
        || !base_live_member_index_matches(base_state.inter_index.beam_lookup, base_member)
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "base beam live-group alias",
        });
    }

    let vertical = Segment {
        x1: reference_point.x,
        y1: reference_point.y,
        x2: reference_point.x - (1_000.0 * reachability_system.global_slope),
        y2: reference_point.y + 1_000.0,
    };
    let mut traces = Vec::with_capacity(live_members.len());
    for (member_ordinal, member) in live_members.iter().enumerate() {
        let beam = find_stump_beam(stump_system, member.source)?;
        let cross = generic_intersection(vertical, beam.median);
        let left_limit = beam.median.x1 - f64::from(certificate.max_beam_side_dx);
        let right_limit = beam.median.x2 + f64::from(certificate.max_beam_side_dx);
        let selected = left_limit <= cross.x && cross.x <= right_limit;
        traces.push(NativeStemsBeamSiblingGroupMemberTrace {
            member_ordinal,
            source: member.source,
            cross,
            left_limit,
            right_limit,
            selected,
            sorted_ordinal: None,
            removed_as_base: false,
        });
    }
    let mut selected_indices = traces
        .iter()
        .enumerate()
        .filter_map(|(index, row)| row.selected.then_some(index))
        .collect::<Vec<_>>();
    selected_indices
        .sort_by(|left, right| traces[*left].cross.y.total_cmp(&traces[*right].cross.y));
    for (sorted_ordinal, index) in selected_indices.iter().copied().enumerate() {
        traces[index].sorted_ordinal = Some(sorted_ordinal);
    }
    if let Some(index) = selected_indices
        .iter()
        .position(|index| traces[*index].source == base_source)
    {
        traces[selected_indices[index]].removed_as_base = true;
        selected_indices.remove(index);
    }
    if !group_member_traces_equal(&traces, &scan.members) {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "group member geometry/sort trace",
        });
    }
    let sorted_before_removal = traces.iter().filter(|row| row.selected).collect::<Vec<_>>();
    let mut sorted_before_removal = sorted_before_removal;
    sorted_before_removal.sort_by_key(|row| row.sorted_ordinal);
    if inspection.siblings.len() != sorted_before_removal.len()
        || inspection
            .siblings
            .iter()
            .zip(sorted_before_removal)
            .any(|(prior, now)| {
                prior.beam != now.source || !point_bits_equal(prior.cross, now.cross)
            })
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::PredecessorMismatch);
    }
    Ok((
        traces,
        selected_indices
            .into_iter()
            .map(|index| live_members[index].source)
            .collect(),
    ))
}

fn validate_live_member_index(
    member: &NativeStemsBeamSiblingLiveBeam,
    baseline_entry_count: usize,
) -> Result<(), NativeStemsBeamVLinkSiblingLinksError> {
    if member.inter_index_ordinal >= baseline_entry_count
        || member.inter_index_object_matches != 1
        || member.inter_index_id_matches != 1
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "live group beam InterIndex lookup",
        });
    }
    Ok(())
}

fn base_live_member_index_matches(
    lookup: NativeStemsBeamInterIndexLookup,
    member: &NativeStemsBeamSiblingLiveBeam,
) -> bool {
    matches!(
        lookup,
        NativeStemsBeamInterIndexLookup::PresentSameObject {
            index_ordinal,
            inter_id,
            vip,
            object_matches,
            inter_id_matches,
            ..
        } if index_ordinal == member.inter_index_ordinal
            && inter_id == member.runtime.inter_id
            && vip == member.runtime.vip
            && object_matches == member.inter_index_object_matches
            && inter_id_matches == member.inter_index_id_matches
    )
}

fn group_member_traces_equal(
    left: &[NativeStemsBeamSiblingGroupMemberTrace],
    right: &[NativeStemsBeamSiblingGroupMemberTrace],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.member_ordinal == right.member_ordinal
                && left.source == right.source
                && point_bits_equal(left.cross, right.cross)
                && left.left_limit.to_bits() == right.left_limit.to_bits()
                && left.right_limit.to_bits() == right.right_limit.to_bits()
                && left.selected == right.selected
                && left.sorted_ordinal == right.sorted_ordinal
                && left.removed_as_base == right.removed_as_base
        })
}

fn beam_alias(beam_sig_ordinal: usize) -> String {
    format!("beam:{beam_sig_ordinal}")
}

fn point_bits_equal(left: NativeStemPoint, right: NativeStemPoint) -> bool {
    left.x.to_bits() == right.x.to_bits() && left.y.to_bits() == right.y.to_bits()
}

fn validate_glyph_identities(
    base: Option<&NativeStemsBeamSiblingGlyphIdentity>,
    members: &[NativeStemsBeamSiblingLiveBeam],
) -> Result<(), NativeStemsBeamVLinkSiblingLinksError> {
    let mut objects = BTreeMap::new();
    let mut next_object_identity = 0;
    for glyph in members.iter().filter_map(|member| member.glyph.as_ref()) {
        if glyph.token.is_empty()
            || match objects.get(&glyph.object_identity) {
                Some(prior) => prior != &glyph.token,
                None => {
                    if glyph.object_identity != next_object_identity {
                        true
                    } else {
                        objects.insert(glyph.object_identity, glyph.token.clone());
                        next_object_identity += 1;
                        false
                    }
                }
            }
        {
            return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                phase: "group glyph object/content identity",
            });
        }
    }
    if base.is_some_and(|base| objects.get(&base.object_identity) != Some(&base.token)) {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "base glyph absent from dense group identity domain",
        });
    }
    Ok(())
}

fn java_glyph_token(glyph: &NativeStemsBeamGlyph) -> String {
    let orientation = match glyph.run_table.orientation() {
        Orientation::Horizontal => "HORIZONTAL",
        Orientation::Vertical => "VERTICAL",
    };
    let mut bytes = format!(
        "{orientation} {} {}\n",
        glyph.run_table.width(),
        glyph.run_table.height()
    )
    .into_bytes();
    for sequence in 0..glyph.run_table.sequence_count() {
        let mut row = sequence.to_string();
        for run in glyph.run_table.sequence(sequence).unwrap_or_default() {
            row.push_str(&format!(" {}:{}", run.start, run.length));
        }
        row.push('\n');
        bytes.extend_from_slice(row.as_bytes());
    }
    format!(
        "g:{}:{}:{}:{}:{}",
        glyph.bounds.x,
        glyph.bounds.y,
        glyph.bounds.width,
        glyph.bounds.height,
        sha256_hex(&bytes)
    )
}

fn same_glyph_identity(
    left: Option<&NativeStemsBeamSiblingGlyphIdentity>,
    right: Option<&NativeStemsBeamSiblingGlyphIdentity>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => left.object_identity == right.object_identity,
        (None, Some(_)) | (Some(_), None) => false,
    }
}

#[derive(Clone)]
struct CrossQueryRelation {
    object: NativeStemsBeamSiblingRelationObjectIdentity,
    class: String,
    coarse_kind: Option<NativeStemsBeamQueryRelationKind>,
    full_kind: Option<NativeStemsBeamSigRelationKind>,
}

fn validate_cross_query_relations(
    certificate: &NativeStemsBeamVLinkSiblingLinksCertificate,
    state: &NativeStemsBeamVLinkSiblingLinksState,
    stem_inter_id: i32,
    stem_vertex: usize,
) -> Result<(), NativeStemsBeamVLinkSiblingLinksError> {
    let mut observed = BTreeMap::new();
    let mut endpoints = EndpointIdentityCatalogue::with_glyph_ids(
        state
            .base_apply_state_after
            .transaction_state
            .glyph_index
            .known_canonical_glyphs
            .iter()
            .map(|glyph| glyph.glyph_id)
            .chain(
                state
                    .base_apply_state_after
                    .transaction_state
                    .selected_glyph_bindings
                    .iter()
                    .map(|glyph| glyph.glyph_id),
            ),
    );
    endpoints.join(
        state.group_runtime.sig_vertex_identity,
        state.group_runtime.inter_id,
        &state.group_runtime.alias,
        "cross-query group endpoint consistency",
    )?;
    endpoints.join(
        stem_vertex,
        stem_inter_id,
        &state.stem_alias,
        "cross-query stem endpoint consistency",
    )?;
    for member in &state.live_group_members {
        endpoints.join(
            member.runtime.sig_vertex_identity.ok_or(
                NativeStemsBeamVLinkSiblingLinksError::UnsupportedV1 {
                    phase: "cross-query group member SIG vertex",
                },
            )?,
            member.runtime.inter_id,
            &member.alias,
            "cross-query group member endpoint consistency",
        )?;
    }
    for row in &certificate.group_scan.relations {
        endpoints.join(
            row.target_vertex_identity,
            row.target_inter_id,
            &row.target_alias,
            "cross-query group target endpoint consistency",
        )?;
        join_cross_query_relation(
            &mut observed,
            row.graph_relation_identity,
            row.relation_object_identity,
            &row.relation_class,
            Some(NativeStemsBeamQueryRelationKind::Other),
            None,
        )?;
    }
    for step in &certificate.steps {
        for row in &step.directed_pair.source_outgoing_relations {
            join_cross_query_relation(
                &mut observed,
                row.graph_relation_identity,
                row.relation_object_identity,
                &row.relation_class,
                None,
                None,
            )?;
        }
        for row in &step.directed_pair.relations {
            join_cross_query_relation(
                &mut observed,
                row.graph_relation_identity,
                row.relation_object_identity,
                &row.relation_class,
                Some(row.kind),
                None,
            )?;
        }
        if let Some(scan) = &step.stem_incident_after {
            for row in &scan.relations {
                endpoints.join(
                    row.opposite_vertex_identity,
                    row.opposite_inter_id,
                    &row.opposite_alias,
                    "cross-query incident endpoint consistency",
                )?;
                join_cross_query_relation(
                    &mut observed,
                    row.graph_relation_identity,
                    row.relation_object_identity,
                    &row.relation_class,
                    Some(row.kind),
                    None,
                )?;
            }
        }
        if let Some(scan) = &step.beam_incident_after {
            for row in &scan.relations {
                endpoints.join(
                    row.opposite_vertex_identity,
                    row.opposite_inter_id,
                    &row.opposite_alias,
                    "cross-query incident endpoint consistency",
                )?;
                join_cross_query_relation(
                    &mut observed,
                    row.graph_relation_identity,
                    row.relation_object_identity,
                    &row.relation_class,
                    Some(coarse_relation_kind(row.kind)),
                    Some(row.kind),
                )?;
            }
        }
    }
    Ok(())
}

#[derive(Default)]
struct EndpointIdentityCatalogue {
    by_vertex: BTreeMap<usize, (i32, String)>,
    by_inter_id: BTreeMap<i32, (usize, String)>,
    by_alias: BTreeMap<String, (usize, i32)>,
    glyph_ids: BTreeSet<i32>,
}

impl EndpointIdentityCatalogue {
    fn with_glyph_ids(ids: impl IntoIterator<Item = i32>) -> Self {
        Self {
            glyph_ids: ids.into_iter().collect(),
            ..Self::default()
        }
    }

    fn join(
        &mut self,
        vertex: usize,
        inter_id: i32,
        alias: &str,
        phase: &'static str,
    ) -> Result<(), NativeStemsBeamVLinkSiblingLinksError> {
        if inter_id <= 0
            || self.glyph_ids.contains(&inter_id)
            || alias.is_empty()
            || self
                .by_vertex
                .get(&vertex)
                .is_some_and(|prior| *prior != (inter_id, alias.to_owned()))
            || self
                .by_inter_id
                .get(&inter_id)
                .is_some_and(|prior| *prior != (vertex, alias.to_owned()))
            || self
                .by_alias
                .get(alias)
                .is_some_and(|prior| *prior != (vertex, inter_id))
        {
            return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence { phase });
        }
        self.by_vertex.insert(vertex, (inter_id, alias.to_owned()));
        self.by_inter_id
            .insert(inter_id, (vertex, alias.to_owned()));
        self.by_alias.insert(alias.to_owned(), (vertex, inter_id));
        Ok(())
    }
}

fn join_cross_query_relation(
    observed: &mut BTreeMap<usize, CrossQueryRelation>,
    graph_identity: usize,
    object: NativeStemsBeamSiblingRelationObjectIdentity,
    class: &str,
    coarse_kind: Option<NativeStemsBeamQueryRelationKind>,
    full_kind: Option<NativeStemsBeamSigRelationKind>,
) -> Result<(), NativeStemsBeamVLinkSiblingLinksError> {
    if let Some(prior) = observed.get_mut(&graph_identity) {
        if prior.object != object
            || prior.class != class
            || prior
                .coarse_kind
                .zip(coarse_kind)
                .is_some_and(|(left, right)| left != right)
            || prior
                .full_kind
                .zip(full_kind)
                .is_some_and(|(left, right)| left != right)
        {
            return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                phase: "cross-query relation payload consistency",
            });
        }
        if prior.coarse_kind.is_none() {
            prior.coarse_kind = coarse_kind;
        }
        if prior.full_kind.is_none() {
            prior.full_kind = full_kind;
        }
    } else {
        observed.insert(
            graph_identity,
            CrossQueryRelation {
                object,
                class: class.to_owned(),
                coarse_kind,
                full_kind,
            },
        );
    }
    Ok(())
}

fn coarse_relation_kind(kind: NativeStemsBeamSigRelationKind) -> NativeStemsBeamQueryRelationKind {
    match kind {
        NativeStemsBeamSigRelationKind::BeamStem { .. } => {
            NativeStemsBeamQueryRelationKind::BeamStem
        }
        NativeStemsBeamSigRelationKind::BeamRest { .. } => {
            NativeStemsBeamQueryRelationKind::BeamRest
        }
        NativeStemsBeamSigRelationKind::ChordStem => NativeStemsBeamQueryRelationKind::ChordStem,
        NativeStemsBeamSigRelationKind::Other => NativeStemsBeamQueryRelationKind::Other,
    }
}

struct PairResult {
    relations_read: usize,
    first_match: Option<usize>,
}

fn validate_pair_scan(
    step: &NativeStemsBeamSiblingStepCertificate,
    graph_limit: usize,
    relation_objects: &mut BTreeMap<usize, NativeStemsBeamSiblingRelationObjectIdentity>,
) -> Result<PairResult, NativeStemsBeamVLinkSiblingLinksError> {
    let scan = &step.directed_pair;
    if scan.query_relation_count != scan.relations.len()
        || scan.source_outgoing_scanned != scan.source_outgoing_relations.len()
        || !matches!(
            &scan.source_outgoing_provenance,
            NativeStemsBeamSiblingQueryProvenance::ExhaustiveSha256(value)
                if valid_sha256(value)
        )
        || !matches!(
            &scan.pair_provenance,
            NativeStemsBeamSiblingQueryProvenance::ExhaustiveSha256(value)
                if valid_sha256(value)
        )
        || scan.source_outgoing_scanned < scan.relations.len()
        || scan
            .source_outgoing_relations
            .iter()
            .enumerate()
            .any(|(ordinal, row)| row.source_outgoing_ordinal != ordinal)
        || scan
            .source_outgoing_relations
            .windows(2)
            .any(|pair| pair[0].graph_relation_identity >= pair[1].graph_relation_identity)
        || scan
            .source_outgoing_relations
            .iter()
            .any(|row| row.graph_relation_identity >= graph_limit || row.relation_class.is_empty())
        || scan
            .relations
            .iter()
            .enumerate()
            .any(|(ordinal, row)| row.pair_ordinal != ordinal)
        || scan.relations.windows(2).any(|pair| {
            pair[0].source_outgoing_ordinal >= pair[1].source_outgoing_ordinal
                || pair[0].graph_relation_identity >= pair[1].graph_relation_identity
        })
        || scan.relations.iter().any(|row| {
            row.graph_relation_identity >= graph_limit
                || row.source_outgoing_ordinal >= scan.source_outgoing_scanned
                || !relation_class_matches_kind(&row.relation_class, row.kind)
        })
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "directed pair query",
        });
    }
    for row in &scan.source_outgoing_relations {
        join_relation_object(
            relation_objects,
            row.graph_relation_identity,
            row.relation_object_identity,
        )?;
    }
    for row in &scan.relations {
        let source = &scan.source_outgoing_relations[row.source_outgoing_ordinal];
        if source.graph_relation_identity != row.graph_relation_identity
            || source.relation_object_identity != row.relation_object_identity
            || source.relation_class != row.relation_class
        {
            return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                phase: "directed pair/source-outgoing projection",
            });
        }
    }
    let NativeStemsBeamSiblingQueryProvenance::ExhaustiveSha256(source_sha) =
        &scan.source_outgoing_provenance
    else {
        unreachable!("exhaustive provenance checked above")
    };
    let NativeStemsBeamSiblingQueryProvenance::ExhaustiveSha256(pair_sha) = &scan.pair_provenance
    else {
        unreachable!("exhaustive provenance checked above")
    };
    if *source_sha != source_outgoing_query_sha256(&scan.source_outgoing_relations)
        || *pair_sha != pair_query_sha256(&scan.relations)
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "directed pair query provenance",
        });
    }
    let first_match = scan
        .relations
        .iter()
        .position(|row| row.kind == NativeStemsBeamQueryRelationKind::BeamStem);
    for (index, row) in scan.relations.iter().enumerate() {
        let expected_read = match first_match {
            Some(first) if index < first => NativeStemsBeamSiblingPairClassRead::ExaminedContinue,
            Some(first) if index == first => {
                NativeStemsBeamSiblingPairClassRead::ExaminedMatchBreak
            }
            Some(_) => NativeStemsBeamSiblingPairClassRead::UnreadAfterBreak,
            None => NativeStemsBeamSiblingPairClassRead::ExaminedContinue,
        };
        if row.class_read != expected_read {
            return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                phase: "directed pair lazy class reads",
            });
        }
        join_relation_object(
            relation_objects,
            row.graph_relation_identity,
            row.relation_object_identity,
        )?;
    }
    Ok(PairResult {
        relations_read: first_match.map_or(scan.relations.len(), |index| index + 1),
        first_match,
    })
}

fn validate_unread_step(
    step: &NativeStemsBeamSiblingStepCertificate,
    phase: &'static str,
) -> Result<(), NativeStemsBeamVLinkSiblingLinksError> {
    if step.directed_pair.source_outgoing_scanned != 0
        || step.directed_pair.query_relation_count != 0
        || !step.directed_pair.relations.is_empty()
        || !step.directed_pair.source_outgoing_relations.is_empty()
        || step.directed_pair.source_outgoing_provenance
            != NativeStemsBeamSiblingQueryProvenance::NotRead
        || step.directed_pair.pair_provenance != NativeStemsBeamSiblingQueryProvenance::NotRead
        || step.stem_incident_after.is_some()
        || step.beam_incident_after.is_some()
        || step.chord_stem_matches != 0
        || step.builder_lookup.is_some()
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence { phase });
    }
    Ok(())
}

fn validate_no_callback(
    step: &NativeStemsBeamSiblingStepCertificate,
    phase: &'static str,
) -> Result<(), NativeStemsBeamVLinkSiblingLinksError> {
    if step.stem_incident_after.is_some()
        || step.beam_incident_after.is_some()
        || step.chord_stem_matches != 0
        || step.builder_lookup.is_some()
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence { phase });
    }
    Ok(())
}

fn sibling_geometry(
    stem: &NativeStemsBeamKnownSystemStem,
    base: &NativeStemsBeamStumpBeam,
    sibling: &NativeStemsBeamStumpBeam,
    base_cross: NativeStemPoint,
    y_direction: i32,
    certificate: &NativeStemsBeamVLinkSiblingLinksCertificate,
    support_grade: f64,
) -> NativeStemsBeamSiblingGeometryTrace {
    sibling_geometry_values(
        stem_segment(stem),
        base.median,
        sibling.median,
        sibling.height,
        base_cross,
        y_direction,
        certificate.portion_maximum_dx,
        certificate.max_shorter_ratio,
        support_grade,
    )
}

#[allow(clippy::too_many_arguments)]
fn sibling_geometry_values(
    stem_median: Segment,
    base_median: Segment,
    sibling_median: Segment,
    sibling_height: f64,
    base_cross: NativeStemPoint,
    y_direction: i32,
    portion_maximum_dx: i32,
    max_shorter_ratio: f64,
    support_grade: f64,
) -> NativeStemsBeamSiblingGeometryTrace {
    let sibling_cross = generic_intersection(stem_median, sibling_median);
    let base_length = base_median.x2 - base_median.x1;
    let sibling_length = sibling_median.x2 - sibling_median.x1;
    let length_ratio = sibling_length / base_length;
    let shorter_or_equal = length_ratio <= max_shorter_ratio;
    let (delta_y, directed_delta_y, wrong_side) = if shorter_or_equal {
        let delta_y = sibling_cross.y - base_cross.y;
        let directed = delta_y * f64::from(y_direction);
        (Some(delta_y), Some(directed), Some(directed < 0.0))
    } else {
        (None, None, None)
    };
    let proceeds = wrong_side != Some(true);
    let extension_point = proceeds.then(|| NativeStemPoint {
        x: sibling_cross.x,
        y: sibling_cross.y - (f64::from(y_direction) * (sibling_height / 2.0)),
    });
    let left_threshold = proceeds.then(|| sibling_median.x1 + f64::from(portion_maximum_dx));
    let right_threshold = proceeds.then(|| sibling_median.x2 - f64::from(portion_maximum_dx));
    let beam_portion = match (left_threshold, right_threshold) {
        (Some(left), Some(_)) if sibling_cross.x < left => Some(NativeBeamPortion::Left),
        (Some(_), Some(right)) if sibling_cross.x > right => Some(NativeBeamPortion::Right),
        (Some(_), Some(_)) => Some(NativeBeamPortion::Center),
        (None, None) => None,
        _ => unreachable!("portion thresholds are materialized together"),
    };
    NativeStemsBeamSiblingGeometryTrace {
        base_cross,
        sibling_cross,
        base_length,
        sibling_length,
        length_ratio,
        shorter_or_equal,
        delta_y,
        directed_delta_y,
        wrong_side,
        extension_point,
        portion_maximum_dx: proceeds.then_some(portion_maximum_dx),
        left_threshold,
        right_threshold,
        beam_portion,
        support_grade: proceeds.then_some(support_grade),
    }
}

fn stem_segment(stem: &NativeStemsBeamKnownSystemStem) -> Segment {
    Segment {
        x1: stem.geometry.median.start.x,
        y1: stem.geometry.median.start.y,
        x2: stem.geometry.median.stop.x,
        y2: stem.geometry.median.stop.y,
    }
}

fn next_relation_identity(
    base_state: &NativeStemsBeamVLinkBaseApplyState,
    sibling_appends: &[NativeStemsBeamSiblingAppendedRelation],
) -> Result<usize, NativeStemsBeamVLinkSiblingLinksError> {
    base_state
        .sig
        .baseline_relation_count
        .checked_add(base_state.sig.appended_relations.len())
        .and_then(|value| value.checked_add(sibling_appends.len()))
        .ok_or(NativeStemsBeamVLinkSiblingLinksError::UnsupportedV1 {
            phase: "SIG relation identity overflow",
        })
}

fn initial_relation_object_map(
    base_state: &NativeStemsBeamVLinkBaseApplyState,
    sibling_appends: &[NativeStemsBeamSiblingAppendedRelation],
) -> Result<
    BTreeMap<usize, NativeStemsBeamSiblingRelationObjectIdentity>,
    NativeStemsBeamVLinkSiblingLinksError,
> {
    let mut result = BTreeMap::new();
    for relation in &base_state.sig.appended_relations {
        let object = match relation.relation_object_identity {
            crate::native_stems_beam_vlink_base_apply::NativeStemsBeamRelationObjectIdentity::GraphObject(
                identity,
            ) => NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(identity),
            crate::native_stems_beam_vlink_base_apply::NativeStemsBeamRelationObjectIdentity::FreshDraft(
                plan,
            ) => NativeStemsBeamSiblingRelationObjectIdentity::BaseDraft(plan),
        };
        join_relation_object(&mut result, relation.graph_relation_identity, object)?;
    }
    for relation in sibling_appends {
        join_relation_object(
            &mut result,
            relation.graph_relation_identity,
            relation.relation_object_identity,
        )?;
    }
    Ok(result)
}

fn join_relation_object(
    map: &mut BTreeMap<usize, NativeStemsBeamSiblingRelationObjectIdentity>,
    graph_identity: usize,
    object_identity: NativeStemsBeamSiblingRelationObjectIdentity,
) -> Result<(), NativeStemsBeamVLinkSiblingLinksError> {
    if matches!(
        object_identity,
        NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(object)
            if object != graph_identity
    ) || map
        .get(&graph_identity)
        .is_some_and(|prior| *prior != object_identity)
        || map.iter().any(|(prior_graph, prior_object)| {
            *prior_graph != graph_identity && *prior_object == object_identity
        })
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "graph/relation-object identity consistency",
        });
    }
    map.insert(graph_identity, object_identity);
    Ok(())
}

fn relation_class_matches_kind(class: &str, kind: NativeStemsBeamQueryRelationKind) -> bool {
    match kind {
        NativeStemsBeamQueryRelationKind::BeamStem => class == BEAM_STEM_CLASS,
        NativeStemsBeamQueryRelationKind::BeamRest => class == BEAM_REST_CLASS,
        NativeStemsBeamQueryRelationKind::ChordStem => class == CHORD_STEM_CLASS,
        NativeStemsBeamQueryRelationKind::Other => {
            !class.is_empty()
                && class != BEAM_STEM_CLASS
                && class != BEAM_REST_CLASS
                && class != CHORD_STEM_CLASS
        }
    }
}

fn full_kind_matches_class(class: &str, kind: NativeStemsBeamSigRelationKind) -> bool {
    match kind {
        NativeStemsBeamSigRelationKind::BeamStem { .. } => class == BEAM_STEM_CLASS,
        NativeStemsBeamSigRelationKind::BeamRest { .. } => class == BEAM_REST_CLASS,
        NativeStemsBeamSigRelationKind::ChordStem => class == CHORD_STEM_CLASS,
        NativeStemsBeamSigRelationKind::Other => {
            !class.is_empty()
                && class != BEAM_STEM_CLASS
                && class != BEAM_REST_CLASS
                && class != CHORD_STEM_CLASS
        }
    }
}

fn initial_stem_scan(
    base_apply: &NativeStemsBeamVLinkBaseApplyTransaction,
    live_beams: &[NativeStemsBeamSiblingLiveBeam],
    stem_alias: &str,
) -> Result<
    Option<Vec<NativeStemsBeamSiblingStemIncidentRelation>>,
    NativeStemsBeamVLinkSiblingLinksError,
> {
    if !base_apply.callback.called {
        // A suppressed base apply did not read `edgesOf(stem)`. Its live
        // baseline is therefore established by the first sibling callback.
        return Ok(None);
    }
    let scan = &base_apply.consumed_certificate.stem_incident_after;
    if scan.state
        != crate::native_stems_beam_vlink_base_apply::NativeStemsBeamStemIncidentScanState::ExhaustiveIncomingThenOutgoing
        || scan.query_relation_count != scan.relations.len()
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::PredecessorMismatch);
    }
    scan.relations
        .iter()
        .map(|row| {
            let object = match row.relation_object_identity {
                crate::native_stems_beam_vlink_base_apply::NativeStemsBeamRelationObjectIdentity::GraphObject(
                    identity,
                ) => NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(identity),
                crate::native_stems_beam_vlink_base_apply::NativeStemsBeamRelationObjectIdentity::FreshDraft(
                    plan,
                ) => NativeStemsBeamSiblingRelationObjectIdentity::BaseDraft(plan),
            };
            let opposite_alias = match row.opposite {
                NativeStemsBeamIncidentOpposite::Beam => live_beams
                    .iter()
                    .find(|beam| {
                        beam.runtime.sig_vertex_identity == Some(row.opposite_vertex_ordinal)
                            && beam.runtime.inter_id == row.opposite_inter_id
                    })
                    .map(|beam| beam.alias.clone())
                    .ok_or(NativeStemsBeamVLinkSiblingLinksError::PredecessorMismatch)?,
                NativeStemsBeamIncidentOpposite::Stem => {
                    if row.opposite_inter_id != base_apply.stem_after.inter_id.unwrap_or_default() {
                        return Err(NativeStemsBeamVLinkSiblingLinksError::PredecessorMismatch);
                    }
                    stem_alias.to_owned()
                }
                NativeStemsBeamIncidentOpposite::OtherInter => {
                    format!("inter:{}", row.opposite_inter_id)
                }
            };
            Ok(NativeStemsBeamSiblingStemIncidentRelation {
                incident_ordinal: row.incident_ordinal,
                direction: row.direction,
                direction_ordinal: row.direction_ordinal,
                graph_relation_identity: row.graph_relation_identity,
                relation_object_identity: object,
                relation_class: row.relation_class.clone(),
                kind: row.kind,
                opposite_vertex_identity: row.opposite_vertex_ordinal,
                opposite: row.opposite,
                opposite_alias,
                opposite_inter_id: row.opposite_inter_id,
                chord_stem_match: row.chord_stem_match,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

#[allow(clippy::too_many_arguments)]
fn validate_stem_incident_scan(
    scan: &NativeStemsBeamSiblingStemIncidentScan,
    fresh: &NativeStemsBeamSiblingAppendedRelation,
    stem_inter_id: i32,
    sibling_inter_id: i32,
    sibling_alias: &str,
    graph_limit: usize,
    previous: Option<&[NativeStemsBeamSiblingStemIncidentRelation]>,
    relation_objects: &mut BTreeMap<usize, NativeStemsBeamSiblingRelationObjectIdentity>,
) -> Result<(), NativeStemsBeamVLinkSiblingLinksError> {
    if scan.query_relation_count != scan.relations.len()
        || !valid_sha256(&scan.query_provenance_sha256)
        || scan.query_provenance_sha256 != stem_incident_query_sha256(&scan.relations)
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "stem incident query header",
        });
    }
    validate_incident_chronology(
        scan.relations.iter().map(|row| {
            (
                row.incident_ordinal,
                row.direction,
                row.direction_ordinal,
                row.graph_relation_identity,
            )
        }),
        graph_limit,
        "stem incident chronology",
    )?;
    let mut fresh_matches = 0;
    let mut endpoints = EndpointIdentityCatalogue::default();
    for row in &scan.relations {
        if !relation_class_matches_kind(&row.relation_class, row.kind)
            || row.chord_stem_match
                != matches!(row.kind, NativeStemsBeamQueryRelationKind::ChordStem)
            || row.opposite_inter_id <= 0
            || row.opposite_alias.is_empty()
        {
            return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                phase: "stem incident relation payload",
            });
        }
        match row.opposite {
            NativeStemsBeamIncidentOpposite::Beam => {
                if row.opposite_inter_id == stem_inter_id
                    || row.opposite_vertex_identity == fresh.target_vertex_identity
                    || !valid_beam_alias(&row.opposite_alias)
                {
                    return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                        phase: "stem incident beam endpoint",
                    });
                }
            }
            NativeStemsBeamIncidentOpposite::Stem => {
                return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                    phase: "stem incident self endpoint",
                });
            }
            NativeStemsBeamIncidentOpposite::OtherInter => {
                if row.opposite_inter_id == stem_inter_id
                    || row.opposite_inter_id == sibling_inter_id
                    || row.opposite_vertex_identity == fresh.target_vertex_identity
                    || row.opposite_vertex_identity == fresh.source_vertex_identity
                    || row.opposite_alias != format!("inter:{}", row.opposite_inter_id)
                {
                    return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                        phase: "stem incident other endpoint collision",
                    });
                }
            }
        }
        endpoints.join(
            row.opposite_vertex_identity,
            row.opposite_inter_id,
            &row.opposite_alias,
            "stem incident endpoint identity bijection",
        )?;
        join_relation_object(
            relation_objects,
            row.graph_relation_identity,
            row.relation_object_identity,
        )?;
        if row.graph_relation_identity == fresh.graph_relation_identity {
            fresh_matches += 1;
            if row.relation_object_identity != fresh.relation_object_identity
                || row.direction != NativeStemsBeamIncidentDirection::Incoming
                || row.opposite != NativeStemsBeamIncidentOpposite::Beam
                || row.opposite_alias != sibling_alias
                || row.opposite_vertex_identity != fresh.source_vertex_identity
                || row.opposite_inter_id != sibling_inter_id
                || row.kind != NativeStemsBeamQueryRelationKind::BeamStem
                || row.chord_stem_match
            {
                return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                    phase: "fresh edge stem incidence",
                });
            }
        }
    }
    if fresh_matches != 1 {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "fresh stem incidence cardinality",
        });
    }
    if let Some(previous) = previous {
        let mut retained = scan
            .relations
            .iter()
            .filter(|row| row.graph_relation_identity != fresh.graph_relation_identity)
            .cloned()
            .collect::<Vec<_>>();
        renumber_stem_incidents(&mut retained);
        if retained.as_slice() != previous {
            return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                phase: "serial stem incident prefix",
            });
        }
    }
    Ok(())
}

struct BeamIncidentValidationContext<'a> {
    beam_kind: BeamKind,
    fresh: &'a NativeStemsBeamSiblingAppendedRelation,
    source_outgoing_before: &'a [NativeStemsBeamSiblingSourceOutgoingRelation],
    stem_inter_id: i32,
    sibling_inter_id: i32,
    stem_alias: &'a str,
    group_inter_id: i32,
    group_vertex: usize,
    group_alias: &'a str,
    live_group_members: &'a [NativeStemsBeamSiblingLiveBeam],
    graph_limit: usize,
    abnormal_before: bool,
}

fn validate_beam_incident_scan(
    scan: &NativeStemsBeamSiblingBeamIncidentScan,
    context: BeamIncidentValidationContext<'_>,
    relation_objects: &mut BTreeMap<usize, NativeStemsBeamSiblingRelationObjectIdentity>,
) -> Result<(bool, NativeStemsBeamSiblingBeamAbnormalTrace), NativeStemsBeamVLinkSiblingLinksError>
{
    let BeamIncidentValidationContext {
        beam_kind,
        fresh,
        source_outgoing_before,
        stem_inter_id,
        sibling_inter_id,
        stem_alias,
        group_inter_id,
        group_vertex,
        group_alias,
        live_group_members,
        graph_limit,
        abnormal_before,
    } = context;
    if scan.query_relation_count != scan.relations.len()
        || !valid_sha256(&scan.query_provenance_sha256)
        || scan.query_provenance_sha256
            != beam_incident_query_sha256(&scan.relations, beam_kind == BeamKind::Hook)
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "beam incident query header",
        });
    }
    validate_incident_chronology(
        scan.relations.iter().map(|row| {
            (
                row.incident_ordinal,
                row.direction,
                row.direction_ordinal,
                row.graph_relation_identity,
            )
        }),
        graph_limit,
        "beam incident chronology",
    )?;
    let hook = beam_kind == BeamKind::Hook;
    let expected_rule = if hook {
        NativeStemsBeamBeamIncidentRule::HookHasAnyBeamStem
    } else {
        NativeStemsBeamBeamIncidentRule::RawBeamLeftAndRight
    };
    if scan.rule != expected_rule {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "polymorphic beam abnormal rule",
        });
    }
    let first_beam_stem = scan
        .relations
        .iter()
        .position(|row| matches!(row.kind, NativeStemsBeamSigRelationKind::BeamStem { .. }));
    let mut fresh_matches = 0;
    let mut left_found = false;
    let mut right_found = false;
    let mut relations_read = 0;
    let mut endpoints = EndpointIdentityCatalogue::default();
    for (index, row) in scan.relations.iter().enumerate() {
        if !full_kind_matches_class(&row.relation_class, row.kind)
            || row.opposite_inter_id <= 0
            || row.opposite_alias.is_empty()
        {
            return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                phase: "beam incident relation payload",
            });
        }
        match row.opposite {
            NativeStemsBeamIncidentOpposite::Stem => {
                if row.opposite_inter_id == sibling_inter_id
                    || row.opposite_vertex_identity == fresh.source_vertex_identity
                    || row.opposite_inter_id != stem_inter_id
                    || row.opposite_vertex_identity != fresh.target_vertex_identity
                    || row.opposite_alias != stem_alias
                {
                    return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                        phase: "beam incident stem endpoint",
                    });
                }
            }
            NativeStemsBeamIncidentOpposite::Beam => {
                let matches = live_group_members
                    .iter()
                    .filter(|member| {
                        member.alias == row.opposite_alias
                            && member.runtime.inter_id == row.opposite_inter_id
                            && member.runtime.sig_vertex_identity
                                == Some(row.opposite_vertex_identity)
                    })
                    .count();
                if matches != 1
                    || row.opposite_inter_id == sibling_inter_id
                    || row.opposite_vertex_identity == fresh.source_vertex_identity
                {
                    return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                        phase: "beam incident beam endpoint",
                    });
                }
            }
            NativeStemsBeamIncidentOpposite::OtherInter => {
                let is_group = row.opposite_inter_id == group_inter_id
                    && row.opposite_vertex_identity == group_vertex
                    && row.opposite_alias == group_alias;
                if row.opposite_inter_id == stem_inter_id
                    || row.opposite_inter_id == sibling_inter_id
                    || row.opposite_vertex_identity == fresh.target_vertex_identity
                    || row.opposite_vertex_identity == fresh.source_vertex_identity
                    || (!is_group
                        && row.opposite_alias != format!("inter:{}", row.opposite_inter_id))
                {
                    return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                        phase: "beam incident other endpoint collision",
                    });
                }
            }
        }
        endpoints.join(
            row.opposite_vertex_identity,
            row.opposite_inter_id,
            &row.opposite_alias,
            "beam incident endpoint identity bijection",
        )?;
        join_relation_object(
            relation_objects,
            row.graph_relation_identity,
            row.relation_object_identity,
        )?;
        if hook {
            let expected_read = if first_beam_stem.is_some_and(|first| index > first) {
                NativeStemsBeamBeamIncidentRead::UnreadAfterBreak
            } else {
                NativeStemsBeamBeamIncidentRead::Examined
            };
            let expected_relevant = first_beam_stem == Some(index);
            if row.read != expected_read
                || row.relevant != expected_relevant
                || row.beam_portion.is_some()
            {
                return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                    phase: "hook lazy abnormal scan",
                });
            }
            if row.read == NativeStemsBeamBeamIncidentRead::Examined {
                relations_read += 1;
            }
        } else {
            let relation_portion = match row.kind {
                NativeStemsBeamSigRelationKind::BeamStem { beam_portion }
                | NativeStemsBeamSigRelationKind::BeamRest { beam_portion } => beam_portion,
                NativeStemsBeamSigRelationKind::ChordStem
                | NativeStemsBeamSigRelationKind::Other => None,
            };
            let relevant = relation_portion.is_some();
            if row.read != NativeStemsBeamBeamIncidentRead::Examined
                || row.relevant != relevant
                || row.beam_portion != relation_portion
            {
                return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                    phase: "raw beam exhaustive abnormal scan",
                });
            }
            match relation_portion {
                Some(NativeBeamPortion::Left) => left_found = true,
                Some(NativeBeamPortion::Right) => right_found = true,
                Some(NativeBeamPortion::Center) | None => {}
            }
            relations_read += 1;
        }
        if row.graph_relation_identity == fresh.graph_relation_identity {
            fresh_matches += 1;
            if row.relation_object_identity != fresh.relation_object_identity
                || row.direction != NativeStemsBeamIncidentDirection::Outgoing
                || row.opposite != NativeStemsBeamIncidentOpposite::Stem
                || row.opposite_alias != stem_alias
                || row.opposite_vertex_identity != fresh.target_vertex_identity
                || row.opposite_inter_id != stem_inter_id
                || row.kind
                    != (NativeStemsBeamSigRelationKind::BeamStem {
                        beam_portion: Some(fresh.beam_portion),
                    })
            {
                return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                    phase: "fresh edge beam incidence",
                });
            }
        }
    }
    if fresh_matches != 1 {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "fresh beam incidence cardinality",
        });
    }
    let outgoing = scan
        .relations
        .iter()
        .filter(|row| row.direction == NativeStemsBeamIncidentDirection::Outgoing)
        .collect::<Vec<_>>();
    if outgoing.len() != source_outgoing_before.len() + 1
        || outgoing
            .iter()
            .take(source_outgoing_before.len())
            .zip(source_outgoing_before)
            .any(|(after, before)| {
                after.graph_relation_identity != before.graph_relation_identity
                    || after.relation_object_identity != before.relation_object_identity
                    || after.relation_class != before.relation_class
            })
        || outgoing.last().is_none_or(|row| {
            row.graph_relation_identity != fresh.graph_relation_identity
                || row.relation_object_identity != fresh.relation_object_identity
                || row.relation_class != BEAM_STEM_CLASS
        })
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "beam post-edge outgoing projection",
        });
    }
    if hook {
        let after = first_beam_stem.is_none();
        Ok((
            after,
            NativeStemsBeamSiblingBeamAbnormalTrace::HookAnyBeamStem {
                incident_relation_count: scan.relations.len(),
                relations_read,
                before: abnormal_before,
                after,
            },
        ))
    } else {
        let after = !(left_found && right_found);
        Ok((
            after,
            NativeStemsBeamSiblingBeamAbnormalTrace::RawBeamSides {
                incident_relation_count: scan.relations.len(),
                left_found,
                right_found,
                before: abnormal_before,
                after,
            },
        ))
    }
}

fn validate_incident_chronology(
    rows: impl IntoIterator<Item = (usize, NativeStemsBeamIncidentDirection, usize, usize)>,
    graph_limit: usize,
    phase: &'static str,
) -> Result<(), NativeStemsBeamVLinkSiblingLinksError> {
    let rows = rows.into_iter().collect::<Vec<_>>();
    let mut outgoing = false;
    let mut next_incoming = 0;
    let mut next_outgoing = 0;
    let mut last_incoming = None;
    let mut last_outgoing = None;
    for (incident_ordinal, direction, direction_ordinal, graph_identity) in rows {
        if incident_ordinal != next_incoming + next_outgoing || graph_identity >= graph_limit {
            return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence { phase });
        }
        match direction {
            NativeStemsBeamIncidentDirection::Incoming if !outgoing => {
                if direction_ordinal != next_incoming
                    || last_incoming.is_some_and(|prior| prior >= graph_identity)
                {
                    return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence { phase });
                }
                next_incoming += 1;
                last_incoming = Some(graph_identity);
            }
            NativeStemsBeamIncidentDirection::Outgoing => {
                outgoing = true;
                if direction_ordinal != next_outgoing
                    || last_outgoing.is_some_and(|prior| prior >= graph_identity)
                {
                    return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence { phase });
                }
                next_outgoing += 1;
                last_outgoing = Some(graph_identity);
            }
            NativeStemsBeamIncidentDirection::Incoming => {
                return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence { phase });
            }
        }
    }
    Ok(())
}

fn renumber_stem_incidents(rows: &mut [NativeStemsBeamSiblingStemIncidentRelation]) {
    let mut incoming = 0;
    let mut outgoing = 0;
    for (incident_ordinal, row) in rows.iter_mut().enumerate() {
        row.incident_ordinal = incident_ordinal;
        match row.direction {
            NativeStemsBeamIncidentDirection::Incoming => {
                row.direction_ordinal = incoming;
                incoming += 1;
            }
            NativeStemsBeamIncidentDirection::Outgoing => {
                row.direction_ordinal = outgoing;
                outgoing += 1;
            }
        }
    }
}

fn valid_beam_alias(alias: &str) -> bool {
    alias
        .strip_prefix("beam:")
        .is_some_and(|ordinal| !ordinal.is_empty() && ordinal.parse::<usize>().is_ok())
}

fn append_edge_operations(
    sibling_ordinal: usize,
    graph_relation_identity: usize,
    operations: &mut Vec<NativeStemsBeamVLinkSiblingLinksOperation>,
) {
    operations.push(
        NativeStemsBeamVLinkSiblingLinksOperation::SigGlobalRelationInserted {
            sibling_ordinal,
            graph_relation_identity,
        },
    );
    operations.push(
        NativeStemsBeamVLinkSiblingLinksOperation::BeamOutgoingRelationInserted {
            sibling_ordinal,
            graph_relation_identity,
        },
    );
    operations.push(
        NativeStemsBeamVLinkSiblingLinksOperation::StemIncomingRelationInserted {
            sibling_ordinal,
            graph_relation_identity,
        },
    );
    operations.push(
        NativeStemsBeamVLinkSiblingLinksOperation::SigEdgeEventDispatched {
            sibling_ordinal,
            graph_relation_identity,
        },
    );
    operations.push(
        NativeStemsBeamVLinkSiblingLinksOperation::StandardSigListenerEdgeCallbackStarted {
            sibling_ordinal,
        },
    );
    operations.push(
        NativeStemsBeamVLinkSiblingLinksOperation::BeamStemRelationCallbackStarted {
            sibling_ordinal,
        },
    );
}

fn validate_builder_lookup(
    scan: &NativeStemsBeamSiblingBuilderLookupScan,
    builder: &crate::native_stems_beam_builders::NativeStemsBeamBuilder,
    sibling: NativeStemsBeamSource,
    live_beams: &[NativeStemsBeamSiblingLiveBeam],
) -> Result<Option<NativeStemsBeamBLinkerRef>, NativeStemsBeamVLinkSiblingLinksError> {
    validate_builder_lookup_items(scan, &builder.items, builder.start, sibling, live_beams)
}

fn validate_builder_lookup_items(
    scan: &NativeStemsBeamSiblingBuilderLookupScan,
    items: &[crate::native_stems_beam_builders::NativeStemsBeamBuilderItem],
    start: crate::native_stems_beam_vlinkers::NativeStemsBeamVLinkerRef,
    sibling: NativeStemsBeamSource,
    live_beams: &[NativeStemsBeamSiblingLiveBeam],
) -> Result<Option<NativeStemsBeamBLinkerRef>, NativeStemsBeamVLinkSiblingLinksError> {
    if scan.query_item_count != scan.rows.len()
        || scan.rows.len() != items.len()
        || !valid_sha256(&scan.query_provenance_sha256)
        || scan.query_provenance_sha256 != builder_lookup_query_sha256(&scan.rows)
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "builder lookup query header",
        });
    }

    let mut selected = None;
    let mut selected_alias = None;
    let mut source_aliases = BTreeMap::new();
    let mut source_ids = BTreeMap::new();
    for (item_ordinal, (row, item)) in scan.rows.iter().zip(items).enumerate() {
        if row.item_ordinal != item_ordinal || row.item_kind != item.kind {
            return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                phase: "builder item order/kind projection",
            });
        }
        if selected.is_some() {
            if row.read != NativeStemsBeamSiblingBuilderItemRead::UnreadAfterBreak
                || row.runtime_class.is_some()
                || row.linker_read != NativeStemsBeamSiblingBuilderLinkerRead::NotRead
                || row.source_read != NativeStemsBeamSiblingBuilderSourceRead::NotRead
                || row.linker.is_some()
                || row.source_beam.is_some()
                || row.linker_alias.is_some()
                || row.linker_runtime_class.is_some()
                || row.source_alias.is_some()
                || row.source_inter_id.is_some()
                || row.identity_match.is_some()
                || row.action != NativeStemsBeamSiblingBuilderAction::UnreadAfterBreak
            {
                return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                    phase: "builder lookup unread suffix",
                });
            }
            continue;
        }

        let runtime_class = builder_item_runtime_class(item.kind);
        if row.runtime_class.as_deref() != Some(runtime_class) {
            return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                phase: "builder item runtime class",
            });
        }
        match item.kind {
            NativeStemsBeamBuilderItemKind::SeedGlyph
            | NativeStemsBeamBuilderItemKind::ChunkGlyph
            | NativeStemsBeamBuilderItemKind::Gap => {
                if row.read != NativeStemsBeamSiblingBuilderItemRead::NotALinker
                    || row.linker_read != NativeStemsBeamSiblingBuilderLinkerRead::NotLinkerItem
                    || row.source_read != NativeStemsBeamSiblingBuilderSourceRead::NotRead
                    || row.linker.is_some()
                    || row.source_beam.is_some()
                    || row.linker_alias.is_some()
                    || row.linker_runtime_class.is_some()
                    || row.source_alias.is_some()
                    || row.source_inter_id.is_some()
                    || row.identity_match.is_some()
                    || row.action != NativeStemsBeamSiblingBuilderAction::Continue
                {
                    return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                        phase: "builder non-linker lazy row",
                    });
                }
            }
            NativeStemsBeamBuilderItemKind::StartHalfLinker => {
                let reference = start.b_linker;
                let (source_alias, source_inter_id) =
                    live_beam_identity(live_beams, reference.beam)?;
                let b_alias = b_linker_alias(source_alias, reference)?;
                let linker_alias = format!("{b_alias}:v:{}", vertical_side_token(start.side));
                validate_read_builder_linker_row(
                    row,
                    NativeStemsBeamSiblingBuilderLinkerIdentity::StartVLinker,
                    Some(reference.beam),
                    &linker_alias,
                    BEAM_V_LINKER_CLASS,
                    source_alias,
                    source_inter_id,
                    false,
                )?;
            }
            NativeStemsBeamBuilderItemKind::BeamLinker => {
                let Some(NativeStemsBeamBuilderTargetRef::Beam(reference)) = item.target else {
                    return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                        phase: "builder BeamLinker target shape",
                    });
                };
                let (source_alias, source_inter_id) =
                    live_beam_identity(live_beams, reference.beam)?;
                let linker_alias = b_linker_alias(source_alias, reference)?;
                let matches = reference.beam == sibling;
                validate_read_builder_linker_row(
                    row,
                    NativeStemsBeamSiblingBuilderLinkerIdentity::BeamBLinker(reference),
                    Some(reference.beam),
                    &linker_alias,
                    BEAM_B_LINKER_CLASS,
                    source_alias,
                    source_inter_id,
                    matches,
                )?;
                if matches {
                    selected = Some(reference);
                    selected_alias = Some(linker_alias);
                }
            }
            NativeStemsBeamBuilderItemKind::HeadHalfLinker => {
                let Some(NativeStemsBeamBuilderTargetRef::Head(reference)) = item.target else {
                    return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                        phase: "builder HeadHalfLinker target shape",
                    });
                };
                let linker_alias = format!(
                    "h:{}:{}:{}",
                    reference.sig_ordinal,
                    horizontal_side_token(reference.horizontal),
                    vertical_side_token(reference.vertical)
                );
                let source_alias = format!("head:{}", reference.sig_ordinal);
                let source_inter_id = row.source_inter_id.filter(|id| *id > 0).ok_or(
                    NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                        phase: "head builder source Java ID",
                    },
                )?;
                validate_read_builder_linker_row(
                    row,
                    NativeStemsBeamSiblingBuilderLinkerIdentity::HeadCLinker,
                    None,
                    &linker_alias,
                    HEAD_C_LINKER_CLASS,
                    &source_alias,
                    source_inter_id,
                    false,
                )?;
            }
        }
        if let (Some(alias), Some(inter_id)) = (&row.source_alias, row.source_inter_id) {
            if source_aliases
                .insert(alias.clone(), inter_id)
                .is_some_and(|prior| prior != inter_id)
                || source_ids
                    .insert(inter_id, alias.clone())
                    .is_some_and(|prior| prior != *alias)
            {
                return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
                    phase: "builder source alias/Java-ID bijection",
                });
            }
        }
    }
    let expected_state = if selected.is_some() {
        NativeStemsBeamSiblingBuilderLookupState::FirstSourceIdentityMatch
    } else {
        NativeStemsBeamSiblingBuilderLookupState::ExhaustiveNoMatch
    };
    if scan.state != expected_state
        || scan.timing != NativeStemsBeamSiblingBuilderLookupTiming::ReconstructedFromImmutableItems
        || scan.selected_b_linker != selected
        || scan.selected_alias != selected_alias
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "builder lookup selection/result",
        });
    }
    Ok(selected)
}

#[allow(clippy::too_many_arguments)]
fn validate_read_builder_linker_row(
    row: &NativeStemsBeamSiblingBuilderLookupRow,
    linker: NativeStemsBeamSiblingBuilderLinkerIdentity,
    source_beam: Option<NativeStemsBeamSource>,
    linker_alias: &str,
    linker_runtime_class: &str,
    source_alias: &str,
    source_inter_id: i32,
    matches: bool,
) -> Result<(), NativeStemsBeamVLinkSiblingLinksError> {
    let expected_read = if matches {
        NativeStemsBeamSiblingBuilderItemRead::ExaminedSelectBreak
    } else {
        NativeStemsBeamSiblingBuilderItemRead::ExaminedContinue
    };
    let expected_action = if matches {
        NativeStemsBeamSiblingBuilderAction::SelectBreak
    } else {
        NativeStemsBeamSiblingBuilderAction::Continue
    };
    if row.read != expected_read
        || row.linker_read != NativeStemsBeamSiblingBuilderLinkerRead::ReadLinker
        || row.source_read != NativeStemsBeamSiblingBuilderSourceRead::ReadSource
        || row.linker != Some(linker)
        || row.source_beam != source_beam
        || row.linker_alias.as_deref() != Some(linker_alias)
        || row.linker_runtime_class.as_deref() != Some(linker_runtime_class)
        || row.source_alias.as_deref() != Some(source_alias)
        || row.source_inter_id != Some(source_inter_id)
        || row.identity_match != Some(matches)
        || row.action != expected_action
    {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidEvidence {
            phase: "builder linker lookup row",
        });
    }
    Ok(())
}

fn builder_item_runtime_class(kind: NativeStemsBeamBuilderItemKind) -> &'static str {
    match kind {
        NativeStemsBeamBuilderItemKind::StartHalfLinker
        | NativeStemsBeamBuilderItemKind::HeadHalfLinker => STEM_HALF_LINKER_ITEM_CLASS,
        NativeStemsBeamBuilderItemKind::BeamLinker => STEM_LINKER_ITEM_CLASS,
        NativeStemsBeamBuilderItemKind::SeedGlyph | NativeStemsBeamBuilderItemKind::ChunkGlyph => {
            STEM_GLYPH_ITEM_CLASS
        }
        NativeStemsBeamBuilderItemKind::Gap => STEM_GAP_ITEM_CLASS,
    }
}

fn live_beam_identity(
    live_beams: &[NativeStemsBeamSiblingLiveBeam],
    source: NativeStemsBeamSource,
) -> Result<(&str, i32), NativeStemsBeamVLinkSiblingLinksError> {
    let matches = live_beams
        .iter()
        .filter(|beam| beam.source == source)
        .collect::<Vec<_>>();
    if matches.len() != 1 || matches[0].alias.is_empty() || matches[0].runtime.inter_id <= 0 {
        return Err(NativeStemsBeamVLinkSiblingLinksError::UnsupportedV1 {
            phase: "builder beam source outside compact live-group catalogue",
        });
    }
    Ok((&matches[0].alias, matches[0].runtime.inter_id))
}

fn b_linker_alias(
    beam_alias: &str,
    reference: NativeStemsBeamBLinkerRef,
) -> Result<String, NativeStemsBeamVLinkSiblingLinksError> {
    let ordinal =
        reference
            .id
            .checked_sub(1)
            .ok_or(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
                phase: "B-linker one-based Java ID",
            })?;
    Ok(format!("{beam_alias}:b:{ordinal}"))
}

fn horizontal_side_token(side: NativeStemHeadSide) -> &'static str {
    match side {
        NativeStemHeadSide::Left => "LEFT",
        NativeStemHeadSide::Right => "RIGHT",
    }
}

fn vertical_side_token(side: NativeStemVerticalSide) -> &'static str {
    match side {
        NativeStemVerticalSide::Top => "TOP",
        NativeStemVerticalSide::Bottom => "BOTTOM",
    }
}

#[cfg(test)]
fn structural_builder_lookup(
    items: &[crate::native_stems_beam_builders::NativeStemsBeamBuilderItem],
    start: NativeStemsBeamBLinkerRef,
    sibling: NativeStemsBeamSource,
) -> (
    Vec<NativeStemsBeamSiblingBuilderItemRead>,
    Option<NativeStemsBeamBLinkerRef>,
) {
    let mut selected = None;
    let reads = items
        .iter()
        .map(|item| {
            if selected.is_some() {
                return NativeStemsBeamSiblingBuilderItemRead::UnreadAfterBreak;
            }
            let (is_linker, candidate) = match item.kind {
                NativeStemsBeamBuilderItemKind::StartHalfLinker => (true, Some(start)),
                NativeStemsBeamBuilderItemKind::BeamLinker => match item.target {
                    Some(NativeStemsBeamBuilderTargetRef::Beam(reference)) => {
                        (true, Some(reference))
                    }
                    _ => (false, None),
                },
                NativeStemsBeamBuilderItemKind::HeadHalfLinker => (true, None),
                NativeStemsBeamBuilderItemKind::SeedGlyph
                | NativeStemsBeamBuilderItemKind::ChunkGlyph
                | NativeStemsBeamBuilderItemKind::Gap => (false, None),
            };
            match candidate {
                Some(reference) if reference.beam == sibling => {
                    selected = Some(reference);
                    NativeStemsBeamSiblingBuilderItemRead::ExaminedSelectBreak
                }
                Some(_) => NativeStemsBeamSiblingBuilderItemRead::ExaminedContinue,
                None if is_linker => NativeStemsBeamSiblingBuilderItemRead::ExaminedContinue,
                None => NativeStemsBeamSiblingBuilderItemRead::NotALinker,
            }
        })
        .collect();
    (reads, selected)
}

fn validate_unique_cells(
    cells: &[NativeStemsBeamSiblingBLinkerCell],
) -> Result<(), NativeStemsBeamVLinkSiblingLinksError> {
    if cells.iter().enumerate().any(|(index, cell)| {
        cell.reference.id == 0
            || cells[..index]
                .iter()
                .any(|prior| prior.reference == cell.reference)
    }) {
        return Err(NativeStemsBeamVLinkSiblingLinksError::InvalidState {
            phase: "sibling B-linker cell identity",
        });
    }
    Ok(())
}

fn dirty_cascade(
    sibling_ordinal: usize,
    state: &mut NativeStemsBeamVLinkSiblingLinksState,
    operations: &mut Vec<NativeStemsBeamVLinkSiblingLinksOperation>,
) {
    state.sheet_edit.stub_modified = true;
    operations.push(
        NativeStemsBeamVLinkSiblingLinksOperation::SheetStubModifiedSetTrue { sibling_ordinal },
    );
    state.sheet_edit.book_modified = true;
    operations
        .push(NativeStemsBeamVLinkSiblingLinksOperation::BookModifiedSetTrue { sibling_ordinal });
    state.sheet_edit.book_dirty = true;
    operations
        .push(NativeStemsBeamVLinkSiblingLinksOperation::BookDirtySetTrue { sibling_ordinal });
}

fn sheet_edit_delta(
    before: NativeStemsBeamSheetEditState,
    after: NativeStemsBeamSheetEditState,
) -> usize {
    usize::from(before.stub_modified != after.stub_modified)
        + usize::from(before.book_modified != after.book_modified)
        + usize::from(before.book_dirty != after.book_dirty)
}

fn java_rint_i32(value: f64) -> i32 {
    value.round_ties_even() as i32
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

fn relation_object_token(identity: NativeStemsBeamSiblingRelationObjectIdentity) -> String {
    match identity {
        NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(identity) => {
            format!("sig-relation-object:{identity}")
        }
        NativeStemsBeamSiblingRelationObjectIdentity::BaseDraft(plan) => {
            format!("base-draft:{plan}")
        }
        NativeStemsBeamSiblingRelationObjectIdentity::SiblingDraft {
            plan_ordinal,
            sibling_ordinal,
        } => format!("sibling-draft:{plan_ordinal}:{sibling_ordinal}"),
    }
}

fn group_query_sha256(rows: &[NativeStemsBeamSiblingGroupRelation]) -> String {
    query_rows_sha256(rows.iter().map(|row| {
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            row.outgoing_ordinal,
            graph_relation_token(row.graph_relation_identity),
            relation_object_token(row.relation_object_identity),
            row.relation_class,
            row.target_alias,
            row.target_class,
            row.target_inter_id,
            row.target_vertex_identity,
            row.containment_match,
            row.member_ordinal
                .map_or_else(|| "-".to_owned(), |value| value.to_string()),
        )
    }))
}

fn source_outgoing_query_sha256(rows: &[NativeStemsBeamSiblingSourceOutgoingRelation]) -> String {
    query_rows_sha256(rows.iter().map(|row| {
        format!(
            "{}:{}:{}:{}",
            row.source_outgoing_ordinal,
            graph_relation_token(row.graph_relation_identity),
            relation_object_token(row.relation_object_identity),
            row.relation_class,
        )
    }))
}

fn pair_query_sha256(rows: &[NativeStemsBeamSiblingPairRelation]) -> String {
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

fn portion_token(portion: Option<NativeBeamPortion>) -> &'static str {
    match portion {
        None => "-",
        Some(NativeBeamPortion::Left) => "LEFT",
        Some(NativeBeamPortion::Center) => "CENTER",
        Some(NativeBeamPortion::Right) => "RIGHT",
    }
}

fn stem_incident_query_sha256(rows: &[NativeStemsBeamSiblingStemIncidentRelation]) -> String {
    query_rows_sha256(rows.iter().map(|row| {
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            row.incident_ordinal,
            incident_direction_token(row.direction),
            row.direction_ordinal,
            graph_relation_token(row.graph_relation_identity),
            relation_object_token(row.relation_object_identity),
            row.relation_class,
            row.opposite_alias,
            row.opposite_inter_id,
            row.opposite_vertex_identity,
            row.chord_stem_match,
        )
    }))
}

fn beam_incident_query_sha256(
    rows: &[NativeStemsBeamSiblingBeamIncidentRelation],
    hook: bool,
) -> String {
    query_rows_sha256(rows.iter().map(|row| {
        let read_state = match row.read {
            NativeStemsBeamBeamIncidentRead::UnreadAfterBreak => "UnreadAfterBreak",
            NativeStemsBeamBeamIncidentRead::Examined if !hook && row.beam_portion.is_some() => {
                "ExaminedClassAndPortion"
            }
            NativeStemsBeamBeamIncidentRead::Examined => "ExaminedClassOnly",
        };
        let contribution = if hook && row.relevant {
            "FirstBeamStem"
        } else {
            match row.beam_portion {
                Some(NativeBeamPortion::Left) => "Left",
                Some(NativeBeamPortion::Right) => "Right",
                Some(NativeBeamPortion::Center) | None => "None",
            }
        };
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            row.incident_ordinal,
            incident_direction_token(row.direction),
            row.direction_ordinal,
            graph_relation_token(row.graph_relation_identity),
            relation_object_token(row.relation_object_identity),
            row.relation_class,
            row.opposite_alias,
            row.opposite_inter_id,
            row.opposite_vertex_identity,
            read_state,
            row.relevant,
            portion_token(row.beam_portion),
            contribution,
        )
    }))
}

fn builder_lookup_query_sha256(rows: &[NativeStemsBeamSiblingBuilderLookupRow]) -> String {
    query_rows_sha256(rows.iter().map(|row| {
        let linker_read = match row.linker_read {
            NativeStemsBeamSiblingBuilderLinkerRead::NotRead => "NotRead",
            NativeStemsBeamSiblingBuilderLinkerRead::NotLinkerItem => "NotLinkerItem",
            NativeStemsBeamSiblingBuilderLinkerRead::ReadLinker => "ReadLinker",
        };
        let source_read = match row.source_read {
            NativeStemsBeamSiblingBuilderSourceRead::NotRead => "NotRead",
            NativeStemsBeamSiblingBuilderSourceRead::ReadSource => "ReadSource",
        };
        let action = match row.action {
            NativeStemsBeamSiblingBuilderAction::Continue => "Continue",
            NativeStemsBeamSiblingBuilderAction::SelectBreak => "SelectBreak",
            NativeStemsBeamSiblingBuilderAction::UnreadAfterBreak => "UnreadAfterBreak",
        };
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            row.item_ordinal,
            row.runtime_class.as_deref().unwrap_or("-"),
            linker_read,
            source_read,
            row.linker_alias.as_deref().unwrap_or("-"),
            row.linker_runtime_class.as_deref().unwrap_or("-"),
            row.source_alias.as_deref().unwrap_or("-"),
            row.source_inter_id
                .map_or_else(|| "-".to_owned(), |id| id.to_string()),
            row.identity_match
                .map_or_else(|| "-".to_owned(), |value| value.to_string()),
            action,
        )
    }))
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
        for (index, word) in words[..16].iter_mut().enumerate() {
            let offset = index * 4;
            *word = u32::from_be_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
        }
        for index in 16..64 {
            let low = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let high = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(low)
                .wrapping_add(words[index - 7])
                .wrapping_add(high);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = digest;
        for index in 0..64 {
            let sigma1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let first = h
                .wrapping_add(sigma1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let sigma0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let second = sigma0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(first);
            d = c;
            c = b;
            b = a;
            a = first.wrapping_add(second);
        }
        for (slot, value) in digest.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
    digest.iter().map(|word| format!("{word:08x}")).collect()
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        native_stems_beam_builders::NativeStemsBeamBuilderItem,
        stems_step::{NativeStemLine, NativeStemVerticalSide},
    };
    use audiveris_image::{
        run_table::{FOREGROUND, RunTable},
        section::Bounds,
    };

    const BASE: NativeStemsBeamSource = NativeStemsBeamSource::RawBeam(0);
    const SIBLING: NativeStemsBeamSource = NativeStemsBeamSource::RawBeam(1);
    const START_B: NativeStemsBeamBLinkerRef = NativeStemsBeamBLinkerRef { beam: BASE, id: 1 };
    const SIBLING_B: NativeStemsBeamBLinkerRef = NativeStemsBeamBLinkerRef {
        beam: SIBLING,
        id: 2,
    };

    fn horizontal(x1: f64, x2: f64, y: f64) -> Segment {
        Segment {
            x1,
            y1: y,
            x2,
            y2: y,
        }
    }

    fn vertical(x: f64) -> Segment {
        Segment {
            x1: x,
            y1: -100.0,
            x2: x,
            y2: 100.0,
        }
    }

    fn sha() -> String {
        "0".repeat(64)
    }

    fn object() -> NativeStemsBeamSiblingRelationObjectIdentity {
        NativeStemsBeamSiblingRelationObjectIdentity::SiblingDraft {
            plan_ordinal: 3,
            sibling_ordinal: 0,
        }
    }

    fn fresh(portion: NativeBeamPortion) -> NativeStemsBeamSiblingAppendedRelation {
        NativeStemsBeamSiblingAppendedRelation {
            graph_relation_identity: 7,
            relation_object_identity: object(),
            source: SIBLING,
            source_vertex_identity: 4,
            target_stem_identity: 2,
            target_vertex_identity: 5,
            extension_point: NativeStemPoint { x: 5.0, y: 8.0 },
            beam_portion: portion,
            grade: 0.7,
        }
    }

    #[test]
    fn shorter_wrong_side_stops_before_extension_portion_and_grade_reads() {
        let trace = sibling_geometry_values(
            vertical(5.0),
            horizontal(0.0, 100.0, 0.0),
            horizontal(0.0, 50.0, -10.0),
            8.0,
            NativeStemPoint { x: 5.0, y: 0.0 },
            1,
            5,
            0.8,
            0.7,
        );
        assert_eq!(trace.length_ratio, 0.5);
        assert_eq!(trace.delta_y, Some(-10.0));
        assert_eq!(trace.directed_delta_y, Some(-10.0));
        assert_eq!(trace.wrong_side, Some(true));
        assert!(trace.extension_point.is_none());
        assert!(trace.beam_portion.is_none());
        assert!(trace.support_grade.is_none());
    }

    #[test]
    fn longer_sibling_never_reads_shorter_side_delta_but_builds_relation() {
        let trace = sibling_geometry_values(
            vertical(5.0),
            horizontal(0.0, 100.0, 0.0),
            horizontal(0.0, 90.0, -10.0),
            8.0,
            NativeStemPoint { x: 5.0, y: 0.0 },
            1,
            5,
            0.8,
            0.7,
        );
        assert!(!trace.shorter_or_equal);
        assert!(trace.delta_y.is_none());
        assert!(trace.wrong_side.is_none());
        assert_eq!(
            trace.extension_point,
            Some(NativeStemPoint { x: 5.0, y: -14.0 })
        );
        assert_eq!(trace.beam_portion, Some(NativeBeamPortion::Center));
        assert_eq!(trace.support_grade, Some(0.7));
    }

    #[test]
    fn inclusive_shorter_and_strict_portion_thresholds_match_java() {
        let trace = sibling_geometry_values(
            vertical(5.0),
            horizontal(0.0, 100.0, 0.0),
            horizontal(0.0, 80.0, 10.0),
            10.0,
            NativeStemPoint { x: 5.0, y: 0.0 },
            1,
            5,
            0.8,
            0.7,
        );
        assert!(trace.shorter_or_equal);
        assert_eq!(trace.wrong_side, Some(false));
        assert_eq!(trace.beam_portion, Some(NativeBeamPortion::Center));
        assert_eq!(java_rint_i32(10.5), 10);
        assert_eq!(java_rint_i32(11.5), 12);
    }

    #[test]
    fn pair_lookup_is_first_runtime_beamstem_match_with_unread_suffix() {
        let source_outgoing_relations =
            vec![
                NativeStemsBeamSiblingSourceOutgoingRelation {
                    source_outgoing_ordinal: 0,
                    graph_relation_identity: 1,
                    relation_object_identity:
                        NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(1),
                    relation_class: BEAM_REST_CLASS.into(),
                },
                NativeStemsBeamSiblingSourceOutgoingRelation {
                    source_outgoing_ordinal: 1,
                    graph_relation_identity: 2,
                    relation_object_identity:
                        NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(2),
                    relation_class: "org.audiveris.omr.sig.relation.OtherRelation".into(),
                },
                NativeStemsBeamSiblingSourceOutgoingRelation {
                    source_outgoing_ordinal: 2,
                    graph_relation_identity: 3,
                    relation_object_identity:
                        NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(3),
                    relation_class: BEAM_STEM_CLASS.into(),
                },
                NativeStemsBeamSiblingSourceOutgoingRelation {
                    source_outgoing_ordinal: 3,
                    graph_relation_identity: 4,
                    relation_object_identity:
                        NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(4),
                    relation_class: CHORD_STEM_CLASS.into(),
                },
            ];
        let pair_relations =
            vec![
                NativeStemsBeamSiblingPairRelation {
                    pair_ordinal: 0,
                    source_outgoing_ordinal: 0,
                    graph_relation_identity: 1,
                    relation_object_identity:
                        NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(1),
                    relation_class: BEAM_REST_CLASS.into(),
                    kind: NativeStemsBeamQueryRelationKind::BeamRest,
                    class_read: NativeStemsBeamSiblingPairClassRead::ExaminedContinue,
                },
                NativeStemsBeamSiblingPairRelation {
                    pair_ordinal: 1,
                    source_outgoing_ordinal: 2,
                    graph_relation_identity: 3,
                    relation_object_identity:
                        NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(3),
                    relation_class: BEAM_STEM_CLASS.into(),
                    kind: NativeStemsBeamQueryRelationKind::BeamStem,
                    class_read: NativeStemsBeamSiblingPairClassRead::ExaminedMatchBreak,
                },
                NativeStemsBeamSiblingPairRelation {
                    pair_ordinal: 2,
                    source_outgoing_ordinal: 3,
                    graph_relation_identity: 4,
                    relation_object_identity:
                        NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(4),
                    relation_class: CHORD_STEM_CLASS.into(),
                    kind: NativeStemsBeamQueryRelationKind::ChordStem,
                    class_read: NativeStemsBeamSiblingPairClassRead::UnreadAfterBreak,
                },
            ];
        let step = NativeStemsBeamSiblingStepCertificate {
            sibling_ordinal: 0,
            source: SIBLING,
            directed_pair: NativeStemsBeamSiblingPairScan {
                source_outgoing_scanned: 4,
                source_outgoing_provenance: NativeStemsBeamSiblingQueryProvenance::ExhaustiveSha256(
                    source_outgoing_query_sha256(&source_outgoing_relations),
                ),
                source_outgoing_relations,
                query_relation_count: 3,
                pair_provenance: NativeStemsBeamSiblingQueryProvenance::ExhaustiveSha256(
                    pair_query_sha256(&pair_relations),
                ),
                relations: pair_relations,
            },
            stem_incident_after: None,
            beam_incident_after: None,
            chord_stem_matches: 0,
            builder_lookup: None,
        };
        let result = validate_pair_scan(&step, 8, &mut BTreeMap::new()).unwrap();
        assert_eq!(result.first_match, Some(1));
        assert_eq!(result.relations_read, 2);
    }

    #[test]
    fn same_glyph_requires_explicit_not_read_pair_provenance() {
        let mut step = NativeStemsBeamSiblingStepCertificate {
            sibling_ordinal: 0,
            source: SIBLING,
            directed_pair: NativeStemsBeamSiblingPairScan {
                source_outgoing_scanned: 0,
                source_outgoing_provenance: NativeStemsBeamSiblingQueryProvenance::NotRead,
                source_outgoing_relations: Vec::new(),
                query_relation_count: 0,
                pair_provenance: NativeStemsBeamSiblingQueryProvenance::NotRead,
                relations: Vec::new(),
            },
            stem_incident_after: None,
            beam_incident_after: None,
            chord_stem_matches: 0,
            builder_lookup: None,
        };
        assert!(validate_unread_step(&step, "test").is_ok());
        step.directed_pair.pair_provenance =
            NativeStemsBeamSiblingQueryProvenance::ExhaustiveSha256(sha());
        assert!(validate_unread_step(&step, "test").is_err());
    }

    #[test]
    fn hook_callback_stops_at_first_beamstem_and_preserves_no_portion_read() {
        let fresh = fresh(NativeBeamPortion::Left);
        let mut scan = NativeStemsBeamSiblingBeamIncidentScan {
            rule: NativeStemsBeamBeamIncidentRule::HookHasAnyBeamStem,
            query_relation_count: 1,
            query_provenance_sha256: String::new(),
            relations: vec![NativeStemsBeamSiblingBeamIncidentRelation {
                incident_ordinal: 0,
                direction: NativeStemsBeamIncidentDirection::Outgoing,
                direction_ordinal: 0,
                graph_relation_identity: 7,
                relation_object_identity: object(),
                relation_class: BEAM_STEM_CLASS.into(),
                kind: NativeStemsBeamSigRelationKind::BeamStem {
                    beam_portion: Some(NativeBeamPortion::Left),
                },
                opposite_vertex_identity: 5,
                opposite: NativeStemsBeamIncidentOpposite::Stem,
                opposite_alias: "stem:9".into(),
                opposite_inter_id: 9,
                read: NativeStemsBeamBeamIncidentRead::Examined,
                relevant: true,
                beam_portion: None,
            }],
        };
        scan.query_provenance_sha256 = beam_incident_query_sha256(&scan.relations, true);
        let (after, trace) = validate_beam_incident_scan(
            &scan,
            BeamIncidentValidationContext {
                beam_kind: BeamKind::Hook,
                fresh: &fresh,
                source_outgoing_before: &[],
                stem_inter_id: 9,
                sibling_inter_id: 8,
                stem_alias: "stem:9",
                group_inter_id: 30,
                group_vertex: 20,
                group_alias: "beam-group:20",
                live_group_members: &[],
                graph_limit: 8,
                abnormal_before: true,
            },
            &mut BTreeMap::new(),
        )
        .unwrap();
        assert!(!after);
        assert!(matches!(
            trace,
            NativeStemsBeamSiblingBeamAbnormalTrace::HookAnyBeamStem {
                relations_read: 1,
                ..
            }
        ));
    }

    #[test]
    fn beam_callback_accepts_nonself_live_beam_incident_endpoint() {
        let fresh = fresh(NativeBeamPortion::Left);
        let other = live_beam(NativeStemsBeamSource::RawBeam(2), "beam:2", 6, 10);
        let mut scan = NativeStemsBeamSiblingBeamIncidentScan {
            rule: NativeStemsBeamBeamIncidentRule::HookHasAnyBeamStem,
            query_relation_count: 2,
            query_provenance_sha256: String::new(),
            relations: vec![
                NativeStemsBeamSiblingBeamIncidentRelation {
                    incident_ordinal: 0,
                    direction: NativeStemsBeamIncidentDirection::Incoming,
                    direction_ordinal: 0,
                    graph_relation_identity: 5,
                    relation_object_identity:
                        NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(5),
                    relation_class: "org.audiveris.omr.sig.relation.Exclusion".into(),
                    kind: NativeStemsBeamSigRelationKind::Other,
                    opposite_vertex_identity: 6,
                    opposite: NativeStemsBeamIncidentOpposite::Beam,
                    opposite_alias: "beam:2".into(),
                    opposite_inter_id: 10,
                    read: NativeStemsBeamBeamIncidentRead::Examined,
                    relevant: false,
                    beam_portion: None,
                },
                NativeStemsBeamSiblingBeamIncidentRelation {
                    incident_ordinal: 1,
                    direction: NativeStemsBeamIncidentDirection::Outgoing,
                    direction_ordinal: 0,
                    graph_relation_identity: 7,
                    relation_object_identity: object(),
                    relation_class: BEAM_STEM_CLASS.into(),
                    kind: NativeStemsBeamSigRelationKind::BeamStem {
                        beam_portion: Some(NativeBeamPortion::Left),
                    },
                    opposite_vertex_identity: 5,
                    opposite: NativeStemsBeamIncidentOpposite::Stem,
                    opposite_alias: "stem:9".into(),
                    opposite_inter_id: 9,
                    read: NativeStemsBeamBeamIncidentRead::Examined,
                    relevant: true,
                    beam_portion: None,
                },
            ],
        };
        scan.query_provenance_sha256 = beam_incident_query_sha256(&scan.relations, true);
        assert!(
            validate_beam_incident_scan(
                &scan,
                BeamIncidentValidationContext {
                    beam_kind: BeamKind::Hook,
                    fresh: &fresh,
                    source_outgoing_before: &[],
                    stem_inter_id: 9,
                    sibling_inter_id: 8,
                    stem_alias: "stem:9",
                    group_inter_id: 30,
                    group_vertex: 20,
                    group_alias: "beam-group:20",
                    live_group_members: &[other],
                    graph_limit: 8,
                    abnormal_before: true,
                },
                &mut BTreeMap::new(),
            )
            .is_ok()
        );
    }

    #[test]
    fn raw_callback_requires_both_side_portions() {
        let fresh = fresh(NativeBeamPortion::Left);
        let mut scan = NativeStemsBeamSiblingBeamIncidentScan {
            rule: NativeStemsBeamBeamIncidentRule::RawBeamLeftAndRight,
            query_relation_count: 1,
            query_provenance_sha256: String::new(),
            relations: vec![NativeStemsBeamSiblingBeamIncidentRelation {
                incident_ordinal: 0,
                direction: NativeStemsBeamIncidentDirection::Outgoing,
                direction_ordinal: 0,
                graph_relation_identity: 7,
                relation_object_identity: object(),
                relation_class: BEAM_STEM_CLASS.into(),
                kind: NativeStemsBeamSigRelationKind::BeamStem {
                    beam_portion: Some(NativeBeamPortion::Left),
                },
                opposite_vertex_identity: 5,
                opposite: NativeStemsBeamIncidentOpposite::Stem,
                opposite_alias: "stem:9".into(),
                opposite_inter_id: 9,
                read: NativeStemsBeamBeamIncidentRead::Examined,
                relevant: true,
                beam_portion: Some(NativeBeamPortion::Left),
            }],
        };
        scan.query_provenance_sha256 = beam_incident_query_sha256(&scan.relations, false);
        let (after, _) = validate_beam_incident_scan(
            &scan,
            BeamIncidentValidationContext {
                beam_kind: BeamKind::Beam,
                fresh: &fresh,
                source_outgoing_before: &[],
                stem_inter_id: 9,
                sibling_inter_id: 8,
                stem_alias: "stem:9",
                group_inter_id: 30,
                group_vertex: 20,
                group_alias: "beam-group:20",
                live_group_members: &[],
                graph_limit: 8,
                abnormal_before: false,
            },
            &mut BTreeMap::new(),
        )
        .unwrap();
        assert!(after);
        scan.relations.insert(
            0,
            NativeStemsBeamSiblingBeamIncidentRelation {
                incident_ordinal: 0,
                direction: NativeStemsBeamIncidentDirection::Outgoing,
                direction_ordinal: 0,
                graph_relation_identity: 6,
                relation_object_identity: NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(
                    6,
                ),
                relation_class: BEAM_REST_CLASS.into(),
                kind: NativeStemsBeamSigRelationKind::BeamRest {
                    beam_portion: Some(NativeBeamPortion::Right),
                },
                opposite_vertex_identity: 10,
                opposite: NativeStemsBeamIncidentOpposite::OtherInter,
                opposite_alias: "inter:11".into(),
                opposite_inter_id: 11,
                read: NativeStemsBeamBeamIncidentRead::Examined,
                relevant: true,
                beam_portion: Some(NativeBeamPortion::Right),
            },
        );
        scan.query_relation_count = 2;
        scan.relations[1].incident_ordinal = 1;
        scan.relations[1].direction_ordinal = 1;
        scan.query_provenance_sha256 = beam_incident_query_sha256(&scan.relations, false);
        let source_outgoing = [NativeStemsBeamSiblingSourceOutgoingRelation {
            source_outgoing_ordinal: 0,
            graph_relation_identity: 6,
            relation_object_identity: NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(6),
            relation_class: BEAM_REST_CLASS.into(),
        }];
        let (after, _) = validate_beam_incident_scan(
            &scan,
            BeamIncidentValidationContext {
                beam_kind: BeamKind::Beam,
                fresh: &fresh,
                source_outgoing_before: &source_outgoing,
                stem_inter_id: 9,
                sibling_inter_id: 8,
                stem_alias: "stem:9",
                group_inter_id: 30,
                group_vertex: 20,
                group_alias: "beam-group:20",
                live_group_members: &[],
                graph_limit: 8,
                abnormal_before: true,
            },
            &mut BTreeMap::new(),
        )
        .unwrap();
        assert!(!after);
        let mut corrupted_source_outgoing = source_outgoing.clone();
        corrupted_source_outgoing[0].relation_class = BEAM_STEM_CLASS.into();
        assert!(
            validate_beam_incident_scan(
                &scan,
                BeamIncidentValidationContext {
                    beam_kind: BeamKind::Beam,
                    fresh: &fresh,
                    source_outgoing_before: &corrupted_source_outgoing,
                    stem_inter_id: 9,
                    sibling_inter_id: 8,
                    stem_alias: "stem:9",
                    group_inter_id: 30,
                    group_vertex: 20,
                    group_alias: "beam-group:20",
                    live_group_members: &[],
                    graph_limit: 8,
                    abnormal_before: true,
                },
                &mut BTreeMap::new(),
            )
            .is_err()
        );
    }

    fn item(
        kind: NativeStemsBeamBuilderItemKind,
        target: Option<NativeStemsBeamBuilderTargetRef>,
    ) -> NativeStemsBeamBuilderItem {
        NativeStemsBeamBuilderItem {
            kind,
            glyph: None,
            target,
            reference_point: None,
            head_bounds: None,
            line: NativeStemLine {
                start: NativeStemPoint { x: 0.0, y: 0.0 },
                stop: NativeStemPoint { x: 0.0, y: 1.0 },
            },
            contribution: 0,
        }
    }

    fn live_beam(
        source: NativeStemsBeamSource,
        alias: &str,
        vertex: usize,
        inter_id: i32,
    ) -> NativeStemsBeamSiblingLiveBeam {
        NativeStemsBeamSiblingLiveBeam {
            source,
            alias: alias.into(),
            runtime: NativeStemsBeamVLinkBeamRuntimeState {
                source,
                sig_vertex_identity: Some(vertex),
                inter_id,
                inter_indexed: true,
                sig_system_id: 1,
                removed: false,
                vip: false,
                abnormal: false,
                stump_group_ordinal: 0,
                beam_group: None,
            },
            inter_index_ordinal: vertex,
            inter_index_object_matches: 1,
            inter_index_id_matches: 1,
            glyph: None,
        }
    }

    #[test]
    fn live_group_inter_index_lookup_is_exact_bounded_and_base_joined() {
        let mut beam = live_beam(BASE, "beam:1", 1, 41);
        assert!(validate_live_member_index(&beam, 2).is_ok());
        assert!(base_live_member_index_matches(
            NativeStemsBeamInterIndexLookup::PresentSameObject {
                index_ordinal: 1,
                inter_id: 41,
                vip: false,
                object_matches: 1,
                inter_id_matches: 1,
                glyph_active_matches: 0,
                glyph_original_matches: 0,
            },
            &beam,
        ));

        beam.inter_index_object_matches = 2;
        assert!(validate_live_member_index(&beam, 2).is_err());
        beam.inter_index_object_matches = 1;
        beam.inter_index_ordinal = 2;
        assert!(validate_live_member_index(&beam, 2).is_err());
        beam.inter_index_ordinal = 1;
        assert!(!base_live_member_index_matches(
            NativeStemsBeamInterIndexLookup::PresentSameObject {
                index_ordinal: 0,
                inter_id: 41,
                vip: false,
                object_matches: 1,
                inter_id_matches: 1,
                glyph_active_matches: 0,
                glyph_original_matches: 0,
            },
            &beam,
        ));
    }

    #[test]
    fn beam_alias_uses_beam_only_sig_ordinal_not_full_graph_vertex() {
        assert_eq!(beam_alias(2), "beam:2");
        assert_ne!(beam_alias(2), "beam:17");
    }

    #[test]
    fn builder_lookup_selects_first_exact_sibling_b_and_leaves_suffix_unread() {
        let other = NativeStemsBeamBLinkerRef {
            beam: NativeStemsBeamSource::RawBeam(2),
            id: 1,
        };
        let items = vec![
            item(NativeStemsBeamBuilderItemKind::StartHalfLinker, None),
            item(
                NativeStemsBeamBuilderItemKind::BeamLinker,
                Some(NativeStemsBeamBuilderTargetRef::Beam(other)),
            ),
            item(
                NativeStemsBeamBuilderItemKind::BeamLinker,
                Some(NativeStemsBeamBuilderTargetRef::Beam(SIBLING_B)),
            ),
            item(NativeStemsBeamBuilderItemKind::Gap, None),
        ];
        let (rows, selected) = structural_builder_lookup(&items, START_B, SIBLING);
        assert_eq!(selected, Some(SIBLING_B));
        assert_eq!(
            rows[2],
            NativeStemsBeamSiblingBuilderItemRead::ExaminedSelectBreak
        );
        assert_eq!(
            rows[3],
            NativeStemsBeamSiblingBuilderItemRead::UnreadAfterBreak
        );
    }

    #[test]
    fn builder_lookup_hash_matches_java_tokens_and_detects_payload_drift() {
        let mut rows = vec![
            NativeStemsBeamSiblingBuilderLookupRow {
                item_ordinal: 0,
                item_kind: NativeStemsBeamBuilderItemKind::StartHalfLinker,
                linker: Some(NativeStemsBeamSiblingBuilderLinkerIdentity::StartVLinker),
                source_beam: Some(BASE),
                read: NativeStemsBeamSiblingBuilderItemRead::ExaminedContinue,
                runtime_class: Some(STEM_HALF_LINKER_ITEM_CLASS.into()),
                linker_read: NativeStemsBeamSiblingBuilderLinkerRead::ReadLinker,
                source_read: NativeStemsBeamSiblingBuilderSourceRead::ReadSource,
                linker_alias: Some("beam:1:b:0:v:TOP".into()),
                linker_runtime_class: Some(BEAM_V_LINKER_CLASS.into()),
                source_alias: Some("beam:1".into()),
                source_inter_id: Some(41),
                identity_match: Some(false),
                action: NativeStemsBeamSiblingBuilderAction::Continue,
            },
            NativeStemsBeamSiblingBuilderLookupRow {
                item_ordinal: 1,
                item_kind: NativeStemsBeamBuilderItemKind::BeamLinker,
                linker: Some(NativeStemsBeamSiblingBuilderLinkerIdentity::BeamBLinker(
                    SIBLING_B,
                )),
                source_beam: Some(SIBLING),
                read: NativeStemsBeamSiblingBuilderItemRead::ExaminedSelectBreak,
                runtime_class: Some(STEM_LINKER_ITEM_CLASS.into()),
                linker_read: NativeStemsBeamSiblingBuilderLinkerRead::ReadLinker,
                source_read: NativeStemsBeamSiblingBuilderSourceRead::ReadSource,
                linker_alias: Some("beam:2:b:1".into()),
                linker_runtime_class: Some(BEAM_B_LINKER_CLASS.into()),
                source_alias: Some("beam:2".into()),
                source_inter_id: Some(42),
                identity_match: Some(true),
                action: NativeStemsBeamSiblingBuilderAction::SelectBreak,
            },
            NativeStemsBeamSiblingBuilderLookupRow {
                item_ordinal: 2,
                item_kind: NativeStemsBeamBuilderItemKind::Gap,
                linker: None,
                source_beam: None,
                read: NativeStemsBeamSiblingBuilderItemRead::UnreadAfterBreak,
                runtime_class: None,
                linker_read: NativeStemsBeamSiblingBuilderLinkerRead::NotRead,
                source_read: NativeStemsBeamSiblingBuilderSourceRead::NotRead,
                linker_alias: None,
                linker_runtime_class: None,
                source_alias: None,
                source_inter_id: None,
                identity_match: None,
                action: NativeStemsBeamSiblingBuilderAction::UnreadAfterBreak,
            },
        ];
        assert_eq!(
            builder_lookup_query_sha256(&rows),
            "bfe7b605e0f67fd28eaaf4e225f3a01105d53a0a702dc83684fe84999d513865"
        );
        let exact = builder_lookup_query_sha256(&rows);
        rows[1].source_inter_id = Some(43);
        assert_ne!(builder_lookup_query_sha256(&rows), exact);
    }

    #[test]
    fn exact_builder_lookup_certificate_selects_first_b_and_fails_closed_on_alias_drift() {
        let items = vec![
            item(NativeStemsBeamBuilderItemKind::StartHalfLinker, None),
            item(
                NativeStemsBeamBuilderItemKind::BeamLinker,
                Some(NativeStemsBeamBuilderTargetRef::Beam(SIBLING_B)),
            ),
            item(NativeStemsBeamBuilderItemKind::Gap, None),
        ];
        let rows = vec![
            NativeStemsBeamSiblingBuilderLookupRow {
                item_ordinal: 0,
                item_kind: NativeStemsBeamBuilderItemKind::StartHalfLinker,
                linker: Some(NativeStemsBeamSiblingBuilderLinkerIdentity::StartVLinker),
                source_beam: Some(BASE),
                read: NativeStemsBeamSiblingBuilderItemRead::ExaminedContinue,
                runtime_class: Some(STEM_HALF_LINKER_ITEM_CLASS.into()),
                linker_read: NativeStemsBeamSiblingBuilderLinkerRead::ReadLinker,
                source_read: NativeStemsBeamSiblingBuilderSourceRead::ReadSource,
                linker_alias: Some("beam:1:b:0:v:TOP".into()),
                linker_runtime_class: Some(BEAM_V_LINKER_CLASS.into()),
                source_alias: Some("beam:1".into()),
                source_inter_id: Some(41),
                identity_match: Some(false),
                action: NativeStemsBeamSiblingBuilderAction::Continue,
            },
            NativeStemsBeamSiblingBuilderLookupRow {
                item_ordinal: 1,
                item_kind: NativeStemsBeamBuilderItemKind::BeamLinker,
                linker: Some(NativeStemsBeamSiblingBuilderLinkerIdentity::BeamBLinker(
                    SIBLING_B,
                )),
                source_beam: Some(SIBLING),
                read: NativeStemsBeamSiblingBuilderItemRead::ExaminedSelectBreak,
                runtime_class: Some(STEM_LINKER_ITEM_CLASS.into()),
                linker_read: NativeStemsBeamSiblingBuilderLinkerRead::ReadLinker,
                source_read: NativeStemsBeamSiblingBuilderSourceRead::ReadSource,
                linker_alias: Some("beam:2:b:1".into()),
                linker_runtime_class: Some(BEAM_B_LINKER_CLASS.into()),
                source_alias: Some("beam:2".into()),
                source_inter_id: Some(42),
                identity_match: Some(true),
                action: NativeStemsBeamSiblingBuilderAction::SelectBreak,
            },
            NativeStemsBeamSiblingBuilderLookupRow {
                item_ordinal: 2,
                item_kind: NativeStemsBeamBuilderItemKind::Gap,
                linker: None,
                source_beam: None,
                read: NativeStemsBeamSiblingBuilderItemRead::UnreadAfterBreak,
                runtime_class: None,
                linker_read: NativeStemsBeamSiblingBuilderLinkerRead::NotRead,
                source_read: NativeStemsBeamSiblingBuilderSourceRead::NotRead,
                linker_alias: None,
                linker_runtime_class: None,
                source_alias: None,
                source_inter_id: None,
                identity_match: None,
                action: NativeStemsBeamSiblingBuilderAction::UnreadAfterBreak,
            },
        ];
        let mut scan = NativeStemsBeamSiblingBuilderLookupScan {
            state: NativeStemsBeamSiblingBuilderLookupState::FirstSourceIdentityMatch,
            timing: NativeStemsBeamSiblingBuilderLookupTiming::ReconstructedFromImmutableItems,
            query_item_count: rows.len(),
            query_provenance_sha256: builder_lookup_query_sha256(&rows),
            rows,
            selected_b_linker: Some(SIBLING_B),
            selected_alias: Some("beam:2:b:1".into()),
        };
        let live = [
            live_beam(BASE, "beam:1", 1, 41),
            live_beam(SIBLING, "beam:2", 2, 42),
        ];
        let start = crate::native_stems_beam_vlinkers::NativeStemsBeamVLinkerRef {
            b_linker: START_B,
            side: NativeStemVerticalSide::Top,
        };
        assert_eq!(
            validate_builder_lookup_items(&scan, &items, start, SIBLING, &live).unwrap(),
            Some(SIBLING_B)
        );
        scan.rows[1].linker_alias = Some("beam:2:b:0".into());
        scan.query_provenance_sha256 = builder_lookup_query_sha256(&scan.rows);
        assert!(validate_builder_lookup_items(&scan, &items, start, SIBLING, &live).is_err());
    }

    #[test]
    fn embedded_sha256_matches_standard_empty_and_abc_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn java_glyph_token_uses_exact_orientation_bounds_and_run_serialization() {
        let glyph = NativeStemsBeamGlyph {
            bounds: Bounds {
                x: 3,
                y: 4,
                width: 2,
                height: 2,
            },
            weight: 2,
            run_table: RunTable::from_pixels(
                Orientation::Horizontal,
                2,
                2,
                &[FOREGROUND, 255, 255, FOREGROUND],
            )
            .expect("valid run table"),
        };
        assert_eq!(
            java_glyph_token(&glyph),
            "g:3:4:2:2:14d6b8bdea67b2bcd03f798d0d49bf74793213a293a9e484e4d070c70baef3f4"
        );
    }

    #[test]
    fn member_inclusive_group_hash_advances_exactly_on_abnormal_change() {
        let before = "0".repeat(64);
        let after = "1".repeat(64);
        assert!(validate_group_member_state_transition(&before, &before, 0).is_ok());
        assert!(validate_group_member_state_transition(&before, &after, 1).is_ok());
        assert!(validate_group_member_state_transition(&before, &after, 0).is_err());
        assert!(validate_group_member_state_transition(&before, &before, 1).is_err());
        assert!(validate_group_member_state_transition(&before, &"A".repeat(64), 1).is_err());
    }

    #[test]
    fn same_glyph_uses_java_object_identity_and_rejects_object_payload_drift() {
        let first = NativeStemsBeamSiblingGlyphIdentity {
            object_identity: 0,
            token: "glyph-a".into(),
        };
        let same = first.clone();
        let different_object = NativeStemsBeamSiblingGlyphIdentity {
            object_identity: 1,
            token: "glyph-a".into(),
        };
        assert!(same_glyph_identity(Some(&first), Some(&same)));
        assert!(!same_glyph_identity(Some(&first), Some(&different_object)));
        assert!(same_glyph_identity(None, None));

        let mut conflicting = live_beam(BASE, "beam:1", 1, 41);
        conflicting.glyph = Some(NativeStemsBeamSiblingGlyphIdentity {
            object_identity: 0,
            token: "glyph-b".into(),
        });
        assert!(validate_glyph_identities(Some(&first), &[conflicting]).is_err());

        let mut non_dense = live_beam(BASE, "beam:1", 1, 41);
        non_dense.glyph = Some(NativeStemsBeamSiblingGlyphIdentity {
            object_identity: 1,
            token: "glyph-a".into(),
        });
        assert!(validate_glyph_identities(Some(&different_object), &[non_dense]).is_err());
    }

    #[test]
    fn incident_chronology_is_incoming_then_outgoing_and_strict_per_direction() {
        assert!(
            validate_incident_chronology(
                [
                    (0, NativeStemsBeamIncidentDirection::Incoming, 0, 1),
                    (1, NativeStemsBeamIncidentDirection::Incoming, 1, 3),
                    (2, NativeStemsBeamIncidentDirection::Outgoing, 0, 2),
                ],
                8,
                "test",
            )
            .is_ok()
        );
        assert!(
            validate_incident_chronology(
                [
                    (0, NativeStemsBeamIncidentDirection::Outgoing, 0, 2),
                    (1, NativeStemsBeamIncidentDirection::Incoming, 0, 3),
                ],
                8,
                "test",
            )
            .is_err()
        );
    }

    #[test]
    fn relation_and_endpoint_identity_domains_fail_closed() {
        let mut relations = BTreeMap::new();
        assert!(join_relation_object(&mut relations, 1, object()).is_ok());
        assert!(join_relation_object(&mut relations, 2, object()).is_err());
        assert!(
            join_relation_object(
                &mut BTreeMap::new(),
                2,
                NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(1)
            )
            .is_err()
        );

        let mut endpoints = EndpointIdentityCatalogue::default();
        assert!(endpoints.join(4, 9, "stem:9", "test").is_ok());
        assert!(endpoints.join(4, 10, "inter:10", "test").is_err());
        assert!(endpoints.join(5, 9, "stem:9", "test").is_err());
        assert!(endpoints.join(5, 9, "other-stem:9", "test").is_err());
        assert!(endpoints.join(5, 10, "stem:9", "test").is_err());
        assert!(
            EndpointIdentityCatalogue::with_glyph_ids([12])
                .join(6, 12, "inter:12", "test")
                .is_err()
        );

        let mut cross = BTreeMap::new();
        assert!(
            join_cross_query_relation(
                &mut cross,
                3,
                NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(3),
                BEAM_STEM_CLASS,
                Some(NativeStemsBeamQueryRelationKind::BeamStem),
                Some(NativeStemsBeamSigRelationKind::BeamStem {
                    beam_portion: Some(NativeBeamPortion::Left),
                }),
            )
            .is_ok()
        );
        assert!(
            join_cross_query_relation(
                &mut cross,
                3,
                NativeStemsBeamSiblingRelationObjectIdentity::GraphObject(3),
                BEAM_STEM_CLASS,
                Some(NativeStemsBeamQueryRelationKind::BeamStem),
                Some(NativeStemsBeamSigRelationKind::BeamStem {
                    beam_portion: Some(NativeBeamPortion::Right),
                }),
            )
            .is_err()
        );
    }

    #[test]
    fn operation_order_places_structural_edge_before_synchronous_callback() {
        let mut operations = Vec::new();
        append_edge_operations(0, 7, &mut operations);
        assert!(matches!(
            operations.as_slice(),
            [
                NativeStemsBeamVLinkSiblingLinksOperation::SigGlobalRelationInserted { .. },
                NativeStemsBeamVLinkSiblingLinksOperation::BeamOutgoingRelationInserted { .. },
                NativeStemsBeamVLinkSiblingLinksOperation::StemIncomingRelationInserted { .. },
                NativeStemsBeamVLinkSiblingLinksOperation::SigEdgeEventDispatched { .. },
                NativeStemsBeamVLinkSiblingLinksOperation::StandardSigListenerEdgeCallbackStarted { .. },
                NativeStemsBeamVLinkSiblingLinksOperation::BeamStemRelationCallbackStarted { .. },
            ]
        ));
    }

    #[test]
    fn b_cell_catalogue_rejects_duplicate_or_zero_java_ids() {
        let valid = NativeStemsBeamSiblingBLinkerCell {
            reference: SIBLING_B,
            linked: false,
            closed: false,
        };
        assert!(validate_unique_cells(std::slice::from_ref(&valid)).is_ok());
        assert!(validate_unique_cells(&[valid.clone(), valid]).is_err());
        assert!(
            validate_unique_cells(&[NativeStemsBeamSiblingBLinkerCell {
                reference: NativeStemsBeamBLinkerRef {
                    beam: SIBLING,
                    id: 0,
                },
                linked: false,
                closed: false,
            }])
            .is_err()
        );
    }

    #[test]
    fn vertical_side_domain_used_by_builder_start_remains_exact() {
        let v = crate::native_stems_beam_vlinkers::NativeStemsBeamVLinkerRef {
            b_linker: START_B,
            side: NativeStemVerticalSide::Top,
        };
        assert_eq!(v.side, NativeStemVerticalSide::Top);
        assert_eq!(
            BEAM_V_LINKER_CLASS,
            "org.audiveris.omr.sheet.stem.BeamLinker$BLinker$VLinker"
        );
    }
}
