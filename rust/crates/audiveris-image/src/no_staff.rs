// SPDX-License-Identifier: AGPL-3.0-or-later

//! The staff-free image: Java `Picture.buildNoStaffBuffer` and
//! `buildNoStaffTable`.
//!
//! Everything downstream of GRID that looks at *ink* rather than geometry
//! starts here rather than from the binary raster. `LedgersFilter` builds its
//! runs from it, `SheetScanner` scans it for text, `KeyExtractor` reads it, and
//! BEAMS works from the same staff-free image. Until it exists, none of those
//! stages can be driven on a real page at all -- which makes this the gate on
//! the whole second half of the pipeline rather than a detail of any one stage.
//!
//! # What Java does
//!
//! It does not recompute anything. It takes the **binary** raster, paints every
//! staff line's glyph onto it in white, and extracts vertical runs from the
//! result:
//!
//! ```text
//! g.setColor(Color.WHITE);
//! for each staff, for each line:
//!     glyph.getRunTable().render(g, glyph.getTopLeft());
//! ...
//! new RunTableFactory(VERTICAL).createTable(source)
//! ```
//!
//! So the staff lines are erased by *the ink that was assigned to them*, not by
//! a geometric band around the line. A pixel a line's glyph never claimed
//! survives even if it sits exactly on the line, and that is the point: what is
//! left is precisely the ink GRID could not explain as a staff line.
//!
//! `RunTable.render` fills each run as a one-pixel-high rectangle at
//! `(offset.x + run.start, offset.y + sequence)`, which for the horizontal run
//! tables staff-line glyphs carry means one horizontal span per glyph row.
//!
//! # Why erasing can be exact
//!
//! Painting white over black is idempotent and order-independent, so this is
//! bit-exact comparable against Java with a plain hash of the resulting buffer,
//! the same gate `oracle/grid-binary.txt` uses for the binary raster. It has no
//! thresholds and no floating point.

use crate::run_table::{BACKGROUND, Orientation, RunTable, RunTableError};
use crate::staff_line_conversion::StaffGlyph;

/// Paints every staff-line glyph white over a copy of the binary raster.
///
/// `binary` is row-major, one byte per pixel, foreground 0 and background 255,
/// as [`crate::adaptive`] produces.
///
/// A glyph run that falls outside the raster is clipped rather than refused:
/// Java renders through a `Graphics2D` whose clip is the image, so an
/// off-image run silently paints nothing. Refusing here would turn a
/// tolerated condition into a failure the Java run does not have.
///
/// # Errors
///
/// [`RunTableError::InvalidPixels`] if `binary` does not match `width` by
/// `height`.
pub fn erase_staff_glyphs(
    binary: &[u8],
    width: usize,
    height: usize,
    glyphs: &[StaffGlyph],
) -> Result<Vec<u8>, RunTableError> {
    if binary.len()
        != width
            .checked_mul(height)
            .ok_or(RunTableError::InvalidDimensions)?
    {
        return Err(RunTableError::InvalidPixels);
    }
    let mut pixels = binary.to_vec();

    for glyph in glyphs {
        // Staff-line glyphs carry horizontal run tables, so a sequence is a
        // row and a run is a horizontal span within it.
        for sequence in 0..glyph.runs.sequence_count() {
            let Some(runs) = glyph.runs.sequence(sequence) else {
                continue;
            };
            let Some(row) = glyph.y.checked_add(sequence) else {
                continue;
            };
            if row >= height {
                continue;
            }
            for run in runs {
                let Some(start) = glyph.x.checked_add(run.start) else {
                    continue;
                };
                let stop = start.saturating_add(run.length).min(width);
                if start >= width {
                    continue;
                }
                pixels[row * width + start..row * width + stop].fill(BACKGROUND);
            }
        }
    }

    Ok(pixels)
}

/// `Picture.buildNoStaffTable`: the vertical run table of the staff-free image.
///
/// # Errors
///
/// Whatever [`erase_staff_glyphs`] or run extraction raises.
pub fn build_no_staff_table(
    binary: &[u8],
    width: usize,
    height: usize,
    glyphs: &[StaffGlyph],
) -> Result<RunTable, RunTableError> {
    let pixels = erase_staff_glyphs(binary, width, height, glyphs)?;
    RunTable::from_pixels(Orientation::Vertical, width, height, &pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_table::FOREGROUND;

    fn glyph(x: usize, y: usize, rows: &[&[(usize, usize)]]) -> StaffGlyph {
        let width = rows
            .iter()
            .flat_map(|row| row.iter().map(|(start, length)| start + length))
            .max()
            .unwrap_or(0);
        let mut pixels = vec![BACKGROUND; width * rows.len()];
        for (index, row) in rows.iter().enumerate() {
            for (start, length) in *row {
                pixels[index * width + start..index * width + start + length].fill(FOREGROUND);
            }
        }
        StaffGlyph {
            x,
            y,
            runs: RunTable::from_pixels(Orientation::Horizontal, width, rows.len(), &pixels)
                .expect("a glyph table"),
        }
    }

    #[test]
    fn erases_exactly_the_ink_the_glyph_claimed() {
        // A 6x3 raster with a full black middle row.
        let mut binary = vec![BACKGROUND; 18];
        binary[6..12].fill(FOREGROUND);
        // A glyph claiming only the middle four pixels of that row.
        let glyphs = [glyph(1, 1, &[&[(0, 4)]])];

        let erased = erase_staff_glyphs(&binary, 6, 3, &glyphs).expect("erased");
        assert_eq!(
            &erased[6..12],
            &[
                FOREGROUND, BACKGROUND, BACKGROUND, BACKGROUND, BACKGROUND, FOREGROUND
            ]
        );
        // The pixels the glyph never claimed survive, which is the whole point:
        // what is left is the ink GRID could not explain as a staff line.
        assert_eq!(erased[6], FOREGROUND);
        assert_eq!(erased[11], FOREGROUND);
    }

    #[test]
    fn painting_white_is_idempotent_and_order_independent() {
        let mut binary = vec![FOREGROUND; 12];
        binary[0] = BACKGROUND;
        let a = glyph(0, 0, &[&[(0, 3)]]);
        let b = glyph(1, 0, &[&[(0, 3)]]);

        let one = erase_staff_glyphs(&binary, 4, 3, &[a.clone(), b.clone()]).expect("erased");
        let other = erase_staff_glyphs(&binary, 4, 3, &[b, a.clone()]).expect("erased");
        assert_eq!(one, other);
        let twice = erase_staff_glyphs(&one, 4, 3, &[a]).expect("erased");
        assert_eq!(one, twice);
    }

    #[test]
    fn a_run_past_the_edge_is_clipped_rather_than_refused() {
        // Java renders through a Graphics2D clipped to the image, so an
        // off-image run paints nothing and does not fail.
        let binary = vec![FOREGROUND; 9];
        let glyphs = [glyph(2, 2, &[&[(0, 5)]]), glyph(0, 9, &[&[(0, 2)]])];
        let erased = erase_staff_glyphs(&binary, 3, 3, &glyphs).expect("clipped, not refused");
        assert_eq!(erased[8], BACKGROUND, "the in-image part is painted");
        assert_eq!(erased[0], FOREGROUND, "the off-image glyph painted nothing");
    }

    #[test]
    fn the_table_is_vertical_and_sees_what_survives() {
        let mut binary = vec![BACKGROUND; 9];
        binary[3..6].fill(FOREGROUND);
        let table = build_no_staff_table(&binary, 3, 3, &[]).expect("a table");
        assert_eq!(table.orientation(), Orientation::Vertical);
        // Three columns, each with a single one-pixel run in the middle row.
        assert_eq!(table.sequence_count(), 3);
        for column in 0..3 {
            assert_eq!(table.sequence(column).unwrap_or_default().len(), 1);
        }

        let erased =
            build_no_staff_table(&binary, 3, 3, &[glyph(0, 1, &[&[(0, 3)]])]).expect("a table");
        assert!(
            (0..3).all(|column| erased.sequence(column).unwrap_or_default().is_empty()),
            "erasing the only ink leaves an empty table"
        );
    }

    #[test]
    fn a_mismatched_raster_is_refused() {
        assert!(erase_staff_glyphs(&[0; 5], 3, 3, &[]).is_err());
    }
}
