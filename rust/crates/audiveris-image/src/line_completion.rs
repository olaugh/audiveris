// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact stage lifecycle for Java `LinesRetriever.completeLines`.
//!
//! Concrete geometry and ownership remain in the individual helpers. This
//! module freezes their production order and deliberately retains partial
//! mutation when a stage fails.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCompletionStage {
    DefineEndPoints,
    IncludeDiscardedFilaments,
    FillHolesInitial,
    DispatchHorizontalSections,
    IncludeThickSections,
    IncludeThinSections,
    PolishCurvatures,
    FillHolesAfterPolish,
    IncludeStickers,
    InspectCrossingChunks,
    FillHolesFinal,
}

pub trait LineCompletionExecutor {
    type Error;

    /// Java loads `Picture.SourceKey.BINARY` before entering its `try/finally`.
    fn load_binary_buffer(&mut self) -> Result<(), Self::Error>;

    fn run_stage(&mut self, stage: LineCompletionStage) -> Result<(), Self::Error>;

    /// Java's stopwatch-printing `finally` hook.
    fn finish(&mut self);
}

/// Execute the exact headless `completeLines` order.
pub fn complete_lines<Executor>(
    executor: &mut Executor,
    inspect_crossing_chunks: bool,
) -> Result<(), Executor::Error>
where
    Executor: LineCompletionExecutor,
{
    executor.load_binary_buffer()?;

    let stages = [
        LineCompletionStage::DefineEndPoints,
        LineCompletionStage::IncludeDiscardedFilaments,
        LineCompletionStage::FillHolesInitial,
        LineCompletionStage::DispatchHorizontalSections,
        LineCompletionStage::IncludeThickSections,
        LineCompletionStage::IncludeThinSections,
        LineCompletionStage::PolishCurvatures,
        LineCompletionStage::FillHolesAfterPolish,
        LineCompletionStage::IncludeStickers,
    ];

    let result = (|| {
        for stage in stages {
            executor.run_stage(stage)?;
        }
        if inspect_crossing_chunks {
            executor.run_stage(LineCompletionStage::InspectCrossingChunks)?;
        }
        executor.run_stage(LineCompletionStage::FillHolesFinal)
    })();
    executor.finish();
    result
}

use crate::run_table::Orientation;
use crate::section::Section;
use std::collections::BTreeSet;
use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurvaturePoint {
    pub x: f64,
    pub y: f64,
}

impl CurvaturePoint {
    fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

/// Rectangle already expressed in the filament's rough orientation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrientedSectionBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl OrientedSectionBounds {
    /// Java `Rectangle2D.intersects`: edge-only contact is not intersection.
    fn intersects(self, other: Self) -> bool {
        self.width > 0.0
            && self.height > 0.0
            && other.width > 0.0
            && other.height > 0.0
            && other.x + other.width > self.x
            && other.y + other.height > self.y
            && other.x < self.x + self.width
            && other.y < self.y + self.height
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurvatureSection {
    pub id: usize,
    pub oriented_bounds: OrientedSectionBounds,
    /// Absolute integer centroid exposed by Java `Section.getCentroid()`.
    pub centroid: CurvaturePoint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurvedFilamentPolishState {
    pub orientation: Orientation,
    pub interline: i32,
    pub start_point: CurvaturePoint,
    pub stop_point: CurvaturePoint,
    /// `None` represents invalidated spline/point caches.
    pub points: Option<Vec<CurvaturePoint>>,
    pub members: Vec<CurvatureSection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurvatureRemoval {
    pub section_id: usize,
    /// Interior point whose radius was the first strict minimum.
    pub radius_point_index: usize,
    /// Point actually probed after Java's endpoint adjustment.
    pub probe_point_index: usize,
    pub radius: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurvaturePolishReport {
    pub removals: Vec<CurvatureRemoval>,
    /// Number of `setEndingPoints(oldStartPoint, oldStopPoint)` recomputations.
    pub recomputations: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CurvaturePolishError<E> {
    Recompute(E),
    InvalidPointCount { count: usize },
}

impl<E: fmt::Display> fmt::Display for CurvaturePolishError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Recompute(error) => write!(formatter, "curvature recomputation failed: {error}"),
            Self::InvalidPointCount { count } => {
                write!(
                    formatter,
                    "curved filament requires at least two points, got {count}"
                )
            }
        }
    }
}

impl<E: Error + 'static> Error for CurvaturePolishError<E> {}

/// Dependency-light port of Java `CurvedFilament.polishCurvature`.
///
/// The callback is the direct boundary around pixel-probe spline recomputation.
/// Removals are retained if a later recomputation fails, matching Java's
/// non-transactional mutation. Candidate section order is preserved for equal
/// centroid distance.
pub fn polish_curvature<E>(
    state: &mut CurvedFilamentPolishState,
    minimum_radius: i32,
    mut recompute: impl FnMut(
        &[CurvatureSection],
        CurvaturePoint,
        CurvaturePoint,
    ) -> Result<Vec<CurvaturePoint>, E>,
) -> Result<CurvaturePolishReport, CurvaturePolishError<E>> {
    let old_start = state.start_point;
    let old_stop = state.stop_point;
    let mut removals = Vec::new();
    let mut recomputations = 0;

    loop {
        if state.points.is_none() {
            let points = recompute(&state.members, old_start, old_stop)
                .map_err(CurvaturePolishError::Recompute)?;
            if points.len() < 2 {
                return Err(CurvaturePolishError::InvalidPointCount {
                    count: points.len(),
                });
            }
            // setEndingPoints preserves these exact endpoint objects/values.
            state.start_point = old_start;
            state.stop_point = old_stop;
            state.points = Some(points);
            recomputations += 1;
        }

        let Some(removal) = curvature_removal_candidate(state, minimum_radius)? else {
            break;
        };
        let section_index = state
            .members
            .iter()
            .position(|section| section.id == removal.section_id)
            .expect("candidate belongs to the current member set");
        let section = state.members.remove(section_index);
        debug_assert_eq!(section.id, removal.section_id);
        removals.push(removal);
        // removeSection invalidates points and spline; the next do/while pass
        // restores the original endpoints and recomputes from remaining ink.
        state.points = None;
    }

    Ok(CurvaturePolishReport {
        removals,
        recomputations,
    })
}

/// Select Java's next strict-minimum curvature removal without mutating.
pub fn curvature_removal_candidate<E>(
    state: &CurvedFilamentPolishState,
    minimum_radius: i32,
) -> Result<Option<CurvatureRemoval>, CurvaturePolishError<E>> {
    let points = state
        .points
        .as_ref()
        .ok_or(CurvaturePolishError::InvalidPointCount { count: 0 })?;
    if points.len() < 2 {
        return Err(CurvaturePolishError::InvalidPointCount {
            count: points.len(),
        });
    }

    let mut minimum = f64::from(i32::MAX);
    let mut minimum_index = 0;
    for index in 1..points.len() - 1 {
        let radius = radius_at(points, index);
        if minimum > radius {
            minimum = radius;
            minimum_index = index;
        }
    }
    if !matches!(
        minimum.partial_cmp(&f64::from(minimum_radius)),
        Some(std::cmp::Ordering::Less)
    ) {
        return Ok(None);
    }

    let probe_index = if minimum_index == 1 {
        0
    } else if minimum_index == points.len() - 2 {
        points.len() - 1
    } else {
        minimum_index
    };
    let point = points[probe_index];
    let oriented_point = orient_point(point, state.orientation);
    let probe_width = (f64::from(state.interline) * 0.5).round_ties_even();
    let probe = OrientedSectionBounds {
        x: oriented_point.x - probe_width / 2.0,
        y: oriented_point.y - probe_width / 2.0,
        width: probe_width,
        height: probe_width,
    };
    let mut selected = None;
    let mut selected_distance = f64::NAN;
    for section in &state.members {
        if !probe.intersects(section.oriented_bounds) {
            continue;
        }
        let distance = point.distance(section.centroid);
        if selected.is_none() || distance.total_cmp(&selected_distance).is_lt() {
            selected = Some(section.id);
            selected_distance = distance;
        }
    }
    Ok(selected.map(|section_id| CurvatureRemoval {
        section_id,
        radius_point_index: minimum_index,
        probe_point_index: probe_index,
        radius: minimum,
    }))
}

fn orient_point(point: CurvaturePoint, orientation: Orientation) -> CurvaturePoint {
    match orientation {
        Orientation::Horizontal => point,
        Orientation::Vertical => CurvaturePoint {
            x: point.y,
            y: point.x,
        },
    }
}

fn radius_at(points: &[CurvaturePoint], index: usize) -> f64 {
    let previous = bisector(points[index - 1], points[index]);
    let next = bisector(points[index], points[index + 1]);
    let center = line_intersection(previous.0, previous.1, next.0, next.1);
    center.distance(points[index])
}

fn bisector(one: CurvaturePoint, two: CurvaturePoint) -> (CurvaturePoint, CurvaturePoint) {
    let half_dx = (two.x - one.x) / 2.0;
    let half_dy = (two.y - one.y) / 2.0;
    let middle_x = one.x + half_dx;
    let middle_y = one.y + half_dy;
    (
        CurvaturePoint {
            x: middle_x + half_dy,
            y: middle_y - half_dx,
        },
        CurvaturePoint {
            x: middle_x - half_dy,
            y: middle_y + half_dx,
        },
    )
}

fn line_intersection(
    one_start: CurvaturePoint,
    one_stop: CurvaturePoint,
    two_start: CurvaturePoint,
    two_stop: CurvaturePoint,
) -> CurvaturePoint {
    let denominator = ((one_start.x - one_stop.x) * (two_start.y - two_stop.y))
        - ((one_start.y - one_stop.y) * (two_start.x - two_stop.x));
    let value_one = (one_start.x * one_stop.y) - (one_start.y * one_stop.x);
    let value_two = (two_start.x * two_stop.y) - (two_start.y * two_stop.x);
    CurvaturePoint {
        x: ((value_one * (two_start.x - two_stop.x)) - ((one_start.x - one_stop.x) * value_two))
            / denominator,
        y: ((value_one * (two_start.y - two_stop.y)) - ((one_start.y - one_stop.y) * value_two))
            / denominator,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InspectedLineFilament {
    pub id: usize,
    pub members: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InspectedStaff {
    pub id: usize,
    pub left_abscissa: usize,
    pub lines: Vec<InspectedLineFilament>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CrossingChunkRemoval {
    pub section_ids: Vec<usize>,
    pub slope: f64,
    pub relative_slope: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CrossingLineUpdate {
    pub staff_id: usize,
    pub filament_id: usize,
    /// Chunk order, with section IDs in `Section.byFullAbscissa` order.
    pub chunks: Vec<CrossingChunkRemoval>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinesInspectorReport {
    pub updates: Vec<CrossingLineUpdate>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LinesInspectorError<E> {
    MissingChunkStart {
        section_id: usize,
    },
    InsufficientGlyphPixels {
        section_ids: Vec<usize>,
        count: usize,
    },
    MissingSectionDuringRemoval {
        section_id: usize,
    },
    Recompute(E),
}

impl<E: fmt::Display> fmt::Display for LinesInspectorError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunkStart { section_id } => {
                write!(
                    formatter,
                    "section {section_id} did not start a crossing chunk"
                )
            }
            Self::InsufficientGlyphPixels { section_ids, count } => write!(
                formatter,
                "crossing chunk {section_ids:?} has only {count} distinct pixels"
            ),
            Self::MissingSectionDuringRemoval { section_id } => {
                write!(formatter, "section {section_id} disappeared before removal")
            }
            Self::Recompute(error) => write!(formatter, "filament recomputation failed: {error}"),
        }
    }
}

impl<E: Error + 'static> Error for LinesInspectorError<E> {}

/// Port Java `LinesRetriever.LinesInspector.process` with real section pixels.
///
/// `recompute` is invoked exactly once after all flagged sections have been
/// deleted from a changed filament. A callback failure retains those deletions,
/// matching the production method's non-transactional mutation.
pub fn inspect_crossing_chunks<E>(
    staves: &mut [InspectedStaff],
    minimum_offset: usize,
    global_slope: f64,
    minimum_chunk_slope: f64,
    mut recompute: impl FnMut(usize, usize, &[Section]) -> Result<(), E>,
) -> Result<LinesInspectorReport, LinesInspectorError<E>> {
    inspect_crossing_chunks_detailed(
        staves,
        minimum_offset,
        global_slope,
        minimum_chunk_slope,
        |staff_id, filament_id, _, remaining| recompute(staff_id, filament_id, remaining),
    )
}

/// Detailed form of [`inspect_crossing_chunks`] used by concrete adapters.
///
/// The callback observes the ordered chunks after their sections have been
/// removed and before the line update is reported successful. This exposes
/// Java's exact non-transactional mutation boundary without changing the
/// simpler kernel API.
pub fn inspect_crossing_chunks_detailed<E>(
    staves: &mut [InspectedStaff],
    minimum_offset: usize,
    global_slope: f64,
    minimum_chunk_slope: f64,
    mut recompute: impl FnMut(usize, usize, &[CrossingChunkRemoval], &[Section]) -> Result<(), E>,
) -> Result<LinesInspectorReport, LinesInspectorError<E>> {
    let mut updates = Vec::new();

    for staff in staves {
        let x_min = staff.left_abscissa + minimum_offset;
        for filament in &mut staff.lines {
            // SectionCompound members are a TreeSet(Section.byFullAbscissa).
            let mut ordered = (0..filament.members.len()).collect::<Vec<_>>();
            ordered.sort_by_key(|&index| {
                let section = &filament.members[index];
                let bounds = section.bounds();
                (bounds.x, bounds.y, section.id())
            });

            let mut flagged = Vec::new();
            let mut chunk = Vec::new();
            let mut x_end = 0;

            for index in ordered {
                let section = &filament.members[index];
                let bounds = section.bounds();
                if bounds.x < x_min {
                    continue;
                }

                if bounds.x > x_end {
                    check_crossing_chunk(
                        &chunk,
                        &filament.members,
                        global_slope,
                        minimum_chunk_slope,
                        &mut flagged,
                    )?;
                    chunk.clear();
                    x_end = 0;
                } else if chunk.is_empty() {
                    // With production xMin this is unreachable, but Java would
                    // dereference a null chunk here. Make the failure explicit.
                    return Err(LinesInspectorError::MissingChunkStart {
                        section_id: section.id(),
                    });
                }

                chunk.push(index);
                x_end = x_end.max(bounds.x + bounds.width - 1);
            }
            check_crossing_chunk(
                &chunk,
                &filament.members,
                global_slope,
                minimum_chunk_slope,
                &mut flagged,
            )?;

            if flagged.is_empty() {
                continue;
            }

            // Java traverses toRemove chunks, then each chunk's sorted members.
            for removal in &flagged {
                for &section_id in &removal.section_ids {
                    let Some(index) = filament
                        .members
                        .iter()
                        .position(|section| section.id() == section_id)
                    else {
                        return Err(LinesInspectorError::MissingSectionDuringRemoval {
                            section_id,
                        });
                    };
                    filament.members.remove(index);
                }
            }

            recompute(staff.id, filament.id, &flagged, &filament.members)
                .map_err(LinesInspectorError::Recompute)?;
            updates.push(CrossingLineUpdate {
                staff_id: staff.id,
                filament_id: filament.id,
                chunks: flagged,
            });
        }
    }

    Ok(LinesInspectorReport { updates })
}

fn check_crossing_chunk<E>(
    chunk: &[usize],
    members: &[Section],
    global_slope: f64,
    minimum_chunk_slope: f64,
    flagged: &mut Vec<CrossingChunkRemoval>,
) -> Result<(), LinesInspectorError<E>> {
    if chunk.len() <= 1 {
        return Ok(());
    }
    let sections = chunk
        .iter()
        .map(|&index| &members[index])
        .collect::<Vec<_>>();
    let slope = basic_line_pixel_slope(&sections).map_err(|count| {
        LinesInspectorError::InsufficientGlyphPixels {
            section_ids: sections.iter().map(|section| section.id()).collect(),
            count,
        }
    })?;
    let relative_slope = slope - global_slope;
    let magnitude = relative_slope.abs();
    if matches!(
        magnitude.partial_cmp(&minimum_chunk_slope),
        Some(std::cmp::Ordering::Equal | std::cmp::Ordering::Greater)
    ) {
        flagged.push(CrossingChunkRemoval {
            section_ids: sections.iter().map(|section| section.id()).collect(),
            slope,
            relative_slope,
        });
    }
    Ok(())
}

/// Reproduce the `Glyph`/`BasicLine` pixel regression used by LinesInspector.
fn basic_line_pixel_slope(sections: &[&Section]) -> Result<f64, usize> {
    // SectionCompound.toBuffer unions pixels before rebuilding its RunTable.
    let mut pixels = BTreeSet::new();
    for section in sections {
        for (offset, run) in section.runs().iter().enumerate() {
            let position = section.first_pos() + offset;
            for coordinate in run.start..=run.stop() {
                let pixel = match section.orientation() {
                    Orientation::Horizontal => (coordinate, position),
                    Orientation::Vertical => (position, coordinate),
                };
                pixels.insert(pixel);
            }
        }
    }
    if pixels.len() < 2 {
        return Err(pixels.len());
    }

    let mut sum_x = 0.0;
    let mut sum_x2 = 0.0;
    let mut sum_xy = 0.0;
    let mut sum_y = 0.0;
    let mut sum_y2 = 0.0;
    let mut x_min = f64::MAX;
    let mut x_max = f64::MIN_POSITIVE;
    let mut y_min = f64::MAX;
    let mut y_max = f64::MIN_POSITIVE;
    for &(x, y) in &pixels {
        let x = x as f64;
        let y = y as f64;
        sum_x += x;
        sum_x2 += x * x;
        sum_xy += x * y;
        sum_y += y;
        sum_y2 += y * y;
        x_min = x_min.min(x);
        x_max = x_max.max(x);
        y_min = y_min.min(y);
        y_max = y_max.max(y);
    }
    let count = pixels.len() as f64;
    let horizontal_denominator = (count * sum_x2) - (sum_x * sum_x);
    let vertical_denominator = (count * sum_y2) - (sum_y * sum_y);
    let covariance = (count * sum_xy) - (sum_x * sum_y);

    if horizontal_denominator.abs() >= vertical_denominator.abs() {
        let mut a = covariance / horizontal_denominator;
        let mut b = -1.0;
        let mut c = ((sum_y * sum_x2) - (sum_x * sum_xy)) / horizontal_denominator;
        let norm = a.hypot(b);
        a /= norm;
        b /= norm;
        c /= norm;
        let y_one = (((-a * x_min) - c) / b) + 0.5;
        let y_two = (((-a * x_max) - c) / b) + 0.5;
        Ok((y_two - y_one) / ((x_max + 1.0) - x_min))
    } else {
        let mut a: f64 = -1.0;
        let mut b: f64 = covariance / vertical_denominator;
        let mut c: f64 = ((sum_x * sum_y2) - (sum_y * sum_xy)) / vertical_denominator;
        let norm = a.hypot(b);
        a /= norm;
        b /= norm;
        c /= norm;
        let x_one = (-((b * y_min) + c) / a) + 0.5;
        let x_two = (-((b * y_max) + c) / a) + 0.5;
        Ok(((y_max + 1.0) - y_min) / (x_two - x_one))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_table::{Run, RunTable};
    use crate::section::{JunctionPolicy, build_sections_from_id};
    use std::convert::Infallible;

    #[derive(Debug, Default)]
    struct RecordingExecutor {
        loaded: bool,
        calls: Vec<LineCompletionStage>,
        fail_load: bool,
        fail_at: Option<LineCompletionStage>,
        finished: usize,
    }

    impl LineCompletionExecutor for RecordingExecutor {
        type Error = &'static str;

        fn load_binary_buffer(&mut self) -> Result<(), Self::Error> {
            if self.fail_load {
                return Err("binary unavailable");
            }
            self.loaded = true;
            Ok(())
        }

        fn run_stage(&mut self, stage: LineCompletionStage) -> Result<(), Self::Error> {
            self.calls.push(stage);
            if self.fail_at == Some(stage) {
                Err("completion failed")
            } else {
                Ok(())
            }
        }

        fn finish(&mut self) {
            self.finished += 1;
        }
    }

    #[test]
    fn successful_completion_preserves_exact_java_order_and_optional_inspector() {
        let mut executor = RecordingExecutor::default();
        assert_eq!(complete_lines(&mut executor, true), Ok(()));
        assert!(executor.loaded);
        assert_eq!(executor.finished, 1);
        assert_eq!(
            executor.calls,
            [
                LineCompletionStage::DefineEndPoints,
                LineCompletionStage::IncludeDiscardedFilaments,
                LineCompletionStage::FillHolesInitial,
                LineCompletionStage::DispatchHorizontalSections,
                LineCompletionStage::IncludeThickSections,
                LineCompletionStage::IncludeThinSections,
                LineCompletionStage::PolishCurvatures,
                LineCompletionStage::FillHolesAfterPolish,
                LineCompletionStage::IncludeStickers,
                LineCompletionStage::InspectCrossingChunks,
                LineCompletionStage::FillHolesFinal,
            ]
        );

        let mut without_inspector = RecordingExecutor::default();
        complete_lines(&mut without_inspector, false).unwrap();
        assert!(
            !without_inspector
                .calls
                .contains(&LineCompletionStage::InspectCrossingChunks)
        );
        assert_eq!(
            without_inspector.calls.last(),
            Some(&LineCompletionStage::FillHolesFinal)
        );
    }

    #[test]
    fn stage_failure_runs_finally_and_retains_partial_mutation() {
        let mut executor = RecordingExecutor {
            fail_at: Some(LineCompletionStage::IncludeThinSections),
            ..RecordingExecutor::default()
        };

        assert_eq!(
            complete_lines(&mut executor, true),
            Err("completion failed")
        );
        assert_eq!(executor.finished, 1);
        assert_eq!(
            executor.calls.last(),
            Some(&LineCompletionStage::IncludeThinSections)
        );
        assert!(
            !executor
                .calls
                .contains(&LineCompletionStage::PolishCurvatures)
        );
    }

    #[test]
    fn binary_buffer_failure_happens_before_java_finally_scope() {
        let mut executor = RecordingExecutor {
            fail_load: true,
            ..RecordingExecutor::default()
        };

        assert_eq!(
            complete_lines(&mut executor, true),
            Err("binary unavailable")
        );
        assert!(!executor.loaded);
        assert!(executor.calls.is_empty());
        assert_eq!(executor.finished, 0);
    }

    fn point(x: f64, y: f64) -> CurvaturePoint {
        CurvaturePoint { x, y }
    }

    fn sharp_points() -> Vec<CurvaturePoint> {
        vec![point(0.0, 0.0), point(1.0, 1.0), point(2.0, 0.0)]
    }

    fn straight_points() -> Vec<CurvaturePoint> {
        vec![point(0.0, 0.0), point(1.0, 0.0), point(2.0, 0.0)]
    }

    fn curvature_section(
        id: usize,
        bounds: OrientedSectionBounds,
        centroid: CurvaturePoint,
    ) -> CurvatureSection {
        CurvatureSection {
            id,
            oriented_bounds: bounds,
            centroid,
        }
    }

    fn polish_state(members: Vec<CurvatureSection>) -> CurvedFilamentPolishState {
        CurvedFilamentPolishState {
            orientation: Orientation::Horizontal,
            interline: 4,
            start_point: point(0.0, 0.0),
            stop_point: point(2.0, 0.0),
            points: Some(sharp_points()),
            members,
        }
    }

    #[test]
    fn curvature_radius_and_threshold_match_java_fixture() {
        let member = curvature_section(
            7,
            OrientedSectionBounds {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            point(0.0, 0.0),
        );
        let mut at_boundary = polish_state(vec![member.clone()]);
        let report = polish_curvature(&mut at_boundary, 1, |_, _, _| {
            Ok::<_, Infallible>(straight_points())
        })
        .unwrap();
        assert!(report.removals.is_empty()); // radius == threshold is retained
        assert_eq!(report.recomputations, 0);

        let mut above_boundary = polish_state(vec![member]);
        let report = polish_curvature(&mut above_boundary, 2, |_, start, stop| {
            assert_eq!((start, stop), (point(0.0, 0.0), point(2.0, 0.0)));
            Ok::<_, Infallible>(straight_points())
        })
        .unwrap();
        assert_eq!(report.removals.len(), 1);
        assert_eq!(report.removals[0].section_id, 7);
        assert_eq!(report.removals[0].radius_point_index, 1);
        // With three points Java's idx==1 branch wins and probes endpoint 0.
        assert_eq!(report.removals[0].probe_point_index, 0);
        assert!((report.removals[0].radius - 1.0).abs() < 1e-12);
        assert_eq!(report.recomputations, 1);
    }

    #[test]
    fn probe_width_uses_java_ties_even_and_edge_contact_does_not_intersect() {
        let member = curvature_section(
            3,
            OrientedSectionBounds {
                x: 1.25,
                y: -0.25,
                width: 0.25,
                height: 0.5,
            },
            point(1.25, 0.0),
        );
        let mut interline_five = polish_state(vec![member.clone()]);
        interline_five.interline = 5; // 2.5 -> 2, probe right edge is x=1
        let report = polish_curvature(&mut interline_five, 2, |_, _, _| {
            Ok::<_, Infallible>(straight_points())
        })
        .unwrap();
        assert!(report.removals.is_empty());
        assert_eq!(interline_five.members.len(), 1);

        let mut interline_seven = polish_state(vec![member]);
        interline_seven.interline = 7; // 3.5 -> 4, probe right edge is x=2
        let report = polish_curvature(&mut interline_seven, 2, |_, _, _| {
            Ok::<_, Infallible>(straight_points())
        })
        .unwrap();
        assert_eq!(report.removals[0].section_id, 3);
    }

    #[test]
    fn intersecting_members_choose_nearest_centroid_then_stable_source_order() {
        let shared_bounds = OrientedSectionBounds {
            x: -0.5,
            y: -0.5,
            width: 1.0,
            height: 1.0,
        };
        let farther = curvature_section(1, shared_bounds, point(0.4, 0.0));
        let nearer = curvature_section(2, shared_bounds, point(0.1, 0.0));
        let mut state = polish_state(vec![farther, nearer]);
        let mut calls = 0;
        let report = polish_curvature(&mut state, 2, |_, _, _| {
            calls += 1;
            Ok::<_, Infallible>(straight_points())
        })
        .unwrap();
        assert_eq!(report.removals[0].section_id, 2);
        assert_eq!(calls, 1);

        let first = curvature_section(10, shared_bounds, point(0.2, 0.0));
        let second = curvature_section(11, shared_bounds, point(-0.2, 0.0));
        let mut tied = polish_state(vec![first, second]);
        let report = polish_curvature(&mut tied, 2, |_, _, _| {
            Ok::<_, Infallible>(straight_points())
        })
        .unwrap();
        assert_eq!(report.removals[0].section_id, 10);
    }

    #[test]
    fn vertical_orientation_swaps_only_probe_geometry() {
        let member = curvature_section(
            5,
            OrientedSectionBounds {
                // Absolute endpoint (0, 0) is unchanged by this fixture, so
                // use a translated point set to make the swap observable.
                x: 19.0,
                y: 9.0,
                width: 2.0,
                height: 2.0,
            },
            point(10.0, 20.0),
        );
        let mut state = CurvedFilamentPolishState {
            orientation: Orientation::Vertical,
            interline: 4,
            start_point: point(10.0, 20.0),
            stop_point: point(12.0, 20.0),
            points: Some(vec![
                point(10.0, 20.0),
                point(11.0, 21.0),
                point(12.0, 20.0),
            ]),
            members: vec![member],
        };
        let report = polish_curvature(&mut state, 2, |_, _, _| {
            Ok::<_, Infallible>(vec![
                point(10.0, 20.0),
                point(11.0, 20.0),
                point(12.0, 20.0),
            ])
        })
        .unwrap();
        assert_eq!(report.removals[0].section_id, 5);
    }

    #[test]
    fn repeated_removals_recompute_after_each_mutation_with_preserved_endpoints() {
        let bounds = OrientedSectionBounds {
            x: -0.5,
            y: -0.5,
            width: 1.0,
            height: 1.0,
        };
        let mut state = polish_state(vec![
            curvature_section(1, bounds, point(0.0, 0.0)),
            curvature_section(2, bounds, point(0.25, 0.0)),
        ]);
        let mut member_counts = Vec::new();
        let report = polish_curvature(&mut state, 2, |members, start, stop| {
            member_counts.push(members.len());
            assert_eq!(start, point(0.0, 0.0));
            assert_eq!(stop, point(2.0, 0.0));
            Ok::<_, Infallible>(if members.is_empty() {
                straight_points()
            } else {
                sharp_points()
            })
        })
        .unwrap();

        assert_eq!(
            report
                .removals
                .iter()
                .map(|removal| removal.section_id)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(member_counts, [1, 0]);
        assert_eq!(report.recomputations, 2);
        assert_eq!(state.points, Some(straight_points()));
        assert_eq!(state.start_point, point(0.0, 0.0));
        assert_eq!(state.stop_point, point(2.0, 0.0));
    }

    #[test]
    fn recompute_failure_is_explicit_and_retains_prior_removal() {
        let member = curvature_section(
            9,
            OrientedSectionBounds {
                x: -0.5,
                y: -0.5,
                width: 1.0,
                height: 1.0,
            },
            point(0.0, 0.0),
        );
        let mut state = polish_state(vec![member]);
        let result = polish_curvature(&mut state, 2, |_, _, _| Err("probe failed"));

        assert_eq!(result, Err(CurvaturePolishError::Recompute("probe failed")));
        assert!(state.members.is_empty());
        assert!(state.points.is_none());
        assert_eq!(state.start_point, point(0.0, 0.0));
        assert_eq!(state.stop_point, point(2.0, 0.0));
    }

    #[test]
    fn invalid_recomputed_point_count_fails_without_installing_cache() {
        let mut state = polish_state(Vec::new());
        state.points = None;
        let result = polish_curvature(&mut state, 2, |_, _, _| {
            Ok::<_, Infallible>(vec![point(0.0, 0.0)])
        });

        assert_eq!(
            result,
            Err(CurvaturePolishError::InvalidPointCount { count: 1 })
        );
        assert!(state.points.is_none());
    }

    fn pixel_section(id: usize, rows: &[(usize, usize, usize)]) -> Section {
        let width = rows
            .iter()
            .map(|(_, start, length)| start + length)
            .max()
            .unwrap_or(1)
            + 1;
        let height = rows.iter().map(|(y, _, _)| y + 1).max().unwrap_or(1) + 1;
        let mut table = RunTable::new(Orientation::Horizontal, width, height).unwrap();
        for &(y, start, length) in rows {
            assert!(table.add_run(y, Run::new(start, length)).unwrap());
        }
        build_sections_from_id(&table, JunctionPolicy::All, id).remove(0)
    }

    fn diagonal_chunk(first_id: usize, x: usize, y: usize) -> [Section; 2] {
        [
            pixel_section(first_id, &[(y, x, 2)]),
            pixel_section(first_id + 1, &[(y + 1, x + 1, 2)]),
        ]
    }

    fn inspected_staff(members: Vec<Section>) -> Vec<InspectedStaff> {
        vec![InspectedStaff {
            id: 1,
            left_abscissa: 0,
            lines: vec![InspectedLineFilament { id: 2, members }],
        }]
    }

    #[test]
    fn crossing_slope_comes_from_section_pixels_and_threshold_equality_removes() {
        let chunk = diagonal_chunk(1, 10, 10);
        let slope = basic_line_pixel_slope(&[&chunk[0], &chunk[1]]).unwrap();
        // BasicLine.toCenterLine extends the x span to the pixel limit, so the
        // center-line slope is 1/3 rather than the raw regression slope 1/2.
        assert!((slope - (1.0 / 3.0)).abs() < 1e-12);

        let mut at_boundary = inspected_staff(chunk.to_vec());
        let mut recomputes = 0;
        let report =
            inspect_crossing_chunks(&mut at_boundary, 10, 0.0, slope.abs(), |_, _, remaining| {
                recomputes += 1;
                assert!(remaining.is_empty());
                Ok::<_, Infallible>(())
            })
            .unwrap();
        assert_eq!(report.updates[0].chunks[0].section_ids, [1, 2]);
        assert_eq!(report.updates[0].chunks[0].slope, slope);
        assert_eq!(recomputes, 1);

        let mut above_boundary = inspected_staff(chunk.to_vec());
        let next_threshold = f64::from_bits(slope.abs().to_bits() + 1);
        let report =
            inspect_crossing_chunks(&mut above_boundary, 10, 0.0, next_threshold, |_, _, _| {
                Ok::<_, Infallible>(())
            })
            .unwrap();
        assert!(report.updates.is_empty());
        assert_eq!(above_boundary[0].lines[0].members.len(), 2);
    }

    #[test]
    fn header_exclusion_and_inclusive_x_end_preserve_member_order() {
        let header = pixel_section(9, &[(8, 9, 2)]);
        let [first, second] = diagonal_chunk(1, 10, 10);
        // Starts strictly after prior inclusive xEnd=12, so it is a new,
        // single-member chunk and cannot be removed.
        let singleton = pixel_section(8, &[(12, 13, 2), (13, 14, 2)]);
        let mut staves = inspected_staff(vec![second, header, singleton, first]);
        let mut remaining_ids = Vec::new();

        let report = inspect_crossing_chunks(&mut staves, 10, 0.0, 0.04, |_, _, remaining| {
            remaining_ids = remaining.iter().map(Section::id).collect();
            Ok::<_, Infallible>(())
        })
        .unwrap();

        assert_eq!(report.updates[0].chunks.len(), 1);
        assert_eq!(report.updates[0].chunks[0].section_ids, [1, 2]);
        // Mutation removes in flagged chunk order while preserving unrelated
        // filament member order. Header x=9 and singleton x=13 survive.
        assert_eq!(remaining_ids, [9, 8]);
        assert_eq!(
            staves[0].lines[0]
                .members
                .iter()
                .map(Section::id)
                .collect::<Vec<_>>(),
            [9, 8]
        );
    }

    #[test]
    fn one_member_chunk_is_never_tested_even_with_steep_pixels() {
        let steep = pixel_section(1, &[(10, 10, 2), (11, 11, 2)]);
        assert!(basic_line_pixel_slope(&[&steep]).unwrap().abs() > 0.04);
        let mut staves = inspected_staff(vec![steep]);
        let mut recomputes = 0;

        let report = inspect_crossing_chunks(&mut staves, 10, 0.0, 0.04, |_, _, _| {
            recomputes += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap();

        assert!(report.updates.is_empty());
        assert_eq!(recomputes, 0);
        assert_eq!(staves[0].lines[0].members.len(), 1);
    }

    #[test]
    fn multiple_flagged_chunks_delete_in_chunk_order_and_recompute_once() {
        let [a1, a2] = diagonal_chunk(10, 10, 10);
        let [b1, b2] = diagonal_chunk(20, 20, 20);
        let survivor = pixel_section(30, &[(30, 30, 2)]);
        let mut staves = inspected_staff(vec![b2, survivor, a2, b1, a1]);
        let mut callbacks = Vec::new();

        let report = inspect_crossing_chunks(&mut staves, 10, 0.0, 0.04, |staff, line, rest| {
            callbacks.push((
                staff,
                line,
                rest.iter().map(Section::id).collect::<Vec<_>>(),
            ));
            Ok::<_, Infallible>(())
        })
        .unwrap();

        assert_eq!(
            report.updates[0]
                .chunks
                .iter()
                .map(|chunk| chunk.section_ids.clone())
                .collect::<Vec<_>>(),
            [vec![10, 11], vec![20, 21]]
        );
        assert_eq!(callbacks, [(1, 2, vec![30])]);
    }

    #[test]
    fn global_slope_is_subtracted_before_inclusive_magnitude_gate() {
        let chunk = diagonal_chunk(1, 10, 10);
        let slope = basic_line_pixel_slope(&[&chunk[0], &chunk[1]]).unwrap();
        let global_slope = 0.1;
        let relative_slope = slope - global_slope;
        let mut staves = inspected_staff(chunk.to_vec());
        let report = inspect_crossing_chunks(
            &mut staves,
            10,
            global_slope,
            relative_slope.abs(),
            |_, _, _| Ok::<_, Infallible>(()),
        )
        .unwrap();
        assert_eq!(report.updates.len(), 1);
        assert_eq!(report.updates[0].chunks[0].relative_slope, relative_slope);
    }

    #[test]
    fn degenerate_union_failure_is_explicit_and_non_mutating() {
        let one = pixel_section(1, &[(10, 10, 1)]);
        let two = pixel_section(2, &[(10, 10, 1)]);
        let mut staves = inspected_staff(vec![two, one]);
        let result =
            inspect_crossing_chunks(
                &mut staves,
                10,
                0.0,
                0.04,
                |_, _, _| Ok::<_, Infallible>(()),
            );

        assert_eq!(
            result,
            Err(LinesInspectorError::InsufficientGlyphPixels {
                section_ids: vec![1, 2],
                count: 1,
            })
        );
        assert_eq!(staves[0].lines[0].members.len(), 2);
    }

    #[test]
    fn recompute_failure_retains_ordered_section_deletions() {
        let chunk = diagonal_chunk(1, 10, 10);
        let mut staves = inspected_staff(chunk.to_vec());
        let result = inspect_crossing_chunks(&mut staves, 10, 0.0, 0.04, |_, _, remaining| {
            assert!(remaining.is_empty());
            Err("line failed")
        });

        assert_eq!(result, Err(LinesInspectorError::Recompute("line failed")));
        assert!(staves[0].lines[0].members.is_empty());
    }
}
