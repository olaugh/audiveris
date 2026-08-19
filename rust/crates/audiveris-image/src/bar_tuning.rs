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

/// A printed measure-number anchor (position plus decoded value).
///
/// Supplied externally: circled-number OCR stays in Python for now, so
/// pure-Rust invocations pass an empty slice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    pub x: f64,
    pub value: u32,
}

/// Provenance of a tuned boundary, mirroring the Python source labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoundarySource {
    /// Sorted by label to reproduce the Python `"+".join(sorted(...))`
    /// convention: `paired_projection` < `printed_number` < `rust` <
    /// `system_left` < `system_right`.
    PairedProjection,
    PrintedNumber,
    Rust,
    SystemLeft,
    SystemRight,
}

impl BoundarySource {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::PairedProjection => "paired_projection",
            Self::PrintedNumber => "printed_number",
            Self::Rust => "rust",
            Self::SystemLeft => "system_left",
            Self::SystemRight => "system_right",
        }
    }
}

/// One boundary of the tuned hypothesis, edges included.
#[derive(Debug, Clone, PartialEq)]
pub struct TunedBoundary {
    pub x: f64,
    pub evidence: f64,
    /// Sorted and deduplicated provenance labels.
    pub sources: Vec<BoundarySource>,
}

/// Result of the visual interval count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeometryCount {
    /// Number of measure intervals implied by the counted bars.
    pub intervals: usize,
    /// Number of geometry-certain interior bars (strong, corroborated, or
    /// certain-connector backed).
    pub certain_bars: usize,
}

/// Counts measure intervals from strong or corroborated raw bars plus
/// certain connectors.
///
/// Mirrors the Python `geometry_count`, including the signature-prefix
/// guard, the edge margin for final/repeat double-bar strokes, and the
/// height-scaled corroboration gap limit for clean-flanked candidates.
#[must_use]
pub fn geometry_count(
    input: &SystemBarInput,
    projected: &[ProjectionCandidate],
    parameters: &BarTuningParameters,
) -> GeometryCount {
    let left = input.band.left;
    let right = input.band.right;
    let height = input.band.height();
    let guard = left
        + parameters
            .signature_guard_floor
            .max(parameters.signature_guard_ratio * height);
    let mut xs: Vec<f64> = Vec::new();
    let interior = if input.boundaries.len() >= 2 {
        &input.boundaries[1..input.boundaries.len() - 1]
    } else {
        &[]
    };
    for boundary in interior {
        // A weaker Rust boundary may be a real bar interrupted by noteheads
        // or dense beams.  Preserve it when an independent grayscale
        // projection is close and covers most of the system with only short
        // gaps.  Taller systems open proportionally longer gaps on genuine
        // bars, so the gap limit grows with system height — only for
        // candidates whose flanks are clean, which aligned stems never are.
        let corroborated = projected.iter().any(|item| {
            (item.x - boundary.x).abs() <= parameters.corroboration_proximity
                && item.paired_occupancy >= parameters.corroboration_paired
                && item.full_occupancy >= parameters.corroboration_full
                && (item.maximum_gap <= parameters.corroboration_max_gap
                    || (item.maximum_gap as f64 <= parameters.tall_gap_height_ratio * height
                        && item.flank_noise <= parameters.flank_noise_limit))
        });
        if boundary.support >= 2 && (boundary.max_grade >= parameters.strong_grade || corroborated)
        {
            xs.push(boundary.x);
        }
    }
    // The thin stroke of a final or repeat double bar sits within the edge
    // margin and duplicates the edge barline rather than closing a measure.
    xs.extend(
        projected
            .iter()
            .filter(|item| {
                guard <= item.x
                    && item.x <= right - parameters.edge_bar_margin
                    && item.is_certain_connector(parameters)
            })
            .map(|item| item.x),
    );
    xs.sort_by(|a, b| a.partial_cmp(b).expect("finite bar positions"));
    let mut merged: Vec<f64> = Vec::new();
    for x in xs {
        match merged.last_mut() {
            Some(last) if x - *last <= parameters.geometry_merge => {
                *last = (*last + x) / 2.0;
            }
            _ => merged.push(x),
        }
    }
    GeometryCount {
        intervals: merged.len() + 1,
        certain_bars: merged.len(),
    }
}

#[derive(Debug, Clone)]
struct SelectCandidate {
    x: f64,
    evidence: f64,
    sources: Vec<BoundarySource>,
}

impl SelectCandidate {
    fn merge_sources(&mut self, other: &[BoundarySource]) {
        for source in other {
            if !self.sources.contains(source) {
                self.sources.push(*source);
            }
        }
        self.sources.sort();
    }
}

/// Selects exactly `count - 1` interior boundaries (or honestly fewer).
///
/// Mirrors the Python `select_boundaries`: evidence-weighted candidate
/// merging, a DP with a broad log-spacing prior, and a fallback that prefers
/// an honestly short hypothesis to inventing crowded barlines.  The DP
/// replicates the Python dict-iteration semantics: states are scanned in
/// insertion order and ties keep the first optimum.
#[must_use]
pub fn select_boundaries(
    input: &SystemBarInput,
    projected: &[ProjectionCandidate],
    anchors: &[Anchor],
    count: usize,
    parameters: &BarTuningParameters,
) -> Vec<TunedBoundary> {
    let left = input.band.left;
    let right = input.band.right;
    let height = input.band.height();
    // Clef, key signature, and time signature occupy this prefix.  Their
    // flats and cut-time strokes can be vertically aligned in both staves,
    // but cannot be measure barlines.  Rust or a printed-number anchor may
    // still support a boundary here; projection-only candidates may not.
    let signature_guard_right = left
        + parameters
            .signature_guard_floor
            .max(parameters.signature_guard_ratio * height);
    let average = (right - left) / count.max(1) as f64;

    let mut candidates: Vec<SelectCandidate> = Vec::new();
    let interior = if input.boundaries.len() >= 2 {
        &input.boundaries[1..input.boundaries.len() - 1]
    } else {
        &[]
    };
    for boundary in interior {
        // Once both piano staves independently support a high-grade bar,
        // weak count or spacing priors must not replace it with nearby ink.
        let evidence = if boundary.is_strong(parameters) {
            parameters.strong_evidence
        } else {
            parameters.weak_rust_evidence
        };
        candidates.push(SelectCandidate {
            x: boundary.x,
            evidence,
            sources: vec![BoundarySource::Rust],
        });
    }
    for item in projected {
        if item.x < signature_guard_right {
            continue;
        }
        let certain = item.is_certain_connector(parameters);
        // An incomplete vertical whose flanks are crowded on both sides is an
        // aligned-stem column, not a degraded barline; it must not stay in
        // the lattice where a count target could draft it.
        if !certain && item.flank_noise > parameters.flank_noise_limit {
            continue;
        }
        let evidence = if certain {
            parameters.strong_evidence
        } else {
            parameters.weak_projection_base
                + parameters.weak_projection_paired_weight * item.paired_occupancy
                + parameters.weak_projection_bridge_weight * item.bridge_occupancy
        };
        candidates.push(SelectCandidate {
            x: item.x,
            evidence,
            sources: vec![BoundarySource::PairedProjection],
        });
    }
    for anchor in anchors {
        candidates.push(SelectCandidate {
            x: anchor.x,
            evidence: parameters.anchor_evidence,
            sources: vec![BoundarySource::PrintedNumber],
        });
    }
    candidates.sort_by(|a, b| a.x.partial_cmp(&b.x).expect("finite candidate positions"));

    let mut merged: Vec<SelectCandidate> = Vec::new();
    for item in candidates {
        if item.x <= left + parameters.edge_bar_margin
            || item.x >= right - parameters.edge_bar_margin
        {
            continue;
        }
        match merged.last_mut() {
            Some(prior) if item.x - prior.x <= parameters.select_merge => {
                let total = prior.evidence + item.evidence;
                prior.x = (prior.x * prior.evidence + item.x * item.evidence) / total;
                prior.evidence = total;
                prior.merge_sources(&item.sources);
            }
            _ => merged.push(item),
        }
    }

    let need = count.saturating_sub(1);
    let points: Vec<SelectCandidate> = std::iter::once(SelectCandidate {
        x: left,
        evidence: 0.0,
        sources: vec![BoundarySource::SystemLeft],
    })
    .chain(merged.iter().cloned())
    .chain(std::iter::once(SelectCandidate {
        x: right,
        evidence: 0.0,
        sources: vec![BoundarySource::SystemRight],
    }))
    .collect();

    // DP selects exactly `need` interior boundaries.  Spacing is a broad
    // prior, deliberately weaker than paired-staff ink and printed-number
    // evidence.  States live in a Vec in insertion order, matching the
    // Python dict semantics: candidate states are scanned oldest-first and
    // a strict `<` keeps the first optimum on ties.
    struct DpEntry {
        chosen: usize,
        index: usize,
        cost: f64,
        path: Vec<usize>,
    }
    let minimum_gap = parameters
        .dp_min_gap_floor
        .max(parameters.dp_min_gap_ratio * average);
    let mut dp: Vec<DpEntry> = vec![DpEntry {
        chosen: 0,
        index: 0,
        cost: 0.0,
        path: vec![0],
    }];
    let last_index = points.len() - 1;
    for chosen in 0..=need {
        for index in 1..points.len() {
            let mut best: Option<(f64, Vec<usize>)> = None;
            for entry in &dp {
                if entry.chosen != chosen || entry.index >= index {
                    continue;
                }
                let is_right = index == last_index;
                if is_right != (chosen == need) {
                    continue;
                }
                let gap = points[index].x - points[entry.index].x;
                if gap < minimum_gap {
                    continue;
                }
                let ratio = (gap / average).max(parameters.spacing_floor);
                let spacing = parameters.spacing_weight * ratio.ln() * ratio.ln();
                let evidence = if is_right {
                    0.0
                } else {
                    points[index].evidence
                };
                let cost = entry.cost + spacing - evidence;
                if best.as_ref().is_none_or(|(current, _)| cost < *current) {
                    let mut path = entry.path.clone();
                    path.push(index);
                    best = Some((cost, path));
                }
            }
            if let Some((cost, path)) = best {
                let key_chosen = chosen + usize::from(index != last_index);
                if let Some(existing) = dp
                    .iter_mut()
                    .find(|entry| entry.chosen == key_chosen && entry.index == index)
                {
                    existing.cost = cost;
                    existing.path = path;
                } else {
                    dp.push(DpEntry {
                        chosen: key_chosen,
                        index,
                        cost,
                        path,
                    });
                }
            }
        }
    }

    let interior: Vec<SelectCandidate> = match dp
        .iter()
        .find(|entry| entry.chosen == need && entry.index == last_index)
    {
        Some(final_entry) => final_entry.path[1..final_entry.path.len() - 1]
            .iter()
            .map(|index| points[*index].clone())
            .collect(),
        None => {
            // Prefer an honestly short hypothesis to inventing crowded
            // barlines merely to satisfy the count target.
            let mut ordered: Vec<&SelectCandidate> = merged.iter().collect();
            ordered.sort_by(|a, b| {
                b.evidence
                    .partial_cmp(&a.evidence)
                    .expect("finite candidate evidence")
            });
            let mut chosen: Vec<SelectCandidate> = Vec::new();
            for item in ordered {
                if item.x - left < minimum_gap || right - item.x < minimum_gap {
                    continue;
                }
                if chosen
                    .iter()
                    .any(|picked| (item.x - picked.x).abs() < minimum_gap)
                {
                    continue;
                }
                chosen.push(item.clone());
                if chosen.len() == need {
                    break;
                }
            }
            chosen
        }
    };

    let mut result: Vec<TunedBoundary> = Vec::with_capacity(interior.len() + 2);
    result.push(TunedBoundary {
        x: points[0].x,
        evidence: 0.0,
        sources: points[0].sources.clone(),
    });
    let mut sorted_interior = interior;
    sorted_interior.sort_by(|a, b| a.x.partial_cmp(&b.x).expect("finite boundary positions"));
    result.extend(sorted_interior.into_iter().map(|item| TunedBoundary {
        x: item.x,
        evidence: item.evidence,
        sources: item.sources,
    }));
    result.push(TunedBoundary {
        x: points[last_index].x,
        evidence: 0.0,
        sources: points[last_index].sources.clone(),
    });
    result
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

    /// Mirror of the Python `row()` fixture helper: every boundary is a
    /// strong two-staff bar unless the test weakens it afterwards.
    fn system(boundaries: &[f64], right: f64, bottom: f64) -> SystemBarInput {
        SystemBarInput {
            band: SystemBand {
                left: 0.0,
                right,
                top: 0.0,
                bottom,
            },
            boundaries: boundaries
                .iter()
                .map(|x| RawBoundary {
                    x: *x,
                    support: 2,
                    max_grade: 0.92,
                    kind: RawBoundaryKind::Unknown,
                })
                .collect(),
        }
    }

    fn perfect_connector(x: f64) -> ProjectionCandidate {
        ProjectionCandidate {
            x,
            paired_occupancy: 1.0,
            mean_occupancy: 1.0,
            bridge_occupancy: 1.0,
            full_occupancy: 1.0,
            maximum_gap: 0,
            left_flank_noise: 0.0,
            right_flank_noise: 0.0,
            flank_noise: 0.0,
        }
    }

    #[test]
    fn signature_prefix_rejects_projection_only_candidates() {
        let input = system(&[0.0, 70.0, 100.0], 100.0, 100.0);
        let parameters = BarTuningParameters::python_reference();
        let selected = select_boundaries(
            &input,
            &[perfect_connector(35.0), perfect_connector(70.0)],
            &[],
            2,
            &parameters,
        );
        assert!(!selected.iter().any(|point| (point.x - 35.0).abs() <= 2.0));
    }

    #[test]
    fn complete_connectors_determine_count_without_partial_stem() {
        let input = system(&[0.0, 200.0], 200.0, 40.0);
        let parameters = BarTuningParameters::python_reference();
        let mut projected: Vec<ProjectionCandidate> = [60.0, 90.0, 120.0, 150.0]
            .iter()
            .map(|x| perfect_connector(*x))
            .collect();
        projected.push(ProjectionCandidate {
            x: 135.0,
            paired_occupancy: 0.75,
            mean_occupancy: 0.80,
            bridge_occupancy: 0.65,
            full_occupancy: 0.75,
            maximum_gap: 8,
            left_flank_noise: 0.0,
            right_flank_noise: 0.0,
            flank_noise: 0.0,
        });
        let count = geometry_count(&input, &projected, &parameters);
        assert_eq!(
            (count.intervals, count.certain_bars),
            (5, 4),
            "partial stem must not inflate the geometry count"
        );
    }

    #[test]
    fn projection_corroborates_an_interrupted_weak_raw_bar() {
        let mut input = system(&[0.0, 50.0, 100.0], 100.0, 100.0);
        input.boundaries[1].max_grade = 0.69;
        let parameters = BarTuningParameters::python_reference();
        let projected = [ProjectionCandidate {
            x: 51.0,
            paired_occupancy: 0.91,
            mean_occupancy: 0.95,
            bridge_occupancy: 0.78,
            full_occupancy: 0.85,
            maximum_gap: 6,
            left_flank_noise: 0.0,
            right_flank_noise: 0.0,
            flank_noise: 0.0,
        }];
        let count = geometry_count(&input, &projected, &parameters);
        assert_eq!((count.intervals, count.certain_bars), (2, 1));
    }

    #[test]
    fn final_double_bar_stroke_does_not_inflate_geometry_count() {
        // The thin stroke of a final/repeat double bar sits a few pixels
        // inside the system's right edge: same physical boundary, not
        // another measure.
        let input = system(&[0.0, 100.0, 200.0], 200.0, 40.0);
        let parameters = BarTuningParameters::python_reference();
        let projected = [perfect_connector(100.0), perfect_connector(195.0)];
        let count = geometry_count(&input, &projected, &parameters);
        assert_eq!((count.intervals, count.certain_bars), (2, 1));
    }

    #[test]
    fn count_target_cannot_draft_dirty_flank_projection() {
        let input = system(&[0.0, 50.0, 100.0], 100.0, 40.0);
        let parameters = BarTuningParameters::python_reference();
        let stemlike = ProjectionCandidate {
            x: 75.0,
            paired_occupancy: 1.0,
            mean_occupancy: 1.0,
            bridge_occupancy: 0.74,
            full_occupancy: 0.85,
            maximum_gap: 10,
            left_flank_noise: 0.21,
            right_flank_noise: 0.30,
            flank_noise: 0.21,
        };
        let selected = select_boundaries(&input, &[stemlike.clone()], &[], 3, &parameters);
        assert_eq!(
            selected.iter().map(|point| point.x).collect::<Vec<_>>(),
            vec![0.0, 50.0, 100.0],
            "dirty flanks leave the hypothesis honestly short"
        );
        let clean = ProjectionCandidate {
            left_flank_noise: 0.02,
            right_flank_noise: 0.02,
            flank_noise: 0.02,
            ..stemlike
        };
        let selected = select_boundaries(&input, &[clean], &[], 3, &parameters);
        assert_eq!(
            selected.iter().map(|point| point.x).collect::<Vec<_>>(),
            vec![0.0, 50.0, 75.0, 100.0]
        );
    }

    #[test]
    fn tall_system_corroboration_scales_gap_limit_with_clean_flanks() {
        // A warp-degraded real bar on a tall (high-resolution) system opens
        // gaps past the reference limit; with clean flanks the limit scales
        // with system height.  Dirty flanks (aligned stems) get no relief.
        let mut input = system(&[0.0, 700.0, 1400.0], 1400.0, 220.0);
        input.boundaries[1].max_grade = 0.79;
        let parameters = BarTuningParameters::python_reference();
        let degraded = ProjectionCandidate {
            x: 701.0,
            paired_occupancy: 1.0,
            mean_occupancy: 1.0,
            bridge_occupancy: 1.0,
            full_occupancy: 0.95,
            maximum_gap: 10,
            left_flank_noise: 0.02,
            right_flank_noise: 0.02,
            flank_noise: 0.02,
        };
        let count = geometry_count(&input, &[degraded.clone()], &parameters);
        assert_eq!((count.intervals, count.certain_bars), (2, 1));
        let stemlike = ProjectionCandidate {
            flank_noise: 0.25,
            ..degraded.clone()
        };
        let count = geometry_count(&input, &[stemlike], &parameters);
        assert_eq!((count.intervals, count.certain_bars), (1, 0));
        // At the reference scale (short systems) the relaxed arm stays shut.
        let mut short = system(&[0.0, 50.0, 100.0], 100.0, 100.0);
        short.boundaries[1].max_grade = 0.79;
        let count = geometry_count(
            &short,
            &[ProjectionCandidate {
                x: 51.0,
                ..degraded
            }],
            &parameters,
        );
        assert_eq!((count.intervals, count.certain_bars), (1, 0));
    }

    #[test]
    fn impossible_count_prefers_honest_short_hypothesis_over_crowded_duplicate() {
        let input = system(&[0.0, 100.0], 100.0, 40.0);
        let parameters = BarTuningParameters::python_reference();
        let projected = [
            perfect_connector(50.0),
            ProjectionCandidate {
                x: 57.0,
                paired_occupancy: 0.80,
                mean_occupancy: 0.90,
                bridge_occupancy: 0.72,
                full_occupancy: 0.76,
                maximum_gap: 10,
                left_flank_noise: 0.0,
                right_flank_noise: 0.0,
                flank_noise: 0.0,
            },
        ];
        let selected = select_boundaries(&input, &projected, &[], 3, &parameters);
        assert_eq!(
            selected.iter().map(|point| point.x).collect::<Vec<_>>(),
            vec![0.0, 50.0, 100.0]
        );
    }
}
