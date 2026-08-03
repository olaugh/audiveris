// SPDX-License-Identifier: AGPL-3.0-or-later

//! Geometry-only port of Java `BeamGroupInter.groupBeams`.
//!
//! Group identities and SIG mutations remain owned by the caller.  This
//! kernel preserves Java's stable ordinate ordering, neighborhood thresholds,
//! provisional multi-membership, and first-ensemble merge order.

use crate::beam_structure::Segment;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BeamBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroupingBeam {
    pub id: usize,
    pub median: Segment,
    pub height: f64,
    /// Java `Inter.getBounds()`, retained explicitly to preserve integer
    /// `Rectangle.grow` and `Rectangle.intersects` behavior.
    pub bounds: BeamBounds,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamGroupParameters {
    pub min_x_overlap: f64,
    pub max_y_distance: f64,
    pub max_slope_diff: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeighborRejection {
    XOverlap,
    YDistance,
    SlopeDifference,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NeighborEvidence {
    pub one_id: usize,
    pub two_id: usize,
    pub x_overlap: f64,
    pub y_distance: Option<f64>,
    pub slope_difference: Option<f64>,
    pub rejection: Option<NeighborRejection>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BeamGroupEvent {
    Created {
        group_index: usize,
        beam_id: usize,
    },
    Added {
        group_index: usize,
        beam_id: usize,
        via_beam_id: usize,
    },
    Merged {
        survivor_index: usize,
        removed_index: usize,
        witness_beam_id: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeamGroupEvidence {
    pub ordered_beam_ids: Vec<usize>,
    pub neighbor_checks: Vec<NeighborEvidence>,
    /// Surviving groups in creation order, with members in relation insertion
    /// order. Group indices referenced by events are provisional indices.
    pub groups: Vec<Vec<usize>>,
    pub events: Vec<BeamGroupEvent>,
}

#[must_use]
pub fn can_be_neighbors(
    one: GroupingBeam,
    two: GroupingBeam,
    parameters: BeamGroupParameters,
) -> NeighborEvidence {
    let max_left = one.median.x1.max(two.median.x1);
    let min_right = one.median.x2.min(two.median.x2);
    let x_overlap = min_right - max_left;
    if x_overlap < parameters.min_x_overlap {
        return NeighborEvidence {
            one_id: one.id,
            two_id: two.id,
            x_overlap,
            y_distance: None,
            slope_difference: None,
            rejection: Some(NeighborRejection::XOverlap),
        };
    }

    let x = (max_left + min_right) / 2.0;
    let y_distance = (y_at_x(two.median, x) - y_at_x(one.median, x)).abs();
    if y_distance > parameters.max_y_distance {
        return NeighborEvidence {
            one_id: one.id,
            two_id: two.id,
            x_overlap,
            y_distance: Some(y_distance),
            slope_difference: None,
            rejection: Some(NeighborRejection::YDistance),
        };
    }

    let slope_difference = (slope(two.median) - slope(one.median)).abs();
    NeighborEvidence {
        one_id: one.id,
        two_id: two.id,
        x_overlap,
        y_distance: Some(y_distance),
        slope_difference: Some(slope_difference),
        rejection: (slope_difference > parameters.max_slope_diff || slope_difference.is_nan())
            .then_some(NeighborRejection::SlopeDifference),
    }
}

/// Produce the exact grouping plan without allocating group identities or
/// mutating the caller's graph.
#[must_use]
pub fn group_beam_evidence(
    beams: &[GroupingBeam],
    parameters: BeamGroupParameters,
) -> BeamGroupEvidence {
    let mut ordered = beams.iter().copied().enumerate().collect::<Vec<_>>();
    // Java's `Collections.sort` is stable and `Inters.byOrdinate` compares
    // only bounds.y. The source index therefore resolves equal ordinates.
    ordered.sort_by_key(|(source_index, beam)| (beam.bounds.y, *source_index));

    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut active: Vec<bool> = Vec::new();
    let mut memberships = vec![Vec::<usize>::new(); beams.len()];
    let mut neighbor_checks = Vec::new();
    let mut events = Vec::new();

    for i in 0..ordered.len() {
        let (source_i, beam) = ordered[i];
        let group_index = memberships[source_i].first().copied().unwrap_or_else(|| {
            let index = groups.len();
            groups.push(vec![beam.id]);
            active.push(true);
            memberships[source_i].push(index);
            events.push(BeamGroupEvent::Created {
                group_index: index,
                beam_id: beam.id,
            });
            index
        });

        let grow = (parameters.max_y_distance - (beam.height / 2.0)).ceil() as i32;
        let lookup = grow_vertical(beam.bounds, grow);
        let y_break = lookup.y.saturating_add(lookup.height);
        for &(source_j, other) in &ordered[(i + 1)..] {
            if rectangles_intersect(lookup, other.bounds) {
                let check = can_be_neighbors(beam, other, parameters);
                let accepted = check.rejection.is_none();
                neighbor_checks.push(check);
                if accepted && !memberships[source_j].contains(&group_index) {
                    memberships[source_j].push(group_index);
                    groups[group_index].push(other.id);
                    events.push(BeamGroupEvent::Added {
                        group_index,
                        beam_id: other.id,
                        via_beam_id: beam.id,
                    });
                }
            } else if other.bounds.y >= y_break {
                break;
            }
        }
    }

    // Java revisits beams in the sorted order. The first ensemble survives;
    // later ensembles and their member relations are moved in insertion order.
    for &(source_index, beam) in &ordered {
        let beam_groups = memberships[source_index]
            .iter()
            .copied()
            .filter(|index| active[*index])
            .collect::<Vec<_>>();
        let Some((&survivor, others)) = beam_groups.split_first() else {
            continue;
        };
        for &removed in others {
            if !active[removed] || removed == survivor {
                continue;
            }
            let moved = groups[removed].clone();
            for member_id in moved {
                if !groups[survivor].contains(&member_id) {
                    groups[survivor].push(member_id);
                }
                if let Some((member_index, _)) = beams
                    .iter()
                    .enumerate()
                    .find(|(_, member)| member.id == member_id)
                {
                    if !memberships[member_index].contains(&survivor) {
                        memberships[member_index].push(survivor);
                    }
                    memberships[member_index].retain(|index| *index != removed);
                }
            }
            groups[removed].clear();
            active[removed] = false;
            events.push(BeamGroupEvent::Merged {
                survivor_index: survivor,
                removed_index: removed,
                witness_beam_id: beam.id,
            });
        }
    }

    BeamGroupEvidence {
        ordered_beam_ids: ordered.iter().map(|(_, beam)| beam.id).collect(),
        neighbor_checks,
        groups: groups
            .into_iter()
            .zip(active)
            .filter_map(|(members, active)| active.then_some(members))
            .collect(),
        events,
    }
}

fn slope(line: Segment) -> f64 {
    (line.y2 - line.y1) / (line.x2 - line.x1)
}

fn y_at_x(line: Segment, x: f64) -> f64 {
    line.y1 + ((x - line.x1) * slope(line))
}

fn grow_vertical(bounds: BeamBounds, amount: i32) -> BeamBounds {
    BeamBounds {
        y: bounds.y.saturating_sub(amount),
        height: bounds.height.saturating_add(amount.saturating_mul(2)),
        ..bounds
    }
}

fn rectangles_intersect(one: BeamBounds, two: BeamBounds) -> bool {
    one.width > 0
        && one.height > 0
        && two.width > 0
        && two.height > 0
        && one.x < two.x.saturating_add(two.width)
        && two.x < one.x.saturating_add(one.width)
        && one.y < two.y.saturating_add(two.height)
        && two.y < one.y.saturating_add(one.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beam(id: usize, y: i32, x1: f64, x2: f64) -> GroupingBeam {
        GroupingBeam {
            id,
            median: Segment {
                x1,
                y1: f64::from(y) + 2.0,
                x2,
                y2: f64::from(y) + 2.0,
            },
            height: 4.0,
            bounds: BeamBounds {
                x: x1.floor() as i32,
                y,
                width: (x2 - x1).ceil() as i32,
                height: 4,
            },
        }
    }

    fn parameters() -> BeamGroupParameters {
        BeamGroupParameters {
            min_x_overlap: 7.0,
            max_y_distance: 12.0,
            max_slope_diff: 0.065,
        }
    }

    #[test]
    fn thresholds_are_inclusive_and_fail_in_java_order() {
        let one = beam(1, 0, 0.0, 20.0);
        let mut two = beam(2, 8, 13.0, 30.0);
        let accepted = can_be_neighbors(one, two, parameters());
        assert_eq!(accepted.x_overlap, 7.0);
        assert_eq!(accepted.y_distance, Some(8.0));
        assert_eq!(accepted.rejection, None);

        two.median.x1 = 13.01;
        assert_eq!(
            can_be_neighbors(one, two, parameters()).rejection,
            Some(NeighborRejection::XOverlap)
        );
        two.median.x1 = 13.0;
        two.median.y1 = 14.01;
        two.median.y2 = 14.01;
        assert_eq!(
            can_be_neighbors(one, two, parameters()).rejection,
            Some(NeighborRejection::YDistance)
        );
    }

    #[test]
    fn stable_ordinate_order_and_break_exclude_unreached_beams() {
        let evidence = group_beam_evidence(
            &[
                beam(2, 0, 0.0, 20.0),
                beam(1, 0, 2.0, 22.0),
                beam(3, 40, 0.0, 20.0),
            ],
            parameters(),
        );
        assert_eq!(evidence.ordered_beam_ids, vec![2, 1, 3]);
        assert_eq!(evidence.groups, vec![vec![2, 1], vec![3]]);
        assert_eq!(evidence.neighbor_checks.len(), 1);
    }

    #[test]
    fn bridge_beam_merges_provisional_groups_in_first_ensemble_order() {
        // 10 reaches 30, 20 reaches 30, while 10 and 20 do not overlap.
        let evidence = group_beam_evidence(
            &[
                beam(10, 0, 0.0, 20.0),
                beam(20, 5, 30.0, 50.0),
                beam(30, 10, 10.0, 40.0),
            ],
            parameters(),
        );
        assert_eq!(evidence.groups, vec![vec![10, 30, 20]]);
        assert!(evidence.events.contains(&BeamGroupEvent::Merged {
            survivor_index: 0,
            removed_index: 1,
            witness_beam_id: 30,
        }));
    }

    #[test]
    fn zero_length_median_fails_slope_without_panicking() {
        let one = beam(1, 0, 0.0, 20.0);
        let mut two = beam(2, 8, 13.0, 30.0);
        two.median.x2 = two.median.x1;
        let mut permissive = parameters();
        permissive.min_x_overlap = 0.0;
        assert_eq!(
            can_be_neighbors(one, two, permissive).rejection,
            Some(NeighborRejection::SlopeDifference)
        );
    }
}
