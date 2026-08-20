// SPDX-License-Identifier: AGPL-3.0-or-later

//! Raster-only port of the geometry kernel behind Java `BeamStructure` and
//! `BeamsBuilder.computeImpacts`.
//!
//! Interpretation grading and SIG materialization deliberately remain outside
//! this module. The input is the same vertical run-table geometry used by the
//! Java beam-spot path; the output is deterministic border, item, and raster
//! evidence that a caller can classify or materialize independently.

use std::{
    cmp::{Ordering, Reverse},
    error::Error,
    fmt,
};

use audiveris_core::java_math::java_positive_pow;

use crate::{
    run_table::{FOREGROUND, Orientation, RunTable},
    section::{JunctionPolicy, Section, build_sections},
};

const MAX_SECTION_SLOPE_GAP: f64 = 0.3;
const MAX_BORDER_JITTER_RATIO: f64 = 0.8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl Segment {
    #[must_use]
    pub fn slope(self) -> f64 {
        (self.y2 - self.y1) / (self.x2 - self.x1)
    }

    #[must_use]
    pub fn y_at_x(self, x: f64) -> f64 {
        line_util_y_at_x(self.x1, self.y1, self.x2, self.y2, x)
    }

    #[must_use]
    pub fn at_x(self, x: f64) -> (f64, f64) {
        (x, self.y_at_x(x))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamItem {
    pub median: Segment,
    pub height: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeamLine {
    pub median: Segment,
    pub height: f64,
    pub items: Vec<BeamItem>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamStructureParameters {
    pub typical_height: f64,
    pub core_section_width: usize,
    pub min_hook_width_low: f64,
    pub max_item_x_gap: i32,
    pub min_beam_width_low: f64,
    pub max_hook_width: f64,
    /// Java's dormant `allowBorderCreation`: synthesize a missing opposite
    /// border instead of refusing the structure. False is Java's shipped
    /// behavior.
    pub allow_border_creation: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeamStructureAnalysis {
    pub lines: Vec<BeamLine>,
    pub global_distance: f64,
    pub mean_thickness: f64,
    /// True when this structure is an outer-ink envelope rather than paired
    /// borders. Envelope structures split by ink projection or not at all:
    /// the arithmetic splits would happily cut a fused ledger-and-notehead
    /// row -- whose envelope is tall enough to pass every height rule --
    /// into credible false beams (measured: the page-9 m50 ledger fixtures
    /// are the guard).
    pub envelope: bool,
    /// Median `y1` of every line built on synthesized evidence: a border the
    /// creation retry invented, or a level the ink-deficit probe proposed.
    /// Such lines take the inner-line belt treatment -- their outer belt is
    /// exactly the fused note ink whose border loss made the synthesis
    /// necessary -- and still pass every other impact on their own ink.
    /// Stored as (reference x, y at that x): `adjust_sides` moves the
    /// endpoints along the line, so `y1` is not stable but y at any fixed
    /// abscissa is.
    pub synthetic_medians: Vec<(f64, f64)>,
}

impl BeamStructureAnalysis {
    /// Java `BeamStructure.splitLines`: split only the single-line/two-beam
    /// case selected by `Math.rint` (ties to even). Newly split lines have no
    /// items, just as in Java; downstream item retrieval/materialization owns
    /// any subsequent interpretation work.
    pub fn split_stuck_lines(&mut self, typical_height: f64) {
        self.split_stuck_lines_up_to(typical_height, 2);
    }

    /// [`Self::split_stuck_lines`] generalized past Java's two-beam case.
    ///
    /// Java derives how many beams are fused -- `rint(meanThickness /
    /// typicalHeight)` -- and then splits into exactly two regardless, so a
    /// three- or four-beam stack can never be recovered once its gutters fill
    /// in. `max_count == 2` is that exact behavior; a larger limit lays the
    /// derived count out evenly across the same total height, which is the
    /// only evidence available when the gaps are no longer visible.
    ///
    /// At `max_count == 2` this is arithmetically identical to Java: the
    /// two-line case reduces to the same heights and offsets.
    pub fn split_stuck_lines_up_to(&mut self, typical_height: f64, max_count: usize) {
        let target_count = (self.mean_thickness / typical_height).round_ties_even() as usize;
        if self.lines.len() > 1 || target_count <= self.lines.len() {
            return;
        }
        let count = target_count.min(max_count.max(2));
        let line = &self.lines[0];
        let parent_synthetic = self.is_synthetic_line(line);
        let total_gutter = line.height - (count as f64 * typical_height);
        if total_gutter < 0.0 {
            return;
        }
        let gutter_each = total_gutter / (count - 1) as f64;
        let median = line.median;
        let first = -(line.height / 2.0) + (typical_height / 2.0);
        self.lines = (0..count)
            .map(|index| {
                let dy = first + (index as f64 * (typical_height + gutter_each));
                BeamLine {
                    median: Segment {
                        y1: median.y1 + dy,
                        y2: median.y2 + dy,
                        ..median
                    },
                    height: typical_height,
                    items: Vec::new(),
                }
            })
            .collect();
        // Provenance is inherited; flag-off structures carry no markers, so
        // this cannot disturb parity.
        if parent_synthetic {
            let children: Vec<BeamLine> = self.lines.clone();
            for child in &children {
                self.mark_synthetic(child);
            }
        }
    }

    /// Split a thick line along the gray seams between its beams
    /// (enhancement).
    ///
    /// Binarization destroys exactly the cue a reader uses on a low-res
    /// scan: a bridged gutter is not as DARK as the beams it joins. The
    /// pre-closing raster is grayscale, so the mean gray along the
    /// slope-corrected line at each vertical offset gives a profile in
    /// which beams are dark bands and gutters are lighter seams -- even
    /// seams well below the binarization threshold. The band count is
    /// authoritative: it can neither overcount (arithmetic on an envelope
    /// fattened by fused noteheads reads four where there are three) nor
    /// undercount fused pairs whose seam survives in gray. Lines are
    /// placed at the darkness-weighted centroid of each band. Lines where
    /// fewer than two bands emerge are left for the run projection and the
    /// arithmetic splits; split children have no items -- run
    /// [`Self::populate_empty_items`] afterwards.
    #[allow(clippy::too_many_arguments)]
    pub fn split_lines_by_gray_seams(
        &mut self,
        erased: &[u8],
        image_width: usize,
        image_height: usize,
        typical_height: f64,
        max_count: usize,
    ) {
        /// A seam must be lighter than its darker neighboring band by this
        /// fraction of the line's own ink-to-paper range. Nothing guarantees
        /// a normalized scan -- the Schenker pages happen to span 20..255,
        /// but a dim photocopy might span 80..200 -- so the thresholds are
        /// relative to what this line's profile actually contains.
        const SEAM_CONTRAST_RATIO: f64 = 0.07;
        /// A valley must be at least this deep into the range to be ink.
        const BAND_CEILING_RATIO: f64 = 0.65;
        const STEP: f64 = 0.5;
        let lines = std::mem::take(&mut self.lines);
        for line in lines {
            let parent_synthetic = self.is_synthetic_line(&line);
            let count_by_height =
                (line.height / typical_height).round_ties_even() as usize;
            if count_by_height < 2 {
                self.lines.push(line);
                continue;
            }
            let x1 = (line.median.x1.max(0.0)) as usize;
            let x2 = (line.median.x2.min(image_width as f64 - 1.0)) as usize;
            if x2 <= x1 {
                self.lines.push(line);
                continue;
            }
            let reach = line.height / 2.0 + typical_height / 2.0;
            let steps = (2.0 * reach / STEP) as usize + 1;
            let mut profile = Vec::with_capacity(steps);
            for index in 0..steps {
                let dy = -reach + index as f64 * STEP;
                let mut sum = 0.0;
                let mut count = 0usize;
                for x in x1..=x2 {
                    let y = line.median.y_at_x(x as f64) + dy;
                    if y < 0.0 || y >= image_height as f64 {
                        continue;
                    }
                    sum += f64::from(erased[y as usize * image_width + x]);
                    count += 1;
                }
                profile.push(if count == 0 { 255.0 } else { sum / count as f64 });
            }
            // Per-line normalization: ink is the darkest the profile gets,
            // paper the lightest; both come from this very line.
            // Light smoothing so single-pixel noise cannot fake a seam.
            let smooth: Vec<f64> = (0..profile.len())
                .map(|index| {
                    let lo = index.saturating_sub(1);
                    let hi = (index + 1).min(profile.len() - 1);
                    (profile[lo] + profile[index] + profile[hi]) / 3.0
                })
                .collect();
            let ink = smooth.iter().copied().fold(f64::MAX, f64::min);
            let paper = smooth.iter().copied().fold(f64::MIN, f64::max);
            let range = (paper - ink).max(1.0);
            let seam_contrast = SEAM_CONTRAST_RATIO * range;
            let band_ceiling = ink + BAND_CEILING_RATIO * range;
            // A seam on a real scan is lighter than the beams around it but
            // often darker than any fixed ceiling, so a threshold crossing
            // cannot find it. Valley-and-peak analysis can: valleys are the
            // beam centers (dark enough to be ink), and a peak between two
            // valleys is a seam when it is lighter than the darker valley by
            // the contrast margin; otherwise the two valleys are texture
            // inside one beam and merge (keeping the darker).
            let mut valleys: Vec<usize> = Vec::new();
            for index in 1..smooth.len().saturating_sub(1) {
                if smooth[index] <= band_ceiling
                    && smooth[index] <= smooth[index - 1]
                    && smooth[index] <= smooth[index + 1]
                    && (valleys.last() != Some(&(index - 1))
                        || smooth[index] < smooth[index - 1])
                {
                    valleys.push(index);
                }
            }
            let mut kept: Vec<usize> = Vec::new();
            let mut boundaries: Vec<usize> = Vec::new();
            for valley in valleys {
                if let Some(&last) = kept.last() {
                    let (seam_index, seam) = smooth[last..=valley]
                        .iter()
                        .enumerate()
                        .map(|(offset, &value)| (last + offset, value))
                        .fold((last, f64::MIN), |best, candidate| {
                            if candidate.1 > best.1 { candidate } else { best }
                        });
                    let darker = smooth[last].max(smooth[valley]);
                    if seam - darker < seam_contrast {
                        // Same beam: keep whichever valley is darker.
                        if smooth[valley] < smooth[last] {
                            *kept.last_mut().unwrap() = valley;
                        }
                        continue;
                    }
                    boundaries.push(seam_index);
                }
                kept.push(valley);
            }
            // Bands span boundary to boundary around each kept valley.
            let mut merged: Vec<(usize, usize)> = Vec::new();
            for (which, _valley) in kept.iter().enumerate() {
                let lo = if which == 0 { 0 } else { boundaries[which - 1] };
                let hi = if which + 1 == kept.len() {
                    smooth.len() - 1
                } else {
                    boundaries[which]
                };
                merged.push((lo, hi));
            }
            // A band narrower than a third of a beam is an edge artifact.
            merged.retain(|&(s, e)| (e - s + 1) as f64 * STEP >= 0.35 * typical_height);
            // One THIN band is a verdict, not a decline: the gray shows a
            // single beam's worth of ink and no seam, so the fused-head
            // arithmetic (envelope height over typical reads two) must not
            // manufacture a second beam. Collapse to one line at the band
            // centroid. A thick seamless band stays undecided -- a fully
            // saturated pair shows no seam either, and the later splitters
            // keep their chance.
            if merged.len() == 1 {
                let (s, e) = merged[0];
                if (e - s + 1) as f64 * STEP <= 1.35 * typical_height {
                    let mut weight_sum = 0.0;
                    let mut dy_sum = 0.0;
                    for index in s..=e {
                        let weight = 255.0 - smooth[index];
                        weight_sum += weight;
                        dy_sum += weight * (-reach + index as f64 * STEP);
                    }
                    let dy = if weight_sum > 0.0 { dy_sum / weight_sum } else { 0.0 };
                    let child = BeamLine {
                        median: Segment {
                            y1: line.median.y1 + dy,
                            y2: line.median.y2 + dy,
                            ..line.median
                        },
                        height: typical_height,
                        items: Vec::new(),
                    };
                    if parent_synthetic {
                        self.mark_synthetic(&child);
                    }
                    self.lines.push(child);
                    continue;
                }
            }
            if merged.len() < 2 || merged.len() > max_count.max(2) {
                self.lines.push(line);
                continue;
            }
            for (s, e) in merged {
                let mut weight_sum = 0.0;
                let mut dy_sum = 0.0;
                for index in s..=e {
                    let weight = 255.0 - smooth[index];
                    weight_sum += weight;
                    dy_sum += weight * (-reach + index as f64 * STEP);
                }
                let dy = if weight_sum > 0.0 { dy_sum / weight_sum } else { 0.0 };
                let child = BeamLine {
                    median: Segment {
                        y1: line.median.y1 + dy,
                        y2: line.median.y2 + dy,
                        ..line.median
                    },
                    height: typical_height,
                    items: Vec::new(),
                };
                if parent_synthetic {
                    self.mark_synthetic(&child);
                }
                self.lines.push(child);
            }
        }
        self.lines
            .sort_by(|one, two| one.median.y1.total_cmp(&two.median.y1));
    }

    /// Re-derive a line's levels from the pre-closing ink (enhancement).
    ///
    /// Even spacing is arithmetic, not evidence: an envelope over a fused
    /// stack includes fused notehead ink, so `height / typical` over-counts
    /// the levels and the phantom lines land on gutters and die core-pale
    /// (measured: a four-level stack split into seven). And the closed
    /// raster cannot help -- the closing disk swallows the gutters, which
    /// is what made the stack an envelope case to begin with -- so the
    /// levels are read from the PRE-closing raster, where each beam is
    /// still a separate bar. Each column's ink runs vote with their
    /// center's offset from the line median (slope-corrected by
    /// construction) and the offsets cluster into one group per level;
    /// lines are placed at the cluster medians. Falls back to leaving the
    /// line alone when fewer than two clusters emerge; run
    /// [`Self::populate_empty_items`] afterwards.
    #[allow(clippy::too_many_arguments)]
    pub fn split_lines_by_projection(
        &mut self,
        erased: &[u8],
        image_width: usize,
        image_height: usize,
        ink_threshold: u8,
        interline: f64,
        typical_height: f64,
        max_count: usize,
    ) {
        // Vote hygiene: a ledger or staff residue run is thinner than any
        // beam, and a notehead run is taller than one; neither is level
        // evidence. Both bounds were measured on the page-9 ledger fixtures
        // -- without them the head and ledger rows of a fused run vote
        // themselves into credible false beams.
        let run_floor = 0.35 * interline;
        let run_cap = 1.3 * typical_height;
        let lines = std::mem::take(&mut self.lines);
        for line in lines {
            let parent_synthetic = self.is_synthetic_line(&line);
            let count_by_height =
                (line.height / typical_height).round_ties_even() as usize;
            if count_by_height < 2 {
                self.lines.push(line);
                continue;
            }
            let reach = line.height / 2.0 + typical_height;
            let x1 = (line.median.x1.max(0.0)) as usize;
            let x2 = (line.median.x2.min(image_width as f64 - 1.0)) as usize;
            let mut offsets: Vec<(usize, f64)> = Vec::new();
            for x in x1..=x2 {
                let median_y = line.median.y_at_x(x as f64);
                let lo = ((median_y - reach).max(0.0)) as usize;
                let hi = ((median_y + reach).min(image_height as f64 - 1.0)) as usize;
                let mut start: Option<usize> = None;
                for y in lo..=hi {
                    // `spots::threshold` counts `pixel <= level` as ink.
                    let ink = erased[y * image_width + x] <= ink_threshold;
                    let run_end = |s: usize, e: usize, offsets: &mut Vec<(usize, f64)>| {
                        let length = (e - s) as f64;
                        if length >= run_floor && length <= run_cap {
                            offsets.push((x, (s + e) as f64 / 2.0 - median_y));
                        }
                    };
                    match (ink, start) {
                        (true, None) => start = Some(y),
                        (false, Some(s)) => {
                            run_end(s, y, &mut offsets);
                            start = None;
                        }
                        _ => {}
                    }
                }
                if let Some(s) = start {
                    let length = (hi + 1 - s) as f64;
                    if length >= run_floor && length <= run_cap {
                        offsets.push((x, (s + hi + 1) as f64 / 2.0 - median_y));
                    }
                }
            }
            offsets.sort_by(|a, b| a.1.total_cmp(&b.1));
            let mut clusters: Vec<Vec<(usize, f64)>> = Vec::new();
            for &(x, dy) in &offsets {
                match clusters.last_mut() {
                    Some(cluster)
                        if dy - cluster[cluster.len() / 2].1 <= 0.6 * typical_height =>
                    {
                        cluster.push((x, dy));
                    }
                    _ => clusters.push(vec![(x, dy)]),
                }
            }
            // A level runs the length of the beam; fused noteheads, ledger
            // stubs, and a neighbor's ink do not. Clusters must be supported
            // by at least a third of the line's columns.
            let span = (x2 - x1 + 1).max(1);
            let support = |cluster: &[(usize, f64)]| {
                let mut columns: Vec<usize> = cluster.iter().map(|&(x, _)| x).collect();
                columns.sort_unstable();
                columns.dedup();
                columns.len()
            };
            // And a level is STRAIGHT: a bar's run centers align within a
            // pixel of each other once the median's slope is subtracted,
            // while the partial edge runs of a fused notehead row scatter.
            // Median absolute deviation separates them cleanly (measured:
            // without this, the page-9 fused head row voted itself into a
            // credible false beam that re-blocked the lone A).
            let tight = |cluster: &[(usize, f64)]| {
                let center = cluster[cluster.len() / 2].1;
                let mut deviations: Vec<f64> =
                    cluster.iter().map(|&(_, dy)| (dy - center).abs()).collect();
                deviations.sort_by(f64::total_cmp);
                deviations[deviations.len() / 2] <= (0.35 * typical_height).max(0.8)
            };
            clusters.retain(|cluster| support(cluster) * 3 >= span && tight(cluster));
            // An envelope structure carries no border evidence at all, so
            // two clusters are as likely a fused ledger row plus a head row
            // as a beam pair -- the population that vetoed the page-9 lone
            // A. Only a three-or-more stack is unambiguous from projection
            // alone; paired-border structures keep the two-cluster case.
            let min_clusters = if self.envelope { 3 } else { 2 };
            if clusters.len() < min_clusters || clusters.len() > max_count.max(2) {
                self.lines.push(line);
                continue;
            }
            for cluster in clusters {
                let dy = cluster[cluster.len() / 2].1;
                let child = BeamLine {
                    median: Segment {
                        y1: line.median.y1 + dy,
                        y2: line.median.y2 + dy,
                        ..line.median
                    },
                    height: typical_height,
                    items: Vec::new(),
                };
                if parent_synthetic {
                    self.mark_synthetic(&child);
                }
                self.lines.push(child);
            }
        }
        self.lines
            .sort_by(|one, two| one.median.y1.total_cmp(&two.median.y1));
    }

    /// Whether a line rests on synthesized evidence (envelope structures
    /// count wholesale). Split children must re-register through
    /// [`Self::mark_synthetic`] because their medians move.
    fn is_synthetic_line(&self, line: &BeamLine) -> bool {
        self.envelope
            || self.synthetic_medians.iter().any(|&(x_ref, y_ref)| {
                (line.median.y_at_x(x_ref) - y_ref).abs() < 0.5
            })
    }

    fn mark_synthetic(&mut self, line: &BeamLine) {
        self.synthetic_medians.push((
            (line.median.x1 + line.median.x2) / 2.0,
            (line.median.y1 + line.median.y2) / 2.0,
        ));
    }

    /// Drop synthetic lines the pre-closing ink cannot substantiate
    /// (enhancement).
    ///
    /// A created or probed line is a hypothesis, and the closed raster
    /// cannot audit it: the closing fattens a 2px ledger row into a
    /// beam-thick bar, so a line invented opposite a ledger's own (long,
    /// straight) border grades like a beam and earns veto power (measured:
    /// the page-9 lone A fixture caught exactly this). The pre-closing
    /// raster still knows the difference. Each synthetic line must find
    /// ink at its median in at least half its columns, with a mean run
    /// length of at least 0.7x the typical beam height.
    pub fn retain_substantiated_synthetic_lines(
        &mut self,
        erased: &[u8],
        image_width: usize,
        image_height: usize,
        ink_threshold: u8,
        typical_height: f64,
    ) {
        if self.synthetic_medians.is_empty() {
            return;
        }
        let synthetic = self.synthetic_medians.clone();
        self.lines.retain(|line| {
            if !synthetic
                .iter()
                .any(|&(x_ref, y_ref)| (line.median.y_at_x(x_ref) - y_ref).abs() < 0.5)
            {
                return true;
            }
            let x1 = (line.median.x1.max(0.0)) as usize;
            let x2 = (line.median.x2.min(image_width as f64 - 1.0)) as usize;
            if x2 <= x1 {
                return false;
            }
            let mut inked_columns = 0_usize;
            let mut total_length = 0.0_f64;
            for x in x1..=x2 {
                let y = line.median.y_at_x(x as f64);
                if y < 0.0 || y >= image_height as f64 {
                    continue;
                }
                let y = y as usize;
                // `spots::threshold` counts `pixel <= level` as ink.
                if erased[y * image_width + x] > ink_threshold {
                    continue;
                }
                let mut lo = y;
                while lo > 0 && erased[(lo - 1) * image_width + x] <= ink_threshold {
                    lo -= 1;
                }
                let mut hi = y;
                while hi + 1 < image_height
                    && erased[(hi + 1) * image_width + x] <= ink_threshold
                {
                    hi += 1;
                }
                inked_columns += 1;
                total_length += (hi - lo + 1) as f64;
            }
            let span = x2 - x1 + 1;
            inked_columns * 2 >= span
                && total_length / inked_columns.max(1) as f64 >= 0.7 * typical_height
        });
    }

    /// Split every line that is itself thick enough to be a fused pair
    /// (enhancement).
    ///
    /// Java's `splitLines` handles only the fully fused case -- one border
    /// line for the whole stack -- and its own TODOs ask "what if beamCount =
    /// 2 and targetCount = 3 or more?". A partially fused stack pairs into
    /// two lines, one thin and one still holding a fused pair, and that pair
    /// either dies on the height ceiling or is accepted as a single fat beam.
    /// The whole-glyph ink ratio cannot see it: gaps in the blob dilute
    /// weight/width below the split threshold. Here each line is split by its
    /// own border-fit height. Unlike Java's whole-glyph split, a negative
    /// gutter is allowed: two beams whose gutter the closing swallowed
    /// entirely measure thinner than two typical heights, and the honest
    /// geometry is two typical beams overlapping slightly. Split lines lose
    /// their items; run [`Self::populate_empty_items`] afterwards.
    /// `max_single_line_width` guards the lone-line case: inside a
    /// multi-line structure a thick line is beam-stack context, but a lone
    /// wide fat line at PAIR height is as likely a fused ledger-and-notehead
    /// row as a beam pair (both measure ~2x typical), and splitting one
    /// manufactures credible false beams that veto the true ledgers
    /// (measured: the page-9 m50 run peaks died exactly this way). A narrow
    /// fat fragment stays splittable -- below the credible-veto width it
    /// cannot veto anything -- and so does a lone line of at least triple
    /// height: no ledger-and-head row is three typical heights of solid
    /// horizontal ink, only a beam stack is.
    pub fn split_thick_lines_up_to(
        &mut self,
        typical_height: f64,
        max_count: usize,
        max_single_line_width: f64,
    ) {
        let single = self.lines.len() == 1;
        let lines = std::mem::take(&mut self.lines);
        for line in lines {
            let parent_synthetic = self.is_synthetic_line(&line);
            let count = (line.height / typical_height).round_ties_even() as usize;
            // A lone wide line at pair height is as likely a fused ledger row
            // (ledger plus a head row measures ~2x typical) -- but nothing
            // except a beam stack is three typical heights of solid
            // horizontal ink, so a triple-height lone line splits.
            let guarded = single
                && line.median.x2 - line.median.x1 > max_single_line_width
                && count < 3;
            if count < 2 || guarded {
                self.lines.push(line);
                continue;
            }
            let count = count.min(max_count.max(2));
            let gutter_each = (line.height - (count as f64 * typical_height))
                / (count - 1) as f64;
            let first = -(line.height / 2.0) + (typical_height / 2.0);
            let median = line.median;
            for index in 0..count {
                let dy = first + (index as f64 * (typical_height + gutter_each));
                let child = BeamLine {
                    median: Segment {
                        y1: median.y1 + dy,
                        y2: median.y2 + dy,
                        ..median
                    },
                    height: typical_height,
                    items: Vec::new(),
                };
                // Provenance is inherited: a level cut out of synthesized
                // evidence is itself synthesized evidence.
                if parent_synthetic {
                    self.mark_synthetic(&child);
                }
                self.lines.push(child);
            }
        }
    }

    /// Propose the stack levels the borders lost to fusion (enhancement).
    ///
    /// A thirty-second stack whose top beam fuses with the run's noteheads
    /// loses that level's borders entirely; the remaining borders pair
    /// evenly, so the whole structure builds cleanly -- one level short. The
    /// glyph's own ink records the loss: `mean_thickness` implies more
    /// levels than the structure holds lines. For each missing level this
    /// proposes a candidate line above and below the stack, at the stack's
    /// own level spacing, and keeps the side whose median actually runs
    /// through ink (at least half its columns); item grading then accepts or
    /// refuses the proposal like any other line. Multi-line structures only:
    /// a lone line's ink surplus is as likely fused noteheads or a ledger
    /// row (the page-9 lesson) as a hidden level. Added lines have no items;
    /// run [`Self::populate_empty_items`] afterwards.
    pub fn add_missing_outer_lines(
        &mut self,
        glyph: &RunTable,
        offset_x: i32,
        offset_y: i32,
        typical_height: f64,
        max_count: usize,
    ) {
        if self.lines.len() < 2 {
            return;
        }
        let target = ((self.mean_thickness / typical_height).round_ties_even() as usize)
            .min(max_count.max(2));
        if target <= self.lines.len() {
            return;
        }
        let sections = build_sections(glyph, JunctionPolicy::DEFAULT_RATIO);
        let coverage = |median: Segment| -> f64 {
            let x1 = median.x1.round_ties_even() as i32;
            let x2 = median.x2.round_ties_even() as i32;
            if x2 <= x1 {
                return 0.0;
            }
            let mut hit = 0_usize;
            for x in x1..=x2 {
                let y = median.y_at_x(f64::from(x)) - f64::from(offset_y);
                let x_local = f64::from(x - offset_x);
                if sections
                    .iter()
                    .any(|section| section_contains(section, x_local, y))
                {
                    hit += 1;
                }
            }
            hit as f64 / f64::from(x2 - x1 + 1)
        };
        for _ in self.lines.len()..target {
            let spacing = (self.lines.last().unwrap().median.y1
                - self.lines.first().unwrap().median.y1)
                / (self.lines.len() - 1) as f64;
            let shifted = |line: &BeamLine, dy: f64| Segment {
                y1: line.median.y1 + dy,
                y2: line.median.y2 + dy,
                ..line.median
            };
            let above = shifted(self.lines.first().unwrap(), -spacing);
            let below = shifted(self.lines.last().unwrap(), spacing);
            let (above_cov, below_cov) = (coverage(above), coverage(below));
            let (median, cov) = if above_cov >= below_cov {
                (above, above_cov)
            } else {
                (below, below_cov)
            };
            if cov < 0.5 {
                return;
            }
            self.synthetic_medians
                .push(((median.x1 + median.x2) / 2.0, (median.y1 + median.y2) / 2.0));
            self.lines.push(BeamLine {
                median,
                height: typical_height,
                items: Vec::new(),
            });
            self.lines
                .sort_by(|one, two| one.median.y1.total_cmp(&two.median.y1));
        }
    }

    /// Retrieve items for lines the split left empty (enhancement).
    ///
    /// Java's `splitLines` replaces a fused line -- whose items `computeLines`
    /// had already retrieved -- with fresh `BeamLine`s whose item lists are
    /// empty, and nothing repopulates them, so `createBeamInters` iterates
    /// nothing and every stuck beam stack silently produces zero inters: the
    /// split is dead code. On a clean scan the gutters between stacked beams
    /// stay open and the split rarely fires; on a low-resolution scan the
    /// morphological closing bridges them, so every multi-beam group dies
    /// here. This re-runs the same item retrieval `computeLines` used, on the
    /// same sections, for each line that has no items. Call sites gate it;
    /// not calling it is byte-exact Java.
    pub fn populate_empty_items(
        &mut self,
        glyph: &RunTable,
        offset_x: i32,
        offset_y: i32,
        max_item_x_gap: i32,
    ) {
        if self.lines.iter().all(|line| !line.items.is_empty()) {
            return;
        }
        let sections = build_sections(glyph, JunctionPolicy::DEFAULT_RATIO);
        for line in &mut self.lines {
            if line.items.is_empty() {
                line.items = retrieve_items(
                    &sections,
                    offset_x,
                    offset_y,
                    line.median,
                    line.height,
                    max_item_x_gap,
                );
            }
        }
    }

    /// Java `BeamStructure.adjustSides` over the tight run-table bounds.
    pub fn adjust_sides(&mut self, glyph_left: i32, glyph_width: usize, min_beam_width_low: f64) {
        let left = f64::from(glyph_left);
        let right = left + glyph_width as f64 - 1.0;
        for line in &mut self.lines {
            if line.median.x1 - left < min_beam_width_low {
                let (_, y1) = line.median.at_x(left);
                line.median.x1 = left;
                line.median.y1 = y1;
            }
            if right - line.median.x2 < min_beam_width_low {
                let (_, y2) = line.median.at_x(right);
                line.median.x2 = right;
                line.median.y2 = y2;
            }
        }
    }

    #[must_use]
    pub fn max_consecutive_slope_gap(&self, max_hook_width: f64) -> f64 {
        let mut previous = None;
        let mut maximum = 0.0_f64;
        for line in &self.lines {
            if line.median.x2 - line.median.x1 > max_hook_width {
                let slope = line.median.slope();
                if let Some(previous) = previous {
                    maximum = maximum.max(f64::abs(slope - previous));
                }
                previous = Some(slope);
            }
        }
        maximum
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeamStructureError {
    NonVerticalRunTable,
    NoUsableTopBorder,
    NoUsableBottomBorder,
    InconsistentBorderPairs,
    UndefinedBorderLine,
}

impl fmt::Display for BeamStructureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for BeamStructureError {}

/// Retrieve border lines and beam items from one tight beam-spot run table.
pub fn analyze_beam_structure(
    glyph: &RunTable,
    offset_x: i32,
    offset_y: i32,
    parameters: BeamStructureParameters,
) -> Result<BeamStructureAnalysis, BeamStructureError> {
    if glyph.orientation() != Orientation::Vertical {
        return Err(BeamStructureError::NonVerticalRunTable);
    }
    let sections = build_sections(glyph, JunctionPolicy::DEFAULT_RATIO);
    let center = glyph_centroid(glyph, offset_x, offset_y);
    let top = border_lines(
        &sections,
        offset_x,
        offset_y,
        center,
        parameters,
        BorderSide::Top,
    )?;
    let bottom = border_lines(
        &sections,
        offset_x,
        offset_y,
        center,
        parameters,
        BorderSide::Bottom,
    )?;
    let global_distance = top
        .iter()
        .chain(&bottom)
        .map(|line| line.mean_distance() * line.count)
        .sum::<f64>()
        / top
            .iter()
            .chain(&bottom)
            .map(|line| line.count)
            .sum::<f64>();
    let global_slope = top
        .iter()
        .chain(&bottom)
        .max_by(|one, two| {
            one.span()
                .partial_cmp(&two.span())
                .unwrap_or(Ordering::Equal)
        })
        .ok_or(BeamStructureError::UndefinedBorderLine)?
        .slope();
    let mut top = line_map(top, center, global_slope)?;
    let mut bottom = line_map(bottom, center, global_slope)?;
    let mut created_offsets: Vec<f64> = Vec::new();
    let complete = |top: &mut Vec<(f64, Segment)>,
                    bottom: &mut Vec<(f64, Segment)>,
                    created: &mut Vec<f64>,
                    create: bool| {
        complete_border_lines(
            1.0,
            center,
            global_slope,
            parameters.typical_height,
            create,
            2.0 * parameters.min_beam_width_low,
            top,
            bottom,
            created,
        );
        complete_border_lines(
            -1.0,
            center,
            global_slope,
            parameters.typical_height,
            create,
            2.0 * parameters.min_beam_width_low,
            bottom,
            top,
            created,
        );
    };
    let mut synthetic_envelope = false;
    let saved = parameters
        .allow_border_creation
        .then(|| (top.clone(), bottom.clone()));
    complete(&mut top, &mut bottom, &mut created_offsets, false);
    if top.len() != bottom.len()
        && let Some((saved_top, saved_bottom)) = saved
    {
        // A partially fused stack pairs unevenly -- some gutters open, some
        // bridged -- and Java refuses the whole structure. Only for that
        // already-doomed population, retry with Java's dormant
        // allowBorderCreation branch enabled; structures that pair evenly
        // never take this path, so the healthy population is untouched.
        top = saved_top;
        bottom = saved_bottom;
        created_offsets.clear();
        complete(&mut top, &mut bottom, &mut created_offsets, true);
    }
    top.sort_by(|one, two| one.0.total_cmp(&two.0));
    bottom.sort_by(|one, two| one.0.total_cmp(&two.0));
    if top.len() != bottom.len()
        && parameters.allow_border_creation
        && top.len().min(bottom.len()) >= 3
    {
        // Even with created borders the counts can stay uneven -- fused
        // noteheads manufacture partial borders on one side only. Refusing
        // the whole structure loses every level; salvage instead the
        // alignment that pairs each border with an opposite one typical
        // height away and drop the leftovers of the longer side. Only for
        // structures that are stacks on both sides (three or more borders):
        // a two-border salvage is as likely a fused ledger-and-notehead row
        // manufacturing a credible false beam (measured: it re-blocked the
        // page-9 lone A that four earlier fixes recovered).
        salvage_border_pairs(&mut top, &mut bottom, parameters.typical_height);
    }
    if top.len() != bottom.len() && parameters.allow_border_creation {
        // Last resort before refusing the whole structure: when the borders
        // cannot be paired at all, the outermost top and bottom are still
        // trustworthy -- they face open paper, not fused ink. Collapse to
        // that envelope as one line; the height-based split then derives the
        // level count from the envelope the same way it does for a fully
        // solid stack. This recovers the four-level stacks whose partial
        // gutters yield two borders on one side and three on the other,
        // which no pairing can reconcile.
        let envelope_top = top.remove(0);
        let envelope_bottom = bottom.pop().expect("borders are non-empty");
        top = vec![envelope_top];
        bottom = vec![envelope_bottom];
        synthetic_envelope = true;
    }
    if top.len() != bottom.len() {
        return Err(BeamStructureError::InconsistentBorderPairs);
    }

    let mut lines = Vec::with_capacity(top.len());
    let mut synthetic_medians = Vec::new();
    for ((top_offset, top), (bottom_offset, bottom)) in top.into_iter().zip(bottom) {
        let x1 = top.x1.min(bottom.x1);
        let x2 = top.x2.max(bottom.x2);
        let yt1 = top.y_at_x(x1);
        let yb1 = bottom.y_at_x(x1);
        let yt2 = top.y_at_x(x2);
        let yb2 = bottom.y_at_x(x2);
        let height = ((yb1 - yt1) + (yb2 - yt2)) / 2.0;
        let median = Segment {
            x1,
            y1: (yt1 + yb1) / 2.0,
            x2,
            y2: (yt2 + yb2) / 2.0,
        };
        let items = retrieve_items(
            &sections,
            offset_x,
            offset_y,
            median,
            height,
            parameters.max_item_x_gap,
        );
        if synthetic_envelope
            || created_offsets
                .iter()
                .any(|offset| *offset == top_offset || *offset == bottom_offset)
        {
            synthetic_medians
                .push(((median.x1 + median.x2) / 2.0, (median.y1 + median.y2) / 2.0));
        }
        lines.push(BeamLine {
            median,
            height,
            items,
        });
    }
    Ok(BeamStructureAnalysis {
        lines,
        global_distance,
        mean_thickness: glyph.weight() as f64 / glyph.width() as f64,
        envelope: false,
        synthetic_medians,
    })
}

/// Build a one-line structure from the glyph's outer ink envelope
/// (enhancement).
///
/// When fused inner borders make the normal analysis fail -- pairing that
/// cannot reconcile, or a border-fit residual the straightness gate refuses
/// -- the outermost ink edges are still trustworthy: they face open paper.
/// Least-squares lines through each column's topmost run start and
/// bottommost run stop become the envelope; the projection split then
/// re-derives the levels from the ink inside it. The envelope line is
/// synthetic (belt-exempt), and every later grade still applies.
#[must_use]
pub fn envelope_analysis(
    glyph: &RunTable,
    offset_x: i32,
    offset_y: i32,
) -> Option<BeamStructureAnalysis> {
    if glyph.orientation() != Orientation::Vertical {
        return None;
    }
    let mut top_line = audiveris_core::basic_line::BasicLine::default();
    let mut bottom_line = audiveris_core::basic_line::BasicLine::default();
    let mut x_min = None;
    let mut x_max = None;
    for x in 0..glyph.sequence_count() {
        let Some(runs) = glyph.sequence(x) else {
            continue;
        };
        let (Some(first), Some(last)) = (runs.first(), runs.last()) else {
            continue;
        };
        let gx = f64::from(offset_x) + x as f64;
        top_line.include_point(gx, f64::from(offset_y) + first.start as f64);
        bottom_line.include_point(gx, f64::from(offset_y) + (last.stop() + 1) as f64);
        x_min = Some(x_min.map_or(gx, |v: f64| v.min(gx)));
        x_max = Some(x_max.map_or(gx, |v: f64| v.max(gx)));
    }
    let (x1, x2) = (x_min?, x_max?);
    if x2 <= x1 {
        return None;
    }
    let (yt1, yt2) = (top_line.y_at_x(x1).ok()?, top_line.y_at_x(x2).ok()?);
    let (yb1, yb2) = (bottom_line.y_at_x(x1).ok()?, bottom_line.y_at_x(x2).ok()?);
    let height = ((yb1 - yt1) + (yb2 - yt2)) / 2.0;
    if height <= 0.0 {
        return None;
    }
    let median = Segment {
        x1,
        y1: (yt1 + yb1) / 2.0,
        x2,
        y2: (yt2 + yb2) / 2.0,
    };
    let count = top_line.count() + bottom_line.count();
    let global_distance = (top_line.mean_distance().ok()? * top_line.count() as f64
        + bottom_line.mean_distance().ok()? * bottom_line.count() as f64)
        / count as f64;
    Some(BeamStructureAnalysis {
        lines: vec![BeamLine {
            median,
            height,
            items: Vec::new(),
        }],
        global_distance,
        mean_thickness: glyph.weight() as f64 / glyph.width() as f64,
        envelope: true,
        synthetic_medians: vec![(
            (median.x1 + median.x2) / 2.0,
            (median.y1 + median.y2) / 2.0,
        )],
    })
}

#[derive(Clone, Copy)]
enum BorderSide {
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug)]
struct BasicLine {
    count: f64,
    sum_x: f64,
    sum_y: f64,
    sum_x2: f64,
    sum_y2: f64,
    sum_xy: f64,
    min_x: f64,
    max_x: f64,
}

impl BasicLine {
    fn empty() -> Self {
        Self {
            count: 0.0,
            sum_x: 0.0,
            sum_y: 0.0,
            sum_x2: 0.0,
            sum_y2: 0.0,
            sum_xy: 0.0,
            min_x: f64::MAX,
            max_x: f64::MIN,
        }
    }

    fn include(&mut self, x: f64, y: f64) {
        self.count += 1.0;
        self.sum_x += x;
        self.sum_y += y;
        self.sum_x2 += x * x;
        self.sum_y2 += y * y;
        self.sum_xy += x * y;
        self.min_x = self.min_x.min(x);
        self.max_x = self.max_x.max(x);
    }

    fn include_line(&mut self, other: Self) {
        self.count += other.count;
        self.sum_x += other.sum_x;
        self.sum_y += other.sum_y;
        self.sum_x2 += other.sum_x2;
        self.sum_y2 += other.sum_y2;
        self.sum_xy += other.sum_xy;
        self.min_x = self.min_x.min(other.min_x);
        self.max_x = self.max_x.max(other.max_x);
    }

    fn coefficients(self) -> Option<(f64, f64, f64)> {
        if self.count < 2.0 {
            return None;
        }
        let horizontal = (self.count * self.sum_x2) - (self.sum_x * self.sum_x);
        let vertical = (self.count * self.sum_y2) - (self.sum_y * self.sum_y);
        let (mut a, mut b, mut c) = if horizontal.abs() >= vertical.abs() {
            (
                ((self.count * self.sum_xy) - (self.sum_x * self.sum_y)) / horizontal,
                -1.0,
                ((self.sum_y * self.sum_x2) - (self.sum_x * self.sum_xy)) / horizontal,
            )
        } else {
            (
                -1.0,
                ((self.count * self.sum_xy) - (self.sum_x * self.sum_y)) / vertical,
                ((self.sum_x * self.sum_y2) - (self.sum_y * self.sum_xy)) / vertical,
            )
        };
        // `java_hypot`, not `f64::hypot`: this is a second copy of
        // `BasicLine.normalize`, and the platform libm's answer differs from
        // Java's fdlibm one by an ulp. `mean_distance` below subtracts
        // near-equal terms, which amplifies that ulp to about 1e-9 -- enough to
        // move the ninth decimal, and enough to differ between macOS and Linux.
        // CI caught this on its ubuntu leg while every local run was green.
        let norm = audiveris_core::basic_line::java_hypot(a, b);
        a /= norm;
        b /= norm;
        c /= norm;
        (a.is_finite() && b.is_finite() && c.is_finite()).then_some((a, b, c))
    }

    fn slope(self) -> f64 {
        let (a, b, _) = self.coefficients().expect("usable border line");
        -a / b
    }

    fn y_at_x(self, x: f64) -> f64 {
        let (a, b, c) = self.coefficients().expect("usable border line");
        ((-a * x) - c) / b
    }

    fn segment(self) -> Option<Segment> {
        self.coefficients()?;
        Some(Segment {
            x1: self.min_x,
            y1: self.y_at_x(self.min_x),
            x2: self.max_x,
            y2: self.y_at_x(self.max_x),
        })
    }

    fn span(self) -> f64 {
        self.max_x - self.min_x + 1.0
    }

    fn mean_distance(self) -> f64 {
        let (a, b, c) = self.coefficients().expect("usable border line");
        let mut squared = ((a * a * self.sum_x2)
            + (b * b * self.sum_y2)
            + (c * c * self.count)
            + (2.0 * a * b * self.sum_xy)
            + (2.0 * a * c * self.sum_x)
            + (2.0 * b * c * self.sum_y))
            / self.count;
        if squared < 0.0 {
            squared = 0.0;
        }
        squared.sqrt()
    }
}

fn border_lines(
    sections: &[Section],
    offset_x: i32,
    offset_y: i32,
    center: (f64, f64),
    parameters: BeamStructureParameters,
    side: BorderSide,
) -> Result<Vec<BasicLine>, BeamStructureError> {
    let mut borders = Vec::<(usize, BasicLine, f64)>::new();
    for section in sections {
        if section.bounds().width < parameters.core_section_width {
            continue;
        }
        let mut line = BasicLine::empty();
        for (index, run) in section.runs().iter().enumerate() {
            let x = f64::from(offset_x) + (section.first_pos() + index) as f64;
            let local_y = match side {
                BorderSide::Top => run.start,
                BorderSide::Bottom => run.stop() + 1,
            };
            line.include(x, f64::from(offset_y) + local_y as f64);
        }
        if line.coefficients().is_some() {
            borders.push((section.run_count(), line, 0.0));
        }
    }
    if borders.is_empty() {
        return Err(match side {
            BorderSide::Top => BeamStructureError::NoUsableTopBorder,
            BorderSide::Bottom => BeamStructureError::NoUsableBottomBorder,
        });
    }
    borders.sort_by_key(|entry| Reverse(entry.0));
    let mut weighted_slope = 0.0;
    let mut points = 0.0;
    for (_, line, _) in &borders {
        let slope = line.slope();
        if points != 0.0 && (slope - (weighted_slope / points)).abs() > MAX_SECTION_SLOPE_GAP {
            break;
        }
        points += line.count;
        weighted_slope += line.count * slope;
    }
    let global_slope = weighted_slope / points;
    borders.retain(|(_, line, _)| (line.slope() - global_slope).abs() <= MAX_SECTION_SLOPE_GAP);
    for (_, line, dy) in &mut borders {
        let x = (line.min_x + line.max_x) / 2.0;
        *dy = line.y_at_x(x) - reference_y(center, global_slope, x);
    }
    borders.sort_by(|one, two| one.2.total_cmp(&two.2));
    let delta = parameters.typical_height * MAX_BORDER_JITTER_RATIO;
    let mut groups = Vec::<BasicLine>::new();
    let mut group_weight = 0.0;
    let mut group_sum_dy = 0.0;
    for (_, line, dy) in borders {
        if groups.is_empty() || dy - (group_sum_dy / group_weight) > delta {
            groups.push(BasicLine::empty());
            group_weight = 0.0;
            group_sum_dy = 0.0;
        }
        group_weight += line.count;
        group_sum_dy += line.count * dy;
        groups.last_mut().unwrap().include_line(line);
    }
    groups.retain(|line| {
        line.segment()
            .is_some_and(|segment| segment.x2 - segment.x1 >= parameters.min_hook_width_low)
    });
    if groups.is_empty() {
        return Err(match side {
            BorderSide::Top => BeamStructureError::NoUsableTopBorder,
            BorderSide::Bottom => BeamStructureError::NoUsableBottomBorder,
        });
    }
    Ok(groups)
}

/// Drop entries from the longer border list so the sorted zip pairs each
/// top with the bottom nearest one typical height below it.
fn salvage_border_pairs(
    top: &mut Vec<(f64, Segment)>,
    bottom: &mut Vec<(f64, Segment)>,
    typical_height: f64,
) {
    let (longer, shorter, sign) = if top.len() > bottom.len() {
        (top, bottom, 1.0)
    } else {
        (bottom, top, -1.0)
    };
    while longer.len() > shorter.len() {
        // Removing the entry whose best pairing error is worst leaves the
        // alignment that zips most consistently.
        let cost = |offset: f64, other: &[(f64, Segment)]| -> f64 {
            other
                .iter()
                .map(|(other_offset, _)| {
                    ((other_offset - offset) * sign - typical_height).abs()
                })
                .fold(f64::INFINITY, f64::min)
        };
        let worst = (0..longer.len())
            .max_by(|&a, &b| {
                cost(longer[a].0, shorter)
                    .total_cmp(&cost(longer[b].0, shorter))
            })
            .unwrap();
        longer.remove(worst);
    }
}

fn line_map(
    lines: Vec<BasicLine>,
    center: (f64, f64),
    global_slope: f64,
) -> Result<Vec<(f64, Segment)>, BeamStructureError> {
    lines
        .into_iter()
        .map(|line| {
            let segment = line
                .segment()
                .ok_or(BeamStructureError::UndefinedBorderLine)?;
            let x = (segment.x1 + segment.x2) / 2.0;
            let offset = line.y_at_x(x) - reference_y(center, global_slope, x);
            Ok((offset, segment))
        })
        .collect()
}

fn complete_border_lines(
    y_direction: f64,
    center: (f64, f64),
    global_slope: f64,
    typical_height: f64,
    allow_border_creation: bool,
    min_created_base_width: f64,
    base: &mut Vec<(f64, Segment)>,
    other: &mut Vec<(f64, Segment)>,
    created: &mut Vec<f64>,
) {
    let base = base.clone();
    let base: &[(f64, Segment)] = &base;
    let shift = y_direction * typical_height;
    let tolerance = typical_height * MAX_BORDER_JITTER_RATIO;
    for &(base_offset, base_line) in base {
        let target = base_offset + shift;
        let Some(index) = other
            .iter()
            .position(|(offset, _)| (*offset - target).abs() <= tolerance)
        else {
            if allow_border_creation
                && base_line.x2 - base_line.x1 >= min_created_base_width
            {
                // Java's dormant allowBorderCreation branch: the missing
                // opposite border is the found one, one typical height away.
                // Only a LONG border earns a created opposite: a beam
                // level's border spans the blob, while the short arcs of a
                // fused notehead row do not, and creating opposites for
                // those manufactures credible false beams (measured: the
                // page-9 lone A fixture is the guard).
                created.push(target);
                other.push((
                    target,
                    Segment {
                        x1: base_line.x1,
                        y1: base_line.y1 + shift,
                        x2: base_line.x2,
                        y2: base_line.y2 + shift,
                    },
                ));
            }
            continue;
        };
        let (_, candidate) = other[index];
        let x_mid = (candidate.x1 + candidate.x2) / 2.0;
        let y_mid = (candidate.y1 + candidate.y2) / 2.0;
        let height = y_mid - base_line.y_at_x(x_mid);
        let (x1, y1) = if base_line.x1 < candidate.x1 {
            (base_line.x1, base_line.y1 + height)
        } else {
            (candidate.x1, candidate.y1)
        };
        let (x2, y2) = if base_line.x2 > candidate.x2 {
            (base_line.x2, base_line.y2 + height)
        } else {
            (candidate.x2, candidate.y2)
        };
        let replacement = Segment { x1, y1, x2, y2 };
        let x = (x1 + x2) / 2.0;
        let offset = replacement.y_at_x(x) - reference_y(center, global_slope, x);
        other[index] = (offset, replacement);
    }
}

fn retrieve_items(
    sections: &[Section],
    offset_x: i32,
    offset_y: i32,
    median: Segment,
    height: f64,
    max_gap: i32,
) -> Vec<BeamItem> {
    let mut items = Vec::new();
    let mut start = None::<i32>;
    let mut stop = None::<i32>;
    for section in sections {
        let bounds = section.bounds();
        let left = offset_x + bounds.x as i32;
        let right_after = left + bounds.width as i32;
        let center_x = f64::from(left) + bounds.width as f64 / 2.0;
        let y = median.y_at_x(center_x) - f64::from(offset_y);
        if !section_contains(section, center_x - f64::from(offset_x), y) {
            continue;
        }
        if let Some(current_stop) = stop {
            if left - current_stop > max_gap {
                items.push(item_from_span(median, height, start.unwrap(), current_stop));
                start = Some(left);
            }
            stop = Some(current_stop.max(right_after));
        } else {
            start = Some(left);
            stop = Some(right_after);
        }
    }
    if let (Some(start), Some(stop)) = (start, stop) {
        items.push(item_from_span(median, height, start, stop));
    }
    items
}

fn section_contains(section: &Section, x: f64, y: f64) -> bool {
    let position = x.floor() as usize;
    if position < section.first_pos() || position > section.last_pos() {
        return false;
    }
    let run = section.runs()[position - section.first_pos()];
    y >= run.start as f64 && y < (run.stop() + 1) as f64
}

fn item_from_span(median: Segment, height: f64, start: i32, stop: i32) -> BeamItem {
    BeamItem {
        median: Segment {
            x1: f64::from(start),
            y1: median.y_at_x(f64::from(start)),
            x2: f64::from(stop),
            y2: median.y_at_x(f64::from(stop)),
        },
        height,
    }
}

fn glyph_centroid(glyph: &RunTable, offset_x: i32, offset_y: i32) -> (f64, f64) {
    let mut weight = 0_usize;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    for x in 0..glyph.width() {
        for run in glyph.sequence(x).unwrap_or_default() {
            weight += run.length;
            sum_x += (f64::from(offset_x) + x as f64) * run.length as f64;
            sum_y +=
                ((run.start + run.stop()) as f64 / 2.0 + f64::from(offset_y)) * run.length as f64;
        }
    }
    (sum_x / weight as f64, sum_y / weight as f64)
}

/// Java `LineUtil.yAtX(Point2D p1, double slope, double x)`.
///
/// Java does not evaluate the point-slope form either. It manufactures a second
/// point a thousand units along -- `(x1 + 1000, y1 + 1000 * slope)` -- and
/// intersects, so the multiplication by a thousand and the division back out
/// are both in the answer's last bits.
fn reference_y(center: (f64, f64), slope: f64, x: f64) -> f64 {
    intersection_at_x_from_slope(center, slope, x).1
}

/// Java `LineUtil.intersectionAtX(Point2D p1, double slope, double x)`.
///
/// Returns both coordinates, because the abscissa is not exactly `x`: Java
/// divides one rounded product by another, and the quotient is only
/// algebraically the query abscissa. Callers that use `p.getX()` get that
/// value, not their own.
#[must_use]
pub fn intersection_at_x_from_slope(point: (f64, f64), slope: f64, x: f64) -> (f64, f64) {
    line_util_intersection(
        point.0,
        point.1,
        point.0 + 1_000.0,
        point.1 + (1_000.0 * slope),
        x,
    )
}

/// Java `LineUtil.intersection`, specialised to the vertical query line every
/// `yAtX` and `intersectionAtX` in `LineUtil` uses: `(x, 0)` to `(x, 1000)`.
#[must_use]
pub fn line_util_intersection(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> (f64, f64) {
    let (x3, y3, x4, y4) = (x, 0.0, x, 1_000.0);
    let den = ((x1 - x2) * (y3 - y4)) - ((y1 - y2) * (x3 - x4));
    let v12 = (x1 * y2) - (y1 * x2);
    let v34 = (x3 * y4) - (y3 * x4);
    (
        ((v12 * (x3 - x4)) - ((x1 - x2) * v34)) / den,
        ((v12 * (y3 - y4)) - ((y1 - y2) * v34)) / den,
    )
}

/// Java `GradeUtil.clamp`: every impact is squeezed into `[0, 1]`.
///
/// `GradeImpacts.setImpact` applies this to each term before any grade is
/// taken. A width impact of 1.79 is entirely normal -- it happens whenever an
/// item is wider than the *hook* thresholds expect -- and leaving it unclamped
/// inflates the geometric mean.
#[must_use]
pub fn clamp_impact(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

/// Applies Java's clamp to all six terms, as `Impacts`' constructor does.
#[must_use]
pub fn clamped(impacts: BeamImpacts) -> BeamImpacts {
    BeamImpacts {
        width: clamp_impact(impacts.width),
        min_height: clamp_impact(impacts.min_height),
        max_height: clamp_impact(impacts.max_height),
        core: clamp_impact(impacts.core),
        belt: clamp_impact(impacts.belt),
        distance: clamp_impact(impacts.distance),
        raster: impacts.raster,
    }
}

/// `GradeImpacts.getGrade` for a beam: the weighted geometric mean, clamped
/// term by term first, times `Grades.intrinsicRatio`.
///
/// The clamp is part of this and not the caller's business. Three copies of
/// this function existed without it, which made every grade above a saturating
/// term plausible and too high.
#[must_use]
pub fn beam_grade(impacts: BeamImpacts) -> f64 {
    const WEIGHTS: [f64; 6] = [0.5, 1.0, 1.0, 2.0, 2.0, 2.0];
    let impacts = clamped(impacts);
    let values = [
        impacts.width,
        impacts.min_height,
        impacts.max_height,
        impacts.core,
        impacts.belt,
        impacts.distance,
    ];
    let mut product = 1.0;
    let mut total = 0.0;
    for (value, weight) in values.into_iter().zip(WEIGHTS) {
        total += weight;
        if value == 0.0 {
            product = 0.0;
        } else if weight != 0.0 {
            product *= java_positive_pow(value, weight);
        }
    }
    0.8 * java_positive_pow(product, 1.0 / total)
}

/// Which border of a beam line the jitter is measured against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JitterSide {
    Top,
    Bottom,
}

/// Java `BeamStructure.computeJitter`: how ragged one border of a line is.
///
/// This becomes the sixth beam impact, `jit`, and Java computes it once per
/// structure from the outermost lines rather than once per item. It is a
/// least-squares fit through one endpoint of every run in every section whose
/// centre lies on the median, divided by the glyph's width, so a straight
/// border scores near zero and a stepped one does not.
///
/// Two details are Java's and neither is what a rewrite would pick. The section
/// centre here is the *integer* one -- `GeoUtil.center`, with integer division,
/// where `retrieveItems` uses the `double` `center2D` -- and the abscissa
/// window is inset by the corner margin at both ends, so a beam's own tapered
/// corners do not count against it.
///
/// # Panics
///
/// Panics if the run table is not the vertical spot raster.
#[must_use]
pub fn compute_jitter(
    glyph: &RunTable,
    offset_x: i32,
    offset_y: i32,
    median: Segment,
    side: JitterSide,
    corner_margin: f64,
) -> f64 {
    let points = jitter_border_points(glyph, offset_x, offset_y, median, side, corner_margin);
    let mut line = audiveris_core::basic_line::BasicLine::default();
    for &(x, y) in &points {
        line.include_point(x, y);
    }
    line.mean_distance().unwrap_or(0.0) / glyph.width() as f64
}

/// [`compute_jitter`] with fused-ink endpoints trimmed out (enhancement).
///
/// The plain jitter fits one line through every border run endpoint, so the
/// endpoints of noteheads and stem stubs the morphological closing fused onto
/// a beam dominate the residual -- and dominate a fresh fit too, so trimming
/// against one's own fit removes nothing (measured: 135 of 136 items still
/// zeroed). The trim reference must come from outside the contaminated
/// population: `reference` is the border the structure analysis itself
/// placed (line median shifted by half the line height), and `tolerance` is
/// the sheet's `max_distance_to_border` -- the scale Java itself uses for
/// "on the border" when linking stems. Endpoints farther than that from the
/// analysed border are fused-object ink, not border; the survivors get their
/// own least-squares fit and the usual residual-over-width ratio. Falls back
/// to the untrimmed value when fewer than two points survive.
#[must_use]
pub fn compute_jitter_trimmed(
    glyph: &RunTable,
    offset_x: i32,
    offset_y: i32,
    median: Segment,
    side: JitterSide,
    corner_margin: f64,
    reference: Segment,
    tolerance: f64,
) -> f64 {
    let points = jitter_border_points(glyph, offset_x, offset_y, median, side, corner_margin);
    let mut line = audiveris_core::basic_line::BasicLine::default();
    for &(x, y) in &points {
        line.include_point(x, y);
    }
    let untrimmed = line.mean_distance().unwrap_or(0.0) / glyph.width() as f64;
    let mut trimmed = audiveris_core::basic_line::BasicLine::default();
    for &(x, y) in &points {
        if (y - reference.y_at_x(x)).abs() <= tolerance {
            trimmed.include_point(x, y);
        }
    }
    match trimmed.mean_distance() {
        Ok(distance) if trimmed.count() >= 2 => distance / glyph.width() as f64,
        _ => untrimmed,
    }
}

/// The border run endpoints both jitter measures fit through.
fn jitter_border_points(
    glyph: &RunTable,
    offset_x: i32,
    offset_y: i32,
    median: Segment,
    side: JitterSide,
    corner_margin: f64,
) -> Vec<(f64, f64)> {
    assert_eq!(
        glyph.orientation(),
        Orientation::Vertical,
        "jitter needs the vertical spot raster"
    );
    let sections = build_sections(glyph, JunctionPolicy::DEFAULT_RATIO);

    let dx = corner_margin.round_ties_even() as i32;
    let x1 = (median.x1 + f64::from(dx)).round_ties_even() as i32;
    let x2 = (median.x2 - f64::from(dx)).round_ties_even() as i32;

    let mut points = Vec::new();
    for section in &sections {
        let bounds = section.bounds();
        let center_x = offset_x + bounds.x as i32 + (bounds.width as i32) / 2;
        let y = median.y_at_x(f64::from(center_x)).round_ties_even() as i32;

        if !section_contains(
            section,
            f64::from(center_x - offset_x),
            f64::from(y - offset_y),
        ) {
            continue;
        }

        // Java walks a counter alongside the runs because a section's runs are
        // consecutive positions from `firstPos`; enumerate says the same thing.
        let first = offset_x + section.first_pos() as i32;
        for (index, run) in section.runs().iter().enumerate() {
            let x = first + index as i32;
            if x >= x1 && x <= x2 {
                let end = match side {
                    JitterSide::Top => run.start,
                    JitterSide::Bottom => run.stop(),
                };
                points.push((f64::from(x), f64::from(offset_y + end as i32)));
            }
        }
    }
    points
}

/// Java `LineUtil.yAtX(Line2D, double)`, term for term.
///
/// Not the point-slope form, which is what it looks like it should be and what
/// this used to be. Java computes a general line-line intersection against the
/// vertical segment from `(x, 0)` to `(x, 1000)`, via a determinant:
///
/// ```text
/// den = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4)
/// v12 = x1 * y2 - y1 * x2
/// v34 = x3 * y4 - y3 * x4
/// y   = (v12 * (y3 - y4) - (y1 - y2) * v34) / den
/// ```
///
/// The two forms are algebraically identical and differ in the last bits, and
/// the last bits are load-bearing here. A beam item's left edge is decided by
/// whether a section's centre lies *on* the median, and that containment test
/// is half-open: for one spot on chula the two forms give 924.9999999999998 and
/// 925.0 for the same query, a run ends at 924, and so the item starts a whole
/// section late. Three other medians differed in their ninth decimal for the
/// same reason.
fn line_util_y_at_x(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    line_util_intersection(x1, y1, x2, y2, x).1
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamImpactParameters {
    pub belt_margin_dx: i32,
    pub belt_margin_dy: i32,
    pub min_core_black_ratio: f64,
    pub max_belt_black_ratio: f64,
    pub min_width_low: f64,
    pub min_width_high: f64,
    pub min_height_low: f64,
    pub typical_height: f64,
    pub max_height_high: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BeamBeltSides {
    pub above: bool,
    pub below: bool,
    /// Fused-context exemption: sample no belt at all, take belt impact 1.0.
    /// In a dense run the ink continues into the neighboring symbol on every
    /// side, so any belt sample measures fusion, not separation. False
    /// everywhere Java's behavior applies.
    pub neutral: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct BeamRaster<'a> {
    pub table: &'a RunTable,
    pub offset_x: i32,
    pub offset_y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamRasterEvidence {
    pub core_foreground: usize,
    pub core_count: usize,
    pub belt_foreground: usize,
    pub belt_count: usize,
    pub core_ratio: f64,
    pub belt_ratio: f64,
    pub rounded_width: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamImpacts {
    pub width: f64,
    pub min_height: f64,
    pub max_height: f64,
    pub core: f64,
    pub belt: f64,
    pub distance: f64,
    pub raster: BeamRasterEvidence,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BeamImpactRejection {
    Width,
    HeightBelow,
    HeightAbove,
    CoreRatio(BeamRasterEvidence),
    BeltRatio(BeamRasterEvidence),
}

/// Java `AreaMask` sampling and `BeamsBuilder.computeImpacts`. Absolute mask
/// points outside the raster still count in the denominator and are background.
pub fn compute_beam_impacts(
    item: BeamItem,
    sides: BeamBeltSides,
    raster: BeamRaster<'_>,
    distance_impact: f64,
    parameters: BeamImpactParameters,
) -> Result<BeamImpacts, BeamImpactRejection> {
    let core = Parallelogram::new(item.median, item.height);
    let top = if sides.above {
        parameters.belt_margin_dy
    } else {
        0
    };
    let bottom = if sides.below {
        parameters.belt_margin_dy
    } else {
        0
    };
    let shift_y = f64::from(bottom - top) / 2.0;
    let x1 = item.median.x1 - f64::from(parameters.belt_margin_dx);
    let x2 = item.median.x2 + f64::from(parameters.belt_margin_dx);
    let belt_median = Segment {
        x1,
        y1: item.median.y_at_x(x1) + shift_y,
        x2,
        y2: item.median.y_at_x(x2) + shift_y,
    };
    let outer = Parallelogram::new(belt_median, item.height + f64::from(top + bottom));
    let (core_foreground, core_count) =
        sample_mask(core, None, raster.table, raster.offset_x, raster.offset_y);
    let (belt_foreground, belt_count) = if sides.neutral {
        (0, 0)
    } else {
        sample_mask(
            outer,
            Some(core),
            raster.table,
            raster.offset_x,
            raster.offset_y,
        )
    };
    // Java double division intentionally yields NaN for a zero-area mask.
    let core_ratio = core_foreground as f64 / core_count as f64;
    let belt_ratio = if sides.neutral {
        0.0
    } else {
        belt_foreground as f64 / belt_count as f64
    };
    let rounded_width = (item.median.x2 - item.median.x1 + 1.0).round_ties_even() as i32;
    let raster_evidence = BeamRasterEvidence {
        core_foreground,
        core_count,
        belt_foreground,
        belt_count,
        core_ratio,
        belt_ratio,
        rounded_width,
    };
    if f64::from(rounded_width) < parameters.min_width_low {
        return Err(BeamImpactRejection::Width);
    }
    if item.height < parameters.min_height_low {
        return Err(BeamImpactRejection::HeightBelow);
    }
    if item.height > parameters.max_height_high {
        return Err(BeamImpactRejection::HeightAbove);
    }
    if core_ratio < parameters.min_core_black_ratio {
        return Err(BeamImpactRejection::CoreRatio(raster_evidence));
    }
    if belt_ratio > parameters.max_belt_black_ratio {
        return Err(BeamImpactRejection::BeltRatio(raster_evidence));
    }
    Ok(BeamImpacts {
        width: (f64::from(rounded_width) - parameters.min_width_low)
            / (parameters.min_width_high - parameters.min_width_low),
        min_height: (item.height - parameters.min_height_low)
            / (parameters.typical_height - parameters.min_height_low),
        max_height: (parameters.max_height_high - item.height)
            / (parameters.max_height_high - parameters.typical_height),
        core: (core_ratio - parameters.min_core_black_ratio)
            / (1.0 - parameters.min_core_black_ratio),
        belt: 1.0 - (belt_ratio / parameters.max_belt_black_ratio),
        distance: distance_impact,
        raster: raster_evidence,
    })
}

#[derive(Clone, Copy)]
struct Parallelogram {
    median: Segment,
    half_height: f64,
}

impl Parallelogram {
    fn new(median: Segment, height: f64) -> Self {
        Self {
            median,
            half_height: height / 2.0,
        }
    }

    /// `java.awt.geom.Area.contains(x, y)` for this parallelogram.
    ///
    /// Even-odd crossings, which is what Java2D's `Crossings` computes, and it
    /// is not the same as testing the point against the two sloped edges as
    /// functions of `x`. Each edge is taken over a half-open range in `y`, and
    /// the crossing abscissa is interpolated as **x at y**. Asking instead for
    /// y at x -- the obvious formulation, and what this used to do -- disagrees
    /// wherever an edge passes exactly through a pixel centre, and disagrees in
    /// *both* directions: on one beam Java included the point and on another it
    /// excluded an identical-looking one, which is why no open/closed
    /// convention could be made to fit. Measured over all 194 beams of
    /// BachInvention5, this rule reproduces Java exactly and each of the four
    /// obvious conventions gets one wrong.
    fn contains(self, x: f64, y: f64) -> bool {
        let corners = self.corners();
        let mut crossings = 0;
        for index in 0..4 {
            let (ax, ay) = corners[index];
            let (bx, by) = corners[(index + 1) % 4];
            if (ay <= y) == (by <= y) {
                continue;
            }
            if ax + (y - ay) * ((bx - ax) / (by - ay)) > x {
                crossings += 1;
            }
        }
        crossings % 2 == 1
    }

    /// The four defining points, in `AreaUtil.horizontalParallelogramPath`
    /// order: upper left, upper right, lower right, lower left.
    fn corners(self) -> [(f64, f64); 4] {
        [
            (self.median.x1, self.median.y1 - self.half_height),
            (self.median.x2, self.median.y2 - self.half_height),
            (self.median.x2, self.median.y2 + self.half_height),
            (self.median.x1, self.median.y1 + self.half_height),
        ]
    }

    fn integer_bounds(self) -> (i32, i32, i32, i32) {
        let min_y = (self.median.y1.min(self.median.y2) - self.half_height).floor() as i32;
        let max_y = (self.median.y1.max(self.median.y2) + self.half_height).ceil() as i32;
        (
            self.median.x1.floor() as i32,
            min_y,
            self.median.x2.ceil() as i32,
            max_y,
        )
    }
}

fn sample_mask(
    area: Parallelogram,
    subtract: Option<Parallelogram>,
    raster: &RunTable,
    offset_x: i32,
    offset_y: i32,
) -> (usize, usize) {
    let (left, top, right, bottom) = area.integer_bounds();
    let mut foreground = 0;
    let mut count = 0;
    for y in top..bottom {
        for x in left..right {
            if !area.contains(f64::from(x), f64::from(y))
                || subtract.is_some_and(|core| core.contains(f64::from(x), f64::from(y)))
            {
                continue;
            }
            count += 1;
            let local_x = x - offset_x;
            let local_y = y - offset_y;
            if local_x >= 0
                && local_y >= 0
                && (local_x as usize) < raster.width()
                && (local_y as usize) < raster.height()
                && raster.get(local_x as usize, local_y as usize) == FOREGROUND
            {
                foreground += 1;
            }
        }
    }
    (foreground, count)
}

/// Count Java `AreaMask` integer samples in a beam-shaped parallelogram.
/// This is shared by the post-classification extension kernel for middle and
/// side core-ratio checks.
#[must_use]
pub fn sample_beam_core(item: BeamItem, raster: BeamRaster<'_>) -> (usize, usize) {
    sample_mask(
        Parallelogram::new(item.median, item.height),
        None,
        raster.table,
        raster.offset_x,
        raster.offset_y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_table::{BACKGROUND, Run};

    fn vertical_table(width: usize, height: usize, columns: &[Run]) -> RunTable {
        assert_eq!(width, columns.len());
        let mut table = RunTable::new(Orientation::Vertical, width, height).unwrap();
        for (x, run) in columns.iter().copied().enumerate() {
            table.add_run(x, run).unwrap();
        }
        table
    }

    fn structure_parameters() -> BeamStructureParameters {
        BeamStructureParameters {
            typical_height: 4.0,
            core_section_width: 2,
            min_hook_width_low: 2.0,
            max_item_x_gap: 1,
            min_beam_width_low: 3.0,
            max_hook_width: 7.0,
            allow_border_creation: false,
        }
    }

    #[test]
    fn parallel_borders_form_one_exact_item_with_exclusive_bottom_border() {
        let glyph = vertical_table(8, 9, &[Run::new(2, 4); 8]);
        let analysis = analyze_beam_structure(&glyph, 10, 20, structure_parameters()).unwrap();
        assert_eq!(analysis.lines.len(), 1);
        let line = &analysis.lines[0];
        assert_eq!(
            line.median,
            Segment {
                x1: 10.0,
                y1: 24.0,
                x2: 17.0,
                y2: 24.0
            }
        );
        assert_eq!(line.height, 4.0);
        assert_eq!(
            line.items,
            vec![BeamItem {
                median: Segment {
                    x1: 10.0,
                    y1: 24.0,
                    x2: 18.0,
                    y2: 24.0
                },
                height: 4.0
            }]
        );
        assert_eq!(analysis.global_distance, 0.0);
        assert_eq!(analysis.mean_thickness, 4.0);
    }

    #[test]
    fn border_groups_recover_two_beams_and_item_gap_is_strictly_greater() {
        let mut glyph = RunTable::new(Orientation::Vertical, 9, 16).unwrap();
        for x in 0..4 {
            glyph.add_run(x, Run::new(1, 4)).unwrap();
            glyph.add_run(x, Run::new(9, 4)).unwrap();
        }
        for x in 6..9 {
            glyph.add_run(x, Run::new(1, 4)).unwrap();
            glyph.add_run(x, Run::new(9, 4)).unwrap();
        }
        let analysis = analyze_beam_structure(&glyph, 0, 0, structure_parameters()).unwrap();
        assert_eq!(analysis.lines.len(), 2);
        assert_eq!(analysis.lines[0].items.len(), 2); // gap 2 is strictly greater than max gap 1
        assert_eq!(analysis.lines[1].height, 4.0);
    }

    #[test]
    fn split_uses_ties_even_and_preserves_java_empty_item_behavior() {
        let line = BeamLine {
            median: Segment {
                x1: 1.0,
                y1: 6.0,
                x2: 9.0,
                y2: 6.0,
            },
            height: 8.0,
            items: vec![],
        };
        let mut even = BeamStructureAnalysis {
            envelope: false,
            synthetic_medians: Vec::new(),
            lines: vec![line.clone()],
            global_distance: 0.0,
            mean_thickness: 6.0,
        };
        even.split_stuck_lines(4.0); // 1.5 ties to even 2
        assert_eq!(even.lines.len(), 2);
        assert_eq!(even.lines[0].height, 4.0);
        assert_eq!(even.lines[0].median.y1, 4.0);
        assert!(even.lines.iter().all(|line| line.items.is_empty()));
        let mut odd = BeamStructureAnalysis {
            envelope: false,
            synthetic_medians: Vec::new(),
            lines: vec![line],
            global_distance: 0.0,
            mean_thickness: 5.999,
        };
        odd.split_stuck_lines(4.0);
        assert_eq!(odd.lines.len(), 1);
    }

    fn impact_parameters() -> BeamImpactParameters {
        BeamImpactParameters {
            belt_margin_dx: 1,
            belt_margin_dy: 1,
            min_core_black_ratio: 0.75,
            max_belt_black_ratio: 0.25,
            min_width_low: 4.0,
            min_width_high: 8.0,
            min_height_low: 2.0,
            typical_height: 4.0,
            max_height_high: 6.0,
        }
    }

    #[test]
    fn core_and_belt_sample_absolute_integer_points_and_outside_is_background() {
        let mut pixels = vec![BACKGROUND; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                pixels[y * 8 + x] = FOREGROUND;
            }
        }
        let raster = RunTable::from_pixels(Orientation::Horizontal, 8, 8, &pixels).unwrap();
        let item = BeamItem {
            median: Segment {
                x1: 2.0,
                y1: 4.0,
                x2: 6.0,
                y2: 4.0,
            },
            height: 4.0,
        };
        let impacts = compute_beam_impacts(
            item,
            BeamBeltSides {
                above: true,
                below: true,
                            neutral: false,
            },
            BeamRaster {
                table: &raster,
                offset_x: 0,
                offset_y: 0,
            },
            0.8,
            impact_parameters(),
        )
        .unwrap();
        assert_eq!(
            (impacts.raster.core_foreground, impacts.raster.core_count),
            (16, 16)
        );
        assert_eq!(
            (impacts.raster.belt_foreground, impacts.raster.belt_count),
            (0, 20)
        );
        assert_eq!(impacts.raster.rounded_width, 5);
        assert_eq!(impacts.width, 0.25);
        assert_eq!(impacts.core, 1.0);
        assert_eq!(impacts.belt, 1.0);
        assert_eq!(impacts.distance, 0.8);
    }

    #[test]
    fn one_sided_belt_and_rejection_order_match_java() {
        let raster = vertical_table(4, 4, &[Run::new(0, 4); 4]);
        let short = BeamItem {
            median: Segment {
                x1: 0.0,
                y1: 2.0,
                x2: 2.4,
                y2: 2.0,
            },
            height: 1.0,
        };
        assert_eq!(
            compute_beam_impacts(
                short,
                BeamBeltSides {
                    above: true,
                    below: false,
                                    neutral: false,
                },
                BeamRaster {
                    table: &raster,
                    offset_x: 0,
                    offset_y: 0,
                },
                0.0,
                impact_parameters()
            ),
            Err(BeamImpactRejection::Width)
        );
        let low_core = BeamItem {
            median: Segment {
                x1: 0.0,
                y1: 8.0,
                x2: 4.0,
                y2: 8.0,
            },
            height: 4.0,
        };
        assert!(matches!(
            compute_beam_impacts(
                low_core,
                BeamBeltSides {
                    above: false,
                    below: false,
                                    neutral: false,
                },
                BeamRaster {
                    table: &raster,
                    offset_x: 0,
                    offset_y: 0,
                },
                0.0,
                impact_parameters()
            ),
            Err(BeamImpactRejection::CoreRatio(_))
        ));
    }

    #[test]
    fn non_vertical_structure_input_fails_closed() {
        let raster =
            RunTable::from_pixels(Orientation::Horizontal, 2, 2, &[FOREGROUND; 4]).unwrap();
        assert_eq!(
            analyze_beam_structure(&raster, 0, 0, structure_parameters()),
            Err(BeamStructureError::NonVerticalRunTable)
        );
    }
}
