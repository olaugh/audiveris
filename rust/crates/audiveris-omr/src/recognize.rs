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

use crate::grid_executor::HeadlessSkew;
use audiveris_image::adaptive;
use audiveris_image::bar_alignment::BarAlignment;
use audiveris_image::bar_alignments::{
    AlignmentBuildReport, AlignmentParameters, AlignmentStaff, find_all_alignments,
};
use audiveris_image::bar_column::StaffId;
use audiveris_image::bar_connections::{
    ConnectionBuildReport, ConnectionParameters, ConnectionRaster, find_connections,
};
use audiveris_image::bar_sticks::{BarStickBuildState, BarStickParameters, build_bar_sticks};
use audiveris_image::ingest::{self, LoadError};
use audiveris_image::line_short_sections::HorizontalSectionLag;
use audiveris_image::lines_coordinator::{
    ClusterPassState, StaffCandidate, retrieve_staff_candidates,
};
use audiveris_image::peak_graph::PeakGraph;
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
use audiveris_image::section::{InitialGridLags, build_initial_grid_lags};
use audiveris_image::staff_peak::{StaffPeak, StaffPeakKey};

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

/// Cross-staff barline structure recovered by the peak graph.
#[derive(Debug, Clone)]
pub struct PeakGraphReport {
    pub alignment_count: usize,
    pub connection_count: usize,
    /// Peaks that yielded a registered bar filament.
    pub stick_count: usize,
    /// Peaks Java drops because no acceptable bar filament could be built.
    pub stickless_peak_count: usize,
    /// Staff ids grouped by shared aligned barlines, in Java's system order.
    pub systems: Vec<Vec<usize>>,
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
    pub peak_graph: PeakGraphReport,
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
) -> Result<Vec<StaffPeak>, GridRecognitionError> {
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

    Ok(result.peaks)
}

/// Builds Java `PeakGraph` alignments across staves and groups the staves
/// that share aligned barlines.
///
/// Mirrors `PeakGraph.findAllAlignments`: every peak first receives its
/// deskewed center through the sheet skew, then `checkAlignment` scores each
/// candidate pair by inverted-slope agreement and width delta. Systems are the
/// connected components over the surviving alignment edges, which is how Java
/// derives its staff grouping before the connection purges.
fn build_peak_graph(
    staff_peaks: &mut [Vec<StaffPeak>],
    alignment_staffs: &[AlignmentStaff],
    global_slope: f64,
    interline: i32,
    raster_pixels: &[u8],
    lags: &InitialGridLags,
    (width, height): (usize, usize),
) -> Result<PeakGraphReport, GridRecognitionError> {
    let skew = HeadlessSkew::new(
        global_slope,
        i32::try_from(width).unwrap_or(i32::MAX),
        i32::try_from(height).unwrap_or(i32::MAX),
    );
    let mut graph: PeakGraph<BarAlignment> = PeakGraph::new();
    for peaks in staff_peaks.iter_mut() {
        for peak in peaks.iter_mut() {
            peak.compute_deskewed_center(|point| skew.deskewed(point))
                .map_err(grid_stage("deskewed center"))?;
            if !graph.add_vertex(peak.clone()) {
                return Err(GridRecognitionError::Stage {
                    stage: "peak graph",
                    message: format!("duplicate peak key {:?}", peak.key()),
                });
            }
        }
    }

    let mut report = AlignmentBuildReport::default();
    find_all_alignments(
        &mut graph,
        alignment_staffs,
        AlignmentParameters {
            // Java uses the negated sheet skew slope as its vertical reference.
            sheet_slope: global_slope,
            // PeakGraph.maxAlignmentSlope = 0.06 (ratio).
            maximum_alignment_slope: 0.06,
            // maxAlignmentDeltaWidth = rint(0.6 * interline).
            maximum_alignment_delta_width: (0.6 * f64::from(interline)).round_ties_even() as i32,
        },
        &mut report,
    )
    .map_err(grid_stage("alignments"))?;

    let alignment_count = graph.edges().len();

    // Java `PeakGraph.buildBarSticks` registers one vertical filament per
    // peak; peaks without an acceptable stick are dropped from the graph.
    let peak_order: Vec<StaffPeakKey> = graph.vertices().iter().map(StaffPeak::key).collect();
    let stick_parameters = BarStickParameters {
        // PeakGraph.bracketLookupExtension = rint(2.0 * interline).
        vertical_extension: pixels(2.0, interline),
        // BarFilamentFactory.minCoreSectionLength = rint(0.5 * interline).
        minimum_core_section_length: pixels(0.5, interline).max(1) as usize,
        // Filament.probeWidth = rint(0.5 * interline).
        probe_width: pixels(0.5, interline).max(1) as usize,
        // Filament.probeMinWeight = rint(0.2 * interline).
        minimum_probe_weight: pixels(0.2, interline).max(1) as usize,
        // BarFilamentFactory.segmentLength = rint(1 * interline).
        segment_length: pixels(1.0, interline).max(1) as usize,
        // PeakGraph.minBarCurvature = rint(10 * interline).
        minimum_mean_curvature: f64::from(pixels(10.0, interline)),
        first_filament_id: 1,
    };
    let mut stick_state = BarStickBuildState::new(1).map_err(grid_stage("bar stick state"))?;
    build_bar_sticks(
        &mut graph,
        &peak_order,
        &lags.vertical,
        &lags.horizontal,
        stick_parameters,
        &mut stick_state,
    )
    .map_err(grid_stage("bar sticks"))?;
    let stick_count = stick_state.sticks().len();
    let stickless_peak_count = stick_state.removed_peaks().len();

    // Java `PeakGraph.findConnections` promotes an alignment when the
    // inter-staff corridor is mostly ink: gap within one interline and white
    // ratio at most 0.25. Only connected peaks bind two staves into a system.
    let mut connection_report = ConnectionBuildReport::default();
    find_connections(
        &mut graph,
        ConnectionRaster {
            width,
            height,
            pixels: raster_pixels,
        },
        stick_state.sticks(),
        ConnectionParameters {
            maximum_gap: pixels(1.0, interline),
            maximum_white_ratio: 0.25,
        },
        &mut connection_report,
    )
    .map_err(grid_stage("connections"))?;

    let mut connection_count = 0usize;
    let mut connected_pairs: Vec<(usize, usize)> = Vec::new();
    for decision in connection_report.decisions() {
        if decision.promoted_edge.is_none() {
            continue;
        }
        connection_count += 1;
        if let (Some(source), Some(target)) = (
            staff_of_key(staff_peaks, alignment_staffs, decision.source),
            staff_of_key(staff_peaks, alignment_staffs, decision.target),
        ) {
            connected_pairs.push((source, target));
        }
    }

    // Every staff starts alone; connected pairs merge their groups.
    let mut systems: Vec<Vec<usize>> = alignment_staffs
        .iter()
        .map(|staff| vec![staff.staff_id.value()])
        .collect();
    for (source, target) in connected_pairs {
        systems.push(vec![source, target]);
    }
    merge_overlapping(&mut systems);

    Ok(PeakGraphReport {
        alignment_count,
        connection_count,
        stick_count,
        stickless_peak_count,
        systems,
    })
}

/// Java `Scale.toPixels(Fraction)`: `rint(ratio * interline)`, ties to even.
fn pixels(ratio: f64, interline: i32) -> i32 {
    (ratio * f64::from(interline)).round_ties_even() as i32
}

/// Resolves which staff owns a peak key.
fn staff_of_key(
    staff_peaks: &[Vec<StaffPeak>],
    staffs: &[AlignmentStaff],
    key: StaffPeakKey,
) -> Option<usize> {
    staff_peaks
        .iter()
        .position(|peaks| peaks.iter().any(|peak| peak.key() == key))
        .and_then(|index| staffs.get(index))
        .map(|staff| staff.staff_id.value())
}

/// Collapses staff groups that share a staff into single systems.
fn merge_overlapping(groups: &mut Vec<Vec<usize>>) {
    let mut merged = true;
    while merged {
        merged = false;
        'outer: for i in 0..groups.len() {
            for j in (i + 1)..groups.len() {
                if groups[i].iter().any(|id| groups[j].contains(id)) {
                    let other = groups.remove(j);
                    groups[i].extend(other);
                    groups[i].sort_unstable();
                    groups[i].dedup();
                    merged = true;
                    break 'outer;
                }
            }
        }
    }
    groups.sort();
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
    let lags = build_initial_grid_lags(&tables, seed_parameters.raster.maximum_vertical_run_shift);

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
    let mut staff_peaks: Vec<Vec<StaffPeak>> = Vec::with_capacity(result.staffs().len());
    let mut alignment_staffs: Vec<AlignmentStaff> = Vec::with_capacity(result.staffs().len());
    for staff in result.staffs() {
        let projected = project_staff_peaks(
            &scale_recognition,
            projector_pixels,
            staff,
            &primary,
            &projector_scale,
        )?;
        let peaks = projected
            .iter()
            .map(|peak| {
                (
                    peak.start(),
                    peak.stop(),
                    peak.impacts().map_or(0.0, |impacts| impacts.grade()),
                )
            })
            .collect();
        // Java `AlignmentStaff` top/bottom are the first and last line
        // ordinates taken at the staff's left end.
        let line_ordinate = |index: usize| -> Result<f64, GridRecognitionError> {
            let id = staff.line_ids()[index];
            primary
                .filaments()
                .get(&id)
                .and_then(|filament| filament.geometry().ok())
                .and_then(|geometry| geometry.position_at(staff.left()).ok())
                .ok_or_else(|| GridRecognitionError::Stage {
                    stage: "alignment",
                    message: format!("staff {} line ordinate is unavailable", staff.id()),
                })
        };
        alignment_staffs.push(AlignmentStaff {
            staff_id: StaffId::new(staff.id()),
            left: staff.left(),
            right: staff.right(),
            top: line_ordinate(0)?,
            bottom: line_ordinate(staff.line_ids().len().saturating_sub(1))?,
            short: staff.is_short(),
            peaks: projected.iter().map(StaffPeak::key).collect(),
        });
        staff_peaks.push(projected);
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
    let peak_graph = build_peak_graph(
        &mut staff_peaks,
        &alignment_staffs,
        global_slope,
        scale_recognition.scale.interline.main,
        projector_pixels,
        &lags,
        (scale_recognition.width, scale_recognition.height),
    )?;
    let discarded_filament_count = result.primary().discarded_filaments().len();

    Ok(GridLinesRecognition {
        scale: scale_recognition,
        global_slope,
        filament_count,
        sloped_reject_count,
        discarded_filament_count,
        staves,
        peak_graph,
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
    report.push_str(&format!(
        "systems=alignments:{};connections:{};sticks:{};stickless:{};groups:{}\n",
        recognition.peak_graph.alignment_count,
        recognition.peak_graph.connection_count,
        recognition.peak_graph.stick_count,
        recognition.peak_graph.stickless_peak_count,
        if recognition.peak_graph.systems.is_empty() {
            "none".to_owned()
        } else {
            recognition
                .peak_graph
                .systems
                .iter()
                .enumerate()
                .map(|(index, ids)| {
                    let members = ids
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("#{}[{members}]", index + 1)
                })
                .collect::<Vec<_>>()
                .join(" ")
        }
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
    fn systems_match_the_java_oracle_on_representative_pages() {
        // One page per system shape seen in the corpus, from live Java 5.11
        // GRID runs ("PeakGraph | Systems: ...").
        let cases: [(&str, Vec<Vec<usize>>); 3] = [
            // Grand-staff piano: two-staff systems.
            ("chula.png", vec![vec![1, 2], vec![3, 4], vec![5, 6]]),
            // Single-staff score: each staff is its own system.
            (
                "hove.png",
                vec![vec![1], vec![2], vec![3], vec![4], vec![5]],
            ),
            // Mixed two- and three-staff systems.
            (
                "D0392410-1.256.png",
                vec![vec![1, 2], vec![3, 4], vec![5, 6, 7], vec![8, 9, 10]],
            ),
        ];
        for (name, expected) in cases {
            let recognition = recognize_grid_lines(repo_path(&format!("data/examples/{name}")))
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(
                recognition.peak_graph.systems, expected,
                "{name} system grouping diverged"
            );
        }
    }

    /// Full nine-page corpus sweep. Excluded from the default suite because
    /// a debug-build run costs about a minute; run with
    /// `cargo test -p audiveris-omr -- --ignored`.
    #[test]
    #[ignore]
    fn systems_match_the_java_oracle_across_the_example_corpus() {
        // Staff groupings logged by live Java Audiveris 5.11 GRID runs
        // ("PeakGraph | Systems: ...") on each example page.
        const JAVA_SYSTEMS: [(&str, &[&[usize]]); 9] = [
            (
                "D0392410-1.256.png",
                &[&[1, 2], &[3, 4], &[5, 6, 7], &[8, 9, 10]],
            ),
            ("allegretto.png", &[&[1, 2], &[3, 4], &[5, 6]]),
            ("batuque.png", &[&[1, 2], &[3, 4], &[5, 6]]),
            (
                "carmen.png",
                &[&[1, 2], &[3, 4], &[5, 6], &[7, 8], &[9, 10]],
            ),
            ("chula.png", &[&[1, 2], &[3, 4], &[5, 6]]),
            ("cucaracha.png", &[&[1, 2], &[3, 4], &[5, 6]]),
            // Single-staff score: every staff is its own system.
            ("hove.png", &[&[1], &[2], &[3], &[4], &[5]]),
            ("zizi.png", &[&[1, 2], &[3, 4]]),
            (
                "BachInvention5.jpg",
                &[&[1, 2], &[3, 4], &[5, 6], &[7, 8], &[9, 10], &[11, 12]],
            ),
        ];
        for (name, expected) in JAVA_SYSTEMS {
            let recognition = recognize_grid_lines(repo_path(&format!("data/examples/{name}")))
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            let systems: Vec<Vec<usize>> = recognition.peak_graph.systems.clone();
            let expected: Vec<Vec<usize>> = expected.iter().map(|ids| ids.to_vec()).collect();
            assert_eq!(systems, expected, "{name} system grouping diverged");
        }
    }

    #[test]
    fn missing_file_reports_load_error() {
        let error = recognize_scale(repo_path("data/examples/does-not-exist.png"))
            .expect_err("missing file must fail");
        assert!(matches!(error, ScaleRecognitionError::Load(_)));
    }
}
