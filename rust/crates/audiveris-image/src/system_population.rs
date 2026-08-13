// SPDX-License-Identifier: AGPL-3.0-or-later

//! Headless ownership boundary for Java `SystemManager.populateSystems`.
//!
//! Population is deliberately non-transactional. Java mutates coordinates,
//! areas, section containers, indentation flags, pages, and page references in
//! order; an unchecked failure leaves every earlier mutation visible.

use std::collections::BTreeMap;

/// Stable identity used at the sheet/system boundary.
pub type SystemId = usize;

/// Calls made by `SystemManager.populateSystems`, in production order.
pub trait SystemPopulationExecutor {
    type Error;

    /// System identities in `sheet.getSystems()` order.
    fn system_ids(&self) -> Vec<SystemId>;

    /// Staff identities in `system.getStaves()` order.
    fn staff_ids(&self, system_id: SystemId) -> Vec<usize>;

    fn update_system_coordinates(&mut self, system_id: SystemId) -> Result<(), Self::Error>;
    fn compute_system_area(&mut self, system_id: SystemId) -> Result<(), Self::Error>;
    fn compute_staff_area(&mut self, staff_id: usize) -> Result<(), Self::Error>;
    fn dispatch_horizontal_sections(&mut self) -> Result<(), Self::Error>;
    fn dispatch_vertical_sections(&mut self) -> Result<(), Self::Error>;
    fn check_indentations(&mut self) -> Result<(), Self::Error>;
    fn allocate_pages(&mut self) -> Result<(), Self::Error>;
    fn report_results(&mut self) -> Result<(), Self::Error>;
}

/// Execute the exact outer lifecycle of Java `SystemManager.populateSystems`.
///
/// The system list is fetched again for the staff-area pass, matching the two
/// separate enhanced-for loops in Java. No rollback or finalizer exists.
pub fn populate_systems<Executor>(executor: &mut Executor) -> Result<(), Executor::Error>
where
    Executor: SystemPopulationExecutor,
{
    for system_id in executor.system_ids() {
        executor.update_system_coordinates(system_id)?;
        executor.compute_system_area(system_id)?;
    }

    for system_id in executor.system_ids() {
        for staff_id in executor.staff_ids(system_id) {
            executor.compute_staff_area(staff_id)?;
        }
    }

    executor.dispatch_horizontal_sections()?;
    executor.dispatch_vertical_sections()?;
    executor.check_indentations()?;
    executor.allocate_pages()?;
    executor.report_results()?;
    Ok(())
}

/// One lag section at the ownership boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopulationSection {
    pub id: usize,
    pub centroid_x: f64,
    pub centroid_y: f64,
}

/// Section containers owned by one system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemSectionOwnership {
    pub system_id: SystemId,
    pub horizontal_sections: Vec<usize>,
    pub vertical_sections: Vec<usize>,
}

/// Which production lag is being dispatched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopulationLag {
    Horizontal,
    Vertical,
}

/// Reproduce both Java section dispatch methods.
///
/// Every system container for the selected lag is cleared first. Sections are
/// then visited in lag entity order and linked to every containing system in
/// system order. A centroid on no system is silently left unowned.
pub fn dispatch_sections(
    lag: PopulationLag,
    systems: &mut [SystemSectionOwnership],
    sections: &[PopulationSection],
    mut contains: impl FnMut(SystemId, f64, f64) -> bool,
) {
    for system in systems.iter_mut() {
        match lag {
            PopulationLag::Horizontal => system.horizontal_sections.clear(),
            PopulationLag::Vertical => system.vertical_sections.clear(),
        }
    }

    for section in sections {
        for system in systems.iter_mut() {
            if contains(system.system_id, section.centroid_x, section.centroid_y) {
                match lag {
                    PopulationLag::Horizontal => system.horizontal_sections.push(section.id),
                    PopulationLag::Vertical => system.vertical_sections.push(section.id),
                }
            }
        }
    }
}

/// Geometry needed by Java's indentation decision after system areas exist.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopulationSystemGeometry {
    pub system_id: SystemId,
    pub left: i32,
    pub width: i32,
    pub top: i32,
    pub bottom: i32,
    pub area_left: i32,
    /// Deskewed x coordinate of `(left, top)`.
    pub deskewed_upper_left_x: f64,
}

impl PopulationSystemGeometry {
    fn x_overlaps(self, other: Self) -> bool {
        let common_left = self.left.max(other.left);
        let common_right = (self.left + self.width - 1).min(other.left + other.width - 1);
        common_right > common_left
    }

    fn y_overlaps(self, other: Self) -> bool {
        self.top.max(other.top) < self.bottom.min(other.bottom)
    }
}

/// Exact result of Java `checkIndentation` using already-deskewed coordinates.
#[must_use]
pub fn check_population_indentation(
    systems: &[PopulationSystemGeometry],
    system_index: usize,
    minimum_indentation: f64,
) -> bool {
    let current = systems[system_index];
    // A system beside another system is only eligible when it owns the left
    // edge of its area slice.
    if current.area_left != 0 {
        return false;
    }

    for direction in [-1_isize, 1] {
        let neighbors = vertical_neighbors(systems, system_index, direction);
        if let Some(other_index) = neighbors.first() {
            let delta = current.deskewed_upper_left_x - systems[*other_index].deskewed_upper_left_x;
            if delta >= minimum_indentation {
                return true;
            }
        }
    }
    false
}

/// What the neighbour walks need of a placed rectangle.
///
/// Java runs the same search twice over different collections:
/// `SystemManager.vertNeighbors` over systems and `StaffManager.vertNeighbors`
/// over staves, with identical bodies. This is that shape, so the two callers
/// share one implementation rather than one being a transcription of the other
/// that can drift.
trait Placed: Copy {
    fn sort_key(self) -> usize;
    fn x_overlaps(self, other: Self) -> bool;
    fn y_overlaps(self, other: Self) -> bool;
}

impl Placed for PopulationSystemGeometry {
    fn sort_key(self) -> usize {
        self.system_id
    }

    fn x_overlaps(self, other: Self) -> bool {
        Self::x_overlaps(self, other)
    }

    fn y_overlaps(self, other: Self) -> bool {
        Self::y_overlaps(self, other)
    }
}

fn vertical_neighbors<P: Placed>(
    systems: &[P],
    current_index: usize,
    direction: isize,
) -> Vec<usize> {
    let current = systems[current_index];
    let mut cursor = current_index as isize + direction;
    let mut first = None;
    while cursor >= 0 && (cursor as usize) < systems.len() {
        if current.x_overlaps(systems[cursor as usize]) {
            first = Some(cursor as usize);
            break;
        }
        cursor += direction;
    }

    let Some(first) = first else {
        return Vec::new();
    };
    let mut neighbors = vec![first];
    for horizontal_direction in [-1_isize, 1] {
        let mut next = first;
        while let Some(found) = horizontal_neighbor(systems, next, horizontal_direction) {
            neighbors.push(found);
            next = found;
        }
    }
    // Java sorts the collected row by SystemInfo.byId, and callers use the
    // first item rather than necessarily the initially intersecting item.
    neighbors.sort_by_key(|index| systems[*index].sort_key());
    neighbors
}

fn horizontal_neighbor<P: Placed>(
    systems: &[P],
    current_index: usize,
    direction: isize,
) -> Option<usize> {
    let current = systems[current_index];
    let mut cursor = current_index as isize + direction;
    while cursor >= 0 && (cursor as usize) < systems.len() {
        if current.y_overlaps(systems[cursor as usize]) {
            return Some(cursor as usize);
        }
        cursor += direction;
    }
    None
}

/// One exact path segment from a staff-line spline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoundarySegment {
    Line {
        start: (f64, f64),
        end: (f64, f64),
    },
    Quadratic {
        start: (f64, f64),
        control: (f64, f64),
        end: (f64, f64),
    },
    Cubic {
        start: (f64, f64),
        control1: (f64, f64),
        control2: (f64, f64),
        end: (f64, f64),
    },
}

impl BoundarySegment {
    fn start(self) -> (f64, f64) {
        match self {
            Self::Line { start, .. }
            | Self::Quadratic { start, .. }
            | Self::Cubic { start, .. } => start,
        }
    }

    fn end(self) -> (f64, f64) {
        match self {
            Self::Line { end, .. } | Self::Quadratic { end, .. } | Self::Cubic { end, .. } => end,
        }
    }

    fn translated_y(self, delta: f64) -> Self {
        let point = |(x, y): (f64, f64)| (x, y + delta);
        match self {
            Self::Line { start, end } => Self::Line {
                start: point(start),
                end: point(end),
            },
            Self::Quadratic {
                start,
                control,
                end,
            } => Self::Quadratic {
                start: point(start),
                control: point(control),
                end: point(end),
            },
            Self::Cubic {
                start,
                control1,
                control2,
                end,
            } => Self::Cubic {
                start: point(start),
                control1: point(control1),
                control2: point(control2),
                end: point(end),
            },
        }
    }

    fn reversed(self) -> Self {
        match self {
            Self::Line { start, end } => Self::Line {
                start: end,
                end: start,
            },
            Self::Quadratic {
                start,
                control,
                end,
            } => Self::Quadratic {
                start: end,
                control,
                end: start,
            },
            Self::Cubic {
                start,
                control1,
                control2,
                end,
            } => Self::Cubic {
                start: end,
                control1: control2,
                control2: control1,
                end: start,
            },
        }
    }

    fn y_at_x(self, x: f64) -> Option<f64> {
        let (start_x, _) = self.start();
        let (end_x, _) = self.end();
        if x < start_x || x > end_x {
            return None;
        }
        if end_x == start_x {
            return None;
        }
        let mut low = 0.0;
        let mut high = 1.0;
        // Staff splines are x-monotone. Bisection evaluates their actual
        // quadratic/cubic x controls, unlike `GeoPath.yAtX`'s convenience
        // parameterization, because Java Area containment uses path geometry.
        for _ in 0..64 {
            let middle = (low + high) / 2.0;
            if self.point_at(middle).0 < x {
                low = middle;
            } else {
                high = middle;
            }
        }
        Some(self.point_at((low + high) / 2.0).1)
    }

    fn point_at(self, t: f64) -> (f64, f64) {
        let u = 1.0 - t;
        match self {
            Self::Line { start, end } => ((u * start.0) + (t * end.0), (u * start.1) + (t * end.1)),
            Self::Quadratic {
                start,
                control,
                end,
            } => (
                (u * u * start.0) + (2.0 * u * t * control.0) + (t * t * end.0),
                (u * u * start.1) + (2.0 * u * t * control.1) + (t * t * end.1),
            ),
            Self::Cubic {
                start,
                control1,
                control2,
                end,
            } => (
                (u * u * u * start.0)
                    + (3.0 * u * u * t * control1.0)
                    + (3.0 * u * t * t * control2.0)
                    + (t * t * t * end.0),
                (u * u * u * start.1)
                    + (3.0 * u * u * t * control1.1)
                    + (3.0 * u * t * t * control2.1)
                    + (t * t * t * end.1),
            ),
        }
    }
}

/// Ordered spline path for one first or last staff line.
#[derive(Clone, Debug, PartialEq)]
pub struct StaffBoundary {
    pub segments: Vec<BoundarySegment>,
}

impl StaffBoundary {
    /// The boundary's ordinate at `x`, evaluating the actual spline segment
    /// rather than interpolating between its endpoints.
    ///
    /// `Staff.distanceTo` calls `LineInfo.yAt(x)`, which walks the spline, and
    /// a polyline through the same points answers differently mid-segment --
    /// enough to pick the wrong staff where two areas overlap and the distance
    /// is the tie-break.
    #[must_use]
    pub fn y_at_x(&self, x: f64) -> Option<f64> {
        boundary_y_at_x(self, x)
    }

    /// Java `LineInfo.yAt(double)`: evaluate within the spline and extrapolate
    /// outside it using the chord through the two endpoints.
    ///
    /// Both branches follow `StaffLine.yAt` op for op, because callers compare
    /// the result against Java bit for bit. The in-range branch is
    /// `getSpline().yAtX(x)`, i.e. `GeoPath.yAtX`'s convenience parameter --
    /// *not* the true-curve bisection of [`Self::y_at_x`], which answers a few
    /// ulp differently on a curved segment. The out-of-range branch forms the
    /// slope first and multiplies second, as Java does; folding it into one
    /// multiply-then-divide expression moves the last ulp.
    #[must_use]
    pub fn y_at_x_ext(&self, x: f64) -> f64 {
        let start = self.first_point();
        let stop = self.last_point();
        if x < start.0 || x > stop.0 {
            let dx = stop.0 - start.0;
            if dx == 0.0 {
                start.1
            } else {
                let slope = (stop.1 - start.1) / dx;
                start.1 + (slope * (x - start.0))
            }
        } else {
            self.geopath_y_at_x(x).unwrap_or(start.1)
        }
    }

    /// Java `GeoPath.yAtX(double)`, whose curve parameter is derived from the
    /// segment endpoints' x coordinates rather than by inverting curve x(t).
    /// `Staff.buildAllLedgerLines()` uses this convenience evaluation.
    #[must_use]
    pub fn geopath_y_at_x(&self, x: f64) -> Option<f64> {
        let segment = self
            .segments
            .iter()
            .copied()
            .find(|segment| x <= segment.end().0)?;
        let start = segment.start();
        let stop = segment.end();
        let t = (x - start.0) / (stop.0 - start.0);
        let u = 1.0 - t;
        Some(match segment {
            BoundarySegment::Line { .. } => start.1 + (t * (stop.1 - start.1)),
            BoundarySegment::Quadratic { control, .. } => {
                (start.1 * u * u) + (2.0 * control.1 * t * u) + (stop.1 * t * t)
            }
            BoundarySegment::Cubic {
                control1, control2, ..
            } => {
                (start.1 * u * u * u)
                    + (3.0 * control1.1 * t * u * u)
                    + (3.0 * control2.1 * t * t * u)
                    + (stop.1 * t * t * t)
            }
        })
    }

    /// Translate every path coordinate vertically, as Java's
    /// `new GeoPath(line, AffineTransform.getTranslateInstance(0, dy))`.
    #[must_use]
    pub fn translated_y(&self, delta: f64) -> Self {
        Self {
            segments: self
                .segments
                .iter()
                .copied()
                .map(|segment| segment.translated_y(delta))
                .collect(),
        }
    }

    /// Java `StaffLine.getBounds()`: a rectangle around the defining spline
    /// points, with a minimum height of one pixel.
    #[must_use]
    pub fn defining_point_bounds(&self) -> (i32, i32, i32, i32) {
        let first = self.first_point();
        let mut min_x = first.0;
        let mut max_x = first.0;
        let mut min_y = first.1;
        let mut max_y = first.1;
        for point in self.segments.iter().copied().map(BoundarySegment::end) {
            min_x = min_x.min(point.0);
            max_x = max_x.max(point.0);
            min_y = min_y.min(point.1);
            max_y = max_y.max(point.1);
        }
        let x = min_x.floor() as i32;
        let y = min_y.floor() as i32;
        let right = max_x.ceil() as i32;
        let bottom = max_y.ceil() as i32;
        (
            x,
            y,
            right.saturating_sub(x),
            bottom.saturating_sub(y).max(1),
        )
    }

    fn first_point(&self) -> (f64, f64) {
        self.segments
            .first()
            .expect("a staff line has a spline segment")
            .start()
    }

    fn last_point(&self) -> (f64, f64) {
        self.segments
            .last()
            .expect("a staff line has a spline segment")
            .end()
    }

    /// Java `ReversePathIterator` geometry: segment order and direction are
    /// reversed, including the cubic control-point swap.
    #[must_use]
    pub fn reversed(&self) -> Self {
        Self {
            segments: self
                .segments
                .iter()
                .rev()
                .copied()
                .map(BoundarySegment::reversed)
                .collect(),
        }
    }
}

/// Staff boundaries associated with one system.
#[derive(Clone, Debug, PartialEq)]
pub struct SystemStaffBoundaries {
    pub first_line: StaffBoundary,
    pub last_line: StaffBoundary,
}

/// Curved system area produced by Java `computeSystemArea`.
#[derive(Clone, Debug, PartialEq)]
pub struct PopulationSystemArea {
    pub system_id: SystemId,
    pub left: i32,
    pub right: i32,
    north: StaffBoundary,
    south: StaffBoundary,
}

impl PopulationSystemArea {
    /// Java `Area.contains(Point2D)` for the x-monotone path.
    ///
    /// `java.awt.Shape` defines insideness as half-open, not exclusive: a point
    /// exactly on the boundary is inside when the space immediately adjacent in
    /// the increasing-x direction is inside, and for a horizontal segment when
    /// the space in the increasing-y direction is. So the **left and north**
    /// edges belong to the area and the **right and south** edges do not.
    ///
    /// This previously excluded all four, which no system test caught because
    /// system areas are sampled well inside their bounds. Staff areas reach the
    /// sheet edge -- a staff's north boundary is `y = 0` when nothing is above
    /// it -- and there Java assigns the point to the staff while an exclusive
    /// rule assigns it to nothing at all.
    #[must_use]
    pub fn contains(&self, x: f64, y: f64) -> bool {
        if x < f64::from(self.left) || x >= f64::from(self.right) {
            return false;
        }
        let Some(north) = boundary_y_at_x(&self.north, x) else {
            return false;
        };
        let Some(south) = boundary_y_at_x(&self.south, x) else {
            return false;
        };
        y >= north && y < south
    }

    #[must_use]
    pub fn north(&self) -> &StaffBoundary {
        &self.north
    }

    #[must_use]
    pub fn south(&self) -> &StaffBoundary {
        &self.south
    }

    /// Raw Java `wholePath` segment order before optional slice intersection:
    /// north left-to-right, a connector, south right-to-left, and closure.
    #[must_use]
    pub fn outline_segments(&self) -> Vec<BoundarySegment> {
        let mut outline = self.north.segments.clone();
        let reversed_south = self.south.reversed();
        let north_end = self.north.last_point();
        let south_start = reversed_south.first_point();
        if north_end != south_start {
            outline.push(BoundarySegment::Line {
                start: north_end,
                end: south_start,
            });
        }
        outline.extend(reversed_south.segments);
        let south_end = self.south.first_point();
        let north_start = self.north.first_point();
        if south_end != north_start {
            outline.push(BoundarySegment::Line {
                start: south_end,
                end: north_start,
            });
        }
        outline
    }
}

fn boundary_y_at_x(boundary: &StaffBoundary, x: f64) -> Option<f64> {
    boundary
        .segments
        .iter()
        .copied()
        .find_map(|segment| segment.y_at_x(x))
}

/// Build every system area in sheet order, including side-by-side slices.
///
/// `vertical_margin` is the pixel conversion of Java's `verticalAreaMargin`.
/// Neighbor rows are concatenated by system ID exactly as `getGlobalLine` does.
#[must_use]
pub fn build_population_system_areas(
    systems: &[PopulationSystemGeometry],
    staff_lines: &[SystemStaffBoundaries],
    sheet_width: i32,
    sheet_height: i32,
    vertical_margin: i32,
) -> Vec<PopulationSystemArea> {
    assert_eq!(systems.len(), staff_lines.len());
    systems
        .iter()
        .enumerate()
        .map(|(index, system)| {
            let left_neighbor = horizontal_neighbor(systems, index, -1);
            let left = left_neighbor.map_or(0, |neighbor| {
                ((systems[neighbor].left + systems[neighbor].width) + system.left) / 2
            });
            let right_neighbor = horizontal_neighbor(systems, index, 1);
            let right = right_neighbor.map_or(sheet_width, |neighbor| {
                ((system.left + system.width) + systems[neighbor].left) / 2
            });

            let aboves = vertical_neighbors(systems, index, -1);
            let north = if aboves.is_empty() {
                horizontal_boundary(left, right, 0)
            } else {
                global_boundary(&aboves, staff_lines, false, sheet_width, vertical_margin)
            };
            let belows = vertical_neighbors(systems, index, 1);
            let south = if belows.is_empty() {
                horizontal_boundary(left, right, sheet_height)
            } else {
                global_boundary(&belows, staff_lines, true, sheet_width, -vertical_margin)
            };

            PopulationSystemArea {
                system_id: system.system_id,
                left,
                right,
                north,
                south,
            }
        })
        .collect()
}

/// A staff line's path, as an area boundary.
///
/// Java walks `LineInfo.getSpline()`'s `PathIterator` to build an area, so the
/// boundary has to be the same segments rather than a resampling of the curve:
/// a natural cubic through the same points is not the same path as a polyline
/// through them, and `Area.contains` can differ near a segment join.
///
/// # Errors
///
/// Whatever `NaturalSpline::interpolate` raises for a degenerate point list.
pub fn boundary_from_points(
    points: &[(f64, f64)],
) -> Result<StaffBoundary, audiveris_core::natural_spline::SplineError> {
    use audiveris_core::natural_spline::{NaturalSpline, Segment};

    let spline = NaturalSpline::interpolate(points)?;
    Ok(StaffBoundary {
        segments: spline
            .segments()
            .iter()
            .map(|segment| match *segment {
                Segment::Line { start, end } => BoundarySegment::Line { start, end },
                Segment::Quadratic {
                    start,
                    control,
                    end,
                } => BoundarySegment::Quadratic {
                    start,
                    control,
                    end,
                },
                Segment::Cubic {
                    start,
                    control1,
                    control2,
                    end,
                } => BoundarySegment::Cubic {
                    start,
                    control1,
                    control2,
                    end,
                },
            })
            .collect(),
    })
}

/// One staff's placement, for `StaffManager.computeStaffArea`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PopulationStaffGeometry {
    pub staff_id: usize,
    pub system_id: SystemId,
    pub left: i32,
    pub width: i32,
    pub top: i32,
    pub bottom: i32,
}

impl Placed for PopulationStaffGeometry {
    fn sort_key(self) -> usize {
        self.staff_id
    }

    fn x_overlaps(self, other: Self) -> bool {
        let common_left = self.left.max(other.left);
        let common_right = (self.left + self.width - 1).min(other.left + other.width - 1);
        common_right > common_left
    }

    fn y_overlaps(self, other: Self) -> bool {
        self.top.max(other.top) < self.bottom.min(other.bottom)
    }
}

/// One staff's area, as `StaffManager.getClosestStaff` uses it.
#[derive(Clone, Debug, PartialEq)]
pub struct PopulationStaffArea {
    pub staff_id: usize,
    pub area: PopulationSystemArea,
}

/// Java `StaffManager.computeStaffArea`, for every staff.
///
/// A horizontal slice between the staves above and below, intersected with the
/// containing system's area ends. Two differences from
/// [`build_population_system_areas`] are Java's rather than simplifications:
/// there is **no vertical margin** -- the slice runs to the neighbours' own
/// lines -- and the horizontal extent comes from the system rather than from
/// midpoints between horizontal neighbours.
///
/// `system_area_ends` maps a system id to its `getAreaEnd(LEFT/RIGHT)`. Java
/// notes those "may not be known yet", in which case they are zero, and guards
/// the intersection with
/// `(left != 0) || ((right != 0) && (right != sheetWidth))`. That guard is
/// reproduced: an unknown pair leaves the staff spanning the sheet. A left
/// without a right yields a negative-width slice and therefore an empty area,
/// which is also Java's outcome and is left to fall out of `contains` rather
/// than special-cased.
///
/// # Panics
///
/// If `staves` and `staff_lines` differ in length.
#[must_use]
pub fn build_population_staff_areas(
    staves: &[PopulationStaffGeometry],
    staff_lines: &[SystemStaffBoundaries],
    system_area_ends: &dyn Fn(SystemId) -> (i32, i32),
    sheet_width: i32,
    sheet_height: i32,
) -> Vec<PopulationStaffArea> {
    assert_eq!(staves.len(), staff_lines.len());
    staves
        .iter()
        .enumerate()
        .map(|(index, staff)| {
            let aboves = vertical_neighbors(staves, index, -1);
            let north = if aboves.is_empty() {
                horizontal_boundary(0, sheet_width, 0)
            } else {
                global_boundary(&aboves, staff_lines, false, sheet_width, 0)
            };
            let belows = vertical_neighbors(staves, index, 1);
            let south = if belows.is_empty() {
                horizontal_boundary(0, sheet_width, sheet_height)
            } else {
                global_boundary(&belows, staff_lines, true, sheet_width, 0)
            };

            let (system_left, system_right) = system_area_ends(staff.system_id);
            let (left, right) =
                if system_left != 0 || (system_right != 0 && system_right != sheet_width) {
                    (system_left, system_right)
                } else {
                    (0, sheet_width)
                };

            PopulationStaffArea {
                staff_id: staff.staff_id,
                area: PopulationSystemArea {
                    system_id: staff.system_id,
                    left,
                    right,
                    north,
                    south,
                },
            }
        })
        .collect()
}

fn horizontal_boundary(left: i32, right: i32, y: i32) -> StaffBoundary {
    StaffBoundary {
        segments: vec![BoundarySegment::Line {
            start: (f64::from(left), f64::from(y)),
            end: (f64::from(right), f64::from(y)),
        }],
    }
}

fn global_boundary(
    system_indices: &[usize],
    staff_lines: &[SystemStaffBoundaries],
    first_line: bool,
    sheet_width: i32,
    vertical_translation: i32,
) -> StaffBoundary {
    let chosen = |index: usize| {
        if first_line {
            &staff_lines[index].first_line
        } else {
            &staff_lines[index].last_line
        }
    };
    let delta = f64::from(vertical_translation);
    let first_point = chosen(system_indices[0]).first_point();
    let mut current = (0.0, first_point.1 + delta);
    let mut segments = Vec::new();
    for &index in system_indices {
        let line = chosen(index);
        let start = line.first_point();
        let translated_start = (start.0, start.1 + delta);
        if current != translated_start {
            segments.push(BoundarySegment::Line {
                start: current,
                end: translated_start,
            });
        }
        segments.extend(
            line.segments
                .iter()
                .copied()
                .map(|segment| segment.translated_y(delta)),
        );
        current = (line.last_point().0, line.last_point().1 + delta);
    }
    let last_y = chosen(*system_indices.last().expect("nonempty neighbor row"))
        .last_point()
        .1
        + delta;
    let right = (f64::from(sheet_width), last_y);
    if current != right {
        segments.push(BoundarySegment::Line {
            start: current,
            end: right,
        });
    }
    StaffBoundary { segments }
}

/// Stable identity replacing Java's shared `SystemRef` object reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PopulationSystemRefId(u64);

impl PopulationSystemRefId {
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Stable identity replacing Java's shared `PageRef` object reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PopulationPageRefId(u64);

impl PopulationPageRefId {
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Exact value returned by `Staff.getStaffConfig()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PopulationStaffConfig {
    pub line_count: usize,
    pub is_small: bool,
}

/// Physical staff input to `PartRef.computeStaffConfigs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PopulationReferenceStaff {
    pub staff_id: usize,
    pub config: PopulationStaffConfig,
}

/// Physical part input in `SystemInfo.getParts()` order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulationReferencePart {
    pub part_id: usize,
    pub staves: Vec<PopulationReferenceStaff>,
}

/// Fresh `PartRef` produced by its two-argument Java constructor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulationPartRef {
    pub system_ref: PopulationSystemRefId,
    pub staff_configs: Vec<PopulationStaffConfig>,
    pub name: Option<String>,
    pub logical_id: Option<i32>,
    pub manual: bool,
}

/// Fresh `SystemRef` produced by `SystemInfo.buildRef`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulationSystemRef {
    pub page_ref_id: PopulationPageRefId,
    pub parts: Vec<PopulationPartRef>,
}

/// Physical system's transient `systemRef` field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PopulationSystemRefState {
    pub system_ref: Option<PopulationSystemRefId>,
}

/// Headless ownership registry for shared Java soft-reference objects.
#[derive(Clone, Debug, Default)]
pub struct PopulationReferenceRegistry {
    next_page_ref_id: u64,
    next_system_ref_id: u64,
    system_refs: BTreeMap<PopulationSystemRefId, PopulationSystemRef>,
}

impl PopulationReferenceRegistry {
    #[must_use]
    pub fn get(&self, id: PopulationSystemRefId) -> Option<&PopulationSystemRef> {
        self.system_refs.get(&id)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.system_refs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.system_refs.is_empty()
    }
}

/// Soft page ownership. Java stores the actual `SystemRef` objects; stable IDs
/// retain the same shared identity without self-referential Rust values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulationReferencePage {
    pub object_id: PopulationPageRefId,
    pub id: usize,
    pub movement_start: bool,
    pub systems: Vec<PopulationSystemRefId>,
}

/// Concrete Java `SystemInfo.buildRef` port.
///
/// A new object replaces the system's prior reference. Its page backlink is set
/// before parts are visited. Parts and their staff configurations preserve
/// physical list order; constructor defaults remain absent/false.
pub fn build_population_system_ref(
    state: &mut PopulationSystemRefState,
    registry: &mut PopulationReferenceRegistry,
    page_ref_id: PopulationPageRefId,
    parts: &[PopulationReferencePart],
) -> PopulationSystemRefId {
    registry.next_system_ref_id += 1;
    let id = PopulationSystemRefId(registry.next_system_ref_id);
    registry.system_refs.insert(
        id,
        PopulationSystemRef {
            page_ref_id,
            parts: Vec::new(),
        },
    );
    state.system_ref = Some(id);

    for part in parts {
        let staff_configs = part.staves.iter().map(|staff| staff.config).collect();
        registry
            .system_refs
            .get_mut(&id)
            .expect("fresh system reference remains registered")
            .parts
            .push(PopulationPartRef {
                system_ref: id,
                staff_configs,
                name: None,
                logical_id: None,
                manual: false,
            });
    }
    id
}

/// Reproduce the separate `pageRef.addSystem(system.buildRef())` ownership step.
pub fn append_population_system_ref(
    page: &mut PopulationReferencePage,
    registry: &PopulationReferenceRegistry,
    system_ref: PopulationSystemRefId,
) -> Result<(), PopulationReferenceError> {
    let reference = registry
        .get(system_ref)
        .ok_or(PopulationReferenceError::UnknownSystemRef(system_ref))?;
    if reference.page_ref_id != page.object_id {
        return Err(PopulationReferenceError::WrongPage {
            system_ref,
            expected: reference.page_ref_id,
            actual: page.object_id,
        });
    }
    page.systems.push(system_ref);
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopulationReferenceError {
    UnknownSystemRef(PopulationSystemRefId),
    WrongPage {
        system_ref: PopulationSystemRefId,
        expected: PopulationPageRefId,
        actual: PopulationPageRefId,
    },
}

/// Fallible adapter preserving the precise partial-mutation boundary of Java
/// `SystemInfo.buildRef` when an unchecked collaborator fails.
pub trait SystemReferenceExecutor {
    type Error;

    fn install_empty_system_ref(&mut self) -> Result<(), Self::Error>;
    fn bind_system_ref_to_page(&mut self) -> Result<(), Self::Error>;
    fn part_ids(&self) -> Vec<usize>;
    fn staff_ids(&self, part_id: usize) -> Vec<usize>;
    fn staff_config(&mut self, staff_id: usize) -> Result<PopulationStaffConfig, Self::Error>;
    fn append_part_ref(
        &mut self,
        part_id: usize,
        configs: Vec<PopulationStaffConfig>,
    ) -> Result<(), Self::Error>;
}

pub fn build_population_system_ref_with<Executor>(
    executor: &mut Executor,
) -> Result<(), Executor::Error>
where
    Executor: SystemReferenceExecutor,
{
    executor.install_empty_system_ref()?;
    executor.bind_system_ref_to_page()?;
    for part_id in executor.part_ids() {
        let mut configs = Vec::new();
        for staff_id in executor.staff_ids(part_id) {
            configs.push(executor.staff_config(staff_id)?);
        }
        executor.append_part_ref(part_id, configs)?;
    }
    Ok(())
}

/// System state consumed and mutated by Java `allocatePages`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulationSystem {
    pub id: SystemId,
    pub indented: bool,
    pub parts: Vec<PopulationReferencePart>,
    pub system_ref: PopulationSystemRefState,
    pub page_id: Option<usize>,
}

/// Physical page allocated on the sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulationPage {
    pub id: usize,
    pub first_system_id: Option<SystemId>,
    pub last_system_id: Option<SystemId>,
    pub system_ids: Vec<SystemId>,
}

/// Soft page reference allocated on the sheet stub.
/// Allocate pages and page references exactly as Java `allocatePages` does.
///
/// Existing pages and references are retained. This matters because the Java
/// helper itself does not clear them; callers such as `rebuildPages` do so.
/// System references are appended in sheet order, and an indented system closes
/// the preceding page before starting a movement page.
pub fn allocate_population_pages(
    systems: &mut [PopulationSystem],
    pages: &mut Vec<PopulationPage>,
    page_refs: &mut Vec<PopulationReferencePage>,
    registry: &mut PopulationReferenceRegistry,
) {
    let result = allocate_population_pages_with(systems, pages, page_refs, registry, |staff| {
        Ok::<_, std::convert::Infallible>(staff.config)
    });
    match result {
        Ok(()) => {}
        Err(never) => match never {},
    }
}

/// Fallible form of [`allocate_population_pages`] used to preserve Java's
/// unchecked-failure boundary around `Staff.getStaffConfig()`.
///
/// A failure leaves a newly allocated physical page and `PageRef`, the
/// system-to-page link, and the fresh replacement `SystemRef` (including any
/// completed part prefix) in place. The failing system reference is not added
/// to `PageRef.systems`, and the current page's cached system slice is not
/// finalized.
pub fn allocate_population_pages_with<Error>(
    systems: &mut [PopulationSystem],
    pages: &mut Vec<PopulationPage>,
    page_refs: &mut Vec<PopulationReferencePage>,
    registry: &mut PopulationReferenceRegistry,
    mut staff_config: impl FnMut(&PopulationReferenceStaff) -> Result<PopulationStaffConfig, Error>,
) -> Result<(), Error> {
    let mut active_page_index: Option<usize> = None;
    let mut active_ref_index: Option<usize> = None;

    for index in 0..systems.len() {
        let system_id = systems[index].id;
        let page_id = 1 + pages.len();

        if systems[index].indented {
            if let Some(page_index) = active_page_index {
                pages[page_index].last_system_id = Some(system_id - 1);
                set_page_systems_from(&mut pages[page_index], systems);
            }

            registry.next_page_ref_id += 1;
            page_refs.push(PopulationReferencePage {
                object_id: PopulationPageRefId(registry.next_page_ref_id),
                id: page_id,
                movement_start: true,
                systems: Vec::new(),
            });
            pages.push(PopulationPage {
                id: page_id,
                first_system_id: (system_id != 1).then_some(system_id),
                last_system_id: None,
                system_ids: Vec::new(),
            });
            active_page_index = Some(pages.len() - 1);
            active_ref_index = Some(page_refs.len() - 1);
        } else if active_page_index.is_none() {
            registry.next_page_ref_id += 1;
            page_refs.push(PopulationReferencePage {
                object_id: PopulationPageRefId(registry.next_page_ref_id),
                id: page_id,
                movement_start: false,
                systems: Vec::new(),
            });
            pages.push(PopulationPage {
                id: page_id,
                first_system_id: None,
                last_system_id: None,
                system_ids: Vec::new(),
            });
            active_page_index = Some(pages.len() - 1);
            active_ref_index = Some(page_refs.len() - 1);
        }

        let page_index = active_page_index.expect("the first system always allocates a page");
        let page_ref_index =
            active_ref_index.expect("physical and soft pages are allocated together");
        systems[index].page_id = Some(pages[page_index].id);
        let page_ref_id = page_refs[page_ref_index].object_id;

        registry.next_system_ref_id += 1;
        let system_ref_id = PopulationSystemRefId(registry.next_system_ref_id);
        registry.system_refs.insert(
            system_ref_id,
            PopulationSystemRef {
                page_ref_id,
                parts: Vec::new(),
            },
        );
        systems[index].system_ref.system_ref = Some(system_ref_id);

        for part in &systems[index].parts {
            let mut configs = Vec::with_capacity(part.staves.len());
            for staff in &part.staves {
                configs.push(staff_config(staff)?);
            }
            registry
                .system_refs
                .get_mut(&system_ref_id)
                .expect("fresh system reference remains registered")
                .parts
                .push(PopulationPartRef {
                    system_ref: system_ref_id,
                    staff_configs: configs,
                    name: None,
                    logical_id: None,
                    manual: false,
                });
        }
        page_refs[page_ref_index].systems.push(system_ref_id);
    }

    if let Some(page_index) = active_page_index {
        set_page_systems_from(&mut pages[page_index], systems);
    }
    Ok(())
}

fn set_page_systems_from(page: &mut PopulationPage, systems: &[PopulationSystem]) {
    let first = page.first_system_id.unwrap_or(1) - 1;
    let last = page.last_system_id.unwrap_or(systems.len()) - 1;
    page.system_ids = systems[first..=last]
        .iter()
        .map(|system| system.id)
        .collect();
}

/// Values computed by Java `reportResults` for one page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulationPageReport {
    pub page_id: usize,
    pub part_count: usize,
    pub system_count: usize,
    pub tablature_count: usize,
}

/// One system's counts used by the reporting-only tail of population.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PopulationSystemCounts {
    pub system_id: SystemId,
    pub part_count: usize,
    pub tablature_count: usize,
}

/// Compute the maxima reported for a page by Java `reportResults`.
#[must_use]
pub fn population_page_report(
    page: &PopulationPage,
    systems: &[PopulationSystemCounts],
) -> PopulationPageReport {
    let mut part_count = 0;
    let mut tablature_count = 0;
    for system_id in &page.system_ids {
        if let Some(system) = systems.iter().find(|system| system.system_id == *system_id) {
            part_count = part_count.max(system.part_count);
            tablature_count = tablature_count.max(system.tablature_count);
        }
    }
    PopulationPageReport {
        page_id: page.id,
        part_count,
        system_count: page.system_ids.len(),
        tablature_count,
    }
}

#[cfg(test)]
mod staff_area_tests {
    use super::*;

    fn boundary(y: f64, width: i32) -> StaffBoundary {
        StaffBoundary {
            segments: vec![BoundarySegment::Line {
                start: (0.0, y),
                end: (f64::from(width), y),
            }],
        }
    }

    fn staff(id: usize, system: usize, top: i32, bottom: i32) -> PopulationStaffGeometry {
        PopulationStaffGeometry {
            staff_id: id,
            system_id: system,
            left: 0,
            width: 1000,
            top,
            bottom,
        }
    }

    fn lines(top: f64, bottom: f64) -> SystemStaffBoundaries {
        SystemStaffBoundaries {
            first_line: boundary(top, 1000),
            last_line: boundary(bottom, 1000),
        }
    }

    #[test]
    fn a_lone_staff_spans_the_sheet() {
        let areas = build_population_staff_areas(
            &[staff(1, 1, 100, 200)],
            &[lines(100.0, 200.0)],
            &|_| (0, 0),
            1000,
            800,
        );
        assert_eq!(areas.len(), 1);
        let area = &areas[0].area;
        assert!(
            area.contains(500.0, 50.0),
            "above the staff is still its own"
        );
        assert!(area.contains(500.0, 700.0), "and so is below it");
    }

    #[test]
    fn neighbours_split_the_sheet_at_their_own_lines() {
        // Two staves, one under the other. The boundary between them is the
        // upper staff's last line and the lower staff's first line, with no
        // margin -- that is the difference from a system area.
        let areas = build_population_staff_areas(
            &[staff(1, 1, 100, 200), staff(2, 1, 400, 500)],
            &[lines(100.0, 200.0), lines(400.0, 500.0)],
            &|_| (0, 0),
            1000,
            800,
        );
        let (upper, lower) = (&areas[0].area, &areas[1].area);
        assert!(upper.contains(500.0, 300.0), "just under its last line");
        assert!(!upper.contains(500.0, 450.0), "not past the staff below");
        assert!(lower.contains(500.0, 450.0));
        assert!(!lower.contains(500.0, 150.0), "not up into the staff above");
    }

    #[test]
    fn unknown_system_ends_leave_the_staff_spanning_the_sheet() {
        // Java: `if ((left != 0) || ((right != 0) && (right != sheetWidth)))`.
        // Both unknown means no intersection at all.
        let areas = build_population_staff_areas(
            &[staff(1, 1, 100, 200)],
            &[lines(100.0, 200.0)],
            &|_| (0, 0),
            1000,
            800,
        );
        assert!(areas[0].area.contains(5.0, 150.0));
        assert!(areas[0].area.contains(995.0, 150.0));
    }

    #[test]
    fn known_system_ends_clip_the_staff() {
        let areas = build_population_staff_areas(
            &[staff(1, 1, 100, 200)],
            &[lines(100.0, 200.0)],
            &|_| (200, 800),
            1000,
            800,
        );
        assert!(!areas[0].area.contains(100.0, 150.0), "left of the system");
        assert!(areas[0].area.contains(500.0, 150.0));
        assert!(!areas[0].area.contains(900.0, 150.0), "right of the system");
    }

    #[test]
    fn a_left_end_without_a_right_end_yields_an_empty_area() {
        // Java intersects with `Rectangle(left, 0, right - left, height)`, and a
        // right of zero makes that width negative, so the area is empty. Left
        // to fall out of `contains` rather than special-cased, because that is
        // what Java does.
        let areas = build_population_staff_areas(
            &[staff(1, 1, 100, 200)],
            &[lines(100.0, 200.0)],
            &|_| (200, 0),
            1000,
            800,
        );
        assert!(!areas[0].area.contains(500.0, 150.0));
        assert!(!areas[0].area.contains(250.0, 150.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_system(id: SystemId, indented: bool) -> PopulationSystem {
        PopulationSystem {
            id,
            indented,
            parts: Vec::new(),
            system_ref: PopulationSystemRefState::default(),
            page_id: None,
        }
    }

    fn straight_boundary(x1: f64, y1: f64, x2: f64, y2: f64) -> StaffBoundary {
        StaffBoundary {
            segments: vec![BoundarySegment::Line {
                start: (x1, y1),
                end: (x2, y2),
            }],
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Call {
        Update(SystemId),
        SystemArea(SystemId),
        StaffArea(usize),
        Horizontal,
        Vertical,
        Indentations,
        Pages,
        Report,
    }

    #[derive(Default)]
    struct RecordingPopulation {
        calls: Vec<Call>,
        fail_at: Option<Call>,
    }

    impl RecordingPopulation {
        fn record(&mut self, call: Call) -> Result<(), &'static str> {
            self.calls.push(call.clone());
            if self.fail_at == Some(call) {
                Err("population failure")
            } else {
                Ok(())
            }
        }
    }

    impl SystemPopulationExecutor for RecordingPopulation {
        type Error = &'static str;

        fn system_ids(&self) -> Vec<SystemId> {
            vec![1, 2]
        }

        fn staff_ids(&self, system_id: SystemId) -> Vec<usize> {
            match system_id {
                1 => vec![10, 11],
                2 => vec![20],
                _ => Vec::new(),
            }
        }

        fn update_system_coordinates(&mut self, id: SystemId) -> Result<(), Self::Error> {
            self.record(Call::Update(id))
        }

        fn compute_system_area(&mut self, id: SystemId) -> Result<(), Self::Error> {
            self.record(Call::SystemArea(id))
        }

        fn compute_staff_area(&mut self, id: usize) -> Result<(), Self::Error> {
            self.record(Call::StaffArea(id))
        }

        fn dispatch_horizontal_sections(&mut self) -> Result<(), Self::Error> {
            self.record(Call::Horizontal)
        }

        fn dispatch_vertical_sections(&mut self) -> Result<(), Self::Error> {
            self.record(Call::Vertical)
        }

        fn check_indentations(&mut self) -> Result<(), Self::Error> {
            self.record(Call::Indentations)
        }

        fn allocate_pages(&mut self) -> Result<(), Self::Error> {
            self.record(Call::Pages)
        }

        fn report_results(&mut self) -> Result<(), Self::Error> {
            self.record(Call::Report)
        }
    }

    #[test]
    fn lifecycle_matches_java_order() {
        let mut population = RecordingPopulation::default();
        assert_eq!(populate_systems(&mut population), Ok(()));
        assert_eq!(
            population.calls,
            [
                Call::Update(1),
                Call::SystemArea(1),
                Call::Update(2),
                Call::SystemArea(2),
                Call::StaffArea(10),
                Call::StaffArea(11),
                Call::StaffArea(20),
                Call::Horizontal,
                Call::Vertical,
                Call::Indentations,
                Call::Pages,
                Call::Report,
            ]
        );
    }

    #[test]
    fn failure_retains_prior_mutation_and_stops_immediately() {
        let mut population = RecordingPopulation {
            fail_at: Some(Call::Vertical),
            ..RecordingPopulation::default()
        };
        assert_eq!(populate_systems(&mut population), Err("population failure"));
        assert_eq!(population.calls.last(), Some(&Call::Vertical));
        assert!(!population.calls.contains(&Call::Indentations));
        assert!(!population.calls.contains(&Call::Pages));
    }

    #[test]
    fn section_dispatch_clears_then_links_all_containing_systems_in_order() {
        let mut systems = vec![
            SystemSectionOwnership {
                system_id: 1,
                horizontal_sections: vec![99],
                vertical_sections: vec![88],
            },
            SystemSectionOwnership {
                system_id: 2,
                horizontal_sections: vec![98],
                vertical_sections: vec![87],
            },
        ];
        let sections = [
            PopulationSection {
                id: 4,
                centroid_x: 5.0,
                centroid_y: 10.0,
            },
            PopulationSection {
                id: 7,
                centroid_x: 5.0,
                centroid_y: 20.0,
            },
            PopulationSection {
                id: 9,
                centroid_x: 5.0,
                centroid_y: 99.0,
            },
        ];
        dispatch_sections(
            PopulationLag::Horizontal,
            &mut systems,
            &sections,
            |id, _, y| (id == 1 && y <= 20.0) || (id == 2 && (10.0..=20.0).contains(&y)),
        );
        assert_eq!(systems[0].horizontal_sections, [4, 7]);
        assert_eq!(systems[1].horizontal_sections, [4, 7]);
        assert_eq!(systems[0].vertical_sections, [88]);
        assert_eq!(systems[1].vertical_sections, [87]);

        dispatch_sections(
            PopulationLag::Vertical,
            &mut systems,
            &sections[..1],
            |id, _, _| id == 2,
        );
        assert_eq!(systems[0].vertical_sections, []);
        assert_eq!(systems[1].vertical_sections, [4]);
        assert_eq!(systems[0].horizontal_sections, [4, 7]);
    }

    #[test]
    fn page_allocation_splits_movements_and_links_both_ownership_trees() {
        let mut systems = vec![
            reference_system(1, false),
            reference_system(2, false),
            reference_system(3, true),
            reference_system(4, false),
            reference_system(5, true),
        ];
        let mut pages = Vec::new();
        let mut refs = Vec::new();
        let mut registry = PopulationReferenceRegistry::default();
        allocate_population_pages(&mut systems, &mut pages, &mut refs, &mut registry);

        assert_eq!(
            systems
                .iter()
                .map(|system| system.page_id)
                .collect::<Vec<_>>(),
            [Some(1), Some(1), Some(2), Some(2), Some(3)]
        );
        assert_eq!(
            pages
                .iter()
                .map(|page| page.system_ids.clone())
                .collect::<Vec<_>>(),
            [vec![1, 2], vec![3, 4], vec![5]]
        );
        assert_eq!(
            pages
                .iter()
                .map(|page| page.first_system_id)
                .collect::<Vec<_>>(),
            [None, Some(3), Some(5)]
        );
        assert_eq!(
            pages
                .iter()
                .map(|page| page.last_system_id)
                .collect::<Vec<_>>(),
            [Some(2), Some(4), None]
        );
        assert_eq!(
            refs.iter()
                .map(|page| page.movement_start)
                .collect::<Vec<_>>(),
            [false, true, true]
        );
        assert_eq!(
            refs.iter()
                .map(|page| { page.systems.iter().map(|id| id.value()).collect::<Vec<_>>() })
                .collect::<Vec<_>>(),
            [vec![1, 2], vec![3, 4], vec![5]]
        );
        assert_eq!(
            systems
                .iter()
                .map(|system| system.system_ref.system_ref.expect("built").value())
                .collect::<Vec<_>>(),
            [1, 2, 3, 4, 5]
        );
        for page_ref in &refs {
            for system_ref in &page_ref.systems {
                assert_eq!(
                    registry.get(*system_ref).expect("registered").page_ref_id,
                    page_ref.object_id
                );
            }
        }
    }

    #[test]
    fn page_allocation_builds_fresh_refs_after_page_link_and_preserves_parts() {
        let mut registry = PopulationReferenceRegistry::default();
        let mut old_state = PopulationSystemRefState::default();
        let old = build_population_system_ref(
            &mut old_state,
            &mut registry,
            PopulationPageRefId(99),
            &[],
        );
        let parts = vec![
            PopulationReferencePart {
                part_id: 8,
                staves: vec![
                    PopulationReferenceStaff {
                        staff_id: 12,
                        config: PopulationStaffConfig {
                            line_count: 1,
                            is_small: true,
                        },
                    },
                    PopulationReferenceStaff {
                        staff_id: 11,
                        config: PopulationStaffConfig {
                            line_count: 5,
                            is_small: false,
                        },
                    },
                ],
            },
            PopulationReferencePart {
                part_id: 3,
                staves: Vec::new(),
            },
        ];
        let mut systems = vec![PopulationSystem {
            id: 1,
            indented: false,
            parts,
            system_ref: old_state,
            page_id: None,
        }];
        let mut pages = Vec::new();
        let mut page_refs = Vec::new();

        allocate_population_pages(&mut systems, &mut pages, &mut page_refs, &mut registry);

        let fresh = systems[0].system_ref.system_ref.expect("replacement ref");
        assert_ne!(fresh, old);
        assert_eq!(page_refs[0].systems, [fresh]);
        let reference = registry.get(fresh).expect("fresh ref registered");
        assert_eq!(reference.page_ref_id, page_refs[0].object_id);
        assert_eq!(
            reference
                .parts
                .iter()
                .map(|part| part.staff_configs.clone())
                .collect::<Vec<_>>(),
            [
                vec![
                    PopulationStaffConfig {
                        line_count: 1,
                        is_small: true,
                    },
                    PopulationStaffConfig {
                        line_count: 5,
                        is_small: false,
                    },
                ],
                vec![],
            ]
        );
        assert!(reference.parts.iter().all(|part| part.system_ref == fresh));
        assert!(registry.get(old).is_some());
    }

    #[test]
    fn page_allocation_failure_retains_exact_built_prefix_and_unfinalized_page() {
        let mut first = reference_system(1, false);
        first.parts.push(PopulationReferencePart {
            part_id: 1,
            staves: vec![PopulationReferenceStaff {
                staff_id: 10,
                config: PopulationStaffConfig {
                    line_count: 5,
                    is_small: false,
                },
            }],
        });
        let mut failing = reference_system(2, true);
        failing.parts = vec![
            PopulationReferencePart {
                part_id: 2,
                staves: vec![PopulationReferenceStaff {
                    staff_id: 20,
                    config: PopulationStaffConfig {
                        line_count: 5,
                        is_small: false,
                    },
                }],
            },
            PopulationReferencePart {
                part_id: 3,
                staves: vec![PopulationReferenceStaff {
                    staff_id: 21,
                    config: PopulationStaffConfig {
                        line_count: 5,
                        is_small: false,
                    },
                }],
            },
        ];
        let mut systems = vec![first, failing];
        let mut pages = Vec::new();
        let mut page_refs = Vec::new();
        let mut registry = PopulationReferenceRegistry::default();
        let mut visited = Vec::new();

        let result = allocate_population_pages_with(
            &mut systems,
            &mut pages,
            &mut page_refs,
            &mut registry,
            |staff| {
                visited.push(staff.staff_id);
                if staff.staff_id == 21 {
                    Err("staff config failure")
                } else {
                    Ok(staff.config)
                }
            },
        );

        assert_eq!(result, Err("staff config failure"));
        assert_eq!(visited, [10, 20, 21]);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].last_system_id, Some(1));
        assert_eq!(pages[0].system_ids, [1]);
        assert!(pages[1].system_ids.is_empty());
        assert_eq!(systems[1].page_id, Some(2));
        assert_eq!(page_refs[0].systems.len(), 1);
        assert!(page_refs[1].systems.is_empty());
        assert!(page_refs[1].movement_start);

        let failed_ref = systems[1].system_ref.system_ref.expect("fresh failed ref");
        let failed = registry.get(failed_ref).expect("failed ref retained");
        assert_eq!(failed.page_ref_id, page_refs[1].object_id);
        assert_eq!(failed.parts.len(), 1);
        assert_eq!(
            failed.parts[0].staff_configs,
            [PopulationStaffConfig {
                line_count: 5,
                is_small: false,
            }]
        );
    }

    #[test]
    fn indentation_uses_strict_overlap_and_inclusive_shift_threshold() {
        let systems = [
            PopulationSystemGeometry {
                system_id: 1,
                left: 10,
                width: 100,
                top: 0,
                bottom: 40,
                area_left: 0,
                deskewed_upper_left_x: 10.0,
            },
            PopulationSystemGeometry {
                system_id: 2,
                left: 30,
                width: 80,
                top: 50,
                bottom: 90,
                area_left: 0,
                deskewed_upper_left_x: 30.0,
            },
        ];
        assert!(check_population_indentation(&systems, 1, 20.0));
        assert!(!check_population_indentation(&systems, 1, 20.01));
    }

    #[test]
    fn side_by_side_system_with_nonzero_left_area_end_cannot_be_indented() {
        let systems = [
            PopulationSystemGeometry {
                system_id: 1,
                left: 0,
                width: 60,
                top: 0,
                bottom: 40,
                area_left: 0,
                deskewed_upper_left_x: 0.0,
            },
            PopulationSystemGeometry {
                system_id: 2,
                left: 80,
                width: 60,
                top: 0,
                bottom: 40,
                area_left: 70,
                deskewed_upper_left_x: 80.0,
            },
        ];
        assert!(!check_population_indentation(&systems, 1, 1.0));
    }

    #[test]
    fn system_areas_concatenate_neighbor_row_by_id_and_apply_vertical_margin() {
        let systems = [
            PopulationSystemGeometry {
                system_id: 1,
                left: 0,
                width: 90,
                top: 0,
                bottom: 30,
                area_left: 0,
                deskewed_upper_left_x: 0.0,
            },
            PopulationSystemGeometry {
                system_id: 2,
                left: 110,
                width: 90,
                top: 0,
                bottom: 30,
                area_left: 100,
                deskewed_upper_left_x: 110.0,
            },
            PopulationSystemGeometry {
                system_id: 3,
                left: 20,
                width: 160,
                top: 100,
                bottom: 130,
                area_left: 0,
                deskewed_upper_left_x: 20.0,
            },
        ];
        let lines = [
            SystemStaffBoundaries {
                first_line: straight_boundary(10.0, 10.0, 80.0, 10.0),
                last_line: straight_boundary(10.0, 20.0, 80.0, 20.0),
            },
            SystemStaffBoundaries {
                first_line: straight_boundary(120.0, 12.0, 190.0, 12.0),
                last_line: straight_boundary(120.0, 30.0, 190.0, 30.0),
            },
            SystemStaffBoundaries {
                first_line: straight_boundary(20.0, 105.0, 180.0, 105.0),
                last_line: straight_boundary(20.0, 125.0, 180.0, 125.0),
            },
        ];
        let areas = build_population_system_areas(&systems, &lines, 200, 180, 5);
        let lower = &areas[2];
        assert_eq!((lower.left, lower.right), (0, 200));
        // The initially found upper neighbor is #2, but Java expands the row
        // and sorts it by ID before concatenating #1 then #2.
        assert_eq!(boundary_y_at_x(lower.north(), 50.0), Some(25.0));
        assert_eq!(boundary_y_at_x(lower.north(), 150.0), Some(35.0));
        assert!(lower.contains(50.0, 25.1));
        // On the north boundary is inside, per Java's half-open insideness.
        assert!(lower.contains(50.0, 25.0));
    }

    #[test]
    fn side_by_side_slice_uses_java_get_right_midpoint_and_excludes_divider() {
        let systems = [
            PopulationSystemGeometry {
                system_id: 1,
                left: 0,
                width: 90,
                top: 0,
                bottom: 40,
                area_left: 0,
                deskewed_upper_left_x: 0.0,
            },
            PopulationSystemGeometry {
                system_id: 2,
                left: 111,
                width: 89,
                top: 0,
                bottom: 40,
                area_left: 100,
                deskewed_upper_left_x: 111.0,
            },
        ];
        let lines = [
            SystemStaffBoundaries {
                first_line: straight_boundary(0.0, 10.0, 90.0, 10.0),
                last_line: straight_boundary(0.0, 30.0, 90.0, 30.0),
            },
            SystemStaffBoundaries {
                first_line: straight_boundary(111.0, 10.0, 200.0, 10.0),
                last_line: straight_boundary(111.0, 30.0, 200.0, 30.0),
            },
        ];
        let areas = build_population_system_areas(&systems, &lines, 200, 100, 0);
        // Java getRight() is left + width, so (90 + 111) / 2 truncates to 100.
        assert_eq!((areas[0].left, areas[0].right), (0, 100));
        assert_eq!((areas[1].left, areas[1].right), (100, 200));
        // The divider belongs to the right-hand system, not to neither: a
        // point on a left edge is inside and on a right edge is outside, so
        // exactly one of the two claims it. This previously asserted that
        // neither did, which would leave a one-pixel column assigned to
        // nothing.
        assert!(areas[0].contains(99.999, 50.0));
        assert!(!areas[0].contains(100.0, 50.0));
        assert!(areas[1].contains(100.0, 50.0));
        assert!(areas[1].contains(100.001, 50.0));
    }

    #[test]
    fn area_containment_evaluates_actual_cubic_x_controls() {
        let curved = StaffBoundary {
            segments: vec![BoundarySegment::Cubic {
                start: (0.0, 10.0),
                control1: (0.0, 40.0),
                control2: (20.0, 40.0),
                end: (100.0, 10.0),
            }],
        };
        let north = boundary_y_at_x(&curved, 50.0).expect("x is on cubic");
        // A linear endpoint parameter would yield 32.5 at x=50. The Area
        // path uses cubic x controls and therefore reaches a later parameter.
        assert!((north - 32.5).abs() > 0.5);
        let area = PopulationSystemArea {
            system_id: 1,
            left: 0,
            right: 100,
            north: curved,
            south: straight_boundary(0.0, 80.0, 100.0, 80.0),
        };
        // The north edge belongs to the area and the south edge does not.
        // This previously asserted the opposite for the north; checked against
        // Java, `new Area(new Rectangle2D.Double(0, 0, 100, 100))` answers
        // contains(50, 0) true and contains(50, 100) false.
        assert!(area.contains(50.0, north));
        assert!(area.contains(50.0, north + 0.001));
        assert!(!area.contains(50.0, 80.0));
    }

    #[test]
    fn outline_reverses_south_segments_and_cubic_controls_like_java_iterator() {
        let area = PopulationSystemArea {
            system_id: 1,
            left: 0,
            right: 100,
            north: straight_boundary(0.0, 10.0, 100.0, 10.0),
            south: StaffBoundary {
                segments: vec![BoundarySegment::Cubic {
                    start: (0.0, 70.0),
                    control1: (20.0, 75.0),
                    control2: (80.0, 65.0),
                    end: (100.0, 70.0),
                }],
            },
        };
        assert_eq!(
            area.outline_segments(),
            [
                BoundarySegment::Line {
                    start: (0.0, 10.0),
                    end: (100.0, 10.0),
                },
                BoundarySegment::Line {
                    start: (100.0, 10.0),
                    end: (100.0, 70.0),
                },
                BoundarySegment::Cubic {
                    start: (100.0, 70.0),
                    control1: (80.0, 65.0),
                    control2: (20.0, 75.0),
                    end: (0.0, 70.0),
                },
                BoundarySegment::Line {
                    start: (0.0, 70.0),
                    end: (0.0, 10.0),
                },
            ]
        );
    }

    #[test]
    fn build_ref_replaces_system_field_and_preserves_part_staff_order_and_defaults() {
        let mut state = PopulationSystemRefState::default();
        let mut registry = PopulationReferenceRegistry::default();
        let parts = [
            PopulationReferencePart {
                part_id: 20,
                staves: vec![
                    PopulationReferenceStaff {
                        staff_id: 4,
                        config: PopulationStaffConfig {
                            line_count: 5,
                            is_small: false,
                        },
                    },
                    PopulationReferenceStaff {
                        staff_id: 5,
                        config: PopulationStaffConfig {
                            line_count: 1,
                            is_small: true,
                        },
                    },
                ],
            },
            PopulationReferencePart {
                part_id: 10,
                staves: vec![PopulationReferenceStaff {
                    staff_id: 9,
                    config: PopulationStaffConfig {
                        line_count: 6,
                        is_small: false,
                    },
                }],
            },
        ];
        let page_ref_id = PopulationPageRefId(7);
        let id = build_population_system_ref(&mut state, &mut registry, page_ref_id, &parts);
        assert_eq!(state.system_ref, Some(id));
        let reference = registry.get(id).expect("fresh reference");
        assert_eq!(reference.page_ref_id, page_ref_id);
        assert_eq!(
            reference
                .parts
                .iter()
                .map(|part| part.staff_configs.clone())
                .collect::<Vec<_>>(),
            [
                vec![
                    PopulationStaffConfig {
                        line_count: 5,
                        is_small: false,
                    },
                    PopulationStaffConfig {
                        line_count: 1,
                        is_small: true,
                    },
                ],
                vec![PopulationStaffConfig {
                    line_count: 6,
                    is_small: false,
                }],
            ]
        );
        assert!(reference.parts.iter().all(|part| {
            part.system_ref == id
                && part.name.is_none()
                && part.logical_id.is_none()
                && !part.manual
        }));
    }

    #[test]
    fn page_link_is_separate_and_rebuild_does_not_retarget_old_page_object() {
        let mut state = PopulationSystemRefState::default();
        let mut registry = PopulationReferenceRegistry::default();
        let page_ref_id = PopulationPageRefId(3);
        let mut page = PopulationReferencePage {
            object_id: page_ref_id,
            id: 3,
            movement_start: false,
            systems: Vec::new(),
        };
        let old = build_population_system_ref(&mut state, &mut registry, page_ref_id, &[]);
        append_population_system_ref(&mut page, &registry, old).unwrap();
        let new = build_population_system_ref(&mut state, &mut registry, page_ref_id, &[]);

        assert_ne!(old, new);
        assert_eq!(state.system_ref, Some(new));
        assert_eq!(page.systems, [old]);
        assert!(registry.get(old).is_some());
        assert!(registry.get(new).is_some());
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn wrong_page_link_fails_without_mutating_page_order() {
        let mut state = PopulationSystemRefState::default();
        let mut registry = PopulationReferenceRegistry::default();
        let source_page = PopulationPageRefId(2);
        let other_page = PopulationPageRefId(4);
        let id = build_population_system_ref(&mut state, &mut registry, source_page, &[]);
        let mut page = PopulationReferencePage {
            object_id: other_page,
            id: 4,
            movement_start: false,
            systems: vec![],
        };
        assert_eq!(
            append_population_system_ref(&mut page, &registry, id),
            Err(PopulationReferenceError::WrongPage {
                system_ref: id,
                expected: source_page,
                actual: other_page,
            })
        );
        assert!(page.systems.is_empty());
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum RefCall {
        Install,
        Bind,
        Config(usize),
        Append(usize, Vec<PopulationStaffConfig>),
    }

    #[derive(Default)]
    struct RecordingRefBuilder {
        calls: Vec<RefCall>,
        fail_staff: Option<usize>,
    }

    impl SystemReferenceExecutor for RecordingRefBuilder {
        type Error = &'static str;

        fn install_empty_system_ref(&mut self) -> Result<(), Self::Error> {
            self.calls.push(RefCall::Install);
            Ok(())
        }

        fn bind_system_ref_to_page(&mut self) -> Result<(), Self::Error> {
            self.calls.push(RefCall::Bind);
            Ok(())
        }

        fn part_ids(&self) -> Vec<usize> {
            vec![10, 20]
        }

        fn staff_ids(&self, part_id: usize) -> Vec<usize> {
            match part_id {
                10 => vec![1],
                20 => vec![2, 3],
                _ => Vec::new(),
            }
        }

        fn staff_config(&mut self, staff_id: usize) -> Result<PopulationStaffConfig, Self::Error> {
            self.calls.push(RefCall::Config(staff_id));
            if self.fail_staff == Some(staff_id) {
                Err("staff config failure")
            } else {
                Ok(PopulationStaffConfig {
                    line_count: staff_id,
                    is_small: false,
                })
            }
        }

        fn append_part_ref(
            &mut self,
            part_id: usize,
            configs: Vec<PopulationStaffConfig>,
        ) -> Result<(), Self::Error> {
            self.calls.push(RefCall::Append(part_id, configs));
            Ok(())
        }
    }

    #[test]
    fn build_ref_failure_keeps_new_empty_ref_and_completed_part_prefix() {
        let mut builder = RecordingRefBuilder {
            fail_staff: Some(3),
            ..RecordingRefBuilder::default()
        };
        assert_eq!(
            build_population_system_ref_with(&mut builder),
            Err("staff config failure")
        );
        assert_eq!(
            builder.calls,
            [
                RefCall::Install,
                RefCall::Bind,
                RefCall::Config(1),
                RefCall::Append(
                    10,
                    vec![PopulationStaffConfig {
                        line_count: 1,
                        is_small: false,
                    }],
                ),
                RefCall::Config(2),
                RefCall::Config(3),
            ]
        );
    }

    #[test]
    fn first_indented_system_is_a_movement_page_without_first_id_override() {
        let mut systems = vec![reference_system(1, true)];
        let mut pages = Vec::new();
        let mut refs = Vec::new();
        let mut registry = PopulationReferenceRegistry::default();
        allocate_population_pages(&mut systems, &mut pages, &mut refs, &mut registry);
        assert_eq!(pages[0].first_system_id, None);
        assert_eq!(pages[0].system_ids, [1]);
        assert!(refs[0].movement_start);
    }

    #[test]
    fn page_report_uses_max_parts_and_tablatures_across_systems() {
        let page = PopulationPage {
            id: 2,
            first_system_id: Some(3),
            last_system_id: Some(4),
            system_ids: vec![3, 4],
        };
        let report = population_page_report(
            &page,
            &[
                PopulationSystemCounts {
                    system_id: 3,
                    part_count: 2,
                    tablature_count: 1,
                },
                PopulationSystemCounts {
                    system_id: 4,
                    part_count: 4,
                    tablature_count: 3,
                },
            ],
        );
        assert_eq!(
            report,
            PopulationPageReport {
                page_id: 2,
                part_count: 4,
                system_count: 2,
                tablature_count: 3
            }
        );
    }
}
