// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production composition of native GRID and BEAMS output into LEDGERS.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use audiveris_image::{section::Bounds, system_population::StaffBoundary};

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
    pub builder_survivor_count: usize,
    pub discarded_filament_ids: Vec<usize>,
    pub rebuilt_system_ids: Vec<usize>,
    pub ledger_lines: Vec<NativeLedgerLine>,
    pub materializer: LedgerMaterializer,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeLedgerLine {
    pub system_id: usize,
    pub staff_id: usize,
    pub index: i32,
    /// Cumulative vertical translation from the first/last staff line.
    pub translation_y: f64,
    pub geometry: StaffBoundary,
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

/// Run Java's `LedgersFilter`, `LedgersBuilder` and sheet-wide statistical
/// post-analysis composition over native GRID/BEAMS products.
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
    let mut system_candidates = Vec::new();
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
        system_candidates.push((system.id, sourced.candidates));
    }

    let builder = materialize_ledgers(&staves, &system_candidates, scale, parameters)?;
    let builder_survivor_count = builder
        .inters()
        .iter()
        .filter(|inter| !inter.removed)
        .count();
    let discarded = post_analysis_discards(&staves, &builder);
    let rebuilt_system_ids = discarded.keys().copied().collect::<Vec<_>>();
    let discarded_filament_ids = discarded
        .values()
        .flat_map(BTreeSet::iter)
        .copied()
        .collect::<Vec<_>>();
    // `LedgersBuilder.stick2ledger` retains inters removed by an overlap
    // exclusion. On a later post-analysis rebuild Java finds that removed
    // inter and refuses to add it again; rebuilding from candidate geometry
    // must therefore carry the same tombstones explicitly.
    let retired_filament_ids = builder
        .inters()
        .iter()
        .filter(|inter| inter.removed)
        .map(|inter| inter.filament_id)
        .collect::<BTreeSet<_>>();
    let rebuilt_candidates = system_candidates
        .iter()
        .map(|(system_id, candidates)| {
            let removed = discarded.get(system_id);
            (
                *system_id,
                candidates
                    .iter()
                    .filter(|candidate| {
                        !retired_filament_ids.contains(&candidate.id)
                            && removed.is_none_or(|ids| !ids.contains(&candidate.id))
                    })
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    let materializer = materialize_ledgers(&staves, &rebuilt_candidates, scale, parameters)?;
    let ledger_lines = build_ledger_lines(&staves, &materializer);

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
        builder_survivor_count,
        discarded_filament_ids,
        rebuilt_system_ids,
        ledger_lines,
        materializer,
    })
}

fn build_ledger_lines(
    staves: &[RawLedgerStaffZone],
    materializer: &LedgerMaterializer,
) -> Vec<NativeLedgerLine> {
    let mut lines = Vec::new();
    for staff in staves {
        for increment in [-1, 1] {
            let mut geometry = if increment < 0 {
                staff.first_line.clone()
            } else {
                staff.last_line.clone()
            };
            let mut translation_y = 0.0;
            let mut index = increment;
            loop {
                let inter_ids = materializer.staff_inter_ids(staff.system_id, staff.id, index);
                if inter_ids.is_empty() {
                    break;
                }
                let mut sum = 0.0;
                let mut count = 0;
                for inter in inter_ids
                    .iter()
                    .filter_map(|id| materializer.inter_by_id(*id))
                {
                    let middle = (
                        (inter.median.0.0 + inter.median.1.0) / 2.0,
                        (inter.median.0.1 + inter.median.1.1) / 2.0,
                    );
                    let Some(y_line) = geometry.geopath_y_at_x(middle.0) else {
                        continue;
                    };
                    sum += middle.1 - y_line;
                    count += 1;
                }
                if count == 0 {
                    break;
                }
                let delta = sum / f64::from(count);
                translation_y += delta;
                geometry = geometry.translated_y(delta);
                lines.push(NativeLedgerLine {
                    system_id: staff.system_id,
                    staff_id: staff.id,
                    index,
                    translation_y,
                    geometry: geometry.clone(),
                });
                index += increment;
            }
        }
    }
    lines
}

fn materialize_ledgers(
    staves: &[RawLedgerStaffZone],
    system_candidates: &[(usize, Vec<RawLedgerCandidate>)],
    scale: RawLedgerScale,
    parameters: RawLedgerCandidateParameters,
) -> Result<LedgerMaterializer, NativeLedgerRecognitionError> {
    let mut materializer = LedgerMaterializer::new(1, 1, 1);
    for (system_id, candidates) in system_candidates {
        for staff in staves.iter().filter(|staff| staff.system_id == *system_id) {
            for increment in [-1, 1] {
                let mut index = increment;
                let mut previous = Vec::new();
                loop {
                    let evaluated = evaluate_ledger_line_unreduced(
                        staff, index, candidates, &previous, scale, parameters,
                    );
                    if evaluated.is_empty() {
                        break;
                    }
                    let outcome =
                        materializer.materialize_line(*system_id, staff.id, index, &evaluated);
                    if let Some(source) = outcome.error {
                        return Err(NativeLedgerRecognitionError::Materialization {
                            system_id: *system_id,
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
    Ok(materializer)
}

#[derive(Clone, Copy)]
struct LedgerPostInfo {
    system_id: usize,
    filament_id: usize,
    index: i32,
    interline: i32,
    delta: f64,
    delta_ratio: f64,
    height: f64,
    height_ratio: f64,
}

#[derive(Default)]
struct LedgerPopulation {
    count: usize,
    sum: f64,
    squares_sum: f64,
}

impl LedgerPopulation {
    fn include(&mut self, value: f64) {
        self.count += 1;
        self.sum += value;
        self.squares_sum += value * value;
    }

    fn mean_and_deviation(&self) -> Option<(f64, f64)> {
        if self.count == 0 {
            return None;
        }
        let count = self.count as f64;
        let mean = self.sum / count;
        if self.count == 1 {
            return Some((mean, 0.0));
        }
        let biased = ((self.squares_sum - ((self.sum * self.sum) / count)) / count).max(0.0);
        Some((mean, ((count * biased) / (count - 1.0)).sqrt()))
    }
}

fn post_analysis_discards(
    staves: &[RawLedgerStaffZone],
    materializer: &LedgerMaterializer,
) -> BTreeMap<usize, BTreeSet<usize>> {
    let mut delta_above = LedgerPopulation::default();
    let mut delta_below = LedgerPopulation::default();
    let mut heights = LedgerPopulation::default();
    // Java's `infoMap` is keyed by ledger identity. A ledger reused in more
    // than one staff-map entry contributes every observation to populations,
    // while its last (TreeMap-order) entry supplies the eventual filter info.
    let mut infos = BTreeMap::new();
    let mut discarded: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();

    for staff in staves {
        let indexes = materializer.staff_ledger_indexes(staff.system_id, staff.id);
        for index in indexes {
            for inter_id in materializer.staff_inter_ids(staff.system_id, staff.id, index) {
                let Some(inter) = materializer.inter_by_id(*inter_id) else {
                    continue;
                };
                let middle = (
                    (inter.median.0.0 + inter.median.1.0) / 2.0,
                    (inter.median.0.1 + inter.median.1.1) / 2.0,
                );
                let delta = if index.abs() >= 2 {
                    let previous_index = index - index.signum();
                    let previous = materializer
                        .staff_inter_ids(staff.system_id, staff.id, previous_index)
                        .iter()
                        .filter_map(|id| materializer.inter_by_id(*id))
                        .find(|previous| {
                            middle.0 >= previous.bounds.x
                                && middle.0 < previous.bounds.x + previous.bounds.width
                        });
                    let Some(previous) = previous else {
                        discarded
                            .entry(inter.system_id)
                            .or_default()
                            .insert(inter.filament_id);
                        continue;
                    };
                    (middle.1 - line_y_at(previous.median, middle.0)).abs()
                } else {
                    staff.distance_to(middle.0, middle.1)
                };
                let interline = f64::from(staff.specific_interline);
                let info = LedgerPostInfo {
                    system_id: inter.system_id,
                    filament_id: inter.filament_id,
                    index,
                    interline: staff.specific_interline,
                    delta,
                    delta_ratio: delta / interline,
                    height: inter.thickness,
                    height_ratio: inter.thickness / interline,
                };
                if index < 0 {
                    delta_above.include(info.delta_ratio);
                } else {
                    delta_below.include(info.delta_ratio);
                }
                heights.include(info.height_ratio);
                infos.insert(inter.id, info);
            }
        }
    }

    let above = delta_above.mean_and_deviation();
    let below = delta_below.mean_and_deviation();
    let height = heights.mean_and_deviation();
    for info in infos.into_values() {
        let Some((delta_mean, delta_deviation)) = (if info.index < 0 { above } else { below })
        else {
            continue;
        };
        let Some((height_mean, height_deviation)) = height else {
            continue;
        };
        let interline = f64::from(info.interline);
        let min_delta = ((delta_mean - delta_deviation) * interline).floor();
        let max_delta = ((delta_mean + delta_deviation) * interline).ceil();
        let min_height = ((height_mean - height_deviation) * interline).floor();
        let max_height = ((height_mean + height_deviation) * interline).ceil();
        if info.delta.ceil() < min_delta
            || info.delta.floor() > max_delta
            || info.height.ceil() < min_height
            || info.height.floor() > max_height
        {
            discarded
                .entry(info.system_id)
                .or_default()
                .insert(info.filament_id);
        }
    }
    discarded
}

fn line_y_at(median: ((f64, f64), (f64, f64)), x: f64) -> f64 {
    let dx = median.1.0 - median.0.0;
    if dx == 0.0 {
        median.0.1
    } else {
        median.0.1 + ((x - median.0.0) * (median.1.1 - median.0.1) / dx)
    }
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

#[cfg(test)]
mod tests {
    use super::LedgerPopulation;

    #[test]
    fn ledger_population_uses_java_unbiased_standard_deviation() {
        let mut population = LedgerPopulation::default();
        for value in [1.0, 2.0, 3.0] {
            population.include(value);
        }

        assert_eq!(population.mean_and_deviation(), Some((2.0, 1.0)));
    }
}
