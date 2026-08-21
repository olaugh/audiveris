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
    native_stem_seeds::NativeStemSeedRecognition,
    native_stem_seeds::STEM_SEEDS_BELT_MARGIN_RATIO,
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
        NativeStemsBeamSidesCarrier, NativeStemsBeamSidesContext, NativeStemsBeamSidesTransaction,
        NativeStemsBeamStumpsTransaction, NativeStemsHeadPhase1Carrier,
        advance_native_stems_beam_sides_transaction_from_modeled_registry,
        begin_native_stems_head_linking_phase1,
        continue_native_stems_beam_sides_carrier_into_stumps,
        drive_native_stems_beam_stumps_from_modeled_registry,
        initialize_native_stems_beam_serial_sides_carrier_from_modeled_registry,
        initialize_native_stems_beam_sides_carrier_from_modeled_registry,
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
    /// The immutable builder count is a strict progress bound. A competing-hook
    /// checkpoint or any non-SIDES terminal rejects the whole returned drive;
    /// callers never receive a guessed partial system completion.
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
        loop {
            match &carrier.scheduler.status {
                NativeStemsBeamSchedulerStatus::SidesExhausted { .. } => {
                    return Ok(NativeStemsSystemSidesDrive {
                        system_id,
                        registry,
                        carrier,
                        transactions,
                    });
                }
                NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(_) => {}
                NativeStemsBeamSchedulerStatus::AwaitingHookRemovalTransaction(_) => {
                    return Err(phase(
                        format!("system {system_id} reached a competing-hook checkpoint"),
                        "SIDES drive",
                    ));
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
            let plans = system(
                &self.components.plans.systems,
                system_id,
                |system| system.system_id,
                "HEADS phase-1 plans",
            )?;
            let carrier = begin_native_stems_head_linking_phase1(
                &completed.carrier,
                head_corners,
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
