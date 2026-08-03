// SPDX-License-Identifier: AGPL-3.0-or-later

//! Signed-short projection storage used by Java `grid.StaffProjector`.
//!
//! Staff projection counts are stored in Java `short` cells. Increments narrow
//! with two's-complement wrapping, while reads and derivatives widen to `int`.

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::{error::Error, fmt};

use crate::run_table::FOREGROUND;
use crate::staff_peak::HorizontalSide;

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
    pub y_top: i32,
    pub y_bottom: i32,
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
            y_top,
            y_bottom,
            core,
            full_height_core,
            rejection,
        })
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
    #[should_panic(expected = "projection position 14 outside 10..=13")]
    fn value_outside_domain_matches_java_bounds_failure() {
        let projection = ShortProjection::new(10, 13).unwrap();
        let _ = projection.value(14);
    }
}
