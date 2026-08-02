// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::run_table::{BACKGROUND, FOREGROUND};

pub const DEFAULT_HALF_WINDOW: usize = 18;
pub const DEFAULT_MEAN_COEFF: f64 = 0.7;
pub const DEFAULT_STD_DEV_COEFF: f64 = 0.9;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdaptiveSettings {
    pub half_window: usize,
    pub mean_coeff: f64,
    pub std_dev_coeff: f64,
}

impl Default for AdaptiveSettings {
    fn default() -> Self {
        Self {
            half_window: DEFAULT_HALF_WINDOW,
            mean_coeff: DEFAULT_MEAN_COEFF,
            std_dev_coeff: DEFAULT_STD_DEV_COEFF,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdaptiveContext {
    pub mean: f64,
    pub standard_deviation: f64,
    pub threshold: f64,
}

#[derive(Debug, Clone)]
struct Integrals {
    stride: usize,
    plain: Vec<u64>,
    squared: Vec<u64>,
}

impl Integrals {
    fn new(width: usize, height: usize, pixels: &[u8]) -> Self {
        assert_eq!(pixels.len(), width * height);
        let stride = width + 1;
        let mut plain = vec![0u64; stride * (height + 1)];
        let mut squared = vec![0u64; stride * (height + 1)];
        for y in 0..height {
            let mut row = 0u64;
            let mut square_row = 0u64;
            for x in 0..width {
                let value = u64::from(pixels[y * width + x]);
                row += value;
                square_row += value * value;
                let index = (y + 1) * stride + x + 1;
                plain[index] = plain[y * stride + x + 1] + row;
                squared[index] = squared[y * stride + x + 1] + square_row;
            }
        }
        Self {
            stride,
            plain,
            squared,
        }
    }

    fn sum(&self, table: &[u64], x0: usize, y0: usize, x1: usize, y1: usize) -> u64 {
        let a = table[y0 * self.stride + x0];
        let b = table[y0 * self.stride + x1];
        let c = table[y1 * self.stride + x0];
        let d = table[y1 * self.stride + x1];
        (a + d) - b - c
    }

    fn context(
        &self,
        width: usize,
        height: usize,
        x: usize,
        y: usize,
        settings: AdaptiveSettings,
    ) -> AdaptiveContext {
        let x0 = x.saturating_sub(settings.half_window);
        let y0 = y.saturating_sub(settings.half_window);
        let x1 = (x + settings.half_window + 1).min(width);
        let y1 = (y + settings.half_window + 1).min(height);
        let area = (x1 - x0) * (y1 - y0);
        let mean = self.sum(&self.plain, x0, y0, x1, y1) as f64 / area as f64;
        let squared_mean = self.sum(&self.squared, x0, y0, x1, y1) as f64 / area as f64;
        let variance = (squared_mean - mean * mean).abs();
        let standard_deviation = variance.sqrt();
        let threshold = settings.mean_coeff * mean + settings.std_dev_coeff * standard_deviation;
        AdaptiveContext {
            mean,
            standard_deviation,
            threshold,
        }
    }
}

#[must_use]
pub fn adaptive_filter(
    width: usize,
    height: usize,
    pixels: &[u8],
    half_window: usize,
    mean_coeff: f64,
    std_dev_coeff: f64,
) -> Vec<u8> {
    let integrals = Integrals::new(width, height, pixels);
    let settings = AdaptiveSettings {
        half_window,
        mean_coeff,
        std_dev_coeff,
    };
    pixels
        .iter()
        .enumerate()
        .map(|(index, &pixel)| {
            let context = integrals.context(width, height, index % width, index / width, settings);
            if f64::from(pixel) <= context.threshold {
                FOREGROUND
            } else {
                BACKGROUND
            }
        })
        .collect()
}

#[must_use]
pub fn default_adaptive_filter(width: usize, height: usize, pixels: &[u8]) -> Vec<u8> {
    adaptive_filter(
        width,
        height,
        pixels,
        DEFAULT_HALF_WINDOW,
        DEFAULT_MEAN_COEFF,
        DEFAULT_STD_DEV_COEFF,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_clipped_window_context() {
        let integrals = Integrals::new(3, 3, &[0, 10, 20, 30, 40, 50, 60, 70, 80]);
        let settings = AdaptiveSettings {
            half_window: 1,
            mean_coeff: 0.7,
            std_dev_coeff: 0.9,
        };
        let corner = integrals.context(3, 3, 0, 0, settings);
        assert_eq!(corner.mean, 20.0);
        assert!((corner.standard_deviation - 15.811_388_300_841_896).abs() < 1e-12);
        let center = integrals.context(3, 3, 1, 1, settings);
        assert_eq!(center.mean, 40.0);
    }

    #[test]
    fn uses_inclusive_foreground_threshold() {
        let pixels = [100; 9];
        assert_eq!(adaptive_filter(3, 3, &pixels, 1, 1.0, 0.0), [0; 9]);
    }

    #[test]
    fn separates_dark_marks_from_white_field() {
        let mut pixels = [255; 25];
        pixels[12] = 0;
        let result = default_adaptive_filter(5, 5, &pixels);
        assert_eq!(result[12], FOREGROUND);
        assert_eq!(result[0], BACKGROUND);
    }
}
