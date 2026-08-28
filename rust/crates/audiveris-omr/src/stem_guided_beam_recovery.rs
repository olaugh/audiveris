// SPDX-License-Identifier: AGPL-3.0-or-later

//! Optional recovery of primary beams from two stem endpoints.
//!
//! The Java spot chain can split a long, slanted beam into fragments before
//! STEMS has enough context to join it to both endpoint stems.  This extension
//! uses accepted `VERTICAL_SEED` geometry only to propose a corridor; the
//! source `NO_STAFF` raster remains authoritative.  It is disabled by default
//! so the ordinary entry point stays Java-exact.

use audiveris_image::{
    beam_extension::ExtensionGlyph,
    beam_structure::{
        BeamBeltSides, BeamItem, BeamRaster, Segment, beam_grade, compute_beam_impacts,
    },
};

use crate::{
    beam_inters::{BeamKind, MIN_INTER_GRADE, RawBeam, clamped},
    beam_parameters::{ItemParameters, SheetParameters},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StemGuidedBeamRecoveryConfig {
    pub enabled: bool,
    pub minimum_stem_length_interlines: f64,
    pub maximum_stem_length_interlines: f64,
    pub minimum_span_interlines: f64,
    pub maximum_span_interlines: f64,
    pub maximum_absolute_slope: f64,
    pub maximum_right_clusters: usize,
    pub minimum_core_ratio: f64,
    pub maximum_belt_ratio: f64,
}

impl Default for StemGuidedBeamRecoveryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            minimum_stem_length_interlines: 2.0,
            maximum_stem_length_interlines: 9.0,
            minimum_span_interlines: 2.0,
            maximum_span_interlines: 10.0,
            maximum_absolute_slope: 0.35,
            maximum_right_clusters: 5,
            minimum_core_ratio: 0.70,
            maximum_belt_ratio: 0.65,
        }
    }
}

impl StemGuidedBeamRecoveryConfig {
    #[must_use]
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StemGuidedBeamRecovery {
    pub beam: RawBeam,
    pub left_seed_id: usize,
    pub right_seed_id: usize,
}

#[derive(Clone, Copy)]
struct StemEndpoint {
    id: usize,
    x: f64,
    width: f64,
    top: f64,
    bottom: f64,
}

#[must_use]
pub fn recover_stem_guided_beams(
    seeds: &[ExtensionGlyph],
    occupied: &[RawBeam],
    raster: BeamRaster<'_>,
    item: &ItemParameters,
    sheet: &SheetParameters,
    interline: i32,
    config: StemGuidedBeamRecoveryConfig,
) -> Vec<StemGuidedBeamRecovery> {
    if !config.enabled || interline <= 0 || config.maximum_right_clusters == 0 {
        return Vec::new();
    }
    let minimum_length = config.minimum_stem_length_interlines * f64::from(interline);
    let maximum_length = config.maximum_stem_length_interlines * f64::from(interline);
    let minimum_span = config.minimum_span_interlines * f64::from(interline);
    let maximum_span = config.maximum_span_interlines * f64::from(interline);
    let mut stems = seeds
        .iter()
        .filter_map(|seed| {
            let median = seed.vertical_median?;
            let top = median.y1.min(median.y2);
            let bottom = median.y1.max(median.y2);
            ((minimum_length..=maximum_length).contains(&(bottom - top))).then_some(StemEndpoint {
                id: seed.id,
                x: (median.x1 + median.x2) / 2.0,
                width: seed.width as f64,
                top,
                bottom,
            })
        })
        .collect::<Vec<_>>();
    stems.sort_by(|left, right| left.x.total_cmp(&right.x).then(left.id.cmp(&right.id)));

    let cluster_tolerance = (item.typical_height * 0.5).max(1.0);
    let mut clusters = Vec::<Vec<StemEndpoint>>::new();
    for stem in stems {
        if clusters
            .last()
            .is_some_and(|cluster| (cluster[0].x - stem.x).abs() <= cluster_tolerance)
        {
            clusters.last_mut().expect("cluster exists").push(stem);
        } else {
            clusters.push(vec![stem]);
        }
    }

    let mut accepted = occupied.to_vec();
    let mut recovered = Vec::new();
    for left_index in 0..clusters.len() {
        let right_stop = (left_index + 1 + config.maximum_right_clusters).min(clusters.len());
        for side in [EndpointSide::Top, EndpointSide::Bottom] {
            for right_index in (left_index + 1)..right_stop {
                let span = clusters[right_index][0].x - clusters[left_index][0].x;
                if span > maximum_span {
                    break;
                }
                if span < minimum_span {
                    continue;
                }
                let best = clusters[left_index]
                    .iter()
                    .flat_map(|left| {
                        clusters[right_index].iter().filter_map(move |right| {
                            strongest_between_endpoints(
                                *left, *right, side, raster, item, sheet, config,
                            )
                        })
                    })
                    .max_by(|left, right| left.beam.grade.total_cmp(&right.beam.grade));
                let Some(candidate) = best else {
                    continue;
                };
                if occupied_ratio(candidate.beam.item, &accepted) >= 0.70 {
                    break;
                }
                accepted.push(candidate.beam);
                recovered.push(candidate);
                // Consecutive stem pairs form a deterministic chain. Once a
                // supported right endpoint is found, do not also emit every
                // longer chord through the same ink.
                break;
            }
        }
    }
    recovered
}

#[derive(Clone, Copy)]
enum EndpointSide {
    Top,
    Bottom,
}

fn strongest_between_endpoints(
    left: StemEndpoint,
    right: StemEndpoint,
    side: EndpointSide,
    raster: BeamRaster<'_>,
    item: &ItemParameters,
    sheet: &SheetParameters,
    config: StemGuidedBeamRecoveryConfig,
) -> Option<StemGuidedBeamRecovery> {
    let x1 = left.x + 0.5 * left.width;
    let x2 = right.x - 0.5 * right.width;
    if x2 - x1 < item.min_beam_width_low {
        return None;
    }
    let left_endpoint = match side {
        EndpointSide::Top => left.top,
        EndpointSide::Bottom => left.bottom,
    };
    let right_endpoint = match side {
        EndpointSide::Top => right.top,
        EndpointSide::Bottom => right.bottom,
    };
    let slope = (right_endpoint - left_endpoint) / (right.x - left.x);
    if !slope.is_finite() || slope.abs() > sheet.max_beam_slope.min(config.maximum_absolute_slope) {
        return None;
    }
    let maximum_offset = item.typical_height.ceil() as i32;
    let mut best = None::<RawBeam>;
    for offset in -maximum_offset..=maximum_offset {
        let offset = f64::from(offset);
        let median = Segment {
            x1,
            y1: left_endpoint + offset + slope * (x1 - left.x),
            x2,
            y2: left_endpoint + offset + slope * (x2 - left.x),
        };
        let candidate_item = BeamItem {
            median,
            height: item.typical_height,
        };
        let mut parameters = item.impacts(sheet);
        parameters.min_core_black_ratio = 0.0;
        parameters.max_belt_black_ratio = 1.0;
        let Ok(impacts) = compute_beam_impacts(
            candidate_item,
            BeamBeltSides {
                above: true,
                below: true,
            },
            raster,
            1.0,
            parameters,
        ) else {
            continue;
        };
        if impacts.raster.core_ratio < config.minimum_core_ratio
            || impacts.raster.belt_ratio > config.maximum_belt_ratio
        {
            continue;
        }
        let impacts = clamped(impacts);
        let beam = RawBeam {
            kind: BeamKind::Beam,
            item: candidate_item,
            impacts,
            grade: beam_grade(impacts).max(MIN_INTER_GRADE),
        };
        if best.is_none_or(|current| beam.grade > current.grade) {
            best = Some(beam);
        }
    }
    best.map(|beam| StemGuidedBeamRecovery {
        beam,
        left_seed_id: left.id,
        right_seed_id: right.id,
    })
}

fn occupied_ratio(candidate: BeamItem, occupied: &[RawBeam]) -> f64 {
    let left = candidate.median.x1.min(candidate.median.x2);
    let right = candidate.median.x1.max(candidate.median.x2);
    let width = right - left;
    if width <= 0.0 {
        return 1.0;
    }
    occupied
        .iter()
        .filter(|beam| {
            let middle = (left + right) / 2.0;
            (beam.item.median.y_at_x(middle) - candidate.median.y_at_x(middle)).abs()
                <= candidate.height.max(beam.item.height)
        })
        .map(|beam| {
            let other_left = beam.item.median.x1.min(beam.item.median.x2);
            let other_right = beam.item.median.x1.max(beam.item.median.x2);
            (right.min(other_right) - left.max(other_left)).max(0.0) / width
        })
        .fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiveris_image::run_table::{Orientation, RunTable};

    #[test]
    fn recovers_a_thick_band_but_rejects_a_hairpin_stroke() {
        let mut pixels = vec![255; 100 * 80];
        for x in 20..=70 {
            let y = 20 + (x - 20) / 10;
            for dy in 0..7 {
                pixels[(y + dy) * 100 + x] = 0;
            }
        }
        let table = RunTable::from_pixels(Orientation::Vertical, 100, 80, &pixels).unwrap();
        let seeds = vec![seed(1, 20.0, 20.0, 60.0), seed(2, 70.0, 25.0, 65.0)];
        let item = ItemParameters::new(10, 7.0, false);
        let sheet = SheetParameters::new(10);
        let found = recover_stem_guided_beams(
            &seeds,
            &[],
            BeamRaster {
                table: &table,
                offset_x: 0,
                offset_y: 0,
            },
            &item,
            &sheet,
            10,
            StemGuidedBeamRecoveryConfig::enabled(),
        );
        assert_eq!(found.len(), 1);

        pixels.fill(255);
        for x in 20..=70 {
            pixels[(20 + (x - 20) / 10) * 100 + x] = 0;
        }
        let table = RunTable::from_pixels(Orientation::Vertical, 100, 80, &pixels).unwrap();
        assert!(
            recover_stem_guided_beams(
                &seeds,
                &[],
                BeamRaster {
                    table: &table,
                    offset_x: 0,
                    offset_y: 0
                },
                &item,
                &sheet,
                10,
                StemGuidedBeamRecoveryConfig::enabled(),
            )
            .is_empty()
        );
    }

    fn seed(id: usize, x: f64, top: f64, bottom: f64) -> ExtensionGlyph {
        ExtensionGlyph {
            id,
            left: x as i32 - 1,
            top: top as i32,
            width: 3,
            height: (bottom - top) as usize + 1,
            vertical_median: Some(Segment {
                x1: x,
                y1: top,
                x2: x,
                y2: bottom,
            }),
        }
    }
}
