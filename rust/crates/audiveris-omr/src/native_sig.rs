// SPDX-License-Identifier: AGPL-3.0-or-later

//! The recognition-owned SIG at the boundary after HEADS and before STEMS.
//!
//! GRID, HEADERS, BEAMS, LEDGERS, and HEADS each expose complete typed products,
//! but historically the Rust port kept those products in separate collections.
//! This module gives their live interpretations one per-system identity domain and
//! preserves the insertion order Java's `SIGraph` exposes to later `edgesOf` scans.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use audiveris_image::{
    bars_logic::{BracketKind, ConnectorInterKind, PeakWidthClass, VerticalInterKind},
    grid_sig::{GridInterId, GridSigNode, GridSigRelation},
    system_population::StaffBoundary,
};
use audiveris_music_font::{MusicFamily, layout_bounds};

use crate::{
    beam_inters::{BeamKind, RawBeam, beam_bounds},
    clef_column::NeutralClefKind,
    head_template::HeadTemplateShape,
    header_time_column::{NeutralSpecificTimeShape, NeutralTimeCandidate},
    native_headers::{NativeHeaderRecognition, NativeHeaderStaffRecognition},
    native_heads::NativeHeadsRecognition,
    native_heads_competitors::NativeHeadsCompetitorSource,
    native_heads_staff_epilog::NativeHeadStaffEpilogRef,
    native_ledgers::NativeLedgerRecognition,
    native_reduction::{
        NativeReductionAreaGeometry, NativeReductionAreaPoint, NativeReductionGlyphGeometry,
        NativeReductionInterGeometry,
    },
    native_stems_beam_stumps::NativeStemsBeamSource,
    recognize::{GridLinesRecognition, NativeBeamRecognition},
    stems_step::{NativeBeamPortion, NativeStemHeadSide, NativeStemPoint},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeSigBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl NativeSigBounds {
    fn union(self, other: Self) -> Self {
        let right = self
            .x
            .saturating_add(self.width)
            .max(other.x.saturating_add(other.width));
        let bottom = self
            .y
            .saturating_add(self.height)
            .max(other.y.saturating_add(other.height));
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self {
            x,
            y,
            width: right.saturating_sub(x),
            height: bottom.saturating_sub(y),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSigBeamGeometry {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NativeSigVertexId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NativeSigEdgeId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSigInterKind {
    Brace,
    Barline,
    Bracket,
    BarConnector,
    BracketConnector,
    Clef,
    KeyAlter,
    Key,
    TimeWhole,
    TimePair,
    Beam,
    BeamHook,
    SmallBeam,
    BeamGroup,
    Ledger,
    Head,
    Stem,
}

impl NativeSigInterKind {
    #[must_use]
    pub const fn java_class(self) -> &'static str {
        match self {
            Self::Brace => "BraceInter",
            Self::Barline => "BarlineInter",
            Self::Bracket => "BracketInter",
            Self::BarConnector => "BarConnectorInter",
            Self::BracketConnector => "BracketConnectorInter",
            Self::Clef => "ClefInter",
            Self::KeyAlter => "KeyAlterInter",
            Self::Key => "KeyInter",
            Self::TimeWhole => "TimeWholeInter",
            Self::TimePair => "TimePairInter",
            Self::Beam => "BeamInter",
            Self::BeamHook => "BeamHookInter",
            Self::SmallBeam => "SmallBeamInter",
            Self::BeamGroup => "BeamGroupInter",
            Self::Ledger => "LedgerInter",
            Self::Head => "HeadInter",
            Self::Stem => "StemInter",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeSigVertex {
    /// Zero-based system-local insertion ordinal.
    pub ordinal: usize,
    /// Live graph membership. Removal never renumbers this stable slot.
    pub active: bool,
    pub removed: bool,
    /// Java `Inter.isFrozen()`. GRID supplies its measured frozen bit and all
    /// selected header inters are frozen by `StaffHeader.freeze()`.
    pub frozen: bool,
    pub kind: NativeSigInterKind,
    /// Java `Shape.name()`, or `None` for shape-less ensembles.
    pub shape: Option<String>,
    pub grade: f64,
    /// Java `Inter.contextualGrade`, recomputed by the STEMS sheet epilog.
    pub contextual_grade: Option<f64>,
    pub bounds: NativeSigBounds,
    pub abnormal: bool,
    pub beam_geometry: Option<NativeSigBeamGeometry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSigRelationKind {
    NoExclusion,
    BarConnection,
    BarGroup,
    KeyAlters,
    Containment,
    ClefKey,
    Exclusion,
    BeamBeam,
    BeamStem,
    BeamHead,
    BeamRest,
    HeadStem,
    HeadHead,
    ChordStem,
}

/// Native origin of a relation object in the insertion-ordered SIG.
///
/// This is deliberately independent of Java object identity. Later STEMS
/// boundaries nevertheless distinguish a relation that was already in the
/// graph from the still-live draft object inserted by B14/B16/B17.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSigRelationOrigin {
    BaselineGraph,
    BeamVBaseDraft {
        plan_ordinal: usize,
    },
    BeamVSiblingDraft {
        plan_ordinal: usize,
        sibling_ordinal: usize,
    },
    BeamVHeadDraft {
        plan_ordinal: usize,
        map_ordinal: usize,
    },
    HeadCLinkDraft {
        head_sig_ordinal: usize,
        constructor_ordinal: usize,
    },
    HeadCLinkBeamDraft {
        head_sig_ordinal: usize,
        beam_linker_id: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSigBarConnectionImpacts {
    pub align: f64,
    pub width: f64,
    pub gap: f64,
    pub white: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSigSupport {
    pub grade: f64,
    pub bar_connection_impacts: Option<NativeSigBarConnectionImpacts>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSigHeadStemPayload {
    pub dx: f64,
    pub dy: f64,
    pub head_side: NativeStemHeadSide,
    pub extension_point: NativeStemPoint,
    pub consistency: f64,
    pub manual: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSigEdge {
    pub ordinal: usize,
    /// Live graph membership. Removal leaves an insertion-order tombstone.
    pub active: bool,
    pub source: usize,
    pub target: usize,
    pub kind: NativeSigRelationKind,
    pub origin: NativeSigRelationOrigin,
    pub support: Option<NativeSigSupport>,
    /// Payload retained by BeamStem/BeamRest relations for later abnormal scans.
    pub beam_portion: Option<NativeBeamPortion>,
    /// BeamStem extension retained for later callback and reuse projections.
    pub stem_extension: Option<NativeStemPoint>,
    /// HeadStem fields retained for later reuse and callback projections.
    pub head_stem: Option<NativeSigHeadStemPayload>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeSigSystem {
    pub system_id: usize,
    pub vertices: Vec<NativeSigVertex>,
    pub edges: Vec<NativeSigEdge>,
}

impl NativeSigSystem {
    #[must_use]
    pub fn vertex(&self, ordinal: usize) -> Option<&NativeSigVertex> {
        self.vertices.get(ordinal).filter(|vertex| vertex.active)
    }

    /// Java/JGraphT `incomingEdgesOf`, retaining global edge insertion order.
    pub fn incoming_edges(&self, vertex: usize) -> Result<Vec<&NativeSigEdge>, NativeSigError> {
        self.require_vertex(vertex)?;
        Ok(self
            .edges
            .iter()
            .filter(|edge| edge.active && edge.target == vertex)
            .collect())
    }

    /// Java/JGraphT `outgoingEdgesOf`, retaining global edge insertion order.
    pub fn outgoing_edges(&self, vertex: usize) -> Result<Vec<&NativeSigEdge>, NativeSigError> {
        self.require_vertex(vertex)?;
        Ok(self
            .edges
            .iter()
            .filter(|edge| edge.active && edge.source == vertex)
            .collect())
    }

    /// Java/JGraphT `edgesOf`: incoming insertion order followed by outgoing order.
    pub fn incident_edges(&self, vertex: usize) -> Result<Vec<&NativeSigEdge>, NativeSigError> {
        self.require_vertex(vertex)?;
        Ok(self
            .edges
            .iter()
            .filter(|edge| edge.active && edge.target == vertex)
            .chain(
                self.edges
                    .iter()
                    .filter(|edge| edge.active && edge.source == vertex),
            )
            .collect())
    }

    /// Java/JGraphT directed pair query, in source-outgoing insertion order.
    pub fn directed_edges(
        &self,
        source: usize,
        target: usize,
    ) -> Result<Vec<&NativeSigEdge>, NativeSigError> {
        self.require_vertex(source)?;
        self.require_vertex(target)?;
        Ok(self
            .edges
            .iter()
            .filter(|edge| edge.active && edge.source == source && edge.target == target)
            .collect())
    }

    /// Append one live vertex while preserving the graph's dense identity domain.
    pub fn append_vertex(&mut self, vertex: NativeSigVertex) -> Result<(), NativeSigError> {
        if vertex.ordinal != self.vertices.len() {
            return Err(NativeSigError::InvalidVertexOrdinal {
                system_id: self.system_id,
                expected: self.vertices.len(),
                actual: vertex.ordinal,
            });
        }
        if !vertex.active || vertex.removed {
            return Err(NativeSigError::InvalidVertexState {
                system_id: self.system_id,
                ordinal: vertex.ordinal,
            });
        }
        self.vertices.push(vertex);
        Ok(())
    }

    /// Append one live relation in global insertion order.
    pub fn append_edge(&mut self, edge: NativeSigEdge) -> Result<(), NativeSigError> {
        if edge.ordinal != self.edges.len() {
            return Err(NativeSigError::InvalidEdgeOrdinal {
                system_id: self.system_id,
                expected: self.edges.len(),
                actual: edge.ordinal,
            });
        }
        self.require_vertex(edge.source)?;
        self.require_vertex(edge.target)?;
        if edge.source == edge.target {
            return Err(NativeSigError::SelfLoop {
                system_id: self.system_id,
                ordinal: edge.ordinal,
            });
        }
        if !edge.active {
            return Err(NativeSigError::InvalidEdgeState {
                system_id: self.system_id,
                ordinal: edge.ordinal,
            });
        }
        if !valid_edge_payload(&edge) {
            return Err(NativeSigError::InvalidRelationPayload {
                system_id: self.system_id,
                ordinal: edge.ordinal,
            });
        }
        self.edges.push(edge);
        Ok(())
    }

    pub fn validate_integrity(&self) -> Result<(), NativeSigError> {
        for (ordinal, vertex) in self.vertices.iter().enumerate() {
            if vertex.ordinal != ordinal || vertex.active == vertex.removed {
                return Err(NativeSigError::InvalidVertexState {
                    system_id: self.system_id,
                    ordinal,
                });
            }
        }
        for (ordinal, edge) in self.edges.iter().enumerate() {
            if edge.ordinal != ordinal
                || edge.source == edge.target
                || !valid_edge_payload(edge)
                || (edge.active
                    && (self.vertex(edge.source).is_none() || self.vertex(edge.target).is_none()))
            {
                return Err(NativeSigError::InvalidEdgeState {
                    system_id: self.system_id,
                    ordinal,
                });
            }
        }
        Ok(())
    }

    pub fn remove_edge(&mut self, id: NativeSigEdgeId) -> Result<(), NativeSigError> {
        let edge = self.edges.get_mut(id.0).filter(|edge| edge.active).ok_or(
            NativeSigError::MissingEdge {
                system_id: self.system_id,
                ordinal: id.0,
            },
        )?;
        edge.active = false;
        Ok(())
    }

    /// Tombstone a vertex and all incident edges without renumbering later objects.
    pub fn remove_vertex(&mut self, id: NativeSigVertexId) -> Result<(), NativeSigError> {
        let vertex = self
            .vertices
            .get_mut(id.0)
            .filter(|vertex| vertex.active)
            .ok_or(NativeSigError::MissingVertex {
                system_id: self.system_id,
                ordinal: id.0,
            })?;
        vertex.active = false;
        vertex.removed = true;
        for edge in &mut self.edges {
            if edge.active && (edge.source == id.0 || edge.target == id.0) {
                edge.active = false;
            }
        }
        Ok(())
    }

    pub fn set_abnormal(
        &mut self,
        id: NativeSigVertexId,
        abnormal: bool,
    ) -> Result<(), NativeSigError> {
        let vertex = self
            .vertices
            .get_mut(id.0)
            .filter(|vertex| vertex.active)
            .ok_or(NativeSigError::MissingVertex {
                system_id: self.system_id,
                ordinal: id.0,
            })?;
        vertex.abnormal = abnormal;
        Ok(())
    }

    /// Java `SIGraph.contextualize()`: compute every live inter's contextual
    /// grade from intrinsic partner grades and support/exclusion topology.
    pub fn contextualize(&mut self) -> NativeSigContextualization {
        let before = self
            .vertices
            .iter()
            .map(|vertex| vertex.contextual_grade)
            .collect::<Vec<_>>();
        let grades = self
            .vertices
            .iter()
            .map(|vertex| vertex.grade)
            .collect::<Vec<_>>();
        let active = self
            .vertices
            .iter()
            .map(|vertex| vertex.active)
            .collect::<Vec<_>>();
        let kinds = self
            .vertices
            .iter()
            .map(|vertex| vertex.kind)
            .collect::<Vec<_>>();
        let mut contextual = (0..self.vertices.len())
            .map(|focus| {
                active[focus]
                    .then(|| contextual_grade(self, focus, &grades, &active, &kinds))
                    .flatten()
            })
            .collect::<Vec<_>>();

        // BeamGroupInter overrides AbstractInter#getContextualGrade(): its visible
        // value is the arithmetic mean of the best grades of its live contained
        // beams, rather than the intrinsic grade stored by SIGraph.contextualize().
        // Apply that ensemble view only after every ordinary vertex has its fresh
        // contextual grade, matching EnsembleHelper.computeMeanContextualGrade().
        for group in 0..self.vertices.len() {
            if !active[group] || kinds[group] != NativeSigInterKind::BeamGroup {
                continue;
            }
            let mut sum = 0.0;
            let mut count = 0_usize;
            for edge in self.edges.iter().filter(|edge| {
                edge.active
                    && edge.kind == NativeSigRelationKind::Containment
                    && edge.source == group
                    && active.get(edge.target).copied().unwrap_or(false)
            }) {
                sum += contextual[edge.target].unwrap_or(grades[edge.target]);
                count += 1;
            }
            contextual[group] = (count > 0).then_some(sum / count as f64);
        }
        for (vertex, grade) in self.vertices.iter_mut().zip(&contextual) {
            if vertex.active {
                vertex.contextual_grade = *grade;
            }
        }
        NativeSigContextualization {
            system_id: self.system_id,
            contextualized_vertices: active.into_iter().filter(|active| *active).count(),
            changed_values: before
                .iter()
                .zip(&contextual)
                .filter(|(before, after)| before != after)
                .count(),
            contextual_grade_digest: contextual_grade_digest(&contextual),
        }
    }

    fn require_vertex(&self, ordinal: usize) -> Result<(), NativeSigError> {
        if self.vertex(ordinal).is_some() {
            Ok(())
        } else {
            Err(NativeSigError::MissingVertex {
                system_id: self.system_id,
                ordinal,
            })
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeSigContextualization {
    pub system_id: usize,
    pub contextualized_vertices: usize,
    pub changed_values: usize,
    /// FNV-1a-64 over sorted live contextual-grade IEEE-754 big-endian bits.
    pub contextual_grade_digest: u64,
}

fn contextual_grade_digest(grades: &[Option<f64>]) -> u64 {
    let mut bits = grades
        .iter()
        .filter_map(|grade| grade.map(f64::to_bits))
        .collect::<Vec<_>>();
    bits.sort_unstable();
    let mut digest = 0xcbf2_9ce4_8422_2325_u64;
    for value in bits {
        for byte in value.to_be_bytes() {
            digest ^= u64::from(byte);
            digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    digest
}

fn contextual_grade(
    sig: &NativeSigSystem,
    focus: usize,
    grades: &[f64],
    active: &[bool],
    kinds: &[NativeSigInterKind],
) -> Option<f64> {
    let mut partners = Vec::new();
    let mut contributions = BTreeMap::new();
    for edge in sig.edges.iter().filter(|edge| edge.active) {
        let Some(support) = edge.support else {
            continue;
        };
        let (partner, ratio) = if edge.target == focus {
            (edge.source, native_support_ratio(edge, false))
        } else if edge.source == focus {
            (edge.target, native_support_ratio(edge, true))
        } else {
            continue;
        };
        if ratio > 1.0 && active.get(partner).copied().unwrap_or(false) {
            partners.push(partner);
            contributions.insert(partner, grades[partner] * (ratio - 1.0));
        }
        let _ = support;
    }
    partners.sort_by(|left, right| grades[*right].total_cmp(&grades[*left]));
    let n = partners.len();
    let mut concurrent_sets = vec![BTreeSet::new(); n];
    let mut conflict = false;
    for i in 0..n {
        let partner = partners[i];
        for edge in sig.edges.iter().filter(|edge| {
            edge.active
                && edge.kind == NativeSigRelationKind::Exclusion
                && (edge.source == partner || edge.target == partner)
        }) {
            let other = if edge.source == partner {
                edge.target
            } else {
                edge.source
            };
            if let Some(index) = partners.iter().position(|candidate| *candidate == other)
                && index > i
            {
                concurrent_sets[i].insert(index);
                conflict = true;
            }
        }
        if kinds[focus] == NativeSigInterKind::Head && kinds[partner] == NativeSigInterKind::Stem {
            for stem in partners
                .iter()
                .copied()
                .filter(|candidate| kinds[*candidate] == NativeSigInterKind::Stem)
            {
                if let Some(index) = partners.iter().position(|candidate| *candidate == stem)
                    && index > i
                {
                    concurrent_sets[i].insert(index);
                    conflict = true;
                }
            }
        }
    }

    let partitions = if conflict {
        let mut sequences = vec![vec![0_i8; n]];
        for i in 0..n {
            let stop = sequences.len();
            for index in 0..stop {
                if sequences[index][i] == -1 {
                    continue;
                }
                sequences[index][i] = 1;
                if !concurrent_sets[i].is_empty() {
                    let mut without = sequences[index].clone();
                    without[i] = 0;
                    sequences.push(without);
                    for &other in &concurrent_sets[i] {
                        sequences[index][other] = -1;
                    }
                }
            }
        }
        sequences
    } else {
        vec![vec![1_i8; n]]
    };
    let mut best = 0.0_f64;
    for partition in partitions {
        let contribution = partition
            .iter()
            .enumerate()
            .filter(|(_, selected)| **selected == 1)
            .map(|(index, _)| contributions[&partners[index]])
            .sum::<f64>();
        best = best.max(audiveris_core::grade::contextual(
            grades[focus],
            contribution,
        ));
    }
    Some(best)
}

fn native_support_ratio(edge: &NativeSigEdge, focus_is_source: bool) -> f64 {
    let grade = edge.support.map_or(0.0, |support| support.grade);
    let coefficient = match edge.kind {
        NativeSigRelationKind::BarConnection
        | NativeSigRelationKind::KeyAlters
        | NativeSigRelationKind::ClefKey => 5.0,
        NativeSigRelationKind::BeamBeam => 3.0,
        NativeSigRelationKind::BeamStem if focus_is_source => 4.0,
        NativeSigRelationKind::BeamStem => {
            if edge.beam_portion == Some(NativeBeamPortion::Center) {
                3.0
            } else {
                10.0
            }
        }
        NativeSigRelationKind::HeadStem if focus_is_source => {
            4.0 * edge.head_stem.map_or(1.0, |payload| payload.consistency)
        }
        NativeSigRelationKind::HeadStem => 10.0,
        NativeSigRelationKind::HeadHead => 0.75,
        NativeSigRelationKind::BeamHead if focus_is_source => 0.0,
        NativeSigRelationKind::BeamHead => 1.0,
        NativeSigRelationKind::NoExclusion
        | NativeSigRelationKind::BarGroup
        | NativeSigRelationKind::Containment
        | NativeSigRelationKind::Exclusion
        | NativeSigRelationKind::BeamRest
        | NativeSigRelationKind::ChordStem => 0.0,
    };
    1.0 + coefficient * grade
}

fn valid_edge_payload(edge: &NativeSigEdge) -> bool {
    let valid_origin = match edge.origin {
        NativeSigRelationOrigin::BaselineGraph => true,
        NativeSigRelationOrigin::BeamVBaseDraft { .. }
        | NativeSigRelationOrigin::BeamVSiblingDraft { .. }
        | NativeSigRelationOrigin::HeadCLinkBeamDraft { .. } => {
            edge.kind == NativeSigRelationKind::BeamStem
        }
        NativeSigRelationOrigin::BeamVHeadDraft { .. }
        | NativeSigRelationOrigin::HeadCLinkDraft { .. } => {
            edge.kind == NativeSigRelationKind::HeadStem
        }
    };
    if !valid_origin {
        return false;
    }
    match edge.kind {
        NativeSigRelationKind::BeamStem => {
            edge.support.is_some()
                && edge.beam_portion.is_some()
                && edge.stem_extension.is_some()
                && edge.head_stem.is_none()
        }
        NativeSigRelationKind::BeamHead => {
            edge.support.is_some()
                && edge.beam_portion.is_none()
                && edge.stem_extension.is_none()
                && edge.head_stem.is_none()
        }
        NativeSigRelationKind::BeamRest => {
            edge.support.is_some() && edge.stem_extension.is_none() && edge.head_stem.is_none()
        }
        NativeSigRelationKind::HeadStem => {
            edge.support.is_some()
                && edge.beam_portion.is_none()
                && edge.stem_extension.is_none()
                && edge.head_stem.is_some_and(|head| {
                    head.dx.is_finite()
                        && head.dy.is_finite()
                        && head.extension_point.x.is_finite()
                        && head.extension_point.y.is_finite()
                        && head.consistency.is_finite()
                })
        }
        _ => {
            edge.beam_portion.is_none() && edge.stem_extension.is_none() && edge.head_stem.is_none()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeSigRecognition {
    pub systems: Vec<NativeSigSystem>,
    pub bindings: Vec<NativeSigSystemBindings>,
}

/// Exact staff-owned state Java's REDUCTION `checkLedgers()` reads and mutates.
///
/// A ledger can deliberately occur in two adjacent staff maps before
/// `fixAllSharedLedgers()`.  Keeping the map entries separate from each
/// ledger's single SIG identity preserves that pathological-but-supported
/// state losslessly.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeSigReductionStaff {
    pub staff_id: usize,
    pub tablature: bool,
    pub specific_interline: i32,
    pub line_count: usize,
    pub first_line: StaffBoundary,
    pub last_line: StaffBoundary,
    pub ledger_lines: BTreeMap<i32, StaffBoundary>,
    pub ledger_map: BTreeMap<i32, Vec<NativeSigVertexId>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeSigSystemBindings {
    pub system_id: usize,
    pub beam_vertices: BTreeMap<NativeStemsBeamSource, NativeSigVertexId>,
    pub beam_group_vertices: BTreeMap<usize, NativeSigVertexId>,
    pub stem_vertices: BTreeMap<usize, NativeSigVertexId>,
    pub head_vertices: BTreeMap<NativeHeadStaffEpilogRef, NativeSigVertexId>,
    /// Live `LedgerInter.id -> SIG vertex` bindings. Staff ownership is kept
    /// separately because Java temporarily permits one identity in two maps.
    pub ledger_vertices: BTreeMap<usize, NativeSigVertexId>,
    /// Sheet `Scale.getInterline()` used by orphan-ledger support boxes.
    pub reduction_interline: i32,
    /// Staff order, exact splines, full ledger-line paths and mutable ledger
    /// ownership maps consumed by REDUCTION.
    pub reduction_staffs: Vec<NativeSigReductionStaff>,
    /// Ordered first/last staff IDs for Java merged-grand-staff parts.
    ///
    /// `SigReducer.lookupHead` remaps only the two gutter pitches across each
    /// such pair, so REDUCTION needs the actual part topology rather than a
    /// staff-distance guess.
    pub merged_staff_pairs: Vec<(usize, usize)>,
    /// Exact immutable overlap evidence for vertices published before STEMS.
    /// Dynamic StemInter entries are joined from the terminal stem registry.
    pub overlap_geometry: BTreeMap<NativeSigVertexId, NativeReductionInterGeometry>,
}

impl NativeSigSystemBindings {
    pub fn validate_against(&self, sig: &NativeSigSystem) -> Result<(), NativeSigError> {
        if self.system_id != sig.system_id {
            return Err(NativeSigError::InvalidBeamSourceBinding {
                system_id: self.system_id,
            });
        }
        let bound_beams = self
            .beam_vertices
            .values()
            .map(|vertex| vertex.0)
            .collect::<BTreeSet<_>>();
        for vertex in self.beam_vertices.values() {
            if sig.vertex(vertex.0).is_none_or(|vertex| {
                !matches!(
                    vertex.kind,
                    NativeSigInterKind::Beam
                        | NativeSigInterKind::BeamHook
                        | NativeSigInterKind::SmallBeam
                )
            }) {
                return Err(NativeSigError::InvalidBeamSourceBinding {
                    system_id: self.system_id,
                });
            }
        }
        let bound_ledgers = self
            .ledger_vertices
            .values()
            .copied()
            .collect::<BTreeSet<_>>();
        for vertex in &bound_ledgers {
            if sig
                .vertex(vertex.0)
                .is_none_or(|item| item.kind != NativeSigInterKind::Ledger)
            {
                return Err(NativeSigError::InvalidLedgerBinding {
                    system_id: self.system_id,
                });
            }
        }
        if self.reduction_staffs.iter().any(|staff| {
            staff
                .ledger_map
                .values()
                .flatten()
                .any(|vertex| !bound_ledgers.contains(vertex))
        }) {
            return Err(NativeSigError::InvalidLedgerBinding {
                system_id: self.system_id,
            });
        }
        for (&group_ordinal, vertex) in &self.beam_group_vertices {
            if sig
                .vertex(vertex.0)
                .is_none_or(|vertex| vertex.kind != NativeSigInterKind::BeamGroup)
                || sig.outgoing_edges(vertex.0)?.iter().any(|edge| {
                    edge.kind != NativeSigRelationKind::Containment
                        || !bound_beams.contains(&edge.target)
                })
            {
                return Err(NativeSigError::DuplicateBeamGroupBinding {
                    system_id: self.system_id,
                    group_ordinal,
                });
            }
        }
        for (&stem_identity, vertex) in &self.stem_vertices {
            if sig
                .vertex(vertex.0)
                .is_none_or(|vertex| vertex.kind != NativeSigInterKind::Stem)
            {
                return Err(NativeSigError::DuplicateStemBinding {
                    system_id: self.system_id,
                    stem_identity,
                });
            }
        }
        for (&head, vertex) in &self.head_vertices {
            if sig
                .vertex(vertex.0)
                .is_none_or(|vertex| vertex.kind != NativeSigInterKind::Head)
            {
                return Err(NativeSigError::DuplicateHeadBinding {
                    system_id: self.system_id,
                    head,
                });
            }
        }
        Ok(())
    }

    pub fn bind_beam_group(
        &mut self,
        group_ordinal: usize,
        vertex: NativeSigVertexId,
    ) -> Result<(), NativeSigError> {
        if self
            .beam_group_vertices
            .insert(group_ordinal, vertex)
            .is_some()
        {
            return Err(NativeSigError::DuplicateBeamGroupBinding {
                system_id: self.system_id,
                group_ordinal,
            });
        }
        Ok(())
    }

    pub fn bind_stem(
        &mut self,
        stem_identity: usize,
        vertex: NativeSigVertexId,
    ) -> Result<(), NativeSigError> {
        if self.stem_vertices.insert(stem_identity, vertex).is_some() {
            return Err(NativeSigError::DuplicateStemBinding {
                system_id: self.system_id,
                stem_identity,
            });
        }
        Ok(())
    }

    pub fn bind_head(
        &mut self,
        head: NativeHeadStaffEpilogRef,
        vertex: NativeSigVertexId,
    ) -> Result<(), NativeSigError> {
        if self.head_vertices.insert(head, vertex).is_some() {
            return Err(NativeSigError::DuplicateHeadBinding {
                system_id: self.system_id,
                head,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeSigError {
    MissingGridSystem(usize),
    MissingHeaderSystem(usize),
    MissingLedgerGlyph(usize),
    MissingHeadsSystem(usize),
    MissingBraceGlyph(usize),
    MissingGridGlyph(audiveris_image::staff_peak::StaffPeakKey),
    MissingBracketFontGeometry,
    MissingStaffLine(usize),
    MissingSelected(&'static str, usize),
    MissingBeamGroup(usize),
    MissingVertex {
        system_id: usize,
        ordinal: usize,
    },
    MissingEdge {
        system_id: usize,
        ordinal: usize,
    },
    InvalidVertexState {
        system_id: usize,
        ordinal: usize,
    },
    InvalidEdgeState {
        system_id: usize,
        ordinal: usize,
    },
    InvalidRelationPayload {
        system_id: usize,
        ordinal: usize,
    },
    SelfLoop {
        system_id: usize,
        ordinal: usize,
    },
    DuplicateBeamBinding {
        system_id: usize,
        source: NativeStemsBeamSource,
    },
    DuplicateStemBinding {
        system_id: usize,
        stem_identity: usize,
    },
    DuplicateBeamGroupBinding {
        system_id: usize,
        group_ordinal: usize,
    },
    DuplicateHeadBinding {
        system_id: usize,
        head: NativeHeadStaffEpilogRef,
    },
    DuplicateLedgerBinding {
        system_id: usize,
        inter_id: usize,
    },
    MissingLedgerBinding {
        system_id: usize,
        inter_id: usize,
    },
    InvalidBeamSourceBinding {
        system_id: usize,
    },
    InvalidLedgerBinding {
        system_id: usize,
    },
    InvalidVertexOrdinal {
        system_id: usize,
        expected: usize,
        actual: usize,
    },
    InvalidEdgeOrdinal {
        system_id: usize,
        expected: usize,
        actual: usize,
    },
    InvalidBeamMember {
        system_id: usize,
        index: usize,
    },
    UnsupportedTime {
        numerator: i32,
        denominator: i32,
    },
}

impl fmt::Display for NativeSigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingGridSystem(id) => write!(formatter, "missing GRID SIG system {id}"),
            Self::MissingHeaderSystem(id) => write!(formatter, "missing HEADERS system {id}"),
            Self::MissingLedgerGlyph(id) => {
                write!(formatter, "missing ledger glyph for inter {id}")
            }
            Self::MissingHeadsSystem(id) => write!(formatter, "missing HEADS system {id}"),
            Self::MissingBraceGlyph(id) => write!(formatter, "missing brace glyph {id}"),
            Self::MissingGridGlyph(peak) => write!(
                formatter,
                "missing GRID glyph for staff {} peak {}..{}",
                peak.staff_id().value(),
                peak.start(),
                peak.stop()
            ),
            Self::MissingBracketFontGeometry => {
                formatter.write_str("missing Bravura bracket-serif geometry")
            }
            Self::MissingStaffLine(id) => write!(formatter, "missing staff-line geometry {id}"),
            Self::MissingSelected(kind, staff) => {
                write!(formatter, "missing selected {kind} for staff {staff}")
            }
            Self::MissingBeamGroup(id) => write!(formatter, "missing BEAMS groups for system {id}"),
            Self::MissingVertex { system_id, ordinal } => {
                write!(formatter, "system {system_id} has no SIG vertex {ordinal}")
            }
            Self::MissingEdge { system_id, ordinal } => {
                write!(formatter, "system {system_id} has no SIG edge {ordinal}")
            }
            Self::InvalidVertexState { system_id, ordinal } => write!(
                formatter,
                "system {system_id} cannot append inactive SIG vertex {ordinal}"
            ),
            Self::InvalidEdgeState { system_id, ordinal } => write!(
                formatter,
                "system {system_id} cannot append inactive SIG edge {ordinal}"
            ),
            Self::InvalidRelationPayload { system_id, ordinal } => write!(
                formatter,
                "system {system_id} has invalid payload for SIG edge {ordinal}"
            ),
            Self::SelfLoop { system_id, ordinal } => write!(
                formatter,
                "system {system_id} cannot append SIG self-loop {ordinal}"
            ),
            Self::DuplicateBeamBinding { system_id, source } => write!(
                formatter,
                "system {system_id} has duplicate beam binding {source:?}"
            ),
            Self::DuplicateStemBinding {
                system_id,
                stem_identity,
            } => write!(
                formatter,
                "system {system_id} has duplicate stem binding {stem_identity}"
            ),
            Self::DuplicateBeamGroupBinding {
                system_id,
                group_ordinal,
            } => write!(
                formatter,
                "system {system_id} has duplicate beam-group binding {group_ordinal}"
            ),
            Self::DuplicateHeadBinding { system_id, head } => write!(
                formatter,
                "system {system_id} has duplicate head binding {head:?}"
            ),
            Self::DuplicateLedgerBinding {
                system_id,
                inter_id,
            } => write!(
                formatter,
                "system {system_id} has duplicate ledger binding {inter_id}"
            ),
            Self::MissingLedgerBinding {
                system_id,
                inter_id,
            } => write!(
                formatter,
                "system {system_id} has no live ledger binding {inter_id}"
            ),
            Self::InvalidBeamSourceBinding { system_id } => {
                write!(
                    formatter,
                    "system {system_id} has ambiguous beam source binding"
                )
            }
            Self::InvalidLedgerBinding { system_id } => write!(
                formatter,
                "system {system_id} has an invalid REDUCTION ledger binding"
            ),
            Self::InvalidVertexOrdinal {
                system_id,
                expected,
                actual,
            } => write!(
                formatter,
                "system {system_id} expected SIG vertex ordinal {expected}, got {actual}"
            ),
            Self::InvalidEdgeOrdinal {
                system_id,
                expected,
                actual,
            } => write!(
                formatter,
                "system {system_id} expected SIG edge ordinal {expected}, got {actual}"
            ),
            Self::InvalidBeamMember { system_id, index } => {
                write!(
                    formatter,
                    "system {system_id} has invalid beam member {index}"
                )
            }
            Self::UnsupportedTime {
                numerator,
                denominator,
            } => {
                write!(
                    formatter,
                    "unsupported time shape {numerator}/{denominator}"
                )
            }
        }
    }
}

impl Error for NativeSigError {}

/// Assemble the live per-system graph through HEADS from production products only.
pub fn assemble_native_sig(
    grid: &GridLinesRecognition,
    headers: &NativeHeaderRecognition,
    beams: &NativeBeamRecognition,
    ledgers: &NativeLedgerRecognition,
    heads: &NativeHeadsRecognition,
) -> Result<NativeSigRecognition, NativeSigError> {
    let mut systems = Vec::with_capacity(grid.peak_graph.sig.systems.len());
    let mut bindings = Vec::with_capacity(grid.peak_graph.sig.systems.len());
    for grid_system in &grid.peak_graph.sig.systems {
        let system_id = grid_system.system_id;
        let header_system = headers
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .ok_or(NativeSigError::MissingHeaderSystem(system_id))?;
        let head_system = heads
            .epilog
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .ok_or(NativeSigError::MissingHeadsSystem(system_id))?;
        let staff_head_system = heads
            .epilog
            .staff_epilog
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .ok_or(NativeSigError::MissingHeadsSystem(system_id))?;

        let mut graph = NativeSigSystem {
            system_id,
            vertices: Vec::new(),
            edges: Vec::new(),
        };
        let mut system_bindings = NativeSigSystemBindings {
            system_id,
            beam_vertices: BTreeMap::new(),
            beam_group_vertices: BTreeMap::new(),
            stem_vertices: BTreeMap::new(),
            head_vertices: BTreeMap::new(),
            ledger_vertices: BTreeMap::new(),
            reduction_interline: grid.scale.scale.interline.main,
            reduction_staffs: Vec::new(),
            merged_staff_pairs: merged_staff_pairs(heads, system_id)?,
            overlap_geometry: BTreeMap::new(),
        };
        append_grid(grid, grid_system, &mut graph, &mut system_bindings)?;
        append_headers(header_system.staffs.as_slice(), &mut graph)?;
        append_beams(
            beams,
            head_system,
            system_id,
            grid_system.interline.round() as i32,
            &mut graph,
            &mut system_bindings,
        )?;
        append_ledgers(ledgers, system_id, &mut graph, &mut system_bindings)?;
        system_bindings.reduction_staffs = reduction_staff_bindings(
            grid,
            header_system,
            ledgers,
            &system_bindings.ledger_vertices,
        )?;
        append_heads(
            heads,
            head_system,
            staff_head_system,
            &mut graph,
            &mut system_bindings,
        )?;
        graph.validate_integrity()?;
        system_bindings.validate_against(&graph)?;
        systems.push(graph);
        bindings.push(system_bindings);
    }
    Ok(NativeSigRecognition { systems, bindings })
}

fn merged_staff_pairs(
    heads: &NativeHeadsRecognition,
    system_id: usize,
) -> Result<Vec<(usize, usize)>, NativeSigError> {
    let scanner_system = heads
        .scanners
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or(NativeSigError::MissingHeadsSystem(system_id))?;
    let mut pairs = Vec::new();
    let mut index = 0;
    while index < scanner_system.staffs.len() {
        if !scanner_system.staffs[index].merged {
            index += 1;
            continue;
        }
        let Some(last) = scanner_system.staffs.get(index + 1) else {
            return Err(NativeSigError::MissingHeadsSystem(system_id));
        };
        if !last.merged {
            return Err(NativeSigError::MissingHeadsSystem(system_id));
        }
        pairs.push((scanner_system.staffs[index].staff_id, last.staff_id));
        index += 2;
    }
    Ok(pairs)
}

fn push_vertex(graph: &mut NativeSigSystem, mut vertex: NativeSigVertex) -> usize {
    let ordinal = graph.vertices.len();
    vertex.ordinal = ordinal;
    graph.vertices.push(vertex);
    ordinal
}

fn push_edge(
    graph: &mut NativeSigSystem,
    mut source: usize,
    mut target: usize,
    kind: NativeSigRelationKind,
) {
    if matches!(
        kind,
        NativeSigRelationKind::Exclusion | NativeSigRelationKind::BeamBeam
    ) && source > target
    {
        std::mem::swap(&mut source, &mut target);
    }
    let ordinal = graph.edges.len();
    let support = match kind {
        NativeSigRelationKind::NoExclusion
        | NativeSigRelationKind::KeyAlters
        | NativeSigRelationKind::ClefKey
        | NativeSigRelationKind::BeamBeam
        | NativeSigRelationKind::HeadHead => Some(NativeSigSupport {
            grade: 1.0,
            bar_connection_impacts: None,
        }),
        NativeSigRelationKind::BarConnection
        | NativeSigRelationKind::BarGroup
        | NativeSigRelationKind::Containment
        | NativeSigRelationKind::Exclusion
        | NativeSigRelationKind::BeamStem
        | NativeSigRelationKind::BeamHead
        | NativeSigRelationKind::BeamRest
        | NativeSigRelationKind::HeadStem
        | NativeSigRelationKind::ChordStem => None,
    };
    graph.edges.push(NativeSigEdge {
        ordinal,
        active: true,
        source,
        target,
        kind,
        origin: NativeSigRelationOrigin::BaselineGraph,
        support,
        beam_portion: None,
        stem_extension: None,
        head_stem: None,
    });
}

fn push_bar_connection_edge(
    graph: &mut NativeSigSystem,
    source: usize,
    target: usize,
    grade: f64,
    impacts: NativeSigBarConnectionImpacts,
) {
    let ordinal = graph.edges.len();
    graph.edges.push(NativeSigEdge {
        ordinal,
        active: true,
        source,
        target,
        kind: NativeSigRelationKind::BarConnection,
        origin: NativeSigRelationOrigin::BaselineGraph,
        support: Some(NativeSigSupport {
            grade,
            bar_connection_impacts: Some(impacts),
        }),
        beam_portion: None,
        stem_extension: None,
        head_stem: None,
    });
}

fn append_grid(
    grid: &GridLinesRecognition,
    system: &crate::grid_executor::HeadlessSystemSigState,
    graph: &mut NativeSigSystem,
    bindings: &mut NativeSigSystemBindings,
) -> Result<(), NativeSigError> {
    for brace in grid
        .peak_graph
        .brace_sig
        .system_nodes(system.system_id)
        .unwrap_or_default()
    {
        let registered = grid
            .peak_graph
            .brace_sig
            .originals()
            .iter()
            .find(|item| item.id == brace.glyph)
            .ok_or(NativeSigError::MissingBraceGlyph(brace.glyph.value()))?;
        let glyph = &registered.glyph;
        let first_staff_id = *system
            .staff_ids
            .first()
            .ok_or(NativeSigError::MissingGridSystem(system.system_id))?;
        let last_staff_id = *system
            .staff_ids
            .last()
            .ok_or(NativeSigError::MissingGridSystem(system.system_id))?;
        let first_staff = grid
            .staff_lines
            .iter()
            .find(|staff| staff.staff_id == first_staff_id)
            .ok_or(NativeSigError::MissingStaffLine(first_staff_id))?;
        let last_staff = grid
            .staff_lines
            .iter()
            .find(|staff| staff.staff_id == last_staff_id)
            .ok_or(NativeSigError::MissingStaffLine(last_staff_id))?;
        let x = i32::try_from(glyph.x)
            .map_err(|_| NativeSigError::MissingBraceGlyph(brace.glyph.value()))?;
        let width = i32::try_from(glyph.runs.width())
            .map_err(|_| NativeSigError::MissingBraceGlyph(brace.glyph.value()))?;
        let x_right = x.saturating_add(width);
        let y1 = first_staff
            .first_line
            .y_at_x_ext(f64::from(x_right))
            .round_ties_even() as i32;
        let y2 = last_staff
            .last_line
            .y_at_x_ext(f64::from(x_right))
            .round_ties_even() as i32;
        let vertex = NativeSigVertexId(push_vertex(
            graph,
            NativeSigVertex {
                ordinal: 0,
                active: true,
                removed: false,
                frozen: false,
                kind: NativeSigInterKind::Brace,
                shape: Some("BRACE".to_owned()),
                grade: brace.grade,
                contextual_grade: None,
                bounds: NativeSigBounds {
                    x,
                    y: y1,
                    width,
                    height: y2.saturating_sub(y1).saturating_add(1),
                },
                abnormal: false,
                beam_geometry: None,
            },
        ));
        bindings.overlap_geometry.insert(
            vertex,
            NativeReductionInterGeometry {
                bounds: graph.vertices[vertex.0].bounds,
                core_bounds: graph.vertices[vertex.0].bounds,
                implicit: false,
                glyph: Some(NativeReductionGlyphGeometry {
                    left: x,
                    top: i32::try_from(glyph.y)
                        .map_err(|_| NativeSigError::MissingBraceGlyph(brace.glyph.value()))?,
                    run_table: glyph.runs.clone(),
                }),
                area: None,
                head: None,
                ensemble_members: Vec::new(),
            },
        );
    }

    let base = graph.vertices.len();
    let ordered = system.sig.nodes_in_order().collect::<Vec<_>>();
    for (_, node) in &ordered {
        let frozen = match node {
            GridSigNode::Vertical { frozen, .. } | GridSigNode::Connector { frozen, .. } => *frozen,
        };
        let (kind, shape, bounds) = match node {
            GridSigNode::Vertical { plan, .. } => {
                let half = plan.width / 2.0;
                let left = (plan.median.x - half).floor();
                let right = (plan.median.x + half).ceil();
                let top = plan.median.top.floor();
                let bottom = plan.median.bottom.ceil();
                let bounds = NativeSigBounds {
                    x: left as i32,
                    y: top as i32,
                    width: (right - left) as i32,
                    height: (bottom - top) as i32,
                };
                match plan.kind {
                    VerticalInterKind::Barline { width_class, .. } => (
                        NativeSigInterKind::Barline,
                        match width_class {
                            PeakWidthClass::Thin => "THIN_BARLINE",
                            PeakWidthClass::Thick => "THICK_BARLINE",
                        },
                        bounds,
                    ),
                    VerticalInterKind::Bracket(kind) => (
                        NativeSigInterKind::Bracket,
                        match kind {
                            BracketKind::None => "BRACKET_MIDDLE",
                            BracketKind::Top => "BRACKET_TOP",
                            BracketKind::Bottom => "BRACKET_BOTTOM",
                            BracketKind::Both => "BRACKET",
                        },
                        area_integer_bounds(&bracket_area(plan, grid.scale.scale.interline.main)?),
                    ),
                }
            }
            GridSigNode::Connector { plan, .. } => {
                let peak = |key| {
                    system
                        .staff_peaks
                        .iter()
                        .flatten()
                        .find(|peak| peak.key() == key)
                };
                let top = peak(plan.top).expect("connector top peak");
                let bottom = peak(plan.bottom).expect("connector bottom peak");
                let x1 = f64::from(top.start()) + (f64::from(top.width()) / 2.0);
                let x2 = f64::from(bottom.start()) + (f64::from(bottom.width()) / 2.0);
                let half_line = f64::from(grid.scale.scale.line.main) / 2.0;
                let y1 = f64::from(top.bottom()) + half_line + 0.5;
                let y2 = f64::from(bottom.top()) - half_line + 0.5;
                let width = f64::from(top.width() + bottom.width()) / 2.0;
                let left = (x1.min(x2) - (width / 2.0)).floor();
                let right = (x1.max(x2) + (width / 2.0)).ceil();
                let top_y = y1.min(y2).floor();
                let bottom_y = y1.max(y2).ceil();
                let bounds = NativeSigBounds {
                    x: left as i32,
                    y: top_y as i32,
                    width: (right - left) as i32,
                    height: (bottom_y - top_y) as i32,
                };
                match plan.kind {
                    ConnectorInterKind::Barline(PeakWidthClass::Thin) => {
                        (NativeSigInterKind::BarConnector, "THIN_CONNECTOR", bounds)
                    }
                    ConnectorInterKind::Barline(PeakWidthClass::Thick) => {
                        (NativeSigInterKind::BarConnector, "THICK_CONNECTOR", bounds)
                    }
                    ConnectorInterKind::Bracket => (
                        NativeSigInterKind::BracketConnector,
                        "BRACKET_CONNECTOR",
                        bounds,
                    ),
                }
            }
        };
        let vertex = NativeSigVertexId(push_vertex(
            graph,
            NativeSigVertex {
                ordinal: 0,
                active: true,
                removed: false,
                frozen,
                kind,
                shape: Some(shape.to_owned()),
                grade: node.intrinsic_grade(),
                contextual_grade: None,
                bounds,
                abnormal: false,
                beam_geometry: None,
            },
        ));
        if let Some(geometry) = grid_overlap_geometry(grid, system, node, bounds)? {
            bindings.overlap_geometry.insert(vertex, geometry);
        }
    }
    for edge in system.sig.edges() {
        let ordinal = |id: GridInterId| {
            ordered
                .iter()
                .position(|(candidate, _)| *candidate == id)
                .map(|index| base + index)
                .expect("GRID edge endpoint is present")
        };
        match edge.relation {
            GridSigRelation::NoExclusion => push_edge(
                graph,
                ordinal(edge.source),
                ordinal(edge.target),
                NativeSigRelationKind::NoExclusion,
            ),
            GridSigRelation::BarConnectionSupport { grade } => {
                let endpoint_peak = |id| match system.sig.node(id) {
                    Some(GridSigNode::Vertical { plan, .. }) => Some(plan.peak),
                    _ => None,
                };
                let source_peak =
                    endpoint_peak(edge.source).expect("bar-connection source is a vertical inter");
                let target_peak =
                    endpoint_peak(edge.target).expect("bar-connection target is a vertical inter");
                let connector = grid
                    .peak_graph
                    .sig
                    .connections
                    .iter()
                    .find(|candidate| {
                        candidate.system_id == system.system_id
                            && candidate.plan.top == source_peak
                            && candidate.plan.bottom == target_peak
                    })
                    .map(|candidate| &candidate.plan)
                    .expect("bar-connection support retains its connection plan");
                let relation = grid
                    .peak_graph
                    .sig
                    .peak_graph
                    .edge(connector.edge)
                    .expect("connector retains its peak-graph relation")
                    .relation();
                let audiveris_image::bar_alignment::BarImpacts::Connection {
                    align,
                    width,
                    gap,
                    white,
                } = relation.impacts()
                else {
                    panic!("connector relation carries connection impacts");
                };
                push_bar_connection_edge(
                    graph,
                    ordinal(edge.source),
                    ordinal(edge.target),
                    grade,
                    NativeSigBarConnectionImpacts {
                        align,
                        width,
                        gap,
                        white,
                    },
                );
            }
            GridSigRelation::BarGroup { .. } => push_edge(
                graph,
                ordinal(edge.source),
                ordinal(edge.target),
                NativeSigRelationKind::BarGroup,
            ),
        }
    }
    Ok(())
}

fn grid_overlap_geometry(
    grid: &GridLinesRecognition,
    system: &crate::grid_executor::HeadlessSystemSigState,
    node: &GridSigNode,
    bounds: NativeSigBounds,
) -> Result<Option<NativeReductionInterGeometry>, NativeSigError> {
    match node {
        GridSigNode::Vertical { plan, .. } => {
            let exact = grid
                .peak_graph
                .bar_glyphs
                .iter()
                .find(|glyph| glyph.peak == plan.peak);
            // Brace/bracket peak replacement transfers Java's existing
            // filament object while changing the StaffPeak key. The retained
            // source sticks precede that mutation, so recover the transferred
            // object only when its staff and x-span identify one candidate.
            let mut transferred = grid.peak_graph.bar_glyphs.iter().filter(|glyph| {
                glyph.peak.staff_id() == plan.peak.staff_id()
                    && i32::try_from(glyph.left).is_ok_and(|left| left <= plan.peak.stop())
                    && i32::try_from(glyph.left.saturating_add(glyph.run_table.width()))
                        .is_ok_and(|right| right > plan.peak.start())
            });
            let transferred_first = transferred.next();
            let glyph = exact
                .or_else(|| transferred_first.filter(|_| transferred.next().is_none()))
                .ok_or(NativeSigError::MissingGridGlyph(plan.peak))?;
            let glyph = NativeReductionGlyphGeometry {
                left: i32::try_from(glyph.left)
                    .map_err(|_| NativeSigError::MissingGridGlyph(plan.peak))?,
                top: i32::try_from(glyph.top)
                    .map_err(|_| NativeSigError::MissingGridGlyph(plan.peak))?,
                run_table: glyph.run_table.clone(),
            };
            let area = match plan.kind {
                VerticalInterKind::Bracket(_) => {
                    bracket_area(plan, grid.scale.scale.interline.main)?
                }
                VerticalInterKind::Barline { .. } => vertical_ribbon_area(
                    plan.median.x,
                    plan.median.top,
                    plan.median.x,
                    plan.median.bottom,
                    plan.width,
                ),
            };
            Ok(Some(NativeReductionInterGeometry {
                bounds,
                core_bounds: bounds,
                implicit: false,
                glyph: Some(glyph),
                area: Some(area),
                head: None,
                ensemble_members: Vec::new(),
            }))
        }
        GridSigNode::Connector { plan, .. } => {
            let peak = |key| {
                system
                    .staff_peaks
                    .iter()
                    .flatten()
                    .find(|peak| peak.key() == key)
            };
            let top = peak(plan.top).ok_or(NativeSigError::MissingGridGlyph(plan.top))?;
            let bottom = peak(plan.bottom).ok_or(NativeSigError::MissingGridGlyph(plan.bottom))?;
            let x1 = f64::from(top.start()) + (f64::from(top.width()) / 2.0);
            let x2 = f64::from(bottom.start()) + (f64::from(bottom.width()) / 2.0);
            let half_line = f64::from(grid.scale.scale.line.main) / 2.0;
            let y1 = f64::from(top.bottom()) + half_line + 0.5;
            let y2 = f64::from(bottom.top()) - half_line + 0.5;
            let width = f64::from(top.width() + bottom.width()) / 2.0;
            Ok(Some(NativeReductionInterGeometry {
                bounds,
                core_bounds: bounds,
                implicit: false,
                glyph: None,
                area: Some(vertical_ribbon_area(x1, y1, x2, y2, width)),
                head: None,
                ensemble_members: Vec::new(),
            }))
        }
    }
}

fn bracket_area(
    plan: &audiveris_image::bars_logic::VerticalInterPlan,
    interline: i32,
) -> Result<NativeReductionAreaGeometry, NativeSigError> {
    let mut area = vertical_ribbon_area(
        plan.median.x,
        plan.median.top,
        plan.median.x,
        plan.median.bottom,
        plan.width,
    );
    let VerticalInterKind::Bracket(kind) = plan.kind else {
        return Ok(area);
    };
    let upper = layout_bounds(MusicFamily::Bravura, "BRACKET_UPPER_SERIF", interline)
        .map_err(|_| NativeSigError::MissingBracketFontGeometry)?
        .ok_or(NativeSigError::MissingBracketFontGeometry)?;
    let lower = layout_bounds(MusicFamily::Bravura, "BRACKET_LOWER_SERIF", interline)
        .map_err(|_| NativeSigError::MissingBracketFontGeometry)?
        .ok_or(NativeSigError::MissingBracketFontGeometry)?;
    if matches!(kind, BracketKind::Top | BracketKind::Both) {
        area.components.push(rectangle_area(
            plan.median.x - (plan.width / 2.0),
            plan.median.top - upper.height,
            upper.width,
            upper.height,
        ));
    }
    if matches!(kind, BracketKind::Bottom | BracketKind::Both) {
        area.components.push(rectangle_area(
            plan.median.x - (plan.width / 2.0),
            plan.median.bottom,
            lower.width,
            lower.height,
        ));
    }
    Ok(area)
}

fn area_integer_bounds(area: &NativeReductionAreaGeometry) -> NativeSigBounds {
    let minimum_x = area
        .components
        .iter()
        .flatten()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let maximum_x = area
        .components
        .iter()
        .flatten()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let minimum_y = area
        .components
        .iter()
        .flatten()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    let maximum_y = area
        .components
        .iter()
        .flatten()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let left = minimum_x.floor() as i32;
    let top = minimum_y.floor() as i32;
    let right = maximum_x.ceil() as i32;
    let bottom = maximum_y.ceil() as i32;
    NativeSigBounds {
        x: left,
        y: top,
        width: right.saturating_sub(left),
        height: bottom.saturating_sub(top),
    }
}

fn rectangle_area(x: f64, y: f64, width: f64, height: f64) -> Vec<NativeReductionAreaPoint> {
    vec![
        NativeReductionAreaPoint { x, y },
        NativeReductionAreaPoint { x: x + width, y },
        NativeReductionAreaPoint {
            x: x + width,
            y: y + height,
        },
        NativeReductionAreaPoint { x, y: y + height },
    ]
}

fn vertical_ribbon_area(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    width: f64,
) -> NativeReductionAreaGeometry {
    let dx = width / 2.0;
    NativeReductionAreaGeometry {
        components: vec![vec![
            NativeReductionAreaPoint { x: x1 - dx, y: y1 },
            NativeReductionAreaPoint { x: x2 - dx, y: y2 },
            NativeReductionAreaPoint { x: x2 + dx, y: y2 },
            NativeReductionAreaPoint { x: x1 + dx, y: y1 },
        ]],
    }
}

fn horizontal_ribbon_area(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    height: f64,
) -> NativeReductionAreaGeometry {
    let dy = height / 2.0;
    NativeReductionAreaGeometry {
        components: vec![vec![
            NativeReductionAreaPoint { x: x1, y: y1 - dy },
            NativeReductionAreaPoint { x: x2, y: y2 - dy },
            NativeReductionAreaPoint { x: x2, y: y2 + dy },
            NativeReductionAreaPoint { x: x1, y: y1 + dy },
        ]],
    }
}

fn append_headers(
    staffs: &[NativeHeaderStaffRecognition],
    graph: &mut NativeSigSystem,
) -> Result<(), NativeSigError> {
    let mut clefs = Vec::with_capacity(staffs.len());
    for staff in staffs {
        let Some(selected) = staff.selected_clef_id else {
            clefs.push(None);
            continue;
        };
        let clef = staff
            .clef_candidates
            .iter()
            .find(|candidate| candidate.id == selected)
            .ok_or(NativeSigError::MissingSelected("clef", staff.staff_id))?;
        let shape = match clef.kind {
            NeutralClefKind::Treble => "G_CLEF",
            NeutralClefKind::Bass => "F_CLEF",
            NeutralClefKind::Baritone => "C_CLEF",
            NeutralClefKind::Tenor => "C_CLEF",
            NeutralClefKind::Alto => "C_CLEF",
            NeutralClefKind::MezzoSoprano => "C_CLEF",
            NeutralClefKind::Soprano => "C_CLEF",
            NeutralClefKind::Percussion => "PERCUSSION_CLEF",
        };
        clefs.push(Some(push_header_vertex(
            graph,
            NativeSigInterKind::Clef,
            Some(shape),
            clef.grade,
            clef.bounds,
        )));
    }

    for (staff_index, staff) in staffs.iter().enumerate() {
        let Some(selected) = staff.selected_key_id else {
            continue;
        };
        let key = staff
            .key_candidates
            .iter()
            .find(|candidate| candidate.id == selected)
            .ok_or(NativeSigError::MissingSelected("key", staff.staff_id))?;
        let mut alters = Vec::new();
        for slice in &key.slices {
            if slice.alter_id.is_none() {
                continue;
            }
            let bounds = slice
                .alter_bounds
                .ok_or(NativeSigError::MissingSelected("key alter", staff.staff_id))?;
            let grade = slice.alter_grade.ok_or(NativeSigError::MissingSelected(
                "key alter grade",
                staff.staff_id,
            ))?;
            alters.push(push_header_vertex(
                graph,
                NativeSigInterKind::KeyAlter,
                Some(if key.fifths < 0 { "FLAT" } else { "SHARP" }),
                grade,
                bounds,
            ));
        }
        let grade = alters
            .iter()
            .map(|&ordinal| graph.vertices[ordinal].grade)
            .sum::<f64>()
            / alters.len() as f64;
        let key_ordinal =
            push_header_vertex(graph, NativeSigInterKind::Key, None, grade, key.bounds);
        for pair in alters.windows(2) {
            push_edge(graph, pair[0], pair[1], NativeSigRelationKind::KeyAlters);
        }
        for alter in alters {
            push_edge(
                graph,
                key_ordinal,
                alter,
                NativeSigRelationKind::Containment,
            );
        }
        if let Some(clef) = clefs[staff_index] {
            push_edge(graph, clef, key_ordinal, NativeSigRelationKind::ClefKey);
        }
    }

    for staff in staffs {
        let Some(selected) = staff.selected_time_id else {
            continue;
        };
        let time = staff
            .time_candidates
            .iter()
            .find(|candidate| candidate.id == selected)
            .ok_or(NativeSigError::MissingSelected("time", staff.staff_id))?;
        let shape = time_shape(time)?;
        push_header_vertex(
            graph,
            if time.member_ids.is_empty() {
                NativeSigInterKind::TimeWhole
            } else {
                NativeSigInterKind::TimePair
            },
            Some(shape),
            time.grade,
            time.symbol_bounds,
        );
    }
    Ok(())
}

fn push_header_vertex(
    graph: &mut NativeSigSystem,
    kind: NativeSigInterKind,
    shape: Option<&str>,
    grade: f64,
    bounds: crate::staff_header::HeaderBounds,
) -> usize {
    push_vertex(
        graph,
        NativeSigVertex {
            ordinal: 0,
            active: true,
            removed: false,
            frozen: true,
            kind,
            shape: shape.map(str::to_owned),
            grade,
            contextual_grade: None,
            bounds: NativeSigBounds {
                x: bounds.x,
                y: bounds.y,
                width: bounds.width,
                height: bounds.height,
            },
            abnormal: false,
            beam_geometry: None,
        },
    )
}

fn time_shape(time: &NeutralTimeCandidate) -> Result<&'static str, NativeSigError> {
    if time.value.specific_shape == Some(NeutralSpecificTimeShape::Common) {
        return Ok("COMMON_TIME");
    }
    if time.value.specific_shape == Some(NeutralSpecificTimeShape::Cut) {
        return Ok("CUT_TIME");
    }
    match (time.value.numerator, time.value.denominator) {
        (2, 2) => Ok("TIME_TWO_TWO"),
        (2, 4) => Ok("TIME_TWO_FOUR"),
        (3, 4) => Ok("TIME_THREE_FOUR"),
        (4, 4) => Ok("TIME_FOUR_FOUR"),
        (6, 8) => Ok("TIME_SIX_EIGHT"),
        (9, 8) => Ok("TIME_NINE_EIGHT"),
        (12, 8) => Ok("TIME_TWELVE_EIGHT"),
        (numerator, denominator) => Err(NativeSigError::UnsupportedTime {
            numerator,
            denominator,
        }),
    }
}

fn append_beams(
    beams: &NativeBeamRecognition,
    heads: &crate::native_heads_epilog::NativeHeadsEpilogSystem,
    system_id: usize,
    interline: i32,
    graph: &mut NativeSigSystem,
    bindings: &mut NativeSigSystemBindings,
) -> Result<(), NativeSigError> {
    let first = graph.vertices.len();
    // Java's `BeamsBuilder.buildBeams()` creates BeamGroupInter containment
    // before `BeamsStep` invokes `MultipleRestsBuilder`. Replacing a rest-like
    // beam removes that beam and its incident relations, but it does not
    // geometrically regroup the surviving beams. Keep the pre-rest grouping
    // input and map its members onto the live post-rest vertex stream below.
    let grouping = beams
        .raw_beams
        .iter()
        .enumerate()
        .filter(|(_, (owner, _))| *owner == system_id)
        .map(|(ordinal, (_, beam))| (NativeStemsBeamSource::RawBeam(ordinal), beam))
        .chain(
            beams
                .hooks
                .iter()
                .enumerate()
                .filter(|(_, (owner, _))| *owner == system_id)
                .map(|(ordinal, (_, beam))| (NativeStemsBeamSource::Hook(ordinal), beam)),
        )
        .collect::<Vec<_>>();
    let evidence = crate::beam_inters::group_beams(
        &grouping.iter().map(|(_, beam)| **beam).collect::<Vec<_>>(),
        interline,
    );
    let groups = beams
        .group_memberships
        .iter()
        .find(|membership| membership.system_id == system_id)
        .ok_or(NativeSigError::MissingBeamGroup(system_id))?;
    if evidence.groups != groups.groups {
        return Err(NativeSigError::MissingBeamGroup(system_id));
    }

    let removed_raw_beams = beams
        .multiple_rests
        .iter()
        .map(|rest| rest.source_beam_ordinal)
        .collect::<BTreeSet<_>>();
    let removed_by_heads = heads
        .small_beams
        .beam_provenance
        .iter()
        .zip(&heads.small_beams.beam_inputs)
        .zip(&heads.small_beams.arbitration.beam_removed)
        .filter_map(|((provenance, beam), &removed)| {
            (removed || beam.removed).then_some(match provenance.source {
                NativeHeadsCompetitorSource::RawBeam(ordinal) => {
                    NativeStemsBeamSource::RawBeam(ordinal)
                }
                NativeHeadsCompetitorSource::Hook(ordinal) => NativeStemsBeamSource::Hook(ordinal),
                _ => return None,
            })
        })
        .collect::<BTreeSet<_>>();
    // `beams_after_multiple_rests` intentionally stores geometry without
    // identity. Equal beam values can legitimately occur, so recovering a
    // source by value is ambiguous. Preserve the source ordinal directly from
    // the aligned pre-replacement stream while applying the same removal set.
    let mut created = beams
        .raw_beams
        .iter()
        .enumerate()
        .filter(|(ordinal, (owner, _))| {
            *owner == system_id
                && !removed_raw_beams.contains(ordinal)
                && !removed_by_heads.contains(&NativeStemsBeamSource::RawBeam(*ordinal))
        })
        .map(|(ordinal, (_, beam))| (NativeStemsBeamSource::RawBeam(ordinal), beam))
        .collect::<Vec<_>>();
    created.extend(
        beams
            .hooks
            .iter()
            .enumerate()
            .filter(|(ordinal, (owner, _))| {
                *owner == system_id
                    && !removed_by_heads.contains(&NativeStemsBeamSource::Hook(*ordinal))
            })
            .map(|(ordinal, (_, beam))| (NativeStemsBeamSource::Hook(ordinal), beam)),
    );
    for &(source, beam) in &created {
        let vertex = NativeSigVertexId(push_beam_vertex(graph, beam));
        if bindings.beam_vertices.insert(source, vertex).is_some() {
            return Err(NativeSigError::DuplicateBeamBinding { system_id, source });
        }
        let glyph = match source {
            NativeStemsBeamSource::RawBeam(ordinal) => beams.raw_beam_glyphs.get(ordinal),
            NativeStemsBeamSource::Hook(ordinal) => beams.hook_glyphs.get(ordinal),
        }
        .filter(|(owner, _)| *owner == system_id)
        .map(|(_, glyph)| glyph)
        .ok_or(NativeSigError::InvalidBeamSourceBinding { system_id })?;
        bindings.overlap_geometry.insert(
            vertex,
            NativeReductionInterGeometry {
                bounds: graph.vertices[vertex.0].bounds,
                core_bounds: graph.vertices[vertex.0].bounds,
                implicit: false,
                glyph: Some(NativeReductionGlyphGeometry {
                    left: glyph.bounds.x,
                    top: glyph.bounds.y,
                    run_table: glyph.run_table.clone(),
                }),
                area: Some(horizontal_ribbon_area(
                    beam.item.median.x1,
                    beam.item.median.y1,
                    beam.item.median.x2,
                    beam.item.median.y2,
                    beam.item.height,
                )),
                head: None,
                ensemble_members: Vec::new(),
            },
        );
    }
    let live_indices = created
        .iter()
        .enumerate()
        .map(|(index, (source, _))| (*source, index))
        .collect::<BTreeMap<_, _>>();
    let grouping_to_live = grouping
        .iter()
        .map(|(source, _)| live_indices.get(source).copied())
        .collect::<Vec<_>>();
    let exclusions = grouping
        .windows(2)
        .filter(|pair| {
            pair[0].1.item == pair[1].1.item
                && pair[0].1.kind == BeamKind::Hook
                && pair[1].1.kind != BeamKind::Hook
        })
        .filter_map(|pair| {
            let source = live_indices.get(&pair[0].0).copied()?;
            let target = live_indices.get(&pair[1].0).copied()?;
            Some((first + source, first + target))
        })
        .collect::<BTreeSet<_>>();
    for &(source, target) in &exclusions {
        push_edge(graph, source, target, NativeSigRelationKind::Exclusion);
    }

    let mut active = Vec::<bool>::new();
    for event in &evidence.events {
        match *event {
            audiveris_image::beam_groups::BeamGroupEvent::Created { group_index, .. } => {
                if active.len() <= group_index {
                    active.resize(group_index + 1, false);
                }
                active[group_index] = true;
            }
            audiveris_image::beam_groups::BeamGroupEvent::Merged { removed_index, .. } => {
                active[removed_index] = false;
            }
            audiveris_image::beam_groups::BeamGroupEvent::Added { .. } => {}
        }
    }
    let mut group_ordinals = vec![None; active.len()];
    let mut final_groups = groups.groups.iter().enumerate();
    for (provisional, is_active) in active.iter().copied().enumerate() {
        if !is_active {
            continue;
        }
        let (pre_rest_group_ordinal, group) = final_groups
            .next()
            .expect("one final group per active identity");
        let mut bounds = None;
        for &index in group {
            let Some(live_index) = grouping_to_live.get(index).copied().flatten() else {
                continue;
            };
            let beam = created
                .get(live_index)
                .ok_or(NativeSigError::InvalidBeamMember { system_id, index })?
                .1;
            let item = beam_bounds(beam.item);
            let item = NativeSigBounds {
                x: item.x,
                y: item.y,
                width: item.width,
                height: item.height,
            };
            bounds = Some(bounds.map_or(item, |current: NativeSigBounds| current.union(item)));
        }
        // Removing the sole member extensively removes its dying ensemble in
        // Java. Such an empty pre-rest group has no live SIG vertex or binding.
        let Some(bounds) = bounds else {
            continue;
        };
        let vertex = NativeSigVertexId(push_vertex(
            graph,
            NativeSigVertex {
                ordinal: 0,
                active: true,
                removed: false,
                frozen: false,
                kind: NativeSigInterKind::BeamGroup,
                shape: None,
                grade: 1.0,
                contextual_grade: None,
                bounds,
                abnormal: false,
                beam_geometry: None,
            },
        ));
        group_ordinals[provisional] = Some(vertex.0);
        // STEMS records the ordinal from Java's pre-rest BeamGroup list.
        // Extensively removing an empty group must leave a hole rather than
        // renumbering every surviving group and redirecting later B-linkers.
        bind_surviving_pre_rest_beam_group(bindings, pre_rest_group_ordinal, vertex)?;
    }

    #[derive(Clone, Copy)]
    enum Pending {
        Containment(usize, usize),
        BeamBeam(usize, usize),
    }
    let mut members = vec![Vec::<usize>::new(); active.len()];
    let mut pending = Vec::<Pending>::new();
    let add_member = |group: usize,
                      pre_rest_beam: usize,
                      members: &mut Vec<Vec<usize>>,
                      pending: &mut Vec<Pending>| {
        let Some(beam) = grouping_to_live.get(pre_rest_beam).copied().flatten() else {
            return;
        };
        if members[group].contains(&beam) {
            return;
        }
        pending.push(Pending::Containment(group, beam));
        for &old in &members[group] {
            let pair = (
                (first + old).min(first + beam),
                (first + old).max(first + beam),
            );
            if !exclusions.contains(&pair)
                && !pending.iter().any(|edge| {
                    matches!(*edge, Pending::BeamBeam(one, two) if (one.min(two), one.max(two)) == pair)
                })
            {
                pending.push(Pending::BeamBeam(first + old, first + beam));
            }
        }
        members[group].push(beam);
    };
    for event in &evidence.events {
        match *event {
            audiveris_image::beam_groups::BeamGroupEvent::Created {
                group_index,
                beam_id,
            }
            | audiveris_image::beam_groups::BeamGroupEvent::Added {
                group_index,
                beam_id,
                ..
            } => add_member(group_index, beam_id, &mut members, &mut pending),
            audiveris_image::beam_groups::BeamGroupEvent::Merged {
                survivor_index,
                removed_index,
                ..
            } => {
                let moved = members[removed_index].clone();
                for beam in moved {
                    add_member(survivor_index, beam, &mut members, &mut pending);
                }
                pending.retain(|edge| {
                    !matches!(*edge, Pending::Containment(group, _) if group == removed_index)
                });
            }
        }
    }
    for edge in pending {
        match edge {
            Pending::Containment(group, beam) => {
                if let Some(group) = group_ordinals[group] {
                    push_edge(
                        graph,
                        group,
                        first + beam,
                        NativeSigRelationKind::Containment,
                    );
                }
            }
            Pending::BeamBeam(source, target) => {
                push_edge(graph, source, target, NativeSigRelationKind::BeamBeam);
            }
        }
    }
    Ok(())
}

fn bind_surviving_pre_rest_beam_group(
    bindings: &mut NativeSigSystemBindings,
    pre_rest_group_ordinal: usize,
    vertex: NativeSigVertexId,
) -> Result<(), NativeSigError> {
    bindings.bind_beam_group(pre_rest_group_ordinal, vertex)
}

fn push_beam_vertex(graph: &mut NativeSigSystem, beam: &RawBeam) -> usize {
    let bounds = beam_bounds(beam.item);
    push_vertex(
        graph,
        NativeSigVertex {
            ordinal: 0,
            active: true,
            removed: false,
            frozen: false,
            kind: match beam.kind {
                BeamKind::Beam => NativeSigInterKind::Beam,
                BeamKind::Hook => NativeSigInterKind::BeamHook,
                BeamKind::SmallBeam => NativeSigInterKind::SmallBeam,
            },
            shape: Some(beam.kind.shape().to_owned()),
            grade: beam.grade,
            contextual_grade: None,
            bounds: NativeSigBounds {
                x: bounds.x,
                y: bounds.y,
                width: bounds.width,
                height: bounds.height,
            },
            abnormal: true,
            beam_geometry: Some(NativeSigBeamGeometry {
                x1: beam.item.median.x1,
                y1: beam.item.median.y1,
                x2: beam.item.median.x2,
                y2: beam.item.median.y2,
                height: beam.item.height,
            }),
        },
    )
}

/// Append one already graded active-CUE_BEAMS product in Java SIG insertion
/// order. Fixed-glyph registration is intentionally owned by the caller, just
/// as ordinary native BEAMS keeps glyph evidence beside rather than inside the
/// graph vertex.
pub fn append_native_cue_beam_vertex(
    graph: &mut NativeSigSystem,
    beam: &RawBeam,
) -> NativeSigVertexId {
    debug_assert_eq!(beam.kind, BeamKind::SmallBeam);
    NativeSigVertexId(push_beam_vertex(graph, beam))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeCueBeamStemRelationAppend {
    pub edge: NativeSigEdgeId,
    pub created: bool,
}

/// Append one committed active-CUE_BEAMS `BeamStemRelation`.
///
/// Java's `Link.applyTo()` is idempotent at the graph level and invokes the
/// relation's `added()` callback. The callback recomputes beam abnormality
/// after the relation payload has been installed. Keeping those semantics in
/// this small SIG primitive lets the cue-beam orchestration preserve Java's
/// group iteration order without duplicating graph invariants.
pub fn append_native_cue_beam_stem_relation(
    graph: &mut NativeSigSystem,
    beam: NativeSigVertexId,
    stem: NativeSigVertexId,
    grade: f64,
    beam_portion: NativeBeamPortion,
    stem_extension: NativeStemPoint,
) -> Result<NativeCueBeamStemRelationAppend, NativeSigError> {
    let system_id = graph.system_id;
    if graph
        .vertex(beam.0)
        .is_none_or(|vertex| vertex.kind != NativeSigInterKind::SmallBeam)
    {
        return Err(NativeSigError::InvalidVertexState {
            system_id,
            ordinal: beam.0,
        });
    }
    if graph
        .vertex(stem.0)
        .is_none_or(|vertex| vertex.kind != NativeSigInterKind::Stem)
    {
        return Err(NativeSigError::InvalidVertexState {
            system_id,
            ordinal: stem.0,
        });
    }
    if let Some(edge) = graph.edges.iter().find(|edge| {
        edge.active
            && edge.source == beam.0
            && edge.target == stem.0
            && edge.kind == NativeSigRelationKind::BeamStem
    }) {
        return Ok(NativeCueBeamStemRelationAppend {
            edge: NativeSigEdgeId(edge.ordinal),
            created: false,
        });
    }

    let edge = NativeSigEdgeId(graph.edges.len());
    graph.append_edge(NativeSigEdge {
        ordinal: edge.0,
        active: true,
        source: beam.0,
        target: stem.0,
        kind: NativeSigRelationKind::BeamStem,
        origin: NativeSigRelationOrigin::BaselineGraph,
        support: Some(NativeSigSupport {
            grade,
            bar_connection_impacts: None,
        }),
        beam_portion: Some(beam_portion),
        stem_extension: Some(stem_extension),
        head_stem: None,
    })?;

    let mut left = false;
    let mut right = false;
    for relation in graph.incident_edges(beam.0)?.into_iter().filter(|edge| {
        matches!(
            edge.kind,
            NativeSigRelationKind::BeamStem | NativeSigRelationKind::BeamRest
        )
    }) {
        match relation.beam_portion {
            Some(NativeBeamPortion::Left) => left = true,
            Some(NativeBeamPortion::Right) => right = true,
            Some(NativeBeamPortion::Center) | None => {}
        }
    }
    graph.set_abnormal(beam, !left || !right)?;

    Ok(NativeCueBeamStemRelationAppend {
        edge,
        created: true,
    })
}

/// Materialize Java `BeamGroupInter.populateCueAggregate` evidence into SIG.
///
/// `beam_vertices[beam_id]` is the already inserted `SmallBeamInter`. Group
/// identities, containment relations, and pairwise `BeamBeamRelation`s follow
/// the grouping kernel's provisional event order, including group merges.
pub fn append_native_cue_beam_groups(
    graph: &mut NativeSigSystem,
    beam_vertices: &[NativeSigVertexId],
    evidence: &audiveris_image::beam_groups::BeamGroupEvidence,
) -> Result<Vec<NativeSigVertexId>, NativeSigError> {
    let system_id = graph.system_id;
    for (index, &vertex) in beam_vertices.iter().enumerate() {
        if graph
            .vertices
            .get(vertex.0)
            .is_none_or(|inter| inter.kind != NativeSigInterKind::SmallBeam)
        {
            return Err(NativeSigError::InvalidBeamMember { system_id, index });
        }
    }

    let mut active = Vec::<bool>::new();
    for event in &evidence.events {
        match *event {
            audiveris_image::beam_groups::BeamGroupEvent::Created { group_index, .. } => {
                if active.len() <= group_index {
                    active.resize(group_index + 1, false);
                }
                active[group_index] = true;
            }
            audiveris_image::beam_groups::BeamGroupEvent::Merged { removed_index, .. } => {
                active[removed_index] = false;
            }
            audiveris_image::beam_groups::BeamGroupEvent::Added { .. } => {}
        }
    }

    let mut group_ordinals = vec![None; active.len()];
    let mut final_groups = evidence.groups.iter();
    let mut created_groups = Vec::with_capacity(evidence.groups.len());
    for (provisional, is_active) in active.iter().copied().enumerate() {
        if !is_active {
            continue;
        }
        let group = final_groups
            .next()
            .expect("one final cue group per active identity");
        let mut bounds = None;
        for &beam_id in group {
            let vertex = beam_vertices
                .get(beam_id)
                .ok_or(NativeSigError::InvalidBeamMember {
                    system_id,
                    index: beam_id,
                })?;
            let item = graph.vertices[vertex.0].bounds;
            bounds = Some(bounds.map_or(item, |current: NativeSigBounds| current.union(item)));
        }
        let bounds = bounds.ok_or(NativeSigError::InvalidBeamMember {
            system_id,
            index: provisional,
        })?;
        let vertex = NativeSigVertexId(push_vertex(
            graph,
            NativeSigVertex {
                ordinal: 0,
                active: true,
                removed: false,
                frozen: false,
                kind: NativeSigInterKind::BeamGroup,
                shape: None,
                grade: 1.0,
                contextual_grade: None,
                bounds,
                abnormal: false,
                beam_geometry: None,
            },
        ));
        group_ordinals[provisional] = Some(vertex);
        created_groups.push(vertex);
    }

    #[derive(Clone, Copy)]
    enum Pending {
        Containment(usize, usize),
        BeamBeam(usize, usize),
    }
    let mut members = vec![Vec::<usize>::new(); active.len()];
    let mut pending = Vec::<Pending>::new();
    let add_member = |group: usize,
                      beam_id: usize,
                      members: &mut Vec<Vec<usize>>,
                      pending: &mut Vec<Pending>| {
        if members[group].contains(&beam_id) {
            return;
        }
        pending.push(Pending::Containment(group, beam_id));
        for &old in &members[group] {
            let pair = (old.min(beam_id), old.max(beam_id));
            if !pending.iter().any(|edge| {
                matches!(*edge, Pending::BeamBeam(one, two) if (one.min(two), one.max(two)) == pair)
            }) {
                pending.push(Pending::BeamBeam(old, beam_id));
            }
        }
        members[group].push(beam_id);
    };
    for event in &evidence.events {
        match *event {
            audiveris_image::beam_groups::BeamGroupEvent::Created {
                group_index,
                beam_id,
            }
            | audiveris_image::beam_groups::BeamGroupEvent::Added {
                group_index,
                beam_id,
                ..
            } => add_member(group_index, beam_id, &mut members, &mut pending),
            audiveris_image::beam_groups::BeamGroupEvent::Merged {
                survivor_index,
                removed_index,
                ..
            } => {
                let moved = members[removed_index].clone();
                for beam_id in moved {
                    add_member(survivor_index, beam_id, &mut members, &mut pending);
                }
                pending.retain(|edge| {
                    !matches!(*edge, Pending::Containment(group, _) if group == removed_index)
                });
            }
        }
    }
    for edge in pending {
        match edge {
            Pending::Containment(group, beam_id) => {
                if let Some(group) = group_ordinals[group] {
                    let beam = beam_vertices[beam_id];
                    push_edge(graph, group.0, beam.0, NativeSigRelationKind::Containment);
                }
            }
            Pending::BeamBeam(one, two) => push_edge(
                graph,
                beam_vertices[one].0,
                beam_vertices[two].0,
                NativeSigRelationKind::BeamBeam,
            ),
        }
    }
    Ok(created_groups)
}

fn append_ledgers(
    ledgers: &NativeLedgerRecognition,
    system_id: usize,
    graph: &mut NativeSigSystem,
    bindings: &mut NativeSigSystemBindings,
) -> Result<(), NativeSigError> {
    for inter in ledgers
        .materializer
        .inters()
        .iter()
        .filter(|inter| inter.system_id == system_id && !inter.removed)
    {
        let ((x1, y1), (x2, y2)) = inter.median;
        let half = inter.thickness / 2.0;
        let left = x1.min(x2).floor();
        let top = (y1.min(y2) - half).floor();
        let right = x1.max(x2).ceil();
        let bottom = (y1.max(y2) + half).ceil();
        let vertex = NativeSigVertexId(push_vertex(
            graph,
            NativeSigVertex {
                ordinal: 0,
                active: true,
                removed: false,
                frozen: false,
                kind: NativeSigInterKind::Ledger,
                shape: Some("LEDGER".to_owned()),
                grade: inter.grade,
                contextual_grade: None,
                bounds: NativeSigBounds {
                    x: left as i32,
                    y: top as i32,
                    width: (right - left) as i32,
                    height: (bottom - top) as i32,
                },
                abnormal: false,
                beam_geometry: None,
            },
        ));
        if bindings.ledger_vertices.insert(inter.id, vertex).is_some() {
            return Err(NativeSigError::DuplicateLedgerBinding {
                system_id,
                inter_id: inter.id,
            });
        }
    }
    Ok(())
}

fn reduction_staff_bindings(
    grid: &GridLinesRecognition,
    headers: &crate::native_headers::NativeHeaderSystemRecognition,
    ledgers: &NativeLedgerRecognition,
    ledger_vertices: &BTreeMap<usize, NativeSigVertexId>,
) -> Result<Vec<NativeSigReductionStaff>, NativeSigError> {
    let mut staffs = Vec::with_capacity(headers.staffs.len());
    for header_staff in &headers.staffs {
        let staff_id = header_staff.staff_id;
        let geometry = grid
            .staff_lines
            .iter()
            .find(|staff| staff.staff_id == staff_id)
            .ok_or(NativeSigError::MissingStaffLine(staff_id))?;
        let candidate = grid
            .staves
            .iter()
            .find(|staff| staff.id == staff_id)
            .ok_or(NativeSigError::MissingStaffLine(staff_id))?;
        let ledger_lines = ledgers
            .ledger_lines
            .iter()
            .filter(|line| line.system_id == headers.system_id && line.staff_id == staff_id)
            .map(|line| (line.index, line.geometry.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut ledger_map = BTreeMap::new();
        for index in ledgers
            .materializer
            .staff_ledger_indexes(headers.system_id, staff_id)
        {
            let vertices = ledgers
                .materializer
                .staff_inter_ids(headers.system_id, staff_id, index)
                .iter()
                .map(|inter_id| {
                    ledger_vertices.get(inter_id).copied().ok_or(
                        NativeSigError::MissingLedgerBinding {
                            system_id: headers.system_id,
                            inter_id: *inter_id,
                        },
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            if !vertices.is_empty() {
                ledger_map.insert(index, vertices);
            }
        }
        staffs.push(NativeSigReductionStaff {
            staff_id,
            tablature: candidate.kind == "tablature",
            specific_interline: geometry.interline,
            line_count: candidate.line_count,
            first_line: geometry.first_line.clone(),
            last_line: geometry.last_line.clone(),
            ledger_lines,
            ledger_map,
        });
    }
    Ok(staffs)
}

fn append_heads(
    heads: &NativeHeadsRecognition,
    system: &crate::native_heads_epilog::NativeHeadsEpilogSystem,
    staff_system: &crate::native_heads_staff_epilog::NativeHeadsStaffEpilogSystem,
    graph: &mut NativeSigSystem,
    bindings: &mut NativeSigSystemBindings,
) -> Result<(), NativeSigError> {
    let first = graph.vertices.len();
    let removed = system
        .beam_removed_heads
        .iter()
        .map(|reference| (reference.staff_index, reference.head_index))
        .collect::<BTreeSet<_>>();
    let survivors = system
        .heads_in_sig_order
        .iter()
        .map(|reference| (reference.staff_index, reference.head_index))
        .filter(|key| !removed.contains(key))
        .collect::<Vec<_>>();
    for &(staff, head_index) in &survivors {
        let head = &staff_system.staffs[staff].heads[head_index];
        let vertex = push_vertex(
            graph,
            NativeSigVertex {
                ordinal: 0,
                active: true,
                removed: false,
                frozen: false,
                kind: NativeSigInterKind::Head,
                shape: Some(
                    match head.shape {
                        HeadTemplateShape::NoteheadBlack => "NOTEHEAD_BLACK",
                        HeadTemplateShape::NoteheadVoid => "NOTEHEAD_VOID",
                        HeadTemplateShape::WholeNote => "WHOLE_NOTE",
                        HeadTemplateShape::Breve => "BREVE",
                        HeadTemplateShape::NoteheadBlackSmall => "NOTEHEAD_BLACK_SMALL",
                        HeadTemplateShape::NoteheadVoidSmall => "NOTEHEAD_VOID_SMALL",
                        HeadTemplateShape::WholeNoteSmall => "WHOLE_NOTE_SMALL",
                        HeadTemplateShape::BreveSmall => "BREVE_SMALL",
                    }
                    .to_owned(),
                ),
                grade: f64::from_bits(head.grade_bits),
                contextual_grade: None,
                bounds: NativeSigBounds {
                    x: head.bounds.x,
                    y: head.bounds.y,
                    width: head.bounds.width,
                    height: head.bounds.height,
                },
                abnormal: true,
                beam_geometry: None,
            },
        );
        bindings.bind_head(
            NativeHeadStaffEpilogRef {
                staff_index: staff,
                head_index,
            },
            NativeSigVertexId(vertex),
        )?;
        let glyph = source_head_glyph(
            heads,
            system.system_id,
            staff_system.staffs[staff].staff_id,
            head.origin,
        )
        .ok_or(NativeSigError::MissingHeadsSystem(system.system_id))?;
        let bounds = graph.vertices[vertex].bounds;
        bindings.overlap_geometry.insert(
            NativeSigVertexId(vertex),
            NativeReductionInterGeometry::head(
                bounds,
                NativeReductionGlyphGeometry {
                    left: glyph.glyph_bounds.x,
                    top: glyph.glyph_bounds.y,
                    run_table: glyph.run_table.clone(),
                },
                Some(staff_system.staffs[staff].staff_id),
                f64::from_bits(head.pitch_bits).round_ties_even() as i32,
            ),
        );
    }
    let ordinal_of = survivors
        .iter()
        .enumerate()
        .map(|(index, &key)| (key, first + index))
        .collect::<std::collections::BTreeMap<_, _>>();
    for (staff_index, staff) in staff_system.staffs.iter().enumerate() {
        for decision in &staff.purge.overlap.decisions {
            let purged = (staff_index, decision.purged_index);
            let kept = (staff_index, decision.kept_index);
            if let (Some(&source), Some(&target)) = (ordinal_of.get(&purged), ordinal_of.get(&kept))
            {
                push_edge(graph, source, target, NativeSigRelationKind::Exclusion);
            }
        }
    }
    Ok(())
}

fn source_head_glyph(
    heads: &NativeHeadsRecognition,
    system_id: usize,
    staff_id: usize,
    origin: crate::native_heads_staff_epilog::NativeHeadStaffEpilogOrigin,
) -> Option<&crate::head_glyph_retrieval::RetrievedHeadGlyph> {
    use crate::native_heads_staff_epilog::NativeHeadStaffEpilogOrigin;

    match origin {
        NativeHeadStaffEpilogOrigin::Seed(ordinal) => heads
            .seed_glyphs
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .and_then(|system| {
                system
                    .staffs
                    .iter()
                    .find(|staff| staff.staff_id == staff_id)
            })
            .and_then(|staff| staff.heads.iter().find(|source| source.ordinal == ordinal))
            .map(|source| &source.glyph),
        NativeHeadStaffEpilogOrigin::Range(ordinal) => heads
            .range_glyphs
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .and_then(|system| {
                system
                    .staffs
                    .iter()
                    .find(|staff| staff.staff_id == staff_id)
            })
            .and_then(|staff| staff.heads.iter().find(|source| source.ordinal == ordinal))
            .map(|source| &source.glyph),
    }
}

#[cfg(test)]
mod overlap_geometry_tests {
    use super::*;
    use audiveris_image::{
        bar_column::StaffId,
        bars_logic::{VerticalInterPlan, VerticalMedian},
        beam_structure::{BeamImpacts, BeamItem, BeamRasterEvidence, Segment},
        staff_peak::StaffPeak,
    };

    #[test]
    fn surviving_beam_group_bindings_keep_pre_rest_ordinal_holes() {
        let mut bindings = NativeSigSystemBindings {
            system_id: 1,
            beam_vertices: BTreeMap::new(),
            beam_group_vertices: BTreeMap::new(),
            stem_vertices: BTreeMap::new(),
            head_vertices: BTreeMap::new(),
            ledger_vertices: BTreeMap::new(),
            reduction_interline: 10,
            reduction_staffs: Vec::new(),
            merged_staff_pairs: Vec::new(),
            overlap_geometry: BTreeMap::new(),
        };

        bind_surviving_pre_rest_beam_group(&mut bindings, 0, NativeSigVertexId(20)).unwrap();
        // Group 1 was extensively removed with its last member. Java's later
        // STEMS products still refer to the following survivor as group 2.
        bind_surviving_pre_rest_beam_group(&mut bindings, 2, NativeSigVertexId(21)).unwrap();

        assert_eq!(
            bindings.beam_group_vertices,
            BTreeMap::from([(0, NativeSigVertexId(20)), (2, NativeSigVertexId(21)),])
        );
        assert!(!bindings.beam_group_vertices.contains_key(&1));
    }

    #[test]
    fn cue_beam_append_allocates_the_next_small_beam_vertex() {
        let mut graph = NativeSigSystem {
            system_id: 3,
            vertices: Vec::new(),
            edges: Vec::new(),
        };
        let beam = RawBeam {
            kind: BeamKind::SmallBeam,
            item: BeamItem {
                median: Segment {
                    x1: 10.0,
                    y1: 20.0,
                    x2: 40.0,
                    y2: 22.0,
                },
                height: 4.0,
            },
            impacts: BeamImpacts {
                width: 1.0,
                min_height: 1.0,
                max_height: 1.0,
                core: 1.0,
                belt: 1.0,
                distance: 1.0,
                raster: BeamRasterEvidence {
                    core_foreground: 120,
                    core_count: 120,
                    belt_foreground: 0,
                    belt_count: 60,
                    core_ratio: 1.0,
                    belt_ratio: 0.0,
                    rounded_width: 31,
                },
            },
            grade: 0.8,
        };

        let id = append_native_cue_beam_vertex(&mut graph, &beam);

        assert_eq!(id, NativeSigVertexId(0));
        let vertex = graph.vertex(id.0).unwrap();
        assert_eq!(vertex.kind, NativeSigInterKind::SmallBeam);
        assert_eq!(vertex.shape.as_deref(), Some("BEAM_SMALL"));
        assert_eq!(vertex.grade, 0.8);
        assert!(vertex.abnormal);
        assert_eq!(vertex.beam_geometry.unwrap().height, 4.0);
    }

    #[test]
    fn cue_beam_stem_append_is_idempotent_and_runs_beam_callback() {
        fn stem_vertex(x: i32) -> NativeSigVertex {
            NativeSigVertex {
                ordinal: 0,
                active: true,
                removed: false,
                frozen: false,
                kind: NativeSigInterKind::Stem,
                shape: Some("STEM".to_owned()),
                grade: 0.7,
                contextual_grade: None,
                bounds: NativeSigBounds {
                    x,
                    y: 10,
                    width: 2,
                    height: 40,
                },
                abnormal: false,
                beam_geometry: None,
            }
        }

        let beam = RawBeam {
            kind: BeamKind::SmallBeam,
            item: BeamItem {
                median: Segment {
                    x1: 10.0,
                    y1: 20.0,
                    x2: 40.0,
                    y2: 20.0,
                },
                height: 4.0,
            },
            impacts: BeamImpacts {
                width: 1.0,
                min_height: 1.0,
                max_height: 1.0,
                core: 1.0,
                belt: 1.0,
                distance: 1.0,
                raster: BeamRasterEvidence {
                    core_foreground: 120,
                    core_count: 120,
                    belt_foreground: 0,
                    belt_count: 60,
                    core_ratio: 1.0,
                    belt_ratio: 0.0,
                    rounded_width: 31,
                },
            },
            grade: 0.8,
        };
        let mut graph = NativeSigSystem {
            system_id: 3,
            vertices: Vec::new(),
            edges: Vec::new(),
        };
        let beam_id = append_native_cue_beam_vertex(&mut graph, &beam);
        let left_stem = NativeSigVertexId(push_vertex(&mut graph, stem_vertex(10)));
        let right_stem = NativeSigVertexId(push_vertex(&mut graph, stem_vertex(39)));

        let left = append_native_cue_beam_stem_relation(
            &mut graph,
            beam_id,
            left_stem,
            0.6,
            NativeBeamPortion::Left,
            NativeStemPoint { x: 11.0, y: 22.0 },
        )
        .unwrap();
        assert!(left.created);
        assert!(graph.vertex(beam_id.0).unwrap().abnormal);
        assert_eq!(graph.edges[left.edge.0].support.unwrap().grade, 0.6);

        let duplicate = append_native_cue_beam_stem_relation(
            &mut graph,
            beam_id,
            left_stem,
            0.1,
            NativeBeamPortion::Center,
            NativeStemPoint { x: 0.0, y: 0.0 },
        )
        .unwrap();
        assert_eq!(duplicate.edge, left.edge);
        assert!(!duplicate.created);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[left.edge.0].support.unwrap().grade, 0.6);

        let right = append_native_cue_beam_stem_relation(
            &mut graph,
            beam_id,
            right_stem,
            0.6,
            NativeBeamPortion::Right,
            NativeStemPoint { x: 40.0, y: 22.0 },
        )
        .unwrap();
        assert!(right.created);
        assert!(!graph.vertex(beam_id.0).unwrap().abnormal);
        assert_eq!(graph.edges.len(), 2);
        graph.validate_integrity().unwrap();
    }

    #[test]
    fn cue_beam_group_append_preserves_event_relation_order() {
        use audiveris_image::beam_groups::{
            BeamGroupParameters, GroupingBeam, group_beam_evidence,
        };

        fn cue_beam(y: f64) -> RawBeam {
            RawBeam {
                kind: BeamKind::SmallBeam,
                item: BeamItem {
                    median: Segment {
                        x1: 10.0,
                        y1: y,
                        x2: 40.0,
                        y2: y,
                    },
                    height: 4.0,
                },
                impacts: BeamImpacts {
                    width: 1.0,
                    min_height: 1.0,
                    max_height: 1.0,
                    core: 1.0,
                    belt: 1.0,
                    distance: 1.0,
                    raster: BeamRasterEvidence {
                        core_foreground: 120,
                        core_count: 120,
                        belt_foreground: 0,
                        belt_count: 60,
                        core_ratio: 1.0,
                        belt_ratio: 0.0,
                        rounded_width: 31,
                    },
                },
                grade: 0.8,
            }
        }

        let beams = [cue_beam(20.0), cue_beam(27.0)];
        let members = beams
            .iter()
            .enumerate()
            .map(|(id, beam)| GroupingBeam {
                id,
                median: beam.item.median,
                height: beam.item.height,
                bounds: crate::beam_inters::beam_bounds(beam.item),
            })
            .collect::<Vec<_>>();
        let evidence = group_beam_evidence(
            &members,
            BeamGroupParameters {
                min_x_overlap: 7.0,
                max_y_distance: 10.0,
                max_slope_diff: 0.2,
            },
        );
        assert_eq!(evidence.groups, [vec![0, 1]]);

        let mut graph = NativeSigSystem {
            system_id: 3,
            vertices: Vec::new(),
            edges: Vec::new(),
        };
        let beam_vertices = beams
            .iter()
            .map(|beam| append_native_cue_beam_vertex(&mut graph, beam))
            .collect::<Vec<_>>();
        let groups = append_native_cue_beam_groups(&mut graph, &beam_vertices, &evidence).unwrap();

        assert_eq!(groups, [NativeSigVertexId(2)]);
        assert_eq!(graph.vertices[2].kind, NativeSigInterKind::BeamGroup);
        assert_eq!(
            graph.vertices[2].bounds,
            NativeSigBounds {
                x: 10,
                y: 18,
                width: 30,
                height: 11
            }
        );
        assert_eq!(
            graph
                .edges
                .iter()
                .map(|edge| (edge.source, edge.target, edge.kind))
                .collect::<Vec<_>>(),
            [
                (2, 0, NativeSigRelationKind::Containment),
                (2, 1, NativeSigRelationKind::Containment),
                (0, 1, NativeSigRelationKind::BeamBeam),
            ]
        );
    }

    #[test]
    fn bracket_area_and_bounds_include_both_bravura_serifs() {
        let peak = StaffPeak::new(StaffId::new(1), 20, 80, 98, 101).expect("valid peak");
        let plan = VerticalInterPlan {
            peak: peak.key(),
            median: VerticalMedian {
                x: 100.0,
                top: 20.0,
                bottom: 80.0,
            },
            width: 4.0,
            impacts: None,
            kind: VerticalInterKind::Bracket(BracketKind::Both),
        };
        let interline = 20;

        let area = bracket_area(&plan, interline).expect("Bravura serif geometry");
        assert_eq!(area.components.len(), 3, "ribbon plus two serifs");

        let upper = layout_bounds(MusicFamily::Bravura, "BRACKET_UPPER_SERIF", interline)
            .expect("Bravura parses")
            .expect("upper serif exists");
        let lower = layout_bounds(MusicFamily::Bravura, "BRACKET_LOWER_SERIF", interline)
            .expect("Bravura parses")
            .expect("lower serif exists");
        let left = 98.0_f64;
        let expected_left = left.floor() as i32;
        let expected_top = (20.0 - upper.height).min(20.0).floor() as i32;
        let expected_right = 102.0_f64
            .max(left + upper.width)
            .max(left + lower.width)
            .ceil() as i32;
        let expected_bottom = 80.0_f64.max(80.0 + lower.height).ceil() as i32;
        assert_eq!(
            area_integer_bounds(&area),
            NativeSigBounds {
                x: expected_left,
                y: expected_top,
                width: expected_right - expected_left,
                height: expected_bottom - expected_top,
            }
        );
        assert!(expected_top < 20, "upper serif expands above the ribbon");
        assert!(expected_right > 102, "serifs expand right of the ribbon");
        assert!(expected_bottom > 80, "lower serif expands below the ribbon");
    }
}
