// SPDX-License-Identifier: AGPL-3.0-or-later

//! Raw-raster construction boundary for Java's per-staff `StaffProjector`s.
//!
//! This adapter consumes the prepared staff prefix and the live binary raster,
//! then runs the already ported neutral projection logic. Every retained peak
//! receives the sheet's exact deskew transform before it enters
//! [`BarsProjectorRegistry`]. Detached brace candidates receive the same
//! treatment. The adapter can also materialize the exact ordered peak-graph
//! vertices. A single retained staff has an unambiguous one-system grouping
//! and can therefore reach bar-column construction without any invented
//! alignment edges. Multi-staff grouping stops explicitly at the still-
//! missing Java alignment and connection-discovery collaborators.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use audiveris_image::{
    bar_alignment::BarAlignment,
    bar_alignments::{
        AlignmentBuildError, AlignmentBuildReport, AlignmentParameters, AlignmentStaff,
        find_all_alignments,
    },
    bar_column::{BarColumn, StaffId},
    bar_connections::{
        ConnectionBuildError, ConnectionBuildReport, ConnectionParameters, ConnectionRaster,
        find_connections,
    },
    bar_sticks::{BarStickBuildState, BarStickError, BarStickParameters, build_bar_sticks},
    bars_logic::{BarsLogicError, build_bar_columns_from_graph},
    filament::{FilamentError, FilamentGeometry},
    lines_coordinator::StaffCandidateKind,
    peak_graph::PeakGraph,
    prepared_lines::{PreparedStaff, PreparedStaffHandoff},
    projection::{
        BarlineHeightSpec, BarsProjectorRegistry, BraceSearchRequest, PeakCoreGeometry,
        ProjectionError, ProjectorRegistration, StaffProjectorProcessRequest,
        StaffProjectorProcessTuning, StaffProjectorScaleRatios, StaffProjectorScaleRequest,
        barline_height, process_staff_projection,
    },
    section::Section,
    staff_peak::{StaffPeak, StaffPeakError, StaffPeakKey},
};

use crate::grid_executor::HeadlessSkew;

/// Borrowed live sheet raster. Zero is foreground, as in Audiveris run tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawProjectorRaster<'a> {
    pub width: usize,
    pub height: usize,
    pub pixels: &'a [u8],
}

/// Scale-independent settings that differ by prepared staff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawStaffProjectorSettings {
    pub staff_id: usize,
    pub barline_height: BarlineHeightSpec,
    /// Optional source-resolved brace window. Brace discovery is deliberately
    /// not inferred by this construction adapter.
    pub brace_search: Option<BraceSearchRequest>,
}

/// Explicit sheet-scale and Java-constant inputs for projector construction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawProjectorParameters<'a> {
    pub large_interline: i32,
    pub foreground_thickness: i32,
    pub ratios: StaffProjectorScaleRatios,
    pub tuning: StaffProjectorProcessTuning,
    pub staffs: &'a [RawStaffProjectorSettings],
}

/// One detached brace peak, kept outside normal projector/graph registration.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedBracePeak {
    pub staff_id: StaffId,
    pub peak: StaffPeak,
}

/// Construction result for one prepared staff handoff (normally one sheet).
#[derive(Clone, Debug, PartialEq)]
pub struct RawProjectorPreparation {
    pub registry: BarsProjectorRegistry,
    /// One decision per prepared staff, in prepared-staff order.
    pub registrations: Vec<ProjectorRegistration>,
    /// Detached candidates in prepared-staff order, omitting absent braces.
    pub brace_peaks: Vec<PreparedBracePeak>,
}

/// Deepest honest downstream boundary available from raw projectors alone.
///
/// Java discovers cross-staff alignments and concrete connections before it
/// decides system membership. Those collaborators are not ported yet, so the
/// multi-staff case is represented as pending rather than as fabricated
/// singleton systems.
#[derive(Clone, Debug)]
pub enum RawSystemGroupingBoundary {
    /// With one retained staff, Java's fallback grouping is unambiguous and
    /// isolated graph vertices are valid one-peak bar chains.
    CompleteSingleStaff {
        system_id: usize,
        staff_id: StaffId,
        columns: Vec<BarColumn>,
    },
    /// The graph vertices are complete, but the edges and resulting system
    /// partition require Java's missing alignment/connection discovery.
    NeedsAlignmentAndConnectionDiscovery { staff_ids: Vec<StaffId> },
    /// Raw alignments exist, but Java cannot group systems until pixel-backed
    /// concrete connection promotion has run.
    NeedsConnectionDiscovery {
        staff_ids: Vec<StaffId>,
        alignment_count: usize,
    },
    /// Concrete connections have been promoted, but Java still performs
    /// merged-peak splitting and conflict purge before system inference.
    NeedsSplitAndAlignmentPurge {
        staff_ids: Vec<StaffId>,
        alignment_count: usize,
        connection_count: usize,
    },
}

/// Ordered peak graph plus the system-grouping boundary reached from it.
#[derive(Clone, Debug)]
pub struct RawProjectorGraphBridge {
    pub graph: PeakGraph<BarAlignment>,
    /// Retained projector staff IDs in Java projector order.
    pub retained_staff_ids: Vec<StaffId>,
    /// One-line staves discarded by `findBarPeaks`, in encounter order.
    pub discarded_one_line_staff_ids: Vec<StaffId>,
    /// Brace candidates remain detached from ordinary graph registration.
    pub brace_peaks: Vec<PreparedBracePeak>,
    pub grouping: RawSystemGroupingBoundary,
}

/// Raw projector graph after real section-backed sticks have been built and
/// the weak Java curvature pass has marked brace-like peaks.
#[derive(Clone, Debug)]
pub struct RawBarStickGraphBridge {
    pub projectors: RawProjectorGraphBridge,
    pub sticks: BarStickBuildState,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawAlignmentBridgeParameters {
    pub sticks: BarStickParameters,
    pub maximum_alignment_slope: f64,
    pub maximum_alignment_delta_width: i32,
    pub maximum_column_dx: i32,
}

/// Stick-backed graph after `findAllAlignments`, stopped immediately before
/// Java's raster-backed `findConnections` pass.
#[derive(Clone, Debug)]
pub struct RawAlignmentGraphBridge {
    pub bars: RawBarStickGraphBridge,
    pub alignments: AlignmentBuildReport,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawConnectionBridgeParameters {
    pub alignments: RawAlignmentBridgeParameters,
    pub maximum_connection_gap: i32,
    pub maximum_connection_white_ratio: f64,
}

/// Pixel-backed connection graph, stopped before split/purge semantics.
#[derive(Clone, Debug)]
pub struct RawConnectionGraphBridge {
    pub alignments: RawAlignmentGraphBridge,
    pub connections: ConnectionBuildReport,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RawProjectorAdapterError {
    DuplicateStaffSettings(usize),
    UnknownStaffSettings(usize),
    MissingStaffSettings(usize),
    MissingPreparedStaff(usize),
    EmptyStaffLines(usize),
    StaffLineCountOverflow(usize),
    StaffInterlineOverflow {
        staff_id: usize,
        interline: usize,
    },
    Filament {
        staff_id: usize,
        source: FilamentError,
    },
    Projection {
        staff_id: usize,
        source: ProjectionError,
    },
    Deskew {
        staff_id: usize,
        source: StaffPeakError,
    },
    MissingRegisteredPeak(StaffPeakKey),
    DuplicateRegisteredPeak(StaffPeakKey),
    BarSticks(BarStickError),
    Alignments(AlignmentBuildError),
    Connections(ConnectionBuildError),
    BarColumns(BarsLogicError),
}

impl fmt::Display for RawProjectorAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateStaffSettings(id) => {
                write!(formatter, "staff {id} has duplicate projector settings")
            }
            Self::UnknownStaffSettings(id) => {
                write!(formatter, "projector settings refer to unknown staff {id}")
            }
            Self::MissingStaffSettings(id) => {
                write!(formatter, "staff {id} has no projector settings")
            }
            Self::MissingPreparedStaff(id) => {
                write!(
                    formatter,
                    "retained projector staff {id} has no prepared staff"
                )
            }
            Self::EmptyStaffLines(id) => write!(formatter, "staff {id} has no prepared lines"),
            Self::StaffLineCountOverflow(id) => {
                write!(formatter, "staff {id} line count does not fit Java int")
            }
            Self::StaffInterlineOverflow {
                staff_id,
                interline,
            } => write!(
                formatter,
                "staff {staff_id} interline {interline} does not fit Java int"
            ),
            Self::Filament { staff_id, source } => {
                write!(
                    formatter,
                    "staff {staff_id} filament geometry failed: {source}"
                )
            }
            Self::Projection { staff_id, source } => {
                write!(formatter, "staff {staff_id} projection failed: {source}")
            }
            Self::Deskew { staff_id, source } => {
                write!(formatter, "staff {staff_id} peak deskew failed: {source}")
            }
            Self::MissingRegisteredPeak(key) => write!(
                formatter,
                "registered peak on staff {} at {}-{} is absent from retained projectors",
                key.staff_id().value(),
                key.start(),
                key.stop()
            ),
            Self::DuplicateRegisteredPeak(key) => write!(
                formatter,
                "registered peak on staff {} at {}-{} occurs twice in graph order",
                key.staff_id().value(),
                key.start(),
                key.stop()
            ),
            Self::BarSticks(source) => write!(formatter, "bar-stick construction failed: {source}"),
            Self::Alignments(source) => {
                write!(formatter, "bar-alignment discovery failed: {source}")
            }
            Self::Connections(source) => {
                write!(formatter, "bar-connection discovery failed: {source}")
            }
            Self::BarColumns(source) => {
                write!(formatter, "bar-column construction failed: {source}")
            }
        }
    }
}

impl Error for RawProjectorAdapterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Filament { source, .. } => Some(source),
            Self::Projection { source, .. } => Some(source),
            Self::Deskew { source, .. } => Some(source),
            Self::BarSticks(source) => Some(source),
            Self::Alignments(source) => Some(source),
            Self::Connections(source) => Some(source),
            Self::BarColumns(source) => Some(source),
            _ => None,
        }
    }
}

/// Materialize Java's ordered `PeakGraph` vertices and proceed only as far as
/// the available evidence permits.
///
/// No alignment edge is synthesized here. A multi-staff result deliberately
/// stops before system grouping because `findAllAlignments`, `findConnections`,
/// and their pixel-evidence checks are not ported yet.
pub fn bridge_raw_projectors_to_graph(
    preparation: &RawProjectorPreparation,
    maximum_column_dx: i32,
) -> Result<RawProjectorGraphBridge, RawProjectorAdapterError> {
    let mut graph = PeakGraph::new();
    for &key in preparation.registry.graph_vertex_order() {
        let peak = preparation
            .registry
            .projectors()
            .iter()
            .flat_map(|projector| &projector.result.peaks)
            .find(|peak| peak.key() == key)
            .cloned()
            .ok_or(RawProjectorAdapterError::MissingRegisteredPeak(key))?;
        if !graph.add_vertex(peak) {
            return Err(RawProjectorAdapterError::DuplicateRegisteredPeak(key));
        }
    }

    let retained_staff_ids = preparation
        .registry
        .projectors()
        .iter()
        .map(|projector| projector.staff_id)
        .collect::<Vec<_>>();
    let grouping = group_raw_projector_graph(&graph, &retained_staff_ids, maximum_column_dx)?;

    Ok(RawProjectorGraphBridge {
        graph,
        retained_staff_ids,
        discarded_one_line_staff_ids: preparation.registry.discarded_one_line_staves().to_vec(),
        brace_peaks: preparation.brace_peaks.clone(),
        grouping,
    })
}

/// Continue the raw graph through Java's section-backed `buildBarSticks` and
/// weak curvature pass, without creating any alignment or connection edge.
pub fn bridge_raw_projectors_through_bar_sticks(
    preparation: &RawProjectorPreparation,
    vertical_sections: &[Section],
    horizontal_sections: &[Section],
    stick_parameters: BarStickParameters,
    maximum_column_dx: i32,
) -> Result<RawBarStickGraphBridge, RawProjectorAdapterError> {
    let peak_order = preparation.registry.graph_vertex_order().to_vec();
    let mut projectors = bridge_raw_projectors_to_graph(preparation, maximum_column_dx)?;
    let mut sticks = BarStickBuildState::new(stick_parameters.first_filament_id)
        .map_err(RawProjectorAdapterError::BarSticks)?;
    build_bar_sticks(
        &mut projectors.graph,
        &peak_order,
        vertical_sections,
        horizontal_sections,
        stick_parameters,
        &mut sticks,
    )
    .map_err(RawProjectorAdapterError::BarSticks)?;

    projectors.grouping = group_raw_projector_graph(
        &projectors.graph,
        &projectors.retained_staff_ids,
        maximum_column_dx,
    )?;
    Ok(RawBarStickGraphBridge { projectors, sticks })
}

/// Continue through Java `findAllAlignments` and stop before any raster-backed
/// connection promotion or system inference.
pub fn bridge_raw_projectors_through_alignments(
    handoff: &PreparedStaffHandoff,
    preparation: &RawProjectorPreparation,
    vertical_sections: &[Section],
    horizontal_sections: &[Section],
    skew: &HeadlessSkew,
    parameters: RawAlignmentBridgeParameters,
) -> Result<RawAlignmentGraphBridge, RawProjectorAdapterError> {
    let mut bars = bridge_raw_projectors_through_bar_sticks(
        preparation,
        vertical_sections,
        horizontal_sections,
        parameters.sticks,
        parameters.maximum_column_dx,
    )?;
    let staffs = alignment_staffs(handoff, &bars.projectors)?;
    let mut alignments = AlignmentBuildReport::default();
    find_all_alignments(
        &mut bars.projectors.graph,
        &staffs,
        AlignmentParameters {
            sheet_slope: skew.slope,
            maximum_alignment_slope: parameters.maximum_alignment_slope,
            maximum_alignment_delta_width: parameters.maximum_alignment_delta_width,
        },
        &mut alignments,
    )
    .map_err(RawProjectorAdapterError::Alignments)?;

    if bars.projectors.retained_staff_ids.len() > 1 {
        bars.projectors.grouping = RawSystemGroupingBoundary::NeedsConnectionDiscovery {
            staff_ids: bars.projectors.retained_staff_ids.clone(),
            alignment_count: alignments.edge_ids().len(),
        };
    }
    Ok(RawAlignmentGraphBridge { bars, alignments })
}

/// Continue through Java `findConnections`, preserving stick/section
/// ownership evidence, and stop before merged-peak splitting or edge purge.
pub fn bridge_raw_projectors_through_connections(
    handoff: &PreparedStaffHandoff,
    preparation: &RawProjectorPreparation,
    raster: RawProjectorRaster<'_>,
    vertical_sections: &[Section],
    horizontal_sections: &[Section],
    skew: &HeadlessSkew,
    parameters: RawConnectionBridgeParameters,
) -> Result<RawConnectionGraphBridge, RawProjectorAdapterError> {
    let mut alignments = bridge_raw_projectors_through_alignments(
        handoff,
        preparation,
        vertical_sections,
        horizontal_sections,
        skew,
        parameters.alignments,
    )?;
    let mut connections = ConnectionBuildReport::default();
    find_connections(
        &mut alignments.bars.projectors.graph,
        ConnectionRaster {
            width: raster.width,
            height: raster.height,
            pixels: raster.pixels,
        },
        alignments.bars.sticks.sticks(),
        ConnectionParameters {
            maximum_gap: parameters.maximum_connection_gap,
            maximum_white_ratio: parameters.maximum_connection_white_ratio,
        },
        &mut connections,
    )
    .map_err(RawProjectorAdapterError::Connections)?;

    if alignments.bars.projectors.retained_staff_ids.len() > 1 {
        alignments.bars.projectors.grouping =
            RawSystemGroupingBoundary::NeedsSplitAndAlignmentPurge {
                staff_ids: alignments.bars.projectors.retained_staff_ids.clone(),
                alignment_count: alignments.alignments.edge_ids().len(),
                connection_count: connections.promoted_count(),
            };
    }
    Ok(RawConnectionGraphBridge {
        alignments,
        connections,
    })
}

fn alignment_staffs(
    handoff: &PreparedStaffHandoff,
    bridge: &RawProjectorGraphBridge,
) -> Result<Vec<AlignmentStaff>, RawProjectorAdapterError> {
    bridge
        .retained_staff_ids
        .iter()
        .map(|&staff_id| {
            let staff = handoff
                .staffs
                .iter()
                .find(|staff| staff.id == staff_id.value())
                .ok_or(RawProjectorAdapterError::MissingPreparedStaff(
                    staff_id.value(),
                ))?;
            let first = staff
                .lines
                .first()
                .ok_or(RawProjectorAdapterError::EmptyStaffLines(staff.id))?
                .filament
                .geometry()
                .map_err(|source| RawProjectorAdapterError::Filament {
                    staff_id: staff.id,
                    source,
                })?;
            let last = staff
                .lines
                .last()
                .expect("nonempty staff was checked above")
                .filament
                .geometry()
                .map_err(|source| RawProjectorAdapterError::Filament {
                    staff_id: staff.id,
                    source,
                })?;
            Ok(AlignmentStaff {
                staff_id,
                left: staff.left,
                right: staff.right,
                top: first.start().1,
                bottom: last.start().1,
                short: staff.short,
                peaks: bridge
                    .graph
                    .vertices()
                    .iter()
                    .filter(|peak| peak.staff_id() == staff_id)
                    .map(StaffPeak::key)
                    .collect(),
            })
        })
        .collect()
}

fn group_raw_projector_graph(
    graph: &PeakGraph<BarAlignment>,
    retained_staff_ids: &[StaffId],
    maximum_column_dx: i32,
) -> Result<RawSystemGroupingBoundary, RawProjectorAdapterError> {
    if let [staff_id] = retained_staff_ids {
        let columns = build_bar_columns_from_graph(graph, &[*staff_id], maximum_column_dx)
            .map_err(RawProjectorAdapterError::BarColumns)?;
        Ok(RawSystemGroupingBoundary::CompleteSingleStaff {
            system_id: 1,
            staff_id: *staff_id,
            columns,
        })
    } else {
        Ok(
            RawSystemGroupingBoundary::NeedsAlignmentAndConnectionDiscovery {
                staff_ids: retained_staff_ids.to_vec(),
            },
        )
    }
}

/// Build and register raw per-staff projectors with construction-time deskew.
pub fn prepare_raw_projectors(
    handoff: &PreparedStaffHandoff,
    raster: RawProjectorRaster<'_>,
    skew: &HeadlessSkew,
    parameters: RawProjectorParameters<'_>,
) -> Result<RawProjectorPreparation, RawProjectorAdapterError> {
    let staff_ids = handoff
        .staffs
        .iter()
        .map(|staff| staff.id)
        .collect::<BTreeSet<_>>();
    let mut settings = BTreeMap::new();
    for entry in parameters.staffs {
        if !staff_ids.contains(&entry.staff_id) {
            return Err(RawProjectorAdapterError::UnknownStaffSettings(
                entry.staff_id,
            ));
        }
        if settings.insert(entry.staff_id, *entry).is_some() {
            return Err(RawProjectorAdapterError::DuplicateStaffSettings(
                entry.staff_id,
            ));
        }
    }

    let mut registry = BarsProjectorRegistry::new();
    let mut registrations = Vec::with_capacity(handoff.staffs.len());
    let mut brace_peaks = Vec::new();

    for staff in &handoff.staffs {
        let setting = settings
            .get(&staff.id)
            .copied()
            .ok_or(RawProjectorAdapterError::MissingStaffSettings(staff.id))?;
        let prepared = prepare_staff_projector(staff, raster, skew, parameters, setting)?;
        if let Some(peak) = prepared.brace_peak {
            brace_peaks.push(PreparedBracePeak {
                staff_id: StaffId::new(staff.id),
                peak,
            });
        }
        registrations.push(registry.register(prepared.output, prepared.is_one_line));
    }

    Ok(RawProjectorPreparation {
        registry,
        registrations,
        brace_peaks,
    })
}

struct PreparedProjector {
    output: audiveris_image::projection::StaffProjectorProcessOutput,
    brace_peak: Option<StaffPeak>,
    is_one_line: bool,
}

fn prepare_staff_projector(
    staff: &PreparedStaff,
    raster: RawProjectorRaster<'_>,
    skew: &HeadlessSkew,
    parameters: RawProjectorParameters<'_>,
    setting: RawStaffProjectorSettings,
) -> Result<PreparedProjector, RawProjectorAdapterError> {
    if staff.lines.is_empty() {
        return Err(RawProjectorAdapterError::EmptyStaffLines(staff.id));
    }
    let line_count = i32::try_from(staff.lines.len())
        .map_err(|_| RawProjectorAdapterError::StaffLineCountOverflow(staff.id))?;
    let interline = i32::try_from(staff.interline).map_err(|_| {
        RawProjectorAdapterError::StaffInterlineOverflow {
            staff_id: staff.id,
            interline: staff.interline,
        }
    })?;
    let geometries = staff
        .lines
        .iter()
        .map(|line| line.filament.geometry())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| RawProjectorAdapterError::Filament {
            staff_id: staff.id,
            source,
        })?;
    let thicknesses = staff
        .lines
        .iter()
        .map(|line| line.filament.thickness())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| RawProjectorAdapterError::Filament {
            staff_id: staff.id,
            source,
        })?;
    let ordinates = precompute_ordinates(staff.id, raster.width, &geometries)?;
    let middle = geometries.len() / 2;
    let is_one_line = staff.kind == StaffCandidateKind::OneLine;
    let half_bar_height = barline_height(setting.barline_height, interline) / 2;
    let first = if is_one_line {
        ordinates[middle]
            .iter()
            .map(|y| y.wrapping_sub(half_bar_height))
            .collect::<Vec<_>>()
    } else {
        ordinates[0].clone()
    };
    let last = if is_one_line {
        ordinates[middle]
            .iter()
            .map(|y| y.wrapping_add(half_bar_height))
            .collect::<Vec<_>>()
    } else {
        ordinates[ordinates.len() - 1].clone()
    };
    let staff_id = StaffId::new(staff.id);
    let mut output = process_staff_projection(
        raster.width,
        raster.height,
        raster.pixels,
        StaffProjectorProcessRequest {
            staff_id,
            staff_left: staff.left.round_ties_even() as i32,
            staff_right: staff.right.round_ties_even() as i32,
            line_thicknesses: &thicknesses,
            staff_line_count: line_count,
            foreground_thickness: parameters.foreground_thickness,
            scale: StaffProjectorScaleRequest {
                large_interline: parameters.large_interline,
                staff_specific_interline: interline,
                is_one_line_staff: is_one_line,
                barline_height: setting.barline_height,
                ratios: parameters.ratios,
            },
            tuning: parameters.tuning,
        },
        |x| first[index_x(x, raster.width)],
        |x| last[index_x(x, raster.width)],
        |x| {
            let x = index_x(x, raster.width);
            PeakCoreGeometry::new(
                ordinates[0][x],
                ordinates[middle][x],
                ordinates[line_count as usize - 1][x],
            )
        },
    )
    .map_err(|source| RawProjectorAdapterError::Projection {
        staff_id: staff.id,
        source,
    })?;

    for peak in &mut output.result.peaks {
        peak.compute_deskewed_center(|point| skew.deskewed(point))
            .map_err(|source| RawProjectorAdapterError::Deskew {
                staff_id: staff.id,
                source,
            })?;
    }

    let brace_candidate = setting
        .brace_search
        .map(|request| {
            output
                .result
                .projection
                .find_brace_candidate(&output.result.all_blanks, request)
        })
        .transpose()
        .map_err(|source| RawProjectorAdapterError::Projection {
            staff_id: staff.id,
            source,
        })?
        .flatten();
    output.result.set_brace_candidate(brace_candidate);
    let brace_peak = brace_candidate
        .map(|candidate| {
            candidate.into_staff_peak(
                staff_id,
                |x| {
                    let x = index_x(x, raster.width);
                    (ordinates[0][x], ordinates[ordinates.len() - 1][x])
                },
                |point| skew.deskewed(point),
            )
        })
        .transpose()
        .map_err(|source| RawProjectorAdapterError::Projection {
            staff_id: staff.id,
            source,
        })?;

    Ok(PreparedProjector {
        output,
        brace_peak,
        is_one_line,
    })
}

fn precompute_ordinates(
    staff_id: usize,
    width: usize,
    geometries: &[FilamentGeometry],
) -> Result<Vec<Vec<i32>>, RawProjectorAdapterError> {
    geometries
        .iter()
        .map(|geometry| {
            (0..width)
                .map(|x| {
                    geometry
                        .position_at(x as f64)
                        .map(|y| y.round_ties_even() as i32)
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| RawProjectorAdapterError::Filament { staff_id, source })
}

fn index_x(x: i32, width: usize) -> usize {
    usize::try_from(x)
        .expect("projection only requests nonnegative in-domain abscissae")
        .min(width.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiveris_image::{
        bar_alignment::BarAlignmentKind,
        prepared_lines::PreparedStaffLine,
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections},
        staff_peak::{PeakPoint, StaffPeakAttribute},
    };

    fn one_system_fixture() -> (PreparedStaffHandoff, Vec<u8>) {
        let width = 20;
        let height = 6;
        let mut pixels = vec![255; width * height];
        let mut table = RunTable::new(Orientation::Horizontal, width, height).unwrap();
        for y in [0, 5] {
            table.add_run(y, Run::new(0, width)).unwrap();
            for x in 0..width {
                pixels[(y * width) + x] = 0;
            }
        }
        for x in 5..=6 {
            for y in 0..height {
                pixels[(y * width) + x] = 0;
            }
        }

        let lines = build_sections(&table, JunctionPolicy::All)
            .into_iter()
            .enumerate()
            .map(|(index, section)| {
                let mut filament = audiveris_image::filament::StaffFilament::new(5).unwrap();
                filament.add_section(section).unwrap();
                PreparedStaffLine {
                    id: index + 1,
                    filament,
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);

        (
            PreparedStaffHandoff {
                staffs: vec![PreparedStaff {
                    id: 1,
                    kind: StaffCandidateKind::Standard,
                    left: 5.0,
                    right: 6.0,
                    interline: 5,
                    small: false,
                    short: false,
                    lines,
                }],
            },
            pixels,
        )
    }

    fn two_staff_connection_fixture() -> (PreparedStaffHandoff, Vec<u8>) {
        let width = 20;
        let height = 12;
        let mut pixels = vec![255; width * height];
        let line_pair = |first_y: usize, last_y: usize| {
            let mut table = RunTable::new(Orientation::Horizontal, width, height).unwrap();
            for y in [first_y, last_y] {
                table.add_run(y, Run::new(0, width)).unwrap();
            }
            build_sections(&table, JunctionPolicy::All)
                .into_iter()
                .enumerate()
                .map(|(index, section)| {
                    let mut filament = audiveris_image::filament::StaffFilament::new(5).unwrap();
                    filament.add_section(section).unwrap();
                    PreparedStaffLine {
                        id: index + 1,
                        filament,
                    }
                })
                .collect::<Vec<_>>()
        };
        for y in [0, 5, 6, 11] {
            for x in 0..width {
                pixels[(y * width) + x] = 0;
            }
        }
        for x in 5..=6 {
            for y in 0..height {
                pixels[(y * width) + x] = 0;
            }
        }
        (
            PreparedStaffHandoff {
                staffs: vec![
                    PreparedStaff {
                        id: 1,
                        kind: StaffCandidateKind::Standard,
                        left: 5.0,
                        right: 6.0,
                        interline: 5,
                        small: false,
                        short: false,
                        lines: line_pair(0, 5),
                    },
                    PreparedStaff {
                        id: 2,
                        kind: StaffCandidateKind::Standard,
                        left: 5.0,
                        right: 6.0,
                        interline: 5,
                        small: false,
                        short: false,
                        lines: line_pair(6, 11),
                    },
                ],
            },
            pixels,
        )
    }

    #[test]
    fn synthetic_system_registers_deskewed_bar_and_detached_brace() {
        let (handoff, pixels) = one_system_fixture();
        let ratios = StaffProjectorScaleRatios {
            staff_abscissa_margin: 20.0,
            bar_refine_dx: 2.0,
            bar_threshold: 0.8,
            brace_threshold: 0.8,
            gap_threshold: 0.2,
            minimum_wide_blank_width: 2.0,
            maximum_bar_width: 4.0,
            chunk_width: 1.0,
            ..StaffProjectorScaleRatios::java_defaults()
        };
        let tuning = StaffProjectorProcessTuning {
            top_derivative_count: 2,
            minimum_derivative_ratio: 1.0,
            blank_threshold_ratio: 2.1,
            chunk_threshold_ratio: 0.4,
            minimum_white_ratio_beyond_serif: 0.3,
        };
        let settings = [RawStaffProjectorSettings {
            staff_id: 1,
            barline_height: BarlineHeightSpec::Four,
            brace_search: Some(BraceSearchRequest::new(5, 0, 7, 2, 4)),
        }];
        let skew = HeadlessSkew::new(0.25, 20, 6);

        let prepared = prepare_raw_projectors(
            &handoff,
            RawProjectorRaster {
                width: 20,
                height: 6,
                pixels: &pixels,
            },
            &skew,
            RawProjectorParameters {
                large_interline: 1,
                foreground_thickness: 2,
                ratios,
                tuning,
                staffs: &settings,
            },
        )
        .unwrap();

        assert_eq!(prepared.registrations.len(), 1);
        assert_eq!(prepared.registry.projectors()[0].result.peaks.len(), 1);
        assert!(
            matches!(
                &prepared.registrations[0],
                ProjectorRegistration::Retained {
                    projector_index: 0,
                    added_graph_vertices,
                } if added_graph_vertices.len() == 1
            ),
            "{:?}",
            prepared.registrations
        );
        assert_eq!(prepared.registry.graph_vertex_order().len(), 1);
        let peak = &prepared.registry.projectors()[0].result.peaks[0];
        assert_eq!(
            (peak.start(), peak.stop(), peak.top(), peak.bottom()),
            (5, 6, 0, 5)
        );
        assert_eq!(
            peak.deskewed_center(),
            Some(skew.deskewed(PeakPoint::new(5.5, 2.5)))
        );

        assert_eq!(prepared.brace_peaks.len(), 1);
        let brace = &prepared.brace_peaks[0];
        assert_eq!(brace.staff_id, StaffId::new(1));
        assert!(brace.peak.is_set(StaffPeakAttribute::Brace));
        assert_eq!((brace.peak.start(), brace.peak.stop()), (4, 7));
        assert_eq!(
            brace.peak.deskewed_center(),
            Some(skew.deskewed(PeakPoint::new(5.5, 2.5)))
        );
        assert_eq!(
            prepared.registry.projectors()[0].result.brace_candidate,
            Some(audiveris_image::projection::ProjectionBraceCandidate {
                raw_start: 5,
                raw_stop: 6,
                start: 4,
                stop: 7,
                search_right: 7,
            })
        );

        let expected_key = peak.key();
        let expected_center = peak.deskewed_center().unwrap();
        let mut bridge = bridge_raw_projectors_to_graph(&prepared, 2).unwrap();
        assert_eq!(
            bridge
                .graph
                .vertices()
                .iter()
                .map(StaffPeak::key)
                .collect::<Vec<_>>(),
            prepared.registry.graph_vertex_order()
        );
        assert_eq!(
            bridge.graph.vertex(expected_key).unwrap().deskewed_center(),
            Some(expected_center)
        );
        assert_eq!(bridge.retained_staff_ids, vec![StaffId::new(1)]);
        assert!(bridge.discarded_one_line_staff_ids.is_empty());
        assert_eq!(bridge.brace_peaks, prepared.brace_peaks);
        match &mut bridge.grouping {
            RawSystemGroupingBoundary::CompleteSingleStaff {
                system_id,
                staff_id,
                columns,
            } => {
                assert_eq!(*system_id, 1);
                assert_eq!(*staff_id, StaffId::new(1));
                assert_eq!(columns.len(), 1);
                let peak = columns[0].peaks()[0].unwrap();
                assert_eq!(peak.id().value(), 1);
                assert_eq!(peak.staff_id(), StaffId::new(1));
                assert_eq!(peak.deskewed_x(), expected_center.x);
                assert_eq!(columns[0].deskewed_x(), expected_center.x);
            }
            RawSystemGroupingBoundary::NeedsAlignmentAndConnectionDiscovery { .. } => {
                panic!("one retained staff does not require alignment discovery")
            }
            RawSystemGroupingBoundary::NeedsConnectionDiscovery { .. } => {
                panic!("one retained staff does not require connection discovery")
            }
            RawSystemGroupingBoundary::NeedsSplitAndAlignmentPurge { .. } => {
                panic!("one retained staff does not require split or alignment purge")
            }
        }

        let mut vertical_table = RunTable::new(Orientation::Vertical, 20, 6).unwrap();
        vertical_table.add_run(5, Run::new(0, 6)).unwrap();
        vertical_table.add_run(6, Run::new(0, 6)).unwrap();
        let vertical_sections = build_sections(&vertical_table, JunctionPolicy::All);
        let with_sticks = bridge_raw_projectors_through_bar_sticks(
            &prepared,
            &vertical_sections,
            &[],
            BarStickParameters {
                vertical_extension: 0,
                minimum_core_section_length: 3,
                probe_width: 1,
                minimum_probe_weight: 1,
                segment_length: 3,
                minimum_mean_curvature: 0.0,
                first_filament_id: 1,
            },
            2,
        )
        .unwrap();
        assert_eq!(with_sticks.sticks.sticks().len(), 1);
        assert_eq!(with_sticks.sticks.sticks()[0].peak, expected_key);
        assert_eq!(with_sticks.sticks.sticks()[0].id, 1);
        assert_eq!(with_sticks.sticks.sticks()[0].members.len(), 1);
        assert_eq!(
            with_sticks
                .projectors
                .graph
                .vertex(expected_key)
                .unwrap()
                .deskewed_center(),
            Some(expected_center)
        );
        assert!(matches!(
            with_sticks.projectors.grouping,
            RawSystemGroupingBoundary::CompleteSingleStaff { ref columns, .. }
                if columns.len() == 1
        ));
    }

    #[test]
    fn multiple_staffs_stop_before_missing_alignment_discovery() {
        let (mut handoff, pixels) = one_system_fixture();
        let mut second = handoff.staffs[0].clone();
        second.id = 2;
        handoff.staffs.push(second);
        let settings = [1, 2].map(|staff_id| RawStaffProjectorSettings {
            staff_id,
            barline_height: BarlineHeightSpec::Four,
            brace_search: None,
        });
        let prepared = prepare_raw_projectors(
            &handoff,
            RawProjectorRaster {
                width: 20,
                height: 6,
                pixels: &pixels,
            },
            &HeadlessSkew::new(0.0, 20, 6),
            RawProjectorParameters {
                large_interline: 1,
                foreground_thickness: 2,
                ratios: StaffProjectorScaleRatios {
                    staff_abscissa_margin: 20.0,
                    bar_refine_dx: 2.0,
                    bar_threshold: 0.8,
                    gap_threshold: 0.2,
                    minimum_wide_blank_width: 2.0,
                    maximum_bar_width: 4.0,
                    chunk_width: 1.0,
                    ..StaffProjectorScaleRatios::java_defaults()
                },
                tuning: StaffProjectorProcessTuning {
                    top_derivative_count: 2,
                    minimum_derivative_ratio: 1.0,
                    blank_threshold_ratio: 2.1,
                    chunk_threshold_ratio: 0.4,
                    minimum_white_ratio_beyond_serif: 0.3,
                },
                staffs: &settings,
            },
        )
        .unwrap();

        let bridge = bridge_raw_projectors_to_graph(&prepared, 2).unwrap();
        assert_eq!(bridge.graph.vertices().len(), 2);
        assert!(bridge.graph.edges().is_empty());
        assert!(matches!(
            bridge.grouping,
            RawSystemGroupingBoundary::NeedsAlignmentAndConnectionDiscovery { ref staff_ids }
                if staff_ids == &[StaffId::new(1), StaffId::new(2)]
        ));

        let mut vertical_table = RunTable::new(Orientation::Vertical, 20, 6).unwrap();
        vertical_table.add_run(5, Run::new(0, 6)).unwrap();
        vertical_table.add_run(6, Run::new(0, 6)).unwrap();
        let vertical_sections = build_sections(&vertical_table, JunctionPolicy::All);
        let aligned = bridge_raw_projectors_through_alignments(
            &handoff,
            &prepared,
            &vertical_sections,
            &[],
            &HeadlessSkew::new(0.0, 20, 6),
            RawAlignmentBridgeParameters {
                sticks: BarStickParameters {
                    vertical_extension: 0,
                    minimum_core_section_length: 3,
                    probe_width: 1,
                    minimum_probe_weight: 1,
                    segment_length: 3,
                    minimum_mean_curvature: 0.0,
                    first_filament_id: 1,
                },
                maximum_alignment_slope: 0.06,
                maximum_alignment_delta_width: 1,
                maximum_column_dx: 2,
            },
        )
        .unwrap();
        assert_eq!(aligned.alignments.edge_ids().len(), 1);
        assert_eq!(aligned.bars.projectors.graph.edges().len(), 1);
        let edge = &aligned.bars.projectors.graph.edges()[0];
        assert_eq!(edge.source().staff_id(), StaffId::new(1));
        assert_eq!(edge.target().staff_id(), StaffId::new(2));
        assert!(matches!(
            aligned.bars.projectors.grouping,
            RawSystemGroupingBoundary::NeedsConnectionDiscovery {
                ref staff_ids,
                alignment_count: 1,
            } if staff_ids == &[StaffId::new(1), StaffId::new(2)]
        ));
    }

    #[test]
    fn settings_are_exact_and_fail_closed() {
        let (handoff, pixels) = one_system_fixture();
        let skew = HeadlessSkew::new(0.0, 20, 6);
        let error = prepare_raw_projectors(
            &handoff,
            RawProjectorRaster {
                width: 20,
                height: 6,
                pixels: &pixels,
            },
            &skew,
            RawProjectorParameters {
                large_interline: 5,
                foreground_thickness: 1,
                ratios: StaffProjectorScaleRatios::java_defaults(),
                tuning: StaffProjectorProcessTuning::java_defaults(),
                staffs: &[],
            },
        )
        .unwrap_err();
        assert_eq!(error, RawProjectorAdapterError::MissingStaffSettings(1));
    }

    #[test]
    fn two_staff_raw_raster_promotes_connection_and_stops_before_split_purge() {
        let (handoff, pixels) = two_staff_connection_fixture();
        let settings = [1, 2].map(|staff_id| RawStaffProjectorSettings {
            staff_id,
            barline_height: BarlineHeightSpec::Four,
            brace_search: None,
        });
        let skew = HeadlessSkew::new(0.0, 20, 12);
        let raster = RawProjectorRaster {
            width: 20,
            height: 12,
            pixels: &pixels,
        };
        let prepared = prepare_raw_projectors(
            &handoff,
            raster,
            &skew,
            RawProjectorParameters {
                large_interline: 1,
                foreground_thickness: 2,
                ratios: StaffProjectorScaleRatios {
                    staff_abscissa_margin: 20.0,
                    bar_refine_dx: 2.0,
                    bar_threshold: 0.8,
                    gap_threshold: 0.2,
                    minimum_wide_blank_width: 2.0,
                    maximum_bar_width: 4.0,
                    chunk_width: 1.0,
                    ..StaffProjectorScaleRatios::java_defaults()
                },
                tuning: StaffProjectorProcessTuning {
                    top_derivative_count: 2,
                    minimum_derivative_ratio: 1.0,
                    blank_threshold_ratio: 2.1,
                    chunk_threshold_ratio: 0.4,
                    minimum_white_ratio_beyond_serif: 0.3,
                },
                staffs: &settings,
            },
        )
        .unwrap();
        let mut vertical_table = RunTable::new(Orientation::Vertical, 20, 12).unwrap();
        vertical_table.add_run(5, Run::new(0, 12)).unwrap();
        vertical_table.add_run(6, Run::new(0, 12)).unwrap();
        let vertical_sections = build_sections(&vertical_table, JunctionPolicy::All);

        let connected = bridge_raw_projectors_through_connections(
            &handoff,
            &prepared,
            raster,
            &vertical_sections,
            &[],
            &skew,
            RawConnectionBridgeParameters {
                alignments: RawAlignmentBridgeParameters {
                    sticks: BarStickParameters {
                        vertical_extension: 0,
                        minimum_core_section_length: 3,
                        probe_width: 1,
                        minimum_probe_weight: 1,
                        segment_length: 3,
                        minimum_mean_curvature: 0.0,
                        first_filament_id: 1,
                    },
                    maximum_alignment_slope: 0.06,
                    maximum_alignment_delta_width: 1,
                    maximum_column_dx: 2,
                },
                maximum_connection_gap: 1,
                maximum_connection_white_ratio: 0.25,
            },
        )
        .unwrap();

        assert_eq!(connected.connections.decisions().len(), 1);
        assert_eq!(connected.connections.promoted_count(), 1);
        let graph = &connected.alignments.bars.projectors.graph;
        assert_eq!(graph.edges().len(), 1);
        assert_eq!(
            graph.edges()[0].relation().kind(),
            BarAlignmentKind::Connection
        );
        assert!(matches!(
            connected.alignments.bars.projectors.grouping,
            RawSystemGroupingBoundary::NeedsSplitAndAlignmentPurge {
                ref staff_ids,
                alignment_count: 1,
                connection_count: 1,
            } if staff_ids == &[StaffId::new(1), StaffId::new(2)]
        ));
    }
}
