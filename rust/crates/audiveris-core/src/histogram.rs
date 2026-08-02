// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Histogram<K> {
    buckets: BTreeMap<K, i32>,
    total_count: i32,
}

impl<K> Default for Histogram<K> {
    fn default() -> Self {
        Self {
            buckets: BTreeMap::new(),
            total_count: 0,
        }
    }
}

impl<K: Ord> Histogram<K> {
    pub fn clear(&mut self) {
        self.buckets.clear();
        self.total_count = 0;
    }

    pub fn increase_count(&mut self, bucket: K, delta: i32) {
        *self.buckets.entry(bucket).or_default() += delta;
        self.total_count += delta;
    }

    #[must_use]
    pub fn count(&self, bucket: &K) -> i32 {
        self.buckets.get(bucket).copied().unwrap_or_default()
    }

    #[must_use]
    pub fn total_count(&self) -> i32 {
        self.total_count
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.buckets.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }

    pub fn first_bucket(&self) -> Option<&K> {
        self.buckets.first_key_value().map(|(key, _)| key)
    }

    pub fn last_bucket(&self) -> Option<&K> {
        self.buckets.last_key_value().map(|(key, _)| key)
    }

    pub fn max_bucket(&self) -> Option<&K> {
        self.maximum().map(|(key, _)| key)
    }

    pub fn max_count(&self) -> Option<i32> {
        self.buckets.values().copied().max()
    }

    pub fn maximum(&self) -> Option<(&K, i32)> {
        // Java keeps the first (lowest-key) bucket when counts tie.
        self.buckets.iter().fold(None, |best, (key, &count)| {
            if best.is_none_or(|(_, best_count)| count > best_count) {
                Some((key, count))
            } else {
                best
            }
        })
    }

    pub fn buckets(&self) -> impl Iterator<Item = &K> {
        self.buckets.keys()
    }

    pub fn entries(&self) -> impl Iterator<Item = (&K, i32)> {
        self.buckets.iter().map(|(key, &count)| (key, count))
    }

    pub fn values(&self) -> impl Iterator<Item = i32> + '_ {
        self.buckets.values().copied()
    }

    #[must_use]
    pub fn quorum_value(&self, quorum_ratio: f64) -> i32 {
        // Java Math.rint uses ties-to-even, as does f64::round_ties_even.
        (quorum_ratio * f64::from(self.total_count)).round_ties_even() as i32
    }
}

impl<K: Ord + fmt::Display> Histogram<K> {
    #[must_use]
    pub fn data_string(&self) -> String {
        let values = self
            .buckets
            .iter()
            .map(|(key, count)| format!("{key}={count}"))
            .collect::<Vec<_>>()
            .join(" ");
        format!("[{values}]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn java_fixture() -> Histogram<i32> {
        let mut histogram = Histogram::default();
        for (bucket, count) in [(3, 2), (4, 10), (5, 12), (8, 3), (10, 6), (11, 0)] {
            histogram.increase_count(bucket, count);
        }
        histogram
    }

    #[test]
    fn ports_java_bucket_and_count_cases() {
        let histogram = java_fixture();
        assert_eq!(
            histogram.buckets().copied().collect::<Vec<_>>(),
            [3, 4, 5, 8, 10, 11]
        );
        assert_eq!(histogram.values().collect::<Vec<_>>(), [2, 10, 12, 3, 6, 0]);
        assert_eq!(histogram.first_bucket(), Some(&3));
        assert_eq!(histogram.last_bucket(), Some(&11));
        assert_eq!(histogram.count(&4), 10);
        assert_eq!(histogram.count(&9), 0);
        assert_eq!(histogram.total_count(), 33);
        assert_eq!(histogram.len(), 6);
    }

    #[test]
    fn ports_java_maximum_and_increase_cases() {
        let mut histogram = java_fixture();
        assert_eq!(histogram.max_bucket(), Some(&5));
        assert_eq!(histogram.max_count(), Some(12));
        assert_eq!(histogram.maximum(), Some((&5, 12)));
        histogram.increase_count(10, 50);
        assert_eq!(histogram.count(&10), 56);
        assert_eq!(histogram.total_count(), 83);

        let mut tied = Histogram::default();
        tied.increase_count(4, 7);
        tied.increase_count(5, 7);
        assert_eq!(tied.max_bucket(), Some(&4));
    }

    #[test]
    fn formats_and_clears() {
        let mut histogram = java_fixture();
        assert_eq!(histogram.data_string(), "[3=2 4=10 5=12 8=3 10=6 11=0]");
        assert_eq!(histogram.quorum_value(0.5), 16);
        histogram.clear();
        assert!(histogram.is_empty());
        assert_eq!(histogram.total_count(), 0);
    }
}
