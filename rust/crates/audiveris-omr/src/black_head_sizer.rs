// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light measurement kernel from Java `BlackHeadSizer`.
//!
//! This module stops at `Scale.BlackHeadScale`. Java immediately feeds the
//! measured width to `MusicFont.buildMusicFontScale`, but that conversion is a
//! font-metric dependency and deliberately remains outside this kernel.

use std::{error::Error, fmt};

use audiveris_core::population::Population;
use audiveris_image::{
    global_filter::global_filter,
    glyph_factory::{GlyphComponent, build_glyph_components},
    morphology::close_with_disk,
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable, RunTableError},
};

/// Java `BlackHeadSizer.Constants` resolved against a sheet interline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlackHeadSizerParameters {
    /// `closingDiameter`, kept unrounded as `Scale.toPixelsDouble` does.
    pub closing_diameter: f64,
    /// Inclusive ImageJ threshold applied after head-oriented closing.
    pub binarization_threshold: u8,
    pub min_width: i32,
    pub max_width: i32,
    pub min_height: i32,
    pub max_height: i32,
    pub min_weight: i32,
    pub max_weight: i32,
    pub min_stack_height: i32,
    pub max_stack_height: i32,
    pub min_stack_weight: i32,
    pub max_stack_weight: i32,
    pub singles_quorum: usize,
}

impl BlackHeadSizerParameters {
    /// Resolve the source defaults with Java `Scale.toPixels` arithmetic.
    ///
    /// Fractions use `Math.rint` (ties to even). Area fractions first perform
    /// Java's wrapping `int * int`, then widen the product to `double`.
    #[must_use]
    pub fn from_interline(interline: i32) -> Self {
        let length = |fraction| java_rint(f64::from(interline) * fraction);
        let square = interline.wrapping_mul(interline);
        let area = |fraction| java_rint(f64::from(square) * fraction);

        Self {
            closing_diameter: f64::from(interline) * 0.9,
            binarization_threshold: 110,
            min_width: length(1.0),
            max_width: length(2.0),
            min_height: length(0.9),
            max_height: length(1.5),
            min_weight: area(0.8),
            max_weight: area(1.5),
            min_stack_height: length(2.0),
            max_stack_height: length(4.0),
            min_stack_weight: area(2.0),
            max_stack_weight: area(4.0),
            singles_quorum: 20,
        }
    }

    /// `(float) (diameter - 1) / 2`, preserving Java's narrowing point.
    #[must_use]
    pub fn closing_radius(self) -> f32 {
        (self.closing_diameter - 1.0) as f32 / 2.0
    }
}

/// Whether to execute Java `closeBlackHead` or measure the supplied spots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlackHeadClosing {
    Enabled,
    Disabled,
}

/// Typed input to the measurement-only part of `BlackHeadSizer.process`.
#[derive(Clone, Copy, Debug)]
pub struct BlackHeadSizingInput<'a> {
    /// Beam-threshold components in Java `GlyphFactory` order.
    pub spots: &'a [GlyphComponent],
    pub parameters: BlackHeadSizerParameters,
    pub closing: BlackHeadClosing,
}

/// Which invocation of Java `checkSpot` rejected a candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlackHeadCheckPhase {
    BeforeClosing,
    AfterClosing,
}

/// The first failed condition in Java `checkSpot` source order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlackHeadCheckRejection {
    WidthBelowMinimum,
    WidthAboveMaximum,
    WeightBelowMinimum,
    WeightAboveMaximum,
    HeightBelowMinimum,
    HeightAboveMaximum,
}

/// Final branch taken for one input spot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlackHeadCandidateDecision {
    Rejected {
        phase: BlackHeadCheckPhase,
        reason: BlackHeadCheckRejection,
    },
    /// Head closing did not leave exactly one component.
    ClosingComponentCount(usize),
    Single,
    Stack,
    /// Passed both `checkSpot` calls but met neither single nor stack limits.
    Unclassified,
}

/// Input-order trace for one source spot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlackHeadCandidateTrace {
    pub source_index: usize,
    pub source_ancestor_mark: usize,
    pub decision: BlackHeadCandidateDecision,
}

/// A candidate retained by either the single or stack branch.
#[derive(Clone, Debug, PartialEq)]
pub struct MeasuredBlackHeadSpot {
    pub source_index: usize,
    pub was_closed: bool,
    pub glyph: GlyphComponent,
}

/// Java `Scale.BlackHeadScale`, before music-font point-size selection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlackHeadScale {
    pub width_mean: f64,
    pub width_std: f64,
    pub height_mean: f64,
    pub height_std: f64,
}

/// Exact observable result of the dependency-light measurement kernel.
#[derive(Clone, Debug, PartialEq)]
pub struct BlackHeadSizingResult {
    /// One record per source spot, always in input order.
    pub traces: Vec<BlackHeadCandidateTrace>,
    /// Java's `singles` list after `measureSingles`: discovery order below
    /// quorum, stable increasing-width order at or above quorum.
    pub singles: Vec<MeasuredBlackHeadSpot>,
    /// Java's `stacks` list, which always remains in discovery order.
    pub stacks: Vec<MeasuredBlackHeadSpot>,
    /// Source indexes in the half-open middle-half sublist used by Population.
    pub core_single_source_indices: Vec<usize>,
    /// Absent when the single count is below `singlesQuorum`.
    pub black_head_scale: Option<BlackHeadScale>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlackHeadSizerError {
    GeometryOutsideJavaInt { source_index: usize },
    EmptyComponent { source_index: usize },
    InconsistentComponentRaster { source_index: usize },
    RunTable(RunTableError),
}

impl fmt::Display for BlackHeadSizerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GeometryOutsideJavaInt { source_index } => {
                write!(
                    formatter,
                    "black-head spot {source_index} exceeds Java int geometry"
                )
            }
            Self::EmptyComponent { source_index } => {
                write!(formatter, "black-head spot {source_index} has no runs")
            }
            Self::InconsistentComponentRaster { source_index } => write!(
                formatter,
                "black-head spot {source_index} has runs outside its declared bounds"
            ),
            Self::RunTable(error) => write!(formatter, "invalid closed black-head raster: {error}"),
        }
    }
}

impl Error for BlackHeadSizerError {}

impl From<RunTableError> for BlackHeadSizerError {
    fn from(error: RunTableError) -> Self {
        Self::RunTable(error)
    }
}

/// Port of `BlackHeadSizer.process` through `BlackHeadScale` construction.
pub fn measure_black_heads(
    input: &BlackHeadSizingInput<'_>,
) -> Result<BlackHeadSizingResult, BlackHeadSizerError> {
    let mut traces = Vec::with_capacity(input.spots.len());
    let mut singles = Vec::new();
    let mut stacks = Vec::new();

    for (source_index, source) in input.spots.iter().enumerate() {
        let source_ancestor_mark = source.ancestor_mark;
        if let Some(reason) = check_spot(source, input.parameters, source_index)? {
            traces.push(BlackHeadCandidateTrace {
                source_index,
                source_ancestor_mark,
                decision: BlackHeadCandidateDecision::Rejected {
                    phase: BlackHeadCheckPhase::BeforeClosing,
                    reason,
                },
            });
            continue;
        }

        let measured = match input.closing {
            BlackHeadClosing::Disabled => source.clone(),
            BlackHeadClosing::Enabled => {
                let closed = close_black_head(source, input.parameters, source_index)?;
                if closed.len() != 1 {
                    traces.push(BlackHeadCandidateTrace {
                        source_index,
                        source_ancestor_mark,
                        decision: BlackHeadCandidateDecision::ClosingComponentCount(closed.len()),
                    });
                    continue;
                }
                closed.into_iter().next().unwrap()
            }
        };

        if let Some(reason) = check_spot(&measured, input.parameters, source_index)? {
            traces.push(BlackHeadCandidateTrace {
                source_index,
                source_ancestor_mark,
                decision: BlackHeadCandidateDecision::Rejected {
                    phase: BlackHeadCheckPhase::AfterClosing,
                    reason,
                },
            });
            continue;
        }

        let height = i32::try_from(measured.height)
            .map_err(|_| BlackHeadSizerError::GeometryOutsideJavaInt { source_index })?;
        let weight = i32::try_from(measured.weight)
            .map_err(|_| BlackHeadSizerError::GeometryOutsideJavaInt { source_index })?;
        let retained = MeasuredBlackHeadSpot {
            source_index,
            was_closed: input.closing == BlackHeadClosing::Enabled,
            glyph: measured,
        };

        let decision =
            if height <= input.parameters.max_height && weight <= input.parameters.max_weight {
                singles.push(retained);
                BlackHeadCandidateDecision::Single
            } else if height >= input.parameters.min_stack_height
                && weight >= input.parameters.min_stack_weight
            {
                stacks.push(retained);
                BlackHeadCandidateDecision::Stack
            } else {
                BlackHeadCandidateDecision::Unclassified
            };
        traces.push(BlackHeadCandidateTrace {
            source_index,
            source_ancestor_mark,
            decision,
        });
    }

    let (core_single_source_indices, black_head_scale) =
        measure_singles(&mut singles, input.parameters.singles_quorum);

    Ok(BlackHeadSizingResult {
        traces,
        singles,
        stacks,
        core_single_source_indices,
        black_head_scale,
    })
}

fn check_spot(
    glyph: &GlyphComponent,
    parameters: BlackHeadSizerParameters,
    source_index: usize,
) -> Result<Option<BlackHeadCheckRejection>, BlackHeadSizerError> {
    let width = i32::try_from(glyph.width)
        .map_err(|_| BlackHeadSizerError::GeometryOutsideJavaInt { source_index })?;
    if width < parameters.min_width {
        return Ok(Some(BlackHeadCheckRejection::WidthBelowMinimum));
    }
    if width > parameters.max_width {
        return Ok(Some(BlackHeadCheckRejection::WidthAboveMaximum));
    }

    let weight = i32::try_from(glyph.weight)
        .map_err(|_| BlackHeadSizerError::GeometryOutsideJavaInt { source_index })?;
    if weight < parameters.min_weight {
        return Ok(Some(BlackHeadCheckRejection::WeightBelowMinimum));
    }
    if weight > parameters.max_stack_weight {
        return Ok(Some(BlackHeadCheckRejection::WeightAboveMaximum));
    }

    let height = i32::try_from(glyph.height)
        .map_err(|_| BlackHeadSizerError::GeometryOutsideJavaInt { source_index })?;
    if height < parameters.min_height {
        return Ok(Some(BlackHeadCheckRejection::HeightBelowMinimum));
    }
    if height > parameters.max_stack_height {
        return Ok(Some(BlackHeadCheckRejection::HeightAboveMaximum));
    }

    Ok(None)
}

fn close_black_head(
    glyph: &GlyphComponent,
    parameters: BlackHeadSizerParameters,
    source_index: usize,
) -> Result<Vec<GlyphComponent>, BlackHeadSizerError> {
    let mut pixels = component_pixels(glyph, source_index)?;
    close_with_disk(
        &mut pixels,
        glyph.width,
        glyph.height,
        parameters.closing_radius(),
    );
    let pixels = global_filter(&pixels, parameters.binarization_threshold);
    let table = RunTable::from_pixels(Orientation::Vertical, glyph.width, glyph.height, &pixels)?;
    Ok(build_glyph_components(&table, glyph.left, glyph.top))
}

fn component_pixels(
    glyph: &GlyphComponent,
    source_index: usize,
) -> Result<Vec<u8>, BlackHeadSizerError> {
    let min_sequence = glyph
        .runs
        .iter()
        .map(|entry| entry.sequence)
        .min()
        .ok_or(BlackHeadSizerError::EmptyComponent { source_index })?;
    let min_coordinate = glyph
        .runs
        .iter()
        .map(|entry| entry.run.start)
        .min()
        .ok_or(BlackHeadSizerError::EmptyComponent { source_index })?;
    let mut pixels = vec![BACKGROUND; glyph.width * glyph.height];

    for entry in &glyph.runs {
        for coordinate in entry.run.start..=entry.run.stop() {
            let (x, y) = match glyph.orientation {
                Orientation::Horizontal => {
                    (coordinate - min_coordinate, entry.sequence - min_sequence)
                }
                Orientation::Vertical => {
                    (entry.sequence - min_sequence, coordinate - min_coordinate)
                }
            };
            let Some(index) = y
                .checked_mul(glyph.width)
                .and_then(|row| row.checked_add(x))
                .filter(|&index| x < glyph.width && y < glyph.height && index < pixels.len())
            else {
                return Err(BlackHeadSizerError::InconsistentComponentRaster { source_index });
            };
            pixels[index] = FOREGROUND;
        }
    }
    Ok(pixels)
}

fn measure_singles(
    singles: &mut [MeasuredBlackHeadSpot],
    quorum: usize,
) -> (Vec<usize>, Option<BlackHeadScale>) {
    if singles.len() < quorum {
        return (Vec::new(), None);
    }

    // `Collections.sort` is stable. Width is its sole comparison key.
    singles.sort_by_key(|single| single.glyph.width);
    let core_start = singles.len() / 4;
    let core_stop = (3 * singles.len()) / 4;
    let core = &singles[core_start..core_stop];
    let mut widths = Population::default();
    let mut heights = Population::default();
    for single in core {
        widths.include(single.glyph.width as f64);
        heights.include(single.glyph.height as f64);
    }

    let scale = BlackHeadScale {
        width_mean: widths
            .mean()
            .expect("quorum yields a non-empty middle half"),
        width_std: widths
            .standard_deviation()
            .expect("quorum yields a non-empty middle half"),
        height_mean: heights
            .mean()
            .expect("quorum yields a non-empty middle half"),
        height_std: heights
            .standard_deviation()
            .expect("quorum yields a non-empty middle half"),
    };
    (
        core.iter().map(|single| single.source_index).collect(),
        Some(scale),
    )
}

fn java_rint(value: f64) -> i32 {
    value.round_ties_even() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_component(width: usize, height: usize, mark: usize) -> GlyphComponent {
        let pixels = vec![FOREGROUND; width * height];
        component_from_pixels(width, height, &pixels, mark)
    }

    fn component_from_pixels(
        width: usize,
        height: usize,
        pixels: &[u8],
        mark: usize,
    ) -> GlyphComponent {
        let table = RunTable::from_pixels(Orientation::Vertical, width, height, pixels).unwrap();
        let mut components = build_glyph_components(&table, 100, 200);
        assert_eq!(components.len(), 1);
        let mut component = components.remove(0);
        component.ancestor_mark = mark;
        component
    }

    fn disabled<'a>(spots: &'a [GlyphComponent], interline: i32) -> BlackHeadSizingInput<'a> {
        BlackHeadSizingInput {
            spots,
            parameters: BlackHeadSizerParameters::from_interline(interline),
            closing: BlackHeadClosing::Disabled,
        }
    }

    #[test]
    fn parameters_use_length_and_area_rint_at_the_java_expression_boundaries() {
        let parameters = BlackHeadSizerParameters::from_interline(5);
        assert_eq!(parameters.min_height, 4); // rint(4.5), even side down.
        assert_eq!(parameters.max_height, 8); // rint(7.5), even side up.
        assert_eq!(parameters.min_weight, 20);
        assert_eq!(parameters.max_weight, 38); // rint(37.5).
        assert_eq!(parameters.min_stack_weight, 50);
        assert_eq!(parameters.max_stack_weight, 100);
        assert_eq!(parameters.closing_diameter, 4.5);
        assert_eq!(parameters.closing_radius(), 1.75);
    }

    #[test]
    fn filtering_and_single_stack_branches_follow_source_order() {
        let spots = vec![
            solid_component(9, 9, 1),   // Width rejected before its weight.
            solid_component(10, 9, 2),  // Single at all lower bounds.
            solid_component(10, 20, 3), // Stack at all lower bounds.
            solid_component(10, 16, 4), // Neither branch.
            solid_component(10, 41, 5), // Height rejected after width/weight.
        ];
        let result = measure_black_heads(&disabled(&spots, 10)).unwrap();
        assert_eq!(
            result
                .traces
                .iter()
                .map(|trace| trace.decision)
                .collect::<Vec<_>>(),
            [
                BlackHeadCandidateDecision::Rejected {
                    phase: BlackHeadCheckPhase::BeforeClosing,
                    reason: BlackHeadCheckRejection::WidthBelowMinimum,
                },
                BlackHeadCandidateDecision::Single,
                BlackHeadCandidateDecision::Stack,
                BlackHeadCandidateDecision::Unclassified,
                BlackHeadCandidateDecision::Rejected {
                    phase: BlackHeadCheckPhase::BeforeClosing,
                    reason: BlackHeadCheckRejection::WeightAboveMaximum,
                },
            ]
        );
        assert_eq!(result.singles[0].source_index, 1);
        assert_eq!(result.stacks[0].source_index, 2);
        assert!(result.black_head_scale.is_none());
    }

    #[test]
    fn precheck_happens_before_optional_closing() {
        let mut holed = vec![FOREGROUND; 6 * 5];
        holed[2 * 6 + 2] = BACKGROUND;
        holed[2 * 6 + 3] = BACKGROUND;
        let spot = component_from_pixels(6, 5, &holed, 7);

        let disabled_result =
            measure_black_heads(&disabled(std::slice::from_ref(&spot), 5)).unwrap();
        assert_eq!(disabled_result.singles[0].glyph.weight, 28);

        let enabled = BlackHeadSizingInput {
            spots: std::slice::from_ref(&spot),
            parameters: BlackHeadSizerParameters::from_interline(5),
            closing: BlackHeadClosing::Enabled,
        };
        let enabled_result = measure_black_heads(&enabled).unwrap();
        // This source-shaped head survives the black-on-white operator. The
        // close is still observable in the rebuilt vertical component and its
        // retained provenance flag; `MorphoProcessor.close` is not a generic
        // black-hole filler (it dilates white first).
        assert_eq!(enabled_result.singles[0].glyph.weight, 28);
        assert_eq!(
            enabled_result.singles[0].glyph.orientation,
            Orientation::Vertical
        );
        assert!(enabled_result.singles[0].was_closed);

        // A one-pixel plus spans a valid candidate box but is narrower than
        // the head-oriented element. Disabled mode retains it; enabled mode
        // follows closeBlackHead's `glyphs.size() != 1` rejection with zero
        // rebuilt components.
        let mut plus = vec![BACKGROUND; 9 * 9];
        for coordinate in 0..9 {
            plus[4 * 9 + coordinate] = FOREGROUND;
            plus[coordinate * 9 + 4] = FOREGROUND;
        }
        let plus = component_from_pixels(9, 9, &plus, 9);
        let mut permissive = BlackHeadSizerParameters::from_interline(5);
        permissive.min_weight = 1;
        let retained = measure_black_heads(&BlackHeadSizingInput {
            spots: std::slice::from_ref(&plus),
            parameters: permissive,
            closing: BlackHeadClosing::Disabled,
        })
        .unwrap();
        assert_eq!(
            retained.traces[0].decision,
            BlackHeadCandidateDecision::Unclassified
        );
        let erased = measure_black_heads(&BlackHeadSizingInput {
            spots: std::slice::from_ref(&plus),
            parameters: permissive,
            closing: BlackHeadClosing::Enabled,
        })
        .unwrap();
        assert_eq!(
            erased.traces[0].decision,
            BlackHeadCandidateDecision::ClosingComponentCount(0)
        );

        // Java rejects this low-weight candidate before invoking
        // closeBlackHead. Clearing its runs makes that ordering observable:
        // reaching closing would report EmptyComponent instead.
        let mut ring = vec![BACKGROUND; 25];
        for x in 0..5 {
            ring[x] = FOREGROUND;
            ring[4 * 5 + x] = FOREGROUND;
        }
        for y in 1..4 {
            ring[y * 5] = FOREGROUND;
            ring[y * 5 + 4] = FOREGROUND;
        }
        let mut ring = component_from_pixels(5, 5, &ring, 8);
        ring.runs.clear();
        let rejected = measure_black_heads(&BlackHeadSizingInput {
            spots: std::slice::from_ref(&ring),
            parameters: BlackHeadSizerParameters::from_interline(5),
            closing: BlackHeadClosing::Enabled,
        })
        .unwrap();
        assert_eq!(
            rejected.traces[0].decision,
            BlackHeadCandidateDecision::Rejected {
                phase: BlackHeadCheckPhase::BeforeClosing,
                reason: BlackHeadCheckRejection::WeightBelowMinimum,
            }
        );
    }

    #[test]
    fn quorum_sort_is_stable_and_core_uses_java_integer_quarters() {
        let widths = [
            14, 10, 12, 11, 13, 10, 15, 12, 14, 11, 13, 10, 15, 12, 14, 11, 13, 10, 15, 12, 14,
        ];
        let spots = widths
            .iter()
            .enumerate()
            .map(|(index, &width)| solid_component(width, 9 + index % 2, index + 1))
            .collect::<Vec<_>>();
        let result = measure_black_heads(&disabled(&spots, 10)).unwrap();

        assert_eq!(
            result
                .singles
                .iter()
                .map(|single| single.source_index)
                .collect::<Vec<_>>(),
            [
                1, 5, 11, 17, 3, 9, 15, 2, 7, 13, 19, 4, 10, 16, 0, 8, 14, 20, 6, 12, 18
            ]
        );
        // n=21: Java subList(21/4, (3*21)/4) is [5, 15), ten values.
        assert_eq!(
            result.core_single_source_indices,
            [9, 15, 2, 7, 13, 19, 4, 10, 16, 0]
        );
        let scale = result.black_head_scale.unwrap();
        assert_eq!(scale.width_mean, 12.3);
        // Population's literal operation order leaves this residual below the
        // mathematical sqrt(.9); pin the Java-shaped arithmetic, not a formula
        // simplified by hand.
        assert_eq!(scale.width_std, 0.948_683_298_050_508_4);
        assert_eq!(scale.height_mean, 9.5);
        assert!((scale.height_std - (5.0_f64 / 18.0).sqrt()).abs() < 1e-15);
    }

    #[test]
    fn below_quorum_leaves_singles_in_discovery_order() {
        let spots = (0..19)
            .map(|index| solid_component(15 - (index % 6), 9, index + 1))
            .collect::<Vec<_>>();
        let result = measure_black_heads(&disabled(&spots, 10)).unwrap();
        assert_eq!(
            result
                .singles
                .iter()
                .map(|single| single.source_index)
                .collect::<Vec<_>>(),
            (0..19).collect::<Vec<_>>()
        );
        assert!(result.core_single_source_indices.is_empty());
        assert!(result.black_head_scale.is_none());
    }
}
