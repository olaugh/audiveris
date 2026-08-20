// SPDX-License-Identifier: AGPL-3.0-or-later

//! Measure a sheet's real beam thickness from the beams it actually found.
//!
//! `Scale.beam` is a page-level histogram estimate rounded to a whole pixel,
//! and on the Schenker scans it reads 4 where confident beams measure 4.3.
//! Every beam impact is scaled by that value -- `min_height_low` is 0.7x it,
//! `max_height_high` is 1.4x it, and both impact denominators derive from it
//! -- so a low estimate makes the ceiling too tight and grades legitimate
//! beams to death once the spot closing fuses a notehead onto them.
//!
//! This is the same move `black_head_sizer` already makes for noteheads:
//! trust the ink over the sheet-level estimate.
//!
//! The estimate deliberately uses the MEDIAN over LONG beams. The mode is not
//! usable: on a page where beam recall has collapsed the surviving population
//! is dominated by short false positives (3px ledger slivers on this corpus),
//! so the mode measures the noise exactly when the correction is needed most.
//! A beam at least `MIN_LENGTH_INTERLINES` interlines long is essentially
//! never such a sliver, and its median is stable page to page.

/// A beam must span at least this many interlines to inform the estimate.
pub const MIN_LENGTH_INTERLINES: f64 = 3.0;

/// And be graded at least this well.
pub const MIN_GRADE: f64 = 0.4;

/// Below this many qualifying beams the sheet estimate stands.
pub const MIN_SAMPLE: usize = 8;

/// The estimate is clamped to this band around the declared scale, so a
/// pathological page cannot move the ceiling arbitrarily far.
pub const MAX_RATIO: f64 = 1.35;
pub const MIN_RATIO: f64 = 0.85;

/// `AUDIVERIS_MEASURED_BEAM_THICKNESS` gate (enhancement, default OFF).
#[must_use]
pub fn measured_beam_thickness_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("AUDIVERIS_MEASURED_BEAM_THICKNESS")
            .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
    })
}

/// One beam's evidence for the estimate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamSample {
    pub length: f64,
    pub thickness: f64,
    pub grade: f64,
}

/// The measurement and the evidence behind it, so a report can show its work.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamSizing {
    pub declared: f64,
    /// `None` when too few beams qualified, in which case `declared` stands.
    pub measured: Option<f64>,
    /// What the caller should actually use.
    pub effective: f64,
    pub sample_count: usize,
    pub clamped: bool,
}

/// Median thickness of the long, confident beams among `samples`.
#[must_use]
pub fn measure_beam_thickness(samples: &[BeamSample], interline: f64, declared: f64) -> BeamSizing {
    let minimum_length = MIN_LENGTH_INTERLINES * interline;
    let mut thicknesses = samples
        .iter()
        .filter(|sample| sample.length >= minimum_length && sample.grade >= MIN_GRADE)
        .map(|sample| sample.thickness)
        .collect::<Vec<_>>();
    if thicknesses.len() < MIN_SAMPLE {
        return BeamSizing {
            declared,
            measured: None,
            effective: declared,
            sample_count: thicknesses.len(),
            clamped: false,
        };
    }
    thicknesses.sort_by(f64::total_cmp);
    let middle = thicknesses.len() / 2;
    let median = if thicknesses.len() % 2 == 0 {
        (thicknesses[middle - 1] + thicknesses[middle]) / 2.0
    } else {
        thicknesses[middle]
    };
    let low = declared * MIN_RATIO;
    let high = declared * MAX_RATIO;
    let effective = median.clamp(low, high);
    BeamSizing {
        declared,
        measured: Some(median),
        effective,
        sample_count: thicknesses.len(),
        clamped: (effective - median).abs() > f64::EPSILON,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(length: f64, thickness: f64) -> BeamSample {
        BeamSample {
            length,
            thickness,
            grade: 0.6,
        }
    }

    #[test]
    fn short_and_weak_beams_do_not_inform_the_estimate() {
        // Ten 3px slivers, all too short: the estimate must not follow them.
        let mut samples = vec![sample(9.0, 3.0); 10];
        samples.extend(std::iter::repeat_n(sample(30.0, 4.4), 8));
        let sizing = measure_beam_thickness(&samples, 7.0, 4.0);
        assert_eq!(sizing.sample_count, 8);
        assert_eq!(sizing.measured, Some(4.4));
        assert!((sizing.effective - 4.4).abs() < 1e-9);
    }

    #[test]
    fn too_few_qualifying_beams_leave_the_sheet_estimate_alone() {
        let samples = vec![sample(30.0, 4.4); MIN_SAMPLE - 1];
        let sizing = measure_beam_thickness(&samples, 7.0, 4.0);
        assert_eq!(sizing.measured, None);
        assert!((sizing.effective - 4.0).abs() < 1e-9);
    }

    #[test]
    fn a_wild_estimate_is_clamped_to_the_declared_band() {
        let samples = vec![sample(30.0, 12.0); 20];
        let sizing = measure_beam_thickness(&samples, 7.0, 4.0);
        assert_eq!(sizing.measured, Some(12.0));
        assert!(sizing.clamped);
        assert!((sizing.effective - 4.0 * MAX_RATIO).abs() < 1e-9);
    }
}
