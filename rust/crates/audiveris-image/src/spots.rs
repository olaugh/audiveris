// SPDX-License-Identifier: AGPL-3.0-or-later

//! The chain from the staff-free page to the beam spots BEAMS recognises.
//!
//! Ported from `sheet/beam/SpotsBuilder.java`. `buildSheetSpots` is eight
//! transforms deep before a single beam is considered, and Audiveris caches
//! only the last of them, so they are separated here and each is gradeable on
//! its own:
//!
//! 1. stem-run removal -- horizontal runs shorter than the stem width dropped;
//! 2. a median filter at radius 1;
//! 3. a gaussian blur at radius 1;
//! 4. the staff header areas painted white;
//! 5. a morphological closing with a disk sized from the beam thickness;
//! 6. a threshold at 140 for beams, and a second at 170 kept for HEADS;
//! 7. vertical runs;
//! 8. connected components.
//!
//! Step 4 is the one that reaches outside this crate. Java takes the header
//! extent from `Staff.getHeaderStop`, which HEADERS produces, so the erase
//! rectangles are an *input* here rather than something derived. That keeps the
//! other seven gradeable while HEADERS is still a lifecycle-only stage, and
//! makes the dependency visible instead of implicit.

use crate::gaussian::gaussian_gray;
use crate::median::median_gray;
use crate::morphology::{BEAM_CIRCLE_DIAMETER_RATIO, close_with_disk, spot_radius};
use crate::run_table::{BACKGROUND, Orientation, RunTable, RunTableError};

/// Audiveris's `beamBinarizationThreshold` default.
pub const BEAM_BINARIZATION_THRESHOLD: u8 = 140;

/// Audiveris's `headBinarizationThreshold` default.
pub const HEAD_BINARIZATION_THRESHOLD: u8 = 170;

/// Audiveris's `Picture.medianRadius` default.
pub const MEDIAN_RADIUS: usize = 1;

/// One rectangle `eraseHeaderAreas` paints white.
///
/// Java derives it per system: the abscissa runs from the system's left bound
/// to the first header stop among its staves, and the ordinates from the first
/// staff's top line to the last staff's bottom line at that abscissa, each
/// pushed out by `staffVerticalMargin`. A system whose staves all lack a header
/// -- a tablature system -- contributes nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderErase {
    /// `system.getBounds().x`.
    pub x: i32,
    /// `staff.getHeaderStop()`, inclusive.
    pub stop: i32,
    /// `firstStaff.getFirstLine().yAt(stop) - margin`.
    pub top: i32,
    /// `lastStaff.getLastLine().yAt(stop) + margin`.
    pub bottom: i32,
}

/// What `SpotsBuilder` needs that it does not compute itself.
#[derive(Clone, Debug, PartialEq)]
pub struct SpotParameters {
    /// `scale.getMaxStem()`, the run length below which a run is dropped.
    pub max_stem: usize,
    /// The beam thickness the disk is sized from: the smaller of the main and
    /// small beam scales, as `buildSheetSpots` picks it.
    pub beam: i32,
    pub median_radius: usize,
    pub gaussian_radius: f32,
    pub diameter_ratio: f64,
    /// The header rectangles, which come from HEADERS. Empty is a valid input
    /// and means "no headers known", not "no headers".
    pub header_erases: Vec<HeaderErase>,
}

impl SpotParameters {
    /// The defaults, for a page whose stem width and beam thickness are known.
    #[must_use]
    pub fn new(max_stem: usize, beam: i32) -> Self {
        Self {
            max_stem,
            beam,
            median_radius: MEDIAN_RADIUS,
            gaussian_radius: crate::gaussian::DEFAULT_RADIUS,
            diameter_ratio: BEAM_CIRCLE_DIAMETER_RATIO,
            header_erases: Vec::new(),
        }
    }

    /// The disk radius `close` uses, as `SpotsBuilder.close` derives it.
    #[must_use]
    pub fn radius(&self) -> f32 {
        spot_radius(self.beam, self.diameter_ratio)
    }
}

/// Every buffer the chain passes through, so a divergence names its own stage.
#[derive(Clone, Debug)]
pub struct SpotChain {
    /// After stem-run removal.
    pub destemmed: Vec<u8>,
    pub median: Vec<u8>,
    pub gaussian: Vec<u8>,
    /// After the header erase. Identical to `gaussian` when no rectangles were
    /// supplied.
    pub erased: Vec<u8>,
    /// After the morphological closing. This is what both thresholds read.
    pub closed: Vec<u8>,
}

/// `SpotsBuilder.getBuffer` followed by the erase and the closing.
///
/// # Errors
///
/// Returns an error if the run table cannot be built from the buffer.
///
/// # Panics
///
/// Panics if `no_staff` is not exactly `width * height` bytes.
pub fn spot_chain(
    no_staff: &[u8],
    width: usize,
    height: usize,
    parameters: &SpotParameters,
) -> Result<SpotChain, RunTableError> {
    assert_eq!(
        no_staff.len(),
        width * height,
        "buffer is not width * height"
    );

    // Java removes stems by building a filtered run table and painting it back
    // out, which is why a dropped run leaves white rather than merging its
    // neighbours. The comment in Java calls this "could be much more efficient
    // if performed on buffer directly"; done directly it would not be the same
    // operation, so it is done the same way.
    let destemmed = RunTable::from_pixels_with_min_length(
        Orientation::Horizontal,
        width,
        height,
        no_staff,
        parameters.max_stem,
    )?
    .to_pixels();

    let median = median_gray(width, height, &destemmed, parameters.median_radius);
    let gaussian = gaussian_gray(width, height, &median, parameters.gaussian_radius);

    let mut erased = gaussian.clone();
    erase_headers(&mut erased, width, height, &parameters.header_erases);

    let mut closed = erased.clone();
    close_with_disk(&mut closed, width, height, parameters.radius());

    Ok(SpotChain {
        destemmed,
        median,
        gaussian,
        erased,
        closed,
    })
}

/// `SpotsBuilder.eraseHeaderAreas`: paint each header rectangle white.
///
/// The rectangles are clipped to the buffer the way ImageJ's `setRoi` clips
/// them, so one that starts left of the page or runs past its bottom paints the
/// part that is there rather than nothing or too much.
pub fn erase_headers(pixels: &mut [u8], width: usize, height: usize, erases: &[HeaderErase]) {
    for erase in erases {
        let left = erase.x.max(0) as usize;
        let right = (erase.stop + 1).clamp(0, width as i32) as usize;
        let top = erase.top.max(0) as usize;
        let bottom = (erase.bottom + 1).clamp(0, height as i32) as usize;

        for y in top..bottom.min(height) {
            for x in left..right.min(width) {
                pixels[x + y * width] = BACKGROUND;
            }
        }
    }
}

/// ImageJ's `ByteProcessor.threshold`: at or below the level is ink.
#[must_use]
pub fn threshold(pixels: &[u8], level: u8) -> Vec<u8> {
    pixels
        .iter()
        .map(|&pixel| if pixel <= level { 0 } else { 255 })
        .collect()
}

/// The vertical run table BEAMS builds its spot glyphs from.
///
/// # Errors
///
/// Returns an error if the buffer does not match the given dimensions.
pub fn spot_runs(
    closed: &[u8],
    width: usize,
    height: usize,
    level: u8,
) -> Result<RunTable, RunTableError> {
    // Java's SPOT_ORIENTATION. Vertical, not horizontal like the stem filter
    // above -- the two tables in this chain read the page in different
    // directions, and swapping them is a silent way to get plausible garbage.
    RunTable::from_pixels(
        Orientation::Vertical,
        width,
        height,
        &threshold(closed, level),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_splits_at_the_level_inclusively() {
        // ImageJ puts the level itself on the ink side. One off here moves
        // every spot boundary by a gray level.
        assert_eq!(
            threshold(&[0, 139, 140, 141, 255], 140),
            [0, 0, 0, 255, 255]
        );
    }

    #[test]
    fn stem_removal_drops_short_runs_without_joining_their_neighbours() {
        // A row of three-long, one-long, three-long ink. At a stem width of two
        // the middle run goes and the outer two stay separate.
        let width = 9;
        let mut pixels = vec![255_u8; width];
        for x in [0, 1, 2, 4, 6, 7, 8] {
            pixels[x] = 0;
        }
        let table =
            RunTable::from_pixels_with_min_length(Orientation::Horizontal, width, 1, &pixels, 2)
                .expect("a table");
        assert_eq!(table.to_pixels(), [0, 0, 0, 255, 255, 255, 0, 0, 0]);
    }

    #[test]
    fn header_erase_clips_to_the_page() {
        let (width, height) = (8, 4);
        let mut pixels = vec![0_u8; width * height];
        erase_headers(
            &mut pixels,
            width,
            height,
            &[HeaderErase {
                x: -3,
                stop: 2,
                top: 1,
                bottom: 99,
            }],
        );
        let white: Vec<usize> = pixels
            .iter()
            .enumerate()
            .filter_map(|(index, &pixel)| (pixel == 255).then_some(index))
            .collect();
        // Columns 0..=2 of rows 1..=3, and nothing outside the buffer.
        assert_eq!(white, [8, 9, 10, 16, 17, 18, 24, 25, 26]);
    }

    #[test]
    fn an_empty_erase_list_leaves_the_gaussian_alone() {
        // "No headers known" has to mean "erase nothing", not "erase
        // everything" or "skip the stage" -- a port without HEADERS runs this
        // path, and it must differ from Java only by the erase.
        let parameters = SpotParameters::new(4, 12);
        assert!(parameters.header_erases.is_empty());
        let chain = spot_chain(&vec![255; 40 * 30], 40, 30, &parameters).expect("a chain");
        assert_eq!(chain.erased, chain.gaussian);
    }
}
