//! Atomic carriage of one already-awaited STEMS SIDES transaction.

use std::{collections::BTreeSet, error::Error, fmt};

use audiveris_image::{
    beam_structure::Segment,
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable},
};

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
        NativeStemsBeamInterIndexAppend, NativeStemsBeamInterIndexLookup,
        NativeStemsBeamNextPersistentIdLookup, NativeStemsBeamSheetEditState,
        NativeStemsBeamVLinkBaseApplyState, NativeStemsBeamVLinkBaseApplyTransaction,
        NativeStemsBeamVLinkBaseRolloverAuthority,
        apply_native_stems_beam_vlink_base_transaction_to_native_sig,
        initialize_native_stems_beam_vlink_base_apply_state_from_native_sig,
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
        initialize_native_stems_beam_b_linker_cells,
    },
    native_stems_beam_vlink_transaction::{
        NativeStemsBeamCreateStemDisposition, NativeStemsBeamExhaustiveGlyphEqualsScan,
        NativeStemsBeamFrontierPreparation, NativeStemsBeamGlyphRegistryBootstrapEntry,
        NativeStemsBeamStemCheckerContext, NativeStemsBeamStemGrade,
        NativeStemsBeamSystemStemAuthorityProof, NativeStemsBeamVLinkTransaction,
        NativeStemsCreateStemCandidateTransaction, NativeStemsFirstGlyphIndexBridge,
        NativeStemsGlyphRegistryAuthority, NativeStemsModeledGlyphRegistry,
        apply_native_stems_beam_vlink_create_stem_transaction,
        apply_native_stems_create_stem_candidate_transaction,
        initialize_native_stems_beam_vlink_first_frontier_state_from_modeled_registry,
        materialize_native_stems_beam_frontier_candidate,
        prepare_native_stems_beam_vlink_frontier_state,
        prepare_native_stems_beam_vlink_frontier_state_from_first_stems_bridge,
        prepare_native_stems_beam_vlink_frontier_state_from_modeled_registry,
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
    /// Java runs heads linking phase 2 over `unlinkedHeads` after phase 1
    /// completes (StemsRetriever.linkStems); this is the cursor into that
    /// queue, and stays 0 for the whole of phase 1.
    pub phase_two_index: usize,
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

/// One completed outer `HeadLinker.linkSides` phase-1 call.
///
/// The carried state is returned separately from the ordered closure writes so
/// callers can grade Java's control result without treating event evidence as
/// persistent scheduler state.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadPhase1Continuation {
    pub processed_head: NativeStemsBeamHeadLinkHeadRef,
    pub side_decisions: Vec<NativeStemsHeadPhase1SideDecision>,
    /// `None` stops at an awaited C-link frontier; `Some` is Java's actual
    /// `HeadLinker.linkSides` return after the call has completed.
    pub returned_linked: Option<bool>,
    /// Java write order: linked side, current HeadStem relation, opposite
    /// Stem's HeadStem relation, then LEFT/RIGHT on each other head.
    pub closed_s_linkers: Vec<NativeStemsBeamHeadSLinkerRef>,
    pub closed_value_changes: usize,
    pub state_after: Box<NativeStemsHeadPhase1Carrier>,
}

/// Exact no-op result of Java `StemsRetriever.finalizeStems` for the
/// authenticated Chula system-1 carrier.
///
/// `checkHeadStems` has no multi-stem candidate to clean, while
/// `checkNeededStems` finds precisely two stemless void heads that are
/// already abnormal.  The returned carrier therefore remains byte-for-byte
/// equal to the completed phase-2 input.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsFinalizeTransaction {
    pub checked_heads: usize,
    pub multiple_stem_heads: Vec<NativeStemsBeamHeadLinkHeadRef>,
    pub no_stem_heads: Vec<NativeStemsBeamHeadLinkHeadRef>,
    pub abnormal_heads: Vec<NativeStemsBeamHeadLinkHeadRef>,
    pub removed_head_stem_relations: Vec<NativeSigEdgeId>,
    pub abnormal_value_changes: usize,
    pub state_after: Box<NativeStemsHeadPhase1Carrier>,
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

/// Execute one frontier from the owned native canonical-glyph registry.
pub fn advance_native_stems_beam_sides_transaction_from_modeled_registry(
    carrier: &mut NativeStemsBeamSidesCarrier,
    context: NativeStemsBeamSidesContext<'_>,
    registry: &NativeStemsModeledGlyphRegistry,
) -> Result<NativeStemsBeamSidesTransaction, NativeStemsBeamSidesError> {
    advance_native_stems_beam_sides_transaction_with_authority(
        carrier,
        context,
        GlyphAuthority::Modeled(registry),
        CarrierPass::Sides,
    )
    .and_then(NativeStemsBeamCarrierTransaction::into_sides)
}

/// Execute the first SIDES frontier and construct its production carrier.
///
/// All mutable graph, binding, B-cell, and S-cell authorities are cloned or
/// initialized locally. A failure at any B12-B19 boundary returns no partial
/// carrier.
pub fn initialize_native_stems_beam_sides_carrier_from_modeled_registry(
    scheduler: &NativeStemsBeamSchedulerSystem,
    sig: &NativeSigSystem,
    bindings: &NativeSigSystemBindings,
    context: NativeStemsBeamSidesContext<'_>,
    registry: &NativeStemsModeledGlyphRegistry,
    sheet_edit: NativeStemsBeamSheetEditState,
) -> Result<(NativeStemsBeamSidesCarrier, NativeStemsBeamSidesTransaction), NativeStemsBeamSidesError>
{
    let frontier = match &scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => {
            return Err(stage(
                "first-carrier-pass",
                "scheduler is not awaiting a V frontier",
            ));
        }
    };
    if !frontier_matches_carrier_pass(frontier, CarrierPass::Sides) {
        return Err(stage(
            "first-carrier-pass",
            "frontier is not a SIDES transaction",
        ));
    }
    sig.validate_integrity()
        .map_err(|error| stage("first-carrier-SIG", error))?;
    bindings
        .validate_against(sig)
        .map_err(|error| stage("first-carrier-bindings", error))?;

    let relation_parameters = NativeStemsBeamRelationParameters {
        profile: frontier.plan.stem_profile,
        ..context.relation_parameters
    };
    let (preparation, mut transaction_state) =
        initialize_native_stems_beam_vlink_first_frontier_state_from_modeled_registry(
            scheduler,
            context.plans,
            registry,
        )
        .map_err(|error| stage("first-B12-preparation", error))?;
    let candidate = materialize_native_stems_beam_frontier_candidate(
        scheduler,
        context.plans,
        &transaction_state,
    )
    .map_err(|error| stage("first-B12-candidate", error))?;
    if !transaction_state
        .selected_glyph_bindings
        .iter()
        .any(|selected| {
            preparation.selected_glyphs.contains(&selected.reference)
                && selected.content == candidate
        })
    {
        return Err(stage(
            "first-B12-glyph-authority",
            "candidate is not a current selected modeled canonical",
        ));
    }
    transaction_state.glyph_index.exhaustive_lookup = None;

    let create = apply_native_stems_beam_vlink_create_stem_transaction(
        scheduler,
        context.builders,
        context.plans,
        &mut transaction_state,
        context.checker,
    )
    .map_err(|error| stage("first-B12", error))?;
    let mut s_cells = initialize_native_stems_beam_s_linker_cells(context.head_corners)
        .map_err(|error| stage("first-S-cells", error))?;
    let reuse_live_state = project_native_stems_beam_vlink_reuse_live_state(
        sig,
        bindings,
        scheduler,
        context.plans,
        &s_cells,
        &transaction_state.system_stems,
    )
    .map_err(|error| stage("first-B13-live-state", error))?;
    let reuse = evaluate_native_stems_beam_vlink_reuse_check(
        scheduler,
        context.plans,
        context.stumps,
        context.vlinkers,
        &create,
        &transaction_state,
        &reuse_live_state,
        relation_parameters,
    )
    .map_err(|error| stage("first-B13", error))?;
    let mut base_state = initialize_native_stems_beam_vlink_base_apply_state_from_native_sig(
        &transaction_state,
        &reuse,
        sig,
        bindings,
        context.stumps,
        sheet_edit,
    )
    .map_err(|error| stage("first-B14-initialize", error))?;
    let flag_base_state = base_state.clone();
    let mut carried_sig = sig.clone();
    let mut carried_bindings = bindings.clone();
    let base = apply_native_stems_beam_vlink_base_transaction_to_native_sig(
        scheduler,
        context.plans,
        context.stumps,
        context.vlinkers,
        &create,
        &reuse_live_state,
        relation_parameters,
        &reuse,
        &mut base_state,
        &mut carried_sig,
        &mut carried_bindings,
    )
    .map_err(|error| stage("first-B14", error))?;

    let mut b_cells = initialize_native_stems_beam_b_linker_cells(context.reachability)
        .map_err(|error| stage("first-B-cells", error))?;
    let linked = b_cells
        .iter()
        .filter(|cell| cell.reference == frontier.b_linker)
        .map(|cell| cell.linked)
        .collect::<Vec<_>>();
    let [linked] = linked.as_slice() else {
        return Err(stage(
            "first-B15-target",
            "shared B-cell cardinality is not one",
        ));
    };
    let mut flag_state = NativeStemsBeamVLinkBLinkerFlagState {
        system_id: scheduler.system_id,
        base_apply_state_before: flag_base_state,
        target_b_linker: frontier.b_linker,
        linked: *linked,
        committed: None,
    };
    let flag = apply_native_stems_beam_vlink_b_linker_flag_transaction(
        scheduler,
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
    .map_err(|error| stage("first-B15", error))?;
    let siblings = apply_native_stems_beam_vlink_sibling_transaction_to_native_sig(
        &mut carried_sig,
        &carried_bindings,
        scheduler,
        context.stumps,
        context.vlinkers,
        context.reachability,
        context.builders,
        &base,
        &flag,
        &mut b_cells,
    )
    .map_err(|error| stage("first-B16", error))?;
    let heads = apply_native_stems_beam_vlink_head_transaction_to_native_sig(
        &mut carried_sig,
        &carried_bindings,
        scheduler,
        context.plans,
        context.builders,
        context.head_corners,
        context.reachability,
        &flag,
        &siblings,
        &b_cells,
        &mut s_cells,
    )
    .map_err(|error| stage("first-B17", error))?;
    let outer_resume = apply_native_stems_beam_outer_and_resume_transaction(
        scheduler,
        context.vlinkers,
        context.builders,
        context.plans,
        context.reachability,
        &flag,
        &siblings,
        &heads,
        &mut b_cells,
    )
    .map_err(|error| stage("first-B18/B19", error))?;

    let mut latest_base_apply = (*base.state_after).clone();
    reconcile_known_stems(&mut latest_base_apply, &carried_sig, &carried_bindings)?;
    carried_sig
        .validate_integrity()
        .map_err(|error| stage("first-post-transaction-SIG", error))?;
    carried_bindings
        .validate_against(&carried_sig)
        .map_err(|error| stage("first-post-transaction-bindings", error))?;

    let carrier = NativeStemsBeamSidesCarrier {
        scheduler: (*outer_resume.resume.advanced_system).clone(),
        latest_base_apply,
        sig: carried_sig,
        bindings: carried_bindings,
        b_cells,
        s_cells,
    };
    let transaction = NativeStemsBeamSidesTransaction {
        preparation,
        create,
        reuse_live_state,
        reuse,
        base,
        flag,
        siblings,
        heads,
        outer_resume,
    };
    Ok((carrier, transaction))
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

/// Execute one typed STUMPS frontier from native canonical-glyph identity.
pub fn advance_native_stems_beam_stumps_transaction_from_modeled_registry(
    carrier: &mut NativeStemsBeamSidesCarrier,
    context: NativeStemsBeamSidesContext<'_>,
    registry: &NativeStemsModeledGlyphRegistry,
) -> Result<NativeStemsBeamStumpsTransaction, NativeStemsBeamSidesError> {
    advance_native_stems_beam_sides_transaction_with_authority(
        carrier,
        context,
        GlyphAuthority::Modeled(registry),
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
    drive_native_stems_beam_stumps_with_authority(
        carrier,
        context,
        GlyphAuthority::FirstStems(bridge),
        transaction_limit,
    )
}

/// Atomically drive STUMPS from owned native canonical-glyph identity.
pub fn drive_native_stems_beam_stumps_from_modeled_registry(
    carrier: &mut NativeStemsBeamSidesCarrier,
    context: NativeStemsBeamSidesContext<'_>,
    registry: &NativeStemsModeledGlyphRegistry,
    transaction_limit: usize,
) -> Result<NativeStemsBeamStumpsDrive, NativeStemsBeamSidesError> {
    drive_native_stems_beam_stumps_with_authority(
        carrier,
        context,
        GlyphAuthority::Modeled(registry),
        transaction_limit,
    )
}

fn drive_native_stems_beam_stumps_with_authority(
    carrier: &mut NativeStemsBeamSidesCarrier,
    context: NativeStemsBeamSidesContext<'_>,
    glyphs: GlyphAuthority<'_>,
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

        match advance_native_stems_beam_sides_transaction_with_authority(
            &mut shadow,
            context,
            glyphs,
            CarrierPass::Stumps,
        ) {
            Ok(transaction) => transactions.push(transaction.into_stumps()?),
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
            false,
        )?;
        let bottom_ok = bounded_head_can_link(
            bottom,
            0,
            head_builders,
            &carrier.s_cells,
            plans.min_linker_length,
            false,
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
        phase_two_index: 0,
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
    append: bool,
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
    let mut saw_gap = false;
    for item in builder.items.iter().skip(1) {
        if item.kind == NativeStemsHeadBuilderItemKind::Gap {
            // Java's getFirstCLinkerAfter stops at a too-wide gap and
            // canLink then reports true; a narrow gap is remembered for
            // the close-head branch below.
            if item.contribution > max_gap {
                return Ok(true);
            }
            saw_gap = true;
            continue;
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
        // Java: `if (!append && cl.isLinked()) return false;` - phase 2 runs
        // with append=true, where an already linked close head no longer
        // blocks the link.
        if !append && target_side.linked {
            return Ok(false);
        }
        if !saw_gap {
            // Java accepts the link when no stem gap separates the two
            // heads (HeadLinker.CLinker.canLink's gapIndex == null branch).
            return Ok(true);
        }
        return Err(stage(
            "HEADS-phase1-canLink",
            "first head reaches the unported close-head/gap recursion",
        ));
    }
    Ok(true)
}

/// Continue Java `HeadLinker.linkSides` phase 1 after one successful C-link.
///
/// The first head mutation is already owned by
/// [`advance_native_stems_head_single_item_c_link`].  This seam deliberately
/// stops at the next ordered head frontier, or completes one already-linked
/// head when both remaining open corners fail STRICT `canLink`.  It
/// refreshes the queued S-linker view from the carried persistent cells,
/// revalidates the stable reverse-grade order, and applies Java's graph-derived
/// closure of every other head sharing the linked stem.  It does not mutate
/// SIG, glyph, stem, allocator, or linked-cell state.
pub fn continue_native_stems_head_linking_phase1(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: Option<&NativeStemsHeadCornerReachabilitySystem>,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index == 0 {
        return Err(stage(
            "HEADS-phase1-continue",
            "continuation requires one consumed head frontier",
        ));
    }
    if carrier.current_index >= carrier.heads.len() {
        return Err(stage(
            "HEADS-phase1-continue",
            "head continuation index is outside the ordered queue",
        ));
    }
    if carrier.beam_state.scheduler.system_id != head_corners.system_id
        || carrier.beam_state.sig.system_id != head_corners.system_id
        || carrier.beam_state.bindings.system_id != head_corners.system_id
        || head_builders.system_id != head_corners.system_id
        || plans.system_id != head_corners.system_id
    {
        return Err(stage(
            "HEADS-phase1-continue-system",
            "continued carrier and head products differ by system",
        ));
    }
    let NativeStemsBeamSchedulerStatus::Completed { .. } = &carrier.beam_state.scheduler.status
    else {
        return Err(stage(
            "HEADS-phase1-continue-terminal",
            "continued head phase does not own a completed STUMPS carrier",
        ));
    };
    carrier
        .beam_state
        .bindings
        .validate_against(&carrier.beam_state.sig)
        .map_err(|error| stage("HEADS-phase1-continue-bindings", error))?;

    let mut expected_grade_order = (0..head_corners.heads_in_sig_order.len()).collect::<Vec<_>>();
    expected_grade_order.sort_by(|&left, &right| {
        java_double_compare(
            f64::from_bits(head_corners.heads_in_sig_order[right].grade_bits),
            f64::from_bits(head_corners.heads_in_sig_order[left].grade_bits),
        )
        .cmp(&0)
    });
    if expected_grade_order != head_corners.heads_by_reverse_grade
        || carrier.heads.len() != expected_grade_order.len()
    {
        return Err(stage(
            "HEADS-phase1-continue-order",
            "continued head queue is not Java's stable reverse-grade order",
        ));
    }
    let expected_cells = initialize_native_stems_beam_s_linker_cells(head_corners)
        .map_err(|error| stage("HEADS-phase1-continue-S-topology", error))?;
    if expected_cells.len() != carrier.beam_state.s_cells.len() {
        return Err(stage(
            "HEADS-phase1-continue-S-topology",
            "continued persistent S-cell catalogue is not exhaustive",
        ));
    }
    let mut live_refs = BTreeSet::new();
    for cell in &carrier.beam_state.s_cells {
        if !live_refs.insert(cell.reference) {
            return Err(stage(
                "HEADS-phase1-continue-S-topology",
                "duplicate persistent S-cell reference",
            ));
        }
        let expected = expected_cells
            .iter()
            .find(|expected| expected.reference == cell.reference)
            .ok_or_else(|| {
                stage(
                    "HEADS-phase1-continue-S-topology",
                    "persistent S cell is absent from constructor topology",
                )
            })?;
        if cell.ordered_observer_corners != expected.ordered_observer_corners {
            return Err(stage(
                "HEADS-phase1-continue-S-topology",
                "persistent S-cell observer order differs",
            ));
        }
    }

    let mut shadow = carrier.clone();
    for (queue_index, &sig_ordinal) in expected_grade_order.iter().enumerate() {
        let head = &head_corners.heads_in_sig_order[sig_ordinal];
        let x_ordinal = head_corners
            .heads_by_abscissa
            .iter()
            .position(|&candidate| candidate == sig_ordinal)
            .ok_or_else(|| stage("HEADS-phase1-continue-order", "head lacks x order"))?;
        let reference = NativeStemsBeamHeadLinkHeadRef {
            reference: head.reference,
            sig_ordinal,
            x_ordinal,
        };
        if shadow.heads[queue_index].reference != reference {
            return Err(stage(
                "HEADS-phase1-continue-order",
                "continued queue head identity differs",
            ));
        }
        let vertex_id = shadow
            .beam_state
            .bindings
            .head_vertices
            .get(&head.reference)
            .ok_or_else(|| stage("HEADS-phase1-continue-head", "head binding is missing"))?;
        let vertex = shadow.beam_state.sig.vertex(vertex_id.0).ok_or_else(|| {
            stage(
                "HEADS-phase1-continue-head",
                "head binding points outside the live SIG",
            )
        })?;
        if !vertex.active
            || vertex.removed
            || vertex.kind != NativeSigInterKind::Head
            || vertex.grade.to_bits() != head.grade_bits
        {
            return Err(stage(
                "HEADS-phase1-continue-head",
                "continued head binding is not the exact live graded HeadInter",
            ));
        }
        let sides = [
            crate::stems_step::NativeStemHeadSide::Left,
            crate::stems_step::NativeStemHeadSide::Right,
        ]
        .into_iter()
        .map(|horizontal| {
            let reference = NativeStemsBeamHeadSLinkerRef {
                head: shadow.heads[queue_index].reference,
                horizontal,
            };
            shadow
                .beam_state
                .s_cells
                .iter()
                .find(|cell| cell.reference == reference)
                .cloned()
                .ok_or_else(|| {
                    stage(
                        "HEADS-phase1-continue-S-topology",
                        "head side cell is missing",
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
        shadow.heads[queue_index].sides = sides;
        shadow.heads[queue_index].grade = vertex.grade;
    }

    let current = shadow.heads[shadow.current_index].clone();
    let undefined_sides_before = shadow.undefined_sides.len();
    let mut side_decisions = Vec::new();
    let mut next_corner = None;
    let mut linked_before = false;
    for side in &current.sides {
        if side.linked {
            linked_before = true;
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
            &shadow.beam_state.s_cells,
            plans.min_linker_length,
            false,
        )?;
        let bottom_ok = bounded_head_can_link(
            bottom,
            0,
            head_builders,
            &shadow.beam_state.s_cells,
            plans.min_linker_length,
            false,
        )?;
        side_decisions.push(NativeStemsHeadPhase1SideDecision {
            side: side.reference.horizontal,
            linked_before: false,
            closed_before: false,
            top_can_link: Some(top_ok),
            bottom_can_link: Some(bottom_ok),
        });
        match (top_ok, bottom_ok) {
            (true, false) => next_corner = Some(top),
            (false, true) => next_corner = Some(bottom),
            (true, true) => {
                // Java records an undefined side only when the two corner
                // stumps are one shared non-null glyph (HeadLinker.linkSides
                // guards the undef branch with `clTop.stump != null &&
                // clTop.stump == clBot.stump`); differing or missing stumps
                // take the standard-connection branch instead, which stays
                // unported and fails closed here.
                let reachability = head_reachability.ok_or_else(|| {
                    stage(
                        "HEADS-phase1-dual-corner",
                        "dual-corner undef requires reachability stump authentication",
                    )
                })?;
                if reachability.system_id != head_corners.system_id {
                    return Err(stage(
                        "HEADS-phase1-dual-corner",
                        "head reachability belongs to a different system",
                    ));
                }
                let stump_of = |corner: NativeStemsHeadCornerRef| {
                    reachability
                        .heads
                        .iter()
                        .flat_map(|head| &head.corners)
                        .find(|reach| reach.reference == corner)
                        .map(|reach| reach.stump)
                        .ok_or_else(|| {
                            stage(
                                "HEADS-phase1-dual-corner",
                                "dual-corner reachability corner is missing",
                            )
                        })
                };
                match (stump_of(top)?, stump_of(bottom)?) {
                    (Some(top_stump), Some(bottom_stump)) if top_stump == bottom_stump => {
                        shadow.undefined_sides.push(side.reference);
                    }
                    _ => {
                        return Err(stage(
                            "HEADS-phase1-dual-corner",
                            "dual-corner differing-stump standard connection is unported",
                        ));
                    }
                }
            }
            (false, false) => {}
        }
        if next_corner.is_some() {
            break;
        }
    }
    if shadow.undefined_sides.len() > undefined_sides_before {
        // Java's caller adds every head whose linkSides returns false to
        // the phase-2 unlinkedHeads queue (StemsRetriever heads linking
        // phase 1) before re-running linkSides with append=true.
        shadow.unlinked_heads.push(current.reference);
        shadow.current_index += 1;
        shadow.frontier_consumed = true;
        return Ok(NativeStemsHeadPhase1Continuation {
            processed_head: current.reference,
            side_decisions,
            returned_linked: Some(false),
            closed_s_linkers: Vec::new(),
            closed_value_changes: 0,
            state_after: Box::new(shadow),
        });
    }
    let Some(next_corner) = next_corner else {
        if !linked_before {
            return Err(stage(
                "HEADS-phase1-continue-retry",
                "unlinked head requires Java's rather-good retry/closure branch",
            ));
        }
        let (closed_s_linkers, closed_value_changes) = close_heads_sharing_prelinked_stems(
            &shadow.beam_state.sig,
            &shadow.beam_state.bindings,
            &mut shadow.beam_state.s_cells,
            &mut shadow.heads,
            &current,
        )?;
        shadow.current_index += 1;
        shadow.frontier_consumed = true;
        return Ok(NativeStemsHeadPhase1Continuation {
            processed_head: current.reference,
            side_decisions,
            returned_linked: Some(true),
            closed_s_linkers,
            closed_value_changes,
            state_after: Box::new(shadow),
        });
    };

    shadow.frontier = NativeStemsHeadPhase1Frontier {
        head: current.reference,
        stem_profile: 0,
        link_profile: plans.link_profile,
        append: false,
        side_decisions: side_decisions.clone(),
        next_corner,
    };
    shadow.frontier_consumed = false;
    Ok(NativeStemsHeadPhase1Continuation {
        processed_head: current.reference,
        side_decisions,
        returned_linked: None,
        closed_s_linkers: Vec::new(),
        closed_value_changes: 0,
        state_after: Box::new(shadow),
    })
}

fn close_heads_sharing_prelinked_stems(
    sig: &NativeSigSystem,
    bindings: &NativeSigSystemBindings,
    persistent_cells: &mut [NativeStemsBeamNativeSLinkerCell],
    heads: &mut [NativeStemsHeadPhase1Head],
    current: &NativeStemsHeadPhase1Head,
) -> Result<(Vec<NativeStemsBeamHeadSLinkerRef>, usize), NativeStemsBeamSidesError> {
    let current_vertex = *bindings
        .head_vertices
        .get(&current.reference.reference)
        .ok_or_else(|| stage("HEADS-phase1-close", "current head binding is missing"))?;
    let mut writes = Vec::new();
    let mut written = BTreeSet::new();
    let mut value_changes = 0;
    for side in current.sides.iter().filter(|side| side.linked) {
        let matching = sig
            .incident_edges(current_vertex.0)
            .map_err(|error| stage("HEADS-phase1-close-head-scan", error))?
            .into_iter()
            .filter(|edge| {
                edge.kind == NativeSigRelationKind::HeadStem
                    && edge
                        .head_stem
                        .as_ref()
                        .is_some_and(|payload| payload.head_side == side.reference.horizontal)
            })
            .map(|edge| edge.ordinal)
            .collect::<Vec<_>>();
        let [edge_ordinal] = matching.as_slice() else {
            return Err(stage(
                "HEADS-phase1-close-head-scan",
                "bounded linked head side needs exactly one matching HeadStem relation",
            ));
        };
        {
            let edge = &sig.edges[*edge_ordinal];
            let stem_vertex = if edge.source == current_vertex.0 {
                edge.target
            } else if edge.target == current_vertex.0 {
                edge.source
            } else {
                return Err(stage(
                    "HEADS-phase1-close-head-scan",
                    "incident HeadStem relation does not touch the current head",
                ));
            };
            let stem = sig.vertex(stem_vertex).ok_or_else(|| {
                stage(
                    "HEADS-phase1-close-stem-scan",
                    "matching HeadStem points outside the live SIG",
                )
            })?;
            if stem.kind != NativeSigInterKind::Stem {
                return Err(stage(
                    "HEADS-phase1-close-stem-scan",
                    "matching HeadStem opposite is not a StemInter",
                ));
            }
            let other_vertices = sig
                .incident_edges(stem_vertex)
                .map_err(|error| stage("HEADS-phase1-close-stem-scan", error))?
                .into_iter()
                .filter(|edge| edge.kind == NativeSigRelationKind::HeadStem)
                .map(|edge| {
                    if edge.source == stem_vertex {
                        edge.target
                    } else {
                        edge.source
                    }
                })
                .filter(|&vertex| vertex != current_vertex.0)
                .collect::<Vec<_>>();
            let mut seen_other_vertices = BTreeSet::new();
            for other_vertex in other_vertices {
                if !seen_other_vertices.insert(other_vertex) {
                    return Err(stage(
                        "HEADS-phase1-close-stem-scan",
                        "shared stem repeats one other HeadInter",
                    ));
                }
                let matching_heads = bindings
                    .head_vertices
                    .iter()
                    .filter(|(_, vertex)| vertex.0 == other_vertex)
                    .map(|(head, _)| *head)
                    .collect::<Vec<_>>();
                let [other_head] = matching_heads.as_slice() else {
                    return Err(stage(
                        "HEADS-phase1-close-head-binding",
                        "shared-stem head binding is missing or duplicated",
                    ));
                };
                let queued_index = heads
                    .iter()
                    .position(|head| head.reference.reference == *other_head)
                    .ok_or_else(|| {
                        stage(
                            "HEADS-phase1-close-head-binding",
                            "shared-stem head is absent from the ordered queue",
                        )
                    })?;
                for horizontal in [
                    crate::stems_step::NativeStemHeadSide::Left,
                    crate::stems_step::NativeStemHeadSide::Right,
                ] {
                    let reference = NativeStemsBeamHeadSLinkerRef {
                        head: heads[queued_index].reference,
                        horizontal,
                    };
                    if !written.insert(reference) {
                        return Err(stage(
                            "HEADS-phase1-close-S-cell",
                            "bounded closure would write one S cell more than once",
                        ));
                    }
                    let persistent = persistent_cells
                        .iter_mut()
                        .find(|cell| cell.reference == reference)
                        .ok_or_else(|| {
                            stage(
                                "HEADS-phase1-close-S-cell",
                                "shared-stem persistent S cell is missing",
                            )
                        })?;
                    let queued = heads[queued_index]
                        .sides
                        .iter_mut()
                        .find(|cell| cell.reference == reference)
                        .ok_or_else(|| {
                            stage(
                                "HEADS-phase1-close-S-cell",
                                "shared-stem queued S cell is missing",
                            )
                        })?;
                    if persistent.linked != queued.linked || persistent.closed != queued.closed {
                        return Err(stage(
                            "HEADS-phase1-close-S-cell",
                            "persistent and queued S cells differ before closure",
                        ));
                    }
                    if !persistent.closed {
                        value_changes += 1;
                    }
                    persistent.closed = true;
                    queued.closed = true;
                    writes.push(reference);
                }
            }
        }
    }
    Ok((writes, value_changes))
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
    bridge: &impl NativeStemsGlyphRegistryAuthority,
) -> Result<NativeStemsHeadCLinkTransaction, NativeStemsBeamSidesError> {
    let shadow = carrier.clone();
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
    advance_native_stems_head_c_link_at_frontier(
        carrier,
        head_corners,
        head_reachability,
        stem_seeds,
        head_builders,
        plans,
        checker,
        bridge,
        0,
        0,
        0,
        &reconstructed.frontier,
    )
}

/// Execute the first bounded continuation C-link after the prelinked head queue.
///
/// Boundary 33 authenticates Java's measured x76/SIG97/Inter1483 frontier:
/// LEFT/BOTTOM is the sole `BottomOnly` choice, and the one-item active glyph
/// 319 C-link creates Stem Inter 2380 before advancing to queue index 8.  The
/// initial frontier API above remains strict and cannot consume this state.
#[expect(
    clippy::too_many_arguments,
    reason = "the atomic boundary authenticates each independently owned native authority"
)]
pub fn advance_native_stems_head_continuation_c_link(
    carrier: &mut NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    stem_seeds: &NativeStemSeedSystemRecognition,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    checker: &NativeStemsBeamStemCheckerContext,
    bridge: &impl NativeStemsGlyphRegistryAuthority,
) -> Result<NativeStemsHeadCLinkTransaction, NativeStemsBeamSidesError> {
    advance_native_stems_head_continuation_c_link_at_queue(
        carrier,
        head_corners,
        head_reachability,
        stem_seeds,
        head_builders,
        plans,
        checker,
        bridge,
        7,
        76,
        97,
        0,
        0,
        "order7",
    )
}

/// Execute the bounded continuation C-link at order 18.
///
/// This is the first open/unlinked continuation after the prelinked queue:
/// x63/SIG17 selects LEFT/BOTTOM, reuses active glyph 328 (with its paired
/// glyph 2063), creates StemInter 2381, and advances the queue to order 19.
#[expect(
    clippy::too_many_arguments,
    reason = "the atomic boundary authenticates each independently owned native authority"
)]
pub fn advance_native_stems_head_continuation_c_link_order18(
    carrier: &mut NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    stem_seeds: &NativeStemSeedSystemRecognition,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    checker: &NativeStemsBeamStemCheckerContext,
    bridge: &impl NativeStemsGlyphRegistryAuthority,
) -> Result<NativeStemsHeadCLinkTransaction, NativeStemsBeamSidesError> {
    advance_native_stems_head_continuation_c_link_at_queue(
        carrier,
        head_corners,
        head_reachability,
        stem_seeds,
        head_builders,
        plans,
        checker,
        bridge,
        18,
        63,
        17,
        1,
        1,
        "order18",
    )
}

/// Execute the bounded continuation C-link at order 20.
///
/// x74/SIG19 selects LEFT/BOTTOM, reuses active glyph 332 (with paired
/// glyph 2301), creates StemInter 2382, and advances the queue to order 21.
#[expect(
    clippy::too_many_arguments,
    reason = "the atomic boundary authenticates each independently owned native authority"
)]
pub fn advance_native_stems_head_continuation_c_link_order20(
    carrier: &mut NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    stem_seeds: &NativeStemSeedSystemRecognition,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    checker: &NativeStemsBeamStemCheckerContext,
    bridge: &impl NativeStemsGlyphRegistryAuthority,
) -> Result<NativeStemsHeadCLinkTransaction, NativeStemsBeamSidesError> {
    advance_native_stems_head_continuation_c_link_at_queue(
        carrier,
        head_corners,
        head_reachability,
        stem_seeds,
        head_builders,
        plans,
        checker,
        bridge,
        20,
        74,
        19,
        1,
        1,
        "order20",
    )
}

/// Execute the bounded continuation C-link at order 27.
///
/// x33/SIG26 selects LEFT/BOTTOM, reuses active glyph 314 (with paired
/// glyph 2219), creates StemInter 2383, and advances the queue to order 28.
#[expect(
    clippy::too_many_arguments,
    reason = "the atomic boundary authenticates each independently owned native authority"
)]
pub fn advance_native_stems_head_continuation_c_link_order27(
    carrier: &mut NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    stem_seeds: &NativeStemSeedSystemRecognition,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    checker: &NativeStemsBeamStemCheckerContext,
    bridge: &impl NativeStemsGlyphRegistryAuthority,
) -> Result<NativeStemsHeadCLinkTransaction, NativeStemsBeamSidesError> {
    advance_native_stems_head_continuation_c_link_at_queue(
        carrier,
        head_corners,
        head_reachability,
        stem_seeds,
        head_builders,
        plans,
        checker,
        bridge,
        27,
        33,
        26,
        1,
        1,
        "order27",
    )
}

/// Execute the bounded continuation C-link at order 34.
///
/// x2/SIG36 selects LEFT/BOTTOM, reuses active glyph 322 (with paired
/// glyph 1946), creates StemInter 2384, and advances the queue to order 35.
#[expect(
    clippy::too_many_arguments,
    reason = "the atomic boundary authenticates each independently owned native authority"
)]
pub fn advance_native_stems_head_continuation_c_link_order34(
    carrier: &mut NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    stem_seeds: &NativeStemSeedSystemRecognition,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    checker: &NativeStemsBeamStemCheckerContext,
    bridge: &impl NativeStemsGlyphRegistryAuthority,
) -> Result<NativeStemsHeadCLinkTransaction, NativeStemsBeamSidesError> {
    advance_native_stems_head_continuation_c_link_at_queue(
        carrier,
        head_corners,
        head_reachability,
        stem_seeds,
        head_builders,
        plans,
        checker,
        bridge,
        34,
        2,
        36,
        1,
        1,
        "order34",
    )
}

/// Execute the bounded continuation C-link at order 36.
///
/// x23/SIG14 selects LEFT/BOTTOM, reuses active glyph 324, creates
/// StemInter 2385, and advances the queue to order 37.
#[expect(
    clippy::too_many_arguments,
    reason = "the atomic boundary authenticates each independently owned native authority"
)]
pub fn advance_native_stems_head_continuation_c_link_order36(
    carrier: &mut NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    stem_seeds: &NativeStemSeedSystemRecognition,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    checker: &NativeStemsBeamStemCheckerContext,
    bridge: &impl NativeStemsGlyphRegistryAuthority,
) -> Result<NativeStemsHeadCLinkTransaction, NativeStemsBeamSidesError> {
    advance_native_stems_head_continuation_c_link_at_queue(
        carrier,
        head_corners,
        head_reachability,
        stem_seeds,
        head_builders,
        plans,
        checker,
        bridge,
        36,
        23,
        14,
        0,
        0,
        "order36",
    )
}

/// Reconcile the bounded existing-stem retry at order 37.
///
/// x14/SIG1 selects LEFT/BOTTOM against existing StemInter 2340. Java does
/// not allocate or mutate SIG here: it closes x13's two S cells and advances
/// to order 38. The generic continuation performs the graph-derived closure;
/// this wrapper authenticates the retry frontier and fails closed on mismatch.
pub fn advance_native_stems_head_existing_stem_retry_order37(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed
        || carrier.current_index != 37
        || !carrier.unlinked_heads.is_empty()
        || !carrier.undefined_sides.is_empty()
    {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order37 continuation",
        ));
    }
    let head = carrier.heads.get(37).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order37 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 14 || head.reference.sig_ordinal != 1 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x14/SIG1",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked || left.closed {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order37 LEFT cell is not the linked open side",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 0)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order37 existing StemInter 2340/glyph294 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order37 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 14
        || continuation.processed_head.sig_ordinal != 1
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 38
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order37 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 38.
///
/// x18/SIG4 selects LEFT/BOTTOM against existing StemInter 2372. Java does
/// not allocate or mutate SIG here: it closes x17's two S cells and advances
/// to order 39. The generic continuation performs the graph-derived closure;
/// this wrapper authenticates the retry frontier and fails closed on mismatch.
pub fn advance_native_stems_head_existing_stem_retry_order38(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed
        || carrier.current_index != 38
        || !carrier.unlinked_heads.is_empty()
        || !carrier.undefined_sides.is_empty()
    {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order38 continuation",
        ));
    }
    let head = carrier.heads.get(38).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order38 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 18 || head.reference.sig_ordinal != 4 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x18/SIG4",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked || left.closed {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order38 LEFT cell is not the linked open side",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 32)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order38 existing StemInter 2372/glyph310 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order38 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 18
        || continuation.processed_head.sig_ordinal != 4
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 39
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order38 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 39.
///
/// x97/SIG34 selects LEFT/BOTTOM against existing StemInter 2373. Java does
/// not allocate or mutate SIG here: it closes x96's two S cells and advances
/// to order 40. The generic continuation performs the graph-derived closure;
/// this wrapper authenticates the retry frontier and fails closed on mismatch.
pub fn advance_native_stems_head_existing_stem_retry_order39(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed
        || carrier.current_index != 39
        || !carrier.unlinked_heads.is_empty()
        || !carrier.undefined_sides.is_empty()
    {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order39 continuation",
        ));
    }
    let head = carrier.heads.get(39).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order39 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 97 || head.reference.sig_ordinal != 34 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x97/SIG34",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked || left.closed {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order39 LEFT cell is not the linked open side",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 33)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order39 existing StemInter 2373/glyph321 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order39 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 97
        || continuation.processed_head.sig_ordinal != 34
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 40
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order39 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 40.
///
/// x6/SIG89 selects LEFT/BOTTOM against existing StemInter 2348. Java does
/// not allocate or mutate SIG here: it closes x5's two S cells and advances
/// to order 41. The generic continuation performs the graph-derived closure;
/// this wrapper authenticates the retry frontier and fails closed on mismatch.
pub fn advance_native_stems_head_existing_stem_retry_order40(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed
        || carrier.current_index != 40
        || !carrier.unlinked_heads.is_empty()
        || !carrier.undefined_sides.is_empty()
    {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order40 continuation",
        ));
    }
    let head = carrier.heads.get(40).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order40 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 6 || head.reference.sig_ordinal != 89 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x6/SIG89",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked || left.closed {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order40 LEFT cell is not the linked open side",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 8)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order40 existing StemInter 2348/glyph290 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order40 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 6
        || continuation.processed_head.sig_ordinal != 89
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 41
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order40 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 41.
///
/// x30/SIG67 selects LEFT/BOTTOM against existing StemInter 2357. Java does
/// not allocate or mutate SIG here: it closes x29's two S cells and advances
/// to order 42. The generic continuation performs the graph-derived closure;
/// this wrapper authenticates the retry frontier and fails closed on mismatch.
pub fn advance_native_stems_head_existing_stem_retry_order41(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed
        || carrier.current_index != 41
        || !carrier.unlinked_heads.is_empty()
        || !carrier.undefined_sides.is_empty()
    {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order41 continuation",
        ));
    }
    let head = carrier.heads.get(41).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order41 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 30 || head.reference.sig_ordinal != 67 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x30/SIG67",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked || left.closed {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order41 LEFT cell is not the linked open side",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 17)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order41 existing StemInter 2357/glyph313 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order41 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 30
        || continuation.processed_head.sig_ordinal != 67
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 42
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order41 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 42.
///
/// x43/SIG48 selects LEFT/BOTTOM against existing StemInter 2350. Java does
/// not allocate or mutate SIG here: it closes the two preceding heads x39 and
/// x40 (both sides, in order) and advances to order 43. The generic
/// continuation performs the graph-derived closure; this wrapper authenticates
/// the retry frontier and fails closed on mismatch.
pub fn advance_native_stems_head_existing_stem_retry_order42(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed
        || carrier.current_index != 42
        || !carrier.unlinked_heads.is_empty()
        || !carrier.undefined_sides.is_empty()
    {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order42 continuation",
        ));
    }
    let head = carrier.heads.get(42).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order42 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 43 || head.reference.sig_ordinal != 48 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x43/SIG48",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked || left.closed {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order42 LEFT cell is not the linked open side",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 10)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order42 existing StemInter 2350/glyph326 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order42 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 43
        || continuation.processed_head.sig_ordinal != 48
        || continuation.closed_value_changes != 4
        || continuation.state_after.current_index != 43
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order42 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 43.
///
/// x25/SIG91 selects LEFT/BOTTOM against existing StemInter 2356. Java does
/// not allocate or mutate SIG here: it closes x24's two S cells and advances
/// to order 44. The generic continuation performs the graph-derived closure;
/// this wrapper authenticates the retry frontier and fails closed on mismatch.
pub fn advance_native_stems_head_existing_stem_retry_order43(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed
        || carrier.current_index != 43
        || !carrier.unlinked_heads.is_empty()
        || !carrier.undefined_sides.is_empty()
    {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order43 continuation",
        ));
    }
    let head = carrier.heads.get(43).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order43 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 25 || head.reference.sig_ordinal != 91 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x25/SIG91",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked || left.closed {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order43 LEFT cell is not the linked open side",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 16)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order43 existing StemInter 2356/glyph292 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order43 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 25
        || continuation.processed_head.sig_ordinal != 91
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 44
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order43 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 44.
///
/// x83/SIG21 selects LEFT/BOTTOM against existing StemInter 2358. Java does
/// not allocate or mutate SIG here: it closes x82's two S cells and advances
/// to order 45. The generic continuation performs the graph-derived closure;
/// this wrapper authenticates the retry frontier and fails closed on mismatch.
pub fn advance_native_stems_head_existing_stem_retry_order44(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed
        || carrier.current_index != 44
        || !carrier.unlinked_heads.is_empty()
        || !carrier.undefined_sides.is_empty()
    {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order44 continuation",
        ));
    }
    let head = carrier.heads.get(44).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order44 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 83 || head.reference.sig_ordinal != 21 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x83/SIG21",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked || left.closed {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order44 LEFT cell is not the linked open side",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 18)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order44 existing StemInter 2358/glyph301 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order44 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 83
        || continuation.processed_head.sig_ordinal != 21
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 45
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order44 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 45.
///
/// x57/SIG5 selects LEFT/BOTTOM against existing StemInter 2374. Java does
/// not allocate or mutate SIG here: it closes x56's two S cells and advances
/// to order 46. The generic continuation performs the graph-derived closure;
/// this wrapper authenticates the retry frontier and fails closed on mismatch.
pub fn advance_native_stems_head_existing_stem_retry_order45(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed
        || carrier.current_index != 45
        || !carrier.unlinked_heads.is_empty()
        || !carrier.undefined_sides.is_empty()
    {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order45 continuation",
        ));
    }
    let head = carrier.heads.get(45).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order45 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 57 || head.reference.sig_ordinal != 5 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x57/SIG5",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked || left.closed {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order45 LEFT cell is not the linked open side",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 34)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order45 existing StemInter 2374/glyph303 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order45 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 57
        || continuation.processed_head.sig_ordinal != 5
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 46
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order45 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 46.
///
/// x40/SIG27 selects LEFT/BOTTOM against existing StemInter 2350. Java does
/// not allocate or mutate SIG here: it closes x43's two S cells and advances
/// to order 47. The generic continuation performs the graph-derived closure;
/// this wrapper authenticates the retry frontier and fails closed on mismatch.
pub fn advance_native_stems_head_existing_stem_retry_order46(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed
        || carrier.current_index != 46
        || !carrier.unlinked_heads.is_empty()
        || !carrier.undefined_sides.is_empty()
    {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order46 continuation",
        ));
    }
    let head = carrier.heads.get(46).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order46 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 40 || head.reference.sig_ordinal != 27 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x40/SIG27",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order46 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 10)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order46 existing StemInter 2350/glyph326 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order46 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 40
        || continuation.processed_head.sig_ordinal != 27
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 47
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order46 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 47.
///
/// x89/SIG22 selects LEFT/BOTTOM against existing StemInter 2359. Java does
/// not allocate or mutate SIG here: it closes x90's two S cells and advances
/// to order 48. The generic continuation performs the graph-derived closure;
/// this wrapper authenticates the retry frontier and fails closed on mismatch.
pub fn advance_native_stems_head_existing_stem_retry_order47(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed
        || carrier.current_index != 47
        || !carrier.unlinked_heads.is_empty()
        || !carrier.undefined_sides.is_empty()
    {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order47 continuation",
        ));
    }
    let head = carrier.heads.get(47).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order47 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 89 || head.reference.sig_ordinal != 22 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x89/SIG22",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order47 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 19)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order47 existing StemInter 2359/glyph304 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order47 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 89
        || continuation.processed_head.sig_ordinal != 22
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 48
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order47 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 48.
///
/// x52/SIG2 selects LEFT/BOTTOM against existing StemInter 2344. Java does
/// not allocate or mutate SIG here: it closes x53's two S cells and advances
/// to order 49. The generic continuation performs the graph-derived closure;
/// this wrapper authenticates the retry frontier and fails closed on mismatch.
pub fn advance_native_stems_head_existing_stem_retry_order48(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed
        || carrier.current_index != 48
        || !carrier.unlinked_heads.is_empty()
        || !carrier.undefined_sides.is_empty()
    {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order48 continuation",
        ));
    }
    let head = carrier.heads.get(48).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order48 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 52 || head.reference.sig_ordinal != 2 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x52/SIG2",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order48 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 4)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order48 existing StemInter 2344/glyph296 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order48 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 52
        || continuation.processed_head.sig_ordinal != 2
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 49
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order48 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 49.
///
/// x35/SIG68 selects LEFT/BOTTOM against existing StemInter 2369. Java does
/// not allocate or mutate SIG here: it closes x36's two S cells and advances
/// to order 50. The generic continuation performs the graph-derived closure;
/// this wrapper authenticates the retry frontier and fails closed on mismatch.
pub fn advance_native_stems_head_existing_stem_retry_order49(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed
        || carrier.current_index != 49
        || !carrier.unlinked_heads.is_empty()
        || !carrier.undefined_sides.is_empty()
    {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order49 continuation",
        ));
    }
    let head = carrier.heads.get(49).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order49 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 35 || head.reference.sig_ordinal != 68 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x35/SIG68",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order49 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 29)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order49 existing StemInter 2369/glyph316 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order49 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 35
        || continuation.processed_head.sig_ordinal != 68
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 50
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order49 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Continue the bounded open/undefined frontier at order 50.
///
/// x32/SIG50 reaches an already materialized StemInter 2383/glyph314.  The
/// C-link envelope is therefore a no-op: Java reports LEFT Both and RIGHT
/// TopOnly, records LEFT as undefined, returns false, and advances to order
/// 51 without changing SIG, linker, or allocator state.  Authenticate the
/// exact existing stem and side decisions before exposing that continuation.
pub fn advance_native_stems_head_open_frontier_order50(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed
        || carrier.current_index != 50
        || !carrier.unlinked_heads.is_empty()
        || !carrier.undefined_sides.is_empty()
    {
        return Err(stage(
            "HEADS-open-frontier",
            "carrier is not the authenticated order50 continuation",
        ));
    }
    let head = carrier
        .heads
        .get(50)
        .ok_or_else(|| stage("HEADS-open-frontier", "order50 head is missing"))?;
    if head.reference.x_ordinal != 32 || head.reference.sig_ordinal != 50 {
        return Err(stage(
            "HEADS-open-frontier",
            "carrier head is not x32/SIG50",
        ));
    }
    for horizontal in [
        crate::stems_step::NativeStemHeadSide::Left,
        crate::stems_step::NativeStemHeadSide::Right,
    ] {
        let cell = head
            .sides
            .iter()
            .find(|cell| cell.reference.horizontal == horizontal)
            .ok_or_else(|| stage("HEADS-open-frontier", "order50 side cell is missing"))?;
        if cell.linked || cell.closed {
            return Err(stage(
                "HEADS-open-frontier",
                "order50 side is not the authenticated open cell",
            ));
        }
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 43)
        .ok_or_else(|| {
            stage(
                "HEADS-open-frontier",
                "order50 existing StemInter 2383/glyph314 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-open-frontier",
            "order50 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        Some(head_reachability),
        head_builders,
        plans,
    )?;
    let expected_left = NativeStemsBeamHeadSLinkerRef {
        head: head.reference,
        horizontal: crate::stems_step::NativeStemHeadSide::Left,
    };
    let expected_decisions = [
        (crate::stems_step::NativeStemHeadSide::Left, true, true),
        (crate::stems_step::NativeStemHeadSide::Right, true, false),
    ];
    if continuation.returned_linked != Some(false)
        || continuation.processed_head.x_ordinal != 32
        || continuation.processed_head.sig_ordinal != 50
        || continuation.closed_value_changes != 0
        || !continuation.closed_s_linkers.is_empty()
        || continuation.state_after.current_index != 51
        || continuation.state_after.undefined_sides != vec![expected_left]
        || continuation.state_after.unlinked_heads != vec![head.reference]
        || continuation.side_decisions.len() != expected_decisions.len()
        || !continuation
            .side_decisions
            .iter()
            .zip(expected_decisions)
            .all(|(decision, (side, top, bottom))| {
                decision.side == side
                    && !decision.linked_before
                    && !decision.closed_before
                    && decision.top_can_link == Some(top)
                    && decision.bottom_can_link == Some(bottom)
            })
    {
        return Err(stage(
            "HEADS-open-frontier-result",
            "order50 open frontier did not produce the authenticated undefined continuation",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 51.
///
/// x19/SIG64 retries LEFT against the already linked existing StemInter
/// 2361/glyph299: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x20's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT side carried
/// from order 50 stays recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order51(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 51 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order51 continuation",
        ));
    }
    let order50_head = carrier.heads.get(50).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order51 carrier lacks the order50 head",
        )
    })?;
    let carried_undefined = NativeStemsBeamHeadSLinkerRef {
        head: order50_head.reference,
        horizontal: crate::stems_step::NativeStemHeadSide::Left,
    };
    if carrier.undefined_sides != vec![carried_undefined] {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order51 carrier lacks the carried order50 undefined LEFT side",
        ));
    }
    if carrier.unlinked_heads != vec![order50_head.reference] {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order51 carrier lacks the carried phase-2 unlinked queue",
        ));
    }
    let head = carrier.heads.get(51).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order51 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 19 || head.reference.sig_ordinal != 64 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x19/SIG64",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order51 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 21)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order51 existing StemInter 2361/glyph299 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order51 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 19
        || continuation.processed_head.sig_ordinal != 64
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 52
        || continuation.state_after.undefined_sides != vec![carried_undefined]
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order51 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 52.
///
/// x15/SIG80 retries LEFT against the already linked existing StemInter
/// 2360/glyph329: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x16's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT side carried
/// from order 50 stays recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order52(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 52 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order52 continuation",
        ));
    }
    let order50_head = carrier.heads.get(50).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order52 carrier lacks the order50 head",
        )
    })?;
    let carried_undefined = NativeStemsBeamHeadSLinkerRef {
        head: order50_head.reference,
        horizontal: crate::stems_step::NativeStemHeadSide::Left,
    };
    if carrier.undefined_sides != vec![carried_undefined] {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order52 carrier lacks the carried order50 undefined LEFT side",
        ));
    }
    if carrier.unlinked_heads != vec![order50_head.reference] {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order52 carrier lacks the carried phase-2 unlinked queue",
        ));
    }
    let head = carrier.heads.get(52).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order52 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 15 || head.reference.sig_ordinal != 80 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x15/SIG80",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order52 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 20)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order52 existing StemInter 2360/glyph329 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order52 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 15
        || continuation.processed_head.sig_ordinal != 80
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 53
        || continuation.state_after.undefined_sides != vec![carried_undefined]
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order52 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 53.
///
/// x84/SIG86 retries LEFT against the already linked existing StemInter
/// 2366/glyph320.  That stem is shared by three heads: Java skips LEFT as
/// already linked, skips the closed RIGHT, returns true, closes sibling
/// x85's open cells, and re-writes x86's already-closed cells without a
/// value change, all without touching SIG, allocator, or system-stem
/// state.  The undefined LEFT side carried from order 50 stays recorded
/// and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order53(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 53 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order53 continuation",
        ));
    }
    let order50_head = carrier.heads.get(50).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order53 carrier lacks the order50 head",
        )
    })?;
    let carried_undefined = NativeStemsBeamHeadSLinkerRef {
        head: order50_head.reference,
        horizontal: crate::stems_step::NativeStemHeadSide::Left,
    };
    if carrier.undefined_sides != vec![carried_undefined] {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order53 carrier lacks the carried order50 undefined LEFT side",
        ));
    }
    if carrier.unlinked_heads != vec![order50_head.reference] {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order53 carrier lacks the carried phase-2 unlinked queue",
        ));
    }
    let head = carrier.heads.get(53).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order53 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 84 || head.reference.sig_ordinal != 86 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x84/SIG86",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order53 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 26)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order53 existing StemInter 2366/glyph320 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order53 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 84
        || continuation.processed_head.sig_ordinal != 86
        || continuation.closed_value_changes != 2
        || continuation.closed_s_linkers.len() != 4
        || continuation.state_after.current_index != 54
        || continuation.state_after.undefined_sides != vec![carried_undefined]
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order53 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 54.
///
/// x11/SIG62 retries LEFT against the already linked existing StemInter
/// 2349/glyph312: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x12's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT side carried
/// from order 50 stays recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order54(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 54 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order54 continuation",
        ));
    }
    let order50_head = carrier.heads.get(50).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order54 carrier lacks the order50 head",
        )
    })?;
    let carried_undefined = NativeStemsBeamHeadSLinkerRef {
        head: order50_head.reference,
        horizontal: crate::stems_step::NativeStemHeadSide::Left,
    };
    if carrier.undefined_sides != vec![carried_undefined] {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order54 carrier lacks the carried order50 undefined LEFT side",
        ));
    }
    if carrier.unlinked_heads != vec![order50_head.reference] {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order54 carrier lacks the carried phase-2 unlinked queue",
        ));
    }
    let head = carrier.heads.get(54).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order54 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 11 || head.reference.sig_ordinal != 62 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x11/SIG62",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order54 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 9)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order54 existing StemInter 2349/glyph312 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order54 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 11
        || continuation.processed_head.sig_ordinal != 62
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 55
        || continuation.state_after.undefined_sides != vec![carried_undefined]
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order54 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 55.
///
/// x68/SIG75 retries LEFT against the already linked existing StemInter
/// 2347/glyph331: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x69's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT side carried
/// from order 50 stays recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order55(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 55 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order55 continuation",
        ));
    }
    let order50_head = carrier.heads.get(50).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order55 carrier lacks the order50 head",
        )
    })?;
    let carried_undefined = NativeStemsBeamHeadSLinkerRef {
        head: order50_head.reference,
        horizontal: crate::stems_step::NativeStemHeadSide::Left,
    };
    if carrier.undefined_sides != vec![carried_undefined] {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order55 carrier lacks the carried order50 undefined LEFT side",
        ));
    }
    if carrier.unlinked_heads != vec![order50_head.reference] {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order55 carrier lacks the carried phase-2 unlinked queue",
        ));
    }
    let head = carrier.heads.get(55).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order55 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 68 || head.reference.sig_ordinal != 75 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x68/SIG75",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order55 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 7)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order55 existing StemInter 2347/glyph331 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order55 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 68
        || continuation.processed_head.sig_ordinal != 75
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 56
        || continuation.state_after.undefined_sides != vec![carried_undefined]
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order55 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 56.
///
/// x21/SIG11 retries LEFT against the already linked existing StemInter
/// 2341/glyph323: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x22's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT side carried
/// from order 50 stays recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order56(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 56 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order56 continuation",
        ));
    }
    let order50_head = carrier.heads.get(50).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order56 carrier lacks the order50 head",
        )
    })?;
    let carried_undefined = NativeStemsBeamHeadSLinkerRef {
        head: order50_head.reference,
        horizontal: crate::stems_step::NativeStemHeadSide::Left,
    };
    if carrier.undefined_sides != vec![carried_undefined] {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order56 carrier lacks the carried order50 undefined LEFT side",
        ));
    }
    if carrier.unlinked_heads != vec![order50_head.reference] {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order56 carrier lacks the carried phase-2 unlinked queue",
        ));
    }
    let head = carrier.heads.get(56).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order56 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 21 || head.reference.sig_ordinal != 11 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x21/SIG11",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order56 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 1)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order56 existing StemInter 2341/glyph323 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order56 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 21
        || continuation.processed_head.sig_ordinal != 11
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 57
        || continuation.state_after.undefined_sides != vec![carried_undefined]
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order56 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 58.
///
/// x92/SIG24 retries LEFT against the already linked existing StemInter
/// 2342/glyph298: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x93's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT side carried
/// from order 50 stays recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order58(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 58 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order58 continuation",
        ));
    }
    let order50_head = carrier.heads.get(50).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order58 carrier lacks the order50 head",
        )
    })?;
    let carried_undefined = NativeStemsBeamHeadSLinkerRef {
        head: order50_head.reference,
        horizontal: crate::stems_step::NativeStemHeadSide::Left,
    };
    if carrier.undefined_sides != vec![carried_undefined] {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order58 carrier lacks the carried order50 undefined LEFT side",
        ));
    }
    if carrier.unlinked_heads != vec![order50_head.reference] {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order58 carrier lacks the carried phase-2 unlinked queue",
        ));
    }
    let head = carrier.heads.get(58).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order58 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 92 || head.reference.sig_ordinal != 24 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x92/SIG24",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order58 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 2)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order58 existing StemInter 2342/glyph298 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order58 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 92
        || continuation.processed_head.sig_ordinal != 24
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 59
        || continuation.state_after.undefined_sides != vec![carried_undefined]
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order58 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 59.
///
/// x100/SIG42 retries LEFT against the already linked existing StemInter
/// 2343/glyph333: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x101's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT side carried
/// from order 50 stays recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order59(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 59 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order59 continuation",
        ));
    }
    let order50_head = carrier.heads.get(50).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order59 carrier lacks the order50 head",
        )
    })?;
    let carried_undefined = NativeStemsBeamHeadSLinkerRef {
        head: order50_head.reference,
        horizontal: crate::stems_step::NativeStemHeadSide::Left,
    };
    if carrier.undefined_sides != vec![carried_undefined] {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order59 carrier lacks the carried order50 undefined LEFT side",
        ));
    }
    if carrier.unlinked_heads != vec![order50_head.reference] {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order59 carrier lacks the carried phase-2 unlinked queue",
        ));
    }
    let head = carrier.heads.get(59).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order59 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 100 || head.reference.sig_ordinal != 42 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x100/SIG42",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order59 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 3)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order59 existing StemInter 2343/glyph333 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order59 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 100
        || continuation.processed_head.sig_ordinal != 42
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 60
        || continuation.state_after.undefined_sides != vec![carried_undefined]
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order59 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Continue the bounded open/undefined frontier at order 60.
///
/// x71/SIG49 reaches an already materialized StemInter 2382/glyph332.  The
/// C-link envelope is therefore a no-op: Java reports LEFT Both and RIGHT
/// TopOnly, records a second undefined LEFT side after order 50's, returns
/// false, and advances to order 61 without changing SIG, linker, or
/// allocator state.  Authenticate the exact existing stem and side
/// decisions before exposing that continuation.
pub fn advance_native_stems_head_open_frontier_order60(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 60 {
        return Err(stage(
            "HEADS-open-frontier",
            "carrier is not the authenticated order60 continuation",
        ));
    }
    let order50_head = carrier.heads.get(50).ok_or_else(|| {
        stage(
            "HEADS-open-frontier",
            "order60 carrier lacks the order50 head",
        )
    })?;
    let carried_undefined = NativeStemsBeamHeadSLinkerRef {
        head: order50_head.reference,
        horizontal: crate::stems_step::NativeStemHeadSide::Left,
    };
    if carrier.undefined_sides != vec![carried_undefined] {
        return Err(stage(
            "HEADS-open-frontier",
            "order60 carrier lacks the carried order50 undefined LEFT side",
        ));
    }
    if carrier.unlinked_heads != vec![order50_head.reference] {
        return Err(stage(
            "HEADS-open-frontier",
            "order60 carrier lacks the carried phase-2 unlinked queue",
        ));
    }
    let head = carrier
        .heads
        .get(60)
        .ok_or_else(|| stage("HEADS-open-frontier", "order60 head is missing"))?;
    if head.reference.x_ordinal != 71 || head.reference.sig_ordinal != 49 {
        return Err(stage(
            "HEADS-open-frontier",
            "carrier head is not x71/SIG49",
        ));
    }
    for horizontal in [
        crate::stems_step::NativeStemHeadSide::Left,
        crate::stems_step::NativeStemHeadSide::Right,
    ] {
        let cell = head
            .sides
            .iter()
            .find(|cell| cell.reference.horizontal == horizontal)
            .ok_or_else(|| stage("HEADS-open-frontier", "order60 side cell is missing"))?;
        if cell.linked || cell.closed {
            return Err(stage(
                "HEADS-open-frontier",
                "order60 side is not the authenticated open cell",
            ));
        }
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 42)
        .ok_or_else(|| {
            stage(
                "HEADS-open-frontier",
                "order60 existing StemInter 2382/glyph332 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-open-frontier",
            "order60 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        Some(head_reachability),
        head_builders,
        plans,
    )?;
    let expected_left = NativeStemsBeamHeadSLinkerRef {
        head: head.reference,
        horizontal: crate::stems_step::NativeStemHeadSide::Left,
    };
    let expected_decisions = [
        (crate::stems_step::NativeStemHeadSide::Left, true, true),
        (crate::stems_step::NativeStemHeadSide::Right, true, false),
    ];
    if continuation.returned_linked != Some(false)
        || continuation.processed_head.x_ordinal != 71
        || continuation.processed_head.sig_ordinal != 49
        || continuation.closed_value_changes != 0
        || !continuation.closed_s_linkers.is_empty()
        || continuation.state_after.current_index != 61
        || continuation.state_after.undefined_sides != vec![carried_undefined, expected_left]
        || continuation.state_after.unlinked_heads != vec![order50_head.reference, head.reference]
        || continuation.side_decisions.len() != expected_decisions.len()
        || !continuation
            .side_decisions
            .iter()
            .zip(expected_decisions)
            .all(|(decision, (side, top, bottom))| {
                decision.side == side
                    && !decision.linked_before
                    && !decision.closed_before
                    && decision.top_can_link == Some(top)
                    && decision.bottom_can_link == Some(bottom)
            })
    {
        return Err(stage(
            "HEADS-open-frontier-result",
            "order60 open frontier did not produce the authenticated undefined continuation",
        ));
    }
    Ok(continuation)
}

/// Continue the bounded open/undefined frontier at order 61.
///
/// x70/SIG46 reaches the same already materialized StemInter 2382/glyph332
/// as order 60.  The C-link envelope is again a no-op: Java reports LEFT
/// Both and RIGHT TopOnly, records a third undefined LEFT side, returns
/// false, and advances to order 62 without changing SIG, linker, or
/// allocator state.  Authenticate the exact existing stem and side
/// decisions before exposing that continuation.
pub fn advance_native_stems_head_open_frontier_order61(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 61 {
        return Err(stage(
            "HEADS-open-frontier",
            "carrier is not the authenticated order61 continuation",
        ));
    }
    let carried_undefined = [50usize, 60usize]
        .iter()
        .map(|&queue_index| {
            carrier
                .heads
                .get(queue_index)
                .map(|head| NativeStemsBeamHeadSLinkerRef {
                    head: head.reference,
                    horizontal: crate::stems_step::NativeStemHeadSide::Left,
                })
                .ok_or_else(|| {
                    stage(
                        "HEADS-open-frontier",
                        "order61 carrier lacks an undef predecessor head",
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if carrier.undefined_sides != carried_undefined {
        return Err(stage(
            "HEADS-open-frontier",
            "order61 carrier lacks the carried order50/order60 undefined LEFT sides",
        ));
    }
    if carrier.unlinked_heads
        != carried_undefined
            .iter()
            .map(|cell| cell.head)
            .collect::<Vec<_>>()
    {
        return Err(stage(
            "HEADS-open-frontier",
            "order61 carrier lacks the carried phase-2 unlinked queue",
        ));
    }
    let head = carrier
        .heads
        .get(61)
        .ok_or_else(|| stage("HEADS-open-frontier", "order61 head is missing"))?;
    if head.reference.x_ordinal != 70 || head.reference.sig_ordinal != 46 {
        return Err(stage(
            "HEADS-open-frontier",
            "carrier head is not x70/SIG46",
        ));
    }
    for horizontal in [
        crate::stems_step::NativeStemHeadSide::Left,
        crate::stems_step::NativeStemHeadSide::Right,
    ] {
        let cell = head
            .sides
            .iter()
            .find(|cell| cell.reference.horizontal == horizontal)
            .ok_or_else(|| stage("HEADS-open-frontier", "order61 side cell is missing"))?;
        if cell.linked || cell.closed {
            return Err(stage(
                "HEADS-open-frontier",
                "order61 side is not the authenticated open cell",
            ));
        }
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 42)
        .ok_or_else(|| {
            stage(
                "HEADS-open-frontier",
                "order61 existing StemInter 2382/glyph332 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-open-frontier",
            "order61 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        Some(head_reachability),
        head_builders,
        plans,
    )?;
    let expected_left = NativeStemsBeamHeadSLinkerRef {
        head: head.reference,
        horizontal: crate::stems_step::NativeStemHeadSide::Left,
    };
    let mut expected_undefined = carried_undefined.clone();
    expected_undefined.push(expected_left);
    let expected_decisions = [
        (crate::stems_step::NativeStemHeadSide::Left, true, true),
        (crate::stems_step::NativeStemHeadSide::Right, true, false),
    ];
    if continuation.returned_linked != Some(false)
        || continuation.processed_head.x_ordinal != 70
        || continuation.processed_head.sig_ordinal != 46
        || continuation.closed_value_changes != 0
        || !continuation.closed_s_linkers.is_empty()
        || continuation.state_after.current_index != 62
        || continuation.state_after.undefined_sides != expected_undefined
        || continuation.state_after.unlinked_heads
            != expected_undefined
                .iter()
                .map(|cell| cell.head)
                .collect::<Vec<_>>()
        || continuation.side_decisions.len() != expected_decisions.len()
        || !continuation
            .side_decisions
            .iter()
            .zip(expected_decisions)
            .all(|(decision, (side, top, bottom))| {
                decision.side == side
                    && !decision.linked_before
                    && !decision.closed_before
                    && decision.top_can_link == Some(top)
                    && decision.bottom_can_link == Some(bottom)
            })
    {
        return Err(stage(
            "HEADS-open-frontier-result",
            "order61 open frontier did not produce the authenticated undefined continuation",
        ));
    }
    Ok(continuation)
}

/// Continue the bounded open/undefined frontier at order 68.
///
/// x0/SIG51 reaches an already materialized StemInter 2384/glyph322.  The
/// C-link envelope is a no-op: Java reports LEFT Both and RIGHT Neither,
/// records a fourth undefined LEFT side, returns false, and advances to
/// order 69 without changing SIG, linker, or allocator state.
pub fn advance_native_stems_head_open_frontier_order68(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 68 {
        return Err(stage(
            "HEADS-open-frontier",
            "carrier is not the authenticated order68 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61], "order68")?;
    let head = carrier
        .heads
        .get(68)
        .ok_or_else(|| stage("HEADS-open-frontier", "order68 head is missing"))?;
    if head.reference.x_ordinal != 0 || head.reference.sig_ordinal != 51 {
        return Err(stage("HEADS-open-frontier", "carrier head is not x0/SIG51"));
    }
    for horizontal in [
        crate::stems_step::NativeStemHeadSide::Left,
        crate::stems_step::NativeStemHeadSide::Right,
    ] {
        let cell = head
            .sides
            .iter()
            .find(|cell| cell.reference.horizontal == horizontal)
            .ok_or_else(|| stage("HEADS-open-frontier", "order68 side cell is missing"))?;
        if cell.linked || cell.closed {
            return Err(stage(
                "HEADS-open-frontier",
                "order68 side is not the authenticated open cell",
            ));
        }
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 44)
        .ok_or_else(|| {
            stage(
                "HEADS-open-frontier",
                "order68 existing StemInter 2384/glyph322 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-open-frontier",
            "order68 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        Some(head_reachability),
        head_builders,
        plans,
    )?;
    let expected_left = NativeStemsBeamHeadSLinkerRef {
        head: head.reference,
        horizontal: crate::stems_step::NativeStemHeadSide::Left,
    };
    let mut expected_undefined = carried_undefined.clone();
    expected_undefined.push(expected_left);
    let mut expected_unlinked = carried_undefined
        .iter()
        .map(|cell| cell.head)
        .collect::<Vec<_>>();
    expected_unlinked.push(head.reference);
    let expected_decisions = [
        (crate::stems_step::NativeStemHeadSide::Left, true, true),
        (crate::stems_step::NativeStemHeadSide::Right, false, false),
    ];
    if continuation.returned_linked != Some(false)
        || continuation.processed_head.x_ordinal != 0
        || continuation.processed_head.sig_ordinal != 51
        || continuation.closed_value_changes != 0
        || !continuation.closed_s_linkers.is_empty()
        || continuation.state_after.current_index != 69
        || continuation.state_after.undefined_sides != expected_undefined
        || continuation.state_after.unlinked_heads != expected_unlinked
        || continuation.side_decisions.len() != expected_decisions.len()
        || !continuation
            .side_decisions
            .iter()
            .zip(expected_decisions)
            .all(|(decision, (side, top, bottom))| {
                decision.side == side
                    && !decision.linked_before
                    && !decision.closed_before
                    && decision.top_can_link == Some(top)
                    && decision.bottom_can_link == Some(bottom)
            })
    {
        return Err(stage(
            "HEADS-open-frontier-result",
            "order68 open frontier did not produce the authenticated undefined continuation",
        ));
    }
    Ok(continuation)
}

/// Continue the bounded open/undefined frontier at order 75.
///
/// x31/SIG47 reaches an already materialized StemInter 2383/glyph314.  The
/// C-link envelope is a no-op: Java reports LEFT Both and RIGHT TopOnly,
/// records a fifth undefined LEFT side, returns false, and advances to
/// order 76 without changing SIG, linker, or allocator state.
pub fn advance_native_stems_head_open_frontier_order75(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 75 {
        return Err(stage(
            "HEADS-open-frontier",
            "carrier is not the authenticated order75 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68], "order75")?;
    let head = carrier
        .heads
        .get(75)
        .ok_or_else(|| stage("HEADS-open-frontier", "order75 head is missing"))?;
    if head.reference.x_ordinal != 31 || head.reference.sig_ordinal != 47 {
        return Err(stage(
            "HEADS-open-frontier",
            "carrier head is not x31/SIG47",
        ));
    }
    for horizontal in [
        crate::stems_step::NativeStemHeadSide::Left,
        crate::stems_step::NativeStemHeadSide::Right,
    ] {
        let cell = head
            .sides
            .iter()
            .find(|cell| cell.reference.horizontal == horizontal)
            .ok_or_else(|| stage("HEADS-open-frontier", "order75 side cell is missing"))?;
        if cell.linked || cell.closed {
            return Err(stage(
                "HEADS-open-frontier",
                "order75 side is not the authenticated open cell",
            ));
        }
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 43)
        .ok_or_else(|| {
            stage(
                "HEADS-open-frontier",
                "order75 existing StemInter 2383/glyph314 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-open-frontier",
            "order75 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        Some(head_reachability),
        head_builders,
        plans,
    )?;
    let expected_left = NativeStemsBeamHeadSLinkerRef {
        head: head.reference,
        horizontal: crate::stems_step::NativeStemHeadSide::Left,
    };
    let mut expected_undefined = carried_undefined.clone();
    expected_undefined.push(expected_left);
    let mut expected_unlinked = carried_undefined
        .iter()
        .map(|cell| cell.head)
        .collect::<Vec<_>>();
    expected_unlinked.push(head.reference);
    let expected_decisions = [
        (crate::stems_step::NativeStemHeadSide::Left, true, true),
        (crate::stems_step::NativeStemHeadSide::Right, true, false),
    ];
    if continuation.returned_linked != Some(false)
        || continuation.processed_head.x_ordinal != 31
        || continuation.processed_head.sig_ordinal != 47
        || continuation.closed_value_changes != 0
        || !continuation.closed_s_linkers.is_empty()
        || continuation.state_after.current_index != 76
        || continuation.state_after.undefined_sides != expected_undefined
        || continuation.state_after.unlinked_heads != expected_unlinked
        || continuation.side_decisions.len() != expected_decisions.len()
        || !continuation
            .side_decisions
            .iter()
            .zip(expected_decisions)
            .all(|(decision, (side, top, bottom))| {
                decision.side == side
                    && !decision.linked_before
                    && !decision.closed_before
                    && decision.top_can_link == Some(top)
                    && decision.bottom_can_link == Some(bottom)
            })
    {
        return Err(stage(
            "HEADS-open-frontier-result",
            "order75 open frontier did not produce the authenticated undefined continuation",
        ));
    }
    Ok(continuation)
}

/// Decide Java `CLinker.expand`'s `-1` outcome for a bounded phase-2 link
/// attempt.
///
/// Java returns `-1` from two places: at the first show-stopping gap
/// (`contrib > maxYGap`) when the walk has not yet reached the hard tail
/// target, and again after every item has been seen if the tail target is
/// still short.  Either way `link` returns false having built nothing.
/// This helper decides only that outcome; any walk that does not stop at
/// such a gap fails closed, because a successful phase-2 expansion would
/// reach the unported `reuseStem` append path.
fn bounded_phase_two_expand_returns_minus_one(
    corner: NativeStemsHeadCornerRef,
    stem_profile: i32,
    head_builders: &NativeStemsHeadBuilderSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
) -> Result<bool, NativeStemsBeamSidesError> {
    let builder = head_builders
        .builders
        .iter()
        .find(|entry| entry.start == corner)
        .ok_or_else(|| stage("HEADS-phase2-expand", "corner lacks a stem builder"))?;
    let max_gap = head_builders
        .gap_map
        .get(&stem_profile)
        .copied()
        .ok_or_else(|| stage("HEADS-phase2-expand", "builder lacks the gap threshold"))?;
    let reach = head_reachability
        .heads
        .iter()
        .flat_map(|entry| &entry.corners)
        .find(|entry| entry.reference == corner)
        .ok_or_else(|| stage("HEADS-phase2-expand", "corner lacks reachability"))?;
    let minimum_tail = java_rint(1.75 * f64::from(head_builders.interline));
    let y_dir = builder.y_direction;
    let y_hard = reach.reference_point.y + f64::from(y_dir * minimum_tail);
    // Java: `double lastY = theoLine.getY1();` - taken from the theoretical
    // line as stored, before the yDir orientation swap applied to stemLine.
    let mut last_y = builder.theoretical_line.start.y;
    for item in &builder.items {
        if item.kind == NativeStemsHeadBuilderItemKind::Gap {
            if item.contribution > max_gap {
                return Ok(y_dir * java_double_compare(last_y, y_hard) < 0);
            }
            continue;
        }
        last_y = if y_dir > 0 {
            last_y.max(item.line.stop.y)
        } else {
            last_y.min(item.line.start.y)
        };
    }
    // Java checks the hard tail target once more after every item has been
    // seen: `if (yDir * Double.compare(lastY, yHard) < 0) return -1;`.
    if y_dir * java_double_compare(last_y, y_hard) < 0 {
        return Ok(true);
    }
    // The walk reached the tail target, so Java would go on to
    // checkStemRelation and, if that holds, build or reuse a stem.
    Err(stage(
        "HEADS-phase2-expand",
        "phase-2 expansion reaches the hard tail target and the unported checkStemRelation append path",
    ))
}

/// Advance one entry of Java's heads-linking phase 2.
///
/// After phase 1 exhausts the head queue, `StemsRetriever.linkStems` re-runs
/// `HeadLinker.linkSides` with `append=true` over `unlinkedHeads`.  With
/// `append` set, a linked side still short-circuits to a true return, but the
/// closed-side skip no longer applies, so a closed-yet-unlinked side is
/// re-evaluated and may reach a real `link` attempt.  On chula system 1 every
/// one of the five entries leaves the graph untouched, and this consumes them
/// one at a time under that authentication.
pub fn advance_native_stems_head_phase_two_append_retry(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != carrier.heads.len() {
        return Err(stage(
            "HEADS-phase2-append",
            "carrier is not the completed phase-1 terminal",
        ));
    }
    let queue = authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "phase2")?;
    if queue.len() != 5 {
        return Err(stage(
            "HEADS-phase2-append",
            "phase-2 queue is not the authenticated five-head list",
        ));
    }
    let queue_index = carrier.phase_two_index;
    let head_ref = *carrier.unlinked_heads.get(queue_index).ok_or_else(|| {
        stage(
            "HEADS-phase2-append",
            "phase-2 cursor is past the authenticated queue",
        )
    })?;
    let head = carrier
        .heads
        .iter()
        .find(|entry| entry.reference == head_ref)
        .ok_or_else(|| stage("HEADS-phase2-append", "queued head is missing"))?;
    if head_reachability.system_id != head_corners.system_id {
        return Err(stage(
            "HEADS-phase2-append",
            "head reachability belongs to a different system",
        ));
    }

    let mut side_decisions = Vec::new();
    let mut linked = false;
    let mut recorded_undef = false;
    for horizontal in [
        crate::stems_step::NativeStemHeadSide::Left,
        crate::stems_step::NativeStemHeadSide::Right,
    ] {
        let side = head
            .sides
            .iter()
            .find(|cell| cell.reference.horizontal == horizontal)
            .ok_or_else(|| stage("HEADS-phase2-append", "side cell is missing"))?;
        if side.linked {
            // Java: `if (sLinker.isLinked()) { linked = true; continue; }`.
            linked = true;
            side_decisions.push(NativeStemsHeadPhase1SideDecision {
                side: horizontal,
                linked_before: true,
                closed_before: side.closed,
                top_can_link: None,
                bottom_can_link: None,
            });
            continue;
        }
        // Java skips a closed side only when append is false, so phase 2
        // deliberately falls through to the corner evaluation here.
        let top = NativeStemsHeadCornerRef {
            head: head.reference.reference,
            sig_ordinal: head.reference.sig_ordinal,
            x_ordinal: head.reference.x_ordinal,
            horizontal,
            vertical: crate::stems_step::NativeStemVerticalSide::Top,
        };
        let bottom = NativeStemsHeadCornerRef {
            vertical: crate::stems_step::NativeStemVerticalSide::Bottom,
            ..top
        };
        let top_ok = bounded_head_can_link(
            top,
            0,
            head_builders,
            &carrier.beam_state.s_cells,
            plans.min_linker_length,
            true,
        )?;
        let bottom_ok = bounded_head_can_link(
            bottom,
            0,
            head_builders,
            &carrier.beam_state.s_cells,
            plans.min_linker_length,
            true,
        )?;
        side_decisions.push(NativeStemsHeadPhase1SideDecision {
            side: horizontal,
            linked_before: false,
            closed_before: side.closed,
            top_can_link: Some(top_ok),
            bottom_can_link: Some(bottom_ok),
        });
        match (top_ok, bottom_ok) {
            (true, true) => {
                let stump_of = |corner: NativeStemsHeadCornerRef| {
                    head_reachability
                        .heads
                        .iter()
                        .flat_map(|entry| &entry.corners)
                        .find(|reach| reach.reference == corner)
                        .map(|reach| reach.stump)
                        .ok_or_else(|| {
                            stage("HEADS-phase2-append", "reachability corner is missing")
                        })
                };
                match (stump_of(top)?, stump_of(bottom)?) {
                    (Some(top_stump), Some(bottom_stump)) if top_stump == bottom_stump => {
                        // Java re-adds the side to an undefs EnumSet that
                        // already holds it, and returns false immediately.
                        recorded_undef = true;
                    }
                    _ => {
                        return Err(stage(
                            "HEADS-phase2-append",
                            "phase-2 dual-corner differing-stump branch is unported",
                        ));
                    }
                }
                break;
            }
            (true, false) => {
                // Java calls `clTop.link(..., append)`.  Every chula system 1
                // attempt fails inside expand, which returns -1 before the
                // hard tail target and so builds nothing.
                if !bounded_phase_two_expand_returns_minus_one(
                    top,
                    0,
                    head_builders,
                    head_reachability,
                )? {
                    return Err(stage(
                        "HEADS-phase2-append",
                        "phase-2 link attempt reaches the unported reuseStem append path",
                    ));
                }
            }
            (false, true) => {
                if !bounded_phase_two_expand_returns_minus_one(
                    bottom,
                    0,
                    head_builders,
                    head_reachability,
                )? {
                    return Err(stage(
                        "HEADS-phase2-append",
                        "phase-2 link attempt reaches the unported reuseStem append path",
                    ));
                }
            }
            (false, false) => {}
        }
    }
    if !linked && !recorded_undef {
        return Err(stage(
            "HEADS-phase2-append",
            "phase-2 retry reaches the unported rather-good retry and closure branch",
        ));
    }

    let mut shadow = carrier.clone();
    shadow.phase_two_index = queue_index + 1;
    // A phase-2 entry that returns true runs Java's ordered closure over the
    // heads sharing its already linked stems, exactly as a phase-1 return
    // does.  Entries that return false stop at the undef branch first and
    // close nothing.
    let (closed_s_linkers, closed_value_changes) = if linked {
        let current = shadow
            .heads
            .iter()
            .find(|entry| entry.reference == head_ref)
            .ok_or_else(|| stage("HEADS-phase2-append", "queued head is missing"))?
            .clone();
        close_heads_sharing_prelinked_stems(
            &shadow.beam_state.sig,
            &shadow.beam_state.bindings,
            &mut shadow.beam_state.s_cells,
            &mut shadow.heads,
            &current,
        )?
    } else {
        (Vec::new(), 0)
    };
    Ok(NativeStemsHeadPhase1Continuation {
        processed_head: head_ref,
        side_decisions,
        returned_linked: Some(linked),
        closed_s_linkers,
        closed_value_changes,
        state_after: Box::new(shadow),
    })
}

#[derive(Clone, Copy)]
struct NativeFinalizeHeadStemLink {
    edge: NativeSigEdgeId,
    stem: NativeSigVertexId,
    stem_grade: f64,
    relation_grade: f64,
    payload: NativeSigHeadStemPayload,
}

/// Execute Java's generic `StemsRetriever.finalizeStems` terminal.
///
/// The completed carrier owns every input used by `checkHeadStems` and
/// `checkNeededStems`: the live SIG, stem exclusions and grades, physical stem
/// medians, head geometry, and Java's reverse-grade head order. Mutations are
/// applied to a clone, so malformed evidence fails without changing the input.
pub fn finalize_native_stems(
    carrier: &NativeStemsHeadPhase1Carrier,
) -> Result<NativeStemsFinalizeTransaction, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed
        || carrier.current_index != carrier.heads.len()
        || carrier.phase_two_index != carrier.unlinked_heads.len()
    {
        return Err(stage(
            "finalizeStems-frontier",
            "carrier has not completed both HEADS linking phases",
        ));
    }
    carrier
        .beam_state
        .sig
        .validate_integrity()
        .map_err(|error| stage("finalizeStems-SIG", error))?;
    carrier
        .beam_state
        .bindings
        .validate_against(&carrier.beam_state.sig)
        .map_err(|error| stage("finalizeStems-bindings", error))?;
    let live_vertices = carrier
        .beam_state
        .sig
        .vertices
        .iter()
        .filter(|vertex| vertex.active && !vertex.removed)
        .count();
    let live_edges = carrier
        .beam_state
        .sig
        .edges
        .iter()
        .filter(|edge| edge.active)
        .count();
    if carrier.beam_state.sig.system_id == 1
        && carrier.heads.len() == 102
        && carrier.phase_two_index == 5
        && live_vertices == 267
        && live_edges == 370
        && carrier
            .beam_state
            .latest_base_apply
            .transaction_state
            .system_stems
            .known_stems
            .len()
            == 46
    {
        // Preserve the stricter frozen v104 Chula authentication as a
        // specialization of the generic terminal.
        authenticate_chula_finalize_native_stems(carrier)?;
    }

    let mut shadow = carrier.clone();
    let mut multiple_stem_heads = Vec::new();
    for head in &carrier.heads {
        let head_id = shadow
            .beam_state
            .bindings
            .head_vertices
            .get(&head.reference.reference)
            .copied()
            .ok_or_else(|| stage("finalizeStems-head", "head lacks its live SIG binding"))?;
        if shadow
            .beam_state
            .sig
            .vertex(head_id.0)
            .is_none_or(|vertex| vertex.kind != NativeSigInterKind::Head)
        {
            return Err(stage(
                "finalizeStems-head",
                "head binding does not resolve to a live HeadInter",
            ));
        }
        let mut stems = native_finalize_head_stem_links(&shadow, head_id)?;
        if stems.len() > 1 {
            multiple_stem_heads.push(head.reference);
            stems.sort_by(|left, right| right.stem_grade.total_cmp(&left.stem_grade));
            for partition in native_finalize_stem_partitions(&shadow.beam_state.sig, &stems)? {
                let mut links = partition
                    .into_iter()
                    .filter_map(|index| {
                        let link = stems[index];
                        shadow.beam_state.sig.edges[link.edge.0]
                            .active
                            .then_some(link)
                    })
                    .collect::<Vec<_>>();
                while links.len() > 2 {
                    remove_native_finalize_worst(&mut shadow, &mut links)?;
                }
                if links.len() == 2
                    && !native_finalize_is_canonical_share(&shadow, head_id, &links)?
                {
                    remove_native_finalize_worst(&mut shadow, &mut links)?;
                }
            }
        }
    }

    let mut no_stem_heads = Vec::new();
    for head in &carrier.heads {
        let head_id = shadow.beam_state.bindings.head_vertices[&head.reference.reference];
        if native_finalize_head_stem_links(&shadow, head_id)?.is_empty() {
            no_stem_heads.push(head.reference);
            if !shadow.beam_state.sig.vertices[head_id.0].abnormal {
                shadow
                    .beam_state
                    .sig
                    .set_abnormal(head_id, true)
                    .map_err(|error| stage("finalizeStems-abnormal", error))?;
            }
        }
    }
    reconcile_known_stems(
        &mut shadow.beam_state.latest_base_apply,
        &shadow.beam_state.sig,
        &shadow.beam_state.bindings,
    )?;
    let abnormal_value_changes = carrier
        .beam_state
        .sig
        .vertices
        .iter()
        .zip(&shadow.beam_state.sig.vertices)
        .filter(|(before, after)| before.abnormal != after.abnormal)
        .count();
    let abnormal_heads = carrier
        .heads
        .iter()
        .filter_map(|head| {
            let id = shadow.beam_state.bindings.head_vertices[&head.reference.reference];
            shadow.beam_state.sig.vertices[id.0]
                .abnormal
                .then_some(head.reference)
        })
        .collect();
    let removed_head_stem_relations = carrier
        .beam_state
        .sig
        .edges
        .iter()
        .zip(&shadow.beam_state.sig.edges)
        .filter_map(|(before, after)| {
            (before.kind == NativeSigRelationKind::HeadStem && before.active && !after.active)
                .then_some(NativeSigEdgeId(before.ordinal))
        })
        .collect();
    shadow
        .beam_state
        .sig
        .validate_integrity()
        .map_err(|error| stage("finalizeStems-result", error))?;

    Ok(NativeStemsFinalizeTransaction {
        checked_heads: carrier.heads.len(),
        multiple_stem_heads,
        no_stem_heads,
        abnormal_heads,
        removed_head_stem_relations,
        abnormal_value_changes,
        state_after: Box::new(shadow),
    })
}

fn native_finalize_head_stem_links(
    carrier: &NativeStemsHeadPhase1Carrier,
    head: NativeSigVertexId,
) -> Result<Vec<NativeFinalizeHeadStemLink>, NativeStemsBeamSidesError> {
    let mut links = Vec::new();
    for edge in carrier
        .beam_state
        .sig
        .incident_edges(head.0)
        .map_err(|error| stage("finalizeStems-HeadStem", error))?
        .into_iter()
        .filter(|edge| edge.kind == NativeSigRelationKind::HeadStem)
    {
        if edge.source != head.0 {
            return Err(stage(
                "finalizeStems-HeadStem",
                "HeadStem relation is not directed from its head",
            ));
        }
        let stem = carrier
            .beam_state
            .sig
            .vertex(edge.target)
            .ok_or_else(|| stage("finalizeStems-HeadStem", "target stem is not live"))?;
        let relation_grade = edge
            .support
            .ok_or_else(|| stage("finalizeStems-HeadStem", "relation lacks support grade"))?
            .grade;
        let payload = edge
            .head_stem
            .ok_or_else(|| stage("finalizeStems-HeadStem", "relation lacks HeadStem payload"))?;
        if stem.kind != NativeSigInterKind::Stem
            || !stem.grade.is_finite()
            || !relation_grade.is_finite()
            || !payload.dy.is_finite()
        {
            return Err(stage(
                "finalizeStems-HeadStem",
                "relation carries invalid stem or measurement state",
            ));
        }
        links.push(NativeFinalizeHeadStemLink {
            edge: NativeSigEdgeId(edge.ordinal),
            stem: NativeSigVertexId(edge.target),
            stem_grade: stem.grade,
            relation_grade,
            payload,
        });
    }
    Ok(links)
}

fn native_finalize_stem_partitions(
    sig: &NativeSigSystem,
    stems: &[NativeFinalizeHeadStemLink],
) -> Result<Vec<Vec<usize>>, NativeStemsBeamSidesError> {
    let mut concurrent = vec![BTreeSet::new(); stems.len()];
    let mut conflict = false;
    for (index, stem) in stems.iter().enumerate() {
        for edge in sig
            .incident_edges(stem.stem.0)
            .map_err(|error| stage("finalizeStems-partitions", error))?
            .into_iter()
            .filter(|edge| edge.kind == NativeSigRelationKind::Exclusion)
        {
            let opposite = if edge.source == stem.stem.0 {
                edge.target
            } else {
                edge.source
            };
            if let Some(other) = stems
                .iter()
                .position(|candidate| candidate.stem.0 == opposite)
                && other > index
            {
                concurrent[index].insert(other);
                conflict = true;
            }
        }
    }
    if !conflict {
        return Ok(vec![(0..stems.len()).collect()]);
    }

    let mut sequences = vec![vec![0_i8; stems.len()]];
    for (index, forbidden) in concurrent.iter().enumerate() {
        let prior_count = sequences.len();
        for sequence_index in 0..prior_count {
            if sequences[sequence_index][index] == -1 {
                continue;
            }
            sequences[sequence_index][index] = 1;
            if !forbidden.is_empty() {
                let mut excluded = sequences[sequence_index].clone();
                excluded[index] = 0;
                sequences.push(excluded);
                for &other in forbidden {
                    sequences[sequence_index][other] = -1;
                }
            }
        }
    }
    Ok(sequences
        .into_iter()
        .map(|sequence| {
            sequence
                .into_iter()
                .enumerate()
                .filter_map(|(index, selected)| (selected == 1).then_some(index))
                .collect()
        })
        .collect())
}

fn remove_native_finalize_worst(
    carrier: &mut NativeStemsHeadPhase1Carrier,
    links: &mut Vec<NativeFinalizeHeadStemLink>,
) -> Result<(), NativeStemsBeamSidesError> {
    let (index, _) = links
        .iter()
        .enumerate()
        .map(|(index, link)| {
            // Support.getTargetRatio() = 1 + 10 * HeadStemRelation grade.
            let target_ratio = 1.0 + (10.0 * link.relation_grade);
            (index, link.stem_grade * (target_ratio - 1.0))
        })
        .min_by(|(_, left), (_, right)| left.total_cmp(right))
        .ok_or_else(|| stage("finalizeStems-cleaner", "empty contribution set"))?;
    let discarded = links.remove(index);
    let edge = carrier.beam_state.sig.edges[discarded.edge.0];
    carrier
        .beam_state
        .sig
        .remove_edge(discarded.edge)
        .map_err(|error| stage("finalizeStems-cleaner", error))?;
    for vertex in [NativeSigVertexId(edge.source), discarded.stem] {
        let abnormal = !carrier
            .beam_state
            .sig
            .incident_edges(vertex.0)
            .map_err(|error| stage("finalizeStems-callback", error))?
            .into_iter()
            .any(|relation| relation.kind == NativeSigRelationKind::HeadStem);
        carrier
            .beam_state
            .sig
            .set_abnormal(vertex, abnormal)
            .map_err(|error| stage("finalizeStems-callback", error))?;
    }
    Ok(())
}

fn native_finalize_is_canonical_share(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_id: NativeSigVertexId,
    links: &[NativeFinalizeHeadStemLink],
) -> Result<bool, NativeStemsBeamSidesError> {
    if links.iter().any(|link| link.payload.dy > 0.2) {
        return Ok(false);
    }
    let left = links
        .iter()
        .find(|link| link.payload.head_side == crate::stems_step::NativeStemHeadSide::Left);
    let right = links
        .iter()
        .find(|link| link.payload.head_side == crate::stems_step::NativeStemHeadSide::Right);
    let (Some(left), Some(right)) = (left, right) else {
        return Ok(false);
    };
    let geometry = |stem: NativeSigVertexId| {
        let identity = carrier
            .beam_state
            .bindings
            .stem_vertices
            .iter()
            .find_map(|(identity, id)| (*id == stem).then_some(*identity))
            .ok_or_else(|| stage("finalizeStems-canonical", "stem lacks native binding"))?;
        carrier
            .beam_state
            .latest_base_apply
            .transaction_state
            .system_stems
            .known_stems
            .iter()
            .find(|known| known.stem_identity == identity)
            .map(|known| known.geometry.median)
            .ok_or_else(|| stage("finalizeStems-canonical", "stem lacks physical median"))
    };
    let left_line = geometry(left.stem)?;
    let right_line = geometry(right.stem)?;
    let head = &carrier.beam_state.sig.vertices[head_id.0];
    let head_center_y = head.bounds.y + (head.bounds.height / 2);
    let left_mid = (left_line.start.y + left_line.stop.y) / 2.0;
    let right_mid = (right_line.start.y + right_line.stop.y) / 2.0;
    if f64::from(head_center_y) >= left_mid || f64::from(head_center_y) <= right_mid {
        return Ok(false);
    }
    let portion = |line: crate::stems_step::NativeStemLine, extension_y: f64| {
        let margin = f64::from(head.bounds.height) * 0.275;
        let midpoint = (line.start.y + line.stop.y) / 2.0;
        if extension_y >= midpoint {
            if extension_y > line.stop.y - margin {
                1
            } else {
                0
            }
        } else if extension_y < line.start.y + margin {
            -1
        } else {
            0
        }
    };
    Ok(portion(left_line, left.payload.extension_point.y) == -1
        && portion(right_line, right.payload.extension_point.y) == 1)
}

/// Execute the bounded Chula system-1 `finalizeStems` terminal.
///
/// This authenticates both private Java substeps from owned native state.
/// `checkHeadStems` would only mutate a head with more than one live
/// `HeadStemRelation`; none exists here. `checkNeededStems` would mark a
/// stemless stem-head abnormal; the only two such heads are already abnormal,
/// so the complete finalizer is an intentional no-op. Any different census
/// fails closed before a carrier is returned.
fn authenticate_chula_finalize_native_stems(
    carrier: &NativeStemsHeadPhase1Carrier,
) -> Result<NativeStemsFinalizeTransaction, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed
        || carrier.current_index != carrier.heads.len()
        || carrier.heads.len() != 102
        || carrier.phase_two_index != carrier.unlinked_heads.len()
        || carrier.phase_two_index != 5
    {
        return Err(stage(
            "finalizeStems-frontier",
            "carrier is not the authenticated completed HEADS terminal",
        ));
    }
    authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "finalizeStems")?;
    carrier
        .beam_state
        .sig
        .validate_integrity()
        .map_err(|error| stage("finalizeStems-SIG", error))?;
    carrier
        .beam_state
        .bindings
        .validate_against(&carrier.beam_state.sig)
        .map_err(|error| stage("finalizeStems-bindings", error))?;

    let live_vertices = carrier
        .beam_state
        .sig
        .vertices
        .iter()
        .filter(|vertex| vertex.active && !vertex.removed)
        .count();
    let live_edges = carrier
        .beam_state
        .sig
        .edges
        .iter()
        .filter(|edge| edge.active)
        .count();
    let known_stems = &carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems;
    // The Java oracle counts the full system SIG (685/706). This carrier owns
    // the corresponding native STEMS projection, whose exact live domain is
    // 267 vertices / 370 edges, plus all 46 system stems.
    if live_vertices != 267 || live_edges != 370 || known_stems.len() != 46 {
        return Err(stage(
            "finalizeStems-graph",
            format!(
                "carrier graph differs from the authenticated finalizer census: {live_vertices}/{live_edges}/{}",
                known_stems.len()
            ),
        ));
    }

    let mut multiple_stem_heads = Vec::new();
    let mut no_stem_heads = Vec::new();
    let mut abnormal_heads = Vec::new();
    for head in &carrier.heads {
        let vertex_id = carrier
            .beam_state
            .bindings
            .head_vertices
            .get(&head.reference.reference)
            .ok_or_else(|| stage("finalizeStems-head", "head lacks its live SIG binding"))?;
        let vertex = carrier
            .beam_state
            .sig
            .vertex(vertex_id.0)
            .ok_or_else(|| stage("finalizeStems-head", "head binding is not live"))?;
        if vertex.kind != NativeSigInterKind::Head {
            return Err(stage(
                "finalizeStems-head",
                "head binding does not resolve to a HeadInter",
            ));
        }
        let head_stems = carrier
            .beam_state
            .sig
            .incident_edges(vertex_id.0)
            .map_err(|error| stage("finalizeStems-HeadStem", error))?
            .into_iter()
            .filter(|edge| edge.kind == NativeSigRelationKind::HeadStem)
            .collect::<Vec<_>>();
        if head_stems.iter().any(|edge| edge.source != vertex_id.0) {
            return Err(stage(
                "finalizeStems-HeadStem",
                "HeadStem relation is not directed from its head",
            ));
        }
        match head_stems.len() {
            0 => no_stem_heads.push(head.reference),
            1 => {}
            _ => multiple_stem_heads.push(head.reference),
        }
        if vertex.abnormal {
            abnormal_heads.push(head.reference);
        }
    }

    let expected_stemless = [(32, 50), (31, 47)];
    let stemless = no_stem_heads
        .iter()
        .map(|head| (head.x_ordinal, head.sig_ordinal))
        .collect::<Vec<_>>();
    let abnormal = abnormal_heads
        .iter()
        .map(|head| (head.x_ordinal, head.sig_ordinal))
        .collect::<Vec<_>>();
    if !multiple_stem_heads.is_empty()
        || stemless != expected_stemless
        || abnormal != expected_stemless
        || no_stem_heads.iter().any(|head| {
            carrier
                .beam_state
                .bindings
                .head_vertices
                .get(&head.reference)
                .and_then(|id| carrier.beam_state.sig.vertex(id.0))
                .is_none_or(|vertex| vertex.shape.as_deref() != Some("NOTEHEAD_VOID"))
        })
    {
        return Err(stage(
            "finalizeStems-census",
            "finalizer reaches an unported cleaner or abnormal mutation",
        ));
    }

    Ok(NativeStemsFinalizeTransaction {
        checked_heads: carrier.heads.len(),
        multiple_stem_heads,
        no_stem_heads,
        abnormal_heads,
        removed_head_stem_relations: Vec::new(),
        abnormal_value_changes: 0,
        state_after: Box::new(carrier.clone()),
    })
}

/// Build the exact carried undefined-LEFT list for the given queue indexes
/// and authenticate that the carrier holds precisely that list.
fn authenticated_carried_undefined_sides(
    carrier: &NativeStemsHeadPhase1Carrier,
    queue_indexes: &[usize],
    order_label: &str,
) -> Result<Vec<NativeStemsBeamHeadSLinkerRef>, NativeStemsBeamSidesError> {
    let carried = queue_indexes
        .iter()
        .map(|&queue_index| {
            carrier
                .heads
                .get(queue_index)
                .map(|head| NativeStemsBeamHeadSLinkerRef {
                    head: head.reference,
                    horizontal: crate::stems_step::NativeStemHeadSide::Left,
                })
                .ok_or_else(|| {
                    stage(
                        "HEADS-existing-stem-retry-frontier",
                        format!("{order_label} carrier lacks an undef predecessor head"),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if carrier.undefined_sides != carried {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            format!("{order_label} carrier lacks the carried undefined LEFT sides"),
        ));
    }
    if carrier.unlinked_heads != carried.iter().map(|cell| cell.head).collect::<Vec<_>>() {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            format!("{order_label} carrier lacks the carried phase-2 unlinked queue"),
        ));
    }
    Ok(carried)
}

/// Reconcile the bounded existing-stem retry at order 62.
///
/// x9/SIG8 retries LEFT against the already linked existing StemInter
/// 2355/glyph318: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x10's cells without touching
/// SIG, allocator, or system-stem state.  The three undefined LEFT sides
/// carried from orders 50, 60, and 61 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order62(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 62 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order62 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61], "order62")?;
    let head = carrier.heads.get(62).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order62 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 9 || head.reference.sig_ordinal != 8 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x9/SIG8",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order62 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 15)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order62 existing StemInter 2355/glyph318 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order62 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 9
        || continuation.processed_head.sig_ordinal != 8
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 63
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order62 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 63.
///
/// x41/SIG92 retries LEFT against the already linked existing StemInter
/// 2352/glyph293: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x42's cells without touching
/// SIG, allocator, or system-stem state.  The three undefined LEFT sides
/// carried from orders 50, 60, and 61 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order63(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 63 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order63 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61], "order63")?;
    let head = carrier.heads.get(63).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order63 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 41 || head.reference.sig_ordinal != 92 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x41/SIG92",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order63 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 12)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order63 existing StemInter 2352/glyph293 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order63 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 41
        || continuation.processed_head.sig_ordinal != 92
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 64
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order63 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 64.
///
/// x3/SIG6 retries LEFT against the already linked existing StemInter
/// 2354/glyph315: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x4's cells without touching
/// SIG, allocator, or system-stem state.  The three undefined LEFT sides
/// and the phase-2 queue carried from orders 50, 60, and 61 stay recorded
/// and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order64(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 64 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order64 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61], "order64")?;
    let head = carrier.heads.get(64).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order64 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 3 || head.reference.sig_ordinal != 6 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x3/SIG6",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order64 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 14)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order64 existing StemInter 2354/glyph315 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order64 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 3
        || continuation.processed_head.sig_ordinal != 6
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 65
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order64 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 65.
///
/// x58/SIG73 retries LEFT against the already linked existing StemInter
/// 2363/glyph311: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x59's cells without touching
/// SIG, allocator, or system-stem state.  The three undefined LEFT sides
/// and the phase-2 queue carried from orders 50, 60, and 61 stay recorded
/// and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order65(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 65 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order65 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61], "order65")?;
    let head = carrier.heads.get(65).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order65 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 58 || head.reference.sig_ordinal != 73 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x58/SIG73",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order65 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 23)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order65 existing StemInter 2363/glyph311 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order65 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 58
        || continuation.processed_head.sig_ordinal != 73
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 66
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order65 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 66.
///
/// x13/SIG0 retries LEFT against the already linked existing StemInter
/// 2340/glyph294: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x14's cells without touching
/// SIG, allocator, or system-stem state.  The three undefined LEFT sides
/// and the phase-2 queue carried from orders 50, 60, and 61 stay recorded
/// and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order66(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 66 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order66 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61], "order66")?;
    let head = carrier.heads.get(66).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order66 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 13 || head.reference.sig_ordinal != 0 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x13/SIG0",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order66 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 0)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order66 existing StemInter 2340/glyph294 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order66 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 13
        || continuation.processed_head.sig_ordinal != 0
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 67
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order66 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 69.
///
/// x87/SIG83 retries LEFT against the already linked existing StemInter
/// 2367/glyph295: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x88's cells without touching
/// SIG, allocator, or system-stem state.  The four undefined LEFT sides
/// and the phase-2 queue carried from orders 50, 60, 61, and 68 stay
/// recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order69(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 69 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order69 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68], "order69")?;
    let head = carrier.heads.get(69).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order69 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 87 || head.reference.sig_ordinal != 83 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x87/SIG83",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order69 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 27)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order69 existing StemInter 2367/glyph295 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order69 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 87
        || continuation.processed_head.sig_ordinal != 83
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 70
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order69 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 71.
///
/// x77/SIG38 retries LEFT against the already linked existing StemInter
/// 2370/glyph309: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x78's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, and 68 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order71(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 71 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order71 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68], "order71")?;
    let head = carrier.heads.get(71).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order71 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 77 || head.reference.sig_ordinal != 38 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x77/SIG38",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order71 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 30)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order71 existing StemInter 2370/glyph309 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order71 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 77
        || continuation.processed_head.sig_ordinal != 38
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 72
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order71 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 74.
///
/// x49/SIG71 retries LEFT against the already linked existing StemInter
/// 2353/glyph317: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x50's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, and 68 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order74(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 74 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order74 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68], "order74")?;
    let head = carrier.heads.get(74).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order74 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 49 || head.reference.sig_ordinal != 71 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x49/SIG71",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order74 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 13)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order74 existing StemInter 2353/glyph317 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order74 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 49
        || continuation.processed_head.sig_ordinal != 71
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 75
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order74 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 76.
///
/// x66/SIG58 retries LEFT against the already linked existing StemInter
/// 2375/glyph308: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x67's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order76(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 76 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order76 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order76")?;
    let head = carrier.heads.get(76).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order76 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 66 || head.reference.sig_ordinal != 58 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x66/SIG58",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order76 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 35)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order76 existing StemInter 2375/glyph308 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order76 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 66
        || continuation.processed_head.sig_ordinal != 58
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 77
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order76 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 77.
///
/// x64/SIG94 retries LEFT against the already linked existing StemInter
/// 2346/glyph291: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x65's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order77(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 77 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order77 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order77")?;
    let head = carrier.heads.get(77).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order77 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 64 || head.reference.sig_ordinal != 94 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x64/SIG94",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order77 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 6)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order77 existing StemInter 2346/glyph291 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order77 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 64
        || continuation.processed_head.sig_ordinal != 94
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 78
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order77 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 78.
///
/// x82/SIG20 retries LEFT against the already linked existing StemInter
/// 2358/glyph301: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x83's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order78(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 78 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order78 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order78")?;
    let head = carrier.heads.get(78).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order78 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 82 || head.reference.sig_ordinal != 20 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x82/SIG20",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order78 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 18)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order78 existing StemInter 2358/glyph301 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order78 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 82
        || continuation.processed_head.sig_ordinal != 20
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 79
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order78 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 79.
///
/// x17/SIG10 retries LEFT against the already linked existing StemInter
/// 2372/glyph310: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x18's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order79(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 79 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order79 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order79")?;
    let head = carrier.heads.get(79).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order79 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 17 || head.reference.sig_ordinal != 10 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x17/SIG10",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order79 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 32)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order79 existing StemInter 2372/glyph310 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order79 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 17
        || continuation.processed_head.sig_ordinal != 10
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 80
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order79 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 80.
///
/// x29/SIG66 retries LEFT against the already linked existing StemInter
/// 2357/glyph313: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x30's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order80(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 80 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order80 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order80")?;
    let head = carrier.heads.get(80).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order80 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 29 || head.reference.sig_ordinal != 66 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x29/SIG66",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order80 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 17)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order80 existing StemInter 2357/glyph313 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order80 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 29
        || continuation.processed_head.sig_ordinal != 66
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 81
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order80 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 81.
///
/// x98/SIG60 retries LEFT against the already linked existing StemInter
/// 2365/glyph330: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x99's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order81(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 81 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order81 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order81")?;
    let head = carrier.heads.get(81).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order81 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 98 || head.reference.sig_ordinal != 60 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x98/SIG60",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order81 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 25)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order81 existing StemInter 2365/glyph330 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order81 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 98
        || continuation.processed_head.sig_ordinal != 60
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 82
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order81 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 82.
///
/// x80/SIG32 retries LEFT against the already linked existing StemInter
/// 2371/glyph306: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x81's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order82(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 82 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order82 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order82")?;
    let head = carrier.heads.get(82).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order82 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 80 || head.reference.sig_ordinal != 32 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x80/SIG32",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order82 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 31)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order82 existing StemInter 2371/glyph306 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order82 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 80
        || continuation.processed_head.sig_ordinal != 32
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 83
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order82 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 83.
///
/// x24/SIG90 retries LEFT against the already linked existing StemInter
/// 2356/glyph292: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x25's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order83(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 83 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order83 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order83")?;
    let head = carrier.heads.get(83).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order83 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 24 || head.reference.sig_ordinal != 90 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x24/SIG90",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order83 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 16)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order83 existing StemInter 2356/glyph292 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order83 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 24
        || continuation.processed_head.sig_ordinal != 90
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 84
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order83 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 84.
///
/// x94/SIG99 retries LEFT against the already linked existing StemInter
/// 2364/glyph297: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x95's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order84(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 84 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order84 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order84")?;
    let head = carrier.heads.get(84).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order84 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 94 || head.reference.sig_ordinal != 99 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x94/SIG99",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order84 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 24)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order84 existing StemInter 2364/glyph297 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order84 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 94
        || continuation.processed_head.sig_ordinal != 99
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 85
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order84 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 85.
///
/// x79/SIG40 retries LEFT against the already linked existing StemInter
/// 2371/glyph306: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and re-writes the already-closed cells of siblings
/// x80 and x81 without a value change or any effect on
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order85(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 85 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order85 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order85")?;
    let head = carrier.heads.get(85).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order85 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 79 || head.reference.sig_ordinal != 40 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x79/SIG40",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order85 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 31)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order85 existing StemInter 2371/glyph306 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order85 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 79
        || continuation.processed_head.sig_ordinal != 40
        || continuation.closed_value_changes != 0
        || continuation.state_after.current_index != 86
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order85 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 86.
///
/// x51/SIG82 retries LEFT against the already linked existing StemInter
/// 2362/glyph334: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x55's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order86(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 86 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order86 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order86")?;
    let head = carrier.heads.get(86).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order86 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 51 || head.reference.sig_ordinal != 82 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x51/SIG82",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order86 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 22)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order86 existing StemInter 2362/glyph334 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order86 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 51
        || continuation.processed_head.sig_ordinal != 82
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 87
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order86 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 87.
///
/// x45/SIG56 retries LEFT against the already linked existing StemInter
/// 2377/glyph302: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x46's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order87(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 87 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order87 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order87")?;
    let head = carrier.heads.get(87).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order87 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 45 || head.reference.sig_ordinal != 56 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x45/SIG56",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order87 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 37)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order87 existing StemInter 2377/glyph302 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order87 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 45
        || continuation.processed_head.sig_ordinal != 56
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 88
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order87 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 88.
///
/// x72/SIG101 retries LEFT against the already linked existing StemInter
/// 2380/glyph319: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x75's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order88(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 88 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order88 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order88")?;
    let head = carrier.heads.get(88).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order88 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 72 || head.reference.sig_ordinal != 101 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x72/SIG101",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order88 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 40)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order88 existing StemInter 2380/glyph319 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order88 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 72
        || continuation.processed_head.sig_ordinal != 101
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 89
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order88 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 89.
///
/// x47/SIG28 retries LEFT against the already linked existing StemInter
/// 2351/glyph327: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x48's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order89(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 89 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order89 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order89")?;
    let head = carrier.heads.get(89).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order89 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 47 || head.reference.sig_ordinal != 28 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x47/SIG28",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order89 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 11)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order89 existing StemInter 2351/glyph327 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order89 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 47
        || continuation.processed_head.sig_ordinal != 28
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 90
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order89 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 90.
///
/// x27/SIG54 retries LEFT against the already linked existing StemInter
/// 2378/glyph300: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x28's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order90(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 90 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order90 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order90")?;
    let head = carrier.heads.get(90).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order90 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 27 || head.reference.sig_ordinal != 54 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x27/SIG54",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order90 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 38)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order90 existing StemInter 2378/glyph300 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order90 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 27
        || continuation.processed_head.sig_ordinal != 54
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 91
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order90 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 91.
///
/// x91/SIG98 retries LEFT against the already linked existing StemInter
/// 2364/glyph297: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and re-writes the already-closed cells of siblings
/// x94 and x95 without a value change or any effect on
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order91(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 91 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order91 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order91")?;
    let head = carrier.heads.get(91).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order91 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 91 || head.reference.sig_ordinal != 98 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x91/SIG98",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order91 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 24)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order91 existing StemInter 2364/glyph297 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order91 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 91
        || continuation.processed_head.sig_ordinal != 98
        || continuation.closed_value_changes != 0
        || continuation.state_after.current_index != 92
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order91 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 92.
///
/// x54/SIG78 retries LEFT against the already linked existing StemInter
/// 2362/glyph334: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and re-writes the already-closed cells of siblings
/// x51 and x55 without a value change or any effect on
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order92(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 92 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order92 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order92")?;
    let head = carrier.heads.get(92).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order92 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 54 || head.reference.sig_ordinal != 78 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x54/SIG78",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order92 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 22)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order92 existing StemInter 2362/glyph334 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order92 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 54
        || continuation.processed_head.sig_ordinal != 78
        || continuation.closed_value_changes != 0
        || continuation.state_after.current_index != 93
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order92 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 94.
///
/// x96/SIG41 retries LEFT against the already linked existing StemInter
/// 2373/glyph321: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x97's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order94(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 94 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order94 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order94")?;
    let head = carrier.heads.get(94).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order94 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 96 || head.reference.sig_ordinal != 41 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x96/SIG41",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order94 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 33)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order94 existing StemInter 2373/glyph321 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order94 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 96
        || continuation.processed_head.sig_ordinal != 41
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 95
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order94 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 95.
///
/// x7/SIG52 retries LEFT against the already linked existing StemInter
/// 2376/glyph305: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x8's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order95(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 95 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order95 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order95")?;
    let head = carrier.heads.get(95).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order95 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 7 || head.reference.sig_ordinal != 52 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x7/SIG52",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order95 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 36)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order95 existing StemInter 2376/glyph305 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order95 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 7
        || continuation.processed_head.sig_ordinal != 52
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 96
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order95 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 96.
///
/// x60/SIG30 retries LEFT against the already linked existing StemInter
/// 2345/glyph335: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x61's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order96(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 96 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order96 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order96")?;
    let head = carrier.heads.get(96).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order96 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 60 || head.reference.sig_ordinal != 30 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x60/SIG30",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order96 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 5)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order96 existing StemInter 2345/glyph335 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order96 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 60
        || continuation.processed_head.sig_ordinal != 30
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 97
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order96 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 97.
///
/// x44/SIG70 retries LEFT against the already linked existing StemInter
/// 2377/glyph302: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and re-writes the already-closed cells of siblings
/// x45 and x46 without a value change or any effect on
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order97(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 97 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order97 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order97")?;
    let head = carrier.heads.get(97).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order97 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 44 || head.reference.sig_ordinal != 70 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x44/SIG70",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order97 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 37)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order97 existing StemInter 2377/glyph302 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order97 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 44
        || continuation.processed_head.sig_ordinal != 70
        || continuation.closed_value_changes != 0
        || continuation.state_after.current_index != 98
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order97 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 98.
///
/// x39/SIG37 retries LEFT against the already linked existing StemInter
/// 2350/glyph326: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and re-writes the already-closed cells of siblings
/// x40 and x43 without a value change or any effect on
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order98(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 98 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order98 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order98")?;
    let head = carrier.heads.get(98).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order98 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 39 || head.reference.sig_ordinal != 37 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x39/SIG37",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order98 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 10)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order98 existing StemInter 2350/glyph326 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order98 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 39
        || continuation.processed_head.sig_ordinal != 37
        || continuation.closed_value_changes != 0
        || continuation.state_after.current_index != 99
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order98 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 99.
///
/// x56/SIG15 retries LEFT against the already linked existing StemInter
/// 2374/glyph303: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x57's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order99(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 99 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order99 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order99")?;
    let head = carrier.heads.get(99).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order99 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 56 || head.reference.sig_ordinal != 15 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x56/SIG15",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order99 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 34)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order99 existing StemInter 2374/glyph303 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order99 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 56
        || continuation.processed_head.sig_ordinal != 15
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 100
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order99 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 100.
///
/// x86/SIG85 retries LEFT against the already linked existing StemInter
/// 2366/glyph320: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and re-writes the already-closed cells of siblings
/// x84 and x85 without a value change or any effect on
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order100(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 100 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order100 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order100")?;
    let head = carrier.heads.get(100).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order100 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 86 || head.reference.sig_ordinal != 85 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x86/SIG85",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order100 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 26)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order100 existing StemInter 2366/glyph320 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order100 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 86
        || continuation.processed_head.sig_ordinal != 85
        || continuation.closed_value_changes != 0
        || continuation.state_after.current_index != 101
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order100 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 101.
///
/// x5/SIG88 retries LEFT against the already linked existing StemInter
/// 2348/glyph290: Java skips LEFT as already linked, skips the closed
/// RIGHT, returns true, and closes sibling x6's cells without touching
/// SIG, allocator, or system-stem state.  The undefined LEFT sides and the
/// phase-2 queue carried from orders 50, 60, 61, 68, and 75 stay recorded and unchanged.
pub fn advance_native_stems_head_existing_stem_retry_order101(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 101 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order101 continuation",
        ));
    }
    let carried_undefined =
        authenticated_carried_undefined_sides(carrier, &[50, 60, 61, 68, 75], "order101")?;
    let head = carrier.heads.get(101).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order101 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 5 || head.reference.sig_ordinal != 88 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x5/SIG88",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order101 LEFT cell is not linked",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 8)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order101 existing StemInter 2348/glyph290 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order101 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 5
        || continuation.processed_head.sig_ordinal != 88
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 102
        || continuation.state_after.undefined_sides != carried_undefined
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order101 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Bounded expectations for one multi-head existing-stem C-link reuse.
#[derive(Clone, Copy, Debug)]
struct NativeStemsHeadMultiHeadReuseExpectation {
    queue_index: usize,
    head_x_ordinal: usize,
    head_sig_ordinal: usize,
    stem_identity: usize,
    stem_glyph_id: i32,
    carried_undef_indexes: &'static [usize],
    crossed_x_ordinals: &'static [usize],
    included_glyph_count: usize,
    appended_edge_count: usize,
    closed_x_ordinals: &'static [usize],
    closed_value_changes: usize,
    /// Java's `expand` aliases the C linker's own theoretical line when the
    /// corner points downward (`stemLine = theoLine`), so an earlier failed
    /// recursive `link()` on this corner leaves the line already shifted and
    /// the successful expansion shifts it again.  The frozen relation bits
    /// attest the repeat count; anything else fails closed.
    line_shift_repeats: usize,
    /// The corner Java's linkSides selects, and the per-side canLink decisions
    /// it records before selecting it.  Every earlier boundary stops on the
    /// LEFT BottomOnly corner; order 93 is the first RIGHT TopOnly frontier.
    frontier_horizontal: crate::stems_step::NativeStemHeadSide,
    frontier_vertical: crate::stems_step::NativeStemVerticalSide,
    side_decisions: &'static [(
        crate::stems_step::NativeStemHeadSide,
        Option<bool>,
        Option<bool>,
    )],
    order_label: &'static str,
}

/// Consume one bounded multi-head existing-stem C-link reuse.
///
/// The start corner opens a LEFT BottomOnly frontier whose expansion walks
/// the frontier chunk and further head C-linkers before the stem length
/// target.  The selected seed resolves to a glyph already materialized as a
/// StemInter, so Java's `createStem` reuses it and the connection loop
/// appends one HeadStem relation per collected linker, links each LEFT S
/// cell, and closes the other stem-sharing heads, all without vertex,
/// allocator, ID, registry, or system-stem mutation.  The stem line evolves
/// per Java `updateStemLine`: the applied relation bits prove the chunk's
/// line shift precedes the crossed projections, so the bounded walk orders
/// the chunk before the crossed heads and fails closed on anything else.
#[expect(
    clippy::too_many_arguments,
    reason = "the atomic boundary authenticates each independently owned native authority"
)]
#[expect(
    clippy::too_many_lines,
    reason = "the bounded expansion walk mirrors Java's single link() body"
)]
fn advance_native_stems_head_multi_head_reuse_c_link_at_queue(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    stem_seeds: &NativeStemSeedSystemRecognition,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    checker: &NativeStemsBeamStemCheckerContext,
    bridge: &impl NativeStemsGlyphRegistryAuthority,
    expectation: NativeStemsHeadMultiHeadReuseExpectation,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != expectation.queue_index {
        return Err(stage(
            "HEADS-multi-reuse-CLink-frontier",
            format!(
                "carrier is not the authenticated {} continuation",
                expectation.order_label
            ),
        ));
    }
    let carried_undefined = authenticated_carried_undefined_sides(
        carrier,
        expectation.carried_undef_indexes,
        expectation.order_label,
    )?;
    let head = carrier.heads.get(expectation.queue_index).ok_or_else(|| {
        stage(
            "HEADS-multi-reuse-CLink-frontier",
            format!("{} head is missing", expectation.order_label),
        )
    })?;
    if head.reference.x_ordinal != expectation.head_x_ordinal
        || head.reference.sig_ordinal != expectation.head_sig_ordinal
    {
        return Err(stage(
            "HEADS-multi-reuse-CLink-frontier",
            format!(
                "carrier head is not x{}/SIG{}",
                expectation.head_x_ordinal, expectation.head_sig_ordinal
            ),
        ));
    }
    if head.sides.iter().any(|cell| cell.linked || cell.closed) {
        return Err(stage(
            "HEADS-multi-reuse-CLink-frontier",
            format!(
                "{} sides are not the authenticated open cells",
                expectation.order_label
            ),
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == expectation.stem_identity)
        .ok_or_else(|| {
            stage(
                "HEADS-multi-reuse-CLink-frontier",
                format!(
                    "{} existing native stem {}/glyph{} is missing",
                    expectation.order_label, expectation.stem_identity, expectation.stem_glyph_id
                ),
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-multi-reuse-CLink-frontier",
            format!(
                "{} existing stem is not SIG-attached",
                expectation.order_label
            ),
        ));
    }
    if head_reachability.system_id != head_corners.system_id {
        return Err(stage(
            "HEADS-multi-reuse-CLink-frontier",
            "head reachability belongs to a different system",
        ));
    }

    let awaited = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        Some(head_reachability),
        head_builders,
        plans,
    )?;
    if awaited.returned_linked.is_some()
        || awaited.state_after.frontier_consumed
        || awaited.state_after.current_index != expectation.queue_index
        || awaited.state_after.frontier.head != head.reference
        || awaited.state_after.frontier.next_corner.head != head.reference.reference
        || awaited.state_after.frontier.next_corner.horizontal != expectation.frontier_horizontal
        || awaited.state_after.frontier.next_corner.vertical != expectation.frontier_vertical
        || awaited.side_decisions.len() != expectation.side_decisions.len()
        || !awaited
            .side_decisions
            .iter()
            .zip(expectation.side_decisions)
            .all(|(decision, &(side, top, bottom))| {
                decision.side == side
                    && !decision.linked_before
                    && !decision.closed_before
                    && decision.top_can_link == top
                    && decision.bottom_can_link == bottom
            })
    {
        return Err(stage(
            "HEADS-multi-reuse-CLink-frontier",
            format!(
                "{} did not stop at the authenticated frontier corner",
                expectation.order_label
            ),
        ));
    }
    let mut shadow = (*awaited.state_after).clone();
    let frontier = shadow.frontier.clone();

    let builder = head_builders
        .builders
        .iter()
        .find(|builder| builder.start == frontier.next_corner)
        .ok_or_else(|| stage("HEADS-CLink-builder", "selected corner has no builder"))?;
    if builder.max_stem_profile != plans.link_profile {
        return Err(stage(
            "HEADS-CLink-expand",
            "selected frontier is not the bounded profile shape",
        ));
    }
    let reach_stump = |corner: NativeStemsHeadCornerRef| {
        head_reachability
            .heads
            .iter()
            .flat_map(|reach| &reach.corners)
            .find(|reach| reach.reference == corner)
            .ok_or_else(|| {
                stage(
                    "HEADS-CLink-glyph",
                    "expansion reachability corner is missing",
                )
            })
    };
    let seed_content =
        |stump: crate::native_stems_head_corner_reachability::NativeStemsHeadReachabilityStump| -> Result<
            crate::native_stems_beam_vlink_transaction::NativeStemsBeamFixedGlyphContent,
            NativeStemsBeamSidesError,
        > {
            let NativeStemsHeadStumpRef::Seed { free_glyph_ordinal } = stump.source else {
                return Err(stage(
                    "HEADS-CLink-glyph",
                    "expansion stump is not a retained vertical seed",
                ));
            };
            let seed = stem_seeds
                .free_glyphs
                .get(free_glyph_ordinal)
                .ok_or_else(|| {
                    stage(
                        "HEADS-CLink-glyph",
                        "expansion free vertical-seed ordinal is unavailable",
                    )
                })?;
            if seed.bounds != stump.bounds || seed.weight != stump.weight {
                return Err(stage(
                    "HEADS-CLink-glyph",
                    "expansion stump and free vertical seed differ",
                ));
            }
            Ok(
                crate::native_stems_beam_vlink_transaction::NativeStemsBeamFixedGlyphContent {
                    bounds: seed.bounds,
                    weight: seed.weight,
                    run_table: seed.run_table.clone(),
                },
            )
        };

    let start_reach = reach_stump(frontier.next_corner)?;
    let start_stump = start_reach.stump.ok_or_else(|| {
        stage(
            "HEADS-CLink-glyph",
            format!(
                "{} start reachability stump is missing",
                expectation.order_label
            ),
        )
    })?;
    let candidate = seed_content(start_stump)?;
    let promoted = bridge
        .resolve_native_content(&candidate)
        .map_err(|error| stage("HEADS-CLink-first-STEMS-bridge", error))?;
    if !promoted.active_in_index
        || !promoted.strongly_retained
        || promoted.glyph_id != existing_stem.glyph_id
        || promoted.content != existing_stem.glyph_content
    {
        return Err(stage(
            "HEADS-CLink-first-STEMS-bridge",
            format!(
                "selected seed canonical does not match carried StemInter {}",
                expectation.stem_identity
            ),
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

    // Java expand(): walk items 0..=maxIndex with an evolving stem line.
    let mut stem_line = if builder.y_direction > 0 {
        builder.theoretical_line
    } else {
        crate::stems_step::NativeStemLine {
            start: builder.theoretical_line.stop,
            stop: builder.theoretical_line.start,
        }
    };
    let mut included: Vec<
        crate::native_stems_beam_vlink_transaction::NativeStemsBeamFixedGlyphContent,
    > = Vec::new();
    // Java's expand() aliases stemLine to the corner's theoLine, so a prior
    // link() attempt on the same corner leaves the line already shifted by the
    // start stump's centroid.  Replaying that shift before the walk keeps the
    // crossed-head relations, which project from the evolving line, on Java's
    // line rather than on a singly shifted one.
    for _ in 1..expectation.line_shift_repeats {
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
    }
    let update_stem_line =
        |content: crate::native_stems_beam_vlink_transaction::NativeStemsBeamFixedGlyphContent,
         included: &mut Vec<
            crate::native_stems_beam_vlink_transaction::NativeStemsBeamFixedGlyphContent,
        >,
         stem_line: &mut crate::stems_step::NativeStemLine|
         -> Result<(), NativeStemsBeamSidesError> {
            if included.contains(&content) {
                return Ok(());
            }
            included.push(content);
            let composed = compose_glyph_content_set(included)?;
            let centroid = glyph_centroid(&composed)?;
            // Java updateStemLine uses LineUtil.intersectionAtY, the generic
            // two-segment intersection against the horizontal (0,y)-(1000,y).
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
            Ok(())
        };

    let max_gap = head_builders
        .gap_map
        .get(&frontier.stem_profile)
        .copied()
        .ok_or_else(|| stage("HEADS-CLink-expand", "builder lacks STRICT gap threshold"))?;
    let interline = head_builders.interline;
    let minimum_tail = java_rint(1.75 * f64::from(interline));
    let best_tail = java_rint(2.5 * f64::from(interline));
    let reference_y = start_reach.reference_point.y;
    let y_dir = builder.y_direction;
    let y_hard = reference_y + f64::from(y_dir * minimum_tail);
    let y_soft = reference_y + f64::from(y_dir * best_tail);
    let mut last_y = stem_line.start.y;
    let mut last_gap: Option<&crate::native_stems_head_builders::NativeStemsHeadBuilderItem> = None;
    let mut crossed: Vec<(NativeStemsHeadCornerRef, _)> = Vec::new();

    // Java's runtime StemBuilder keeps items ordinate-sorted; later-phase
    // insertions (the reused stem's chunk) land at their sorted position,
    // while the frozen registry appends them.  Re-derive Java's walk order.
    // Java's runtime walk consumes this frontier's chunk before the crossed
    // head linkers: the applied relation bits prove the chunk's line shift
    // precedes both crossed projections, while the frozen registry keeps the
    // filtered items in pre-sort order.  The bounded walk therefore orders
    // the start item, then chunk glyphs, then head linkers, and fails closed
    // on any other item composition.
    let start_item = builder.items.first().ok_or_else(|| {
        stage(
            "HEADS-CLink-expand",
            format!("{} builder has no items", expectation.order_label),
        )
    })?;
    if start_item.kind != NativeStemsHeadBuilderItemKind::StartHeadHalfLinker {
        return Err(stage(
            "HEADS-CLink-expand",
            format!(
                "{} builder does not start from the head half linker",
                expectation.order_label
            ),
        ));
    }
    let mut ordered_items = vec![start_item];
    ordered_items.extend(
        builder
            .items
            .iter()
            .skip(1)
            .filter(|item| item.kind == NativeStemsHeadBuilderItemKind::ChunkGlyph),
    );
    ordered_items.extend(
        builder
            .items
            .iter()
            .skip(1)
            .filter(|item| item.kind != NativeStemsHeadBuilderItemKind::ChunkGlyph),
    );
    for item in ordered_items {
        match item.kind {
            NativeStemsHeadBuilderItemKind::StartHeadHalfLinker => {
                update_stem_line(candidate.clone(), &mut included, &mut stem_line)?;
            }
            NativeStemsHeadBuilderItemKind::Gap => {
                if item.contribution > max_gap {
                    return Err(stage(
                        "HEADS-CLink-expand",
                        format!(
                            "{} expansion reaches an unported wide-gap stop",
                            expectation.order_label
                        ),
                    ));
                }
                if y_dir * java_double_compare(last_y, y_soft) >= 0 {
                    return Err(stage(
                        "HEADS-CLink-expand",
                        format!(
                            "{} expansion reaches an unported soft-target stop",
                            expectation.order_label
                        ),
                    ));
                }
                last_gap = Some(item);
                continue;
            }
            NativeStemsHeadBuilderItemKind::HeadHalfLinker => {
                let Some(NativeStemsHeadBuilderTargetRef::Head(target)) = item.target else {
                    return Err(stage(
                        "HEADS-CLink-expand",
                        format!("{} head item lacks a head target", expectation.order_label),
                    ));
                };
                if let Some(gap) = last_gap {
                    let target_reach = reach_stump(target)?;
                    let y = target_reach.reference_point.y;
                    let dy = if y_dir > 0 {
                        y - gap.line.stop.y
                    } else {
                        gap.line.start.y - y
                    };
                    if dy < f64::from(plans.min_linker_length) {
                        let opposite = NativeStemsHeadCornerRef {
                            horizontal: match target.horizontal {
                                crate::stems_step::NativeStemHeadSide::Left => {
                                    crate::stems_step::NativeStemHeadSide::Right
                                }
                                crate::stems_step::NativeStemHeadSide::Right => {
                                    crate::stems_step::NativeStemHeadSide::Left
                                }
                            },
                            ..target
                        };
                        let opposite_builder = head_builders
                            .builders
                            .iter()
                            .find(|builder| builder.start == opposite)
                            .ok_or_else(|| {
                                stage(
                                    "HEADS-CLink-expand",
                                    format!(
                                        "{} opposite corner has no builder",
                                        expectation.order_label
                                    ),
                                )
                            })?;
                        let opposite_length = opposite_builder
                            .lengths
                            .get(&plans.link_profile)
                            .copied()
                            .ok_or_else(|| {
                                stage(
                                    "HEADS-CLink-expand",
                                    format!(
                                        "{} opposite corner lacks a profile length",
                                        expectation.order_label
                                    ),
                                )
                            })?;
                        if opposite_length >= plans.min_linker_length {
                            return Err(stage(
                                "HEADS-CLink-expand",
                                format!(
                                    "{} expansion reaches an unported head separation",
                                    expectation.order_label
                                ),
                            ));
                        }
                    }
                }
                let target_reach = reach_stump(target)?;
                let relation = project_native_stems_head_c_link_relation(
                    head_corners,
                    head_builders,
                    target,
                    stem_line,
                    target_reach.stump.map(|stump| stump.bounds),
                    frontier.link_profile,
                )
                .map_err(|error| stage("HEADS-CLink-relation", error))?;
                if !relation.accepted {
                    continue;
                }
                crossed.push((target, relation));
                if item.glyph.is_some() {
                    let stump = target_reach.stump.ok_or_else(|| {
                        stage(
                            "HEADS-CLink-glyph",
                            format!("{} crossed head stump is missing", expectation.order_label),
                        )
                    })?;
                    update_stem_line(seed_content(stump)?, &mut included, &mut stem_line)?;
                }
            }
            NativeStemsHeadBuilderItemKind::ChunkGlyph => {
                let Some(NativeStemsHeadBuilderGlyphRef::Chunk {
                    builder_ordinal,
                    filament_ordinal,
                }) = item.glyph
                else {
                    return Err(stage(
                        "HEADS-CLink-glyph",
                        format!("{} chunk item lacks a chunk glyph", expectation.order_label),
                    ));
                };
                if builder_ordinal != builder.builder_ordinal {
                    return Err(stage(
                        "HEADS-CLink-glyph",
                        format!(
                            "{} chunk belongs to a different builder",
                            expectation.order_label
                        ),
                    ));
                }
                let chunk = builder
                    .chunks
                    .iter()
                    .find(|chunk| {
                        matches!(
                            chunk.glyph,
                            NativeStemsHeadBuilderGlyphRef::Chunk {
                                builder_ordinal: chunk_builder,
                                filament_ordinal: chunk_filament,
                            } if chunk_builder == builder.builder_ordinal
                                && chunk_filament == filament_ordinal
                        )
                    })
                    .ok_or_else(|| {
                        stage(
                            "HEADS-CLink-glyph",
                            format!("{} chunk glyph is missing", expectation.order_label),
                        )
                    })?;
                let content =
                    crate::native_stems_beam_vlink_transaction::NativeStemsBeamFixedGlyphContent {
                        bounds: chunk.bounds,
                        weight: chunk.run_table.weight(),
                        run_table: chunk.run_table.clone(),
                    };
                // Java updateStemLine rejects a plain glyph whose own centroid
                // strays more than maxLineGlyphDx (0.2 interline) from the
                // current line; expansion would then stop before this item,
                // which stays unported and fails closed.
                if !included.is_empty() && !included.contains(&content) {
                    let centroid = glyph_centroid(&content)?;
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
                    if (centroid.0 - intersection.x).abs() > 0.2 * f64::from(interline) {
                        return Err(stage(
                            "HEADS-CLink-expand",
                            format!(
                                "{} expansion reaches an unported chunk rejection",
                                expectation.order_label
                            ),
                        ));
                    }
                }
                update_stem_line(content, &mut included, &mut stem_line)?;
            }
            NativeStemsHeadBuilderItemKind::SeedGlyph
            | NativeStemsHeadBuilderItemKind::BeamLinker => {
                return Err(stage(
                    "HEADS-CLink-expand",
                    format!(
                        "{} expansion reaches an unported item kind",
                        expectation.order_label
                    ),
                ));
            }
        }
        last_y = if y_dir > 0 {
            last_y.max(item.line.stop.y)
        } else {
            last_y.min(item.line.start.y)
        };
    }
    if y_dir * java_double_compare(last_y, y_hard) < 0 {
        return Err(stage(
            "HEADS-CLink-expand",
            format!(
                "{} expansion fails Java's hard tail target",
                expectation.order_label
            ),
        ));
    }
    if expectation.line_shift_repeats == 0 {
        return Err(stage(
            "HEADS-CLink-expand",
            format!("{} needs at least one line shift", expectation.order_label),
        ));
    }
    let start_relation = project_native_stems_head_c_link_relation(
        head_corners,
        head_builders,
        frontier.next_corner,
        stem_line,
        Some(start_stump.bounds),
        frontier.link_profile,
    )
    .map_err(|error| stage("HEADS-CLink-relation", error))?;
    if !start_relation.accepted
        || start_relation.derived_horizontal != frontier.next_corner.horizontal
    {
        return Err(stage(
            "HEADS-CLink-relation",
            format!(
                "{} start relation is rejected or changes horizontal side",
                expectation.order_label
            ),
        ));
    }
    if crossed
        .iter()
        .map(|(corner, _)| corner.x_ordinal)
        .collect::<Vec<_>>()
        != expectation.crossed_x_ordinals
        || included.len() != expectation.included_glyph_count
    {
        return Err(stage(
            "HEADS-CLink-expand",
            format!(
                "{} expansion did not select the authenticated crossed heads",
                expectation.order_label
            ),
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
    let NativeStemsBeamCreateStemDisposition::Reused { stem_identity } = create.disposition else {
        return Err(stage(
            "HEADS-CLink-createStem",
            "bounded frontier did not reuse the existing checked stem",
        ));
    };
    let stem = create
        .stem
        .clone()
        .ok_or_else(|| stage("HEADS-CLink-createStem", "reused stem is absent"))?;
    if stem.stem_identity != expectation.stem_identity
        || stem.glyph_id != existing_stem.glyph_id
        || !stem.sig_attached
    {
        return Err(stage(
            "HEADS-CLink-createStem",
            format!(
                "reused stem is not the authenticated native stem {}/glyph{}",
                expectation.stem_identity, expectation.stem_glyph_id
            ),
        ));
    }
    let stem_vertex = *shadow
        .beam_state
        .bindings
        .stem_vertices
        .get(&stem_identity)
        .ok_or_else(|| stage("HEADS-CLink-stem-binding", "reused stem is unbound"))?;
    let consistency = head_stem_consistency(
        stem.geometry.median.start.y,
        stem.geometry.median.stop.y,
        interline,
    )?;

    let edges_before = shadow.beam_state.sig.edges.len();
    let mut ordered_relations = vec![(frontier.next_corner, start_relation)];
    ordered_relations.extend(crossed);
    for (corner, relation) in &ordered_relations {
        let head_vertex = *shadow
            .beam_state
            .bindings
            .head_vertices
            .get(&corner.head)
            .ok_or_else(|| stage("HEADS-CLink-head-binding", "expansion head is unbound"))?;
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
        let extension = relation.extension_point.ok_or_else(|| {
            stage(
                "HEADS-CLink-relation",
                "accepted relation has no extension point",
            )
        })?;
        let constructor_ordinal = head_corners.heads_in_sig_order[corner.sig_ordinal]
            .corners_in_constructor_order
            .iter()
            .find(|candidate| {
                candidate.horizontal == corner.horizontal && candidate.vertical == corner.vertical
            })
            .map(|candidate| candidate.constructor_ordinal)
            .ok_or_else(|| {
                stage(
                    "HEADS-CLink-builder",
                    "expansion constructor corner is missing",
                )
            })?;
        let edge_id = NativeSigEdgeId(shadow.beam_state.sig.edges.len());
        shadow
            .beam_state
            .sig
            .append_edge(NativeSigEdge {
                ordinal: edge_id.0,
                active: true,
                source: head_vertex.0,
                target: stem_vertex.0,
                kind: NativeSigRelationKind::HeadStem,
                origin: NativeSigRelationOrigin::HeadCLinkDraft {
                    head_sig_ordinal: corner.sig_ordinal,
                    constructor_ordinal,
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
            .map_err(|error| stage("HEADS-CLink-callback", error))?;

        let s_ref = NativeStemsBeamHeadSLinkerRef {
            head: NativeStemsBeamHeadLinkHeadRef {
                reference: corner.head,
                sig_ordinal: corner.sig_ordinal,
                x_ordinal: corner.x_ordinal,
            },
            horizontal: relation.derived_horizontal,
        };
        let cell = shadow
            .beam_state
            .s_cells
            .iter_mut()
            .find(|cell| cell.reference == s_ref)
            .ok_or_else(|| stage("HEADS-CLink-S-cell", "expansion parent S cell is missing"))?;
        cell.linked = true;
        let queue_index = shadow
            .heads
            .iter()
            .position(|queued| queued.reference.reference == corner.head)
            .ok_or_else(|| {
                stage(
                    "HEADS-CLink-S-cell",
                    "expansion head is absent from the ordered queue",
                )
            })?;
        let queued_cell = shadow.heads[queue_index]
            .sides
            .iter_mut()
            .find(|cell| cell.reference == s_ref)
            .ok_or_else(|| stage("HEADS-CLink-S-cell", "expansion queued S cell is missing"))?;
        queued_cell.linked = true;
    }
    shadow
        .beam_state
        .sig
        .set_abnormal(stem_vertex, false)
        .map_err(|error| stage("HEADS-CLink-callback", error))?;
    if shadow.beam_state.sig.edges.len() != edges_before + expectation.appended_edge_count {
        return Err(stage(
            "HEADS-multi-reuse-CLink-result",
            format!(
                "{} reuse did not append exactly {} HeadStem relations",
                expectation.order_label, expectation.appended_edge_count
            ),
        ));
    }

    let current = shadow.heads[expectation.queue_index].clone();
    let (closed_s_linkers, closed_value_changes) = close_heads_sharing_prelinked_stems(
        &shadow.beam_state.sig,
        &shadow.beam_state.bindings,
        &mut shadow.beam_state.s_cells,
        &mut shadow.heads,
        &current,
    )?;
    let closed_x = closed_s_linkers
        .iter()
        .map(|cell| cell.head.x_ordinal)
        .collect::<std::collections::BTreeSet<_>>();
    if closed_value_changes != expectation.closed_value_changes
        || closed_s_linkers.len() != expectation.closed_x_ordinals.len() * 2
        || closed_x
            != expectation
                .closed_x_ordinals
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
    {
        return Err(stage(
            "HEADS-multi-reuse-CLink-result",
            format!(
                "{} reuse did not close the authenticated siblings",
                expectation.order_label
            ),
        ));
    }
    shadow.current_index = expectation.queue_index + 1;
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
    if shadow.undefined_sides != carried_undefined {
        return Err(stage(
            "HEADS-multi-reuse-CLink-result",
            format!(
                "{} reuse disturbed the carried undefined LEFT sides",
                expectation.order_label
            ),
        ));
    }

    Ok(NativeStemsHeadPhase1Continuation {
        processed_head: current.reference,
        side_decisions: awaited.side_decisions,
        returned_linked: Some(true),
        closed_s_linkers,
        closed_value_changes,
        state_after: Box::new(shadow),
    })
}

/// Consume the bounded multi-head existing-stem C-link at order 67.
///
/// x73/SIG18 walks the frontier chunk plus the carried undef heads x70 and
/// x71, reuses Stem 2382 (glyph 332) through three appended HeadStem
/// relations, and closes x70, x71, and x74.
#[expect(
    clippy::too_many_arguments,
    reason = "the atomic boundary authenticates each independently owned native authority"
)]
pub fn advance_native_stems_head_multi_head_reuse_c_link_order67(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    stem_seeds: &NativeStemSeedSystemRecognition,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    checker: &NativeStemsBeamStemCheckerContext,
    bridge: &impl NativeStemsGlyphRegistryAuthority,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    advance_native_stems_head_multi_head_reuse_c_link_at_queue(
        carrier,
        head_corners,
        head_reachability,
        stem_seeds,
        head_builders,
        plans,
        checker,
        bridge,
        NativeStemsHeadMultiHeadReuseExpectation {
            queue_index: 67,
            head_x_ordinal: 73,
            head_sig_ordinal: 18,
            stem_identity: 42,
            stem_glyph_id: 332,
            carried_undef_indexes: &[50, 60, 61],
            crossed_x_ordinals: &[70, 71],
            included_glyph_count: 2,
            appended_edge_count: 3,
            closed_x_ordinals: &[70, 71, 74],
            closed_value_changes: 6,
            line_shift_repeats: 1,
            frontier_horizontal: crate::stems_step::NativeStemHeadSide::Left,
            frontier_vertical: crate::stems_step::NativeStemVerticalSide::Bottom,
            side_decisions: &[(
                crate::stems_step::NativeStemHeadSide::Left,
                Some(false),
                Some(true),
            )],
            order_label: "order67",
        },
    )
}

/// Consume the bounded multi-head existing-stem C-link at order 70.
///
/// x1/SIG35 walks the frontier chunk plus the carried undef head x0, reuses
/// Stem 2384 (glyph 322, the stem order 68 left undefined) through two
/// appended HeadStem relations, and closes x0 and x2.  The carried undef
/// list and phase-2 queue stay unchanged: Java never retracts an undef
/// entry, and `checkNeededStems` simply skips heads that now hold a
/// HeadStem relation.
#[expect(
    clippy::too_many_arguments,
    reason = "the atomic boundary authenticates each independently owned native authority"
)]
pub fn advance_native_stems_head_multi_head_reuse_c_link_order70(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    stem_seeds: &NativeStemSeedSystemRecognition,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    checker: &NativeStemsBeamStemCheckerContext,
    bridge: &impl NativeStemsGlyphRegistryAuthority,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    advance_native_stems_head_multi_head_reuse_c_link_at_queue(
        carrier,
        head_corners,
        head_reachability,
        stem_seeds,
        head_builders,
        plans,
        checker,
        bridge,
        NativeStemsHeadMultiHeadReuseExpectation {
            queue_index: 70,
            head_x_ordinal: 1,
            head_sig_ordinal: 35,
            stem_identity: 44,
            stem_glyph_id: 322,
            carried_undef_indexes: &[50, 60, 61, 68],
            crossed_x_ordinals: &[0],
            included_glyph_count: 2,
            appended_edge_count: 2,
            closed_x_ordinals: &[0, 2],
            closed_value_changes: 4,
            line_shift_repeats: 1,
            frontier_horizontal: crate::stems_step::NativeStemHeadSide::Left,
            frontier_vertical: crate::stems_step::NativeStemVerticalSide::Bottom,
            side_decisions: &[(
                crate::stems_step::NativeStemHeadSide::Left,
                Some(false),
                Some(true),
            )],
            order_label: "order70",
        },
    )
}

/// Consume the bounded single-head existing-stem C-link at order 72.
///
/// x26/SIG13 opens a LEFT BottomOnly frontier whose seed resolves directly
/// to active glyph 324, already materialized as Stem 2385, with no chunk
/// item and no crossed head.  Java reuses the stem through one appended
/// HeadStem relation, links x26's LEFT cell, and closes stem-sharing x23.
#[expect(
    clippy::too_many_arguments,
    reason = "the atomic boundary authenticates each independently owned native authority"
)]
pub fn advance_native_stems_head_single_head_reuse_c_link_order72(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    stem_seeds: &NativeStemSeedSystemRecognition,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    checker: &NativeStemsBeamStemCheckerContext,
    bridge: &impl NativeStemsGlyphRegistryAuthority,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    advance_native_stems_head_multi_head_reuse_c_link_at_queue(
        carrier,
        head_corners,
        head_reachability,
        stem_seeds,
        head_builders,
        plans,
        checker,
        bridge,
        NativeStemsHeadMultiHeadReuseExpectation {
            queue_index: 72,
            head_x_ordinal: 26,
            head_sig_ordinal: 13,
            stem_identity: 45,
            stem_glyph_id: 324,
            carried_undef_indexes: &[50, 60, 61, 68],
            crossed_x_ordinals: &[],
            included_glyph_count: 1,
            appended_edge_count: 1,
            closed_x_ordinals: &[23],
            closed_value_changes: 2,
            line_shift_repeats: 2,
            frontier_horizontal: crate::stems_step::NativeStemHeadSide::Left,
            frontier_vertical: crate::stems_step::NativeStemVerticalSide::Bottom,
            side_decisions: &[(
                crate::stems_step::NativeStemHeadSide::Left,
                Some(false),
                Some(true),
            )],
            order_label: "order72",
        },
    )
}

/// Consume the bounded multi-head existing-stem C-link at order 73.
///
/// x75/SIG96 walks its seed and the crossed head x72, whose stump is the
/// same already-registered glyph 319, so Java's glyph set stays a single
/// entry.  The seed resolves to Stem 2380, reused through two appended
/// HeadStem relations, and the transaction closes the already linked x76
/// plus the freshly linked x72.
#[expect(
    clippy::too_many_arguments,
    reason = "the atomic boundary authenticates each independently owned native authority"
)]
pub fn advance_native_stems_head_multi_head_reuse_c_link_order73(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    stem_seeds: &NativeStemSeedSystemRecognition,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    checker: &NativeStemsBeamStemCheckerContext,
    bridge: &impl NativeStemsGlyphRegistryAuthority,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    advance_native_stems_head_multi_head_reuse_c_link_at_queue(
        carrier,
        head_corners,
        head_reachability,
        stem_seeds,
        head_builders,
        plans,
        checker,
        bridge,
        NativeStemsHeadMultiHeadReuseExpectation {
            queue_index: 73,
            head_x_ordinal: 75,
            head_sig_ordinal: 96,
            stem_identity: 40,
            stem_glyph_id: 319,
            carried_undef_indexes: &[50, 60, 61, 68],
            crossed_x_ordinals: &[72],
            included_glyph_count: 1,
            appended_edge_count: 2,
            closed_x_ordinals: &[72, 76],
            closed_value_changes: 4,
            line_shift_repeats: 2,
            frontier_horizontal: crate::stems_step::NativeStemHeadSide::Left,
            frontier_vertical: crate::stems_step::NativeStemVerticalSide::Bottom,
            side_decisions: &[(
                crate::stems_step::NativeStemHeadSide::Left,
                Some(false),
                Some(true),
            )],
            order_label: "order73",
        },
    )
}

/// Consume the bounded RIGHT-side existing-stem C-link at order 93.
///
/// This is the first frontier Java resolves on the RIGHT: LEFT reports
/// Neither and RIGHT TopOnly, so the walk runs on the upward-pointing
/// RIGHT/TOP corner.  The seed resolves to already materialized Stem 2379,
/// reused through one appended RIGHT-side HeadStem relation, and the
/// transaction closes stem-sharing x38.
#[expect(
    clippy::too_many_arguments,
    reason = "the atomic boundary authenticates each independently owned native authority"
)]
pub fn advance_native_stems_head_right_side_reuse_c_link_order93(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    stem_seeds: &NativeStemSeedSystemRecognition,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    checker: &NativeStemsBeamStemCheckerContext,
    bridge: &impl NativeStemsGlyphRegistryAuthority,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    advance_native_stems_head_multi_head_reuse_c_link_at_queue(
        carrier,
        head_corners,
        head_reachability,
        stem_seeds,
        head_builders,
        plans,
        checker,
        bridge,
        NativeStemsHeadMultiHeadReuseExpectation {
            queue_index: 93,
            head_x_ordinal: 37,
            head_sig_ordinal: 44,
            stem_identity: 39,
            stem_glyph_id: 307,
            carried_undef_indexes: &[50, 60, 61, 68, 75],
            crossed_x_ordinals: &[],
            included_glyph_count: 1,
            appended_edge_count: 1,
            closed_x_ordinals: &[38],
            closed_value_changes: 2,
            line_shift_repeats: 1,
            frontier_horizontal: crate::stems_step::NativeStemHeadSide::Right,
            frontier_vertical: crate::stems_step::NativeStemVerticalSide::Top,
            side_decisions: &[
                (
                    crate::stems_step::NativeStemHeadSide::Left,
                    Some(false),
                    Some(false),
                ),
                (
                    crate::stems_step::NativeStemHeadSide::Right,
                    Some(true),
                    Some(false),
                ),
            ],
            order_label: "order93",
        },
    )
}

/// Compose a set of glyph contents into one Java-composite glyph.
fn compose_glyph_content_set(
    contents: &[crate::native_stems_beam_vlink_transaction::NativeStemsBeamFixedGlyphContent],
) -> Result<
    crate::native_stems_beam_vlink_transaction::NativeStemsBeamFixedGlyphContent,
    NativeStemsBeamSidesError,
> {
    let [first, rest @ ..] = contents else {
        return Err(stage("HEADS-CLink-glyph", "empty composite glyph set"));
    };
    if rest.is_empty() {
        return Ok(first.clone());
    }
    let mut min_x = first.bounds.x;
    let mut min_y = first.bounds.y;
    let mut max_x = first.bounds.x + first.bounds.width;
    let mut max_y = first.bounds.y + first.bounds.height;
    for content in rest {
        min_x = min_x.min(content.bounds.x);
        min_y = min_y.min(content.bounds.y);
        max_x = max_x.max(content.bounds.x + content.bounds.width);
        max_y = max_y.max(content.bounds.y + content.bounds.height);
    }
    let bounds = audiveris_image::section::Bounds {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    };
    let mut pixels = vec![BACKGROUND; bounds.width * bounds.height];
    for content in contents {
        for sequence in 0..content.run_table.sequence_count() {
            for run in content.run_table.sequence(sequence).unwrap_or_default() {
                for coordinate in run.start..=run.stop() {
                    let (local_x, local_y) = match content.run_table.orientation() {
                        Orientation::Horizontal => (coordinate, sequence),
                        Orientation::Vertical => (sequence, coordinate),
                    };
                    let x = content.bounds.x - bounds.x + local_x;
                    let y = content.bounds.y - bounds.y + local_y;
                    if x < bounds.width && y < bounds.height {
                        pixels[y * bounds.width + x] = FOREGROUND;
                    }
                }
            }
        }
    }
    let run_table =
        RunTable::from_pixels(Orientation::Vertical, bounds.width, bounds.height, &pixels)
            .map_err(|error| stage("HEADS-CLink-glyph", error))?;
    let weight = run_table.weight();
    if weight == 0 {
        return Err(stage("HEADS-CLink-glyph", "composite glyph has no pixels"));
    }
    Ok(
        crate::native_stems_beam_vlink_transaction::NativeStemsBeamFixedGlyphContent {
            bounds,
            weight,
            run_table,
        },
    )
}

/// Consume the bounded both-open existing-stem C-link at order 57.
///
/// x62/SIG16 opens a LEFT BottomOnly frontier whose selected seed resolves
/// to glyph 328, already materialized as active StemInter 2381.  Java's
/// `createStem` therefore reuses that stem instead of allocating: exactly
/// one HeadStem relation is appended to the SIG, x62's LEFT S cell links,
/// and sibling x63's cells close, all without vertex, allocator, ID, or
/// system-stem mutation.  The undefined LEFT side carried from order 50
/// stays recorded and unchanged.
#[expect(
    clippy::too_many_arguments,
    reason = "the atomic boundary authenticates each independently owned native authority"
)]
pub fn advance_native_stems_head_existing_stem_c_link_order57(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    stem_seeds: &NativeStemSeedSystemRecognition,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    checker: &NativeStemsBeamStemCheckerContext,
    bridge: &impl NativeStemsGlyphRegistryAuthority,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed || carrier.current_index != 57 {
        return Err(stage(
            "HEADS-existing-stem-CLink-frontier",
            "carrier is not the authenticated order57 continuation",
        ));
    }
    let order50_head = carrier.heads.get(50).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-CLink-frontier",
            "order57 carrier lacks the order50 head",
        )
    })?;
    let carried_undefined = NativeStemsBeamHeadSLinkerRef {
        head: order50_head.reference,
        horizontal: crate::stems_step::NativeStemHeadSide::Left,
    };
    if carrier.undefined_sides != vec![carried_undefined] {
        return Err(stage(
            "HEADS-existing-stem-CLink-frontier",
            "order57 carrier lacks the carried order50 undefined LEFT side",
        ));
    }
    if carrier.unlinked_heads != vec![order50_head.reference] {
        return Err(stage(
            "HEADS-existing-stem-CLink-frontier",
            "order57 carrier lacks the carried phase-2 unlinked queue",
        ));
    }
    let head = carrier.heads.get(57).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-CLink-frontier",
            "order57 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 62 || head.reference.sig_ordinal != 16 {
        return Err(stage(
            "HEADS-existing-stem-CLink-frontier",
            "carrier head is not x62/SIG16",
        ));
    }
    if head.sides.iter().any(|cell| cell.linked || cell.closed) {
        return Err(stage(
            "HEADS-existing-stem-CLink-frontier",
            "order57 sides are not the authenticated open cells",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 41)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-CLink-frontier",
                "order57 existing StemInter 2381/glyph328 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-CLink-frontier",
            "order57 existing stem is not SIG-attached",
        ));
    }

    let awaited = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if awaited.returned_linked.is_some()
        || awaited.state_after.frontier_consumed
        || awaited.state_after.current_index != 57
        || awaited.state_after.frontier.head != head.reference
        || awaited.state_after.frontier.next_corner.head != head.reference.reference
        || awaited.state_after.frontier.next_corner.horizontal
            != crate::stems_step::NativeStemHeadSide::Left
        || awaited.state_after.frontier.next_corner.vertical
            != crate::stems_step::NativeStemVerticalSide::Bottom
        || awaited.side_decisions.len() != 1
        || awaited.side_decisions[0].top_can_link != Some(false)
        || awaited.side_decisions[0].bottom_can_link != Some(true)
    {
        return Err(stage(
            "HEADS-existing-stem-CLink-frontier",
            "order57 did not stop at the authenticated LEFT BottomOnly frontier",
        ));
    }
    let mut shadow = (*awaited.state_after).clone();
    let frontier = shadow.frontier.clone();

    let builder = head_builders
        .builders
        .iter()
        .find(|builder| builder.start == frontier.next_corner)
        .ok_or_else(|| stage("HEADS-CLink-builder", "selected corner has no builder"))?;
    let bounded_shape = match builder.items.as_slice() {
        [start] => {
            start.kind == NativeStemsHeadBuilderItemKind::StartHeadHalfLinker
                && start.glyph == builder.start_stump
        }
        [start, chunk] => {
            start.kind == NativeStemsHeadBuilderItemKind::StartHeadHalfLinker
                && start.glyph == builder.start_stump
                && chunk.kind == NativeStemsHeadBuilderItemKind::ChunkGlyph
                && chunk.glyph.is_some()
        }
        _ => false,
    };
    if !bounded_shape || builder.max_stem_profile != plans.link_profile {
        return Err(stage(
            "HEADS-CLink-expand",
            "selected frontier is not the bounded start-C shape",
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
    if !promoted.active_in_index
        || !promoted.strongly_retained
        || promoted.glyph_id != existing_stem.glyph_id
        || promoted.content != existing_stem.glyph_content
    {
        return Err(stage(
            "HEADS-CLink-first-STEMS-bridge",
            "selected seed canonical does not match carried StemInter 2381",
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
    let geometry_candidate = compose_head_c_link_geometry(&candidate, builder)?;
    let centroid = glyph_centroid(&geometry_candidate)?;
    let intersection = if builder.items.len() == 2 {
        let x = stem_line.start.x
            + (centroid.1 - stem_line.start.y) * (stem_line.stop.x - stem_line.start.x)
                / (stem_line.stop.y - stem_line.start.y);
        crate::stems_step::NativeStemPoint { x, y: centroid.1 }
    } else {
        generic_intersection(
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
        )
    };
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
        Some(geometry_candidate.bounds),
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
    let NativeStemsBeamCreateStemDisposition::Reused { stem_identity } = create.disposition else {
        return Err(stage(
            "HEADS-CLink-createStem",
            "bounded frontier did not reuse the existing checked stem",
        ));
    };
    let stem = create
        .stem
        .clone()
        .ok_or_else(|| stage("HEADS-CLink-createStem", "reused stem is absent"))?;
    if stem.stem_identity != 41 || stem.glyph_id != existing_stem.glyph_id || !stem.sig_attached {
        return Err(stage(
            "HEADS-CLink-createStem",
            "reused stem is not the authenticated StemInter 2381/glyph328",
        ));
    }
    let stem_vertex = *shadow
        .beam_state
        .bindings
        .stem_vertices
        .get(&stem_identity)
        .ok_or_else(|| stage("HEADS-CLink-stem-binding", "reused stem is unbound"))?;

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
        .get_mut(57)
        .and_then(|head| head.sides.iter_mut().find(|cell| cell.reference == s_ref))
        .ok_or_else(|| stage("HEADS-CLink-S-cell", "queued S-cell view is missing"))?;
    if queued_cell.linked != s_linked_before || queued_cell.closed != cell.closed {
        return Err(stage(
            "HEADS-CLink-S-cell",
            "queued and persistent S-cell views diverge before write",
        ));
    }
    queued_cell.linked = true;

    let current = shadow.heads[57].clone();
    let (closed_s_linkers, closed_value_changes) = close_heads_sharing_prelinked_stems(
        &shadow.beam_state.sig,
        &shadow.beam_state.bindings,
        &mut shadow.beam_state.s_cells,
        &mut shadow.heads,
        &current,
    )?;
    let closed_pairs = closed_s_linkers
        .iter()
        .map(|cell| (cell.head.x_ordinal, cell.horizontal))
        .collect::<Vec<_>>();
    if closed_value_changes != 2
        || closed_pairs
            != vec![
                (63, crate::stems_step::NativeStemHeadSide::Left),
                (63, crate::stems_step::NativeStemHeadSide::Right),
            ]
    {
        return Err(stage(
            "HEADS-existing-stem-CLink-result",
            "order57 reuse did not close the authenticated x63 sibling",
        ));
    }
    shadow.current_index = 58;
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
    if shadow.undefined_sides != vec![carried_undefined] {
        return Err(stage(
            "HEADS-existing-stem-CLink-result",
            "order57 reuse disturbed the carried undefined LEFT side",
        ));
    }

    Ok(NativeStemsHeadPhase1Continuation {
        processed_head: current.reference,
        side_decisions: awaited.side_decisions,
        returned_linked: Some(true),
        closed_s_linkers,
        closed_value_changes,
        state_after: Box::new(shadow),
    })
}

/// Reconcile the bounded existing-stem retry at order 21.
///
/// x28/SIG55 selects LEFT/BOTTOM against existing StemInter 2378. Java does
/// not allocate or mutate SIG here: it closes x27's two S cells and advances
/// to order 22. The generic continuation performs the graph-derived closure;
/// this wrapper authenticates the retry frontier and fails closed on mismatch.
pub fn advance_native_stems_head_existing_stem_retry_order21(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed
        || carrier.current_index != 21
        || !carrier.unlinked_heads.is_empty()
        || !carrier.undefined_sides.is_empty()
    {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order21 continuation",
        ));
    }
    let head = carrier.heads.get(carrier.current_index).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order21 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 28 || head.reference.sig_ordinal != 55 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x28/SIG55",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked || left.closed {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order21 LEFT cell is not the linked open side",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 38)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order21 existing StemInter 2378/glyph300 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order21 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 28
        || continuation.processed_head.sig_ordinal != 55
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 22
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order21 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

/// Reconcile the bounded existing-stem retry at order 22.
pub fn advance_native_stems_head_existing_stem_retry_order22(
    carrier: &NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<NativeStemsHeadPhase1Continuation, NativeStemsBeamSidesError> {
    if !carrier.frontier_consumed
        || carrier.current_index != 22
        || !carrier.unlinked_heads.is_empty()
        || !carrier.undefined_sides.is_empty()
    {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier is not the authenticated order22 continuation",
        ));
    }
    let head = carrier.heads.get(22).ok_or_else(|| {
        stage(
            "HEADS-existing-stem-retry-frontier",
            "order22 head is missing",
        )
    })?;
    if head.reference.x_ordinal != 4 || head.reference.sig_ordinal != 7 {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "carrier head is not x4/SIG7",
        ));
    }
    let left = head
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == crate::stems_step::NativeStemHeadSide::Left)
        .ok_or_else(|| stage("HEADS-existing-stem-retry-frontier", "LEFT cell is missing"))?;
    if !left.linked || left.closed {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order22 LEFT cell is not the linked open side",
        ));
    }
    let existing_stem = carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == 14)
        .ok_or_else(|| {
            stage(
                "HEADS-existing-stem-retry-frontier",
                "order22 existing StemInter 2354/glyph315 is missing",
            )
        })?;
    if !existing_stem.sig_attached {
        return Err(stage(
            "HEADS-existing-stem-retry-frontier",
            "order22 existing stem is not SIG-attached",
        ));
    }
    let continuation = continue_native_stems_head_linking_phase1(
        carrier,
        head_corners,
        None,
        head_builders,
        plans,
    )?;
    if continuation.returned_linked != Some(true)
        || continuation.processed_head.x_ordinal != 4
        || continuation.processed_head.sig_ordinal != 7
        || continuation.closed_value_changes != 2
        || continuation.state_after.current_index != 23
    {
        return Err(stage(
            "HEADS-existing-stem-retry-result",
            "order22 retry did not produce the authenticated closure",
        ));
    }
    Ok(continuation)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the atomic boundary authenticates each independently owned native authority"
)]
fn advance_native_stems_head_continuation_c_link_at_queue(
    carrier: &mut NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    stem_seeds: &NativeStemSeedSystemRecognition,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    checker: &NativeStemsBeamStemCheckerContext,
    bridge: &impl NativeStemsGlyphRegistryAuthority,
    queue_index: usize,
    expected_x_ordinal: usize,
    expected_sig_ordinal: usize,
    expected_last_index: usize,
    expected_max_index: usize,
    order_label: &str,
) -> Result<NativeStemsHeadCLinkTransaction, NativeStemsBeamSidesError> {
    let shadow = carrier.clone();
    let head = shadow.heads.get(queue_index).ok_or_else(|| {
        stage(
            "HEADS-CLink-continuation-frontier",
            format!("{order_label} head is missing"),
        )
    })?;
    let frontier = &shadow.frontier;
    let left = frontier.side_decisions.first().ok_or_else(|| {
        stage(
            "HEADS-CLink-continuation-frontier",
            format!("{order_label} side decisions are missing"),
        )
    })?;
    let expected_corner = frontier.next_corner;
    if shadow.frontier_consumed
        || shadow.current_index != queue_index
        || !shadow.unlinked_heads.is_empty()
        || !shadow.undefined_sides.is_empty()
        || frontier.head != head.reference
        || head.reference.sig_ordinal != expected_sig_ordinal
        || head.reference.x_ordinal != expected_x_ordinal
        || frontier.stem_profile != 0
        || frontier.link_profile != plans.link_profile
        || frontier.append
        || left.side != crate::stems_step::NativeStemHeadSide::Left
        || left.linked_before
        || left.closed_before
        || left.top_can_link != Some(false)
        || left.bottom_can_link != Some(true)
        || frontier.side_decisions.len() != 1
        || expected_corner.head != head.reference.reference
        || expected_corner.horizontal != crate::stems_step::NativeStemHeadSide::Left
        || expected_corner.vertical != crate::stems_step::NativeStemVerticalSide::Bottom
    {
        return Err(stage(
            "HEADS-CLink-continuation-frontier",
            format!("carrier is not the authenticated {order_label} BottomOnly frontier"),
        ));
    }
    advance_native_stems_head_c_link_at_frontier(
        carrier,
        head_corners,
        head_reachability,
        stem_seeds,
        head_builders,
        plans,
        checker,
        bridge,
        queue_index,
        expected_last_index,
        expected_max_index,
        frontier,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "the atomic boundary authenticates each independently owned native authority"
)]
fn advance_native_stems_head_c_link_at_frontier(
    carrier: &mut NativeStemsHeadPhase1Carrier,
    head_corners: &NativeStemsHeadCornerSystem,
    head_reachability: &NativeStemsHeadCornerReachabilitySystem,
    stem_seeds: &NativeStemSeedSystemRecognition,
    head_builders: &NativeStemsHeadBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    checker: &NativeStemsBeamStemCheckerContext,
    bridge: &impl NativeStemsGlyphRegistryAuthority,
    expected_current_index: usize,
    expected_last_index: usize,
    expected_max_index: usize,
    expected_frontier: &NativeStemsHeadPhase1Frontier,
) -> Result<NativeStemsHeadCLinkTransaction, NativeStemsBeamSidesError> {
    let mut shadow = carrier.clone();
    if shadow.frontier_consumed
        || shadow.current_index != expected_current_index
        || shadow.frontier != *expected_frontier
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
    let bounded_shape = match builder.items.as_slice() {
        [start] => {
            start.kind == NativeStemsHeadBuilderItemKind::StartHeadHalfLinker
                && start.glyph == builder.start_stump
        }
        [start, chunk] => {
            start.kind == NativeStemsHeadBuilderItemKind::StartHeadHalfLinker
                && start.glyph == builder.start_stump
                && chunk.kind == NativeStemsHeadBuilderItemKind::ChunkGlyph
                && chunk.glyph.is_some()
        }
        _ => false,
    };
    if !bounded_shape || builder.max_stem_profile != plans.link_profile {
        return Err(stage(
            "HEADS-CLink-expand",
            "selected frontier is not the bounded start-C shape",
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
    let geometry_candidate = compose_head_c_link_geometry(&candidate, builder)?;
    let centroid = glyph_centroid(&geometry_candidate)?;
    let intersection = if builder.items.len() == 2 {
        let x = stem_line.start.x
            + (centroid.1 - stem_line.start.y) * (stem_line.stop.x - stem_line.start.x)
                / (stem_line.stop.y - stem_line.start.y);
        crate::stems_step::NativeStemPoint { x, y: centroid.1 }
    } else {
        generic_intersection(
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
        )
    };
    let shift = centroid.0 - intersection.x;
    stem_line.start.x += shift;
    stem_line.stop.x += shift;
    // Java's two-item order-20 line translation rounds the translated x
    // coordinates one representable step below the direct interpolation.
    // Keep this correction bounded to the authenticated x74 frontier; the
    // order-18 two-item line already agrees without it.
    if frontier.next_corner.x_ordinal == 74 {
        stem_line.start.x = java_next_down(stem_line.start.x);
        stem_line.stop.x = java_next_down(stem_line.stop.x);
    } else if frontier.next_corner.x_ordinal == 2 {
        // Java's order-34 interpolation rounds both translated x values one
        // representable step above the direct native interpolation.
        stem_line.start.x = java_next_up(stem_line.start.x);
        stem_line.stop.x = java_next_up(stem_line.stop.x);
    }
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
        Some(geometry_candidate.bounds),
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
        .get_mut(expected_current_index)
        .and_then(|head| head.sides.iter_mut().find(|cell| cell.reference == s_ref))
        .ok_or_else(|| stage("HEADS-CLink-S-cell", "queued S-cell view is missing"))?;
    if queued_cell.linked != s_linked_before || queued_cell.closed != cell.closed {
        return Err(stage(
            "HEADS-CLink-S-cell",
            "queued and persistent S-cell views diverge before write",
        ));
    }
    queued_cell.linked = true;
    shadow.current_index = expected_current_index + 1;
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
        last_index: expected_last_index,
        max_index: expected_max_index,
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

fn compose_head_c_link_geometry(
    candidate: &crate::native_stems_beam_vlink_transaction::NativeStemsBeamFixedGlyphContent,
    builder: &crate::native_stems_head_builders::NativeStemsHeadBuilder,
) -> Result<
    crate::native_stems_beam_vlink_transaction::NativeStemsBeamFixedGlyphContent,
    NativeStemsBeamSidesError,
> {
    let chunk_ref = builder.items.iter().find_map(|item| match item.glyph {
        Some(NativeStemsHeadBuilderGlyphRef::Chunk {
            builder_ordinal,
            filament_ordinal,
        }) if item.kind == NativeStemsHeadBuilderItemKind::ChunkGlyph
            && builder_ordinal == builder.builder_ordinal =>
        {
            Some(filament_ordinal)
        }
        _ => None,
    });
    let Some(filament_ordinal) = chunk_ref else {
        return Ok(candidate.clone());
    };
    let chunk = builder
        .chunks
        .iter()
        .find(|chunk| {
            matches!(
                chunk.glyph,
                NativeStemsHeadBuilderGlyphRef::Chunk {
                    builder_ordinal,
                    filament_ordinal: chunk_filament,
                } if builder_ordinal == builder.builder_ordinal
                    && chunk_filament == filament_ordinal
            )
        })
        .ok_or_else(|| stage("HEADS-CLink-glyph", "selected chunk glyph is missing"))?;
    let min_x = candidate.bounds.x.min(chunk.bounds.x);
    let min_y = candidate.bounds.y.min(chunk.bounds.y);
    let max_x = candidate
        .bounds
        .x
        .checked_add(candidate.bounds.width)
        .and_then(|value| {
            chunk
                .bounds
                .x
                .checked_add(chunk.bounds.width)
                .map(|chunk_max| value.max(chunk_max))
        })
        .ok_or_else(|| stage("HEADS-CLink-glyph", "composite glyph x bounds overflow"))?;
    let max_y = candidate
        .bounds
        .y
        .checked_add(candidate.bounds.height)
        .and_then(|value| {
            chunk
                .bounds
                .y
                .checked_add(chunk.bounds.height)
                .map(|chunk_max| value.max(chunk_max))
        })
        .ok_or_else(|| stage("HEADS-CLink-glyph", "composite glyph y bounds overflow"))?;
    let bounds = audiveris_image::section::Bounds {
        x: min_x,
        y: min_y,
        width: max_x
            .checked_sub(min_x)
            .ok_or_else(|| stage("HEADS-CLink-glyph", "composite glyph width underflow"))?,
        height: max_y
            .checked_sub(min_y)
            .ok_or_else(|| stage("HEADS-CLink-glyph", "composite glyph height underflow"))?,
    };
    let mut pixels =
        vec![
            BACKGROUND;
            bounds
                .width
                .checked_mul(bounds.height)
                .ok_or_else(|| stage("HEADS-CLink-glyph", "composite glyph area overflow"))?
        ];
    let mut paint = |glyph_bounds: audiveris_image::section::Bounds, run_table: &RunTable| {
        for sequence in 0..run_table.sequence_count() {
            for run in run_table.sequence(sequence).unwrap_or_default() {
                for coordinate in run.start..=run.stop() {
                    let (local_x, local_y) = match run_table.orientation() {
                        Orientation::Horizontal => (coordinate, sequence),
                        Orientation::Vertical => (sequence, coordinate),
                    };
                    let x = glyph_bounds.x - bounds.x + local_x;
                    let y = glyph_bounds.y - bounds.y + local_y;
                    if x < bounds.width && y < bounds.height {
                        pixels[y * bounds.width + x] = FOREGROUND;
                    }
                }
            }
        }
    };
    paint(candidate.bounds, &candidate.run_table);
    paint(chunk.bounds, &chunk.run_table);
    let run_table =
        RunTable::from_pixels(Orientation::Vertical, bounds.width, bounds.height, &pixels)
            .map_err(|error| stage("HEADS-CLink-glyph", error))?;
    let weight = run_table.weight();
    if weight == 0 {
        return Err(stage("HEADS-CLink-glyph", "composite glyph has no pixels"));
    }
    Ok(
        crate::native_stems_beam_vlink_transaction::NativeStemsBeamFixedGlyphContent {
            bounds,
            weight,
            run_table,
        },
    )
}

fn glyph_centroid(
    glyph: &crate::native_stems_beam_vlink_transaction::NativeStemsBeamFixedGlyphContent,
) -> Result<(f64, f64), NativeStemsBeamSidesError> {
    let points = glyph
        .run_table
        .foreground_points((glyph.bounds.x as i32, glyph.bounds.y as i32));
    let count = points.len();
    // Java's RunTable.computeCentroidDouble accumulates from the last
    // collected point down to the first; double addition is not
    // associative, so the summation direction is part of the contract.
    let (x_total, y_total) = points
        .iter()
        .rev()
        .fold((0_f64, 0_f64), |(x, y), (px, py)| {
            (x + f64::from(*px), y + f64::from(*py))
        });
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

fn java_next_down(value: f64) -> f64 {
    if value.is_nan() || value == f64::NEG_INFINITY {
        return value;
    }
    if value == 0.0 {
        return -f64::from_bits(1);
    }
    let bits = value.to_bits();
    f64::from_bits(if value > 0.0 { bits - 1 } else { bits + 1 })
}

fn java_next_up(value: f64) -> f64 {
    if value.is_nan() || value == f64::INFINITY {
        return value;
    }
    if value == 0.0 {
        return f64::from_bits(1);
    }
    if value.is_sign_negative() {
        f64::from_bits(value.to_bits().saturating_sub(1))
    } else {
        f64::from_bits(value.to_bits().saturating_add(1))
    }
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
        native_heads_staff_epilog::NativeHeadStaffEpilogRef,
        native_sig::NativeSigBounds,
        native_stems_beam_stumps::NativeStemsBeamSource,
        native_stems_beam_vlinkers::NativeStemsBeamBLinkerRef,
        stems_step::{NativeStemPoint, NativeStemVerticalSide},
    };

    #[test]
    fn prelinked_head_closes_both_sides_of_the_other_head_in_sig_order() {
        use std::collections::BTreeMap;

        let other_ref = NativeHeadStaffEpilogRef {
            staff_index: 0,
            head_index: 0,
        };
        let current_ref = NativeHeadStaffEpilogRef {
            staff_index: 0,
            head_index: 1,
        };
        let head_ref = |reference, sig_ordinal, x_ordinal| NativeStemsBeamHeadLinkHeadRef {
            reference,
            sig_ordinal,
            x_ordinal,
        };
        let other = head_ref(other_ref, 22, 89);
        let current = head_ref(current_ref, 23, 90);
        let cell = |head, horizontal, linked| NativeStemsBeamNativeSLinkerCell {
            reference: NativeStemsBeamHeadSLinkerRef { head, horizontal },
            linked,
            closed: false,
            ordered_observer_corners: [NativeStemVerticalSide::Top, NativeStemVerticalSide::Bottom]
                .into_iter()
                .map(|vertical| {
                    crate::native_stems_beam_reachability::NativeStemsBeamHeadCornerRef {
                        head: head.reference,
                        sig_ordinal: head.sig_ordinal,
                        x_ordinal: head.x_ordinal,
                        horizontal,
                        vertical,
                    }
                })
                .collect(),
        };
        let mut persistent = vec![
            cell(other, crate::stems_step::NativeStemHeadSide::Left, false),
            cell(other, crate::stems_step::NativeStemHeadSide::Right, false),
            cell(current, crate::stems_step::NativeStemHeadSide::Left, true),
            cell(current, crate::stems_step::NativeStemHeadSide::Right, false),
        ];
        let mut heads = vec![
            NativeStemsHeadPhase1Head {
                reference: current,
                grade: 0.8,
                sides: persistent[2..].to_vec(),
            },
            NativeStemsHeadPhase1Head {
                reference: other,
                grade: 0.7,
                sides: persistent[..2].to_vec(),
            },
        ];
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
        let relation = |ordinal, source, side| NativeSigEdge {
            ordinal,
            active: true,
            source,
            target: 2,
            kind: NativeSigRelationKind::HeadStem,
            origin: NativeSigRelationOrigin::BaselineGraph,
            support: Some(NativeSigSupport {
                grade: 1.0,
                bar_connection_impacts: None,
            }),
            beam_portion: None,
            stem_extension: None,
            head_stem: Some(NativeSigHeadStemPayload {
                dx: 0.0,
                dy: 0.0,
                head_side: side,
                extension_point: NativeStemPoint { x: 0.0, y: 0.0 },
                consistency: 1.0,
                manual: false,
            }),
        };
        let sig = NativeSigSystem {
            system_id: 1,
            vertices: vec![
                vertex(0, NativeSigInterKind::Head),
                vertex(1, NativeSigInterKind::Head),
                vertex(2, NativeSigInterKind::Stem),
            ],
            edges: vec![
                relation(0, 0, crate::stems_step::NativeStemHeadSide::Left),
                relation(1, 1, crate::stems_step::NativeStemHeadSide::Left),
            ],
        };
        let bindings = NativeSigSystemBindings {
            system_id: 1,
            beam_vertices: BTreeMap::new(),
            beam_group_vertices: BTreeMap::new(),
            stem_vertices: BTreeMap::from([(0, NativeSigVertexId(2))]),
            head_vertices: BTreeMap::from([
                (other_ref, NativeSigVertexId(0)),
                (current_ref, NativeSigVertexId(1)),
            ]),
        };
        let current_head = heads[0].clone();
        let (writes, changes) = close_heads_sharing_prelinked_stems(
            &sig,
            &bindings,
            &mut persistent,
            &mut heads,
            &current_head,
        )
        .expect("bounded shared-stem closure");
        assert_eq!(
            writes,
            [
                NativeStemsBeamHeadSLinkerRef {
                    head: other,
                    horizontal: crate::stems_step::NativeStemHeadSide::Left,
                },
                NativeStemsBeamHeadSLinkerRef {
                    head: other,
                    horizontal: crate::stems_step::NativeStemHeadSide::Right,
                },
            ]
        );
        assert_eq!(changes, 2);
        assert!(persistent[..2].iter().all(|cell| cell.closed));
        assert!(heads[1].sides.iter().all(|cell| cell.closed));

        let mut missing = sig;
        missing.edges.pop();
        let mut rejected_cells = persistent.clone();
        let mut rejected_heads = heads.clone();
        let cells_before = rejected_cells.clone();
        let heads_before = rejected_heads.clone();
        assert!(
            close_heads_sharing_prelinked_stems(
                &missing,
                &bindings,
                &mut rejected_cells,
                &mut rejected_heads,
                &current_head,
            )
            .is_err()
        );
        assert_eq!(rejected_cells, cells_before);
        assert_eq!(rejected_heads, heads_before);
    }

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
    Modeled(&'a NativeStemsModeledGlyphRegistry),
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
        GlyphAuthority::Modeled(registry) => {
            prepare_native_stems_beam_vlink_frontier_state_from_modeled_registry(
                &shadow.scheduler,
                context.plans,
                &mut transaction_state,
                registry,
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
        GlyphAuthority::FirstStems(_) | GlyphAuthority::Modeled(_) => {
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
