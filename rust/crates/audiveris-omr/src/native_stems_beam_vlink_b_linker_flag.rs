// SPDX-License-Identifier: AGPL-3.0-or-later

//! The single B-linker flag assignment after the base BeamStem link.
//!
//! This boundary resumes an exact boundary-14
//! `ReadyBeforeBLinkerFlagMutation` product, executes only Java's
//! `getBLinker().setLinked(true)`, and stops before `linkSiblings`.  The flag
//! belongs to the outer `BLinker`; every `VLinker` child observes that same
//! cell.  Head-side `SLinker` flags are a separate identity domain and are not
//! touched here.

use std::{error::Error, fmt};

use crate::{
    native_stems_beam_link_plans::NativeStemsBeamLinkPlanSystem,
    native_stems_beam_scheduler::{NativeStemsBeamSchedulerStatus, NativeStemsBeamSchedulerSystem},
    native_stems_beam_stumps::NativeStemsBeamStumpSystem,
    native_stems_beam_vlink_base_apply::{
        NativeStemsBeamVLinkBaseApplyKey, NativeStemsBeamVLinkBaseApplyOutcome,
        NativeStemsBeamVLinkBaseApplyState, NativeStemsBeamVLinkBaseApplyTransaction,
        apply_native_stems_beam_vlink_base_transaction,
    },
    native_stems_beam_vlink_reuse_check::{
        NativeStemsBeamRelationParameters, NativeStemsBeamVLinkReuseCheck,
        NativeStemsBeamVLinkReuseLiveState,
    },
    native_stems_beam_vlink_transaction::{
        NativeStemsBeamKnownSystemStem, NativeStemsBeamVLinkTransaction,
    },
    native_stems_beam_vlinkers::{
        NativeStemsBeamBLinkerRef, NativeStemsBeamVLinkerRef, NativeStemsBeamVLinkerSystem,
    },
    stems_step::NativeStemVerticalSide,
};

/// Mutable state for exactly one target B-linker cell.
///
/// The live Java `allBLinkers` arena can also contain dynamic anchors created
/// after the frozen constructor product.  This compact state deliberately
/// models only the scheduler-selected, V-owning B-linker proven by the
/// predecessor topology; it makes no completeness claim about unrelated B
/// objects.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkBLinkerFlagState {
    pub system_id: usize,
    /// Exact state immediately before boundary 14.  Boundary 15 clones and
    /// replays it before it trusts the supplied boundary-14 transaction.
    pub base_apply_state_before: NativeStemsBeamVLinkBaseApplyState,
    pub target_b_linker: NativeStemsBeamBLinkerRef,
    pub linked: bool,
    /// Native one-shot guard.  This is transaction metadata, not a Java field.
    pub committed: Option<NativeStemsBeamVLinkBaseApplyKey>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamVLinkBLinkerFlagOperation {
    BLinkerLinkedAssigned {
        target_b_linker: NativeStemsBeamBLinkerRef,
        triggering_v_linker: NativeStemsBeamVLinkerRef,
        ordered_observer_v_linkers: Vec<NativeStemsBeamVLinkerRef>,
        before: bool,
        after: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamVLinkBLinkerFlagOutcome {
    ReadyBeforeSiblingBeamLinks {
        stem_identity: usize,
        continuation_support_grade: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkBLinkerFlagTransaction {
    pub key: NativeStemsBeamVLinkBaseApplyKey,
    pub target_b_linker: NativeStemsBeamBLinkerRef,
    pub triggering_v_linker: NativeStemsBeamVLinkerRef,
    /// Exact `EnumMap<VerticalSide,VLinker>.values()` observer order.
    pub ordered_observer_v_linkers: Vec<NativeStemsBeamVLinkerRef>,
    pub linked_before: bool,
    pub linked_after: bool,
    pub operations: Vec<NativeStemsBeamVLinkBLinkerFlagOperation>,
    /// The Java setter is invoked once even when the cell is already true.
    pub linked_write_count: usize,
    /// Actual field-value changes, distinct from assignment attempts.
    pub linked_value_change_count: usize,
    pub b_linker_flag_mutation_count: usize,
    pub s_linker_flag_mutation_count: usize,
    pub persistent_id_mutation_count: usize,
    pub inter_index_mutation_count: usize,
    pub sig_vertex_mutation_count: usize,
    pub sig_relation_mutation_count: usize,
    pub stem_mutation_count: usize,
    pub beam_mutation_count: usize,
    pub sheet_edit_mutation_count: usize,
    pub sibling_link_mutation_count: usize,
    pub head_link_mutation_count: usize,
    /// `Link.applyTo`'s ignored result is retained but is not read by this
    /// setter, so every non-throwing boundary-14 disposition reaches the write.
    pub apply_returned: bool,
    pub continuation_support_grade: f64,
    pub stem_after: NativeStemsBeamKnownSystemStem,
    pub outcome: NativeStemsBeamVLinkBLinkerFlagOutcome,
    pub state_after: Box<NativeStemsBeamVLinkBLinkerFlagState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamVLinkBLinkerFlagError {
    Predecessor { phase: &'static str },
    PredecessorMismatch,
    PredecessorNotReady,
    InvalidState { phase: &'static str },
    InvalidTopology { phase: &'static str },
}

impl fmt::Display for NativeStemsBeamVLinkBLinkerFlagError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid beam VLink B-linker flag boundary: {self:?}"
        )
    }
}

impl Error for NativeStemsBeamVLinkBLinkerFlagError {}

/// Assign the scheduler-selected outer B-linker's shared `linked` cell.
///
/// Boundary 14 is independently replayed from its exact pre-state before the
/// supplied transaction or any flag state is trusted.  Every error therefore
/// occurs before the first mutation of `state`.
#[allow(clippy::too_many_arguments)]
pub fn apply_native_stems_beam_vlink_b_linker_flag_transaction(
    scheduler_system: &NativeStemsBeamSchedulerSystem,
    plan_system: &NativeStemsBeamLinkPlanSystem,
    stump_system: &NativeStemsBeamStumpSystem,
    vlinker_system: &NativeStemsBeamVLinkerSystem,
    create_transaction: &NativeStemsBeamVLinkTransaction,
    reuse_live_state: &NativeStemsBeamVLinkReuseLiveState,
    relation_parameters: NativeStemsBeamRelationParameters,
    reuse_check: &NativeStemsBeamVLinkReuseCheck,
    base_apply_transaction: &NativeStemsBeamVLinkBaseApplyTransaction,
    state: &mut NativeStemsBeamVLinkBLinkerFlagState,
) -> Result<NativeStemsBeamVLinkBLinkerFlagTransaction, NativeStemsBeamVLinkBLinkerFlagError> {
    if state.committed.is_some() {
        return Err(NativeStemsBeamVLinkBLinkerFlagError::InvalidState {
            phase: "one-shot transaction already committed",
        });
    }

    let mut replay_state = state.base_apply_state_before.clone();
    let replayed = apply_native_stems_beam_vlink_base_transaction(
        scheduler_system,
        plan_system,
        stump_system,
        vlinker_system,
        create_transaction,
        reuse_live_state,
        relation_parameters,
        reuse_check,
        &mut replay_state,
    )
    .map_err(|_| NativeStemsBeamVLinkBLinkerFlagError::Predecessor {
        phase: "boundary-14 exact replay",
    })?;
    if &replayed != base_apply_transaction
        || &replay_state != base_apply_transaction.state_after.as_ref()
        || replayed.state_after.as_ref() != &replay_state
    {
        return Err(NativeStemsBeamVLinkBLinkerFlagError::PredecessorMismatch);
    }

    // Continue from the independently replayed value, never from the
    // caller-supplied comparison copy.
    let (apply_returned, outcome_grade) = match &replayed.outcome {
        NativeStemsBeamVLinkBaseApplyOutcome::ReadyBeforeBLinkerFlagMutation {
            apply_returned,
            continuation_support_grade,
        } => (*apply_returned, *continuation_support_grade),
    };
    validate_base_apply_terminal(
        apply_returned,
        replayed.apply_returned,
        outcome_grade,
        replayed.continuation_support_grade,
        replayed.fresh_relation.grade,
        replayed.linker_flag_mutation_count,
        replayed.sibling_link_mutation_count,
        replayed.head_link_mutation_count,
    )?;

    let frontier = match &scheduler_system.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => return Err(NativeStemsBeamVLinkBLinkerFlagError::PredecessorNotReady),
    };
    let key = replayed.key;
    if state.system_id != scheduler_system.system_id
        || vlinker_system.system_id != state.system_id
        || state.system_id != key.system_id
        || frontier.invocation_ordinal != key.invocation_ordinal
        || frontier.plan != key.plan
        || frontier.v_linker.b_linker != frontier.b_linker
        || frontier.b_linker.beam != frontier.beam
        || state.target_b_linker != frontier.b_linker
        || replayed.fresh_relation.beam != frontier.beam
    {
        return Err(NativeStemsBeamVLinkBLinkerFlagError::InvalidState {
            phase: "system/frontier/base-apply join",
        });
    }

    let ordered_observer_v_linkers =
        resolve_target_observers(vlinker_system, state.target_b_linker, frontier.v_linker)?;
    let (before, operation) = assign_target_linked(
        state.target_b_linker,
        frontier.v_linker,
        &ordered_observer_v_linkers,
        &mut state.linked,
    );
    state.committed = Some(key);

    let value_change_count = usize::from(!before);
    Ok(NativeStemsBeamVLinkBLinkerFlagTransaction {
        key,
        target_b_linker: state.target_b_linker,
        triggering_v_linker: frontier.v_linker,
        ordered_observer_v_linkers,
        linked_before: before,
        linked_after: true,
        operations: vec![operation],
        linked_write_count: 1,
        linked_value_change_count: value_change_count,
        b_linker_flag_mutation_count: value_change_count,
        s_linker_flag_mutation_count: 0,
        persistent_id_mutation_count: 0,
        inter_index_mutation_count: 0,
        sig_vertex_mutation_count: 0,
        sig_relation_mutation_count: 0,
        stem_mutation_count: 0,
        beam_mutation_count: 0,
        sheet_edit_mutation_count: 0,
        sibling_link_mutation_count: 0,
        head_link_mutation_count: 0,
        apply_returned,
        continuation_support_grade: outcome_grade,
        stem_after: replayed.stem_after.clone(),
        outcome: NativeStemsBeamVLinkBLinkerFlagOutcome::ReadyBeforeSiblingBeamLinks {
            stem_identity: replayed.stem_after.stem_identity,
            continuation_support_grade: outcome_grade,
        },
        state_after: Box::new(state.clone()),
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_base_apply_terminal(
    outcome_apply_returned: bool,
    transaction_apply_returned: bool,
    outcome_grade: f64,
    transaction_grade: f64,
    fresh_relation_grade: f64,
    linker_flag_mutation_count: usize,
    sibling_link_mutation_count: usize,
    head_link_mutation_count: usize,
) -> Result<(), NativeStemsBeamVLinkBLinkerFlagError> {
    if outcome_apply_returned != transaction_apply_returned
        || !outcome_grade.is_finite()
        || outcome_grade.to_bits() != transaction_grade.to_bits()
        || outcome_grade.to_bits() != fresh_relation_grade.to_bits()
        || linker_flag_mutation_count != 0
        || sibling_link_mutation_count != 0
        || head_link_mutation_count != 0
    {
        return Err(NativeStemsBeamVLinkBLinkerFlagError::PredecessorNotReady);
    }
    Ok(())
}

fn assign_target_linked(
    target: NativeStemsBeamBLinkerRef,
    triggering: NativeStemsBeamVLinkerRef,
    ordered_observers: &[NativeStemsBeamVLinkerRef],
    linked: &mut bool,
) -> (bool, NativeStemsBeamVLinkBLinkerFlagOperation) {
    let before = *linked;
    // Exact Java operation: `this.linked = linked` with `linked == true`.
    *linked = true;
    (
        before,
        NativeStemsBeamVLinkBLinkerFlagOperation::BLinkerLinkedAssigned {
            target_b_linker: target,
            triggering_v_linker: triggering,
            ordered_observer_v_linkers: ordered_observers.to_vec(),
            before,
            after: true,
        },
    )
}

fn resolve_target_observers(
    system: &NativeStemsBeamVLinkerSystem,
    target: NativeStemsBeamBLinkerRef,
    triggering: NativeStemsBeamVLinkerRef,
) -> Result<Vec<NativeStemsBeamVLinkerRef>, NativeStemsBeamVLinkBLinkerFlagError> {
    let mut target_matches = Vec::new();
    for constructor in &system.constructors {
        for (ordinal, b_linker) in constructor.b_linkers.iter().enumerate() {
            if b_linker.reference.beam != constructor.source
                || b_linker.reference.id != ordinal + 1
                || b_linker
                    .v_linkers
                    .iter()
                    .any(|v_linker| v_linker.reference.b_linker != b_linker.reference)
            {
                return Err(NativeStemsBeamVLinkBLinkerFlagError::InvalidTopology {
                    phase: "frozen B/V ownership",
                });
            }
            if b_linker.reference == target {
                target_matches.push(b_linker);
            }
        }
    }
    if target_matches.len() != 1 {
        return Err(NativeStemsBeamVLinkBLinkerFlagError::InvalidTopology {
            phase: "target B-linker cardinality",
        });
    }
    let observers = target_matches[0]
        .v_linkers
        .iter()
        .map(|v_linker| (v_linker.reference, v_linker.y_direction))
        .collect::<Vec<_>>();
    validate_ordered_observers(target, triggering, &observers)?;
    Ok(observers
        .into_iter()
        .map(|(reference, _)| reference)
        .collect())
}

fn validate_ordered_observers(
    target: NativeStemsBeamBLinkerRef,
    triggering: NativeStemsBeamVLinkerRef,
    observers: &[(NativeStemsBeamVLinkerRef, i32)],
) -> Result<(), NativeStemsBeamVLinkBLinkerFlagError> {
    if observers.is_empty()
        || observers.len() > 2
        || observers
            .iter()
            .filter(|&&(reference, _)| reference == triggering)
            .count()
            != 1
        || observers
            .iter()
            .enumerate()
            .any(|(index, observer)| observers[..index].contains(observer))
        || observers.iter().any(|(reference, y_direction)| {
            reference.b_linker != target
                || *y_direction
                    != match reference.side {
                        NativeStemVerticalSide::Top => -1,
                        NativeStemVerticalSide::Bottom => 1,
                    }
        })
        || observers
            .windows(2)
            .any(|pair| side_rank(pair[0].0.side) >= side_rank(pair[1].0.side))
    {
        return Err(NativeStemsBeamVLinkBLinkerFlagError::InvalidTopology {
            phase: "ordered shared V-linker observers",
        });
    }
    Ok(())
}

fn side_rank(side: NativeStemVerticalSide) -> u8 {
    match side {
        NativeStemVerticalSide::Top => 0,
        NativeStemVerticalSide::Bottom => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_stems_beam_stumps::NativeStemsBeamSource;

    const B: NativeStemsBeamBLinkerRef = NativeStemsBeamBLinkerRef {
        beam: NativeStemsBeamSource::RawBeam(4),
        id: 1,
    };
    const TOP: NativeStemsBeamVLinkerRef = NativeStemsBeamVLinkerRef {
        b_linker: B,
        side: NativeStemVerticalSide::Top,
    };
    const BOTTOM: NativeStemsBeamVLinkerRef = NativeStemsBeamVLinkerRef {
        b_linker: B,
        side: NativeStemVerticalSide::Bottom,
    };

    #[test]
    fn false_to_true_is_one_assignment_and_one_value_change() {
        let mut linked = false;
        let (before, operation) = assign_target_linked(B, TOP, &[TOP, BOTTOM], &mut linked);
        assert!(!before);
        assert!(linked);
        assert_eq!(
            operation,
            NativeStemsBeamVLinkBLinkerFlagOperation::BLinkerLinkedAssigned {
                target_b_linker: B,
                triggering_v_linker: TOP,
                ordered_observer_v_linkers: vec![TOP, BOTTOM],
                before: false,
                after: true,
            }
        );
        assert_eq!(usize::from(!before), 1);
    }

    #[test]
    fn true_to_true_still_executes_the_assignment_without_a_value_change() {
        let mut linked = true;
        let (before, operation) = assign_target_linked(B, BOTTOM, &[TOP, BOTTOM], &mut linked);
        assert!(before);
        assert!(linked);
        assert_eq!(
            operation,
            NativeStemsBeamVLinkBLinkerFlagOperation::BLinkerLinkedAssigned {
                target_b_linker: B,
                triggering_v_linker: BOTTOM,
                ordered_observer_v_linkers: vec![TOP, BOTTOM],
                before: true,
                after: true,
            }
        );
        assert_eq!(usize::from(!before), 0);
    }

    #[test]
    fn observer_validation_pins_java_enum_map_order_and_directions() {
        assert!(validate_ordered_observers(B, TOP, &[(TOP, -1)]).is_ok());
        assert!(validate_ordered_observers(B, BOTTOM, &[(TOP, -1), (BOTTOM, 1)]).is_ok());
        assert!(validate_ordered_observers(B, TOP, &[(BOTTOM, 1), (TOP, -1)]).is_err());
        assert!(validate_ordered_observers(B, TOP, &[(TOP, 1), (BOTTOM, 1)]).is_err());
    }

    #[test]
    fn observer_validation_rejects_missing_duplicate_and_wrong_parent() {
        let other_b = NativeStemsBeamBLinkerRef { id: 2, ..B };
        let other = NativeStemsBeamVLinkerRef {
            b_linker: other_b,
            side: NativeStemVerticalSide::Top,
        };
        assert!(validate_ordered_observers(B, TOP, &[]).is_err());
        assert!(validate_ordered_observers(B, TOP, &[(BOTTOM, 1)]).is_err());
        assert!(validate_ordered_observers(B, TOP, &[(TOP, -1), (TOP, -1)]).is_err());
        assert!(validate_ordered_observers(B, TOP, &[(other, -1)]).is_err());
    }

    #[test]
    fn observer_validation_accepts_each_single_java_enum_map_side() {
        assert!(validate_ordered_observers(B, TOP, &[(TOP, -1)]).is_ok());
        assert!(validate_ordered_observers(B, BOTTOM, &[(BOTTOM, 1)]).is_ok());
    }

    #[test]
    fn base_apply_terminal_accepts_ignored_false_return_without_prior_mutation() {
        assert!(validate_base_apply_terminal(false, false, 0.75, 0.75, 0.75, 0, 0, 0).is_ok());
    }

    #[test]
    fn base_apply_terminal_fails_closed_on_value_or_stop_boundary_drift() {
        assert!(validate_base_apply_terminal(true, false, 0.75, 0.75, 0.75, 0, 0, 0).is_err());
        assert!(
            validate_base_apply_terminal(
                true,
                true,
                f64::from_bits(0),
                f64::from_bits(1_u64 << 63),
                f64::from_bits(0),
                0,
                0,
                0,
            )
            .is_err(),
            "signed-zero drift is not an exact predecessor grade"
        );
        assert!(
            validate_base_apply_terminal(true, true, f64::NAN, f64::NAN, f64::NAN, 0, 0, 0)
                .is_err()
        );
        assert!(validate_base_apply_terminal(true, true, 0.75, 0.75, 0.75, 1, 0, 0).is_err());
        assert!(validate_base_apply_terminal(true, true, 0.75, 0.75, 0.75, 0, 1, 0).is_err());
        assert!(validate_base_apply_terminal(true, true, 0.75, 0.75, 0.75, 0, 0, 1).is_err());
    }
}
