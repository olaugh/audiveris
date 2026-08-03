// SPDX-License-Identifier: AGPL-3.0-or-later

//! Neutral orchestration port of Java `ClefBuilder.Column`.
//!
//! Pixel extraction, glyph clustering, shape classification, font bounds,
//! and clef-kind inference remain one injected visual dependency.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::{
    header_builder::HeaderSigExclusion,
    headers_step::HeadlessHeaderSystem,
    staff_header::{HeaderBounds, HeaderComponent, StaffHeaderRange},
};

/// Java `Constants.maxClefEnd` before staff-interline scale conversion.
pub const MAXIMUM_CLEF_END_INTERLINES: f64 = 4.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NeutralClefKind {
    Treble,
    Bass,
    Alto,
    Tenor,
    Percussion,
    TrebleOttavaAlta,
    TrebleOttavaBassa,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralClefCandidate {
    pub id: usize,
    pub kind: NeutralClefKind,
    pub grade: f64,
    /// Set by later key-support work before `selectClefs`, if available.
    pub contextual_grade: Option<f64>,
    pub bounds: HeaderBounds,
    pub glyph_id: Option<usize>,
    pub glyph_bounds: Option<HeaderBounds>,
    /// Artificial clefs found in the lookup area are already SIG vertices.
    pub in_sig: bool,
    pub staff_id: Option<usize>,
    pub original_glyph_registered: bool,
    pub removed: bool,
}

impl NeutralClefCandidate {
    #[must_use]
    pub fn best_grade(&self) -> f64 {
        self.contextual_grade.unwrap_or(self.grade)
    }
}

/// First visual dependency corresponding to per-staff `ClefBuilder.findClefs`.
/// Returned candidates are the already classified/purged best-per-kind set.
pub trait VisualClefRecognizer {
    type Error;

    fn find_clefs(
        &mut self,
        system_id: usize,
        staff_id: usize,
        range: &StaffHeaderRange,
    ) -> Result<Vec<NeutralClefCandidate>, Self::Error>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClefColumnError<VisualError> {
    MissingHeader { staff_id: usize },
    DuplicateInterId { staff_id: usize, inter_id: usize },
    Visual { staff_id: usize, source: VisualError },
}

impl<VisualError: fmt::Display> fmt::Display for ClefColumnError<VisualError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHeader { staff_id } => {
                write!(formatter, "staff {staff_id} has no header for clef retrieval")
            }
            Self::DuplicateInterId { staff_id, inter_id } => write!(
                formatter,
                "staff {staff_id} clef candidate duplicates live SIG inter {inter_id}"
            ),
            Self::Visual { staff_id, source } => {
                write!(formatter, "staff {staff_id} visual clef recognition failed: {source}")
            }
        }
    }
}

impl<VisualError: Error + 'static> Error for ClefColumnError<VisualError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Visual { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralClefBuilderState {
    pub staff_id: usize,
    pub candidates: Vec<NeutralClefCandidate>,
}

pub struct HeadlessClefColumn<Visual> {
    visual: Visual,
    /// Java `TreeMap<Staff, ClefBuilder>(Staff.byId)`.
    builders: BTreeMap<usize, NeutralClefBuilderState>,
}

impl<Visual> HeadlessClefColumn<Visual> {
    #[must_use]
    pub const fn new(visual: Visual) -> Self {
        Self {
            visual,
            builders: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }

    #[must_use]
    pub fn builders(&self) -> &BTreeMap<usize, NeutralClefBuilderState> {
        &self.builders
    }
}

impl<Visual> HeadlessClefColumn<Visual>
where
    Visual: VisualClefRecognizer,
{
    /// Java `Column.retrieveClefs`, retaining every completed staff prefix.
    pub fn retrieve_clefs(
        &mut self,
        system: &mut HeadlessHeaderSystem,
    ) -> Result<i32, ClefColumnError<Visual::Error>> {
        let mut maximum_offset = 0;
        for staff_index in 0..system.staffs.len() {
            if system.staffs[staff_index].tablature {
                continue;
            }
            let staff_id = system.staffs[staff_index].id;
            let measure_start = system.staffs[staff_index]
                .header
                .as_ref()
                .ok_or(ClefColumnError::MissingHeader { staff_id })?
                .start;
            let maximum_end = system.staffs[staff_index].maximum_clef_end;
            let header = system.staffs[staff_index]
                .header
                .as_mut()
                .expect("header existence was checked");
            let range = header
                .clef_range
                .get_or_insert_with(StaffHeaderRange::default);
            range.browse_start = measure_start;
            range.browse_stop = measure_start.wrapping_add(maximum_end);

            // Java inserts the builder before `findClefs`; a visual failure
            // must retain this empty builder and initialized range.
            self.builders.insert(
                staff_id,
                NeutralClefBuilderState {
                    staff_id,
                    candidates: Vec::new(),
                },
            );
            let candidates = self
                .visual
                .find_clefs(system.id, staff_id, range)
                .map_err(|source| ClefColumnError::Visual { staff_id, source })?;
            register_candidates(system, staff_index, candidates, &mut self.builders)?;

            let precise_stop = system.staffs[staff_index]
                .header
                .as_ref()
                .and_then(|header| header.clef_range.as_ref())
                .and_then(StaffHeaderRange::precise_stop);
            if let Some(stop) = precise_stop {
                maximum_offset = maximum_offset.max(stop.wrapping_sub(measure_start));
            }
        }
        Ok(maximum_offset)
    }

    /// Java `Column.selectClefs`, traversing builder values by staff ID.
    pub fn select_clefs(
        &mut self,
        system: &mut HeadlessHeaderSystem,
    ) -> Result<(), ClefColumnError<Visual::Error>> {
        let staff_ids = self.builders.keys().copied().collect::<Vec<_>>();
        for staff_id in staff_ids {
            let staff_index = system
                .staffs
                .iter()
                .position(|staff| staff.id == staff_id)
                .ok_or(ClefColumnError::MissingHeader { staff_id })?;
            let header = system.staffs[staff_index]
                .header
                .as_mut()
                .ok_or(ClefColumnError::MissingHeader { staff_id })?;
            let range_stop = header
                .clef_range
                .as_ref()
                .ok_or(ClefColumnError::MissingHeader { staff_id })?
                .stop();
            let builder = self.builders.get_mut(&staff_id).expect("key came from map");
            // Java first takes the last clef whose abscissa precedes the
            // range stop, then includes every same-staff exclusion peer.
            let Some(anchor) = builder
                .candidates
                .iter()
                .enumerate()
                .filter(|(_, candidate)| !candidate.removed && candidate.bounds.x < range_stop)
                .max_by_key(|(index, candidate)| (candidate.bounds.x, *index))
                .map(|(index, _)| index)
            else {
                continue;
            };
            let anchor_id = builder.candidates[anchor].id;
            let mut active = builder
                .candidates
                .iter()
                .enumerate()
                .filter(|(_, candidate)| {
                    !candidate.removed
                        && (candidate.id == anchor_id
                            || system.sig_exclusions.iter().any(|exclusion| {
                                (exclusion.one == anchor_id && exclusion.two == candidate.id)
                                    || (exclusion.two == anchor_id
                                        && exclusion.one == candidate.id)
                            }))
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            active.sort_by(|one, two| {
                builder.candidates[*two]
                    .best_grade()
                    .total_cmp(&builder.candidates[*one].best_grade())
            });
            let winner = active[0];
            if builder.candidates[winner].glyph_id.is_some() {
                builder.candidates[winner].original_glyph_registered = true;
            }
            let selected = &builder.candidates[winner];
            header.clef = Some(HeaderComponent::new(selected.id, selected.bounds));

            for &loser in &active[1..] {
                let id = builder.candidates[loser].id;
                builder.candidates[loser].removed = true;
                system.sig_vertex_ids.retain(|candidate| *candidate != id);
                system
                    .sig_exclusions
                    .retain(|exclusion| exclusion.one != id && exclusion.two != id);
            }
        }
        Ok(())
    }
}

fn register_candidates<VisualError>(
    system: &mut HeadlessHeaderSystem,
    staff_index: usize,
    mut candidates: Vec<NeutralClefCandidate>,
    builders: &mut BTreeMap<usize, NeutralClefBuilderState>,
) -> Result<(), ClefColumnError<VisualError>> {
    let staff_id = system.staffs[staff_index].id;
    candidates.sort_by(|one, two| two.grade.total_cmp(&one.grade));
    for (index, candidate) in candidates.iter_mut().enumerate() {
        if candidate.glyph_id.is_some() && !candidate.in_sig {
            if system.sig_vertex_ids.contains(&candidate.id) {
                return Err(ClefColumnError::DuplicateInterId {
                    staff_id,
                    inter_id: candidate.id,
                });
            }
            system.sig_vertex_ids.push(candidate.id);
            candidate.in_sig = true;
        }
        candidate.staff_id = Some(staff_id);
        if index == 0 {
            let stop_bounds = candidate
                .glyph_bounds
                .map_or(candidate.bounds, |glyph| intersection(glyph, candidate.bounds));
            let header = system.staffs[staff_index]
                .header
                .as_mut()
                .expect("caller checked header");
            let range = header
                .clef_range
                .as_mut()
                .expect("caller initialized clef range");
            range.set_stop(stop_bounds.right());
            range.valid = true;
        }
    }
    for one in 0..candidates.len() {
        for two in (one + 1)..candidates.len() {
            system.sig_exclusions.push(HeaderSigExclusion {
                one: candidates[one].id,
                two: candidates[two].id,
            });
        }
    }
    builders
        .get_mut(&staff_id)
        .expect("builder inserted before recognition")
        .candidates = candidates;
    Ok(())
}

fn intersection(one: HeaderBounds, two: HeaderBounds) -> HeaderBounds {
    let x = one.x.max(two.x);
    let y = one.y.max(two.y);
    let right = one.right().min(two.right());
    let bottom = one
        .y
        .wrapping_add(one.height)
        .wrapping_sub(1)
        .min(two.y.wrapping_add(two.height).wrapping_sub(1));
    HeaderBounds {
        x,
        y,
        width: right.wrapping_sub(x).wrapping_add(1),
        height: bottom.wrapping_sub(y).wrapping_add(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{headers_step::HeadlessHeaderStaff, staff_header::StaffHeader};
    use std::convert::Infallible;

    #[derive(Default)]
    struct FakeVisual {
        by_staff: BTreeMap<usize, Result<Vec<NeutralClefCandidate>, &'static str>>,
        calls: Vec<usize>,
    }

    impl VisualClefRecognizer for FakeVisual {
        type Error = &'static str;

        fn find_clefs(
            &mut self,
            _system_id: usize,
            staff_id: usize,
            _range: &StaffHeaderRange,
        ) -> Result<Vec<NeutralClefCandidate>, Self::Error> {
            self.calls.push(staff_id);
            self.by_staff.remove(&staff_id).unwrap_or(Ok(Vec::new()))
        }
    }

    fn bounds(x: i32, y: i32, width: i32, height: i32) -> HeaderBounds {
        HeaderBounds { x, y, width, height }
    }

    fn candidate(id: usize, grade: f64, x: i32) -> NeutralClefCandidate {
        NeutralClefCandidate {
            id,
            kind: NeutralClefKind::Treble,
            grade,
            contextual_grade: None,
            bounds: bounds(x, 4, 8, 14),
            glyph_id: Some(id + 100),
            glyph_bounds: None,
            in_sig: false,
            staff_id: None,
            original_glyph_registered: false,
            removed: false,
        }
    }

    fn staff(id: usize, start: i32, maximum_clef_end: i32) -> HeadlessHeaderStaff {
        let mut staff = HeadlessHeaderStaff::new(id);
        staff.maximum_clef_end = maximum_clef_end;
        staff.header = Some(StaffHeader::new(start));
        staff
    }

    #[test]
    fn retrieve_preserves_source_order_skips_tablature_and_registers_by_grade() {
        let mut tablature = staff(2, 20, 9);
        tablature.tablature = true;
        let mut system = HeadlessHeaderSystem::new(
            7,
            vec![staff(5, 10, 12), tablature, staff(3, 30, 10)],
        );
        let mut visual = FakeVisual::default();
        let mut low = candidate(51, 0.2, 13);
        low.glyph_bounds = Some(bounds(14, 4, 3, 14));
        visual
            .by_staff
            .insert(5, Ok(vec![low, candidate(50, 0.9, 11)]));
        visual.by_staff.insert(3, Ok(vec![candidate(30, 0.8, 31)]));
        let mut column = HeadlessClefColumn::new(visual);

        assert_eq!(column.retrieve_clefs(&mut system), Ok(8));
        assert_eq!(column.visual().calls, vec![5, 3]);
        assert_eq!(column.builders().keys().copied().collect::<Vec<_>>(), vec![3, 5]);
        assert_eq!(system.sig_vertex_ids, vec![50, 51, 30]);
        assert_eq!(system.sig_exclusions, vec![HeaderSigExclusion { one: 50, two: 51 }]);
        assert!(system.staffs[1].header.as_ref().unwrap().clef_range.is_none());
        let five = system.staffs[0].header.as_ref().unwrap().clef_range.as_ref().unwrap();
        assert_eq!((five.browse_start, five.browse_stop, five.precise_stop()), (10, 22, Some(18)));
        assert_eq!(column.builders()[&5].candidates[0].staff_id, Some(5));
    }

    #[test]
    fn visual_failure_retains_initialized_range_and_empty_builder() {
        let mut system = HeadlessHeaderSystem::new(7, vec![staff(1, 10, 12)]);
        let mut visual = FakeVisual::default();
        visual.by_staff.insert(1, Err("classifier unavailable"));
        let mut column = HeadlessClefColumn::new(visual);

        assert_eq!(
            column.retrieve_clefs(&mut system),
            Err(ClefColumnError::Visual { staff_id: 1, source: "classifier unavailable" })
        );
        assert!(column.builders()[&1].candidates.is_empty());
        let range = system.staffs[0].header.as_ref().unwrap().clef_range.as_ref().unwrap();
        assert_eq!((range.browse_start, range.browse_stop), (10, 22));
    }

    #[test]
    fn selection_uses_contextual_grade_and_exclusion_peers_beyond_stop() {
        let mut system = HeadlessHeaderSystem::new(7, vec![staff(5, 10, 12)]);
        let mut visual = FakeVisual::default();
        let first = candidate(50, 0.9, 11);
        let mut peer = candidate(51, 0.2, 30);
        peer.contextual_grade = Some(1.1);
        visual.by_staff.insert(5, Ok(vec![first, peer]));
        let mut column = HeadlessClefColumn::new(visual);
        column.retrieve_clefs(&mut system).unwrap();

        column.select_clefs(&mut system).unwrap();
        let selected = system.staffs[0].header.as_ref().unwrap().clef.as_ref().unwrap();
        assert_eq!(selected.id, 51);
        assert_eq!(system.sig_vertex_ids, vec![51]);
        assert!(column.builders()[&5].candidates[1].original_glyph_registered);
        assert!(column.builders()[&5].candidates[0].removed);
        assert!(system.sig_exclusions.is_empty());
    }

    #[test]
    fn empty_visual_result_keeps_range_invalid_and_returns_zero_offset() {
        let mut system = HeadlessHeaderSystem::new(7, vec![staff(1, 10, 12)]);
        let mut column = HeadlessClefColumn::new(FakeVisual::default());
        assert_eq!(column.retrieve_clefs(&mut system), Ok(0));
        assert!(!system.staffs[0]
            .header
            .as_ref()
            .unwrap()
            .clef_range
            .as_ref()
            .unwrap()
            .valid);
    }

    fn _assert_infallible_is_error(value: ClefColumnError<Infallible>) -> impl Error {
        value
    }
}
