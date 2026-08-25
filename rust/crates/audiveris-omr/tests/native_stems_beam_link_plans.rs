// SPDX-License-Identifier: AGPL-3.0-or-later

//! Fail-closed differential gate for the pure beam-origin
//! `VLinker.expand`/`VLinker.link` prefix.
//!
//! The active release test pins the two-pass eight-page Java freeze, validates
//! manifest/body/fixture/trailer algebra, builds the complete native
//! predecessor chain, and independently projects every semantic row. The
//! parser rejects every unknown family, missing/reordered label, malformed
//! hierarchy, non-contiguous lifecycle ordinal, and forbidden mutation.

use std::{collections::BTreeMap, fmt::Write as _, path::PathBuf};

use audiveris_image::{
    beam_structure::Segment,
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable},
    section::Bounds,
};

use audiveris_omr::{
    beam_recognizer::run_table_center_line,
    head_scanner_slices::JavaRectangle,
    head_template::HeadTemplateShape,
    native_headers::recognize_native_headers,
    native_heads::{NativeHeadsRecognition, recognize_native_heads},
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::{NativeStemSeedRecognition, recognize_native_stem_seeds},
    native_stems_beam_builders::{
        NativeStemsBeamBuilder, NativeStemsBeamBuilderGlyphRef, NativeStemsBeamBuilderItem,
        NativeStemsBeamBuilderItemKind, NativeStemsBeamBuilderRecognition,
        NativeStemsBeamBuilderTargetRef, materialize_native_stems_beam_builders,
    },
    native_stems_beam_link_plans::{
        NativeStemsBeamExpandStep, NativeStemsBeamExpandStopCause, NativeStemsBeamGapAction,
        NativeStemsBeamGlyphUpdate, NativeStemsBeamHeadRelationCheck,
        NativeStemsBeamHorizontalGapKind, NativeStemsBeamLinkGlyphKey,
        NativeStemsBeamLinkPlanAttempt, NativeStemsBeamLinkPlanOutcome,
        NativeStemsBeamLinkPlanRecognition, NativeStemsBeamSelectedGlyph,
        NativeStemsBeamStemPortion, materialize_native_stems_beam_link_plans,
    },
    native_stems_beam_reachability::{
        NativeStemsBeamHeadCornerRef, NativeStemsBeamReachabilityRecognition,
        materialize_native_stems_beam_reachability,
    },
    native_stems_beam_stumps::{
        NativeStemsBeamGlyph, NativeStemsBeamSource, NativeStemsBeamStumpBeam,
        NativeStemsBeamStumpRecognition, NativeStemsBeamStumpRef,
        materialize_native_stems_beam_stumps,
    },
    native_stems_beam_vlinkers::{
        NativeStemsBeamBLinker, NativeStemsBeamBLinkerRef, NativeStemsBeamVLinkerRecognition,
        materialize_native_stems_beam_vlinkers,
    },
    native_stems_head_builders::materialize_native_stems_head_builders,
    native_stems_head_corner_reachability::materialize_native_stems_head_corner_reachability,
    native_stems_head_corners::{
        NativeStemsHeadCornerHead, NativeStemsHeadCornerRecognition,
        materialize_native_stems_head_corners,
    },
    native_stems_head_seeds::materialize_native_stems_head_seeds,
    native_stems_head_stumps::{
        NativeStemsHeadStumpBuild, NativeStemsHeadStumpOutcome, NativeStemsHeadStumpRecognition,
        materialize_native_stems_head_stumps,
    },
    recognize::{
        GridLinesRecognition, NativeBeamRecognition, recognize_grid_lines,
        recognize_native_beams_with_stem_seeds,
    },
    stems_step::{NativeStemHeadSide, NativeStemLine, NativeStemPoint, NativeStemVerticalSide},
};

const SCHEMA_HEADER: &str = "# schema: stems-beam-expand-v1";
const MANIFEST_SCHEMA_HEADER: &str = "# schema: stems-beam-expand-manifest-v1";
const MANIFEST_PATH: &str = "rust/oracle/stems-beam-expand-manifest.txt";
const PROBE_PATH: &str = "rust/oracle/java/StemsBeamExpandProbe.java";
const RUNNER_PATH: &str = "rust/oracle/java/run-stems-beam-expand.sh";
const INSPECT_PROFILE: i32 = 1;

const EXPECTED_MANIFEST_SHA256: &str =
    "f511b049cf5e32de6fb0151a36a1385efb78b4965fd704c7545eaef8522a2f87";
const EXPECTED_PROBE_SHA256: &str =
    "2a5e107f947e140e030f3cc1dff06105ab730af3e41381e76f5f8113a17b0fa2";
const EXPECTED_RUNNER_SHA256: &str =
    "a73ed3977662427062b8d81ac8796ffa54d51daa2f97ea1f109a3d606d0c13b7";
const EXPECTED_FULL_BODY_SHA256: &str =
    "ac0fcb9880dbf720c8b73e6baf02867d05e0f2d5a62f208f52e9fa7d5c764966";
const EXPECTED_FULL_BODY_LINES: usize = 120_646;
const EXPECTED_FULL_BODY_BYTES: usize = 104_048_204;
const EXPECTED_FULL_ROW_COUNTS: [usize; 12] = [
    8, 30, 11_573, 578, 9_869, 18_416, 37_683, 18_345, 12_523, 11_573, 30, 8,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PageSpec {
    image: &'static str,
    page: &'static str,
    fixture: &'static str,
}

const PAGES: &[PageSpec] = &[
    PageSpec {
        image: "chula.png",
        page: "chula.png#1",
        fixture: "stems-beam-expand-chula.txt",
    },
    PageSpec {
        image: "allegretto.png",
        page: "allegretto.png#1",
        fixture: "stems-beam-expand-allegretto.txt",
    },
    PageSpec {
        image: "batuque.png",
        page: "batuque.png#1",
        fixture: "stems-beam-expand-batuque.txt",
    },
    PageSpec {
        image: "carmen.png",
        page: "carmen.png#1",
        fixture: "stems-beam-expand-carmen.txt",
    },
    PageSpec {
        image: "cucaracha.png",
        page: "cucaracha.png#1",
        fixture: "stems-beam-expand-cucaracha.txt",
    },
    PageSpec {
        image: "hove.png",
        page: "hove.png#1",
        fixture: "stems-beam-expand-hove.txt",
    },
    PageSpec {
        image: "zizi.png",
        page: "zizi.png#1",
        fixture: "stems-beam-expand-zizi.txt",
    },
    PageSpec {
        image: "BachInvention5.jpg",
        page: "BachInvention5.jpg#1",
        fixture: "stems-beam-expand-BachInvention5.txt",
    },
];

const MANIFEST_ENTRY_FIELDS: &[&str] = &[
    "ordinal",
    "page",
    "fixture",
    "pageHash",
    "rowCounts",
    "emittedBodySha256",
    "emittedBodyLines",
    "emittedBodyBytes",
    "fixtureSha256",
    "fixtureLines",
    "fixtureBytes",
];
const MANIFEST_SUMMARY_FIELDS: &[&str] = &[
    "schema",
    "entries",
    "probeSourceSha256",
    "runnerSourceSha256",
    "manifestBodySha256",
    "manifestBodyLines",
    "manifestBodyBytes",
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct ManifestEntry {
    ordinal: usize,
    page: String,
    fixture: String,
    page_hash: String,
    row_counts: [usize; 12],
    body_sha256: String,
    body_lines: usize,
    body_bytes: usize,
    fixture_sha256: String,
    fixture_lines: usize,
    fixture_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BeamExpandManifest {
    entries: Vec<ManifestEntry>,
    probe_sha256: String,
    runner_sha256: String,
    body_sha256: String,
    body_lines: usize,
    body_bytes: usize,
}

impl BeamExpandManifest {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
        if text.lines().next() != Some(MANIFEST_SCHEMA_HEADER) {
            return Err("beam-expand manifest schema header differs".to_owned());
        }
        let mut entries = Vec::new();
        let mut summary = None;
        for (offset, line) in text.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            match line.split_ascii_whitespace().next() {
                Some("stemsbeamexpandmanifestentry") => {
                    if summary.is_some() {
                        return Err(format!(
                            "manifest entry after summary at line {}",
                            offset + 1
                        ));
                    }
                    let values = parse_exact_labeled_row(
                        line,
                        "stemsbeamexpandmanifestentry",
                        MANIFEST_ENTRY_FIELDS,
                    )?;
                    entries.push(ManifestEntry {
                        ordinal: parse_usize(values[0], "manifest ordinal")?,
                        page: values[1].to_owned(),
                        fixture: values[2].to_owned(),
                        page_hash: parse_lower_hex(values[3], 16, "page hash")?.to_owned(),
                        row_counts: parse_row_counts(values[4])?,
                        body_sha256: parse_lower_hex(values[5], 64, "body SHA-256")?.to_owned(),
                        body_lines: parse_usize(values[6], "body lines")?,
                        body_bytes: parse_usize(values[7], "body bytes")?,
                        fixture_sha256: parse_lower_hex(values[8], 64, "fixture SHA-256")?
                            .to_owned(),
                        fixture_lines: parse_usize(values[9], "fixture lines")?,
                        fixture_bytes: parse_usize(values[10], "fixture bytes")?,
                    });
                }
                Some("stemsbeamexpandmanifestsummary") => {
                    if summary.is_some() {
                        return Err("duplicate manifest summary".to_owned());
                    }
                    let values = parse_exact_labeled_row(
                        line,
                        "stemsbeamexpandmanifestsummary",
                        MANIFEST_SUMMARY_FIELDS,
                    )?;
                    if values[0] != "stems-beam-expand-manifest-v1" {
                        return Err("manifest summary schema differs".to_owned());
                    }
                    summary = Some((
                        parse_usize(values[1], "manifest entries")?,
                        parse_lower_hex(values[2], 64, "probe SHA-256")?.to_owned(),
                        parse_lower_hex(values[3], 64, "runner SHA-256")?.to_owned(),
                        parse_lower_hex(values[4], 64, "manifest body SHA-256")?.to_owned(),
                        parse_usize(values[5], "manifest body lines")?,
                        parse_usize(values[6], "manifest body bytes")?,
                    ));
                }
                Some(family) => return Err(format!("unknown manifest family {family:?}")),
                None => unreachable!("nonempty manifest row"),
            }
        }
        let (entry_count, probe_sha256, runner_sha256, body_sha256, body_lines, body_bytes) =
            summary.ok_or_else(|| "missing manifest summary".to_owned())?;
        if entries.len() != entry_count || entries.len() != PAGES.len() {
            return Err(format!(
                "manifest entry count differs: rows {} summary {entry_count} corpus {}",
                entries.len(),
                PAGES.len(),
            ));
        }
        for (ordinal, (entry, spec)) in entries.iter().zip(PAGES).enumerate() {
            if entry.ordinal != ordinal || entry.page != spec.page || entry.fixture != spec.fixture
            {
                return Err(format!("manifest page order differs at {}", entry.ordinal));
            }
        }
        Ok(Self {
            entries,
            probe_sha256,
            runner_sha256,
            body_sha256,
            body_lines,
            body_bytes,
        })
    }
}

fn parse_exact_labeled_row<'a>(
    row: &'a str,
    family: &str,
    labels: &[&str],
) -> Result<Vec<&'a str>, String> {
    let tokens = row.split_ascii_whitespace().collect::<Vec<_>>();
    if tokens.first().copied() != Some(family) || tokens.len() != 1 + 2 * labels.len() {
        return Err(format!("malformed {family} row"));
    }
    let observed = tokens[1..]
        .chunks_exact(2)
        .map(|pair| pair[0])
        .collect::<Vec<_>>();
    if observed != labels {
        return Err(format!(
            "{family} labels differ: {observed:?} != {labels:?}"
        ));
    }
    Ok(tokens[1..].chunks_exact(2).map(|pair| pair[1]).collect())
}

fn parse_usize(value: &str, label: &str) -> Result<usize, String> {
    value
        .parse()
        .map_err(|error| format!("invalid {label} {value:?}: {error}"))
}

fn parse_lower_hex<'a>(value: &'a str, length: usize, label: &str) -> Result<&'a str, String> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!("invalid {label} {value:?}"));
    }
    Ok(value)
}

fn parse_row_counts(value: &str) -> Result<[usize; 12], String> {
    let counts = value
        .split(':')
        .map(|token| parse_usize(token, "row count"))
        .collect::<Result<Vec<_>, _>>()?;
    let length = counts.len();
    counts
        .try_into()
        .map_err(|_| format!("rowCounts needs 12 values, got {length}"))
}

const PAGE_FIELDS: &[&str] = &["systems", "staves", "family"];
const SYSTEM_FIELDS: &[&str] = &[
    "system",
    "profile",
    "stubProfile",
    "interline",
    "minLinkerLength",
    "beams",
    "heads",
    "beamSigOrder",
];
const PLAN_FIELDS: &[&str] = &[
    "system",
    "plan",
    "builder",
    "beamSig",
    "bOrdinal",
    "bAlias",
    "bId",
    "hSide",
    "vSide",
    "yDir",
    "constructionMax",
    "stemProfile",
    "linkProfile",
    "items",
    "maxIndex",
    "headTargets",
    "targetCount",
    "startStump",
    "theo",
    "initialMaxYGap",
    "standardMaxYGap",
    "minLinkerLength",
];
const GAP_FIELDS: &[&str] = &[
    "system",
    "plan",
    "item",
    "contrib",
    "maxYGap",
    "stoppingIndex",
    "action",
    "glyphsBefore",
    "restoredGlyphs",
    "relationsRetained",
    "stemLineRetained",
];
const SEPARATION_FIELDS: &[&str] = &[
    "system",
    "plan",
    "item",
    "candidate",
    "stoppingIndex",
    "gapIndex",
    "closeDy",
    "minLinkerLength",
    "underMin",
    "opposite",
    "oppositeLength",
    "oppositeConcrete",
    "action",
];
const RELATION_FIELDS: &[&str] = &[
    "system",
    "plan",
    "item",
    "cAlias",
    "headShape",
    "headBounds",
    "headCenter",
    "cHSide",
    "cVSide",
    "stemLine",
    "relativeCCW",
    "xDir",
    "relationHeadSide",
    "sideMismatch",
    "ref",
    "xStem",
    "xGapPixels",
    "yGapPixels",
    "dx",
    "dy",
    "xKind",
    "xMax",
    "yMax",
    "xImpactRaw",
    "yImpactRaw",
    "xImpact",
    "yImpact",
    "xWeight",
    "yWeight",
    "grade",
    "minGrade",
    "accepted",
    "actualHeadSide",
    "actualDx",
    "actualDy",
    "actualGrade",
    "impactNames",
    "impactValues",
    "impactWeights",
    "extension",
    "stoppingSide",
    "glyphsBefore",
    "stoppingEligible",
    "compositeLine",
    "stemPortion",
    "isEnd",
    "stoppingUpdate",
    "stoppingSnapshot",
    "mapAction",
    "mapOrdinal",
    "firstItem",
    "latestItem",
];
const UPDATE_FIELDS: &[&str] = &[
    "system",
    "plan",
    "item",
    "itemRef",
    "attempted",
    "canonicalBefore",
    "action",
    "glyphsBefore",
    "glyphsAfter",
    "lineBefore",
    "lineAfter",
    "compositeBounds",
    "compositeWeight",
    "compositeRuns",
    "compositeCentroid",
    "intersection",
    "lineShift",
];
const FINAL_RELATION_FIELDS: &[&str] = &[
    "system",
    "plan",
    "ordinal",
    "cAlias",
    "firstItem",
    "latestItem",
    "pastReturn",
    "headSide",
    "dx",
    "dy",
    "grade",
    "extension",
    "impacts",
];
const GLYPH_FIELDS: &[&str] = &[
    "system",
    "plan",
    "ordinal",
    "sourceItem",
    "sourceRef",
    "content",
    "bounds",
    "weight",
    "centroid",
    "centerLine",
];
const END_FIELDS: &[&str] = &[
    "system",
    "plan",
    "builder",
    "outcome",
    "expandInvoked",
    "lastIndex",
    "relationCount",
    "relations",
    "glyphCount",
    "glyphs",
    "stoppingIndex",
    "stopCause",
    "relationsPastReturn",
    "storedTheoBefore",
    "storedTheoAfter",
    "storedTheoMutated",
    "attachmentAliasesTheo",
    "builderAliasesTheo",
    "attachmentLineMutated",
    "storedTheoShiftDx",
    "terminalKind",
    "terminalC",
    "terminalRelationSide",
    "terminalPortion",
    "terminalCorrectSideEnd",
    "beamSideReadyWithoutStoppingHead",
    "finalStemLine",
    "restoredGlyphLine",
    "rollbackLineDiverges",
    "traceSha256",
    "visits",
    "gaps",
    "showStoppingGaps",
    "separationChecks",
    "separationStops",
    "relationAttempts",
    "relationAccepts",
    "relationRejects",
    "stoppingUpdates",
    "glyphUpdateCalls",
    "glyphInsertions",
    "glyphEqualSkips",
    "sigVertexDelta",
    "sigEdgeDelta",
    "stemInterDelta",
    "systemStemDelta",
    "glyphIndexDelta",
    "filamentIndexDelta",
    "linkMutations",
    "builderMutations",
];
const SYSTEM_SUMMARY_FIELDS: &[&str] = &[
    "system",
    "systems",
    "builders",
    "plans",
    "noHeadTarget",
    "expandFailed",
    "noRelations",
    "noGlyphs",
    "ready",
    "relations",
    "glyphs",
    "postReturnRelations",
    "plansWithPostReturnRelations",
    "rollbackLineDivergences",
    "relationSideMismatches",
    "storedTheoMutations",
    "attachmentLineMutations",
    "beamSideReady",
    "beamSideReadyWithoutStoppingHead",
    "beamSideReadyBeyondStoppingHead",
    "beamSideReadyAtStoppingHead",
    "maxAbsStoredTheoShift",
    "sigMutations",
    "systemStemMutations",
    "glyphIndexMutations",
    "filamentIndexMutations",
    "linkMutations",
    "builderMutations",
    "hash",
];
const PAGE_SUMMARY_FIELDS: &[&str] = &[
    // The first value is Sheet.getSystems().size(); the second starts Totals.fields().
    "systems",
    "systems",
    "builders",
    "plans",
    "noHeadTarget",
    "expandFailed",
    "noRelations",
    "noGlyphs",
    "ready",
    "relations",
    "glyphs",
    "postReturnRelations",
    "plansWithPostReturnRelations",
    "rollbackLineDivergences",
    "relationSideMismatches",
    "storedTheoMutations",
    "attachmentLineMutations",
    "beamSideReady",
    "beamSideReadyWithoutStoppingHead",
    "beamSideReadyBeyondStoppingHead",
    "beamSideReadyAtStoppingHead",
    "maxAbsStoredTheoShift",
    "sigMutations",
    "systemStemMutations",
    "glyphIndexMutations",
    "filamentIndexMutations",
    "linkMutations",
    "builderMutations",
    "hash",
];
const CORPUS_SUMMARY_FIELDS: &[&str] = &[
    "schema",
    "mode",
    "pages",
    "pageRefs",
    "rowCounts",
    "probeSourceSha256",
    "runnerSourceSha256",
    "emittedBodySha256",
    "emittedBodyLines",
    "emittedBodyBytes",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Family {
    Page,
    System,
    Plan,
    Gap,
    Separation,
    Relation,
    Update,
    FinalRelation,
    Glyph,
    End,
    SystemSummary,
    PageSummary,
    CorpusSummary,
}

impl Family {
    fn parse(token: &str) -> Result<Self, String> {
        match token {
            "stemsbeamexpandpage" => Ok(Self::Page),
            "stemsbeamexpandsystem" => Ok(Self::System),
            "stemsbeamexpandplan" => Ok(Self::Plan),
            "stemsbeamexpandgap" => Ok(Self::Gap),
            "stemsbeamexpandseparation" => Ok(Self::Separation),
            "stemsbeamexpandrelation" => Ok(Self::Relation),
            "stemsbeamexpandupdate" => Ok(Self::Update),
            "stemsbeamexpandfinalrelation" => Ok(Self::FinalRelation),
            "stemsbeamexpandglyph" => Ok(Self::Glyph),
            "stemsbeamexpandend" => Ok(Self::End),
            "stemsbeamexpandsystemsummary" => Ok(Self::SystemSummary),
            "stemsbeamexpandpagesummary" => Ok(Self::PageSummary),
            "stemsbeamexpandcorpussummary" => Ok(Self::CorpusSummary),
            _ => Err(format!("unsupported beam-expand row family {token:?}")),
        }
    }

    fn labels(self) -> Option<&'static [&'static str]> {
        match self {
            Self::Page => Some(PAGE_FIELDS),
            Self::System => Some(SYSTEM_FIELDS),
            Self::Plan => Some(PLAN_FIELDS),
            Self::Gap => Some(GAP_FIELDS),
            Self::Separation => Some(SEPARATION_FIELDS),
            Self::Relation => Some(RELATION_FIELDS),
            Self::Update => Some(UPDATE_FIELDS),
            Self::FinalRelation => Some(FINAL_RELATION_FIELDS),
            Self::Glyph => Some(GLYPH_FIELDS),
            Self::End => Some(END_FIELDS),
            Self::SystemSummary => Some(SYSTEM_SUMMARY_FIELDS),
            Self::PageSummary => Some(PAGE_SUMMARY_FIELDS),
            Self::CorpusSummary => Some(CORPUS_SUMMARY_FIELDS),
        }
    }

    fn row_count_index(self) -> Option<usize> {
        match self {
            Self::Page => Some(0),
            Self::System => Some(1),
            Self::Plan => Some(2),
            Self::Gap => Some(3),
            Self::Separation => Some(4),
            Self::Relation => Some(5),
            Self::Update => Some(6),
            Self::FinalRelation => Some(7),
            Self::Glyph => Some(8),
            Self::End => Some(9),
            Self::SystemSummary => Some(10),
            Self::PageSummary => Some(11),
            Self::CorpusSummary => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OracleRow<'a> {
    line_number: usize,
    raw: &'a str,
    family: Family,
    page: &'a str,
    fields: Vec<(&'a str, &'a str)>,
}

impl<'a> OracleRow<'a> {
    fn value(&self, label: &str) -> Result<&'a str, String> {
        self.fields
            .iter()
            .find_map(|&(candidate, value)| (candidate == label).then_some(value))
            .ok_or_else(|| format!("line {} lacks {label}", self.line_number))
    }

    fn number<T>(&self, label: &str) -> Result<T, String>
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Debug,
    {
        let value = self.value(label)?;
        value.parse().map_err(|error| {
            format!(
                "line {} has invalid {label} value {value:?}: {error:?}",
                self.line_number
            )
        })
    }
}

#[derive(Debug)]
struct OracleFixture<'a> {
    page: &'a str,
    rows: Vec<OracleRow<'a>>,
}

impl<'a> OracleFixture<'a> {
    fn parse(text: &'a str) -> Result<Self, String> {
        if !text.lines().any(|line| line == SCHEMA_HEADER) {
            return Err(format!("missing exact schema header {SCHEMA_HEADER:?}"));
        }
        let mut rows = Vec::new();
        for (offset, line) in text.lines().enumerate() {
            let line_number = offset + 1;
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            rows.push(parse_row(line, line_number)?);
        }
        let page = rows
            .first()
            .ok_or_else(|| "empty beam-expand fixture".to_owned())?
            .page;
        if rows.iter().any(|row| row.page != page) {
            return Err("split fixture mixes pages".to_owned());
        }
        validate_hierarchy(&rows)?;
        Ok(Self { page, rows })
    }
}

fn parse_row(line: &str, line_number: usize) -> Result<OracleRow<'_>, String> {
    let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
    if tokens.len() < 2 {
        return Err(format!("short oracle row at line {line_number}"));
    }
    let family = Family::parse(tokens[0])?;
    let (page, pairs) = if family == Family::CorpusSummary {
        if tokens.len() < 5 || tokens[1] != "schema" {
            return Err(format!("malformed corpus summary at line {line_number}"));
        }
        let page_index = tokens
            .iter()
            .position(|&token| token == "pageRefs")
            .ok_or_else(|| format!("corpus summary lacks pageRefs at line {line_number}"))?;
        let page = *tokens
            .get(page_index + 1)
            .ok_or_else(|| format!("corpus summary lacks page value at line {line_number}"))?;
        (page, &tokens[1..])
    } else {
        (tokens[1], &tokens[2..])
    };
    if pairs.len() % 2 != 0 {
        return Err(format!("odd label/value tail at line {line_number}"));
    }
    let fields = pairs
        .chunks_exact(2)
        .map(|pair| (pair[0], pair[1]))
        .collect::<Vec<_>>();
    let labels = fields.iter().map(|&(label, _)| label).collect::<Vec<_>>();
    let expected = family.labels().expect("known labels");
    if labels != expected {
        return Err(format!(
            "line {line_number} {family:?} labels differ: {labels:?} != {expected:?}"
        ));
    }
    Ok(OracleRow {
        line_number,
        raw: line,
        family,
        page,
        fields,
    })
}

fn validate_hierarchy(rows: &[OracleRow<'_>]) -> Result<(), String> {
    if rows.first().map(|row| row.family) != Some(Family::Page) {
        return Err("fixture does not begin with a page row".to_owned());
    }
    let mut current_system = None;
    let mut current_plan = None;
    let mut next_plan_by_system = BTreeMap::<usize, usize>::new();
    let mut next_final_relation = 0_usize;
    let mut next_glyph = 0_usize;
    // 0 = chronological trace, 1 = ordered glyphs, 2 = ordered final relations.
    let mut plan_phase = 0_u8;
    let mut saw_page_summary = false;
    let mut saw_corpus_summary = false;

    for row in &rows[1..] {
        if saw_corpus_summary {
            return Err(format!(
                "row after corpus summary at line {}",
                row.line_number
            ));
        }
        if saw_page_summary && row.family != Family::CorpusSummary {
            return Err(format!(
                "non-trailer row after page summary at line {}",
                row.line_number
            ));
        }
        match row.family {
            Family::Page => {
                return Err(format!(
                    "misplaced {:?} at line {}",
                    row.family, row.line_number
                ));
            }
            Family::CorpusSummary => {
                if !saw_page_summary || row.value("schema")? != "stems-beam-expand-v1" {
                    return Err(format!(
                        "misplaced/invalid corpus summary at line {}",
                        row.line_number
                    ));
                }
                saw_corpus_summary = true;
            }
            Family::System => {
                if current_system.is_some() || current_plan.is_some() {
                    return Err(format!("nested system at line {}", row.line_number));
                }
                current_system = Some(row.number("system")?);
            }
            Family::Plan => {
                let system: usize = row.number("system")?;
                if current_system != Some(system) || current_plan.is_some() {
                    return Err(format!("misnested plan at line {}", row.line_number));
                }
                let plan = row.number("plan")?;
                let expected = next_plan_by_system.entry(system).or_default();
                if plan != *expected {
                    return Err(format!(
                        "non-contiguous plan at line {}: {plan} != {expected}",
                        row.line_number
                    ));
                }
                *expected += 1;
                current_plan = Some(plan);
                next_final_relation = 0;
                next_glyph = 0;
                plan_phase = 0;
            }
            Family::Gap
            | Family::Separation
            | Family::Relation
            | Family::Update
            | Family::FinalRelation
            | Family::Glyph
            | Family::End => {
                let system: usize = row.number("system")?;
                let plan: usize = row.number("plan")?;
                if current_system != Some(system) || current_plan != Some(plan) {
                    return Err(format!(
                        "row escapes current plan at line {}",
                        row.line_number
                    ));
                }
                match row.family {
                    Family::FinalRelation => {
                        if plan_phase > 2 {
                            return Err(format!(
                                "final relation after plan end at line {}",
                                row.line_number
                            ));
                        }
                        plan_phase = 2;
                        let ordinal: usize = row.number("ordinal")?;
                        if ordinal != next_final_relation {
                            return Err(format!(
                                "non-contiguous final relation at line {}",
                                row.line_number
                            ));
                        }
                        next_final_relation += 1;
                    }
                    Family::Glyph => {
                        if plan_phase > 1 {
                            return Err(format!(
                                "glyph after final relation at line {}",
                                row.line_number
                            ));
                        }
                        plan_phase = 1;
                        let ordinal: usize = row.number("ordinal")?;
                        if ordinal != next_glyph {
                            return Err(format!(
                                "non-contiguous glyph at line {}",
                                row.line_number
                            ));
                        }
                        next_glyph += 1;
                    }
                    Family::End => {
                        current_plan = None;
                        plan_phase = 3;
                    }
                    Family::Gap | Family::Separation | Family::Relation | Family::Update => {
                        if plan_phase != 0 {
                            return Err(format!(
                                "trace row after terminal row group at line {}",
                                row.line_number
                            ));
                        }
                    }
                    _ => unreachable!("plan lifecycle family"),
                }
            }
            Family::SystemSummary => {
                let system: usize = row.number("system")?;
                if current_system != Some(system) || current_plan.is_some() {
                    return Err(format!(
                        "misnested system summary at line {}",
                        row.line_number
                    ));
                }
                current_system = None;
            }
            Family::PageSummary => {
                if current_system.is_some() || current_plan.is_some() {
                    return Err(format!("early page summary at line {}", row.line_number));
                }
                saw_page_summary = true;
            }
        }
    }
    if !saw_page_summary
        || !saw_corpus_summary
        || current_system.is_some()
        || current_plan.is_some()
    {
        return Err("fixture ends before its summaries".to_owned());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TraceFamily {
    Gap,
    Separation,
    Relation,
    Update,
}

fn native_trace_topology(trace: &[NativeStemsBeamExpandStep]) -> Vec<(usize, TraceFamily)> {
    let mut result = Vec::new();
    for step in trace {
        if step.gap.is_some() {
            result.push((step.item_index, TraceFamily::Gap));
        }
        if step.separation.is_some() {
            result.push((step.item_index, TraceFamily::Separation));
        }
        if step.relation_check.is_some() {
            result.push((step.item_index, TraceFamily::Relation));
        }
        if step.glyph_update.is_some() {
            result.push((step.item_index, TraceFamily::Update));
        }
    }
    result
}

fn expected_trace_topology(rows: &[&OracleRow<'_>]) -> Result<Vec<(usize, TraceFamily)>, String> {
    rows.iter()
        .filter_map(|row| {
            let family = match row.family {
                Family::Gap => TraceFamily::Gap,
                Family::Separation => TraceFamily::Separation,
                Family::Relation => TraceFamily::Relation,
                Family::Update => TraceFamily::Update,
                _ => return None,
            };
            Some(row.number("item").map(|item| (item, family)))
        })
        .collect()
}

fn assert_projection_topology(
    fixture: &OracleFixture<'_>,
    actual: &NativeStemsBeamLinkPlanRecognition,
) -> Result<(), String> {
    let plan_rows = fixture
        .rows
        .iter()
        .filter(|row| row.family == Family::Plan)
        .collect::<Vec<_>>();
    if plan_rows.len() != actual.attempt_count {
        return Err(format!(
            "{} plan rows != {} native attempts",
            plan_rows.len(),
            actual.attempt_count
        ));
    }

    let mut flat_index = 0;
    for system in &actual.systems {
        let mut system_plan = 0_usize;
        for builder in &system.builders {
            for attempt in &builder.attempts {
                let plan_row = plan_rows[flat_index];
                let plan = plan_row.number::<usize>("plan")?;
                if plan != system_plan
                    || plan_row.number::<usize>("system")? != system.system_id
                    || plan_row.number::<usize>("builder")? != builder.builder_ordinal
                    || plan_row.number::<i32>("constructionMax")?
                        != builder.construction_max_profile
                    || plan_row.number::<i32>("stemProfile")? != attempt.stem_profile
                    || plan_row.number::<i32>("linkProfile")? != attempt.link_profile
                    || plan_row.number::<usize>("targetCount")? != attempt.head_target_count
                {
                    return Err(format!(
                        "{} system {} plan {} input projection differs",
                        fixture.page, system.system_id, plan
                    ));
                }

                let plan_rows = fixture
                    .rows
                    .iter()
                    .skip_while(|row| {
                        row.family != Family::Plan
                            || row.number::<usize>("system").ok() != Some(system.system_id)
                            || row.number::<usize>("plan").ok() != Some(plan)
                    })
                    .skip(1)
                    .take_while(|row| row.family != Family::End)
                    .collect::<Vec<_>>();
                let expected_trace = expected_trace_topology(&plan_rows)?;
                let native_trace = native_trace_topology(&attempt.trace);
                if expected_trace != native_trace {
                    return Err(format!(
                        "{} system {} plan {} trace topology differs",
                        fixture.page, system.system_id, plan
                    ));
                }

                let end = fixture
                    .rows
                    .iter()
                    .find(|row| {
                        row.family == Family::End
                            && row.number::<usize>("system").ok() == Some(system.system_id)
                            && row.number::<usize>("plan").ok() == Some(plan)
                    })
                    .ok_or_else(|| {
                        format!(
                            "missing end row for system {} plan {plan}",
                            system.system_id
                        )
                    })?;
                let expected_outcome = match attempt.outcome {
                    NativeStemsBeamLinkPlanOutcome::NoHeadTarget => "NoHeadTarget",
                    NativeStemsBeamLinkPlanOutcome::ExpandFailed => "ExpandFailed",
                    NativeStemsBeamLinkPlanOutcome::NoRelations => "NoRelations",
                    NativeStemsBeamLinkPlanOutcome::NoGlyphs => "NoGlyphs",
                    NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem => "ReadyForCreateStem",
                };
                if end.value("outcome")? != expected_outcome
                    || end.number::<usize>("relationCount")? != attempt.relations.len()
                    || end.number::<usize>("glyphCount")? != attempt.glyphs.len()
                    || end.number::<usize>("sigVertexDelta")? != 0
                    || end.number::<usize>("sigEdgeDelta")? != 0
                    || end.number::<usize>("stemInterDelta")? != 0
                    || end.number::<usize>("systemStemDelta")? != 0
                    || end.number::<usize>("glyphIndexDelta")? != 0
                    || end.number::<usize>("filamentIndexDelta")? != 0
                    || end.number::<usize>("linkMutations")? != 0
                    || end.number::<usize>("builderMutations")? != 0
                {
                    return Err(format!(
                        "{} system {} plan {} end projection differs",
                        fixture.page, system.system_id, plan
                    ));
                }
                flat_index += 1;
                system_plan += 1;
            }
        }
    }
    Ok(())
}

struct NativePage {
    grid: GridLinesRecognition,
    stem_seeds: NativeStemSeedRecognition,
    beams: NativeBeamRecognition,
    heads: NativeHeadsRecognition,
    corners: NativeStemsHeadCornerRecognition,
    beam_stumps: NativeStemsBeamStumpRecognition,
    beam_vlinkers: NativeStemsBeamVLinkerRecognition,
    beam_reachability: NativeStemsBeamReachabilityRecognition,
    head_stumps: NativeStemsHeadStumpRecognition,
    beam_builders: NativeStemsBeamBuilderRecognition,
    head_builders: audiveris_omr::native_stems_head_builders::NativeStemsHeadBuilderRecognition,
    plans: NativeStemsBeamLinkPlanRecognition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GateGlyph {
    reference: NativeStemsBeamBuilderGlyphRef,
    bounds: Bounds,
    weight: usize,
    run_table: RunTable,
    modeled_canonical_ordinal: usize,
}

impl GateGlyph {
    fn selected(&self) -> NativeStemsBeamSelectedGlyph {
        NativeStemsBeamSelectedGlyph {
            reference: self.reference,
            bounds: self.bounds,
            weight: self.weight,
            structural_key: NativeStemsBeamLinkGlyphKey {
                left: self.bounds.x,
                top: self.bounds.y,
                run_table: self.run_table.clone(),
            },
            structural_digest: structural_digest(self.bounds, &self.run_table),
            modeled_canonical_ordinal: self.modeled_canonical_ordinal,
        }
    }

    fn content_token(&self) -> String {
        glyph_content_token(self.bounds, &self.run_table)
    }
}

#[derive(Clone, Debug)]
struct RetainedGlyph {
    glyph: GateGlyph,
    source_item: usize,
}

fn horizontal_token(side: NativeStemHeadSide) -> &'static str {
    match side {
        NativeStemHeadSide::Left => "LEFT",
        NativeStemHeadSide::Right => "RIGHT",
    }
}

fn vertical_token(side: NativeStemVerticalSide) -> &'static str {
    match side {
        NativeStemVerticalSide::Top => "TOP",
        NativeStemVerticalSide::Bottom => "BOTTOM",
    }
}

fn bool_token(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn list_token(values: impl IntoIterator<Item = String>) -> String {
    let values = values.into_iter().collect::<Vec<_>>();
    if values.is_empty() {
        "-".to_owned()
    } else {
        values.join(",")
    }
}

fn java_hex_float(value: f64) -> String {
    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let magnitude = bits & 0x7fff_ffff_ffff_ffff;
    let prefix = if negative { "-" } else { "" };
    let exponent_bits = ((magnitude >> 52) & 0x7ff) as i32;
    let fraction = magnitude & 0x000f_ffff_ffff_ffff;
    if exponent_bits == 0x7ff {
        return if fraction == 0 {
            format!("{prefix}Infinity")
        } else {
            "NaN".to_owned()
        };
    }
    if exponent_bits == 0 && fraction == 0 {
        return format!("{prefix}0x0.0p0");
    }
    let mut digits = format!("{fraction:013x}");
    while digits.len() > 1 && digits.ends_with('0') {
        digits.pop();
    }
    if exponent_bits == 0 {
        format!("{prefix}0x0.{digits}p-1022")
    } else {
        format!("{prefix}0x1.{digits}p{}", exponent_bits - 1023)
    }
}

fn hex_double(value: f64) -> String {
    format!("{}/{:016x}", java_hex_float(value), value.to_bits())
}

fn point_token(point: NativeStemPoint) -> String {
    format!("{}:{}", hex_double(point.x), hex_double(point.y))
}

fn line_token(line: NativeStemLine) -> String {
    format!("{}:{}", point_token(line.start), point_token(line.stop))
}

fn line_bits_token(line: NativeStemLine) -> String {
    format!(
        "{:016x}:{:016x}:{:016x}:{:016x}",
        line.start.x.to_bits(),
        line.start.y.to_bits(),
        line.stop.x.to_bits(),
        line.stop.y.to_bits(),
    )
}

fn line_bits_equal(one: NativeStemLine, two: NativeStemLine) -> bool {
    one.start.x.to_bits() == two.start.x.to_bits()
        && one.start.y.to_bits() == two.start.y.to_bits()
        && one.stop.x.to_bits() == two.stop.x.to_bits()
        && one.stop.y.to_bits() == two.stop.y.to_bits()
}

fn bounds_token(bounds: Bounds) -> String {
    format!(
        "{}:{}:{}:{}",
        bounds.x, bounds.y, bounds.width, bounds.height
    )
}

fn rectangle_token(bounds: JavaRectangle) -> String {
    format!(
        "{}:{}:{}:{}",
        bounds.x, bounds.y, bounds.width, bounds.height
    )
}

fn line_from_segment(segment: Segment) -> NativeStemLine {
    NativeStemLine {
        start: NativeStemPoint {
            x: segment.x1,
            y: segment.y1,
        },
        stop: NativeStemPoint {
            x: segment.x2,
            y: segment.y2,
        },
    }
}

fn head_shape_token(shape: HeadTemplateShape) -> &'static str {
    match shape {
        HeadTemplateShape::NoteheadBlack => "NOTEHEAD_BLACK",
        HeadTemplateShape::NoteheadVoid => "NOTEHEAD_VOID",
        HeadTemplateShape::WholeNote => "WHOLE_NOTE",
        HeadTemplateShape::Breve => "BREVE",
        HeadTemplateShape::NoteheadBlackSmall => "NOTEHEAD_BLACK_SMALL",
        HeadTemplateShape::NoteheadVoidSmall => "NOTEHEAD_VOID_SMALL",
        HeadTemplateShape::WholeNoteSmall => "WHOLE_NOTE_SMALL",
        HeadTemplateShape::BreveSmall => "BREVE_SMALL",
    }
}

fn stem_portion_token(portion: NativeStemsBeamStemPortion) -> &'static str {
    match portion {
        NativeStemsBeamStemPortion::Top => "STEM_TOP",
        NativeStemsBeamStemPortion::Middle => "STEM_MIDDLE",
        NativeStemsBeamStemPortion::Bottom => "STEM_BOTTOM",
    }
}

fn outcome_token(outcome: NativeStemsBeamLinkPlanOutcome) -> &'static str {
    match outcome {
        NativeStemsBeamLinkPlanOutcome::NoHeadTarget => "NoHeadTarget",
        NativeStemsBeamLinkPlanOutcome::ExpandFailed => "ExpandFailed",
        NativeStemsBeamLinkPlanOutcome::NoRelations => "NoRelations",
        NativeStemsBeamLinkPlanOutcome::NoGlyphs => "NoGlyphs",
        NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem => "ReadyForCreateStem",
    }
}

fn stop_cause_token(cause: Option<NativeStemsBeamExpandStopCause>) -> &'static str {
    match cause {
        None => "NoHeadTarget",
        Some(NativeStemsBeamExpandStopCause::CompletedAllItems) => "ExhaustedItems",
        Some(NativeStemsBeamExpandStopCause::ShowStoppingGapBeforeHead) => {
            "ShowStoppingGapBeforeStoppingHead"
        }
        Some(NativeStemsBeamExpandStopCause::ShowStoppingGapRestoredHead) => {
            "ShowStoppingGapRollback"
        }
        Some(NativeStemsBeamExpandStopCause::SeparatedBeforeHead) => "SeparatedHeadRollback",
    }
}

fn item_kind_token(kind: NativeStemsBeamBuilderItemKind) -> &'static str {
    match kind {
        NativeStemsBeamBuilderItemKind::StartHalfLinker => "start",
        NativeStemsBeamBuilderItemKind::BeamLinker => "B",
        NativeStemsBeamBuilderItemKind::HeadHalfLinker => "C",
        NativeStemsBeamBuilderItemKind::SeedGlyph | NativeStemsBeamBuilderItemKind::ChunkGlyph => {
            "glyph"
        }
        NativeStemsBeamBuilderItemKind::Gap => "gap",
    }
}

struct GateSystem<'a> {
    system_id: usize,
    seed_system: &'a audiveris_omr::native_stem_seeds::NativeStemSeedSystemRecognition,
    stump_system: &'a audiveris_omr::native_stems_beam_stumps::NativeStemsBeamStumpSystem,
    v_system: &'a audiveris_omr::native_stems_beam_vlinkers::NativeStemsBeamVLinkerSystem,
    reachability_system:
        &'a audiveris_omr::native_stems_beam_reachability::NativeStemsBeamReachabilitySystem,
    corner_system: &'a audiveris_omr::native_stems_head_corners::NativeStemsHeadCornerSystem,
    head_stump_system: &'a audiveris_omr::native_stems_head_stumps::NativeStemsHeadStumpSystem,
    builder_system: &'a audiveris_omr::native_stems_beam_builders::NativeStemsBeamBuilderSystem,
    modeled_registry:
        &'a [audiveris_omr::native_stems_beam_builders::NativeStemsModeledCanonicalGlyph],
}

impl<'a> GateSystem<'a> {
    fn new(page: &'a NativePage, index: usize) -> Self {
        let system_id = page.plans.systems[index].system_id;
        let result = Self {
            system_id,
            seed_system: &page.stem_seeds.systems[index],
            stump_system: &page.beam_stumps.systems[index],
            v_system: &page.beam_vlinkers.systems[index],
            reachability_system: &page.beam_reachability.systems[index],
            corner_system: &page.corners.systems[index],
            head_stump_system: &page.head_stumps.systems[index],
            builder_system: &page.beam_builders.systems[index],
            modeled_registry: &page.head_builders.modeled_canonical_glyphs,
        };
        assert_eq!(result.seed_system.raw.system_id, system_id);
        assert_eq!(result.stump_system.system_id, system_id);
        assert_eq!(result.v_system.system_id, system_id);
        assert_eq!(result.reachability_system.system_id, system_id);
        assert_eq!(result.corner_system.system_id, system_id);
        assert_eq!(result.head_stump_system.system_id, system_id);
        assert_eq!(result.builder_system.system_id, system_id);
        result
    }

    fn beam(&self, source: NativeStemsBeamSource) -> &'a NativeStemsBeamStumpBeam {
        self.stump_system
            .beams_by_abscissa
            .iter()
            .find(|beam| beam.source == source)
            .expect("beam source")
    }

    fn beam_sig_ordinal(&self, source: NativeStemsBeamSource) -> usize {
        self.beam(source).sig_ordinal
    }

    fn b_linker(&self, reference: NativeStemsBeamBLinkerRef) -> &'a NativeStemsBeamBLinker {
        self.v_system
            .constructors
            .iter()
            .find(|constructor| constructor.source == reference.beam)
            .and_then(|constructor| {
                constructor
                    .b_linkers
                    .iter()
                    .find(|candidate| candidate.reference == reference)
            })
            .expect("B linker")
    }

    fn b_alias(&self, reference: NativeStemsBeamBLinkerRef) -> String {
        format!(
            "beam:{}:b:{}",
            self.beam_sig_ordinal(reference.beam),
            reference.id.checked_sub(1).expect("one-based B id"),
        )
    }

    fn attachment_aliases_v_theoretical_line(
        &self,
        reference: audiveris_omr::native_stems_beam_vlinkers::NativeStemsBeamVLinkerRef,
    ) -> bool {
        self.b_linker(reference.b_linker)
            .v_linkers
            .last()
            .is_some_and(|candidate| candidate.reference == reference)
    }

    fn c_alias(&self, reference: NativeStemsBeamHeadCornerRef) -> String {
        format!(
            "h:{}:{}:{}",
            reference.x_ordinal,
            horizontal_token(reference.horizontal),
            vertical_token(reference.vertical),
        )
    }

    fn head(&self, reference: NativeStemsBeamHeadCornerRef) -> &'a NativeStemsHeadCornerHead {
        let head = self
            .corner_system
            .heads_in_sig_order
            .get(reference.sig_ordinal)
            .expect("head SIG ordinal");
        assert_eq!(head.reference, reference.head);
        head
    }

    fn item_ref(&self, item: &NativeStemsBeamBuilderItem, index: usize) -> String {
        match item.kind {
            NativeStemsBeamBuilderItemKind::Gap => format!("gap:{index}"),
            NativeStemsBeamBuilderItemKind::StartHalfLinker => format!("start:{index}"),
            NativeStemsBeamBuilderItemKind::BeamLinker => {
                let Some(NativeStemsBeamBuilderTargetRef::Beam(reference)) = item.target else {
                    panic!("beam item target");
                };
                format!("B:{}", self.b_alias(reference))
            }
            NativeStemsBeamBuilderItemKind::HeadHalfLinker => {
                let Some(NativeStemsBeamBuilderTargetRef::Head(reference)) = item.target else {
                    panic!("head item target");
                };
                format!("C:{}", self.c_alias(reference))
            }
            NativeStemsBeamBuilderItemKind::SeedGlyph
            | NativeStemsBeamBuilderItemKind::ChunkGlyph => format!("glyphItem:{index}"),
        }
    }

    fn beam_stump_glyph(
        &self,
        beam: &NativeStemsBeamStumpBeam,
        stump: &NativeStemsBeamStumpRef,
    ) -> NativeStemsBeamGlyph {
        match stump {
            NativeStemsBeamStumpRef::Seed {
                free_glyph_ordinal, ..
            } => {
                let glyph = &self.seed_system.free_glyphs[*free_glyph_ordinal];
                NativeStemsBeamGlyph {
                    bounds: glyph.bounds,
                    weight: glyph.weight,
                    run_table: glyph.run_table.clone(),
                }
            }
            NativeStemsBeamStumpRef::Built {
                canonical_glyph_index,
            } => beam
                .sides
                .iter()
                .filter_map(|side| side.build.as_ref())
                .find(|build| {
                    build.candidate.is_some()
                        && build.canonical_glyph_index == Some(*canonical_glyph_index)
                })
                .and_then(|build| build.candidate.clone())
                .expect("built beam stump glyph"),
        }
    }

    fn head_build_glyph(
        &self,
        reference: NativeStemsBeamBuilderGlyphRef,
        build: &NativeStemsHeadStumpBuild,
    ) -> GateGlyph {
        let glyph = build.candidate.as_ref().expect("built head stump glyph");
        self.gate_glyph(
            reference,
            glyph.bounds,
            glyph.weight,
            glyph.run_table.clone(),
        )
    }

    fn gate_glyph(
        &self,
        reference: NativeStemsBeamBuilderGlyphRef,
        bounds: Bounds,
        weight: usize,
        run_table: RunTable,
    ) -> GateGlyph {
        let matches = self
            .modeled_registry
            .iter()
            .filter(|entry| {
                entry.bounds == bounds && entry.weight == weight && entry.run_table == run_table
            })
            .collect::<Vec<_>>();
        let [modeled] = matches.as_slice() else {
            panic!("modeled canonical glyph cardinality: {}", matches.len());
        };
        GateGlyph {
            reference,
            bounds,
            weight,
            run_table,
            modeled_canonical_ordinal: modeled.modeled_canonical_ordinal,
        }
    }

    fn resolve_glyph(
        &self,
        builder: &NativeStemsBeamBuilder,
        reference: NativeStemsBeamBuilderGlyphRef,
    ) -> GateGlyph {
        match reference {
            NativeStemsBeamBuilderGlyphRef::StemSeed { free_glyph_ordinal } => {
                let glyph = &self.seed_system.free_glyphs[free_glyph_ordinal];
                self.gate_glyph(
                    reference,
                    glyph.bounds,
                    glyph.weight,
                    glyph.run_table.clone(),
                )
            }
            NativeStemsBeamBuilderGlyphRef::BeamStump { b_linker } => {
                let linker = self.b_linker(b_linker);
                let stump = linker.stump.as_ref().expect("beam stump");
                let glyph = self.beam_stump_glyph(self.beam(b_linker.beam), stump);
                self.gate_glyph(reference, glyph.bounds, glyph.weight, glyph.run_table)
            }
            NativeStemsBeamBuilderGlyphRef::HeadStump { corner } => {
                let head = self
                    .head_stump_system
                    .heads_by_abscissa
                    .iter()
                    .find(|head| head.sig_ordinal == corner.sig_ordinal)
                    .expect("head stump head");
                let constructor_ordinal = self
                    .head(corner)
                    .corners_in_constructor_order
                    .iter()
                    .find(|candidate| {
                        candidate.horizontal == corner.horizontal
                            && candidate.vertical == corner.vertical
                    })
                    .expect("head corner")
                    .constructor_ordinal;
                let stump = head
                    .corners_in_constructor_order
                    .iter()
                    .find(|candidate| candidate.constructor_ordinal == constructor_ordinal)
                    .expect("head stump corner");
                match stump.outcome {
                    NativeStemsHeadStumpOutcome::Seed { free_glyph_ordinal } => {
                        let glyph = &self.seed_system.free_glyphs[free_glyph_ordinal];
                        self.gate_glyph(
                            reference,
                            glyph.bounds,
                            glyph.weight,
                            glyph.run_table.clone(),
                        )
                    }
                    NativeStemsHeadStumpOutcome::Built { .. } => self.head_build_glyph(
                        reference,
                        stump.build.as_ref().expect("head stump build"),
                    ),
                    NativeStemsHeadStumpOutcome::None => panic!("missing head stump glyph"),
                }
            }
            NativeStemsBeamBuilderGlyphRef::Chunk { .. } => {
                let registration = builder
                    .glyph_registrations
                    .iter()
                    .find(|registration| registration.glyph == reference)
                    .expect("chunk registration");
                self.gate_glyph(
                    reference,
                    registration.bounds,
                    registration.weight,
                    registration.run_table.clone(),
                )
            }
        }
    }
}

fn structural_digest(bounds: Bounds, run_table: &RunTable) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut put = |value: u64| {
        for byte in value.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
    };
    put(bounds.x as u64);
    put(bounds.y as u64);
    put(bounds.width as u64);
    put(bounds.height as u64);
    put(match run_table.orientation() {
        Orientation::Horizontal => 0,
        Orientation::Vertical => 1,
    });
    for sequence in 0..run_table.sequence_count() {
        put(sequence as u64);
        for run in run_table.sequence(sequence).unwrap_or_default() {
            put(run.start as u64);
            put(run.length as u64);
        }
        put(u64::MAX);
    }
    hash
}

fn glyph_run_sha256(run_table: &RunTable) -> String {
    let orientation = match run_table.orientation() {
        Orientation::Horizontal => "HORIZONTAL",
        Orientation::Vertical => "VERTICAL",
    };
    let mut bytes = format!(
        "{orientation} {} {}\n",
        run_table.width(),
        run_table.height()
    )
    .into_bytes();
    for sequence in 0..run_table.sequence_count() {
        let mut row = sequence.to_string();
        for run in run_table.sequence(sequence).unwrap_or_default() {
            write!(&mut row, " {}:{}", run.start, run.length).expect("run token");
        }
        row.push('\n');
        bytes.extend_from_slice(row.as_bytes());
    }
    sha256_hex(&bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let bit_len = u64::try_from(bytes.len()).expect("SHA input length") * 8;
    let mut padded = bytes.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    let mut state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in padded.chunks_exact(64) {
        let mut words = [0u32; 64];
        for (index, word) in words[..16].iter_mut().enumerate() {
            let start = 4 * index;
            *word = u32::from_be_bytes(chunk[start..start + 4].try_into().expect("SHA word"));
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
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
    let mut result = String::with_capacity(64);
    for word in state {
        write!(&mut result, "{word:08x}").expect("format SHA-256");
    }
    result
}

fn glyph_content_token(bounds: Bounds, run_table: &RunTable) -> String {
    format!(
        "g:{}:{}:{}:{}:{}",
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        glyph_run_sha256(run_table),
    )
}

fn selected_content_token(glyph: &NativeStemsBeamSelectedGlyph) -> String {
    glyph_content_token(glyph.bounds, &glyph.structural_key.run_table)
}

fn retained_content_tokens(glyphs: &[RetainedGlyph]) -> String {
    list_token(glyphs.iter().map(|glyph| glyph.glyph.content_token()))
}

fn retained_source_tokens(
    system: &GateSystem<'_>,
    builder: &NativeStemsBeamBuilder,
    glyphs: &[RetainedGlyph],
) -> String {
    list_token(
        glyphs
            .iter()
            .map(|glyph| system.item_ref(&builder.items[glyph.source_item], glyph.source_item)),
    )
}

fn selected_content_tokens(glyphs: &[NativeStemsBeamSelectedGlyph]) -> String {
    list_token(glyphs.iter().map(selected_content_token))
}

fn centroid(bounds: Bounds, run_table: &RunTable) -> NativeStemPoint {
    let left = i32::try_from(bounds.x).expect("glyph left");
    let top = i32::try_from(bounds.y).expect("glyph top");
    let mut sum_x = 0_f64;
    let mut sum_y = 0_f64;
    for (x, y) in run_table.foreground_points((left, top)) {
        sum_x += f64::from(x);
        sum_y += f64::from(y);
    }
    let weight = run_table.weight();
    NativeStemPoint {
        x: sum_x / weight as f64,
        y: sum_y / weight as f64,
    }
}

#[derive(Clone)]
struct GateComposite {
    bounds: Bounds,
    weight: usize,
    run_table: RunTable,
    centroid: NativeStemPoint,
    center_line: NativeStemLine,
}

fn gate_composite(glyphs: &[RetainedGlyph]) -> GateComposite {
    let first = glyphs.first().expect("nonempty glyph set");
    if glyphs.len() == 1 {
        let center_line = run_table_center_line(
            &first.glyph.run_table,
            i32::try_from(first.glyph.bounds.x).expect("glyph left"),
            i32::try_from(first.glyph.bounds.y).expect("glyph top"),
        )
        .map(line_from_segment)
        .expect("glyph center line");
        return GateComposite {
            bounds: first.glyph.bounds,
            weight: first.glyph.weight,
            run_table: first.glyph.run_table.clone(),
            centroid: centroid(first.glyph.bounds, &first.glyph.run_table),
            center_line,
        };
    }
    let min_x = glyphs
        .iter()
        .map(|glyph| glyph.glyph.bounds.x)
        .min()
        .unwrap();
    let min_y = glyphs
        .iter()
        .map(|glyph| glyph.glyph.bounds.y)
        .min()
        .unwrap();
    let max_x = glyphs
        .iter()
        .map(|glyph| glyph.glyph.bounds.x + glyph.glyph.bounds.width)
        .max()
        .unwrap();
    let max_y = glyphs
        .iter()
        .map(|glyph| glyph.glyph.bounds.y + glyph.glyph.bounds.height)
        .max()
        .unwrap();
    let bounds = Bounds {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    };
    let mut pixels = vec![BACKGROUND; bounds.width * bounds.height];
    for retained in glyphs {
        let glyph = &retained.glyph;
        for sequence in 0..glyph.run_table.sequence_count() {
            for run in glyph.run_table.sequence(sequence).unwrap_or_default() {
                for coordinate in run.start..=run.stop() {
                    let (local_x, local_y) = match glyph.run_table.orientation() {
                        Orientation::Horizontal => (coordinate, sequence),
                        Orientation::Vertical => (sequence, coordinate),
                    };
                    let x = glyph.bounds.x - bounds.x + local_x;
                    let y = glyph.bounds.y - bounds.y + local_y;
                    pixels[y * bounds.width + x] = FOREGROUND;
                }
            }
        }
    }
    let run_table =
        RunTable::from_pixels(Orientation::Vertical, bounds.width, bounds.height, &pixels)
            .expect("compound run table");
    let weight = run_table.weight();
    let centroid = centroid(bounds, &run_table);
    let center_line = run_table_center_line(
        &run_table,
        i32::try_from(bounds.x).expect("compound left"),
        i32::try_from(bounds.y).expect("compound top"),
    )
    .map(line_from_segment)
    .expect("compound center line");
    GateComposite {
        bounds,
        weight,
        run_table,
        centroid,
        center_line,
    }
}

fn generic_intersection(one: NativeStemLine, two: NativeStemLine) -> NativeStemPoint {
    let denominator = ((one.start.x - one.stop.x) * (two.start.y - two.stop.y))
        - ((one.start.y - one.stop.y) * (two.start.x - two.stop.x));
    let one_cross = (one.start.x * one.stop.y) - (one.start.y * one.stop.x);
    let two_cross = (two.start.x * two.stop.y) - (two.start.y * two.stop.x);
    NativeStemPoint {
        x: ((one_cross * (two.start.x - two.stop.x)) - ((one.start.x - one.stop.x) * two_cross))
            / denominator,
        y: ((one_cross * (two.start.y - two.stop.y)) - ((one.start.y - one.stop.y) * two_cross))
            / denominator,
    }
}

fn intersection_at_y(line: NativeStemLine, y: f64) -> NativeStemPoint {
    generic_intersection(
        line,
        NativeStemLine {
            start: NativeStemPoint { x: 0.0, y },
            stop: NativeStemPoint { x: 1000.0, y },
        },
    )
}

fn update_line_from_retained(glyphs: &[RetainedGlyph], line: &mut NativeStemLine) {
    let composite = gate_composite(glyphs);
    let crossing = intersection_at_y(*line, composite.centroid.y);
    let dx = composite.centroid.x - crossing.x;
    line.start.x += dx;
    line.stop.x += dx;
}

fn relative_ccw(line: NativeStemLine, point: NativeStemPoint) -> i32 {
    let delta_x = line.stop.x - line.start.x;
    let delta_y = line.stop.y - line.start.y;
    let mut point_x = point.x - line.start.x;
    let mut point_y = point.y - line.start.y;
    let mut ccw = (point_x * delta_y) - (point_y * delta_x);
    if ccw == 0.0 {
        ccw = (point_x * delta_x) + (point_y * delta_y);
        if ccw > 0.0 {
            point_x -= delta_x;
            point_y -= delta_y;
            ccw = (point_x * delta_x) + (point_y * delta_y);
            if ccw < 0.0 {
                ccw = 0.0;
            }
        }
    }
    if ccw < 0.0 {
        -1
    } else if ccw > 0.0 {
        1
    } else {
        0
    }
}

#[derive(Clone)]
struct ProjectedRelation {
    corner: NativeStemsBeamHeadCornerRef,
    first_item: usize,
    latest_item: usize,
    check: NativeStemsBeamHeadRelationCheck,
}

#[derive(Clone, Copy, Default)]
struct GateTotals {
    systems: usize,
    builders: usize,
    plans: usize,
    no_head_target: usize,
    expand_failed: usize,
    no_relations: usize,
    no_glyphs: usize,
    ready: usize,
    relations: usize,
    glyphs: usize,
    post_return_relations: usize,
    plans_with_post_return_relations: usize,
    rollback_line_divergences: usize,
    relation_side_mismatches: usize,
    stored_theo_mutations: usize,
    attachment_line_mutations: usize,
    beam_side_ready: usize,
    beam_side_ready_without_stopping_head: usize,
    beam_side_ready_beyond_stopping_head: usize,
    beam_side_ready_at_stopping_head: usize,
    max_abs_stored_theo_shift: f64,
}

impl GateTotals {
    fn add_attempt(&mut self, attempt: &NativeStemsBeamLinkPlanAttempt) {
        self.plans += 1;
        match attempt.outcome {
            NativeStemsBeamLinkPlanOutcome::NoHeadTarget => self.no_head_target += 1,
            NativeStemsBeamLinkPlanOutcome::ExpandFailed => self.expand_failed += 1,
            NativeStemsBeamLinkPlanOutcome::NoRelations => self.no_relations += 1,
            NativeStemsBeamLinkPlanOutcome::NoGlyphs => self.no_glyphs += 1,
            NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem => self.ready += 1,
        }
        self.relations += attempt.relations.len();
        self.glyphs += attempt.glyphs.len();
        self.post_return_relations += attempt.relations_past_return_count;
        self.plans_with_post_return_relations +=
            usize::from(attempt.relations_past_return_count != 0);
        self.rollback_line_divergences +=
            usize::from(attempt.rollback_line_diverges_from_restored_glyphs);
        self.relation_side_mismatches += attempt
            .trace
            .iter()
            .filter_map(|step| step.relation_check.as_ref())
            .filter(|check| check.horizontal_side_diverges)
            .count();
        self.stored_theo_mutations += usize::from(attempt.stored_theoretical_line_would_mutate);
        self.attachment_line_mutations += usize::from(attempt.attachment_alias_would_mutate);
        let ready_side = attempt.stem_profile == 4
            && attempt.outcome == NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem;
        self.beam_side_ready += usize::from(ready_side);
        self.beam_side_ready_without_stopping_head +=
            usize::from(attempt.beam_side_ready_without_stopping_head);
        self.beam_side_ready_beyond_stopping_head +=
            usize::from(attempt.beam_side_ready_beyond_stopping_head);
        self.beam_side_ready_at_stopping_head +=
            usize::from(attempt.beam_side_ready_at_stopping_head);
        self.max_abs_stored_theo_shift = self.max_abs_stored_theo_shift.max(
            (attempt.stored_theoretical_line_after.start.x
                - attempt.stored_theoretical_line_before.start.x)
                .abs(),
        );
    }

    fn include(&mut self, other: Self) {
        self.systems += other.systems;
        self.builders += other.builders;
        self.plans += other.plans;
        self.no_head_target += other.no_head_target;
        self.expand_failed += other.expand_failed;
        self.no_relations += other.no_relations;
        self.no_glyphs += other.no_glyphs;
        self.ready += other.ready;
        self.relations += other.relations;
        self.glyphs += other.glyphs;
        self.post_return_relations += other.post_return_relations;
        self.plans_with_post_return_relations += other.plans_with_post_return_relations;
        self.rollback_line_divergences += other.rollback_line_divergences;
        self.relation_side_mismatches += other.relation_side_mismatches;
        self.stored_theo_mutations += other.stored_theo_mutations;
        self.attachment_line_mutations += other.attachment_line_mutations;
        self.beam_side_ready += other.beam_side_ready;
        self.beam_side_ready_without_stopping_head += other.beam_side_ready_without_stopping_head;
        self.beam_side_ready_beyond_stopping_head += other.beam_side_ready_beyond_stopping_head;
        self.beam_side_ready_at_stopping_head += other.beam_side_ready_at_stopping_head;
        self.max_abs_stored_theo_shift = self
            .max_abs_stored_theo_shift
            .max(other.max_abs_stored_theo_shift);
    }

    fn fields(&self) -> String {
        format!(
            "systems {} builders {} plans {} noHeadTarget {} expandFailed {} \
             noRelations {} noGlyphs {} ready {} relations {} glyphs {} \
             postReturnRelations {} plansWithPostReturnRelations {} \
             rollbackLineDivergences {} relationSideMismatches {} \
             storedTheoMutations {} attachmentLineMutations {} beamSideReady {} \
             beamSideReadyWithoutStoppingHead {} beamSideReadyBeyondStoppingHead {} \
             beamSideReadyAtStoppingHead {} maxAbsStoredTheoShift {} \
             sigMutations 0 systemStemMutations 0 glyphIndexMutations 0 \
             filamentIndexMutations 0 linkMutations 0 builderMutations 0",
            self.systems,
            self.builders,
            self.plans,
            self.no_head_target,
            self.expand_failed,
            self.no_relations,
            self.no_glyphs,
            self.ready,
            self.relations,
            self.glyphs,
            self.post_return_relations,
            self.plans_with_post_return_relations,
            self.rollback_line_divergences,
            self.relation_side_mismatches,
            self.stored_theo_mutations,
            self.attachment_line_mutations,
            self.beam_side_ready,
            self.beam_side_ready_without_stopping_head,
            self.beam_side_ready_beyond_stopping_head,
            self.beam_side_ready_at_stopping_head,
            hex_double(self.max_abs_stored_theo_shift),
        )
    }
}

#[derive(Clone, Copy)]
struct Fnv64(u64);

impl Default for Fnv64 {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Fnv64 {
    fn add(&mut self, row: &str) {
        for byte in row.bytes().chain([b'\n']) {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x100_0000_01b3);
        }
    }
}

fn relation_alias_tokens(system: &GateSystem<'_>, relations: &[ProjectedRelation]) -> String {
    list_token(
        relations
            .iter()
            .map(|relation| system.c_alias(relation.corner)),
    )
}

fn c_alias_from_head_ref(
    reference: audiveris_omr::native_stems_head_corner_reachability::NativeStemsHeadCornerRef,
) -> String {
    format!(
        "h:{}:{}:{}",
        reference.x_ordinal,
        horizontal_token(reference.horizontal),
        vertical_token(reference.vertical),
    )
}

fn relation_digest(check: &NativeStemsBeamHeadRelationCheck) -> String {
    format!(
        "{}:{}:{}:{:x}:{:x}:{:x}",
        vertical_token(check.vertical),
        horizontal_token(check.encountered_horizontal),
        horizontal_token(check.derived_horizontal),
        check.dx.to_bits(),
        check.dy.to_bits(),
        check.grade.to_bits(),
    )
}

fn project_attempt_rows(
    page: &str,
    system: &GateSystem<'_>,
    builder: &NativeStemsBeamBuilder,
    attempt: &NativeStemsBeamLinkPlanAttempt,
    plan_ordinal: usize,
) -> Vec<String> {
    let b_reference = builder.start.b_linker;
    let b_linker = system.b_linker(b_reference);
    let b_ordinal = b_reference.id.checked_sub(1).expect("one-based B id");
    let head_targets = builder
        .items
        .iter()
        .filter_map(|item| match item.target {
            Some(NativeStemsBeamBuilderTargetRef::Head(reference)) => {
                Some(system.c_alias(reference))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let start_stump = builder.start_stump.map_or_else(
        || "-".to_owned(),
        |reference| system.resolve_glyph(builder, reference).content_token(),
    );
    let initial_gap = system.builder_system.gap_map[&attempt.stem_profile];
    let standard_gap = system.builder_system.gap_map[&1];
    let mut rows = vec![format!(
        "stemsbeamexpandplan {page} system {} plan {plan_ordinal} builder {} beamSig {} \
         bOrdinal {b_ordinal} bAlias {} bId {} hSide {} vSide {} yDir {} \
         constructionMax {} stemProfile {} linkProfile {} items {} maxIndex {} \
         headTargets {} targetCount {} startStump {start_stump} theo {} \
         initialMaxYGap {initial_gap} standardMaxYGap {standard_gap} minLinkerLength {}",
        system.system_id,
        builder.builder_ordinal,
        system.beam_sig_ordinal(b_reference.beam),
        system.b_alias(b_reference),
        b_reference.id,
        b_linker
            .horizontal_side
            .map(horizontal_token)
            .unwrap_or("-"),
        vertical_token(builder.start.side),
        builder.v_y_direction,
        builder.max_stem_profile,
        attempt.stem_profile,
        attempt.link_profile,
        builder.items.len(),
        i64::try_from(builder.items.len()).expect("item count") - 1,
        list_token(head_targets),
        attempt.head_target_count,
        line_token(attempt.stored_theoretical_line_before),
        (f64::from(system.builder_system.interline) * 0.85).round_ties_even() as i32,
    )];

    let mut glyphs = Vec::<RetainedGlyph>::new();
    let mut stopping_glyphs = Vec::<RetainedGlyph>::new();
    let mut stopping_index = None::<usize>;
    let mut relations = Vec::<ProjectedRelation>::new();
    let mut trace_bytes = Vec::<u8>::new();

    for step in &attempt.trace {
        let item = &builder.items[step.item_index];
        trace_bytes.extend_from_slice(
            format!("visit {} {}\n", step.item_index, item_kind_token(item.kind)).as_bytes(),
        );

        if let Some(gap) = &step.gap {
            let row_action = match &gap.action {
                NativeStemsBeamGapAction::Continue => "continue",
                NativeStemsBeamGapAction::FailBeforeStoppingHead => "fail",
                NativeStemsBeamGapAction::RestoreStoppingHead {
                    restored_glyphs, ..
                } => {
                    assert_eq!(
                        selected_content_tokens(restored_glyphs),
                        retained_content_tokens(&stopping_glyphs),
                        "rollback product retains the stopping snapshot",
                    );
                    "rewind"
                }
            };
            // The probe exposes the live stopping snapshot whenever one has
            // been established, not only on the later row that consumes it.
            let restored = if stopping_index.is_some() {
                retained_content_tokens(&stopping_glyphs)
            } else {
                "-".to_owned()
            };
            rows.push(format!(
                "stemsbeamexpandgap {page} system {} plan {plan_ordinal} item {} contrib {} \
                 maxYGap {} stoppingIndex {} action {row_action} glyphsBefore {} \
                 restoredGlyphs {restored} relationsRetained {} stemLineRetained {}",
                system.system_id,
                step.item_index,
                gap.contribution,
                gap.threshold,
                stopping_index.map_or_else(|| "-".to_owned(), |value| value.to_string()),
                retained_content_tokens(&glyphs),
                relation_alias_tokens(system, &relations),
                line_token(step.stem_line_before),
            ));
            if gap.show_stopping {
                trace_bytes.extend_from_slice(
                    format!(
                        "gapstop {} {} {} {}\n",
                        step.item_index,
                        gap.contribution,
                        gap.threshold,
                        if stopping_index.is_some() {
                            "rollback"
                        } else {
                            "fail"
                        },
                    )
                    .as_bytes(),
                );
            }
            if matches!(
                gap.action,
                NativeStemsBeamGapAction::RestoreStoppingHead { .. }
            ) {
                glyphs.clone_from(&stopping_glyphs);
            }
        }

        if let Some(separation) = &step.separation {
            let Some(NativeStemsBeamBuilderTargetRef::Head(candidate)) = item.target else {
                panic!("separation head item");
            };
            rows.push(format!(
                "stemsbeamexpandseparation {page} system {} plan {plan_ordinal} item {} \
                 candidate {} stoppingIndex {} gapIndex {} closeDy {} minLinkerLength {} \
                 underMin {} opposite {} oppositeLength {} oppositeConcrete {} action {}",
                system.system_id,
                step.item_index,
                system.c_alias(candidate),
                stopping_index.expect("separation stopping index"),
                separation
                    .last_gap_index
                    .map_or_else(|| "-".to_owned(), |value| value.to_string()),
                separation
                    .directed_distance
                    .map_or_else(|| "-".to_owned(), hex_double),
                separation.min_linker_length,
                separation.directed_distance.map_or_else(
                    || "-".to_owned(),
                    |_| bool_token(separation.close_before_head).to_owned(),
                ),
                separation
                    .opposite_corner
                    .map_or_else(|| "-".to_owned(), c_alias_from_head_ref),
                separation
                    .opposite_length
                    .map_or_else(|| "-".to_owned(), |value| value.to_string()),
                separation
                    .opposite_has_concrete_start
                    .map_or_else(|| "-".to_owned(), |value| bool_token(value).to_owned(),),
                if separation.separated {
                    "rollback"
                } else {
                    "continue"
                },
            ));
            let gap_index = separation
                .last_gap_index
                .and_then(|value| i32::try_from(value).ok())
                .unwrap_or(-1);
            let close_bits = separation.directed_distance.map_or_else(
                || "-".to_owned(),
                |value| (value.to_bits() as i64).to_string(),
            );
            trace_bytes.extend_from_slice(
                format!(
                    "separation {} {gap_index} {close_bits} {}\n",
                    step.item_index,
                    separation.opposite_has_concrete_start.unwrap_or(false),
                )
                .as_bytes(),
            );
            if separation.separated {
                glyphs.clone_from(&stopping_glyphs);
            }
        }

        if let Some(check) = &step.relation_check {
            project_relation_row(
                &mut rows,
                &mut trace_bytes,
                page,
                system,
                builder,
                attempt,
                plan_ordinal,
                step,
                check,
                &glyphs,
                &mut relations,
                &mut stopping_index,
                &mut stopping_glyphs,
            );
        }

        if let Some(update) = &step.glyph_update {
            project_update_row(
                &mut rows,
                &mut trace_bytes,
                page,
                system,
                builder,
                plan_ordinal,
                step,
                update,
                &mut glyphs,
            );
        }
    }

    assert_eq!(
        glyphs
            .iter()
            .map(|glyph| glyph.glyph.selected())
            .collect::<Vec<_>>(),
        attempt.glyphs,
        "final projected glyph order"
    );
    assert_eq!(relations.len(), attempt.relations.len());
    for (ordinal, (projected, native)) in relations.iter().zip(&attempt.relations).enumerate() {
        assert_eq!(native.map_ordinal, ordinal, "relation map ordinal");
        assert_eq!(projected.corner, native.corner, "relation corner");
        assert_eq!(
            projected.first_item, native.first_item_index,
            "relation first item"
        );
        assert_eq!(
            projected.latest_item, native.latest_item_index,
            "relation latest item"
        );
        assert_eq!(projected.check, native.check, "relation latest payload");
        assert_eq!(
            native.replaced_existing_payload,
            native.first_item_index != native.latest_item_index,
            "relation replacement history",
        );
    }

    for (ordinal, retained) in glyphs.iter().enumerate() {
        let center = centroid(retained.glyph.bounds, &retained.glyph.run_table);
        let center_line = run_table_center_line(
            &retained.glyph.run_table,
            i32::try_from(retained.glyph.bounds.x).expect("glyph left"),
            i32::try_from(retained.glyph.bounds.y).expect("glyph top"),
        )
        .map(line_from_segment)
        .expect("glyph center line");
        rows.push(format!(
            "stemsbeamexpandglyph {page} system {} plan {plan_ordinal} ordinal {ordinal} \
             sourceItem {} sourceRef {} content {} bounds {} weight {} centroid {} centerLine {}",
            system.system_id,
            retained.source_item,
            system.item_ref(&builder.items[retained.source_item], retained.source_item),
            retained.glyph.content_token(),
            bounds_token(retained.glyph.bounds),
            retained.glyph.weight,
            point_token(center),
            line_token(center_line),
        ));
    }

    for (ordinal, relation) in attempt.relations.iter().enumerate() {
        let check = &relation.check;
        let past_return = attempt
            .expand_last_index
            .is_some_and(|last| last >= 0 && relation.latest_item_index as i32 > last);
        rows.push(format!(
            "stemsbeamexpandfinalrelation {page} system {} plan {plan_ordinal} ordinal {ordinal} \
             cAlias {} firstItem {} latestItem {} pastReturn {} headSide {} dx {} dy {} \
             grade {} extension {} impacts {}",
            system.system_id,
            system.c_alias(relation.corner),
            relation.first_item_index,
            relation.latest_item_index,
            bool_token(past_return),
            horizontal_token(check.derived_horizontal),
            hex_double(check.dx),
            hex_double(check.dy),
            hex_double(check.grade),
            check
                .extension_point
                .map_or_else(|| "-".to_owned(), point_token),
            impact_payload(check),
        ));
    }

    let restored_line = recompute_restored_line(attempt.initial_stem_line, &glyphs);
    assert_eq!(
        attempt.rollback_line_diverges_from_restored_glyphs,
        !line_bits_equal(attempt.final_stem_line, restored_line),
        "independently recomputed restored-glyph line divergence",
    );
    let trace_sha256 = sha256_hex(&trace_bytes);
    rows.push(project_end_row(
        page,
        system,
        builder,
        attempt,
        plan_ordinal,
        &glyphs,
        restored_line,
        &trace_sha256,
    ));
    rows
}

#[allow(clippy::too_many_arguments)]
fn project_relation_row(
    rows: &mut Vec<String>,
    trace_bytes: &mut Vec<u8>,
    page: &str,
    system: &GateSystem<'_>,
    builder: &NativeStemsBeamBuilder,
    _attempt: &NativeStemsBeamLinkPlanAttempt,
    plan_ordinal: usize,
    step: &NativeStemsBeamExpandStep,
    check: &NativeStemsBeamHeadRelationCheck,
    glyphs: &[RetainedGlyph],
    relations: &mut Vec<ProjectedRelation>,
    stopping_index: &mut Option<usize>,
    stopping_glyphs: &mut Vec<RetainedGlyph>,
) {
    let corner = check.encountered_corner;
    let head = system.head(corner);
    let head_center = NativeStemPoint {
        x: f64::from(check.head_center.0),
        y: f64::from(check.head_center.1),
    };
    let ccw = relative_ccw(step.stem_line_before, head_center);
    let x_direction = -ccw;
    let existing = relations
        .iter()
        .position(|relation| relation.corner == corner);
    let map_ordinal = existing.unwrap_or(relations.len());
    let first_item = existing
        .map(|index| relations[index].first_item)
        .unwrap_or(step.item_index);
    let map_action = if check.accepted {
        if existing.is_some() { "replace" } else { "new" }
    } else {
        "none"
    };
    let stopping = step.stopping_check.as_ref();
    if let Some(composite_center_line) = stopping.and_then(|value| value.composite_center_line) {
        assert_eq!(
            composite_center_line,
            gate_composite(glyphs).center_line,
            "independent stopping composite center line",
        );
    }
    let stopping_side = if builder.v_y_direction < 0 {
        NativeStemHeadSide::Left
    } else {
        NativeStemHeadSide::Right
    };
    let stopping_eligible =
        check.accepted && check.derived_horizontal == stopping_side && !glyphs.is_empty();
    let composite_line = stopping
        .and_then(|value| value.composite_center_line)
        .map_or_else(|| "-".to_owned(), line_token);
    let portion = stopping
        .and_then(|value| value.stem_portion)
        .map_or("-", stem_portion_token);
    let is_end = if check.accepted {
        bool_token(stopping.is_some_and(|value| value.is_required_end))
    } else {
        "-"
    };
    let stopping_update = if check.accepted {
        bool_token(stopping.is_some_and(|value| value.became_stopping_head))
    } else {
        "-"
    };
    let stopping_snapshot = if stopping.is_some_and(|value| value.became_stopping_head) {
        retained_content_tokens(glyphs)
    } else {
        "-".to_owned()
    };
    let (
        actual_head_side,
        actual_dx,
        actual_dy,
        actual_grade,
        impact_names,
        impact_values,
        impact_weights,
        extension,
    ) = if check.accepted {
        (
            horizontal_token(check.derived_horizontal).to_owned(),
            hex_double(check.dx),
            hex_double(check.dy),
            hex_double(check.grade),
            impact_names(check).to_owned(),
            impact_values(check),
            impact_weights(check),
            check
                .extension_point
                .map_or_else(|| "-".to_owned(), point_token),
        )
    } else {
        (
            "-".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
        )
    };
    rows.push(format!(
        "stemsbeamexpandrelation {page} system {} plan {plan_ordinal} item {} cAlias {} \
         headShape {} headBounds {} headCenter {}:{} cHSide {} cVSide {} stemLine {} \
         relativeCCW {ccw} xDir {x_direction} relationHeadSide {} sideMismatch {} ref {} \
         xStem {} xGapPixels {} yGapPixels {} dx {} dy {} xKind {} xMax {} yMax {} \
         xImpactRaw {} yImpactRaw {} xImpact {} yImpact {} xWeight {} yWeight {} \
         grade {} minGrade {} accepted {} actualHeadSide {actual_head_side} actualDx {actual_dx} \
         actualDy {actual_dy} actualGrade {actual_grade} impactNames {impact_names} \
         impactValues {impact_values} impactWeights {impact_weights} extension {extension} \
         stoppingSide {} glyphsBefore {} stoppingEligible {} compositeLine {composite_line} \
         stemPortion {portion} isEnd {is_end} stoppingUpdate {stopping_update} \
         stoppingSnapshot {stopping_snapshot} mapAction {map_action} mapOrdinal {} \
         firstItem {} latestItem {}",
        system.system_id,
        step.item_index,
        system.c_alias(corner),
        head_shape_token(head.shape),
        rectangle_token(head.bounds),
        check.head_center.0,
        check.head_center.1,
        horizontal_token(check.encountered_horizontal),
        vertical_token(check.vertical),
        line_token(step.stem_line_before),
        horizontal_token(check.derived_horizontal),
        bool_token(check.horizontal_side_diverges),
        point_token(check.reference_point),
        hex_double(check.x_stem),
        hex_double(check.x_gap_pixels),
        hex_double(check.y_gap_pixels),
        hex_double(check.dx),
        hex_double(check.dy),
        match check.horizontal_gap_kind {
            NativeStemsBeamHorizontalGapKind::In => "IN",
            NativeStemsBeamHorizontalGapKind::Out => "OUT",
        },
        hex_double(check.x_maximum),
        hex_double(check.y_maximum),
        hex_double(check.raw_x_impact),
        hex_double(check.raw_y_impact),
        hex_double(check.x_impact),
        hex_double(check.y_impact),
        hex_double(check.x_weight),
        hex_double(check.y_weight),
        hex_double(check.grade),
        hex_double(0.1),
        bool_token(check.accepted),
        horizontal_token(stopping_side),
        retained_content_tokens(glyphs),
        bool_token(stopping_eligible),
        if check.accepted {
            map_ordinal.to_string()
        } else {
            "-".to_owned()
        },
        if check.accepted {
            first_item.to_string()
        } else {
            "-".to_owned()
        },
        if check.accepted {
            step.item_index.to_string()
        } else {
            "-".to_owned()
        },
    ));

    trace_bytes.extend_from_slice(
        format!(
            "relation {} {} {}{}\n",
            step.item_index,
            if check.accepted { "accept" } else { "reject" },
            relation_digest(check),
            if check.accepted {
                format!(
                    " stop={}",
                    stopping.is_some_and(|value| value.became_stopping_head)
                )
            } else {
                String::new()
            },
        )
        .as_bytes(),
    );

    if check.accepted {
        if let Some(index) = existing {
            relations[index].latest_item = step.item_index;
            relations[index].check = check.clone();
        } else {
            relations.push(ProjectedRelation {
                corner,
                first_item: step.item_index,
                latest_item: step.item_index,
                check: check.clone(),
            });
        }
        if stopping.is_some_and(|value| value.became_stopping_head) {
            *stopping_index = Some(step.item_index);
            *stopping_glyphs = glyphs.to_vec();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn project_update_row(
    rows: &mut Vec<String>,
    trace_bytes: &mut Vec<u8>,
    page: &str,
    system: &GateSystem<'_>,
    builder: &NativeStemsBeamBuilder,
    plan_ordinal: usize,
    step: &NativeStemsBeamExpandStep,
    update: &NativeStemsBeamGlyphUpdate,
    glyphs: &mut Vec<RetainedGlyph>,
) {
    let item = &builder.items[step.item_index];
    let item_ref = system.item_ref(item, step.item_index);
    let before = retained_source_tokens(system, builder, glyphs);
    let (
        attempted,
        canonical,
        action,
        composite_bounds,
        composite_weight,
        composite_runs,
        composite_centroid,
        intersection,
        line_shift,
    ) = match update {
        NativeStemsBeamGlyphUpdate::NoGlyph => (
            "-".to_owned(),
            "-".to_owned(),
            "null",
            "-".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
        ),
        NativeStemsBeamGlyphUpdate::DuplicateStructuralGlyph {
            attempted,
            retained,
            structural_digest: digest,
        } => {
            let attempted_glyph = system.resolve_glyph(builder, *attempted);
            assert_eq!(*attempted, attempted_glyph.reference);
            assert_eq!(
                *digest,
                structural_digest(attempted_glyph.bounds, &attempted_glyph.run_table),
                "duplicate structural digest",
            );
            let retained = glyphs
                .iter()
                .find(|candidate| candidate.glyph.reference == *retained)
                .expect("retained duplicate");
            (
                item_ref.clone(),
                system.item_ref(&builder.items[retained.source_item], retained.source_item),
                "equalSkip",
                "-".to_owned(),
                "-".to_owned(),
                "-".to_owned(),
                "-".to_owned(),
                "-".to_owned(),
                "-".to_owned(),
            )
        }
        NativeStemsBeamGlyphUpdate::Added {
            glyph,
            composite_bounds,
            composite_weight,
            composite_key,
            composite_centroid,
            line_intersection,
            shift_x,
            structural_digest: glyph_digest,
            insertion_ordinal,
            composite_digest,
        } => {
            let retained = RetainedGlyph {
                glyph: system.resolve_glyph(builder, *glyph),
                source_item: step.item_index,
            };
            assert_eq!(
                *glyph_digest,
                structural_digest(retained.glyph.bounds, &retained.glyph.run_table),
                "inserted structural digest",
            );
            assert_eq!(*insertion_ordinal, glyphs.len());
            glyphs.push(retained.clone());
            let independent = gate_composite(glyphs);
            assert_eq!(*composite_bounds, independent.bounds);
            assert_eq!(*composite_weight, independent.weight);
            assert_eq!(composite_key.left, independent.bounds.x);
            assert_eq!(composite_key.top, independent.bounds.y);
            assert_eq!(composite_key.run_table, independent.run_table);
            assert_eq!(
                *composite_digest,
                structural_digest(independent.bounds, &independent.run_table),
                "composite structural digest",
            );
            assert_eq!(*composite_centroid, independent.centroid);
            let independent_intersection =
                intersection_at_y(step.stem_line_before, independent.centroid.y);
            assert_eq!(*line_intersection, independent_intersection);
            assert_eq!(
                shift_x.to_bits(),
                (independent.centroid.x - independent_intersection.x).to_bits(),
            );
            (
                item_ref.clone(),
                "-".to_owned(),
                "insert",
                bounds_token(*composite_bounds),
                composite_weight.to_string(),
                glyph_run_sha256(&composite_key.run_table),
                point_token(*composite_centroid),
                point_token(*line_intersection),
                hex_double(*shift_x),
            )
        }
    };
    let after = retained_source_tokens(system, builder, glyphs);
    rows.push(format!(
        "stemsbeamexpandupdate {page} system {} plan {plan_ordinal} item {} itemRef {item_ref} \
         attempted {attempted} canonicalBefore {canonical} action {action} glyphsBefore {before} \
         glyphsAfter {after} lineBefore {} lineAfter {} compositeBounds {composite_bounds} \
         compositeWeight {composite_weight} compositeRuns {composite_runs} \
         compositeCentroid {composite_centroid} intersection {intersection} lineShift {line_shift}",
        system.system_id,
        step.item_index,
        line_token(step.stem_line_before),
        line_token(step.stem_line_after),
    ));
    trace_bytes.extend_from_slice(
        format!(
            "update {attempted} {canonical} {action} {} {}\n",
            line_bits_token(step.stem_line_before),
            line_bits_token(step.stem_line_after),
        )
        .as_bytes(),
    );
}

fn impact_names(check: &NativeStemsBeamHeadRelationCheck) -> &'static str {
    match check.horizontal_gap_kind {
        NativeStemsBeamHorizontalGapKind::In => "xInGap,yGap",
        NativeStemsBeamHorizontalGapKind::Out => "xOutGap,yGap",
    }
}

fn impact_values(check: &NativeStemsBeamHeadRelationCheck) -> String {
    format!(
        "{},{}",
        hex_double(check.x_impact),
        hex_double(check.y_impact)
    )
}

fn impact_weights(check: &NativeStemsBeamHeadRelationCheck) -> String {
    format!(
        "{},{}",
        hex_double(check.x_weight),
        hex_double(check.y_weight)
    )
}

fn impact_payload(check: &NativeStemsBeamHeadRelationCheck) -> String {
    format!(
        "{}:{}:{}",
        impact_names(check),
        impact_values(check),
        impact_weights(check)
    )
}

fn recompute_restored_line(
    initial: NativeStemLine,
    final_glyphs: &[RetainedGlyph],
) -> NativeStemLine {
    let mut line = initial;
    let mut retained = Vec::new();
    for glyph in final_glyphs {
        retained.push(glyph.clone());
        update_line_from_retained(&retained, &mut line);
    }
    line
}

#[allow(clippy::too_many_arguments)]
fn project_end_row(
    page: &str,
    system: &GateSystem<'_>,
    builder: &NativeStemsBeamBuilder,
    attempt: &NativeStemsBeamLinkPlanAttempt,
    plan_ordinal: usize,
    glyphs: &[RetainedGlyph],
    restored_line: NativeStemLine,
    trace_sha256: &str,
) -> String {
    let last_index_token = attempt
        .expand_last_index
        .map_or_else(|| "-".to_owned(), |value| value.to_string());
    let stopping_index_token = attempt
        .stopping_head_item_index
        .map_or_else(|| "-".to_owned(), |value| value.to_string());
    let relation_tokens = list_token(
        attempt
            .relations
            .iter()
            .map(|relation| system.c_alias(relation.corner)),
    );
    let glyph_tokens = retained_content_tokens(glyphs);
    let post_return = list_token(attempt.relations.iter().filter_map(|relation| {
        attempt.expand_last_index.and_then(|last| {
            (last >= 0 && relation.latest_item_index as i32 > last).then(|| {
                format!(
                    "{}@{}",
                    system.c_alias(relation.corner),
                    relation.latest_item_index
                )
            })
        })
    }));

    let mut terminal_kind = "-".to_owned();
    let mut terminal_c = "-".to_owned();
    let mut terminal_relation_side = "-".to_owned();
    let mut terminal_portion = "-".to_owned();
    let mut terminal_correct_side_end = false;
    if let Some(last) = attempt.expand_last_index.filter(|&value| value >= 0) {
        if let Some(item) = usize::try_from(last)
            .ok()
            .and_then(|index| builder.items.get(index))
        {
            terminal_kind = item_kind_token(item.kind).to_owned();
            if let Some(NativeStemsBeamBuilderTargetRef::Head(corner)) = item.target {
                terminal_c = system.c_alias(corner);
                if let Some(relation) = attempt
                    .relations
                    .iter()
                    .find(|relation| relation.corner == corner)
                {
                    terminal_relation_side =
                        horizontal_token(relation.check.derived_horizontal).to_owned();
                    terminal_correct_side_end =
                        attempt.stopping_head_item_index == usize::try_from(last).ok();
                    if terminal_correct_side_end {
                        terminal_portion = if builder.v_y_direction > 0 {
                            "STEM_BOTTOM"
                        } else {
                            "STEM_TOP"
                        }
                        .to_owned();
                    }
                }
            }
        }
    }

    let visits = attempt.trace.len();
    let gaps = attempt
        .trace
        .iter()
        .filter(|step| step.item_kind == NativeStemsBeamBuilderItemKind::Gap)
        .count();
    let show_stopping_gaps = attempt
        .trace
        .iter()
        .filter_map(|step| step.gap.as_ref())
        .filter(|gap| gap.show_stopping)
        .count();
    let separation_checks = attempt
        .trace
        .iter()
        .filter(|step| step.separation.is_some())
        .count();
    let separation_stops = attempt
        .trace
        .iter()
        .filter_map(|step| step.separation.as_ref())
        .filter(|separation| separation.separated)
        .count();
    let relation_attempts = attempt
        .trace
        .iter()
        .filter(|step| step.relation_check.is_some())
        .count();
    let relation_accepts = attempt
        .trace
        .iter()
        .filter_map(|step| step.relation_check.as_ref())
        .filter(|check| check.accepted)
        .count();
    let relation_rejects = relation_attempts - relation_accepts;
    let stopping_updates = attempt
        .trace
        .iter()
        .filter_map(|step| step.stopping_check.as_ref())
        .filter(|check| check.became_stopping_head)
        .count();
    let glyph_update_calls = attempt
        .trace
        .iter()
        .filter(|step| step.glyph_update.is_some())
        .count();
    let glyph_insertions = attempt
        .trace
        .iter()
        .filter_map(|step| step.glyph_update.as_ref())
        .filter(|update| matches!(update, NativeStemsBeamGlyphUpdate::Added { .. }))
        .count();
    let glyph_equal_skips = attempt
        .trace
        .iter()
        .filter_map(|step| step.glyph_update.as_ref())
        .filter(|update| {
            matches!(
                update,
                NativeStemsBeamGlyphUpdate::DuplicateStructuralGlyph { .. }
            )
        })
        .count();
    let stored_shift = attempt.stored_theoretical_line_after.start.x
        - attempt.stored_theoretical_line_before.start.x;
    let stored_theo_mutated = !line_bits_equal(
        attempt.stored_theoretical_line_before,
        attempt.stored_theoretical_line_after,
    );
    let attachment_aliases = system.attachment_aliases_v_theoretical_line(builder.start);
    let builder_aliases = builder.v_builder_assignment == builder.start;
    let attachment_mutated = attachment_aliases && stored_theo_mutated;
    assert_eq!(
        attempt.stored_theoretical_line_would_mutate, stored_theo_mutated,
        "stored theoretical-line mutation projection",
    );
    assert_eq!(
        attempt.attachment_aliases_stored_theoretical_line, attachment_aliases,
        "beam attachment alias topology",
    );
    assert_eq!(
        attempt.builder_line_aliases_stored_theoretical_line, builder_aliases,
        "builder/V theoretical-line alias topology",
    );
    assert_eq!(
        attempt.attachment_alias_would_mutate, attachment_mutated,
        "beam attachment line mutation projection",
    );

    format!(
        "stemsbeamexpandend {page} system {} plan {plan_ordinal} builder {} outcome {} \
         expandInvoked {} lastIndex {last_index_token} relationCount {} relations {relation_tokens} \
         glyphCount {} glyphs {glyph_tokens} stoppingIndex {stopping_index_token} stopCause {} \
         relationsPastReturn {post_return} storedTheoBefore {} storedTheoAfter {} \
         storedTheoMutated {} attachmentAliasesTheo {} builderAliasesTheo {} \
         attachmentLineMutated {} storedTheoShiftDx {} terminalKind {terminal_kind} \
         terminalC {terminal_c} terminalRelationSide {terminal_relation_side} \
         terminalPortion {terminal_portion} terminalCorrectSideEnd {} \
         beamSideReadyWithoutStoppingHead {} finalStemLine {} restoredGlyphLine {} \
         rollbackLineDiverges {} traceSha256 {trace_sha256} visits {visits} gaps {gaps} \
         showStoppingGaps {show_stopping_gaps} separationChecks {separation_checks} \
         separationStops {separation_stops} relationAttempts {relation_attempts} \
         relationAccepts {relation_accepts} relationRejects {relation_rejects} \
         stoppingUpdates {stopping_updates} glyphUpdateCalls {glyph_update_calls} \
         glyphInsertions {glyph_insertions} glyphEqualSkips {glyph_equal_skips} \
         sigVertexDelta 0 sigEdgeDelta 0 stemInterDelta 0 systemStemDelta 0 \
         glyphIndexDelta 0 filamentIndexDelta 0 linkMutations 0 builderMutations 0",
        system.system_id,
        builder.builder_ordinal,
        outcome_token(attempt.outcome),
        bool_token(attempt.head_target_count != 0),
        attempt.relations.len(),
        attempt.glyphs.len(),
        stop_cause_token(attempt.stop_cause),
        line_token(attempt.stored_theoretical_line_before),
        line_token(attempt.stored_theoretical_line_after),
        bool_token(stored_theo_mutated),
        bool_token(attachment_aliases),
        bool_token(builder_aliases),
        bool_token(attachment_mutated),
        hex_double(stored_shift),
        bool_token(terminal_correct_side_end),
        bool_token(attempt.beam_side_ready_without_stopping_head),
        line_token(attempt.final_stem_line),
        line_token(restored_line),
        bool_token(attempt.rollback_line_diverges_from_restored_glyphs),
    )
}

fn project_native_rows(page_key: &str, page: &NativePage) -> Vec<String> {
    let system_count = page.plans.systems.len();
    assert_eq!(page.stem_seeds.systems.len(), system_count);
    assert_eq!(page.heads.epilog.systems.len(), system_count);
    assert_eq!(page.corners.systems.len(), system_count);
    assert_eq!(page.beam_stumps.systems.len(), system_count);
    assert_eq!(page.beam_vlinkers.systems.len(), system_count);
    assert_eq!(page.beam_reachability.systems.len(), system_count);
    assert_eq!(page.head_stumps.systems.len(), system_count);
    assert_eq!(page.beam_builders.systems.len(), system_count);
    assert!(
        page.beams.music_font_scale.is_some(),
        "beam-expand corpus requires the measured music-family scale",
    );

    let mut rows = vec![format!(
        "stemsbeamexpandpage {page_key} systems {system_count} staves {} family Bravura",
        page.grid.staves.len(),
    )];
    let mut page_totals = GateTotals::default();
    let mut page_hash = Fnv64::default();

    for (system_index, plan_system) in page.plans.systems.iter().enumerate() {
        let system = GateSystem::new(page, system_index);
        let profile = system.stump_system.profile;
        let stub_profile = system.seed_system.raw.profile;
        assert_eq!(plan_system.system_id, system.system_id);
        assert_eq!(plan_system.interline, system.builder_system.interline);
        assert_eq!(plan_system.interline, system.stump_system.interline);
        assert_eq!(plan_system.link_profile, profile);
        assert_eq!(stub_profile, profile);
        assert_eq!(
            plan_system.min_linker_length,
            (f64::from(plan_system.interline) * 0.85).round_ties_even() as i32,
        );
        assert_eq!(
            page.heads.epilog.systems[system_index].system_id,
            system.system_id,
        );
        assert!(
            page.heads.epilog.systems[system_index]
                .heads_in_sig_order
                .len()
                >= system.corner_system.heads_in_sig_order.len(),
            "the corner product is the stem-capable Java head subset",
        );
        assert_eq!(
            system.reachability_system.beam_sources_in_inspection_order,
            system
                .stump_system
                .beams_by_abscissa
                .iter()
                .map(|beam| beam.source)
                .collect::<Vec<_>>(),
        );

        let beam_sig_order = list_token(
            system
                .stump_system
                .beams_by_abscissa
                .iter()
                .map(|beam| beam.sig_ordinal.to_string()),
        );
        let system_row = format!(
            "stemsbeamexpandsystem {page_key} system {} profile {profile} stubProfile \
             {stub_profile} interline {} minLinkerLength {} beams {} heads {} beamSigOrder \
             {beam_sig_order}",
            system.system_id,
            plan_system.interline,
            plan_system.min_linker_length,
            system.stump_system.beams_by_abscissa.len(),
            system.corner_system.heads_in_sig_order.len(),
        );
        rows.push(system_row.clone());
        let mut system_hash = Fnv64::default();
        system_hash.add(&system_row);
        page_hash.add(&system_row);

        assert_eq!(
            plan_system.builders.len(),
            system.builder_system.builders.len(),
        );
        let mut plan_ordinal = 0_usize;
        let mut system_totals = GateTotals::default();
        for (plan_builder, native_builder) in plan_system
            .builders
            .iter()
            .zip(&system.builder_system.builders)
        {
            assert_eq!(plan_builder.builder_ordinal, native_builder.builder_ordinal);
            assert_eq!(plan_builder.start, native_builder.start);
            assert_eq!(
                plan_builder.construction_max_profile,
                native_builder.max_stem_profile,
            );
            assert_eq!(
                plan_builder.attempts.len(),
                usize::try_from(native_builder.max_stem_profile + 1)
                    .expect("nonnegative construction profile"),
            );
            for attempt in &plan_builder.attempts {
                let attempt_rows =
                    project_attempt_rows(page_key, &system, native_builder, attempt, plan_ordinal);
                for row in attempt_rows {
                    system_hash.add(&row);
                    page_hash.add(&row);
                    rows.push(row);
                }
                system_totals.add_attempt(attempt);
                plan_ordinal += 1;
            }
        }
        system_totals.systems = 1;
        system_totals.builders = plan_system.builders.len();
        let summary = format!(
            "stemsbeamexpandsystemsummary {page_key} system {} {} hash {:016x}",
            system.system_id,
            system_totals.fields(),
            system_hash.0,
        );
        page_hash.add(&summary);
        rows.push(summary);
        page_totals.include(system_totals);
    }

    rows.push(format!(
        "stemsbeamexpandpagesummary {page_key} systems {system_count} {} hash {:016x}",
        page_totals.fields(),
        page_hash.0,
    ));
    rows
}

fn assert_exact_semantic_rows(
    fixture: &OracleFixture<'_>,
    actual: &[String],
) -> Result<(), String> {
    let expected = fixture
        .rows
        .iter()
        .filter(|row| row.family != Family::CorpusSummary)
        .map(|row| row.raw)
        .collect::<Vec<_>>();
    if expected.len() != actual.len() {
        return Err(format!(
            "{} semantic row count differs: Java {} != Rust {}",
            fixture.page,
            expected.len(),
            actual.len(),
        ));
    }
    if let Some((index, (java, rust))) = expected
        .iter()
        .zip(actual)
        .enumerate()
        .find(|(_, (java, rust))| **java != rust.as_str())
    {
        return Err(format!(
            "{} first semantic mismatch at body row {}:\nJava: {}\nRust: {}",
            fixture.page,
            index + 1,
            java,
            rust,
        ));
    }
    Ok(())
}

fn line_count(bytes: &[u8]) -> usize {
    bytes.iter().filter(|&&byte| byte == b'\n').count()
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

struct FixtureSlices<'a> {
    body: &'a [u8],
    header: &'a [u8],
    semantic: &'a [u8],
}

fn fixture_slices(bytes: &[u8]) -> Result<FixtureSlices<'_>, String> {
    let trailer_marker = b"stemsbeamexpandcorpussummary ";
    let trailer_start = find_bytes(bytes, trailer_marker)
        .ok_or_else(|| "fixture lacks corpus-summary trailer".to_owned())?;
    if find_bytes(
        &bytes[trailer_start + trailer_marker.len()..],
        trailer_marker,
    )
    .is_some()
    {
        return Err("fixture has duplicate corpus-summary trailer".to_owned());
    }
    let body = &bytes[..trailer_start];
    let page_marker = b"stemsbeamexpandpage ";
    let semantic_start =
        find_bytes(body, page_marker).ok_or_else(|| "fixture body lacks page row".to_owned())?;
    if !body.ends_with(b"\n") || !bytes.ends_with(b"\n") {
        return Err("fixture/body must end in a newline".to_owned());
    }
    Ok(FixtureSlices {
        body,
        header: &body[..semantic_start],
        semantic: &body[semantic_start..],
    })
}

fn validate_fixture_algebra(entry: &ManifestEntry, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() != entry.fixture_bytes
        || line_count(bytes) != entry.fixture_lines
        || sha256_hex(bytes) != entry.fixture_sha256
    {
        return Err(format!("{} fixture fingerprint differs", entry.page));
    }
    let body = fixture_slices(bytes)?.body;
    if body.len() != entry.body_bytes
        || line_count(body) != entry.body_lines
        || sha256_hex(body) != entry.body_sha256
    {
        return Err(format!("{} emitted body fingerprint differs", entry.page));
    }
    let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let fixture = OracleFixture::parse(text)?;
    if fixture.page != entry.page {
        return Err(format!("{} split fixture page differs", entry.page));
    }
    let mut row_counts = [0_usize; 12];
    for row in &fixture.rows {
        if let Some(index) = row.family.row_count_index() {
            row_counts[index] += 1;
        }
    }
    if row_counts != entry.row_counts {
        return Err(format!(
            "{} row-count algebra differs: {:?} != {:?}",
            entry.page, row_counts, entry.row_counts,
        ));
    }
    let page_summary = fixture
        .rows
        .iter()
        .find(|row| row.family == Family::PageSummary)
        .ok_or_else(|| format!("{} lacks page summary", entry.page))?;
    if page_summary.value("hash")? != entry.page_hash
        || page_summary.number::<usize>("plans")? != row_counts[2]
        || page_summary.number::<usize>("relations")? != row_counts[7]
        || page_summary.number::<usize>("glyphs")? != row_counts[8]
        || row_counts[2] != row_counts[9]
    {
        return Err(format!("{} page-summary row algebra differs", entry.page));
    }
    let mut outcomes = [0_usize; 5];
    let mut post_return_relations = 0_usize;
    let mut rollback_divergences = 0_usize;
    let mut stored_mutations = 0_usize;
    let mut attachment_mutations = 0_usize;
    for end in fixture.rows.iter().filter(|row| row.family == Family::End) {
        let outcome = match end.value("outcome")? {
            "NoHeadTarget" => 0,
            "ExpandFailed" => 1,
            "NoRelations" => 2,
            "NoGlyphs" => 3,
            "ReadyForCreateStem" => 4,
            value => return Err(format!("{} unknown outcome {value:?}", entry.page)),
        };
        outcomes[outcome] += 1;
        let past = end.value("relationsPastReturn")?;
        post_return_relations += if past == "-" {
            0
        } else {
            past.split(',').count()
        };
        rollback_divergences += usize::from(end.value("rollbackLineDiverges")? == "true");
        stored_mutations += usize::from(end.value("storedTheoMutated")? == "true");
        attachment_mutations += usize::from(end.value("attachmentLineMutated")? == "true");
    }
    if outcomes
        != [
            page_summary.number("noHeadTarget")?,
            page_summary.number("expandFailed")?,
            page_summary.number("noRelations")?,
            page_summary.number("noGlyphs")?,
            page_summary.number("ready")?,
        ]
        || post_return_relations != page_summary.number("postReturnRelations")?
        || rollback_divergences != page_summary.number("rollbackLineDivergences")?
        || stored_mutations != page_summary.number("storedTheoMutations")?
        || attachment_mutations != page_summary.number("attachmentLineMutations")?
    {
        return Err(format!("{} end/page-summary totals differ", entry.page));
    }
    let side_mismatches = fixture
        .rows
        .iter()
        .filter(|row| row.family == Family::Relation)
        .filter_map(|row| row.value("sideMismatch").ok())
        .filter(|&value| value == "true")
        .count();
    if side_mismatches != page_summary.number("relationSideMismatches")? {
        return Err(format!(
            "{} relation-side mismatch total differs",
            entry.page
        ));
    }

    let trailer = fixture
        .rows
        .last()
        .filter(|row| row.family == Family::CorpusSummary)
        .ok_or_else(|| format!("{} lacks corpus trailer", entry.page))?;
    let row_count_token = entry
        .row_counts
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(":");
    if trailer.value("schema")? != "stems-beam-expand-v1"
        || trailer.value("mode")? != "page"
        || trailer.number::<usize>("pages")? != 1
        || trailer.value("pageRefs")? != entry.page
        || trailer.value("rowCounts")? != row_count_token
        || trailer.value("probeSourceSha256")? != EXPECTED_PROBE_SHA256
        || trailer.value("runnerSourceSha256")? != EXPECTED_RUNNER_SHA256
        || trailer.value("emittedBodySha256")? != entry.body_sha256
        || trailer.number::<usize>("emittedBodyLines")? != entry.body_lines
        || trailer.number::<usize>("emittedBodyBytes")? != entry.body_bytes
    {
        return Err(format!("{} corpus trailer algebra differs", entry.page));
    }
    Ok(())
}

fn native_page(image: &str) -> NativePage {
    let path = repo_root().join("data/examples").join(image);
    let grid =
        recognize_grid_lines(path).unwrap_or_else(|error| panic!("{image}: GRID failed: {error}"));
    let headers = recognize_native_headers(&grid)
        .unwrap_or_else(|error| panic!("{image}: HEADERS failed: {error}"));
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers)
        .unwrap_or_else(|error| panic!("{image}: STEM_SEEDS failed: {error}"));
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .unwrap_or_else(|error| panic!("{image}: BEAMS failed: {error}"));
    let ledgers = recognize_native_ledgers(&grid, &beams)
        .unwrap_or_else(|error| panic!("{image}: LEDGERS failed: {error}"));
    let heads = recognize_native_heads(&grid, &headers, &stem_seeds, &beams, &ledgers)
        .unwrap_or_else(|error| panic!("{image}: HEADS failed: {error}"));
    let corners = materialize_native_stems_head_corners(&heads, &stem_seeds)
        .unwrap_or_else(|error| panic!("{image}: STEMS corners failed: {error}"));
    let head_seeds = materialize_native_stems_head_seeds(&grid, &stem_seeds, &corners)
        .unwrap_or_else(|error| panic!("{image}: STEMS head seeds failed: {error}"));
    let beam_stumps =
        materialize_native_stems_beam_stumps(&grid, &beams, &heads, &stem_seeds, &head_seeds)
            .unwrap_or_else(|error| panic!("{image}: STEMS beam stumps failed: {error}"));
    let beam_vlinkers =
        materialize_native_stems_beam_vlinkers(&grid, &beams, &stem_seeds, &beam_stumps)
            .unwrap_or_else(|error| panic!("{image}: STEMS beam VLinkers failed: {error}"));
    let beam_reachability =
        materialize_native_stems_beam_reachability(&beams, &beam_stumps, &beam_vlinkers, &corners)
            .unwrap_or_else(|error| panic!("{image}: STEMS beam reachability failed: {error}"));
    let head_stumps =
        materialize_native_stems_head_stumps(&grid, &stem_seeds, &corners, &head_seeds)
            .unwrap_or_else(|error| panic!("{image}: STEMS head stumps failed: {error}"));
    let beam_builders = materialize_native_stems_beam_builders(
        &grid,
        &beams,
        &ledgers,
        &heads,
        &stem_seeds,
        &beam_stumps,
        &beam_vlinkers,
        &corners,
        &head_stumps,
        &beam_reachability,
    )
    .unwrap_or_else(|error| panic!("{image}: STEMS beam builders failed: {error}"));
    let head_reachability = materialize_native_stems_head_corner_reachability(
        &grid,
        &stem_seeds,
        &heads,
        &corners,
        &head_seeds,
        &head_stumps,
        &beam_stumps,
        &beam_vlinkers,
        &beam_reachability,
    )
    .unwrap_or_else(|error| panic!("{image}: STEMS head reachability failed: {error}"));
    let head_builders = materialize_native_stems_head_builders(
        &grid,
        &beams,
        &ledgers,
        &heads,
        &stem_seeds,
        &beam_stumps,
        &beam_vlinkers,
        &head_stumps,
        &beam_builders,
        &head_reachability,
        INSPECT_PROFILE,
    )
    .unwrap_or_else(|error| panic!("{image}: STEMS head builders failed: {error}"));
    let plans = materialize_native_stems_beam_link_plans(
        &stem_seeds,
        &beam_stumps,
        &beam_vlinkers,
        &corners,
        &head_stumps,
        &beam_reachability,
        &beam_builders,
        &head_builders,
    )
    .unwrap_or_else(|error| panic!("{image}: STEMS beam link plans failed: {error}"));
    NativePage {
        grid,
        stem_seeds,
        beams,
        heads,
        corners,
        beam_stumps,
        beam_vlinkers,
        beam_reachability,
        head_stumps,
        beam_builders,
        head_builders,
        plans,
    }
}

/// Active release gate over the frozen eight-page Java corpus. Every manifest,
/// source, fixture, body, row-family, summary, trailer, and projected semantic
/// row is checked independently and fail-closed.
#[test]
fn native_stems_beam_link_plans_match_java_corpus_exactly() {
    let root = repo_root();
    let manifest_bytes =
        std::fs::read(root.join(MANIFEST_PATH)).expect("frozen beam-expand manifest is installed");
    assert_eq!(sha256_hex(&manifest_bytes), EXPECTED_MANIFEST_SHA256);
    let manifest = BeamExpandManifest::parse(&manifest_bytes)
        .unwrap_or_else(|error| panic!("invalid beam-expand manifest: {error}"));
    assert_eq!(manifest.probe_sha256, EXPECTED_PROBE_SHA256);
    assert_eq!(manifest.runner_sha256, EXPECTED_RUNNER_SHA256);
    assert_eq!(
        sha256_hex(&std::fs::read(root.join(PROBE_PATH)).expect("beam-expand probe source")),
        EXPECTED_PROBE_SHA256,
    );
    assert_eq!(
        sha256_hex(&std::fs::read(root.join(RUNNER_PATH)).expect("beam-expand runner source")),
        EXPECTED_RUNNER_SHA256,
    );
    let manifest_summary_start = find_bytes(&manifest_bytes, b"stemsbeamexpandmanifestsummary ")
        .expect("manifest summary row");
    let manifest_body = &manifest_bytes[..manifest_summary_start];
    assert_eq!(sha256_hex(manifest_body), manifest.body_sha256);
    assert_eq!(line_count(manifest_body), manifest.body_lines);
    assert_eq!(manifest_body.len(), manifest.body_bytes);

    // Reconstruct exactly what the full runner emitted: one common header,
    // then every page body without any per-page corpus trailer.
    let mut full_body = Vec::with_capacity(EXPECTED_FULL_BODY_BYTES);
    let mut common_header = None::<Vec<u8>>;
    let mut summed_row_counts = [0_usize; 12];
    for entry in &manifest.entries {
        let bytes = std::fs::read(root.join("rust/oracle").join(&entry.fixture))
            .unwrap_or_else(|error| panic!("{}: missing fixture: {error}", entry.page));
        validate_fixture_algebra(entry, &bytes)
            .unwrap_or_else(|error| panic!("{}: {error}", entry.page));
        let slices =
            fixture_slices(&bytes).unwrap_or_else(|error| panic!("{}: {error}", entry.page));
        let header = slices.header;
        let semantic = slices.semantic;
        if let Some(expected) = &common_header {
            assert_eq!(header, expected, "{} common fixture header", entry.page);
        } else {
            common_header = Some(header.to_vec());
            full_body.extend_from_slice(header);
        }
        full_body.extend_from_slice(semantic);
        for (total, count) in summed_row_counts.iter_mut().zip(entry.row_counts) {
            *total += count;
        }
    }
    assert_eq!(summed_row_counts, EXPECTED_FULL_ROW_COUNTS);
    assert_eq!(full_body.len(), EXPECTED_FULL_BODY_BYTES);
    assert_eq!(line_count(&full_body), EXPECTED_FULL_BODY_LINES);
    assert_eq!(sha256_hex(&full_body), EXPECTED_FULL_BODY_SHA256);
    drop(full_body);

    for (entry, spec) in manifest.entries.iter().zip(PAGES) {
        let text = std::fs::read_to_string(root.join("rust/oracle").join(&entry.fixture))
            .unwrap_or_else(|error| panic!("{}: missing fixture: {error}", entry.page));
        let fixture = OracleFixture::parse(&text)
            .unwrap_or_else(|error| panic!("{}: invalid fixture: {error}", entry.page));
        assert_eq!(fixture.page, spec.page, "split fixture page");
        let actual = native_page(spec.image);
        assert_projection_topology(&fixture, &actual.plans)
            .unwrap_or_else(|error| panic!("{}: {error}", spec.page));
        let projected = project_native_rows(spec.page, &actual);
        assert_exact_semantic_rows(&fixture, &projected)
            .unwrap_or_else(|error| panic!("{}: {error}", spec.page));
    }
}

/// Development bridge to a freshly generated single-page stream. The path is
/// deliberately supplied at run time so an uninstalled `/private/tmp` oracle
/// can never become an accidental checked-in dependency.
#[test]
#[ignore = "set AUDIVERIS_BEAM_EXPAND_ORACLE to a final Chula stream"]
fn native_stems_beam_link_plans_match_fresh_chula_rows_exactly() {
    let path = std::env::var("AUDIVERIS_BEAM_EXPAND_ORACLE")
        .expect("AUDIVERIS_BEAM_EXPAND_ORACLE must name the Chula stream");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {path}: {error}"));
    let fixture = OracleFixture::parse(&text)
        .unwrap_or_else(|error| panic!("invalid fresh Chula stream: {error}"));
    assert_eq!(fixture.page, "chula.png#1");
    let page = native_page("chula.png");
    assert_projection_topology(&fixture, &page.plans)
        .unwrap_or_else(|error| panic!("fresh Chula topology: {error}"));
    let projected = project_native_rows(fixture.page, &page);
    assert_exact_semantic_rows(&fixture, &projected)
        .unwrap_or_else(|error| panic!("fresh Chula exact projection: {error}"));
}

/// Faster aggregate-only checkpoint for local iteration. Kept ignored because
/// the active exact corpus gate is authoritative.
#[test]
#[ignore = "expensive Chula end-to-end checkpoint"]
fn native_stems_beam_link_plans_match_chula_aggregate_checkpoint() {
    let page = native_page("chula.png");
    let actual = &page.plans;
    assert_eq!(actual.systems.len(), 3);
    assert_eq!(actual.builder_count, 354);
    assert_eq!(actual.attempt_count, 1_735);
    assert_eq!(actual.no_head_target_count, 625);
    assert_eq!(actual.expand_failed_count, 100);
    assert_eq!(actual.no_relations_count, 2);
    assert_eq!(actual.no_glyphs_count, 14);
    assert_eq!(actual.ready_for_create_stem_count, 994);
    assert_eq!(actual.relation_count, 2_034);
    assert_eq!(actual.selected_glyph_count, 1_478);
    assert_eq!(actual.relations_past_return_count, 0);
    assert_eq!(actual.rollback_line_divergence_count, 0);
    assert_eq!(actual.stored_theoretical_line_delta_count, 161);
    assert_eq!(actual.beam_side_ready_without_stopping_head_count, 9);
    assert_eq!(actual.beam_side_ready_beyond_stopping_head_count, 82);
    assert_eq!(actual.beam_side_ready_at_stopping_head_count, 87);
    assert_eq!(actual.forbidden_mutation_count, 0);

    let attempts = actual
        .systems
        .iter()
        .flat_map(|system| &system.builders)
        .flat_map(|builder| &builder.attempts)
        .collect::<Vec<_>>();
    let side_mismatches = attempts
        .iter()
        .flat_map(|attempt| &attempt.trace)
        .filter_map(|step| step.relation_check.as_ref())
        .filter(|check| check.horizontal_side_diverges)
        .count();
    let attachment_mutations = attempts
        .iter()
        .filter(|attempt| attempt.attachment_alias_would_mutate)
        .count();
    let max_abs_stored_shift = attempts.iter().fold(0.0_f64, |maximum, attempt| {
        maximum.max(
            (attempt.stored_theoretical_line_after.start.x
                - attempt.stored_theoretical_line_before.start.x)
                .abs(),
        )
    });
    assert_eq!(side_mismatches, 2);
    assert_eq!(attachment_mutations, 161);
    assert_eq!(max_abs_stored_shift.to_bits(), 0x4020_f8e6_0e2a_3f80);
}

#[test]
fn beam_expand_row_parser_rejects_schema_drift() {
    let page = "stemsbeamexpandpage chula.png#1 systems 3 staves 6 family Bravura";
    let parsed = parse_row(page, 1).expect("canonical page row");
    assert_eq!(parsed.family, Family::Page);
    assert_eq!(parsed.page, "chula.png#1");
    assert!(
        parse_row(
            "stemsbeamexpandpage chula.png#1 systems 3 staves 6 renamed Bravura",
            1,
        )
        .is_err(),
        "renamed field fails closed",
    );
    assert!(
        parse_row("stemsbeamexpandmystery chula.png#1 field value", 1).is_err(),
        "unknown family fails closed",
    );
    assert!(
        OracleFixture::parse(page).is_err(),
        "missing schema header and hierarchy fail closed",
    );
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}
