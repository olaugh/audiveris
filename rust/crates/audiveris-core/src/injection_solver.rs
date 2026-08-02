// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionError {
    EmptyDomain,
    RangeTooSmall,
}

impl fmt::Display for InjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDomain => f.write_str("injection domain is empty"),
            Self::RangeTooSmall => f.write_str("injection range is smaller than its domain"),
        }
    }
}

impl Error for InjectionError {}

pub fn solve<F>(
    domain_size: usize,
    range_size: usize,
    distance: F,
) -> Result<(Vec<usize>, i32), InjectionError>
where
    F: Fn(usize, usize) -> i32,
{
    if domain_size == 0 {
        return Err(InjectionError::EmptyDomain);
    }
    if range_size < domain_size {
        return Err(InjectionError::RangeTooSmall);
    }

    struct Search<'a, F> {
        domain_size: usize,
        range_size: usize,
        distance: &'a F,
        free: Vec<bool>,
        current: Vec<usize>,
        best: Vec<usize>,
        best_cost: i32,
    }

    impl<F> Search<'_, F>
    where
        F: Fn(usize, usize) -> i32,
    {
        fn inspect(&mut self, domain: usize, cost: i32) {
            for range in 0..self.range_size {
                if !self.free[range] {
                    continue;
                }
                self.free[range] = false;
                self.current[domain] = range;
                let new_cost = cost + (self.distance)(domain, range);
                if domain + 1 < self.domain_size {
                    self.inspect(domain + 1, new_cost);
                } else if new_cost < self.best_cost {
                    self.best.clone_from(&self.current);
                    self.best_cost = new_cost;
                }
                self.free[range] = true;
            }
        }
    }

    let mut search = Search {
        domain_size,
        range_size,
        distance: &distance,
        free: vec![true; range_size],
        current: vec![0; domain_size],
        best: vec![0; domain_size],
        best_cost: i32::MAX,
    };
    search.inspect(0, 0);
    Ok((search.best, search.best_cost))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_java_solver_case_and_tie_order() {
        let (mapping, cost) = solve(3, 3, |domain, range| {
            (i32::try_from(domain + 1).unwrap() - i32::try_from(range).unwrap()).abs()
        })
        .unwrap();
        assert_eq!(mapping, [0, 1, 2]);
        assert_eq!(cost, 3);
    }

    #[test]
    fn solves_rectangular_assignment() {
        let (mapping, cost) = solve(2, 3, |domain, range| {
            let targets = [2, 0];
            (targets[domain] - i32::try_from(range).unwrap()).abs()
        })
        .unwrap();
        assert_eq!(mapping, [2, 0]);
        assert_eq!(cost, 0);
    }

    #[test]
    fn rejects_impossible_shapes() {
        assert_eq!(solve(0, 0, |_, _| 0), Err(InjectionError::EmptyDomain));
        assert_eq!(solve(2, 1, |_, _| 0), Err(InjectionError::RangeTooSmall));
    }
}
