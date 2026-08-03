// SPDX-License-Identifier: AGPL-3.0-or-later

//! Raw-raster construction boundary for Java's per-staff `StaffProjector`s.
//!
//! This adapter consumes the prepared staff prefix and the live binary raster,
//! then runs the already ported neutral projection logic. Every retained peak
//! receives the sheet's exact deskew transform before it enters
//! [`BarsProjectorRegistry`]. Detached brace candidates receive the same
//! treatment. Peak-graph alignment and system/column grouping remain later
//! stages.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use audiveris_image::{
    bar_column::StaffId,
    filament::{FilamentError, FilamentGeometry},
    lines_coordinator::StaffCandidateKind,
    prepared_lines::{PreparedStaff, PreparedStaffHandoff},
    projection::{
        BarlineHeightSpec, BarsProjectorRegistry, BraceSearchRequest, PeakCoreGeometry,
        ProjectionError, ProjectorRegistration, StaffProjectorProcessRequest,
        StaffProjectorProcessTuning, StaffProjectorScaleRatios, StaffProjectorScaleRequest,
        barline_height, process_staff_projection,
    },
    staff_peak::{StaffPeak, StaffPeakError},
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

#[derive(Clone, Debug, PartialEq)]
pub enum RawProjectorAdapterError {
    DuplicateStaffSettings(usize),
    UnknownStaffSettings(usize),
    MissingStaffSettings(usize),
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
        }
    }
}

impl Error for RawProjectorAdapterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Filament { source, .. } => Some(source),
            Self::Projection { source, .. } => Some(source),
            Self::Deskew { source, .. } => Some(source),
            _ => None,
        }
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
}
