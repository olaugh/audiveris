// SPDX-License-Identifier: AGPL-3.0-or-later

//! Raster evidence kernel for Java `BeamsBuilder.browseHooks` and
//! `checkHookGlyph`. SIG registration remains a caller-owned seam.

use crate::{
    beam_extension::{BeamExtensionClass, ExtensionBeam},
    beam_structure::{
        BeamBeltSides, BeamImpactParameters, BeamImpacts, BeamItem, BeamRaster, Segment,
        beam_grade, compute_beam_impacts, intersection_at_x_from_slope,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookSide {
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookRejection {
    SmallBaseBeam,
    TooNarrow,
    TooSlim,
    TooThick,
    Overlap,
    Impacts,
    Grade,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HookGlyph {
    pub id: usize,
    pub left: i32,
    pub top: i32,
    pub width: usize,
    pub height: usize,
    pub weight: usize,
    pub centroid_x: f64,
    pub centroid_y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HookParameters {
    pub min_hook_width_low: f64,
    pub minimum_grade: f64,
    /// `min_width_low/high` are the resolved hook widths, not beam widths.
    pub impacts: BeamImpactParameters,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HookEvidence {
    pub beam_id: usize,
    pub side: HookSide,
    pub glyph_id: usize,
    pub item: Option<BeamItem>,
    pub mean_height: f64,
    pub impacts: Option<BeamImpacts>,
    pub grade: Option<f64>,
    pub rejection: Option<HookRejection>,
}

#[derive(Clone, Copy, Debug)]
pub struct HookSearchInput<'a> {
    pub base: ExtensionBeam,
    /// Remaining spots in Java `sortedBeamSpots` full-ordinate order.
    pub spots: &'a [HookGlyph],
    /// Java `rawSystemBeams` order, used by the overlap test.
    pub raw_beams: &'a [ExtensionBeam],
    pub raster: BeamRaster<'a>,
    pub parameters: HookParameters,
}

/// Search one base beam on one vertical side. Evidence is returned in the
/// original spot order (`LinkedHashSet` order in Java).
#[must_use]
pub fn hook_search_evidence(input: HookSearchInput<'_>, side: HookSide) -> Vec<HookEvidence> {
    if input.base.class == BeamExtensionClass::Small {
        return Vec::new();
    }
    let shift = match side {
        HookSide::Top => -1.5 * input.base.height,
        HookSide::Bottom => 1.5 * input.base.height,
    };
    let lookup = BeamItem {
        median: Segment {
            y1: input.base.median.y1 + shift,
            y2: input.base.median.y2 + shift,
            ..input.base.median
        },
        height: input.base.height,
    };
    input
        .spots
        .iter()
        .copied()
        .filter(|glyph| item_intersects_rect(lookup, *glyph))
        .map(|glyph| check_hook(input, side, glyph))
        .collect()
}

fn check_hook(input: HookSearchInput<'_>, side: HookSide, glyph: HookGlyph) -> HookEvidence {
    let mean_height = glyph.weight as f64 / glyph.width as f64;
    if (glyph.width as f64) < input.parameters.min_hook_width_low {
        return rejected(
            input.base.id,
            side,
            glyph.id,
            mean_height,
            HookRejection::TooNarrow,
        );
    }
    if mean_height < input.parameters.impacts.min_height_low {
        return rejected(
            input.base.id,
            side,
            glyph.id,
            mean_height,
            HookRejection::TooSlim,
        );
    }
    if mean_height > input.parameters.impacts.max_height_high {
        return rejected(
            input.base.id,
            side,
            glyph.id,
            mean_height,
            HookRejection::TooThick,
        );
    }
    // Java takes the glyph's *double* centroid and the base beam's slope, and
    // intersects that line with the verticals at the glyph's own box edges. The
    // resulting abscissae are what Java stores -- `p1.getX()`, not `box.x` --
    // because the intersection divides one rounded product by another and is
    // only algebraically the query abscissa.
    let slope = input.base.median.slope();
    let centroid = (glyph.centroid_x, glyph.centroid_y);
    let (x1, y1) = intersection_at_x_from_slope(centroid, slope, f64::from(glyph.left));
    let (x2, y2) =
        intersection_at_x_from_slope(centroid, slope, f64::from(glyph.left) + glyph.width as f64);
    let item = BeamItem {
        median: Segment {
            x1,
            y1: y1 + 0.5,
            x2,
            y2: y2 + 0.5,
        },
        height: input.base.height,
    };
    if overlaps_raw_beam(item, input.raw_beams) {
        return HookEvidence {
            beam_id: input.base.id,
            side,
            glyph_id: glyph.id,
            item: Some(item),
            mean_height,
            impacts: None,
            grade: None,
            rejection: Some(HookRejection::Overlap),
        };
    }
    let impacts = compute_beam_impacts(
        item,
        BeamBeltSides {
            above: true,
            below: true,
                    neutral: false,
        },
        input.raster,
        input.base.distance_impact,
        input.parameters.impacts,
    )
    .ok();
    let grade = impacts.map(beam_grade);
    let rejection = if impacts.is_none() {
        Some(HookRejection::Impacts)
    } else if grade.is_none_or(|grade| grade < input.parameters.minimum_grade) {
        Some(HookRejection::Grade)
    } else {
        None
    };
    HookEvidence {
        beam_id: input.base.id,
        side,
        glyph_id: glyph.id,
        item: Some(item),
        mean_height,
        impacts,
        grade,
        rejection,
    }
}

fn overlaps_raw_beam(item: BeamItem, beams: &[ExtensionBeam]) -> bool {
    let bounds_center = area_bounds_center(item);
    beams
        .iter()
        .copied()
        .filter(|beam| !beam.removed)
        .any(|beam| {
            items_intersect(
                item,
                BeamItem {
                    median: beam.median,
                    height: beam.height,
                },
            ) && item_contains(
                BeamItem {
                    median: beam.median,
                    height: beam.height,
                },
                bounds_center,
            )
        })
}

fn area_bounds_center(item: BeamItem) -> (f64, f64) {
    let half = item.height / 2.0;
    let left = item.median.x1.floor();
    let right = item.median.x2.ceil();
    let top = (item.median.y1.min(item.median.y2) - half).floor();
    let bottom = (item.median.y1.max(item.median.y2) + half).ceil();
    ((left + right) / 2.0, (top + bottom) / 2.0)
}

fn item_contains(item: BeamItem, point: (f64, f64)) -> bool {
    point.0 >= item.median.x1
        && point.0 < item.median.x2
        && point.1 >= item.median.y_at_x(point.0) - (item.height / 2.0)
        && point.1 < item.median.y_at_x(point.0) + (item.height / 2.0)
}

fn item_intersects_rect(item: BeamItem, glyph: HookGlyph) -> bool {
    let left = f64::from(glyph.left);
    let right = left + glyph.width as f64;
    let top = f64::from(glyph.top);
    let bottom = top + glyph.height as f64;
    convex_intersects(
        &item_polygon(item),
        &[(left, top), (right, top), (right, bottom), (left, bottom)],
    )
}

fn items_intersect(one: BeamItem, two: BeamItem) -> bool {
    convex_intersects(&item_polygon(one), &item_polygon(two))
}

fn item_polygon(item: BeamItem) -> [(f64, f64); 4] {
    let half = item.height / 2.0;
    [
        (item.median.x1, item.median.y1 - half),
        (item.median.x2, item.median.y2 - half),
        (item.median.x2, item.median.y2 + half),
        (item.median.x1, item.median.y1 + half),
    ]
}

fn convex_intersects(one: &[(f64, f64)], two: &[(f64, f64)]) -> bool {
    one.iter()
        .zip(one.iter().cycle().skip(1))
        .chain(two.iter().zip(two.iter().cycle().skip(1)))
        .all(|(start, stop)| {
            let axis = (-(stop.1 - start.1), stop.0 - start.0);
            let range = |polygon: &[(f64, f64)]| {
                polygon
                    .iter()
                    .map(|point| (point.0 * axis.0) + (point.1 * axis.1))
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |range, value| {
                        (range.0.min(value), range.1.max(value))
                    })
            };
            let one = range(one);
            let two = range(two);
            one.0 < two.1 && two.0 < one.1
        })
}

fn rejected(
    beam_id: usize,
    side: HookSide,
    glyph_id: usize,
    mean_height: f64,
    rejection: HookRejection,
) -> HookEvidence {
    HookEvidence {
        beam_id,
        side,
        glyph_id,
        item: None,
        mean_height,
        impacts: None,
        grade: None,
        rejection: Some(rejection),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_table::{FOREGROUND, Orientation, RunTable};

    fn beam(id: usize, y: f64) -> ExtensionBeam {
        ExtensionBeam {
            id,
            median: Segment {
                x1: 5.0,
                y1: y,
                x2: 15.0,
                y2: y,
            },
            height: 4.0,
            distance_impact: 1.0,
            class: BeamExtensionClass::Standard,
            glyph_id: Some(id),
            removed: false,
        }
    }

    fn parameters() -> HookParameters {
        HookParameters {
            min_hook_width_low: 3.0,
            minimum_grade: 0.08,
            impacts: BeamImpactParameters {
                belt_margin_dx: 1,
                belt_margin_dy: 1,
                min_core_black_ratio: 0.5,
                max_belt_black_ratio: 2.0,
                min_width_low: 3.0,
                min_width_high: 8.0,
                min_height_low: 2.0,
                typical_height: 4.0,
                max_height_high: 6.0,
            },
        }
    }

    fn glyph(id: usize, left: i32, top: i32, width: usize, height: usize) -> HookGlyph {
        HookGlyph {
            id,
            left,
            top,
            width,
            height,
            weight: width * height,
            centroid_x: f64::from(left) + (width as f64 - 1.0) / 2.0,
            centroid_y: f64::from(top) + (height as f64 - 1.0) / 2.0,
        }
    }

    #[test]
    fn top_lookup_preserves_spot_order_and_builds_half_pixel_median() {
        let table =
            RunTable::from_pixels(Orientation::Horizontal, 25, 25, &[FOREGROUND; 625]).unwrap();
        let spots = [glyph(9, 9, 1, 4, 4), glyph(7, 5, 1, 4, 4)];
        let evidence = hook_search_evidence(
            HookSearchInput {
                base: beam(1, 8.0),
                spots: &spots,
                raw_beams: &[],
                raster: BeamRaster {
                    table: &table,
                    offset_x: 0,
                    offset_y: 0,
                },
                parameters: parameters(),
            },
            HookSide::Top,
        );
        assert_eq!(
            evidence
                .iter()
                .map(|entry| entry.glyph_id)
                .collect::<Vec<_>>(),
            vec![9, 7]
        );
        assert_eq!(evidence[0].item.unwrap().median.y1, 3.0);
        assert!(evidence.iter().all(|entry| entry.rejection.is_none()));
    }

    #[test]
    fn width_then_height_rejection_order_matches_java() {
        let table =
            RunTable::from_pixels(Orientation::Horizontal, 20, 20, &[FOREGROUND; 400]).unwrap();
        let spots = [
            glyph(1, 6, 1, 2, 8),
            HookGlyph {
                weight: 4,
                ..glyph(2, 9, 1, 4, 4)
            },
        ];
        let evidence = hook_search_evidence(
            HookSearchInput {
                base: beam(5, 8.0),
                spots: &spots,
                raw_beams: &[],
                raster: BeamRaster {
                    table: &table,
                    offset_x: 0,
                    offset_y: 0,
                },
                parameters: parameters(),
            },
            HookSide::Top,
        );
        assert_eq!(evidence[0].rejection, Some(HookRejection::TooNarrow));
        assert_eq!(evidence[1].rejection, Some(HookRejection::TooSlim));
    }

    #[test]
    fn center_inside_existing_beam_rejects_overlap_before_impacts() {
        let table =
            RunTable::from_pixels(Orientation::Horizontal, 20, 20, &[FOREGROUND; 400]).unwrap();
        let spots = [glyph(3, 7, 1, 4, 4)];
        let overlap = beam(6, 3.0);
        let evidence = hook_search_evidence(
            HookSearchInput {
                base: beam(5, 8.0),
                spots: &spots,
                raw_beams: &[overlap],
                raster: BeamRaster {
                    table: &table,
                    offset_x: 0,
                    offset_y: 0,
                },
                parameters: parameters(),
            },
            HookSide::Top,
        );
        assert_eq!(evidence[0].rejection, Some(HookRejection::Overlap));
        assert!(evidence[0].impacts.is_none());
    }
}
