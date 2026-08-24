// SPDX-License-Identifier: AGPL-3.0-or-later

//! Native semantic primitives for Java's `ReductionStep`.
//!
//! The dependency-light lifecycle in [`crate::reduction_step`] owns sheet and
//! system ordering.  This module starts the production bridge from terminal
//! native STEMS SIGs with Java's deterministic `SIGraph.reduceExclusions()`
//! algorithm.  Overlap discovery and the foundation-specific consistency
//! passes remain later REDUCTION boundaries.

use crate::native_sig::{
    NativeSigContextualization, NativeSigEdge, NativeSigEdgeId, NativeSigError, NativeSigInterKind,
    NativeSigRelationKind, NativeSigRelationOrigin, NativeSigSystem, NativeSigVertexId,
};
use crate::native_stems::NativeStemsRecognition;
use crate::stems_step::NativeBeamPortion;

/// One selected exclusion and the branch Java removed from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeReductionExclusionDecision {
    pub exclusion: NativeSigEdgeId,
    pub source: NativeSigVertexId,
    pub source_best_grade: f64,
    pub target: NativeSigVertexId,
    pub target_best_grade: f64,
    pub removed: NativeSigVertexId,
}

/// Exact observable result of one `SIGraph.reduceExclusions()` call.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionExclusionTransaction {
    pub system_id: usize,
    /// Decision order. Java uses exclusion insertion order as the tie-breaker.
    pub decisions: Vec<NativeReductionExclusionDecision>,
    /// Java `LinkedHashSet` order: weaker inter, then any dying ensembles.
    pub removed_vertices: Vec<NativeSigVertexId>,
    /// Fresh whole-SIG contextualizations after each removal. Recomputing the
    /// whole native SIG is value-equivalent to Java's impacted-neighbor update.
    pub contextualizations: Vec<NativeSigContextualization>,
}

/// Owned native pipeline state after reducing exclusions already present in
/// each terminal STEMS SIG.
///
/// This is deliberately not named a completed REDUCTION recognition: Java's
/// preceding overlap discovery and its foundation consistency checks are not
/// part of this boundary yet.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionExistingExclusionsRecognition {
    pub stems: NativeStemsRecognition,
    pub initial_contextualizations: Vec<NativeSigContextualization>,
    pub transactions: Vec<NativeReductionExclusionTransaction>,
}

/// Ordered removals from one foundation consistency check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeReductionBeamPruneTransaction {
    pub system_id: usize,
    /// Beam/hook iteration order; this is Java's modification count authority.
    pub removed_beams: Vec<NativeSigVertexId>,
    /// Dying sole-member groups removed extensively with those beams.
    pub removed_groups: Vec<NativeSigVertexId>,
}

/// Ordered orphan removal from the first branch of Java `checkHeads()` or
/// `checkStems()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeReductionOrphanPruneTransaction {
    pub system_id: usize,
    pub kind: NativeSigInterKind,
    pub removed_vertices: Vec<NativeSigVertexId>,
    pub removed_ensembles: Vec<NativeSigVertexId>,
}

/// Java `Grades.minContextualGrade`.
pub const MIN_REDUCTION_CONTEXTUAL_GRADE: f64 = 0.5;

/// Java `SigReducer.Constants.minIou` broad-phase threshold.
pub const MIN_REDUCTION_OVERLAP_IOU: f64 = 0.05;

/// Precise geometry which the recognition-owned SIG deliberately does not
/// flatten into rectangles.
///
/// Java's `AbstractInter.overlaps()` dispatches across glyph run tables,
/// areas, ensembles, staff/pitch-aware heads, and bounds.  The REDUCTION
/// scheduler owns when these questions are asked; a later adapter can resolve
/// them from the retained stage products without weakening them to box tests.
pub trait NativeReductionOverlapGeometry {
    /// Whether `right` belongs to the mirror entity set built for `left`.
    fn is_mirror_entity(&mut self, left: NativeSigVertexId, right: NativeSigVertexId) -> bool;

    /// Java's mutual `left.overlaps(right) && right.overlaps(left)` test.
    fn mutually_overlaps(&mut self, left: NativeSigVertexId, right: NativeSigVertexId) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeReductionOverlapDisposition {
    MirrorAccepted,
    CompatibleAccepted,
    BelowIou,
    BeyondRightEdge,
    PreciseRejected,
    StandardHeadStemAccepted,
    ExistingExclusion,
    SupportAccepted,
    ExclusionInserted,
}

/// One pair visited by Java's stable left-abscissa nested scan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeReductionOverlapPair {
    pub left: NativeSigVertexId,
    pub right: NativeSigVertexId,
    pub iou: Option<f64>,
    pub disposition: NativeReductionOverlapDisposition,
    pub exclusion: Option<NativeSigEdgeId>,
}

/// Exact scheduling and insertion result of `SigReducer.detectOverlaps()`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionOverlapTransaction {
    pub system_id: usize,
    /// Active non-header/non-disabled vertices, stable-sorted by bounds x.
    pub scan_order: Vec<NativeSigVertexId>,
    pub pairs: Vec<NativeReductionOverlapPair>,
    pub inserted_exclusions: Vec<NativeSigEdgeId>,
}

/// Result of `SigReducer.contextualizeAndPurge()` with `purgeWeaks=true`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionWeakPurgeTransaction {
    pub system_id: usize,
    pub contextualization: NativeSigContextualization,
    /// Weak vertices snapshotted in SIG order before any removal.
    pub removed_vertices: Vec<NativeSigVertexId>,
    /// Additional unique ensemble members removed by extensive cascade.
    pub cascaded_members: Vec<NativeSigVertexId>,
}

/// Consume terminal native STEMS state and reduce every exclusion which is
/// already present, in sheet system order.
///
/// A later production boundary will run Java's overlap discovery before this
/// solver and then continue with the foundation-specific consistency epochs.
pub fn reduce_native_stems_existing_exclusions(
    mut stems: NativeStemsRecognition,
) -> Result<NativeReductionExistingExclusionsRecognition, NativeSigError> {
    let mut initial_contextualizations = Vec::with_capacity(stems.systems.len());
    let mut transactions = Vec::with_capacity(stems.systems.len());
    for system in &mut stems.systems {
        let sig = &mut system.transaction.state_after.beam_state.sig;
        initial_contextualizations.push(sig.contextualize());
        transactions.push(reduce_native_sig_exclusions(sig)?);
    }
    Ok(NativeReductionExistingExclusionsRecognition {
        stems,
        initial_contextualizations,
        transactions,
    })
}

/// Port of Java `SIGraph.reduceExclusions()` over the terminal native SIG.
///
/// The caller must contextualize the graph before entry, as `SigReducer`
/// does. A branch's contextual grade is preferred when present, otherwise its
/// intrinsic grade is used. Equal branches remove the target, matching Java's
/// `(source < target) ? source : target` expression.
pub fn reduce_native_sig_exclusions(
    sig: &mut NativeSigSystem,
) -> Result<NativeReductionExclusionTransaction, NativeSigError> {
    sig.validate_integrity()?;
    let system_id = sig.system_id;
    let mut exclusions = sig
        .edges
        .iter()
        .filter(|edge| edge.active && edge.kind == NativeSigRelationKind::Exclusion)
        .map(|edge| NativeSigEdgeId(edge.ordinal))
        .collect::<Vec<_>>();
    let mut decisions = Vec::new();
    let mut removed_vertices = Vec::new();
    let mut contextualizations = Vec::new();

    loop {
        exclusions.retain(|id| sig.edges.get(id.0).is_some_and(|edge| edge.active));
        let mut best_grade = 0.0_f64;
        let mut best = None;
        for &id in &exclusions {
            let edge = &sig.edges[id.0];
            let source_grade = best_grade_of(sig, edge.source);
            let target_grade = best_grade_of(sig, edge.target);
            let contribution = source_grade.max(target_grade);
            if best_grade < contribution {
                best_grade = contribution;
                best = Some((
                    id,
                    NativeSigVertexId(edge.source),
                    source_grade,
                    NativeSigVertexId(edge.target),
                    target_grade,
                ));
            }
        }
        let Some((exclusion, source, source_grade, target, target_grade)) = best else {
            break;
        };
        let weaker = if source_grade < target_grade {
            source
        } else {
            target
        };
        let dying_ensembles = dying_ensembles(sig, weaker);

        // AbstractInter.remove(true) marks the weaker first, removes any dying
        // ensemble non-extensively, then removes the weaker from the graph.
        remove_vertex_extensively(sig, weaker, &dying_ensembles)?;

        decisions.push(NativeReductionExclusionDecision {
            exclusion,
            source,
            source_best_grade: source_grade,
            target,
            target_best_grade: target_grade,
            removed: weaker,
        });
        push_unique(&mut removed_vertices, weaker);
        for ensemble in dying_ensembles {
            push_unique(&mut removed_vertices, ensemble);
        }
        contextualizations.push(sig.contextualize());
        exclusions.retain(|id| *id != exclusion);
    }

    sig.validate_integrity()?;
    Ok(NativeReductionExclusionTransaction {
        system_id,
        decisions,
        removed_vertices,
        contextualizations,
    })
}

/// Java `SigReducer.checkHooksHaveStem()`.
///
/// A hook survives only when at least one live `BeamStemRelation` reaches a
/// non-center beam portion. The check snapshots hook insertion order before
/// mutation, exactly like `SIGraph.inters(BeamHookInter.class)`.
pub fn prune_native_foundation_hooks(
    sig: &mut NativeSigSystem,
) -> Result<NativeReductionBeamPruneTransaction, NativeSigError> {
    let hooks = sig
        .vertices
        .iter()
        .filter(|vertex| vertex.active && vertex.kind == NativeSigInterKind::BeamHook)
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    prune_beams(sig, hooks, |sig, hook| {
        sig.edges.iter().any(|edge| {
            edge.active
                && edge.kind == NativeSigRelationKind::BeamStem
                && (edge.source == hook.0 || edge.target == hook.0)
                && edge.beam_portion != Some(NativeBeamPortion::Center)
        })
    })
}

/// Java `SigReducer.checkBeamsHaveBothStems()`.
///
/// Only standard `BeamInter` vertices are checked. Hooks and small beams have
/// their own Java paths. A standard beam must have a live BeamStem relation on
/// both horizontal portions; center relations do not satisfy either side.
pub fn prune_native_foundation_beams(
    sig: &mut NativeSigSystem,
) -> Result<NativeReductionBeamPruneTransaction, NativeSigError> {
    let beams = sig
        .vertices
        .iter()
        .filter(|vertex| vertex.active && vertex.kind == NativeSigInterKind::Beam)
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    prune_beams(sig, beams, |sig, beam| {
        let has_left = sig.edges.iter().any(|edge| {
            edge.active
                && edge.kind == NativeSigRelationKind::BeamStem
                && (edge.source == beam.0 || edge.target == beam.0)
                && edge.beam_portion == Some(NativeBeamPortion::Left)
        });
        let has_right = sig.edges.iter().any(|edge| {
            edge.active
                && edge.kind == NativeSigRelationKind::BeamStem
                && (edge.source == beam.0 || edge.target == beam.0)
                && edge.beam_portion == Some(NativeBeamPortion::Right)
        });
        has_left && has_right
    })
}

/// Java `checkHeads()`'s `headHasStem` removal branch.
///
/// This is intentionally a sub-primitive rather than the complete check: the
/// subsequent `checkHeadSide` geometry pass remains a separate boundary.
pub fn prune_native_foundation_heads_without_stems(
    sig: &mut NativeSigSystem,
) -> Result<NativeReductionOrphanPruneTransaction, NativeSigError> {
    prune_orphans_without_relation(
        sig,
        NativeSigInterKind::Head,
        NativeSigRelationKind::HeadStem,
    )
}

/// Java `checkStems()`'s `stem.getHeads().isEmpty()` removal branch.
///
/// The later `stemHasSingleHeadEnd` geometry pass is deliberately not claimed
/// by this graph-only primitive.
pub fn prune_native_foundation_stems_without_heads(
    sig: &mut NativeSigSystem,
) -> Result<NativeReductionOrphanPruneTransaction, NativeSigError> {
    prune_orphans_without_relation(
        sig,
        NativeSigInterKind::Stem,
        NativeSigRelationKind::HeadStem,
    )
}

/// Java `SigReducer.contextualizeAndPurge()` for foundation reduction.
///
/// Frozen ownership is carried by the terminal native SIG from GRID and
/// HEADERS. Ledgers are never removed by this generic grade floor; Java
/// handles them in `checkLedgers()`.
pub fn contextualize_and_purge_native_weaks(
    sig: &mut NativeSigSystem,
) -> Result<NativeReductionWeakPurgeTransaction, NativeSigError> {
    sig.validate_integrity()?;
    let contextualization = sig.contextualize();
    let removed_vertices = sig
        .vertices
        .iter()
        .filter(|vertex| {
            vertex.active
                && vertex.kind != NativeSigInterKind::Ledger
                && !vertex.frozen
                && vertex
                    .contextual_grade
                    .is_some_and(|grade| grade < MIN_REDUCTION_CONTEXTUAL_GRADE)
        })
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    let mut cascaded_members = Vec::new();
    for &vertex in &removed_vertices {
        if sig.vertex(vertex.0).is_none() {
            continue;
        }
        let dying = dying_ensembles(sig, vertex);
        let cascaded = unique_ensemble_members(sig, vertex);
        remove_vertex_extensively(sig, vertex, &dying)?;
        for member in cascaded {
            if !removed_vertices.contains(&member) {
                push_unique(&mut cascaded_members, member);
            }
        }
    }
    sig.validate_integrity()?;
    Ok(NativeReductionWeakPurgeTransaction {
        system_id: sig.system_id,
        contextualization,
        removed_vertices,
        cascaded_members,
    })
}

/// Port Java `SigReducer.detectOverlaps()` without approximating its precise
/// geometry dispatch.
///
/// This owns the stable abscissa sort, disabled/header filtering, beam-family
/// compatibility, Java rectangle IOU gate, early break, standard-head/stem
/// exception, support suppression, and normalized exclusion insertion.  Only
/// mirror membership and mutual precise overlap are delegated.
pub fn detect_native_reduction_overlaps(
    sig: &mut NativeSigSystem,
    geometry: &mut impl NativeReductionOverlapGeometry,
) -> Result<NativeReductionOverlapTransaction, NativeSigError> {
    sig.validate_integrity()?;
    let mut scan_order = sig
        .vertices
        .iter()
        .filter(|vertex| {
            vertex.active && !is_overlap_disabled(vertex.kind) && !is_header_inter(vertex.kind)
        })
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    // Slice sorting is stable, matching Stream.sorted with a comparator which
    // compares only the left x ordinate.
    scan_order.sort_by_key(|id| sig.vertices[id.0].bounds.x);

    let mut pairs = Vec::new();
    let mut inserted_exclusions = Vec::new();
    for left_index in 0..scan_order.len().saturating_sub(1) {
        let left = scan_order[left_index];
        let left_vertex = &sig.vertices[left.0];
        let left_bounds = left_vertex.bounds;
        let left_max_x = f64::from(left_bounds.x) + f64::from(left_bounds.width);
        for &right in &scan_order[left_index + 1..] {
            if geometry.is_mirror_entity(left, right) {
                pairs.push(overlap_pair(
                    left,
                    right,
                    None,
                    NativeReductionOverlapDisposition::MirrorAccepted,
                    None,
                ));
                continue;
            }
            if overlap_compatible(sig.vertices[left.0].kind, sig.vertices[right.0].kind) {
                pairs.push(overlap_pair(
                    left,
                    right,
                    None,
                    NativeReductionOverlapDisposition::CompatibleAccepted,
                    None,
                ));
                continue;
            }

            let right_bounds = sig.vertices[right.0].bounds;
            let iou = java_rectangle_iou(left_bounds, right_bounds);
            if iou < MIN_REDUCTION_OVERLAP_IOU {
                let beyond = f64::from(right_bounds.x) > left_max_x;
                pairs.push(overlap_pair(
                    left,
                    right,
                    Some(iou),
                    if beyond {
                        NativeReductionOverlapDisposition::BeyondRightEdge
                    } else {
                        NativeReductionOverlapDisposition::BelowIou
                    },
                    None,
                ));
                if beyond {
                    break;
                }
                continue;
            }
            if !geometry.mutually_overlaps(left, right) {
                pairs.push(overlap_pair(
                    left,
                    right,
                    Some(iou),
                    NativeReductionOverlapDisposition::PreciseRejected,
                    None,
                ));
                continue;
            }
            if is_standard_head_stem_pair(&sig.vertices[left.0], &sig.vertices[right.0]) {
                pairs.push(overlap_pair(
                    left,
                    right,
                    Some(iou),
                    NativeReductionOverlapDisposition::StandardHeadStemAccepted,
                    None,
                ));
                continue;
            }

            let (source, target) = if left.0 < right.0 {
                (left, right)
            } else {
                (right, left)
            };
            if let Some(existing) = active_exclusion_between(sig, source, target) {
                pairs.push(overlap_pair(
                    left,
                    right,
                    Some(iou),
                    NativeReductionOverlapDisposition::ExistingExclusion,
                    Some(existing),
                ));
                continue;
            }
            if has_support_between(sig, source, target) {
                pairs.push(overlap_pair(
                    left,
                    right,
                    Some(iou),
                    NativeReductionOverlapDisposition::SupportAccepted,
                    None,
                ));
                continue;
            }

            let exclusion = NativeSigEdgeId(sig.edges.len());
            sig.append_edge(NativeSigEdge {
                ordinal: exclusion.0,
                active: true,
                source: source.0,
                target: target.0,
                kind: NativeSigRelationKind::Exclusion,
                origin: NativeSigRelationOrigin::BaselineGraph,
                support: None,
                beam_portion: None,
                stem_extension: None,
                head_stem: None,
            })?;
            inserted_exclusions.push(exclusion);
            pairs.push(overlap_pair(
                left,
                right,
                Some(iou),
                NativeReductionOverlapDisposition::ExclusionInserted,
                Some(exclusion),
            ));
        }
    }
    sig.validate_integrity()?;
    Ok(NativeReductionOverlapTransaction {
        system_id: sig.system_id,
        scan_order,
        pairs,
        inserted_exclusions,
    })
}

fn overlap_pair(
    left: NativeSigVertexId,
    right: NativeSigVertexId,
    iou: Option<f64>,
    disposition: NativeReductionOverlapDisposition,
    exclusion: Option<NativeSigEdgeId>,
) -> NativeReductionOverlapPair {
    NativeReductionOverlapPair {
        left,
        right,
        iou,
        disposition,
        exclusion,
    }
}

fn is_overlap_disabled(kind: NativeSigInterKind) -> bool {
    matches!(
        kind,
        NativeSigInterKind::BeamGroup | NativeSigInterKind::Ledger
    )
}

fn is_header_inter(kind: NativeSigInterKind) -> bool {
    matches!(
        kind,
        NativeSigInterKind::Clef
            | NativeSigInterKind::KeyAlter
            | NativeSigInterKind::Key
            | NativeSigInterKind::TimeWhole
            | NativeSigInterKind::TimePair
    )
}

fn is_beam_family(kind: NativeSigInterKind) -> bool {
    matches!(
        kind,
        NativeSigInterKind::Beam | NativeSigInterKind::BeamHook | NativeSigInterKind::SmallBeam
    )
}

fn overlap_compatible(left: NativeSigInterKind, right: NativeSigInterKind) -> bool {
    is_beam_family(left) && is_beam_family(right)
}

fn is_standard_head_stem_pair(
    left: &crate::native_sig::NativeSigVertex,
    right: &crate::native_sig::NativeSigVertex,
) -> bool {
    let head = if left.kind == NativeSigInterKind::Head && right.kind == NativeSigInterKind::Stem {
        left
    } else if right.kind == NativeSigInterKind::Head && left.kind == NativeSigInterKind::Stem {
        right
    } else {
        return false;
    };
    head.shape.as_deref().is_some_and(|shape| {
        !matches!(
            shape,
            "NOTEHEAD_BLACK_SMALL" | "NOTEHEAD_VOID_SMALL" | "WHOLE_NOTE_SMALL" | "BREVE_SMALL"
        )
    })
}

fn active_exclusion_between(
    sig: &NativeSigSystem,
    source: NativeSigVertexId,
    target: NativeSigVertexId,
) -> Option<NativeSigEdgeId> {
    sig.edges
        .iter()
        .find(|edge| {
            edge.active
                && edge.source == source.0
                && edge.target == target.0
                && edge.kind == NativeSigRelationKind::Exclusion
        })
        .map(|edge| NativeSigEdgeId(edge.ordinal))
}

fn has_support_between(
    sig: &NativeSigSystem,
    source: NativeSigVertexId,
    target: NativeSigVertexId,
) -> bool {
    sig.edges.iter().any(|edge| {
        edge.active
            && ((edge.source == source.0 && edge.target == target.0)
                || (edge.source == target.0 && edge.target == source.0))
            && is_support_relation(edge.kind)
    })
}

fn is_support_relation(kind: NativeSigRelationKind) -> bool {
    matches!(
        kind,
        NativeSigRelationKind::NoExclusion
            | NativeSigRelationKind::BarConnection
            | NativeSigRelationKind::KeyAlters
            | NativeSigRelationKind::ClefKey
            | NativeSigRelationKind::BeamBeam
            | NativeSigRelationKind::BeamStem
            | NativeSigRelationKind::BeamHead
            | NativeSigRelationKind::BeamRest
            | NativeSigRelationKind::HeadStem
    )
}

/// Java `GeoUtil.iou(Rectangle, Rectangle)`, including its signed Rectangle
/// intersection dimensions rather than clamping them independently.
fn java_rectangle_iou(
    one: crate::native_sig::NativeSigBounds,
    two: crate::native_sig::NativeSigBounds,
) -> f64 {
    let inter_left = one.x.max(two.x);
    let inter_top = one.y.max(two.y);
    let inter_right = one
        .x
        .wrapping_add(one.width)
        .min(two.x.wrapping_add(two.width));
    let inter_bottom = one
        .y
        .wrapping_add(one.height)
        .min(two.y.wrapping_add(two.height));
    let inter_area = inter_right
        .wrapping_sub(inter_left)
        .wrapping_mul(inter_bottom.wrapping_sub(inter_top));
    if inter_area == 0 {
        return 0.0;
    }
    let one_area = one.width.wrapping_mul(one.height);
    let two_area = two.width.wrapping_mul(two.height);
    let union_area = one_area.wrapping_add(two_area).wrapping_sub(inter_area);
    f64::from(inter_area) / f64::from(union_area)
}

fn prune_beams(
    sig: &mut NativeSigSystem,
    candidates: Vec<NativeSigVertexId>,
    survives: impl Fn(&NativeSigSystem, NativeSigVertexId) -> bool,
) -> Result<NativeReductionBeamPruneTransaction, NativeSigError> {
    sig.validate_integrity()?;
    let mut removed_beams = Vec::new();
    let mut removed_groups = Vec::new();
    for candidate in candidates {
        if !survives(sig, candidate) {
            let dying = dying_ensembles(sig, candidate);
            remove_vertex_extensively(sig, candidate, &dying)?;
            removed_beams.push(candidate);
            for group in dying {
                push_unique(&mut removed_groups, group);
            }
        }
    }
    sig.validate_integrity()?;
    Ok(NativeReductionBeamPruneTransaction {
        system_id: sig.system_id,
        removed_beams,
        removed_groups,
    })
}

fn prune_orphans_without_relation(
    sig: &mut NativeSigSystem,
    kind: NativeSigInterKind,
    required_relation: NativeSigRelationKind,
) -> Result<NativeReductionOrphanPruneTransaction, NativeSigError> {
    sig.validate_integrity()?;
    let candidates = sig
        .vertices
        .iter()
        .filter(|vertex| vertex.active && vertex.kind == kind)
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    let mut removed_vertices = Vec::new();
    let mut removed_ensembles = Vec::new();
    for candidate in candidates {
        let linked = sig.edges.iter().any(|edge| {
            edge.active
                && edge.kind == required_relation
                && (edge.source == candidate.0 || edge.target == candidate.0)
        });
        if !linked {
            let dying = dying_ensembles(sig, candidate);
            remove_vertex_extensively(sig, candidate, &dying)?;
            removed_vertices.push(candidate);
            for ensemble in dying {
                push_unique(&mut removed_ensembles, ensemble);
            }
        }
    }
    sig.validate_integrity()?;
    Ok(NativeReductionOrphanPruneTransaction {
        system_id: sig.system_id,
        kind,
        removed_vertices,
        removed_ensembles,
    })
}

fn remove_vertex_extensively(
    sig: &mut NativeSigSystem,
    vertex: NativeSigVertexId,
    dying_ensembles: &[NativeSigVertexId],
) -> Result<(), NativeSigError> {
    let unique_members = unique_ensemble_members(sig, vertex);
    for &ensemble in dying_ensembles {
        sig.remove_vertex(ensemble)?;
    }
    for member in unique_members {
        if sig.vertex(member.0).is_some() {
            // Java removes unique members with `extensive=false`.
            sig.remove_vertex(member)?;
        }
    }
    if sig.vertex(vertex.0).is_some() {
        sig.remove_vertex(vertex)?;
    }
    Ok(())
}

fn unique_ensemble_members(
    sig: &NativeSigSystem,
    ensemble: NativeSigVertexId,
) -> Vec<NativeSigVertexId> {
    if sig
        .vertex(ensemble.0)
        .is_none_or(|vertex| vertex.kind != NativeSigInterKind::BeamGroup)
    {
        return Vec::new();
    }
    sig.edges
        .iter()
        .filter(|edge| {
            edge.active
                && edge.kind == NativeSigRelationKind::Containment
                && edge.source == ensemble.0
        })
        .filter_map(|edge| {
            let other_ensemble = sig.edges.iter().any(|candidate| {
                candidate.active
                    && candidate.kind == NativeSigRelationKind::Containment
                    && candidate.target == edge.target
                    && candidate.source != ensemble.0
                    && sig.vertices[candidate.source].active
            });
            (!other_ensemble).then_some(NativeSigVertexId(edge.target))
        })
        .collect()
}

fn best_grade_of(sig: &NativeSigSystem, ordinal: usize) -> f64 {
    let vertex = &sig.vertices[ordinal];
    vertex.contextual_grade.unwrap_or(vertex.grade)
}

fn dying_ensembles(sig: &NativeSigSystem, member: NativeSigVertexId) -> Vec<NativeSigVertexId> {
    sig.edges
        .iter()
        .filter(|edge| {
            edge.active
                && edge.kind == NativeSigRelationKind::Containment
                && edge.target == member.0
                && sig.vertices[edge.source].active
                && sig.vertices[edge.source].kind == NativeSigInterKind::BeamGroup
        })
        .filter_map(|edge| {
            let member_count = sig
                .edges
                .iter()
                .filter(|candidate| {
                    candidate.active
                        && candidate.kind == NativeSigRelationKind::Containment
                        && candidate.source == edge.source
                        && sig.vertices[candidate.target].active
                })
                .count();
            (member_count == 1).then_some(NativeSigVertexId(edge.source))
        })
        .collect()
}

fn push_unique(vertices: &mut Vec<NativeSigVertexId>, vertex: NativeSigVertexId) {
    if !vertices.contains(&vertex) {
        vertices.push(vertex);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_sig::{
        NativeSigBounds, NativeSigEdge, NativeSigHeadStemPayload, NativeSigRelationOrigin,
        NativeSigSupport, NativeSigVertex,
    };
    use crate::stems_step::{NativeStemHeadSide, NativeStemPoint};
    use std::collections::BTreeSet;

    #[derive(Default)]
    struct ScriptedOverlapGeometry {
        mirrors: BTreeSet<(usize, usize)>,
        overlaps: BTreeSet<(usize, usize)>,
        precise_calls: Vec<(usize, usize)>,
    }

    impl NativeReductionOverlapGeometry for ScriptedOverlapGeometry {
        fn is_mirror_entity(&mut self, left: NativeSigVertexId, right: NativeSigVertexId) -> bool {
            self.mirrors.contains(&(left.0, right.0))
        }

        fn mutually_overlaps(&mut self, left: NativeSigVertexId, right: NativeSigVertexId) -> bool {
            self.precise_calls.push((left.0, right.0));
            self.overlaps.contains(&(left.0, right.0))
        }
    }

    fn vertex(ordinal: usize, kind: NativeSigInterKind, grade: f64) -> NativeSigVertex {
        NativeSigVertex {
            ordinal,
            active: true,
            removed: false,
            frozen: false,
            kind,
            shape: None,
            grade,
            contextual_grade: Some(grade),
            bounds: NativeSigBounds {
                x: ordinal as i32 * 10,
                y: 0,
                width: 4,
                height: 10,
            },
            abnormal: false,
            beam_geometry: None,
        }
    }

    fn edge(
        ordinal: usize,
        source: usize,
        target: usize,
        kind: NativeSigRelationKind,
    ) -> NativeSigEdge {
        NativeSigEdge {
            ordinal,
            active: true,
            source,
            target,
            kind,
            origin: NativeSigRelationOrigin::BaselineGraph,
            support: None,
            beam_portion: None,
            stem_extension: None,
            head_stem: None,
        }
    }

    fn beam_stem_edge(
        ordinal: usize,
        beam: usize,
        stem: usize,
        portion: NativeBeamPortion,
    ) -> NativeSigEdge {
        NativeSigEdge {
            support: Some(NativeSigSupport {
                grade: 0.8,
                bar_connection_impacts: None,
            }),
            beam_portion: Some(portion),
            stem_extension: Some(NativeStemPoint { x: 2.0, y: 3.0 }),
            ..edge(ordinal, beam, stem, NativeSigRelationKind::BeamStem)
        }
    }

    fn head_stem_edge(ordinal: usize, head: usize, stem: usize) -> NativeSigEdge {
        NativeSigEdge {
            support: Some(NativeSigSupport {
                grade: 0.8,
                bar_connection_impacts: None,
            }),
            head_stem: Some(NativeSigHeadStemPayload {
                dx: 0.1,
                dy: 0.2,
                head_side: NativeStemHeadSide::Left,
                extension_point: NativeStemPoint { x: 2.0, y: 3.0 },
                consistency: 1.0,
                manual: false,
            }),
            ..edge(ordinal, head, stem, NativeSigRelationKind::HeadStem)
        }
    }

    #[test]
    fn overlap_scan_filters_headers_and_disabled_kinds_and_stably_sorts_by_x() {
        let mut vertices = vec![
            vertex(0, NativeSigInterKind::Head, 0.8),
            vertex(1, NativeSigInterKind::Ledger, 0.8),
            vertex(2, NativeSigInterKind::Beam, 0.8),
            vertex(3, NativeSigInterKind::BeamHook, 0.8),
            vertex(4, NativeSigInterKind::Clef, 0.8),
            vertex(5, NativeSigInterKind::Stem, 0.8),
            vertex(6, NativeSigInterKind::BeamGroup, 0.8),
        ];
        for &ordinal in &[1, 2, 3, 4, 6] {
            vertices[ordinal].bounds.x = 0;
        }
        vertices[0].bounds.x = 20;
        vertices[5].bounds.x = 100;
        let mut sig = NativeSigSystem {
            system_id: 4,
            vertices,
            edges: Vec::new(),
        };
        let mut geometry = ScriptedOverlapGeometry::default();

        let result = detect_native_reduction_overlaps(&mut sig, &mut geometry).unwrap();

        assert_eq!(
            result.scan_order,
            vec![
                NativeSigVertexId(2),
                NativeSigVertexId(3),
                NativeSigVertexId(0),
                NativeSigVertexId(5)
            ]
        );
        assert_eq!(
            result.pairs[0].disposition,
            NativeReductionOverlapDisposition::CompatibleAccepted
        );
        assert!(result.pairs.iter().any(|pair| {
            pair.left == NativeSigVertexId(0)
                && pair.right == NativeSigVertexId(5)
                && pair.disposition == NativeReductionOverlapDisposition::BeyondRightEdge
        }));
        assert!(geometry.precise_calls.is_empty());
        assert!(result.inserted_exclusions.is_empty());
    }

    #[test]
    fn overlap_scan_uses_inclusive_java_iou_and_normalizes_new_exclusion_ids() {
        let mut vertices = vec![
            vertex(0, NativeSigInterKind::Brace, 0.8),
            vertex(1, NativeSigInterKind::Barline, 0.8),
        ];
        // Intersection 2x10=20, union 200+220-20=400: exactly 0.05.
        vertices[0].bounds = NativeSigBounds {
            x: 18,
            y: 0,
            width: 22,
            height: 10,
        };
        vertices[1].bounds = NativeSigBounds {
            x: 0,
            y: 0,
            width: 20,
            height: 10,
        };
        let mut sig = NativeSigSystem {
            system_id: 5,
            vertices,
            edges: Vec::new(),
        };
        let mut geometry = ScriptedOverlapGeometry::default();
        geometry.overlaps.insert((1, 0));

        let result = detect_native_reduction_overlaps(&mut sig, &mut geometry).unwrap();

        assert_eq!(geometry.precise_calls, vec![(1, 0)]);
        assert_eq!(result.pairs[0].iou, Some(0.05));
        assert_eq!(
            result.pairs[0].disposition,
            NativeReductionOverlapDisposition::ExclusionInserted
        );
        assert_eq!(result.inserted_exclusions, vec![NativeSigEdgeId(0)]);
        assert_eq!((sig.edges[0].source, sig.edges[0].target), (0, 1));
        assert_eq!(sig.edges[0].kind, NativeSigRelationKind::Exclusion);
    }

    #[test]
    fn overlap_scan_delegates_mirrors_and_precise_rejection_without_mutation() {
        let mut sig = NativeSigSystem {
            system_id: 6,
            vertices: vec![
                vertex(0, NativeSigInterKind::Brace, 0.8),
                vertex(1, NativeSigInterKind::Barline, 0.8),
                vertex(2, NativeSigInterKind::Bracket, 0.8),
            ],
            edges: Vec::new(),
        };
        for item in &mut sig.vertices {
            item.bounds.x = 0;
            item.bounds.width = 10;
        }
        let mut geometry = ScriptedOverlapGeometry::default();
        geometry.mirrors.insert((0, 1));

        let result = detect_native_reduction_overlaps(&mut sig, &mut geometry).unwrap();

        assert_eq!(
            result.pairs[0].disposition,
            NativeReductionOverlapDisposition::MirrorAccepted
        );
        assert_eq!(geometry.precise_calls, vec![(0, 2), (1, 2)]);
        assert!(result.pairs[1..].iter().all(|pair| {
            pair.disposition == NativeReductionOverlapDisposition::PreciseRejected
        }));
        assert!(sig.edges.is_empty());
    }

    #[test]
    fn overlap_exclusion_respects_head_stem_support_and_existing_relation_exceptions() {
        fn disposition(
            mut vertices: Vec<NativeSigVertex>,
            edges: Vec<NativeSigEdge>,
        ) -> (NativeReductionOverlapDisposition, usize) {
            for vertex in &mut vertices {
                vertex.bounds.x = 0;
                vertex.bounds.width = 10;
            }
            let mut sig = NativeSigSystem {
                system_id: 7,
                vertices,
                edges,
            };
            let mut geometry = ScriptedOverlapGeometry::default();
            geometry.overlaps.insert((0, 1));
            let result = detect_native_reduction_overlaps(&mut sig, &mut geometry).unwrap();
            (result.pairs[0].disposition, sig.edges.len())
        }

        let mut head = vertex(0, NativeSigInterKind::Head, 0.8);
        head.shape = Some("NOTEHEAD_BLACK".to_owned());
        assert_eq!(
            disposition(
                vec![head.clone(), vertex(1, NativeSigInterKind::Stem, 0.8)],
                Vec::new()
            ),
            (
                NativeReductionOverlapDisposition::StandardHeadStemAccepted,
                0
            )
        );
        head.shape = Some("NOTEHEAD_BLACK_SMALL".to_owned());
        assert_eq!(
            disposition(
                vec![head, vertex(1, NativeSigInterKind::Stem, 0.8)],
                Vec::new()
            )
            .0,
            NativeReductionOverlapDisposition::ExclusionInserted
        );

        let mut support = edge(0, 0, 1, NativeSigRelationKind::NoExclusion);
        support.support = Some(NativeSigSupport {
            grade: 1.0,
            bar_connection_impacts: None,
        });
        assert_eq!(
            disposition(
                vec![
                    vertex(0, NativeSigInterKind::Brace, 0.8),
                    vertex(1, NativeSigInterKind::Barline, 0.8)
                ],
                vec![support]
            ),
            (NativeReductionOverlapDisposition::SupportAccepted, 1)
        );
        assert_eq!(
            disposition(
                vec![
                    vertex(0, NativeSigInterKind::Brace, 0.8),
                    vertex(1, NativeSigInterKind::Barline, 0.8)
                ],
                vec![edge(0, 0, 1, NativeSigRelationKind::Exclusion)]
            ),
            (NativeReductionOverlapDisposition::ExistingExclusion, 1)
        );
    }

    #[test]
    fn orphan_head_prune_keeps_only_heads_with_a_live_head_stem_relation() {
        let mut inactive = head_stem_edge(1, 2, 3);
        inactive.active = false;
        let mut sig = NativeSigSystem {
            system_id: 8,
            vertices: vec![
                vertex(0, NativeSigInterKind::Head, 0.8),
                vertex(1, NativeSigInterKind::Head, 0.8),
                vertex(2, NativeSigInterKind::Head, 0.8),
                vertex(3, NativeSigInterKind::Stem, 0.8),
            ],
            edges: vec![head_stem_edge(0, 1, 3), inactive],
        };

        let result = prune_native_foundation_heads_without_stems(&mut sig).unwrap();

        assert_eq!(result.kind, NativeSigInterKind::Head);
        assert_eq!(
            result.removed_vertices,
            vec![NativeSigVertexId(0), NativeSigVertexId(2)]
        );
        assert!(sig.vertex(0).is_none());
        assert!(sig.vertex(1).is_some());
        assert!(sig.vertex(2).is_none());
        assert!(sig.vertex(3).is_some());
    }

    #[test]
    fn orphan_stem_prune_snapshots_stem_order_and_ignores_other_relations() {
        let mut sig = NativeSigSystem {
            system_id: 9,
            vertices: vec![
                vertex(0, NativeSigInterKind::Head, 0.8),
                vertex(1, NativeSigInterKind::Stem, 0.8),
                vertex(2, NativeSigInterKind::Stem, 0.8),
                vertex(3, NativeSigInterKind::Stem, 0.8),
            ],
            edges: vec![
                head_stem_edge(0, 0, 2),
                edge(1, 1, 3, NativeSigRelationKind::Exclusion),
            ],
        };

        let result = prune_native_foundation_stems_without_heads(&mut sig).unwrap();

        assert_eq!(result.kind, NativeSigInterKind::Stem);
        assert_eq!(
            result.removed_vertices,
            vec![NativeSigVertexId(1), NativeSigVertexId(3)]
        );
        assert!(sig.vertex(1).is_none());
        assert!(sig.vertex(2).is_some());
        assert!(sig.vertex(3).is_none());
    }

    #[test]
    fn reduces_strongest_exclusion_first_and_target_on_tie() {
        let mut sig = NativeSigSystem {
            system_id: 7,
            vertices: vec![
                vertex(0, NativeSigInterKind::Head, 0.9),
                vertex(1, NativeSigInterKind::Beam, 0.2),
                vertex(2, NativeSigInterKind::Beam, 0.6),
                vertex(3, NativeSigInterKind::BeamHook, 0.6),
            ],
            edges: vec![
                edge(0, 0, 1, NativeSigRelationKind::Exclusion),
                edge(1, 2, 3, NativeSigRelationKind::Exclusion),
            ],
        };

        let result = reduce_native_sig_exclusions(&mut sig).unwrap();

        assert_eq!(
            result
                .decisions
                .iter()
                .map(|decision| (decision.exclusion.0, decision.removed.0))
                .collect::<Vec<_>>(),
            vec![(0, 1), (1, 3)]
        );
        assert_eq!(
            result.removed_vertices,
            vec![NativeSigVertexId(1), NativeSigVertexId(3)]
        );
        assert!(sig.vertices[0].active);
        assert!(sig.vertices[2].active);
        assert!(sig.vertices[1].removed);
        assert!(sig.vertices[3].removed);
        assert!(sig.edges.iter().all(|edge| !edge.active));
    }

    #[test]
    fn removes_a_dying_ensemble_after_its_weaker_member() {
        let mut sig = NativeSigSystem {
            system_id: 3,
            vertices: vec![
                vertex(0, NativeSigInterKind::BeamGroup, 1.0),
                vertex(1, NativeSigInterKind::Beam, 0.3),
                vertex(2, NativeSigInterKind::Head, 0.8),
            ],
            edges: vec![
                edge(0, 0, 1, NativeSigRelationKind::Containment),
                edge(1, 1, 2, NativeSigRelationKind::Exclusion),
            ],
        };

        let result = reduce_native_sig_exclusions(&mut sig).unwrap();

        assert_eq!(
            result.removed_vertices,
            vec![NativeSigVertexId(1), NativeSigVertexId(0)]
        );
        assert!(sig.vertices[0].removed);
        assert!(sig.vertices[1].removed);
        assert!(sig.vertices[2].active);
    }

    #[test]
    fn recomputes_support_context_before_selecting_the_next_conflict() {
        let mut sig = NativeSigSystem {
            system_id: 11,
            vertices: vec![
                vertex(0, NativeSigInterKind::Head, 0.8),
                vertex(1, NativeSigInterKind::Stem, 0.2),
                vertex(2, NativeSigInterKind::Beam, 0.45),
                vertex(3, NativeSigInterKind::BeamHook, 0.4),
            ],
            edges: vec![
                edge(0, 0, 1, NativeSigRelationKind::Exclusion),
                NativeSigEdge {
                    support: Some(NativeSigSupport {
                        grade: 1.0,
                        bar_connection_impacts: None,
                    }),
                    ..edge(1, 1, 2, NativeSigRelationKind::BeamBeam)
                },
                edge(2, 2, 3, NativeSigRelationKind::Exclusion),
            ],
        };
        sig.contextualize();

        let result = reduce_native_sig_exclusions(&mut sig).unwrap();

        assert_eq!(result.decisions.len(), 2);
        assert_eq!(result.decisions[0].removed, NativeSigVertexId(1));
        assert_eq!(result.decisions[1].removed, NativeSigVertexId(3));
        assert_eq!(result.contextualizations.len(), 2);
    }

    #[test]
    fn hook_requires_a_non_center_stem_and_removes_its_dying_group() {
        let mut sig = NativeSigSystem {
            system_id: 4,
            vertices: vec![
                vertex(0, NativeSigInterKind::BeamGroup, 1.0),
                vertex(1, NativeSigInterKind::BeamHook, 0.7),
                vertex(2, NativeSigInterKind::Stem, 0.8),
                vertex(3, NativeSigInterKind::BeamHook, 0.7),
                vertex(4, NativeSigInterKind::Stem, 0.8),
            ],
            edges: vec![
                edge(0, 0, 1, NativeSigRelationKind::Containment),
                beam_stem_edge(1, 1, 2, NativeBeamPortion::Center),
                beam_stem_edge(2, 3, 4, NativeBeamPortion::Left),
            ],
        };

        let result = prune_native_foundation_hooks(&mut sig).unwrap();

        assert_eq!(result.removed_beams, vec![NativeSigVertexId(1)]);
        assert_eq!(result.removed_groups, vec![NativeSigVertexId(0)]);
        assert!(sig.vertices[0].removed);
        assert!(sig.vertices[1].removed);
        assert!(sig.vertices[3].active);
    }

    #[test]
    fn standard_beam_requires_both_side_stems_but_small_beam_is_not_scanned() {
        let mut sig = NativeSigSystem {
            system_id: 5,
            vertices: vec![
                vertex(0, NativeSigInterKind::Beam, 0.8),
                vertex(1, NativeSigInterKind::Stem, 0.8),
                vertex(2, NativeSigInterKind::Stem, 0.8),
                vertex(3, NativeSigInterKind::Beam, 0.8),
                vertex(4, NativeSigInterKind::Stem, 0.8),
                vertex(5, NativeSigInterKind::SmallBeam, 0.8),
            ],
            edges: vec![
                beam_stem_edge(0, 0, 1, NativeBeamPortion::Left),
                beam_stem_edge(1, 0, 2, NativeBeamPortion::Right),
                beam_stem_edge(2, 3, 4, NativeBeamPortion::Center),
            ],
        };

        let result = prune_native_foundation_beams(&mut sig).unwrap();

        assert_eq!(result.removed_beams, vec![NativeSigVertexId(3)]);
        assert!(sig.vertices[0].active);
        assert!(sig.vertices[3].removed);
        assert!(sig.vertices[5].active);
    }

    #[test]
    fn weak_purge_skips_frozen_and_ledgers() {
        let mut sig = NativeSigSystem {
            system_id: 6,
            vertices: vec![
                vertex(0, NativeSigInterKind::Head, 0.2),
                vertex(1, NativeSigInterKind::Clef, 0.1),
                vertex(2, NativeSigInterKind::Ledger, 0.1),
            ],
            edges: vec![],
        };
        sig.vertices[1].frozen = true;

        let result = contextualize_and_purge_native_weaks(&mut sig).unwrap();

        assert_eq!(result.removed_vertices, vec![NativeSigVertexId(0)]);
        assert!(result.cascaded_members.is_empty());
        assert!(sig.vertices[0].removed);
        assert!(sig.vertices[1].active);
        assert!(sig.vertices[2].active);
    }

    #[test]
    fn excluding_an_ensemble_cascades_only_uniquely_owned_members() {
        let mut sig = NativeSigSystem {
            system_id: 8,
            vertices: vec![
                vertex(0, NativeSigInterKind::BeamGroup, 0.1),
                vertex(1, NativeSigInterKind::Beam, 0.9),
                vertex(2, NativeSigInterKind::Head, 0.8),
            ],
            edges: vec![
                edge(0, 0, 1, NativeSigRelationKind::Containment),
                edge(1, 0, 2, NativeSigRelationKind::Exclusion),
            ],
        };

        let result = reduce_native_sig_exclusions(&mut sig).unwrap();

        assert_eq!(result.removed_vertices, vec![NativeSigVertexId(0)]);
        assert!(sig.vertices[0].removed);
        assert!(sig.vertices[1].removed);
        assert!(sig.vertices[2].active);
    }
}
