// SPDX-License-Identifier: AGPL-3.0-or-later

//! Neutral vertical sampling from Java `ClustersRetriever.retrieveCombs`.
//!
//! This slice discovers regular filament sequences only. Recursive cluster
//! construction, filament back-links, display state, and skew-aware merging
//! remain outside the module.

use std::{cmp::Ordering, error::Error, fmt};

use audiveris_core::histogram::Histogram;

use crate::{
    filament::{FilamentError, FilamentGeometry},
    filament_comb::{FilamentComb, FilamentCombError},
};

#[derive(Clone, Debug)]
pub struct CombFilament {
    id: usize,
    ancestor_id: usize,
    geometry: FilamentGeometry,
}

impl CombFilament {
    pub fn new(
        id: usize,
        ancestor_id: usize,
        geometry: FilamentGeometry,
    ) -> Result<Self, CombBuilderError> {
        if id == 0 || ancestor_id == 0 {
            return Err(CombBuilderError::InvalidFilamentId);
        }
        Ok(Self {
            id,
            ancestor_id,
            geometry,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CombColumn {
    index: usize,
    x: usize,
    combs: Vec<FilamentComb>,
}

impl CombColumn {
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    #[must_use]
    pub const fn x(&self) -> usize {
        self.x
    }

    #[must_use]
    pub fn combs(&self) -> &[FilamentComb] {
        &self.combs
    }
}

/// Sample all interior columns and collect consecutive interline-compatible combs.
pub fn retrieve_combs(
    picture_width: usize,
    sampling_dx: usize,
    minimum_delta_y: isize,
    maximum_delta_y: isize,
    filaments: &[CombFilament],
) -> Result<Vec<CombColumn>, CombBuilderError> {
    if picture_width == 0 || sampling_dx == 0 || minimum_delta_y > maximum_delta_y {
        return Err(CombBuilderError::InvalidSampling);
    }

    let interval_count = (picture_width as f64 / sampling_dx as f64).round_ties_even() as usize;
    if interval_count == 0 {
        return Ok(Vec::new());
    }
    let sample_count = interval_count - 1;
    let precise_dx = picture_width as f64 / interval_count as f64;
    let mut columns = Vec::with_capacity(sample_count);

    for column_index in 1..=sample_count {
        let x = (precise_dx * column_index as f64).round_ties_even() as usize;
        let mut samples = Vec::new();
        for filament in filaments {
            if filament.geometry.is_within_range(x as f64) {
                samples.push((
                    filament.id,
                    filament.ancestor_id,
                    filament.geometry.position_at(x as f64)?,
                ));
            }
        }
        // Java Collections.sort is stable, and FilY compares only ordinates.
        samples.sort_by(|one, two| one.2.partial_cmp(&two.2).unwrap_or(Ordering::Equal));

        let mut combs = Vec::new();
        let mut active: Option<FilamentComb> = None;
        let mut previous: Option<(usize, usize, f64)> = None;
        for sample in samples {
            if let Some(prior) = previous {
                let delta = (sample.2 - prior.2).round_ties_even() as isize;
                if delta >= minimum_delta_y && delta <= maximum_delta_y {
                    if active.is_none() {
                        let mut comb = FilamentComb::new(column_index as i32);
                        comb.append(prior.0, prior.1, prior.2)?;
                        active = Some(comb);
                    }
                    active
                        .as_mut()
                        .expect("compatible delta starts a comb")
                        .append(sample.0, sample.1, sample.2)?;
                } else if let Some(comb) = active.take() {
                    combs.push(comb);
                }
            }
            previous = Some(sample);
        }
        if let Some(comb) = active {
            combs.push(comb);
        }
        columns.push(CombColumn {
            index: column_index,
            x,
            combs,
        });
    }

    Ok(columns)
}

/// Java `retrievePopularSize`: each comb votes with its own line count.
#[must_use]
pub fn popular_comb_size(columns: &[CombColumn]) -> Option<usize> {
    let mut histogram = Histogram::default();
    for comb in columns.iter().flat_map(|column| &column.combs) {
        let count = comb.count();
        histogram.increase_count(count, i32::try_from(count).unwrap_or(i32::MAX));
    }
    histogram.max_bucket().copied()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CombBuilderError {
    InvalidSampling,
    InvalidFilamentId,
    Filament(FilamentError),
    Comb(FilamentCombError),
}

impl From<FilamentError> for CombBuilderError {
    fn from(value: FilamentError) -> Self {
        Self::Filament(value)
    }
}

impl From<FilamentCombError> for CombBuilderError {
    fn from(value: FilamentCombError) -> Self {
        Self::Comb(value)
    }
}

impl fmt::Display for CombBuilderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSampling => formatter.write_str("invalid comb sampling parameters"),
            Self::InvalidFilamentId => formatter.write_str("comb filament IDs must be positive"),
            Self::Filament(error) => write!(formatter, "comb filament error: {error}"),
            Self::Comb(error) => write!(formatter, "comb state error: {error}"),
        }
    }
}

impl Error for CombBuilderError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        filament::StaffFilament,
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections},
    };

    fn sample(id: usize, y: usize, start: usize, length: usize) -> CombFilament {
        let mut table = RunTable::new(Orientation::Horizontal, start + length + 1, y + 2).unwrap();
        table.add_run(y, Run::new(start, length)).unwrap();
        let mut filament = StaffFilament::new(10).unwrap();
        filament
            .add_section(build_sections(&table, JunctionPolicy::All).remove(0))
            .unwrap();
        CombFilament::new(id, id, filament.geometry().unwrap()).unwrap()
    }

    #[test]
    fn samples_interior_columns_and_builds_consecutive_combs() {
        let filaments = [
            sample(1, 2, 0, 100),
            sample(2, 12, 0, 100),
            sample(3, 22, 0, 100),
            sample(4, 45, 0, 100),
        ];
        let columns = retrieve_combs(100, 25, 9, 11, &filaments).unwrap();

        assert_eq!(
            columns.iter().map(CombColumn::x).collect::<Vec<_>>(),
            [25, 50, 75]
        );
        for (index, column) in columns.iter().enumerate() {
            assert_eq!(column.index(), index + 1);
            assert_eq!(column.combs().len(), 1);
            assert_eq!(column.combs()[0].filament_ids(), [1, 2, 3]);
            assert!(
                column.combs()[0]
                    .ordinates()
                    .iter()
                    .zip([2.0_f64, 12.0, 22.0])
                    .all(|(actual, expected)| (*actual - expected).abs() < 1e-12)
            );
        }
    }

    #[test]
    fn restarts_after_a_gap_and_excludes_filaments_outside_the_sample() {
        let filaments = [
            sample(1, 2, 0, 100),
            sample(2, 12, 0, 100),
            sample(3, 30, 0, 100),
            sample(4, 40, 0, 100),
            sample(5, 50, 0, 40),
        ];
        let columns = retrieve_combs(100, 50, 10, 10, &filaments).unwrap();

        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].x(), 50);
        assert_eq!(columns[0].combs().len(), 2);
        assert_eq!(columns[0].combs()[0].filament_ids(), [1, 2]);
        assert_eq!(columns[0].combs()[1].filament_ids(), [3, 4]);
    }

    #[test]
    fn preserves_java_ties_even_interval_count_and_inclusive_delta_bounds() {
        let filaments = [sample(1, 2, 0, 50), sample(2, 12, 0, 50)];
        let columns = retrieve_combs(50, 20, 10, 10, &filaments).unwrap();

        // 50/20 = 2.5 rounds to the even interval count 2, leaving one sample.
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].x(), 25);
        assert_eq!(columns[0].combs()[0].count(), 2);
        assert_eq!(
            retrieve_combs(10, 0, 1, 2, &filaments),
            Err(CombBuilderError::InvalidSampling)
        );
    }

    #[test]
    fn popular_size_weights_each_comb_by_its_line_count_and_keeps_low_tie() {
        let mut two_a = FilamentComb::new(1);
        two_a.append_root(1, 1.0).unwrap();
        two_a.append_root(2, 2.0).unwrap();
        let mut two_b = FilamentComb::new(2);
        two_b.append_root(3, 1.0).unwrap();
        two_b.append_root(4, 2.0).unwrap();
        let mut four = FilamentComb::new(3);
        for id in 5..=8 {
            four.append_root(id, id as f64).unwrap();
        }
        let columns = [CombColumn {
            index: 1,
            x: 10,
            combs: vec![four, two_a, two_b],
        }];

        // Size 2 and size 4 each receive four votes; Histogram keeps size 2.
        assert_eq!(popular_comb_size(&columns), Some(2));
        assert_eq!(popular_comb_size(&[]), None);
    }
}
