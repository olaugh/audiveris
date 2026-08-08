// SPDX-License-Identifier: AGPL-3.0-or-later

//! Native evidence for Java `MultipleRestsBuilder` serif discovery and
//! `BarFilamentBuilder` materialization.
//!
//! Identity allocation and SIG mutation deliberately remain in the OMR layer.

use crate::{
    bars_logic::{LocatedSectionId, SectionLag},
    beam_structure::Segment,
    projection::{
        MultiRestSideRequest, NeutralStaffProjectorResult, PeakCoreGeometry, ProjectionError,
    },
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable, RunTableError},
    section::{Bounds, Section},
    staff_peak::{HorizontalSide, PeakBounds, StaffPeak, StaffVerticalImpacts},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SerifStaffLines {
    pub first: Segment,
    pub middle: Segment,
    pub last: Segment,
    pub middle_thickness: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MultipleRestSerifSearchEvidence {
    pub added_chunk: i32,
    pub left_peaks: Vec<StaffPeak>,
    pub right_peaks: Vec<StaffPeak>,
    /// Java chooses the rightmost LEFT peak and leftmost RIGHT peak.
    pub left: Option<StaffPeak>,
    pub right: Option<StaffPeak>,
}

#[derive(Clone, Copy, Debug)]
pub struct MultipleRestSerifSearchInput<'a> {
    pub projector: &'a NeutralStaffProjectorResult,
    pub raster_width: usize,
    pub raster_height: usize,
    pub pixels: &'a [u8],
    pub beam_bounds: PeakBounds,
    pub beam_height: f64,
    pub staff_lines: SerifStaffLines,
    /// Scale-resolved template. `x`, `side`, and `added_chunk` are replaced
    /// for each Java LEFT/RIGHT call.
    pub request: MultiRestSideRequest,
}

/// Serif-search inputs when the caller owns richer staff-line geometry than
/// the straight [`Segment`] boundary used by [`MultipleRestSerifSearchInput`].
#[derive(Clone, Copy, Debug)]
pub struct MultipleRestSerifGeometrySearchInput<'a> {
    pub projector: &'a NeutralStaffProjectorResult,
    pub raster_width: usize,
    pub raster_height: usize,
    pub pixels: &'a [u8],
    pub beam_bounds: PeakBounds,
    pub beam_height: f64,
    pub middle_line_thickness: f64,
    pub request: MultiRestSideRequest,
}

pub fn search_multiple_rest_serifs(
    input: MultipleRestSerifSearchInput<'_>,
) -> Result<MultipleRestSerifSearchEvidence, ProjectionError> {
    search_multiple_rest_serifs_with_geometry(
        MultipleRestSerifGeometrySearchInput {
            projector: input.projector,
            raster_width: input.raster_width,
            raster_height: input.raster_height,
            pixels: input.pixels,
            beam_bounds: input.beam_bounds,
            beam_height: input.beam_height,
            middle_line_thickness: input.staff_lines.middle_thickness,
            request: input.request,
        },
        |x| {
            PeakCoreGeometry::new(
                y_at_x(input.staff_lines.first, x),
                y_at_x(input.staff_lines.last, x),
                y_at_x(input.staff_lines.middle, x),
            )
        },
    )
}

/// Java `getSerifPeaks` with caller-owned current staff-line geometry.
///
/// `StaffProjector` owns its projection independently of the staff-line
/// geometry that `processMultiRestSide` reads while constructing peak cores.
/// This callback boundary preserves that split, including a projector created
/// afresh during BEAMS from the completed lines.
pub fn search_multiple_rest_serifs_with_geometry<Geometry>(
    input: MultipleRestSerifGeometrySearchInput<'_>,
    mut core_geometry_at: Geometry,
) -> Result<MultipleRestSerifSearchEvidence, ProjectionError>
where
    Geometry: FnMut(i32) -> PeakCoreGeometry,
{
    let added_chunk = (input.beam_height - input.middle_line_thickness).round_ties_even() as i32;
    let left_x = input.beam_bounds.x;
    let right_x = input
        .beam_bounds
        .x
        .wrapping_add(input.beam_bounds.width)
        .wrapping_sub(1);
    let left_peaks = input.projector.process_multi_rest_side(
        input.raster_width,
        input.raster_height,
        input.pixels,
        MultiRestSideRequest {
            x: left_x,
            side: HorizontalSide::Left,
            added_chunk,
            ..input.request
        },
        &mut core_geometry_at,
    )?;
    let right_peaks = input.projector.process_multi_rest_side(
        input.raster_width,
        input.raster_height,
        input.pixels,
        MultiRestSideRequest {
            x: right_x,
            side: HorizontalSide::Right,
            added_chunk,
            ..input.request
        },
        core_geometry_at,
    )?;
    Ok(MultipleRestSerifSearchEvidence {
        added_chunk,
        left: left_peaks.last().cloned(),
        right: right_peaks.first().cloned(),
        left_peaks,
        right_peaks,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SerifFilamentParameters {
    pub minimum_core_section_length: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SerifGlyphEvidence {
    pub peak: StaffPeak,
    pub member_section_ids: Vec<LocatedSectionId>,
    pub left: i32,
    pub top: i32,
    pub raster: RunTable,
    pub weight: usize,
    pub centroid_x: f64,
    pub centroid_y: f64,
    pub median: Segment,
    pub width: f64,
    pub impacts: Option<StaffVerticalImpacts>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SerifMaterializationError {
    InvalidParameters,
    NoCoreSection,
    InvalidBounds,
    Raster(RunTableError),
}

impl From<RunTableError> for SerifMaterializationError {
    fn from(value: RunTableError) -> Self {
        Self::Raster(value)
    }
}

/// Port of `getSectionsByWidth`, `BarFilamentBuilder.buildFilament`,
/// `BarFilamentFactory`, and `SectionCompound.toGlyph` for one serif peak.
pub fn materialize_serif_glyph(
    peak: StaffPeak,
    maximum_serif_width: i32,
    system_bounds: PeakBounds,
    vertical_sections: &[Section],
    horizontal_sections: &[Section],
    foreground_thickness: f64,
    parameters: SerifFilamentParameters,
) -> Result<SerifGlyphEvidence, SerifMaterializationError> {
    if maximum_serif_width < 0
        || parameters.minimum_core_section_length == 0
        || !foreground_thickness.is_finite()
        || foreground_thickness < 0.0
    {
        return Err(SerifMaterializationError::InvalidParameters);
    }
    let mut source = vertical_sections
        .iter()
        .filter(|section| eligible_section(section, maximum_serif_width, system_bounds))
        .map(|section| LocatedSection {
            id: LocatedSectionId {
                lag: SectionLag::Vertical,
                id: section.id(),
            },
            section,
        })
        .chain(
            horizontal_sections
                .iter()
                .filter(|section| eligible_section(section, maximum_serif_width, system_bounds))
                .map(|section| LocatedSection {
                    id: LocatedSectionId {
                        lag: SectionLag::Horizontal,
                        id: section.id(),
                    },
                    section,
                }),
        )
        .collect::<Vec<_>>();
    // Stable x-only sort retains VLAG-before-HLAG and entity order on ties.
    source.sort_by_key(|candidate| candidate.section.bounds().x);

    let core = peak.bounds();
    if core.width <= 0 || core.height <= 0 {
        return Err(SerifMaterializationError::InvalidBounds);
    }
    let mut candidates = Vec::new();
    let right = i128::from(core.x) + i128::from(core.width);
    for candidate in source {
        let bounds = candidate.section.bounds();
        if positive_intersection(bounds, core) {
            if candidate.section.length(Orientation::Horizontal) <= core.width as usize {
                candidates.push(candidate);
            }
        } else if bounds.x as i128 >= right {
            break;
        }
    }

    let mut members = Vec::new();
    for candidate in &candidates {
        let centroid_x = candidate.section.centroid().0 as i32;
        if candidate.section.length(Orientation::Vertical) >= parameters.minimum_core_section_length
            && positive_intersection(candidate.section.bounds(), core)
            && centroid_x >= core.x
            && centroid_x < core.x.wrapping_add(core.width)
        {
            members.push(*candidate);
        }
    }
    if members.is_empty() {
        return Err(SerifMaterializationError::NoCoreSection);
    }

    loop {
        let next = candidates.iter().copied().find(|candidate| {
            !members.iter().any(|member| member.id == candidate.id)
                && members
                    .iter()
                    .any(|member| member.section.touches(candidate.section))
                && union_fits_core_width(core, candidate.section.bounds())
        });
        let Some(next) = next else { break };
        members.push(next);
    }

    let bounds = member_bounds(&members).ok_or(SerifMaterializationError::InvalidBounds)?;
    let mut pixels = vec![BACKGROUND; bounds.width * bounds.height];
    for member in &members {
        let section_bounds = member.section.bounds();
        for y in section_bounds.y..section_bounds.y + section_bounds.height {
            for x in section_bounds.x..section_bounds.x + section_bounds.width {
                if member.section.pixel_count_in(Bounds {
                    x,
                    y,
                    width: 1,
                    height: 1,
                }) != 0
                {
                    pixels[(y - bounds.y) * bounds.width + (x - bounds.x)] = FOREGROUND;
                }
            }
        }
    }
    let orientation = if bounds.width > bounds.height {
        Orientation::Horizontal
    } else {
        Orientation::Vertical
    };
    let raster = RunTable::from_pixels(orientation, bounds.width, bounds.height, &pixels)?;
    let mut weight = 0_usize;
    let mut sum_x = 0_f64;
    let mut sum_y = 0_f64;
    for (index, pixel) in pixels.iter().enumerate() {
        if *pixel == FOREGROUND {
            weight += 1;
            sum_x += (bounds.x + (index % bounds.width)) as f64;
            sum_y += (bounds.y + (index / bounds.width)) as f64;
        }
    }
    if weight == 0 {
        return Err(SerifMaterializationError::NoCoreSection);
    }
    let half_line = foreground_thickness / 2.0;
    let x = f64::from(peak.start()) + (f64::from(peak.width()) / 2.0);
    Ok(SerifGlyphEvidence {
        impacts: peak.impacts(),
        median: Segment {
            x1: x,
            y1: f64::from(peak.top()) - half_line + 0.5,
            x2: x,
            y2: f64::from(peak.bottom()) + half_line + 0.5,
        },
        width: f64::from(peak.width()),
        peak,
        member_section_ids: members.iter().map(|member| member.id).collect(),
        left: bounds.x as i32,
        top: bounds.y as i32,
        raster,
        weight,
        centroid_x: sum_x / weight as f64,
        centroid_y: sum_y / weight as f64,
    })
}

#[derive(Clone, Copy)]
struct LocatedSection<'a> {
    id: LocatedSectionId,
    section: &'a Section,
}

fn y_at_x(line: Segment, x: i32) -> i32 {
    let slope = (line.y2 - line.y1) / (line.x2 - line.x1);
    (line.y1 + (slope * (f64::from(x) - line.x1))).round_ties_even() as i32
}

fn eligible_section(section: &Section, maximum_width: i32, system: PeakBounds) -> bool {
    section.length(Orientation::Horizontal) <= maximum_width as usize
        && positive_intersection(section.bounds(), system)
}

fn positive_intersection(section: Bounds, area: PeakBounds) -> bool {
    area.width > 0
        && area.height > 0
        && (section.x as i128) < i128::from(area.x) + i128::from(area.width)
        && (section.x + section.width) as i128 > i128::from(area.x)
        && (section.y as i128) < i128::from(area.y) + i128::from(area.height)
        && (section.y + section.height) as i128 > i128::from(area.y)
}

fn union_fits_core_width(core: PeakBounds, section: Bounds) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bar_column::StaffId,
        projection::{
            MultiRestSideRequest, PeakConstructionParams, PeakCoreParams, PeakRefinementParams,
            PeakSearchBounds, ShortProjection, StaffProjectionRequest,
        },
        run_table::RunTable,
        section::{JunctionPolicy, build_sections_from_id},
    };

    #[test]
    fn search_selects_rightmost_left_and_leftmost_right_peak() {
        let width = 16;
        let height = 9;
        let mut pixels = vec![BACKGROUND; width * height];
        for x in 0..width {
            pixels[x] = FOREGROUND;
            pixels[(height - 1) * width + x] = FOREGROUND;
        }
        for y in 2..=6 {
            pixels[y * width + 5] = FOREGROUND;
            pixels[y * width + 10] = FOREGROUND;
        }
        let accumulation = ShortProjection::from_staff_raster(
            width,
            height,
            &pixels,
            StaffProjectionRequest::new(0, 15, 0),
            |_| 0,
            |_| 9,
        )
        .unwrap();
        let projector = NeutralStaffProjectorResult {
            projection: accumulation.projection,
            derivative_threshold: 5,
            all_blanks: Vec::new(),
            peak_search_bounds: PeakSearchBounds {
                x_min: 0,
                x_max: 15,
            },
            peaks: Vec::new(),
            brace_candidate: None,
        };
        let refinement = PeakRefinementParams::new(6, 2, 4, 2, 1).unwrap();
        let evidence = search_multiple_rest_serifs(MultipleRestSerifSearchInput {
            projector: &projector,
            raster_width: width,
            raster_height: height,
            pixels: &pixels,
            beam_bounds: PeakBounds {
                x: 5,
                y: 4,
                width: 6,
                height: 2,
            },
            beam_height: 2.5,
            staff_lines: SerifStaffLines {
                first: Segment {
                    x1: 0.0,
                    y1: 0.0,
                    x2: 15.0,
                    y2: 0.0,
                },
                middle: Segment {
                    x1: 0.0,
                    y1: 4.0,
                    x2: 15.0,
                    y2: 4.0,
                },
                last: Segment {
                    x1: 0.0,
                    y1: 8.0,
                    x2: 15.0,
                    y2: 8.0,
                },
                middle_thickness: 1.0,
            },
            request: MultiRestSideRequest {
                x: 0,
                side: HorizontalSide::Left,
                added_chunk: 0,
                vertical_serif_width: 4,
                bar_threshold: 6,
                lines_threshold: 2,
                total_height: 12,
                staff_id: StaffId::new(1),
                peak_construction: PeakConstructionParams::new(refinement, 4).unwrap(),
                peak_core: PeakCoreParams::new(1, 0.2).unwrap(),
            },
        })
        .unwrap();
        assert_eq!(evidence.added_chunk, 2);
        assert_eq!(
            evidence
                .left_peaks
                .iter()
                .map(StaffPeak::start)
                .collect::<Vec<_>>(),
            vec![5, 10]
        );
        assert_eq!(evidence.left.as_ref().map(StaffPeak::start), Some(10));
        assert_eq!(evidence.right.as_ref().map(StaffPeak::start), Some(5));
    }

    #[test]
    fn materialization_preserves_vlag_hlag_order_union_pixels_and_median() {
        let mut vertical_pixels = vec![BACKGROUND; 12 * 12];
        for y in 2..=9 {
            vertical_pixels[y * 12 + 5] = FOREGROUND;
        }
        let vertical_table =
            RunTable::from_pixels(Orientation::Vertical, 12, 12, &vertical_pixels).unwrap();
        let vertical = build_sections_from_id(&vertical_table, JunctionPolicy::All, 10);
        let mut horizontal_pixels = vec![BACKGROUND; 12 * 12];
        horizontal_pixels[6 * 12 + 4] = FOREGROUND;
        horizontal_pixels[6 * 12 + 5] = FOREGROUND;
        let horizontal_table =
            RunTable::from_pixels(Orientation::Horizontal, 12, 12, &horizontal_pixels).unwrap();
        let horizontal = build_sections_from_id(&horizontal_table, JunctionPolicy::All, 20);
        let peak = StaffPeak::new(StaffId::new(1), 3, 8, 4, 6).unwrap();
        let glyph = materialize_serif_glyph(
            peak,
            3,
            PeakBounds {
                x: 0,
                y: 0,
                width: 12,
                height: 12,
            },
            &vertical,
            &horizontal,
            2.0,
            SerifFilamentParameters {
                minimum_core_section_length: 4,
            },
        )
        .unwrap();
        assert_eq!(
            glyph.member_section_ids,
            vec![
                LocatedSectionId {
                    lag: SectionLag::Vertical,
                    id: 10,
                },
                LocatedSectionId {
                    lag: SectionLag::Horizontal,
                    id: 20,
                },
            ]
        );
        assert_eq!((glyph.left, glyph.top), (4, 2));
        assert_eq!(glyph.weight, 9);
        assert_eq!(glyph.median.x1, 5.5);
        assert_eq!((glyph.median.y1, glyph.median.y2), (2.5, 9.5));
    }

    #[test]
    fn materialization_rejects_missing_core_without_partial_glyph() {
        let table =
            RunTable::from_pixels(Orientation::Horizontal, 8, 8, &[BACKGROUND; 64]).unwrap();
        let sections = build_sections_from_id(&table, JunctionPolicy::All, 1);
        let peak = StaffPeak::new(StaffId::new(1), 2, 5, 3, 4).unwrap();
        assert_eq!(
            materialize_serif_glyph(
                peak,
                2,
                PeakBounds {
                    x: 0,
                    y: 0,
                    width: 8,
                    height: 8,
                },
                &sections,
                &[],
                1.0,
                SerifFilamentParameters {
                    minimum_core_section_length: 3,
                },
            ),
            Err(SerifMaterializationError::NoCoreSection)
        );
    }
}
