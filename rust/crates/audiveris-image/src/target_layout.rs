// SPDX-License-Identifier: AGPL-3.0-or-later

//! Immutable, cycle-free target page/system/staff layout containers.
//!
//! This is the dependency-light semantic core of Java `TargetPage`,
//! `TargetSystem`, and `TargetStaff`. Source `SystemInfo`/`Staff` references are
//! represented by stable IDs, while target lines are owned values.

use std::collections::HashSet;
use std::{error::Error, fmt};

use crate::target_line::TargetLine;

#[derive(Clone, Debug)]
pub struct TargetPage {
    width: f64,
    height: f64,
    systems: Vec<TargetSystem>,
}

impl TargetPage {
    pub fn new(
        width: f64,
        height: f64,
        systems: Vec<TargetSystem>,
    ) -> Result<Self, TargetLayoutError> {
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err(TargetLayoutError::InvalidPageDimensions);
        }

        let mut system_ids = HashSet::with_capacity(systems.len());
        let mut staff_ids = HashSet::new();
        for system in &systems {
            if !system_ids.insert(system.id) {
                return Err(TargetLayoutError::DuplicateSystemId(system.id));
            }
            if system.top < 0.0 || system.top > height || system.left < 0.0 || system.right > width
            {
                return Err(TargetLayoutError::SystemOutsidePage(system.id));
            }
            for staff in &system.staves {
                if !staff_ids.insert(staff.id) {
                    return Err(TargetLayoutError::DuplicateStaffId(staff.id));
                }
                if staff.top < system.top || staff.top > height {
                    return Err(TargetLayoutError::StaffOutsideSystem {
                        staff_id: staff.id,
                        system_id: system.id,
                    });
                }
                for (line_index, line) in staff.lines.iter().enumerate() {
                    if line.target_y() < 0.0 || line.target_y() > height {
                        return Err(TargetLayoutError::LineOutsidePage {
                            staff_id: staff.id,
                            line_index,
                        });
                    }
                }
            }
        }

        Ok(Self {
            width,
            height,
            systems,
        })
    }

    #[must_use]
    pub const fn width(&self) -> f64 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> f64 {
        self.height
    }

    /// Systems in caller append order; no geometric sorting is performed.
    #[must_use]
    pub fn systems(&self) -> &[TargetSystem] {
        &self.systems
    }
}

#[derive(Clone, Debug)]
pub struct TargetSystem {
    id: usize,
    top: f64,
    left: f64,
    right: f64,
    staves: Vec<TargetStaff>,
}

impl TargetSystem {
    pub fn new(
        id: usize,
        top: f64,
        left: f64,
        right: f64,
        staves: Vec<TargetStaff>,
    ) -> Result<Self, TargetLayoutError> {
        if id == 0 {
            return Err(TargetLayoutError::InvalidSystemId);
        }
        if !top.is_finite()
            || !left.is_finite()
            || !right.is_finite()
            || left < 0.0
            || right <= left
        {
            return Err(TargetLayoutError::InvalidSystemGeometry(id));
        }
        let mut ids = HashSet::with_capacity(staves.len());
        for staff in &staves {
            if staff.system_id != id {
                return Err(TargetLayoutError::StaffSystemMismatch {
                    staff_id: staff.id,
                    expected_system_id: id,
                    actual_system_id: staff.system_id,
                });
            }
            if !ids.insert(staff.id) {
                return Err(TargetLayoutError::DuplicateStaffId(staff.id));
            }
        }
        Ok(Self {
            id,
            top,
            left,
            right,
            staves,
        })
    }

    #[must_use]
    pub const fn id(&self) -> usize {
        self.id
    }

    #[must_use]
    pub const fn top(&self) -> f64 {
        self.top
    }

    #[must_use]
    pub const fn left(&self) -> f64 {
        self.left
    }

    #[must_use]
    pub const fn right(&self) -> f64 {
        self.right
    }

    /// Staves in caller append order.
    #[must_use]
    pub fn staves(&self) -> &[TargetStaff] {
        &self.staves
    }
}

#[derive(Clone, Debug)]
pub struct TargetStaff {
    id: usize,
    system_id: usize,
    top: f64,
    lines: Vec<TargetLine>,
}

impl TargetStaff {
    pub fn new(
        id: usize,
        system_id: usize,
        top: f64,
        lines: Vec<TargetLine>,
    ) -> Result<Self, TargetLayoutError> {
        if id == 0 {
            return Err(TargetLayoutError::InvalidStaffId);
        }
        if system_id == 0 {
            return Err(TargetLayoutError::InvalidSystemId);
        }
        if !top.is_finite() || top < 0.0 {
            return Err(TargetLayoutError::InvalidStaffTop(id));
        }
        Ok(Self {
            id,
            system_id,
            top,
            lines,
        })
    }

    #[must_use]
    pub const fn id(&self) -> usize {
        self.id
    }

    /// Stable replacement for Java's `TargetSystem` object back-reference.
    #[must_use]
    pub const fn system_id(&self) -> usize {
        self.system_id
    }

    #[must_use]
    pub const fn top(&self) -> f64 {
        self.top
    }

    /// Target lines in caller append order.
    #[must_use]
    pub fn lines(&self) -> &[TargetLine] {
        &self.lines
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetLayoutError {
    InvalidPageDimensions,
    InvalidSystemId,
    InvalidStaffId,
    InvalidSystemGeometry(usize),
    InvalidStaffTop(usize),
    DuplicateSystemId(usize),
    DuplicateStaffId(usize),
    StaffSystemMismatch {
        staff_id: usize,
        expected_system_id: usize,
        actual_system_id: usize,
    },
    SystemOutsidePage(usize),
    StaffOutsideSystem {
        staff_id: usize,
        system_id: usize,
    },
    LineOutsidePage {
        staff_id: usize,
        line_index: usize,
    },
}

impl fmt::Display for TargetLayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPageDimensions => formatter.write_str("invalid target page dimensions"),
            Self::InvalidSystemId => formatter.write_str("target system ID must be positive"),
            Self::InvalidStaffId => formatter.write_str("target staff ID must be positive"),
            Self::InvalidSystemGeometry(id) => {
                write!(formatter, "invalid geometry for target system {id}")
            }
            Self::InvalidStaffTop(id) => write!(formatter, "invalid top for target staff {id}"),
            Self::DuplicateSystemId(id) => write!(formatter, "duplicate target system ID {id}"),
            Self::DuplicateStaffId(id) => write!(formatter, "duplicate target staff ID {id}"),
            Self::StaffSystemMismatch {
                staff_id,
                expected_system_id,
                actual_system_id,
            } => write!(
                formatter,
                "staff {staff_id} belongs to system {actual_system_id}, not {expected_system_id}"
            ),
            Self::SystemOutsidePage(id) => {
                write!(formatter, "target system {id} lies outside its page")
            }
            Self::StaffOutsideSystem {
                staff_id,
                system_id,
            } => write!(
                formatter,
                "target staff {staff_id} lies outside system {system_id}"
            ),
            Self::LineOutsidePage {
                staff_id,
                line_index,
            } => write!(
                formatter,
                "target line {line_index} of staff {staff_id} lies outside its page"
            ),
        }
    }
}

impl Error for TargetLayoutError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        filament::StaffFilament,
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections},
    };

    fn line(y: f64, left: f64, right: f64) -> TargetLine {
        let mut table = RunTable::new(Orientation::Horizontal, 40, 5).unwrap();
        table.add_run(2, Run::new(0, 40)).unwrap();
        let mut filament = StaffFilament::new(10).unwrap();
        filament
            .add_section(build_sections(&table, JunctionPolicy::All).remove(0))
            .unwrap();
        TargetLine::new(filament.geometry().unwrap(), y, left, right).unwrap()
    }

    fn staff(id: usize, system_id: usize, top: f64) -> TargetStaff {
        TargetStaff::new(
            id,
            system_id,
            top,
            vec![line(top + 4.0, 10.0, 190.0), line(top, 10.0, 190.0)],
        )
        .unwrap()
    }

    #[test]
    fn preserves_system_staff_and_line_append_order() {
        let second = TargetSystem::new(
            2,
            100.0,
            10.0,
            190.0,
            vec![staff(20, 2, 100.0), staff(21, 2, 130.0)],
        )
        .unwrap();
        let first = TargetSystem::new(1, 20.0, 10.0, 190.0, vec![staff(10, 1, 20.0)]).unwrap();
        let page = TargetPage::new(200.0, 180.0, vec![second, first]).unwrap();

        assert_eq!((page.width(), page.height()), (200.0, 180.0));
        assert_eq!(
            page.systems()
                .iter()
                .map(TargetSystem::id)
                .collect::<Vec<_>>(),
            [2, 1]
        );
        assert_eq!(
            page.systems()[0]
                .staves()
                .iter()
                .map(TargetStaff::id)
                .collect::<Vec<_>>(),
            [20, 21]
        );
        assert_eq!(page.systems()[0].staves()[0].system_id(), 2);
        assert_eq!(
            page.systems()[0].staves()[0]
                .lines()
                .iter()
                .map(TargetLine::target_y)
                .collect::<Vec<_>>(),
            [104.0, 100.0]
        );
    }

    #[test]
    fn rejects_invalid_dimensions_bounds_tops_and_back_references() {
        assert!(matches!(
            TargetPage::new(f64::NAN, 100.0, vec![]),
            Err(TargetLayoutError::InvalidPageDimensions)
        ));
        assert!(matches!(
            TargetSystem::new(1, 0.0, 20.0, 20.0, vec![]),
            Err(TargetLayoutError::InvalidSystemGeometry(1))
        ));
        assert!(matches!(
            TargetStaff::new(1, 1, f64::INFINITY, vec![]),
            Err(TargetLayoutError::InvalidStaffTop(1))
        ));
        assert!(matches!(
            TargetSystem::new(1, 0.0, 0.0, 50.0, vec![staff(2, 9, 0.0)]),
            Err(TargetLayoutError::StaffSystemMismatch { .. })
        ));
    }

    #[test]
    fn page_rejects_duplicate_ids_and_out_of_page_geometry() {
        let duplicate_staff = TargetSystem::new(
            1,
            10.0,
            0.0,
            100.0,
            vec![staff(5, 1, 10.0), staff(5, 1, 20.0)],
        );
        assert!(matches!(
            duplicate_staff,
            Err(TargetLayoutError::DuplicateStaffId(5))
        ));

        let duplicate_systems = vec![
            TargetSystem::new(1, 10.0, 0.0, 100.0, vec![]).unwrap(),
            TargetSystem::new(1, 50.0, 0.0, 100.0, vec![]).unwrap(),
        ];
        assert!(matches!(
            TargetPage::new(100.0, 100.0, duplicate_systems),
            Err(TargetLayoutError::DuplicateSystemId(1))
        ));

        let outside = TargetSystem::new(1, 10.0, 0.0, 120.0, vec![staff(5, 1, 10.0)]).unwrap();
        assert!(matches!(
            TargetPage::new(100.0, 100.0, vec![outside]),
            Err(TargetLayoutError::SystemOutsidePage(1))
        ));

        let above_system = TargetSystem::new(1, 20.0, 0.0, 100.0, vec![staff(5, 1, 10.0)]).unwrap();
        assert!(matches!(
            TargetPage::new(100.0, 100.0, vec![above_system]),
            Err(TargetLayoutError::StaffOutsideSystem { .. })
        ));

        let high_line = TargetStaff::new(5, 1, 90.0, vec![line(110.0, 0.0, 100.0)]).unwrap();
        let high_line_system = TargetSystem::new(1, 90.0, 0.0, 100.0, vec![high_line]).unwrap();
        assert!(matches!(
            TargetPage::new(100.0, 100.0, vec![high_line_system]),
            Err(TargetLayoutError::LineOutsidePage { .. })
        ));
    }
}
