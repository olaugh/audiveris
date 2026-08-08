// SPDX-License-Identifier: AGPL-3.0-or-later

//! Pure port of Java `HeadSeedTally.analyze`.
//!
//! The caller flattens the tallies in Java traversal order: tally collection
//! order, then `HorizontalSide` enum order, then each side's `LinkedHashMap`
//! insertion order. Shape ordering must match Java `Shape` declaration order.

use std::collections::BTreeMap;

/// Java `HeadSeedTally.Constants.quorum` default.
pub const HEAD_SEED_TALLY_QUORUM: usize = 10;

/// Java `HorizontalSide`, in declaration and `EnumMap` iteration order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HeadSeedTallySide {
    Left,
    Right,
}

/// One identity-free entry from a Java tally's insertion-preserving side map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadSeedTallySample<Shape> {
    pub shape: Shape,
    pub side: HeadSeedTallySide,
    pub dx: f64,
    /// Java retains removed `HeadInter` keys until purge and also checks this
    /// flag during analysis itself.
    pub removed: bool,
}

/// One `HeadSeedScale.putDx` call, in Java `EnumMap` output order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadSeedScaleEntry<Shape> {
    pub shape: Shape,
    pub side: HeadSeedTallySide,
    pub dx: f64,
    /// Population cardinality at the quorum decision.
    pub cardinality: usize,
}

#[derive(Clone, Copy, Debug, Default)]
struct Population {
    cardinality: usize,
    sum: f64,
}

impl Population {
    fn include_value(&mut self, value: f64) {
        // Preserve Java `Population.includeValue`: increment and one ordered
        // binary64 addition per live sample. Do not use a regrouping sum.
        self.cardinality += 1;
        self.sum += value;
    }
}

/// Analyze live tally samples using Java grouping, quorum, and floating order.
///
/// `Shape::Ord` is the equivalent of Java `Shape`'s enum ordinal. The returned
/// vector is ordered first by that shape order and then left before right,
/// exactly as the two nested Java `EnumMap` traversals.
#[must_use]
pub fn analyze_head_seed_tallies<Shape>(
    samples: &[HeadSeedTallySample<Shape>],
) -> Vec<HeadSeedScaleEntry<Shape>>
where
    Shape: Copy + Ord,
{
    let mut populations = BTreeMap::<(Shape, HeadSeedTallySide), Population>::new();
    for sample in samples {
        if sample.removed {
            continue;
        }
        populations
            .entry((sample.shape, sample.side))
            .or_default()
            .include_value(sample.dx);
    }

    populations
        .into_iter()
        .filter(|(_, population)| population.cardinality >= HEAD_SEED_TALLY_QUORUM)
        .map(|((shape, side), population)| HeadSeedScaleEntry {
            shape,
            side,
            dx: population.sum / population.cardinality as f64,
            cardinality: population.cardinality,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    enum Shape {
        Breve,
        Whole,
        Void,
        Black,
    }

    fn sample(shape: Shape, side: HeadSeedTallySide, dx: f64) -> HeadSeedTallySample<Shape> {
        HeadSeedTallySample {
            shape,
            side,
            dx,
            removed: false,
        }
    }

    #[test]
    fn filters_removed_samples_and_applies_inclusive_quorum_per_bucket() {
        let mut samples = (0..9)
            .map(|index| sample(Shape::Black, HeadSeedTallySide::Left, index as f64))
            .collect::<Vec<_>>();
        samples.extend(
            (0..10).map(|index| sample(Shape::Black, HeadSeedTallySide::Right, (index + 1) as f64)),
        );
        samples.extend((0..10).map(|_| HeadSeedTallySample {
            shape: Shape::Void,
            side: HeadSeedTallySide::Left,
            dx: 99.0,
            removed: true,
        }));

        assert_eq!(
            analyze_head_seed_tallies(&samples),
            [HeadSeedScaleEntry {
                shape: Shape::Black,
                side: HeadSeedTallySide::Right,
                dx: 5.5,
                cardinality: 10,
            }]
        );
    }

    #[test]
    fn emits_java_shape_then_side_order_independently_of_first_occurrence() {
        let mut samples = Vec::new();
        for _ in 0..10 {
            samples.push(sample(Shape::Black, HeadSeedTallySide::Right, 4.0));
            samples.push(sample(Shape::Void, HeadSeedTallySide::Right, 3.0));
            samples.push(sample(Shape::Black, HeadSeedTallySide::Left, 2.0));
            samples.push(sample(Shape::Breve, HeadSeedTallySide::Left, 1.0));
        }

        assert_eq!(
            analyze_head_seed_tallies(&samples)
                .into_iter()
                .map(|entry| (entry.shape, entry.side))
                .collect::<Vec<_>>(),
            [
                (Shape::Breve, HeadSeedTallySide::Left),
                (Shape::Void, HeadSeedTallySide::Right),
                (Shape::Black, HeadSeedTallySide::Left),
                (Shape::Black, HeadSeedTallySide::Right),
            ]
        );
    }

    #[test]
    fn preserves_population_insertion_order_for_non_associative_sums() {
        let mut ordered = vec![
            sample(Shape::Whole, HeadSeedTallySide::Left, 1.0e16),
            sample(Shape::Whole, HeadSeedTallySide::Left, -1.0e16),
            sample(Shape::Whole, HeadSeedTallySide::Left, 1.0),
        ];
        ordered.extend((0..7).map(|_| sample(Shape::Whole, HeadSeedTallySide::Left, 0.0)));
        let ordered_mean = analyze_head_seed_tallies(&ordered)[0].dx;
        assert_eq!(ordered_mean.to_bits(), 0.1_f64.to_bits());

        ordered.swap(1, 2);
        let regrouped_mean = analyze_head_seed_tallies(&ordered)[0].dx;
        assert_eq!(regrouped_mean.to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn population_starts_at_java_positive_zero() {
        let samples = [sample(Shape::Whole, HeadSeedTallySide::Left, -0.0); 10];
        assert_eq!(analyze_head_seed_tallies(&samples)[0].dx.to_bits(), 0);
    }
}
