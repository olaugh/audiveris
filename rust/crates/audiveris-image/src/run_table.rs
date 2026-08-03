// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{error::Error, fmt};

pub const FOREGROUND: u8 = 0;
pub const BACKGROUND: u8 = 255;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Run {
    pub start: usize,
    pub length: usize,
}

impl Run {
    #[must_use]
    pub const fn new(start: usize, length: usize) -> Self {
        Self { start, length }
    }

    #[must_use]
    pub const fn stop(self) -> usize {
        self.start + self.length - 1
    }

    #[must_use]
    pub fn common_length(self, other: Self) -> isize {
        let start = self.start.max(other.start);
        let stop = self.stop().min(other.stop());
        isize::try_from(stop).unwrap_or(isize::MAX) - isize::try_from(start).unwrap_or(isize::MAX)
            + 1
    }

    pub fn translate(&mut self, delta: isize) -> Result<(), RunTableError> {
        self.start = self
            .start
            .checked_add_signed(delta)
            .ok_or(RunTableError::OutOfBounds)?;
        Ok(())
    }
}

impl fmt::Display for Run {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Run{{{}/{}}}", self.start, self.length)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunTableError {
    InvalidDimensions,
    InvalidRun,
    OutOfBounds,
    InvalidPixels,
    InvalidRle,
    IncompatibleTable,
    RunNotFound,
}

impl fmt::Display for RunTableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Error for RunTableError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunTable {
    orientation: Orientation,
    width: usize,
    height: usize,
    sequences: Vec<Vec<Run>>,
}

impl RunTable {
    pub fn new(
        orientation: Orientation,
        width: usize,
        height: usize,
    ) -> Result<Self, RunTableError> {
        if width == 0 || height == 0 {
            return Err(RunTableError::InvalidDimensions);
        }
        let sequence_count = match orientation {
            Orientation::Horizontal => height,
            Orientation::Vertical => width,
        };
        Ok(Self {
            orientation,
            width,
            height,
            sequences: vec![Vec::new(); sequence_count],
        })
    }

    pub fn from_pixels(
        orientation: Orientation,
        width: usize,
        height: usize,
        pixels: &[u8],
    ) -> Result<Self, RunTableError> {
        if pixels.len()
            != width
                .checked_mul(height)
                .ok_or(RunTableError::InvalidDimensions)?
        {
            return Err(RunTableError::InvalidPixels);
        }
        let mut table = Self::new(orientation, width, height)?;
        let (sequence_count, coordinate_count) = match orientation {
            Orientation::Horizontal => (height, width),
            Orientation::Vertical => (width, height),
        };
        for sequence in 0..sequence_count {
            let mut coordinate = 0;
            while coordinate < coordinate_count {
                let foreground = match orientation {
                    Orientation::Horizontal => pixels[sequence * width + coordinate] == FOREGROUND,
                    Orientation::Vertical => pixels[coordinate * width + sequence] == FOREGROUND,
                };
                if !foreground {
                    coordinate += 1;
                    continue;
                }
                let start = coordinate;
                while coordinate < coordinate_count {
                    let still_foreground = match orientation {
                        Orientation::Horizontal => {
                            pixels[sequence * width + coordinate] == FOREGROUND
                        }
                        Orientation::Vertical => {
                            pixels[coordinate * width + sequence] == FOREGROUND
                        }
                    };
                    if !still_foreground {
                        break;
                    }
                    coordinate += 1;
                }
                table.sequences[sequence].push(Run::new(start, coordinate - start));
            }
        }
        Ok(table)
    }

    #[must_use]
    pub const fn orientation(&self) -> Orientation {
        self.orientation
    }

    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    #[must_use]
    pub fn sequence_count(&self) -> usize {
        self.sequences.len()
    }

    pub fn sequence(&self, index: usize) -> Option<&[Run]> {
        self.sequences.get(index).map(Vec::as_slice)
    }

    pub fn sequence_rle(&self, index: usize) -> Option<Vec<usize>> {
        let runs = self.sequences.get(index)?;
        if runs.is_empty() {
            return None;
        }
        let mut rle = Vec::with_capacity(runs.len() * 2 + 1);
        if runs[0].start > 0 {
            rle.push(0);
            rle.push(runs[0].start);
        }
        rle.push(runs[0].length);
        for pair in runs.windows(2) {
            rle.push(pair[1].start - pair[0].stop() - 1);
            rle.push(pair[1].length);
        }
        Some(rle)
    }

    pub fn set_sequence_rle(
        &mut self,
        index: usize,
        rle: Option<&[usize]>,
    ) -> Result<(), RunTableError> {
        let sequence = self
            .sequences
            .get_mut(index)
            .ok_or(RunTableError::OutOfBounds)?;
        sequence.clear();
        let Some(rle) = rle else { return Ok(()) };
        if rle.is_empty() {
            return Ok(());
        }
        if rle.len() % 2 == 0 {
            return Err(RunTableError::InvalidRle);
        }
        let mut coordinate = 0;
        for (index, &length) in rle.iter().enumerate() {
            if index % 2 == 0 {
                if length == 0 && index != 0 {
                    return Err(RunTableError::InvalidRle);
                }
                if length > 0 {
                    sequence.push(Run::new(coordinate, length));
                }
            } else if length == 0 {
                return Err(RunTableError::InvalidRle);
            }
            coordinate += length;
        }
        if coordinate > self.coordinate_count() {
            return Err(RunTableError::OutOfBounds);
        }
        Ok(())
    }

    fn coordinate_count(&self) -> usize {
        match self.orientation {
            Orientation::Horizontal => self.width,
            Orientation::Vertical => self.height,
        }
    }

    pub fn add_run(&mut self, index: usize, run: Run) -> Result<bool, RunTableError> {
        if run.length == 0 || run.stop() >= self.coordinate_count() {
            return Err(RunTableError::InvalidRun);
        }
        let sequence = self
            .sequences
            .get_mut(index)
            .ok_or(RunTableError::OutOfBounds)?;
        if sequence
            .iter()
            .any(|current| current.start <= run.stop() && run.start <= current.stop())
        {
            return Ok(false);
        }
        sequence.push(run);
        sequence.sort_unstable_by_key(|item| item.start);
        let mut merged: Vec<Run> = Vec::with_capacity(sequence.len());
        for current in sequence.drain(..) {
            if let Some(previous) = merged.last_mut()
                && previous.stop() + 1 == current.start
            {
                previous.length += current.length;
                continue;
            }
            merged.push(current);
        }
        *sequence = merged;
        Ok(true)
    }

    pub fn run_at(&self, x: usize, y: usize) -> Option<Run> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let (sequence, coordinate) = match self.orientation {
            Orientation::Horizontal => (y, x),
            Orientation::Vertical => (x, y),
        };
        self.sequences[sequence]
            .iter()
            .copied()
            .find(|run| run.start <= coordinate && coordinate <= run.stop())
    }

    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> u8 {
        if self.run_at(x, y).is_some() {
            FOREGROUND
        } else {
            BACKGROUND
        }
    }

    #[must_use]
    pub fn to_pixels(&self) -> Vec<u8> {
        let mut pixels = vec![BACKGROUND; self.width * self.height];
        for (sequence, runs) in self.sequences.iter().enumerate() {
            for run in runs {
                for coordinate in run.start..=run.stop() {
                    let offset = match self.orientation {
                        Orientation::Horizontal => sequence * self.width + coordinate,
                        Orientation::Vertical => coordinate * self.width + sequence,
                    };
                    pixels[offset] = FOREGROUND;
                }
            }
        }
        pixels
    }

    #[must_use]
    pub fn total_run_count(&self) -> usize {
        self.sequences.iter().map(Vec::len).sum()
    }

    #[must_use]
    pub fn weight(&self) -> usize {
        self.sequences.iter().flatten().map(|run| run.length).sum()
    }

    pub fn include(&mut self, other: &Self) -> Result<(), RunTableError> {
        if self.orientation != other.orientation
            || self.width != other.width
            || self.height != other.height
        {
            return Err(RunTableError::IncompatibleTable);
        }
        let mut pixels = self.to_pixels();
        for (target, source) in pixels.iter_mut().zip(other.to_pixels()) {
            if source == FOREGROUND {
                *target = FOREGROUND;
            }
        }
        *self = Self::from_pixels(self.orientation, self.width, self.height, &pixels)?;
        Ok(())
    }

    pub fn purge<F>(
        &mut self,
        predicate: F,
        mut removed: Option<&mut Self>,
    ) -> Result<(), RunTableError>
    where
        F: Fn(Run) -> bool,
    {
        if let Some(target) = removed.as_ref()
            && (self.orientation != target.orientation
                || self.width != target.width
                || self.height != target.height)
        {
            return Err(RunTableError::IncompatibleTable);
        }
        for (sequence_index, sequence) in self.sequences.iter_mut().enumerate() {
            let mut kept = Vec::with_capacity(sequence.len());
            for run in sequence.drain(..) {
                if predicate(run) {
                    if let Some(target) = removed.as_deref_mut() {
                        target.sequences[sequence_index].push(run);
                    }
                } else {
                    kept.push(run);
                }
            }
            *sequence = kept;
        }
        Ok(())
    }

    pub fn remove_run(&mut self, index: usize, run: Run) -> Result<(), RunTableError> {
        let sequence = self
            .sequences
            .get_mut(index)
            .ok_or(RunTableError::OutOfBounds)?;
        let position = sequence
            .iter()
            .position(|candidate| *candidate == run)
            .ok_or(RunTableError::RunNotFound)?;
        sequence.remove(position);
        Ok(())
    }

    pub fn trim(&self) -> Result<(Self, (usize, usize)), RunTableError> {
        let pixels = self.to_pixels();
        let mut min_x = self.width;
        let mut min_y = self.height;
        let mut max_x = 0;
        let mut max_y = 0;
        let mut found = false;
        for y in 0..self.height {
            for x in 0..self.width {
                if pixels[y * self.width + x] == FOREGROUND {
                    found = true;
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }
        if !found {
            return Err(RunTableError::InvalidPixels);
        }
        if min_x == 0 && min_y == 0 && max_x + 1 == self.width && max_y + 1 == self.height {
            return Ok((self.clone(), (0, 0)));
        }
        let width = max_x - min_x + 1;
        let height = max_y - min_y + 1;
        let mut cropped = vec![BACKGROUND; width * height];
        for y in 0..height {
            for x in 0..width {
                cropped[y * width + x] = pixels[(y + min_y) * self.width + x + min_x];
            }
        }
        Ok((
            Self::from_pixels(self.orientation, width, height, &cropped)?,
            (min_x, min_y),
        ))
    }
}

/// Split a vertical binary run table for the GRID step, matching Java
/// `LagManager.dispatchRuns`.
///
/// Runs at least `1 + rint(max_fore * ledger_thickness)` pixels long are
/// copied into the returned vertical table. All remaining pixels are rebuilt
/// as horizontal runs in the other returned table. The source is not modified.
/// Java's `Math.rint` uses ties-to-even rounding, which is significant for
/// half-pixel thresholds.
pub fn dispatch_grid_runs(
    source: &RunTable,
    max_fore: usize,
    ledger_thickness: f64,
) -> Result<(RunTable, RunTable), RunTableError> {
    if source.orientation() != Orientation::Vertical {
        return Err(RunTableError::IncompatibleTable);
    }

    let adjusted = (max_fore as f64) * ledger_thickness;
    if !adjusted.is_finite() || adjusted < 0.0 || adjusted > (usize::MAX - 1) as f64 {
        return Err(RunTableError::InvalidDimensions);
    }
    let min_vertical_run_length = 1 + adjusted.round_ties_even() as usize;

    let mut short_vertical = source.clone();
    let mut long_vertical = RunTable::new(Orientation::Vertical, source.width(), source.height())?;
    short_vertical.purge(
        |run| run.length >= min_vertical_run_length,
        Some(&mut long_vertical),
    )?;
    let horizontal = RunTable::from_pixels(
        Orientation::Horizontal,
        source.width(),
        source.height(),
        &short_vertical.to_pixels(),
    )?;

    Ok((horizontal, long_vertical))
}

/// Run-table partitions produced by Java `LinesRetriever.createBothLags`
/// before section construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridRunTables {
    /// Long vertical runs used to build the barline lag.
    pub long_vertical: RunTable,
    /// Horizontal runs shorter than `min_horizontal_run_length`, held back
    /// until `LinesRetriever.addShortSections`.
    pub short_horizontal: RunTable,
    /// Remaining horizontal runs used for initial staff-line retrieval.
    pub long_horizontal: RunTable,
}

/// Produce the three run-table partitions used by
/// `LinesRetriever.createBothLags` without mutating `source`.
pub fn create_grid_run_tables(
    source: &RunTable,
    max_fore: usize,
    ledger_thickness: f64,
    min_horizontal_run_length: usize,
) -> Result<GridRunTables, RunTableError> {
    let (mut horizontal, long_vertical) = dispatch_grid_runs(source, max_fore, ledger_thickness)?;
    let mut short_horizontal =
        RunTable::new(Orientation::Horizontal, source.width(), source.height())?;
    horizontal.purge(
        |run| run.length < min_horizontal_run_length,
        Some(&mut short_horizontal),
    )?;

    Ok(GridRunTables {
        long_vertical,
        short_horizontal,
        long_horizontal: horizontal,
    })
}

impl fmt::Display for RunTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let orientation = match self.orientation {
            Orientation::Horizontal => "HORIZONTAL",
            Orientation::Vertical => "VERTICAL",
        };
        write!(
            f,
            "RunTable{{{orientation} {}x{}}}",
            self.width, self.height
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn horizontal() -> RunTable {
        let mut table = RunTable::new(Orientation::Horizontal, 10, 5).unwrap();
        for (sequence, run) in [
            (0, Run::new(1, 2)),
            (0, Run::new(5, 3)),
            (1, Run::new(0, 1)),
            (1, Run::new(4, 2)),
            (3, Run::new(0, 2)),
            (3, Run::new(4, 1)),
            (3, Run::new(8, 2)),
            (4, Run::new(2, 2)),
            (4, Run::new(6, 4)),
        ] {
            assert!(table.add_run(sequence, run).unwrap());
        }
        table
    }

    fn large_horizontal() -> RunTable {
        let mut table = RunTable::new(Orientation::Horizontal, 15, 8).unwrap();
        for (sequence, run) in [
            (2, Run::new(1, 2)),
            (2, Run::new(5, 3)),
            (3, Run::new(4, 2)),
            (5, Run::new(4, 1)),
            (5, Run::new(8, 2)),
            (6, Run::new(2, 2)),
            (6, Run::new(6, 4)),
        ] {
            assert!(table.add_run(sequence, run).unwrap());
        }
        table
    }

    #[test]
    fn dispatch_grid_runs_matches_java_threshold_and_reorientation() {
        let mut source = RunTable::new(Orientation::Vertical, 5, 8).unwrap();
        for (x, run) in [
            (0, Run::new(0, 2)),
            (0, Run::new(4, 3)),
            (1, Run::new(1, 4)),
            (3, Run::new(0, 1)),
            (3, Run::new(3, 2)),
            (4, Run::new(2, 5)),
        ] {
            source.add_run(x, run).unwrap();
        }
        let before = source.clone();

        // 1 + Math.rint(2 * 1.25) = 1 + 2 = 3 (ties to even).
        let (horizontal, long_vertical) = dispatch_grid_runs(&source, 2, 1.25).unwrap();

        assert_eq!(source, before, "dispatch must not mutate its source");
        assert_eq!(long_vertical.orientation(), Orientation::Vertical);
        assert_eq!(long_vertical.total_run_count(), 3);
        assert_eq!(long_vertical.weight(), 12);
        assert_eq!(horizontal.orientation(), Orientation::Horizontal);
        assert_eq!(horizontal.weight(), 5);
        assert_eq!(
            horizontal.to_pixels(),
            vec![
                0, 255, 255, 0, 255, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                0, 255, 255, 255, 255, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                255, 255, 255, 255, 255,
            ]
        );
    }

    #[test]
    fn dispatch_grid_runs_rejects_nonvertical_and_invalid_thresholds() {
        let source = RunTable::new(Orientation::Horizontal, 2, 2).unwrap();
        assert_eq!(
            dispatch_grid_runs(&source, 2, 1.0),
            Err(RunTableError::IncompatibleTable)
        );

        let source = RunTable::new(Orientation::Vertical, 2, 2).unwrap();
        assert_eq!(
            dispatch_grid_runs(&source, 2, f64::NAN),
            Err(RunTableError::InvalidDimensions)
        );
        assert_eq!(
            dispatch_grid_runs(&source, 2, -1.0),
            Err(RunTableError::InvalidDimensions)
        );
    }

    #[test]
    fn create_grid_run_tables_holds_back_only_short_horizontal_runs() {
        let pixels = [
            0, 0, 0, 255, 0, 255, 255, 255, 255, 255, 255, 255, 0, 255, 255, 255, 0, 0, 0, 0, 0, 0,
            255, 255, 255, 255, 255, 255, 0, 255, 255, 255,
        ];
        let source = RunTable::from_pixels(Orientation::Vertical, 8, 4, &pixels).unwrap();

        // Vertical threshold 3 removes the x=4 run (length 4). Its removal
        // splits the second horizontal row into lengths 4 and 1; min length 4
        // holds back the length-3 and length-1 runs.
        let tables = create_grid_run_tables(&source, 2, 1.0, 4).unwrap();

        assert_eq!(tables.long_vertical.weight(), 4);
        assert_eq!(tables.short_horizontal.weight(), 4);
        assert_eq!(tables.long_horizontal.weight(), 4);
        assert_eq!(source.weight(), 12);
        let mut reunited = tables.short_horizontal.clone();
        reunited.include(&tables.long_horizontal).unwrap();
        assert_eq!(reunited.weight(), 8);
    }

    #[test]
    fn create_grid_run_tables_keeps_threshold_length_in_long_partition() {
        let mut source = RunTable::new(Orientation::Vertical, 5, 3).unwrap();
        for x in 0..5 {
            source.add_run(x, Run::new(1, 1)).unwrap();
        }

        let tables = create_grid_run_tables(&source, 3, 1.0, 5).unwrap();
        assert_eq!(tables.short_horizontal.weight(), 0);
        assert_eq!(tables.long_horizontal.weight(), 5);
        assert_eq!(
            tables.long_horizontal.sequence(1).unwrap(),
            [Run::new(0, 5)]
        );
    }

    #[test]
    fn ports_java_dimensions_copy_counts_and_rle() {
        let table = horizontal();
        assert_eq!(table.clone(), table);
        assert_eq!((table.width(), table.height()), (10, 5));
        assert_eq!(table.sequence_count(), 5);
        assert_eq!(table.total_run_count(), 9);
        assert_eq!(table.sequence_rle(1), Some(vec![1, 3, 2]));
        assert_eq!(table.to_string(), "RunTable{HORIZONTAL 10x5}");
    }

    #[test]
    fn ports_java_pixel_and_run_queries() {
        let horizontal = horizontal();
        let vertical =
            RunTable::from_pixels(Orientation::Vertical, 10, 5, &horizontal.to_pixels()).unwrap();
        assert_eq!(vertical.total_run_count(), 16);
        assert_eq!(
            [
                vertical.get(0, 0),
                vertical.get(1, 0),
                vertical.get(2, 0),
                vertical.get(3, 0)
            ],
            [255, 0, 0, 255]
        );
        assert_eq!([vertical.get(0, 1), vertical.get(1, 1)], [0, 255]);
        assert_eq!(horizontal.run_at(0, 0), None);
        assert_eq!(horizontal.run_at(6, 0), Some(Run::new(5, 3)));
    }

    #[test]
    fn ports_java_inverse_union_case() {
        let mut table = horizontal();
        let inverse: Vec<u8> = table
            .to_pixels()
            .into_iter()
            .map(|pixel| {
                if pixel == FOREGROUND {
                    BACKGROUND
                } else {
                    FOREGROUND
                }
            })
            .collect();
        let inverse = RunTable::from_pixels(Orientation::Horizontal, 10, 5, &inverse).unwrap();
        table.include(&inverse).unwrap();
        assert_eq!(table.total_run_count(), 5);
        assert_eq!(table.weight(), 50);
    }

    #[test]
    fn ports_java_purge_and_removed_cases() {
        let mut table = horizontal();
        table.purge(|run| run.length <= 2, None).unwrap();
        assert_eq!(table.total_run_count(), 2);
        table.purge(|run| run.length > 2, None).unwrap();
        assert_eq!(table.total_run_count(), 0);

        let mut table = horizontal();
        let mut removed = RunTable::new(Orientation::Horizontal, 10, 5).unwrap();
        table
            .purge(|run| run.length == 2, Some(&mut removed))
            .unwrap();
        assert_eq!(removed.total_run_count(), 5);
        table
            .purge(|run| run.length == 1, Some(&mut removed))
            .unwrap();
        assert_eq!(removed.total_run_count(), 7);
    }

    #[test]
    fn ports_java_remove_set_sequence_and_trim_cases() {
        let mut table = horizontal();
        table.remove_run(3, Run::new(4, 1)).unwrap();
        assert_eq!(table.total_run_count(), 8);
        table.set_sequence_rle(1, Some(&[0, 3, 7])).unwrap();
        assert_eq!(table.sequence(1), Some([Run::new(3, 7)].as_slice()));

        let (trimmed, offset) = large_horizontal().trim().unwrap();
        assert_eq!(offset, (1, 2));
        assert_eq!((trimmed.width(), trimmed.height()), (9, 5));
    }

    #[test]
    fn run_operations_match_java() {
        let mut run = Run::new(5, 3);
        assert_eq!(run.stop(), 7);
        assert_eq!(run.to_string(), "Run{5/3}");
        assert_eq!(run.common_length(Run::new(6, 4)), 2);
        run.translate(-2).unwrap();
        assert_eq!(run, Run::new(3, 3));
    }
}
