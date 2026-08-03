// SPDX-License-Identifier: AGPL-3.0-or-later

//! Signed-short projection storage used by Java `grid.StaffProjector`.
//!
//! Staff projection counts are stored in Java `short` cells. Increments narrow
//! with two's-complement wrapping, while reads and derivatives widen to `int`.

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::{error::Error, fmt};

use crate::bar_column::StaffId;
use crate::run_table::FOREGROUND;
use crate::staff_peak::{HorizontalSide, StaffPeak, StaffPeakError, StaffVerticalImpacts};

const MINIMUM_STAFF_PEAK_GRADE: f64 = 0.08;

/// Java `StaffProjector.Blank`: an inclusive region without staff lines.
#[derive(Clone, Copy, Debug)]
pub struct ProjectionBlank {
    start: i32,
    stop: i32,
}

/// Scale-resolved inputs consumed by Java `StaffProjector.refinePeakSide`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeakRefinementParams {
    bar_threshold: i32,
    lines_threshold: i32,
    chunk_threshold: i32,
    refine_dx: i32,
    chunk_width: i32,
}

/// Per-side controls supplied by Java `createPeak` to `refinePeakSide`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeakRefinementRequest {
    pub x_start: i32,
    pub x_stop: i32,
    pub direction: i32,
    pub half_mode: bool,
    pub minimum_derivative: i32,
    pub added_chunk: i32,
}

impl PeakRefinementRequest {
    #[must_use]
    pub const fn new(
        x_start: i32,
        x_stop: i32,
        direction: i32,
        half_mode: bool,
        minimum_derivative: i32,
        added_chunk: i32,
    ) -> Self {
        Self {
            x_start,
            x_stop,
            direction,
            half_mode,
            minimum_derivative,
            added_chunk,
        }
    }
}

impl PeakRefinementParams {
    pub fn new(
        bar_threshold: i32,
        lines_threshold: i32,
        chunk_threshold: i32,
        refine_dx: i32,
        chunk_width: i32,
    ) -> Result<Self, ProjectionError> {
        if refine_dx < 0 || chunk_width <= 0 {
            return Err(ProjectionError::InvalidRefinementParameters);
        }
        Ok(Self {
            bar_threshold,
            lines_threshold,
            chunk_threshold,
            refine_dx,
            chunk_width,
        })
    }
}

/// Java `StaffProjector.PeakSide` numeric result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PeakSide {
    pub abscissa: i32,
    pub derivative_grade: f64,
    pub chunk_grade: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeakSearchBounds {
    pub x_min: i32,
    pub x_max: i32,
}

/// Sheet-resolved horizontal inputs to Java
/// `StaffProjector.computeProjection`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaffProjectionRequest {
    pub staff_left: i32,
    pub staff_right: i32,
    pub staff_abscissa_margin: i32,
}

impl StaffProjectionRequest {
    #[must_use]
    pub const fn new(staff_left: i32, staff_right: i32, staff_abscissa_margin: i32) -> Self {
        Self {
            staff_left,
            staff_right,
            staff_abscissa_margin,
        }
    }
}

/// Pure raster result of Java `StaffProjector.computeProjection`, before its
/// derivative threshold is computed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaffProjectionAccumulation {
    pub projection: ShortProjection,
    pub bounds: PeakSearchBounds,
}

/// Scale- and staff-resolved controls for the neutral portion of Java
/// `StaffProjector.process` plus optional brace discovery.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NeutralStaffProjectorRequest {
    pub staff_id: StaffId,
    pub staff_left: i32,
    pub staff_right: i32,
    pub blank_threshold: i32,
    pub minimum_wide_blank_width: i32,
    pub top_derivative_count: usize,
    pub minimum_derivative_ratio: f64,
    pub use_one_line_half_mode: bool,
    pub is_one_line_staff: bool,
    pub bar_threshold: i32,
    pub total_height: i32,
    pub peak_construction: PeakConstructionParams,
    pub peak_core: PeakCoreParams,
    pub brace_search: Option<BraceSearchRequest>,
}

/// Source-faithful projector state before any `Sheet`, `Staff`, `SystemInfo`,
/// peak-graph, attribute, or deskew mutation.
#[derive(Clone, Debug, PartialEq)]
pub struct NeutralStaffProjectorResult {
    pub projection: ShortProjection,
    pub derivative_threshold: i32,
    pub all_blanks: Vec<ProjectionBlank>,
    pub peak_search_bounds: PeakSearchBounds,
    pub peaks: Vec<StaffPeak>,
    pub brace_candidate: Option<ProjectionBraceCandidate>,
}

/// Mutation-free decision produced by Java `StaffProjector.checkLinesRoot`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinesRootTransition {
    pub staff_left: i32,
    pub clear_staff_left_end_at: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeakConstructionParams {
    refinement: PeakRefinementParams,
    maximum_bar_width: i32,
}

impl PeakConstructionParams {
    pub fn new(
        refinement: PeakRefinementParams,
        maximum_bar_width: i32,
    ) -> Result<Self, ProjectionError> {
        if maximum_bar_width <= 0 {
            return Err(ProjectionError::InvalidMaximumBarWidth(maximum_bar_width));
        }
        Ok(Self {
            refinement,
            maximum_bar_width,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeakConstructionRequest {
    pub raw_start: i32,
    pub raw_stop: i32,
    pub half_mode: bool,
    pub minimum_derivative_up: i32,
    pub minimum_derivative_down: i32,
    pub added_chunk: i32,
}

/// Java `StaffProjector.PeakMode` controls whether the numeric scan uses
/// full-height or half-height peak evidence. `InitialHalf` currently has the
/// same behavior as `Half`, matching the disabled mode-reset block in Java.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionPeakMode {
    Full,
    InitialHalf,
    Half,
}

impl ProjectionPeakMode {
    const fn is_half(self) -> bool {
        matches!(self, Self::InitialHalf | Self::Half)
    }
}

/// Controls Java `StaffProjector.browseRange` without transferring ownership
/// of its sheet or staff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeakRangeRequest {
    pub range_start: i32,
    pub range_stop: i32,
    pub half_mode: bool,
    pub minimum_derivative_up: i32,
    pub minimum_derivative_down: i32,
    pub added_chunk: i32,
}

impl PeakRangeRequest {
    #[must_use]
    pub const fn new(
        range_start: i32,
        range_stop: i32,
        half_mode: bool,
        minimum_derivative_up: i32,
        minimum_derivative_down: i32,
        added_chunk: i32,
    ) -> Self {
        Self {
            range_start,
            range_stop,
            half_mode,
            minimum_derivative_up,
            minimum_derivative_down,
            added_chunk,
        }
    }
}

/// Controls the pure portion of Java `StaffProjector.findPeaksInRange`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeakScanRequest {
    pub x_min: i32,
    pub x_max: i32,
    pub mode: ProjectionPeakMode,
    pub minimum_count: i32,
    pub minimum_derivative_up: i32,
    pub minimum_derivative_down: i32,
    pub added_chunk: i32,
}

/// Inputs to Java `StaffProjector.findBracePeak` after scale resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BraceSearchRequest {
    pub staff_left: i32,
    pub minimum_left: i32,
    pub maximum_right: i32,
    pub minimum_wide_blank_width: i32,
    pub minimum_value: i32,
}

impl BraceSearchRequest {
    #[must_use]
    pub const fn new(
        staff_left: i32,
        minimum_left: i32,
        maximum_right: i32,
        minimum_wide_blank_width: i32,
        minimum_value: i32,
    ) -> Self {
        Self {
            staff_left,
            minimum_left,
            maximum_right,
            minimum_wide_blank_width,
            minimum_value,
        }
    }
}

/// Projection-only result of Java `findBracePeak`/`createBracePeak`, before
/// staff-line ordinates and sheet deskew are consulted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectionBraceCandidate {
    pub raw_start: i32,
    pub raw_stop: i32,
    pub start: i32,
    pub stop: i32,
    pub search_right: i32,
}

impl ProjectionBraceCandidate {
    /// Resolve the remaining source-owned geometry and create Java's neutral
    /// `StaffPeak` with only its `BRACE` attribute set.
    pub fn into_staff_peak(
        self,
        staff_id: StaffId,
        ordinates_at: impl FnOnce(i32) -> (i32, i32),
        deskew: impl FnOnce(crate::staff_peak::PeakPoint) -> crate::staff_peak::PeakPoint,
    ) -> Result<StaffPeak, ProjectionError> {
        // Java uses an integer midpoint for line ordinates, then StaffPeak uses
        // a precise half-pixel midpoint for deskewing.
        let ordinate_x = self.start.wrapping_add(self.stop) / 2;
        let (top, bottom) = ordinates_at(ordinate_x);
        let mut peak = StaffPeak::new(staff_id, top, bottom, self.start, self.stop)
            .map_err(ProjectionError::StaffPeak)?;
        peak.set(crate::staff_peak::StaffPeakAttribute::Brace);
        peak.compute_deskewed_center(deskew)
            .map_err(ProjectionError::StaffPeak)?;
        Ok(peak)
    }
}

impl PeakScanRequest {
    #[must_use]
    pub const fn new(
        x_min: i32,
        x_max: i32,
        mode: ProjectionPeakMode,
        minimum_count: i32,
        minimum_derivative_up: i32,
        minimum_derivative_down: i32,
        added_chunk: i32,
    ) -> Self {
        Self {
            x_min,
            x_max,
            mode,
            minimum_count,
            minimum_derivative_up,
            minimum_derivative_down,
            added_chunk,
        }
    }
}

impl PeakConstructionRequest {
    #[must_use]
    pub const fn new(
        raw_start: i32,
        raw_stop: i32,
        half_mode: bool,
        minimum_derivative_up: i32,
        minimum_derivative_down: i32,
        added_chunk: i32,
    ) -> Self {
        Self {
            raw_start,
            raw_stop,
            half_mode,
            minimum_derivative_up,
            minimum_derivative_down,
            added_chunk,
        }
    }
}

/// Numeric front half of Java `StaffProjector.createPeak`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectionPeakCandidate {
    pub raw_start: i32,
    pub raw_stop: i32,
    pub start: i32,
    pub stop: i32,
    pub maximum_value: i32,
    pub left: PeakSide,
    pub right: PeakSide,
}

/// Resolved staff ordinates at the peak midpoint, supplied without transferring
/// ownership of Java's `Staff` or line geometry into this numeric kernel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeakCoreGeometry {
    pub y_top: i32,
    pub y_bottom: i32,
    pub y_mid: i32,
}

impl PeakCoreGeometry {
    #[must_use]
    pub const fn new(y_top: i32, y_bottom: i32, y_mid: i32) -> Self {
        Self {
            y_top,
            y_bottom,
            y_mid,
        }
    }
}

/// Scale-resolved acceptance thresholds used after Java `createPeak` has
/// refined the two horizontal sides.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PeakCoreParams {
    gap_threshold: i32,
    minimum_white_ratio_beyond_serif: f64,
}

/// Resolved projection values used by the last pure portion of Java
/// `StaffProjector.createPeak`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeakGradeParams {
    pub bar_threshold: i32,
    pub total_height: i32,
    pub half_mode: bool,
}

impl PeakGradeParams {
    #[must_use]
    pub const fn new(bar_threshold: i32, total_height: i32, half_mode: bool) -> Self {
        Self {
            bar_threshold,
            total_height,
            half_mode,
        }
    }
}

impl PeakCoreParams {
    pub fn new(
        gap_threshold: i32,
        minimum_white_ratio_beyond_serif: f64,
    ) -> Result<Self, ProjectionError> {
        if gap_threshold < 0
            || !minimum_white_ratio_beyond_serif.is_finite()
            || !(0.0..=1.0).contains(&minimum_white_ratio_beyond_serif)
        {
            return Err(ProjectionError::InvalidCoreParameters);
        }
        Ok(Self {
            gap_threshold,
            minimum_white_ratio_beyond_serif,
        })
    }
}

/// Java `AreaUtil.CoreData` for an axis-aligned vertical probe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VerticalCoreData {
    pub length: i32,
    pub gap: i32,
    pub white_ratio: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeakCoreRejection {
    GapTooLarge,
    InsufficientWhiteBeyondSerif,
}

/// Pixel evidence and the bounded accept/reject decision immediately following
/// peak construction in Java `StaffProjector.createPeak`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PeakCoreValidation {
    pub start: i32,
    pub stop: i32,
    pub y_top: i32,
    pub y_bottom: i32,
    pub gap_threshold: i32,
    pub core: VerticalCoreData,
    pub full_height_core: Option<VerticalCoreData>,
    pub rejection: Option<PeakCoreRejection>,
}

impl PeakCoreValidation {
    #[must_use]
    pub const fn is_accepted(self) -> bool {
        self.rejection.is_none()
    }
}

impl ProjectionPeakCandidate {
    /// Pixel/core validation from the middle of Java
    /// `StaffProjector.createPeak`.
    ///
    /// `pixels` uses the same row-major zero-is-foreground convention as
    /// [`crate::ingest::GrayRaster::pixels`] and [`crate::run_table::RunTable::to_pixels`].
    /// The caller resolves the three staff-line ordinates at the candidate's
    /// midpoint; no sheet, staff, graph, or score mutation crosses this boundary.
    pub fn validate_core(
        self,
        raster_width: usize,
        raster_height: usize,
        pixels: &[u8],
        geometry: PeakCoreGeometry,
        added_chunk: i32,
        params: PeakCoreParams,
    ) -> Result<PeakCoreValidation, ProjectionError> {
        let expected = raster_width.checked_mul(raster_height).ok_or(
            ProjectionError::InvalidRasterDimensions {
                width: raster_width,
                height: raster_height,
            },
        )?;
        if raster_width == 0 || raster_height == 0 {
            return Err(ProjectionError::InvalidRasterDimensions {
                width: raster_width,
                height: raster_height,
            });
        }
        if pixels.len() != expected {
            return Err(ProjectionError::InvalidRasterPixels {
                expected,
                actual: pixels.len(),
            });
        }
        if geometry.y_top > geometry.y_bottom
            || (added_chunk != 0
                && (geometry.y_mid < geometry.y_top || geometry.y_mid > geometry.y_bottom))
        {
            return Err(ProjectionError::InvalidCoreGeometry(geometry));
        }

        let width = self.stop.wrapping_sub(self.start).wrapping_add(1);
        let dx = i32::from(width <= 2);
        let x_min = self.start.wrapping_sub(dx);
        let x_max = self.stop.wrapping_add(dx);

        let full_height_core = if added_chunk != 0 {
            Some(vertical_core_data(
                raster_width,
                raster_height,
                pixels,
                x_min,
                x_max,
                geometry.y_top,
                geometry.y_bottom,
            )?)
        } else {
            None
        };

        let (y_top, y_bottom) = if added_chunk != 0 {
            (
                geometry
                    .y_top
                    .wrapping_add(geometry.y_mid.wrapping_sub(geometry.y_top) / 2),
                geometry
                    .y_bottom
                    .wrapping_sub(geometry.y_bottom.wrapping_sub(geometry.y_mid) / 2),
            )
        } else {
            (geometry.y_top, geometry.y_bottom)
        };
        let core = vertical_core_data(
            raster_width,
            raster_height,
            pixels,
            x_min,
            x_max,
            y_top,
            y_bottom,
        )?;

        let rejection = if core.gap > params.gap_threshold {
            Some(PeakCoreRejection::GapTooLarge)
        } else if full_height_core
            .is_some_and(|data| data.white_ratio < params.minimum_white_ratio_beyond_serif)
        {
            Some(PeakCoreRejection::InsufficientWhiteBeyondSerif)
        } else {
            None
        };

        Ok(PeakCoreValidation {
            start: self.start,
            stop: self.stop,
            y_top,
            y_bottom,
            gap_threshold: params.gap_threshold,
            core,
            full_height_core,
            rejection,
        })
    }

    /// Finish the pure `StaffProjector.createPeak` decision by computing the
    /// six Java staff-vertical impacts and constructing a neutral `StaffPeak`.
    ///
    /// A rejected core or a grade below `Grades.minInterGrade` (`0.08`) returns
    /// `None`. The returned peak deliberately has no semantic attributes and no
    /// deskewed center: those are assigned later by source stages that own the
    /// sheet transform and peak graph.
    pub fn into_staff_peak(
        self,
        validation: PeakCoreValidation,
        staff_id: StaffId,
        params: PeakGradeParams,
    ) -> Result<Option<StaffPeak>, ProjectionError> {
        if self.start != validation.start || self.stop != validation.stop {
            return Err(ProjectionError::MismatchedCoreValidation);
        }
        if !validation.is_accepted() {
            return Ok(None);
        }

        let minimum_value = if params.half_mode {
            params.bar_threshold / 2
        } else {
            params.bar_threshold
        };
        let effective_height = if params.half_mode {
            params.total_height / 2
        } else {
            params.total_height
        };
        let value_range = effective_height.wrapping_sub(minimum_value);
        let core_impact =
            f64::from(self.maximum_value.wrapping_sub(minimum_value)) / f64::from(value_range);
        let gap_impact =
            1.0 - (f64::from(validation.core.gap) / f64::from(validation.gap_threshold));
        let impacts = StaffVerticalImpacts::new(
            core_impact,
            gap_impact,
            self.left.derivative_grade,
            self.right.derivative_grade,
            self.left.chunk_grade,
            self.right.chunk_grade,
        );
        if impacts.grade() < MINIMUM_STAFF_PEAK_GRADE || impacts.grade().is_nan() {
            return Ok(None);
        }

        StaffPeak::with_impacts(
            staff_id,
            validation.y_top,
            validation.y_bottom,
            self.start,
            self.stop,
            impacts,
        )
        .map(Some)
        .map_err(ProjectionError::StaffPeak)
    }
}

/// Axis-aligned specialization of Java `AreaUtil.verticalCore` used by
/// `StaffProjector`: a row is black if any pixel in the inclusive x span is
/// foreground. Leading and trailing white rows contribute to `white_ratio` but
/// not to the largest enclosed gap, matching the source implementation.
fn vertical_core_data(
    raster_width: usize,
    raster_height: usize,
    pixels: &[u8],
    x_min: i32,
    x_max: i32,
    y_min: i32,
    y_max: i32,
) -> Result<VerticalCoreData, ProjectionError> {
    let in_bounds = |x: i32, y: i32| {
        x >= 0
            && y >= 0
            && usize::try_from(x).is_ok_and(|x| x < raster_width)
            && usize::try_from(y).is_ok_and(|y| y < raster_height)
    };
    if x_min > x_max || y_min > y_max || !in_bounds(x_min, y_min) || !in_bounds(x_max, y_max) {
        return Err(ProjectionError::CoreProbeOutOfBounds {
            x_min,
            x_max,
            y_min,
            y_max,
        });
    }

    let mut largest_gap = 0;
    let mut last_black_y = -1;
    let mut last_white_y = -1;
    let mut white_count = 0;

    for y in y_min..=y_max {
        let y_index = usize::try_from(y).expect("validated nonnegative y");
        let row = y_index * raster_width;
        let empty = (x_min..=x_max).all(|x| {
            let x_index = usize::try_from(x).expect("validated nonnegative x");
            pixels[row + x_index] != FOREGROUND
        });
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
    Ok(VerticalCoreData {
        length,
        gap: largest_gap,
        white_ratio: f64::from(white_count) / f64::from(length),
    })
}

impl ProjectionBlank {
    #[must_use]
    pub const fn start(self) -> i32 {
        self.start
    }

    #[must_use]
    pub const fn stop(self) -> i32 {
        self.stop
    }

    /// Java's inclusive `stop - start + 1` arithmetic.
    #[must_use]
    pub const fn width(self) -> i32 {
        self.stop.wrapping_sub(self.start).wrapping_add(1)
    }

    const fn midpoint(self) -> i32 {
        self.start.wrapping_add(self.stop) / 2
    }
}

impl PartialEq for ProjectionBlank {
    fn eq(&self, other: &Self) -> bool {
        // Java Blank equality and ordering intentionally use start only.
        self.start == other.start
    }
}

impl Eq for ProjectionBlank {}

impl PartialOrd for ProjectionBlank {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ProjectionBlank {
    fn cmp(&self, other: &Self) -> Ordering {
        self.start.cmp(&other.start)
    }
}

impl Hash for ProjectionBlank {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.start.hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortProjection {
    start: i32,
    stop: i32,
    values: Vec<i16>,
}

impl ShortProjection {
    pub fn new(start: i32, stop: i32) -> Result<Self, ProjectionError> {
        if stop < start {
            return Err(ProjectionError::InvalidDomain { start, stop });
        }
        let length = i64::from(stop) - i64::from(start) + 1;
        let length =
            usize::try_from(length).map_err(|_| ProjectionError::InvalidDomain { start, stop })?;
        Ok(Self {
            start,
            stop,
            values: vec![0; length],
        })
    }

    /// Accumulate the foreground pixels between the first and last staff-line
    /// ordinates, exactly as Java `StaffProjector.computeProjection` does.
    ///
    /// The callbacks own staff-specific geometry. For a one-line staff, the
    /// caller supplies the already translated top and bottom line ordinates.
    /// Pixels are row-major and zero denotes foreground.
    pub fn from_staff_raster<First, Last>(
        raster_width: usize,
        raster_height: usize,
        pixels: &[u8],
        request: StaffProjectionRequest,
        mut first_ordinate_at: First,
        mut last_ordinate_at: Last,
    ) -> Result<StaffProjectionAccumulation, ProjectionError>
    where
        First: FnMut(i32) -> i32,
        Last: FnMut(i32) -> i32,
    {
        let expected = raster_width.checked_mul(raster_height).ok_or(
            ProjectionError::InvalidRasterDimensions {
                width: raster_width,
                height: raster_height,
            },
        )?;
        let width =
            i32::try_from(raster_width).map_err(|_| ProjectionError::InvalidRasterDimensions {
                width: raster_width,
                height: raster_height,
            })?;
        let height =
            i32::try_from(raster_height).map_err(|_| ProjectionError::InvalidRasterDimensions {
                width: raster_width,
                height: raster_height,
            })?;
        if width == 0 || height == 0 {
            return Err(ProjectionError::InvalidRasterDimensions {
                width: raster_width,
                height: raster_height,
            });
        }
        if pixels.len() != expected {
            return Err(ProjectionError::InvalidRasterPixels {
                expected,
                actual: pixels.len(),
            });
        }

        let last_x = width - 1;
        let last_y = height - 1;
        let x_min = request
            .staff_left
            .wrapping_sub(request.staff_abscissa_margin)
            .clamp(0, last_x);
        let x_max = request
            .staff_right
            .wrapping_add(request.staff_abscissa_margin)
            .clamp(0, last_x);
        let mut projection = Self::new(0, last_x)?;

        if x_min <= x_max {
            for x in x_min..=x_max {
                let y_min = first_ordinate_at(x).clamp(0, last_y);
                let y_max = last_ordinate_at(x).wrapping_sub(1).clamp(0, last_y);
                let mut count = 0_i16;
                if y_min <= y_max {
                    for y in y_min..=y_max {
                        let index = y as usize * raster_width + x as usize;
                        if pixels[index] == FOREGROUND {
                            count = count.wrapping_add(1);
                        }
                    }
                }
                projection.increment(x, i32::from(count));
            }
        }

        Ok(StaffProjectionAccumulation {
            projection,
            bounds: PeakSearchBounds { x_min, x_max },
        })
    }

    #[must_use]
    pub const fn start(&self) -> i32 {
        self.start
    }

    #[must_use]
    pub const fn stop(&self) -> i32 {
        self.stop
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    #[must_use]
    pub fn value(&self, position: i32) -> i32 {
        i32::from(self.values[self.index(position)])
    }

    /// Java `Projection.Short.increment(pos)`.
    pub fn increment_one(&mut self, position: i32) {
        self.increment(position, 1);
    }

    /// Java narrows the wrapped `int` sum back to a signed `short`.
    pub fn increment(&mut self, position: i32, increment: i32) {
        let index = self.index(position);
        let sum = i32::from(self.values[index]).wrapping_add(increment);
        self.values[index] = sum as i16;
    }

    /// Java returns zero at (and even below) the projection start.
    #[must_use]
    pub fn derivative(&self, position: i32) -> i32 {
        if position <= self.start {
            0
        } else {
            self.value(position) - self.value(position - 1)
        }
    }

    /// Adaptive threshold from Java `StaffProjector.computeProjection`.
    ///
    /// Derivatives are observed after `x_min`, the largest `top_count` absolute
    /// values are averaged, then scaled and rounded with Java `Math.rint`.
    pub fn staff_derivative_threshold(
        &self,
        x_min: i32,
        x_max: i32,
        top_count: usize,
        minimum_ratio: f64,
    ) -> Result<i32, ProjectionError> {
        if x_min < self.start || x_max > self.stop || x_min > x_max {
            return Err(ProjectionError::InvalidDerivativeRange { x_min, x_max });
        }
        if top_count == 0 {
            // Java computes rint(0.0 / 0 * ratio), then narrows NaN to zero.
            return Ok(0);
        }

        let mut derivatives = Vec::new();
        let mut position = x_min;
        while position < x_max {
            position += 1;
            derivatives.push(self.derivative(position).abs());
        }
        if derivatives.len() < top_count {
            return Err(ProjectionError::InsufficientDerivativeSamples {
                available: derivatives.len(),
                required: top_count,
            });
        }

        derivatives.sort_unstable();
        let cumulative = derivatives
            .iter()
            .rev()
            .take(top_count)
            .fold(0_i32, |sum, derivative| sum.wrapping_add(*derivative));
        let elite = f64::from(cumulative) / top_count as f64;
        Ok((elite * minimum_ratio).round_ties_even() as i32)
    }

    /// Java `StaffProjector.findAllBlanks` over the projection domain.
    #[must_use]
    pub fn blank_regions(&self, maximum_value: i32) -> Vec<ProjectionBlank> {
        let mut blanks = Vec::new();
        let mut active_start = None;

        for position in self.start..=self.stop {
            if self.value(position) <= maximum_value {
                active_start.get_or_insert(position);
            } else if let Some(start) = active_start.take() {
                blanks.push(ProjectionBlank {
                    start,
                    stop: position - 1,
                });
            }
        }
        if let Some(start) = active_start {
            blanks.push(ProjectionBlank {
                start,
                stop: self.stop,
            });
        }
        blanks
    }

    /// Java `findPeaks` window after `selectEndingBlanks`.
    #[must_use]
    pub fn peak_search_bounds(
        &self,
        blanks: &[ProjectionBlank],
        staff_left: i32,
        staff_right: i32,
        minimum_wide_blank_width: i32,
    ) -> PeakSearchBounds {
        let left = select_blank(
            blanks,
            HorizontalSide::Left,
            staff_left,
            minimum_wide_blank_width,
        );
        let right = select_blank(
            blanks,
            HorizontalSide::Right,
            staff_right,
            minimum_wide_blank_width,
        );
        PeakSearchBounds {
            x_min: left.map_or(self.start, ProjectionBlank::stop),
            x_max: right.map_or(self.stop, ProjectionBlank::start),
        }
    }

    /// Projection-only portion of Java `StaffProjector.findBracePeak` and
    /// `createBracePeak`.
    ///
    /// The scan moves right-to-left, requires a below-threshold valley before
    /// accepting brace ink, and then refines the candidate against neighboring
    /// blanks and the first minimum toward the bar. Staff-line geometry,
    /// attributes, and deskewing remain in [`ProjectionBraceCandidate`].
    pub fn find_brace_candidate(
        &self,
        blanks: &[ProjectionBlank],
        request: BraceSearchRequest,
    ) -> Result<Option<ProjectionBraceCandidate>, ProjectionError> {
        let left_ending_blank = select_blank(
            blanks,
            HorizontalSide::Left,
            request.staff_left,
            request.minimum_wide_blank_width,
        );
        let mut maximum_right = request.maximum_right;
        let x_min = if let Some(left_blank) = left_ending_blank {
            if left_blank.stop.wrapping_add(2) >= maximum_right {
                maximum_right = left_blank.start.wrapping_sub(1);
                select_blank(
                    blanks,
                    HorizontalSide::Left,
                    maximum_right,
                    request.minimum_wide_blank_width,
                )
                .map_or(request.minimum_left, ProjectionBlank::stop)
            } else {
                request.minimum_left.max(left_blank.stop)
            }
        } else {
            request.minimum_left.max(self.start)
        };

        if x_min > maximum_right {
            return Ok(None);
        }
        if x_min < self.start || maximum_right > self.stop {
            return Err(ProjectionError::InvalidBraceSearchRange {
                x_min,
                x_max: maximum_right,
            });
        }

        let mut brace_stop = None;
        let mut brace_start = None;
        let mut valley_hit = false;
        let mut x = maximum_right;
        loop {
            let value = self.value(x);
            if value >= request.minimum_value {
                if valley_hit {
                    brace_stop.get_or_insert(x);
                    brace_start = Some(x);
                }
            } else if !valley_hit {
                valley_hit = true;
            } else if let (Some(raw_start), Some(raw_stop)) = (brace_start, brace_stop) {
                return self.refine_brace_candidate(blanks, raw_start, raw_stop, maximum_right);
            }

            if x == x_min {
                break;
            }
            x = x.wrapping_sub(1);
        }

        if let (Some(raw_start), Some(raw_stop)) = (brace_start, brace_stop)
            && raw_start >= 0
        {
            self.refine_brace_candidate(blanks, raw_start, raw_stop, maximum_right)
        } else {
            Ok(None)
        }
    }

    fn refine_brace_candidate(
        &self,
        blanks: &[ProjectionBlank],
        raw_start: i32,
        raw_stop: i32,
        maximum_right: i32,
    ) -> Result<Option<ProjectionBraceCandidate>, ProjectionError> {
        let mut left_blank = None;
        for &blank in blanks {
            if blank.stop >= raw_start {
                break;
            }
            left_blank = Some(blank);
        }

        let mut start = left_blank.map_or(raw_start, ProjectionBlank::stop);
        if start < self.start
            || start > self.stop
            || raw_stop < self.start
            || maximum_right > self.stop
        {
            return Err(ProjectionError::InvalidBraceRefinementRange {
                start,
                raw_stop,
                maximum_right,
            });
        }
        let mut value = self.value(start);
        while start > self.start {
            let x = start.wrapping_sub(1);
            let next_value = self.value(x);
            if next_value < value {
                value = next_value;
                start = x;
            } else {
                break;
            }
        }

        let mut best_value = i32::MAX;
        let mut stop = None;
        if raw_stop <= maximum_right {
            for x in raw_stop..=maximum_right {
                let value = self.value(x);
                if value < best_value {
                    best_value = value;
                    stop = Some(x);
                }
            }
        }

        Ok(stop.map(|stop| ProjectionBraceCandidate {
            raw_start,
            raw_stop,
            start,
            stop,
            search_right: maximum_right,
        }))
    }

    /// Pure numeric kernel from Java `StaffProjector.refinePeakSide`.
    pub fn refine_peak_side(
        &self,
        request: PeakRefinementRequest,
        params: PeakRefinementParams,
    ) -> Result<Option<PeakSide>, ProjectionError> {
        let PeakRefinementRequest {
            x_start,
            x_stop,
            direction,
            half_mode,
            minimum_derivative,
            added_chunk,
        } = request;
        if direction != -1 && direction != 1 {
            return Err(ProjectionError::InvalidDirection(direction));
        }
        if x_start < self.start || x_stop > self.stop || x_start > x_stop {
            return Err(ProjectionError::InvalidPeakRange { x_start, x_stop });
        }

        let minimum_bar = if half_mode {
            params.bar_threshold / 2
        } else {
            params.bar_threshold
        };
        let minimum_chunk = added_chunk.wrapping_add(params.lines_threshold);
        let maximum_chunk = added_chunk.wrapping_add(params.chunk_threshold);
        let midpoint = f64::from(x_stop.wrapping_add(x_start)) / 2.0;
        let x1 = if direction > 0 {
            midpoint.ceil() as i32
        } else {
            midpoint.floor() as i32
        };
        let x2 = if direction > 0 {
            x_stop.wrapping_add(params.refine_dx)
        } else {
            x_start.wrapping_sub(params.refine_dx)
        }
        .clamp(self.start, self.stop);

        let mut best_derivative = 0_i32;
        let mut best_x = None;
        let mut x = x1;
        while direction.wrapping_mul(x2.wrapping_sub(x)) >= 0 {
            let derivative = self.derivative(x);
            if direction.wrapping_mul(best_derivative.wrapping_sub(derivative)) > 0 {
                best_derivative = derivative;
                best_x = Some(x);
            }
            x = x.wrapping_add(direction);
        }

        best_derivative = best_derivative.abs();
        let derivative_denominator = minimum_bar.wrapping_sub(minimum_derivative);
        if best_derivative >= minimum_derivative
            && let Some(best_x) = best_x
        {
            let abscissa = if direction > 0 {
                best_x.wrapping_sub(1)
            } else {
                best_x
            };
            let derivative_grade = f64::from(best_derivative) / f64::from(derivative_denominator);
            let chunk = self.chunk_minimum(abscissa, direction, params.chunk_width);
            let chunk_grade = if chunk < minimum_chunk {
                1.0
            } else if chunk > maximum_chunk {
                0.0
            } else {
                f64::from(maximum_chunk.wrapping_sub(chunk))
                    / f64::from(maximum_chunk.wrapping_sub(minimum_chunk))
            };
            return Ok(Some(PeakSide {
                abscissa,
                derivative_grade,
                chunk_grade,
            }));
        }

        let border = if direction > 0 { self.stop } else { self.start };
        if x2 == border {
            let derivative = self.value(border);
            if derivative >= minimum_derivative {
                return Ok(Some(PeakSide {
                    abscissa: border,
                    derivative_grade: f64::from(derivative) / f64::from(derivative_denominator),
                    chunk_grade: 1.0,
                }));
            }
        }
        Ok(None)
    }

    /// Numeric front half of Java `StaffProjector.createPeak`.
    ///
    /// Staff-line ordinates, vertical-core pixels, gap/white ratios, composite
    /// impacts, and final grade acceptance intentionally remain outside.
    pub fn construct_peak_candidate(
        &self,
        request: PeakConstructionRequest,
        params: PeakConstructionParams,
    ) -> Result<Option<ProjectionPeakCandidate>, ProjectionError> {
        let left_request = PeakRefinementRequest::new(
            request.raw_start,
            request.raw_stop,
            -1,
            request.half_mode,
            request.minimum_derivative_up,
            request.added_chunk,
        );
        let Some(left) = self.refine_peak_side(left_request, params.refinement)? else {
            return Ok(None);
        };
        let right_request = PeakRefinementRequest::new(
            request.raw_start,
            request.raw_stop,
            1,
            request.half_mode,
            request.minimum_derivative_down,
            request.added_chunk,
        );
        let Some(right) = self.refine_peak_side(right_request, params.refinement)? else {
            return Ok(None);
        };

        let width = right.abscissa.wrapping_sub(left.abscissa).wrapping_add(1);
        if width > params.maximum_bar_width {
            return Ok(None);
        }

        let mut maximum_value = 0;
        for position in left.abscissa..=right.abscissa {
            maximum_value = maximum_value.max(self.value(position));
        }
        Ok(Some(ProjectionPeakCandidate {
            raw_start: request.raw_start,
            raw_stop: request.raw_stop,
            start: left.abscissa,
            stop: right.abscissa,
            maximum_value,
            left,
            right,
        }))
    }

    /// Pure numeric composition of Java `StaffProjector.browseRange`.
    ///
    /// Derivative rises and falls can split a broad count range into multiple
    /// candidates. Candidate construction can abstain independently for each
    /// subrange; core pixels, staff ordinates, grade acceptance, graph mutation,
    /// and deskew ownership deliberately remain with later stages.
    pub fn browse_peak_range(
        &self,
        request: PeakRangeRequest,
        params: PeakConstructionParams,
    ) -> Result<Vec<ProjectionPeakCandidate>, ProjectionError> {
        if request.range_start < self.start
            || request.range_stop > self.stop
            || request.range_start > request.range_stop
        {
            return Err(ProjectionError::InvalidPeakRange {
                x_start: request.range_start,
                x_stop: request.range_stop,
            });
        }

        let mut candidates = Vec::new();
        let mut start = Some(request.range_start);
        let mut x = request.range_start;

        while x <= request.range_stop {
            let derivative = self.derivative(x);
            if derivative >= request.minimum_derivative_up {
                // Retain the last strictly improving rising derivative.
                let mut maximum_derivative = derivative;
                let mut xx = x.wrapping_add(1);
                while xx <= request.range_stop {
                    let next_derivative = self.derivative(xx);
                    if next_derivative > maximum_derivative {
                        maximum_derivative = next_derivative;
                        x = xx;
                    } else {
                        break;
                    }
                    xx = xx.wrapping_add(1);
                }
                start = Some(x);
            } else if derivative <= request.minimum_derivative_down.wrapping_neg() {
                // Java advances across equal falling derivatives (`<=`). Its
                // sheet clamp makes range_stop+1 observable only inside the
                // projection domain.
                let mut minimum_derivative = derivative;
                let ending_limit = request
                    .range_stop
                    .wrapping_add(1)
                    .clamp(self.start, self.stop);
                let mut xx = x.wrapping_add(1);
                while xx <= ending_limit {
                    let next_derivative = self.derivative(xx);
                    if next_derivative <= minimum_derivative {
                        minimum_derivative = next_derivative;
                        x = xx;
                    } else {
                        break;
                    }
                    xx = xx.wrapping_add(1);
                }

                if x == request.range_stop {
                    x = request.range_stop.wrapping_add(1);
                }
                let stop = x;
                if let Some(peak_start) = start
                    && peak_start < stop
                {
                    let construction = PeakConstructionRequest::new(
                        peak_start,
                        stop.wrapping_sub(1),
                        request.half_mode,
                        request.minimum_derivative_up,
                        request.minimum_derivative_down,
                        request.added_chunk,
                    );
                    if let Some(candidate) = self.construct_peak_candidate(construction, params)? {
                        candidates.push(candidate);
                    }
                    start = None;
                }
            }
            x = x.wrapping_add(1);
        }

        // Java sends a still-open range directly to createPeak.
        if let Some(peak_start) = start {
            let construction = PeakConstructionRequest::new(
                peak_start,
                request.range_stop,
                request.half_mode,
                request.minimum_derivative_up,
                request.minimum_derivative_down,
                request.added_chunk,
            );
            if let Some(candidate) = self.construct_peak_candidate(construction, params)? {
                candidates.push(candidate);
            }
        }

        Ok(candidates)
    }

    /// Pure range-scanning composition of Java
    /// `StaffProjector.findPeaksInRange`.
    ///
    /// Contiguous values at or above `minimum_count` are delegated to
    /// [`Self::browse_peak_range`]. The right-edge case goes straight through
    /// candidate construction, exactly as in Java. `accept` is the dependency
    /// boundary where the caller supplies staff ordinates and runs core/grade
    /// acceptance. It returns `None` for a rejected tentative candidate or an
    /// accepted value of its choosing. Only accepted candidates advance the
    /// scan cursor to their refined stop, which preserves Java's non-overlap
    /// rule without moving sheet, staff, graph, or deskew ownership here.
    pub fn find_peaks_in_range<T, F>(
        &self,
        request: PeakScanRequest,
        params: PeakConstructionParams,
        mut accept: F,
    ) -> Result<Vec<T>, ProjectionError>
    where
        F: FnMut(ProjectionPeakCandidate) -> Result<Option<T>, ProjectionError>,
    {
        if request.x_min < self.start || request.x_max > self.stop || request.x_min > request.x_max
        {
            return Err(ProjectionError::InvalidPeakRange {
                x_start: request.x_min,
                x_stop: request.x_max,
            });
        }

        let mut candidates = Vec::new();
        let half_mode = request.mode.is_half();
        let mut start = None;
        let mut stop = None;
        let mut x = request.x_min;

        while x <= request.x_max {
            if self.value(x) >= request.minimum_count {
                start.get_or_insert(x);
                stop = Some(x);
            } else if let (Some(range_start), Some(range_stop)) = (start, stop) {
                let range = PeakRangeRequest::new(
                    range_start,
                    range_stop,
                    half_mode,
                    request.minimum_derivative_up,
                    request.minimum_derivative_down,
                    request.added_chunk,
                );
                for candidate in self.browse_peak_range(range, params)? {
                    if let Some(accepted) = accept(candidate)? {
                        x = x.max(candidate.stop);
                        candidates.push(accepted);
                    }
                }
                start = None;
                stop = None;
            }
            x = x.wrapping_add(1);
        }

        if let (Some(range_start), Some(range_stop)) = (start, stop) {
            let construction = PeakConstructionRequest::new(
                range_start,
                range_stop,
                half_mode,
                request.minimum_derivative_up,
                request.minimum_derivative_down,
                request.added_chunk,
            );
            if let Some(candidate) = self.construct_peak_candidate(construction, params)?
                && let Some(accepted) = accept(candidate)?
            {
                candidates.push(accepted);
            }
        }

        Ok(candidates)
    }

    /// Java `StaffProjector.getChunk`: minimum projection immediately outside
    /// a refined peak side, or zero when the full probe leaves the image.
    fn chunk_minimum(&self, x0: i32, direction: i32, chunk_width: i32) -> i32 {
        let x1 = x0.wrapping_add(direction);
        let x2 = x1.wrapping_add(direction.wrapping_mul(chunk_width.wrapping_sub(1)));
        if x2 < self.start || x2 > self.stop {
            return 0;
        }

        let mut chunk = i32::MAX;
        let mut x = x1;
        while direction.wrapping_mul(x2.wrapping_sub(x)) >= 0 {
            chunk = chunk.min(self.value(x));
            x = x.wrapping_add(direction);
        }
        chunk
    }

    fn index(&self, position: i32) -> usize {
        assert!(
            (self.start..=self.stop).contains(&position),
            "projection position {position} outside {}..={}",
            self.start,
            self.stop
        );
        usize::try_from(i64::from(position) - i64::from(self.start))
            .expect("validated projection offset is nonnegative")
    }
}

impl StaffProjectionAccumulation {
    /// Compose the mutation-free part of Java `StaffProjector.process` from an
    /// already accumulated raster projection.
    ///
    /// The callback supplies source-owned staff ordinates for candidate core
    /// validation. Returned peaks are graded but remain neutral: graph
    /// insertion, semantic attributes, and deskew centers belong to later
    /// source stages. Brace discovery likewise returns projection geometry,
    /// not a graph-owned `StaffPeak`.
    pub fn finish_neutral<Geometry>(
        self,
        raster_width: usize,
        raster_height: usize,
        pixels: &[u8],
        request: NeutralStaffProjectorRequest,
        mut core_geometry_at: Geometry,
    ) -> Result<NeutralStaffProjectorResult, ProjectionError>
    where
        Geometry: FnMut(i32) -> PeakCoreGeometry,
    {
        let derivative_threshold = self.projection.staff_derivative_threshold(
            self.bounds.x_min,
            self.bounds.x_max,
            request.top_derivative_count,
            request.minimum_derivative_ratio,
        )?;
        let all_blanks = self.projection.blank_regions(request.blank_threshold);
        let peak_search_bounds = self.projection.peak_search_bounds(
            &all_blanks,
            request.staff_left,
            request.staff_right,
            request.minimum_wide_blank_width,
        );
        let half_mode = request.use_one_line_half_mode && request.is_one_line_staff;
        let minimum_count = if half_mode {
            request.bar_threshold / 2
        } else {
            request.bar_threshold
        };
        let minimum_derivative = if half_mode {
            derivative_threshold / 2
        } else {
            derivative_threshold
        };
        let scan = PeakScanRequest::new(
            peak_search_bounds.x_min,
            peak_search_bounds.x_max,
            if half_mode {
                ProjectionPeakMode::InitialHalf
            } else {
                ProjectionPeakMode::Full
            },
            minimum_count,
            minimum_derivative,
            minimum_derivative,
            0,
        );
        let peaks =
            self.projection
                .find_peaks_in_range(scan, request.peak_construction, |candidate| {
                    let midpoint = candidate.start.wrapping_add(candidate.stop) / 2;
                    let validation = candidate.validate_core(
                        raster_width,
                        raster_height,
                        pixels,
                        core_geometry_at(midpoint),
                        0,
                        request.peak_core,
                    )?;
                    candidate.into_staff_peak(
                        validation,
                        request.staff_id,
                        PeakGradeParams::new(
                            request.bar_threshold,
                            request.total_height,
                            half_mode,
                        ),
                    )
                })?;
        let brace_candidate = request
            .brace_search
            .map(|brace| self.projection.find_brace_candidate(&all_blanks, brace))
            .transpose()?
            .flatten();

        Ok(NeutralStaffProjectorResult {
            projection: self.projection,
            derivative_threshold,
            all_blanks,
            peak_search_bounds,
            peaks,
            brace_candidate,
        })
    }
}

/// Java `StaffProjector.selectBlank` over its start-ordered blank list.
#[must_use]
pub fn select_blank(
    blanks: &[ProjectionBlank],
    side: HorizontalSide,
    start: i32,
    minimum_width: i32,
) -> Option<ProjectionBlank> {
    let qualifies = |blank: &&ProjectionBlank| {
        let direction = match side {
            HorizontalSide::Left => -1_i32,
            HorizontalSide::Right => 1_i32,
        };
        direction.wrapping_mul(blank.midpoint().wrapping_sub(start)) > 0
            && blank.width() >= minimum_width
    };

    match side {
        HorizontalSide::Left => blanks.iter().rev().find(qualifies).copied(),
        HorizontalSide::Right => blanks.iter().find(qualifies).copied(),
    }
}

/// Exact decision kernel of Java `StaffProjector.checkLinesRoot`.
///
/// The caller applies `clear_staff_left_end_at` to its graph-owned peak and
/// writes `staff_left` back to its `Staff`. Returning the decision keeps both
/// mutations outside this neutral module.
#[must_use]
pub fn check_lines_root_transition(
    peaks: &[StaffPeak],
    blanks: &[ProjectionBlank],
    brace_present: bool,
    start_peak_index: Option<usize>,
    staff_left: i32,
    minimum_small_blank_width: i32,
    maximum_left_extremum: i32,
) -> LinesRootTransition {
    let unchanged = LinesRootTransition {
        staff_left,
        clear_staff_left_end_at: None,
    };
    if brace_present || peaks.is_empty() {
        return unchanged;
    }

    let Some(start_peak_index) = start_peak_index.filter(|index| *index < peaks.len()) else {
        return unchanged;
    };
    let first_peak = &peaks[0];
    let Some(blank) = select_blank(
        blanks,
        HorizontalSide::Left,
        first_peak.start(),
        minimum_small_blank_width,
    ) else {
        return unchanged;
    };
    let gap = first_peak
        .start()
        .wrapping_sub(1)
        .wrapping_sub(blank.stop());
    if gap > maximum_left_extremum {
        LinesRootTransition {
            staff_left: blank.stop().wrapping_add(1),
            clear_staff_left_end_at: Some(start_peak_index),
        }
    } else {
        unchanged
    }
}

/// Java `StaffProjector.hasStandardBlank`.
#[must_use]
pub fn has_blank_between(
    blanks: &[ProjectionBlank],
    start: i32,
    stop: i32,
    minimum_width: i32,
) -> bool {
    if stop <= start {
        return false;
    }
    select_blank(blanks, HorizontalSide::Right, start, minimum_width)
        .is_some_and(|blank| blank.start <= stop)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionError {
    InvalidDomain {
        start: i32,
        stop: i32,
    },
    InvalidDerivativeRange {
        x_min: i32,
        x_max: i32,
    },
    InsufficientDerivativeSamples {
        available: usize,
        required: usize,
    },
    InvalidDirection(i32),
    InvalidPeakRange {
        x_start: i32,
        x_stop: i32,
    },
    InvalidRefinementParameters,
    InvalidMaximumBarWidth(i32),
    InvalidCoreParameters,
    InvalidRasterDimensions {
        width: usize,
        height: usize,
    },
    InvalidRasterPixels {
        expected: usize,
        actual: usize,
    },
    InvalidCoreGeometry(PeakCoreGeometry),
    CoreProbeOutOfBounds {
        x_min: i32,
        x_max: i32,
        y_min: i32,
        y_max: i32,
    },
    MismatchedCoreValidation,
    StaffPeak(StaffPeakError),
    InvalidBraceSearchRange {
        x_min: i32,
        x_max: i32,
    },
    InvalidBraceRefinementRange {
        start: i32,
        raw_stop: i32,
        maximum_right: i32,
    },
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDomain { start, stop } => {
                write!(formatter, "invalid projection domain {start}..={stop}")
            }
            Self::InvalidDerivativeRange { x_min, x_max } => {
                write!(formatter, "invalid derivative range {x_min}..={x_max}")
            }
            Self::InsufficientDerivativeSamples {
                available,
                required,
            } => write!(
                formatter,
                "only {available} derivative samples available, need {required}"
            ),
            Self::InvalidDirection(direction) => {
                write!(
                    formatter,
                    "peak refinement direction must be -1 or 1, got {direction}"
                )
            }
            Self::InvalidPeakRange { x_start, x_stop } => {
                write!(formatter, "invalid peak range {x_start}..={x_stop}")
            }
            Self::InvalidRefinementParameters => {
                formatter.write_str("peak refinement dx and chunk width are invalid")
            }
            Self::InvalidMaximumBarWidth(width) => {
                write!(formatter, "maximum bar width must be positive, got {width}")
            }
            Self::InvalidCoreParameters => {
                formatter.write_str("core gap threshold or minimum white ratio is invalid")
            }
            Self::InvalidRasterDimensions { width, height } => {
                write!(formatter, "invalid raster dimensions {width}x{height}")
            }
            Self::InvalidRasterPixels { expected, actual } => {
                write!(formatter, "raster has {actual} pixels, expected {expected}",)
            }
            Self::InvalidCoreGeometry(geometry) => write!(
                formatter,
                "invalid core geometry top:{} mid:{} bottom:{}",
                geometry.y_top, geometry.y_mid, geometry.y_bottom,
            ),
            Self::CoreProbeOutOfBounds {
                x_min,
                x_max,
                y_min,
                y_max,
            } => write!(
                formatter,
                "core probe ({x_min}..={x_max}, {y_min}..={y_max}) is outside raster",
            ),
            Self::MismatchedCoreValidation => {
                formatter.write_str("core validation belongs to a different peak")
            }
            Self::StaffPeak(error) => error.fmt(formatter),
            Self::InvalidBraceSearchRange { x_min, x_max } => {
                write!(formatter, "invalid brace search range {x_min}..={x_max}")
            }
            Self::InvalidBraceRefinementRange {
                start,
                raw_stop,
                maximum_right,
            } => write!(
                formatter,
                "invalid brace refinement start:{start} stop:{raw_stop} right:{maximum_right}",
            ),
        }
    }
}

impl Error for ProjectionError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(start: i32, stop: i32) -> ProjectionPeakCandidate {
        ProjectionPeakCandidate {
            raw_start: start,
            raw_stop: stop,
            start,
            stop,
            maximum_value: 10,
            left: PeakSide {
                abscissa: start,
                derivative_grade: 1.0,
                chunk_grade: 1.0,
            },
            right: PeakSide {
                abscissa: stop,
                derivative_grade: 1.0,
                chunk_grade: 1.0,
            },
        }
    }

    #[test]
    fn nonzero_domain_values_and_derivatives_match_java() {
        let mut projection = ShortProjection::new(10, 13).unwrap();
        assert_eq!(projection.start(), 10);
        assert_eq!(projection.stop(), 13);
        assert_eq!(projection.len(), 4);
        assert!(!projection.is_empty());
        assert_eq!(projection.value(10), 0);

        projection.increment(10, 3);
        projection.increment_one(11);
        projection.increment(11, 4);
        projection.increment(12, -2);
        assert_eq!(projection.value(10), 3);
        assert_eq!(projection.value(11), 5);
        assert_eq!(projection.value(12), -2);
        assert_eq!(projection.derivative(10), 0);
        assert_eq!(projection.derivative(11), 2);
        assert_eq!(projection.derivative(12), -7);
        assert_eq!(projection.derivative(9), 0);
    }

    #[test]
    fn increments_narrow_with_java_signed_short_wrapping() {
        let mut projection = ShortProjection::new(0, 1).unwrap();
        projection.increment(0, i32::from(i16::MAX));
        projection.increment_one(0);
        assert_eq!(projection.value(0), i32::from(i16::MIN));

        projection.increment(1, 65_537);
        assert_eq!(projection.value(1), 1);
        projection.increment(1, i32::MAX);
        assert_eq!(projection.value(1), 0);
    }

    #[test]
    fn staff_raster_accumulation_matches_java_bounds_and_ordinates() {
        let width = 6;
        let height = 5;
        let mut pixels = vec![255; width * height];
        for (x, ys) in [
            (0, vec![0, 1, 2, 3, 4]),
            (1, vec![0, 4]),
            (2, vec![0, 1, 2, 4]),
            (3, vec![0, 1, 2, 3, 4]),
            (4, vec![3]),
            (5, vec![0, 1, 2, 3, 4]),
        ] {
            for y in ys {
                pixels[y * width + x] = FOREGROUND;
            }
        }

        let accumulation = ShortProjection::from_staff_raster(
            width,
            height,
            &pixels,
            StaffProjectionRequest::new(2, 3, 1),
            |x| match x {
                1 => -2,
                2 => 1,
                3 => 4,
                4 => 3,
                _ => unreachable!(),
            },
            |x| match x {
                1 => 6,
                2 => 4,
                3 => 2,
                4 => 4,
                _ => unreachable!(),
            },
        )
        .unwrap();

        assert_eq!(accumulation.bounds, PeakSearchBounds { x_min: 1, x_max: 4 });
        assert_eq!(accumulation.projection.start(), 0);
        assert_eq!(accumulation.projection.stop(), 5);
        assert_eq!(
            (0..width as i32)
                .map(|x| accumulation.projection.value(x))
                .collect::<Vec<_>>(),
            [0, 2, 2, 0, 1, 0]
        );
    }

    #[test]
    fn staff_raster_accumulation_preserves_java_short_count_wrapping() {
        let height = usize::from(i16::MAX as u16) + 2;
        let pixels = vec![FOREGROUND; height];
        let accumulation = ShortProjection::from_staff_raster(
            1,
            height,
            &pixels,
            StaffProjectionRequest::new(0, 0, 0),
            |_| 0,
            |_| i32::try_from(height).unwrap(),
        )
        .unwrap();

        assert_eq!(accumulation.projection.value(0), i32::from(i16::MIN) + 1);
    }

    #[test]
    fn staff_raster_accumulation_uses_wrapping_margin_and_empty_reverse_window() {
        use std::cell::Cell;

        let callback_count = Cell::new(0);
        let accumulation = ShortProjection::from_staff_raster(
            4,
            2,
            &[FOREGROUND; 8],
            StaffProjectionRequest::new(i32::MIN, i32::MAX, 1),
            |_| {
                callback_count.set(callback_count.get() + 1);
                0
            },
            |_| 2,
        )
        .unwrap();

        assert_eq!(accumulation.bounds, PeakSearchBounds { x_min: 3, x_max: 0 });
        assert_eq!(callback_count.get(), 0);
        assert_eq!(
            (0..4)
                .map(|x| accumulation.projection.value(x))
                .collect::<Vec<_>>(),
            [0, 0, 0, 0]
        );
    }

    #[test]
    fn staff_raster_accumulation_rejects_invalid_rasters() {
        assert_eq!(
            ShortProjection::from_staff_raster(
                0,
                1,
                &[],
                StaffProjectionRequest::new(0, 0, 0),
                |_| 0,
                |_| 1,
            ),
            Err(ProjectionError::InvalidRasterDimensions {
                width: 0,
                height: 1,
            })
        );
        assert_eq!(
            ShortProjection::from_staff_raster(
                2,
                2,
                &[255; 3],
                StaffProjectionRequest::new(0, 1, 0),
                |_| 0,
                |_| 2,
            ),
            Err(ProjectionError::InvalidRasterPixels {
                expected: 4,
                actual: 3,
            })
        );
    }

    #[test]
    fn neutral_projector_composes_projection_blanks_peaks_grading_and_brace() {
        let width = 20;
        let height = 5;
        let mut pixels = vec![255; width * height];
        for x in 0..width {
            pixels[x] = FOREGROUND;
            pixels[(height - 1) * width + x] = FOREGROUND;
        }
        for x in 5..=6 {
            for y in 0..height {
                pixels[y * width + x] = FOREGROUND;
            }
        }

        let accumulation = ShortProjection::from_staff_raster(
            width,
            height,
            &pixels,
            StaffProjectionRequest::new(5, 6, 20),
            |_| 0,
            |_| 5,
        )
        .unwrap();
        let peak_refinement = PeakRefinementParams::new(4, 2, 4, 2, 1).unwrap();
        let result = accumulation
            .finish_neutral(
                width,
                height,
                &pixels,
                NeutralStaffProjectorRequest {
                    staff_id: StaffId::new(1),
                    staff_left: 5,
                    staff_right: 6,
                    blank_threshold: 2,
                    minimum_wide_blank_width: 2,
                    top_derivative_count: 2,
                    minimum_derivative_ratio: 1.0,
                    use_one_line_half_mode: false,
                    is_one_line_staff: false,
                    bar_threshold: 4,
                    total_height: 5,
                    peak_construction: PeakConstructionParams::new(peak_refinement, 4).unwrap(),
                    peak_core: PeakCoreParams::new(1, 0.3).unwrap(),
                    brace_search: Some(BraceSearchRequest::new(5, 0, 7, 2, 4)),
                },
                |x| {
                    assert_eq!(x, 5);
                    PeakCoreGeometry::new(0, 4, 2)
                },
            )
            .unwrap();

        assert_eq!(result.derivative_threshold, 3);
        assert_eq!(
            result.all_blanks,
            [
                ProjectionBlank { start: 0, stop: 4 },
                ProjectionBlank { start: 7, stop: 19 },
            ]
        );
        assert_eq!(
            result.peak_search_bounds,
            PeakSearchBounds { x_min: 4, x_max: 7 }
        );
        assert_eq!(result.peaks.len(), 1);
        let peak = &result.peaks[0];
        assert_eq!(
            (peak.staff_id().value(), peak.start(), peak.stop()),
            (1, 5, 6)
        );
        assert_eq!((peak.top(), peak.bottom()), (0, 4));
        assert_eq!(
            result.brace_candidate,
            Some(ProjectionBraceCandidate {
                raw_start: 5,
                raw_stop: 6,
                start: 4,
                stop: 7,
                search_right: 7,
            })
        );
        assert_eq!(result.projection.value(5), 5);
    }

    #[test]
    fn lines_root_transition_uses_first_peak_blank_gap_and_start_index() {
        let peaks = [
            StaffPeak::new(StaffId::new(1), 0, 4, 20, 21).unwrap(),
            StaffPeak::new(StaffId::new(1), 0, 4, 25, 26).unwrap(),
        ];
        let blanks = [
            ProjectionBlank { start: 0, stop: 1 },
            ProjectionBlank { start: 5, stop: 10 },
            ProjectionBlank {
                start: 12,
                stop: 13,
            },
        ];

        // Java measures from peaks[0], even when the marked start peak is at
        // a later index: 20 - 1 - 10 = 9, strictly greater than 8.
        assert_eq!(
            check_lines_root_transition(&peaks, &blanks, false, Some(1), 3, 4, 8),
            LinesRootTransition {
                staff_left: 11,
                clear_staff_left_end_at: Some(1),
            }
        );
        // The extremum test is strict.
        assert_eq!(
            check_lines_root_transition(&peaks, &blanks, false, Some(1), 3, 4, 9),
            LinesRootTransition {
                staff_left: 3,
                clear_staff_left_end_at: None,
            }
        );
        // A brace, no start peak, or no qualifying blank leaves state intact.
        for transition in [
            check_lines_root_transition(&peaks, &blanks, true, Some(1), 3, 4, 8),
            check_lines_root_transition(&peaks, &blanks, false, None, 3, 4, 8),
            check_lines_root_transition(&peaks, &blanks, false, Some(1), 3, 7, 8),
        ] {
            assert_eq!(
                transition,
                LinesRootTransition {
                    staff_left: 3,
                    clear_staff_left_end_at: None,
                }
            );
        }
    }

    #[test]
    fn rejects_reversed_domain() {
        assert_eq!(
            ShortProjection::new(4, 3),
            Err(ProjectionError::InvalidDomain { start: 4, stop: 3 })
        );
    }

    #[test]
    fn staff_threshold_averages_the_largest_absolute_derivatives() {
        let mut projection = ShortProjection::new(0, 6).unwrap();
        for (position, value) in [(1, 2), (2, 7), (3, 6), (4, 14), (5, 14), (6, 11)] {
            projection.increment(position, value);
        }
        // Absolute derivatives are [2, 5, 1, 8, 0, 3]. The top five sum to
        // 19, average to 3.8, and scale to 1.9, which rint rounds to 2.
        assert_eq!(
            projection.staff_derivative_threshold(0, 6, 5, 0.5).unwrap(),
            2
        );
        // A bounded StaffProjector scan uses only derivatives inside its x
        // interval: [1, 8, 0] here, whose top two average is 4.5.
        assert_eq!(
            projection.staff_derivative_threshold(2, 5, 2, 1.0).unwrap(),
            4
        );
    }

    #[test]
    fn staff_threshold_preserves_java_ties_even_rounding() {
        let mut projection = ShortProjection::new(10, 12).unwrap();
        projection.increment(11, 5);
        // Derivatives are +5 and -5, so elite is exactly 5.
        assert_eq!(
            projection
                .staff_derivative_threshold(10, 12, 2, 0.5)
                .unwrap(),
            2
        );
        assert_eq!(
            projection
                .staff_derivative_threshold(10, 12, 2, 0.7)
                .unwrap(),
            4
        );
        assert_eq!(
            projection
                .staff_derivative_threshold(10, 12, 0, f64::NAN)
                .unwrap(),
            0
        );
    }

    #[test]
    fn staff_threshold_rejects_invalid_or_undersized_windows() {
        let projection = ShortProjection::new(10, 15).unwrap();
        assert_eq!(
            projection.staff_derivative_threshold(9, 15, 5, 0.3),
            Err(ProjectionError::InvalidDerivativeRange {
                x_min: 9,
                x_max: 15,
            })
        );
        assert_eq!(
            projection.staff_derivative_threshold(10, 16, 5, 0.3),
            Err(ProjectionError::InvalidDerivativeRange {
                x_min: 10,
                x_max: 16,
            })
        );
        assert_eq!(
            projection.staff_derivative_threshold(12, 11, 1, 0.3),
            Err(ProjectionError::InvalidDerivativeRange {
                x_min: 12,
                x_max: 11,
            })
        );
        assert_eq!(
            projection.staff_derivative_threshold(10, 12, 3, 0.3),
            Err(ProjectionError::InsufficientDerivativeSamples {
                available: 2,
                required: 3,
            })
        );
    }

    #[test]
    fn blank_regions_use_inclusive_thresholds_and_finish_at_domain_end() {
        let mut projection = ShortProjection::new(0, 9).unwrap();
        for (position, value) in [
            (0, 0),
            (1, 0),
            (2, 2),
            (3, 1),
            (4, 0),
            (5, 0),
            (6, 0),
            (7, 3),
            (8, 0),
            (9, 0),
        ] {
            projection.increment(position, value);
        }
        let blanks = projection.blank_regions(1);
        assert_eq!(
            blanks
                .iter()
                .map(|blank| (blank.start(), blank.stop(), blank.width()))
                .collect::<Vec<_>>(),
            [(0, 1, 2), (3, 6, 4), (8, 9, 2)]
        );

        let mut none = ShortProjection::new(20, 21).unwrap();
        none.increment(20, 2);
        none.increment(21, 2);
        assert!(none.blank_regions(1).is_empty());
        assert_eq!(
            ShortProjection::new(20, 21)
                .unwrap()
                .blank_regions(0)
                .iter()
                .map(|blank| (blank.start(), blank.stop()))
                .collect::<Vec<_>>(),
            [(20, 21)]
        );
    }

    #[test]
    fn blank_selection_respects_side_width_and_strict_midpoint_position() {
        let mut projection = ShortProjection::new(0, 9).unwrap();
        projection.increment(2, 2);
        projection.increment(7, 2);
        let blanks = projection.blank_regions(1);

        assert_eq!(
            select_blank(&blanks, HorizontalSide::Right, 2, 2)
                .map(|blank| (blank.start(), blank.stop())),
            Some((3, 6))
        );
        // The 3..6 midpoint is 4 and is not strictly right of start=4.
        assert_eq!(
            select_blank(&blanks, HorizontalSide::Right, 4, 2)
                .map(|blank| (blank.start(), blank.stop())),
            Some((8, 9))
        );
        assert_eq!(
            select_blank(&blanks, HorizontalSide::Left, 7, 2)
                .map(|blank| (blank.start(), blank.stop())),
            Some((3, 6))
        );
        assert_eq!(select_blank(&blanks, HorizontalSide::Right, 2, 5), None);
    }

    #[test]
    fn blank_selection_preserves_java_negative_midpoint_truncation() {
        let projection = ShortProjection::new(-13, -10).unwrap();
        let blanks = projection.blank_regions(0);
        // Java (-13 + -10) / 2 truncates -11.5 toward zero to -11.
        assert_eq!(
            select_blank(&blanks, HorizontalSide::Right, -12, 4)
                .map(|blank| (blank.start(), blank.stop())),
            Some((-13, -10))
        );
        assert_eq!(select_blank(&blanks, HorizontalSide::Left, -11, 4), None);
    }

    #[test]
    fn standard_blank_range_uses_blank_start_and_rejects_empty_ranges() {
        let mut projection = ShortProjection::new(0, 9).unwrap();
        projection.increment(2, 2);
        projection.increment(7, 2);
        let blanks = projection.blank_regions(1);

        assert!(has_blank_between(&blanks, 2, 3, 2));
        assert!(!has_blank_between(&blanks, 2, 2, 2));
        assert!(!has_blank_between(&blanks, 2, 9, 5));
        assert!(!has_blank_between(&blanks, 9, 3, 1));
    }

    #[test]
    fn peak_search_bounds_compose_selected_wide_blanks() {
        let mut projection = ShortProjection::new(0, 20).unwrap();
        for position in [3, 4, 9, 10, 11, 16, 17, 18] {
            projection.increment_one(position);
        }
        let blanks = projection.blank_regions(0);
        assert_eq!(
            blanks
                .iter()
                .map(|blank| (blank.start(), blank.stop()))
                .collect::<Vec<_>>(),
            [(0, 2), (5, 8), (12, 15), (19, 20)]
        );
        assert_eq!(
            projection.peak_search_bounds(&blanks, 9, 10, 3),
            PeakSearchBounds {
                x_min: 8,
                x_max: 12
            }
        );
        assert_eq!(
            projection.peak_search_bounds(&blanks, 9, 10, 5),
            PeakSearchBounds {
                x_min: 0,
                x_max: 20
            }
        );
    }

    #[test]
    fn peak_side_refinement_finds_directional_extrema_and_grades_chunks() {
        let mut projection = ShortProjection::new(0, 12).unwrap();
        for (position, value) in [(3, 2), (4, 8), (5, 10), (6, 10), (7, 10), (8, 5), (9, 3)] {
            projection.increment(position, value);
        }
        let params = PeakRefinementParams::new(10, 2, 5, 2, 2).unwrap();

        let left = projection
            .refine_peak_side(PeakRefinementRequest::new(4, 7, -1, false, 4, 0), params)
            .unwrap()
            .unwrap();
        assert_eq!(left.abscissa, 4);
        assert_eq!(left.derivative_grade, 1.0);
        assert_eq!(left.chunk_grade, 1.0);

        let right = projection
            .refine_peak_side(PeakRefinementRequest::new(4, 7, 1, false, 4, 0), params)
            .unwrap()
            .unwrap();
        assert_eq!(right.abscissa, 7);
        assert!((right.derivative_grade - (5.0 / 6.0)).abs() < 1.0e-14);
        assert!((right.chunk_grade - (2.0 / 3.0)).abs() < 1.0e-14);

        let added = projection
            .refine_peak_side(PeakRefinementRequest::new(4, 7, 1, false, 4, 2), params)
            .unwrap()
            .unwrap();
        assert_eq!(added.chunk_grade, 1.0);

        let half = projection
            .refine_peak_side(PeakRefinementRequest::new(4, 7, -1, true, 4, 0), params)
            .unwrap()
            .unwrap();
        assert_eq!(half.derivative_grade, 6.0);
    }

    #[test]
    fn peak_side_refinement_keeps_first_equal_extremum() {
        let mut projection = ShortProjection::new(0, 10).unwrap();
        for (position, value) in [(5, 10), (6, 10), (7, 10), (8, 5)] {
            projection.increment(position, value);
        }
        let params = PeakRefinementParams::new(10, 2, 5, 2, 2).unwrap();
        // Derivatives at x=8 and x=9 are both -5. Java's strict comparison
        // retains x=8, then the right peak side is x=8-1.
        assert_eq!(
            projection
                .refine_peak_side(PeakRefinementRequest::new(4, 7, 1, false, 4, 0), params)
                .unwrap()
                .unwrap()
                .abscissa,
            7
        );
    }

    #[test]
    fn peak_side_refinement_uses_java_border_fallback() {
        let mut projection = ShortProjection::new(0, 5).unwrap();
        projection.increment(4, 4);
        projection.increment(5, 4);
        let params = PeakRefinementParams::new(10, 2, 5, 2, 2).unwrap();
        let side = projection
            .refine_peak_side(PeakRefinementRequest::new(4, 5, 1, false, 3, 0), params)
            .unwrap()
            .unwrap();
        assert_eq!(side.abscissa, 5);
        assert!((side.derivative_grade - (4.0 / 7.0)).abs() < 1.0e-14);
        assert_eq!(side.chunk_grade, 1.0);

        let projection = ShortProjection::new(0, 9).unwrap();
        assert_eq!(
            projection
                .refine_peak_side(PeakRefinementRequest::new(2, 3, 1, false, 3, 0), params)
                .unwrap(),
            None
        );
    }

    #[test]
    fn peak_side_refinement_rejects_invalid_control_inputs() {
        let projection = ShortProjection::new(0, 9).unwrap();
        let params = PeakRefinementParams::new(10, 2, 5, 2, 2).unwrap();
        assert_eq!(
            projection.refine_peak_side(PeakRefinementRequest::new(2, 3, 0, false, 3, 0), params),
            Err(ProjectionError::InvalidDirection(0))
        );
        assert_eq!(
            projection.refine_peak_side(PeakRefinementRequest::new(-1, 3, 1, false, 3, 0), params),
            Err(ProjectionError::InvalidPeakRange {
                x_start: -1,
                x_stop: 3,
            })
        );
        assert_eq!(
            projection.refine_peak_side(PeakRefinementRequest::new(4, 3, 1, false, 3, 0), params),
            Err(ProjectionError::InvalidPeakRange {
                x_start: 4,
                x_stop: 3,
            })
        );
        assert_eq!(
            PeakRefinementParams::new(10, 2, 5, -1, 2),
            Err(ProjectionError::InvalidRefinementParameters)
        );
        assert_eq!(
            PeakRefinementParams::new(10, 2, 5, 1, 0),
            Err(ProjectionError::InvalidRefinementParameters)
        );
    }

    #[test]
    fn peak_construction_composes_refined_sides_width_and_maximum() {
        let mut projection = ShortProjection::new(0, 12).unwrap();
        for (position, value) in [(3, 2), (4, 8), (5, 10), (6, 10), (7, 10), (8, 5), (9, 3)] {
            projection.increment(position, value);
        }
        let refinement = PeakRefinementParams::new(10, 2, 5, 2, 2).unwrap();
        let request = PeakConstructionRequest::new(4, 7, false, 4, 4, 0);
        let params = PeakConstructionParams::new(refinement, 4).unwrap();
        let candidate = projection
            .construct_peak_candidate(request, params)
            .unwrap()
            .unwrap();
        assert_eq!(candidate.raw_start, 4);
        assert_eq!(candidate.raw_stop, 7);
        assert_eq!(candidate.start, 4);
        assert_eq!(candidate.stop, 7);
        assert_eq!(candidate.maximum_value, 10);
        assert_eq!(candidate.left.abscissa, 4);
        assert_eq!(candidate.right.abscissa, 7);

        assert_eq!(
            projection
                .construct_peak_candidate(
                    request,
                    PeakConstructionParams::new(refinement, 3).unwrap(),
                )
                .unwrap(),
            None
        );
    }

    #[test]
    fn peak_construction_abstains_on_missing_side_and_validates_width() {
        let projection = ShortProjection::new(0, 9).unwrap();
        let refinement = PeakRefinementParams::new(10, 2, 5, 2, 2).unwrap();
        let params = PeakConstructionParams::new(refinement, 4).unwrap();
        assert_eq!(
            projection
                .construct_peak_candidate(
                    PeakConstructionRequest::new(2, 3, false, 3, 3, 0),
                    params,
                )
                .unwrap(),
            None
        );
        assert_eq!(
            projection.construct_peak_candidate(
                PeakConstructionRequest::new(-1, 3, false, 3, 3, 0),
                params,
            ),
            Err(ProjectionError::InvalidPeakRange {
                x_start: -1,
                x_stop: 3,
            })
        );
        assert_eq!(
            PeakConstructionParams::new(refinement, 0),
            Err(ProjectionError::InvalidMaximumBarWidth(0))
        );
    }

    #[test]
    fn peak_range_browse_splits_derivative_peaks_and_keeps_right_edge() {
        let mut projection = ShortProjection::new(0, 12).unwrap();
        for (position, value) in [
            (2, 8),
            (3, 10),
            (4, 10),
            (5, 3),
            (8, 8),
            (9, 10),
            (10, 10),
            (11, 3),
        ] {
            projection.increment(position, value);
        }
        let refinement = PeakRefinementParams::new(10, 2, 5, 2, 2).unwrap();
        let params = PeakConstructionParams::new(refinement, 4).unwrap();
        let candidates = projection
            .browse_peak_range(PeakRangeRequest::new(2, 10, false, 4, 4, 0), params)
            .unwrap();

        assert_eq!(candidates.len(), 2);
        assert_eq!(
            candidates
                .iter()
                .map(|peak| (peak.raw_start, peak.raw_stop, peak.start, peak.stop))
                .collect::<Vec<_>>(),
            [(2, 4, 2, 4), (8, 10, 8, 10)]
        );
    }

    #[test]
    fn peak_range_browse_preserves_derivative_ties_and_mutated_cursor() {
        let mut projection = ShortProjection::new(0, 8).unwrap();
        for (position, value) in [(2, 4), (3, 10), (4, 10), (5, 5)] {
            projection.increment(position, value);
        }
        let refinement = PeakRefinementParams::new(10, 2, 5, 2, 2).unwrap();
        let params = PeakConstructionParams::new(refinement, 4).unwrap();
        let candidates = projection
            .browse_peak_range(PeakRangeRequest::new(2, 6, false, 4, 4, 0), params)
            .unwrap();

        assert_eq!(candidates.len(), 1);
        let peak = candidates[0];
        // The rising scan moves from +4 at x=2 to the strictly better +6 at
        // x=3. Equal -5 derivatives at x=5 and x=6 both advance the falling
        // cursor, then x==rangeStop becomes stop=rangeStop+1.
        assert_eq!((peak.raw_start, peak.raw_stop), (3, 6));
        assert_eq!((peak.start, peak.stop), (3, 4));
    }

    #[test]
    fn count_range_scan_composes_closed_and_ongoing_peak_paths() {
        let mut projection = ShortProjection::new(0, 12).unwrap();
        for (position, value) in [
            (2, 8),
            (3, 10),
            (4, 10),
            (5, 3),
            (8, 8),
            (9, 10),
            (10, 10),
            (11, 3),
        ] {
            projection.increment(position, value);
        }
        let refinement = PeakRefinementParams::new(10, 2, 5, 2, 2).unwrap();
        let params = PeakConstructionParams::new(refinement, 4).unwrap();
        let request = PeakScanRequest::new(0, 10, ProjectionPeakMode::Full, 8, 4, 4, 0);
        let candidates = projection
            .find_peaks_in_range(request, params, |candidate| Ok(Some(candidate)))
            .unwrap();

        assert_eq!(
            candidates
                .iter()
                .map(|peak| (peak.raw_start, peak.raw_stop, peak.start, peak.stop))
                .collect::<Vec<_>>(),
            [(2, 4, 2, 4), (8, 10, 8, 10)]
        );

        let half = projection
            .find_peaks_in_range(
                PeakScanRequest::new(0, 4, ProjectionPeakMode::InitialHalf, 8, 4, 4, 0),
                params,
                |candidate| Ok(Some(candidate)),
            )
            .unwrap();
        assert_eq!(half.len(), 1);
        assert!(half[0].left.derivative_grade > candidates[0].left.derivative_grade);

        assert_eq!(
            projection.find_peaks_in_range(
                PeakScanRequest::new(-1, 4, ProjectionPeakMode::Half, 8, 4, 4, 0),
                params,
                |candidate| Ok(Some(candidate)),
            ),
            Err(ProjectionError::InvalidPeakRange {
                x_start: -1,
                x_stop: 4,
            })
        );
    }

    #[test]
    fn rejected_tentative_peak_does_not_skip_the_next_count_range() {
        let mut projection = ShortProjection::new(0, 6).unwrap();
        for (position, value) in [(2, 10), (3, 7), (4, 10)] {
            projection.increment(position, value);
        }
        let refinement = PeakRefinementParams::new(10, 2, 5, 3, 2).unwrap();
        let params = PeakConstructionParams::new(refinement, 4).unwrap();
        let mut tentative_ranges = Vec::new();
        let accepted = projection
            .find_peaks_in_range(
                PeakScanRequest::new(0, 5, ProjectionPeakMode::Full, 8, 3, 3, 0),
                params,
                |candidate| {
                    tentative_ranges.push((candidate.raw_start, candidate.stop));
                    Ok((candidate.raw_start == 4).then_some(candidate))
                },
            )
            .unwrap();

        // The first tentative candidate refines through x=4, beyond the x=3
        // count-threshold break. Since core/grade acceptance rejects it, Java
        // does not advance x and the high-count run at x=4 is still visited.
        assert_eq!(tentative_ranges, [(2, 4), (4, 4)]);
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].raw_start, 4);
    }

    #[test]
    fn rejected_overwide_range_leaves_cursor_for_following_oracle_peak() {
        let mut projection = ShortProjection::new(0, 22).unwrap();
        for position in 2..=18 {
            projection.increment(position, 30);
        }
        projection.increment(20, 40);
        let refinement = PeakRefinementParams::new(25, 2, 5, 2, 2).unwrap();
        let params = PeakConstructionParams::new(refinement, 15).unwrap();
        assert!(
            projection
                .browse_peak_range(PeakRangeRequest::new(2, 18, false, 20, 20, 0), params)
                .unwrap()
                .is_empty()
        );
        let accepted = projection
            .find_peaks_in_range(
                PeakScanRequest::new(0, 21, ProjectionPeakMode::Full, 25, 20, 20, 0),
                params,
                |candidate| Ok(Some(candidate)),
            )
            .unwrap();
        assert_eq!(
            accepted
                .iter()
                .map(|candidate| (
                    candidate.raw_start,
                    candidate.raw_stop,
                    candidate.start,
                    candidate.stop
                ))
                .collect::<Vec<_>>(),
            [(20, 20, 20, 20)]
        );
    }

    #[test]
    fn brace_search_requires_valley_and_refines_to_neighboring_minima() {
        let mut projection = ShortProjection::new(0, 15).unwrap();
        for (position, value) in [(9, 6), (10, 7), (12, 8)] {
            projection.increment(position, value);
        }
        let blanks = projection.blank_regions(0);
        let candidate = projection
            .find_brace_candidate(&blanks, BraceSearchRequest::new(0, 0, 12, 2, 5))
            .unwrap()
            .unwrap();

        // x=12 is bar-side ink, x=11 establishes the valley, and x=10..9 is
        // the brace threshold run. Refinement begins at the preceding blank's
        // stop and retains the first strict minimum toward the bar.
        assert_eq!(
            candidate,
            ProjectionBraceCandidate {
                raw_start: 9,
                raw_stop: 10,
                start: 8,
                stop: 11,
                search_right: 12,
            }
        );

        let peak = candidate
            .into_staff_peak(
                StaffId::new(3),
                |x| {
                    assert_eq!(x, 9); // Java integer (8+11)/2.
                    (20, 40)
                },
                |point| {
                    assert_eq!(point, crate::staff_peak::PeakPoint::new(9.5, 30.0));
                    crate::staff_peak::PeakPoint::new(point.x + 1.0, point.y - 2.0)
                },
            )
            .unwrap();
        assert_eq!(
            (peak.top(), peak.bottom(), peak.start(), peak.stop()),
            (20, 40, 8, 11)
        );
        assert_eq!(peak.impacts(), None);
        assert_eq!(
            peak.attributes().collect::<Vec<_>>(),
            [crate::staff_peak::StaffPeakAttribute::Brace]
        );
        assert_eq!(
            peak.deskewed_center(),
            Some(crate::staff_peak::PeakPoint::new(10.5, 28.0))
        );
    }

    #[test]
    fn brace_search_looks_left_of_large_ending_blank() {
        let mut projection = ShortProjection::new(0, 20).unwrap();
        for (position, value) in [(6, 6), (7, 7), (9, 8)] {
            projection.increment(position, value);
        }
        let blanks = [
            ProjectionBlank { start: 3, stop: 5 },
            ProjectionBlank { start: 8, stop: 8 },
            ProjectionBlank {
                start: 10,
                stop: 12,
            },
        ];
        let candidate = projection
            .find_brace_candidate(&blanks, BraceSearchRequest::new(15, 0, 14, 3, 5))
            .unwrap()
            .unwrap();

        // The 10..12 ending blank reaches maximum_right once Java's +2
        // tolerance is applied. Search therefore moves left of it to x=9 and
        // uses 3..5 as the previous significant blank.
        assert_eq!(
            candidate,
            ProjectionBraceCandidate {
                raw_start: 6,
                raw_stop: 7,
                start: 5,
                stop: 8,
                search_right: 9,
            }
        );
    }

    #[test]
    fn brace_refinement_uses_strict_left_descent_and_first_right_minimum() {
        let mut projection = ShortProjection::new(0, 12).unwrap();
        for (position, value) in [(3, 2), (4, 2), (5, 3), (9, 5), (10, 1), (11, 1)] {
            projection.increment(position, value);
        }
        let candidate = projection
            .refine_brace_candidate(&[ProjectionBlank { start: 5, stop: 5 }], 8, 9, 11)
            .unwrap()
            .unwrap();
        assert_eq!(candidate.start, 4);
        assert_eq!(candidate.stop, 10);
    }

    #[test]
    fn brace_search_abstains_without_valley_and_validates_effective_range() {
        let mut projection = ShortProjection::new(0, 5).unwrap();
        for x in 0..=5 {
            projection.increment(x, 6);
        }
        assert_eq!(
            projection
                .find_brace_candidate(&[], BraceSearchRequest::new(0, 0, 5, 2, 5))
                .unwrap(),
            None
        );
        assert_eq!(
            projection.find_brace_candidate(&[], BraceSearchRequest::new(0, 0, 6, 2, 5),),
            Err(ProjectionError::InvalidBraceSearchRange { x_min: 0, x_max: 6 })
        );
    }

    #[test]
    fn vertical_core_counts_only_enclosed_gaps_but_all_white_rows() {
        let mut pixels = vec![255; 5 * 9];
        for y in [2, 6] {
            pixels[y * 5 + 2] = FOREGROUND;
        }
        let data = vertical_core_data(5, 9, &pixels, 2, 2, 0, 8).unwrap();
        assert_eq!(data.length, 9);
        // Rows 3..=5 are enclosed by black rows 2 and 6. Java records
        // lastWhiteY-lastBlackY, which is 5-2 = 3.
        assert_eq!(data.gap, 3);
        assert!((data.white_ratio - (7.0 / 9.0)).abs() < 1.0e-14);

        // Trailing white rows do not close another gap.
        let data = vertical_core_data(5, 9, &pixels, 2, 2, 6, 8).unwrap();
        assert_eq!(data.gap, 0);
        assert!((data.white_ratio - (2.0 / 3.0)).abs() < 1.0e-14);
    }

    #[test]
    fn peak_core_validation_thickens_thin_serif_and_limits_its_height() {
        let mut pixels = vec![255; 8 * 9];
        for y in 2..=6 {
            // The candidate itself is x=3. Ink only at adjacent x=2 proves
            // the Java one-pixel thickening is part of the lookup.
            pixels[y * 8 + 2] = FOREGROUND;
        }
        let validation = candidate(3, 3)
            .validate_core(
                8,
                9,
                &pixels,
                PeakCoreGeometry::new(0, 8, 4),
                4,
                PeakCoreParams::new(1, 0.3).unwrap(),
            )
            .unwrap();
        assert!(validation.is_accepted());
        assert_eq!((validation.y_top, validation.y_bottom), (2, 6));
        assert_eq!(validation.core.gap, 0);
        assert_eq!(validation.core.white_ratio, 0.0);
        let full = validation.full_height_core.unwrap();
        assert_eq!(full.gap, 0);
        assert!((full.white_ratio - (4.0 / 9.0)).abs() < 1.0e-14);
    }

    #[test]
    fn peak_core_validation_rejects_gap_then_tall_multiple_rest_serif() {
        let mut pixels = vec![255; 8 * 9];
        for y in [0, 4, 5, 6, 7, 8] {
            pixels[y * 8 + 3] = FOREGROUND;
        }
        let gap = candidate(3, 3)
            .validate_core(
                8,
                9,
                &pixels,
                PeakCoreGeometry::new(0, 8, 4),
                0,
                PeakCoreParams::new(2, 0.3).unwrap(),
            )
            .unwrap();
        assert_eq!(gap.core.gap, 3);
        assert_eq!(gap.rejection, Some(PeakCoreRejection::GapTooLarge));

        pixels.fill(255);
        for y in 0..=8 {
            pixels[y * 8 + 3] = FOREGROUND;
        }
        let serif = candidate(3, 3)
            .validate_core(
                8,
                9,
                &pixels,
                PeakCoreGeometry::new(0, 8, 4),
                4,
                PeakCoreParams::new(2, 0.3).unwrap(),
            )
            .unwrap();
        assert_eq!(serif.core.gap, 0);
        assert_eq!(serif.full_height_core.unwrap().white_ratio, 0.0);
        assert_eq!(
            serif.rejection,
            Some(PeakCoreRejection::InsufficientWhiteBeyondSerif)
        );
    }

    #[test]
    fn peak_core_validation_fails_closed_for_invalid_inputs() {
        assert_eq!(
            PeakCoreParams::new(-1, 0.3),
            Err(ProjectionError::InvalidCoreParameters)
        );
        assert_eq!(
            PeakCoreParams::new(1, f64::NAN),
            Err(ProjectionError::InvalidCoreParameters)
        );

        let pixels = vec![255; 8 * 9];
        let params = PeakCoreParams::new(1, 0.3).unwrap();
        assert_eq!(
            candidate(0, 0)
                .validate_core(8, 9, &pixels, PeakCoreGeometry::new(0, 8, 4), 0, params,),
            Err(ProjectionError::CoreProbeOutOfBounds {
                x_min: -1,
                x_max: 1,
                y_min: 0,
                y_max: 8,
            })
        );
        assert_eq!(
            candidate(3, 3).validate_core(
                8,
                9,
                &pixels[..10],
                PeakCoreGeometry::new(0, 8, 4),
                0,
                params,
            ),
            Err(ProjectionError::InvalidRasterPixels {
                expected: 72,
                actual: 10,
            })
        );
    }

    #[test]
    fn validated_candidate_becomes_neutral_graded_staff_peak() {
        let mut pixels = vec![255; 8 * 9];
        for y in 0..=8 {
            pixels[y * 8 + 3] = FOREGROUND;
        }
        let source = candidate(3, 3);
        let validation = source
            .validate_core(
                8,
                9,
                &pixels,
                PeakCoreGeometry::new(0, 8, 4),
                0,
                PeakCoreParams::new(2, 0.3).unwrap(),
            )
            .unwrap();
        let peak = source
            .into_staff_peak(
                validation,
                StaffId::new(2),
                PeakGradeParams::new(4, 12, false),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            (peak.staff_id().value(), peak.start(), peak.stop()),
            (2, 3, 3)
        );
        assert_eq!((peak.top(), peak.bottom()), (0, 8));
        assert_eq!(peak.deskewed_center(), None);
        assert!(peak.attributes().next().is_none());

        let impacts = peak.impacts().unwrap();
        assert_eq!(impacts.core(), 0.75);
        assert_eq!(impacts.gap(), 1.0);
        assert_eq!(impacts.start(), 1.0);
        assert_eq!(impacts.stop(), 1.0);
        assert_eq!(impacts.left(), 1.0);
        assert_eq!(impacts.right(), 1.0);
        assert!(impacts.grade() > MINIMUM_STAFF_PEAK_GRADE);
    }

    #[test]
    fn staff_peak_grade_abstains_below_threshold_and_rejects_mixed_evidence() {
        let mut pixels = vec![255; 8 * 9];
        for y in 0..=8 {
            pixels[y * 8 + 3] = FOREGROUND;
        }
        let params = PeakCoreParams::new(2, 0.3).unwrap();
        let source = candidate(3, 3);
        let validation = source
            .validate_core(8, 9, &pixels, PeakCoreGeometry::new(0, 8, 4), 0, params)
            .unwrap();

        let mut weak = source;
        weak.maximum_value = 4;
        assert_eq!(
            weak.into_staff_peak(
                validation,
                StaffId::new(2),
                PeakGradeParams::new(4, 12, false),
            )
            .unwrap(),
            None
        );
        assert!(matches!(
            candidate(4, 4).into_staff_peak(
                validation,
                StaffId::new(2),
                PeakGradeParams::new(4, 12, false),
            ),
            Err(ProjectionError::MismatchedCoreValidation)
        ));
        assert!(matches!(
            source.into_staff_peak(
                validation,
                StaffId::new(0),
                PeakGradeParams::new(4, 12, false),
            ),
            Err(ProjectionError::StaffPeak(StaffPeakError::InvalidStaffId))
        ));

        pixels.fill(255);
        for y in [0, 4, 8] {
            pixels[y * 8 + 3] = FOREGROUND;
        }
        let rejected = source
            .validate_core(8, 9, &pixels, PeakCoreGeometry::new(0, 8, 4), 0, params)
            .unwrap();
        assert_eq!(rejected.rejection, Some(PeakCoreRejection::GapTooLarge));
        assert_eq!(
            source
                .into_staff_peak(
                    rejected,
                    StaffId::new(2),
                    PeakGradeParams::new(4, 12, false),
                )
                .unwrap(),
            None
        );
    }

    #[test]
    #[should_panic(expected = "projection position 14 outside 10..=13")]
    fn value_outside_domain_matches_java_bounds_failure() {
        let projection = ShortProjection::new(10, 13).unwrap();
        let _ = projection.value(14);
    }
}
