// SPDX-License-Identifier: AGPL-3.0-or-later

//! Deterministic, mutation-free prefix of Java's beam STEMS scheduler.
//!
//! The isolated [`crate::native_stems_beam_link_plans`] product tells us what
//! happens before `StemBuilder.createStem` for one beam-origin V linker and
//! profile. This module supplies the missing live scheduler context: canonical
//! beam Glyph aliasing, ordered beam/hook exclusions, stable reverse-width
//! traversal, side/profile selection, and stump order. It advances through
//! outcomes which are already known to return `false`, then stops at the first
//! operation whose result requires persistent Java state. No line delta,
//! GlyphIndex registration, SIG edit, system-stem insertion, or link flag is
//! applied here.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use audiveris_image::{run_table::RunTable, section::Bounds};

use crate::{
    beam_inters::BeamKind,
    native_heads_small_beams::raw_hook_beam_exclusion_key,
    native_stems_beam_builders::{
        NativeStemsBeamBuilderRecognition, NativeStemsBeamBuilderSystem,
        NativeStemsBeamBuilderTargetRef,
    },
    native_stems_beam_link_plans::{
        NativeStemsBeamLinkPlanAttempt, NativeStemsBeamLinkPlanOutcome,
        NativeStemsBeamLinkPlanRecognition, NativeStemsBeamLinkPlanSystem,
    },
    native_stems_beam_stumps::{
        NativeStemsBeamSource, NativeStemsBeamStumpBeam, NativeStemsBeamStumpRecognition,
        NativeStemsBeamStumpRef, NativeStemsBeamStumpSystem,
    },
    native_stems_beam_vlinkers::{
        NativeStemsBeamBLinker, NativeStemsBeamBLinkerRef, NativeStemsBeamVLinkerConstructor,
        NativeStemsBeamVLinkerRecognition, NativeStemsBeamVLinkerRef, NativeStemsBeamVLinkerSystem,
    },
    recognize::NativeBeamRecognition,
    stems_step::{NativeStemHeadSide, NativeStemLine, NativeStemVerticalSide},
};

const BEAM_SEED_PROFILE: i32 = 3;
const BEAM_SIDE_PROFILE: i32 = 4;

/// All independently executable scheduler prefixes, one per system.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSchedulerRecognition {
    pub systems: Vec<NativeStemsBeamSchedulerSystem>,
    pub live_beam_count: usize,
    pub canonical_glyph_class_count: usize,
    pub live_exclusion_count: usize,
    pub prefix_event_count: usize,
    pub invoked_known_false_plan_count: usize,
    pub deferred_line_delta_count: usize,
    pub completed_system_count: usize,
    pub awaiting_v_link_transaction_count: usize,
    pub awaiting_hook_removal_transaction_count: usize,
    /// This boundary describes, but never applies, persistent mutations.
    pub forbidden_mutation_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSchedulerSystem {
    pub system_id: usize,
    pub link_profile: i32,
    /// Live post-tremolo beam/hook vertices in fresh beam-only SIG order.
    pub glyphs_in_sig_order: Vec<NativeStemsBeamCanonicalGlyph>,
    /// Supported live initial raw hook/full exclusions, in their own creation
    /// order. Other SIG exclusion families do not contribute ordinals here.
    pub live_exclusions: Vec<NativeStemsBeamLiveExclusion>,
    /// Stable `Inters.byReverseWidth` order used by both scheduler passes.
    pub beams_by_reverse_width: Vec<NativeStemsBeamScheduledBeam>,
    pub prefix_events: Vec<NativeStemsBeamSchedulerEvent>,
    /// Definite Java line-alias writes made by known-false prefix attempts.
    pub deferred_line_deltas: Vec<NativeStemsBeamDeferredLineDelta>,
    pub status: NativeStemsBeamSchedulerStatus,
}

/// A sheet-global dense alias class for exact Java fixed-Glyph content among
/// live beams. It deliberately does not claim a `GlyphIndex` numeric ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeStemsBeamCanonicalGlyph {
    pub source: NativeStemsBeamSource,
    pub scheduler_sig_ordinal: usize,
    pub pre_tremolo_sig_ordinal: usize,
    pub alias_class: usize,
    pub bounds: Bounds,
    pub run_table: RunTable,
    pub run_digest: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeStemsBeamLiveExclusion {
    /// Sheet-global ordinal among supported initial same-item raw hook/full
    /// exclusions (not among all SIG exclusion edges).
    pub creation_ordinal: usize,
    /// System-local dense ordinal after removed/tremolo endpoints are filtered.
    pub live_ordinal: usize,
    pub hook: NativeStemsBeamSource,
    pub beam: NativeStemsBeamSource,
    pub canonical_glyph_alias: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamScheduledBeam {
    pub width_ordinal: usize,
    pub source: NativeStemsBeamSource,
    pub scheduler_sig_ordinal: usize,
    pub integer_width: i32,
    pub kind: BeamKind,
    pub canonical_glyph_alias: usize,
    pub competing_hook: Option<NativeStemsBeamSource>,
    pub selected_side_stem_profile: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeStemsBeamWorklistSnapshot {
    pub pass: NativeStemsBeamSchedulerPass,
    pub current_index: usize,
    pub sources: Vec<NativeStemsBeamSource>,
    pub current: NativeStemsBeamSource,
    pub remaining: Vec<NativeStemsBeamSource>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamSchedulerPass {
    Sides,
    Stumps,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamPlanRef {
    pub system_id: usize,
    /// Flattened builder/profile ordinal in the isolated plan product.
    pub plan_ordinal: usize,
    pub builder_ordinal: usize,
    pub stem_profile: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamSchedulerEvent {
    BeamPassStart {
        event_ordinal: usize,
        snapshot: NativeStemsBeamWorklistSnapshot,
        kind: BeamKind,
        competing_hook: Option<NativeStemsBeamSource>,
        selected_stem_profile: i32,
    },
    MissingSideBLinker {
        event_ordinal: usize,
        beam: NativeStemsBeamSource,
        side: NativeStemHeadSide,
    },
    EmptyVLinkerSideSuccess {
        event_ordinal: usize,
        beam: NativeStemsBeamSource,
        side: NativeStemHeadSide,
        b_linker: NativeStemsBeamBLinkerRef,
        /// Java returns true without setting `BLinker.linked`.
        linked_flag_after: bool,
    },
    SideVSkippedEmptyTargets {
        event_ordinal: usize,
        beam: NativeStemsBeamSource,
        side: NativeStemHeadSide,
        b_linker: NativeStemsBeamBLinkerRef,
        v_linker: NativeStemsBeamVLinkerRef,
        builder_ordinal: usize,
    },
    InvokedKnownFalsePlan {
        event_ordinal: usize,
        invocation_ordinal: usize,
        pass: NativeStemsBeamSchedulerPass,
        beam: NativeStemsBeamSource,
        horizontal_side: Option<NativeStemHeadSide>,
        b_linker: NativeStemsBeamBLinkerRef,
        v_linker: NativeStemsBeamVLinkerRef,
        plan: NativeStemsBeamPlanRef,
        outcome: NativeStemsBeamLinkPlanOutcome,
        deferred_line_delta_ordinal: Option<usize>,
    },
    SideBLinkerResult {
        event_ordinal: usize,
        beam: NativeStemsBeamSource,
        side: NativeStemHeadSide,
        b_linker: NativeStemsBeamBLinkerRef,
        logical_success: bool,
        linked_flag_after: bool,
    },
    BeamRetainedForStumps {
        event_ordinal: usize,
        beam: NativeStemsBeamSource,
        linked_sides: Vec<NativeStemHeadSide>,
    },
    BeamRemovedFromLocalWorklist {
        event_ordinal: usize,
        beam: NativeStemsBeamSource,
        failed_side: Option<NativeStemHeadSide>,
        worklist_after: Vec<NativeStemsBeamSource>,
    },
    StumpSkippedStructuralSideGlyph {
        event_ordinal: usize,
        beam: NativeStemsBeamSource,
        b_linker: NativeStemsBeamBLinkerRef,
        v_linker: NativeStemsBeamVLinkerRef,
        canonical_stump_alias: usize,
    },
    StumpSkippedAlreadyLinkedB {
        event_ordinal: usize,
        beam: NativeStemsBeamSource,
        b_linker: NativeStemsBeamBLinkerRef,
        v_linker: NativeStemsBeamVLinkerRef,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamDeferredLineDelta {
    pub delta_ordinal: usize,
    pub invocation_ordinal: usize,
    pub plan: NativeStemsBeamPlanRef,
    pub v_linker: NativeStemsBeamVLinkerRef,
    pub before: NativeStemLine,
    pub after: NativeStemLine,
    pub builder_line_aliases: bool,
    pub attachment_aliases: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamSchedulerStatus {
    Completed {
        retained_for_stumps: Vec<NativeStemsBeamSource>,
        final_local_worklist: Vec<NativeStemsBeamSource>,
    },
    AwaitingVLinkTransaction(Box<NativeStemsBeamAwaitingVLinkTransaction>),
    AwaitingHookRemovalTransaction(Box<NativeStemsBeamAwaitingHookRemovalTransaction>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamAwaitingVLinkTransaction {
    pub invocation_ordinal: usize,
    pub snapshot: NativeStemsBeamWorklistSnapshot,
    pub beam: NativeStemsBeamSource,
    pub horizontal_side: Option<NativeStemHeadSide>,
    pub b_linker: NativeStemsBeamBLinkerRef,
    pub v_linker: NativeStemsBeamVLinkerRef,
    pub vertical_side: NativeStemVerticalSide,
    pub plan: NativeStemsBeamPlanRef,
    pub outcome: NativeStemsBeamLinkPlanOutcome,
    pub linked_sides_before: Vec<NativeStemHeadSide>,
    pub retained_beams_before: Vec<NativeStemsBeamSource>,
    pub would_apply_stored_line_delta: Option<NativeStemsBeamDeferredLineDelta>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeStemsBeamAwaitingHookRemovalTransaction {
    pub snapshot: NativeStemsBeamWorklistSnapshot,
    pub beam: NativeStemsBeamSource,
    pub competing_hook: NativeStemsBeamSource,
    pub linked_sides: Vec<NativeStemHeadSide>,
    pub retained_beams_before: Vec<NativeStemsBeamSource>,
}

#[derive(Debug)]
pub enum NativeStemsBeamSchedulerError {
    SystemOrder,
    MissingSystemProduct {
        system_id: usize,
        product: &'static str,
    },
    InvalidBeamSource {
        system_id: usize,
        source: NativeStemsBeamSource,
    },
    InvalidLiveSigOrder {
        system_id: usize,
    },
    InvalidCanonicalGlyph {
        system_id: usize,
        source: NativeStemsBeamSource,
    },
    UnsupportedExclusionTopology {
        system_id: usize,
        source: NativeStemsBeamSource,
    },
    MissingBLinker {
        system_id: usize,
        reference: NativeStemsBeamBLinkerRef,
    },
    MissingBuilder {
        system_id: usize,
        reference: NativeStemsBeamVLinkerRef,
    },
    MissingPlan {
        system_id: usize,
        reference: NativeStemsBeamVLinkerRef,
        stem_profile: i32,
    },
    RepeatedVAttemptAfterLineDelta {
        system_id: usize,
        reference: NativeStemsBeamVLinkerRef,
    },
    InvalidStump {
        system_id: usize,
        reference: NativeStemsBeamBLinkerRef,
    },
}

impl fmt::Display for NativeStemsBeamSchedulerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid native STEMS beam scheduler frontier: {self:?}"
        )
    }
}

impl Error for NativeStemsBeamSchedulerError {}

/// Materialize the exact deterministic scheduler prefix for every system.
///
/// The implementation follows below the public schema so the integration gate
/// can independently reconstruct every event from the five predecessor
/// products.
pub fn materialize_native_stems_beam_scheduler_frontiers(
    beams: &NativeBeamRecognition,
    beam_stumps: &NativeStemsBeamStumpRecognition,
    beam_vlinkers: &NativeStemsBeamVLinkerRecognition,
    beam_builders: &NativeStemsBeamBuilderRecognition,
    link_plans: &NativeStemsBeamLinkPlanRecognition,
) -> Result<NativeStemsBeamSchedulerRecognition, NativeStemsBeamSchedulerError> {
    let ids = link_plans
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    if ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len()
        || ids.windows(2).any(|pair| pair[0] >= pair[1])
        || beam_stumps
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect::<Vec<_>>()
            != ids
        || beam_vlinkers
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect::<Vec<_>>()
            != ids
        || beam_builders
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect::<Vec<_>>()
            != ids
    {
        return Err(NativeStemsBeamSchedulerError::SystemOrder);
    }

    let creation_pairs = raw_exclusion_pairs(beams, &beam_stumps.systems)?;
    let mut canonical_registry = Vec::<(Bounds, RunTable)>::new();
    let mut totals = SchedulerTotals::default();
    let mut systems = Vec::with_capacity(ids.len());
    for index in 0..ids.len() {
        let system = materialize_system(
            beams,
            &creation_pairs,
            &beam_stumps.systems[index],
            &beam_vlinkers.systems[index],
            &beam_builders.systems[index],
            &link_plans.systems[index],
            &mut canonical_registry,
        )?;
        totals.live_beams += system.glyphs_in_sig_order.len();
        totals.live_exclusions += system.live_exclusions.len();
        totals.events += system.prefix_events.len();
        totals.known_false += system
            .prefix_events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    NativeStemsBeamSchedulerEvent::InvokedKnownFalsePlan { .. }
                )
            })
            .count();
        totals.line_deltas += system.deferred_line_deltas.len();
        match system.status {
            NativeStemsBeamSchedulerStatus::Completed { .. } => totals.completed += 1,
            NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(_) => {
                totals.awaiting_v += 1;
            }
            NativeStemsBeamSchedulerStatus::AwaitingHookRemovalTransaction(_) => {
                totals.awaiting_hook += 1;
            }
        }
        systems.push(system);
    }

    Ok(NativeStemsBeamSchedulerRecognition {
        systems,
        live_beam_count: totals.live_beams,
        canonical_glyph_class_count: canonical_registry.len(),
        live_exclusion_count: totals.live_exclusions,
        prefix_event_count: totals.events,
        invoked_known_false_plan_count: totals.known_false,
        deferred_line_delta_count: totals.line_deltas,
        completed_system_count: totals.completed,
        awaiting_v_link_transaction_count: totals.awaiting_v,
        awaiting_hook_removal_transaction_count: totals.awaiting_hook,
        forbidden_mutation_count: 0,
    })
}

#[derive(Default)]
struct SchedulerTotals {
    live_beams: usize,
    live_exclusions: usize,
    events: usize,
    known_false: usize,
    line_deltas: usize,
    completed: usize,
    awaiting_v: usize,
    awaiting_hook: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RawExclusionPair {
    creation_ordinal: usize,
    system_id: usize,
    hook: NativeStemsBeamSource,
    beam: NativeStemsBeamSource,
}

struct LiveBeam<'a> {
    stump: &'a NativeStemsBeamStumpBeam,
    constructor: &'a NativeStemsBeamVLinkerConstructor,
    scheduler_sig_ordinal: usize,
    alias_class: usize,
}

struct PlanLookup<'a> {
    reference: NativeStemsBeamPlanRef,
    attempt: &'a NativeStemsBeamLinkPlanAttempt,
}

#[derive(Default)]
struct ReplayState {
    events: Vec<NativeStemsBeamSchedulerEvent>,
    deferred_line_deltas: Vec<NativeStemsBeamDeferredLineDelta>,
    shifted_v_linkers: Vec<NativeStemsBeamVLinkerRef>,
    invocation_ordinal: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KnownSideDisposition {
    Retain,
    RemoveLocal,
    AwaitHookRemoval,
}

fn raw_exclusion_pairs(
    beams: &NativeBeamRecognition,
    stump_systems: &[NativeStemsBeamStumpSystem],
) -> Result<Vec<RawExclusionPair>, NativeStemsBeamSchedulerError> {
    let mut pairs = Vec::new();
    for system in stump_systems {
        // Java enumerates the surviving pair relations from each system SIG,
        // in sheet-system order. `createBeamInters` registers hook then beam
        // and immediately inserts their exclusion, so the pair edge order is
        // the post-HEADS, pre-tremolo beam-only SIG order -- not the global raw
        // recognition stream.
        let mut sig_beams = system.beams_by_abscissa.iter().collect::<Vec<_>>();
        sig_beams.sort_by_key(|beam| beam.sig_ordinal);
        if sig_beams
            .iter()
            .enumerate()
            .any(|(ordinal, beam)| beam.sig_ordinal != ordinal)
        {
            return Err(NativeStemsBeamSchedulerError::InvalidLiveSigOrder {
                system_id: system.system_id,
            });
        }
        let sig_sources = sig_beams.iter().map(|beam| beam.source).collect::<Vec<_>>();
        let mut pair_by_hook = BTreeMap::new();
        for stump in &sig_beams {
            let NativeStemsBeamSource::RawBeam(hook_ordinal) = stump.source else {
                continue;
            };
            let Some((owner, hook)) = beams.raw_beams.get(hook_ordinal) else {
                return Err(NativeStemsBeamSchedulerError::InvalidBeamSource {
                    system_id: system.system_id,
                    source: stump.source,
                });
            };
            if *owner != system.system_id || hook.kind != stump.kind {
                return Err(NativeStemsBeamSchedulerError::InvalidBeamSource {
                    system_id: system.system_id,
                    source: stump.source,
                });
            }
            if hook.kind != BeamKind::Hook
                || raw_hook_beam_exclusion_key(beams, hook_ordinal) != Some(hook_ordinal)
            {
                continue;
            }
            let Some(beam_ordinal) = hook_ordinal.checked_add(1) else {
                return Err(
                    NativeStemsBeamSchedulerError::UnsupportedExclusionTopology {
                        system_id: system.system_id,
                        source: stump.source,
                    },
                );
            };
            let full_source = NativeStemsBeamSource::RawBeam(beam_ordinal);
            if sig_sources.contains(&full_source) {
                pair_by_hook.insert(stump.source, full_source);
            }
        }
        push_sig_ordered_pairs(system.system_id, &sig_sources, &pair_by_hook, &mut pairs)?;
    }
    Ok(pairs)
}

fn push_sig_ordered_pairs(
    system_id: usize,
    sig_sources: &[NativeStemsBeamSource],
    pair_by_hook: &BTreeMap<NativeStemsBeamSource, NativeStemsBeamSource>,
    pairs: &mut Vec<RawExclusionPair>,
) -> Result<(), NativeStemsBeamSchedulerError> {
    for (sig_ordinal, &hook) in sig_sources.iter().enumerate() {
        let Some(&beam) = pair_by_hook.get(&hook) else {
            continue;
        };
        if sig_sources.get(sig_ordinal + 1) != Some(&beam) {
            return Err(
                NativeStemsBeamSchedulerError::UnsupportedExclusionTopology {
                    system_id,
                    source: hook,
                },
            );
        }
        pairs.push(RawExclusionPair {
            creation_ordinal: pairs.len(),
            system_id,
            hook,
            beam,
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn materialize_system(
    beams: &NativeBeamRecognition,
    creation_pairs: &[RawExclusionPair],
    stump_system: &NativeStemsBeamStumpSystem,
    v_system: &NativeStemsBeamVLinkerSystem,
    builder_system: &NativeStemsBeamBuilderSystem,
    plan_system: &NativeStemsBeamLinkPlanSystem,
    canonical_registry: &mut Vec<(Bounds, RunTable)>,
) -> Result<NativeStemsBeamSchedulerSystem, NativeStemsBeamSchedulerError> {
    let system_id = stump_system.system_id;
    if v_system.system_id != system_id
        || builder_system.system_id != system_id
        || plan_system.system_id != system_id
        || stump_system.profile != v_system.profile
        || stump_system.profile != plan_system.link_profile
        || !(0..=4).contains(&plan_system.link_profile)
        || stump_system.interline <= 0
        || stump_system.interline != v_system.interline
        || stump_system.interline != builder_system.interline
        || stump_system.interline != plan_system.interline
    {
        return Err(NativeStemsBeamSchedulerError::SystemOrder);
    }
    validate_builder_plan_matrix(system_id, builder_system, plan_system)?;

    if v_system.constructors.len() != stump_system.beams_by_abscissa.len() {
        return Err(NativeStemsBeamSchedulerError::InvalidLiveSigOrder { system_id });
    }
    let mut pre_tremolo_sig_ordinals = stump_system
        .beams_by_abscissa
        .iter()
        .map(|beam| beam.sig_ordinal)
        .collect::<Vec<_>>();
    pre_tremolo_sig_ordinals.sort_unstable();
    if pre_tremolo_sig_ordinals != (0..stump_system.beams_by_abscissa.len()).collect::<Vec<_>>() {
        return Err(NativeStemsBeamSchedulerError::InvalidLiveSigOrder { system_id });
    }
    let mut constructor_sources = Vec::with_capacity(v_system.constructors.len());
    let mut live = Vec::new();
    for (x_ordinal, constructor) in v_system.constructors.iter().enumerate() {
        validate_constructor_topology(system_id, constructor)?;
        if constructor_sources.contains(&constructor.source) {
            return Err(NativeStemsBeamSchedulerError::InvalidLiveSigOrder { system_id });
        }
        constructor_sources.push(constructor.source);
        let stump = find_stump_beam(stump_system, constructor.source).ok_or(
            NativeStemsBeamSchedulerError::InvalidBeamSource {
                system_id,
                source: constructor.source,
            },
        )?;
        if constructor.x_ordinal != x_ordinal
            || stump.x_ordinal != x_ordinal
            || stump_system.beams_by_abscissa[x_ordinal].source != constructor.source
            || constructor.looks_like_tremolo != stump.looks_like_tremolo
            || constructor.survives_constructor_loop == constructor.looks_like_tremolo
        {
            return Err(NativeStemsBeamSchedulerError::InvalidLiveSigOrder { system_id });
        }
        if constructor.survives_constructor_loop {
            live.push((stump, constructor));
        }
    }
    if stump_system
        .beams_by_abscissa
        .iter()
        .any(|beam| !constructor_sources.contains(&beam.source))
    {
        return Err(NativeStemsBeamSchedulerError::InvalidLiveSigOrder { system_id });
    }
    let expected_builder_starts = v_system
        .constructors
        .iter()
        .filter(|constructor| constructor.survives_constructor_loop)
        .flat_map(|constructor| &constructor.b_linkers)
        .flat_map(|b_linker| &b_linker.v_linkers)
        .map(|v_linker| v_linker.reference)
        .collect::<Vec<_>>();
    if expected_builder_starts
        != builder_system
            .builders
            .iter()
            .map(|builder| builder.start)
            .collect::<Vec<_>>()
    {
        return Err(NativeStemsBeamSchedulerError::MissingSystemProduct {
            system_id,
            product: "complete ordered beam VLinker builder coverage",
        });
    }
    live.sort_by_key(|(beam, _)| beam.sig_ordinal);
    for pair in live.windows(2) {
        if pair[0].0.sig_ordinal >= pair[1].0.sig_ordinal {
            return Err(NativeStemsBeamSchedulerError::InvalidLiveSigOrder { system_id });
        }
    }

    let mut glyphs_in_sig_order = Vec::with_capacity(live.len());
    let mut live_records = Vec::with_capacity(live.len());
    for (scheduler_sig_ordinal, (stump, constructor)) in live.into_iter().enumerate() {
        validate_live_beam(beams, system_id, stump)?;
        let alias_class = canonical_registry
            .iter()
            .position(|(bounds, run_table)| {
                *bounds == stump.beam_glyph.bounds && *run_table == stump.beam_glyph.run_table
            })
            .unwrap_or_else(|| {
                let alias = canonical_registry.len();
                canonical_registry
                    .push((stump.beam_glyph.bounds, stump.beam_glyph.run_table.clone()));
                alias
            });
        glyphs_in_sig_order.push(NativeStemsBeamCanonicalGlyph {
            source: stump.source,
            scheduler_sig_ordinal,
            pre_tremolo_sig_ordinal: stump.sig_ordinal,
            alias_class,
            bounds: stump.beam_glyph.bounds,
            run_table: stump.beam_glyph.run_table.clone(),
            run_digest: stump.beam_glyph.run_digest(),
        });
        live_records.push(LiveBeam {
            stump,
            constructor,
            scheduler_sig_ordinal,
            alias_class,
        });
    }

    let live_map = live_records
        .iter()
        .map(|record| (record.stump.source, record))
        .collect::<BTreeMap<_, _>>();
    let mut live_exclusions = Vec::new();
    for pair in creation_pairs
        .iter()
        .filter(|pair| pair.system_id == system_id)
    {
        let hook = live_map.get(&pair.hook).copied();
        let beam = live_map.get(&pair.beam).copied();
        if let (Some(hook), Some(beam)) = (hook, beam) {
            if hook.stump.kind != BeamKind::Hook
                || beam.stump.kind != BeamKind::Beam
                || hook.alias_class != beam.alias_class
            {
                return Err(
                    NativeStemsBeamSchedulerError::UnsupportedExclusionTopology {
                        system_id,
                        source: pair.beam,
                    },
                );
            }
            live_exclusions.push(NativeStemsBeamLiveExclusion {
                creation_ordinal: pair.creation_ordinal,
                live_ordinal: live_exclusions.len(),
                hook: pair.hook,
                beam: pair.beam,
                canonical_glyph_alias: beam.alias_class,
            });
        }
    }

    let mut width_order = live_records.iter().collect::<Vec<_>>();
    width_order.sort_by(|left, right| {
        right
            .stump
            .bounds
            .width
            .cmp(&left.stump.bounds.width)
            .then_with(|| left.scheduler_sig_ordinal.cmp(&right.scheduler_sig_ordinal))
    });
    validate_exclusion_visibility(system_id, &width_order, &live_exclusions)?;

    let mut beams_by_reverse_width = Vec::with_capacity(width_order.len());
    for (width_ordinal, record) in width_order.iter().enumerate() {
        let competing_hook = competing_hook(
            system_id,
            record.stump.source,
            record.alias_class,
            &live_map,
            &live_exclusions,
        )?;
        let selected_side_stem_profile =
            if record.stump.kind == BeamKind::Hook || competing_hook.is_some() {
                plan_system.link_profile
            } else {
                BEAM_SIDE_PROFILE
            };
        beams_by_reverse_width.push(NativeStemsBeamScheduledBeam {
            width_ordinal,
            source: record.stump.source,
            scheduler_sig_ordinal: record.scheduler_sig_ordinal,
            integer_width: record.stump.bounds.width,
            kind: record.stump.kind,
            canonical_glyph_alias: record.alias_class,
            competing_hook,
            selected_side_stem_profile,
        });
    }

    let mut state = ReplayState::default();
    let status = replay_prefix(
        system_id,
        &beams_by_reverse_width,
        &live_map,
        builder_system,
        plan_system,
        &mut state,
    )?;
    Ok(NativeStemsBeamSchedulerSystem {
        system_id,
        link_profile: plan_system.link_profile,
        glyphs_in_sig_order,
        live_exclusions,
        beams_by_reverse_width,
        prefix_events: state.events,
        deferred_line_deltas: state.deferred_line_deltas,
        status,
    })
}

fn validate_builder_plan_matrix(
    system_id: usize,
    builders: &NativeStemsBeamBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<(), NativeStemsBeamSchedulerError> {
    if builders.builders.len() != plans.builders.len() {
        return Err(NativeStemsBeamSchedulerError::MissingSystemProduct {
            system_id,
            product: "beam builder/link-plan matrix",
        });
    }
    let mut starts = Vec::with_capacity(builders.builders.len());
    for (expected_ordinal, (builder, plan)) in
        builders.builders.iter().zip(&plans.builders).enumerate()
    {
        let expected_attempt_count = plan
            .construction_max_profile
            .checked_add(1)
            .and_then(|count| usize::try_from(count).ok());
        if builder.builder_ordinal != expected_ordinal
            || plan.builder_ordinal != expected_ordinal
            || builder.start != plan.start
            || builder.max_stem_profile != plan.construction_max_profile
            || !(3..=4).contains(&plan.construction_max_profile)
            || expected_attempt_count != Some(plan.attempts.len())
            || starts.contains(&builder.start)
        {
            return Err(NativeStemsBeamSchedulerError::MissingSystemProduct {
                system_id,
                product: "beam builder/link-plan matrix",
            });
        }
        starts.push(builder.start);
        let head_target_count = final_builder_target_counts(builder).1;
        for (profile, attempt) in (0..=plan.construction_max_profile).zip(&plan.attempts) {
            if attempt.stem_profile != profile
                || attempt.link_profile != plans.link_profile
                || attempt.head_target_count != head_target_count
                || !head_target_outcome_consistent(head_target_count, attempt.outcome)
            {
                return Err(NativeStemsBeamSchedulerError::MissingSystemProduct {
                    system_id,
                    product: "beam builder/link-plan profile matrix",
                });
            }
        }
    }
    Ok(())
}

fn validate_constructor_topology(
    system_id: usize,
    constructor: &NativeStemsBeamVLinkerConstructor,
) -> Result<(), NativeStemsBeamSchedulerError> {
    if constructor.side_b_linkers.len() != 2
        || constructor.side_b_linkers[0].side != NativeStemHeadSide::Left
        || constructor.side_b_linkers[1].side != NativeStemHeadSide::Right
    {
        return Err(NativeStemsBeamSchedulerError::MissingSystemProduct {
            system_id,
            product: "ordered beam side map",
        });
    }
    for (index, b_linker) in constructor.b_linkers.iter().enumerate() {
        if b_linker.reference.beam != constructor.source
            || b_linker.reference.id != index + 1
            || b_linker
                .v_linkers
                .iter()
                .any(|v| v.reference.b_linker != b_linker.reference)
            || b_linker
                .v_linkers
                .windows(2)
                .any(|pair| pair[0].reference.side >= pair[1].reference.side)
        {
            return Err(NativeStemsBeamSchedulerError::MissingBLinker {
                system_id,
                reference: b_linker.reference,
            });
        }
    }
    for side in &constructor.side_b_linkers {
        if let Some(reference) = side.b_linker
            && find_b_linker(constructor, reference).is_none()
        {
            return Err(NativeStemsBeamSchedulerError::MissingBLinker {
                system_id,
                reference,
            });
        }
    }
    let expected_stump_order = constructor
        .stump_equipments
        .iter()
        .flat_map(|equipment| equipment.v_linkers.iter().copied())
        .collect::<Vec<_>>();
    if expected_stump_order != constructor.stump_v_linkers
        || constructor
            .stump_v_linkers
            .iter()
            .enumerate()
            .any(|(index, reference)| constructor.stump_v_linkers[..index].contains(reference))
        || constructor
            .stump_v_linkers
            .iter()
            .any(|reference| find_b_linker(constructor, reference.b_linker).is_none())
    {
        return Err(NativeStemsBeamSchedulerError::MissingSystemProduct {
            system_id,
            product: "ordered stump VLinker list",
        });
    }
    Ok(())
}

fn validate_live_beam(
    beams: &NativeBeamRecognition,
    system_id: usize,
    beam: &NativeStemsBeamStumpBeam,
) -> Result<(), NativeStemsBeamSchedulerError> {
    let pair = match beam.source {
        NativeStemsBeamSource::RawBeam(ordinal) => beams
            .raw_beams
            .get(ordinal)
            .zip(beams.raw_beam_glyphs.get(ordinal)),
        NativeStemsBeamSource::Hook(ordinal) => {
            beams.hooks.get(ordinal).zip(beams.hook_glyphs.get(ordinal))
        }
    }
    .ok_or(NativeStemsBeamSchedulerError::InvalidBeamSource {
        system_id,
        source: beam.source,
    })?;
    let ((owner, raw), (glyph_owner, glyph)) = pair;
    let native_bounds = &beam.beam_glyph.bounds;
    if *owner != system_id
        || *glyph_owner != system_id
        || raw.kind != beam.kind
        || usize::try_from(glyph.bounds.x).ok() != Some(native_bounds.x)
        || usize::try_from(glyph.bounds.y).ok() != Some(native_bounds.y)
        || usize::try_from(glyph.bounds.width).ok() != Some(native_bounds.width)
        || usize::try_from(glyph.bounds.height).ok() != Some(native_bounds.height)
        || usize::try_from(beam.bounds.x).ok() != Some(native_bounds.x)
        || usize::try_from(beam.bounds.y).ok() != Some(native_bounds.y)
        || usize::try_from(beam.bounds.width).ok() != Some(native_bounds.width)
        || usize::try_from(beam.bounds.height).ok() != Some(native_bounds.height)
        || glyph.run_table != beam.beam_glyph.run_table
    {
        return Err(NativeStemsBeamSchedulerError::InvalidCanonicalGlyph {
            system_id,
            source: beam.source,
        });
    }
    Ok(())
}

fn validate_exclusion_visibility(
    system_id: usize,
    width_order: &[&LiveBeam<'_>],
    exclusions: &[NativeStemsBeamLiveExclusion],
) -> Result<(), NativeStemsBeamSchedulerError> {
    for exclusion in exclusions {
        let hook = width_order
            .iter()
            .position(|record| record.stump.source == exclusion.hook)
            .ok_or(
                NativeStemsBeamSchedulerError::UnsupportedExclusionTopology {
                    system_id,
                    source: exclusion.hook,
                },
            )?;
        let beam = width_order
            .iter()
            .position(|record| record.stump.source == exclusion.beam)
            .ok_or(
                NativeStemsBeamSchedulerError::UnsupportedExclusionTopology {
                    system_id,
                    source: exclusion.beam,
                },
            )?;
        if width_order[hook].stump.bounds.width != width_order[beam].stump.bounds.width
            || hook >= beam
        {
            // A deferred full-beam hook removal would otherwise be visible to
            // a later hook scheduler entry, which this pure prefix cannot fake.
            return Err(
                NativeStemsBeamSchedulerError::UnsupportedExclusionTopology {
                    system_id,
                    source: exclusion.beam,
                },
            );
        }
    }
    Ok(())
}

fn competing_hook(
    system_id: usize,
    source: NativeStemsBeamSource,
    alias: usize,
    live: &BTreeMap<NativeStemsBeamSource, &LiveBeam<'_>>,
    exclusions: &[NativeStemsBeamLiveExclusion],
) -> Result<Option<NativeStemsBeamSource>, NativeStemsBeamSchedulerError> {
    let mut selected = None;
    for exclusion in exclusions {
        let opposite = if exclusion.beam == source {
            Some(exclusion.hook)
        } else if exclusion.hook == source {
            Some(exclusion.beam)
        } else {
            None
        };
        let Some(opposite) = opposite else {
            continue;
        };
        let opposite_record = live.get(&opposite).copied().ok_or(
            NativeStemsBeamSchedulerError::UnsupportedExclusionTopology { system_id, source },
        )?;
        // `getCompetingHook` scans ordered Exclusion neighbors and filters the
        // opposite shape before testing canonical object identity.
        if opposite_record.stump.kind == BeamKind::Hook && opposite_record.alias_class == alias {
            if selected.is_some() {
                return Err(
                    NativeStemsBeamSchedulerError::UnsupportedExclusionTopology {
                        system_id,
                        source,
                    },
                );
            }
            selected = Some(opposite);
        }
    }
    Ok(selected)
}

fn replay_prefix(
    system_id: usize,
    scheduled: &[NativeStemsBeamScheduledBeam],
    live: &BTreeMap<NativeStemsBeamSource, &LiveBeam<'_>>,
    builders: &NativeStemsBeamBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    state: &mut ReplayState,
) -> Result<NativeStemsBeamSchedulerStatus, NativeStemsBeamSchedulerError> {
    let mut side_worklist = scheduled.iter().map(|beam| beam.source).collect::<Vec<_>>();
    let mut retained = Vec::new();
    let mut index = 0;
    while index < side_worklist.len() {
        let source = side_worklist[index];
        let snapshot =
            worklist_snapshot(NativeStemsBeamSchedulerPass::Sides, index, &side_worklist);
        let scheduled_beam = scheduled
            .iter()
            .find(|beam| beam.source == source)
            .ok_or(NativeStemsBeamSchedulerError::InvalidBeamSource { system_id, source })?;
        let record = live
            .get(&source)
            .copied()
            .ok_or(NativeStemsBeamSchedulerError::InvalidBeamSource { system_id, source })?;
        let event_ordinal = state.events.len();
        state
            .events
            .push(NativeStemsBeamSchedulerEvent::BeamPassStart {
                event_ordinal,
                snapshot: snapshot.clone(),
                kind: scheduled_beam.kind,
                competing_hook: scheduled_beam.competing_hook,
                selected_stem_profile: scheduled_beam.selected_side_stem_profile,
            });

        let mut linked_sides = Vec::new();
        let mut failed_side = None;
        for side in [NativeStemHeadSide::Left, NativeStemHeadSide::Right] {
            let side_entry = record
                .constructor
                .side_b_linkers
                .iter()
                .find(|entry| entry.side == side)
                .ok_or(NativeStemsBeamSchedulerError::MissingSystemProduct {
                    system_id,
                    product: "LEFT/RIGHT side BLinker map",
                })?;
            let Some(b_reference) = side_entry.b_linker else {
                let event_ordinal = state.events.len();
                state
                    .events
                    .push(NativeStemsBeamSchedulerEvent::MissingSideBLinker {
                        event_ordinal,
                        beam: source,
                        side,
                    });
                continue;
            };
            let b_linker = find_b_linker(record.constructor, b_reference).ok_or(
                NativeStemsBeamSchedulerError::MissingBLinker {
                    system_id,
                    reference: b_reference,
                },
            )?;
            let logical_success = if b_linker.v_linkers.is_empty() {
                let event_ordinal = state.events.len();
                state
                    .events
                    .push(NativeStemsBeamSchedulerEvent::EmptyVLinkerSideSuccess {
                        event_ordinal,
                        beam: source,
                        side,
                        b_linker: b_reference,
                        linked_flag_after: false,
                    });
                true
            } else {
                for v_linker in &b_linker.v_linkers {
                    let builder = find_builder(builders, v_linker.reference).ok_or(
                        NativeStemsBeamSchedulerError::MissingBuilder {
                            system_id,
                            reference: v_linker.reference,
                        },
                    )?;
                    // `BLinker.link` asks the completed StemBuilder for its
                    // target linkers. A retained inspection candidate can be
                    // absent from the final item sequence after construction,
                    // so `targets_after_filter` is intentionally not this
                    // scheduler precheck.
                    if final_builder_target_counts(builder).0 == 0 {
                        let event_ordinal = state.events.len();
                        state.events.push(
                            NativeStemsBeamSchedulerEvent::SideVSkippedEmptyTargets {
                                event_ordinal,
                                beam: source,
                                side,
                                b_linker: b_reference,
                                v_linker: v_linker.reference,
                                builder_ordinal: builder.builder_ordinal,
                            },
                        );
                        continue;
                    }
                    let lookup = find_plan(
                        system_id,
                        builders,
                        plans,
                        v_linker.reference,
                        scheduled_beam.selected_side_stem_profile,
                    )?;
                    if lookup.attempt.outcome == NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem
                    {
                        reject_repeated_v(system_id, state, v_linker.reference)?;
                        return Ok(NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(
                            Box::new(NativeStemsBeamAwaitingVLinkTransaction {
                                invocation_ordinal: state.invocation_ordinal,
                                snapshot,
                                beam: source,
                                horizontal_side: Some(side),
                                b_linker: b_reference,
                                v_linker: v_linker.reference,
                                vertical_side: v_linker.reference.side,
                                plan: lookup.reference,
                                outcome: lookup.attempt.outcome,
                                linked_sides_before: linked_sides,
                                retained_beams_before: retained,
                                would_apply_stored_line_delta: pending_line_delta(
                                    state,
                                    lookup.reference,
                                    v_linker.reference,
                                    lookup.attempt,
                                ),
                            }),
                        ));
                    }
                    record_known_false(
                        system_id,
                        state,
                        NativeStemsBeamSchedulerPass::Sides,
                        source,
                        Some(side),
                        b_reference,
                        v_linker.reference,
                        lookup,
                    )?;
                }
                // Before the first unresolved transaction every invoked V has
                // returned false, so this non-empty B map remains unlinked.
                false
            };
            let event_ordinal = state.events.len();
            state
                .events
                .push(NativeStemsBeamSchedulerEvent::SideBLinkerResult {
                    event_ordinal,
                    beam: source,
                    side,
                    b_linker: b_reference,
                    logical_success,
                    linked_flag_after: false,
                });
            if logical_success {
                linked_sides.push(side);
            } else if scheduled_beam.kind != BeamKind::Hook {
                failed_side = Some(side);
                break;
            }
        }

        match known_side_disposition(
            scheduled_beam.kind,
            failed_side,
            &linked_sides,
            scheduled_beam.competing_hook.is_some(),
        ) {
            KnownSideDisposition::RemoveLocal => {
                side_worklist.remove(index);
                let event_ordinal = state.events.len();
                state.events.push(
                    NativeStemsBeamSchedulerEvent::BeamRemovedFromLocalWorklist {
                        event_ordinal,
                        beam: source,
                        failed_side,
                        worklist_after: side_worklist.clone(),
                    },
                );
                continue;
            }
            KnownSideDisposition::AwaitHookRemoval => {
                let competing_hook = scheduled_beam.competing_hook.ok_or(
                    NativeStemsBeamSchedulerError::UnsupportedExclusionTopology {
                        system_id,
                        source,
                    },
                )?;
                return Ok(
                    NativeStemsBeamSchedulerStatus::AwaitingHookRemovalTransaction(Box::new(
                        NativeStemsBeamAwaitingHookRemovalTransaction {
                            snapshot,
                            beam: source,
                            competing_hook,
                            linked_sides,
                            retained_beams_before: retained,
                        },
                    )),
                );
            }
            KnownSideDisposition::Retain => {}
        }

        retained.push(source);
        let event_ordinal = state.events.len();
        state
            .events
            .push(NativeStemsBeamSchedulerEvent::BeamRetainedForStumps {
                event_ordinal,
                beam: source,
                linked_sides,
            });
        index += 1;
    }

    if retained != side_worklist {
        return Err(NativeStemsBeamSchedulerError::InvalidLiveSigOrder { system_id });
    }

    let mut stump_processed = Vec::new();
    for (stump_index, &source) in side_worklist.iter().enumerate() {
        let snapshot = worklist_snapshot(
            NativeStemsBeamSchedulerPass::Stumps,
            stump_index,
            &side_worklist,
        );
        let scheduled_beam = scheduled
            .iter()
            .find(|beam| beam.source == source)
            .ok_or(NativeStemsBeamSchedulerError::InvalidBeamSource { system_id, source })?;
        let record = live
            .get(&source)
            .copied()
            .ok_or(NativeStemsBeamSchedulerError::InvalidBeamSource { system_id, source })?;
        let event_ordinal = state.events.len();
        state
            .events
            .push(NativeStemsBeamSchedulerEvent::BeamPassStart {
                event_ordinal,
                snapshot: snapshot.clone(),
                kind: scheduled_beam.kind,
                competing_hook: scheduled_beam.competing_hook,
                selected_stem_profile: BEAM_SEED_PROFILE,
            });

        let side_aliases = record
            .stump
            .sides
            .iter()
            .filter_map(|side| side.final_stump.as_ref().map(stump_canonical_alias))
            .collect::<Vec<_>>();
        for &v_reference in &record.constructor.stump_v_linkers {
            let b_linker = find_b_linker(record.constructor, v_reference.b_linker).ok_or(
                NativeStemsBeamSchedulerError::MissingBLinker {
                    system_id,
                    reference: v_reference.b_linker,
                },
            )?;
            let stump =
                b_linker
                    .stump
                    .as_ref()
                    .ok_or(NativeStemsBeamSchedulerError::InvalidStump {
                        system_id,
                        reference: b_linker.reference,
                    })?;
            let stump_alias = stump_canonical_alias(stump);
            validate_inline_built_stump(system_id, record.stump, b_linker.reference, stump)?;
            if side_aliases.contains(&stump_alias) {
                let event_ordinal = state.events.len();
                state.events.push(
                    NativeStemsBeamSchedulerEvent::StumpSkippedStructuralSideGlyph {
                        event_ordinal,
                        beam: source,
                        b_linker: b_linker.reference,
                        v_linker: v_reference,
                        canonical_stump_alias: stump_alias,
                    },
                );
                continue;
            }

            let lookup = find_plan(system_id, builders, plans, v_reference, BEAM_SEED_PROFILE)?;
            if lookup.attempt.outcome == NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem {
                reject_repeated_v(system_id, state, v_reference)?;
                return Ok(NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(
                    Box::new(NativeStemsBeamAwaitingVLinkTransaction {
                        invocation_ordinal: state.invocation_ordinal,
                        snapshot,
                        beam: source,
                        horizontal_side: None,
                        b_linker: b_linker.reference,
                        v_linker: v_reference,
                        vertical_side: v_reference.side,
                        plan: lookup.reference,
                        outcome: lookup.attempt.outcome,
                        linked_sides_before: Vec::new(),
                        retained_beams_before: stump_processed,
                        would_apply_stored_line_delta: pending_line_delta(
                            state,
                            lookup.reference,
                            v_reference,
                            lookup.attempt,
                        ),
                    }),
                ));
            }
            record_known_false(
                system_id,
                state,
                NativeStemsBeamSchedulerPass::Stumps,
                source,
                None,
                b_linker.reference,
                v_reference,
                lookup,
            )?;
        }
        stump_processed.push(source);
    }

    Ok(NativeStemsBeamSchedulerStatus::Completed {
        retained_for_stumps: retained,
        final_local_worklist: side_worklist,
    })
}

fn final_builder_target_counts(
    builder: &crate::native_stems_beam_builders::NativeStemsBeamBuilder,
) -> (usize, usize) {
    final_target_item_counts(&builder.items)
}

fn final_target_item_counts(
    items: &[crate::native_stems_beam_builders::NativeStemsBeamBuilderItem],
) -> (usize, usize) {
    items.iter().fold((0, 0), |(all, heads), item| {
        if let Some(target) = item.target {
            (
                all + 1,
                heads + usize::from(matches!(target, NativeStemsBeamBuilderTargetRef::Head(_))),
            )
        } else {
            (all, heads)
        }
    })
}

fn head_target_outcome_consistent(
    head_target_count: usize,
    outcome: NativeStemsBeamLinkPlanOutcome,
) -> bool {
    (head_target_count == 0) == (outcome == NativeStemsBeamLinkPlanOutcome::NoHeadTarget)
}

fn known_side_disposition(
    kind: BeamKind,
    failed_side: Option<NativeStemHeadSide>,
    linked_sides: &[NativeStemHeadSide],
    has_competing_hook: bool,
) -> KnownSideDisposition {
    if failed_side.is_some() || (kind == BeamKind::Hook && linked_sides.is_empty()) {
        KnownSideDisposition::RemoveLocal
    } else if kind != BeamKind::Hook && linked_sides.len() == 2 && has_competing_hook {
        KnownSideDisposition::AwaitHookRemoval
    } else {
        KnownSideDisposition::Retain
    }
}

fn worklist_snapshot(
    pass: NativeStemsBeamSchedulerPass,
    current_index: usize,
    sources: &[NativeStemsBeamSource],
) -> NativeStemsBeamWorklistSnapshot {
    NativeStemsBeamWorklistSnapshot {
        pass,
        current_index,
        sources: sources.to_vec(),
        current: sources[current_index],
        remaining: sources[(current_index + 1)..].to_vec(),
    }
}

fn find_stump_beam(
    system: &NativeStemsBeamStumpSystem,
    source: NativeStemsBeamSource,
) -> Option<&NativeStemsBeamStumpBeam> {
    system
        .beams_by_abscissa
        .iter()
        .find(|beam| beam.source == source)
}

fn find_b_linker(
    constructor: &NativeStemsBeamVLinkerConstructor,
    reference: NativeStemsBeamBLinkerRef,
) -> Option<&NativeStemsBeamBLinker> {
    constructor
        .b_linkers
        .iter()
        .find(|linker| linker.reference == reference)
}

fn find_builder(
    system: &NativeStemsBeamBuilderSystem,
    reference: NativeStemsBeamVLinkerRef,
) -> Option<&crate::native_stems_beam_builders::NativeStemsBeamBuilder> {
    system
        .builders
        .iter()
        .find(|builder| builder.start == reference)
}

fn find_plan<'a>(
    system_id: usize,
    builders: &NativeStemsBeamBuilderSystem,
    plans: &'a NativeStemsBeamLinkPlanSystem,
    reference: NativeStemsBeamVLinkerRef,
    stem_profile: i32,
) -> Result<PlanLookup<'a>, NativeStemsBeamSchedulerError> {
    let builder =
        find_builder(builders, reference).ok_or(NativeStemsBeamSchedulerError::MissingBuilder {
            system_id,
            reference,
        })?;
    let plan_builder = plans
        .builders
        .iter()
        .find(|candidate| candidate.builder_ordinal == builder.builder_ordinal)
        .ok_or(NativeStemsBeamSchedulerError::MissingPlan {
            system_id,
            reference,
            stem_profile,
        })?;
    if plan_builder.start != reference {
        return Err(NativeStemsBeamSchedulerError::MissingPlan {
            system_id,
            reference,
            stem_profile,
        });
    }
    let attempt_index = plan_builder
        .attempts
        .iter()
        .position(|attempt| attempt.stem_profile == stem_profile)
        .ok_or(NativeStemsBeamSchedulerError::MissingPlan {
            system_id,
            reference,
            stem_profile,
        })?;
    let plan_ordinal = plans
        .builders
        .iter()
        .take_while(|candidate| candidate.builder_ordinal != builder.builder_ordinal)
        .map(|candidate| candidate.attempts.len())
        .sum::<usize>()
        + attempt_index;
    let attempt = &plan_builder.attempts[attempt_index];
    if attempt.link_profile != plans.link_profile {
        return Err(NativeStemsBeamSchedulerError::MissingPlan {
            system_id,
            reference,
            stem_profile,
        });
    }
    Ok(PlanLookup {
        reference: NativeStemsBeamPlanRef {
            system_id,
            plan_ordinal,
            builder_ordinal: builder.builder_ordinal,
            stem_profile,
        },
        attempt,
    })
}

#[allow(clippy::too_many_arguments)]
fn record_known_false(
    system_id: usize,
    state: &mut ReplayState,
    pass: NativeStemsBeamSchedulerPass,
    beam: NativeStemsBeamSource,
    horizontal_side: Option<NativeStemHeadSide>,
    b_linker: NativeStemsBeamBLinkerRef,
    v_linker: NativeStemsBeamVLinkerRef,
    lookup: PlanLookup<'_>,
) -> Result<(), NativeStemsBeamSchedulerError> {
    if lookup.attempt.outcome == NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem {
        return Err(NativeStemsBeamSchedulerError::MissingPlan {
            system_id,
            reference: v_linker,
            stem_profile: lookup.reference.stem_profile,
        });
    }
    reject_repeated_v(system_id, state, v_linker)?;
    let deferred_line_delta_ordinal = if lookup.attempt.stored_theoretical_line_would_mutate {
        state.shifted_v_linkers.push(v_linker);
        let delta_ordinal = state.deferred_line_deltas.len();
        state
            .deferred_line_deltas
            .push(NativeStemsBeamDeferredLineDelta {
                delta_ordinal,
                invocation_ordinal: state.invocation_ordinal,
                plan: lookup.reference,
                v_linker,
                before: lookup.attempt.stored_theoretical_line_before,
                after: lookup.attempt.stored_theoretical_line_after,
                builder_line_aliases: lookup.attempt.builder_line_aliases_stored_theoretical_line,
                attachment_aliases: lookup.attempt.attachment_aliases_stored_theoretical_line,
            });
        Some(delta_ordinal)
    } else {
        None
    };
    let event_ordinal = state.events.len();
    state
        .events
        .push(NativeStemsBeamSchedulerEvent::InvokedKnownFalsePlan {
            event_ordinal,
            invocation_ordinal: state.invocation_ordinal,
            pass,
            beam,
            horizontal_side,
            b_linker,
            v_linker,
            plan: lookup.reference,
            outcome: lookup.attempt.outcome,
            deferred_line_delta_ordinal,
        });
    state.invocation_ordinal += 1;
    Ok(())
}

fn reject_repeated_v(
    system_id: usize,
    state: &ReplayState,
    reference: NativeStemsBeamVLinkerRef,
) -> Result<(), NativeStemsBeamSchedulerError> {
    if state.shifted_v_linkers.contains(&reference) {
        return Err(
            NativeStemsBeamSchedulerError::RepeatedVAttemptAfterLineDelta {
                system_id,
                reference,
            },
        );
    }
    Ok(())
}

fn pending_line_delta(
    state: &ReplayState,
    plan: NativeStemsBeamPlanRef,
    v_linker: NativeStemsBeamVLinkerRef,
    attempt: &NativeStemsBeamLinkPlanAttempt,
) -> Option<NativeStemsBeamDeferredLineDelta> {
    attempt
        .stored_theoretical_line_would_mutate
        .then_some(NativeStemsBeamDeferredLineDelta {
            delta_ordinal: state.deferred_line_deltas.len(),
            invocation_ordinal: state.invocation_ordinal,
            plan,
            v_linker,
            before: attempt.stored_theoretical_line_before,
            after: attempt.stored_theoretical_line_after,
            builder_line_aliases: attempt.builder_line_aliases_stored_theoretical_line,
            attachment_aliases: attempt.attachment_aliases_stored_theoretical_line,
        })
}

fn stump_canonical_alias(stump: &NativeStemsBeamStumpRef) -> usize {
    match stump {
        NativeStemsBeamStumpRef::Seed {
            canonical_glyph_index,
            ..
        }
        | NativeStemsBeamStumpRef::Built {
            canonical_glyph_index,
        } => *canonical_glyph_index,
    }
}

fn validate_inline_built_stump(
    system_id: usize,
    beam: &NativeStemsBeamStumpBeam,
    b_linker: NativeStemsBeamBLinkerRef,
    stump: &NativeStemsBeamStumpRef,
) -> Result<(), NativeStemsBeamSchedulerError> {
    let NativeStemsBeamStumpRef::Built {
        canonical_glyph_index,
    } = stump
    else {
        return Ok(());
    };
    let candidates = beam
        .sides
        .iter()
        .filter_map(|side| side.build.as_ref())
        .filter(|build| build.canonical_glyph_index == Some(*canonical_glyph_index))
        .filter_map(|build| build.candidate.as_ref())
        .collect::<Vec<_>>();
    let Some(first) = candidates.first() else {
        return Err(NativeStemsBeamSchedulerError::InvalidStump {
            system_id,
            reference: b_linker,
        });
    };
    if candidates
        .iter()
        .any(|candidate| candidate.bounds != first.bounds || candidate.run_table != first.run_table)
    {
        return Err(NativeStemsBeamSchedulerError::InvalidStump {
            system_id,
            reference: b_linker,
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Boundary 19: resume the SIDES pass after an executed V-link transaction.
//
// The prefix scheduler above stops at the first `ReadyForCreateStem` frontier.
// Boundaries 12-18 execute that transaction; this resume continues the same
// worklist from the suspended position -- remaining V linkers of the completed
// B, the side result the Boundary-18 outer assignment determined, the
// remaining sides and beams -- and stops at the second frontier without
// executing it, or reports SIDES exhaustion.  Scope is deliberately SIDES
// only; the STUMPS continuation is a later boundary.
//
// The per-attempt outcomes come from the same frozen plan matrix the prefix
// consumed.  Post-transaction Java expands run against a mutated SIG, so this
// is an equivalence claim, not a tautology: the Boundary-19 gate compares
// every resumed attempt against the Java oracle, and a page where the
// mutation changes an expand outcome fails the gate loudly.
// ---------------------------------------------------------------------------

/// Evidence that the awaited transaction at the prefix frontier ran to its
/// Boundary-18 completion: the executed plan identity and the outer B-linker
/// flag state it left behind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeStemsBeamCompletedVLinkEvidence {
    pub plan: NativeStemsBeamPlanRef,
    pub b_linker: NativeStemsBeamBLinkerRef,
    pub v_linker: NativeStemsBeamVLinkerRef,
    /// `BLinker.isLinked()` after the Boundary-18 outer assignment.
    pub outer_b_linked_after: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamSchedulerResumeStatus {
    AwaitingVLinkTransaction(Box<NativeStemsBeamAwaitingVLinkTransaction>),
    AwaitingHookRemovalTransaction(Box<NativeStemsBeamAwaitingHookRemovalTransaction>),
    SidesExhaustedBeforeSecondFrontier {
        retained_for_stumps: Vec<NativeStemsBeamSource>,
        final_local_worklist: Vec<NativeStemsBeamSource>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSchedulerResume {
    pub system_id: usize,
    /// Events after the prefix, in order; ordinals continue the prefix stream.
    pub resume_events: Vec<NativeStemsBeamSchedulerEvent>,
    pub status: NativeStemsBeamSchedulerResumeStatus,
}

/// Gate/plan handling for one V attempt during the resume, shared by the
/// completed-B continuation and fresh sides/beams.  Returns the second
/// frontier when the plan is ready, `None` after a skip or known-false event.
#[allow(clippy::too_many_arguments)]
fn resume_side_v_attempt(
    system_id: usize,
    builders: &NativeStemsBeamBuilderSystem,
    plans: &NativeStemsBeamLinkPlanSystem,
    state: &mut ReplayState,
    source: NativeStemsBeamSource,
    side: NativeStemHeadSide,
    b_reference: NativeStemsBeamBLinkerRef,
    v_reference: NativeStemsBeamVLinkerRef,
    stem_profile: i32,
    snapshot: &NativeStemsBeamWorklistSnapshot,
    linked_sides: &[NativeStemHeadSide],
    retained: &[NativeStemsBeamSource],
) -> Result<Option<NativeStemsBeamAwaitingVLinkTransaction>, NativeStemsBeamSchedulerError> {
    let builder = find_builder(builders, v_reference).ok_or(
        NativeStemsBeamSchedulerError::MissingBuilder {
            system_id,
            reference: v_reference,
        },
    )?;
    if final_builder_target_counts(builder).0 == 0 {
        let event_ordinal = state.events.len();
        state
            .events
            .push(NativeStemsBeamSchedulerEvent::SideVSkippedEmptyTargets {
                event_ordinal,
                beam: source,
                side,
                b_linker: b_reference,
                v_linker: v_reference,
                builder_ordinal: builder.builder_ordinal,
            });
        return Ok(None);
    }
    let lookup = find_plan(system_id, builders, plans, v_reference, stem_profile)?;
    if lookup.attempt.outcome == NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem {
        reject_repeated_v(system_id, state, v_reference)?;
        return Ok(Some(NativeStemsBeamAwaitingVLinkTransaction {
            invocation_ordinal: state.invocation_ordinal,
            snapshot: snapshot.clone(),
            beam: source,
            horizontal_side: Some(side),
            b_linker: b_reference,
            v_linker: v_reference,
            vertical_side: v_reference.side,
            plan: lookup.reference,
            outcome: lookup.attempt.outcome,
            linked_sides_before: linked_sides.to_vec(),
            retained_beams_before: retained.to_vec(),
            would_apply_stored_line_delta: pending_line_delta(
                state,
                lookup.reference,
                v_reference,
                lookup.attempt,
            ),
        }));
    }
    record_known_false(
        system_id,
        state,
        NativeStemsBeamSchedulerPass::Sides,
        source,
        Some(side),
        b_reference,
        v_reference,
        lookup,
    )?;
    Ok(None)
}

/// Resume the SIDES pass immediately after the prefix frontier's transaction
/// completed through Boundary 18, and stop at the second frontier.
pub fn resume_native_stems_beam_scheduler_after_transaction(
    scheduler_system: &NativeStemsBeamSchedulerSystem,
    v_system: &NativeStemsBeamVLinkerSystem,
    builder_system: &NativeStemsBeamBuilderSystem,
    plan_system: &NativeStemsBeamLinkPlanSystem,
    completed: &NativeStemsBeamCompletedVLinkEvidence,
) -> Result<NativeStemsBeamSchedulerResume, NativeStemsBeamSchedulerError> {
    let system_id = v_system.system_id;
    if builder_system.system_id != system_id
        || plan_system.system_id != system_id
        || scheduler_system.system_id != system_id
        || scheduler_system.link_profile != plan_system.link_profile
    {
        return Err(NativeStemsBeamSchedulerError::SystemOrder);
    }
    let NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(awaiting) =
        &scheduler_system.status
    else {
        return Err(NativeStemsBeamSchedulerError::SystemOrder);
    };
    let Some(entry_side) = awaiting.horizontal_side else {
        return Err(NativeStemsBeamSchedulerError::SystemOrder);
    };
    if awaiting.snapshot.pass != NativeStemsBeamSchedulerPass::Sides
        || awaiting.plan != completed.plan
        || awaiting.b_linker != completed.b_linker
        || awaiting.v_linker != completed.v_linker
        || !completed.outer_b_linked_after
    {
        return Err(NativeStemsBeamSchedulerError::SystemOrder);
    }

    let mut state = ReplayState {
        events: scheduler_system.prefix_events.clone(),
        deferred_line_deltas: scheduler_system.deferred_line_deltas.clone(),
        // The executed transaction consumed its own expand; a repeated attempt
        // on that V would consult a stale matrix row, so it is rejected like
        // any post-delta repeat.
        shifted_v_linkers: scheduler_system
            .deferred_line_deltas
            .iter()
            .map(|delta| delta.v_linker)
            .chain(std::iter::once(completed.v_linker))
            .collect(),
        invocation_ordinal: awaiting
            .invocation_ordinal
            .checked_add(1)
            .ok_or(NativeStemsBeamSchedulerError::SystemOrder)?,
    };
    let prefix_event_count = state.events.len();
    let scheduled = &scheduler_system.beams_by_reverse_width;
    let mut side_worklist = awaiting.snapshot.sources.clone();
    let mut retained = awaiting.retained_beams_before.clone();
    let mut index = awaiting.snapshot.current_index;
    if side_worklist.get(index).copied() != Some(awaiting.beam) {
        return Err(NativeStemsBeamSchedulerError::InvalidBeamSource {
            system_id,
            source: awaiting.beam,
        });
    }
    let mut pending_entry = Some((entry_side, awaiting.linked_sides_before.clone()));

    let status = 'sides: loop {
        if index >= side_worklist.len() {
            if retained != side_worklist {
                return Err(NativeStemsBeamSchedulerError::InvalidLiveSigOrder { system_id });
            }
            break NativeStemsBeamSchedulerResumeStatus::SidesExhaustedBeforeSecondFrontier {
                retained_for_stumps: retained,
                final_local_worklist: side_worklist,
            };
        }
        let source = side_worklist[index];
        let snapshot =
            worklist_snapshot(NativeStemsBeamSchedulerPass::Sides, index, &side_worklist);
        let scheduled_beam = scheduled
            .iter()
            .find(|beam| beam.source == source)
            .ok_or(NativeStemsBeamSchedulerError::InvalidBeamSource { system_id, source })?;
        let constructor = v_system
            .constructors
            .iter()
            .find(|constructor| constructor.source == source)
            .ok_or(NativeStemsBeamSchedulerError::InvalidBeamSource { system_id, source })?;
        let entry = pending_entry.take();
        if entry.is_none() {
            let event_ordinal = state.events.len();
            state
                .events
                .push(NativeStemsBeamSchedulerEvent::BeamPassStart {
                    event_ordinal,
                    snapshot: snapshot.clone(),
                    kind: scheduled_beam.kind,
                    competing_hook: scheduled_beam.competing_hook,
                    selected_stem_profile: scheduled_beam.selected_side_stem_profile,
                });
        }
        let mut linked_sides = entry
            .as_ref()
            .map(|(_, sides)| sides.clone())
            .unwrap_or_default();
        let mut failed_side = None;
        for side in [NativeStemHeadSide::Left, NativeStemHeadSide::Right] {
            let resumed_here = matches!(&entry, Some((entry_side, _)) if *entry_side == side);
            if let Some((entry_side, _)) = &entry {
                // Sides the prefix fully handled are represented by
                // `linked_sides_before`; only the suspended side onward runs.
                if *entry_side == NativeStemHeadSide::Right && side == NativeStemHeadSide::Left {
                    continue;
                }
            }
            let side_entry = constructor
                .side_b_linkers
                .iter()
                .find(|candidate| candidate.side == side)
                .ok_or(NativeStemsBeamSchedulerError::MissingSystemProduct {
                    system_id,
                    product: "LEFT/RIGHT side BLinker map",
                })?;
            let Some(b_reference) = side_entry.b_linker else {
                if resumed_here {
                    return Err(NativeStemsBeamSchedulerError::SystemOrder);
                }
                let event_ordinal = state.events.len();
                state
                    .events
                    .push(NativeStemsBeamSchedulerEvent::MissingSideBLinker {
                        event_ordinal,
                        beam: source,
                        side,
                    });
                continue;
            };
            let b_linker = find_b_linker(constructor, b_reference).ok_or(
                NativeStemsBeamSchedulerError::MissingBLinker {
                    system_id,
                    reference: b_reference,
                },
            )?;
            let logical_success = if resumed_here {
                if b_reference != completed.b_linker {
                    return Err(NativeStemsBeamSchedulerError::SystemOrder);
                }
                // Java's V loop has no break: linkers after the completed one
                // still run even though the B is already linked.
                let mut seen_completed = false;
                for v_linker in &b_linker.v_linkers {
                    if v_linker.reference == completed.v_linker {
                        seen_completed = true;
                        continue;
                    }
                    if !seen_completed {
                        continue;
                    }
                    if let Some(frontier) = resume_side_v_attempt(
                        system_id,
                        builder_system,
                        plan_system,
                        &mut state,
                        source,
                        side,
                        b_reference,
                        v_linker.reference,
                        scheduled_beam.selected_side_stem_profile,
                        &snapshot,
                        &linked_sides,
                        &retained,
                    )? {
                        break 'sides NativeStemsBeamSchedulerResumeStatus
                            ::AwaitingVLinkTransaction(Box::new(frontier));
                    }
                }
                if !seen_completed {
                    return Err(NativeStemsBeamSchedulerError::SystemOrder);
                }
                true
            } else if b_linker.v_linkers.is_empty() {
                let event_ordinal = state.events.len();
                state
                    .events
                    .push(NativeStemsBeamSchedulerEvent::EmptyVLinkerSideSuccess {
                        event_ordinal,
                        beam: source,
                        side,
                        b_linker: b_reference,
                        linked_flag_after: false,
                    });
                true
            } else {
                for v_linker in &b_linker.v_linkers {
                    if let Some(frontier) = resume_side_v_attempt(
                        system_id,
                        builder_system,
                        plan_system,
                        &mut state,
                        source,
                        side,
                        b_reference,
                        v_linker.reference,
                        scheduled_beam.selected_side_stem_profile,
                        &snapshot,
                        &linked_sides,
                        &retained,
                    )? {
                        break 'sides NativeStemsBeamSchedulerResumeStatus
                            ::AwaitingVLinkTransaction(Box::new(frontier));
                    }
                }
                false
            };
            let event_ordinal = state.events.len();
            state
                .events
                .push(NativeStemsBeamSchedulerEvent::SideBLinkerResult {
                    event_ordinal,
                    beam: source,
                    side,
                    b_linker: b_reference,
                    logical_success,
                    linked_flag_after: resumed_here,
                });
            if logical_success {
                linked_sides.push(side);
            } else if scheduled_beam.kind != BeamKind::Hook {
                failed_side = Some(side);
                break;
            }
        }

        match known_side_disposition(
            scheduled_beam.kind,
            failed_side,
            &linked_sides,
            scheduled_beam.competing_hook.is_some(),
        ) {
            KnownSideDisposition::RemoveLocal => {
                side_worklist.remove(index);
                let event_ordinal = state.events.len();
                state.events.push(
                    NativeStemsBeamSchedulerEvent::BeamRemovedFromLocalWorklist {
                        event_ordinal,
                        beam: source,
                        failed_side,
                        worklist_after: side_worklist.clone(),
                    },
                );
                continue;
            }
            KnownSideDisposition::AwaitHookRemoval => {
                let competing_hook = scheduled_beam.competing_hook.ok_or(
                    NativeStemsBeamSchedulerError::UnsupportedExclusionTopology {
                        system_id,
                        source,
                    },
                )?;
                break NativeStemsBeamSchedulerResumeStatus::AwaitingHookRemovalTransaction(
                    Box::new(NativeStemsBeamAwaitingHookRemovalTransaction {
                        snapshot,
                        beam: source,
                        competing_hook,
                        linked_sides,
                        retained_beams_before: retained,
                    }),
                );
            }
            KnownSideDisposition::Retain => {}
        }

        retained.push(source);
        let event_ordinal = state.events.len();
        state
            .events
            .push(NativeStemsBeamSchedulerEvent::BeamRetainedForStumps {
                event_ordinal,
                beam: source,
                linked_sides,
            });
        index += 1;
    };

    Ok(NativeStemsBeamSchedulerResume {
        system_id,
        resume_events: state.events[prefix_event_count..].to_vec(),
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW: NativeStemsBeamSource = NativeStemsBeamSource::RawBeam(7);

    fn v_linker(side: NativeStemVerticalSide) -> NativeStemsBeamVLinkerRef {
        NativeStemsBeamVLinkerRef {
            b_linker: NativeStemsBeamBLinkerRef { beam: RAW, id: 2 },
            side,
        }
    }

    #[test]
    fn worklist_snapshot_retains_current_and_remaining_order() {
        let sources = [
            NativeStemsBeamSource::RawBeam(3),
            NativeStemsBeamSource::Hook(5),
            RAW,
        ];
        let snapshot = worklist_snapshot(NativeStemsBeamSchedulerPass::Sides, 1, &sources);
        assert_eq!(snapshot.current_index, 1);
        assert_eq!(snapshot.current, NativeStemsBeamSource::Hook(5));
        assert_eq!(snapshot.sources, sources);
        assert_eq!(snapshot.remaining, vec![RAW]);
    }

    #[test]
    fn side_disposition_preserves_hook_or_and_empty_regular_success() {
        assert_eq!(
            known_side_disposition(BeamKind::Hook, None, &[], false),
            KnownSideDisposition::RemoveLocal
        );
        assert_eq!(
            known_side_disposition(BeamKind::Hook, None, &[NativeStemHeadSide::Right], false),
            KnownSideDisposition::Retain
        );
        assert_eq!(
            known_side_disposition(BeamKind::Beam, None, &[], false),
            KnownSideDisposition::Retain
        );
        assert_eq!(
            known_side_disposition(BeamKind::Beam, Some(NativeStemHeadSide::Left), &[], false),
            KnownSideDisposition::RemoveLocal
        );
    }

    #[test]
    fn two_logical_sides_stop_before_competing_hook_removal() {
        assert_eq!(
            known_side_disposition(
                BeamKind::Beam,
                None,
                &[NativeStemHeadSide::Left, NativeStemHeadSide::Right],
                true
            ),
            KnownSideDisposition::AwaitHookRemoval
        );
        assert_eq!(
            known_side_disposition(
                BeamKind::Beam,
                None,
                &[NativeStemHeadSide::Left, NativeStemHeadSide::Right],
                false
            ),
            KnownSideDisposition::Retain
        );
    }

    #[test]
    fn side_stump_contains_uses_registry_structural_alias_not_ref_variant() {
        let seed = NativeStemsBeamStumpRef::Seed {
            kept_ordinal: 1,
            free_glyph_ordinal: 9,
            canonical_glyph_index: 42,
        };
        let built = NativeStemsBeamStumpRef::Built {
            canonical_glyph_index: 42,
        };
        assert_ne!(seed, built);
        assert_eq!(stump_canonical_alias(&seed), stump_canonical_alias(&built));
    }

    #[test]
    fn only_a_prior_line_shift_blocks_frozen_plan_reuse() {
        let reference = v_linker(NativeStemVerticalSide::Bottom);
        let mut state = ReplayState::default();
        assert!(reject_repeated_v(1, &state, reference).is_ok());
        state.invocation_ordinal = 1;
        assert!(reject_repeated_v(1, &state, reference).is_ok());
        state.shifted_v_linkers.push(reference);
        assert!(matches!(
            reject_repeated_v(1, &state, reference),
            Err(
                NativeStemsBeamSchedulerError::RepeatedVAttemptAfterLineDelta {
                    system_id: 1,
                    reference: rejected,
                }
            ) if rejected == reference
        ));
    }

    #[test]
    fn exclusion_creation_order_follows_sig_order_not_raw_source_order() {
        let first = RawExclusionPair {
            creation_ordinal: 0,
            system_id: 1,
            hook: NativeStemsBeamSource::RawBeam(20),
            beam: NativeStemsBeamSource::RawBeam(21),
        };
        let sig_sources = [
            NativeStemsBeamSource::RawBeam(99),
            NativeStemsBeamSource::RawBeam(8),
            NativeStemsBeamSource::RawBeam(9),
            NativeStemsBeamSource::RawBeam(2),
            NativeStemsBeamSource::RawBeam(3),
        ];
        let pair_by_hook = BTreeMap::from([
            (
                NativeStemsBeamSource::RawBeam(2),
                NativeStemsBeamSource::RawBeam(3),
            ),
            (
                NativeStemsBeamSource::RawBeam(8),
                NativeStemsBeamSource::RawBeam(9),
            ),
        ]);
        let mut pairs = vec![first];
        push_sig_ordered_pairs(2, &sig_sources, &pair_by_hook, &mut pairs)
            .expect("valid adjacent pair topology");
        assert_eq!(
            pairs,
            vec![
                first,
                RawExclusionPair {
                    creation_ordinal: 1,
                    system_id: 2,
                    hook: NativeStemsBeamSource::RawBeam(8),
                    beam: NativeStemsBeamSource::RawBeam(9),
                },
                RawExclusionPair {
                    creation_ordinal: 2,
                    system_id: 2,
                    hook: NativeStemsBeamSource::RawBeam(2),
                    beam: NativeStemsBeamSource::RawBeam(3),
                },
            ]
        );
    }

    #[test]
    fn exclusion_creation_order_rejects_nonadjacent_pair_endpoints() {
        let hook = NativeStemsBeamSource::RawBeam(8);
        let beam = NativeStemsBeamSource::RawBeam(9);
        let sig_sources = [hook, NativeStemsBeamSource::RawBeam(99), beam];
        let pair_by_hook = BTreeMap::from([(hook, beam)]);
        assert!(matches!(
            push_sig_ordered_pairs(3, &sig_sources, &pair_by_hook, &mut Vec::new()),
            Err(
                NativeStemsBeamSchedulerError::UnsupportedExclusionTopology {
                    system_id: 3,
                    source,
                }
            ) if source == hook
        ));
    }

    #[test]
    fn side_precheck_counts_only_final_target_items() {
        use crate::{
            native_heads_staff_epilog::NativeHeadStaffEpilogRef,
            native_stems_beam_builders::{
                NativeStemsBeamBuilderItem, NativeStemsBeamBuilderItemKind,
            },
            native_stems_beam_reachability::NativeStemsBeamHeadCornerRef,
        };

        let line = NativeStemLine {
            start: crate::stems_step::NativeStemPoint { x: 0.0, y: 0.0 },
            stop: crate::stems_step::NativeStemPoint { x: 0.0, y: 1.0 },
        };
        let item = |target| NativeStemsBeamBuilderItem {
            kind: NativeStemsBeamBuilderItemKind::Gap,
            glyph: None,
            target,
            reference_point: None,
            head_bounds: None,
            line,
            contribution: 0,
        };
        let retained_candidate =
            NativeStemsBeamBuilderTargetRef::Beam(NativeStemsBeamBLinkerRef { beam: RAW, id: 1 });
        // A retained inspection candidate can disappear from the final item
        // list. Java's BLinker precheck sees zero in that case.
        assert_eq!(final_target_item_counts(&[item(None)]), (0, 0));
        assert!(matches!(
            retained_candidate,
            NativeStemsBeamBuilderTargetRef::Beam(_)
        ));

        let head = NativeStemsBeamBuilderTargetRef::Head(NativeStemsBeamHeadCornerRef {
            head: NativeHeadStaffEpilogRef {
                staff_index: 0,
                head_index: 0,
            },
            sig_ordinal: 0,
            x_ordinal: 0,
            horizontal: NativeStemHeadSide::Left,
            vertical: NativeStemVerticalSide::Top,
        });
        assert_eq!(
            final_target_item_counts(&[
                item(Some(retained_candidate)),
                item(None),
                item(Some(head)),
            ]),
            (2, 1)
        );
        assert!(head_target_outcome_consistent(
            0,
            NativeStemsBeamLinkPlanOutcome::NoHeadTarget
        ));
        assert!(!head_target_outcome_consistent(
            0,
            NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem
        ));
        assert!(!head_target_outcome_consistent(
            1,
            NativeStemsBeamLinkPlanOutcome::NoHeadTarget
        ));
        assert!(head_target_outcome_consistent(
            1,
            NativeStemsBeamLinkPlanOutcome::ExpandFailed
        ));
    }
}
