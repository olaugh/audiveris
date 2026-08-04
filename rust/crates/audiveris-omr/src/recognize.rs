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

use std::collections::BTreeMap;
use std::path::Path;

use crate::grid_executor::HeadlessSkew;
use crate::production_stages::TerminalRasterStages;
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
use audiveris_image::bars_coordinator::{
    BarsPurgeParameters, BarsRightEvidence, BarsRootEvidence, BarsStaffState, BarsSystemState,
};
use audiveris_image::grid_lifecycle::{GridStepExecutor, GridStepStage};

use crate::grid_executor::{
    HeadlessGridBook, HeadlessGridExecutor, HeadlessGridSheet, HeadlessGridSigState,
    HeadlessPopulationState, HeadlessStaff,
};
use audiveris_image::ingest::{self, LoadError};
use audiveris_image::line_completion::LineCompletionStage;
use audiveris_image::line_short_sections::HorizontalSectionLag;
use audiveris_image::lines_coordinator::{
    ClusterPassState, StaffCandidate, retrieve_staff_candidates,
};
use audiveris_image::peak_graph::PeakGraph;
use audiveris_image::prepared_bars::ProductionProcessBars;
use audiveris_image::prepared_completion::{ProductionCompleteLines, production_line_completion};
use audiveris_image::prepared_lines::{PreparedStaff, RawProductionRetrieveLines};
use audiveris_image::production_grid_params::{
    ProductionGridParameters, production_grid_parameters,
};
use audiveris_image::projection::{
    BarlineHeightSpec, NeutralStaffProjectorRequest, PeakConstructionParams, PeakCoreGeometry,
    PeakCoreParams, PeakRefinementParams, ProjectionBlank, ShortProjection, StaffProjectionRequest,
    StaffProjectorScaleParameters, StaffProjectorScaleRatios, StaffProjectorScaleRequest,
    has_blank_between, staff_projector_scale_parameters,
};
use audiveris_image::raster_grid_builder::HeadlessRasterGridBuilder;
use audiveris_image::raw_line_adapter::build_primary_cluster_pass;
use audiveris_image::run_table::{Orientation, RunTable, RunTableError, create_grid_run_tables};
use audiveris_image::scale_estimate::{
    ScaleEstimate, ScaleEstimateError, ScaleOptions, estimate_scale,
};
use audiveris_image::scale_runs::vertical_run_histograms;
use audiveris_image::section::{InitialGridLags, build_initial_grid_lags};
use audiveris_image::staff_peak::{HorizontalSide, PeakBounds, StaffPeak, StaffPeakKey};

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
        "page={}x{}/{:016x}\nscale=line:{},{},{};interline:{},{},{};small-interline:{};beam:{};small-beam:{}\nresolution={:?}\n",
        recognition.width,
        recognition.height,
        recognition.gray_digest,
        scale.line.min,
        scale.line.main,
        scale.line.max,
        scale.interline.min,
        scale.interline.main,
        scale.interline.max,
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
    /// Peaks surviving the `BarsRetriever` purges, i.e. real barlines.
    pub retained_peaks: usize,
    pub purged_peaks: usize,
    /// Surviving peak abscissae per staff id, for oracle diffing.
    pub surviving_barlines: Vec<(usize, Vec<i32>)>,
    /// Staff ids grouped by shared aligned barlines, in Java's system order.
    pub systems: Vec<Vec<usize>>,
    /// What the ported `completeLines` chain did to the retrieved staff lines.
    pub completion: LineCompletionReport,
    /// The sheet SIG the GRID step promoted: barline, bracket, brace, and
    /// connector inters per system, plus their relations.
    pub sig: HeadlessGridSigState,
    /// Staffs the GRID step installed on the sheet, with their recorded
    /// barlines.
    pub sheet_staffs: Vec<HeadlessStaff>,
}

/// Tallies from Java `LinesRetriever.completeLines`.
///
/// The stage mutates staff-line geometry rather than producing a value, so its
/// per-stage counts are what a run can be compared on.
#[derive(Debug, Clone)]
pub struct LineCompletionReport {
    /// Every stage that ran, in Java's fixed order.
    pub completed_stages: Vec<LineCompletionStage>,
    /// Staves whose endpoints `defineEndPoints` fixed.
    pub endpoint_staffs: usize,
    /// Sections Java `getAllStickers` offered to `includeStickers`.
    pub sticker_candidates: usize,
    pub sticker_batches: usize,
    pub section_batches: usize,
    pub discarded_filament_steals: usize,
    pub curvature_removals: usize,
    /// Java calls `fillHoles` three times; each call is a separate invocation.
    pub hole_fill_invocations: usize,
    /// Final staff-line geometry, the actual product of `completeLines`. One
    /// entry per staff, each holding its lines top to bottom.
    pub staff_lines: Vec<CompletedStaffLines>,
}

/// Final geometry of one staff's lines after `completeLines`.
#[derive(Debug, Clone)]
pub struct CompletedStaffLines {
    pub staff_id: usize,
    /// Per line, the spline endpoints as `(start_x, start_y, stop_x, stop_y)`.
    /// This is what Java persists as the staff line in `sheet#N.xml`.
    pub lines: Vec<(f64, f64, f64, f64)>,
}

/// Reads each completed staff line's endpoints out of its filament.
///
/// A filament whose spline cannot be rebuilt is skipped rather than faked; a
/// short line list is a visible signal, an invented endpoint is not.
fn completed_staff_lines(staffs: &[PreparedStaff]) -> Vec<CompletedStaffLines> {
    staffs
        .iter()
        .map(|staff| CompletedStaffLines {
            staff_id: staff.id,
            lines: staff
                .lines
                .iter()
                .filter_map(|line| {
                    let geometry = line.filament.geometry().ok()?;
                    let (start, stop) = (geometry.start(), geometry.stop());
                    Some((start.0, start.1, stop.0, stop.1))
                })
                .collect(),
        })
        .collect()
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
) -> Result<(Vec<StaffPeak>, Vec<ProjectionBlank>, i32, i32), GridRecognitionError> {
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
        return Ok((
            Vec::new(),
            Vec::new(),
            scale_parameters.minimum_standard_blank_width,
            staff.left().round() as i32,
        ));
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

    // Java's projector marks the peak sitting at the staff's left end as
    // STAFF_LEFT_END, and `purgeTooLeft` exempts marked peaks. Without the
    // mark every staff loses its opening barline to that purge.
    let mut peaks = result.peaks;
    let staff_left = staff.left().round() as i32;
    // The opening bar starts at or before the staff edge and sits within
    // Java's `maxLeftExtremum` of it; anything further left is a brace or
    // bracket, which `purgeTooLeft` already exempts on its own.
    // Take the rightmost such peak: anything further left is a brace or
    // bracket, and marking that instead would leave the real opening bar
    // exposed to the purge.
    if let Some(opening) = peaks.iter_mut().rfind(|peak| peak.start() <= staff_left) {
        opening.set_staff_end(HorizontalSide::Left);
    }
    Ok((
        peaks,
        result.all_blanks,
        scale_parameters.minimum_standard_blank_width,
        staff_left,
    ))
}

/// Builds Java `PeakGraph` alignments across staves and groups the staves
/// that share aligned barlines.
///
/// Mirrors `PeakGraph.findAllAlignments`: every peak first receives its
/// deskewed center through the sheet skew, then `checkAlignment` scores each
/// candidate pair by inverted-slope agreement and width delta. Systems are the
/// connected components over the surviving alignment edges, which is how Java
/// derives its staff grouping before the connection purges.
#[allow(clippy::too_many_arguments)]
fn build_peak_graph(
    staff_peaks: &mut [Vec<StaffPeak>],
    alignment_staffs: &[AlignmentStaff],
    global_slope: f64,
    interline: i32,
    fore: i32,
    raster_pixels: &[u8],
    lags: &InitialGridLags,
    staff_blanks: &[BTreeMap<StaffPeakKey, bool>],
    staff_projection_blanks: &[Vec<ProjectionBlank>],
    projector_scale: &StaffProjectorScaleParameters,
    production: &ProductionGridParameters,
    source: &RunTable,
    (width, height): (usize, usize),
) -> Result<PeakGraphReport, GridRecognitionError> {
    let bars_parameters = &production.bars;
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

    // Java `PeakGraph.buildSystems` order: findAllAlignments, findConnections,
    // splitMergedGroups, then purgeAlignments -- on one sheet-wide graph, which
    // is also the graph the per-system columns are later built from.
    graph.purge_alignments();

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

    let derived = derive_bars_systems(
        staff_peaks,
        alignment_staffs,
        &systems,
        &stick_state,
        staff_blanks,
        &graph,
    )?;

    // Java `BarsRetriever.process` per system: drop peaks left of the staff
    // start, unaligned peaks, curved and short peaks, then run the width,
    // C-clef, and column purges, and finally `purgeExtendingPeaks`. This
    // narrows the raw projector candidates to real barlines, and is exactly the
    // ported `ProductionProcessBars` stage.
    // Java `verifyLinesRoot` and `refineRightEnds` read the staff's projection
    // blanks, the sheet's right edge, and the projector's blank/extremum
    // thresholds. `sheet_right` is Java's `sheet.getWidth() - 1`.
    let sheet_right = i32::try_from(width).unwrap_or(i32::MAX).saturating_sub(1);
    let (root_evidence, right_evidence): (Vec<_>, Vec<_>) = alignment_staffs
        .iter()
        .enumerate()
        .map(|(index, staff)| {
            let blanks = staff_projection_blanks
                .get(index)
                .cloned()
                .unwrap_or_default();
            (
                BarsRootEvidence {
                    staff_id: staff.staff_id,
                    blanks: blanks.clone(),
                    minimum_small_blank_width: projector_scale.minimum_small_blank_width,
                    maximum_left_extremum: projector_scale.maximum_left_extremum,
                },
                BarsRightEvidence {
                    staff_id: staff.staff_id,
                    blanks,
                    sheet_right,
                    minimum_small_blank_width: projector_scale.minimum_small_blank_width,
                    maximum_right_extremum: projector_scale.maximum_right_extremum,
                },
            )
        })
        .unzip();

    let candidate_peaks: usize = staff_peaks.iter().map(Vec::len).sum();
    let bars = ProductionProcessBars::new(
        RawProductionRetrieveLines::new(
            production.raw_primary.clone(),
            production.lines,
            TerminalRasterStages::new(),
        ),
        derived.systems,
        *bars_parameters,
        production.maximum_group_gap,
    )
    .map_err(grid_stage("process bars"))?
    .with_extending_purge(
        derived.filament_bounds,
        BarsPurgeParameters {
            // BarsRetriever.largeSystemStaffCount = 4.
            large_system_staff_count: 4,
            maximum_foreground_thickness: fore,
            // maxBarExtension = rint(1.0 * interline).
            maximum_bar_extension: f64::from(pixels(1.0, interline)),
        },
    )
    .with_staff_limit_refinement(root_evidence, right_evidence);
    // The full ported decorator chain, composed as
    // `HeadlessGridExecutor::from_completed_raw_bars_complete_lines` does.
    // `retrieveLines` runs inside it and republishes the staffs, so
    // `ProductionProcessBars` cross-checks the systems derived above against
    // the ported line retrieval before purging anything.
    let stages = ProductionCompleteLines::new(
        bars,
        production_line_completion(production.completion),
        Some(source.clone()),
        production.maximum_thin_weight,
        production.inspect_crossing_chunks,
    )
    .with_completion_systems_from_prepared_bars();
    // Java `GridStep.doit` runs the builder inside the step, which promotes the
    // surviving peaks into the sheet's SIG as barline, bracket, brace, and
    // connector inters and installs the staffs. Driving the builder alone stops
    // short of all of that, so the executor owns the run.
    let builder = HeadlessRasterGridBuilder::new(source.clone(), production.raster, stages);
    let mut executor = HeadlessGridExecutor::new(
        builder,
        HeadlessGridSheet {
            sheet_number: 1,
            population: HeadlessPopulationState {
                sheet_width: i32::try_from(width).unwrap_or(i32::MAX),
                sheet_height: i32::try_from(height).unwrap_or(i32::MAX),
                ..HeadlessPopulationState::default()
            },
            ..HeadlessGridSheet::default()
        },
        HeadlessGridBook::default(),
    )
    .with_raster_grid_handoff()
    .with_raw_line_metadata_handoff()
    .with_prepared_staff_handoff()
    .with_prepared_bars_handoff();
    executor
        .run_grid_step_stage(GridStepStage::BuildGrid)
        .map_err(grid_stage("grid build"))?;
    let builder = &mut executor.builder;

    if std::env::var_os("AUDIVERIS_DEBUG_PURGE").is_some() {
        for (system_id, removed) in builder.stages().upstream().removals() {
            eprintln!(
                "purge system={system_id} peak={:?} stage={:?}",
                removed.peak, removed.stage
            );
        }
    }
    let completion = builder
        .stages()
        .state()
        .ok_or_else(|| GridRecognitionError::Stage {
            stage: "complete lines",
            message: "stage published no completion state".to_owned(),
        })
        .map(|state| LineCompletionReport {
            completed_stages: state.completed_stages.clone(),
            endpoint_staffs: state.defined_endpoints.len(),
            sticker_candidates: state.sticker_section_ids.len(),
            sticker_batches: state.sticker_inclusion_batches.len(),
            section_batches: state.section_inclusion_batches.len(),
            discarded_filament_steals: state.discarded_filament_steals.len(),
            curvature_removals: state.curvature_removals.len(),
            hole_fill_invocations: state.fill_hole_invocations.len(),
            staff_lines: completed_staff_lines(&state.staffs),
        })?;
    // The executor consumed the prepared bars handoff to build the sheet SIG,
    // so the post-purge peaks are read from there. That is also the structure
    // Java ends GRID with, which makes it the right thing to diff.
    let mut surviving: Vec<(usize, Vec<i32>)> = Vec::new();
    for system in &executor.sheet.sig.systems {
        for (staff_id, peaks) in system.staff_ids.iter().zip(system.staff_peaks.iter()) {
            surviving.push((*staff_id, peaks.iter().map(StaffPeak::start).collect()));
        }
    }
    let retained_peaks: usize = surviving.iter().map(|(_, peaks)| peaks.len()).sum();
    let purged_peaks = candidate_peaks.saturating_sub(retained_peaks);

    Ok(PeakGraphReport {
        sig: executor.sheet.sig.clone(),

        sheet_staffs: executor.sheet.staffs.clone(),
        alignment_count,
        connection_count,
        stick_count,
        stickless_peak_count,
        retained_peaks,
        purged_peaks,
        surviving_barlines: surviving,
        systems,
        completion,
    })
}

/// Per-system bars state, derived but not yet purged.
///
/// This is the seam between deriving `BarsRetriever` input and running its
/// purges. `ProductionProcessBars` takes an already-built `Vec<BarsSystemState>`
/// rather than deriving one, so anything driving the ported GRID decorator
/// chain needs exactly this, produced exactly this way.
#[derive(Debug)]
pub struct DerivedBarsSystems {
    /// One state per system, in Java's system order, ids starting at 1.
    pub systems: Vec<BarsSystemState>,
    /// Bar-filament bounds keyed by peak, for the `purgeExtendingPeaks` stage
    /// that `process_bars_system` does not cover.
    pub filament_bounds: Vec<(StaffPeakKey, PeakBounds)>,
}

/// Builds each system's `BarsSystemState` from the sheet-wide peak derivation.
///
/// Java rebuilds alignments inside each `SystemInfo` rather than reusing the
/// sheet graph, so this re-runs `findAllAlignments` and `purgeAlignments` over
/// a graph holding only that system's peaks.
fn derive_bars_systems(
    staff_peaks: &[Vec<StaffPeak>],
    alignment_staffs: &[AlignmentStaff],
    systems: &[Vec<usize>],
    stick_state: &BarStickBuildState,
    staff_blanks: &[BTreeMap<StaffPeakKey, bool>],
    sheet_graph: &PeakGraph<BarAlignment>,
) -> Result<DerivedBarsSystems, GridRecognitionError> {
    let filament_bounds: Vec<(StaffPeakKey, PeakBounds)> = stick_state
        .sticks()
        .iter()
        .map(|stick| {
            (
                stick.peak,
                PeakBounds {
                    x: i32::try_from(stick.bounds.x).unwrap_or(i32::MAX),
                    y: i32::try_from(stick.bounds.y).unwrap_or(i32::MAX),
                    width: i32::try_from(stick.bounds.width).unwrap_or(i32::MAX),
                    height: i32::try_from(stick.bounds.height).unwrap_or(i32::MAX),
                },
            )
        })
        .collect();

    let mut states = Vec::with_capacity(systems.len());
    for (system_index, member_ids) in systems.iter().enumerate() {
        let mut bars_staffs = Vec::with_capacity(member_ids.len());
        let mut system_graph: PeakGraph<BarAlignment> = PeakGraph::new();
        for staff_id in member_ids {
            let Some(index) = alignment_staffs
                .iter()
                .position(|staff| staff.staff_id.value() == *staff_id)
            else {
                continue;
            };
            let peaks = staff_peaks[index].clone();
            for peak in &peaks {
                system_graph.add_vertex(peak.clone());
            }
            bars_staffs.push(
                BarsStaffState::new(
                    StaffId::new(*staff_id),
                    alignment_staffs[index].left.round() as i32,
                    false,
                    peaks,
                    staff_blanks[index].clone(),
                )
                .map_err(grid_stage("bars staff state"))?
                .with_right(alignment_staffs[index].right.round() as i32),
            );
        }
        // Java keeps a single sheet-wide `PeakGraph` and builds each system's
        // columns from it, so this system's graph is that graph's induced
        // subgraph, not a fresh re-derivation. Rebuilding would re-run
        // `findAllAlignments` and drop every `findConnections` promotion, which
        // leaves `BarColumn::is_fully_connected` false for every column and so
        // no start column at all.
        let members: std::collections::BTreeSet<StaffPeakKey> =
            system_graph.vertices().iter().map(StaffPeak::key).collect();
        for edge in sheet_graph.edges() {
            if members.contains(&edge.source()) && members.contains(&edge.target()) {
                system_graph
                    .add_edge(edge.source(), edge.target(), *edge.relation())
                    .map_err(grid_stage("system graph edge"))?;
            }
        }

        states.push(
            BarsSystemState::new(system_index + 1, bars_staffs, system_graph)
                .map_err(grid_stage("bars system state"))?,
        );
    }

    Ok(DerivedBarsSystems {
        systems: states,
        filament_bounds,
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
    let pass = build_primary_cluster_pass(&lag, parameters.raw_primary.clone())
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
    let mut staff_blanks: Vec<BTreeMap<StaffPeakKey, bool>> = Vec::new();
    // Java `StaffProjector` keeps its blanks for the two stages that set a
    // staff's abscissae; the port previously used them only for the
    // blank-to-lines test and dropped the rest.
    let mut staff_projection_blanks: Vec<Vec<ProjectionBlank>> = Vec::new();
    for staff in result.staffs() {
        let (projected, blanks, minimum_standard_blank, refined_left) = project_staff_peaks(
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
            left: f64::from(refined_left),
            right: staff.right(),
            top: line_ordinate(0)?,
            bottom: line_ordinate(staff.line_ids().len().saturating_sub(1))?,
            short: staff.is_short(),
            peaks: projected.iter().map(StaffPeak::key).collect(),
        });
        // Java `BarsRetriever.detectStartColumns` asks each projector
        // `hasStandardBlank(peak.getStop(), xLeft)`: the range runs from the
        // peak's stop rightwards to the staff's own left abscissa, so a peak
        // separated from its lines by a standard blank cannot start the staff.
        // The range is directional -- `hasStandardBlank` returns false outright
        // when `stop <= start` -- so the argument order is load-bearing, and
        // `xLeft` must be the same left the start-column check compares against.
        let blank_evidence: BTreeMap<StaffPeakKey, bool> = projected
            .iter()
            .map(|peak| {
                (
                    peak.key(),
                    has_blank_between(&blanks, peak.stop(), refined_left, minimum_standard_blank),
                )
            })
            .collect();
        staff_blanks.push(blank_evidence);
        staff_projection_blanks.push(blanks);
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
        scale_recognition.scale.line.main,
        projector_pixels,
        &lags,
        &staff_blanks,
        &staff_projection_blanks,
        &projector_scale,
        &parameters,
        &scale_recognition.vertical_runs,
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
        "systems=alignments:{};connections:{};sticks:{};stickless:{};barlines:{};purged:{};groups:{}\n",
        recognition.peak_graph.alignment_count,
        recognition.peak_graph.connection_count,
        recognition.peak_graph.stick_count,
        recognition.peak_graph.stickless_peak_count,
        recognition.peak_graph.retained_peaks,
        recognition.peak_graph.purged_peaks,
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
    let completion = &recognition.peak_graph.completion;
    report.push_str(&format!(
        "completion=stages:{};endpoints:{};sections:{};stickers:{}/{};steals:{};curvature:{};holes:{}\n",
        completion.completed_stages.len(),
        completion.endpoint_staffs,
        completion.section_batches,
        completion.sticker_batches,
        completion.sticker_candidates,
        completion.discarded_filament_steals,
        completion.curvature_removals,
        completion.hole_fill_invocations,
    ));
    for (staff_id, kept) in &recognition.peak_graph.surviving_barlines {
        report.push_str(&format!(
            "barlines={staff_id}:{}\n",
            kept.iter()
                .map(i32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
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
    use audiveris_image::bars_logic::{PeakWidthClass, VerticalInterKind};
    use audiveris_image::grid_sig::GridSigNode;
    use std::collections::BTreeSet;

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
             scale=line:2,3,4;interline:20,21,22;small-interline:null;beam:12;small-beam:null\n\
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
            let expected_barlines: usize = match name {
                "D0392410-1.256.png" => 53,
                "allegretto.png" => 44,
                "batuque.png" => 64,
                "carmen.png" => 76,
                "chula.png" => 58,
                "cucaracha.png" => 40,
                "hove.png" => 17,
                "zizi.png" => 22,
                _ => 46,
            };
            assert_eq!(
                recognition.peak_graph.retained_peaks, expected_barlines,
                "{name} barline total diverged from Java"
            );
            let expected: Vec<Vec<usize>> = expected.iter().map(|ids| ids.to_vec()).collect();
            assert_eq!(systems, expected, "{name} system grouping diverged");
        }
    }

    /// The prepared bars handoff is what `completeLines` will consume, so it
    /// must agree with the barline report that is already oracle-locked rather
    /// than being a parallel, unvalidated view of the same page.
    /// One Java `completeLines` record: a page, its staff abscissae, and every
    /// completed staff line's endpoints.
    #[derive(Default)]
    struct JavaCompletion {
        staffs: Vec<(usize, i32, i32)>,
        lines: Vec<(usize, f64, f64, f64, f64)>,
    }

    /// Parses `rust/oracle/grid-completed-lines.txt`, keyed by page file name.
    fn java_completion_oracle() -> BTreeMap<String, JavaCompletion> {
        let text = include_str!("../../../oracle/grid-completed-lines.txt");
        let mut pages: BTreeMap<String, JavaCompletion> = BTreeMap::new();
        let mut current = String::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split_whitespace().collect();
            match fields.as_slice() {
                ["page", name] => {
                    current = (*name).to_owned();
                    pages.entry(current.clone()).or_default();
                }
                ["staff", id, left, right] => {
                    let entry = pages.get_mut(&current).expect("staff before page");
                    entry.staffs.push((
                        id.parse().expect("staff id"),
                        left.parse().expect("staff left"),
                        right.parse().expect("staff right"),
                    ));
                }
                ["line", id, x1, y1, x2, y2] => {
                    let entry = pages.get_mut(&current).expect("line before page");
                    entry.lines.push((
                        id.parse().expect("staff id"),
                        x1.parse().expect("start x"),
                        y1.parse().expect("start y"),
                        x2.parse().expect("stop x"),
                        y2.parse().expect("stop y"),
                    ));
                }
                _ => panic!("unparsable oracle record: {line}"),
            }
        }
        pages
    }

    /// The only endpoints that do not reproduce Java bit for bit, as
    /// `(page, line index, component)` where component is `1` for the start
    /// ordinate and `3` for the stop ordinate.
    ///
    /// Descends from the JPEG decoder gap; see [`JPEG_DECODER_DIVERGENT_PAGE`].
    /// Not arithmetic: every one is nine or more orders of magnitude above an
    /// f64 ulp, and Java has been strict-IEEE since 17 while Rust never enables
    /// fast-math, so the two already agree bitwise on `+ - * /` and `sqrt`.
    ///
    /// The mechanism from those 844 flipped binary pixels to here is worth
    /// keeping, because it shows how small an input difference needs to be to
    /// matter. A live diff of `Staff.getEndingSlope(LEFT)` showed both runtimes
    /// holding the same number of spline points per line but different member
    /// section counts on exactly the lines whose slopes disagree -- staff 5 line
    /// 4 carries 63 sections against Java's 62, moving its slope from `0.004620`
    /// to `0.008368`. `getEndingSlope` discards the extreme slopes and averages
    /// the middle three, so that one changed slope reorders the sort, a
    /// different value enters the mean, and the staff's ending slope lands at
    /// `0.001469` against Java's `0.000537`. One flipped pixel, amplified 1.7x
    /// by a sort boundary.
    ///
    /// Listing them individually rather than applying a blanket tolerance keeps
    /// every other endpoint pinned to equality, so a new divergence anywhere
    /// fails instead of hiding under a bound.
    /// Each affected line diverges in both ordinates: the start ordinate by up
    /// to 0.085px and the stop ordinate by under 0.0003px.
    const KNOWN_ENDPOINT_RESIDUALS: [(&str, usize, usize); 6] = [
        ("BachInvention5.jpg", 13, 1),
        ("BachInvention5.jpg", 13, 3),
        ("BachInvention5.jpg", 24, 1),
        ("BachInvention5.jpg", 24, 3),
        ("BachInvention5.jpg", 45, 1),
        ("BachInvention5.jpg", 45, 3),
    ];

    /// Largest residual any listed exception may show, in pixels.
    const MAXIMUM_KNOWN_RESIDUAL: f64 = 0.1;

    /// The oracle fixture stores six decimals, so agreement can only be
    /// asserted to half a unit in that last place. Every unlisted endpoint must
    /// land inside this; it is the fixture's precision, not a tolerance for
    /// disagreement.
    const ORACLE_PRECISION: f64 = 5e-7;

    /// Diffs GRID's staff abscissae and completed line geometry against a live
    /// Java Audiveris 5.11 run on every example page.
    ///
    /// This is the assertion the staff-limit work was for. Java pins each line
    /// ending at `staff.getAbscissa(side)`, which `BarsRetriever` refines during
    /// `processBars`: the start column sets LEFT, `verifyLinesRoot` may push it
    /// right, and `refineRightEnds` sets RIGHT. All three now travel with the
    /// prepared bars handoff and are adopted before `defineEndPoints` runs.
    #[test]
    fn completed_staff_lines_match_the_java_oracle_across_the_example_corpus() {
        let oracle = java_completion_oracle();
        assert_eq!(oracle.len(), 9, "oracle should cover every example page");
        let mut checked = 0usize;

        for (name, java) in &oracle {
            let recognition = recognize_grid_lines(repo_path(&format!("data/examples/{name}")))
                .unwrap_or_else(|error| panic!("{name}: {error}"));

            // Staff abscissae, in Java's staff order.
            let produced_staffs: Vec<(usize, i32, i32)> = recognition
                .peak_graph
                .sig
                .systems
                .iter()
                .flat_map(|system| {
                    system
                        .staff_ids
                        .iter()
                        .zip(system.staff_limits.iter())
                        .map(|(id, &(left, right))| (*id, left, right))
                })
                .collect();
            assert_eq!(
                produced_staffs, java.staffs,
                "{name} staff abscissae diverged from Java"
            );

            // Completed line endpoints, in Java's staff-then-line order.
            let produced: Vec<(usize, f64, f64, f64, f64)> = recognition
                .peak_graph
                .completion
                .staff_lines
                .iter()
                .flat_map(|staff| {
                    staff
                        .lines
                        .iter()
                        .map(move |line| (staff.staff_id, line.0, line.1, line.2, line.3))
                })
                .collect();
            assert_eq!(
                produced.len(),
                java.lines.len(),
                "{name} produced a different number of staff lines"
            );

            for (index, (produced, java_line)) in produced.iter().zip(&java.lines).enumerate() {
                assert_eq!(
                    produced.0, java_line.0,
                    "{name} line {index} belongs to another staff"
                );
                let produced = [produced.1, produced.2, produced.3, produced.4];
                let expected = [java_line.1, java_line.2, java_line.3, java_line.4];
                for (component, (&produced, &expected)) in
                    produced.iter().zip(expected.iter()).enumerate()
                {
                    checked += 1;
                    let residual = (produced - expected).abs();
                    if residual <= ORACLE_PRECISION {
                        continue;
                    }
                    let known =
                        KNOWN_ENDPOINT_RESIDUALS.contains(&(name.as_str(), index, component));
                    assert!(
                        known && residual < MAXIMUM_KNOWN_RESIDUAL,
                        "{name} line {index} component {component} diverged from Java by \
                         {residual}: {produced} vs {expected}"
                    );
                }
            }
        }
        assert_eq!(
            checked,
            4 * 325,
            "every endpoint component must be compared"
        );
    }

    /// Parses `rust/oracle/grid-barlines.txt`: surviving barline abscissae per
    /// staff, keyed by page file name.
    fn java_barline_oracle() -> BTreeMap<String, Vec<(usize, Vec<i32>)>> {
        let text = include_str!("../../../oracle/grid-barlines.txt");
        let mut pages: BTreeMap<String, Vec<(usize, Vec<i32>)>> = BTreeMap::new();
        let mut current = String::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut fields = line.split_whitespace();
            match fields.next() {
                Some("page") => {
                    current = fields.next().expect("page name").to_owned();
                    pages.entry(current.clone()).or_default();
                }
                Some("staff") => {
                    let id = fields.next().expect("staff id").parse().expect("staff id");
                    let starts = fields.map(|v| v.parse().expect("peak start")).collect();
                    pages
                        .get_mut(&current)
                        .expect("staff before page")
                        .push((id, starts));
                }
                _ => panic!("unparsable oracle record: {line}"),
            }
        }
        pages
    }

    /// The only barlines that do not reproduce Java's abscissa.
    ///
    /// `(page, staff id, index within the staff, Java's value, the port's)`.
    /// Descends from the JPEG decoder gap; see [`JPEG_DECODER_DIVERGENT_PAGE`].
    const KNOWN_BARLINE_RESIDUALS: [(&str, usize, usize, i32, i32); 1] =
        [("BachInvention5.jpg", 10, 1, 744, 745)];

    /// Diffs every surviving barline against a live Java run, position by
    /// position, on every example page.
    ///
    /// `barline_totals_match_the_java_oracle_on_representative_pages` checks
    /// only counts, which a compensating pair of errors would satisfy; this
    /// pins the abscissae themselves.
    #[test]
    fn barline_positions_match_the_java_oracle_across_the_example_corpus() {
        let oracle = java_barline_oracle();
        assert_eq!(oracle.len(), 9, "oracle should cover every example page");
        let mut checked = 0usize;

        for (name, java) in &oracle {
            let recognition = recognize_grid_lines(repo_path(&format!("data/examples/{name}")))
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            let produced = &recognition.peak_graph.surviving_barlines;
            assert_eq!(
                produced.len(),
                java.len(),
                "{name} reported a different number of staves"
            );

            for ((produced_id, produced_starts), (java_id, java_starts)) in
                produced.iter().zip(java)
            {
                assert_eq!(produced_id, java_id, "{name} staff order diverged");
                assert_eq!(
                    produced_starts.len(),
                    java_starts.len(),
                    "{name} staff {java_id} kept {} barlines against Java's {}",
                    produced_starts.len(),
                    java_starts.len()
                );
                for (index, (&produced, &expected)) in
                    produced_starts.iter().zip(java_starts).enumerate()
                {
                    checked += 1;
                    if produced == expected {
                        continue;
                    }
                    assert!(
                        KNOWN_BARLINE_RESIDUALS.contains(&(
                            name.as_str(),
                            *java_id,
                            index,
                            expected,
                            produced
                        )),
                        "{name} staff {java_id} barline {index} diverged: \
                         {produced} vs Java {expected}"
                    );
                }
            }
        }
        assert_eq!(checked, 420, "every barline must be compared");
    }

    /// One Java GRID barline inter, as persisted in `sheet#1.xml`.
    #[derive(Debug, PartialEq)]
    struct JavaBarline {
        staff: usize,
        shape: String,
        width: f64,
        grade: f64,
        ctx_grade: f64,
        frozen: bool,
        staff_end: String,
        median: (f64, f64, f64, f64),
    }

    #[derive(Default)]
    struct JavaSig {
        barlines: Vec<JavaBarline>,
        connectors: Vec<(String, f64, f64, bool)>,
        relations: BTreeMap<String, usize>,
    }

    /// Parses `rust/oracle/grid-sig.txt`, keyed by page file name.
    fn java_sig_oracle() -> BTreeMap<String, JavaSig> {
        let text = include_str!("../../../oracle/grid-sig.txt");
        let mut pages: BTreeMap<String, JavaSig> = BTreeMap::new();
        let mut current = String::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let f: Vec<&str> = line.split_whitespace().collect();
            match f.as_slice() {
                ["page", name] => {
                    current = (*name).to_owned();
                    pages.entry(current.clone()).or_default();
                }
                [
                    "barline",
                    staff,
                    shape,
                    width,
                    grade,
                    ctx,
                    frozen,
                    end,
                    x1,
                    y1,
                    x2,
                    y2,
                ] => {
                    pages
                        .get_mut(&current)
                        .expect("barline before page")
                        .barlines
                        .push(JavaBarline {
                            staff: staff.parse().expect("staff"),
                            shape: (*shape).to_owned(),
                            width: width.parse().expect("width"),
                            grade: grade.parse().expect("grade"),
                            ctx_grade: ctx.parse().expect("ctx grade"),
                            frozen: *frozen == "true",
                            staff_end: (*end).to_owned(),
                            median: (
                                x1.parse().expect("x1"),
                                y1.parse().expect("y1"),
                                x2.parse().expect("x2"),
                                y2.parse().expect("y2"),
                            ),
                        });
                }
                ["connector", shape, width, grade, ctx, frozen] => {
                    pages
                        .get_mut(&current)
                        .expect("connector before page")
                        .connectors
                        .push((
                            (*shape).to_owned(),
                            width.parse().expect("width"),
                            grade.parse().expect("grade"),
                            *frozen == "true",
                        ));
                    let _ = ctx;
                }
                ["relation", kind, count] => {
                    pages
                        .get_mut(&current)
                        .expect("relation before page")
                        .relations
                        .insert((*kind).to_owned(), count.parse().expect("count"));
                }
                _ => panic!("unparsable sig record: {line}"),
            }
        }
        pages
    }

    /// Grades are persisted with three decimals, so agreement is asserted to
    /// half a unit in that last place.
    const SIG_GRADE_PRECISION: f64 = 5e-4;

    /// Per-page ledger of how far the promoted SIG is from Java's, as
    /// `(page, inters differing in a core field, inters differing in a median,
    /// largest median delta, inters differing in a grade, largest grade delta)`.
    ///
    /// Core fields are staff, shape, width, frozen, and staff-end. Counts are
    /// asserted exactly, not as ceilings, so fixing one of these fails the test
    /// and forces the ledger to be updated rather than silently drifting.
    ///
    /// Eight of the nine pages are exact or nearly so: five inters across four
    /// pages differ in grade by at most 0.005, and one chula median sits a pixel
    /// high. `BachInvention5.jpg` carries the rest, including three medians 11
    /// to 18 pixels short at the bottom, and all of it descends from the JPEG
    /// decoder gap; see [`JPEG_DECODER_DIVERGENT_PAGE`].
    const SIG_PAGE_LEDGER: [(&str, usize, usize, f64, usize, f64); 9] = [
        ("BachInvention5.jpg", 1, 9, 18.0, 19, 0.18),
        ("D0392410-1.256.png", 0, 0, 0.0, 2, 0.005),
        ("allegretto.png", 0, 0, 0.0, 0, 0.0),
        ("batuque.png", 0, 0, 0.0, 0, 0.0),
        ("carmen.png", 0, 0, 0.0, 2, 0.004),
        ("chula.png", 0, 1, 1.0, 0, 0.0),
        ("cucaracha.png", 0, 0, 0.0, 1, 0.004),
        ("hove.png", 0, 0, 0.0, 0, 0.0),
        ("zizi.png", 0, 0, 0.0, 0, 0.0),
    ];

    /// Diffs the sheet SIG the GRID step promotes against Java's persisted
    /// `sheet#1.xml` on every example page.
    ///
    /// This is the step's real product. `recognize_grid_lines` drives
    /// `HeadlessGridExecutor`, which promotes every surviving peak into a
    /// barline inter carrying its median, width, intrinsic grade, contextual
    /// grade, frozen flag, and staff-end marks, then installs the staffs with
    /// their recorded barlines.
    ///
    /// Two things are deliberately not compared, and are gaps rather than
    /// omissions: braces, because the port keeps `brace_peaks` detached from the
    /// SIG promotion path so Java's `<brace>` inters have no counterpart; and
    /// connector medians and widths, because `ConnectionInterPlan` carries
    /// neither. Connectors are still compared by count.
    #[test]
    fn sheet_sig_matches_the_java_oracle_across_the_example_corpus() {
        let oracle = java_sig_oracle();
        assert_eq!(oracle.len(), 9, "oracle should cover every example page");
        let mut total = 0usize;

        for (name, java) in &oracle {
            let (_, expected_core, expected_medians, median_bound, expected_grades, grade_bound) =
                SIG_PAGE_LEDGER
                    .iter()
                    .find(|entry| entry.0 == name)
                    .copied()
                    .unwrap_or_else(|| panic!("{name} is missing from the SIG ledger"));

            let recognition = recognize_grid_lines(repo_path(&format!("data/examples/{name}")))
                .unwrap_or_else(|error| panic!("{name}: {error}"));

            let mut produced: Vec<JavaBarline> = Vec::new();
            let mut connectors = 0usize;
            for system in &recognition.peak_graph.sig.systems {
                for (_, node) in system.sig.nodes_in_order() {
                    match node {
                        GridSigNode::Vertical {
                            plan,
                            frozen,
                            contextual_grade,
                            ..
                        } => {
                            let VerticalInterKind::Barline {
                                width_class,
                                left_staff_end,
                                right_staff_end,
                            } = plan.kind
                            else {
                                panic!("{name}: unexpected bracket inter {:?}", plan.kind);
                            };
                            produced.push(JavaBarline {
                                staff: plan.peak.staff_id().value(),
                                shape: match width_class {
                                    PeakWidthClass::Thin => "THIN_BARLINE".to_owned(),
                                    PeakWidthClass::Thick => "THICK_BARLINE".to_owned(),
                                },
                                width: plan.width,
                                grade: plan.impacts.map_or(0.0, |i| i.grade()),
                                ctx_grade: contextual_grade.unwrap_or(f64::NAN),
                                frozen: *frozen,
                                staff_end: if left_staff_end {
                                    "LEFT".to_owned()
                                } else if right_staff_end {
                                    "RIGHT".to_owned()
                                } else {
                                    "NONE".to_owned()
                                },
                                median: (
                                    plan.median.x,
                                    plan.median.top,
                                    plan.median.x,
                                    plan.median.bottom,
                                ),
                            });
                        }
                        GridSigNode::Connector { .. } => connectors += 1,
                    }
                }
            }

            // Java lists inters in its own traversal order; sorting both sides
            // by staff and abscissa compares content without asserting an order
            // the port does not claim to reproduce.
            let key = |b: &JavaBarline| (b.staff, b.median.0);
            let mut expected: Vec<&JavaBarline> = java.barlines.iter().collect();
            expected.sort_by(|a, b| key(a).partial_cmp(&key(b)).expect("finite abscissae"));
            produced.sort_by(|a, b| key(a).partial_cmp(&key(b)).expect("finite abscissae"));

            assert_eq!(
                produced.len(),
                expected.len(),
                "{name} promoted {} barline inters against Java's {}",
                produced.len(),
                expected.len()
            );
            total += produced.len();

            let (mut core, mut medians, mut grades) = (0usize, 0usize, 0usize);
            for (index, (produced, expected)) in produced.iter().zip(&expected).enumerate() {
                if produced.staff != expected.staff
                    || produced.shape != expected.shape
                    || produced.width != expected.width
                    || produced.frozen != expected.frozen
                    || produced.staff_end != expected.staff_end
                {
                    core += 1;
                }
                let median_delta = (produced.median.0 - expected.median.0)
                    .abs()
                    .max((produced.median.1 - expected.median.1).abs())
                    .max((produced.median.3 - expected.median.3).abs());
                if median_delta > 0.0 {
                    medians += 1;
                    assert!(
                        median_delta <= median_bound,
                        "{name} inter {index} median off by {median_delta}, over the \
                         recorded {median_bound}"
                    );
                }
                let grade_delta = (produced.grade - expected.grade)
                    .abs()
                    .max((produced.ctx_grade - expected.ctx_grade).abs());
                if grade_delta > SIG_GRADE_PRECISION {
                    grades += 1;
                    assert!(
                        grade_delta <= grade_bound,
                        "{name} inter {index} grade off by {grade_delta}, over the \
                         recorded {grade_bound}"
                    );
                }
            }
            assert_eq!(core, expected_core, "{name} core-field mismatches");
            assert_eq!(medians, expected_medians, "{name} median mismatches");
            assert_eq!(grades, expected_grades, "{name} grade mismatches");

            assert_eq!(
                connectors,
                java.connectors.len(),
                "{name} promoted {connectors} connector inters against Java's {}",
                java.connectors.len()
            );
        }
        assert_eq!(total, 420, "every promoted barline inter must be compared");
    }

    /// Java's BINARY raster digest per page, from `rust/oracle/grid-binary.txt`.
    fn java_binary_oracle() -> BTreeMap<String, (usize, usize, u64)> {
        let text = include_str!("../../../oracle/grid-binary.txt");
        let mut pages = BTreeMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let f: Vec<&str> = line.split_whitespace().collect();
            let ["page", name, width, height, digest] = f.as_slice() else {
                panic!("unparsable binary record: {line}");
            };
            pages.insert(
                (*name).to_owned(),
                (
                    width.parse().expect("width"),
                    height.parse().expect("height"),
                    u64::from_str_radix(digest, 16).expect("digest"),
                ),
            );
        }
        pages
    }

    /// `BachInvention5.jpg` is the corpus's only JPEG, and its binary raster is
    /// the one input the port does not reproduce bit for bit.
    ///
    /// The cause is the JPEG decoder, not anything ported. Measured on this
    /// page: Java's decode agrees with libjpeg exactly, while `image` 0.25's
    /// `zune-jpeg` differs on 177046 of 5018112 grayscale samples by up to 4.
    /// The adaptive threshold absorbs nearly all of that -- only 844 binary
    /// pixels flip, 0.017%, and 820 of those are isolated single pixels flipping
    /// in both directions, the signature of samples sitting on the threshold.
    /// The older pure-Rust `jpeg-decoder` is no closer (224558 samples, delta 3).
    ///
    /// Proof that nothing downstream is implicated: re-encoding the same page
    /// losslessly to PNG and running both runtimes over it yields **zero**
    /// differing binary pixels. Every remaining GRID divergence on this page --
    /// the one-pixel barline abscissa, the three completed-line endpoints, the
    /// nine SIG medians, the section-membership counts -- descends from these
    /// 844 pixels.
    ///
    /// Closing it needs a libjpeg-compatible decoder, which today means a C
    /// dependency (libjpeg-turbo). That is a dependency decision, not a porting
    /// fix, so the divergence is recorded rather than papered over.
    const JPEG_DECODER_DIVERGENT_PAGE: &str = "BachInvention5.jpg";

    /// Digest of the port's own binary raster for that page, pinned so the
    /// JPEG path still cannot drift unnoticed.
    const JPEG_PAGE_RUST_BINARY_DIGEST: u64 = 0xc267_5dde_2cfc_7545;

    /// Every page's binary raster must equal Java's bit for bit.
    ///
    /// This is the foundation the rest of GRID parity rests on: run tables,
    /// SCALE, filaments, projectors, and peaks all read this raster, so an
    /// input that already differs makes every later comparison unreliable. It is
    /// checked by digest rather than by shipping nine PNGs.
    #[test]
    fn binary_rasters_match_the_java_oracle_across_the_example_corpus() {
        let oracle = java_binary_oracle();
        assert_eq!(oracle.len(), 9, "oracle should cover every example page");
        for (name, &(width, height, digest)) in &oracle {
            let recognition = recognize_scale(repo_path(&format!("data/examples/{name}")))
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(
                (recognition.width, recognition.height),
                (width, height),
                "{name} raster size"
            );
            let produced = audiveris_image::ingest::fnv1a64_bytes(&recognition.binary);
            if name == JPEG_DECODER_DIVERGENT_PAGE {
                assert_ne!(
                    produced, digest,
                    "{name} now matches Java's binary: the JPEG decoder gap has closed, \
                     so fold this page back into the exact assertion below"
                );
                assert_eq!(
                    produced, JPEG_PAGE_RUST_BINARY_DIGEST,
                    "{name} binary raster changed; the JPEG decode is no longer what was measured"
                );
                continue;
            }
            assert_eq!(
                produced, digest,
                "{name} binary raster diverged from Java's"
            );
        }
    }

    #[test]
    fn line_completion_runs_every_java_stage_on_the_example_corpus() {
        const EXPECTED_STAGES: [LineCompletionStage; 10] = [
            LineCompletionStage::DefineEndPoints,
            LineCompletionStage::IncludeDiscardedFilaments,
            LineCompletionStage::FillHolesInitial,
            LineCompletionStage::DispatchHorizontalSections,
            LineCompletionStage::IncludeThickSections,
            LineCompletionStage::IncludeThinSections,
            LineCompletionStage::PolishCurvatures,
            LineCompletionStage::FillHolesAfterPolish,
            LineCompletionStage::IncludeStickers,
            LineCompletionStage::FillHolesFinal,
        ];
        for name in [
            "chula.png",
            "allegretto.png",
            "hove.png",
            "carmen.png",
            "D0392410-1.256.png",
        ] {
            let recognition = recognize_grid_lines(repo_path(&format!("data/examples/{name}")))
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            let completion = &recognition.peak_graph.completion;
            assert_eq!(
                completion.completed_stages, EXPECTED_STAGES,
                "{name} ran completeLines stages out of Java's fixed order"
            );
            let staff_count = recognition.staves.len();
            assert_eq!(
                completion.endpoint_staffs, staff_count,
                "{name} left some staves without defined endpoints"
            );
            // Java dispatches thick and thin sections per staff line, and every
            // example staff has five lines.
            assert_eq!(
                completion.section_batches,
                staff_count * 10,
                "{name} section dispatch skipped a staff line"
            );
            assert_eq!(
                completion.hole_fill_invocations, 3,
                "{name} did not call fillHoles Java's three times"
            );
            assert!(
                completion.sticker_candidates > 0,
                "{name} offered no sticker candidates"
            );
        }
    }

    #[test]
    fn chula_sheet_sig_agrees_with_the_oracle_locked_barlines() {
        let recognition = recognize_grid_lines(repo_path("data/examples/chula.png"))
            .unwrap_or_else(|error| panic!("chula: {error}"));
        let bars = &recognition.peak_graph.sig;

        let staff_ids: Vec<Vec<usize>> = bars
            .systems
            .iter()
            .map(|system| system.staff_ids.clone())
            .collect();
        assert_eq!(staff_ids, vec![vec![1, 2], vec![3, 4], vec![5, 6]]);
        assert_eq!(
            bars.systems
                .iter()
                .map(|system| system.system_id)
                .collect::<Vec<_>>(),
            vec![1, 2, 3],
            "system ids must be Java's 1-based traversal order"
        );

        // Every published peak, in handoff order, must be exactly the surviving
        // barlines the Java oracle pins position by position.
        let published: Vec<(usize, Vec<i32>)> = bars
            .systems
            .iter()
            .flat_map(|system| {
                system
                    .staff_ids
                    .iter()
                    .zip(system.staff_peaks.iter())
                    .map(|(id, peaks)| (*id, peaks.iter().map(StaffPeak::start).collect()))
            })
            .collect();
        assert_eq!(published, recognition.peak_graph.surviving_barlines);
        assert_eq!(
            published
                .iter()
                .map(|(_, peaks)| peaks.len())
                .sum::<usize>(),
            58
        );

        // The handoff graph carries the post-purge peaks, so it holds the
        // survivors and nothing else.
        let graph_keys: BTreeSet<_> = bars
            .peak_graph
            .vertices()
            .iter()
            .map(StaffPeak::key)
            .collect();
        let published_keys: BTreeSet<_> = bars
            .systems
            .iter()
            .flat_map(|system| system.staff_peaks.iter().flatten())
            .map(StaffPeak::key)
            .collect();
        assert_eq!(graph_keys, published_keys);

        // Each connection must name a system that exists in the handoff.
        for connection in &bars.connections {
            assert!(
                bars.systems
                    .iter()
                    .any(|system| system.system_id == connection.system_id),
                "connection references unknown system {}",
                connection.system_id
            );
        }
    }

    #[test]
    fn chula_barlines_match_the_java_oracle_exactly() {
        // Per-staff barline abscissae from a live Java Audiveris 5.11 GRID run
        // (sheet#1.xml barline inter medians). Java reports 58 in total.
        const JAVA: [&[i32]; 6] = [
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
        let surviving = &recognition.peak_graph.surviving_barlines;
        assert_eq!(surviving.len(), JAVA.len());
        for ((staff_id, kept), expected) in surviving.iter().zip(JAVA) {
            assert_eq!(
                kept.len(),
                expected.len(),
                "staff {staff_id} kept {kept:?}, Java kept {expected:?}"
            );
            // Peak starts sit a few pixels left of Java's barline medians.
            for (&actual, &java) in kept.iter().zip(expected) {
                assert!(
                    (java - actual).abs() <= 8,
                    "staff {staff_id}: peak {actual} does not match Java barline {java}"
                );
            }
        }
        assert_eq!(recognition.peak_graph.retained_peaks, 58);
    }

    #[test]
    fn barline_totals_match_the_java_oracle_on_representative_pages() {
        // Totals from live Java Audiveris 5.11 GRID runs (sum of per-staff
        // barline lists in sheet#1.xml). One page per system shape.
        for (name, expected) in [
            ("chula.png", 58),
            ("hove.png", 17),
            ("D0392410-1.256.png", 53),
        ] {
            let recognition = recognize_grid_lines(repo_path(&format!("data/examples/{name}")))
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(
                recognition.peak_graph.retained_peaks, expected,
                "{name} barline total diverged from Java"
            );
        }
    }

    #[test]
    fn scale_triples_match_the_java_oracle_across_the_corpus() {
        // `<line>` and `<interline>` min/main/max plus beam thickness, read
        // from each page's sheet#1.xml after a live Java 5.11 SCALE run. The
        // percentiles matter as much as the main values: the GRID comb bounds
        // derive from `interline.min` and `interline.max`.
        const JAVA_SCALE: [(&str, [i32; 3], [i32; 3], i32); 9] = [
            ("D0392410-1.256.png", [1, 4, 5], [18, 20, 21], 12),
            ("allegretto.png", [2, 3, 4], [21, 21, 23], 12),
            ("batuque.png", [2, 3, 4], [20, 21, 22], 12),
            ("carmen.png", [2, 3, 4], [20, 21, 23], 12),
            ("chula.png", [2, 3, 4], [20, 21, 22], 12),
            ("cucaracha.png", [2, 3, 4], [20, 21, 23], 12),
            ("hove.png", [2, 3, 5], [19, 20, 22], 11),
            ("zizi.png", [2, 3, 4], [20, 21, 22], 12),
            ("BachInvention5.jpg", [3, 4, 5], [15, 17, 18], 10),
        ];
        for (name, line, interline, beam) in JAVA_SCALE {
            let recognition = recognize_scale(repo_path(&format!("data/examples/{name}")))
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            let scale = &recognition.scale;
            assert_eq!(
                [scale.line.min, scale.line.main, scale.line.max],
                line,
                "{name} line scale diverged"
            );
            assert_eq!(
                [
                    scale.interline.min,
                    scale.interline.main,
                    scale.interline.max
                ],
                interline,
                "{name} interline scale diverged"
            );
            assert_eq!(scale.beam.main, beam, "{name} beam thickness diverged");
        }
    }

    #[test]
    fn missing_file_reports_load_error() {
        let error = recognize_scale(repo_path("data/examples/does-not-exist.png"))
            .expect_err("missing file must fail");
        assert!(matches!(error, ScaleRecognitionError::Load(_)));
    }
}
