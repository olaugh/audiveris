// SPDX-License-Identifier: AGPL-3.0-or-later

//! The recognition-owned SIG at the boundary after HEADS and before STEMS.
//!
//! GRID, HEADERS, BEAMS, LEDGERS, and HEADS each expose complete typed products,
//! but historically the Rust port kept those products in separate collections.
//! This module gives their live interpretations one per-system identity domain and
//! preserves the insertion order Java's `SIGraph` exposes to later `edgesOf` scans.

use std::{collections::BTreeSet, error::Error, fmt};

use audiveris_image::{
    bars_logic::{BracketKind, ConnectorInterKind, PeakWidthClass, VerticalInterKind},
    grid_sig::{GridInterId, GridSigNode, GridSigRelation},
};

use crate::{
    beam_inters::{BeamKind, RawBeam, beam_bounds},
    clef_column::NeutralClefKind,
    head_template::HeadTemplateShape,
    header_time_column::{NeutralSpecificTimeShape, NeutralTimeCandidate},
    native_headers::{NativeHeaderRecognition, NativeHeaderStaffRecognition},
    native_heads::NativeHeadsRecognition,
    native_ledgers::NativeLedgerRecognition,
    recognize::{GridLinesRecognition, NativeBeamRecognition},
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
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeSigVertex {
    /// Zero-based system-local insertion ordinal.
    pub ordinal: usize,
    pub kind: NativeSigInterKind,
    /// Java `Shape.name()`, or `None` for shape-less ensembles.
    pub shape: Option<String>,
    pub grade: f64,
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
pub struct NativeSigEdge {
    pub ordinal: usize,
    pub source: usize,
    pub target: usize,
    pub kind: NativeSigRelationKind,
    pub support: Option<NativeSigSupport>,
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
        self.vertices.get(ordinal)
    }

    /// Java/JGraphT `incomingEdgesOf`, retaining global edge insertion order.
    pub fn incoming_edges(&self, vertex: usize) -> Result<Vec<&NativeSigEdge>, NativeSigError> {
        self.require_vertex(vertex)?;
        Ok(self
            .edges
            .iter()
            .filter(|edge| edge.target == vertex)
            .collect())
    }

    /// Java/JGraphT `outgoingEdgesOf`, retaining global edge insertion order.
    pub fn outgoing_edges(&self, vertex: usize) -> Result<Vec<&NativeSigEdge>, NativeSigError> {
        self.require_vertex(vertex)?;
        Ok(self
            .edges
            .iter()
            .filter(|edge| edge.source == vertex)
            .collect())
    }

    /// Java/JGraphT `edgesOf`: incoming insertion order followed by outgoing order.
    pub fn incident_edges(&self, vertex: usize) -> Result<Vec<&NativeSigEdge>, NativeSigError> {
        self.require_vertex(vertex)?;
        Ok(self
            .edges
            .iter()
            .filter(|edge| edge.target == vertex)
            .chain(self.edges.iter().filter(|edge| edge.source == vertex))
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
            .filter(|edge| edge.source == source && edge.target == target)
            .collect())
    }

    fn require_vertex(&self, ordinal: usize) -> Result<(), NativeSigError> {
        if self.vertices.get(ordinal).is_some() {
            Ok(())
        } else {
            Err(NativeSigError::MissingVertex {
                system_id: self.system_id,
                ordinal,
            })
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeSigRecognition {
    pub systems: Vec<NativeSigSystem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeSigError {
    MissingGridSystem(usize),
    MissingHeaderSystem(usize),
    MissingLedgerGlyph(usize),
    MissingHeadsSystem(usize),
    MissingBraceGlyph(usize),
    MissingStaffLine(usize),
    MissingSelected(&'static str, usize),
    MissingBeamGroup(usize),
    MissingVertex { system_id: usize, ordinal: usize },
    InvalidBeamMember { system_id: usize, index: usize },
    UnsupportedTime { numerator: i32, denominator: i32 },
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
            Self::MissingStaffLine(id) => write!(formatter, "missing staff-line geometry {id}"),
            Self::MissingSelected(kind, staff) => {
                write!(formatter, "missing selected {kind} for staff {staff}")
            }
            Self::MissingBeamGroup(id) => write!(formatter, "missing BEAMS groups for system {id}"),
            Self::MissingVertex { system_id, ordinal } => {
                write!(formatter, "system {system_id} has no SIG vertex {ordinal}")
            }
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
        append_grid(grid, grid_system, &mut graph)?;
        append_headers(header_system.staffs.as_slice(), &mut graph)?;
        append_beams(
            beams,
            system_id,
            grid_system.interline.round() as i32,
            &mut graph,
        )?;
        append_ledgers(ledgers, system_id, &mut graph)?;
        append_heads(head_system, staff_head_system, &mut graph);
        systems.push(graph);
    }
    Ok(NativeSigRecognition { systems })
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
        | NativeSigRelationKind::BeamBeam => Some(NativeSigSupport {
            grade: 1.0,
            bar_connection_impacts: None,
        }),
        NativeSigRelationKind::BarConnection
        | NativeSigRelationKind::BarGroup
        | NativeSigRelationKind::Containment
        | NativeSigRelationKind::Exclusion => None,
    };
    graph.edges.push(NativeSigEdge {
        ordinal,
        source,
        target,
        kind,
        support,
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
        source,
        target,
        kind: NativeSigRelationKind::BarConnection,
        support: Some(NativeSigSupport {
            grade,
            bar_connection_impacts: Some(impacts),
        }),
    });
}

fn append_grid(
    grid: &GridLinesRecognition,
    system: &crate::grid_executor::HeadlessSystemSigState,
    graph: &mut NativeSigSystem,
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
        push_vertex(
            graph,
            NativeSigVertex {
                ordinal: 0,
                kind: NativeSigInterKind::Brace,
                shape: Some("BRACE".to_owned()),
                grade: brace.grade,
                bounds: NativeSigBounds {
                    x,
                    y: y1,
                    width,
                    height: y2.saturating_sub(y1).saturating_add(1),
                },
                abnormal: false,
                beam_geometry: None,
            },
        );
    }

    let base = graph.vertices.len();
    let ordered = system.sig.nodes_in_order().collect::<Vec<_>>();
    for (_, node) in &ordered {
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
                        bounds,
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
        push_vertex(
            graph,
            NativeSigVertex {
                ordinal: 0,
                kind,
                shape: Some(shape.to_owned()),
                grade: node.intrinsic_grade(),
                bounds,
                abnormal: false,
                beam_geometry: None,
            },
        );
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
            kind,
            shape: shape.map(str::to_owned),
            grade,
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
    system_id: usize,
    interline: i32,
    graph: &mut NativeSigSystem,
) -> Result<(), NativeSigError> {
    let first = graph.vertices.len();
    let created = beams
        .beams_after_multiple_rests
        .iter()
        .filter(|(id, _)| *id == system_id)
        .map(|(_, beam)| beam)
        .chain(
            beams
                .hooks
                .iter()
                .filter(|(id, _)| *id == system_id)
                .map(|(_, beam)| beam),
        )
        .collect::<Vec<_>>();
    for beam in &created {
        push_beam_vertex(graph, beam);
    }
    let exclusions = created
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| {
            pair[0].item == pair[1].item
                && pair[0].kind == BeamKind::Hook
                && pair[1].kind != BeamKind::Hook
        })
        .map(|(index, _)| (first + index, first + index + 1))
        .collect::<BTreeSet<_>>();
    for &(source, target) in &exclusions {
        push_edge(graph, source, target, NativeSigRelationKind::Exclusion);
    }

    let evidence = crate::beam_inters::group_beams(
        &created.iter().map(|beam| **beam).collect::<Vec<_>>(),
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
    let mut final_groups = groups.groups.iter();
    for (provisional, is_active) in active.iter().copied().enumerate() {
        if !is_active {
            continue;
        }
        let group = final_groups
            .next()
            .expect("one final group per active identity");
        let mut bounds = None;
        for &index in group {
            let beam = created
                .get(index)
                .ok_or(NativeSigError::InvalidBeamMember { system_id, index })?;
            let item = beam_bounds(beam.item);
            let item = NativeSigBounds {
                x: item.x,
                y: item.y,
                width: item.width,
                height: item.height,
            };
            bounds = Some(bounds.map_or(item, |current: NativeSigBounds| current.union(item)));
        }
        group_ordinals[provisional] = Some(push_vertex(
            graph,
            NativeSigVertex {
                ordinal: 0,
                kind: NativeSigInterKind::BeamGroup,
                shape: None,
                grade: 1.0,
                bounds: bounds.unwrap_or(NativeSigBounds {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                }),
                abnormal: false,
                beam_geometry: None,
            },
        ));
    }

    #[derive(Clone, Copy)]
    enum Pending {
        Containment(usize, usize),
        BeamBeam(usize, usize),
    }
    let mut members = vec![Vec::<usize>::new(); active.len()];
    let mut pending = Vec::<Pending>::new();
    let add_member = |group: usize,
                      beam: usize,
                      members: &mut Vec<Vec<usize>>,
                      pending: &mut Vec<Pending>| {
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

fn push_beam_vertex(graph: &mut NativeSigSystem, beam: &RawBeam) {
    let bounds = beam_bounds(beam.item);
    push_vertex(
        graph,
        NativeSigVertex {
            ordinal: 0,
            kind: match beam.kind {
                BeamKind::Beam => NativeSigInterKind::Beam,
                BeamKind::Hook => NativeSigInterKind::BeamHook,
                BeamKind::SmallBeam => NativeSigInterKind::SmallBeam,
            },
            shape: Some(beam.kind.shape().to_owned()),
            grade: beam.grade,
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
    );
}

fn append_ledgers(
    ledgers: &NativeLedgerRecognition,
    system_id: usize,
    graph: &mut NativeSigSystem,
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
        push_vertex(
            graph,
            NativeSigVertex {
                ordinal: 0,
                kind: NativeSigInterKind::Ledger,
                shape: Some("LEDGER".to_owned()),
                grade: inter.grade,
                bounds: NativeSigBounds {
                    x: left as i32,
                    y: top as i32,
                    width: (right - left) as i32,
                    height: (bottom - top) as i32,
                },
                abnormal: false,
                beam_geometry: None,
            },
        );
    }
    Ok(())
}

fn append_heads(
    system: &crate::native_heads_epilog::NativeHeadsEpilogSystem,
    staff_system: &crate::native_heads_staff_epilog::NativeHeadsStaffEpilogSystem,
    graph: &mut NativeSigSystem,
) {
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
    for &(staff, head) in &survivors {
        let head = &staff_system.staffs[staff].heads[head];
        push_vertex(
            graph,
            NativeSigVertex {
                ordinal: 0,
                kind: NativeSigInterKind::Head,
                shape: Some(
                    match head.shape {
                        HeadTemplateShape::NoteheadBlack => "NOTEHEAD_BLACK",
                        HeadTemplateShape::NoteheadVoid => "NOTEHEAD_VOID",
                        HeadTemplateShape::WholeNote => "WHOLE_NOTE",
                        HeadTemplateShape::Breve => "BREVE",
                    }
                    .to_owned(),
                ),
                grade: f64::from_bits(head.grade_bits),
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
}
