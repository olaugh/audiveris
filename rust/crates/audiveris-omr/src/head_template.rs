// SPDX-License-Identifier: AGPL-3.0-or-later

//! Frozen-data representation and pure evaluator for Java HEADS templates.
//!
//! Java builds these records from a rendered music-font symbol. Rendering is
//! deliberately outside this module: callers must supply the already frozen
//! geometry, anchor offsets, and row-major `PixelDistance` records. This keeps
//! the matching kernel deterministic and prevents it from silently substituting
//! a different rasterizer for the Java catalog.

use std::error::Error;
use std::fmt;

use audiveris_image::chamfer::VALUE_UNKNOWN;

use crate::heads_step::NeutralDistanceTable;

/// Java `Template.Constants.foreWeight`.
pub const FOREGROUND_WEIGHT: f64 = 6.0;
/// Java `Template.Constants.backWeight`.
pub const BACKGROUND_WEIGHT: f64 = 1.0;
/// Java `Template.Constants.holeWeight`.
pub const HOLE_WEIGHT: f64 = 4.0;

/// The normal-staff head shapes used by the measured HEADS scan.
///
/// Discriminants preserve `TemplateFactory` construction order, which differs
/// from `ShapeSet.Heads` scan order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum HeadTemplateShape {
    NoteheadBlack = 0,
    NoteheadVoid = 1,
    WholeNote = 2,
    Breve = 3,
}

impl HeadTemplateShape {
    pub const FACTORY_ORDER: [Self; 4] = [
        Self::NoteheadBlack,
        Self::NoteheadVoid,
        Self::WholeNote,
        Self::Breve,
    ];

    const STEMMED_ANCHORS: [HeadTemplateAnchor; 9] = HeadTemplateAnchor::ALL;
    const STEMLESS_ANCHORS: [HeadTemplateAnchor; 3] = [
        HeadTemplateAnchor::Center,
        HeadTemplateAnchor::MiddleLeft,
        HeadTemplateAnchor::MiddleRight,
    ];

    #[must_use]
    pub const fn factory_ordinal(self) -> usize {
        self as usize
    }

    #[must_use]
    pub const fn required_anchors(self) -> &'static [HeadTemplateAnchor] {
        match self {
            Self::NoteheadBlack | Self::NoteheadVoid => &Self::STEMMED_ANCHORS,
            Self::WholeNote | Self::Breve => &Self::STEMLESS_ANCHORS,
        }
    }
}

/// Java `Anchored.Anchor`, in declaration/`EnumMap` order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum HeadTemplateAnchor {
    Center = 0,
    MiddleLeft = 1,
    MiddleRight = 2,
    LeftStem = 3,
    TopLeftStem = 4,
    BottomLeftStem = 5,
    RightStem = 6,
    TopRightStem = 7,
    BottomRightStem = 8,
}

impl HeadTemplateAnchor {
    pub const ALL: [Self; 9] = [
        Self::Center,
        Self::MiddleLeft,
        Self::MiddleRight,
        Self::LeftStem,
        Self::TopLeftStem,
        Self::BottomLeftStem,
        Self::RightStem,
        Self::TopRightStem,
        Self::BottomRightStem,
    ];

    const fn horizontal_side(self) -> HorizontalSide {
        match self {
            Self::MiddleLeft | Self::LeftStem | Self::TopLeftStem | Self::BottomLeftStem => {
                HorizontalSide::Left
            }
            Self::Center => HorizontalSide::Center,
            Self::MiddleRight | Self::RightStem | Self::TopRightStem | Self::BottomRightStem => {
                HorizontalSide::Right
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HorizontalSide {
    Left,
    Center,
    Right,
}

/// The only music family present in the frozen normal-head catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HeadTemplateFamily {
    Bravura,
}

/// An integer rectangle with Java `Rectangle` width/height semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeadTemplateBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// One precise anchor translation from the template upper-left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadTemplateAnchorOffset {
    pub anchor: HeadTemplateAnchor,
    pub dx: f64,
    pub dy: f64,
}

/// One Java `PixelDistance`, retained in factory row-major order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadTemplatePixelDistance {
    pub x: i32,
    pub y: i32,
    pub distance: f64,
}

/// Exact serialized fields needed to construct one frozen template.
#[derive(Clone, Debug, PartialEq)]
pub struct HeadTemplateData {
    pub shape: HeadTemplateShape,
    pub family: HeadTemplateFamily,
    pub point_size: i32,
    pub width: i32,
    pub height: i32,
    pub slim_bounds: HeadTemplateBounds,
    pub anchors: Vec<HeadTemplateAnchorOffset>,
    pub key_points: Vec<HeadTemplatePixelDistance>,
}

/// A frozen Java `Template` record.
#[derive(Clone, Debug, PartialEq)]
pub struct HeadTemplate {
    shape: HeadTemplateShape,
    family: HeadTemplateFamily,
    point_size: i32,
    width: i32,
    height: i32,
    slim_bounds: HeadTemplateBounds,
    anchors: Vec<HeadTemplateAnchorOffset>,
    key_points: Vec<HeadTemplatePixelDistance>,
}

impl HeadTemplate {
    /// Construct a template from exact catalog data.
    ///
    /// The constructor validates all invariants that would otherwise be
    /// guaranteed by Java `TemplateFactory`; it never fills or clamps data.
    pub fn from_catalog_data(data: HeadTemplateData) -> Result<Self, HeadTemplateDataError> {
        let HeadTemplateData {
            shape,
            family,
            point_size,
            width,
            height,
            slim_bounds,
            anchors,
            key_points,
        } = data;
        if point_size <= 0 {
            return Err(HeadTemplateDataError::NonPositivePointSize(point_size));
        }
        if width <= 0 || height <= 0 {
            return Err(HeadTemplateDataError::NonPositiveDimensions { width, height });
        }
        validate_slim_bounds(slim_bounds, width, height)?;

        let required = shape.required_anchors();
        if anchors.len() != required.len() {
            return Err(HeadTemplateDataError::WrongAnchorCount {
                shape,
                expected: required.len(),
                actual: anchors.len(),
            });
        }
        for (ordinal, (&expected, offset)) in required.iter().zip(&anchors).enumerate() {
            if offset.anchor != expected {
                return Err(HeadTemplateDataError::AnchorOrder {
                    ordinal,
                    expected,
                    actual: offset.anchor,
                });
            }
            if !offset.dx.is_finite() || !offset.dy.is_finite() {
                return Err(HeadTemplateDataError::NonFiniteAnchor {
                    anchor: offset.anchor,
                });
            }
            rounded_offset(*offset)?;
        }

        if key_points.is_empty() {
            return Err(HeadTemplateDataError::EmptyKeyPoints);
        }
        let mut previous = None;
        for (ordinal, point) in key_points.iter().enumerate() {
            if point.x < 0 || point.x >= width || point.y < 0 || point.y >= height {
                return Err(HeadTemplateDataError::KeyPointOutOfBounds {
                    ordinal,
                    x: point.x,
                    y: point.y,
                    width,
                    height,
                });
            }
            if !point.distance.is_finite() {
                return Err(HeadTemplateDataError::NonFiniteKeyPointDistance { ordinal });
            }
            if point.distance.fract() != 0.0 {
                return Err(HeadTemplateDataError::NonIntegralKeyPointDistance {
                    ordinal,
                    distance: point.distance,
                });
            }
            let key = (point.y, point.x);
            if previous.is_some_and(|prior| prior >= key) {
                return Err(HeadTemplateDataError::KeyPointOrder { ordinal });
            }
            previous = Some(key);
        }

        Ok(Self {
            shape,
            family,
            point_size,
            width,
            height,
            slim_bounds,
            anchors,
            key_points,
        })
    }

    #[must_use]
    pub const fn shape(&self) -> HeadTemplateShape {
        self.shape
    }

    #[must_use]
    pub const fn family(&self) -> HeadTemplateFamily {
        self.family
    }

    #[must_use]
    pub const fn point_size(&self) -> i32 {
        self.point_size
    }

    #[must_use]
    pub const fn width(&self) -> i32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> i32 {
        self.height
    }

    #[must_use]
    pub const fn slim_bounds(&self) -> HeadTemplateBounds {
        self.slim_bounds
    }

    #[must_use]
    pub fn anchors(&self) -> &[HeadTemplateAnchorOffset] {
        &self.anchors
    }

    #[must_use]
    pub fn key_points(&self) -> &[HeadTemplatePixelDistance] {
        &self.key_points
    }

    /// Return Java `Template.getOffset`, including its asymmetric horizontal
    /// tie-break around the slim rectangle.
    pub fn rounded_anchor_offset(
        &self,
        anchor: HeadTemplateAnchor,
    ) -> Result<(i32, i32), HeadTemplateEvaluationError> {
        let offset = self
            .anchors
            .get(anchor as usize)
            .filter(|offset| offset.anchor == anchor)
            .copied()
            .ok_or(HeadTemplateEvaluationError::UnavailableAnchor {
                shape: self.shape,
                anchor,
            })?;
        rounded_offset(offset).map_err(HeadTemplateEvaluationError::InvalidTemplateData)
    }

    /// Pure port of Java `Template.evaluate` over the native signed Chamfer table.
    ///
    /// As in Java, key points outside the image and `VALUE_UNKNOWN` cells are
    /// ignored. Expected distances are reduced to foreground (`== 0`) versus
    /// background/hole (`!= 0`); their magnitude is not compared.
    pub fn evaluate(
        &self,
        x: i32,
        y: i32,
        anchor: Option<HeadTemplateAnchor>,
        distances: &NeutralDistanceTable,
    ) -> Result<f64, HeadTemplateEvaluationError> {
        validate_distance_table(distances)?;

        let (ul_x, ul_y) = if let Some(anchor) = anchor {
            let (dx, dy) = self.rounded_anchor_offset(anchor)?;
            (
                x.checked_sub(dx)
                    .ok_or(HeadTemplateEvaluationError::CoordinateOverflow)?,
                y.checked_sub(dy)
                    .ok_or(HeadTemplateEvaluationError::CoordinateOverflow)?,
            )
        } else {
            (x, y)
        };

        let image_width = i32::try_from(distances.width)
            .map_err(|_| HeadTemplateEvaluationError::DistanceDimensionsTooLarge)?;
        let image_height = i32::try_from(distances.height)
            .map_err(|_| HeadTemplateEvaluationError::DistanceDimensionsTooLarge)?;
        let mut weights = 0.0;
        let mut total = 0.0;

        for point in &self.key_points {
            let nx = ul_x
                .checked_add(point.x)
                .ok_or(HeadTemplateEvaluationError::CoordinateOverflow)?;
            let ny = ul_y
                .checked_add(point.y)
                .ok_or(HeadTemplateEvaluationError::CoordinateOverflow)?;

            // Java deliberately ignores template points outside the image.
            if nx < 0 || nx >= image_width || ny < 0 || ny >= image_height {
                continue;
            }

            let index = (usize::try_from(ny).expect("non-negative ordinate") * distances.width)
                + usize::try_from(nx).expect("non-negative abscissa");
            let actual_distance = distances.values[index];
            if actual_distance == VALUE_UNKNOWN {
                continue;
            }
            if actual_distance < VALUE_UNKNOWN {
                return Err(HeadTemplateEvaluationError::InvalidDistanceValue {
                    x: nx,
                    y: ny,
                    value: actual_distance,
                });
            }

            let weight = if point.distance == 0.0 {
                FOREGROUND_WEIGHT
            } else if point.distance > 0.0 {
                BACKGROUND_WEIGHT
            } else {
                HOLE_WEIGHT
            };
            let expected = if point.distance == 0.0 { 0.0 } else { 1.0 };
            let actual = if actual_distance == 0 { 0.0 } else { 1.0 };
            let difference = f64::abs(actual - expected);

            // Preserve Java's source operation order.
            total += weight * difference;
            weights += weight;
        }

        if weights == 0.0 {
            Ok(f64::MAX)
        } else {
            Ok(total / weights)
        }
    }
}

/// A complete active normal-head template catalog for one font point size.
#[derive(Clone, Debug, PartialEq)]
pub struct HeadTemplateCatalog {
    family: HeadTemplateFamily,
    point_size: i32,
    templates: Vec<HeadTemplate>,
}

impl HeadTemplateCatalog {
    /// Construct the catalog from four exact frozen records in Java factory order.
    pub fn from_catalog_data(
        family: HeadTemplateFamily,
        point_size: i32,
        templates: Vec<HeadTemplate>,
    ) -> Result<Self, HeadTemplateDataError> {
        if point_size <= 0 {
            return Err(HeadTemplateDataError::NonPositivePointSize(point_size));
        }
        if templates.len() != HeadTemplateShape::FACTORY_ORDER.len() {
            return Err(HeadTemplateDataError::WrongTemplateCount {
                expected: HeadTemplateShape::FACTORY_ORDER.len(),
                actual: templates.len(),
            });
        }
        for (ordinal, (&expected_shape, template)) in HeadTemplateShape::FACTORY_ORDER
            .iter()
            .zip(&templates)
            .enumerate()
        {
            if template.shape != expected_shape {
                return Err(HeadTemplateDataError::TemplateOrder {
                    ordinal,
                    expected: expected_shape,
                    actual: template.shape,
                });
            }
            if template.family != family || template.point_size != point_size {
                return Err(HeadTemplateDataError::TemplateCatalogMismatch {
                    ordinal,
                    expected_family: family,
                    actual_family: template.family,
                    expected_point_size: point_size,
                    actual_point_size: template.point_size,
                });
            }
        }

        Ok(Self {
            family,
            point_size,
            templates,
        })
    }

    #[must_use]
    pub const fn family(&self) -> HeadTemplateFamily {
        self.family
    }

    #[must_use]
    pub const fn point_size(&self) -> i32 {
        self.point_size
    }

    #[must_use]
    pub fn templates(&self) -> &[HeadTemplate] {
        &self.templates
    }

    #[must_use]
    pub fn template(&self, shape: HeadTemplateShape) -> &HeadTemplate {
        &self.templates[shape.factory_ordinal()]
    }
}

fn validate_slim_bounds(
    bounds: HeadTemplateBounds,
    width: i32,
    height: i32,
) -> Result<(), HeadTemplateDataError> {
    let right = bounds.x.checked_add(bounds.width);
    let bottom = bounds.y.checked_add(bounds.height);
    if bounds.x < 0
        || bounds.y < 0
        || bounds.width <= 0
        || bounds.height <= 0
        || right.is_none_or(|right| right > width)
        || bottom.is_none_or(|bottom| bottom > height)
    {
        return Err(HeadTemplateDataError::InvalidSlimBounds {
            bounds,
            template_width: width,
            template_height: height,
        });
    }
    Ok(())
}

fn rounded_offset(offset: HeadTemplateAnchorOffset) -> Result<(i32, i32), HeadTemplateDataError> {
    if !offset.dx.is_finite() || !offset.dy.is_finite() {
        return Err(HeadTemplateDataError::NonFiniteAnchor {
            anchor: offset.anchor,
        });
    }

    // Keep the exact Java expression grouping from Template.getOffset:
    // Math.round(offset - 0.5 [-/+ 0.001]).
    let x = match offset.anchor.horizontal_side() {
        HorizontalSide::Left => java_math_round(offset.dx - 0.5 - 0.001),
        HorizontalSide::Center => java_math_round(offset.dx - 0.5),
        HorizontalSide::Right => java_math_round(offset.dx - 0.5 + 0.001),
    };
    let y = java_math_round(offset.dy - 0.5);
    Ok((x, y))
}

fn java_math_round(value: f64) -> i32 {
    // Java `Math.round(double)` is floor(a + 1/2), followed by Java's
    // saturating double-to-int narrowing conversion.
    (value + 0.5).floor() as i32
}

fn validate_distance_table(
    table: &NeutralDistanceTable,
) -> Result<(), HeadTemplateEvaluationError> {
    if table.width == 0 || table.height == 0 || table.normalizer <= 0 {
        return Err(HeadTemplateEvaluationError::InvalidDistanceTable {
            width: table.width,
            height: table.height,
            normalizer: table.normalizer,
            values: table.values.len(),
        });
    }
    let expected = table
        .width
        .checked_mul(table.height)
        .ok_or(HeadTemplateEvaluationError::DistanceDimensionsTooLarge)?;
    if table.values.len() != expected {
        return Err(HeadTemplateEvaluationError::InvalidDistanceTable {
            width: table.width,
            height: table.height,
            normalizer: table.normalizer,
            values: table.values.len(),
        });
    }
    Ok(())
}

/// Invalid or incomplete frozen catalog data.
#[derive(Clone, Debug, PartialEq)]
pub enum HeadTemplateDataError {
    NonPositivePointSize(i32),
    NonPositiveDimensions {
        width: i32,
        height: i32,
    },
    InvalidSlimBounds {
        bounds: HeadTemplateBounds,
        template_width: i32,
        template_height: i32,
    },
    WrongAnchorCount {
        shape: HeadTemplateShape,
        expected: usize,
        actual: usize,
    },
    AnchorOrder {
        ordinal: usize,
        expected: HeadTemplateAnchor,
        actual: HeadTemplateAnchor,
    },
    NonFiniteAnchor {
        anchor: HeadTemplateAnchor,
    },
    EmptyKeyPoints,
    KeyPointOutOfBounds {
        ordinal: usize,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    },
    NonFiniteKeyPointDistance {
        ordinal: usize,
    },
    NonIntegralKeyPointDistance {
        ordinal: usize,
        distance: f64,
    },
    KeyPointOrder {
        ordinal: usize,
    },
    WrongTemplateCount {
        expected: usize,
        actual: usize,
    },
    TemplateOrder {
        ordinal: usize,
        expected: HeadTemplateShape,
        actual: HeadTemplateShape,
    },
    TemplateCatalogMismatch {
        ordinal: usize,
        expected_family: HeadTemplateFamily,
        actual_family: HeadTemplateFamily,
        expected_point_size: i32,
        actual_point_size: i32,
    },
}

impl fmt::Display for HeadTemplateDataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid HEADS template catalog data: {self:?}")
    }
}

impl Error for HeadTemplateDataError {}

/// Typed failure while evaluating a structurally validated template.
#[derive(Clone, Debug, PartialEq)]
pub enum HeadTemplateEvaluationError {
    InvalidTemplateData(HeadTemplateDataError),
    UnavailableAnchor {
        shape: HeadTemplateShape,
        anchor: HeadTemplateAnchor,
    },
    InvalidDistanceTable {
        width: usize,
        height: usize,
        normalizer: i32,
        values: usize,
    },
    InvalidDistanceValue {
        x: i32,
        y: i32,
        value: i32,
    },
    DistanceDimensionsTooLarge,
    CoordinateOverflow,
}

impl fmt::Display for HeadTemplateEvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "HEADS template evaluation failed: {self:?}")
    }
}

impl Error for HeadTemplateEvaluationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidTemplateData(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chula_84_anchors(shape: HeadTemplateShape) -> Vec<HeadTemplateAnchorOffset> {
        let stemmed = vec![
            HeadTemplateAnchorOffset {
                anchor: HeadTemplateAnchor::Center,
                dx: 15.5,
                dy: 14.0,
            },
            HeadTemplateAnchorOffset {
                anchor: HeadTemplateAnchor::MiddleLeft,
                dx: 3.0,
                dy: 14.0,
            },
            HeadTemplateAnchorOffset {
                anchor: HeadTemplateAnchor::MiddleRight,
                dx: 28.0,
                dy: 14.0,
            },
            HeadTemplateAnchorOffset {
                anchor: HeadTemplateAnchor::LeftStem,
                dx: 5.5,
                dy: 14.0,
            },
            HeadTemplateAnchorOffset {
                anchor: HeadTemplateAnchor::TopLeftStem,
                dx: 5.5,
                dy: 14.0,
            },
            HeadTemplateAnchorOffset {
                anchor: HeadTemplateAnchor::BottomLeftStem,
                dx: 5.5,
                dy: 20.0,
            },
            HeadTemplateAnchorOffset {
                anchor: HeadTemplateAnchor::RightStem,
                dx: 25.5,
                dy: 14.0,
            },
            HeadTemplateAnchorOffset {
                anchor: HeadTemplateAnchor::TopRightStem,
                dx: 25.5,
                dy: 8.0,
            },
            HeadTemplateAnchorOffset {
                anchor: HeadTemplateAnchor::BottomRightStem,
                dx: 25.5,
                dy: 14.0,
            },
        ];
        match shape {
            HeadTemplateShape::NoteheadBlack | HeadTemplateShape::NoteheadVoid => stemmed,
            HeadTemplateShape::WholeNote => vec![
                HeadTemplateAnchorOffset {
                    anchor: HeadTemplateAnchor::Center,
                    dx: 20.5,
                    dy: 14.0,
                },
                HeadTemplateAnchorOffset {
                    anchor: HeadTemplateAnchor::MiddleLeft,
                    dx: 3.0,
                    dy: 14.0,
                },
                HeadTemplateAnchorOffset {
                    anchor: HeadTemplateAnchor::MiddleRight,
                    dx: 38.0,
                    dy: 14.0,
                },
            ],
            HeadTemplateShape::Breve => vec![
                HeadTemplateAnchorOffset {
                    anchor: HeadTemplateAnchor::Center,
                    dx: 28.0,
                    dy: 16.0,
                },
                HeadTemplateAnchorOffset {
                    anchor: HeadTemplateAnchor::MiddleLeft,
                    dx: 3.0,
                    dy: 16.0,
                },
                HeadTemplateAnchorOffset {
                    anchor: HeadTemplateAnchor::MiddleRight,
                    dx: 53.0,
                    dy: 16.0,
                },
            ],
        }
    }

    fn chula_84_black(points: Vec<HeadTemplatePixelDistance>) -> HeadTemplate {
        HeadTemplate::from_catalog_data(HeadTemplateData {
            shape: HeadTemplateShape::NoteheadBlack,
            family: HeadTemplateFamily::Bravura,
            point_size: 84,
            width: 31,
            height: 27,
            slim_bounds: HeadTemplateBounds {
                x: 3,
                y: 4,
                width: 25,
                height: 20,
            },
            anchors: chula_84_anchors(HeadTemplateShape::NoteheadBlack),
            key_points: points,
        })
        .unwrap()
    }

    #[test]
    fn chula_catalog_geometry_pixels_and_offsets_match_frozen_java_rows() {
        let template = chula_84_black(vec![
            HeadTemplatePixelDistance {
                x: 12,
                y: 2,
                distance: f64::from_bits(0x4020_0000_0000_0000),
            },
            HeadTemplatePixelDistance {
                x: 13,
                y: 2,
                distance: f64::from_bits(0x401c_0000_0000_0000),
            },
            HeadTemplatePixelDistance {
                x: 14,
                y: 4,
                distance: 0.0,
            },
        ]);

        assert_eq!(template.shape(), HeadTemplateShape::NoteheadBlack);
        assert_eq!(template.family(), HeadTemplateFamily::Bravura);
        assert_eq!(template.point_size(), 84);
        assert_eq!((template.width(), template.height()), (31, 27));
        assert_eq!(
            template.slim_bounds(),
            HeadTemplateBounds {
                x: 3,
                y: 4,
                width: 25,
                height: 20
            }
        );
        assert_eq!(
            template.key_points()[0].distance.to_bits(),
            0x4020_0000_0000_0000
        );
        assert_eq!(
            template.key_points()[1].distance.to_bits(),
            0x401c_0000_0000_0000
        );

        let expected = [
            (15, 14),
            (2, 14),
            (28, 14),
            (5, 14),
            (5, 14),
            (5, 20),
            (25, 14),
            (25, 8),
            (25, 14),
        ];
        for (&anchor, expected) in HeadTemplateAnchor::ALL.iter().zip(expected) {
            assert_eq!(template.rounded_anchor_offset(anchor), Ok(expected));
        }
    }

    #[test]
    fn evaluator_matches_java_foreground_background_hole_unknown_and_bounds_branches() {
        // All coordinates/distances are selected from the frozen chula point-size-84
        // catalog: positive exterior, zero foreground, and negative void-hole records.
        let template = chula_84_black(vec![
            HeadTemplatePixelDistance {
                x: 12,
                y: 2,
                distance: 8.0,
            },
            HeadTemplatePixelDistance {
                x: 14,
                y: 4,
                distance: 0.0,
            },
            HeadTemplatePixelDistance {
                x: 19,
                y: 7,
                distance: -3.0,
            },
        ]);
        let mut table = NeutralDistanceTable {
            id: 1,
            width: 31,
            height: 27,
            normalizer: 3,
            values: vec![3; 31 * 27],
        };

        // Background matches, foreground mismatches, hole matches: 6 / (1+6+4).
        assert_eq!(
            template.evaluate(0, 0, None, &table).unwrap().to_bits(),
            (6.0_f64 / 11.0).to_bits()
        );

        // Flip background and hole to foreground and foreground to foreground:
        // (1+4) / (1+6+4).
        table.values[(2 * 31) + 12] = 0;
        table.values[(4 * 31) + 14] = 0;
        table.values[(7 * 31) + 19] = 0;
        assert_eq!(
            template.evaluate(0, 0, None, &table).unwrap().to_bits(),
            (5.0_f64 / 11.0).to_bits()
        );

        // UNKNOWN and out-of-image points are ignored exactly as Java does.
        table.values[(2 * 31) + 12] = VALUE_UNKNOWN;
        table.values[(4 * 31) + 14] = VALUE_UNKNOWN;
        table.values[(7 * 31) + 19] = VALUE_UNKNOWN;
        assert_eq!(template.evaluate(0, 0, None, &table), Ok(f64::MAX));
        assert_eq!(template.evaluate(-20, -10, None, &table), Ok(f64::MAX));
    }

    #[test]
    fn anchor_positioning_uses_the_java_rounded_offset() {
        let template = chula_84_black(vec![HeadTemplatePixelDistance {
            x: 14,
            y: 4,
            distance: 0.0,
        }]);
        let mut table = NeutralDistanceTable {
            id: 2,
            width: 40,
            height: 40,
            normalizer: 3,
            values: vec![3; 40 * 40],
        };
        // Pivot (20,20), CENTER offset (15,14) gives UL (5,6), point (14,4) => (19,10).
        table.values[(10 * 40) + 19] = 0;
        assert_eq!(
            template.evaluate(20, 20, Some(HeadTemplateAnchor::Center), &table),
            Ok(0.0)
        );
    }

    #[test]
    fn catalog_requires_all_four_records_in_exact_factory_order() {
        let geometry = [
            (
                HeadTemplateShape::NoteheadBlack,
                31,
                27,
                (3, 4, 25, 20),
                (12, 2),
            ),
            (
                HeadTemplateShape::NoteheadVoid,
                31,
                27,
                (3, 4, 25, 20),
                (12, 2),
            ),
            (
                HeadTemplateShape::WholeNote,
                42,
                27,
                (3, 4, 35, 20),
                (11, 2),
            ),
            (HeadTemplateShape::Breve, 57, 33, (3, 3, 50, 26), (1, 1)),
        ];
        let templates = geometry
            .into_iter()
            .map(
                |(shape, width, height, (x, y, slim_width, slim_height), (point_x, point_y))| {
                    HeadTemplate::from_catalog_data(HeadTemplateData {
                        shape,
                        family: HeadTemplateFamily::Bravura,
                        point_size: 84,
                        width,
                        height,
                        slim_bounds: HeadTemplateBounds {
                            x,
                            y,
                            width: slim_width,
                            height: slim_height,
                        },
                        anchors: chula_84_anchors(shape),
                        key_points: vec![HeadTemplatePixelDistance {
                            x: point_x,
                            y: point_y,
                            distance: 8.0,
                        }],
                    })
                    .unwrap()
                },
            )
            .collect();
        let catalog =
            HeadTemplateCatalog::from_catalog_data(HeadTemplateFamily::Bravura, 84, templates)
                .unwrap();

        assert_eq!(catalog.family(), HeadTemplateFamily::Bravura);
        assert_eq!(catalog.point_size(), 84);
        assert_eq!(catalog.templates().len(), 4);
        assert_eq!(catalog.template(HeadTemplateShape::Breve).width(), 57);
    }

    #[test]
    fn invalid_inputs_fail_with_typed_errors_instead_of_being_clamped() {
        let mut anchors = chula_84_anchors(HeadTemplateShape::NoteheadBlack);
        anchors.swap(0, 1);
        assert!(matches!(
            HeadTemplate::from_catalog_data(HeadTemplateData {
                shape: HeadTemplateShape::NoteheadBlack,
                family: HeadTemplateFamily::Bravura,
                point_size: 84,
                width: 31,
                height: 27,
                slim_bounds: HeadTemplateBounds {
                    x: 3,
                    y: 4,
                    width: 25,
                    height: 20,
                },
                anchors,
                key_points: vec![HeadTemplatePixelDistance {
                    x: 0,
                    y: 0,
                    distance: 0.0,
                }],
            }),
            Err(HeadTemplateDataError::AnchorOrder { ordinal: 0, .. })
        ));

        let template = chula_84_black(vec![HeadTemplatePixelDistance {
            x: 0,
            y: 0,
            distance: 0.0,
        }]);
        let malformed = NeutralDistanceTable {
            id: 3,
            width: 2,
            height: 2,
            normalizer: 3,
            values: vec![0; 3],
        };
        assert!(matches!(
            template.evaluate(0, 0, None, &malformed),
            Err(HeadTemplateEvaluationError::InvalidDistanceTable { .. })
        ));

        let invalid_value = NeutralDistanceTable {
            id: 4,
            width: 1,
            height: 1,
            normalizer: 3,
            values: vec![-2],
        };
        assert_eq!(
            template.evaluate(0, 0, None, &invalid_value),
            Err(HeadTemplateEvaluationError::InvalidDistanceValue {
                x: 0,
                y: 0,
                value: -2,
            })
        );
    }
}
