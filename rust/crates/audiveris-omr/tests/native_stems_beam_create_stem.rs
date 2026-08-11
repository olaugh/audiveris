// SPDX-License-Identifier: AGPL-3.0-or-later

//! Fail-closed differential gate for the first persistent beam-origin
//! `StemBuilder.createStem` transaction.
//!
//! The Java corpus executes each target system from a fresh post-HEADS sheet.
//! Consequently only system 1 carries a natural sheet-first numeric-ID
//! chronology; later rows are isolated target-system snapshots and are never
//! concatenated into a fictitious page-wide allocator history.

#![allow(dead_code)]

use std::{collections::BTreeMap, fmt::Write as _, path::PathBuf};

use audiveris_image::{
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable},
    section::Bounds,
};
use audiveris_omr::{
    head_scanner_slices::JavaRectangle,
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads,
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::{NativeStemSeedRecognition, recognize_native_stem_seeds},
    native_stems_beam_builders::{
        NativeStemsBeamBuilderRecognition, materialize_native_stems_beam_builders,
    },
    native_stems_beam_link_plans::{
        NativeStemsBeamLinkPlanAttempt, NativeStemsBeamLinkPlanRecognition,
        NativeStemsBeamLinkPlanSystem, materialize_native_stems_beam_link_plans,
    },
    native_stems_beam_reachability::materialize_native_stems_beam_reachability,
    native_stems_beam_scheduler::{
        NativeStemsBeamDeferredLineDelta, NativeStemsBeamSchedulerRecognition,
        NativeStemsBeamSchedulerStatus, materialize_native_stems_beam_scheduler_frontiers,
    },
    native_stems_beam_stumps::{
        NativeStemsBeamStumpRecognition, materialize_native_stems_beam_stumps,
    },
    native_stems_beam_vlink_transaction::{
        NativeStemsBeamAppliedLineDelta, NativeStemsBeamAppliedLineDeltaSource,
        NativeStemsBeamCreateStemDisposition, NativeStemsBeamExhaustiveGlyphEqualsScan,
        NativeStemsBeamExhaustiveGlyphLookup, NativeStemsBeamExhaustiveSystemStemEqualsScan,
        NativeStemsBeamExhaustiveSystemStemLookup, NativeStemsBeamFixedGlyphContent,
        NativeStemsBeamGlyphAliasOrder, NativeStemsBeamGlyphIndexTransactionState,
        NativeStemsBeamGlyphRegistrationAction, NativeStemsBeamPersistentIdState,
        NativeStemsBeamSelectedGlyphBinding, NativeStemsBeamStemCheckerContext,
        NativeStemsBeamStemGrade, NativeStemsBeamSystemStemTransactionState,
        NativeStemsBeamVLinkLineState, NativeStemsBeamVLinkMutation,
        NativeStemsBeamVLinkTransaction, NativeStemsBeamVLinkTransactionError,
        NativeStemsBeamVLinkTransactionScope, NativeStemsBeamVLinkTransactionState,
        apply_native_stems_beam_vlink_create_stem_transaction,
    },
    native_stems_beam_vlinkers::{
        NativeStemsBeamVLinkerRecognition, materialize_native_stems_beam_vlinkers,
    },
    native_stems_head_builders::materialize_native_stems_head_builders,
    native_stems_head_corner_reachability::materialize_native_stems_head_corner_reachability,
    native_stems_head_corners::materialize_native_stems_head_corners,
    native_stems_head_seeds::materialize_native_stems_head_seeds,
    native_stems_head_stumps::materialize_native_stems_head_stumps,
    recognize::{
        GridLinesRecognition, NativeBeamRecognition, recognize_grid_lines,
        recognize_native_beams_with_stem_seeds,
    },
    stem_seeds_step::{NativeStemCheckResult, NativeStemCheckerParameters},
    stems_step::{NativeStemHeadSide, NativeStemLine, NativeStemVerticalSide},
};

const SCHEMA_HEADER: &str = "# schema: stems-beam-create-stem-v1";
const INSPECT_PROFILE: i32 = 1;
const STEM_MINIMUM_GRADE: f64 = 0.8 * 0.1;
const ARTIFICIAL_STEM_GRADE: f64 = 0.4;

const PAGE_FIELDS: &[&str] = &[
    "systems",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "executionMode",
    "registryHashMode",
];
const BASELINE_FIELDS: &[&str] = &[
    "system",
    "executionMode",
    "allocator",
    "glyphActive",
    "glyphOriginals",
    "interIndex",
    "sigVertices",
    "sigEdges",
    "systemStems",
    "noStaff",
    "glyphActiveHash",
    "glyphOriginalsHash",
    "interIndexHash",
    "sigHash",
    "systemStemsHash",
];
const FRONTIER_FIELDS: &[&str] = &[
    "system",
    "beamOrder",
    "beamSig",
    "hSide",
    "bAlias",
    "vSide",
    "builder",
    "plan",
    "stemProfile",
    "linkProfile",
    "lineBefore",
    "selectedGlyphRefs",
];
const EXPAND_FIELDS: &[&str] = &[
    "system",
    "plan",
    "lastIndex",
    "relations",
    "glyphs",
    "lineAfter",
    "lineChanged",
    "builderAliases",
    "attachmentAliases",
];
const LOOKUP_FIELDS: &[&str] = &[
    "system",
    "certificate",
    "candidate",
    "candidateBounds",
    "candidateWeight",
    "candidateRunTable",
    "aliasOrder",
    "baselineUnionSize",
    "scannedActive",
    "scannedOriginals",
    "activeEqualMatches",
    "originalEqualMatches",
    "lookup",
    "presentAlias",
    "presentId",
    "presentActive",
    "presentGlyph",
    "systemStemCertificate",
    "scannedSystemStems",
    "systemStemEqualMatches",
    "systemStemLookup",
    "systemStemInterId",
    "systemStemGrade",
    "activeHash",
    "originalsHash",
    "systemStemsHash",
];
const RESULT_FIELDS: &[&str] = &[
    "system",
    "plan",
    "candidate",
    "candidateComponents",
    "registration",
    "candidateObjectIdBefore",
    "canonicalGlyphIdBefore",
    "registeredAlias",
    "postAliasOrder",
    "postUnionSize",
    "registeredGlyphId",
    "disposition",
    "returnedStemInterId",
    "stemGrade",
    "stemMedian",
    "stemMeanThickness",
    "stemBounds",
    "stemAbnormal",
    "stemSigAttached",
    "stemMinGrade",
    "checkerMinThreshold",
    "artificialGrade",
    "impacts",
];
const DELTA_FIELDS: &[&str] = &[
    "system",
    "allocatorBefore",
    "allocatorAfter",
    "allocatorDelta",
    "glyphActiveBefore",
    "glyphActiveAfter",
    "glyphOriginalsBefore",
    "glyphOriginalsAfter",
    "systemStemsBefore",
    "systemStemsAfter",
    "registeredAlias",
    "registeredGlyph",
    "glyphActiveHashAfter",
    "glyphOriginalsHashAfter",
    "systemStemsHashAfter",
];
const GUARD_FIELDS: &[&str] = &[
    "system",
    "lineDeltaRetained",
    "interIndexUnchanged",
    "sigUnchanged",
    "relationsUnchanged",
    "linkerFlagsUnchanged",
    "stopBeforeVReuse",
    "stopBeforeBeamStemCheck",
];
const SUMMARY_FIELDS: &[&str] = &[
    "system",
    "transaction",
    "registration",
    "disposition",
    "allocatorDelta",
];
const PAGE_SUMMARY_FIELDS: &[&str] = &[
    "systems",
    "transactions",
    "newGlyphs",
    "reusedGlyphs",
    "reinsertedGlyphs",
    "checkedStems",
    "reusedStems",
    "artificialStems",
    "rejectedStems",
    "allocatorDelta",
    "sigMutations",
    "relationMutations",
    "linkerFlagMutations",
];
const CORPUS_SUMMARY_FIELDS: &[&str] = &[
    "schema",
    "mode",
    "pages",
    "pageRefs",
    "rowCounts",
    "probeSourceSha256",
    "runnerSourceSha256",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "emittedBodySha256",
    "emittedBodyLines",
    "emittedBodyBytes",
    "freshJvmPerPage",
    "freshJvmPerSystem",
    "javaProcessesPerPage",
    "freshSheetPerSystem",
    "runnerJavaProcessesReaped",
    "backgroundJavaProcessesStarted",
];

const MANIFEST_SCHEMA_HEADER: &str = "# schema: stems-beam-create-stem-manifest-v1";
const MANIFEST_PATH: &str = "rust/oracle/stems-beam-create-stem-manifest.txt";
const PROBE_PATH: &str = "rust/oracle/java/StemsBeamCreateStemProbe.java";
const RUNNER_PATH: &str = "rust/oracle/java/run-stems-beam-create-stem.sh";
const EXPECTED_MANIFEST_SHA256: &str =
    "b7e6fe6e7dc2f5eeba106133c930249f20e2c75d764704252289724bbe28c3e0";
const EXPECTED_PROBE_SHA256: &str =
    "36fecabe18d7713c823ce6990dae717e78997354a9ae0b142cba55f7d75004f3";
const EXPECTED_RUNNER_SHA256: &str =
    "6d95ff62d0acb502d531d6fb2aea0382fcb9dcb8fdd871fb7b0e2fba2ffb1de8";
const EXPECTED_CORPUS_BODY_SHA256: &str =
    "0c8c51e1c170a0dc3ec7e5910e6dca63a82f7d8fe6699b585c9556f183b359dc";
const EXPECTED_CORPUS_BODY_LINES: usize = 261;
const EXPECTED_CORPUS_BODY_BYTES: usize = 153_517;
const EXPECTED_CORPUS_ROW_COUNTS: [usize; 10] = [8, 30, 30, 30, 30, 30, 30, 30, 30, 8];
const EXPECTED_MANIFEST_BODY_SHA256: &str =
    "67d983b056548118015f5b7d18a9e2772860e08e0d2ab076118b25a9678c40af";
const EXPECTED_MANIFEST_BODY_LINES: usize = 9;
const EXPECTED_MANIFEST_BODY_BYTES: usize = 5_691;
const EXPECTED_CHULA_FIXTURE_SHA256: &str =
    "10ada930287be952dcb31666b7af0e77a30f2c513ca69698cc53ab00b206ef6c";
const MANIFEST_ENTRY_FIELDS: &[&str] = &[
    "ordinal",
    "page",
    "fixture",
    "rowCounts",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "emittedBodySha256",
    "emittedBodyLines",
    "emittedBodyBytes",
    "fixtureSha256",
    "fixtureLines",
    "fixtureBytes",
    "freshJvmPerPage",
    "freshJvmPerSystem",
    "javaProcessesPerPage",
    "freshSheetPerSystem",
    "runnerJavaProcessesReaped",
    "backgroundJavaProcessesStarted",
];
const MANIFEST_SUMMARY_FIELDS: &[&str] = &[
    "schema",
    "entries",
    "probeSourceSha256",
    "runnerSourceSha256",
    "corpusBodySha256",
    "corpusBodyLines",
    "corpusBodyBytes",
    "corpusRowCounts",
    "totalJavaProcesses",
    "freshJvmPerPage",
    "freshJvmPerSystem",
    "freshSheetPerSystem",
    "runnerJavaProcessesReaped",
    "backgroundJavaProcessesStarted",
    "manifestBodySha256",
    "manifestBodyLines",
    "manifestBodyBytes",
];

#[derive(Clone, Copy)]
struct PageSpec {
    image: &'static str,
    page: &'static str,
    fixture: &'static str,
}

const PAGES: [PageSpec; 8] = [
    PageSpec {
        image: "chula.png",
        page: "chula.png#1",
        fixture: "stems-beam-create-stem-chula.txt",
    },
    PageSpec {
        image: "allegretto.png",
        page: "allegretto.png#1",
        fixture: "stems-beam-create-stem-allegretto.txt",
    },
    PageSpec {
        image: "batuque.png",
        page: "batuque.png#1",
        fixture: "stems-beam-create-stem-batuque.txt",
    },
    PageSpec {
        image: "carmen.png",
        page: "carmen.png#1",
        fixture: "stems-beam-create-stem-carmen.txt",
    },
    PageSpec {
        image: "cucaracha.png",
        page: "cucaracha.png#1",
        fixture: "stems-beam-create-stem-cucaracha.txt",
    },
    PageSpec {
        image: "hove.png",
        page: "hove.png#1",
        fixture: "stems-beam-create-stem-hove.txt",
    },
    PageSpec {
        image: "zizi.png",
        page: "zizi.png#1",
        fixture: "stems-beam-create-stem-zizi.txt",
    },
    PageSpec {
        image: "BachInvention5.jpg",
        page: "BachInvention5.jpg#1",
        fixture: "stems-beam-create-stem-BachInvention5.txt",
    },
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Family {
    Page,
    Baseline,
    Frontier,
    Expand,
    Lookup,
    Result,
    Delta,
    Guard,
    Summary,
    PageSummary,
}

impl Family {
    fn parse(label: &str) -> Option<Self> {
        Some(match label {
            "stemsbeamcreatestempage" => Self::Page,
            "stemsbeamcreatestembaseline" => Self::Baseline,
            "stemsbeamcreatestemfrontier" => Self::Frontier,
            "stemsbeamcreatestemexpand" => Self::Expand,
            "stemsbeamcreatestemlookup" => Self::Lookup,
            "stemsbeamcreatestemresult" => Self::Result,
            "stemsbeamcreatestemdelta" => Self::Delta,
            "stemsbeamcreatestemguard" => Self::Guard,
            "stemsbeamcreatestemsummary" => Self::Summary,
            "stemsbeamcreatestempagesummary" => Self::PageSummary,
            _ => return None,
        })
    }

    const fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Page => PAGE_FIELDS,
            Self::Baseline => BASELINE_FIELDS,
            Self::Frontier => FRONTIER_FIELDS,
            Self::Expand => EXPAND_FIELDS,
            Self::Lookup => LOOKUP_FIELDS,
            Self::Result => RESULT_FIELDS,
            Self::Delta => DELTA_FIELDS,
            Self::Guard => GUARD_FIELDS,
            Self::Summary => SUMMARY_FIELDS,
            Self::PageSummary => PAGE_SUMMARY_FIELDS,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OracleRow {
    family: Family,
    page: String,
    values: Vec<String>,
    line_number: usize,
}

impl OracleRow {
    fn value(&self, field: &str) -> Result<&str, String> {
        self.family
            .fields()
            .iter()
            .position(|candidate| *candidate == field)
            .map(|index| self.values[index].as_str())
            .ok_or_else(|| {
                format!(
                    "line {} {:?} row has no {field} field",
                    self.line_number, self.family
                )
            })
    }

    fn usize(&self, field: &str) -> Result<usize, String> {
        let value = self.value(field)?;
        value.parse::<usize>().map_err(|_| {
            format!(
                "line {} {field} is not an unsigned integer: {value}",
                self.line_number
            )
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OracleSystem {
    system_id: usize,
    rows: BTreeMap<Family, OracleRow>,
}

impl OracleSystem {
    fn row(&self, family: Family) -> Result<&OracleRow, String> {
        self.rows
            .get(&family)
            .ok_or_else(|| format!("system {} lacks {family:?} row", self.system_id))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OracleFixture {
    page: String,
    page_row: OracleRow,
    systems: Vec<OracleSystem>,
    page_summary: OracleRow,
    corpus_summary: CorpusSummary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CorpusSummary {
    mode: String,
    page_ref: String,
    row_counts: [usize; 10],
    probe_sha256: String,
    runner_sha256: String,
    scheduler_sha256: String,
    expand_sha256: String,
    body_sha256: String,
    body_lines: usize,
    body_bytes: usize,
    java_processes_per_page: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ManifestEntry {
    ordinal: usize,
    page: String,
    fixture: String,
    row_counts: [usize; 10],
    scheduler_sha256: String,
    expand_sha256: String,
    body_sha256: String,
    body_lines: usize,
    body_bytes: usize,
    fixture_sha256: String,
    fixture_lines: usize,
    fixture_bytes: usize,
    java_processes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CreateStemManifest {
    entries: Vec<ManifestEntry>,
    probe_sha256: String,
    runner_sha256: String,
    corpus_body_sha256: String,
    corpus_body_lines: usize,
    corpus_body_bytes: usize,
    corpus_row_counts: [usize; 10],
    total_java_processes: usize,
    manifest_body_sha256: String,
    manifest_body_lines: usize,
    manifest_body_bytes: usize,
}

impl CreateStemManifest {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
        if text.lines().next() != Some(MANIFEST_SCHEMA_HEADER) {
            return Err("createStem manifest schema header differs".to_owned());
        }
        let mut entries = Vec::new();
        let mut summary = None;
        for (offset, line) in text.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            match line.split_ascii_whitespace().next() {
                Some("stemsbeamcreatestemmanifestentry") => {
                    if summary.is_some() {
                        return Err(format!(
                            "manifest entry after summary at line {}",
                            offset + 1
                        ));
                    }
                    let values = parse_exact_labeled_row(
                        line,
                        "stemsbeamcreatestemmanifestentry",
                        MANIFEST_ENTRY_FIELDS,
                    )?;
                    if values[12] != "false"
                        || values[13] != "true"
                        || values[15] != "true"
                        || values[16] != "true"
                        || values[17] != "0"
                    {
                        return Err(format!(
                            "manifest JVM isolation guard differs at line {}",
                            offset + 1
                        ));
                    }
                    entries.push(ManifestEntry {
                        ordinal: parse_usize(values[0], "manifest ordinal")?,
                        page: values[1].to_owned(),
                        fixture: values[2].to_owned(),
                        row_counts: parse_row_counts(values[3])?,
                        scheduler_sha256: parse_lower_hex(
                            values[4],
                            64,
                            "scheduler fixture SHA-256",
                        )?
                        .to_owned(),
                        expand_sha256: parse_lower_hex(values[5], 64, "expand fixture SHA-256")?
                            .to_owned(),
                        body_sha256: parse_lower_hex(values[6], 64, "body SHA-256")?.to_owned(),
                        body_lines: parse_usize(values[7], "body lines")?,
                        body_bytes: parse_usize(values[8], "body bytes")?,
                        fixture_sha256: parse_lower_hex(values[9], 64, "fixture SHA-256")?
                            .to_owned(),
                        fixture_lines: parse_usize(values[10], "fixture lines")?,
                        fixture_bytes: parse_usize(values[11], "fixture bytes")?,
                        java_processes: parse_usize(values[14], "Java processes")?,
                    });
                }
                Some("stemsbeamcreatestemmanifestsummary") => {
                    if summary.is_some() {
                        return Err("duplicate createStem manifest summary".to_owned());
                    }
                    let values = parse_exact_labeled_row(
                        line,
                        "stemsbeamcreatestemmanifestsummary",
                        MANIFEST_SUMMARY_FIELDS,
                    )?;
                    if values[0] != "stems-beam-create-stem-manifest-v1"
                        || values[9] != "false"
                        || values[10] != "true"
                        || values[11] != "true"
                        || values[12] != "true"
                        || values[13] != "0"
                    {
                        return Err("createStem manifest summary guard differs".to_owned());
                    }
                    summary = Some((
                        parse_usize(values[1], "manifest entries")?,
                        parse_lower_hex(values[2], 64, "probe SHA-256")?.to_owned(),
                        parse_lower_hex(values[3], 64, "runner SHA-256")?.to_owned(),
                        parse_lower_hex(values[4], 64, "corpus body SHA-256")?.to_owned(),
                        parse_usize(values[5], "corpus body lines")?,
                        parse_usize(values[6], "corpus body bytes")?,
                        parse_row_counts(values[7])?,
                        parse_usize(values[8], "total Java processes")?,
                        parse_lower_hex(values[14], 64, "manifest body SHA-256")?.to_owned(),
                        parse_usize(values[15], "manifest body lines")?,
                        parse_usize(values[16], "manifest body bytes")?,
                    ));
                }
                Some(family) => return Err(format!("unknown createStem manifest family {family}")),
                None => unreachable!("nonempty manifest line"),
            }
        }
        let (
            entry_count,
            probe_sha256,
            runner_sha256,
            corpus_body_sha256,
            corpus_body_lines,
            corpus_body_bytes,
            corpus_row_counts,
            total_java_processes,
            manifest_body_sha256,
            manifest_body_lines,
            manifest_body_bytes,
        ) = summary.ok_or_else(|| "missing createStem manifest summary".to_owned())?;
        if entries.len() != entry_count || entries.len() != PAGES.len() {
            return Err("createStem manifest entry count differs".to_owned());
        }
        for (ordinal, (entry, spec)) in entries.iter().zip(PAGES).enumerate() {
            if entry.ordinal != ordinal || entry.page != spec.page || entry.fixture != spec.fixture
            {
                return Err(format!("createStem manifest order differs at {ordinal}"));
            }
        }
        Ok(Self {
            entries,
            probe_sha256,
            runner_sha256,
            corpus_body_sha256,
            corpus_body_lines,
            corpus_body_bytes,
            corpus_row_counts,
            total_java_processes,
            manifest_body_sha256,
            manifest_body_lines,
            manifest_body_bytes,
        })
    }
}

fn parse_exact_labeled_row<'a>(
    row: &'a str,
    family: &str,
    labels: &[&str],
) -> Result<Vec<&'a str>, String> {
    let tokens = row.split_ascii_whitespace().collect::<Vec<_>>();
    if tokens.first().copied() != Some(family) || tokens.len() != 1 + (2 * labels.len()) {
        return Err(format!("malformed {family} row"));
    }
    let observed = tokens[1..]
        .chunks_exact(2)
        .map(|pair| pair[0])
        .collect::<Vec<_>>();
    if observed != labels {
        return Err(format!("{family} labels differ: {observed:?}"));
    }
    Ok(tokens[1..].chunks_exact(2).map(|pair| pair[1]).collect())
}

fn parse_usize(value: &str, label: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| format!("invalid {label}: {value}"))
}

struct NativePage {
    grid: GridLinesRecognition,
    stem_seeds: NativeStemSeedRecognition,
    beams: NativeBeamRecognition,
    beam_stumps: NativeStemsBeamStumpRecognition,
    beam_vlinkers: NativeStemsBeamVLinkerRecognition,
    beam_builders: NativeStemsBeamBuilderRecognition,
    plans: NativeStemsBeamLinkPlanRecognition,
    scheduler: NativeStemsBeamSchedulerRecognition,
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
    let scheduler = materialize_native_stems_beam_scheduler_frontiers(
        &beams,
        &beam_stumps,
        &beam_vlinkers,
        &beam_builders,
        &plans,
    )
    .unwrap_or_else(|error| panic!("{image}: STEMS beam scheduler failed: {error}"));
    NativePage {
        grid,
        stem_seeds,
        beams,
        beam_stumps,
        beam_vlinkers,
        beam_builders,
        plans,
        scheduler,
    }
}

impl OracleFixture {
    fn parse(text: &str) -> Result<Self, String> {
        if text.lines().filter(|line| *line == SCHEMA_HEADER).count() != 1 {
            return Err("beam createStem schema header must occur exactly once".to_owned());
        }
        let mut rows = Vec::new();
        let mut corpus_summary = None;
        for (offset, line) in text.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with("stemsbeamcreatestemcorpussummary ") {
                if corpus_summary.is_some() {
                    return Err("duplicate beam createStem corpus summary".to_owned());
                }
                corpus_summary = Some(parse_corpus_summary(line, offset + 1)?);
            } else {
                rows.push(parse_row(line, offset + 1)?);
            }
        }
        let corpus_summary =
            corpus_summary.ok_or_else(|| "missing beam createStem corpus summary".to_owned())?;
        let page_row = rows
            .first()
            .filter(|row| row.family == Family::Page)
            .cloned()
            .ok_or_else(|| "first semantic row is not createStem page".to_owned())?;
        let page = page_row.page.clone();
        let system_count = page_row.usize("systems")?;
        if page_row.value("executionMode")? != "foregroundJvmPerSystem"
            || page_row.value("registryHashMode")? != "StructuralGlyphMultisetMembershipOnly"
        {
            return Err("createStem page execution mode differs".to_owned());
        }
        parse_lower_hex(
            page_row.value("schedulerFixtureSha256")?,
            64,
            "scheduler fixture SHA-256",
        )?;
        parse_lower_hex(
            page_row.value("expandFixtureSha256")?,
            64,
            "expand fixture SHA-256",
        )?;

        let expected_family_order = [
            Family::Baseline,
            Family::Frontier,
            Family::Expand,
            Family::Lookup,
            Family::Result,
            Family::Delta,
            Family::Guard,
            Family::Summary,
        ];
        let expected_semantic_rows = 2_usize
            .checked_add(
                system_count
                    .checked_mul(expected_family_order.len())
                    .ok_or_else(|| "createStem fixture system row count overflow".to_owned())?,
            )
            .ok_or_else(|| "createStem fixture row count overflow".to_owned())?;
        if rows.len() != expected_semantic_rows {
            return Err(format!(
                "createStem fixture has {} semantic rows, expected {expected_semantic_rows}",
                rows.len()
            ));
        }
        let mut systems = Vec::with_capacity(system_count);
        let mut cursor = 1;
        for system_id in 1..=system_count {
            let mut system_rows = BTreeMap::new();
            for family in expected_family_order {
                let row = rows
                    .get(cursor)
                    .ok_or_else(|| format!("missing system {system_id} {family:?} row"))?;
                if row.family != family || row.page != page || row.usize("system")? != system_id {
                    return Err(format!(
                        "line {} does not match system {system_id} {family:?} hierarchy",
                        row.line_number
                    ));
                }
                validate_system_row(row, system_id)?;
                if system_rows.insert(family, row.clone()).is_some() {
                    return Err(format!("duplicate system {system_id} {family:?} row"));
                }
                cursor += 1;
            }
            let system = OracleSystem {
                system_id,
                rows: system_rows,
            };
            validate_system_algebra(&system)?;
            systems.push(system);
        }
        let page_summary = rows
            .get(cursor)
            .filter(|row| row.family == Family::PageSummary && row.page == page)
            .cloned()
            .ok_or_else(|| "missing final createStem page summary".to_owned())?;
        validate_page_summary(&page_summary, system_count)?;
        validate_fixture_algebra(&page, &page_row, &systems, &page_summary, &corpus_summary)?;
        Ok(Self {
            page,
            page_row,
            systems,
            page_summary,
            corpus_summary,
        })
    }
}

fn parse_corpus_summary(line: &str, line_number: usize) -> Result<CorpusSummary, String> {
    let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
    if tokens.first() != Some(&"stemsbeamcreatestemcorpussummary")
        || tokens.len() != 1 + (2 * CORPUS_SUMMARY_FIELDS.len())
    {
        return Err(format!(
            "invalid createStem corpus summary at line {line_number}"
        ));
    }
    let mut values = Vec::with_capacity(CORPUS_SUMMARY_FIELDS.len());
    for (ordinal, expected) in CORPUS_SUMMARY_FIELDS.iter().enumerate() {
        let label_index = 1 + (ordinal * 2);
        if tokens[label_index] != *expected {
            return Err(format!(
                "line {line_number} corpus field {ordinal} is {}, expected {expected}",
                tokens[label_index]
            ));
        }
        values.push(tokens[label_index + 1]);
    }
    if values[0] != "stems-beam-create-stem-v1"
        || values[2] != "1"
        || values[12] != "false"
        || values[13] != "true"
        || values[15] != "true"
        || values[16] != "true"
        || values[17] != "0"
    {
        return Err(format!("line {line_number} corpus execution guard differs"));
    }
    for (value, label) in values[5..10].iter().zip([
        "probe SHA-256",
        "runner SHA-256",
        "scheduler fixture SHA-256",
        "expand fixture SHA-256",
        "body SHA-256",
    ]) {
        parse_lower_hex(value, 64, label)?;
    }
    Ok(CorpusSummary {
        mode: values[1].to_owned(),
        page_ref: values[3].to_owned(),
        row_counts: parse_row_counts(values[4])?,
        probe_sha256: values[5].to_owned(),
        runner_sha256: values[6].to_owned(),
        scheduler_sha256: values[7].to_owned(),
        expand_sha256: values[8].to_owned(),
        body_sha256: values[9].to_owned(),
        body_lines: values[10]
            .parse()
            .map_err(|_| format!("line {line_number} invalid body lines"))?,
        body_bytes: values[11]
            .parse()
            .map_err(|_| format!("line {line_number} invalid body bytes"))?,
        java_processes_per_page: values[14]
            .parse()
            .map_err(|_| format!("line {line_number} invalid Java process count"))?,
    })
}

fn parse_row(line: &str, line_number: usize) -> Result<OracleRow, String> {
    let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
    let family = tokens
        .first()
        .and_then(|label| Family::parse(label))
        .ok_or_else(|| format!("unknown createStem row family at line {line_number}: {line}"))?;
    let page = tokens
        .get(1)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("missing page at line {line_number}"))?;
    let fields = family.fields();
    if tokens.len() != 2 + (2 * fields.len()) {
        return Err(format!(
            "line {line_number} {family:?} row has {} tokens, expected {}",
            tokens.len(),
            2 + (2 * fields.len())
        ));
    }
    let mut values = Vec::with_capacity(fields.len());
    for (ordinal, expected) in fields.iter().enumerate() {
        let label_index = 2 + (ordinal * 2);
        let actual = tokens[label_index];
        if actual != *expected {
            return Err(format!(
                "line {line_number} field {ordinal} is {actual}, expected {expected}"
            ));
        }
        values.push(tokens[label_index + 1].to_owned());
    }
    Ok(OracleRow {
        family,
        page: (*page).to_owned(),
        values,
        line_number,
    })
}

fn validate_system_row(row: &OracleRow, system_id: usize) -> Result<(), String> {
    match row.family {
        Family::Baseline => {
            let expected_mode = if system_id == 1 {
                "sheet-first"
            } else {
                "isolated-system-frontier"
            };
            if row.value("executionMode")? != expected_mode {
                return Err(format!(
                    "system {system_id} execution mode is not {expected_mode}"
                ));
            }
            for field in [
                "allocator",
                "glyphActive",
                "glyphOriginals",
                "interIndex",
                "sigVertices",
                "sigEdges",
                "systemStems",
            ] {
                row.usize(field)?;
            }
            parse_no_staff(row.value("noStaff")?)?;
            for field in [
                "glyphActiveHash",
                "glyphOriginalsHash",
                "interIndexHash",
                "sigHash",
                "systemStemsHash",
            ] {
                parse_lower_hex(row.value(field)?, 64, field)?;
            }
        }
        Family::Frontier => {
            for field in [
                "beamOrder",
                "beamSig",
                "builder",
                "plan",
                "stemProfile",
                "linkProfile",
            ] {
                row.usize(field)?;
            }
            if !matches!(row.value("hSide")?, "LEFT" | "RIGHT")
                || !matches!(row.value("vSide")?, "TOP" | "BOTTOM")
            {
                return Err(format!("line {} invalid frontier side", row.line_number));
            }
            parse_line_token(row.value("lineBefore")?)?;
            parse_list(row.value("selectedGlyphRefs")?)?;
        }
        Family::Expand => {
            for field in ["plan", "lastIndex", "relations", "glyphs"] {
                row.usize(field)?;
            }
            parse_line_token(row.value("lineAfter")?)?;
            for field in ["lineChanged", "builderAliases", "attachmentAliases"] {
                parse_true_false(row.value(field)?, field)?;
            }
            if row.value("builderAliases")? != "true" {
                return Err("builder/theoretical-line alias is not retained".to_owned());
            }
        }
        Family::Lookup => {
            if row.value("certificate")? != "ExhaustiveGlyphEqualsScan" {
                return Err(format!(
                    "line {} lookup certificate differs",
                    row.line_number
                ));
            }
            parse_glyph_token(row.value("candidate")?)?;
            parse_bounds_token(row.value("candidateBounds")?)?;
            row.usize("candidateWeight")?;
            parse_run_table_token(row.value("candidateRunTable")?)?;
            if row.value("aliasOrder")? != "JavaGlyphId" {
                return Err(format!("line {} alias order differs", row.line_number));
            }
            for field in [
                "baselineUnionSize",
                "scannedActive",
                "scannedOriginals",
                "activeEqualMatches",
                "originalEqualMatches",
            ] {
                row.usize(field)?;
            }
            let union_size = row.usize("baselineUnionSize")?;
            let scanned_active = row.usize("scannedActive")?;
            let scanned_originals = row.usize("scannedOriginals")?;
            let active_matches = row.usize("activeEqualMatches")?;
            let original_matches = row.usize("originalEqualMatches")?;
            if union_size < scanned_active.max(scanned_originals)
                || union_size > scanned_active.saturating_add(scanned_originals)
                || active_matches > 1
                || original_matches > 1
                || active_matches > scanned_active
                || original_matches > scanned_originals
            {
                return Err(format!(
                    "line {} equality scan/set coverage differs",
                    row.line_number
                ));
            }
            match row.value("lookup")? {
                "Absent" => {
                    if row.usize("originalEqualMatches")? != 0
                        || ["presentAlias", "presentId", "presentActive", "presentGlyph"]
                            .into_iter()
                            .any(|field| row.value(field).ok() != Some("-"))
                    {
                        return Err(format!(
                            "line {} absent lookup carries a present glyph",
                            row.line_number
                        ));
                    }
                }
                "Present" => {
                    if row.usize("originalEqualMatches")? != 1 {
                        return Err(format!(
                            "line {} present lookup lacks one original match",
                            row.line_number
                        ));
                    }
                    if parse_glyph_alias(row.value("presentAlias")?)? != row.usize("presentId")? {
                        return Err(format!(
                            "line {} present alias differs from Java glyph ID",
                            row.line_number
                        ));
                    }
                    if row
                        .value("presentId")?
                        .parse::<usize>()
                        .ok()
                        .filter(|value| *value > 0)
                        .is_none()
                    {
                        return Err(format!(
                            "line {} present glyph ID is invalid",
                            row.line_number
                        ));
                    }
                    parse_true_false(row.value("presentActive")?, "presentActive")?;
                    parse_glyph_token(row.value("presentGlyph")?)?;
                    if row.value("candidate")? != row.value("presentGlyph")? {
                        return Err(format!(
                            "line {} equality certificate keys differ",
                            row.line_number
                        ));
                    }
                }
                value => {
                    return Err(format!(
                        "line {} invalid candidate lookup: {value}",
                        row.line_number
                    ));
                }
            }
            if row.value("systemStemCertificate")? != "ExhaustiveSystemStemEqualsScan" {
                return Err(format!(
                    "line {} systemStem certificate differs",
                    row.line_number
                ));
            }
            row.usize("scannedSystemStems")?;
            if row.usize("systemStemEqualMatches")? > 1
                || row.usize("systemStemEqualMatches")? > row.usize("scannedSystemStems")?
            {
                return Err(format!(
                    "line {} systemStem equality scan is ambiguous",
                    row.line_number
                ));
            }
            match row.value("systemStemLookup")? {
                "Absent" => {
                    if row.usize("systemStemEqualMatches")? != 0
                        || row.value("systemStemInterId")? != "-"
                        || row.value("systemStemGrade")? != "-"
                    {
                        return Err(format!(
                            "line {} absent system stem carries state",
                            row.line_number
                        ));
                    }
                }
                "Present" => {
                    return Err(format!(
                        "line {} v1 compact envelope cannot hydrate a present system stem",
                        row.line_number
                    ));
                }
                value => {
                    return Err(format!(
                        "line {} invalid system-stem lookup: {value}",
                        row.line_number
                    ));
                }
            }
            for field in ["activeHash", "originalsHash", "systemStemsHash"] {
                parse_lower_hex(row.value(field)?, 64, field)?;
            }
        }
        Family::Result => {
            row.usize("plan")?;
            parse_glyph_token(row.value("candidate")?)?;
            parse_list(row.value("candidateComponents")?)?;
            if !matches!(
                row.value("registration")?,
                "New" | "ReuseActive" | "ReinsertOriginal"
            ) {
                return Err(format!("line {} invalid registration", row.line_number));
            }
            for field in ["candidateObjectIdBefore", "registeredGlyphId"] {
                row.usize(field)?;
            }
            parse_optional_usize(
                row.value("canonicalGlyphIdBefore")?,
                "canonicalGlyphIdBefore",
            )?;
            parse_glyph_alias(row.value("registeredAlias")?)?;
            if row.value("postAliasOrder")? != "JavaGlyphId" {
                return Err(format!(
                    "line {} post-registration alias order differs",
                    row.line_number
                ));
            }
            row.usize("postUnionSize")?;
            if !matches!(
                row.value("disposition")?,
                "Reused" | "CreatedChecked" | "CreatedArtificial" | "Rejected"
            ) {
                return Err(format!("line {} invalid disposition", row.line_number));
            }
            parse_optional_usize(row.value("returnedStemInterId")?, "returnedStemInterId")?;
            let stem_grade = parse_optional_hex_double(row.value("stemGrade")?, "stemGrade")?;
            if stem_grade.is_some() {
                parse_line_token(row.value("stemMedian")?)?;
                parse_hex_double(row.value("stemMeanThickness")?, "stem mean thickness")?;
                parse_rectangle_token(row.value("stemBounds")?)?;
                parse_true_false(row.value("stemAbnormal")?, "stemAbnormal")?;
                parse_true_false(row.value("stemSigAttached")?, "stemSigAttached")?;
            } else if [
                "stemMedian",
                "stemMeanThickness",
                "stemBounds",
                "stemAbnormal",
                "stemSigAttached",
            ]
            .into_iter()
            .any(|field| row.value(field).ok() != Some("-"))
            {
                return Err(format!(
                    "line {} null stem carries geometry/state",
                    row.line_number
                ));
            }
            for field in ["stemMinGrade", "checkerMinThreshold", "artificialGrade"] {
                parse_hex_double(row.value(field)?, field)?;
            }
            parse_impacts(row.value("impacts")?)?;
        }
        Family::Delta => {
            for field in [
                "allocatorBefore",
                "allocatorAfter",
                "allocatorDelta",
                "glyphActiveBefore",
                "glyphActiveAfter",
                "glyphOriginalsBefore",
                "glyphOriginalsAfter",
                "systemStemsBefore",
                "systemStemsAfter",
            ] {
                row.usize(field)?;
            }
            parse_glyph_alias(row.value("registeredAlias")?)?;
            parse_glyph_token(row.value("registeredGlyph")?)?;
            for field in [
                "glyphActiveHashAfter",
                "glyphOriginalsHashAfter",
                "systemStemsHashAfter",
            ] {
                parse_lower_hex(row.value(field)?, 64, field)?;
            }
        }
        Family::Guard => {
            for field in GUARD_FIELDS.iter().skip(1) {
                if row.value(field)? != "true" {
                    return Err(format!("line {} guard {field} is false", row.line_number));
                }
            }
        }
        Family::Summary => {
            if row.value("transaction")? != "CreateStemOnly" {
                return Err(format!("line {} transaction seam differs", row.line_number));
            }
            if !matches!(
                row.value("registration")?,
                "New" | "ReuseActive" | "ReinsertOriginal"
            ) || !matches!(
                row.value("disposition")?,
                "Reused" | "CreatedChecked" | "CreatedArtificial" | "Rejected"
            ) {
                return Err(format!("line {} invalid summary enum", row.line_number));
            }
            row.usize("allocatorDelta")?;
        }
        Family::Page | Family::PageSummary => {
            return Err(format!("unexpected per-system {:?} row", row.family));
        }
    }
    Ok(())
}

fn parse_glyph_alias(value: &str) -> Result<usize, String> {
    value
        .strip_prefix("glyph:")
        .and_then(|ordinal| ordinal.parse::<usize>().ok())
        .ok_or_else(|| format!("invalid Java glyph alias: {value}"))
}

fn validate_page_summary(row: &OracleRow, systems: usize) -> Result<(), String> {
    if row.usize("systems")? != systems || row.usize("transactions")? != systems {
        return Err("page summary system/transaction count differs".to_owned());
    }
    for field in PAGE_SUMMARY_FIELDS.iter().skip(2) {
        row.usize(field)?;
    }
    for field in ["sigMutations", "relationMutations", "linkerFlagMutations"] {
        if row.usize(field)? != 0 {
            return Err(format!("page summary {field} crosses transaction seam"));
        }
    }
    Ok(())
}

fn validate_system_algebra(system: &OracleSystem) -> Result<(), String> {
    let baseline = system.row(Family::Baseline)?;
    let frontier = system.row(Family::Frontier)?;
    let expand = system.row(Family::Expand)?;
    let lookup = system.row(Family::Lookup)?;
    let result = system.row(Family::Result)?;
    let delta = system.row(Family::Delta)?;
    let summary = system.row(Family::Summary)?;

    let plan = frontier.usize("plan")?;
    if expand.usize("plan")? != plan || result.usize("plan")? != plan {
        return Err(format!("system {} plan join differs", system.system_id));
    }
    let selected = parse_list(frontier.value("selectedGlyphRefs")?)?;
    if selected.is_empty()
        || selected != parse_list(result.value("candidateComponents")?)?
        || selected.len() != expand.usize("glyphs")?
        || expand.usize("relations")? == 0
    {
        return Err(format!(
            "system {} selected createStem evidence differs",
            system.system_id
        ));
    }
    let selected_evidence = selected
        .iter()
        .map(|value| parse_selected_glyph(value))
        .collect::<Result<Vec<_>, _>>()?;
    let expected_candidate_object_id = if selected_evidence.len() == 1 {
        usize::try_from(selected_evidence[0].glyph_id)
            .map_err(|_| format!("system {} selected glyph ID is negative", system.system_id))?
    } else {
        0
    };
    if result.usize("candidateObjectIdBefore")? != expected_candidate_object_id {
        return Err(format!(
            "system {} transient candidate object ID differs",
            system.system_id
        ));
    }
    let line_changed = frontier.value("lineBefore")? != expand.value("lineAfter")?;
    if parse_true_false(expand.value("lineChanged")?, "lineChanged")? != line_changed.to_string() {
        return Err(format!(
            "system {} stored line delta flag differs",
            system.system_id
        ));
    }
    let candidate = result.value("candidate")?;
    if lookup.value("candidate")? != candidate || delta.value("registeredGlyph")? != candidate {
        return Err(format!(
            "system {} candidate structural key join differs",
            system.system_id
        ));
    }
    for (baseline_field, lookup_field) in [
        ("glyphActive", "scannedActive"),
        ("glyphOriginals", "scannedOriginals"),
        ("systemStems", "scannedSystemStems"),
    ] {
        if baseline.usize(baseline_field)? != lookup.usize(lookup_field)? {
            return Err(format!(
                "system {} exhaustive scan count differs",
                system.system_id
            ));
        }
    }
    for (baseline_field, lookup_field) in [
        ("glyphActiveHash", "activeHash"),
        ("glyphOriginalsHash", "originalsHash"),
        ("systemStemsHash", "systemStemsHash"),
    ] {
        if baseline.value(baseline_field)? != lookup.value(lookup_field)? {
            return Err(format!(
                "system {} exhaustive scan provenance differs",
                system.system_id
            ));
        }
    }

    let allocator_before = baseline.usize("allocator")?;
    let active_before = baseline.usize("glyphActive")?;
    let originals_before = baseline.usize("glyphOriginals")?;
    let stems_before = baseline.usize("systemStems")?;
    for (field, expected) in [
        ("allocatorBefore", allocator_before),
        ("glyphActiveBefore", active_before),
        ("glyphOriginalsBefore", originals_before),
        ("systemStemsBefore", stems_before),
    ] {
        if delta.usize(field)? != expected {
            return Err(format!(
                "system {} delta {field} does not join baseline",
                system.system_id
            ));
        }
    }

    let registration = result.value("registration")?;
    if summary.value("registration")? != registration {
        return Err(format!(
            "system {} registration summary differs",
            system.system_id
        ));
    }
    let expected_canonical_before = match lookup.value("lookup")? {
        "Absent" => None,
        "Present" => Some(lookup.usize("presentId")?),
        _ => unreachable!("validated lookup enum"),
    };
    if parse_optional_usize(
        result.value("canonicalGlyphIdBefore")?,
        "canonicalGlyphIdBefore",
    )? != expected_canonical_before
    {
        return Err(format!(
            "system {} canonical predecessor ID differs",
            system.system_id
        ));
    }
    let (allocator_delta, active_delta, originals_delta, registered_id) = match registration {
        "New" => {
            if lookup.value("lookup")? != "Absent" {
                return Err(format!(
                    "system {} new registration has a canonical predecessor",
                    system.system_id
                ));
            }
            let id = allocator_before
                .checked_add(1)
                .ok_or_else(|| format!("system {} shared allocator exhausted", system.system_id))?;
            (1, 1, 1, id)
        }
        "ReuseActive" => {
            if lookup.value("lookup")? != "Present" || lookup.value("presentActive")? != "true" {
                return Err(format!(
                    "system {} active reuse certificate differs",
                    system.system_id
                ));
            }
            (0, 0, 0, lookup.usize("presentId")?)
        }
        "ReinsertOriginal" => {
            if lookup.value("lookup")? != "Present" || lookup.value("presentActive")? != "false" {
                return Err(format!(
                    "system {} original reinsert certificate differs",
                    system.system_id
                ));
            }
            (0, 1, 0, lookup.usize("presentId")?)
        }
        _ => unreachable!("validated registration enum"),
    };
    if result.usize("registeredGlyphId")? != registered_id
        || parse_glyph_alias(result.value("registeredAlias")?)? != registered_id
        || parse_glyph_alias(delta.value("registeredAlias")?)? != registered_id
        || delta.usize("allocatorDelta")? != allocator_delta
        || summary.usize("allocatorDelta")? != allocator_delta
        || delta.usize("allocatorAfter")? != allocator_before + allocator_delta
        || delta.usize("glyphActiveAfter")? != active_before + active_delta
        || delta.usize("glyphOriginalsAfter")? != originals_before + originals_delta
    {
        return Err(format!(
            "system {} registry/allocator transition differs",
            system.system_id
        ));
    }
    let active_hash_changed =
        delta.value("glyphActiveHashAfter")? != baseline.value("glyphActiveHash")?;
    let originals_hash_changed =
        delta.value("glyphOriginalsHashAfter")? != baseline.value("glyphOriginalsHash")?;
    let registry_hash_transition_matches = match registration {
        "New" => active_hash_changed && originals_hash_changed,
        "ReuseActive" => !active_hash_changed && !originals_hash_changed,
        "ReinsertOriginal" => active_hash_changed && !originals_hash_changed,
        _ => unreachable!("validated registration enum"),
    };
    if !registry_hash_transition_matches {
        return Err(format!(
            "system {} identity-preserving registry hash differs",
            system.system_id
        ));
    }

    let disposition = result.value("disposition")?;
    if summary.value("disposition")? != disposition {
        return Err(format!(
            "system {} disposition summary differs",
            system.system_id
        ));
    }
    let system_stem_present = lookup.value("systemStemLookup")? == "Present";
    let expected_stem_delta = match disposition {
        "Reused" => {
            if !system_stem_present
                || result.value("returnedStemInterId")? != lookup.value("systemStemInterId")?
                || result.value("stemGrade")? != lookup.value("systemStemGrade")?
            {
                return Err(format!(
                    "system {} reused stem identity differs",
                    system.system_id
                ));
            }
            0
        }
        "CreatedChecked" | "CreatedArtificial" => {
            if system_stem_present
                || result.value("returnedStemInterId")? != "0"
                || result.value("stemGrade")? == "-"
                || result.value("stemMedian")? == "-"
                || result.value("stemMeanThickness")? == "-"
                || result.value("stemBounds")? == "-"
                || result.value("stemAbnormal")? != "false"
                || result.value("stemSigAttached")? != "false"
            {
                return Err(format!(
                    "system {} created stem geometry/state differs",
                    system.system_id
                ));
            }
            if disposition == "CreatedArtificial" && frontier.usize("stemProfile")? != 4 {
                return Err(format!(
                    "system {} artificial stem is not profile 4",
                    system.system_id
                ));
            }
            1
        }
        "Rejected" => {
            if system_stem_present
                || result.value("returnedStemInterId")? != "-"
                || result.value("stemGrade")? != "-"
                || result.value("stemMedian")? != "-"
                || result.value("stemMeanThickness")? != "-"
                || result.value("stemBounds")? != "-"
                || result.value("stemAbnormal")? != "-"
                || result.value("stemSigAttached")? != "-"
            {
                return Err(format!(
                    "system {} rejected stem retained an interpretation",
                    system.system_id
                ));
            }
            0
        }
        _ => unreachable!("validated disposition enum"),
    };
    if delta.usize("systemStemsAfter")? != stems_before + expected_stem_delta {
        return Err(format!(
            "system {} systemStems transition differs",
            system.system_id
        ));
    }
    let system_stems_hash_changed =
        delta.value("systemStemsHashAfter")? != baseline.value("systemStemsHash")?;
    if system_stems_hash_changed != (expected_stem_delta == 1) {
        return Err(format!(
            "system {} identity-preserving systemStems hash differs",
            system.system_id
        ));
    }
    Ok(())
}

fn validate_fixture_algebra(
    page: &str,
    page_row: &OracleRow,
    systems: &[OracleSystem],
    page_summary: &OracleRow,
    corpus: &CorpusSummary,
) -> Result<(), String> {
    let mut registrations = BTreeMap::<&str, usize>::new();
    let mut dispositions = BTreeMap::<&str, usize>::new();
    let mut allocator_delta = 0;
    for system in systems {
        let summary = system.row(Family::Summary)?;
        *registrations
            .entry(summary.value("registration")?)
            .or_default() += 1;
        *dispositions
            .entry(summary.value("disposition")?)
            .or_default() += 1;
        allocator_delta += summary.usize("allocatorDelta")?;
    }
    let expected = [
        ("newGlyphs", *registrations.get("New").unwrap_or(&0)),
        (
            "reusedGlyphs",
            *registrations.get("ReuseActive").unwrap_or(&0),
        ),
        (
            "reinsertedGlyphs",
            *registrations.get("ReinsertOriginal").unwrap_or(&0),
        ),
        (
            "checkedStems",
            *dispositions.get("CreatedChecked").unwrap_or(&0),
        ),
        ("reusedStems", *dispositions.get("Reused").unwrap_or(&0)),
        (
            "artificialStems",
            *dispositions.get("CreatedArtificial").unwrap_or(&0),
        ),
        ("rejectedStems", *dispositions.get("Rejected").unwrap_or(&0)),
        ("allocatorDelta", allocator_delta),
    ];
    for (field, value) in expected {
        if page_summary.usize(field)? != value {
            return Err(format!("{page} page summary {field} differs"));
        }
    }
    if page_summary.usize("reusedStems")? != 0 {
        return Err(format!(
            "{page} v1 compact envelope cannot hydrate reused system stems"
        ));
    }
    let expected_counts = [
        1,
        systems.len(),
        systems.len(),
        systems.len(),
        systems.len(),
        systems.len(),
        systems.len(),
        systems.len(),
        systems.len(),
        1,
    ];
    if corpus.page_ref != page
        || corpus.row_counts != expected_counts
        || corpus.java_processes_per_page != systems.len()
        || corpus.scheduler_sha256 != page_row.value("schedulerFixtureSha256")?
        || corpus.expand_sha256 != page_row.value("expandFixtureSha256")?
    {
        return Err(format!("{page} corpus provenance algebra differs"));
    }
    Ok(())
}

fn parse_row_counts(value: &str) -> Result<[usize; 10], String> {
    let values = value
        .split(':')
        .map(|value| value.parse::<usize>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| format!("invalid corpus row counts: {value}"))?;
    values
        .try_into()
        .map_err(|_| format!("corpus row counts do not have ten fields: {value}"))
}

fn parse_lower_hex<'a>(value: &'a str, length: usize, label: &str) -> Result<&'a str, String> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{label} is not {length}-digit lowercase hex: {value}"
        ));
    }
    Ok(value)
}

fn parse_true_false<'a>(value: &'a str, label: &str) -> Result<&'a str, String> {
    if matches!(value, "true" | "false") {
        Ok(value)
    } else {
        Err(format!("{label} is not true/false: {value}"))
    }
}

fn parse_optional_usize(value: &str, label: &str) -> Result<Option<usize>, String> {
    if value == "-" {
        Ok(None)
    } else {
        value
            .parse::<usize>()
            .map(Some)
            .map_err(|_| format!("{label} is not '-' or unsigned integer: {value}"))
    }
}

fn parse_no_staff(value: &str) -> Result<(), String> {
    let (dimensions, digest) = value
        .split_once(':')
        .ok_or_else(|| format!("invalid noStaff token: {value}"))?;
    let (width, height) = dimensions
        .split_once('x')
        .ok_or_else(|| format!("invalid noStaff dimensions: {value}"))?;
    if width
        .parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
        .is_none()
        || height
            .parse::<usize>()
            .ok()
            .filter(|value| *value > 0)
            .is_none()
    {
        return Err(format!("invalid noStaff dimensions: {value}"));
    }
    parse_lower_hex(digest, 64, "noStaff digest")?;
    Ok(())
}

fn parse_bounds_token(value: &str) -> Result<Bounds, String> {
    let values = value
        .split(':')
        .map(|value| value.parse::<usize>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| format!("invalid bounds token: {value}"))?;
    let [x, y, width, height]: [usize; 4] = values
        .try_into()
        .map_err(|_| format!("bounds token does not have four fields: {value}"))?;
    if width == 0 || height == 0 {
        return Err(format!("bounds token has an empty extent: {value}"));
    }
    Ok(Bounds {
        x,
        y,
        width,
        height,
    })
}

fn parse_rectangle_token(value: &str) -> Result<JavaRectangle, String> {
    let values = value
        .split(':')
        .map(|value| value.parse::<i32>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| format!("invalid Java rectangle token: {value}"))?;
    let [x, y, width, height]: [i32; 4] = values
        .try_into()
        .map_err(|_| format!("Java rectangle does not have four fields: {value}"))?;
    if width <= 0 || height <= 0 {
        return Err(format!("Java rectangle has an empty extent: {value}"));
    }
    Ok(JavaRectangle {
        x,
        y,
        width,
        height,
    })
}

fn parse_run_table_token(value: &str) -> Result<RunTable, String> {
    let (orientation, remainder) = value
        .split_once(':')
        .ok_or_else(|| format!("run-table token lacks orientation: {value}"))?;
    let orientation = match orientation {
        "HORIZONTAL" => Orientation::Horizontal,
        "VERTICAL" => Orientation::Vertical,
        _ => return Err(format!("run-table orientation differs: {value}")),
    };
    let (dimensions, sequences) = remainder
        .split_once(':')
        .ok_or_else(|| format!("run-table token lacks dimensions: {value}"))?;
    let (width, height) = dimensions
        .split_once('x')
        .ok_or_else(|| format!("run-table dimensions differ: {value}"))?;
    let width = width
        .parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("run-table width differs: {value}"))?;
    let height = height
        .parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("run-table height differs: {value}"))?;
    let body = sequences
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| format!("run-table sequences differ: {value}"))?;
    let sequence_tokens = body.split(';').collect::<Vec<_>>();
    let expected_sequences = match orientation {
        Orientation::Horizontal => height,
        Orientation::Vertical => width,
    };
    if sequence_tokens.len() != expected_sequences {
        return Err(format!("run-table sequence coverage differs: {value}"));
    }
    let mut pixels = vec![BACKGROUND; width * height];
    let coordinate_limit = match orientation {
        Orientation::Horizontal => width,
        Orientation::Vertical => height,
    };
    for (expected_sequence, token) in sequence_tokens.into_iter().enumerate() {
        let (sequence, runs) = token
            .split_once('=')
            .ok_or_else(|| format!("run-table sequence lacks '=': {value}"))?;
        if sequence.parse::<usize>().ok() != Some(expected_sequence) {
            return Err(format!("run-table sequence order differs: {value}"));
        }
        if runs == "-" {
            continue;
        }
        let mut prior_stop = None;
        for run in runs.split(',') {
            let (start, length) = run
                .split_once(':')
                .ok_or_else(|| format!("run-table run differs: {value}"))?;
            let start = start
                .parse::<usize>()
                .map_err(|_| format!("run-table start differs: {value}"))?;
            let length = length
                .parse::<usize>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| format!("run-table length differs: {value}"))?;
            let stop = start
                .checked_add(length - 1)
                .filter(|stop| *stop < coordinate_limit)
                .ok_or_else(|| format!("run-table run is out of bounds: {value}"))?;
            if prior_stop.is_some_and(|prior| start <= prior + 1) {
                return Err(format!("run-table runs overlap or touch: {value}"));
            }
            prior_stop = Some(stop);
            for coordinate in start..=stop {
                let (x, y) = match orientation {
                    Orientation::Horizontal => (coordinate, expected_sequence),
                    Orientation::Vertical => (expected_sequence, coordinate),
                };
                pixels[y * width + x] = FOREGROUND;
            }
        }
    }
    let table = RunTable::from_pixels(orientation, width, height, &pixels)
        .map_err(|error| format!("run-table reconstruction failed: {error}"))?;
    if run_table_token(&table) != value {
        return Err(format!("run-table token is not canonical: {value}"));
    }
    Ok(table)
}

fn run_table_token(table: &RunTable) -> String {
    let orientation = match table.orientation() {
        Orientation::Horizontal => "HORIZONTAL",
        Orientation::Vertical => "VERTICAL",
    };
    let mut result = format!("{orientation}:{}x{}:[", table.width(), table.height());
    for sequence in 0..table.sequence_count() {
        if sequence > 0 {
            result.push(';');
        }
        result.push_str(&format!("{sequence}="));
        let runs = table.sequence(sequence).unwrap_or_default();
        if runs.is_empty() {
            result.push('-');
        } else {
            for (ordinal, run) in runs.iter().enumerate() {
                if ordinal > 0 {
                    result.push(',');
                }
                result.push_str(&format!("{}:{}", run.start, run.length));
            }
        }
    }
    result.push(']');
    result
}

fn glyph_run_sha256(table: &RunTable) -> String {
    let orientation = match table.orientation() {
        Orientation::Horizontal => "HORIZONTAL",
        Orientation::Vertical => "VERTICAL",
    };
    let mut bytes = format!("{orientation} {} {}\n", table.width(), table.height()).into_bytes();
    for sequence in 0..table.sequence_count() {
        let mut row = sequence.to_string();
        for run in table.sequence(sequence).unwrap_or_default() {
            write!(row, " {}:{}", run.start, run.length).expect("writing to String cannot fail");
        }
        row.push('\n');
        bytes.extend_from_slice(row.as_bytes());
    }
    sha256_hex(&bytes)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GlyphEvidence {
    bounds: Bounds,
    run_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SelectedGlyphEvidence {
    alias: usize,
    glyph_id: i32,
    membership: String,
    glyph: GlyphEvidence,
}

fn parse_glyph_evidence(value: &str) -> Result<GlyphEvidence, String> {
    let fields = value.split(':').collect::<Vec<_>>();
    if fields.len() != 6 || fields[0] != "g" {
        return Err(format!("invalid glyph token: {value}"));
    }
    let bounds = parse_bounds_token(&fields[1..5].join(":"))?;
    let run_sha256 = parse_lower_hex(fields[5], 64, "glyph run SHA-256")?.to_owned();
    Ok(GlyphEvidence { bounds, run_sha256 })
}

fn parse_selected_glyph(value: &str) -> Result<SelectedGlyphEvidence, String> {
    let fields = value.split(':').collect::<Vec<_>>();
    if fields.len() != 10 || fields[0] != "glyph" || fields[4] != "g" {
        return Err(format!("invalid selected glyph evidence: {value}"));
    }
    let alias = fields[1]
        .parse::<usize>()
        .map_err(|_| format!("selected glyph alias differs: {value}"))?;
    if !matches!(fields[2], "active" | "original-only") {
        return Err(format!("selected glyph membership differs: {value}"));
    }
    let glyph_id = fields[3]
        .strip_prefix("id=")
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("selected glyph ID differs: {value}"))?;
    if alias != usize::try_from(glyph_id).map_err(|_| format!("negative glyph ID: {value}"))? {
        return Err(format!("selected alias/ID join differs: {value}"));
    }
    let glyph = parse_glyph_evidence(&fields[4..].join(":"))?;
    Ok(SelectedGlyphEvidence {
        alias,
        glyph_id,
        membership: fields[2].to_owned(),
        glyph,
    })
}

fn fixed_content(
    bounds: Bounds,
    weight: usize,
    run_table: RunTable,
) -> NativeStemsBeamFixedGlyphContent {
    NativeStemsBeamFixedGlyphContent {
        bounds,
        weight,
        run_table,
    }
}

fn attempt_for_plan(
    system: &NativeStemsBeamLinkPlanSystem,
    plan_ordinal: usize,
) -> Result<&NativeStemsBeamLinkPlanAttempt, String> {
    system
        .builders
        .iter()
        .flat_map(|builder| &builder.attempts)
        .nth(plan_ordinal)
        .ok_or_else(|| {
            format!(
                "system {} lacks flattened plan {plan_ordinal}",
                system.system_id
            )
        })
}

fn attempt_for_plan_mut(
    system: &mut NativeStemsBeamLinkPlanSystem,
    plan_ordinal: usize,
) -> Result<&mut NativeStemsBeamLinkPlanAttempt, String> {
    system
        .builders
        .iter_mut()
        .flat_map(|builder| &mut builder.attempts)
        .nth(plan_ordinal)
        .ok_or_else(|| {
            format!(
                "system {} lacks flattened mutable plan {plan_ordinal}",
                system.system_id
            )
        })
}

fn make_synthetic_absent_compound(
    plans: &mut NativeStemsBeamLinkPlanSystem,
    state: &mut NativeStemsBeamVLinkTransactionState,
    plan_ordinal: usize,
) -> Result<NativeStemsBeamFixedGlyphContent, String> {
    let attempt = attempt_for_plan_mut(plans, plan_ordinal)?;
    if attempt.glyphs.len() < 2 {
        return Err("synthetic new-registration pin needs a compound plan".to_owned());
    }
    let glyph = attempt
        .glyphs
        .last_mut()
        .expect("compound has a last glyph");
    glyph.bounds.x = glyph
        .bounds
        .x
        .checked_add(2)
        .ok_or_else(|| "synthetic glyph shift overflow".to_owned())?;
    glyph.structural_key.left = glyph.bounds.x;
    let binding = state
        .selected_glyph_bindings
        .iter_mut()
        .find(|binding| binding.reference == glyph.reference)
        .ok_or_else(|| "synthetic shifted glyph binding missing".to_owned())?;
    binding.content.bounds = glyph.bounds;

    let candidate = independent_candidate(attempt)?;
    let glyph_scan = state
        .glyph_index
        .exhaustive_lookup
        .as_mut()
        .ok_or_else(|| "synthetic glyph certificate missing".to_owned())?;
    glyph_scan.candidate = candidate.clone();
    glyph_scan.equal_active_matches = 0;
    glyph_scan.equal_original_matches = 0;
    glyph_scan.lookup = NativeStemsBeamExhaustiveGlyphLookup::Absent;
    let stem_scan = state
        .system_stems
        .exhaustive_lookup
        .as_mut()
        .ok_or_else(|| "synthetic systemStem certificate missing".to_owned())?;
    stem_scan.candidate = candidate.clone();
    stem_scan.equal_glyph_matches = 0;
    stem_scan.lookup = NativeStemsBeamExhaustiveSystemStemLookup::Absent;
    Ok(candidate)
}

fn independent_candidate(
    attempt: &NativeStemsBeamLinkPlanAttempt,
) -> Result<NativeStemsBeamFixedGlyphContent, String> {
    let first = attempt
        .glyphs
        .first()
        .ok_or_else(|| "createStem attempt has no glyphs".to_owned())?;
    if attempt.glyphs.len() == 1 {
        return Ok(fixed_content(
            first.bounds,
            first.weight,
            first.structural_key.run_table.clone(),
        ));
    }
    let minimum_x = attempt
        .glyphs
        .iter()
        .map(|glyph| glyph.bounds.x)
        .min()
        .unwrap();
    let minimum_y = attempt
        .glyphs
        .iter()
        .map(|glyph| glyph.bounds.y)
        .min()
        .unwrap();
    let maximum_x = attempt
        .glyphs
        .iter()
        .map(|glyph| glyph.bounds.x + glyph.bounds.width)
        .max()
        .unwrap();
    let maximum_y = attempt
        .glyphs
        .iter()
        .map(|glyph| glyph.bounds.y + glyph.bounds.height)
        .max()
        .unwrap();
    let bounds = Bounds {
        x: minimum_x,
        y: minimum_y,
        width: maximum_x - minimum_x,
        height: maximum_y - minimum_y,
    };
    let mut pixels = vec![BACKGROUND; bounds.width * bounds.height];
    for glyph in &attempt.glyphs {
        for sequence in 0..glyph.structural_key.run_table.sequence_count() {
            for run in glyph
                .structural_key
                .run_table
                .sequence(sequence)
                .unwrap_or_default()
            {
                for coordinate in run.start..=run.stop() {
                    let (local_x, local_y) = match glyph.structural_key.run_table.orientation() {
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
            .map_err(|error| format!("independent compound failed: {error}"))?;
    Ok(fixed_content(bounds, run_table.weight(), run_table))
}

fn state_from_fixture(
    oracle: &OracleSystem,
    scheduler: &audiveris_omr::native_stems_beam_scheduler::NativeStemsBeamSchedulerSystem,
    attempt: &NativeStemsBeamLinkPlanAttempt,
) -> Result<NativeStemsBeamVLinkTransactionState, String> {
    if !scheduler.deferred_line_deltas.is_empty() {
        return Err(format!(
            "system {} needs unsupported known-false fixture state",
            scheduler.system_id
        ));
    }
    let baseline = oracle.row(Family::Baseline)?;
    let frontier_row = oracle.row(Family::Frontier)?;
    let lookup = oracle.row(Family::Lookup)?;
    let frontier = match &scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => {
            return Err(format!(
                "system {} is not a V transaction",
                scheduler.system_id
            ));
        }
    };
    let selected_rows = parse_list(frontier_row.value("selectedGlyphRefs")?)?
        .into_iter()
        .map(parse_selected_glyph)
        .collect::<Result<Vec<_>, _>>()?;
    if selected_rows.len() != attempt.glyphs.len() {
        return Err(format!(
            "system {} selected binding count differs",
            scheduler.system_id
        ));
    }
    let mut selected_glyph_bindings = Vec::with_capacity(attempt.glyphs.len());
    for (selected, glyph) in selected_rows.iter().zip(&attempt.glyphs) {
        let content = fixed_content(
            glyph.bounds,
            glyph.weight,
            glyph.structural_key.run_table.clone(),
        );
        if selected.glyph.bounds != content.bounds
            || selected.glyph.run_sha256 != glyph_run_sha256(&content.run_table)
        {
            return Err(format!(
                "system {} selected structural glyph differs",
                scheduler.system_id
            ));
        }
        selected_glyph_bindings.push(NativeStemsBeamSelectedGlyphBinding {
            reference: glyph.reference,
            canonical_alias: selected.alias,
            glyph_id: selected.glyph_id,
            content,
        });
    }
    let candidate = fixed_content(
        parse_bounds_token(lookup.value("candidateBounds")?)?,
        lookup.usize("candidateWeight")?,
        parse_run_table_token(lookup.value("candidateRunTable")?)?,
    );
    let candidate_evidence = parse_glyph_evidence(lookup.value("candidate")?)?;
    if candidate_evidence.bounds != candidate.bounds
        || candidate_evidence.run_sha256 != glyph_run_sha256(&candidate.run_table)
        || independent_candidate(attempt)? != candidate
    {
        return Err(format!(
            "system {} independent candidate differs",
            scheduler.system_id
        ));
    }
    let glyph_lookup = match lookup.value("lookup")? {
        "Absent" => NativeStemsBeamExhaustiveGlyphLookup::Absent,
        "Present" => NativeStemsBeamExhaustiveGlyphLookup::Present {
            canonical_alias: parse_glyph_alias(lookup.value("presentAlias")?)?,
            glyph_id: i32::try_from(lookup.usize("presentId")?)
                .map_err(|_| "present glyph ID exceeds i32".to_owned())?,
            active_in_index: lookup.value("presentActive")? == "true",
        },
        _ => unreachable!("lookup enum validated"),
    };
    if lookup.value("systemStemLookup")? != "Absent" {
        return Err(format!(
            "system {} real fixture unexpectedly has a baseline system stem",
            scheduler.system_id
        ));
    }
    let allocator = i32::try_from(baseline.usize("allocator")?)
        .map_err(|_| "baseline allocator exceeds i32".to_owned())?;
    Ok(NativeStemsBeamVLinkTransactionState {
        scope: if scheduler.system_id == 1 {
            NativeStemsBeamVLinkTransactionScope::SharedSheetFirstFrontier {
                system_id: scheduler.system_id,
            }
        } else {
            NativeStemsBeamVLinkTransactionScope::IsolatedFreshSheetFrontier {
                system_id: scheduler.system_id,
            }
        },
        glyph_index: NativeStemsBeamGlyphIndexTransactionState {
            persistent_ids: NativeStemsBeamPersistentIdState {
                sheet_last_id: allocator,
                glyph_index_last_id: allocator,
                inter_index_last_id: allocator,
            },
            alias_order: NativeStemsBeamGlyphAliasOrder::JavaGlyphId,
            union_size: lookup.usize("baselineUnionSize")?,
            known_canonical_glyphs: Vec::new(),
            exhaustive_lookup: Some(NativeStemsBeamExhaustiveGlyphEqualsScan {
                candidate: candidate.clone(),
                alias_order: NativeStemsBeamGlyphAliasOrder::JavaGlyphId,
                baseline_union_size: lookup.usize("baselineUnionSize")?,
                baseline_active_count: baseline.usize("glyphActive")?,
                baseline_live_original_count: baseline.usize("glyphOriginals")?,
                baseline_active_sha256: baseline.value("glyphActiveHash")?.to_owned(),
                baseline_live_original_sha256: baseline.value("glyphOriginalsHash")?.to_owned(),
                scanned_active_count: lookup.usize("scannedActive")?,
                scanned_live_original_count: lookup.usize("scannedOriginals")?,
                equal_active_matches: lookup.usize("activeEqualMatches")?,
                equal_original_matches: lookup.usize("originalEqualMatches")?,
                lookup: glyph_lookup,
            }),
        },
        selected_glyph_bindings,
        line_states: vec![NativeStemsBeamVLinkLineState {
            v_linker: frontier.v_linker,
            stored_theoretical_line: attempt.stored_theoretical_line_before,
            builder_line: attempt.initial_stem_line,
            current_attachment_line: attempt
                .attachment_aliases_stored_theoretical_line
                .then_some(attempt.stored_theoretical_line_before),
        }],
        applied_line_deltas: Vec::new(),
        system_stems: NativeStemsBeamSystemStemTransactionState {
            system_id: scheduler.system_id,
            next_stem_identity: 0,
            // Hydrated from Java rows: keep the scan requirement.
            authority: audiveris_omr::native_stems_beam_vlink_transaction::NativeStemsBeamRegistryAuthority::RequiresExhaustiveScan,
            known_stems: Vec::new(),
            exhaustive_lookup: Some(NativeStemsBeamExhaustiveSystemStemEqualsScan {
                candidate,
                baseline_entry_count: baseline.usize("systemStems")?,
                baseline_sha256: baseline.value("systemStemsHash")?.to_owned(),
                scanned_entry_count: lookup.usize("scannedSystemStems")?,
                equal_glyph_matches: lookup.usize("systemStemEqualMatches")?,
                lookup: NativeStemsBeamExhaustiveSystemStemLookup::Absent,
            }),
        },
    })
}

fn checker_context(page: &NativePage) -> NativeStemsBeamStemCheckerContext {
    let interline = page.grid.scale.scale.interline.main;
    NativeStemsBeamStemCheckerContext {
        no_staff: page.grid.no_staff.clone(),
        parameters: NativeStemCheckerParameters {
            interline,
            maximum_stem_width: page.stem_seeds.maximum_stem_thickness,
            belt_margin_dx: (0.15 * f64::from(interline)).round_ties_even() as i32,
            sheet_skew_slope: page.grid.global_slope,
        },
        minimum_stem_grade: STEM_MINIMUM_GRADE,
        artificial_stem_grade: ARTIFICIAL_STEM_GRADE,
    }
}

fn parse_line_token(value: &str) -> Result<(), String> {
    let values = value.split(':').collect::<Vec<_>>();
    if values.len() != 4 {
        return Err(format!("line does not have four coordinates: {value}"));
    }
    for coordinate in values {
        parse_hex_double(coordinate, "line coordinate")?;
    }
    Ok(())
}

fn parse_optional_hex_double(value: &str, label: &str) -> Result<Option<f64>, String> {
    if value == "-" {
        Ok(None)
    } else {
        parse_hex_double(value, label).map(Some)
    }
}

fn parse_hex_double(value: &str, label: &str) -> Result<f64, String> {
    let (java, bits) = value
        .split_once('/')
        .ok_or_else(|| format!("{label} lacks Java-hex/raw-bits pair: {value}"))?;
    let bits = u64::from_str_radix(parse_lower_hex(bits, 16, label)?, 16)
        .map_err(|_| format!("{label} raw bits differ: {value}"))?;
    let parsed = parse_java_hex_float(java)
        .ok_or_else(|| format!("{label} Java hex float differs: {value}"))?;
    if parsed.to_bits() != bits {
        return Err(format!("{label} Java hex and raw bits disagree: {value}"));
    }
    Ok(parsed)
}

fn parse_java_hex_float(value: &str) -> Option<f64> {
    let negative = value.starts_with('-');
    let value = value.strip_prefix('-').unwrap_or(value);
    let (significand, exponent) = value.split_once('p')?;
    let exponent = exponent.parse::<i32>().ok()?;
    let significand = significand.strip_prefix("0x")?;
    let (integer, fraction) = significand.split_once('.')?;
    let mut result = u64::from_str_radix(integer, 16).ok()? as f64;
    let mut place = 1.0 / 16.0;
    for digit in fraction.bytes() {
        let value = char::from(digit).to_digit(16)?;
        result += f64::from(value) * place;
        place /= 16.0;
    }
    result *= 2_f64.powi(exponent);
    Some(if negative { -result } else { result })
}

fn parse_glyph_token(value: &str) -> Result<(), String> {
    let fields = value.split(':').collect::<Vec<_>>();
    if fields.len() != 6 || fields[0] != "g" {
        return Err(format!("invalid glyph token: {value}"));
    }
    for (ordinal, field) in fields[1..5].iter().enumerate() {
        let number = field
            .parse::<i32>()
            .map_err(|_| format!("invalid glyph coordinate {ordinal}: {value}"))?;
        if ordinal >= 2 && number <= 0 {
            return Err(format!("non-positive glyph extent: {value}"));
        }
    }
    parse_lower_hex(fields[5], 64, "glyph run SHA-256")?;
    Ok(())
}

fn parse_list(value: &str) -> Result<Vec<&str>, String> {
    if value == "-" {
        return Ok(Vec::new());
    }
    let body = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| format!("invalid list token: {value}"))?;
    if body.is_empty() || body.split(',').any(str::is_empty) {
        return Err(format!("empty list item: {value}"));
    }
    Ok(body.split(',').collect())
}

fn parse_impacts(value: &str) -> Result<(), String> {
    let impacts = parse_list(value)?;
    if impacts.is_empty() {
        return Err("createStem result has no checker impacts".to_owned());
    }
    let expected = [
        "Slope",
        "Straight",
        "Length",
        "Clean",
        "Black",
        "BlackRatio",
        "Gap",
    ];
    if impacts.len() != expected.len() {
        return Err(format!("checker impact count differs: {value}"));
    }
    for (impact, expected_name) in impacts.into_iter().zip(expected) {
        let (name, rest) = impact
            .split_once(':')
            .ok_or_else(|| format!("invalid checker impact: {impact}"))?;
        if name != expected_name {
            return Err(format!("checker impact {name} is not {expected_name}"));
        }
        let (value, weight) = rest
            .split_once(":w=")
            .ok_or_else(|| format!("checker impact lacks weight: {impact}"))?;
        parse_hex_double(value, "checker impact")?;
        parse_hex_double(weight, "checker weight")?;
    }
    Ok(())
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

fn line_token(line: NativeStemLine) -> String {
    format!(
        "{}:{}:{}:{}",
        hex_double(line.start.x),
        hex_double(line.start.y),
        hex_double(line.stop.x),
        hex_double(line.stop.y)
    )
}

fn rectangle_token(bounds: JavaRectangle) -> String {
    format!(
        "{}:{}:{}:{}",
        bounds.x, bounds.y, bounds.width, bounds.height
    )
}

fn glyph_token(content: &NativeStemsBeamFixedGlyphContent) -> String {
    format!(
        "g:{}:{}:{}:{}:{}",
        content.bounds.x,
        content.bounds.y,
        content.bounds.width,
        content.bounds.height,
        glyph_run_sha256(&content.run_table)
    )
}

fn head_side_token(side: NativeStemHeadSide) -> &'static str {
    match side {
        NativeStemHeadSide::Left => "LEFT",
        NativeStemHeadSide::Right => "RIGHT",
    }
}

fn vertical_side_token(side: NativeStemVerticalSide) -> &'static str {
    match side {
        NativeStemVerticalSide::Top => "TOP",
        NativeStemVerticalSide::Bottom => "BOTTOM",
    }
}

fn beam_sig(
    system: &audiveris_omr::native_stems_beam_stumps::NativeStemsBeamStumpSystem,
    source: audiveris_omr::native_stems_beam_stumps::NativeStemsBeamSource,
) -> Result<usize, String> {
    system
        .beams_by_abscissa
        .iter()
        .find(|beam| beam.source == source)
        .map(|beam| beam.sig_ordinal)
        .ok_or_else(|| format!("system {} lacks scheduler beam source", system.system_id))
}

fn b_alias(
    system: &audiveris_omr::native_stems_beam_stumps::NativeStemsBeamStumpSystem,
    reference: audiveris_omr::native_stems_beam_vlinkers::NativeStemsBeamBLinkerRef,
) -> Result<String, String> {
    let zero_based_id = reference
        .id
        .checked_sub(1)
        .ok_or_else(|| "Java B-linker ID is not one-based".to_owned())?;
    Ok(format!(
        "beam:{}:b:{zero_based_id}",
        beam_sig(system, reference.beam)?
    ))
}

fn registration_token(action: NativeStemsBeamGlyphRegistrationAction) -> &'static str {
    match action {
        NativeStemsBeamGlyphRegistrationAction::Reused {
            reinserted_into_active_index: false,
        } => "ReuseActive",
        NativeStemsBeamGlyphRegistrationAction::Reused {
            reinserted_into_active_index: true,
        } => "ReinsertOriginal",
        NativeStemsBeamGlyphRegistrationAction::Registered => "New",
    }
}

fn disposition_token(disposition: NativeStemsBeamCreateStemDisposition) -> &'static str {
    match disposition {
        NativeStemsBeamCreateStemDisposition::Reused { .. } => "Reused",
        NativeStemsBeamCreateStemDisposition::CreatedChecked { .. } => "CreatedChecked",
        NativeStemsBeamCreateStemDisposition::CreatedArtificial { .. } => "CreatedArtificial",
        NativeStemsBeamCreateStemDisposition::Rejected => "Rejected",
    }
}

fn assert_checker_projection(
    system_id: usize,
    check: &NativeStemCheckResult,
    result: &OracleRow,
) -> Result<(), String> {
    let expected_grade = parse_hex_double(result.value("stemGrade")?, "stemGrade")?;
    if check.grade.to_bits() != expected_grade.to_bits() {
        return Err(format!(
            "system {system_id} checker grade differs: native {}, Java {}",
            hex_double(check.grade),
            result.value("stemGrade")?
        ));
    }
    let actual: [(&str, f64, f64); 7] = [
        ("Slope", check.impacts.slope, 1.0),
        ("Straight", check.impacts.straight, 1.0),
        ("Length", check.impacts.length, 2.0),
        ("Clean", check.impacts.clean, -1.0),
        ("Black", check.impacts.black, 1.0),
        ("BlackRatio", check.impacts.black_ratio, 1.0),
        ("Gap", check.impacts.gap, 5.0),
    ];
    let expected = parse_list(result.value("impacts")?)?;
    if expected.len() != actual.len() {
        return Err(format!("system {system_id} checker impact count differs"));
    }
    for (token, (name, value, weight)) in expected.into_iter().zip(actual) {
        let (actual_name, rest) = token
            .split_once(':')
            .ok_or_else(|| format!("system {system_id} invalid impact token"))?;
        let (expected_value, expected_weight) = rest
            .split_once(":w=")
            .ok_or_else(|| format!("system {system_id} impact weight missing"))?;
        if actual_name != name
            || parse_hex_double(expected_value, "impact")?.to_bits() != value.to_bits()
            || parse_hex_double(expected_weight, "impact weight")?.to_bits() != weight.to_bits()
        {
            return Err(format!(
                "system {system_id} {name} impact differs: native {}:w={}, Java {token}",
                hex_double(value),
                hex_double(weight)
            ));
        }
    }
    Ok(())
}

fn assert_transaction_projection(
    oracle: &OracleSystem,
    attempt: &NativeStemsBeamLinkPlanAttempt,
    before: &NativeStemsBeamVLinkTransactionState,
    state: &NativeStemsBeamVLinkTransactionState,
    transaction: &NativeStemsBeamVLinkTransaction,
) -> Result<(), String> {
    let system_id = oracle.system_id;
    let baseline = oracle.row(Family::Baseline)?;
    let expand = oracle.row(Family::Expand)?;
    let lookup = oracle.row(Family::Lookup)?;
    let result = oracle.row(Family::Result)?;
    let delta = oracle.row(Family::Delta)?;
    let summary = oracle.row(Family::Summary)?;

    if transaction.system_id != system_id
        || transaction.scope != before.scope
        || state.scope != before.scope
        || glyph_token(&transaction.candidate) != lookup.value("candidate")?
        || transaction.candidate != independent_candidate(attempt)?
        || transaction.registration.alias_order != NativeStemsBeamGlyphAliasOrder::JavaGlyphId
        || transaction.registration.canonical_alias
            != parse_glyph_alias(result.value("registeredAlias")?)?
        || transaction.registration.glyph_id
            != i32::try_from(result.usize("registeredGlyphId")?)
                .map_err(|_| "registered glyph ID exceeds i32".to_owned())?
        || transaction.registration.post_union_size != result.usize("postUnionSize")?
        || registration_token(transaction.registration.action) != result.value("registration")?
        || disposition_token(transaction.disposition) != result.value("disposition")?
        || result.value("registration")? != summary.value("registration")?
        || result.value("disposition")? != summary.value("disposition")?
    {
        return Err(format!("system {system_id} transaction projection differs"));
    }
    for (field, expected) in [
        ("stemMinGrade", STEM_MINIMUM_GRADE),
        ("checkerMinThreshold", 0.2),
        ("artificialGrade", ARTIFICIAL_STEM_GRADE),
    ] {
        if parse_hex_double(result.value(field)?, field)?.to_bits() != expected.to_bits() {
            return Err(format!("system {system_id} {field} differs"));
        }
    }

    let canonical_before_id = match lookup.value("lookup")? {
        "Present" => Some(lookup.usize("presentId")?),
        "Absent" => None,
        _ => unreachable!("lookup enum was validated"),
    };
    if parse_optional_usize(
        result.value("canonicalGlyphIdBefore")?,
        "canonicalGlyphIdBefore",
    )? != canonical_before_id
    {
        return Err(format!(
            "system {system_id} canonical predecessor ID differs"
        ));
    }
    let expected_before_id = if attempt.glyphs.len() == 1 {
        Some(
            state
                .selected_glyph_bindings
                .first()
                .ok_or_else(|| format!("system {system_id} singleton binding disappeared"))?
                .glyph_id,
        )
    } else {
        None
    };
    if transaction.candidate_glyph_id_before_registration != expected_before_id {
        return Err(format!(
            "system {system_id} candidate pre-registration ID differs"
        ));
    }
    if result.usize("candidateObjectIdBefore")?
        != transaction
            .candidate_glyph_id_before_registration
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(0)
    {
        return Err(format!("system {system_id} candidate object ID differs"));
    }

    let expected_line_change = expand.value("lineChanged")? == "true";
    if transaction.applied_line_delta.is_some() != expected_line_change {
        return Err(format!("system {system_id} line-delta presence differs"));
    }
    if !transaction.applied_prefix_line_deltas.is_empty() {
        return Err(format!(
            "system {system_id} isolated fixture unexpectedly applied a known-false prefix"
        ));
    }
    let line = state
        .line_states
        .first()
        .ok_or_else(|| format!("system {system_id} current line state disappeared"))?;
    let before_line = before
        .line_states
        .first()
        .ok_or_else(|| format!("system {system_id} baseline line state disappeared"))?;
    let expected_builder_line =
        if expected_line_change && attempt.builder_line_aliases_stored_theoretical_line {
            attempt.stored_theoretical_line_after
        } else {
            before_line.builder_line
        };
    let expected_attachment_line =
        if expected_line_change && attempt.attachment_aliases_stored_theoretical_line {
            Some(attempt.stored_theoretical_line_after)
        } else {
            before_line.current_attachment_line
        };
    if line.stored_theoretical_line != attempt.stored_theoretical_line_after
        || line.builder_line != expected_builder_line
        || line.current_attachment_line != expected_attachment_line
    {
        return Err(format!("system {system_id} committed line state differs"));
    }

    if state.glyph_index.persistent_ids.sheet_last_id
        != i32::try_from(delta.usize("allocatorAfter")?)
            .map_err(|_| "allocatorAfter exceeds i32".to_owned())?
        || state.glyph_index.persistent_ids.glyph_index_last_id
            != state.glyph_index.persistent_ids.sheet_last_id
        || state.glyph_index.persistent_ids.inter_index_last_id
            != state.glyph_index.persistent_ids.sheet_last_id
        || state.glyph_index.union_size != result.usize("postUnionSize")?
        || state.glyph_index.exhaustive_lookup.is_some()
        || state.system_stems.exhaustive_lookup.is_some()
    {
        return Err(format!(
            "system {system_id} committed registry state differs"
        ));
    }
    let canonical = state
        .glyph_index
        .known_canonical_glyphs
        .iter()
        .find(|glyph| glyph.content == transaction.candidate)
        .ok_or_else(|| format!("system {system_id} lacks committed canonical glyph"))?;
    if canonical.canonical_alias != transaction.registration.canonical_alias
        || canonical.glyph_id != transaction.registration.glyph_id
        || !canonical.active_in_index
        || !canonical.strongly_retained
    {
        return Err(format!(
            "system {system_id} canonical registry payload differs"
        ));
    }

    match transaction.disposition {
        NativeStemsBeamCreateStemDisposition::CreatedChecked { stem_identity } => {
            let stem = transaction
                .stem
                .as_ref()
                .ok_or_else(|| format!("system {system_id} checked result lacks stem"))?;
            if stem_identity != before.system_stems.next_stem_identity
                || stem.stem_identity != stem_identity
                || stem.glyph_id != transaction.registration.glyph_id
                || stem.glyph_content != transaction.candidate
                || stem.inter_id.is_some()
                || result.usize("returnedStemInterId")? != 0
                || line_token(stem.geometry.median) != result.value("stemMedian")?
                || parse_hex_double(result.value("stemMeanThickness")?, "stemMeanThickness")?
                    .to_bits()
                    != stem.geometry.mean_thickness.to_bits()
                || parse_rectangle_token(result.value("stemBounds")?)?
                    != stem.geometry.ribbon_bounds
                || (result.value("stemAbnormal")? == "true") != stem.abnormal
                || (result.value("stemSigAttached")? == "true") != stem.sig_attached
                || state.system_stems.known_stems.len() != baseline.usize("systemStems")? + 1
                || state
                    .system_stems
                    .known_stems
                    .iter()
                    .find(|known| known.stem_identity == stem_identity)
                    != Some(stem)
                || state.system_stems.next_stem_identity != stem_identity + 1
            {
                return Err(format!(
                    "system {system_id} checked stem geometry/state differs"
                ));
            }
            let check = transaction
                .checker_result
                .as_ref()
                .ok_or_else(|| format!("system {system_id} checked result lacks checker"))?;
            assert_checker_projection(system_id, check, result)?;
            if !matches!(
                transaction.stem.as_ref().map(|stem| &stem.grade),
                Some(NativeStemsBeamStemGrade::Checked(stem_check)) if stem_check == check
            ) {
                return Err(format!("system {system_id} checked stem grade differs"));
            }
        }
        _ => {
            return Err(format!(
                "system {system_id} frozen Chula disposition unexpectedly differs"
            ));
        }
    }

    let expected_mutations = if expected_line_change {
        vec![
            NativeStemsBeamVLinkMutation::StoredLineDeltaApplied,
            NativeStemsBeamVLinkMutation::SystemStemInserted { stem_identity: 0 },
        ]
    } else {
        vec![NativeStemsBeamVLinkMutation::SystemStemInserted { stem_identity: 0 }]
    };
    if transaction.mutation_order != expected_mutations
        || transaction.sig_vertex_mutation_count != 0
        || transaction.sig_relation_mutation_count != 0
        || transaction.linker_flag_mutation_count != 0
        || before.applied_line_deltas.len() + usize::from(expected_line_change)
            != state.applied_line_deltas.len()
    {
        return Err(format!("system {system_id} transaction guard differs"));
    }
    Ok(())
}

fn replay_fixture(
    fixture: &OracleFixture,
    native: &NativePage,
) -> Result<(usize, usize, usize), String> {
    if native.scheduler.systems.len() != fixture.systems.len() {
        return Err(format!("{} native system count differs", fixture.page));
    }
    let checker = checker_context(native);
    let mut compound_frontiers = 0;
    let mut singleton_frontiers = 0;
    let mut changed_lines = 0;
    for oracle in &fixture.systems {
        let system_id = oracle.system_id;
        let scheduler = native
            .scheduler
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .ok_or_else(|| format!("system {system_id} scheduler product missing"))?;
        let builder = native
            .beam_builders
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .ok_or_else(|| format!("system {system_id} builder product missing"))?;
        let plans = native
            .plans
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .ok_or_else(|| format!("system {system_id} plan product missing"))?;
        let stumps = native
            .beam_stumps
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .ok_or_else(|| format!("system {system_id} stump product missing"))?;
        let frontier = match &scheduler.status {
            NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
            _ => return Err(format!("system {system_id} is not createStem-ready")),
        };
        let frontier_row = oracle.row(Family::Frontier)?;
        let expand = oracle.row(Family::Expand)?;
        let attempt = attempt_for_plan(plans, frontier.plan.plan_ordinal)?;
        if frontier.snapshot.current_index != frontier_row.usize("beamOrder")?
            || beam_sig(stumps, frontier.beam)? != frontier_row.usize("beamSig")?
            || frontier
                .horizontal_side
                .map(head_side_token)
                .ok_or_else(|| format!("system {system_id} lacks horizontal side"))?
                != frontier_row.value("hSide")?
            || b_alias(stumps, frontier.b_linker)? != frontier_row.value("bAlias")?
            || vertical_side_token(frontier.vertical_side) != frontier_row.value("vSide")?
            || frontier.plan.builder_ordinal != frontier_row.usize("builder")?
            || frontier.plan.plan_ordinal != frontier_row.usize("plan")?
            || usize::try_from(frontier.plan.stem_profile).ok()
                != Some(frontier_row.usize("stemProfile")?)
            || usize::try_from(plans.link_profile).ok() != Some(frontier_row.usize("linkProfile")?)
            || line_token(attempt.stored_theoretical_line_before)
                != frontier_row.value("lineBefore")?
            || attempt
                .expand_last_index
                .and_then(|value| usize::try_from(value).ok())
                != Some(expand.usize("lastIndex")?)
            || attempt.relations.len() != expand.usize("relations")?
            || attempt.glyphs.len() != expand.usize("glyphs")?
            || line_token(attempt.stored_theoretical_line_after) != expand.value("lineAfter")?
            || attempt.stored_theoretical_line_would_mutate
                != (expand.value("lineChanged")? == "true")
            || attempt.builder_line_aliases_stored_theoretical_line
                != (expand.value("builderAliases")? == "true")
            || attempt.attachment_aliases_stored_theoretical_line
                != (expand.value("attachmentAliases")? == "true")
        {
            return Err(format!(
                "system {system_id} immutable frontier join differs"
            ));
        }
        if attempt.glyphs.len() == 1 {
            singleton_frontiers += 1;
        } else {
            compound_frontiers += 1;
        }
        changed_lines += usize::from(attempt.stored_theoretical_line_would_mutate);

        let mut state = state_from_fixture(oracle, scheduler, attempt)?;
        let before = state.clone();
        let transaction = apply_native_stems_beam_vlink_create_stem_transaction(
            scheduler, builder, plans, &mut state, &checker,
        )
        .map_err(|error| format!("system {system_id} transaction failed: {error}"))?;
        assert_transaction_projection(oracle, attempt, &before, &state, &transaction)?;
    }
    Ok((compound_frontiers, singleton_frontiers, changed_lines))
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
    let trailer_marker = b"stemsbeamcreatestemcorpussummary ";
    let trailer_start = find_bytes(bytes, trailer_marker)
        .ok_or_else(|| "fixture lacks createStem corpus trailer".to_owned())?;
    if find_bytes(
        &bytes[trailer_start + trailer_marker.len()..],
        trailer_marker,
    )
    .is_some()
    {
        return Err("fixture has duplicate createStem corpus trailer".to_owned());
    }
    let body = &bytes[..trailer_start];
    let page_marker = b"stemsbeamcreatestempage ";
    let semantic_start = find_bytes(body, page_marker)
        .ok_or_else(|| "fixture body lacks createStem page row".to_owned())?;
    if !body.ends_with(b"\n") || !bytes.ends_with(b"\n") {
        return Err("fixture and emitted body must end in newline".to_owned());
    }
    Ok(FixtureSlices {
        body,
        header: &body[..semantic_start],
        semantic: &body[semantic_start..],
    })
}

fn validate_manifest_fixture(
    entry: &ManifestEntry,
    bytes: &[u8],
    probe_sha256: &str,
    runner_sha256: &str,
) -> Result<OracleFixture, String> {
    if bytes.len() != entry.fixture_bytes
        || line_count(bytes) != entry.fixture_lines
        || sha256_hex(bytes) != entry.fixture_sha256
    {
        return Err(format!("{} fixture fingerprint differs", entry.page));
    }
    let slices = fixture_slices(bytes)?;
    if slices.body.len() != entry.body_bytes
        || line_count(slices.body) != entry.body_lines
        || sha256_hex(slices.body) != entry.body_sha256
    {
        return Err(format!("{} emitted-body fingerprint differs", entry.page));
    }
    let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let fixture = OracleFixture::parse(text)?;
    let systems = fixture.systems.len();
    let row_counts = [
        1, systems, systems, systems, systems, systems, systems, systems, systems, 1,
    ];
    if fixture.page != entry.page
        || entry.row_counts != row_counts
        || entry.java_processes != systems
        || fixture.page_row.value("schedulerFixtureSha256")? != entry.scheduler_sha256
        || fixture.page_row.value("expandFixtureSha256")? != entry.expand_sha256
        || fixture.corpus_summary.probe_sha256 != probe_sha256
        || fixture.corpus_summary.runner_sha256 != runner_sha256
        || fixture.corpus_summary.body_sha256 != entry.body_sha256
        || fixture.corpus_summary.body_lines != entry.body_lines
        || fixture.corpus_summary.body_bytes != entry.body_bytes
        || fixture.corpus_summary.row_counts != entry.row_counts
        || fixture.corpus_summary.java_processes_per_page != entry.java_processes
    {
        return Err(format!("{} manifest/fixture algebra differs", entry.page));
    }
    Ok(fixture)
}

#[test]
fn beam_create_stem_parser_fails_closed_on_schema_and_hierarchy_drift() {
    let row = concat!(
        "stemsbeamcreatestemfrontier chula.png#1 system 1 beamOrder 2 beamSig 9 ",
        "hSide LEFT bAlias beam:9:b:0 vSide TOP builder 18 plan 90 stemProfile 4 ",
        "linkProfile 1 lineBefore 0x0.0p0/0000000000000000:0x0.0p0/0000000000000000:",
        "0x1.0p0/3ff0000000000000:0x1.0p0/3ff0000000000000 selectedGlyphRefs [x]"
    );
    let parsed = parse_row(row, 7).expect("canonical createStem frontier row");
    assert_eq!(parsed.family, Family::Frontier);
    assert_eq!(parsed.usize("system").unwrap(), 1);
    assert!(parse_row(&row.replace("lineBefore", "renamedLine"), 7).is_err());
    assert!(parse_row(&row.replace("stemsbeamcreatestemfrontier", "unknown"), 7).is_err());
    assert!(
        OracleFixture::parse(row).is_err(),
        "schema/header and hierarchy are mandatory"
    );
}

#[test]
fn beam_create_stem_chula_fixture_parses_fail_closed() {
    let bytes = std::fs::read(repo_root().join("rust/oracle/stems-beam-create-stem-chula.txt"))
        .expect("frozen Chula createStem fixture");
    assert_eq!(sha256_hex(&bytes), EXPECTED_CHULA_FIXTURE_SHA256);
    let text = std::str::from_utf8(&bytes).expect("fixture UTF-8");
    let fixture = OracleFixture::parse(text).expect("fail-closed Chula fixture parser");
    assert_eq!(fixture.systems.len(), 3);
    assert_eq!(fixture.corpus_summary.body_lines, 31);
    assert_eq!(fixture.corpus_summary.body_bytes, 15_812);
    assert!(
        OracleFixture::parse(&text.replacen(
            "systemStemLookup Absent",
            "systemStemLookup Present",
            1,
        ))
        .is_err(),
        "v1 compact envelope cannot hydrate a present system stem"
    );
    assert!(
        OracleFixture::parse(&text.replacen("reusedStems 0", "reusedStems 1", 1)).is_err(),
        "v1 real corpus pins reused system stems to zero"
    );
    assert!(
        OracleFixture::parse(&text.replacen("systemStemInterId", "systemStemId", 1,)).is_err(),
        "legacy ambiguous ID labels fail closed"
    );
    assert!(
        OracleFixture::parse(&text.replacen("returnedStemInterId 0", "returnedStemInterId 1", 1,))
            .is_err(),
        "created stems must retain the explicit pre-SIG null-ID sentinel"
    );
    assert!(
        OracleFixture::parse(&text.replacen("stemAbnormal false", "stemAbnormal true", 1,))
            .is_err(),
        "created stem abnormal state is evidence, not decoration"
    );
    assert!(
        OracleFixture::parse(&text.replacen("stemSigAttached false", "stemSigAttached true", 1,))
            .is_err(),
        "createStem must stop before SIG attachment"
    );
}

#[test]
fn native_beam_create_stem_matches_java_chula_exactly() {
    let text =
        std::fs::read_to_string(repo_root().join("rust/oracle/stems-beam-create-stem-chula.txt"))
            .expect("frozen Chula createStem fixture");
    let fixture = OracleFixture::parse(&text).expect("fail-closed Chula fixture parser");
    let native = native_page("chula.png");
    let checker = checker_context(&native);
    assert_eq!(native.scheduler.systems.len(), fixture.systems.len());

    let mut compound_frontiers = 0;
    let mut singleton_frontiers = 0;
    let mut changed_lines = 0;
    for oracle in &fixture.systems {
        let system_id = oracle.system_id;
        let scheduler = native
            .scheduler
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .unwrap_or_else(|| panic!("missing scheduler system {system_id}"));
        let builder = native
            .beam_builders
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .unwrap_or_else(|| panic!("missing builder system {system_id}"));
        let plans = native
            .plans
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .unwrap_or_else(|| panic!("missing plan system {system_id}"));
        let stumps = native
            .beam_stumps
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .unwrap_or_else(|| panic!("missing stump system {system_id}"));
        let frontier = match &scheduler.status {
            NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
            status => panic!("system {system_id} is not createStem-ready: {status:?}"),
        };
        let frontier_row = oracle.row(Family::Frontier).expect("frontier row");
        let expand_row = oracle.row(Family::Expand).expect("expand row");
        let attempt = attempt_for_plan(plans, frontier.plan.plan_ordinal)
            .unwrap_or_else(|error| panic!("{error}"));

        assert_eq!(
            frontier.snapshot.current_index,
            frontier_row.usize("beamOrder").unwrap()
        );
        assert_eq!(
            beam_sig(stumps, frontier.beam).unwrap(),
            frontier_row.usize("beamSig").unwrap()
        );
        assert_eq!(
            head_side_token(frontier.horizontal_side.expect("beam side")),
            frontier_row.value("hSide").unwrap()
        );
        assert_eq!(
            b_alias(stumps, frontier.b_linker).unwrap(),
            frontier_row.value("bAlias").unwrap()
        );
        assert_eq!(
            vertical_side_token(frontier.vertical_side),
            frontier_row.value("vSide").unwrap()
        );
        assert_eq!(
            frontier.plan.builder_ordinal,
            frontier_row.usize("builder").unwrap()
        );
        assert_eq!(
            frontier.plan.plan_ordinal,
            frontier_row.usize("plan").unwrap()
        );
        assert_eq!(
            usize::try_from(frontier.plan.stem_profile).unwrap(),
            frontier_row.usize("stemProfile").unwrap()
        );
        assert_eq!(
            usize::try_from(plans.link_profile).unwrap(),
            frontier_row.usize("linkProfile").unwrap()
        );
        assert_eq!(
            line_token(attempt.stored_theoretical_line_before),
            frontier_row.value("lineBefore").unwrap()
        );
        assert_eq!(
            attempt
                .expand_last_index
                .and_then(|value| usize::try_from(value).ok()),
            Some(expand_row.usize("lastIndex").unwrap())
        );
        assert_eq!(
            attempt.relations.len(),
            expand_row.usize("relations").unwrap()
        );
        assert_eq!(attempt.glyphs.len(), expand_row.usize("glyphs").unwrap());
        assert_eq!(
            line_token(attempt.stored_theoretical_line_after),
            expand_row.value("lineAfter").unwrap()
        );
        assert_eq!(
            attempt.stored_theoretical_line_would_mutate,
            expand_row.value("lineChanged").unwrap() == "true"
        );
        assert_eq!(
            attempt.builder_line_aliases_stored_theoretical_line,
            expand_row.value("builderAliases").unwrap() == "true"
        );
        assert_eq!(
            attempt.attachment_aliases_stored_theoretical_line,
            expand_row.value("attachmentAliases").unwrap() == "true"
        );

        if attempt.glyphs.len() == 1 {
            singleton_frontiers += 1;
        } else {
            compound_frontiers += 1;
        }
        changed_lines += usize::from(attempt.stored_theoretical_line_would_mutate);

        let mut state = state_from_fixture(oracle, scheduler, attempt)
            .unwrap_or_else(|error| panic!("system {system_id}: {error}"));
        let before = state.clone();
        let transaction = apply_native_stems_beam_vlink_create_stem_transaction(
            scheduler, builder, plans, &mut state, &checker,
        )
        .unwrap_or_else(|error| panic!("system {system_id} transaction failed: {error}"));
        assert_transaction_projection(oracle, attempt, &before, &state, &transaction)
            .unwrap_or_else(|error| panic!("{error}"));
    }
    assert_eq!(
        (compound_frontiers, singleton_frontiers, changed_lines),
        (2, 1, 1)
    );
}

#[test]
fn native_beam_create_stem_matches_java_corpus_exactly() {
    let root = repo_root();
    let manifest_bytes =
        std::fs::read(root.join(MANIFEST_PATH)).expect("frozen createStem manifest");
    assert_eq!(sha256_hex(&manifest_bytes), EXPECTED_MANIFEST_SHA256);
    let manifest = CreateStemManifest::parse(&manifest_bytes)
        .unwrap_or_else(|error| panic!("invalid createStem manifest: {error}"));
    assert_eq!(manifest.probe_sha256, EXPECTED_PROBE_SHA256);
    assert_eq!(manifest.runner_sha256, EXPECTED_RUNNER_SHA256);
    assert_eq!(manifest.corpus_body_sha256, EXPECTED_CORPUS_BODY_SHA256);
    assert_eq!(manifest.corpus_body_lines, EXPECTED_CORPUS_BODY_LINES);
    assert_eq!(manifest.corpus_body_bytes, EXPECTED_CORPUS_BODY_BYTES);
    assert_eq!(manifest.corpus_row_counts, EXPECTED_CORPUS_ROW_COUNTS);
    assert_eq!(manifest.manifest_body_sha256, EXPECTED_MANIFEST_BODY_SHA256);
    assert_eq!(manifest.manifest_body_lines, EXPECTED_MANIFEST_BODY_LINES);
    assert_eq!(manifest.manifest_body_bytes, EXPECTED_MANIFEST_BODY_BYTES);
    assert_eq!(
        sha256_hex(&std::fs::read(root.join(PROBE_PATH)).expect("createStem probe source")),
        manifest.probe_sha256
    );
    assert_eq!(
        sha256_hex(&std::fs::read(root.join(RUNNER_PATH)).expect("createStem runner source")),
        manifest.runner_sha256
    );
    let manifest_summary_start =
        find_bytes(&manifest_bytes, b"stemsbeamcreatestemmanifestsummary ")
            .expect("createStem manifest summary");
    let manifest_body = &manifest_bytes[..manifest_summary_start];
    assert_eq!(sha256_hex(manifest_body), manifest.manifest_body_sha256);
    assert_eq!(line_count(manifest_body), manifest.manifest_body_lines);
    assert_eq!(manifest_body.len(), manifest.manifest_body_bytes);

    let mut common_header = None::<Vec<u8>>;
    let mut corpus_body = Vec::with_capacity(manifest.corpus_body_bytes);
    let mut row_counts = [0_usize; 10];
    let mut java_processes = 0;
    let mut transaction_count = 0;
    let mut compound_count = 0;
    let mut singleton_count = 0;
    let mut changed_line_count = 0;
    for (entry, spec) in manifest.entries.iter().zip(PAGES) {
        let bytes = std::fs::read(root.join("rust/oracle").join(&entry.fixture))
            .unwrap_or_else(|error| panic!("{} fixture missing: {error}", entry.page));
        let fixture = validate_manifest_fixture(
            entry,
            &bytes,
            &manifest.probe_sha256,
            &manifest.runner_sha256,
        )
        .unwrap_or_else(|error| panic!("{}: {error}", entry.page));
        let slices =
            fixture_slices(&bytes).unwrap_or_else(|error| panic!("{}: {error}", entry.page));
        if let Some(expected) = &common_header {
            assert_eq!(slices.header, expected, "{} common header", entry.page);
        } else {
            common_header = Some(slices.header.to_vec());
            corpus_body.extend_from_slice(slices.header);
        }
        corpus_body.extend_from_slice(slices.semantic);
        for (total, count) in row_counts.iter_mut().zip(entry.row_counts) {
            *total += count;
        }
        java_processes += entry.java_processes;

        let native = native_page(spec.image);
        let (compound, singleton, changed) = replay_fixture(&fixture, &native)
            .unwrap_or_else(|error| panic!("{}: {error}", entry.page));
        transaction_count += fixture.systems.len();
        compound_count += compound;
        singleton_count += singleton;
        changed_line_count += changed;
    }
    assert_eq!(row_counts, manifest.corpus_row_counts);
    assert_eq!(java_processes, manifest.total_java_processes);
    assert_eq!(transaction_count, 30);
    assert_eq!(
        (compound_count, singleton_count, changed_line_count),
        (15, 15, 14)
    );
    assert_eq!(corpus_body.len(), manifest.corpus_body_bytes);
    assert_eq!(line_count(&corpus_body), manifest.corpus_body_lines);
    assert_eq!(sha256_hex(&corpus_body), manifest.corpus_body_sha256);
}

#[test]
fn native_beam_create_stem_synthetic_transaction_invariants() {
    let text =
        std::fs::read_to_string(repo_root().join("rust/oracle/stems-beam-create-stem-chula.txt"))
            .expect("frozen Chula createStem fixture");
    let fixture = OracleFixture::parse(&text).expect("fail-closed Chula fixture parser");
    let native = native_page("chula.png");
    let checker = checker_context(&native);
    let oracle = &fixture.systems[0];
    let scheduler = &native.scheduler.systems[0];
    let builder = &native.beam_builders.systems[0];
    let plans = &native.plans.systems[0];
    let frontier = match &scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => panic!("Chula system 1 is not createStem-ready"),
    };
    let attempt = attempt_for_plan(plans, frontier.plan.plan_ordinal).expect("Chula plan");
    let base = state_from_fixture(oracle, scheduler, attempt).expect("synthetic baseline state");

    // A count/hash snapshot without a candidate-specific exact scan is not a
    // complete registry. The current invocation is unchanged on the typed
    // failure.
    let mut incomplete = base.clone();
    incomplete.glyph_index.exhaustive_lookup = None;
    let before_incomplete = incomplete.clone();
    assert!(matches!(
        apply_native_stems_beam_vlink_create_stem_transaction(
            scheduler,
            builder,
            plans,
            &mut incomplete,
            &checker,
        ),
        Err(NativeStemsBeamVLinkTransactionError::AwaitingCompleteGlyphRegistry)
    ));
    assert_eq!(incomplete, before_incomplete);

    let mut wrong_sheet_scope = base.clone();
    wrong_sheet_scope.scope =
        NativeStemsBeamVLinkTransactionScope::SharedSheetFirstFrontier { system_id: 2 };
    let before_wrong_scope = wrong_sheet_scope.clone();
    assert!(matches!(
        apply_native_stems_beam_vlink_create_stem_transaction(
            scheduler,
            builder,
            plans,
            &mut wrong_sheet_scope,
            &checker,
        ),
        Err(NativeStemsBeamVLinkTransactionError::SystemOrder)
    ));
    assert_eq!(wrong_sheet_scope, before_wrong_scope);

    // A canonical original outside the active index is reinserted without an
    // allocation, preserving its direct Java-ID alias.
    let mut reinsert = base.clone();
    let scan = reinsert
        .glyph_index
        .exhaustive_lookup
        .as_mut()
        .expect("reinsert certificate");
    let (canonical_alias, glyph_id) = match scan.lookup {
        NativeStemsBeamExhaustiveGlyphLookup::Present {
            canonical_alias,
            glyph_id,
            ..
        } => (canonical_alias, glyph_id),
        NativeStemsBeamExhaustiveGlyphLookup::Absent => panic!("Chula canonical missing"),
    };
    scan.equal_active_matches = 0;
    scan.lookup = NativeStemsBeamExhaustiveGlyphLookup::Present {
        canonical_alias,
        glyph_id,
        active_in_index: false,
    };
    let reinserted = apply_native_stems_beam_vlink_create_stem_transaction(
        scheduler,
        builder,
        plans,
        &mut reinsert,
        &checker,
    )
    .expect("canonical reinsert transaction");
    assert_eq!(
        reinserted.registration.action,
        NativeStemsBeamGlyphRegistrationAction::Reused {
            reinserted_into_active_index: true
        }
    );
    assert_eq!(reinserted.registration.canonical_alias, glyph_id as usize);
    assert_eq!(
        reinserted.mutation_order,
        vec![
            NativeStemsBeamVLinkMutation::GlyphReinserted { glyph_id },
            NativeStemsBeamVLinkMutation::SystemStemInserted { stem_identity: 0 },
        ]
    );

    // Profile 4 retains a below-threshold checker result as an artificial
    // StemInter with the exact configured grade.
    let mut artificial_state = base.clone();
    let mut artificial_checker = checker.clone();
    artificial_checker.minimum_stem_grade = 2.0;
    let artificial = apply_native_stems_beam_vlink_create_stem_transaction(
        scheduler,
        builder,
        plans,
        &mut artificial_state,
        &artificial_checker,
    )
    .expect("profile-4 artificial transaction");
    assert!(matches!(
        artificial.disposition,
        NativeStemsBeamCreateStemDisposition::CreatedArtificial { stem_identity: 0 }
    ));
    assert!(artificial.checker_result.is_some());
    assert!(matches!(
        artificial.stem.as_ref().map(|stem| &stem.grade),
        Some(NativeStemsBeamStemGrade::Artificial(grade))
            if grade.to_bits() == ARTIFICIAL_STEM_GRADE.to_bits()
    ));
    let artificial_stem = artificial.stem.as_ref().expect("artificial stem payload");
    assert_eq!(artificial_stem.inter_id, None);
    assert!(!artificial_stem.sig_attached);
    assert!(!artificial_stem.abnormal);
    assert!(artificial_stem.geometry.mean_thickness > 0.0);
    assert!(!artificial_stem.geometry.ribbon_bounds.is_empty());
    assert_eq!(
        artificial_state.system_stems.known_stems.first(),
        Some(artificial_stem)
    );

    // The just-created systemStem is found structurally on the next call.
    // Its native dense identity is independent of its still-zero Java Inter
    // ID before SIG insertion.
    let mut reuse_state = base.clone();
    let created = apply_native_stems_beam_vlink_create_stem_transaction(
        scheduler,
        builder,
        plans,
        &mut reuse_state,
        &checker,
    )
    .expect("initial checked stem");
    assert!(matches!(
        created.disposition,
        NativeStemsBeamCreateStemDisposition::CreatedChecked { stem_identity: 0 }
    ));
    assert_eq!(created.stem.as_ref().and_then(|stem| stem.inter_id), None);
    let created_stem = created.stem.as_ref().expect("checked stem payload").clone();
    assert!(!created_stem.sig_attached);
    assert!(!created_stem.abnormal);
    assert!(created_stem.geometry.mean_thickness > 0.0);
    assert!(!created_stem.geometry.ribbon_bounds.is_empty());
    let reused = apply_native_stems_beam_vlink_create_stem_transaction(
        scheduler,
        builder,
        plans,
        &mut reuse_state,
        &checker,
    )
    .expect("existing systemStem reuse");
    assert_eq!(
        reused.disposition,
        NativeStemsBeamCreateStemDisposition::Reused { stem_identity: 0 }
    );
    assert!(reused.checker_result.is_none());
    assert_eq!(reused.stem.as_ref().and_then(|stem| stem.inter_id), None);
    assert_eq!(reused.stem.as_ref(), Some(&created_stem));
    assert!(reused.mutation_order.is_empty());
    assert_eq!(reuse_state.system_stems.known_stems.len(), 1);

    // All three allocator views are one Java AtomicInteger. A partial
    // interleaving is rejected; a coherent external allocation is retained
    // and the new glyph receives the following shared ID.
    let mut split_allocator = base.clone();
    split_allocator
        .glyph_index
        .persistent_ids
        .glyph_index_last_id += 1;
    let before_split = split_allocator.clone();
    assert!(matches!(
        apply_native_stems_beam_vlink_create_stem_transaction(
            scheduler,
            builder,
            plans,
            &mut split_allocator,
            &checker,
        ),
        Err(NativeStemsBeamVLinkTransactionError::PersistentAllocatorMismatch)
    ));
    assert_eq!(split_allocator, before_split);

    let interleaved_scheduler = scheduler.clone();
    let mut interleaved_plans = plans.clone();
    let mut interleaved_state = base.clone();
    let prior_id = interleaved_state.glyph_index.persistent_ids.sheet_last_id + 17;
    interleaved_state.glyph_index.persistent_ids = NativeStemsBeamPersistentIdState {
        sheet_last_id: prior_id,
        glyph_index_last_id: prior_id,
        inter_index_last_id: prior_id,
    };
    let candidate = make_synthetic_absent_compound(
        &mut interleaved_plans,
        &mut interleaved_state,
        frontier.plan.plan_ordinal,
    )
    .expect("distinct synthetic compound");
    let registered = apply_native_stems_beam_vlink_create_stem_transaction(
        &interleaved_scheduler,
        builder,
        &interleaved_plans,
        &mut interleaved_state,
        &checker,
    )
    .expect("shared-allocator new registration");
    assert_eq!(registered.candidate, candidate);
    assert_eq!(registered.candidate_glyph_id_before_registration, None);
    assert_eq!(registered.registration.glyph_id, prior_id + 1);
    assert_eq!(
        registered.registration.canonical_alias,
        (prior_id + 1) as usize
    );
    assert_eq!(
        registered.registration.action,
        NativeStemsBeamGlyphRegistrationAction::Registered
    );
    assert_eq!(
        interleaved_state.glyph_index.persistent_ids,
        NativeStemsBeamPersistentIdState {
            sheet_last_id: prior_id + 1,
            glyph_index_last_id: prior_id + 1,
            inter_index_last_id: prior_id + 1,
        }
    );

    // A non-profile-4 null outcome still commits Java's registration prefix.
    // A rejected new compound then fails closed until weak-liveness evidence
    // is refreshed.
    let mut rejected_scheduler = interleaved_scheduler.clone();
    let mut rejected_plans = plans.clone();
    let mut rejected_state = base.clone();
    let rejected_candidate = make_synthetic_absent_compound(
        &mut rejected_plans,
        &mut rejected_state,
        frontier.plan.plan_ordinal,
    )
    .expect("rejected synthetic compound");
    let rejected_attempt =
        attempt_for_plan_mut(&mut rejected_plans, frontier.plan.plan_ordinal).unwrap();
    rejected_attempt.stem_profile = 3;
    match &mut rejected_scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => {
            frontier.plan.stem_profile = 3;
        }
        _ => unreachable!("cloned Chula createStem frontier"),
    }
    let mut rejecting_checker = checker.clone();
    rejecting_checker.minimum_stem_grade = 2.0;
    let allocator_before_rejection = rejected_state.glyph_index.persistent_ids.sheet_last_id;
    let rejected = apply_native_stems_beam_vlink_create_stem_transaction(
        &rejected_scheduler,
        builder,
        &rejected_plans,
        &mut rejected_state,
        &rejecting_checker,
    )
    .expect("ordinary Java null rejection");
    assert_eq!(rejected.candidate, rejected_candidate);
    assert_eq!(
        rejected.disposition,
        NativeStemsBeamCreateStemDisposition::Rejected
    );
    assert!(rejected.checker_result.is_some());
    assert!(rejected.stem.is_none());
    assert_eq!(
        rejected.registration.glyph_id,
        allocator_before_rejection + 1
    );
    assert_eq!(
        rejected.mutation_order,
        vec![
            NativeStemsBeamVLinkMutation::GlyphRegistered {
                glyph_id: allocator_before_rejection + 1,
            },
            NativeStemsBeamVLinkMutation::RegistryWeakLivenessBecameUnknown {
                glyph_id: allocator_before_rejection + 1,
            },
        ]
    );
    assert!(
        rejected_state
            .glyph_index
            .known_canonical_glyphs
            .iter()
            .any(|glyph| glyph.content == rejected_candidate && !glyph.strongly_retained)
    );
    let after_rejection = rejected_state.clone();
    assert!(matches!(
        apply_native_stems_beam_vlink_create_stem_transaction(
            &rejected_scheduler,
            builder,
            &rejected_plans,
            &mut rejected_state,
            &rejecting_checker,
        ),
        Err(NativeStemsBeamVLinkTransactionError::AwaitingCompleteGlyphRegistry)
    ));
    assert_eq!(rejected_state, after_rejection);

    // A compound can be structurally identical to one of its selected,
    // registered components (the real Chula system-1 frontier is such a
    // case). That selected object is both proof that an exhaustive Absent
    // result is impossible and a strong reference to a reused canonical
    // original after an ordinary rejection.
    let selected_equal_candidate = independent_candidate(attempt).expect("Chula compound");
    assert!(attempt.glyphs.len() > 1);
    let selected_equal_binding = base
        .selected_glyph_bindings
        .iter()
        .find(|binding| binding.content == selected_equal_candidate)
        .expect("Chula compound equals one selected canonical component");
    let mut contradictory_absent = base.clone();
    let contradictory_scan = contradictory_absent
        .glyph_index
        .exhaustive_lookup
        .as_mut()
        .expect("Chula candidate certificate");
    contradictory_scan.equal_active_matches = 0;
    contradictory_scan.equal_original_matches = 0;
    contradictory_scan.lookup = NativeStemsBeamExhaustiveGlyphLookup::Absent;
    let before_contradictory_absent = contradictory_absent.clone();
    assert!(matches!(
        apply_native_stems_beam_vlink_create_stem_transaction(
            scheduler,
            builder,
            plans,
            &mut contradictory_absent,
            &checker,
        ),
        Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
            phase: "registered selected glyph absent from originals"
        })
    ));
    assert_eq!(contradictory_absent, before_contradictory_absent);

    let mut rejected_reuse_scheduler = scheduler.clone();
    let mut rejected_reuse_plans = plans.clone();
    attempt_for_plan_mut(&mut rejected_reuse_plans, frontier.plan.plan_ordinal)
        .expect("Chula mutable plan")
        .stem_profile = 3;
    match &mut rejected_reuse_scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(current) => {
            current.plan.stem_profile = 3;
        }
        _ => unreachable!("cloned Chula createStem frontier"),
    }
    let mut rejected_reuse_state = base.clone();
    let rejected_reuse = apply_native_stems_beam_vlink_create_stem_transaction(
        &rejected_reuse_scheduler,
        builder,
        &rejected_reuse_plans,
        &mut rejected_reuse_state,
        &rejecting_checker,
    )
    .expect("selected-canonical rejected reuse");
    assert_eq!(
        rejected_reuse.disposition,
        NativeStemsBeamCreateStemDisposition::Rejected
    );
    assert_eq!(
        rejected_reuse.registration.action,
        NativeStemsBeamGlyphRegistrationAction::Reused {
            reinserted_into_active_index: false
        }
    );
    assert!(
        !rejected_reuse
            .mutation_order
            .iter()
            .any(|mutation| matches!(
                mutation,
                NativeStemsBeamVLinkMutation::RegistryWeakLivenessBecameUnknown { .. }
            ))
    );
    let retained = rejected_reuse_state
        .glyph_index
        .known_canonical_glyphs
        .iter()
        .find(|glyph| glyph.content == selected_equal_candidate)
        .expect("reused selected canonical glyph");
    assert_eq!(retained.glyph_id, selected_equal_binding.glyph_id);
    assert!(retained.strongly_retained);

    // A state that claims a known-false prefix was committed but still holds
    // the pre-delta aliased line is rejected without changing the invocation.
    let mut committed_prefix_scheduler = scheduler.clone();
    let other_v = audiveris_omr::native_stems_beam_vlinkers::NativeStemsBeamVLinkerRef {
        b_linker: frontier.v_linker.b_linker,
        side: match frontier.v_linker.side {
            NativeStemVerticalSide::Top => NativeStemVerticalSide::Bottom,
            NativeStemVerticalSide::Bottom => NativeStemVerticalSide::Top,
        },
    };
    let mut prefix_after = attempt.stored_theoretical_line_before;
    prefix_after.start.x += 0.25;
    prefix_after.stop.x += 0.25;
    let prefix = NativeStemsBeamDeferredLineDelta {
        delta_ordinal: 0,
        invocation_ordinal: 0,
        plan: frontier.plan,
        v_linker: other_v,
        before: attempt.stored_theoretical_line_before,
        after: prefix_after,
        builder_line_aliases: true,
        attachment_aliases: true,
    };
    committed_prefix_scheduler
        .deferred_line_deltas
        .push(prefix.clone());
    match &mut committed_prefix_scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(current) => {
            current.invocation_ordinal = current.invocation_ordinal.max(1);
        }
        _ => unreachable!("cloned Chula createStem frontier"),
    }

    // The public result carries the exact ordered known-false prefix that
    // this invocation committed; callers do not have to infer it from generic
    // mutation tags or from the post-state ledger.
    let mut pending_prefix_state = base.clone();
    pending_prefix_state
        .line_states
        .push(NativeStemsBeamVLinkLineState {
            v_linker: other_v,
            stored_theoretical_line: prefix.before,
            builder_line: prefix.before,
            current_attachment_line: Some(prefix.before),
        });
    let applied_prefix = apply_native_stems_beam_vlink_create_stem_transaction(
        &committed_prefix_scheduler,
        builder,
        plans,
        &mut pending_prefix_state,
        &checker,
    )
    .expect("known-false prefix transaction");
    assert_eq!(
        applied_prefix.applied_prefix_line_deltas,
        vec![prefix.clone()]
    );
    assert_eq!(
        pending_prefix_state.applied_line_deltas.first(),
        Some(&NativeStemsBeamAppliedLineDelta {
            system_id: scheduler.system_id,
            source: NativeStemsBeamAppliedLineDeltaSource::KnownFalsePrefix { delta_ordinal: 0 },
            delta: prefix.clone(),
        })
    );
    let committed_line = pending_prefix_state
        .line_states
        .iter()
        .find(|line| line.v_linker == other_v)
        .expect("known-false prefix line state");
    assert_eq!(committed_line.stored_theoretical_line, prefix.after);
    assert_eq!(committed_line.builder_line, prefix.after);
    assert_eq!(committed_line.current_attachment_line, Some(prefix.after));

    let mut committed_prefix_state = base.clone();
    committed_prefix_state
        .line_states
        .push(NativeStemsBeamVLinkLineState {
            v_linker: other_v,
            stored_theoretical_line: prefix.before,
            builder_line: prefix.before,
            current_attachment_line: Some(prefix.before),
        });
    committed_prefix_state
        .applied_line_deltas
        .push(NativeStemsBeamAppliedLineDelta {
            system_id: scheduler.system_id,
            source: NativeStemsBeamAppliedLineDeltaSource::KnownFalsePrefix { delta_ordinal: 0 },
            delta: prefix,
        });
    let claimed_commit = committed_prefix_state.clone();
    let committed_prefix_failure = apply_native_stems_beam_vlink_create_stem_transaction(
        &committed_prefix_scheduler,
        builder,
        plans,
        &mut committed_prefix_state,
        &checker,
    );
    assert!(
        matches!(
            committed_prefix_failure,
            Err(NativeStemsBeamVLinkTransactionError::LineStateMismatch { reference })
                if reference == other_v
        ),
        "unexpected committed-prefix result: {committed_prefix_failure:?}"
    );
    assert_eq!(committed_prefix_state, claimed_commit);
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
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
    let mut padded = bytes.to_vec();
    let bit_len = u64::try_from(bytes.len()).expect("fixture length fits u64") * 8;
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    let mut state = [
        0x6a09e667_u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in padded.chunks_exact(64) {
        let mut words = [0_u32; 64];
        for (index, word) in words[..16].iter_mut().enumerate() {
            let offset = index * 4;
            *word = u32::from_be_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
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
            let sigma1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temporary1 = h
                .wrapping_add(sigma1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let sigma0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temporary2 = sigma0.wrapping_add(majority);
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
    state.iter().map(|word| format!("{word:08x}")).collect()
}
