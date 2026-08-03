// SPDX-License-Identifier: AGPL-3.0-or-later

//! Stable-ID semantic core of Java `grid.FilamentComb`.
//!
//! Java links each comb back into mutable `StaffFilament` objects. This neutral
//! representation instead keeps parallel filament IDs, resolved ancestor IDs,
//! and ordinates. It preserves append order and avoids object cycles.

use std::collections::HashSet;
use std::{error::Error, fmt};

/// A vertical sequence of filament samples taken at one column.
#[derive(Clone, Debug, PartialEq)]
pub struct FilamentComb {
    column: i32,
    filament_ids: Vec<usize>,
    ancestor_ids: Vec<usize>,
    ordinates: Vec<f64>,
    processed: bool,
}

impl FilamentComb {
    #[must_use]
    pub const fn new(column: i32) -> Self {
        Self {
            column,
            filament_ids: Vec::new(),
            ancestor_ids: Vec::new(),
            ordinates: Vec::new(),
            processed: false,
        }
    }

    /// Restore a comb from parallel persisted or adapter-owned state.
    pub fn from_parallel(
        column: i32,
        filament_ids: Vec<usize>,
        ancestor_ids: Vec<usize>,
        ordinates: Vec<f64>,
        processed: bool,
    ) -> Result<Self, FilamentCombError> {
        if filament_ids.len() != ancestor_ids.len() || filament_ids.len() != ordinates.len() {
            return Err(FilamentCombError::ParallelLengthMismatch {
                filament_ids: filament_ids.len(),
                ancestor_ids: ancestor_ids.len(),
                ordinates: ordinates.len(),
            });
        }

        let comb = Self {
            column,
            filament_ids,
            ancestor_ids,
            ordinates,
            processed,
        };
        comb.validate_entries()?;
        Ok(comb)
    }

    /// Append one sample, preserving caller order exactly as Java does.
    pub fn append(
        &mut self,
        filament_id: usize,
        ancestor_id: usize,
        ordinate: f64,
    ) -> Result<(), FilamentCombError> {
        validate_entry(self.filament_ids.len(), filament_id, ancestor_id, ordinate)?;
        if self.filament_ids.contains(&filament_id) {
            return Err(FilamentCombError::DuplicateFilamentId(filament_id));
        }
        self.filament_ids.push(filament_id);
        self.ancestor_ids.push(ancestor_id);
        self.ordinates.push(ordinate);
        Ok(())
    }

    /// Append a filament which is currently its own ancestor.
    pub fn append_root(
        &mut self,
        filament_id: usize,
        ordinate: f64,
    ) -> Result<(), FilamentCombError> {
        self.append(filament_id, filament_id, ordinate)
    }

    #[must_use]
    pub const fn column(&self) -> i32 {
        self.column
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.filament_ids.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.filament_ids.is_empty()
    }

    #[must_use]
    pub fn filament_id(&self, index: usize) -> Option<usize> {
        self.filament_ids.get(index).copied()
    }

    #[must_use]
    pub fn filament_ids(&self) -> &[usize] {
        &self.filament_ids
    }

    #[must_use]
    pub fn ancestor_id(&self, index: usize) -> Option<usize> {
        self.ancestor_ids.get(index).copied()
    }

    #[must_use]
    pub fn ancestor_ids(&self) -> &[usize] {
        &self.ancestor_ids
    }

    #[must_use]
    pub fn y(&self, index: usize) -> Option<f64> {
        self.ordinates.get(index).copied()
    }

    #[must_use]
    pub fn ordinates(&self) -> &[f64] {
        &self.ordinates
    }

    /// Return the first entry with the provided stored, resolved ancestor ID.
    #[must_use]
    pub fn index_of_ancestor(&self, ancestor_id: usize) -> Option<usize> {
        self.ancestor_ids
            .iter()
            .position(|candidate| *candidate == ancestor_id)
    }

    /// Resolve current ancestry at lookup time, matching Java after merges.
    ///
    /// The resolver receives the queried filament ID and then each comb member
    /// ID in append order. It must return each filament's current stable
    /// ancestor ID.
    pub fn index_of_with_ancestors(
        &self,
        filament_id: usize,
        mut ancestor_of: impl FnMut(usize) -> usize,
    ) -> Option<usize> {
        let ancestor_id = ancestor_of(filament_id);
        self.filament_ids
            .iter()
            .position(|candidate| ancestor_of(*candidate) == ancestor_id)
    }

    #[must_use]
    pub const fn is_processed(&self) -> bool {
        self.processed
    }

    pub const fn set_processed(&mut self, processed: bool) {
        self.processed = processed;
    }

    fn validate_entries(&self) -> Result<(), FilamentCombError> {
        let mut seen = HashSet::with_capacity(self.filament_ids.len());
        for (index, ((filament_id, ancestor_id), ordinate)) in self
            .filament_ids
            .iter()
            .zip(&self.ancestor_ids)
            .zip(&self.ordinates)
            .enumerate()
        {
            validate_entry(index, *filament_id, *ancestor_id, *ordinate)?;
            if !seen.insert(*filament_id) {
                return Err(FilamentCombError::DuplicateFilamentId(*filament_id));
            }
        }
        Ok(())
    }
}

fn validate_entry(
    index: usize,
    filament_id: usize,
    ancestor_id: usize,
    ordinate: f64,
) -> Result<(), FilamentCombError> {
    if filament_id == 0 {
        return Err(FilamentCombError::InvalidFilamentId { index });
    }
    if ancestor_id == 0 {
        return Err(FilamentCombError::InvalidAncestorId { index });
    }
    if !ordinate.is_finite() {
        return Err(FilamentCombError::NonFiniteOrdinate { index });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilamentCombError {
    ParallelLengthMismatch {
        filament_ids: usize,
        ancestor_ids: usize,
        ordinates: usize,
    },
    InvalidFilamentId {
        index: usize,
    },
    InvalidAncestorId {
        index: usize,
    },
    NonFiniteOrdinate {
        index: usize,
    },
    DuplicateFilamentId(usize),
}

impl fmt::Display for FilamentCombError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParallelLengthMismatch {
                filament_ids,
                ancestor_ids,
                ordinates,
            } => write!(
                formatter,
                "parallel comb lengths differ: {filament_ids} filament IDs, {ancestor_ids} ancestor IDs, {ordinates} ordinates"
            ),
            Self::InvalidFilamentId { index } => {
                write!(formatter, "filament ID at index {index} is zero")
            }
            Self::InvalidAncestorId { index } => {
                write!(formatter, "ancestor ID at index {index} is zero")
            }
            Self::NonFiniteOrdinate { index } => {
                write!(formatter, "ordinate at index {index} is not finite")
            }
            Self::DuplicateFilamentId(id) => write!(formatter, "duplicate filament ID {id}"),
        }
    }
}

impl Error for FilamentCombError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_preserves_parallel_order_and_access() {
        let mut comb = FilamentComb::new(17);
        comb.append_root(30, 12.5).unwrap();
        comb.append(10, 100, 4.25).unwrap();
        comb.append(20, 200, 8.0).unwrap();

        assert_eq!(comb.column(), 17);
        assert_eq!(comb.count(), 3);
        assert!(!comb.is_empty());
        assert_eq!(comb.filament_ids(), [30, 10, 20]);
        assert_eq!(comb.ancestor_ids(), [30, 100, 200]);
        assert_eq!(comb.ordinates(), [12.5, 4.25, 8.0]);
        assert_eq!(comb.filament_id(1), Some(10));
        assert_eq!(comb.ancestor_id(1), Some(100));
        assert_eq!(comb.y(1), Some(4.25));
        assert_eq!(comb.filament_id(3), None);
        assert_eq!(comb.y(3), None);
    }

    #[test]
    fn ancestor_lookup_returns_first_stored_or_current_match() {
        let comb = FilamentComb::from_parallel(
            4,
            vec![10, 20, 30],
            vec![100, 200, 100],
            vec![2.0, 4.0, 6.0],
            false,
        )
        .unwrap();
        assert_eq!(comb.index_of_ancestor(100), Some(0));
        assert_eq!(comb.index_of_ancestor(999), None);

        let current_ancestor = |id| match id {
            10 => 500,
            20 | 99 => 600,
            30 => 700,
            _ => id,
        };
        assert_eq!(comb.index_of_with_ancestors(99, current_ancestor), Some(1));
        assert_eq!(comb.index_of_with_ancestors(42, current_ancestor), None);
    }

    #[test]
    fn processed_flag_round_trips() {
        let mut comb = FilamentComb::new(0);
        assert!(!comb.is_processed());
        comb.set_processed(true);
        assert!(comb.is_processed());
        comb.set_processed(false);
        assert!(!comb.is_processed());
    }

    #[test]
    fn rejects_invalid_parallel_and_entry_state() {
        assert_eq!(
            FilamentComb::from_parallel(1, vec![1], vec![], vec![2.0], false),
            Err(FilamentCombError::ParallelLengthMismatch {
                filament_ids: 1,
                ancestor_ids: 0,
                ordinates: 1,
            })
        );
        assert_eq!(
            FilamentComb::from_parallel(1, vec![1, 1], vec![1, 1], vec![2.0, 3.0], false,),
            Err(FilamentCombError::DuplicateFilamentId(1))
        );

        let mut comb = FilamentComb::new(1);
        assert_eq!(
            comb.append(0, 1, 2.0),
            Err(FilamentCombError::InvalidFilamentId { index: 0 })
        );
        assert_eq!(
            comb.append(1, 0, 2.0),
            Err(FilamentCombError::InvalidAncestorId { index: 0 })
        );
        assert_eq!(
            comb.append(1, 1, f64::NAN),
            Err(FilamentCombError::NonFiniteOrdinate { index: 0 })
        );
    }
}
