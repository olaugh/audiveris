// SPDX-License-Identifier: AGPL-3.0-or-later
//! Boundary 18: the caller seam in `BLinker.link` immediately after
//! `VLinker.link` returns `true`.
//!
//! Java executes one plain `setLinked(true)` on the outer B-linker and moves
//! on to its next `VLinker`.  Boundary 15 already wrote the same shared cell
//! from inside `VLinker.link` (`getBLinker().setLinked(true)` on the lexical
//! parent), so on every real transaction the outer write is idempotent: the
//! boundary's content is the exact write accounting, the certified identity
//! between the current V's parent and the target cell, and the loop-resumption
//! facts (`EnumMap` order, empty-target skips, and the determined
//! `isLinked()` return).
//!
//! Fast evidence per `rust/PORTING.md`: this boundary consumes a minimal
//! typed cell plus the boundary-15 commit key rather than the full boundary-15
//! state struct; the gate hydrates both from the frozen fixtures, and the next
//! full checkpoint composes the chain end to end.

use std::error::Error;
use std::fmt;

use crate::native_stems_beam_vlink_base_apply::NativeStemsBeamVLinkBaseApplyKey;
use crate::native_stems_beam_vlinkers::{NativeStemsBeamBLinkerRef, NativeStemsBeamVLinkerRef};

/// The shared `BLinker.linked` cell as boundary 18 sees it: committed by
/// boundary 15, target identity certified, `closed` never touched here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeStemsBeamOuterBLinkerCell {
    pub target_b_linker: NativeStemsBeamBLinkerRef,
    pub linked: bool,
    pub closed: bool,
    /// The boundary-15 transaction that committed this cell.  Boundary 18
    /// refuses to run against a cell no flag transaction has written.
    pub committed_by_flag_boundary: Option<NativeStemsBeamVLinkBaseApplyKey>,
}

/// One entry of the outer B's `EnumMap<VerticalSide, VLinker>` in exact
/// `values()` order, with Java's `sb.getTargetLinkers().isEmpty()` fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeStemsBeamOuterVLinkerFact {
    pub v_linker: NativeStemsBeamVLinkerRef,
    pub has_target_linkers: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeStemsBeamVLinkOuterBLinkerOutcome {
    /// The outer assignment executed; the caller loop would move to its next
    /// `VLinker`, and `isLinked()` is already determined to return `true`.
    AssignedOuterBLinkerBeforeNextVIteration { implied_link_return: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeStemsBeamVLinkOuterBLinkerTransaction {
    pub key: NativeStemsBeamVLinkBaseApplyKey,
    pub target_b_linker: NativeStemsBeamBLinkerRef,
    pub current_v_linker: NativeStemsBeamVLinkerRef,
    pub linked_before: bool,
    pub linked_after: bool,
    /// The Java setter is invoked exactly once even when the cell is already
    /// `true`.
    pub linked_write_count: usize,
    /// Actual field-value changes, distinct from assignment attempts.  Zero on
    /// every real corpus transaction.
    pub linked_value_change_count: usize,
    pub closed_before: bool,
    pub closed_after: bool,
    pub v_map_size: usize,
    pub current_v_ordinal: usize,
    pub remaining_with_targets: usize,
    pub remaining_skipped: usize,
    pub outcome: NativeStemsBeamVLinkOuterBLinkerOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamVLinkOuterBLinkerError {
    Predecessor { phase: &'static str },
    PredecessorNotReady,
    InvalidTopology { phase: &'static str },
}

impl fmt::Display for NativeStemsBeamVLinkOuterBLinkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid beam VLink outer B-linker boundary: {self:?}"
        )
    }
}

impl Error for NativeStemsBeamVLinkOuterBLinkerError {}

/// Execute the outer `setLinked(true)` and record the loop-resumption facts.
///
/// Every error occurs before the first mutation of `cell`.
pub fn apply_native_stems_beam_vlink_outer_b_linker_transaction(
    key: NativeStemsBeamVLinkBaseApplyKey,
    head_links_returned_true: bool,
    current_v_linker: NativeStemsBeamVLinkerRef,
    ordered_v_linker_facts: &[NativeStemsBeamOuterVLinkerFact],
    cell: &mut NativeStemsBeamOuterBLinkerCell,
) -> Result<NativeStemsBeamVLinkOuterBLinkerTransaction, NativeStemsBeamVLinkOuterBLinkerError> {
    // Boundary 17 must have returned true; a false return never reaches the
    // outer assignment in Java.
    if !head_links_returned_true {
        return Err(NativeStemsBeamVLinkOuterBLinkerError::Predecessor {
            phase: "head-links-returned-true",
        });
    }
    // The cell must be the one boundary 15 committed for this same
    // transaction; boundary 18 never writes a virgin cell.
    match cell.committed_by_flag_boundary {
        Some(committed) if committed == key => {}
        Some(_) => {
            return Err(NativeStemsBeamVLinkOuterBLinkerError::Predecessor {
                phase: "flag-boundary-key",
            });
        }
        None => return Err(NativeStemsBeamVLinkOuterBLinkerError::PredecessorNotReady),
    }
    // Java's `this` in `setLinked(true)` is the lexical parent of the V that
    // just linked: `VLinker.getBLinker()` returns it, and the probe certifies
    // the identity.  The typed model certifies the same fact structurally.
    if current_v_linker.b_linker != cell.target_b_linker {
        return Err(NativeStemsBeamVLinkOuterBLinkerError::InvalidTopology {
            phase: "current-v-lexical-parent",
        });
    }
    let mut current_v_ordinal = None;
    let mut remaining_with_targets = 0;
    let mut remaining_skipped = 0;
    for (ordinal, fact) in ordered_v_linker_facts.iter().enumerate() {
        if fact.v_linker.b_linker != cell.target_b_linker {
            return Err(NativeStemsBeamVLinkOuterBLinkerError::InvalidTopology {
                phase: "v-map-foreign-linker",
            });
        }
        if fact.v_linker == current_v_linker {
            if current_v_ordinal.is_some() {
                return Err(NativeStemsBeamVLinkOuterBLinkerError::InvalidTopology {
                    phase: "current-v-duplicated",
                });
            }
            // The current V ran, so it necessarily had target linkers.
            if !fact.has_target_linkers {
                return Err(NativeStemsBeamVLinkOuterBLinkerError::InvalidTopology {
                    phase: "current-v-without-targets",
                });
            }
            current_v_ordinal = Some(ordinal);
        } else if current_v_ordinal.is_some() {
            if fact.has_target_linkers {
                remaining_with_targets += 1;
            } else {
                remaining_skipped += 1;
            }
        }
    }
    let Some(current_v_ordinal) = current_v_ordinal else {
        return Err(NativeStemsBeamVLinkOuterBLinkerError::InvalidTopology {
            phase: "current-v-missing",
        });
    };
    // Boundary 15 wrote this cell true from inside VLinker.link; reaching the
    // outer assignment with a false cell means the chain is broken.
    if !cell.linked {
        return Err(NativeStemsBeamVLinkOuterBLinkerError::PredecessorNotReady);
    }

    let linked_before = cell.linked;
    let closed_before = cell.closed;
    // The exact source seam: one plain assignment, nothing else.
    cell.linked = true;
    let linked_after = cell.linked;

    Ok(NativeStemsBeamVLinkOuterBLinkerTransaction {
        key,
        target_b_linker: cell.target_b_linker,
        current_v_linker,
        linked_before,
        linked_after,
        linked_write_count: 1,
        linked_value_change_count: usize::from(linked_before != linked_after),
        closed_before,
        closed_after: cell.closed,
        v_map_size: ordered_v_linker_facts.len(),
        current_v_ordinal,
        remaining_with_targets,
        remaining_skipped,
        outcome:
            NativeStemsBeamVLinkOuterBLinkerOutcome::AssignedOuterBLinkerBeforeNextVIteration {
                implied_link_return: linked_after,
            },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_stems_beam_scheduler::NativeStemsBeamPlanRef;
    use crate::native_stems_beam_stumps::NativeStemsBeamSource;
    use crate::stems_step::NativeStemVerticalSide;

    fn key(plan_ordinal: usize) -> NativeStemsBeamVLinkBaseApplyKey {
        NativeStemsBeamVLinkBaseApplyKey {
            system_id: 1,
            invocation_ordinal: 0,
            plan: NativeStemsBeamPlanRef {
                system_id: 1,
                plan_ordinal,
                builder_ordinal: 0,
                stem_profile: 6,
            },
        }
    }

    fn b_ref() -> NativeStemsBeamBLinkerRef {
        NativeStemsBeamBLinkerRef {
            beam: NativeStemsBeamSource::RawBeam(12),
            id: 0,
        }
    }

    fn v_ref(side: NativeStemVerticalSide) -> NativeStemsBeamVLinkerRef {
        NativeStemsBeamVLinkerRef {
            b_linker: b_ref(),
            side,
        }
    }

    fn committed_cell() -> NativeStemsBeamOuterBLinkerCell {
        NativeStemsBeamOuterBLinkerCell {
            target_b_linker: b_ref(),
            linked: true,
            closed: false,
            committed_by_flag_boundary: Some(key(143)),
        }
    }

    fn top_fact() -> NativeStemsBeamOuterVLinkerFact {
        NativeStemsBeamOuterVLinkerFact {
            v_linker: v_ref(NativeStemVerticalSide::Top),
            has_target_linkers: true,
        }
    }

    #[test]
    fn real_shape_single_v_idempotent_write() {
        let mut cell = committed_cell();
        let transaction = apply_native_stems_beam_vlink_outer_b_linker_transaction(
            key(143),
            true,
            v_ref(NativeStemVerticalSide::Top),
            &[top_fact()],
            &mut cell,
        )
        .expect("real-shape transaction");
        assert!(transaction.linked_before && transaction.linked_after);
        assert_eq!(transaction.linked_write_count, 1);
        assert_eq!(transaction.linked_value_change_count, 0);
        assert_eq!(transaction.v_map_size, 1);
        assert_eq!(transaction.current_v_ordinal, 0);
        assert_eq!(transaction.remaining_with_targets, 0);
        assert_eq!(transaction.remaining_skipped, 0);
        assert_eq!(
            transaction.outcome,
            NativeStemsBeamVLinkOuterBLinkerOutcome::AssignedOuterBLinkerBeforeNextVIteration {
                implied_link_return: true,
            }
        );
        assert!(cell.linked && !cell.closed);
    }

    #[test]
    fn remaining_v_facts_follow_enum_map_order() {
        let mut cell = committed_cell();
        let facts = [
            top_fact(),
            NativeStemsBeamOuterVLinkerFact {
                v_linker: v_ref(NativeStemVerticalSide::Bottom),
                has_target_linkers: false,
            },
        ];
        let transaction = apply_native_stems_beam_vlink_outer_b_linker_transaction(
            key(143),
            true,
            v_ref(NativeStemVerticalSide::Top),
            &facts,
            &mut cell,
        )
        .expect("two-sided transaction");
        assert_eq!(transaction.v_map_size, 2);
        assert_eq!(transaction.current_v_ordinal, 0);
        assert_eq!(transaction.remaining_with_targets, 0);
        assert_eq!(transaction.remaining_skipped, 1);

        let mut cell = committed_cell();
        let facts = [
            NativeStemsBeamOuterVLinkerFact {
                v_linker: v_ref(NativeStemVerticalSide::Top),
                has_target_linkers: true,
            },
            NativeStemsBeamOuterVLinkerFact {
                v_linker: v_ref(NativeStemVerticalSide::Bottom),
                has_target_linkers: true,
            },
        ];
        let transaction = apply_native_stems_beam_vlink_outer_b_linker_transaction(
            key(143),
            true,
            v_ref(NativeStemVerticalSide::Top),
            &facts,
            &mut cell,
        )
        .expect("remaining-with-targets transaction");
        assert_eq!(transaction.remaining_with_targets, 1);
        assert_eq!(transaction.remaining_skipped, 0);
    }

    #[test]
    fn earlier_siblings_are_not_remaining() {
        let mut cell = committed_cell();
        let facts = [
            NativeStemsBeamOuterVLinkerFact {
                v_linker: v_ref(NativeStemVerticalSide::Top),
                has_target_linkers: false,
            },
            NativeStemsBeamOuterVLinkerFact {
                v_linker: v_ref(NativeStemVerticalSide::Bottom),
                has_target_linkers: true,
            },
        ];
        let transaction = apply_native_stems_beam_vlink_outer_b_linker_transaction(
            key(143),
            true,
            v_ref(NativeStemVerticalSide::Bottom),
            &facts,
            &mut cell,
        )
        .expect("second-ordinal transaction");
        assert_eq!(transaction.current_v_ordinal, 1);
        assert_eq!(transaction.remaining_with_targets, 0);
        assert_eq!(transaction.remaining_skipped, 0);
    }

    #[test]
    fn false_head_links_return_never_assigns() {
        let mut cell = committed_cell();
        let error = apply_native_stems_beam_vlink_outer_b_linker_transaction(
            key(143),
            false,
            v_ref(NativeStemVerticalSide::Top),
            &[top_fact()],
            &mut cell,
        )
        .unwrap_err();
        assert_eq!(
            error,
            NativeStemsBeamVLinkOuterBLinkerError::Predecessor {
                phase: "head-links-returned-true",
            }
        );
        assert_eq!(cell, committed_cell());
    }

    #[test]
    fn uncommitted_or_foreign_cell_is_refused() {
        let mut cell = committed_cell();
        cell.committed_by_flag_boundary = None;
        let error = apply_native_stems_beam_vlink_outer_b_linker_transaction(
            key(143),
            true,
            v_ref(NativeStemVerticalSide::Top),
            &[top_fact()],
            &mut cell,
        )
        .unwrap_err();
        assert_eq!(
            error,
            NativeStemsBeamVLinkOuterBLinkerError::PredecessorNotReady
        );

        let mut cell = committed_cell();
        cell.committed_by_flag_boundary = Some(key(999));
        let error = apply_native_stems_beam_vlink_outer_b_linker_transaction(
            key(143),
            true,
            v_ref(NativeStemVerticalSide::Top),
            &[top_fact()],
            &mut cell,
        )
        .unwrap_err();
        assert_eq!(
            error,
            NativeStemsBeamVLinkOuterBLinkerError::Predecessor {
                phase: "flag-boundary-key",
            }
        );
    }

    #[test]
    fn broken_chain_and_topology_drift_fail_before_mutation() {
        let mut cell = committed_cell();
        cell.linked = false;
        let error = apply_native_stems_beam_vlink_outer_b_linker_transaction(
            key(143),
            true,
            v_ref(NativeStemVerticalSide::Top),
            &[top_fact()],
            &mut cell,
        )
        .unwrap_err();
        assert_eq!(
            error,
            NativeStemsBeamVLinkOuterBLinkerError::PredecessorNotReady
        );
        assert!(!cell.linked, "error paths never mutate the cell");

        let mut cell = committed_cell();
        let foreign = NativeStemsBeamVLinkerRef {
            b_linker: NativeStemsBeamBLinkerRef {
                beam: NativeStemsBeamSource::RawBeam(7),
                id: 0,
            },
            side: NativeStemVerticalSide::Top,
        };
        let error = apply_native_stems_beam_vlink_outer_b_linker_transaction(
            key(143),
            true,
            foreign,
            &[top_fact()],
            &mut cell,
        )
        .unwrap_err();
        assert_eq!(
            error,
            NativeStemsBeamVLinkOuterBLinkerError::InvalidTopology {
                phase: "current-v-lexical-parent",
            }
        );

        let mut cell = committed_cell();
        let error = apply_native_stems_beam_vlink_outer_b_linker_transaction(
            key(143),
            true,
            v_ref(NativeStemVerticalSide::Bottom),
            &[top_fact()],
            &mut cell,
        )
        .unwrap_err();
        assert_eq!(
            error,
            NativeStemsBeamVLinkOuterBLinkerError::InvalidTopology {
                phase: "current-v-missing",
            }
        );

        let mut cell = committed_cell();
        let mut skipped_current = top_fact();
        skipped_current.has_target_linkers = false;
        let error = apply_native_stems_beam_vlink_outer_b_linker_transaction(
            key(143),
            true,
            v_ref(NativeStemVerticalSide::Top),
            &[skipped_current],
            &mut cell,
        )
        .unwrap_err();
        assert_eq!(
            error,
            NativeStemsBeamVLinkOuterBLinkerError::InvalidTopology {
                phase: "current-v-without-targets",
            }
        );
    }
}
