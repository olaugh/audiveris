// SPDX-License-Identifier: AGPL-3.0-or-later

//! Raster-only port of the geometry kernel behind Java `BeamStructure` and
//! `BeamsBuilder.computeImpacts`.
//!
//! Interpretation grading and SIG materialization deliberately remain outside
//! this module. The input is the same vertical run-table geometry used by the
//! Java beam-spot path; the output is deterministic border, item, and raster
//! evidence that a caller can classify or materialize independently.

use std::{
    cmp::{Ordering, Reverse},
    error::Error,
    fmt,
};

use audiveris_core::java_math::java_positive_pow;

use crate::{
    run_table::{FOREGROUND, Orientation, RunTable},
    section::{JunctionPolicy, Section, build_sections},
};

const MAX_SECTION_SLOPE_GAP: f64 = 0.3;
const MAX_BORDER_JITTER_RATIO: f64 = 0.8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl Segment {
    #[must_use]
    pub fn slope(self) -> f64 {
        (self.y2 - self.y1) / (self.x2 - self.x1)
    }

    #[must_use]
    pub fn y_at_x(self, x: f64) -> f64 {
        line_util_y_at_x(self.x1, self.y1, self.x2, self.y2, x)
    }

    #[must_use]
    pub fn at_x(self, x: f64) -> (f64, f64) {
        (x, self.y_at_x(x))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamItem {
    pub median: Segment,
    pub height: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeamLine {
    pub median: Segment,
    pub height: f64,
    pub items: Vec<BeamItem>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamStructureParameters {
    pub typical_height: f64,
    pub core_section_width: usize,
    pub min_hook_width_low: f64,
    pub max_item_x_gap: i32,
    pub min_beam_width_low: f64,
    pub max_hook_width: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeamStructureAnalysis {
    pub lines: Vec<BeamLine>,
    pub global_distance: f64,
    pub mean_thickness: f64,
}

impl BeamStructureAnalysis {
    /// Java `BeamStructure.splitLines`: split only the single-line/two-beam
    /// case selected by `Math.rint` (ties to even). Newly split lines have no
    /// items, just as in Java; downstream item retrieval/materialization owns
    /// any subsequent interpretation work.
    pub fn split_stuck_lines(&mut self, typical_height: f64) {
        let target_count = (self.mean_thickness / typical_height).round_ties_even() as usize;
        if self.lines.len() > 1 || target_count <= self.lines.len() {
            return;
        }
        let line = &self.lines[0];
        let gutter = line.height - (2.0 * typical_height);
        if gutter < 0.0 {
            return;
        }
        let new_height = (line.height - gutter) / 2.0;
        let dy = (new_height + gutter) / 2.0;
        let median = line.median;
        self.lines = vec![
            BeamLine {
                median: Segment {
                    y1: median.y1 - dy,
                    y2: median.y2 - dy,
                    ..median
                },
                height: new_height,
                items: Vec::new(),
            },
            BeamLine {
                median: Segment {
                    y1: median.y1 + dy,
                    y2: median.y2 + dy,
                    ..median
                },
                height: new_height,
                items: Vec::new(),
            },
        ];
    }

    /// Java `BeamStructure.adjustSides` over the tight run-table bounds.
    pub fn adjust_sides(&mut self, glyph_left: i32, glyph_width: usize, min_beam_width_low: f64) {
        let left = f64::from(glyph_left);
        let right = left + glyph_width as f64 - 1.0;
        for line in &mut self.lines {
            if line.median.x1 - left < min_beam_width_low {
                let (_, y1) = line.median.at_x(left);
                line.median.x1 = left;
                line.median.y1 = y1;
            }
            if right - line.median.x2 < min_beam_width_low {
                let (_, y2) = line.median.at_x(right);
                line.median.x2 = right;
                line.median.y2 = y2;
            }
        }
    }

    #[must_use]
    pub fn max_consecutive_slope_gap(&self, max_hook_width: f64) -> f64 {
        let mut previous = None;
        let mut maximum = 0.0_f64;
        for line in &self.lines {
            if line.median.x2 - line.median.x1 > max_hook_width {
                let slope = line.median.slope();
                if let Some(previous) = previous {
                    maximum = maximum.max(f64::abs(slope - previous));
                }
                previous = Some(slope);
            }
        }
        maximum
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeamStructureError {
    NonVerticalRunTable,
    NoUsableTopBorder,
    NoUsableBottomBorder,
    InconsistentBorderPairs,
    UndefinedBorderLine,
}

impl fmt::Display for BeamStructureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for BeamStructureError {}

/// Retrieve border lines and beam items from one tight beam-spot run table.
pub fn analyze_beam_structure(
    glyph: &RunTable,
    offset_x: i32,
    offset_y: i32,
    parameters: BeamStructureParameters,
) -> Result<BeamStructureAnalysis, BeamStructureError> {
    if glyph.orientation() != Orientation::Vertical {
        return Err(BeamStructureError::NonVerticalRunTable);
    }
    let sections = build_sections(glyph, JunctionPolicy::DEFAULT_RATIO);
    let center = glyph_centroid(glyph, offset_x, offset_y);
    let top = border_lines(
        &sections,
        offset_x,
        offset_y,
        center,
        parameters,
        BorderSide::Top,
    )?;
    let bottom = border_lines(
        &sections,
        offset_x,
        offset_y,
        center,
        parameters,
        BorderSide::Bottom,
    )?;
    let global_distance = top
        .iter()
        .chain(&bottom)
        .map(|line| line.mean_distance() * line.count)
        .sum::<f64>()
        / top
            .iter()
            .chain(&bottom)
            .map(|line| line.count)
            .sum::<f64>();
    let global_slope = top
        .iter()
        .chain(&bottom)
        .max_by(|one, two| {
            one.span()
                .partial_cmp(&two.span())
                .unwrap_or(Ordering::Equal)
        })
        .ok_or(BeamStructureError::UndefinedBorderLine)?
        .slope();
    let mut top = line_map(top, center, global_slope)?;
    let mut bottom = line_map(bottom, center, global_slope)?;
    complete_border_lines(
        1.0,
        center,
        global_slope,
        parameters.typical_height,
        &top,
        &mut bottom,
    );
    complete_border_lines(
        -1.0,
        center,
        global_slope,
        parameters.typical_height,
        &bottom,
        &mut top,
    );
    top.sort_by(|one, two| one.0.total_cmp(&two.0));
    bottom.sort_by(|one, two| one.0.total_cmp(&two.0));
    if top.len() != bottom.len() {
        return Err(BeamStructureError::InconsistentBorderPairs);
    }

    let mut lines = Vec::with_capacity(top.len());
    for ((_, top), (_, bottom)) in top.into_iter().zip(bottom) {
        let x1 = top.x1.min(bottom.x1);
        let x2 = top.x2.max(bottom.x2);
        let yt1 = top.y_at_x(x1);
        let yb1 = bottom.y_at_x(x1);
        let yt2 = top.y_at_x(x2);
        let yb2 = bottom.y_at_x(x2);
        let height = ((yb1 - yt1) + (yb2 - yt2)) / 2.0;
        let median = Segment {
            x1,
            y1: (yt1 + yb1) / 2.0,
            x2,
            y2: (yt2 + yb2) / 2.0,
        };
        let items = retrieve_items(
            &sections,
            offset_x,
            offset_y,
            median,
            height,
            parameters.max_item_x_gap,
        );
        lines.push(BeamLine {
            median,
            height,
            items,
        });
    }
    Ok(BeamStructureAnalysis {
        lines,
        global_distance,
        mean_thickness: glyph.weight() as f64 / glyph.width() as f64,
    })
}

#[derive(Clone, Copy)]
enum BorderSide {
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug)]
struct BasicLine {
    count: f64,
    sum_x: f64,
    sum_y: f64,
    sum_x2: f64,
    sum_y2: f64,
    sum_xy: f64,
    min_x: f64,
    max_x: f64,
}

impl BasicLine {
    fn empty() -> Self {
        Self {
            count: 0.0,
            sum_x: 0.0,
            sum_y: 0.0,
            sum_x2: 0.0,
            sum_y2: 0.0,
            sum_xy: 0.0,
            min_x: f64::MAX,
            max_x: f64::MIN,
        }
    }

    fn include(&mut self, x: f64, y: f64) {
        self.count += 1.0;
        self.sum_x += x;
        self.sum_y += y;
        self.sum_x2 += x * x;
        self.sum_y2 += y * y;
        self.sum_xy += x * y;
        self.min_x = self.min_x.min(x);
        self.max_x = self.max_x.max(x);
    }

    fn include_line(&mut self, other: Self) {
        self.count += other.count;
        self.sum_x += other.sum_x;
        self.sum_y += other.sum_y;
        self.sum_x2 += other.sum_x2;
        self.sum_y2 += other.sum_y2;
        self.sum_xy += other.sum_xy;
        self.min_x = self.min_x.min(other.min_x);
        self.max_x = self.max_x.max(other.max_x);
    }

    fn coefficients(self) -> Option<(f64, f64, f64)> {
        if self.count < 2.0 {
            return None;
        }
        let horizontal = (self.count * self.sum_x2) - (self.sum_x * self.sum_x);
        let vertical = (self.count * self.sum_y2) - (self.sum_y * self.sum_y);
        let (mut a, mut b, mut c) = if horizontal.abs() >= vertical.abs() {
            (
                ((self.count * self.sum_xy) - (self.sum_x * self.sum_y)) / horizontal,
                -1.0,
                ((self.sum_y * self.sum_x2) - (self.sum_x * self.sum_xy)) / horizontal,
            )
        } else {
            (
                -1.0,
                ((self.count * self.sum_xy) - (self.sum_x * self.sum_y)) / vertical,
                ((self.sum_x * self.sum_y2) - (self.sum_y * self.sum_xy)) / vertical,
            )
        };
        // `java_hypot`, not `f64::hypot`: this is a second copy of
        // `BasicLine.normalize`, and the platform libm's answer differs from
        // Java's fdlibm one by an ulp. `mean_distance` below subtracts
        // near-equal terms, which amplifies that ulp to about 1e-9 -- enough to
        // move the ninth decimal, and enough to differ between macOS and Linux.
        // CI caught this on its ubuntu leg while every local run was green.
        let norm = audiveris_core::basic_line::java_hypot(a, b);
        a /= norm;
        b /= norm;
        c /= norm;
        (a.is_finite() && b.is_finite() && c.is_finite()).then_some((a, b, c))
    }

    fn slope(self) -> f64 {
        let (a, b, _) = self.coefficients().expect("usable border line");
        -a / b
    }

    fn y_at_x(self, x: f64) -> f64 {
        let (a, b, c) = self.coefficients().expect("usable border line");
        ((-a * x) - c) / b
    }

    fn segment(self) -> Option<Segment> {
        self.coefficients()?;
        Some(Segment {
            x1: self.min_x,
            y1: self.y_at_x(self.min_x),
            x2: self.max_x,
            y2: self.y_at_x(self.max_x),
        })
    }

    fn span(self) -> f64 {
        self.max_x - self.min_x + 1.0
    }

    fn mean_distance(self) -> f64 {
        let (a, b, c) = self.coefficients().expect("usable border line");
        let mut squared = ((a * a * self.sum_x2)
            + (b * b * self.sum_y2)
            + (c * c * self.count)
            + (2.0 * a * b * self.sum_xy)
            + (2.0 * a * c * self.sum_x)
            + (2.0 * b * c * self.sum_y))
            / self.count;
        if squared < 0.0 {
            squared = 0.0;
        }
        squared.sqrt()
    }
}

fn border_lines(
    sections: &[Section],
    offset_x: i32,
    offset_y: i32,
    center: (f64, f64),
    parameters: BeamStructureParameters,
    side: BorderSide,
) -> Result<Vec<BasicLine>, BeamStructureError> {
    let mut borders = Vec::<(usize, BasicLine, f64)>::new();
    for section in sections {
        if section.bounds().width < parameters.core_section_width {
            continue;
        }
        let mut line = BasicLine::empty();
        for (index, run) in section.runs().iter().enumerate() {
            let x = f64::from(offset_x) + (section.first_pos() + index) as f64;
            let local_y = match side {
                BorderSide::Top => run.start,
                BorderSide::Bottom => run.stop() + 1,
            };
            line.include(x, f64::from(offset_y) + local_y as f64);
        }
        if line.coefficients().is_some() {
            borders.push((section.run_count(), line, 0.0));
        }
    }
    if borders.is_empty() {
        return Err(match side {
            BorderSide::Top => BeamStructureError::NoUsableTopBorder,
            BorderSide::Bottom => BeamStructureError::NoUsableBottomBorder,
        });
    }
    borders.sort_by_key(|entry| Reverse(entry.0));
    let mut weighted_slope = 0.0;
    let mut points = 0.0;
    for (_, line, _) in &borders {
        let slope = line.slope();
        if points != 0.0 && (slope - (weighted_slope / points)).abs() > MAX_SECTION_SLOPE_GAP {
            break;
        }
        points += line.count;
        weighted_slope += line.count * slope;
    }
    let global_slope = weighted_slope / points;
    borders.retain(|(_, line, _)| (line.slope() - global_slope).abs() <= MAX_SECTION_SLOPE_GAP);
    for (_, line, dy) in &mut borders {
        let x = (line.min_x + line.max_x) / 2.0;
        *dy = line.y_at_x(x) - reference_y(center, global_slope, x);
    }
    borders.sort_by(|one, two| one.2.total_cmp(&two.2));
    let delta = parameters.typical_height * MAX_BORDER_JITTER_RATIO;
    let mut groups = Vec::<BasicLine>::new();
    let mut group_weight = 0.0;
    let mut group_sum_dy = 0.0;
    for (_, line, dy) in borders {
        if groups.is_empty() || dy - (group_sum_dy / group_weight) > delta {
            groups.push(BasicLine::empty());
            group_weight = 0.0;
            group_sum_dy = 0.0;
        }
        group_weight += line.count;
        group_sum_dy += line.count * dy;
        groups.last_mut().unwrap().include_line(line);
    }
    groups.retain(|line| {
        line.segment()
            .is_some_and(|segment| segment.x2 - segment.x1 >= parameters.min_hook_width_low)
    });
    if groups.is_empty() {
        return Err(match side {
            BorderSide::Top => BeamStructureError::NoUsableTopBorder,
            BorderSide::Bottom => BeamStructureError::NoUsableBottomBorder,
        });
    }
    Ok(groups)
}

fn line_map(
    lines: Vec<BasicLine>,
    center: (f64, f64),
    global_slope: f64,
) -> Result<Vec<(f64, Segment)>, BeamStructureError> {
    lines
        .into_iter()
        .map(|line| {
            let segment = line
                .segment()
                .ok_or(BeamStructureError::UndefinedBorderLine)?;
            let x = (segment.x1 + segment.x2) / 2.0;
            let offset = line.y_at_x(x) - reference_y(center, global_slope, x);
            Ok((offset, segment))
        })
        .collect()
}

fn complete_border_lines(
    y_direction: f64,
    center: (f64, f64),
    global_slope: f64,
    typical_height: f64,
    base: &[(f64, Segment)],
    other: &mut [(f64, Segment)],
) {
    let shift = y_direction * typical_height;
    let tolerance = typical_height * MAX_BORDER_JITTER_RATIO;
    for &(base_offset, base_line) in base {
        let target = base_offset + shift;
        let Some(index) = other
            .iter()
            .position(|(offset, _)| (*offset - target).abs() <= tolerance)
        else {
            // Java's allowBorderCreation is false.
            continue;
        };
        let (_, candidate) = other[index];
        let x_mid = (candidate.x1 + candidate.x2) / 2.0;
        let y_mid = (candidate.y1 + candidate.y2) / 2.0;
        let height = y_mid - base_line.y_at_x(x_mid);
        let (x1, y1) = if base_line.x1 < candidate.x1 {
            (base_line.x1, base_line.y1 + height)
        } else {
            (candidate.x1, candidate.y1)
        };
        let (x2, y2) = if base_line.x2 > candidate.x2 {
            (base_line.x2, base_line.y2 + height)
        } else {
            (candidate.x2, candidate.y2)
        };
        let replacement = Segment { x1, y1, x2, y2 };
        let x = (x1 + x2) / 2.0;
        let offset = replacement.y_at_x(x) - reference_y(center, global_slope, x);
        other[index] = (offset, replacement);
    }
}

fn retrieve_items(
    sections: &[Section],
    offset_x: i32,
    offset_y: i32,
    median: Segment,
    height: f64,
    max_gap: i32,
) -> Vec<BeamItem> {
    let mut items = Vec::new();
    let mut start = None::<i32>;
    let mut stop = None::<i32>;
    for section in sections {
        let bounds = section.bounds();
        let left = offset_x + bounds.x as i32;
        let right_after = left + bounds.width as i32;
        let center_x = f64::from(left) + bounds.width as f64 / 2.0;
        let y = median.y_at_x(center_x) - f64::from(offset_y);
        if !section_contains(section, center_x - f64::from(offset_x), y) {
            continue;
        }
        if let Some(current_stop) = stop {
            if left - current_stop > max_gap {
                items.push(item_from_span(median, height, start.unwrap(), current_stop));
                start = Some(left);
            }
            stop = Some(current_stop.max(right_after));
        } else {
            start = Some(left);
            stop = Some(right_after);
        }
    }
    if let (Some(start), Some(stop)) = (start, stop) {
        items.push(item_from_span(median, height, start, stop));
    }
    items
}

fn section_contains(section: &Section, x: f64, y: f64) -> bool {
    let position = x.floor() as usize;
    if position < section.first_pos() || position > section.last_pos() {
        return false;
    }
    let run = section.runs()[position - section.first_pos()];
    y >= run.start as f64 && y < (run.stop() + 1) as f64
}

fn item_from_span(median: Segment, height: f64, start: i32, stop: i32) -> BeamItem {
    BeamItem {
        median: Segment {
            x1: f64::from(start),
            y1: median.y_at_x(f64::from(start)),
            x2: f64::from(stop),
            y2: median.y_at_x(f64::from(stop)),
        },
        height,
    }
}

fn glyph_centroid(glyph: &RunTable, offset_x: i32, offset_y: i32) -> (f64, f64) {
    let mut weight = 0_usize;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    for x in 0..glyph.width() {
        for run in glyph.sequence(x).unwrap_or_default() {
            weight += run.length;
            sum_x += (f64::from(offset_x) + x as f64) * run.length as f64;
            sum_y +=
                ((run.start + run.stop()) as f64 / 2.0 + f64::from(offset_y)) * run.length as f64;
        }
    }
    (sum_x / weight as f64, sum_y / weight as f64)
}

/// Java `LineUtil.yAtX(Point2D p1, double slope, double x)`.
///
/// Java does not evaluate the point-slope form either. It manufactures a second
/// point a thousand units along -- `(x1 + 1000, y1 + 1000 * slope)` -- and
/// intersects, so the multiplication by a thousand and the division back out
/// are both in the answer's last bits.
fn reference_y(center: (f64, f64), slope: f64, x: f64) -> f64 {
    intersection_at_x_from_slope(center, slope, x).1
}

/// Java `LineUtil.intersectionAtX(Point2D p1, double slope, double x)`.
///
/// Returns both coordinates, because the abscissa is not exactly `x`: Java
/// divides one rounded product by another, and the quotient is only
/// algebraically the query abscissa. Callers that use `p.getX()` get that
/// value, not their own.
#[must_use]
pub fn intersection_at_x_from_slope(point: (f64, f64), slope: f64, x: f64) -> (f64, f64) {
    line_util_intersection(
        point.0,
        point.1,
        point.0 + 1_000.0,
        point.1 + (1_000.0 * slope),
        x,
    )
}

/// Java `LineUtil.intersection`, specialised to the vertical query line every
/// `yAtX` and `intersectionAtX` in `LineUtil` uses: `(x, 0)` to `(x, 1000)`.
#[must_use]
pub fn line_util_intersection(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> (f64, f64) {
    let (x3, y3, x4, y4) = (x, 0.0, x, 1_000.0);
    let den = ((x1 - x2) * (y3 - y4)) - ((y1 - y2) * (x3 - x4));
    let v12 = (x1 * y2) - (y1 * x2);
    let v34 = (x3 * y4) - (y3 * x4);
    (
        ((v12 * (x3 - x4)) - ((x1 - x2) * v34)) / den,
        ((v12 * (y3 - y4)) - ((y1 - y2) * v34)) / den,
    )
}

/// Java `GradeUtil.clamp`: every impact is squeezed into `[0, 1]`.
///
/// `GradeImpacts.setImpact` applies this to each term before any grade is
/// taken. A width impact of 1.79 is entirely normal -- it happens whenever an
/// item is wider than the *hook* thresholds expect -- and leaving it unclamped
/// inflates the geometric mean.
#[must_use]
pub fn clamp_impact(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

/// Applies Java's clamp to all six terms, as `Impacts`' constructor does.
#[must_use]
pub fn clamped(impacts: BeamImpacts) -> BeamImpacts {
    BeamImpacts {
        width: clamp_impact(impacts.width),
        min_height: clamp_impact(impacts.min_height),
        max_height: clamp_impact(impacts.max_height),
        core: clamp_impact(impacts.core),
        belt: clamp_impact(impacts.belt),
        distance: clamp_impact(impacts.distance),
        raster: impacts.raster,
    }
}

/// `GradeImpacts.getGrade` for a beam: the weighted geometric mean, clamped
/// term by term first, times `Grades.intrinsicRatio`.
///
/// The clamp is part of this and not the caller's business. Three copies of
/// this function existed without it, which made every grade above a saturating
/// term plausible and too high.
#[must_use]
pub fn beam_grade(impacts: BeamImpacts) -> f64 {
    const WEIGHTS: [f64; 6] = [0.5, 1.0, 1.0, 2.0, 2.0, 2.0];
    let impacts = clamped(impacts);
    let values = [
        impacts.width,
        impacts.min_height,
        impacts.max_height,
        impacts.core,
        impacts.belt,
        impacts.distance,
    ];
    let mut product = 1.0;
    let mut total = 0.0;
    for (value, weight) in values.into_iter().zip(WEIGHTS) {
        total += weight;
        if value == 0.0 {
            product = 0.0;
        } else if weight != 0.0 {
            product *= java_positive_pow(value, weight);
        }
    }
    0.8 * java_positive_pow(product, 1.0 / total)
}

/// Which border of a beam line the jitter is measured against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JitterSide {
    Top,
    Bottom,
}

/// Java `BeamStructure.computeJitter`: how ragged one border of a line is.
///
/// This becomes the sixth beam impact, `jit`, and Java computes it once per
/// structure from the outermost lines rather than once per item. It is a
/// least-squares fit through one endpoint of every run in every section whose
/// centre lies on the median, divided by the glyph's width, so a straight
/// border scores near zero and a stepped one does not.
///
/// Two details are Java's and neither is what a rewrite would pick. The section
/// centre here is the *integer* one -- `GeoUtil.center`, with integer division,
/// where `retrieveItems` uses the `double` `center2D` -- and the abscissa
/// window is inset by the corner margin at both ends, so a beam's own tapered
/// corners do not count against it.
///
/// # Panics
///
/// Panics if the run table is not the vertical spot raster.
#[must_use]
pub fn compute_jitter(
    glyph: &RunTable,
    offset_x: i32,
    offset_y: i32,
    median: Segment,
    side: JitterSide,
    corner_margin: f64,
) -> f64 {
    assert_eq!(
        glyph.orientation(),
        Orientation::Vertical,
        "jitter needs the vertical spot raster"
    );
    let sections = build_sections(glyph, JunctionPolicy::DEFAULT_RATIO);

    let dx = corner_margin.round_ties_even() as i32;
    let x1 = (median.x1 + f64::from(dx)).round_ties_even() as i32;
    let x2 = (median.x2 - f64::from(dx)).round_ties_even() as i32;

    let mut line = audiveris_core::basic_line::BasicLine::default();
    for section in &sections {
        let bounds = section.bounds();
        let center_x = offset_x + bounds.x as i32 + (bounds.width as i32) / 2;
        let y = median.y_at_x(f64::from(center_x)).round_ties_even() as i32;

        if !section_contains(
            section,
            f64::from(center_x - offset_x),
            f64::from(y - offset_y),
        ) {
            continue;
        }

        // Java walks a counter alongside the runs because a section's runs are
        // consecutive positions from `firstPos`; enumerate says the same thing.
        let first = offset_x + section.first_pos() as i32;
        for (index, run) in section.runs().iter().enumerate() {
            let x = first + index as i32;
            if x >= x1 && x <= x2 {
                let end = match side {
                    JitterSide::Top => run.start,
                    JitterSide::Bottom => run.stop(),
                };
                line.include_point(f64::from(x), f64::from(offset_y + end as i32));
            }
        }
    }

    line.mean_distance().unwrap_or(0.0) / glyph.width() as f64
}

/// Java `LineUtil.yAtX(Line2D, double)`, term for term.
///
/// Not the point-slope form, which is what it looks like it should be and what
/// this used to be. Java computes a general line-line intersection against the
/// vertical segment from `(x, 0)` to `(x, 1000)`, via a determinant:
///
/// ```text
/// den = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4)
/// v12 = x1 * y2 - y1 * x2
/// v34 = x3 * y4 - y3 * x4
/// y   = (v12 * (y3 - y4) - (y1 - y2) * v34) / den
/// ```
///
/// The two forms are algebraically identical and differ in the last bits, and
/// the last bits are load-bearing here. A beam item's left edge is decided by
/// whether a section's centre lies *on* the median, and that containment test
/// is half-open: for one spot on chula the two forms give 924.9999999999998 and
/// 925.0 for the same query, a run ends at 924, and so the item starts a whole
/// section late. Three other medians differed in their ninth decimal for the
/// same reason.
fn line_util_y_at_x(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    line_util_intersection(x1, y1, x2, y2, x).1
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamImpactParameters {
    pub belt_margin_dx: i32,
    pub belt_margin_dy: i32,
    pub min_core_black_ratio: f64,
    pub max_belt_black_ratio: f64,
    pub min_width_low: f64,
    pub min_width_high: f64,
    pub min_height_low: f64,
    pub typical_height: f64,
    pub max_height_high: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BeamBeltSides {
    pub above: bool,
    pub below: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct BeamRaster<'a> {
    pub table: &'a RunTable,
    pub offset_x: i32,
    pub offset_y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamRasterEvidence {
    pub core_foreground: usize,
    pub core_count: usize,
    pub belt_foreground: usize,
    pub belt_count: usize,
    pub core_ratio: f64,
    pub belt_ratio: f64,
    pub rounded_width: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamImpacts {
    pub width: f64,
    pub min_height: f64,
    pub max_height: f64,
    pub core: f64,
    pub belt: f64,
    pub distance: f64,
    pub raster: BeamRasterEvidence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeamImpactRejection {
    Width,
    HeightBelow,
    HeightAbove,
    CoreRatio,
    BeltRatio,
}

/// Java `AreaMask` sampling and `BeamsBuilder.computeImpacts`. Absolute mask
/// points outside the raster still count in the denominator and are background.
pub fn compute_beam_impacts(
    item: BeamItem,
    sides: BeamBeltSides,
    raster: BeamRaster<'_>,
    distance_impact: f64,
    parameters: BeamImpactParameters,
) -> Result<BeamImpacts, BeamImpactRejection> {
    let core = Parallelogram::new(item.median, item.height);
    let top = if sides.above {
        parameters.belt_margin_dy
    } else {
        0
    };
    let bottom = if sides.below {
        parameters.belt_margin_dy
    } else {
        0
    };
    let shift_y = f64::from(bottom - top) / 2.0;
    let x1 = item.median.x1 - f64::from(parameters.belt_margin_dx);
    let x2 = item.median.x2 + f64::from(parameters.belt_margin_dx);
    let belt_median = Segment {
        x1,
        y1: item.median.y_at_x(x1) + shift_y,
        x2,
        y2: item.median.y_at_x(x2) + shift_y,
    };
    let outer = Parallelogram::new(belt_median, item.height + f64::from(top + bottom));
    let (core_foreground, core_count) =
        sample_mask(core, None, raster.table, raster.offset_x, raster.offset_y);
    let (belt_foreground, belt_count) = sample_mask(
        outer,
        Some(core),
        raster.table,
        raster.offset_x,
        raster.offset_y,
    );
    // Java double division intentionally yields NaN for a zero-area mask.
    let core_ratio = core_foreground as f64 / core_count as f64;
    let belt_ratio = belt_foreground as f64 / belt_count as f64;
    let rounded_width = (item.median.x2 - item.median.x1 + 1.0).round_ties_even() as i32;
    let raster_evidence = BeamRasterEvidence {
        core_foreground,
        core_count,
        belt_foreground,
        belt_count,
        core_ratio,
        belt_ratio,
        rounded_width,
    };
    if f64::from(rounded_width) < parameters.min_width_low {
        return Err(BeamImpactRejection::Width);
    }
    if item.height < parameters.min_height_low {
        return Err(BeamImpactRejection::HeightBelow);
    }
    if item.height > parameters.max_height_high {
        return Err(BeamImpactRejection::HeightAbove);
    }
    if core_ratio < parameters.min_core_black_ratio {
        return Err(BeamImpactRejection::CoreRatio);
    }
    if belt_ratio > parameters.max_belt_black_ratio {
        return Err(BeamImpactRejection::BeltRatio);
    }
    Ok(BeamImpacts {
        width: (f64::from(rounded_width) - parameters.min_width_low)
            / (parameters.min_width_high - parameters.min_width_low),
        min_height: (item.height - parameters.min_height_low)
            / (parameters.typical_height - parameters.min_height_low),
        max_height: (parameters.max_height_high - item.height)
            / (parameters.max_height_high - parameters.typical_height),
        core: (core_ratio - parameters.min_core_black_ratio)
            / (1.0 - parameters.min_core_black_ratio),
        belt: 1.0 - (belt_ratio / parameters.max_belt_black_ratio),
        distance: distance_impact,
        raster: raster_evidence,
    })
}

#[derive(Clone, Copy)]
struct Parallelogram {
    median: Segment,
    half_height: f64,
}

impl Parallelogram {
    fn new(median: Segment, height: f64) -> Self {
        Self {
            median,
            half_height: height / 2.0,
        }
    }

    /// `java.awt.geom.Area.contains(x, y)` for this parallelogram.
    ///
    /// Even-odd crossings, which is what Java2D's `Crossings` computes, and it
    /// is not the same as testing the point against the two sloped edges as
    /// functions of `x`. Each edge is taken over a half-open range in `y`, and
    /// the crossing abscissa is interpolated as **x at y**. Asking instead for
    /// y at x -- the obvious formulation, and what this used to do -- disagrees
    /// wherever an edge passes exactly through a pixel centre, and disagrees in
    /// *both* directions: on one beam Java included the point and on another it
    /// excluded an identical-looking one, which is why no open/closed
    /// convention could be made to fit. Measured over all 194 beams of
    /// BachInvention5, this rule reproduces Java exactly and each of the four
    /// obvious conventions gets one wrong.
    fn contains(self, x: f64, y: f64) -> bool {
        let corners = self.corners();
        let mut crossings = 0;
        for index in 0..4 {
            let (ax, ay) = corners[index];
            let (bx, by) = corners[(index + 1) % 4];
            if (ay <= y) == (by <= y) {
                continue;
            }
            if ax + (y - ay) * ((bx - ax) / (by - ay)) > x {
                crossings += 1;
            }
        }
        crossings % 2 == 1
    }

    /// The four defining points, in `AreaUtil.horizontalParallelogramPath`
    /// order: upper left, upper right, lower right, lower left.
    fn corners(self) -> [(f64, f64); 4] {
        [
            (self.median.x1, self.median.y1 - self.half_height),
            (self.median.x2, self.median.y2 - self.half_height),
            (self.median.x2, self.median.y2 + self.half_height),
            (self.median.x1, self.median.y1 + self.half_height),
        ]
    }

    fn integer_bounds(self) -> (i32, i32, i32, i32) {
        let min_y = (self.median.y1.min(self.median.y2) - self.half_height).floor() as i32;
        let max_y = (self.median.y1.max(self.median.y2) + self.half_height).ceil() as i32;
        (
            self.median.x1.floor() as i32,
            min_y,
            self.median.x2.ceil() as i32,
            max_y,
        )
    }
}

fn sample_mask(
    area: Parallelogram,
    subtract: Option<Parallelogram>,
    raster: &RunTable,
    offset_x: i32,
    offset_y: i32,
) -> (usize, usize) {
    let (left, top, right, bottom) = area.integer_bounds();
    let mut foreground = 0;
    let mut count = 0;
    for y in top..bottom {
        for x in left..right {
            if !area.contains(f64::from(x), f64::from(y))
                || subtract.is_some_and(|core| core.contains(f64::from(x), f64::from(y)))
            {
                continue;
            }
            count += 1;
            let local_x = x - offset_x;
            let local_y = y - offset_y;
            if local_x >= 0
                && local_y >= 0
                && (local_x as usize) < raster.width()
                && (local_y as usize) < raster.height()
                && raster.get(local_x as usize, local_y as usize) == FOREGROUND
            {
                foreground += 1;
            }
        }
    }
    (foreground, count)
}

/// Count Java `AreaMask` integer samples in a beam-shaped parallelogram.
/// This is shared by the post-classification extension kernel for middle and
/// side core-ratio checks.
#[must_use]
pub fn sample_beam_core(item: BeamItem, raster: BeamRaster<'_>) -> (usize, usize) {
    sample_mask(
        Parallelogram::new(item.median, item.height),
        None,
        raster.table,
        raster.offset_x,
        raster.offset_y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_table::{BACKGROUND, Run};

    fn vertical_table(width: usize, height: usize, columns: &[Run]) -> RunTable {
        assert_eq!(width, columns.len());
        let mut table = RunTable::new(Orientation::Vertical, width, height).unwrap();
        for (x, run) in columns.iter().copied().enumerate() {
            table.add_run(x, run).unwrap();
        }
        table
    }

    fn structure_parameters() -> BeamStructureParameters {
        BeamStructureParameters {
            typical_height: 4.0,
            core_section_width: 2,
            min_hook_width_low: 2.0,
            max_item_x_gap: 1,
            min_beam_width_low: 3.0,
            max_hook_width: 7.0,
        }
    }

    #[test]
    fn parallel_borders_form_one_exact_item_with_exclusive_bottom_border() {
        let glyph = vertical_table(8, 9, &[Run::new(2, 4); 8]);
        let analysis = analyze_beam_structure(&glyph, 10, 20, structure_parameters()).unwrap();
        assert_eq!(analysis.lines.len(), 1);
        let line = &analysis.lines[0];
        assert_eq!(
            line.median,
            Segment {
                x1: 10.0,
                y1: 24.0,
                x2: 17.0,
                y2: 24.0
            }
        );
        assert_eq!(line.height, 4.0);
        assert_eq!(
            line.items,
            vec![BeamItem {
                median: Segment {
                    x1: 10.0,
                    y1: 24.0,
                    x2: 18.0,
                    y2: 24.0
                },
                height: 4.0
            }]
        );
        assert_eq!(analysis.global_distance, 0.0);
        assert_eq!(analysis.mean_thickness, 4.0);
    }

    #[test]
    fn border_groups_recover_two_beams_and_item_gap_is_strictly_greater() {
        let mut glyph = RunTable::new(Orientation::Vertical, 9, 16).unwrap();
        for x in 0..4 {
            glyph.add_run(x, Run::new(1, 4)).unwrap();
            glyph.add_run(x, Run::new(9, 4)).unwrap();
        }
        for x in 6..9 {
            glyph.add_run(x, Run::new(1, 4)).unwrap();
            glyph.add_run(x, Run::new(9, 4)).unwrap();
        }
        let analysis = analyze_beam_structure(&glyph, 0, 0, structure_parameters()).unwrap();
        assert_eq!(analysis.lines.len(), 2);
        assert_eq!(analysis.lines[0].items.len(), 2); // gap 2 is strictly greater than max gap 1
        assert_eq!(analysis.lines[1].height, 4.0);
    }

    #[test]
    fn split_uses_ties_even_and_preserves_java_empty_item_behavior() {
        let line = BeamLine {
            median: Segment {
                x1: 1.0,
                y1: 6.0,
                x2: 9.0,
                y2: 6.0,
            },
            height: 8.0,
            items: vec![],
        };
        let mut even = BeamStructureAnalysis {
            lines: vec![line.clone()],
            global_distance: 0.0,
            mean_thickness: 6.0,
        };
        even.split_stuck_lines(4.0); // 1.5 ties to even 2
        assert_eq!(even.lines.len(), 2);
        assert_eq!(even.lines[0].height, 4.0);
        assert_eq!(even.lines[0].median.y1, 4.0);
        assert!(even.lines.iter().all(|line| line.items.is_empty()));
        let mut odd = BeamStructureAnalysis {
            lines: vec![line],
            global_distance: 0.0,
            mean_thickness: 5.999,
        };
        odd.split_stuck_lines(4.0);
        assert_eq!(odd.lines.len(), 1);
    }

    fn impact_parameters() -> BeamImpactParameters {
        BeamImpactParameters {
            belt_margin_dx: 1,
            belt_margin_dy: 1,
            min_core_black_ratio: 0.75,
            max_belt_black_ratio: 0.25,
            min_width_low: 4.0,
            min_width_high: 8.0,
            min_height_low: 2.0,
            typical_height: 4.0,
            max_height_high: 6.0,
        }
    }

    #[test]
    fn core_and_belt_sample_absolute_integer_points_and_outside_is_background() {
        let mut pixels = vec![BACKGROUND; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                pixels[y * 8 + x] = FOREGROUND;
            }
        }
        let raster = RunTable::from_pixels(Orientation::Horizontal, 8, 8, &pixels).unwrap();
        let item = BeamItem {
            median: Segment {
                x1: 2.0,
                y1: 4.0,
                x2: 6.0,
                y2: 4.0,
            },
            height: 4.0,
        };
        let impacts = compute_beam_impacts(
            item,
            BeamBeltSides {
                above: true,
                below: true,
            },
            BeamRaster {
                table: &raster,
                offset_x: 0,
                offset_y: 0,
            },
            0.8,
            impact_parameters(),
        )
        .unwrap();
        assert_eq!(
            (impacts.raster.core_foreground, impacts.raster.core_count),
            (16, 16)
        );
        assert_eq!(
            (impacts.raster.belt_foreground, impacts.raster.belt_count),
            (0, 20)
        );
        assert_eq!(impacts.raster.rounded_width, 5);
        assert_eq!(impacts.width, 0.25);
        assert_eq!(impacts.core, 1.0);
        assert_eq!(impacts.belt, 1.0);
        assert_eq!(impacts.distance, 0.8);
    }

    #[test]
    fn one_sided_belt_and_rejection_order_match_java() {
        let raster = vertical_table(4, 4, &[Run::new(0, 4); 4]);
        let short = BeamItem {
            median: Segment {
                x1: 0.0,
                y1: 2.0,
                x2: 2.4,
                y2: 2.0,
            },
            height: 1.0,
        };
        assert_eq!(
            compute_beam_impacts(
                short,
                BeamBeltSides {
                    above: true,
                    below: false,
                },
                BeamRaster {
                    table: &raster,
                    offset_x: 0,
                    offset_y: 0,
                },
                0.0,
                impact_parameters()
            ),
            Err(BeamImpactRejection::Width)
        );
        let low_core = BeamItem {
            median: Segment {
                x1: 0.0,
                y1: 8.0,
                x2: 4.0,
                y2: 8.0,
            },
            height: 4.0,
        };
        assert_eq!(
            compute_beam_impacts(
                low_core,
                BeamBeltSides {
                    above: false,
                    below: false,
                },
                BeamRaster {
                    table: &raster,
                    offset_x: 0,
                    offset_y: 0,
                },
                0.0,
                impact_parameters()
            ),
            Err(BeamImpactRejection::CoreRatio)
        );
    }

    #[test]
    fn non_vertical_structure_input_fails_closed() {
        let raster =
            RunTable::from_pixels(Orientation::Horizontal, 2, 2, &[FOREGROUND; 4]).unwrap();
        assert_eq!(
            analyze_beam_structure(&raster, 0, 0, structure_parameters()),
            Err(BeamStructureError::NonVerticalRunTable)
        );
    }
}
