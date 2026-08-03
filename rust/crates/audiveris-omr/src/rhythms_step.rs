// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle and ownership port of Java `RhythmsStep`.
//!
//! `PageRhythm`, `StackRhythm`, slot mapping, voice inference and duration
//! analysis remain injected at the first semantic dependency. This module owns
//! page/system/stack traversal, rhythm-state mutation, identity validation,
//! checked continuation, impact order, and retained failure prefixes.

use std::{collections::BTreeSet, error::Error, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RhythmRational {
    pub numerator: i64,
    pub denominator: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralRhythmInterClass {
    AugmentationDot,
    Barline,
    BeamHook,
    Beam,
    Flag,
    HeadChord,
    Head,
    RestChord,
    Rest,
    SmallBeam,
    SmallFlag,
    StaffBarline,
    Stem,
    Tuplet,
    Time,
    Brace,
    Slur,
    TimeNumber,
    SmallChord,
    GraceChord,
    HiddenHead,
    HiddenStem,
    Other,
}

impl NeutralRhythmInterClass {
    #[must_use]
    pub const fn impacts_page(self) -> bool {
        matches!(
            self,
            Self::Time | Self::Brace | Self::Slur | Self::TimeNumber
        )
    }

    #[must_use]
    pub const fn impacts_stack(self) -> bool {
        matches!(
            self,
            Self::AugmentationDot
                | Self::Barline
                | Self::BeamHook
                | Self::Beam
                | Self::Flag
                | Self::HeadChord
                | Self::Head
                | Self::RestChord
                | Self::Rest
                | Self::SmallBeam
                | Self::SmallFlag
                | Self::StaffBarline
                | Self::Stem
                | Self::Tuplet
        )
    }

    #[must_use]
    pub const fn is_white(self) -> bool {
        matches!(
            self,
            Self::SmallChord | Self::GraceChord | Self::HiddenHead | Self::HiddenStem
        )
    }

    #[must_use]
    pub const fn is_frat(self) -> bool {
        matches!(
            self,
            Self::Flag | Self::RestChord | Self::AugmentationDot | Self::Tuplet
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralRhythmInter {
    pub id: usize,
    pub class: NeutralRhythmInterClass,
    pub stack_id: Option<usize>,
    pub measure_id: Option<usize>,
    pub abscissa: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralRhythmRelationClass {
    Augmentation,
    BeamStem,
    ChordTuplet,
    DoubleDot,
    HeadStem,
    NextInVoice,
    SameTime,
    SeparateTime,
    SeparateVoice,
    Other,
}

impl NeutralRhythmRelationClass {
    #[must_use]
    pub const fn impacts_stack(self) -> bool {
        !matches!(self, Self::Other)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralRhythmRelation {
    pub id: usize,
    pub class: NeutralRhythmRelationClass,
    pub source_inter_id: usize,
    pub target_inter_id: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RhythmOperation {
    Do,
    Undo,
    Redo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RhythmEditKind {
    Addition,
    Removal,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RhythmImpactTask {
    Inter {
        inter_id: usize,
        edit: RhythmEditKind,
    },
    Stack {
        stack_id: usize,
    },
    Page {
        page_id: usize,
    },
    SystemMerge {
        system_id: usize,
    },
    Relation {
        relation_id: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralSlotChordStatus {
    Begin,
    Continue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralVoiceSlotInfo {
    pub slot_id: usize,
    pub chord_id: usize,
    pub status: NeutralSlotChordStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralRhythmVoice {
    pub id: usize,
    pub measure_rest: bool,
    pub chord_ids: Vec<usize>,
    pub slot_infos: Vec<NeutralVoiceSlotInfo>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralRhythmMeasure {
    pub id: usize,
    pub abnormal: bool,
    pub frat_inter_ids: Vec<usize>,
    pub chord_ids: Vec<usize>,
    pub voices: Vec<NeutralRhythmVoice>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralRhythmSlot {
    pub id: usize,
    pub time_offset: RhythmRational,
    pub chord_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralRhythmStack {
    pub id: usize,
    pub abnormal: bool,
    pub expected_duration: Option<RhythmRational>,
    pub actual_duration: Option<RhythmRational>,
    pub time_signature_inter_ids: Vec<usize>,
    pub frat_inter_ids: Vec<usize>,
    pub measures: Vec<NeutralRhythmMeasure>,
    pub slots: Vec<NeutralRhythmSlot>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralRhythmSystem {
    pub id: usize,
    pub inters: Vec<NeutralRhythmInter>,
    pub relations: Vec<NeutralRhythmRelation>,
    pub stacks: Vec<NeutralRhythmStack>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralRhythmPage {
    pub id: usize,
    pub systems: Vec<NeutralRhythmSystem>,
    pub last_time_rational: Option<RhythmRational>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralRhythmSheet {
    pub id: usize,
    pub pages: Vec<NeutralRhythmPage>,
    pub mutations: Vec<RhythmsMutation>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RhythmsDelta {
    pub mutations: Vec<RhythmsDeltaMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RhythmsDeltaMutation {
    SetPageLastTime {
        value: Option<RhythmRational>,
    },
    AttachTimeSignature {
        stack_id: usize,
        inter_id: usize,
    },
    AttachFrat {
        stack_id: usize,
        measure_id: usize,
        inter_id: usize,
    },
    ResetStack {
        stack_id: usize,
    },
    SetStackExpectedDuration {
        stack_id: usize,
        value: Option<RhythmRational>,
    },
    SetStackActualDuration {
        stack_id: usize,
        value: Option<RhythmRational>,
    },
    SetStackAbnormal {
        stack_id: usize,
        abnormal: bool,
    },
    ResetMeasure {
        measure_id: usize,
    },
    SetMeasureAbnormal {
        measure_id: usize,
        abnormal: bool,
    },
    AddVoice {
        measure_id: usize,
        voice: NeutralRhythmVoice,
    },
    AddSlot {
        stack_id: usize,
        slot: NeutralRhythmSlot,
    },
    PutVoiceSlot {
        measure_id: usize,
        voice_id: usize,
        info: NeutralVoiceSlotInfo,
    },
    ReassignVoiceId {
        measure_id: usize,
        old_id: usize,
        new_id: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RhythmsStageOutcome<SemanticError> {
    pub delta: RhythmsDelta,
    pub error: Option<SemanticError>,
}

impl<SemanticError> RhythmsStageOutcome<SemanticError> {
    #[must_use]
    pub fn success(delta: RhythmsDelta) -> Self {
        Self { delta, error: None }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RhythmPageInput<'a> {
    pub sheet_id: usize,
    pub page: &'a NeutralRhythmPage,
}

#[derive(Clone, Copy, Debug)]
pub struct RhythmStackInput<'a> {
    pub sheet_id: usize,
    pub page_id: usize,
    pub system_id: usize,
    pub stack: &'a NeutralRhythmStack,
}

/// First semantic dependency: page time ranges, FRAT assignment,
/// measure/voice inference, slot mapping, and duration analysis.
pub trait SemanticRhythmAnalyzer {
    type Error;

    fn process_page(&mut self, input: RhythmPageInput<'_>) -> RhythmsStageOutcome<Self::Error>;

    fn reprocess_stack(&mut self, input: RhythmStackInput<'_>) -> RhythmsStageOutcome<Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RhythmPhase {
    Page,
    StackImpact,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RhythmFailure<SemanticError> {
    pub page_id: usize,
    pub stack_id: Option<usize>,
    pub phase: RhythmPhase,
    pub source: SemanticError,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RhythmsReport<SemanticError> {
    pub failures: Vec<RhythmFailure<SemanticError>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RhythmsStepError {
    Contract(RhythmsContractError),
}

impl fmt::Display for RhythmsStepError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(source) => write!(formatter, "rhythms contract failed: {source}"),
        }
    }
}

impl Error for RhythmsStepError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RhythmsContractError {
    DuplicatePage(usize),
    DuplicateSystem(usize),
    DuplicateStack(usize),
    DuplicateMeasure(usize),
    InvalidRational(RhythmRational),
    UnknownPage(usize),
    UnknownSystem(usize),
    UnknownStack(usize),
    UnknownMeasure(usize),
    UnknownInter(usize),
    UnknownRelation(usize),
    WrongInterClass(usize),
    InterStackMismatch { inter_id: usize, stack_id: usize },
    InterMeasureMismatch { inter_id: usize, measure_id: usize },
    DuplicateStackInter { stack_id: usize, inter_id: usize },
    DuplicateMeasureInter { measure_id: usize, inter_id: usize },
    DuplicateVoice { measure_id: usize, voice_id: usize },
    UnknownVoice { measure_id: usize, voice_id: usize },
    DuplicateSlot { stack_id: usize, slot_id: usize },
    UnknownSlot { stack_id: usize, slot_id: usize },
    UnknownChord { measure_id: usize, chord_id: usize },
    DuplicateVoiceSlot { voice_id: usize, slot_id: usize },
}

impl fmt::Display for RhythmsContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for RhythmsContractError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RhythmsMutation {
    PageLastTimeSet {
        page_id: usize,
    },
    TimeSignatureAttached {
        stack_id: usize,
        inter_id: usize,
    },
    FratAttached {
        stack_id: usize,
        measure_id: usize,
        inter_id: usize,
    },
    StackReset {
        stack_id: usize,
    },
    StackExpectedDurationSet {
        stack_id: usize,
    },
    StackActualDurationSet {
        stack_id: usize,
    },
    StackAbnormalSet {
        stack_id: usize,
        abnormal: bool,
    },
    MeasureReset {
        measure_id: usize,
    },
    MeasureAbnormalSet {
        measure_id: usize,
        abnormal: bool,
    },
    VoiceAdded {
        measure_id: usize,
        voice_id: usize,
    },
    SlotAdded {
        stack_id: usize,
        slot_id: usize,
    },
    VoiceSlotPut {
        measure_id: usize,
        voice_id: usize,
        slot_id: usize,
    },
    VoiceIdReassigned {
        measure_id: usize,
        old_id: usize,
        new_id: usize,
    },
    Failed {
        page_id: usize,
        stack_id: Option<usize>,
        phase: RhythmPhase,
    },
}

pub struct HeadlessRhythmsStep<Semantic> {
    semantic: Semantic,
}

impl<Semantic> HeadlessRhythmsStep<Semantic> {
    #[must_use]
    pub const fn new(semantic: Semantic) -> Self {
        Self { semantic }
    }

    #[must_use]
    pub const fn semantic(&self) -> &Semantic {
        &self.semantic
    }
}

impl<Semantic: SemanticRhythmAnalyzer> HeadlessRhythmsStep<Semantic> {
    /// Java `RhythmsStep.doit`: pages are processed sequentially in sheet
    /// order. A checked injected failure retains its prefix and later pages
    /// continue, mirroring the deeper Java stack-level catch boundary.
    pub fn process(
        &mut self,
        sheet: &mut NeutralRhythmSheet,
    ) -> Result<RhythmsReport<Semantic::Error>, RhythmsStepError> {
        validate_sheet(sheet).map_err(RhythmsStepError::Contract)?;
        let mut report = RhythmsReport {
            failures: Vec::new(),
        };
        let page_ids = sheet.pages.iter().map(|page| page.id).collect::<Vec<_>>();
        for page_id in page_ids {
            self.process_page_id(sheet, page_id, &mut report)?;
        }
        Ok(report)
    }

    /// Java collection overload: impacted stacks are de-duplicated while
    /// preserving first encounter order, then reprocessed in that order.
    pub fn impact_inters(
        &mut self,
        sheet: &mut NeutralRhythmSheet,
        inter_ids: &[usize],
    ) -> Result<RhythmsReport<Semantic::Error>, RhythmsStepError> {
        validate_sheet(sheet).map_err(RhythmsStepError::Contract)?;
        let mut stack_ids = Vec::new();
        for inter_id in inter_ids {
            let inter = find_inter_in_sheet(sheet, *inter_id)
                .ok_or(RhythmsContractError::UnknownInter(*inter_id))
                .map_err(RhythmsStepError::Contract)?;
            if inter.class.impacts_stack() {
                if let Some(stack_id) = inter.stack_id {
                    push_unique(&mut stack_ids, stack_id);
                }
            }
        }
        let mut report = RhythmsReport {
            failures: Vec::new(),
        };
        for stack_id in stack_ids {
            self.reprocess_stack_id(sheet, stack_id, &mut report)?;
        }
        Ok(report)
    }

    /// Java task-list overload. All impacts are collected before mutation;
    /// pages run first, followed by stacks, each in insertion order.
    pub fn impact_tasks(
        &mut self,
        sheet: &mut NeutralRhythmSheet,
        tasks: &[RhythmImpactTask],
        operation: RhythmOperation,
    ) -> Result<RhythmsReport<Semantic::Error>, RhythmsStepError> {
        validate_sheet(sheet).map_err(RhythmsStepError::Contract)?;
        let mut page_ids = Vec::new();
        let mut stack_ids = Vec::new();
        for task in tasks {
            collect_task_impacts(sheet, *task, operation, &mut page_ids, &mut stack_ids)?;
        }
        let mut report = RhythmsReport {
            failures: Vec::new(),
        };
        for page_id in page_ids {
            self.process_page_id(sheet, page_id, &mut report)?;
        }
        for stack_id in stack_ids {
            self.reprocess_stack_id(sheet, stack_id, &mut report)?;
        }
        Ok(report)
    }

    fn process_page_id(
        &mut self,
        sheet: &mut NeutralRhythmSheet,
        page_id: usize,
        report: &mut RhythmsReport<Semantic::Error>,
    ) -> Result<(), RhythmsStepError> {
        let page_index = sheet
            .pages
            .iter()
            .position(|page| page.id == page_id)
            .ok_or(RhythmsContractError::UnknownPage(page_id))
            .map_err(RhythmsStepError::Contract)?;
        let outcome = self.semantic.process_page(RhythmPageInput {
            sheet_id: sheet.id,
            page: &sheet.pages[page_index],
        });
        apply_delta(sheet, page_index, outcome.delta).map_err(RhythmsStepError::Contract)?;
        if let Some(source) = outcome.error {
            sheet.mutations.push(RhythmsMutation::Failed {
                page_id,
                stack_id: None,
                phase: RhythmPhase::Page,
            });
            report.failures.push(RhythmFailure {
                page_id,
                stack_id: None,
                phase: RhythmPhase::Page,
                source,
            });
        }
        Ok(())
    }

    fn reprocess_stack_id(
        &mut self,
        sheet: &mut NeutralRhythmSheet,
        stack_id: usize,
        report: &mut RhythmsReport<Semantic::Error>,
    ) -> Result<(), RhythmsStepError> {
        let (page_index, system_index, stack_index) = locate_stack(sheet, stack_id)
            .ok_or(RhythmsContractError::UnknownStack(stack_id))
            .map_err(RhythmsStepError::Contract)?;
        let page_id = sheet.pages[page_index].id;
        let system_id = sheet.pages[page_index].systems[system_index].id;
        let outcome = self.semantic.reprocess_stack(RhythmStackInput {
            sheet_id: sheet.id,
            page_id,
            system_id,
            stack: &sheet.pages[page_index].systems[system_index].stacks[stack_index],
        });
        apply_delta(sheet, page_index, outcome.delta).map_err(RhythmsStepError::Contract)?;
        if let Some(source) = outcome.error {
            sheet.mutations.push(RhythmsMutation::Failed {
                page_id,
                stack_id: Some(stack_id),
                phase: RhythmPhase::StackImpact,
            });
            report.failures.push(RhythmFailure {
                page_id,
                stack_id: Some(stack_id),
                phase: RhythmPhase::StackImpact,
                source,
            });
        }
        Ok(())
    }
}

fn valid_rational(value: RhythmRational) -> bool {
    value.denominator != 0
}

fn push_unique(values: &mut Vec<usize>, value: usize) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn collect_task_impacts(
    sheet: &NeutralRhythmSheet,
    task: RhythmImpactTask,
    operation: RhythmOperation,
    page_ids: &mut Vec<usize>,
    stack_ids: &mut Vec<usize>,
) -> Result<(), RhythmsStepError> {
    match task {
        RhythmImpactTask::Inter { inter_id, edit } => {
            let (page_index, system_index, inter) = locate_inter(sheet, inter_id)
                .ok_or(RhythmsContractError::UnknownInter(inter_id))
                .map_err(RhythmsStepError::Contract)?;
            if inter.class.is_white() {
                return Ok(());
            }
            if inter.class.impacts_page() {
                push_unique(page_ids, sheet.pages[page_index].id);
            }
            if inter.class.impacts_stack() {
                if let Some(stack_id) = inter.stack_id {
                    push_unique(stack_ids, stack_id);
                    let includes_next = matches!(
                        inter.class,
                        NeutralRhythmInterClass::Barline | NeutralRhythmInterClass::StaffBarline
                    ) && ((edit == RhythmEditKind::Removal
                        && operation == RhythmOperation::Undo)
                        || (edit == RhythmEditKind::Addition
                            && operation != RhythmOperation::Undo));
                    if includes_next {
                        let system = &sheet.pages[page_index].systems[system_index];
                        if let Some(index) =
                            system.stacks.iter().position(|stack| stack.id == stack_id)
                        {
                            if let Some(next) = system.stacks.get(index + 1) {
                                push_unique(stack_ids, next.id);
                            }
                        }
                    }
                }
            }
        }
        RhythmImpactTask::Stack { stack_id } => {
            locate_stack(sheet, stack_id)
                .ok_or(RhythmsContractError::UnknownStack(stack_id))
                .map_err(RhythmsStepError::Contract)?;
            push_unique(stack_ids, stack_id);
        }
        RhythmImpactTask::Page { page_id } => {
            if !sheet.pages.iter().any(|page| page.id == page_id) {
                return Err(RhythmsStepError::Contract(
                    RhythmsContractError::UnknownPage(page_id),
                ));
            }
            push_unique(page_ids, page_id);
        }
        RhythmImpactTask::SystemMerge { system_id } => {
            let page_id = sheet
                .pages
                .iter()
                .find(|page| page.systems.iter().any(|system| system.id == system_id))
                .map(|page| page.id)
                .ok_or(RhythmsContractError::UnknownSystem(system_id))
                .map_err(RhythmsStepError::Contract)?;
            push_unique(page_ids, page_id);
        }
        RhythmImpactTask::Relation { relation_id } => {
            let (_, _, relation) = locate_relation(sheet, relation_id)
                .ok_or(RhythmsContractError::UnknownRelation(relation_id))
                .map_err(RhythmsStepError::Contract)?;
            if relation.class.impacts_stack() {
                for inter_id in [relation.source_inter_id, relation.target_inter_id] {
                    let inter = find_inter_in_sheet(sheet, inter_id)
                        .ok_or(RhythmsContractError::UnknownInter(inter_id))
                        .map_err(RhythmsStepError::Contract)?;
                    if let Some(stack_id) = inter.stack_id {
                        push_unique(stack_ids, stack_id);
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_sheet(sheet: &NeutralRhythmSheet) -> Result<(), RhythmsContractError> {
    let mut page_ids = BTreeSet::new();
    let mut system_ids = BTreeSet::new();
    let mut stack_ids = BTreeSet::new();
    let mut measure_ids = BTreeSet::new();
    for page in &sheet.pages {
        if !page_ids.insert(page.id) {
            return Err(RhythmsContractError::DuplicatePage(page.id));
        }
        if page
            .last_time_rational
            .is_some_and(|value| !valid_rational(value))
        {
            return Err(RhythmsContractError::InvalidRational(
                page.last_time_rational.unwrap(),
            ));
        }
        for system in &page.systems {
            if !system_ids.insert(system.id) {
                return Err(RhythmsContractError::DuplicateSystem(system.id));
            }
            for stack in &system.stacks {
                if !stack_ids.insert(stack.id) {
                    return Err(RhythmsContractError::DuplicateStack(stack.id));
                }
                for value in [stack.expected_duration, stack.actual_duration]
                    .into_iter()
                    .flatten()
                {
                    if !valid_rational(value) {
                        return Err(RhythmsContractError::InvalidRational(value));
                    }
                }
                for measure in &stack.measures {
                    if !measure_ids.insert(measure.id) {
                        return Err(RhythmsContractError::DuplicateMeasure(measure.id));
                    }
                }
            }
        }
    }
    Ok(())
}

fn apply_delta(
    sheet: &mut NeutralRhythmSheet,
    page_index: usize,
    delta: RhythmsDelta,
) -> Result<(), RhythmsContractError> {
    for mutation in delta.mutations {
        apply_mutation(sheet, page_index, mutation)?;
    }
    Ok(())
}

fn apply_mutation(
    sheet: &mut NeutralRhythmSheet,
    page_index: usize,
    mutation: RhythmsDeltaMutation,
) -> Result<(), RhythmsContractError> {
    let page_id = sheet.pages[page_index].id;
    match mutation {
        RhythmsDeltaMutation::SetPageLastTime { value } => {
            if value.is_some_and(|value| !valid_rational(value)) {
                return Err(RhythmsContractError::InvalidRational(value.unwrap()));
            }
            sheet.pages[page_index].last_time_rational = value;
            sheet
                .mutations
                .push(RhythmsMutation::PageLastTimeSet { page_id });
        }
        RhythmsDeltaMutation::AttachTimeSignature { stack_id, inter_id } => {
            let inter = find_inter(&sheet.pages[page_index], inter_id)
                .ok_or(RhythmsContractError::UnknownInter(inter_id))?;
            if inter.class != NeutralRhythmInterClass::Time {
                return Err(RhythmsContractError::WrongInterClass(inter_id));
            }
            if inter.stack_id != Some(stack_id) {
                return Err(RhythmsContractError::InterStackMismatch { inter_id, stack_id });
            }
            let stack = find_stack_mut(&mut sheet.pages[page_index], stack_id)
                .ok_or(RhythmsContractError::UnknownStack(stack_id))?;
            if stack.time_signature_inter_ids.contains(&inter_id) {
                return Err(RhythmsContractError::DuplicateStackInter { stack_id, inter_id });
            }
            stack.time_signature_inter_ids.push(inter_id);
            sheet
                .mutations
                .push(RhythmsMutation::TimeSignatureAttached { stack_id, inter_id });
        }
        RhythmsDeltaMutation::AttachFrat {
            stack_id,
            measure_id,
            inter_id,
        } => {
            let inter = find_inter(&sheet.pages[page_index], inter_id)
                .ok_or(RhythmsContractError::UnknownInter(inter_id))?;
            if !inter.class.is_frat() {
                return Err(RhythmsContractError::WrongInterClass(inter_id));
            }
            if inter.stack_id != Some(stack_id) {
                return Err(RhythmsContractError::InterStackMismatch { inter_id, stack_id });
            }
            if inter.measure_id != Some(measure_id) {
                return Err(RhythmsContractError::InterMeasureMismatch {
                    inter_id,
                    measure_id,
                });
            }
            let stack = find_stack_mut(&mut sheet.pages[page_index], stack_id)
                .ok_or(RhythmsContractError::UnknownStack(stack_id))?;
            let measure = stack
                .measures
                .iter_mut()
                .find(|measure| measure.id == measure_id)
                .ok_or(RhythmsContractError::UnknownMeasure(measure_id))?;
            if stack.frat_inter_ids.contains(&inter_id) {
                return Err(RhythmsContractError::DuplicateStackInter { stack_id, inter_id });
            }
            if measure.frat_inter_ids.contains(&inter_id) {
                return Err(RhythmsContractError::DuplicateMeasureInter {
                    measure_id,
                    inter_id,
                });
            }
            stack.frat_inter_ids.push(inter_id);
            measure.frat_inter_ids.push(inter_id);
            sheet.mutations.push(RhythmsMutation::FratAttached {
                stack_id,
                measure_id,
                inter_id,
            });
        }
        RhythmsDeltaMutation::ResetStack { stack_id } => {
            let stack = find_stack_mut(&mut sheet.pages[page_index], stack_id)
                .ok_or(RhythmsContractError::UnknownStack(stack_id))?;
            stack.abnormal = false;
            stack.actual_duration = None;
            stack.slots.clear();
            sheet
                .mutations
                .push(RhythmsMutation::StackReset { stack_id });
        }
        RhythmsDeltaMutation::SetStackExpectedDuration { stack_id, value } => {
            set_stack_duration(sheet, page_index, stack_id, value, true)?;
        }
        RhythmsDeltaMutation::SetStackActualDuration { stack_id, value } => {
            set_stack_duration(sheet, page_index, stack_id, value, false)?;
        }
        RhythmsDeltaMutation::SetStackAbnormal { stack_id, abnormal } => {
            let stack = find_stack_mut(&mut sheet.pages[page_index], stack_id)
                .ok_or(RhythmsContractError::UnknownStack(stack_id))?;
            stack.abnormal = abnormal;
            sheet
                .mutations
                .push(RhythmsMutation::StackAbnormalSet { stack_id, abnormal });
        }
        RhythmsDeltaMutation::ResetMeasure { measure_id } => {
            let measure = find_measure_mut(&mut sheet.pages[page_index], measure_id)
                .ok_or(RhythmsContractError::UnknownMeasure(measure_id))?;
            measure.abnormal = false;
            measure.voices.clear();
            sheet
                .mutations
                .push(RhythmsMutation::MeasureReset { measure_id });
        }
        RhythmsDeltaMutation::SetMeasureAbnormal {
            measure_id,
            abnormal,
        } => {
            let measure = find_measure_mut(&mut sheet.pages[page_index], measure_id)
                .ok_or(RhythmsContractError::UnknownMeasure(measure_id))?;
            measure.abnormal = abnormal;
            sheet.mutations.push(RhythmsMutation::MeasureAbnormalSet {
                measure_id,
                abnormal,
            });
        }
        RhythmsDeltaMutation::AddVoice { measure_id, voice } => {
            let measure = find_measure_mut(&mut sheet.pages[page_index], measure_id)
                .ok_or(RhythmsContractError::UnknownMeasure(measure_id))?;
            if measure
                .voices
                .iter()
                .any(|existing| existing.id == voice.id)
            {
                return Err(RhythmsContractError::DuplicateVoice {
                    measure_id,
                    voice_id: voice.id,
                });
            }
            for chord_id in &voice.chord_ids {
                if !measure.chord_ids.contains(chord_id) {
                    return Err(RhythmsContractError::UnknownChord {
                        measure_id,
                        chord_id: *chord_id,
                    });
                }
            }
            let voice_id = voice.id;
            measure.voices.push(voice);
            sheet.mutations.push(RhythmsMutation::VoiceAdded {
                measure_id,
                voice_id,
            });
        }
        RhythmsDeltaMutation::AddSlot { stack_id, slot } => {
            if !valid_rational(slot.time_offset) {
                return Err(RhythmsContractError::InvalidRational(slot.time_offset));
            }
            let stack = find_stack_mut(&mut sheet.pages[page_index], stack_id)
                .ok_or(RhythmsContractError::UnknownStack(stack_id))?;
            if stack.slots.iter().any(|existing| existing.id == slot.id) {
                return Err(RhythmsContractError::DuplicateSlot {
                    stack_id,
                    slot_id: slot.id,
                });
            }
            for chord_id in &slot.chord_ids {
                if !stack
                    .measures
                    .iter()
                    .any(|measure| measure.chord_ids.contains(chord_id))
                {
                    return Err(RhythmsContractError::UnknownChord {
                        measure_id: 0,
                        chord_id: *chord_id,
                    });
                }
            }
            let slot_id = slot.id;
            stack.slots.push(slot);
            sheet
                .mutations
                .push(RhythmsMutation::SlotAdded { stack_id, slot_id });
        }
        RhythmsDeltaMutation::PutVoiceSlot {
            measure_id,
            voice_id,
            info,
        } => {
            let stack_id = sheet.pages[page_index]
                .systems
                .iter()
                .flat_map(|system| &system.stacks)
                .find(|stack| {
                    stack
                        .measures
                        .iter()
                        .any(|measure| measure.id == measure_id)
                })
                .map(|stack| stack.id)
                .ok_or(RhythmsContractError::UnknownMeasure(measure_id))?;
            let slot_exists = sheet.pages[page_index]
                .systems
                .iter()
                .flat_map(|system| &system.stacks)
                .find(|stack| stack.id == stack_id)
                .is_some_and(|stack| stack.slots.iter().any(|slot| slot.id == info.slot_id));
            if !slot_exists {
                return Err(RhythmsContractError::UnknownSlot {
                    stack_id,
                    slot_id: info.slot_id,
                });
            }
            let (_stack_id, measure) =
                find_stack_and_measure_mut(&mut sheet.pages[page_index], measure_id)
                    .ok_or(RhythmsContractError::UnknownMeasure(measure_id))?;
            let slot_exists = measure.chord_ids.contains(&info.chord_id);
            if !slot_exists {
                return Err(RhythmsContractError::UnknownChord {
                    measure_id,
                    chord_id: info.chord_id,
                });
            }
            let voice = measure
                .voices
                .iter_mut()
                .find(|voice| voice.id == voice_id)
                .ok_or(RhythmsContractError::UnknownVoice {
                    measure_id,
                    voice_id,
                })?;
            if voice
                .slot_infos
                .iter()
                .any(|entry| entry.slot_id == info.slot_id)
            {
                return Err(RhythmsContractError::DuplicateVoiceSlot {
                    voice_id,
                    slot_id: info.slot_id,
                });
            }
            voice.slot_infos.push(info);
            sheet.mutations.push(RhythmsMutation::VoiceSlotPut {
                measure_id,
                voice_id,
                slot_id: info.slot_id,
            });
        }
        RhythmsDeltaMutation::ReassignVoiceId {
            measure_id,
            old_id,
            new_id,
        } => {
            let measure = find_measure_mut(&mut sheet.pages[page_index], measure_id)
                .ok_or(RhythmsContractError::UnknownMeasure(measure_id))?;
            if measure.voices.iter().any(|voice| voice.id == new_id) {
                return Err(RhythmsContractError::DuplicateVoice {
                    measure_id,
                    voice_id: new_id,
                });
            }
            let voice = measure
                .voices
                .iter_mut()
                .find(|voice| voice.id == old_id)
                .ok_or(RhythmsContractError::UnknownVoice {
                    measure_id,
                    voice_id: old_id,
                })?;
            voice.id = new_id;
            sheet.mutations.push(RhythmsMutation::VoiceIdReassigned {
                measure_id,
                old_id,
                new_id,
            });
        }
    }
    Ok(())
}

fn set_stack_duration(
    sheet: &mut NeutralRhythmSheet,
    page_index: usize,
    stack_id: usize,
    value: Option<RhythmRational>,
    expected: bool,
) -> Result<(), RhythmsContractError> {
    if value.is_some_and(|value| !valid_rational(value)) {
        return Err(RhythmsContractError::InvalidRational(value.unwrap()));
    }
    let stack = find_stack_mut(&mut sheet.pages[page_index], stack_id)
        .ok_or(RhythmsContractError::UnknownStack(stack_id))?;
    if expected {
        stack.expected_duration = value;
        sheet
            .mutations
            .push(RhythmsMutation::StackExpectedDurationSet { stack_id });
    } else {
        stack.actual_duration = value;
        sheet
            .mutations
            .push(RhythmsMutation::StackActualDurationSet { stack_id });
    }
    Ok(())
}

fn find_inter(page: &NeutralRhythmPage, inter_id: usize) -> Option<&NeutralRhythmInter> {
    page.systems
        .iter()
        .flat_map(|system| &system.inters)
        .find(|inter| inter.id == inter_id)
}

fn find_inter_in_sheet(sheet: &NeutralRhythmSheet, inter_id: usize) -> Option<&NeutralRhythmInter> {
    sheet
        .pages
        .iter()
        .find_map(|page| find_inter(page, inter_id))
}

fn locate_inter(
    sheet: &NeutralRhythmSheet,
    inter_id: usize,
) -> Option<(usize, usize, &NeutralRhythmInter)> {
    for (page_index, page) in sheet.pages.iter().enumerate() {
        for (system_index, system) in page.systems.iter().enumerate() {
            if let Some(inter) = system.inters.iter().find(|inter| inter.id == inter_id) {
                return Some((page_index, system_index, inter));
            }
        }
    }
    None
}

fn locate_relation(
    sheet: &NeutralRhythmSheet,
    relation_id: usize,
) -> Option<(usize, usize, &NeutralRhythmRelation)> {
    for (page_index, page) in sheet.pages.iter().enumerate() {
        for (system_index, system) in page.systems.iter().enumerate() {
            if let Some(relation) = system
                .relations
                .iter()
                .find(|relation| relation.id == relation_id)
            {
                return Some((page_index, system_index, relation));
            }
        }
    }
    None
}

fn find_stack_mut(
    page: &mut NeutralRhythmPage,
    stack_id: usize,
) -> Option<&mut NeutralRhythmStack> {
    page.systems
        .iter_mut()
        .flat_map(|system| &mut system.stacks)
        .find(|stack| stack.id == stack_id)
}

fn find_measure_mut(
    page: &mut NeutralRhythmPage,
    measure_id: usize,
) -> Option<&mut NeutralRhythmMeasure> {
    page.systems
        .iter_mut()
        .flat_map(|system| &mut system.stacks)
        .flat_map(|stack| &mut stack.measures)
        .find(|measure| measure.id == measure_id)
}

fn find_stack_and_measure_mut(
    page: &mut NeutralRhythmPage,
    measure_id: usize,
) -> Option<(usize, &mut NeutralRhythmMeasure)> {
    for system in &mut page.systems {
        for stack in &mut system.stacks {
            if let Some(index) = stack
                .measures
                .iter()
                .position(|measure| measure.id == measure_id)
            {
                return Some((stack.id, &mut stack.measures[index]));
            }
        }
    }
    None
}

fn locate_stack(sheet: &NeutralRhythmSheet, stack_id: usize) -> Option<(usize, usize, usize)> {
    for (page_index, page) in sheet.pages.iter().enumerate() {
        for (system_index, system) in page.systems.iter().enumerate() {
            if let Some(stack_index) = system.stacks.iter().position(|stack| stack.id == stack_id) {
                return Some((page_index, system_index, stack_index));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct FakeSemantic {
        pages: BTreeMap<usize, RhythmsStageOutcome<&'static str>>,
        stacks: BTreeMap<usize, RhythmsStageOutcome<&'static str>>,
        calls: Vec<(&'static str, usize)>,
    }

    impl SemanticRhythmAnalyzer for FakeSemantic {
        type Error = &'static str;

        fn process_page(&mut self, input: RhythmPageInput<'_>) -> RhythmsStageOutcome<Self::Error> {
            self.calls.push(("page", input.page.id));
            self.pages
                .remove(&input.page.id)
                .unwrap_or_else(|| RhythmsStageOutcome::success(RhythmsDelta::default()))
        }

        fn reprocess_stack(
            &mut self,
            input: RhythmStackInput<'_>,
        ) -> RhythmsStageOutcome<Self::Error> {
            self.calls.push(("stack", input.stack.id));
            self.stacks
                .remove(&input.stack.id)
                .unwrap_or_else(|| RhythmsStageOutcome::success(RhythmsDelta::default()))
        }
    }

    fn rational(numerator: i64, denominator: i64) -> RhythmRational {
        RhythmRational {
            numerator,
            denominator,
        }
    }

    fn stack(id: usize, measure_id: usize) -> NeutralRhythmStack {
        NeutralRhythmStack {
            id,
            abnormal: true,
            expected_duration: None,
            actual_duration: Some(rational(1, 1)),
            time_signature_inter_ids: Vec::new(),
            frat_inter_ids: Vec::new(),
            measures: vec![NeutralRhythmMeasure {
                id: measure_id,
                abnormal: true,
                frat_inter_ids: Vec::new(),
                chord_ids: vec![1000 + measure_id],
                voices: Vec::new(),
            }],
            slots: vec![NeutralRhythmSlot {
                id: 9,
                time_offset: rational(0, 1),
                chord_ids: vec![1000 + measure_id],
            }],
        }
    }

    fn sheet() -> NeutralRhythmSheet {
        NeutralRhythmSheet {
            id: 1,
            pages: vec![
                NeutralRhythmPage {
                    id: 10,
                    systems: vec![NeutralRhythmSystem {
                        id: 20,
                        inters: vec![
                            NeutralRhythmInter {
                                id: 1,
                                class: NeutralRhythmInterClass::Time,
                                stack_id: Some(30),
                                measure_id: Some(40),
                                abscissa: 5,
                            },
                            NeutralRhythmInter {
                                id: 2,
                                class: NeutralRhythmInterClass::Flag,
                                stack_id: Some(30),
                                measure_id: Some(40),
                                abscissa: 8,
                            },
                        ],
                        relations: Vec::new(),
                        stacks: vec![stack(30, 40)],
                    }],
                    last_time_rational: None,
                },
                NeutralRhythmPage {
                    id: 11,
                    systems: vec![NeutralRhythmSystem {
                        id: 21,
                        inters: Vec::new(),
                        relations: Vec::new(),
                        stacks: vec![stack(31, 41)],
                    }],
                    last_time_rational: None,
                },
            ],
            mutations: Vec::new(),
        }
    }

    #[test]
    fn pages_run_sequentially_and_checked_failure_keeps_prefix_then_continues() {
        let mut semantic = FakeSemantic::default();
        semantic.pages.insert(
            10,
            RhythmsStageOutcome {
                delta: RhythmsDelta {
                    mutations: vec![RhythmsDeltaMutation::ResetStack { stack_id: 30 }],
                },
                error: Some("stack analysis failed"),
            },
        );
        semantic.pages.insert(
            11,
            RhythmsStageOutcome::success(RhythmsDelta {
                mutations: vec![RhythmsDeltaMutation::SetStackExpectedDuration {
                    stack_id: 31,
                    value: Some(rational(3, 4)),
                }],
            }),
        );
        let mut step = HeadlessRhythmsStep::new(semantic);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(step.semantic().calls, vec![("page", 10), ("page", 11)]);
        assert_eq!(report.failures.len(), 1);
        assert!(sheet.pages[0].systems[0].stacks[0].slots.is_empty());
        assert_eq!(
            sheet.pages[1].systems[0].stacks[0].expected_duration,
            Some(rational(3, 4))
        );
        assert_eq!(
            sheet.mutations,
            vec![
                RhythmsMutation::StackReset { stack_id: 30 },
                RhythmsMutation::Failed {
                    page_id: 10,
                    stack_id: None,
                    phase: RhythmPhase::Page,
                },
                RhythmsMutation::StackExpectedDurationSet { stack_id: 31 },
            ]
        );
    }

    #[test]
    fn task_impacts_collect_before_mutation_then_run_pages_and_stacks_in_java_order() {
        let mut sheet = sheet();
        let system = &mut sheet.pages[0].systems[0];
        system.stacks.push(stack(32, 42));
        system.inters.extend([
            NeutralRhythmInter {
                id: 3,
                class: NeutralRhythmInterClass::StaffBarline,
                stack_id: Some(30),
                measure_id: Some(40),
                abscissa: 10,
            },
            NeutralRhythmInter {
                id: 4,
                class: NeutralRhythmInterClass::Head,
                stack_id: Some(32),
                measure_id: Some(42),
                abscissa: 12,
            },
        ]);
        system.relations.push(NeutralRhythmRelation {
            id: 50,
            class: NeutralRhythmRelationClass::SameTime,
            source_inter_id: 2,
            target_inter_id: 4,
        });
        let tasks = [
            RhythmImpactTask::Relation { relation_id: 50 },
            RhythmImpactTask::Inter {
                inter_id: 3,
                edit: RhythmEditKind::Addition,
            },
            RhythmImpactTask::Page { page_id: 10 },
            RhythmImpactTask::Stack { stack_id: 30 },
        ];
        let mut step = HeadlessRhythmsStep::new(FakeSemantic::default());

        step.impact_tasks(&mut sheet, &tasks, RhythmOperation::Do)
            .unwrap();

        assert_eq!(
            step.semantic().calls,
            vec![("page", 10), ("stack", 30), ("stack", 32)]
        );
    }

    #[test]
    fn mutation_contract_tracks_time_frat_voice_slot_and_refinement_order() {
        let mut semantic = FakeSemantic::default();
        semantic.pages.insert(
            10,
            RhythmsStageOutcome::success(RhythmsDelta {
                mutations: vec![
                    RhythmsDeltaMutation::AttachTimeSignature {
                        stack_id: 30,
                        inter_id: 1,
                    },
                    RhythmsDeltaMutation::AttachFrat {
                        stack_id: 30,
                        measure_id: 40,
                        inter_id: 2,
                    },
                    RhythmsDeltaMutation::ResetMeasure { measure_id: 40 },
                    RhythmsDeltaMutation::AddVoice {
                        measure_id: 40,
                        voice: NeutralRhythmVoice {
                            id: 1,
                            measure_rest: false,
                            chord_ids: vec![1040],
                            slot_infos: Vec::new(),
                        },
                    },
                    RhythmsDeltaMutation::ResetStack { stack_id: 30 },
                    RhythmsDeltaMutation::AddSlot {
                        stack_id: 30,
                        slot: NeutralRhythmSlot {
                            id: 1,
                            time_offset: rational(0, 1),
                            chord_ids: vec![1040],
                        },
                    },
                    RhythmsDeltaMutation::PutVoiceSlot {
                        measure_id: 40,
                        voice_id: 1,
                        info: NeutralVoiceSlotInfo {
                            slot_id: 1,
                            chord_id: 1040,
                            status: NeutralSlotChordStatus::Begin,
                        },
                    },
                    RhythmsDeltaMutation::ReassignVoiceId {
                        measure_id: 40,
                        old_id: 1,
                        new_id: 2,
                    },
                ],
            }),
        );
        let mut step = HeadlessRhythmsStep::new(semantic);
        let mut sheet = sheet();

        step.process(&mut sheet).unwrap();

        let stack = &sheet.pages[0].systems[0].stacks[0];
        assert_eq!(stack.time_signature_inter_ids, vec![1]);
        assert_eq!(stack.frat_inter_ids, vec![2]);
        assert_eq!(stack.slots[0].id, 1);
        assert_eq!(stack.measures[0].voices[0].id, 2);
        assert_eq!(stack.measures[0].voices[0].slot_infos[0].slot_id, 1);
    }

    #[test]
    fn contract_failure_retains_prior_mutations() {
        let mut semantic = FakeSemantic::default();
        semantic.pages.insert(
            10,
            RhythmsStageOutcome::success(RhythmsDelta {
                mutations: vec![
                    RhythmsDeltaMutation::ResetStack { stack_id: 30 },
                    RhythmsDeltaMutation::AttachFrat {
                        stack_id: 30,
                        measure_id: 40,
                        inter_id: 1,
                    },
                ],
            }),
        );
        let mut step = HeadlessRhythmsStep::new(semantic);
        let mut sheet = sheet();

        assert_eq!(
            step.process(&mut sheet),
            Err(RhythmsStepError::Contract(
                RhythmsContractError::WrongInterClass(1)
            ))
        );
        assert!(sheet.pages[0].systems[0].stacks[0].slots.is_empty());
        assert_eq!(
            sheet.mutations,
            vec![RhythmsMutation::StackReset { stack_id: 30 }]
        );
    }

    #[test]
    fn white_classes_do_not_impact_and_stack_page_sets_match_java() {
        assert!(NeutralRhythmInterClass::GraceChord.is_white());
        assert!(!NeutralRhythmInterClass::GraceChord.impacts_stack());
        assert!(NeutralRhythmInterClass::Barline.impacts_stack());
        assert!(NeutralRhythmInterClass::Time.impacts_page());
        assert!(!NeutralRhythmInterClass::Other.impacts_page());
    }
}
