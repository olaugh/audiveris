// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light orchestration port of Java `HeaderTimeColumn`.
//!
//! Projection, glyph assembly, classification, half pairing, and local
//! conflict filtering remain one injected visual operation. Builder ordering,
//! column-wide value compatibility, Java HashMap/tie behavior, cleanup,
//! selection, header assignment, and offset reporting are native Rust.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::{
    headers_step::HeadlessHeaderSystem,
    staff_header::{HeaderBounds, HeaderComponent, StaffHeaderRange},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NeutralSpecificTimeShape {
    Common,
    Cut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NeutralTimeValue {
    pub specific_shape: Option<NeutralSpecificTimeShape>,
    pub numerator: i32,
    pub denominator: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralTimeCandidateKind {
    Whole,
    Pair,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralTimeCandidate {
    pub id: usize,
    pub kind: NeutralTimeCandidateKind,
    pub value: NeutralTimeValue,
    pub grade: f64,
    /// Symbol bounds used by `HeaderTimeBuilder.createTimeSig`.
    pub symbol_bounds: HeaderBounds,
    /// Empty for a whole; top then bottom IDs for a pair.
    pub member_ids: Vec<usize>,
    pub staff_id: Option<usize>,
    pub in_sig: bool,
    pub original_glyphs_registered: bool,
    pub removed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderTimeRecognitionInput {
    pub system_id: usize,
    pub staff_id: usize,
    pub browse_start: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeaderTimeRecognition {
    /// Mutated Java `StaffHeader.timeRange` after projection browsing.
    pub range: StaffHeaderRange,
    /// Whole candidates first, then pair candidates in relation traversal order.
    pub candidates: Vec<NeutralTimeCandidate>,
    /// Java `filterCandidates` return value.
    pub filter_passed: bool,
}

/// First unavailable seam: header projection, glyphs, classifier, and local
/// whole/num/den filtering.
pub trait VisualHeaderTimeRecognizer {
    type Error;

    fn recognize_time(
        &mut self,
        input: HeaderTimeRecognitionInput,
        range: &StaffHeaderRange,
    ) -> Result<HeaderTimeRecognition, Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeWholeClassifierProposal {
    pub id: usize,
    pub value: NeutralTimeValue,
    pub grade: f64,
    pub symbol_bounds: HeaderBounds,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeNumberClassifierProposal {
    pub id: usize,
    pub value: i32,
    pub grade: f64,
    pub bounds: HeaderBounds,
    pub center_x: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeaderTimeClassifierProposals {
    pub range: StaffHeaderRange,
    pub wholes: Vec<TimeWholeClassifierProposal>,
    pub numerators: Vec<TimeNumberClassifierProposal>,
    pub denominators: Vec<TimeNumberClassifierProposal>,
}

pub trait VisualHeaderTimeProposalRecognizer {
    type Error;

    fn classify_time_parts(
        &mut self,
        input: HeaderTimeRecognitionInput,
        range: &StaffHeaderRange,
    ) -> Result<HeaderTimeClassifierProposals, Self::Error>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeaderTimeLifecycleContext {
    pub maximum_halves_dx: i32,
    pub top_bottom_source_ratio: f64,
    /// Stable pair inter IDs allocated outside the classifier seam.
    pub pair_ids: BTreeMap<(usize, usize), usize>,
    /// Java defaults plus configured optional time values, in no special order.
    pub supported_values: Vec<(i32, i32)>,
}

/// Native whole/number lifecycle around injected classifier proposals.
pub struct HeaderTimeLifecycleRecognizer<Visual> {
    visual: Visual,
    contexts: BTreeMap<usize, HeaderTimeLifecycleContext>,
}

impl<Visual> HeaderTimeLifecycleRecognizer<Visual> {
    #[must_use]
    pub fn new(visual: Visual, contexts: BTreeMap<usize, HeaderTimeLifecycleContext>) -> Self {
        Self { visual, contexts }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }
}

impl<Visual> VisualHeaderTimeRecognizer for HeaderTimeLifecycleRecognizer<Visual>
where
    Visual: VisualHeaderTimeProposalRecognizer,
{
    type Error = Visual::Error;

    fn recognize_time(
        &mut self,
        input: HeaderTimeRecognitionInput,
        range: &StaffHeaderRange,
    ) -> Result<HeaderTimeRecognition, Self::Error> {
        let proposals = self.visual.classify_time_parts(input, range)?;
        let context = &self.contexts[&input.staff_id];
        let mut candidates = proposals
            .wholes
            .into_iter()
            .map(|whole| NeutralTimeCandidate {
                id: whole.id,
                kind: NeutralTimeCandidateKind::Whole,
                value: whole.value,
                grade: whole.grade,
                symbol_bounds: whole.symbol_bounds,
                member_ids: Vec::new(),
                staff_id: Some(input.staff_id),
                in_sig: false,
                original_glyphs_registered: false,
                removed: false,
            })
            .collect::<Vec<_>>();

        // Java top list outer loop, bottom list inner loop, strict dx gate.
        for numerator in &proposals.numerators {
            for denominator in &proposals.denominators {
                if denominator.center_x.wrapping_sub(numerator.center_x).abs()
                    > context.maximum_halves_dx
                    || !context
                        .supported_values
                        .contains(&(numerator.value, denominator.value))
                {
                    continue;
                }
                let Some(pair_id) = context.pair_ids.get(&(numerator.id, denominator.id)) else {
                    continue;
                };
                let top_contribution =
                    contribution_of(denominator.grade, context.top_bottom_source_ratio);
                let bottom_contribution =
                    contribution_of(numerator.grade, context.top_bottom_source_ratio);
                let grade = (contextual(numerator.grade, top_contribution)
                    + contextual(denominator.grade, bottom_contribution))
                    / 2.0;
                candidates.push(NeutralTimeCandidate {
                    id: *pair_id,
                    kind: NeutralTimeCandidateKind::Pair,
                    value: NeutralTimeValue {
                        specific_shape: None,
                        numerator: numerator.value,
                        denominator: denominator.value,
                    },
                    grade,
                    symbol_bounds: union_bounds(numerator.bounds, denominator.bounds),
                    member_ids: vec![numerator.id, denominator.id],
                    staff_id: Some(input.staff_id),
                    in_sig: false,
                    original_glyphs_registered: false,
                    removed: false,
                });
            }
        }
        let filter_passed = !candidates.is_empty();
        Ok(HeaderTimeRecognition {
            range: proposals.range,
            candidates,
            filter_passed,
        })
    }
}

fn contribution_of(partner: f64, ratio: f64) -> f64 {
    partner * (ratio - 1.0)
}

fn contextual(intrinsic: f64, contribution: f64) -> f64 {
    ((1.0 + contribution) * intrinsic) / (1.0 + (contribution * intrinsic))
}

fn union_bounds(one: HeaderBounds, two: HeaderBounds) -> HeaderBounds {
    let x = one.x.min(two.x);
    let y = one.y.min(two.y);
    let right = one.right().max(two.right());
    let bottom = (one.y + one.height - 1).max(two.y + two.height - 1);
    HeaderBounds {
        x,
        y,
        width: right - x + 1,
        height: bottom - y + 1,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum HeaderTimeColumnError<VisualError> {
    MissingHeader {
        staff_id: usize,
    },
    DuplicateInterId {
        staff_id: usize,
        inter_id: usize,
    },
    InvalidCandidate {
        staff_id: usize,
        inter_id: usize,
    },
    Visual {
        staff_id: usize,
        source: VisualError,
    },
}

impl<VisualError: fmt::Display> fmt::Display for HeaderTimeColumnError<VisualError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHeader { staff_id } => write!(formatter, "staff {staff_id} has no header"),
            Self::DuplicateInterId { staff_id, inter_id } => write!(
                formatter,
                "staff {staff_id} time candidate duplicates live SIG inter {inter_id}"
            ),
            Self::InvalidCandidate { staff_id, inter_id } => write!(
                formatter,
                "staff {staff_id} time candidate {inter_id} has invalid pair members or value"
            ),
            Self::Visual { staff_id, source } => {
                write!(
                    formatter,
                    "staff {staff_id} visual time recognition failed: {source}"
                )
            }
        }
    }
}

impl<VisualError: Error + 'static> Error for HeaderTimeColumnError<VisualError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Visual { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralTimeBuilderState {
    pub input: HeaderTimeRecognitionInput,
    pub candidates: Vec<NeutralTimeCandidate>,
    pub selected_candidate_id: Option<usize>,
}

pub struct HeadlessHeaderTimeColumn<Visual> {
    visual: Visual,
    builders: BTreeMap<usize, NeutralTimeBuilderState>,
    time_value: Option<NeutralTimeValue>,
}

impl<Visual> HeadlessHeaderTimeColumn<Visual> {
    #[must_use]
    pub const fn new(visual: Visual) -> Self {
        Self {
            visual,
            builders: BTreeMap::new(),
            time_value: None,
        }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }

    #[must_use]
    pub fn builders(&self) -> &BTreeMap<usize, NeutralTimeBuilderState> {
        &self.builders
    }

    #[must_use]
    pub const fn time_value(&self) -> Option<NeutralTimeValue> {
        self.time_value
    }
}

impl<Visual> HeadlessHeaderTimeColumn<Visual>
where
    Visual: VisualHeaderTimeRecognizer,
{
    /// Java `HeaderTimeColumn.retrieveTime`: maximum exclusive time-stop
    /// offset, or `-1` for an invalid column.
    pub fn retrieve_time(
        &mut self,
        system: &mut HeadlessHeaderSystem,
    ) -> Result<i32, HeaderTimeColumnError<Visual::Error>> {
        self.builders.clear();
        self.time_value = None;

        // Allocate all builders in source staff order. The constructor installs
        // a missing time range immediately and reuses an existing one unchanged.
        for staff in &mut system.staffs {
            if staff.tablature {
                continue;
            }
            let header = staff
                .header
                .as_mut()
                .ok_or(HeaderTimeColumnError::MissingHeader { staff_id: staff.id })?;
            let browse_start = header.stop;
            let range = header.time_range.get_or_insert_with(|| {
                let mut range = StaffHeaderRange::default();
                range.browse_start = browse_start;
                range
            });
            self.builders.insert(
                staff.id,
                NeutralTimeBuilderState {
                    input: HeaderTimeRecognitionInput {
                        system_id: system.id,
                        staff_id: staff.id,
                        browse_start: range.browse_start,
                    },
                    candidates: Vec::new(),
                    selected_candidate_id: None,
                },
            );
        }

        // Process builders in Staff.byId TreeMap order.
        for staff_id in self.builders.keys().copied().collect::<Vec<_>>() {
            let input = self.builders[&staff_id].input;
            let staff_index = system
                .staffs
                .iter()
                .position(|staff| staff.id == staff_id)
                .ok_or(HeaderTimeColumnError::MissingHeader { staff_id })?;
            let range = system.staffs[staff_index]
                .header
                .as_ref()
                .and_then(|header| header.time_range.as_ref())
                .expect("builder allocation installed range")
                .clone();
            let recognition = self
                .visual
                .recognize_time(input, &range)
                .map_err(|source| HeaderTimeColumnError::Visual { staff_id, source })?;

            // Projection range mutation precedes candidate registration/filter.
            system.staffs[staff_index]
                .header
                .as_mut()
                .expect("header checked during allocation")
                .time_range = Some(recognition.range);
            self.register_candidates(system, staff_id, recognition.candidates)?;
            if !recognition.filter_passed {
                self.cleanup(system);
                return Ok(-1);
            }
        }

        if !self.check_consistency(system) {
            // Java does not clean up a column that merely fails system-level
            // consistency; candidate SIG mutations remain for diagnostics.
            return Ok(-1);
        }

        // Java `HeaderTimeColumn.retrieveTime` computes the offset from `Staff.getTimeStop()`
        // -- the getter, inclusive right edge -- not from the exclusive value `setTimeStop`
        // stored a moment earlier. See `StaffHeader::time_stop`.
        Ok(system
            .staffs
            .iter()
            .filter(|staff| !staff.tablature)
            .filter_map(|staff| {
                let header = staff.header.as_ref()?;
                Some(header.time_stop()?.wrapping_sub(header.start))
            })
            .max()
            .unwrap_or(0))
    }

    fn register_candidates(
        &mut self,
        system: &mut HeadlessHeaderSystem,
        staff_id: usize,
        candidates: Vec<NeutralTimeCandidate>,
    ) -> Result<(), HeaderTimeColumnError<Visual::Error>> {
        let mut candidates = candidates;
        // Java value-vector traversal visits whole list before num/den pairs.
        candidates.sort_by_key(|candidate| candidate.kind == NeutralTimeCandidateKind::Pair);
        for mut candidate in candidates {
            if candidate.denominator_is_invalid()
                || (candidate.kind == NeutralTimeCandidateKind::Whole
                    && !candidate.member_ids.is_empty())
                || (candidate.kind == NeutralTimeCandidateKind::Pair
                    && candidate.member_ids.len() != 2)
            {
                return Err(HeaderTimeColumnError::InvalidCandidate {
                    staff_id,
                    inter_id: candidate.id,
                });
            }
            if system.sig_vertex_ids.contains(&candidate.id) {
                return Err(HeaderTimeColumnError::DuplicateInterId {
                    staff_id,
                    inter_id: candidate.id,
                });
            }
            system.sig_vertex_ids.push(candidate.id);
            // Pair members can be shared by multiple pair candidates in one staff.
            for member_id in &candidate.member_ids {
                if !system.sig_vertex_ids.contains(member_id) {
                    system.sig_vertex_ids.push(*member_id);
                }
            }
            candidate.staff_id = Some(staff_id);
            candidate.in_sig = true;
            self.builders
                .get_mut(&staff_id)
                .expect("builder allocated")
                .candidates
                .push(candidate);
        }
        Ok(())
    }

    fn check_consistency(&mut self, system: &mut HeadlessHeaderSystem) -> bool {
        let (value_order, vectors) = self.value_vectors(system);
        let mut complete = Vec::new();
        for value in value_order {
            let vector = &vectors[&value];
            let mut mean = 0.0;
            let mut count = 0;
            let mut missing = false;
            for (staff_index, staff) in system.staffs.iter().enumerate() {
                if staff.tablature {
                    continue;
                }
                let Some(candidate_id) = vector[staff_index] else {
                    missing = true;
                    break;
                };
                mean += self
                    .candidate(candidate_id)
                    .expect("vector ID exists")
                    .grade;
                count += 1;
            }
            if !missing {
                complete.push((value, mean / f64::from(count)));
            }
        }

        // Java iterates a HashMap. Reproduce its default-table bucket order;
        // strict `>` keeps the first entry on equal means.
        complete.sort_by_key(|(value, _)| java_hash_bucket(*value));
        let mut best_grade = 0.0;
        for (value, grade) in complete {
            if grade > best_grade {
                best_grade = grade;
                self.time_value = Some(value);
            }
        }
        let Some(chosen_value) = self.time_value else {
            return false;
        };
        let chosen = &vectors[&chosen_value];
        let selections = system
            .staffs
            .iter()
            .enumerate()
            .filter(|(_, staff)| !staff.tablature)
            .map(|(staff_index, staff)| (staff.id, chosen[staff_index].expect("complete vector")))
            .collect::<Vec<_>>();
        for (staff_id, candidate_id) in selections {
            self.select_staff(system, staff_id, candidate_id);
        }
        true
    }

    fn value_vectors(
        &self,
        system: &HeadlessHeaderSystem,
    ) -> (
        Vec<NeutralTimeValue>,
        BTreeMap<NeutralTimeValue, Vec<Option<usize>>>,
    ) {
        let mut order = Vec::new();
        let mut vectors = BTreeMap::new();
        for (staff_index, staff) in system.staffs.iter().enumerate() {
            if staff.tablature {
                continue;
            }
            let builder = &self.builders[&staff.id];
            for candidate in builder
                .candidates
                .iter()
                .filter(|candidate| !candidate.removed)
            {
                vectors.entry(candidate.value).or_insert_with(|| {
                    order.push(candidate.value);
                    vec![None; system.staffs.len()]
                });
                let slot = &mut vectors.get_mut(&candidate.value).unwrap()[staff_index];
                if slot.is_none_or(|id| candidate.grade > self.candidate(id).unwrap().grade) {
                    *slot = Some(candidate.id);
                }
            }
        }
        (order, vectors)
    }

    fn candidate(&self, id: usize) -> Option<&NeutralTimeCandidate> {
        self.builders
            .values()
            .flat_map(|builder| &builder.candidates)
            .find(|candidate| candidate.id == id)
    }

    fn select_staff(
        &mut self,
        system: &mut HeadlessHeaderSystem,
        staff_id: usize,
        selected_id: usize,
    ) {
        let builder = self.builders.get_mut(&staff_id).unwrap();
        let selected_index = builder
            .candidates
            .iter()
            .position(|candidate| candidate.id == selected_id)
            .expect("value vector candidate exists");
        builder.candidates[selected_index].original_glyphs_registered = true;
        builder.selected_candidate_id = Some(selected_id);
        let selected = builder.candidates[selected_index].clone();
        let retained_members = selected.member_ids.to_vec();

        for (index, candidate) in builder.candidates.iter_mut().enumerate() {
            if index != selected_index {
                remove_sig_id(system, candidate.id);
                candidate.in_sig = false;
                candidate.removed = true;
            }
        }
        let all_members = builder
            .candidates
            .iter()
            .flat_map(|candidate| candidate.member_ids.iter().copied())
            .collect::<Vec<_>>();
        for member in all_members {
            if !retained_members.contains(&member) {
                remove_sig_id(system, member);
            }
        }

        let staff = system
            .staffs
            .iter_mut()
            .find(|staff| staff.id == staff_id)
            .unwrap();
        staff.time_stop = Some(
            selected
                .symbol_bounds
                .x
                .wrapping_add(selected.symbol_bounds.width),
        );
        staff.header.as_mut().unwrap().time =
            Some(HeaderComponent::new(selected.id, selected.symbol_bounds));
    }

    fn cleanup(&mut self, system: &mut HeadlessHeaderSystem) {
        for builder in self.builders.values_mut() {
            for candidate in &mut builder.candidates {
                remove_sig_id(system, candidate.id);
                for member in &candidate.member_ids {
                    remove_sig_id(system, *member);
                }
                candidate.in_sig = false;
                candidate.removed = true;
            }
        }
    }
}

impl NeutralTimeCandidate {
    fn denominator_is_invalid(&self) -> bool {
        self.value.denominator == 0 || self.value.numerator == 0
    }
}

impl Ord for NeutralTimeValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.numerator, self.denominator, self.specific_shape).cmp(&(
            other.numerator,
            other.denominator,
            other.specific_shape,
        ))
    }
}

impl PartialOrd for NeutralTimeValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NeutralSpecificTimeShape {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

impl PartialOrd for NeutralSpecificTimeShape {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn remove_sig_id(system: &mut HeadlessHeaderSystem, id: usize) {
    system.sig_vertex_ids.retain(|candidate| *candidate != id);
    system
        .sig_exclusions
        .retain(|exclusion| exclusion.one != id && exclusion.two != id);
}

/// Java `TimeValue.hashCode`, `HashMap.hash`, and default 16-bucket table.
fn java_hash_bucket(value: NeutralTimeValue) -> u32 {
    let mut rational_hash = 7_i32;
    rational_hash = rational_hash.wrapping_mul(97).wrapping_add(value.numerator);
    rational_hash = rational_hash
        .wrapping_mul(97)
        .wrapping_add(value.denominator);
    let hash = 7_i32.wrapping_mul(83).wrapping_add(rational_hash);
    let spread = hash ^ ((hash as u32 >> 16) as i32);
    (spread as u32) & 15
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{headers_step::HeadlessHeaderStaff, staff_header::StaffHeader};

    #[derive(Default)]
    struct FakeVisual {
        results: BTreeMap<usize, Result<HeaderTimeRecognition, &'static str>>,
        calls: Vec<HeaderTimeRecognitionInput>,
    }

    #[derive(Default)]
    struct FakeProposalVisual {
        proposals: BTreeMap<usize, HeaderTimeClassifierProposals>,
    }

    impl VisualHeaderTimeProposalRecognizer for FakeProposalVisual {
        type Error = &'static str;

        fn classify_time_parts(
            &mut self,
            input: HeaderTimeRecognitionInput,
            _range: &StaffHeaderRange,
        ) -> Result<HeaderTimeClassifierProposals, Self::Error> {
            Ok(self.proposals.remove(&input.staff_id).unwrap())
        }
    }

    impl VisualHeaderTimeRecognizer for FakeVisual {
        type Error = &'static str;

        fn recognize_time(
            &mut self,
            input: HeaderTimeRecognitionInput,
            _range: &StaffHeaderRange,
        ) -> Result<HeaderTimeRecognition, Self::Error> {
            self.calls.push(input);
            self.results.remove(&input.staff_id).unwrap_or_else(|| {
                Ok(HeaderTimeRecognition {
                    range: StaffHeaderRange::default(),
                    candidates: Vec::new(),
                    filter_passed: false,
                })
            })
        }
    }

    fn value(numerator: i32, denominator: i32) -> NeutralTimeValue {
        NeutralTimeValue {
            specific_shape: None,
            numerator,
            denominator,
        }
    }

    fn bounds(x: i32, width: i32) -> HeaderBounds {
        HeaderBounds {
            x,
            y: 3,
            width,
            height: 22,
        }
    }

    fn whole(id: usize, time: NeutralTimeValue, grade: f64, x: i32) -> NeutralTimeCandidate {
        NeutralTimeCandidate {
            id,
            kind: NeutralTimeCandidateKind::Whole,
            value: time,
            grade,
            symbol_bounds: bounds(x, 8),
            member_ids: Vec::new(),
            staff_id: None,
            in_sig: false,
            original_glyphs_registered: false,
            removed: false,
        }
    }

    fn pair(
        id: usize,
        members: [usize; 2],
        time: NeutralTimeValue,
        grade: f64,
        x: i32,
    ) -> NeutralTimeCandidate {
        NeutralTimeCandidate {
            id,
            kind: NeutralTimeCandidateKind::Pair,
            value: time,
            grade,
            symbol_bounds: bounds(x, 10),
            member_ids: members.to_vec(),
            staff_id: None,
            in_sig: false,
            original_glyphs_registered: false,
            removed: false,
        }
    }

    fn recognition(
        browse_start: i32,
        candidates: Vec<NeutralTimeCandidate>,
        filter_passed: bool,
    ) -> HeaderTimeRecognition {
        let mut range = StaffHeaderRange::default();
        range.browse_start = browse_start;
        range.browse_stop = browse_start + 30;
        range.set_start(browse_start + 2);
        range.set_stop(browse_start + 15);
        range.valid = filter_passed;
        HeaderTimeRecognition {
            range,
            candidates,
            filter_passed,
        }
    }

    fn staff(id: usize, start: i32, stop: i32) -> HeadlessHeaderStaff {
        let mut staff = HeadlessHeaderStaff::new(id);
        let mut header = StaffHeader::new(start);
        header.stop = stop;
        staff.header = Some(header);
        staff
    }

    #[test]
    fn allocates_source_order_processes_staff_id_order_and_reuses_existing_range() {
        let mut tablature = staff(2, 20, 27);
        tablature.tablature = true;
        let mut five = staff(5, 10, 18);
        let mut existing = StaffHeaderRange::default();
        existing.browse_start = 99;
        five.header.as_mut().unwrap().time_range = Some(existing);
        let mut system = HeadlessHeaderSystem::new(7, vec![five, tablature, staff(3, 30, 37)]);
        let mut visual = FakeVisual::default();
        visual.results.insert(
            5,
            Ok(recognition(
                99,
                vec![whole(50, value(4, 4), 0.8, 100)],
                true,
            )),
        );
        visual.results.insert(
            3,
            Ok(recognition(37, vec![whole(30, value(4, 4), 0.7, 39)], true)),
        );
        let mut column = HeadlessHeaderTimeColumn::new(visual);

        // One less than the stored (exclusive) time stop: Java's `HeaderTimeColumn.retrieveTime`
        // computes the offset from the `getTimeStop()` getter, whose answer is the inclusive
        // right edge -- the corpus caught the +1 on all seventeen time-bearing staves.
        assert_eq!(column.retrieve_time(&mut system), Ok(97));
        assert_eq!(
            column
                .visual()
                .calls
                .iter()
                .map(|call| call.staff_id)
                .collect::<Vec<_>>(),
            vec![3, 5]
        );
        assert_eq!(column.builders()[&5].input.browse_start, 99);
        assert_eq!(column.builders()[&3].input.browse_start, 37);
        assert!(
            system.staffs[1]
                .header
                .as_ref()
                .unwrap()
                .time_range
                .is_none()
        );
        assert_eq!(system.staffs[0].time_stop, Some(108));
        assert_eq!(system.staffs[2].time_stop, Some(47));
    }

    #[test]
    fn chooses_highest_staff_candidate_for_complete_value_and_discards_every_other_kind() {
        let mut system = HeadlessHeaderSystem::new(7, vec![staff(1, 10, 15), staff(2, 20, 25)]);
        let mut visual = FakeVisual::default();
        visual.results.insert(
            1,
            Ok(recognition(
                15,
                vec![
                    pair(110, [111, 112], value(3, 4), 0.9, 18),
                    whole(100, value(3, 4), 0.7, 17),
                    whole(120, value(6, 8), 0.95, 17),
                ],
                true,
            )),
        );
        visual.results.insert(
            2,
            Ok(recognition(
                25,
                vec![
                    whole(200, value(3, 4), 0.8, 28),
                    whole(220, value(6, 8), 0.1, 28),
                ],
                true,
            )),
        );
        let mut column = HeadlessHeaderTimeColumn::new(visual);

        // Inclusive-getter offset; see the note in the sibling test.
        assert_eq!(column.retrieve_time(&mut system), Ok(17));
        assert_eq!(column.time_value(), Some(value(3, 4)));
        assert_eq!(column.builders()[&1].selected_candidate_id, Some(110));
        assert_eq!(column.builders()[&2].selected_candidate_id, Some(200));
        assert_eq!(system.sig_vertex_ids, vec![110, 111, 112, 200]);
        assert!(
            column.builders()[&1]
                .candidates
                .iter()
                .find(|candidate| candidate.id == 110)
                .unwrap()
                .original_glyphs_registered
        );
    }

    #[test]
    fn local_filter_failure_cleans_completed_and_current_staff_candidates() {
        let mut system = HeadlessHeaderSystem::new(7, vec![staff(1, 10, 15), staff(2, 20, 25)]);
        let mut visual = FakeVisual::default();
        visual.results.insert(
            1,
            Ok(recognition(15, vec![whole(10, value(3, 4), 0.8, 18)], true)),
        );
        visual.results.insert(
            2,
            Ok(recognition(
                25,
                vec![whole(20, value(4, 4), 0.8, 28)],
                false,
            )),
        );
        let mut column = HeadlessHeaderTimeColumn::new(visual);

        assert_eq!(column.retrieve_time(&mut system), Ok(-1));
        assert!(system.sig_vertex_ids.is_empty());
        assert!(
            column
                .builders()
                .values()
                .flat_map(|builder| &builder.candidates)
                .all(|candidate| candidate.removed)
        );
    }

    #[test]
    fn incomplete_system_column_fails_without_cleanup() {
        let mut system = HeadlessHeaderSystem::new(7, vec![staff(1, 10, 15), staff(2, 20, 25)]);
        let mut visual = FakeVisual::default();
        visual.results.insert(
            1,
            Ok(recognition(15, vec![whole(10, value(3, 4), 0.8, 18)], true)),
        );
        visual.results.insert(
            2,
            Ok(recognition(25, vec![whole(20, value(4, 4), 0.8, 28)], true)),
        );
        let mut column = HeadlessHeaderTimeColumn::new(visual);

        assert_eq!(column.retrieve_time(&mut system), Ok(-1));
        assert_eq!(system.sig_vertex_ids, vec![10, 20]);
        assert!(
            column
                .builders()
                .values()
                .flat_map(|builder| &builder.candidates)
                .all(|candidate| !candidate.removed)
        );
    }

    #[test]
    fn visual_failure_retains_ranges_and_completed_sig_prefix() {
        let mut system = HeadlessHeaderSystem::new(7, vec![staff(1, 10, 15), staff(2, 20, 25)]);
        let mut visual = FakeVisual::default();
        visual.results.insert(
            1,
            Ok(recognition(15, vec![whole(10, value(3, 4), 0.8, 18)], true)),
        );
        visual.results.insert(2, Err("classifier offline"));
        let mut column = HeadlessHeaderTimeColumn::new(visual);

        assert_eq!(
            column.retrieve_time(&mut system),
            Err(HeaderTimeColumnError::Visual {
                staff_id: 2,
                source: "classifier offline"
            })
        );
        assert_eq!(system.sig_vertex_ids, vec![10]);
        assert_eq!(column.builders()[&1].candidates.len(), 1);
        assert!(column.builders()[&2].candidates.is_empty());
        assert_eq!(
            system.staffs[1]
                .header
                .as_ref()
                .unwrap()
                .time_range
                .as_ref()
                .unwrap()
                .browse_start,
            25
        );
    }

    #[test]
    fn equal_column_means_follow_java_hash_bucket_then_strict_tie() {
        let three_four = value(3, 4);
        let four_four = value(4, 4);
        assert_ne!(java_hash_bucket(three_four), java_hash_bucket(four_four));
        let expected = if java_hash_bucket(three_four) < java_hash_bucket(four_four) {
            three_four
        } else {
            four_four
        };
        let mut system = HeadlessHeaderSystem::new(7, vec![staff(1, 10, 15)]);
        let mut visual = FakeVisual::default();
        visual.results.insert(
            1,
            Ok(recognition(
                15,
                vec![
                    whole(10, three_four, 0.8, 18),
                    whole(20, four_four, 0.8, 18),
                ],
                true,
            )),
        );
        let mut column = HeadlessHeaderTimeColumn::new(visual);
        assert!(column.retrieve_time(&mut system).unwrap() >= 0);
        assert_eq!(column.time_value(), Some(expected));
    }

    fn lifecycle_range() -> StaffHeaderRange {
        let mut range = StaffHeaderRange::default();
        range.browse_start = 15;
        range.browse_stop = 45;
        range.set_start(18);
        range.set_stop(35);
        range.valid = true;
        range
    }

    fn number(
        id: usize,
        value: i32,
        grade: f64,
        center_x: i32,
        y: i32,
    ) -> TimeNumberClassifierProposal {
        TimeNumberClassifierProposal {
            id,
            value,
            grade,
            bounds: HeaderBounds {
                x: center_x - 3,
                y,
                width: 7,
                height: 10,
            },
            center_x,
        }
    }

    fn lifecycle_context() -> HeaderTimeLifecycleContext {
        HeaderTimeLifecycleContext {
            maximum_halves_dx: 3,
            top_bottom_source_ratio: 2.0,
            pair_ids: BTreeMap::from([((11, 21), 100), ((12, 21), 101)]),
            supported_values: vec![
                (2, 2),
                (3, 2),
                (2, 4),
                (3, 4),
                (4, 4),
                (5, 4),
                (6, 4),
                (3, 8),
                (6, 8),
                (9, 8),
                (12, 8),
            ],
        }
    }

    #[test]
    fn time_lifecycle_preserves_whole_then_top_bottom_pair_order_and_contextual_grade() {
        let proposals = HeaderTimeClassifierProposals {
            range: lifecycle_range(),
            wholes: vec![TimeWholeClassifierProposal {
                id: 1,
                value: value(4, 4),
                grade: 0.7,
                symbol_bounds: bounds(18, 8),
            }],
            numerators: vec![number(11, 3, 0.8, 25, 10), number(12, 6, 0.6, 26, 10)],
            denominators: vec![number(21, 8, 0.9, 24, 25)],
        };
        let visual = FakeProposalVisual {
            proposals: BTreeMap::from([(1, proposals)]),
        };
        let mut lifecycle =
            HeaderTimeLifecycleRecognizer::new(visual, BTreeMap::from([(1, lifecycle_context())]));
        let result = lifecycle
            .recognize_time(
                HeaderTimeRecognitionInput {
                    system_id: 7,
                    staff_id: 1,
                    browse_start: 15,
                },
                &StaffHeaderRange::default(),
            )
            .unwrap();
        assert!(result.filter_passed);
        assert_eq!(
            result
                .candidates
                .iter()
                .map(|candidate| candidate.id)
                .collect::<Vec<_>>(),
            vec![1, 100, 101]
        );
        assert_eq!(result.candidates[1].member_ids, vec![11, 21]);
        assert!(result.candidates[1].grade > 0.85);
        assert_eq!(
            result.candidates[1].symbol_bounds,
            HeaderBounds {
                x: 21,
                y: 10,
                width: 8,
                height: 25
            }
        );
    }

    #[test]
    fn time_lifecycle_rejects_unsupported_or_misaligned_pairs_and_fails_empty_filter() {
        let proposals = HeaderTimeClassifierProposals {
            range: lifecycle_range(),
            wholes: Vec::new(),
            numerators: vec![number(11, 7, 0.8, 10, 10), number(12, 3, 0.8, 30, 10)],
            denominators: vec![number(21, 8, 0.9, 20, 25)],
        };
        let visual = FakeProposalVisual {
            proposals: BTreeMap::from([(1, proposals)]),
        };
        let mut lifecycle =
            HeaderTimeLifecycleRecognizer::new(visual, BTreeMap::from([(1, lifecycle_context())]));
        let result = lifecycle
            .recognize_time(
                HeaderTimeRecognitionInput {
                    system_id: 7,
                    staff_id: 1,
                    browse_start: 15,
                },
                &StaffHeaderRange::default(),
            )
            .unwrap();
        assert!(!result.filter_passed);
        assert!(result.candidates.is_empty());
        assert_eq!(result.range.precise_stop(), Some(35));
    }

    #[test]
    fn lifecycle_integrates_with_column_cleanup_on_empty_standard_staff() {
        let visual = FakeProposalVisual {
            proposals: BTreeMap::from([(
                1,
                HeaderTimeClassifierProposals {
                    range: lifecycle_range(),
                    wholes: Vec::new(),
                    numerators: Vec::new(),
                    denominators: Vec::new(),
                },
            )]),
        };
        let lifecycle =
            HeaderTimeLifecycleRecognizer::new(visual, BTreeMap::from([(1, lifecycle_context())]));
        let mut column = HeadlessHeaderTimeColumn::new(lifecycle);
        let mut system = HeadlessHeaderSystem::new(7, vec![staff(1, 10, 15)]);
        assert_eq!(column.retrieve_time(&mut system), Ok(-1));
        assert!(system.sig_vertex_ids.is_empty());
        assert_eq!(
            system.staffs[0]
                .header
                .as_ref()
                .unwrap()
                .time_range
                .as_ref()
                .unwrap()
                .precise_stop(),
            Some(35)
        );
    }
}
