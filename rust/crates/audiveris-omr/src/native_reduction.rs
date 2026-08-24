// SPDX-License-Identifier: AGPL-3.0-or-later

//! Native semantic primitives for Java's `ReductionStep`.
//!
//! The dependency-light lifecycle in [`crate::reduction_step`] owns sheet and
//! system ordering.  This module starts the production bridge from terminal
//! native STEMS SIGs with Java's deterministic `SIGraph.reduceExclusions()`
//! algorithm.  Overlap discovery and the foundation-specific consistency
//! passes remain later REDUCTION boundaries.

use crate::native_sig::{
    NativeSigContextualization, NativeSigEdgeId, NativeSigError, NativeSigInterKind,
    NativeSigRelationKind, NativeSigSystem, NativeSigVertexId,
};
use crate::native_stems::NativeStemsRecognition;

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
        for &ensemble in &dying_ensembles {
            sig.remove_vertex(ensemble)?;
        }
        sig.remove_vertex(weaker)?;

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
        NativeSigBounds, NativeSigEdge, NativeSigRelationOrigin, NativeSigSupport, NativeSigVertex,
    };

    fn vertex(ordinal: usize, kind: NativeSigInterKind, grade: f64) -> NativeSigVertex {
        NativeSigVertex {
            ordinal,
            active: true,
            removed: false,
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
}
