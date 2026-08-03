// SPDX-License-Identifier: AGPL-3.0-or-later

//! Headless ownership boundary for Java `SystemManager.populateSystems`.
//!
//! Population is deliberately non-transactional. Java mutates coordinates,
//! areas, section containers, indentation flags, pages, and page references in
//! order; an unchecked failure leaves every earlier mutation visible.

/// Stable identity used at the sheet/system boundary.
pub type SystemId = usize;

/// Calls made by `SystemManager.populateSystems`, in production order.
pub trait SystemPopulationExecutor {
    type Error;

    /// System identities in `sheet.getSystems()` order.
    fn system_ids(&self) -> Vec<SystemId>;

    /// Staff identities in `system.getStaves()` order.
    fn staff_ids(&self, system_id: SystemId) -> Vec<usize>;

    fn update_system_coordinates(&mut self, system_id: SystemId) -> Result<(), Self::Error>;
    fn compute_system_area(&mut self, system_id: SystemId) -> Result<(), Self::Error>;
    fn compute_staff_area(&mut self, staff_id: usize) -> Result<(), Self::Error>;
    fn dispatch_horizontal_sections(&mut self) -> Result<(), Self::Error>;
    fn dispatch_vertical_sections(&mut self) -> Result<(), Self::Error>;
    fn check_indentations(&mut self) -> Result<(), Self::Error>;
    fn allocate_pages(&mut self) -> Result<(), Self::Error>;
    fn report_results(&mut self) -> Result<(), Self::Error>;
}

/// Execute the exact outer lifecycle of Java `SystemManager.populateSystems`.
///
/// The system list is fetched again for the staff-area pass, matching the two
/// separate enhanced-for loops in Java. No rollback or finalizer exists.
pub fn populate_systems<Executor>(executor: &mut Executor) -> Result<(), Executor::Error>
where
    Executor: SystemPopulationExecutor,
{
    for system_id in executor.system_ids() {
        executor.update_system_coordinates(system_id)?;
        executor.compute_system_area(system_id)?;
    }

    for system_id in executor.system_ids() {
        for staff_id in executor.staff_ids(system_id) {
            executor.compute_staff_area(staff_id)?;
        }
    }

    executor.dispatch_horizontal_sections()?;
    executor.dispatch_vertical_sections()?;
    executor.check_indentations()?;
    executor.allocate_pages()?;
    executor.report_results()?;
    Ok(())
}

/// One lag section at the ownership boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopulationSection {
    pub id: usize,
    pub centroid_x: f64,
    pub centroid_y: f64,
}

/// Section containers owned by one system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemSectionOwnership {
    pub system_id: SystemId,
    pub horizontal_sections: Vec<usize>,
    pub vertical_sections: Vec<usize>,
}

/// Which production lag is being dispatched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopulationLag {
    Horizontal,
    Vertical,
}

/// Reproduce both Java section dispatch methods.
///
/// Every system container for the selected lag is cleared first. Sections are
/// then visited in lag entity order and linked to every containing system in
/// system order. A centroid on no system is silently left unowned.
pub fn dispatch_sections(
    lag: PopulationLag,
    systems: &mut [SystemSectionOwnership],
    sections: &[PopulationSection],
    mut contains: impl FnMut(SystemId, f64, f64) -> bool,
) {
    for system in systems.iter_mut() {
        match lag {
            PopulationLag::Horizontal => system.horizontal_sections.clear(),
            PopulationLag::Vertical => system.vertical_sections.clear(),
        }
    }

    for section in sections {
        for system in systems.iter_mut() {
            if contains(system.system_id, section.centroid_x, section.centroid_y) {
                match lag {
                    PopulationLag::Horizontal => system.horizontal_sections.push(section.id),
                    PopulationLag::Vertical => system.vertical_sections.push(section.id),
                }
            }
        }
    }
}

/// Geometry needed by Java's indentation decision after system areas exist.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopulationSystemGeometry {
    pub system_id: SystemId,
    pub left: i32,
    pub width: i32,
    pub top: i32,
    pub bottom: i32,
    pub area_left: i32,
    /// Deskewed x coordinate of `(left, top)`.
    pub deskewed_upper_left_x: f64,
}

impl PopulationSystemGeometry {
    fn x_overlaps(self, other: Self) -> bool {
        let common_left = self.left.max(other.left);
        let common_right = (self.left + self.width - 1).min(other.left + other.width - 1);
        common_right > common_left
    }

    fn y_overlaps(self, other: Self) -> bool {
        self.top.max(other.top) < self.bottom.min(other.bottom)
    }
}

/// Exact result of Java `checkIndentation` using already-deskewed coordinates.
#[must_use]
pub fn check_population_indentation(
    systems: &[PopulationSystemGeometry],
    system_index: usize,
    minimum_indentation: f64,
) -> bool {
    let current = systems[system_index];
    // A system beside another system is only eligible when it owns the left
    // edge of its area slice.
    if current.area_left != 0 {
        return false;
    }

    for direction in [-1_isize, 1] {
        let neighbors = vertical_neighbors(systems, system_index, direction);
        if let Some(other_index) = neighbors.first() {
            let delta = current.deskewed_upper_left_x - systems[*other_index].deskewed_upper_left_x;
            if delta >= minimum_indentation {
                return true;
            }
        }
    }
    false
}

fn vertical_neighbors(
    systems: &[PopulationSystemGeometry],
    current_index: usize,
    direction: isize,
) -> Vec<usize> {
    let current = systems[current_index];
    let mut cursor = current_index as isize + direction;
    let mut first = None;
    while cursor >= 0 && (cursor as usize) < systems.len() {
        if current.x_overlaps(systems[cursor as usize]) {
            first = Some(cursor as usize);
            break;
        }
        cursor += direction;
    }

    let Some(first) = first else {
        return Vec::new();
    };
    let mut neighbors = vec![first];
    for horizontal_direction in [-1_isize, 1] {
        let mut next = first;
        while let Some(found) = horizontal_neighbor(systems, next, horizontal_direction) {
            neighbors.push(found);
            next = found;
        }
    }
    // Java sorts the collected row by SystemInfo.byId, and callers use the
    // first item rather than necessarily the initially intersecting item.
    neighbors.sort_by_key(|index| systems[*index].system_id);
    neighbors
}

fn horizontal_neighbor(
    systems: &[PopulationSystemGeometry],
    current_index: usize,
    direction: isize,
) -> Option<usize> {
    let current = systems[current_index];
    let mut cursor = current_index as isize + direction;
    while cursor >= 0 && (cursor as usize) < systems.len() {
        if current.y_overlaps(systems[cursor as usize]) {
            return Some(cursor as usize);
        }
        cursor += direction;
    }
    None
}

/// System state consumed and mutated by Java `allocatePages`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulationSystem<Reference> {
    pub id: SystemId,
    pub indented: bool,
    pub system_ref: Reference,
    pub page_id: Option<usize>,
}

/// Physical page allocated on the sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulationPage {
    pub id: usize,
    pub first_system_id: Option<SystemId>,
    pub last_system_id: Option<SystemId>,
    pub system_ids: Vec<SystemId>,
}

/// Soft page reference allocated on the sheet stub.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulationPageRef<Reference> {
    pub id: usize,
    pub movement_start: bool,
    pub systems: Vec<Reference>,
}

/// Allocate pages and page references exactly as Java `allocatePages` does.
///
/// Existing pages and references are retained. This matters because the Java
/// helper itself does not clear them; callers such as `rebuildPages` do so.
/// System references are appended in sheet order, and an indented system closes
/// the preceding page before starting a movement page.
pub fn allocate_population_pages<Reference: Clone>(
    systems: &mut [PopulationSystem<Reference>],
    pages: &mut Vec<PopulationPage>,
    page_refs: &mut Vec<PopulationPageRef<Reference>>,
) {
    let mut active_page_index: Option<usize> = None;
    let mut active_ref_index: Option<usize> = None;

    for index in 0..systems.len() {
        let system_id = systems[index].id;
        let page_id = 1 + pages.len();

        if systems[index].indented {
            if let Some(page_index) = active_page_index {
                pages[page_index].last_system_id = Some(system_id - 1);
                set_page_systems_from(&mut pages[page_index], systems);
            }

            page_refs.push(PopulationPageRef {
                id: page_id,
                movement_start: true,
                systems: Vec::new(),
            });
            pages.push(PopulationPage {
                id: page_id,
                first_system_id: (system_id != 1).then_some(system_id),
                last_system_id: None,
                system_ids: Vec::new(),
            });
            active_page_index = Some(pages.len() - 1);
            active_ref_index = Some(page_refs.len() - 1);
        } else if active_page_index.is_none() {
            page_refs.push(PopulationPageRef {
                id: page_id,
                movement_start: false,
                systems: Vec::new(),
            });
            pages.push(PopulationPage {
                id: page_id,
                first_system_id: None,
                last_system_id: None,
                system_ids: Vec::new(),
            });
            active_page_index = Some(pages.len() - 1);
            active_ref_index = Some(page_refs.len() - 1);
        }

        let page_index = active_page_index.expect("the first system always allocates a page");
        let page_ref_index =
            active_ref_index.expect("physical and soft pages are allocated together");
        systems[index].page_id = Some(pages[page_index].id);
        page_refs[page_ref_index]
            .systems
            .push(systems[index].system_ref.clone());
    }

    if let Some(page_index) = active_page_index {
        set_page_systems_from(&mut pages[page_index], systems);
    }
}

fn set_page_systems_from<Reference>(
    page: &mut PopulationPage,
    systems: &[PopulationSystem<Reference>],
) {
    let first = page.first_system_id.unwrap_or(1) - 1;
    let last = page.last_system_id.unwrap_or(systems.len()) - 1;
    page.system_ids = systems[first..=last]
        .iter()
        .map(|system| system.id)
        .collect();
}

/// Values computed by Java `reportResults` for one page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulationPageReport {
    pub page_id: usize,
    pub part_count: usize,
    pub system_count: usize,
    pub tablature_count: usize,
}

/// One system's counts used by the reporting-only tail of population.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PopulationSystemCounts {
    pub system_id: SystemId,
    pub part_count: usize,
    pub tablature_count: usize,
}

/// Compute the maxima reported for a page by Java `reportResults`.
#[must_use]
pub fn population_page_report(
    page: &PopulationPage,
    systems: &[PopulationSystemCounts],
) -> PopulationPageReport {
    let mut part_count = 0;
    let mut tablature_count = 0;
    for system_id in &page.system_ids {
        if let Some(system) = systems.iter().find(|system| system.system_id == *system_id) {
            part_count = part_count.max(system.part_count);
            tablature_count = tablature_count.max(system.tablature_count);
        }
    }
    PopulationPageReport {
        page_id: page.id,
        part_count,
        system_count: page.system_ids.len(),
        tablature_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Call {
        Update(SystemId),
        SystemArea(SystemId),
        StaffArea(usize),
        Horizontal,
        Vertical,
        Indentations,
        Pages,
        Report,
    }

    #[derive(Default)]
    struct RecordingPopulation {
        calls: Vec<Call>,
        fail_at: Option<Call>,
    }

    impl RecordingPopulation {
        fn record(&mut self, call: Call) -> Result<(), &'static str> {
            self.calls.push(call.clone());
            if self.fail_at == Some(call) {
                Err("population failure")
            } else {
                Ok(())
            }
        }
    }

    impl SystemPopulationExecutor for RecordingPopulation {
        type Error = &'static str;

        fn system_ids(&self) -> Vec<SystemId> {
            vec![1, 2]
        }

        fn staff_ids(&self, system_id: SystemId) -> Vec<usize> {
            match system_id {
                1 => vec![10, 11],
                2 => vec![20],
                _ => Vec::new(),
            }
        }

        fn update_system_coordinates(&mut self, id: SystemId) -> Result<(), Self::Error> {
            self.record(Call::Update(id))
        }

        fn compute_system_area(&mut self, id: SystemId) -> Result<(), Self::Error> {
            self.record(Call::SystemArea(id))
        }

        fn compute_staff_area(&mut self, id: usize) -> Result<(), Self::Error> {
            self.record(Call::StaffArea(id))
        }

        fn dispatch_horizontal_sections(&mut self) -> Result<(), Self::Error> {
            self.record(Call::Horizontal)
        }

        fn dispatch_vertical_sections(&mut self) -> Result<(), Self::Error> {
            self.record(Call::Vertical)
        }

        fn check_indentations(&mut self) -> Result<(), Self::Error> {
            self.record(Call::Indentations)
        }

        fn allocate_pages(&mut self) -> Result<(), Self::Error> {
            self.record(Call::Pages)
        }

        fn report_results(&mut self) -> Result<(), Self::Error> {
            self.record(Call::Report)
        }
    }

    #[test]
    fn lifecycle_matches_java_order() {
        let mut population = RecordingPopulation::default();
        assert_eq!(populate_systems(&mut population), Ok(()));
        assert_eq!(
            population.calls,
            [
                Call::Update(1),
                Call::SystemArea(1),
                Call::Update(2),
                Call::SystemArea(2),
                Call::StaffArea(10),
                Call::StaffArea(11),
                Call::StaffArea(20),
                Call::Horizontal,
                Call::Vertical,
                Call::Indentations,
                Call::Pages,
                Call::Report,
            ]
        );
    }

    #[test]
    fn failure_retains_prior_mutation_and_stops_immediately() {
        let mut population = RecordingPopulation {
            fail_at: Some(Call::Vertical),
            ..RecordingPopulation::default()
        };
        assert_eq!(populate_systems(&mut population), Err("population failure"));
        assert_eq!(population.calls.last(), Some(&Call::Vertical));
        assert!(!population.calls.contains(&Call::Indentations));
        assert!(!population.calls.contains(&Call::Pages));
    }

    #[test]
    fn section_dispatch_clears_then_links_all_containing_systems_in_order() {
        let mut systems = vec![
            SystemSectionOwnership {
                system_id: 1,
                horizontal_sections: vec![99],
                vertical_sections: vec![88],
            },
            SystemSectionOwnership {
                system_id: 2,
                horizontal_sections: vec![98],
                vertical_sections: vec![87],
            },
        ];
        let sections = [
            PopulationSection {
                id: 4,
                centroid_x: 5.0,
                centroid_y: 10.0,
            },
            PopulationSection {
                id: 7,
                centroid_x: 5.0,
                centroid_y: 20.0,
            },
            PopulationSection {
                id: 9,
                centroid_x: 5.0,
                centroid_y: 99.0,
            },
        ];
        dispatch_sections(
            PopulationLag::Horizontal,
            &mut systems,
            &sections,
            |id, _, y| (id == 1 && y <= 20.0) || (id == 2 && (10.0..=20.0).contains(&y)),
        );
        assert_eq!(systems[0].horizontal_sections, [4, 7]);
        assert_eq!(systems[1].horizontal_sections, [4, 7]);
        assert_eq!(systems[0].vertical_sections, [88]);
        assert_eq!(systems[1].vertical_sections, [87]);

        dispatch_sections(
            PopulationLag::Vertical,
            &mut systems,
            &sections[..1],
            |id, _, _| id == 2,
        );
        assert_eq!(systems[0].vertical_sections, []);
        assert_eq!(systems[1].vertical_sections, [4]);
        assert_eq!(systems[0].horizontal_sections, [4, 7]);
    }

    #[test]
    fn page_allocation_splits_movements_and_links_both_ownership_trees() {
        let mut systems = vec![
            PopulationSystem {
                id: 1,
                indented: false,
                system_ref: "s1",
                page_id: None,
            },
            PopulationSystem {
                id: 2,
                indented: false,
                system_ref: "s2",
                page_id: None,
            },
            PopulationSystem {
                id: 3,
                indented: true,
                system_ref: "s3",
                page_id: None,
            },
            PopulationSystem {
                id: 4,
                indented: false,
                system_ref: "s4",
                page_id: None,
            },
            PopulationSystem {
                id: 5,
                indented: true,
                system_ref: "s5",
                page_id: None,
            },
        ];
        let mut pages = Vec::new();
        let mut refs = Vec::new();
        allocate_population_pages(&mut systems, &mut pages, &mut refs);

        assert_eq!(
            systems
                .iter()
                .map(|system| system.page_id)
                .collect::<Vec<_>>(),
            [Some(1), Some(1), Some(2), Some(2), Some(3)]
        );
        assert_eq!(
            pages
                .iter()
                .map(|page| page.system_ids.clone())
                .collect::<Vec<_>>(),
            [vec![1, 2], vec![3, 4], vec![5]]
        );
        assert_eq!(
            pages
                .iter()
                .map(|page| page.first_system_id)
                .collect::<Vec<_>>(),
            [None, Some(3), Some(5)]
        );
        assert_eq!(
            pages
                .iter()
                .map(|page| page.last_system_id)
                .collect::<Vec<_>>(),
            [Some(2), Some(4), None]
        );
        assert_eq!(
            refs.iter()
                .map(|page| page.movement_start)
                .collect::<Vec<_>>(),
            [false, true, true]
        );
        assert_eq!(
            refs.iter()
                .map(|page| page.systems.clone())
                .collect::<Vec<_>>(),
            [vec!["s1", "s2"], vec!["s3", "s4"], vec!["s5"]]
        );
    }

    #[test]
    fn indentation_uses_strict_overlap_and_inclusive_shift_threshold() {
        let systems = [
            PopulationSystemGeometry {
                system_id: 1,
                left: 10,
                width: 100,
                top: 0,
                bottom: 40,
                area_left: 0,
                deskewed_upper_left_x: 10.0,
            },
            PopulationSystemGeometry {
                system_id: 2,
                left: 30,
                width: 80,
                top: 50,
                bottom: 90,
                area_left: 0,
                deskewed_upper_left_x: 30.0,
            },
        ];
        assert!(check_population_indentation(&systems, 1, 20.0));
        assert!(!check_population_indentation(&systems, 1, 20.01));
    }

    #[test]
    fn side_by_side_system_with_nonzero_left_area_end_cannot_be_indented() {
        let systems = [
            PopulationSystemGeometry {
                system_id: 1,
                left: 0,
                width: 60,
                top: 0,
                bottom: 40,
                area_left: 0,
                deskewed_upper_left_x: 0.0,
            },
            PopulationSystemGeometry {
                system_id: 2,
                left: 80,
                width: 60,
                top: 0,
                bottom: 40,
                area_left: 70,
                deskewed_upper_left_x: 80.0,
            },
        ];
        assert!(!check_population_indentation(&systems, 1, 1.0));
    }

    #[test]
    fn first_indented_system_is_a_movement_page_without_first_id_override() {
        let mut systems = vec![PopulationSystem {
            id: 1,
            indented: true,
            system_ref: 10,
            page_id: None,
        }];
        let mut pages = Vec::new();
        let mut refs = Vec::new();
        allocate_population_pages(&mut systems, &mut pages, &mut refs);
        assert_eq!(pages[0].first_system_id, None);
        assert_eq!(pages[0].system_ids, [1]);
        assert!(refs[0].movement_start);
    }

    #[test]
    fn page_report_uses_max_parts_and_tablatures_across_systems() {
        let page = PopulationPage {
            id: 2,
            first_system_id: Some(3),
            last_system_id: Some(4),
            system_ids: vec![3, 4],
        };
        let report = population_page_report(
            &page,
            &[
                PopulationSystemCounts {
                    system_id: 3,
                    part_count: 2,
                    tablature_count: 1,
                },
                PopulationSystemCounts {
                    system_id: 4,
                    part_count: 4,
                    tablature_count: 3,
                },
            ],
        );
        assert_eq!(
            report,
            PopulationPageReport {
                page_id: 2,
                part_count: 4,
                system_count: 2,
                tablature_count: 3
            }
        );
    }
}
