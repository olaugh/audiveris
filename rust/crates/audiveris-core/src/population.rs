// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Population {
    sum: f64,
    squares_sum: f64,
    count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyPopulation;

impl fmt::Display for EmptyPopulation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("population is empty")
    }
}

impl Error for EmptyPopulation {}

impl Default for Population {
    fn default() -> Self {
        Self {
            sum: 0.0,
            squares_sum: 0.0,
            count: 0,
        }
    }
}

impl Population {
    #[must_use]
    pub fn cardinality(&self) -> usize {
        self.count
    }

    pub fn include(&mut self, value: f64) {
        self.count += 1;
        self.sum += value;
        self.squares_sum += value * value;
    }

    pub fn include_population(&mut self, other: &Self) {
        self.count += other.count;
        self.sum += other.sum;
        self.squares_sum += other.squares_sum;
    }

    pub fn exclude(&mut self, value: f64) -> Result<(), EmptyPopulation> {
        if self.count == 0 {
            return Err(EmptyPopulation);
        }
        self.count -= 1;
        self.sum -= value;
        self.squares_sum -= value * value;
        Ok(())
    }

    pub fn mean(&self) -> Result<f64, EmptyPopulation> {
        if self.count == 0 {
            Err(EmptyPopulation)
        } else {
            Ok(self.sum / self.count as f64)
        }
    }

    pub fn variance(&self) -> Result<f64, EmptyPopulation> {
        if self.count == 0 {
            return Err(EmptyPopulation);
        }
        if self.count == 1 {
            return Ok(0.0);
        }
        let n = self.count as f64;
        let biased = ((self.squares_sum - self.sum * self.sum / n) / n).max(0.0);
        Ok(n * biased / (n - 1.0))
    }

    pub fn standard_deviation(&self) -> Result<f64, EmptyPopulation> {
        Ok(self.variance()?.sqrt())
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn reset_to(&mut self, value: f64) {
        self.reset();
        self.include(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_empty_and_singleton_cases() {
        let mut population = Population::default();
        assert_eq!(population.cardinality(), 0);
        assert_eq!(population.mean(), Err(EmptyPopulation));
        assert_eq!(population.variance(), Err(EmptyPopulation));
        assert_eq!(population.exclude(123.0), Err(EmptyPopulation));
        population.include(123.0);
        assert_eq!(population.mean().unwrap(), 123.0);
        assert_eq!(population.variance().unwrap(), 0.0);
        assert_eq!(population.standard_deviation().unwrap(), 0.0);
    }

    #[test]
    fn ports_include_and_exclude_cases() {
        let mut population = Population::default();
        for value in [5.0, 6.0, 8.0, 9.0] {
            population.include(value);
        }
        assert_eq!(population.cardinality(), 4);
        assert_eq!(population.mean().unwrap(), 7.0);
        assert!((population.variance().unwrap() - 10.0 / 3.0).abs() < 1e-12);
        population.exclude(5.0).unwrap();
        population.exclude(9.0).unwrap();
        assert_eq!(population.mean().unwrap(), 7.0);
        assert!((population.variance().unwrap() - 2.0).abs() < 1e-12);
    }
}
