// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::run_table::{Orientation, RunTable};

pub const MAX_BLACK_HEIGHT_RATIO: f64 = 0.08;
pub const MAX_WHITE_HEIGHT_RATIO: f64 = 0.25;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerticalRunHistograms {
    pub max_black: usize,
    pub max_white: usize,
    pub black: Vec<usize>,
    pub combo: Vec<usize>,
    /// The combo restricted to pairs whose black component is line-thin
    /// (<= [`LINE_BLACK_CAP`]): the staff-anchored interline evidence.
    pub line_combo: Vec<usize>,
}

impl VerticalRunHistograms {
    #[must_use]
    pub fn black_pixel_count(&self) -> usize {
        self.black
            .iter()
            .enumerate()
            .map(|(length, &count)| length * count)
            .sum()
    }
}

#[must_use]
/// Black runs thicker than this cannot be staff lines at the resolutions
/// this pipeline targets (lines are 1-2px at interline 6-8, 2-3px at 20).
pub const LINE_BLACK_CAP: usize = 3;

pub fn vertical_run_histograms(table: &RunTable) -> VerticalRunHistograms {
    assert_eq!(table.orientation(), Orientation::Vertical);
    let max_black = (table.height() as f64 * MAX_BLACK_HEIGHT_RATIO).round_ties_even() as usize;
    let max_white = (table.height() as f64 * MAX_WHITE_HEIGHT_RATIO).round_ties_even() as usize;
    let mut black = vec![0usize; max_black + 1];
    let mut combo = vec![0usize; max_black + max_white + 1];
    let mut line_combo = vec![0usize; max_black + max_white + 1];

    for x in 0..table.width() {
        let mut y_last = 0usize;
        let mut last_black = 0usize;
        for run in table.sequence(x).unwrap_or_default() {
            if run.length <= max_black {
                black[run.length] += 1;
            }
            if run.start > y_last {
                let white = run.start - y_last;
                if white <= max_white && last_black != 0 {
                    let previous_combo = last_black + white;
                    let next_combo = white + run.length;
                    // IntegerFunction.addValue ignores values outside its fixed
                    // domain, including combos with an oversized adjacent run.
                    if previous_combo < combo.len() {
                        combo[previous_combo] += 1;
                        // Staff-anchored variant: only pairs whose black
                        // component is line-thin. On a dense page the
                        // notehead pairs outvote the staff pairs in the
                        // plain combo and the interline is misread (a full
                        // bar of 32nds at interline 6 reads as 9); staff
                        // lines are the only ink that is reliably 1-3px.
                        if last_black <= LINE_BLACK_CAP {
                            line_combo[previous_combo] += 1;
                        }
                    }
                    if next_combo < combo.len() {
                        combo[next_combo] += 1;
                        if run.length <= LINE_BLACK_CAP {
                            line_combo[next_combo] += 1;
                        }
                    }
                }
            }
            last_black = run.length;
            y_last = run.start + run.length;
        }
    }

    VerticalRunHistograms {
        max_black,
        max_white,
        black,
        combo,
        line_combo,
    }
}

#[cfg(test)]
mod tests {
    use crate::run_table::{Run, RunTable};

    use super::*;

    #[test]
    fn ports_scale_builder_black_and_combo_counting() {
        let mut table = RunTable::new(Orientation::Vertical, 2, 100).unwrap();
        table.add_run(0, Run::new(2, 2)).unwrap();
        table.add_run(0, Run::new(10, 3)).unwrap();
        table.add_run(0, Run::new(20, 2)).unwrap();
        table.add_run(1, Run::new(0, 9)).unwrap();
        let histograms = vertical_run_histograms(&table);
        assert_eq!((histograms.max_black, histograms.max_white), (8, 25));
        assert_eq!(histograms.black[2], 2);
        assert_eq!(histograms.black[3], 1);
        assert_eq!(histograms.black[8], 0);
        assert_eq!(histograms.combo[8], 1); // previous 2 + white 6
        assert_eq!(histograms.combo[10], 1); // previous 3 + white 7
        assert_eq!(histograms.combo[9], 2); // 6+3 and 7+2
        assert_eq!(histograms.black_pixel_count(), 7);
    }

    #[test]
    fn omits_leading_and_trailing_white_runs() {
        let mut table = RunTable::new(Orientation::Vertical, 1, 100).unwrap();
        table.add_run(0, Run::new(10, 2)).unwrap();
        let histograms = vertical_run_histograms(&table);
        assert_eq!(histograms.combo.iter().sum::<usize>(), 0);
    }

    #[test]
    fn drops_out_of_domain_combo_like_java_integer_function() {
        let mut table = RunTable::new(Orientation::Vertical, 1, 100).unwrap();
        table.add_run(0, Run::new(0, 2)).unwrap();
        table.add_run(0, Run::new(5, 50)).unwrap();

        let histograms = vertical_run_histograms(&table);
        assert_eq!(histograms.combo[5], 1);
        assert_eq!(histograms.combo.iter().sum::<usize>(), 1);
    }
}
