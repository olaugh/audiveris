// SPDX-License-Identifier: AGPL-3.0-or-later

//! Pure beam-origin V-link decision after `StemBuilder.createStem`.
//!
//! This boundary reads the already committed create-stem transaction, performs
//! Java's rare head-side stem-reuse loop in `LinkedHashMap` insertion order,
//! and evaluates `BeamStemRelation.checkLink`. It stops before `SIGraph`
//! mutation, `Link.applyTo`, sibling links, or linker flags. The committed
//! transaction state is accepted only by shared reference and cannot be
//! changed here.

use std::{error::Error, fmt};

use audiveris_core::java_math::java_positive_pow;
use audiveris_image::beam_structure::Segment;

use crate::{
    native_sig::{
        NativeSigInterKind, NativeSigRelationKind, NativeSigSystem, NativeSigSystemBindings,
    },
    native_stems_beam_link_plans::{
        NativeStemsBeamHeadRelation, NativeStemsBeamLinkPlanAttempt,
        NativeStemsBeamLinkPlanOutcome, NativeStemsBeamLinkPlanSystem,
    },
    native_stems_beam_reachability::NativeStemsBeamHeadCornerRef,
    native_stems_beam_scheduler::{
        NativeStemsBeamPlanRef, NativeStemsBeamSchedulerStatus, NativeStemsBeamSchedulerSystem,
    },
    native_stems_beam_stumps::{
        NativeStemsBeamSource, NativeStemsBeamStumpBeam, NativeStemsBeamStumpSystem,
    },
    native_stems_beam_vlink_head_links::{
        NativeStemsBeamHeadLinkHeadRef, NativeStemsBeamHeadSLinkerRef,
        NativeStemsBeamNativeSLinkerCell,
    },
    native_stems_beam_vlink_transaction::{
        NativeStemsBeamAppliedLineDeltaSource, NativeStemsBeamCreateStemDisposition,
        NativeStemsBeamGlyphRegistrationAction, NativeStemsBeamKnownSystemStem,
        NativeStemsBeamStemGrade, NativeStemsBeamSystemStemTransactionState,
        NativeStemsBeamVLinkMutation, NativeStemsBeamVLinkTransaction,
        NativeStemsBeamVLinkTransactionState, build_compound, sha256_hex, valid_stem_grade,
        validate_known_stem, validate_transaction_state as validate_create_stem_state,
    },
    native_stems_beam_vlinkers::{
        NativeStemsBeamVLinkerRef, NativeStemsBeamVLinkerSystem, beam_border, generic_intersection,
    },
    stem_seeds_step::NativeStemCheckResult,
    stems_step::{
        NativeBeamPortion, NativeStemHeadSide, NativeStemLine, NativeStemPoint,
        NativeStemVerticalSide,
    },
};

/// Live reads required after a successful `createStem` call.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkReuseLiveState {
    pub system_id: usize,
    /// Exact full payloads for the distinct StemInter targets present in every
    /// examined HeadStem scan. This is deliberately separate from
    /// `StemsRetriever.systemStems`: a live SIG stem need not be in that map.
    /// Unrelated or unread-suffix stems are not represented.
    pub live_sig_stems: Vec<NativeStemsBeamKnownSystemStem>,
    pub evaluation: NativeStemsBeamVLinkReuseLiveEvaluation,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamVLinkReuseLiveEvaluation {
    /// Java returned before the reuse loop because `createStem` returned null.
    NotReadCreateStemRejected,
    /// One entry per frozen plan relation, including an explicit unread suffix
    /// after Java's first unique reuse selection.
    Entries(Vec<NativeStemsBeamReuseEntryEvidence>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamReuseEntryEvidence {
    pub map_ordinal: usize,
    pub corner: NativeStemsBeamHeadCornerRef,
    pub observation: NativeStemsBeamReuseEntryObservation,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamReuseEntryObservation {
    Examined {
        /// `CLinker.isLinked()` reads this shared parent S-linker flag.
        s_linker_linked: bool,
        head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence,
    },
    /// Java broke on an earlier unique side-stem object and reads nothing from
    /// this entry, including its shared S-linker flag.
    UnreadAfterSelection,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamHeadStemLookupEvidence {
    /// Required when the shared S-linker is not linked; Java does not ask the
    /// graph for incident HeadStem relations.
    NotRead,
    Exhaustive(NativeStemsBeamHeadStemScan),
}

/// Complete live `HeadStemRelation` scan for one queried HeadInter.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamHeadStemScan {
    pub corner: NativeStemsBeamHeadCornerRef,
    pub baseline_relation_count: usize,
    pub scanned_relation_count: usize,
    pub provenance_sha256: String,
    pub edges: Vec<NativeStemsBeamHeadStemEdge>,
}

impl NativeStemsBeamHeadStemScan {
    /// Recompute the Java probe's `snapshotHash` for the first-reference
    /// live-stem catalogue.
    pub fn java_snapshot_sha256(
        &self,
        stem_catalogue: &[NativeStemsBeamKnownSystemStem],
    ) -> Result<String, NativeStemsBeamVLinkReuseCheckError> {
        head_stem_scan_hashes(self, NativeStemHeadSide::Left, stem_catalogue, 0)
            .map(|(snapshot, _)| snapshot)
    }

    /// Recompute the Java probe's `projectionHash` for an independently known
    /// parent side and the first-reference live-stem catalogue.
    pub fn java_projection_sha256(
        &self,
        parent_side: NativeStemHeadSide,
        stem_catalogue: &[NativeStemsBeamKnownSystemStem],
    ) -> Result<String, NativeStemsBeamVLinkReuseCheckError> {
        head_stem_scan_hashes(self, parent_side, stem_catalogue, 0)
            .map(|(_, projection)| projection)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamHeadStemEdge {
    /// Dense position in this `HeadInter.getSideStems()` relation scan.
    pub scan_ordinal: usize,
    pub relation_identity: usize,
    /// Side stored on the live HeadStemRelation, independently of the queried
    /// C-linker's parent horizontal side.
    pub relation_head_side: NativeStemHeadSide,
    /// Dense native object identity, never a Java Inter ID.
    pub target_stem_identity: usize,
}

/// Effective Java constants and Scale values used by `checkLink`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsBeamRelationParameters {
    pub interline: i32,
    pub main_stem_thickness: f64,
    pub profile: i32,
    pub x_in_gap_maximum_profile0: f64,
    pub x_out_gap_maximum: f64,
    pub y_gap_maximum: f64,
    pub x_weight: f64,
    pub y_weight: f64,
    pub intrinsic_ratio: f64,
    pub minimum_grade: f64,
}

impl NativeStemsBeamRelationParameters {
    /// Derive Java's BeamStemRelation/checkLink constants from the native
    /// system products that own the page scale and active STEMS profile.
    pub fn from_native_products(
        plans: &NativeStemsBeamLinkPlanSystem,
        vlinkers: &NativeStemsBeamVLinkerSystem,
        profile: i32,
    ) -> Result<Self, NativeStemsBeamVLinkReuseCheckError> {
        if plans.system_id != vlinkers.system_id
            || plans.interline <= 0
            || plans.interline != vlinkers.interline
            || vlinkers.main_stem_thickness <= 0
            || !(0..=4).contains(&profile)
        {
            return Err(NativeStemsBeamVLinkReuseCheckError::InvalidParameters);
        }
        Ok(Self {
            interline: plans.interline,
            main_stem_thickness: f64::from(vlinkers.main_stem_thickness),
            profile,
            x_in_gap_maximum_profile0: 0.5,
            x_out_gap_maximum: 0.2,
            y_gap_maximum: 4.0,
            x_weight: 1.0,
            y_weight: 4.0,
            intrinsic_ratio: 1.0,
            minimum_grade: 0.1,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkReuseCheck {
    pub system_id: usize,
    pub invocation_ordinal: usize,
    pub plan: NativeStemsBeamPlanRef,
    pub initial_stem: Option<NativeStemsBeamKnownSystemStem>,
    pub final_stem: Option<NativeStemsBeamKnownSystemStem>,
    pub reuse_disposition: NativeStemsBeamReuseDisposition,
    pub reuse_trace: Vec<NativeStemsBeamReuseTrace>,
    pub relation_check: Option<NativeStemsBeamRelationCheck>,
    pub outcome: NativeStemsBeamVLinkReuseCheckOutcome,
    pub persistent_id_mutation_count: usize,
    pub system_stem_mutation_count: usize,
    pub sig_vertex_mutation_count: usize,
    pub sig_relation_mutation_count: usize,
    pub linker_flag_mutation_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamReuseDisposition {
    NotEvaluatedCreateStemRejected,
    AllUnlinked,
    NoUnique,
    Selected {
        map_ordinal: usize,
        stem_identity: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamReuseTrace {
    pub map_ordinal: usize,
    pub corner: NativeStemsBeamHeadCornerRef,
    pub parent_horizontal_side: NativeStemHeadSide,
    pub plan_relation_head_side: NativeStemHeadSide,
    pub s_linker_linked: Option<bool>,
    pub lookup: NativeStemsBeamHeadStemLookupTrace,
    pub action: NativeStemsBeamReuseAction,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamHeadStemLookupTrace {
    NotRead,
    Present {
        incident_relation_count: usize,
        matching_side_edge_count: usize,
        distinct_stem_identities: Vec<usize>,
    },
    UnreadAfterSelection,
    UnreadCreateStemRejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamReuseAction {
    UnreadCreateStemRejected,
    SkipUnlinked,
    ContinueMultiplicity,
    SelectUnique { stem_identity: usize },
    UnreadAfterSelection,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamRelationCheck {
    pub beam: NativeStemsBeamSource,
    pub v_linker: NativeStemsBeamVLinkerRef,
    pub vertical_side: NativeStemVerticalSide,
    pub head_to_beam: NativeStemVerticalSide,
    pub beam_median: Segment,
    pub beam_height: f64,
    pub beam_border: Segment,
    pub stem_identity: usize,
    pub stem_inter_id: Option<i32>,
    pub stem_median: NativeStemLine,
    pub interline: i32,
    pub main_stem_thickness: f64,
    pub profile: i32,
    pub x_in_gap_maximum_profile0: f64,
    pub portion_maximum_dx: i32,
    pub x_out_gap_maximum: f64,
    pub y_gap_maximum: f64,
    pub x_weight: f64,
    pub y_weight: f64,
    pub intrinsic_ratio: f64,
    pub minimum_grade: f64,
    pub cross: NativeStemPoint,
    pub portion: NativeBeamPortion,
    pub signed_x_gap_pixels: f64,
    pub x_gap_pixels: f64,
    pub y_gap_pixels: f64,
    /// Stored `AbstractConnection.dx`, after `setOutGaps` takes `abs`.
    pub dx: f64,
    pub dy: f64,
    pub raw_x_impact: f64,
    pub raw_y_impact: f64,
    pub x_impact: f64,
    pub y_impact: f64,
    pub grade: f64,
    pub extension_point: Option<NativeStemPoint>,
    pub accepted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamRelationDraft {
    pub beam: NativeStemsBeamSource,
    pub partner_stem_identity: usize,
    pub partner_inter_id: Option<i32>,
    pub beam_portion: NativeBeamPortion,
    pub dx: f64,
    pub dy: f64,
    pub x_impact: f64,
    pub y_impact: f64,
    pub grade: f64,
    pub extension_point: NativeStemPoint,
    pub outgoing: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamVLinkReuseCheckOutcome {
    CreateStemRejected,
    BeamStemRejected,
    ReadyBeforeSigMutation {
        relation: NativeStemsBeamRelationDraft,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum NativeStemsBeamVLinkReuseCheckError {
    SystemOrder,
    InvalidProduct {
        system_id: usize,
        phase: &'static str,
    },
    InvalidParameters,
    TransactionStateInvariant {
        phase: &'static str,
    },
    InvalidLiveState {
        map_ordinal: usize,
        phase: &'static str,
    },
    MissingStem {
        stem_identity: usize,
    },
    /// Java would dereference `head.getSideStems().get(parentSide)` and throw
    /// because no map entry exists for the queried side.
    JavaNullHeadSideStemSet {
        map_ordinal: usize,
        corner: NativeStemsBeamHeadCornerRef,
    },
}

impl fmt::Display for NativeStemsBeamVLinkReuseCheckError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid beam VLink reuse/checkLink boundary: {self:?}"
        )
    }
}

impl Error for NativeStemsBeamVLinkReuseCheckError {}

/// Project B13's lazy live reads from the owned graph and persistent S cells.
///
/// Java reads the shared S flag first and skips `getSideStems` when it is
/// false. For a linked cell, the native SIG supplies Java's incoming-then-
/// outgoing relation order, while the carried `systemStems` map supplies the
/// persistent StemInter payloads. Targets are interned into the returned live
/// catalogue in first-reference order, exactly like the Java probe.
pub fn project_native_stems_beam_vlink_reuse_live_state(
    sig: &NativeSigSystem,
    bindings: &NativeSigSystemBindings,
    scheduler_system: &NativeStemsBeamSchedulerSystem,
    plan_system: &NativeStemsBeamLinkPlanSystem,
    s_cells: &[NativeStemsBeamNativeSLinkerCell],
    system_stems: &NativeStemsBeamSystemStemTransactionState,
) -> Result<NativeStemsBeamVLinkReuseLiveState, NativeStemsBeamVLinkReuseCheckError> {
    let system_id = scheduler_system.system_id;
    if sig.system_id != system_id
        || bindings.system_id != system_id
        || plan_system.system_id != system_id
        || system_stems.system_id != system_id
        || bindings.validate_against(sig).is_err()
    {
        return Err(NativeStemsBeamVLinkReuseCheckError::SystemOrder);
    }
    for (index, cell) in s_cells.iter().enumerate() {
        if cell.closed
            || s_cells[..index]
                .iter()
                .any(|prior| prior.reference == cell.reference)
            || cell.ordered_observer_corners.len() != 2
            || cell.ordered_observer_corners[0].head != cell.reference.head.reference
            || cell.ordered_observer_corners[1].head != cell.reference.head.reference
            || cell.ordered_observer_corners[0].horizontal != cell.reference.horizontal
            || cell.ordered_observer_corners[1].horizontal != cell.reference.horizontal
            || cell.ordered_observer_corners[0].vertical
                != crate::stems_step::NativeStemVerticalSide::Top
            || cell.ordered_observer_corners[1].vertical
                != crate::stems_step::NativeStemVerticalSide::Bottom
        {
            return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                map_ordinal: 0,
                phase: "native S-cell catalogue",
            });
        }
    }
    let frontier = match &scheduler_system.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => {
            return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
                system_id,
                phase: "native reuse scheduler frontier",
            });
        }
    };
    let (attempt, builder_start) = resolve_attempt(plan_system, frontier.plan)?;
    if builder_start != frontier.v_linker {
        return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
            system_id,
            phase: "native reuse plan/V join",
        });
    }
    let mut entries = Vec::with_capacity(attempt.relations.len());
    let mut live_sig_stems = Vec::new();
    let mut selected = false;
    for (map_ordinal, relation) in attempt.relations.iter().enumerate() {
        if relation.map_ordinal != map_ordinal {
            return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
                system_id,
                phase: "native reuse relation order",
            });
        }
        if selected {
            entries.push(NativeStemsBeamReuseEntryEvidence {
                map_ordinal,
                corner: relation.corner,
                observation: NativeStemsBeamReuseEntryObservation::UnreadAfterSelection,
            });
            continue;
        }
        let head = NativeStemsBeamHeadLinkHeadRef {
            reference: relation.corner.head,
            sig_ordinal: relation.corner.sig_ordinal,
            x_ordinal: relation.corner.x_ordinal,
        };
        let head_vertex = bindings
            .head_vertices
            .get(&relation.corner.head)
            .copied()
            .ok_or(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                map_ordinal,
                phase: "native head binding",
            })?;
        if sig
            .vertex(head_vertex.0)
            .is_none_or(|vertex| vertex.kind != NativeSigInterKind::Head)
        {
            return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                map_ordinal,
                phase: "native live head vertex",
            });
        }
        let reference = NativeStemsBeamHeadSLinkerRef {
            head,
            horizontal: relation.corner.horizontal,
        };
        let matches = s_cells
            .iter()
            .filter(|cell| cell.reference == reference)
            .collect::<Vec<_>>();
        let [cell] = matches.as_slice() else {
            return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                map_ordinal,
                phase: "native S-cell cardinality",
            });
        };
        let head_stem_lookup = if cell.linked {
            let mut edges = Vec::new();
            for edge in sig
                .incident_edges(head_vertex.0)
                .map_err(|_| NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                    map_ordinal,
                    phase: "native HeadStem incident scan",
                })?
                .into_iter()
                .filter(|edge| edge.kind == NativeSigRelationKind::HeadStem)
            {
                if edge.source != head_vertex.0 {
                    return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                        map_ordinal,
                        phase: "native HeadStem direction",
                    });
                }
                let stem_identity = bindings
                    .stem_vertices
                    .iter()
                    .find_map(|(identity, vertex)| (vertex.0 == edge.target).then_some(*identity))
                    .ok_or(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                        map_ordinal,
                        phase: "native HeadStem target binding",
                    })?;
                let stem = system_stems
                    .known_stems
                    .iter()
                    .find(|stem| stem.stem_identity == stem_identity)
                    .ok_or(NativeStemsBeamVLinkReuseCheckError::MissingStem { stem_identity })?;
                if !stem.sig_attached || stem.inter_id.is_none() {
                    return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                        map_ordinal,
                        phase: "native HeadStem target persistence",
                    });
                }
                if !live_sig_stems
                    .iter()
                    .any(|known: &NativeStemsBeamKnownSystemStem| {
                        known.stem_identity == stem_identity
                    })
                {
                    live_sig_stems.push(stem.clone());
                }
                let payload = edge.head_stem.ok_or(
                    NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                        map_ordinal,
                        phase: "native HeadStem payload",
                    },
                )?;
                edges.push(NativeStemsBeamHeadStemEdge {
                    scan_ordinal: edges.len(),
                    relation_identity: edge.ordinal,
                    relation_head_side: payload.head_side,
                    target_stem_identity: stem_identity,
                });
            }
            let mut scan = NativeStemsBeamHeadStemScan {
                corner: relation.corner,
                baseline_relation_count: edges.len(),
                scanned_relation_count: edges.len(),
                provenance_sha256: String::new(),
                edges,
            };
            let (snapshot, projection) = head_stem_scan_hashes(
                &scan,
                relation.corner.horizontal,
                &live_sig_stems,
                map_ordinal,
            )?;
            scan.provenance_sha256 = snapshot;
            if !valid_sha256(&projection) {
                return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                    map_ordinal,
                    phase: "Java HeadStem projection hash",
                });
            }
            let distinct = scan
                .edges
                .iter()
                .filter(|edge| edge.relation_head_side == relation.corner.horizontal)
                .map(|edge| edge.target_stem_identity)
                .fold(Vec::new(), |mut values, identity| {
                    if !values.contains(&identity) {
                        values.push(identity);
                    }
                    values
                });
            selected = distinct.len() == 1;
            NativeStemsBeamHeadStemLookupEvidence::Exhaustive(scan)
        } else {
            NativeStemsBeamHeadStemLookupEvidence::NotRead
        };
        entries.push(NativeStemsBeamReuseEntryEvidence {
            map_ordinal,
            corner: relation.corner,
            observation: NativeStemsBeamReuseEntryObservation::Examined {
                s_linker_linked: cell.linked,
                head_stem_lookup,
            },
        });
    }
    Ok(NativeStemsBeamVLinkReuseLiveState {
        system_id,
        live_sig_stems,
        evaluation: NativeStemsBeamVLinkReuseLiveEvaluation::Entries(entries),
    })
}

/// Evaluate the read-only continuation after one committed create-stem
/// transaction.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_native_stems_beam_vlink_reuse_check(
    scheduler_system: &NativeStemsBeamSchedulerSystem,
    plan_system: &NativeStemsBeamLinkPlanSystem,
    stump_system: &NativeStemsBeamStumpSystem,
    vlinker_system: &NativeStemsBeamVLinkerSystem,
    transaction: &NativeStemsBeamVLinkTransaction,
    transaction_state: &NativeStemsBeamVLinkTransactionState,
    live_state: &NativeStemsBeamVLinkReuseLiveState,
    parameters: NativeStemsBeamRelationParameters,
) -> Result<NativeStemsBeamVLinkReuseCheck, NativeStemsBeamVLinkReuseCheckError> {
    let system_id = scheduler_system.system_id;
    if plan_system.system_id != system_id
        || stump_system.system_id != system_id
        || vlinker_system.system_id != system_id
        || transaction.system_id != system_id
        || transaction_state.system_stems.system_id != system_id
        || live_state.system_id != system_id
        || plan_system.interline != stump_system.interline
        || plan_system.interline != vlinker_system.interline
        || stump_system.profile != vlinker_system.profile
        || scheduler_system.link_profile != plan_system.link_profile
    {
        return Err(NativeStemsBeamVLinkReuseCheckError::SystemOrder);
    }
    validate_parameters(parameters, plan_system, vlinker_system, transaction.plan)?;

    let frontier = match &scheduler_system.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => {
            return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
                system_id,
                phase: "scheduler frontier",
            });
        }
    };
    if frontier.plan != transaction.plan
        || frontier.invocation_ordinal != transaction.invocation_ordinal
        || frontier.outcome != NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem
        || frontier.vertical_side != frontier.v_linker.side
        || transaction.applied_line_delta != frontier.would_apply_stored_line_delta
    {
        return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
            system_id,
            phase: "transaction/frontier join",
        });
    }
    let (attempt, builder_start) = resolve_attempt(plan_system, transaction.plan)?;
    if builder_start != frontier.v_linker {
        return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
            system_id,
            phase: "plan builder start",
        });
    }
    validate_attempt(
        attempt,
        transaction.plan,
        plan_system.link_profile,
        system_id,
    )?;
    validate_vlinker(vlinker_system, frontier.v_linker, frontier.beam, system_id)?;
    let beam = resolve_beam(stump_system, frontier.beam, system_id)?;
    validate_beam(beam, system_id)?;
    validate_create_stem_state(transaction_state).map_err(|_| {
        NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
            phase: "shared createStem state validator",
        }
    })?;
    validate_transaction_candidate(attempt, transaction, transaction_state)?;
    validate_committed_line_state(
        scheduler_system,
        frontier.v_linker,
        attempt,
        transaction,
        transaction_state,
    )?;
    let initial_stem = validate_committed_transaction_state(transaction, transaction_state)?;

    if initial_stem.is_none() {
        if !live_state.live_sig_stems.is_empty() {
            return Err(
                NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                    phase: "rejected createStem live stem catalogue",
                },
            );
        }
        if live_state.evaluation
            != NativeStemsBeamVLinkReuseLiveEvaluation::NotReadCreateStemRejected
        {
            return Err(
                NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                    phase: "rejected createStem live reads",
                },
            );
        }
        let reuse_trace = attempt
            .relations
            .iter()
            .map(|relation| NativeStemsBeamReuseTrace {
                map_ordinal: relation.map_ordinal,
                corner: relation.corner,
                parent_horizontal_side: relation.corner.horizontal,
                plan_relation_head_side: relation.check.derived_horizontal,
                s_linker_linked: None,
                lookup: NativeStemsBeamHeadStemLookupTrace::UnreadCreateStemRejected,
                action: NativeStemsBeamReuseAction::UnreadCreateStemRejected,
            })
            .collect();
        return Ok(NativeStemsBeamVLinkReuseCheck {
            system_id,
            invocation_ordinal: transaction.invocation_ordinal,
            plan: transaction.plan,
            initial_stem: None,
            final_stem: None,
            reuse_disposition: NativeStemsBeamReuseDisposition::NotEvaluatedCreateStemRejected,
            reuse_trace,
            relation_check: None,
            outcome: NativeStemsBeamVLinkReuseCheckOutcome::CreateStemRejected,
            persistent_id_mutation_count: 0,
            system_stem_mutation_count: 0,
            sig_vertex_mutation_count: 0,
            sig_relation_mutation_count: 0,
            linker_flag_mutation_count: 0,
        });
    }

    let entries = match &live_state.evaluation {
        NativeStemsBeamVLinkReuseLiveEvaluation::Entries(entries) => entries,
        NativeStemsBeamVLinkReuseLiveEvaluation::NotReadCreateStemRejected => {
            return Err(
                NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                    phase: "accepted createStem missing reuse reads",
                },
            );
        }
    };
    if entries.len() != attempt.relations.len() {
        return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
            system_id,
            phase: "reuse entry cardinality",
        });
    }
    validate_live_sig_stems(&live_state.live_sig_stems, transaction_state)?;

    let initial_stem = initial_stem.expect("non-rejected transaction was validated with a stem");
    let (final_stem, reuse_disposition, reuse_trace) = evaluate_reuse(
        &attempt.relations,
        entries,
        &live_state.live_sig_stems,
        initial_stem.clone(),
    )?;
    let relation_check = evaluate_relation(
        beam,
        frontier.v_linker,
        frontier.vertical_side,
        &final_stem,
        parameters,
    );
    let outcome = if relation_check.accepted {
        let extension_point = relation_check
            .extension_point
            .expect("accepted relation check has an extension point");
        NativeStemsBeamVLinkReuseCheckOutcome::ReadyBeforeSigMutation {
            relation: NativeStemsBeamRelationDraft {
                beam: frontier.beam,
                partner_stem_identity: final_stem.stem_identity,
                partner_inter_id: final_stem.inter_id,
                beam_portion: relation_check.portion,
                dx: relation_check.dx,
                dy: relation_check.dy,
                x_impact: relation_check.x_impact,
                y_impact: relation_check.y_impact,
                grade: relation_check.grade,
                extension_point,
                outgoing: true,
            },
        }
    } else {
        NativeStemsBeamVLinkReuseCheckOutcome::BeamStemRejected
    };

    Ok(NativeStemsBeamVLinkReuseCheck {
        system_id,
        invocation_ordinal: transaction.invocation_ordinal,
        plan: transaction.plan,
        initial_stem: Some(initial_stem),
        final_stem: Some(final_stem),
        reuse_disposition,
        reuse_trace,
        relation_check: Some(relation_check),
        outcome,
        persistent_id_mutation_count: 0,
        system_stem_mutation_count: 0,
        sig_vertex_mutation_count: 0,
        sig_relation_mutation_count: 0,
        linker_flag_mutation_count: 0,
    })
}

fn validate_parameters(
    parameters: NativeStemsBeamRelationParameters,
    plan_system: &NativeStemsBeamLinkPlanSystem,
    vlinker_system: &NativeStemsBeamVLinkerSystem,
    plan: NativeStemsBeamPlanRef,
) -> Result<(), NativeStemsBeamVLinkReuseCheckError> {
    let finite = [
        parameters.main_stem_thickness,
        parameters.x_in_gap_maximum_profile0,
        parameters.x_out_gap_maximum,
        parameters.y_gap_maximum,
        parameters.x_weight,
        parameters.y_weight,
        parameters.intrinsic_ratio,
        parameters.minimum_grade,
    ]
    .into_iter()
    .all(f64::is_finite);
    if parameters.interline <= 0
        || parameters.interline != plan_system.interline
        || parameters.interline != vlinker_system.interline
        || parameters.main_stem_thickness <= 0.0
        || parameters.main_stem_thickness != f64::from(vlinker_system.main_stem_thickness)
        || parameters.profile != plan.stem_profile
        || !(0..=4).contains(&parameters.profile)
        || !finite
        || parameters.x_in_gap_maximum_profile0 <= 0.0
        || parameters.x_out_gap_maximum <= 0.0
        || parameters.y_gap_maximum <= 0.0
        || parameters.x_weight < 0.0
        || parameters.y_weight < 0.0
        || !(parameters.x_weight + parameters.y_weight).is_finite()
        || parameters.x_weight + parameters.y_weight <= 0.0
        || parameters.intrinsic_ratio != 1.0
        || !(0.0..=1.0).contains(&parameters.minimum_grade)
    {
        return Err(NativeStemsBeamVLinkReuseCheckError::InvalidParameters);
    }
    let rounded =
        (f64::from(parameters.interline) * parameters.x_in_gap_maximum_profile0).round_ties_even();
    if rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
        return Err(NativeStemsBeamVLinkReuseCheckError::InvalidParameters);
    }
    Ok(())
}

fn resolve_attempt(
    plan_system: &NativeStemsBeamLinkPlanSystem,
    reference: NativeStemsBeamPlanRef,
) -> Result<
    (&NativeStemsBeamLinkPlanAttempt, NativeStemsBeamVLinkerRef),
    NativeStemsBeamVLinkReuseCheckError,
> {
    let mut plan_ordinal = 0_usize;
    for (builder_ordinal, builder) in plan_system.builders.iter().enumerate() {
        if builder.builder_ordinal != builder_ordinal {
            return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
                system_id: plan_system.system_id,
                phase: "plan builder order",
            });
        }
        for attempt in &builder.attempts {
            if plan_ordinal == reference.plan_ordinal {
                if builder_ordinal != reference.builder_ordinal {
                    return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
                        system_id: plan_system.system_id,
                        phase: "flattened plan ordinal",
                    });
                }
                return Ok((attempt, builder.start));
            }
            plan_ordinal += 1;
        }
    }
    Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
        system_id: plan_system.system_id,
        phase: "missing flattened plan",
    })
}

fn validate_attempt(
    attempt: &NativeStemsBeamLinkPlanAttempt,
    plan: NativeStemsBeamPlanRef,
    link_profile: i32,
    system_id: usize,
) -> Result<(), NativeStemsBeamVLinkReuseCheckError> {
    if plan.system_id != system_id
        || plan.stem_profile != attempt.stem_profile
        || attempt.link_profile != link_profile
        || attempt.outcome != NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem
        || attempt.glyphs.is_empty()
        || attempt.relations.is_empty()
        || attempt.registry_mutation_count != 0
        || attempt.sig_mutation_count != 0
        || attempt.system_stem_mutation_count != 0
        || attempt.link_flag_mutation_count != 0
    {
        return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
            system_id,
            phase: "ready plan payload",
        });
    }
    for (map_ordinal, relation) in attempt.relations.iter().enumerate() {
        if relation.map_ordinal != map_ordinal
            || relation.first_item_index > relation.latest_item_index
            || relation.latest_item_index >= attempt.trace.len()
            || relation.check.encountered_corner != relation.corner
            || relation.check.encountered_horizontal != relation.corner.horizontal
            || !relation.check.accepted
            || attempt.relations[..map_ordinal]
                .iter()
                .any(|prior| prior.corner == relation.corner)
            || attempt.relations[..map_ordinal].iter().any(|prior| {
                prior.corner.head == relation.corner.head
                    && (prior.corner.sig_ordinal != relation.corner.sig_ordinal
                        || prior.corner.x_ordinal != relation.corner.x_ordinal)
            })
        {
            return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
                system_id,
                phase: "LinkedHashMap relation order",
            });
        }
    }
    Ok(())
}

fn validate_transaction_candidate(
    attempt: &NativeStemsBeamLinkPlanAttempt,
    transaction: &NativeStemsBeamVLinkTransaction,
    state: &NativeStemsBeamVLinkTransactionState,
) -> Result<(), NativeStemsBeamVLinkReuseCheckError> {
    let mut selected = Vec::with_capacity(attempt.glyphs.len());
    for glyph in &attempt.glyphs {
        let matches = state
            .selected_glyph_bindings
            .iter()
            .filter(|binding| binding.reference == glyph.reference)
            .collect::<Vec<_>>();
        let [binding] = matches.as_slice() else {
            return Err(
                NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                    phase: "selected plan glyph binding cardinality",
                },
            );
        };
        if binding.content.bounds != glyph.bounds
            || binding.content.weight != glyph.weight
            || binding.content.bounds.x != glyph.structural_key.left
            || binding.content.bounds.y != glyph.structural_key.top
            || binding.content.run_table != glyph.structural_key.run_table
        {
            return Err(
                NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                    phase: "selected plan glyph binding payload",
                },
            );
        }
        selected.push(*binding);
    }
    let (candidate, glyph_id_before_registration) = if selected.len() == 1 {
        (selected[0].content.clone(), Some(selected[0].glyph_id))
    } else {
        (
            build_compound(&selected).map_err(|_| {
                NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                    phase: "compound plan candidate",
                }
            })?,
            None,
        )
    };
    if transaction.candidate != candidate
        || transaction.candidate_glyph_id_before_registration != glyph_id_before_registration
    {
        return Err(
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "plan candidate transaction join",
            },
        );
    }
    Ok(())
}

fn validate_committed_line_state(
    scheduler: &NativeStemsBeamSchedulerSystem,
    current_v_linker: NativeStemsBeamVLinkerRef,
    attempt: &NativeStemsBeamLinkPlanAttempt,
    transaction: &NativeStemsBeamVLinkTransaction,
    state: &NativeStemsBeamVLinkTransactionState,
) -> Result<(), NativeStemsBeamVLinkReuseCheckError> {
    let system_id = scheduler.system_id;
    if attempt.stored_theoretical_line_would_mutate
        != (attempt.stored_theoretical_line_before != attempt.stored_theoretical_line_after)
    {
        return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
            system_id,
            phase: "ready line mutation semantics",
        });
    }
    let mut previous_invocation = None;
    let mut seen_v_linkers = Vec::new();
    for (ordinal, delta) in scheduler.deferred_line_deltas.iter().enumerate() {
        if delta.delta_ordinal != ordinal
            || delta.invocation_ordinal >= transaction.invocation_ordinal
            || previous_invocation.is_some_and(|previous| delta.invocation_ordinal <= previous)
            || delta.plan.system_id != system_id
            || delta.before == delta.after
            || seen_v_linkers.contains(&delta.v_linker)
        {
            return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
                system_id,
                phase: "known-false line delta order",
            });
        }
        previous_invocation = Some(delta.invocation_ordinal);
        seen_v_linkers.push(delta.v_linker);
    }
    if seen_v_linkers.contains(&current_v_linker) {
        return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
            system_id,
            phase: "repeated current V line delta",
        });
    }

    let suffix_start = scheduler
        .deferred_line_deltas
        .len()
        .checked_sub(transaction.applied_prefix_line_deltas.len())
        .ok_or(
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "committed line prefix cardinality",
            },
        )?;
    if scheduler.deferred_line_deltas[suffix_start..] != transaction.applied_prefix_line_deltas {
        return Err(
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "committed line prefix suffix",
            },
        );
    }
    let expected_ledger_len = scheduler.deferred_line_deltas.len()
        + usize::from(transaction.applied_line_delta.is_some());
    if state.applied_line_deltas.len() != expected_ledger_len {
        return Err(
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "committed line ledger cardinality",
            },
        );
    }
    for (ordinal, expected) in scheduler.deferred_line_deltas.iter().enumerate() {
        let applied = &state.applied_line_deltas[ordinal];
        if applied.system_id != system_id
            || applied.source
                != (NativeStemsBeamAppliedLineDeltaSource::KnownFalsePrefix {
                    delta_ordinal: ordinal,
                })
            || applied.delta != *expected
        {
            return Err(
                NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                    phase: "committed known-false line ledger",
                },
            );
        }
        validate_line_after_commit(state, expected)?;
    }
    if let Some(delta) = &transaction.applied_line_delta {
        if delta.delta_ordinal != scheduler.deferred_line_deltas.len()
            || delta.invocation_ordinal != transaction.invocation_ordinal
            || delta.plan != transaction.plan
            || delta.v_linker != current_v_linker
            || delta.before != attempt.stored_theoretical_line_before
            || delta.after != attempt.stored_theoretical_line_after
            || delta.builder_line_aliases != attempt.builder_line_aliases_stored_theoretical_line
            || delta.attachment_aliases != attempt.attachment_aliases_stored_theoretical_line
            || state.applied_line_deltas.last().is_none_or(|applied| {
                applied.system_id != system_id
                    || applied.source != NativeStemsBeamAppliedLineDeltaSource::ReadyTransaction
                    || applied.delta != *delta
            })
        {
            return Err(
                NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                    phase: "committed ready line ledger",
                },
            );
        }
        validate_line_after_commit(state, delta)?;
    } else if attempt.stored_theoretical_line_would_mutate
        || attempt.stored_theoretical_line_before != attempt.stored_theoretical_line_after
    {
        return Err(
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "missing committed ready line delta",
            },
        );
    }
    if transaction.applied_line_delta.is_some() != attempt.stored_theoretical_line_would_mutate {
        return Err(
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "ready line mutation presence",
            },
        );
    }
    let current_lines = state
        .line_states
        .iter()
        .filter(|line| line.v_linker == current_v_linker)
        .collect::<Vec<_>>();
    let [current_line] = current_lines.as_slice() else {
        return Err(
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "current committed V line cardinality",
            },
        );
    };
    let expected_stored = transaction
        .applied_line_delta
        .as_ref()
        .map_or(attempt.stored_theoretical_line_before, |delta| delta.after);
    let expected_builder = if transaction
        .applied_line_delta
        .as_ref()
        .is_some_and(|delta| delta.builder_line_aliases)
    {
        expected_stored
    } else {
        attempt.initial_stem_line
    };
    if current_line.stored_theoretical_line != expected_stored
        || current_line.builder_line != expected_builder
        || (attempt.attachment_aliases_stored_theoretical_line
            && current_line.current_attachment_line != Some(expected_stored))
    {
        return Err(
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "current committed V line state",
            },
        );
    }
    Ok(())
}

fn validate_line_after_commit(
    state: &NativeStemsBeamVLinkTransactionState,
    delta: &crate::native_stems_beam_scheduler::NativeStemsBeamDeferredLineDelta,
) -> Result<(), NativeStemsBeamVLinkReuseCheckError> {
    let matches = state
        .line_states
        .iter()
        .filter(|line| line.v_linker == delta.v_linker)
        .collect::<Vec<_>>();
    let [line] = matches.as_slice() else {
        return Err(
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "committed V line state cardinality",
            },
        );
    };
    if line.stored_theoretical_line != delta.after
        || (delta.builder_line_aliases && line.builder_line != delta.after)
        || (delta.attachment_aliases && line.current_attachment_line != Some(delta.after))
    {
        return Err(
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "committed V line state",
            },
        );
    }
    Ok(())
}

fn validate_vlinker(
    system: &NativeStemsBeamVLinkerSystem,
    reference: NativeStemsBeamVLinkerRef,
    beam: NativeStemsBeamSource,
    system_id: usize,
) -> Result<(), NativeStemsBeamVLinkReuseCheckError> {
    if reference.b_linker.beam != beam {
        return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
            system_id,
            phase: "VLinker beam identity",
        });
    }
    let mut matches = 0_usize;
    for constructor in &system.constructors {
        for b_linker in &constructor.b_linkers {
            for v_linker in &b_linker.v_linkers {
                if v_linker.reference == reference {
                    if constructor.source != beam
                        || b_linker.reference != reference.b_linker
                        || v_linker.y_direction
                            != match reference.side {
                                NativeStemVerticalSide::Top => -1,
                                NativeStemVerticalSide::Bottom => 1,
                            }
                    {
                        return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
                            system_id,
                            phase: "VLinker ownership",
                        });
                    }
                    matches += 1;
                }
            }
        }
    }
    if matches != 1 {
        return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
            system_id,
            phase: "VLinker identity",
        });
    }
    Ok(())
}

fn resolve_beam(
    system: &NativeStemsBeamStumpSystem,
    source: NativeStemsBeamSource,
    system_id: usize,
) -> Result<&NativeStemsBeamStumpBeam, NativeStemsBeamVLinkReuseCheckError> {
    let mut matches = system
        .beams_by_abscissa
        .iter()
        .filter(|beam| beam.source == source);
    let beam = matches
        .next()
        .ok_or(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
            system_id,
            phase: "missing starting beam",
        })?;
    if matches.next().is_some() {
        return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
            system_id,
            phase: "duplicate starting beam",
        });
    }
    Ok(beam)
}

fn validate_beam(
    beam: &NativeStemsBeamStumpBeam,
    system_id: usize,
) -> Result<(), NativeStemsBeamVLinkReuseCheckError> {
    if ![
        beam.median.x1,
        beam.median.y1,
        beam.median.x2,
        beam.median.y2,
        beam.height,
    ]
    .into_iter()
    .all(f64::is_finite)
        || beam.median.x1 >= beam.median.x2
        || beam.height <= 0.0
    {
        return Err(NativeStemsBeamVLinkReuseCheckError::InvalidProduct {
            system_id,
            phase: "starting beam geometry",
        });
    }
    Ok(())
}

fn validate_committed_transaction_state(
    transaction: &NativeStemsBeamVLinkTransaction,
    state: &NativeStemsBeamVLinkTransactionState,
) -> Result<Option<NativeStemsBeamKnownSystemStem>, NativeStemsBeamVLinkReuseCheckError> {
    let scope_matches_system = match transaction.scope {
        crate::native_stems_beam_vlink_transaction::NativeStemsBeamVLinkTransactionScope::SharedSheetFirstFrontier {
            system_id,
        } => system_id == transaction.system_id && system_id == 1,
        crate::native_stems_beam_vlink_transaction::NativeStemsBeamVLinkTransactionScope::SharedSheetSerial {
            system_id,
        } => system_id == transaction.system_id,
        crate::native_stems_beam_vlink_transaction::NativeStemsBeamVLinkTransactionScope::IsolatedFreshSheetFrontier {
            system_id,
        } => system_id == transaction.system_id,
    };
    if transaction.scope != state.scope
        || !scope_matches_system
        || transaction.sig_vertex_mutation_count != 0
        || transaction.sig_relation_mutation_count != 0
        || transaction.linker_flag_mutation_count != 0
        || transaction.registration.glyph_id <= 0
        || transaction.registration.glyph_id
            != i32::try_from(transaction.registration.canonical_alias).unwrap_or(i32::MIN)
    {
        return Err(
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "createStem transaction header",
            },
        );
    }
    validate_checker_result_payload(transaction.disposition, transaction.checker_result.as_ref())?;
    let canonical_matches = state
        .glyph_index
        .known_canonical_glyphs
        .iter()
        .filter(|known| {
            known.glyph_id == transaction.registration.glyph_id
                && known.canonical_alias == transaction.registration.canonical_alias
                && known.content == transaction.candidate
                && known.active_in_index
        })
        .count();
    if canonical_matches != 1 {
        return Err(
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "committed canonical Glyph",
            },
        );
    }
    if transaction.registration.alias_order != state.glyph_index.alias_order
        || transaction.registration.post_union_size != state.glyph_index.union_size
    {
        return Err(
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "committed registration state",
            },
        );
    }
    match transaction.registration.action {
        NativeStemsBeamGlyphRegistrationAction::Registered
            if transaction.candidate_glyph_id_before_registration.is_some()
                || state.glyph_index.persistent_ids.sheet_last_id
                    != transaction.registration.glyph_id =>
        {
            return Err(
                NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                    phase: "registered persistent ID",
                },
            );
        }
        NativeStemsBeamGlyphRegistrationAction::Reused { .. }
            if state.glyph_index.persistent_ids.sheet_last_id
                < transaction.registration.glyph_id =>
        {
            return Err(
                NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                    phase: "reused persistent ID",
                },
            );
        }
        _ => {}
    }
    for (index, stem) in state.system_stems.known_stems.iter().enumerate() {
        if stem.stem_identity >= state.system_stems.next_stem_identity
            || ![
                stem.geometry.median.start.x,
                stem.geometry.median.start.y,
                stem.geometry.median.stop.x,
                stem.geometry.median.stop.y,
                stem.geometry.mean_thickness,
            ]
            .into_iter()
            .all(f64::is_finite)
            || stem.geometry.median.start.y >= stem.geometry.median.stop.y
            || stem.geometry.mean_thickness <= 0.0
            || stem.sig_attached != stem.inter_id.is_some()
            || state.system_stems.known_stems[..index].iter().any(|other| {
                other.stem_identity == stem.stem_identity
                    || other.glyph_content == stem.glyph_content
                    || (other.inter_id.is_some() && other.inter_id == stem.inter_id)
            })
        {
            return Err(
                NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                    phase: "known system stem catalogue",
                },
            );
        }
    }
    let disposition_identity = match transaction.disposition {
        NativeStemsBeamCreateStemDisposition::Reused { stem_identity }
        | NativeStemsBeamCreateStemDisposition::CreatedChecked { stem_identity }
        | NativeStemsBeamCreateStemDisposition::CreatedArtificial { stem_identity } => {
            Some(stem_identity)
        }
        NativeStemsBeamCreateStemDisposition::Rejected => None,
    };
    let canonical = state
        .glyph_index
        .known_canonical_glyphs
        .iter()
        .find(|known| {
            known.glyph_id == transaction.registration.glyph_id
                && known.content == transaction.candidate
        })
        .expect("unique committed canonical Glyph was counted above");
    let rejected = matches!(
        transaction.disposition,
        NativeStemsBeamCreateStemDisposition::Rejected
    );
    if matches!(
        transaction.registration.action,
        NativeStemsBeamGlyphRegistrationAction::Registered
    ) && canonical.strongly_retained == rejected
    {
        return Err(
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "registered canonical retention",
            },
        );
    }
    let mut expected_mutations = vec![
        NativeStemsBeamVLinkMutation::StoredLineDeltaApplied;
        transaction.applied_prefix_line_deltas.len()
            + usize::from(transaction.applied_line_delta.is_some())
    ];
    match transaction.registration.action {
        NativeStemsBeamGlyphRegistrationAction::Registered => {
            expected_mutations.push(NativeStemsBeamVLinkMutation::GlyphRegistered {
                glyph_id: transaction.registration.glyph_id,
            });
        }
        NativeStemsBeamGlyphRegistrationAction::Reused {
            reinserted_into_active_index: true,
        } => {
            expected_mutations.push(NativeStemsBeamVLinkMutation::GlyphReinserted {
                glyph_id: transaction.registration.glyph_id,
            });
        }
        NativeStemsBeamGlyphRegistrationAction::Reused {
            reinserted_into_active_index: false,
        } => {}
    }
    if !canonical.strongly_retained {
        expected_mutations.push(
            NativeStemsBeamVLinkMutation::RegistryWeakLivenessBecameUnknown {
                glyph_id: transaction.registration.glyph_id,
            },
        );
    }
    if let NativeStemsBeamCreateStemDisposition::CreatedChecked { stem_identity }
    | NativeStemsBeamCreateStemDisposition::CreatedArtificial { stem_identity } =
        transaction.disposition
    {
        expected_mutations.push(NativeStemsBeamVLinkMutation::SystemStemInserted { stem_identity });
    }
    if transaction.mutation_order != expected_mutations {
        return Err(
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "createStem mutation order",
            },
        );
    }

    match (disposition_identity, &transaction.stem) {
        (None, None) => {
            if transaction.checker_result.is_none() || transaction.plan.stem_profile == 4 {
                return Err(
                    NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                        phase: "rejected checker result",
                    },
                );
            }
            if state
                .system_stems
                .known_stems
                .iter()
                .any(|stem| stem.glyph_content == transaction.candidate)
            {
                return Err(
                    NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                        phase: "rejected systemStems key",
                    },
                );
            }
            Ok(None)
        }
        (Some(stem_identity), Some(stem))
            if stem.stem_identity == stem_identity
                && stem.glyph_id == transaction.registration.glyph_id
                && stem.glyph_content == transaction.candidate =>
        {
            let result_matches = match (&transaction.disposition, &stem.grade) {
                (
                    NativeStemsBeamCreateStemDisposition::Reused { .. },
                    NativeStemsBeamStemGrade::Checked(_) | NativeStemsBeamStemGrade::Artificial(_),
                ) => transaction.checker_result.is_none(),
                (
                    NativeStemsBeamCreateStemDisposition::CreatedChecked { .. },
                    NativeStemsBeamStemGrade::Checked(check),
                ) => transaction.checker_result.as_ref() == Some(check),
                (
                    NativeStemsBeamCreateStemDisposition::CreatedArtificial { .. },
                    NativeStemsBeamStemGrade::Artificial(_),
                ) => transaction.checker_result.is_some() && transaction.plan.stem_profile == 4,
                _ => false,
            };
            let created_payload_matches = matches!(
                transaction.disposition,
                NativeStemsBeamCreateStemDisposition::Reused { .. }
            ) || (stem.inter_id.is_none()
                && !stem.sig_attached
                && !stem.abnormal
                && state.system_stems.next_stem_identity
                    == stem_identity.checked_add(1).ok_or(
                        NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                            phase: "created stem identity overflow",
                        },
                    )?);
            if !result_matches || !created_payload_matches {
                return Err(
                    NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                        phase: "createStem result semantics",
                    },
                );
            }
            let state_matches = state
                .system_stems
                .known_stems
                .iter()
                .filter(|known| *known == stem)
                .count();
            if state_matches != 1 {
                return Err(
                    NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                        phase: "transaction/systemStems exact stem",
                    },
                );
            }
            Ok(Some(stem.clone()))
        }
        _ => Err(
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "createStem disposition payload",
            },
        ),
    }
}

fn validate_checker_result_payload(
    disposition: NativeStemsBeamCreateStemDisposition,
    checker_result: Option<&NativeStemCheckResult>,
) -> Result<(), NativeStemsBeamVLinkReuseCheckError> {
    let result_presence_matches = match disposition {
        NativeStemsBeamCreateStemDisposition::Reused { .. } => checker_result.is_none(),
        NativeStemsBeamCreateStemDisposition::CreatedChecked { .. }
        | NativeStemsBeamCreateStemDisposition::CreatedArtificial { .. }
        | NativeStemsBeamCreateStemDisposition::Rejected => checker_result.is_some(),
    };
    if !result_presence_matches
        || checker_result
            .is_some_and(|check| !valid_stem_grade(&NativeStemsBeamStemGrade::Checked(*check)))
    {
        return Err(
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "createStem checker result payload",
            },
        );
    }
    Ok(())
}

fn validate_live_sig_stems(
    live_stems: &[NativeStemsBeamKnownSystemStem],
    state: &NativeStemsBeamVLinkTransactionState,
) -> Result<(), NativeStemsBeamVLinkReuseCheckError> {
    let last_id = state.glyph_index.persistent_ids.sheet_last_id;
    let mut represented_glyphs = Vec::new();
    let mut represented_state_stems = state.system_stems.known_stems.iter().collect::<Vec<_>>();
    for glyph in &state.glyph_index.known_canonical_glyphs {
        represented_glyphs.push((glyph.glyph_id, &glyph.content));
    }
    for binding in &state.selected_glyph_bindings {
        represented_glyphs.push((binding.glyph_id, &binding.content));
    }
    if let Some(scan) = &state.glyph_index.exhaustive_lookup
        && let crate::native_stems_beam_vlink_transaction::NativeStemsBeamExhaustiveGlyphLookup::Present {
            glyph_id,
            ..
        } = scan.lookup
    {
        represented_glyphs.push((glyph_id, &scan.candidate));
    }
    for stem in &state.system_stems.known_stems {
        represented_glyphs.push((stem.glyph_id, &stem.glyph_content));
    }
    if let Some(scan) = &state.system_stems.exhaustive_lookup
        && let crate::native_stems_beam_vlink_transaction::NativeStemsBeamExhaustiveSystemStemLookup::Present(
            stem,
        ) = &scan.lookup
    {
        represented_glyphs.push((stem.glyph_id, &stem.glyph_content));
        represented_state_stems.push(stem);
    }

    for (index, stem) in live_stems.iter().enumerate() {
        validate_known_stem(stem, last_id).map_err(|_| {
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "live SIG stem payload",
            }
        })?;
        let Some(inter_id) = stem.inter_id else {
            return Err(
                NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                    phase: "live SIG stem Inter identity",
                },
            );
        };
        if !stem.sig_attached
            || inter_id <= 0
            || live_stems[..index].iter().any(|prior| {
                prior.stem_identity == stem.stem_identity || prior.inter_id == stem.inter_id
            })
        {
            return Err(
                NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                    phase: "unique live SIG stem identities",
                },
            );
        }
        for state_stem in &represented_state_stems {
            let same_dense_identity = state_stem.stem_identity == stem.stem_identity;
            let same_inter_id = state_stem.inter_id == stem.inter_id;
            if (same_dense_identity || same_inter_id) && *state_stem != stem {
                return Err(
                    NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                        phase: "live/systemStems object identity",
                    },
                );
            }
        }
        if represented_glyphs.iter().any(|(glyph_id, content)| {
            let same_id = *glyph_id == stem.glyph_id;
            let same_content = *content == &stem.glyph_content;
            same_id != same_content
        }) || live_stems[..index].iter().any(|prior| {
            let same_id = prior.glyph_id == stem.glyph_id;
            let same_content = prior.glyph_content == stem.glyph_content;
            same_id != same_content
        }) {
            return Err(
                NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                    phase: "live canonical Glyph identity",
                },
            );
        }
        if represented_glyphs
            .iter()
            .any(|(glyph_id, _)| *glyph_id == inter_id)
            || live_stems.iter().any(|other| other.glyph_id == inter_id)
            || represented_state_stems.iter().any(|state_stem| {
                state_stem
                    .inter_id
                    .is_some_and(|state_inter_id| state_inter_id == stem.glyph_id)
            })
        {
            return Err(
                NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                    phase: "live persistent ID domains",
                },
            );
        }
    }
    Ok(())
}

fn evaluate_reuse(
    relations: &[NativeStemsBeamHeadRelation],
    entries: &[NativeStemsBeamReuseEntryEvidence],
    stem_catalogue: &[NativeStemsBeamKnownSystemStem],
    initial_stem: NativeStemsBeamKnownSystemStem,
) -> Result<
    (
        NativeStemsBeamKnownSystemStem,
        NativeStemsBeamReuseDisposition,
        Vec<NativeStemsBeamReuseTrace>,
    ),
    NativeStemsBeamVLinkReuseCheckError,
> {
    let mut selected = None;
    let mut trace = Vec::with_capacity(relations.len());
    let mut shared_flags = Vec::new();
    let mut shared_head_scans: Vec<&NativeStemsBeamHeadStemScan> = Vec::new();
    let mut global_relation_identities: Vec<(
        NativeStemsBeamHeadCornerRef,
        NativeStemsBeamHeadStemEdge,
    )> = Vec::new();
    let mut referenced_stem_identities = Vec::new();
    let mut every_examined_unlinked = true;
    for (relation, evidence) in relations.iter().zip(entries) {
        if evidence.map_ordinal != relation.map_ordinal || evidence.corner != relation.corner {
            return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                map_ordinal: relation.map_ordinal,
                phase: "relation evidence identity",
            });
        }
        if selected.is_some() {
            if evidence.observation != NativeStemsBeamReuseEntryObservation::UnreadAfterSelection {
                return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                    map_ordinal: relation.map_ordinal,
                    phase: "evidence after unique break",
                });
            }
            trace.push(reuse_trace(
                relation,
                None,
                NativeStemsBeamHeadStemLookupTrace::UnreadAfterSelection,
                NativeStemsBeamReuseAction::UnreadAfterSelection,
            ));
            continue;
        }
        let NativeStemsBeamReuseEntryObservation::Examined {
            s_linker_linked,
            head_stem_lookup,
        } = &evidence.observation
        else {
            return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                map_ordinal: relation.map_ordinal,
                phase: "unread entry before selection",
            });
        };
        let shared_key = (relation.corner.head, relation.corner.horizontal);
        if let Some((_, _, prior)) = shared_flags
            .iter()
            .find(|(head, side, _)| (*head, *side) == shared_key)
            && prior != s_linker_linked
        {
            return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                map_ordinal: relation.map_ordinal,
                phase: "shared S-linker flag",
            });
        }
        shared_flags.push((shared_key.0, shared_key.1, *s_linker_linked));
        if !s_linker_linked {
            if !matches!(
                head_stem_lookup,
                NativeStemsBeamHeadStemLookupEvidence::NotRead
            ) {
                return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                    map_ordinal: relation.map_ordinal,
                    phase: "unlinked graph read",
                });
            }
            trace.push(reuse_trace(
                relation,
                Some(false),
                NativeStemsBeamHeadStemLookupTrace::NotRead,
                NativeStemsBeamReuseAction::SkipUnlinked,
            ));
            continue;
        }
        every_examined_unlinked = false;
        let NativeStemsBeamHeadStemLookupEvidence::Exhaustive(scan) = head_stem_lookup else {
            return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                map_ordinal: relation.map_ordinal,
                phase: "linked graph scan",
            });
        };
        if let Some(prior) = shared_head_scans
            .iter()
            .find(|prior| prior.corner.head == scan.corner.head)
        {
            if prior.baseline_relation_count != scan.baseline_relation_count
                || prior.scanned_relation_count != scan.scanned_relation_count
                || prior.provenance_sha256 != scan.provenance_sha256
                || prior.edges != scan.edges
            {
                return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                    map_ordinal: relation.map_ordinal,
                    phase: "shared HeadInter graph snapshot",
                });
            }
        } else {
            shared_head_scans.push(scan);
        }
        for edge in &scan.edges {
            if let Some((prior_corner, prior_edge)) = global_relation_identities
                .iter()
                .find(|(_, prior)| prior.relation_identity == edge.relation_identity)
            {
                if prior_corner.head != scan.corner.head || prior_edge != edge {
                    return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                        map_ordinal: relation.map_ordinal,
                        phase: "global HeadStem relation identity",
                    });
                }
            } else {
                global_relation_identities.push((scan.corner, *edge));
            }
        }
        let (matching_side_edge_count, distinct) =
            validate_head_stem_scan(scan, relation, stem_catalogue)?;
        for edge in &scan.edges {
            if !referenced_stem_identities.contains(&edge.target_stem_identity) {
                if stem_catalogue
                    .get(referenced_stem_identities.len())
                    .is_none_or(|stem| stem.stem_identity != edge.target_stem_identity)
                {
                    return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                        map_ordinal: relation.map_ordinal,
                        phase: "ordered live SIG stem catalogue",
                    });
                }
                referenced_stem_identities.push(edge.target_stem_identity);
            }
        }
        if distinct.is_empty() {
            if stem_catalogue.len() != referenced_stem_identities.len() {
                return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                    map_ordinal: relation.map_ordinal,
                    phase: "ordered live SIG stem catalogue",
                });
            }
            return Err(
                NativeStemsBeamVLinkReuseCheckError::JavaNullHeadSideStemSet {
                    map_ordinal: relation.map_ordinal,
                    corner: relation.corner,
                },
            );
        }
        let lookup = NativeStemsBeamHeadStemLookupTrace::Present {
            incident_relation_count: scan.edges.len(),
            matching_side_edge_count,
            distinct_stem_identities: distinct.clone(),
        };
        if distinct.len() == 1 {
            let stem_identity = distinct[0];
            let stem = stem_catalogue
                .iter()
                .find(|stem| stem.stem_identity == stem_identity)
                .ok_or(NativeStemsBeamVLinkReuseCheckError::MissingStem { stem_identity })?
                .clone();
            selected = Some((relation.map_ordinal, stem));
            trace.push(reuse_trace(
                relation,
                Some(true),
                lookup,
                NativeStemsBeamReuseAction::SelectUnique { stem_identity },
            ));
        } else {
            trace.push(reuse_trace(
                relation,
                Some(true),
                lookup,
                NativeStemsBeamReuseAction::ContinueMultiplicity,
            ));
        }
    }
    if stem_catalogue.len() != referenced_stem_identities.len()
        || stem_catalogue
            .iter()
            .zip(&referenced_stem_identities)
            .any(|(stem, referenced)| stem.stem_identity != *referenced)
    {
        return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
            map_ordinal: relations.len(),
            phase: "ordered live SIG stem catalogue",
        });
    }
    if let Some((map_ordinal, stem)) = selected {
        let identity = stem.stem_identity;
        Ok((
            stem,
            NativeStemsBeamReuseDisposition::Selected {
                map_ordinal,
                stem_identity: identity,
            },
            trace,
        ))
    } else if every_examined_unlinked {
        Ok((
            initial_stem,
            NativeStemsBeamReuseDisposition::AllUnlinked,
            trace,
        ))
    } else {
        Ok((
            initial_stem,
            NativeStemsBeamReuseDisposition::NoUnique,
            trace,
        ))
    }
}

fn reuse_trace(
    relation: &NativeStemsBeamHeadRelation,
    s_linker_linked: Option<bool>,
    lookup: NativeStemsBeamHeadStemLookupTrace,
    action: NativeStemsBeamReuseAction,
) -> NativeStemsBeamReuseTrace {
    NativeStemsBeamReuseTrace {
        map_ordinal: relation.map_ordinal,
        corner: relation.corner,
        parent_horizontal_side: relation.corner.horizontal,
        plan_relation_head_side: relation.check.derived_horizontal,
        s_linker_linked,
        lookup,
        action,
    }
}

fn validate_head_stem_scan(
    scan: &NativeStemsBeamHeadStemScan,
    relation: &NativeStemsBeamHeadRelation,
    stem_catalogue: &[NativeStemsBeamKnownSystemStem],
) -> Result<(usize, Vec<usize>), NativeStemsBeamVLinkReuseCheckError> {
    if scan.corner != relation.corner
        || scan.baseline_relation_count != scan.scanned_relation_count
        || scan.scanned_relation_count != scan.edges.len()
        || !valid_sha256(&scan.provenance_sha256)
    {
        return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
            map_ordinal: relation.map_ordinal,
            phase: "exhaustive HeadStem scan coverage",
        });
    }
    let mut distinct = Vec::new();
    let mut matching_side_edge_count = 0_usize;
    for (scan_ordinal, edge) in scan.edges.iter().enumerate() {
        if edge.scan_ordinal != scan_ordinal
            || scan.edges[..scan_ordinal]
                .iter()
                .any(|prior| prior.relation_identity == edge.relation_identity)
        {
            return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                map_ordinal: relation.map_ordinal,
                phase: "ordered HeadStem edge identities",
            });
        }
        let target = stem_catalogue
            .iter()
            .find(|stem| stem.stem_identity == edge.target_stem_identity)
            .ok_or(NativeStemsBeamVLinkReuseCheckError::MissingStem {
                stem_identity: edge.target_stem_identity,
            })?;
        if !target.sig_attached || target.inter_id.is_none_or(|id| id <= 0) {
            return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                map_ordinal: relation.map_ordinal,
                phase: "HeadStem target SIG identity",
            });
        }
        if edge.relation_head_side == relation.corner.horizontal {
            matching_side_edge_count += 1;
            if !distinct.contains(&edge.target_stem_identity) {
                distinct.push(edge.target_stem_identity);
            }
        }
    }
    let (snapshot_sha256, _) = head_stem_scan_hashes(
        scan,
        relation.corner.horizontal,
        stem_catalogue,
        relation.map_ordinal,
    )?;
    if scan.provenance_sha256 != snapshot_sha256 {
        return Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
            map_ordinal: relation.map_ordinal,
            phase: "Java HeadStem snapshot hash",
        });
    }
    Ok((matching_side_edge_count, distinct))
}

fn head_stem_scan_hashes(
    scan: &NativeStemsBeamHeadStemScan,
    parent_side: NativeStemHeadSide,
    stem_catalogue: &[NativeStemsBeamKnownSystemStem],
    map_ordinal: usize,
) -> Result<(String, String), NativeStemsBeamVLinkReuseCheckError> {
    let side_token = |side| match side {
        NativeStemHeadSide::Left => "LEFT",
        NativeStemHeadSide::Right => "RIGHT",
    };
    let mut seen = Vec::new();
    let mut snapshot_bytes = Vec::new();
    let mut projection_bytes = Vec::new();
    for edge in &scan.edges {
        let stem_ordinal = stem_catalogue
            .iter()
            .position(|stem| stem.stem_identity == edge.target_stem_identity)
            .ok_or(NativeStemsBeamVLinkReuseCheckError::MissingStem {
                stem_identity: edge.target_stem_identity,
            })?;
        let stem = &stem_catalogue[stem_ordinal];
        let inter_id =
            stem.inter_id
                .ok_or(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                    map_ordinal,
                    phase: "Java HeadStem projection persistent ID",
                })?;
        let distinct = !seen.contains(&(edge.relation_head_side, edge.target_stem_identity));
        if distinct {
            seen.push((edge.relation_head_side, edge.target_stem_identity));
        }
        let matches = edge.relation_head_side == parent_side;
        let snapshot = format!(
            "{}:{}:live:{}:{}:sig=true:glyph={}:distinct={}",
            edge.relation_identity,
            side_token(edge.relation_head_side),
            stem_ordinal,
            inter_id,
            stem.glyph_id,
            distinct
        );
        snapshot_bytes.extend_from_slice(snapshot.as_bytes());
        snapshot_bytes.push(b'\n');
        projection_bytes.extend_from_slice(
            format!(
                "{snapshot}:parent={}:matches={}\n",
                side_token(parent_side),
                matches
            )
            .as_bytes(),
        );
    }
    Ok((sha256_hex(&snapshot_bytes), sha256_hex(&projection_bytes)))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn evaluate_relation(
    beam: &NativeStemsBeamStumpBeam,
    v_linker: NativeStemsBeamVLinkerRef,
    vertical_side: NativeStemVerticalSide,
    stem: &NativeStemsBeamKnownSystemStem,
    parameters: NativeStemsBeamRelationParameters,
) -> NativeStemsBeamRelationCheck {
    let border = beam_border(beam, vertical_side);
    evaluate_relation_geometry(
        beam.source,
        beam.median,
        beam.height,
        border,
        v_linker,
        vertical_side,
        stem,
        parameters,
    )
}

#[allow(clippy::too_many_arguments)]
fn evaluate_relation_geometry(
    beam_source: NativeStemsBeamSource,
    beam_median: Segment,
    beam_height: f64,
    border: Segment,
    v_linker: NativeStemsBeamVLinkerRef,
    vertical_side: NativeStemVerticalSide,
    stem: &NativeStemsBeamKnownSystemStem,
    parameters: NativeStemsBeamRelationParameters,
) -> NativeStemsBeamRelationCheck {
    let head_to_beam = opposite_vertical(vertical_side);
    let stem_segment = stem_segment(stem.geometry.median);
    let cross = generic_intersection(stem_segment, border);
    let rounded =
        (f64::from(parameters.interline) * parameters.x_in_gap_maximum_profile0).round_ties_even();
    let portion_maximum_dx = rounded as i32;
    let portion = if cross.x < beam_median.x1 + f64::from(portion_maximum_dx) {
        NativeBeamPortion::Left
    } else if cross.x > beam_median.x2 - f64::from(portion_maximum_dx) {
        NativeBeamPortion::Right
    } else {
        NativeBeamPortion::Center
    };
    let half_stem_width = parameters.main_stem_thickness / 2.0;
    let signed_x_gap_pixels = match portion {
        NativeBeamPortion::Center => 0.0,
        NativeBeamPortion::Left => border.x1 + half_stem_width - cross.x,
        NativeBeamPortion::Right => cross.x - border.x2 + half_stem_width,
    };
    let x_gap_pixels = signed_x_gap_pixels.abs();
    let y_gap_pixels = java_max(
        java_max(0.0, stem.geometry.median.start.y - cross.y),
        java_max(0.0, cross.y - stem.geometry.median.stop.y),
    );
    // Java first converts the signed pixel gap through Scale.pixelsToFrac,
    // then AbstractConnection.setOutGaps applies Math.abs.
    let signed_dx = signed_x_gap_pixels / f64::from(parameters.interline);
    let dx = signed_dx.abs();
    let dy = y_gap_pixels / f64::from(parameters.interline);
    let raw_x_impact = (parameters.x_out_gap_maximum - dx) / parameters.x_out_gap_maximum;
    let raw_y_impact = (parameters.y_gap_maximum - dy) / parameters.y_gap_maximum;
    let x_impact = java_clamp(raw_x_impact);
    let y_impact = java_clamp(raw_y_impact);
    let grade = weighted_grade(
        [x_impact, y_impact],
        [parameters.x_weight, parameters.y_weight],
        parameters.intrinsic_ratio,
    );
    let accepted = grade >= parameters.minimum_grade;
    let extension_point = accepted.then_some(NativeStemPoint {
        x: cross.x,
        y: cross.y + (f64::from(vertical_direction(head_to_beam)) * (beam_height - 1.0)),
    });
    NativeStemsBeamRelationCheck {
        beam: beam_source,
        v_linker,
        vertical_side,
        head_to_beam,
        beam_median,
        beam_height,
        beam_border: border,
        stem_identity: stem.stem_identity,
        stem_inter_id: stem.inter_id,
        stem_median: stem.geometry.median,
        interline: parameters.interline,
        main_stem_thickness: parameters.main_stem_thickness,
        profile: parameters.profile,
        x_in_gap_maximum_profile0: parameters.x_in_gap_maximum_profile0,
        portion_maximum_dx,
        x_out_gap_maximum: parameters.x_out_gap_maximum,
        y_gap_maximum: parameters.y_gap_maximum,
        x_weight: parameters.x_weight,
        y_weight: parameters.y_weight,
        intrinsic_ratio: parameters.intrinsic_ratio,
        minimum_grade: parameters.minimum_grade,
        cross,
        portion,
        signed_x_gap_pixels,
        x_gap_pixels,
        y_gap_pixels,
        dx,
        dy,
        raw_x_impact,
        raw_y_impact,
        x_impact,
        y_impact,
        grade,
        extension_point,
        accepted,
    }
}

const fn opposite_vertical(side: NativeStemVerticalSide) -> NativeStemVerticalSide {
    match side {
        NativeStemVerticalSide::Top => NativeStemVerticalSide::Bottom,
        NativeStemVerticalSide::Bottom => NativeStemVerticalSide::Top,
    }
}

const fn vertical_direction(side: NativeStemVerticalSide) -> i32 {
    match side {
        NativeStemVerticalSide::Top => -1,
        NativeStemVerticalSide::Bottom => 1,
    }
}

const fn stem_segment(line: NativeStemLine) -> Segment {
    Segment {
        x1: line.start.x,
        y1: line.start.y,
        x2: line.stop.x,
        y2: line.stop.y,
    }
}

fn java_max(one: f64, two: f64) -> f64 {
    if one.is_nan() {
        one
    } else if two.is_nan() || (one == 0.0 && two == 0.0 && one.is_sign_negative()) {
        two
    } else if one >= two {
        one
    } else {
        two
    }
}

fn java_clamp(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn weighted_grade(impacts: [f64; 2], weights: [f64; 2], intrinsic_ratio: f64) -> f64 {
    let mut global = 1.0_f64;
    let mut total_weight = 0.0_f64;
    for (impact, weight) in impacts.into_iter().zip(weights) {
        total_weight += weight;
        if impact == 0.0 {
            global = 0.0;
        } else if weight != 0.0 {
            let weighted_impact = if impact.is_nan() {
                impact
            } else {
                java_positive_pow(impact, weight)
            };
            global *= weighted_impact;
        }
    }
    let mean = if global.is_nan() {
        global
    } else {
        java_positive_pow(global, 1.0 / total_weight)
    };
    intrinsic_ratio * mean
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiveris_image::{
        run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable},
        section::Bounds,
    };

    use crate::{
        head_scanner_slices::VerticalRibbonArea,
        native_heads_staff_epilog::NativeHeadStaffEpilogRef,
        native_stems_beam_link_plans::{
            NativeStemsBeamHeadRelationCheck, NativeStemsBeamHorizontalGapKind,
        },
        native_stems_beam_vlink_transaction::{
            NativeStemsBeamCreatedStemGeometry, NativeStemsBeamFixedGlyphContent,
            NativeStemsBeamGlyphAliasOrder, NativeStemsBeamGlyphIndexTransactionState,
            NativeStemsBeamPersistentIdState, NativeStemsBeamRegistryAuthority,
            NativeStemsBeamSystemStemTransactionState, NativeStemsBeamVLinkTransactionScope,
        },
        native_stems_beam_vlinkers::NativeStemsBeamBLinkerRef,
        stem_seeds_step::{NativeStemCounts, NativeStemImpacts},
    };

    fn v_linker() -> NativeStemsBeamVLinkerRef {
        NativeStemsBeamVLinkerRef {
            b_linker: NativeStemsBeamBLinkerRef {
                beam: NativeStemsBeamSource::RawBeam(0),
                id: 1,
            },
            side: NativeStemVerticalSide::Bottom,
        }
    }

    fn parameters() -> NativeStemsBeamRelationParameters {
        NativeStemsBeamRelationParameters {
            interline: 10,
            main_stem_thickness: 4.0,
            profile: 0,
            x_in_gap_maximum_profile0: 0.5,
            x_out_gap_maximum: 0.15,
            y_gap_maximum: 0.8,
            x_weight: 1.0,
            y_weight: 4.0,
            intrinsic_ratio: 1.0,
            minimum_grade: 0.1,
        }
    }

    fn content(x: usize) -> NativeStemsBeamFixedGlyphContent {
        let mut pixels = vec![BACKGROUND; 6];
        pixels.fill(FOREGROUND);
        let run_table =
            RunTable::from_pixels(Orientation::Vertical, 1, 6, &pixels).expect("valid run table");
        NativeStemsBeamFixedGlyphContent {
            bounds: Bounds {
                x,
                y: 0,
                width: 1,
                height: 6,
            },
            weight: run_table.weight(),
            run_table,
        }
    }

    fn known_stem(
        stem_identity: usize,
        inter_id: Option<i32>,
        sig_attached: bool,
        median: NativeStemLine,
        mean_thickness: f64,
    ) -> NativeStemsBeamKnownSystemStem {
        let ribbon_bounds = VerticalRibbonArea::new(
            Segment {
                x1: median.start.x,
                y1: median.start.y,
                x2: median.stop.x,
                y2: median.stop.y,
            },
            mean_thickness,
        )
        .integer_bounds();
        NativeStemsBeamKnownSystemStem {
            stem_identity,
            glyph_id: i32::try_from(stem_identity + 1).expect("small test identity"),
            glyph_content: content(stem_identity),
            inter_id,
            grade: NativeStemsBeamStemGrade::Artificial(0.5),
            geometry: NativeStemsBeamCreatedStemGeometry {
                median,
                mean_thickness,
                ribbon_bounds,
            },
            sig_attached,
            abnormal: false,
        }
    }

    fn vertical_line(x: f64) -> NativeStemLine {
        NativeStemLine {
            start: NativeStemPoint { x, y: 10.0 },
            stop: NativeStemPoint { x, y: 30.0 },
        }
    }

    fn empty_committed_state(last_id: i32) -> NativeStemsBeamVLinkTransactionState {
        NativeStemsBeamVLinkTransactionState {
            scope: NativeStemsBeamVLinkTransactionScope::SharedSheetFirstFrontier { system_id: 1 },
            glyph_index: NativeStemsBeamGlyphIndexTransactionState {
                persistent_ids: NativeStemsBeamPersistentIdState {
                    sheet_last_id: last_id,
                    glyph_index_last_id: last_id,
                    inter_index_last_id: last_id,
                },
                alias_order: NativeStemsBeamGlyphAliasOrder::JavaGlyphId,
                union_size: 0,
                known_canonical_glyphs: Vec::new(),
                exhaustive_lookup: None,
            },
            selected_glyph_bindings: Vec::new(),
            line_states: Vec::new(),
            applied_line_deltas: Vec::new(),
            system_stems: NativeStemsBeamSystemStemTransactionState {
                system_id: 1,
                next_stem_identity: 0,
                // Hydrated from Java rows: keep the scan requirement.
                authority: NativeStemsBeamRegistryAuthority::RequiresExhaustiveScan,
                known_stems: Vec::new(),
                exhaustive_lookup: None,
            },
        }
    }

    fn relation_check(corner: NativeStemsBeamHeadCornerRef) -> NativeStemsBeamHeadRelationCheck {
        NativeStemsBeamHeadRelationCheck {
            encountered_corner: corner,
            encountered_horizontal: corner.horizontal,
            derived_horizontal: corner.horizontal,
            horizontal_side_diverges: false,
            vertical: corner.vertical,
            head_center: (0, 0),
            reference_point: NativeStemPoint { x: 0.0, y: 0.0 },
            stump_bounds: None,
            x_stem: 0.0,
            x_gap_pixels: 0.0,
            y_gap_pixels: 0.0,
            dx: 0.0,
            dy: 0.0,
            horizontal_gap_kind: NativeStemsBeamHorizontalGapKind::Out,
            x_maximum: 1.0,
            y_maximum: 1.0,
            x_weight: 1.0,
            y_weight: 1.0,
            raw_x_impact: 1.0,
            raw_y_impact: 1.0,
            x_impact: 1.0,
            y_impact: 1.0,
            grade: 1.0,
            extension_point: Some(NativeStemPoint { x: 0.0, y: 0.0 }),
            accepted: true,
        }
    }

    fn corner(
        head_index: usize,
        x_ordinal: usize,
        horizontal: NativeStemHeadSide,
    ) -> NativeStemsBeamHeadCornerRef {
        NativeStemsBeamHeadCornerRef {
            head: NativeHeadStaffEpilogRef {
                staff_index: 0,
                head_index,
            },
            sig_ordinal: head_index,
            x_ordinal,
            horizontal,
            vertical: NativeStemVerticalSide::Bottom,
        }
    }

    fn relation(
        map_ordinal: usize,
        corner: NativeStemsBeamHeadCornerRef,
    ) -> NativeStemsBeamHeadRelation {
        NativeStemsBeamHeadRelation {
            corner,
            map_ordinal,
            first_item_index: map_ordinal,
            latest_item_index: map_ordinal,
            replaced_existing_payload: false,
            check: relation_check(corner),
        }
    }

    fn scan(
        corner: NativeStemsBeamHeadCornerRef,
        edges: Vec<NativeStemsBeamHeadStemEdge>,
        catalogue: &[NativeStemsBeamKnownSystemStem],
    ) -> NativeStemsBeamHeadStemScan {
        let mut scan = NativeStemsBeamHeadStemScan {
            corner,
            baseline_relation_count: edges.len(),
            scanned_relation_count: edges.len(),
            provenance_sha256: String::new(),
            edges,
        };
        scan.provenance_sha256 = head_stem_scan_hashes(&scan, corner.horizontal, catalogue, 0)
            .expect("valid test HeadStem projection")
            .0;
        scan
    }

    fn edge(
        scan_ordinal: usize,
        relation_identity: usize,
        relation_head_side: NativeStemHeadSide,
        target_stem_identity: usize,
    ) -> NativeStemsBeamHeadStemEdge {
        NativeStemsBeamHeadStemEdge {
            scan_ordinal,
            relation_identity,
            relation_head_side,
            target_stem_identity,
        }
    }

    fn valid_checker_result() -> NativeStemCheckResult {
        NativeStemCheckResult {
            values: NativeStemImpacts {
                slope: 0.0,
                straight: 0.0,
                length: 0.0,
                clean: 0.0,
                black: 0.0,
                black_ratio: 0.0,
                gap: 0.0,
            },
            impacts: NativeStemImpacts {
                slope: 0.5,
                straight: 0.5,
                length: 0.5,
                clean: 0.5,
                black: 0.5,
                black_ratio: 0.5,
                gap: 0.5,
            },
            counts: NativeStemCounts {
                largest_gap: 0,
                white: 0,
                black: 0,
                left: 0,
                right: 0,
                both: 0,
                clean: 0,
            },
            grade: 0.5,
        }
    }

    #[test]
    fn java_head_stem_projection_hash_matches_measured_allegretto_transaction_28() {
        let corner = corner(2, 2, NativeStemHeadSide::Right);
        let mut stem = known_stem(0, Some(2227), true, vertical_line(603.0), 3.0);
        stem.glyph_id = 266;
        let scan = NativeStemsBeamHeadStemScan {
            corner,
            baseline_relation_count: 1,
            scanned_relation_count: 1,
            provenance_sha256: String::new(),
            edges: vec![edge(0, 229, NativeStemHeadSide::Right, 0)],
        };
        let (snapshot, _) = head_stem_scan_hashes(
            &scan,
            NativeStemHeadSide::Right,
            std::slice::from_ref(&stem),
            0,
        )
        .expect("measured graph row is projectable");
        let projection = scan
            .java_projection_sha256(NativeStemHeadSide::Right, &[stem])
            .expect("measured Java projection is projectable");
        assert_eq!(
            snapshot,
            "08b72a351a5ad443cbadb12f040dbb74e42ff6c031ef796f4dc563b502279a63"
        );
        assert_eq!(
            projection,
            "46502fb158aa90d31c7594ec55686d4a2d1e796eebf55b0c7dfdc63f223abff6"
        );
    }

    #[test]
    fn malformed_rejected_and_artificial_checker_payloads_fail_closed() {
        let valid = valid_checker_result();
        let dispositions = [
            NativeStemsBeamCreateStemDisposition::Rejected,
            NativeStemsBeamCreateStemDisposition::CreatedArtificial { stem_identity: 0 },
        ];
        for disposition in dispositions {
            assert!(validate_checker_result_payload(disposition, Some(&valid)).is_ok());
        }

        let mut nan_grade = valid;
        nan_grade.grade = f64::NAN;
        let mut impossible_counts = valid;
        impossible_counts.counts.black = 1;
        let mut nan_impact = valid;
        nan_impact.impacts.gap = f64::NAN;
        for invalid in [nan_grade, impossible_counts, nan_impact] {
            for disposition in dispositions {
                assert!(matches!(
                    validate_checker_result_payload(disposition, Some(&invalid)),
                    Err(
                        NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                            phase: "createStem checker result payload"
                        }
                    )
                ));
            }
        }
    }

    #[test]
    fn relation_uses_scale_stem_width_and_accepts_minimum_inclusively() {
        let stem = known_stem(0, None, false, vertical_line(1.0), 50.0);
        let median = Segment {
            x1: 0.0,
            y1: 20.0,
            x2: 100.0,
            y2: 20.0,
        };
        let border = Segment {
            x1: 0.0,
            y1: 22.0,
            x2: 100.0,
            y2: 22.0,
        };
        let accepted = evaluate_relation_geometry(
            NativeStemsBeamSource::RawBeam(0),
            median,
            4.0,
            border,
            v_linker(),
            NativeStemVerticalSide::Bottom,
            &stem,
            parameters(),
        );
        assert_eq!(accepted.portion, NativeBeamPortion::Left);
        assert_eq!(accepted.signed_x_gap_pixels, 1.0);
        assert_eq!(accepted.dx, 0.1);
        assert!(accepted.accepted);
        assert!(accepted.extension_point.is_some());

        let at_minimum = evaluate_relation_geometry(
            NativeStemsBeamSource::RawBeam(0),
            median,
            4.0,
            border,
            v_linker(),
            NativeStemVerticalSide::Bottom,
            &stem,
            NativeStemsBeamRelationParameters {
                minimum_grade: accepted.grade,
                ..parameters()
            },
        );
        assert!(at_minimum.accepted);
        let above_grade = f64::from_bits(accepted.grade.to_bits() + 1);
        let rejected = evaluate_relation_geometry(
            NativeStemsBeamSource::RawBeam(0),
            median,
            4.0,
            border,
            v_linker(),
            NativeStemVerticalSide::Bottom,
            &stem,
            NativeStemsBeamRelationParameters {
                minimum_grade: above_grade,
                ..parameters()
            },
        );
        assert!(!rejected.accepted);
        assert!(rejected.extension_point.is_none());
    }

    #[test]
    fn portion_thresholds_are_strict_and_rint_is_ties_even() {
        let median = Segment {
            x1: 0.0,
            y1: 20.0,
            x2: 100.0,
            y2: 20.0,
        };
        let border = Segment {
            x1: 0.0,
            y1: 22.0,
            x2: 100.0,
            y2: 22.0,
        };
        let check = |x| {
            evaluate_relation_geometry(
                NativeStemsBeamSource::RawBeam(0),
                median,
                4.0,
                border,
                v_linker(),
                NativeStemVerticalSide::Bottom,
                &known_stem(0, None, false, vertical_line(x), 1.0),
                parameters(),
            )
        };
        assert_eq!(check(5.0).portion_maximum_dx, 5);
        assert_eq!(check(5.0).portion, NativeBeamPortion::Center);
        assert_eq!(
            check(f64::from_bits(5.0_f64.to_bits() - 1)).portion,
            NativeBeamPortion::Left
        );
        assert_eq!(check(95.0).portion, NativeBeamPortion::Center);
        assert_eq!(
            check(f64::from_bits(95.0_f64.to_bits() + 1)).portion,
            NativeBeamPortion::Right
        );

        let mut ties_even = parameters();
        ties_even.interline = 5;
        assert_eq!(
            evaluate_relation_geometry(
                NativeStemsBeamSource::RawBeam(0),
                median,
                4.0,
                border,
                v_linker(),
                NativeStemVerticalSide::Bottom,
                &known_stem(0, None, false, vertical_line(10.0), 1.0),
                ties_even,
            )
            .portion_maximum_dx,
            2
        );
    }

    #[test]
    fn parallel_intersection_preserves_coordinate_nonfinites_and_rejects() {
        let stem = known_stem(
            0,
            None,
            false,
            NativeStemLine {
                start: NativeStemPoint { x: 10.0, y: 10.0 },
                stop: NativeStemPoint { x: 30.0, y: 10.0 },
            },
            1.0,
        );
        let horizontal_beam = Segment {
            x1: 10.0,
            y1: 22.0,
            x2: 30.0,
            y2: 22.0,
        };
        let result = evaluate_relation_geometry(
            NativeStemsBeamSource::RawBeam(0),
            horizontal_beam,
            4.0,
            horizontal_beam,
            v_linker(),
            NativeStemVerticalSide::Bottom,
            &stem,
            parameters(),
        );
        assert!(result.cross.x.is_infinite());
        assert!(result.cross.x.is_sign_negative());
        assert!(result.cross.y.is_nan());
        assert!(result.grade.is_nan());
        assert!(!result.accepted);
        assert!(result.extension_point.is_none());
    }

    #[test]
    fn linked_scan_continues_on_multiple_selects_unique_and_leaves_catalogue_unchanged() {
        let left = corner(0, 0, NativeStemHeadSide::Left);
        let right = corner(0, 1, NativeStemHeadSide::Right);
        let unread = corner(1, 2, NativeStemHeadSide::Left);
        let relations = vec![relation(0, left), relation(1, right), relation(2, unread)];
        let edges = vec![
            edge(0, 10, NativeStemHeadSide::Left, 1),
            edge(1, 11, NativeStemHeadSide::Left, 2),
            edge(2, 12, NativeStemHeadSide::Right, 1),
        ];
        let initial = known_stem(0, None, false, vertical_line(20.0), 1.0);
        let catalogue = vec![
            initial.clone(),
            known_stem(1, Some(101), true, vertical_line(30.0), 1.0),
            known_stem(2, Some(102), true, vertical_line(40.0), 1.0),
        ];
        let entries = vec![
            NativeStemsBeamReuseEntryEvidence {
                map_ordinal: 0,
                corner: left,
                observation: NativeStemsBeamReuseEntryObservation::Examined {
                    s_linker_linked: true,
                    head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::Exhaustive(scan(
                        left,
                        edges.clone(),
                        &catalogue[1..],
                    )),
                },
            },
            NativeStemsBeamReuseEntryEvidence {
                map_ordinal: 1,
                corner: right,
                observation: NativeStemsBeamReuseEntryObservation::Examined {
                    s_linker_linked: true,
                    head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::Exhaustive(scan(
                        right,
                        edges,
                        &catalogue[1..],
                    )),
                },
            },
            NativeStemsBeamReuseEntryEvidence {
                map_ordinal: 2,
                corner: unread,
                observation: NativeStemsBeamReuseEntryObservation::UnreadAfterSelection,
            },
        ];
        let before = catalogue.clone();
        let (selected, disposition, trace) =
            evaluate_reuse(&relations, &entries, &catalogue[1..], initial).expect("valid reuse");
        assert_eq!(selected.stem_identity, 1);
        assert_eq!(
            disposition,
            NativeStemsBeamReuseDisposition::Selected {
                map_ordinal: 1,
                stem_identity: 1
            }
        );
        assert_eq!(
            trace[0].action,
            NativeStemsBeamReuseAction::ContinueMultiplicity
        );
        assert_eq!(
            trace[1].action,
            NativeStemsBeamReuseAction::SelectUnique { stem_identity: 1 }
        );
        assert_eq!(
            trace[2].action,
            NativeStemsBeamReuseAction::UnreadAfterSelection
        );
        assert_eq!(catalogue, before);
        assert!(catalogue.iter().any(|stem| stem.stem_identity == 0));

        let reversed_live_catalogue = [catalogue[2].clone(), catalogue[1].clone()];
        assert!(matches!(
            evaluate_reuse(
                &relations,
                &entries,
                &reversed_live_catalogue,
                catalogue[0].clone()
            ),
            Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                phase: "Java HeadStem snapshot hash",
                ..
            })
        ));
    }

    #[test]
    fn all_unlinked_and_linked_multiplicity_are_distinct_no_reuse_results() {
        let left = corner(0, 0, NativeStemHeadSide::Left);
        let relations = vec![relation(0, left)];
        let initial = known_stem(0, None, false, vertical_line(20.0), 1.0);
        let first_live = known_stem(1, Some(101), true, vertical_line(30.0), 1.0);
        let mut second_live = known_stem(2, Some(102), true, vertical_line(40.0), 1.0);
        second_live.glyph_id = first_live.glyph_id;
        second_live.glyph_content = first_live.glyph_content.clone();
        let catalogue = [initial.clone(), first_live, second_live];
        assert!(validate_live_sig_stems(&catalogue[1..], &empty_committed_state(200)).is_ok());
        let unlinked = vec![NativeStemsBeamReuseEntryEvidence {
            map_ordinal: 0,
            corner: left,
            observation: NativeStemsBeamReuseEntryObservation::Examined {
                s_linker_linked: false,
                head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::NotRead,
            },
        }];
        let (_, disposition, _) =
            evaluate_reuse(&relations, &unlinked, &[], initial.clone()).unwrap();
        assert_eq!(disposition, NativeStemsBeamReuseDisposition::AllUnlinked);

        let multiple = vec![NativeStemsBeamReuseEntryEvidence {
            map_ordinal: 0,
            corner: left,
            observation: NativeStemsBeamReuseEntryObservation::Examined {
                s_linker_linked: true,
                head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::Exhaustive(scan(
                    left,
                    vec![
                        edge(0, 10, NativeStemHeadSide::Left, 1),
                        edge(1, 11, NativeStemHeadSide::Left, 2),
                    ],
                    &catalogue[1..],
                )),
            },
        }];
        let (same, disposition, _) =
            evaluate_reuse(&relations, &multiple, &catalogue[1..], initial).unwrap();
        assert_eq!(same.stem_identity, 0);
        assert_eq!(disposition, NativeStemsBeamReuseDisposition::NoUnique);
    }

    #[test]
    fn invalid_head_scan_certificates_fail_closed_or_preserve_java_npe() {
        let left = corner(0, 0, NativeStemHeadSide::Left);
        let relation = relation(0, left);
        let attached = known_stem(1, Some(101), true, vertical_line(30.0), 1.0);
        let invalid_target = known_stem(2, None, false, vertical_line(40.0), 1.0);
        let extra_attached = known_stem(3, Some(103), true, vertical_line(50.0), 1.0);
        let catalogue = vec![attached.clone(), invalid_target];
        let extra_catalogue = [attached.clone(), extra_attached];

        let missing_side = scan(
            left,
            vec![edge(0, 10, NativeStemHeadSide::Right, 1)],
            std::slice::from_ref(&attached),
        );
        let entries = vec![NativeStemsBeamReuseEntryEvidence {
            map_ordinal: 0,
            corner: left,
            observation: NativeStemsBeamReuseEntryObservation::Examined {
                s_linker_linked: true,
                head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::Exhaustive(missing_side),
            },
        }];
        assert!(matches!(
            evaluate_reuse(
                std::slice::from_ref(&relation),
                &entries,
                std::slice::from_ref(&attached),
                known_stem(0, None, false, vertical_line(20.0), 1.0)
            ),
            Err(NativeStemsBeamVLinkReuseCheckError::JavaNullHeadSideStemSet { .. })
        ));
        assert!(matches!(
            evaluate_reuse(
                std::slice::from_ref(&relation),
                &entries,
                &extra_catalogue,
                known_stem(0, None, false, vertical_line(20.0), 1.0)
            ),
            Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                phase: "ordered live SIG stem catalogue",
                ..
            })
        ));
        assert!(matches!(
            evaluate_reuse(
                std::slice::from_ref(&relation),
                &entries,
                &[],
                known_stem(0, None, false, vertical_line(20.0), 1.0)
            ),
            Err(NativeStemsBeamVLinkReuseCheckError::MissingStem { stem_identity: 1 })
        ));

        let second_attached = known_stem(4, Some(104), true, vertical_line(60.0), 1.0);
        let reordered_missing_side = vec![NativeStemsBeamReuseEntryEvidence {
            map_ordinal: 0,
            corner: left,
            observation: NativeStemsBeamReuseEntryObservation::Examined {
                s_linker_linked: true,
                head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::Exhaustive(scan(
                    left,
                    vec![
                        edge(0, 20, NativeStemHeadSide::Right, attached.stem_identity),
                        edge(
                            1,
                            21,
                            NativeStemHeadSide::Right,
                            second_attached.stem_identity,
                        ),
                    ],
                    &[second_attached.clone(), attached.clone()],
                )),
            },
        }];
        assert!(matches!(
            evaluate_reuse(
                std::slice::from_ref(&relation),
                &reordered_missing_side,
                &[second_attached, attached],
                known_stem(0, None, false, vertical_line(20.0), 1.0)
            ),
            Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                phase: "ordered live SIG stem catalogue",
                ..
            })
        ));

        let invalid_sig = NativeStemsBeamHeadStemScan {
            corner: left,
            baseline_relation_count: 1,
            scanned_relation_count: 1,
            provenance_sha256: "a".repeat(64),
            edges: vec![edge(0, 10, NativeStemHeadSide::Left, 2)],
        };
        assert!(matches!(
            validate_head_stem_scan(&invalid_sig, &relation, &catalogue),
            Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                phase: "HeadStem target SIG identity",
                ..
            })
        ));
        let mut uppercase_provenance = scan(
            left,
            vec![edge(0, 10, NativeStemHeadSide::Left, 1)],
            &catalogue,
        );
        uppercase_provenance.provenance_sha256 = "A".repeat(64);
        assert!(validate_head_stem_scan(&uppercase_provenance, &relation, &catalogue).is_err());
    }

    #[test]
    fn repeated_head_reads_must_share_snapshot_and_relation_identity_is_global() {
        let left = corner(0, 0, NativeStemHeadSide::Left);
        let right = corner(0, 1, NativeStemHeadSide::Right);
        let relations = vec![relation(0, left), relation(1, right)];
        let catalogue = [
            known_stem(0, None, false, vertical_line(20.0), 1.0),
            known_stem(1, Some(101), true, vertical_line(30.0), 1.0),
            known_stem(2, Some(102), true, vertical_line(40.0), 1.0),
        ];
        let entries = vec![
            NativeStemsBeamReuseEntryEvidence {
                map_ordinal: 0,
                corner: left,
                observation: NativeStemsBeamReuseEntryObservation::Examined {
                    s_linker_linked: true,
                    head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::Exhaustive(scan(
                        left,
                        vec![
                            edge(0, 10, NativeStemHeadSide::Left, 1),
                            edge(1, 11, NativeStemHeadSide::Left, 2),
                        ],
                        &catalogue[1..],
                    )),
                },
            },
            NativeStemsBeamReuseEntryEvidence {
                map_ordinal: 1,
                corner: right,
                observation: NativeStemsBeamReuseEntryObservation::Examined {
                    s_linker_linked: true,
                    head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::Exhaustive(scan(
                        right,
                        vec![edge(0, 10, NativeStemHeadSide::Right, 1)],
                        &catalogue[1..],
                    )),
                },
            },
        ];
        assert!(matches!(
            evaluate_reuse(&relations, &entries, &catalogue[1..], catalogue[0].clone()),
            Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                phase: "shared HeadInter graph snapshot",
                ..
            })
        ));

        let other_head = corner(1, 2, NativeStemHeadSide::Left);
        let global_relations = vec![relation(0, left), relation(1, other_head)];
        let global_entries = vec![
            NativeStemsBeamReuseEntryEvidence {
                map_ordinal: 0,
                corner: left,
                observation: NativeStemsBeamReuseEntryObservation::Examined {
                    s_linker_linked: true,
                    head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::Exhaustive(scan(
                        left,
                        vec![
                            edge(0, 10, NativeStemHeadSide::Left, 1),
                            edge(1, 11, NativeStemHeadSide::Left, 2),
                        ],
                        &catalogue[1..],
                    )),
                },
            },
            NativeStemsBeamReuseEntryEvidence {
                map_ordinal: 1,
                corner: other_head,
                observation: NativeStemsBeamReuseEntryObservation::Examined {
                    s_linker_linked: true,
                    head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::Exhaustive(scan(
                        other_head,
                        vec![
                            edge(0, 10, NativeStemHeadSide::Left, 1),
                            edge(1, 12, NativeStemHeadSide::Left, 2),
                        ],
                        &catalogue[1..],
                    )),
                },
            },
        ];
        assert!(matches!(
            evaluate_reuse(
                &global_relations,
                &global_entries,
                &catalogue[1..],
                catalogue[0].clone()
            ),
            Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState {
                phase: "global HeadStem relation identity",
                ..
            })
        ));
    }
}
