// SPDX-License-Identifier: AGPL-3.0-or-later

//! Deterministic scale selection from vertical-run histograms.
//!
//! This module ports the decision-making part of Java's `ScaleBuilder`. The
//! histogram construction remains in [`crate::scale_runs`], while derivative
//! peak detection is shared with `audiveris-core`.

use std::fmt;

use audiveris_core::{
    integer_function::IntegerFunction, peak_finder::HiLoPeakFinder, range::Range,
};

use crate::scale_runs::VerticalRunHistograms;

/// Default production thresholds from Java `ScaleBuilder.Constants`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleThresholds {
    pub min_interline: i32,
    pub max_interline: i32,
    pub min_gain_ratio: f64,
    pub min_derivative_ratio: f64,
    pub min_black_ratio: f64,
    pub min_second_ratio: f64,
    pub max_second_ratio: f64,
    pub max_line_ratio: f64,
    pub beam_min_fraction: f64,
    pub beam_max_fraction: f64,
    pub beam_min_count_ratio: f64,
    pub beam_range_ratio: f64,
    pub beam_max_diff: i32,
    pub min_count_ratio: f64,
}

impl Default for ScaleThresholds {
    fn default() -> Self {
        Self {
            min_interline: 11,
            max_interline: 100,
            min_gain_ratio: 0.03,
            min_derivative_ratio: 0.025,
            min_black_ratio: 0.001,
            min_second_ratio: 1.2,
            max_second_ratio: 1.9,
            max_line_ratio: 0.5,
            beam_min_fraction: 0.45,
            beam_max_fraction: 0.95,
            beam_min_count_ratio: 0.01,
            beam_range_ratio: 0.25,
            beam_max_diff: 2,
            min_count_ratio: 0.4,
        }
    }
}

/// User inputs and contextual values that are not encoded in the histograms.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ScaleOptions {
    /// A non-zero value replaces the detected interline, as in Java.
    pub specified_interline: Option<i32>,
    /// A non-zero value replaces measured or guessed beam thickness.
    pub specified_beam: Option<i32>,
    /// Enables Java's almost-blank-page check when the image size is known.
    pub image_size: Option<(usize, usize)>,
    pub thresholds: ScaleThresholds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineScale {
    pub min: i32,
    pub main: i32,
    pub max: i32,
}

impl From<Range> for LineScale {
    fn from(range: Range) -> Self {
        Self {
            min: range.min,
            main: range.main,
            max: range.max,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterlineScale {
    pub min: i32,
    pub main: i32,
    pub max: i32,
}

impl From<Range> for InterlineScale {
    fn from(range: Range) -> Self {
        Self {
            min: range.min,
            main: range.main,
            max: range.max,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeamScale {
    pub main: i32,
    pub extrapolated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionStatus {
    Specified,
    TooLow,
    Accepted,
    TooHigh,
}

/// The scale plus the intermediate decisions needed for parity diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaleEstimate {
    pub interline: InterlineScale,
    pub small_interline: Option<InterlineScale>,
    pub line: LineScale,
    pub beam: BeamScale,
    pub small_beam: Option<BeamScale>,
    pub resolution: ResolutionStatus,
    pub primary_combo_peak: Range,
    pub secondary_combo_peak: Option<Range>,
    pub significant_beam_key: Option<i32>,
    pub beam_quorum: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScaleEstimateError {
    EmptyHistogram(&'static str),
    CountOverflow {
        histogram: &'static str,
        count: usize,
    },
    ImageAreaOverflow,
    AlmostBlank,
    NoRegularlySpacedLines,
    NoSignificantBlackLines,
}

impl fmt::Display for ScaleEstimateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyHistogram(name) => write!(f, "empty {name} histogram"),
            Self::CountOverflow { histogram, count } => {
                write!(f, "{histogram} histogram count {count} exceeds i32")
            }
            Self::ImageAreaOverflow => write!(f, "image area exceeds usize"),
            Self::AlmostBlank => write!(f, "too few black pixels"),
            Self::NoRegularlySpacedLines => write!(f, "no regularly spaced lines found"),
            Self::NoSignificantBlackLines => write!(f, "no significant black lines found"),
        }
    }
}

impl std::error::Error for ScaleEstimateError {}

/// Reproduces the scale decisions made by Java `ScaleBuilder`.
pub fn estimate_scale(
    histograms: &VerticalRunHistograms,
    options: ScaleOptions,
) -> Result<ScaleEstimate, ScaleEstimateError> {
    let black_function = histogram_function("black", &histograms.black)?;
    let combo_function = histogram_function("combo", &histograms.combo)?;
    let thresholds = options.thresholds;

    let specified_interline = options.specified_interline.filter(|value| *value != 0);
    let (combo_peak, combo_peak2) = if let Some(interline) = specified_interline {
        // Java still runs combo peak detection before honoring this override. A
        // synthetic peak avoids its null dereference when two black peaks exist.
        (Range::new(interline, interline, interline), None)
    } else {
        retrieve_interline_peaks(&combo_function, thresholds)?
    };

    // Java validates combo spacing before checking whether the page is blank.
    if let Some((width, height)) = options.image_size {
        let image_area = width
            .checked_mul(height)
            .ok_or(ScaleEstimateError::ImageAreaOverflow)?;
        let black_ratio = histograms.black_pixel_count() as f64 / image_area as f64;
        if black_ratio < thresholds.min_black_ratio {
            return Err(ScaleEstimateError::AlmostBlank);
        }
    }

    let (black_peak, significant_beam_key) =
        retrieve_line_peak(&black_function, combo_peak, thresholds)?;
    let beam_quorum =
        round_to_i32(f64::from(black_function.area()) * thresholds.beam_min_count_ratio);
    let beams = compute_beams(
        &black_function,
        black_peak,
        combo_peak,
        combo_peak2,
        significant_beam_key,
        specified_interline,
        options.specified_beam.filter(|value| *value != 0),
        thresholds,
    );

    let (interline, small_interline, resolution) = if let Some(interline) = specified_interline {
        (
            InterlineScale {
                min: interline,
                main: interline,
                max: interline,
            },
            None,
            ResolutionStatus::Specified,
        )
    } else {
        let resolution = if combo_peak.main < thresholds.min_interline {
            ResolutionStatus::TooLow
        } else if combo_peak.main > thresholds.max_interline {
            ResolutionStatus::TooHigh
        } else {
            ResolutionStatus::Accepted
        };
        let (normal, small) = order_interlines(combo_peak, combo_peak2);
        (normal.into(), small.map(Into::into), resolution)
    };

    Ok(ScaleEstimate {
        interline,
        small_interline,
        line: black_peak.into(),
        beam: beams.normal,
        small_beam: beams.small,
        resolution,
        primary_combo_peak: combo_peak,
        secondary_combo_peak: combo_peak2,
        significant_beam_key,
        beam_quorum,
    })
}

fn histogram_function(
    name: &'static str,
    counts: &[usize],
) -> Result<IntegerFunction, ScaleEstimateError> {
    let x_max = counts
        .len()
        .checked_sub(1)
        .ok_or(ScaleEstimateError::EmptyHistogram(name))?;
    let x_max = i32::try_from(x_max).map_err(|_| ScaleEstimateError::CountOverflow {
        histogram: name,
        count: x_max,
    })?;
    let mut function = IntegerFunction::new(0, x_max);
    for (x, count) in counts.iter().copied().enumerate() {
        let value = i32::try_from(count).map_err(|_| ScaleEstimateError::CountOverflow {
            histogram: name,
            count,
        })?;
        function.set_value(i32::try_from(x).expect("validated histogram domain"), value);
    }
    Ok(function)
}

fn retrieve_interline_peaks(
    function: &IntegerFunction,
    thresholds: ScaleThresholds,
) -> Result<(Range, Option<Range>), ScaleEstimateError> {
    let mut finder = HiLoPeakFinder::new("combo", function);
    let derivative = round_to_i32(f64::from(function.area()) * thresholds.min_derivative_ratio);
    let peaks = finder
        .find_peaks(1, derivative, thresholds.min_gain_ratio)
        .to_vec();
    select_interline_peaks(peaks, thresholds)
}

fn select_interline_peaks(
    peaks: Vec<Range>,
    thresholds: ScaleThresholds,
) -> Result<(Range, Option<Range>), ScaleEstimateError> {
    let Some(mut primary) = peaks.first().copied() else {
        return Err(ScaleEstimateError::NoRegularlySpacedLines);
    };
    let mut survivors = Vec::new();

    for peak in peaks.into_iter().skip(1) {
        let min = peak.main.min(primary.main);
        let max = peak.main.max(primary.main);
        let ratio = f64::from(max) / f64::from(min);
        if ratio > thresholds.max_second_ratio {
            continue;
        }
        if ratio < thresholds.min_second_ratio {
            primary = Range::new(
                peak.min.min(primary.min),
                peak.main.wrapping_add(primary.main) / 2,
                peak.max.max(primary.max),
            );
            continue;
        }
        survivors.push(peak);
    }

    Ok((primary, survivors.first().copied()))
}

fn retrieve_line_peak(
    function: &IntegerFunction,
    combo_peak: Range,
    thresholds: ScaleThresholds,
) -> Result<(Range, Option<i32>), ScaleEstimateError> {
    let mut finder = HiLoPeakFinder::new("black", function);
    let derivative = round_to_i32(f64::from(function.area()) * thresholds.min_derivative_ratio);
    let peaks = finder
        .find_peaks(1, derivative, thresholds.min_gain_ratio)
        .to_vec();
    select_line_peak(function, peaks, combo_peak, thresholds)
}

fn select_line_peak(
    function: &IntegerFunction,
    mut peaks: Vec<Range>,
    combo_peak: Range,
    thresholds: ScaleThresholds,
) -> Result<(Range, Option<i32>), ScaleEstimateError> {
    let Some(first) = peaks.first() else {
        return Err(ScaleEstimateError::NoSignificantBlackLines);
    };

    if peaks.len() > 1 {
        let min_count =
            round_to_i32(f64::from(function.value(first.main)) * thresholds.min_count_ratio);
        if let Some(first_below) = peaks
            .iter()
            .skip(1)
            .position(|peak| function.value(peak.main) < min_count)
        {
            peaks.truncate(first_below + 1);
        }
    }

    if peaks.len() == 1 {
        return Ok((peaks[0], None));
    }

    // Java's stable object sort preserves count order when main keys tie.
    peaks.sort_by_key(|peak| peak.main);
    let p0 = peaks[0];
    let p1 = peaks[1];
    let max_line_key = round_to_i32(f64::from(combo_peak.main) * thresholds.max_line_ratio);
    if p1.main <= max_line_key {
        Ok((Range::new(p0.min, p1.main, p1.max), None))
    } else {
        Ok((p0, Some(p1.main)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BeamScales {
    normal: BeamScale,
    small: Option<BeamScale>,
}

#[allow(clippy::too_many_arguments)]
fn compute_beams(
    black_function: &IntegerFunction,
    black_peak: Range,
    combo_peak: Range,
    combo_peak2: Option<Range>,
    significant_beam_key: Option<i32>,
    specified_interline: Option<i32>,
    specified_beam: Option<i32>,
    thresholds: ScaleThresholds,
) -> BeamScales {
    if let Some(main) = specified_beam {
        return BeamScales {
            normal: BeamScale {
                main,
                extrapolated: false,
            },
            small: None,
        };
    }

    if let Some(main) = significant_beam_key {
        return BeamScales {
            normal: BeamScale {
                main,
                extrapolated: false,
            },
            small: None,
        };
    }

    let larger_interline = specified_interline.unwrap_or_else(|| {
        combo_peak2.map_or(combo_peak.main, |second| combo_peak.main.max(second.main))
    });
    let min_height = black_peak.max.max(round_to_i32(
        thresholds.beam_min_fraction * f64::from(larger_interline),
    ));
    let max_height = (larger_interline - black_peak.min / 2).min(round_to_i32(
        thresholds.beam_max_fraction * f64::from(larger_interline),
    ));
    let quorum = round_to_i32(f64::from(black_function.area()) * thresholds.beam_min_count_ratio);
    let guess =
        min_height + round_to_i32(thresholds.beam_range_ratio * f64::from(max_height - min_height));
    let peaks = black_function.local_maxima(min_height, max_height);
    let mut beam_key = None;
    let mut beam_key2 = None;

    if let Some(&peak) = peaks.first() {
        let quorum_ratio = f64::from(black_function.value(peak)) / f64::from(quorum);
        if quorum_ratio >= 1.0 || (peak - guess).abs() <= thresholds.beam_max_diff {
            beam_key = Some(peak);
        }

        if combo_peak2.is_some() && peaks.len() > 1 {
            let peak2 = peaks[1];
            let quorum_ratio2 = f64::from(black_function.value(peak2)) / f64::from(quorum);
            if quorum_ratio2 >= 1.0 {
                beam_key2 = Some(peak2);
            }
        }
    }

    match (beam_key, beam_key2) {
        (Some(first), Some(second)) => BeamScales {
            normal: BeamScale {
                main: first.max(second),
                extrapolated: false,
            },
            small: Some(BeamScale {
                main: first.min(second),
                extrapolated: false,
            }),
        },
        (Some(main), None) => BeamScales {
            normal: BeamScale {
                main,
                extrapolated: false,
            },
            small: None,
        },
        (None, _) => BeamScales {
            normal: BeamScale {
                main: guess,
                extrapolated: true,
            },
            small: None,
        },
    }
}

fn order_interlines(primary: Range, secondary: Option<Range>) -> (Range, Option<Range>) {
    match secondary {
        None => (primary, None),
        Some(second) if second.main < primary.main => (primary, Some(second)),
        Some(second) => (second, Some(primary)),
    }
}

fn round_to_i32(value: f64) -> i32 {
    value.round_ties_even() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function(x_max: i32, values: &[(i32, i32)]) -> IntegerFunction {
        let mut function = IntegerFunction::new(0, x_max);
        for &(x, value) in values {
            function.set_value(x, value);
        }
        function
    }

    #[test]
    fn second_interline_peaks_are_merged_kept_or_discarded_in_sequence() {
        let peaks = vec![
            Range::new(19, 20, 21),
            Range::new(21, 22, 23),
            Range::new(49, 50, 51),
            Range::new(29, 30, 31),
        ];
        let selected = select_interline_peaks(peaks, ScaleThresholds::default()).unwrap();
        assert_eq!(selected.0, Range::new(19, 21, 23));
        assert_eq!(selected.1, Some(Range::new(29, 30, 31)));
    }

    #[test]
    fn line_peaks_merge_below_half_interline() {
        let black = function(20, &[(3, 90), (4, 100)]);
        let peaks = vec![Range::new(4, 4, 5), Range::new(2, 3, 3)];
        let selected = select_line_peak(
            &black,
            peaks,
            Range::new(19, 20, 21),
            ScaleThresholds::default(),
        )
        .unwrap();
        assert_eq!(selected, (Range::new(2, 4, 5), None));
    }

    #[test]
    fn line_peak_filtering_prevents_a_weak_peak_becoming_a_beam() {
        let black = function(20, &[(2, 100), (12, 39)]);
        let peaks = vec![Range::new(1, 2, 3), Range::new(11, 12, 13)];
        let selected = select_line_peak(
            &black,
            peaks,
            Range::new(19, 20, 21),
            ScaleThresholds::default(),
        )
        .unwrap();
        assert_eq!(selected, (Range::new(1, 2, 3), None));
    }

    #[test]
    fn significant_second_black_peak_is_a_measured_beam() {
        let black = function(20, &[(2, 100), (12, 80)]);
        let peaks = vec![Range::new(1, 2, 3), Range::new(11, 12, 13)];
        let selected = select_line_peak(
            &black,
            peaks,
            Range::new(19, 20, 21),
            ScaleThresholds::default(),
        )
        .unwrap();
        assert_eq!(selected, (Range::new(1, 2, 3), Some(12)));
    }

    #[test]
    fn beam_local_maximum_is_measured_when_it_reaches_quorum() {
        let black = function(20, &[(2, 100), (13, 5)]);
        let beams = compute_beams(
            &black,
            Range::new(1, 2, 3),
            Range::new(19, 20, 21),
            None,
            None,
            None,
            None,
            ScaleThresholds::default(),
        );
        assert_eq!(
            beams.normal,
            BeamScale {
                main: 13,
                extrapolated: false
            }
        );
        assert_eq!(beams.small, None);
    }

    #[test]
    fn missing_beam_measurement_uses_range_guess() {
        let black = function(20, &[(2, 100)]);
        let beams = compute_beams(
            &black,
            Range::new(1, 2, 3),
            Range::new(19, 20, 21),
            None,
            None,
            None,
            None,
            ScaleThresholds::default(),
        );
        assert_eq!(
            beams.normal,
            BeamScale {
                main: 11,
                extrapolated: true
            }
        );
    }

    #[test]
    fn two_interline_populations_allow_two_measured_beams() {
        let black = function(30, &[(2, 100), (12, 7), (14, 6)]);
        let beams = compute_beams(
            &black,
            Range::new(1, 2, 3),
            Range::new(15, 16, 17),
            Some(Range::new(23, 24, 25)),
            None,
            None,
            None,
            ScaleThresholds::default(),
        );
        assert_eq!(beams.normal.main, 14);
        assert_eq!(beams.small.unwrap().main, 12);
    }

    #[test]
    fn complete_estimate_detects_line_interline_and_guessed_beam() {
        let mut black = vec![0; 11];
        black[2] = 100;
        let mut combo = vec![0; 31];
        combo[20] = 100;
        let histograms = VerticalRunHistograms {
            max_black: 10,
            max_white: 20,
            black,
            combo,
        };

        let scale = estimate_scale(&histograms, ScaleOptions::default()).unwrap();
        assert_eq!(
            scale.line,
            LineScale {
                min: 2,
                main: 2,
                max: 2
            }
        );
        assert_eq!(
            scale.interline,
            InterlineScale {
                min: 20,
                main: 20,
                max: 20
            }
        );
        assert_eq!(scale.small_interline, None);
        assert!(scale.beam.extrapolated);
        assert_eq!(scale.beam.main, 11);
        assert_eq!(scale.resolution, ResolutionStatus::Accepted);
    }

    #[test]
    fn explicit_scales_override_detection_and_are_not_extrapolated() {
        let histograms = VerticalRunHistograms {
            max_black: 8,
            max_white: 25,
            black: vec![0, 0, 100, 0, 0, 0, 0, 0, 0],
            combo: vec![0; 34],
        };
        let scale = estimate_scale(
            &histograms,
            ScaleOptions {
                specified_interline: Some(18),
                specified_beam: Some(8),
                ..ScaleOptions::default()
            },
        )
        .unwrap();
        assert_eq!(scale.interline.main, 18);
        assert_eq!(scale.resolution, ResolutionStatus::Specified);
        assert_eq!(
            scale.beam,
            BeamScale {
                main: 8,
                extrapolated: false
            }
        );
    }
}
