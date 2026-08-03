// SPDX-License-Identifier: AGPL-3.0-or-later

//! Neutral directed-graph seam for Java `grid.PeakGraph`.
//!
//! Java uses JGraphT's `SimpleDirectedGraph<StaffPeak, BarAlignment>`: vertices
//! use `StaffPeak` equality, self-loops and parallel edges are forbidden, and
//! removing a vertex removes all incident edges. This module preserves those
//! contracts and explicit insertion order without owning sheet/projector/UI
//! state. The generic relation payload can later be the neutral bar alignment.

use std::{error::Error, fmt};

use crate::bar_alignment::{BarAlignment, BarAlignmentKind, VerticalSide};
use crate::staff_peak::{StaffPeak, StaffPeakKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PeakEdgeId(usize);

impl PeakEdgeId {
    #[must_use]
    pub const fn value(self) -> usize {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeakGraphEdge<R> {
    id: PeakEdgeId,
    source: StaffPeakKey,
    target: StaffPeakKey,
    relation: R,
}

impl<R> PeakGraphEdge<R> {
    #[must_use]
    pub const fn id(&self) -> PeakEdgeId {
        self.id
    }

    #[must_use]
    pub const fn source(&self) -> StaffPeakKey {
        self.source
    }

    #[must_use]
    pub const fn target(&self) -> StaffPeakKey {
        self.target
    }

    #[must_use]
    pub const fn relation(&self) -> &R {
        &self.relation
    }
}

#[derive(Clone, Debug)]
pub struct PeakGraph<R> {
    vertices: Vec<StaffPeak>,
    edges: Vec<PeakGraphEdge<R>>,
    next_edge_id: usize,
}

impl<R> Default for PeakGraph<R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R> PeakGraph<R> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            vertices: Vec::new(),
            edges: Vec::new(),
            next_edge_id: 1,
        }
    }

    /// Java `addVertex`: an equal vertex is retained and reports `false`.
    pub fn add_vertex(&mut self, peak: StaffPeak) -> bool {
        if self.contains_vertex(peak.key()) {
            false
        } else {
            self.vertices.push(peak);
            true
        }
    }

    #[must_use]
    pub fn contains_vertex(&self, key: StaffPeakKey) -> bool {
        self.vertex(key).is_some()
    }

    #[must_use]
    pub fn vertex(&self, key: StaffPeakKey) -> Option<&StaffPeak> {
        self.vertices.iter().find(|peak| peak.key() == key)
    }

    pub fn vertex_mut(&mut self, key: StaffPeakKey) -> Option<&mut StaffPeak> {
        self.vertices.iter_mut().find(|peak| peak.key() == key)
    }

    /// Vertices in successful insertion order.
    #[must_use]
    pub fn vertices(&self) -> &[StaffPeak] {
        &self.vertices
    }

    /// Java simple-directed-graph insertion: endpoints must already exist,
    /// loops are forbidden, and only one directed edge can join a pair.
    pub fn add_edge(
        &mut self,
        source: StaffPeakKey,
        target: StaffPeakKey,
        relation: R,
    ) -> Result<PeakEdgeId, PeakGraphError> {
        self.validate_new_edge(source, target)?;
        let id = self.allocate_edge_id()?;
        self.edges.push(PeakGraphEdge {
            id,
            source,
            target,
            relation,
        });
        Ok(id)
    }

    #[must_use]
    pub fn edges(&self) -> &[PeakGraphEdge<R>] {
        &self.edges
    }

    #[must_use]
    pub fn edge(&self, id: PeakEdgeId) -> Option<&PeakGraphEdge<R>> {
        self.edges.iter().find(|edge| edge.id == id)
    }

    #[must_use]
    pub fn edge_between(
        &self,
        source: StaffPeakKey,
        target: StaffPeakKey,
    ) -> Option<&PeakGraphEdge<R>> {
        self.edges
            .iter()
            .find(|edge| edge.source == source && edge.target == target)
    }

    pub fn incoming_edges(
        &self,
        target: StaffPeakKey,
    ) -> Result<impl Iterator<Item = &PeakGraphEdge<R>>, PeakGraphError> {
        self.require_vertex(target)?;
        Ok(self.edges.iter().filter(move |edge| edge.target == target))
    }

    pub fn outgoing_edges(
        &self,
        source: StaffPeakKey,
    ) -> Result<impl Iterator<Item = &PeakGraphEdge<R>>, PeakGraphError> {
        self.require_vertex(source)?;
        Ok(self.edges.iter().filter(move |edge| edge.source == source))
    }

    pub fn in_degree(&self, target: StaffPeakKey) -> Result<usize, PeakGraphError> {
        Ok(self.incoming_edges(target)?.count())
    }

    pub fn out_degree(&self, source: StaffPeakKey) -> Result<usize, PeakGraphError> {
        Ok(self.outgoing_edges(source)?.count())
    }

    /// Java's connection promotion removes the alignment then adds a new edge.
    /// The replacement therefore receives a fresh ID and moves to the end of
    /// the stable edge sequence.
    pub fn replace_edge(
        &mut self,
        id: PeakEdgeId,
        relation: R,
    ) -> Result<(R, PeakEdgeId), PeakGraphError> {
        let index = self.edge_index(id).ok_or(PeakGraphError::MissingEdge(id))?;
        let inserted_id = self.allocate_edge_id()?;
        let removed = self.edges.remove(index);
        self.edges.push(PeakGraphEdge {
            id: inserted_id,
            source: removed.source,
            target: removed.target,
            relation,
        });
        Ok((removed.relation, inserted_id))
    }

    pub fn remove_edge(&mut self, id: PeakEdgeId) -> Option<PeakGraphEdge<R>> {
        self.edge_index(id).map(|index| self.edges.remove(index))
    }

    /// Java `removeVertex`: absent vertices report `None`; removal cascades to
    /// every incoming and outgoing edge while preserving survivor order.
    pub fn remove_vertex(&mut self, key: StaffPeakKey) -> Option<StaffPeak> {
        let index = self.vertices.iter().position(|peak| peak.key() == key)?;
        let removed = self.vertices.remove(index);
        self.edges
            .retain(|edge| edge.source != key && edge.target != key);
        Some(removed)
    }

    fn validate_new_edge(
        &self,
        source: StaffPeakKey,
        target: StaffPeakKey,
    ) -> Result<(), PeakGraphError> {
        self.require_vertex(source)?;
        self.require_vertex(target)?;
        if source == target {
            return Err(PeakGraphError::SelfLoop(source));
        }
        if self.edge_between(source, target).is_some() {
            return Err(PeakGraphError::DuplicateEdge { source, target });
        }
        Ok(())
    }

    fn require_vertex(&self, key: StaffPeakKey) -> Result<(), PeakGraphError> {
        if self.contains_vertex(key) {
            Ok(())
        } else {
            Err(PeakGraphError::MissingVertex(key))
        }
    }

    fn edge_index(&self, id: PeakEdgeId) -> Option<usize> {
        self.edges.iter().position(|edge| edge.id == id)
    }

    fn allocate_edge_id(&mut self) -> Result<PeakEdgeId, PeakGraphError> {
        let id = PeakEdgeId(self.next_edge_id);
        self.next_edge_id = self
            .next_edge_id
            .checked_add(1)
            .ok_or(PeakGraphError::EdgeIdExhausted)?;
        Ok(id)
    }
}

impl PeakGraph<BarAlignment> {
    /// Java `PeakGraph.getConnectedPeaks`: retain concrete connections on the
    /// requested side and return partner peaks in `StaffPeak.compareTo` order.
    pub fn connected_peaks(
        &self,
        peak: StaffPeakKey,
        side: VerticalSide,
    ) -> Result<Vec<&StaffPeak>, PeakGraphError> {
        self.require_vertex(peak)?;
        let mut partners = self
            .edges
            .iter()
            .filter(|edge| edge.relation.kind() == BarAlignmentKind::Connection)
            .filter_map(|edge| match side {
                VerticalSide::Top if edge.target == peak => Some(edge.source),
                VerticalSide::Bottom if edge.source == peak => Some(edge.target),
                _ => None,
            })
            .collect::<Vec<_>>();
        partners.sort_unstable();
        partners.dedup();
        Ok(partners
            .into_iter()
            .map(|key| {
                self.vertex(key)
                    .expect("peak graph edges always reference present vertices")
            })
            .collect())
    }

    /// Java `BarsRetriever.isConnected`: direction matters and alignments do
    /// not count until promoted to concrete connections.
    pub fn is_connected(
        &self,
        peak: StaffPeakKey,
        side: VerticalSide,
    ) -> Result<bool, PeakGraphError> {
        self.require_vertex(peak)?;
        Ok(self.edges.iter().any(|edge| {
            edge.relation.kind() == BarAlignmentKind::Connection
                && match side {
                    VerticalSide::Top => edge.target == peak,
                    VerticalSide::Bottom => edge.source == peak,
                }
        }))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeakGraphError {
    MissingVertex(StaffPeakKey),
    SelfLoop(StaffPeakKey),
    DuplicateEdge {
        source: StaffPeakKey,
        target: StaffPeakKey,
    },
    MissingEdge(PeakEdgeId),
    EdgeIdExhausted,
}

impl fmt::Display for PeakGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingVertex(key) => write!(formatter, "missing peak vertex {key:?}"),
            Self::SelfLoop(key) => write!(formatter, "peak graph forbids self-loop at {key:?}"),
            Self::DuplicateEdge { source, target } => {
                write!(
                    formatter,
                    "duplicate directed peak edge {source:?} -> {target:?}"
                )
            }
            Self::MissingEdge(id) => write!(formatter, "missing peak edge {}", id.value()),
            Self::EdgeIdExhausted => formatter.write_str("peak edge ID space exhausted"),
        }
    }
}

impl Error for PeakGraphError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bar_alignment::{AlignmentPeak, BarImpacts};
    use crate::bar_column::{PeakId, StaffId};
    use crate::staff_peak::StaffPeakAttribute;

    fn peak(staff: usize, start: i32, stop: i32) -> StaffPeak {
        StaffPeak::new(StaffId::new(staff), 10, 20, start, stop).unwrap()
    }

    fn relation(
        id: usize,
        top: StaffPeakKey,
        bottom: StaffPeakKey,
        kind: BarAlignmentKind,
    ) -> BarAlignment {
        let alignment = BarAlignment::new(
            AlignmentPeak::new(PeakId::new(id), top.staff_id(), top.start(), 1.0).unwrap(),
            AlignmentPeak::new(
                PeakId::new(id + 100),
                bottom.staff_id(),
                bottom.start(),
                1.0,
            )
            .unwrap(),
            0.0,
            0.0,
            BarImpacts::alignment(1.0, 1.0).unwrap(),
        )
        .unwrap();
        match kind {
            BarAlignmentKind::Alignment => alignment,
            BarAlignmentKind::Connection => BarAlignment::connection(&alignment, 1.0, 1.0).unwrap(),
        }
    }

    #[test]
    fn vertex_insertion_is_stable_and_equal_duplicates_do_not_replace() {
        let mut graph = PeakGraph::<&'static str>::new();
        let first = peak(2, 30, 31);
        let first_key = first.key();
        let second = peak(1, 10, 12);
        assert!(graph.add_vertex(first));
        assert!(graph.add_vertex(second));

        let mut duplicate = StaffPeak::new(StaffId::new(2), -100, 100, 30, 31).unwrap();
        duplicate.set(StaffPeakAttribute::Brace);
        assert!(!graph.add_vertex(duplicate));
        assert_eq!(graph.vertices().len(), 2);
        assert_eq!(graph.vertices()[0].key(), first_key);
        assert_eq!(graph.vertices()[1].staff_id(), StaffId::new(1));
        assert!(!graph.vertex(first_key).unwrap().is_brace());

        graph
            .vertex_mut(first_key)
            .unwrap()
            .set(StaffPeakAttribute::Thick);
        assert!(
            graph
                .vertex(first_key)
                .unwrap()
                .is_set(StaffPeakAttribute::Thick)
        );
    }

    #[test]
    fn simple_directed_edge_rules_fail_without_partial_mutation() {
        let mut graph = PeakGraph::new();
        let top = peak(1, 10, 11);
        let bottom = peak(2, 10, 11);
        let absent = peak(3, 10, 11).key();
        let top_key = top.key();
        let bottom_key = bottom.key();
        graph.add_vertex(top);
        graph.add_vertex(bottom);

        assert_eq!(
            graph.add_edge(absent, bottom_key, "missing"),
            Err(PeakGraphError::MissingVertex(absent))
        );
        assert_eq!(
            graph.add_edge(top_key, top_key, "loop"),
            Err(PeakGraphError::SelfLoop(top_key))
        );
        let id = graph.add_edge(top_key, bottom_key, "alignment").unwrap();
        assert_eq!(id.value(), 1);
        assert_eq!(
            graph.add_edge(top_key, bottom_key, "parallel"),
            Err(PeakGraphError::DuplicateEdge {
                source: top_key,
                target: bottom_key,
            })
        );
        assert_eq!(graph.edges().len(), 1);

        // The opposite direction is a distinct directed pair.
        assert_eq!(
            graph
                .add_edge(bottom_key, top_key, "reverse")
                .unwrap()
                .value(),
            2
        );
    }

    #[test]
    fn directional_queries_preserve_edge_insertion_order() {
        let mut graph = PeakGraph::new();
        let a = peak(1, 10, 11);
        let b = peak(2, 10, 11);
        let c = peak(2, 20, 21);
        let d = peak(3, 10, 11);
        let keys = [a.key(), b.key(), c.key(), d.key()];
        for vertex in [a, b, c, d] {
            graph.add_vertex(vertex);
        }

        graph.add_edge(keys[0], keys[1], "a-b").unwrap();
        graph.add_edge(keys[0], keys[2], "a-c").unwrap();
        graph.add_edge(keys[2], keys[3], "c-d").unwrap();
        graph.add_edge(keys[1], keys[3], "b-d").unwrap();

        assert_eq!(graph.out_degree(keys[0]).unwrap(), 2);
        assert_eq!(graph.in_degree(keys[3]).unwrap(), 2);
        assert_eq!(
            graph
                .outgoing_edges(keys[0])
                .unwrap()
                .map(|edge| *edge.relation())
                .collect::<Vec<_>>(),
            ["a-b", "a-c"]
        );
        assert_eq!(
            graph
                .incoming_edges(keys[3])
                .unwrap()
                .map(|edge| *edge.relation())
                .collect::<Vec<_>>(),
            ["c-d", "b-d"]
        );
        assert_eq!(
            graph.in_degree(peak(9, 0, 1).key()),
            Err(PeakGraphError::MissingVertex(peak(9, 0, 1).key()))
        );
    }

    #[test]
    fn connection_promotion_removes_then_appends_with_a_fresh_identity() {
        let mut graph = PeakGraph::new();
        let a = peak(1, 10, 11);
        let b = peak(2, 10, 11);
        let c = peak(3, 10, 11);
        let keys = [a.key(), b.key(), c.key()];
        for vertex in [a, b, c] {
            graph.add_vertex(vertex);
        }
        let promoted = graph.add_edge(keys[0], keys[1], "alignment").unwrap();
        let other = graph.add_edge(keys[1], keys[2], "other").unwrap();

        let (old, connection) = graph.replace_edge(promoted, "connection").unwrap();
        assert_eq!(old, "alignment");
        assert_eq!(connection.value(), 3);
        assert!(graph.edge(promoted).is_none());
        assert_eq!(
            graph
                .edges()
                .iter()
                .map(|edge| (edge.id(), *edge.relation()))
                .collect::<Vec<_>>(),
            [(other, "other"), (connection, "connection")]
        );
        assert_eq!(
            graph.replace_edge(promoted, "impossible"),
            Err(PeakGraphError::MissingEdge(promoted))
        );
    }

    #[test]
    fn vertex_removal_cascades_incident_edges_and_preserves_survivors() {
        let mut graph = PeakGraph::new();
        let a = peak(1, 10, 11);
        let b = peak(2, 10, 11);
        let c = peak(3, 10, 11);
        let keys = [a.key(), b.key(), c.key()];
        for vertex in [a, b, c] {
            graph.add_vertex(vertex);
        }
        graph.add_edge(keys[0], keys[1], "drop-in").unwrap();
        graph.add_edge(keys[1], keys[2], "drop-out").unwrap();
        let survivor = graph.add_edge(keys[0], keys[2], "keep").unwrap();

        assert_eq!(graph.remove_vertex(keys[1]).unwrap().key(), keys[1]);
        assert_eq!(
            graph
                .vertices()
                .iter()
                .map(StaffPeak::key)
                .collect::<Vec<_>>(),
            [keys[0], keys[2]]
        );
        assert_eq!(graph.edges().len(), 1);
        assert_eq!(graph.edges()[0].id(), survivor);
        assert_eq!(*graph.edges()[0].relation(), "keep");
        assert!(graph.remove_vertex(keys[1]).is_none());
        assert!(graph.remove_edge(survivor).is_some());
        assert!(graph.remove_edge(survivor).is_none());
    }

    #[test]
    fn connected_peak_queries_filter_alignments_respect_side_and_sort_partners() {
        let mut graph = PeakGraph::new();
        let focus = peak(2, 50, 51);
        let top_late = peak(1, 30, 31);
        let top_alignment = peak(1, 20, 21);
        let top_early = peak(1, 10, 11);
        let bottom_late = peak(3, 40, 41);
        let bottom_early = peak(3, 5, 6);
        let focus_key = focus.key();
        let keys = [
            top_late.key(),
            top_alignment.key(),
            top_early.key(),
            bottom_late.key(),
            bottom_early.key(),
        ];
        for vertex in [
            focus,
            top_late,
            top_alignment,
            top_early,
            bottom_late,
            bottom_early,
        ] {
            graph.add_vertex(vertex);
        }

        graph
            .add_edge(
                keys[0],
                focus_key,
                relation(1, keys[0], focus_key, BarAlignmentKind::Connection),
            )
            .unwrap();
        graph
            .add_edge(
                keys[1],
                focus_key,
                relation(2, keys[1], focus_key, BarAlignmentKind::Alignment),
            )
            .unwrap();
        graph
            .add_edge(
                keys[2],
                focus_key,
                relation(3, keys[2], focus_key, BarAlignmentKind::Connection),
            )
            .unwrap();
        graph
            .add_edge(
                focus_key,
                keys[3],
                relation(4, focus_key, keys[3], BarAlignmentKind::Connection),
            )
            .unwrap();
        graph
            .add_edge(
                focus_key,
                keys[4],
                relation(5, focus_key, keys[4], BarAlignmentKind::Connection),
            )
            .unwrap();

        assert_eq!(
            graph
                .connected_peaks(focus_key, VerticalSide::Top)
                .unwrap()
                .into_iter()
                .map(StaffPeak::key)
                .collect::<Vec<_>>(),
            [keys[2], keys[0]]
        );
        assert_eq!(
            graph
                .connected_peaks(focus_key, VerticalSide::Bottom)
                .unwrap()
                .into_iter()
                .map(StaffPeak::key)
                .collect::<Vec<_>>(),
            [keys[4], keys[3]]
        );
        assert!(graph.is_connected(focus_key, VerticalSide::Top).unwrap());
        assert!(graph.is_connected(focus_key, VerticalSide::Bottom).unwrap());
        assert!(!graph.is_connected(keys[1], VerticalSide::Bottom).unwrap());

        let missing = peak(9, 0, 1).key();
        assert!(matches!(
            graph.connected_peaks(missing, VerticalSide::Top),
            Err(PeakGraphError::MissingVertex(key)) if key == missing
        ));
        assert_eq!(
            graph.is_connected(missing, VerticalSide::Top),
            Err(PeakGraphError::MissingVertex(missing))
        );
    }
}
