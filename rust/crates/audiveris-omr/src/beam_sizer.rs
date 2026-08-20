// SPDX-License-Identifier: AGPL-3.0-or-later

//! Measure a sheet's real single-beam thickness from its own beam ink.
//!
//! `Scale.beam` is a page-level black-run histogram peak, and a sheet
//! dominated by fused multi-beam stacks poisons it: the synthetic 64th-run
//! corpus declares 6 where the true beam is 3.5, so `min_height_low`
//! (0.7x declared) rejects every real beam as too thin and only the fused
//! blobs survive. Measuring from accepted beams cannot recover -- the first
//! pass only accepts what fits the wrong scale -- so the estimate comes from
//! the border-line heights of every browsed structure BEFORE any split: a
//! stack's border line is never thinner than one beam, so the smallest
//! recurring height cluster across the page is the single-beam thickness.
//!
//! The policy is a dead band, learned from a negative result: re-centring
//! the scale when the declaration is already right (Schenker declares 4,
//! measures 4.3) moves the floor and the split counts with it and loses
//! beams. The declared value therefore stands whenever the estimate agrees
//! with it within the band; only an estimate far outside -- a declaration
//! the sheet's own ink refutes -- replaces it, unclamped.

/// A structure line must come from a glyph at least this wide (in
/// interlines) to inform the estimate; narrower fragments jitter.
pub const MIN_WIDTH_INTERLINES: f64 = 2.0;

/// Cluster half-width: heights within this ratio of the running cluster
/// median belong to the same beam count.
pub const CLUSTER_RATIO: f64 = 1.3;

/// Below this many qualifying lines in the smallest cluster the declared
/// scale stands.
pub const MIN_SAMPLE: usize = 4;

/// The dead band around the declared scale: an estimate inside it means the
/// declaration is sound and stands untouched. The floor is generous because
/// the pre-closing raster under-measures by about half a pixel of
/// antialiasing: a true-3.5px sheet declared at 4 measures 3.0 (ratio 0.75)
/// and must NOT refute, while the stack-poisoned case measures 3.0 against
/// a declared 6 (ratio 0.5) and must.
pub const MAX_RATIO: f64 = 1.35;
pub const MIN_RATIO: f64 = 0.70;

/// `AUDIVERIS_MEASURED_BEAM_THICKNESS` gate (enhancement, default OFF).
#[must_use]
pub fn measured_beam_thickness_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("AUDIVERIS_MEASURED_BEAM_THICKNESS")
            .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
    })
}

/// The measurement and the evidence behind it, so a report can show its work.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamSizing {
    pub declared: f64,
    /// The smallest-cluster median, `None` when too few lines qualified.
    pub measured: Option<f64>,
    /// What item grading should use.
    pub effective: f64,
    /// Lines in the smallest cluster.
    pub sample_count: usize,
    /// True when the estimate replaced the declared scale for item grading.
    pub applied: bool,
    /// What the morphological closing should be sized from. Less strict than
    /// `applied`: a smaller disk only un-bridges gutters, so any
    /// meaningfully smaller measurement improves the spots even when the
    /// declared item band is still sound (declared 4 / true 3: the band
    /// tolerates 3px items, but the size-4 disk closes every 1px gutter
    /// solid).
    pub closing: f64,
}

/// Estimate the single-beam thickness from pre-split structure line heights.
///
/// `heights` are the border-line heights of every browsed structure whose
/// glyph spans at least [`MIN_WIDTH_INTERLINES`]; the caller filters width.
#[must_use]
pub fn measure_from_line_heights(heights: &[f64], interline: f64, declared: f64) -> BeamSizing {
    // Nothing thinner than a third of an interline is a beam: border lines
    // at 1-2px are staff-line and ledger fragments, and letting them seed
    // the cluster halves every real beam through the split (measured: the
    // first estimator cost the pairs corpus 14 of 24 beams).
    let floor = 0.35 * interline;
    let mut sorted: Vec<f64> = heights
        .iter()
        .copied()
        .filter(|height| *height >= floor)
        .collect();
    sorted.sort_by(f64::total_cmp);
    let mut cluster: Vec<f64> = Vec::new();
    for &height in &sorted {
        if cluster.is_empty() {
            cluster.push(height);
            continue;
        }
        let median = cluster[cluster.len() / 2];
        if height <= median * CLUSTER_RATIO {
            cluster.push(height);
        } else if cluster.len() < MIN_SAMPLE {
            // Too few to be a population: restart from this height. A lone
            // sliver below every real beam must not become the estimate.
            cluster.clear();
            cluster.push(height);
        } else {
            break;
        }
    }
    if cluster.len() < MIN_SAMPLE {
        return BeamSizing {
            declared,
            measured: None,
            effective: declared,
            sample_count: cluster.len(),
            applied: false,
            closing: declared,
        };
    }
    let measured = cluster[cluster.len() / 2];
    // Downward only: a stack-poisoned histogram over-declares, and that is
    // the failure this corrects. Raising the scale toward a larger estimate
    // was measured to lose beams on the Schenker corpus (the floor and the
    // split counts rise with it), so an estimate above the declared scale
    // never applies.
    let refuted = measured < declared * MIN_RATIO;
    BeamSizing {
        declared,
        measured: Some(measured),
        effective: if refuted { measured } else { declared },
        sample_count: cluster.len(),
        applied: refuted,
        // The closing follows the full refute only: a scanned page's thin
        // print measures ~0.75x its (correct) declared scale, and shrinking
        // the disk there fragments every real beam (measured: Schenker
        // page 7 fell from 579 beams to 17 under a 0.95 threshold).
        closing: if refuted { measured } else { declared },
    }
}

/// Estimate the single-beam thickness from a pre-closing spot raster.
///
/// The closing radius itself derives from the declared beam scale, so a
/// poisoned declaration bridges the stack gutters before any structure is
/// analysed and no honest line height ever exists to measure (the 64th-run
/// corpus at declared 6: every stack closes into one 17px blob and dies
/// "inconsistent border pairs"). The pre-closing raster still shows each
/// level separately. A vertical ink run is sampled when a run of similar
/// vertical extent exists one interline to each side -- ink that continues
/// horizontally both ways is a bar, and noteheads (about one interline
/// wide) fail one side. Stems exceed the length cap; staff lines and
/// ledgers fall below the floor.
#[must_use]
pub fn measure_from_erased(
    erased: &[u8],
    width: usize,
    height: usize,
    ink_threshold: u8,
    interline: f64,
    declared: f64,
) -> BeamSizing {
    let step = interline.round() as usize;
    let floor = 0.35 * interline;
    let cap = 3.0 * interline;
    let column = |x: usize| -> Vec<(usize, usize)> {
        let mut runs = Vec::new();
        let mut start = None;
        for y in 0..height {
            // `spots::threshold` counts `pixel <= level` as ink.
            let ink = erased[y * width + x] <= ink_threshold;
            match (ink, start) {
                (true, None) => start = Some(y),
                (false, Some(s)) => {
                    runs.push((s, y));
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(s) = start {
            runs.push((s, height));
        }
        runs
    };
    let overlaps = |runs: &[(usize, usize)], lo: usize, hi: usize| -> bool {
        runs.iter().any(|&(s, e)| {
            let inter = e.min(hi).saturating_sub(s.max(lo));
            inter * 2 >= hi - lo
        })
    };
    let columns: Vec<Vec<(usize, usize)>> = (0..width).map(column).collect();
    let mut heights = Vec::new();
    if width > 2 * step {
        for x in step..width - step {
            for &(s, e) in &columns[x] {
                let length = (e - s) as f64;
                if length >= floor
                    && length <= cap
                    && overlaps(&columns[x - step], s, e)
                    && overlaps(&columns[x + step], s, e)
                {
                    heights.push(length);
                }
            }
        }
    }
    measure_from_line_heights(&heights, interline, declared)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_declaration_stands() {
        // Schenker: declares 4, long beams measure ~4.3 -- inside the band.
        let heights = [4.2, 4.3, 4.3, 4.4, 4.5, 8.9, 9.1, 13.2];
        let sizing = measure_from_line_heights(&heights, 6.0, 4.0);
        assert_eq!(sizing.effective, 4.0);
        assert!(!sizing.applied);
        assert_eq!(sizing.measured, Some(4.3));
    }

    #[test]
    fn antialiased_under_measurement_does_not_refute() {
        // A true-3.5px sheet declared at 4 measures ~3.0 pre-closing.
        let heights = [2.9, 3.0, 3.0, 3.0, 3.1, 3.1];
        let sizing = measure_from_line_heights(&heights, 6.0, 4.0);
        assert_eq!(sizing.effective, 4.0);
        assert!(!sizing.applied);
    }

    #[test]
    fn refuted_declaration_is_replaced() {
        // The 64th corpus: declares 6, single beams measure 3.5.
        let heights = [3.4, 3.5, 3.5, 3.6, 3.6, 7.1, 7.2, 13.8, 14.0];
        let sizing = measure_from_line_heights(&heights, 6.0, 6.0);
        assert_eq!(sizing.effective, 3.5);
        assert!(sizing.applied);
    }

    #[test]
    fn lone_sliver_does_not_seed_the_cluster() {
        // Sub-beam fragments (staff-line and ledger residue) are filtered
        // before clustering; the real population at 4.3 wins.
        let heights = [1.5, 1.6, 1.6, 1.7, 4.2, 4.3, 4.3, 4.4, 4.5];
        let sizing = measure_from_line_heights(&heights, 6.0, 4.0);
        assert_eq!(sizing.measured, Some(4.3));
        assert!(!sizing.applied);
    }

    #[test]
    fn estimates_above_the_declared_scale_never_apply() {
        let heights = [5.4, 5.5, 5.5, 5.6, 5.7];
        let sizing = measure_from_line_heights(&heights, 6.0, 4.0);
        assert_eq!(sizing.effective, 4.0);
        assert!(!sizing.applied);
    }

    #[test]
    fn too_few_lines_keep_declared() {
        let sizing = measure_from_line_heights(&[3.5, 3.6], 6.0, 6.0);
        assert_eq!(sizing.effective, 6.0);
        assert!(sizing.measured.is_none());
    }
}
