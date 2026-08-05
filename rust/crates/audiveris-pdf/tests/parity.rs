// SPDX-License-Identifier: AGPL-3.0-or-later

//! Every sample must equal Java2D's.
//!
//! The expectations in `oracle/java2d-bicubic.txt` come from
//! `oracle/java/Java2dBicubicProbe.java`, which draws the same patterns through
//! `Graphics2D.drawImage` with Audiveris's two rendering hints. Regenerate both
//! together if either changes.

use std::path::Path;

use audiveris_pdf::{GrayImage, scale_bicubic};

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

/// The same deterministic bilevel pattern the Java probe draws.
///
/// Isolated pixels and short runs are the point: a smooth ramp would agree
/// under almost any bicubic variant, and would not have caught the coefficient
/// table's derived tail or the fixed-point rounding.
fn pattern(width: usize, height: usize) -> GrayImage {
    let mut samples = vec![255u8; width * height];
    for down in 0..height {
        for across in 0..width {
            if (across * 7 + down * 13) % 11 < 4 {
                samples[down * width + across] = 0;
            }
        }
    }
    GrayImage {
        width,
        height,
        samples,
    }
}

#[test]
fn scaling_matches_java2d_across_geometries_and_ratios() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/java2d-bicubic.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));

    let mut checked = 0usize;
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        let (label, geometry, expected) = (fields[1], fields[2], fields[3]);
        // "<srcW>x<srcH>-><dstW>x<dstH>"
        let (source, destination) = geometry.split_once("->").expect("geometry");
        let dimensions = |text: &str| -> (usize, usize) {
            let (one, two) = text.split_once('x').expect("dimensions");
            (one.parse().expect("width"), two.parse().expect("height"))
        };
        let (source_width, source_height) = dimensions(source);
        let (width, height) = dimensions(destination);
        // "g<w>x<h>s<scale>" or "b<w>x<h>s<scale>"
        let scale: f64 = label
            .rsplit_once('s')
            .expect("label carries its scale")
            .1
            .parse()
            .expect("scale");

        let produced = scale_bicubic(
            &pattern(source_width, source_height),
            width,
            height,
            scale,
            scale,
            // PDFBox hands Java2D a destination that starts black, and Java2D
            // leaves every pixel outside the transformed image untouched.
            0,
        );
        assert_eq!(
            (produced.width, produced.height),
            (width, height),
            "{label}: geometry"
        );
        assert_eq!(
            format!("{:016x}", fnv1a64(&produced.samples)),
            expected,
            "{label}: raster differs from Java2D's"
        );
        checked += 1;
    }
    assert_eq!(checked, 112, "the oracle lost cases");
}
