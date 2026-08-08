// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact checked-in Bravura catalogs used by the measured HEADS pipeline.

use std::error::Error;
use std::fmt;

use crate::head_template::{
    HeadTemplate, HeadTemplateAnchor, HeadTemplateAnchorOffset, HeadTemplateBounds,
    HeadTemplateCatalog, HeadTemplateData, HeadTemplateDataError, HeadTemplateFamily,
    HeadTemplatePixelDistance, HeadTemplateShape,
};

const ASSET: &[u8] = include_bytes!("data/bravura-head-templates.bin");
const MAGIC: &[u8; 8] = b"AVHTPL01";
const ORACLE_DIGEST: [u8; 32] = [
    0x84, 0xc3, 0x92, 0x08, 0x89, 0x15, 0x30, 0x96, 0x5f, 0x5d, 0x9c, 0xe7, 0x1f, 0xf9, 0xb7, 0x9c,
    0xf3, 0x73, 0xc1, 0x01, 0xf4, 0xda, 0x80, 0x36, 0x05, 0x9c, 0xdb, 0xf2, 0x5e, 0x2a, 0x2e, 0xa6,
];

/// SHA-256 of the complete fresh-JVM Java catalog oracle encoded by this asset.
pub const BRAVURA_HEAD_TEMPLATE_ORACLE_SHA256: &str =
    "84c39208891530965f5d9ce71ff9b79cf373c101f4da8036059cdbf25e2a2ea6";

/// The exact point sizes selected by the measured normal-staff corpus.
pub const BRAVURA_HEAD_TEMPLATE_POINT_SIZES: [i32; 5] = [78, 83, 84, 85, 87];

/// Decode the versioned checked-in catalog asset.
///
/// The asset stores Java's precise anchor `f64` bits and every signed key
/// distance. The source oracle is not read by production code.
pub fn load_bravura_head_template_catalogs()
-> Result<Vec<HeadTemplateCatalog>, HeadTemplateCatalogAssetError> {
    decode_catalogs(ASSET)
}

fn decode_catalogs(
    bytes: &[u8],
) -> Result<Vec<HeadTemplateCatalog>, HeadTemplateCatalogAssetError> {
    let mut reader = AssetReader::new(bytes);
    if reader.array::<8>()? != *MAGIC {
        return Err(HeadTemplateCatalogAssetError::BadMagic);
    }
    let actual_digest = reader.array::<32>()?;
    if actual_digest != ORACLE_DIGEST {
        return Err(HeadTemplateCatalogAssetError::OracleDigestMismatch {
            expected: ORACLE_DIGEST,
            actual: actual_digest,
        });
    }

    let family = match reader.u8()? {
        0 => HeadTemplateFamily::Bravura,
        value => return Err(HeadTemplateCatalogAssetError::UnsupportedFamily(value)),
    };
    let catalog_count = usize::from(reader.u8()?);
    require_count(
        "catalogs",
        BRAVURA_HEAD_TEMPLATE_POINT_SIZES.len(),
        catalog_count,
    )?;
    let mut catalogs = Vec::with_capacity(catalog_count);
    let mut prior_point_size = None;

    for catalog_ordinal in 0..catalog_count {
        let point_size = i32::from(reader.i16()?);
        if prior_point_size.is_some_and(|prior| prior >= point_size) {
            return Err(HeadTemplateCatalogAssetError::CatalogOrder {
                catalog_ordinal,
                prior_point_size,
                point_size,
            });
        }
        prior_point_size = Some(point_size);

        let template_count = usize::from(reader.u8()?);
        require_count(
            "templates",
            HeadTemplateShape::FACTORY_ORDER.len(),
            template_count,
        )?;
        let mut templates = Vec::with_capacity(template_count);
        for _ in 0..template_count {
            let shape = decode_shape(reader.u8()?)?;
            let width = i32::from(reader.i16()?);
            let height = i32::from(reader.i16()?);
            let slim_bounds = HeadTemplateBounds {
                x: i32::from(reader.i16()?),
                y: i32::from(reader.i16()?),
                width: i32::from(reader.i16()?),
                height: i32::from(reader.i16()?),
            };

            let anchor_count = usize::from(reader.u8()?);
            require_count("anchors", shape.required_anchors().len(), anchor_count)?;
            let mut anchors = Vec::with_capacity(anchor_count);
            for _ in 0..anchor_count {
                anchors.push(HeadTemplateAnchorOffset {
                    anchor: decode_anchor(reader.u8()?)?,
                    dx: f64::from_bits(reader.u64()?),
                    dy: f64::from_bits(reader.u64()?),
                });
            }

            let pixel_count = usize::try_from(reader.u32()?)
                .map_err(|_| HeadTemplateCatalogAssetError::CountTooLarge)?;
            let maximum_pixels = width
                .checked_mul(height)
                .and_then(|area| usize::try_from(area).ok())
                .ok_or(HeadTemplateCatalogAssetError::CountTooLarge)?;
            if pixel_count == 0 || pixel_count > maximum_pixels {
                return Err(HeadTemplateCatalogAssetError::CountOutOfRange {
                    context: "pixels",
                    count: pixel_count,
                    maximum: maximum_pixels,
                });
            }
            let mut key_points = Vec::with_capacity(pixel_count);
            for _ in 0..pixel_count {
                key_points.push(HeadTemplatePixelDistance {
                    x: i32::from(reader.i16()?),
                    y: i32::from(reader.i16()?),
                    distance: f64::from(reader.i16()?),
                });
            }

            templates.push(HeadTemplate::from_catalog_data(HeadTemplateData {
                shape,
                family,
                point_size,
                width,
                height,
                slim_bounds,
                anchors,
                key_points,
            })?);
        }
        catalogs.push(HeadTemplateCatalog::from_catalog_data(
            family, point_size, templates,
        )?);
    }

    if reader.remaining() != 0 {
        return Err(HeadTemplateCatalogAssetError::TrailingBytes {
            offset: reader.offset,
            remaining: reader.remaining(),
        });
    }
    if catalogs
        .iter()
        .map(HeadTemplateCatalog::point_size)
        .ne(BRAVURA_HEAD_TEMPLATE_POINT_SIZES)
    {
        return Err(HeadTemplateCatalogAssetError::UnexpectedPointSizes(
            catalogs
                .iter()
                .map(HeadTemplateCatalog::point_size)
                .collect(),
        ));
    }
    Ok(catalogs)
}

fn require_count(
    context: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), HeadTemplateCatalogAssetError> {
    if actual == expected {
        Ok(())
    } else {
        Err(HeadTemplateCatalogAssetError::UnexpectedCount {
            context,
            expected,
            actual,
        })
    }
}

fn decode_shape(value: u8) -> Result<HeadTemplateShape, HeadTemplateCatalogAssetError> {
    match value {
        0 => Ok(HeadTemplateShape::NoteheadBlack),
        1 => Ok(HeadTemplateShape::NoteheadVoid),
        2 => Ok(HeadTemplateShape::WholeNote),
        3 => Ok(HeadTemplateShape::Breve),
        value => Err(HeadTemplateCatalogAssetError::UnsupportedShape(value)),
    }
}

fn decode_anchor(value: u8) -> Result<HeadTemplateAnchor, HeadTemplateCatalogAssetError> {
    match value {
        0 => Ok(HeadTemplateAnchor::Center),
        1 => Ok(HeadTemplateAnchor::MiddleLeft),
        2 => Ok(HeadTemplateAnchor::MiddleRight),
        3 => Ok(HeadTemplateAnchor::LeftStem),
        4 => Ok(HeadTemplateAnchor::TopLeftStem),
        5 => Ok(HeadTemplateAnchor::BottomLeftStem),
        6 => Ok(HeadTemplateAnchor::RightStem),
        7 => Ok(HeadTemplateAnchor::TopRightStem),
        8 => Ok(HeadTemplateAnchor::BottomRightStem),
        value => Err(HeadTemplateCatalogAssetError::UnsupportedAnchor(value)),
    }
}

struct AssetReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> AssetReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], HeadTemplateCatalogAssetError> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(HeadTemplateCatalogAssetError::CountTooLarge)?;
        let slice =
            self.bytes
                .get(self.offset..end)
                .ok_or(HeadTemplateCatalogAssetError::Truncated {
                    offset: self.offset,
                    needed: N,
                    remaining: self.remaining(),
                })?;
        self.offset = end;
        Ok(slice.try_into().expect("slice length was checked"))
    }

    fn u8(&mut self) -> Result<u8, HeadTemplateCatalogAssetError> {
        Ok(self.array::<1>()?[0])
    }

    fn i16(&mut self) -> Result<i16, HeadTemplateCatalogAssetError> {
        Ok(i16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, HeadTemplateCatalogAssetError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, HeadTemplateCatalogAssetError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }
}

/// Invalid checked-in native catalog data.
#[derive(Clone, Debug, PartialEq)]
pub enum HeadTemplateCatalogAssetError {
    BadMagic,
    OracleDigestMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
    UnsupportedFamily(u8),
    UnsupportedShape(u8),
    UnsupportedAnchor(u8),
    Truncated {
        offset: usize,
        needed: usize,
        remaining: usize,
    },
    TrailingBytes {
        offset: usize,
        remaining: usize,
    },
    CatalogOrder {
        catalog_ordinal: usize,
        prior_point_size: Option<i32>,
        point_size: i32,
    },
    UnexpectedPointSizes(Vec<i32>),
    UnexpectedCount {
        context: &'static str,
        expected: usize,
        actual: usize,
    },
    CountOutOfRange {
        context: &'static str,
        count: usize,
        maximum: usize,
    },
    CountTooLarge,
    InvalidTemplateData(HeadTemplateDataError),
}

impl From<HeadTemplateDataError> for HeadTemplateCatalogAssetError {
    fn from(error: HeadTemplateDataError) -> Self {
        Self::InvalidTemplateData(error)
    }
}

impl fmt::Display for HeadTemplateCatalogAssetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid native HEADS template asset: {self:?}")
    }
}

impl Error for HeadTemplateCatalogAssetError {
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

    #[test]
    fn checked_in_asset_has_the_complete_active_catalog_set() {
        let catalogs = load_bravura_head_template_catalogs().unwrap();
        assert_eq!(catalogs.len(), 5);
        assert_eq!(
            catalogs
                .iter()
                .map(HeadTemplateCatalog::point_size)
                .collect::<Vec<_>>(),
            BRAVURA_HEAD_TEMPLATE_POINT_SIZES
        );
        assert_eq!(
            catalogs
                .iter()
                .flat_map(HeadTemplateCatalog::templates)
                .count(),
            20
        );
        assert_eq!(
            catalogs
                .iter()
                .flat_map(HeadTemplateCatalog::templates)
                .flat_map(HeadTemplate::key_points)
                .count(),
            17_094
        );
    }

    #[test]
    fn decoder_rejects_truncation_and_trailing_data() {
        assert!(matches!(
            decode_catalogs(&ASSET[..ASSET.len() - 1]),
            Err(HeadTemplateCatalogAssetError::Truncated { .. })
        ));
        let mut extended = ASSET.to_vec();
        extended.push(0);
        assert!(matches!(
            decode_catalogs(&extended),
            Err(HeadTemplateCatalogAssetError::TrailingBytes { .. })
        ));
    }
}
