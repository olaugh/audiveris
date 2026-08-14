//! Atomic carriage of one already-awaited STEMS SIDES transaction.

use std::{collections::BTreeSet, error::Error, fmt};

use audiveris_image::{beam_structure::Segment, run_table::Orientation};

use crate::{
    native_sig::{
        NativeSigEdge, NativeSigEdgeId, NativeSigHeadStemPayload, NativeSigInterKind,
        NativeSigRelationKind, NativeSigRelationOrigin, NativeSigSupport, NativeSigSystem,
        NativeSigSystemBindings, NativeSigVertex, NativeSigVertexId,
    },
    native_stem_seeds::NativeStemSeedSystemRecognition,
    native_stems_beam_builders::{NativeStemsBeamBuilderSystem, java_double_compare},
    native_stems_beam_link_plans::{
        NativeStemsBeamHeadRelationCheck, NativeStemsBeamLinkPlanSystem,
        project_native_stems_head_c_link_relation,
    },
    native_stems_beam_reachability::NativeStemsBeamReachabilitySystem,
    native_stems_beam_scheduler::{
        BEAM_SEED_PROFILE, NativeStemsBeamCompletedStumpVLinkEvidence,
        NativeStemsBeamSchedulerPass, NativeStemsBeamSchedulerStatus,
        NativeStemsBeamSchedulerStumpsContinuation, NativeStemsBeamSchedulerStumpsStatus,
        NativeStemsBeamSchedulerSystem, continue_native_stems_beam_scheduler_into_stumps,
        resume_native_stems_beam_scheduler_after_hook_removal,
        resume_native_stems_beam_scheduler_after_stumps_transaction,
    },
    native_stems_beam_stumps::NativeStemsBeamStumpSystem,
    native_stems_beam_vlink_b_linker_flag::{
        NativeStemsBeamVLinkBLinkerFlagState, NativeStemsBeamVLinkBLinkerFlagTransaction,
        apply_native_stems_beam_vlink_b_linker_flag_transaction,
    },
    native_stems_beam_vlink_base_apply::{
        NativeStemsBeamBeamInterIndexBootstrapEntry, NativeStemsBeamInterIndexAppend,
        NativeStemsBeamInterIndexLookup, NativeStemsBeamNextPersistentIdLookup,
        NativeStemsBeamVLinkBaseApplyState, NativeStemsBeamVLinkBaseApplyTransaction,
        NativeStemsBeamVLinkBaseRolloverAuthority,
        apply_native_stems_beam_vlink_base_transaction_to_native_sig,
        roll_native_stems_beam_vlink_base_apply_state,
    },
    native_stems_beam_vlink_head_links::{
        NativeStemsBeamHeadLinkHeadRef, NativeStemsBeamHeadSLinkerRef,
        NativeStemsBeamNativeHeadTransaction, NativeStemsBeamNativeSLinkerCell,
        apply_native_stems_beam_vlink_head_transaction_to_native_sig,
        initialize_native_stems_beam_s_linker_cells,
    },
    native_stems_beam_vlink_outer_b_linker::{
        NativeStemsBeamNativeOuterResumeTransaction,
        apply_native_stems_beam_outer_and_resume_transaction,
    },
    native_stems_beam_vlink_reuse_check::{
        NativeStemsBeamRelationParameters, NativeStemsBeamVLinkReuseCheck,
        NativeStemsBeamVLinkReuseLiveState, evaluate_native_stems_beam_vlink_reuse_check,
        project_native_stems_beam_vlink_reuse_live_state,
    },
    native_stems_beam_vlink_sibling_links::{
        NativeStemsBeamNativeBLinkerCell, NativeStemsBeamNativeSiblingTransaction,
        apply_native_stems_beam_vlink_sibling_transaction_to_native_sig,
    },
    native_stems_beam_vlink_transaction::{
        NativeStemsBeamCreateStemDisposition, NativeStemsBeamExhaustiveGlyphEqualsScan,
        NativeStemsBeamFrontierPreparation, NativeStemsBeamGlyphRegistryBootstrapEntry,
        NativeStemsBeamStemCheckerContext, NativeStemsBeamStemGrade,
        NativeStemsBeamSystemStemAuthorityProof, NativeStemsBeamVLinkTransaction,
        NativeStemsCreateStemCandidateTransaction, NativeStemsFirstGlyphIndexBridge,
        apply_native_stems_beam_vlink_create_stem_transaction,
        apply_native_stems_create_stem_candidate_transaction,
        materialize_native_stems_beam_frontier_candidate,
        prepare_native_stems_beam_vlink_frontier_state,
        prepare_native_stems_beam_vlink_frontier_state_from_first_stems_bridge,
    },
    native_stems_beam_vlinkers::{NativeStemsBeamVLinkerSystem, generic_intersection},
    native_stems_head_builders::{
        NativeStemsHeadBuilderGlyphRef, NativeStemsHeadBuilderItemKind,
        NativeStemsHeadBuilderSystem, NativeStemsHeadBuilderTargetRef,
    },
    native_stems_head_corner_reachability::{
        NativeStemsHeadCornerReachabilitySystem, NativeStemsHeadCornerRef, NativeStemsHeadStumpRef,
    },
    native_stems_head_corners::NativeStemsHeadCornerSystem,
};

/// Mutable authorities which survive one SIDES transaction.
///
/// Transaction state has one owner: `latest_base_apply.transaction_state`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSidesCarrier {
    pub scheduler: NativeStemsBeamSchedulerSystem,
    pub latest_base_apply: NativeStemsBeamVLinkBaseApplyState,
    pub sig: NativeSigSystem,
    pub bindings: NativeSigSystemBindings,
    pub b_cells: Vec<NativeStemsBeamNativeBLinkerCell>,
    pub s_cells: Vec<NativeStemsBeamNativeSLinkerCell>,
    pub beam_inter_index: Vec<NativeStemsBeamBeamInterIndexBootstrapEntry>,
    pub configured_inter_vip_ids: Vec<i32>,
}

/// Immutable products shared by transactions in one system's SIDES pass.
#[derive(Clone, Copy, Debug)]
pub struct NativeStemsBeamSidesContext<'a> {
    pub plans: &'a NativeStemsBeamLinkPlanSystem,
    pub builders: &'a NativeStemsBeamBuilderSystem,
    pub stumps: &'a NativeStemsBeamStumpSystem,
    pub vlinkers: &'a NativeStemsBeamVLinkerSystem,
    pub reachability: &'a NativeStemsBeamReachabilitySystem,
    pub head_corners: &'a NativeStemsHeadCornerSystem,
    pub checker: &'a NativeStemsBeamStemCheckerContext,
    pub relation_parameters: NativeStemsBeamRelationParameters,
}

/// Candidate-specific page GlyphIndex evidence for one frontier.
#[derive(Clone, Copy, Debug)]
pub struct NativeStemsBeamSidesGlyphEvidence<'a> {
    pub selected: &'a [NativeStemsBeamGlyphRegistryBootstrapEntry],
    pub exhaustive_candidate: Option<&'a NativeStemsBeamExhaustiveGlyphEqualsScan>,
}

/// Exact trace of one B12-B19 carrier advance.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSidesTransaction {
    pub preparation: NativeStemsBeamFrontierPreparation,
    pub create: NativeStemsBeamVLinkTransaction,
    pub reuse_live_state: NativeStemsBeamVLinkReuseLiveState,
    pub reuse: NativeStemsBeamVLinkReuseCheck,
    pub base: NativeStemsBeamVLinkBaseApplyTransaction,
    pub flag: NativeStemsBeamVLinkBLinkerFlagTransaction,
    pub siblings: NativeStemsBeamNativeSiblingTransaction,
    pub heads: NativeStemsBeamNativeHeadTransaction,
    pub outer_resume: NativeStemsBeamNativeOuterResumeTransaction,
}

/// Exact B12-B17 mutation plus B19 continuation for one STUMPS V frontier.
///
/// Java's `linkStumps` invokes `VLinker.link` directly, so this trace has no
/// B18 outer assignment. B15 and B16 are the persistent B-cell authorities
/// visible to the resumed stump loop.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamStumpsTransaction {
    pub preparation: NativeStemsBeamFrontierPreparation,
    pub create: NativeStemsBeamVLinkTransaction,
    pub reuse_live_state: NativeStemsBeamVLinkReuseLiveState,
    pub reuse: NativeStemsBeamVLinkReuseCheck,
    pub base: NativeStemsBeamVLinkBaseApplyTransaction,
    pub flag: NativeStemsBeamVLinkBLinkerFlagTransaction,
    pub siblings: NativeStemsBeamNativeSiblingTransaction,
    pub heads: NativeStemsBeamNativeHeadTransaction,
    pub resume: NativeStemsBeamSchedulerStumpsContinuation,
}

/// Atomic result of driving up to a bounded number of STUMPS V frontiers.
///
/// `status` is the carrier's exact scheduler status after the returned
/// transactions. An awaiting status means the caller-provided limit was
/// reached; completion is Java's true post-STUMPS terminal.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamStumpsDrive {
    pub transactions: Vec<NativeStemsBeamStumpsTransaction>,
    pub status: NativeStemsBeamSchedulerStumpsStatus,
}

/// Production-owned entry to Java's head-linking phase 1.
///
/// The embedded beam carrier is the complete post-STUMPS authority. `heads`
/// follows stable `Inters.byReverseGrade` order, while every head entry reads
/// the two persistent S-linker cells already mutated by beam-origin links.
/// This boundary deliberately stops before `HeadLinker.linkSides` evaluates
/// either C linker.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadPhase1Carrier {
    pub beam_state: NativeStemsBeamSidesCarrier,
    pub heads: Vec<NativeStemsHeadPhase1Head>,
    pub current_index: usize,
    pub unlinked_heads: Vec<NativeStemsBeamHeadLinkHeadRef>,
    pub undefined_sides: Vec<NativeStemsBeamHeadSLinkerRef>,
    pub frontier: NativeStemsHeadPhase1Frontier,
    pub frontier_consumed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadCLinkTransaction {
    pub system_id: usize,
    pub corner: NativeStemsHeadCornerRef,
    pub last_index: usize,
    pub max_index: usize,
    pub selected_glyph_id: i32,
    pub relation: NativeStemsBeamHeadRelationCheck,
    pub create: NativeStemsCreateStemCandidateTransaction,
    pub stem_vertex: NativeSigVertexId,
    pub head_stem_edge: NativeSigEdgeId,
    pub s_linker: NativeStemsBeamHeadSLinkerRef,
    pub s_linked_before: bool,
    pub s_linked_after: bool,
    pub closed_cell_changes: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadPhase1Head {
    pub reference: NativeStemsBeamHeadLinkHeadRef,
    pub grade: f64,
    /// Java `HorizontalSide.values()`: LEFT, then RIGHT.
    pub sides: Vec<NativeStemsBeamNativeSLinkerCell>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadPhase1Frontier {
    pub head: NativeStemsBeamHeadLinkHeadRef,
    pub stem_profile: i32,
    pub link_profile: i32,
    pub append: bool,
    pub side_decisions: Vec<NativeStemsHeadPhase1SideDecision>,
    pub next_corner: NativeStemsHeadCornerRef,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadPhase1SideDecision {
    pub side: crate::stems_step::NativeStemHeadSide,
    pub linked_before: bool,
    pub closed_before: bool,
    pub top_can_link: Option<bool>,
    pub bottom_can_link: Option<bool>,
}

/// Atomic removal of one competing hook plus the following SIDES continuation.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamHookRemovalTransaction {
    pub system_id: usize,
    pub beam: crate::native_stems_beam_stumps::NativeStemsBeamSource,
    pub competing_hook: crate::native_stems_beam_stumps::NativeStemsBeamSource,
    pub hook_vertex: NativeSigVertexId,
    pub group_vertex: NativeSigVertexId,
    pub removed_edges: Vec<NativeSigEdgeId>,
    pub active_vertex_count_before: usize,
    pub active_vertex_count_after: usize,
    pub active_edge_count_before: usize,
    pub active_edge_count_after: usize,
    pub group_members_before: Vec<crate::native_stems_beam_stumps::NativeStemsBeamSource>,
    pub group_members_after: Vec<crate::native_stems_beam_stumps::NativeStemsBeamSource>,
    pub resume: crate::native_stems_beam_scheduler::NativeStemsBeamSchedulerResume,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamSidesError {
    pub stage: &'static str,
    pub detail: String,
}

impl fmt::Display for NativeStemsBeamSidesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "native SIDES carrier failed at {}: {}",
            self.stage, self.detail
        )
    }
}

impl Error for NativeStemsBeamSidesError {}

fn stage(stage: &'static str, error: impl fmt::Debug) -> NativeStemsBeamSidesError {
    NativeStemsBeamSidesError {
        stage,
        detail: format!("{error:?}"),
    }
}

fn reconcile_known_stems(
    base: &mut NativeStemsBeamVLinkBaseApplyState,
    sig: &NativeSigSystem,
    bindings: &NativeSigSystemBindings,
) -> Result<(), NativeStemsBeamSidesError> {
    let stems = &mut base.transaction_state.system_stems.known_stems;
    if stems.len() != bindings.stem_vertices.len() {
        return Err(stage(
            "stem-runtime-reconciliation",
            "known-stem/binding coverage differs",
        ));
    }
    for stem in stems {
        let id = bindings
            .stem_vertices
            .get(&stem.stem_identity)
            .ok_or_else(|| stage("stem-runtime-reconciliation", "missing native stem binding"))?;
        let vertex = sig
            .vertices
            .get(id.0)
            .ok_or_else(|| stage("stem-runtime-reconciliation", "stem binding outside SIG"))?;
        if vertex.ordinal != id.0
            || !vertex.active
            || vertex.removed
            || vertex.kind != NativeSigInterKind::Stem
            || stem.inter_id.is_none()
        {
            return Err(stage(
                "stem-runtime-reconciliation",
                "bound stem is not live and attached",
            ));
        }
        stem.sig_attached = true;
        stem.abnormal = vertex.abnormal;
    }
    Ok(())
}

/// Execute one already-awaited later frontier through B12-B19.
///
/// All mutable state is cloned first and swapped only after scheduler resume,
/// so even a late B17/B19 failure leaves the caller's carrier unchanged.
pub fn advance_native_stems_beam_sides_transaction(
    carrier: &mut NativeStemsBeamSidesCarrier,
    context: NativeStemsBeamSidesContext<'_>,
    glyphs: NativeStemsBeamSidesGlyphEvidence<'_>,
) -> Result<NativeStemsBeamSidesTransaction, NativeStemsBeamSidesError> {
    advance_native_stems_beam_sides_transaction_with_authority(
        carrier,
        context,
        GlyphAuthority::Legacy(glyphs),
        CarrierPass::Sides,
    )
    .and_then(NativeStemsBeamCarrierTransaction::into_sides)
}

/// Execute one frontier from the validated one-time first-STEMS bridge.
/// No candidate-specific Java registry row is accepted by this entry point.
pub fn advance_native_stems_beam_sides_transaction_from_first_stems_bridge(
    carrier: &mut NativeStemsBeamSidesCarrier,
    context: NativeStemsBeamSidesContext<'_>,
    bridge: &NativeStemsFirstGlyphIndexBridge,
) -> Result<NativeStemsBeamSidesTransaction, NativeStemsBeamSidesError> {
    advance_native_stems_beam_sides_transaction_with_authority(
        carrier,
        context,
        GlyphAuthority::FirstStems(bridge),
        CarrierPass::Sides,
    )
    .and_then(NativeStemsBeamCarrierTransaction::into_sides)
}

/// Execute one typed STUMPS frontier through B12-B17 and resume its stump
/// worklist. The first-STEMS bridge remains the glyph identity authority.
pub fn advance_native_stems_beam_stumps_transaction_from_first_stems_bridge(
    carrier: &mut NativeStemsBeamSidesCarrier,
    context: NativeStemsBeamSidesContext<'_>,
    bridge: &NativeStemsFirstGlyphIndexBridge,
) -> Result<NativeStemsBeamStumpsTransaction, NativeStemsBeamSidesError> {
    advance_native_stems_beam_sides_transaction_with_authority(
        carrier,
        context,
        GlyphAuthority::FirstStems(bridge),
        CarrierPass::Stumps,
    )
    .and_then(NativeStemsBeamCarrierTransaction::into_stumps)
}

/// Atomically drive a bounded sequence of already-awaited STUMPS frontiers.
///
/// Every transaction is the same B12-B17 plus scheduler-resume operation as
/// [`advance_native_stems_beam_stumps_transaction_from_first_stems_bridge`].
/// The complete carrier is committed only after the batch reaches true
/// post-STUMPS completion or consumes `transaction_limit`; any later failure
/// rolls back the earlier transactions in this call as well.
pub fn drive_native_stems_beam_stumps_from_first_stems_bridge(
    carrier: &mut NativeStemsBeamSidesCarrier,
    context: NativeStemsBeamSidesContext<'_>,
    bridge: &NativeStemsFirstGlyphIndexBridge,
    transaction_limit: usize,
) -> Result<NativeStemsBeamStumpsDrive, NativeStemsBeamSidesError> {
    if transaction_limit == 0 {
        return Err(stage(
            "STUMPS-drive-limit",
            "transaction limit must be positive",
        ));
    }

    let mut shadow = carrier.clone();
    let mut transactions = Vec::new();
    loop {
        let status = current_stumps_status(&shadow.scheduler)?;
        if matches!(
            status,
            NativeStemsBeamSchedulerStumpsStatus::Completed { .. }
        ) || transactions.len() == transaction_limit
        {
            *carrier = shadow;
            return Ok(NativeStemsBeamStumpsDrive {
                transactions,
                status,
            });
        }

        match advance_native_stems_beam_stumps_transaction_from_first_stems_bridge(
            &mut shadow,
            context,
            bridge,
        ) {
            Ok(transaction) => transactions.push(transaction),
            Err(mut error) => {
                error.detail = format!(
                    "after {} successful shadow transactions: {}",
                    transactions.len(),
                    error.detail
                );
                return Err(error);
            }
        }
    }
}

/// Transfer an authenticated post-STUMPS carrier into head-linking phase 1.
///
/// This is an atomic ownership boundary rather than a head-link mutation: the
/// returned carrier owns a clone of every mutable authority, and failure does
/// not consume or alter the caller's completed beam carrier.
pub fn begin_native_stems_head_linking_phase1(
    carrier: &NativeStemsBeamSidesCarrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Carrier, NativeStemsBeamSidesError> {
    let NativeStemsBeamSchedulerStatus::Completed { .. } = &carrier.scheduler.status else {
        return Err(stage(
            "HEADS-phase1-terminal",
            "beam scheduler has not completed STUMPS",
        ));
    };
    if carrier.scheduler.system_id != head_corners.system_id
        || carrier.sig.system_id != head_corners.system_id
        || carrier.bindings.system_id != head_corners.system_id
        || head_builders.system_id != head_corners.system_id
        || plans.system_id != head_corners.system_id
    {
        return Err(stage(
            "HEADS-phase1-system",
            "completed carrier and head product differ by system",
        ));
    }
    carrier
        .bindings
        .validate_against(&carrier.sig)
        .map_err(|error| stage("HEADS-phase1-bindings", error))?;

    let mut expected_grade_order = (0..head_corners.heads_in_sig_order.len()).collect::<Vec<_>>();
    expected_grade_order.sort_by(|&left, &right| {
        java_double_compare(
            f64::from_bits(head_corners.heads_in_sig_order[right].grade_bits),
            f64::from_bits(head_corners.heads_in_sig_order[left].grade_bits),
        )
        .cmp(&0)
    });
    if expected_grade_order != head_corners.heads_by_reverse_grade {
        return Err(stage(
            "HEADS-phase1-order",
            "head reverse-grade permutation is not Java's stable order",
        ));
    }

    let expected_cells = initialize_native_stems_beam_s_linker_cells(head_corners)
        .map_err(|error| stage("HEADS-phase1-S-topology", error))?;
    if expected_cells.len() != carrier.s_cells.len() {
        return Err(stage(
            "HEADS-phase1-S-topology",
            "persistent S-cell catalogue is not exhaustive",
        ));
    }
    let mut live_refs = BTreeSet::new();
    for cell in &carrier.s_cells {
        if !live_refs.insert(cell.reference) {
            return Err(stage(
                "HEADS-phase1-S-topology",
                "duplicate persistent S-cell reference",
            ));
        }
        let expected = expected_cells
            .iter()
            .find(|expected| expected.reference == cell.reference)
            .ok_or_else(|| {
                stage(
                    "HEADS-phase1-S-topology",
                    "persistent S cell is absent from constructor topology",
                )
            })?;
        if cell.ordered_observer_corners != expected.ordered_observer_corners {
            return Err(stage(
                "HEADS-phase1-S-topology",
                "persistent S-cell observer order differs",
            ));
        }
    }

    let mut heads = Vec::with_capacity(expected_grade_order.len());
    for sig_ordinal in expected_grade_order {
        let head = &head_corners.heads_in_sig_order[sig_ordinal];
        let x_ordinal = head_corners
            .heads_by_abscissa
            .iter()
            .position(|&candidate| candidate == sig_ordinal)
            .ok_or_else(|| stage("HEADS-phase1-order", "head absent from abscissa order"))?;
        let reference = NativeStemsBeamHeadLinkHeadRef {
            reference: head.reference,
            sig_ordinal,
            x_ordinal,
        };
        let vertex_id = carrier
            .bindings
            .head_vertices
            .get(&head.reference)
            .ok_or_else(|| stage("HEADS-phase1-head", "head product lacks a live SIG binding"))?;
        let vertex = carrier.sig.vertex(vertex_id.0).ok_or_else(|| {
            stage(
                "HEADS-phase1-head",
                "head binding points outside the live SIG",
            )
        })?;
        if !vertex.active
            || vertex.removed
            || vertex.kind != NativeSigInterKind::Head
            || vertex.grade.to_bits() != head.grade_bits
        {
            return Err(stage(
                "HEADS-phase1-head",
                "head binding is not the exact live graded HeadInter",
            ));
        }
        let sides = [
            crate::stems_step::NativeStemHeadSide::Left,
            crate::stems_step::NativeStemHeadSide::Right,
        ]
        .into_iter()
        .map(|horizontal| {
            let side_ref = NativeStemsBeamHeadSLinkerRef {
                head: reference,
                horizontal,
            };
            carrier
                .s_cells
                .iter()
                .find(|cell| cell.reference == side_ref)
                .cloned()
                .ok_or_else(|| stage("HEADS-phase1-S-topology", "head side cell is missing"))
        })
        .collect::<Result<Vec<_>, _>>()?;
        heads.push(NativeStemsHeadPhase1Head {
            reference,
            grade: vertex.grade,
            sides,
        });
    }

    let first = heads
        .first()
        .cloned()
        .ok_or_else(|| stage("HEADS-phase1-frontier", "system has no stem-capable head"))?;
    let mut side_decisions = Vec::new();
    let mut next_corner = None;
    for side in &first.sides {
        if side.linked {
            side_decisions.push(NativeStemsHeadPhase1SideDecision {
                side: side.reference.horizontal,
                linked_before: true,
                closed_before: side.closed,
                top_can_link: None,
                bottom_can_link: None,
            });
            continue;
        }
        if side.closed {
            side_decisions.push(NativeStemsHeadPhase1SideDecision {
                side: side.reference.horizontal,
                linked_before: false,
                closed_before: true,
                top_can_link: None,
                bottom_can_link: None,
            });
            continue;
        }
        let top = side.ordered_observer_corners[0];
        let bottom = side.ordered_observer_corners[1];
        let top = NativeStemsHeadCornerRef {
            head: top.head,
            sig_ordinal: top.sig_ordinal,
            x_ordinal: top.x_ordinal,
            horizontal: top.horizontal,
            vertical: top.vertical,
        };
        let bottom = NativeStemsHeadCornerRef {
            head: bottom.head,
            sig_ordinal: bottom.sig_ordinal,
            x_ordinal: bottom.x_ordinal,
            horizontal: bottom.horizontal,
            vertical: bottom.vertical,
        };
        let top_ok = bounded_head_can_link(
            top,
            0,
            head_builders,
            &carrier.s_cells,
            plans.min_linker_length,
        )?;
        let bottom_ok = bounded_head_can_link(
            bottom,
            0,
            head_builders,
            &carrier.s_cells,
            plans.min_linker_length,
        )?;
        side_decisions.push(NativeStemsHeadPhase1SideDecision {
            side: side.reference.horizontal,
            linked_before: false,
            closed_before: false,
            top_can_link: Some(top_ok),
            bottom_can_link: Some(bottom_ok),
        });
        match (top_ok, bottom_ok) {
            (true, false) => {
                next_corner = Some(top);
                break;
            }
            (false, true) => {
                next_corner = Some(bottom);
                break;
            }
            (true, true) => {
                return Err(stage(
                    "HEADS-phase1-frontier",
                    "first head reaches the unported dual-corner selection branch",
                ));
            }
            (false, false) => {}
        }
    }
    let next_corner = next_corner.ok_or_else(|| {
        stage(
            "HEADS-phase1-frontier",
            "first head has no bounded C-link transaction",
        )
    })?;

    Ok(NativeStemsHeadPhase1Carrier {
        beam_state: carrier.clone(),
        heads,
        current_index: 0,
        unlinked_heads: Vec::new(),
        undefined_sides: Vec::new(),
        frontier: NativeStemsHeadPhase1Frontier {
            head: first.reference,
            stem_profile: 0,
            link_profile: plans.link_profile,
            append: false,
            side_decisions,
            next_corner,
        },
        frontier_consumed: false,
    })
}

fn bounded_head_can_link(
    corner: NativeStemsHeadCornerRef,
    stem_profile: i32,
    builders: &NativeStemsHeadBuilderSystem,
    s_cells: &[NativeStemsBeamNativeSLinkerCell],
    min_linker_length: i32,
) -> Result<bool, NativeStemsBeamSidesError> {
    let builder = builders
        .builders
        .iter()
        .find(|builder| builder.start == corner)
        .ok_or_else(|| stage("HEADS-phase1-canLink", "corner has no C-origin builder"))?;
    let length = builder
        .lengths
        .get(&stem_profile)
        .copied()
        .ok_or_else(|| stage("HEADS-phase1-canLink", "builder lacks STRICT length"))?;
    if length < min_linker_length {
        return Ok(false);
    }
    let max_gap = builders
        .gap_map
        .get(&stem_profile)
        .copied()
        .ok_or_else(|| stage("HEADS-phase1-canLink", "builder lacks STRICT gap threshold"))?;
    for item in builder.items.iter().skip(1) {
        if item.kind == NativeStemsHeadBuilderItemKind::Gap && item.contribution > max_gap {
            return Ok(true);
        }
        let Some(NativeStemsHeadBuilderTargetRef::Head(target)) = item.target else {
            continue;
        };
        let target_ref = NativeStemsBeamHeadLinkHeadRef {
            reference: target.head,
            sig_ordinal: target.sig_ordinal,
            x_ordinal: target.x_ordinal,
        };
        let target_side = s_cells
            .iter()
            .find(|cell| {
                cell.reference.head == target_ref && cell.reference.horizontal == target.horizontal
            })
            .ok_or_else(|| stage("HEADS-phase1-canLink", "target C linker lacks S cell"))?;
        if target_side.linked {
            return Ok(false);
        }
        return Err(stage(
            "HEADS-phase1-canLink",
            "first head reaches the unported close-head/gap recursion",
        ));
    }
    Ok(true)
}

/// Execute the bounded single-item head-origin C-link frontier selected by
/// [`begin_native_stems_head_linking_phase1`].
///
/// This owns Java `CLinker.link` through expansion, origin-neutral
/// `StemBuilder.createStem`, SIG/InterIndex attachment, one HeadStem relation
/// and the shared S-linker write. It stops after the current head returns true,
/// before the outer phase-1 loop visits the next head.
#[expect(
    clippy::too_many_arguments,
    reason = "the atomic boundary authenticates each independently owned native authority"
)]
pub fn advance_native_stems_head_single_item_c_link(
    carrier: &mut NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    stem_seeds: &NativeStemSeedSystemRecognition,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    checker: &NativeStemsBeamStemCheckerContext,
    bridge: &NativeStemsFirstGlyphIndexBridge,
) -> Result<NativeStemsHeadCLinkTransaction, NativeStemsBeamSidesError> {
    let mut shadow = carrier.clone();
    let reconstructed = begin_native_stems_head_linking_phase1(
        &shadow.beam_state,
        head_corners,
        head_builders,
        plans,
    )?;
    if shadow.frontier_consumed
        || shadow.current_index != 0
        || shadow.heads != reconstructed.heads
        || shadow.frontier != reconstructed.frontier
        || !shadow.unlinked_heads.is_empty()
        || !shadow.undefined_sides.is_empty()
    {
        return Err(stage(
            "HEADS-CLink-frontier",
            "head carrier is not the authenticated phase-1 entry",
        ));
    }
    let frontier = shadow.frontier.clone();
    let builder = head_builders
        .builders
        .iter()
        .find(|builder| builder.start == frontier.next_corner)
        .ok_or_else(|| stage("HEADS-CLink-builder", "selected corner has no builder"))?;
    if builder.items.len() != 1
        || builder.items[0].kind != NativeStemsHeadBuilderItemKind::StartHeadHalfLinker
        || builder.items[0].glyph != builder.start_stump
        || builder.max_stem_profile != plans.link_profile
    {
        return Err(stage(
            "HEADS-CLink-expand",
            "selected frontier is not the bounded one-item start-C shape",
        ));
    }
    let Some(NativeStemsHeadBuilderGlyphRef::HeadStump {
        corner: stump_corner,
    }) = builder.start_stump
    else {
        return Err(stage(
            "HEADS-CLink-glyph",
            "bounded head frontier does not start from an attached head stump",
        ));
    };
    if stump_corner != frontier.next_corner {
        return Err(stage(
            "HEADS-CLink-glyph",
            "start stump belongs to a different C linker",
        ));
    }
    let selected_constructor_ordinal = head_corners.heads_in_sig_order
        [frontier.next_corner.sig_ordinal]
        .corners_in_constructor_order
        .iter()
        .find(|corner| {
            corner.horizontal == frontier.next_corner.horizontal
                && corner.vertical == frontier.next_corner.vertical
        })
        .map(|corner| corner.constructor_ordinal)
        .ok_or_else(|| {
            stage(
                "HEADS-CLink-builder",
                "selected constructor corner is missing",
            )
        })?;
    if head_reachability.system_id != head_corners.system_id {
        return Err(stage(
            "HEADS-CLink-glyph",
            "head reachability belongs to a different system",
        ));
    }
    let reach_corner = head_reachability
        .heads
        .iter()
        .flat_map(|head| &head.corners)
        .find(|corner| corner.reference == frontier.next_corner)
        .ok_or_else(|| {
            stage(
                "HEADS-CLink-glyph",
                "selected reachability corner is missing",
            )
        })?;
    let stump = reach_corner.stump.ok_or_else(|| {
        stage(
            "HEADS-CLink-glyph",
            "selected reachability stump is missing",
        )
    })?;
    let NativeStemsHeadStumpRef::Seed { free_glyph_ordinal } = stump.source else {
        return Err(stage(
            "HEADS-CLink-glyph",
            "bounded selected stump is not a retained vertical seed",
        ));
    };
    let seed = stem_seeds
        .free_glyphs
        .get(free_glyph_ordinal)
        .ok_or_else(|| {
            stage(
                "HEADS-CLink-glyph",
                "selected free vertical-seed ordinal is unavailable",
            )
        })?;
    if seed.bounds != stump.bounds || seed.weight != stump.weight {
        return Err(stage(
            "HEADS-CLink-glyph",
            "reachability stump and free vertical seed differ",
        ));
    }
    let candidate = crate::native_stems_beam_vlink_transaction::NativeStemsBeamFixedGlyphContent {
        bounds: seed.bounds,
        weight: seed.weight,
        run_table: seed.run_table.clone(),
    };
    let promoted = bridge
        .resolve_native_content(&candidate)
        .map_err(|error| stage("HEADS-CLink-first-STEMS-bridge", error))?;
    if !promoted.active_in_index || !promoted.strongly_retained {
        return Err(stage(
            "HEADS-CLink-first-STEMS-bridge",
            "selected seed canonical is not active and strongly retained",
        ));
    }
    let known = &mut shadow
        .beam_state
        .latest_base_apply
        .transaction_state
        .glyph_index
        .known_canonical_glyphs;
    let existing = known
        .iter()
        .filter(|glyph| glyph.content == candidate)
        .collect::<Vec<_>>();
    match existing.as_slice() {
        [] => known.push(
            crate::native_stems_beam_vlink_transaction::NativeStemsBeamKnownCanonicalGlyph {
                canonical_alias: promoted.canonical_alias,
                glyph_id: promoted.glyph_id,
                content: candidate.clone(),
                active_in_index: promoted.active_in_index,
                strongly_retained: promoted.strongly_retained,
            },
        ),
        [glyph]
            if glyph.glyph_id == promoted.glyph_id
                && glyph.canonical_alias == promoted.canonical_alias => {}
        _ => {
            return Err(stage(
                "HEADS-CLink-first-STEMS-bridge",
                "promoted seed conflicts with carried canonical state",
            ));
        }
    }

    let mut stem_line = if builder.y_direction > 0 {
        builder.theoretical_line
    } else {
        crate::stems_step::NativeStemLine {
            start: builder.theoretical_line.stop,
            stop: builder.theoretical_line.start,
        }
    };
    let centroid = glyph_centroid(&candidate)?;
    let intersection = generic_intersection(
        Segment {
            x1: stem_line.start.x,
            y1: stem_line.start.y,
            x2: stem_line.stop.x,
            y2: stem_line.stop.y,
        },
        Segment {
            x1: 0.0,
            y1: centroid.1,
            x2: 1000.0,
            y2: centroid.1,
        },
    );
    let shift = centroid.0 - intersection.x;
    stem_line.start.x += shift;
    stem_line.stop.x += shift;
    let minimum_tail = java_rint(1.75 * f64::from(head_builders.interline));
    let last_y = if builder.y_direction > 0 {
        builder.items[0]
            .line
            .start
            .y
            .max(builder.items[0].line.stop.y)
    } else {
        builder.items[0]
            .line
            .start
            .y
            .min(builder.items[0].line.stop.y)
    };
    let hard_y = builder.theoretical_line.start.y + f64::from(builder.y_direction * minimum_tail);
    if builder.y_direction * java_double_compare(last_y, hard_y) < 0 {
        return Err(stage(
            "HEADS-CLink-expand",
            "single item fails Java's hard tail target",
        ));
    }
    let relation = project_native_stems_head_c_link_relation(
        head_corners,
        head_builders,
        frontier.next_corner,
        stem_line,
        Some(candidate.bounds),
        frontier.link_profile,
    )
    .map_err(|error| stage("HEADS-CLink-relation", error))?;
    if !relation.accepted || relation.derived_horizontal != frontier.next_corner.horizontal {
        return Err(stage(
            "HEADS-CLink-relation",
            "start-head relation is rejected or changes horizontal side",
        ));
    }

    let create = apply_native_stems_create_stem_candidate_transaction(
        head_corners.system_id,
        frontier.stem_profile,
        candidate,
        Some((promoted.glyph_id, promoted.canonical_alias)),
        &mut shadow.beam_state.latest_base_apply.transaction_state,
        checker,
    )
    .map_err(|error| stage("HEADS-CLink-createStem", error))?;
    let NativeStemsBeamCreateStemDisposition::CreatedChecked { stem_identity } = create.disposition
    else {
        return Err(stage(
            "HEADS-CLink-createStem",
            "bounded frontier did not create a checked stem",
        ));
    };
    let mut stem = create
        .stem
        .clone()
        .ok_or_else(|| stage("HEADS-CLink-createStem", "created stem is absent"))?;
    let before_id = shadow
        .beam_state
        .latest_base_apply
        .transaction_state
        .glyph_index
        .persistent_ids
        .sheet_last_id;
    let inter_id = before_id
        .checked_add(1)
        .ok_or_else(|| stage("HEADS-CLink-InterIndex", "persistent ID overflow"))?;
    let stem_vertex = NativeSigVertexId(shadow.beam_state.sig.vertices.len());
    let grade = match &stem.grade {
        NativeStemsBeamStemGrade::Checked(check) => check.grade,
        NativeStemsBeamStemGrade::Artificial(grade) => *grade,
    };
    let bounds = stem.geometry.ribbon_bounds;
    shadow
        .beam_state
        .sig
        .append_vertex(NativeSigVertex {
            ordinal: stem_vertex.0,
            active: true,
            removed: false,
            kind: NativeSigInterKind::Stem,
            shape: Some("STEM".to_owned()),
            grade,
            bounds: crate::native_sig::NativeSigBounds {
                x: bounds.x,
                y: bounds.y,
                width: bounds.width,
                height: bounds.height,
            },
            abnormal: false,
            beam_geometry: None,
        })
        .map_err(|error| stage("HEADS-CLink-SIG-vertex", error))?;
    shadow
        .beam_state
        .bindings
        .bind_stem(stem_identity, stem_vertex)
        .map_err(|error| stage("HEADS-CLink-stem-binding", error))?;
    stem.inter_id = Some(inter_id);
    stem.sig_attached = true;
    let known = shadow
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter_mut()
        .find(|known| known.stem_identity == stem_identity)
        .ok_or_else(|| stage("HEADS-CLink-systemStems", "new stem insertion vanished"))?;
    *known = stem.clone();
    let ids = &mut shadow
        .beam_state
        .latest_base_apply
        .transaction_state
        .glyph_index
        .persistent_ids;
    ids.sheet_last_id = inter_id;
    ids.glyph_index_last_id = inter_id;
    ids.inter_index_last_id = inter_id;
    let inter_index = &mut shadow.beam_state.latest_base_apply.inter_index;
    let index_ordinal = inter_index.baseline_entry_count + inter_index.appended_entries.len();
    inter_index
        .appended_entries
        .push(NativeStemsBeamInterIndexAppend {
            index_ordinal,
            stem_identity,
            inter_id,
            vip: false,
        });
    inter_index.stem_lookup = NativeStemsBeamInterIndexLookup::PresentSameObject {
        index_ordinal,
        inter_id,
        vip: false,
        object_matches: 1,
        inter_id_matches: 1,
        glyph_active_matches: 0,
        glyph_original_matches: 0,
    };
    inter_index.next_id_lookup = NativeStemsBeamNextPersistentIdLookup::OccupiedByAppendedStem {
        persistent_id: inter_id,
        stem_identity,
    };

    let head_vertex = *shadow
        .beam_state
        .bindings
        .head_vertices
        .get(&frontier.next_corner.head)
        .ok_or_else(|| stage("HEADS-CLink-head-binding", "selected head is unbound"))?;
    if shadow
        .beam_state
        .sig
        .directed_edges(head_vertex.0, stem_vertex.0)
        .map_err(|error| stage("HEADS-CLink-pair", error))?
        .iter()
        .any(|edge| edge.kind == NativeSigRelationKind::HeadStem)
    {
        return Err(stage(
            "HEADS-CLink-pair",
            "HeadStem relation already exists",
        ));
    }
    let consistency = head_stem_consistency(
        stem.geometry.median.start.y,
        stem.geometry.median.stop.y,
        head_builders.interline,
    )?;
    let extension = relation.extension_point.ok_or_else(|| {
        stage(
            "HEADS-CLink-relation",
            "accepted relation has no extension point",
        )
    })?;
    let head_stem_edge = NativeSigEdgeId(shadow.beam_state.sig.edges.len());
    shadow
        .beam_state
        .sig
        .append_edge(NativeSigEdge {
            ordinal: head_stem_edge.0,
            active: true,
            source: head_vertex.0,
            target: stem_vertex.0,
            kind: NativeSigRelationKind::HeadStem,
            origin: NativeSigRelationOrigin::HeadCLinkDraft {
                head_sig_ordinal: frontier.next_corner.sig_ordinal,
                constructor_ordinal: selected_constructor_ordinal,
            },
            support: Some(NativeSigSupport {
                grade: relation.grade,
                bar_connection_impacts: None,
            }),
            beam_portion: None,
            stem_extension: None,
            head_stem: Some(NativeSigHeadStemPayload {
                dx: relation.dx,
                dy: relation.dy,
                head_side: relation.derived_horizontal,
                extension_point: extension,
                consistency,
                manual: false,
            }),
        })
        .map_err(|error| stage("HEADS-CLink-HeadStem", error))?;
    shadow
        .beam_state
        .sig
        .set_abnormal(head_vertex, false)
        .and_then(|()| shadow.beam_state.sig.set_abnormal(stem_vertex, false))
        .map_err(|error| stage("HEADS-CLink-callback", error))?;
    let s_ref = NativeStemsBeamHeadSLinkerRef {
        head: frontier.head,
        horizontal: relation.derived_horizontal,
    };
    let matching_count = shadow
        .beam_state
        .s_cells
        .iter()
        .filter(|cell| cell.reference == s_ref)
        .count();
    if matching_count != 1 {
        return Err(stage(
            "HEADS-CLink-S-cell",
            "selected parent S cell is missing or duplicated",
        ));
    }
    let cell = shadow
        .beam_state
        .s_cells
        .iter_mut()
        .find(|cell| cell.reference == s_ref)
        .expect("unique selected S cell was counted");
    let s_linked_before = cell.linked;
    cell.linked = true;
    let queued_cell = shadow
        .heads
        .get_mut(0)
        .and_then(|head| head.sides.iter_mut().find(|cell| cell.reference == s_ref))
        .ok_or_else(|| stage("HEADS-CLink-S-cell", "queued S-cell view is missing"))?;
    if queued_cell.linked != s_linked_before || queued_cell.closed != cell.closed {
        return Err(stage(
            "HEADS-CLink-S-cell",
            "queued and persistent S-cell views diverge before write",
        ));
    }
    queued_cell.linked = true;
    shadow.current_index = 1;
    shadow.frontier_consumed = true;
    shadow
        .beam_state
        .sig
        .validate_integrity()
        .map_err(|error| stage("HEADS-CLink-final-SIG", error))?;
    shadow
        .beam_state
        .bindings
        .validate_against(&shadow.beam_state.sig)
        .map_err(|error| stage("HEADS-CLink-final-bindings", error))?;

    let transaction = NativeStemsHeadCLinkTransaction {
        system_id: head_corners.system_id,
        corner: frontier.next_corner,
        last_index: 0,
        max_index: 0,
        selected_glyph_id: promoted.glyph_id,
        relation,
        create,
        stem_vertex,
        head_stem_edge,
        s_linker: s_ref,
        s_linked_before,
        s_linked_after: true,
        closed_cell_changes: 0,
    };
    *carrier = shadow;
    Ok(transaction)
}

fn glyph_centroid(
    glyph: &crate::native_stems_beam_vlink_transaction::NativeStemsBeamFixedGlyphContent,
) -> Result<(f64, f64), NativeStemsBeamSidesError> {
    let mut count = 0_usize;
    let mut x_total = 0_f64;
    let mut y_total = 0_f64;
    for sequence in (0..glyph.run_table.sequence_count()).rev() {
        for run in glyph
            .run_table
            .sequence(sequence)
            .unwrap_or_default()
            .iter()
            .rev()
        {
            for coordinate in (run.start..=run.stop()).rev() {
                match glyph.run_table.orientation() {
                    Orientation::Horizontal => {
                        x_total += glyph.bounds.x as f64 + coordinate as f64;
                        y_total += glyph.bounds.y as f64 + sequence as f64;
                    }
                    Orientation::Vertical => {
                        x_total += glyph.bounds.x as f64 + sequence as f64;
                        y_total += glyph.bounds.y as f64 + coordinate as f64;
                    }
                }
                count += 1;
            }
        }
    }
    if count != glyph.weight || count == 0 {
        return Err(stage(
            "HEADS-CLink-centroid",
            "candidate weight and materialized points differ",
        ));
    }
    Ok((x_total / count as f64, y_total / count as f64))
}

fn java_rint(value: f64) -> i32 {
    value.round_ties_even() as i32
}

fn head_stem_consistency(
    y1: f64,
    y2: f64,
    interline: i32,
) -> Result<f64, NativeStemsBeamSidesError> {
    if !y1.is_finite() || !y2.is_finite() || interline <= 0 {
        return Err(stage(
            "HEADS-CLink-consistency",
            "invalid stem consistency geometry",
        ));
    }
    let value = ((y2 - y1) / f64::from(interline)) / 2.8;
    if !value.is_finite() {
        return Err(stage(
            "HEADS-CLink-consistency",
            "non-finite stem consistency",
        ));
    }
    Ok(value)
}

/// Atomically remove the competing hook named by the current SIDES frontier
/// and continue until the next persistent frontier or SIDES exhaustion.
///
/// Java keeps the removed Inter in its persistent InterIndex but removes it
/// from the SIG and its beam group. Native persistent-ID evidence is therefore
/// retained while the live beam binding is removed.
pub fn remove_native_stems_beam_competing_hook_and_resume(
    carrier: &mut NativeStemsBeamSidesCarrier,
    context: NativeStemsBeamSidesContext<'_>,
) -> Result<NativeStemsBeamHookRemovalTransaction, NativeStemsBeamSidesError> {
    let mut shadow = carrier.clone();
    if !linked_b_cells_match(&shadow.scheduler.linked_b_linkers, &shadow.b_cells) {
        return Err(stage(
            "hook-removal-linked-B-authority",
            "scheduler linked-B set differs from persistent true cells",
        ));
    }
    let NativeStemsBeamSchedulerStatus::AwaitingHookRemovalTransaction(frontier) =
        &shadow.scheduler.status
    else {
        return Err(stage(
            "hook-removal-frontier",
            "scheduler is not awaiting competing-hook removal",
        ));
    };
    let frontier = frontier.as_ref().clone();
    if frontier.linked_sides
        != [
            crate::stems_step::NativeStemHeadSide::Left,
            crate::stems_step::NativeStemHeadSide::Right,
        ]
    {
        return Err(stage(
            "hook-removal-frontier",
            "full beam does not have both sides linked",
        ));
    }
    let scheduled = shadow
        .scheduler
        .beams_by_reverse_width
        .iter()
        .find(|scheduled| scheduled.source == frontier.beam)
        .ok_or_else(|| stage("hook-removal-frontier", "missing scheduled full beam"))?;
    if scheduled.kind == crate::beam_inters::BeamKind::Hook
        || scheduled.competing_hook != Some(frontier.competing_hook)
    {
        return Err(stage(
            "hook-removal-frontier",
            "scheduled competing-hook identity differs",
        ));
    }

    let beam_vertex = shadow
        .bindings
        .beam_vertices
        .get(&frontier.beam)
        .copied()
        .ok_or_else(|| stage("hook-removal-binding", "missing full-beam binding"))?;
    let hook_vertex = shadow
        .bindings
        .beam_vertices
        .get(&frontier.competing_hook)
        .copied()
        .ok_or_else(|| stage("hook-removal-binding", "missing hook binding"))?;
    if shadow.sig.vertex(beam_vertex.0).is_none_or(|vertex| {
        !matches!(
            vertex.kind,
            NativeSigInterKind::Beam | NativeSigInterKind::SmallBeam
        )
    }) || shadow
        .sig
        .vertex(hook_vertex.0)
        .is_none_or(|vertex| vertex.kind != NativeSigInterKind::BeamHook)
    {
        return Err(stage(
            "hook-removal-binding",
            "live full-beam/hook kinds differ",
        ));
    }
    let incident = shadow
        .sig
        .incident_edges(hook_vertex.0)
        .map_err(|error| stage("hook-removal-incident", error))?;
    let active_vertex_count_before = shadow
        .sig
        .vertices
        .iter()
        .filter(|vertex| vertex.active)
        .count();
    let active_edge_count_before = shadow.sig.edges.iter().filter(|edge| edge.active).count();
    let removed_edges = incident
        .iter()
        .map(|edge| NativeSigEdgeId(edge.ordinal))
        .collect::<Vec<_>>();
    let exclusion_count = incident
        .iter()
        .filter(|edge| {
            edge.kind == NativeSigRelationKind::Exclusion
                && ((edge.source == hook_vertex.0 && edge.target == beam_vertex.0)
                    || (edge.target == hook_vertex.0 && edge.source == beam_vertex.0))
        })
        .count();
    let containing_groups = incident
        .iter()
        .filter(|edge| {
            edge.kind == NativeSigRelationKind::Containment && edge.target == hook_vertex.0
        })
        .map(|edge| NativeSigVertexId(edge.source))
        .collect::<Vec<_>>();
    let [group_vertex] = containing_groups.as_slice() else {
        return Err(stage(
            "hook-removal-group",
            "hook must belong to exactly one live beam group",
        ));
    };
    if exclusion_count != 1
        || shadow
            .sig
            .vertex(group_vertex.0)
            .is_none_or(|vertex| vertex.kind != NativeSigInterKind::BeamGroup)
    {
        return Err(stage(
            "hook-removal-incident",
            "hook/full exclusion or group identity differs",
        ));
    }
    let group_members_before =
        live_group_member_sources(&shadow.sig, &shadow.bindings, *group_vertex)?;
    if group_members_before.len() < 2
        || !group_members_before.contains(&frontier.competing_hook)
        || !group_members_before.contains(&frontier.beam)
    {
        return Err(stage(
            "hook-removal-group",
            "bounded removal requires a surviving multi-member beam group",
        ));
    }

    shadow
        .sig
        .remove_vertex(hook_vertex)
        .map_err(|error| stage("hook-removal-SIG", error))?;
    let removed_binding = shadow
        .bindings
        .beam_vertices
        .remove(&frontier.competing_hook);
    if removed_binding != Some(hook_vertex) {
        return Err(stage(
            "hook-removal-binding",
            "removed hook binding differs",
        ));
    }
    let group_members_after =
        live_group_member_sources(&shadow.sig, &shadow.bindings, *group_vertex)?;
    let active_vertex_count_after = shadow
        .sig
        .vertices
        .iter()
        .filter(|vertex| vertex.active)
        .count();
    let active_edge_count_after = shadow.sig.edges.iter().filter(|edge| edge.active).count();
    if active_vertex_count_after.checked_add(1) != Some(active_vertex_count_before)
        || active_edge_count_after.checked_add(removed_edges.len())
            != Some(active_edge_count_before)
    {
        return Err(stage(
            "hook-removal-SIG",
            "active graph delta differs from one vertex and its incident edges",
        ));
    }
    let expected_after = group_members_before
        .iter()
        .copied()
        .filter(|source| *source != frontier.competing_hook)
        .collect::<Vec<_>>();
    if group_members_after != expected_after {
        return Err(stage(
            "hook-removal-group",
            "group membership delta is not hook-only",
        ));
    }
    shadow
        .sig
        .validate_integrity()
        .map_err(|error| stage("hook-removal-SIG", error))?;
    shadow
        .bindings
        .validate_against(&shadow.sig)
        .map_err(|error| stage("hook-removal-bindings", error))?;
    let resume = resume_native_stems_beam_scheduler_after_hook_removal(
        &shadow.scheduler,
        context.vlinkers,
        context.builders,
        context.plans,
        frontier.competing_hook,
    )
    .map_err(|error| stage("hook-removal-resume", error))?;
    shadow.scheduler = (*resume.advanced_system).clone();
    if !linked_b_cells_match(&shadow.scheduler.linked_b_linkers, &shadow.b_cells) {
        return Err(stage(
            "hook-removal-linked-B-commit",
            "continuation changed the persistent linked-B view",
        ));
    }

    let transaction = NativeStemsBeamHookRemovalTransaction {
        system_id: shadow.scheduler.system_id,
        beam: frontier.beam,
        competing_hook: frontier.competing_hook,
        hook_vertex,
        group_vertex: *group_vertex,
        removed_edges,
        active_vertex_count_before,
        active_vertex_count_after,
        active_edge_count_before,
        active_edge_count_after,
        group_members_before,
        group_members_after,
        resume,
    };
    *carrier = shadow;
    Ok(transaction)
}

/// Atomically leave an exhausted SIDES carrier and enter its STUMPS worklist.
///
/// The scheduler's carried linked-B set is accepted only when it is a bijective
/// view of the persistent true B cells. No graph, binding, registry, or shared
/// linker cell is changed by this read-only scheduler continuation.
pub fn continue_native_stems_beam_sides_carrier_into_stumps(
    carrier: &mut NativeStemsBeamSidesCarrier,
    context: NativeStemsBeamSidesContext<'_>,
) -> Result<NativeStemsBeamSchedulerStumpsContinuation, NativeStemsBeamSidesError> {
    let mut shadow = carrier.clone();
    if !linked_b_cells_match(&shadow.scheduler.linked_b_linkers, &shadow.b_cells) {
        return Err(stage(
            "STUMPS-linked-B-authority",
            "scheduler linked-B set differs from persistent true cells",
        ));
    }
    let unchanged = (
        shadow.latest_base_apply.clone(),
        shadow.sig.clone(),
        shadow.bindings.clone(),
        shadow.b_cells.clone(),
        shadow.s_cells.clone(),
        shadow.beam_inter_index.clone(),
        shadow.configured_inter_vip_ids.clone(),
    );
    let continuation = continue_native_stems_beam_scheduler_into_stumps(
        &shadow.scheduler,
        context.stumps,
        context.vlinkers,
        context.builders,
        context.plans,
    )
    .map_err(|error| stage("STUMPS-scheduler", error))?;
    shadow.scheduler = (*continuation.advanced_system).clone();
    if unchanged
        != (
            shadow.latest_base_apply.clone(),
            shadow.sig.clone(),
            shadow.bindings.clone(),
            shadow.b_cells.clone(),
            shadow.s_cells.clone(),
            shadow.beam_inter_index.clone(),
            shadow.configured_inter_vip_ids.clone(),
        )
    {
        return Err(stage(
            "STUMPS-atomicity",
            "scheduler continuation changed persistent carrier state",
        ));
    }
    *carrier = shadow;
    Ok(continuation)
}

fn linked_b_cells_match(
    linked: &[crate::native_stems_beam_vlinkers::NativeStemsBeamBLinkerRef],
    cells: &[NativeStemsBeamNativeBLinkerCell],
) -> bool {
    if cells.iter().enumerate().any(|(index, cell)| {
        cells[..index]
            .iter()
            .any(|prior| prior.reference == cell.reference)
    }) || linked
        .iter()
        .enumerate()
        .any(|(index, reference)| linked[..index].contains(reference))
    {
        return false;
    }
    let true_cells = cells
        .iter()
        .filter(|cell| cell.linked)
        .map(|cell| cell.reference)
        .collect::<Vec<_>>();
    true_cells.len() == linked.len()
        && true_cells
            .iter()
            .all(|reference| linked.contains(reference))
        && linked
            .iter()
            .all(|reference| true_cells.contains(reference))
}

fn live_group_member_sources(
    sig: &NativeSigSystem,
    bindings: &NativeSigSystemBindings,
    group: NativeSigVertexId,
) -> Result<Vec<crate::native_stems_beam_stumps::NativeStemsBeamSource>, NativeStemsBeamSidesError>
{
    let mut members = Vec::new();
    for edge in sig
        .outgoing_edges(group.0)
        .map_err(|error| stage("hook-removal-group", error))?
    {
        if edge.kind != NativeSigRelationKind::Containment {
            return Err(stage(
                "hook-removal-group",
                "beam group has a non-containment outgoing relation",
            ));
        }
        let sources = bindings
            .beam_vertices
            .iter()
            .filter(|(_, vertex)| vertex.0 == edge.target)
            .map(|(source, _)| *source)
            .collect::<Vec<_>>();
        let [source] = sources.as_slice() else {
            return Err(stage(
                "hook-removal-group",
                "group member has no unique live beam binding",
            ));
        };
        members.push(*source);
    }
    Ok(members)
}

fn current_stumps_status(
    scheduler: &NativeStemsBeamSchedulerSystem,
) -> Result<NativeStemsBeamSchedulerStumpsStatus, NativeStemsBeamSidesError> {
    match &scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier)
            if frontier_matches_carrier_pass(frontier, CarrierPass::Stumps) =>
        {
            Ok(NativeStemsBeamSchedulerStumpsStatus::AwaitingVLinkTransaction(frontier.clone()))
        }
        NativeStemsBeamSchedulerStatus::Completed {
            retained_for_stumps,
            final_local_worklist,
        } => Ok(NativeStemsBeamSchedulerStumpsStatus::Completed {
            retained_for_stumps: retained_for_stumps.clone(),
            final_local_worklist: final_local_worklist.clone(),
        }),
        _ => Err(stage(
            "STUMPS-drive-status",
            "scheduler is neither awaiting a typed STUMPS frontier nor complete",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        native_stems_beam_stumps::NativeStemsBeamSource,
        native_stems_beam_vlinkers::NativeStemsBeamBLinkerRef,
    };

    #[test]
    fn linked_b_authority_is_an_exact_true_cell_bijection() {
        let one = NativeStemsBeamBLinkerRef {
            beam: NativeStemsBeamSource::RawBeam(1),
            id: 1,
        };
        let two = NativeStemsBeamBLinkerRef {
            beam: NativeStemsBeamSource::RawBeam(2),
            id: 1,
        };
        let cells = [
            NativeStemsBeamNativeBLinkerCell {
                reference: one,
                linked: true,
                closed: false,
            },
            NativeStemsBeamNativeBLinkerCell {
                reference: two,
                linked: false,
                closed: false,
            },
        ];
        assert!(linked_b_cells_match(&[one], &cells));
        assert!(!linked_b_cells_match(&[one, two], &cells));
        assert!(!linked_b_cells_match(&[], &cells));
        let mut closed = cells;
        closed[1].closed = true;
        assert!(linked_b_cells_match(&[one], &closed));
        closed[0].closed = true;
        assert!(linked_b_cells_match(&[one], &closed));
    }

    #[test]
    fn stumps_drive_accepts_only_awaiting_stumps_or_true_completion() {
        let retained = vec![NativeStemsBeamSource::RawBeam(1)];
        let worklist = vec![NativeStemsBeamSource::RawBeam(2)];
        let mut scheduler = NativeStemsBeamSchedulerSystem {
            system_id: 1,
            link_profile: 1,
            glyphs_in_sig_order: Vec::new(),
            live_exclusions: Vec::new(),
            beams_by_reverse_width: Vec::new(),
            prefix_events: Vec::new(),
            deferred_line_deltas: Vec::new(),
            consumed_v_linkers: Vec::new(),
            linked_b_linkers: Vec::new(),
            status: NativeStemsBeamSchedulerStatus::Completed {
                retained_for_stumps: retained.clone(),
                final_local_worklist: worklist.clone(),
            },
        };
        assert_eq!(
            current_stumps_status(&scheduler).expect("true completion"),
            NativeStemsBeamSchedulerStumpsStatus::Completed {
                retained_for_stumps: retained.clone(),
                final_local_worklist: worklist.clone(),
            }
        );

        scheduler.status = NativeStemsBeamSchedulerStatus::SidesExhausted {
            retained_for_stumps: retained,
            final_local_worklist: worklist,
        };
        assert_eq!(
            current_stumps_status(&scheduler)
                .expect_err("pre-STUMPS terminal must not masquerade as completion")
                .stage,
            "STUMPS-drive-status"
        );
    }

    #[test]
    fn hook_group_members_follow_live_containment_order_and_bindings() {
        use std::collections::BTreeMap;

        use crate::native_sig::{
            NativeSigBounds, NativeSigEdge, NativeSigRelationOrigin, NativeSigVertex,
        };

        let hook = NativeStemsBeamSource::RawBeam(4);
        let beam = NativeStemsBeamSource::RawBeam(5);
        let vertex = |ordinal, kind| NativeSigVertex {
            ordinal,
            active: true,
            removed: false,
            kind,
            shape: None,
            grade: 1.0,
            bounds: NativeSigBounds {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            abnormal: false,
            beam_geometry: None,
        };
        let containment = |ordinal, target| NativeSigEdge {
            ordinal,
            active: true,
            source: 0,
            target,
            kind: NativeSigRelationKind::Containment,
            origin: NativeSigRelationOrigin::BaselineGraph,
            support: None,
            beam_portion: None,
            stem_extension: None,
            head_stem: None,
        };
        let sig = NativeSigSystem {
            system_id: 1,
            vertices: vec![
                vertex(0, NativeSigInterKind::BeamGroup),
                vertex(1, NativeSigInterKind::BeamHook),
                vertex(2, NativeSigInterKind::Beam),
            ],
            edges: vec![containment(0, 1), containment(1, 2)],
        };
        let mut bindings = NativeSigSystemBindings {
            system_id: 1,
            beam_vertices: BTreeMap::from([
                (hook, NativeSigVertexId(1)),
                (beam, NativeSigVertexId(2)),
            ]),
            beam_group_vertices: BTreeMap::from([(7, NativeSigVertexId(0))]),
            stem_vertices: BTreeMap::new(),
            head_vertices: BTreeMap::new(),
        };
        assert_eq!(
            live_group_member_sources(&sig, &bindings, NativeSigVertexId(0))
                .expect("ordered live members"),
            [hook, beam]
        );
        bindings.beam_vertices.remove(&hook);
        assert!(live_group_member_sources(&sig, &bindings, NativeSigVertexId(0)).is_err());
    }

    #[test]
    fn stump_carrier_requires_stumps_profile_and_no_horizontal_side() {
        use crate::{
            native_stems_beam_link_plans::NativeStemsBeamLinkPlanOutcome,
            native_stems_beam_scheduler::{
                NativeStemsBeamAwaitingVLinkTransaction, NativeStemsBeamPlanRef,
                NativeStemsBeamWorklistSnapshot,
            },
            native_stems_beam_vlinkers::NativeStemsBeamVLinkerRef,
            stems_step::NativeStemVerticalSide,
        };

        let beam = NativeStemsBeamSource::RawBeam(12);
        let b_linker = NativeStemsBeamBLinkerRef { beam, id: 2 };
        let v_linker = NativeStemsBeamVLinkerRef {
            b_linker,
            side: NativeStemVerticalSide::Top,
        };
        let mut frontier = NativeStemsBeamAwaitingVLinkTransaction {
            invocation_ordinal: 32,
            snapshot: NativeStemsBeamWorklistSnapshot {
                pass: NativeStemsBeamSchedulerPass::Stumps,
                current_index: 0,
                sources: vec![beam],
                current: beam,
                remaining: Vec::new(),
            },
            beam,
            horizontal_side: None,
            b_linker,
            v_linker,
            vertical_side: NativeStemVerticalSide::Top,
            plan: NativeStemsBeamPlanRef {
                system_id: 1,
                plan_ordinal: 147,
                builder_ordinal: 0,
                stem_profile: BEAM_SEED_PROFILE,
            },
            outcome: NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem,
            linked_sides_before: Vec::new(),
            retained_beams_before: Vec::new(),
            would_apply_stored_line_delta: None,
        };
        assert!(frontier_matches_carrier_pass(
            &frontier,
            CarrierPass::Stumps
        ));
        assert!(!frontier_matches_carrier_pass(
            &frontier,
            CarrierPass::Sides
        ));

        frontier.plan.stem_profile -= 1;
        assert!(!frontier_matches_carrier_pass(
            &frontier,
            CarrierPass::Stumps
        ));
    }
}

#[derive(Clone, Copy)]
enum GlyphAuthority<'a> {
    Legacy(NativeStemsBeamSidesGlyphEvidence<'a>),
    FirstStems(&'a NativeStemsFirstGlyphIndexBridge),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CarrierPass {
    Sides,
    Stumps,
}

fn frontier_matches_carrier_pass(
    frontier: &crate::native_stems_beam_scheduler::NativeStemsBeamAwaitingVLinkTransaction,
    pass: CarrierPass,
) -> bool {
    match pass {
        CarrierPass::Sides => {
            frontier.snapshot.pass == NativeStemsBeamSchedulerPass::Sides
                && frontier.horizontal_side.is_some()
        }
        CarrierPass::Stumps => {
            frontier.snapshot.pass == NativeStemsBeamSchedulerPass::Stumps
                && frontier.horizontal_side.is_none()
                && frontier.plan.stem_profile == BEAM_SEED_PROFILE
                && frontier.linked_sides_before.is_empty()
        }
    }
}

enum CarrierTerminal {
    Sides(NativeStemsBeamNativeOuterResumeTransaction),
    Stumps(NativeStemsBeamSchedulerStumpsContinuation),
}

struct NativeStemsBeamCarrierTransaction {
    preparation: NativeStemsBeamFrontierPreparation,
    create: NativeStemsBeamVLinkTransaction,
    reuse_live_state: NativeStemsBeamVLinkReuseLiveState,
    reuse: NativeStemsBeamVLinkReuseCheck,
    base: NativeStemsBeamVLinkBaseApplyTransaction,
    flag: NativeStemsBeamVLinkBLinkerFlagTransaction,
    siblings: NativeStemsBeamNativeSiblingTransaction,
    heads: NativeStemsBeamNativeHeadTransaction,
    terminal: CarrierTerminal,
}

impl NativeStemsBeamCarrierTransaction {
    fn into_sides(self) -> Result<NativeStemsBeamSidesTransaction, NativeStemsBeamSidesError> {
        let CarrierTerminal::Sides(outer_resume) = self.terminal else {
            return Err(stage("carrier-pass", "STUMPS terminal returned for SIDES"));
        };
        Ok(NativeStemsBeamSidesTransaction {
            preparation: self.preparation,
            create: self.create,
            reuse_live_state: self.reuse_live_state,
            reuse: self.reuse,
            base: self.base,
            flag: self.flag,
            siblings: self.siblings,
            heads: self.heads,
            outer_resume,
        })
    }

    fn into_stumps(self) -> Result<NativeStemsBeamStumpsTransaction, NativeStemsBeamSidesError> {
        let CarrierTerminal::Stumps(resume) = self.terminal else {
            return Err(stage("carrier-pass", "SIDES terminal returned for STUMPS"));
        };
        Ok(NativeStemsBeamStumpsTransaction {
            preparation: self.preparation,
            create: self.create,
            reuse_live_state: self.reuse_live_state,
            reuse: self.reuse,
            base: self.base,
            flag: self.flag,
            siblings: self.siblings,
            heads: self.heads,
            resume,
        })
    }
}

fn advance_native_stems_beam_sides_transaction_with_authority(
    carrier: &mut NativeStemsBeamSidesCarrier,
    context: NativeStemsBeamSidesContext<'_>,
    glyphs: GlyphAuthority<'_>,
    pass: CarrierPass,
) -> Result<NativeStemsBeamCarrierTransaction, NativeStemsBeamSidesError> {
    let mut shadow = carrier.clone();
    let frontier = match &shadow.scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => {
            return Err(stage(
                "carrier-pass",
                "scheduler is not awaiting a V frontier",
            ));
        }
    };
    if !frontier_matches_carrier_pass(frontier, pass) {
        return Err(stage(
            "carrier-pass",
            "frontier pass/profile/horizontal-side semantics differ",
        ));
    }
    if pass == CarrierPass::Stumps
        && !linked_b_cells_match(&shadow.scheduler.linked_b_linkers, &shadow.b_cells)
    {
        return Err(stage(
            "STUMPS-linked-B-authority",
            "scheduler linked-B set differs from persistent true cells",
        ));
    }
    let relation_parameters = NativeStemsBeamRelationParameters {
        profile: frontier.plan.stem_profile,
        ..context.relation_parameters
    };
    reconcile_known_stems(&mut shadow.latest_base_apply, &shadow.sig, &shadow.bindings)?;
    let mut transaction_state = shadow.latest_base_apply.transaction_state.clone();
    let proof = NativeStemsBeamSystemStemAuthorityProof::from_empty_stems_entry(
        &transaction_state.system_stems,
        0,
    )
    .map_err(|error| stage("system-stem-authority", error))?;
    let preparation = match glyphs {
        GlyphAuthority::Legacy(glyphs) => prepare_native_stems_beam_vlink_frontier_state(
            &shadow.scheduler,
            context.plans,
            &mut transaction_state,
            glyphs.selected,
            proof,
        ),
        GlyphAuthority::FirstStems(bridge) => {
            prepare_native_stems_beam_vlink_frontier_state_from_first_stems_bridge(
                &shadow.scheduler,
                context.plans,
                &mut transaction_state,
                bridge,
                proof,
            )
        }
    }
    .map_err(|error| stage("B12-preparation", error))?;
    let candidate = materialize_native_stems_beam_frontier_candidate(
        &shadow.scheduler,
        context.plans,
        &transaction_state,
    )
    .map_err(|error| stage("B12-candidate", error))?;
    match glyphs {
        GlyphAuthority::Legacy(glyphs) => {
            if let Some(scan) = glyphs.exhaustive_candidate {
                if scan.candidate != candidate {
                    return Err(stage("B12-glyph-authority", "candidate differs"));
                }
                transaction_state.glyph_index.exhaustive_lookup = Some(scan.clone());
            }
        }
        GlyphAuthority::FirstStems(_) => {
            if !transaction_state
                .selected_glyph_bindings
                .iter()
                .any(|selected| {
                    preparation.selected_glyphs.contains(&selected.reference)
                        && selected.content == candidate
                })
            {
                return Err(stage(
                    "B12-glyph-authority",
                    "candidate is not a current selected modeled canonical",
                ));
            }
            transaction_state.glyph_index.exhaustive_lookup = None;
        }
    }
    let create = apply_native_stems_beam_vlink_create_stem_transaction(
        &shadow.scheduler,
        context.builders,
        context.plans,
        &mut transaction_state,
        context.checker,
    )
    .map_err(|error| stage("B12", error))?;
    let reuse_live_state = project_native_stems_beam_vlink_reuse_live_state(
        &shadow.sig,
        &shadow.bindings,
        &shadow.scheduler,
        context.plans,
        &shadow.s_cells,
        &transaction_state.system_stems,
    )
    .map_err(|error| stage("B13-live-state", error))?;
    let reuse = evaluate_native_stems_beam_vlink_reuse_check(
        &shadow.scheduler,
        context.plans,
        context.stumps,
        context.vlinkers,
        &create,
        &transaction_state,
        &reuse_live_state,
        relation_parameters,
    )
    .map_err(|error| stage("B13", error))?;
    let mut base_state = roll_native_stems_beam_vlink_base_apply_state(
        &shadow.latest_base_apply,
        &transaction_state,
        &reuse,
        &shadow.sig,
        &shadow.bindings,
        NativeStemsBeamVLinkBaseRolloverAuthority {
            stump_system: context.stumps,
            beam_inter_index: &shadow.beam_inter_index,
            configured_inter_vip_ids: &shadow.configured_inter_vip_ids,
        },
    )
    .map_err(|error| stage("B14-rollover", error))?;
    let flag_base_state = base_state.clone();
    let base = apply_native_stems_beam_vlink_base_transaction_to_native_sig(
        &shadow.scheduler,
        context.plans,
        context.stumps,
        context.vlinkers,
        &create,
        &reuse_live_state,
        relation_parameters,
        &reuse,
        &mut base_state,
        &mut shadow.sig,
        &mut shadow.bindings,
    )
    .map_err(|error| stage("B14", error))?;
    let target = match &shadow.scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.b_linker,
        _ => return Err(stage("B15-target", "scheduler is not awaiting a frontier")),
    };
    let linked = shadow
        .b_cells
        .iter()
        .filter(|cell| cell.reference == target)
        .map(|cell| cell.linked)
        .collect::<Vec<_>>();
    let [linked] = linked.as_slice() else {
        return Err(stage("B15-target", "shared B-cell cardinality is not one"));
    };
    let mut flag_state = NativeStemsBeamVLinkBLinkerFlagState {
        system_id: shadow.scheduler.system_id,
        base_apply_state_before: flag_base_state,
        target_b_linker: target,
        linked: *linked,
        committed: None,
    };
    let flag = apply_native_stems_beam_vlink_b_linker_flag_transaction(
        &shadow.scheduler,
        context.plans,
        context.stumps,
        context.vlinkers,
        &create,
        &reuse_live_state,
        relation_parameters,
        &reuse,
        &base,
        &mut flag_state,
    )
    .map_err(|error| stage("B15", error))?;
    let siblings = apply_native_stems_beam_vlink_sibling_transaction_to_native_sig(
        &mut shadow.sig,
        &shadow.bindings,
        &shadow.scheduler,
        context.stumps,
        context.vlinkers,
        context.reachability,
        context.builders,
        &base,
        &flag,
        &mut shadow.b_cells,
    )
    .map_err(|error| stage("B16", error))?;
    let heads = apply_native_stems_beam_vlink_head_transaction_to_native_sig(
        &mut shadow.sig,
        &shadow.bindings,
        &shadow.scheduler,
        context.plans,
        context.builders,
        context.head_corners,
        context.reachability,
        &flag,
        &siblings,
        &shadow.b_cells,
        &mut shadow.s_cells,
    )
    .map_err(|error| stage("B17", error))?;
    let terminal = match pass {
        CarrierPass::Sides => CarrierTerminal::Sides(
            apply_native_stems_beam_outer_and_resume_transaction(
                &shadow.scheduler,
                context.vlinkers,
                context.builders,
                context.plans,
                context.reachability,
                &flag,
                &siblings,
                &heads,
                &mut shadow.b_cells,
            )
            .map_err(|error| stage("B18/B19", error))?,
        ),
        CarrierPass::Stumps => {
            if !flag.linked_after
                || siblings.system_id != shadow.scheduler.system_id
                || heads.system_id != shadow.scheduler.system_id
                || siblings.plan_ordinal != flag.key.plan.plan_ordinal
                || heads.plan_ordinal != flag.key.plan.plan_ordinal
                || siblings.stem_identity != heads.stem_identity
                || !heads.returned_true
            {
                return Err(stage(
                    "B19-STUMPS-predecessor",
                    "B15-B17 terminal join differs",
                ));
            }
            let completed = NativeStemsBeamCompletedStumpVLinkEvidence {
                plan: flag.key.plan,
                b_linker: flag.target_b_linker,
                v_linker: flag.triggering_v_linker,
                b15_linked_after: flag.linked_after,
                sibling_linked_b_linkers: siblings.assigned_b_linkers.clone(),
            };
            CarrierTerminal::Stumps(
                resume_native_stems_beam_scheduler_after_stumps_transaction(
                    &shadow.scheduler,
                    context.stumps,
                    context.vlinkers,
                    context.builders,
                    context.plans,
                    &completed,
                )
                .map_err(|error| stage("B19-STUMPS", error))?,
            )
        }
    };
    let mut carried_base = (*base.state_after).clone();
    reconcile_known_stems(&mut carried_base, &shadow.sig, &shadow.bindings)?;
    shadow.latest_base_apply = carried_base;
    shadow.scheduler = match &terminal {
        CarrierTerminal::Sides(outer_resume) => (*outer_resume.resume.advanced_system).clone(),
        CarrierTerminal::Stumps(resume) => (*resume.advanced_system).clone(),
    };
    if pass == CarrierPass::Stumps
        && !linked_b_cells_match(&shadow.scheduler.linked_b_linkers, &shadow.b_cells)
    {
        return Err(stage(
            "STUMPS-linked-B-commit",
            "resumed scheduler differs from committed B15/B16 cells",
        ));
    }
    shadow
        .sig
        .validate_integrity()
        .map_err(|error| stage("post-transaction-SIG", error))?;
    shadow
        .bindings
        .validate_against(&shadow.sig)
        .map_err(|error| stage("post-transaction-bindings", error))?;

    let transaction = NativeStemsBeamCarrierTransaction {
        preparation,
        create,
        reuse_live_state,
        reuse,
        base,
        flag,
        siblings,
        heads,
        terminal,
    };
    *carrier = shadow;
    Ok(transaction)
}
