// SPDX-License-Identifier: AGPL-3.0-or-later

//! Native page recognition entry points.
//!
//! This module assembles the already-ported stages into runnable page
//! processing. The current slice covers `LOAD -> BINARY -> SCALE` on a raster
//! image using the same production settings as the Java pipeline: max-channel
//! grayscale ingest, integral-image adaptive binarization, vertical run
//! histograms, and the full `ScaleBuilder` decision including the
//! almost-blank-page check. The GRID continuation will extend
//! [`ScaleRecognition`] rather than replace it.

use std::path::Path;

use audiveris_image::adaptive;
use audiveris_image::bar_column::StaffId;
use audiveris_image::ingest::{self, LoadError};
use audiveris_image::line_short_sections::HorizontalSectionLag;
use audiveris_image::lines_coordinator::{
    ClusterPassState, StaffCandidate, retrieve_staff_candidates,
};
use audiveris_image::production_grid_params::production_grid_parameters;
use audiveris_image::projection::{
    BarlineHeightSpec, NeutralStaffProjectorRequest, PeakConstructionParams, PeakCoreGeometry,
    PeakCoreParams, PeakRefinementParams, ShortProjection, StaffProjectionRequest,
    StaffProjectorScaleParameters, StaffProjectorScaleRatios, StaffProjectorScaleRequest,
    staff_projector_scale_parameters,
};
use audiveris_image::raw_line_adapter::build_primary_cluster_pass;
use audiveris_image::run_table::{Orientation, RunTable, RunTableError, create_grid_run_tables};
use audiveris_image::scale_estimate::{
    ScaleEstimate, ScaleEstimateError, ScaleOptions, estimate_scale,
};
use audiveris_image::scale_runs::vertical_run_histograms;

/// Result of running `LOAD -> BINARY -> SCALE` natively on one raster page.
#[derive(Debug, Clone)]
pub struct ScaleRecognition {
    pub width: usize,
    pub height: usize,
    /// FNV-1a digest of the loaded grayscale raster, as in the parity vectors.
    pub gray_digest: u64,
    /// Adaptive binary mask in Java ByteProcessor convention:
    /// `0` is ink, `255` is background, row-major.
    pub binary: Vec<u8>,
    /// Vertical black runs of the binary mask, the SCALE/GRID source table.
    pub vertical_runs: RunTable,
    pub scale: ScaleEstimate,
}

/// Failure of the native `LOAD -> BINARY -> SCALE` slice.
#[derive(Debug)]
pub enum ScaleRecognitionError {
    Load(LoadError),
    Runs(RunTableError),
    Scale(ScaleEstimateError),
}

impl std::fmt::Display for ScaleRecognitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Load(error) => write!(f, "cannot load page image: {error}"),
            Self::Runs(error) => write!(f, "cannot build vertical runs: {error}"),
            Self::Scale(error) => write!(f, "cannot estimate scale: {error:?}"),
        }
    }
}

impl std::error::Error for ScaleRecognitionError {}

impl From<LoadError> for ScaleRecognitionError {
    fn from(error: LoadError) -> Self {
        Self::Load(error)
    }
}

impl From<RunTableError> for ScaleRecognitionError {
    fn from(error: RunTableError) -> Self {
        Self::Runs(error)
    }
}

impl From<ScaleEstimateError> for ScaleRecognitionError {
    fn from(error: ScaleEstimateError) -> Self {
        Self::Scale(error)
    }
}

/// Runs `LOAD -> BINARY -> SCALE` on the raster at `path` with production
/// settings.
pub fn recognize_scale(path: impl AsRef<Path>) -> Result<ScaleRecognition, ScaleRecognitionError> {
    let loaded = ingest::load_max_channel_gray(path)?;
    let (width, height) = (loaded.width(), loaded.height());
    let gray_digest = loaded.fnv1a64();
    let binary = adaptive::default_adaptive_filter(width, height, loaded.pixels());
    let vertical_runs = RunTable::from_pixels(Orientation::Vertical, width, height, &binary)?;
    let histograms = vertical_run_histograms(&vertical_runs);
    let scale = estimate_scale(
        &histograms,
        ScaleOptions {
            image_size: Some((width, height)),
            ..ScaleOptions::default()
        },
    )?;
    Ok(ScaleRecognition {
        width,
        height,
        gray_digest,
        binary,
        vertical_runs,
        scale,
    })
}

/// Renders the recognition outcome as stable, line-oriented text for CLI and
/// test consumption. The scale line reuses the canonical parity-vector shape.
pub fn scale_report(recognition: &ScaleRecognition) -> String {
    let scale = &recognition.scale;
    let optional =
        |value: Option<i32>| value.map_or_else(|| "null".to_owned(), |value| value.to_string());
    format!(
        "page={}x{}/{:016x}\nscale=line:{};interline:{};small-interline:{};beam:{};small-beam:{}\nresolution={:?}\n",
        recognition.width,
        recognition.height,
        recognition.gray_digest,
        scale.line.main,
        scale.interline.main,
        optional(scale.small_interline.map(|value| value.main)),
        scale.beam.main,
        optional(scale.small_beam.map(|value| value.main)),
        scale.resolution,
    )
}

/// One staff candidate found by the native GRID line retrieval.
#[derive(Debug, Clone)]
pub struct StaffCandidateReport {
    pub id: usize,
    pub kind: String,
    pub left: f64,
    pub right: f64,
    pub interline: usize,
    pub small: bool,
    pub short: bool,
    pub line_count: usize,
    /// Graded bar peaks from this staff's projector, as `(start, stop, grade)`.
    pub peaks: Vec<(i32, i32, f64)>,
}

/// Result of running the native GRID staff-line slice on one raster page.
///
/// This covers Java `LinesRetriever` through `buildStaves`: run partition,
/// horizontal lag, primary filament pass, measured global slope, cluster
/// retrieval, and staff-candidate construction. Bars, projectors, and line
/// completion are the next slices.
#[derive(Debug, Clone)]
pub struct GridLinesRecognition {
    pub scale: ScaleRecognition,
    /// Sheet slope measured from the top filaments, as Java re-measures it.
    pub global_slope: f64,
    pub filament_count: usize,
    pub sloped_reject_count: usize,
    pub discarded_filament_count: usize,
    pub staves: Vec<StaffCandidateReport>,
}

/// Failure of the native GRID staff-line slice.
#[derive(Debug)]
pub enum GridRecognitionError {
    Scale(ScaleRecognitionError),
    Stage {
        stage: &'static str,
        message: String,
    },
}

impl std::fmt::Display for GridRecognitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scale(error) => write!(f, "{error}"),
            Self::Stage { stage, message } => write!(f, "GRID {stage} failed: {message}"),
        }
    }
}

impl std::error::Error for GridRecognitionError {}

impl From<ScaleRecognitionError> for GridRecognitionError {
    fn from(error: ScaleRecognitionError) -> Self {
        Self::Scale(error)
    }
}

fn grid_stage<E: std::fmt::Debug>(stage: &'static str) -> impl FnOnce(E) -> GridRecognitionError {
    move |error| GridRecognitionError::Stage {
        stage,
        message: format!("{error:?}"),
    }
}

/// Runs one staff's `StaffProjector`, returning its graded bar peaks.
///
/// Reproduces Java `StaffProjector.process`: raster accumulation over the
/// staff band, per-staff thresholds measured from actual line thickness
/// (`computeLineThresholds`), and peak construction with refinement.
fn project_staff_peaks(
    recognition: &ScaleRecognition,
    projector_pixels: &[u8],
    staff: &StaffCandidate,
    primary: &ClusterPassState,
    scale_parameters: &StaffProjectorScaleParameters,
) -> Result<Vec<(i32, i32, f64)>, GridRecognitionError> {
    let lines: Vec<_> = staff
        .line_ids()
        .iter()
        .map(|id| {
            primary
                .filaments()
                .get(id)
                .ok_or_else(|| GridRecognitionError::Stage {
                    stage: "projector",
                    message: format!("staff line {} is missing", id.value()),
                })
        })
        .collect::<Result<_, _>>()?;
    let Some((first, last)) = lines.first().zip(lines.last()) else {
        return Ok(Vec::new());
    };
    let middle = lines[lines.len() / 2];
    // Build each needed line's spline geometry once, not per abscissa.
    let geometry_of = |filament: &audiveris_image::filament::StaffFilament, which: &'static str| {
        filament
            .geometry()
            .map_err(|error| GridRecognitionError::Stage {
                stage: "projector",
                message: format!("{which} staff line geometry: {error:?}"),
            })
    };
    let first_geometry = geometry_of(first, "first")?;
    let last_geometry = geometry_of(last, "last")?;
    let middle_geometry = geometry_of(middle, "middle")?;
    let ordinate = |geometry: &audiveris_image::filament::FilamentGeometry, x: i32| {
        geometry.position_at(f64::from(x)).unwrap_or(0.0).round() as i32
    };

    // Java computeCoreLinesThickness: sum of line thicknesses, scaled by
    // (n-1)/n for multi-line staves.
    let mut lines_cumul = lines
        .iter()
        .map(|line| line.thickness().unwrap_or(0.0))
        .sum::<f64>();
    let line_count = lines.len();
    if line_count > 1 {
        lines_cumul *= (line_count as f64 - 1.0) / line_count as f64;
    }
    // blankThreshold uses floor, not rint; linesThreshold uses rint.
    let blank_threshold = (0.5 * lines_cumul).floor() as i32;
    let lines_threshold = lines_cumul.round_ties_even() as i32;
    let specific_interline = staff.interline() as i32;
    // chunkThreshold = (lineCount - 1) * fore + rint(1.2 * specificInterline)
    let chunk_threshold = (line_count as i32 - 1) * recognition.scale.line.main
        + (1.2 * f64::from(specific_interline)).round_ties_even() as i32;
    let total_height = specific_interline * (line_count as i32 - 1);

    let accumulation = ShortProjection::from_staff_raster(
        recognition.width,
        recognition.height,
        projector_pixels,
        StaffProjectionRequest::new(
            staff.left().round() as i32,
            staff.right().round() as i32,
            scale_parameters.staff_abscissa_margin,
        ),
        |x| ordinate(&first_geometry, x),
        |x| ordinate(&last_geometry, x),
    )
    .map_err(grid_stage("projection accumulation"))?;

    let refinement = PeakRefinementParams::new(
        scale_parameters.bar_threshold,
        lines_threshold,
        chunk_threshold,
        scale_parameters.bar_refine_dx,
        scale_parameters.chunk_width.max(1),
    )
    .map_err(grid_stage("peak refinement parameters"))?;
    let result = accumulation
        .finish_neutral(
            recognition.width,
            recognition.height,
            projector_pixels,
            NeutralStaffProjectorRequest {
                staff_id: StaffId::new(staff.id()),
                staff_left: staff.left().round() as i32,
                staff_right: staff.right().round() as i32,
                blank_threshold,
                minimum_wide_blank_width: scale_parameters.minimum_wide_blank_width,
                // Java topDerivativeNumber = 5, minDerivativeRatio = 0.3.
                top_derivative_count: 5,
                minimum_derivative_ratio: 0.3,
                use_one_line_half_mode: scale_parameters.use_one_line_half_mode,
                is_one_line_staff: false,
                bar_threshold: scale_parameters.bar_threshold,
                total_height,
                peak_construction: PeakConstructionParams::new(
                    refinement,
                    scale_parameters.maximum_bar_width,
                )
                .map_err(grid_stage("peak construction parameters"))?,
                peak_core: PeakCoreParams::new(scale_parameters.gap_threshold, 0.3)
                    .map_err(grid_stage("peak core parameters"))?,
                brace_search: None,
            },
            |x| {
                PeakCoreGeometry::new(
                    ordinate(&first_geometry, x),
                    ordinate(&last_geometry, x),
                    ordinate(&middle_geometry, x),
                )
            },
        )
        .map_err(grid_stage("projector"))?;

    Ok(result
        .peaks
        .iter()
        .map(|peak| {
            (
                peak.start(),
                peak.stop(),
                peak.impacts().map_or(0.0, |impacts| impacts.grade()),
            )
        })
        .collect())
}

/// Runs the native GRID staff-line slice with production, scale-derived
/// parameters.
///
/// The primary filament pass runs twice: first with a zero slope seed to
/// measure the sheet slope from the top filaments, then rebuilt with the
/// measured slope, mirroring Java's measure-then-cluster order.
pub fn recognize_grid_lines(
    path: impl AsRef<Path>,
) -> Result<GridLinesRecognition, GridRecognitionError> {
    let scale_recognition = recognize_scale(path)?;

    let seed_parameters = production_grid_parameters(&scale_recognition.scale, 0.0)
        .map_err(grid_stage("parameter derivation"))?;
    let tables = create_grid_run_tables(
        &scale_recognition.vertical_runs,
        seed_parameters.raster.max_fore,
        seed_parameters.raster.ledger_thickness,
        seed_parameters.raster.minimum_horizontal_run_length,
    )
    .map_err(grid_stage("run partition"))?;
    let lag = HorizontalSectionLag::from_long_runs(tables.long_horizontal.clone())
        .map_err(grid_stage("horizontal lag"))?;

    let seed_pass = build_primary_cluster_pass(&lag, seed_parameters.raw_primary)
        .map_err(grid_stage("primary pass (slope seed)"))?;
    let global_slope = seed_pass.global_slope();

    let parameters = production_grid_parameters(&scale_recognition.scale, global_slope)
        .map_err(grid_stage("parameter derivation"))?;
    let pass = build_primary_cluster_pass(&lag, parameters.raw_primary)
        .map_err(grid_stage("primary pass"))?;
    let filament_count = pass.factory_creation_ids().len();
    let sloped_reject_count = pass.sloped_ids().len();
    let mut primary = pass.into_state();
    let result = retrieve_staff_candidates(&mut primary, None, parameters.lines)
        .map_err(grid_stage("staff retrieval"))?;

    // The adaptive filter already emits Java ByteProcessor semantics
    // (`FOREGROUND == 0`, `BACKGROUND == 255`), which is exactly what the
    // projector expects; no inversion.
    let projector_pixels = scale_recognition.binary.as_slice();

    let projector_scale = staff_projector_scale_parameters(StaffProjectorScaleRequest {
        large_interline: scale_recognition.scale.interline.main,
        staff_specific_interline: 0,
        is_one_line_staff: false,
        // Java default BarlineHeight for standard staves.
        barline_height: BarlineHeightSpec::Four,
        ratios: StaffProjectorScaleRatios::java_defaults(),
    });

    let mut staves = Vec::with_capacity(result.staffs().len());
    for staff in result.staffs() {
        let peaks = project_staff_peaks(
            &scale_recognition,
            projector_pixels,
            staff,
            &primary,
            &projector_scale,
        )?;
        staves.push(StaffCandidateReport {
            id: staff.id(),
            kind: format!("{:?}", staff.kind()).to_lowercase(),
            left: staff.left(),
            right: staff.right(),
            interline: staff.interline(),
            small: staff.is_small(),
            short: staff.is_short(),
            line_count: staff.line_ids().len(),
            peaks,
        });
    }
    let discarded_filament_count = result.primary().discarded_filaments().len();

    Ok(GridLinesRecognition {
        scale: scale_recognition,
        global_slope,
        filament_count,
        sloped_reject_count,
        discarded_filament_count,
        staves,
    })
}

/// Renders the GRID staff-line outcome as stable, line-oriented text.
pub fn grid_lines_report(recognition: &GridLinesRecognition) -> String {
    let mut report = scale_report(&recognition.scale);
    report.push_str(&format!(
        "grid=slope:{:.6};filaments:{};sloped:{};discarded:{};staves:{}\n",
        recognition.global_slope,
        recognition.filament_count,
        recognition.sloped_reject_count,
        recognition.discarded_filament_count,
        recognition.staves.len(),
    ));
    for staff in &recognition.staves {
        report.push_str(&format!(
            "staff={}:{}:x{:.0}-{:.0}:interline:{}:lines:{}{}{}:peaks:{}\n",
            staff.id,
            staff.kind,
            staff.left,
            staff.right,
            staff.interline,
            staff.line_count,
            if staff.small { ":small" } else { "" },
            if staff.short { ":short" } else { "" },
            staff.peaks.len(),
        ));
        if !staff.peaks.is_empty() {
            report.push_str(&format!(
                "  peak-x={}\n",
                staff
                    .peaks
                    .iter()
                    .map(|(start, stop, _)| format!("{start}-{stop}"))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_path(relative: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join(relative)
    }

    #[test]
    fn recognizes_chula_scale_with_production_settings() {
        let recognition = recognize_scale(repo_path("data/examples/chula.png"))
            .expect("chula page must recognize");
        // Values locked by the existing Java/Rust parity vectors
        // (load.chula, scale.chula) at the frozen oracle baseline.
        assert_eq!((recognition.width, recognition.height), (2450, 1954));
        assert_eq!(recognition.gray_digest, 0x2179_468e_de9f_7ec6);
        let scale = &recognition.scale;
        assert_eq!(scale.line.main, 3);
        assert_eq!(scale.interline.main, 21);
        assert!(scale.small_interline.is_none());
        assert_eq!(scale.beam.main, 12);
        assert!(scale.small_beam.is_none());
        assert_eq!(
            scale_report(&recognition),
            "page=2450x1954/2179468ede9f7ec6\n\
             scale=line:3;interline:21;small-interline:null;beam:12;small-beam:null\n\
             resolution=Accepted\n"
        );
    }

    #[test]
    fn recognizes_chula_staves_matching_java_oracle() {
        let recognition = recognize_grid_lines(repo_path("data/examples/chula.png"))
            .expect("chula staff retrieval");
        // Java oracle (Audiveris 5.11, JDK 25) on the same page reports:
        //   LinesRetriever | Global slope: 0.00792
        //   PeakGraph      | Systems: #1[1, 2] #2[3, 4] #3[5, 6]
        //   SystemManager  | Indentation detected for system #1
        assert!((recognition.global_slope - 0.007915).abs() < 5e-6);
        assert_eq!(recognition.staves.len(), 6);
        for staff in &recognition.staves {
            assert_eq!(staff.kind, "standard");
            assert_eq!(staff.line_count, 5);
            assert_eq!(staff.interline, 21);
            assert!(!staff.small);
        }
        // First-system staves are indented relative to the rest.
        assert!(recognition.staves[0].left > 190.0);
        assert!(recognition.staves[1].left > 190.0);
        for staff in &recognition.staves[2..] {
            assert!(staff.left < 100.0);
        }
        let report = grid_lines_report(&recognition);
        assert!(report.contains("staves:6"));
        assert!(report.contains("staff=1:standard:x203-2323:interline:21:lines:5"));
        assert!(report.contains("staff=6:standard:x83-2309:interline:21:lines:5"));
    }

    #[test]
    fn chula_peaks_cover_every_java_barline() {
        // Barline abscissae extracted from a live Java Audiveris 5.11 GRID run
        // on this page (sheet#1.xml barline inter medians, per staff).
        const JAVA_BARLINES: [&[i32]; 6] = [
            &[
                200, 464, 832, 1174, 1364, 1546, 1804, 1817, 1828, 1962, 2325,
            ],
            &[
                202, 466, 833, 1175, 1364, 1546, 1804, 1818, 1830, 1963, 2326,
            ],
            &[86, 558, 986, 1283, 1297, 1452, 1902, 2325],
            &[87, 558, 986, 1282, 1296, 1452, 1902, 2325],
            &[82, 413, 460, 607, 976, 1344, 1668, 2034, 2312, 2322],
            &[82, 414, 460, 608, 978, 1345, 1668, 2034, 2312, 2324],
        ];
        let recognition = recognize_grid_lines(repo_path("data/examples/chula.png"))
            .expect("chula grid recognition");
        assert_eq!(recognition.staves.len(), JAVA_BARLINES.len());
        for (staff, expected) in recognition.staves.iter().zip(JAVA_BARLINES) {
            for &barline in expected {
                // The projector emits raw candidates whose refined sides may
                // exceed Java's final barline box by a few pixels; the
                // peak-graph purges that narrow these are a later slice.
                let covered = staff
                    .peaks
                    .iter()
                    .any(|&(start, stop, _)| (start - 3..=stop + 3).contains(&barline));
                assert!(
                    covered,
                    "staff {} lost Java barline at x={barline}; peaks: {:?}",
                    staff.id, staff.peaks
                );
            }
            // Every peak must be graded above Java's minimum inter grade.
            assert!(staff.peaks.iter().all(|&(_, _, grade)| grade > 0.0));
        }
        // Raw projector output is deliberately a superset of the final
        // barlines: stems and other vertical ink survive until the peak graph
        // filters them.
        let total: usize = recognition.staves.iter().map(|s| s.peaks.len()).sum();
        assert!((58..=200).contains(&total), "unexpected peak total {total}");
    }

    #[test]
    fn missing_file_reports_load_error() {
        let error = recognize_scale(repo_path("data/examples/does-not-exist.png"))
            .expect_err("missing file must fail");
        assert!(matches!(error, ScaleRecognitionError::Load(_)));
    }
}
