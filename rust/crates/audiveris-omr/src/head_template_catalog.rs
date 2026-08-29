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
const CHOPIN_CUE_ASSET: &[u8] = include_bytes!("data/bravura-head-templates-chopin-cue.bin");
const PRACTICAL_ASSET: &[u8] = include_bytes!("data/bravura-head-templates-practical.bin");
const MAGIC: &[u8; 8] = b"AVHTPL02";
const ORACLE_DIGEST: [u8; 32] = [
    0x0f, 0x1c, 0x86, 0xff, 0x8a, 0x83, 0x18, 0x52, 0x0f, 0xae, 0xeb, 0x43, 0xe3, 0x37, 0x5b, 0x5c,
    0x76, 0x30, 0x1e, 0xb1, 0x63, 0xba, 0x0c, 0xd4, 0x44, 0x2b, 0x8c, 0xcb, 0x11, 0x80, 0xf8, 0x32,
];
const CHOPIN_CUE_ORACLE_DIGEST: [u8; 32] = [
    0x00, 0x05, 0xd6, 0x5f, 0x3b, 0x0e, 0xe4, 0xdd, 0x66, 0xa9, 0x26, 0x00, 0xef, 0xd2, 0xec, 0x3e,
    0x3a, 0xf7, 0x64, 0x3c, 0x9e, 0xba, 0x56, 0xa6, 0xd2, 0x78, 0xc2, 0xc7, 0xa2, 0x06, 0xd6, 0xc2,
];
const PRACTICAL_ORACLE_DIGEST: [u8; 32] = [
    0xfb, 0xa5, 0x70, 0xde, 0x98, 0x9e, 0xcb, 0xcf, 0x5d, 0x21, 0xaf, 0x42, 0x25, 0x7b, 0x50, 0x5b,
    0x84, 0x46, 0x09, 0xf4, 0x15, 0x63, 0x0b, 0xad, 0xd2, 0x9d, 0x2e, 0x84, 0x51, 0xb6, 0x6e, 0x76,
];

/// SHA-256 of the complete fresh-JVM Java catalog oracle encoded by this asset.
pub const BRAVURA_HEAD_TEMPLATE_ORACLE_SHA256: &str =
    "0f1c86ff8a8318520faeeb43e3375b5c76301eb163ba0cd4442b8ccb1180f832";

/// Lowest exact Bravura template point size available to native HEADS.
pub const BRAVURA_HEAD_TEMPLATE_MIN_POINT_SIZE: i32 = 24;

/// Highest exact Bravura template point size available to native HEADS.
pub const BRAVURA_HEAD_TEMPLATE_MAX_POINT_SIZE: i32 = 128;

/// Every integer point size available to native HEADS.
pub const BRAVURA_HEAD_TEMPLATE_POINT_SIZES: [i32; 105] = practical_point_sizes();

/// Previously measured catalogs retained as independently frozen parity assets.
pub const BRAVURA_HEAD_TEMPLATE_PINNED_POINT_SIZES: [i32; 8] = [52, 53, 54, 78, 83, 84, 85, 87];
const BASE_POINT_SIZES: [i32; 5] = [78, 83, 84, 85, 87];
const CHOPIN_CUE_POINT_SIZES: [i32; 3] = [52, 53, 54];
const PRACTICAL_POINT_SIZES: [i32; 97] = supplemental_point_sizes();

const fn practical_point_sizes() -> [i32; 105] {
    let mut sizes = [0; 105];
    let mut index = 0;
    let mut point_size = BRAVURA_HEAD_TEMPLATE_MIN_POINT_SIZE;
    while point_size <= BRAVURA_HEAD_TEMPLATE_MAX_POINT_SIZE {
        sizes[index] = point_size;
        index += 1;
        point_size += 1;
    }
    sizes
}

const fn supplemental_point_sizes() -> [i32; 97] {
    let mut sizes = [0; 97];
    let mut index = 0;
    let mut point_size = BRAVURA_HEAD_TEMPLATE_MIN_POINT_SIZE;
    while point_size <= BRAVURA_HEAD_TEMPLATE_MAX_POINT_SIZE {
        if !matches!(point_size, 52 | 53 | 54 | 78 | 83 | 84 | 85 | 87) {
            sizes[index] = point_size;
            index += 1;
        }
        point_size += 1;
    }
    sizes
}

/// Decode the versioned checked-in catalog asset.
///
/// The asset stores Java's precise anchor `f64` bits and every signed key
/// distance. The source oracle is not read by production code.
pub fn load_bravura_head_template_catalogs()
-> Result<Vec<HeadTemplateCatalog>, HeadTemplateCatalogAssetError> {
    let mut catalogs = decode_catalogs(ASSET, ORACLE_DIGEST, &BASE_POINT_SIZES)?;
    catalogs.extend(decode_catalogs(
        CHOPIN_CUE_ASSET,
        CHOPIN_CUE_ORACLE_DIGEST,
        &CHOPIN_CUE_POINT_SIZES,
    )?);
    catalogs.extend(decode_catalogs(
        PRACTICAL_ASSET,
        PRACTICAL_ORACLE_DIGEST,
        &PRACTICAL_POINT_SIZES,
    )?);
    catalogs.sort_by_key(HeadTemplateCatalog::point_size);
    Ok(catalogs)
}

fn decode_catalogs(
    bytes: &[u8],
    expected_digest: [u8; 32],
    expected_point_sizes: &[i32],
) -> Result<Vec<HeadTemplateCatalog>, HeadTemplateCatalogAssetError> {
    let mut reader = AssetReader::new(bytes);
    if reader.array::<8>()? != *MAGIC {
        return Err(HeadTemplateCatalogAssetError::BadMagic);
    }
    let actual_digest = reader.array::<32>()?;
    if actual_digest != expected_digest {
        return Err(HeadTemplateCatalogAssetError::OracleDigestMismatch {
            expected: expected_digest,
            actual: actual_digest,
        });
    }

    let family = match reader.u8()? {
        0 => HeadTemplateFamily::Bravura,
        value => return Err(HeadTemplateCatalogAssetError::UnsupportedFamily(value)),
    };
    let catalog_count = usize::from(reader.u8()?);
    require_count("catalogs", expected_point_sizes.len(), catalog_count)?;
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
        .ne(expected_point_sizes.iter().copied())
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
        4 => Ok(HeadTemplateShape::NoteheadBlackSmall),
        5 => Ok(HeadTemplateShape::NoteheadVoidSmall),
        6 => Ok(HeadTemplateShape::WholeNoteSmall),
        7 => Ok(HeadTemplateShape::BreveSmall),
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
        assert_eq!(catalogs.len(), 105);
        assert_eq!(
            catalogs
                .iter()
                .map(HeadTemplateCatalog::point_size)
                .collect::<Vec<_>>(),
            BRAVURA_HEAD_TEMPLATE_POINT_SIZES.to_vec()
        );
        assert_eq!(
            catalogs
                .iter()
                .flat_map(HeadTemplateCatalog::templates)
                .count(),
            840
        );
        assert_eq!(
            catalogs
                .iter()
                .flat_map(HeadTemplateCatalog::templates)
                .flat_map(HeadTemplate::key_points)
                .count(),
            534_390
        );
    }

    #[test]
    fn practical_asset_keeps_the_frozen_parity_catalogs_separate() {
        assert!(
            PRACTICAL_POINT_SIZES
                .iter()
                .all(|point_size| !BRAVURA_HEAD_TEMPLATE_PINNED_POINT_SIZES.contains(point_size))
        );
        assert_eq!(
            PRACTICAL_POINT_SIZES.len() + BASE_POINT_SIZES.len() + CHOPIN_CUE_POINT_SIZES.len(),
            105
        );
    }

    #[test]
    fn decoder_rejects_truncation_and_trailing_data() {
        assert!(matches!(
            decode_catalogs(&ASSET[..ASSET.len() - 1], ORACLE_DIGEST, &BASE_POINT_SIZES),
            Err(HeadTemplateCatalogAssetError::Truncated { .. })
        ));
        let mut extended = ASSET.to_vec();
        extended.push(0);
        assert!(matches!(
            decode_catalogs(&extended, ORACLE_DIGEST, &BASE_POINT_SIZES),
            Err(HeadTemplateCatalogAssetError::TrailingBytes { .. })
        ));
    }
}
