// SPDX-License-Identifier: AGPL-3.0-or-later

//! Pure decision kernel for Java HEADS duplicate and overlap purges.
//!
//! `NoteHeadsBuilder.purge` owns ordering, rectangle gating, grade/tally
//! selection, true duplicate removal, and overlap-exclusion insertion. The two
//! pair predicates do not belong to that pure loop: `Inter.isSameAs` may inspect
//! glyph identity, while `HeadInter.overlaps` also uses staff/pitch and overlap
//! ratios. Callers therefore supply those predicates over original input
//! indices. They are invoked only after Java `Rectangle.intersects` succeeds.

use std::cmp::Ordering;

use crate::head_scanner_slices::JavaRectangle;

/// Java `NoteHeadsBuilder.EPSILON` used to classify equal grades.
pub const PURGE_GRADE_EPSILON: f64 = 1.0e-5;

/// Provenance retained for composition and differential diagnostics.
///
/// Java's choice logic does not inspect this value directly. Seed preference
/// is represented by the presence of [`HeadSeedTally`] data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeadPurgeOrigin {
    Seed,
    Range,
}

/// The two optional stem-seed distances used by `purgedEquals`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeadSeedTally {
    pub left_dx: Option<f64>,
    pub right_dx: Option<f64>,
}

impl HeadSeedTally {
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.left_dx.is_none() && self.right_dx.is_none()
    }
}

/// Input fields owned by the pure purge loop.
///
/// `shape` is only used by the equal-grade tally-preservation branch. It does
/// not replace either caller-supplied pair predicate.
#[derive(Clone, Debug, PartialEq)]
pub struct HeadPurgeInput<Shape> {
    pub java_id: i32,
    pub origin: HeadPurgeOrigin,
    pub bounds: JavaRectangle,
    pub grade: f64,
    pub shape: Shape,
    pub good: bool,
    pub tally: HeadSeedTally,
    pub removed: bool,
}

/// Why Java selected one member of a matching pair for purge/exclusion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeadPurgeReason {
    LowerGradeLeft,
    LowerGradeRight,
    EqualLeftNoSeed,
    EqualRightNoSeed,
    EqualDifferentShape,
    EqualDifferentBounds,
    EqualPreserveLeft,
}

/// One pair visited by the Java nested loop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeadPurgeCheck {
    pub left_index: usize,
    pub right_index: usize,
    pub intersects: bool,
    pub predicate_matches: bool,
    pub breaks: bool,
}

/// One duplicate removal or overlap exclusion decision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadPurgeDecision {
    pub left_index: usize,
    pub right_index: usize,
    pub purged_index: usize,
    pub kept_index: usize,
    pub grade_difference: f64,
    pub reason: HeadPurgeReason,
}

/// Trace of one `purge` invocation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeadPurgePhase {
    pub checks: Vec<HeadPurgeCheck>,
    pub decisions: Vec<HeadPurgeDecision>,
}

/// Result of the production duplicate pass followed by the overlap pass.
#[derive(Clone, Debug, PartialEq)]
pub struct HeadPurgeResult {
    /// Original indices in Java `Inters.byFullAbscissa` order.
    pub ordered_indices: Vec<usize>,
    /// True removals performed by the duplicate pass, in decision order.
    pub duplicate_removed_indices: Vec<usize>,
    /// Ordered list after Java's duplicate `heads.removeAll(removed)`.
    /// Pre-removed inputs remain present, exactly as in Java, but are skipped.
    pub retained_indices: Vec<usize>,
    /// Removed state after the duplicate pass, indexed by original input.
    pub removed: Vec<bool>,
    /// Tallies after both passes, including equal-pair replication.
    pub tallies: Vec<HeadSeedTally>,
    pub duplicate: HeadPurgePhase,
    /// Each decision represents one inserted overlap exclusion. No head is
    /// removed by this phase, and the same head can occur in many decisions.
    pub overlap: HeadPurgePhase,
}

/// Port the per-staff duplicate-removal and overlap-exclusion purge sequence.
///
/// Predicates receive indices into `heads`, not sorted positions. `is_same_as`
/// must supply the complete Java duplicate predicate, including glyph identity
/// when relevant. `overlaps` must supply the complete Java head-overlap result.
#[must_use]
pub fn purge_staff_heads<Shape, Same, Overlap>(
    heads: &[HeadPurgeInput<Shape>],
    mut is_same_as: Same,
    mut overlaps: Overlap,
) -> HeadPurgeResult
where
    Shape: Eq,
    Same: FnMut(usize, usize) -> bool,
    Overlap: FnMut(usize, usize) -> bool,
{
    let mut ordered_indices = (0..heads.len()).collect::<Vec<_>>();
    ordered_indices.sort_by(|&left, &right| full_abscissa_compare(heads, left, right));

    let mut tallies = heads.iter().map(|head| head.tally).collect::<Vec<_>>();
    let mut removed = heads.iter().map(|head| head.removed).collect::<Vec<_>>();
    let mut duplicate_removed_indices = Vec::new();
    let duplicate = run_purge(
        heads,
        &ordered_indices,
        &mut tallies,
        &mut removed,
        true,
        &mut duplicate_removed_indices,
        &mut is_same_as,
    );

    // Java removes only objects selected during this invocation. An object
    // already marked removed on entry is skipped but remains in the list.
    let retained_indices = ordered_indices
        .iter()
        .copied()
        .filter(|index| !duplicate_removed_indices.contains(index))
        .collect::<Vec<_>>();
    let overlap = run_purge(
        heads,
        &retained_indices,
        &mut tallies,
        &mut removed,
        false,
        &mut Vec::new(),
        &mut overlaps,
    );

    HeadPurgeResult {
        ordered_indices,
        duplicate_removed_indices,
        retained_indices,
        removed,
        tallies,
        duplicate,
        overlap,
    }
}

#[allow(clippy::too_many_arguments)]
fn run_purge<Shape, Predicate>(
    heads: &[HeadPurgeInput<Shape>],
    ordered_indices: &[usize],
    tallies: &mut [HeadSeedTally],
    removed: &mut [bool],
    do_remove: bool,
    newly_removed: &mut Vec<usize>,
    predicate: &mut Predicate,
) -> HeadPurgePhase
where
    Shape: Eq,
    Predicate: FnMut(usize, usize) -> bool,
{
    let mut phase = HeadPurgePhase::default();
    if ordered_indices.len() < 2 {
        return phase;
    }

    'left: for left_position in 0..(ordered_indices.len() - 1) {
        let left_index = ordered_indices[left_position];
        if removed[left_index] {
            continue;
        }

        let left_box = heads[left_index].bounds;
        let x_max = left_box.x.wrapping_add(left_box.width).wrapping_sub(1);
        for &right_index in &ordered_indices[(left_position + 1)..] {
            if removed[right_index] {
                continue;
            }

            let right_box = heads[right_index].bounds;
            let intersects = left_box.intersects(right_box);
            let predicate_matches = intersects && predicate(left_index, right_index);
            let breaks = !intersects && right_box.x > x_max;
            phase.checks.push(HeadPurgeCheck {
                left_index,
                right_index,
                intersects,
                predicate_matches,
                breaks,
            });

            if predicate_matches {
                let decision = choose_purged(heads, tallies, left_index, right_index);
                phase.decisions.push(decision);
                if do_remove {
                    removed[decision.purged_index] = true;
                    newly_removed.push(decision.purged_index);
                    if decision.purged_index == left_index {
                        continue 'left;
                    }
                }
            }
            if breaks {
                break;
            }
        }
    }
    phase
}

fn choose_purged<Shape: Eq>(
    heads: &[HeadPurgeInput<Shape>],
    tallies: &mut [HeadSeedTally],
    left_index: usize,
    right_index: usize,
) -> HeadPurgeDecision {
    let left = &heads[left_index];
    let right = &heads[right_index];
    let grade_difference = (left.grade - right.grade).abs();
    let (purged_index, reason) = if grade_difference < PURGE_GRADE_EPSILON {
        choose_equal(heads, tallies, left_index, right_index)
    } else if left.grade < right.grade {
        (left_index, HeadPurgeReason::LowerGradeLeft)
    } else {
        (right_index, HeadPurgeReason::LowerGradeRight)
    };
    let kept_index = if purged_index == left_index {
        right_index
    } else {
        left_index
    };
    HeadPurgeDecision {
        left_index,
        right_index,
        purged_index,
        kept_index,
        grade_difference,
        reason,
    }
}

fn choose_equal<Shape: Eq>(
    heads: &[HeadPurgeInput<Shape>],
    tallies: &mut [HeadSeedTally],
    left_index: usize,
    right_index: usize,
) -> (usize, HeadPurgeReason) {
    let left_tally = tallies[left_index];
    if left_tally.is_empty() {
        return (left_index, HeadPurgeReason::EqualLeftNoSeed);
    }

    let right_tally = tallies[right_index];
    if right_tally.is_empty() {
        return (right_index, HeadPurgeReason::EqualRightNoSeed);
    }

    let left = &heads[left_index];
    let right = &heads[right_index];
    if left.shape != right.shape {
        return (right_index, HeadPurgeReason::EqualDifferentShape);
    }
    if left.bounds != right.bounds {
        return (right_index, HeadPurgeReason::EqualDifferentBounds);
    }

    if left.good && right.good {
        if tallies[left_index].left_dx.is_none() && right_tally.left_dx.is_some() {
            tallies[left_index].left_dx = right_tally.left_dx;
        }
        if tallies[left_index].right_dx.is_none() && right_tally.right_dx.is_some() {
            tallies[left_index].right_dx = right_tally.right_dx;
        }
    }
    (right_index, HeadPurgeReason::EqualPreserveLeft)
}

fn full_abscissa_compare<Shape>(
    heads: &[HeadPurgeInput<Shape>],
    left_index: usize,
    right_index: usize,
) -> Ordering {
    if left_index == right_index {
        return Ordering::Equal;
    }
    let left = &heads[left_index];
    let right = &heads[right_index];
    let dx = left.bounds.x.wrapping_sub(right.bounds.x);
    if dx != 0 {
        return dx.cmp(&0);
    }
    let dy = left.bounds.y.wrapping_sub(right.bounds.y);
    if dy != 0 {
        return dy.cmp(&0);
    }
    left.java_id.cmp(&right.java_id)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Shape {
        Black,
        Void,
    }

    fn head(id: i32, x: i32, y: i32, grade: f64) -> HeadPurgeInput<Shape> {
        HeadPurgeInput {
            java_id: id,
            origin: HeadPurgeOrigin::Range,
            bounds: JavaRectangle::new(x, y, 10, 10),
            grade,
            shape: Shape::Black,
            good: true,
            tally: HeadSeedTally::default(),
            removed: false,
        }
    }

    fn always_false(_: usize, _: usize) -> bool {
        false
    }

    #[test]
    fn full_abscissa_order_uses_x_then_y_then_id_and_is_stable_on_equal_keys() {
        let mut heads = vec![
            head(7, 10, 3, 1.0),
            head(2, 10, 3, 1.0),
            head(2, 10, 3, 1.0),
            head(9, 9, 50, 1.0),
            head(1, 10, 2, 1.0),
        ];
        for item in &mut heads {
            item.bounds.width = 0;
        }
        let result = purge_staff_heads(&heads, always_false, always_false);
        assert_eq!(result.ordered_indices, vec![3, 4, 1, 2, 0]);
    }

    #[test]
    fn full_abscissa_x_and_y_subtraction_wrap_like_java_int() {
        let x_extremes = [head(0, i32::MIN, 0, 1.0), head(1, i32::MAX, 0, 1.0)];
        assert_eq!(full_abscissa_compare(&x_extremes, 0, 1), Ordering::Greater);

        let y_extremes = [head(0, 0, i32::MIN, 1.0), head(1, 0, i32::MAX, 1.0)];
        assert_eq!(full_abscissa_compare(&y_extremes, 0, 1), Ordering::Greater);
    }

    #[test]
    fn rectangle_edge_boundary_and_inclusive_xmax_match_java() {
        let heads = [
            head(0, 0, 0, 1.0),
            head(1, 9, 20, 1.0),
            head(2, 10, 0, 1.0),
            head(3, 11, 0, 1.0),
        ];
        let left_zero_calls = Cell::new(0);
        let result = purge_staff_heads(
            &heads,
            |left, _| {
                if left == 0 {
                    left_zero_calls.set(left_zero_calls.get() + 1);
                }
                false
            },
            always_false,
        );
        assert_eq!(left_zero_calls.get(), 0);
        assert_eq!(
            result.duplicate.checks[..2],
            [
                HeadPurgeCheck {
                    left_index: 0,
                    right_index: 1,
                    intersects: false,
                    predicate_matches: false,
                    breaks: false,
                },
                HeadPurgeCheck {
                    left_index: 0,
                    right_index: 2,
                    intersects: false,
                    predicate_matches: false,
                    breaks: true,
                },
            ]
        );
        assert!(
            !result
                .duplicate
                .checks
                .iter()
                .any(|check| { check.left_index == 0 && check.right_index == 3 })
        );
    }

    #[test]
    fn x_max_uses_java_int_overflow() {
        let mut left = head(0, i32::MAX - 2, 0, 1.0);
        left.bounds.width = 10;
        let right = head(1, i32::MAX - 1, 20, 1.0);
        let result = purge_staff_heads(&[left, right], always_false, always_false);
        assert_eq!(result.duplicate.checks.len(), 1);
        assert!(result.duplicate.checks[0].breaks);
    }

    #[test]
    fn duplicate_removal_skips_removed_right_and_abandons_removed_left() {
        let heads = [
            head(0, 0, 0, 0.1),
            head(1, 1, 0, 0.9),
            head(2, 2, 0, 0.8),
            head(3, 3, 0, 0.7),
        ];
        let result = purge_staff_heads(&heads, |_, _| true, always_false);
        assert_eq!(result.duplicate_removed_indices, vec![0, 2, 3]);
        assert_eq!(result.retained_indices, vec![1]);
        assert_eq!(
            result
                .duplicate
                .decisions
                .iter()
                .map(|decision| (decision.left_index, decision.right_index))
                .collect::<Vec<_>>(),
            vec![(0, 1), (1, 2), (1, 3)]
        );
    }

    #[test]
    fn epsilon_boundary_is_strict_and_nan_follows_java_comparisons() {
        let boundary = [head(0, 0, 0, 1.0), head(1, 0, 0, 1.0 + PURGE_GRADE_EPSILON)];
        let result = purge_staff_heads(&boundary, |_, _| true, always_false);
        assert_eq!(
            result.duplicate.decisions[0].reason,
            HeadPurgeReason::LowerGradeLeft
        );

        let below = f64::from_bits((1.0 + PURGE_GRADE_EPSILON).to_bits() - 1);
        let result = purge_staff_heads(
            &[head(0, 0, 0, 1.0), head(1, 0, 0, below)],
            |_, _| true,
            always_false,
        );
        assert_eq!(
            result.duplicate.decisions[0].reason,
            HeadPurgeReason::EqualLeftNoSeed
        );

        let result = purge_staff_heads(
            &[head(0, 0, 0, f64::NAN), head(1, 0, 0, 1.0)],
            |_, _| true,
            always_false,
        );
        assert_eq!(result.duplicate_removed_indices, vec![1]);
        assert_eq!(
            result.duplicate.decisions[0].reason,
            HeadPurgeReason::LowerGradeRight
        );
    }

    #[test]
    fn equal_grade_prefers_the_head_with_seed_tally_not_origin_label() {
        let mut seed_without_tally = head(0, 0, 0, 0.5);
        seed_without_tally.origin = HeadPurgeOrigin::Seed;
        let range_without_tally = head(1, 0, 0, 0.5);
        let result = purge_staff_heads(
            &[seed_without_tally, range_without_tally],
            |_, _| true,
            always_false,
        );
        assert_eq!(result.duplicate_removed_indices, vec![0]);

        let mut seed_with_tally = head(0, 0, 0, 0.5);
        seed_with_tally.origin = HeadPurgeOrigin::Seed;
        seed_with_tally.tally.left_dx = Some(-0.5);
        let result = purge_staff_heads(
            &[seed_with_tally, head(1, 0, 0, 0.5)],
            |_, _| true,
            always_false,
        );
        assert_eq!(result.duplicate_removed_indices, vec![1]);
        assert_eq!(
            result.duplicate.decisions[0].reason,
            HeadPurgeReason::EqualRightNoSeed
        );
    }

    #[test]
    fn equal_shape_and_bounds_replicate_complementary_good_tallies() {
        let mut left = head(0, 0, 0, 0.5);
        left.origin = HeadPurgeOrigin::Seed;
        left.tally.left_dx = Some(-0.25);
        let mut right = head(1, 0, 0, 0.5);
        right.origin = HeadPurgeOrigin::Seed;
        right.tally.right_dx = Some(0.75);
        let result = purge_staff_heads(&[left, right], |_, _| true, always_false);
        assert_eq!(result.duplicate_removed_indices, vec![1]);
        assert_eq!(
            result.duplicate.decisions[0].reason,
            HeadPurgeReason::EqualPreserveLeft
        );
        assert_eq!(
            result.tallies[0],
            HeadSeedTally {
                left_dx: Some(-0.25),
                right_dx: Some(0.75),
            }
        );
    }

    #[test]
    fn different_shape_or_bounds_prevents_tally_replication() {
        let mut left = head(0, 0, 0, 0.5);
        left.tally.left_dx = Some(-0.25);
        let mut different_shape = head(1, 0, 0, 0.5);
        different_shape.shape = Shape::Void;
        different_shape.tally.right_dx = Some(0.75);
        let result = purge_staff_heads(&[left.clone(), different_shape], |_, _| true, always_false);
        assert_eq!(
            result.duplicate.decisions[0].reason,
            HeadPurgeReason::EqualDifferentShape
        );
        assert_eq!(result.tallies[0].right_dx, None);

        let mut different_bounds = head(1, 1, 0, 0.5);
        different_bounds.tally.right_dx = Some(0.75);
        let result = purge_staff_heads(&[left, different_bounds], |_, _| true, always_false);
        assert_eq!(
            result.duplicate.decisions[0].reason,
            HeadPurgeReason::EqualDifferentBounds
        );
        assert_eq!(result.tallies[0].right_dx, None);
    }

    #[test]
    fn overlap_pass_inserts_exclusions_without_removal() {
        let heads = [head(0, 0, 0, 0.1), head(1, 1, 0, 0.9), head(2, 2, 0, 0.5)];
        let result = purge_staff_heads(&heads, always_false, |_, _| true);
        assert_eq!(result.retained_indices, vec![0, 1, 2]);
        assert_eq!(result.overlap.decisions.len(), 3);
        assert_eq!(
            result
                .overlap
                .decisions
                .iter()
                .map(|decision| decision.purged_index)
                .collect::<Vec<_>>(),
            vec![0, 0, 2]
        );
        assert_eq!(result.removed, vec![false, false, false]);
    }

    #[test]
    fn overlap_equal_pair_can_replicate_tally_without_removing_either_head() {
        let mut left = head(0, 0, 0, 0.5);
        left.tally.left_dx = Some(-0.25);
        let mut right = head(1, 0, 0, 0.5);
        right.tally.right_dx = Some(0.75);
        let result = purge_staff_heads(&[left, right], always_false, |_, _| true);
        assert_eq!(result.retained_indices, vec![0, 1]);
        assert_eq!(result.overlap.decisions.len(), 1);
        assert_eq!(
            result.overlap.decisions[0].reason,
            HeadPurgeReason::EqualPreserveLeft
        );
        assert_eq!(
            result.tallies[0],
            HeadSeedTally {
                left_dx: Some(-0.25),
                right_dx: Some(0.75),
            }
        );
        assert_eq!(result.removed, vec![false, false]);
    }

    #[test]
    fn pre_removed_inputs_are_skipped_but_not_physically_removed() {
        let mut middle = head(1, 1, 0, 0.9);
        middle.removed = true;
        let heads = [head(0, 0, 0, 0.1), middle, head(2, 2, 0, 0.5)];
        let result = purge_staff_heads(&heads, always_false, always_false);
        assert_eq!(result.retained_indices, vec![0, 1, 2]);
        assert!(result.removed[1]);
        assert!(
            result
                .duplicate
                .checks
                .iter()
                .all(|check| check.left_index != 1 && check.right_index != 1)
        );
        assert!(
            result
                .overlap
                .checks
                .iter()
                .all(|check| check.left_index != 1 && check.right_index != 1)
        );
    }
}
