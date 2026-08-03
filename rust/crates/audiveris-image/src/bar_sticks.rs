// SPDX-License-Identifier: AGPL-3.0-or-later

//! Headless construction of Java `PeakGraph.buildBarSticks` filaments.
//!
//! This stage consumes the two existing section lags and ordered raw peak
//! vertices. It preserves Java's VLAG-before-HLAG candidate order, per-peak
//! section attachment, successful filament registration order, removal of
//! peaks with no core filament, and the weak curvature-to-brace marking pass.
//! It deliberately creates no alignment or connection edges.

use std::{error::Error, fmt};

use crate::{
    bar_alignment::BarAlignment,
    bars_logic::{LocatedSectionId, SectionLag, sections_by_width},
    peak_graph::PeakGraph,
    run_table::Orientation,
    section::{Bounds, Section},
    staff_peak::{PeakPoint, StaffPeakAttribute, StaffPeakKey},
};

/// Source-resolved constants used by Java's bar-filament factory and weak
/// curvature pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarStickParameters {
    pub vertical_extension: i32,
    pub minimum_core_section_length: usize,
    pub probe_width: usize,
    pub minimum_probe_weight: usize,
    pub segment_length: usize,
    pub minimum_mean_curvature: f64,
    /// First ID assigned by the sheet filament index.
    pub first_filament_id: usize,
}

/// A successful registered bar filament, attached to one peak by key.
#[derive(Clone, Debug, PartialEq)]
pub struct BarStick {
    pub id: usize,
    pub peak: StaffPeakKey,
    /// Java `Section.byFullAbscissa` member order.
    pub members: Vec<LocatedSectionId>,
    pub bounds: Bounds,
    pub points: Vec<PeakPoint>,
    pub mean_curvature: f64,
    pub marked_brace: bool,
}

/// Caller-owned mutable state makes Java's mutation prefix observable if
/// filament-index registration fails partway through the peak traversal.
#[derive(Clone, Debug, PartialEq)]
pub struct BarStickBuildState {
    next_filament_id: usize,
    sticks: Vec<BarStick>,
    removed_peaks: Vec<StaffPeakKey>,
}

impl BarStickBuildState {
    pub fn new(first_filament_id: usize) -> Result<Self, BarStickError> {
        if first_filament_id == 0 {
            return Err(BarStickError::InvalidFirstFilamentId);
        }
        Ok(Self {
            next_filament_id: first_filament_id,
            sticks: Vec::new(),
            removed_peaks: Vec::new(),
        })
    }

    #[must_use]
    pub fn sticks(&self) -> &[BarStick] {
        &self.sticks
    }

    #[must_use]
    pub fn removed_peaks(&self) -> &[StaffPeakKey] {
        &self.removed_peaks
    }

    #[must_use]
    pub const fn next_filament_id(&self) -> usize {
        self.next_filament_id
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BarStickError {
    InvalidFirstFilamentId,
    InvalidParameters,
    NonEmptyState,
    MissingPeak(StaffPeakKey),
    MissingMember(LocatedSectionId),
    FilamentIdExhausted(StaffPeakKey),
}

impl fmt::Display for BarStickError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFirstFilamentId => {
                formatter.write_str("first bar-filament ID must be positive")
            }
            Self::InvalidParameters => formatter.write_str("invalid bar-stick parameters"),
            Self::NonEmptyState => formatter.write_str("bar-stick build state is not empty"),
            Self::MissingPeak(key) => write!(formatter, "bar peak is absent from graph: {key:?}"),
            Self::MissingMember(member) => {
                write!(formatter, "bar filament member is absent: {member:?}")
            }
            Self::FilamentIdExhausted(key) => {
                write!(formatter, "bar-filament ID exhausted at peak {key:?}")
            }
        }
    }
}

/// Build and register one split subfilament from exactly the old filament's
/// members. Registration precedes attachment to a graph/projector, preserving
/// Java's observable orphan when construction of the other half later fails.
pub fn build_sub_stick(
    peak: &crate::staff_peak::StaffPeak,
    old_members: &[LocatedSectionId],
    vertical_sections: &[Section],
    horizontal_sections: &[Section],
    parameters: BarStickParameters,
    state: &mut BarStickBuildState,
) -> Result<Option<BarStick>, BarStickError> {
    if parameters.vertical_extension < 0
        || parameters.minimum_core_section_length == 0
        || parameters.probe_width == 0
        || parameters.minimum_probe_weight == 0
        || parameters.segment_length == 0
        || !parameters.minimum_mean_curvature.is_finite()
        || parameters.minimum_mean_curvature < 0.0
        || parameters.first_filament_id != state.next_filament_id
    {
        return Err(BarStickError::InvalidParameters);
    }
    let mut source = Vec::with_capacity(old_members.len());
    for &location in old_members {
        let sections = match location.lag {
            SectionLag::Vertical => vertical_sections,
            SectionLag::Horizontal => horizontal_sections,
        };
        let section = sections
            .iter()
            .find(|section| section.id() == location.id)
            .ok_or(BarStickError::MissingMember(location))?;
        source.push(LocatedSection { location, section });
    }
    let Some(candidate) = construct_stick(peak, &source, parameters) else {
        return Ok(None);
    };
    let id = state.next_filament_id;
    state.next_filament_id = id
        .checked_add(1)
        .ok_or(BarStickError::FilamentIdExhausted(peak.key()))?;
    let stick = BarStick {
        id,
        peak: peak.key(),
        members: candidate.members,
        bounds: candidate.bounds,
        points: candidate.points,
        mean_curvature: candidate.mean_curvature,
        marked_brace: false,
    };
    state.sticks.push(stick.clone());
    Ok(Some(stick))
}

impl Error for BarStickError {}

#[derive(Clone, Copy)]
struct LocatedSection<'a> {
    location: LocatedSectionId,
    section: &'a Section,
}

/// Build and attach sticks in projector/peak order, then perform Java's
/// `detectCurvedPeaks` weak marking pass as each stick becomes available.
///
/// Peaks without a core filament are removed from the graph immediately.
/// Registration-ID exhaustion returns with all earlier graph and state
/// mutations intact, matching the non-transactional Java traversal.
pub fn build_bar_sticks(
    graph: &mut PeakGraph<BarAlignment>,
    peak_order: &[StaffPeakKey],
    vertical_sections: &[Section],
    horizontal_sections: &[Section],
    parameters: BarStickParameters,
    state: &mut BarStickBuildState,
) -> Result<(), BarStickError> {
    if parameters.vertical_extension < 0
        || parameters.minimum_core_section_length == 0
        || parameters.probe_width == 0
        || parameters.minimum_probe_weight == 0
        || parameters.segment_length == 0
        || !parameters.minimum_mean_curvature.is_finite()
        || parameters.minimum_mean_curvature < 0.0
        || parameters.first_filament_id != state.next_filament_id
    {
        return Err(BarStickError::InvalidParameters);
    }
    if !state.sticks.is_empty() || !state.removed_peaks.is_empty() {
        return Err(BarStickError::NonEmptyState);
    }

    // Java computes this global prefilter before touching the first peak.
    let peaks = peak_order
        .iter()
        .map(|&key| {
            graph
                .vertex(key)
                .cloned()
                .ok_or(BarStickError::MissingPeak(key))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let maximum_width = peaks
        .iter()
        .filter(|peak| !peak.is_brace())
        .map(|peak| peak.width())
        .max()
        .unwrap_or(0);
    let source = locate_sections(vertical_sections, horizontal_sections, maximum_width);

    for peak in peaks {
        let key = peak.key();
        let selected = select_peak_sections(&peak, parameters.vertical_extension, &source);
        let Some(candidate) = construct_stick(&peak, &selected, parameters) else {
            graph.remove_vertex(key);
            state.removed_peaks.push(key);
            continue;
        };

        // `FilamentIndex.register` happens before `StaffPeak.setFilament`.
        let id = state.next_filament_id;
        state.next_filament_id = id
            .checked_add(1)
            .ok_or(BarStickError::FilamentIdExhausted(key))?;
        state.sticks.push(BarStick {
            id,
            peak: key,
            members: candidate.members,
            bounds: candidate.bounds,
            points: candidate.points,
            mean_curvature: candidate.mean_curvature,
            marked_brace: false,
        });
    }

    // Java completes every `buildBarSticks` registration before entering the
    // separate `detectCurvedPeaks` traversal.
    for stick in &mut state.sticks {
        if stick.mean_curvature < parameters.minimum_mean_curvature {
            graph
                .vertex_mut(stick.peak)
                .expect("registered stick peak remains in graph")
                .set(StaffPeakAttribute::Brace);
            stick.marked_brace = true;
        }
    }
    Ok(())
}

fn locate_sections<'a>(
    vertical_sections: &'a [Section],
    horizontal_sections: &'a [Section],
    maximum_width: i32,
) -> Vec<LocatedSection<'a>> {
    sections_by_width(vertical_sections, horizontal_sections, maximum_width)
        .into_iter()
        .map(|location| {
            let source = match location.lag {
                SectionLag::Vertical => vertical_sections,
                SectionLag::Horizontal => horizontal_sections,
            };
            let section = source
                .iter()
                .find(|section| section.id() == location.id)
                .expect("section location was produced from this source lag");
            LocatedSection { location, section }
        })
        .collect()
}

fn select_peak_sections<'a>(
    peak: &crate::staff_peak::StaffPeak,
    vertical_extension: i32,
    source: &[LocatedSection<'a>],
) -> Vec<LocatedSection<'a>> {
    let bounds = peak.bounds();
    if bounds.width <= 0 || bounds.height <= 0 {
        return Vec::new();
    }
    let left = i128::from(bounds.x);
    let right = left + i128::from(bounds.width);
    let top = i128::from(bounds.y) - i128::from(vertical_extension);
    let bottom = i128::from(bounds.y) + i128::from(bounds.height) + i128::from(vertical_extension);
    let maximum_width = bounds.width as usize;
    let mut selected = Vec::new();
    for &candidate in source {
        let section = candidate.section.bounds();
        let section_left = section.x as i128;
        let section_right = section_left + section.width as i128;
        let section_top = section.y as i128;
        let section_bottom = section_top + section.height as i128;
        if section_left < right
            && section_right > left
            && section_top < bottom
            && section_bottom > top
        {
            if candidate.section.length(Orientation::Horizontal) <= maximum_width {
                selected.push(candidate);
            }
        } else if section_left >= right {
            break;
        }
    }
    selected
}

struct StickCandidate {
    members: Vec<LocatedSectionId>,
    bounds: Bounds,
    points: Vec<PeakPoint>,
    mean_curvature: f64,
}

fn construct_stick(
    peak: &crate::staff_peak::StaffPeak,
    source: &[LocatedSection<'_>],
    parameters: BarStickParameters,
) -> Option<StickCandidate> {
    let core = peak.bounds();
    if core.width <= 0 || core.height <= 0 {
        return None;
    }
    let mut members = Vec::<LocatedSection<'_>>::new();
    for &candidate in source {
        if candidate.section.length(Orientation::Vertical) >= parameters.minimum_core_section_length
            && intersects_peak(candidate.section.bounds(), core)
            && candidate
                .section
                .pixel_centroid_in(candidate.section.bounds())
                .is_some_and(|centroid| {
                    let rounded_x = centroid.0.round_ties_even() as i32;
                    rounded_x >= core.x && rounded_x < core.x.wrapping_add(core.width)
                })
        {
            insert_member(&mut members, candidate);
        }
    }
    if members.is_empty() {
        return None;
    }

    loop {
        let next = source.iter().copied().find(|candidate| {
            !contains_member(&members, *candidate)
                && members
                    .iter()
                    .any(|member| member.section.touches(candidate.section))
                && union_with_core_fits_width(core, candidate.section.bounds())
        });
        let Some(next) = next else { break };
        insert_member(&mut members, next);
    }

    let bounds = member_bounds(&members)?;
    let points = defining_points(&members, bounds, parameters)?;
    let mean_curvature = mean_curvature(&points);
    Some(StickCandidate {
        members: members.iter().map(|member| member.location).collect(),
        bounds,
        points,
        mean_curvature,
    })
}

fn insert_member<'a>(members: &mut Vec<LocatedSection<'a>>, candidate: LocatedSection<'a>) {
    if contains_member(members, candidate) {
        return;
    }
    members.push(candidate);
    members.sort_by_key(|member| {
        let bounds = member.section.bounds();
        (bounds.x, bounds.y, member.section.id())
    });
    members.dedup_by(|one, two| {
        let one_bounds = one.section.bounds();
        let two_bounds = two.section.bounds();
        (one_bounds.x, one_bounds.y, one.section.id())
            == (two_bounds.x, two_bounds.y, two.section.id())
    });
}

fn contains_member(members: &[LocatedSection<'_>], candidate: LocatedSection<'_>) -> bool {
    members.iter().any(|member| {
        let one = member.section.bounds();
        let two = candidate.section.bounds();
        (one.x, one.y, member.section.id()) == (two.x, two.y, candidate.section.id())
    })
}

fn intersects_peak(section: Bounds, peak: crate::staff_peak::PeakBounds) -> bool {
    let left = i128::from(peak.x);
    let right = left + i128::from(peak.width);
    let top = i128::from(peak.y);
    let bottom = top + i128::from(peak.height);
    (section.x as i128) < right
        && (section.x + section.width) as i128 > left
        && (section.y as i128) < bottom
        && (section.y + section.height) as i128 > top
}

fn union_with_core_fits_width(core: crate::staff_peak::PeakBounds, section: Bounds) -> bool {
    let core_left = i128::from(core.x);
    let core_right = core_left + i128::from(core.width);
    let section_left = section.x as i128;
    let section_right = section_left + section.width as i128;
    core_right.max(section_right) - core_left.min(section_left) <= i128::from(core.width)
}

fn member_bounds(members: &[LocatedSection<'_>]) -> Option<Bounds> {
    let first = members.first()?.section.bounds();
    let (mut left, mut top) = (first.x, first.y);
    let (mut right, mut bottom) = (first.x + first.width, first.y + first.height);
    for member in &members[1..] {
        let bounds = member.section.bounds();
        left = left.min(bounds.x);
        top = top.min(bounds.y);
        right = right.max(bounds.x + bounds.width);
        bottom = bottom.max(bounds.y + bounds.height);
    }
    Some(Bounds {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    })
}

fn defining_points(
    members: &[LocatedSection<'_>],
    bounds: Bounds,
    parameters: BarStickParameters,
) -> Option<Vec<PeakPoint>> {
    let orientation = if bounds.height > bounds.width {
        Orientation::Vertical
    } else {
        Orientation::Horizontal
    };
    let (oriented_start, oriented_stop, cross_start, cross_length) = match orientation {
        Orientation::Horizontal => (
            bounds.x as f64,
            (bounds.x + bounds.width - 1) as f64,
            bounds.y,
            bounds.height,
        ),
        Orientation::Vertical => (
            bounds.y as f64,
            (bounds.y + bounds.height - 1) as f64,
            bounds.x,
            bounds.width,
        ),
    };
    let length = oriented_stop - oriented_start + 1.0;
    let segment_count = (length / parameters.segment_length as f64).round_ties_even() as usize;
    let precise_segment_length = length / segment_count as f64;
    let mut points = Vec::with_capacity(segment_count.saturating_add(1));

    let first_coord = oriented_start.ceil() as usize;
    let first_roi = oriented_roi(
        orientation,
        first_coord,
        parameters.probe_width,
        cross_start,
        cross_length,
    );
    let first = compound_centroid(members, first_roi, 1)?;
    points.push(endpoint_point(orientation, oriented_start, first));

    for index in 1..segment_count {
        let coord =
            (oriented_start + (index as f64 * precise_segment_length)).round_ties_even() as usize;
        let roi = oriented_roi(
            orientation,
            coord,
            parameters.probe_width,
            cross_start,
            cross_length,
        );
        if let Some(point) = compound_centroid(members, roi, parameters.minimum_probe_weight) {
            points.push(point);
        }
    }

    let final_coord = (oriented_stop - parameters.probe_width as f64 + 1.0).floor() as usize;
    let final_roi = oriented_roi(
        orientation,
        final_coord,
        parameters.probe_width,
        cross_start,
        cross_length,
    );
    let last = compound_centroid(members, final_roi, 1)?;
    points.push(endpoint_point(orientation, oriented_stop, last));
    (points.len() >= 2).then_some(points)
}

fn endpoint_point(orientation: Orientation, coordinate: f64, centroid: PeakPoint) -> PeakPoint {
    match orientation {
        Orientation::Horizontal => PeakPoint::new(coordinate, centroid.y),
        Orientation::Vertical => PeakPoint::new(centroid.x, coordinate),
    }
}

fn oriented_roi(
    orientation: Orientation,
    coordinate: usize,
    coordinate_length: usize,
    cross: usize,
    cross_length: usize,
) -> Bounds {
    match orientation {
        Orientation::Horizontal => Bounds {
            x: coordinate,
            y: cross,
            width: coordinate_length,
            height: cross_length,
        },
        Orientation::Vertical => Bounds {
            x: cross,
            y: coordinate,
            width: cross_length,
            height: coordinate_length,
        },
    }
}

fn compound_centroid(
    members: &[LocatedSection<'_>],
    roi: Bounds,
    minimum_weight: usize,
) -> Option<PeakPoint> {
    let mut count = 0_usize;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    for member in members {
        let weight = member.section.pixel_count_in(roi);
        if weight == 0 {
            continue;
        }
        let centroid = member
            .section
            .pixel_centroid_in(roi)
            .expect("positive section weight has a centroid");
        count += weight;
        sum_x += centroid.0 * weight as f64;
        sum_y += centroid.1 * weight as f64;
    }
    (count >= minimum_weight).then(|| PeakPoint::new(sum_x / count as f64, sum_y / count as f64))
}

fn mean_curvature(points: &[PeakPoint]) -> f64 {
    if points.len() < 3 {
        return f64::INFINITY;
    }
    let mut sum = 0.0;
    let mut count = 0_usize;
    for triple in points.windows(3) {
        let one = bisector(triple[0], triple[1]);
        let two = bisector(triple[1], triple[2]);
        let center = line_intersection(one, two);
        let radius = (center.x - triple[1].x).hypot(center.y - triple[1].y);
        sum += 1.0 / radius;
        count += 1;
    }
    1.0 / (sum / count as f64)
}

fn bisector(one: PeakPoint, two: PeakPoint) -> (PeakPoint, PeakPoint) {
    let half_dx = (two.x - one.x) / 2.0;
    let half_dy = (two.y - one.y) / 2.0;
    let middle = PeakPoint::new(one.x + half_dx, one.y + half_dy);
    (
        PeakPoint::new(middle.x + half_dy, middle.y - half_dx),
        PeakPoint::new(middle.x - half_dy, middle.y + half_dx),
    )
}

fn line_intersection(one: (PeakPoint, PeakPoint), two: (PeakPoint, PeakPoint)) -> PeakPoint {
    let denominator =
        ((one.0.x - one.1.x) * (two.0.y - two.1.y)) - ((one.0.y - one.1.y) * (two.0.x - two.1.x));
    let cross_one = (one.0.x * one.1.y) - (one.0.y * one.1.x);
    let cross_two = (two.0.x * two.1.y) - (two.0.y * two.1.x);
    PeakPoint::new(
        ((cross_one * (two.0.x - two.1.x)) - ((one.0.x - one.1.x) * cross_two)) / denominator,
        ((cross_one * (two.0.y - two.1.y)) - ((one.0.y - one.1.y) * cross_two)) / denominator,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        run_table::{Run, RunTable},
        section::{JunctionPolicy, build_sections},
        staff_peak::StaffPeak,
    };

    fn peak(staff: usize, start: i32, stop: i32) -> StaffPeak {
        let mut peak =
            StaffPeak::new(crate::bar_column::StaffId::new(staff), 2, 9, start, stop).unwrap();
        peak.compute_deskewed_center(|point| point).unwrap();
        peak
    }

    fn two_bar_sections() -> (Vec<Section>, Vec<Section>) {
        let mut vertical = RunTable::new(Orientation::Vertical, 24, 14).unwrap();
        vertical.add_run(5, Run::new(2, 8)).unwrap();
        vertical.add_run(12, Run::new(2, 8)).unwrap();
        let mut horizontal = RunTable::new(Orientation::Horizontal, 24, 14).unwrap();
        horizontal.add_run(1, Run::new(5, 2)).unwrap();
        horizontal.add_run(10, Run::new(12, 2)).unwrap();
        (
            build_sections(&vertical, JunctionPolicy::All),
            build_sections(&horizontal, JunctionPolicy::All),
        )
    }

    fn parameters(first_filament_id: usize) -> BarStickParameters {
        BarStickParameters {
            vertical_extension: 2,
            minimum_core_section_length: 4,
            probe_width: 2,
            minimum_probe_weight: 1,
            segment_length: 4,
            minimum_mean_curvature: 0.0,
            first_filament_id,
        }
    }

    #[test]
    fn attaches_sections_and_registers_sticks_in_peak_order() {
        let (vertical, horizontal) = two_bar_sections();
        let first = peak(1, 5, 6);
        let second = peak(2, 12, 13);
        let order = [second.key(), first.key()];
        let mut graph = PeakGraph::new();
        assert!(graph.add_vertex(first));
        assert!(graph.add_vertex(second));
        let mut state = BarStickBuildState::new(10).unwrap();

        build_bar_sticks(
            &mut graph,
            &order,
            &vertical,
            &horizontal,
            parameters(10),
            &mut state,
        )
        .unwrap();

        assert_eq!(
            state
                .sticks()
                .iter()
                .map(|stick| (stick.id, stick.peak))
                .collect::<Vec<_>>(),
            [(10, order[0]), (11, order[1])]
        );
        assert_eq!(
            state.sticks()[0]
                .members
                .iter()
                .map(|member| member.lag)
                .collect::<Vec<_>>(),
            [SectionLag::Vertical, SectionLag::Horizontal]
        );
        assert_eq!(
            state.sticks()[1]
                .members
                .iter()
                .map(|member| member.lag)
                .collect::<Vec<_>>(),
            [SectionLag::Horizontal, SectionLag::Vertical]
        );
        assert!(state.removed_peaks().is_empty());
        assert_eq!(graph.vertices().len(), 2);
        assert!(graph.edges().is_empty());
    }

    #[test]
    fn missing_core_removes_vertex_but_later_peak_still_builds() {
        let (vertical, horizontal) = two_bar_sections();
        let missing = peak(1, 18, 19);
        let present = peak(2, 12, 13);
        let order = [missing.key(), present.key()];
        let mut graph = PeakGraph::new();
        assert!(graph.add_vertex(missing));
        assert!(graph.add_vertex(present));
        let mut state = BarStickBuildState::new(20).unwrap();

        build_bar_sticks(
            &mut graph,
            &order,
            &vertical,
            &horizontal,
            parameters(20),
            &mut state,
        )
        .unwrap();

        assert_eq!(state.removed_peaks(), [order[0]]);
        assert_eq!(state.sticks().len(), 1);
        assert_eq!(state.sticks()[0].peak, order[1]);
        assert!(!graph.contains_vertex(order[0]));
        assert!(graph.contains_vertex(order[1]));
    }

    #[test]
    fn sub_sticks_use_only_old_members_and_register_before_graph_attachment() {
        let (vertical, horizontal) = two_bar_sections();
        let old = peak(1, 5, 13);
        let mut graph = PeakGraph::new();
        graph.add_vertex(old.clone());
        let mut state = BarStickBuildState::new(30).unwrap();
        build_bar_sticks(
            &mut graph,
            &[old.key()],
            &vertical,
            &horizontal,
            parameters(30),
            &mut state,
        )
        .unwrap();
        let old_members = state.sticks()[0].members.clone();
        let left = peak(1, 5, 8);
        let right = peak(1, 10, 13);
        let left_stick = build_sub_stick(
            &left,
            &old_members,
            &vertical,
            &horizontal,
            parameters(31),
            &mut state,
        )
        .unwrap()
        .unwrap();
        let right_stick = build_sub_stick(
            &right,
            &old_members,
            &vertical,
            &horizontal,
            parameters(32),
            &mut state,
        )
        .unwrap()
        .unwrap();

        assert_eq!((left_stick.id, right_stick.id), (31, 32));
        assert_eq!(state.next_filament_id(), 33);
        assert!(!graph.contains_vertex(left.key()));
        assert!(!graph.contains_vertex(right.key()));
        assert!(
            left_stick
                .members
                .iter()
                .all(|member| old_members.contains(member))
        );
        assert!(
            right_stick
                .members
                .iter()
                .all(|member| old_members.contains(member))
        );
    }

    #[test]
    fn registration_failure_retains_successful_prefix() {
        let (vertical, horizontal) = two_bar_sections();
        let first = peak(1, 5, 6);
        let second = peak(2, 12, 13);
        let order = [first.key(), second.key()];
        let mut graph = PeakGraph::new();
        assert!(graph.add_vertex(first));
        assert!(graph.add_vertex(second));
        let mut state = BarStickBuildState::new(usize::MAX - 1).unwrap();

        assert_eq!(
            build_bar_sticks(
                &mut graph,
                &order,
                &vertical,
                &horizontal,
                parameters(usize::MAX - 1),
                &mut state,
            ),
            Err(BarStickError::FilamentIdExhausted(order[1]))
        );
        assert_eq!(state.sticks().len(), 1);
        assert_eq!(state.sticks()[0].id, usize::MAX - 1);
        assert_eq!(state.sticks()[0].peak, order[0]);
        assert!(graph.contains_vertex(order[0]));
        assert!(graph.contains_vertex(order[1]));
        assert!(graph.edges().is_empty());
    }

    #[test]
    fn finite_curvature_is_marked_only_after_stick_registration() {
        let mut table = RunTable::new(Orientation::Horizontal, 16, 12).unwrap();
        for (y, x) in [5, 5, 6, 7, 7, 6, 5, 5].into_iter().enumerate() {
            table.add_run(y + 2, Run::new(x, 2)).unwrap();
        }
        let sections = build_sections(&table, JunctionPolicy::All);
        assert_eq!(sections.len(), 1);
        let peak = peak(1, 5, 8);
        let key = peak.key();
        let mut graph = PeakGraph::new();
        assert!(graph.add_vertex(peak));
        let mut state = BarStickBuildState::new(1).unwrap();
        let mut tuning = parameters(1);
        tuning.segment_length = 2;
        tuning.minimum_mean_curvature = 1.0e9;

        build_bar_sticks(&mut graph, &[key], &[], &sections, tuning, &mut state).unwrap();

        assert_eq!(state.sticks().len(), 1);
        assert!(state.sticks()[0].mean_curvature.is_finite());
        assert!(state.sticks()[0].marked_brace);
        assert!(graph.vertex(key).unwrap().is_set(StaffPeakAttribute::Brace));
    }
}
