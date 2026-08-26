// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact gate for head-origin `CLinker` `StemBuilder` construction.
//!
//! The frozen seam runs the real serial page chronology: one system's
//! linker stumps, beam-origin builders, and head-origin builders before the
//! next system starts.  The gate therefore does not reuse the older
//! beam-only registry actions as authoritative.  It replays one page-wide
//! structural registry, preserving registration occurrences, structural
//! glyph keys, and modeled canonical aliases separately.
//!
//! The live native projector renders and hashes every semantic row against
//! eight independently pinned one-page Java fixtures. Only the smaller
//! exploratory Chula diagnostic remains ignored; the corpus differential is
//! an active, fail-closed test.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use audiveris_image::{
    run_table::{Orientation, RunTable},
    section::Bounds,
};
use audiveris_omr::{
    grid_executor::HeadlessStaffLine,
    head_scanner_slices::JavaRectangle,
    native_headers::recognize_native_headers,
    native_heads::{NativeHeadsRecognition, recognize_native_heads},
    native_ledgers::{NativeLedgerRecognition, recognize_native_ledgers},
    native_stem_seeds::{NativeStemSeedRecognition, recognize_native_stem_seeds},
    native_stems_beam_builders::{
        NativeStemsBeamBuilderPreBuilderGlyphSource, NativeStemsBeamBuilderRecognition,
        NativeStemsBeamBuilderRegistrationAction, materialize_native_stems_beam_builders,
    },
    native_stems_beam_reachability::{
        NativeStemsBeamReachabilityRecognition, materialize_native_stems_beam_reachability,
    },
    native_stems_beam_stumps::{
        NativeStemsBeamSource, NativeStemsBeamStumpRecognition, NativeStemsBeamStumpSystem,
        materialize_native_stems_beam_stumps,
    },
    native_stems_beam_vlinkers::{
        NativeStemsBeamBLinkerRef, NativeStemsBeamVLinkerRecognition,
        materialize_native_stems_beam_vlinkers,
    },
    native_stems_head_builders::{
        NativeStemsHeadBuilder, NativeStemsHeadBuilderAlignmentSubject,
        NativeStemsHeadBuilderChunkAction, NativeStemsHeadBuilderGlyphRef,
        NativeStemsHeadBuilderItem, NativeStemsHeadBuilderItemKind,
        NativeStemsHeadBuilderRecognition, NativeStemsHeadBuilderRegistryOccurrence,
        NativeStemsHeadBuilderTargetRef, materialize_native_stems_head_builders,
    },
    native_stems_head_corner_reachability::{
        NativeStemsHeadCornerReachabilityRecognition, NativeStemsHeadCornerRef,
        NativeStemsHeadReachabilityCorner, NativeStemsHeadReachabilityTarget,
        NativeStemsHeadSeedOverlapAction, materialize_native_stems_head_corner_reachability,
    },
    native_stems_head_corners::{
        NativeStemsHeadCornerRecognition, materialize_native_stems_head_corners,
    },
    native_stems_head_seeds::{
        NativeStemsHeadSeedRecognition, materialize_native_stems_head_seeds,
    },
    native_stems_head_stumps::{
        NativeStemsHeadStumpRecognition, materialize_native_stems_head_stumps,
    },
    recognize::{
        GridLinesRecognition, NativeBeamRecognition, recognize_grid_lines,
        recognize_native_beams_with_stem_seeds,
    },
    stems_step::{NativeStemHeadSide, NativeStemLine, NativeStemPoint, NativeStemVerticalSide},
};

#[path = "native_stems_head_builders/dense_rows.rs"]
mod dense_rows;
#[path = "native_stems_head_builders/registry_rows.rs"]
mod registry_rows;
#[path = "native_stems_head_builders/sparse_rows.rs"]
mod sparse_rows;

const SCHEMA: usize = 1;
const MANIFEST_PATH: &str = "rust/oracle/stems-head-stem-builders-manifest.txt";
const PROBE_PATH: &str = "rust/oracle/java/StemsHeadStemBuilderProbe.java";
const RUNNER_PATH: &str = "rust/oracle/java/run-stems-head-stem-builders.sh";

const EXPECTED_MANIFEST_SHA256: Option<&str> =
    Some("31db6d63abc6c7e38152a9aac4a73f690717bfb814bf974825f2189a5a383480");
const EXPECTED_PROBE_SHA256: Option<&str> =
    Some("ab657f96502869a4b710bcc98e507cb7539f1492635f9d097cff012b9629a9bb");
const EXPECTED_RUNNER_SHA256: Option<&str> =
    Some("8542d682241d7be645f3bfc1e474a6acbc430f15a6e4c5ee92f25221a20d751b");

const PREDECESSOR_HEADS: usize = 3_521;
const PREDECESSOR_SYSTEMS: usize = 30;
const PREDECESSOR_C_BUILDERS: usize = 14_084;
const PREDECESSOR_C_TOP: usize = 7_042;
const PREDECESSOR_C_BOTTOM: usize = 7_042;
const PREDECESSOR_INPUT_SEEDS: usize = 1_340;
const PREDECESSOR_INPUT_C_TARGETS: usize = 4_566;
const PREDECESSOR_INPUT_B_TARGETS: usize = 8_120;
const PREDECESSOR_HEAD_ANCHORS: usize = 1_687;
const PREDECESSOR_INITIAL_BS: usize = 2_116;
const PREDECESSOR_BEAM_ANCHORS: usize = 145;
const PREDECESSOR_FINAL_BS: usize = 3_948;
const PREDECESSOR_BEAM_BUILDERS: usize = 2_417;
const PREDECESSOR_BEAM_REGISTRATIONS: usize = 1_442;
const PREDECESSOR_START_STUMPS: usize = 4_940;
const PREDECESSOR_STUMPLESS_STARTS: usize = 9_144;
const PREDECESSOR_LENGTH_ROWS: usize = 70_420;
/// The runner uses the unmodified Audiveris stub profile on every frozen page.
/// This is also the explicit argument the live native projector will pass.
const CORPUS_INSPECT_PROFILE: i32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PageSpec {
    ordinal: usize,
    page: &'static str,
    fixture: &'static str,
}

const PAGES: &[PageSpec] = &[
    PageSpec {
        ordinal: 0,
        page: "chula.png#1",
        fixture: "stems-head-stem-builders-chula.txt",
    },
    PageSpec {
        ordinal: 1,
        page: "allegretto.png#1",
        fixture: "stems-head-stem-builders-allegretto.txt",
    },
    PageSpec {
        ordinal: 2,
        page: "batuque.png#1",
        fixture: "stems-head-stem-builders-batuque.txt",
    },
    PageSpec {
        ordinal: 3,
        page: "carmen.png#1",
        fixture: "stems-head-stem-builders-carmen.txt",
    },
    PageSpec {
        ordinal: 4,
        page: "cucaracha.png#1",
        fixture: "stems-head-stem-builders-cucaracha.txt",
    },
    PageSpec {
        ordinal: 5,
        page: "hove.png#1",
        fixture: "stems-head-stem-builders-hove.txt",
    },
    PageSpec {
        ordinal: 6,
        page: "zizi.png#1",
        fixture: "stems-head-stem-builders-zizi.txt",
    },
    PageSpec {
        ordinal: 7,
        page: "BachInvention5.jpg#1",
        fixture: "stems-head-stem-builders-BachInvention5.txt",
    },
];

const REQUIRED_FAMILIES: &[&str] = &[
    "stemsheadbuilderpage",
    "stemsheadbuilderoriginbaseline",
    "stemsheadbuilderregistryboundary",
    "stemsheadbuilderstumpreg",
    "stemsheadbuildersystem",
    "stemsheadbuilderseedduplicates",
    "stemsheadbuilderbeamreg",
    "stemsheadbuilderbeamchron",
    "stemsheadbuilderhead",
    "stemsheadbuilder",
    "stemsheadbuilderseedsource",
    "stemsheadbuilderseedsort",
    "stemsheadbuilderseedoverlap",
    "stemsheadbuilderinputseed",
    "stemsheadbuilderinputtarget",
    "stemsheadbuildervsection",
    "stemsheadbuilderhsection",
    "stemsheadbuilderfilamentmember",
    "stemsheadbuilderfilament",
    "stemsheadbuilderglyphreg",
    "stemsheadbuilderchunkduplicates",
    "stemsheadbuilderalign",
    "stemsheadbuilderalignresult",
    "stemsheadbuildertargetfilter",
    "stemsheadbuilderlasthead",
    "stemsheadbuilderseedfilter",
    "stemsheadbuilderheadparts",
    "stemsheadbuilderchunkfilter",
    "stemsheadbuilderseeditemfilter",
    "stemsheadbuildersortaudit",
    "stemsheadbuildersort",
    "stemsheadbuilderitempre",
    "stemsheadbuildergap",
    "stemsheadbuilderitem",
    "stemsheadbuilderlength",
    "stemsheadbuilderend",
    "stemsheadbuildersystemsummary",
    "stemsheadbuilderpagesummary",
    "stemsheadbuildercheckpoint",
];

const MAX_FIELDS: &[&str] = &[
    "maxSortItems",
    "maxTargetSortItems",
    "maxFinalSortItems",
    "maxRetrieveSeedSortItems",
];

const SUMMARY_FIELDS: &[&str] = &[
    "stumpRegistrations",
    "stumpGlyphNew",
    "stumpGlyphReuse",
    "stumpActionDiffs",
    "beamBuilders",
    "beamFilaments",
    "beamGlyphNew",
    "beamGlyphReuse",
    "builders",
    "topBuilders",
    "bottomBuilders",
    "directionDivergences",
    "profileDivergences",
    "stumplessStarts",
    "anchorsCreated",
    "vScans",
    "vAccepts",
    "hScans",
    "hAccepts",
    "filaments",
    "filamentMembers",
    "glyphNew",
    "glyphReuse",
    "lowRemainChunks",
    "headPartDrops",
    "items",
    "gaps",
    "lengths",
    "sortCycles",
    "sortEquivalence",
    "inputTargets",
    "removedTargets",
    "inputSeeds",
    "alignComparisons",
    "alignRemovals",
    "chunkDrops",
    "seedItemDrops",
    "preItems",
    "sortRows",
    "maxTargetSortItems",
    "maxFinalSortItems",
    "targetSortListsAtLeast32",
    "finalSortListsAtLeast32",
    "gapChecks",
    "gapInserts",
    "gapTruncations",
    "seedSourceScans",
    "retrieveSeedSortRows",
    "maxRetrieveSeedSortItems",
    "retrieveSeedSortListsAtLeast32",
    "vipHeads",
    "vipBuilders",
    "smallHeads",
    "removeVipOnly",
    "keepNonVipBug",
    "beamActionDiffs",
    "headToLaterBeamReuses",
    "seedDuplicateKeys",
    "seedDuplicateExtraOccurrences",
    "chunkDuplicateKeys",
    "chunkDuplicateExtraOccurrences",
    "sortAudits",
    "sigMutations",
    "systemStemMutations",
    "linkMutations",
    "unexpectedBuilderMutations",
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct PagePin {
    ordinal: usize,
    page: String,
    fixture: String,
    fixture_sha256: String,
    body_sha256: String,
    fixture_lines: usize,
    fixture_bytes: usize,
    body_lines: usize,
    body_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Manifest {
    probe_sha256: String,
    runner_sha256: String,
    pages: Vec<PagePin>,
    corpus: BTreeMap<String, usize>,
}

impl Manifest {
    fn parse(text: &str) -> Self {
        let mut header = None;
        let mut pages = Vec::new();
        let mut corpus = None;
        for (line_number, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let family = line
                .split_whitespace()
                .next()
                .expect("nonempty manifest row");
            match family {
                "stemsheadbuildermanifest" => {
                    assert!(header.is_none(), "duplicate manifest header");
                    assert_exact_field_schema_after(
                        line,
                        1,
                        &["schema", "pages", "probeSourceSha256", "runnerSourceSha256"],
                    );
                    assert_eq!(required_usize(line, "schema"), SCHEMA);
                    assert_eq!(required_usize(line, "pages"), PAGES.len());
                    header = Some((
                        required_value(line, "probeSourceSha256").to_owned(),
                        required_value(line, "runnerSourceSha256").to_owned(),
                    ));
                }
                "stemsheadbuildermanifestpage" => {
                    assert_exact_field_schema_after(
                        line,
                        1,
                        &[
                            "ordinal",
                            "page",
                            "fixture",
                            "fixtureSha256",
                            "bodySha256",
                            "fixtureLines",
                            "fixtureBytes",
                            "bodyLines",
                            "bodyBytes",
                        ],
                    );
                    pages.push(PagePin {
                        ordinal: required_usize(line, "ordinal"),
                        page: required_value(line, "page").to_owned(),
                        fixture: required_value(line, "fixture").to_owned(),
                        fixture_sha256: required_value(line, "fixtureSha256").to_owned(),
                        body_sha256: required_value(line, "bodySha256").to_owned(),
                        fixture_lines: required_usize(line, "fixtureLines"),
                        fixture_bytes: required_usize(line, "fixtureBytes"),
                        body_lines: required_usize(line, "bodyLines"),
                        body_bytes: required_usize(line, "bodyBytes"),
                    });
                }
                "stemsheadbuildermanifestcorpus" => {
                    assert!(corpus.is_none(), "duplicate manifest corpus row");
                    let mut fields = vec!["systems"];
                    fields.extend(SUMMARY_FIELDS.iter().copied());
                    assert_exact_field_schema_after(line, 1, &fields);
                    corpus = Some(numeric_fields(line, 1));
                }
                _ => panic!("unknown manifest row {line_number}: {line}"),
            }
        }
        let (probe_sha256, runner_sha256) = header.expect("missing manifest header");
        pages.sort_by_key(|page| page.ordinal);
        assert_eq!(pages.len(), PAGES.len(), "manifest page count");
        for (actual, expected) in pages.iter().zip(PAGES) {
            assert_eq!(actual.ordinal, expected.ordinal);
            assert_eq!(actual.page, expected.page);
            assert_eq!(actual.fixture, expected.fixture);
            assert_sha256(&actual.fixture_sha256, "fixture pin");
            assert_sha256(&actual.body_sha256, "body pin");
            assert!(actual.fixture_bytes < 100_000_000, "fixture size guard");
            assert!(actual.body_bytes < 100_000_000, "body size guard");
            assert!(actual.fixture_lines > actual.body_lines);
            assert!(actual.fixture_bytes > actual.body_bytes);
        }
        assert_sha256(&probe_sha256, "probe source pin");
        assert_sha256(&runner_sha256, "runner source pin");
        Self {
            probe_sha256,
            runner_sha256,
            pages,
            corpus: corpus.expect("missing manifest corpus row"),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CorpusTotals(BTreeMap<String, usize>);

impl CorpusTotals {
    fn include_page(&mut self, page: &BTreeMap<String, usize>) {
        for (field, &value) in page {
            if MAX_FIELDS.contains(&field.as_str()) {
                self.0
                    .entry(field.clone())
                    .and_modify(|current| *current = (*current).max(value))
                    .or_insert(value);
            } else {
                *self.0.entry(field.clone()).or_default() += value;
            }
        }
    }

    fn get(&self, field: &str) -> usize {
        *self
            .0
            .get(field)
            .unwrap_or_else(|| panic!("missing corpus field {field}"))
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct RowCounts {
    by_family: BTreeMap<String, usize>,
    stump_registrations: usize,
    stump_registration_new: usize,
    stump_registration_reuse: usize,
    stump_action_diffs: usize,
    heads: usize,
    c_builders: usize,
    c_top: usize,
    c_bottom: usize,
    stumpless_starts: usize,
    input_seeds: usize,
    input_c_targets: usize,
    input_b_targets: usize,
    beam_registrations: usize,
    head_registrations: usize,
    head_registration_new: usize,
    head_registration_reuse: usize,
    head_anchors: usize,
    length_rows: usize,
    vip_heads: usize,
    small_heads: usize,
    keep_non_vip_bug: usize,
    remove_vip_only: usize,
    beam_action_diffs: usize,
    head_to_later_beam_reuses: usize,
    profile_divergences: usize,
    builder_top: usize,
    builder_bottom: usize,
    direction_divergences: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RegistryCursor {
    count: usize,
    sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FixtureAudit {
    page: String,
    body_sha256: String,
    body_lines: usize,
    body_bytes: usize,
    checkpoint: BTreeMap<String, String>,
    page_summary: BTreeMap<String, usize>,
    counts: RowCounts,
}

/// Owned live pipeline needed by the final row projector.  Keeping every
/// predecessor here makes identity normalization derive from native products,
/// never from an oracle row.
#[allow(dead_code)]
struct NativeProjectionPage {
    grid: GridLinesRecognition,
    stem_seeds: NativeStemSeedRecognition,
    beams: NativeBeamRecognition,
    ledgers: NativeLedgerRecognition,
    heads: NativeHeadsRecognition,
    corners: NativeStemsHeadCornerRecognition,
    head_seeds: NativeStemsHeadSeedRecognition,
    beam_stumps: NativeStemsBeamStumpRecognition,
    beam_vlinkers: NativeStemsBeamVLinkerRecognition,
    beam_reachability: NativeStemsBeamReachabilityRecognition,
    head_stumps: NativeStemsHeadStumpRecognition,
    beam_builders: NativeStemsBeamBuilderRecognition,
    head_reachability: NativeStemsHeadCornerReachabilityRecognition,
    head_builders: NativeStemsHeadBuilderRecognition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BoundedGlyph {
    bounds: BoundedBounds,
    run_table: RunTable,
}

/// The Java/native glyph pipeline uses signed sheet coordinates.  Do not use
/// `audiveris_image::section::Bounds` here: it is an unsigned section-local
/// rectangle and would turn this independent identity projection into a
/// lossy/invalid conversion at the product boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BoundedBounds {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BoundedOriginEntry {
    glyph: BoundedGlyph,
    categories: BTreeSet<&'static str>,
}

/// Independent mirror of the deliberately bounded native registry.  Baseline
/// aliases are first-occurrence ordinals, never Java `GlyphIndex` ids.  The
/// history stream is retained as bytes so the gate recomputes every SHA rather
/// than trusting a production digest.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct BoundedOriginRegistry {
    entries: Vec<BoundedOriginEntry>,
    history: Vec<u8>,
    sealed: bool,
    baseline_count: usize,
}

impl BoundedOriginRegistry {
    fn from_pre_stems(native: &NativeProjectionPage) -> Self {
        let mut value = Self::default();
        for staff in &native.grid.peak_graph.sheet_staffs {
            for line in &staff.lines {
                let HeadlessStaffLine::Persistent { line, .. } = line else {
                    continue;
                };
                value.add(
                    BoundedGlyph {
                        bounds: BoundedBounds {
                            x: i32::try_from(line.glyph.x).expect("staff glyph x"),
                            y: i32::try_from(line.glyph.y).expect("staff glyph y"),
                            width: i32::try_from(line.glyph.runs.width())
                                .expect("staff glyph width"),
                            height: i32::try_from(line.glyph.runs.height())
                                .expect("staff glyph height"),
                        },
                        run_table: line.glyph.runs.clone(),
                    },
                    "grid",
                );
            }
        }
        for system in &native.stem_seeds.systems {
            for glyph in &system.registered_glyphs {
                value.add(
                    BoundedGlyph {
                        bounds: BoundedBounds {
                            x: i32::try_from(glyph.bounds.x).expect("stem-seed glyph x"),
                            y: i32::try_from(glyph.bounds.y).expect("stem-seed glyph y"),
                            width: i32::try_from(glyph.bounds.width)
                                .expect("stem-seed glyph width"),
                            height: i32::try_from(glyph.bounds.height)
                                .expect("stem-seed glyph height"),
                        },
                        run_table: glyph.run_table.clone(),
                    },
                    "checkedStemSeedCandidate",
                );
            }
        }
        for (raw_ordinal, (_, glyph)) in native.beams.raw_beam_glyphs.iter().enumerate() {
            if native
                .beams
                .multiple_rests
                .iter()
                .any(|rest| rest.source_beam_ordinal == raw_ordinal)
            {
                continue;
            }
            value.add(
                BoundedGlyph {
                    bounds: BoundedBounds {
                        x: glyph.bounds.x,
                        y: glyph.bounds.y,
                        width: glyph.bounds.width,
                        height: glyph.bounds.height,
                    },
                    run_table: glyph.run_table.clone(),
                },
                "rawBeam",
            );
        }
        for (_, glyph) in &native.beams.hook_glyphs {
            value.add(
                BoundedGlyph {
                    bounds: BoundedBounds {
                        x: glyph.bounds.x,
                        y: glyph.bounds.y,
                        width: glyph.bounds.width,
                        height: glyph.bounds.height,
                    },
                    run_table: glyph.run_table.clone(),
                },
                "rawBeam",
            );
        }
        for glyph in &native.ledgers.ledger_glyphs {
            value.add(
                BoundedGlyph {
                    bounds: BoundedBounds {
                        x: i32::try_from(glyph.bounds.x).expect("ledger glyph x"),
                        y: i32::try_from(glyph.bounds.y).expect("ledger glyph y"),
                        width: i32::try_from(glyph.bounds.width).expect("ledger glyph width"),
                        height: i32::try_from(glyph.bounds.height).expect("ledger glyph height"),
                    },
                    run_table: glyph.run_table.clone(),
                },
                "ledger",
            );
        }
        for system in &native.heads.seed_glyphs.systems {
            for staff in &system.staffs {
                for head in &staff.heads {
                    value.add(retrieved_head_glyph(&head.glyph), "head");
                }
            }
        }
        for system in &native.heads.range_glyphs.systems {
            for staff in &system.staffs {
                for head in &staff.heads {
                    value.add(retrieved_head_glyph(&head.glyph), "head");
                }
            }
        }
        value.seal_baseline();
        value
    }

    fn add(&mut self, glyph: BoundedGlyph, category: &'static str) -> usize {
        assert!(!self.sealed, "baseline registry cannot grow after sealing");
        let ordinal = self
            .entries
            .iter()
            .position(|entry| entry.glyph == glyph)
            .unwrap_or_else(|| {
                let ordinal = self.entries.len();
                self.entries.push(BoundedOriginEntry {
                    glyph,
                    categories: BTreeSet::new(),
                });
                ordinal
            });
        self.entries[ordinal].categories.insert(category);
        ordinal
    }

    fn seal_baseline(&mut self) {
        assert!(!self.sealed, "bounded registry sealed twice");
        self.entries
            .sort_by_key(|entry| bounded_glyph_alias(&entry.glyph));
        self.history.clear();
        for entry in &self.entries {
            self.history
                .extend_from_slice(format!("{}\n", bounded_glyph_alias(&entry.glyph)).as_bytes());
        }
        self.baseline_count = self.entries.len();
        self.sealed = true;
    }

    fn count(&self) -> usize {
        self.entries.len()
    }

    fn history_sha256(&self) -> String {
        sha256_hex(&self.history)
    }

    fn categories(&self, glyph: &BoundedGlyph) -> String {
        self.entries
            .iter()
            .find(|entry| entry.glyph == *glyph)
            .map(|entry| {
                entry
                    .categories
                    .iter()
                    .copied()
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .filter(|categories| !categories.is_empty())
            .unwrap_or_else(|| "-".to_owned())
    }

    /// Match the oracle's bounded `OriginRegistry.register`: the history
    /// records one source-scoped registration attempt, while provenance labels
    /// themselves deliberately do not mutate that chronology digest.
    fn register(
        &mut self,
        glyph: BoundedGlyph,
        category: &'static str,
        source: &str,
    ) -> BoundedRegistration {
        assert!(self.sealed, "unsealed modeled registry");
        let prior = self.categories(&glyph);
        let reuse = prior != "-";
        let ordinal =
            if let Some(ordinal) = self.entries.iter().position(|entry| entry.glyph == glyph) {
                ordinal
            } else {
                let ordinal = self.entries.len();
                self.entries.push(BoundedOriginEntry {
                    glyph,
                    categories: BTreeSet::new(),
                });
                ordinal
            };
        self.entries[ordinal].categories.insert(category);
        self.history.extend_from_slice(
            format!(
                "{source}:{}:{}\n",
                if reuse { "Reuse" } else { "New" },
                bounded_glyph_alias(&self.entries[ordinal].glyph),
            )
            .as_bytes(),
        );
        BoundedRegistration {
            reuse,
            prior_categories: prior,
            canonical_alias: self.canonical_alias(&self.entries[ordinal].glyph),
            canonical_ordinal: ordinal,
        }
    }

    fn label(&mut self, glyph: &BoundedGlyph, category: &'static str) {
        assert!(self.sealed, "unsealed modeled registry");
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.glyph == *glyph)
            .expect("provenance cannot invent a registration");
        entry.categories.insert(category);
    }

    fn canonical_alias(&self, glyph: &BoundedGlyph) -> String {
        let ordinal = self
            .entries
            .iter()
            .position(|entry| entry.glyph == *glyph)
            .expect("modeled canonical glyph");
        if ordinal < self.baseline_count {
            format!("baseline:{ordinal}")
        } else {
            format!("registered:{}", ordinal - self.baseline_count)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BoundedRegistration {
    reuse: bool,
    prior_categories: String,
    canonical_alias: String,
    canonical_ordinal: usize,
}

fn retrieved_head_glyph(
    glyph: &audiveris_omr::head_glyph_retrieval::RetrievedHeadGlyph,
) -> BoundedGlyph {
    BoundedGlyph {
        bounds: BoundedBounds {
            x: glyph.glyph_bounds.x,
            y: glyph.glyph_bounds.y,
            width: glyph.glyph_bounds.width,
            height: glyph.glyph_bounds.height,
        },
        run_table: glyph.run_table.clone(),
    }
}

fn bounded_glyph_alias(glyph: &BoundedGlyph) -> String {
    format!(
        "g:{}:{}:{}:{}:{}",
        glyph.bounds.x,
        glyph.bounds.y,
        glyph.bounds.width,
        glyph.bounds.height,
        glyph_run_sha256(&glyph.run_table),
    )
}

fn glyph_run_sha256(table: &RunTable) -> String {
    let orientation = match table.orientation() {
        audiveris_image::run_table::Orientation::Horizontal => "HORIZONTAL",
        audiveris_image::run_table::Orientation::Vertical => "VERTICAL",
    };
    let mut bytes = format!("{orientation} {} {}\n", table.width(), table.height()).into_bytes();
    for sequence in 0..table.sequence_count() {
        let mut row = sequence.to_string();
        for run in table.sequence(sequence).unwrap_or_default() {
            row.push_str(&format!(" {}:{}", run.start, run.length));
        }
        row.push('\n');
        bytes.extend_from_slice(row.as_bytes());
    }
    sha256_hex(&bytes)
}

fn glyph_run_count(table: &RunTable) -> usize {
    (0..table.sequence_count())
        .map(|sequence| table.sequence(sequence).unwrap_or_default().len())
        .sum()
}

fn orientation_token(orientation: Orientation) -> &'static str {
    match orientation {
        Orientation::Horizontal => "HORIZONTAL",
        Orientation::Vertical => "VERTICAL",
    }
}

fn bounds_token(bounds: Bounds) -> String {
    format!(
        "{}:{}:{}:{}",
        bounds.x, bounds.y, bounds.width, bounds.height
    )
}

fn rectangle_token(rectangle: JavaRectangle) -> String {
    format!(
        "{}:{}:{}:{}",
        rectangle.x, rectangle.y, rectangle.width, rectangle.height
    )
}

fn java_hex_double(value: f64) -> String {
    let raw_bits = value.to_bits();
    let canonical_bits = if value.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        raw_bits
    };
    let java = if value.is_nan() {
        "NaN".to_owned()
    } else if value == f64::INFINITY {
        "Infinity".to_owned()
    } else if value == f64::NEG_INFINITY {
        "-Infinity".to_owned()
    } else {
        let sign = if (raw_bits >> 63) != 0 { "-" } else { "" };
        let exponent_bits = ((raw_bits >> 52) & 0x7ff) as i32;
        let fraction_bits = raw_bits & 0x000f_ffff_ffff_ffff;
        if exponent_bits == 0 && fraction_bits == 0 {
            format!("{sign}0x0.0p0")
        } else {
            let mut fraction = format!("{fraction_bits:013x}");
            while fraction.len() > 1 && fraction.ends_with('0') {
                fraction.pop();
            }
            if exponent_bits == 0 {
                format!("{sign}0x0.{fraction}p-1022")
            } else {
                format!("{sign}0x1.{fraction}p{}", exponent_bits - 1023)
            }
        }
    };
    format!("{java}/{canonical_bits:016x}")
}

fn point_token(point: NativeStemPoint) -> String {
    format!("{}:{}", java_hex_double(point.x), java_hex_double(point.y))
}

fn line_token(line: NativeStemLine) -> String {
    format!("{}:{}", point_token(line.start), point_token(line.stop))
}

fn string_list(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_owned()
    } else {
        values.join(",")
    }
}

fn optional_double_token(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_owned(), java_hex_double)
}

fn native_c_alias(reference: NativeStemsHeadCornerRef) -> String {
    format!(
        "head:{}:{}:{}",
        reference.x_ordinal,
        head_side_token(reference.horizontal),
        vertical_side_token(reference.vertical),
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

fn b_alias(stumps: &NativeStemsBeamStumpSystem, reference: NativeStemsBeamBLinkerRef) -> String {
    format!(
        "beam:{}:b:{}",
        beam_sig_ordinal(stumps, reference.beam),
        reference.id.checked_sub(1).expect("one-based B linker id"),
    )
}

fn beam_sig_ordinal(stumps: &NativeStemsBeamStumpSystem, source: NativeStemsBeamSource) -> usize {
    stumps
        .beams_by_abscissa
        .iter()
        .find(|beam| beam.source == source)
        .expect("beam source")
        .sig_ordinal
}

#[allow(dead_code)]
fn native_projection_page(image: &str) -> NativeProjectionPage {
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

    // The calls are deliberately ordered like Java's serial STEMS chronology.
    // Head reachability does not consume the builder product, but the final C
    // constructor does consume both products and replays the interleaved page
    // registry rather than trusting isolated beam registration actions.
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
        CORPUS_INSPECT_PROFILE,
    )
    .unwrap_or_else(|error| panic!("{image}: STEMS head builders failed: {error}"));

    NativeProjectionPage {
        grid,
        stem_seeds,
        beams,
        ledgers,
        heads,
        corners,
        head_seeds,
        beam_stumps,
        beam_vlinkers,
        beam_reachability,
        head_stumps,
        beam_builders,
        head_reachability,
        head_builders,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RowHasher(u64);

impl Default for RowHasher {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl RowHasher {
    fn add(&mut self, row: &str) {
        for byte in row.bytes().chain([b'\n']) {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

impl FixtureAudit {
    fn parse(bytes: &[u8], expected_page: &str) -> Self {
        let text = std::str::from_utf8(bytes).expect("fixture UTF-8");
        let checkpoint_start = text
            .rfind("stemsheadbuildercheckpoint ")
            .expect("missing final checkpoint row");
        assert!(
            text[checkpoint_start..].lines().count() == 1,
            "checkpoint must be the final fixture row"
        );
        let body = &text[..checkpoint_start];
        let checkpoint_row = text[checkpoint_start..].trim_end_matches(['\r', '\n']);
        let checkpoint = string_fields(checkpoint_row, 1);
        assert_eq!(required_usize(checkpoint_row, "schema"), SCHEMA);
        assert_eq!(required_value(checkpoint_row, "page"), expected_page);
        assert_eq!(
            required_value(checkpoint_row, "bodySha256"),
            sha256_hex(body.as_bytes())
        );
        assert_eq!(
            required_usize(checkpoint_row, "bodyLines"),
            body.lines().count()
        );
        assert_eq!(required_usize(checkpoint_row, "bodyBytes"), body.len());

        let mut counts = RowCounts::default();
        let mut seen_families = BTreeSet::new();
        let mut page_summary = None;
        let mut page_rows = 0usize;
        let mut registry = None::<RegistryCursor>;
        let mut beam_actions = Vec::<String>::new();
        let mut head_actions = BTreeMap::<(usize, usize), Vec<String>>::new();
        let mut sort_phases = BTreeMap::<(usize, usize), BTreeSet<String>>::new();
        let mut glyph_events = BTreeSet::<(usize, usize, String)>::new();
        let mut chunk_events = BTreeSet::<(usize, usize, String)>::new();
        let mut page_hasher = RowHasher::default();
        let mut system_hasher = None::<(usize, RowHasher)>;
        let mut page_algebra = BTreeMap::<String, usize>::new();
        let mut system_algebra = None::<(usize, BTreeMap<String, usize>)>;

        for (line_number, row) in body.lines().enumerate() {
            let row = row.trim_end_matches('\r');
            if row.is_empty() || row.starts_with('#') {
                continue;
            }
            let family = row.split_whitespace().next().expect("semantic row");
            assert!(
                REQUIRED_FAMILIES.contains(&family),
                "unknown semantic row {line_number}: {row}"
            );
            assert_ne!(
                family, "stemsheadbuildercheckpoint",
                "checkpoint inside body"
            );
            validate_row_schema(family, row);
            seen_families.insert(family.to_owned());
            *counts.by_family.entry(family.to_owned()).or_default() += 1;

            match family {
                "stemsheadbuilderpage" => {}
                "stemsheadbuilderoriginbaseline" => page_hasher.add(row),
                "stemsheadbuildersystemsummary" => {
                    let system = required_usize(row, "system");
                    let (hashed_system, hasher) = system_hasher.take().expect("system hash start");
                    assert_eq!(system, hashed_system, "system hash order");
                    assert_eq!(
                        required_value(row, "hash"),
                        format!("{:016x}", hasher.0),
                        "system semantic row hash"
                    );
                    let (algebra_system, algebra) =
                        system_algebra.take().expect("system algebra start");
                    assert_eq!(system, algebra_system, "system algebra order");
                    assert_summary_algebra(row, 2, &algebra);
                    include_summary_algebra(&mut page_algebra, &algebra);
                    bump(&mut page_algebra, "systems", 1);
                    page_hasher.add(row);
                }
                "stemsheadbuilderpagesummary" => {
                    assert!(system_hasher.is_none(), "open system at page summary");
                    assert!(
                        system_algebra.is_none(),
                        "open system algebra at page summary"
                    );
                    assert_eq!(
                        required_value(row, "hash"),
                        format!("{:016x}", page_hasher.0),
                        "page semantic row hash"
                    );
                    assert_eq!(
                        required_usize(row, "systems"),
                        *page_algebra.get("systems").unwrap_or(&0),
                        "page system count algebra"
                    );
                    assert_summary_algebra(row, 2, &page_algebra);
                }
                _ => {
                    let system = required_usize(row, "system");
                    let entry = system_hasher.get_or_insert((system, RowHasher::default()));
                    assert_eq!(entry.0, system);
                    entry.1.add(row);
                    page_hasher.add(row);
                    let algebra = system_algebra
                        .get_or_insert_with(|| (system, BTreeMap::<String, usize>::new()));
                    assert_eq!(algebra.0, system, "system algebra row order");
                    update_summary_algebra(family, row, &mut algebra.1);
                }
            }

            match family {
                "stemsheadbuilderpage" => {
                    page_rows += 1;
                    assert_eq!(row.split_whitespace().nth(1), Some(expected_page));
                }
                "stemsheadbuilderoriginbaseline" => {
                    assert!(registry.is_none(), "duplicate origin baseline");
                    registry = Some(registry_point(row, "modeledRegistry", "registrySha256"));
                    let raw = required_usize(row, "rawSeedCandidates");
                    let checked = required_usize(row, "checkedSeedCandidates");
                    let unique = required_usize(row, "checkedSeedStructuralKeys");
                    assert!(raw >= checked, "checked candidates exceed raw candidates");
                    assert!(
                        checked >= unique,
                        "structural keys exceed checked candidates"
                    );
                    assert!(
                        required_usize(row, "modeledRegistry") >= unique,
                        "modeled registry omits checked STEM_SEEDS candidates"
                    );
                }
                "stemsheadbuilderregistryboundary" => {
                    let before = registry_point(row, "modeledBefore", "shaBefore");
                    if let Some(prior) = &registry {
                        assert_eq!(&before, prior, "registry reset at system boundary");
                    }
                    registry = Some(registry_point(row, "modeledAfter", "shaAfter"));
                }
                "stemsheadbuildersystem" => {
                    let system_profile = required_i32(row, "profile");
                    let inspect_profile = required_i32(row, "inspectProfile");
                    assert_eq!(inspect_profile, CORPUS_INSPECT_PROFILE);
                    if system_profile != inspect_profile {
                        counts.profile_divergences += 1;
                    }
                }
                "stemsheadbuilderstumpreg" => {
                    counts.stump_registrations += 1;
                    let result = required_value(row, "result");
                    let origins = required_value(row, "preOriginCategories");
                    validate_registration(result, origins, row);
                    match result {
                        "New" => counts.stump_registration_new += 1,
                        "Reuse" => counts.stump_registration_reuse += 1,
                        _ => unreachable!(),
                    }
                    let action_diff = required_bool(row, "actionDiff");
                    assert_eq!(
                        action_diff,
                        required_value(row, "beamOnlyResult") != result,
                        "stump isolated registration action"
                    );
                    counts.stump_action_diffs += usize::from(action_diff);
                }
                "stemsheadbuilderbeamreg" => {
                    counts.beam_registrations += 1;
                    let result = required_value(row, "result");
                    let canonical = required_value(row, "canonical");
                    let origins = required_value(row, "preOriginCategories");
                    validate_registration(result, origins, row);
                    let beam_only_result = required_value(row, "beamOnlyResult");
                    let beam_only_origins = required_value(row, "beamOnlyPreOriginCategories");
                    validate_registration(beam_only_result, beam_only_origins, row);
                    beam_actions.push(format!("{result}:{canonical}:{origins}\n"));
                    let action_diff = required_bool(row, "actionDiff");
                    let head_reuse = required_bool(row, "headToLaterBeamReuse");
                    assert_eq!(action_diff, result != beam_only_result);
                    if action_diff {
                        assert_eq!(result, "Reuse");
                        assert_eq!(beam_only_result, "New");
                        counts.beam_action_diffs += 1;
                    }
                    if head_reuse {
                        assert!(action_diff, "head-to-beam reuse without action diff");
                        assert!(origins.split(',').any(|value| value == "headBuilderChunk"));
                        counts.head_to_later_beam_reuses += 1;
                    }
                }
                "stemsheadbuilderbeamchron" => {
                    validate_registry_transition(row, &mut registry);
                    assert_eq!(
                        required_value(row, "registrationActionSha256"),
                        sha256_hex(beam_actions.concat().as_bytes()),
                        "beam registration action digest"
                    );
                    assert_eq!(required_usize(row, "filaments"), beam_actions.len());
                    beam_actions.clear();
                }
                "stemsheadbuilderhead" => {
                    counts.heads += 1;
                    let vip = required_bool(row, "vip");
                    if vip {
                        counts.vip_heads += 1;
                    }
                    let shape = required_value(row, "shape");
                    assert!(
                        matches!(shape, "NOTEHEAD_BLACK" | "NOTEHEAD_VOID"),
                        "unsupported head shape {shape}"
                    );
                    if shape.contains("SMALL") {
                        counts.small_heads += 1;
                    }
                    required_value(row, "glyph");
                }
                "stemsheadbuilder" => {
                    counts.c_builders += 1;
                    let c_y_direction = required_i32(row, "cYDir");
                    match required_value(row, "vSide") {
                        "TOP" => {
                            counts.c_top += 1;
                            assert_eq!(c_y_direction, -1);
                        }
                        "BOTTOM" => {
                            counts.c_bottom += 1;
                            assert_eq!(c_y_direction, 1);
                        }
                        side => panic!("unknown C vertical side {side}"),
                    }
                    let builder_y_direction = required_i32(row, "builderYDir");
                    match builder_y_direction {
                        -1 => counts.builder_top += 1,
                        1 => counts.builder_bottom += 1,
                        direction => panic!("invalid builder direction {direction}: {row}"),
                    }
                    let diverges = required_bool(row, "directionDiverges");
                    assert_eq!(diverges, c_y_direction != builder_y_direction);
                    if diverges {
                        counts.direction_divergences += 1;
                    }
                    if required_value(row, "startStump") == "-" {
                        counts.stumpless_starts += 1;
                    }
                }
                "stemsheadbuilderinputseed" => counts.input_seeds += 1,
                "stemsheadbuilderinputtarget" => match required_value(row, "kind") {
                    "C" => counts.input_c_targets += 1,
                    "B" => counts.input_b_targets += 1,
                    kind => panic!("unknown target kind {kind}"),
                },
                "stemsheadbuilderglyphreg" => {
                    counts.head_registrations += 1;
                    let system = required_usize(row, "system");
                    let builder = required_usize(row, "builder");
                    let ordinal = required_usize(row, "ordinal");
                    let event = optional_value(row, "event")
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("chunkEvent:s{system}:c{builder}:{ordinal}"));
                    assert!(
                        glyph_events.insert((system, builder, event)),
                        "duplicate registration occurrence"
                    );
                    let result = required_value(row, "result");
                    let canonical = required_value(row, "canonical");
                    let origins = required_value(row, "originCategories");
                    validate_registration(result, origins, row);
                    match result {
                        "New" => counts.head_registration_new += 1,
                        "Reuse" => counts.head_registration_reuse += 1,
                        _ => unreachable!(),
                    }
                    head_actions
                        .entry((system, builder))
                        .or_default()
                        .push(format!("{result}:{canonical}:{origins}\n"));
                }
                "stemsheadbuilderseedduplicates" | "stemsheadbuilderchunkduplicates" => {
                    assert_sha256(required_value(row, "duplicateSha256"), "duplicate digest");
                }
                "stemsheadbuilderalign" => validate_alignment_occurrences(row),
                "stemsheadbuilderheadparts" => match required_value(row, "action") {
                    "keepNonVipBug" => {
                        assert!(!required_bool(row, "vip"));
                        counts.keep_non_vip_bug += 1;
                    }
                    "removeVipOnly" => {
                        assert!(required_bool(row, "vip"));
                        counts.remove_vip_only += 1;
                    }
                    "keep" => {}
                    action => panic!("unknown head-parts action {action}"),
                },
                "stemsheadbuilderchunkfilter" => {
                    let key = (
                        required_usize(row, "system"),
                        required_usize(row, "builder"),
                        required_value(row, "event").to_owned(),
                    );
                    assert!(chunk_events.insert(key), "duplicate chunk occurrence");
                }
                "stemsheadbuildersortaudit" => {
                    let key = (
                        required_usize(row, "system"),
                        required_usize(row, "builder"),
                    );
                    let phase = required_value(row, "phase").to_owned();
                    assert!(
                        matches!(phase.as_str(), "retrieveSeeds" | "targets" | "items"),
                        "unknown sort phase {phase}"
                    );
                    assert!(sort_phases.entry(key).or_default().insert(phase));
                    assert!(required_usize(row, "items") < 32);
                    assert!(required_bool(row, "jdk25MiniTimSort"));
                    assert_sha256(
                        required_value(row, "offenderSha256"),
                        "sort offender digest",
                    );
                }
                "stemsheadbuildervsection" | "stemsheadbuilderhsection" => {
                    assert_sha256(required_value(row, "rejectSha256"), "section reject digest");
                }
                "stemsheadbuilderlength" => counts.length_rows += 1,
                "stemsheadbuilderend" => {
                    validate_registry_transition(row, &mut registry);
                    let key = (
                        required_usize(row, "system"),
                        required_usize(row, "builder"),
                    );
                    let actions = head_actions.remove(&key).unwrap_or_default();
                    assert_eq!(
                        required_value(row, "registrationActionSha256"),
                        sha256_hex(actions.concat().as_bytes()),
                        "head registration action digest"
                    );
                    assert_eq!(required_usize(row, "filaments"), actions.len());
                    counts.head_anchors += required_usize(row, "anchorsCreated");
                    assert!(required_bool(row, "sbAssigned"));
                    for field in [
                        "sigVertexDelta",
                        "sigEdgeDelta",
                        "stemInterDelta",
                        "systemStemDelta",
                        "linkMutations",
                    ] {
                        assert_eq!(required_usize(row, field), 0, "forbidden mutation {field}");
                    }
                }
                "stemsheadbuilderpagesummary" => {
                    assert!(page_summary.is_none(), "duplicate page summary");
                    page_summary = Some(numeric_fields(row, 2));
                }
                _ => {}
            }
        }

        assert_eq!(page_rows, 1, "page row cardinality");
        assert!(system_hasher.is_none(), "unclosed system hash");
        assert!(beam_actions.is_empty(), "unclosed beam registration batch");
        assert!(head_actions.is_empty(), "unclosed head registration batch");
        assert_eq!(glyph_events, chunk_events, "registration/chunk occurrences");
        for family in REQUIRED_FAMILIES {
            if *family != "stemsheadbuildercheckpoint" {
                assert!(
                    seen_families.contains(*family),
                    "missing row family {family}"
                );
            }
        }
        assert_eq!(sort_phases.len(), counts.c_builders);
        for phases in sort_phases.values() {
            assert_eq!(
                phases,
                &BTreeSet::from([
                    "retrieveSeeds".to_owned(),
                    "targets".to_owned(),
                    "items".to_owned(),
                ]),
                "incomplete three-sort evidence"
            );
        }

        Self {
            page: expected_page.to_owned(),
            body_sha256: sha256_hex(body.as_bytes()),
            body_lines: body.lines().count(),
            body_bytes: body.len(),
            checkpoint,
            page_summary: page_summary.expect("missing page summary"),
            counts,
        }
    }
}

#[test]
fn head_builder_manifest_parser_is_fail_closed() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let fixture_hash = "0".repeat(64);
    let body_hash = "1".repeat(64);
    let mut rows = vec![format!(
        "stemsheadbuildermanifest schema 1 pages 8 probeSourceSha256 {} \
         runnerSourceSha256 {}",
        "2".repeat(64),
        "3".repeat(64)
    )];
    for page in PAGES {
        rows.push(format!(
            "stemsheadbuildermanifestpage ordinal {} page {} fixture {} fixtureSha256 {} \
             bodySha256 {} fixtureLines 2 fixtureBytes 2 bodyLines 1 bodyBytes 1",
            page.ordinal, page.page, page.fixture, fixture_hash, body_hash
        ));
    }
    let mut corpus = vec![
        "stemsheadbuildermanifestcorpus".to_owned(),
        "systems".to_owned(),
        "30".to_owned(),
    ];
    for field in SUMMARY_FIELDS {
        corpus.push((*field).to_owned());
        corpus.push(if *field == "builders" {
            PREDECESSOR_C_BUILDERS.to_string()
        } else {
            "0".to_owned()
        });
    }
    rows.push(corpus.join(" "));
    let manifest = Manifest::parse(&rows.join("\n"));
    assert_eq!(manifest.pages.len(), 8);
    assert_eq!(manifest.corpus["builders"], PREDECESSOR_C_BUILDERS);

    let bad = rows.join("\n").replace("schema 1", "schema 2");
    assert!(std::panic::catch_unwind(|| Manifest::parse(&bad)).is_err());
}

#[test]
#[ignore = "developer-only live Chula checkpoint"]
fn exploratory_native_head_builder_materialization() {
    let native = native_projection_page("chula.png");
    assert_eq!(native.head_builders.builder_count, 1_304);
    let mut registry = BoundedOriginRegistry::from_pre_stems(&native);
    assert!(registry.count() > 0, "bounded registry baseline");
    assert_eq!(registry.history_sha256().len(), 64, "baseline history hash");
    let first = registry.entries[0].glyph.clone();
    assert_eq!(registry.canonical_alias(&first), "baseline:0");
    assert_ne!(registry.categories(&first), "-");
    let registration = registry.register(first.clone(), "checkpoint", "test:0");
    assert!(registration.reuse);
    assert_eq!(registration.canonical_alias, "baseline:0");
    assert_ne!(registration.prior_categories, "-");
    registry.label(&first, "checkpointLabel");
}

#[test]
fn native_head_stem_builders_match_frozen_java() {
    let root = repo_root();
    let manifest_path = root.join(MANIFEST_PATH);
    let manifest_bytes = fs::read(&manifest_path).expect("frozen head-builder manifest");
    assert_eq!(
        sha256_hex(&manifest_bytes),
        frozen_pin(EXPECTED_MANIFEST_SHA256, "manifest SHA-256")
    );
    let manifest =
        Manifest::parse(std::str::from_utf8(&manifest_bytes).expect("manifest must be UTF-8"));
    assert_eq!(
        sha256_hex(&fs::read(root.join(PROBE_PATH)).expect("head-builder probe source")),
        frozen_pin(EXPECTED_PROBE_SHA256, "probe SHA-256")
    );
    assert_eq!(
        manifest.probe_sha256,
        frozen_pin(EXPECTED_PROBE_SHA256, "probe SHA-256")
    );
    assert_eq!(
        sha256_hex(&fs::read(root.join(RUNNER_PATH)).expect("head-builder runner source")),
        frozen_pin(EXPECTED_RUNNER_SHA256, "runner SHA-256")
    );
    assert_eq!(
        manifest.runner_sha256,
        frozen_pin(EXPECTED_RUNNER_SHA256, "runner SHA-256")
    );

    let mut totals = CorpusTotals::default();
    let mut row_counts = RowCounts::default();
    for (pin, expected) in manifest.pages.iter().zip(PAGES) {
        let fixture_path = root.join("rust/oracle").join(&pin.fixture);
        let bytes = fs::read(&fixture_path).expect("frozen one-page head-builder fixture");
        assert_eq!(bytes.len(), pin.fixture_bytes);
        assert_eq!(line_count(&bytes), pin.fixture_lines);
        assert_eq!(sha256_hex(&bytes), pin.fixture_sha256);
        let audit = FixtureAudit::parse(&bytes, expected.page);
        assert_eq!(audit.body_sha256, pin.body_sha256);
        assert_eq!(audit.body_lines, pin.body_lines);
        assert_eq!(audit.body_bytes, pin.body_bytes);
        assert_eq!(
            required_value_map(&audit.checkpoint, "probeSourceSha256"),
            manifest.probe_sha256
        );
        assert_eq!(
            required_value_map(&audit.checkpoint, "runnerSourceSha256"),
            manifest.runner_sha256
        );
        let image = expected
            .page
            .strip_suffix("#1")
            .expect("one-page head-builder label");
        let native = native_projection_page(image);
        let registry = registry_rows::validate_registry_projection(expected.page, &bytes, &native);
        let sparse =
            sparse_rows::validate_sparse_projection(expected.page, &bytes, &native, &registry);
        let projected = projected_body_rows(expected.page, &native, &registry, &sparse);
        validate_projected_body(expected.page, &bytes, &projected);
        totals.include_page(&audit.page_summary);
        include_row_counts(&mut row_counts, &audit.counts);
    }
    assert_eq!(totals.0, manifest.corpus, "manifest corpus summary");
    validate_predecessor_algebra(&totals, &row_counts);
}

fn projected_body_rows(
    page: &str,
    native: &NativeProjectionPage,
    registry: &registry_rows::RegistryProjection,
    sparse: &sparse_rows::SparseProjection,
) -> Vec<String> {
    let mut rows = vec![sparse.page.clone(), registry.baseline.clone()];
    let mut page_hasher = RowHasher::default();
    page_hasher.add(&registry.baseline);
    let mut page_algebra = BTreeMap::<String, usize>::new();

    for system in &native.head_builders.systems {
        let system_id = system.system_id;
        let beam_system = native
            .beam_builders
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system_id)
            .expect("projected beam-builder system");
        let reach_system = native
            .head_reachability
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system_id)
            .expect("projected head reachability system");
        let registry_system = &registry.systems[&system_id];
        let sparse_system = &sparse.systems[&system_id];
        let mut system_rows = Vec::<String>::new();
        system_rows.push(sparse_system.seed_duplicates.clone());
        system_rows.extend(registry_system.stump_rows.iter().cloned());
        system_rows.push(registry_system.boundary.clone());
        system_rows.push(sparse_system.system.clone());

        for builder in &beam_system.builders {
            system_rows.extend(
                registry_system
                    .beam_rows
                    .get(&builder.builder_ordinal)
                    .into_iter()
                    .flatten()
                    .cloned(),
            );
            system_rows.push(registry_system.beam_chron_rows[&builder.builder_ordinal].clone());
        }

        let mut projected_builder_count = 0usize;
        for head in &reach_system.heads {
            system_rows.push(sparse_system.heads[&head.x_ordinal].clone());
            let head_builders = system
                .builders
                .iter()
                .filter(|builder| builder.start.x_ordinal == head.x_ordinal)
                .collect::<Vec<_>>();
            assert_eq!(
                head_builders.len(),
                4,
                "{page}: system {system_id} head {} constructor coverage",
                head.x_ordinal
            );
            for builder in head_builders {
                let glyph_rows = registry_system
                    .glyph_rows
                    .get(&builder.builder_ordinal)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                let dense =
                    dense_rows::dense_builder_rows(page, native, system, reach_system, builder);
                let mut inserted_glyph_rows = 0usize;
                for row in dense {
                    let family = row.split_whitespace().next().expect("dense row family");
                    if family == "stemsheadbuilderfilament" {
                        let ordinal = required_usize(&row, "ordinal");
                        assert_eq!(ordinal, inserted_glyph_rows, "{page}: filament order");
                        system_rows.push(row);
                        system_rows.push(
                            glyph_rows
                                .get(ordinal)
                                .unwrap_or_else(|| {
                                    panic!(
                                        "{page}: missing glyph registration for system {system_id} builder {} filament {ordinal}",
                                        builder.builder_ordinal
                                    )
                                })
                                .clone(),
                        );
                        inserted_glyph_rows += 1;
                    } else {
                        system_rows.push(row);
                    }
                }
                assert_eq!(
                    inserted_glyph_rows,
                    glyph_rows.len(),
                    "{page}: system {system_id} builder {} glyph-registration coverage",
                    builder.builder_ordinal
                );
                system_rows.push(sparse_system.ends[&builder.builder_ordinal].clone());
                projected_builder_count += 1;
            }
        }
        assert_eq!(
            projected_builder_count,
            system.builders.len(),
            "{page}: system {system_id} builder chronology coverage"
        );

        let mut system_hasher = RowHasher::default();
        let mut system_algebra = BTreeMap::<String, usize>::new();
        for row in &system_rows {
            system_hasher.add(row);
            page_hasher.add(row);
            let family = row.split_whitespace().next().expect("projected row family");
            update_summary_algebra(family, row, &mut system_algebra);
        }
        bump(
            &mut system_algebra,
            "vipBuilders",
            system
                .builders
                .iter()
                .filter(|builder| builder.source_is_vip)
                .count(),
        );
        bump(
            &mut system_algebra,
            "unexpectedBuilderMutations",
            system
                .builders
                .iter()
                .map(|builder| builder.beam_arena_mutation_count)
                .sum(),
        );
        rows.extend(system_rows);
        let summary =
            projected_summary_row(page, Some(system_id), &system_algebra, system_hasher.0);
        page_hasher.add(&summary);
        rows.push(summary);
        include_summary_algebra(&mut page_algebra, &system_algebra);
        bump(&mut page_algebra, "systems", 1);
    }
    rows.push(projected_summary_row(
        page,
        None,
        &page_algebra,
        page_hasher.0,
    ));
    rows
}

fn projected_summary_row(
    page: &str,
    system: Option<usize>,
    algebra: &BTreeMap<String, usize>,
    hash: u64,
) -> String {
    let mut row = match system {
        Some(system) => format!("stemsheadbuildersystemsummary {page} system {system}"),
        None => format!(
            "stemsheadbuilderpagesummary {page} systems {}",
            algebra.get("systems").copied().unwrap_or(0)
        ),
    };
    for field in SUMMARY_FIELDS {
        row.push_str(&format!(
            " {field} {}",
            algebra.get(*field).copied().unwrap_or(0)
        ));
    }
    row.push_str(&format!(" hash {hash:016x}"));
    row
}

fn validate_projected_body(page: &str, bytes: &[u8], projected: &[String]) {
    let expected = std::str::from_utf8(bytes)
        .expect("head-builder projected fixture UTF-8")
        .lines()
        .filter(|row| !row.is_empty() && !row.starts_with('#'))
        .take_while(|row| !row.starts_with("stemsheadbuildercheckpoint "))
        .collect::<Vec<_>>();
    assert_eq!(
        projected.len(),
        expected.len(),
        "{page}: projected semantic body row count"
    );
    for (ordinal, (actual, expected)) in projected.iter().zip(expected).enumerate() {
        assert_eq!(actual, expected, "{page}: projected semantic row {ordinal}");
    }
}

fn validate_predecessor_algebra(totals: &CorpusTotals, rows: &RowCounts) {
    let family = |name: &str| {
        *rows
            .by_family
            .get(name)
            .unwrap_or_else(|| panic!("missing corpus row family {name}"))
    };
    assert_eq!(family("stemsheadbuilderpage"), PAGES.len());
    assert_eq!(family("stemsheadbuilderoriginbaseline"), PAGES.len());
    assert_eq!(family("stemsheadbuilderstumpreg"), rows.stump_registrations);
    assert_eq!(family("stemsheadbuildersystem"), PREDECESSOR_SYSTEMS);
    assert_eq!(
        family("stemsheadbuilderregistryboundary"),
        PREDECESSOR_SYSTEMS
    );
    assert_eq!(
        family("stemsheadbuilderseedduplicates"),
        PREDECESSOR_SYSTEMS
    );
    assert_eq!(rows.heads, PREDECESSOR_HEADS);
    assert_eq!(family("stemsheadbuilderhead"), rows.heads);
    assert_eq!(rows.c_builders, PREDECESSOR_C_BUILDERS);
    assert_eq!(family("stemsheadbuilder"), rows.c_builders);
    assert_eq!(family("stemsheadbuilderend"), rows.c_builders);
    assert_eq!(family("stemsheadbuilderchunkduplicates"), rows.c_builders);
    assert_eq!(family("stemsheadbuilderalignresult"), 2 * rows.c_builders);
    assert_eq!(family("stemsheadbuildersortaudit"), 3 * rows.c_builders);
    assert_eq!(family("stemsheadbuilderlasthead"), rows.c_builders);
    assert_eq!(family("stemsheadbuildervsection"), rows.c_builders);
    assert_eq!(family("stemsheadbuilderhsection"), rows.c_builders);
    assert_eq!(rows.c_builders, 4 * rows.heads);
    assert_eq!(rows.c_top, PREDECESSOR_C_TOP);
    assert_eq!(rows.c_bottom, PREDECESSOR_C_BOTTOM);
    assert_eq!(rows.c_top + rows.c_bottom, rows.c_builders);
    assert_eq!(
        rows.stump_registration_new + rows.stump_registration_reuse,
        rows.stump_registrations
    );
    assert_eq!(rows.profile_divergences, 0, "inspect profile scope");
    assert_eq!(rows.builder_top + rows.builder_bottom, rows.c_builders);
    assert_eq!(rows.input_seeds, PREDECESSOR_INPUT_SEEDS);
    assert_eq!(rows.input_c_targets, PREDECESSOR_INPUT_C_TARGETS);
    assert_eq!(rows.input_b_targets, PREDECESSOR_INPUT_B_TARGETS);
    assert_eq!(rows.beam_registrations, PREDECESSOR_BEAM_REGISTRATIONS);
    assert_eq!(
        family("stemsheadbuilderbeamchron"),
        PREDECESSOR_BEAM_BUILDERS
    );
    assert_eq!(rows.stumpless_starts, PREDECESSOR_STUMPLESS_STARTS);
    assert_eq!(
        PREDECESSOR_START_STUMPS + rows.stumpless_starts,
        rows.c_builders
    );
    assert_eq!(rows.head_anchors, PREDECESSOR_HEAD_ANCHORS);
    assert_eq!(
        PREDECESSOR_INITIAL_BS + PREDECESSOR_BEAM_ANCHORS + rows.head_anchors,
        PREDECESSOR_FINAL_BS
    );
    assert_eq!(rows.length_rows, PREDECESSOR_LENGTH_ROWS);
    assert_eq!(family("stemsheadbuilderlength"), rows.length_rows);
    assert_eq!(rows.length_rows, 5 * rows.c_builders);
    assert_eq!(rows.vip_heads, 0, "corpus VIP certificate");
    assert_eq!(rows.small_heads, 0, "small-head scope certificate");
    assert_eq!(
        rows.remove_vip_only, 0,
        "no VIP-only drop without VIP input"
    );
    assert!(
        rows.keep_non_vip_bug > 0,
        "VIP-only Java behavior must stay live"
    );
    assert_eq!(
        rows.head_registration_new + rows.head_registration_reuse,
        rows.head_registrations
    );
    assert_eq!(family("stemsheadbuilderglyphreg"), rows.head_registrations);
    assert_eq!(family("stemsheadbuilderfilament"), rows.head_registrations);
    assert_eq!(
        family("stemsheadbuilderchunkfilter"),
        rows.head_registrations
    );
    assert!(rows.head_to_later_beam_reuses <= rows.beam_action_diffs);

    assert_eq!(totals.get("beamBuilders"), PREDECESSOR_BEAM_BUILDERS);
    assert_eq!(totals.get("stumpRegistrations"), rows.stump_registrations);
    assert_eq!(totals.get("stumpGlyphNew"), rows.stump_registration_new);
    assert_eq!(totals.get("stumpGlyphReuse"), rows.stump_registration_reuse);
    assert_eq!(totals.get("stumpActionDiffs"), rows.stump_action_diffs);
    assert_eq!(totals.get("systems"), PREDECESSOR_SYSTEMS);
    assert_eq!(totals.get("builders"), PREDECESSOR_C_BUILDERS);
    assert_eq!(totals.get("profileDivergences"), 0);
    assert_eq!(totals.get("topBuilders"), rows.builder_top);
    assert_eq!(totals.get("bottomBuilders"), rows.builder_bottom);
    assert_eq!(
        totals.get("directionDivergences"),
        rows.direction_divergences
    );
    assert_eq!(totals.get("inputSeeds"), PREDECESSOR_INPUT_SEEDS);
    assert_eq!(
        totals.get("inputTargets"),
        PREDECESSOR_INPUT_C_TARGETS + PREDECESSOR_INPUT_B_TARGETS
    );
    assert_eq!(totals.get("anchorsCreated"), PREDECESSOR_HEAD_ANCHORS);
    assert_eq!(totals.get("lengths"), PREDECESSOR_LENGTH_ROWS);
    assert_eq!(totals.get("vipHeads"), 0);
    assert_eq!(totals.get("vipBuilders"), 0);
    assert_eq!(totals.get("smallHeads"), 0);
    assert_eq!(totals.get("removeVipOnly"), 0);
    assert_eq!(totals.get("keepNonVipBug"), rows.keep_non_vip_bug);
    assert_eq!(totals.get("glyphNew"), rows.head_registration_new);
    assert_eq!(totals.get("glyphReuse"), rows.head_registration_reuse);
    assert_eq!(totals.get("filaments"), rows.head_registrations);
    assert_eq!(totals.get("beamActionDiffs"), rows.beam_action_diffs);
    assert_eq!(
        totals.get("headToLaterBeamReuses"),
        rows.head_to_later_beam_reuses
    );
    assert_eq!(totals.get("sortAudits"), 3 * PREDECESSOR_C_BUILDERS);
    for field in [
        "chunkDuplicateKeys",
        "chunkDuplicateExtraOccurrences",
        "seedDuplicateKeys",
        "seedDuplicateExtraOccurrences",
    ] {
        let _ = totals.get(field);
    }
    assert_eq!(totals.get("retrieveSeedSortListsAtLeast32"), 0);
    assert_eq!(totals.get("targetSortListsAtLeast32"), 0);
    assert_eq!(totals.get("finalSortListsAtLeast32"), 0);
    assert_eq!(totals.get("sigMutations"), 0);
    assert_eq!(totals.get("systemStemMutations"), 0);
    assert_eq!(totals.get("linkMutations"), 0);
    assert_eq!(totals.get("unexpectedBuilderMutations"), 0);
}

fn validate_alignment_occurrences(row: &str) {
    for field in [
        "firstOccurrence",
        "secondOccurrence",
        "selectedAlienOccurrence",
        "actualRemovedOccurrence",
    ] {
        required_value(row, field);
    }
}

fn bump(values: &mut BTreeMap<String, usize>, field: &str, amount: usize) {
    *values.entry(field.to_owned()).or_default() += amount;
}

fn maximize(values: &mut BTreeMap<String, usize>, field: &str, value: usize) {
    values
        .entry(field.to_owned())
        .and_modify(|current| *current = (*current).max(value))
        .or_insert(value);
}

fn include_summary_algebra(page: &mut BTreeMap<String, usize>, system: &BTreeMap<String, usize>) {
    for field in SUMMARY_FIELDS {
        let value = *system.get(*field).unwrap_or(&0);
        if MAX_FIELDS.contains(field) {
            maximize(page, field, value);
        } else {
            bump(page, field, value);
        }
    }
}

fn assert_summary_algebra(row: &str, skip: usize, derived: &BTreeMap<String, usize>) {
    let reported = numeric_fields(row, skip);
    for field in SUMMARY_FIELDS {
        assert_eq!(
            reported
                .get(*field)
                .copied()
                .unwrap_or_else(|| panic!("missing summary field {field}: {row}")),
            *derived.get(*field).unwrap_or(&0),
            "summary algebra for {field}"
        );
    }
}

fn update_summary_algebra(family: &str, row: &str, values: &mut BTreeMap<String, usize>) {
    match family {
        "stemsheadbuildersystem" => {
            bump(
                values,
                "profileDivergences",
                usize::from(required_i32(row, "profile") != required_i32(row, "inspectProfile")),
            );
        }
        "stemsheadbuilderstumpreg" => {
            bump(values, "stumpRegistrations", 1);
            match required_value(row, "result") {
                "New" => bump(values, "stumpGlyphNew", 1),
                "Reuse" => bump(values, "stumpGlyphReuse", 1),
                action => panic!("unknown stump registration {action}: {row}"),
            }
            bump(
                values,
                "stumpActionDiffs",
                usize::from(required_bool(row, "actionDiff")),
            );
        }
        "stemsheadbuilderbeamreg" => {
            bump(values, "beamFilaments", 1);
            match required_value(row, "result") {
                "New" => bump(values, "beamGlyphNew", 1),
                "Reuse" => bump(values, "beamGlyphReuse", 1),
                action => panic!("unknown beam registration {action}: {row}"),
            }
            bump(
                values,
                "beamActionDiffs",
                usize::from(required_bool(row, "actionDiff")),
            );
            bump(
                values,
                "headToLaterBeamReuses",
                usize::from(required_bool(row, "headToLaterBeamReuse")),
            );
        }
        "stemsheadbuilderbeamchron" => bump(values, "beamBuilders", 1),
        "stemsheadbuilderhead" => {
            bump(values, "vipHeads", usize::from(required_bool(row, "vip")));
            bump(
                values,
                "smallHeads",
                usize::from(required_value(row, "shape").contains("SMALL")),
            );
        }
        "stemsheadbuilder" => {
            bump(values, "builders", 1);
            let c_direction = required_i32(row, "cYDir");
            let builder_direction = required_i32(row, "builderYDir");
            bump(
                values,
                if builder_direction < 0 {
                    "topBuilders"
                } else {
                    "bottomBuilders"
                },
                1,
            );
            bump(
                values,
                "directionDivergences",
                usize::from(c_direction != builder_direction),
            );
            bump(
                values,
                "stumplessStarts",
                usize::from(required_value(row, "startStump") == "-"),
            );
        }
        "stemsheadbuildervsection" => {
            bump(values, "vScans", required_usize(row, "sourceCount"));
            bump(values, "vAccepts", required_usize(row, "acceptedCount"));
        }
        "stemsheadbuilderhsection" => {
            bump(values, "hScans", required_usize(row, "sourceCount"));
            bump(values, "hAccepts", required_usize(row, "acceptedCount"));
        }
        "stemsheadbuilderfilamentmember" => bump(values, "filamentMembers", 1),
        "stemsheadbuilderglyphreg" => {
            bump(values, "filaments", 1);
            match required_value(row, "result") {
                "New" => bump(values, "glyphNew", 1),
                "Reuse" => bump(values, "glyphReuse", 1),
                action => panic!("unknown head registration {action}: {row}"),
            }
        }
        "stemsheadbuilderheadparts" => {
            bump(
                values,
                "lowRemainChunks",
                usize::from(required_bool(row, "lowRemain")),
            );
            match required_value(row, "action") {
                "removeVipOnly" => {
                    bump(values, "headPartDrops", 1);
                    bump(values, "removeVipOnly", 1);
                }
                "keepNonVipBug" => bump(values, "keepNonVipBug", 1),
                "keep" => {}
                action => panic!("unknown head-parts action {action}: {row}"),
            }
        }
        "stemsheadbuilderitem" => {
            bump(values, "items", 1);
            bump(
                values,
                "gaps",
                usize::from(required_value(row, "kind") == "gap"),
            );
        }
        "stemsheadbuilderlength" => bump(values, "lengths", 1),
        "stemsheadbuildersortaudit" => {
            bump(values, "sortAudits", 1);
            bump(values, "sortCycles", required_usize(row, "strictCycles"));
            bump(
                values,
                "sortEquivalence",
                required_usize(row, "equivalenceInconsistencies"),
            );
            let items = required_usize(row, "items");
            match required_value(row, "phase") {
                "retrieveSeeds" => {
                    maximize(values, "maxRetrieveSeedSortItems", items);
                    bump(
                        values,
                        "retrieveSeedSortListsAtLeast32",
                        usize::from(items >= 32),
                    );
                }
                "targets" => {
                    maximize(values, "maxTargetSortItems", items);
                    bump(values, "targetSortListsAtLeast32", usize::from(items >= 32));
                }
                "items" => {
                    maximize(values, "maxFinalSortItems", items);
                    bump(values, "finalSortListsAtLeast32", usize::from(items >= 32));
                }
                phase => panic!("unknown summary sort phase {phase}: {row}"),
            }
        }
        "stemsheadbuildertargetfilter" => {
            bump(values, "inputTargets", 1);
            bump(
                values,
                "removedTargets",
                usize::from(required_value(row, "action") == "remove"),
            );
        }
        "stemsheadbuilderseedfilter" => bump(values, "inputSeeds", 1),
        "stemsheadbuilderalign" => {
            bump(values, "alignComparisons", 1);
            bump(
                values,
                "alignRemovals",
                usize::from(!required_bool(row, "aligned")),
            );
        }
        "stemsheadbuilderchunkfilter" => bump(
            values,
            "chunkDrops",
            usize::from(required_value(row, "action") != "keep"),
        ),
        "stemsheadbuilderseeditemfilter" => bump(
            values,
            "seedItemDrops",
            usize::from(required_value(row, "action") != "keep"),
        ),
        "stemsheadbuilderitempre" => bump(values, "preItems", 1),
        "stemsheadbuildersort" => bump(values, "sortRows", 1),
        "stemsheadbuildergap" => {
            bump(values, "gapChecks", 1);
            match required_value(row, "action") {
                "insert" => bump(values, "gapInserts", 1),
                "truncate" => bump(values, "gapTruncations", 1),
                "initial" | "contiguous" => {}
                action => panic!("unknown gap action {action}: {row}"),
            }
        }
        "stemsheadbuilderseedsource" => bump(values, "seedSourceScans", 1),
        "stemsheadbuilderseedsort" => bump(values, "retrieveSeedSortRows", 1),
        "stemsheadbuilderseedduplicates" => {
            bump(
                values,
                "seedDuplicateKeys",
                required_usize(row, "duplicateStructuralKeys"),
            );
            bump(
                values,
                "seedDuplicateExtraOccurrences",
                required_usize(row, "extraOccurrences"),
            );
        }
        "stemsheadbuilderchunkduplicates" => {
            bump(
                values,
                "chunkDuplicateKeys",
                required_usize(row, "duplicateStructuralKeys"),
            );
            bump(
                values,
                "chunkDuplicateExtraOccurrences",
                required_usize(row, "extraOccurrences"),
            );
        }
        "stemsheadbuilderend" => {
            bump(
                values,
                "anchorsCreated",
                required_usize(row, "anchorsCreated"),
            );
            bump(
                values,
                "sigMutations",
                required_usize(row, "sigVertexDelta")
                    + required_usize(row, "sigEdgeDelta")
                    + required_usize(row, "stemInterDelta"),
            );
            bump(
                values,
                "systemStemMutations",
                required_usize(row, "systemStemDelta"),
            );
            bump(
                values,
                "linkMutations",
                required_usize(row, "linkMutations"),
            );
        }
        "stemsheadbuilderregistryboundary"
        | "stemsheadbuilderinputseed"
        | "stemsheadbuilderinputtarget"
        | "stemsheadbuilderfilament"
        | "stemsheadbuilderalignresult"
        | "stemsheadbuilderlasthead"
        | "stemsheadbuilderseedoverlap" => {}
        family => panic!("summary algebra missing row family {family}"),
    }
}

fn validate_row_schema(family: &str, row: &str) {
    if matches!(
        family,
        "stemsheadbuildersystemsummary" | "stemsheadbuilderpagesummary"
    ) {
        let mut fields = vec![if family == "stemsheadbuildersystemsummary" {
            "system"
        } else {
            "systems"
        }];
        fields.extend(SUMMARY_FIELDS.iter().copied());
        fields.push("hash");
        assert_exact_field_schema(row, &fields);
        return;
    }
    let fields: &[&str] = match family {
        "stemsheadbuilderpage" => &["systems", "staves", "family"],
        "stemsheadbuilderoriginbaseline" => &[
            "rawSeedCandidates",
            "checkedSeedCandidates",
            "checkedSeedStructuralKeys",
            "modeledRegistry",
            "registrySha256",
        ],
        "stemsheadbuilderregistryboundary" => &[
            "system",
            "phase",
            "modeledBefore",
            "modeledAfter",
            "shaBefore",
            "shaAfter",
        ],
        "stemsheadbuildersystem" => &[
            "system",
            "profile",
            "inspectProfile",
            "interline",
            "bounds",
            "beamSigOrder",
            "beamXOrder",
            "headSigOrder",
            "headXOrder",
            "seeds",
            "modeledRegistry",
            "maxStemThickness",
            "maxLineSectionDx",
            "maxStemAlignmentDx",
            "maxStemAlignmentDy",
            "minCoreSectionLength",
            "minSideRatio",
            "gap0",
            "gap1",
            "gap2",
            "gap3",
            "gap4",
        ],
        "stemsheadbuilderstumpreg" => &[
            "system",
            "event",
            "kind",
            "source",
            "result",
            "beamOnlyResult",
            "actionDiff",
            "canonicalAlias",
            "canonical",
            "preOriginCategories",
            "attachment",
            "bounds",
            "orientation",
            "weight",
            "runs",
            "sha256",
        ],
        "stemsheadbuilderseedduplicates" => &[
            "system",
            "keptSeeds",
            "duplicateStructuralKeys",
            "extraOccurrences",
            "duplicateSha256",
        ],
        "stemsheadbuilderbeamreg" => &[
            "system",
            "builder",
            "ordinal",
            "result",
            "canonicalAlias",
            "canonical",
            "preOriginCategories",
            "beamOnlyResult",
            "beamOnlyPreOriginCategories",
            "actionDiff",
            "headToLaterBeamReuse",
            "bounds",
            "orientation",
            "weight",
            "runs",
            "sha256",
        ],
        "stemsheadbuilderbeamchron" => &[
            "system",
            "ordinal",
            "beamSig",
            "bAlias",
            "vSide",
            "profile",
            "filaments",
            "glyphNew",
            "glyphReuse",
            "bArenaBefore",
            "bArenaAfter",
            "modeledRegistryBefore",
            "modeledRegistryAfter",
            "registryShaBefore",
            "registryShaAfter",
            "registrationActionSha256",
        ],
        "stemsheadbuilderhead" => &[
            "system",
            "xOrdinal",
            "sigOrdinal",
            "staff",
            "shape",
            "bounds",
            "center",
            "grade",
            "vip",
            "glyph",
            "glyphWeight",
            "glyphRuns",
        ],
        "stemsheadbuilder" => &[
            "system",
            "builder",
            "head",
            "cornerOrdinal",
            "corner",
            "alias",
            "hSide",
            "vSide",
            "profile",
            "cYDir",
            "builderYDir",
            "directionDiverges",
            "ref",
            "theo",
            "luBounds",
            "startStump",
            "modeledRegistryBefore",
            "bArenaBefore",
        ],
        "stemsheadbuilderseedsource" => &[
            "system",
            "builder",
            "sourceOrdinal",
            "systemSeedOrdinal",
            "glyph",
            "bounds",
            "contrib",
            "minContrib",
            "distance",
            "maxDistance",
            "action",
        ],
        "stemsheadbuilderseedsort" => &[
            "system",
            "builder",
            "input",
            "output",
            "sourceOrdinal",
            "systemSeedOrdinal",
            "glyph",
            "contrib",
        ],
        "stemsheadbuilderseedoverlap" => &[
            "system",
            "builder",
            "sortedOrdinal",
            "glyph",
            "conflict",
            "action",
        ],
        "stemsheadbuilderinputseed" => &["system", "builder", "ordinal", "glyph", "bounds"],
        "stemsheadbuilderinputtarget" => match required_value(row, "kind") {
            "C" => &["system", "builder", "ordinal", "kind", "alias", "action"],
            "B" => &[
                "system",
                "builder",
                "ordinal",
                "kind",
                "alias",
                "action",
                "beforeArena",
                "best",
                "bestDx",
                "cross",
            ],
            kind => panic!("unknown input-target kind {kind}: {row}"),
        },
        "stemsheadbuildervsection" | "stemsheadbuilderhsection" => &[
            "system",
            "builder",
            "sourceCount",
            "acceptedCount",
            "acceptedSourceOrdinals",
            "reasons",
            "rejectSha256",
        ],
        "stemsheadbuilderfilamentmember" => &[
            "system",
            "builder",
            "filament",
            "ordinal",
            "alias",
            "orientation",
            "bounds",
            "weight",
            "runs",
        ],
        "stemsheadbuilderfilament" => &[
            "system",
            "builder",
            "ordinal",
            "members",
            "bounds",
            "weight",
            "centerLine",
            "meanThickness",
            "meanDistance",
            "length",
        ],
        "stemsheadbuilderglyphreg" => &[
            "system",
            "builder",
            "ordinal",
            "result",
            "canonicalAlias",
            "canonical",
            "originCategories",
            "bounds",
            "orientation",
            "weight",
            "runs",
            "sha256",
        ],
        "stemsheadbuilderchunkduplicates" => &[
            "system",
            "builder",
            "attempts",
            "duplicateStructuralKeys",
            "extraOccurrences",
            "duplicateSha256",
        ],
        "stemsheadbuilderalign" => &[
            "system",
            "builder",
            "phase",
            "ordinal",
            "startStump",
            "promotedOccurrence",
            "firstOccurrence",
            "secondOccurrence",
            "first",
            "second",
            "firstDeskew",
            "secondDeskew",
            "dy",
            "maxDy",
            "dyBypass",
            "dx",
            "maxDx",
            "aligned",
            "selectedAlienOccurrence",
            "actualRemovedOccurrence",
            "alien",
            "tieRemoveSecond",
        ],
        "stemsheadbuilderalignresult" => &[
            "system",
            "builder",
            "phase",
            "removedStructuralKeys",
            "retainedOccurrences",
        ],
        "stemsheadbuildertargetfilter" => &[
            "system",
            "builder",
            "ordinal",
            "kind",
            "alias",
            "stump",
            "removedByStructuralSeed",
            "action",
        ],
        "stemsheadbuilderlasthead" => &[
            "system",
            "builder",
            "lastHeadY",
            "pastSeedDrops",
            "pastVSectionDrops",
            "pastHSectionDrops",
        ],
        "stemsheadbuilderseedfilter" => &[
            "system",
            "builder",
            "ordinal",
            "glyph",
            "removedStructural",
            "retainedIdentity",
            "pastLastHead",
        ],
        "stemsheadbuilderheadparts" => &[
            "system",
            "builder",
            "event",
            "glyph",
            "head",
            "yOverlap",
            "weight",
            "removed",
            "remain",
            "threshold",
            "vip",
            "lowRemain",
            "action",
        ],
        "stemsheadbuilderchunkfilter" => &[
            "system",
            "builder",
            "inputOrdinal",
            "event",
            "glyph",
            "finalOrdinal",
            "action",
        ],
        "stemsheadbuilderseeditemfilter" => &[
            "system",
            "builder",
            "ordinal",
            "glyph",
            "duplicateTargetIdentity",
            "duplicateAlias",
            "startYOverlap",
            "contrib",
            "action",
        ],
        "stemsheadbuildersortaudit" => &[
            "system",
            "builder",
            "phase",
            "items",
            "strictCycles",
            "equivalenceInconsistencies",
            "offenderSha256",
            "jdk25MiniTimSort",
        ],
        "stemsheadbuildersort" => &[
            "system",
            "builder",
            "phase",
            "input",
            "output",
            "alias",
            "kind",
            "line",
            "ref",
            "key1",
            "key2",
            "equalInputs",
            "stableEqualPredecessors",
        ],
        "stemsheadbuilderitempre" => &[
            "system",
            "builder",
            "creationOrdinal",
            "sortedOrdinal",
            "kind",
            "alias",
            "line",
            "glyph",
            "contrib",
        ],
        "stemsheadbuildergap" => &[
            "system",
            "builder",
            "ordinal",
            "itemIndex",
            "itemAlias",
            "start",
            "stop",
            "lastBefore",
            "gap",
            "maxGap",
            "epsilon",
            "action",
            "insertedLine",
            "insertedContrib",
        ],
        "stemsheadbuilderitem" => &[
            "system", "builder", "ordinal", "kind", "alias", "line", "glyph", "contrib",
        ],
        "stemsheadbuilderlength" => &[
            "system",
            "builder",
            "profile",
            "threshold",
            "length",
            "replayLength",
        ],
        "stemsheadbuilderend" => &[
            "system",
            "builder",
            "seeds",
            "items",
            "lengths",
            "filaments",
            "glyphNew",
            "glyphReuse",
            "bArenaBefore",
            "bArenaAfter",
            "anchorsCreated",
            "sigVertexDelta",
            "sigEdgeDelta",
            "stemInterDelta",
            "systemStemDelta",
            "linkMutations",
            "sbAssigned",
            "modeledRegistryBefore",
            "modeledRegistryAfter",
            "registryShaBefore",
            "registryShaAfter",
            "registrationActionSha256",
        ],
        _ => panic!("row family missing schema contract: {family}"),
    };
    assert_exact_field_schema(row, fields);
}

/// The fixture's semantic schema is a source contract, not a lower bound.
/// Accepting a subset of fields would let the Java probe grow a new identity
/// dependency without requiring the native projector to model it.
fn assert_exact_field_schema(row: &str, expected: &[&str]) {
    assert_exact_field_schema_after(row, 2, expected);
}

fn assert_exact_field_schema_after(row: &str, skip: usize, expected: &[&str]) {
    let words = row.split_whitespace().skip(skip).collect::<Vec<_>>();
    assert_eq!(words.len() % 2, 0, "odd field/value stream in {row}");
    assert_eq!(
        words.len() / 2,
        expected.len(),
        "field count changed in {row}"
    );
    for (index, (pair, field)) in words.chunks_exact(2).zip(expected).enumerate() {
        assert_eq!(pair[0], *field, "field {index} changed in {row}");
    }
}

fn validate_registration(result: &str, origins: &str, row: &str) {
    match result {
        "New" => assert_eq!(origins, "-", "New registration had prior origin: {row}"),
        "Reuse" => {
            assert_ne!(
                origins, "-",
                "Reuse registration lacks modeled origin: {row}"
            );
            assert!(!origins.split(',').any(|origin| origin == "otherPreStems"));
        }
        action => panic!("unknown registration action {action}: {row}"),
    }
}

fn registry_point(row: &str, count_field: &str, sha_field: &str) -> RegistryCursor {
    let sha256 = required_value(row, sha_field).to_owned();
    assert_sha256(&sha256, sha_field);
    RegistryCursor {
        count: required_usize(row, count_field),
        sha256,
    }
}

fn validate_registry_transition(row: &str, registry: &mut Option<RegistryCursor>) {
    let before = registry_point(row, "modeledRegistryBefore", "registryShaBefore");
    assert_eq!(
        registry.as_ref(),
        Some(&before),
        "registry chronology before row"
    );
    let after = registry_point(row, "modeledRegistryAfter", "registryShaAfter");
    assert!(after.count >= before.count, "registry shrank");
    *registry = Some(after);
}

fn include_row_counts(total: &mut RowCounts, page: &RowCounts) {
    for (family, count) in &page.by_family {
        *total.by_family.entry(family.clone()).or_default() += count;
    }
    total.stump_registrations += page.stump_registrations;
    total.stump_registration_new += page.stump_registration_new;
    total.stump_registration_reuse += page.stump_registration_reuse;
    total.stump_action_diffs += page.stump_action_diffs;
    total.heads += page.heads;
    total.c_builders += page.c_builders;
    total.c_top += page.c_top;
    total.c_bottom += page.c_bottom;
    total.stumpless_starts += page.stumpless_starts;
    total.input_seeds += page.input_seeds;
    total.input_c_targets += page.input_c_targets;
    total.input_b_targets += page.input_b_targets;
    total.beam_registrations += page.beam_registrations;
    total.head_registrations += page.head_registrations;
    total.head_registration_new += page.head_registration_new;
    total.head_registration_reuse += page.head_registration_reuse;
    total.head_anchors += page.head_anchors;
    total.length_rows += page.length_rows;
    total.vip_heads += page.vip_heads;
    total.small_heads += page.small_heads;
    total.keep_non_vip_bug += page.keep_non_vip_bug;
    total.remove_vip_only += page.remove_vip_only;
    total.beam_action_diffs += page.beam_action_diffs;
    total.head_to_later_beam_reuses += page.head_to_later_beam_reuses;
    total.profile_divergences += page.profile_divergences;
    total.builder_top += page.builder_top;
    total.builder_bottom += page.builder_bottom;
    total.direction_divergences += page.direction_divergences;
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repository root")
}

fn frozen_pin<'a>(pin: Option<&'a str>, label: &str) -> &'a str {
    pin.unwrap_or_else(|| panic!("{label} is not frozen"))
}

fn line_count(bytes: &[u8]) -> usize {
    bytes.iter().filter(|&&byte| byte == b'\n').count()
}

fn required_value_map<'a>(fields: &'a BTreeMap<String, String>, field: &str) -> &'a str {
    fields
        .get(field)
        .map(String::as_str)
        .unwrap_or_else(|| panic!("missing field {field}"))
}

fn required_value<'a>(row: &'a str, field: &str) -> &'a str {
    optional_value(row, field).unwrap_or_else(|| panic!("missing {field} in {row}"))
}

fn optional_value<'a>(row: &'a str, field: &str) -> Option<&'a str> {
    let family = row.split_whitespace().next()?;
    let skip = if family.starts_with("stemsheadbuildermanifest")
        || family == "stemsheadbuildercheckpoint"
    {
        1
    } else {
        2
    };
    row.split_whitespace()
        .skip(skip)
        .collect::<Vec<_>>()
        .chunks_exact(2)
        .find_map(|pair| (pair[0] == field).then_some(pair[1]))
}

fn required_usize(row: &str, field: &str) -> usize {
    required_value(row, field)
        .parse()
        .unwrap_or_else(|_| panic!("invalid usize {field} in {row}"))
}

fn required_i32(row: &str, field: &str) -> i32 {
    required_value(row, field)
        .parse()
        .unwrap_or_else(|_| panic!("invalid i32 {field} in {row}"))
}

fn required_bool(row: &str, field: &str) -> bool {
    match required_value(row, field) {
        "true" => true,
        "false" => false,
        value => panic!("invalid bool {field}={value} in {row}"),
    }
}

fn string_fields(row: &str, skip: usize) -> BTreeMap<String, String> {
    let words = row.split_whitespace().skip(skip).collect::<Vec<_>>();
    assert_eq!(words.len() % 2, 0, "odd field/value stream in {row}");
    words
        .chunks_exact(2)
        .map(|pair| (pair[0].to_owned(), pair[1].to_owned()))
        .collect()
}

fn numeric_fields(row: &str, skip: usize) -> BTreeMap<String, usize> {
    string_fields(row, skip)
        .into_iter()
        .filter_map(|(field, value)| value.parse().ok().map(|value| (field, value)))
        .collect()
}

fn assert_sha256(value: &str, label: &str) {
    assert_eq!(value.len(), 64, "{label} length");
    assert!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
        "{label} must be lowercase hexadecimal"
    );
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
        use std::fmt::Write as _;
        write!(&mut result, "{word:08x}").expect("format SHA-256");
    }
    result
}
