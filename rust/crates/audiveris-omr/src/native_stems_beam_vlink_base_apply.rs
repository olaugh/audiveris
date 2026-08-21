// SPDX-License-Identifier: AGPL-3.0-or-later

//! First SIG mutation of a beam-origin `VLinker.link` call.
//!
//! This boundary resumes an exact boundary-13 `ReadyBeforeSigMutation`
//! product, conditionally registers and inserts the selected `StemInter`, and
//! applies only the already-checked base `BeamStemRelation`. It stops before
//! the B-linker flag, sibling beams, or any head relation. The live graph
//! snapshot is deliberately partial: every query made by these Java lines has
//! an exhaustive ordered certificate, while unrelated vertices and edges are
//! represented only by frozen counts and provenance.

use std::{error::Error, fmt};

use crate::{
    native_sig::{
        NativeSigBounds, NativeSigEdge, NativeSigInterKind, NativeSigRelationKind,
        NativeSigSupport, NativeSigSystem, NativeSigSystemBindings, NativeSigVertex,
        NativeSigVertexId,
    },
    native_stems_beam_link_plans::NativeStemsBeamLinkPlanSystem,
    native_stems_beam_scheduler::{NativeStemsBeamPlanRef, NativeStemsBeamSchedulerSystem},
    native_stems_beam_stumps::{NativeStemsBeamSource, NativeStemsBeamStumpSystem},
    native_stems_beam_vlink_reuse_check::{
        NativeStemsBeamRelationDraft, NativeStemsBeamRelationParameters,
        NativeStemsBeamVLinkReuseCheck, NativeStemsBeamVLinkReuseCheckOutcome,
        NativeStemsBeamVLinkReuseLiveState, evaluate_native_stems_beam_vlink_reuse_check,
    },
    native_stems_beam_vlink_transaction::{
        NativeStemsBeamKnownSystemStem, NativeStemsBeamVLinkTransaction,
        NativeStemsBeamVLinkTransactionState,
    },
    native_stems_beam_vlinkers::NativeStemsBeamVLinkerSystem,
    stems_step::NativeBeamPortion,
};

/// Derive every graph query B14 will read from the production-owned SIG.
///
/// Persistent Java EntityIndex IDs are deliberately not inputs. Certificate endpoint
/// IDs use the native one-based insertion identity, while graph and relation identities
/// remain zero-based insertion ordinals.
pub fn project_native_stems_beam_vlink_base_apply_certificate(
    sig: &NativeSigSystem,
    bindings: &NativeSigSystemBindings,
    beam: NativeStemsBeamSource,
    stem_vertex: Option<NativeSigVertexId>,
    draft: &NativeStemsBeamRelationDraft,
    plan: NativeStemsBeamPlanRef,
) -> Result<NativeStemsBeamVLinkBaseApplyCertificate, NativeStemsBeamVLinkBaseApplyError> {
    if bindings.system_id != sig.system_id || draft.beam != beam {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native SIG projection identity join",
        });
    }
    sig.validate_integrity()
        .map_err(|_| NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native SIG projection integrity",
        })?;
    let beam_vertex = *bindings.beam_vertices.get(&beam).ok_or(
        NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native SIG beam binding",
        },
    )?;
    let beam_payload =
        sig.vertex(beam_vertex.0)
            .ok_or(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                phase: "native SIG live beam vertex",
            })?;
    let hook = match beam_payload.kind {
        NativeSigInterKind::BeamHook => true,
        NativeSigInterKind::Beam | NativeSigInterKind::SmallBeam => false,
        _ => {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                phase: "native SIG beam binding kind",
            });
        }
    };
    if let Some(stem) = stem_vertex {
        if sig
            .vertex(stem.0)
            .is_none_or(|vertex| vertex.kind != NativeSigInterKind::Stem)
        {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                phase: "native SIG live stem vertex",
            });
        }
    }

    let fresh = NativeStemsBeamRelationObjectIdentity::FreshDraft(plan.plan_ordinal);
    let beam_before = project_native_beam_incident(
        sig,
        beam_vertex,
        stem_vertex,
        hook,
        None,
        draft,
        plan.plan_ordinal,
    )?;
    let stem_before = match stem_vertex {
        None => NativeStemsBeamStemIncidentScan {
            state: NativeStemsBeamStemIncidentScanState::MissingVertex,
            query_relation_count: 0,
            query_provenance_sha256: query_rows_sha256(std::iter::empty()),
            relations: Vec::new(),
        },
        Some(stem) => project_native_stem_incident(sig, stem, None, beam_vertex, fresh)?,
    };

    let outgoing = sig.outgoing_edges(beam_vertex.0).map_err(|_| {
        NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native SIG beam outgoing query",
        }
    })?;
    let directed = match stem_vertex {
        None => Vec::new(),
        Some(stem) => outgoing
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.target == stem.0)
            .map(|(source_outgoing_ordinal, edge)| (source_outgoing_ordinal, *edge))
            .collect(),
    };
    let mut matched = false;
    let pair_rows = directed
        .iter()
        .enumerate()
        .map(|(pair_ordinal, (source_outgoing_ordinal, edge))| {
            let kind = native_query_kind(edge.kind);
            let class_read = if matched {
                NativeStemsBeamPairClassRead::UnreadAfterBreak
            } else if kind == NativeStemsBeamQueryRelationKind::BeamStem {
                matched = true;
                NativeStemsBeamPairClassRead::ExaminedMatchBreak
            } else {
                NativeStemsBeamPairClassRead::ExaminedContinue
            };
            NativeStemsBeamDirectedPairRelation {
                pair_ordinal,
                source_outgoing_ordinal: *source_outgoing_ordinal,
                graph_relation_identity: edge.ordinal,
                relation_object_identity: NativeStemsBeamRelationObjectIdentity::GraphObject(
                    edge.ordinal,
                ),
                relation_class: native_relation_class(edge.kind).to_owned(),
                kind,
                class_read,
            }
        })
        .collect::<Vec<_>>();
    let outgoing_hash = query_rows_sha256(outgoing.iter().map(|edge| {
        format!(
            "{}:{}",
            graph_relation_alias(edge.ordinal),
            native_relation_class(edge.kind)
        )
    }));
    let pair_hash = query_rows_sha256(pair_rows.iter().map(|row| {
        format!(
            "{}:{}:{}:{}",
            row.pair_ordinal,
            row.source_outgoing_ordinal,
            graph_relation_alias(row.graph_relation_identity),
            row.relation_class
        )
    }));
    let inserts = !pair_rows
        .iter()
        .any(|row| row.kind == NativeStemsBeamQueryRelationKind::BeamStem);
    let new_edge = inserts.then_some(sig.edges.len());
    let effective_stem = stem_vertex.unwrap_or(NativeSigVertexId(sig.vertices.len()));
    let stem_after = if let Some(edge) = new_edge {
        project_native_stem_incident(sig, effective_stem, Some(edge), beam_vertex, fresh)?
    } else {
        stem_before.clone()
    };
    let beam_after = if let Some(edge) = new_edge {
        project_native_beam_incident(
            sig,
            beam_vertex,
            Some(effective_stem),
            hook,
            Some((edge, effective_stem, fresh)),
            draft,
            plan.plan_ordinal,
        )?
    } else {
        beam_before.clone()
    };
    let chord_stem_matches = stem_after
        .relations
        .iter()
        .filter(|row| row.kind == NativeStemsBeamQueryRelationKind::ChordStem)
        .count();

    Ok(NativeStemsBeamVLinkBaseApplyCertificate {
        system_id: sig.system_id,
        headless: true,
        listener_topology: NativeStemsBeamSigListenerTopology::SoleStandardSigListener,
        endpoint_identity: NativeStemsBeamCertificateEndpointIdentity::NativeVertexOneBased,
        directed_pair_scan: NativeStemsBeamDirectedPairScan {
            source_outgoing_scanned: outgoing.len(),
            source_outgoing_provenance: NativeStemsBeamQueryProvenance::ExhaustiveSha256(
                outgoing_hash,
            ),
            query_relation_count: pair_rows.len(),
            pair_provenance: NativeStemsBeamQueryProvenance::ExhaustiveSha256(pair_hash),
            relations: pair_rows,
        },
        stem_incident_before: stem_before,
        stem_incident_after: stem_after,
        beam_incident_before: beam_before,
        beam_incident_after: beam_after,
        chord_stem_matches,
        fresh_relation_object_identity: fresh,
        fresh_relation_graph_matches: 0,
    })
}

/// One Java object participating in the base relation seam.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkStemRuntimeState {
    pub stem_identity: usize,
    /// Dense SIG vertex identity, independently of the Java Inter ID.
    pub sig_vertex_identity: Option<usize>,
    pub inter_indexed: bool,
    pub sig_system_id: Option<usize>,
    pub removed: bool,
    pub vip: bool,
    pub abnormal: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkBeamRuntimeState {
    pub source: NativeStemsBeamSource,
    /// Dense live SIG vertex identity. Java removal preserves the object's
    /// positive Inter ID and SIG pointer while removing this live membership.
    pub sig_vertex_identity: Option<usize>,
    pub inter_id: i32,
    pub inter_indexed: bool,
    pub sig_system_id: usize,
    pub removed: bool,
    pub vip: bool,
    pub abnormal: bool,
    /// Dense group ordinal in the native stump/predecessor product.
    pub stump_group_ordinal: usize,
    /// Independent live Java `BeamGroupInter` identity/state, when present.
    /// This seam never mutates it.
    pub beam_group: Option<NativeStemsBeamGroupRuntimeState>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamGroupRuntimeState {
    pub sig_vertex_ordinal: usize,
    pub state_sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamSheetEditState {
    pub stub_modified: bool,
    pub book_modified: bool,
    pub book_dirty: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamInterIndexLookup {
    Absent,
    PresentSameObject {
        index_ordinal: usize,
        inter_id: i32,
        vip: bool,
        object_matches: usize,
        /// Exhaustive `InterIndex` entries carrying this persistent ID.
        inter_id_matches: usize,
        glyph_active_matches: usize,
        glyph_original_matches: usize,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct NativeStemsBeamVLinkBaseRolloverAuthority<'a> {
    pub stump_system: &'a NativeStemsBeamStumpSystem,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamNextPersistentIdLookup {
    NotRead,
    VacantAndNotVip {
        persistent_id: i32,
        inter_id_matches: usize,
        glyph_active_matches: usize,
        glyph_original_matches: usize,
        configured_vip_matches: usize,
    },
    OccupiedByAppendedStem {
        persistent_id: i32,
        stem_identity: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamInterIndexAppend {
    pub index_ordinal: usize,
    pub stem_identity: usize,
    pub inter_id: i32,
    pub vip: bool,
}

/// Opaque global baseline plus every reversible lookup and local append used
/// by this compact seam.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamInterIndexApplyState {
    pub baseline_entry_count: usize,
    pub baseline_provenance_sha256: String,
    pub beam_lookup: NativeStemsBeamInterIndexLookup,
    pub stem_lookup: NativeStemsBeamInterIndexLookup,
    pub next_id_lookup: NativeStemsBeamNextPersistentIdLookup,
    pub appended_entries: Vec<NativeStemsBeamInterIndexAppend>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamSigVertexLookup {
    Absent,
    PresentSameObject {
        vertex_ordinal: usize,
        sig_vertex_identity: usize,
        inter_id: i32,
        object_matches: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamSigVertexAppend {
    pub vertex_ordinal: usize,
    pub sig_vertex_identity: usize,
    pub stem_identity: usize,
    pub inter_id: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamSigRelationKind {
    BeamStem {
        beam_portion: Option<NativeBeamPortion>,
    },
    BeamRest {
        beam_portion: Option<NativeBeamPortion>,
    },
    ChordStem,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamQueryRelationKind {
    BeamStem,
    BeamRest,
    ChordStem,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamRelationObjectIdentity {
    GraphObject(usize),
    FreshDraft(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamSigRelationState {
    /// Dense/global SIG edge identity in source order.
    pub graph_relation_identity: usize,
    /// Exact Java `Relation` object identity, independent of graph order.
    pub relation_object_identity: NativeStemsBeamRelationObjectIdentity,
    pub source_vertex_identity: usize,
    pub target_vertex_identity: usize,
    pub kind: NativeStemsBeamSigRelationKind,
}

/// Opaque global SIG baseline plus endpoint lookups and locally appended
/// objects. Exact live relation query payloads live in the one-shot
/// certificate.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSigApplyState {
    pub system_id: usize,
    pub baseline_vertex_count: usize,
    pub baseline_vertex_provenance_sha256: String,
    pub baseline_relation_count: usize,
    pub baseline_relation_provenance_sha256: String,
    pub beam_vertex: NativeStemsBeamSigVertexLookup,
    pub stem_vertex: NativeStemsBeamSigVertexLookup,
    pub appended_vertices: Vec<NativeStemsBeamSigVertexAppend>,
    pub appended_relations: Vec<NativeStemsBeamSigRelationState>,
    pub listener_topology: NativeStemsBeamSigListenerTopology,
    pub beam: NativeStemsBeamVLinkBeamRuntimeState,
    pub stem: NativeStemsBeamVLinkStemRuntimeState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamPairClassRead {
    ExaminedContinue,
    ExaminedMatchBreak,
    UnreadAfterBreak,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamQueryProvenance {
    NotRead,
    ExhaustiveSha256(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamDirectedPairRelation {
    pub pair_ordinal: usize,
    pub source_outgoing_ordinal: usize,
    pub graph_relation_identity: usize,
    pub relation_object_identity: NativeStemsBeamRelationObjectIdentity,
    pub relation_class: String,
    pub kind: NativeStemsBeamQueryRelationKind,
    pub class_read: NativeStemsBeamPairClassRead,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamDirectedPairScan {
    pub source_outgoing_scanned: usize,
    pub source_outgoing_provenance: NativeStemsBeamQueryProvenance,
    pub query_relation_count: usize,
    pub pair_provenance: NativeStemsBeamQueryProvenance,
    pub relations: Vec<NativeStemsBeamDirectedPairRelation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamIncidentDirection {
    Incoming,
    Outgoing,
}

/// Exact object-role identity of the opposite endpoint in an incident query.
/// Java aliases such as `beam:<id>` and `created:<plan>` are projected into
/// these typed domains instead of being normalized to a generic Inter ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamIncidentOpposite {
    Beam,
    Stem,
    OtherInter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamStemIncidentScanState {
    NotRead,
    MissingVertex,
    ExhaustiveIncomingThenOutgoing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamStemIncidentRelation {
    pub incident_ordinal: usize,
    pub direction: NativeStemsBeamIncidentDirection,
    pub direction_ordinal: usize,
    pub graph_relation_identity: usize,
    pub relation_object_identity: NativeStemsBeamRelationObjectIdentity,
    pub relation_class: String,
    pub kind: NativeStemsBeamQueryRelationKind,
    pub opposite_vertex_ordinal: usize,
    pub opposite: NativeStemsBeamIncidentOpposite,
    pub opposite_inter_id: i32,
    pub chord_stem_match: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamStemIncidentScan {
    pub state: NativeStemsBeamStemIncidentScanState,
    pub query_relation_count: usize,
    pub query_provenance_sha256: String,
    pub relations: Vec<NativeStemsBeamStemIncidentRelation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamBeamIncidentRule {
    NotRead,
    HookHasAnyBeamStem,
    RawBeamLeftAndRight,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamBeamIncidentRead {
    Examined,
    UnreadAfterBreak,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamBeamIncidentRelation {
    pub incident_ordinal: usize,
    pub direction: NativeStemsBeamIncidentDirection,
    pub direction_ordinal: usize,
    pub graph_relation_identity: usize,
    pub relation_object_identity: NativeStemsBeamRelationObjectIdentity,
    pub relation_class: String,
    pub kind: NativeStemsBeamQueryRelationKind,
    pub opposite_vertex_ordinal: usize,
    pub opposite: NativeStemsBeamIncidentOpposite,
    pub opposite_inter_id: i32,
    pub read: NativeStemsBeamBeamIncidentRead,
    pub relevant: bool,
    pub beam_portion: Option<NativeBeamPortion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamBeamIncidentScan {
    pub rule: NativeStemsBeamBeamIncidentRule,
    pub query_relation_count: usize,
    pub query_provenance_sha256: String,
    pub relations: Vec<NativeStemsBeamBeamIncidentRelation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamSigListenerTopology {
    SoleStandardSigListener,
}

/// Identity domain used for opposite endpoints in the exhaustive SIG query
/// rows carried by a base-apply certificate.
///
/// Frozen Java oracle rows use persistent `Inter` IDs. Certificates projected
/// from the port-owned SIG instead use the stable, one-based native vertex ID;
/// this keeps production projection independent of a fixture-derived
/// Java-ID map while preserving the existing oracle corpus verbatim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamCertificateEndpointIdentity {
    JavaPersistentInterId,
    NativeVertexOneBased,
}

/// One-shot evidence for every live read made by `SIGraph.addVertex`,
/// `Link.applyTo`, and the synchronous `BeamStemRelation.added` callback.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkBaseApplyCertificate {
    pub system_id: usize,
    pub headless: bool,
    pub listener_topology: NativeStemsBeamSigListenerTopology,
    pub endpoint_identity: NativeStemsBeamCertificateEndpointIdentity,
    /// Directed `beam -> stem` `getAllEdges` order used by duplicate lookup.
    pub directed_pair_scan: NativeStemsBeamDirectedPairScan,
    /// Post-edge `edgesOf(stem)` callback order, or explicit not-read state.
    pub stem_incident_before: NativeStemsBeamStemIncidentScan,
    pub stem_incident_after: NativeStemsBeamStemIncidentScan,
    /// Post-edge `edgesOf(beam)` order used by virtual `checkAbnormal`, or
    /// explicit not-read state.
    pub beam_incident_before: NativeStemsBeamBeamIncidentScan,
    pub beam_incident_after: NativeStemsBeamBeamIncidentScan,
    pub chord_stem_matches: usize,
    /// Stable identity of the fresh, not-yet-attached Java relation draft.
    pub fresh_relation_object_identity: NativeStemsBeamRelationObjectIdentity,
    /// Exhaustive matches for this fresh object in the pre-edge SIG.
    pub fresh_relation_graph_matches: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamVLinkBaseApplyKey {
    pub system_id: usize,
    pub invocation_ordinal: usize,
    pub plan: NativeStemsBeamPlanRef,
}

/// Mutable state for exactly one base-relation transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkBaseApplyState {
    pub transaction_state: NativeStemsBeamVLinkTransactionState,
    pub inter_index: NativeStemsBeamInterIndexApplyState,
    pub sig: NativeStemsBeamSigApplyState,
    pub sheet_edit: NativeStemsBeamSheetEditState,
    pub certificate: Option<NativeStemsBeamVLinkBaseApplyCertificate>,
    pub committed: Option<NativeStemsBeamVLinkBaseApplyKey>,
}

/// Construct the first B14 compact state from the production-owned SIG.
///
/// The native vertex arena is also the initial local InterIndex domain: its
/// zero-based ordinal is stable insertion order and its persistent identity is
/// the one-based vertex identity. The shared persistent-ID counter remains an
/// explicit field of `transaction_state`, but no opaque Java InterIndex count,
/// hash, endpoint row, or SIG snapshot participates in this projection.
pub fn initialize_native_stems_beam_vlink_base_apply_state_from_native_sig(
    transaction_state: &NativeStemsBeamVLinkTransactionState,
    reuse_check: &NativeStemsBeamVLinkReuseCheck,
    sig: &NativeSigSystem,
    bindings: &NativeSigSystemBindings,
    stump_system: &NativeStemsBeamStumpSystem,
    sheet_edit: NativeStemsBeamSheetEditState,
) -> Result<NativeStemsBeamVLinkBaseApplyState, NativeStemsBeamVLinkBaseApplyError> {
    sig.validate_integrity()
        .map_err(|_| NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native initial B14 SIG integrity",
        })?;
    bindings.validate_against(sig).map_err(|_| {
        NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native initial B14 bindings",
        }
    })?;
    if transaction_state.system_stems.system_id != sig.system_id
        || reuse_check.system_id != sig.system_id
        || stump_system.system_id != sig.system_id
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native initial B14 system",
        });
    }
    let NativeStemsBeamVLinkReuseCheckOutcome::ReadyBeforeSigMutation { relation } =
        &reuse_check.outcome
    else {
        return Err(NativeStemsBeamVLinkBaseApplyError::PredecessorNotReady);
    };
    let final_stem = reuse_check
        .final_stem
        .as_ref()
        .ok_or(NativeStemsBeamVLinkBaseApplyError::PredecessorNotReady)?;
    if final_stem.inter_id.is_some()
        || final_stem.sig_attached
        || bindings
            .stem_vertices
            .contains_key(&final_stem.stem_identity)
        || transaction_state
            .system_stems
            .known_stems
            .iter()
            .filter(|stem| stem.stem_identity == final_stem.stem_identity)
            .count()
            != 1
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native initial B14 fresh stem",
        });
    }
    let stump_beam = stump_system
        .beams_by_abscissa
        .iter()
        .find(|beam| beam.source == relation.beam)
        .ok_or(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native initial B14 stump beam",
        })?;
    let beam_vertex = bindings.beam_vertices.get(&relation.beam).copied().ok_or(
        NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native initial B14 beam binding",
        },
    )?;
    let beam =
        sig.vertex(beam_vertex.0)
            .ok_or(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                phase: "native initial B14 live beam",
            })?;
    if beam.removed
        || !beam.active
        || !matches!(
            beam.kind,
            NativeSigInterKind::Beam | NativeSigInterKind::BeamHook | NativeSigInterKind::SmallBeam
        )
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native initial B14 beam kind/state",
        });
    }
    let beam_inter_id = native_inter_id(beam_vertex)?;
    let ids = transaction_state.glyph_index.persistent_ids;
    let next_id = ids.sheet_last_id.checked_add(1).ok_or(
        NativeStemsBeamVLinkBaseApplyError::UnsupportedV1 {
            phase: "native initial B14 persistent ID overflow",
        },
    )?;
    if transaction_state
        .system_stems
        .known_stems
        .iter()
        .any(|stem| stem.inter_id == Some(next_id))
        || transaction_state
            .glyph_index
            .known_canonical_glyphs
            .iter()
            .any(|glyph| glyph.glyph_id == next_id)
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native initial B14 next persistent ID collision",
        });
    }
    let group_vertex = bindings
        .beam_group_vertices
        .get(&stump_beam.group_ordinal)
        .copied()
        .ok_or(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native initial B14 beam group binding",
        })?;
    let group =
        sig.vertex(group_vertex.0)
            .ok_or(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                phase: "native initial B14 live beam group",
            })?;
    if group.kind != NativeSigInterKind::BeamGroup {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native initial B14 beam group kind",
        });
    }
    let group_state = sig
        .outgoing_edges(group_vertex.0)
        .map_err(|_| NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native initial B14 beam group query",
        })?
        .iter()
        .map(|edge| {
            let member = sig.vertex(edge.target).expect("validated active endpoint");
            format!(
                "{}:{}:{}:{}",
                edge.ordinal, edge.target, member.active, member.abnormal
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let vertex_lineage = sig
        .vertices
        .iter()
        .map(|vertex| format!("{vertex:?}"))
        .collect::<Vec<_>>()
        .join("\n");
    let edge_lineage = sig
        .edges
        .iter()
        .map(|edge| format!("{edge:?}"))
        .collect::<Vec<_>>()
        .join("\n");
    let inter_lineage = format!("native-inter-index-initial-v1:{vertex_lineage}");
    let certificate = project_native_stems_beam_vlink_base_apply_certificate(
        sig,
        bindings,
        relation.beam,
        None,
        relation,
        reuse_check.plan,
    )?;
    Ok(NativeStemsBeamVLinkBaseApplyState {
        transaction_state: transaction_state.clone(),
        inter_index: NativeStemsBeamInterIndexApplyState {
            baseline_entry_count: sig.vertices.len(),
            baseline_provenance_sha256: sha256_hex(inter_lineage.as_bytes()),
            beam_lookup: NativeStemsBeamInterIndexLookup::PresentSameObject {
                index_ordinal: beam_vertex.0,
                inter_id: beam_inter_id,
                vip: false,
                object_matches: 1,
                inter_id_matches: 1,
                glyph_active_matches: 0,
                glyph_original_matches: 0,
            },
            stem_lookup: NativeStemsBeamInterIndexLookup::Absent,
            next_id_lookup: NativeStemsBeamNextPersistentIdLookup::VacantAndNotVip {
                persistent_id: next_id,
                inter_id_matches: 0,
                glyph_active_matches: 0,
                glyph_original_matches: 0,
                configured_vip_matches: 0,
            },
            appended_entries: Vec::new(),
        },
        sig: NativeStemsBeamSigApplyState {
            system_id: sig.system_id,
            baseline_vertex_count: sig.vertices.len(),
            baseline_vertex_provenance_sha256: sha256_hex(vertex_lineage.as_bytes()),
            baseline_relation_count: sig.edges.len(),
            baseline_relation_provenance_sha256: sha256_hex(edge_lineage.as_bytes()),
            beam_vertex: NativeStemsBeamSigVertexLookup::PresentSameObject {
                vertex_ordinal: beam_vertex.0,
                sig_vertex_identity: beam_vertex.0,
                inter_id: beam_inter_id,
                object_matches: 1,
            },
            stem_vertex: NativeStemsBeamSigVertexLookup::Absent,
            appended_vertices: Vec::new(),
            appended_relations: Vec::new(),
            listener_topology: NativeStemsBeamSigListenerTopology::SoleStandardSigListener,
            beam: NativeStemsBeamVLinkBeamRuntimeState {
                source: relation.beam,
                sig_vertex_identity: Some(beam_vertex.0),
                inter_id: beam_inter_id,
                inter_indexed: true,
                sig_system_id: sig.system_id,
                removed: false,
                vip: false,
                abnormal: beam.abnormal,
                stump_group_ordinal: stump_beam.group_ordinal,
                beam_group: Some(NativeStemsBeamGroupRuntimeState {
                    sig_vertex_ordinal: group_vertex.0,
                    state_sha256: sha256_hex(group_state.as_bytes()),
                }),
            },
            stem: NativeStemsBeamVLinkStemRuntimeState {
                stem_identity: final_stem.stem_identity,
                sig_vertex_identity: None,
                inter_indexed: false,
                sig_system_id: None,
                removed: false,
                vip: false,
                abnormal: false,
            },
        },
        sheet_edit,
        certificate: Some(certificate),
        committed: None,
    })
}

/// Roll a committed one-shot B14 state onto the next native scheduler
/// frontier without importing a transaction-2 B14 fixture.
///
/// The prior InterIndex lineage is folded into a fresh baseline, while SIG
/// counts and provenance are recomputed from the owned graph. Beam identity is
/// the stable one-based native SIG vertex identity; no Java InterIndex row or
/// configured Java VIP list participates in the rollover.
pub fn roll_native_stems_beam_vlink_base_apply_state(
    prior: &NativeStemsBeamVLinkBaseApplyState,
    transaction_state: &NativeStemsBeamVLinkTransactionState,
    reuse_check: &NativeStemsBeamVLinkReuseCheck,
    sig: &NativeSigSystem,
    bindings: &NativeSigSystemBindings,
    authority: NativeStemsBeamVLinkBaseRolloverAuthority<'_>,
) -> Result<NativeStemsBeamVLinkBaseApplyState, NativeStemsBeamVLinkBaseApplyError> {
    let stump_system = authority.stump_system;
    sig.validate_integrity()
        .map_err(|_| NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native rollover SIG integrity",
        })?;
    bindings.validate_against(sig).map_err(|_| {
        NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native rollover bindings",
        }
    })?;
    if prior.committed.is_none()
        || prior.certificate.is_some()
        || prior.sig.appended_vertices.len() != 1
        || prior.sig.appended_relations.len() != 1
        || prior.inter_index.appended_entries.len() != 1
        || transaction_state.system_stems.system_id != sig.system_id
        || reuse_check.system_id != sig.system_id
        || stump_system.system_id != sig.system_id
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native rollover predecessor",
        });
    }
    let NativeStemsBeamVLinkReuseCheckOutcome::ReadyBeforeSigMutation { relation } =
        &reuse_check.outcome
    else {
        return Err(NativeStemsBeamVLinkBaseApplyError::PredecessorNotReady);
    };
    let stump_beam = stump_system
        .beams_by_abscissa
        .iter()
        .find(|beam| beam.source == relation.beam)
        .ok_or(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native rollover stump beam",
        })?;
    let beam_vertex = bindings.beam_vertices.get(&relation.beam).copied().ok_or(
        NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native rollover beam binding",
        },
    )?;
    let beam =
        sig.vertex(beam_vertex.0)
            .ok_or(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                phase: "native rollover live beam",
            })?;
    if !matches!(
        beam.kind,
        NativeSigInterKind::Beam | NativeSigInterKind::BeamHook | NativeSigInterKind::SmallBeam
    ) {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native rollover beam kind",
        });
    }
    let beam_inter_id = native_inter_id(beam_vertex)?;
    let final_stem = reuse_check
        .final_stem
        .as_ref()
        .ok_or(NativeStemsBeamVLinkBaseApplyError::PredecessorNotReady)?;
    if transaction_state
        .system_stems
        .known_stems
        .iter()
        .filter(|stem| stem.stem_identity == final_stem.stem_identity)
        .count()
        != 1
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native rollover stem identity",
        });
    }
    let stem_vertex = bindings
        .stem_vertices
        .get(&final_stem.stem_identity)
        .copied();
    match (final_stem.inter_id, final_stem.sig_attached, stem_vertex) {
        (None, false, None) => {}
        (Some(inter_id), true, Some(vertex)) => {
            let live =
                sig.vertex(vertex.0)
                    .ok_or(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                        phase: "native rollover live stem",
                    })?;
            if inter_id <= 0 || live.kind != NativeSigInterKind::Stem || live.removed {
                return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                    phase: "native rollover existing stem",
                });
            }
            if live.abnormal != final_stem.abnormal {
                return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                    phase: "native rollover existing stem abnormal",
                });
            }
            // `inter_id` is the carried persistent glyph/InterIndex identity;
            // `vertex` is the native SIG insertion identity. They are joined
            // by the owned stem binding and intentionally need not be equal.
        }
        _ => {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                phase: "native rollover stem membership",
            });
        }
    }
    let ids = transaction_state.glyph_index.persistent_ids;
    let next_id = if stem_vertex.is_none() {
        let next_id = ids.sheet_last_id.checked_add(1).ok_or(
            NativeStemsBeamVLinkBaseApplyError::UnsupportedV1 {
                phase: "native rollover persistent ID overflow",
            },
        )?;
        let inter_id_matches = transaction_state
            .system_stems
            .known_stems
            .iter()
            .filter(|stem| stem.inter_id == Some(next_id))
            .count();
        let glyph_matches = transaction_state
            .glyph_index
            .known_canonical_glyphs
            .iter()
            .filter(|glyph| glyph.glyph_id == next_id)
            .count();
        if inter_id_matches != 0 || glyph_matches != 0 {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                phase: "native rollover next persistent ID collision",
            });
        }
        Some(next_id)
    } else {
        None
    };
    let baseline_entry_count = prior
        .inter_index
        .baseline_entry_count
        .checked_add(prior.inter_index.appended_entries.len())
        .ok_or(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native rollover InterIndex count overflow",
        })?;
    let inter_lineage = format!(
        "native-inter-index-rollover-v1:{}:{}:{:?}",
        prior.inter_index.baseline_provenance_sha256,
        prior.inter_index.baseline_entry_count,
        prior.inter_index.appended_entries
    );
    let vertex_lineage = sig
        .vertices
        .iter()
        .map(|vertex| format!("{vertex:?}"))
        .collect::<Vec<_>>()
        .join("\n");
    let edge_lineage = sig
        .edges
        .iter()
        .map(|edge| format!("{edge:?}"))
        .collect::<Vec<_>>()
        .join("\n");
    let group_vertex = bindings
        .beam_group_vertices
        .get(&stump_beam.group_ordinal)
        .copied()
        .ok_or(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native rollover beam group binding",
        })?;
    let group =
        sig.vertex(group_vertex.0)
            .ok_or(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                phase: "native rollover live beam group",
            })?;
    if group.kind != NativeSigInterKind::BeamGroup {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native rollover beam group kind",
        });
    }
    let group_state = sig
        .outgoing_edges(group_vertex.0)
        .map_err(|_| NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native rollover beam group query",
        })?
        .iter()
        .map(|edge| {
            let member = sig.vertex(edge.target).expect("validated active endpoint");
            format!(
                "{}:{}:{}:{}",
                edge.ordinal, edge.target, member.active, member.abnormal
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let beam_runtime = NativeStemsBeamVLinkBeamRuntimeState {
        source: relation.beam,
        sig_vertex_identity: Some(beam_vertex.0),
        inter_id: beam_inter_id,
        inter_indexed: true,
        sig_system_id: sig.system_id,
        removed: beam.removed,
        vip: false,
        abnormal: beam.abnormal,
        stump_group_ordinal: stump_beam.group_ordinal,
        beam_group: Some(NativeStemsBeamGroupRuntimeState {
            sig_vertex_ordinal: group_vertex.0,
            state_sha256: sha256_hex(group_state.as_bytes()),
        }),
    };
    let certificate = project_native_stems_beam_vlink_base_apply_certificate(
        sig,
        bindings,
        relation.beam,
        stem_vertex,
        relation,
        reuse_check.plan,
    )?;
    Ok(NativeStemsBeamVLinkBaseApplyState {
        transaction_state: transaction_state.clone(),
        inter_index: NativeStemsBeamInterIndexApplyState {
            baseline_entry_count,
            baseline_provenance_sha256: sha256_hex(inter_lineage.as_bytes()),
            beam_lookup: NativeStemsBeamInterIndexLookup::PresentSameObject {
                index_ordinal: beam_vertex.0,
                inter_id: beam_inter_id,
                vip: false,
                object_matches: 1,
                inter_id_matches: 1,
                glyph_active_matches: 0,
                glyph_original_matches: 0,
            },
            stem_lookup: stem_vertex.map_or(NativeStemsBeamInterIndexLookup::Absent, |vertex| {
                NativeStemsBeamInterIndexLookup::PresentSameObject {
                    index_ordinal: vertex.0,
                    inter_id: final_stem.inter_id.expect("existing stem has an ID"),
                    vip: false,
                    object_matches: 1,
                    inter_id_matches: 1,
                    glyph_active_matches: 0,
                    glyph_original_matches: 0,
                }
            }),
            next_id_lookup: next_id.map_or(
                NativeStemsBeamNextPersistentIdLookup::NotRead,
                |persistent_id| NativeStemsBeamNextPersistentIdLookup::VacantAndNotVip {
                    persistent_id,
                    inter_id_matches: 0,
                    glyph_active_matches: 0,
                    glyph_original_matches: 0,
                    configured_vip_matches: 0,
                },
            ),
            appended_entries: Vec::new(),
        },
        sig: NativeStemsBeamSigApplyState {
            system_id: sig.system_id,
            baseline_vertex_count: sig.vertices.len(),
            baseline_vertex_provenance_sha256: sha256_hex(vertex_lineage.as_bytes()),
            baseline_relation_count: sig.edges.len(),
            baseline_relation_provenance_sha256: sha256_hex(edge_lineage.as_bytes()),
            beam_vertex: NativeStemsBeamSigVertexLookup::PresentSameObject {
                vertex_ordinal: beam_vertex.0,
                sig_vertex_identity: beam_vertex.0,
                inter_id: beam_runtime.inter_id,
                object_matches: 1,
            },
            stem_vertex: stem_vertex.map_or(NativeStemsBeamSigVertexLookup::Absent, |vertex| {
                NativeStemsBeamSigVertexLookup::PresentSameObject {
                    vertex_ordinal: vertex.0,
                    sig_vertex_identity: vertex.0,
                    inter_id: final_stem.inter_id.expect("existing stem has an ID"),
                    object_matches: 1,
                }
            }),
            appended_vertices: Vec::new(),
            appended_relations: Vec::new(),
            listener_topology: NativeStemsBeamSigListenerTopology::SoleStandardSigListener,
            beam: beam_runtime,
            stem: NativeStemsBeamVLinkStemRuntimeState {
                stem_identity: final_stem.stem_identity,
                sig_vertex_identity: stem_vertex.map(|vertex| vertex.0),
                inter_indexed: stem_vertex.is_some(),
                sig_system_id: stem_vertex.map(|_| sig.system_id),
                removed: stem_vertex
                    .and_then(|vertex| sig.vertex(vertex.0))
                    .is_some_and(|vertex| vertex.removed),
                vip: false,
                abnormal: stem_vertex
                    .and_then(|vertex| sig.vertex(vertex.0))
                    .is_some_and(|vertex| vertex.abnormal),
            },
        },
        sheet_edit: prior.sheet_edit,
        certificate: Some(certificate),
        committed: None,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamVLinkVertexAction {
    SkippedPositiveInterId,
    RegisteredAndAdded {
        inter_id: i32,
        sig_vertex_identity: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamVLinkBaseApplyDisposition {
    Added { graph_relation_identity: usize },
    SuppressedSourceRemoved,
    SuppressedTargetRemoved,
    SuppressedExistingBeamStem { graph_relation_identity: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamVLinkBaseApplyOperation {
    SharedPersistentIdAdvanced {
        before: i32,
        after: i32,
    },
    StemInterIdAssigned {
        stem_identity: usize,
        inter_id: i32,
    },
    InterIndexInserted {
        stem_identity: usize,
        inter_id: i32,
    },
    SigVertexInserted {
        sig_vertex_identity: usize,
    },
    StemSigAttached {
        system_id: usize,
    },
    StemRemovedCleared {
        before: bool,
    },
    StemAbnormalSet {
        before: bool,
        after: bool,
    },
    SigVertexEventDispatched,
    StandardSigListenerVertexCallbackCompleted,
    StemAddedCallbackStarted,
    StemAddedCallbackCompleted,
    SheetStubModifiedSetTrue,
    BookModifiedSetTrue,
    BookDirtySetTrue,
    SigGlobalRelationInserted {
        graph_relation_identity: usize,
    },
    BeamOutgoingRelationInserted {
        graph_relation_identity: usize,
    },
    StemIncomingRelationInserted {
        graph_relation_identity: usize,
    },
    SigEdgeEventDispatched {
        graph_relation_identity: usize,
    },
    StandardSigListenerEdgeCallbackStarted,
    BeamStemRelationCallbackStarted,
    StemChordIncidentScanCompleted {
        incident_relation_count: usize,
        chord_stem_matches: usize,
    },
    BeamAbnormalSet {
        before: bool,
        after: bool,
    },
    BeamStemRelationCallbackCompleted,
    StandardSigListenerEdgeCallbackCompleted,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkRemovedReadTrace {
    pub source_removed: bool,
    pub target_removed: Option<bool>,
    pub directed_pair_relations_read: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamVLinkBeamAbnormalTrace {
    NotReadSuppressed,
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
pub struct NativeStemsBeamVLinkBaseCallbackTrace {
    pub called: bool,
    pub extension_preserved: bool,
    pub beam_portion_preserved: bool,
    pub stem_incident_graph_relation_identities: Vec<usize>,
    pub chord_stem_matches: usize,
    pub chord_cache_invalidation_count: usize,
    pub beam_abnormal: NativeStemsBeamVLinkBeamAbnormalTrace,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamVLinkBaseApplyOutcome {
    ReadyBeforeBLinkerFlagMutation {
        apply_returned: bool,
        continuation_support_grade: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkBaseApplyTransaction {
    pub key: NativeStemsBeamVLinkBaseApplyKey,
    pub stem_before: NativeStemsBeamKnownSystemStem,
    pub stem_after: NativeStemsBeamKnownSystemStem,
    pub fresh_relation_object_identity: NativeStemsBeamRelationObjectIdentity,
    pub fresh_relation: NativeStemsBeamRelationDraft,
    pub continuation_support_grade: f64,
    pub graph_relation_identity: Option<usize>,
    pub vertex_action: NativeStemsBeamVLinkVertexAction,
    pub apply_disposition: NativeStemsBeamVLinkBaseApplyDisposition,
    pub apply_returned: bool,
    pub removed_reads: NativeStemsBeamVLinkRemovedReadTrace,
    pub directed_pair_graph_relation_identities: Vec<usize>,
    pub callback: NativeStemsBeamVLinkBaseCallbackTrace,
    pub consumed_certificate: NativeStemsBeamVLinkBaseApplyCertificate,
    pub operations: Vec<NativeStemsBeamVLinkBaseApplyOperation>,
    pub sheet_edit_before: NativeStemsBeamSheetEditState,
    pub sheet_edit_after: NativeStemsBeamSheetEditState,
    pub persistent_id_mutation_count: usize,
    pub inter_index_mutation_count: usize,
    pub sig_vertex_mutation_count: usize,
    pub sig_relation_mutation_count: usize,
    pub stem_abnormal_mutation_count: usize,
    pub beam_abnormal_mutation_count: usize,
    pub beam_group_mutation_count: usize,
    pub linker_flag_mutation_count: usize,
    pub sibling_link_mutation_count: usize,
    pub head_link_mutation_count: usize,
    pub outcome: NativeStemsBeamVLinkBaseApplyOutcome,
    /// Complete committed state, for exact state/result projection.
    pub state_after: Box<NativeStemsBeamVLinkBaseApplyState>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamVLinkBaseApplyError {
    Predecessor { phase: &'static str },
    PredecessorMismatch,
    PredecessorNotReady,
    InvalidState { phase: &'static str },
    InvalidEvidence { phase: &'static str },
    UnsupportedV1 { phase: &'static str },
}

impl fmt::Display for NativeStemsBeamVLinkBaseApplyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid beam VLink base-apply boundary: {self:?}"
        )
    }
}

impl Error for NativeStemsBeamVLinkBaseApplyError {}

/// Commit `SIGraph.addVertex` (only for an ID-zero stem) and the fresh base
/// BeamStem `Link.applyTo`. All certificates are validated and the complete
/// boundary-13 result is reconstructed before the first state change.
#[allow(clippy::too_many_arguments)]
pub fn apply_native_stems_beam_vlink_base_transaction(
    scheduler_system: &NativeStemsBeamSchedulerSystem,
    plan_system: &NativeStemsBeamLinkPlanSystem,
    stump_system: &NativeStemsBeamStumpSystem,
    vlinker_system: &NativeStemsBeamVLinkerSystem,
    create_transaction: &NativeStemsBeamVLinkTransaction,
    reuse_live_state: &NativeStemsBeamVLinkReuseLiveState,
    relation_parameters: NativeStemsBeamRelationParameters,
    reuse_check: &NativeStemsBeamVLinkReuseCheck,
    state: &mut NativeStemsBeamVLinkBaseApplyState,
) -> Result<NativeStemsBeamVLinkBaseApplyTransaction, NativeStemsBeamVLinkBaseApplyError> {
    let reconstructed = evaluate_native_stems_beam_vlink_reuse_check(
        scheduler_system,
        plan_system,
        stump_system,
        vlinker_system,
        create_transaction,
        &state.transaction_state,
        reuse_live_state,
        relation_parameters,
    )
    .map_err(|_| NativeStemsBeamVLinkBaseApplyError::Predecessor {
        phase: "boundary-13 reconstruction",
    })?;
    if &reconstructed != reuse_check {
        return Err(NativeStemsBeamVLinkBaseApplyError::PredecessorMismatch);
    }
    let NativeStemsBeamVLinkReuseCheckOutcome::ReadyBeforeSigMutation { relation } =
        &reuse_check.outcome
    else {
        return Err(NativeStemsBeamVLinkBaseApplyError::PredecessorNotReady);
    };
    let stem = reuse_check
        .final_stem
        .as_ref()
        .ok_or(NativeStemsBeamVLinkBaseApplyError::PredecessorNotReady)?;
    let prepared = prepare_commit(
        stump_system,
        create_transaction,
        reuse_live_state,
        reuse_check,
        relation,
        stem,
        state,
    )?;
    Ok(commit(prepared, state))
}

/// Project and apply B14 against the mutable port-owned SIG.
///
/// The graph and compact boundary state are cloned and committed together, so
/// any projection, predecessor, validation, or graph-mutation failure leaves
/// all caller-owned state unchanged. Java persistent IDs remain part of the
/// compact replay state, but are never imported into the native graph query
/// certificate.
#[allow(clippy::too_many_arguments)]
pub fn apply_native_stems_beam_vlink_base_transaction_to_native_sig(
    scheduler_system: &NativeStemsBeamSchedulerSystem,
    plan_system: &NativeStemsBeamLinkPlanSystem,
    stump_system: &NativeStemsBeamStumpSystem,
    vlinker_system: &NativeStemsBeamVLinkerSystem,
    create_transaction: &NativeStemsBeamVLinkTransaction,
    reuse_live_state: &NativeStemsBeamVLinkReuseLiveState,
    relation_parameters: NativeStemsBeamRelationParameters,
    reuse_check: &NativeStemsBeamVLinkReuseCheck,
    state: &mut NativeStemsBeamVLinkBaseApplyState,
    sig: &mut NativeSigSystem,
    bindings: &mut NativeSigSystemBindings,
) -> Result<NativeStemsBeamVLinkBaseApplyTransaction, NativeStemsBeamVLinkBaseApplyError> {
    let NativeStemsBeamVLinkReuseCheckOutcome::ReadyBeforeSigMutation { relation } =
        &reuse_check.outcome
    else {
        return Err(NativeStemsBeamVLinkBaseApplyError::PredecessorNotReady);
    };
    let stem = reuse_check
        .final_stem
        .as_ref()
        .ok_or(NativeStemsBeamVLinkBaseApplyError::PredecessorNotReady)?;
    if sig.system_id != state.sig.system_id
        || bindings.system_id != sig.system_id
        || sig.vertices.len() != state.sig.baseline_vertex_count
        || sig.edges.len() != state.sig.baseline_relation_count
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native SIG base-apply baseline join",
        });
    }
    let stem_vertex = bindings.stem_vertices.get(&stem.stem_identity).copied();
    if stem_vertex.map(|vertex| vertex.0) != state.sig.stem.sig_vertex_identity {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native SIG stem binding join",
        });
    }

    let certificate = project_native_stems_beam_vlink_base_apply_certificate(
        sig,
        bindings,
        relation.beam,
        stem_vertex,
        relation,
        reuse_check.plan,
    )?;
    let mut next_state = state.clone();
    next_state.certificate = Some(certificate);
    let mut next_sig = sig.clone();
    let mut next_bindings = bindings.clone();
    let transaction = apply_native_stems_beam_vlink_base_transaction(
        scheduler_system,
        plan_system,
        stump_system,
        vlinker_system,
        create_transaction,
        reuse_live_state,
        relation_parameters,
        reuse_check,
        &mut next_state,
    )?;

    let effective_stem_vertex = match transaction.vertex_action {
        NativeStemsBeamVLinkVertexAction::RegisteredAndAdded {
            sig_vertex_identity,
            ..
        } => {
            let bounds = &transaction.stem_after.geometry.ribbon_bounds;
            let grade = match &transaction.stem_after.grade {
                crate::native_stems_beam_vlink_transaction::NativeStemsBeamStemGrade::Checked(
                    check,
                ) => check.grade,
                crate::native_stems_beam_vlink_transaction::NativeStemsBeamStemGrade::Artificial(
                    grade,
                ) => *grade,
            };
            let vertex = NativeSigVertexId(sig_vertex_identity);
            next_sig
                .append_vertex(NativeSigVertex {
                    ordinal: vertex.0,
                    active: true,
                    removed: false,
                    kind: NativeSigInterKind::Stem,
                    shape: Some("STEM".to_owned()),
                    grade,
                    bounds: NativeSigBounds {
                        x: bounds.x,
                        y: bounds.y,
                        width: bounds.width,
                        height: bounds.height,
                    },
                    abnormal: transaction.stem_after.abnormal,
                    beam_geometry: None,
                })
                .map_err(|_| NativeStemsBeamVLinkBaseApplyError::InvalidState {
                    phase: "native SIG stem append",
                })?;
            next_bindings
                .bind_stem(transaction.stem_after.stem_identity, vertex)
                .map_err(|_| NativeStemsBeamVLinkBaseApplyError::InvalidState {
                    phase: "native SIG stem binding append",
                })?;
            vertex
        }
        NativeStemsBeamVLinkVertexAction::SkippedPositiveInterId => {
            stem_vertex.ok_or(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                phase: "native SIG existing stem binding",
            })?
        }
    };

    if let NativeStemsBeamVLinkBaseApplyDisposition::Added {
        graph_relation_identity,
    } = transaction.apply_disposition
    {
        let beam_vertex = *next_bindings.beam_vertices.get(&relation.beam).ok_or(
            NativeStemsBeamVLinkBaseApplyError::InvalidState {
                phase: "native SIG committed beam binding",
            },
        )?;
        next_sig
            .append_edge(NativeSigEdge {
                ordinal: graph_relation_identity,
                active: true,
                source: beam_vertex.0,
                target: effective_stem_vertex.0,
                kind: NativeSigRelationKind::BeamStem,
                origin: crate::native_sig::NativeSigRelationOrigin::BeamVBaseDraft {
                    plan_ordinal: reuse_check.plan.plan_ordinal,
                },
                support: Some(NativeSigSupport {
                    grade: transaction.fresh_relation.grade,
                    bar_connection_impacts: None,
                }),
                beam_portion: Some(transaction.fresh_relation.beam_portion),
                stem_extension: Some(transaction.fresh_relation.extension_point),
                head_stem: None,
            })
            .map_err(|_| NativeStemsBeamVLinkBaseApplyError::InvalidState {
                phase: "native SIG BeamStem append",
            })?;
    }
    let beam_vertex = *next_bindings.beam_vertices.get(&relation.beam).ok_or(
        NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native SIG committed beam binding",
        },
    )?;
    let beam_abnormal_after = match transaction.callback.beam_abnormal {
        NativeStemsBeamVLinkBeamAbnormalTrace::NotReadSuppressed => None,
        NativeStemsBeamVLinkBeamAbnormalTrace::HookAnyBeamStem { after, .. }
        | NativeStemsBeamVLinkBeamAbnormalTrace::RawBeamSides { after, .. } => Some(after),
    };
    if let Some(abnormal) = beam_abnormal_after {
        next_sig.set_abnormal(beam_vertex, abnormal).map_err(|_| {
            NativeStemsBeamVLinkBaseApplyError::InvalidState {
                phase: "native SIG beam abnormal update",
            }
        })?;
    }
    next_sig
        .set_abnormal(effective_stem_vertex, transaction.stem_after.abnormal)
        .map_err(|_| NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native SIG stem abnormal update",
        })?;
    next_sig.validate_integrity().map_err(|_| {
        NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native SIG committed integrity",
        }
    })?;
    next_bindings.validate_against(&next_sig).map_err(|_| {
        NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native SIG committed binding integrity",
        }
    })?;

    *state = next_state;
    *sig = next_sig;
    *bindings = next_bindings;
    Ok(transaction)
}

#[derive(Clone)]
struct PreparedCommit {
    key: NativeStemsBeamVLinkBaseApplyKey,
    stem_before: NativeStemsBeamKnownSystemStem,
    relation: NativeStemsBeamRelationDraft,
    certificate: NativeStemsBeamVLinkBaseApplyCertificate,
    vertex_inter_id: Option<i32>,
    vertex_identity: Option<usize>,
    apply_disposition: NativeStemsBeamVLinkBaseApplyDisposition,
    pair_relations_read: usize,
    graph_relation_identity: Option<usize>,
    post_stem_incident: Vec<usize>,
    post_beam_incident: Vec<usize>,
    beam_abnormal_after: Option<bool>,
    beam_abnormal_read_count: usize,
    raw_left_found: bool,
    raw_right_found: bool,
    source_removed: bool,
    target_removed_read: Option<bool>,
}

fn prepare_commit(
    stump_system: &NativeStemsBeamStumpSystem,
    create_transaction: &NativeStemsBeamVLinkTransaction,
    reuse_live_state: &NativeStemsBeamVLinkReuseLiveState,
    reuse_check: &NativeStemsBeamVLinkReuseCheck,
    relation: &NativeStemsBeamRelationDraft,
    stem: &NativeStemsBeamKnownSystemStem,
    state: &NativeStemsBeamVLinkBaseApplyState,
) -> Result<PreparedCommit, NativeStemsBeamVLinkBaseApplyError> {
    if state.committed.is_some() {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "one-shot transaction already committed",
        });
    }
    let certificate =
        state
            .certificate
            .as_ref()
            .ok_or(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "missing one-shot certificate",
            })?;
    let key = NativeStemsBeamVLinkBaseApplyKey {
        system_id: reuse_check.system_id,
        invocation_ordinal: reuse_check.invocation_ordinal,
        plan: reuse_check.plan,
    };
    validate_fresh_relation_identity(certificate, key.plan.plan_ordinal)?;
    let shared_sheet_frontier = matches!(
        create_transaction.scope,
        crate::native_stems_beam_vlink_transaction::NativeStemsBeamVLinkTransactionScope::SharedSheetFirstFrontier { .. }
    );
    if certificate.system_id != key.system_id
        || state.sig.system_id != key.system_id
        || create_transaction.system_id != key.system_id
        || state.sig.beam.source != relation.beam
        || state.sig.stem.stem_identity != stem.stem_identity
        || state.sig.stem.abnormal != stem.abnormal
        || state.sig.stem.sig_system_id != stem.sig_attached.then_some(key.system_id)
        || state.sig.stem.inter_indexed != (stem.inter_id.is_some() && !state.sig.stem.removed)
        || state.sig.beam.sig_system_id != key.system_id
        || state.sig.beam.inter_indexed == state.sig.beam.removed
        || state.sig.beam.inter_id <= 0
        || (shared_sheet_frontier && !state.sig.beam.removed && state.sig.beam.beam_group.is_none())
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "endpoint/predecessor join",
        });
    }
    let beam = stump_system
        .beams_by_abscissa
        .iter()
        .find(|beam| beam.source == relation.beam)
        .ok_or(NativeStemsBeamVLinkBaseApplyError::Predecessor {
            phase: "starting beam",
        })?;
    if beam.group_ordinal != state.sig.beam.stump_group_ordinal {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "beam group identity",
        });
    }
    let payload_views = state
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .chain(reuse_live_state.live_sig_stems.iter())
        .filter(|known| known.stem_identity == stem.stem_identity)
        .collect::<Vec<_>>();
    if payload_views.is_empty() || payload_views.iter().any(|known| *known != stem) {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "final stem object alias",
        });
    }
    if stem.inter_id.is_none()
        && state
            .transaction_state
            .system_stems
            .known_stems
            .iter()
            .filter(|known| known.stem_identity == stem.stem_identity)
            .count()
            != 1
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "ID-zero systemStems shared-object alias",
        });
    }

    validate_compact_state(state, certificate, stem)?;
    let ids = state.transaction_state.glyph_index.persistent_ids;
    let (vertex_inter_id, vertex_identity) = match stem.inter_id {
        None => {
            let next_id = ids.sheet_last_id.checked_add(1).ok_or(
                NativeStemsBeamVLinkBaseApplyError::UnsupportedV1 {
                    phase: "shared persistent ID overflow",
                },
            )?;
            if state.sig.stem.sig_vertex_identity.is_some()
                || state.sig.stem.inter_indexed
                || state.sig.stem.sig_system_id.is_some()
                || state.sig.stem.removed
                || state.sig.stem.abnormal
                || state.inter_index.stem_lookup != NativeStemsBeamInterIndexLookup::Absent
                || state.sig.stem_vertex != NativeStemsBeamSigVertexLookup::Absent
                || state.inter_index.next_id_lookup
                    != (NativeStemsBeamNextPersistentIdLookup::VacantAndNotVip {
                        persistent_id: next_id,
                        inter_id_matches: 0,
                        glyph_active_matches: 0,
                        glyph_original_matches: 0,
                        configured_vip_matches: 0,
                    })
            {
                return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                    phase: "ID-zero registration lookup",
                });
            }
            let vertex_identity = state.sig.baseline_vertex_count;
            validate_new_vertex_capacity(state.inter_index.baseline_entry_count, vertex_identity)?;
            (Some(next_id), Some(vertex_identity))
        }
        Some(inter_id) => {
            if inter_id <= 0
                || inter_id > ids.sheet_last_id
                || state.sig.stem.sig_system_id != Some(key.system_id)
                || state.inter_index.next_id_lookup
                    != NativeStemsBeamNextPersistentIdLookup::NotRead
            {
                return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                    phase: "existing stem registration state",
                });
            }
            validate_existing_endpoint_state(state, inter_id)?;
            (None, None)
        }
    };

    let certificate_beam_inter_id = certificate_endpoint_id(
        certificate,
        state.sig.beam.sig_vertex_identity,
        state.sig.beam.inter_id,
        "beam certificate endpoint identity",
    )?;
    let certificate_stem_inter_id = certificate_endpoint_id(
        certificate,
        state.sig.stem.sig_vertex_identity.or(vertex_identity),
        stem.inter_id
            .or(vertex_inter_id)
            .expect("stem ID was resolved"),
        "stem certificate endpoint identity",
    )?;

    let pair = &certificate.directed_pair_scan.relations;
    if stem.inter_id.is_none() && !pair.is_empty() {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
            phase: "ID-zero pre-edge relation evidence",
        });
    }
    validate_directed_pair_query(state, certificate, certificate_stem_inter_id)?;

    let first_beam_stem = pair
        .iter()
        .position(|edge| edge.kind == NativeStemsBeamQueryRelationKind::BeamStem);
    let next_relation_identity = state.sig.baseline_relation_count;
    if !state.sig.beam.removed
        && !state.sig.stem.removed
        && first_beam_stem.is_none()
        && next_relation_identity.checked_add(1).is_none()
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::UnsupportedV1 {
            phase: "SIG relation identity overflow",
        });
    }
    let (apply_disposition, pair_relations_read, graph_relation_identity) =
        if state.sig.beam.removed {
            (
                NativeStemsBeamVLinkBaseApplyDisposition::SuppressedSourceRemoved,
                0,
                None,
            )
        } else if state.sig.stem.removed {
            (
                NativeStemsBeamVLinkBaseApplyDisposition::SuppressedTargetRemoved,
                0,
                None,
            )
        } else if let Some(index) = first_beam_stem {
            (
                NativeStemsBeamVLinkBaseApplyDisposition::SuppressedExistingBeamStem {
                    graph_relation_identity: pair[index].graph_relation_identity,
                },
                pair.iter()
                    .filter(|row| row.class_read != NativeStemsBeamPairClassRead::UnreadAfterBreak)
                    .count(),
                None,
            )
        } else {
            (
                NativeStemsBeamVLinkBaseApplyDisposition::Added {
                    graph_relation_identity: next_relation_identity,
                },
                pair.len(),
                Some(next_relation_identity),
            )
        };

    validate_callback_queries(
        state,
        certificate,
        relation,
        graph_relation_identity,
        certificate_beam_inter_id,
        certificate_stem_inter_id,
    )?;
    validate_zero_chord_envelope(certificate)?;

    let post_stem_incident = certificate
        .stem_incident_after
        .relations
        .iter()
        .map(|row| row.graph_relation_identity)
        .collect::<Vec<_>>();
    let post_beam_incident = certificate
        .beam_incident_after
        .relations
        .iter()
        .map(|row| row.graph_relation_identity)
        .collect::<Vec<_>>();
    let mut beam_abnormal_after = None;
    let mut beam_abnormal_read_count = 0;
    let mut raw_left_found = false;
    let mut raw_right_found = false;
    if graph_relation_identity.is_some() {
        if matches!(relation.beam, NativeStemsBeamSource::Hook(_)) {
            beam_abnormal_read_count = certificate
                .beam_incident_after
                .relations
                .iter()
                .filter(|row| row.read == NativeStemsBeamBeamIncidentRead::Examined)
                .count();
            beam_abnormal_after = Some(false);
        } else {
            for row in &certificate.beam_incident_after.relations {
                match row.kind {
                    NativeStemsBeamQueryRelationKind::BeamStem
                    | NativeStemsBeamQueryRelationKind::BeamRest => match row.beam_portion {
                        Some(NativeBeamPortion::Left) => raw_left_found = true,
                        Some(NativeBeamPortion::Right) => raw_right_found = true,
                        _ => {}
                    },
                    _ => {}
                }
            }
            beam_abnormal_read_count = post_beam_incident.len();
            beam_abnormal_after = Some(!raw_left_found || !raw_right_found);
        }
    }

    Ok(PreparedCommit {
        key,
        stem_before: stem.clone(),
        relation: relation.clone(),
        certificate: certificate.clone(),
        vertex_inter_id,
        vertex_identity,
        apply_disposition,
        pair_relations_read,
        graph_relation_identity,
        post_stem_incident,
        post_beam_incident,
        beam_abnormal_after,
        beam_abnormal_read_count,
        raw_left_found,
        raw_right_found,
        source_removed: state.sig.beam.removed,
        target_removed_read: (!state.sig.beam.removed).then_some(state.sig.stem.removed),
    })
}

fn validate_compact_state(
    state: &NativeStemsBeamVLinkBaseApplyState,
    certificate: &NativeStemsBeamVLinkBaseApplyCertificate,
    stem: &NativeStemsBeamKnownSystemStem,
) -> Result<(), NativeStemsBeamVLinkBaseApplyError> {
    if !certificate.headless
        || certificate.listener_topology
            != NativeStemsBeamSigListenerTopology::SoleStandardSigListener
        || state.sig.listener_topology != certificate.listener_topology
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::UnsupportedV1 {
            phase: "headless/listener envelope",
        });
    }
    if ![
        &state.inter_index.baseline_provenance_sha256,
        &state.sig.baseline_vertex_provenance_sha256,
        &state.sig.baseline_relation_provenance_sha256,
        &certificate.stem_incident_before.query_provenance_sha256,
        &certificate.stem_incident_after.query_provenance_sha256,
        &certificate.beam_incident_before.query_provenance_sha256,
        &certificate.beam_incident_after.query_provenance_sha256,
    ]
    .into_iter()
    .all(|hash| valid_sha256(hash))
        || !state.inter_index.appended_entries.is_empty()
        || !state.sig.appended_vertices.is_empty()
        || !state.sig.appended_relations.is_empty()
        || certificate.fresh_relation_graph_matches != 0
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "compact baseline/append state",
        });
    }
    let ids = state.transaction_state.glyph_index.persistent_ids;
    if state.sig.beam.inter_id > ids.sheet_last_id {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "beam persistent ID range",
        });
    }
    if state.sig.beam.removed {
        if state.sig.beam.inter_indexed
            || state.sig.beam.sig_vertex_identity.is_some()
            || state.inter_index.beam_lookup != NativeStemsBeamInterIndexLookup::Absent
            || state.sig.beam_vertex != NativeStemsBeamSigVertexLookup::Absent
        {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                phase: "removed beam live membership",
            });
        }
    } else {
        if !state.sig.beam.inter_indexed {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                phase: "live beam InterIndex membership",
            });
        }
        validate_index_lookup(
            state.inter_index.beam_lookup,
            state.inter_index.baseline_entry_count,
            state.sig.beam.inter_id,
            state.sig.beam.vip,
            "beam InterIndex lookup",
        )?;
        validate_vertex_lookup(
            state.sig.beam_vertex,
            state.sig.baseline_vertex_count,
            state.sig.beam.inter_id,
            state.sig.beam.sig_vertex_identity,
            "beam SIG lookup",
        )?;
    }
    if state.sig.beam.beam_group.as_ref().is_some_and(|group| {
        group.sig_vertex_ordinal >= state.sig.baseline_vertex_count
            || state.sig.beam.sig_vertex_identity == Some(group.sig_vertex_ordinal)
            || !valid_sha256(&group.state_sha256)
    }) {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "beam-group live identity/state",
        });
    }
    if stem.inter_id.is_none()
        && (state.inter_index.stem_lookup != NativeStemsBeamInterIndexLookup::Absent
            || state.sig.stem_vertex != NativeStemsBeamSigVertexLookup::Absent)
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "ID-zero endpoint absence",
        });
    }
    Ok(())
}

fn validate_fresh_relation_identity(
    certificate: &NativeStemsBeamVLinkBaseApplyCertificate,
    plan_ordinal: usize,
) -> Result<(), NativeStemsBeamVLinkBaseApplyError> {
    if certificate.fresh_relation_object_identity
        != NativeStemsBeamRelationObjectIdentity::FreshDraft(plan_ordinal)
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
            phase: "fresh relation draft identity",
        });
    }
    Ok(())
}

fn validate_existing_endpoint_state(
    state: &NativeStemsBeamVLinkBaseApplyState,
    stem_inter_id: i32,
) -> Result<Option<usize>, NativeStemsBeamVLinkBaseApplyError> {
    if stem_inter_id == state.sig.beam.inter_id {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "beam/stem endpoint identity collision",
        });
    }
    if state.sig.stem.removed {
        if state.sig.stem.inter_indexed
            || state.sig.stem.sig_vertex_identity.is_some()
            || state.inter_index.stem_lookup != NativeStemsBeamInterIndexLookup::Absent
            || state.sig.stem_vertex != NativeStemsBeamSigVertexLookup::Absent
        {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                phase: "removed stem live membership",
            });
        }
        return Ok(None);
    }
    if !state.sig.stem.inter_indexed {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "live stem InterIndex membership",
        });
    }
    let stem_index_ordinal = validate_index_lookup(
        state.inter_index.stem_lookup,
        state.inter_index.baseline_entry_count,
        stem_inter_id,
        state.sig.stem.vip,
        "existing stem InterIndex lookup",
    )?;
    let stem_vertex_identity = validate_vertex_lookup(
        state.sig.stem_vertex,
        state.sig.baseline_vertex_count,
        stem_inter_id,
        state.sig.stem.sig_vertex_identity,
        "existing stem SIG lookup",
    )?;
    if let Some(beam_vertex_identity) = state.sig.beam.sig_vertex_identity {
        let beam_index_ordinal = validate_index_lookup(
            state.inter_index.beam_lookup,
            state.inter_index.baseline_entry_count,
            state.sig.beam.inter_id,
            state.sig.beam.vip,
            "beam InterIndex lookup",
        )?;
        if stem_index_ordinal == beam_index_ordinal || stem_vertex_identity == beam_vertex_identity
        {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
                phase: "beam/stem endpoint identity collision",
            });
        }
    }
    if state
        .sig
        .beam
        .beam_group
        .as_ref()
        .is_some_and(|group| group.sig_vertex_ordinal == stem_vertex_identity)
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "beam/stem endpoint identity collision",
        });
    }
    Ok(Some(stem_vertex_identity))
}

fn validate_zero_chord_envelope(
    certificate: &NativeStemsBeamVLinkBaseApplyCertificate,
) -> Result<(), NativeStemsBeamVLinkBaseApplyError> {
    if certificate.chord_stem_matches != 0 {
        return Err(NativeStemsBeamVLinkBaseApplyError::UnsupportedV1 {
            phase: "ChordStem callback mutation",
        });
    }
    Ok(())
}

fn validate_new_vertex_capacity(
    baseline_entry_count: usize,
    baseline_vertex_count: usize,
) -> Result<(), NativeStemsBeamVLinkBaseApplyError> {
    if baseline_entry_count.checked_add(1).is_none() {
        return Err(NativeStemsBeamVLinkBaseApplyError::UnsupportedV1 {
            phase: "InterIndex ordinal overflow",
        });
    }
    if baseline_vertex_count.checked_add(1).is_none() {
        return Err(NativeStemsBeamVLinkBaseApplyError::UnsupportedV1 {
            phase: "SIG vertex identity overflow",
        });
    }
    Ok(())
}

fn validate_index_lookup(
    lookup: NativeStemsBeamInterIndexLookup,
    baseline_count: usize,
    expected_inter_id: i32,
    expected_vip: bool,
    phase: &'static str,
) -> Result<usize, NativeStemsBeamVLinkBaseApplyError> {
    match lookup {
        NativeStemsBeamInterIndexLookup::PresentSameObject {
            index_ordinal,
            inter_id,
            vip,
            object_matches: 1,
            inter_id_matches: 1,
            glyph_active_matches: 0,
            glyph_original_matches: 0,
        } if index_ordinal < baseline_count
            && inter_id == expected_inter_id
            && inter_id > 0
            && vip == expected_vip =>
        {
            Ok(index_ordinal)
        }
        _ => Err(NativeStemsBeamVLinkBaseApplyError::InvalidState { phase }),
    }
}

fn validate_vertex_lookup(
    lookup: NativeStemsBeamSigVertexLookup,
    baseline_count: usize,
    expected_inter_id: i32,
    expected_identity: Option<usize>,
    phase: &'static str,
) -> Result<usize, NativeStemsBeamVLinkBaseApplyError> {
    match lookup {
        NativeStemsBeamSigVertexLookup::PresentSameObject {
            vertex_ordinal,
            sig_vertex_identity,
            inter_id,
            object_matches: 1,
        } if vertex_ordinal < baseline_count
            && sig_vertex_identity == vertex_ordinal
            && inter_id == expected_inter_id
            && expected_identity == Some(sig_vertex_identity) =>
        {
            Ok(sig_vertex_identity)
        }
        _ => Err(NativeStemsBeamVLinkBaseApplyError::InvalidState { phase }),
    }
}

fn validate_directed_pair_query(
    state: &NativeStemsBeamVLinkBaseApplyState,
    certificate: &NativeStemsBeamVLinkBaseApplyCertificate,
    target_inter_id: i32,
) -> Result<(), NativeStemsBeamVLinkBaseApplyError> {
    let scan = &certificate.directed_pair_scan;
    if scan.query_relation_count != scan.relations.len() {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
            phase: "directed-pair query coverage",
        });
    }
    if state.sig.beam.removed || state.sig.stem.removed {
        if scan.source_outgoing_scanned != 0
            || !scan.relations.is_empty()
            || scan.source_outgoing_provenance != NativeStemsBeamQueryProvenance::NotRead
            || scan.pair_provenance != NativeStemsBeamQueryProvenance::NotRead
        {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "lazy removed-endpoint pair query",
            });
        }
        return Ok(());
    }
    let outgoing_hash = query_rows_sha256(
        certificate
            .beam_incident_before
            .relations
            .iter()
            .filter(|row| row.direction == NativeStemsBeamIncidentDirection::Outgoing)
            .map(|row| {
                format!(
                    "{}:{}",
                    graph_relation_alias(row.graph_relation_identity),
                    row.relation_class,
                )
            }),
    );
    let pair_hash = query_rows_sha256(scan.relations.iter().map(|row| {
        format!(
            "{}:{}:{}:{}",
            row.pair_ordinal,
            row.source_outgoing_ordinal,
            graph_relation_alias(row.graph_relation_identity),
            row.relation_class,
        )
    }));
    if !matches!(
        &scan.source_outgoing_provenance,
        NativeStemsBeamQueryProvenance::ExhaustiveSha256(hash)
            if valid_sha256(hash) && *hash == outgoing_hash
    ) || !matches!(
        &scan.pair_provenance,
        NativeStemsBeamQueryProvenance::ExhaustiveSha256(hash)
            if valid_sha256(hash) && *hash == pair_hash
    ) {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
            phase: "directed-pair query provenance",
        });
    }
    let mut matched = false;
    for (index, row) in scan.relations.iter().enumerate() {
        let expected_read = if matched {
            NativeStemsBeamPairClassRead::UnreadAfterBreak
        } else if relation_is_beam_stem(row.kind) {
            matched = true;
            NativeStemsBeamPairClassRead::ExaminedMatchBreak
        } else {
            NativeStemsBeamPairClassRead::ExaminedContinue
        };
        if row.pair_ordinal != index
            || row.source_outgoing_ordinal >= scan.source_outgoing_scanned
            || row.graph_relation_identity >= state.sig.baseline_relation_count
            || row.relation_object_identity
                != NativeStemsBeamRelationObjectIdentity::GraphObject(row.graph_relation_identity)
            || !relation_class_matches_kind(&row.relation_class, row.kind)
            || row.class_read != expected_read
            || scan.relations[..index].iter().any(|prior| {
                prior.source_outgoing_ordinal >= row.source_outgoing_ordinal
                    || prior.graph_relation_identity >= row.graph_relation_identity
                    || prior.relation_object_identity == row.relation_object_identity
            })
        {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "directed-pair query order/payload",
            });
        }
    }
    let outgoing = certificate
        .beam_incident_before
        .relations
        .iter()
        .filter(|row| row.direction == NativeStemsBeamIncidentDirection::Outgoing)
        .collect::<Vec<_>>();
    let expected_pair = outgoing
        .iter()
        .filter(|row| row.opposite_inter_id == target_inter_id)
        .map(|row| row.graph_relation_identity)
        .collect::<Vec<_>>();
    if scan.source_outgoing_scanned != outgoing.len()
        || expected_pair
            != scan
                .relations
                .iter()
                .map(|row| row.graph_relation_identity)
                .collect::<Vec<_>>()
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
            phase: "directed-pair exhaustive outgoing projection",
        });
    }
    for pair in &scan.relations {
        let beam_row = certificate
            .beam_incident_before
            .relations
            .iter()
            .find(|row| row.graph_relation_identity == pair.graph_relation_identity);
        let stem_row = certificate
            .stem_incident_before
            .relations
            .iter()
            .find(|row| row.graph_relation_identity == pair.graph_relation_identity);
        if beam_row.is_none_or(|row| {
            row.direction != NativeStemsBeamIncidentDirection::Outgoing
                || row.direction_ordinal != pair.source_outgoing_ordinal
                || row.relation_object_identity != pair.relation_object_identity
                || row.relation_class != pair.relation_class
                || row.kind != pair.kind
        }) || stem_row.is_none_or(|row| {
            row.direction != NativeStemsBeamIncidentDirection::Incoming
                || row.relation_object_identity != pair.relation_object_identity
                || row.relation_class != pair.relation_class
                || row.kind != pair.kind
        }) {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "directed-pair endpoint incident join",
            });
        }
    }
    Ok(())
}

fn certificate_endpoint_id(
    certificate: &NativeStemsBeamVLinkBaseApplyCertificate,
    vertex_ordinal: Option<usize>,
    java_inter_id: i32,
    phase: &'static str,
) -> Result<i32, NativeStemsBeamVLinkBaseApplyError> {
    match certificate.endpoint_identity {
        NativeStemsBeamCertificateEndpointIdentity::JavaPersistentInterId => (java_inter_id > 0)
            .then_some(java_inter_id)
            .ok_or(NativeStemsBeamVLinkBaseApplyError::InvalidState { phase }),
        NativeStemsBeamCertificateEndpointIdentity::NativeVertexOneBased => vertex_ordinal
            .and_then(|ordinal| ordinal.checked_add(1))
            .and_then(|identity| i32::try_from(identity).ok())
            .filter(|identity| *identity > 0)
            .ok_or(NativeStemsBeamVLinkBaseApplyError::InvalidState { phase }),
    }
}

fn validate_callback_queries(
    state: &NativeStemsBeamVLinkBaseApplyState,
    certificate: &NativeStemsBeamVLinkBaseApplyCertificate,
    draft: &NativeStemsBeamRelationDraft,
    graph_relation_identity: Option<usize>,
    beam_inter_id: i32,
    stem_inter_id: i32,
) -> Result<(), NativeStemsBeamVLinkBaseApplyError> {
    let stem_before = &certificate.stem_incident_before;
    let stem_after = &certificate.stem_incident_after;
    let beam_before = &certificate.beam_incident_before;
    let beam_after = &certificate.beam_incident_after;
    for (count, len, hash) in [
        (
            stem_before.query_relation_count,
            stem_before.relations.len(),
            &stem_before.query_provenance_sha256,
        ),
        (
            stem_after.query_relation_count,
            stem_after.relations.len(),
            &stem_after.query_provenance_sha256,
        ),
        (
            beam_before.query_relation_count,
            beam_before.relations.len(),
            &beam_before.query_provenance_sha256,
        ),
        (
            beam_after.query_relation_count,
            beam_after.relations.len(),
            &beam_after.query_provenance_sha256,
        ),
    ] {
        if count != len || !valid_sha256(hash) {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "callback query coverage",
            });
        }
    }
    // Source provenance distinguishes raw-beam and hook construction paths,
    // but Java's callback rule follows the live Inter runtime class. A
    // RawBeam source can materialize as a BeamHook (Allegretto plan 15), so
    // validate the hash with the rule projected from the bound SIG vertex.
    let hook = matches!(
        beam_before.rule,
        NativeStemsBeamBeamIncidentRule::HookHasAnyBeamStem
    );
    if graph_relation_identity.is_some() && beam_after.rule != beam_before.rule {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
            phase: "callback beam rule continuity",
        });
    }
    if stem_before.query_provenance_sha256 != stem_incident_query_sha256(&stem_before.relations)
        || stem_after.query_provenance_sha256 != stem_incident_query_sha256(&stem_after.relations)
        || beam_before.query_provenance_sha256
            != beam_incident_query_sha256(&beam_before.relations, hook)
        || beam_after.query_provenance_sha256
            != beam_incident_query_sha256(&beam_after.relations, hook)
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
            phase: "callback query provenance payload",
        });
    }
    if state.sig.stem.sig_vertex_identity.is_none() {
        if stem_before.state != NativeStemsBeamStemIncidentScanState::MissingVertex
            || !stem_before.relations.is_empty()
        {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "ID-zero pre-edge stem query",
            });
        }
    } else {
        if stem_before.state != NativeStemsBeamStemIncidentScanState::ExhaustiveIncomingThenOutgoing
        {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "existing pre-edge stem query",
            });
        }
        validate_stem_incident_rows(
            state,
            certificate,
            beam_inter_id,
            stem_inter_id,
            None,
            false,
        )?;
    }
    let expected_beam_rule = if hook {
        NativeStemsBeamBeamIncidentRule::HookHasAnyBeamStem
    } else {
        NativeStemsBeamBeamIncidentRule::RawBeamLeftAndRight
    };
    if state.sig.beam.sig_vertex_identity.is_none() {
        if beam_before.rule != NativeStemsBeamBeamIncidentRule::NotRead
            || !beam_before.relations.is_empty()
        {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "missing pre-edge beam query",
            });
        }
    } else {
        if beam_before.rule != expected_beam_rule {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "pre-edge beam query rule",
            });
        }
        validate_beam_incident_rows(
            state,
            certificate,
            beam_inter_id,
            stem_inter_id,
            draft,
            None,
            false,
        )?;
    }
    validate_pre_callback_snapshot_consistency(state, certificate, beam_inter_id, stem_inter_id)?;

    let Some(new_graph_identity) = graph_relation_identity else {
        if stem_after.state != NativeStemsBeamStemIncidentScanState::NotRead
            || beam_after.rule != NativeStemsBeamBeamIncidentRule::NotRead
            || !stem_after.relations.is_empty()
            || !beam_after.relations.is_empty()
            || certificate.chord_stem_matches != 0
        {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "suppressed callback not-read state",
            });
        }
        return Ok(());
    };
    if new_graph_identity != state.sig.baseline_relation_count
        || stem_after.state != NativeStemsBeamStemIncidentScanState::ExhaustiveIncomingThenOutgoing
        || beam_after.rule != expected_beam_rule
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
            phase: "added callback scan state",
        });
    }

    validate_stem_incident_rows(
        state,
        certificate,
        beam_inter_id,
        stem_inter_id,
        Some(new_graph_identity),
        true,
    )?;
    validate_beam_incident_rows(
        state,
        certificate,
        beam_inter_id,
        stem_inter_id,
        draft,
        Some(new_graph_identity),
        true,
    )?;
    if state.sig.stem.sig_vertex_identity.is_none() && stem_after.relations.len() != 1 {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
            phase: "ID-zero post-edge stem incidence",
        });
    }
    validate_cross_query_relations(
        state,
        certificate,
        new_graph_identity,
        beam_inter_id,
        stem_inter_id,
    )?;
    Ok(())
}

fn validate_stem_incident_rows(
    state: &NativeStemsBeamVLinkBaseApplyState,
    certificate: &NativeStemsBeamVLinkBaseApplyCertificate,
    beam_inter_id: i32,
    stem_inter_id: i32,
    new_graph_identity: Option<usize>,
    after_callback: bool,
) -> Result<(), NativeStemsBeamVLinkBaseApplyError> {
    let rows = if after_callback {
        &certificate.stem_incident_after.relations
    } else {
        &certificate.stem_incident_before.relations
    };
    validate_incident_ordinals(rows.iter().map(|row| {
        (
            row.incident_ordinal,
            row.direction,
            row.direction_ordinal,
            row.graph_relation_identity,
        )
    }))?;
    let mut new_matches = 0;
    let mut chord_matches = 0;
    for (index, row) in rows.iter().enumerate() {
        let is_new = new_graph_identity == Some(row.graph_relation_identity);
        if is_new {
            new_matches += 1;
        }
        if row.incident_ordinal != index
            || !valid_stem_opposite(
                row.opposite,
                row.opposite_vertex_ordinal,
                row.opposite_inter_id,
                state.sig.beam.sig_vertex_identity,
                beam_inter_id,
                state
                    .sig
                    .stem
                    .sig_vertex_identity
                    .or(Some(state.sig.baseline_vertex_count)),
                stem_inter_id,
            )
            || !relation_class_matches_kind(&row.relation_class, row.kind)
            || row.chord_stem_match != (row.kind == NativeStemsBeamQueryRelationKind::ChordStem)
            || rows[..index].iter().any(|prior| {
                prior.graph_relation_identity == row.graph_relation_identity
                    || prior.relation_object_identity == row.relation_object_identity
            })
            || (!is_new
                && (row.graph_relation_identity >= state.sig.baseline_relation_count
                    || row.opposite_vertex_ordinal >= state.sig.baseline_vertex_count
                    || row.relation_object_identity
                        != NativeStemsBeamRelationObjectIdentity::GraphObject(
                            row.graph_relation_identity,
                        )))
            || (is_new
                && (row.direction != NativeStemsBeamIncidentDirection::Incoming
                    || row.opposite != NativeStemsBeamIncidentOpposite::Beam
                    || state.sig.beam.sig_vertex_identity != Some(row.opposite_vertex_ordinal)
                    || row.opposite_inter_id != beam_inter_id
                    || !relation_is_beam_stem(row.kind)
                    || row.relation_object_identity != certificate.fresh_relation_object_identity))
        {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "stem incident query payload",
            });
        }
        chord_matches += usize::from(row.chord_stem_match);
    }
    if new_matches != usize::from(new_graph_identity.is_some())
        || (after_callback && chord_matches != certificate.chord_stem_matches)
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
            phase: "stem callback new edge/ChordStem count",
        });
    }
    Ok(())
}

fn validate_beam_incident_rows(
    state: &NativeStemsBeamVLinkBaseApplyState,
    certificate: &NativeStemsBeamVLinkBaseApplyCertificate,
    beam_inter_id: i32,
    stem_inter_id: i32,
    draft: &NativeStemsBeamRelationDraft,
    new_graph_identity: Option<usize>,
    after_callback: bool,
) -> Result<(), NativeStemsBeamVLinkBaseApplyError> {
    let rows = if after_callback {
        &certificate.beam_incident_after.relations
    } else {
        &certificate.beam_incident_before.relations
    };
    validate_incident_ordinals(rows.iter().map(|row| {
        (
            row.incident_ordinal,
            row.direction,
            row.direction_ordinal,
            row.graph_relation_identity,
        )
    }))?;
    // The source enum records how the worklist item was discovered, whereas
    // Java dispatches the incident query from the live SIG vertex class. A
    // raw-beam source can therefore carry the hook rule after materializing
    // as a BeamHook (Allegretto plan 15).
    let hook = matches!(
        if after_callback {
            certificate.beam_incident_after.rule
        } else {
            certificate.beam_incident_before.rule
        },
        NativeStemsBeamBeamIncidentRule::HookHasAnyBeamStem
    );
    let effective_stem_vertex = state
        .sig
        .stem
        .sig_vertex_identity
        .or_else(|| new_graph_identity.map(|_| state.sig.baseline_vertex_count));
    let mut hook_found = false;
    let mut new_matches = 0;
    for (index, row) in rows.iter().enumerate() {
        let is_new = new_graph_identity == Some(row.graph_relation_identity);
        if is_new {
            new_matches += 1;
        }
        let examined = !hook || !hook_found;
        let expected_read = if examined {
            NativeStemsBeamBeamIncidentRead::Examined
        } else {
            NativeStemsBeamBeamIncidentRead::UnreadAfterBreak
        };
        let expected_relevant = examined && relation_is_beam_stem(row.kind);
        if hook && expected_relevant {
            hook_found = true;
        }
        // Raw `AbstractBeamInter.checkAbnormal` reads a BeamStem/BeamRest
        // portion only when the live relation actually carries one. A null
        // portion is an examined class-only row and contributes to neither
        // side; it is not a Java invariant failure.
        let raw_relevant = !hook
            && (relation_is_beam_stem(row.kind) || relation_is_beam_rest(row.kind))
            && row.beam_portion.is_some();
        if row.incident_ordinal != index
            || !valid_beam_opposite(
                row.opposite,
                row.opposite_vertex_ordinal,
                row.opposite_inter_id,
                state.sig.beam.sig_vertex_identity,
                beam_inter_id,
                effective_stem_vertex,
                stem_inter_id,
            )
            || !relation_class_matches_kind(&row.relation_class, row.kind)
            || row.read != expected_read
            || row.relevant
                != if hook {
                    expected_relevant
                } else {
                    raw_relevant
                }
            || rows[..index].iter().any(|prior| {
                prior.graph_relation_identity == row.graph_relation_identity
                    || prior.relation_object_identity == row.relation_object_identity
            })
            || (!is_new
                && (row.graph_relation_identity >= state.sig.baseline_relation_count
                    || row.opposite_vertex_ordinal >= state.sig.baseline_vertex_count
                    || row.relation_object_identity
                        != NativeStemsBeamRelationObjectIdentity::GraphObject(
                            row.graph_relation_identity,
                        )))
            || (is_new
                && (row.direction != NativeStemsBeamIncidentDirection::Outgoing
                    || row.opposite != NativeStemsBeamIncidentOpposite::Stem
                    || row.opposite_vertex_ordinal
                        != state
                            .sig
                            .stem
                            .sig_vertex_identity
                            .unwrap_or(state.sig.baseline_vertex_count)
                    || row.opposite_inter_id != stem_inter_id
                    || !relation_is_beam_stem(row.kind)
                    || row.relation_object_identity != certificate.fresh_relation_object_identity))
            // BeamHook.checkAbnormal ignores BeamStemRelation beam portions.
            // Preserve whatever the live graph carries instead of imposing
            // the source-construction convention on this runtime-class query.
            || (!hook && row.relevant && row.beam_portion.is_none())
            || (!hook && !row.relevant && row.beam_portion.is_some())
            || (is_new && !hook && row.beam_portion != Some(draft.beam_portion))
        {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "beam incident query payload",
            });
        }
    }
    if new_matches != usize::from(new_graph_identity.is_some())
        || (after_callback && new_graph_identity.is_some() && hook && !hook_found)
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
            phase: "beam callback new edge/lazy liveness",
        });
    }
    Ok(())
}

fn validate_cross_query_relations(
    state: &NativeStemsBeamVLinkBaseApplyState,
    certificate: &NativeStemsBeamVLinkBaseApplyCertificate,
    new_graph_identity: usize,
    beam_inter_id: i32,
    stem_inter_id: i32,
) -> Result<(), NativeStemsBeamVLinkBaseApplyError> {
    let mut common = Vec::<(
        usize,
        NativeStemsBeamRelationObjectIdentity,
        &str,
        NativeStemsBeamQueryRelationKind,
    )>::new();
    for row in &certificate.directed_pair_scan.relations {
        common.push((
            row.graph_relation_identity,
            row.relation_object_identity,
            &row.relation_class,
            row.kind,
        ));
    }
    for scan in [
        &certificate.stem_incident_before,
        &certificate.stem_incident_after,
    ] {
        for row in &scan.relations {
            common.push((
                row.graph_relation_identity,
                row.relation_object_identity,
                &row.relation_class,
                row.kind,
            ));
        }
    }
    for scan in [
        &certificate.beam_incident_before,
        &certificate.beam_incident_after,
    ] {
        for row in &scan.relations {
            common.push((
                row.graph_relation_identity,
                row.relation_object_identity,
                &row.relation_class,
                row.kind,
            ));
        }
    }
    validate_common_relation_payloads(&common)?;
    validate_incident_endpoint_domains(state, certificate, beam_inter_id, stem_inter_id, true)?;
    validate_selected_endpoint_incidence_join(
        &certificate.stem_incident_after.relations,
        &certificate.beam_incident_after.relations,
    )?;
    validate_stem_phase_consistency(certificate, new_graph_identity)?;
    validate_beam_phase_consistency(certificate, new_graph_identity)?;
    Ok(())
}

fn validate_pre_callback_snapshot_consistency(
    state: &NativeStemsBeamVLinkBaseApplyState,
    certificate: &NativeStemsBeamVLinkBaseApplyCertificate,
    beam_inter_id: i32,
    stem_inter_id: i32,
) -> Result<(), NativeStemsBeamVLinkBaseApplyError> {
    let mut common = Vec::<(
        usize,
        NativeStemsBeamRelationObjectIdentity,
        &str,
        NativeStemsBeamQueryRelationKind,
    )>::new();
    for row in &certificate.directed_pair_scan.relations {
        common.push((
            row.graph_relation_identity,
            row.relation_object_identity,
            &row.relation_class,
            row.kind,
        ));
    }
    for row in &certificate.stem_incident_before.relations {
        common.push((
            row.graph_relation_identity,
            row.relation_object_identity,
            &row.relation_class,
            row.kind,
        ));
    }
    for row in &certificate.beam_incident_before.relations {
        common.push((
            row.graph_relation_identity,
            row.relation_object_identity,
            &row.relation_class,
            row.kind,
        ));
    }
    validate_common_relation_payloads(&common)?;
    validate_incident_endpoint_domains(state, certificate, beam_inter_id, stem_inter_id, false)?;
    validate_selected_endpoint_incidence_join(
        &certificate.stem_incident_before.relations,
        &certificate.beam_incident_before.relations,
    )
}

fn validate_selected_endpoint_incidence_join(
    stem_rows: &[NativeStemsBeamStemIncidentRelation],
    beam_rows: &[NativeStemsBeamBeamIncidentRelation],
) -> Result<(), NativeStemsBeamVLinkBaseApplyError> {
    for stem in stem_rows {
        let matching = beam_rows
            .iter()
            .filter(|beam| beam.graph_relation_identity == stem.graph_relation_identity)
            .collect::<Vec<_>>();
        if matching.is_empty() {
            if stem.opposite == NativeStemsBeamIncidentOpposite::Beam {
                return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                    phase: "selected endpoint incident mirror",
                });
            }
            continue;
        }
        let [beam] = matching.as_slice() else {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "selected endpoint incident mirror",
            });
        };
        if stem.opposite != NativeStemsBeamIncidentOpposite::Beam
            || beam.opposite != NativeStemsBeamIncidentOpposite::Stem
            || stem.direction == beam.direction
        {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "selected endpoint incident direction",
            });
        }
    }
    for beam in beam_rows {
        if beam.opposite == NativeStemsBeamIncidentOpposite::Stem
            && !stem_rows
                .iter()
                .any(|stem| stem.graph_relation_identity == beam.graph_relation_identity)
        {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "selected endpoint incident mirror",
            });
        }
    }
    Ok(())
}

fn validate_common_relation_payloads(
    common: &[(
        usize,
        NativeStemsBeamRelationObjectIdentity,
        &str,
        NativeStemsBeamQueryRelationKind,
    )],
) -> Result<(), NativeStemsBeamVLinkBaseApplyError> {
    for index in 0..common.len() {
        if common[..index].iter().any(|prior| {
            (prior.0 == common[index].0 || prior.1 == common[index].1) && *prior != common[index]
        }) {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "cross-query relation identity/payload",
            });
        }
    }
    Ok(())
}

fn validate_stem_phase_consistency(
    certificate: &NativeStemsBeamVLinkBaseApplyCertificate,
    new_graph_identity: usize,
) -> Result<(), NativeStemsBeamVLinkBaseApplyError> {
    if certificate.stem_incident_after.relations.len()
        != certificate.stem_incident_before.relations.len() + 1
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
            phase: "stem pre/post relation cardinality",
        });
    }
    for before in &certificate.stem_incident_before.relations {
        let matching = certificate
            .stem_incident_after
            .relations
            .iter()
            .filter(|after| after.graph_relation_identity == before.graph_relation_identity)
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "stem pre/post relation retention",
            });
        }
        let after = matching[0];
        if before.relation_object_identity != after.relation_object_identity
            || before.relation_class != after.relation_class
            || before.kind != after.kind
            || before.direction != after.direction
            || before.direction_ordinal != after.direction_ordinal
            || before.opposite_vertex_ordinal != after.opposite_vertex_ordinal
            || before.opposite != after.opposite
            || before.opposite_inter_id != after.opposite_inter_id
            || before.chord_stem_match != after.chord_stem_match
        {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "stem pre/post relation payload",
            });
        }
    }
    if certificate
        .stem_incident_after
        .relations
        .iter()
        .filter(|after| after.graph_relation_identity != new_graph_identity)
        .any(|after| {
            certificate
                .stem_incident_before
                .relations
                .iter()
                .filter(|before| before.graph_relation_identity == after.graph_relation_identity)
                .count()
                != 1
        })
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
            phase: "stem fabricated post-edge relation",
        });
    }
    Ok(())
}

fn validate_incident_endpoint_domains(
    state: &NativeStemsBeamVLinkBaseApplyState,
    certificate: &NativeStemsBeamVLinkBaseApplyCertificate,
    beam_inter_id: i32,
    stem_inter_id: i32,
    include_after: bool,
) -> Result<(), NativeStemsBeamVLinkBaseApplyError> {
    let stem_vertex_ordinal = state
        .sig
        .stem
        .sig_vertex_identity
        .or_else(|| include_after.then_some(state.sig.baseline_vertex_count));
    let mut endpoints = Vec::<(usize, i32, NativeStemsBeamIncidentOpposite)>::new();
    if let Some(beam_vertex_ordinal) = state.sig.beam.sig_vertex_identity {
        endpoints.push((
            beam_vertex_ordinal,
            beam_inter_id,
            NativeStemsBeamIncidentOpposite::Beam,
        ));
    }
    if let Some(stem_vertex_ordinal) = stem_vertex_ordinal {
        endpoints.push((
            stem_vertex_ordinal,
            stem_inter_id,
            NativeStemsBeamIncidentOpposite::Stem,
        ));
    }
    let stem_scans = if include_after {
        vec![
            &certificate.stem_incident_before,
            &certificate.stem_incident_after,
        ]
    } else {
        vec![&certificate.stem_incident_before]
    };
    for scan in stem_scans {
        endpoints.extend(scan.relations.iter().map(|row| {
            (
                row.opposite_vertex_ordinal,
                row.opposite_inter_id,
                row.opposite,
            )
        }));
    }
    let beam_scans = if include_after {
        vec![
            &certificate.beam_incident_before,
            &certificate.beam_incident_after,
        ]
    } else {
        vec![&certificate.beam_incident_before]
    };
    for scan in beam_scans {
        endpoints.extend(scan.relations.iter().map(|row| {
            (
                row.opposite_vertex_ordinal,
                row.opposite_inter_id,
                row.opposite,
            )
        }));
    }
    for index in 0..endpoints.len() {
        if endpoints[..index].iter().any(|prior| {
            let same_vertex = prior.0 == endpoints[index].0;
            let same_inter_id = prior.1 == endpoints[index].1;
            same_vertex != same_inter_id
                || ((same_vertex || same_inter_id) && prior.2 != endpoints[index].2)
        }) {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "incident endpoint identity domain",
            });
        }
    }
    Ok(())
}

fn validate_beam_phase_consistency(
    certificate: &NativeStemsBeamVLinkBaseApplyCertificate,
    new_graph_identity: usize,
) -> Result<(), NativeStemsBeamVLinkBaseApplyError> {
    if certificate.beam_incident_after.relations.len()
        != certificate.beam_incident_before.relations.len() + 1
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
            phase: "beam pre/post relation cardinality",
        });
    }
    for before in &certificate.beam_incident_before.relations {
        let matching = certificate
            .beam_incident_after
            .relations
            .iter()
            .filter(|after| after.graph_relation_identity == before.graph_relation_identity)
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "beam pre/post relation retention",
            });
        }
        let after = matching[0];
        if before.relation_object_identity != after.relation_object_identity
            || before.relation_class != after.relation_class
            || before.kind != after.kind
            || before.direction != after.direction
            || before.direction_ordinal != after.direction_ordinal
            || before.opposite_vertex_ordinal != after.opposite_vertex_ordinal
            || before.opposite != after.opposite
            || before.opposite_inter_id != after.opposite_inter_id
            || before.read != after.read
            || before.relevant != after.relevant
            || before.beam_portion != after.beam_portion
        {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "beam pre/post relation payload",
            });
        }
    }
    if certificate
        .beam_incident_after
        .relations
        .iter()
        .filter(|after| after.graph_relation_identity != new_graph_identity)
        .any(|after| {
            certificate
                .beam_incident_before
                .relations
                .iter()
                .filter(|before| before.graph_relation_identity == after.graph_relation_identity)
                .count()
                != 1
        })
    {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
            phase: "beam fabricated post-edge relation",
        });
    }
    Ok(())
}

fn validate_incident_ordinals(
    rows: impl Iterator<Item = (usize, NativeStemsBeamIncidentDirection, usize, usize)>,
) -> Result<(), NativeStemsBeamVLinkBaseApplyError> {
    let mut expected_incoming = 0;
    let mut expected_outgoing = 0;
    let mut outgoing_seen = false;
    let mut last_incoming = None;
    let mut last_outgoing = None;
    for (expected_incident, (incident, direction, direction_ordinal, graph_identity)) in
        rows.enumerate()
    {
        let valid = match direction {
            NativeStemsBeamIncidentDirection::Incoming => {
                let ordered = !outgoing_seen
                    && direction_ordinal == expected_incoming
                    && last_incoming.is_none_or(|prior| prior < graph_identity);
                expected_incoming += 1;
                last_incoming = Some(graph_identity);
                ordered
            }
            NativeStemsBeamIncidentDirection::Outgoing => {
                outgoing_seen = true;
                let ordered = direction_ordinal == expected_outgoing
                    && last_outgoing.is_none_or(|prior| prior < graph_identity);
                expected_outgoing += 1;
                last_outgoing = Some(graph_identity);
                ordered
            }
        };
        if incident != expected_incident || !valid {
            return Err(NativeStemsBeamVLinkBaseApplyError::InvalidEvidence {
                phase: "incident query source order",
            });
        }
    }
    Ok(())
}

fn relation_is_beam_stem(kind: NativeStemsBeamQueryRelationKind) -> bool {
    kind == NativeStemsBeamQueryRelationKind::BeamStem
}

fn relation_is_beam_rest(kind: NativeStemsBeamQueryRelationKind) -> bool {
    kind == NativeStemsBeamQueryRelationKind::BeamRest
}

fn relation_class_matches_kind(class: &str, kind: NativeStemsBeamQueryRelationKind) -> bool {
    let known = [
        "org.audiveris.omr.sig.relation.BeamStemRelation",
        "org.audiveris.omr.sig.relation.BeamRestRelation",
        "org.audiveris.omr.sig.relation.ChordStemRelation",
    ];
    match kind {
        NativeStemsBeamQueryRelationKind::BeamStem => class == known[0],
        NativeStemsBeamQueryRelationKind::BeamRest => class == known[1],
        NativeStemsBeamQueryRelationKind::ChordStem => class == known[2],
        NativeStemsBeamQueryRelationKind::Other => !class.is_empty() && !known.contains(&class),
    }
}

fn valid_stem_opposite(
    opposite: NativeStemsBeamIncidentOpposite,
    vertex_ordinal: usize,
    inter_id: i32,
    beam_vertex_ordinal: Option<usize>,
    beam_inter_id: i32,
    stem_vertex_ordinal: Option<usize>,
    stem_inter_id: i32,
) -> bool {
    inter_id > 0
        && match opposite {
            NativeStemsBeamIncidentOpposite::Beam => {
                beam_vertex_ordinal == Some(vertex_ordinal) && inter_id == beam_inter_id
            }
            NativeStemsBeamIncidentOpposite::Stem => false,
            NativeStemsBeamIncidentOpposite::OtherInter => {
                inter_id != beam_inter_id
                    && inter_id != stem_inter_id
                    && beam_vertex_ordinal != Some(vertex_ordinal)
                    && stem_vertex_ordinal != Some(vertex_ordinal)
            }
        }
}

fn valid_beam_opposite(
    opposite: NativeStemsBeamIncidentOpposite,
    vertex_ordinal: usize,
    inter_id: i32,
    beam_vertex_ordinal: Option<usize>,
    beam_inter_id: i32,
    stem_vertex_ordinal: Option<usize>,
    stem_inter_id: i32,
) -> bool {
    inter_id > 0
        && match opposite {
            NativeStemsBeamIncidentOpposite::Beam => false,
            NativeStemsBeamIncidentOpposite::Stem => {
                stem_vertex_ordinal == Some(vertex_ordinal) && inter_id == stem_inter_id
            }
            NativeStemsBeamIncidentOpposite::OtherInter => {
                inter_id != beam_inter_id
                    && inter_id != stem_inter_id
                    && beam_vertex_ordinal != Some(vertex_ordinal)
                    && stem_vertex_ordinal != Some(vertex_ordinal)
            }
        }
}

fn commit(
    prepared: PreparedCommit,
    state: &mut NativeStemsBeamVLinkBaseApplyState,
) -> NativeStemsBeamVLinkBaseApplyTransaction {
    let sheet_edit_before = state.sheet_edit;
    let beam_abnormal_before = state.sig.beam.abnormal;
    let stem_abnormal_before = state.sig.stem.abnormal;
    let mut operations = Vec::new();
    let vertex_action = if let (Some(inter_id), Some(sig_vertex_identity)) =
        (prepared.vertex_inter_id, prepared.vertex_identity)
    {
        let ids = &mut state.transaction_state.glyph_index.persistent_ids;
        let before = ids.sheet_last_id;
        ids.sheet_last_id = inter_id;
        ids.glyph_index_last_id = inter_id;
        ids.inter_index_last_id = inter_id;
        operations.push(
            NativeStemsBeamVLinkBaseApplyOperation::SharedPersistentIdAdvanced {
                before,
                after: inter_id,
            },
        );
        update_transaction_stem(
            &mut state.transaction_state,
            prepared.stem_before.stem_identity,
            |stem| stem.inter_id = Some(inter_id),
        );
        operations.push(
            NativeStemsBeamVLinkBaseApplyOperation::StemInterIdAssigned {
                stem_identity: prepared.stem_before.stem_identity,
                inter_id,
            },
        );
        state
            .inter_index
            .appended_entries
            .push(NativeStemsBeamInterIndexAppend {
                index_ordinal: state.inter_index.baseline_entry_count,
                stem_identity: prepared.stem_before.stem_identity,
                inter_id,
                vip: state.sig.stem.vip,
            });
        state.inter_index.stem_lookup = NativeStemsBeamInterIndexLookup::PresentSameObject {
            index_ordinal: state.inter_index.baseline_entry_count,
            inter_id,
            vip: state.sig.stem.vip,
            object_matches: 1,
            inter_id_matches: 1,
            glyph_active_matches: 0,
            glyph_original_matches: 0,
        };
        state.inter_index.next_id_lookup =
            NativeStemsBeamNextPersistentIdLookup::OccupiedByAppendedStem {
                persistent_id: inter_id,
                stem_identity: prepared.stem_before.stem_identity,
            };
        state.sig.stem.inter_indexed = true;
        operations.push(NativeStemsBeamVLinkBaseApplyOperation::InterIndexInserted {
            stem_identity: prepared.stem_before.stem_identity,
            inter_id,
        });
        state
            .sig
            .appended_vertices
            .push(NativeStemsBeamSigVertexAppend {
                vertex_ordinal: state.sig.baseline_vertex_count,
                sig_vertex_identity,
                stem_identity: prepared.stem_before.stem_identity,
                inter_id,
            });
        state.sig.stem_vertex = NativeStemsBeamSigVertexLookup::PresentSameObject {
            vertex_ordinal: state.sig.baseline_vertex_count,
            sig_vertex_identity,
            inter_id,
            object_matches: 1,
        };
        state.sig.stem.sig_vertex_identity = Some(sig_vertex_identity);
        operations.push(NativeStemsBeamVLinkBaseApplyOperation::SigVertexInserted {
            sig_vertex_identity,
        });
        operations.push(NativeStemsBeamVLinkBaseApplyOperation::SigVertexEventDispatched);
        operations.push(
            NativeStemsBeamVLinkBaseApplyOperation::StandardSigListenerVertexCallbackCompleted,
        );
        state.sig.stem.sig_system_id = Some(prepared.key.system_id);
        update_transaction_stem(
            &mut state.transaction_state,
            prepared.stem_before.stem_identity,
            |stem| stem.sig_attached = true,
        );
        operations.push(NativeStemsBeamVLinkBaseApplyOperation::StemSigAttached {
            system_id: prepared.key.system_id,
        });
        operations.push(NativeStemsBeamVLinkBaseApplyOperation::StemAddedCallbackStarted);
        let removed_before = state.sig.stem.removed;
        state.sig.stem.removed = false;
        operations.push(NativeStemsBeamVLinkBaseApplyOperation::StemRemovedCleared {
            before: removed_before,
        });
        state.sig.stem.abnormal = true;
        update_transaction_stem(
            &mut state.transaction_state,
            prepared.stem_before.stem_identity,
            |stem| stem.abnormal = true,
        );
        operations.push(NativeStemsBeamVLinkBaseApplyOperation::StemAbnormalSet {
            before: stem_abnormal_before,
            after: true,
        });
        if !stem_abnormal_before {
            dirty_cascade(state, &mut operations);
        }
        operations.push(NativeStemsBeamVLinkBaseApplyOperation::StemAddedCallbackCompleted);
        NativeStemsBeamVLinkVertexAction::RegisteredAndAdded {
            inter_id,
            sig_vertex_identity,
        }
    } else {
        NativeStemsBeamVLinkVertexAction::SkippedPositiveInterId
    };

    if let Some(relation_identity) = prepared.graph_relation_identity {
        let stem_vertex = state
            .sig
            .stem
            .sig_vertex_identity
            .expect("prepared added relation has a stem vertex");
        let relation = NativeStemsBeamSigRelationState {
            graph_relation_identity: relation_identity,
            relation_object_identity: prepared.certificate.fresh_relation_object_identity,
            source_vertex_identity: state
                .sig
                .beam
                .sig_vertex_identity
                .expect("prepared added relation has a beam vertex"),
            target_vertex_identity: stem_vertex,
            kind: NativeStemsBeamSigRelationKind::BeamStem {
                beam_portion: Some(prepared.relation.beam_portion),
            },
        };
        state.sig.appended_relations.push(relation);
        operations.push(
            NativeStemsBeamVLinkBaseApplyOperation::SigGlobalRelationInserted {
                graph_relation_identity: relation_identity,
            },
        );
        operations.push(
            NativeStemsBeamVLinkBaseApplyOperation::BeamOutgoingRelationInserted {
                graph_relation_identity: relation_identity,
            },
        );
        operations.push(
            NativeStemsBeamVLinkBaseApplyOperation::StemIncomingRelationInserted {
                graph_relation_identity: relation_identity,
            },
        );
        operations.push(
            NativeStemsBeamVLinkBaseApplyOperation::SigEdgeEventDispatched {
                graph_relation_identity: relation_identity,
            },
        );
        operations
            .push(NativeStemsBeamVLinkBaseApplyOperation::StandardSigListenerEdgeCallbackStarted);
        operations.push(NativeStemsBeamVLinkBaseApplyOperation::BeamStemRelationCallbackStarted);
        operations.push(
            NativeStemsBeamVLinkBaseApplyOperation::StemChordIncidentScanCompleted {
                incident_relation_count: prepared.post_stem_incident.len(),
                chord_stem_matches: 0,
            },
        );
        let after = prepared
            .beam_abnormal_after
            .expect("added relation prepared callback abnormal result");
        if beam_abnormal_before != after {
            state.sig.beam.abnormal = after;
            operations.push(NativeStemsBeamVLinkBaseApplyOperation::BeamAbnormalSet {
                before: beam_abnormal_before,
                after,
            });
            dirty_cascade(state, &mut operations);
        }
        operations.push(NativeStemsBeamVLinkBaseApplyOperation::BeamStemRelationCallbackCompleted);
        operations
            .push(NativeStemsBeamVLinkBaseApplyOperation::StandardSigListenerEdgeCallbackCompleted);
    }
    state.committed = Some(prepared.key);
    state.certificate = None;

    let stem_after = current_stem_payload(state, &prepared.stem_before);
    let apply_returned = prepared.graph_relation_identity.is_some();
    let beam_abnormal = if prepared.graph_relation_identity.is_none() {
        NativeStemsBeamVLinkBeamAbnormalTrace::NotReadSuppressed
    } else if matches!(prepared.relation.beam, NativeStemsBeamSource::Hook(_)) {
        NativeStemsBeamVLinkBeamAbnormalTrace::HookAnyBeamStem {
            incident_relation_count: prepared.post_beam_incident.len(),
            relations_read: prepared.beam_abnormal_read_count,
            before: beam_abnormal_before,
            after: prepared.beam_abnormal_after.expect("added hook result"),
        }
    } else {
        NativeStemsBeamVLinkBeamAbnormalTrace::RawBeamSides {
            incident_relation_count: prepared.post_beam_incident.len(),
            left_found: prepared.raw_left_found,
            right_found: prepared.raw_right_found,
            before: beam_abnormal_before,
            after: prepared.beam_abnormal_after.expect("added raw result"),
        }
    };
    let stem_incident_graph_relation_identities = if apply_returned {
        prepared.post_stem_incident.clone()
    } else {
        Vec::new()
    };
    let callback = NativeStemsBeamVLinkBaseCallbackTrace {
        called: apply_returned,
        extension_preserved: apply_returned,
        beam_portion_preserved: apply_returned,
        stem_incident_graph_relation_identities,
        chord_stem_matches: 0,
        chord_cache_invalidation_count: 0,
        beam_abnormal,
    };
    let continuation_support_grade = prepared.relation.grade;
    NativeStemsBeamVLinkBaseApplyTransaction {
        key: prepared.key,
        stem_before: prepared.stem_before,
        stem_after,
        fresh_relation_object_identity: prepared.certificate.fresh_relation_object_identity,
        fresh_relation: prepared.relation,
        continuation_support_grade,
        graph_relation_identity: prepared.graph_relation_identity,
        vertex_action,
        apply_disposition: prepared.apply_disposition,
        apply_returned,
        removed_reads: NativeStemsBeamVLinkRemovedReadTrace {
            source_removed: prepared.source_removed,
            target_removed: prepared.target_removed_read,
            directed_pair_relations_read: prepared.pair_relations_read,
        },
        directed_pair_graph_relation_identities: prepared
            .certificate
            .directed_pair_scan
            .relations
            .iter()
            .map(|row| row.graph_relation_identity)
            .collect(),
        callback,
        consumed_certificate: prepared.certificate.clone(),
        operations,
        sheet_edit_before,
        sheet_edit_after: state.sheet_edit,
        persistent_id_mutation_count: usize::from(prepared.vertex_inter_id.is_some()),
        inter_index_mutation_count: usize::from(prepared.vertex_inter_id.is_some()),
        sig_vertex_mutation_count: usize::from(prepared.vertex_inter_id.is_some()),
        sig_relation_mutation_count: usize::from(apply_returned),
        stem_abnormal_mutation_count: usize::from(
            prepared.vertex_inter_id.is_some() && !stem_abnormal_before,
        ),
        beam_abnormal_mutation_count: usize::from(
            apply_returned
                && prepared
                    .beam_abnormal_after
                    .is_some_and(|after| after != beam_abnormal_before),
        ),
        beam_group_mutation_count: 0,
        linker_flag_mutation_count: 0,
        sibling_link_mutation_count: 0,
        head_link_mutation_count: 0,
        outcome: NativeStemsBeamVLinkBaseApplyOutcome::ReadyBeforeBLinkerFlagMutation {
            apply_returned,
            continuation_support_grade,
        },
        state_after: Box::new(state.clone()),
    }
}

fn dirty_cascade(
    state: &mut NativeStemsBeamVLinkBaseApplyState,
    operations: &mut Vec<NativeStemsBeamVLinkBaseApplyOperation>,
) {
    state.sheet_edit.stub_modified = true;
    operations.push(NativeStemsBeamVLinkBaseApplyOperation::SheetStubModifiedSetTrue);
    state.sheet_edit.book_modified = true;
    operations.push(NativeStemsBeamVLinkBaseApplyOperation::BookModifiedSetTrue);
    state.sheet_edit.book_dirty = true;
    operations.push(NativeStemsBeamVLinkBaseApplyOperation::BookDirtySetTrue);
}

fn update_transaction_stem(
    state: &mut NativeStemsBeamVLinkTransactionState,
    stem_identity: usize,
    update: impl FnOnce(&mut NativeStemsBeamKnownSystemStem),
) {
    let stem = state
        .system_stems
        .known_stems
        .iter_mut()
        .find(|stem| stem.stem_identity == stem_identity)
        .expect("ID-zero final stem alias was validated before commit");
    update(stem);
}

fn current_stem_payload(
    state: &NativeStemsBeamVLinkBaseApplyState,
    before: &NativeStemsBeamKnownSystemStem,
) -> NativeStemsBeamKnownSystemStem {
    state
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == before.stem_identity)
        .cloned()
        .unwrap_or_else(|| {
            let mut stem = before.clone();
            stem.abnormal = state.sig.stem.abnormal;
            stem
        })
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn native_relation_class(kind: NativeSigRelationKind) -> &'static str {
    match kind {
        NativeSigRelationKind::NoExclusion => "org.audiveris.omr.sig.relation.NoExclusion",
        NativeSigRelationKind::BarConnection => {
            "org.audiveris.omr.sig.relation.BarConnectionRelation"
        }
        NativeSigRelationKind::BarGroup => "org.audiveris.omr.sig.relation.BarGroupRelation",
        NativeSigRelationKind::KeyAlters => "org.audiveris.omr.sig.relation.KeyAltersRelation",
        NativeSigRelationKind::Containment => "org.audiveris.omr.sig.relation.Containment",
        NativeSigRelationKind::ClefKey => "org.audiveris.omr.sig.relation.ClefKeyRelation",
        NativeSigRelationKind::Exclusion => "org.audiveris.omr.sig.relation.Exclusion",
        NativeSigRelationKind::BeamBeam => "org.audiveris.omr.sig.relation.BeamBeamRelation",
        NativeSigRelationKind::BeamStem => "org.audiveris.omr.sig.relation.BeamStemRelation",
        NativeSigRelationKind::BeamRest => "org.audiveris.omr.sig.relation.BeamRestRelation",
        NativeSigRelationKind::HeadStem => "org.audiveris.omr.sig.relation.HeadStemRelation",
        NativeSigRelationKind::ChordStem => "org.audiveris.omr.sig.relation.ChordStemRelation",
    }
}

fn native_query_kind(kind: NativeSigRelationKind) -> NativeStemsBeamQueryRelationKind {
    match kind {
        NativeSigRelationKind::BeamStem => NativeStemsBeamQueryRelationKind::BeamStem,
        NativeSigRelationKind::BeamRest => NativeStemsBeamQueryRelationKind::BeamRest,
        NativeSigRelationKind::ChordStem => NativeStemsBeamQueryRelationKind::ChordStem,
        _ => NativeStemsBeamQueryRelationKind::Other,
    }
}

fn native_inter_id(vertex: NativeSigVertexId) -> Result<i32, NativeStemsBeamVLinkBaseApplyError> {
    i32::try_from(vertex.0.checked_add(1).ok_or(
        NativeStemsBeamVLinkBaseApplyError::UnsupportedV1 {
            phase: "native SIG Inter identity overflow",
        },
    )?)
    .map_err(|_| NativeStemsBeamVLinkBaseApplyError::UnsupportedV1 {
        phase: "native SIG Inter identity overflow",
    })
}

fn native_edge_direction(
    edge: &NativeSigEdge,
    queried: NativeSigVertexId,
) -> (NativeStemsBeamIncidentDirection, NativeSigVertexId) {
    if edge.target == queried.0 {
        (
            NativeStemsBeamIncidentDirection::Incoming,
            NativeSigVertexId(edge.source),
        )
    } else {
        (
            NativeStemsBeamIncidentDirection::Outgoing,
            NativeSigVertexId(edge.target),
        )
    }
}

fn project_native_stem_incident(
    sig: &NativeSigSystem,
    stem: NativeSigVertexId,
    new_edge: Option<usize>,
    beam: NativeSigVertexId,
    fresh: NativeStemsBeamRelationObjectIdentity,
) -> Result<NativeStemsBeamStemIncidentScan, NativeStemsBeamVLinkBaseApplyError> {
    let mut relations = Vec::new();
    let mut incoming = 0;
    let mut outgoing = 0;
    if stem.0 < sig.vertices.len() {
        for edge in sig.incident_edges(stem.0).map_err(|_| {
            NativeStemsBeamVLinkBaseApplyError::InvalidState {
                phase: "native SIG stem incident query",
            }
        })? {
            let (direction, opposite_vertex) = native_edge_direction(edge, stem);
            let direction_ordinal = match direction {
                NativeStemsBeamIncidentDirection::Incoming => {
                    let ordinal = incoming;
                    incoming += 1;
                    ordinal
                }
                NativeStemsBeamIncidentDirection::Outgoing => {
                    let ordinal = outgoing;
                    outgoing += 1;
                    ordinal
                }
            };
            sig.vertex(opposite_vertex.0).ok_or(
                NativeStemsBeamVLinkBaseApplyError::InvalidState {
                    phase: "native SIG stem opposite vertex",
                },
            )?;
            let kind = native_query_kind(edge.kind);
            relations.push(NativeStemsBeamStemIncidentRelation {
                incident_ordinal: relations.len(),
                direction,
                direction_ordinal,
                graph_relation_identity: edge.ordinal,
                relation_object_identity: NativeStemsBeamRelationObjectIdentity::GraphObject(
                    edge.ordinal,
                ),
                relation_class: native_relation_class(edge.kind).to_owned(),
                kind,
                opposite_vertex_ordinal: opposite_vertex.0,
                opposite: if opposite_vertex == beam {
                    NativeStemsBeamIncidentOpposite::Beam
                } else {
                    NativeStemsBeamIncidentOpposite::OtherInter
                },
                opposite_inter_id: native_inter_id(opposite_vertex)?,
                chord_stem_match: kind == NativeStemsBeamQueryRelationKind::ChordStem,
            });
        }
    } else if new_edge.is_none() || stem.0 != sig.vertices.len() {
        return Err(NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native SIG virtual stem identity",
        });
    }
    if let Some(edge) = new_edge {
        relations.push(NativeStemsBeamStemIncidentRelation {
            incident_ordinal: relations.len(),
            direction: NativeStemsBeamIncidentDirection::Incoming,
            direction_ordinal: incoming,
            graph_relation_identity: edge,
            relation_object_identity: fresh,
            relation_class: native_relation_class(NativeSigRelationKind::BeamStem).to_owned(),
            kind: NativeStemsBeamQueryRelationKind::BeamStem,
            opposite_vertex_ordinal: beam.0,
            opposite: NativeStemsBeamIncidentOpposite::Beam,
            opposite_inter_id: native_inter_id(beam)?,
            chord_stem_match: false,
        });
    }
    Ok(NativeStemsBeamStemIncidentScan {
        state: NativeStemsBeamStemIncidentScanState::ExhaustiveIncomingThenOutgoing,
        query_relation_count: relations.len(),
        query_provenance_sha256: stem_incident_query_sha256(&relations),
        relations,
    })
}

fn project_native_beam_incident(
    sig: &NativeSigSystem,
    beam: NativeSigVertexId,
    stem: Option<NativeSigVertexId>,
    hook: bool,
    new_edge: Option<(
        usize,
        NativeSigVertexId,
        NativeStemsBeamRelationObjectIdentity,
    )>,
    draft: &NativeStemsBeamRelationDraft,
    plan_ordinal: usize,
) -> Result<NativeStemsBeamBeamIncidentScan, NativeStemsBeamVLinkBaseApplyError> {
    let mut source = sig
        .incident_edges(beam.0)
        .map_err(|_| NativeStemsBeamVLinkBaseApplyError::InvalidState {
            phase: "native SIG beam incident query",
        })?
        .into_iter()
        .map(|edge| (edge, None))
        .collect::<Vec<_>>();
    let virtual_edge = new_edge.map(|(ordinal, stem, identity)| {
        (
            NativeSigEdge {
                ordinal,
                active: true,
                source: beam.0,
                target: stem.0,
                kind: NativeSigRelationKind::BeamStem,
                origin: crate::native_sig::NativeSigRelationOrigin::BeamVBaseDraft { plan_ordinal },
                support: None,
                beam_portion: Some(draft.beam_portion),
                stem_extension: Some(draft.extension_point),
                head_stem: None,
            },
            identity,
        )
    });
    if let Some((edge, _)) = virtual_edge.as_ref() {
        source.push((edge, Some(edge.ordinal)));
    }
    let mut incoming = 0;
    let mut outgoing = 0;
    let mut hook_found = false;
    let mut relations = Vec::new();
    for (edge, virtual_ordinal) in source {
        let (direction, opposite_vertex) = native_edge_direction(edge, beam);
        let direction_ordinal = match direction {
            NativeStemsBeamIncidentDirection::Incoming => {
                let ordinal = incoming;
                incoming += 1;
                ordinal
            }
            NativeStemsBeamIncidentDirection::Outgoing => {
                let ordinal = outgoing;
                outgoing += 1;
                ordinal
            }
        };
        if opposite_vertex.0 != sig.vertices.len() {
            sig.vertex(opposite_vertex.0).ok_or(
                NativeStemsBeamVLinkBaseApplyError::InvalidState {
                    phase: "native SIG beam opposite vertex",
                },
            )?;
        }
        let kind = native_query_kind(edge.kind);
        let examined = !hook || !hook_found;
        let relevant = examined
            && if hook {
                kind == NativeStemsBeamQueryRelationKind::BeamStem
            } else {
                matches!(
                    kind,
                    NativeStemsBeamQueryRelationKind::BeamStem
                        | NativeStemsBeamQueryRelationKind::BeamRest
                ) && edge.beam_portion.is_some()
            };
        if hook && relevant {
            hook_found = true;
        }
        relations.push(NativeStemsBeamBeamIncidentRelation {
            incident_ordinal: relations.len(),
            direction,
            direction_ordinal,
            graph_relation_identity: edge.ordinal,
            relation_object_identity: virtual_ordinal.map_or(
                NativeStemsBeamRelationObjectIdentity::GraphObject(edge.ordinal),
                |_| virtual_edge.expect("virtual edge is retained").1,
            ),
            relation_class: native_relation_class(edge.kind).to_owned(),
            kind,
            opposite_vertex_ordinal: opposite_vertex.0,
            opposite: if Some(opposite_vertex) == stem {
                NativeStemsBeamIncidentOpposite::Stem
            } else {
                NativeStemsBeamIncidentOpposite::OtherInter
            },
            opposite_inter_id: native_inter_id(opposite_vertex)?,
            read: if examined {
                NativeStemsBeamBeamIncidentRead::Examined
            } else {
                NativeStemsBeamBeamIncidentRead::UnreadAfterBreak
            },
            relevant,
            beam_portion: edge.beam_portion,
        });
    }
    Ok(NativeStemsBeamBeamIncidentScan {
        rule: if hook {
            NativeStemsBeamBeamIncidentRule::HookHasAnyBeamStem
        } else {
            NativeStemsBeamBeamIncidentRule::RawBeamLeftAndRight
        },
        query_relation_count: relations.len(),
        query_provenance_sha256: beam_incident_query_sha256(&relations, hook),
        relations,
    })
}

fn query_rows_sha256(rows: impl IntoIterator<Item = String>) -> String {
    let mut bytes = Vec::new();
    for row in rows {
        bytes.extend_from_slice(row.as_bytes());
        bytes.push(b'\n');
    }
    sha256_hex(&bytes)
}

fn graph_relation_alias(identity: usize) -> String {
    format!("sig-edge:{identity}")
}

fn incident_direction_token(direction: NativeStemsBeamIncidentDirection) -> &'static str {
    match direction {
        NativeStemsBeamIncidentDirection::Incoming => "Incoming",
        NativeStemsBeamIncidentDirection::Outgoing => "Outgoing",
    }
}

fn beam_portion_token(portion: Option<NativeBeamPortion>) -> &'static str {
    match portion {
        None => "-",
        Some(NativeBeamPortion::Left) => "LEFT",
        Some(NativeBeamPortion::Center) => "CENTER",
        Some(NativeBeamPortion::Right) => "RIGHT",
    }
}

fn stem_incident_query_sha256(rows: &[NativeStemsBeamStemIncidentRelation]) -> String {
    query_rows_sha256(rows.iter().map(|row| {
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}",
            row.incident_ordinal,
            incident_direction_token(row.direction),
            row.direction_ordinal,
            graph_relation_alias(row.graph_relation_identity),
            row.relation_class,
            row.opposite_inter_id,
            row.opposite_vertex_ordinal,
            row.chord_stem_match,
        )
    }))
}

fn beam_incident_query_sha256(rows: &[NativeStemsBeamBeamIncidentRelation], hook: bool) -> String {
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
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            row.incident_ordinal,
            incident_direction_token(row.direction),
            row.direction_ordinal,
            graph_relation_alias(row.graph_relation_identity),
            row.relation_class,
            read_state,
            row.opposite_inter_id,
            row.opposite_vertex_ordinal,
            row.relevant,
            beam_portion_token(row.beam_portion),
            contribution,
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

#[cfg(test)]
mod tests {
    use super::*;
    use audiveris_image::{
        run_table::{FOREGROUND, Orientation, RunTable},
        section::Bounds,
    };

    use crate::{
        head_scanner_slices::JavaRectangle,
        native_stems_beam_vlink_transaction::{
            NativeStemsBeamFixedGlyphContent, NativeStemsBeamGlyphAliasOrder,
            NativeStemsBeamGlyphIndexTransactionState, NativeStemsBeamPersistentIdState,
            NativeStemsBeamRegistryAuthority, NativeStemsBeamStemGrade,
            NativeStemsBeamSystemStemTransactionState, NativeStemsBeamVLinkTransactionScope,
        },
        stems_step::{NativeStemLine, NativeStemPoint},
    };

    const SYSTEM_ID: usize = 1;
    const PLAN_ORDINAL: usize = 7;
    const BEAM_INTER_ID: i32 = 5;
    const EXISTING_STEM_INTER_ID: i32 = 6;
    const GENERATED_STEM_INTER_ID: i32 = 11;
    const BEAM_VERTEX: usize = 0;
    const EXISTING_STEM_VERTEX: usize = 1;
    const BASELINE_VERTEX_COUNT: usize = 3;
    const NEW_GRAPH_RELATION: usize = 2;

    fn hash() -> String {
        "0".repeat(64)
    }

    fn plan() -> NativeStemsBeamPlanRef {
        NativeStemsBeamPlanRef {
            system_id: SYSTEM_ID,
            plan_ordinal: PLAN_ORDINAL,
            builder_ordinal: 0,
            stem_profile: 4,
        }
    }

    fn glyph_content() -> NativeStemsBeamFixedGlyphContent {
        let pixels = vec![FOREGROUND; 4];
        let run_table = RunTable::from_pixels(Orientation::Vertical, 1, 4, &pixels)
            .expect("valid test run table");
        NativeStemsBeamFixedGlyphContent {
            bounds: Bounds {
                x: 10,
                y: 10,
                width: 1,
                height: 4,
            },
            weight: run_table.weight(),
            run_table,
        }
    }

    fn known_stem(inter_id: Option<i32>) -> NativeStemsBeamKnownSystemStem {
        NativeStemsBeamKnownSystemStem {
            stem_identity: 0,
            glyph_id: 1,
            glyph_content: glyph_content(),
            inter_id,
            grade: NativeStemsBeamStemGrade::Artificial(0.5),
            geometry:
                crate::native_stems_beam_vlink_transaction::NativeStemsBeamCreatedStemGeometry {
                    median: NativeStemLine {
                        start: NativeStemPoint { x: 10.0, y: 10.0 },
                        stop: NativeStemPoint { x: 10.0, y: 30.0 },
                    },
                    mean_thickness: 1.0,
                    ribbon_bounds: JavaRectangle::new(9, 10, 2, 21),
                },
            sig_attached: inter_id.is_some(),
            abnormal: false,
        }
    }

    fn relation(
        source: NativeStemsBeamSource,
        inter_id: Option<i32>,
    ) -> NativeStemsBeamRelationDraft {
        NativeStemsBeamRelationDraft {
            beam: source,
            partner_stem_identity: 0,
            // This is deliberately the pre-apply boundary-13 snapshot.
            partner_inter_id: inter_id,
            beam_portion: NativeBeamPortion::Left,
            dx: 0.0,
            dy: 0.0,
            x_impact: 1.0,
            y_impact: 1.0,
            grade: 0.5,
            extension_point: NativeStemPoint { x: 10.0, y: 10.0 },
            outgoing: true,
        }
    }

    fn incident_rule(source: NativeStemsBeamSource) -> NativeStemsBeamBeamIncidentRule {
        if matches!(source, NativeStemsBeamSource::Hook(_)) {
            NativeStemsBeamBeamIncidentRule::HookHasAnyBeamStem
        } else {
            NativeStemsBeamBeamIncidentRule::RawBeamLeftAndRight
        }
    }

    fn certificate(
        source: NativeStemsBeamSource,
        new_stem: bool,
        added: bool,
    ) -> NativeStemsBeamVLinkBaseApplyCertificate {
        let stem_inter_id = if new_stem {
            GENERATED_STEM_INTER_ID
        } else {
            EXISTING_STEM_INTER_ID
        };
        let stem_vertex = if new_stem {
            BASELINE_VERTEX_COUNT
        } else {
            EXISTING_STEM_VERTEX
        };
        let fresh = NativeStemsBeamRelationObjectIdentity::FreshDraft(PLAN_ORDINAL);
        let stem_after_relations = if added {
            vec![NativeStemsBeamStemIncidentRelation {
                incident_ordinal: 0,
                direction: NativeStemsBeamIncidentDirection::Incoming,
                direction_ordinal: 0,
                graph_relation_identity: NEW_GRAPH_RELATION,
                relation_object_identity: fresh,
                relation_class: "org.audiveris.omr.sig.relation.BeamStemRelation".to_owned(),
                kind: NativeStemsBeamQueryRelationKind::BeamStem,
                opposite_vertex_ordinal: BEAM_VERTEX,
                opposite: NativeStemsBeamIncidentOpposite::Beam,
                opposite_inter_id: BEAM_INTER_ID,
                chord_stem_match: false,
            }]
        } else {
            Vec::new()
        };
        let beam_after_relations = if added {
            vec![NativeStemsBeamBeamIncidentRelation {
                incident_ordinal: 0,
                direction: NativeStemsBeamIncidentDirection::Outgoing,
                direction_ordinal: 0,
                graph_relation_identity: NEW_GRAPH_RELATION,
                relation_object_identity: fresh,
                relation_class: "org.audiveris.omr.sig.relation.BeamStemRelation".to_owned(),
                kind: NativeStemsBeamQueryRelationKind::BeamStem,
                opposite_vertex_ordinal: stem_vertex,
                opposite: NativeStemsBeamIncidentOpposite::Stem,
                opposite_inter_id: stem_inter_id,
                read: NativeStemsBeamBeamIncidentRead::Examined,
                relevant: true,
                beam_portion: matches!(source, NativeStemsBeamSource::Hook(_))
                    .then_some(None)
                    .unwrap_or(Some(NativeBeamPortion::Left)),
            }]
        } else {
            Vec::new()
        };
        let mut certificate = NativeStemsBeamVLinkBaseApplyCertificate {
            system_id: SYSTEM_ID,
            headless: true,
            listener_topology: NativeStemsBeamSigListenerTopology::SoleStandardSigListener,
            endpoint_identity: NativeStemsBeamCertificateEndpointIdentity::JavaPersistentInterId,
            directed_pair_scan: NativeStemsBeamDirectedPairScan {
                source_outgoing_scanned: 0,
                source_outgoing_provenance: NativeStemsBeamQueryProvenance::ExhaustiveSha256(hash()),
                query_relation_count: 0,
                pair_provenance: NativeStemsBeamQueryProvenance::ExhaustiveSha256(hash()),
                relations: Vec::new(),
            },
            stem_incident_before: NativeStemsBeamStemIncidentScan {
                state: if new_stem {
                    NativeStemsBeamStemIncidentScanState::MissingVertex
                } else {
                    NativeStemsBeamStemIncidentScanState::ExhaustiveIncomingThenOutgoing
                },
                query_relation_count: 0,
                query_provenance_sha256: hash(),
                relations: Vec::new(),
            },
            stem_incident_after: NativeStemsBeamStemIncidentScan {
                state: if added {
                    NativeStemsBeamStemIncidentScanState::ExhaustiveIncomingThenOutgoing
                } else {
                    NativeStemsBeamStemIncidentScanState::NotRead
                },
                query_relation_count: stem_after_relations.len(),
                query_provenance_sha256: hash(),
                relations: stem_after_relations,
            },
            beam_incident_before: NativeStemsBeamBeamIncidentScan {
                rule: incident_rule(source),
                query_relation_count: 0,
                query_provenance_sha256: hash(),
                relations: Vec::new(),
            },
            beam_incident_after: NativeStemsBeamBeamIncidentScan {
                rule: if added {
                    incident_rule(source)
                } else {
                    NativeStemsBeamBeamIncidentRule::NotRead
                },
                query_relation_count: beam_after_relations.len(),
                query_provenance_sha256: hash(),
                relations: beam_after_relations,
            },
            chord_stem_matches: 0,
            fresh_relation_object_identity: fresh,
            fresh_relation_graph_matches: 0,
        };
        let empty_query_hash = query_rows_sha256(Vec::<String>::new());
        certificate.directed_pair_scan.source_outgoing_provenance =
            NativeStemsBeamQueryProvenance::ExhaustiveSha256(empty_query_hash.clone());
        certificate.directed_pair_scan.pair_provenance =
            NativeStemsBeamQueryProvenance::ExhaustiveSha256(empty_query_hash);
        certificate.stem_incident_before.query_provenance_sha256 =
            stem_incident_query_sha256(&certificate.stem_incident_before.relations);
        certificate.stem_incident_after.query_provenance_sha256 =
            stem_incident_query_sha256(&certificate.stem_incident_after.relations);
        let hook = matches!(source, NativeStemsBeamSource::Hook(_));
        certificate.beam_incident_before.query_provenance_sha256 =
            beam_incident_query_sha256(&certificate.beam_incident_before.relations, hook);
        certificate.beam_incident_after.query_provenance_sha256 =
            beam_incident_query_sha256(&certificate.beam_incident_after.relations, hook);
        certificate
    }

    fn transaction_state(
        stem: NativeStemsBeamKnownSystemStem,
    ) -> NativeStemsBeamVLinkTransactionState {
        NativeStemsBeamVLinkTransactionState {
            scope: NativeStemsBeamVLinkTransactionScope::SharedSheetFirstFrontier {
                system_id: SYSTEM_ID,
            },
            glyph_index: NativeStemsBeamGlyphIndexTransactionState {
                persistent_ids: NativeStemsBeamPersistentIdState {
                    sheet_last_id: 10,
                    glyph_index_last_id: 10,
                    inter_index_last_id: 10,
                },
                alias_order: NativeStemsBeamGlyphAliasOrder::JavaGlyphId,
                union_size: 1,
                known_canonical_glyphs: Vec::new(),
                exhaustive_lookup: None,
            },
            selected_glyph_bindings: Vec::new(),
            line_states: Vec::new(),
            applied_line_deltas: Vec::new(),
            system_stems: NativeStemsBeamSystemStemTransactionState {
                system_id: SYSTEM_ID,
                next_stem_identity: 1,
                // Hydrated from Java rows: keep the scan requirement.
                authority: NativeStemsBeamRegistryAuthority::RequiresExhaustiveScan,
                known_stems: vec![stem],
                exhaustive_lookup: None,
            },
        }
    }

    fn state(
        source: NativeStemsBeamSource,
        inter_id: Option<i32>,
        added: bool,
        beam_abnormal: bool,
    ) -> NativeStemsBeamVLinkBaseApplyState {
        let stem = known_stem(inter_id);
        let new_stem = inter_id.is_none();
        let certificate = certificate(source, new_stem, added);
        NativeStemsBeamVLinkBaseApplyState {
            transaction_state: transaction_state(stem),
            inter_index: NativeStemsBeamInterIndexApplyState {
                baseline_entry_count: if new_stem { 1 } else { 2 },
                baseline_provenance_sha256: hash(),
                beam_lookup: NativeStemsBeamInterIndexLookup::PresentSameObject {
                    index_ordinal: 0,
                    inter_id: BEAM_INTER_ID,
                    vip: false,
                    object_matches: 1,
                    inter_id_matches: 1,
                    glyph_active_matches: 0,
                    glyph_original_matches: 0,
                },
                stem_lookup: if new_stem {
                    NativeStemsBeamInterIndexLookup::Absent
                } else {
                    NativeStemsBeamInterIndexLookup::PresentSameObject {
                        index_ordinal: 1,
                        inter_id: EXISTING_STEM_INTER_ID,
                        vip: false,
                        object_matches: 1,
                        inter_id_matches: 1,
                        glyph_active_matches: 0,
                        glyph_original_matches: 0,
                    }
                },
                next_id_lookup: if new_stem {
                    NativeStemsBeamNextPersistentIdLookup::VacantAndNotVip {
                        persistent_id: GENERATED_STEM_INTER_ID,
                        inter_id_matches: 0,
                        glyph_active_matches: 0,
                        glyph_original_matches: 0,
                        configured_vip_matches: 0,
                    }
                } else {
                    NativeStemsBeamNextPersistentIdLookup::NotRead
                },
                appended_entries: Vec::new(),
            },
            sig: NativeStemsBeamSigApplyState {
                system_id: SYSTEM_ID,
                baseline_vertex_count: BASELINE_VERTEX_COUNT,
                baseline_vertex_provenance_sha256: hash(),
                baseline_relation_count: NEW_GRAPH_RELATION,
                baseline_relation_provenance_sha256: hash(),
                beam_vertex: NativeStemsBeamSigVertexLookup::PresentSameObject {
                    vertex_ordinal: BEAM_VERTEX,
                    sig_vertex_identity: BEAM_VERTEX,
                    inter_id: BEAM_INTER_ID,
                    object_matches: 1,
                },
                stem_vertex: if new_stem {
                    NativeStemsBeamSigVertexLookup::Absent
                } else {
                    NativeStemsBeamSigVertexLookup::PresentSameObject {
                        vertex_ordinal: EXISTING_STEM_VERTEX,
                        sig_vertex_identity: EXISTING_STEM_VERTEX,
                        inter_id: EXISTING_STEM_INTER_ID,
                        object_matches: 1,
                    }
                },
                appended_vertices: Vec::new(),
                appended_relations: Vec::new(),
                listener_topology: NativeStemsBeamSigListenerTopology::SoleStandardSigListener,
                beam: NativeStemsBeamVLinkBeamRuntimeState {
                    source,
                    sig_vertex_identity: Some(BEAM_VERTEX),
                    inter_id: BEAM_INTER_ID,
                    inter_indexed: true,
                    sig_system_id: SYSTEM_ID,
                    removed: false,
                    vip: false,
                    abnormal: beam_abnormal,
                    stump_group_ordinal: 0,
                    beam_group: None,
                },
                stem: NativeStemsBeamVLinkStemRuntimeState {
                    stem_identity: 0,
                    sig_vertex_identity: (!new_stem).then_some(EXISTING_STEM_VERTEX),
                    inter_indexed: !new_stem,
                    sig_system_id: (!new_stem).then_some(SYSTEM_ID),
                    removed: false,
                    vip: false,
                    abnormal: false,
                },
            },
            sheet_edit: NativeStemsBeamSheetEditState {
                stub_modified: false,
                book_modified: false,
                book_dirty: false,
            },
            certificate: Some(certificate),
            committed: None,
        }
    }

    fn mark_source_removed(state: &mut NativeStemsBeamVLinkBaseApplyState) {
        state.sig.beam.removed = true;
        state.sig.beam.inter_indexed = false;
        state.sig.beam.sig_vertex_identity = None;
        state.inter_index.beam_lookup = NativeStemsBeamInterIndexLookup::Absent;
        state.sig.beam_vertex = NativeStemsBeamSigVertexLookup::Absent;
        let certificate = state.certificate.as_mut().expect("certificate");
        certificate.directed_pair_scan.source_outgoing_scanned = 0;
        certificate.directed_pair_scan.source_outgoing_provenance =
            NativeStemsBeamQueryProvenance::NotRead;
        certificate.directed_pair_scan.query_relation_count = 0;
        certificate.directed_pair_scan.pair_provenance = NativeStemsBeamQueryProvenance::NotRead;
        certificate.directed_pair_scan.relations.clear();
        certificate.beam_incident_before.rule = NativeStemsBeamBeamIncidentRule::NotRead;
        certificate.beam_incident_before.query_relation_count = 0;
        certificate.beam_incident_before.relations.clear();
    }

    fn mark_target_removed(state: &mut NativeStemsBeamVLinkBaseApplyState) {
        state.sig.stem.removed = true;
        state.sig.stem.inter_indexed = false;
        state.sig.stem.sig_vertex_identity = None;
        state.inter_index.stem_lookup = NativeStemsBeamInterIndexLookup::Absent;
        state.sig.stem_vertex = NativeStemsBeamSigVertexLookup::Absent;
        let certificate = state.certificate.as_mut().expect("certificate");
        certificate.directed_pair_scan.source_outgoing_scanned = 0;
        certificate.directed_pair_scan.source_outgoing_provenance =
            NativeStemsBeamQueryProvenance::NotRead;
        certificate.directed_pair_scan.query_relation_count = 0;
        certificate.directed_pair_scan.pair_provenance = NativeStemsBeamQueryProvenance::NotRead;
        certificate.directed_pair_scan.relations.clear();
        certificate.stem_incident_before.state =
            NativeStemsBeamStemIncidentScanState::MissingVertex;
        certificate.stem_incident_before.query_relation_count = 0;
        certificate.stem_incident_before.relations.clear();
    }

    fn prepared(
        state: &NativeStemsBeamVLinkBaseApplyState,
        disposition: NativeStemsBeamVLinkBaseApplyDisposition,
        added: bool,
    ) -> PreparedCommit {
        let inter_id = state.transaction_state.system_stems.known_stems[0].inter_id;
        let source = state.sig.beam.source;
        PreparedCommit {
            key: NativeStemsBeamVLinkBaseApplyKey {
                system_id: SYSTEM_ID,
                invocation_ordinal: 0,
                plan: plan(),
            },
            stem_before: state.transaction_state.system_stems.known_stems[0].clone(),
            relation: relation(source, inter_id),
            certificate: state.certificate.clone().expect("test certificate"),
            vertex_inter_id: inter_id.is_none().then_some(GENERATED_STEM_INTER_ID),
            vertex_identity: inter_id.is_none().then_some(BASELINE_VERTEX_COUNT),
            apply_disposition: disposition,
            pair_relations_read: 0,
            graph_relation_identity: added.then_some(NEW_GRAPH_RELATION),
            post_stem_incident: added
                .then_some(vec![NEW_GRAPH_RELATION])
                .unwrap_or_default(),
            post_beam_incident: added
                .then_some(vec![NEW_GRAPH_RELATION])
                .unwrap_or_default(),
            beam_abnormal_after: added.then_some(!matches!(source, NativeStemsBeamSource::Hook(_))),
            beam_abnormal_read_count: usize::from(added),
            raw_left_found: added && !matches!(source, NativeStemsBeamSource::Hook(_)),
            raw_right_found: false,
            source_removed: state.sig.beam.removed,
            target_removed_read: (!state.sig.beam.removed).then_some(state.sig.stem.removed),
        }
    }

    #[test]
    fn new_id_added_commits_exact_vertex_edge_callback_order() {
        let mut state = state(NativeStemsBeamSource::RawBeam(0), None, true, false);
        let prepared = prepared(
            &state,
            NativeStemsBeamVLinkBaseApplyDisposition::Added {
                graph_relation_identity: NEW_GRAPH_RELATION,
            },
            true,
        );
        let transaction = commit(prepared, &mut state);

        assert_eq!(transaction.stem_before.inter_id, None);
        assert_eq!(
            transaction.stem_after.inter_id,
            Some(GENERATED_STEM_INTER_ID)
        );
        assert_eq!(transaction.fresh_relation.partner_inter_id, None);
        assert!(transaction.apply_returned);
        assert_eq!(transaction.inter_index_mutation_count, 1);
        assert_eq!(transaction.sig_vertex_mutation_count, 1);
        assert_eq!(transaction.sig_relation_mutation_count, 1);
        assert_eq!(transaction.stem_abnormal_mutation_count, 1);
        assert_eq!(transaction.beam_abnormal_mutation_count, 1);
        assert_eq!(
            state
                .transaction_state
                .glyph_index
                .persistent_ids
                .sheet_last_id,
            11
        );
        assert_eq!(state.sig.appended_relations[0].graph_relation_identity, 2);
        assert_eq!(transaction.state_after.as_ref(), &state);
        assert_eq!(
            transaction.operations,
            vec![
                NativeStemsBeamVLinkBaseApplyOperation::SharedPersistentIdAdvanced {
                    before: 10,
                    after: 11,
                },
                NativeStemsBeamVLinkBaseApplyOperation::StemInterIdAssigned {
                    stem_identity: 0,
                    inter_id: 11,
                },
                NativeStemsBeamVLinkBaseApplyOperation::InterIndexInserted {
                    stem_identity: 0,
                    inter_id: 11,
                },
                NativeStemsBeamVLinkBaseApplyOperation::SigVertexInserted {
                    sig_vertex_identity: 3,
                },
                NativeStemsBeamVLinkBaseApplyOperation::SigVertexEventDispatched,
                NativeStemsBeamVLinkBaseApplyOperation::StandardSigListenerVertexCallbackCompleted,
                NativeStemsBeamVLinkBaseApplyOperation::StemSigAttached { system_id: 1 },
                NativeStemsBeamVLinkBaseApplyOperation::StemAddedCallbackStarted,
                NativeStemsBeamVLinkBaseApplyOperation::StemRemovedCleared { before: false },
                NativeStemsBeamVLinkBaseApplyOperation::StemAbnormalSet {
                    before: false,
                    after: true,
                },
                NativeStemsBeamVLinkBaseApplyOperation::SheetStubModifiedSetTrue,
                NativeStemsBeamVLinkBaseApplyOperation::BookModifiedSetTrue,
                NativeStemsBeamVLinkBaseApplyOperation::BookDirtySetTrue,
                NativeStemsBeamVLinkBaseApplyOperation::StemAddedCallbackCompleted,
                NativeStemsBeamVLinkBaseApplyOperation::SigGlobalRelationInserted {
                    graph_relation_identity: 2,
                },
                NativeStemsBeamVLinkBaseApplyOperation::BeamOutgoingRelationInserted {
                    graph_relation_identity: 2,
                },
                NativeStemsBeamVLinkBaseApplyOperation::StemIncomingRelationInserted {
                    graph_relation_identity: 2,
                },
                NativeStemsBeamVLinkBaseApplyOperation::SigEdgeEventDispatched {
                    graph_relation_identity: 2,
                },
                NativeStemsBeamVLinkBaseApplyOperation::StandardSigListenerEdgeCallbackStarted,
                NativeStemsBeamVLinkBaseApplyOperation::BeamStemRelationCallbackStarted,
                NativeStemsBeamVLinkBaseApplyOperation::StemChordIncidentScanCompleted {
                    incident_relation_count: 1,
                    chord_stem_matches: 0,
                },
                NativeStemsBeamVLinkBaseApplyOperation::BeamAbnormalSet {
                    before: false,
                    after: true,
                },
                NativeStemsBeamVLinkBaseApplyOperation::SheetStubModifiedSetTrue,
                NativeStemsBeamVLinkBaseApplyOperation::BookModifiedSetTrue,
                NativeStemsBeamVLinkBaseApplyOperation::BookDirtySetTrue,
                NativeStemsBeamVLinkBaseApplyOperation::BeamStemRelationCallbackCompleted,
                NativeStemsBeamVLinkBaseApplyOperation::StandardSigListenerEdgeCallbackCompleted,
            ]
        );
    }

    #[test]
    fn existing_positive_stem_skips_registration_and_hook_clears_abnormal() {
        let mut state = state(
            NativeStemsBeamSource::Hook(0),
            Some(EXISTING_STEM_INTER_ID),
            true,
            true,
        );
        let prepared = prepared(
            &state,
            NativeStemsBeamVLinkBaseApplyDisposition::Added {
                graph_relation_identity: NEW_GRAPH_RELATION,
            },
            true,
        );
        let transaction = commit(prepared, &mut state);
        assert_eq!(
            transaction.vertex_action,
            NativeStemsBeamVLinkVertexAction::SkippedPositiveInterId
        );
        assert_eq!(transaction.persistent_id_mutation_count, 0);
        assert_eq!(transaction.sig_vertex_mutation_count, 0);
        assert_eq!(transaction.beam_abnormal_mutation_count, 1);
        assert!(!state.sig.beam.abnormal);
        assert_eq!(transaction.continuation_support_grade, 0.5);
    }

    #[test]
    fn all_apply_suppressions_retain_fresh_draft_grade() {
        let cases = [
            NativeStemsBeamVLinkBaseApplyDisposition::SuppressedSourceRemoved,
            NativeStemsBeamVLinkBaseApplyDisposition::SuppressedTargetRemoved,
            NativeStemsBeamVLinkBaseApplyDisposition::SuppressedExistingBeamStem {
                graph_relation_identity: 1,
            },
        ];
        for (index, disposition) in cases.into_iter().enumerate() {
            let inter_id = (index != 0).then_some(EXISTING_STEM_INTER_ID);
            let mut state = state(NativeStemsBeamSource::RawBeam(0), inter_id, false, false);
            match disposition {
                NativeStemsBeamVLinkBaseApplyDisposition::SuppressedSourceRemoved => {
                    mark_source_removed(&mut state);
                }
                NativeStemsBeamVLinkBaseApplyDisposition::SuppressedTargetRemoved => {
                    mark_target_removed(&mut state);
                }
                NativeStemsBeamVLinkBaseApplyDisposition::SuppressedExistingBeamStem {
                    graph_relation_identity,
                } => {
                    let certificate = state.certificate.as_mut().expect("certificate");
                    certificate.directed_pair_scan.relations.push(
                        NativeStemsBeamDirectedPairRelation {
                            pair_ordinal: 0,
                            source_outgoing_ordinal: 0,
                            graph_relation_identity,
                            relation_object_identity:
                                NativeStemsBeamRelationObjectIdentity::GraphObject(
                                    graph_relation_identity,
                                ),
                            relation_class: "org.audiveris.omr.sig.relation.BeamStemRelation"
                                .to_owned(),
                            kind: NativeStemsBeamQueryRelationKind::BeamStem,
                            class_read: NativeStemsBeamPairClassRead::ExaminedMatchBreak,
                        },
                    );
                }
                NativeStemsBeamVLinkBaseApplyDisposition::Added { .. } => unreachable!(),
            }
            let mut prepared = prepared(&state, disposition, false);
            prepared.pair_relations_read = usize::from(index == 2);
            let transaction = commit(prepared, &mut state);
            assert!(!transaction.apply_returned);
            assert_eq!(transaction.continuation_support_grade, 0.5);
            assert_eq!(transaction.sig_relation_mutation_count, 0);
            assert!(!transaction.callback.called);
            if index == 0 {
                assert_eq!(transaction.sig_vertex_mutation_count, 1);
                assert_eq!(transaction.stem_abnormal_mutation_count, 1);
            } else {
                assert_eq!(transaction.sig_vertex_mutation_count, 0);
            }
        }
    }

    #[test]
    fn callback_validator_accepts_exact_raw_and_hook_zero_chord_traces() {
        for source in [
            NativeStemsBeamSource::RawBeam(0),
            NativeStemsBeamSource::Hook(0),
        ] {
            let state = state(source, None, true, false);
            let certificate = state.certificate.as_ref().expect("certificate");
            assert!(
                validate_callback_queries(
                    &state,
                    certificate,
                    &relation(source, None),
                    Some(NEW_GRAPH_RELATION),
                    BEAM_INTER_ID,
                    GENERATED_STEM_INTER_ID,
                )
                .is_ok()
            );
            assert!(validate_zero_chord_envelope(certificate).is_ok());
        }
    }

    #[test]
    fn raw_null_beam_support_is_examined_without_side_contribution() {
        let state = state(NativeStemsBeamSource::RawBeam(0), None, true, false);
        let mut certificate = state.certificate.clone().expect("certificate");
        let null_portion = NativeStemsBeamBeamIncidentRelation {
            incident_ordinal: 0,
            direction: NativeStemsBeamIncidentDirection::Outgoing,
            direction_ordinal: 0,
            graph_relation_identity: 0,
            relation_object_identity: NativeStemsBeamRelationObjectIdentity::GraphObject(0),
            relation_class: "org.audiveris.omr.sig.relation.BeamStemRelation".to_owned(),
            kind: NativeStemsBeamQueryRelationKind::BeamStem,
            opposite_vertex_ordinal: 2,
            opposite: NativeStemsBeamIncidentOpposite::OtherInter,
            opposite_inter_id: 9,
            read: NativeStemsBeamBeamIncidentRead::Examined,
            relevant: false,
            beam_portion: None,
        };
        certificate.beam_incident_before.relations = vec![null_portion.clone()];
        certificate.beam_incident_before.query_relation_count = 1;
        certificate.beam_incident_before.query_provenance_sha256 =
            beam_incident_query_sha256(&certificate.beam_incident_before.relations, false);
        certificate.beam_incident_after.relations[0].incident_ordinal = 1;
        certificate.beam_incident_after.relations[0].direction_ordinal = 1;
        certificate
            .beam_incident_after
            .relations
            .insert(0, null_portion);
        certificate.beam_incident_after.query_relation_count = 2;
        certificate.beam_incident_after.query_provenance_sha256 =
            beam_incident_query_sha256(&certificate.beam_incident_after.relations, false);

        assert!(
            validate_callback_queries(
                &state,
                &certificate,
                &relation(NativeStemsBeamSource::RawBeam(0), None),
                Some(NEW_GRAPH_RELATION),
                BEAM_INTER_ID,
                GENERATED_STEM_INTER_ID,
            )
            .is_ok()
        );
    }

    #[test]
    fn zero_chord_envelope_rejects_any_match() {
        let mut certificate = certificate(NativeStemsBeamSource::RawBeam(0), true, true);
        certificate.chord_stem_matches = 1;
        assert!(matches!(
            validate_zero_chord_envelope(&certificate),
            Err(NativeStemsBeamVLinkBaseApplyError::UnsupportedV1 { .. })
        ));
    }

    #[test]
    fn new_vertex_append_ordinals_fail_closed_on_overflow() {
        assert!(validate_new_vertex_capacity(1, 1).is_ok());
        assert!(validate_new_vertex_capacity(usize::MAX, 1).is_err());
        assert!(validate_new_vertex_capacity(1, usize::MAX).is_err());
    }

    #[test]
    fn endpoint_role_and_identity_collisions_fail_closed() {
        let state = state(NativeStemsBeamSource::RawBeam(0), None, true, false);
        let mut certificate = state.certificate.clone().expect("certificate");
        certificate.beam_incident_after.relations[0].opposite =
            NativeStemsBeamIncidentOpposite::OtherInter;
        assert!(
            validate_callback_queries(
                &state,
                &certificate,
                &relation(NativeStemsBeamSource::RawBeam(0), None),
                Some(NEW_GRAPH_RELATION),
                BEAM_INTER_ID,
                GENERATED_STEM_INTER_ID,
            )
            .is_err()
        );

        let mut certificate = state.certificate.clone().expect("certificate");
        certificate
            .stem_incident_after
            .relations
            .push(NativeStemsBeamStemIncidentRelation {
                incident_ordinal: 1,
                direction: NativeStemsBeamIncidentDirection::Incoming,
                direction_ordinal: 1,
                graph_relation_identity: NEW_GRAPH_RELATION + 1,
                relation_object_identity: NativeStemsBeamRelationObjectIdentity::GraphObject(1),
                relation_class: "example.OtherRelation".to_owned(),
                kind: NativeStemsBeamQueryRelationKind::Other,
                opposite_vertex_ordinal: BEAM_VERTEX,
                opposite: NativeStemsBeamIncidentOpposite::OtherInter,
                opposite_inter_id: 9,
                chord_stem_match: false,
            });
        assert!(
            validate_incident_endpoint_domains(
                &state,
                &certificate,
                BEAM_INTER_ID,
                GENERATED_STEM_INTER_ID,
                true,
            )
            .is_err()
        );
    }

    #[test]
    fn selected_endpoint_incidence_requires_inverse_mirrored_rows() {
        let stem = NativeStemsBeamStemIncidentRelation {
            incident_ordinal: 0,
            direction: NativeStemsBeamIncidentDirection::Incoming,
            direction_ordinal: 0,
            graph_relation_identity: 0,
            relation_object_identity: NativeStemsBeamRelationObjectIdentity::GraphObject(0),
            relation_class: "example.OtherRelation".to_owned(),
            kind: NativeStemsBeamQueryRelationKind::Other,
            opposite_vertex_ordinal: BEAM_VERTEX,
            opposite: NativeStemsBeamIncidentOpposite::Beam,
            opposite_inter_id: BEAM_INTER_ID,
            chord_stem_match: false,
        };
        let mut beam = NativeStemsBeamBeamIncidentRelation {
            incident_ordinal: 0,
            direction: NativeStemsBeamIncidentDirection::Incoming,
            direction_ordinal: 0,
            graph_relation_identity: 0,
            relation_object_identity: NativeStemsBeamRelationObjectIdentity::GraphObject(0),
            relation_class: "example.OtherRelation".to_owned(),
            kind: NativeStemsBeamQueryRelationKind::Other,
            opposite_vertex_ordinal: EXISTING_STEM_VERTEX,
            opposite: NativeStemsBeamIncidentOpposite::Stem,
            opposite_inter_id: EXISTING_STEM_INTER_ID,
            read: NativeStemsBeamBeamIncidentRead::Examined,
            relevant: false,
            beam_portion: None,
        };

        assert!(
            validate_selected_endpoint_incidence_join(std::slice::from_ref(&stem), &[]).is_err()
        );
        assert!(
            validate_selected_endpoint_incidence_join(
                std::slice::from_ref(&stem),
                std::slice::from_ref(&beam),
            )
            .is_err()
        );
        beam.direction = NativeStemsBeamIncidentDirection::Outgoing;
        assert!(validate_selected_endpoint_incidence_join(&[stem], &[beam]).is_ok());
    }

    #[test]
    fn phase_validator_rejects_extra_old_edge_and_read_drift() {
        let state = state(
            NativeStemsBeamSource::Hook(0),
            Some(EXISTING_STEM_INTER_ID),
            true,
            false,
        );
        let mut certificate = state.certificate.clone().expect("certificate");
        certificate.stem_incident_after.relations.insert(
            0,
            NativeStemsBeamStemIncidentRelation {
                incident_ordinal: 0,
                direction: NativeStemsBeamIncidentDirection::Incoming,
                direction_ordinal: 0,
                graph_relation_identity: 1,
                relation_object_identity: NativeStemsBeamRelationObjectIdentity::GraphObject(1),
                relation_class: "example.OtherRelation".to_owned(),
                kind: NativeStemsBeamQueryRelationKind::Other,
                opposite_vertex_ordinal: 2,
                opposite: NativeStemsBeamIncidentOpposite::OtherInter,
                opposite_inter_id: 9,
                chord_stem_match: false,
            },
        );
        certificate.stem_incident_after.relations[1].incident_ordinal = 1;
        certificate.stem_incident_after.relations[1].direction_ordinal = 1;
        assert!(validate_stem_phase_consistency(&certificate, NEW_GRAPH_RELATION).is_err());

        let mut certificate = state.certificate.clone().expect("certificate");
        let mut old = certificate.beam_incident_after.relations[0].clone();
        old.graph_relation_identity = 0;
        old.relation_object_identity = NativeStemsBeamRelationObjectIdentity::GraphObject(0);
        old.opposite = NativeStemsBeamIncidentOpposite::OtherInter;
        old.opposite_vertex_ordinal = 2;
        old.opposite_inter_id = 9;
        old.kind = NativeStemsBeamQueryRelationKind::Other;
        old.relation_class = "example.OtherRelation".to_owned();
        old.relevant = false;
        certificate.beam_incident_before.relations = vec![old.clone()];
        certificate.beam_incident_after.relations.insert(0, old);
        certificate.beam_incident_after.relations[0].read =
            NativeStemsBeamBeamIncidentRead::UnreadAfterBreak;
        assert!(validate_beam_phase_consistency(&certificate, NEW_GRAPH_RELATION).is_err());
    }

    #[test]
    fn suppression_still_requires_one_coherent_pre_callback_graph_snapshot() {
        let state = state(
            NativeStemsBeamSource::RawBeam(0),
            Some(EXISTING_STEM_INTER_ID),
            false,
            false,
        );
        let mut certificate = state.certificate.clone().expect("certificate");
        certificate
            .stem_incident_before
            .relations
            .push(NativeStemsBeamStemIncidentRelation {
                incident_ordinal: 0,
                direction: NativeStemsBeamIncidentDirection::Incoming,
                direction_ordinal: 0,
                graph_relation_identity: 0,
                relation_object_identity: NativeStemsBeamRelationObjectIdentity::GraphObject(0),
                relation_class: "example.OtherRelation".to_owned(),
                kind: NativeStemsBeamQueryRelationKind::Other,
                opposite_vertex_ordinal: 2,
                opposite: NativeStemsBeamIncidentOpposite::OtherInter,
                opposite_inter_id: 9,
                chord_stem_match: false,
            });
        certificate
            .beam_incident_before
            .relations
            .push(NativeStemsBeamBeamIncidentRelation {
                incident_ordinal: 0,
                direction: NativeStemsBeamIncidentDirection::Incoming,
                direction_ordinal: 0,
                graph_relation_identity: 0,
                relation_object_identity: NativeStemsBeamRelationObjectIdentity::GraphObject(0),
                relation_class: "org.audiveris.omr.sig.relation.BeamRestRelation".to_owned(),
                kind: NativeStemsBeamQueryRelationKind::BeamRest,
                opposite_vertex_ordinal: 2,
                opposite: NativeStemsBeamIncidentOpposite::OtherInter,
                opposite_inter_id: 9,
                read: NativeStemsBeamBeamIncidentRead::Examined,
                relevant: true,
                beam_portion: Some(NativeBeamPortion::Left),
            });
        assert!(
            validate_pre_callback_snapshot_consistency(
                &state,
                &certificate,
                BEAM_INTER_ID,
                EXISTING_STEM_INTER_ID,
            )
            .is_err()
        );
    }

    #[test]
    fn malformed_index_and_sig_certificates_leave_state_unchanged() {
        let mut state = state(NativeStemsBeamSource::RawBeam(0), None, true, false);
        let before = state.clone();
        state.inter_index.beam_lookup = NativeStemsBeamInterIndexLookup::PresentSameObject {
            index_ordinal: 0,
            inter_id: BEAM_INTER_ID,
            vip: false,
            object_matches: 0,
            inter_id_matches: 1,
            glyph_active_matches: 0,
            glyph_original_matches: 0,
        };
        let invalid = state.clone();
        assert!(
            validate_compact_state(
                &state,
                state.certificate.as_ref().expect("certificate"),
                &state.transaction_state.system_stems.known_stems[0],
            )
            .is_err()
        );
        assert_eq!(state, invalid);
        assert_ne!(state, before);

        let mut state = before;
        state.sig.beam_vertex = NativeStemsBeamSigVertexLookup::PresentSameObject {
            vertex_ordinal: BEAM_VERTEX,
            sig_vertex_identity: 2,
            inter_id: BEAM_INTER_ID,
            object_matches: 1,
        };
        let invalid = state.clone();
        assert!(
            validate_compact_state(
                &state,
                state.certificate.as_ref().expect("certificate"),
                &state.transaction_state.system_stems.known_stems[0],
            )
            .is_err()
        );
        assert_eq!(state, invalid);
    }

    #[test]
    fn existing_endpoints_must_have_distinct_index_and_sig_identities() {
        let mut state = state(
            NativeStemsBeamSource::RawBeam(0),
            Some(EXISTING_STEM_INTER_ID),
            true,
            false,
        );
        assert!(validate_existing_endpoint_state(&state, EXISTING_STEM_INTER_ID).is_ok());

        if let NativeStemsBeamInterIndexLookup::PresentSameObject { index_ordinal, .. } =
            &mut state.inter_index.stem_lookup
        {
            *index_ordinal = 0;
        }
        assert!(validate_existing_endpoint_state(&state, EXISTING_STEM_INTER_ID).is_err());
    }

    #[test]
    fn removed_endpoint_pair_query_requires_typed_not_read_provenance() {
        let mut state = state(
            NativeStemsBeamSource::RawBeam(0),
            Some(EXISTING_STEM_INTER_ID),
            false,
            false,
        );
        state.sig.beam.removed = true;
        let mut certificate = state.certificate.clone().expect("certificate");
        assert!(
            validate_directed_pair_query(&state, &certificate, EXISTING_STEM_INTER_ID).is_err()
        );
        certificate.directed_pair_scan.source_outgoing_provenance =
            NativeStemsBeamQueryProvenance::NotRead;
        certificate.directed_pair_scan.pair_provenance = NativeStemsBeamQueryProvenance::NotRead;
        assert!(validate_directed_pair_query(&state, &certificate, EXISTING_STEM_INTER_ID).is_ok());
    }

    #[test]
    fn removed_source_is_absent_but_retains_sig_pointer_and_short_circuits_target_read() {
        let mut state = state(
            NativeStemsBeamSource::RawBeam(0),
            Some(EXISTING_STEM_INTER_ID),
            false,
            false,
        );
        mark_source_removed(&mut state);
        let certificate = state.certificate.as_ref().expect("certificate");
        let stem = &state.transaction_state.system_stems.known_stems[0];
        assert_eq!(state.sig.beam.sig_system_id, SYSTEM_ID);
        assert!(validate_compact_state(&state, certificate, stem).is_ok());
        assert!(validate_existing_endpoint_state(&state, EXISTING_STEM_INTER_ID).is_ok());
        assert!(validate_directed_pair_query(&state, certificate, EXISTING_STEM_INTER_ID).is_ok());
        assert!(
            validate_callback_queries(
                &state,
                certificate,
                &relation(NativeStemsBeamSource::RawBeam(0), stem.inter_id),
                None,
                BEAM_INTER_ID,
                EXISTING_STEM_INTER_ID,
            )
            .is_ok()
        );

        let prepared = prepared(
            &state,
            NativeStemsBeamVLinkBaseApplyDisposition::SuppressedSourceRemoved,
            false,
        );
        let mut committed = state;
        let transaction = commit(prepared, &mut committed);
        assert!(transaction.removed_reads.source_removed);
        assert_eq!(transaction.removed_reads.target_removed, None);
        assert_eq!(transaction.sig_vertex_mutation_count, 0);
        assert_eq!(transaction.sig_relation_mutation_count, 0);
        assert_eq!(committed.sig.beam.sig_vertex_identity, None);
        assert_eq!(
            committed.sig.beam_vertex,
            NativeStemsBeamSigVertexLookup::Absent
        );
    }

    #[test]
    fn removed_target_is_absent_but_retains_sig_pointer_and_is_read_second() {
        let mut state = state(
            NativeStemsBeamSource::RawBeam(0),
            Some(EXISTING_STEM_INTER_ID),
            false,
            false,
        );
        mark_target_removed(&mut state);
        let certificate = state.certificate.as_ref().expect("certificate");
        let stem = &state.transaction_state.system_stems.known_stems[0];
        assert_eq!(state.sig.stem.sig_system_id, Some(SYSTEM_ID));
        assert!(validate_compact_state(&state, certificate, stem).is_ok());
        assert_eq!(
            validate_existing_endpoint_state(&state, EXISTING_STEM_INTER_ID),
            Ok(None)
        );
        assert!(validate_directed_pair_query(&state, certificate, EXISTING_STEM_INTER_ID).is_ok());
        assert!(
            validate_callback_queries(
                &state,
                certificate,
                &relation(NativeStemsBeamSource::RawBeam(0), stem.inter_id),
                None,
                BEAM_INTER_ID,
                EXISTING_STEM_INTER_ID,
            )
            .is_ok()
        );

        let mut fabricated = certificate.clone();
        fabricated
            .beam_incident_before
            .relations
            .push(NativeStemsBeamBeamIncidentRelation {
                incident_ordinal: 0,
                direction: NativeStemsBeamIncidentDirection::Incoming,
                direction_ordinal: 0,
                graph_relation_identity: 0,
                relation_object_identity: NativeStemsBeamRelationObjectIdentity::GraphObject(0),
                relation_class: "example.OtherRelation".to_owned(),
                kind: NativeStemsBeamQueryRelationKind::Other,
                opposite_vertex_ordinal: BASELINE_VERTEX_COUNT,
                opposite: NativeStemsBeamIncidentOpposite::Stem,
                opposite_inter_id: EXISTING_STEM_INTER_ID,
                read: NativeStemsBeamBeamIncidentRead::Examined,
                relevant: false,
                beam_portion: None,
            });
        fabricated.beam_incident_before.query_relation_count = 1;
        fabricated.beam_incident_before.query_provenance_sha256 =
            beam_incident_query_sha256(&fabricated.beam_incident_before.relations, false);
        assert!(
            validate_callback_queries(
                &state,
                &fabricated,
                &relation(NativeStemsBeamSource::RawBeam(0), stem.inter_id),
                None,
                BEAM_INTER_ID,
                EXISTING_STEM_INTER_ID,
            )
            .is_err()
        );

        let prepared = prepared(
            &state,
            NativeStemsBeamVLinkBaseApplyDisposition::SuppressedTargetRemoved,
            false,
        );
        let mut committed = state;
        let transaction = commit(prepared, &mut committed);
        assert!(!transaction.removed_reads.source_removed);
        assert_eq!(transaction.removed_reads.target_removed, Some(true));
        assert_eq!(transaction.sig_vertex_mutation_count, 0);
        assert_eq!(transaction.sig_relation_mutation_count, 0);
        assert_eq!(committed.sig.stem.sig_vertex_identity, None);
        assert_eq!(
            committed.sig.stem_vertex,
            NativeStemsBeamSigVertexLookup::Absent
        );
    }

    #[test]
    fn id_zero_vertex_prefix_commits_before_source_removed_suppression() {
        let mut state = state(NativeStemsBeamSource::RawBeam(0), None, false, false);
        mark_source_removed(&mut state);
        let certificate = state.certificate.as_ref().expect("certificate");
        let stem = &state.transaction_state.system_stems.known_stems[0];
        assert!(validate_compact_state(&state, certificate, stem).is_ok());
        assert!(validate_directed_pair_query(&state, certificate, GENERATED_STEM_INTER_ID).is_ok());
        assert!(
            validate_callback_queries(
                &state,
                certificate,
                &relation(NativeStemsBeamSource::RawBeam(0), None),
                None,
                BEAM_INTER_ID,
                GENERATED_STEM_INTER_ID,
            )
            .is_ok()
        );

        let prepared = prepared(
            &state,
            NativeStemsBeamVLinkBaseApplyDisposition::SuppressedSourceRemoved,
            false,
        );
        let transaction = commit(prepared, &mut state);
        assert_eq!(transaction.removed_reads.target_removed, None);
        assert_eq!(transaction.persistent_id_mutation_count, 1);
        assert_eq!(transaction.inter_index_mutation_count, 1);
        assert_eq!(transaction.sig_vertex_mutation_count, 1);
        assert_eq!(transaction.sig_relation_mutation_count, 0);
        assert_eq!(
            transaction.stem_after.inter_id,
            Some(GENERATED_STEM_INTER_ID)
        );
        assert!(transaction.stem_after.sig_attached);
        assert_eq!(state.sig.beam.sig_vertex_identity, None);
        assert_eq!(
            state.sig.stem.sig_vertex_identity,
            Some(BASELINE_VERTEX_COUNT)
        );
    }

    #[test]
    fn removed_membership_certificates_fail_closed_without_mutation() {
        let mut source = state(
            NativeStemsBeamSource::RawBeam(0),
            Some(EXISTING_STEM_INTER_ID),
            false,
            false,
        );
        mark_source_removed(&mut source);
        source.sig.beam.sig_vertex_identity = Some(BEAM_VERTEX);
        let before = source.clone();
        assert!(
            validate_compact_state(
                &source,
                source.certificate.as_ref().expect("certificate"),
                &source.transaction_state.system_stems.known_stems[0],
            )
            .is_err()
        );
        assert_eq!(source, before);

        let mut target = state(
            NativeStemsBeamSource::RawBeam(0),
            Some(EXISTING_STEM_INTER_ID),
            false,
            false,
        );
        mark_target_removed(&mut target);
        target.inter_index.stem_lookup = NativeStemsBeamInterIndexLookup::PresentSameObject {
            index_ordinal: 1,
            inter_id: EXISTING_STEM_INTER_ID,
            vip: false,
            object_matches: 1,
            inter_id_matches: 1,
            glyph_active_matches: 0,
            glyph_original_matches: 0,
        };
        let before = target.clone();
        assert!(validate_existing_endpoint_state(&target, EXISTING_STEM_INTER_ID).is_err());
        assert_eq!(target, before);
    }

    #[test]
    fn lowercase_sha_and_fresh_draft_domains_are_strict() {
        assert!(valid_sha256(&hash()));
        assert!(!valid_sha256(&"A".repeat(64)));
        assert_eq!(
            query_rows_sha256(Vec::<String>::new()),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let mut certificate = certificate(NativeStemsBeamSource::RawBeam(0), true, true);
        assert!(validate_fresh_relation_identity(&certificate, plan().plan_ordinal).is_ok());
        certificate.fresh_relation_object_identity =
            NativeStemsBeamRelationObjectIdentity::FreshDraft(plan().plan_ordinal + 1);
        assert!(validate_fresh_relation_identity(&certificate, plan().plan_ordinal).is_err());
    }

    #[test]
    fn every_query_local_hash_is_recomputed_not_just_well_formed() {
        let state = state(NativeStemsBeamSource::RawBeam(0), None, true, false);
        let exact = state.certificate.clone().expect("certificate");
        let corrupt = "1".repeat(64);

        for source_hash in [true, false] {
            let mut certificate = exact.clone();
            if source_hash {
                certificate.directed_pair_scan.source_outgoing_provenance =
                    NativeStemsBeamQueryProvenance::ExhaustiveSha256(corrupt.clone());
            } else {
                certificate.directed_pair_scan.pair_provenance =
                    NativeStemsBeamQueryProvenance::ExhaustiveSha256(corrupt.clone());
            }
            assert!(
                validate_directed_pair_query(&state, &certificate, GENERATED_STEM_INTER_ID)
                    .is_err()
            );
        }

        for field in 0..4 {
            let mut certificate = exact.clone();
            match field {
                0 => certificate.stem_incident_before.query_provenance_sha256 = corrupt.clone(),
                1 => certificate.stem_incident_after.query_provenance_sha256 = corrupt.clone(),
                2 => certificate.beam_incident_before.query_provenance_sha256 = corrupt.clone(),
                3 => certificate.beam_incident_after.query_provenance_sha256 = corrupt.clone(),
                _ => unreachable!(),
            }
            assert!(
                validate_callback_queries(
                    &state,
                    &certificate,
                    &relation(NativeStemsBeamSource::RawBeam(0), None),
                    Some(NEW_GRAPH_RELATION),
                    BEAM_INTER_ID,
                    GENERATED_STEM_INTER_ID,
                )
                .is_err()
            );
        }
    }
}
