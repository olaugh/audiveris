// SPDX-License-Identifier: AGPL-3.0-or-later

//! Numeric parity of the `bar_tuning` engine against its Python oracle.
//!
//! The Python prototype (`stage-omr-data/scripts/tune_sonata_barlines.py`)
//! is the oracle for this enhancement layer — the Java executable remains
//! the oracle for parity-port code only.  Fixtures live in
//! `rust/oracle/py-barline-tuning/` and reference scan images by paths
//! relative to the workspace root, which must be supplied through
//! `AUDIVERIS_BARLINE_TUNING_FIXTURES`; the test skips (loudly) when the
//! variable is unset.  Regeneration recipe: see the fixture file headers.

use std::path::{Path, PathBuf};

use audiveris_image::bar_tuning::{
    Anchor, BarClassificationParameters, BarTuningParameters, RawBoundary, RawBoundaryKind,
    SystemBand, SystemBarInput, diff_boundaries, geometry_count, geometry_count_with_voltas,
    projection_candidates, resolve_count, select_boundaries, volta_hook,
};
use audiveris_image::ingest::GrayRaster;

struct FixtureSystem {
    id: String,
    image: PathBuf,
    input: SystemBarInput,
    anchors: Vec<Anchor>,
    decoded_count: usize,
    expected_candidates: Vec<Vec<f64>>,
    expected_geometry: (usize, usize),
    expected_count: usize,
    expected_tuned: Vec<f64>,
    expected_added: Vec<f64>,
    expected_demoted: Vec<f64>,
}

fn parse_kind(label: &str) -> RawBoundaryKind {
    match label {
        "thin" => RawBoundaryKind::Thin,
        "thick" => RawBoundaryKind::Thick,
        "edge" => RawBoundaryKind::Edge,
        _ => RawBoundaryKind::Unknown,
    }
}

fn floats(fields: &[&str]) -> Vec<f64> {
    fields
        .iter()
        .map(|field| field.parse::<f64>().expect("fixture float"))
        .collect()
}

fn parse_fixture(path: &Path, workspace: &Path) -> Vec<FixtureSystem> {
    let text = std::fs::read_to_string(path).expect("fixture file readable");
    let mut systems = Vec::new();
    let mut image = PathBuf::new();
    let mut current: Option<FixtureSystem> = None;
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        match fields.first().copied() {
            None | Some("#") => {}
            Some("image") => image = workspace.join(fields[1]),
            Some("system") => {
                current = Some(FixtureSystem {
                    id: fields[1].to_string(),
                    image: image.clone(),
                    input: SystemBarInput {
                        band: SystemBand {
                            left: fields[2].parse().unwrap(),
                            right: fields[3].parse().unwrap(),
                            top: fields[4].parse().unwrap(),
                            bottom: fields[5].parse().unwrap(),
                        },
                        boundaries: Vec::new(),
                    },
                    anchors: Vec::new(),
                    decoded_count: fields[6].parse().unwrap(),
                    expected_candidates: Vec::new(),
                    expected_geometry: (0, 0),
                    expected_count: 0,
                    expected_tuned: Vec::new(),
                    expected_added: Vec::new(),
                    expected_demoted: Vec::new(),
                });
            }
            Some("raw") => current
                .as_mut()
                .unwrap()
                .input
                .boundaries
                .push(RawBoundary {
                    x: fields[1].parse().unwrap(),
                    support: fields[2].parse().unwrap(),
                    max_grade: fields[3].parse().unwrap(),
                    kind: parse_kind(fields[4]),
                }),
            Some("anchor") => current.as_mut().unwrap().anchors.push(Anchor {
                x: fields[1].parse().unwrap(),
                value: fields[2].parse().unwrap(),
            }),
            Some("expect_candidate") => current
                .as_mut()
                .unwrap()
                .expected_candidates
                .push(floats(&fields[1..])),
            Some("expect_geometry") => {
                current.as_mut().unwrap().expected_geometry =
                    (fields[1].parse().unwrap(), fields[2].parse().unwrap());
            }
            Some("expect_count") => {
                current.as_mut().unwrap().expected_count = fields[1].parse().unwrap();
            }
            Some("expect_tuned") => {
                current.as_mut().unwrap().expected_tuned = floats(&fields[1..]);
            }
            Some("expect_added") => {
                current.as_mut().unwrap().expected_added = floats(&fields[1..]);
            }
            Some("expect_demoted") => {
                current.as_mut().unwrap().expected_demoted = floats(&fields[1..]);
            }
            Some("end") => systems.push(current.take().unwrap()),
            Some(other) => panic!("unknown fixture directive {other}"),
        }
    }
    systems
}

/// Python `round(x, 2)` equivalent for the boundary positions the oracle
/// rounds before diffing.
fn round2(value: f64) -> f64 {
    (value * 100.0).round_ties_even() / 100.0
}

fn assert_close(context: &str, actual: f64, expected: f64, tolerance: f64, worst: &mut f64) {
    let delta = (actual - expected).abs();
    *worst = worst.max(delta);
    assert!(
        delta <= tolerance,
        "{context}: {actual} vs expected {expected} (delta {delta:.3e})"
    );
}

/// Interline-mode divergences from the pixel oracle that are understood,
/// deliberate, and pinned so further drift is caught.  Currently empty:
/// the one prior residual (the GGR p2s5 volta first-ending barline, which
/// the pixel oracle recovered only through an accidentally inflated count)
/// is now recovered on principle - its detected volta bracket counts the
/// interrupted, stem-clean candidate in the geometry.
const KNOWN_INTERLINE_RESIDUALS: &[(&str, &str, usize)] = &[];

/// How strictly a fixture run is graded.
enum Comparison {
    /// Pixel-mode parity: every candidate metric to 1e-9, all else exact.
    Exact,
    /// Interline-mode: candidate metrics legitimately differ (flank
    /// distances, windows, and merges scale), but the *decisions* — the
    /// resolved count, the tuned boundary set, and the added/demoted
    /// classification — must survive within an engraving tolerance.
    Decisions { position_tolerance: f64 },
}

fn run_fixture(
    name: &str,
    expected_system_count: usize,
    parameters: &BarTuningParameters,
    comparison: &Comparison,
) {
    let Ok(workspace) = std::env::var("AUDIVERIS_BARLINE_TUNING_FIXTURES") else {
        eprintln!(
            "SKIPPED barline_tuning_parity({name}): set AUDIVERIS_BARLINE_TUNING_FIXTURES to the \
             workspace root containing stage-omr-data"
        );
        return;
    };
    let workspace = PathBuf::from(workspace);
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/py-barline-tuning")
        .join(name);
    let systems = parse_fixture(&fixture, &workspace);
    assert_eq!(
        systems.len(),
        expected_system_count,
        "fixture {name} lists every system — nothing silently skipped"
    );
    let mut raster_cache: Option<(PathBuf, GrayRaster)> = None;
    let mut worst = 0.0_f64;
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
        let context = format!("{name}:{}", system.id);
        let candidates = projection_candidates(&gray, &system.input.band, parameters);
        if let Comparison::Decisions { position_tolerance } = comparison {
            // Intermediate counts may legitimately differ: e.g. a section
            // double bar recorded as thick+thin strokes 7 px apart counts as
            // two bars at the reference merge tolerance but folds into one
            // boundary at interline scale — the honest, better count.  The
            // graded decision is the final tuned boundary set.
            // Interline mode is volta-aware: hooks detected over candidate
            // positions let interrupted ending barlines count on principle.
            let classification =
                BarClassificationParameters::from_scale((parameters.geometry_merge).max(6.0));
            let hooks: Vec<f64> = candidates
                .iter()
                .map(|candidate| candidate.x)
                .filter(|x| volta_hook(&gray, &system.input.band, *x, &classification))
                .collect();
            let geometry =
                geometry_count_with_voltas(&system.input, &candidates, &hooks, parameters);
            let count = resolve_count(geometry, system.decoded_count);
            if count != system.expected_count {
                eprintln!(
                    "{context}: resolved count {count} vs oracle {} (graded on boundaries)",
                    system.expected_count
                );
            }
            let tuned = select_boundaries(
                &system.input,
                &candidates,
                &system.anchors,
                count,
                parameters,
            );
            if let Some((_, _, pinned_len)) = KNOWN_INTERLINE_RESIDUALS
                .iter()
                .find(|(fixture, id, _)| *fixture == name && *id == system.id)
            {
                assert_eq!(
                    tuned.len(),
                    *pinned_len,
                    "{context}: pinned residual drifted ({:?})",
                    tuned.iter().map(|b| round2(b.x)).collect::<Vec<_>>(),
                );
                eprintln!(
                    "{context}: pinned interline residual ({} boundaries vs oracle {})",
                    tuned.len(),
                    system.expected_tuned.len()
                );
                continue;
            }
            assert_eq!(
                tuned.len(),
                system.expected_tuned.len(),
                "{context}: tuned boundary count ({:?} vs {:?})",
                tuned.iter().map(|b| round2(b.x)).collect::<Vec<_>>(),
                system.expected_tuned,
            );
            for (actual, expected) in tuned.iter().zip(&system.expected_tuned) {
                assert_close(
                    &context,
                    actual.x,
                    *expected,
                    *position_tolerance,
                    &mut worst,
                );
            }
            continue;
        }
        assert_eq!(
            candidates.len(),
            system.expected_candidates.len(),
            "{context}: candidate count"
        );
        for (candidate, expected) in candidates.iter().zip(&system.expected_candidates) {
            assert_close(&context, candidate.x, expected[0], 0.0, &mut worst);
            assert_close(
                &context,
                candidate.paired_occupancy,
                expected[1],
                1e-9,
                &mut worst,
            );
            assert_close(
                &context,
                candidate.mean_occupancy,
                expected[2],
                1e-9,
                &mut worst,
            );
            assert_close(
                &context,
                candidate.bridge_occupancy,
                expected[3],
                1e-9,
                &mut worst,
            );
            assert_close(
                &context,
                candidate.full_occupancy,
                expected[4],
                1e-9,
                &mut worst,
            );
            assert_eq!(
                candidate.maximum_gap, expected[5] as usize,
                "{context}: maximum gap at x={}",
                candidate.x
            );
            assert_close(
                &context,
                candidate.left_flank_noise,
                expected[6],
                1e-9,
                &mut worst,
            );
            assert_close(
                &context,
                candidate.right_flank_noise,
                expected[7],
                1e-9,
                &mut worst,
            );
            assert_close(
                &context,
                candidate.flank_noise,
                expected[8],
                1e-9,
                &mut worst,
            );
        }
        let geometry = geometry_count(&system.input, &candidates, parameters);
        assert_eq!(
            (geometry.intervals, geometry.certain_bars),
            system.expected_geometry,
            "{context}: geometry count"
        );
        let count = resolve_count(geometry, system.decoded_count);
        assert_eq!(count, system.expected_count, "{context}: resolved count");
        let tuned = select_boundaries(
            &system.input,
            &candidates,
            &system.anchors,
            count,
            parameters,
        );
        let tuned_rounded: Vec<f64> = tuned.iter().map(|boundary| round2(boundary.x)).collect();
        assert_eq!(
            tuned_rounded.len(),
            system.expected_tuned.len(),
            "{context}: tuned boundary count"
        );
        for (actual, expected) in tuned_rounded.iter().zip(&system.expected_tuned) {
            assert_close(&context, *actual, *expected, 1e-9, &mut worst);
        }
        let raw_x: Vec<f64> = system.input.boundaries.iter().map(|b| b.x).collect();
        let diff = diff_boundaries(&raw_x, &tuned_rounded, 6.0);
        assert_eq!(diff.added, system.expected_added, "{context}: added");
        assert_eq!(diff.demoted, system.expected_demoted, "{context}: demoted");
    }
    eprintln!(
        "barline_tuning_parity({name}): {} systems byte-compared, worst float delta {worst:.3e}",
        systems.len()
    );
}

#[test]
fn python_oracle_parity_on_schenker_sonata01() {
    run_fixture(
        "schenker-sonata01.txt",
        104,
        &BarTuningParameters::python_reference(),
        &Comparison::Exact,
    );
}

#[test]
fn python_oracle_parity_on_graceful_ghost_rag() {
    run_fixture(
        "graceful-ghost-rag.txt",
        20,
        &BarTuningParameters::python_reference(),
        &Comparison::Exact,
    );
}

#[test]
fn interline_mode_preserves_decisions_on_graceful_ghost_rag() {
    // GGR scans at interline 12 (twice the Schenker calibration scale).
    // The oracle ran them with fixed pixel constants; deriving the pixel
    // values from the interline must reach the same measure decisions.
    // Tolerance of one interline: a tuned boundary may anchor on either
    // stroke of a section double bar (the evidence-weighted merge shifts
    // with the scaled merge distances), but distinct bars are never closer
    // than the DP minimum gap, so identity within an interline is exact.
    run_fixture(
        "graceful-ghost-rag.txt",
        20,
        &BarTuningParameters::from_scale(12.0),
        &Comparison::Decisions {
            position_tolerance: 12.0,
        },
    );
}

#[test]
fn interline_mode_is_the_reference_at_schenker_scale() {
    // from_scale(6) == python_reference() is unit-tested in the engine;
    // running the full corpus in interline mode is therefore the same run.
    run_fixture(
        "schenker-sonata01.txt",
        104,
        &BarTuningParameters::from_scale(6.0),
        &Comparison::Exact,
    );
}
