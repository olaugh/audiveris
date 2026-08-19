// SPDX-License-Identifier: AGPL-3.0-or-later

//! Barline-form classification against Verovio-engraved ground truth.
//!
//! Every `truth` entry in `verovio-synthetic.txt` carries the engraver's
//! form label (single, double, final, rptstart, rptend); the classifier
//! must reproduce it from the rendered ink alone, at pixel interlines 6,
//! 12, and 18.  Real-scan spot checks live in the sibling test below.

use std::path::{Path, PathBuf};

use audiveris_image::bar_tuning::{
    BarClassificationParameters, BarlineForm, SystemBand, classify_boundary,
};
use audiveris_image::ingest::GrayRaster;

fn form_label(form: BarlineForm) -> &'static str {
    match form {
        BarlineForm::Unknown => "unknown",
        BarlineForm::Single => "single",
        BarlineForm::Double => "double",
        BarlineForm::Final => "final",
        BarlineForm::RepeatStart => "rptstart",
        BarlineForm::RepeatEnd => "rptend",
        BarlineForm::RepeatBoth => "rptboth",
    }
}

#[test]
fn classification_matches_engraver_forms_on_the_verovio_corpus() {
    let Ok(workspace) = std::env::var("AUDIVERIS_BARLINE_TUNING_FIXTURES") else {
        eprintln!(
            "SKIPPED barline_classification: set AUDIVERIS_BARLINE_TUNING_FIXTURES to the \
             workspace root containing stage-omr-data"
        );
        return;
    };
    let workspace = PathBuf::from(workspace);
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/py-barline-tuning/verovio-synthetic.txt");
    let text = std::fs::read_to_string(fixture).expect("fixture readable");
    let mut raster: Option<GrayRaster> = None;
    let mut band = SystemBand {
        left: 0.0,
        right: 0.0,
        top: 0.0,
        bottom: 0.0,
    };
    let mut interline = 0.0_f64;
    let mut system_id = String::new();
    let mut checked = 0usize;
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        match fields.first().copied() {
            Some("image") => {
                let image = workspace.join(fields[1]);
                let dynamic =
                    image::open(&image).unwrap_or_else(|error| panic!("open {image:?}: {error}"));
                raster = Some(GrayRaster::from_dynamic(&dynamic));
            }
            Some("system") => {
                system_id = fields[1].to_string();
                band = SystemBand {
                    left: fields[2].parse().unwrap(),
                    right: fields[3].parse().unwrap(),
                    top: fields[4].parse().unwrap(),
                    bottom: fields[5].parse().unwrap(),
                };
                interline = fields[6].parse().unwrap();
            }
            Some("truth") => {
                let x: f64 = fields[1].parse().unwrap();
                let expected = fields[2];
                let parameters = BarClassificationParameters::from_scale(interline);
                let classified = classify_boundary(raster.as_ref().unwrap(), &band, x, &parameters);
                assert_eq!(
                    form_label(classified.form),
                    expected,
                    "{system_id} x={x}: strokes {:?}, dot densities L {:.2} R {:.2}",
                    classified.strokes,
                    classified.left_dot_density,
                    classified.right_dot_density,
                );
                checked += 1;
            }
            _ => {}
        }
    }
    assert_eq!(checked, 60, "every labeled barline classified");
    eprintln!("barline_classification: {checked} engraved forms reproduced");
}

#[test]
fn classification_matches_hand_verified_scan_boundaries() {
    let Ok(workspace) = std::env::var("AUDIVERIS_BARLINE_TUNING_FIXTURES") else {
        eprintln!(
            "SKIPPED barline_classification (scans): set AUDIVERIS_BARLINE_TUNING_FIXTURES to \
             the workspace root containing stage-omr-data"
        );
        return;
    };
    let workspace = PathBuf::from(workspace);
    // Each case was verified by eye on the scan before being encoded here.
    type ScanCase = (&'static str, [f64; 4], f64, f64, BarlineForm, &'static str);
    let cases: &[ScanCase] = &[
        (
            "stage-omr-data/data/real-datasets/ggr-warped/p1s1.png",
            [78.0, 1448.0, 99.0, 301.0],
            12.0,
            609.0,
            BarlineForm::RepeatStart,
            "section A opens its repeat: thick+thin with dots right",
        ),
        (
            "stage-omr-data/data/real-datasets/ggr-warped/p2s5.png",
            [74.0, 1483.0, 79.0, 275.0],
            12.0,
            540.0,
            BarlineForm::RepeatEnd,
            "first ending closes: dots left of the stroke pair",
        ),
        (
            "stage-omr-data/data/real-datasets/ggr-warped/p4s5.png",
            [74.0, 1486.0, 75.0, 295.0],
            12.0,
            1482.0,
            // Verified at high zoom: this edition closes the system with a
            // single stroke (an earlier hand label guessed thin+thick).
            // Under the page-edge warp the stroke only resolves through the
            // shear scan; the check pins that no second stroke is invented.
            BarlineForm::Single,
            "system-end bar under the page-edge warp: one stroke, no dots",
        ),
        (
            "stage-omr-data/data/real-datasets/schenker-beethoven/pages/sonata-01/page-05.png",
            [36.0, 715.0, 872.0, 965.0],
            6.0,
            712.0,
            BarlineForm::Final,
            "Sonata 1 movement I final double bar (thin 710 + thick 715)",
        ),
    ];
    for (image_rel, band, interline, x, expected, note) in cases {
        let path = workspace.join(image_rel);
        let dynamic = image::open(&path).unwrap_or_else(|error| panic!("open {path:?}: {error}"));
        let gray = GrayRaster::from_dynamic(&dynamic);
        let band = SystemBand {
            left: band[0],
            right: band[1],
            top: band[2],
            bottom: band[3],
        };
        let parameters = BarClassificationParameters::from_scale(*interline);
        let classified = classify_boundary(&gray, &band, *x, &parameters);
        assert_eq!(
            classified.form, *expected,
            "{image_rel} x={x} ({note}): strokes {:?}, dot densities L {:.2} R {:.2}",
            classified.strokes, classified.left_dot_density, classified.right_dot_density,
        );
    }
    eprintln!("barline_classification: 4 hand-verified scan boundaries reproduced");
}
