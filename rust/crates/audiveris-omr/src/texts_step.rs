// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle port of Java `TextsStep`.
//!
//! OCR, text geometry, role classification, and section mapping remain one
//! injected seam. This module owns the sheet prolog, OCR language context,
//! ordered system traversal, glyph/SIG ownership, sentence membership, lyric
//! numbering, checked continuation, fatal prefixes, and the empty epilog.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NeutralTextRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralTextRole {
    Lyrics,
    ChordName,
    Metronome,
    Standard,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralOcrWord {
    pub id: usize,
    pub value: String,
    pub bounds: NeutralTextRect,
    pub confidence_milli: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralOcrLine {
    pub id: usize,
    pub bounds: NeutralTextRect,
    pub words: Vec<NeutralOcrWord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralTextBuffer {
    pub id: usize,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextsContext {
    pub languages: String,
    /// Java leaves the scanner buffer null when OCR is unavailable.
    pub buffer: Option<NeutralTextBuffer>,
    /// Shared read-only whole-sheet OCR result.
    pub text_lines: Vec<NeutralOcrLine>,
    pub ocr_available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralTextGlyph {
    pub id: usize,
    pub source_word_id: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralTextInterKind {
    Sentence(NeutralTextRole),
    Word(NeutralTextRole),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralTextInter {
    pub id: usize,
    pub kind: NeutralTextInterKind,
    pub staff_id: Option<usize>,
    pub glyph_id: Option<usize>,
    pub value: String,
    /// Sentence membership in Java insertion order.
    pub member_ids: Vec<usize>,
    pub lyric_number: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralMappedTextWord {
    pub system_id: usize,
    pub source_line_id: usize,
    pub source_word_id: usize,
    pub glyph_id: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralTextsSystem {
    pub id: usize,
    pub staff_ids: Vec<usize>,
    pub inters: Vec<NeutralTextInter>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralTextsSheet {
    pub id: usize,
    pub ocr_languages: String,
    pub systems: Vec<NeutralTextsSystem>,
    pub registered_glyphs: Vec<NeutralTextGlyph>,
    pub mapped_words: Vec<NeutralMappedTextWord>,
    pub next_glyph_id: usize,
    pub live_inter_ids: Vec<usize>,
    pub retired_inter_ids: Vec<usize>,
    pub next_inter_id: usize,
    pub mutations: Vec<TextsMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextsMutation {
    GlyphRegistered {
        system_id: usize,
        glyph_id: usize,
    },
    WordGlyphBound(NeutralMappedTextWord),
    WordRejected {
        system_id: usize,
        source_line_id: usize,
        source_word_id: usize,
    },
    InterAdded {
        system_id: usize,
        inter_id: usize,
    },
    InterRemoved {
        system_id: usize,
        inter_id: usize,
    },
    MemberAttached {
        system_id: usize,
        owner_id: usize,
        member_id: usize,
    },
    MemberDetached {
        system_id: usize,
        owner_id: usize,
        member_id: usize,
    },
    LyricLinesNumbered {
        system_id: usize,
        assignments: Vec<(usize, usize)>,
    },
    SystemFailed {
        system_id: usize,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextsDelta {
    /// Exact successful Java mutation prefix before failure.
    pub mutations: Vec<TextsDeltaMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextsDeltaMutation {
    RegisterGlyph(NeutralTextGlyph),
    BindWordGlyph(NeutralMappedTextWord),
    RejectWord {
        source_line_id: usize,
        source_word_id: usize,
    },
    AddInter(NeutralTextInter),
    RemoveInter(usize),
    AttachMember {
        owner_id: usize,
        member_id: usize,
    },
    DetachMember {
        owner_id: usize,
        member_id: usize,
    },
    NumberLyricLines(Vec<(usize, usize)>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextsFailure<E> {
    Checked(E),
    Fatal(E),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextsSystemOutcome<E> {
    pub delta: TextsDelta,
    pub failure: Option<TextsFailure<E>>,
}

impl<E> TextsSystemOutcome<E> {
    #[must_use]
    pub fn success(delta: TextsDelta) -> Self {
        Self {
            delta,
            failure: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TextsPrologInput<'a> {
    pub sheet: &'a NeutralTextsSheet,
    pub languages: &'a str,
}

#[derive(Clone, Copy, Debug)]
pub struct TextsSystemInput<'a> {
    pub sheet: &'a NeutralTextsSheet,
    pub system_id: usize,
    pub context: &'a TextsContext,
}

/// First unavailable dependency: OCR availability and whole-sheet recognition.
/// The same adapter implements `TextBuilder`'s geometry/classification seam.
pub trait ExternalTexts {
    type Error;

    fn is_ocr_available(&mut self, input: TextsPrologInput<'_>) -> bool;
    fn scan_sheet(
        &mut self,
        input: TextsPrologInput<'_>,
    ) -> Result<(NeutralTextBuffer, Vec<NeutralOcrLine>), Self::Error>;
    fn process_system_texts(
        &mut self,
        input: TextsSystemInput<'_>,
    ) -> TextsSystemOutcome<Self::Error>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextsReport<E> {
    pub context: TextsContext,
    pub system_errors: Vec<(usize, E)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextsStepError<E> {
    FatalScan {
        sheet_id: usize,
        source: E,
    },
    FatalSystem {
        system_id: usize,
        source: E,
        checked_errors: Vec<(usize, E)>,
        context: TextsContext,
    },
    Contract(TextsContractError),
}

impl<E: fmt::Display> fmt::Display for TextsStepError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FatalScan { sheet_id, source } => {
                write!(f, "texts sheet {sheet_id} OCR failed: {source}")
            }
            Self::FatalSystem {
                system_id, source, ..
            } => {
                write!(f, "texts system {system_id} failed: {source}")
            }
            Self::Contract(source) => write!(f, "texts contract failed: {source}"),
        }
    }
}

impl<E: Error + 'static> Error for TextsStepError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::FatalScan { source, .. } | Self::FatalSystem { source, .. } => Some(source),
            Self::Contract(source) => Some(source),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextsContractError {
    DuplicateSystem(usize),
    DuplicateStaff(usize),
    DuplicateGlyph(usize),
    InvalidGlyphAllocator { expected: usize, actual: usize },
    DuplicateInter(usize),
    InvalidInterAllocator { expected: usize, actual: usize },
    InvalidLiveInterIndex,
    InvalidRetiredInter(usize),
    UnknownSystem(usize),
    UnknownStaff { system_id: usize, staff_id: usize },
    UnknownGlyph(usize),
    DuplicateWordBinding { system_id: usize, word_id: usize },
    BindingSystemMismatch { expected: usize, actual: usize },
    DuplicateOcrLine(usize),
    DuplicateOcrWord(usize),
    UnknownOcrLine(usize),
    UnknownOcrWord { line_id: usize, word_id: usize },
    GlyphSourceMismatch { glyph_id: usize, word_id: usize },
    UnknownInter { system_id: usize, inter_id: usize },
    InvalidSentenceOwner(usize),
    InvalidWordMember(usize),
    DuplicateMember { owner_id: usize, member_id: usize },
    UnknownMember { owner_id: usize, member_id: usize },
    InvalidLyricLine(usize),
    DuplicateLyricNumber(usize),
}

impl fmt::Display for TextsContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Error for TextsContractError {}

pub struct HeadlessTextsStep<External> {
    external: External,
}

impl<External> HeadlessTextsStep<External> {
    #[must_use]
    pub const fn new(external: External) -> Self {
        Self { external }
    }

    #[must_use]
    pub const fn external(&self) -> &External {
        &self.external
    }
}

impl<External: ExternalTexts> HeadlessTextsStep<External> {
    /// Java `AbstractSystemStep.doit`: prolog, every system, empty epilog.
    pub fn process(
        &mut self,
        sheet: &mut NeutralTextsSheet,
    ) -> Result<TextsReport<External::Error>, TextsStepError<External::Error>> {
        validate_sheet(sheet).map_err(TextsStepError::Contract)?;
        let languages = sheet.ocr_languages.clone();
        let available = self.external.is_ocr_available(TextsPrologInput {
            sheet,
            languages: &languages,
        });
        let context = if available {
            let (buffer, text_lines) = self
                .external
                .scan_sheet(TextsPrologInput {
                    sheet,
                    languages: &languages,
                })
                .map_err(|source| TextsStepError::FatalScan {
                    sheet_id: sheet.id,
                    source,
                })?;
            validate_ocr_lines(&text_lines).map_err(TextsStepError::Contract)?;
            TextsContext {
                languages,
                buffer: Some(buffer),
                text_lines,
                ocr_available: true,
            }
        } else {
            TextsContext {
                languages,
                buffer: None,
                text_lines: Vec::new(),
                ocr_available: false,
            }
        };

        let mut system_errors = Vec::new();
        for system_index in 0..sheet.systems.len() {
            let system_id = sheet.systems[system_index].id;
            let outcome = self.external.process_system_texts(TextsSystemInput {
                sheet,
                system_id,
                context: &context,
            });
            apply_delta(sheet, system_id, &context, outcome.delta)
                .map_err(TextsStepError::Contract)?;
            match outcome.failure {
                None => {}
                Some(TextsFailure::Checked(source)) => {
                    sheet
                        .mutations
                        .push(TextsMutation::SystemFailed { system_id });
                    system_errors.push((system_id, source));
                }
                Some(TextsFailure::Fatal(source)) => {
                    return Err(TextsStepError::FatalSystem {
                        system_id,
                        source,
                        checked_errors: system_errors,
                        context,
                    });
                }
            }
        }

        Ok(TextsReport {
            context,
            system_errors,
        })
    }
}

fn apply_delta(
    sheet: &mut NeutralTextsSheet,
    system_id: usize,
    context: &TextsContext,
    delta: TextsDelta,
) -> Result<(), TextsContractError> {
    let system_index = sheet
        .systems
        .iter()
        .position(|system| system.id == system_id)
        .ok_or(TextsContractError::UnknownSystem(system_id))?;
    for mutation in delta.mutations {
        match mutation {
            TextsDeltaMutation::RegisterGlyph(glyph) => {
                if glyph.id != sheet.next_glyph_id {
                    return Err(TextsContractError::InvalidGlyphAllocator {
                        expected: sheet.next_glyph_id,
                        actual: glyph.id,
                    });
                }
                if sheet.registered_glyphs.iter().any(|old| old.id == glyph.id) {
                    return Err(TextsContractError::DuplicateGlyph(glyph.id));
                }
                require_ocr_word(context, glyph.source_word_id)?;
                sheet.next_glyph_id = sheet.next_glyph_id.checked_add(1).ok_or(
                    TextsContractError::InvalidGlyphAllocator {
                        expected: sheet.next_glyph_id,
                        actual: glyph.id,
                    },
                )?;
                let glyph_id = glyph.id;
                sheet.registered_glyphs.push(glyph);
                sheet.mutations.push(TextsMutation::GlyphRegistered {
                    system_id,
                    glyph_id,
                });
            }
            TextsDeltaMutation::BindWordGlyph(binding) => {
                bind_word(sheet, system_id, context, binding)?;
            }
            TextsDeltaMutation::RejectWord {
                source_line_id,
                source_word_id,
            } => {
                require_ocr_word_in_line(context, source_line_id, source_word_id)?;
                sheet.mutations.push(TextsMutation::WordRejected {
                    system_id,
                    source_line_id,
                    source_word_id,
                });
            }
            TextsDeltaMutation::AddInter(inter) => add_inter(sheet, system_index, inter)?,
            TextsDeltaMutation::RemoveInter(inter_id) => {
                remove_inter(sheet, system_index, inter_id)?;
            }
            TextsDeltaMutation::AttachMember {
                owner_id,
                member_id,
            } => attach_member(sheet, system_index, owner_id, member_id)?,
            TextsDeltaMutation::DetachMember {
                owner_id,
                member_id,
            } => detach_member(sheet, system_index, owner_id, member_id)?,
            TextsDeltaMutation::NumberLyricLines(assignments) => {
                number_lyrics(sheet, system_index, assignments)?;
            }
        }
    }
    Ok(())
}

fn bind_word(
    sheet: &mut NeutralTextsSheet,
    system_id: usize,
    context: &TextsContext,
    binding: NeutralMappedTextWord,
) -> Result<(), TextsContractError> {
    if binding.system_id != system_id {
        return Err(TextsContractError::BindingSystemMismatch {
            expected: system_id,
            actual: binding.system_id,
        });
    }
    require_ocr_word_in_line(context, binding.source_line_id, binding.source_word_id)?;
    let glyph = sheet
        .registered_glyphs
        .iter()
        .find(|glyph| glyph.id == binding.glyph_id)
        .ok_or(TextsContractError::UnknownGlyph(binding.glyph_id))?;
    if glyph.source_word_id != binding.source_word_id {
        return Err(TextsContractError::GlyphSourceMismatch {
            glyph_id: binding.glyph_id,
            word_id: binding.source_word_id,
        });
    }
    if sheet
        .mapped_words
        .iter()
        .any(|word| word.system_id == system_id && word.source_word_id == binding.source_word_id)
    {
        return Err(TextsContractError::DuplicateWordBinding {
            system_id,
            word_id: binding.source_word_id,
        });
    }
    sheet.mapped_words.push(binding);
    sheet.mutations.push(TextsMutation::WordGlyphBound(binding));
    Ok(())
}

fn add_inter(
    sheet: &mut NeutralTextsSheet,
    system_index: usize,
    inter: NeutralTextInter,
) -> Result<(), TextsContractError> {
    let system_id = sheet.systems[system_index].id;
    if inter.id != sheet.next_inter_id {
        return Err(TextsContractError::InvalidInterAllocator {
            expected: sheet.next_inter_id,
            actual: inter.id,
        });
    }
    if sheet.live_inter_ids.contains(&inter.id) || sheet.retired_inter_ids.contains(&inter.id) {
        return Err(TextsContractError::DuplicateInter(inter.id));
    }
    if let Some(staff_id) = inter.staff_id
        && !sheet.systems[system_index].staff_ids.contains(&staff_id)
    {
        return Err(TextsContractError::UnknownStaff {
            system_id,
            staff_id,
        });
    }
    if let Some(glyph_id) = inter.glyph_id
        && !sheet
            .registered_glyphs
            .iter()
            .any(|glyph| glyph.id == glyph_id)
    {
        return Err(TextsContractError::UnknownGlyph(glyph_id));
    }
    // Java adds a blank sentence/word vertex, then calls sentence.addMember.
    if let Some(&member_id) = inter.member_ids.first() {
        return Err(TextsContractError::DuplicateMember {
            owner_id: inter.id,
            member_id,
        });
    }
    let inter_id = inter.id;
    sheet.next_inter_id =
        sheet
            .next_inter_id
            .checked_add(1)
            .ok_or(TextsContractError::InvalidInterAllocator {
                expected: sheet.next_inter_id,
                actual: inter.id,
            })?;
    sheet.systems[system_index].inters.push(inter);
    sheet.live_inter_ids.push(inter_id);
    sheet.mutations.push(TextsMutation::InterAdded {
        system_id,
        inter_id,
    });
    Ok(())
}

fn attach_member(
    sheet: &mut NeutralTextsSheet,
    system_index: usize,
    owner_id: usize,
    member_id: usize,
) -> Result<(), TextsContractError> {
    let system_id = sheet.systems[system_index].id;
    let member = sheet.systems[system_index]
        .inters
        .iter()
        .find(|inter| inter.id == member_id)
        .ok_or(TextsContractError::UnknownInter {
            system_id,
            inter_id: member_id,
        })?;
    if !matches!(member.kind, NeutralTextInterKind::Word(_)) {
        return Err(TextsContractError::InvalidWordMember(member_id));
    }
    let member_kind = member.kind;
    let owner = sheet.systems[system_index]
        .inters
        .iter_mut()
        .find(|inter| inter.id == owner_id)
        .ok_or(TextsContractError::UnknownInter {
            system_id,
            inter_id: owner_id,
        })?;
    if !matches!(owner.kind, NeutralTextInterKind::Sentence(_)) {
        return Err(TextsContractError::InvalidSentenceOwner(owner_id));
    }
    if !matches!(
        (owner.kind, member_kind),
        (
            NeutralTextInterKind::Sentence(owner_role),
            NeutralTextInterKind::Word(member_role)
        ) if owner_role == member_role
    ) {
        return Err(TextsContractError::InvalidWordMember(member_id));
    }
    if owner.member_ids.contains(&member_id) {
        return Err(TextsContractError::DuplicateMember {
            owner_id,
            member_id,
        });
    }
    owner.member_ids.push(member_id);
    sheet.mutations.push(TextsMutation::MemberAttached {
        system_id,
        owner_id,
        member_id,
    });
    Ok(())
}

fn detach_member(
    sheet: &mut NeutralTextsSheet,
    system_index: usize,
    owner_id: usize,
    member_id: usize,
) -> Result<(), TextsContractError> {
    let system_id = sheet.systems[system_index].id;
    let owner = sheet.systems[system_index]
        .inters
        .iter_mut()
        .find(|inter| inter.id == owner_id)
        .ok_or(TextsContractError::UnknownInter {
            system_id,
            inter_id: owner_id,
        })?;
    let member_index = owner
        .member_ids
        .iter()
        .position(|id| *id == member_id)
        .ok_or(TextsContractError::UnknownMember {
            owner_id,
            member_id,
        })?;
    owner.member_ids.remove(member_index);
    sheet.mutations.push(TextsMutation::MemberDetached {
        system_id,
        owner_id,
        member_id,
    });
    Ok(())
}

fn number_lyrics(
    sheet: &mut NeutralTextsSheet,
    system_index: usize,
    assignments: Vec<(usize, usize)>,
) -> Result<(), TextsContractError> {
    let system_id = sheet.systems[system_index].id;
    let mut numbers = BTreeSet::new();
    for &(inter_id, number) in &assignments {
        if !numbers.insert(number) {
            return Err(TextsContractError::DuplicateLyricNumber(number));
        }
        let inter = sheet.systems[system_index]
            .inters
            .iter()
            .find(|inter| inter.id == inter_id)
            .ok_or(TextsContractError::UnknownInter {
                system_id,
                inter_id,
            })?;
        if inter.kind != NeutralTextInterKind::Sentence(NeutralTextRole::Lyrics) {
            return Err(TextsContractError::InvalidLyricLine(inter_id));
        }
    }
    for &(inter_id, number) in &assignments {
        let inter = sheet.systems[system_index]
            .inters
            .iter_mut()
            .find(|inter| inter.id == inter_id)
            .expect("validated lyric line remains live");
        inter.lyric_number = Some(number);
    }
    sheet.mutations.push(TextsMutation::LyricLinesNumbered {
        system_id,
        assignments,
    });
    Ok(())
}

fn remove_inter(
    sheet: &mut NeutralTextsSheet,
    system_index: usize,
    inter_id: usize,
) -> Result<(), TextsContractError> {
    let system_id = sheet.systems[system_index].id;
    let inter_index = sheet.systems[system_index]
        .inters
        .iter()
        .position(|inter| inter.id == inter_id)
        .ok_or(TextsContractError::UnknownInter {
            system_id,
            inter_id,
        })?;
    let owner_ids = sheet.systems[system_index]
        .inters
        .iter()
        .filter(|owner| owner.member_ids.contains(&inter_id))
        .map(|owner| owner.id)
        .collect::<Vec<_>>();
    for owner_id in owner_ids {
        detach_member(sheet, system_index, owner_id, inter_id)?;
    }
    for member_id in sheet.systems[system_index].inters[inter_index]
        .member_ids
        .clone()
    {
        sheet.mutations.push(TextsMutation::MemberDetached {
            system_id,
            owner_id: inter_id,
            member_id,
        });
    }
    sheet.systems[system_index].inters.remove(inter_index);
    let live_index = sheet
        .live_inter_ids
        .iter()
        .position(|id| *id == inter_id)
        .ok_or(TextsContractError::InvalidLiveInterIndex)?;
    sheet.live_inter_ids.remove(live_index);
    sheet.retired_inter_ids.push(inter_id);
    sheet.mutations.push(TextsMutation::InterRemoved {
        system_id,
        inter_id,
    });
    Ok(())
}

fn require_ocr_word(context: &TextsContext, word_id: usize) -> Result<(), TextsContractError> {
    context
        .text_lines
        .iter()
        .any(|line| line.words.iter().any(|word| word.id == word_id))
        .then_some(())
        .ok_or(TextsContractError::UnknownOcrWord {
            line_id: usize::MAX,
            word_id,
        })
}

fn require_ocr_word_in_line(
    context: &TextsContext,
    line_id: usize,
    word_id: usize,
) -> Result<(), TextsContractError> {
    let line = context
        .text_lines
        .iter()
        .find(|line| line.id == line_id)
        .ok_or(TextsContractError::UnknownOcrLine(line_id))?;
    line.words
        .iter()
        .any(|word| word.id == word_id)
        .then_some(())
        .ok_or(TextsContractError::UnknownOcrWord { line_id, word_id })
}

fn validate_ocr_lines(lines: &[NeutralOcrLine]) -> Result<(), TextsContractError> {
    let mut line_ids = BTreeSet::new();
    let mut word_ids = BTreeSet::new();
    for line in lines {
        if !line_ids.insert(line.id) {
            return Err(TextsContractError::DuplicateOcrLine(line.id));
        }
        for word in &line.words {
            if !word_ids.insert(word.id) {
                return Err(TextsContractError::DuplicateOcrWord(word.id));
            }
        }
    }
    Ok(())
}

fn validate_sheet(sheet: &NeutralTextsSheet) -> Result<(), TextsContractError> {
    let mut glyph_ids = BTreeSet::new();
    for glyph in &sheet.registered_glyphs {
        if !glyph_ids.insert(glyph.id) {
            return Err(TextsContractError::DuplicateGlyph(glyph.id));
        }
        if glyph.id >= sheet.next_glyph_id {
            return Err(TextsContractError::InvalidGlyphAllocator {
                expected: sheet.next_glyph_id,
                actual: glyph.id,
            });
        }
    }
    let mut system_ids = BTreeSet::new();
    let mut inter_ids = BTreeSet::new();
    for system in &sheet.systems {
        if !system_ids.insert(system.id) {
            return Err(TextsContractError::DuplicateSystem(system.id));
        }
        let mut staff_ids = BTreeSet::new();
        for &staff_id in &system.staff_ids {
            if !staff_ids.insert(staff_id) {
                return Err(TextsContractError::DuplicateStaff(staff_id));
            }
        }
        for inter in &system.inters {
            if !inter_ids.insert(inter.id) {
                return Err(TextsContractError::DuplicateInter(inter.id));
            }
            if inter.id >= sheet.next_inter_id {
                return Err(TextsContractError::InvalidInterAllocator {
                    expected: sheet.next_inter_id,
                    actual: inter.id,
                });
            }
            if let Some(staff_id) = inter.staff_id
                && !staff_ids.contains(&staff_id)
            {
                return Err(TextsContractError::UnknownStaff {
                    system_id: system.id,
                    staff_id,
                });
            }
            if let Some(glyph_id) = inter.glyph_id
                && !glyph_ids.contains(&glyph_id)
            {
                return Err(TextsContractError::UnknownGlyph(glyph_id));
            }
            let mut members = BTreeSet::new();
            for &member_id in &inter.member_ids {
                if !matches!(inter.kind, NeutralTextInterKind::Sentence(_)) {
                    return Err(TextsContractError::InvalidSentenceOwner(inter.id));
                }
                if !members.insert(member_id) {
                    return Err(TextsContractError::DuplicateMember {
                        owner_id: inter.id,
                        member_id,
                    });
                }
                let member = system
                    .inters
                    .iter()
                    .find(|candidate| candidate.id == member_id)
                    .ok_or(TextsContractError::UnknownMember {
                        owner_id: inter.id,
                        member_id,
                    })?;
                if !matches!(member.kind, NeutralTextInterKind::Word(_)) {
                    return Err(TextsContractError::InvalidWordMember(member_id));
                }
            }
        }
    }
    let live = sheet
        .live_inter_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if live.len() != sheet.live_inter_ids.len() || live != inter_ids {
        return Err(TextsContractError::InvalidLiveInterIndex);
    }
    let mut retired = BTreeSet::new();
    for &inter_id in &sheet.retired_inter_ids {
        if inter_id >= sheet.next_inter_id
            || inter_ids.contains(&inter_id)
            || !retired.insert(inter_id)
        {
            return Err(TextsContractError::InvalidRetiredInter(inter_id));
        }
    }
    let mut bindings = BTreeSet::new();
    for binding in &sheet.mapped_words {
        if !system_ids.contains(&binding.system_id) {
            return Err(TextsContractError::UnknownSystem(binding.system_id));
        }
        if !glyph_ids.contains(&binding.glyph_id) {
            return Err(TextsContractError::UnknownGlyph(binding.glyph_id));
        }
        let glyph = sheet
            .registered_glyphs
            .iter()
            .find(|glyph| glyph.id == binding.glyph_id)
            .expect("validated glyph remains live");
        if glyph.source_word_id != binding.source_word_id {
            return Err(TextsContractError::GlyphSourceMismatch {
                glyph_id: binding.glyph_id,
                word_id: binding.source_word_id,
            });
        }
        if !bindings.insert((binding.system_id, binding.source_word_id)) {
            return Err(TextsContractError::DuplicateWordBinding {
                system_id: binding.system_id,
                word_id: binding.source_word_id,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct FakeTexts {
        available: bool,
        availability_languages: Vec<String>,
        scan_languages: Vec<String>,
        calls: Vec<(usize, String, bool, Vec<usize>)>,
        scan_error: Option<&'static str>,
        outcomes: BTreeMap<usize, TextsSystemOutcome<&'static str>>,
    }

    impl ExternalTexts for FakeTexts {
        type Error = &'static str;

        fn is_ocr_available(&mut self, input: TextsPrologInput<'_>) -> bool {
            self.availability_languages.push(input.languages.to_owned());
            self.available
        }

        fn scan_sheet(
            &mut self,
            input: TextsPrologInput<'_>,
        ) -> Result<(NeutralTextBuffer, Vec<NeutralOcrLine>), Self::Error> {
            self.scan_languages.push(input.languages.to_owned());
            if let Some(error) = self.scan_error.take() {
                return Err(error);
            }
            Ok((
                NeutralTextBuffer {
                    id: 7,
                    width: 800,
                    height: 1200,
                },
                ocr_lines(),
            ))
        }

        fn process_system_texts(
            &mut self,
            input: TextsSystemInput<'_>,
        ) -> TextsSystemOutcome<Self::Error> {
            self.calls.push((
                input.system_id,
                input.context.languages.clone(),
                input.context.buffer.is_some(),
                input
                    .context
                    .text_lines
                    .iter()
                    .map(|line| line.id)
                    .collect(),
            ));
            self.outcomes
                .remove(&input.system_id)
                .unwrap_or_else(|| TextsSystemOutcome::success(TextsDelta::default()))
        }
    }

    fn ocr_lines() -> Vec<NeutralOcrLine> {
        vec![NeutralOcrLine {
            id: 10,
            bounds: NeutralTextRect {
                x: 20,
                y: 30,
                width: 80,
                height: 12,
            },
            words: vec![NeutralOcrWord {
                id: 11,
                value: "Hallo".to_owned(),
                bounds: NeutralTextRect {
                    x: 20,
                    y: 30,
                    width: 35,
                    height: 12,
                },
                confidence_milli: 940,
            }],
        }]
    }

    fn sheet() -> NeutralTextsSheet {
        NeutralTextsSheet {
            id: 3,
            ocr_languages: "deu+eng".to_owned(),
            systems: vec![
                NeutralTextsSystem {
                    id: 4,
                    staff_ids: vec![20],
                    inters: Vec::new(),
                },
                NeutralTextsSystem {
                    id: 2,
                    staff_ids: vec![30],
                    inters: Vec::new(),
                },
            ],
            registered_glyphs: Vec::new(),
            mapped_words: Vec::new(),
            next_glyph_id: 1,
            live_inter_ids: Vec::new(),
            retired_inter_ids: Vec::new(),
            next_inter_id: 1,
            mutations: Vec::new(),
        }
    }

    fn first_system_delta() -> TextsDelta {
        TextsDelta {
            mutations: vec![
                TextsDeltaMutation::RegisterGlyph(NeutralTextGlyph {
                    id: 1,
                    source_word_id: 11,
                }),
                TextsDeltaMutation::BindWordGlyph(NeutralMappedTextWord {
                    system_id: 4,
                    source_line_id: 10,
                    source_word_id: 11,
                    glyph_id: 1,
                }),
                TextsDeltaMutation::AddInter(NeutralTextInter {
                    id: 1,
                    kind: NeutralTextInterKind::Sentence(NeutralTextRole::Lyrics),
                    staff_id: Some(20),
                    glyph_id: None,
                    value: "Hallo".to_owned(),
                    member_ids: Vec::new(),
                    lyric_number: None,
                }),
                TextsDeltaMutation::AddInter(NeutralTextInter {
                    id: 2,
                    kind: NeutralTextInterKind::Word(NeutralTextRole::Lyrics),
                    staff_id: Some(20),
                    glyph_id: Some(1),
                    value: "Hallo".to_owned(),
                    member_ids: Vec::new(),
                    lyric_number: None,
                }),
                TextsDeltaMutation::AttachMember {
                    owner_id: 1,
                    member_id: 2,
                },
                TextsDeltaMutation::NumberLyricLines(vec![(1, 1)]),
            ],
        }
    }

    #[test]
    fn exact_prolog_and_system_order_share_language_buffer_and_lines() {
        let mut external = FakeTexts {
            available: true,
            ..FakeTexts::default()
        };
        external
            .outcomes
            .insert(4, TextsSystemOutcome::success(first_system_delta()));
        let mut step = HeadlessTextsStep::new(external);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(step.external().availability_languages, ["deu+eng"]);
        assert_eq!(step.external().scan_languages, ["deu+eng"]);
        assert_eq!(
            step.external().calls,
            [
                (4, "deu+eng".to_owned(), true, vec![10]),
                (2, "deu+eng".to_owned(), true, vec![10]),
            ]
        );
        assert!(report.system_errors.is_empty());
        assert_eq!(sheet.systems[0].inters[0].member_ids, [2]);
        assert_eq!(sheet.systems[0].inters[0].lyric_number, Some(1));
        assert_eq!(sheet.live_inter_ids, [1, 2]);
        assert_eq!(sheet.mapped_words[0].glyph_id, 1);
        assert_eq!(
            sheet.mutations[..6],
            [
                TextsMutation::GlyphRegistered {
                    system_id: 4,
                    glyph_id: 1,
                },
                TextsMutation::WordGlyphBound(NeutralMappedTextWord {
                    system_id: 4,
                    source_line_id: 10,
                    source_word_id: 11,
                    glyph_id: 1,
                }),
                TextsMutation::InterAdded {
                    system_id: 4,
                    inter_id: 1,
                },
                TextsMutation::InterAdded {
                    system_id: 4,
                    inter_id: 2,
                },
                TextsMutation::MemberAttached {
                    system_id: 4,
                    owner_id: 1,
                    member_id: 2,
                },
                TextsMutation::LyricLinesNumbered {
                    system_id: 4,
                    assignments: vec![(1, 1)],
                },
            ]
        );
    }

    #[test]
    fn unavailable_ocr_skips_scan_but_still_traverses_every_system() {
        let mut step = HeadlessTextsStep::new(FakeTexts::default());
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert!(!report.context.ocr_available);
        assert!(report.context.buffer.is_none());
        assert!(report.context.text_lines.is_empty());
        assert!(step.external().scan_languages.is_empty());
        assert_eq!(step.external().calls.len(), 2);
        assert!(step.external().calls.iter().all(|call| !call.2));
    }

    #[test]
    fn scan_failure_has_sheet_prefix_and_starts_no_system() {
        let external = FakeTexts {
            available: true,
            scan_error: Some("ocr crashed"),
            ..FakeTexts::default()
        };
        let mut step = HeadlessTextsStep::new(external);
        let mut sheet = sheet();

        assert_eq!(
            step.process(&mut sheet),
            Err(TextsStepError::FatalScan {
                sheet_id: 3,
                source: "ocr crashed",
            })
        );
        assert!(step.external().calls.is_empty());
    }

    #[test]
    fn checked_failure_keeps_mutation_prefix_and_continues() {
        let mut external = FakeTexts {
            available: true,
            ..FakeTexts::default()
        };
        external.outcomes.insert(
            4,
            TextsSystemOutcome {
                delta: TextsDelta {
                    mutations: first_system_delta().mutations[..2].to_vec(),
                },
                failure: Some(TextsFailure::Checked("bad system text")),
            },
        );
        let mut step = HeadlessTextsStep::new(external);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors, [(4, "bad system text")]);
        assert_eq!(step.external().calls.len(), 2);
        assert_eq!(sheet.registered_glyphs[0].id, 1);
        assert!(
            sheet
                .mutations
                .contains(&TextsMutation::SystemFailed { system_id: 4 })
        );
    }

    #[test]
    fn fatal_system_returns_context_and_skips_later_system() {
        let mut external = FakeTexts {
            available: true,
            ..FakeTexts::default()
        };
        external.outcomes.insert(
            4,
            TextsSystemOutcome {
                delta: TextsDelta {
                    mutations: first_system_delta().mutations[..2].to_vec(),
                },
                failure: Some(TextsFailure::Fatal("fatal text")),
            },
        );
        let mut step = HeadlessTextsStep::new(external);
        let mut sheet = sheet();

        let error = step.process(&mut sheet).unwrap_err();

        assert!(matches!(
            error,
            TextsStepError::FatalSystem {
                system_id: 4,
                source: "fatal text",
                context: TextsContext {
                    ocr_available: true,
                    ..
                },
                ..
            }
        ));
        assert_eq!(step.external().calls.len(), 1);
        assert_eq!(sheet.mapped_words.len(), 1);
    }

    #[test]
    fn removal_cascades_sentence_ownership_and_preserves_allocator_hole() {
        let mut external = FakeTexts {
            available: true,
            ..FakeTexts::default()
        };
        let mut delta = first_system_delta();
        delta.mutations.push(TextsDeltaMutation::RemoveInter(2));
        external
            .outcomes
            .insert(4, TextsSystemOutcome::success(delta));
        let mut step = HeadlessTextsStep::new(external);
        let mut sheet = sheet();

        step.process(&mut sheet).unwrap();

        assert_eq!(sheet.next_inter_id, 3);
        assert_eq!(sheet.retired_inter_ids, [2]);
        assert_eq!(sheet.live_inter_ids, [1]);
        assert!(sheet.systems[0].inters[0].member_ids.is_empty());
        assert!(sheet.mutations.iter().any(|mutation| matches!(
            mutation,
            TextsMutation::MemberDetached {
                owner_id: 1,
                member_id: 2,
                ..
            }
        )));
    }
}
