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

        let points = state.points.as_ref().expect("points recomputed above");
        if points.len() < 2 {
            return Err(CurvaturePolishError::InvalidPointCount {
                count: points.len(),
            });
        }

        let mut minimum = f64::from(i32::MAX);
        let mut minimum_index = 0;
        for index in 1..points.len() - 1 {
            let radius = radius_at(points, index);
            // Strict comparison preserves the first index on equal radii and
            // ignores NaN exactly like Java's `minRadius > radius`.
            if minimum > radius {
                minimum = radius;
                minimum_index = index;
            }
        }

        if !matches!(
            minimum.partial_cmp(&f64::from(minimum_radius)),
            Some(std::cmp::Ordering::Less)
        ) {
            break;
        }

        let radius_index = minimum_index;
        let probe_index = if minimum_index == 1 {
            0
        } else if minimum_index == points.len() - 2 {
            points.len() - 1
        } else {
            minimum_index
        };
        let point = points[probe_index];
        let oriented_point = orient_point(point, state.orientation);
        // InterlineScale.toPixels(interline, Fraction(0.5)) uses Math.rint.
        let probe_width = (f64::from(state.interline) * 0.5).round_ties_even();
        let probe = OrientedSectionBounds {
            x: oriented_point.x - probe_width / 2.0,
            y: oriented_point.y - probe_width / 2.0,
            width: probe_width,
            height: probe_width,
        };

        let mut selected = None;
        let mut selected_distance = f64::NAN;
        for (index, section) in state.members.iter().enumerate() {
            if !probe.intersects(section.oriented_bounds) {
                continue;
            }
            let distance = point.distance(section.centroid);
            if selected.is_none() || distance.total_cmp(&selected_distance).is_lt() {
                selected = Some(index);
                selected_distance = distance;
            }
        }

        let Some(section_index) = selected else {
            break;
        };
        let section = state.members.remove(section_index);
        removals.push(CurvatureRemoval {
            section_id: section.id,
            radius_point_index: radius_index,
            probe_point_index: probe_index,
            radius: minimum,
        });
        // removeSection invalidates points and spline; the next do/while pass
        // restores the original endpoints and recomputes from remaining ink.
        state.points = None;
    }

    Ok(CurvaturePolishReport {
        removals,
        recomputations,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
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
}
