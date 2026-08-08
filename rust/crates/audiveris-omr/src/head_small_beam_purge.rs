// SPDX-License-Identifier: AGPL-3.0-or-later

//! Pure control-flow kernel for Java `NoteHeadsBuilder.purgeSmallBeams`.
//!
//! Despite the method name, Java does not select only `SmallBeamInter` nodes.
//! It selects every shape in `ShapeSet.Beams` whose integer bounds width is
//! strictly less than the HEADS `minBeamWidth`. Candidate beams retain SIG
//! order. Live heads are stable-sorted solely by their bounds ordinate before
//! the nested scan.
//!
//! Contextual beam grades are inputs to this kernel. Production composition
//! must obtain each value with the equivalent of `SIGraph.computeContextualGrade`
//! immediately before that beam's scan. Geometry is the exact Java2D
//! horizontal-parallelogram-versus-rectangle predicate already used by the
//! native HEADS lookups.

use audiveris_image::beam_structure::Segment;

use crate::{
    head_scanner_slices::JavaRectangle,
    head_template_overlap::horizontal_parallelogram_intersects_rectangle,
};

/// The four members of Java `ShapeSet.Beams`, plus an explicit non-beam case
/// so callers can pass a live SIG-order stream without filtering it first.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeadBeamShape {
    Beam,
    BeamSmall,
    BeamHook,
    BeamHookSmall,
    Other,
}

impl HeadBeamShape {
    #[must_use]
    pub const fn is_beam(self) -> bool {
        matches!(
            self,
            Self::Beam | Self::BeamSmall | Self::BeamHook | Self::BeamHookSmall
        )
    }
}

/// One `HeadInter` in SIG insertion order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmallBeamPurgeHead {
    pub bounds: JavaRectangle,
    pub grade: f64,
    /// Initial graph-removal state. Java's `sig.inters(HeadInter.class)` omits
    /// these nodes; retaining the bit makes composition over persistent native
    /// vectors equivalent to that live-vertex query.
    pub removed: bool,
}

/// One possible beam interpretation in SIG insertion order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmallBeamPurgeBeam {
    pub shape: HeadBeamShape,
    pub bounds: JavaRectangle,
    pub median: Segment,
    pub height: f64,
    /// Result of Java `SIGraph.computeContextualGrade` for this beam at this
    /// point in the HEADS epilog.
    pub contextual_grade: f64,
    pub removed: bool,
}

/// One beam/head pair visited by Java's nested iterators.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SmallBeamPurgeCheck {
    /// Index in the caller's original `beams` slice.
    pub beam_index: usize,
    /// Index in the caller's original `heads` slice.
    pub head_index: usize,
    pub intersects: bool,
    /// The non-intersection crossed the beam's inclusive integer bottom and
    /// therefore stopped this beam's head scan.
    pub breaks: bool,
}

/// Which graph vertex Java removed for an intersecting pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmallBeamPurgeTarget {
    Beam,
    Head,
}

/// One real graph removal performed by the arbitration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmallBeamPurgeDecision {
    pub beam_index: usize,
    pub head_index: usize,
    pub beam_contextual_grade: f64,
    pub head_grade: f64,
    pub target: SmallBeamPurgeTarget,
}

/// Full result and provenance of the Java-order traversal.
#[derive(Clone, Debug, PartialEq)]
pub struct SmallBeamPurgeResult {
    /// Live input heads in stable `Inters.byOrdinate` order.
    pub ordered_head_indices: Vec<usize>,
    /// Live, qualifying beams in original SIG order.
    pub candidate_beam_indices: Vec<usize>,
    /// Candidates left in Java's local `smallBeams` list after arbitration.
    pub remaining_beam_indices: Vec<usize>,
    /// Heads left in Java's local ordinate-sorted list after arbitration.
    pub remaining_head_indices: Vec<usize>,
    /// Final graph removal state, indexed by original input.
    pub beam_removed: Vec<bool>,
    /// Final graph removal state, indexed by original input.
    pub head_removed: Vec<bool>,
    /// Contextual grade computed immediately before each candidate beam scan.
    /// Non-candidates remain `None`.
    pub computed_contextual_grades: Vec<Option<f64>>,
    pub checks: Vec<SmallBeamPurgeCheck>,
    pub decisions: Vec<SmallBeamPurgeDecision>,
}

/// Port Java `NoteHeadsBuilder.purgeSmallBeams` over persistent native inputs.
///
/// The width test is an integer strict-less-than comparison. An intersecting
/// head defeats a beam only when `head.grade > beam.contextualGrade`; equality
/// and every comparison involving NaN take Java's `else` branch and remove the
/// head. This pass inserts no exclusions: every decision is a real removal.
#[must_use]
pub fn purge_small_beams(
    heads: &[SmallBeamPurgeHead],
    beams: &[SmallBeamPurgeBeam],
    min_beam_width: i32,
) -> SmallBeamPurgeResult {
    purge_small_beams_with_contextual_grades(heads, beams, min_beam_width, |beam_index, _, _| {
        beams[beam_index].contextual_grade
    })
}

/// Dynamic-grade form of [`purge_small_beams`].
///
/// Java calls `SIGraph.computeContextualGrade(iBeam)` immediately before each
/// beam's head scan. The callback receives the original beam index and current
/// graph-removal bits, so production composition can omit support edges of a
/// beam purged earlier in this same traversal.
#[must_use]
pub fn purge_small_beams_with_contextual_grades<ContextualGrade>(
    heads: &[SmallBeamPurgeHead],
    beams: &[SmallBeamPurgeBeam],
    min_beam_width: i32,
    mut contextual_grade: ContextualGrade,
) -> SmallBeamPurgeResult
where
    ContextualGrade: FnMut(usize, &[bool], &[bool]) -> f64,
{
    let mut head_removed = heads.iter().map(|head| head.removed).collect::<Vec<_>>();
    let mut beam_removed = beams.iter().map(|beam| beam.removed).collect::<Vec<_>>();

    // `Collections.sort` is stable, and `Inters.byOrdinate` compares only y.
    // Rust's slice sort is stable as well, preserving SIG input order on ties.
    let mut ordered_head_indices = (0..heads.len())
        .filter(|&index| !head_removed[index])
        .collect::<Vec<_>>();
    ordered_head_indices.sort_by(|&left, &right| heads[left].bounds.y.cmp(&heads[right].bounds.y));
    let mut remaining_head_indices = ordered_head_indices.clone();

    let candidate_beam_indices = (0..beams.len())
        .filter(|&index| {
            let beam = beams[index];
            !beam_removed[index] && beam.shape.is_beam() && beam.bounds.width < min_beam_width
        })
        .collect::<Vec<_>>();
    let mut remaining_beam_indices = candidate_beam_indices.clone();
    let mut checks = Vec::new();
    let mut decisions = Vec::new();
    let mut computed_contextual_grades = vec![None; beams.len()];

    let mut beam_position = 0;
    while beam_position < remaining_beam_indices.len() {
        let beam_index = remaining_beam_indices[beam_position];
        let beam = beams[beam_index];
        let beam_contextual_grade = contextual_grade(beam_index, &beam_removed, &head_removed);
        computed_contextual_grades[beam_index] = Some(beam_contextual_grade);
        let beam_bottom = beam
            .bounds
            .y
            .wrapping_add(beam.bounds.height)
            .wrapping_sub(1);
        let mut head_position = 0;
        let mut purged_beam = false;

        while head_position < remaining_head_indices.len() {
            let head_index = remaining_head_indices[head_position];
            let head = heads[head_index];
            let intersects = horizontal_parallelogram_intersects_rectangle(
                beam.median,
                beam.height,
                head.bounds,
            );
            let breaks = !intersects && head.bounds.y > beam_bottom;
            checks.push(SmallBeamPurgeCheck {
                beam_index,
                head_index,
                intersects,
                breaks,
            });

            if intersects {
                let target = if head.grade > beam_contextual_grade {
                    beam_removed[beam_index] = true;
                    purged_beam = true;
                    SmallBeamPurgeTarget::Beam
                } else {
                    head_removed[head_index] = true;
                    SmallBeamPurgeTarget::Head
                };
                decisions.push(SmallBeamPurgeDecision {
                    beam_index,
                    head_index,
                    beam_contextual_grade,
                    head_grade: head.grade,
                    target,
                });

                if purged_beam {
                    // `iBeam.remove`, `itb.remove`, then `break`.
                    remaining_beam_indices.remove(beam_position);
                    break;
                }

                // `iHead.remove` and `ith.remove`: the shifted next head is
                // visited at this same position, and later beams never see it.
                remaining_head_indices.remove(head_position);
                continue;
            }

            if breaks {
                break;
            }
            head_position += 1;
        }

        if !purged_beam {
            beam_position += 1;
        }
    }

    SmallBeamPurgeResult {
        ordered_head_indices,
        candidate_beam_indices,
        remaining_beam_indices,
        remaining_head_indices,
        beam_removed,
        head_removed,
        computed_contextual_grades,
        checks,
        decisions,
    }
}
