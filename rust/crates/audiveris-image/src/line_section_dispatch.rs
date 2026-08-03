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
}
