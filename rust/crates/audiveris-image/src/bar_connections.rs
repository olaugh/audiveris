// SPDX-License-Identifier: AGPL-3.0-or-later

//! Pixel-backed Java `PeakGraph.findConnections` seam.
//!
//! Initial alignment edges are snapshotted in graph order. Each qualifying
//! edge is removed and reinserted as a connection, receiving a fresh edge ID
//! at the end of the graph. No split or conflict-purge behavior belongs here.

use std::{error::Error, fmt};

use crate::{
    bar_alignment::{BarAlignment, BarAlignmentError, BarAlignmentKind},
    bar_sticks::BarStick,
    peak_graph::{PeakEdgeId, PeakGraph, PeakGraphError},
    staff_peak::StaffPeakKey,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionRaster<'a> {
    pub width: usize,
    pub height: usize,
    /// Zero is foreground, matching Audiveris `ByteProcessor` binary input.
    pub pixels: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConnectionParameters {
    pub maximum_gap: i32,
    pub maximum_white_ratio: f64,
}

/// Java `AreaUtil.CoreData` measured between one aligned peak pair.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConnectionCoreData {
    pub length: i32,
    pub gap: i32,
    pub white_ratio: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionDecision {
    pub alignment_edge: PeakEdgeId,
    pub source: StaffPeakKey,
    pub target: StaffPeakKey,
    pub top_filament_id: usize,
    pub bottom_filament_id: usize,
    pub top_section_count: usize,
    pub bottom_section_count: usize,
    pub core: ConnectionCoreData,
    pub promoted_edge: Option<PeakEdgeId>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConnectionBuildReport {
    decisions: Vec<ConnectionDecision>,
}

impl ConnectionBuildReport {
    #[must_use]
    pub fn decisions(&self) -> &[ConnectionDecision] {
        &self.decisions
    }

    #[must_use]
    pub fn promoted_count(&self) -> usize {
        self.decisions
            .iter()
            .filter(|decision| decision.promoted_edge.is_some())
            .count()
    }

    /// Append one immediate split-peak `checkConnection` decision.
    pub fn record(&mut self, decision: ConnectionDecision) {
        self.decisions.push(decision);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionBuildError {
    InvalidRaster,
    InvalidParameters,
    DuplicateStick(StaffPeakKey),
    EmptyStick(StaffPeakKey),
    MissingStick(StaffPeakKey),
    WrongRelation(PeakEdgeId),
    ProbeOutOfBounds { x: i32, y: i32 },
    InvalidProbeRange { y_min: i32, y_max: i32 },
    Relation(BarAlignmentError),
    Graph(PeakGraphError),
}

impl fmt::Display for ConnectionBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRaster => formatter.write_str("invalid connection raster"),
            Self::InvalidParameters => formatter.write_str("invalid connection parameters"),
            Self::DuplicateStick(peak) => {
                write!(formatter, "peak has duplicate bar sticks: {peak:?}")
            }
            Self::EmptyStick(peak) => write!(formatter, "peak bar stick has no sections: {peak:?}"),
            Self::MissingStick(peak) => {
                write!(formatter, "alignment peak has no bar stick: {peak:?}")
            }
            Self::WrongRelation(edge) => write!(
                formatter,
                "connection traversal edge {} is not an alignment",
                edge.value()
            ),
            Self::ProbeOutOfBounds { x, y } => {
                write!(
                    formatter,
                    "connection probe pixel is out of bounds: ({x},{y})"
                )
            }
            Self::InvalidProbeRange { y_min, y_max } => {
                write!(formatter, "invalid connection probe rows {y_min}..={y_max}")
            }
            Self::Relation(source) => write!(formatter, "connection relation failed: {source}"),
            Self::Graph(source) => write!(formatter, "connection graph mutation failed: {source}"),
        }
    }
}

impl Error for ConnectionBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Relation(source) => Some(source),
            Self::Graph(source) => Some(source),
            _ => None,
        }
    }
}

/// Traverse the initial alignment snapshot and promote pixel-backed
/// connections. Failures retain every earlier decision and graph mutation.
pub fn find_connections(
    graph: &mut PeakGraph<BarAlignment>,
    raster: ConnectionRaster<'_>,
    sticks: &[BarStick],
    parameters: ConnectionParameters,
    report: &mut ConnectionBuildReport,
) -> Result<(), ConnectionBuildError> {
    validate(raster, sticks, parameters)?;
    let snapshot = graph
        .edges()
        .iter()
        .map(|edge| (edge.id(), *edge.relation()))
        .collect::<Vec<_>>();

    for (edge_id, alignment) in snapshot {
        if alignment.kind() != BarAlignmentKind::Alignment {
            return Err(ConnectionBuildError::WrongRelation(edge_id));
        }
        report.decisions.push(check_connection_edge(
            graph, edge_id, raster, sticks, parameters,
        )?);
    }
    Ok(())
}

/// Evaluate and, when qualified, promote one alignment edge. This is the
/// immediate `checkConnection` operation used after a split peak is inserted.
pub fn check_connection_edge(
    graph: &mut PeakGraph<BarAlignment>,
    edge_id: PeakEdgeId,
    raster: ConnectionRaster<'_>,
    sticks: &[BarStick],
    parameters: ConnectionParameters,
) -> Result<ConnectionDecision, ConnectionBuildError> {
    validate(raster, sticks, parameters)?;
    let edge =
        graph
            .edge(edge_id)
            .ok_or(ConnectionBuildError::Graph(PeakGraphError::MissingEdge(
                edge_id,
            )))?;
    if edge.relation().kind() != BarAlignmentKind::Alignment {
        return Err(ConnectionBuildError::WrongRelation(edge_id));
    }
    let source = edge.source();
    let target = edge.target();
    let alignment = *edge.relation();
    let top_stick = stick_for(sticks, source)?;
    let bottom_stick = stick_for(sticks, target)?;
    let top =
        graph
            .vertex(source)
            .ok_or(ConnectionBuildError::Graph(PeakGraphError::MissingVertex(
                source,
            )))?;
    let bottom =
        graph
            .vertex(target)
            .ok_or(ConnectionBuildError::Graph(PeakGraphError::MissingVertex(
                target,
            )))?;
    let core = connection_core(raster, top, bottom)?;
    let mut decision = ConnectionDecision {
        alignment_edge: edge_id,
        source,
        target,
        top_filament_id: top_stick.id,
        bottom_filament_id: bottom_stick.id,
        top_section_count: top_stick.members.len(),
        bottom_section_count: bottom_stick.members.len(),
        core,
        promoted_edge: None,
    };

    if core.gap <= parameters.maximum_gap && core.white_ratio <= parameters.maximum_white_ratio {
        let gap_impact = 1.0 - (f64::from(core.gap) / f64::from(parameters.maximum_gap));
        let white_impact = 1.0 - (core.white_ratio / parameters.maximum_white_ratio);
        let connection = BarAlignment::connection(&alignment, gap_impact, white_impact)
            .map_err(ConnectionBuildError::Relation)?;
        let (_, promoted) = graph
            .replace_edge(edge_id, connection)
            .map_err(ConnectionBuildError::Graph)?;
        decision.promoted_edge = Some(promoted);
    }
    Ok(decision)
}

fn validate(
    raster: ConnectionRaster<'_>,
    sticks: &[BarStick],
    parameters: ConnectionParameters,
) -> Result<(), ConnectionBuildError> {
    if raster.width == 0
        || raster.height == 0
        || raster.width.checked_mul(raster.height) != Some(raster.pixels.len())
    {
        return Err(ConnectionBuildError::InvalidRaster);
    }
    if parameters.maximum_gap <= 0
        || !parameters.maximum_white_ratio.is_finite()
        || parameters.maximum_white_ratio <= 0.0
    {
        return Err(ConnectionBuildError::InvalidParameters);
    }
    for (index, stick) in sticks.iter().enumerate() {
        if stick.members.is_empty() {
            return Err(ConnectionBuildError::EmptyStick(stick.peak));
        }
        if sticks[..index]
            .iter()
            .any(|previous| previous.peak == stick.peak)
        {
            return Err(ConnectionBuildError::DuplicateStick(stick.peak));
        }
    }
    Ok(())
}

fn stick_for(sticks: &[BarStick], peak: StaffPeakKey) -> Result<&BarStick, ConnectionBuildError> {
    sticks
        .iter()
        .find(|stick| stick.peak == peak)
        .ok_or(ConnectionBuildError::MissingStick(peak))
}

fn connection_core(
    raster: ConnectionRaster<'_>,
    top: &crate::staff_peak::StaffPeak,
    bottom: &crate::staff_peak::StaffPeak,
) -> Result<ConnectionCoreData, ConnectionBuildError> {
    let y_min = top.bottom();
    let y_max = bottom.top();
    if y_min > y_max {
        return Err(ConnectionBuildError::InvalidProbeRange { y_min, y_max });
    }
    let mut largest_gap = 0;
    let mut last_black_y = -1;
    let mut last_white_y = -1;
    let mut white_count = 0;

    for y in y_min..=y_max {
        let left = interpolate_x(
            f64::from(top.start()),
            f64::from(y_min),
            f64::from(bottom.start()),
            f64::from(y_max),
            f64::from(y),
        )
        .floor() as i32;
        let right = interpolate_x(
            f64::from(top.stop()),
            f64::from(y_min),
            f64::from(bottom.stop()),
            f64::from(y_max),
            f64::from(y),
        )
        .ceil() as i32;
        let mut empty = true;
        for x in left..=right {
            let x_index = usize::try_from(x)
                .ok()
                .filter(|&x| x < raster.width)
                .ok_or(ConnectionBuildError::ProbeOutOfBounds { x, y })?;
            let y_index = usize::try_from(y)
                .ok()
                .filter(|&y| y < raster.height)
                .ok_or(ConnectionBuildError::ProbeOutOfBounds { x, y })?;
            if raster.pixels[(y_index * raster.width) + x_index] == 0 {
                empty = false;
                break;
            }
        }
        if empty {
            white_count += 1;
            last_white_y = y;
            continue;
        }
        if last_white_y != -1 && last_black_y != -1 {
            largest_gap = largest_gap.max(last_white_y - last_black_y);
            last_white_y = -1;
        }
        last_black_y = y;
    }
    let length = y_max.wrapping_sub(y_min).wrapping_add(1);
    Ok(ConnectionCoreData {
        length,
        gap: largest_gap,
        white_ratio: f64::from(white_count) / f64::from(length),
    })
}

fn interpolate_x(x1: f64, y1: f64, x2: f64, y2: f64, y: f64) -> f64 {
    if y1 == y2 {
        x1
    } else {
        x1 + (((y - y1) / (y2 - y1)) * (x2 - x1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bar_alignment::{AlignmentPeak, BarImpacts},
        bar_column::{PeakId, StaffId},
        bars_logic::{LocatedSectionId, SectionLag},
        section::Bounds,
        staff_peak::{PeakPoint, StaffPeak, StaffVerticalImpacts},
    };

    fn peak(staff: usize, top: i32, bottom: i32, start: i32, stop: i32) -> StaffPeak {
        let mut peak = StaffPeak::with_impacts(
            StaffId::new(staff),
            top,
            bottom,
            start,
            stop,
            StaffVerticalImpacts::new(1.0, 1.0, 1.0, 1.0, 1.0, 1.0),
        )
        .unwrap();
        peak.compute_deskewed_center(|point| point).unwrap();
        peak
    }

    fn relation(
        top: &StaffPeak,
        bottom: &StaffPeak,
        top_id: usize,
        bottom_id: usize,
    ) -> BarAlignment {
        let alignment_peak = |peak: &StaffPeak, id| {
            AlignmentPeak::with_geometry(
                PeakId::new(id),
                peak.staff_id(),
                peak.start(),
                peak.width() as usize,
                peak.top(),
                peak.bottom(),
                peak.impacts().unwrap().grade(),
            )
            .unwrap()
        };
        BarAlignment::new(
            alignment_peak(top, top_id),
            alignment_peak(bottom, bottom_id),
            0.0,
            0.0,
            BarImpacts::alignment(1.0, 1.0).unwrap(),
        )
        .unwrap()
    }

    fn stick(id: usize, peak: StaffPeakKey) -> BarStick {
        BarStick {
            id,
            peak,
            members: vec![LocatedSectionId {
                lag: SectionLag::Vertical,
                id,
            }],
            bounds: Bounds {
                x: peak.start() as usize,
                y: 0,
                width: (peak.stop() - peak.start() + 1) as usize,
                height: 1,
            },
            points: vec![PeakPoint::new(0.0, 0.0), PeakPoint::new(0.0, 1.0)],
            mean_curvature: f64::INFINITY,
            marked_brace: false,
        }
    }

    fn parameters() -> ConnectionParameters {
        ConnectionParameters {
            maximum_gap: 2,
            maximum_white_ratio: 0.25,
        }
    }

    #[test]
    fn solid_sloped_core_promotes_with_fresh_edge_id_and_ownership() {
        let top = peak(1, 1, 4, 5, 6);
        let bottom = peak(2, 12, 15, 7, 8);
        let mut graph = PeakGraph::new();
        assert!(graph.add_vertex(top.clone()));
        assert!(graph.add_vertex(bottom.clone()));
        let alignment_id = graph
            .add_edge(top.key(), bottom.key(), relation(&top, &bottom, 1, 2))
            .unwrap();
        let pixels = vec![0; 20 * 20];
        let mut report = ConnectionBuildReport::default();

        find_connections(
            &mut graph,
            ConnectionRaster {
                width: 20,
                height: 20,
                pixels: &pixels,
            },
            &[stick(11, top.key()), stick(12, bottom.key())],
            parameters(),
            &mut report,
        )
        .unwrap();

        assert_eq!(report.promoted_count(), 1);
        let decision = &report.decisions()[0];
        assert_eq!(decision.alignment_edge, alignment_id);
        assert_eq!(
            (decision.top_filament_id, decision.bottom_filament_id),
            (11, 12)
        );
        assert_eq!(
            (decision.top_section_count, decision.bottom_section_count),
            (1, 1)
        );
        assert_eq!(decision.core.gap, 0);
        assert_eq!(decision.core.white_ratio, 0.0);
        let promoted = decision.promoted_edge.unwrap();
        assert_ne!(promoted, alignment_id);
        assert!(graph.edge(alignment_id).is_none());
        let connection = graph.edge(promoted).unwrap().relation();
        assert_eq!(connection.kind(), BarAlignmentKind::Connection);
        assert_eq!(
            connection.impacts(),
            BarImpacts::Connection {
                align: 1.0,
                width: 1.0,
                gap: 1.0,
                white: 1.0,
            }
        );
        assert_eq!(connection.grade(), 0.8);
    }

    #[test]
    fn enclosed_gap_rejects_and_keeps_original_alignment() {
        let top = peak(1, 1, 4, 5, 6);
        let bottom = peak(2, 12, 15, 5, 6);
        let mut graph = PeakGraph::new();
        assert!(graph.add_vertex(top.clone()));
        assert!(graph.add_vertex(bottom.clone()));
        let edge = graph
            .add_edge(top.key(), bottom.key(), relation(&top, &bottom, 1, 2))
            .unwrap();
        let mut pixels = vec![255; 20 * 20];
        for y in [4, 12] {
            pixels[(y * 20) + 5] = 0;
        }
        let mut report = ConnectionBuildReport::default();

        find_connections(
            &mut graph,
            ConnectionRaster {
                width: 20,
                height: 20,
                pixels: &pixels,
            },
            &[stick(1, top.key()), stick(2, bottom.key())],
            parameters(),
            &mut report,
        )
        .unwrap();

        assert_eq!(report.promoted_count(), 0);
        assert_eq!(report.decisions()[0].core.length, 9);
        assert_eq!(report.decisions()[0].core.gap, 7);
        assert!((report.decisions()[0].core.white_ratio - (7.0 / 9.0)).abs() < 1.0e-14);
        assert_eq!(report.decisions()[0].promoted_edge, None);
        assert_eq!(
            graph.edge(edge).unwrap().relation().kind(),
            BarAlignmentKind::Alignment
        );
    }

    #[test]
    fn exact_gap_and_white_thresholds_promote_even_at_zero_grade() {
        let top = peak(1, 1, 4, 5, 6);
        let bottom = peak(2, 7, 10, 5, 6);
        let mut graph = PeakGraph::new();
        assert!(graph.add_vertex(top.clone()));
        assert!(graph.add_vertex(bottom.clone()));
        graph
            .add_edge(top.key(), bottom.key(), relation(&top, &bottom, 1, 2))
            .unwrap();
        let mut pixels = vec![255; 20 * 20];
        for y in [4, 6, 7] {
            pixels[(y * 20) + 5] = 0;
        }
        let mut report = ConnectionBuildReport::default();

        find_connections(
            &mut graph,
            ConnectionRaster {
                width: 20,
                height: 20,
                pixels: &pixels,
            },
            &[stick(1, top.key()), stick(2, bottom.key())],
            ConnectionParameters {
                maximum_gap: 1,
                maximum_white_ratio: 0.25,
            },
            &mut report,
        )
        .unwrap();

        assert_eq!(report.decisions()[0].core.gap, 1);
        assert_eq!(report.decisions()[0].core.white_ratio, 0.25);
        assert_eq!(report.promoted_count(), 1);
        let connection = graph.edges()[0].relation();
        assert_eq!(connection.kind(), BarAlignmentKind::Connection);
        assert_eq!(connection.grade(), 0.0);
    }

    #[test]
    fn missing_later_stick_preserves_promoted_prefix_and_edge_order() {
        let top = peak(1, 1, 4, 5, 6);
        let middle = peak(2, 8, 11, 5, 6);
        let bottom = peak(3, 15, 18, 5, 6);
        let mut graph = PeakGraph::new();
        for peak in [top.clone(), middle.clone(), bottom.clone()] {
            assert!(graph.add_vertex(peak));
        }
        let first = graph
            .add_edge(top.key(), middle.key(), relation(&top, &middle, 1, 2))
            .unwrap();
        let second = graph
            .add_edge(middle.key(), bottom.key(), relation(&middle, &bottom, 2, 3))
            .unwrap();
        let pixels = vec![0; 20 * 20];
        let mut report = ConnectionBuildReport::default();

        assert_eq!(
            find_connections(
                &mut graph,
                ConnectionRaster {
                    width: 20,
                    height: 20,
                    pixels: &pixels,
                },
                &[stick(1, top.key()), stick(2, middle.key())],
                parameters(),
                &mut report,
            ),
            Err(ConnectionBuildError::MissingStick(bottom.key()))
        );
        assert_eq!(report.decisions().len(), 1);
        assert_eq!(report.decisions()[0].alignment_edge, first);
        let promoted = report.decisions()[0].promoted_edge.unwrap();
        assert_eq!(
            graph
                .edges()
                .iter()
                .map(|edge| (edge.id(), edge.relation().kind()))
                .collect::<Vec<_>>(),
            [
                (second, BarAlignmentKind::Alignment),
                (promoted, BarAlignmentKind::Connection),
            ]
        );
    }
}
