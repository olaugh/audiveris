// SPDX-License-Identifier: AGPL-3.0-or-later

//! Ground-truth validation of the `bar_tuning` engine on synthetic scores.
//!
//! Unlike the scan-corpus parity tests (whose reference is the Python
//! oracle, itself recognition output), this fixture's answers are *known*:
//! MEI rendered by Verovio, with barline positions and forms parsed from
//! the SVG the engraver itself emitted.  Each piece is rasterized at pixel
//! interlines 6, 12, and 18, so the interline-derived parameter set is
//! validated at scales the scan corpora do not cover.  Regeneration:
//! `stage-omr-data/scripts/generate_verovio_barline_corpus.py` (see the
//! fixture header).
//!
//! The engine receives *no* raw boundaries (only synthetic system edges):
//! every interior barline must be recovered by grayscale projection alone,
//! and the aligned-stem piece must not fabricate one.

use std::path::{Path, PathBuf};

use audiveris_image::bar_tuning::{
    BarTuningParameters, RawBoundary, RawBoundaryKind, SystemBand, SystemBarInput, geometry_count,
    projection_candidates, resolve_count, select_boundaries,
};
use audiveris_image::ingest::GrayRaster;

struct TruthSystem {
    id: String,
    image: PathBuf,
    band: SystemBand,
    interline: f64,
    truths: Vec<(f64, String)>,
}

fn parse_fixture(path: &Path, workspace: &Path) -> Vec<TruthSystem> {
    let text = std::fs::read_to_string(path).expect("fixture file readable");
    let mut systems = Vec::new();
    let mut image = PathBuf::new();
    let mut current: Option<TruthSystem> = None;
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        match fields.first().copied() {
            None | Some("#") => {}
            Some("image") => image = workspace.join(fields[1]),
            Some("system") => {
                current = Some(TruthSystem {
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
                });
            }
            Some("truth") => current
                .as_mut()
                .unwrap()
                .truths
                .push((fields[1].parse().unwrap(), fields[2].to_string())),
            Some("end") => systems.push(current.take().unwrap()),
            Some(other) => panic!("unknown fixture directive {other}"),
        }
    }
    systems
}

#[test]
fn projection_recovers_every_synthetic_barline_at_every_scale() {
    let Ok(workspace) = std::env::var("AUDIVERIS_BARLINE_TUNING_FIXTURES") else {
        eprintln!(
            "SKIPPED barline_tuning_synthetic: set AUDIVERIS_BARLINE_TUNING_FIXTURES to the \
             workspace root containing stage-omr-data"
        );
        return;
    };
    let workspace = PathBuf::from(workspace);
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/py-barline-tuning/verovio-synthetic.txt");
    let systems = parse_fixture(&fixture, &workspace);
    assert_eq!(systems.len(), 15, "three pieces at three scales");
    let mut raster_cache: Option<(PathBuf, GrayRaster)> = None;
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
        let parameters = BarTuningParameters::from_scale(system.interline);
        let context = &system.id;
        // Line-end barlines coincide with the system edge and are modeled by
        // the edge boundaries themselves, exactly like scan systems.
        let interior: Vec<&(f64, String)> = system
            .truths
            .iter()
            .filter(|(x, _)| {
                *x > system.band.left + parameters.edge_bar_margin
                    && *x < system.band.right - parameters.edge_bar_margin
            })
            .collect();
        let edge = |x: f64| RawBoundary {
            x,
            support: 0,
            max_grade: 0.0,
            kind: RawBoundaryKind::Edge,
        };
        let input = SystemBarInput {
            band: system.band,
            boundaries: vec![edge(system.band.left), edge(system.band.right)],
        };
        let candidates = projection_candidates(&gray, &system.band, &parameters);
        let geometry = geometry_count(&input, &candidates, &parameters);
        assert_eq!(
            geometry.intervals,
            interior.len() + 1,
            "{context}: measure count from projection alone (certain bars: {}, truths: {:?})",
            geometry.certain_bars,
            interior
                .iter()
                .map(|(x, form)| (*x, form.as_str()))
                .collect::<Vec<_>>(),
        );
        // Every interior barline is recovered as a certain connector within
        // one interline of the engraver's position.
        for (x, form) in &interior {
            let hit = candidates
                .iter()
                .filter(|candidate| candidate.is_certain_connector(&parameters))
                .any(|candidate| (candidate.x - x).abs() <= system.interline);
            assert!(
                hit,
                "{context}: no certain connector near truth x={x} ({form})"
            );
        }
        // The full selection path lands on the same answer.
        let count = resolve_count(geometry, 1);
        let tuned = select_boundaries(&input, &candidates, &[], count, &parameters);
        assert_eq!(
            tuned.len() - 1,
            interior.len() + 1,
            "{context}: tuned measure count"
        );
        for ((truth_x, form), boundary) in interior.iter().zip(tuned[1..tuned.len() - 1].iter()) {
            assert!(
                (boundary.x - truth_x).abs() <= system.interline,
                "{context}: tuned boundary {} vs truth {truth_x} ({form})",
                boundary.x
            );
        }
    }
    eprintln!(
        "barline_tuning_synthetic: {} systems validated against engraver ground truth",
        systems.len()
    );
}
