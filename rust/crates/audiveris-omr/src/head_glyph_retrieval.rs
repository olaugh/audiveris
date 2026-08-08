// SPDX-License-Identifier: AGPL-3.0-or-later

//! Pure kernel for Java `HeadInter.retrieveGlyph`.
//!
//! The provisional inter bounds are the template's slim bounds in sheet
//! coordinates. Java expands them back to the full template box, retains only
//! factory key points whose expected distance is exactly zero and whose source
//! BINARY pixel is foreground, crops those points inclusively, and constructs
//! a vertical run table. This module stops before glyph-index registration.

use std::{error::Error, fmt};

use audiveris_image::run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable, RunTableError};

use crate::head_template::{HeadTemplate, HeadTemplateBounds};

/// Original row-major BINARY raster and provisional head geometry.
#[derive(Clone, Copy, Debug)]
pub struct HeadGlyphRetrievalInput<'a> {
    pub template: &'a HeadTemplate,
    pub slim_inter_bounds: HeadTemplateBounds,
    pub raster_width: usize,
    pub raster_height: usize,
    pub binary: &'a [u8],
}

/// Identity-free fixed glyph produced before Java `GlyphIndex.registerOriginal`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetrievedHeadGlyph {
    /// Java `Template.getBounds(interBox)` in sheet coordinates.
    pub template_bounds: HeadTemplateBounds,
    /// Inclusive `PointUtil.boundsOf` result, relative to `template_bounds`.
    pub foreground_bounds: HeadTemplateBounds,
    /// Positioned cropped glyph bounds in sheet coordinates.
    pub glyph_bounds: HeadTemplateBounds,
    pub weight: usize,
    /// Java-oracle FNV-1a-64 over orientation, dimensions, and every run.
    pub run_digest: u64,
    pub run_table: RunTable,
    /// `HeadInter.retrieveGlyph` replaces the provisional bounds with these.
    pub new_inter_bounds: HeadTemplateBounds,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeadGlyphRetrievalError {
    EmptyRaster,
    RasterDimensionsTooLarge { width: usize, height: usize },
    RasterAreaOverflow,
    InvalidRasterLength { expected: usize, actual: usize },
    RunTable(RunTableError),
}

impl fmt::Display for HeadGlyphRetrievalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRaster => write!(formatter, "head-glyph BINARY raster is empty"),
            Self::RasterDimensionsTooLarge { width, height } => write!(
                formatter,
                "head-glyph BINARY raster {width}x{height} exceeds Java int dimensions"
            ),
            Self::RasterAreaOverflow => write!(formatter, "head-glyph BINARY raster area overflow"),
            Self::InvalidRasterLength { expected, actual } => write!(
                formatter,
                "head-glyph BINARY raster has {actual} pixels, expected {expected}"
            ),
            Self::RunTable(error) => write!(formatter, "head-glyph run table: {error}"),
        }
    }
}

impl Error for HeadGlyphRetrievalError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RunTable(error) => Some(error),
            Self::EmptyRaster
            | Self::RasterDimensionsTooLarge { .. }
            | Self::RasterAreaOverflow
            | Self::InvalidRasterLength { .. } => None,
        }
    }
}

impl From<RunTableError> for HeadGlyphRetrievalError {
    fn from(value: RunTableError) -> Self {
        Self::RunTable(value)
    }
}

/// Port Java `HeadInter.retrieveGlyph` through fixed-glyph construction.
///
/// `Ok(None)` is Java's `null` result when no expected template-foreground
/// point lands on a foreground BINARY pixel. All additions and subtractions at
/// sheet-coordinate boundaries use Java `int` wrapping.
pub fn retrieve_head_glyph(
    input: HeadGlyphRetrievalInput<'_>,
) -> Result<Option<RetrievedHeadGlyph>, HeadGlyphRetrievalError> {
    if input.raster_width == 0 || input.raster_height == 0 {
        return Err(HeadGlyphRetrievalError::EmptyRaster);
    }
    let raster_width = i32::try_from(input.raster_width).map_err(|_| {
        HeadGlyphRetrievalError::RasterDimensionsTooLarge {
            width: input.raster_width,
            height: input.raster_height,
        }
    })?;
    let raster_height = i32::try_from(input.raster_height).map_err(|_| {
        HeadGlyphRetrievalError::RasterDimensionsTooLarge {
            width: input.raster_width,
            height: input.raster_height,
        }
    })?;
    let expected = input
        .raster_width
        .checked_mul(input.raster_height)
        .ok_or(HeadGlyphRetrievalError::RasterAreaOverflow)?;
    if input.binary.len() != expected {
        return Err(HeadGlyphRetrievalError::InvalidRasterLength {
            expected,
            actual: input.binary.len(),
        });
    }

    let template_slim = input.template.slim_bounds();
    let template_bounds = HeadTemplateBounds {
        x: input.slim_inter_bounds.x.wrapping_sub(template_slim.x),
        y: input.slim_inter_bounds.y.wrapping_sub(template_slim.y),
        width: input.template.width(),
        height: input.template.height(),
    };

    let mut foreground = Vec::new();
    for point in input.template.key_points() {
        if point.distance != 0.0 {
            continue;
        }
        let x = template_bounds.x.wrapping_add(point.x);
        let y = template_bounds.y.wrapping_add(point.y);
        if x < 0 || x >= raster_width || y < 0 || y >= raster_height {
            continue;
        }
        let index = (usize::try_from(y).expect("non-negative ordinate") * input.raster_width)
            + usize::try_from(x).expect("non-negative abscissa");
        if input.binary[index] == FOREGROUND {
            // Java stores template-relative key-point coordinates here.
            foreground.push((point.x, point.y));
        }
    }
    let Some(&(first_x, first_y)) = foreground.first() else {
        return Ok(None);
    };

    let mut minimum_x = first_x;
    let mut maximum_x = first_x;
    let mut minimum_y = first_y;
    let mut maximum_y = first_y;
    for &(x, y) in &foreground[1..] {
        minimum_x = minimum_x.min(x);
        maximum_x = maximum_x.max(x);
        minimum_y = minimum_y.min(y);
        maximum_y = maximum_y.max(y);
    }
    // Template points are validated inside positive Java-int dimensions, so
    // these inclusive PointUtil dimensions cannot overflow.
    let foreground_bounds = HeadTemplateBounds {
        x: minimum_x,
        y: minimum_y,
        width: maximum_x - minimum_x + 1,
        height: maximum_y - minimum_y + 1,
    };
    let cropped_width = usize::try_from(foreground_bounds.width).expect("positive point width");
    let cropped_height = usize::try_from(foreground_bounds.height).expect("positive point height");
    let mut cropped = vec![BACKGROUND; cropped_width * cropped_height];
    for (x, y) in foreground {
        let relative_x = usize::try_from(x - minimum_x).expect("cropped x");
        let relative_y = usize::try_from(y - minimum_y).expect("cropped y");
        cropped[(relative_y * cropped_width) + relative_x] = FOREGROUND;
    }
    let run_table = RunTable::from_pixels(
        Orientation::Vertical,
        cropped_width,
        cropped_height,
        &cropped,
    )?;
    let glyph_bounds = HeadTemplateBounds {
        x: template_bounds.x.wrapping_add(foreground_bounds.x),
        y: template_bounds.y.wrapping_add(foreground_bounds.y),
        width: foreground_bounds.width,
        height: foreground_bounds.height,
    };
    let weight = run_table.weight();
    let run_digest = run_table_digest(&run_table);

    Ok(Some(RetrievedHeadGlyph {
        template_bounds,
        foreground_bounds,
        glyph_bounds,
        weight,
        run_digest,
        run_table,
        new_inter_bounds: glyph_bounds,
    }))
}

fn run_table_digest(table: &RunTable) -> u64 {
    let orientation = match table.orientation() {
        Orientation::Horizontal => "HORIZONTAL",
        Orientation::Vertical => "VERTICAL",
    };
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash_row(
        &mut hash,
        &format!("{orientation} {} {}", table.width(), table.height()),
    );
    for sequence in 0..table.sequence_count() {
        let mut row = sequence.to_string();
        for run in table.sequence(sequence).unwrap_or_default() {
            row.push_str(&format!(" {}:{}", run.start, run.length));
        }
        hash_row(&mut hash, &row);
    }
    hash
}

fn hash_row(hash: &mut u64, row: &str) {
    for byte in row.bytes().chain(std::iter::once(b'\n')) {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x100_0000_01b3);
    }
}

#[cfg(test)]
mod tests {
    use crate::head_template::{
        HeadTemplateAnchor, HeadTemplateAnchorOffset, HeadTemplateData, HeadTemplateFamily,
        HeadTemplatePixelDistance, HeadTemplateShape,
    };

    use super::*;

    #[test]
    fn retrieves_edge_points_in_factory_order_as_minimal_vertical_runs() {
        let template = template(vec![
            point(0, 0, 0.0),
            point(2, 0, 1.0),
            point(4, 0, 0.0),
            point(1, 1, 0.0),
            point(1, 2, 0.0),
            point(4, 3, 0.0),
        ]);
        let mut binary = vec![BACKGROUND; 20 * 30];
        for (x, y) in [(10, 20), (14, 20), (11, 21), (11, 22), (14, 23)] {
            binary[(y * 20) + x] = FOREGROUND;
        }

        let glyph = retrieve_head_glyph(HeadGlyphRetrievalInput {
            template: &template,
            slim_inter_bounds: bounds(11, 21, 3, 2),
            raster_width: 20,
            raster_height: 30,
            binary: &binary,
        })
        .unwrap()
        .unwrap();

        assert_eq!(glyph.template_bounds, bounds(10, 20, 5, 4));
        assert_eq!(glyph.foreground_bounds, bounds(0, 0, 5, 4));
        assert_eq!(glyph.glyph_bounds, bounds(10, 20, 5, 4));
        assert_eq!(glyph.new_inter_bounds, glyph.glyph_bounds);
        assert_eq!(glyph.weight, 5);
        assert_eq!(glyph.run_table.orientation(), Orientation::Vertical);
        assert_eq!(
            glyph.run_table.sequence(0).unwrap(),
            &[audiveris_image::run_table::Run::new(0, 1)]
        );
        assert_eq!(
            glyph.run_table.sequence(1).unwrap(),
            &[audiveris_image::run_table::Run::new(1, 2)]
        );
        assert!(glyph.run_table.sequence(2).unwrap().is_empty());
        assert!(glyph.run_table.sequence(3).unwrap().is_empty());
        assert_eq!(
            glyph.run_table.sequence(4).unwrap(),
            &[
                audiveris_image::run_table::Run::new(0, 1),
                audiveris_image::run_table::Run::new(3, 1),
            ]
        );
        assert_eq!(glyph.run_digest, 0x4e74_97e8_1fd6_1879);
    }

    #[test]
    fn crops_inclusive_point_bounds_and_repositions_the_inter() {
        let template = template(vec![point(1, 1, 0.0), point(2, 2, 0.0)]);
        let mut binary = vec![BACKGROUND; 8 * 8];
        binary[(3 * 8) + 3] = FOREGROUND;
        binary[(4 * 8) + 4] = FOREGROUND;

        let glyph = retrieve_head_glyph(HeadGlyphRetrievalInput {
            template: &template,
            slim_inter_bounds: bounds(3, 3, 3, 2),
            raster_width: 8,
            raster_height: 8,
            binary: &binary,
        })
        .unwrap()
        .unwrap();

        assert_eq!(glyph.template_bounds, bounds(2, 2, 5, 4));
        assert_eq!(glyph.foreground_bounds, bounds(1, 1, 2, 2));
        assert_eq!(glyph.glyph_bounds, bounds(3, 3, 2, 2));
        assert_eq!(
            glyph.run_table.to_pixels(),
            vec![FOREGROUND, BACKGROUND, BACKGROUND, FOREGROUND]
        );
    }

    #[test]
    fn returns_none_when_no_expected_foreground_point_hits_binary_foreground() {
        let template = template(vec![point(0, 0, 0.0), point(1, 0, 2.0)]);
        let mut binary = vec![BACKGROUND; 4 * 4];
        binary[1] = FOREGROUND; // Non-zero-distance key points are ignored.
        assert_eq!(
            retrieve_head_glyph(HeadGlyphRetrievalInput {
                template: &template,
                slim_inter_bounds: bounds(1, 1, 3, 2),
                raster_width: 4,
                raster_height: 4,
                binary: &binary,
            }),
            Ok(None)
        );
    }

    #[test]
    fn java_wrapping_template_and_key_point_additions_do_not_escape_the_raster() {
        let template = template(vec![point(0, 0, 0.0), point(1, 0, 0.0)]);
        let binary = vec![FOREGROUND; 2];
        assert_eq!(
            retrieve_head_glyph(HeadGlyphRetrievalInput {
                template: &template,
                slim_inter_bounds: bounds(i32::MIN, 1, 3, 2),
                raster_width: 2,
                raster_height: 1,
                binary: &binary,
            }),
            Ok(None)
        );
    }

    #[test]
    fn malformed_raster_is_rejected_before_lookup() {
        let template = template(vec![point(0, 0, 0.0)]);
        assert_eq!(
            retrieve_head_glyph(HeadGlyphRetrievalInput {
                template: &template,
                slim_inter_bounds: bounds(0, 0, 3, 2),
                raster_width: 2,
                raster_height: 2,
                binary: &[FOREGROUND; 3],
            }),
            Err(HeadGlyphRetrievalError::InvalidRasterLength {
                expected: 4,
                actual: 3,
            })
        );
    }

    fn template(key_points: Vec<HeadTemplatePixelDistance>) -> HeadTemplate {
        HeadTemplate::from_catalog_data(HeadTemplateData {
            shape: HeadTemplateShape::NoteheadBlack,
            family: HeadTemplateFamily::Bravura,
            point_size: 10,
            width: 5,
            height: 4,
            slim_bounds: bounds(1, 1, 3, 2),
            anchors: HeadTemplateAnchor::ALL
                .into_iter()
                .map(|anchor| HeadTemplateAnchorOffset {
                    anchor,
                    dx: 0.0,
                    dy: 0.0,
                })
                .collect(),
            key_points,
        })
        .unwrap()
    }

    const fn point(x: i32, y: i32, distance: f64) -> HeadTemplatePixelDistance {
        HeadTemplatePixelDistance { x, y, distance }
    }

    const fn bounds(x: i32, y: i32, width: i32, height: i32) -> HeadTemplateBounds {
        HeadTemplateBounds {
            x,
            y,
            width,
            height,
        }
    }
}
