// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production composition boundary for the native STEMS stage.
//!
//! The individual read-only STEMS products were originally composed only by
//! an integration-test helper. This module owns that Java-order composition
//! in production code. It deliberately stops before the first mutating SIDES
//! transaction: later boundaries can replace the remaining identity
//! authorities without rebuilding or fixture-hydrating the predecessor.

use std::{error::Error, fmt};

use crate::{
    beam_inters::MIN_INTER_GRADE,
    native_headers::NativeHeaderRecognition,
    native_heads::NativeHeadsRecognition,
    native_ledgers::NativeLedgerRecognition,
    native_sig::{NativeSigRecognition, assemble_native_sig},
    native_stem_seeds::STEM_SEEDS_BELT_MARGIN_RATIO,
    native_stem_seeds::{NativeStemSeedGlyph, NativeStemSeedRecognition},
    native_stems_beam_builders::{
        NativeStemsBeamBuilderRecognition, materialize_native_stems_beam_builders,
    },
    native_stems_beam_link_plans::{
        NativeStemsBeamLinkPlanRecognition, materialize_native_stems_beam_link_plans,
    },
    native_stems_beam_reachability::{
        NativeStemsBeamReachabilityRecognition, materialize_native_stems_beam_reachability,
    },
    native_stems_beam_scheduler::{
        NativeStemsBeamSchedulerRecognition, NativeStemsBeamSchedulerStatus,
        NativeStemsBeamSchedulerStumpsContinuation, NativeStemsBeamSchedulerStumpsStatus,
        NativeStemsBeamSchedulerSystem, materialize_native_stems_beam_scheduler_frontiers,
    },
    native_stems_beam_sides::{
        NativeStemsBeamHookRemovalTransaction, NativeStemsBeamSidesCarrier,
        NativeStemsBeamSidesContext, NativeStemsBeamSidesTransaction,
        NativeStemsBeamStumpsTransaction, NativeStemsFinalizeTransaction,
        NativeStemsHeadCLinkTransaction, NativeStemsHeadPhase1Carrier,
        NativeStemsHeadPhase1Continuation,
        advance_native_stems_beam_sides_transaction_from_modeled_registry,
        advance_native_stems_head_c_link_or_no_link,
        advance_native_stems_head_multi_head_created_c_link_allegretto_system1_order79_from_glyphs,
        advance_native_stems_head_multi_head_created_c_link_allegretto_system3_order29_from_glyphs,
        advance_native_stems_head_multi_head_reuse_c_link_allegretto_system1_order65_from_glyphs,
        advance_native_stems_head_multi_head_reuse_c_link_order67_from_glyphs,
        advance_native_stems_head_multi_head_reuse_c_link_order70_from_glyphs,
        advance_native_stems_head_multi_head_reuse_c_link_order73_from_glyphs,
        advance_native_stems_head_multi_head_reuse_c_link_system2_order54_from_glyphs,
        advance_native_stems_head_phase_two_append_c_link_bach_system2_order8,
        advance_native_stems_head_phase_two_append_c_link_bach_system2_order9,
        advance_native_stems_head_phase_two_append_c_link_bach_system3_order3,
        advance_native_stems_head_phase_two_append_c_link_carmen_system3_x0,
        advance_native_stems_head_phase_two_append_c_link_carmen_system3_x1,
        advance_native_stems_head_phase_two_append_c_link_cucaracha_system1_order6,
        advance_native_stems_head_phase_two_append_c_link_cucaracha_system1_order7,
        advance_native_stems_head_phase_two_append_c_link_cucaracha_system1_order8,
        advance_native_stems_head_phase_two_append_c_link_cucaracha_system1_order10,
        advance_native_stems_head_phase_two_append_c_link_cucaracha_system1_order17,
        advance_native_stems_head_phase_two_append_c_link_cucaracha_system1_order19,
        advance_native_stems_head_phase_two_append_c_link_cucaracha_system1_order20,
        advance_native_stems_head_phase_two_append_c_link_cucaracha_system1_order22,
        advance_native_stems_head_phase_two_append_c_link_cucaracha_system2_order8,
        advance_native_stems_head_phase_two_append_c_link_cucaracha_system2_order9,
        advance_native_stems_head_phase_two_append_c_link_cucaracha_system2_order10,
        advance_native_stems_head_phase_two_append_c_link_cucaracha_system2_order16,
        advance_native_stems_head_phase_two_append_c_link_cucaracha_system3_order19,
        advance_native_stems_head_phase_two_append_c_link_hove_system5_order1,
        advance_native_stems_head_phase_two_append_retry, begin_native_stems_head_linking_phase1,
        continue_native_stems_beam_sides_carrier_into_stumps,
        continue_native_stems_head_linking_phase1,
        drive_native_stems_beam_stumps_from_modeled_registry, finalize_native_stems,
        initialize_native_stems_beam_serial_sides_carrier_from_modeled_registry,
        initialize_native_stems_beam_sides_carrier_from_modeled_registry,
        remove_native_stems_beam_competing_hook_and_resume,
    },
    native_stems_beam_stumps::{
        NativeStemsBeamStumpRecognition, materialize_native_stems_beam_stumps,
    },
    native_stems_beam_vlink_base_apply::NativeStemsBeamSheetEditState,
    native_stems_beam_vlink_transaction::{
        NativeStemsBeamStemCheckerContext, NativeStemsBeamVLinkTransactionState,
        NativeStemsModeledGlyphRegistry,
    },
    native_stems_beam_vlinkers::{
        NativeStemsBeamVLinkerRecognition, materialize_native_stems_beam_vlinkers,
    },
    native_stems_head_builders::{
        NativeStemsHeadBuilderRecognition, materialize_native_stems_head_builders,
    },
    native_stems_head_corner_reachability::{
        NativeStemsHeadCornerReachabilityRecognition,
        materialize_native_stems_head_corner_reachability,
    },
    native_stems_head_corners::{
        NativeStemsHeadCornerRecognition, materialize_native_stems_head_corners,
    },
    native_stems_head_seeds::{
        NativeStemsHeadSeedRecognition, materialize_native_stems_head_seeds,
    },
    native_stems_head_stumps::{
        NativeStemsHeadStumpRecognition, materialize_native_stems_head_stumps,
    },
    recognize::{GridLinesRecognition, NativeBeamRecognition},
    stem_seeds_step::NativeStemCheckerParameters,
};

/// Java `StemsRetriever.Constants.artificialStemGrade`.
const ARTIFICIAL_STEM_GRADE: f64 = 0.4;

/// Complete immutable read-only STEMS construction products.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsComponentRecognition {
    pub inspect_profile: i32,
    /// Accepted system free glyphs retained for head-origin C-link mutation.
    pub stem_seed_glyphs: Vec<NativeStemsStemSeedGlyphSystem>,
    pub head_corners: NativeStemsHeadCornerRecognition,
    pub head_seeds: NativeStemsHeadSeedRecognition,
    pub beam_stumps: NativeStemsBeamStumpRecognition,
    pub beam_vlinkers: NativeStemsBeamVLinkerRecognition,
    pub beam_reachability: NativeStemsBeamReachabilityRecognition,
    pub head_stumps: NativeStemsHeadStumpRecognition,
    pub beam_builders: NativeStemsBeamBuilderRecognition,
    pub head_reachability: NativeStemsHeadCornerReachabilityRecognition,
    pub head_builders: NativeStemsHeadBuilderRecognition,
    pub plans: NativeStemsBeamLinkPlanRecognition,
    pub scheduler: NativeStemsBeamSchedulerRecognition,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsStemSeedGlyphSystem {
    pub system_id: usize,
    pub free_glyphs: Vec<NativeStemSeedGlyph>,
}

/// Complete immutable native state immediately before the first mutating
/// STEMS scheduler transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsPreparedRecognition {
    pub components: NativeStemsComponentRecognition,
    pub sig: NativeSigRecognition,
    /// Page-wide checker state shared by every mutating SIDES/STUMPS frontier.
    pub stem_checker: NativeStemsBeamStemCheckerContext,
}

/// Atomic production start of the sheet's first mutating SIDES pass.
///
/// The registry, carrier, and first transaction are returned together so no
/// caller can mix system-local products or inject a checker configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsSystemSidesStart {
    pub system_id: usize,
    pub registry: NativeStemsModeledGlyphRegistry,
    pub carrier: NativeStemsBeamSidesCarrier,
    pub first_transaction: NativeStemsBeamSidesTransaction,
}

/// Atomic completion of one system SIDES pass.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsSystemSidesDrive {
    pub system_id: usize,
    pub registry: NativeStemsModeledGlyphRegistry,
    pub carrier: NativeStemsBeamSidesCarrier,
    pub transactions: Vec<NativeStemsBeamSidesTransaction>,
    pub hook_removals: Vec<NativeStemsBeamHookRemovalTransaction>,
}

/// Compatibility name for the first production system drive.
pub type NativeStemsFirstSystemSidesDrive = NativeStemsSystemSidesDrive;

/// Atomic SIDES completion for every consecutive system on one page.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsPageSidesDrive {
    pub systems: Vec<NativeStemsSystemSidesDrive>,
}

/// Atomic completion of one system's SIDES and STUMPS beam-origin passes.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsSystemStumpsDrive {
    pub system_id: usize,
    pub registry: NativeStemsModeledGlyphRegistry,
    pub carrier: NativeStemsBeamSidesCarrier,
    pub hook_removals: Vec<NativeStemsBeamHookRemovalTransaction>,
    pub continuation: NativeStemsBeamSchedulerStumpsContinuation,
    pub transactions: Vec<NativeStemsBeamStumpsTransaction>,
}

/// Atomic SIDES+STUMPS completion for every consecutive system on one page.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsPageStumpsDrive {
    pub systems: Vec<NativeStemsSystemStumpsDrive>,
}

/// Production-owned first head-linking frontier for one completed system.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsSystemHeadPhase1Start {
    pub system_id: usize,
    pub registry: NativeStemsModeledGlyphRegistry,
    pub carrier: NativeStemsHeadPhase1Carrier,
}

/// Atomic page transfer from all post-STUMPS carriers into head phase 1.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsPageHeadPhase1Start {
    pub systems: Vec<NativeStemsSystemHeadPhase1Start>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsSystemHeadPhase1FirstOutcome {
    Linked(Box<NativeStemsHeadCLinkTransaction>),
    MutatedUnlinked(Box<NativeStemsHeadCLinkTransaction>),
    Unlinked(NativeStemsHeadPhase1Continuation),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsSystemHeadPhase1FirstAdvance {
    pub system_id: usize,
    pub registry: NativeStemsModeledGlyphRegistry,
    pub carrier: NativeStemsHeadPhase1Carrier,
    pub outcome: NativeStemsSystemHeadPhase1FirstOutcome,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsPageHeadPhase1FirstAdvance {
    pub systems: Vec<NativeStemsSystemHeadPhase1FirstAdvance>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadPhase1ProgressStatus {
    AwaitingFrontier,
    Phase1Complete,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsSystemHeadPhase1Progress {
    pub system_id: usize,
    pub registry: NativeStemsModeledGlyphRegistry,
    pub carrier: NativeStemsHeadPhase1Carrier,
    pub first_outcome: NativeStemsSystemHeadPhase1FirstOutcome,
    pub continuations: Vec<NativeStemsHeadPhase1Continuation>,
    pub status: NativeStemsHeadPhase1ProgressStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsPageHeadPhase1Progress {
    pub systems: Vec<NativeStemsSystemHeadPhase1Progress>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsSystemHeadPhase1NextAdvance {
    pub system_id: usize,
    pub registry: NativeStemsModeledGlyphRegistry,
    pub carrier: NativeStemsHeadPhase1Carrier,
    pub prior_continuations: Vec<NativeStemsHeadPhase1Continuation>,
    pub outcome: NativeStemsSystemHeadPhase1FirstOutcome,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsPageHeadPhase1NextAdvance {
    pub systems: Vec<NativeStemsSystemHeadPhase1NextAdvance>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsHeadPhase1DriveEvent {
    Continuation(NativeStemsHeadPhase1Continuation),
    Linked(Box<NativeStemsHeadCLinkTransaction>),
    MutatedUnlinked(Box<NativeStemsHeadCLinkTransaction>),
    Unlinked(NativeStemsHeadPhase1Continuation),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsSystemHeadPhase1Drive {
    pub system_id: usize,
    pub registry: NativeStemsModeledGlyphRegistry,
    pub carrier: NativeStemsHeadPhase1Carrier,
    pub events: Vec<NativeStemsHeadPhase1DriveEvent>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsPageHeadPhase1Drive {
    pub systems: Vec<NativeStemsSystemHeadPhase1Drive>,
}

/// Atomic completion of one system's phase-1 head queue and carried phase-2
/// append-retry queue.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsSystemHeadPhase2Drive {
    pub system_id: usize,
    pub registry: NativeStemsModeledGlyphRegistry,
    pub carrier: NativeStemsHeadPhase1Carrier,
    pub phase_one_events: Vec<NativeStemsHeadPhase1DriveEvent>,
    pub retries: Vec<NativeStemsHeadPhase1Continuation>,
}

/// Atomic page-wide completion of Java's two head-linking phases.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsPageHeadPhase2Drive {
    pub systems: Vec<NativeStemsSystemHeadPhase2Drive>,
}

/// Atomic generic `finalizeStems` result for one page system.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsSystemFinalizeDrive {
    pub system_id: usize,
    pub registry: NativeStemsModeledGlyphRegistry,
    pub phase_one_events: Vec<NativeStemsHeadPhase1DriveEvent>,
    pub retries: Vec<NativeStemsHeadPhase1Continuation>,
    pub transaction: NativeStemsFinalizeTransaction,
}

/// Atomic page-wide completion through Java's generic `finalizeStems` pass.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsPageFinalizeDrive {
    pub systems: Vec<NativeStemsSystemFinalizeDrive>,
}

/// Complete owned native STEMS result after every page system has finalized.
///
/// Construction products remain available for diagnostics and later
/// publication, while `systems` owns each system's terminal SIG, registry,
/// head-phase trace, retries, and generic finalization transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsRecognition {
    pub components: NativeStemsComponentRecognition,
    pub systems: Vec<NativeStemsSystemFinalizeDrive>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsPreparationError {
    pub phase: &'static str,
    pub message: String,
}

impl fmt::Display for NativeStemsPreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "native STEMS {} failed: {}",
            self.phase, self.message
        )
    }
}

impl Error for NativeStemsPreparationError {}

fn phase(error: impl fmt::Display, phase: &'static str) -> NativeStemsPreparationError {
    NativeStemsPreparationError {
        phase,
        message: error.to_string(),
    }
}

fn system<'a, T>(
    systems: &'a [T],
    system_id: usize,
    id: impl Fn(&T) -> usize,
    phase_name: &'static str,
) -> Result<&'a T, NativeStemsPreparationError> {
    systems
        .iter()
        .find(|system| id(system) == system_id)
        .ok_or_else(|| phase(format!("system {system_id} is absent"), phase_name))
}

impl NativeStemsPreparedRecognition {
    fn sides_context(
        &self,
        system_id: usize,
    ) -> Result<NativeStemsBeamSidesContext<'_>, NativeStemsPreparationError> {
        let components = &self.components;
        Ok(NativeStemsBeamSidesContext {
            plans: system(
                &components.plans.systems,
                system_id,
                |system| system.system_id,
                "SIDES plans",
            )?,
            builders: system(
                &components.beam_builders.systems,
                system_id,
                |system| system.system_id,
                "SIDES builders",
            )?,
            stumps: system(
                &components.beam_stumps.systems,
                system_id,
                |system| system.system_id,
                "SIDES stumps",
            )?,
            vlinkers: system(
                &components.beam_vlinkers.systems,
                system_id,
                |system| system.system_id,
                "SIDES VLinkers",
            )?,
            reachability: system(
                &components.beam_reachability.systems,
                system_id,
                |system| system.system_id,
                "SIDES reachability",
            )?,
            head_corners: system(
                &components.head_corners.systems,
                system_id,
                |system| system.system_id,
                "SIDES head corners",
            )?,
            checker: &self.stem_checker,
        })
    }

    /// Initialize and execute system 1's first SIDES transaction from
    /// production-owned products only. Later systems must start from the
    /// shared allocator/registry state committed by every preceding system;
    /// they are deliberately not reconstructed in isolation here.
    pub fn initialize_first_system_sides(
        &self,
    ) -> Result<NativeStemsSystemSidesStart, NativeStemsPreparationError> {
        let components = &self.components;
        let scheduler: &NativeStemsBeamSchedulerSystem = components
            .scheduler
            .systems
            .first()
            .ok_or_else(|| phase("first system is absent", "SIDES scheduler"))?;
        let system_id = scheduler.system_id;
        if system_id != 1 {
            return Err(phase(
                format!("first system id is {system_id}, expected 1"),
                "SIDES scheduler",
            ));
        }
        let context = self.sides_context(system_id)?;
        let sig = system(
            &self.sig.systems,
            system_id,
            |system| system.system_id,
            "SIDES SIG",
        )?;
        let bindings = system(
            &self.sig.bindings,
            system_id,
            |bindings| bindings.system_id,
            "SIDES bindings",
        )?;
        let registry = NativeStemsModeledGlyphRegistry::from_head_builder_recognition(
            system_id,
            &components.head_builders,
        )
        .map_err(|error| phase(error, "SIDES modeled registry"))?;
        let (carrier, first_transaction) =
            initialize_native_stems_beam_sides_carrier_from_modeled_registry(
                scheduler, sig, bindings, context, &registry,
            )
            .map_err(|error| phase(error, "SIDES first transaction"))?;
        Ok(NativeStemsSystemSidesStart {
            system_id,
            registry,
            carrier,
            first_transaction,
        })
    }

    /// Drive system 1 from its first frontier through the true SIDES terminal.
    ///
    /// The immutable builder count is a strict progress bound. Competing-hook
    /// checkpoints execute the same atomic native removal transaction used by
    /// the focused gate; any other non-SIDES terminal rejects the whole drive.
    pub fn drive_first_system_sides(
        &self,
    ) -> Result<NativeStemsFirstSystemSidesDrive, NativeStemsPreparationError> {
        let start = self.initialize_first_system_sides()?;
        self.drive_system_sides_start(start)
    }

    fn drive_system_sides_start(
        &self,
        start: NativeStemsSystemSidesStart,
    ) -> Result<NativeStemsSystemSidesDrive, NativeStemsPreparationError> {
        let NativeStemsSystemSidesStart {
            system_id,
            registry,
            mut carrier,
            first_transaction,
        } = start;
        let context = self.sides_context(system_id)?;
        let transaction_limit = context.builders.builders.len();
        if transaction_limit == 0 {
            return Err(phase(
                format!("system {system_id} has no builders"),
                "SIDES drive",
            ));
        }
        let mut transactions = vec![first_transaction];
        let mut hook_removals = Vec::new();
        loop {
            match &carrier.scheduler.status {
                NativeStemsBeamSchedulerStatus::SidesExhausted { .. } => {
                    return Ok(NativeStemsSystemSidesDrive {
                        system_id,
                        registry,
                        carrier,
                        transactions,
                        hook_removals,
                    });
                }
                NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(_) => {}
                NativeStemsBeamSchedulerStatus::AwaitingHookRemovalTransaction(_) => {
                    let removal =
                        remove_native_stems_beam_competing_hook_and_resume(&mut carrier, context)
                            .map_err(|error| {
                            phase(
                                format!(
                                    "system {system_id} hook removal {}: {error}",
                                    hook_removals.len() + 1
                                ),
                                "SIDES hook removal",
                            )
                        })?;
                    hook_removals.push(removal);
                    continue;
                }
                NativeStemsBeamSchedulerStatus::Completed { .. } => {
                    return Err(phase(
                        format!("system {system_id} reached STUMPS completion during SIDES"),
                        "SIDES drive",
                    ));
                }
            }
            if transactions.len() >= transaction_limit {
                return Err(phase(
                    format!(
                        "system {system_id} exceeded its {transaction_limit}-builder progress bound"
                    ),
                    "SIDES drive",
                ));
            }
            let transaction = advance_native_stems_beam_sides_transaction_from_modeled_registry(
                &mut carrier,
                context,
                &registry,
            )
            .map_err(|error| {
                phase(
                    format!(
                        "system {system_id} transaction {}: {error}",
                        transactions.len() + 1
                    ),
                    "SIDES drive transaction",
                )
            })?;
            transactions.push(transaction);
        }
    }

    fn initialize_next_system_sides(
        &self,
        completed: &NativeStemsSystemSidesDrive,
    ) -> Result<NativeStemsSystemSidesStart, NativeStemsPreparationError> {
        self.initialize_next_system_sides_from_carried(
            &completed.registry,
            &completed.carrier.latest_base_apply.transaction_state,
            completed.carrier.latest_base_apply.sheet_edit,
        )
    }

    fn initialize_next_system_sides_from_carried(
        &self,
        completed_registry: &NativeStemsModeledGlyphRegistry,
        completed_state: &NativeStemsBeamVLinkTransactionState,
        sheet_edit: NativeStemsBeamSheetEditState,
    ) -> Result<NativeStemsSystemSidesStart, NativeStemsPreparationError> {
        let registry = completed_registry
            .carry_into_next_system(completed_state, &self.components.head_builders)
            .map_err(|error| phase(error, "SIDES cross-system registry"))?;
        let system_id = registry.system_id();
        let scheduler = system(
            &self.components.scheduler.systems,
            system_id,
            |system| system.system_id,
            "SIDES scheduler",
        )?;
        let context = self.sides_context(system_id)?;
        let sig = system(
            &self.sig.systems,
            system_id,
            |system| system.system_id,
            "SIDES SIG",
        )?;
        let bindings = system(
            &self.sig.bindings,
            system_id,
            |bindings| bindings.system_id,
            "SIDES bindings",
        )?;
        let (carrier, first_transaction) =
            initialize_native_stems_beam_serial_sides_carrier_from_modeled_registry(
                scheduler, sig, bindings, context, &registry, sheet_edit,
            )
            .map_err(|error| phase(error, "SIDES next-system transaction"))?;
        Ok(NativeStemsSystemSidesStart {
            system_id,
            registry,
            carrier,
            first_transaction,
        })
    }

    /// Drive system 1 to its SIDES terminal and atomically enter system 2's
    /// first shared-sheet SIDES transaction.
    ///
    /// System 2 receives fresh system-local SIG/binding/linker authorities,
    /// while the exact page registry, allocator, and sheet edit state come
    /// from the completed first system. No isolated system-2 reconstruction is
    /// accepted by this production entry point.
    pub fn initialize_second_system_sides(
        &self,
    ) -> Result<NativeStemsSystemSidesStart, NativeStemsPreparationError> {
        let first = self.drive_first_system_sides()?;
        self.initialize_next_system_sides(&first)
    }

    /// Drive Batuque's second system from its serial first frontier to a true
    /// SIDES terminal. Any competing-hook checkpoint remains fail-closed until
    /// its removal transaction is carried by production state.
    pub fn drive_second_system_sides(
        &self,
    ) -> Result<NativeStemsSystemSidesDrive, NativeStemsPreparationError> {
        let start = self.initialize_second_system_sides()?;
        self.drive_system_sides_start(start)
    }

    /// Drive every consecutive page system through its true SIDES terminal.
    ///
    /// Each later system is initialized only from the preceding committed
    /// drive's exact registry, allocator, and edit state. The complete vector
    /// is returned only when all scheduler systems finish; a later failure
    /// exposes no partial page drive.
    pub fn drive_all_system_sides(
        &self,
    ) -> Result<NativeStemsPageSidesDrive, NativeStemsPreparationError> {
        let system_count = self.components.scheduler.systems.len();
        if system_count == 0 {
            return Err(phase("page has no scheduler systems", "SIDES page drive"));
        }
        let mut systems = Vec::with_capacity(system_count);
        systems.push(self.drive_first_system_sides()?);
        while systems.len() < system_count {
            let start = self.initialize_next_system_sides(
                systems
                    .last()
                    .expect("nonempty after first system SIDES drive"),
            )?;
            let drive = self.drive_system_sides_start(start)?;
            let expected_system_id = systems.len() + 1;
            if drive.system_id != expected_system_id {
                return Err(phase(
                    format!(
                        "system {} followed {}, expected {expected_system_id}",
                        drive.system_id,
                        systems
                            .last()
                            .expect("nonempty after first system SIDES drive")
                            .system_id
                    ),
                    "SIDES page drive",
                ));
            }
            systems.push(drive);
        }
        Ok(NativeStemsPageSidesDrive { systems })
    }

    /// Drive system 1 through its complete STUMPS worklist after SIDES.
    ///
    /// This intentionally precedes rebuilding system 2 so every STUMPS glyph
    /// and StemInter allocation can participate in the next system's shared
    /// page identity seed.
    pub fn drive_first_system_stumps(
        &self,
    ) -> Result<NativeStemsSystemStumpsDrive, NativeStemsPreparationError> {
        let sides = self.drive_first_system_sides()?;
        self.drive_system_stumps_from_sides(sides)
    }

    fn drive_system_stumps_start(
        &self,
        start: NativeStemsSystemSidesStart,
    ) -> Result<NativeStemsSystemStumpsDrive, NativeStemsPreparationError> {
        let sides = self.drive_system_sides_start(start)?;
        self.drive_system_stumps_from_sides(sides)
    }

    fn drive_system_stumps_from_sides(
        &self,
        sides: NativeStemsSystemSidesDrive,
    ) -> Result<NativeStemsSystemStumpsDrive, NativeStemsPreparationError> {
        let NativeStemsSystemSidesDrive {
            system_id,
            registry,
            mut carrier,
            hook_removals,
            ..
        } = sides;
        let context = self.sides_context(system_id)?;
        let continuation =
            continue_native_stems_beam_sides_carrier_into_stumps(&mut carrier, context)
                .map_err(|error| phase(error, "STUMPS scheduler continuation"))?;
        let transaction_limit = context.builders.builders.len();
        let drive = drive_native_stems_beam_stumps_from_modeled_registry(
            &mut carrier,
            context,
            &registry,
            transaction_limit,
        )
        .map_err(|error| phase(error, "STUMPS drive"))?;
        if !matches!(
            drive.status,
            NativeStemsBeamSchedulerStumpsStatus::Completed { .. }
        ) {
            return Err(phase(
                format!(
                    "system {system_id} did not complete within its {transaction_limit}-builder bound"
                ),
                "STUMPS drive",
            ));
        }
        Ok(NativeStemsSystemStumpsDrive {
            system_id,
            registry,
            carrier,
            hook_removals,
            continuation,
            transactions: drive.transactions,
        })
    }

    /// Initialize the next system only after every prior STUMPS mutation has
    /// joined the shared page registry, allocator, and edit state.
    pub fn initialize_next_system_sides_after_stumps(
        &self,
        completed: &NativeStemsSystemStumpsDrive,
    ) -> Result<NativeStemsSystemSidesStart, NativeStemsPreparationError> {
        self.initialize_next_system_sides_from_carried(
            &completed.registry,
            &completed.carrier.latest_base_apply.transaction_state,
            completed.carrier.latest_base_apply.sheet_edit,
        )
    }

    /// Drive every page system through both beam-origin passes in serial Java
    /// order. No later system can observe the earlier SIDES-only allocator.
    pub fn drive_all_system_stumps(
        &self,
    ) -> Result<NativeStemsPageStumpsDrive, NativeStemsPreparationError> {
        let system_count = self.components.scheduler.systems.len();
        if system_count == 0 {
            return Err(phase("page has no scheduler systems", "STUMPS page drive"));
        }
        let mut systems = Vec::with_capacity(system_count);
        systems.push(self.drive_first_system_stumps()?);
        while systems.len() < system_count {
            let start = self.initialize_next_system_sides_after_stumps(
                systems
                    .last()
                    .expect("nonempty after first system STUMPS drive"),
            )?;
            let expected_system_id = systems.len() + 1;
            let drive = self.drive_system_stumps_start(start)?;
            if drive.system_id != expected_system_id {
                return Err(phase(
                    format!(
                        "system {} followed {}, expected {expected_system_id}",
                        drive.system_id,
                        systems
                            .last()
                            .expect("nonempty after first system STUMPS drive")
                            .system_id
                    ),
                    "STUMPS page drive",
                ));
            }
            systems.push(drive);
        }
        Ok(NativeStemsPageStumpsDrive { systems })
    }

    /// Transfer every completed page system into its first phase-1 head
    /// frontier. A failure in any system exposes no partial page vector.
    pub fn begin_all_system_head_linking_phase1(
        &self,
    ) -> Result<NativeStemsPageHeadPhase1Start, NativeStemsPreparationError> {
        let stumps = self.drive_all_system_stumps()?;
        let mut systems = Vec::with_capacity(stumps.systems.len());
        for completed in stumps.systems {
            let system_id = completed.system_id;
            let head_corners = system(
                &self.components.head_corners.systems,
                system_id,
                |system| system.system_id,
                "HEADS phase-1 corners",
            )?;
            let head_builders = system(
                &self.components.head_builders.systems,
                system_id,
                |system| system.system_id,
                "HEADS phase-1 builders",
            )?;
            let head_reachability = system(
                &self.components.head_reachability.systems,
                system_id,
                |system| system.system_id,
                "HEADS phase-1 reachability",
            )?;
            let plans = system(
                &self.components.plans.systems,
                system_id,
                |system| system.system_id,
                "HEADS phase-1 plans",
            )?;
            let carrier = begin_native_stems_head_linking_phase1(
                &completed.carrier,
                head_corners,
                head_reachability,
                head_builders,
                plans,
            )
            .map_err(|error| {
                phase(
                    format!("system {system_id}: {error}"),
                    "HEADS phase-1 page transfer",
                )
            })?;
            systems.push(NativeStemsSystemHeadPhase1Start {
                system_id,
                registry: completed.registry,
                carrier,
            });
        }
        Ok(NativeStemsPageHeadPhase1Start { systems })
    }

    /// Consume every system's first production-carried phase-1 frontier.
    ///
    /// Successful bounded C-links mutate the native SIG and allocator. A
    /// normal Java `link()` rejection closes the head and queues it for phase
    /// 2 without graph mutation. The page vector remains atomic.
    pub fn advance_all_system_first_head_frontiers(
        &self,
    ) -> Result<NativeStemsPageHeadPhase1FirstAdvance, NativeStemsPreparationError> {
        let starts = self.begin_all_system_head_linking_phase1()?;
        let mut systems = Vec::with_capacity(starts.systems.len());
        for start in starts.systems {
            let NativeStemsSystemHeadPhase1Start {
                system_id,
                registry,
                mut carrier,
            } = start;
            let head_corners = system(
                &self.components.head_corners.systems,
                system_id,
                |system| system.system_id,
                "HEADS first C-link corners",
            )?;
            let head_reachability = system(
                &self.components.head_reachability.systems,
                system_id,
                |system| system.system_id,
                "HEADS first C-link reachability",
            )?;
            let seed_glyphs = system(
                &self.components.stem_seed_glyphs,
                system_id,
                |system| system.system_id,
                "HEADS first C-link stem seeds",
            )?;
            let head_builders = system(
                &self.components.head_builders.systems,
                system_id,
                |system| system.system_id,
                "HEADS first C-link builders",
            )?;
            let plans = system(
                &self.components.plans.systems,
                system_id,
                |system| system.system_id,
                "HEADS first C-link plans",
            )?;
            let vlinkers = system(
                &self.components.beam_vlinkers.systems,
                system_id,
                |system| system.system_id,
                "HEADS first C-link V-linkers",
            )?;
            let outcome = advance_native_stems_head_c_link_or_no_link(
                &mut carrier,
                head_corners,
                head_reachability,
                &seed_glyphs.free_glyphs,
                head_builders,
                plans,
                vlinkers,
                &self.stem_checker,
                &registry,
            )
            .map_err(|error| {
                phase(
                    format!("system {system_id}: {error}"),
                    "HEADS first C-link page drive",
                )
            })?;
            let outcome = match outcome {
                Ok(transaction) if transaction.returned_linked => {
                    NativeStemsSystemHeadPhase1FirstOutcome::Linked(Box::new(transaction))
                }
                Ok(transaction) => {
                    NativeStemsSystemHeadPhase1FirstOutcome::MutatedUnlinked(Box::new(transaction))
                }
                Err(continuation) => {
                    NativeStemsSystemHeadPhase1FirstOutcome::Unlinked(continuation)
                }
            };
            systems.push(NativeStemsSystemHeadPhase1FirstAdvance {
                system_id,
                registry,
                carrier,
                outcome,
            });
        }
        Ok(NativeStemsPageHeadPhase1FirstAdvance { systems })
    }

    /// Carry each system after its first mixed outcome through ordinary
    /// prelinked/undefined continuations until the next C-link frontier or a
    /// true phase-1 queue terminal.
    pub fn continue_all_system_heads_to_next_frontier(
        &self,
    ) -> Result<NativeStemsPageHeadPhase1Progress, NativeStemsPreparationError> {
        let first_page = self.advance_all_system_first_head_frontiers()?;
        let mut systems = Vec::with_capacity(first_page.systems.len());
        for first in first_page.systems {
            let NativeStemsSystemHeadPhase1FirstAdvance {
                system_id,
                registry,
                mut carrier,
                outcome,
            } = first;
            let head_corners = system(
                &self.components.head_corners.systems,
                system_id,
                |system| system.system_id,
                "HEADS continuation corners",
            )?;
            let head_reachability = system(
                &self.components.head_reachability.systems,
                system_id,
                |system| system.system_id,
                "HEADS continuation reachability",
            )?;
            let head_builders = system(
                &self.components.head_builders.systems,
                system_id,
                |system| system.system_id,
                "HEADS continuation builders",
            )?;
            let plans = system(
                &self.components.plans.systems,
                system_id,
                |system| system.system_id,
                "HEADS continuation plans",
            )?;
            let mut continuations = Vec::new();
            let status = loop {
                if carrier.current_index == carrier.heads.len() {
                    break NativeStemsHeadPhase1ProgressStatus::Phase1Complete;
                }
                let continuation = continue_native_stems_head_linking_phase1(
                    &carrier,
                    head_corners,
                    Some(head_reachability),
                    head_builders,
                    plans,
                )
                .map_err(|error| {
                    phase(
                        format!("system {system_id}: {error}"),
                        "HEADS continuation page drive",
                    )
                })?;
                let awaiting_frontier = continuation.returned_linked.is_none();
                carrier = (*continuation.state_after).clone();
                continuations.push(continuation);
                if awaiting_frontier {
                    break NativeStemsHeadPhase1ProgressStatus::AwaitingFrontier;
                }
            };
            systems.push(NativeStemsSystemHeadPhase1Progress {
                system_id,
                registry,
                carrier,
                first_outcome: outcome,
                continuations,
                status,
            });
        }
        Ok(NativeStemsPageHeadPhase1Progress { systems })
    }

    /// Consume each Boundary-157 actionable frontier while preserving carried
    /// unlinked/undefined phase-2 authority.
    pub fn advance_all_system_next_head_frontiers(
        &self,
    ) -> Result<NativeStemsPageHeadPhase1NextAdvance, NativeStemsPreparationError> {
        let progress = self.continue_all_system_heads_to_next_frontier()?;
        let mut systems = Vec::with_capacity(progress.systems.len());
        for system_progress in progress.systems {
            let NativeStemsSystemHeadPhase1Progress {
                system_id,
                registry,
                mut carrier,
                continuations,
                status,
                ..
            } = system_progress;
            if status != NativeStemsHeadPhase1ProgressStatus::AwaitingFrontier {
                return Err(phase(
                    format!("system {system_id} has no next actionable frontier"),
                    "HEADS next C-link page drive",
                ));
            }
            let head_corners = system(
                &self.components.head_corners.systems,
                system_id,
                |system| system.system_id,
                "HEADS next C-link corners",
            )?;
            let head_reachability = system(
                &self.components.head_reachability.systems,
                system_id,
                |system| system.system_id,
                "HEADS next C-link reachability",
            )?;
            let seed_glyphs = system(
                &self.components.stem_seed_glyphs,
                system_id,
                |system| system.system_id,
                "HEADS next C-link stem seeds",
            )?;
            let head_builders = system(
                &self.components.head_builders.systems,
                system_id,
                |system| system.system_id,
                "HEADS next C-link builders",
            )?;
            let plans = system(
                &self.components.plans.systems,
                system_id,
                |system| system.system_id,
                "HEADS next C-link plans",
            )?;
            let vlinkers = system(
                &self.components.beam_vlinkers.systems,
                system_id,
                |system| system.system_id,
                "HEADS next C-link V-linkers",
            )?;
            let outcome = advance_native_stems_head_c_link_or_no_link(
                &mut carrier,
                head_corners,
                head_reachability,
                &seed_glyphs.free_glyphs,
                head_builders,
                plans,
                vlinkers,
                &self.stem_checker,
                &registry,
            )
            .map_err(|error| {
                phase(
                    format!("system {system_id}: {error}"),
                    "HEADS next C-link page drive",
                )
            })?;
            let outcome = match outcome {
                Ok(transaction) if transaction.returned_linked => {
                    NativeStemsSystemHeadPhase1FirstOutcome::Linked(Box::new(transaction))
                }
                Ok(transaction) => {
                    NativeStemsSystemHeadPhase1FirstOutcome::MutatedUnlinked(Box::new(transaction))
                }
                Err(continuation) => {
                    NativeStemsSystemHeadPhase1FirstOutcome::Unlinked(continuation)
                }
            };
            systems.push(NativeStemsSystemHeadPhase1NextAdvance {
                system_id,
                registry,
                carrier,
                prior_continuations: continuations,
                outcome,
            });
        }
        Ok(NativeStemsPageHeadPhase1NextAdvance { systems })
    }

    /// Drive every system through all currently supported phase-1 head
    /// outcomes. The page is returned only at true queue completion; any
    /// unsupported expansion/reuse branch rejects the whole shadow drive.
    pub fn drive_all_system_head_linking_phase1(
        &self,
    ) -> Result<NativeStemsPageHeadPhase1Drive, NativeStemsPreparationError> {
        let starts = self.begin_all_system_head_linking_phase1()?;
        let mut systems = Vec::with_capacity(starts.systems.len());
        for start in starts.systems {
            let NativeStemsSystemHeadPhase1Start {
                system_id,
                registry,
                mut carrier,
            } = start;
            let head_corners = system(
                &self.components.head_corners.systems,
                system_id,
                |system| system.system_id,
                "HEADS phase-1 drive corners",
            )?;
            let head_reachability = system(
                &self.components.head_reachability.systems,
                system_id,
                |system| system.system_id,
                "HEADS phase-1 drive reachability",
            )?;
            let seed_glyphs = system(
                &self.components.stem_seed_glyphs,
                system_id,
                |system| system.system_id,
                "HEADS phase-1 drive stem seeds",
            )?;
            let head_builders = system(
                &self.components.head_builders.systems,
                system_id,
                |system| system.system_id,
                "HEADS phase-1 drive builders",
            )?;
            let plans = system(
                &self.components.plans.systems,
                system_id,
                |system| system.system_id,
                "HEADS phase-1 drive plans",
            )?;
            let vlinkers = system(
                &self.components.beam_vlinkers.systems,
                system_id,
                |system| system.system_id,
                "HEADS phase-1 drive V-linkers",
            )?;
            let mut events = Vec::new();
            while carrier.current_index < carrier.heads.len() {
                if events.len() > carrier.heads.len().saturating_mul(2) {
                    return Err(phase(
                        format!("system {system_id} exceeded its bounded head-event count"),
                        "HEADS phase-1 page drive",
                    ));
                }
                let authenticated_reuse = if carrier.frontier_consumed {
                    let head = &carrier.heads[carrier.current_index];
                    match (
                        carrier.current_index,
                        head.reference.x_ordinal,
                        head.reference.sig_ordinal,
                    ) {
                        (54, 46, 94) => Some(
                            advance_native_stems_head_multi_head_reuse_c_link_system2_order54_from_glyphs(
                                &carrier,
                                head_corners,
                                head_reachability,
                                &seed_glyphs.free_glyphs,
                                head_builders,
                                plans,
                                &self.stem_checker,
                                &registry,
                            ),
                        ),
                        (65, 77, 14) => Some(
                            advance_native_stems_head_multi_head_reuse_c_link_allegretto_system1_order65_from_glyphs(
                                &carrier,
                                head_corners,
                                head_reachability,
                                &seed_glyphs.free_glyphs,
                                head_builders,
                                plans,
                                &self.stem_checker,
                                &registry,
                            ),
                        ),
                        (79, 82, 89) => Some(
                            advance_native_stems_head_multi_head_created_c_link_allegretto_system1_order79_from_glyphs(
                                &carrier,
                                head_corners,
                                head_reachability,
                                &seed_glyphs.free_glyphs,
                                head_builders,
                                plans,
                                &self.stem_checker,
                                &registry,
                            ),
                        ),
                        (29, 114, 76) => Some(
                            advance_native_stems_head_multi_head_created_c_link_allegretto_system3_order29_from_glyphs(
                                &carrier,
                                head_corners,
                                head_reachability,
                                &seed_glyphs.free_glyphs,
                                head_builders,
                                plans,
                                &self.stem_checker,
                                &registry,
                            ),
                        ),
                        (67, 73, 18) => Some(
                            advance_native_stems_head_multi_head_reuse_c_link_order67_from_glyphs(
                                &carrier,
                                head_corners,
                                head_reachability,
                                &seed_glyphs.free_glyphs,
                                head_builders,
                                plans,
                                &self.stem_checker,
                                &registry,
                            ),
                        ),
                        (70, 1, 35) => Some(
                            advance_native_stems_head_multi_head_reuse_c_link_order70_from_glyphs(
                                &carrier,
                                head_corners,
                                head_reachability,
                                &seed_glyphs.free_glyphs,
                                head_builders,
                                plans,
                                &self.stem_checker,
                                &registry,
                            ),
                        ),
                        (73, 75, 96) => Some(
                            advance_native_stems_head_multi_head_reuse_c_link_order73_from_glyphs(
                                &carrier,
                                head_corners,
                                head_reachability,
                                &seed_glyphs.free_glyphs,
                                head_builders,
                                plans,
                                &self.stem_checker,
                                &registry,
                            ),
                        ),
                        _ => None,
                    }
                } else {
                    None
                };
                if let Some(continuation) = authenticated_reuse {
                    let continuation = continuation.map_err(|error| {
                        phase(
                            format!("system {system_id}: {error}"),
                            "HEADS phase-1 page drive",
                        )
                    })?;
                    carrier = (*continuation.state_after).clone();
                    events.push(NativeStemsHeadPhase1DriveEvent::Continuation(continuation));
                    continue;
                }
                if carrier.frontier_consumed {
                    let continuation = continue_native_stems_head_linking_phase1(
                        &carrier,
                        head_corners,
                        Some(head_reachability),
                        head_builders,
                        plans,
                    )
                    .map_err(|error| {
                        phase(
                            format!("system {system_id}: {error}"),
                            "HEADS phase-1 page drive",
                        )
                    })?;
                    carrier = (*continuation.state_after).clone();
                    events.push(NativeStemsHeadPhase1DriveEvent::Continuation(continuation));
                    continue;
                }
                let current_index = carrier.current_index;
                let current_head = carrier.heads[current_index].reference;
                let outcome = advance_native_stems_head_c_link_or_no_link(
                    &mut carrier,
                    head_corners,
                    head_reachability,
                    &seed_glyphs.free_glyphs,
                    head_builders,
                    plans,
                    vlinkers,
                    &self.stem_checker,
                    &registry,
                )
                .map_err(|error| {
                    phase(
                        format!(
                            "system {system_id} queue {current_index} x{}/SIG{}: {error}",
                            current_head.x_ordinal, current_head.sig_ordinal
                        ),
                        "HEADS phase-1 page drive",
                    )
                })?;
                events.push(match outcome {
                    Ok(transaction) if transaction.returned_linked => {
                        NativeStemsHeadPhase1DriveEvent::Linked(Box::new(transaction))
                    }
                    Ok(transaction) => {
                        NativeStemsHeadPhase1DriveEvent::MutatedUnlinked(Box::new(transaction))
                    }
                    Err(continuation) => NativeStemsHeadPhase1DriveEvent::Unlinked(continuation),
                });
            }
            systems.push(NativeStemsSystemHeadPhase1Drive {
                system_id,
                registry,
                carrier,
                events,
            });
        }
        Ok(NativeStemsPageHeadPhase1Drive { systems })
    }

    /// Drive every system through the complete carried phase-1 queue and then
    /// Java's ordered phase-2 append retries.
    ///
    /// The receiver is immutable and every system advances on local shadows,
    /// so an unsupported real `reuseStem` append or malformed queue rejects the
    /// entire page without exposing a partially advanced carrier.
    pub fn drive_all_system_head_linking_phase2(
        &self,
    ) -> Result<NativeStemsPageHeadPhase2Drive, NativeStemsPreparationError> {
        let phase_one = self.drive_all_system_head_linking_phase1()?;
        let mut systems = Vec::with_capacity(phase_one.systems.len());
        for completed in phase_one.systems {
            let NativeStemsSystemHeadPhase1Drive {
                system_id,
                registry,
                mut carrier,
                events,
            } = completed;
            let head_corners = system(
                &self.components.head_corners.systems,
                system_id,
                |system| system.system_id,
                "HEADS phase-2 drive corners",
            )?;
            let head_reachability = system(
                &self.components.head_reachability.systems,
                system_id,
                |system| system.system_id,
                "HEADS phase-2 drive reachability",
            )?;
            let seed_glyphs = system(
                &self.components.stem_seed_glyphs,
                system_id,
                |system| system.system_id,
                "HEADS phase-2 drive stem seeds",
            )?;
            let head_builders = system(
                &self.components.head_builders.systems,
                system_id,
                |system| system.system_id,
                "HEADS phase-2 drive builders",
            )?;
            let plans = system(
                &self.components.plans.systems,
                system_id,
                |system| system.system_id,
                "HEADS phase-2 drive plans",
            )?;
            let queue_len = carrier.unlinked_heads.len();
            let mut retries = Vec::with_capacity(queue_len);
            while carrier.phase_two_index < queue_len {
                if retries.len() >= queue_len {
                    return Err(phase(
                        format!("system {system_id} exceeded its carried phase-2 queue"),
                        "HEADS phase-2 page drive",
                    ));
                }
                let queued_head = carrier.unlinked_heads[carrier.phase_two_index];
                if system_id == 5
                    && carrier.phase_two_index == 1
                    && queued_head.x_ordinal == 67
                    && queued_head.sig_ordinal == 52
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_hove_system5_order1(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                if system_id == 3
                    && carrier.phase_two_index == 19
                    && queued_head.x_ordinal == 37
                    && queued_head.sig_ordinal == 11
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_cucaracha_system3_order19(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                if system_id == 2
                    && carrier.phase_two_index == 16
                    && queued_head.x_ordinal == 109
                    && queued_head.sig_ordinal == 81
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_cucaracha_system2_order16(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                if system_id == 2
                    && carrier.phase_two_index == 10
                    && queued_head.x_ordinal == 84
                    && queued_head.sig_ordinal == 80
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_cucaracha_system2_order10(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                if system_id == 2
                    && carrier.phase_two_index == 9
                    && queued_head.x_ordinal == 149
                    && queued_head.sig_ordinal == 18
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_bach_system2_order9(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    let expected_index = carrier.phase_two_index + 1;
                    if retry.state_after.phase_two_index != expected_index {
                        return Err(phase(
                            format!(
                                "system {system_id} phase-2 C-link did not advance exactly one queue entry"
                            ),
                            "HEADS phase-2 page drive",
                        ));
                    }
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                if system_id == 2
                    && carrier.phase_two_index == 9
                    && queued_head.x_ordinal == 132
                    && queued_head.sig_ordinal == 84
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_cucaracha_system2_order9(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                if system_id == 2
                    && carrier.phase_two_index == 8
                    && queued_head.x_ordinal == 56
                    && queued_head.sig_ordinal == 78
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_cucaracha_system2_order8(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                if system_id == 1
                    && carrier.phase_two_index == 22
                    && queued_head.x_ordinal == 71
                    && queued_head.sig_ordinal == 66
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_cucaracha_system1_order22(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                if system_id == 1
                    && carrier.phase_two_index == 20
                    && queued_head.x_ordinal == 45
                    && queued_head.sig_ordinal == 62
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_cucaracha_system1_order20(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                if system_id == 1
                    && carrier.phase_two_index == 19
                    && queued_head.x_ordinal == 14
                    && queued_head.sig_ordinal == 58
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_cucaracha_system1_order19(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                if system_id == 1
                    && carrier.phase_two_index == 17
                    && queued_head.x_ordinal == 68
                    && queued_head.sig_ordinal == 76
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_cucaracha_system1_order17(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                if system_id == 1
                    && carrier.phase_two_index == 10
                    && queued_head.x_ordinal == 42
                    && queued_head.sig_ordinal == 73
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_cucaracha_system1_order10(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                if system_id == 1
                    && carrier.phase_two_index == 8
                    && queued_head.x_ordinal == 52
                    && queued_head.sig_ordinal == 75
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_cucaracha_system1_order8(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                if system_id == 1
                    && carrier.phase_two_index == 7
                    && queued_head.x_ordinal == 12
                    && queued_head.sig_ordinal == 69
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_cucaracha_system1_order7(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                if system_id == 1
                    && carrier.phase_two_index == 6
                    && queued_head.x_ordinal == 25
                    && queued_head.sig_ordinal == 71
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_cucaracha_system1_order6(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                if system_id == 3
                    && carrier.phase_two_index == 1
                    && queued_head.x_ordinal == 1
                    && queued_head.sig_ordinal == 53
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_carmen_system3_x1(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    let expected_index = carrier.phase_two_index + 1;
                    if retry.state_after.phase_two_index != expected_index {
                        return Err(phase(
                            format!(
                                "system {system_id} phase-2 C-link did not advance exactly one queue entry"
                            ),
                            "HEADS phase-2 page drive",
                        ));
                    }
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                if system_id == 3
                    && carrier.phase_two_index == 3
                    && queued_head.x_ordinal == 0
                    && queued_head.sig_ordinal == 3
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_carmen_system3_x0(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    let expected_index = carrier.phase_two_index + 1;
                    if retry.state_after.phase_two_index != expected_index {
                        return Err(phase(
                            format!(
                                "system {system_id} phase-2 C-link did not advance exactly one queue entry"
                            ),
                            "HEADS phase-2 page drive",
                        ));
                    }
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                if system_id == 2
                    && carrier.phase_two_index == 8
                    && queued_head.x_ordinal == 123
                    && queued_head.sig_ordinal == 14
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_bach_system2_order8(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    let expected_index = carrier.phase_two_index + 1;
                    if retry.state_after.phase_two_index != expected_index {
                        return Err(phase(
                            format!(
                                "system {system_id} phase-2 C-link did not advance exactly one queue entry"
                            ),
                            "HEADS phase-2 page drive",
                        ));
                    }
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                if system_id == 3
                    && carrier.phase_two_index == 3
                    && queued_head.x_ordinal == 96
                    && queued_head.sig_ordinal == 166
                {
                    let transaction =
                        advance_native_stems_head_phase_two_append_c_link_bach_system3_order3(
                            &carrier,
                            head_corners,
                            head_reachability,
                            &seed_glyphs.free_glyphs,
                            head_builders,
                            plans,
                            &self.stem_checker,
                            &registry,
                        )
                        .map_err(|error| {
                            phase(
                                format!("system {system_id}: {error}"),
                                "HEADS phase-2 page drive",
                            )
                        })?;
                    let retry = transaction.continuation;
                    let expected_index = carrier.phase_two_index + 1;
                    if retry.state_after.phase_two_index != expected_index {
                        return Err(phase(
                            format!(
                                "system {system_id} phase-2 C-link did not advance exactly one queue entry"
                            ),
                            "HEADS phase-2 page drive",
                        ));
                    }
                    carrier = (*retry.state_after).clone();
                    retries.push(retry);
                    continue;
                }
                let retry = advance_native_stems_head_phase_two_append_retry(
                    &carrier,
                    head_corners,
                    head_reachability,
                    head_builders,
                    plans,
                )
                .map_err(|error| {
                    phase(
                        format!("system {system_id}: {error}"),
                        "HEADS phase-2 page drive",
                    )
                })?;
                let expected_index = carrier.phase_two_index + 1;
                if retry.state_after.phase_two_index != expected_index {
                    return Err(phase(
                        format!(
                            "system {system_id} phase-2 retry did not advance exactly one queue entry"
                        ),
                        "HEADS phase-2 page drive",
                    ));
                }
                carrier = (*retry.state_after).clone();
                retries.push(retry);
            }
            if carrier.phase_two_index != carrier.unlinked_heads.len() {
                return Err(phase(
                    format!("system {system_id} phase-2 queue is not complete"),
                    "HEADS phase-2 page drive",
                ));
            }
            systems.push(NativeStemsSystemHeadPhase2Drive {
                system_id,
                registry,
                carrier,
                phase_one_events: events,
                retries,
            });
        }
        Ok(NativeStemsPageHeadPhase2Drive { systems })
    }

    /// Drive every system through both head-linking phases and generic
    /// `finalizeStems`, exposing no partial page when any finalizer rejects.
    pub fn finalize_all_system_stems(
        &self,
    ) -> Result<NativeStemsPageFinalizeDrive, NativeStemsPreparationError> {
        let phase_two = self.drive_all_system_head_linking_phase2()?;
        let mut systems = Vec::with_capacity(phase_two.systems.len());
        for completed in phase_two.systems {
            let transaction = finalize_native_stems(&completed.carrier).map_err(|error| {
                phase(
                    format!("system {}: {error}", completed.system_id),
                    "finalizeStems page drive",
                )
            })?;
            systems.push(NativeStemsSystemFinalizeDrive {
                system_id: completed.system_id,
                registry: completed.registry,
                phase_one_events: completed.phase_one_events,
                retries: completed.retries,
                transaction,
            });
        }
        Ok(NativeStemsPageFinalizeDrive { systems })
    }
}

/// Compose every read-only STEMS construction product in Java order.
///
/// This boundary does not require a complete mutable SIG, so it remains useful
/// on wider pages whose upstream BEAMS-group SIG publication is not complete.
pub fn materialize_native_stems_components(
    grid: &GridLinesRecognition,
    stem_seeds: &NativeStemSeedRecognition,
    beams: &NativeBeamRecognition,
    ledgers: &NativeLedgerRecognition,
    heads: &NativeHeadsRecognition,
    inspect_profile: i32,
) -> Result<NativeStemsComponentRecognition, NativeStemsPreparationError> {
    let head_corners = materialize_native_stems_head_corners(heads, stem_seeds)
        .map_err(|error| phase(error, "head corners"))?;
    let head_seeds = materialize_native_stems_head_seeds(grid, stem_seeds, &head_corners)
        .map_err(|error| phase(error, "head seeds"))?;
    let beam_stumps =
        materialize_native_stems_beam_stumps(grid, beams, heads, stem_seeds, &head_seeds)
            .map_err(|error| phase(error, "beam stumps"))?;
    let beam_vlinkers =
        materialize_native_stems_beam_vlinkers(grid, beams, stem_seeds, &beam_stumps)
            .map_err(|error| phase(error, "beam VLinkers"))?;
    let beam_reachability = materialize_native_stems_beam_reachability(
        beams,
        &beam_stumps,
        &beam_vlinkers,
        &head_corners,
    )
    .map_err(|error| phase(error, "beam reachability"))?;
    let head_stumps =
        materialize_native_stems_head_stumps(grid, stem_seeds, &head_corners, &head_seeds)
            .map_err(|error| phase(error, "head stumps"))?;
    let beam_builders = materialize_native_stems_beam_builders(
        grid,
        beams,
        ledgers,
        heads,
        stem_seeds,
        &beam_stumps,
        &beam_vlinkers,
        &head_corners,
        &head_stumps,
        &beam_reachability,
    )
    .map_err(|error| phase(error, "beam builders"))?;
    let head_reachability = materialize_native_stems_head_corner_reachability(
        grid,
        stem_seeds,
        heads,
        &head_corners,
        &head_seeds,
        &head_stumps,
        &beam_stumps,
        &beam_vlinkers,
        &beam_reachability,
    )
    .map_err(|error| phase(error, "head reachability"))?;
    let head_builders = materialize_native_stems_head_builders(
        grid,
        beams,
        ledgers,
        heads,
        stem_seeds,
        &beam_stumps,
        &beam_vlinkers,
        &head_stumps,
        &beam_builders,
        &head_reachability,
        inspect_profile,
    )
    .map_err(|error| phase(error, "head builders"))?;
    let plans = materialize_native_stems_beam_link_plans(
        stem_seeds,
        &beam_stumps,
        &beam_vlinkers,
        &head_corners,
        &head_stumps,
        &beam_reachability,
        &beam_builders,
        &head_builders,
    )
    .map_err(|error| phase(error, "beam link plans"))?;
    let scheduler = materialize_native_stems_beam_scheduler_frontiers(
        beams,
        &beam_stumps,
        &beam_vlinkers,
        &beam_builders,
        &plans,
    )
    .map_err(|error| phase(error, "beam scheduler"))?;

    Ok(NativeStemsComponentRecognition {
        inspect_profile,
        stem_seed_glyphs: stem_seeds
            .systems
            .iter()
            .map(|system| NativeStemsStemSeedGlyphSystem {
                system_id: system.raw.system_id,
                free_glyphs: system.free_glyphs.clone(),
            })
            .collect(),
        head_corners,
        head_seeds,
        beam_stumps,
        beam_vlinkers,
        beam_reachability,
        head_stumps,
        beam_builders,
        head_reachability,
        head_builders,
        plans,
        scheduler,
    })
}

/// Compose every live pre-mutation STEMS product and its mutable SIG in Java
/// order.
///
/// All inputs are completed native products from earlier stages. The function
/// is side-effect free: a failure returns no partially constructed carrier.
pub fn prepare_native_stems(
    grid: &GridLinesRecognition,
    headers: &NativeHeaderRecognition,
    stem_seeds: &NativeStemSeedRecognition,
    beams: &NativeBeamRecognition,
    ledgers: &NativeLedgerRecognition,
    heads: &NativeHeadsRecognition,
    inspect_profile: i32,
) -> Result<NativeStemsPreparedRecognition, NativeStemsPreparationError> {
    let components = materialize_native_stems_components(
        grid,
        stem_seeds,
        beams,
        ledgers,
        heads,
        inspect_profile,
    )?;
    let sig = assemble_native_sig(grid, headers, beams, ledgers, heads)
        .map_err(|error| phase(error, "SIG assembly"))?;
    let interline = grid.scale.scale.interline.main;
    if interline <= 0 || stem_seeds.maximum_stem_thickness <= 0 {
        return Err(phase(
            "non-positive interline or maximum stem thickness",
            "stem checker",
        ));
    }
    let stem_checker = NativeStemsBeamStemCheckerContext {
        no_staff: grid.no_staff.clone(),
        parameters: NativeStemCheckerParameters {
            interline,
            maximum_stem_width: stem_seeds.maximum_stem_thickness,
            belt_margin_dx: (STEM_SEEDS_BELT_MARGIN_RATIO * f64::from(interline)).round_ties_even()
                as i32,
            sheet_skew_slope: grid.global_slope,
        },
        minimum_stem_grade: MIN_INTER_GRADE,
        artificial_stem_grade: ARTIFICIAL_STEM_GRADE,
    };
    Ok(NativeStemsPreparedRecognition {
        components,
        sig,
        stem_checker,
    })
}

/// Run the complete native STEMS step over live upstream recognition products.
///
/// Every fallible predecessor, SIDES/STUMPS transaction, both head-linking
/// phases, and generic `finalizeStems` executes inside this call. Nothing is
/// returned unless every system reaches its finalized terminal, so callers
/// cannot observe or publish a partial page.
pub fn recognize_native_stems(
    grid: &GridLinesRecognition,
    headers: &NativeHeaderRecognition,
    stem_seeds: &NativeStemSeedRecognition,
    beams: &NativeBeamRecognition,
    ledgers: &NativeLedgerRecognition,
    heads: &NativeHeadsRecognition,
    inspect_profile: i32,
) -> Result<NativeStemsRecognition, NativeStemsPreparationError> {
    let prepared = prepare_native_stems(
        grid,
        headers,
        stem_seeds,
        beams,
        ledgers,
        heads,
        inspect_profile,
    )?;
    let finalized = prepared.finalize_all_system_stems()?;
    Ok(NativeStemsRecognition {
        components: prepared.components,
        systems: finalized.systems,
    })
}
