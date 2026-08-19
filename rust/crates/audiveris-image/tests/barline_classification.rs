// SPDX-License-Identifier: AGPL-3.0-or-later

//! Barline-form classification against Verovio-engraved ground truth.
//!
//! Every `truth` entry in `verovio-synthetic.txt` carries the engraver's
//! form label (single, double, final, rptstart, rptend); the classifier
//! must reproduce it from the rendered ink alone, at pixel interlines 6,
//! 12, and 18.  Real-scan spot checks live in the sibling test below.

use std::path::{Path, PathBuf};

use audiveris_image::bar_tuning::{
    BarClassificationParameters, BarlineForm, SystemBand, classify_boundary, volta_hook,
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

#[test]
fn volta_hooks_match_the_engraver_on_the_verovio_corpus() {
    let Ok(workspace) = std::env::var("AUDIVERIS_BARLINE_TUNING_FIXTURES") else {
        eprintln!(
            "SKIPPED volta detection: set AUDIVERIS_BARLINE_TUNING_FIXTURES to the workspace \
             root containing stage-omr-data"
        );
        return;
    };
    let workspace = PathBuf::from(workspace);
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/py-barline-tuning/verovio-synthetic.txt");
    let text = std::fs::read_to_string(fixture).expect("fixture readable");
    // First pass groups the fixture per system so volta truth is complete
    // before boundaries are graded.
    struct System {
        id: String,
        image: PathBuf,
        band: SystemBand,
        interline: f64,
        truths: Vec<f64>,
        hooks: Vec<f64>,
    }
    let mut systems: Vec<System> = Vec::new();
    let mut image = PathBuf::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        match fields.first().copied() {
            Some("image") => image = workspace.join(fields[1]),
            Some("system") => systems.push(System {
                id: fields[1].to_string(),
                image: image.clone(),
                band: SystemBand {
                    left: fields[2].parse().unwrap(),
                    right: fields[3].parse().unwrap(),
                    top: fields[4].parse().unwrap(),
                    bottom: fields[5].parse().unwrap(),
                },
                interline: fields[6].parse().unwrap(),
                truths: Vec::new(),
                hooks: Vec::new(),
            }),
            Some("truth") => systems
                .last_mut()
                .unwrap()
                .truths
                .push(fields[1].parse().unwrap()),
            Some("volta") => systems
                .last_mut()
                .unwrap()
                .hooks
                .push(fields[1].parse().unwrap()),
            _ => {}
        }
    }
    let mut raster_cache: Option<(PathBuf, GrayRaster)> = None;
    let mut positives = 0usize;
    let mut negatives = 0usize;
    for system in &systems {
        let gray = match &raster_cache {
            Some((path, raster)) if path == &system.image => raster.clone(),
            _ => {
                let dynamic = image::open(&system.image)
                    .unwrap_or_else(|error| panic!("open {:?}: {error}", system.image));
                let raster = GrayRaster::from_dynamic(&dynamic);
                raster_cache = Some((system.image.clone(), raster.clone()));
                raster
            }
        };
        let parameters = BarClassificationParameters::from_scale(system.interline);
        for x in &system.truths {
            let expected = system
                .hooks
                .iter()
                .any(|hook| (hook - x).abs() <= parameters.volta_search);
            let detected = volta_hook(&gray, &system.band, *x, &parameters);
            assert_eq!(
                detected, expected,
                "{} x={x}: volta hook (truth hooks {:?})",
                system.id, system.hooks
            );
            if expected {
                positives += 1;
            } else {
                negatives += 1;
            }
        }
    }
    assert!(positives >= 6, "corpus exercises volta positives");
    eprintln!("volta detection: {positives} positives and {negatives} negatives reproduced");
}

#[test]
fn volta_hooks_match_hand_verified_scan_boundaries() {
    let Ok(workspace) = std::env::var("AUDIVERIS_BARLINE_TUNING_FIXTURES") else {
        eprintln!(
            "SKIPPED volta detection (scans): set AUDIVERIS_BARLINE_TUNING_FIXTURES to the \
             workspace root containing stage-omr-data"
        );
        return;
    };
    let workspace = PathBuf::from(workspace);
    // GGR p2s5: the first ending opens at the projection-only barline near
    // x=299 (the pinned interline residual) and the second ending begins at
    // the repeat close near x=540; the Grazioso bars carry no volta.  The
    // Schenker cases put ties and slurs directly above genuine barlines —
    // the classic false-positive source for hook detection.
    type VoltaCase = (&'static str, [f64; 4], f64, f64, bool, &'static str);
    let cases: &[VoltaCase] = &[
        (
            "stage-omr-data/data/real-datasets/ggr-warped/p2s5.png",
            [74.0, 1483.0, 79.0, 275.0],
            12.0,
            299.0,
            true,
            "first ending opens",
        ),
        (
            "stage-omr-data/data/real-datasets/ggr-warped/p2s5.png",
            [74.0, 1483.0, 79.0, 275.0],
            12.0,
            540.0,
            // The second ending is text-marked ("||2.", no bracket), but
            // the FIRST ending's bracket line closes at this repeat bar -
            // genuine ending evidence at this boundary either way.
            true,
            "first ending's bracket closes at the repeat bar",
        ),
        (
            "stage-omr-data/data/real-datasets/ggr-warped/p2s5.png",
            [74.0, 1483.0, 79.0, 275.0],
            12.0,
            818.5,
            false,
            "Grazioso interior bar, no volta",
        ),
        (
            "stage-omr-data/data/real-datasets/schenker-beethoven/pages/sonata-01/page-05.png",
            [38.0, 716.0, 707.0, 801.0],
            6.0,
            600.0,
            false,
            "genuine barline with a tie arcing directly above",
        ),
        (
            "stage-omr-data/data/real-datasets/schenker-beethoven/pages/sonata-01/page-05.png",
            [39.0, 713.0, 80.0, 168.0],
            6.0,
            339.0,
            false,
            "recovered barline with slurs above",
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
        assert_eq!(
            volta_hook(&gray, &band, *x, &parameters),
            *expected,
            "{image_rel} x={x} ({note})"
        );
    }
    eprintln!("volta detection: 5 hand-verified scan cases reproduced");
}
