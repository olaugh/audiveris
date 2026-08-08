// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production GRID parameters derived from a sheet [`ScaleEstimate`].
//!
//! Java resolves every GRID threshold from `Scale` inside each stage's
//! `Parameters` constructor. This module reproduces those derivations exactly
//! so the raster-driven grid path can run on a real page. Formula sources are
//! cited per field against the frozen Java baseline
//! (`9e1e55cd2746037d059345881c53e6a6754bffbd`).
//!
//! Rounding: every Java `Scale.toPixels` is `(int) Math.rint(..)`, i.e.
//! round-half-to-EVEN, reproduced here by [`f64::round_ties_even`]. The only
//! deviations on this path are `LinesRetriever.maxStickerExtension`
//! (`Math.ceil`) and `LagManager.minVerticalRunLength` (`1 + rint(..)`),
//! both handled below.
//!
//! Several parameters derive from staff-line thickness (`lineScale`), not
//! interline: sticker thickness/extension/gap, `FilamentFactory.maxThickness`
//! and `maxPosGap`, and the vertical-run partition. `ClustersRetriever` comb
//! bounds use the interline percentile `min`/`max`, not `main`.
//!
//! Every `global_slope` handed in here is a construction-time placeholder:
//! the raster executor path measures the real slope and routes it through
//! `RawLineMetadataHandoff`, exactly as Java re-measures after raw lines.

use std::collections::BTreeSet;

use crate::bars_coordinator::{BarsCoordinatorParameters, CClefParameters};
use crate::cluster_expand::ClusterExpansionParameters;
use crate::cluster_finalize::ClusterConsistencyParameters;
use crate::cluster_merge::{ClusterMergeParameters, ClusterMergePassParameters};
use crate::cluster_pipeline::ClusterRetrievalParameters;
use crate::crossing_completion::InspectCrossingChunksParameters;
use crate::discarded_completion::IncludeDiscardedFilamentsParameters;
use crate::filament_factory::{FilamentFactoryParams, OverlapParams};
use crate::line_endpoints::DefineEndPointsParameters;
use crate::lines_coordinator::LinesCoordinatorParameters;
use crate::prepared_completion::ProductionLineCompletionParameters;
use crate::raster_grid_builder::RasterGridParameters;
use crate::raw_line_adapter::RawPrimaryPassParameters;
use crate::scale_estimate::ScaleEstimate;
use crate::section_completion::IncludeSectionsParameters;
use crate::sticker_completion::IncludeStickersParameters;

/// Complete scale-derived inputs for a production raster-driven GRID run.
#[derive(Debug, Clone)]
pub struct ProductionGridParameters {
    pub raster: RasterGridParameters,
    pub raw_primary: RawPrimaryPassParameters,
    pub lines: LinesCoordinatorParameters,
    pub completion: ProductionLineCompletionParameters,
    /// Java `LinesRetriever.maxThinStickerWeight` = `rint(0.06 * I * I)`.
    pub maximum_thin_weight: usize,
    /// Java `LinesRetriever.constants.purgeCrossingChunks` default (false).
    pub inspect_crossing_chunks: bool,
    pub bars: BarsCoordinatorParameters,
    pub braces: ProductionBraceParameters,
    /// Java `BarsRetriever.maxDoubleBarGap` = `rint(0.75 * I)`, consumed by
    /// `peak_groups` as the inclusive double-bar grouping gap.
    pub maximum_group_gap: i32,
}

/// Scale-resolved constants consumed between Java's bar prefix and its
/// post-brace purges.  Projection thresholds remain staff-specific and live
/// with each `StaffProjectorProcessOutput`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProductionBraceParameters {
    pub neutral_gap: i32,
    pub maximum_peak_width: i32,
    pub maximum_bar_gap: i32,
    pub lookup_extension: i32,
    pub minimum_portion_height: i32,
    pub maximum_section_width: i32,
    pub maximum_curvature: i32,
    pub maximum_alignment_dx: i32,
}

/// A stage constructor rejected the derived values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionGridParametersError {
    pub stage: &'static str,
    pub message: String,
}

impl std::fmt::Display for ProductionGridParametersError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid derived {} parameters: {}",
            self.stage, self.message
        )
    }
}

impl std::error::Error for ProductionGridParametersError {}

fn stage_error<E: std::fmt::Debug>(
    stage: &'static str,
) -> impl FnOnce(E) -> ProductionGridParametersError {
    move |error| ProductionGridParametersError {
        stage,
        message: format!("{error:?}"),
    }
}

/// Java `(int) Math.rint(value)`: round half to even.
fn rint(value: f64) -> i32 {
    let rounded = value.round_ties_even();
    // The GRID derivations only produce small positive magnitudes.
    rounded as i32
}

fn rint_usize(value: f64) -> usize {
    rint(value).max(0) as usize
}

/// Derives the full production GRID parameter set from a sheet scale.
///
/// `global_slope` seeds every slope-carrying parameter and is overwritten at
/// run time by the measured raw-line slope through the raster handoffs; pass
/// `0.0` unless replaying a known sheet.
pub fn production_grid_parameters(
    scale: &ScaleEstimate,
    global_slope: f64,
) -> Result<ProductionGridParameters, ProductionGridParametersError> {
    let interline = f64::from(scale.interline.main);
    let interline_px = scale.interline.main;
    let fore = f64::from(scale.line.main);
    let max_fore = scale.line.max;

    // LagManager.dispatchRuns: minVerticalRunLength = 1 + rint(1.2 * maxFore),
    // encoded by the run-table splitter as `1 + rint(max_fore * ledger)`.
    // LinesRetriever.minRunLength = rint(0.25 * I);
    // LagManager.maxVerticalRunShift = rint(0.05 * I).
    let raster = RasterGridParameters {
        max_fore: max_fore.max(0) as usize,
        ledger_thickness: 1.2,
        minimum_horizontal_run_length: rint_usize(0.25 * interline),
        maximum_vertical_run_shift: rint_usize(0.05 * interline),
    };

    // FilamentFactory.Parameters.initialize(), LinesRetriever defaults.
    let factory = FilamentFactoryParams {
        interline: interline_px.max(0) as usize,
        // minCoreSectionLength = rint(0.5 * I)
        min_core_section_length: rint_usize(0.5 * interline),
        // minSectionAspect = 3 (ratio)
        min_section_aspect: 3.0,
        // maxCoordGap = rint(1.7 * I)
        max_coord_gap: f64::from(rint(1.7 * interline)),
        // maxPosGap = rint(0.75 * FORE) - line-thickness based
        max_pos_gap: f64::from(rint(0.75 * fore)),
        // maxPosGapForSlope = rint(0.1 * I)
        max_pos_gap_for_slope: f64::from(rint(0.1 * interline)),
        // maxGapSlope = 0.5 (tangent)
        max_gap_slope: 0.5,
        // minLengthForDeltaSlope = rint(10 * I)
        min_length_for_delta_slope: f64::from(rint(10.0 * interline)),
        // maxDeltaSlope = 0.01 (ratio)
        max_delta_slope: 0.01,
    };
    let overlap = OverlapParams {
        // Filament.probeWidth = rint(0.5 * I)
        probe_width: rint_usize(0.5 * interline),
        // maxOverlapDeltaPos = rint(0.25 * I)
        max_overlap_delta_pos: f64::from(rint(0.25 * interline)),
        // maxFilamentThickness = rint(1.5 * FORE) - line-thickness based
        max_thickness: f64::from(rint(1.5 * fore)),
        // maxOverlapSpace = rint(0.16 * I)
        max_overlap_space: f64::from(rint(0.16 * interline)),
        // maxExpansionSpace = rint(0.02 * I)
        max_expansion_space: f64::from(rint(0.02 * interline)),
        // maxInvolvingLength = rint(2 * I)
        max_involving_length: f64::from(rint(2.0 * interline)),
        // maxConsistentRatio = 1.7 (ratio)
        max_consistent_ratio: 1.7,
    };

    // ClustersRetriever pass-1 values: X-axis from the large interline,
    // Y-axis from the pass interline (same object in pass 1).
    let max_merge_dx = i64::from(rint(6.0 * interline));
    let cluster_y_margin = i64::from(rint(2.0 * interline));
    let expansion = ClusterExpansionParameters::new(
        global_slope,
        // maxMergeDx = rint(6 * I): clusterBox.grow(maxMergeDx, yMargin)
        max_merge_dx,
        // clusterYMargin = rint(2 * I)
        cluster_y_margin,
        // maxExpandDx = rint(2 * I): getPointsAt lookup
        rint_usize(2.0 * interline),
        // maxExpandDy = rint(0.175 * I)
        f64::from(rint(0.175 * interline)),
        // Filament probe width = rint(0.5 * I)
        rint_usize(0.5 * interline),
        // LineCluster thickness cut: scale.getMaxFore()
        max_fore.max(0) as usize,
    )
    .map_err(stage_error("cluster expansion"))?;
    let compatibility = ClusterMergeParameters::new(
        global_slope,
        // maxMergeDx = rint(6 * I)
        max_merge_dx,
        // maxMergeDy = rint(0.4 * I)
        f64::from(rint(0.4 * interline)),
        // maxExtrapolationDx = rint(6 * I)
        rint_usize(6.0 * interline),
        rint_usize(0.5 * interline),
        max_fore.max(0) as usize,
    )
    .map_err(stage_error("cluster merge"))?;
    let merge_pass = ClusterMergePassParameters::new(
        compatibility,
        // maxMergeDxSingle = rint(10 * I)
        i64::from(rint(10.0 * interline)),
        cluster_y_margin,
    )
    .map_err(stage_error("cluster merge pass"))?;
    let consistency = ClusterConsistencyParameters::new(
        // maxClusterDiffLengthRatio = 0.5 (ratio)
        0.5,
    )
    .map_err(stage_error("cluster consistency"))?;
    let retrieval = ClusterRetrievalParameters::new(
        interline_px.max(0) as usize,
        // Standard five-line staves; one-line/tablature staves are a later,
        // stub-configured concern.
        BTreeSet::from([5]),
        expansion,
        merge_pass,
        global_slope,
        // minClusterTablatureLengthRatio = 0.5, minClusterLengthRatio = 0.2
        0.5,
        0.2,
        // maxMergeCenterDy = rint(1.0 * I)
        i64::from(rint(1.0 * interline)),
        max_merge_dx,
        Some(consistency),
        cluster_y_margin,
    )
    .map_err(stage_error("cluster retrieval"))?;

    let raw_primary = RawPrimaryPassParameters {
        factory,
        overlap,
        // samplingDx = rint(1 * I)
        sampling_dx: rint_usize(interline),
        // Comb bounds use the interline percentile bounds with the comb
        // margins: dMin = interline.min - rint(0.05 * I),
        // dMax = interline.max + rint(0.05 * I).
        minimum_delta_y: (scale.interline.min - rint(0.05 * interline)) as isize,
        maximum_delta_y: (scale.interline.max + rint(0.05 * interline)) as isize,
        retrieval,
    };

    let lines = LinesCoordinatorParameters::new(
        global_slope,
        // minStaffLength = rint(30 * I)
        rint_usize(30.0 * interline),
        // Scale.getFore()
        Some(scale.line.main.max(0) as usize),
        // maxOneLineSlope = 0.001 (tangent)
        0.001,
        // minTrueLengthRatio = 0.3 (ratio)
        0.3,
        // maxRightIndentation = rint(5 * I)
        f64::from(rint(5.0 * interline)),
    )
    .map_err(stage_error("lines coordinator"))?;

    // Sticker geometry: thickness from lineScale.max, gap/extension from
    // lineScale.main.
    let sticker_thickness = f64::from(rint(1.0 * f64::from(max_fore)));
    let sticker_gap = 0.25 * fore; // toPixelsDouble, NOT rounded
    let sticker_extension = (1.2 * fore).ceil() as i32; // Math.ceil
    let completion = ProductionLineCompletionParameters {
        define_end_points: DefineEndPointsParameters {
            scale_interline: interline_px,
            foreground_thickness: scale.line.main,
            // maxEndingDx = rint(1.0 * I)
            maximum_ending_dx: rint(1.0 * interline),
            // patternWidth = rint(1.0 * I)
            pattern_width: rint(1.0 * interline),
            // patternJitter = rint(0.25 * I)
            pattern_jitter: rint(0.25 * interline),
        },
        include_discarded_filaments: IncludeDiscardedFilamentsParameters {
            scale_interline: interline_px,
            foreground_thickness: scale.line.main,
            maximum_sticker_thickness: sticker_thickness,
            maximum_sticker_gap: sticker_gap,
            maximum_sticker_extension: sticker_extension,
        },
        include_sections: IncludeSectionsParameters {
            scale_interline: interline_px,
            foreground_thickness: scale.line.main,
            maximum_sticker_thickness: sticker_thickness,
            maximum_sticker_gap: sticker_gap,
            maximum_sticker_extension: sticker_extension,
        },
        // minRadius = rint(12 * I)
        minimum_curvature_radius: rint(12.0 * interline),
        include_stickers: IncludeStickersParameters {
            // maxStickerConnectionLength = rint(0.05 * I)
            maximum_connection_length: rint_usize(0.05 * interline),
        },
        inspect_crossing_chunks: InspectCrossingChunksParameters {
            // minCrossingOffset = rint(10.0 * I)
            minimum_offset: rint_usize(10.0 * interline),
            global_slope,
            // minChunkSlope = 0.04 (tangent)
            minimum_chunk_slope: 0.04,
        },
    };

    let maximum_double_bar_gap = rint(0.75 * interline);
    let bars = BarsCoordinatorParameters::new(
        // maxColumnDx = rint(0.75 * I)
        rint(0.75 * interline),
        // maxBraceBarGap = rint(2.0 * I)
        rint(2.0 * interline),
        maximum_double_bar_gap,
        // maxLinesLeftToStartBar = rint(0.15 * I)
        rint(0.15 * interline),
        scale.line.main,
        interline_px,
        // minThinThickDelta = 0.2, used raw (normed), never converted
        0.2,
        Some(CClefParameters {
            // minPeak1WidthForCClef = rint(0.3 * I)
            minimum_first_peak_width: rint(0.3 * interline),
            // maxPeak2WidthForCClef = rint(0.3 * I)
            maximum_second_peak_width: rint(0.3 * interline),
            // minMeasureWidth = rint(2.0 * I)
            minimum_measure_width: rint(2.0 * interline),
            // cClefTail = rint(2.0 * I)
            tail_width: rint(2.0 * interline),
        }),
    )
    .map_err(stage_error("bars coordinator"))?;
    let braces = ProductionBraceParameters {
        // BarsRetriever.braceBarNeutralGap = rint(0.1 * I)
        neutral_gap: rint(0.1 * interline),
        // BarsRetriever.maxBracePeakWidth = rint(3.0 * I)
        maximum_peak_width: rint(3.0 * interline),
        // BarsRetriever.maxBraceBarGap = rint(2.0 * I)
        maximum_bar_gap: rint(2.0 * interline),
        // BarsRetriever.braceLookupExtension = rint(0.5 * I)
        lookup_extension: rint(0.5 * interline),
        // BarsRetriever.minBracePortionHeight = rint(3.0 * I)
        minimum_portion_height: rint(3.0 * interline),
        // BarsRetriever.maxBraceThickness = rint(1.0 * I)
        maximum_section_width: rint(1.0 * interline),
        // BarsRetriever.maxBraceCurvature = rint(25.0 * I)
        maximum_curvature: rint(25.0 * interline),
        // PeakGraph.maxAlignmentBraceDx = rint(0.75 * I)
        maximum_alignment_dx: rint(0.75 * interline),
    };

    Ok(ProductionGridParameters {
        raster,
        raw_primary,
        lines,
        completion,
        // maxThinStickerWeight = rint(0.06 * I * I) (area fraction)
        maximum_thin_weight: rint_usize(0.06 * interline * interline),
        // purgeCrossingChunks defaults to false in Java
        inspect_crossing_chunks: false,
        bars,
        braces,
        maximum_group_gap: maximum_double_bar_gap,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adaptive;
    use crate::ingest;
    use crate::run_table::{Orientation, RunTable};
    use crate::scale_estimate::{ScaleOptions, estimate_scale};
    use crate::scale_runs::vertical_run_histograms;

    fn chula_scale() -> ScaleEstimate {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../data/examples/chula.png");
        let loaded = ingest::load_max_channel_gray(path).expect("chula loads");
        let (width, height) = (loaded.width(), loaded.height());
        let binary = adaptive::default_adaptive_filter(width, height, loaded.pixels());
        let vertical =
            RunTable::from_pixels(Orientation::Vertical, width, height, &binary).expect("runs");
        estimate_scale(
            &vertical_run_histograms(&vertical),
            ScaleOptions {
                image_size: Some((width, height)),
                ..ScaleOptions::default()
            },
        )
        .expect("scale")
    }

    #[test]
    fn derives_java_values_for_chula() {
        // Chula frozen oracle: interline 20/21/22, line 2/3/4.
        let scale = chula_scale();
        assert_eq!(
            (
                scale.interline.min,
                scale.interline.main,
                scale.interline.max
            ),
            (20, 21, 22)
        );
        assert_eq!((scale.line.min, scale.line.main, scale.line.max), (2, 3, 4));

        let parameters = production_grid_parameters(&scale, 0.0).expect("derivation");

        // LagManager/LinesRetriever partition: maxFore=4, ledger 1.2,
        // minRunLength = rint(5.25) = 5, maxShift = rint(1.05) = 1.
        assert_eq!(parameters.raster.max_fore, 4);
        assert_eq!(parameters.raster.ledger_thickness, 1.2);
        assert_eq!(parameters.raster.minimum_horizontal_run_length, 5);
        assert_eq!(parameters.raster.maximum_vertical_run_shift, 1);

        // FilamentFactory: banker's rounding makes rint(10.5) = 10.
        let factory = &parameters.raw_primary.factory;
        assert_eq!(factory.interline, 21);
        assert_eq!(factory.min_core_section_length, 10);
        assert_eq!(factory.max_coord_gap, 36.0); // rint(35.7)
        assert_eq!(factory.max_pos_gap, 2.0); // rint(0.75 * 3) = rint(2.25)
        assert_eq!(factory.max_pos_gap_for_slope, 2.0); // rint(2.1)
        assert_eq!(factory.min_length_for_delta_slope, 210.0);

        let overlap = &parameters.raw_primary.overlap;
        assert_eq!(overlap.probe_width, 10); // rint(10.5) ties-to-even
        assert_eq!(overlap.max_overlap_delta_pos, 5.0); // rint(5.25)
        assert_eq!(overlap.max_thickness, 4.0); // rint(4.5) ties-to-even!
        assert_eq!(overlap.max_overlap_space, 3.0); // rint(3.36)
        assert_eq!(overlap.max_expansion_space, 0.0); // rint(0.42)
        assert_eq!(overlap.max_involving_length, 42.0);

        // Comb bounds from percentile interline bounds with rint(1.05)=1.
        assert_eq!(parameters.raw_primary.sampling_dx, 21);
        assert_eq!(parameters.raw_primary.minimum_delta_y, 19);
        assert_eq!(parameters.raw_primary.maximum_delta_y, 23);

        // Completion: jitter rint(5.25)=5, radius 252, sticker gap 0.75
        // (unrounded double), extension ceil(3.6)=4, thin weight rint(26.46).
        let endpoints = &parameters.completion.define_end_points;
        assert_eq!(endpoints.pattern_jitter, 5);
        assert_eq!(endpoints.maximum_ending_dx, 21);
        assert_eq!(parameters.completion.minimum_curvature_radius, 252);
        let discarded = &parameters.completion.include_discarded_filaments;
        assert_eq!(discarded.maximum_sticker_thickness, 4.0);
        assert_eq!(discarded.maximum_sticker_gap, 0.75);
        assert_eq!(discarded.maximum_sticker_extension, 4);
        assert_eq!(
            parameters
                .completion
                .include_stickers
                .maximum_connection_length,
            1
        );
        assert_eq!(
            parameters.completion.inspect_crossing_chunks.minimum_offset,
            210
        );
        assert_eq!(parameters.maximum_thin_weight, 26);
        assert!(!parameters.inspect_crossing_chunks);

        // Bars: maxColumnDx = rint(15.75) = 16, group gap = maxDoubleBarGap.
        assert_eq!(parameters.maximum_group_gap, 16);
        assert_eq!(
            parameters.braces,
            ProductionBraceParameters {
                neutral_gap: 2,
                maximum_peak_width: 63,
                maximum_bar_gap: 42,
                lookup_extension: 10,
                minimum_portion_height: 63,
                maximum_section_width: 21,
                maximum_curvature: 525,
                maximum_alignment_dx: 16,
            }
        );
    }

    #[test]
    fn ties_to_even_rounding_matches_java_rint() {
        assert_eq!(rint(0.5), 0);
        assert_eq!(rint(1.5), 2);
        assert_eq!(rint(2.5), 2);
        assert_eq!(rint(10.5), 10);
        assert_eq!(rint(4.5), 4);
        assert_eq!(rint(5.25), 5);
    }
}
