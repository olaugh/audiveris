// SPDX-License-Identifier: AGPL-3.0-or-later

//! Grayscale morphology: the structuring element, and the closing that BEAMS
//! starts from.
//!
//! Ported from Audiveris's `image/StructureElement.java` and
//! `image/MorphoProcessor.java`, which are in turn a vendored copy of an ImageJ
//! plugin. Only the circular element and `close` are here, because those are
//! the only things Audiveris asks for: `SpotsBuilder` and `BlackHeadSizer` both
//! build `new StructureElement(0, 1, radius, {0, 0})` and call `close`. The
//! other element shapes, and the histogram-based fast variants (`fclose`,
//! `fopen`), are unreachable from Audiveris and are not ported -- porting code
//! no oracle can grade is how a port acquires bugs it cannot see.
//!
//! The arithmetic below looks stranger than it is. `getMinMax` adds 255 to
//! every sample before taking a maximum and then masks the result to a byte,
//! and the caller subtracts 255 again; the two cancel exactly, and what comes
//! out is an ordinary dilation. It is written the long way here because that is
//! how Java writes it, and because the cancellation depends on the padding
//! values -- an out-of-bounds sample contributes 0 to a dilation and 255 to an
//! erosion, which is what makes closing shrink nothing at the page edge.

/// Audiveris's `MorphoConstants.CIRCLE`. The only shape it constructs.
const CIRCLE: i32 = 0;

/// `MorphoConstants.cor3`, a fudge term in the circular distance metric.
fn cor3() -> f64 {
    1.5 - 3.0_f64.sqrt()
}

/// Which side of `getMinMax` is wanted.
///
/// Java passes an `int` and tests it twice per sample inside the inner loop.
/// The tests are loop-invariant, so they are hoisted here into the type; the
/// values computed are identical.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    Dilate,
    Erode,
}

/// A structuring element: a small mask, plus the offset list `close` reads.
pub struct StructureElement {
    kind: i32,
    width: usize,
    height: usize,
    mask: Vec<i32>,
    /// Per set cell: row offset, column offset, mask value, rounded distance.
    ///
    /// The fourth column is unused by every operator ported here. It is built
    /// anyway because it is cheap and because it is part of what the oracle
    /// pins -- a vector that is right except for a column is a vector whose
    /// next reader gets a surprise.
    vect: Vec<[i32; 4]>,
}

impl StructureElement {
    /// Builds the circular element, as `new StructureElement(CIRCLE, shift, radius, offset)`.
    ///
    /// `radius` is an `f32` because Audiveris computes it as one --
    /// `(float) (diameter - 1) / 2` -- and then widens it. Taking an `f64`
    /// here would silently accept a value Java cannot produce and give a
    /// different mask for it.
    #[must_use]
    pub fn circle(shift: i32, radius: f32, offset: [i32; 2]) -> Self {
        let radius = f64::from(radius);

        // Java's guard against an offset that would push the disk off its own
        // mask. With the {0, 0} offset Audiveris always passes, it fires only
        // for a non-positive radius.
        let offset = if ((offset[0] * offset[0] + offset[1] * offset[1]) as f64) >= radius / 2.0 {
            [0, 0]
        } else {
            offset
        };

        let width = ((radius + f64::from(shift) + 0.5) as usize) * 2 + 1;
        let height = width;

        let mut mask = vec![0; width * width];
        // Always integral: `width` is odd, so `width / 2.0 - 0.5` is a whole
        // number, and the loops below run exactly `width` times.
        let r = (width as f64 / 2.0) - 0.5;
        let r2 = radius * radius + 1.0;

        // x outer, y inner, with a running index -- so the mask is stored
        // transposed with respect to the row-major convention everywhere else
        // in this crate. That is Java's, and it is invisible for the symmetric
        // masks Audiveris builds, but reproducing it costs nothing and keeps
        // the vector below in Java's order.
        let mut index = 0;
        let mut x = -r;
        while x <= r {
            let mut y = -r;
            while y <= r {
                let dx = x - f64::from(offset[0]);
                let dy = y - f64::from(offset[1]);
                if dx * dx + dy * dy < r2 {
                    mask[index] = 255;
                }
                index += 1;
                y += 1.0;
            }
            x += 1.0;
        }

        let vect = calc_vect(&mask, width, CIRCLE);

        Self {
            kind: CIRCLE,
            width,
            height,
            mask,
            vect,
        }
    }

    /// The element's width, `getWidth`.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// The element's height, `getHeight`.
    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// The element's shape constant, `getType`.
    #[must_use]
    pub fn kind(&self) -> i32 {
        self.kind
    }

    /// The mask, `getMask`.
    #[must_use]
    pub fn mask(&self) -> &[i32] {
        &self.mask
    }

    /// The offset list, `getVect`.
    #[must_use]
    pub fn vect(&self) -> &[[i32; 4]] {
        &self.vect
    }

    /// The count of set cells, `getArea`.
    #[must_use]
    pub fn area(&self) -> usize {
        self.mask.iter().filter(|&&value| value != 0).count()
    }
}

/// `StructureElement.calcVect`: the set cells, as offsets from the centre.
fn calc_vect(mask: &[i32], width: usize, kind: i32) -> Vec<[i32; 4]> {
    let height = mask.len() / width;
    let p = (height / 2) as i32;
    let q = (width / 2) as i32;

    let mut vect = Vec::new();
    for (c, &value) in mask.iter().enumerate() {
        if value <= 0 {
            continue;
        }
        let row = (c / width) as i32 - p;
        let column = (c % width) as i32 - q;
        // Java passes {column, row}, in that order, to a metric that squares
        // both -- so the swap is invisible for the circular case and would not
        // be for the others. Kept as written.
        let distance = distance([column, row], kind);
        vect.push([row, column, value, round_half_up(distance)]);
    }
    vect
}

/// `StructureElement.getDistance`, for the shapes reachable from here.
fn distance(point: [i32; 2], kind: i32) -> f64 {
    let squared = f64::from(point[0] * point[0] + point[1] * point[1]);
    if kind == CIRCLE {
        (squared + 1.0).sqrt() + cor3()
    } else {
        squared.sqrt()
    }
}

/// `Math.round(double)`: floor of the value plus a half, not ties-to-even.
fn round_half_up(value: f64) -> i32 {
    (value + 0.5).floor() as i32
}

/// The closing operator, as `MorphoProcessor`.
pub struct MorphoProcessor {
    se: StructureElement,
}

impl MorphoProcessor {
    /// Wraps an element.
    ///
    /// Java's constructor also derives two gradient elements here. They feed
    /// only `fclose` and `fopen`, which nothing calls, so they are omitted.
    #[must_use]
    pub fn new(se: StructureElement) -> Self {
        Self { se }
    }

    /// The element in use.
    #[must_use]
    pub fn structure_element(&self) -> &StructureElement {
        &self.se
    }

    /// Grayscale closing in place: dilation, then erosion.
    ///
    /// # Panics
    ///
    /// Panics if `pixels` is not exactly `width * height` bytes.
    pub fn close(&self, pixels: &mut [u8], width: usize, height: usize) {
        assert_eq!(pixels.len(), width * height, "buffer is not width * height");

        let pg = &self.se.vect;
        let w = self.se.width as isize;
        let h = self.se.height as isize;
        let length = pixels.len() as isize;
        let stride = width as isize;

        let mut dilated = vec![0_u8; pixels.len()];
        let mut eroded = vec![0_u8; pixels.len()];

        // Java interleaves the two passes, running the erosion a whole element
        // behind the dilation -- far enough back that everything the erosion
        // reads has already been written. Reproduced rather than simplified
        // into two sequential passes: the lag is what makes the two loops below
        // cover every index exactly once between them, and that is easier to
        // check against Java as written than as argued about.
        for row in 1..=height as isize {
            for col in 0..stride {
                let index = (row - 1) * stride + col;
                if index < length {
                    let max = min_max(index, width, height, pixels, pg, Op::Dilate)[1] - 255;
                    dilated[index as usize] = max as u8;
                }

                let index2 = (row - h - 1) * stride + col - w;
                if (0..length).contains(&index2) {
                    let min = min_max(index2, width, height, &dilated, pg, Op::Erode)[0] + 255;
                    eroded[index2 as usize] = min as u8;
                }
            }
        }

        // The tail the lag left behind.
        for row in height as isize..=(height as isize + h) {
            for col in 0..(stride + w) {
                let index2 = (row - h - 1) * stride + col - w;
                if (0..length).contains(&index2) {
                    let min = min_max(index2, width, height, &dilated, pg, Op::Erode)[0] + 255;
                    eroded[index2 as usize] = min as u8;
                }
            }
        }

        pixels.copy_from_slice(&eroded);
    }
}

/// `MorphoProcessor.getMinMax`: the extremes of one element-sized window.
///
/// Returns `[min, max]`, each already masked to a byte the way Java masks them.
/// Only one of the two is ever read by the caller, but both are computed --
/// dropping the unread one would be a change in behaviour if the masking ever
/// stopped cancelling, and it costs a comparison.
fn min_max(
    index: isize,
    width: usize,
    height: usize,
    pixels: &[u8],
    pg: &[[i32; 4]],
    op: Op,
) -> [i32; 2] {
    let i = index / width as isize;
    let j = index % width as isize;

    let mut min = 255;
    let mut max = 0;

    for cell in pg {
        let y = i + cell[0] as isize;
        let x = j + cell[1] as isize;

        // Outside the image, a dilation sees black and an erosion sees white,
        // once the +/-255 below is applied. That is what keeps a closing from
        // eating into the border.
        let mut k = if x >= width as isize || y >= height as isize || x < 0 || y < 0 {
            match op {
                Op::Dilate => 0,
                Op::Erode => 255,
            }
        } else {
            i32::from(pixels[(x + width as isize * y) as usize])
        };

        match op {
            Op::Dilate => k += cell[2],
            Op::Erode => k -= cell[2],
        }

        if k < min {
            min = k;
        }
        if k > max {
            max = k;
        }
    }

    [min & 0xFF, max & 0xFF]
}

/// Closes a buffer in place with a disk of the given radius.
///
/// The shorthand both Audiveris call sites use.
///
/// # Panics
///
/// Panics if `pixels` is not exactly `width * height` bytes.
pub fn close_with_disk(pixels: &mut [u8], width: usize, height: usize, radius: f32) {
    MorphoProcessor::new(StructureElement::circle(1, radius, [0, 0])).close(pixels, width, height);
}

/// The radius `SpotsBuilder.close` derives from a beam thickness.
///
/// Every step is a Java expression: the ratio is applied in `double`, the
/// subtraction of one happens before the halving, and the result is narrowed to
/// `float` before it ever reaches the structuring element.
#[must_use]
pub fn spot_radius(beam: i32, diameter_ratio: f64) -> f32 {
    let diameter = f64::from(beam) * diameter_ratio;
    (diameter - 1.0) as f32 / 2.0
}

/// Audiveris's `beamCircleDiameterRatio` default.
pub const BEAM_CIRCLE_DIAMETER_RATIO: f64 = 0.8;

/// FNV-1a over a buffer, the digest every oracle probe here uses.
#[must_use]
pub fn digest(pixels: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for &pixel in pixels {
        hash = (hash ^ u64::from(pixel)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORACLE: &str = include_str!("../../../oracle/morphology.txt");

    /// The records of one kind, each already split into fields.
    fn records(kind: &str) -> Vec<Vec<&'static str>> {
        ORACLE
            .lines()
            .filter(|line| !line.starts_with('#'))
            .map(|line| line.split_whitespace().collect::<Vec<_>>())
            .filter(|fields| fields.first() == Some(&kind))
            .collect()
    }

    /// The generator the oracle probe uses, so the port can rebuild its inputs
    /// without shipping fixtures. A Java `int` multiply, so it wraps.
    fn synthetic(width: usize, height: usize, seed: i32) -> Vec<u8> {
        let mut state = seed;
        (0..width * height)
            .map(|_| {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12345);
                (((state as u32) >> 16) & 0xff) as u8
            })
            .collect()
    }

    /// The probe's drawn buffer: white, with bars and a blob in it.
    fn shapes(width: usize, height: usize) -> Vec<u8> {
        let mut buffer = vec![255_u8; width * height];
        for y in 0..height {
            for x in 0..width {
                let (xi, yi) = (x as i32, y as i32);
                let bar = (y % 11) < 4 && (x % 13) > (x / 37);
                let blob = (xi - 20) * (xi - 20) + (yi - 12) * (yi - 12) < 30;
                if bar || blob {
                    buffer[x + y * width] = ((xi * 7) % 64) as u8;
                }
            }
        }
        buffer
    }

    /// One of the two generated buffers, by the name the oracle calls it.
    fn generated(name: &str, width: usize, height: usize) -> Vec<u8> {
        match name {
            "noise" => synthetic(width, height, if width == 24 { 12345 } else { 987_654_321 }),
            "shapes" => shapes(width, height),
            other => panic!("unknown generated buffer {other}"),
        }
    }

    #[test]
    fn generators_match_the_probe() {
        // A tripwire ahead of the closing digests: if the two generators have
        // drifted, every one of those fails at once and blames morphology.
        let rows = records("synthsrc");
        assert_eq!(rows.len(), 4);
        for row in rows {
            let (name, width, height, want) = (
                row[1],
                row[2].parse().unwrap(),
                row[3].parse().unwrap(),
                row[4],
            );
            assert_eq!(
                digest(&generated(name, width, height)),
                want,
                "{name} {width}x{height}"
            );
        }
    }

    #[test]
    fn builds_the_structuring_elements_java_builds() {
        let dimensions = records("se");
        assert_eq!(dimensions.len(), 12);

        // The mask and the vector are dumped rather than hashed, so a disk that
        // is one cell wrong says which cell.
        let masks = records("semask");
        let vects = records("sevect");
        let mut mask_at = 0;
        let mut vect_at = 0;

        for row in dimensions {
            let radius: f32 = row[1].parse().unwrap();
            let se = StructureElement::circle(1, radius, [0, 0]);
            assert_eq!(
                [
                    se.width().to_string(),
                    se.height().to_string(),
                    se.area().to_string(),
                    se.vect().len().to_string()
                ],
                [row[2], row[3], row[4], row[5]].map(str::to_owned),
                "radius {radius}"
            );

            for y in 0..se.height() {
                let picture: String = (0..se.width())
                    .map(|x| {
                        if se.mask()[x + y * se.width()] > 0 {
                            '#'
                        } else {
                            '.'
                        }
                    })
                    .collect();
                let expected = &masks[mask_at];
                assert_eq!(
                    (expected[1], picture.as_str()),
                    (y.to_string().as_str(), expected[2]),
                    "radius {radius} row {y}"
                );
                mask_at += 1;
            }

            for (index, cell) in se.vect().iter().enumerate() {
                let expected = &vects[vect_at];
                assert_eq!(
                    [
                        index.to_string(),
                        cell[0].to_string(),
                        cell[1].to_string(),
                        cell[2].to_string(),
                        cell[3].to_string()
                    ],
                    [
                        expected[1],
                        expected[2],
                        expected[3],
                        expected[4],
                        expected[5]
                    ]
                    .map(str::to_owned),
                    "radius {radius} cell {index}"
                );
                vect_at += 1;
            }
        }

        assert_eq!((mask_at, vect_at), (masks.len(), vects.len()));
    }

    #[test]
    fn closes_the_generated_buffers_the_way_java_does() {
        let rows = records("synth");
        assert_eq!(rows.len(), 12);
        for row in rows {
            let (name, width, height): (&str, usize, usize) =
                (row[1], row[2].parse().unwrap(), row[3].parse().unwrap());
            let radius: f32 = row[4].parse().unwrap();
            let mut buffer = generated(name, width, height);
            close_with_disk(&mut buffer, width, height, radius);
            assert_eq!(digest(&buffer), row[5], "{name} {width}x{height} r{radius}");
        }
    }

    #[test]
    fn closes_the_small_buffers_pixel_for_pixel() {
        // The digests above say whether; these say where.
        for name in ["noise", "shapes"] {
            for radius in [1.5_f32, 3.5] {
                let (width, height) = (24, 16);
                let mut buffer = generated(name, width, height);
                close_with_disk(&mut buffer, width, height, radius);

                let kind = format!("synthpix.{name}.{radius:.1}");
                let rows = records(&kind);
                assert_eq!(rows.len(), height, "{kind}");
                for (y, row) in rows.iter().enumerate() {
                    let actual: String = buffer[y * width..(y + 1) * width]
                        .iter()
                        .map(|pixel| format!("{pixel:02x}"))
                        .collect();
                    assert_eq!(
                        (row[1], actual.as_str()),
                        (y.to_string().as_str(), row[2]),
                        "{kind} row {y}"
                    );
                }
            }
        }
    }

    #[test]
    fn derives_the_radius_spotsbuilder_derives() {
        // chula.png: a beam of 12 gives a diameter of 9.6 and a radius of 4.3.
        let row = &records("radius")[0];
        let beam: i32 = row[2].parse().unwrap();
        let radius: f32 = row[10].parse().unwrap();
        assert_eq!(spot_radius(beam, BEAM_CIRCLE_DIAMETER_RATIO), radius);
    }

    #[test]
    fn closing_leaves_a_flat_buffer_alone() {
        // Idempotence on a constant is the one property the +/-255 masking
        // could plausibly break, and it breaks silently.
        for level in [0_u8, 1, 128, 254, 255] {
            let mut buffer = vec![level; 40 * 30];
            close_with_disk(&mut buffer, 40, 30, 3.5);
            assert_eq!(buffer, vec![level; 40 * 30], "level {level}");
        }
    }

    #[test]
    fn closing_erases_dark_detail_narrower_than_the_disk() {
        // What BEAMS asks of it. Audiveris's buffers are ink-on-white, so a
        // dilation spreads the white; a closing therefore eats dark strokes
        // thinner than the element and leaves thicker ones. That is how stems
        // and staff remnants disappear while beams survive to be found.
        let (width, height) = (48, 24);
        let disk = StructureElement::circle(1, 3.5, [0, 0]);
        assert_eq!(disk.width(), 11); // A footprint spanning 7 columns.

        for (thickness, survives) in [(2_usize, false), (9, true)] {
            let mut buffer = vec![255_u8; width * height];
            for y in 6..6 + thickness {
                for x in 8..40 {
                    buffer[x + y * width] = 0;
                }
            }
            close_with_disk(&mut buffer, width, height, 3.5);
            let still_dark = buffer[24 + (6 + thickness / 2) * width] == 0;
            assert_eq!(still_dark, survives, "thickness {thickness}");
        }
    }

    #[test]
    fn closing_is_dilation_then_erosion() {
        // Java's interleaved loops are equivalent to two whole passes; this
        // pins that, so the interleaving can be optimised away later against a
        // test rather than against an argument.
        let (width, height) = (37, 23);
        let se = StructureElement::circle(1, 2.5, [0, 0]);
        let pg = se.vect();

        let source = synthetic(width, height, 4242);
        let mut dilated = vec![0_u8; source.len()];
        for index in 0..source.len() as isize {
            dilated[index as usize] =
                (min_max(index, width, height, &source, pg, Op::Dilate)[1] - 255) as u8;
        }
        let mut expected = vec![0_u8; source.len()];
        for index in 0..source.len() as isize {
            expected[index as usize] =
                (min_max(index, width, height, &dilated, pg, Op::Erode)[0] + 255) as u8;
        }

        let mut actual = source.clone();
        MorphoProcessor::new(se).close(&mut actual, width, height);
        assert_eq!(actual, expected);
    }
}
