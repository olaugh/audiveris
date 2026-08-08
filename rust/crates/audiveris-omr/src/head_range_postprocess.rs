// SPDX-License-Identifier: AGPL-3.0-or-later

//! Identity-free post-processing shared by Java's x-based head lookup.
//!
//! This module ports the two pure list operations between provisional range
//! matching and glyph retrieval: stable grade-first aggregation around fixed
//! center abscissae, then conflict filtering against seed-created heads.

use std::cmp::Ordering;

use crate::head_scanner_slices::JavaRectangle;

/// Java `NoteHeadsBuilder.Constants.gradeMargin`.
pub const SEED_GRADE_MARGIN: f64 = 0.1;
/// Java `NoteHeadsBuilder.Constants.minIouHeads`.
pub const MIN_SEED_HEAD_IOU: f64 = 0.1;

/// Geometry needed by range aggregation and seed-conflict filtering.
///
/// Indices returned by this module always refer to the caller's input slice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadPostprocessGeometry {
    pub bounds: JavaRectangle,
    pub grade: f64,
}

/// One Java `Aggregate`, in aggregate-creation order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadAggregate {
    /// First (and therefore highest-grade) member retained by Java.
    pub main_index: usize,
    /// Members in stable reverse-grade traversal order.
    pub member_indices: Vec<usize>,
}

/// The first seed head satisfying both inclusive conflict gates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeedConflict {
    pub seed_index: usize,
    pub iou: f64,
}

/// Java `filterSeedConflicts` expressed without mutating or owning heads.
#[derive(Clone, Debug, PartialEq)]
pub struct SeedConflictFilter {
    /// Range-head indices retained in their input order.
    pub retained_range_indices: Vec<usize>,
    /// Rejected range-head indices paired with their first qualifying seed.
    pub conflicts: Vec<RangeSeedConflict>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangeSeedConflict {
    pub range_index: usize,
    pub seed_index: usize,
    pub iou: f64,
}

/// Port Java `NoteHeadsBuilder.aggregateMatches`.
///
/// Java's object sort is stable. `Double.compare` is used in reverse order,
/// including its canonical-NaN and signed-zero ordering. Each new aggregate is
/// anchored forever at its first member's `GeoUtil.center2D`; later heads join
/// the first existing aggregate within the inclusive horizontal threshold.
#[must_use]
pub fn aggregate_matches(
    heads: &[HeadPostprocessGeometry],
    max_template_dx: i32,
) -> Vec<HeadAggregate> {
    let mut sorted_indices = (0..heads.len()).collect::<Vec<_>>();
    sorted_indices
        .sort_by(|&left, &right| java_double_compare(heads[right].grade, heads[left].grade));

    let maximum_dx = f64::from(max_template_dx);
    let mut aggregates = Vec::<HeadAggregate>::new();
    let mut aggregate_centers = Vec::<f64>::new();
    for index in sorted_indices {
        let center = center_x(heads[index].bounds);
        let aggregate_index = aggregate_centers
            .iter()
            .position(|&aggregate_center| (center - aggregate_center).abs() <= maximum_dx);
        if let Some(aggregate_index) = aggregate_index {
            aggregates[aggregate_index].member_indices.push(index);
        } else {
            aggregate_centers.push(center);
            aggregates.push(HeadAggregate {
                main_index: index,
                member_indices: vec![index],
            });
        }
    }
    aggregates
}

/// Port Java `NoteHeadsBuilder.overlapSeed` for an abscissa-sorted seed-head
/// slice, returning the first conflict rather than only a boolean.
#[must_use]
pub fn first_seed_conflict(
    range_head: HeadPostprocessGeometry,
    seed_heads_by_abscissa: &[HeadPostprocessGeometry],
) -> Option<SeedConflict> {
    // Preserve Java's parenthesization: headGrade * (1 - gradeMargin).
    let lowered_grade = range_head.grade * (1.0 - SEED_GRADE_MARGIN);
    let x_max = f64::from(range_head.bounds.x) + f64::from(range_head.bounds.width);

    for (seed_index, seed) in seed_heads_by_abscissa.iter().enumerate() {
        if seed.bounds.intersects(range_head.bounds) {
            let iou = java_rectangle_iou(seed.bounds, range_head.bounds);
            if iou >= MIN_SEED_HEAD_IOU && seed.grade >= lowered_grade {
                return Some(SeedConflict { seed_index, iou });
            }
        } else if f64::from(seed.bounds.x) > x_max {
            break;
        }
    }
    None
}

/// Port Java `NoteHeadsBuilder.filterSeedConflicts` while retaining provenance.
#[must_use]
pub fn filter_seed_conflicts(
    range_heads: &[HeadPostprocessGeometry],
    seed_heads_by_abscissa: &[HeadPostprocessGeometry],
) -> SeedConflictFilter {
    let mut retained_range_indices = Vec::new();
    let mut conflicts = Vec::new();
    for (range_index, &range_head) in range_heads.iter().enumerate() {
        if let Some(conflict) = first_seed_conflict(range_head, seed_heads_by_abscissa) {
            conflicts.push(RangeSeedConflict {
                range_index,
                seed_index: conflict.seed_index,
                iou: conflict.iou,
            });
        } else {
            retained_range_indices.push(range_index);
        }
    }
    SeedConflictFilter {
        retained_range_indices,
        conflicts,
    }
}

/// Port `GeoUtil.iou(Rectangle, Rectangle)`, including Java `int` area
/// overflow. Callers matching `overlapSeed` first require positive rectangle
/// intersection, but this helper deliberately preserves the standalone method.
#[must_use]
pub fn java_rectangle_iou(one: JavaRectangle, two: JavaRectangle) -> f64 {
    let intersection = java_rectangle_intersection(one, two);
    let intersection_area = intersection.width.wrapping_mul(intersection.height);
    if intersection_area == 0 {
        return 0.0;
    }

    let one_area = one.width.wrapping_mul(one.height);
    let two_area = two.width.wrapping_mul(two.height);
    let union_area = one_area
        .wrapping_add(two_area)
        .wrapping_sub(intersection_area);
    f64::from(intersection_area) / f64::from(union_area)
}

fn center_x(bounds: JavaRectangle) -> f64 {
    f64::from(bounds.x) + (f64::from(bounds.width) / 2.0)
}

fn java_double_compare(left: f64, right: f64) -> Ordering {
    if left < right {
        return Ordering::Less;
    }
    if left > right {
        return Ordering::Greater;
    }

    java_double_to_long_bits(left).cmp(&java_double_to_long_bits(right))
}

fn java_double_to_long_bits(value: f64) -> i64 {
    let bits = if value.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        value.to_bits()
    };
    bits as i64
}

fn java_rectangle_intersection(one: JavaRectangle, two: JavaRectangle) -> JavaRectangle {
    let left = one.x.max(two.x);
    let top = one.y.max(two.y);
    let right =
        (i64::from(one.x) + i64::from(one.width)).min(i64::from(two.x) + i64::from(two.width));
    let bottom =
        (i64::from(one.y) + i64::from(one.height)).min(i64::from(two.y) + i64::from(two.height));
    let width = (right - i64::from(left)).max(i64::from(i32::MIN)) as i32;
    let height = (bottom - i64::from(top)).max(i64::from(i32::MIN)) as i32;

    JavaRectangle::new(left, top, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(x: i32, y: i32, width: i32, height: i32, grade: f64) -> HeadPostprocessGeometry {
        HeadPostprocessGeometry {
            bounds: JavaRectangle::new(x, y, width, height),
            grade,
        }
    }

    #[test]
    fn aggregation_uses_stable_reverse_grade_first_aggregate_and_inclusive_dx() {
        let heads = [
            head(-1, 0, 2, 2, 0.8),   // center 0, first equal-grade main
            head(9, 0, 2, 2, 0.8),    // center 10, second aggregate
            head(4, 50, 2, 2, 0.7),   // center 5: joins first, not second
            head(13, -50, 2, 2, 0.6), // center 14: joins second
        ];
        assert_eq!(
            aggregate_matches(&heads, 5),
            vec![
                HeadAggregate {
                    main_index: 0,
                    member_indices: vec![0, 2],
                },
                HeadAggregate {
                    main_index: 1,
                    member_indices: vec![1, 3],
                },
            ]
        );
    }

    #[test]
    fn aggregation_matches_java_double_compare_nan_and_signed_zero_order() {
        let heads = [
            head(0, 0, 2, 2, f64::from_bits(0x7ff8_0000_0000_0001)),
            head(10, 0, 2, 2, f64::from_bits(0xfff8_0000_0000_0042)),
            head(20, 0, 2, 2, 1.0),
            head(30, 0, 2, 2, 0.0),
            head(40, 0, 2, 2, -0.0),
        ];
        assert_eq!(
            aggregate_matches(&heads, 0)
                .into_iter()
                .map(|aggregate| aggregate.main_index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4]
        );
    }

    #[test]
    fn higher_grade_member_becomes_main_even_when_later_in_source_order() {
        let heads = [head(0, 0, 3, 3, 0.2), head(2, 100, 3, 3, 0.9)];
        assert_eq!(
            aggregate_matches(&heads, 2),
            vec![HeadAggregate {
                main_index: 1,
                member_indices: vec![1, 0],
            }]
        );
    }

    #[test]
    fn seed_conflict_gates_are_inclusive_at_exact_iou_and_lowered_grade() {
        let range = head(0, 0, 10, 10, 0.8);
        let lowered_grade = range.grade * (1.0 - SEED_GRADE_MARGIN);
        let seed = head(0, 0, 10, 1, lowered_grade);
        let conflict = first_seed_conflict(range, &[seed]).unwrap();
        assert_eq!(conflict.seed_index, 0);
        assert_eq!(conflict.iou.to_bits(), MIN_SEED_HEAD_IOU.to_bits());

        let below_grade = f64::from_bits(lowered_grade.to_bits() - 1);
        assert!(first_seed_conflict(range, &[head(0, 0, 10, 1, below_grade)]).is_none());
        assert!(first_seed_conflict(range, &[head(0, 0, 9, 1, 1.0)]).is_none());
    }

    #[test]
    fn edge_contact_empty_and_negative_rectangles_do_not_conflict() {
        let range = head(0, 0, 10, 10, 0.5);
        assert!(first_seed_conflict(range, &[head(10, 0, 4, 4, 1.0)]).is_none());
        assert!(first_seed_conflict(range, &[head(0, 0, 0, 4, 1.0)]).is_none());
        assert!(first_seed_conflict(range, &[head(0, 0, -1, 4, 1.0)]).is_none());
    }

    #[test]
    fn conflict_uses_first_qualifying_seed_and_java_xmax_without_int_wrap() {
        let range = head(i32::MAX - 1, 0, 4, 1, 0.5);
        let seeds = [head(i32::MAX, 0, 1, 1, 1.0), head(i32::MAX, 0, 1, 1, 1.0)];
        let conflict = first_seed_conflict(range, &seeds).unwrap();
        assert_eq!(conflict.seed_index, 0);
        assert_eq!(conflict.iou, 0.25);
    }

    #[test]
    fn early_abscissa_break_is_observable_if_the_input_contract_is_violated() {
        let range = head(0, 0, 10, 10, 0.5);
        let not_abscissa_sorted = [head(11, 0, 1, 1, 1.0), head(0, 0, 10, 10, 1.0)];
        assert!(first_seed_conflict(range, &not_abscissa_sorted).is_none());
    }

    #[test]
    fn java_iou_preserves_wrapping_int_area_arithmetic() {
        let large = JavaRectangle::new(0, 0, 50_000, 50_000);
        let half = JavaRectangle::new(0, 0, 50_000, 25_000);
        let iou = java_rectangle_iou(large, half);
        assert!(iou.is_sign_negative());
        assert_eq!(iou.to_bits(), 0xbfe6_48d6_dd5f_0753);

        assert_eq!(java_rectangle_iou(large, large), 1.0);
    }

    #[test]
    fn filtering_retains_input_order_and_reports_each_first_conflict() {
        let ranges = [
            head(0, 0, 10, 10, 0.8),
            head(50, 0, 10, 10, 0.8),
            head(100, 0, 10, 10, 0.8),
            head(150, 0, -10, 10, 0.8),
        ];
        let seeds = [
            head(0, 0, 10, 1, 1.0),
            head(100, 0, 10, 1, 1.0),
            head(200, 0, 10, 1, 1.0),
        ];
        assert_eq!(
            filter_seed_conflicts(&ranges, &seeds),
            SeedConflictFilter {
                retained_range_indices: vec![1, 3],
                conflicts: vec![
                    RangeSeedConflict {
                        range_index: 0,
                        seed_index: 0,
                        iou: 0.1,
                    },
                    RangeSeedConflict {
                        range_index: 2,
                        seed_index: 1,
                        iou: 0.1,
                    },
                ],
            }
        );
    }
}
