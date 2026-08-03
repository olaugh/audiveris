// SPDX-License-Identifier: AGPL-3.0-or-later

//! Headless lifecycle port of Java `PageStep`.
//!
//! Semantic reduction remains injected. This module owns exact sheet/page phase
//! order, hierarchy backlinks, part/measure/slur/lyric/voice mutations, failure
//! prefixes, output ownership, and collaborator cleanup.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PagePhase {
    ReduceSheet,
    ReduceScoreParts,
    FixMeasures,
    ConnectOrphanSlurs,
    CheckCrossTies,
    RefineLyrics,
    RefineVoices,
}

impl PagePhase {
    const PAGE_ORDER: [Self; 6] = [
        Self::ReduceScoreParts,
        Self::FixMeasures,
        Self::ConnectOrphanSlurs,
        Self::CheckCrossTies,
        Self::RefineLyrics,
        Self::RefineVoices,
    ];
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralPageScore {
    pub id: usize,
    pub page_ids: Vec<usize>,
    pub logical_part_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralPage {
    pub id: usize,
    pub score_id: usize,
    pub system_ids: Vec<usize>,
    pub measure_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralPageSystem {
    pub id: usize,
    pub page_id: usize,
    pub part_ids: Vec<usize>,
    pub measure_ids: Vec<usize>,
    pub lyric_line_ids: Vec<usize>,
    pub voice_ids: Vec<usize>,
    pub item_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralLogicalPart {
    pub id: usize,
    pub score_id: usize,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralPhysicalPart {
    pub id: usize,
    pub system_id: usize,
    pub logical_part_id: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralPageMeasure {
    pub id: usize,
    pub system_id: usize,
    pub part_id: usize,
    pub number: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralLyricLine {
    pub id: usize,
    pub system_id: usize,
    pub refined: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralPageVoice {
    pub id: usize,
    pub system_id: usize,
    pub refined_id: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralSlurConnection {
    pub id: usize,
    pub previous_slur_id: usize,
    pub next_slur_id: usize,
    pub previous_system_id: usize,
    pub next_system_id: usize,
    pub tie: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralPageSheet {
    pub id: usize,
    /// Exact `sheet.getPages()` traversal order.
    pub page_ids: Vec<usize>,
    pub valid_selected_stub_ids: Vec<usize>,
    pub scores: Vec<NeutralPageScore>,
    pub pages: Vec<NeutralPage>,
    pub systems: Vec<NeutralPageSystem>,
    pub logical_parts: Vec<NeutralLogicalPart>,
    pub physical_parts: Vec<NeutralPhysicalPart>,
    pub measures: Vec<NeutralPageMeasure>,
    pub lyric_lines: Vec<NeutralLyricLine>,
    pub voices: Vec<NeutralPageVoice>,
    pub slur_connections: Vec<NeutralSlurConnection>,
    pub next_logical_part_id: usize,
    pub retired_logical_part_ids: Vec<usize>,
    pub next_slur_connection_id: usize,
    pub retired_slur_connection_ids: Vec<usize>,
    pub mutations: Vec<PageMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageMutation {
    SystemItemRemoved {
        system_id: usize,
        item_id: usize,
    },
    LogicalPartAdded {
        score_id: usize,
        part_id: usize,
    },
    LogicalPartRemoved {
        score_id: usize,
        part_id: usize,
    },
    PhysicalPartAssigned {
        part_id: usize,
        logical_part_id: Option<usize>,
    },
    MeasureRenumbered {
        measure_id: usize,
        number: i32,
    },
    PageMeasureCountSet {
        page_id: usize,
        count: usize,
    },
    SlursConnected {
        connection_id: usize,
    },
    SlursDisconnected {
        connection_id: usize,
    },
    CrossTieSet {
        connection_id: usize,
        tie: bool,
    },
    LyricRefined {
        line_id: usize,
    },
    VoiceRefined {
        voice_id: usize,
        refined_id: usize,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageDelta {
    pub mutations: Vec<PageDeltaMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageDeltaMutation {
    RemoveSystemItem {
        system_id: usize,
        item_id: usize,
    },
    AddLogicalPart(NeutralLogicalPart),
    RemoveLogicalPart(usize),
    AssignPhysicalPart {
        part_id: usize,
        logical_part_id: Option<usize>,
    },
    RenumberMeasure {
        measure_id: usize,
        number: i32,
    },
    SetPageMeasureCount(usize),
    ConnectSlurs(NeutralSlurConnection),
    DisconnectSlurs(usize),
    SetCrossTie {
        connection_id: usize,
        tie: bool,
    },
    RefineLyric(usize),
    RefineVoice {
        voice_id: usize,
        refined_id: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageFailure<E> {
    Checked(E),
    Fatal(E),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PagePhaseOutcome<E> {
    pub delta: PageDelta,
    pub failure: Option<PageFailure<E>>,
}

impl<E> PagePhaseOutcome<E> {
    #[must_use]
    pub fn success(delta: PageDelta) -> Self {
        Self {
            delta,
            failure: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageContext {
    pub sheet_reduced: bool,
    pub completed_page_ids: Vec<usize>,
    pub completed_page_phases: Vec<(usize, PagePhase)>,
}

#[derive(Clone, Copy, Debug)]
pub struct PagePhaseInput<'a> {
    pub sheet: &'a NeutralPageSheet,
    pub phase: PagePhase,
    pub page_id: Option<usize>,
    pub score_id: Option<usize>,
    pub selected_stub_ids: &'a [usize],
    pub context: &'a PageContext,
}

/// First semantic dependency: Java `SheetReduction.process()`.
pub trait SemanticPageProcessor {
    type Error;
    fn run_phase(&mut self, input: PagePhaseInput<'_>) -> PagePhaseOutcome<Self::Error>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageReport {
    pub processed_page_ids: Vec<usize>,
    pub score_ids: Vec<usize>,
    pub logical_part_ids: Vec<usize>,
    pub measure_ids: Vec<usize>,
    pub semantic_collaborators_released: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageStepError<E> {
    CheckedPhase {
        phase: PagePhase,
        page_id: Option<usize>,
        source: E,
        context: PageContext,
    },
    FatalPhase {
        phase: PagePhase,
        page_id: Option<usize>,
        source: E,
        context: PageContext,
    },
    Contract(PageContractError),
}

impl<E: fmt::Display> fmt::Display for PageStepError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CheckedPhase {
                phase,
                page_id,
                source,
                ..
            } => {
                write!(f, "page step {phase:?} for {page_id:?} failed: {source}")
            }
            Self::FatalPhase {
                phase,
                page_id,
                source,
                ..
            } => {
                write!(f, "page step {phase:?} for {page_id:?} crashed: {source}")
            }
            Self::Contract(source) => write!(f, "page contract failed: {source}"),
        }
    }
}

impl<E: Error + 'static> Error for PageStepError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CheckedPhase { source, .. } | Self::FatalPhase { source, .. } => Some(source),
            Self::Contract(source) => Some(source),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageContractError {
    DuplicateId {
        kind: &'static str,
        id: usize,
    },
    UnknownId {
        kind: &'static str,
        id: usize,
    },
    BacklinkMismatch {
        kind: &'static str,
        id: usize,
    },
    InvalidAllocator {
        kind: &'static str,
        expected: usize,
        actual: usize,
    },
    InvalidRetiredId {
        kind: &'static str,
        id: usize,
    },
    InvalidPhaseMutation(PagePhase),
    InvalidPageOrder,
    InvalidSlurTraversal(usize),
}

impl fmt::Display for PageContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl Error for PageContractError {}

pub struct HeadlessPageStep<Semantic> {
    semantic: Semantic,
}

impl<Semantic> HeadlessPageStep<Semantic> {
    #[must_use]
    pub const fn new(semantic: Semantic) -> Self {
        Self { semantic }
    }
    #[must_use]
    pub const fn semantic(&self) -> &Semantic {
        &self.semantic
    }
}

impl<Semantic: SemanticPageProcessor> HeadlessPageStep<Semantic> {
    pub fn process(
        &mut self,
        sheet: &mut NeutralPageSheet,
    ) -> Result<PageReport, PageStepError<Semantic::Error>> {
        validate_sheet(sheet).map_err(PageStepError::Contract)?;
        let mut context = PageContext::default();
        self.run_one(sheet, PagePhase::ReduceSheet, None, None, &mut context)?;
        context.sheet_reduced = true;
        for index in 0..sheet.page_ids.len() {
            let page_id = sheet.page_ids[index];
            let score_id = sheet
                .pages
                .iter()
                .find(|page| page.id == page_id)
                .expect("validated page remains live")
                .score_id;
            for phase in PagePhase::PAGE_ORDER {
                self.run_one(sheet, phase, Some(page_id), Some(score_id), &mut context)?;
                context.completed_page_phases.push((page_id, phase));
            }
            context.completed_page_ids.push(page_id);
        }
        Ok(PageReport {
            processed_page_ids: context.completed_page_ids,
            score_ids: sheet.scores.iter().map(|score| score.id).collect(),
            logical_part_ids: sheet.logical_parts.iter().map(|part| part.id).collect(),
            measure_ids: sheet.measures.iter().map(|measure| measure.id).collect(),
            semantic_collaborators_released: true,
        })
    }

    fn run_one(
        &mut self,
        sheet: &mut NeutralPageSheet,
        phase: PagePhase,
        page_id: Option<usize>,
        score_id: Option<usize>,
        context: &mut PageContext,
    ) -> Result<(), PageStepError<Semantic::Error>> {
        let outcome = self.semantic.run_phase(PagePhaseInput {
            sheet,
            phase,
            page_id,
            score_id,
            selected_stub_ids: &sheet.valid_selected_stub_ids,
            context,
        });
        apply_delta(sheet, phase, page_id, outcome.delta).map_err(PageStepError::Contract)?;
        match outcome.failure {
            None => Ok(()),
            Some(PageFailure::Checked(source)) => Err(PageStepError::CheckedPhase {
                phase,
                page_id,
                source,
                context: context.clone(),
            }),
            Some(PageFailure::Fatal(source)) => Err(PageStepError::FatalPhase {
                phase,
                page_id,
                source,
                context: context.clone(),
            }),
        }
    }
}

fn apply_delta(
    sheet: &mut NeutralPageSheet,
    phase: PagePhase,
    page_id: Option<usize>,
    delta: PageDelta,
) -> Result<(), PageContractError> {
    for mutation in delta.mutations {
        match mutation {
            PageDeltaMutation::RemoveSystemItem { system_id, item_id } => {
                require_phase(phase, PagePhase::ReduceSheet)?;
                let system = system_mut(sheet, system_id)?;
                let index = system.item_ids.iter().position(|id| *id == item_id).ok_or(
                    PageContractError::UnknownId {
                        kind: "system item",
                        id: item_id,
                    },
                )?;
                system.item_ids.remove(index);
                sheet
                    .mutations
                    .push(PageMutation::SystemItemRemoved { system_id, item_id });
            }
            PageDeltaMutation::AddLogicalPart(part) => {
                require_phase(phase, PagePhase::ReduceScoreParts)?;
                let expected_score = page_score(sheet, page_id)?;
                if part.score_id != expected_score {
                    return Err(PageContractError::BacklinkMismatch {
                        kind: "logical part score",
                        id: part.id,
                    });
                }
                if part.id != sheet.next_logical_part_id {
                    return Err(PageContractError::InvalidAllocator {
                        kind: "logical part",
                        expected: sheet.next_logical_part_id,
                        actual: part.id,
                    });
                }
                if sheet.logical_parts.iter().any(|old| old.id == part.id) {
                    return Err(PageContractError::DuplicateId {
                        kind: "logical part",
                        id: part.id,
                    });
                }
                sheet.next_logical_part_id += 1;
                let score = sheet
                    .scores
                    .iter_mut()
                    .find(|score| score.id == part.score_id)
                    .ok_or(PageContractError::UnknownId {
                        kind: "score",
                        id: part.score_id,
                    })?;
                score.logical_part_ids.push(part.id);
                let score_id = part.score_id;
                let part_id = part.id;
                sheet.logical_parts.push(part);
                sheet
                    .mutations
                    .push(PageMutation::LogicalPartAdded { score_id, part_id });
            }
            PageDeltaMutation::RemoveLogicalPart(part_id) => {
                require_phase(phase, PagePhase::ReduceScoreParts)?;
                let index = sheet
                    .logical_parts
                    .iter()
                    .position(|part| part.id == part_id)
                    .ok_or(PageContractError::UnknownId {
                        kind: "logical part",
                        id: part_id,
                    })?;
                let score_id = sheet.logical_parts[index].score_id;
                sheet.logical_parts.remove(index);
                sheet
                    .scores
                    .iter_mut()
                    .find(|score| score.id == score_id)
                    .expect("logical part score remains live")
                    .logical_part_ids
                    .retain(|id| *id != part_id);
                for part in &mut sheet.physical_parts {
                    if part.logical_part_id == Some(part_id) {
                        part.logical_part_id = None;
                    }
                }
                sheet.retired_logical_part_ids.push(part_id);
                sheet
                    .mutations
                    .push(PageMutation::LogicalPartRemoved { score_id, part_id });
            }
            PageDeltaMutation::AssignPhysicalPart {
                part_id,
                logical_part_id,
            } => {
                require_phase(phase, PagePhase::ReduceScoreParts)?;
                if let Some(id) = logical_part_id
                    && !sheet.logical_parts.iter().any(|part| part.id == id)
                {
                    return Err(PageContractError::UnknownId {
                        kind: "logical part",
                        id,
                    });
                }
                let part = sheet
                    .physical_parts
                    .iter_mut()
                    .find(|part| part.id == part_id)
                    .ok_or(PageContractError::UnknownId {
                        kind: "physical part",
                        id: part_id,
                    })?;
                part.logical_part_id = logical_part_id;
                sheet.mutations.push(PageMutation::PhysicalPartAssigned {
                    part_id,
                    logical_part_id,
                });
            }
            PageDeltaMutation::RenumberMeasure { measure_id, number } => {
                require_phase(phase, PagePhase::FixMeasures)?;
                let measure = sheet
                    .measures
                    .iter_mut()
                    .find(|measure| measure.id == measure_id)
                    .ok_or(PageContractError::UnknownId {
                        kind: "measure",
                        id: measure_id,
                    })?;
                measure.number = number;
                sheet
                    .mutations
                    .push(PageMutation::MeasureRenumbered { measure_id, number });
            }
            PageDeltaMutation::SetPageMeasureCount(count) => {
                require_phase(phase, PagePhase::FixMeasures)?;
                let id = page_id.ok_or(PageContractError::InvalidPhaseMutation(phase))?;
                sheet
                    .pages
                    .iter_mut()
                    .find(|page| page.id == id)
                    .expect("validated page remains live")
                    .measure_count = count;
                sheet
                    .mutations
                    .push(PageMutation::PageMeasureCountSet { page_id: id, count });
            }
            PageDeltaMutation::ConnectSlurs(connection) => {
                require_phase(phase, PagePhase::ConnectOrphanSlurs)?;
                validate_connection(sheet, page_id, &connection)?;
                if connection.id != sheet.next_slur_connection_id {
                    return Err(PageContractError::InvalidAllocator {
                        kind: "slur connection",
                        expected: sheet.next_slur_connection_id,
                        actual: connection.id,
                    });
                }
                sheet.next_slur_connection_id += 1;
                sheet.slur_connections.push(connection);
                sheet.mutations.push(PageMutation::SlursConnected {
                    connection_id: connection.id,
                });
            }
            PageDeltaMutation::DisconnectSlurs(connection_id) => {
                require_phase(phase, PagePhase::ConnectOrphanSlurs)?;
                let index = sheet
                    .slur_connections
                    .iter()
                    .position(|item| item.id == connection_id)
                    .ok_or(PageContractError::UnknownId {
                        kind: "slur connection",
                        id: connection_id,
                    })?;
                sheet.slur_connections.remove(index);
                sheet.retired_slur_connection_ids.push(connection_id);
                sheet
                    .mutations
                    .push(PageMutation::SlursDisconnected { connection_id });
            }
            PageDeltaMutation::SetCrossTie { connection_id, tie } => {
                require_phase(phase, PagePhase::CheckCrossTies)?;
                let connection = sheet
                    .slur_connections
                    .iter_mut()
                    .find(|item| item.id == connection_id)
                    .ok_or(PageContractError::UnknownId {
                        kind: "slur connection",
                        id: connection_id,
                    })?;
                connection.tie = tie;
                sheet
                    .mutations
                    .push(PageMutation::CrossTieSet { connection_id, tie });
            }
            PageDeltaMutation::RefineLyric(line_id) => {
                require_phase(phase, PagePhase::RefineLyrics)?;
                let line = sheet
                    .lyric_lines
                    .iter_mut()
                    .find(|line| line.id == line_id)
                    .ok_or(PageContractError::UnknownId {
                        kind: "lyric line",
                        id: line_id,
                    })?;
                line.refined = true;
                sheet.mutations.push(PageMutation::LyricRefined { line_id });
            }
            PageDeltaMutation::RefineVoice {
                voice_id,
                refined_id,
            } => {
                require_phase(phase, PagePhase::RefineVoices)?;
                let voice = sheet
                    .voices
                    .iter_mut()
                    .find(|voice| voice.id == voice_id)
                    .ok_or(PageContractError::UnknownId {
                        kind: "voice",
                        id: voice_id,
                    })?;
                voice.refined_id = Some(refined_id);
                sheet.mutations.push(PageMutation::VoiceRefined {
                    voice_id,
                    refined_id,
                });
            }
        }
    }
    Ok(())
}

fn require_phase(actual: PagePhase, expected: PagePhase) -> Result<(), PageContractError> {
    (actual == expected)
        .then_some(())
        .ok_or(PageContractError::InvalidPhaseMutation(actual))
}

fn page_score(
    sheet: &NeutralPageSheet,
    page_id: Option<usize>,
) -> Result<usize, PageContractError> {
    let id = page_id.ok_or(PageContractError::InvalidPhaseMutation(
        PagePhase::ReduceScoreParts,
    ))?;
    sheet
        .pages
        .iter()
        .find(|page| page.id == id)
        .map(|page| page.score_id)
        .ok_or(PageContractError::UnknownId { kind: "page", id })
}

fn system_mut(
    sheet: &mut NeutralPageSheet,
    id: usize,
) -> Result<&mut NeutralPageSystem, PageContractError> {
    sheet
        .systems
        .iter_mut()
        .find(|system| system.id == id)
        .ok_or(PageContractError::UnknownId { kind: "system", id })
}

fn validate_connection(
    sheet: &NeutralPageSheet,
    page_id: Option<usize>,
    connection: &NeutralSlurConnection,
) -> Result<(), PageContractError> {
    if sheet
        .slur_connections
        .iter()
        .any(|old| old.id == connection.id)
    {
        return Err(PageContractError::DuplicateId {
            kind: "slur connection",
            id: connection.id,
        });
    }
    let id = page_id.ok_or(PageContractError::InvalidSlurTraversal(connection.id))?;
    let systems = &sheet
        .pages
        .iter()
        .find(|page| page.id == id)
        .ok_or(PageContractError::UnknownId { kind: "page", id })?
        .system_ids;
    let next = systems
        .iter()
        .position(|system| *system == connection.next_system_id)
        .ok_or(PageContractError::InvalidSlurTraversal(connection.id))?;
    if next == 0 || systems[next - 1] != connection.previous_system_id {
        return Err(PageContractError::InvalidSlurTraversal(connection.id));
    }
    Ok(())
}

fn unique(
    ids: impl IntoIterator<Item = usize>,
    kind: &'static str,
) -> Result<BTreeSet<usize>, PageContractError> {
    let mut found = BTreeSet::new();
    for id in ids {
        if !found.insert(id) {
            return Err(PageContractError::DuplicateId { kind, id });
        }
    }
    Ok(found)
}

fn validate_sheet(sheet: &NeutralPageSheet) -> Result<(), PageContractError> {
    let score_ids = unique(sheet.scores.iter().map(|item| item.id), "score")?;
    let page_ids = unique(sheet.pages.iter().map(|item| item.id), "page")?;
    let system_ids = unique(sheet.systems.iter().map(|item| item.id), "system")?;
    let logical_ids = unique(
        sheet.logical_parts.iter().map(|item| item.id),
        "logical part",
    )?;
    let physical_ids = unique(
        sheet.physical_parts.iter().map(|item| item.id),
        "physical part",
    )?;
    let measure_ids = unique(sheet.measures.iter().map(|item| item.id), "measure")?;
    let lyric_ids = unique(sheet.lyric_lines.iter().map(|item| item.id), "lyric line")?;
    let voice_ids = unique(sheet.voices.iter().map(|item| item.id), "voice")?;
    let connection_ids = unique(
        sheet.slur_connections.iter().map(|item| item.id),
        "slur connection",
    )?;
    if unique(sheet.page_ids.iter().copied(), "page order")? != page_ids
        || sheet.page_ids.len() != sheet.pages.len()
    {
        return Err(PageContractError::InvalidPageOrder);
    }

    for page in &sheet.pages {
        if !score_ids.contains(&page.score_id) {
            return Err(PageContractError::UnknownId {
                kind: "score",
                id: page.score_id,
            });
        }
        unique(page.system_ids.iter().copied(), "page system")?;
        for &id in &page.system_ids {
            let system = sheet
                .systems
                .iter()
                .find(|system| system.id == id)
                .ok_or(PageContractError::UnknownId { kind: "system", id })?;
            if system.page_id != page.id {
                return Err(PageContractError::BacklinkMismatch {
                    kind: "system page",
                    id,
                });
            }
        }
    }
    for score in &sheet.scores {
        unique(score.page_ids.iter().copied(), "score page")?;
        unique(score.logical_part_ids.iter().copied(), "score logical part")?;
        for &id in &score.page_ids {
            let page = sheet
                .pages
                .iter()
                .find(|page| page.id == id)
                .ok_or(PageContractError::UnknownId { kind: "page", id })?;
            if page.score_id != score.id {
                return Err(PageContractError::BacklinkMismatch {
                    kind: "page score",
                    id,
                });
            }
        }
        for &id in &score.logical_part_ids {
            let part = sheet
                .logical_parts
                .iter()
                .find(|part| part.id == id)
                .ok_or(PageContractError::UnknownId {
                    kind: "logical part",
                    id,
                })?;
            if part.score_id != score.id {
                return Err(PageContractError::BacklinkMismatch {
                    kind: "logical part score",
                    id,
                });
            }
        }
    }
    for system in &sheet.systems {
        if !page_ids.contains(&system.page_id) {
            return Err(PageContractError::UnknownId {
                kind: "page",
                id: system.page_id,
            });
        }
        for (id, set, kind) in [
            (&system.part_ids, &physical_ids, "physical part"),
            (&system.measure_ids, &measure_ids, "measure"),
            (&system.lyric_line_ids, &lyric_ids, "lyric line"),
            (&system.voice_ids, &voice_ids, "voice"),
        ] {
            unique(id.iter().copied(), kind)?;
            if let Some(missing) = id.iter().find(|id| !set.contains(id)) {
                return Err(PageContractError::UnknownId { kind, id: *missing });
            }
        }
    }
    for part in &sheet.physical_parts {
        let system = sheet
            .systems
            .iter()
            .find(|system| system.id == part.system_id)
            .ok_or(PageContractError::UnknownId {
                kind: "system",
                id: part.system_id,
            })?;
        if !system.part_ids.contains(&part.id) {
            return Err(PageContractError::BacklinkMismatch {
                kind: "physical part system",
                id: part.id,
            });
        }
        if let Some(id) = part.logical_part_id
            && !logical_ids.contains(&id)
        {
            return Err(PageContractError::UnknownId {
                kind: "logical part",
                id,
            });
        }
    }
    for measure in &sheet.measures {
        let system = sheet
            .systems
            .iter()
            .find(|system| system.id == measure.system_id)
            .ok_or(PageContractError::UnknownId {
                kind: "system",
                id: measure.system_id,
            })?;
        if !system.measure_ids.contains(&measure.id) || !physical_ids.contains(&measure.part_id) {
            return Err(PageContractError::BacklinkMismatch {
                kind: "measure owner",
                id: measure.id,
            });
        }
    }
    for line in &sheet.lyric_lines {
        let system = sheet
            .systems
            .iter()
            .find(|system| system.id == line.system_id)
            .ok_or(PageContractError::UnknownId {
                kind: "system",
                id: line.system_id,
            })?;
        if !system.lyric_line_ids.contains(&line.id) {
            return Err(PageContractError::BacklinkMismatch {
                kind: "lyric system",
                id: line.id,
            });
        }
    }
    for voice in &sheet.voices {
        let system = sheet
            .systems
            .iter()
            .find(|system| system.id == voice.system_id)
            .ok_or(PageContractError::UnknownId {
                kind: "system",
                id: voice.system_id,
            })?;
        if !system.voice_ids.contains(&voice.id) {
            return Err(PageContractError::BacklinkMismatch {
                kind: "voice system",
                id: voice.id,
            });
        }
    }
    if logical_ids
        .iter()
        .any(|id| *id >= sheet.next_logical_part_id)
    {
        return Err(PageContractError::InvalidAllocator {
            kind: "logical part",
            expected: sheet.next_logical_part_id,
            actual: *logical_ids.iter().next_back().unwrap(),
        });
    }
    if connection_ids
        .iter()
        .any(|id| *id >= sheet.next_slur_connection_id)
    {
        return Err(PageContractError::InvalidAllocator {
            kind: "slur connection",
            expected: sheet.next_slur_connection_id,
            actual: *connection_ids.iter().next_back().unwrap(),
        });
    }
    for &id in &sheet.retired_logical_part_ids {
        if id >= sheet.next_logical_part_id || logical_ids.contains(&id) {
            return Err(PageContractError::InvalidRetiredId {
                kind: "logical part",
                id,
            });
        }
    }
    for &id in &sheet.retired_slur_connection_ids {
        if id >= sheet.next_slur_connection_id || connection_ids.contains(&id) {
            return Err(PageContractError::InvalidRetiredId {
                kind: "slur connection",
                id,
            });
        }
    }
    for connection in &sheet.slur_connections {
        let next_system = sheet
            .systems
            .iter()
            .find(|system| system.id == connection.next_system_id)
            .ok_or(PageContractError::UnknownId {
                kind: "system",
                id: connection.next_system_id,
            })?;
        validate_connection(sheet, Some(next_system.page_id), connection)?;
    }
    let _ = system_ids;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    type FakeCall = (
        PagePhase,
        Option<usize>,
        Option<usize>,
        Vec<usize>,
        Vec<usize>,
    );

    #[derive(Default)]
    struct FakePage {
        calls: Vec<FakeCall>,
        outcomes: BTreeMap<(PagePhase, Option<usize>), PagePhaseOutcome<&'static str>>,
    }

    impl SemanticPageProcessor for FakePage {
        type Error = &'static str;
        fn run_phase(&mut self, input: PagePhaseInput<'_>) -> PagePhaseOutcome<Self::Error> {
            self.calls.push((
                input.phase,
                input.page_id,
                input.score_id,
                input.selected_stub_ids.to_vec(),
                input.context.completed_page_ids.clone(),
            ));
            self.outcomes
                .remove(&(input.phase, input.page_id))
                .unwrap_or_else(|| PagePhaseOutcome::success(PageDelta::default()))
        }
    }

    fn sheet() -> NeutralPageSheet {
        NeutralPageSheet {
            id: 7,
            page_ids: vec![11, 12],
            valid_selected_stub_ids: vec![70, 72],
            scores: vec![NeutralPageScore {
                id: 1,
                page_ids: vec![11, 12],
                logical_part_ids: vec![1],
            }],
            pages: vec![
                NeutralPage {
                    id: 11,
                    score_id: 1,
                    system_ids: vec![21, 22],
                    measure_count: 2,
                },
                NeutralPage {
                    id: 12,
                    score_id: 1,
                    system_ids: vec![23],
                    measure_count: 1,
                },
            ],
            systems: vec![
                NeutralPageSystem {
                    id: 21,
                    page_id: 11,
                    part_ids: vec![31],
                    measure_ids: vec![41],
                    lyric_line_ids: vec![51],
                    voice_ids: vec![61],
                    item_ids: vec![800],
                },
                NeutralPageSystem {
                    id: 22,
                    page_id: 11,
                    part_ids: vec![32],
                    measure_ids: vec![42],
                    lyric_line_ids: vec![52],
                    voice_ids: vec![62],
                    item_ids: vec![900],
                },
                NeutralPageSystem {
                    id: 23,
                    page_id: 12,
                    part_ids: vec![33],
                    measure_ids: vec![43],
                    lyric_line_ids: vec![53],
                    voice_ids: vec![63],
                    item_ids: vec![],
                },
            ],
            logical_parts: vec![NeutralLogicalPart {
                id: 1,
                score_id: 1,
                name: "Piano".to_owned(),
            }],
            physical_parts: vec![
                NeutralPhysicalPart {
                    id: 31,
                    system_id: 21,
                    logical_part_id: Some(1),
                },
                NeutralPhysicalPart {
                    id: 32,
                    system_id: 22,
                    logical_part_id: Some(1),
                },
                NeutralPhysicalPart {
                    id: 33,
                    system_id: 23,
                    logical_part_id: Some(1),
                },
            ],
            measures: vec![
                NeutralPageMeasure {
                    id: 41,
                    system_id: 21,
                    part_id: 31,
                    number: 1,
                },
                NeutralPageMeasure {
                    id: 42,
                    system_id: 22,
                    part_id: 32,
                    number: 2,
                },
                NeutralPageMeasure {
                    id: 43,
                    system_id: 23,
                    part_id: 33,
                    number: 3,
                },
            ],
            lyric_lines: vec![
                NeutralLyricLine {
                    id: 51,
                    system_id: 21,
                    refined: false,
                },
                NeutralLyricLine {
                    id: 52,
                    system_id: 22,
                    refined: false,
                },
                NeutralLyricLine {
                    id: 53,
                    system_id: 23,
                    refined: false,
                },
            ],
            voices: vec![
                NeutralPageVoice {
                    id: 61,
                    system_id: 21,
                    refined_id: None,
                },
                NeutralPageVoice {
                    id: 62,
                    system_id: 22,
                    refined_id: None,
                },
                NeutralPageVoice {
                    id: 63,
                    system_id: 23,
                    refined_id: None,
                },
            ],
            slur_connections: vec![],
            next_logical_part_id: 2,
            retired_logical_part_ids: vec![],
            next_slur_connection_id: 1,
            retired_slur_connection_ids: vec![],
            mutations: vec![],
        }
    }

    #[test]
    fn exact_sheet_then_page_phase_order_and_outputs() {
        let mut fake = FakePage::default();
        fake.outcomes.insert(
            (PagePhase::ReduceSheet, None),
            PagePhaseOutcome::success(PageDelta {
                mutations: vec![PageDeltaMutation::RemoveSystemItem {
                    system_id: 22,
                    item_id: 900,
                }],
            }),
        );
        fake.outcomes.insert(
            (PagePhase::ReduceScoreParts, Some(11)),
            PagePhaseOutcome::success(PageDelta {
                mutations: vec![
                    PageDeltaMutation::AddLogicalPart(NeutralLogicalPart {
                        id: 2,
                        score_id: 1,
                        name: "Voice".to_owned(),
                    }),
                    PageDeltaMutation::AssignPhysicalPart {
                        part_id: 31,
                        logical_part_id: Some(2),
                    },
                ],
            }),
        );
        fake.outcomes.insert(
            (PagePhase::FixMeasures, Some(11)),
            PagePhaseOutcome::success(PageDelta {
                mutations: vec![
                    PageDeltaMutation::RenumberMeasure {
                        measure_id: 41,
                        number: 0,
                    },
                    PageDeltaMutation::SetPageMeasureCount(2),
                ],
            }),
        );
        fake.outcomes.insert(
            (PagePhase::ConnectOrphanSlurs, Some(11)),
            PagePhaseOutcome::success(PageDelta {
                mutations: vec![PageDeltaMutation::ConnectSlurs(NeutralSlurConnection {
                    id: 1,
                    previous_slur_id: 100,
                    next_slur_id: 101,
                    previous_system_id: 21,
                    next_system_id: 22,
                    tie: false,
                })],
            }),
        );
        fake.outcomes.insert(
            (PagePhase::CheckCrossTies, Some(11)),
            PagePhaseOutcome::success(PageDelta {
                mutations: vec![PageDeltaMutation::SetCrossTie {
                    connection_id: 1,
                    tie: true,
                }],
            }),
        );
        fake.outcomes.insert(
            (PagePhase::RefineLyrics, Some(11)),
            PagePhaseOutcome::success(PageDelta {
                mutations: vec![
                    PageDeltaMutation::RefineLyric(51),
                    PageDeltaMutation::RefineLyric(52),
                ],
            }),
        );
        fake.outcomes.insert(
            (PagePhase::RefineVoices, Some(11)),
            PagePhaseOutcome::success(PageDelta {
                mutations: vec![
                    PageDeltaMutation::RefineVoice {
                        voice_id: 61,
                        refined_id: 1,
                    },
                    PageDeltaMutation::RefineVoice {
                        voice_id: 62,
                        refined_id: 1,
                    },
                ],
            }),
        );
        let mut step = HeadlessPageStep::new(fake);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        let expected = std::iter::once((PagePhase::ReduceSheet, None))
            .chain(
                [11, 12]
                    .into_iter()
                    .flat_map(|id| PagePhase::PAGE_ORDER.map(|phase| (phase, Some(id)))),
            )
            .collect::<Vec<_>>();
        assert_eq!(
            step.semantic()
                .calls
                .iter()
                .map(|call| (call.0, call.1))
                .collect::<Vec<_>>(),
            expected
        );
        assert!(step.semantic().calls.iter().all(|call| call.3 == [70, 72]));
        assert_eq!(report.processed_page_ids, [11, 12]);
        assert_eq!(report.logical_part_ids, [1, 2]);
        assert!(report.semantic_collaborators_released);
        assert!(sheet.slur_connections[0].tie);
        assert!(sheet.lyric_lines[0].refined && sheet.lyric_lines[1].refined);
    }

    #[test]
    fn checked_failure_stops_without_abstract_system_continuation() {
        let mut fake = FakePage::default();
        fake.outcomes.insert(
            (PagePhase::FixMeasures, Some(11)),
            PagePhaseOutcome {
                delta: PageDelta {
                    mutations: vec![PageDeltaMutation::RenumberMeasure {
                        measure_id: 41,
                        number: 0,
                    }],
                },
                failure: Some(PageFailure::Checked("measure failure")),
            },
        );
        let mut step = HeadlessPageStep::new(fake);
        let mut sheet = sheet();
        let error = step.process(&mut sheet).unwrap_err();
        assert!(matches!(
            error,
            PageStepError::CheckedPhase {
                phase: PagePhase::FixMeasures,
                page_id: Some(11),
                source: "measure failure",
                ..
            }
        ));
        assert_eq!(sheet.measures[0].number, 0);
        assert_eq!(step.semantic().calls.len(), 3);
    }

    #[test]
    fn fatal_sheet_reduction_has_prefix_and_skips_pages() {
        let mut fake = FakePage::default();
        fake.outcomes.insert(
            (PagePhase::ReduceSheet, None),
            PagePhaseOutcome {
                delta: PageDelta::default(),
                failure: Some(PageFailure::Fatal("gutter crash")),
            },
        );
        let mut step = HeadlessPageStep::new(fake);
        let mut sheet = sheet();
        assert!(matches!(
            step.process(&mut sheet),
            Err(PageStepError::FatalPhase {
                phase: PagePhase::ReduceSheet,
                page_id: None,
                source: "gutter crash",
                ..
            })
        ));
        assert_eq!(step.semantic().calls.len(), 1);
    }

    #[test]
    fn logical_part_removal_clears_backlinks_and_preserves_allocator_hole() {
        let mut fake = FakePage::default();
        fake.outcomes.insert(
            (PagePhase::ReduceScoreParts, Some(11)),
            PagePhaseOutcome::success(PageDelta {
                mutations: vec![PageDeltaMutation::RemoveLogicalPart(1)],
            }),
        );
        let mut step = HeadlessPageStep::new(fake);
        let mut sheet = sheet();
        step.process(&mut sheet).unwrap();
        assert_eq!(sheet.next_logical_part_id, 2);
        assert_eq!(sheet.retired_logical_part_ids, [1]);
        assert!(
            sheet
                .physical_parts
                .iter()
                .all(|part| part.logical_part_id.is_none())
        );
    }

    #[test]
    fn slur_connection_requires_adjacent_systems_in_current_page() {
        let mut fake = FakePage::default();
        fake.outcomes.insert(
            (PagePhase::ConnectOrphanSlurs, Some(11)),
            PagePhaseOutcome::success(PageDelta {
                mutations: vec![PageDeltaMutation::ConnectSlurs(NeutralSlurConnection {
                    id: 1,
                    previous_slur_id: 1,
                    next_slur_id: 2,
                    previous_system_id: 21,
                    next_system_id: 23,
                    tie: false,
                })],
            }),
        );
        let mut step = HeadlessPageStep::new(fake);
        let mut sheet = sheet();
        assert_eq!(
            step.process(&mut sheet),
            Err(PageStepError::Contract(
                PageContractError::InvalidSlurTraversal(1)
            ))
        );
    }

    #[test]
    fn mutation_is_rejected_in_wrong_java_phase() {
        let mut fake = FakePage::default();
        fake.outcomes.insert(
            (PagePhase::RefineLyrics, Some(11)),
            PagePhaseOutcome::success(PageDelta {
                mutations: vec![PageDeltaMutation::RefineVoice {
                    voice_id: 61,
                    refined_id: 1,
                }],
            }),
        );
        let mut step = HeadlessPageStep::new(fake);
        let mut sheet = sheet();
        assert_eq!(
            step.process(&mut sheet),
            Err(PageStepError::Contract(
                PageContractError::InvalidPhaseMutation(PagePhase::RefineLyrics)
            ))
        );
    }
}
