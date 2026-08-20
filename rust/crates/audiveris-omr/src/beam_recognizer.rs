// SPDX-License-Identifier: AGPL-3.0-or-later

//! `BeamsBuilder.checkBeamGlyph`: what one spot turns out to be.
//!
//! The beam kernel in `audiveris_image::beam_structure` already computes border
//! lines, items and impacts. What was missing around it is this: the six named
//! refusals Java applies before any of that runs, and the three measurements
//! those refusals test. On chula, 211 of 318 dispatched spots never reach the
//! kernel at all -- a port that skips the gate and lets them through is wrong in
//! a way no comparison of accepted beams would ever show.
//!
//! Everything here is measured against `oracle/beam-structures.txt`, which
//! records Java's verdict and its intermediates for every spot on the page.

use audiveris_core::basic_line::BasicLine;
use audiveris_image::beam_structure::{
    BeamStructureAnalysis, BeamStructureError, Segment, analyze_beam_structure,
};
use audiveris_image::glyph_factory::GlyphComponent;
use audiveris_image::run_table::{Orientation, RunTable};

use crate::beam_parameters::{ItemParameters, SheetParameters};

/// Why `checkBeamGlyph` refused a spot, in Java's own words.
///
/// Kept as Java's strings rather than as a tidier vocabulary: they are what the
/// oracle records, and a rename here would be a rename of the evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeamRejection {
    TooNarrow,
    TooSlim,
    TooSteep,
    Vertical,
    WavyBorders,
    NoTopBorder,
    NoBottomBorder,
    InconsistentBorderPairs,
    UndefinedBorderLine,
    TooNarrowWidth,
    DivergingBeams,
}

impl BeamRejection {
    /// The reason string Java logs and the oracle records.
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Self::TooNarrow => "too narrow",
            Self::TooSlim => "too slim",
            Self::TooSteep => "too steep",
            Self::Vertical => "vertical",
            // Java's computeLines folds every border failure into one
            // verdict; the specific error lives in `detail()` so the oracle
            // string stays byte-identical.
            Self::WavyBorders
            | Self::NoTopBorder
            | Self::NoBottomBorder
            | Self::InconsistentBorderPairs
            | Self::UndefinedBorderLine => "wavy or inconsistent borders",
            Self::TooNarrowWidth => "too narrow width",
            Self::DivergingBeams => "diverging beams",
        }
    }

    /// The specific border failure behind a "wavy or inconsistent borders"
    /// verdict, report-only.
    pub fn detail(self) -> Option<&'static str> {
        match self {
            Self::NoTopBorder => Some("no usable top border"),
            Self::NoBottomBorder => Some("no usable bottom border"),
            Self::InconsistentBorderPairs => Some("inconsistent border pairs"),
            Self::UndefinedBorderLine => Some("undefined border line"),
            _ => None,
        }
    }
}

/// What `checkBeamGlyph` measured on the way to its verdict.
///
/// Reported whether or not the spot was accepted, because the refusals are
/// thresholds on these and a port that refuses the right spots for the wrong
/// reason is one page away from refusing the wrong ones.
#[derive(Clone, Debug, PartialEq)]
pub struct BeamCheck {
    /// `glyph.getMeanThickness(HORIZONTAL)`, once the width gate has passed.
    pub mean_height: Option<f64>,
    /// `computeLines`, the mean distance from border points to their lines.
    pub mean_distance: Option<f64>,
    /// `structure.getWidth()`.
    pub structure_width: Option<f64>,
    /// `structure.compareSlopes()`.
    pub slope_gap: Option<f64>,
    /// The structure, if every gate passed.
    pub structure: Option<BeamStructureAnalysis>,
    pub rejection: Option<BeamRejection>,
    /// Border-line heights before any split, for the thickness estimate: a
    /// stack's line is never thinner than one beam, so the smallest
    /// recurring height cluster across the page is the single-beam
    /// thickness even when the sheet-level histogram was poisoned by fused
    /// stacks.
    pub line_heights: Vec<f64>,
}

/// Java's `BeamGroupInter.getMaxSlopeDiff` default.
pub const MAX_SLOPE_DIFF: f64 = 0.07;

/// `Glyph.getMeanThickness(HORIZONTAL)`: ink over horizontal extent.
#[must_use]
pub fn mean_thickness_horizontal(weight: usize, width: usize) -> f64 {
    weight as f64 / width as f64
}

/// `Glyph.getCenterLine()`: the least-squares fit through every glyph pixel.
///
/// Java accumulates the points in a specific order -- each run walked from its
/// far end back to its start -- and a least-squares fit is not order-free in
/// floating point, so the order is reproduced rather than tidied.
#[must_use]
pub fn center_line(component: &GlyphComponent) -> Option<Segment> {
    let mut line = BasicLine::default();
    let horizontal = component.orientation == Orientation::Horizontal;
    for entry in &component.runs {
        let start = entry.run.start;
        for offset in (0..entry.run.length).rev() {
            if horizontal {
                line.include_point(
                    f64::from(component.left) + (start + offset) as f64,
                    f64::from(component.top) + entry.sequence as f64,
                );
            } else {
                line.include_point(
                    f64::from(component.left) + entry.sequence as f64,
                    f64::from(component.top) + (start + offset) as f64,
                );
            }
        }
    }

    fitted_center_line(&line)
}

/// `Glyph.getCenterLine()` for a fixed glyph backed by a `RunTable`.
///
/// This is the same regression as [`center_line`], with the fixed glyph's
/// table origin applied after following Java's sequence/run/pixel order.
#[must_use]
pub fn run_table_center_line(run_table: &RunTable, left: i32, top: i32) -> Option<Segment> {
    let mut line = BasicLine::default();
    let horizontal = run_table.orientation() == Orientation::Horizontal;
    for sequence in 0..run_table.sequence_count() {
        for run in run_table.sequence(sequence).unwrap_or_default() {
            for coordinate in (run.start..=run.stop()).rev() {
                if horizontal {
                    line.include_point(
                        f64::from(left) + coordinate as f64,
                        f64::from(top) + sequence as f64,
                    );
                } else {
                    line.include_point(
                        f64::from(left) + sequence as f64,
                        f64::from(top) + coordinate as f64,
                    );
                }
            }
        }
    }

    fitted_center_line(&line)
}

fn fitted_center_line(line: &BasicLine) -> Option<Segment> {
    // `BasicLine.toCenterLine`: the fit translated to pixel centres and
    // extended to the pixel limits, which is half a pixel wider than the fit.
    let (x_min, x_max, y_min, y_max) = line.extents()?;
    let coefficients = line.coefficients().ok()?;
    if coefficients.rather_vertical {
        Some(Segment {
            x1: line.x_at_y(y_min).ok()? + 0.5,
            y1: y_min,
            x2: line.x_at_y(y_max).ok()? + 0.5,
            y2: y_max + 1.0,
        })
    } else {
        Some(Segment {
            x1: x_min,
            y1: line.y_at_x(x_min).ok()? + 0.5,
            x2: x_max + 1.0,
            y2: line.y_at_x(x_max).ok()? + 0.5,
        })
    }
}

/// Java `Glyph.getStartPoint`/`getStopPoint`: the uncentered least-squares
/// line through every glyph pixel.
///
/// This deliberately differs from [`center_line`]. `Glyph.getCenterLine()`
/// calls `BasicLine.toCenterLine`, while the endpoint getters cache
/// `BasicLine.toDouble`; HEADS' ledger adapter uses the latter.
#[must_use]
pub fn pixel_line(component: &GlyphComponent) -> Option<Segment> {
    let mut line = BasicLine::default();
    let horizontal = component.orientation == Orientation::Horizontal;
    for entry in &component.runs {
        let start = entry.run.start;
        for offset in (0..entry.run.length).rev() {
            if horizontal {
                line.include_point(
                    f64::from(component.left) + (start + offset) as f64,
                    f64::from(component.top) + entry.sequence as f64,
                );
            } else {
                line.include_point(
                    f64::from(component.left) + entry.sequence as f64,
                    f64::from(component.top) + (start + offset) as f64,
                );
            }
        }
    }

    let (x_min, x_max, y_min, y_max) = line.extents()?;
    let coefficients = line.coefficients().ok()?;
    if coefficients.rather_vertical {
        Some(Segment {
            x1: line.x_at_y(y_min).ok()?,
            y1: y_min,
            x2: line.x_at_y(y_max).ok()?,
            y2: y_max,
        })
    } else {
        Some(Segment {
            x1: x_min,
            y1: line.y_at_x(x_min).ok()?,
            x2: x_max,
            y2: line.y_at_x(x_max).ok()?,
        })
    }
}

/// `BeamStructure.getWidth()`: the span of the outermost line endpoints.
#[must_use]
pub fn structure_width(analysis: &BeamStructureAnalysis) -> f64 {
    let mut left = f64::MAX;
    // Java seeds this with Double.MIN_VALUE, which is the smallest *positive*
    // double rather than the most negative one. Harmless here because abscissae
    // are non-negative, and reproduced so it stays harmless for the same reason
    // Java's is.
    let mut right = f64::MIN_POSITIVE;
    for line in &analysis.lines {
        left = left.min(line.median.x1);
        right = right.max(line.median.x2);
    }
    right - left + 1.0
}

/// `BeamStructure.extendMiddleLines()`: square off a stack of three or more.
///
/// Below three lines Java does nothing, so a two-beam structure keeps whatever
/// abscissae its borders gave it.
pub fn extend_middle_lines(analysis: &mut BeamStructureAnalysis) {
    if analysis.lines.len() < 3 {
        return;
    }
    let mut left = f64::MAX;
    let mut right = f64::MIN_POSITIVE;
    for line in &analysis.lines {
        left = left.min(line.median.x1);
        right = right.max(line.median.x2);
    }
    for line in &mut analysis.lines {
        let (_, y_left) = line.median.at_x(left);
        let (_, y_right) = line.median.at_x(right);
        line.median = Segment {
            x1: left,
            y1: y_left,
            x2: right,
            y2: y_right,
        };
    }
}

/// Runs `checkBeamGlyph` over one spot.
///
/// `raster` is the spot's own tight vertical run table, as the kernel wants it.
#[must_use]
pub fn check_beam_glyph(
    component: &GlyphComponent,
    raster: &RunTable,
    item: &ItemParameters,
    sheet: &SheetParameters,
    projection_source: Option<(&[u8], &[u8], usize, usize)>,
) -> BeamCheck {
    let mut check = BeamCheck {
        mean_height: None,
        mean_distance: None,
        structure_width: None,
        slope_gap: None,
        structure: None,
        rejection: None,
        line_heights: Vec::new(),
    };

    // The bounding box, before any ink is looked at.
    if (component.width as f64) < item.min_beam_width_low {
        check.rejection = Some(BeamRejection::TooNarrow);
        return check;
    }

    let mean_height = mean_thickness_horizontal(component.weight, component.width);
    check.mean_height = Some(mean_height);
    if mean_height < item.min_height_low {
        check.rejection = Some(BeamRejection::TooSlim);
        return check;
    }

    // Java catches an exception from the slope of a vertical fit and calls that
    // its own refusal, distinct from a slope that is merely too steep.
    match center_line(component) {
        Some(line) if line.x2 != line.x1 => {
            if line.slope().abs() > sheet.max_beam_slope {
                check.rejection = Some(BeamRejection::TooSteep);
                return check;
            }
        }
        _ => {
            check.rejection = Some(BeamRejection::Vertical);
            return check;
        }
    }

    let mut analysis =
        match analyze_beam_structure(raster, component.left, component.top, item.structure()) {
            Ok(analysis) => Some(analysis),
            // Java's computeLines returns null for any inconsistent border
            // set, and null and "too far" share one refusal. The distinct
            // reasons here are report-only: which error a fused stack died
            // of is the whole diagnosis (a partially bridged gutter shows up
            // as unequal border counts, not as waviness).
            Err(error) => {
                check.rejection = Some(match error {
                    BeamStructureError::NoUsableTopBorder => BeamRejection::NoTopBorder,
                    BeamStructureError::NoUsableBottomBorder => BeamRejection::NoBottomBorder,
                    BeamStructureError::InconsistentBorderPairs => {
                        BeamRejection::InconsistentBorderPairs
                    }
                    BeamStructureError::UndefinedBorderLine => BeamRejection::UndefinedBorderLine,
                    _ => BeamRejection::WavyBorders,
                });
                None
            }
        };
    if let Some(inner) = &analysis {
        check.mean_distance = Some(inner.global_distance);
        if inner.global_distance > sheet.max_distance_to_border {
            check.rejection = Some(BeamRejection::WavyBorders);
            analysis = None;
        }
    }
    let mut analysis = match analysis {
        Some(analysis) => {
            check.rejection = None;
            analysis
        }
        // The borders failed -- garbled pairing or a residual the
        // straightness gate refuses. The outer ink envelope is still
        // trustworthy (it faces open paper); rebuild from it and let the
        // projection split re-derive the levels. Only under the gate; the
        // recorded rejection stands if the envelope cannot be built either.
        None if crate::beam_veto::create_beam_borders_enabled() => {
            match audiveris_image::beam_structure::envelope_analysis(
                raster,
                component.left,
                component.top,
            ) {
                Some(envelope) => {
                    check.mean_distance = Some(envelope.global_distance);
                    check.rejection = None;
                    envelope
                }
                None => return check,
            }
        }
        None => return check,
    };

    let width = structure_width(&analysis);
    check.structure_width = Some(width);
    if width < item.min_beam_width_low {
        check.rejection = Some(BeamRejection::TooNarrowWidth);
        return check;
    }

    let slope_gap = analysis.max_consecutive_slope_gap(item.max_hook_width);
    check.slope_gap = Some(slope_gap);
    if slope_gap > MAX_SLOPE_DIFF {
        check.rejection = Some(BeamRejection::DivergingBeams);
        // Diverging line slopes are garbled inner borders wearing a
        // different hat; the envelope retry below applies just the same.
        if crate::beam_veto::create_beam_borders_enabled()
            && let Some(envelope) = audiveris_image::beam_structure::envelope_analysis(
                raster,
                component.left,
                component.top,
            )
        {
            check.rejection = None;
            check.slope_gap = Some(0.0);
            analysis = envelope;
        } else {
            return check;
        }
    }

    analysis.adjust_sides(component.left, raster.width(), item.min_beam_width_low);
    extend_middle_lines(&mut analysis);
    check.line_heights = analysis.lines.iter().map(|line| line.height).collect();
    if crate::beam_veto::stacked_beam_split_enabled()
        && let Some((erased, gray, image_width, image_height)) = projection_source
    {
        // Evidence before arithmetic, and gray before binary: the seams
        // between beams survive in the pre-closing grayscale even when
        // binarization has fused the stack solid, and the seam count can
        // neither overcount nor undercount the levels. The run projection
        // handles what the gray profile declines, and the even splits are
        // the last resort.
        analysis.split_lines_by_gray_seams(
            gray,
            image_width,
            image_height,
            item.typical_height,
            crate::beam_veto::stacked_beam_split_limit(),
        );
        analysis.split_lines_by_projection(
            erased,
            image_width,
            image_height,
            audiveris_image::spots::BEAM_BINARIZATION_THRESHOLD,
            item.min_beam_width_low,
            item.typical_height,
            crate::beam_veto::stacked_beam_split_limit(),
        );
    }
    analysis.split_stuck_lines_up_to(
        item.typical_height,
        crate::beam_veto::stacked_beam_split_limit(),
    );
    if crate::beam_veto::stacked_beam_split_enabled() {
        // Java leaves split lines with empty item lists, so stuck stacks
        // create nothing (see `populate_empty_items`), and only ever
        // splits the fully fused single-line case (see
        // `split_thick_lines_up_to` for the partially fused one). Envelope
        // structures may split arithmetically too: their children inherit
        // the synthetic provenance and therefore carry no veto power, so a
        // wrong cut can no longer delete anything -- it just publishes a
        // hypothesis that grading and later joint reasoning judge.
        analysis.split_thick_lines_up_to(
            item.typical_height,
            crate::beam_veto::stacked_beam_split_limit(),
            2.0 * item.min_beam_width_low,
        );
        analysis.add_missing_outer_lines(
            raster,
            component.left,
            component.top,
            item.typical_height,
            crate::beam_veto::stacked_beam_split_limit(),
        );
        // No substantiation pruning here: synthetic lines carry no veto
        // power (RawBeam::synthetic), so keeping every hypothesis costs
        // nothing downstream -- and pruning them against the antialiased
        // pre-closing raster measurably deleted real beams (a 3px beam
        // erodes to ~2.7px there and failed the 0.7x-typical floor).
        analysis.populate_empty_items(
            raster,
            component.left,
            component.top,
            item.structure().max_item_x_gap,
        );
    }
    check.structure = Some(analysis);
    check
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiveris_image::{
        beam_structure::BeamLine,
        glyph_factory::build_glyph_components,
        run_table::{BACKGROUND, FOREGROUND, Orientation},
    };

    fn analysis(lines: Vec<(f64, f64, f64, f64)>) -> BeamStructureAnalysis {
        BeamStructureAnalysis {
            envelope: false,
            synthetic_medians: Vec::new(),
            lines: lines
                .into_iter()
                .map(|(x1, y1, x2, y2)| BeamLine {
                    median: Segment { x1, y1, x2, y2 },
                    height: 12.0,
                    items: Vec::new(),
                })
                .collect(),
            global_distance: 0.0,
            mean_thickness: 12.0,
        }
    }

    #[test]
    fn structure_width_spans_the_outermost_endpoints() {
        // Plus one: Java counts pixels, not the distance between them.
        let structure = analysis(vec![(10.0, 0.0, 40.0, 0.0), (15.0, 9.0, 60.0, 9.0)]);
        assert_eq!(structure_width(&structure), 51.0);
    }

    #[test]
    fn glyph_endpoint_line_is_not_the_center_line() {
        let pixels = [
            FOREGROUND, FOREGROUND, FOREGROUND, BACKGROUND, BACKGROUND, FOREGROUND, FOREGROUND,
            FOREGROUND,
        ];
        let table = RunTable::from_pixels(Orientation::Horizontal, 4, 2, &pixels).unwrap();
        let components = build_glyph_components(&table, 10, 20);
        let component = &components[0];
        let endpoints = pixel_line(component).unwrap();
        let centered = center_line(component).unwrap();

        assert_eq!(endpoints.x1, centered.x1);
        assert_eq!(endpoints.x2 + 1.0, centered.x2);
        assert_eq!(endpoints.y1 + 0.5, centered.y1);
        assert_eq!(endpoints.y2 + 0.5, centered.y2);
    }

    #[test]
    fn middle_lines_extend_only_from_three_up() {
        // Two lines are left alone, which is Java's guard and not an oversight:
        // a two-beam structure keeps whatever abscissae its borders gave it.
        let mut two = analysis(vec![(10.0, 0.0, 40.0, 0.0), (15.0, 9.0, 60.0, 9.0)]);
        let before = two.lines.clone();
        extend_middle_lines(&mut two);
        assert_eq!(two.lines, before);

        let mut three = analysis(vec![
            (10.0, 0.0, 40.0, 0.0),
            (15.0, 9.0, 60.0, 9.0),
            (20.0, 18.0, 50.0, 18.0),
        ]);
        extend_middle_lines(&mut three);
        for line in &three.lines {
            assert_eq!((line.median.x1, line.median.x2), (10.0, 60.0));
        }
    }

    #[test]
    fn extending_keeps_each_line_on_its_own_slope() {
        // The lines are squared off, not levelled: a sloping middle line has to
        // stay sloping, or every beam in a stack comes out parallel to the top.
        let mut sloped = analysis(vec![
            (10.0, 0.0, 40.0, 3.0),
            (15.0, 9.0, 35.0, 11.0),
            (20.0, 18.0, 50.0, 21.0),
        ]);
        extend_middle_lines(&mut sloped);
        let middle = sloped.lines[1].median;
        assert!((middle.slope() - 0.1).abs() < 1e-12, "{middle:?}");
        assert!((middle.y1 - 8.5).abs() < 1e-12, "{middle:?}");
    }
}
