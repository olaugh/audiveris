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
    Baritone,
    Tenor,
    Alto,
    MezzoSoprano,
    Soprano,
    Percussion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralClefShape {
    Treble,
    TrebleOttavaAlta,
    TrebleOttavaBassa,
    Bass,
    C,
    Percussion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClefRecognitionPass {
    OuterAndInner,
    InnerOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClefLookupRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClefLookupContext {
    pub outer: ClefLookupRect,
    pub inner: ClefLookupRect,
    pub percussion_only: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClefLookupStaffGeometry {
    pub staff_id: usize,
    pub browse_start: i32,
    pub browse_stop: i32,
    pub left_abscissa: i32,
    pub right_abscissa: i32,
    pub first_line_y: i32,
    pub last_line_y: i32,
    pub percussion_only: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClefLookupParameters {
    pub sheet_height: i32,
    pub above_staff: i32,
    pub below_staff: i32,
    pub belt_margin: i32,
    pub x_core_margin: i32,
    pub y_core_margin: i32,
}

/// Java `getOuterRect` + `getInnerRect`, using the dependency-light constant
/// staff-line geometry supplied by the headless system.
#[must_use]
pub fn build_clef_lookup_contexts(
    staffs: &[ClefLookupStaffGeometry],
    parameters: ClefLookupParameters,
) -> BTreeMap<usize, ClefLookupContext> {
    let mut contexts = BTreeMap::new();
    for staff in staffs {
        let x_mid = staff.browse_start.wrapping_add(staff.browse_stop) / 2;
        let mut y_min = 0.max(staff.first_line_y.wrapping_sub(parameters.above_staff));
        let mut y_max = (parameters.sheet_height - 1)
            .min(staff.last_line_y.wrapping_add(parameters.below_staff));
        for neighbor in staffs {
            if neighbor.staff_id == staff.staff_id
                || neighbor.left_abscissa >= x_mid
                || neighbor.right_abscissa <= x_mid
            {
                continue;
            }
            if neighbor.last_line_y < staff.first_line_y {
                y_min = y_min.max(div_ceil_two(neighbor.last_line_y + staff.first_line_y + 1));
            } else if neighbor.first_line_y > staff.last_line_y {
                y_max = y_max.min((staff.last_line_y + neighbor.first_line_y - 1) / 2);
            }
        }
        let outer = ClefLookupRect {
            x: staff.browse_start + parameters.belt_margin,
            y: y_min,
            width: staff.browse_stop - staff.browse_start + 1 - (2 * parameters.belt_margin),
            height: y_max - y_min + 1,
        };
        let inner = ClefLookupRect {
            x: outer.x + parameters.x_core_margin,
            y: outer.y + parameters.y_core_margin,
            width: outer.width - parameters.x_core_margin,
            height: outer.height - (2 * parameters.y_core_margin),
        };
        contexts.insert(
            staff.staff_id,
            ClefLookupContext {
                outer,
                inner,
                percussion_only: staff.percussion_only,
            },
        );
    }
    contexts
}

fn div_ceil_two(value: i32) -> i32 {
    (value / 2) + i32::from(value % 2 != 0 && value > 0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClefClassifierProposal {
    pub id: usize,
    pub glyph_id: usize,
    pub shape: NeutralClefShape,
    /// Raw classifier grade; Rust applies the intrinsic ratio.
    pub classifier_grade: f64,
    /// Deterministic target pitch derived from glyph center/staff geometry.
    pub reference_pitch: i32,
    pub symbol_bounds: HeaderBounds,
    pub glyph_bounds: HeaderBounds,
}

pub trait VisualClefProposalRecognizer {
    type Error;

    fn existing_clef(
        &mut self,
        system_id: usize,
        staff_id: usize,
        outer: ClefLookupRect,
    ) -> Result<Option<NeutralClefCandidate>, Self::Error>;

    fn classify_clefs(
        &mut self,
        system_id: usize,
        staff_id: usize,
        pass: ClefRecognitionPass,
        lookup: ClefLookupRect,
    ) -> Result<Vec<ClefClassifierProposal>, Self::Error>;
}

/// Native candidate lifecycle around the injected glyph/classifier seam.
pub struct ClefLifecycleRecognizer<Visual> {
    visual: Visual,
    contexts: BTreeMap<usize, ClefLookupContext>,
    intrinsic_ratio: f64,
    maximum_key_contribution: f64,
}

impl<Visual> ClefLifecycleRecognizer<Visual> {
    #[must_use]
    pub fn new(
        visual: Visual,
        contexts: BTreeMap<usize, ClefLookupContext>,
        intrinsic_ratio: f64,
        maximum_key_contribution: f64,
    ) -> Self {
        Self {
            visual,
            contexts,
            intrinsic_ratio,
            maximum_key_contribution,
        }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }
}

impl<Visual> VisualClefRecognizer for ClefLifecycleRecognizer<Visual>
where
    Visual: VisualClefProposalRecognizer,
{
    type Error = Visual::Error;

    fn find_clefs(
        &mut self,
        system_id: usize,
        staff_id: usize,
        _range: &StaffHeaderRange,
    ) -> Result<Vec<NeutralClefCandidate>, Self::Error> {
        let context = self.contexts[&staff_id];
        if let Some(existing) = self
            .visual
            .existing_clef(system_id, staff_id, context.outer)?
        {
            return Ok(vec![existing]);
        }
        let mut best = self.best_map(
            system_id,
            staff_id,
            context,
            ClefRecognitionPass::OuterAndInner,
        )?;
        if best.is_empty() {
            best = self.best_map(system_id, staff_id, context, ClefRecognitionPass::InnerOnly)?;
        }
        purge_clef_candidates(&mut best, self.maximum_key_contribution);
        Ok(best.into_values().collect())
    }
}

impl<Visual> ClefLifecycleRecognizer<Visual>
where
    Visual: VisualClefProposalRecognizer,
{
    fn best_map(
        &mut self,
        system_id: usize,
        staff_id: usize,
        context: ClefLookupContext,
        pass: ClefRecognitionPass,
    ) -> Result<BTreeMap<NeutralClefKind, NeutralClefCandidate>, Visual::Error> {
        let lookup = match pass {
            ClefRecognitionPass::OuterAndInner => context.outer,
            ClefRecognitionPass::InnerOnly => context.inner,
        };
        let proposals = self
            .visual
            .classify_clefs(system_id, staff_id, pass, lookup)?;
        let mut best = BTreeMap::new();
        for proposal in proposals {
            if context.percussion_only && proposal.shape != NeutralClefShape::Percussion {
                continue;
            }
            let Some(kind) = clef_kind(proposal.shape, proposal.reference_pitch) else {
                continue;
            };
            let grade = self.intrinsic_ratio * proposal.classifier_grade;
            if best
                .get(&kind)
                .is_none_or(|candidate: &NeutralClefCandidate| candidate.grade < grade)
            {
                best.insert(
                    kind,
                    NeutralClefCandidate {
                        id: proposal.id,
                        kind,
                        grade,
                        contextual_grade: None,
                        bounds: proposal.symbol_bounds,
                        glyph_id: Some(proposal.glyph_id),
                        glyph_bounds: Some(proposal.glyph_bounds),
                        in_sig: false,
                        staff_id: None,
                        original_glyph_registered: false,
                        removed: false,
                    },
                );
            }
        }
        Ok(best)
    }
}

fn clef_kind(shape: NeutralClefShape, pitch: i32) -> Option<NeutralClefKind> {
    match shape {
        NeutralClefShape::Treble
        | NeutralClefShape::TrebleOttavaAlta
        | NeutralClefShape::TrebleOttavaBassa => Some(NeutralClefKind::Treble),
        NeutralClefShape::Bass => match pitch {
            -2 => Some(NeutralClefKind::Bass),
            0 => Some(NeutralClefKind::Baritone),
            _ => None,
        },
        NeutralClefShape::C => match pitch {
            -2 => Some(NeutralClefKind::Tenor),
            0 => Some(NeutralClefKind::Alto),
            2 => Some(NeutralClefKind::MezzoSoprano),
            4 => Some(NeutralClefKind::Soprano),
            _ => None,
        },
        NeutralClefShape::Percussion => Some(NeutralClefKind::Percussion),
    }
}

fn purge_clef_candidates(
    best: &mut BTreeMap<NeutralClefKind, NeutralClefCandidate>,
    maximum_key_contribution: f64,
) {
    if best.len() <= 1 {
        return;
    }
    let mut ordered = best.values().cloned().collect::<Vec<_>>();
    ordered.sort_by(|one, two| two.grade.total_cmp(&one.grade));
    for one in 0..ordered.len() {
        for two in (one + 1)..ordered.len() {
            let maximum_other = contextual_grade(ordered[two].grade, maximum_key_contribution);
            if ordered[one].grade > maximum_other {
                for poor in &ordered[two..] {
                    best.remove(&poor.kind);
                }
                return;
            }
        }
    }
}

fn contextual_grade(intrinsic: f64, contribution: f64) -> f64 {
    1.0 - ((1.0 - intrinsic) * (1.0 - contribution))
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
    MissingHeader {
        staff_id: usize,
    },
    DuplicateInterId {
        staff_id: usize,
        inter_id: usize,
    },
    Visual {
        staff_id: usize,
        source: VisualError,
    },
}

impl<VisualError: fmt::Display> fmt::Display for ClefColumnError<VisualError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHeader { staff_id } => {
                write!(
                    formatter,
                    "staff {staff_id} has no header for clef retrieval"
                )
            }
            Self::DuplicateInterId { staff_id, inter_id } => write!(
                formatter,
                "staff {staff_id} clef candidate duplicates live SIG inter {inter_id}"
            ),
            Self::Visual { staff_id, source } => {
                write!(
                    formatter,
                    "staff {staff_id} visual clef recognition failed: {source}"
                )
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
                                    || (exclusion.two == anchor_id && exclusion.one == candidate.id)
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
            let stop_bounds = candidate.glyph_bounds.map_or(candidate.bounds, |glyph| {
                intersection(glyph, candidate.bounds)
            });
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
    struct FakeProposalVisual {
        existing: Option<NeutralClefCandidate>,
        passes: Vec<Vec<ClefClassifierProposal>>,
        calls: Vec<ClefRecognitionPass>,
    }

    impl VisualClefProposalRecognizer for FakeProposalVisual {
        type Error = Infallible;

        fn existing_clef(
            &mut self,
            _system_id: usize,
            _staff_id: usize,
            _outer: ClefLookupRect,
        ) -> Result<Option<NeutralClefCandidate>, Self::Error> {
            Ok(self.existing.take())
        }

        fn classify_clefs(
            &mut self,
            _system_id: usize,
            _staff_id: usize,
            pass: ClefRecognitionPass,
            _lookup: ClefLookupRect,
        ) -> Result<Vec<ClefClassifierProposal>, Self::Error> {
            self.calls.push(pass);
            Ok(self.passes.remove(0))
        }
    }

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
        HeaderBounds {
            x,
            y,
            width,
            height,
        }
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
        let mut system =
            HeadlessHeaderSystem::new(7, vec![staff(5, 10, 12), tablature, staff(3, 30, 10)]);
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
        assert_eq!(
            column.builders().keys().copied().collect::<Vec<_>>(),
            vec![3, 5]
        );
        assert_eq!(system.sig_vertex_ids, vec![50, 51, 30]);
        assert_eq!(
            system.sig_exclusions,
            vec![HeaderSigExclusion { one: 50, two: 51 }]
        );
        assert!(
            system.staffs[1]
                .header
                .as_ref()
                .unwrap()
                .clef_range
                .is_none()
        );
        let five = system.staffs[0]
            .header
            .as_ref()
            .unwrap()
            .clef_range
            .as_ref()
            .unwrap();
        assert_eq!(
            (five.browse_start, five.browse_stop, five.precise_stop()),
            (10, 22, Some(18))
        );
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
            Err(ClefColumnError::Visual {
                staff_id: 1,
                source: "classifier unavailable"
            })
        );
        assert!(column.builders()[&1].candidates.is_empty());
        let range = system.staffs[0]
            .header
            .as_ref()
            .unwrap()
            .clef_range
            .as_ref()
            .unwrap();
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
        let selected = system.staffs[0]
            .header
            .as_ref()
            .unwrap()
            .clef
            .as_ref()
            .unwrap();
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
        assert!(
            !system.staffs[0]
                .header
                .as_ref()
                .unwrap()
                .clef_range
                .as_ref()
                .unwrap()
                .valid
        );
    }

    fn _assert_infallible_is_error(value: ClefColumnError<Infallible>) -> impl Error {
        value
    }

    fn proposal(
        id: usize,
        shape: NeutralClefShape,
        pitch: i32,
        grade: f64,
    ) -> ClefClassifierProposal {
        ClefClassifierProposal {
            id,
            glyph_id: id + 100,
            shape,
            classifier_grade: grade,
            reference_pitch: pitch,
            symbol_bounds: bounds(12, 4, 8, 14),
            glyph_bounds: bounds(13, 4, 6, 14),
        }
    }

    fn context(percussion_only: bool) -> BTreeMap<usize, ClefLookupContext> {
        BTreeMap::from([(
            1,
            ClefLookupContext {
                outer: ClefLookupRect {
                    x: 10,
                    y: 20,
                    width: 30,
                    height: 50,
                },
                inner: ClefLookupRect {
                    x: 12,
                    y: 25,
                    width: 28,
                    height: 40,
                },
                percussion_only,
            },
        )])
    }

    #[test]
    fn lookup_geometry_clamps_to_neighbor_gutters_and_builds_asymmetric_inner() {
        let contexts = build_clef_lookup_contexts(
            &[
                ClefLookupStaffGeometry {
                    staff_id: 1,
                    browse_start: 10,
                    browse_stop: 40,
                    left_abscissa: 0,
                    right_abscissa: 100,
                    first_line_y: 20,
                    last_line_y: 40,
                    percussion_only: false,
                },
                ClefLookupStaffGeometry {
                    staff_id: 2,
                    browse_start: 10,
                    browse_stop: 40,
                    left_abscissa: 0,
                    right_abscissa: 100,
                    first_line_y: 60,
                    last_line_y: 80,
                    percussion_only: false,
                },
            ],
            ClefLookupParameters {
                sheet_height: 100,
                above_staff: 30,
                below_staff: 32,
                belt_margin: 2,
                x_core_margin: 3,
                y_core_margin: 4,
            },
        );
        assert_eq!(
            contexts[&1].outer,
            ClefLookupRect {
                x: 12,
                y: 0,
                width: 27,
                height: 50
            }
        );
        assert_eq!(
            contexts[&1].inner,
            ClefLookupRect {
                x: 15,
                y: 4,
                width: 24,
                height: 42
            }
        );
        assert_eq!(contexts[&2].outer.y, 51);
    }

    #[test]
    fn artificial_clef_short_circuits_both_classifier_passes() {
        let visual = FakeProposalVisual {
            existing: Some(candidate(9, 0.3, 12)),
            ..FakeProposalVisual::default()
        };
        let mut lifecycle = ClefLifecycleRecognizer::new(visual, context(false), 0.8, 0.2);
        let result = lifecycle
            .find_clefs(7, 1, &StaffHeaderRange::default())
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, 9);
        assert!(lifecycle.visual().calls.is_empty());
    }

    #[test]
    fn lifecycle_retries_inner_maps_kinds_deduplicates_and_purges_unbeatable_tail() {
        let visual = FakeProposalVisual {
            passes: vec![
                Vec::new(),
                vec![
                    proposal(1, NeutralClefShape::C, -2, 0.9),
                    proposal(2, NeutralClefShape::C, -2, 0.7),
                    proposal(3, NeutralClefShape::Bass, -2, 0.2),
                    proposal(4, NeutralClefShape::Bass, 7, 1.0),
                ],
            ],
            ..FakeProposalVisual::default()
        };
        let mut lifecycle = ClefLifecycleRecognizer::new(visual, context(false), 0.8, 0.1);
        let result = lifecycle
            .find_clefs(7, 1, &StaffHeaderRange::default())
            .unwrap();
        assert_eq!(
            lifecycle.visual().calls,
            vec![
                ClefRecognitionPass::OuterAndInner,
                ClefRecognitionPass::InnerOnly
            ]
        );
        assert_eq!(result.len(), 1);
        assert_eq!((result[0].id, result[0].kind), (1, NeutralClefKind::Tenor));
        assert!((result[0].grade - 0.72).abs() < f64::EPSILON);
    }

    #[test]
    fn percussion_context_rejects_pitched_shapes_before_kind_competition() {
        let visual = FakeProposalVisual {
            passes: vec![vec![
                proposal(1, NeutralClefShape::Treble, 2, 0.9),
                proposal(2, NeutralClefShape::Percussion, 0, 0.5),
            ]],
            ..FakeProposalVisual::default()
        };
        let mut lifecycle = ClefLifecycleRecognizer::new(visual, context(true), 1.0, 0.0);
        let result = lifecycle
            .find_clefs(7, 1, &StaffHeaderRange::default())
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].kind, NeutralClefKind::Percussion);
    }
}
