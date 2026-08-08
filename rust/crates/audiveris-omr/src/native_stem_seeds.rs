// SPDX-License-Identifier: AGPL-3.0-or-later

//! Native raw `STEM_SEEDS` candidate construction.
//!
//! This stops at Java `VerticalsBuilder.retrieveCandidates()`: GRID's live
//! section lags are selected per system and passed to vertical `StickFactory`.
//! Stem checking, glyph materialization and SIG/free-glyph ownership are the
//! next seam.

use std::{error::Error, fmt};

use audiveris_image::{
    run_table::{Orientation, RunTable, RunTableError},
    section::Section,
    stick_factory::{
        IdentifiedVerticalStick, StraightStickError, VerticalStickFactory, VerticalStickParameters,
    },
    system_population::PopulationSystemArea,
};

use crate::{
    recognize::GridLinesRecognition,
    stem_seeds_step::{StemScaleComputation, compute_stem_scale},
};

/// Java's default `SystemInfo.getProfile()` during batch recognition.
pub const DEFAULT_STEM_SEEDS_PROFILE: i32 = 1;
pub const STEM_SEEDS_MINIMUM_SIDE_RATIO: f64 = 0.4;
pub const STEM_SEEDS_MINIMUM_CORE_RATIO: f64 = 1.5;

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
        let right = bounds.right.saturating_add(1);
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
