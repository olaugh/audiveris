// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact headless seam for Java `PeakGraph.getSystemTops`, `createSystems`,
//! `purgeCrossAlignments`, and the immediately following `buildColumns`.

use std::{error::Error, fmt};

use audiveris_image::{
    bar_alignment::{BarAlignment, BarAlignmentKind},
    bar_alignments::AlignmentStaff,
    bar_column::{BarColumn, StaffId},
    bars_logic::{BarsLogicError, aggregate_bar_chains, graph_bar_chains},
    lines_coordinator::StaffCandidateKind,
    peak_graph::{PeakGraph, PeakGraphEdge},
    staff_peak::StaffPeakKey,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SystemGroupingParameters {
    pub maximum_first_connection_x_offset: i32,
    pub maximum_column_dx: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SystemGroupingStaff {
    pub geometry: AlignmentStaff,
    pub kind: StaffCandidateKind,
}

#[derive(Clone, Debug)]
pub struct SystemGroup {
    pub id: usize,
    pub staff_ids: Vec<StaffId>,
    pub columns: Vec<BarColumn>,
}

#[derive(Clone, Debug, Default)]
pub struct SystemGroupingReport {
    /// One-based top staff ID for every retained staff, in manager order.
    pub system_tops: Vec<StaffId>,
    /// Created in staff traversal order. Before column construction succeeds,
    /// this remains the exact `createSystems` prefix with empty columns.
    pub systems: Vec<SystemGroup>,
    /// Removed in stable graph edge order after staff ownership is installed.
    pub purged_cross_alignments: Vec<PeakGraphEdge<BarAlignment>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SystemGroupingError {
    InvalidParameters,
    NoSystemFound,
    NonContiguousStaffId { expected: usize, actual: usize },
    WrongProjectorPeakStaff { peak: StaffPeakKey, staff: StaffId },
    UnknownGraphStaff(StaffId),
    Columns(BarsLogicError),
}

impl fmt::Display for SystemGroupingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParameters => formatter.write_str("invalid system-grouping parameters"),
            Self::NoSystemFound => formatter.write_str("no system found"),
            Self::NonContiguousStaffId { expected, actual } => write!(
                formatter,
                "staff manager order expected staff {expected}, found {actual}"
            ),
            Self::WrongProjectorPeakStaff { peak, staff } => write!(
                formatter,
                "projector peak {peak:?} does not belong to staff {}",
                staff.value()
            ),
            Self::UnknownGraphStaff(staff) => {
                write!(
                    formatter,
                    "graph peak belongs to unknown staff {}",
                    staff.value()
                )
            }
            Self::Columns(source) => {
                write!(formatter, "system column construction failed: {source}")
            }
        }
    }
}

impl Error for SystemGroupingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Columns(source) => Some(source),
            _ => None,
        }
    }
}

/// Create staff/system ownership, purge relations crossing that ownership,
/// then build each system's columns from the surviving graph components.
/// Mutations and report fields are deliberately non-transactional.
pub fn group_systems_and_build_columns(
    graph: &mut PeakGraph<BarAlignment>,
    staffs: &[SystemGroupingStaff],
    parameters: SystemGroupingParameters,
    report: &mut SystemGroupingReport,
) -> Result<(), SystemGroupingError> {
    validate(graph, staffs, parameters)?;
    if staffs.is_empty() {
        return Err(SystemGroupingError::NoSystemFound);
    }

    report.system_tops = get_system_tops(graph, staffs, parameters);
    report.systems = create_systems(&report.system_tops);
    if report.systems.is_empty() {
        return Err(SystemGroupingError::NoSystemFound);
    }

    let system_by_staff = report
        .systems
        .iter()
        .flat_map(|system| {
            system
                .staff_ids
                .iter()
                .map(move |&staff| (staff, system.id))
        })
        .collect::<Vec<_>>();
    let to_remove = graph
        .edges()
        .iter()
        .filter(|edge| {
            system_for(&system_by_staff, edge.source().staff_id())
                != system_for(&system_by_staff, edge.target().staff_id())
        })
        .map(|edge| edge.id())
        .collect::<Vec<_>>();
    report.purged_cross_alignments = to_remove
        .into_iter()
        .filter_map(|edge| graph.remove_edge(edge))
        .collect();

    let chains = graph_bar_chains(graph).map_err(SystemGroupingError::Columns)?;
    for system in &mut report.systems {
        let system_chains = chains
            .iter()
            .filter(|chain| {
                chain
                    .first()
                    .is_some_and(|peak| system.staff_ids.contains(&peak.staff_id()))
            })
            .cloned()
            .collect::<Vec<_>>();
        system.columns = aggregate_bar_chains(
            &system.staff_ids,
            &system_chains,
            parameters.maximum_column_dx,
        )
        .map_err(SystemGroupingError::Columns)?;
    }
    Ok(())
}

fn validate(
    graph: &PeakGraph<BarAlignment>,
    staffs: &[SystemGroupingStaff],
    parameters: SystemGroupingParameters,
) -> Result<(), SystemGroupingError> {
    if parameters.maximum_first_connection_x_offset < 0 || parameters.maximum_column_dx < 0 {
        return Err(SystemGroupingError::InvalidParameters);
    }
    for (index, staff) in staffs.iter().enumerate() {
        let expected = index + 1;
        if staff.geometry.staff_id.value() != expected {
            return Err(SystemGroupingError::NonContiguousStaffId {
                expected,
                actual: staff.geometry.staff_id.value(),
            });
        }
        for &peak in &staff.geometry.peaks {
            if peak.staff_id() != staff.geometry.staff_id {
                return Err(SystemGroupingError::WrongProjectorPeakStaff {
                    peak,
                    staff: staff.geometry.staff_id,
                });
            }
        }
    }
    for peak in graph.vertices() {
        if !staffs
            .iter()
            .any(|staff| staff.geometry.staff_id == peak.staff_id())
        {
            return Err(SystemGroupingError::UnknownGraphStaff(peak.staff_id()));
        }
    }
    Ok(())
}

fn get_system_tops(
    graph: &PeakGraph<BarAlignment>,
    staffs: &[SystemGroupingStaff],
    parameters: SystemGroupingParameters,
) -> Vec<StaffId> {
    let mut tops = vec![None; staffs.len()];
    let mut connections = graph
        .edges()
        .iter()
        .filter(|edge| edge.relation().kind() == BarAlignmentKind::Connection)
        .collect::<Vec<_>>();
    // Java `BarAlignment.compareTo`: top staff ID, then top peak start. The
    // stable sort retains graph iteration order for comparator ties.
    connections.sort_by_key(|edge| {
        let peak = graph
            .vertex(edge.source())
            .expect("validated graph edge source exists");
        (peak.staff_id().value(), peak.start())
    });

    for connection in connections {
        let top_peak = graph
            .vertex(connection.source())
            .expect("validated graph edge source exists");
        let bottom_peak = graph
            .vertex(connection.target())
            .expect("validated graph edge target exists");
        let top = top_peak.staff_id().value();
        let bottom = bottom_peak.staff_id().value();
        if tops[top - 1].is_none() {
            tops[top - 1] = Some(StaffId::new(top));
        }
        if tops[bottom - 1].is_none() {
            let staff = &staffs[bottom - 1];
            if staff.kind != StaffCandidateKind::OneLine {
                let left = java_rint_to_i32(staff.geometry.left);
                let offset = bottom_peak.start().wrapping_sub(left);
                if offset > parameters.maximum_first_connection_x_offset {
                    let last = staff.geometry.peaks.last().copied();
                    if last == Some(bottom_peak.key())
                        || !are_right_connected(graph, staffs, top, bottom)
                    {
                        continue;
                    }
                }
            }
        }
        tops[bottom - 1] = tops[top - 1];
    }
    tops.into_iter()
        .enumerate()
        .map(|(index, top)| top.unwrap_or_else(|| StaffId::new(index + 1)))
        .collect()
}

fn are_right_connected(
    graph: &PeakGraph<BarAlignment>,
    staffs: &[SystemGroupingStaff],
    top: usize,
    bottom: usize,
) -> bool {
    let Some(&top_peak) = staffs[top - 1].geometry.peaks.last() else {
        return false;
    };
    let Some(&bottom_peak) = staffs[bottom - 1].geometry.peaks.last() else {
        return false;
    };
    graph
        .edge_between(top_peak, bottom_peak)
        .is_some_and(|edge| edge.relation().kind() == BarAlignmentKind::Connection)
}

fn create_systems(tops: &[StaffId]) -> Vec<SystemGroup> {
    let mut systems = Vec::<SystemGroup>::new();
    let mut current_top = None;
    for (index, &top) in tops.iter().enumerate() {
        if current_top.is_none_or(|current: StaffId| current.value() < top.value()) {
            current_top = Some(top);
            systems.push(SystemGroup {
                id: systems.len() + 1,
                staff_ids: vec![StaffId::new(index + 1)],
                columns: Vec::new(),
            });
        } else {
            systems
                .last_mut()
                .expect("first staff always starts a system")
                .staff_ids
                .push(StaffId::new(index + 1));
        }
    }
    systems
}

fn system_for(system_by_staff: &[(StaffId, usize)], staff: StaffId) -> Option<usize> {
    system_by_staff
        .iter()
        .find_map(|&(candidate, system)| (candidate == staff).then_some(system))
}

fn java_rint_to_i32(value: f64) -> i32 {
    value.round_ties_even() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiveris_image::{
        bar_alignment::{AlignmentPeak, BarImpacts},
        bar_column::PeakId,
        staff_peak::{StaffPeak, StaffVerticalImpacts},
    };

    fn peak(staff: usize, start: i32) -> StaffPeak {
        let mut peak = StaffPeak::with_impacts(
            StaffId::new(staff),
            (staff as i32 - 1) * 10,
            (staff as i32 - 1) * 10 + 4,
            start,
            start + 1,
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
        connection: bool,
    ) -> BarAlignment {
        let item = |peak: &StaffPeak, id| {
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
        let alignment = BarAlignment::new(
            item(top, top_id),
            item(bottom, bottom_id),
            0.0,
            0.0,
            BarImpacts::alignment(1.0, 1.0).unwrap(),
        )
        .unwrap();
        if connection {
            BarAlignment::connection(&alignment, 1.0, 1.0).unwrap()
        } else {
            alignment
        }
    }

    fn staff(id: usize, peaks: &[StaffPeakKey]) -> SystemGroupingStaff {
        SystemGroupingStaff {
            geometry: AlignmentStaff {
                staff_id: StaffId::new(id),
                left: 0.0,
                right: 120.0,
                top: (id as f64 - 1.0) * 10.0,
                bottom: (id as f64 - 1.0) * 10.0 + 4.0,
                short: false,
                peaks: peaks.to_vec(),
            },
            kind: StaffCandidateKind::Standard,
        }
    }

    fn peak_without_deskew(staff: usize, start: i32) -> StaffPeak {
        StaffPeak::with_impacts(
            StaffId::new(staff),
            (staff as i32 - 1) * 10,
            (staff as i32 - 1) * 10 + 4,
            start,
            start + 1,
            StaffVerticalImpacts::new(1.0, 1.0, 1.0, 1.0, 1.0, 1.0),
        )
        .unwrap()
    }

    #[test]
    fn side_by_side_connections_form_one_contiguous_system_and_columns() {
        let peaks = [peak(1, 5), peak(2, 5), peak(3, 5), peak(4, 5)];
        let mut graph = PeakGraph::new();
        for peak in &peaks {
            graph.add_vertex(peak.clone());
        }
        graph
            .add_edge(
                peaks[0].key(),
                peaks[1].key(),
                relation(&peaks[0], &peaks[1], 1, 2, true),
            )
            .unwrap();
        graph
            .add_edge(
                peaks[1].key(),
                peaks[2].key(),
                relation(&peaks[1], &peaks[2], 2, 3, true),
            )
            .unwrap();
        graph
            .add_edge(
                peaks[1].key(),
                peaks[3].key(),
                relation(&peaks[1], &peaks[3], 2, 4, true),
            )
            .unwrap();
        let staffs = [
            staff(1, &[peaks[0].key()]),
            staff(2, &[peaks[1].key()]),
            staff(3, &[peaks[2].key()]),
            staff(4, &[peaks[3].key()]),
        ];
        let mut report = SystemGroupingReport::default();
        group_systems_and_build_columns(
            &mut graph,
            &staffs,
            SystemGroupingParameters {
                maximum_first_connection_x_offset: 20,
                maximum_column_dx: 2,
            },
            &mut report,
        )
        .unwrap();
        assert_eq!(
            report
                .system_tops
                .iter()
                .map(|id| id.value())
                .collect::<Vec<_>>(),
            vec![1, 1, 1, 1]
        );
        assert_eq!(report.systems.len(), 1);
        assert_eq!(report.systems[0].staff_ids.len(), 4);
        assert_eq!(report.systems[0].columns.len(), 1);
        assert!(report.purged_cross_alignments.is_empty());
    }

    #[test]
    fn far_first_connection_needs_distinct_right_edge_second_chance() {
        let top_left = peak(1, 80);
        let top_right = peak(1, 100);
        let bottom_left = peak(2, 80);
        let bottom_right = peak(2, 100);
        let mut graph = PeakGraph::new();
        for peak in [&top_left, &top_right, &bottom_left, &bottom_right] {
            graph.add_vertex(peak.clone());
        }
        graph
            .add_edge(
                top_left.key(),
                bottom_left.key(),
                relation(&top_left, &bottom_left, 1, 3, true),
            )
            .unwrap();
        graph
            .add_edge(
                top_right.key(),
                bottom_right.key(),
                relation(&top_right, &bottom_right, 2, 4, true),
            )
            .unwrap();
        let staffs = [
            staff(1, &[top_left.key(), top_right.key()]),
            staff(2, &[bottom_left.key(), bottom_right.key()]),
        ];
        let mut accepted = SystemGroupingReport::default();
        group_systems_and_build_columns(
            &mut graph,
            &staffs,
            SystemGroupingParameters {
                maximum_first_connection_x_offset: 10,
                maximum_column_dx: 2,
            },
            &mut accepted,
        )
        .unwrap();
        assert_eq!(
            accepted
                .system_tops
                .iter()
                .map(|id| id.value())
                .collect::<Vec<_>>(),
            vec![1, 1]
        );

        let mut graph = PeakGraph::new();
        for peak in [&top_left, &top_right, &bottom_left, &bottom_right] {
            graph.add_vertex(peak.clone());
        }
        graph
            .add_edge(
                top_left.key(),
                bottom_left.key(),
                relation(&top_left, &bottom_left, 1, 3, true),
            )
            .unwrap();
        let mut rejected = SystemGroupingReport::default();
        group_systems_and_build_columns(
            &mut graph,
            &staffs,
            SystemGroupingParameters {
                maximum_first_connection_x_offset: 10,
                maximum_column_dx: 2,
            },
            &mut rejected,
        )
        .unwrap();
        assert_eq!(
            rejected
                .system_tops
                .iter()
                .map(|id| id.value())
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(rejected.systems.len(), 2);
        assert_eq!(rejected.purged_cross_alignments.len(), 1);
        assert!(graph.edges().is_empty());
    }

    #[test]
    fn column_failure_retains_created_systems_and_cross_edge_purge_prefix() {
        let top = peak_without_deskew(1, 5);
        let bottom = peak_without_deskew(2, 5);
        let mut graph = PeakGraph::new();
        graph.add_vertex(top.clone());
        graph.add_vertex(bottom.clone());
        graph
            .add_edge(
                top.key(),
                bottom.key(),
                relation(&top, &bottom, 1, 2, false),
            )
            .unwrap();
        let staffs = [staff(1, &[top.key()]), staff(2, &[bottom.key()])];
        let mut report = SystemGroupingReport::default();
        assert!(matches!(
            group_systems_and_build_columns(
                &mut graph,
                &staffs,
                SystemGroupingParameters {
                    maximum_first_connection_x_offset: 20,
                    maximum_column_dx: 2,
                },
                &mut report,
            ),
            Err(SystemGroupingError::Columns(
                BarsLogicError::MissingDeskewedCenter(_)
            ))
        ));
        assert_eq!(report.systems.len(), 2);
        assert_eq!(report.purged_cross_alignments.len(), 1);
        assert!(graph.edges().is_empty());
    }

    #[test]
    fn noncontiguous_post_discard_staff_ids_fail_before_graph_mutation() {
        let first = peak(1, 5);
        let third = peak(3, 5);
        let mut graph = PeakGraph::new();
        graph.add_vertex(first.clone());
        graph.add_vertex(third.clone());
        let edge = graph
            .add_edge(
                first.key(),
                third.key(),
                relation(&first, &third, 1, 2, false),
            )
            .unwrap();
        let staffs = [staff(1, &[first.key()]), staff(3, &[third.key()])];
        let mut report = SystemGroupingReport::default();
        assert_eq!(
            group_systems_and_build_columns(
                &mut graph,
                &staffs,
                SystemGroupingParameters {
                    maximum_first_connection_x_offset: 20,
                    maximum_column_dx: 2,
                },
                &mut report,
            ),
            Err(SystemGroupingError::NonContiguousStaffId {
                expected: 2,
                actual: 3,
            })
        );
        assert!(report.systems.is_empty());
        assert_eq!(graph.edges()[0].id(), edge);
    }
}
