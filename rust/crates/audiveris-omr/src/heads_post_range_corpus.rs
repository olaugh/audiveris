// SPDX-License-Identifier: AGPL-3.0-or-later

//! Typed reader for the compact Java HEADS post-range oracle.
//!
//! The default fixture deliberately omits the verbose initial-head, pair-check,
//! tally, and beam-check rows. Their exact counts and FNV-1a-64 digests remain
//! in the summaries. Purge decisions, staff survivors, small-beam inputs and
//! decisions, final system heads, and tally-analysis inputs are retained in
//! full. This module keeps that evidence boundary explicit rather than
//! reconstructing identities which the compact oracle does not contain.

use std::{collections::BTreeMap, error::Error, fmt};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Exact Java `Rectangle` fields.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PostRangeBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// The two head shapes present in the frozen post-range corpus.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PostRangeShape {
    NoteheadVoid,
    NoteheadBlack,
    Beam,
    BeamHook,
}

impl PostRangeShape {
    #[must_use]
    pub const fn java_name(self) -> &'static str {
        match self {
            Self::NoteheadVoid => "NOTEHEAD_VOID",
            Self::NoteheadBlack => "NOTEHEAD_BLACK",
            Self::Beam => "BEAM",
            Self::BeamHook => "BEAM_HOOK",
        }
    }
}

/// A Java-rendered binary64 value together with its authoritative raw bits.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PostRangeDouble {
    pub java_hex: String,
    pub bits: u64,
}

impl PostRangeDouble {
    #[must_use]
    pub fn value(&self) -> f64 {
        f64::from_bits(self.bits)
    }

    fn canonical(&self) -> String {
        format!("{}/{:016x}", self.java_hex, self.bits)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PostRangeImpact {
    pub name: String,
    pub value: PostRangeDouble,
    pub weight: PostRangeDouble,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PostRangeGlyph {
    pub bounds: PostRangeBounds,
    pub weight: usize,
    pub run_hash: u64,
}

/// Identity-free fields emitted by `interRecord` in the Java probe.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PostRangeInter {
    pub class_name: String,
    pub shape: PostRangeShape,
    pub bounds: PostRangeBounds,
    pub grade: PostRangeDouble,
    pub best_grade: PostRangeDouble,
    pub contextual_grade: Option<PostRangeDouble>,
    pub good: bool,
    pub removed: bool,
    pub impacts: Vec<PostRangeImpact>,
    pub glyph: Option<PostRangeGlyph>,
    pub beam_width: Option<i32>,
    pub mean_height: Option<PostRangeDouble>,
}

/// Stable identity-free key shared by staff, purge, and final-head rows.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PostRangeHeadKey {
    pub shape: PostRangeShape,
    pub bounds: PostRangeBounds,
    pub grade_bits: u64,
    pub best_grade_bits: u64,
    pub glyph: Option<PostRangeGlyph>,
}

impl PostRangeInter {
    #[must_use]
    pub fn head_key(&self) -> PostRangeHeadKey {
        PostRangeHeadKey {
            shape: self.shape,
            bounds: self.bounds,
            grade_bits: self.grade.bits,
            best_grade_bits: self.best_grade.bits,
            glyph: self.glyph,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PostRangeOrigin {
    Seed(usize),
    Range(usize),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PostRangeInputRef {
    pub input_ordinal: usize,
    pub origin: PostRangeOrigin,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PostRangeTally {
    pub left: Option<PostRangeDouble>,
    pub right: Option<PostRangeDouble>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PostRangePurgeChoice {
    LowerGradeLeft,
    LowerGradeRight,
    EqualLeftNoSeed,
    EqualRightNoSeed,
    EqualShape,
    EqualBounds,
    EqualPreserveLeft,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostRangePurgeDecision {
    pub ordinal: usize,
    pub left: PostRangeInputRef,
    pub right: PostRangeInputRef,
    pub grade_difference: PostRangeDouble,
    pub choice: PostRangePurgeChoice,
    pub purged_input: usize,
    pub kept_input: usize,
    pub left_tally: PostRangeTally,
    pub right_tally: PostRangeTally,
    /// Exact row retained for category-hash validation.
    pub canonical_row: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostRangeStaffHead {
    pub input_ordinal: usize,
    pub origin: PostRangeOrigin,
    pub stemless: bool,
    pub grade_before: PostRangeDouble,
    pub grade_after: PostRangeDouble,
    pub inter: PostRangeInter,
    pub tally: PostRangeTally,
    pub canonical_row: String,
}

/// Count and FNV digest of a compacted, unexpanded row stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PostRangeCountHash {
    pub count: usize,
    pub hash: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostRangeStaffSummary {
    pub inputs: usize,
    pub seed_inputs: usize,
    pub range_inputs: usize,
    pub duplicates: usize,
    pub after_duplicates: usize,
    pub overlaps: usize,
    pub tally_before: usize,
    pub tally_after: usize,
    pub notes_before: usize,
    pub notes_after: usize,
    pub retained: usize,
    pub input_hash: u64,
    pub duplicate_checks: PostRangeCountHash,
    pub duplicate_hash: u64,
    pub overlap_checks: PostRangeCountHash,
    pub overlap_hash: u64,
    pub tally_hash: u64,
    pub staff_head_hash: u64,
    pub semantic_hash: u64,
}

/// One staff's complete reconstructible compact-oracle contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostRangeStaff {
    pub system_id: usize,
    pub staff_id: usize,
    pub duplicate_decisions: Vec<PostRangePurgeDecision>,
    pub overlap_decisions: Vec<PostRangePurgeDecision>,
    /// Java staff-attachment traversal after duplicate removal. Overlap purge
    /// adds exclusions but does not remove either input.
    pub attachment_heads: Vec<PostRangeStaffHead>,
    pub summary: PostRangeStaffSummary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostRangeHeadPurge {
    /// Ordinal in the system-wide pre-small-beam ordinate order.
    pub head_ordinal: usize,
    pub inter: PostRangeInter,
    pub canonical_row: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostRangeSystemHead {
    pub ordinal: usize,
    pub inter: PostRangeInter,
    pub canonical_row: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostRangeSystemSummary {
    pub inputs: usize,
    pub duplicates: usize,
    pub overlaps: usize,
    pub staff_heads: usize,
    pub beam_inputs: usize,
    pub purged_beams: usize,
    pub purged_heads: usize,
    pub final_heads: usize,
    pub beam_input_hash: u64,
    /// Compact summary of the omitted beam/head comparison rows.
    pub beam_checks: PostRangeCountHash,
    pub beam_decision_hash: u64,
    pub final_head_hash: u64,
    pub semantic_hash: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostRangeSystem {
    pub system_id: usize,
    pub beam_inputs: Vec<PostRangeBeamInput>,
    /// Every production arbitration outcome. The frozen corpus chose the head
    /// on all 26 rows, so no beam-purge record follows any decision.
    pub beam_decisions: Vec<PostRangeBeamDecision>,
    pub purged_heads: Vec<PostRangeHeadPurge>,
    pub final_heads: Vec<PostRangeSystemHead>,
    pub summary: PostRangeSystemSummary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostRangeBeamInput {
    pub ordinal: usize,
    pub inter: PostRangeInter,
    pub canonical_row: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PostRangeBeamPurgeTarget {
    Beam,
    Head,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostRangeBeamDecision {
    pub ordinal: usize,
    pub beam_ordinal: usize,
    pub head_ordinal: usize,
    pub purged: PostRangeBeamPurgeTarget,
    pub canonical_row: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PostRangeSide {
    Left,
    Right,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostRangeScaleInput {
    pub system_id: usize,
    /// Page-wide dense traversal ordinal.
    pub ordinal: usize,
    pub side: PostRangeSide,
    /// Insertion ordinal in the system tally's side map, including removed
    /// entries omitted from the compact stream.
    pub entry_ordinal: usize,
    pub shape: PostRangeShape,
    pub dx: PostRangeDouble,
    pub canonical_row: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostRangeScale {
    pub shape: PostRangeShape,
    pub side: PostRangeSide,
    pub dx: PostRangeDouble,
    pub canonical_row: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostRangePageSummary {
    pub staffs: usize,
    pub inputs: usize,
    pub duplicates: usize,
    pub overlaps: usize,
    pub staff_heads: usize,
    pub purged_beams: usize,
    pub purged_heads: usize,
    pub final_heads: usize,
    pub scale_inputs: usize,
    pub scale_input_hash: u64,
    pub scale_hash: u64,
    pub semantic_hash: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostRangePage {
    pub page: String,
    pub declared_systems: usize,
    pub declared_staves: usize,
    pub family: String,
    pub staffs: Vec<PostRangeStaff>,
    pub systems: Vec<PostRangeSystem>,
    pub scale_inputs: Vec<PostRangeScaleInput>,
    pub scales: Vec<PostRangeScale>,
    pub summary: PostRangePageSummary,
}

impl PostRangePage {
    #[must_use]
    pub fn staff(&self, system_id: usize, staff_id: usize) -> Option<&PostRangeStaff> {
        self.staffs
            .iter()
            .find(|staff| staff.system_id == system_id && staff.staff_id == staff_id)
    }

    #[must_use]
    pub fn system(&self, system_id: usize) -> Option<&PostRangeSystem> {
        self.systems
            .iter()
            .find(|system| system.system_id == system_id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostRangeCorpusSummary {
    pub pages: usize,
    pub systems: usize,
    pub staffs: usize,
    pub inputs: usize,
    pub duplicates: usize,
    pub overlaps: usize,
    pub staff_heads: usize,
    pub purged_beams: usize,
    pub purged_heads: usize,
    pub final_heads: usize,
    pub duplicate_decision_rows: usize,
    pub overlap_decision_rows: usize,
    pub staff_head_rows: usize,
    pub beam_input_rows: usize,
    /// Verbose beam-check rows retained in this compact fixture (zero). The
    /// per-system summaries still commit all checks by count and hash.
    pub beam_check_rows: usize,
    pub beam_decision_rows: usize,
    pub beam_purge_rows: usize,
    pub head_purge_rows: usize,
    pub final_head_rows: usize,
    pub scale_input_rows: usize,
    pub scale_rows: usize,
    pub emitted_body_sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadsPostRangeCorpus {
    pub pages: Vec<PostRangePage>,
    pub summary: PostRangeCorpusSummary,
    /// SHA-256 of the complete fixture, including the corpus-summary row.
    pub fixture_sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadsPostRangeParseError {
    message: String,
}

impl HeadsPostRangeParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn at(line_number: usize, message: impl fmt::Display) -> Self {
        Self::new(format!("line {line_number}: {message}"))
    }
}

impl fmt::Display for HeadsPostRangeParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for HeadsPostRangeParseError {}

#[derive(Default)]
struct StaffBuild {
    duplicate_decisions: Vec<PostRangePurgeDecision>,
    overlap_decisions: Vec<PostRangePurgeDecision>,
    attachment_heads: Vec<PostRangeStaffHead>,
    summary: Option<PostRangeStaffSummary>,
}

#[derive(Default)]
struct SystemBuild {
    beam_inputs: Vec<PostRangeBeamInput>,
    beam_decisions: Vec<PostRangeBeamDecision>,
    purged_heads: Vec<PostRangeHeadPurge>,
    final_heads: Vec<PostRangeSystemHead>,
    summary: Option<PostRangeSystemSummary>,
}

struct PageBuild {
    page: String,
    declared_systems: usize,
    declared_staves: usize,
    family: String,
    staffs: BTreeMap<(usize, usize), StaffBuild>,
    systems: BTreeMap<usize, SystemBuild>,
    scale_inputs: Vec<PostRangeScaleInput>,
    scales: Vec<PostRangeScale>,
}

/// Parse and fully validate the compact post-range oracle.
///
/// Validation covers its SHA-256 body commitment, every reconstructible FNV
/// category hash, hierarchy/count arithmetic, traversal ordinals, and the
/// identity-free multiset transition from staff attachments through the 26
/// recorded head removals to final system heads.
pub fn parse_heads_post_range_corpus(
    text: &str,
) -> Result<HeadsPostRangeCorpus, HeadsPostRangeParseError> {
    let corpus_offset = text
        .find("headpostcorpussummary ")
        .ok_or_else(|| HeadsPostRangeParseError::new("missing corpus summary"))?;
    let emitted_body_sha256 = sha256(&text.as_bytes()[..corpus_offset]);
    let mut pages = Vec::new();
    let mut page: Option<PageBuild> = None;
    let mut corpus_summary = None;

    for (line_index, line) in text.lines().enumerate() {
        let line_number = line_index + 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = Fields::new(line, line_number);
        let kind = fields.next()?;
        match kind {
            "headpostpage" => {
                if page.is_some() {
                    return Err(HeadsPostRangeParseError::at(
                        line_number,
                        "new page before prior page summary",
                    ));
                }
                let page_name = fields.next()?.to_owned();
                fields.expect("systems")?;
                let declared_systems = fields.usize()?;
                fields.expect("staves")?;
                let declared_staves = fields.usize()?;
                fields.expect("family")?;
                let family = fields.next()?.to_owned();
                fields.finish()?;
                page = Some(PageBuild {
                    page: page_name,
                    declared_systems,
                    declared_staves,
                    family,
                    staffs: BTreeMap::new(),
                    systems: BTreeMap::new(),
                    scale_inputs: Vec::new(),
                    scales: Vec::new(),
                });
            }
            "headpostduplicate" | "headpostoverlap" => {
                let page = current_page_mut(&mut page, line_number)?;
                fields.expect("system")?;
                let system_id = fields.usize()?;
                fields.expect("staff")?;
                let staff_id = fields.usize()?;
                let decision = parse_purge_decision(&mut fields, line)?;
                let staff = page.staffs.entry((system_id, staff_id)).or_default();
                if kind == "headpostduplicate" {
                    staff.duplicate_decisions.push(decision);
                } else {
                    staff.overlap_decisions.push(decision);
                }
            }
            "headpoststaffhead" => {
                let page = current_page_mut(&mut page, line_number)?;
                expect_page(&mut fields, &page.page)?;
                fields.expect("system")?;
                let system_id = fields.usize()?;
                fields.expect("staff")?;
                let staff_id = fields.usize()?;
                fields.expect("input")?;
                let input_ordinal = fields.usize()?;
                fields.expect("origin")?;
                let origin = parse_origin(fields.next()?, line_number)?;
                fields.expect("stemless")?;
                let stemless = fields.boolean()?;
                fields.expect("gradeBefore")?;
                let grade_before = fields.double()?;
                fields.expect("gradeAfter")?;
                let grade_after = fields.double()?;
                let inter = parse_inter(&mut fields)?;
                fields.expect("tally")?;
                let tally = fields.tally()?;
                fields.finish()?;
                page.staffs
                    .entry((system_id, staff_id))
                    .or_default()
                    .attachment_heads
                    .push(PostRangeStaffHead {
                        input_ordinal,
                        origin,
                        stemless,
                        grade_before,
                        grade_after,
                        inter,
                        tally,
                        canonical_row: line.to_owned(),
                    });
            }
            "headpoststaffsummary" => {
                let page = current_page_mut(&mut page, line_number)?;
                expect_page(&mut fields, &page.page)?;
                fields.expect("system")?;
                let system_id = fields.usize()?;
                fields.expect("staff")?;
                let staff_id = fields.usize()?;
                let summary = parse_staff_summary(&mut fields)?;
                let slot = &mut page
                    .staffs
                    .entry((system_id, staff_id))
                    .or_default()
                    .summary;
                if slot.replace(summary).is_some() {
                    return Err(HeadsPostRangeParseError::at(
                        line_number,
                        "duplicate staff summary",
                    ));
                }
            }
            "headpostheadpurge" => {
                let page = current_page_mut(&mut page, line_number)?;
                expect_page(&mut fields, &page.page)?;
                fields.expect("system")?;
                let system_id = fields.usize()?;
                fields.expect("head")?;
                let head_ordinal = fields.usize()?;
                let inter = parse_inter(&mut fields)?;
                fields.finish()?;
                page.systems
                    .entry(system_id)
                    .or_default()
                    .purged_heads
                    .push(PostRangeHeadPurge {
                        head_ordinal,
                        inter,
                        canonical_row: line.to_owned(),
                    });
            }
            "headpostbeaminput" => {
                let page = current_page_mut(&mut page, line_number)?;
                expect_page(&mut fields, &page.page)?;
                fields.expect("system")?;
                let system_id = fields.usize()?;
                fields.expect("ordinal")?;
                let ordinal = fields.usize()?;
                let inter = parse_inter(&mut fields)?;
                fields.finish()?;
                page.systems
                    .entry(system_id)
                    .or_default()
                    .beam_inputs
                    .push(PostRangeBeamInput {
                        ordinal,
                        inter,
                        canonical_row: line.to_owned(),
                    });
            }
            "headpostbeamdecision" => {
                let page = current_page_mut(&mut page, line_number)?;
                expect_page(&mut fields, &page.page)?;
                fields.expect("system")?;
                let system_id = fields.usize()?;
                fields.expect("ordinal")?;
                let ordinal = fields.usize()?;
                fields.expect("beam")?;
                let beam_ordinal = fields.usize()?;
                fields.expect("head")?;
                let head_ordinal = fields.usize()?;
                fields.expect("purged")?;
                let purged = match fields.next()? {
                    "beam" => PostRangeBeamPurgeTarget::Beam,
                    "head" => PostRangeBeamPurgeTarget::Head,
                    other => {
                        return Err(fields.error(format_args!("invalid beam purge target {other}")));
                    }
                };
                fields.finish()?;
                page.systems
                    .entry(system_id)
                    .or_default()
                    .beam_decisions
                    .push(PostRangeBeamDecision {
                        ordinal,
                        beam_ordinal,
                        head_ordinal,
                        purged,
                        canonical_row: line.to_owned(),
                    });
            }
            "headpostsystemhead" => {
                let page = current_page_mut(&mut page, line_number)?;
                expect_page(&mut fields, &page.page)?;
                fields.expect("system")?;
                let system_id = fields.usize()?;
                fields.expect("ordinal")?;
                let ordinal = fields.usize()?;
                let inter = parse_inter(&mut fields)?;
                fields.finish()?;
                page.systems
                    .entry(system_id)
                    .or_default()
                    .final_heads
                    .push(PostRangeSystemHead {
                        ordinal,
                        inter,
                        canonical_row: line.to_owned(),
                    });
            }
            "headpostsystemsummary" => {
                let page = current_page_mut(&mut page, line_number)?;
                expect_page(&mut fields, &page.page)?;
                fields.expect("system")?;
                let system_id = fields.usize()?;
                let summary = parse_system_summary(&mut fields)?;
                let slot = &mut page.systems.entry(system_id).or_default().summary;
                if slot.replace(summary).is_some() {
                    return Err(HeadsPostRangeParseError::at(
                        line_number,
                        "duplicate system summary",
                    ));
                }
            }
            "headpostscaleinput" => {
                let page = current_page_mut(&mut page, line_number)?;
                expect_page(&mut fields, &page.page)?;
                fields.expect("system")?;
                let system_id = fields.usize()?;
                fields.expect("ordinal")?;
                let ordinal = fields.usize()?;
                fields.expect("side")?;
                let side = fields.side()?;
                fields.expect("entry")?;
                let entry_ordinal = fields.usize()?;
                fields.expect("shape")?;
                let shape = fields.shape()?;
                fields.expect("dx")?;
                let dx = fields.double()?;
                fields.finish()?;
                page.scale_inputs.push(PostRangeScaleInput {
                    system_id,
                    ordinal,
                    side,
                    entry_ordinal,
                    shape,
                    dx,
                    canonical_row: line.to_owned(),
                });
            }
            "headpostscale" => {
                let page = current_page_mut(&mut page, line_number)?;
                expect_page(&mut fields, &page.page)?;
                fields.expect("shape")?;
                let shape = fields.shape()?;
                fields.expect("side")?;
                let side = fields.side()?;
                fields.expect("dx")?;
                let dx = fields.double()?;
                fields.finish()?;
                page.scales.push(PostRangeScale {
                    shape,
                    side,
                    dx,
                    canonical_row: line.to_owned(),
                });
            }
            "headpostpagesummary" => {
                let mut build = page.take().ok_or_else(|| {
                    HeadsPostRangeParseError::at(line_number, "page summary before page")
                })?;
                expect_page(&mut fields, &build.page)?;
                let summary = parse_page_summary(&mut fields)?;
                pages.push(finish_page(&mut build, summary)?);
            }
            "headpostcorpussummary" => {
                if page.is_some() {
                    return Err(HeadsPostRangeParseError::at(
                        line_number,
                        "corpus summary before page summary",
                    ));
                }
                if corpus_summary.is_some() {
                    return Err(HeadsPostRangeParseError::at(
                        line_number,
                        "duplicate corpus summary",
                    ));
                }
                corpus_summary = Some(parse_corpus_summary(&mut fields)?);
            }
            other => {
                return Err(HeadsPostRangeParseError::at(
                    line_number,
                    format_args!("unsupported compact-oracle row {other}"),
                ));
            }
        }
    }

    if page.is_some() {
        return Err(HeadsPostRangeParseError::new(
            "unterminated page without page summary",
        ));
    }
    let summary = corpus_summary
        .ok_or_else(|| HeadsPostRangeParseError::new("missing parsed corpus summary"))?;
    if summary.emitted_body_sha256 != emitted_body_sha256 {
        return Err(HeadsPostRangeParseError::new(format!(
            "emitted-body SHA-256 mismatch: expected {}, computed {}",
            hex_bytes(&summary.emitted_body_sha256),
            hex_bytes(&emitted_body_sha256)
        )));
    }
    validate_corpus(&pages, &summary)?;
    Ok(HeadsPostRangeCorpus {
        pages,
        summary,
        fixture_sha256: sha256(text.as_bytes()),
    })
}

fn current_page_mut(
    page: &mut Option<PageBuild>,
    line_number: usize,
) -> Result<&mut PageBuild, HeadsPostRangeParseError> {
    page.as_mut()
        .ok_or_else(|| HeadsPostRangeParseError::at(line_number, "record before page"))
}

fn expect_page(fields: &mut Fields<'_>, expected: &str) -> Result<(), HeadsPostRangeParseError> {
    let actual = fields.next()?;
    if actual != expected {
        return Err(fields.error(format_args!(
            "page mismatch: expected {expected}, found {actual}"
        )));
    }
    Ok(())
}

fn parse_purge_decision(
    fields: &mut Fields<'_>,
    line: &str,
) -> Result<PostRangePurgeDecision, HeadsPostRangeParseError> {
    fields.expect("ordinal")?;
    let ordinal = fields.usize()?;
    fields.expect("left")?;
    let left = parse_input_ref(fields.next()?, fields.line_number)?;
    fields.expect("right")?;
    let right = parse_input_ref(fields.next()?, fields.line_number)?;
    fields.expect("diff")?;
    let grade_difference = fields.double()?;
    fields.expect("choice")?;
    let choice = parse_choice(fields.next()?, fields.line_number)?;
    fields.expect("purged")?;
    let purged_input = fields.usize()?;
    fields.expect("kept")?;
    let kept_input = fields.usize()?;
    fields.expect("leftTally")?;
    let left_tally = fields.tally()?;
    fields.expect("rightTally")?;
    let right_tally = fields.tally()?;
    fields.finish()?;
    Ok(PostRangePurgeDecision {
        ordinal,
        left,
        right,
        grade_difference,
        choice,
        purged_input,
        kept_input,
        left_tally,
        right_tally,
        canonical_row: line.to_owned(),
    })
}

fn parse_staff_summary(
    fields: &mut Fields<'_>,
) -> Result<PostRangeStaffSummary, HeadsPostRangeParseError> {
    fields.expect("inputs")?;
    let inputs = fields.usize()?;
    fields.expect("seed")?;
    let seed_inputs = fields.usize()?;
    fields.expect("range")?;
    let range_inputs = fields.usize()?;
    fields.expect("duplicates")?;
    let duplicates = fields.usize()?;
    fields.expect("afterDuplicates")?;
    let after_duplicates = fields.usize()?;
    fields.expect("overlaps")?;
    let overlaps = fields.usize()?;
    fields.expect("tallyBefore")?;
    let tally_before = fields.usize()?;
    fields.expect("tallyAfter")?;
    let tally_after = fields.usize()?;
    fields.expect("notesBefore")?;
    let notes_before = fields.usize()?;
    fields.expect("notesAfter")?;
    let notes_after = fields.usize()?;
    fields.expect("retained")?;
    let retained = fields.usize()?;
    fields.expect("inputHash")?;
    let input_hash = fields.hash()?;
    fields.expect("duplicateChecks")?;
    let duplicate_checks = fields.count_hash()?;
    fields.expect("duplicateHash")?;
    let duplicate_hash = fields.hash()?;
    fields.expect("overlapChecks")?;
    let overlap_checks = fields.count_hash()?;
    fields.expect("overlapHash")?;
    let overlap_hash = fields.hash()?;
    fields.expect("tallyHash")?;
    let tally_hash = fields.hash()?;
    fields.expect("staffHeadHash")?;
    let staff_head_hash = fields.hash()?;
    fields.expect("hash")?;
    let semantic_hash = fields.hash()?;
    fields.finish()?;
    Ok(PostRangeStaffSummary {
        inputs,
        seed_inputs,
        range_inputs,
        duplicates,
        after_duplicates,
        overlaps,
        tally_before,
        tally_after,
        notes_before,
        notes_after,
        retained,
        input_hash,
        duplicate_checks,
        duplicate_hash,
        overlap_checks,
        overlap_hash,
        tally_hash,
        staff_head_hash,
        semantic_hash,
    })
}

fn parse_system_summary(
    fields: &mut Fields<'_>,
) -> Result<PostRangeSystemSummary, HeadsPostRangeParseError> {
    fields.expect("inputs")?;
    let inputs = fields.usize()?;
    fields.expect("duplicates")?;
    let duplicates = fields.usize()?;
    fields.expect("overlaps")?;
    let overlaps = fields.usize()?;
    fields.expect("staffHeads")?;
    let staff_heads = fields.usize()?;
    fields.expect("beamInputs")?;
    let beam_inputs = fields.usize()?;
    fields.expect("purgedBeams")?;
    let purged_beams = fields.usize()?;
    fields.expect("purgedHeads")?;
    let purged_heads = fields.usize()?;
    fields.expect("finalHeads")?;
    let final_heads = fields.usize()?;
    fields.expect("beamInputHash")?;
    let beam_input_hash = fields.hash()?;
    fields.expect("beamChecks")?;
    let beam_checks = fields.count_hash()?;
    fields.expect("beamDecisionHash")?;
    let beam_decision_hash = fields.hash()?;
    fields.expect("finalHeadHash")?;
    let final_head_hash = fields.hash()?;
    fields.expect("hash")?;
    let semantic_hash = fields.hash()?;
    fields.finish()?;
    Ok(PostRangeSystemSummary {
        inputs,
        duplicates,
        overlaps,
        staff_heads,
        beam_inputs,
        purged_beams,
        purged_heads,
        final_heads,
        beam_input_hash,
        beam_checks,
        beam_decision_hash,
        final_head_hash,
        semantic_hash,
    })
}

fn parse_page_summary(
    fields: &mut Fields<'_>,
) -> Result<PostRangePageSummary, HeadsPostRangeParseError> {
    fields.expect("staffs")?;
    let staffs = fields.usize()?;
    fields.expect("inputs")?;
    let inputs = fields.usize()?;
    fields.expect("duplicates")?;
    let duplicates = fields.usize()?;
    fields.expect("overlaps")?;
    let overlaps = fields.usize()?;
    fields.expect("staffHeads")?;
    let staff_heads = fields.usize()?;
    fields.expect("purgedBeams")?;
    let purged_beams = fields.usize()?;
    fields.expect("purgedHeads")?;
    let purged_heads = fields.usize()?;
    fields.expect("finalHeads")?;
    let final_heads = fields.usize()?;
    fields.expect("scaleInputs")?;
    let scale_inputs = fields.usize()?;
    fields.expect("scaleInputHash")?;
    let scale_input_hash = fields.hash()?;
    fields.expect("scaleHash")?;
    let scale_hash = fields.hash()?;
    fields.expect("hash")?;
    let semantic_hash = fields.hash()?;
    fields.finish()?;
    Ok(PostRangePageSummary {
        staffs,
        inputs,
        duplicates,
        overlaps,
        staff_heads,
        purged_beams,
        purged_heads,
        final_heads,
        scale_inputs,
        scale_input_hash,
        scale_hash,
        semantic_hash,
    })
}

fn parse_corpus_summary(
    fields: &mut Fields<'_>,
) -> Result<PostRangeCorpusSummary, HeadsPostRangeParseError> {
    fields.expect("pages")?;
    let pages = fields.usize()?;
    fields.expect("systems")?;
    let systems = fields.usize()?;
    fields.expect("staffs")?;
    let staffs = fields.usize()?;
    fields.expect(
        "totals:inputs:duplicates:overlaps:staffHeads:purgedBeams:purgedHeads:finalHeads",
    )?;
    let totals = parse_colon_usizes(fields.next()?, 7, fields.line_number)?;
    fields.expect("decisionRows")?;
    let decisions = parse_colon_usizes(fields.next()?, 2, fields.line_number)?;
    fields.expect("staffHeadRows")?;
    let staff_head_rows = fields.usize()?;
    fields.expect("beamInputRows")?;
    let beam_input_rows = fields.usize()?;
    fields.expect("beamChecks")?;
    let beam_check_rows = fields.usize()?;
    fields.expect("beamDecisionRows")?;
    let beam_decision_rows = fields.usize()?;
    fields.expect("beamPurgeRows")?;
    let beam_purge_rows = fields.usize()?;
    fields.expect("headPurgeRows")?;
    let head_purge_rows = fields.usize()?;
    fields.expect("finalHeadRows")?;
    let final_head_rows = fields.usize()?;
    fields.expect("scaleInputRows")?;
    let scale_input_rows = fields.usize()?;
    fields.expect("scaleRows")?;
    let scale_rows = fields.usize()?;
    fields.expect("emittedBodySha256")?;
    let emitted_body_sha256 = fields.sha256()?;
    fields.finish()?;
    Ok(PostRangeCorpusSummary {
        pages,
        systems,
        staffs,
        inputs: totals[0],
        duplicates: totals[1],
        overlaps: totals[2],
        staff_heads: totals[3],
        purged_beams: totals[4],
        purged_heads: totals[5],
        final_heads: totals[6],
        duplicate_decision_rows: decisions[0],
        overlap_decision_rows: decisions[1],
        staff_head_rows,
        beam_input_rows,
        beam_check_rows,
        beam_decision_rows,
        beam_purge_rows,
        head_purge_rows,
        final_head_rows,
        scale_input_rows,
        scale_rows,
        emitted_body_sha256,
    })
}

fn parse_inter(fields: &mut Fields<'_>) -> Result<PostRangeInter, HeadsPostRangeParseError> {
    fields.expect("class")?;
    let class_name = fields.next()?.to_owned();
    fields.expect("shape")?;
    let shape = fields.shape()?;
    fields.expect("bounds")?;
    let bounds = fields.bounds()?;
    fields.expect("grade")?;
    let grade = fields.double()?;
    fields.expect("best")?;
    let best_grade = fields.double()?;
    fields.expect("contextual")?;
    let contextual_grade = fields.nullable_double()?;
    fields.expect("good")?;
    let good = fields.boolean()?;
    fields.expect("removed")?;
    let removed = fields.boolean()?;
    fields.expect("impacts")?;
    let impacts = fields.impacts()?;
    fields.expect("glyph")?;
    let glyph = fields.glyph()?;
    let mut beam_width = None;
    let mut mean_height = None;
    if fields.peek() == Some("beamWidth") {
        fields.next()?;
        beam_width = Some(fields.i32()?);
        fields.expect("meanHeight")?;
        mean_height = Some(fields.double()?);
    }
    Ok(PostRangeInter {
        class_name,
        shape,
        bounds,
        grade,
        best_grade,
        contextual_grade,
        good,
        removed,
        impacts,
        glyph,
        beam_width,
        mean_height,
    })
}

fn finish_page(
    build: &mut PageBuild,
    summary: PostRangePageSummary,
) -> Result<PostRangePage, HeadsPostRangeParseError> {
    let mut staffs = Vec::with_capacity(build.staffs.len());
    for ((system_id, staff_id), staff) in std::mem::take(&mut build.staffs) {
        let summary = staff.summary.ok_or_else(|| {
            HeadsPostRangeParseError::new(format!(
                "{} system {system_id} staff {staff_id}: missing summary",
                build.page
            ))
        })?;
        staffs.push(PostRangeStaff {
            system_id,
            staff_id,
            duplicate_decisions: staff.duplicate_decisions,
            overlap_decisions: staff.overlap_decisions,
            attachment_heads: staff.attachment_heads,
            summary,
        });
    }
    let mut systems = Vec::with_capacity(build.systems.len());
    for (system_id, system) in std::mem::take(&mut build.systems) {
        let summary = system.summary.ok_or_else(|| {
            HeadsPostRangeParseError::new(format!(
                "{} system {system_id}: missing summary",
                build.page
            ))
        })?;
        systems.push(PostRangeSystem {
            system_id,
            beam_inputs: system.beam_inputs,
            beam_decisions: system.beam_decisions,
            purged_heads: system.purged_heads,
            final_heads: system.final_heads,
            summary,
        });
    }
    let page = PostRangePage {
        page: build.page.clone(),
        declared_systems: build.declared_systems,
        declared_staves: build.declared_staves,
        family: build.family.clone(),
        staffs,
        systems,
        scale_inputs: std::mem::take(&mut build.scale_inputs),
        scales: std::mem::take(&mut build.scales),
        summary,
    };
    validate_page(&page)?;
    Ok(page)
}

fn validate_page(page: &PostRangePage) -> Result<(), HeadsPostRangeParseError> {
    let fail = |message: String| HeadsPostRangeParseError::new(format!("{}: {message}", page.page));
    if page.staffs.len() != page.declared_staves || page.summary.staffs != page.declared_staves {
        return Err(fail(format!(
            "staff declarations/summaries differ: {} / {} / {}",
            page.declared_staves,
            page.staffs.len(),
            page.summary.staffs
        )));
    }
    if page.systems.len() != page.declared_systems {
        return Err(fail(format!(
            "system declarations/summaries differ: {} / {}",
            page.declared_systems,
            page.systems.len()
        )));
    }

    for staff in &page.staffs {
        let label = format!("system {} staff {}", staff.system_id, staff.staff_id);
        let summary = &staff.summary;
        if summary.inputs != summary.seed_inputs + summary.range_inputs
            || summary.after_duplicates + summary.duplicates != summary.inputs
            || summary.retained != summary.after_duplicates
            || summary.retained != staff.attachment_heads.len()
            || summary.duplicates != staff.duplicate_decisions.len()
            || summary.overlaps != staff.overlap_decisions.len()
            || summary.notes_before != summary.retained
            || summary.notes_after != summary.retained
        {
            return Err(fail(format!("{label}: inconsistent staff counts")));
        }
        validate_decisions(&staff.duplicate_decisions, &label)?;
        validate_decisions(&staff.overlap_decisions, &label)?;
        if fnv_rows(
            staff
                .duplicate_decisions
                .iter()
                .map(|decision| decision.canonical_row.as_str()),
        ) != summary.duplicate_hash
        {
            return Err(fail(format!("{label}: duplicate-decision hash mismatch")));
        }
        if fnv_rows(
            staff
                .overlap_decisions
                .iter()
                .map(|decision| decision.canonical_row.as_str()),
        ) != summary.overlap_hash
        {
            return Err(fail(format!("{label}: overlap-decision hash mismatch")));
        }
        if fnv_rows(
            staff
                .attachment_heads
                .iter()
                .map(|head| head.canonical_row.as_str()),
        ) != summary.staff_head_hash
        {
            return Err(fail(format!("{label}: staff-head hash mismatch")));
        }
        let mut prior = None;
        for head in &staff.attachment_heads {
            if prior.is_some_and(|prior| head.input_ordinal <= prior) {
                return Err(fail(format!("{label}: attachment inputs are not ordered")));
            }
            prior = Some(head.input_ordinal);
            if head.input_ordinal >= summary.inputs
                || head.inter.class_name != "HeadInter"
                || head.inter.removed
                || head.grade_after.bits != head.inter.grade.bits
            {
                return Err(fail(format!("{label}: invalid attachment head")));
            }
        }
        if reconstructed_tally_hash(page, staff) != summary.tally_hash {
            return Err(fail(format!("{label}: reconstructed tally hash mismatch")));
        }
    }

    for system in &page.systems {
        let label = format!("system {}", system.system_id);
        let staffs = page
            .staffs
            .iter()
            .filter(|staff| staff.system_id == system.system_id)
            .collect::<Vec<_>>();
        let sum = |select: fn(&PostRangeStaffSummary) -> usize| {
            staffs
                .iter()
                .map(|staff| select(&staff.summary))
                .sum::<usize>()
        };
        if system.summary.inputs != sum(|summary| summary.inputs)
            || system.summary.duplicates != sum(|summary| summary.duplicates)
            || system.summary.overlaps != sum(|summary| summary.overlaps)
            || system.summary.staff_heads != sum(|summary| summary.retained)
            || system.summary.beam_inputs != system.beam_inputs.len()
            || system.summary.purged_heads != system.beam_decisions.len()
            || system.summary.purged_heads != system.purged_heads.len()
            || system.summary.final_heads != system.final_heads.len()
            || system.summary.staff_heads
                != system.summary.purged_heads + system.summary.final_heads
        {
            return Err(fail(format!("{label}: inconsistent system counts")));
        }
        for (ordinal, head) in system.final_heads.iter().enumerate() {
            if head.ordinal != ordinal || head.inter.removed {
                return Err(fail(format!("{label}: invalid final-head order/state")));
            }
        }
        for (ordinal, beam) in system.beam_inputs.iter().enumerate() {
            if beam.ordinal != ordinal
                || !matches!(
                    beam.inter.shape,
                    PostRangeShape::Beam | PostRangeShape::BeamHook
                )
                || beam.inter.removed
                || beam.inter.beam_width != Some(beam.inter.bounds.width)
                || beam.inter.mean_height.is_none()
            {
                return Err(fail(format!("{label}: invalid beam-input order/state")));
            }
        }
        for (ordinal, decision) in system.beam_decisions.iter().enumerate() {
            if decision.ordinal != ordinal
                || decision.beam_ordinal >= system.beam_inputs.len()
                || decision.purged != PostRangeBeamPurgeTarget::Head
                || system
                    .purged_heads
                    .get(ordinal)
                    .is_none_or(|head| head.head_ordinal != decision.head_ordinal)
            {
                return Err(fail(format!(
                    "{label}: invalid beam-decision order/reference"
                )));
            }
        }
        if system.purged_heads.iter().any(|head| !head.inter.removed) {
            return Err(fail(format!("{label}: purged head is not removed")));
        }
        if fnv_rows(
            system
                .beam_inputs
                .iter()
                .map(|beam| beam.canonical_row.as_str()),
        ) != system.summary.beam_input_hash
        {
            return Err(fail(format!("{label}: beam-input hash mismatch")));
        }
        if fnv_rows(
            system
                .beam_decisions
                .iter()
                .map(|decision| decision.canonical_row.as_str())
                .chain(
                    system
                        .purged_heads
                        .iter()
                        .map(|head| head.canonical_row.as_str()),
                ),
        ) != system.summary.beam_decision_hash
        {
            return Err(fail(format!("{label}: beam-decision hash mismatch")));
        }
        if fnv_rows(
            system
                .final_heads
                .iter()
                .map(|head| head.canonical_row.as_str()),
        ) != system.summary.final_head_hash
        {
            return Err(fail(format!("{label}: final-head hash mismatch")));
        }
        validate_system_head_multiset(page, system)?;
    }

    for (ordinal, input) in page.scale_inputs.iter().enumerate() {
        if input.ordinal != ordinal {
            return Err(fail("non-sequential scale-input ordinal".to_owned()));
        }
    }
    if page.summary.scale_inputs != page.scale_inputs.len()
        || fnv_rows(
            page.scale_inputs
                .iter()
                .map(|input| input.canonical_row.as_str()),
        ) != page.summary.scale_input_hash
        || fnv_rows(page.scales.iter().map(|scale| scale.canonical_row.as_str()))
            != page.summary.scale_hash
    {
        return Err(fail("scale stream count/hash mismatch".to_owned()));
    }

    let staff_sum = |select: fn(&PostRangeStaffSummary) -> usize| {
        page.staffs
            .iter()
            .map(|staff| select(&staff.summary))
            .sum::<usize>()
    };
    let system_sum = |select: fn(&PostRangeSystemSummary) -> usize| {
        page.systems
            .iter()
            .map(|system| select(&system.summary))
            .sum::<usize>()
    };
    if page.summary.inputs != staff_sum(|summary| summary.inputs)
        || page.summary.duplicates != staff_sum(|summary| summary.duplicates)
        || page.summary.overlaps != staff_sum(|summary| summary.overlaps)
        || page.summary.staff_heads != staff_sum(|summary| summary.retained)
        || page.summary.purged_beams != system_sum(|summary| summary.purged_beams)
        || page.summary.purged_heads != system_sum(|summary| summary.purged_heads)
        || page.summary.final_heads != system_sum(|summary| summary.final_heads)
    {
        return Err(fail("page summary does not equal child totals".to_owned()));
    }
    Ok(())
}

fn validate_decisions(
    decisions: &[PostRangePurgeDecision],
    label: &str,
) -> Result<(), HeadsPostRangeParseError> {
    for (ordinal, decision) in decisions.iter().enumerate() {
        if decision.ordinal != ordinal {
            return Err(HeadsPostRangeParseError::new(format!(
                "{label}: non-sequential purge decision"
            )));
        }
        let pair = [decision.left.input_ordinal, decision.right.input_ordinal];
        if !pair.contains(&decision.purged_input)
            || !pair.contains(&decision.kept_input)
            || decision.purged_input == decision.kept_input
        {
            return Err(HeadsPostRangeParseError::new(format!(
                "{label}: purge decision selects outside its pair"
            )));
        }
    }
    Ok(())
}

fn reconstructed_tally_hash(page: &PostRangePage, staff: &PostRangeStaff) -> u64 {
    let mut rows = Vec::new();
    for head in &staff.attachment_heads {
        for (side, dx) in [
            ("LEFT", head.tally.left.as_ref()),
            ("RIGHT", head.tally.right.as_ref()),
        ] {
            if let Some(dx) = dx {
                rows.push(format!(
                    "headposttally {} system {} staff {} input {} side {} dx {}",
                    page.page,
                    staff.system_id,
                    staff.staff_id,
                    head.input_ordinal,
                    side,
                    dx.canonical()
                ));
            }
        }
    }
    fnv_rows(rows.iter().map(String::as_str))
}

fn validate_system_head_multiset(
    page: &PostRangePage,
    system: &PostRangeSystem,
) -> Result<(), HeadsPostRangeParseError> {
    let mut counts = BTreeMap::<PostRangeHeadKey, usize>::new();
    for staff in page
        .staffs
        .iter()
        .filter(|staff| staff.system_id == system.system_id)
    {
        for head in &staff.attachment_heads {
            *counts.entry(head.inter.head_key()).or_default() += 1;
        }
    }
    for key in system
        .purged_heads
        .iter()
        .map(|head| head.inter.head_key())
        .chain(system.final_heads.iter().map(|head| head.inter.head_key()))
    {
        let Some(count) = counts.get_mut(&key) else {
            return Err(HeadsPostRangeParseError::new(format!(
                "{} system {}: output head absent from staff attachments",
                page.page, system.system_id
            )));
        };
        if *count == 0 {
            return Err(HeadsPostRangeParseError::new(format!(
                "{} system {}: output head multiplicity exceeds attachments",
                page.page, system.system_id
            )));
        }
        *count -= 1;
    }
    if counts.values().any(|count| *count != 0) {
        return Err(HeadsPostRangeParseError::new(format!(
            "{} system {}: staff attachment absent from final/purged streams",
            page.page, system.system_id
        )));
    }
    Ok(())
}

fn validate_corpus(
    pages: &[PostRangePage],
    summary: &PostRangeCorpusSummary,
) -> Result<(), HeadsPostRangeParseError> {
    let page_sum = |select: fn(&PostRangePageSummary) -> usize| {
        pages
            .iter()
            .map(|page| select(&page.summary))
            .sum::<usize>()
    };
    let systems = pages.iter().map(|page| page.systems.len()).sum::<usize>();
    let staffs = pages.iter().map(|page| page.staffs.len()).sum::<usize>();
    let scale_rows = pages.iter().map(|page| page.scales.len()).sum::<usize>();
    if summary.pages != pages.len()
        || summary.systems != systems
        || summary.staffs != staffs
        || summary.inputs != page_sum(|page| page.inputs)
        || summary.duplicates != page_sum(|page| page.duplicates)
        || summary.overlaps != page_sum(|page| page.overlaps)
        || summary.staff_heads != page_sum(|page| page.staff_heads)
        || summary.purged_beams != page_sum(|page| page.purged_beams)
        || summary.purged_heads != page_sum(|page| page.purged_heads)
        || summary.final_heads != page_sum(|page| page.final_heads)
        || summary.duplicate_decision_rows != summary.duplicates
        || summary.overlap_decision_rows != summary.overlaps
        || summary.staff_head_rows != summary.staff_heads
        || summary.beam_input_rows
            != pages
                .iter()
                .flat_map(|page| &page.systems)
                .map(|system| system.beam_inputs.len())
                .sum::<usize>()
        || summary.beam_check_rows != 0
        || summary.beam_decision_rows != summary.purged_beams + summary.purged_heads
        || summary.beam_purge_rows != summary.purged_beams
        || summary.head_purge_rows != summary.purged_heads
        || summary.final_head_rows != summary.final_heads
        || summary.scale_input_rows != page_sum(|page| page.scale_inputs)
        || summary.scale_rows != scale_rows
    {
        return Err(HeadsPostRangeParseError::new(
            "corpus summary does not equal parsed page totals",
        ));
    }
    Ok(())
}

fn fnv_rows<'a>(rows: impl IntoIterator<Item = &'a str>) -> u64 {
    let mut hash = FNV_OFFSET;
    for row in rows {
        for byte in row.bytes().chain([b'\n']) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

struct Fields<'a> {
    values: Vec<&'a str>,
    index: usize,
    line_number: usize,
}

impl<'a> Fields<'a> {
    fn new(line: &'a str, line_number: usize) -> Self {
        Self {
            values: line.split_whitespace().collect(),
            index: 0,
            line_number,
        }
    }

    fn error(&self, message: impl fmt::Display) -> HeadsPostRangeParseError {
        HeadsPostRangeParseError::at(self.line_number, message)
    }

    fn next(&mut self) -> Result<&'a str, HeadsPostRangeParseError> {
        let value = self
            .values
            .get(self.index)
            .copied()
            .ok_or_else(|| self.error("unexpected end of row"))?;
        self.index += 1;
        Ok(value)
    }

    fn peek(&self) -> Option<&'a str> {
        self.values.get(self.index).copied()
    }

    fn expect(&mut self, expected: &str) -> Result<(), HeadsPostRangeParseError> {
        let actual = self.next()?;
        if actual != expected {
            return Err(self.error(format_args!("expected field {expected}, found {actual}")));
        }
        Ok(())
    }

    fn finish(&self) -> Result<(), HeadsPostRangeParseError> {
        if self.index != self.values.len() {
            return Err(self.error(format_args!(
                "unexpected trailing field {}",
                self.values[self.index]
            )));
        }
        Ok(())
    }

    fn usize(&mut self) -> Result<usize, HeadsPostRangeParseError> {
        let value = self.next()?;
        value
            .parse()
            .map_err(|_| self.error(format_args!("invalid usize {value}")))
    }

    fn i32(&mut self) -> Result<i32, HeadsPostRangeParseError> {
        let value = self.next()?;
        value
            .parse()
            .map_err(|_| self.error(format_args!("invalid i32 {value}")))
    }

    fn boolean(&mut self) -> Result<bool, HeadsPostRangeParseError> {
        let value = self.next()?;
        match value {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(self.error(format_args!("invalid boolean {value}"))),
        }
    }

    fn hash(&mut self) -> Result<u64, HeadsPostRangeParseError> {
        let value = self.next()?;
        parse_hash(value, self.line_number)
    }

    fn sha256(&mut self) -> Result<[u8; 32], HeadsPostRangeParseError> {
        let value = self.next()?;
        if value.len() != 64 {
            return Err(self.error(format_args!("invalid SHA-256 {value}")));
        }
        let mut output = [0_u8; 32];
        for (index, byte) in output.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
                .map_err(|_| self.error(format_args!("invalid SHA-256 {value}")))?;
        }
        Ok(output)
    }

    fn double(&mut self) -> Result<PostRangeDouble, HeadsPostRangeParseError> {
        parse_double(self.next()?, self.line_number)
    }

    fn nullable_double(&mut self) -> Result<Option<PostRangeDouble>, HeadsPostRangeParseError> {
        let value = self.next()?;
        if value == "-" {
            Ok(None)
        } else {
            parse_double(value, self.line_number).map(Some)
        }
    }

    fn bounds(&mut self) -> Result<PostRangeBounds, HeadsPostRangeParseError> {
        parse_bounds(self.next()?, self.line_number)
    }

    fn shape(&mut self) -> Result<PostRangeShape, HeadsPostRangeParseError> {
        parse_shape(self.next()?, self.line_number)
    }

    fn side(&mut self) -> Result<PostRangeSide, HeadsPostRangeParseError> {
        let value = self.next()?;
        match value {
            "LEFT" => Ok(PostRangeSide::Left),
            "RIGHT" => Ok(PostRangeSide::Right),
            _ => Err(self.error(format_args!("invalid horizontal side {value}"))),
        }
    }

    fn tally(&mut self) -> Result<PostRangeTally, HeadsPostRangeParseError> {
        parse_tally(self.next()?, self.line_number)
    }

    fn count_hash(&mut self) -> Result<PostRangeCountHash, HeadsPostRangeParseError> {
        let value = self.next()?;
        let (count, hash) = value
            .split_once(':')
            .ok_or_else(|| self.error(format_args!("invalid count:hash {value}")))?;
        let count = count
            .parse()
            .map_err(|_| self.error(format_args!("invalid count:hash {value}")))?;
        Ok(PostRangeCountHash {
            count,
            hash: parse_hash(hash, self.line_number)?,
        })
    }

    fn impacts(&mut self) -> Result<Vec<PostRangeImpact>, HeadsPostRangeParseError> {
        let value = self.next()?;
        if value == "-" {
            return Ok(Vec::new());
        }
        value
            .split(',')
            .map(|impact| {
                let pieces = impact.split(':').collect::<Vec<_>>();
                if pieces.len() != 3 {
                    return Err(self.error(format_args!("invalid impact {impact}")));
                }
                Ok(PostRangeImpact {
                    name: pieces[0].to_owned(),
                    value: parse_double(pieces[1], self.line_number)?,
                    weight: parse_double(pieces[2], self.line_number)?,
                })
            })
            .collect()
    }

    fn glyph(&mut self) -> Result<Option<PostRangeGlyph>, HeadsPostRangeParseError> {
        let value = self.next()?;
        if value == "-" {
            return Ok(None);
        }
        let pieces = value.split(':').collect::<Vec<_>>();
        if pieces.len() != 9
            || pieces[0] != "bounds"
            || pieces[5] != "weight"
            || pieces[7] != "runs"
        {
            return Err(self.error(format_args!("invalid glyph {value}")));
        }
        let bounds = parse_bounds(&pieces[1..5].join(":"), self.line_number)?;
        let weight = pieces[6]
            .parse()
            .map_err(|_| self.error(format_args!("invalid glyph weight {value}")))?;
        let run_hash = parse_hash(pieces[8], self.line_number)?;
        Ok(Some(PostRangeGlyph {
            bounds,
            weight,
            run_hash,
        }))
    }
}

fn parse_hash(value: &str, line_number: usize) -> Result<u64, HeadsPostRangeParseError> {
    if value.len() != 16 {
        return Err(HeadsPostRangeParseError::at(
            line_number,
            format_args!("invalid 64-bit hash {value}"),
        ));
    }
    u64::from_str_radix(value, 16).map_err(|_| {
        HeadsPostRangeParseError::at(line_number, format_args!("invalid 64-bit hash {value}"))
    })
}

fn parse_double(
    value: &str,
    line_number: usize,
) -> Result<PostRangeDouble, HeadsPostRangeParseError> {
    let (java_hex, raw) = value.split_once('/').ok_or_else(|| {
        HeadsPostRangeParseError::at(line_number, format_args!("invalid double {value}"))
    })?;
    if !(java_hex.contains("0x") || java_hex == "NaN" || java_hex.ends_with("Infinity")) {
        return Err(HeadsPostRangeParseError::at(
            line_number,
            format_args!("invalid Java double rendering {value}"),
        ));
    }
    Ok(PostRangeDouble {
        java_hex: java_hex.to_owned(),
        bits: parse_hash(raw, line_number)?,
    })
}

fn parse_bounds(
    value: &str,
    line_number: usize,
) -> Result<PostRangeBounds, HeadsPostRangeParseError> {
    let values = value
        .split(':')
        .map(|piece| {
            piece.parse::<i32>().map_err(|_| {
                HeadsPostRangeParseError::at(line_number, format_args!("invalid bounds {value}"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() != 4 {
        return Err(HeadsPostRangeParseError::at(
            line_number,
            format_args!("invalid bounds {value}"),
        ));
    }
    Ok(PostRangeBounds {
        x: values[0],
        y: values[1],
        width: values[2],
        height: values[3],
    })
}

fn parse_shape(
    value: &str,
    line_number: usize,
) -> Result<PostRangeShape, HeadsPostRangeParseError> {
    match value {
        "NOTEHEAD_VOID" => Ok(PostRangeShape::NoteheadVoid),
        "NOTEHEAD_BLACK" => Ok(PostRangeShape::NoteheadBlack),
        "BEAM" => Ok(PostRangeShape::Beam),
        "BEAM_HOOK" => Ok(PostRangeShape::BeamHook),
        _ => Err(HeadsPostRangeParseError::at(
            line_number,
            format_args!("unsupported post-range shape {value}"),
        )),
    }
}

fn parse_origin(
    value: &str,
    line_number: usize,
) -> Result<PostRangeOrigin, HeadsPostRangeParseError> {
    let (kind, ordinal) = value.split_once(':').ok_or_else(|| {
        HeadsPostRangeParseError::at(line_number, format_args!("invalid origin {value}"))
    })?;
    let ordinal = ordinal.parse().map_err(|_| {
        HeadsPostRangeParseError::at(line_number, format_args!("invalid origin {value}"))
    })?;
    match kind {
        "seed" => Ok(PostRangeOrigin::Seed(ordinal)),
        "range" => Ok(PostRangeOrigin::Range(ordinal)),
        _ => Err(HeadsPostRangeParseError::at(
            line_number,
            format_args!("invalid origin {value}"),
        )),
    }
}

fn parse_input_ref(
    value: &str,
    line_number: usize,
) -> Result<PostRangeInputRef, HeadsPostRangeParseError> {
    let mut pieces = value.splitn(2, ':');
    let input_ordinal = pieces
        .next()
        .and_then(|piece| piece.parse().ok())
        .ok_or_else(|| {
            HeadsPostRangeParseError::at(
                line_number,
                format_args!("invalid input reference {value}"),
            )
        })?;
    let origin = pieces.next().ok_or_else(|| {
        HeadsPostRangeParseError::at(line_number, format_args!("invalid input reference {value}"))
    })?;
    Ok(PostRangeInputRef {
        input_ordinal,
        origin: parse_origin(origin, line_number)?,
    })
}

fn parse_tally(
    value: &str,
    line_number: usize,
) -> Result<PostRangeTally, HeadsPostRangeParseError> {
    let (left, right) = value.split_once(':').ok_or_else(|| {
        HeadsPostRangeParseError::at(line_number, format_args!("invalid tally {value}"))
    })?;
    let parse = |value: &str| {
        if value == "-" {
            Ok(None)
        } else {
            parse_double(value, line_number).map(Some)
        }
    };
    Ok(PostRangeTally {
        left: parse(left)?,
        right: parse(right)?,
    })
}

fn parse_choice(
    value: &str,
    line_number: usize,
) -> Result<PostRangePurgeChoice, HeadsPostRangeParseError> {
    match value {
        "lower-grade-left" => Ok(PostRangePurgeChoice::LowerGradeLeft),
        "lower-grade-right" => Ok(PostRangePurgeChoice::LowerGradeRight),
        "equal-left-no-seed" => Ok(PostRangePurgeChoice::EqualLeftNoSeed),
        "equal-right-no-seed" => Ok(PostRangePurgeChoice::EqualRightNoSeed),
        "equal-shape" => Ok(PostRangePurgeChoice::EqualShape),
        "equal-bounds" => Ok(PostRangePurgeChoice::EqualBounds),
        "equal-preserve-left" => Ok(PostRangePurgeChoice::EqualPreserveLeft),
        _ => Err(HeadsPostRangeParseError::at(
            line_number,
            format_args!("invalid purge choice {value}"),
        )),
    }
}

fn parse_colon_usizes(
    value: &str,
    expected: usize,
    line_number: usize,
) -> Result<Vec<usize>, HeadsPostRangeParseError> {
    let values = value
        .split(':')
        .map(|piece| {
            piece.parse().map_err(|_| {
                HeadsPostRangeParseError::at(
                    line_number,
                    format_args!("invalid colon-separated counts {value}"),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() != expected {
        return Err(HeadsPostRangeParseError::at(
            line_number,
            format_args!("invalid colon-separated counts {value}"),
        ));
    }
    Ok(values)
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

// Small dependency-free SHA-256 used only for the oracle's self-commitment.
fn sha256(input: &[u8]) -> [u8; 32] {
    const INITIAL: [u32; 8] = [
        0x6a09_e667,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    const K: [u32; 64] = [
        0x428a_2f98,
        0x7137_4491,
        0xb5c0_fbcf,
        0xe9b5_dba5,
        0x3956_c25b,
        0x59f1_11f1,
        0x923f_82a4,
        0xab1c_5ed5,
        0xd807_aa98,
        0x1283_5b01,
        0x2431_85be,
        0x550c_7dc3,
        0x72be_5d74,
        0x80de_b1fe,
        0x9bdc_06a7,
        0xc19b_f174,
        0xe49b_69c1,
        0xefbe_4786,
        0x0fc1_9dc6,
        0x240c_a1cc,
        0x2de9_2c6f,
        0x4a74_84aa,
        0x5cb0_a9dc,
        0x76f9_88da,
        0x983e_5152,
        0xa831_c66d,
        0xb003_27c8,
        0xbf59_7fc7,
        0xc6e0_0bf3,
        0xd5a7_9147,
        0x06ca_6351,
        0x1429_2967,
        0x27b7_0a85,
        0x2e1b_2138,
        0x4d2c_6dfc,
        0x5338_0d13,
        0x650a_7354,
        0x766a_0abb,
        0x81c2_c92e,
        0x9272_2c85,
        0xa2bf_e8a1,
        0xa81a_664b,
        0xc24b_8b70,
        0xc76c_51a3,
        0xd192_e819,
        0xd699_0624,
        0xf40e_3585,
        0x106a_a070,
        0x19a4_c116,
        0x1e37_6c08,
        0x2748_774c,
        0x34b0_bcb5,
        0x391c_0cb3,
        0x4ed8_aa4a,
        0x5b9c_ca4f,
        0x682e_6ff3,
        0x748f_82ee,
        0x78a5_636f,
        0x84c8_7814,
        0x8cc7_0208,
        0x90be_fffa,
        0xa450_6ceb,
        0xbef9_a3f7,
        0xc671_78f2,
    ];
    let bit_length = (input.len() as u64).wrapping_mul(8);
    let mut padded = input.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_length.to_be_bytes());
    let mut state = INITIAL;
    for chunk in padded.chunks_exact(64) {
        let mut words = [0_u32; 64];
        for (index, word) in words[..16].iter_mut().enumerate() {
            *word = u32::from_be_bytes(chunk[index * 4..index * 4 + 4].try_into().unwrap());
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temporary1 = h
                .wrapping_add(sum1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temporary2 = sum0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temporary1);
            d = c;
            c = b;
            b = a;
            a = temporary1.wrapping_add(temporary2);
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
    let mut digest = [0_u8; 32];
    for (index, word) in state.iter().enumerate() {
        digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_standard_empty_and_abc_vectors() {
        assert_eq!(
            hex_bytes(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex_bytes(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
