// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light shell of Java `HeaderBuilder.processHeader`.

use std::error::Error;
use std::fmt;

use crate::{
    headers_step::{HeaderRecognizer, HeadlessHeaderSystem},
    staff_header::{HeaderBounds, StaffHeader},
};

/// Java `HeaderBuilder.Constants.maxHeaderWidth` before scale conversion.
pub const MAXIMUM_HEADER_WIDTH_INTERLINES: f64 = 15.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeaderBarline {
    pub id: usize,
    pub bounds: Option<HeaderBounds>,
    pub staff_id: Option<usize>,
    pub dummy: bool,
    pub grade: f64,
    pub staff_end: Option<HeaderHorizontalSide>,
    pub removed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderHorizontalSide {
    Left,
    Right,
}

impl HeaderBarline {
    fn center_x(self) -> Option<i32> {
        self.bounds
            .map(|bounds| bounds.x.wrapping_add(bounds.width / 2))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderBarGroupRelation {
    pub source: usize,
    pub target: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderComponentStage {
    Clefs,
    Keys,
    Time,
    SelectClefs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderBuilderMutation {
    HeaderInstalled {
        staff_id: usize,
        start: i32,
    },
    DummyInterRegistered {
        inter_id: usize,
    },
    DummySigVertexAdded {
        inter_id: usize,
    },
    DummyBoundsSet {
        inter_id: usize,
    },
    DummyStaffAssigned {
        inter_id: usize,
        staff_id: usize,
    },
    StaffBarlineAdded {
        staff_id: usize,
        inter_id: usize,
    },
    HeaderStopUpdated {
        stage: HeaderComponentStage,
        staff_id: usize,
        stop: i32,
    },
    StaffBarlineRemoved {
        staff_id: usize,
        inter_id: usize,
    },
    BarlineMarkedRemoved {
        inter_id: usize,
    },
    SigVertexRemoved {
        inter_id: usize,
    },
    HeaderFrozen {
        staff_id: usize,
    },
}

/// The four recognition-dependent calls made by Java `HeaderBuilder`.
pub trait HeaderColumnRecognizer {
    type Error;

    fn retrieve_clefs(&mut self, system: &mut HeadlessHeaderSystem) -> Result<i32, Self::Error>;

    fn retrieve_keys(
        &mut self,
        system: &mut HeadlessHeaderSystem,
        maximum_header_width: i32,
    ) -> Result<i32, Self::Error>;

    fn retrieve_time(&mut self, system: &mut HeadlessHeaderSystem) -> Result<i32, Self::Error>;

    fn select_clefs(&mut self, system: &mut HeadlessHeaderSystem) -> Result<(), Self::Error>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum HeaderBuilderError<RecognitionError> {
    InterIdExhausted,
    MissingBarline {
        staff_id: usize,
        inter_id: usize,
    },
    MissingBarlineBounds {
        staff_id: usize,
        inter_id: usize,
    },
    MissingBarlineOwner {
        inter_id: usize,
        staff_id: usize,
    },
    BarGroupCycle {
        staff_id: usize,
        inter_id: usize,
    },
    MissingHeader {
        staff_id: usize,
    },
    Recognition {
        stage: HeaderComponentStage,
        source: RecognitionError,
    },
}

impl<RecognitionError: fmt::Display> fmt::Display for HeaderBuilderError<RecognitionError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InterIdExhausted => formatter.write_str("header dummy inter ID exhausted"),
            Self::MissingBarline { staff_id, inter_id } => write!(
                formatter,
                "staff {staff_id} header references missing barline {inter_id}"
            ),
            Self::MissingBarlineBounds { staff_id, inter_id } => write!(
                formatter,
                "staff {staff_id} header barline {inter_id} has no bounds"
            ),
            Self::MissingBarlineOwner { inter_id, staff_id } => write!(
                formatter,
                "header barline {inter_id} references missing staff {staff_id}"
            ),
            Self::BarGroupCycle { staff_id, inter_id } => write!(
                formatter,
                "staff {staff_id} opening bar group cycles at {inter_id}"
            ),
            Self::MissingHeader { staff_id } => {
                write!(formatter, "staff {staff_id} has no initialized header")
            }
            Self::Recognition { stage, source } => {
                write!(formatter, "header {stage:?} recognition failed: {source}")
            }
        }
    }
}

impl<RecognitionError: Error + 'static> Error for HeaderBuilderError<RecognitionError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Recognition { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub struct HeadlessHeaderBuilder<Columns> {
    maximum_header_width: i32,
    columns: Columns,
}

impl<Columns> HeadlessHeaderBuilder<Columns> {
    #[must_use]
    pub const fn new(maximum_header_width: i32, columns: Columns) -> Self {
        Self {
            maximum_header_width,
            columns,
        }
    }

    #[must_use]
    pub const fn columns(&self) -> &Columns {
        &self.columns
    }
}

impl<Columns> HeaderRecognizer for HeadlessHeaderBuilder<Columns>
where
    Columns: HeaderColumnRecognizer,
{
    type Error = HeaderBuilderError<Columns::Error>;

    fn process_header(&mut self, system: &mut HeadlessHeaderSystem) -> Result<(), Self::Error> {
        compute_header_starts(system)?;

        let clef_offset = self.columns.retrieve_clefs(system).map_err(|source| {
            HeaderBuilderError::Recognition {
                stage: HeaderComponentStage::Clefs,
                source,
            }
        })?;
        set_system_stop(system, clef_offset, HeaderComponentStage::Clefs)?;

        let key_offset = self
            .columns
            .retrieve_keys(system, self.maximum_header_width)
            .map_err(|source| HeaderBuilderError::Recognition {
                stage: HeaderComponentStage::Keys,
                source,
            })?;
        set_system_stop(system, key_offset, HeaderComponentStage::Keys)?;

        let time_offset = self.columns.retrieve_time(system).map_err(|source| {
            HeaderBuilderError::Recognition {
                stage: HeaderComponentStage::Time,
                source,
            }
        })?;
        set_system_stop(system, time_offset, HeaderComponentStage::Time)?;

        self.columns
            .select_clefs(system)
            .map_err(|source| HeaderBuilderError::Recognition {
                stage: HeaderComponentStage::SelectClefs,
                source,
            })?;

        purge_barlines(system)?;
        freeze_headers(system)?;
        Ok(())
    }
}

pub fn compute_header_starts<RecognitionError>(
    system: &mut HeadlessHeaderSystem,
) -> Result<(), HeaderBuilderError<RecognitionError>> {
    for staff_index in 0..system.staffs.len() {
        if system.staffs[staff_index].tablature {
            continue;
        }
        let staff_id = system.staffs[staff_index].id;
        let left_barline = system.staffs[staff_index].left_side_barline_id;
        if let Some(left_barline) = left_barline {
            let last_barline = rightmost_group_barline(system, staff_id, left_barline)?;
            let bounds = barline(system, staff_id, last_barline)?.bounds.ok_or(
                HeaderBuilderError::MissingBarlineBounds {
                    staff_id,
                    inter_id: last_barline,
                },
            )?;
            install_header(system, staff_index, bounds.right().wrapping_add(1));
        } else {
            let start = system.staffs[staff_index].left_abscissa;
            install_header(system, staff_index, start);
            create_dummy_left_barline(system, staff_index)?;
        }
    }
    Ok(())
}

fn install_header(system: &mut HeadlessHeaderSystem, staff_index: usize, start: i32) {
    let staff_id = system.staffs[staff_index].id;
    system.staffs[staff_index].header = Some(StaffHeader::new(start));
    system
        .header_mutations
        .push(HeaderBuilderMutation::HeaderInstalled { staff_id, start });
}

fn rightmost_group_barline<RecognitionError>(
    system: &HeadlessHeaderSystem,
    staff_id: usize,
    first: usize,
) -> Result<usize, HeaderBuilderError<RecognitionError>> {
    barline(system, staff_id, first)?;
    let mut last = first;
    let mut visited = Vec::new();
    loop {
        if visited.contains(&last) {
            return Err(HeaderBuilderError::BarGroupCycle {
                staff_id,
                inter_id: last,
            });
        }
        visited.push(last);
        let incident = system
            .bar_group_relations
            .iter()
            .copied()
            .filter(|relation| relation.source == last || relation.target == last)
            .collect::<Vec<_>>();
        let mut moved = false;
        for relation in incident {
            if last == relation.source {
                barline(system, staff_id, relation.target)?;
                last = relation.target;
                moved = true;
            }
        }
        if !moved {
            return Ok(last);
        }
    }
}

fn create_dummy_left_barline<RecognitionError>(
    system: &mut HeadlessHeaderSystem,
    staff_index: usize,
) -> Result<(), HeaderBuilderError<RecognitionError>> {
    let staff_id = system.staffs[staff_index].id;
    let inter_id = system
        .last_inter_id
        .checked_add(1)
        .ok_or(HeaderBuilderError::InterIdExhausted)?;
    system.last_inter_id = inter_id;
    system.barlines.push(HeaderBarline {
        id: inter_id,
        bounds: None,
        staff_id: None,
        dummy: true,
        grade: 1.0,
        staff_end: None,
        removed: false,
    });
    system
        .header_mutations
        .push(HeaderBuilderMutation::DummyInterRegistered { inter_id });
    system.sig_vertex_ids.push(inter_id);
    system
        .header_mutations
        .push(HeaderBuilderMutation::DummySigVertexAdded { inter_id });

    let staff = &system.staffs[staff_index];
    let bounds = HeaderBounds {
        x: staff.left_abscissa,
        y: staff.first_line_y_at_left,
        width: 1,
        height: staff
            .last_line_y_at_left
            .wrapping_sub(staff.first_line_y_at_left),
    };
    let dummy = system
        .barlines
        .iter_mut()
        .find(|barline| barline.id == inter_id)
        .expect("dummy inter was just registered");
    dummy.bounds = Some(bounds);
    system
        .header_mutations
        .push(HeaderBuilderMutation::DummyBoundsSet { inter_id });
    dummy.staff_id = Some(staff_id);
    system
        .header_mutations
        .push(HeaderBuilderMutation::DummyStaffAssigned { inter_id, staff_id });

    add_staff_barline(system, staff_index, inter_id)?;
    Ok(())
}

fn add_staff_barline<RecognitionError>(
    system: &mut HeadlessHeaderSystem,
    staff_index: usize,
    inter_id: usize,
) -> Result<(), HeaderBuilderError<RecognitionError>> {
    let staff_id = system.staffs[staff_index].id;
    if !system.staffs[staff_index].barline_ids.contains(&inter_id) {
        system.staffs[staff_index].barline_ids.push(inter_id);
        let bars = &system.barlines;
        system.staffs[staff_index].barline_ids.sort_by_key(|id| {
            bars.iter()
                .find(|barline| barline.id == *id)
                .and_then(|barline| barline.center_x())
                .unwrap_or(i32::MAX)
        });
        retrieve_side_bars(system, staff_index)?;
        system
            .header_mutations
            .push(HeaderBuilderMutation::StaffBarlineAdded { staff_id, inter_id });
    }
    Ok(())
}

fn retrieve_side_bars<RecognitionError>(
    system: &mut HeadlessHeaderSystem,
    staff_index: usize,
) -> Result<(), HeaderBuilderError<RecognitionError>> {
    let staff_id = system.staffs[staff_index].id;
    let first = system.staffs[staff_index].barline_ids.first().copied();
    let last = system.staffs[staff_index].barline_ids.last().copied();
    system.staffs[staff_index].left_side_barline_id = side_barline(
        system,
        staff_id,
        first,
        system.staffs[staff_index].left_abscissa,
    )?;
    system.staffs[staff_index].right_side_barline_id = side_barline(
        system,
        staff_id,
        last,
        system.staffs[staff_index].right_abscissa,
    )?;
    Ok(())
}

fn side_barline<RecognitionError>(
    system: &HeadlessHeaderSystem,
    staff_id: usize,
    inter_id: Option<usize>,
    end: i32,
) -> Result<Option<usize>, HeaderBuilderError<RecognitionError>> {
    let Some(inter_id) = inter_id else {
        return Ok(None);
    };
    let bounds = barline(system, staff_id, inter_id)?
        .bounds
        .ok_or(HeaderBuilderError::MissingBarlineBounds { staff_id, inter_id })?;
    Ok((bounds.x <= end && end <= bounds.right()).then_some(inter_id))
}

fn set_system_stop<RecognitionError>(
    system: &mut HeadlessHeaderSystem,
    largest_offset: i32,
    stage: HeaderComponentStage,
) -> Result<(), HeaderBuilderError<RecognitionError>> {
    if largest_offset > 0 {
        for staff in &mut system.staffs {
            if staff.tablature {
                continue;
            }
            let header = staff
                .header
                .as_mut()
                .ok_or(HeaderBuilderError::MissingHeader { staff_id: staff.id })?;
            header.stop = header.start.wrapping_add(largest_offset);
            system
                .header_mutations
                .push(HeaderBuilderMutation::HeaderStopUpdated {
                    stage,
                    staff_id: staff.id,
                    stop: header.stop,
                });
        }
    }
    Ok(())
}

pub fn purge_barlines<RecognitionError>(
    system: &mut HeadlessHeaderSystem,
) -> Result<(), HeaderBuilderError<RecognitionError>> {
    for staff_index in 0..system.staffs.len() {
        if system.staffs[staff_index].tablature {
            continue;
        }
        let staff_id = system.staffs[staff_index].id;
        let header = system.staffs[staff_index]
            .header
            .as_ref()
            .ok_or(HeaderBuilderError::MissingHeader { staff_id })?;
        let (start, stop) = (header.start, header.stop);
        let snapshot = system.staffs[staff_index].barline_ids.clone();
        for inter_id in snapshot {
            let bar = *barline(system, staff_id, inter_id)?;
            let center = bar
                .center_x()
                .ok_or(HeaderBuilderError::MissingBarlineBounds { staff_id, inter_id })?;
            if center > start && center < stop && bar.staff_end != Some(HeaderHorizontalSide::Left)
            {
                remove_barline(system, staff_index, inter_id)?;
            }
        }
    }
    Ok(())
}

fn remove_barline<RecognitionError>(
    system: &mut HeadlessHeaderSystem,
    staff_index: usize,
    inter_id: usize,
) -> Result<(), HeaderBuilderError<RecognitionError>> {
    let staff_id = system.staffs[staff_index].id;
    let bar = barline(system, staff_id, inter_id)?;
    if bar.removed {
        return Ok(());
    }
    if let Some(owner_id) = bar.staff_id {
        let owner_index = system
            .staffs
            .iter()
            .position(|staff| staff.id == owner_id)
            .ok_or(HeaderBuilderError::MissingBarlineOwner {
                inter_id,
                staff_id: owner_id,
            })?;
        system.staffs[owner_index]
            .barline_ids
            .retain(|id| *id != inter_id);
        retrieve_side_bars(system, owner_index)?;
        system
            .header_mutations
            .push(HeaderBuilderMutation::StaffBarlineRemoved {
                staff_id: owner_id,
                inter_id,
            });
    }
    system
        .barlines
        .iter_mut()
        .find(|barline| barline.id == inter_id)
        .expect("barline existence was checked")
        .removed = true;
    system
        .header_mutations
        .push(HeaderBuilderMutation::BarlineMarkedRemoved { inter_id });
    system.sig_vertex_ids.retain(|id| *id != inter_id);
    system
        .bar_group_relations
        .retain(|relation| relation.source != inter_id && relation.target != inter_id);
    system
        .header_mutations
        .push(HeaderBuilderMutation::SigVertexRemoved { inter_id });
    Ok(())
}

fn freeze_headers<RecognitionError>(
    system: &mut HeadlessHeaderSystem,
) -> Result<(), HeaderBuilderError<RecognitionError>> {
    for staff in &mut system.staffs {
        if staff.tablature {
            continue;
        }
        let header = staff
            .header
            .as_mut()
            .ok_or(HeaderBuilderError::MissingHeader { staff_id: staff.id })?;
        header.freeze();
        system
            .header_mutations
            .push(HeaderBuilderMutation::HeaderFrozen { staff_id: staff.id });
    }
    Ok(())
}

fn barline<RecognitionError>(
    system: &HeadlessHeaderSystem,
    staff_id: usize,
    inter_id: usize,
) -> Result<&HeaderBarline, HeaderBuilderError<RecognitionError>> {
    system
        .barlines
        .iter()
        .find(|barline| barline.id == inter_id && !barline.removed)
        .ok_or(HeaderBuilderError::MissingBarline { staff_id, inter_id })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        headers_step::{HeadlessHeaderStaff, HeadlessHeaderSystem},
        staff_header::{HeaderBounds, HeaderComponent},
    };

    fn bounds(x: i32, width: i32) -> HeaderBounds {
        HeaderBounds {
            x,
            y: 10,
            width,
            height: 40,
        }
    }

    fn barline(id: usize, x: i32, width: i32, staff_id: usize) -> HeaderBarline {
        HeaderBarline {
            id,
            bounds: Some(bounds(x, width)),
            staff_id: Some(staff_id),
            dummy: false,
            grade: 0.8,
            staff_end: None,
            removed: false,
        }
    }

    fn staff(id: usize, left: i32, right: i32) -> HeadlessHeaderStaff {
        let mut staff = HeadlessHeaderStaff::new(id);
        staff.left_abscissa = left;
        staff.right_abscissa = right;
        staff.first_line_y_at_left = 10;
        staff.last_line_y_at_left = 50;
        staff
    }

    #[derive(Default)]
    struct Columns {
        calls: Vec<&'static str>,
        clef_offset: i32,
        key_offset: i32,
        time_offset: i32,
        fail: Option<HeaderComponentStage>,
    }

    impl HeaderColumnRecognizer for Columns {
        type Error = &'static str;

        fn retrieve_clefs(
            &mut self,
            system: &mut HeadlessHeaderSystem,
        ) -> Result<i32, Self::Error> {
            self.calls.push("clefs");
            for staff in &mut system.staffs {
                if !staff.tablature {
                    staff.header.as_mut().unwrap().clef =
                        Some(HeaderComponent::new(100, bounds(2, 3)));
                }
            }
            if self.fail == Some(HeaderComponentStage::Clefs) {
                Err("clefs failed")
            } else {
                Ok(self.clef_offset)
            }
        }

        fn retrieve_keys(
            &mut self,
            _: &mut HeadlessHeaderSystem,
            maximum_header_width: i32,
        ) -> Result<i32, Self::Error> {
            assert_eq!(maximum_header_width, 42);
            self.calls.push("keys");
            if self.fail == Some(HeaderComponentStage::Keys) {
                Err("keys failed")
            } else {
                Ok(self.key_offset)
            }
        }

        fn retrieve_time(&mut self, _: &mut HeadlessHeaderSystem) -> Result<i32, Self::Error> {
            self.calls.push("time");
            if self.fail == Some(HeaderComponentStage::Time) {
                Err("time failed")
            } else {
                Ok(self.time_offset)
            }
        }

        fn select_clefs(&mut self, _: &mut HeadlessHeaderSystem) -> Result<(), Self::Error> {
            self.calls.push("select");
            if self.fail == Some(HeaderComponentStage::SelectClefs) {
                Err("select failed")
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn computes_directed_group_start_and_creates_owned_dummy_in_java_order() {
        let mut first = staff(1, 0, 100);
        first.barline_ids = vec![1, 2];
        first.left_side_barline_id = Some(1);
        let second = staff(2, 20, 100);
        let mut tablature = staff(3, 30, 100);
        tablature.tablature = true;
        let mut system = HeadlessHeaderSystem::new(9, vec![first, second, tablature]);
        system.last_inter_id = 2;
        system.barlines = vec![barline(1, 0, 1, 1), barline(2, 3, 2, 1)];
        system.sig_vertex_ids = vec![1, 2];
        system.bar_group_relations = vec![HeaderBarGroupRelation {
            source: 1,
            target: 2,
        }];

        compute_header_starts::<&'static str>(&mut system).unwrap();

        assert_eq!(system.staffs[0].header.as_ref().unwrap().start, 5);
        assert_eq!(system.staffs[1].header.as_ref().unwrap().start, 20);
        assert!(system.staffs[2].header.is_none());
        assert_eq!(system.last_inter_id, 3);
        let dummy = system.barlines.iter().find(|bar| bar.id == 3).unwrap();
        assert_eq!(dummy.bounds, Some(bounds(20, 1)));
        assert_eq!(dummy.staff_id, Some(2));
        assert!(dummy.dummy);
        assert_eq!(dummy.grade, 1.0);
        assert_eq!(system.staffs[1].barline_ids, [3]);
        assert_eq!(system.staffs[1].left_side_barline_id, Some(3));
        assert_eq!(
            &system.header_mutations[2..],
            [
                HeaderBuilderMutation::DummyInterRegistered { inter_id: 3 },
                HeaderBuilderMutation::DummySigVertexAdded { inter_id: 3 },
                HeaderBuilderMutation::DummyBoundsSet { inter_id: 3 },
                HeaderBuilderMutation::DummyStaffAssigned {
                    inter_id: 3,
                    staff_id: 2,
                },
                HeaderBuilderMutation::StaffBarlineAdded {
                    staff_id: 2,
                    inter_id: 3,
                },
            ]
        );
    }

    #[test]
    fn production_order_propagates_offsets_purges_strict_interior_and_freezes() {
        let mut staff = staff(1, 0, 100);
        staff.barline_ids = vec![1, 2, 4, 3];
        staff.left_side_barline_id = Some(1);
        let mut system = HeadlessHeaderSystem::new(4, vec![staff]);
        system.last_inter_id = 4;
        system.barlines = vec![
            barline(1, 0, 2, 1),
            barline(2, 5, 1, 1),
            barline(3, 11, 1, 1),
            barline(4, 7, 1, 1),
        ];
        system.barlines[0].staff_end = Some(HeaderHorizontalSide::Left);
        system.barlines[3].staff_end = Some(HeaderHorizontalSide::Left);
        system.sig_vertex_ids = vec![1, 2, 3, 4];
        let columns = Columns {
            clef_offset: 5,
            key_offset: 9,
            time_offset: 0,
            ..Columns::default()
        };
        let mut builder = HeadlessHeaderBuilder::new(42, columns);

        builder.process_header(&mut system).unwrap();

        assert_eq!(builder.columns().calls, ["clefs", "keys", "time", "select"]);
        let header = system.staffs[0].header.as_ref().unwrap();
        assert_eq!((header.start, header.stop), (2, 11));
        assert!(header.clef.as_ref().unwrap().is_frozen());
        assert_eq!(system.staffs[0].barline_ids, [1, 4, 3]);
        assert!(
            system
                .barlines
                .iter()
                .find(|bar| bar.id == 2)
                .unwrap()
                .removed
        );
        assert!(!system.sig_vertex_ids.contains(&2));
        assert!(system.sig_vertex_ids.contains(&3));
        assert!(system.sig_vertex_ids.contains(&4));
        assert!(matches!(
            system.header_mutations.last(),
            Some(HeaderBuilderMutation::HeaderFrozen { staff_id: 1 })
        ));
    }

    #[test]
    fn recognition_failure_retains_exact_completed_prefix() {
        let staff = staff(1, 10, 100);
        let mut system = HeadlessHeaderSystem::new(4, vec![staff]);
        let columns = Columns {
            clef_offset: 7,
            fail: Some(HeaderComponentStage::Keys),
            ..Columns::default()
        };
        let mut builder = HeadlessHeaderBuilder::new(42, columns);

        assert_eq!(
            builder.process_header(&mut system),
            Err(HeaderBuilderError::Recognition {
                stage: HeaderComponentStage::Keys,
                source: "keys failed",
            })
        );
        assert_eq!(builder.columns().calls, ["clefs", "keys"]);
        let header = system.staffs[0].header.as_ref().unwrap();
        assert_eq!((header.start, header.stop), (10, 17));
        assert!(!header.clef.as_ref().unwrap().is_frozen());
        assert_eq!(system.staffs[0].barline_ids.len(), 1); // Dummy retained.
        assert!(!system.barlines[0].removed);
    }

    #[test]
    fn directed_cycle_fails_without_installing_current_staff_header() {
        let mut staff = staff(1, 0, 100);
        staff.barline_ids = vec![1, 2];
        staff.left_side_barline_id = Some(1);
        let mut system = HeadlessHeaderSystem::new(2, vec![staff]);
        system.barlines = vec![barline(1, 0, 1, 1), barline(2, 2, 1, 1)];
        system.bar_group_relations = vec![
            HeaderBarGroupRelation {
                source: 1,
                target: 2,
            },
            HeaderBarGroupRelation {
                source: 2,
                target: 1,
            },
        ];

        assert_eq!(
            compute_header_starts::<&'static str>(&mut system),
            Err(HeaderBuilderError::BarGroupCycle {
                staff_id: 1,
                inter_id: 1,
            })
        );
        assert!(system.staffs[0].header.is_none());
    }
}
