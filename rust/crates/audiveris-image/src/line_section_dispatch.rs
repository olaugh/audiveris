// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact headless port of `LinesRetriever.dispatchHorizontalSections`.
//!
//! Java iterates `hLag.getEntities()`, whose `ConcurrentSkipListMap` values are
//! ordered by section ID. Every registered section is dispatched: ownership by
//! a staff line is not considered here (that removal belongs to
//! `getAllStickers`). Each output retains the lag's ID order.

use crate::section::Section;

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

#[cfg(test)]
mod tests {
    use super::*;
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
}
