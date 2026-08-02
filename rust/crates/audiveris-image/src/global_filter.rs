// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::run_table::{BACKGROUND, FOREGROUND};

#[must_use]
pub fn global_filter(pixels: &[u8], threshold: u8) -> Vec<u8> {
    pixels
        .iter()
        .map(|&pixel| {
            if pixel <= threshold {
                FOREGROUND
            } else {
                BACKGROUND
            }
        })
        .collect()
}

#[must_use]
pub fn alpha_over_white(gray: u8, alpha: u8) -> u8 {
    let alpha = f64::from(alpha) / 255.0;
    let blended = (1.0 - alpha) * 255.0 + alpha * f64::from(gray);
    (blended + 0.5).clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_is_inclusive_like_java() {
        assert_eq!(
            global_filter(&[0, 126, 127, 128, 255], 127),
            [FOREGROUND, FOREGROUND, FOREGROUND, BACKGROUND, BACKGROUND]
        );
    }

    #[test]
    fn composites_transparency_over_white_like_buffered_source() {
        assert_eq!(alpha_over_white(0, 0), 255);
        assert_eq!(alpha_over_white(0, 255), 0);
        assert_eq!(alpha_over_white(100, 128), 177);
    }
}
