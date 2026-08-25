// SPDX-License-Identifier: AGPL-3.0-or-later

//! Optional recovery of beam hooks from stem-local raster evidence.
//!
//! Java BEAMS can only build an ordinary hook from a surviving `BEAM_SPOT`.
//! Real scans sometimes preserve a short secondary beam beside a credible
//! stem without preserving a usable spot component.  This module is a narrow
//! extension seam for that case: it proposes a hook only where an accepted
//! vertical seed reaches both an existing full beam and the expected adjacent
//! beam level, then grades source-raster ink with the ordinary hook impacts.
//!
//! The pass is deliberately independent of stage orchestration.  It is
//! disabled by default, never mutates its inputs, and returns explicit source
//! provenance with every recovered hook.

use audiveris_image::{
    beam_extension::ExtensionGlyph,
    beam_structure::{
        BeamBeltSides, BeamItem, BeamRaster, Segment, beam_grade, compute_beam_impacts,
    },
};

use crate::{
    beam_inters::{BeamKind, MIN_INTER_GRADE, RawBeam, beam_bounds, clamped},
    beam_parameters::{ItemParameters, SheetParameters},
};

/// Tunable policy for the non-Java recovery pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StemGuidedHookRecoveryConfig {
    /// Master switch. The default preserves exact Java-compatible BEAMS.
    pub enabled: bool,
    /// Expected centre-to-centre separation from the already detected beam.
    pub beam_spacing_heights: f64,
    /// Half-height of the local vertical search corridor around that estimate.
    pub maximum_vertical_search_heights: f64,
    /// Maximum local slope departure from the parent beam's global median.
    pub maximum_slope_delta: f64,
    /// Number of slope samples on each side of the parent slope.
    pub slope_steps_each_side: i32,
    /// Maximum horizontal anchor displacement as a fraction of seed width.
    /// A hook usually begins at a stem edge rather than its centre line.
    pub maximum_anchor_seed_width_ratio: f64,
    /// Number of horizontal anchor samples on each side of the seed centre.
    pub anchor_steps_each_side: i32,
    /// Additional evidence gate beyond the ordinary hook impact threshold.
    pub minimum_core_ratio: f64,
    /// A two-stem fragment can tolerate more broken core ink because both
    /// endpoints independently validate its placement.
    pub minimum_pair_core_ratio: f64,
    /// Additional evidence gate beyond the ordinary hook impact threshold.
    pub maximum_belt_ratio: f64,
    /// Belt allowance for a span whose two endpoints are validated by stems.
    /// The stems themselves legitimately contribute foreground to the belt.
    pub maximum_pair_belt_ratio: f64,
    /// Minimum accepted intrinsic grade.
    pub minimum_grade: f64,
    /// Reject candidates whose horizontal span is already covered this much
    /// by accepted geometry in the same beam band.
    pub maximum_occupied_ratio: f64,
    /// Maximum distance between two supporting stems, in ordinary hook widths.
    /// This bounds recovery of a missing secondary beam between two stems.
    pub maximum_pair_span_hook_widths: f64,
    /// Maximum empty horizontal gap bridged between two independently
    /// recovered fragments that share a parent beam level.
    pub maximum_bridge_gap_hook_widths: f64,
    /// Emit aggregate rejection counters for corpus tuning.
    pub debug: bool,
}

impl Default for StemGuidedHookRecoveryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            beam_spacing_heights: 1.5,
            maximum_vertical_search_heights: 0.75,
            maximum_slope_delta: 0.25,
            slope_steps_each_side: 2,
            maximum_anchor_seed_width_ratio: 0.5,
            anchor_steps_each_side: 1,
            minimum_core_ratio: 0.70,
            minimum_pair_core_ratio: 0.55,
            maximum_belt_ratio: 0.35,
            maximum_pair_belt_ratio: 1.0,
            minimum_grade: MIN_INTER_GRADE,
            maximum_occupied_ratio: 0.65,
            maximum_pair_span_hook_widths: 4.0,
            maximum_bridge_gap_hook_widths: 2.0,
            debug: false,
        }
    }
}

impl StemGuidedHookRecoveryConfig {
    /// Opt into the conservative production policy without exposing its
    /// thresholds through stage orchestration.
    #[must_use]
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
}

/// Which adjacent beam level supplied the recovered raster evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StemGuidedHookSide {
    Above,
    Below,
}

/// Which way the short beam projects from its stem anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StemGuidedHookDirection {
    Left,
    Right,
}

/// A recovered hook plus enough identity to audit or later remove it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StemGuidedHookRecovery {
    pub hook: RawBeam,
    pub base_beam_ordinal: usize,
    pub stem_seed_id: usize,
    /// Present when raster evidence spans two stems rather than projecting as
    /// a one-ended hook from `stem_seed_id`.
    pub paired_stem_seed_id: Option<usize>,
    pub side: StemGuidedHookSide,
    pub direction: StemGuidedHookDirection,
}

/// Immutable inputs to one system-local recovery pass.
#[derive(Clone, Copy)]
pub struct StemGuidedHookRecoveryInput<'a> {
    /// Full beams after ordinary extension. Hooks are not valid parents.
    pub beams: &'a [RawBeam],
    /// Ordinary beams and hooks already accepted in this system. Recovery
    /// cannot duplicate any of them.
    pub occupied: &'a [RawBeam],
    /// Accepted/free `VERTICAL_SEED` glyphs for this system.
    pub stem_seeds: &'a [ExtensionGlyph],
    pub raster: BeamRaster<'a>,
    pub item_parameters: &'a ItemParameters,
    pub sheet: &'a SheetParameters,
}

#[derive(Default)]
struct HookSearchStats {
    lengths: usize,
    occupied: usize,
    impacts_failed: usize,
    width_rejected: usize,
    height_rejected: usize,
    impact_core_rejected: usize,
    impact_belt_rejected: usize,
    core_rejected: usize,
    belt_rejected: usize,
    grade_rejected: usize,
    accepted_lengths: usize,
    pair_searches: usize,
    accepted_pairs: usize,
    pair_core_at_least_055: usize,
    pair_core_at_least_060: usize,
    pair_core_at_least_065: usize,
    maximum_pair_core_ratio: f64,
    bridge_searches: usize,
    accepted_bridges: usize,
}

#[derive(Clone, Copy)]
struct ReachingStem {
    id: usize,
    x: f64,
    width: f64,
    top: f64,
    bottom: f64,
}

/// Search for stem-anchored secondary beam ink that the spot chain missed.
#[must_use]
pub fn recover_stem_guided_hooks(
    input: StemGuidedHookRecoveryInput<'_>,
    config: StemGuidedHookRecoveryConfig,
) -> Vec<StemGuidedHookRecovery> {
    if !config.enabled
        || !config.beam_spacing_heights.is_finite()
        || config.beam_spacing_heights <= 0.0
        || !config.maximum_vertical_search_heights.is_finite()
        || config.maximum_vertical_search_heights < 0.0
        || !config.maximum_slope_delta.is_finite()
        || config.maximum_slope_delta < 0.0
        || config.slope_steps_each_side < 0
        || !config.maximum_anchor_seed_width_ratio.is_finite()
        || config.maximum_anchor_seed_width_ratio < 0.0
        || config.anchor_steps_each_side < 0
        || !config.maximum_occupied_ratio.is_finite()
        || !(0.0..=1.0).contains(&config.maximum_occupied_ratio)
        || !config.maximum_pair_span_hook_widths.is_finite()
        || config.maximum_pair_span_hook_widths <= 0.0
        || !config.maximum_bridge_gap_hook_widths.is_finite()
        || config.maximum_bridge_gap_hook_widths <= 0.0
    {
        return Vec::new();
    }

    let mut occupied = input.occupied.to_vec();
    let mut recovered = Vec::new();
    let mut base_beams = 0usize;
    let mut seeds_in_x_range = 0usize;
    let mut seeds_reaching_base = 0usize;
    let mut adjacent_levels_reached = 0usize;
    let mut directional_searches = 0usize;
    let mut search_stats = HookSearchStats::default();

    // Recover complete secondary fragments first. Otherwise two individually
    // valid one-ended hooks can consume the same ink and prevent the stronger
    // two-stem interpretation from being considered.
    let paired =
        recover_stem_paired_secondary_beams(input, config, &mut occupied, &mut search_stats);
    recovered.extend(paired);

    for (base_beam_ordinal, base) in input.beams.iter().copied().enumerate() {
        if base.kind != BeamKind::Beam {
            continue;
        }
        base_beams += 1;
        for seed in input.stem_seeds {
            let Some(stem) = seed.vertical_median else {
                continue;
            };
            let stem_x = (stem.x1 + stem.x2) / 2.0;
            let base_left = base.item.median.x1.min(base.item.median.x2);
            let base_right = base.item.median.x1.max(base.item.median.x2);
            if stem_x < base_left - f64::from(input.sheet.max_stem_beam_gap_x)
                || stem_x > base_right + f64::from(input.sheet.max_stem_beam_gap_x)
            {
                continue;
            }
            seeds_in_x_range += 1;
            let base_y = base.item.median.y_at_x(stem_x);
            let stem_top = stem.y1.min(stem.y2);
            let stem_bottom = stem.y1.max(stem.y2);
            // `maxStemBeamGapY` is deliberately broad enough for ordinary
            // extension (0.8 interline). Here it would let a short seed
            // "reach" both adjacent levels without spanning either. Require
            // contact with the beam band, plus one pixel for edge erasure.
            let y_tolerance = (base.item.height / 2.0) + 1.0;
            if !reaches(stem_top, stem_bottom, base_y, y_tolerance) {
                continue;
            }
            seeds_reaching_base += 1;

            for side in [StemGuidedHookSide::Above, StemGuidedHookSide::Below] {
                let sign = match side {
                    StemGuidedHookSide::Above => -1.0,
                    StemGuidedHookSide::Below => 1.0,
                };
                let target_y = base_y + sign * config.beam_spacing_heights * base.item.height;
                if !reaches(stem_top, stem_bottom, target_y, y_tolerance) {
                    continue;
                }
                adjacent_levels_reached += 1;

                for direction in [
                    StemGuidedHookDirection::Left,
                    StemGuidedHookDirection::Right,
                ] {
                    directional_searches += 1;
                    let Some(hook) = strongest_hook_from_anchor(
                        base,
                        stem_x,
                        seed.width as f64,
                        stem_top,
                        stem_bottom,
                        y_tolerance,
                        target_y,
                        direction,
                        &occupied,
                        input.raster,
                        input.item_parameters,
                        input.sheet,
                        config,
                        &mut search_stats,
                    ) else {
                        continue;
                    };
                    occupied.push(hook);
                    recovered.push(StemGuidedHookRecovery {
                        hook,
                        base_beam_ordinal,
                        stem_seed_id: seed.id,
                        paired_stem_seed_id: None,
                        side,
                        direction,
                    });
                }
            }
        }
    }
    let bridges = recover_bridges_between_fragments(&recovered, input, config, &mut search_stats);
    recovered.extend(bridges);
    if config.debug {
        eprintln!(
            "stem_guided_hook\tbase_beams={base_beams}\tstem_seeds={}\tx_range={seeds_in_x_range}\treach_base={seeds_reaching_base}\treach_adjacent={adjacent_levels_reached}\tsearches={directional_searches}\tlengths={}\toccupied={}\timpacts_failed={}\twidth_rejected={}\theight_rejected={}\timpact_core_rejected={}\timpact_belt_rejected={}\tcore_rejected={}\tbelt_rejected={}\tgrade_rejected={}\taccepted_lengths={}\tpair_searches={}\tpair_core_055={}\tpair_core_060={}\tpair_core_065={}\tpair_max_core={:.3}\taccepted_pairs={}\tbridge_searches={}\taccepted_bridges={}\trecovered={}",
            input.stem_seeds.len(),
            search_stats.lengths,
            search_stats.occupied,
            search_stats.impacts_failed,
            search_stats.width_rejected,
            search_stats.height_rejected,
            search_stats.impact_core_rejected,
            search_stats.impact_belt_rejected,
            search_stats.core_rejected,
            search_stats.belt_rejected,
            search_stats.grade_rejected,
            search_stats.accepted_lengths,
            search_stats.pair_searches,
            search_stats.pair_core_at_least_055,
            search_stats.pair_core_at_least_060,
            search_stats.pair_core_at_least_065,
            search_stats.maximum_pair_core_ratio,
            search_stats.accepted_pairs,
            search_stats.bridge_searches,
            search_stats.accepted_bridges,
            recovered.len(),
        );
    }
    recovered
}

fn recover_bridges_between_fragments(
    recoveries: &[StemGuidedHookRecovery],
    input: StemGuidedHookRecoveryInput<'_>,
    config: StemGuidedHookRecoveryConfig,
    stats: &mut HookSearchStats,
) -> Vec<StemGuidedHookRecovery> {
    let mut bridges = Vec::new();
    let maximum_gap = config.maximum_bridge_gap_hook_widths * input.item_parameters.max_hook_width;
    for base_beam_ordinal in 0..input.beams.len() {
        for side in [StemGuidedHookSide::Above, StemGuidedHookSide::Below] {
            let mut fragments = recoveries
                .iter()
                .filter(|recovery| {
                    recovery.base_beam_ordinal == base_beam_ordinal
                        && recovery.side == side
                        && recovery.paired_stem_seed_id.is_none()
                        && recovery.hook.kind == BeamKind::Hook
                })
                .copied()
                .collect::<Vec<_>>();
            fragments.sort_by(|left, right| {
                left.hook
                    .item
                    .median
                    .x1
                    .min(left.hook.item.median.x2)
                    .total_cmp(&right.hook.item.median.x1.min(right.hook.item.median.x2))
            });
            for pair in fragments.windows(2) {
                let [left, right] = [pair[0], pair[1]];
                let left_right = left.hook.item.median.x1.max(left.hook.item.median.x2);
                let right_left = right.hook.item.median.x1.min(right.hook.item.median.x2);
                let gap = right_left - left_right;
                if gap <= input.item_parameters.max_item_x_gap || gap > maximum_gap {
                    continue;
                }
                stats.bridge_searches += 1;
                let Some(beam) = strongest_bridge_between_fragments(
                    left.hook,
                    right.hook,
                    input.raster,
                    input.item_parameters,
                    input.sheet,
                    config,
                ) else {
                    continue;
                };
                stats.accepted_bridges += 1;
                bridges.push(StemGuidedHookRecovery {
                    hook: beam,
                    base_beam_ordinal,
                    stem_seed_id: left.stem_seed_id,
                    paired_stem_seed_id: Some(right.stem_seed_id),
                    side,
                    direction: StemGuidedHookDirection::Right,
                });
            }
        }
    }
    bridges
}

fn strongest_bridge_between_fragments(
    left: RawBeam,
    right: RawBeam,
    raster: BeamRaster<'_>,
    item_parameters: &ItemParameters,
    sheet: &SheetParameters,
    config: StemGuidedHookRecoveryConfig,
) -> Option<RawBeam> {
    let x1 = left.item.median.x1.min(left.item.median.x2);
    let x2 = right.item.median.x1.max(right.item.median.x2);
    if x2 - x1 < item_parameters.min_beam_width_low {
        return None;
    }
    let left_y = left.item.median.y_at_x(x1);
    let right_y = right.item.median.y_at_x(x2);
    let reference_slope = (right_y - left_y) / (x2 - x1);
    if !reference_slope.is_finite() || reference_slope.abs() > sheet.max_beam_slope {
        return None;
    }
    let height = left.item.height.max(right.item.height);
    let mut best = None::<RawBeam>;
    let maximum_vertical_search = (height * 0.25).ceil() as i32;
    for vertical_offset in -maximum_vertical_search..=maximum_vertical_search {
        for slope_step in -config.slope_steps_each_side..=config.slope_steps_each_side {
            let slope_delta = if config.slope_steps_each_side == 0 {
                0.0
            } else {
                0.10 * f64::from(slope_step) / f64::from(config.slope_steps_each_side)
            };
            let candidate_slope = reference_slope + slope_delta;
            let median = Segment {
                x1,
                y1: left_y + f64::from(vertical_offset),
                x2,
                y2: left_y + f64::from(vertical_offset) + candidate_slope * (x2 - x1),
            };
            let item = BeamItem { median, height };
            let mut impact_parameters = item_parameters.impacts(sheet);
            impact_parameters.min_core_black_ratio = 0.0;
            impact_parameters.max_belt_black_ratio = 1.0;
            let Ok(impacts) = compute_beam_impacts(
                item,
                BeamBeltSides {
                    above: true,
                    below: true,
                },
                raster,
                left.impacts.distance.max(right.impacts.distance),
                impact_parameters,
            ) else {
                continue;
            };
            if impacts.raster.core_ratio < config.minimum_pair_core_ratio {
                continue;
            }
            let impacts = clamped(impacts);
            let grade = beam_grade(impacts).max(config.minimum_grade);
            let candidate = RawBeam {
                kind: BeamKind::Beam,
                item,
                impacts,
                grade,
            };
            if best.is_none_or(|current| candidate.grade > current.grade) {
                best = Some(candidate);
            }
        }
    }
    best
}

fn recover_stem_paired_secondary_beams(
    input: StemGuidedHookRecoveryInput<'_>,
    config: StemGuidedHookRecoveryConfig,
    occupied: &mut Vec<RawBeam>,
    stats: &mut HookSearchStats,
) -> Vec<StemGuidedHookRecovery> {
    let mut recovered = Vec::new();
    let maximum_span = config.maximum_pair_span_hook_widths * input.item_parameters.max_hook_width;
    for (base_beam_ordinal, base) in input.beams.iter().copied().enumerate() {
        if base.kind != BeamKind::Beam {
            continue;
        }
        let base_left = base.item.median.x1.min(base.item.median.x2);
        let base_right = base.item.median.x1.max(base.item.median.x2);
        let y_tolerance = (base.item.height / 2.0) + 1.0;
        for side in [StemGuidedHookSide::Above, StemGuidedHookSide::Below] {
            let sign = match side {
                StemGuidedHookSide::Above => -1.0,
                StemGuidedHookSide::Below => 1.0,
            };
            let mut stems = input
                .stem_seeds
                .iter()
                .filter_map(|seed| {
                    let stem = seed.vertical_median?;
                    let x = (stem.x1 + stem.x2) / 2.0;
                    if x < base_left - f64::from(input.sheet.max_stem_beam_gap_x)
                        || x > base_right + f64::from(input.sheet.max_stem_beam_gap_x)
                    {
                        return None;
                    }
                    let top = stem.y1.min(stem.y2);
                    let bottom = stem.y1.max(stem.y2);
                    let base_y = base.item.median.y_at_x(x);
                    let target_y = base_y + sign * config.beam_spacing_heights * base.item.height;
                    (reaches(top, bottom, base_y, y_tolerance)
                        && reaches(top, bottom, target_y, y_tolerance))
                    .then_some(ReachingStem {
                        id: seed.id,
                        x,
                        width: seed.width as f64,
                        top,
                        bottom,
                    })
                })
                .collect::<Vec<_>>();
            stems.sort_by(|left, right| left.x.total_cmp(&right.x));
            for pair in stems.windows(2) {
                let [left, right] = [pair[0], pair[1]];
                let stem_distance = right.x - left.x;
                if stem_distance < input.item_parameters.min_beam_width_low
                    || stem_distance > maximum_span
                {
                    continue;
                }
                stats.pair_searches += 1;
                let Some(beam) = strongest_beam_between_stems(
                    base,
                    left,
                    right,
                    sign,
                    y_tolerance,
                    occupied,
                    input.raster,
                    input.item_parameters,
                    input.sheet,
                    config,
                    stats,
                ) else {
                    continue;
                };
                occupied.push(beam);
                stats.accepted_pairs += 1;
                recovered.push(StemGuidedHookRecovery {
                    hook: beam,
                    base_beam_ordinal,
                    stem_seed_id: left.id,
                    paired_stem_seed_id: Some(right.id),
                    side,
                    direction: StemGuidedHookDirection::Right,
                });
            }
        }
    }
    recovered
}

#[allow(clippy::too_many_arguments)]
fn strongest_beam_between_stems(
    base: RawBeam,
    left: ReachingStem,
    right: ReachingStem,
    side_sign: f64,
    stem_y_tolerance: f64,
    occupied: &[RawBeam],
    raster: BeamRaster<'_>,
    item_parameters: &ItemParameters,
    sheet: &SheetParameters,
    config: StemGuidedHookRecoveryConfig,
    stats: &mut HookSearchStats,
) -> Option<RawBeam> {
    let x1 = left.x + left.width * config.maximum_anchor_seed_width_ratio;
    let x2 = right.x - right.width * config.maximum_anchor_seed_width_ratio;
    if x2 - x1 < item_parameters.min_beam_width_low {
        return None;
    }
    let parent_slope = base.item.median.slope();
    let left_target = base.item.median.y_at_x(left.x)
        + side_sign * config.beam_spacing_heights * base.item.height;
    let maximum_vertical_search =
        (config.maximum_vertical_search_heights * base.item.height).ceil() as i32;
    let mut best = None::<RawBeam>;
    for vertical_offset in -maximum_vertical_search..=maximum_vertical_search {
        let left_y = left_target + f64::from(vertical_offset);
        for slope_step in -config.slope_steps_each_side..=config.slope_steps_each_side {
            let slope_delta = if config.slope_steps_each_side == 0 {
                0.0
            } else {
                config.maximum_slope_delta * f64::from(slope_step)
                    / f64::from(config.slope_steps_each_side)
            };
            let candidate_slope = parent_slope + slope_delta;
            let median = Segment {
                x1,
                y1: left_y + candidate_slope * (x1 - left.x),
                x2,
                y2: left_y + candidate_slope * (x2 - left.x),
            };
            if !reaches(left.top, left.bottom, median.y1, stem_y_tolerance)
                || !reaches(right.top, right.bottom, median.y2, stem_y_tolerance)
            {
                continue;
            }
            let item = BeamItem {
                median,
                height: base.item.height,
            };
            if occupied_ratio(item, occupied) >= config.maximum_occupied_ratio {
                stats.occupied += 1;
                continue;
            }
            // Ask the kernel for raster evidence even when it misses the
            // ordinary early core/belt gates; this pass applies its own
            // (slightly stricter) gates below and needs the ratios to compare
            // nearby stem-pair hypotheses.
            let mut pair_impact_parameters = item_parameters.impacts(sheet);
            pair_impact_parameters.min_core_black_ratio = 0.0;
            pair_impact_parameters.max_belt_black_ratio = 1.0;
            let impacts = match compute_beam_impacts(
                item,
                BeamBeltSides {
                    above: true,
                    below: true,
                },
                raster,
                base.impacts.distance,
                pair_impact_parameters,
            ) {
                Ok(impacts) => impacts,
                Err(_) => {
                    stats.impacts_failed += 1;
                    continue;
                }
            };
            stats.maximum_pair_core_ratio =
                stats.maximum_pair_core_ratio.max(impacts.raster.core_ratio);
            stats.pair_core_at_least_055 += usize::from(impacts.raster.core_ratio >= 0.55);
            stats.pair_core_at_least_060 += usize::from(impacts.raster.core_ratio >= 0.60);
            stats.pair_core_at_least_065 += usize::from(impacts.raster.core_ratio >= 0.65);
            if impacts.raster.core_ratio < config.minimum_pair_core_ratio {
                stats.core_rejected += 1;
                continue;
            }
            if impacts.raster.belt_ratio > config.maximum_pair_belt_ratio {
                stats.belt_rejected += 1;
                continue;
            }
            let impacts = clamped(impacts);
            // Ordinary intrinsic grading has no term for the two accepted
            // stems that make this hypothesis unusually specific. Preserve
            // its raster-derived grade when it is sufficient; otherwise use
            // the ordinary inter threshold as the contextual stem-support
            // floor. The original impacts remain published for audit.
            let grade = beam_grade(impacts).max(config.minimum_grade);
            let candidate = RawBeam {
                kind: BeamKind::Beam,
                item,
                impacts,
                grade,
            };
            if best.is_none_or(|current| candidate.grade > current.grade) {
                best = Some(candidate);
            }
        }
    }
    best
}

fn reaches(top: f64, bottom: f64, y: f64, tolerance: f64) -> bool {
    y >= top - tolerance && y <= bottom + tolerance
}

#[allow(clippy::too_many_arguments)]
fn strongest_hook_from_anchor(
    base: RawBeam,
    stem_x: f64,
    stem_width: f64,
    stem_top: f64,
    stem_bottom: f64,
    stem_y_tolerance: f64,
    target_y: f64,
    direction: StemGuidedHookDirection,
    occupied: &[RawBeam],
    raster: BeamRaster<'_>,
    item_parameters: &ItemParameters,
    sheet: &SheetParameters,
    config: StemGuidedHookRecoveryConfig,
    stats: &mut HookSearchStats,
) -> Option<RawBeam> {
    let min_length = item_parameters.min_hook_width_low.ceil().max(1.0) as i32;
    let max_length = item_parameters.max_hook_width.floor() as i32;
    let slope = base.item.median.slope();
    let mut best = None::<RawBeam>;
    let maximum_vertical_search =
        (config.maximum_vertical_search_heights * base.item.height).ceil() as i32;
    for vertical_offset in -maximum_vertical_search..=maximum_vertical_search {
        let anchor_y = target_y + f64::from(vertical_offset);
        if !reaches(stem_top, stem_bottom, anchor_y, stem_y_tolerance) {
            continue;
        }
        for slope_step in -config.slope_steps_each_side..=config.slope_steps_each_side {
            let slope_delta = if config.slope_steps_each_side == 0 {
                0.0
            } else {
                config.maximum_slope_delta * f64::from(slope_step)
                    / f64::from(config.slope_steps_each_side)
            };
            let candidate_slope = slope + slope_delta;
            for anchor_step in -config.anchor_steps_each_side..=config.anchor_steps_each_side {
                let anchor_offset = if config.anchor_steps_each_side == 0 {
                    0.0
                } else {
                    config.maximum_anchor_seed_width_ratio * stem_width * f64::from(anchor_step)
                        / f64::from(config.anchor_steps_each_side)
                };
                let candidate_anchor_x = stem_x + anchor_offset;
                for length in min_length..=max_length {
                    stats.lengths += 1;
                    let dx = f64::from(length - 1);
                    let (x1, x2) = match direction {
                        StemGuidedHookDirection::Left => {
                            (candidate_anchor_x - dx, candidate_anchor_x)
                        }
                        StemGuidedHookDirection::Right => {
                            (candidate_anchor_x, candidate_anchor_x + dx)
                        }
                    };
                    let median = Segment {
                        x1,
                        y1: anchor_y + candidate_slope * (x1 - stem_x),
                        x2,
                        y2: anchor_y + candidate_slope * (x2 - stem_x),
                    };
                    let item = BeamItem {
                        median,
                        height: base.item.height,
                    };
                    if occupied_ratio(item, occupied) >= config.maximum_occupied_ratio {
                        stats.occupied += 1;
                        continue;
                    }
                    let impacts = match compute_beam_impacts(
                        item,
                        BeamBeltSides {
                            above: true,
                            below: true,
                        },
                        raster,
                        base.impacts.distance,
                        item_parameters.hook_impacts(sheet),
                    ) {
                        Ok(impacts) => impacts,
                        Err(rejection) => {
                            use audiveris_image::beam_structure::BeamImpactRejection;
                            stats.impacts_failed += 1;
                            match rejection {
                                BeamImpactRejection::Width => stats.width_rejected += 1,
                                BeamImpactRejection::HeightBelow
                                | BeamImpactRejection::HeightAbove => {
                                    stats.height_rejected += 1;
                                }
                                BeamImpactRejection::CoreRatio => stats.impact_core_rejected += 1,
                                BeamImpactRejection::BeltRatio => stats.impact_belt_rejected += 1,
                            }
                            continue;
                        }
                    };
                    if impacts.raster.core_ratio < config.minimum_core_ratio {
                        stats.core_rejected += 1;
                        continue;
                    }
                    if impacts.raster.belt_ratio > config.maximum_belt_ratio {
                        stats.belt_rejected += 1;
                        continue;
                    }
                    let impacts = clamped(impacts);
                    let grade = beam_grade(impacts);
                    if grade < config.minimum_grade {
                        stats.grade_rejected += 1;
                        continue;
                    }
                    stats.accepted_lengths += 1;
                    let candidate = RawBeam {
                        kind: BeamKind::Hook,
                        item,
                        impacts,
                        grade,
                    };
                    if best.is_none_or(|current| candidate.grade > current.grade) {
                        best = Some(candidate);
                    }
                }
            }
        }
    }
    best
}

fn occupied_ratio(candidate: BeamItem, occupied: &[RawBeam]) -> f64 {
    let candidate_left = candidate.median.x1.min(candidate.median.x2);
    let candidate_right = candidate.median.x1.max(candidate.median.x2);
    let candidate_width = (candidate_right - candidate_left).max(1.0);
    let mut intervals = occupied
        .iter()
        .map(|beam| beam.item)
        .filter(|other| same_beam_band(candidate, *other))
        .filter_map(|other| {
            let left = candidate_left.max(other.median.x1.min(other.median.x2));
            let right = candidate_right.min(other.median.x1.max(other.median.x2));
            (right > left).then_some((left, right))
        })
        .collect::<Vec<_>>();
    intervals.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut covered = 0.0;
    let mut current = None::<(f64, f64)>;
    for interval in intervals {
        match current {
            Some((left, right)) if interval.0 <= right => {
                current = Some((left, right.max(interval.1)));
            }
            Some((left, right)) => {
                covered += right - left;
                current = Some(interval);
            }
            None => current = Some(interval),
        }
    }
    if let Some((left, right)) = current {
        covered += right - left;
    }
    covered / candidate_width
}

fn same_beam_band(left: BeamItem, right: BeamItem) -> bool {
    let left_bounds = beam_bounds(left);
    let right_bounds = beam_bounds(right);
    let overlap_left = left_bounds.x.max(right_bounds.x);
    let overlap_right =
        (left_bounds.x + left_bounds.width).min(right_bounds.x + right_bounds.width);
    if overlap_right <= overlap_left {
        return false;
    }
    let x = f64::from(overlap_left + overlap_right) / 2.0;
    (left.median.y_at_x(x) - right.median.y_at_x(x)).abs() <= (left.height.max(right.height) * 0.75)
}

#[cfg(test)]
mod tests {
    use audiveris_image::run_table::{FOREGROUND, Orientation, Run, RunTable};

    use super::*;
    use crate::beam_inters::beam_grade;

    fn raster_with_hook() -> RunTable {
        let mut table = RunTable::new(Orientation::Vertical, 60, 50).unwrap();
        // A right-facing hook at y=16, four pixels thick and 12 pixels wide.
        for x in 25..37 {
            table.add_run(x, Run::new(14, 4)).unwrap();
        }
        assert_eq!(table.get(25, 14), FOREGROUND);
        table
    }

    fn raster_with_secondary_beam() -> RunTable {
        let mut table = RunTable::new(Orientation::Vertical, 70, 50).unwrap();
        for x in 26..44 {
            table.add_run(x, Run::new(14, 4)).unwrap();
        }
        table
    }

    fn base_beam() -> RawBeam {
        let impacts = audiveris_image::beam_structure::BeamImpacts {
            width: 1.0,
            min_height: 1.0,
            max_height: 1.0,
            core: 1.0,
            belt: 1.0,
            distance: 0.9,
            raster: audiveris_image::beam_structure::BeamRasterEvidence {
                core_foreground: 40,
                core_count: 40,
                belt_foreground: 0,
                belt_count: 20,
                core_ratio: 1.0,
                belt_ratio: 0.0,
                rounded_width: 20,
            },
        };
        RawBeam {
            kind: BeamKind::Beam,
            item: BeamItem {
                median: Segment {
                    x1: 10.0,
                    y1: 22.0,
                    x2: 45.0,
                    y2: 22.0,
                },
                height: 4.0,
            },
            impacts,
            grade: beam_grade(impacts),
        }
    }

    fn seed() -> ExtensionGlyph {
        ExtensionGlyph {
            id: 91,
            left: 24,
            top: 12,
            width: 3,
            height: 16,
            vertical_median: Some(Segment {
                x1: 25.0,
                y1: 12.0,
                x2: 25.0,
                y2: 28.0,
            }),
        }
    }

    fn second_seed() -> ExtensionGlyph {
        ExtensionGlyph {
            id: 92,
            left: 44,
            top: 12,
            width: 3,
            height: 16,
            vertical_median: Some(Segment {
                x1: 45.0,
                y1: 12.0,
                x2: 45.0,
                y2: 28.0,
            }),
        }
    }

    #[test]
    fn disabled_policy_is_an_exact_no_op() {
        let table = raster_with_hook();
        let base = [base_beam()];
        let seeds = [seed()];
        let item = ItemParameters::new(10, 4.0, false);
        let sheet = SheetParameters::new(10);
        let result = recover_stem_guided_hooks(
            StemGuidedHookRecoveryInput {
                beams: &base,
                occupied: &base,
                stem_seeds: &seeds,
                raster: BeamRaster {
                    table: &table,
                    offset_x: 0,
                    offset_y: 0,
                },
                item_parameters: &item,
                sheet: &sheet,
            },
            StemGuidedHookRecoveryConfig::default(),
        );
        assert!(result.is_empty());
    }

    #[test]
    fn recovers_only_the_ink_backed_direction_at_the_adjacent_level() {
        let table = raster_with_hook();
        let base = [base_beam()];
        let seeds = [seed()];
        let item = ItemParameters::new(10, 4.0, false);
        let sheet = SheetParameters::new(10);
        let result = recover_stem_guided_hooks(
            StemGuidedHookRecoveryInput {
                beams: &base,
                occupied: &base,
                stem_seeds: &seeds,
                raster: BeamRaster {
                    table: &table,
                    offset_x: 0,
                    offset_y: 0,
                },
                item_parameters: &item,
                sheet: &sheet,
            },
            StemGuidedHookRecoveryConfig::enabled(),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].stem_seed_id, 91);
        assert_eq!(result[0].side, StemGuidedHookSide::Above);
        assert_eq!(result[0].direction, StemGuidedHookDirection::Right);
        assert_eq!(result[0].hook.kind, BeamKind::Hook);
    }

    #[test]
    fn seed_must_reach_the_parent_and_secondary_beam_levels() {
        let table = raster_with_hook();
        let base = [base_beam()];
        let mut short_seed = seed();
        short_seed.vertical_median = Some(Segment {
            x1: 25.0,
            y1: 20.0,
            x2: 25.0,
            y2: 24.0,
        });
        let seeds = [short_seed];
        let item = ItemParameters::new(10, 4.0, false);
        let sheet = SheetParameters::new(10);
        let result = recover_stem_guided_hooks(
            StemGuidedHookRecoveryInput {
                beams: &base,
                occupied: &base,
                stem_seeds: &seeds,
                raster: BeamRaster {
                    table: &table,
                    offset_x: 0,
                    offset_y: 0,
                },
                item_parameters: &item,
                sheet: &sheet,
            },
            StemGuidedHookRecoveryConfig::enabled(),
        );
        assert!(result.is_empty());
    }

    #[test]
    fn recovers_a_complete_secondary_fragment_between_two_stems() {
        let table = raster_with_secondary_beam();
        let base = [base_beam()];
        let seeds = [seed(), second_seed()];
        let item = ItemParameters::new(10, 4.0, false);
        let sheet = SheetParameters::new(10);
        let result = recover_stem_guided_hooks(
            StemGuidedHookRecoveryInput {
                beams: &base,
                occupied: &base,
                stem_seeds: &seeds,
                raster: BeamRaster {
                    table: &table,
                    offset_x: 0,
                    offset_y: 0,
                },
                item_parameters: &item,
                sheet: &sheet,
            },
            StemGuidedHookRecoveryConfig::enabled(),
        );
        let paired = result
            .iter()
            .find(|recovery| recovery.paired_stem_seed_id == Some(92))
            .expect("a two-stem secondary beam");
        assert_eq!(paired.stem_seed_id, 91);
        assert_eq!(paired.hook.kind, BeamKind::Beam);
    }

    #[test]
    fn partial_fragment_does_not_hide_a_material_extension() {
        let candidate = BeamItem {
            median: Segment {
                x1: 20.0,
                y1: 16.0,
                x2: 40.0,
                y2: 16.0,
            },
            height: 4.0,
        };
        let mut fragment = base_beam();
        fragment.item = BeamItem {
            median: Segment {
                x1: 20.0,
                y1: 16.0,
                x2: 27.0,
                y2: 16.0,
            },
            height: 4.0,
        };
        assert!((occupied_ratio(candidate, &[fragment]) - 0.35).abs() < 1.0e-9);
    }
}
