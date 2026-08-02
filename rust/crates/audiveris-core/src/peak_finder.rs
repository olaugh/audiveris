// SPDX-License-Identifier: AGPL-3.0-or-later

//! Peak detection based on hysteresis over an integer function's derivative.
//!
//! This is the dependency-free recognition core of Java's `HiLoPeakFinder`.
//! Chart construction is deliberately left to presentation code; the detected
//! derivative ranges, peaks, and replay thresholds are exposed as plain data.

use std::collections::{BTreeMap, HashMap};

use crate::{integer_function::IntegerFunction, range::Range};

/// A minimum peak value, optionally limited to an abscissa interval for display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quorum {
    pub y_min: i32,
    pub x_min: Option<i32>,
    pub x_max: Option<i32>,
}

impl Quorum {
    #[must_use]
    pub const fn new(y_min: i32) -> Self {
        Self {
            y_min,
            x_min: None,
            x_max: None,
        }
    }

    #[must_use]
    pub const fn in_domain(y_min: i32, x_min: i32, x_max: i32) -> Self {
        Self {
            y_min,
            x_min: Some(x_min),
            x_max: Some(x_max),
        }
    }
}

/// Finds value peaks by pairing strong positive and negative derivatives.
#[derive(Debug)]
pub struct HiLoPeakFinder<'a> {
    pub name: String,
    pub function: &'a IntegerFunction,
    pub x_min: i32,
    pub x_max: i32,
    min_gain_ratio: f64,
    min_value: i32,
    quorum: Option<Quorum>,
    min_derivative: i32,
    hilos: Vec<Range>,
    peaks: Vec<Range>,
}

impl<'a> HiLoPeakFinder<'a> {
    /// Creates a finder over the whole function domain.
    #[must_use]
    pub fn new(name: impl Into<String>, function: &'a IntegerFunction) -> Self {
        Self::with_domain(name, function, function.x_min(), function.x_max())
    }

    /// Creates a finder over a sub-domain of the function.
    ///
    /// # Panics
    ///
    /// Panics when the requested domain is not included in the function domain.
    #[must_use]
    pub fn with_domain(
        name: impl Into<String>,
        function: &'a IntegerFunction,
        x_min: i32,
        x_max: i32,
    ) -> Self {
        assert!(
            x_min >= function.x_min() && x_max <= function.x_max(),
            "PeakFinder domain not included in function domain"
        );

        Self {
            name: name.into(),
            function,
            x_min,
            x_max,
            min_gain_ratio: 0.0,
            min_value: 0,
            quorum: None,
            min_derivative: 0,
            hilos: Vec::new(),
            peaks: Vec::new(),
        }
    }

    pub const fn set_quorum(&mut self, quorum: Quorum) {
        self.quorum = Some(quorum);
    }

    pub const fn clear_quorum(&mut self) {
        self.quorum = None;
    }

    #[must_use]
    pub const fn quorum(&self) -> Option<Quorum> {
        self.quorum
    }

    /// Returns derivative HiLo ranges in increasing abscissa order.
    #[must_use]
    pub fn hilos(&self) -> &[Range] {
        &self.hilos
    }

    /// Returns detected peaks in decreasing value order.
    #[must_use]
    pub fn peaks(&self) -> &[Range] {
        &self.peaks
    }

    /// Finds peaks using the same derivative hysteresis and widening rules as Java.
    pub fn find_peaks(
        &mut self,
        min_value: i32,
        min_derivative: i32,
        min_gain_ratio: f64,
    ) -> &[Range] {
        self.min_value = min_value;
        self.min_derivative = min_derivative;
        self.min_gain_ratio = min_gain_ratio;
        self.peaks.clear();
        self.retrieve_hilos();

        // Java uses Range equality both in indexOf() and as HashMap keys.
        let mut hilo_to_peak = HashMap::<Range, Range>::new();
        let mut decreasing = self.hilos.clone();
        decreasing.sort_by(|left, right| {
            self.function
                .value(right.main)
                .cmp(&self.function.value(left.main))
        });

        for hilo in decreasing {
            let index = self
                .hilos
                .iter()
                .position(|candidate| *candidate == hilo)
                .expect("a sorted HiLo comes from the original collection");
            let mut peak_min = hilo.min.wrapping_sub(1).max(1);

            if index > 0 {
                let previous_hilo = self.hilos[index - 1];
                let previous_max = hilo_to_peak
                    .get(&previous_hilo)
                    .map_or(previous_hilo.max, |peak| peak.max);
                peak_min = peak_min.max(previous_max.wrapping_add(1));
            }

            let peak = self.create_peak(peak_min, hilo.main, hilo.max);
            hilo_to_peak.insert(hilo, peak);
            self.peaks.push(peak);
        }

        self.peaks.sort_by(|left, right| {
            self.function
                .value(right.main)
                .cmp(&self.function.value(left.main))
        });

        if let Some(quorum) = self.quorum {
            if let Some(first_below) = self
                .peaks
                .iter()
                .position(|peak| self.function.value(peak.main) < quorum.y_min)
            {
                self.peaks.truncate(first_below);
            }
        }

        &self.peaks
    }

    /// Replays peak widening and reports its threshold at every accepted x.
    #[must_use]
    pub fn peak_thresholds(&self, peak: Range) -> BTreeMap<i32, f64> {
        let mut thresholds = BTreeMap::new();
        let mut total = self.function.value(peak.main);
        let mut start = peak.main;
        let mut stop = peak.main;
        thresholds.insert(peak.main, 0.0);

        loop {
            let before = if start == peak.min {
                0
            } else {
                self.function.value(start.wrapping_sub(1))
            };
            let after = if stop == peak.max {
                0
            } else {
                self.function.value(stop.wrapping_add(1))
            };
            let gain = before.max(after);

            if gain == 0 {
                return thresholds;
            }

            total = total.wrapping_add(gain);
            if before > after {
                start = start.wrapping_sub(1);
                thresholds.insert(start, self.min_gain_ratio * f64::from(total));
            } else {
                stop = stop.wrapping_add(1);
                thresholds.insert(stop, self.min_gain_ratio * f64::from(total));
            }
        }
    }

    fn create_peak(&self, peak_min: i32, main: i32, peak_max: i32) -> Range {
        let mut total = self.function.value(main);
        let mut lower = main;
        let mut upper = main;

        loop {
            let before = if lower == peak_min {
                0
            } else {
                self.function.value(lower.wrapping_sub(1))
            };
            let after = if upper == peak_max {
                0
            } else {
                self.function.value(upper.wrapping_add(1))
            };
            let gain = before.max(after);
            let gain_ratio = f64::from(gain) / f64::from(total.wrapping_add(gain));

            // Java's `>=` is false if either operand is NaN.
            if gain_ratio.is_nan()
                || self.min_gain_ratio.is_nan()
                || gain_ratio < self.min_gain_ratio
            {
                break;
            }

            // This intentional asymmetry is inherited from Java: ties widen right.
            if before > after {
                lower = lower.wrapping_sub(1);
            } else {
                upper = upper.wrapping_add(1);
            }
            total = total.wrapping_add(gain);
        }

        Range::new(lower, main, upper)
    }

    fn retrieve_hilos(&mut self) {
        #[derive(Clone, Copy)]
        struct DerivativePeak {
            min: i32,
            max: i32,
            finished: bool,
        }

        self.hilos.clear();
        let mut high: Option<DerivativePeak> = None;
        let mut low: Option<DerivativePeak> = None;

        for x in self.x_min.wrapping_add(1)..=self.x_max {
            let value = self.function.value(x);
            let derivative = self.function.derivative(x);

            if derivative >= self.min_derivative {
                if let (Some(high_peak), Some(low_peak)) = (high, low) {
                    self.hilos.push(Range::new(
                        high_peak.min,
                        self.function.arg_max(high_peak.min, low_peak.max),
                        low_peak.max,
                    ));
                    high = None;
                    low = None;
                }

                match high {
                    None => {
                        high = Some(DerivativePeak {
                            min: x,
                            max: x,
                            finished: false,
                        });
                    }
                    Some(ref mut peak) if peak.finished => {
                        high = Some(DerivativePeak {
                            min: x,
                            max: x,
                            finished: false,
                        });
                    }
                    Some(ref mut peak) => peak.max = x,
                }
            } else if derivative <= -self.min_derivative {
                match low {
                    None if high.is_some() => {
                        low = Some(DerivativePeak {
                            min: x,
                            max: x,
                            finished: false,
                        });
                    }
                    Some(ref mut peak) => peak.max = x,
                    None => {}
                }
            } else if let (Some(high_peak), Some(low_peak)) = (high, low) {
                self.hilos.push(Range::new(
                    high_peak.min,
                    self.function.arg_max(high_peak.min, low_peak.max),
                    low_peak.max,
                ));
                high = None;
                low = None;
            } else if let Some(ref mut high_peak) = high {
                if value < self.min_value {
                    high = None;
                } else {
                    high_peak.finished = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_function(values: &[i32]) -> IntegerFunction {
        let mut function = IntegerFunction::new(0, i32::try_from(values.len() - 1).unwrap());
        for (x, value) in values.iter().copied().enumerate() {
            function.set_value(i32::try_from(x).unwrap(), value);
        }
        function
    }

    #[test]
    fn derivative_hysteresis_finishes_restarts_and_closes_on_new_high() {
        let function = build_function(&[0, 5, 6, 10, 6, 6, 11, 6, 11, 6, 6]);
        let mut finder = HiLoPeakFinder::new("hysteresis", &function);

        finder.find_peaks(1, 4, 1.0);

        assert_eq!(
            finder.hilos(),
            [
                Range::new(3, 3, 4),
                Range::new(6, 6, 7),
                Range::new(8, 8, 9)
            ]
        );

        // Java does not flush a pending Hi/Lo at the end of the domain.
        let terminal_function = build_function(&[0, 5, 0]);
        let mut terminal_finder = HiLoPeakFinder::new("terminal", &terminal_function);
        assert!(terminal_finder.find_peaks(1, 4, 1.0).is_empty());
        assert!(terminal_finder.hilos().is_empty());
    }

    #[test]
    fn widening_tie_goes_toward_larger_abscissa() {
        let function = build_function(&[0, 0, 5, 10, 5, 0]);
        let mut finder = HiLoPeakFinder::new("tie", &function);
        finder.min_gain_ratio = 0.3;

        assert_eq!(finder.create_peak(2, 3, 4), Range::new(3, 3, 4));
    }

    #[test]
    fn quorum_truncates_at_first_peak_below_threshold() {
        let function = build_function(&[0, 10, 0, 0, 8, 0, 0, 3, 0, 0]);
        let mut finder = HiLoPeakFinder::new("quorum", &function);
        finder.set_quorum(Quorum::new(4));

        assert_eq!(
            finder.find_peaks(1, 2, 1.0),
            [Range::new(1, 1, 1), Range::new(4, 4, 4)]
        );

        finder.set_quorum(Quorum::new(11));
        assert!(finder.find_peaks(1, 2, 1.0).is_empty());
    }

    #[test]
    fn peaks_are_value_ordered_and_ties_remain_abscissa_ordered() {
        let function = build_function(&[0, 3, 0, 0, 10, 0, 0, 10, 0, 0]);
        let mut finder = HiLoPeakFinder::new("ordering", &function);

        assert_eq!(
            finder.find_peaks(1, 2, 1.0),
            [
                Range::new(4, 4, 4),
                Range::new(7, 7, 7),
                Range::new(1, 1, 1),
            ]
        );
    }
}
