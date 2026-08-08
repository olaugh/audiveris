// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production composition of native GRID and BEAMS output into LEDGERS.

use std::{error::Error, fmt};

use audiveris_image::section::Bounds;

use crate::{
    beam_inters::{BeamKind, beam_bounds},
    raw_ledger_filter::{
        LedgerMaterializationError, LedgerMaterializer, LedgerPreviousReference,
        MaterializedLedgerInter, NativeLedgerCandidateOutcome, RawLedgerBeamArea,
        RawLedgerCandidate, RawLedgerCandidateParameters, RawLedgerFilterError,
        RawLedgerFilterParameters, RawLedgerScale, RawLedgerStaffZone, RawLedgerSystemZone,
        evaluate_ledger_line_unreduced, filter_raw_ledger_sections,
        source_native_ledger_candidates,
    },
    recognize::{GridLinesRecognition, NativeBeamRecognition},
};

#[derive(Clone, Debug)]
pub struct NativeLedgerRecognition {
    pub filtered_run_count: usize,
    pub section_count: usize,
    pub system_section_counts: Vec<(usize, usize)>,
    pub candidates: Vec<(usize, RawLedgerCandidate)>,
    pub registered_filament_count: usize,
    pub materializer: LedgerMaterializer,
}

impl NativeLedgerRecognition {
    #[must_use]
    pub fn ledgers(&self) -> Vec<&MaterializedLedgerInter> {
        self.materializer
            .inters()
            .iter()
            .filter(|inter| !inter.removed)
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeLedgerRecognitionError {
    InvalidScale,
    MissingStaffArea(usize),
    MissingStaffLines(usize),
    MissingSystemArea(usize),
    MissingSystemBounds(usize),
    Filter(RawLedgerFilterError),
    Candidate {
        system_id: usize,
        message: String,
    },
    Materialization {
        system_id: usize,
        staff_id: usize,
        index: i32,
        source: LedgerMaterializationError,
    },
}

impl fmt::Display for NativeLedgerRecognitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScale => formatter.write_str("invalid LEDGERS scale"),
            Self::MissingStaffArea(id) => write!(formatter, "staff {id} has no GRID area"),
            Self::MissingStaffLines(id) => write!(formatter, "staff {id} has no GRID lines"),
            Self::MissingSystemArea(id) => write!(formatter, "system {id} has no GRID area"),
            Self::MissingSystemBounds(id) => write!(formatter, "system {id} has no GRID bounds"),
            Self::Filter(source) => write!(formatter, "LEDGERS filter failed: {source}"),
            Self::Candidate { system_id, message } => {
                write!(
                    formatter,
                    "system {system_id} candidate factory failed: {message}"
                )
            }
            Self::Materialization {
                system_id,
                staff_id,
                index,
                source,
            } => write!(
                formatter,
                "system {system_id} staff {staff_id} ledger line {index} failed: {source}"
            ),
        }
    }
}

impl Error for NativeLedgerRecognitionError {}

/// Run Java's `LedgersFilter` and `LedgersBuilder` composition over native
/// GRID/BEAMS products. The sheet-wide statistical post-analysis is a separate
/// tail and is deliberately not hidden in this result.
pub fn recognize_native_ledgers(
    grid: &GridLinesRecognition,
    beams: &NativeBeamRecognition,
) -> Result<NativeLedgerRecognition, NativeLedgerRecognitionError> {
    let large_interline = grid.scale.scale.interline.main;
    let mean_line_thickness = f64::from(grid.scale.scale.line.main);
    if large_interline <= 0 || mean_line_thickness <= 0.0 {
        return Err(NativeLedgerRecognitionError::InvalidScale);
    }
    let scale = RawLedgerScale {
        large_interline,
        mean_line_thickness,
    };
    let parameters = RawLedgerCandidateParameters::default();
    let staves = build_staff_zones(grid)?;
    let systems = build_system_zones(grid, beams)?;
    let filtered = filter_raw_ledger_sections(
        &grid.no_staff,
        &staves,
        &systems,
        RawLedgerFilterParameters::default(),
    )
    .map_err(NativeLedgerRecognitionError::Filter)?;

    let mut next_filament_id = 1_u64;
    let mut registered_filament_count = 0;
    let mut candidates = Vec::new();
    let mut materializer = LedgerMaterializer::new(1, 1, 1);
    for system in &systems {
        let sections = filtered
            .by_system
            .iter()
            .find(|sections| sections.system_id == system.id)
            .map_or(&[][..], |sections| sections.sections.as_slice());
        let sourced = source_native_ledger_candidates(
            &grid.no_staff,
            system,
            sections,
            scale,
            parameters,
            next_filament_id,
        );
        check_candidate_outcome(system.id, &sourced)?;
        next_filament_id = sourced.next_filament_id;
        registered_filament_count += sourced.registered_filament_ids.len();
        candidates.extend(
            sourced
                .candidates
                .iter()
                .cloned()
                .map(|candidate| (system.id, candidate)),
        );

        for staff in staves.iter().filter(|staff| staff.system_id == system.id) {
            for increment in [-1, 1] {
                let mut index = increment;
                let mut previous = Vec::new();
                loop {
                    let evaluated = evaluate_ledger_line_unreduced(
                        staff,
                        index,
                        &sourced.candidates,
                        &previous,
                        scale,
                        parameters,
                    );
                    if evaluated.is_empty() {
                        break;
                    }
                    let outcome =
                        materializer.materialize_line(system.id, staff.id, index, &evaluated);
                    if let Some(source) = outcome.error {
                        return Err(NativeLedgerRecognitionError::Materialization {
                            system_id: system.id,
                            staff_id: staff.id,
                            index,
                            source,
                        });
                    }
                    if outcome.survivor_inter_ids.is_empty() {
                        break;
                    }
                    previous = outcome
                        .survivor_inter_ids
                        .iter()
                        .filter_map(|id| materializer.inter_by_id(*id))
                        .map(|inter| LedgerPreviousReference {
                            candidate_id: inter.filament_id,
                            bounds: inter.bounds,
                            start: inter.median.0,
                            stop: inter.median.1,
                        })
                        .collect();
                    index += increment;
                }
            }
        }
    }

    Ok(NativeLedgerRecognition {
        filtered_run_count: filtered.run_table.total_run_count(),
        section_count: filtered.sections.len(),
        system_section_counts: filtered
            .by_system
            .iter()
            .map(|sections| (sections.system_id, sections.sections.len()))
            .collect(),
        candidates,
        registered_filament_count,
        materializer,
    })
}

fn check_candidate_outcome(
    system_id: usize,
    outcome: &NativeLedgerCandidateOutcome,
) -> Result<(), NativeLedgerRecognitionError> {
    if let Some(source) = &outcome.error {
        return Err(NativeLedgerRecognitionError::Candidate {
            system_id,
            message: source.to_string(),
        });
    }
    Ok(())
}

fn build_staff_zones(
    grid: &GridLinesRecognition,
) -> Result<Vec<RawLedgerStaffZone>, NativeLedgerRecognitionError> {
    let merged_threshold = (2.5 * f64::from(grid.scale.scale.interline.main)).round_ties_even();
    grid.staves
        .iter()
        .map(|staff| {
            let system_id = grid
                .peak_graph
                .systems
                .iter()
                .position(|ids| ids.contains(&staff.id))
                .map_or(0, |index| index + 1);
            let area = grid
                .staff_areas
                .iter()
                .find(|area| area.staff_id == staff.id)
                .ok_or(NativeLedgerRecognitionError::MissingStaffArea(staff.id))?;
            let lines = grid
                .staff_lines
                .iter()
                .find(|lines| lines.staff_id == staff.id)
                .ok_or(NativeLedgerRecognitionError::MissingStaffLines(staff.id))?;
            let system_state = grid
                .peak_graph
                .sig
                .systems
                .iter()
                .find(|system| system.system_id == system_id);
            let part = system_state.and_then(|system| {
                system.bar_tail.parts.iter().find(|part| {
                    i32::try_from(staff.id)
                        .is_ok_and(|id| id >= part.first_staff_id && id <= part.last_staff_id)
                })
            });
            let first_in_part =
                part.is_none_or(|part| i32::try_from(staff.id).ok() == Some(part.first_staff_id));
            let last_in_part =
                part.is_none_or(|part| i32::try_from(staff.id).ok() == Some(part.last_staff_id));
            let merged_part =
                part.is_some_and(|part| {
                    let Some(system_staff_ids) =
                        grid.peak_graph.systems.get(system_id.saturating_sub(1))
                    else {
                        return false;
                    };
                    if system_staff_ids.len() != 2
                        || part.last_staff_id - part.first_staff_id + 1 != 2
                    {
                        return false;
                    }
                    let Some(first) = grid.staff_lines.iter().find(|lines| {
                        i32::try_from(lines.staff_id).ok() == Some(part.first_staff_id)
                    }) else {
                        return false;
                    };
                    let Some(last) = grid.staff_lines.iter().find(|lines| {
                        i32::try_from(lines.staff_id).ok() == Some(part.last_staff_id)
                    }) else {
                        return false;
                    };
                    let x = first.left.max(last.left);
                    last.first_line.y_at_x_ext(f64::from(x))
                        - first.last_line.y_at_x_ext(f64::from(x))
                        < merged_threshold
                });
            Ok(RawLedgerStaffZone {
                id: staff.id,
                system_id,
                specific_interline: i32::try_from(staff.interline).unwrap_or(i32::MAX),
                tablature: staff.kind == "tablature",
                merged_part,
                first_in_part,
                last_in_part,
                area: area.area.clone(),
                first_line: lines.first_line.clone(),
                last_line: lines.last_line.clone(),
            })
        })
        .collect()
}

fn build_system_zones(
    grid: &GridLinesRecognition,
    beams: &NativeBeamRecognition,
) -> Result<Vec<RawLedgerSystemZone>, NativeLedgerRecognitionError> {
    grid.peak_graph
        .systems
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let id = index + 1;
            let area = grid
                .system_areas
                .iter()
                .find(|area| area.system_id == id)
                .ok_or(NativeLedgerRecognitionError::MissingSystemArea(id))?;
            let bounds = grid
                .system_bounds
                .iter()
                .find(|bounds| bounds.system_id == id)
                .ok_or(NativeLedgerRecognitionError::MissingSystemBounds(id))?;
            let raw = beams
                .raw_beams
                .iter()
                .filter(|(system_id, _)| *system_id == id)
                .map(|(_, beam)| raw_beam_area(beam.item))
                .collect::<Vec<_>>();
            let mut all_beams = raw.clone();
            all_beams.extend(
                beams
                    .hooks
                    .iter()
                    .filter(|(system_id, _)| *system_id == id)
                    .map(|(_, beam)| raw_beam_area(beam.item)),
            );
            let good_full_beams = beams
                .raw_beams
                .iter()
                .filter(|(system_id, beam)| {
                    *system_id == id && beam.kind == BeamKind::Beam && beam.grade >= 0.4
                })
                .map(|(_, beam)| raw_beam_area(beam.item))
                .collect();
            Ok(RawLedgerSystemZone {
                id,
                left: bounds.left,
                right: bounds.right,
                area: area.clone(),
                all_beams,
                good_full_beams,
            })
        })
        .collect()
}

fn raw_beam_area(item: audiveris_image::beam_structure::BeamItem) -> RawLedgerBeamArea {
    let bounds = beam_bounds(item);
    RawLedgerBeamArea {
        bounds: Bounds {
            x: usize::try_from(bounds.x).unwrap_or(0),
            y: usize::try_from(bounds.y).unwrap_or(0),
            width: usize::try_from(bounds.width).unwrap_or(0),
            height: usize::try_from(bounds.height).unwrap_or(0),
        },
        item,
    }
}
