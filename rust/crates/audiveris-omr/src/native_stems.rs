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
    native_headers::NativeHeaderRecognition,
    native_heads::NativeHeadsRecognition,
    native_ledgers::NativeLedgerRecognition,
    native_sig::{NativeSigRecognition, assemble_native_sig},
    native_stem_seeds::NativeStemSeedRecognition,
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
        NativeStemsBeamSchedulerRecognition, materialize_native_stems_beam_scheduler_frontiers,
    },
    native_stems_beam_stumps::{
        NativeStemsBeamStumpRecognition, materialize_native_stems_beam_stumps,
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
};

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
    Ok(NativeStemsPreparedRecognition { components, sig })
}
