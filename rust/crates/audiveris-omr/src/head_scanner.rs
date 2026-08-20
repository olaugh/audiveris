// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light geometry kernel for Java `NoteHeadsBuilder.Scanner`.
//!
//! This module stops before image lookup, template evaluation, and SIG
//! materialization. It owns the deterministic scale parameters, search-window
//! ordering, normal-head shape-set ordering, open/closed scanner decision, and
//! theoretical ordinate selection over abstract line and ledger adapters.

use std::error::Error;
use std::fmt;

use crate::head_template::HeadTemplateShape;

const MAX_TEMPLATE_DX: f64 = 0.375;
const MAX_CLOSED_DY: f64 = 0.0;
const MAX_OPEN_DY: f64 = 0.2;
const MIN_BEAM_WIDTH: f64 = 2.5;
const BAR_VERTICAL_MARGIN: f64 = 2.0;

/// Java `Template.maxDistanceLow()`.
pub const MAX_DISTANCE_LOW: f64 = 0.4;
/// Java `Template.reallyBadDistance()`.
pub const REALLY_BAD_DISTANCE: f64 = 1.0;

/// Scale-derived fields used by the deterministic scanner core.
#[derive(Clone, Debug, PartialEq)]
pub struct HeadScannerParameters {
    pub main_interline: i32,
    pub max_stem: i32,
    pub max_distance_low: f64,
    pub really_bad_distance: f64,
    pub max_template_dx: i32,
    pub max_closed_dy: i32,
    pub max_open_dy: i32,
    pub min_beam_width: i32,
    pub vertical_bar_margin: f64,
    pub min_template_width: i32,
    pub template_half: i32,
    pub x_offsets: Vec<i32>,
}

impl HeadScannerParameters {
    /// Port Java `NoteHeadsBuilder.Parameters` and the builder's three adjacent
    /// geometry derivations from the main interline and maximum stem width.
    pub fn derive(main_interline: i32, max_stem: i32) -> Result<Self, HeadScannerError> {
        if main_interline <= 0 {
            return Err(HeadScannerError::NonPositiveMainInterline(main_interline));
        }
        if max_stem <= 0 {
            return Err(HeadScannerError::NonPositiveMaxStem(max_stem));
        }

        let max_template_dx = scale_to_pixels(main_interline, MAX_TEMPLATE_DX)?;
        let max_closed_dy = 1.max(scale_to_pixels(main_interline, MAX_CLOSED_DY)?);
        let max_open_dy = 1.max(scale_to_pixels(main_interline, MAX_OPEN_DY)?);
        let min_beam_width = scale_to_pixels(main_interline, MIN_BEAM_WIDTH)?;
        let vertical_bar_margin = f64::from(main_interline) * BAR_VERTICAL_MARGIN;
        let template_half = main_interline
            .checked_mul(3)
            .ok_or(HeadScannerError::ArithmeticOverflow("template half"))?
            / 2;

        Ok(Self {
            main_interline,
            max_stem,
            max_distance_low: if crate::beam_veto::lax_head_distance_enabled() {
                // Accept every match that still carries a nonzero grade
                // (the grade formula runs out at MAX_DISTANCE_HIGH). Heads
                // fused with accidentals, beams, or ledgers measure "far
                // from the template" and die silently as no_best under
                // Java's 0.4 cap; between 0.4 and 0.5 they are perfectly
                // publishable low-grade hypotheses.
                crate::head_template::MAX_DISTANCE_HIGH
            } else {
                MAX_DISTANCE_LOW
            },
            really_bad_distance: REALLY_BAD_DISTANCE,
            max_template_dx,
            max_closed_dy,
            max_open_dy,
            min_beam_width,
            vertical_bar_margin,
            min_template_width: main_interline,
            template_half,
            x_offsets: compute_x_offsets(max_stem)?,
        })
    }
}

/// The four normal-staff shape intersections in Java `Shape`/`EnumSet` order.
///
/// This order is intentionally different from `TemplateFactory` construction
/// order, which starts with `NOTEHEAD_BLACK`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeadScannerShapeSets {
    pub all: &'static [HeadTemplateShape],
    pub stem: &'static [HeadTemplateShape],
    pub hollow: &'static [HeadTemplateShape],
}

pub const NORMAL_HEAD_SCANNER_SHAPES: HeadScannerShapeSets = HeadScannerShapeSets {
    all: &[
        HeadTemplateShape::Breve,
        HeadTemplateShape::WholeNote,
        HeadTemplateShape::NoteheadVoid,
        HeadTemplateShape::NoteheadBlack,
    ],
    stem: &[
        HeadTemplateShape::NoteheadVoid,
        HeadTemplateShape::NoteheadBlack,
    ],
    hollow: &[
        HeadTemplateShape::Breve,
        HeadTemplateShape::WholeNote,
        HeadTemplateShape::NoteheadVoid,
    ],
};

/// Abstract precise ordinate provider for either a staff line or ledger axis.
pub trait HeadScannerLine {
    fn y_at(&self, x: f64) -> f64;
}

/// Ordered ledger adapter used by the outside-staff midpoint branch.
pub trait HeadScannerLedger: HeadScannerLine {
    fn left_abscissa(&self) -> i32;
    fn right_abscissa(&self) -> i32;
}

/// Input to the pure theoretical-ordinate decision.
pub struct HeadTheoreticalOrdinateInput<'a> {
    pub line: &'a dyn HeadScannerLine,
    pub line2: Option<&'a dyn HeadScannerLine>,
    /// Java `getLedgerAdapters` order. The first ledger containing `x` wins.
    pub farther_ledgers: &'a [&'a dyn HeadScannerLedger],
    pub line_count: i32,
    pub interline: i32,
    pub pitch: i32,
    pub x: i32,
}

/// Which exact Java branch supplied a theoretical ordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeadTheoreticalOrdinateBasis {
    DirectMainLine,
    BetweenStaffLines,
    BetweenLedgerLines { ledger_index: usize },
    HalfInterlineBelowMain,
    HalfInterlineAboveMain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeadTheoreticalOrdinate {
    pub y: i32,
    pub basis: HeadTheoreticalOrdinateBasis,
}

/// Port Java `Scanner.getTheoreticalOrdinate` without line/SIG ownership.
pub fn theoretical_ordinate(
    input: HeadTheoreticalOrdinateInput<'_>,
) -> Result<HeadTheoreticalOrdinate, HeadScannerError> {
    if input.line_count <= 0 {
        return Err(HeadScannerError::NonPositiveLineCount(input.line_count));
    }
    if input.interline <= 0 {
        return Err(HeadScannerError::NonPositiveMainInterline(input.interline));
    }

    let pitch_magnitude = input
        .pitch
        .checked_abs()
        .ok_or(HeadScannerError::PitchMagnitudeOverflow)?;
    let x = f64::from(input.x);
    let outside = pitch_magnitude >= input.line_count;

    if outside {
        if (input.pitch % 2) == 0 {
            return rounded_result(
                ordinate(input.line, x, HeadOrdinateSource::MainLine)?,
                HeadTheoreticalOrdinateBasis::DirectMainLine,
            );
        }

        if pitch_magnitude > input.line_count {
            for (ledger_index, ledger) in input.farther_ledgers.iter().enumerate() {
                let left = ledger.left_abscissa();
                let right = ledger.right_abscissa();
                if left > right {
                    return Err(HeadScannerError::InvalidLedgerBounds {
                        ledger_index,
                        left,
                        right,
                    });
                }
                if input.x >= left && input.x <= right {
                    let main = ordinate(input.line, x, HeadOrdinateSource::MainLine)?;
                    let farther = ordinate(
                        *ledger,
                        x,
                        HeadOrdinateSource::FartherLedger { ledger_index },
                    )?;
                    return rounded_result(
                        (main + farther) / 2.0,
                        HeadTheoreticalOrdinateBasis::BetweenLedgerLines { ledger_index },
                    );
                }
            }
        }

        let main = ordinate(input.line, x, HeadOrdinateSource::MainLine)?;
        let half_interline = f64::from(input.interline) / 2.0;
        if input.pitch > 0 {
            rounded_result(
                main + half_interline,
                HeadTheoreticalOrdinateBasis::HalfInterlineBelowMain,
            )
        } else {
            rounded_result(
                main - half_interline,
                HeadTheoreticalOrdinateBasis::HalfInterlineAboveMain,
            )
        }
    } else if let Some(line2) = input.line2 {
        let main = ordinate(input.line, x, HeadOrdinateSource::MainLine)?;
        let secondary = ordinate(line2, x, HeadOrdinateSource::SecondaryLine)?;
        rounded_result(
            (main + secondary) / 2.0,
            HeadTheoreticalOrdinateBasis::BetweenStaffLines,
        )
    } else {
        rounded_result(
            ordinate(input.line, x, HeadOrdinateSource::MainLine)?,
            HeadTheoreticalOrdinateBasis::DirectMainLine,
        )
    }
}

/// Java's open-space predicate from the scanner constructor.
pub fn is_open_scanner(
    pitch: i32,
    line_count: i32,
    has_secondary_line: bool,
) -> Result<bool, HeadScannerError> {
    if line_count <= 0 {
        return Err(HeadScannerError::NonPositiveLineCount(line_count));
    }
    let magnitude = pitch
        .checked_abs()
        .ok_or(HeadScannerError::PitchMagnitudeOverflow)?;
    Ok((pitch % 2 != 0) && (!has_secondary_line || magnitude == line_count))
}

/// Port the Java open/closed ordinate search order.
pub fn compute_y_offsets(
    is_open: bool,
    direction: i32,
    max_open_dy: i32,
    max_closed_dy: i32,
) -> Result<Vec<i32>, HeadScannerError> {
    if !(-1..=1).contains(&direction) {
        return Err(HeadScannerError::InvalidDirection(direction));
    }
    if is_open && direction == 0 {
        return Err(HeadScannerError::OpenScannerWithoutDirection);
    }
    if max_open_dy < 0 || max_closed_dy < 0 {
        return Err(HeadScannerError::NegativeSearchExtent {
            max_open_dy,
            max_closed_dy,
        });
    }

    let maximum = if is_open { max_open_dy } else { max_closed_dy };
    let length = maximum
        .checked_add(1)
        .ok_or(HeadScannerError::ArithmeticOverflow("y-offset length"))?;
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(
            usize::try_from(length)
                .map_err(|_| HeadScannerError::ArithmeticOverflow("y-offset length"))?,
        )
        .map_err(|_| HeadScannerError::AllocationFailure("y offsets"))?;

    for index in 0..length {
        let offset = if is_open {
            match index {
                0 => 0,
                1 => direction,
                2 => -direction,
                _ => direction
                    .checked_mul(index - 1)
                    .ok_or(HeadScannerError::ArithmeticOverflow("open y offset"))?,
            }
        } else if (index % 2) == 0 {
            index / 2
        } else {
            -((index + 1) / 2)
        };
        offsets.push(offset);
    }
    Ok(offsets)
}

/// Port Java `computeXOffsets`, including its code/comment sign discrepancy.
pub fn compute_x_offsets(max_stem: i32) -> Result<Vec<i32>, HeadScannerError> {
    if max_stem <= 0 {
        return Err(HeadScannerError::NonPositiveMaxStem(max_stem));
    }
    let length = if (max_stem % 2) == 0 {
        max_stem
            .checked_add(1)
            .ok_or(HeadScannerError::ArithmeticOverflow("x-offset length"))?
    } else {
        max_stem
    };
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(
            usize::try_from(length)
                .map_err(|_| HeadScannerError::ArithmeticOverflow("x-offset length"))?,
        )
        .map_err(|_| HeadScannerError::AllocationFailure("x offsets"))?;
    for index in 0..length {
        offsets.push(if (index % 2) == 0 {
            -(index / 2)
        } else {
            (index + 1) / 2
        });
    }
    Ok(offsets)
}

fn scale_to_pixels(interline: i32, fraction: f64) -> Result<i32, HeadScannerError> {
    checked_java_rint(f64::from(interline) * fraction)
}

fn rounded_result(
    ordinate: f64,
    basis: HeadTheoreticalOrdinateBasis,
) -> Result<HeadTheoreticalOrdinate, HeadScannerError> {
    Ok(HeadTheoreticalOrdinate {
        y: checked_java_rint(ordinate)?,
        basis,
    })
}

fn checked_java_rint(value: f64) -> Result<i32, HeadScannerError> {
    if !value.is_finite() {
        return Err(HeadScannerError::NonFiniteOrdinate {
            source: HeadOrdinateSource::Computed,
        });
    }
    let rounded = value.round_ties_even();
    if rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
        return Err(HeadScannerError::OrdinateOutsideJavaInt { value });
    }
    Ok(rounded as i32)
}

fn ordinate(
    line: &dyn HeadScannerLine,
    x: f64,
    source: HeadOrdinateSource,
) -> Result<f64, HeadScannerError> {
    let value = line.y_at(x);
    value
        .is_finite()
        .then_some(value)
        .ok_or(HeadScannerError::NonFiniteOrdinate { source })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeadOrdinateSource {
    MainLine,
    SecondaryLine,
    FartherLedger { ledger_index: usize },
    Computed,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HeadScannerError {
    NonPositiveMainInterline(i32),
    NonPositiveMaxStem(i32),
    NonPositiveLineCount(i32),
    InvalidDirection(i32),
    OpenScannerWithoutDirection,
    NegativeSearchExtent {
        max_open_dy: i32,
        max_closed_dy: i32,
    },
    PitchMagnitudeOverflow,
    InvalidLedgerBounds {
        ledger_index: usize,
        left: i32,
        right: i32,
    },
    NonFiniteOrdinate {
        source: HeadOrdinateSource,
    },
    OrdinateOutsideJavaInt {
        value: f64,
    },
    ArithmeticOverflow(&'static str),
    AllocationFailure(&'static str),
}

impl fmt::Display for HeadScannerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid HEADS scanner input: {self:?}")
    }
}

impl Error for HeadScannerError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct ConstantLine(f64);

    impl HeadScannerLine for ConstantLine {
        fn y_at(&self, _x: f64) -> f64 {
            self.0
        }
    }

    #[derive(Clone, Copy)]
    struct ConstantLedger {
        left: i32,
        right: i32,
        y: f64,
    }

    impl HeadScannerLine for ConstantLedger {
        fn y_at(&self, _x: f64) -> f64 {
            self.y
        }
    }

    impl HeadScannerLedger for ConstantLedger {
        fn left_abscissa(&self) -> i32 {
            self.left
        }

        fn right_abscissa(&self) -> i32 {
            self.right
        }
    }

    #[test]
    fn corpus_parameter_regimes_match_the_frozen_scanner_oracle() {
        let p21 = HeadScannerParameters::derive(21, 4).unwrap();
        assert_eq!(p21.max_distance_low.to_bits(), 0x3fd9_9999_9999_999a);
        assert_eq!(p21.really_bad_distance.to_bits(), 0x3ff0_0000_0000_0000);
        assert_eq!(
            (
                p21.max_template_dx,
                p21.max_closed_dy,
                p21.max_open_dy,
                p21.min_beam_width,
                p21.vertical_bar_margin.to_bits(),
                p21.min_template_width,
                p21.template_half,
            ),
            (8, 1, 4, 52, 0x4045_0000_0000_0000, 21, 31)
        );
        assert_eq!(p21.x_offsets, [0, 1, -1, 2, -2]);

        let p20 = HeadScannerParameters::derive(20, 5).unwrap();
        assert_eq!(
            (
                p20.max_template_dx,
                p20.max_open_dy,
                p20.min_beam_width,
                p20.vertical_bar_margin.to_bits(),
                p20.template_half,
            ),
            (8, 4, 50, 0x4044_0000_0000_0000, 30)
        );
        assert_eq!(p20.x_offsets, [0, 1, -1, 2, -2]);

        let p17 = HeadScannerParameters::derive(17, 5).unwrap();
        assert_eq!(
            (
                p17.max_template_dx,
                p17.max_open_dy,
                p17.min_beam_width,
                p17.vertical_bar_margin.to_bits(),
                p17.template_half,
            ),
            (6, 3, 42, 0x4041_0000_0000_0000, 25)
        );
        assert_eq!(p17.x_offsets, [0, 1, -1, 2, -2]);
    }

    #[test]
    fn scanner_shape_sets_preserve_java_enumset_order() {
        assert_eq!(
            NORMAL_HEAD_SCANNER_SHAPES.all,
            [
                HeadTemplateShape::Breve,
                HeadTemplateShape::WholeNote,
                HeadTemplateShape::NoteheadVoid,
                HeadTemplateShape::NoteheadBlack,
            ]
        );
        assert_eq!(
            NORMAL_HEAD_SCANNER_SHAPES.stem,
            [
                HeadTemplateShape::NoteheadVoid,
                HeadTemplateShape::NoteheadBlack,
            ]
        );
        assert_eq!(
            NORMAL_HEAD_SCANNER_SHAPES.hollow,
            [
                HeadTemplateShape::Breve,
                HeadTemplateShape::WholeNote,
                HeadTemplateShape::NoteheadVoid,
            ]
        );
    }

    #[test]
    fn open_and_closed_offsets_match_both_corpus_directions() {
        assert_eq!(compute_y_offsets(false, 0, 4, 1), Ok(vec![0, -1]));
        assert_eq!(compute_y_offsets(true, 1, 4, 1), Ok(vec![0, 1, -1, 2, 3]));
        assert_eq!(
            compute_y_offsets(true, -1, 4, 1),
            Ok(vec![0, -1, 1, -2, -3])
        );
        assert_eq!(compute_y_offsets(true, -1, 3, 1), Ok(vec![0, -1, 1, -2]));

        assert_eq!(is_open_scanner(-5, 5, false), Ok(true));
        assert_eq!(is_open_scanner(-4, 5, false), Ok(false));
        assert_eq!(is_open_scanner(-3, 5, true), Ok(false));
        assert_eq!(is_open_scanner(7, 5, false), Ok(true));
    }

    #[test]
    fn theoretical_ordinate_preserves_java_branch_priority_and_ties_even() {
        let main = ConstantLine(20.0);
        let secondary = ConstantLine(21.0);
        let farther0 = ConstantLedger {
            left: 100,
            right: 110,
            y: 23.0,
        };
        let farther1 = ConstantLedger {
            left: 105,
            right: 115,
            y: 25.0,
        };
        let ledgers: [&dyn HeadScannerLedger; 2] = [&farther0, &farther1];

        let inside = theoretical_ordinate(HeadTheoreticalOrdinateInput {
            line: &main,
            line2: Some(&secondary),
            farther_ledgers: &[],
            line_count: 5,
            interline: 21,
            pitch: -3,
            x: 100,
        });
        assert_eq!(
            inside,
            Ok(HeadTheoreticalOrdinate {
                y: 20,
                basis: HeadTheoreticalOrdinateBasis::BetweenStaffLines,
            })
        );

        let ledger_midpoint = theoretical_ordinate(HeadTheoreticalOrdinateInput {
            line: &main,
            line2: Some(&secondary),
            farther_ledgers: &ledgers,
            line_count: 5,
            interline: 21,
            pitch: 7,
            x: 105,
        });
        assert_eq!(
            ledger_midpoint,
            Ok(HeadTheoreticalOrdinate {
                y: 22,
                basis: HeadTheoreticalOrdinateBasis::BetweenLedgerLines { ledger_index: 0 },
            })
        );

        let positive_fallback = theoretical_ordinate(HeadTheoreticalOrdinateInput {
            line: &main,
            line2: None,
            farther_ledgers: &ledgers,
            line_count: 5,
            interline: 21,
            pitch: 7,
            x: 99,
        });
        assert_eq!(
            positive_fallback,
            Ok(HeadTheoreticalOrdinate {
                y: 30,
                basis: HeadTheoreticalOrdinateBasis::HalfInterlineBelowMain,
            })
        );

        let negative_fallback = theoretical_ordinate(HeadTheoreticalOrdinateInput {
            line: &main,
            line2: None,
            farther_ledgers: &[],
            line_count: 5,
            interline: 21,
            pitch: -5,
            x: 99,
        });
        assert_eq!(
            negative_fallback,
            Ok(HeadTheoreticalOrdinate {
                y: 10,
                basis: HeadTheoreticalOrdinateBasis::HalfInterlineAboveMain,
            })
        );

        let direct = theoretical_ordinate(HeadTheoreticalOrdinateInput {
            line: &ConstantLine(10.5),
            line2: Some(&secondary),
            farther_ledgers: &ledgers,
            line_count: 5,
            interline: 21,
            pitch: 6,
            x: 105,
        });
        assert_eq!(
            direct,
            Ok(HeadTheoreticalOrdinate {
                y: 10,
                basis: HeadTheoreticalOrdinateBasis::DirectMainLine,
            })
        );
    }

    #[test]
    fn invalid_geometry_fails_typed_instead_of_wrapping_or_guessing() {
        assert_eq!(
            HeadScannerParameters::derive(0, 4),
            Err(HeadScannerError::NonPositiveMainInterline(0))
        );
        assert_eq!(
            compute_y_offsets(true, 0, 4, 1),
            Err(HeadScannerError::OpenScannerWithoutDirection)
        );
        assert_eq!(
            is_open_scanner(i32::MIN, 5, false),
            Err(HeadScannerError::PitchMagnitudeOverflow)
        );

        let main = ConstantLine(20.0);
        let bad = ConstantLedger {
            left: 20,
            right: 10,
            y: 30.0,
        };
        let ledgers: [&dyn HeadScannerLedger; 1] = [&bad];
        assert_eq!(
            theoretical_ordinate(HeadTheoreticalOrdinateInput {
                line: &main,
                line2: None,
                farther_ledgers: &ledgers,
                line_count: 5,
                interline: 21,
                pitch: 7,
                x: 15,
            }),
            Err(HeadScannerError::InvalidLedgerBounds {
                ledger_index: 0,
                left: 20,
                right: 10,
            })
        );
    }
}
