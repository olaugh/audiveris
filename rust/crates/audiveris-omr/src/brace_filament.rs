// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact neutral boundary for Java `BarsRetriever.buildBraceFilament`.
//!
//! This module retains the source polygon, `Area.contains(Rectangle)` selection
//! contract, `CompoundFactory.buildCompoundFromParts` member identity, and the
//! restart-after-first-touch expansion order. Glyph ownership and SIG insertion
//! intentionally begin after this boundary.

use std::{error::Error, fmt};

use audiveris_image::{section::Section, staff_peak::StaffPeak};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BraceFilamentParameters {
    pub interline: usize,
    pub segment_length: usize,
    pub left_margin: i32,
}

#[derive(Clone, Debug)]
pub struct BraceFilamentPortion {
    pub peak: StaffPeak,
    pub members: Vec<Section>,
}

/// The production identity of a newly constructed Java `CurvedFilament`.
///
/// Its member set is already sufficient for `SectionCompound.toGlyph`. Spline
/// sampling remains lazy in Java and is not forced by brace construction.
#[derive(Clone, Debug)]
pub struct CurvedFilamentCompound {
    interline: usize,
    segment_length: usize,
    members: Vec<Section>,
}

impl CurvedFilamentCompound {
    #[must_use]
    pub const fn interline(&self) -> usize {
        self.interline
    }

    #[must_use]
    pub const fn segment_length(&self) -> usize {
        self.segment_length
    }

    #[must_use]
    pub fn members(&self) -> &[Section] {
        &self.members
    }

    #[must_use]
    pub fn touches(&self, section: &Section) -> bool {
        self.members.iter().any(|member| section.touches(member))
    }

    fn add_section(&mut self, section: Section) {
        let identity = section_identity(&section);
        if !self
            .members
            .iter()
            .any(|member| section_identity(member) == identity)
        {
            self.members.push(section);
            self.members.sort_by_key(section_identity);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BraceFilamentError {
    InvalidParameters,
    EmptyPortions,
    EmptyCompound,
}

impl fmt::Display for BraceFilamentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParameters => formatter.write_str("invalid brace-filament parameters"),
            Self::EmptyPortions => formatter.write_str("brace filament has no portions"),
            Self::EmptyCompound => {
                formatter.write_str("cannot build a curved filament from no sections")
            }
        }
    }
}

impl Error for BraceFilamentError {}

/// Java `Path2D` vertices in insertion order, excluding the implicit closing
/// edge. Integer arithmetic deliberately wraps before conversion to `f64`.
#[must_use]
pub fn brace_area_vertices(portions: &[BraceFilamentPortion], left_margin: i32) -> Vec<(f64, f64)> {
    let Some(top) = portions.first() else {
        return Vec::new();
    };
    let mut vertices = Vec::with_capacity((4 * portions.len()) + 1);
    vertices.push((
        f64::from(top.peak.start().wrapping_sub(left_margin)),
        f64::from(top.peak.top()),
    ));

    for portion in portions {
        vertices.push((
            f64::from(portion.peak.stop().wrapping_add(1)),
            f64::from(portion.peak.top()),
        ));
        vertices.push((
            f64::from(portion.peak.stop().wrapping_add(1)),
            f64::from(portion.peak.bottom().wrapping_add(1)),
        ));
    }

    for (reverse_index, portion) in portions.iter().rev().enumerate() {
        let left = f64::from(portion.peak.start().wrapping_sub(left_margin));
        vertices.push((left, f64::from(portion.peak.bottom().wrapping_add(1))));
        if reverse_index + 1 < portions.len() {
            vertices.push((left, f64::from(portion.peak.top())));
        }
    }
    vertices
}

/// Java `getAreaSections`: preserve input order, require full rectangle
/// containment, and stop at the first section whose left edge reaches the
/// area's exclusive right bound.
#[must_use]
pub fn select_brace_area_sections(
    vertices: &[(f64, f64)],
    all_sections: &[Section],
) -> Vec<Section> {
    let Some(maximum_x) = vertices.iter().map(|point| point.0).reduce(f64::max) else {
        return Vec::new();
    };
    // Area.getBounds().x + width. Brace vertices are integral, but retaining
    // ceil makes the contract explicit if a neutral caller supplies fractions.
    let x_break = maximum_x.ceil();
    let mut selected = Vec::new();
    for section in all_sections {
        let bounds = section.bounds();
        if polygon_contains_bounds(vertices, bounds) {
            selected.push(section.clone());
        } else if bounds.x as f64 >= x_break {
            break;
        }
    }
    selected
}

/// Port `CompoundFactory.buildCompoundFromParts`, then the source do/while
/// touch expansion. Failures occur before any externally visible compound.
pub fn build_brace_filament(
    portions: &[BraceFilamentPortion],
    all_brace_sections: &[Section],
    parameters: BraceFilamentParameters,
) -> Result<CurvedFilamentCompound, BraceFilamentError> {
    if parameters.interline == 0 || parameters.segment_length == 0 || parameters.left_margin < 0 {
        return Err(BraceFilamentError::InvalidParameters);
    }
    if portions.is_empty() {
        return Err(BraceFilamentError::EmptyPortions);
    }

    let vertices = brace_area_vertices(portions, parameters.left_margin);
    let mut candidates = select_brace_area_sections(&vertices, all_brace_sections);
    let portion_identities = portions
        .iter()
        .flat_map(|portion| portion.members.iter().map(section_identity))
        .collect::<Vec<_>>();
    candidates.retain(|section| !portion_identities.contains(&section_identity(section)));

    let mut compound = CurvedFilamentCompound {
        interline: parameters.interline,
        segment_length: parameters.segment_length,
        members: Vec::new(),
    };
    for portion in portions {
        for member in &portion.members {
            compound.add_section(member.clone());
        }
    }
    if compound.members.is_empty() {
        return Err(BraceFilamentError::EmptyCompound);
    }

    while let Some(index) = candidates
        .iter()
        .position(|section| compound.touches(section))
    {
        compound.add_section(candidates.remove(index));
    }

    Ok(compound)
}

fn section_identity(section: &Section) -> (usize, usize, usize) {
    let bounds = section.bounds();
    (bounds.x, bounds.y, section.id())
}

fn polygon_contains_bounds(
    vertices: &[(f64, f64)],
    bounds: audiveris_image::section::Bounds,
) -> bool {
    if vertices.len() < 3 {
        return false;
    }
    let left = bounds.x as f64;
    let top = bounds.y as f64;
    let right = bounds.x.saturating_add(bounds.width) as f64;
    let bottom = bounds.y.saturating_add(bounds.height) as f64;
    let corners = [(left, top), (right, top), (right, bottom), (left, bottom)];
    if !corners
        .into_iter()
        .all(|corner| polygon_contains_point(vertices, corner))
    {
        return false;
    }

    // Four contained corners are insufficient for a concave polygon. Reject
    // when any polygon boundary crosses the rectangle's open interior. Edges
    // coincident with its boundary remain accepted, as with Area.contains.
    vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1))
        .take(vertices.len())
        .all(|(one, two)| !segment_meets_open_rectangle(one, two, left, top, right, bottom))
}

fn polygon_contains_point(vertices: &[(f64, f64)], point: (f64, f64)) -> bool {
    let mut inside = false;
    for (one, two) in vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1))
        .take(vertices.len())
    {
        if point_on_segment(point, one, two) {
            return true;
        }
        if (one.1 > point.1) != (two.1 > point.1) {
            let crossing_x = one.0 + ((point.1 - one.1) * (two.0 - one.0) / (two.1 - one.1));
            if point.0 < crossing_x {
                inside = !inside;
            }
        }
    }
    inside
}

fn point_on_segment(point: (f64, f64), one: (f64, f64), two: (f64, f64)) -> bool {
    let cross = ((point.0 - one.0) * (two.1 - one.1)) - ((point.1 - one.1) * (two.0 - one.0));
    let scale = (two.0 - one.0).abs().max((two.1 - one.1).abs()).max(1.0);
    if cross.abs() > f64::EPSILON * scale * scale * 8.0 {
        return false;
    }
    point.0 >= one.0.min(two.0)
        && point.0 <= one.0.max(two.0)
        && point.1 >= one.1.min(two.1)
        && point.1 <= one.1.max(two.1)
}

fn segment_meets_open_rectangle(
    one: (f64, f64),
    two: (f64, f64),
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
) -> bool {
    let mut parameters = vec![0.0, 1.0];
    let delta_x = two.0 - one.0;
    let delta_y = two.1 - one.1;
    if delta_x != 0.0 {
        parameters.extend([(left - one.0) / delta_x, (right - one.0) / delta_x]);
    }
    if delta_y != 0.0 {
        parameters.extend([(top - one.1) / delta_y, (bottom - one.1) / delta_y]);
    }
    parameters.retain(|parameter| *parameter >= 0.0 && *parameter <= 1.0);
    parameters.sort_by(f64::total_cmp);
    parameters.dedup_by(|one, two| one.total_cmp(two).is_eq());

    parameters.windows(2).any(|pair| {
        let parameter = (pair[0] + pair[1]) / 2.0;
        let point = (one.0 + (parameter * delta_x), one.1 + (parameter * delta_y));
        point.0 > left && point.0 < right && point.1 > top && point.1 < bottom
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiveris_image::{
        bar_column::StaffId,
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections_from_id},
    };

    fn rectangle_section(id: usize, x: usize, y: usize, width: usize, height: usize) -> Section {
        let mut table =
            RunTable::new(Orientation::Vertical, x + width + 1, y + height + 1).unwrap();
        for column in x..(x + width) {
            table.add_run(column, Run::new(y, height)).unwrap();
        }
        build_sections_from_id(&table, JunctionPolicy::All, id)
            .into_iter()
            .find(|section| section.id() == id)
            .unwrap()
    }

    fn portion(
        staff: usize,
        top: i32,
        bottom: i32,
        start: i32,
        stop: i32,
        members: Vec<Section>,
    ) -> BraceFilamentPortion {
        BraceFilamentPortion {
            peak: StaffPeak::new(StaffId::new(staff), top, bottom, start, stop).unwrap(),
            members,
        }
    }

    fn parameters() -> BraceFilamentParameters {
        BraceFilamentParameters {
            interline: 10,
            segment_length: 20,
            left_margin: 2,
        }
    }

    #[test]
    fn area_vertices_follow_source_path_order() {
        let portions = [
            portion(1, 10, 20, 10, 12, vec![rectangle_section(1, 10, 10, 1, 4)]),
            portion(2, 30, 40, 11, 13, vec![rectangle_section(2, 11, 36, 1, 4)]),
        ];
        assert_eq!(
            brace_area_vertices(&portions, 2),
            [
                (8.0, 10.0),
                (13.0, 10.0),
                (13.0, 21.0),
                (14.0, 30.0),
                (14.0, 41.0),
                (9.0, 41.0),
                (9.0, 30.0),
                (8.0, 21.0),
            ]
        );
    }

    #[test]
    fn selection_accepts_boundary_and_obeys_far_right_break() {
        let portions = [portion(
            1,
            10,
            30,
            10,
            20,
            vec![rectangle_section(1, 10, 10, 1, 4)],
        )];
        let vertices = brace_area_vertices(&portions, 2);
        let left_boundary = rectangle_section(10, 8, 12, 1, 2);
        let inside = rectangle_section(11, 12, 12, 1, 2);
        let far_right = rectangle_section(12, 21, 12, 1, 2);
        let later_inside = rectangle_section(13, 12, 16, 1, 2);
        let selected = select_brace_area_sections(
            &vertices,
            &[left_boundary, inside, far_right, later_inside],
        );
        assert_eq!(
            selected.iter().map(Section::id).collect::<Vec<_>>(),
            [10, 11]
        );
    }

    #[test]
    fn compound_flattens_parts_then_restarts_touch_expansion() {
        let top = rectangle_section(1, 10, 10, 1, 5);
        let bottom = rectangle_section(2, 10, 30, 1, 5);
        let near = rectangle_section(3, 10, 15, 1, 8);
        let far = rectangle_section(4, 10, 23, 1, 7);
        let isolated = rectangle_section(5, 15, 18, 1, 2);
        let portions = [
            portion(1, 10, 20, 5, 20, vec![top.clone()]),
            portion(2, 25, 35, 5, 20, vec![bottom.clone()]),
        ];
        // `far` is encountered first but cannot join until `near` has joined;
        // Java restarts the scan after that first successful addition.
        let filament =
            build_brace_filament(&portions, &[top, far, near, bottom, isolated], parameters())
                .unwrap();

        assert_eq!(filament.interline(), 10);
        assert_eq!(filament.segment_length(), 20);
        assert_eq!(
            filament
                .members()
                .iter()
                .map(Section::id)
                .collect::<Vec<_>>(),
            [1, 3, 4, 2]
        );
    }

    #[test]
    fn empty_part_population_is_rejected_like_compound_factory() {
        let portions = [portion(1, 10, 20, 5, 20, Vec::new())];
        assert!(matches!(
            build_brace_filament(&portions, &[], parameters()),
            Err(BraceFilamentError::EmptyCompound)
        ));
    }
}
