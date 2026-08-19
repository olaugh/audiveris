// SPDX-License-Identifier: AGPL-3.0-or-later

//! Grayscale barline tuning engine — an enhancement beyond Java parity.
//!
//! Port of the paired-staff barline hypothesis layer prototyped in
//! `stage-omr-data/scripts/tune_sonata_barlines.py` (the Python script is the
//! oracle for this module; see `oracle/py-barline-tuning/`).  The engine
//! re-scores vertical ink over complete piano systems and produces a tuned
//! boundary hypothesis *next to* the accepted GRID output — raw boundaries are
//! never mutated, mirroring the Python design ("build count-constrained
//! barline hypotheses without turning them into truth").
//!
//! Pure decision half: no I/O, no environment variables.  Everything an
//! invocation needs arrives through [`BarTuningParameters`],
//! [`SystemBarInput`], and a [`GrayRaster`].

use crate::ingest::GrayRaster;

/// Resolved thresholds for one tuning invocation.
///
/// Two constructors: [`BarTuningParameters::python_reference`] freezes the
/// exact pixel constants of the Python oracle (used by the ported unit tests
/// and the pixel-mode parity run), and [`BarTuningParameters::from_scale`]
/// derives the pixel values from the sheet interline so the same geometry
/// transfers across scan resolutions.
#[derive(Debug, Clone, PartialEq)]
pub struct BarTuningParameters {
    /// Ink luminance cut: a pixel is ink when `255 - gray > ink_threshold`.
    /// Photometric, never scaled.
    pub ink_threshold: u8,
    /// Horizontal scan starts `scan_margin` inside the system edges.
    pub scan_margin: usize,
    /// Lower floor of the per-staff band height (`max(floor, 0.43 * h)`).
    pub band_floor: usize,
    /// Band height as a ratio of system height.
    pub band_ratio: f64,
    /// Lower floor of the sliding occupancy window (`max(floor, 0.25 * h)`).
    pub span_floor: usize,
    /// Occupancy window as a ratio of system height.
    pub span_ratio: f64,
    /// The inter-staff bridge overlaps this far into each staff band.
    pub bridge_overlap: usize,
    /// Hard filter: candidates need `paired >= paired_cut`.
    pub paired_cut: f64,
    /// Hard filter: candidates need `bridge >= bridge_cut`.
    pub bridge_cut: f64,
    /// Local-maximum test radius, in columns.
    pub local_max_radius: usize,
    /// Peaks within this distance merge into one candidate.
    pub cluster_merge: f64,
    /// Flank analysis: half-width of the wide-row (staff line) neighborhood.
    pub flank_neighborhood: usize,
    /// Rows inked across at least this fraction of the neighborhood are
    /// horizontal structure (staff lines, beams) and carry no flank signal.
    pub flank_wide_row_cut: f64,
    /// Distances from the candidate at which flank ink is sampled.
    pub flank_distances: [usize; 3],
    /// A non-certain projection-only candidate whose cleanest flank exceeds
    /// this is an aligned-stem column and can never be drafted.
    pub flank_noise_limit: f64,
    /// Certain-connector predicate: minimum paired occupancy.
    pub certain_paired: f64,
    /// Certain-connector predicate: minimum mean occupancy.
    pub certain_mean: f64,
    /// Certain-connector predicate: minimum bridge occupancy.
    pub certain_bridge: f64,
    /// Certain-connector predicate: minimum full-height occupancy.
    pub certain_full: f64,
    /// Certain-connector predicate: maximum inkless run, in pixels.
    pub certain_max_gap: usize,
    /// Clef/key/time prefix: projection-only candidates are excluded before
    /// `left + max(signature_guard_floor, signature_guard_ratio * h)`.
    pub signature_guard_floor: f64,
    /// Height ratio of the signature guard.
    pub signature_guard_ratio: f64,
    /// A projection corroborates a weak raw bar within this distance.
    pub corroboration_proximity: f64,
    /// Corroboration: minimum paired occupancy.
    pub corroboration_paired: f64,
    /// Corroboration: minimum full-height occupancy.
    pub corroboration_full: f64,
    /// Corroboration: maximum inkless run at reference scale.
    pub corroboration_max_gap: usize,
    /// Taller systems open proportionally longer gaps on genuine bars; the
    /// gap limit may grow to this fraction of system height when the
    /// candidate's flanks are clean.
    pub tall_gap_height_ratio: f64,
    /// A raw bar is strong when `support >= 2` and `grade >= strong_grade`.
    pub strong_grade: f64,
    /// The thin stroke of a final/repeat double bar sits within this margin
    /// of the system edge and duplicates the edge barline.
    pub edge_bar_margin: f64,
    /// Geometry counting merges bars within this distance.
    pub geometry_merge: f64,
    /// Boundary selection merges candidates within this distance.
    pub select_merge: f64,
    /// Selection drops candidates within `edge_bar_margin` of the edges and
    /// enforces `max(dp_min_gap_floor, dp_min_gap_ratio * average)` spacing.
    pub dp_min_gap_floor: f64,
    /// Spacing floor as a ratio of the average measure width.
    pub dp_min_gap_ratio: f64,
    /// Weight of the log-spacing prior.
    pub spacing_weight: f64,
    /// Floor of the gap/average ratio inside the spacing prior.
    pub spacing_floor: f64,
    /// Evidence of a strong raw bar or a certain connector.
    pub strong_evidence: f64,
    /// Evidence of a weak raw bar.
    pub weak_rust_evidence: f64,
    /// Weak projection evidence: `base + paired_weight * paired +
    /// bridge_weight * bridge`.
    pub weak_projection_base: f64,
    /// See [`BarTuningParameters::weak_projection_base`].
    pub weak_projection_paired_weight: f64,
    /// See [`BarTuningParameters::weak_projection_base`].
    pub weak_projection_bridge_weight: f64,
    /// Evidence of a printed-number anchor.
    pub anchor_evidence: f64,
}

/// Java `(int) Math.rint(value)`: round half to even.
fn rint(value: f64) -> i64 {
    value.round_ties_even() as i64
}

fn rint_usize(value: f64) -> usize {
    rint(value).max(0) as usize
}

impl BarTuningParameters {
    /// The exact pixel constants of the Python oracle
    /// (`tune_sonata_barlines.py`), frozen for parity runs and unit tests.
    #[must_use]
    pub fn python_reference() -> Self {
        Self {
            ink_threshold: 82,
            scan_margin: 3,
            band_floor: 12,
            band_ratio: 0.43,
            span_floor: 12,
            span_ratio: 0.25,
            bridge_overlap: 3,
            paired_cut: 0.70,
            bridge_cut: 0.55,
            local_max_radius: 2,
            cluster_merge: 4.0,
            flank_neighborhood: 15,
            flank_wide_row_cut: 0.85,
            flank_distances: [3, 4, 5],
            flank_noise_limit: 0.12,
            certain_paired: 0.94,
            certain_mean: 0.94,
            certain_bridge: 0.90,
            certain_full: 0.95,
            certain_max_gap: 2,
            signature_guard_floor: 48.0,
            signature_guard_ratio: 0.90,
            corroboration_proximity: 4.0,
            corroboration_paired: 0.85,
            corroboration_full: 0.80,
            corroboration_max_gap: 8,
            tall_gap_height_ratio: 0.05,
            strong_grade: 0.80,
            edge_bar_margin: 8.0,
            geometry_merge: 6.0,
            select_merge: 5.0,
            dp_min_gap_floor: 8.0,
            dp_min_gap_ratio: 0.18,
            spacing_weight: 0.55,
            spacing_floor: 0.15,
            strong_evidence: 50.0,
            weak_rust_evidence: 3.0,
            weak_projection_base: 0.5,
            weak_projection_paired_weight: 1.6,
            weak_projection_bridge_weight: 1.2,
            anchor_evidence: 4.2,
        }
    }

    /// Derives the pixel thresholds from the sheet interline.
    ///
    /// Coefficients are chosen so every expression reproduces the Python
    /// reference exactly at the calibration interline of 6 px (the Schenker
    /// Universal Edition scans); at other scales the geometry follows the
    /// engraving rather than the pixel grid.  Ratios and the photometric ink
    /// cut are never scaled.
    #[must_use]
    pub fn from_scale(interline: f64) -> Self {
        let reference = Self::python_reference();
        Self {
            // 3 px = rint(0.5 * I) at I = 6.
            scan_margin: rint_usize(0.5 * interline),
            // 12 px = rint(2.0 * I) at I = 6.
            band_floor: rint_usize(2.0 * interline),
            span_floor: rint_usize(2.0 * interline),
            // 3 px = rint(0.5 * I).
            bridge_overlap: rint_usize(0.5 * interline),
            // 2 px = max(2, rint(I / 3)).
            local_max_radius: rint_usize(interline / 3.0).max(2),
            // 4 px = rint(0.67 * I).
            cluster_merge: (0.67 * interline).round_ties_even(),
            // 15 px = rint(2.5 * I).
            flank_neighborhood: rint_usize(2.5 * interline),
            // (3, 4, 5) px = rint((0.5, 0.67, 0.83) * I).
            flank_distances: [
                rint_usize(0.5 * interline).max(2),
                rint_usize(0.67 * interline).max(3),
                rint_usize(0.83 * interline).max(4),
            ],
            // 2 px = max(2, rint(I / 3)).
            certain_max_gap: rint_usize(interline / 3.0).max(2),
            // 48 px = rint(8 * I).
            signature_guard_floor: 8.0 * interline,
            // 4 px = rint(0.67 * I).
            corroboration_proximity: (0.67 * interline).round_ties_even(),
            // 8 px = rint(1.33 * I).
            corroboration_max_gap: rint_usize(1.33 * interline),
            // 8 px = rint(1.33 * I).
            edge_bar_margin: (1.33 * interline).round_ties_even(),
            // 6 px = rint(1.0 * I).
            geometry_merge: interline.round_ties_even(),
            // 5 px = rint(0.83 * I).
            select_merge: (0.83 * interline).round_ties_even(),
            // 8 px = rint(1.33 * I).
            dp_min_gap_floor: (1.33 * interline).round_ties_even(),
            ..reference
        }
    }
}

/// The pixel-space band of one recognized system (both piano staves).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemBand {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

impl SystemBand {
    #[must_use]
    pub fn height(&self) -> f64 {
        (self.bottom - self.top).max(1.0)
    }
}

/// Width class of an accepted GRID boundary, when known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawBoundaryKind {
    Thin,
    Thick,
    /// Synthetic entry standing in for a system edge.
    Edge,
    Unknown,
}

/// One accepted GRID boundary (or synthetic system edge).
///
/// `boundaries[0]` and `boundaries[last]` are always the system edges,
/// mirroring the Python raw schema whose interior slice is `[1..len-1]`.
#[derive(Debug, Clone, PartialEq)]
pub struct RawBoundary {
    pub x: f64,
    /// Number of staves supporting the bar (2 = both piano staves).
    pub support: u32,
    /// Best grade across the supporting staves.
    pub max_grade: f64,
    pub kind: RawBoundaryKind,
}

impl RawBoundary {
    fn is_strong(&self, parameters: &BarTuningParameters) -> bool {
        self.support >= 2 && self.max_grade >= parameters.strong_grade
    }
}

/// Everything the engine needs to know about one system.
#[derive(Debug, Clone, PartialEq)]
pub struct SystemBarInput {
    pub band: SystemBand,
    pub boundaries: Vec<RawBoundary>,
}

/// A clustered vertical-ink candidate with its full measurement record.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionCandidate {
    pub x: f64,
    pub paired_occupancy: f64,
    pub mean_occupancy: f64,
    pub bridge_occupancy: f64,
    pub full_occupancy: f64,
    pub maximum_gap: usize,
    pub left_flank_noise: f64,
    pub right_flank_noise: f64,
    /// The cleanest side: `min(left, right)`.
    pub flank_noise: f64,
}

impl ProjectionCandidate {
    /// True for near-unbroken ink spanning the complete piano system.
    #[must_use]
    pub fn is_certain_connector(&self, parameters: &BarTuningParameters) -> bool {
        self.paired_occupancy >= parameters.certain_paired
            && self.mean_occupancy >= parameters.certain_mean
            && self.bridge_occupancy >= parameters.certain_bridge
            && self.full_occupancy >= parameters.certain_full
            && self.maximum_gap <= parameters.certain_max_gap
    }
}

/// Maximum mean ink over any window of `length` samples.
///
/// Mirrors the Python `_window_occupancy`: shorter inputs fall back to their
/// plain mean, empty inputs to zero.  Computed as an exact integer sliding
/// sum, which equals `np.convolve(ink, ones(length), "valid").max() / length`.
#[must_use]
pub fn window_occupancy(ink: &[bool], length: usize) -> f64 {
    if ink.is_empty() {
        return 0.0;
    }
    if ink.len() < length {
        let sum = ink.iter().filter(|value| **value).count();
        return sum as f64 / ink.len() as f64;
    }
    let mut sum: usize = ink[..length].iter().filter(|value| **value).count();
    let mut best = sum;
    for index in length..ink.len() {
        sum += usize::from(ink[index]);
        sum -= usize::from(ink[index - length]);
        best = best.max(sum);
    }
    best as f64 / length as f64
}

/// Longest run without ink in a vertical system profile.
#[must_use]
pub fn maximum_gap(ink: &[bool]) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for value in ink {
        current = if *value { 0 } else { current + 1 };
        longest = longest.max(current);
    }
    longest
}

fn is_ink(gray: u8, parameters: &BarTuningParameters) -> bool {
    255 - gray > parameters.ink_threshold
}

/// Ink beside a candidate column, ignoring staff lines and beams.
///
/// Rows inked across most of a wide neighborhood are horizontal structure
/// (staff lines, beams, a slur near its crossing) and say nothing about
/// whether the flank is clear, so they are excluded.  Each side reports the
/// worst single column at the configured flank distances; the line's own
/// second pixel column can spill into one side when the peak is off-center,
/// which is why callers compare the cleanest side against the limit.
#[must_use]
pub fn flank_noise(
    gray: &GrayRaster,
    x: usize,
    top: usize,
    bottom: usize,
    parameters: &BarTuningParameters,
) -> (f64, f64) {
    let width = gray.width();
    let pixels = gray.pixels();
    let top = top.min(gray.height());
    let bottom = bottom.min(gray.height()).max(top);
    let lo = x.saturating_sub(parameters.flank_neighborhood);
    let hi = (x + parameters.flank_neighborhood + 1).min(width);
    let window = (hi - lo).max(1);
    let mut keep_rows: Vec<usize> = Vec::with_capacity(bottom - top);
    for row in top..bottom {
        let offset = row * width;
        let inked = pixels[offset + lo..offset + hi]
            .iter()
            .filter(|value| is_ink(**value, parameters))
            .count();
        // Python: band[:, lo:hi].mean(axis=1) < 0.85 keeps the row.
        if (inked as f64 / window as f64) < parameters.flank_wide_row_cut {
            keep_rows.push(row);
        }
    }
    let total = keep_rows.len().max(1);
    let side = |sign: i64| -> f64 {
        let mut worst: f64 = 0.0;
        for distance in parameters.flank_distances {
            let column = x as i64 + sign * distance as i64;
            let fraction = if column >= 0 && (column as usize) < width {
                let column = column as usize;
                let inked = keep_rows
                    .iter()
                    .filter(|row| is_ink(pixels[**row * width + column], parameters))
                    .count();
                inked as f64 / total as f64
            } else {
                0.0
            };
            worst = worst.max(fraction);
        }
        worst
    };
    (side(-1), side(1))
}

/// Scans the system band for vertical-ink candidates.
///
/// Mirrors the Python `projection_candidates`: per-column paired/bridge/full
/// occupancy and maximum gap, a hard paired/bridge filter, a local-maximum
/// test on the combined score, clustering, and flank measurement on the
/// surviving peaks.
#[must_use]
pub fn projection_candidates(
    gray: &GrayRaster,
    band: &SystemBand,
    parameters: &BarTuningParameters,
) -> Vec<ProjectionCandidate> {
    // Python truncates the float bounds with int().
    let top = (band.top as i64).max(0) as usize;
    let bottom = ((band.bottom as i64).max(0) as usize).min(gray.height());
    let left = (band.left as i64).max(0) as usize;
    let right = ((band.right as i64).max(0) as usize).min(gray.width());
    if bottom <= top || right <= left {
        return Vec::new();
    }
    let height = (bottom - top).max(1);
    let half = rint_usize(height as f64 * parameters.band_ratio).max(parameters.band_floor);
    let span = rint_usize(height as f64 * parameters.span_ratio).max(parameters.span_floor);
    let width = gray.width();
    let pixels = gray.pixels();

    struct ColumnScore {
        x: usize,
        paired: f64,
        mean: f64,
        bridge: f64,
        full: f64,
        max_gap: usize,
    }

    let scan_start = left + parameters.scan_margin;
    let scan_end = right.saturating_sub(parameters.scan_margin);
    if scan_end <= scan_start {
        return Vec::new();
    }
    let mut scores: Vec<ColumnScore> = Vec::with_capacity(scan_end - scan_start);
    let rows = bottom - top;
    let mut ink = vec![false; rows];
    for x in scan_start..scan_end {
        for (index, slot) in ink.iter_mut().enumerate() {
            let offset = (top + index) * width;
            // 3 px probe strip x-1..=x+1, max over the strip.
            let lo = x.saturating_sub(1);
            let hi = (x + 2).min(width);
            *slot = pixels[offset + lo..offset + hi]
                .iter()
                .any(|value| is_ink(*value, parameters));
        }
        let half = half.min(rows);
        let upper = window_occupancy(&ink[..half], span);
        let lower = window_occupancy(&ink[rows - half..], span);
        let bridge_start = half.saturating_sub(parameters.bridge_overlap);
        let bridge_end = (rows - half + parameters.bridge_overlap).min(rows);
        let bridge = if bridge_end > bridge_start {
            let inked = ink[bridge_start..bridge_end]
                .iter()
                .filter(|value| **value)
                .count();
            inked as f64 / (bridge_end - bridge_start) as f64
        } else {
            0.0
        };
        let inked = ink.iter().filter(|value| **value).count();
        let full = inked as f64 / rows as f64;
        scores.push(ColumnScore {
            x,
            paired: upper.min(lower),
            mean: (upper + lower) / 2.0,
            bridge,
            full,
            max_gap: maximum_gap(&ink),
        });
    }

    let combined =
        |score: &ColumnScore| 0.45 * score.paired + 0.25 * score.bridge + 0.30 * score.full;
    let mut peaks: Vec<ProjectionCandidate> = Vec::new();
    for (index, score) in scores.iter().enumerate() {
        // Piano-system barlines connect the two staves.  Merely aligned stems
        // can score strongly in the upper and lower bands but leave the
        // inter-staff gap empty, so they must not enter the lattice.
        if score.paired < parameters.paired_cut || score.bridge < parameters.bridge_cut {
            continue;
        }
        let radius = parameters.local_max_radius;
        let neighborhood =
            &scores[index.saturating_sub(radius)..(index + radius + 1).min(scores.len())];
        let best = neighborhood
            .iter()
            .map(combined)
            .fold(f64::NEG_INFINITY, f64::max);
        if combined(score) < best {
            continue;
        }
        peaks.push(ProjectionCandidate {
            x: score.x as f64,
            paired_occupancy: score.paired,
            mean_occupancy: score.mean,
            bridge_occupancy: score.bridge,
            full_occupancy: score.full,
            maximum_gap: score.max_gap,
            left_flank_noise: 0.0,
            right_flank_noise: 0.0,
            flank_noise: 0.0,
        });
    }

    let mut clustered: Vec<ProjectionCandidate> = Vec::new();
    for peak in peaks {
        match clustered.last_mut() {
            Some(last) if peak.x - last.x <= parameters.cluster_merge => {
                if peak.paired_occupancy > last.paired_occupancy {
                    *last = peak;
                }
            }
            _ => clustered.push(peak),
        }
    }
    for candidate in &mut clustered {
        let (left_noise, right_noise) =
            flank_noise(gray, candidate.x as usize, top, bottom, parameters);
        candidate.left_flank_noise = left_noise;
        candidate.right_flank_noise = right_noise;
        candidate.flank_noise = left_noise.min(right_noise);
    }
    clustered
}

#[cfg(test)]
mod tests {
    use super::*;

    fn white_canvas(width: usize, height: usize) -> Vec<u8> {
        vec![255; width * height]
    }

    fn paint(
        pixels: &mut [u8],
        width: usize,
        rows: std::ops::Range<usize>,
        cols: std::ops::Range<usize>,
        value: u8,
    ) {
        for row in rows {
            for col in cols.clone() {
                pixels[row * width + col] = value;
            }
        }
    }

    fn full_band() -> SystemBand {
        SystemBand {
            left: 0.0,
            right: 100.0,
            top: 0.0,
            bottom: 100.0,
        }
    }

    #[test]
    fn projection_requires_interstaff_bridge_ink() {
        let mut pixels = white_canvas(100, 100);
        // Aligned stems in each staff, with empty inter-staff space.
        paint(&mut pixels, 100, 0..40, 39..42, 0);
        paint(&mut pixels, 100, 60..100, 39..42, 0);
        // A real connected piano-system barline.
        paint(&mut pixels, 100, 0..100, 69..72, 0);
        let gray = GrayRaster::from_raw_parts(100, 100, pixels);
        let parameters = BarTuningParameters::python_reference();
        let candidates = projection_candidates(&gray, &full_band(), &parameters);
        assert!(candidates.iter().any(|c| (c.x - 70.0).abs() <= 2.0));
        assert!(!candidates.iter().any(|c| (c.x - 40.0).abs() <= 2.0));
    }

    #[test]
    fn certain_connector_requires_complete_system_continuity() {
        let mut pixels = white_canvas(100, 100);
        // Dense aligned stems can occupy both staff bands and the narrow
        // center sample while still having white gaps elsewhere.
        paint(&mut pixels, 100, 0..100, 39..42, 0);
        paint(&mut pixels, 100, 18..23, 39..42, 255);
        paint(&mut pixels, 100, 77..82, 39..42, 255);
        paint(&mut pixels, 100, 0..100, 69..72, 0);
        let gray = GrayRaster::from_raw_parts(100, 100, pixels);
        let parameters = BarTuningParameters::python_reference();
        let candidates = projection_candidates(&gray, &full_band(), &parameters);
        let broken = candidates
            .iter()
            .min_by(|a, b| (a.x - 40.0).abs().partial_cmp(&(b.x - 40.0).abs()).unwrap())
            .unwrap();
        let complete = candidates
            .iter()
            .min_by(|a, b| (a.x - 70.0).abs().partial_cmp(&(b.x - 70.0).abs()).unwrap())
            .unwrap();
        assert!(!broken.is_certain_connector(&parameters));
        assert!(complete.is_certain_connector(&parameters));
    }

    #[test]
    fn crowded_flanks_reject_stem_column_but_keep_clean_vertical() {
        let mut pixels = white_canvas(100, 100);
        // Aligned chord stems bridged by a dynamic glyph: full-height ink at
        // x=40 with noteheads and the glyph pressed against both flanks.
        paint(&mut pixels, 100, 0..100, 39..42, 0);
        paint(&mut pixels, 100, 30..36, 39..42, 255);
        paint(&mut pixels, 100, 8..15, 33..39, 0);
        paint(&mut pixels, 100, 70..78, 42..48, 0);
        paint(&mut pixels, 100, 45..55, 34..47, 0);
        // A degraded but genuine barline: same interruption, clear flanks.
        paint(&mut pixels, 100, 0..100, 69..72, 0);
        paint(&mut pixels, 100, 30..36, 69..72, 255);
        let gray = GrayRaster::from_raw_parts(100, 100, pixels);
        let parameters = BarTuningParameters::python_reference();
        let candidates = projection_candidates(&gray, &full_band(), &parameters);
        let stem = candidates
            .iter()
            .min_by(|a, b| (a.x - 40.0).abs().partial_cmp(&(b.x - 40.0).abs()).unwrap())
            .unwrap();
        let bar = candidates
            .iter()
            .min_by(|a, b| (a.x - 70.0).abs().partial_cmp(&(b.x - 70.0).abs()).unwrap())
            .unwrap();
        assert!(stem.flank_noise > parameters.flank_noise_limit);
        assert!(bar.flank_noise <= parameters.flank_noise_limit);
    }

    #[test]
    fn scale_derivation_reproduces_the_python_reference_at_interline_six() {
        let derived = BarTuningParameters::from_scale(6.0);
        assert_eq!(derived, BarTuningParameters::python_reference());
    }
}
