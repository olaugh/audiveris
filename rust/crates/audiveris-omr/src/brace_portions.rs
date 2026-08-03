// SPDX-License-Identifier: AGPL-3.0-or-later

//! Narrow evidence seam for Java `BarsRetriever.detectBracePortions`.
//!
//! Projection lookup and filament construction stay injected: the caller may
//! use the production projection/section lags without introducing glyph/SIG
//! ownership here. This module owns lookup order, windows, gates, fallback,
//! and top/middle/bottom classification.

use std::{error::Error, fmt};

use audiveris_image::{
    bar_alignment::{BarAlignment, VerticalSide},
    bar_column::{BarColumn, BarColumnError, BarPeak, PeakId, StaffId},
    peak_graph::{PeakGraph, PeakGraphError},
    staff_peak::{StaffPeak, StaffPeakAttribute, StaffPeakKey},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BracePortionParameters {
    pub neutral_gap: i32,
    pub maximum_peak_width: i32,
    pub maximum_bar_gap: i32,
    pub minimum_portion_height: f64,
    pub maximum_curvature: f64,
    pub lookup_extension: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BraceFilamentEvidence {
    pub vertical_length: f64,
    pub mean_curvature: f64,
    pub extension_top: f64,
    pub extension_bottom: f64,
}

#[derive(Clone, Debug)]
pub struct BraceProbe {
    pub peak: StaffPeak,
    /// `None` is Java `buildFilament == null`.
    pub filament: Option<BraceFilamentEvidence>,
}

#[derive(Clone, Debug)]
pub struct BracePortionStaff {
    pub staff_id: StaffId,
    pub peaks: Vec<StaffPeakKey>,
    pub start_peak_index: Option<usize>,
    pub brace_peak: Option<StaffPeak>,
}

#[derive(Clone, Debug)]
pub struct BracePortionSystem {
    pub system_id: usize,
    pub staffs: Vec<BracePortionStaff>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BraceReplacement {
    pub system_id: usize,
    pub staff_id: StaffId,
    pub old_peak: StaffPeakKey,
    pub new_peak: StaffPeakKey,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BracePortionReport {
    /// Successful mistaken-first-bar replacements in traversal order.
    pub replacements: Vec<BraceReplacement>,
}

#[derive(Clone, Debug)]
pub struct BraceColumnSet {
    pub system_id: usize,
    pub columns: Vec<BarColumn>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BracePortionError {
    InvalidParameters,
    InvalidSystem(usize),
    InvalidStaff(StaffId),
    MissingStartPeak { staff: StaffId, index: usize },
    Probe(String),
    MissingReplacementPeak(StaffPeakKey),
    MissingProjectorPeak(StaffPeakKey),
    MissingPeakIdentity(StaffPeakKey),
    PeakIdentityExhausted,
    Column(BarColumnError),
    Graph(PeakGraphError),
}

impl fmt::Display for BracePortionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParameters => formatter.write_str("invalid brace-portion parameters"),
            Self::InvalidSystem(id) => write!(formatter, "invalid brace system {id}"),
            Self::InvalidStaff(id) => write!(formatter, "invalid brace staff {}", id.value()),
            Self::MissingStartPeak { staff, index } => write!(
                formatter,
                "staff {} start peak index {index} is absent",
                staff.value()
            ),
            Self::Probe(message) => write!(formatter, "brace probe failed: {message}"),
            Self::MissingReplacementPeak(peak) => {
                write!(formatter, "brace replacement peak is absent: {peak:?}")
            }
            Self::MissingProjectorPeak(peak) => {
                write!(formatter, "brace projector peak is absent: {peak:?}")
            }
            Self::MissingPeakIdentity(peak) => {
                write!(formatter, "brace peak has no stable identity: {peak:?}")
            }
            Self::PeakIdentityExhausted => formatter.write_str("brace peak identity exhausted"),
            Self::Column(source) => write!(formatter, "brace column replacement failed: {source}"),
            Self::Graph(source) => write!(formatter, "brace graph replacement failed: {source}"),
        }
    }
}

impl Error for BracePortionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Column(source) => Some(source),
            Self::Graph(source) => Some(source),
            _ => None,
        }
    }
}

/// Apply Java `replacePeak` for each mistaken-first-bar intent. Column
/// replacement happens first, followed by projector insertion, graph vertex
/// insertion, incoming copies, outgoing copies, and finally old removal.
/// Errors retain the exact successful mutation prefix.
pub fn apply_brace_replacements(
    graph: &mut PeakGraph<BarAlignment>,
    systems: &mut [BracePortionSystem],
    columns: &mut [BraceColumnSet],
    replacements: &[BraceReplacement],
) -> Result<(), BracePortionError> {
    let mut peak_ids = graph
        .vertices()
        .iter()
        .enumerate()
        .map(|(index, peak)| (PeakId::new(index + 1), peak.key()))
        .collect::<Vec<_>>();
    apply_brace_replacements_with_peak_ids(graph, &mut peak_ids, systems, columns, replacements)
}

/// Stable-ID form used after earlier projector purges have made graph-vector
/// positions diverge from Java entity identities.
pub fn apply_brace_replacements_with_peak_ids(
    graph: &mut PeakGraph<BarAlignment>,
    peak_ids: &mut Vec<(PeakId, StaffPeakKey)>,
    systems: &mut [BracePortionSystem],
    columns: &mut [BraceColumnSet],
    replacements: &[BraceReplacement],
) -> Result<(), BracePortionError> {
    for replacement in replacements {
        let system = systems
            .iter_mut()
            .find(|system| system.system_id == replacement.system_id)
            .ok_or(BracePortionError::InvalidSystem(replacement.system_id))?;
        let staff = system
            .staffs
            .iter_mut()
            .find(|staff| staff.staff_id == replacement.staff_id)
            .ok_or(BracePortionError::InvalidStaff(replacement.staff_id))?;
        let new_peak = staff
            .brace_peak
            .as_ref()
            .filter(|peak| peak.key() == replacement.new_peak)
            .cloned()
            .ok_or(BracePortionError::MissingReplacementPeak(
                replacement.new_peak,
            ))?;
        if graph.vertex(replacement.old_peak).is_none() {
            return Err(BracePortionError::MissingProjectorPeak(
                replacement.old_peak,
            ));
        }
        let old_id = peak_ids
            .iter()
            .find_map(|(id, key)| (*key == replacement.old_peak).then_some(*id))
            .ok_or(BracePortionError::MissingPeakIdentity(replacement.old_peak))?;
        let existing_new_id = peak_ids
            .iter()
            .find_map(|(id, key)| (*key == replacement.new_peak).then_some(*id));
        if graph.vertex(replacement.new_peak).is_some() && existing_new_id.is_none() {
            return Err(BracePortionError::MissingPeakIdentity(replacement.new_peak));
        }
        let new_id = existing_new_id.map_or_else(
            || {
                peak_ids
                    .iter()
                    .map(|(id, _)| id.value())
                    .max()
                    .unwrap_or(0)
                    .checked_add(1)
                    .map(PeakId::new)
                    .ok_or(BracePortionError::PeakIdentityExhausted)
            },
            Ok,
        )?;

        if let Some(column_set) = columns
            .iter_mut()
            .find(|set| set.system_id == replacement.system_id)
            && let Some(column) = column_set.columns.iter_mut().find(|column| {
                column
                    .peaks()
                    .iter()
                    .flatten()
                    .any(|peak| peak.id() == old_id)
            })
        {
            let deskewed =
                new_peak
                    .deskewed_center()
                    .ok_or(BracePortionError::MissingReplacementPeak(
                        replacement.new_peak,
                    ))?;
            let scalar = BarPeak::new(
                new_id,
                new_peak.staff_id(),
                f64::from(new_peak.width()),
                deskewed.x,
                new_peak.is_brace(),
                new_peak.is_staff_end(audiveris_image::staff_peak::HorizontalSide::Left),
            )
            .map_err(BracePortionError::Column)?;
            column.add_peak(scalar).map_err(BracePortionError::Column)?;
        }

        let projector_index = staff
            .peaks
            .iter()
            .position(|&peak| peak == replacement.old_peak)
            .ok_or(BracePortionError::MissingProjectorPeak(
                replacement.old_peak,
            ))?;
        staff.peaks.insert(projector_index, replacement.new_peak);
        let added = graph.add_vertex(new_peak);
        if added && existing_new_id.is_none() {
            peak_ids.push((new_id, replacement.new_peak));
        }

        let incoming = graph
            .incoming_edges(replacement.old_peak)
            .map_err(BracePortionError::Graph)?
            .map(|edge| (edge.source(), *edge.relation()))
            .collect::<Vec<_>>();
        for (source, relation) in incoming {
            graph
                .add_edge(source, replacement.new_peak, relation)
                .map_err(BracePortionError::Graph)?;
        }
        let outgoing = graph
            .outgoing_edges(replacement.old_peak)
            .map_err(BracePortionError::Graph)?
            .map(|edge| (edge.target(), *edge.relation()))
            .collect::<Vec<_>>();
        for (target, relation) in outgoing {
            graph
                .add_edge(replacement.new_peak, target, relation)
                .map_err(BracePortionError::Graph)?;
        }
        let old_index = staff
            .peaks
            .iter()
            .position(|&peak| peak == replacement.old_peak)
            .expect("old projector peak remains until final removal");
        staff.peaks.remove(old_index);
        graph.remove_vertex(replacement.old_peak);
    }
    Ok(())
}

/// Detect brace portions in system then staff order. A rejected first probe
/// still triggers Java's fallback window when the start index is at least one.
pub fn detect_brace_portions(
    systems: &mut [BracePortionSystem],
    parameters: BracePortionParameters,
    mut probe: impl FnMut(StaffId, i32, i32) -> Result<Option<BraceProbe>, String>,
    report: &mut BracePortionReport,
) -> Result<(), BracePortionError> {
    validate(systems, parameters)?;
    for system in systems {
        if system.staffs.len() <= 1 {
            continue;
        }
        let last_staff_index = system.staffs.len() - 1;
        for (staff_index, staff) in system.staffs.iter_mut().enumerate() {
            let Some(start_index) = staff.start_peak_index else {
                continue;
            };
            let first = staff.peaks[0];
            let (minimum, maximum) = lookup_window(first.start(), parameters);
            let mut brace = gated_probe(staff.staff_id, minimum, maximum, parameters, &mut probe)?;
            let mut replacement = false;
            if brace.is_none() && start_index >= 1 {
                let second = staff.peaks[1];
                let (minimum, maximum) = lookup_window(second.start(), parameters);
                brace = gated_probe(staff.staff_id, minimum, maximum, parameters, &mut probe)?;
                replacement = brace.is_some();
            }
            let Some(mut brace) = brace else { continue };
            classify(
                &mut brace.peak,
                brace.filament.expect("gated probe has filament"),
                staff_index,
                last_staff_index,
                parameters.lookup_extension,
            );
            if replacement {
                report.replacements.push(BraceReplacement {
                    system_id: system.system_id,
                    staff_id: staff.staff_id,
                    old_peak: first,
                    new_peak: brace.peak.key(),
                });
            }
            staff.brace_peak = Some(brace.peak);
        }
    }
    Ok(())
}

fn validate(
    systems: &[BracePortionSystem],
    parameters: BracePortionParameters,
) -> Result<(), BracePortionError> {
    if parameters.neutral_gap < 0
        || parameters.maximum_peak_width <= 0
        || parameters.maximum_bar_gap < 0
        || !parameters.minimum_portion_height.is_finite()
        || parameters.minimum_portion_height < 0.0
        || !parameters.maximum_curvature.is_finite()
        || parameters.maximum_curvature <= 0.0
        || !parameters.lookup_extension.is_finite()
        || parameters.lookup_extension < 0.0
    {
        return Err(BracePortionError::InvalidParameters);
    }
    for system in systems {
        if system.system_id == 0 || system.staffs.is_empty() {
            return Err(BracePortionError::InvalidSystem(system.system_id));
        }
        for staff in &system.staffs {
            if staff.staff_id.value() == 0
                || staff
                    .peaks
                    .iter()
                    .any(|peak| peak.staff_id() != staff.staff_id)
            {
                return Err(BracePortionError::InvalidStaff(staff.staff_id));
            }
            if let Some(index) = staff.start_peak_index
                && (index >= staff.peaks.len() || staff.peaks.is_empty())
            {
                return Err(BracePortionError::MissingStartPeak {
                    staff: staff.staff_id,
                    index,
                });
            }
        }
    }
    Ok(())
}

fn lookup_window(start: i32, parameters: BracePortionParameters) -> (i32, i32) {
    let maximum = start.wrapping_sub(1).wrapping_sub(parameters.neutral_gap);
    let minimum = 0.max(
        maximum.wrapping_sub(
            parameters
                .maximum_peak_width
                .wrapping_add(parameters.maximum_bar_gap),
        ),
    );
    (minimum, maximum)
}

fn gated_probe(
    staff: StaffId,
    minimum: i32,
    maximum: i32,
    parameters: BracePortionParameters,
    probe: &mut impl FnMut(StaffId, i32, i32) -> Result<Option<BraceProbe>, String>,
) -> Result<Option<BraceProbe>, BracePortionError> {
    let Some(candidate) = probe(staff, minimum, maximum).map_err(BracePortionError::Probe)? else {
        return Ok(None);
    };
    if candidate.peak.width() > parameters.maximum_peak_width {
        return Ok(None);
    }
    let Some(filament) = candidate.filament else {
        return Ok(None);
    };
    if filament.vertical_length < parameters.minimum_portion_height
        || filament.mean_curvature >= parameters.maximum_curvature
    {
        return Ok(None);
    }
    Ok(Some(candidate))
}

fn classify(
    peak: &mut StaffPeak,
    filament: BraceFilamentEvidence,
    staff_index: usize,
    last_staff_index: usize,
    lookup_extension: f64,
) {
    let beyond_top = filament.extension_top > lookup_extension && staff_index != 0;
    let beyond_bottom =
        filament.extension_bottom > lookup_extension && staff_index != last_staff_index;
    if beyond_top && beyond_bottom {
        peak.set(StaffPeakAttribute::BraceMiddle);
    } else if beyond_bottom {
        peak.set(StaffPeakAttribute::BraceTop);
    } else if beyond_top {
        peak.set(StaffPeakAttribute::BraceBottom);
    }
    debug_assert!(peak.is_set(StaffPeakAttribute::Brace));
    debug_assert_eq!(peak.ordinate(VerticalSide::Top), peak.top());
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiveris_image::{
        bar_alignment::{AlignmentPeak, BarImpacts},
        staff_peak::StaffVerticalImpacts,
    };

    fn peak(staff: usize, start: i32, stop: i32) -> StaffPeak {
        let mut peak = StaffPeak::new(StaffId::new(staff), 10, 20, start, stop).unwrap();
        peak.set(StaffPeakAttribute::Brace);
        peak.compute_deskewed_center(|point| point).unwrap();
        peak
    }

    fn graded_peak(staff: usize, start: i32, stop: i32) -> StaffPeak {
        let mut peak = StaffPeak::with_impacts(
            StaffId::new(staff),
            staff as i32 * 10,
            staff as i32 * 10 + 4,
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
        let value = |peak: &StaffPeak, id| {
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
            value(top, top_id),
            value(bottom, bottom_id),
            0.0,
            0.0,
            BarImpacts::alignment(1.0, 1.0).unwrap(),
        )
        .unwrap()
    }

    fn parameters() -> BracePortionParameters {
        BracePortionParameters {
            neutral_gap: 2,
            maximum_peak_width: 6,
            maximum_bar_gap: 8,
            minimum_portion_height: 9.0,
            maximum_curvature: 20.0,
            lookup_extension: 2.0,
        }
    }

    #[test]
    fn rejected_first_window_falls_back_and_records_replacement_in_order() {
        let first = peak(1, 20, 21);
        let second = peak(1, 30, 31);
        let lower = peak(2, 20, 21);
        let mut systems = [BracePortionSystem {
            system_id: 1,
            staffs: vec![
                BracePortionStaff {
                    staff_id: StaffId::new(1),
                    peaks: vec![first.key(), second.key()],
                    start_peak_index: Some(1),
                    brace_peak: None,
                },
                BracePortionStaff {
                    staff_id: StaffId::new(2),
                    peaks: vec![lower.key()],
                    start_peak_index: Some(0),
                    brace_peak: None,
                },
            ],
        }];
        let mut calls = Vec::new();
        let mut report = BracePortionReport::default();
        detect_brace_portions(
            &mut systems,
            parameters(),
            |staff, left, right| {
                calls.push((staff.value(), left, right));
                if calls.len() == 1 {
                    return Ok(Some(BraceProbe {
                        peak: peak(1, 8, 9),
                        filament: None,
                    }));
                }
                if calls.len() == 2 {
                    return Ok(Some(BraceProbe {
                        peak: peak(1, 15, 17),
                        filament: Some(BraceFilamentEvidence {
                            vertical_length: 10.0,
                            mean_curvature: 19.0,
                            extension_top: 0.0,
                            extension_bottom: 3.0,
                        }),
                    }));
                }
                Ok(None)
            },
            &mut report,
        )
        .unwrap();
        assert_eq!(calls[0], (1, 3, 17));
        assert_eq!(calls[1], (1, 13, 27));
        assert_eq!(report.replacements.len(), 1);
        let brace = systems[0].staffs[0].brace_peak.as_ref().unwrap();
        assert!(brace.is_set(StaffPeakAttribute::BraceTop));
    }

    #[test]
    fn exact_height_and_strict_curvature_and_extension_gates_match_java() {
        let one = peak(1, 20, 21);
        let two = peak(2, 20, 21);
        let three = peak(3, 20, 21);
        let mut systems = [BracePortionSystem {
            system_id: 1,
            staffs: [one, two, three]
                .iter()
                .enumerate()
                .map(|(index, peak)| BracePortionStaff {
                    staff_id: StaffId::new(index + 1),
                    peaks: vec![peak.key()],
                    start_peak_index: Some(0),
                    brace_peak: None,
                })
                .collect(),
        }];
        let mut report = BracePortionReport::default();
        detect_brace_portions(
            &mut systems,
            parameters(),
            |staff, _, _| {
                Ok(Some(BraceProbe {
                    peak: peak(staff.value(), 10, 12),
                    filament: Some(BraceFilamentEvidence {
                        vertical_length: 9.0,
                        mean_curvature: if staff.value() == 3 { 20.0 } else { 19.0 },
                        extension_top: 3.0,
                        extension_bottom: 2.0,
                    }),
                }))
            },
            &mut report,
        )
        .unwrap();
        assert!(systems[0].staffs[0].brace_peak.is_some());
        assert!(
            systems[0].staffs[1]
                .brace_peak
                .as_ref()
                .unwrap()
                .is_set(StaffPeakAttribute::BraceBottom)
        );
        assert!(systems[0].staffs[2].brace_peak.is_none());
    }

    #[test]
    fn replacement_updates_column_then_copies_incoming_and_outgoing_edges() {
        let top = graded_peak(1, 10, 11);
        let old = graded_peak(2, 20, 21);
        let bottom = graded_peak(3, 30, 31);
        let mut replacement = graded_peak(2, 16, 18);
        replacement.set(StaffPeakAttribute::Brace);
        let mut graph = PeakGraph::new();
        for peak in [&top, &old, &bottom] {
            graph.add_vertex(peak.clone());
        }
        let incoming_relation = relation(&top, &old, 1, 2);
        let outgoing_relation = relation(&old, &bottom, 2, 3);
        graph
            .add_edge(top.key(), old.key(), incoming_relation)
            .unwrap();
        graph
            .add_edge(old.key(), bottom.key(), outgoing_relation)
            .unwrap();
        let mut column =
            BarColumn::new(vec![StaffId::new(1), StaffId::new(2), StaffId::new(3)]).unwrap();
        column
            .add_peak(
                BarPeak::new(PeakId::new(2), StaffId::new(2), 2.0, 20.5, false, false).unwrap(),
            )
            .unwrap();
        let mut columns = [BraceColumnSet {
            system_id: 1,
            columns: vec![column],
        }];
        let mut systems = [BracePortionSystem {
            system_id: 1,
            staffs: vec![BracePortionStaff {
                staff_id: StaffId::new(2),
                peaks: vec![old.key()],
                start_peak_index: Some(0),
                brace_peak: Some(replacement.clone()),
            }],
        }];
        let intent = BraceReplacement {
            system_id: 1,
            staff_id: StaffId::new(2),
            old_peak: old.key(),
            new_peak: replacement.key(),
        };
        apply_brace_replacements(&mut graph, &mut systems, &mut columns, &[intent]).unwrap();

        assert_eq!(systems[0].staffs[0].peaks, vec![replacement.key()]);
        assert!(!graph.contains_vertex(old.key()));
        assert_eq!(graph.vertices().last().unwrap().key(), replacement.key());
        assert_eq!(graph.edges().len(), 2);
        assert_eq!(
            (graph.edges()[0].source(), graph.edges()[0].target()),
            (top.key(), replacement.key())
        );
        assert_eq!(
            (graph.edges()[1].source(), graph.edges()[1].target()),
            (replacement.key(), bottom.key())
        );
        assert_eq!(*graph.edges()[0].relation(), incoming_relation);
        assert_eq!(
            columns[0].columns[0].peaks()[1].unwrap().id(),
            PeakId::new(4)
        );
    }

    #[test]
    fn duplicate_edge_failure_keeps_column_projector_and_vertex_prefix() {
        let top = graded_peak(1, 10, 11);
        let old = graded_peak(2, 20, 21);
        let mut replacement = graded_peak(2, 16, 18);
        replacement.set(StaffPeakAttribute::Brace);
        let mut graph = PeakGraph::new();
        for peak in [&top, &old, &replacement] {
            graph.add_vertex(peak.clone());
        }
        graph
            .add_edge(top.key(), old.key(), relation(&top, &old, 1, 2))
            .unwrap();
        graph
            .add_edge(
                top.key(),
                replacement.key(),
                relation(&top, &replacement, 1, 3),
            )
            .unwrap();
        let mut column = BarColumn::new(vec![StaffId::new(1), StaffId::new(2)]).unwrap();
        column
            .add_peak(
                BarPeak::new(PeakId::new(2), StaffId::new(2), 2.0, 20.5, false, false).unwrap(),
            )
            .unwrap();
        let mut columns = [BraceColumnSet {
            system_id: 1,
            columns: vec![column],
        }];
        let mut systems = [BracePortionSystem {
            system_id: 1,
            staffs: vec![BracePortionStaff {
                staff_id: StaffId::new(2),
                peaks: vec![old.key()],
                start_peak_index: Some(0),
                brace_peak: Some(replacement.clone()),
            }],
        }];
        let intent = BraceReplacement {
            system_id: 1,
            staff_id: StaffId::new(2),
            old_peak: old.key(),
            new_peak: replacement.key(),
        };
        assert!(matches!(
            apply_brace_replacements(&mut graph, &mut systems, &mut columns, &[intent]),
            Err(BracePortionError::Graph(
                PeakGraphError::DuplicateEdge { .. }
            ))
        ));
        assert_eq!(
            systems[0].staffs[0].peaks,
            vec![replacement.key(), old.key()]
        );
        assert!(graph.contains_vertex(old.key()));
        assert_eq!(
            columns[0].columns[0].peaks()[1].unwrap().id(),
            PeakId::new(3)
        );
    }

    #[test]
    fn stable_ids_survive_an_earlier_graph_vertex_purge() {
        let old = graded_peak(2, 20, 21);
        let bottom = graded_peak(3, 30, 31);
        let mut replacement = graded_peak(2, 16, 18);
        replacement.set(StaffPeakAttribute::Brace);
        let mut graph = PeakGraph::new();
        graph.add_vertex(old.clone());
        graph.add_vertex(bottom.clone());
        let removed = graded_peak(1, 10, 11);
        let mut peak_ids = vec![
            (PeakId::new(1), removed.key()),
            (PeakId::new(2), old.key()),
            (PeakId::new(3), bottom.key()),
        ];
        let mut column = BarColumn::new(vec![StaffId::new(2), StaffId::new(3)]).unwrap();
        column
            .add_peak(
                BarPeak::new(PeakId::new(2), StaffId::new(2), 2.0, 20.5, false, false).unwrap(),
            )
            .unwrap();
        let mut columns = [BraceColumnSet {
            system_id: 1,
            columns: vec![column],
        }];
        let mut systems = [BracePortionSystem {
            system_id: 1,
            staffs: vec![BracePortionStaff {
                staff_id: StaffId::new(2),
                peaks: vec![old.key()],
                start_peak_index: Some(0),
                brace_peak: Some(replacement.clone()),
            }],
        }];
        let intent = BraceReplacement {
            system_id: 1,
            staff_id: StaffId::new(2),
            old_peak: old.key(),
            new_peak: replacement.key(),
        };

        apply_brace_replacements_with_peak_ids(
            &mut graph,
            &mut peak_ids,
            &mut systems,
            &mut columns,
            &[intent],
        )
        .unwrap();

        assert_eq!(
            columns[0].columns[0].peaks()[0].unwrap().id(),
            PeakId::new(4)
        );
        assert_eq!(peak_ids.last(), Some(&(PeakId::new(4), replacement.key())));
        assert!(!graph.contains_vertex(old.key()));
        assert!(graph.contains_vertex(replacement.key()));
    }
}
