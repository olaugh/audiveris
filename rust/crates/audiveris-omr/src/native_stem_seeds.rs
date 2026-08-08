// SPDX-License-Identifier: AGPL-3.0-or-later

//! Native `STEM_SEEDS` candidate construction and acceptance.
//!
//! GRID's live section lags are selected per system and passed to vertical
//! `StickFactory`.  The accepted boundary then composes HEADERS' staff stops,
//! the concrete `StemChecker`, original-glyph materialization, and the system
//! free-glyph collection.  It deliberately stops before BEAMS consumes the
//! seeds.

use std::{error::Error, fmt};

use audiveris_image::{
    lines_coordinator::StaffCandidateKind,
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable, RunTableError},
    section::{Bounds, Section},
    stick_factory::{
        IdentifiedVerticalStick, StraightStickError, VerticalStickFactory, VerticalStickParameters,
    },
    system_population::PopulationSystemArea,
};

use crate::{
    native_headers::NativeHeaderRecognition,
    recognize::GridLinesRecognition,
    stem_seeds_step::{
        NativeStemCheckResult, NativeStemCheckerParameters, NativeStemGeometryError,
        NativeStemImpacts, NeutralStemGeometry, StemScaleComputation, check_native_stem,
        compute_stem_scale,
    },
};

/// Java's default `SystemInfo.getProfile()` during batch recognition.
pub const DEFAULT_STEM_SEEDS_PROFILE: i32 = 1;
pub const STEM_SEEDS_MINIMUM_SIDE_RATIO: f64 = 0.4;
pub const STEM_SEEDS_MINIMUM_CORE_RATIO: f64 = 1.5;
/// Java `StemChecker.Constants.minCheckResult`.
pub const STEM_SEEDS_MINIMUM_GRADE: f64 = 0.2;
/// Java `StemChecker.Constants.beltMarginDx`, in interline fractions.
pub const STEM_SEEDS_BELT_MARGIN_RATIO: f64 = 0.15;

/// Why production did not materialize or check a raw candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeStemSeedSkipReason {
    NoStaff,
    Tablature,
    Header,
}

/// Staff/header inputs common to a skipped or checked candidate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemSeedGate {
    pub ordinal: usize,
    pub center: (f64, f64),
    pub staff_id: Option<usize>,
    pub header_stop: Option<i32>,
    pub tablature: bool,
}

/// A fixed glyph produced by Java `SectionCompound.toGlyph(null)`.
///
/// `vertical_seed_group` and `free` describe the final state after the grade
/// gate. Rejected checked glyphs are still present in `registered_glyphs`, but
/// both flags remain false.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemSeedGlyph {
    pub source_ordinal: usize,
    pub vertical_seed_group: bool,
    pub free: bool,
    pub bounds: Bounds,
    pub weight: usize,
    pub start: (f64, f64),
    pub stop: (f64, f64),
    pub mean_thickness: f64,
    pub mean_distance: f64,
    pub run_table: RunTable,
}

impl NativeStemSeedGlyph {
    /// Number of concrete runs in the cropped fixed-glyph table.
    #[must_use]
    pub fn run_count(&self) -> usize {
        (0..self.run_table.sequence_count())
            .map(|index| self.run_table.sequence(index).map_or(0, <[_]>::len))
            .sum()
    }

    /// The accepted-glyph oracle's FNV-1a-64 table digest: orientation and
    /// dimensions followed by every sequence's `start:length` runs.
    #[must_use]
    pub fn run_digest(&self) -> u64 {
        let orientation = match self.run_table.orientation() {
            Orientation::Horizontal => "HORIZONTAL",
            Orientation::Vertical => "VERTICAL",
        };
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        hash_row(
            &mut hash,
            &format!(
                "{orientation} {} {}",
                self.run_table.width(),
                self.run_table.height()
            ),
        );
        for sequence in 0..self.run_table.sequence_count() {
            let mut row = sequence.to_string();
            for run in self.run_table.sequence(sequence).unwrap_or_default() {
                row.push_str(&format!(" {}:{}", run.start, run.length));
            }
            hash_row(&mut hash, &row);
        }
        hash
    }
}

fn hash_row(hash: &mut u64, row: &str) {
    for byte in row.bytes().chain(std::iter::once(b'\n')) {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x100_0000_01b3);
    }
}

/// Final decision for one raw factory candidate, in raw ordinal order.
#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemSeedDecision {
    Skipped {
        gate: NativeStemSeedGate,
        reason: NativeStemSeedSkipReason,
    },
    Checked {
        gate: NativeStemSeedGate,
        threshold: f64,
        weights: NativeStemImpacts,
        check: Box<NativeStemCheckResult>,
        registered_glyph_index: usize,
        accepted: bool,
        free_glyph_index: Option<usize>,
    },
}

/// One system after `VerticalsBuilder.checkVerticals(Collection)`.
#[derive(Debug)]
pub struct NativeStemSeedSystemRecognition {
    /// The exact factory input/output which this acceptance pass consumed.
    pub raw: RawStemSeedSystem,
    pub decisions: Vec<NativeStemSeedDecision>,
    /// Every checked candidate, because Java registers before grading.
    pub registered_glyphs: Vec<NativeStemSeedGlyph>,
    /// Accepted `VERTICAL_SEED` system free glyphs, in Java insertion order.
    pub free_glyphs: Vec<NativeStemSeedGlyph>,
}

/// Production boundary immediately before BEAMS.
#[derive(Debug)]
pub struct NativeStemSeedRecognition {
    pub maximum_stem_thickness: i32,
    pub systems: Vec<NativeStemSeedSystemRecognition>,
}

#[derive(Debug)]
pub struct RawStemSeedSystem {
    pub system_id: usize,
    pub profile: i32,
    pub left: i32,
    pub right: i32,
    pub interline: i32,
    pub maximum_stem_thickness: i32,
    pub minimum_core_section_length: i32,
    pub minimum_side_ratio: f64,
    /// Exact sections selected by `VerticalsBuilder`, in system lag order.
    pub vertical_sections: Vec<Section>,
    /// Exact one-pixel-wide horizontal stickers, in system lag order.
    pub horizontal_stickers: Vec<Section>,
    /// Raw factory output in decreasing-core traversal order.
    pub candidates: Vec<IdentifiedVerticalStick>,
}

#[derive(Debug)]
pub struct RawStemSeedRecognition {
    pub maximum_stem_thickness: i32,
    pub systems: Vec<RawStemSeedSystem>,
}

#[derive(Debug)]
pub enum RawStemSeedRecognitionError {
    RunTable(RunTableError),
    InvalidInterline(i32),
    InvalidMaximumStem(i32),
    MissingSystemBounds(usize),
    MissingSystemArea(usize),
    StickFactory {
        system_id: usize,
        source: StraightStickError,
    },
}

impl fmt::Display for RawStemSeedRecognitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RunTable(source) => write!(formatter, "stem run table failed: {source}"),
            Self::InvalidInterline(value) => write!(formatter, "invalid stem interline {value}"),
            Self::InvalidMaximumStem(value) => {
                write!(formatter, "invalid maximum stem thickness {value}")
            }
            Self::MissingSystemBounds(system_id) => {
                write!(formatter, "system {system_id} has no GRID bounds")
            }
            Self::MissingSystemArea(system_id) => {
                write!(formatter, "system {system_id} has no GRID area")
            }
            Self::StickFactory { system_id, source } => {
                write!(
                    formatter,
                    "system {system_id} vertical StickFactory failed: {source}"
                )
            }
        }
    }
}

impl Error for RawStemSeedRecognitionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RunTable(source) => Some(source),
            Self::StickFactory { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<RunTableError> for RawStemSeedRecognitionError {
    fn from(source: RunTableError) -> Self {
        Self::RunTable(source)
    }
}

#[derive(Debug)]
pub enum NativeStemSeedRecognitionError {
    Raw(RawStemSeedRecognitionError),
    MissingHeaderSystem(usize),
    MissingHeaderStaff {
        system_id: usize,
        staff_id: usize,
    },
    MissingStaffArea {
        system_id: usize,
        staff_id: usize,
    },
    MissingStaffGeometry {
        system_id: usize,
        staff_id: usize,
    },
    MissingSheetStaff {
        system_id: usize,
        staff_id: usize,
    },
    Geometry {
        system_id: usize,
        ordinal: usize,
        source: StraightStickError,
    },
    Check {
        system_id: usize,
        ordinal: usize,
        source: NativeStemGeometryError,
    },
    Materialization {
        system_id: usize,
        ordinal: usize,
        source: RunTableError,
    },
}

impl fmt::Display for NativeStemSeedRecognitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Raw(source) => write!(formatter, "raw stem-seed recognition failed: {source}"),
            Self::MissingHeaderSystem(id) => {
                write!(formatter, "STEM_SEEDS system {id} has no HEADERS result")
            }
            Self::MissingHeaderStaff {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "STEM_SEEDS system {system_id} staff {staff_id} has no HEADERS result"
            ),
            Self::MissingStaffArea {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "STEM_SEEDS system {system_id} staff {staff_id} has no GRID area"
            ),
            Self::MissingStaffGeometry {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "STEM_SEEDS system {system_id} staff {staff_id} has no GRID line geometry"
            ),
            Self::MissingSheetStaff {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "STEM_SEEDS system {system_id} staff {staff_id} has no GRID staff"
            ),
            Self::Geometry {
                system_id,
                ordinal,
                source,
            } => write!(
                formatter,
                "STEM_SEEDS system {system_id} candidate {ordinal} geometry failed: {source}"
            ),
            Self::Check {
                system_id,
                ordinal,
                source,
            } => write!(
                formatter,
                "STEM_SEEDS system {system_id} candidate {ordinal} check failed: {source}"
            ),
            Self::Materialization {
                system_id,
                ordinal,
                source,
            } => write!(
                formatter,
                "STEM_SEEDS system {system_id} candidate {ordinal} materialization failed: {source}"
            ),
        }
    }
}

impl Error for NativeStemSeedRecognitionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Raw(source) => Some(source),
            Self::Geometry { source, .. } => Some(source),
            Self::Check { source, .. } => Some(source),
            Self::Materialization { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<RawStemSeedRecognitionError> for NativeStemSeedRecognitionError {
    fn from(source: RawStemSeedRecognitionError) -> Self {
        Self::Raw(source)
    }
}

/// Compose GRID's exact raw vertical candidates with HEADERS and the concrete
/// acceptance/materialization lifecycle. The returned free glyphs are the
/// inputs BEAMS would consume; this function does not run BEAMS.
pub fn recognize_native_stem_seeds(
    grid: &GridLinesRecognition,
    headers: &NativeHeaderRecognition,
) -> Result<NativeStemSeedRecognition, NativeStemSeedRecognitionError> {
    let raw = recognize_raw_stem_seed_candidates(grid)?;
    let parameters = NativeStemCheckerParameters {
        interline: grid.scale.scale.interline.main,
        maximum_stem_width: raw.maximum_stem_thickness,
        belt_margin_dx: (STEM_SEEDS_BELT_MARGIN_RATIO * f64::from(grid.scale.scale.interline.main))
            .round_ties_even() as i32,
        sheet_skew_slope: grid.global_slope,
    };
    let mut systems = Vec::with_capacity(raw.systems.len());

    for raw_system in raw.systems {
        let system_id = raw_system.system_id;
        let header_system = headers
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .ok_or(NativeStemSeedRecognitionError::MissingHeaderSystem(
                system_id,
            ))?;
        let staff_ids = grid.peak_graph.systems.get(system_id - 1).ok_or(
            NativeStemSeedRecognitionError::MissingHeaderSystem(system_id),
        )?;
        // Validate all collaborators up front so a malformed composed result
        // cannot leak a plausible prefix.
        for &staff_id in staff_ids {
            if !grid
                .staff_areas
                .iter()
                .any(|area| area.staff_id == staff_id)
            {
                return Err(NativeStemSeedRecognitionError::MissingStaffArea {
                    system_id,
                    staff_id,
                });
            }
            if !grid
                .staff_lines
                .iter()
                .any(|staff| staff.staff_id == staff_id)
            {
                return Err(NativeStemSeedRecognitionError::MissingStaffGeometry {
                    system_id,
                    staff_id,
                });
            }
            if !grid
                .peak_graph
                .sheet_staffs
                .iter()
                .any(|staff| staff.id == staff_id)
            {
                return Err(NativeStemSeedRecognitionError::MissingSheetStaff {
                    system_id,
                    staff_id,
                });
            }
            if !header_system
                .staffs
                .iter()
                .any(|staff| staff.staff_id == staff_id && staff.header.is_some())
            {
                return Err(NativeStemSeedRecognitionError::MissingHeaderStaff {
                    system_id,
                    staff_id,
                });
            }
        }

        let mut decisions = Vec::with_capacity(raw_system.candidates.len());
        let mut registered_glyphs = Vec::new();
        let mut free_glyphs = Vec::new();
        for (ordinal, candidate) in raw_system.candidates.iter().enumerate() {
            let geometry = candidate.straight_geometry().map_err(|source| {
                NativeStemSeedRecognitionError::Geometry {
                    system_id,
                    ordinal,
                    source,
                }
            })?;
            let center = (
                geometry.bounds.x as f64 + (geometry.bounds.width as f64 / 2.0),
                geometry.bounds.y as f64 + (geometry.bounds.height as f64 / 2.0),
            );
            let closest = closest_system_staff(grid, staff_ids, center.0, center.1);
            let (staff_id, header_stop, tablature) = if let Some(staff_id) = closest {
                let sheet_staff = grid
                    .peak_graph
                    .sheet_staffs
                    .iter()
                    .find(|staff| staff.id == staff_id)
                    .ok_or(NativeStemSeedRecognitionError::MissingSheetStaff {
                        system_id,
                        staff_id,
                    })?;
                let header = header_system
                    .staffs
                    .iter()
                    .find(|staff| staff.staff_id == staff_id)
                    .and_then(|staff| staff.header.as_ref())
                    .ok_or(NativeStemSeedRecognitionError::MissingHeaderStaff {
                        system_id,
                        staff_id,
                    })?;
                (
                    Some(staff_id),
                    Some(header.stop),
                    sheet_staff.kind == StaffCandidateKind::Tablature,
                )
            } else {
                (None, None, false)
            };
            let gate = NativeStemSeedGate {
                ordinal,
                center,
                staff_id,
                header_stop,
                tablature,
            };
            let reason = if staff_id.is_none() {
                Some(NativeStemSeedSkipReason::NoStaff)
            } else if tablature {
                Some(NativeStemSeedSkipReason::Tablature)
            } else if center.0 < f64::from(header_stop.expect("staff gate has a header stop")) {
                Some(NativeStemSeedSkipReason::Header)
            } else {
                None
            };
            if let Some(reason) = reason {
                decisions.push(NativeStemSeedDecision::Skipped { gate, reason });
                continue;
            }

            // Java registers the original fixed glyph before running the
            // minimum-grade gate.
            let registered_glyph_index = registered_glyphs.len();
            let check_geometry = NeutralStemGeometry {
                start_x: geometry.start.0,
                start_y: geometry.start.1,
                stop_x: geometry.stop.0,
                stop_y: geometry.stop.1,
                mean_distance: geometry.mean_distance,
                bounds_x: geometry.bounds.x as i32,
                bounds_y: geometry.bounds.y as i32,
                bounds_width: geometry.bounds.width as i32,
                bounds_height: geometry.bounds.height as i32,
            };
            let check = check_native_stem(
                &grid.no_staff,
                check_geometry,
                usize::try_from(candidate.id()).unwrap_or(usize::MAX),
                parameters,
                raw_system.profile,
            )
            .map_err(|source| NativeStemSeedRecognitionError::Check {
                system_id,
                ordinal,
                source,
            })?;
            let accepted = check.grade >= STEM_SEEDS_MINIMUM_GRADE;
            let mut glyph =
                materialize_stem_glyph(candidate, ordinal, geometry).map_err(|source| {
                    NativeStemSeedRecognitionError::Materialization {
                        system_id,
                        ordinal,
                        source,
                    }
                })?;
            let free_glyph_index = accepted.then(|| {
                glyph.vertical_seed_group = true;
                glyph.free = true;
                let index = free_glyphs.len();
                free_glyphs.push(glyph.clone());
                index
            });
            registered_glyphs.push(glyph);
            decisions.push(NativeStemSeedDecision::Checked {
                gate,
                threshold: STEM_SEEDS_MINIMUM_GRADE,
                weights: stem_check_weights(raw_system.profile),
                check: Box::new(check),
                registered_glyph_index,
                accepted,
                free_glyph_index,
            });
        }
        systems.push(NativeStemSeedSystemRecognition {
            raw: raw_system,
            decisions,
            registered_glyphs,
            free_glyphs,
        });
    }

    Ok(NativeStemSeedRecognition {
        maximum_stem_thickness: raw.maximum_stem_thickness,
        systems,
    })
}

fn stem_check_weights(profile: i32) -> NativeStemImpacts {
    NativeStemImpacts {
        slope: 1.0,
        straight: 1.0,
        length: 2.0,
        clean: if profile >= 4 { -1.0 } else { 2.0 },
        black: 1.0,
        black_ratio: 1.0,
        gap: 5.0,
    }
}

fn closest_system_staff(
    grid: &GridLinesRecognition,
    staff_ids: &[usize],
    x: f64,
    y: f64,
) -> Option<usize> {
    let mut best = None;
    let mut best_distance = f64::MAX;
    for &staff_id in staff_ids {
        let Some(area) = grid
            .staff_areas
            .iter()
            .find(|area| area.staff_id == staff_id)
        else {
            continue;
        };
        if !area.area.contains(x, y) {
            continue;
        }
        let Some(lines) = grid
            .staff_lines
            .iter()
            .find(|lines| lines.staff_id == staff_id)
        else {
            continue;
        };
        // `Staff.distanceTo` truncates the double distance toward zero.
        let distance = (lines.first_line.y_at_x_ext(x) - y)
            .max(y - lines.last_line.y_at_x_ext(x))
            .trunc();
        if distance < best_distance {
            best_distance = distance;
            best = Some(staff_id);
        }
    }
    best
}

fn materialize_stem_glyph(
    candidate: &IdentifiedVerticalStick,
    source_ordinal: usize,
    geometry: audiveris_image::stick_factory::StraightStickGeometry,
) -> Result<NativeStemSeedGlyph, RunTableError> {
    let bounds = geometry.bounds;
    let mut pixels = vec![
        BACKGROUND;
        bounds
            .width
            .checked_mul(bounds.height)
            .ok_or(RunTableError::InvalidDimensions)?
    ];
    for section in candidate.sections() {
        for (offset, run) in section.runs().iter().enumerate() {
            let position = section.first_pos() + offset;
            for coordinate in run.start..=run.stop() {
                let (x, y) = match section.orientation() {
                    Orientation::Horizontal => (coordinate, position),
                    Orientation::Vertical => (position, coordinate),
                };
                let relative_x = x.checked_sub(bounds.x).ok_or(RunTableError::OutOfBounds)?;
                let relative_y = y.checked_sub(bounds.y).ok_or(RunTableError::OutOfBounds)?;
                let index = relative_y
                    .checked_mul(bounds.width)
                    .and_then(|row| row.checked_add(relative_x))
                    .filter(|&index| index < pixels.len())
                    .ok_or(RunTableError::OutOfBounds)?;
                pixels[index] = FOREGROUND;
            }
        }
    }
    let orientation = if bounds.width > bounds.height {
        Orientation::Horizontal
    } else {
        Orientation::Vertical
    };
    let run_table = RunTable::from_pixels(orientation, bounds.width, bounds.height, &pixels)?;
    Ok(NativeStemSeedGlyph {
        source_ordinal,
        vertical_seed_group: false,
        free: false,
        bounds,
        weight: candidate.weight(),
        start: geometry.start,
        stop: geometry.stop,
        mean_thickness: geometry.mean_thickness,
        mean_distance: geometry.mean_distance,
        run_table,
    })
}

/// Run native `STEM_SEEDS` through raw `retrieveCandidates()` from a completed
/// native GRID result.
pub fn recognize_raw_stem_seed_candidates(
    grid: &GridLinesRecognition,
) -> Result<RawStemSeedRecognition, RawStemSeedRecognitionError> {
    let interline = grid.scale.scale.interline.main;
    let interline_usize = usize::try_from(interline)
        .map_err(|_| RawStemSeedRecognitionError::InvalidInterline(interline))?;
    if interline_usize == 0 {
        return Err(RawStemSeedRecognitionError::InvalidInterline(interline));
    }

    // `StemScaler.getBuffer` cleans a private copy. Across the graded corpus
    // its measured mode is unchanged from NO_STAFF, so no downstream state is
    // smuggled in here.
    let pixels = grid.no_staff.to_pixels();
    let horizontal = RunTable::from_pixels(
        Orientation::Horizontal,
        grid.scale.width,
        grid.scale.height,
        &pixels,
    )?;
    let lengths = (0..horizontal.sequence_count())
        .filter_map(|index| horizontal.sequence(index))
        .flat_map(|runs| runs.iter().map(|run| run.length as i32))
        .collect::<Vec<_>>();
    let line = grid.scale.scale.line;
    let stem = compute_stem_scale(
        &lengths,
        StemScaleComputation {
            interline,
            foreground_main: line.main,
            foreground_maximum: line.max,
            minimum_value_ratio: 0.1,
            minimum_derivative_ratio: 0.05,
            minimum_gain_ratio: 0.1,
            stem_as_foreground_ratio: 1.0,
        },
    );
    let maximum_stem = usize::try_from(stem.maximum)
        .map_err(|_| RawStemSeedRecognitionError::InvalidMaximumStem(stem.maximum))?;
    if maximum_stem == 0 {
        return Err(RawStemSeedRecognitionError::InvalidMaximumStem(
            stem.maximum,
        ));
    }
    let minimum_core =
        (STEM_SEEDS_MINIMUM_CORE_RATIO * f64::from(interline)).round_ties_even() as i32;
    let minimum_core_usize = usize::try_from(minimum_core)
        .map_err(|_| RawStemSeedRecognitionError::InvalidInterline(interline))?;

    let mut systems = Vec::with_capacity(grid.peak_graph.systems.len());
    for system_id in 1..=grid.peak_graph.systems.len() {
        let bounds = grid
            .system_bounds
            .iter()
            .find(|bounds| bounds.system_id == system_id)
            .ok_or(RawStemSeedRecognitionError::MissingSystemBounds(system_id))?;
        let area = grid
            .system_areas
            .iter()
            .find(|area| area.system_id == system_id)
            .ok_or(RawStemSeedRecognitionError::MissingSystemArea(system_id))?;
        // `SystemInfo.updateCoordinates` stores `width = last - left + 1`,
        // while `getRight()` returns `left + width`: one pixel beyond the
        // last staff abscissa. `VerticalsBuilder` calls that getter directly.
        let right = bounds.java_right();
        let vertical_sections = select_sections(
            bounds.left,
            right,
            area,
            &grid.peak_graph.vertical_sections,
            false,
        );
        let horizontal_stickers = select_sections(
            bounds.left,
            right,
            area,
            &grid.peak_graph.horizontal_sections,
            true,
        );
        let outcome = VerticalStickFactory::new(VerticalStickParameters {
            interline: interline_usize,
            maximum_stick_thickness: maximum_stem,
            minimum_core_section_length: minimum_core_usize,
            minimum_side_ratio: STEM_SEEDS_MINIMUM_SIDE_RATIO,
        })
        .retrieve_sticks(&vertical_sections, &horizontal_stickers, 1);
        if let Some(source) = outcome.error {
            return Err(RawStemSeedRecognitionError::StickFactory { system_id, source });
        }
        systems.push(RawStemSeedSystem {
            system_id,
            profile: DEFAULT_STEM_SEEDS_PROFILE,
            left: bounds.left,
            right,
            interline,
            maximum_stem_thickness: stem.maximum,
            minimum_core_section_length: minimum_core,
            minimum_side_ratio: STEM_SEEDS_MINIMUM_SIDE_RATIO,
            vertical_sections,
            horizontal_stickers,
            candidates: outcome.result.survivors().to_vec(),
        });
    }

    Ok(RawStemSeedRecognition {
        maximum_stem_thickness: stem.maximum,
        systems,
    })
}

fn select_sections(
    left: i32,
    right: i32,
    area: &PopulationSystemArea,
    sections: &[Section],
    sticker: bool,
) -> Vec<Section> {
    let mut selected = Vec::new();
    for section in sections {
        let (centroid_x, centroid_y) = section.centroid();
        if !contains_section_centroid(area, centroid_x as f64, centroid_y as f64) {
            continue;
        }
        let box_ = section.bounds();
        let center_x = box_.x + (box_.width / 2);
        let within =
            i32::try_from(center_x).is_ok_and(|center_x| center_x > left && center_x < right);
        if within && (!sticker || section.length(Orientation::Horizontal) == 1) {
            selected.push(section.clone());
        }
    }
    selected
}

fn contains_section_centroid(area: &PopulationSystemArea, x: f64, y: f64) -> bool {
    if area.contains(x, y) {
        return true;
    }
    // `Area.contains(Point)` includes the north edge. Java's `Area` evaluates
    // the curved path through its own curve crossing code, whereas the native
    // x-monotone evaluator asks the source spline directly; the two can land
    // on opposite sides by a few ulps at an integer centroid. Admit only that
    // north-edge rounding seam, never a full pixel above it or the exclusive
    // south/right edges.
    if x < f64::from(area.left) || x >= f64::from(area.right) {
        return false;
    }
    let (Some(north), Some(south)) = (
        area.north().geopath_y_at_x(x),
        area.south().geopath_y_at_x(x),
    ) else {
        return false;
    };
    y < south && (north - y).abs() <= 1.0e-9
}
