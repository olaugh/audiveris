// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact headless port of `LinesRetriever.dispatchHorizontalSections`.
//!
//! Java iterates `hLag.getEntities()`, whose `ConcurrentSkipListMap` values are
//! ordered by section ID. Every registered section is dispatched: ownership by
//! a staff line is not considered here (that removal belongs to
//! `getAllStickers`). Each output retains the lag's ID order.

use crate::filament::FilamentInclusionDecision;
use crate::section::Section;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HorizontalSectionDispatch<'a> {
    pub thick: Vec<&'a Section>,
    pub thin: Vec<&'a Section>,
}

/// Dispatch all horizontal-lag sections using Java's strict weight boundary.
///
/// A section is thick only when `weight > max_thin_sticker_weight`; equality
/// remains thin. Input order is immaterial because Java's lag entity index
/// exposes values in ascending ID order.
#[must_use]
pub fn dispatch_horizontal_sections(
    sections: &[Section],
    max_thin_sticker_weight: usize,
) -> HorizontalSectionDispatch<'_> {
    let mut ordered = sections.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|section| section.id());

    let mut thick = Vec::new();
    let mut thin = Vec::new();
    for section in ordered {
        if section.weight() > max_thin_sticker_weight {
            thick.push(section);
        } else {
            thin.push(section);
        }
    }

    HorizontalSectionDispatch { thick, thin }
}

/// Geometry already sampled at the candidate section's horizontal midpoint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SectionInclusionEvidence {
    /// `section.getBounds().height`.
    pub entity_thickness: f64,
    /// `section.getBounds().y`.
    pub section_top: f64,
    /// `section.getCentroid2D().getY()`.
    pub centroid_y: f64,
    /// Staff-filament ordinate at the section bounds' integer midpoint.
    pub line_y_at_mid: f64,
    /// `scale.getFore()`.
    pub staff_foreground_thickness: f64,
    /// `Compounds.getThicknessAt(...)` for section plus filament.
    pub resulting_compound_thickness: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SectionInclusionThresholds {
    pub max_sticker_thickness: f64,
    pub max_sticker_gap: f64,
    pub max_sticker_extension: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SectionInclusionRejection {
    EntityThickness { observed: f64, maximum: f64 },
    CenterGap { observed: f64, maximum: f64 },
    Extension { observed: i32, maximum: i32 },
    CompoundThickness { observed: f64, maximum: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SectionInclusionDecision {
    Include,
    Reject(SectionInclusionRejection),
}

/// Pure decision equivalent of Java `LinesRetriever.canIncludeSection`.
///
/// Checks deliberately remain in Java source order so the rejection reason is
/// the first failed predicate. All four comparisons are strict: equality is
/// accepted. Extension uses `Math.rint` semantics (nearest integer, ties even)
/// before comparison with the integer limit.
#[must_use]
pub fn can_include_section(
    evidence: SectionInclusionEvidence,
    thresholds: SectionInclusionThresholds,
) -> SectionInclusionDecision {
    if evidence.entity_thickness > thresholds.max_sticker_thickness {
        return SectionInclusionDecision::Reject(SectionInclusionRejection::EntityThickness {
            observed: evidence.entity_thickness,
            maximum: thresholds.max_sticker_thickness,
        });
    }

    let center_delta = (evidence.line_y_at_mid - evidence.centroid_y).abs();
    let center_gap = center_delta - (evidence.staff_foreground_thickness / 2.0);
    if center_gap > thresholds.max_sticker_gap {
        return SectionInclusionDecision::Reject(SectionInclusionRejection::CenterGap {
            observed: center_gap,
            maximum: thresholds.max_sticker_gap,
        });
    }

    let section_bottom = evidence.section_top + evidence.entity_thickness;
    let extension = (evidence.line_y_at_mid - evidence.section_top)
        .abs()
        .max((section_bottom - evidence.line_y_at_mid).abs())
        .round_ties_even() as i32;
    if extension > thresholds.max_sticker_extension {
        return SectionInclusionDecision::Reject(SectionInclusionRejection::Extension {
            observed: extension,
            maximum: thresholds.max_sticker_extension,
        });
    }

    if evidence.resulting_compound_thickness > thresholds.max_sticker_thickness {
        return SectionInclusionDecision::Reject(SectionInclusionRejection::CompoundThickness {
            observed: evidence.resulting_compound_thickness,
            maximum: thresholds.max_sticker_thickness,
        });
    }

    SectionInclusionDecision::Include
}

/// Minimal section fields consumed by Java `includeSections` traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionAssignmentCandidate {
    pub id: usize,
    pub first_pos: usize,
    /// Integer centroid x from `Section.getCentroid()` (not `getCentroid2D()`).
    pub centroid_x: isize,
}

/// One staff line in original system/staff/line traversal order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SectionAssignmentLine {
    pub staff_id: usize,
    pub line_id: usize,
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: usize,
    pub max_y: usize,
    /// Original endpoint abscissae preserved across the deferred batch update.
    pub start_endpoint_x: f64,
    pub stop_endpoint_x: f64,
}

/// Lines belonging to one system; each system restarts its candidate scan.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionAssignmentSystem {
    pub system_id: usize,
    pub lines: Vec<SectionAssignmentLine>,
}

/// Intent corresponding to Java's post-batch `fil.setEndingPoints(...)` call.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EndpointRestorationIntent {
    pub start_x: f64,
    pub stop_x: f64,
    /// Both endpoint ordinates must be recomputed from the mutated filament at
    /// the preserved abscissae.
    pub recompute_y_at_preserved_x: bool,
}

/// A deferred set of sections to add to one line, followed by endpoint repair.
#[derive(Debug, Clone, PartialEq)]
pub struct LineSectionInclusionBatch {
    pub system_id: usize,
    pub staff_id: usize,
    pub line_id: usize,
    pub section_ids: Vec<usize>,
    pub restore_endpoints: EndpointRestorationIntent,
}

/// Port the traversal and assignment contract around `canIncludeSection`.
///
/// `decide` is called only after the Java y-window and inclusive centroid-x
/// gates pass. Returned sections are not marked included until the whole line
/// has been scanned, matching Java's deferred `stickers` batch.
#[must_use]
pub fn plan_section_inclusions(
    candidates: &[SectionAssignmentCandidate],
    systems: &[SectionAssignmentSystem],
    mut decide: impl FnMut(usize, usize, usize, usize) -> SectionInclusionDecision,
) -> Vec<LineSectionInclusionBatch> {
    // Collections.sort(List, Section.byPosition) is stable: equal firstPos
    // candidates retain their source order.
    let mut ordered = candidates.to_vec();
    ordered.sort_by_key(|candidate| candidate.first_pos);

    let mut included = HashSet::new();
    let mut batches = Vec::new();

    for system in systems {
        // Java resets iMin for each system because systems can be side by side.
        let mut i_min = 0;

        for line in &system.lines {
            let mut stickers = Vec::new();

            for (index, candidate) in ordered.iter().enumerate().skip(i_min) {
                // Java checks included IDs before updating iMin or y-windowing.
                if included.contains(&candidate.id) {
                    continue;
                }

                if candidate.first_pos < line.min_y {
                    // Deliberately retain the last skipped index, rather than
                    // index + 1, matching the original implementation.
                    i_min = index;
                    continue;
                }
                if candidate.first_pos > line.max_y {
                    break;
                }

                let center_x = candidate.centroid_x as f64;
                if center_x >= line.min_x
                    && center_x <= line.max_x
                    && decide(system.system_id, line.staff_id, line.line_id, candidate.id)
                        == SectionInclusionDecision::Include
                {
                    stickers.push(candidate.id);
                }
            }

            // Apply all additions only after the scan, then restore endpoint y
            // values at the original x values. The batch record preserves that
            // mutation boundary for a stateful filament implementation.
            included.extend(stickers.iter().copied());
            batches.push(LineSectionInclusionBatch {
                system_id: system.system_id,
                staff_id: line.staff_id,
                line_id: line.line_id,
                section_ids: stickers,
                restore_endpoints: EndpointRestorationIntent {
                    start_x: line.start_endpoint_x,
                    stop_x: line.stop_endpoint_x,
                    recompute_y_at_preserved_x: true,
                },
            });
        }
    }

    batches
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FilamentOwner {
    pub system_id: usize,
    pub staff_id: usize,
    pub line_id: usize,
}

/// Neutral candidate record consumed by `includeDiscardedFilaments` traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscardedFilamentCandidate {
    pub id: usize,
    pub top_ordinate: usize,
    /// Integer centroid x returned by Java `Filament.getCentroid()`.
    pub centroid_x: isize,
    pub initial_owner: Option<FilamentOwner>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FilamentEndpoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FilamentAssignmentLine {
    pub staff_id: usize,
    pub line_id: usize,
    pub min_y: usize,
    pub max_y: usize,
    pub start_endpoint: FilamentEndpoint,
    pub stop_endpoint: FilamentEndpoint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilamentAssignmentSystem {
    pub system_id: usize,
    pub lines: Vec<FilamentAssignmentLine>,
}

/// One immediate `filament.stealSections(candidate)` ownership transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilamentSteal {
    pub candidate_id: usize,
    pub new_owner: FilamentOwner,
}

/// One post-scan `setEndingPoints(startPt, stopPt)` cache/line recomputation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FilamentEndpointRecomputation {
    pub owner: FilamentOwner,
    pub preserved_start: FilamentEndpoint,
    pub preserved_stop: FilamentEndpoint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiscardedFilamentTraversal {
    /// Steals in their immediate execution order.
    pub steals: Vec<FilamentSteal>,
    /// At most one recomputation per modified target line.
    pub endpoint_recomputations: Vec<FilamentEndpointRecomputation>,
}

/// Port `LinesRetriever.includeDiscardedFilaments` traversal and ownership.
///
/// Java concatenates discarded candidates before sloped candidates and then
/// performs a stable top-ordinate sort. Accepted steals mark ownership
/// immediately; endpoint geometry is recomputed once after the line scan.
#[must_use]
pub fn plan_discarded_filament_inclusions(
    discarded: &[DiscardedFilamentCandidate],
    sloped: &[DiscardedFilamentCandidate],
    systems: &[FilamentAssignmentSystem],
    mut decide: impl FnMut(usize, usize, usize, usize) -> FilamentInclusionDecision,
) -> DiscardedFilamentTraversal {
    let mut candidates = Vec::with_capacity(discarded.len() + sloped.len());
    candidates.extend_from_slice(discarded);
    candidates.extend_from_slice(sloped);
    // Collections.sort is stable and topComparator compares only bounds.y.
    candidates.sort_by_key(|candidate| candidate.top_ordinate);

    let mut owned = candidates
        .iter()
        .filter(|candidate| candidate.initial_owner.is_some())
        .map(|candidate| candidate.id)
        .collect::<HashSet<_>>();
    let mut steals = Vec::new();
    let mut endpoint_recomputations = Vec::new();

    for system in systems {
        // Side-by-side systems restart from the top of the candidate list.
        let mut i_min = 0;

        for line in &system.lines {
            let owner = FilamentOwner {
                system_id: system.system_id,
                staff_id: line.staff_id,
                line_id: line.line_id,
            };
            let mut modified = false;

            for (index, candidate) in candidates.iter().enumerate().skip(i_min) {
                // Java checks getPartOf before the y window.
                if owned.contains(&candidate.id) {
                    continue;
                }
                if candidate.top_ordinate < line.min_y {
                    i_min = index;
                    continue;
                }
                if candidate.top_ordinate > line.max_y {
                    break;
                }

                let center_x = candidate.centroid_x as f64;
                if center_x >= line.start_endpoint.x
                    && center_x <= line.stop_endpoint.x
                    && decide(system.system_id, line.staff_id, line.line_id, candidate.id)
                        == FilamentInclusionDecision::Include
                {
                    // stealSections sets candidate.partOf immediately, before
                    // the next candidate or target line is considered.
                    owned.insert(candidate.id);
                    steals.push(FilamentSteal {
                        candidate_id: candidate.id,
                        new_owner: owner,
                    });
                    modified = true;
                }
            }

            if modified {
                endpoint_recomputations.push(FilamentEndpointRecomputation {
                    owner,
                    preserved_start: line.start_endpoint,
                    preserved_stop: line.stop_endpoint,
                });
            }
        }
    }

    DiscardedFilamentTraversal {
        steals,
        endpoint_recomputations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filament::FilamentInclusionRejection;
    use crate::run_table::{Orientation, Run, RunTable};
    use crate::section::{JunctionPolicy, build_sections_from_id};

    fn section(id: usize, weight: usize) -> Section {
        let mut table = RunTable::new(Orientation::Horizontal, weight.max(1), 1).unwrap();
        if weight > 0 {
            assert!(table.add_run(0, Run::new(0, weight)).unwrap());
        }
        build_sections_from_id(&table, JunctionPolicy::DEFAULT_RATIO, id).remove(0)
    }

    fn ids(sections: &[&Section]) -> Vec<usize> {
        sections.iter().map(|section| section.id()).collect()
    }

    fn inclusion_evidence() -> SectionInclusionEvidence {
        SectionInclusionEvidence {
            entity_thickness: 4.0,
            section_top: 8.0,
            centroid_y: 10.0,
            line_y_at_mid: 10.0,
            staff_foreground_thickness: 2.0,
            resulting_compound_thickness: 4.0,
        }
    }

    fn inclusion_thresholds() -> SectionInclusionThresholds {
        SectionInclusionThresholds {
            max_sticker_thickness: 4.0,
            max_sticker_gap: 1.0,
            max_sticker_extension: 2,
        }
    }

    #[test]
    fn strict_boundary_keeps_equal_weight_thin() {
        let sections = [section(1, 5), section(2, 6), section(3, 4)];
        let dispatch = dispatch_horizontal_sections(&sections, 5);

        assert_eq!(ids(&dispatch.thick), [2]);
        assert_eq!(ids(&dispatch.thin), [1, 3]);
    }

    #[test]
    fn outputs_follow_lag_id_order_not_geometry_or_input_order() {
        let sections = [
            section(30, 9),
            section(10, 3),
            section(40, 2),
            section(20, 8),
        ];
        let dispatch = dispatch_horizontal_sections(&sections, 5);

        assert_eq!(ids(&dispatch.thick), [20, 30]);
        assert_eq!(ids(&dispatch.thin), [10, 40]);
    }

    #[test]
    fn classifies_every_registered_section_without_owned_member_removal() {
        // IDs 2 and 4 can represent sections already owned by staff filaments.
        // dispatchHorizontalSections has no ownership filter and must retain all
        // four; removal happens only in the later getAllStickers path.
        let sections = [section(1, 2), section(2, 8), section(3, 3), section(4, 9)];
        let dispatch = dispatch_horizontal_sections(&sections, 5);

        assert_eq!(ids(&dispatch.thick), [2, 4]);
        assert_eq!(ids(&dispatch.thin), [1, 3]);
        assert_eq!(dispatch.thick.len() + dispatch.thin.len(), sections.len());
    }

    #[test]
    fn inclusion_accepts_every_exact_boundary() {
        let evidence = SectionInclusionEvidence {
            // Entity and compound thickness equal the limit.
            resulting_compound_thickness: 4.0,
            // abs(10 - 8) - 2 / 2 = 1, exactly the gap limit.
            centroid_y: 8.0,
            ..inclusion_evidence()
        };

        assert_eq!(
            can_include_section(evidence, inclusion_thresholds()),
            SectionInclusionDecision::Include
        );
    }

    #[test]
    fn inclusion_reports_first_failure_in_java_source_order() {
        let thresholds = inclusion_thresholds();
        let all_invalid = SectionInclusionEvidence {
            entity_thickness: 5.0,
            section_top: 0.0,
            centroid_y: 20.0,
            resulting_compound_thickness: 9.0,
            ..inclusion_evidence()
        };
        assert_eq!(
            can_include_section(all_invalid, thresholds),
            SectionInclusionDecision::Reject(SectionInclusionRejection::EntityThickness {
                observed: 5.0,
                maximum: 4.0,
            })
        );

        let bad_gap = SectionInclusionEvidence {
            centroid_y: 7.999_999,
            section_top: 7.0,
            resulting_compound_thickness: 9.0,
            ..inclusion_evidence()
        };
        assert!(matches!(
            can_include_section(bad_gap, thresholds),
            SectionInclusionDecision::Reject(SectionInclusionRejection::CenterGap { .. })
        ));

        let bad_extension = SectionInclusionEvidence {
            section_top: 7.0,
            resulting_compound_thickness: 9.0,
            ..inclusion_evidence()
        };
        assert_eq!(
            can_include_section(bad_extension, thresholds),
            SectionInclusionDecision::Reject(SectionInclusionRejection::Extension {
                observed: 3,
                maximum: 2,
            })
        );

        let bad_compound = SectionInclusionEvidence {
            resulting_compound_thickness: 4.000_001,
            ..inclusion_evidence()
        };
        assert!(matches!(
            can_include_section(bad_compound, thresholds),
            SectionInclusionDecision::Reject(SectionInclusionRejection::CompoundThickness { .. })
        ));
    }

    #[test]
    fn extension_rounding_matches_java_rint_ties_to_even() {
        let thresholds = SectionInclusionThresholds {
            max_sticker_extension: 2,
            ..inclusion_thresholds()
        };
        let rounds_down_to_even = SectionInclusionEvidence {
            entity_thickness: 5.0,
            section_top: 7.5,
            centroid_y: 10.0,
            resulting_compound_thickness: 4.0,
            ..inclusion_evidence()
        };
        let permissive_thickness = SectionInclusionThresholds {
            max_sticker_thickness: 5.0,
            ..thresholds
        };
        assert_eq!(
            can_include_section(rounds_down_to_even, permissive_thickness),
            SectionInclusionDecision::Include
        );

        let rounds_up_to_even = SectionInclusionEvidence {
            entity_thickness: 7.0,
            section_top: 6.5,
            centroid_y: 10.0,
            resulting_compound_thickness: 7.0,
            ..inclusion_evidence()
        };
        let limit_three = SectionInclusionThresholds {
            max_sticker_thickness: 7.0,
            max_sticker_extension: 3,
            ..thresholds
        };
        assert_eq!(
            can_include_section(rounds_up_to_even, limit_three),
            SectionInclusionDecision::Reject(SectionInclusionRejection::Extension {
                observed: 4,
                maximum: 3,
            })
        );
    }

    fn line(
        staff_id: usize,
        line_id: usize,
        min_x: f64,
        max_x: f64,
        min_y: usize,
        max_y: usize,
    ) -> SectionAssignmentLine {
        SectionAssignmentLine {
            staff_id,
            line_id,
            min_x,
            max_x,
            min_y,
            max_y,
            start_endpoint_x: min_x + 0.25,
            stop_endpoint_x: max_x - 0.25,
        }
    }

    const fn candidate(
        id: usize,
        first_pos: usize,
        centroid_x: isize,
    ) -> SectionAssignmentCandidate {
        SectionAssignmentCandidate {
            id,
            first_pos,
            centroid_x,
        }
    }

    #[test]
    fn traversal_is_stable_by_position_with_strict_y_and_inclusive_x_windows() {
        let candidates = [
            candidate(4, 10, 100),
            candidate(7, 21, 50),
            candidate(1, 9, 50),
            candidate(2, 10, 0),
            candidate(3, 10, 101),
            candidate(6, 20, 50),
            candidate(5, 10, 50),
        ];
        let systems = [SectionAssignmentSystem {
            system_id: 8,
            lines: vec![line(9, 10, 0.0, 100.0, 10, 20)],
        }];
        let mut evaluated = Vec::new();

        let batches = plan_section_inclusions(&candidates, &systems, |system, staff, line, id| {
            evaluated.push((system, staff, line, id));
            SectionInclusionDecision::Include
        });

        // Equal-position candidates retain source order (4, 2, 3, 5).
        assert_eq!(
            evaluated,
            [(8, 9, 10, 4), (8, 9, 10, 2), (8, 9, 10, 5), (8, 9, 10, 6)]
        );
        assert_eq!(batches[0].section_ids, [4, 2, 5, 6]);
        // x=0 and x=100 pass; x=101 is skipped. y=9 advances and y=21 breaks.
    }

    #[test]
    fn each_system_restarts_scan_and_included_ids_are_globally_suppressed() {
        let candidates = [candidate(1, 10, 50), candidate(2, 10, 75)];
        let systems = [
            SectionAssignmentSystem {
                system_id: 1,
                // This lower line advances iMin past the top candidates.
                lines: vec![line(1, 1, 0.0, 50.0, 30, 40)],
            },
            SectionAssignmentSystem {
                system_id: 2,
                // Side-by-side system at the same y must restart from index zero.
                lines: vec![line(2, 1, 50.0, 100.0, 10, 10)],
            },
            SectionAssignmentSystem {
                system_id: 3,
                // A duplicate side-by-side window must not claim IDs again.
                lines: vec![line(3, 1, 50.0, 100.0, 10, 10)],
            },
        ];

        let batches = plan_section_inclusions(&candidates, &systems, |_, _, _, _| {
            SectionInclusionDecision::Include
        });

        assert!(batches[0].section_ids.is_empty());
        assert_eq!(batches[1].section_ids, [1, 2]);
        assert!(batches[2].section_ids.is_empty());
    }

    #[test]
    fn rejected_candidates_remain_available_and_batches_restore_endpoints() {
        let candidates = [candidate(11, 5, 10), candidate(12, 5, 20)];
        let systems = [SectionAssignmentSystem {
            system_id: 3,
            lines: vec![line(4, 5, 0.0, 30.0, 5, 5), line(4, 6, 0.0, 30.0, 5, 5)],
        }];

        let batches =
            plan_section_inclusions(&candidates, &systems, |_, _, line_id, section_id| {
                if line_id == 5 && section_id == 11 {
                    SectionInclusionDecision::Reject(SectionInclusionRejection::CenterGap {
                        observed: 2.0,
                        maximum: 1.0,
                    })
                } else {
                    SectionInclusionDecision::Include
                }
            });

        // Line 5 selects ID 12 as one deferred batch. Rejected ID 11 remains
        // available to line 6, while included ID 12 is suppressed there.
        assert_eq!(batches[0].section_ids, [12]);
        assert_eq!(batches[1].section_ids, [11]);
        assert_eq!(
            batches[0].restore_endpoints,
            EndpointRestorationIntent {
                start_x: 0.25,
                stop_x: 29.75,
                recompute_y_at_preserved_x: true,
            }
        );
        assert!(batches[1].restore_endpoints.recompute_y_at_preserved_x);
    }

    fn filament_candidate(
        id: usize,
        top_ordinate: usize,
        centroid_x: isize,
    ) -> DiscardedFilamentCandidate {
        DiscardedFilamentCandidate {
            id,
            top_ordinate,
            centroid_x,
            initial_owner: None,
        }
    }

    fn filament_line(
        staff_id: usize,
        line_id: usize,
        min_x: f64,
        max_x: f64,
        min_y: usize,
        max_y: usize,
    ) -> FilamentAssignmentLine {
        FilamentAssignmentLine {
            staff_id,
            line_id,
            min_y,
            max_y,
            start_endpoint: FilamentEndpoint { x: min_x, y: 20.5 },
            stop_endpoint: FilamentEndpoint { x: max_x, y: 21.5 },
        }
    }

    #[test]
    fn discarded_then_sloped_concatenation_has_stable_top_sort() {
        let discarded = [filament_candidate(1, 10, 20), filament_candidate(2, 10, 20)];
        let sloped = [filament_candidate(3, 10, 20), filament_candidate(4, 5, 20)];
        let systems = [FilamentAssignmentSystem {
            system_id: 1,
            lines: vec![filament_line(2, 3, 0.0, 40.0, 0, 20)],
        }];
        let mut evaluated = Vec::new();

        let plan =
            plan_discarded_filament_inclusions(&discarded, &sloped, &systems, |_, _, _, id| {
                evaluated.push(id);
                FilamentInclusionDecision::Include
            });

        assert_eq!(evaluated, [4, 1, 2, 3]);
        assert_eq!(
            plan.steals
                .iter()
                .map(|steal| steal.candidate_id)
                .collect::<Vec<_>>(),
            [4, 1, 2, 3]
        );
    }

    #[test]
    fn filament_windows_are_strict_in_y_and_inclusive_in_x() {
        let discarded = [
            filament_candidate(1, 9, 20),
            filament_candidate(2, 10, 0),
            filament_candidate(3, 10, 100),
            filament_candidate(4, 10, 101),
            filament_candidate(5, 20, 50),
            filament_candidate(6, 21, 50),
        ];
        let systems = [FilamentAssignmentSystem {
            system_id: 1,
            lines: vec![filament_line(1, 1, 0.0, 100.0, 10, 20)],
        }];
        let mut evaluated = Vec::new();

        let plan = plan_discarded_filament_inclusions(&discarded, &[], &systems, |_, _, _, id| {
            evaluated.push(id);
            FilamentInclusionDecision::Include
        });

        assert_eq!(evaluated, [2, 3, 5]);
        assert_eq!(
            plan.steals
                .iter()
                .map(|steal| steal.candidate_id)
                .collect::<Vec<_>>(),
            [2, 3, 5]
        );
    }

    #[test]
    fn side_by_side_systems_restart_but_owned_candidates_are_skipped() {
        let mut initially_owned = filament_candidate(9, 10, 60);
        initially_owned.initial_owner = Some(FilamentOwner {
            system_id: 99,
            staff_id: 99,
            line_id: 99,
        });
        let discarded = [filament_candidate(1, 10, 50), initially_owned];
        let systems = [
            FilamentAssignmentSystem {
                system_id: 1,
                lines: vec![filament_line(1, 1, 0.0, 100.0, 30, 40)],
            },
            FilamentAssignmentSystem {
                system_id: 2,
                lines: vec![filament_line(2, 1, 0.0, 100.0, 10, 10)],
            },
            FilamentAssignmentSystem {
                system_id: 3,
                lines: vec![filament_line(3, 1, 0.0, 100.0, 10, 10)],
            },
        ];
        let mut evaluated = Vec::new();

        let plan =
            plan_discarded_filament_inclusions(&discarded, &[], &systems, |system, _, _, id| {
                evaluated.push((system, id));
                FilamentInclusionDecision::Include
            });

        // System 2 restarts and steals ID 1. System 3 sees its new owner; ID 9
        // is never evaluated because it arrived already owned.
        assert_eq!(evaluated, [(2, 1)]);
        assert_eq!(plan.steals.len(), 1);
        assert_eq!(plan.steals[0].new_owner.system_id, 2);
        assert_eq!(plan.endpoint_recomputations.len(), 1);
        assert_eq!(plan.endpoint_recomputations[0].owner.system_id, 2);
    }

    #[test]
    fn steals_are_immediate_but_line_recompute_occurs_once_when_modified() {
        let discarded = [
            filament_candidate(1, 10, 20),
            filament_candidate(2, 10, 30),
            filament_candidate(3, 10, 40),
        ];
        let systems = [FilamentAssignmentSystem {
            system_id: 7,
            lines: vec![
                filament_line(8, 9, 0.0, 50.0, 10, 10),
                filament_line(8, 10, 0.0, 50.0, 10, 10),
            ],
        }];

        let plan =
            plan_discarded_filament_inclusions(&discarded, &[], &systems, |_, _, line_id, id| {
                if line_id == 9 && id == 3 {
                    FilamentInclusionDecision::Reject(FilamentInclusionRejection::CenterGap {
                        observed: 2.0,
                        maximum: 1.0,
                    })
                } else {
                    FilamentInclusionDecision::Include
                }
            });

        // IDs 1 and 2 are owned immediately by line 9 and skipped by line 10;
        // rejected ID 3 remains available and is stolen by line 10.
        assert_eq!(
            plan.steals
                .iter()
                .map(|steal| (steal.new_owner.line_id, steal.candidate_id))
                .collect::<Vec<_>>(),
            [(9, 1), (9, 2), (10, 3)]
        );
        assert_eq!(plan.endpoint_recomputations.len(), 2);
        assert_eq!(plan.endpoint_recomputations[0].owner.line_id, 9);
        assert_eq!(
            plan.endpoint_recomputations[0].preserved_start,
            FilamentEndpoint { x: 0.0, y: 20.5 }
        );
        assert_eq!(plan.endpoint_recomputations[1].owner.line_id, 10);
    }
}
