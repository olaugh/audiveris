// SPDX-License-Identifier: AGPL-3.0-or-later

//! Fail-closed differential gate for the beam-origin `VLinker.link` seam
//! between `StemBuilder.createStem` and the first SIG mutation.
//!
//! The independent model below deliberately spells out Java's ordered
//! head-side stem-reuse scan and `BeamStemRelation.checkLink` arithmetic.  It
//! does not use a digest as semantic evidence and never applies the returned
//! link: the boundary ends before `SIGraph.addVertex`, relation insertion, or
//! linker-flag mutation.

#![allow(dead_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    path::PathBuf,
};

use audiveris_core::java_math::java_positive_pow;
use audiveris_image::{
    beam_structure::Segment,
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable},
    section::Bounds,
};
use audiveris_omr::{
    head_scanner_slices::{JavaRectangle, VerticalRibbonArea},
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
        NativeStemsBeamSchedulerRecognition, NativeStemsBeamSchedulerStatus,
        materialize_native_stems_beam_scheduler_frontiers,
    },
    native_stems_beam_stumps::{
        NativeStemsBeamStumpRecognition, materialize_native_stems_beam_stumps,
    },
    native_stems_beam_vlink_reuse_check::{
        NativeStemsBeamHeadStemEdge, NativeStemsBeamHeadStemLookupEvidence,
        NativeStemsBeamHeadStemLookupTrace, NativeStemsBeamHeadStemScan,
        NativeStemsBeamRelationParameters, NativeStemsBeamReuseAction,
        NativeStemsBeamReuseDisposition, NativeStemsBeamReuseEntryEvidence,
        NativeStemsBeamReuseEntryObservation, NativeStemsBeamVLinkReuseCheckError,
        NativeStemsBeamVLinkReuseCheckOutcome, NativeStemsBeamVLinkReuseLiveEvaluation,
        NativeStemsBeamVLinkReuseLiveState, evaluate_native_stems_beam_vlink_reuse_check,
    },
    native_stems_beam_vlink_transaction::{
        NativeStemsBeamExhaustiveGlyphEqualsScan, NativeStemsBeamExhaustiveGlyphLookup,
        NativeStemsBeamExhaustiveSystemStemEqualsScan, NativeStemsBeamExhaustiveSystemStemLookup,
        NativeStemsBeamFixedGlyphContent, NativeStemsBeamGlyphAliasOrder,
        NativeStemsBeamGlyphIndexTransactionState, NativeStemsBeamKnownSystemStem,
        NativeStemsBeamPersistentIdState, NativeStemsBeamSelectedGlyphBinding,
        NativeStemsBeamStemCheckerContext, NativeStemsBeamStemGrade,
        NativeStemsBeamSystemStemTransactionState, NativeStemsBeamVLinkLineState,
        NativeStemsBeamVLinkTransaction, NativeStemsBeamVLinkTransactionScope,
        NativeStemsBeamVLinkTransactionState,
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
    stem_seeds_step::NativeStemCheckerParameters,
    stems_step::{
        NativeBeamPortion, NativeStemHeadSide, NativeStemLine, NativeStemPoint,
        NativeStemVerticalSide,
    },
};

const INSPECT_PROFILE: i32 = 1;
const STEM_MINIMUM_GRADE: f64 = 0.8 * 0.1;
const ARTIFICIAL_STEM_GRADE: f64 = 0.4;

const CREATE_STEM_SCHEMA: &str = "# schema: stems-beam-create-stem-v1";
const CREATE_PAGE_FIELDS: &[&str] = &[
    "systems",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "executionMode",
    "registryHashMode",
];
const CREATE_BASELINE_FIELDS: &[&str] = &[
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
const CREATE_FRONTIER_FIELDS: &[&str] = &[
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
const CREATE_EXPAND_FIELDS: &[&str] = &[
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
const CREATE_LOOKUP_FIELDS: &[&str] = &[
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
const CREATE_RESULT_FIELDS: &[&str] = &[
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
const CREATE_DELTA_FIELDS: &[&str] = &[
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
const CREATE_GUARD_FIELDS: &[&str] = &[
    "system",
    "lineDeltaRetained",
    "interIndexUnchanged",
    "sigUnchanged",
    "relationsUnchanged",
    "linkerFlagsUnchanged",
    "stopBeforeVReuse",
    "stopBeforeBeamStemCheck",
];
const CREATE_SUMMARY_FIELDS: &[&str] = &[
    "system",
    "transaction",
    "registration",
    "disposition",
    "allocatorDelta",
];
const CREATE_PAGE_SUMMARY_FIELDS: &[&str] = &[
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
const CREATE_CORPUS_FIELDS: &[&str] = &[
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

const CREATE_SYSTEM_FAMILIES: [(&str, &[&str]); 8] = [
    ("stemsbeamcreatestembaseline", CREATE_BASELINE_FIELDS),
    ("stemsbeamcreatestemfrontier", CREATE_FRONTIER_FIELDS),
    ("stemsbeamcreatestemexpand", CREATE_EXPAND_FIELDS),
    ("stemsbeamcreatestemlookup", CREATE_LOOKUP_FIELDS),
    ("stemsbeamcreatestemresult", CREATE_RESULT_FIELDS),
    ("stemsbeamcreatestemdelta", CREATE_DELTA_FIELDS),
    ("stemsbeamcreatestemguard", CREATE_GUARD_FIELDS),
    ("stemsbeamcreatestemsummary", CREATE_SUMMARY_FIELDS),
];

const REUSE_SCHEMA: &str = "# schema: stems-beam-vlink-reuse-check-v1";
const REUSE_PAGE_FIELDS: &[&str] = &[
    "systems",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "createStemFixtureSha256",
    "executionMode",
    "relationOrder",
    "registryHashMode",
];
const REUSE_BASELINE_FIELDS: &[&str] = &[
    "system",
    "plan",
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
    "sigRelationStateHash",
    "systemStemsHash",
    "linkerStateHash",
    "lineStateHash",
    "relationInputHash",
];
const REUSE_FRONTIER_FIELDS: &[&str] = &[
    "system",
    "plan",
    "beamOrder",
    "beamSig",
    "hSide",
    "bAlias",
    "vSide",
    "headToBeam",
    "builder",
    "stemProfile",
    "linkProfile",
    "lastIndex",
    "relationCount",
    "relationAliases",
    "relationsPastReturn",
    "glyphCount",
    "liveStemCount",
    "liveStemCatalogueHash",
    "selectedGlyphRefs",
    "createCandidateObjectIdBefore",
    "createRegistration",
    "createDisposition",
    "createRegisteredAlias",
    "createReturnedStemInterId",
    "createStemGrade",
    "createStemMedian",
    "createStemWidth",
    "createStemBounds",
    "createStemAbnormal",
    "createStemSigAttached",
    "predecessorJoin",
];
const REUSE_LIVE_STEM_FIELDS: &[&str] = &[
    "system",
    "plan",
    "catalogOrdinal",
    "sigVertexOrdinal",
    "stemAlias",
    "javaInterId",
    "firstHeadStemEdgeOrdinal",
    "glyphId",
    "glyph",
    "grade",
    "median",
    "width",
    "bounds",
    "abnormal",
    "sigAttached",
    "stateHash",
];
const REUSE_ENTRY_FIELDS: &[&str] = &[
    "system",
    "plan",
    "mapOrdinal",
    "cAlias",
    "conditionRead",
    "evidenceValidated",
    "sLinked",
    "parentHSide",
    "parentVSide",
    "headRef",
    "headInterId",
    "relationHeadSide",
    "sideMismatch",
    "lookupState",
    "scanState",
    "incidentEdges",
    "matchingEdges",
    "distinctSideStems",
    "headSnapshotHash",
    "projectionHash",
    "sideStemAliases",
    "selectedStemAlias",
    "action",
];
const REUSE_HEAD_SCAN_FIELDS: &[&str] = &[
    "system",
    "plan",
    "mapOrdinal",
    "scanOrdinal",
    "sigEdgeOrdinal",
    "scanOrderDomain",
    "sigEdgeIdentityDomain",
    "edgeHeadSide",
    "targetStemAlias",
    "targetStemInterId",
    "targetSigAttached",
    "targetGlyphId",
    "matchesParentSide",
    "distinctInsertion",
    "headSnapshotHash",
    "action",
];
const REUSE_SELECTION_FIELDS: &[&str] = &[
    "system",
    "plan",
    "initialStemAlias",
    "initialStemInterId",
    "finalStemAlias",
    "finalStemInterId",
    "reused",
    "selectedMapOrdinal",
    "selectedC",
    "breakIndex",
    "entriesTotal",
    "entriesConditionRead",
    "unreadSuffix",
    "linkedEntries",
    "allLinked",
    "reuseOutcome",
    "terminal",
    "finalGlyphId",
    "finalGlyph",
    "finalGrade",
    "finalMedian",
    "finalWidth",
    "finalBounds",
    "finalAbnormal",
    "finalSigAttached",
    "finalStateHash",
];
const REUSE_CHECK_CONTEXT_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "beamMedian",
    "beamHeight",
    "vSide",
    "headToBeam",
    "borderSide",
    "beamBorder",
    "stemAlias",
    "stemInterId",
    "stemMedian",
    "stemActualWidth",
    "interline",
    "scaleStemThickness",
    "halfScaleStemWidth",
    "profile",
    "portionXInP0",
    "xOutMax",
    "yMax",
    "maxDxInput",
    "maxDxRint",
    "maxDxInt",
    "xWeight",
    "yWeight",
    "intrinsicRatio",
    "minGrade",
    "portionComparison",
    "gradeComparison",
];
const REUSE_CHECK_TRACE_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "intersectionDen",
    "v12",
    "v34",
    "xNumerator",
    "yNumerator",
    "crossX",
    "crossY",
    "leftThreshold",
    "rightThreshold",
    "portion",
    "signedXGapPixels",
    "signedDxFrac",
    "storedDx",
    "upperGapRaw",
    "upperGap",
    "lowerGapRaw",
    "lowerGap",
    "yGapPixels",
    "storedDy",
    "xImpactRaw",
    "yImpactRaw",
    "xImpact",
    "yImpact",
    "xPow",
    "yPow",
    "global",
    "totalWeight",
    "reciprocalWeight",
    "root",
    "intrinsicRatio",
    "grade",
    "candidateExtension",
    "accepted",
];
const REUSE_CHECK_RESULT_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "checkOutcome",
    "linkNull",
    "partnerAlias",
    "partnerInterId",
    "outgoing",
    "relationClass",
    "portion",
    "dx",
    "dy",
    "grade",
    "impacts",
    "extension",
    "terminal",
];
const REUSE_GUARD_FIELDS: &[&str] = &[
    "system",
    "plan",
    "allocatorBefore",
    "allocatorAfter",
    "glyphActiveHashBefore",
    "glyphActiveHashAfter",
    "glyphOriginalsHashBefore",
    "glyphOriginalsHashAfter",
    "interIndexHashBefore",
    "interIndexHashAfter",
    "sigHashBefore",
    "sigHashAfter",
    "sigRelationStateHashBefore",
    "sigRelationStateHashAfter",
    "systemStemsHashBefore",
    "systemStemsHashAfter",
    "linkerStateHashBefore",
    "linkerStateHashAfter",
    "lineStateHashBefore",
    "lineStateHashAfter",
    "liveStemCatalogueHashBefore",
    "liveStemCatalogueHashAfter",
    "relationInputHashBefore",
    "relationInputHashAfter",
    "createStemStateUnchanged",
    "registriesUnchanged",
    "allocatorUnchanged",
    "linesUnchanged",
    "systemStemsUnchanged",
    "liveStemsUnchanged",
    "interIndexUnchanged",
    "sigUnchanged",
    "relationsUnchanged",
    "linkerFlagsUnchanged",
    "stopBeforeSigAddVertex",
    "stopBeforeRelationApply",
    "stopBeforeLinkerFlagMutation",
];
const REUSE_SUMMARY_FIELDS: &[&str] = &[
    "system",
    "plan",
    "transaction",
    "relationEntries",
    "linkedEntries",
    "sideScans",
    "reused",
    "reuseOutcome",
    "checkOutcome",
    "terminal",
    "realReuseCensus",
];
const REUSE_SYNTHETIC_LIVE_STEM_FIELDS: &[&str] = &[
    "system",
    "plan",
    "case",
    "origin",
    "idNamespace",
    "catalogOrdinal",
    "sigVertexOrdinal",
    "stemAlias",
    "javaInterId",
    "firstHeadStemEdgeOrdinal",
    "glyphId",
    "glyph",
    "grade",
    "median",
    "width",
    "bounds",
    "abnormal",
    "sigAttached",
    "stateHash",
];
const REUSE_SYNTHETIC_ENTRY_FIELDS: &[&str] = &[
    "system",
    "plan",
    "case",
    "mapOrdinal",
    "cAlias",
    "conditionRead",
    "evidenceValidated",
    "sLinked",
    "parentHSide",
    "headInterId",
    "isolatedSig",
    "scanState",
    "incidentEdges",
    "headSnapshotHash",
    "projectionHash",
    "lookupState",
    "sideStemCount",
    "sideStemAliases",
    "selectedStemAlias",
    "action",
];
const REUSE_SYNTHETIC_UNREAD_ENTRY_FIELDS: &[&str] = &[
    "system",
    "plan",
    "case",
    "mapOrdinal",
    "cAlias",
    "conditionRead",
    "evidenceValidated",
    "sLinked",
    "parentHSide",
    "lookupState",
    "sideStemCount",
    "sideStemAliases",
    "selectedStemAlias",
    "action",
];
const REUSE_SYNTHETIC_HEAD_SCAN_FIELDS: &[&str] = &[
    "system",
    "plan",
    "case",
    "mapOrdinal",
    "scanOrdinal",
    "sigEdgeOrdinal",
    "scanOrderDomain",
    "sigEdgeIdentityDomain",
    "edgeHeadSide",
    "targetStemAlias",
    "targetStemInterId",
    "targetSigAttached",
    "targetGlyphId",
    "matchesParentSide",
    "distinctInsertion",
    "headSnapshotHash",
    "action",
];
const REUSE_SYNTHETIC_SELECTION_FIELDS: &[&str] = &[
    "system",
    "plan",
    "case",
    "origin",
    "cardinalityModel",
    "cardinality",
    "entriesTotal",
    "entriesConditionRead",
    "unreadSuffix",
    "selectedMapOrdinal",
    "outcome",
    "finalStemAlias",
    "finalStemInterId",
    "finalGlyphId",
    "finalGlyph",
    "finalGrade",
    "finalMedian",
    "finalWidth",
    "finalBounds",
    "finalAbnormal",
    "finalSigAttached",
    "checkOutcome",
    "zeroMutation",
];
const REUSE_SYNTHETIC_PORTION_FIELDS: &[&str] = &[
    "system",
    "plan",
    "case",
    "xStem",
    "leftThreshold",
    "rightThreshold",
    "maxDx",
    "comparison",
    "portion",
];
const REUSE_SYNTHETIC_THRESHOLD_FIELDS: &[&str] = &[
    "system",
    "plan",
    "grade",
    "minGrade",
    "comparison",
    "accepted",
];
const REUSE_SYNTHETIC_PARALLEL_FIELDS: &[&str] = &[
    "system",
    "plan",
    "stemMedian",
    "border",
    "den",
    "crossX",
    "crossY",
    "grade",
    "checkOutcome",
    "zeroMutation",
];
const REUSE_SYNTHETIC_INTERSECTION_FIELDS: &[&str] = &[
    "system",
    "plan",
    "case",
    "first",
    "second",
    "den",
    "v12",
    "v34",
    "xNumerator",
    "yNumerator",
    "crossX",
    "crossY",
    "finiteInputs",
    "exactLineUtil",
];
const REUSE_SYNTHETIC_GUARD_FIELDS: &[&str] = &[
    "system",
    "plan",
    "origin",
    "isolatedSigHashBefore",
    "isolatedSigHashAfter",
    "liveStemHashBefore",
    "liveStemHashAfter",
    "realAllocatorBefore",
    "realAllocatorAfter",
    "realInterIndexHashBefore",
    "realInterIndexHashAfter",
    "realSigHashBefore",
    "realSigHashAfter",
    "realSigRelationStateHashBefore",
    "realSigRelationStateHashAfter",
    "realSystemStemsHashBefore",
    "realSystemStemsHashAfter",
    "realLinkerStateHashBefore",
    "realLinkerStateHashAfter",
    "realLineStateHashBefore",
    "realLineStateHashAfter",
    "isolatedGraphUnchanged",
    "realSheetUnchanged",
    "allocatorUnchanged",
    "interIndexUnchanged",
    "sigUnchanged",
    "relationsUnchanged",
    "linkerFlagsUnchanged",
    "zeroMutation",
];
const REUSE_PAGE_SUMMARY_FIELDS: &[&str] = &[
    "systems",
    "transactions",
    "relationEntries",
    "linkedEntries",
    "sideScans",
    "liveStemRows",
    "realReuses",
    "acceptedChecks",
    "rejectedChecks",
    "syntheticLiveStemRows",
    "syntheticHeadScanRows",
    "syntheticGuardRows",
    "syntheticZeroCases",
    "syntheticUniqueCases",
    "syntheticMultipleCases",
    "syntheticIntersectionCases",
    "sigMutations",
    "relationMutations",
    "linkerFlagMutations",
];
const REUSE_CORPUS_FIELDS: &[&str] = &[
    "schema",
    "mode",
    "pages",
    "pageRefs",
    "rowCounts",
    "probeSourceSha256",
    "runnerSourceSha256",
    "beamLinkerSourceSha256",
    "beamStemRelationSourceSha256",
    "headInterSourceSha256",
    "headStemRelationSourceSha256",
    "lineUtilSourceSha256",
    "scaleSourceSha256",
    "abstractConnectionSourceSha256",
    "supportImpactsSourceSha256",
    "supportSourceSha256",
    "gradeImpactsSourceSha256",
    "gradeUtilSourceSha256",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "createStemFixtureSha256",
    "pairedFixtureSwapNegative",
    "emittedBodySha256",
    "emittedBodyLines",
    "emittedBodyBytes",
    "freshRunsPerPage",
    "freshRunsByteIdentical",
    "rawPassSha256",
    "freshJvmPerPage",
    "freshJvmPerSystem",
    "compilerJavaProcesses",
    "runtimeJavaProcessesPerPass",
    "runtimeJavaProcesses",
    "totalJavaProcesses",
    "maximumConcurrentJavaProcesses",
    "freshSheetPerSystem",
    "compilerJavaProcessReaped",
    "runnerJavaProcessesReaped",
    "foregroundJavaProcessesOnly",
    "backgroundJavaProcessesStarted",
    "realReuseCensus",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReusePageSpec {
    key: &'static str,
    image: &'static str,
    page: &'static str,
    fixture: &'static str,
    create_stem_fixture: &'static str,
}

const REUSE_PAGES: [ReusePageSpec; 8] = [
    ReusePageSpec {
        key: "chula",
        image: "chula.png",
        page: "chula.png#1",
        fixture: "stems-beam-vlink-reuse-check-chula.txt",
        create_stem_fixture: "stems-beam-create-stem-chula.txt",
    },
    ReusePageSpec {
        key: "allegretto",
        image: "allegretto.png",
        page: "allegretto.png#1",
        fixture: "stems-beam-vlink-reuse-check-allegretto.txt",
        create_stem_fixture: "stems-beam-create-stem-allegretto.txt",
    },
    ReusePageSpec {
        key: "batuque",
        image: "batuque.png",
        page: "batuque.png#1",
        fixture: "stems-beam-vlink-reuse-check-batuque.txt",
        create_stem_fixture: "stems-beam-create-stem-batuque.txt",
    },
    ReusePageSpec {
        key: "carmen",
        image: "carmen.png",
        page: "carmen.png#1",
        fixture: "stems-beam-vlink-reuse-check-carmen.txt",
        create_stem_fixture: "stems-beam-create-stem-carmen.txt",
    },
    ReusePageSpec {
        key: "cucaracha",
        image: "cucaracha.png",
        page: "cucaracha.png#1",
        fixture: "stems-beam-vlink-reuse-check-cucaracha.txt",
        create_stem_fixture: "stems-beam-create-stem-cucaracha.txt",
    },
    ReusePageSpec {
        key: "hove",
        image: "hove.png",
        page: "hove.png#1",
        fixture: "stems-beam-vlink-reuse-check-hove.txt",
        create_stem_fixture: "stems-beam-create-stem-hove.txt",
    },
    ReusePageSpec {
        key: "zizi",
        image: "zizi.png",
        page: "zizi.png#1",
        fixture: "stems-beam-vlink-reuse-check-zizi.txt",
        create_stem_fixture: "stems-beam-create-stem-zizi.txt",
    },
    ReusePageSpec {
        key: "BachInvention5",
        image: "BachInvention5.jpg",
        page: "BachInvention5.jpg#1",
        fixture: "stems-beam-vlink-reuse-check-BachInvention5.txt",
        create_stem_fixture: "stems-beam-create-stem-BachInvention5.txt",
    },
];

const REUSE_SOURCE_PATHS: [(&str, &str); 13] = [
    (
        "probeSourceSha256",
        "rust/oracle/java/StemsBeamVLinkReuseCheckProbe.java",
    ),
    (
        "runnerSourceSha256",
        "rust/oracle/java/run-stems-beam-vlink-reuse-check.sh",
    ),
    (
        "beamLinkerSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/stem/BeamLinker.java",
    ),
    (
        "beamStemRelationSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/relation/BeamStemRelation.java",
    ),
    (
        "headInterSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/inter/HeadInter.java",
    ),
    (
        "headStemRelationSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/relation/HeadStemRelation.java",
    ),
    (
        "lineUtilSourceSha256",
        "app/src/main/java/org/audiveris/omr/math/LineUtil.java",
    ),
    (
        "scaleSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/Scale.java",
    ),
    (
        "abstractConnectionSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/relation/AbstractConnection.java",
    ),
    (
        "supportImpactsSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/relation/SupportImpacts.java",
    ),
    (
        "supportSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/relation/Support.java",
    ),
    (
        "gradeImpactsSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/GradeImpacts.java",
    ),
    (
        "gradeUtilSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/GradeUtil.java",
    ),
];

const REUSE_ROW_COUNT_FIELDS: usize = 22;
const REUSE_MANIFEST_SCHEMA: &str = "# schema: stems-beam-vlink-reuse-check-manifest-v1";
const REUSE_MANIFEST_PATH: &str = "rust/oracle/stems-beam-vlink-reuse-check-manifest.txt";
const EXPECTED_REUSE_MANIFEST_SHA256: &str =
    "4ab7078b760daca6691fcc03e8f29684ec4c976f918d747cb2047f01accd0559";
const EXPECTED_REUSE_CORPUS_BODY_SHA256: &str =
    "76a6d20865a5a372bb6485ff6debeb0c435b64d1f92cf5ee07e1fbe0cf61418f";
const EXPECTED_REUSE_CORPUS_BODY_LINES: usize = 601;
const EXPECTED_REUSE_CORPUS_BODY_BYTES: usize = 472_445;
const EXPECTED_REUSE_CORPUS_ROW_COUNTS: [usize; REUSE_ROW_COUNT_FIELDS] = [
    8, 30, 30, 0, 65, 0, 30, 54, 54, 54, 30, 30, 16, 32, 24, 24, 32, 8, 8, 8, 8, 8,
];
const EXPECTED_REUSE_MANIFEST_BODY_SHA256: &str =
    "58259448c36c5c684cbfef2215eb124a2ca62e5aae8f12d1a73510345687fb6d";
const EXPECTED_REUSE_MANIFEST_BODY_LINES: usize = 9;
const EXPECTED_REUSE_MANIFEST_BODY_BYTES: usize = 9_202;
const REUSE_MANIFEST_ENTRY_FIELDS: &[&str] = &[
    "ordinal",
    "page",
    "fixture",
    "rowCounts",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "createStemFixtureSha256",
    "emittedBodySha256",
    "emittedBodyLines",
    "emittedBodyBytes",
    "rawPassSha256",
    "fixtureSha256",
    "fixtureLines",
    "fixtureBytes",
    "freshRunsPerPage",
    "freshRunsByteIdentical",
    "compilerJavaProcesses",
    "runtimeJavaProcesses",
    "totalJavaProcesses",
    "maximumConcurrentJavaProcesses",
    "freshJvmPerPage",
    "freshJvmPerSystem",
    "freshSheetPerSystem",
    "compilerJavaProcessReaped",
    "runnerJavaProcessesReaped",
    "foregroundJavaProcessesOnly",
    "backgroundJavaProcessesStarted",
    "system1SyntheticBlock",
    "realReuseCensus",
];
const REUSE_MANIFEST_SUMMARY_FIELDS: &[&str] = &[
    "schema",
    "entries",
    "probeSourceSha256",
    "runnerSourceSha256",
    "beamLinkerSourceSha256",
    "beamStemRelationSourceSha256",
    "headInterSourceSha256",
    "headStemRelationSourceSha256",
    "lineUtilSourceSha256",
    "scaleSourceSha256",
    "abstractConnectionSourceSha256",
    "supportImpactsSourceSha256",
    "supportSourceSha256",
    "gradeImpactsSourceSha256",
    "gradeUtilSourceSha256",
    "corpusBodySha256",
    "corpusBodyLines",
    "corpusBodyBytes",
    "corpusRowCounts",
    "realSystems",
    "realTransactions",
    "realRelationEntries",
    "realLinkedEntries",
    "realHeadScans",
    "realLiveStems",
    "realReuseSelections",
    "realChecksAccepted",
    "realChecksRejected",
    "syntheticBlocks",
    "compilerJavaProcesses",
    "runtimeJavaProcesses",
    "totalJavaProcesses",
    "maximumConcurrentJavaProcesses",
    "freshRunsPerPage",
    "freshRunsByteIdentical",
    "freshJvmPerPage",
    "freshJvmPerSystem",
    "freshSheetPerSystem",
    "compilerJavaProcessesReaped",
    "runnerJavaProcessesReaped",
    "foregroundJavaProcessesOnly",
    "backgroundJavaProcessesStarted",
    "realReuseCensus",
    "manifestBodySha256",
    "manifestBodyLines",
    "manifestBodyBytes",
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReuseManifestEntry {
    ordinal: usize,
    page: String,
    fixture: String,
    row_counts: [usize; REUSE_ROW_COUNT_FIELDS],
    scheduler_sha256: String,
    expand_sha256: String,
    create_stem_sha256: String,
    body_sha256: String,
    body_lines: usize,
    body_bytes: usize,
    raw_pass_sha256: String,
    fixture_sha256: String,
    fixture_lines: usize,
    fixture_bytes: usize,
    compiler_java_processes: usize,
    runtime_java_processes: usize,
    total_java_processes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReuseManifest {
    entries: Vec<ReuseManifestEntry>,
    summary: BTreeMap<String, String>,
}

fn mapped_value<'a>(map: &'a BTreeMap<String, String>, label: &str) -> Result<&'a str, String> {
    map.get(label)
        .map(String::as_str)
        .ok_or_else(|| format!("row lacks {label}"))
}

fn mapped_usize(map: &BTreeMap<String, String>, label: &str) -> Result<usize, String> {
    mapped_value(map, label)?
        .parse()
        .map_err(|_| format!("row has invalid {label}"))
}

fn parse_reuse_row_counts(value: &str) -> Result<[usize; REUSE_ROW_COUNT_FIELDS], String> {
    value
        .split(':')
        .map(|field| {
            field
                .parse::<usize>()
                .map_err(|_| format!("invalid reuse row counts: {value}"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| format!("reuse row-count arity differs: {value}"))
}

impl ReuseManifest {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
        let raw_lines = text.lines().collect::<Vec<_>>();
        if !bytes.ends_with(b"\n")
            || raw_lines.first().copied() != Some(REUSE_MANIFEST_SCHEMA)
            || raw_lines.len() != REUSE_PAGES.len() + 2
            || raw_lines[1..]
                .iter()
                .any(|line| line.is_empty() || line.starts_with('#'))
        {
            return Err("reuse/check manifest schema header differs".to_owned());
        }
        let lines = &raw_lines[1..];
        let mut entries = Vec::with_capacity(REUSE_PAGES.len());
        for (ordinal, (line, spec)) in lines[..REUSE_PAGES.len()]
            .iter()
            .zip(REUSE_PAGES)
            .enumerate()
        {
            let row = parse_exact_pairs(
                line,
                "stemsbeamvlinkreusecheckmanifestentry",
                REUSE_MANIFEST_ENTRY_FIELDS,
            )?;
            let row_counts = parse_reuse_row_counts(mapped_value(&row, "rowCounts")?)?;
            let compiler_java_processes = mapped_usize(&row, "compilerJavaProcesses")?;
            let runtime_java_processes = mapped_usize(&row, "runtimeJavaProcesses")?;
            let total_java_processes = mapped_usize(&row, "totalJavaProcesses")?;
            if mapped_usize(&row, "ordinal")? != ordinal
                || mapped_value(&row, "page")? != spec.page
                || mapped_value(&row, "fixture")? != spec.fixture
                || mapped_value(&row, "freshRunsPerPage")? != "2"
                || mapped_value(&row, "freshRunsByteIdentical")? != "true"
                || compiler_java_processes != 1
                || runtime_java_processes != 2 * row_counts[1]
                || total_java_processes != runtime_java_processes + compiler_java_processes
                || mapped_value(&row, "maximumConcurrentJavaProcesses")? != "1"
                || mapped_value(&row, "freshJvmPerPage")? != "false"
                || mapped_value(&row, "freshJvmPerSystem")? != "true"
                || mapped_value(&row, "freshSheetPerSystem")? != "true"
                || mapped_value(&row, "compilerJavaProcessReaped")? != "true"
                || mapped_value(&row, "runnerJavaProcessesReaped")? != "true"
                || mapped_value(&row, "foregroundJavaProcessesOnly")? != "true"
                || mapped_value(&row, "backgroundJavaProcessesStarted")? != "0"
                || mapped_value(&row, "system1SyntheticBlock")? != "true"
                || mapped_value(&row, "realReuseCensus")? != "0"
            {
                return Err(format!(
                    "reuse/check manifest entry {ordinal} algebra differs"
                ));
            }
            for label in [
                "schedulerFixtureSha256",
                "expandFixtureSha256",
                "createStemFixtureSha256",
                "emittedBodySha256",
                "rawPassSha256",
                "fixtureSha256",
            ] {
                parse_lower_hex(mapped_value(&row, label)?, 64, label)?;
            }
            entries.push(ReuseManifestEntry {
                ordinal,
                page: spec.page.to_owned(),
                fixture: spec.fixture.to_owned(),
                row_counts,
                scheduler_sha256: mapped_value(&row, "schedulerFixtureSha256")?.to_owned(),
                expand_sha256: mapped_value(&row, "expandFixtureSha256")?.to_owned(),
                create_stem_sha256: mapped_value(&row, "createStemFixtureSha256")?.to_owned(),
                body_sha256: mapped_value(&row, "emittedBodySha256")?.to_owned(),
                body_lines: mapped_usize(&row, "emittedBodyLines")?,
                body_bytes: mapped_usize(&row, "emittedBodyBytes")?,
                raw_pass_sha256: mapped_value(&row, "rawPassSha256")?.to_owned(),
                fixture_sha256: mapped_value(&row, "fixtureSha256")?.to_owned(),
                fixture_lines: mapped_usize(&row, "fixtureLines")?,
                fixture_bytes: mapped_usize(&row, "fixtureBytes")?,
                compiler_java_processes,
                runtime_java_processes,
                total_java_processes,
            });
        }
        let summary = parse_exact_pairs(
            lines[REUSE_PAGES.len()],
            "stemsbeamvlinkreusecheckmanifestsummary",
            REUSE_MANIFEST_SUMMARY_FIELDS,
        )?;
        for (label, _) in REUSE_SOURCE_PATHS {
            parse_lower_hex(mapped_value(&summary, label)?, 64, label)?;
        }
        for label in ["corpusBodySha256", "manifestBodySha256"] {
            parse_lower_hex(mapped_value(&summary, label)?, 64, label)?;
        }
        let real_systems = entries
            .iter()
            .map(|entry| entry.row_counts[1])
            .sum::<usize>();
        let real_relation_entries = entries
            .iter()
            .map(|entry| entry.row_counts[4])
            .sum::<usize>();
        let compiler_java_processes = entries
            .iter()
            .map(|entry| entry.compiler_java_processes)
            .sum::<usize>();
        let runtime_java_processes = entries
            .iter()
            .map(|entry| entry.runtime_java_processes)
            .sum::<usize>();
        let total_java_processes = entries
            .iter()
            .map(|entry| entry.total_java_processes)
            .sum::<usize>();
        if mapped_value(&summary, "schema")? != "stems-beam-vlink-reuse-check-manifest-v1"
            || mapped_usize(&summary, "entries")? != REUSE_PAGES.len()
            || mapped_usize(&summary, "realSystems")? != real_systems
            || mapped_usize(&summary, "realTransactions")? != real_systems
            || mapped_usize(&summary, "realRelationEntries")? != real_relation_entries
            || mapped_usize(&summary, "realLinkedEntries")? != 0
            || mapped_usize(&summary, "realHeadScans")? != 0
            || mapped_usize(&summary, "realLiveStems")? != 0
            || mapped_usize(&summary, "realReuseSelections")? != 0
            || mapped_usize(&summary, "realChecksAccepted")? != real_systems
            || mapped_usize(&summary, "realChecksRejected")? != 0
            || mapped_usize(&summary, "syntheticBlocks")? != REUSE_PAGES.len()
            || mapped_usize(&summary, "compilerJavaProcesses")? != compiler_java_processes
            || mapped_usize(&summary, "runtimeJavaProcesses")? != runtime_java_processes
            || mapped_usize(&summary, "totalJavaProcesses")? != total_java_processes
            || mapped_value(&summary, "maximumConcurrentJavaProcesses")? != "1"
            || mapped_value(&summary, "freshRunsPerPage")? != "2"
            || mapped_value(&summary, "freshRunsByteIdentical")? != "true"
            || mapped_value(&summary, "freshJvmPerPage")? != "false"
            || mapped_value(&summary, "freshJvmPerSystem")? != "true"
            || mapped_value(&summary, "freshSheetPerSystem")? != "true"
            || mapped_value(&summary, "compilerJavaProcessesReaped")? != "true"
            || mapped_value(&summary, "runnerJavaProcessesReaped")? != "true"
            || mapped_value(&summary, "foregroundJavaProcessesOnly")? != "true"
            || mapped_value(&summary, "backgroundJavaProcessesStarted")? != "0"
            || mapped_value(&summary, "realReuseCensus")? != "0"
        {
            return Err("reuse/check manifest summary algebra differs".to_owned());
        }
        Ok(Self { entries, summary })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExactRow {
    family: String,
    page: String,
    fields: BTreeMap<String, String>,
}

impl ExactRow {
    fn value(&self, label: &str) -> Result<&str, String> {
        self.fields
            .get(label)
            .map(String::as_str)
            .ok_or_else(|| format!("{} row lacks {label}", self.family))
    }

    fn usize(&self, label: &str) -> Result<usize, String> {
        self.value(label)?
            .parse()
            .map_err(|_| format!("{} row has invalid {label}", self.family))
    }

    fn i32(&self, label: &str) -> Result<i32, String> {
        self.value(label)?
            .parse()
            .map_err(|_| format!("{} row has invalid {label}", self.family))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CreateStemSystemRows {
    system_id: usize,
    rows: BTreeMap<String, ExactRow>,
}

impl CreateStemSystemRows {
    fn row(&self, family: &str) -> Result<&ExactRow, String> {
        self.rows
            .get(family)
            .ok_or_else(|| format!("system {} lacks {family}", self.system_id))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CreateStemFixture {
    page: String,
    page_row: ExactRow,
    systems: Vec<CreateStemSystemRows>,
    page_summary: ExactRow,
    corpus: BTreeMap<String, String>,
}

impl CreateStemFixture {
    fn parse(text: &str) -> Result<Self, String> {
        if text
            .lines()
            .filter(|line| *line == CREATE_STEM_SCHEMA)
            .count()
            != 1
        {
            return Err("createStem schema header must occur exactly once".to_owned());
        }
        let lines = text
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect::<Vec<_>>();
        let page_line = lines
            .first()
            .ok_or_else(|| "empty createStem fixture".to_owned())?;
        let page_row = parse_exact_row(page_line, "stemsbeamcreatestempage", CREATE_PAGE_FIELDS)?;
        let page = page_row.page.clone();
        let systems_count = page_row.usize("systems")?;
        if systems_count == 0
            || page_row.value("executionMode")? != "foregroundJvmPerSystem"
            || page_row.value("registryHashMode")? != "StructuralGlyphMultisetMembershipOnly"
        {
            return Err("createStem page execution envelope differs".to_owned());
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
        let expected_lines = 1 + (systems_count * CREATE_SYSTEM_FAMILIES.len()) + 2;
        if lines.len() != expected_lines {
            return Err(format!(
                "createStem semantic line count is {}, expected {expected_lines}",
                lines.len()
            ));
        }
        let mut cursor = 1_usize;
        let mut systems = Vec::with_capacity(systems_count);
        for system_id in 1..=systems_count {
            let mut rows = BTreeMap::new();
            for (family, labels) in CREATE_SYSTEM_FAMILIES {
                let row = parse_exact_row(lines[cursor], family, labels)?;
                if row.page != page || row.usize("system")? != system_id {
                    return Err(format!("createStem {family} hierarchy differs"));
                }
                if rows.insert(family.to_owned(), row).is_some() {
                    return Err(format!("duplicate createStem {family}"));
                }
                cursor += 1;
            }
            let system = CreateStemSystemRows { system_id, rows };
            validate_create_stem_system(&system)?;
            systems.push(system);
        }
        let page_summary = parse_exact_row(
            lines[cursor],
            "stemsbeamcreatestempagesummary",
            CREATE_PAGE_SUMMARY_FIELDS,
        )?;
        cursor += 1;
        if page_summary.page != page
            || page_summary.usize("systems")? != systems_count
            || page_summary.usize("transactions")? != systems_count
            || page_summary.usize("reusedStems")? != 0
            || page_summary.usize("sigMutations")? != 0
            || page_summary.usize("relationMutations")? != 0
            || page_summary.usize("linkerFlagMutations")? != 0
        {
            return Err("createStem page summary algebra differs".to_owned());
        }
        let corpus = parse_exact_pairs(
            lines[cursor],
            "stemsbeamcreatestemcorpussummary",
            CREATE_CORPUS_FIELDS,
        )?;
        if corpus.get("schema").map(String::as_str) != Some("stems-beam-create-stem-v1")
            || corpus.get("pages").map(String::as_str) != Some("1")
            || corpus.get("pageRefs").map(String::as_str) != Some(page.as_str())
            || corpus.get("freshJvmPerPage").map(String::as_str) != Some("false")
            || corpus.get("freshJvmPerSystem").map(String::as_str) != Some("true")
            || corpus.get("freshSheetPerSystem").map(String::as_str) != Some("true")
            || corpus.get("runnerJavaProcessesReaped").map(String::as_str) != Some("true")
            || corpus
                .get("backgroundJavaProcessesStarted")
                .map(String::as_str)
                != Some("0")
            || corpus.get("schedulerFixtureSha256") != page_row.fields.get("schedulerFixtureSha256")
            || corpus.get("expandFixtureSha256") != page_row.fields.get("expandFixtureSha256")
        {
            return Err("createStem corpus/page provenance differs".to_owned());
        }
        for label in [
            "probeSourceSha256",
            "runnerSourceSha256",
            "schedulerFixtureSha256",
            "expandFixtureSha256",
            "emittedBodySha256",
        ] {
            parse_lower_hex(
                corpus
                    .get(label)
                    .ok_or_else(|| format!("createStem corpus lacks {label}"))?,
                64,
                label,
            )?;
        }
        Ok(Self {
            page,
            page_row,
            systems,
            page_summary,
            corpus,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReuseEntryRows {
    entry: ExactRow,
    scans: Vec<ExactRow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReuseSystemRows {
    system_id: usize,
    baseline: ExactRow,
    frontier: ExactRow,
    live_stems: Vec<ExactRow>,
    entries: Vec<ReuseEntryRows>,
    selection: ExactRow,
    check_context: ExactRow,
    check_trace: ExactRow,
    check_result: ExactRow,
    guard: ExactRow,
    summary: ExactRow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReuseSyntheticRows {
    live_stems: Vec<ExactRow>,
    zero_entry: ExactRow,
    zero_scans: Vec<ExactRow>,
    zero_selection: ExactRow,
    unique_entries: Vec<ExactRow>,
    unique_scans: Vec<ExactRow>,
    unique_check_context: ExactRow,
    unique_check_trace: ExactRow,
    unique_check_result: ExactRow,
    unique_selection: ExactRow,
    multiple_entry: ExactRow,
    multiple_scans: Vec<ExactRow>,
    multiple_check_context: ExactRow,
    multiple_check_trace: ExactRow,
    multiple_check_result: ExactRow,
    multiple_selection: ExactRow,
    portions: Vec<ExactRow>,
    threshold: ExactRow,
    parallel_check_context: ExactRow,
    parallel_check_trace: ExactRow,
    parallel_check_result: ExactRow,
    parallel: ExactRow,
    intersection: ExactRow,
    guard: ExactRow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReuseFixture {
    page: String,
    page_row: ExactRow,
    systems: Vec<ReuseSystemRows>,
    synthetic: ReuseSyntheticRows,
    page_summary: ExactRow,
    corpus: BTreeMap<String, String>,
}

impl ReuseFixture {
    fn parse(text: &str) -> Result<Self, String> {
        if text.lines().filter(|line| *line == REUSE_SCHEMA).count() != 1 {
            return Err("reuse/check schema header must occur exactly once".to_owned());
        }
        let lines = text
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect::<Vec<_>>();
        let mut cursor = 0;
        let page_row = take_exact_row(
            &lines,
            &mut cursor,
            "stemsbeamvlinkreusecheckpage",
            REUSE_PAGE_FIELDS,
        )?;
        let page = page_row.page.clone();
        let systems_count = page_row.usize("systems")?;
        if systems_count == 0
            || page_row.value("executionMode")? != "foregroundJvmPerSystem"
            || page_row.value("relationOrder")? != "LinkedHashMap"
            || page_row.value("registryHashMode")? != "StructuralGlyphMultisetMembershipOnly"
        {
            return Err("reuse/check page execution envelope differs".to_owned());
        }
        for label in [
            "schedulerFixtureSha256",
            "expandFixtureSha256",
            "createStemFixtureSha256",
        ] {
            parse_lower_hex(page_row.value(label)?, 64, label)?;
        }

        let mut systems = Vec::with_capacity(systems_count);
        let mut synthetic = None;
        for system_id in 1..=systems_count {
            let baseline = take_system_row(
                &lines,
                &mut cursor,
                "stemsbeamvlinkreusecheckbaseline",
                REUSE_BASELINE_FIELDS,
                &page,
                system_id,
                None,
            )?;
            let plan = baseline.usize("plan")?;
            let frontier = take_system_row(
                &lines,
                &mut cursor,
                "stemsbeamvlinkreusecheckfrontier",
                REUSE_FRONTIER_FIELDS,
                &page,
                system_id,
                Some(plan),
            )?;
            let live_count = frontier.usize("liveStemCount")?;
            let mut live_stems = Vec::with_capacity(live_count);
            for catalog_ordinal in 0..live_count {
                let row = take_system_row(
                    &lines,
                    &mut cursor,
                    "stemsbeamvlinkreusechecklivestem",
                    REUSE_LIVE_STEM_FIELDS,
                    &page,
                    system_id,
                    Some(plan),
                )?;
                if row.usize("catalogOrdinal")? != catalog_ordinal {
                    return Err(format!(
                        "system {system_id} live stem catalogue order differs"
                    ));
                }
                live_stems.push(row);
            }
            let relation_count = frontier.usize("relationCount")?;
            let mut entries = Vec::with_capacity(relation_count);
            for map_ordinal in 0..relation_count {
                let entry = take_system_row(
                    &lines,
                    &mut cursor,
                    "stemsbeamvlinkreusecheckreuseentry",
                    REUSE_ENTRY_FIELDS,
                    &page,
                    system_id,
                    Some(plan),
                )?;
                if entry.usize("mapOrdinal")? != map_ordinal {
                    return Err(format!("system {system_id} reuse map order differs"));
                }
                let scan_count = match entry.value("scanState")? {
                    "NotRead" if entry.value("incidentEdges")? == "-" => 0,
                    "Exhaustive" => entry.usize("incidentEdges")?,
                    _ => {
                        return Err(format!("system {system_id} reuse scan envelope differs"));
                    }
                };
                let mut scans = Vec::with_capacity(scan_count);
                for scan_ordinal in 0..scan_count {
                    let scan = take_system_row(
                        &lines,
                        &mut cursor,
                        "stemsbeamvlinkreusecheckheadstemscan",
                        REUSE_HEAD_SCAN_FIELDS,
                        &page,
                        system_id,
                        Some(plan),
                    )?;
                    if scan.usize("mapOrdinal")? != map_ordinal
                        || scan.usize("scanOrdinal")? != scan_ordinal
                        || scan.value("scanOrderDomain")? != "HeadInterRelationsSourceOrder"
                        || scan.value("sigEdgeIdentityDomain")? != "SystemSigEdgeSetSourceOrder"
                    {
                        return Err(format!("system {system_id} head scan order differs"));
                    }
                    scans.push(scan);
                }
                entries.push(ReuseEntryRows { entry, scans });
            }
            let selection = take_system_row(
                &lines,
                &mut cursor,
                "stemsbeamvlinkreusecheckselection",
                REUSE_SELECTION_FIELDS,
                &page,
                system_id,
                Some(plan),
            )?;
            let check_context = take_scoped_check_row(
                &lines,
                &mut cursor,
                "stemsbeamvlinkreusecheckcheckcontext",
                REUSE_CHECK_CONTEXT_FIELDS,
                &page,
                system_id,
                plan,
                "real",
                "-",
            )?;
            let check_trace = take_scoped_check_row(
                &lines,
                &mut cursor,
                "stemsbeamvlinkreusecheckchecktrace",
                REUSE_CHECK_TRACE_FIELDS,
                &page,
                system_id,
                plan,
                "real",
                "-",
            )?;
            let check_result = take_scoped_check_row(
                &lines,
                &mut cursor,
                "stemsbeamvlinkreusecheckcheckresult",
                REUSE_CHECK_RESULT_FIELDS,
                &page,
                system_id,
                plan,
                "real",
                "-",
            )?;
            let guard = take_system_row(
                &lines,
                &mut cursor,
                "stemsbeamvlinkreusecheckguard",
                REUSE_GUARD_FIELDS,
                &page,
                system_id,
                Some(plan),
            )?;
            let summary = take_system_row(
                &lines,
                &mut cursor,
                "stemsbeamvlinkreusechecksummary",
                REUSE_SUMMARY_FIELDS,
                &page,
                system_id,
                Some(plan),
            )?;
            let system = ReuseSystemRows {
                system_id,
                baseline,
                frontier,
                live_stems,
                entries,
                selection,
                check_context,
                check_trace,
                check_result,
                guard,
                summary,
            };
            validate_reuse_system(&system)?;
            systems.push(system);
            if system_id == 1 {
                synthetic = Some(parse_reuse_synthetic(
                    &lines,
                    &mut cursor,
                    &page,
                    system_id,
                    plan,
                )?);
            }
        }
        let synthetic = synthetic.ok_or_else(|| "missing synthetic reuse rows".to_owned())?;
        let page_summary = take_exact_row(
            &lines,
            &mut cursor,
            "stemsbeamvlinkreusecheckpagesummary",
            REUSE_PAGE_SUMMARY_FIELDS,
        )?;
        if page_summary.page != page
            || page_summary.usize("systems")? != systems_count
            || page_summary.usize("transactions")? != systems_count
            || page_summary.usize("syntheticLiveStemRows")? != 2
            || page_summary.usize("syntheticHeadScanRows")? != 3
            || page_summary.usize("syntheticGuardRows")? != 1
            || page_summary.usize("syntheticZeroCases")? != 1
            || page_summary.usize("syntheticUniqueCases")? != 1
            || page_summary.usize("syntheticMultipleCases")? != 1
            || page_summary.usize("syntheticIntersectionCases")? != 1
            || page_summary.usize("sigMutations")? != 0
            || page_summary.usize("relationMutations")? != 0
            || page_summary.usize("linkerFlagMutations")? != 0
        {
            return Err("reuse/check page summary algebra differs".to_owned());
        }
        let corpus_line = lines
            .get(cursor)
            .ok_or_else(|| "missing reuse/check corpus summary".to_owned())?;
        let corpus = parse_exact_pairs(
            corpus_line,
            "stemsbeamvlinkreusecheckcorpussummary",
            REUSE_CORPUS_FIELDS,
        )?;
        cursor += 1;
        if cursor != lines.len() {
            return Err("unexpected trailing reuse/check semantic rows".to_owned());
        }
        validate_reuse_corpus(&page_row, &page_summary, &corpus, systems_count)?;
        Ok(Self {
            page,
            page_row,
            systems,
            synthetic,
            page_summary,
            corpus,
        })
    }
}

fn take_exact_row(
    lines: &[&str],
    cursor: &mut usize,
    family: &str,
    labels: &[&str],
) -> Result<ExactRow, String> {
    let line = lines
        .get(*cursor)
        .ok_or_else(|| format!("missing {family} at semantic row {cursor}"))?;
    let row = parse_exact_row(line, family, labels)?;
    *cursor += 1;
    Ok(row)
}

#[allow(clippy::too_many_arguments)]
fn take_system_row(
    lines: &[&str],
    cursor: &mut usize,
    family: &str,
    labels: &[&str],
    page: &str,
    system_id: usize,
    plan: Option<usize>,
) -> Result<ExactRow, String> {
    let row = take_exact_row(lines, cursor, family, labels)?;
    if row.page != page || row.usize("system")? != system_id {
        return Err(format!("{family} page/system hierarchy differs"));
    }
    if plan.is_some_and(|expected| row.usize("plan") != Ok(expected)) {
        return Err(format!("{family} plan hierarchy differs"));
    }
    Ok(row)
}

#[allow(clippy::too_many_arguments)]
fn take_scoped_check_row(
    lines: &[&str],
    cursor: &mut usize,
    family: &str,
    labels: &[&str],
    page: &str,
    system_id: usize,
    plan: usize,
    scope: &str,
    case: &str,
) -> Result<ExactRow, String> {
    let row = take_system_row(lines, cursor, family, labels, page, system_id, Some(plan))?;
    if row.value("scope")? != scope || row.value("case")? != case {
        return Err(format!("{family} scope/case differs"));
    }
    Ok(row)
}

#[allow(clippy::too_many_arguments)] // Keeps the exact family/page/system/plan/case hierarchy visible.
fn take_synthetic_row(
    lines: &[&str],
    cursor: &mut usize,
    family: &str,
    labels: &[&str],
    page: &str,
    system_id: usize,
    plan: usize,
    case: &str,
) -> Result<ExactRow, String> {
    let row = take_system_row(lines, cursor, family, labels, page, system_id, Some(plan))?;
    if row.value("case")? != case {
        return Err(format!("{family} synthetic case order differs"));
    }
    Ok(row)
}

fn parse_reuse_synthetic(
    lines: &[&str],
    cursor: &mut usize,
    page: &str,
    system_id: usize,
    plan: usize,
) -> Result<ReuseSyntheticRows, String> {
    let mut live_stems = Vec::with_capacity(2);
    for catalog_ordinal in 0..2 {
        let row = take_synthetic_row(
            lines,
            cursor,
            "stemsbeamvlinkreusechecksyntheticlivestem",
            REUSE_SYNTHETIC_LIVE_STEM_FIELDS,
            page,
            system_id,
            plan,
            "SyntheticReuseTargets",
        )?;
        if row.usize("catalogOrdinal")? != catalog_ordinal {
            return Err("synthetic live stem order differs".to_owned());
        }
        live_stems.push(row);
    }

    let zero_entry = take_synthetic_row(
        lines,
        cursor,
        "stemsbeamvlinkreusechecksyntheticreuseentry",
        REUSE_SYNTHETIC_ENTRY_FIELDS,
        page,
        system_id,
        plan,
        "ZeroMissingJavaNull",
    )?;
    let zero_scans = take_synthetic_scans(
        lines,
        cursor,
        page,
        system_id,
        plan,
        "ZeroMissingJavaNull",
        0,
        zero_entry.usize("incidentEdges")?,
    )?;
    let zero_selection = take_synthetic_row(
        lines,
        cursor,
        "stemsbeamvlinkreusechecksyntheticselection",
        REUSE_SYNTHETIC_SELECTION_FIELDS,
        page,
        system_id,
        plan,
        "ZeroMissingJavaNull",
    )?;

    let unique_first = take_synthetic_row(
        lines,
        cursor,
        "stemsbeamvlinkreusechecksyntheticreuseentry",
        REUSE_SYNTHETIC_ENTRY_FIELDS,
        page,
        system_id,
        plan,
        "UniqueBreakUnreadSuffix",
    )?;
    if unique_first.usize("mapOrdinal")? != 0 {
        return Err("synthetic unique first map order differs".to_owned());
    }
    let unique_scans = take_synthetic_scans(
        lines,
        cursor,
        page,
        system_id,
        plan,
        "UniqueBreakUnreadSuffix",
        0,
        unique_first.usize("incidentEdges")?,
    )?;
    let unique_unread = take_synthetic_row(
        lines,
        cursor,
        "stemsbeamvlinkreusechecksyntheticreuseentry",
        REUSE_SYNTHETIC_UNREAD_ENTRY_FIELDS,
        page,
        system_id,
        plan,
        "UniqueBreakUnreadSuffix",
    )?;
    if unique_unread.usize("mapOrdinal")? != 1 {
        return Err("synthetic unique unread map order differs".to_owned());
    }
    let unique_entries = vec![unique_first, unique_unread];
    let unique_check_context = take_scoped_check_row(
        lines,
        cursor,
        "stemsbeamvlinkreusecheckcheckcontext",
        REUSE_CHECK_CONTEXT_FIELDS,
        page,
        system_id,
        plan,
        "synthetic",
        "UniqueBreakUnreadSuffix",
    )?;
    let unique_check_trace = take_scoped_check_row(
        lines,
        cursor,
        "stemsbeamvlinkreusecheckchecktrace",
        REUSE_CHECK_TRACE_FIELDS,
        page,
        system_id,
        plan,
        "synthetic",
        "UniqueBreakUnreadSuffix",
    )?;
    let unique_check_result = take_scoped_check_row(
        lines,
        cursor,
        "stemsbeamvlinkreusecheckcheckresult",
        REUSE_CHECK_RESULT_FIELDS,
        page,
        system_id,
        plan,
        "synthetic",
        "UniqueBreakUnreadSuffix",
    )?;
    let unique_selection = take_synthetic_row(
        lines,
        cursor,
        "stemsbeamvlinkreusechecksyntheticselection",
        REUSE_SYNTHETIC_SELECTION_FIELDS,
        page,
        system_id,
        plan,
        "UniqueBreakUnreadSuffix",
    )?;

    let multiple_entry = take_synthetic_row(
        lines,
        cursor,
        "stemsbeamvlinkreusechecksyntheticreuseentry",
        REUSE_SYNTHETIC_ENTRY_FIELDS,
        page,
        system_id,
        plan,
        "MultipleContinue",
    )?;
    let multiple_scans = take_synthetic_scans(
        lines,
        cursor,
        page,
        system_id,
        plan,
        "MultipleContinue",
        0,
        multiple_entry.usize("incidentEdges")?,
    )?;
    let multiple_check_context = take_scoped_check_row(
        lines,
        cursor,
        "stemsbeamvlinkreusecheckcheckcontext",
        REUSE_CHECK_CONTEXT_FIELDS,
        page,
        system_id,
        plan,
        "synthetic",
        "MultipleContinue",
    )?;
    let multiple_check_trace = take_scoped_check_row(
        lines,
        cursor,
        "stemsbeamvlinkreusecheckchecktrace",
        REUSE_CHECK_TRACE_FIELDS,
        page,
        system_id,
        plan,
        "synthetic",
        "MultipleContinue",
    )?;
    let multiple_check_result = take_scoped_check_row(
        lines,
        cursor,
        "stemsbeamvlinkreusecheckcheckresult",
        REUSE_CHECK_RESULT_FIELDS,
        page,
        system_id,
        plan,
        "synthetic",
        "MultipleContinue",
    )?;
    let multiple_selection = take_synthetic_row(
        lines,
        cursor,
        "stemsbeamvlinkreusechecksyntheticselection",
        REUSE_SYNTHETIC_SELECTION_FIELDS,
        page,
        system_id,
        plan,
        "MultipleContinue",
    )?;

    let mut portions = Vec::with_capacity(4);
    for case in ["LeftBelow", "LeftExact", "RightExact", "RightAbove"] {
        portions.push(take_synthetic_row(
            lines,
            cursor,
            "stemsbeamvlinkreusechecksyntheticportion",
            REUSE_SYNTHETIC_PORTION_FIELDS,
            page,
            system_id,
            plan,
            case,
        )?);
    }
    let threshold = take_system_row(
        lines,
        cursor,
        "stemsbeamvlinkreusechecksyntheticthreshold",
        REUSE_SYNTHETIC_THRESHOLD_FIELDS,
        page,
        system_id,
        Some(plan),
    )?;
    let parallel_check_context = take_scoped_check_row(
        lines,
        cursor,
        "stemsbeamvlinkreusecheckcheckcontext",
        REUSE_CHECK_CONTEXT_FIELDS,
        page,
        system_id,
        plan,
        "synthetic",
        "DistinctParallelNonFinite",
    )?;
    let parallel_check_trace = take_scoped_check_row(
        lines,
        cursor,
        "stemsbeamvlinkreusecheckchecktrace",
        REUSE_CHECK_TRACE_FIELDS,
        page,
        system_id,
        plan,
        "synthetic",
        "DistinctParallelNonFinite",
    )?;
    let parallel_check_result = take_scoped_check_row(
        lines,
        cursor,
        "stemsbeamvlinkreusecheckcheckresult",
        REUSE_CHECK_RESULT_FIELDS,
        page,
        system_id,
        plan,
        "synthetic",
        "DistinctParallelNonFinite",
    )?;
    let parallel = take_system_row(
        lines,
        cursor,
        "stemsbeamvlinkreusechecksyntheticparallel",
        REUSE_SYNTHETIC_PARALLEL_FIELDS,
        page,
        system_id,
        Some(plan),
    )?;
    let intersection = take_synthetic_row(
        lines,
        cursor,
        "stemsbeamvlinkreusechecksyntheticintersection",
        REUSE_SYNTHETIC_INTERSECTION_FIELDS,
        page,
        system_id,
        plan,
        "DistinctHorizontalInfNaN",
    )?;
    let guard = take_system_row(
        lines,
        cursor,
        "stemsbeamvlinkreusechecksyntheticguard",
        REUSE_SYNTHETIC_GUARD_FIELDS,
        page,
        system_id,
        Some(plan),
    )?;
    let synthetic = ReuseSyntheticRows {
        live_stems,
        zero_entry,
        zero_scans,
        zero_selection,
        unique_entries,
        unique_scans,
        unique_check_context,
        unique_check_trace,
        unique_check_result,
        unique_selection,
        multiple_entry,
        multiple_scans,
        multiple_check_context,
        multiple_check_trace,
        multiple_check_result,
        multiple_selection,
        portions,
        threshold,
        parallel_check_context,
        parallel_check_trace,
        parallel_check_result,
        parallel,
        intersection,
        guard,
    };
    validate_reuse_synthetic(&synthetic)?;
    Ok(synthetic)
}

#[allow(clippy::too_many_arguments)] // Preserves both Java ordinal domains at the row boundary.
fn take_synthetic_scans(
    lines: &[&str],
    cursor: &mut usize,
    page: &str,
    system_id: usize,
    plan: usize,
    case: &str,
    map_ordinal: usize,
    count: usize,
) -> Result<Vec<ExactRow>, String> {
    let mut scans = Vec::with_capacity(count);
    for scan_ordinal in 0..count {
        let row = take_synthetic_row(
            lines,
            cursor,
            "stemsbeamvlinkreusechecksyntheticheadstemscan",
            REUSE_SYNTHETIC_HEAD_SCAN_FIELDS,
            page,
            system_id,
            plan,
            case,
        )?;
        if row.usize("mapOrdinal")? != map_ordinal
            || row.usize("scanOrdinal")? != scan_ordinal
            || row.value("scanOrderDomain")? != "HeadInterRelationsSourceOrder"
            || row.value("sigEdgeIdentityDomain")? != "IsolatedSigEdgeSetSourceOrder"
        {
            return Err("synthetic head scan ordinal/domain differs".to_owned());
        }
        scans.push(row);
    }
    Ok(scans)
}

fn validate_reuse_system(system: &ReuseSystemRows) -> Result<(), String> {
    let system_id = system.system_id;
    let expected_mode = if system_id == 1 {
        "sheet-first"
    } else {
        "isolated-system-frontier"
    };
    let plan = system.baseline.usize("plan")?;
    if system.baseline.value("executionMode")? != expected_mode
        || system.frontier.usize("plan")? != plan
        || system.frontier.value("predecessorJoin")? != "Exact"
        || system.frontier.usize("liveStemCount")? != system.live_stems.len()
        || system.frontier.usize("relationCount")? != system.entries.len()
    {
        return Err(format!("system {system_id} reuse frontier algebra differs"));
    }
    for label in [
        "glyphActiveHash",
        "glyphOriginalsHash",
        "interIndexHash",
        "sigHash",
        "sigRelationStateHash",
        "systemStemsHash",
        "linkerStateHash",
        "lineStateHash",
        "relationInputHash",
    ] {
        parse_lower_hex(system.baseline.value(label)?, 64, label)?;
    }
    parse_lower_hex(
        system.frontier.value("liveStemCatalogueHash")?,
        64,
        "live stem catalogue hash",
    )?;

    // The frozen real first-frontier corpus currently exercises the ordinary
    // all-unlinked/no-reuse path. Reuse remains mandatory in native synthetics.
    if !system.live_stems.is_empty() {
        return Err(format!(
            "system {system_id} unexpectedly entered real live-stem reuse corpus"
        ));
    }
    let relation_aliases = parse_list(system.frontier.value("relationAliases")?)?;
    if relation_aliases.len() != system.entries.len() {
        return Err(format!("system {system_id} relation alias count differs"));
    }
    for (ordinal, rows) in system.entries.iter().enumerate() {
        let entry = &rows.entry;
        if entry.value("cAlias")? != relation_aliases[ordinal]
            || entry.value("conditionRead")? != "true"
            || entry.value("evidenceValidated")? != "true"
            || entry.value("sLinked")? != "false"
            || entry.value("lookupState")? != "NotRead"
            || entry.value("scanState")? != "NotRead"
            || entry.value("incidentEdges")? != "-"
            || entry.value("matchingEdges")? != "-"
            || entry.value("distinctSideStems")? != "-"
            || entry.value("headSnapshotHash")? != "-"
            || entry.value("projectionHash")? != "-"
            || entry.value("sideStemAliases")? != "-"
            || entry.value("selectedStemAlias")? != "-"
            || entry.value("action")? != "SkipUnlinked"
            || !rows.scans.is_empty()
        {
            return Err(format!("system {system_id} real reuse entry differs"));
        }
    }
    let selection = &system.selection;
    if selection.value("initialStemAlias")? != selection.value("finalStemAlias")?
        || selection.value("initialStemInterId")? != "0"
        || selection.value("finalStemInterId")? != "0"
        || selection.value("reused")? != "false"
        || selection.value("selectedMapOrdinal")? != "-"
        || selection.value("selectedC")? != "-"
        || selection.value("breakIndex")? != "-"
        || selection.usize("entriesTotal")? != system.entries.len()
        || selection.usize("entriesConditionRead")? != system.entries.len()
        || selection.usize("unreadSuffix")? != 0
        || selection.usize("linkedEntries")? != 0
        || selection.value("allLinked")? != "false"
        || selection.value("reuseOutcome")? != "AllUnlinked"
        || selection.value("terminal")? != "ContinueToCheck"
        || selection.value("finalAbnormal")? != "false"
        || selection.value("finalSigAttached")? != "false"
    {
        return Err(format!("system {system_id} selection algebra differs"));
    }
    parse_lower_hex(
        selection.value("finalStateHash")?,
        64,
        "final stem state hash",
    )?;
    validate_check_triplet(
        &system.check_context,
        &system.check_trace,
        &system.check_result,
    )?;
    validate_reuse_guard(&system.baseline, &system.frontier, &system.guard)?;
    if system.summary.value("transaction")? != "HeadSideReuseThenBeamStemCheck"
        || system.summary.usize("relationEntries")? != system.entries.len()
        || system.summary.usize("linkedEntries")? != 0
        || system.summary.usize("sideScans")? != 0
        || system.summary.value("reused")? != "false"
        || system.summary.value("reuseOutcome")? != "AllUnlinked"
        || system.summary.usize("realReuseCensus")? != 0
        || system.summary.value("checkOutcome")? != system.check_result.value("checkOutcome")?
        || system.summary.value("terminal")? != system.check_result.value("terminal")?
    {
        return Err(format!("system {system_id} summary algebra differs"));
    }
    Ok(())
}

fn validate_check_triplet(
    context: &ExactRow,
    trace: &ExactRow,
    result: &ExactRow,
) -> Result<(), String> {
    if context.page != trace.page
        || context.page != result.page
        || context.value("system")? != trace.value("system")?
        || context.value("system")? != result.value("system")?
        || context.value("plan")? != trace.value("plan")?
        || context.value("plan")? != result.value("plan")?
        || context.value("scope")? != trace.value("scope")?
        || context.value("scope")? != result.value("scope")?
        || context.value("case")? != trace.value("case")?
        || context.value("case")? != result.value("case")?
        || context.value("stemMedian")? == "-"
        || context.value("beamMedian")? == "-"
        || context.value("beamBorder")? == "-"
        || context.value("portionComparison")? != "StrictLtGt"
        || context.value("gradeComparison")? != "InclusiveGe"
    {
        return Err("reuse relation check row join differs".to_owned());
    }
    for label in [
        "beamHeight",
        "stemActualWidth",
        "halfScaleStemWidth",
        "portionXInP0",
        "xOutMax",
        "yMax",
        "maxDxInput",
        "maxDxRint",
        "xWeight",
        "yWeight",
        "intrinsicRatio",
        "minGrade",
    ] {
        parse_hex_double(context.value(label)?, label)?;
    }
    for label in [
        "intersectionDen",
        "v12",
        "v34",
        "xNumerator",
        "yNumerator",
        "crossX",
        "crossY",
        "leftThreshold",
        "rightThreshold",
        "signedXGapPixels",
        "signedDxFrac",
        "storedDx",
        "upperGapRaw",
        "upperGap",
        "lowerGapRaw",
        "lowerGap",
        "yGapPixels",
        "storedDy",
        "xImpactRaw",
        "yImpactRaw",
        "xImpact",
        "yImpact",
        "xPow",
        "yPow",
        "global",
        "totalWeight",
        "reciprocalWeight",
        "root",
        "intrinsicRatio",
        "grade",
    ] {
        parse_hex_double(trace.value(label)?, label)?;
    }
    let accepted = trace.value("accepted")?;
    if !matches!(accepted, "true" | "false") {
        return Err("relation trace accepted domain differs".to_owned());
    }
    match result.value("checkOutcome")? {
        "Accepted"
            if accepted == "true"
                && result.value("linkNull")? == "false"
                && result.value("outgoing")? == "true"
                && result.value("relationClass")? == "BeamStemRelation"
                && result.value("terminal")? == "ReadyBeforeSigMutation" =>
        {
            for label in ["dx", "dy", "grade"] {
                parse_hex_double(result.value(label)?, label)?;
            }
        }
        "Rejected"
            if accepted == "false"
                && result.value("linkNull")? == "true"
                && result.value("partnerAlias")? == "-"
                && result.value("partnerInterId")? == "-"
                && result.value("outgoing")? == "-"
                && result.value("relationClass")? == "-"
                && result.value("terminal")? == "BeamStemRejected" => {}
        _ => return Err("relation result/trace terminal differs".to_owned()),
    }
    Ok(())
}

fn validate_reuse_guard(
    baseline: &ExactRow,
    frontier: &ExactRow,
    guard: &ExactRow,
) -> Result<(), String> {
    for (before, after) in [
        ("allocatorBefore", "allocatorAfter"),
        ("glyphActiveHashBefore", "glyphActiveHashAfter"),
        ("glyphOriginalsHashBefore", "glyphOriginalsHashAfter"),
        ("interIndexHashBefore", "interIndexHashAfter"),
        ("sigHashBefore", "sigHashAfter"),
        ("sigRelationStateHashBefore", "sigRelationStateHashAfter"),
        ("systemStemsHashBefore", "systemStemsHashAfter"),
        ("linkerStateHashBefore", "linkerStateHashAfter"),
        ("lineStateHashBefore", "lineStateHashAfter"),
        ("liveStemCatalogueHashBefore", "liveStemCatalogueHashAfter"),
        ("relationInputHashBefore", "relationInputHashAfter"),
    ] {
        if guard.value(before)? != guard.value(after)? {
            return Err(format!("reuse guard {before}/{after} differs"));
        }
    }
    for (guard_label, baseline_label) in [
        ("allocatorBefore", "allocator"),
        ("glyphActiveHashBefore", "glyphActiveHash"),
        ("glyphOriginalsHashBefore", "glyphOriginalsHash"),
        ("interIndexHashBefore", "interIndexHash"),
        ("sigHashBefore", "sigHash"),
        ("sigRelationStateHashBefore", "sigRelationStateHash"),
        ("systemStemsHashBefore", "systemStemsHash"),
        ("linkerStateHashBefore", "linkerStateHash"),
        ("lineStateHashBefore", "lineStateHash"),
        ("relationInputHashBefore", "relationInputHash"),
    ] {
        if guard.value(guard_label)? != baseline.value(baseline_label)? {
            return Err(format!("reuse guard/baseline {guard_label} differs"));
        }
    }
    if guard.value("liveStemCatalogueHashBefore")? != frontier.value("liveStemCatalogueHash")? {
        return Err("reuse guard/frontier live catalogue differs".to_owned());
    }
    for label in [
        "createStemStateUnchanged",
        "registriesUnchanged",
        "allocatorUnchanged",
        "linesUnchanged",
        "systemStemsUnchanged",
        "liveStemsUnchanged",
        "interIndexUnchanged",
        "sigUnchanged",
        "relationsUnchanged",
        "linkerFlagsUnchanged",
        "stopBeforeSigAddVertex",
        "stopBeforeRelationApply",
        "stopBeforeLinkerFlagMutation",
    ] {
        if guard.value(label)? != "true" {
            return Err(format!("reuse guard {label} is not true"));
        }
    }
    Ok(())
}

fn validate_reuse_synthetic(synthetic: &ReuseSyntheticRows) -> Result<(), String> {
    if synthetic.live_stems.len() != 2 {
        return Err("synthetic live stem cardinality differs".to_owned());
    }
    for (ordinal, stem) in synthetic.live_stems.iter().enumerate() {
        if stem.usize("catalogOrdinal")? != ordinal
            || stem.value("origin")? != "IsolatedSyntheticSig"
            || stem.value("idNamespace")? != "isolated-1500000000"
            || stem.value("stemAlias")? != format!("synthetic-live:{ordinal}")
            || stem
                .value("javaInterId")?
                .parse::<i32>()
                .ok()
                .filter(|id| *id > 0)
                .is_none()
            || stem
                .value("glyphId")?
                .parse::<i32>()
                .ok()
                .filter(|id| *id > 0)
                .is_none()
            || stem.value("sigAttached")? != "true"
            || stem.value("glyph")? == "-"
            || stem.value("median")? == "-"
            || stem.value("bounds")? == "-"
        {
            return Err("synthetic live SIG stem payload differs".to_owned());
        }
        parse_hex_double(stem.value("grade")?, "synthetic stem grade")?;
        parse_hex_double(stem.value("width")?, "synthetic stem width")?;
        parse_lower_hex(stem.value("stateHash")?, 64, "synthetic stem state hash")?;
    }
    if synthetic.live_stems[0].value("stemAlias")? == synthetic.live_stems[1].value("stemAlias")?
        || synthetic.live_stems[0].value("javaInterId")?
            == synthetic.live_stems[1].value("javaInterId")?
        || synthetic.live_stems[0].value("glyphId")? != synthetic.live_stems[1].value("glyphId")?
        || synthetic.live_stems[0].value("glyph")? != synthetic.live_stems[1].value("glyph")?
    {
        return Err("synthetic StemInter-vs-Glyph identity evidence differs".to_owned());
    }
    if synthetic.zero_entry.usize("mapOrdinal")? != 0
        || !synthetic.zero_scans.is_empty()
        || synthetic.zero_entry.value("isolatedSig")? != "true"
        || synthetic.zero_entry.value("scanState")? != "Exhaustive"
        || synthetic.zero_entry.usize("incidentEdges")? != 0
        || synthetic.zero_entry.value("lookupState")? != "MissingJavaNull"
        || synthetic.zero_entry.value("action")? != "JavaNullPointer"
        || synthetic.zero_selection.value("origin")? != "IsolatedSyntheticSig"
        || synthetic.zero_selection.value("cardinalityModel")? != "ActualHeadInterGetSideStems"
        || synthetic.zero_selection.value("cardinality")? != "0"
        || synthetic.zero_selection.value("outcome")? != "JavaNullPointer"
        || synthetic.zero_selection.value("checkOutcome")? != "NotReachedJavaNull"
        || synthetic.zero_selection.value("zeroMutation")? != "true"
    {
        return Err("synthetic missing-side Java-null case differs".to_owned());
    }
    if synthetic.unique_entries.len() != 2
        || synthetic.unique_scans.len() != 1
        || synthetic.unique_entries[0].value("action")? != "SelectBreak"
        || synthetic.unique_entries[1].value("conditionRead")? != "false"
        || synthetic.unique_entries[1].value("evidenceValidated")? != "false"
        || synthetic.unique_entries[1].value("action")? != "UnreadAfterBreak"
        || synthetic.unique_selection.value("origin")? != "IsolatedSyntheticSig"
        || synthetic.unique_selection.value("cardinalityModel")? != "ActualHeadInterGetSideStems"
        || synthetic.unique_selection.value("cardinality")? != "1"
        || synthetic.unique_selection.value("outcome")? != "Selected"
        || synthetic.unique_selection.usize("unreadSuffix")? != 1
        || synthetic.unique_selection.value("finalSigAttached")? != "true"
        || synthetic.unique_selection.value("zeroMutation")? != "true"
    {
        return Err("synthetic first-unique/break case differs".to_owned());
    }
    validate_check_triplet(
        &synthetic.unique_check_context,
        &synthetic.unique_check_trace,
        &synthetic.unique_check_result,
    )?;
    if synthetic.multiple_entry.usize("mapOrdinal")? != 0
        || synthetic.multiple_scans.len() != 2
        || synthetic.multiple_entry.value("sideStemCount")? != "2"
        || synthetic.multiple_entry.value("action")? != "ContinueMultiple"
        || synthetic.multiple_selection.value("origin")? != "IsolatedSyntheticSig"
        || synthetic.multiple_selection.value("cardinalityModel")? != "ActualHeadInterGetSideStems"
        || synthetic.multiple_selection.value("cardinality")? != "multiple"
        || synthetic.multiple_selection.value("outcome")? != "NoUnique"
        || synthetic.multiple_selection.value("zeroMutation")? != "true"
    {
        return Err("synthetic multiple/continue case differs".to_owned());
    }
    validate_check_triplet(
        &synthetic.multiple_check_context,
        &synthetic.multiple_check_trace,
        &synthetic.multiple_check_result,
    )?;
    for (row, expected) in synthetic
        .portions
        .iter()
        .zip(["LEFT", "CENTER", "CENTER", "RIGHT"])
    {
        if row.value("comparison")? != "StrictLtGt" || row.value("portion")? != expected {
            return Err("synthetic beam-portion strict boundary differs".to_owned());
        }
        for label in ["xStem", "leftThreshold", "rightThreshold"] {
            parse_hex_double(row.value(label)?, label)?;
        }
    }
    if synthetic.threshold.value("grade")? != synthetic.threshold.value("minGrade")?
        || synthetic.threshold.value("comparison")? != "InclusiveGe"
        || synthetic.threshold.value("accepted")? != "true"
    {
        return Err("synthetic inclusive grade threshold differs".to_owned());
    }
    validate_check_triplet(
        &synthetic.parallel_check_context,
        &synthetic.parallel_check_trace,
        &synthetic.parallel_check_result,
    )?;
    if synthetic.parallel.value("checkOutcome")? != "Rejected"
        || synthetic.parallel.value("zeroMutation")? != "true"
        || synthetic.parallel_check_result.value("checkOutcome")? != "Rejected"
    {
        return Err("synthetic parallel rejection differs".to_owned());
    }
    for label in ["den", "crossX", "crossY", "grade"] {
        parse_hex_double(synthetic.parallel.value(label)?, label)?;
    }
    if synthetic.intersection.value("finiteInputs")? != "true"
        || synthetic.intersection.value("exactLineUtil")? != "true"
        || !parse_hex_double(synthetic.intersection.value("crossX")?, "intersection x")?
            .is_infinite()
        || !parse_hex_double(synthetic.intersection.value("crossY")?, "intersection y")?.is_nan()
    {
        return Err("synthetic Inf/NaN determinant grouping differs".to_owned());
    }
    let mut relation_identities = BTreeSet::new();
    for scan in synthetic
        .unique_scans
        .iter()
        .chain(&synthetic.multiple_scans)
    {
        let relation_identity = scan.usize("sigEdgeOrdinal")?;
        if !relation_identities.insert(relation_identity)
            || scan.value("targetSigAttached")? != "true"
            || scan
                .value("targetStemInterId")?
                .parse::<i32>()
                .ok()
                .filter(|id| *id > 0)
                .is_none()
        {
            return Err("synthetic global relation/stem identity differs".to_owned());
        }
    }
    for (before, after) in [
        ("isolatedSigHashBefore", "isolatedSigHashAfter"),
        ("liveStemHashBefore", "liveStemHashAfter"),
        ("realAllocatorBefore", "realAllocatorAfter"),
        ("realInterIndexHashBefore", "realInterIndexHashAfter"),
        ("realSigHashBefore", "realSigHashAfter"),
        (
            "realSigRelationStateHashBefore",
            "realSigRelationStateHashAfter",
        ),
        ("realSystemStemsHashBefore", "realSystemStemsHashAfter"),
        ("realLinkerStateHashBefore", "realLinkerStateHashAfter"),
        ("realLineStateHashBefore", "realLineStateHashAfter"),
    ] {
        if synthetic.guard.value(before)? != synthetic.guard.value(after)? {
            return Err(format!("synthetic guard {before}/{after} differs"));
        }
    }
    if synthetic.guard.value("origin")? != "IsolatedSyntheticSig" {
        return Err("synthetic guard origin differs".to_owned());
    }
    for field in [
        "isolatedGraphUnchanged",
        "realSheetUnchanged",
        "allocatorUnchanged",
        "interIndexUnchanged",
        "sigUnchanged",
        "relationsUnchanged",
        "linkerFlagsUnchanged",
        "zeroMutation",
    ] {
        if synthetic.guard.value(field)? != "true" {
            return Err(format!("synthetic guard {field} differs"));
        }
    }
    Ok(())
}

fn validate_reuse_corpus(
    page: &ExactRow,
    page_summary: &ExactRow,
    corpus: &BTreeMap<String, String>,
    systems_count: usize,
) -> Result<(), String> {
    if corpus.get("schema").map(String::as_str) != Some("stems-beam-vlink-reuse-check-v1")
        || corpus.get("pages").map(String::as_str) != Some("1")
        || corpus.get("pageRefs").map(String::as_str) != Some(page.page.as_str())
        || corpus.get("pairedFixtureSwapNegative").map(String::as_str) != Some("true")
        || corpus.get("freshRunsPerPage").map(String::as_str) != Some("2")
        || corpus.get("freshRunsByteIdentical").map(String::as_str) != Some("true")
        || corpus.get("freshJvmPerPage").map(String::as_str) != Some("false")
        || corpus.get("freshJvmPerSystem").map(String::as_str) != Some("true")
        || corpus
            .get("runtimeJavaProcessesPerPass")
            .map(String::as_str)
            != Some(systems_count.to_string().as_str())
        || corpus
            .get("maximumConcurrentJavaProcesses")
            .map(String::as_str)
            != Some("1")
        || corpus.get("freshSheetPerSystem").map(String::as_str) != Some("true")
        || corpus.get("compilerJavaProcessReaped").map(String::as_str) != Some("true")
        || corpus.get("runnerJavaProcessesReaped").map(String::as_str) != Some("true")
        || corpus
            .get("foregroundJavaProcessesOnly")
            .map(String::as_str)
            != Some("true")
        || corpus
            .get("backgroundJavaProcessesStarted")
            .map(String::as_str)
            != Some("0")
        || corpus.get("realReuseCensus").map(String::as_str) != Some("0")
        || corpus.get("schedulerFixtureSha256") != page.fields.get("schedulerFixtureSha256")
        || corpus.get("expandFixtureSha256") != page.fields.get("expandFixtureSha256")
        || corpus.get("createStemFixtureSha256") != page.fields.get("createStemFixtureSha256")
        || page_summary.usize("realReuses")? != 0
    {
        return Err("reuse/check corpus/page provenance differs".to_owned());
    }
    for label in REUSE_CORPUS_FIELDS
        .iter()
        .copied()
        .filter(|label| label.ends_with("Sha256"))
    {
        parse_lower_hex(
            corpus
                .get(label)
                .ok_or_else(|| format!("reuse/check corpus lacks {label}"))?,
            64,
            label,
        )?;
    }
    Ok(())
}

fn validate_create_stem_system(system: &CreateStemSystemRows) -> Result<(), String> {
    let baseline = system.row("stemsbeamcreatestembaseline")?;
    let frontier = system.row("stemsbeamcreatestemfrontier")?;
    let expand = system.row("stemsbeamcreatestemexpand")?;
    let lookup = system.row("stemsbeamcreatestemlookup")?;
    let result = system.row("stemsbeamcreatestemresult")?;
    let delta = system.row("stemsbeamcreatestemdelta")?;
    let guard = system.row("stemsbeamcreatestemguard")?;
    let summary = system.row("stemsbeamcreatestemsummary")?;
    if frontier.value("plan")? != expand.value("plan")?
        || frontier.value("plan")? != result.value("plan")?
        || lookup.value("candidate")? != result.value("candidate")?
        || lookup.value("systemStemLookup")? != "Absent"
        || lookup.value("systemStemInterId")? != "-"
        || lookup.value("systemStemGrade")? != "-"
        || lookup.value("certificate")? != "ExhaustiveGlyphEqualsScan"
        || lookup.value("systemStemCertificate")? != "ExhaustiveSystemStemEqualsScan"
        || lookup.value("aliasOrder")? != "JavaGlyphId"
        || result.value("postAliasOrder")? != "JavaGlyphId"
        || result.value("registeredAlias")? != delta.value("registeredAlias")?
        || result.value("registration")? != summary.value("registration")?
        || result.value("disposition")? != summary.value("disposition")?
        || summary.value("transaction")? != "CreateStemOnly"
        || baseline.value("glyphActiveHash")? != lookup.value("activeHash")?
        || baseline.value("glyphOriginalsHash")? != lookup.value("originalsHash")?
        || baseline.value("systemStemsHash")? != lookup.value("systemStemsHash")?
    {
        return Err(format!(
            "createStem system {} row algebra differs",
            system.system_id
        ));
    }
    for label in [
        "lineDeltaRetained",
        "interIndexUnchanged",
        "sigUnchanged",
        "relationsUnchanged",
        "linkerFlagsUnchanged",
        "stopBeforeVReuse",
        "stopBeforeBeamStemCheck",
    ] {
        if guard.value(label)? != "true" {
            return Err(format!(
                "createStem system {} guard {label} differs",
                system.system_id
            ));
        }
    }
    Ok(())
}

fn parse_exact_row(line: &str, family: &str, labels: &[&str]) -> Result<ExactRow, String> {
    let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
    if tokens.first().copied() != Some(family) || tokens.len() != 2 + (2 * labels.len()) {
        return Err(format!("malformed {family} row"));
    }
    let page = tokens[1];
    if page.is_empty() {
        return Err(format!("{family} row has empty page"));
    }
    let observed = tokens[2..]
        .chunks_exact(2)
        .map(|pair| pair[0])
        .collect::<Vec<_>>();
    if observed != labels {
        return Err(format!("{family} labels differ: {observed:?}"));
    }
    let fields = labels
        .iter()
        .zip(tokens[2..].chunks_exact(2))
        .map(|(label, pair)| ((*label).to_owned(), pair[1].to_owned()))
        .collect();
    Ok(ExactRow {
        family: family.to_owned(),
        page: page.to_owned(),
        fields,
    })
}

fn parse_exact_pairs(
    line: &str,
    family: &str,
    labels: &[&str],
) -> Result<BTreeMap<String, String>, String> {
    let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
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
    Ok(labels
        .iter()
        .zip(tokens[1..].chunks_exact(2))
        .map(|(label, pair)| ((*label).to_owned(), pair[1].to_owned()))
        .collect())
}

fn parse_lower_hex<'a>(value: &'a str, length: usize, label: &str) -> Result<&'a str, String> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("invalid lowercase {label}: {value}"));
    }
    Ok(value)
}

fn parse_hex_double(value: &str, label: &str) -> Result<f64, String> {
    let (java, bits) = value
        .split_once('/')
        .ok_or_else(|| format!("{label} lacks Java-hex/raw-bits pair: {value}"))?;
    let bits = u64::from_str_radix(parse_lower_hex(bits, 16, label)?, 16)
        .map_err(|_| format!("{label} raw bits differ: {value}"))?;
    let parsed = match java {
        "NaN" => f64::NAN,
        "Infinity" => f64::INFINITY,
        "-Infinity" => f64::NEG_INFINITY,
        _ => parse_java_hex_float(java)
            .ok_or_else(|| format!("{label} Java hex float differs: {value}"))?,
    };
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
        result += f64::from((digit as char).to_digit(16)?) * place;
        place /= 16.0;
    }
    result *= 2.0_f64.powi(exponent);
    Some(if negative { -result } else { result })
}

fn parse_line(value: &str, label: &str) -> Result<NativeStemLine, String> {
    let coordinates = value.split(':').collect::<Vec<_>>();
    if coordinates.len() != 4 {
        return Err(format!("{label} line does not have four coordinates"));
    }
    Ok(NativeStemLine {
        start: NativeStemPoint {
            x: parse_hex_double(coordinates[0], label)?,
            y: parse_hex_double(coordinates[1], label)?,
        },
        stop: NativeStemPoint {
            x: parse_hex_double(coordinates[2], label)?,
            y: parse_hex_double(coordinates[3], label)?,
        },
    })
}

fn parse_point(value: &str, label: &str) -> Result<NativeStemPoint, String> {
    let coordinates = value.split(':').collect::<Vec<_>>();
    if coordinates.len() != 2 {
        return Err(format!("{label} point does not have two coordinates"));
    }
    Ok(NativeStemPoint {
        x: parse_hex_double(coordinates[0], label)?,
        y: parse_hex_double(coordinates[1], label)?,
    })
}

fn parse_list(value: &str) -> Result<Vec<&str>, String> {
    if value == "-" {
        return Ok(Vec::new());
    }
    let body = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| format!("invalid list token: {value}"))?;
    if body.is_empty() {
        return Err("empty lists use '-' sentinel".to_owned());
    }
    Ok(body.split(',').collect())
}

fn parse_bounds(value: &str) -> Result<Bounds, String> {
    let values = value
        .split(':')
        .map(|field| field.parse::<usize>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| format!("invalid bounds: {value}"))?;
    let [x, y, width, height]: [usize; 4] = values
        .try_into()
        .map_err(|_| format!("bounds lacks four fields: {value}"))?;
    if width == 0 || height == 0 {
        return Err(format!("empty bounds: {value}"));
    }
    Ok(Bounds {
        x,
        y,
        width,
        height,
    })
}

fn parse_rectangle(value: &str) -> Result<JavaRectangle, String> {
    let values = value
        .split(':')
        .map(|field| field.parse::<i32>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| format!("invalid Java rectangle: {value}"))?;
    let [x, y, width, height]: [i32; 4] = values
        .try_into()
        .map_err(|_| format!("Java rectangle lacks four fields: {value}"))?;
    if width <= 0 || height <= 0 {
        return Err(format!("empty Java rectangle: {value}"));
    }
    Ok(JavaRectangle {
        x,
        y,
        width,
        height,
    })
}

fn parse_run_table(value: &str) -> Result<RunTable, String> {
    let (orientation, remainder) = value
        .split_once(':')
        .ok_or_else(|| format!("run table lacks orientation: {value}"))?;
    let orientation = match orientation {
        "HORIZONTAL" => Orientation::Horizontal,
        "VERTICAL" => Orientation::Vertical,
        _ => return Err(format!("run table orientation differs: {value}")),
    };
    let (dimensions, sequences) = remainder
        .split_once(':')
        .ok_or_else(|| format!("run table lacks dimensions: {value}"))?;
    let (width, height) = dimensions
        .split_once('x')
        .ok_or_else(|| format!("run table dimensions differ: {value}"))?;
    let width = width
        .parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("run table width differs: {value}"))?;
    let height = height
        .parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("run table height differs: {value}"))?;
    let body = sequences
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| format!("run table sequences differ: {value}"))?;
    let sequence_tokens = body.split(';').collect::<Vec<_>>();
    let expected_sequences = match orientation {
        Orientation::Horizontal => height,
        Orientation::Vertical => width,
    };
    if sequence_tokens.len() != expected_sequences {
        return Err(format!("run table sequence coverage differs: {value}"));
    }
    let mut pixels = vec![BACKGROUND; width * height];
    let coordinate_limit = match orientation {
        Orientation::Horizontal => width,
        Orientation::Vertical => height,
    };
    for (expected_sequence, token) in sequence_tokens.into_iter().enumerate() {
        let (sequence, runs) = token
            .split_once('=')
            .ok_or_else(|| format!("run table sequence lacks '=': {value}"))?;
        if sequence.parse::<usize>().ok() != Some(expected_sequence) {
            return Err(format!("run table sequence order differs: {value}"));
        }
        if runs == "-" {
            continue;
        }
        let mut prior_stop = None;
        for run in runs.split(',') {
            let (start, length) = run
                .split_once(':')
                .ok_or_else(|| format!("run table run differs: {value}"))?;
            let start = start
                .parse::<usize>()
                .map_err(|_| format!("run table start differs: {value}"))?;
            let length = length
                .parse::<usize>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| format!("run table length differs: {value}"))?;
            let stop = start
                .checked_add(length - 1)
                .filter(|stop| *stop < coordinate_limit)
                .ok_or_else(|| format!("run table run out of bounds: {value}"))?;
            if prior_stop.is_some_and(|prior| start <= prior + 1) {
                return Err(format!("run table runs overlap/touch: {value}"));
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
        .map_err(|error| format!("run table reconstruction failed: {error}"))?;
    if run_table_token(&table) != value {
        return Err(format!("run table token is non-canonical: {value}"));
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
        write!(result, "{sequence}=").expect("writing to String cannot fail");
        let runs = table.sequence(sequence).unwrap_or_default();
        if runs.is_empty() {
            result.push('-');
        } else {
            for (ordinal, run) in runs.iter().enumerate() {
                if ordinal > 0 {
                    result.push(',');
                }
                write!(result, "{}:{}", run.start, run.length)
                    .expect("writing to String cannot fail");
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
    active: bool,
    glyph: GlyphEvidence,
}

fn parse_glyph_evidence(value: &str) -> Result<GlyphEvidence, String> {
    let fields = value.split(':').collect::<Vec<_>>();
    if fields.len() != 6 || fields[0] != "g" {
        return Err(format!("invalid glyph token: {value}"));
    }
    Ok(GlyphEvidence {
        bounds: parse_bounds(&fields[1..5].join(":"))?,
        run_sha256: parse_lower_hex(fields[5], 64, "glyph run SHA-256")?.to_owned(),
    })
}

fn parse_selected_glyph(value: &str) -> Result<SelectedGlyphEvidence, String> {
    let fields = value.split(':').collect::<Vec<_>>();
    if fields.len() != 10 || fields[0] != "glyph" || fields[4] != "g" {
        return Err(format!("invalid selected glyph: {value}"));
    }
    let alias = fields[1]
        .parse::<usize>()
        .map_err(|_| format!("selected glyph alias differs: {value}"))?;
    let active = match fields[2] {
        "active" => true,
        "original-only" => false,
        _ => return Err(format!("selected glyph membership differs: {value}")),
    };
    let glyph_id = fields[3]
        .strip_prefix("id=")
        .and_then(|field| field.parse::<i32>().ok())
        .filter(|id| *id > 0)
        .ok_or_else(|| format!("selected glyph ID differs: {value}"))?;
    if usize::try_from(glyph_id).ok() != Some(alias) {
        return Err(format!("selected alias/ID differs: {value}"));
    }
    Ok(SelectedGlyphEvidence {
        alias,
        glyph_id,
        active,
        glyph: parse_glyph_evidence(&fields[4..].join(":"))?,
    })
}

fn parse_glyph_alias(value: &str) -> Result<usize, String> {
    value
        .strip_prefix("glyph:")
        .and_then(|field| field.parse::<usize>().ok())
        .filter(|alias| *alias > 0)
        .ok_or_else(|| format!("invalid glyph alias: {value}"))
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
                "system {} lacks mutable flattened plan {plan_ordinal}",
                system.system_id
            )
        })
}

fn independent_candidate(
    attempt: &NativeStemsBeamLinkPlanAttempt,
) -> Result<NativeStemsBeamFixedGlyphContent, String> {
    let first = attempt
        .glyphs
        .first()
        .ok_or_else(|| "ready attempt has no glyphs".to_owned())?;
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

fn create_stem_state_from_fixture(
    oracle: &CreateStemSystemRows,
    scheduler: &audiveris_omr::native_stems_beam_scheduler::NativeStemsBeamSchedulerSystem,
    attempt: &NativeStemsBeamLinkPlanAttempt,
) -> Result<NativeStemsBeamVLinkTransactionState, String> {
    if !scheduler.deferred_line_deltas.is_empty() {
        return Err(format!(
            "system {} has unsupported real committed-prefix deltas",
            scheduler.system_id
        ));
    }
    let baseline = oracle.row("stemsbeamcreatestembaseline")?;
    let frontier_row = oracle.row("stemsbeamcreatestemfrontier")?;
    let lookup = oracle.row("stemsbeamcreatestemlookup")?;
    let frontier = match &scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => {
            return Err(format!(
                "system {} is not createStem-ready",
                scheduler.system_id
            ));
        }
    };
    let selected_evidence = parse_list(frontier_row.value("selectedGlyphRefs")?)?
        .into_iter()
        .map(parse_selected_glyph)
        .collect::<Result<Vec<_>, _>>()?;
    if selected_evidence.len() != attempt.glyphs.len() {
        return Err(format!(
            "system {} selected glyph count differs",
            scheduler.system_id
        ));
    }
    let mut selected_glyph_bindings = Vec::with_capacity(attempt.glyphs.len());
    for (selected, glyph) in selected_evidence.iter().zip(&attempt.glyphs) {
        let content = fixed_content(
            glyph.bounds,
            glyph.weight,
            glyph.structural_key.run_table.clone(),
        );
        if selected.glyph.bounds != content.bounds
            || selected.glyph.run_sha256 != glyph_run_sha256(&content.run_table)
        {
            return Err(format!(
                "system {} selected glyph structure differs",
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
        parse_bounds(lookup.value("candidateBounds")?)?,
        lookup.usize("candidateWeight")?,
        parse_run_table(lookup.value("candidateRunTable")?)?,
    );
    let candidate_evidence = parse_glyph_evidence(lookup.value("candidate")?)?;
    if candidate_evidence.bounds != candidate.bounds
        || candidate_evidence.run_sha256 != glyph_run_sha256(&candidate.run_table)
        || independent_candidate(attempt)? != candidate
    {
        return Err(format!(
            "system {} independently rebuilt candidate differs",
            scheduler.system_id
        ));
    }
    let glyph_lookup = match lookup.value("lookup")? {
        "Absent" => NativeStemsBeamExhaustiveGlyphLookup::Absent,
        "Present" => {
            let active = match lookup.value("presentActive")? {
                "true" => true,
                "false" => false,
                value => return Err(format!("invalid presentActive: {value}")),
            };
            NativeStemsBeamExhaustiveGlyphLookup::Present {
                canonical_alias: parse_glyph_alias(lookup.value("presentAlias")?)?,
                glyph_id: i32::try_from(lookup.usize("presentId")?)
                    .map_err(|_| "present glyph ID exceeds i32".to_owned())?,
                active_in_index: active,
            }
        }
        value => return Err(format!("invalid exhaustive glyph lookup: {value}")),
    };
    if lookup.value("systemStemLookup")? != "Absent" {
        return Err(format!(
            "system {} compact createStem fixture has a baseline stem",
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

fn create_stem_checker_context(page: &NativePage) -> NativeStemsBeamStemCheckerContext {
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

fn assert_create_stem_checker(
    system_id: usize,
    check: &audiveris_omr::stem_seeds_step::NativeStemCheckResult,
    result: &ExactRow,
) -> Result<(), String> {
    let grade = parse_hex_double(result.value("stemGrade")?, "stemGrade")?;
    if check.grade.to_bits() != grade.to_bits() {
        return Err(format!(
            "system {system_id} createStem checker grade differs"
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
        return Err(format!("system {system_id} createStem impacts differ"));
    }
    for (token, (name, impact, weight)) in expected.into_iter().zip(actual) {
        let (actual_name, rest) = token
            .split_once(':')
            .ok_or_else(|| format!("system {system_id} malformed impact"))?;
        let (expected_impact, expected_weight) = rest
            .split_once(":w=")
            .ok_or_else(|| format!("system {system_id} malformed impact weight"))?;
        if actual_name != name
            || parse_hex_double(expected_impact, "checker impact")?.to_bits() != impact.to_bits()
            || parse_hex_double(expected_weight, "checker weight")?.to_bits() != weight.to_bits()
        {
            return Err(format!("system {system_id} {name} impact differs"));
        }
    }
    Ok(())
}

fn replay_create_stem_system(
    page: &NativePage,
    fixture: &CreateStemFixture,
    system_id: usize,
) -> Result<
    (
        NativeStemsBeamVLinkTransaction,
        NativeStemsBeamVLinkTransactionState,
    ),
    String,
> {
    let oracle = fixture
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or_else(|| format!("missing createStem system {system_id}"))?;
    let scheduler = page
        .scheduler
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or_else(|| format!("missing scheduler system {system_id}"))?;
    let builders = page
        .beam_builders
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or_else(|| format!("missing builder system {system_id}"))?;
    let plans = page
        .plans
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or_else(|| format!("missing plan system {system_id}"))?;
    let frontier = match &scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => return Err(format!("system {system_id} is not createStem-ready")),
    };
    let attempt = attempt_for_plan(plans, frontier.plan.plan_ordinal)?;
    let frontier_row = oracle.row("stemsbeamcreatestemfrontier")?;
    let expand_row = oracle.row("stemsbeamcreatestemexpand")?;
    if frontier.plan.plan_ordinal != frontier_row.usize("plan")?
        || frontier.plan.builder_ordinal != frontier_row.usize("builder")?
        || usize::try_from(frontier.plan.stem_profile).ok()
            != Some(frontier_row.usize("stemProfile")?)
        || usize::try_from(plans.link_profile).ok() != Some(frontier_row.usize("linkProfile")?)
        || attempt.relations.len() != expand_row.usize("relations")?
        || attempt.glyphs.len() != expand_row.usize("glyphs")?
        || parse_line(frontier_row.value("lineBefore")?, "lineBefore")?
            != attempt.stored_theoretical_line_before
        || parse_line(expand_row.value("lineAfter")?, "lineAfter")?
            != attempt.stored_theoretical_line_after
        || (expand_row.value("lineChanged")? == "true")
            != attempt.stored_theoretical_line_would_mutate
    {
        return Err(format!(
            "system {system_id} createStem frontier join differs"
        ));
    }
    let mut state = create_stem_state_from_fixture(oracle, scheduler, attempt)?;
    let transaction = apply_native_stems_beam_vlink_create_stem_transaction(
        scheduler,
        builders,
        plans,
        &mut state,
        &create_stem_checker_context(page),
    )
    .map_err(|error| format!("system {system_id} createStem failed: {error}"))?;
    let lookup = oracle.row("stemsbeamcreatestemlookup")?;
    let result = oracle.row("stemsbeamcreatestemresult")?;
    let delta = oracle.row("stemsbeamcreatestemdelta")?;
    let registration = match transaction.registration.action {
        audiveris_omr::native_stems_beam_vlink_transaction::NativeStemsBeamGlyphRegistrationAction::Reused {
            reinserted_into_active_index: false,
        } => "ReuseActive",
        audiveris_omr::native_stems_beam_vlink_transaction::NativeStemsBeamGlyphRegistrationAction::Reused {
            reinserted_into_active_index: true,
        } => "ReinsertOriginal",
        audiveris_omr::native_stems_beam_vlink_transaction::NativeStemsBeamGlyphRegistrationAction::Registered => "New",
    };
    let disposition = match transaction.disposition {
        audiveris_omr::native_stems_beam_vlink_transaction::NativeStemsBeamCreateStemDisposition::CreatedChecked { .. } => "CreatedChecked",
        audiveris_omr::native_stems_beam_vlink_transaction::NativeStemsBeamCreateStemDisposition::CreatedArtificial { .. } => "CreatedArtificial",
        audiveris_omr::native_stems_beam_vlink_transaction::NativeStemsBeamCreateStemDisposition::Reused { .. } => "Reused",
        audiveris_omr::native_stems_beam_vlink_transaction::NativeStemsBeamCreateStemDisposition::Rejected => "Rejected",
    };
    if transaction.plan != frontier.plan
        || transaction.candidate != independent_candidate(attempt)?
        || transaction.registration.alias_order != NativeStemsBeamGlyphAliasOrder::JavaGlyphId
        || transaction.registration.canonical_alias
            != parse_glyph_alias(result.value("registeredAlias")?)?
        || transaction.registration.glyph_id
            != i32::try_from(result.usize("registeredGlyphId")?)
                .map_err(|_| "registered glyph ID exceeds i32".to_owned())?
        || transaction.registration.post_union_size != result.usize("postUnionSize")?
        || registration != result.value("registration")?
        || disposition != result.value("disposition")?
        || transaction.sig_vertex_mutation_count != 0
        || transaction.sig_relation_mutation_count != 0
        || transaction.linker_flag_mutation_count != 0
        || state.glyph_index.exhaustive_lookup.is_some()
        || state.system_stems.exhaustive_lookup.is_some()
        || state.glyph_index.persistent_ids.sheet_last_id
            != i32::try_from(delta.usize("allocatorAfter")?)
                .map_err(|_| "allocatorAfter exceeds i32".to_owned())?
        || lookup.value("candidate")? != result.value("candidate")?
    {
        return Err(format!("system {system_id} createStem transaction differs"));
    }
    let stem = transaction
        .stem
        .as_ref()
        .ok_or_else(|| format!("system {system_id} real createStem lacks stem"))?;
    let checker = transaction
        .checker_result
        .as_ref()
        .ok_or_else(|| format!("system {system_id} real createStem lacks checker"))?;
    if disposition != "CreatedChecked"
        || stem.glyph_id != transaction.registration.glyph_id
        || stem.glyph_content != transaction.candidate
        || stem.inter_id.is_some()
        || stem.sig_attached
        || stem.abnormal
        || result.usize("returnedStemInterId")? != 0
        || parse_line(result.value("stemMedian")?, "stemMedian")? != stem.geometry.median
        || parse_hex_double(result.value("stemMeanThickness")?, "stemMeanThickness")?.to_bits()
            != stem.geometry.mean_thickness.to_bits()
        || parse_rectangle(result.value("stemBounds")?)? != stem.geometry.ribbon_bounds
        || result.value("stemAbnormal")? != "false"
        || result.value("stemSigAttached")? != "false"
        || state
            .system_stems
            .known_stems
            .iter()
            .filter(|known| *known == stem)
            .count()
            != 1
    {
        return Err(format!(
            "system {system_id} createStem geometry/state differs"
        ));
    }
    assert_create_stem_checker(system_id, checker, result)?;
    Ok((transaction, state))
}

fn relation_parameters(
    interline: i32,
    main_stem_thickness: i32,
    profile: i32,
) -> NativeStemsBeamRelationParameters {
    NativeStemsBeamRelationParameters {
        interline,
        main_stem_thickness: f64::from(main_stem_thickness),
        profile,
        x_in_gap_maximum_profile0: JAVA_RELATION_PARAMETERS.x_in_profile_zero,
        x_out_gap_maximum: beam_x_out_maximum(profile, &JAVA_RELATION_PARAMETERS),
        y_gap_maximum: beam_y_maximum(profile, &JAVA_RELATION_PARAMETERS),
        x_weight: JAVA_RELATION_PARAMETERS.x_out_weight,
        y_weight: JAVA_RELATION_PARAMETERS.y_weight,
        intrinsic_ratio: JAVA_RELATION_PARAMETERS.intrinsic_ratio,
        minimum_grade: JAVA_RELATION_PARAMETERS.minimum_grade,
    }
}

fn project_real_reuse_system(
    page: &NativePage,
    create_fixture: &CreateStemFixture,
    reuse_fixture: &ReuseFixture,
    system_id: usize,
) -> Result<(), String> {
    let oracle = reuse_fixture
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or_else(|| format!("missing reuse/check system {system_id}"))?;
    let create_oracle = create_fixture
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or_else(|| format!("missing createStem system {system_id}"))?;
    let (transaction, state) = replay_create_stem_system(page, create_fixture, system_id)?;
    let scheduler = page
        .scheduler
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or_else(|| format!("missing scheduler system {system_id}"))?;
    let plans = page
        .plans
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or_else(|| format!("missing plan system {system_id}"))?;
    let stumps = page
        .beam_stumps
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or_else(|| format!("missing stump system {system_id}"))?;
    let vlinkers = page
        .beam_vlinkers
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or_else(|| format!("missing VLinker system {system_id}"))?;
    let frontier = match &scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => return Err(format!("system {system_id} lacks V transaction frontier")),
    };
    let attempt = attempt_for_plan(plans, frontier.plan.plan_ordinal)?;
    let beam = stumps
        .beams_by_abscissa
        .iter()
        .find(|beam| beam.source == frontier.beam)
        .ok_or_else(|| format!("system {system_id} lacks starting beam"))?;
    assert_reuse_predecessor_join(
        oracle,
        create_oracle,
        &transaction,
        &state,
        frontier,
        attempt,
        beam,
        plans,
    )?;

    let entries = oracle
        .entries
        .iter()
        .zip(&attempt.relations)
        .map(|(rows, relation)| {
            if !rows.scans.is_empty() || rows.entry.value("sLinked") != Ok("false") {
                return Err(format!(
                    "system {system_id} real live scan needs full typed hydration"
                ));
            }
            Ok(NativeStemsBeamReuseEntryEvidence {
                map_ordinal: rows.entry.usize("mapOrdinal")?,
                corner: relation.corner,
                observation: NativeStemsBeamReuseEntryObservation::Examined {
                    s_linker_linked: false,
                    head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::NotRead,
                },
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let live = NativeStemsBeamVLinkReuseLiveState {
        system_id,
        live_sig_stems: Vec::new(),
        evaluation: NativeStemsBeamVLinkReuseLiveEvaluation::Entries(entries),
    };
    let oracle_parameters = NativeStemsBeamRelationParameters {
        interline: oracle
            .check_context
            .value("interline")?
            .parse()
            .map_err(|_| format!("system {system_id} invalid relation interline"))?,
        main_stem_thickness: oracle
            .check_context
            .value("scaleStemThickness")?
            .parse()
            .map_err(|_| format!("system {system_id} invalid scale stem thickness"))?,
        profile: oracle
            .check_context
            .value("profile")?
            .parse()
            .map_err(|_| format!("system {system_id} invalid relation profile"))?,
        x_in_gap_maximum_profile0: parse_hex_double(
            oracle.check_context.value("portionXInP0")?,
            "x-in profile zero",
        )?,
        x_out_gap_maximum: parse_hex_double(
            oracle.check_context.value("xOutMax")?,
            "x-out maximum",
        )?,
        y_gap_maximum: parse_hex_double(oracle.check_context.value("yMax")?, "y maximum")?,
        x_weight: parse_hex_double(oracle.check_context.value("xWeight")?, "x weight")?,
        y_weight: parse_hex_double(oracle.check_context.value("yWeight")?, "y weight")?,
        intrinsic_ratio: parse_hex_double(
            oracle.check_context.value("intrinsicRatio")?,
            "intrinsic ratio",
        )?,
        minimum_grade: parse_hex_double(oracle.check_context.value("minGrade")?, "minimum grade")?,
    };
    let source_parameters = relation_parameters(
        plans.interline,
        vlinkers.main_stem_thickness,
        frontier.plan.stem_profile,
    );
    if oracle_parameters != source_parameters {
        return Err(format!(
            "system {system_id} Java/source relation parameters differ"
        ));
    }
    let state_before = state.clone();
    let transaction_before = transaction.clone();
    let output = evaluate_native_stems_beam_vlink_reuse_check(
        scheduler,
        plans,
        stumps,
        vlinkers,
        &transaction,
        &state,
        &live,
        oracle_parameters,
    )
    .map_err(|error| format!("system {system_id} production reuse/check: {error}"))?;
    if state != state_before || transaction != transaction_before {
        return Err(format!(
            "system {system_id} reuse/check mutated predecessor"
        ));
    }
    if output.initial_stem != transaction.stem
        || output.final_stem != transaction.stem
        || output.reuse_disposition != NativeStemsBeamReuseDisposition::AllUnlinked
        || output.reuse_trace.len() != attempt.relations.len()
        || output.reuse_trace.iter().any(|trace| {
            trace.s_linker_linked != Some(false)
                || trace.lookup != NativeStemsBeamHeadStemLookupTrace::NotRead
                || trace.action != NativeStemsBeamReuseAction::SkipUnlinked
        })
        || (
            output.persistent_id_mutation_count,
            output.system_stem_mutation_count,
            output.sig_vertex_mutation_count,
            output.sig_relation_mutation_count,
            output.linker_flag_mutation_count,
        ) != (0, 0, 0, 0, 0)
    {
        return Err(format!("system {system_id} production reuse/state differs"));
    }
    for ((trace, relation), rows) in output
        .reuse_trace
        .iter()
        .zip(&attempt.relations)
        .zip(&oracle.entries)
    {
        let expected_alias = corner_alias(relation.corner);
        if trace.map_ordinal != relation.map_ordinal
            || trace.corner != relation.corner
            || trace.parent_horizontal_side != relation.corner.horizontal
            || trace.plan_relation_head_side != relation.check.derived_horizontal
            || rows.entry.value("cAlias")? != expected_alias
            || rows.entry.value("parentHSide")? != head_side_token(relation.corner.horizontal)
            || rows.entry.value("parentVSide")? != vertical_side_token(relation.corner.vertical)
            || rows.entry.value("relationHeadSide")?
                != head_side_token(relation.check.derived_horizontal)
            || rows.entry.value("sideMismatch")?
                != bool_token(relation.corner.horizontal != relation.check.derived_horizontal)
        {
            return Err(format!("system {system_id} reuse trace projection differs"));
        }
    }
    let stem = output
        .final_stem
        .as_ref()
        .ok_or_else(|| format!("system {system_id} lacks final stem"))?;
    assert_selection_matches_stem(&oracle.selection, stem, &transaction)?;
    let independent = independent_check_relation(
        beam.median,
        beam.height,
        stem.geometry.median,
        opposite(frontier.vertical_side),
        plans.interline,
        vlinkers.main_stem_thickness,
        frontier.plan.stem_profile,
        &JAVA_RELATION_PARAMETERS,
    );
    let production_relation = output
        .relation_check
        .as_ref()
        .ok_or_else(|| format!("system {system_id} lacks relation check"))?;
    assert_relation_matches_independent(production_relation, &independent)?;
    assert_oracle_check_matches_independent(
        &oracle.check_context,
        &oracle.check_trace,
        &oracle.check_result,
        oracle.selection.value("finalStemAlias")?,
        stem,
        beam.median,
        beam.height,
        frontier.vertical_side,
        plans.interline,
        &independent,
    )?;
    match (&output.outcome, independent.accepted) {
        (NativeStemsBeamVLinkReuseCheckOutcome::ReadyBeforeSigMutation { relation }, true)
            if relation.partner_stem_identity == stem.stem_identity
                && relation.partner_inter_id == stem.inter_id
                && relation.outgoing
                && same_double(relation.grade, independent.grade)
                && same_double(relation.dx, independent.dx)
                && same_double(relation.dy, independent.dy) => {}
        (NativeStemsBeamVLinkReuseCheckOutcome::BeamStemRejected, false) => {}
        _ => return Err(format!("system {system_id} terminal outcome differs")),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn assert_reuse_predecessor_join(
    reuse: &ReuseSystemRows,
    create: &CreateStemSystemRows,
    transaction: &NativeStemsBeamVLinkTransaction,
    state: &NativeStemsBeamVLinkTransactionState,
    frontier: &audiveris_omr::native_stems_beam_scheduler::NativeStemsBeamAwaitingVLinkTransaction,
    attempt: &NativeStemsBeamLinkPlanAttempt,
    beam: &audiveris_omr::native_stems_beam_stumps::NativeStemsBeamStumpBeam,
    plans: &NativeStemsBeamLinkPlanSystem,
) -> Result<(), String> {
    let system_id = reuse.system_id;
    let create_baseline = create.row("stemsbeamcreatestembaseline")?;
    let create_frontier = create.row("stemsbeamcreatestemfrontier")?;
    let create_expand = create.row("stemsbeamcreatestemexpand")?;
    let create_result = create.row("stemsbeamcreatestemresult")?;
    let create_delta = create.row("stemsbeamcreatestemdelta")?;
    let create_guard = create.row("stemsbeamcreatestemguard")?;
    for (reuse_field, create_row, create_field) in [
        ("allocator", create_delta, "allocatorAfter"),
        ("glyphActive", create_delta, "glyphActiveAfter"),
        ("glyphOriginals", create_delta, "glyphOriginalsAfter"),
        ("interIndex", create_baseline, "interIndex"),
        ("sigVertices", create_baseline, "sigVertices"),
        ("sigEdges", create_baseline, "sigEdges"),
        ("systemStems", create_delta, "systemStemsAfter"),
        ("noStaff", create_baseline, "noStaff"),
        ("glyphActiveHash", create_delta, "glyphActiveHashAfter"),
        (
            "glyphOriginalsHash",
            create_delta,
            "glyphOriginalsHashAfter",
        ),
        ("interIndexHash", create_baseline, "interIndexHash"),
        ("sigHash", create_baseline, "sigHash"),
        ("systemStemsHash", create_delta, "systemStemsHashAfter"),
    ] {
        if reuse.baseline.value(reuse_field)? != create_row.value(create_field)? {
            return Err(format!(
                "system {system_id} createStem/reuse {reuse_field} join differs"
            ));
        }
    }
    for (reuse_field, create_row, create_field) in [
        ("plan", create_result, "plan"),
        ("builder", create_frontier, "builder"),
        ("stemProfile", create_frontier, "stemProfile"),
        ("linkProfile", create_frontier, "linkProfile"),
        ("lastIndex", create_expand, "lastIndex"),
        ("relationCount", create_expand, "relations"),
        ("glyphCount", create_expand, "glyphs"),
        ("selectedGlyphRefs", create_frontier, "selectedGlyphRefs"),
        (
            "createCandidateObjectIdBefore",
            create_result,
            "candidateObjectIdBefore",
        ),
        ("createRegistration", create_result, "registration"),
        ("createDisposition", create_result, "disposition"),
        ("createRegisteredAlias", create_result, "registeredAlias"),
        (
            "createReturnedStemInterId",
            create_result,
            "returnedStemInterId",
        ),
        ("createStemGrade", create_result, "stemGrade"),
        ("createStemMedian", create_result, "stemMedian"),
        ("createStemWidth", create_result, "stemMeanThickness"),
        ("createStemBounds", create_result, "stemBounds"),
        ("createStemAbnormal", create_result, "stemAbnormal"),
        ("createStemSigAttached", create_result, "stemSigAttached"),
    ] {
        if reuse.frontier.value(reuse_field)? != create_row.value(create_field)? {
            return Err(format!(
                "system {system_id} createStem/reuse frontier {reuse_field} differs"
            ));
        }
    }
    if create_guard.value("stopBeforeVReuse")? != "true" {
        return Err(format!(
            "system {system_id} createStem did not stop before reuse"
        ));
    }
    let expected_frontier = [
        ("relationsPastReturn", "-".to_owned()),
        ("beamOrder", frontier.snapshot.current_index.to_string()),
        ("beamSig", beam.sig_ordinal.to_string()),
        (
            "hSide",
            frontier
                .horizontal_side
                .map_or_else(|| "-".to_owned(), |side| head_side_token(side).to_owned()),
        ),
        (
            "bAlias",
            format!(
                "beam:{}:b:{}",
                beam.sig_ordinal,
                frontier.b_linker.id.saturating_sub(1)
            ),
        ),
        (
            "vSide",
            vertical_side_token(frontier.vertical_side).to_owned(),
        ),
        (
            "headToBeam",
            vertical_side_token(opposite(frontier.vertical_side)).to_owned(),
        ),
        ("builder", frontier.plan.builder_ordinal.to_string()),
        ("stemProfile", frontier.plan.stem_profile.to_string()),
        ("linkProfile", plans.link_profile.to_string()),
        (
            "lastIndex",
            attempt
                .expand_last_index
                .map_or_else(|| "-".to_owned(), |index| index.to_string()),
        ),
        ("relationCount", attempt.relations.len().to_string()),
        ("glyphCount", attempt.glyphs.len().to_string()),
        (
            "relationAliases",
            format!(
                "[{}]",
                attempt
                    .relations
                    .iter()
                    .map(|relation| corner_alias(relation.corner))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        ),
    ];
    for (field, expected) in expected_frontier {
        let actual = reuse.frontier.value(field)?;
        if actual != expected {
            return Err(format!(
                "system {system_id} native reuse frontier {field} differs: oracle={actual} native={expected}"
            ));
        }
    }
    if transaction.system_id != system_id || state.system_stems.system_id != system_id {
        return Err(format!(
            "system {system_id} native transaction system join differs"
        ));
    }
    Ok(())
}

fn assert_selection_matches_stem(
    selection: &ExactRow,
    stem: &NativeStemsBeamKnownSystemStem,
    transaction: &NativeStemsBeamVLinkTransaction,
) -> Result<(), String> {
    let grade = match &stem.grade {
        NativeStemsBeamStemGrade::Checked(check) => check.grade,
        NativeStemsBeamStemGrade::Artificial(grade) => *grade,
    };
    let glyph = parse_glyph_evidence(selection.value("finalGlyph")?)?;
    if selection.value("finalStemAlias")? != format!("created:{}", transaction.plan.plan_ordinal)
        || selection.value("finalGlyphId")? != stem.glyph_id.to_string()
        || glyph.bounds != stem.glyph_content.bounds
        || glyph.run_sha256 != glyph_run_sha256(&stem.glyph_content.run_table)
        || !same_double(
            parse_hex_double(selection.value("finalGrade")?, "final stem grade")?,
            grade,
        )
        || parse_line(selection.value("finalMedian")?, "final stem median")? != stem.geometry.median
        || !same_double(
            parse_hex_double(selection.value("finalWidth")?, "final stem width")?,
            stem.geometry.mean_thickness,
        )
        || parse_rectangle(selection.value("finalBounds")?)? != stem.geometry.ribbon_bounds
        || selection.value("finalAbnormal")? != bool_token(stem.abnormal)
        || selection.value("finalSigAttached")? != bool_token(stem.sig_attached)
    {
        return Err(format!(
            "system {} selected final stem differs",
            transaction.system_id
        ));
    }
    Ok(())
}

fn stem_numeric_grade(stem: &NativeStemsBeamKnownSystemStem) -> f64 {
    match &stem.grade {
        NativeStemsBeamStemGrade::Checked(check) => check.grade,
        NativeStemsBeamStemGrade::Artificial(grade) => *grade,
    }
}

fn synthetic_live_stem_from_row(
    row: &ExactRow,
    initial: &NativeStemsBeamKnownSystemStem,
    dense_identity: usize,
) -> Result<NativeStemsBeamKnownSystemStem, String> {
    let ordinal = row.usize("catalogOrdinal")?;
    let glyph = parse_glyph_evidence(row.value("glyph")?)?;
    let java_inter_id = row.i32("javaInterId")?;
    let glyph_id = row.i32("glyphId")?;
    if row.usize("sigVertexOrdinal")? != ordinal + 4
        || row.usize("firstHeadStemEdgeOrdinal")? != ordinal
        || java_inter_id != 1_500_000_101 + i32::try_from(ordinal).unwrap_or(i32::MAX)
        || glyph_id != 1_500_000_001
        || glyph.bounds != initial.glyph_content.bounds
        || glyph.run_sha256 != glyph_run_sha256(&initial.glyph_content.run_table)
        || !same_double(
            parse_hex_double(row.value("grade")?, "synthetic live stem grade")?,
            stem_numeric_grade(initial),
        )
        || parse_line(row.value("median")?, "synthetic live stem median")?
            != initial.geometry.median
        || !same_double(
            parse_hex_double(row.value("width")?, "synthetic live stem width")?,
            initial.geometry.mean_thickness,
        )
        || parse_rectangle(row.value("bounds")?)? != initial.geometry.ribbon_bounds
        || row.value("abnormal")? != "false"
        || row.value("sigAttached")? != "true"
    {
        return Err(format!(
            "synthetic live stem {ordinal} does not preserve the exact created-stem payload"
        ));
    }
    let mut stem = initial.clone();
    stem.stem_identity = dense_identity;
    stem.glyph_id = glyph_id;
    stem.inter_id = Some(java_inter_id);
    stem.sig_attached = true;
    stem.abnormal = false;
    Ok(stem)
}

fn assert_synthetic_selection_matches_stem(
    selection: &ExactRow,
    expected_alias: &str,
    stem: &NativeStemsBeamKnownSystemStem,
) -> Result<(), String> {
    let glyph = parse_glyph_evidence(selection.value("finalGlyph")?)?;
    if selection.value("finalStemAlias")? != expected_alias
        || selection.i32("finalStemInterId")? != stem.inter_id.unwrap_or(0)
        || selection.i32("finalGlyphId")? != stem.glyph_id
        || glyph.bounds != stem.glyph_content.bounds
        || glyph.run_sha256 != glyph_run_sha256(&stem.glyph_content.run_table)
        || !same_double(
            parse_hex_double(selection.value("finalGrade")?, "synthetic final grade")?,
            stem_numeric_grade(stem),
        )
        || parse_line(selection.value("finalMedian")?, "synthetic final median")?
            != stem.geometry.median
        || !same_double(
            parse_hex_double(selection.value("finalWidth")?, "synthetic final width")?,
            stem.geometry.mean_thickness,
        )
        || parse_rectangle(selection.value("finalBounds")?)? != stem.geometry.ribbon_bounds
        || selection.value("finalAbnormal")? != bool_token(stem.abnormal)
        || selection.value("finalSigAttached")? != bool_token(stem.sig_attached)
    {
        return Err(format!(
            "synthetic {} final stem projection differs",
            selection.value("case")?
        ));
    }
    Ok(())
}

fn assert_synthetic_scan_target(
    scan: &ExactRow,
    live_row: &ExactRow,
    expected_scan_ordinal: usize,
    expected_relation_identity: usize,
) -> Result<(), String> {
    if scan.usize("scanOrdinal")? != expected_scan_ordinal
        || scan.usize("sigEdgeOrdinal")? != expected_relation_identity
        || scan.value("targetStemAlias")? != live_row.value("stemAlias")?
        || scan.i32("targetStemInterId")? != live_row.i32("javaInterId")?
        || scan.value("targetSigAttached")? != "true"
        || scan.i32("targetGlyphId")? != live_row.i32("glyphId")?
        || scan.value("edgeHeadSide")? != "LEFT"
        || scan.value("matchesParentSide")? != "true"
        || scan.value("distinctInsertion")? != "true"
        || scan.value("action")? != "IncludeMatching"
    {
        return Err(format!(
            "synthetic {} scan target/order projection differs",
            scan.value("case")?
        ));
    }
    Ok(())
}

fn project_page_synthetic_rows(
    page: &NativePage,
    create_fixture: &CreateStemFixture,
    reuse_fixture: &ReuseFixture,
) -> Result<(), String> {
    let system_id = 1;
    let (transaction, state) = replay_create_stem_system(page, create_fixture, system_id)?;
    let state_before = state.clone();
    let transaction_before = transaction.clone();
    let initial = transaction.stem.as_ref().ok_or_else(|| {
        format!(
            "{} synthetic predecessor lacks created stem",
            reuse_fixture.page
        )
    })?;
    let scheduler = &page.scheduler.systems[0];
    let plans = &page.plans.systems[0];
    let stumps = &page.beam_stumps.systems[0];
    let vlinkers = &page.beam_vlinkers.systems[0];
    let frontier = match &scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => {
            return Err(format!(
                "{} synthetic lacks V transaction frontier",
                reuse_fixture.page
            ));
        }
    };
    let beam = stumps
        .beams_by_abscissa
        .iter()
        .find(|beam| beam.source == frontier.beam)
        .ok_or_else(|| format!("{} synthetic lacks starting beam", reuse_fixture.page))?;
    let synthetic = &reuse_fixture.synthetic;
    let live = synthetic
        .live_stems
        .iter()
        .enumerate()
        .map(|(ordinal, row)| synthetic_live_stem_from_row(row, initial, ordinal + 10_000))
        .collect::<Result<Vec<_>, _>>()?;

    // Exact isolated-graph target joins. `scanOrdinal` is dense per one
    // HeadInter scan; `sigEdgeOrdinal` is the independent global edge identity.
    if synthetic.unique_scans.len() != 1 || synthetic.multiple_scans.len() != 2 {
        return Err("synthetic scan cardinality differs".to_owned());
    }
    assert_synthetic_scan_target(&synthetic.unique_scans[0], &synthetic.live_stems[0], 0, 0)?;
    assert_synthetic_scan_target(&synthetic.multiple_scans[0], &synthetic.live_stems[0], 0, 2)?;
    assert_synthetic_scan_target(&synthetic.multiple_scans[1], &synthetic.live_stems[1], 1, 3)?;
    if synthetic.unique_scans[0].value("headSnapshotHash")?
        != synthetic.unique_entries[0].value("headSnapshotHash")?
        || synthetic.multiple_scans.iter().any(|scan| {
            scan.value("headSnapshotHash") != synthetic.multiple_entry.value("headSnapshotHash")
        })
    {
        return Err("synthetic head snapshot provenance differs".to_owned());
    }

    // Independently replay the actual LinkedHashMap/LinkedHashSet cardinality
    // decisions, including the Java-null zero case and lazy unread suffix.
    let zero = [IndependentReuseEntry {
        map_ordinal: 0,
        relation_side: IndependentHeadSide::Left,
        linked: true,
        parent_side: Some(IndependentHeadSide::Left),
        side_stem_sets: Vec::new(),
    }];
    if independent_reuse_scan(&zero) != Err(IndependentReuseScanError::MissingSideStemSet) {
        return Err("synthetic zero case is not the Java missing-side invariant".to_owned());
    }
    let unique = [
        IndependentReuseEntry {
            map_ordinal: 0,
            relation_side: IndependentHeadSide::Left,
            linked: true,
            parent_side: Some(IndependentHeadSide::Left),
            side_stem_sets: vec![IndependentHeadStemSet {
                side: IndependentHeadSide::Left,
                stem_identities: vec![live[0].stem_identity],
            }],
        },
        IndependentReuseEntry {
            map_ordinal: 1,
            relation_side: IndependentHeadSide::Right,
            linked: true,
            parent_side: None,
            side_stem_sets: Vec::new(),
        },
    ];
    if independent_reuse_scan(&unique)
        != Ok(IndependentReuseTrace {
            disposition: IndependentReuseDisposition::Selected {
                entry_ordinal: 0,
                stem_identity: live[0].stem_identity,
            },
            examined_ordinals: vec![0],
            unread_from_ordinal: Some(1),
        })
    {
        return Err("synthetic first-unique/lazy-break replay differs".to_owned());
    }
    let multiple = [IndependentReuseEntry {
        map_ordinal: 0,
        relation_side: IndependentHeadSide::Left,
        linked: true,
        parent_side: Some(IndependentHeadSide::Left),
        side_stem_sets: vec![IndependentHeadStemSet {
            side: IndependentHeadSide::Left,
            // Distinct StemInter objects remain two entries even with one
            // canonical Glyph identity/content.
            stem_identities: vec![live[0].stem_identity, live[1].stem_identity],
        }],
    }];
    if independent_reuse_scan(&multiple)
        != Ok(IndependentReuseTrace {
            disposition: IndependentReuseDisposition::NoUnique,
            examined_ordinals: vec![0],
            unread_from_ordinal: None,
        })
    {
        return Err("synthetic distinct-object multiplicity replay differs".to_owned());
    }
    assert_synthetic_selection_matches_stem(
        &synthetic.zero_selection,
        &format!("created:{}", transaction.plan.plan_ordinal),
        initial,
    )?;
    assert_synthetic_selection_matches_stem(
        &synthetic.unique_selection,
        synthetic.live_stems[0].value("stemAlias")?,
        &live[0],
    )?;
    assert_synthetic_selection_matches_stem(
        &synthetic.multiple_selection,
        &format!("created:{}", transaction.plan.plan_ordinal),
        initial,
    )?;

    let head_to_beam = opposite(frontier.vertical_side);
    let unique_check = independent_check_relation(
        beam.median,
        beam.height,
        live[0].geometry.median,
        head_to_beam,
        plans.interline,
        vlinkers.main_stem_thickness,
        frontier.plan.stem_profile,
        &JAVA_RELATION_PARAMETERS,
    );
    assert_oracle_check_matches_independent(
        &synthetic.unique_check_context,
        &synthetic.unique_check_trace,
        &synthetic.unique_check_result,
        synthetic.live_stems[0].value("stemAlias")?,
        &live[0],
        beam.median,
        beam.height,
        frontier.vertical_side,
        plans.interline,
        &unique_check,
    )?;
    let multiple_check = independent_check_relation(
        beam.median,
        beam.height,
        initial.geometry.median,
        head_to_beam,
        plans.interline,
        vlinkers.main_stem_thickness,
        frontier.plan.stem_profile,
        &JAVA_RELATION_PARAMETERS,
    );
    assert_oracle_check_matches_independent(
        &synthetic.multiple_check_context,
        &synthetic.multiple_check_trace,
        &synthetic.multiple_check_result,
        &format!("created:{}", transaction.plan.plan_ordinal),
        initial,
        beam.median,
        beam.height,
        frontier.vertical_side,
        plans.interline,
        &multiple_check,
    )?;

    let mut parallel_stem = initial.clone();
    parallel_stem.geometry.median = parse_line(
        synthetic.parallel_check_context.value("stemMedian")?,
        "parallel stem median",
    )?;
    parallel_stem.geometry.mean_thickness = parse_hex_double(
        synthetic.parallel_check_context.value("stemActualWidth")?,
        "parallel stem width",
    )?;
    parallel_stem.geometry.ribbon_bounds = VerticalRibbonArea::new(
        Segment {
            x1: parallel_stem.geometry.median.start.x,
            y1: parallel_stem.geometry.median.start.y,
            x2: parallel_stem.geometry.median.stop.x,
            y2: parallel_stem.geometry.median.stop.y,
        },
        parallel_stem.geometry.mean_thickness,
    )
    .integer_bounds();
    parallel_stem.inter_id = None;
    parallel_stem.sig_attached = false;
    let parallel_check = independent_check_relation(
        beam.median,
        beam.height,
        parallel_stem.geometry.median,
        head_to_beam,
        plans.interline,
        vlinkers.main_stem_thickness,
        frontier.plan.stem_profile,
        &JAVA_RELATION_PARAMETERS,
    );
    assert_oracle_check_matches_independent(
        &synthetic.parallel_check_context,
        &synthetic.parallel_check_trace,
        &synthetic.parallel_check_result,
        "synthetic:parallel",
        &parallel_stem,
        beam.median,
        beam.height,
        frontier.vertical_side,
        plans.interline,
        &parallel_check,
    )?;
    assert_row_line(
        &synthetic.parallel,
        "stemMedian",
        parallel_stem.geometry.median,
        "synthetic parallel",
    )?;
    assert_row_line(
        &synthetic.parallel,
        "border",
        segment_line(parallel_check.beam_border),
        "synthetic parallel",
    )?;
    for (field, expected) in [
        ("den", parallel_check.intersection_denominator),
        ("crossX", parallel_check.cross.x),
        ("crossY", parallel_check.cross.y),
        ("grade", parallel_check.grade),
    ] {
        assert_row_double(&synthetic.parallel, field, expected, "synthetic parallel")?;
    }

    let maximum_dx =
        java_rint_to_i32(f64::from(plans.interline) * JAVA_RELATION_PARAMETERS.x_in_profile_zero);
    let left_threshold = beam.median.x1 + f64::from(maximum_dx);
    let right_threshold = beam.median.x2 - f64::from(maximum_dx);
    let expected_x = [
        f64::from_bits(left_threshold.to_bits() - 1),
        left_threshold,
        right_threshold,
        f64::from_bits(right_threshold.to_bits() + 1),
    ];
    for ((row, x_stem), case) in synthetic.portions.iter().zip(expected_x).zip([
        "LeftBelow",
        "LeftExact",
        "RightExact",
        "RightAbove",
    ]) {
        let (_, portion) = independent_beam_portion(
            beam.median,
            x_stem,
            plans.interline,
            &JAVA_RELATION_PARAMETERS,
        );
        if row.value("case")? != case
            || row.usize("maxDx")? != usize::try_from(maximum_dx).unwrap_or(usize::MAX)
            || row.value("portion")? != independent_portion_token(portion)
        {
            return Err(format!("synthetic portion case {case} differs"));
        }
        for (field, expected) in [
            ("xStem", x_stem),
            ("leftThreshold", left_threshold),
            ("rightThreshold", right_threshold),
        ] {
            assert_row_double(row, field, expected, case)?;
        }
    }
    let minimum = JAVA_RELATION_PARAMETERS.minimum_grade;
    assert_row_double(
        &synthetic.threshold,
        "grade",
        minimum,
        "synthetic threshold",
    )?;
    assert_row_double(
        &synthetic.threshold,
        "minGrade",
        minimum,
        "synthetic threshold",
    )?;
    if synthetic.threshold.value("accepted")?
        != bool_token(relation_grade_is_accepted(minimum, minimum))
    {
        return Err("synthetic inclusive threshold replay differs".to_owned());
    }

    let first = parse_line(synthetic.intersection.value("first")?, "intersection first")?;
    let second = parse_line(
        synthetic.intersection.value("second")?,
        "intersection second",
    )?;
    let one = Segment {
        x1: first.start.x,
        y1: first.start.y,
        x2: first.stop.x,
        y2: first.stop.y,
    };
    let two = Segment {
        x1: second.start.x,
        y1: second.start.y,
        x2: second.stop.x,
        y2: second.stop.y,
    };
    let denominator =
        ((one.x1 - one.x2) * (two.y1 - two.y2)) - ((one.y1 - one.y2) * (two.x1 - two.x2));
    let v12 = (one.x1 * one.y2) - (one.y1 * one.x2);
    let v34 = (two.x1 * two.y2) - (two.y1 * two.x2);
    let x_numerator = (v12 * (two.x1 - two.x2)) - ((one.x1 - one.x2) * v34);
    let y_numerator = (v12 * (two.y1 - two.y2)) - ((one.y1 - one.y2) * v34);
    let cross = determinant_intersection(one, two);
    for (field, expected) in [
        ("den", denominator),
        ("v12", v12),
        ("v34", v34),
        ("xNumerator", x_numerator),
        ("yNumerator", y_numerator),
        ("crossX", cross.x),
        ("crossY", cross.y),
    ] {
        assert_row_double(
            &synthetic.intersection,
            field,
            expected,
            "synthetic intersection",
        )?;
    }

    let baseline = &reuse_fixture.systems[0].baseline;
    for (guard_field, baseline_field) in [
        ("realAllocatorBefore", "allocator"),
        ("realInterIndexHashBefore", "interIndexHash"),
        ("realSigHashBefore", "sigHash"),
        ("realSigRelationStateHashBefore", "sigRelationStateHash"),
        ("realSystemStemsHashBefore", "systemStemsHash"),
        ("realLinkerStateHashBefore", "linkerStateHash"),
        ("realLineStateHashBefore", "lineStateHash"),
    ] {
        if synthetic.guard.value(guard_field)? != baseline.value(baseline_field)? {
            return Err(format!(
                "synthetic guard real-state join {guard_field} differs"
            ));
        }
    }
    if state != state_before || transaction != transaction_before {
        return Err("synthetic projection mutated the real predecessor".to_owned());
    }
    Ok(())
}

fn project_reuse_page(spec: ReusePageSpec) -> Result<usize, String> {
    let root = repo_root();
    let create_text =
        std::fs::read_to_string(root.join("rust/oracle").join(spec.create_stem_fixture))
            .map_err(|error| format!("{} createStem fixture: {error}", spec.page))?;
    let reuse_text = std::fs::read_to_string(root.join("rust/oracle").join(spec.fixture))
        .map_err(|error| format!("{} reuse/check fixture: {error}", spec.page))?;
    let create = CreateStemFixture::parse(&create_text)
        .map_err(|error| format!("{} createStem parser: {error}", spec.page))?;
    let reuse = ReuseFixture::parse(&reuse_text)
        .map_err(|error| format!("{} reuse/check parser: {error}", spec.page))?;
    if create.page != spec.page
        || reuse.page != spec.page
        || create.systems.len() != reuse.systems.len()
        || create.page_row.value("schedulerFixtureSha256")?
            != reuse.page_row.value("schedulerFixtureSha256")?
        || create.page_row.value("expandFixtureSha256")?
            != reuse.page_row.value("expandFixtureSha256")?
    {
        return Err(format!("{} predecessor/reuse page join differs", spec.page));
    }
    validate_actual_reuse_provenance(&reuse, spec.key)?;
    let native = native_page(spec.image);
    project_page_synthetic_rows(&native, &create, &reuse)?;
    for system_id in 1..=reuse.systems.len() {
        project_real_reuse_system(&native, &create, &reuse, system_id)?;
    }
    Ok(reuse.systems.len())
}

fn corner_alias(
    corner: audiveris_omr::native_stems_beam_reachability::NativeStemsBeamHeadCornerRef,
) -> String {
    format!(
        "h:{}:{}:{}",
        corner.x_ordinal,
        head_side_token(corner.horizontal),
        vertical_side_token(corner.vertical)
    )
}

fn same_double(left: f64, right: f64) -> bool {
    left.to_bits() == right.to_bits()
}

fn same_point(left: NativeStemPoint, right: NativeStemPoint) -> bool {
    same_double(left.x, right.x) && same_double(left.y, right.y)
}

fn assert_relation_matches_independent(
    production: &audiveris_omr::native_stems_beam_vlink_reuse_check::NativeStemsBeamRelationCheck,
    independent: &IndependentRelationCheck,
) -> Result<(), String> {
    let portion = match independent.portion {
        IndependentBeamPortion::Left => NativeBeamPortion::Left,
        IndependentBeamPortion::Center => NativeBeamPortion::Center,
        IndependentBeamPortion::Right => NativeBeamPortion::Right,
    };
    let optional_point_matches = match (production.extension_point, independent.extension_point) {
        (None, None) => true,
        (Some(left), Some(right)) => same_point(left, right),
        _ => false,
    };
    if production.head_to_beam != independent.head_to_beam
        || production.beam_border != independent.beam_border
        || !same_point(production.cross, independent.cross)
        || production.portion_maximum_dx != independent.portion_max_dx
        || production.portion != portion
        || !same_double(
            production.main_stem_thickness,
            f64::from(independent.scale_main_stem_thickness),
        )
        || !same_double(
            production.signed_x_gap_pixels,
            independent.x_gap_pixels_before_abs,
        )
        || !same_double(
            production.x_gap_pixels,
            independent.x_gap_pixels_before_abs.abs(),
        )
        || !same_double(production.y_gap_pixels, independent.y_gap_pixels)
        || !same_double(production.dx, independent.dx)
        || !same_double(production.dy, independent.dy)
        || !same_double(production.x_out_gap_maximum, independent.x_maximum)
        || !same_double(production.y_gap_maximum, independent.y_maximum)
        || !same_double(production.raw_x_impact, independent.raw_x_impact)
        || !same_double(production.raw_y_impact, independent.raw_y_impact)
        || !same_double(production.x_impact, independent.x_impact)
        || !same_double(production.y_impact, independent.y_impact)
        || !same_double(production.x_weight, independent.x_weight)
        || !same_double(production.y_weight, independent.y_weight)
        || !same_double(production.intrinsic_ratio, independent.intrinsic_ratio)
        || !same_double(production.minimum_grade, independent.minimum_grade)
        || !same_double(production.grade, independent.grade)
        || production.accepted != independent.accepted
        || !optional_point_matches
    {
        return Err(format!(
            "production relation differs: production={production:#?}, independent={independent:#?}"
        ));
    }
    Ok(())
}

fn assert_row_double(
    row: &ExactRow,
    label: &str,
    expected: f64,
    context: &str,
) -> Result<(), String> {
    let actual = parse_hex_double(row.value(label)?, label)?;
    if !same_double(actual, expected) {
        return Err(format!(
            "{context} {label} differs: oracle={} native={}",
            row.value(label)?,
            expected.to_bits()
        ));
    }
    Ok(())
}

fn assert_row_line(
    row: &ExactRow,
    label: &str,
    expected: NativeStemLine,
    context: &str,
) -> Result<(), String> {
    let actual = parse_line(row.value(label)?, label)?;
    if !same_point(actual.start, expected.start) || !same_point(actual.stop, expected.stop) {
        return Err(format!("{context} {label} differs"));
    }
    Ok(())
}

fn segment_line(segment: Segment) -> NativeStemLine {
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

#[allow(clippy::too_many_arguments)] // Mirrors the independently joined Java check context.
fn assert_oracle_check_matches_independent(
    context: &ExactRow,
    trace: &ExactRow,
    result: &ExactRow,
    stem_alias: &str,
    stem: &NativeStemsBeamKnownSystemStem,
    beam_median: Segment,
    beam_height: f64,
    v_side: NativeStemVerticalSide,
    interline: i32,
    independent: &IndependentRelationCheck,
) -> Result<(), String> {
    let label = format!(
        "system {} {}:{}",
        context.value("system")?,
        context.value("scope")?,
        context.value("case")?
    );
    if context.value("vSide")? != vertical_side_token(v_side)
        || context.value("headToBeam")? != vertical_side_token(independent.head_to_beam)
        || context.value("borderSide")? != vertical_side_token(v_side)
        || context.value("stemAlias")? != stem_alias
        || context.value("stemInterId")?
            != stem
                .inter_id
                .map_or_else(|| "0".to_owned(), |id| id.to_string())
        || context.value("interline")? != interline.to_string()
        || context.value("scaleStemThickness")? != independent.scale_main_stem_thickness.to_string()
        || context.value("profile")? != independent.profile.to_string()
    {
        return Err(format!("{label} relation context identity differs"));
    }
    assert_row_line(context, "beamMedian", segment_line(beam_median), &label)?;
    assert_row_double(context, "beamHeight", beam_height, &label)?;
    assert_row_line(
        context,
        "beamBorder",
        segment_line(independent.beam_border),
        &label,
    )?;
    assert_row_line(context, "stemMedian", stem.geometry.median, &label)?;
    assert_row_double(
        context,
        "stemActualWidth",
        stem.geometry.mean_thickness,
        &label,
    )?;
    for (field, expected) in [
        ("halfScaleStemWidth", independent.half_scale_stem_width),
        ("portionXInP0", independent.portion_x_in_profile_zero),
        ("xOutMax", independent.x_maximum),
        ("yMax", independent.y_maximum),
        ("maxDxInput", independent.maximum_dx_input),
        ("maxDxRint", independent.maximum_dx_rint),
        ("xWeight", independent.x_weight),
        ("yWeight", independent.y_weight),
        ("intrinsicRatio", independent.intrinsic_ratio),
        ("minGrade", independent.minimum_grade),
    ] {
        assert_row_double(context, field, expected, &label)?;
    }
    if context.value("maxDxInt")? != independent.portion_max_dx.to_string() {
        return Err(format!("{label} maxDxInt differs"));
    }

    for (field, expected) in [
        ("intersectionDen", independent.intersection_denominator),
        ("v12", independent.v12),
        ("v34", independent.v34),
        ("xNumerator", independent.x_numerator),
        ("yNumerator", independent.y_numerator),
        ("crossX", independent.cross.x),
        ("crossY", independent.cross.y),
        ("leftThreshold", independent.left_threshold),
        ("rightThreshold", independent.right_threshold),
        ("signedXGapPixels", independent.x_gap_pixels_before_abs),
        ("signedDxFrac", independent.signed_dx_fraction),
        ("storedDx", independent.dx),
        ("upperGapRaw", independent.upper_gap_raw),
        ("upperGap", independent.upper_gap),
        ("lowerGapRaw", independent.lower_gap_raw),
        ("lowerGap", independent.lower_gap),
        ("yGapPixels", independent.y_gap_pixels),
        ("storedDy", independent.dy),
        ("xImpactRaw", independent.raw_x_impact),
        ("yImpactRaw", independent.raw_y_impact),
        ("xImpact", independent.x_impact),
        ("yImpact", independent.y_impact),
        ("xPow", independent.x_power),
        ("yPow", independent.y_power),
        ("global", independent.global),
        ("totalWeight", independent.total_weight),
        ("reciprocalWeight", independent.reciprocal_weight),
        ("root", independent.root),
        ("intrinsicRatio", independent.intrinsic_ratio),
        ("grade", independent.grade),
    ] {
        assert_row_double(trace, field, expected, &label)?;
    }
    if trace.value("portion")? != independent_portion_token(independent.portion)
        || trace.value("accepted")? != bool_token(independent.accepted)
    {
        return Err(format!("{label} relation trace outcome differs"));
    }
    let candidate = parse_point(trace.value("candidateExtension")?, "candidateExtension")?;
    if !same_point(candidate, independent.candidate_extension) {
        return Err(format!("{label} candidate extension differs"));
    }

    if independent.accepted {
        if result.value("checkOutcome")? != "Accepted"
            || result.value("linkNull")? != "false"
            || result.value("partnerAlias")? != stem_alias
            || result.value("partnerInterId")?
                != stem
                    .inter_id
                    .map_or_else(|| "0".to_owned(), |id| id.to_string())
            || result.value("outgoing")? != "true"
            || result.value("relationClass")? != "BeamStemRelation"
            || result.value("portion")? != independent_portion_token(independent.portion)
            || result.value("terminal")? != "ReadyBeforeSigMutation"
        {
            return Err(format!("{label} accepted Link wrapper differs"));
        }
        for (field, expected) in [
            ("dx", independent.dx),
            ("dy", independent.dy),
            ("grade", independent.grade),
        ] {
            assert_row_double(result, field, expected, &label)?;
        }
        let extension = parse_point(result.value("extension")?, "extension")?;
        if !same_point(extension, independent.candidate_extension) {
            return Err(format!("{label} accepted extension differs"));
        }
        let impacts = parse_list(result.value("impacts")?)?;
        let expected = [
            ("xOutGap", independent.x_impact, independent.x_weight),
            ("yGap", independent.y_impact, independent.y_weight),
        ];
        if impacts.len() != expected.len() {
            return Err(format!("{label} impact cardinality differs"));
        }
        for (token, (name, impact, weight)) in impacts.into_iter().zip(expected) {
            let (actual_name, values) = token
                .split_once(':')
                .ok_or_else(|| format!("{label} malformed impact"))?;
            let (actual_impact, actual_weight) = values
                .split_once(":w=")
                .ok_or_else(|| format!("{label} malformed impact weight"))?;
            if actual_name != name
                || !same_double(parse_hex_double(actual_impact, name)?, impact)
                || !same_double(parse_hex_double(actual_weight, name)?, weight)
            {
                return Err(format!("{label} {name} impact differs"));
            }
        }
    } else if result.value("checkOutcome")? != "Rejected"
        || result.value("linkNull")? != "true"
        || result.value("partnerAlias")? != "-"
        || result.value("partnerInterId")? != "-"
        || result.value("outgoing")? != "-"
        || result.value("relationClass")? != "-"
        || result.value("portion")? != "-"
        || result.value("dx")? != "-"
        || result.value("dy")? != "-"
        || result.value("grade")? != "-"
        || result.value("impacts")? != "-"
        || result.value("extension")? != "-"
        || result.value("terminal")? != "BeamStemRejected"
    {
        return Err(format!("{label} rejected Link sentinel differs"));
    }
    Ok(())
}

const fn bool_token(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

const fn vertical_side_token(side: NativeStemVerticalSide) -> &'static str {
    match side {
        NativeStemVerticalSide::Top => "TOP",
        NativeStemVerticalSide::Bottom => "BOTTOM",
    }
}

const fn head_side_token(side: NativeStemHeadSide) -> &'static str {
    match side {
        NativeStemHeadSide::Left => "LEFT",
        NativeStemHeadSide::Right => "RIGHT",
    }
}

const fn independent_portion_token(portion: IndependentBeamPortion) -> &'static str {
    match portion {
        IndependentBeamPortion::Left => "LEFT",
        IndependentBeamPortion::Center => "CENTER",
        IndependentBeamPortion::Right => "RIGHT",
    }
}

fn translated_live_stem(
    base: &NativeStemsBeamKnownSystemStem,
    stem_identity: usize,
    glyph_id: i32,
    inter_id: i32,
    delta_x: i32,
    delta_y: i32,
) -> NativeStemsBeamKnownSystemStem {
    let mut stem = base.clone();
    stem.stem_identity = stem_identity;
    stem.glyph_id = glyph_id;
    stem.glyph_content.bounds.x = base
        .glyph_content
        .bounds
        .x
        .checked_add_signed(delta_x as isize)
        .expect("synthetic glyph x");
    stem.glyph_content.bounds.y = base
        .glyph_content
        .bounds
        .y
        .checked_add_signed(delta_y as isize)
        .expect("synthetic glyph y");
    let dx = f64::from(delta_x);
    let dy = f64::from(delta_y);
    stem.geometry.median.start.x += dx;
    stem.geometry.median.stop.x += dx;
    stem.geometry.median.start.y += dy;
    stem.geometry.median.stop.y += dy;
    stem.geometry.ribbon_bounds = VerticalRibbonArea::new(
        Segment {
            x1: stem.geometry.median.start.x,
            y1: stem.geometry.median.start.y,
            x2: stem.geometry.median.stop.x,
            y2: stem.geometry.median.stop.y,
        },
        stem.geometry.mean_thickness,
    )
    .integer_bounds();
    stem.inter_id = Some(inter_id);
    stem.sig_attached = true;
    stem.abnormal = false;
    stem
}

fn unused_persistent_ids(state: &NativeStemsBeamVLinkTransactionState, count: usize) -> Vec<i32> {
    let mut used = BTreeSet::new();
    used.extend(
        state
            .glyph_index
            .known_canonical_glyphs
            .iter()
            .map(|glyph| glyph.glyph_id),
    );
    used.extend(
        state
            .selected_glyph_bindings
            .iter()
            .map(|binding| binding.glyph_id),
    );
    for stem in &state.system_stems.known_stems {
        used.insert(stem.glyph_id);
        used.extend(stem.inter_id);
    }
    let last_id = state.glyph_index.persistent_ids.sheet_last_id;
    (1..=last_id)
        .rev()
        .filter(|id| !used.contains(id))
        .take(count)
        .collect()
}

const fn opposite_head(side: NativeStemHeadSide) -> NativeStemHeadSide {
    match side {
        NativeStemHeadSide::Left => NativeStemHeadSide::Right,
        NativeStemHeadSide::Right => NativeStemHeadSide::Left,
    }
}

fn head_scan(
    relation: &audiveris_omr::native_stems_beam_link_plans::NativeStemsBeamHeadRelation,
    edges: Vec<(usize, NativeStemHeadSide, usize)>,
    stem_catalogue: &[NativeStemsBeamKnownSystemStem],
) -> NativeStemsBeamHeadStemScan {
    let edges = edges
        .into_iter()
        .enumerate()
        .map(
            |(scan_ordinal, (relation_identity, relation_head_side, target_stem_identity))| {
                NativeStemsBeamHeadStemEdge {
                    scan_ordinal,
                    relation_identity,
                    relation_head_side,
                    target_stem_identity,
                }
            },
        )
        .collect::<Vec<_>>();
    let mut scan = NativeStemsBeamHeadStemScan {
        corner: relation.corner,
        baseline_relation_count: edges.len(),
        scanned_relation_count: edges.len(),
        provenance_sha256: String::new(),
        edges,
    };
    scan.provenance_sha256 = scan
        .java_snapshot_sha256(stem_catalogue)
        .expect("synthetic Java HeadStem snapshot hash");
    scan
}

fn unchecked_head_scan(
    relation: &audiveris_omr::native_stems_beam_link_plans::NativeStemsBeamHeadRelation,
    edges: Vec<(usize, NativeStemHeadSide, usize)>,
) -> NativeStemsBeamHeadStemScan {
    let edges = edges
        .into_iter()
        .enumerate()
        .map(
            |(scan_ordinal, (relation_identity, relation_head_side, target_stem_identity))| {
                NativeStemsBeamHeadStemEdge {
                    scan_ordinal,
                    relation_identity,
                    relation_head_side,
                    target_stem_identity,
                }
            },
        )
        .collect::<Vec<_>>();
    NativeStemsBeamHeadStemScan {
        corner: relation.corner,
        baseline_relation_count: edges.len(),
        scanned_relation_count: edges.len(),
        provenance_sha256: "1".repeat(64),
        edges,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct IndependentRelationParameters {
    x_in_profile_zero: f64,
    x_out_profile_zero: f64,
    x_out_profile_one: f64,
    y_profile_zero: f64,
    y_profile_one: f64,
    y_profile_two: f64,
    y_profile_three: f64,
    x_out_weight: f64,
    y_weight: f64,
    intrinsic_ratio: f64,
    minimum_grade: f64,
}

const JAVA_RELATION_PARAMETERS: IndependentRelationParameters = IndependentRelationParameters {
    x_in_profile_zero: 0.5,
    x_out_profile_zero: 0.15,
    x_out_profile_one: 0.2,
    y_profile_zero: 0.8,
    y_profile_one: 1.2,
    y_profile_two: 2.0,
    y_profile_three: 4.0,
    x_out_weight: 1.0,
    y_weight: 4.0,
    // `AbstractConnection.OutImpacts` extends `SupportImpacts`, whose runtime
    // override is 1.0 (not `Grades.intrinsicRatio`).  The oracle still emits
    // this value and the production boundary accepts it explicitly.
    intrinsic_ratio: 1.0,
    minimum_grade: 0.1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IndependentBeamPortion {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IndependentHeadSide {
    Left,
    Right,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndependentHeadStemSet {
    side: IndependentHeadSide,
    /// Java iterates a `LinkedHashSet`; order is evidence even though only a
    /// singleton can be selected at this seam.
    stem_identities: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndependentReuseEntry {
    map_ordinal: usize,
    relation_side: IndependentHeadSide,
    linked: bool,
    parent_side: Option<IndependentHeadSide>,
    side_stem_sets: Vec<IndependentHeadStemSet>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IndependentReuseDisposition {
    AllUnlinked,
    NoUnique,
    Selected {
        entry_ordinal: usize,
        stem_identity: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndependentReuseTrace {
    disposition: IndependentReuseDisposition,
    examined_ordinals: Vec<usize>,
    /// First relation deliberately left unread after Java's `break`.
    unread_from_ordinal: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IndependentReuseScanError {
    NonContiguousMapOrdinal,
    MissingParentSide,
    MissingSideStemSet,
    EmptySideStemSet,
    DuplicateSideStemSet,
    DuplicateStemIdentity,
}

fn independent_reuse_scan(
    entries: &[IndependentReuseEntry],
) -> Result<IndependentReuseTrace, IndependentReuseScanError> {
    let mut examined_ordinals = Vec::new();
    let mut linked_count = 0_usize;
    for (ordinal, entry) in entries.iter().enumerate() {
        if entry.map_ordinal != ordinal {
            return Err(IndependentReuseScanError::NonContiguousMapOrdinal);
        }
        examined_ordinals.push(ordinal);
        if !entry.linked {
            continue;
        }
        linked_count += 1;
        let parent_side = entry
            .parent_side
            .ok_or(IndependentReuseScanError::MissingParentSide)?;
        if entry
            .side_stem_sets
            .iter()
            .filter(|set| set.side == parent_side)
            .count()
            > 1
        {
            return Err(IndependentReuseScanError::DuplicateSideStemSet);
        }
        let stems = entry
            .side_stem_sets
            .iter()
            .find(|set| set.side == parent_side)
            .ok_or(IndependentReuseScanError::MissingSideStemSet)?;
        if stems.stem_identities.is_empty() {
            return Err(IndependentReuseScanError::EmptySideStemSet);
        }
        let mut unique = stems.stem_identities.clone();
        unique.sort_unstable();
        if unique.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(IndependentReuseScanError::DuplicateStemIdentity);
        }
        if let [stem_identity] = stems.stem_identities.as_slice() {
            return Ok(IndependentReuseTrace {
                disposition: IndependentReuseDisposition::Selected {
                    entry_ordinal: ordinal,
                    stem_identity: *stem_identity,
                },
                examined_ordinals,
                unread_from_ordinal: (ordinal + 1 < entries.len()).then_some(ordinal + 1),
            });
        }
    }
    Ok(IndependentReuseTrace {
        disposition: if linked_count == 0 {
            IndependentReuseDisposition::AllUnlinked
        } else {
            IndependentReuseDisposition::NoUnique
        },
        examined_ordinals,
        unread_from_ordinal: None,
    })
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

#[derive(Clone, Copy, Debug, PartialEq)]
struct IndependentRelationCheck {
    head_to_beam: NativeStemVerticalSide,
    beam_border: Segment,
    portion_x_in_profile_zero: f64,
    maximum_dx_input: f64,
    maximum_dx_rint: f64,
    cross: NativeStemPoint,
    intersection_denominator: f64,
    v12: f64,
    v34: f64,
    x_numerator: f64,
    y_numerator: f64,
    left_threshold: f64,
    right_threshold: f64,
    portion_max_dx: i32,
    portion: IndependentBeamPortion,
    scale_main_stem_thickness: i32,
    profile: i32,
    half_scale_stem_width: f64,
    x_gap_pixels_before_abs: f64,
    signed_dx_fraction: f64,
    upper_gap_raw: f64,
    upper_gap: f64,
    lower_gap_raw: f64,
    lower_gap: f64,
    y_gap_pixels: f64,
    dx: f64,
    dy: f64,
    x_maximum: f64,
    y_maximum: f64,
    raw_x_impact: f64,
    raw_y_impact: f64,
    x_impact: f64,
    y_impact: f64,
    x_power: f64,
    y_power: f64,
    global: f64,
    total_weight: f64,
    reciprocal_weight: f64,
    root: f64,
    x_weight: f64,
    y_weight: f64,
    intrinsic_ratio: f64,
    grade: f64,
    minimum_grade: f64,
    candidate_extension: NativeStemPoint,
    extension_point: Option<NativeStemPoint>,
    accepted: bool,
}

fn opposite(side: NativeStemVerticalSide) -> NativeStemVerticalSide {
    match side {
        NativeStemVerticalSide::Top => NativeStemVerticalSide::Bottom,
        NativeStemVerticalSide::Bottom => NativeStemVerticalSide::Top,
    }
}

fn java_max(left: f64, right: f64) -> f64 {
    if left.is_nan() || right.is_nan() {
        f64::NAN
    } else if left == 0.0 && right == 0.0 {
        // `Math.max(-0.0, +0.0)` is positive zero.
        if left.is_sign_positive() || right.is_sign_positive() {
            0.0
        } else {
            -0.0
        }
    } else if left >= right {
        left
    } else {
        right
    }
}

#[allow(clippy::manual_clamp)] // Exact Java comparisons deliberately preserve NaN.
fn java_clamp(mut value: f64) -> f64 {
    // Java's comparisons leave NaN unchanged.
    if value < 0.0 {
        value = 0.0;
    }
    if value > 1.0 {
        value = 1.0;
    }
    value
}

fn java_rint_to_i32(value: f64) -> i32 {
    // Rust's saturating float-to-int cast matches Java's narrowing conversion
    // after the ties-to-even `Math.rint` result, including NaN and infinities.
    value.round_ties_even() as i32
}

fn determinant_intersection(one: Segment, two: Segment) -> NativeStemPoint {
    // Operation grouping and evaluation order mirror `LineUtil.intersection`.
    let denominator =
        ((one.x1 - one.x2) * (two.y1 - two.y2)) - ((one.y1 - one.y2) * (two.x1 - two.x2));
    let one_cross = (one.x1 * one.y2) - (one.y1 * one.x2);
    let two_cross = (two.x1 * two.y2) - (two.y1 * two.x2);
    NativeStemPoint {
        x: java_canonical_divide(
            (one_cross * (two.x1 - two.x2)) - ((one.x1 - one.x2) * two_cross),
            denominator,
        ),
        y: java_canonical_divide(
            (one_cross * (two.y1 - two.y2)) - ((one.y1 - one.y2) * two_cross),
            denominator,
        ),
    }
}

fn java_canonical_divide(numerator: f64, denominator: f64) -> f64 {
    let quotient = numerator / denominator;
    if quotient.is_nan() {
        f64::NAN
    } else {
        quotient
    }
}

fn independent_beam_border(median: Segment, height: f64, side: NativeStemVerticalSide) -> Segment {
    let dy = match side {
        NativeStemVerticalSide::Top => -height / 2.0,
        NativeStemVerticalSide::Bottom => height / 2.0,
    };
    Segment {
        x1: median.x1,
        y1: median.y1 + dy,
        x2: median.x2,
        y2: median.y2 + dy,
    }
}

fn independent_beam_portion(
    median: Segment,
    x_stem: f64,
    interline: i32,
    parameters: &IndependentRelationParameters,
) -> (i32, IndependentBeamPortion) {
    // `computeBeamPortion` always uses profile zero's x-in maximum.
    let maximum_dx = java_rint_to_i32(f64::from(interline) * parameters.x_in_profile_zero);
    let portion = if x_stem < median.x1 + f64::from(maximum_dx) {
        IndependentBeamPortion::Left
    } else if x_stem > median.x2 - f64::from(maximum_dx) {
        IndependentBeamPortion::Right
    } else {
        IndependentBeamPortion::Center
    };
    (maximum_dx, portion)
}

fn beam_y_maximum(profile: i32, parameters: &IndependentRelationParameters) -> f64 {
    match profile {
        i32::MIN..=0 => parameters.y_profile_zero,
        1 => parameters.y_profile_one,
        2 => parameters.y_profile_two,
        _ => parameters.y_profile_three,
    }
}

fn beam_x_out_maximum(profile: i32, parameters: &IndependentRelationParameters) -> f64 {
    if profile <= 0 {
        parameters.x_out_profile_zero
    } else {
        parameters.x_out_profile_one
    }
}

fn weighted_relation_grade(
    x_impact: f64,
    y_impact: f64,
    parameters: &IndependentRelationParameters,
) -> f64 {
    let mut global = 1.0;
    for (impact, weight) in [
        (x_impact, parameters.x_out_weight),
        (y_impact, parameters.y_weight),
    ] {
        if impact == 0.0 {
            // Java assigns zero here; a later non-zero NaN impact can turn the
            // product back into NaN, so this cannot be an early return.
            global = 0.0;
        } else if weight != 0.0 {
            global *= if impact.is_nan() {
                f64::NAN
            } else {
                java_positive_pow(impact, weight)
            };
        }
    }
    parameters.intrinsic_ratio
        * if global.is_nan() {
            f64::NAN
        } else {
            java_positive_pow(
                global,
                1.0 / (parameters.x_out_weight + parameters.y_weight),
            )
        }
}

fn relation_grade_is_accepted(grade: f64, minimum_grade: f64) -> bool {
    // Java uses an inclusive comparison. NaN therefore rejects naturally.
    grade >= minimum_grade
}

#[allow(clippy::too_many_arguments)] // Mirrors Java checkRelation's independent inputs.
fn independent_check_relation(
    beam_median: Segment,
    beam_height: f64,
    stem_median: NativeStemLine,
    head_to_beam: NativeStemVerticalSide,
    interline: i32,
    scale_main_stem_thickness: i32,
    profile: i32,
    parameters: &IndependentRelationParameters,
) -> IndependentRelationCheck {
    let border_side = opposite(head_to_beam);
    let beam_border = independent_beam_border(beam_median, beam_height, border_side);
    let stem_segment = Segment {
        x1: stem_median.start.x,
        y1: stem_median.start.y,
        x2: stem_median.stop.x,
        y2: stem_median.stop.y,
    };
    let intersection_denominator = ((stem_segment.x1 - stem_segment.x2)
        * (beam_border.y1 - beam_border.y2))
        - ((stem_segment.y1 - stem_segment.y2) * (beam_border.x1 - beam_border.x2));
    let v12 = (stem_segment.x1 * stem_segment.y2) - (stem_segment.y1 * stem_segment.x2);
    let v34 = (beam_border.x1 * beam_border.y2) - (beam_border.y1 * beam_border.x2);
    let x_numerator =
        (v12 * (beam_border.x1 - beam_border.x2)) - ((stem_segment.x1 - stem_segment.x2) * v34);
    let y_numerator =
        (v12 * (beam_border.y1 - beam_border.y2)) - ((stem_segment.y1 - stem_segment.y2) * v34);
    let cross = NativeStemPoint {
        x: java_canonical_divide(x_numerator, intersection_denominator),
        y: java_canonical_divide(y_numerator, intersection_denominator),
    };
    debug_assert!(same_point(
        cross,
        determinant_intersection(stem_segment, beam_border)
    ));
    let (portion_max_dx, portion) =
        independent_beam_portion(beam_median, cross.x, interline, parameters);
    let portion_x_in_profile_zero = parameters.x_in_profile_zero;
    let maximum_dx_input = f64::from(interline) * portion_x_in_profile_zero;
    let maximum_dx_rint = maximum_dx_input.round_ties_even();
    let left_threshold = beam_median.x1 + f64::from(portion_max_dx);
    let right_threshold = beam_median.x2 - f64::from(portion_max_dx);
    let half_stem_width = f64::from(scale_main_stem_thickness) / 2.0;
    let x_gap_pixels_before_abs = match portion {
        IndependentBeamPortion::Center => 0.0,
        IndependentBeamPortion::Left => beam_border.x1 + half_stem_width - cross.x,
        IndependentBeamPortion::Right => cross.x - beam_border.x2 + half_stem_width,
    };
    let upper_gap_raw = stem_median.start.y - cross.y;
    let upper_gap = java_max(0.0, upper_gap_raw);
    let lower_gap_raw = cross.y - stem_median.stop.y;
    let lower_gap = java_max(0.0, lower_gap_raw);
    let y_gap_pixels = java_max(upper_gap, lower_gap);
    let signed_dx_fraction = x_gap_pixels_before_abs / f64::from(interline);
    let dx = signed_dx_fraction.abs();
    let dy = y_gap_pixels / f64::from(interline);
    let x_maximum = beam_x_out_maximum(profile, parameters);
    let y_maximum = beam_y_maximum(profile, parameters);
    let raw_x_impact = (x_maximum - dx) / x_maximum;
    let raw_y_impact = (y_maximum - dy) / y_maximum;
    let x_impact = java_clamp(raw_x_impact);
    let y_impact = java_clamp(raw_y_impact);
    let x_power = if x_impact == 0.0 {
        0.0
    } else if x_impact.is_nan() {
        f64::NAN
    } else {
        java_positive_pow(x_impact, parameters.x_out_weight)
    };
    let y_power = if y_impact == 0.0 {
        0.0
    } else if y_impact.is_nan() {
        f64::NAN
    } else {
        java_positive_pow(y_impact, parameters.y_weight)
    };
    let mut global = 1.0;
    if x_impact == 0.0 {
        global = 0.0;
    } else if parameters.x_out_weight != 0.0 {
        global *= x_power;
    }
    if y_impact == 0.0 {
        global = 0.0;
    } else if parameters.y_weight != 0.0 {
        global *= y_power;
    }
    let total_weight = parameters.x_out_weight + parameters.y_weight;
    let reciprocal_weight = 1.0 / total_weight;
    let root = if global.is_nan() {
        f64::NAN
    } else {
        java_positive_pow(global, reciprocal_weight)
    };
    let grade = parameters.intrinsic_ratio * root;
    debug_assert!(same_double(
        grade,
        weighted_relation_grade(x_impact, y_impact, parameters)
    ));
    let accepted = relation_grade_is_accepted(grade, parameters.minimum_grade);
    let y_direction = match head_to_beam {
        NativeStemVerticalSide::Top => -1.0,
        NativeStemVerticalSide::Bottom => 1.0,
    };
    let candidate_extension = NativeStemPoint {
        x: cross.x,
        y: cross.y + y_direction * (beam_height - 1.0),
    };
    let extension_point = accepted.then_some(candidate_extension);
    IndependentRelationCheck {
        head_to_beam,
        beam_border,
        portion_x_in_profile_zero,
        maximum_dx_input,
        maximum_dx_rint,
        cross,
        intersection_denominator,
        v12,
        v34,
        x_numerator,
        y_numerator,
        left_threshold,
        right_threshold,
        portion_max_dx,
        portion,
        scale_main_stem_thickness,
        profile,
        half_scale_stem_width: half_stem_width,
        x_gap_pixels_before_abs,
        signed_dx_fraction,
        upper_gap_raw,
        upper_gap,
        lower_gap_raw,
        lower_gap,
        y_gap_pixels,
        dx,
        dy,
        x_maximum,
        y_maximum,
        raw_x_impact,
        raw_y_impact,
        x_impact,
        y_impact,
        x_power,
        y_power,
        global,
        total_weight,
        reciprocal_weight,
        root,
        x_weight: parameters.x_out_weight,
        y_weight: parameters.y_weight,
        intrinsic_ratio: parameters.intrinsic_ratio,
        grade,
        minimum_grade: parameters.minimum_grade,
        candidate_extension,
        extension_point,
        accepted,
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn byte_line_count(bytes: &[u8]) -> usize {
    bytes.iter().filter(|byte| **byte == b'\n').count()
}

fn find_byte_slice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

struct ReuseFixtureSlices<'a> {
    body: &'a [u8],
    header: &'a [u8],
    semantic: &'a [u8],
}

fn reuse_fixture_slices(bytes: &[u8]) -> Result<ReuseFixtureSlices<'_>, String> {
    let trailer_marker = b"stemsbeamvlinkreusecheckcorpussummary ";
    let trailer_start = find_byte_slice(bytes, trailer_marker)
        .ok_or_else(|| "reuse/check fixture lacks corpus trailer".to_owned())?;
    if find_byte_slice(
        &bytes[trailer_start + trailer_marker.len()..],
        trailer_marker,
    )
    .is_some()
    {
        return Err("reuse/check fixture has duplicate corpus trailer".to_owned());
    }
    let body = &bytes[..trailer_start];
    let page_marker = b"stemsbeamvlinkreusecheckpage ";
    let semantic_start = find_byte_slice(body, page_marker)
        .ok_or_else(|| "reuse/check body lacks page row".to_owned())?;
    if !body.ends_with(b"\n") || !bytes.ends_with(b"\n") {
        return Err("reuse/check fixture/body lacks terminal newline".to_owned());
    }
    Ok(ReuseFixtureSlices {
        body,
        header: &body[..semantic_start],
        semantic: &body[semantic_start..],
    })
}

fn expected_reuse_row_counts(fixture: &ReuseFixture) -> [usize; REUSE_ROW_COUNT_FIELDS] {
    let systems = fixture.systems.len();
    let live_stems = fixture
        .systems
        .iter()
        .map(|system| system.live_stems.len())
        .sum();
    let entries = fixture
        .systems
        .iter()
        .map(|system| system.entries.len())
        .sum();
    let scans = fixture
        .systems
        .iter()
        .flat_map(|system| &system.entries)
        .map(|entry| entry.scans.len())
        .sum();
    [
        1,
        systems,
        systems,
        live_stems,
        entries,
        scans,
        systems,
        systems + 3,
        systems + 3,
        systems + 3,
        systems,
        systems,
        2,
        4,
        3,
        3,
        4,
        1,
        1,
        1,
        1,
        1,
    ]
}

fn validate_reuse_manifest_fixture(
    entry: &ReuseManifestEntry,
    spec: ReusePageSpec,
    bytes: &[u8],
    manifest: &ReuseManifest,
) -> Result<ReuseFixture, String> {
    if bytes.len() != entry.fixture_bytes
        || byte_line_count(bytes) != entry.fixture_lines
        || sha256_hex(bytes) != entry.fixture_sha256
    {
        return Err(format!("{} fixture fingerprint differs", spec.page));
    }
    let slices = reuse_fixture_slices(bytes)?;
    if slices.body.len() != entry.body_bytes
        || byte_line_count(slices.body) != entry.body_lines
        || sha256_hex(slices.body) != entry.body_sha256
    {
        return Err(format!("{} emitted-body fingerprint differs", spec.page));
    }
    let fixture = ReuseFixture::parse(
        std::str::from_utf8(bytes).map_err(|error| format!("{} UTF-8: {error}", spec.page))?,
    )?;
    let expected_counts = expected_reuse_row_counts(&fixture);
    if fixture.page != spec.page
        || entry.ordinal
            != REUSE_PAGES
                .iter()
                .position(|candidate| *candidate == spec)
                .unwrap_or(usize::MAX)
        || entry.page != spec.page
        || entry.fixture != spec.fixture
        || entry.row_counts != expected_counts
        || parse_reuse_row_counts(mapped_value(&fixture.corpus, "rowCounts")?)? != expected_counts
        || fixture.page_row.value("schedulerFixtureSha256")? != entry.scheduler_sha256
        || fixture.page_row.value("expandFixtureSha256")? != entry.expand_sha256
        || fixture.page_row.value("createStemFixtureSha256")? != entry.create_stem_sha256
        || mapped_value(&fixture.corpus, "emittedBodySha256")? != entry.body_sha256
        || mapped_usize(&fixture.corpus, "emittedBodyLines")? != entry.body_lines
        || mapped_usize(&fixture.corpus, "emittedBodyBytes")? != entry.body_bytes
        || mapped_value(&fixture.corpus, "rawPassSha256")? != entry.raw_pass_sha256
        || mapped_value(&fixture.corpus, "mode")? != spec.key
        || mapped_usize(&fixture.corpus, "compilerJavaProcesses")? != entry.compiler_java_processes
        || mapped_usize(&fixture.corpus, "runtimeJavaProcesses")? != entry.runtime_java_processes
        || mapped_usize(&fixture.corpus, "totalJavaProcesses")? != entry.total_java_processes
    {
        return Err(format!("{} manifest/fixture algebra differs", spec.page));
    }
    for (label, _) in REUSE_SOURCE_PATHS {
        if fixture.corpus.get(label) != manifest.summary.get(label) {
            return Err(format!("{} {label} manifest join differs", spec.page));
        }
    }
    validate_actual_reuse_provenance(&fixture, spec.key)?;
    Ok(fixture)
}

fn validate_reuse_manifest_corpus(manifest_bytes: &[u8]) -> Result<ReuseManifest, String> {
    let manifest = ReuseManifest::parse(manifest_bytes)?;
    for (label, relative) in REUSE_SOURCE_PATHS {
        let actual = actual_file_sha256(relative)?;
        if mapped_value(&manifest.summary, label)? != actual {
            return Err(format!(
                "manifest {label} does not hash the active checkout"
            ));
        }
    }
    let summary_marker = b"stemsbeamvlinkreusecheckmanifestsummary ";
    let summary_start = find_byte_slice(manifest_bytes, summary_marker)
        .ok_or_else(|| "reuse/check manifest lacks summary".to_owned())?;
    let summary_suffix = &manifest_bytes[summary_start..];
    if find_byte_slice(&summary_suffix[summary_marker.len()..], summary_marker).is_some()
        || summary_suffix.iter().filter(|byte| **byte == b'\n').count() != 1
        || !summary_suffix.ends_with(b"\n")
    {
        return Err("reuse/check manifest summary is not the unique final row".to_owned());
    }
    let manifest_body = &manifest_bytes[..summary_start];
    if !manifest_body.ends_with(b"\n")
        || sha256_hex(manifest_body) != mapped_value(&manifest.summary, "manifestBodySha256")?
        || byte_line_count(manifest_body) != mapped_usize(&manifest.summary, "manifestBodyLines")?
        || manifest_body.len() != mapped_usize(&manifest.summary, "manifestBodyBytes")?
    {
        return Err("reuse/check manifest-body fingerprint differs".to_owned());
    }

    let root = repo_root();
    let mut common_header = None::<Vec<u8>>;
    let mut corpus_body = Vec::new();
    let mut aggregate_counts = [0_usize; REUSE_ROW_COUNT_FIELDS];
    for ((entry, spec), expected_ordinal) in manifest
        .entries
        .iter()
        .zip(REUSE_PAGES)
        .zip(0..REUSE_PAGES.len())
    {
        if entry.ordinal != expected_ordinal {
            return Err("reuse/check manifest ordinal differs".to_owned());
        }
        let bytes = std::fs::read(root.join("rust/oracle").join(&entry.fixture))
            .map_err(|error| format!("{} fixture missing: {error}", entry.page))?;
        validate_reuse_manifest_fixture(entry, spec, &bytes, &manifest)?;
        let slices = reuse_fixture_slices(&bytes)?;
        if let Some(expected) = &common_header {
            if slices.header != expected {
                return Err(format!("{} common header differs", entry.page));
            }
        } else {
            common_header = Some(slices.header.to_vec());
        }
        // Boundary 13 intentionally fingerprints each complete emitted page
        // body, including its independently generated common header.
        corpus_body.extend_from_slice(slices.body);
        for (aggregate, count) in aggregate_counts.iter_mut().zip(entry.row_counts) {
            *aggregate += count;
        }
    }
    if aggregate_counts
        != parse_reuse_row_counts(mapped_value(&manifest.summary, "corpusRowCounts")?)?
        || sha256_hex(&corpus_body) != mapped_value(&manifest.summary, "corpusBodySha256")?
        || byte_line_count(&corpus_body) != mapped_usize(&manifest.summary, "corpusBodyLines")?
        || corpus_body.len() != mapped_usize(&manifest.summary, "corpusBodyBytes")?
    {
        return Err("reuse/check reconstructed corpus fingerprint differs".to_owned());
    }
    Ok(manifest)
}

fn actual_file_sha256(relative: &str) -> Result<String, String> {
    let path = repo_root().join(relative);
    let bytes = std::fs::read(&path)
        .map_err(|error| format!("cannot read provenance source {}: {error}", path.display()))?;
    Ok(sha256_hex(&bytes))
}

fn validate_actual_reuse_provenance(fixture: &ReuseFixture, page_key: &str) -> Result<(), String> {
    for (label, relative) in REUSE_SOURCE_PATHS {
        let actual = actual_file_sha256(relative)?;
        if fixture.corpus.get(label).map(String::as_str) != Some(actual.as_str()) {
            return Err(format!("{label} does not hash the active checkout source"));
        }
    }

    for (label, stem) in [
        ("schedulerFixtureSha256", "stems-beam-scheduler"),
        ("expandFixtureSha256", "stems-beam-expand"),
        ("createStemFixtureSha256", "stems-beam-create-stem"),
    ] {
        let relative = format!("rust/oracle/{stem}-{page_key}.txt");
        let actual = actual_file_sha256(&relative)?;
        if fixture.page_row.value(label)? != actual
            || fixture.corpus.get(label).map(String::as_str) != Some(actual.as_str())
        {
            return Err(format!(
                "{label} does not hash the active predecessor fixture"
            ));
        }
    }
    Ok(())
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

#[test]
fn independent_relation_model_pins_java_boundary_arithmetic() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(java_rint_to_i32(2.5), 2);
    assert_eq!(java_rint_to_i32(3.5), 4);
    assert!(
        parse_hex_double("NaN/7ff8000000000000", "synthetic NaN")
            .unwrap()
            .is_nan()
    );
    assert!(parse_hex_double("NaN/7FF8000000000000", "uppercase NaN").is_err());
    let exact = parse_exact_row(
        "family page#1 system 1 plan 2",
        "family",
        &["system", "plan"],
    )
    .unwrap();
    assert_eq!(exact.page, "page#1");
    assert_eq!(exact.usize("plan").unwrap(), 2);
    assert!(
        parse_exact_row(
            "family page#1 plan 2 system 1",
            "family",
            &["system", "plan"]
        )
        .is_err()
    );

    let beam = Segment {
        x1: 10.0,
        y1: 20.0,
        x2: 30.0,
        y2: 20.0,
    };
    let stem = NativeStemLine {
        start: NativeStemPoint { x: 20.0, y: 10.0 },
        stop: NativeStemPoint { x: 20.0, y: 30.0 },
    };
    let center = independent_check_relation(
        beam,
        4.0,
        stem,
        NativeStemVerticalSide::Top,
        5,
        3,
        0,
        &JAVA_RELATION_PARAMETERS,
    );
    assert_eq!(center.portion_max_dx, 2, "Math.rint ties to even");
    assert_eq!(center.portion, IndependentBeamPortion::Center);
    assert_eq!(center.scale_main_stem_thickness, 3);
    assert_eq!(center.dx.to_bits(), 0.0_f64.to_bits());
    assert_eq!(center.intrinsic_ratio.to_bits(), 1.0_f64.to_bits());
    assert!(center.accepted);

    let just_below_left_boundary = f64::from_bits(12.0_f64.to_bits() - 1);
    let just_above_right_boundary = f64::from_bits(28.0_f64.to_bits() + 1);
    assert_eq!(
        independent_beam_portion(beam, just_below_left_boundary, 5, &JAVA_RELATION_PARAMETERS).1,
        IndependentBeamPortion::Left
    );
    assert_eq!(
        independent_beam_portion(beam, 12.0, 5, &JAVA_RELATION_PARAMETERS).1,
        IndependentBeamPortion::Center,
        "left boundary equality is center because Java uses strict <"
    );
    assert_eq!(
        independent_beam_portion(beam, 28.0, 5, &JAVA_RELATION_PARAMETERS).1,
        IndependentBeamPortion::Center,
        "right boundary equality is center because Java uses strict >"
    );
    assert_eq!(
        independent_beam_portion(
            beam,
            just_above_right_boundary,
            5,
            &JAVA_RELATION_PARAMETERS
        )
        .1,
        IndependentBeamPortion::Right
    );
    assert_eq!(
        independent_beam_portion(beam, 20.0, 7, &JAVA_RELATION_PARAMETERS).0,
        4,
        "3.5 pixels rounds to even"
    );
    assert_eq!(
        beam_x_out_maximum(4, &JAVA_RELATION_PARAMETERS).to_bits(),
        JAVA_RELATION_PARAMETERS.x_out_profile_one.to_bits(),
        "x-out profile 4 falls back to the highest declared profile (1)"
    );
    assert_eq!(
        beam_y_maximum(4, &JAVA_RELATION_PARAMETERS).to_bits(),
        JAVA_RELATION_PARAMETERS.y_profile_three.to_bits(),
        "y profile 4 falls back to the highest declared profile (3)"
    );

    let side_stem = NativeStemLine {
        start: NativeStemPoint { x: 9.0, y: 10.0 },
        stop: NativeStemPoint { x: 9.0, y: 30.0 },
    };
    let thin_scale = independent_check_relation(
        beam,
        4.0,
        side_stem,
        NativeStemVerticalSide::Top,
        5,
        2,
        1,
        &JAVA_RELATION_PARAMETERS,
    );
    let thick_scale = independent_check_relation(
        beam,
        4.0,
        side_stem,
        NativeStemVerticalSide::Top,
        5,
        6,
        1,
        &JAVA_RELATION_PARAMETERS,
    );
    assert_eq!(
        thin_scale.x_gap_pixels_before_abs.to_bits(),
        2.0_f64.to_bits()
    );
    assert_eq!(
        thick_scale.x_gap_pixels_before_abs.to_bits(),
        4.0_f64.to_bits()
    );
    assert_eq!(thin_scale.portion, IndependentBeamPortion::Left);

    let outside = independent_check_relation(
        beam,
        4.0,
        NativeStemLine {
            start: NativeStemPoint { x: 20.0, y: -10.0 },
            stop: NativeStemPoint { x: 20.0, y: 0.0 },
        },
        NativeStemVerticalSide::Top,
        5,
        3,
        0,
        &JAVA_RELATION_PARAMETERS,
    );
    assert!(outside.y_gap_pixels > 0.0);
    assert_eq!(outside.y_impact.to_bits(), 0.0_f64.to_bits());
    assert_eq!(outside.grade.to_bits(), 0.0_f64.to_bits());
    assert!(!outside.accepted);

    assert!(relation_grade_is_accepted(0.1, 0.1));
    assert!(!relation_grade_is_accepted(
        0.1,
        f64::from_bits(0.1_f64.to_bits() + 1)
    ));

    let parallel = independent_check_relation(
        beam,
        4.0,
        NativeStemLine {
            start: NativeStemPoint { x: 10.0, y: 10.0 },
            stop: NativeStemPoint { x: 30.0, y: 10.0 },
        },
        NativeStemVerticalSide::Top,
        5,
        3,
        0,
        &JAVA_RELATION_PARAMETERS,
    );
    assert_eq!(parallel.cross.x.to_bits(), 0xfff0_0000_0000_0000);
    assert_eq!(parallel.cross.y.to_bits(), 0x7ff8_0000_0000_0000);
    assert_eq!(parallel.portion, IndependentBeamPortion::Left);
    assert!(parallel.dy.is_nan() && parallel.grade.is_nan());
    assert!(!parallel.accepted && parallel.extension_point.is_none());
}

#[test]
fn paired_create_stem_fixture_parser_fails_closed() {
    let bytes = std::fs::read(repo_root().join("rust/oracle/stems-beam-create-stem-chula.txt"))
        .expect("frozen paired Chula createStem fixture");
    assert_eq!(
        sha256_hex(&bytes),
        "10ada930287be952dcb31666b7af0e77a30f2c513ca69698cc53ab00b206ef6c"
    );
    let text = std::str::from_utf8(&bytes).expect("paired fixture UTF-8");
    let fixture = CreateStemFixture::parse(text).expect("strict paired createStem parser");
    assert_eq!(fixture.page, "chula.png#1");
    assert_eq!(fixture.systems.len(), 3);
    let native = native_page("chula.png");
    for system_id in 1..=3 {
        replay_create_stem_system(&native, &fixture, system_id)
            .unwrap_or_else(|error| panic!("paired system {system_id}: {error}"));
    }
    assert!(
        CreateStemFixture::parse(&text.replacen(
            "stopBeforeVReuse true",
            "stopBeforeVReuse false",
            1,
        ))
        .is_err(),
        "paired predecessor boundary drift must fail closed"
    );
    assert!(
        CreateStemFixture::parse(&text.replacen("stemMedian", "renamedMedian", 1)).is_err(),
        "paired result schema drift must fail closed"
    );
}

#[test]
fn chula_reuse_fixture_parser_and_provenance_fail_closed() {
    let bytes =
        std::fs::read(repo_root().join("rust/oracle/stems-beam-vlink-reuse-check-chula.txt"))
            .expect("retained polished Chula reuse/check fixture");
    assert_eq!(
        sha256_hex(&bytes),
        "a80973509f1d46ea714a9423b25e990b569232ec2c06f91328a309742ae39692"
    );
    assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 68);
    let text = std::str::from_utf8(&bytes).expect("reuse/check fixture UTF-8");
    let fixture = ReuseFixture::parse(text)
        .unwrap_or_else(|error| panic!("strict reuse/check parser: {error}"));
    assert_eq!(fixture.page, "chula.png#1");
    assert_eq!(fixture.systems.len(), 3);
    validate_actual_reuse_provenance(&fixture, "chula")
        .unwrap_or_else(|error| panic!("active-checkout provenance: {error}"));
    let corpus_start = text
        .find("stemsbeamvlinkreusecheckcorpussummary ")
        .expect("reuse/check corpus summary");
    let body = &text.as_bytes()[..corpus_start];
    assert_eq!(body.len(), 51_335);
    assert_eq!(body.iter().filter(|byte| **byte == b'\n').count(), 67);
    assert_eq!(
        sha256_hex(body),
        "28699385cf109cbb393bb8d49db65b5e71a3f8820c0a7591816aec9f6f58a1dd"
    );
    for field in [
        "schedulerFixtureSha256",
        "expandFixtureSha256",
        "createStemFixtureSha256",
    ] {
        let value = fixture
            .page_row
            .value(field)
            .expect("page provenance field");
        let drifted = text.replacen(value, &"0".repeat(64), 1);
        assert!(
            ReuseFixture::parse(&drifted).is_err(),
            "{field} provenance drift must fail closed"
        );
    }

    let mut swapped = fixture.clone();
    let scheduler = swapped
        .page_row
        .value("schedulerFixtureSha256")
        .expect("scheduler fixture SHA")
        .to_owned();
    let expand = swapped
        .page_row
        .value("expandFixtureSha256")
        .expect("expand fixture SHA")
        .to_owned();
    swapped
        .page_row
        .fields
        .insert("schedulerFixtureSha256".to_owned(), expand.clone());
    swapped
        .page_row
        .fields
        .insert("expandFixtureSha256".to_owned(), scheduler.clone());
    swapped
        .corpus
        .insert("schedulerFixtureSha256".to_owned(), expand);
    swapped
        .corpus
        .insert("expandFixtureSha256".to_owned(), scheduler);
    assert!(
        validate_actual_reuse_provenance(&swapped, "chula").is_err(),
        "internally consistent predecessor-hash swapping must fail against the checkout"
    );

    let mut source_drift = fixture.clone();
    source_drift
        .corpus
        .insert("beamLinkerSourceSha256".to_owned(), "0".repeat(64));
    assert!(
        validate_actual_reuse_provenance(&source_drift, "chula").is_err(),
        "Java source drift must fail against the checkout"
    );
}

#[test]
fn native_boundary_thirteen_chula_no_reuse_matches_independent_math() {
    let text =
        std::fs::read_to_string(repo_root().join("rust/oracle/stems-beam-create-stem-chula.txt"))
            .expect("paired Chula createStem fixture");
    let fixture = CreateStemFixture::parse(&text).expect("strict paired createStem fixture");
    let reuse_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-vlink-reuse-check-chula.txt"),
    )
    .expect("polished Chula reuse/check fixture");
    let reuse_fixture = ReuseFixture::parse(&reuse_text).expect("strict Chula reuse/check fixture");
    validate_actual_reuse_provenance(&reuse_fixture, "chula")
        .expect("reuse/check fixture pins the active checkout");
    let page = native_page("chula.png");
    project_page_synthetic_rows(&page, &fixture, &reuse_fixture)
        .unwrap_or_else(|error| panic!("system 1 exact Java synthetic projection: {error}"));
    for system_id in 1..=3 {
        project_real_reuse_system(&page, &fixture, &reuse_fixture, system_id)
            .unwrap_or_else(|error| panic!("system {system_id} exact Java projection: {error}"));
        let (transaction, state) = replay_create_stem_system(&page, &fixture, system_id)
            .unwrap_or_else(|error| panic!("system {system_id} predecessor: {error}"));
        let state_before = state.clone();
        let transaction_before = transaction.clone();
        let scheduler = page
            .scheduler
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .expect("scheduler system");
        let plans = page
            .plans
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .expect("plan system");
        let stumps = page
            .beam_stumps
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .expect("stump system");
        let vlinkers = page
            .beam_vlinkers
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .expect("VLinker system");
        let frontier = match &scheduler.status {
            NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
            _ => panic!("system {system_id} lacks V transaction"),
        };
        let attempt = attempt_for_plan(plans, frontier.plan.plan_ordinal).expect("ready plan");
        let live = NativeStemsBeamVLinkReuseLiveState {
            system_id,
            live_sig_stems: Vec::new(),
            evaluation: NativeStemsBeamVLinkReuseLiveEvaluation::Entries(
                attempt
                    .relations
                    .iter()
                    .map(|relation| NativeStemsBeamReuseEntryEvidence {
                        map_ordinal: relation.map_ordinal,
                        corner: relation.corner,
                        observation: NativeStemsBeamReuseEntryObservation::Examined {
                            s_linker_linked: false,
                            head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::NotRead,
                        },
                    })
                    .collect(),
            ),
        };
        let parameters = relation_parameters(
            plans.interline,
            vlinkers.main_stem_thickness,
            frontier.plan.stem_profile,
        );
        let output = evaluate_native_stems_beam_vlink_reuse_check(
            scheduler,
            plans,
            stumps,
            vlinkers,
            &transaction,
            &state,
            &live,
            parameters,
        )
        .unwrap_or_else(|error| panic!("system {system_id} reuse/check failed: {error}"));
        assert_eq!(state, state_before, "boundary mutated createStem state");
        assert_eq!(
            transaction, transaction_before,
            "boundary mutated createStem transaction"
        );
        assert_eq!(output.initial_stem, transaction.stem);
        assert_eq!(output.final_stem, transaction.stem);
        assert_eq!(
            output.reuse_disposition,
            NativeStemsBeamReuseDisposition::AllUnlinked
        );
        assert_eq!(output.reuse_trace.len(), attempt.relations.len());
        assert!(output.reuse_trace.iter().all(|trace| {
            trace.s_linker_linked == Some(false)
                && trace.action == NativeStemsBeamReuseAction::SkipUnlinked
        }));
        let stem = output.final_stem.as_ref().expect("created stem");
        let beam = stumps
            .beams_by_abscissa
            .iter()
            .find(|beam| beam.source == frontier.beam)
            .expect("starting beam");
        let independent = independent_check_relation(
            beam.median,
            beam.height,
            stem.geometry.median,
            opposite(frontier.vertical_side),
            plans.interline,
            vlinkers.main_stem_thickness,
            frontier.plan.stem_profile,
            &JAVA_RELATION_PARAMETERS,
        );
        let relation = output.relation_check.as_ref().expect("relation check");
        assert_relation_matches_independent(relation, &independent)
            .unwrap_or_else(|error| panic!("system {system_id}: {error}"));
        match (&output.outcome, independent.accepted) {
            (NativeStemsBeamVLinkReuseCheckOutcome::ReadyBeforeSigMutation { relation }, true) => {
                assert!(relation.outgoing);
                assert_eq!(relation.partner_stem_identity, stem.stem_identity);
                assert_eq!(relation.partner_inter_id, stem.inter_id);
                assert!(same_double(relation.grade, independent.grade));
                assert!(same_double(relation.dx, independent.dx));
                assert!(same_double(relation.dy, independent.dy));
            }
            (NativeStemsBeamVLinkReuseCheckOutcome::BeamStemRejected, false) => {}
            (NativeStemsBeamVLinkReuseCheckOutcome::CreateStemRejected, _) => {
                panic!("system {system_id} lost created stem")
            }
            (outcome, accepted) => {
                panic!("system {system_id} outcome/acceptance differs: {outcome:?}/{accepted}")
            }
        }
        assert_eq!(
            (
                output.persistent_id_mutation_count,
                output.system_stem_mutation_count,
                output.sig_vertex_mutation_count,
                output.sig_relation_mutation_count,
                output.linker_flag_mutation_count,
            ),
            (0, 0, 0, 0, 0)
        );
    }
}

#[test]
fn native_boundary_thirteen_installed_page_corpus_matches_exactly() {
    // The fixed eight-page list prevents a missing page from being mistaken
    // for an optional corpus member.
    for spec in REUSE_PAGES {
        let systems = project_reuse_page(spec)
            .unwrap_or_else(|error| panic!("{} exact page projection: {error}", spec.page));
        assert!(systems > 0, "{} has no system transactions", spec.page);
    }
}

#[test]
fn native_boundary_thirteen_manifest_reconstructs_exact_corpus() {
    let bytes =
        std::fs::read(repo_root().join(REUSE_MANIFEST_PATH)).expect("frozen reuse/check manifest");
    assert_eq!(sha256_hex(&bytes), EXPECTED_REUSE_MANIFEST_SHA256);
    let manifest = validate_reuse_manifest_corpus(&bytes)
        .unwrap_or_else(|error| panic!("reuse/check manifest: {error}"));
    assert_eq!(manifest.entries.len(), REUSE_PAGES.len());
    assert_eq!(
        mapped_value(&manifest.summary, "corpusBodySha256").unwrap(),
        EXPECTED_REUSE_CORPUS_BODY_SHA256
    );
    assert_eq!(
        mapped_usize(&manifest.summary, "corpusBodyLines").unwrap(),
        EXPECTED_REUSE_CORPUS_BODY_LINES
    );
    assert_eq!(
        mapped_usize(&manifest.summary, "corpusBodyBytes").unwrap(),
        EXPECTED_REUSE_CORPUS_BODY_BYTES
    );
    assert_eq!(
        parse_reuse_row_counts(mapped_value(&manifest.summary, "corpusRowCounts").unwrap())
            .unwrap(),
        EXPECTED_REUSE_CORPUS_ROW_COUNTS
    );
    assert_eq!(
        mapped_value(&manifest.summary, "manifestBodySha256").unwrap(),
        EXPECTED_REUSE_MANIFEST_BODY_SHA256
    );
    assert_eq!(
        mapped_usize(&manifest.summary, "manifestBodyLines").unwrap(),
        EXPECTED_REUSE_MANIFEST_BODY_LINES
    );
    assert_eq!(
        mapped_usize(&manifest.summary, "manifestBodyBytes").unwrap(),
        EXPECTED_REUSE_MANIFEST_BODY_BYTES
    );
    assert_eq!(mapped_usize(&manifest.summary, "realSystems").unwrap(), 30);
    assert_eq!(
        mapped_usize(&manifest.summary, "realRelationEntries").unwrap(),
        65
    );

    let mut appended = bytes.clone();
    appended.extend_from_slice(b"# unauthenticated trailing comment\n");
    assert!(
        validate_reuse_manifest_corpus(&appended).is_err(),
        "bytes after the unique final manifest summary must fail closed"
    );
}

#[test]
fn native_boundary_thirteen_synthetic_reuse_and_fail_closed() {
    let text =
        std::fs::read_to_string(repo_root().join("rust/oracle/stems-beam-create-stem-chula.txt"))
            .expect("paired Chula createStem fixture");
    let fixture = CreateStemFixture::parse(&text).expect("strict paired createStem fixture");
    let page = native_page("chula.png");
    let system_id = 1;
    let (transaction, state) =
        replay_create_stem_system(&page, &fixture, system_id).expect("system-1 createStem");
    let scheduler = &page.scheduler.systems[0];
    let plans = &page.plans.systems[0];
    let stumps = &page.beam_stumps.systems[0];
    let vlinkers = &page.beam_vlinkers.systems[0];
    let frontier = match &scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => panic!("system 1 lacks V transaction"),
    };
    let attempt = attempt_for_plan(plans, frontier.plan.plan_ordinal).expect("ready plan");
    assert!(
        attempt.relations.len() >= 2,
        "synthetic needs two relations"
    );
    assert_ne!(
        attempt.relations[0].corner.head, attempt.relations[1].corner.head,
        "multi-then-unique uses two independent head snapshots"
    );
    let parameters = relation_parameters(
        plans.interline,
        vlinkers.main_stem_thickness,
        frontier.plan.stem_profile,
    );
    let initial = transaction.stem.as_ref().expect("created stem");
    let ids = unused_persistent_ids(&state, 5);
    assert_eq!(
        ids.len(),
        5,
        "synthetic live stems need five free Sheet IDs"
    );
    let far = translated_live_stem(initial, 1, ids[0], ids[1], 40, 10_000);
    let near = translated_live_stem(initial, 2, ids[2], ids[3], 20, 0);
    let mut same_glyph_second_stem = far.clone();
    same_glyph_second_stem.stem_identity = 3;
    same_glyph_second_stem.inter_id = Some(ids[4]);
    let first = &attempt.relations[0];
    let second = &attempt.relations[1];

    let multi_then_unique = NativeStemsBeamVLinkReuseLiveState {
        system_id,
        live_sig_stems: vec![far.clone(), near.clone()],
        evaluation: NativeStemsBeamVLinkReuseLiveEvaluation::Entries(vec![
            NativeStemsBeamReuseEntryEvidence {
                map_ordinal: first.map_ordinal,
                corner: first.corner,
                observation: NativeStemsBeamReuseEntryObservation::Examined {
                    s_linker_linked: true,
                    head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::Exhaustive(head_scan(
                        first,
                        vec![
                            (10, first.corner.horizontal, far.stem_identity),
                            (11, first.corner.horizontal, near.stem_identity),
                        ],
                        &[far.clone(), near.clone()],
                    )),
                },
            },
            NativeStemsBeamReuseEntryEvidence {
                map_ordinal: second.map_ordinal,
                corner: second.corner,
                observation: NativeStemsBeamReuseEntryObservation::Examined {
                    s_linker_linked: true,
                    head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::Exhaustive(head_scan(
                        second,
                        vec![(12, second.corner.horizontal, far.stem_identity)],
                        &[far.clone(), near.clone()],
                    )),
                },
            },
        ]),
    };
    let state_before = state.clone();
    let selected = evaluate_native_stems_beam_vlink_reuse_check(
        scheduler,
        plans,
        stumps,
        vlinkers,
        &transaction,
        &state,
        &multi_then_unique,
        parameters,
    )
    .expect("multi-then-unique reuse");
    assert_eq!(state, state_before, "reuse evaluation mutated state");
    assert_eq!(
        selected.reuse_disposition,
        NativeStemsBeamReuseDisposition::Selected {
            map_ordinal: 1,
            stem_identity: far.stem_identity,
        }
    );
    assert_eq!(selected.final_stem.as_ref(), Some(&far));
    assert_eq!(
        selected
            .reuse_trace
            .iter()
            .map(|trace| trace.action)
            .collect::<Vec<_>>(),
        vec![
            NativeStemsBeamReuseAction::ContinueMultiplicity,
            NativeStemsBeamReuseAction::SelectUnique {
                stem_identity: far.stem_identity,
            },
        ]
    );
    assert!(matches!(
        selected.outcome,
        NativeStemsBeamVLinkReuseCheckOutcome::BeamStemRejected
    ));
    assert_eq!(
        (
            selected.persistent_id_mutation_count,
            selected.system_stem_mutation_count,
            selected.sig_vertex_mutation_count,
            selected.sig_relation_mutation_count,
            selected.linker_flag_mutation_count,
        ),
        (0, 0, 0, 0, 0)
    );

    let shared_glyph_multiplicity = NativeStemsBeamVLinkReuseLiveState {
        system_id,
        // HeadInter.getSideStems() deduplicates StemInter objects, not their
        // canonical Glyph. These remain two live side stems even though their
        // exact Glyph identity and content are shared.
        live_sig_stems: vec![far.clone(), same_glyph_second_stem.clone()],
        evaluation: NativeStemsBeamVLinkReuseLiveEvaluation::Entries(vec![
            NativeStemsBeamReuseEntryEvidence {
                map_ordinal: first.map_ordinal,
                corner: first.corner,
                observation: NativeStemsBeamReuseEntryObservation::Examined {
                    s_linker_linked: true,
                    head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::Exhaustive(head_scan(
                        first,
                        vec![
                            (50, first.corner.horizontal, far.stem_identity),
                            (
                                51,
                                first.corner.horizontal,
                                same_glyph_second_stem.stem_identity,
                            ),
                        ],
                        &[far.clone(), same_glyph_second_stem.clone()],
                    )),
                },
            },
            NativeStemsBeamReuseEntryEvidence {
                map_ordinal: second.map_ordinal,
                corner: second.corner,
                observation: NativeStemsBeamReuseEntryObservation::Examined {
                    s_linker_linked: false,
                    head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::NotRead,
                },
            },
        ]),
    };
    let retained = evaluate_native_stems_beam_vlink_reuse_check(
        scheduler,
        plans,
        stumps,
        vlinkers,
        &transaction,
        &state,
        &shared_glyph_multiplicity,
        parameters,
    )
    .expect("two StemInter objects sharing one Glyph remain multiplicity two");
    assert_eq!(
        retained.reuse_disposition,
        NativeStemsBeamReuseDisposition::NoUnique
    );
    assert_eq!(retained.final_stem.as_ref(), Some(initial));
    assert_eq!(
        retained.reuse_trace[0].action,
        NativeStemsBeamReuseAction::ContinueMultiplicity
    );

    let unique_then_unread = NativeStemsBeamVLinkReuseLiveState {
        system_id,
        // The exact catalogue is the union of targets in examined scans only;
        // the unread suffix is deliberately not hydrated or validated.
        live_sig_stems: vec![near.clone()],
        evaluation: NativeStemsBeamVLinkReuseLiveEvaluation::Entries(vec![
            NativeStemsBeamReuseEntryEvidence {
                map_ordinal: first.map_ordinal,
                corner: first.corner,
                observation: NativeStemsBeamReuseEntryObservation::Examined {
                    s_linker_linked: true,
                    head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::Exhaustive(head_scan(
                        first,
                        vec![(20, first.corner.horizontal, near.stem_identity)],
                        std::slice::from_ref(&near),
                    )),
                },
            },
            NativeStemsBeamReuseEntryEvidence {
                map_ordinal: second.map_ordinal,
                corner: second.corner,
                observation: NativeStemsBeamReuseEntryObservation::UnreadAfterSelection,
            },
        ]),
    };
    let first_selected = evaluate_native_stems_beam_vlink_reuse_check(
        scheduler,
        plans,
        stumps,
        vlinkers,
        &transaction,
        &state,
        &unique_then_unread,
        parameters,
    )
    .expect("first unique break");
    assert_eq!(first_selected.final_stem.as_ref(), Some(&near));
    assert_eq!(
        first_selected.reuse_trace[1].action,
        NativeStemsBeamReuseAction::UnreadAfterSelection
    );

    let missing_side = NativeStemsBeamVLinkReuseLiveState {
        system_id,
        live_sig_stems: vec![far.clone()],
        evaluation: NativeStemsBeamVLinkReuseLiveEvaluation::Entries(vec![
            NativeStemsBeamReuseEntryEvidence {
                map_ordinal: first.map_ordinal,
                corner: first.corner,
                observation: NativeStemsBeamReuseEntryObservation::Examined {
                    s_linker_linked: true,
                    head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::Exhaustive(head_scan(
                        first,
                        vec![(
                            30,
                            opposite_head(first.corner.horizontal),
                            far.stem_identity,
                        )],
                        std::slice::from_ref(&far),
                    )),
                },
            },
            NativeStemsBeamReuseEntryEvidence {
                map_ordinal: second.map_ordinal,
                corner: second.corner,
                observation: NativeStemsBeamReuseEntryObservation::UnreadAfterSelection,
            },
        ]),
    };
    assert!(matches!(
        evaluate_native_stems_beam_vlink_reuse_check(
            scheduler,
            plans,
            stumps,
            vlinkers,
            &transaction,
            &state,
            &missing_side,
            parameters,
        ),
        Err(NativeStemsBeamVLinkReuseCheckError::JavaNullHeadSideStemSet { map_ordinal: 0, .. })
    ));

    let mut incomplete = multi_then_unique.clone();
    let NativeStemsBeamVLinkReuseLiveEvaluation::Entries(entries) = &mut incomplete.evaluation
    else {
        unreachable!()
    };
    let NativeStemsBeamReuseEntryObservation::Examined {
        head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::Exhaustive(scan),
        ..
    } = &mut entries[0].observation
    else {
        unreachable!()
    };
    scan.scanned_relation_count -= 1;
    assert!(matches!(
        evaluate_native_stems_beam_vlink_reuse_check(
            scheduler,
            plans,
            stumps,
            vlinkers,
            &transaction,
            &state,
            &incomplete,
            parameters,
        ),
        Err(NativeStemsBeamVLinkReuseCheckError::InvalidLiveState { .. })
    ));

    let temporary_target = NativeStemsBeamVLinkReuseLiveState {
        system_id,
        live_sig_stems: vec![initial.clone()],
        evaluation: NativeStemsBeamVLinkReuseLiveEvaluation::Entries(vec![
            NativeStemsBeamReuseEntryEvidence {
                map_ordinal: first.map_ordinal,
                corner: first.corner,
                observation: NativeStemsBeamReuseEntryObservation::Examined {
                    s_linker_linked: true,
                    head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::Exhaustive(
                        unchecked_head_scan(
                            first,
                            vec![(40, first.corner.horizontal, initial.stem_identity)],
                        ),
                    ),
                },
            },
            NativeStemsBeamReuseEntryEvidence {
                map_ordinal: second.map_ordinal,
                corner: second.corner,
                observation: NativeStemsBeamReuseEntryObservation::UnreadAfterSelection,
            },
        ]),
    };
    assert!(matches!(
        evaluate_native_stems_beam_vlink_reuse_check(
            scheduler,
            plans,
            stumps,
            vlinkers,
            &transaction,
            &state,
            &temporary_target,
            parameters,
        ),
        Err(
            NativeStemsBeamVLinkReuseCheckError::TransactionStateInvariant {
                phase: "live SIG stem Inter identity"
            }
        )
    ));
}

#[test]
fn independent_reuse_model_pins_linked_hash_map_scan_and_break() {
    let entries = vec![
        IndependentReuseEntry {
            map_ordinal: 0,
            // The relation's C side is deliberately different: Java selects
            // the map with the parent S-linker's side.
            relation_side: IndependentHeadSide::Right,
            linked: true,
            parent_side: Some(IndependentHeadSide::Left),
            side_stem_sets: vec![IndependentHeadStemSet {
                side: IndependentHeadSide::Left,
                stem_identities: vec![3, 4],
            }],
        },
        IndependentReuseEntry {
            map_ordinal: 1,
            relation_side: IndependentHeadSide::Left,
            linked: true,
            parent_side: Some(IndependentHeadSide::Right),
            side_stem_sets: vec![IndependentHeadStemSet {
                side: IndependentHeadSide::Right,
                stem_identities: vec![9],
            }],
        },
        IndependentReuseEntry {
            map_ordinal: 2,
            relation_side: IndependentHeadSide::Left,
            linked: true,
            // Deliberately malformed but unread after the first unique stem.
            parent_side: None,
            side_stem_sets: Vec::new(),
        },
    ];
    assert_eq!(
        independent_reuse_scan(&entries).unwrap(),
        IndependentReuseTrace {
            disposition: IndependentReuseDisposition::Selected {
                entry_ordinal: 1,
                stem_identity: 9,
            },
            examined_ordinals: vec![0, 1],
            unread_from_ordinal: Some(2),
        }
    );

    let all_unlinked = entries
        .iter()
        .enumerate()
        .map(|(map_ordinal, entry)| IndependentReuseEntry {
            map_ordinal,
            linked: false,
            parent_side: None,
            side_stem_sets: Vec::new(),
            ..entry.clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        independent_reuse_scan(&all_unlinked).unwrap().disposition,
        IndependentReuseDisposition::AllUnlinked
    );

    let no_unique = vec![
        IndependentReuseEntry {
            map_ordinal: 0,
            relation_side: IndependentHeadSide::Left,
            linked: true,
            parent_side: Some(IndependentHeadSide::Left),
            side_stem_sets: vec![IndependentHeadStemSet {
                side: IndependentHeadSide::Left,
                stem_identities: vec![3, 4],
            }],
        },
        IndependentReuseEntry {
            map_ordinal: 1,
            relation_side: IndependentHeadSide::Right,
            linked: true,
            parent_side: Some(IndependentHeadSide::Right),
            side_stem_sets: vec![IndependentHeadStemSet {
                side: IndependentHeadSide::Right,
                stem_identities: vec![5, 6],
            }],
        },
    ];
    assert_eq!(
        independent_reuse_scan(&no_unique).unwrap().disposition,
        IndependentReuseDisposition::NoUnique
    );

    let mut malformed = no_unique[0].clone();
    malformed.parent_side = None;
    assert_eq!(
        independent_reuse_scan(&[malformed]),
        Err(IndependentReuseScanError::MissingParentSide)
    );
    let mut malformed = no_unique[0].clone();
    malformed.side_stem_sets.clear();
    assert_eq!(
        independent_reuse_scan(&[malformed]),
        Err(IndependentReuseScanError::MissingSideStemSet)
    );
    let mut malformed = no_unique[0].clone();
    malformed.side_stem_sets[0].stem_identities.clear();
    assert_eq!(
        independent_reuse_scan(&[malformed]),
        Err(IndependentReuseScanError::EmptySideStemSet)
    );
    let mut malformed = no_unique[0].clone();
    malformed
        .side_stem_sets
        .push(malformed.side_stem_sets[0].clone());
    assert_eq!(
        independent_reuse_scan(&[malformed]),
        Err(IndependentReuseScanError::DuplicateSideStemSet)
    );
}
