// SPDX-License-Identifier: AGPL-3.0-or-later

//! Independent, fail-closed gate for the first stateful SIG mutation in
//! `BeamLinker.VLinker.link`.
//!
//! Boundary 14 starts with boundary 13's `ReadyBeforeSigMutation` product. It
//! conditionally registers the final stem as a SIG vertex, applies the base
//! beam-stem `Link`, and includes the synchronous relation callback. It stops
//! before the B-linker flag, sibling beam links, or head links are changed.
//!
//! The installed Java corpus is intentionally not activated until its Chula
//! fixture and manifest are frozen. The parser and independent state machine
//! below are nevertheless active now, including Java's ordinary suppression
//! outcomes and Rust's prevalidated, zero-mutation error contract.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    path::{Path, PathBuf},
};

use audiveris_image::{
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable},
    section::Bounds,
};
use audiveris_omr::{
    head_scanner_slices::JavaRectangle,
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads,
    native_ledgers::recognize_native_ledgers,
    native_sig::{NativeSigRecognition, assemble_native_sig},
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
    native_stems_beam_vlink_base_apply::{
        NativeStemsBeamBeamIncidentRead, NativeStemsBeamBeamIncidentRelation,
        NativeStemsBeamBeamIncidentRule, NativeStemsBeamBeamIncidentScan,
        NativeStemsBeamCertificateEndpointIdentity, NativeStemsBeamDirectedPairRelation,
        NativeStemsBeamDirectedPairScan, NativeStemsBeamGroupRuntimeState,
        NativeStemsBeamIncidentDirection, NativeStemsBeamIncidentOpposite,
        NativeStemsBeamInterIndexAppend, NativeStemsBeamInterIndexApplyState,
        NativeStemsBeamInterIndexLookup, NativeStemsBeamNextPersistentIdLookup,
        NativeStemsBeamPairClassRead, NativeStemsBeamQueryProvenance,
        NativeStemsBeamQueryRelationKind, NativeStemsBeamRelationObjectIdentity,
        NativeStemsBeamSheetEditState, NativeStemsBeamSigApplyState,
        NativeStemsBeamSigListenerTopology, NativeStemsBeamSigRelationKind,
        NativeStemsBeamSigRelationState, NativeStemsBeamSigVertexAppend,
        NativeStemsBeamSigVertexLookup, NativeStemsBeamStemIncidentRelation,
        NativeStemsBeamStemIncidentScan, NativeStemsBeamStemIncidentScanState,
        NativeStemsBeamVLinkBaseApplyCertificate, NativeStemsBeamVLinkBaseApplyCommit,
        NativeStemsBeamVLinkBaseApplyDisposition, NativeStemsBeamVLinkBaseApplyKey,
        NativeStemsBeamVLinkBaseApplyOperation, NativeStemsBeamVLinkBaseApplyOutcome,
        NativeStemsBeamVLinkBaseApplyState, NativeStemsBeamVLinkBeamAbnormalTrace,
        NativeStemsBeamVLinkBeamRuntimeState, NativeStemsBeamVLinkStemRuntimeState,
        NativeStemsBeamVLinkVertexAction, apply_native_stems_beam_vlink_base_transaction,
        apply_native_stems_beam_vlink_base_transaction_to_native_sig,
        project_native_stems_beam_vlink_base_apply_certificate,
    },
    native_stems_beam_vlink_reuse_check::{
        NativeStemsBeamHeadStemLookupEvidence, NativeStemsBeamRelationParameters,
        NativeStemsBeamReuseDisposition, NativeStemsBeamReuseEntryEvidence,
        NativeStemsBeamReuseEntryObservation, NativeStemsBeamVLinkReuseCheck,
        NativeStemsBeamVLinkReuseCheckOutcome, NativeStemsBeamVLinkReuseLiveEvaluation,
        NativeStemsBeamVLinkReuseLiveState, evaluate_native_stems_beam_vlink_reuse_check,
    },
    native_stems_beam_vlink_transaction::{
        NativeStemsBeamCreateStemDisposition, NativeStemsBeamExhaustiveGlyphEqualsScan,
        NativeStemsBeamExhaustiveGlyphLookup, NativeStemsBeamExhaustiveSystemStemEqualsScan,
        NativeStemsBeamExhaustiveSystemStemLookup, NativeStemsBeamFixedGlyphContent,
        NativeStemsBeamGlyphAliasOrder, NativeStemsBeamGlyphIndexTransactionState,
        NativeStemsBeamGlyphRegistrationAction, NativeStemsBeamPersistentIdState,
        NativeStemsBeamSelectedGlyphBinding, NativeStemsBeamStemCheckerContext,
        NativeStemsBeamStemGrade, NativeStemsBeamSystemStemTransactionState,
        NativeStemsBeamVLinkLineState, NativeStemsBeamVLinkTransaction,
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
        GridLinesRecognition, recognize_grid_lines, recognize_native_beams_with_stem_seeds,
    },
    stem_seeds_step::NativeStemCheckerParameters,
    stems_step::{NativeBeamPortion, NativeStemLine, NativeStemPoint},
};

const INSPECT_PROFILE: i32 = 1;
const STEM_MINIMUM_GRADE: f64 = 0.8 * 0.1;
const ARTIFICIAL_STEM_GRADE: f64 = 0.4;
const CREATE_STEM_SCHEMA: &str = "# schema: stems-beam-create-stem-v1";
const REUSE_CHECK_SCHEMA: &str = "# schema: stems-beam-vlink-reuse-check-v1";

const BASE_APPLY_SCHEMA: &str = "# schema: stems-beam-vlink-base-apply-v1";
const BASE_APPLY_ROW_PREFIX: &str = "stemsbeamvlinkbaseapply";
const BASE_APPLY_MANIFEST_SCHEMA: &str = "# schema: stems-beam-vlink-base-apply-manifest-v1";
const BASE_APPLY_MANIFEST_ENTRY: &str = "stemsbeamvlinkbaseapplymanifestentry";
const BASE_APPLY_MANIFEST_SUMMARY: &str = "stemsbeamvlinkbaseapplymanifestsummary";
const BASE_APPLY_MANIFEST: &str = "rust/oracle/stems-beam-vlink-base-apply-manifest.txt";
const BASE_APPLY_PROBE_SOURCE: &str = "rust/oracle/java/StemsBeamVLinkBaseApplyProbe.java";
const BASE_APPLY_RUNNER_SOURCE: &str = "rust/oracle/java/run-stems-beam-vlink-base-apply.sh";
const CORPUS_PAGES: &[&str] = &[
    "chula",
    "allegretto",
    "batuque",
    "carmen",
    "cucaracha",
    "hove",
    "zizi",
    "BachInvention5",
];
const SYSTEM_ONE_SUPPLEMENTAL_CASES: &[(&str, &str)] = &[
    ("synthetic", "ExistingFullSuccess"),
    ("synthetic", "ExistingHookSuccess"),
    ("synthetic", "ExistingDuplicateSuppress"),
    ("synthetic", "SourceRemovedSuppress"),
    ("synthetic", "TargetRemovedSuppress"),
    ("envelope", "MissingPositiveTargetThrows"),
    ("envelope", "NewVertexListenerThrows"),
    ("envelope", "EarlyEdgeListenerThrows"),
    ("envelope", "LaterEdgeListenerThrows"),
];

// Frozen only after all eight pages produced two byte-identical fresh serial
// passes and the normalized split corpus was independently reconstructed.
const BASE_APPLY_MANIFEST_SHA256: &str =
    "e0d932733c52577d2d112c7aa2a5001e6cd0571be17726815e60aa7919720c80";
const BASE_APPLY_MANIFEST_BODY_SHA256: &str =
    "329f17f1a644b0e868bc4470bfe064fda8dfc6ba40d5830d7ca31c6e214e6c87";
const BASE_APPLY_CORPUS_BODY_SHA256: &str =
    "215a154c416fe0e38616564c22e974236c3ed801f74bd3fb469f523a5acf91e0";
const BASE_APPLY_MANIFEST_LINES: usize = 10;
const BASE_APPLY_MANIFEST_BYTES: usize = 20_812;
const BASE_APPLY_MANIFEST_BODY_LINES: usize = 9;
const BASE_APPLY_MANIFEST_BODY_BYTES: usize = 16_479;
const BASE_APPLY_CORPUS_BODY_LINES: usize = 1_314;
const BASE_APPLY_CORPUS_BODY_BYTES: usize = 1_186_005;
const BASE_APPLY_SPLIT_FIXTURE_LINES: usize = 1_386;
const BASE_APPLY_SPLIT_FIXTURE_BYTES: usize = 1_227_853;
const JGRAPHT_CORE_VERSION: &str = "1.5.2";
const JGRAPHT_CORE_JAR: &str = "jgrapht-core-1.5.2.jar";
const JGRAPHT_CORE_JAR_SHA256: &str =
    "dfa596e9f0d0838f1b5e81dd0cd60e3a76c2c290ac25a0a029ffde58cf5e4c14";
const JGRAPHT_CORE_GRADLE_DECLARATION: &str = "\"org.jgrapht:jgrapht-core:1.5.2\",";

const PREDECESSOR_PATHS: &[(&str, &str)] = &[
    (
        "schedulerFixtureSha256",
        "rust/oracle/stems-beam-scheduler-chula.txt",
    ),
    (
        "expandFixtureSha256",
        "rust/oracle/stems-beam-expand-chula.txt",
    ),
    (
        "createStemFixtureSha256",
        "rust/oracle/stems-beam-create-stem-chula.txt",
    ),
    (
        "reuseCheckFixtureSha256",
        "rust/oracle/stems-beam-vlink-reuse-check-chula.txt",
    ),
];

fn predecessor_paths(page_key: &str) -> Vec<(&'static str, String)> {
    [
        ("schedulerFixtureSha256", "stems-beam-scheduler"),
        ("expandFixtureSha256", "stems-beam-expand"),
        ("createStemFixtureSha256", "stems-beam-create-stem"),
        ("reuseCheckFixtureSha256", "stems-beam-vlink-reuse-check"),
    ]
    .into_iter()
    .map(|(field, stem)| (field, format!("rust/oracle/{stem}-{page_key}.txt")))
    .collect()
}

fn page_image_name(page_key: &str) -> Result<&'static str, String> {
    match page_key {
        "chula" => Ok("chula.png"),
        "allegretto" => Ok("allegretto.png"),
        "batuque" => Ok("batuque.png"),
        "carmen" => Ok("carmen.png"),
        "cucaracha" => Ok("cucaracha.png"),
        "hove" => Ok("hove.png"),
        "zizi" => Ok("zizi.png"),
        "BachInvention5" => Ok("BachInvention5.jpg"),
        value => Err(format!("unknown boundary-14 page key {value}")),
    }
}

fn page_key_from_token(page: &str) -> Result<&str, String> {
    let page_file = page.split_once('#').map_or(page, |(file, _)| file);
    Path::new(page_file)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| "boundary-14 page token lacks a mode stem".to_owned())
}

const SOURCE_PATHS: &[(&str, &str)] = &[
    (
        "beamLinkerSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/stem/BeamLinker.java",
    ),
    (
        "linkSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/relation/Link.java",
    ),
    (
        "sigraphSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/SIGraph.java",
    ),
    (
        "sigListenerSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/SigListener.java",
    ),
    (
        "systemInfoSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/SystemInfo.java",
    ),
    (
        "sheetSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/Sheet.java",
    ),
    (
        "basicIndexSourceSha256",
        "app/src/main/java/org/audiveris/omr/util/BasicIndex.java",
    ),
    (
        "entityIndexSourceSha256",
        "app/src/main/java/org/audiveris/omr/util/EntityIndex.java",
    ),
    (
        "glyphIndexSourceSha256",
        "app/src/main/java/org/audiveris/omr/glyph/GlyphIndex.java",
    ),
    (
        "interIndexSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/InterIndex.java",
    ),
    (
        "abstractEntitySourceSha256",
        "app/src/main/java/org/audiveris/omr/util/AbstractEntity.java",
    ),
    (
        "abstractInterSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/inter/AbstractInter.java",
    ),
    (
        "interSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/inter/Inter.java",
    ),
    (
        "stemInterSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/inter/StemInter.java",
    ),
    (
        "abstractChordInterSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/inter/AbstractChordInter.java",
    ),
    (
        "headChordInterSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/inter/HeadChordInter.java",
    ),
    (
        "relationSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/relation/Relation.java",
    ),
    (
        "supportSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/relation/Support.java",
    ),
    (
        "beamStemRelationSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/relation/BeamStemRelation.java",
    ),
    (
        "beamRestRelationSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/relation/BeamRestRelation.java",
    ),
    (
        "chordStemRelationSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/relation/ChordStemRelation.java",
    ),
    (
        "abstractBeamInterSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/inter/AbstractBeamInter.java",
    ),
    (
        "beamInterSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/inter/BeamInter.java",
    ),
    (
        "beamHookInterSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/inter/BeamHookInter.java",
    ),
    (
        "sheetStubSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/SheetStub.java",
    ),
    (
        "bookSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/Book.java",
    ),
    ("gradleSourceSha256", "app/build.gradle"),
];

const PAGE_FIELDS: &[&str] = &[
    "systems",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "createStemFixtureSha256",
    "reuseCheckFixtureSha256",
    "executionMode",
    "vertexOrder",
    "edgeOrder",
    "incidentOrder",
    "duplicateOrder",
    "listenerMode",
    "headless",
    "registryHashMode",
    "graphHashMode",
];
const BASELINE_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "executionMode",
    "allocator",
    "nextSharedId",
    "nextIdInter",
    "nextIdGlyphActive",
    "nextIdGlyphOriginal",
    "nextIdVipConfigured",
    "interIndex",
    "interIndexHash",
    "interIndexScanned",
    "interIndexBeamIndexOrdinal",
    "interIndexBeamObjectMatches",
    "interIndexBeamIdMatches",
    "glyphActiveBeamIdMatches",
    "glyphOriginalBeamIdMatches",
    "interIndexStemIndexOrdinal",
    "interIndexStemObjectMatches",
    "interIndexStemIdMatches",
    "glyphActiveStemIdMatches",
    "glyphOriginalStemIdMatches",
    "interIndexGeneratedNextIdMatches",
    "glyphActive",
    "glyphActiveHash",
    "glyphOriginals",
    "glyphOriginalsHash",
    "sigVertices",
    "sigVertexHash",
    "sigEdges",
    "sigEdgeHash",
    "sigVertexScanned",
    "sigBeamObjectMatches",
    "sigBeamVertexOrdinal",
    "sigStemObjectMatches",
    "sigStemVertexOrdinal",
    "beamAlias",
    "beamInterId",
    "beamClass",
    "beamAbnormal",
    "beamVip",
    "beamRemoved",
    "beamGroupVertexOrdinal",
    "beamGroupStateHash",
    "beamStateHash",
    "stemAlias",
    "stemInterId",
    "stemSigAttached",
    "stemVip",
    "stemRemoved",
    "stemAbnormal",
    "stemStateHash",
    "stubModified",
    "bookModifiedRaw",
    "bookDirty",
    "listenerHash",
    "noStaff",
    "systemStemsHash",
    "linkerStateHash",
    "lineStateHash",
    "relationInputHash",
];
const PREDECESSOR_COMPAT_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "phase",
    "algorithm",
    "legacyInterIndexHash",
    "legacySigVertices",
    "legacySigEdges",
    "legacySigHash",
    "legacySigRelationStateHash",
    "legacySystemStems",
    "legacySystemStemsHash",
    "legacyLinkerStateHash",
];
const FRONTIER_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
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
    "glyphCount",
    "selectedGlyphRefs",
    "createCandidateObjectIdBefore",
    "createRegistration",
    "createDisposition",
    "createRegisteredAlias",
    "initialStemAlias",
    "finalStemAlias",
    "reuseOutcome",
    "realReuse",
    "freshLinkAlias",
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
    "predecessorJoin",
    "predecessorTerminal",
];
const LISTENER_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "vertexSetCount",
    "vertexSetClasses",
    "graphCount",
    "graphClasses",
    "standardSingleSigListener",
    "callbackDispatch",
];
const VERTEX_TRACE_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "branch",
    "idRead",
    "callAttempted",
    "registerAttempted",
    "allocatorBefore",
    "allocatorAfter",
    "assignedId",
    "vipMembershipRead",
    "vipConfigured",
    "stemVipBefore",
    "stemVipAfter",
    "interIndexBefore",
    "interIndexAfter",
    "vertexCountBefore",
    "vertexCountAfter",
    "interIndexScannedAfterVertex",
    "interIndexStemObjectMatchesAfterVertex",
    "interIndexAssignedIdMatchesAfterVertex",
    "sigVertexScannedAfterVertex",
    "sigBeamObjectMatchesAfterVertex",
    "sigStemObjectMatchesAfterVertex",
    "sigBeamVertexOrdinalAfterVertex",
    "sigStemVertexOrdinalAfterVertex",
    "vertexOrdinal",
    "vertexCallReturn",
    "vertexEventCompleted",
    "stemSigAttachedAfterVertex",
    "stemAddedCompleted",
    "stemRemovedBefore",
    "stemRemovedAfter",
    "abnormalRequest",
    "abnormalChanged",
    "stemAbnormalBefore",
    "stemAbnormalAfter",
    "stubBefore",
    "stubAfter",
    "bookModifiedBefore",
    "bookModifiedAfter",
    "bookDirtyBefore",
    "bookDirtyAfter",
    "throwStage",
    "throwClass",
];
const APPLY_DECISION_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "sourceAlias",
    "sourceInterId",
    "targetAlias",
    "targetInterId",
    "sourceRemovedRead",
    "sourceRemoved",
    "targetRemovedRead",
    "targetRemoved",
    "duplicateLookup",
    "sourceOutgoingScanned",
    "sourceOutgoingProvenanceSha256",
    "pairEdges",
    "classChecksRead",
    "unreadSuffix",
    "firstMatchIdentity",
    "pairProvenanceSha256",
    "firstMatchObjectIdentity",
    "action",
    "relationRuntimeClass",
];
const DUPLICATE_SCAN_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "pairOrdinal",
    "sourceOutgoingOrdinal",
    "globalRelationIdentity",
    "relationObjectIdentity",
    "relationClass",
    "classCheckRead",
    "matchesRuntimeClass",
    "action",
];
const EDGE_STRUCT_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "applyCalled",
    "applyReturn",
    "freshDraftIdentity",
    "draftInGraphBefore",
    "draftInGraphAfter",
    "graphRelationIdentity",
    "globalEdgeOrdinal",
    "sourceOutgoingOrdinal",
    "targetIncomingOrdinal",
    "existingRelationIdentity",
    "existingRelationObjectIdentity",
    "globalInserted",
    "sourceOutgoingInserted",
    "targetIncomingInserted",
    "edgeEventCompleted",
    "callbackOutcome",
    "throwStage",
    "throwClass",
    "throwMessageHash",
];
const STEM_INCIDENT_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "phase",
    "incidentOrdinal",
    "direction",
    "directionOrdinal",
    "globalRelationIdentity",
    "relationObjectIdentity",
    "relationClass",
    "oppositeAlias",
    "oppositeInterId",
    "oppositeVertexOrdinal",
    "chordStemMatch",
    "action",
];
const RELATION_CALLBACK_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "outcome",
    "relationBeforeHash",
    "relationAfterHash",
    "extensionBefore",
    "extensionAfter",
    "portionBefore",
    "portionAfter",
    "preStemScanState",
    "preStemIncidentCount",
    "preStemIncidentHash",
    "stemScanState",
    "stemIncidentCount",
    "stemIncidentHash",
    "chordMatches",
    "chordInvalidations",
    "beamCheckCalled",
    "preBeamScanState",
    "preBeamIncidentCount",
    "preBeamIncidentHash",
    "beamScanState",
    "beamRule",
    "beamIncidentCount",
    "beamIncidentHash",
    "left",
    "right",
    "requestedAbnormal",
    "beamAbnormalBefore",
    "beamAbnormalAfter",
    "beamAbnormalChanged",
    "stubBefore",
    "stubAfter",
    "bookModifiedBefore",
    "bookModifiedAfter",
    "bookDirtyBefore",
    "bookDirtyAfter",
];
const BEAM_INCIDENT_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "phase",
    "incidentOrdinal",
    "direction",
    "directionOrdinal",
    "globalRelationIdentity",
    "relationObjectIdentity",
    "relationClass",
    "oppositeAlias",
    "oppositeInterId",
    "oppositeVertexOrdinal",
    "readState",
    "relevant",
    "portion",
    "contribution",
];
const RESULT_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "terminal",
    "branch",
    "retainedDraftIdentity",
    "retainedDraftGrade",
    "applyReturn",
    "graphRelationIdentity",
    "existingRelationIdentity",
    "existingRelationObjectIdentity",
    "finalStemAlias",
    "finalStemInterId",
    "finalStemSigAttached",
    "finalStemVip",
    "finalStemAbnormal",
    "finalBeamAbnormal",
    "finalBeamVip",
    "finalBeamRemoved",
    "finalBeamGroupVertexOrdinal",
    "finalBeamGroupStateHash",
    "allocator",
    "interIndexHash",
    "sigVertexHash",
    "sigEdgeHash",
    "stubModified",
    "bookModifiedRaw",
    "bookDirty",
];
const DELTA_GUARD_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "allocatorBefore",
    "allocatorAfter",
    "interIndexHashBefore",
    "interIndexHashAfter",
    "glyphActiveHashBefore",
    "glyphActiveHashAfter",
    "glyphOriginalsHashBefore",
    "glyphOriginalsHashAfter",
    "sigVertexHashBefore",
    "sigVertexHashAfter",
    "sigEdgeHashBefore",
    "sigEdgeHashAfter",
    "stemGeometryUnchanged",
    "beamGeometryUnchanged",
    "beamGroupUnchanged",
    "beamGroupVertexOrdinalBefore",
    "beamGroupVertexOrdinalAfter",
    "beamGroupStateHashBefore",
    "beamGroupStateHashAfter",
    "glyphRegistriesUnchanged",
    "systemStemsMapIdentityUnchanged",
    "linesUnchanged",
    "linkerFlagsUnchanged",
    "headRelationsUnchanged",
    "siblingBeamRelationsUnchanged",
    "allowedMutation",
    "stopBeforeBLinkerFlagMutation",
    "stopBeforeSiblingBeamLinks",
    "stopBeforeHeadRelationLoop",
];
const SYNTHETIC_DELTA_GUARD_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "observedSheet",
    "enclosingRealSheetHashBefore",
    "enclosingRealSheetHashAfter",
    "enclosingRealSheetUnchanged",
    "allocatorBefore",
    "allocatorAfter",
    "interIndexHashBefore",
    "interIndexHashAfter",
    "glyphActiveHashBefore",
    "glyphActiveHashAfter",
    "glyphOriginalsHashBefore",
    "glyphOriginalsHashAfter",
    "sigVertexHashBefore",
    "sigVertexHashAfter",
    "sigEdgeHashBefore",
    "sigEdgeHashAfter",
    "stemGeometryUnchanged",
    "beamGeometryUnchanged",
    "beamGroupUnchanged",
    "beamGroupVertexOrdinalBefore",
    "beamGroupVertexOrdinalAfter",
    "beamGroupStateHashBefore",
    "beamGroupStateHashAfter",
    "glyphRegistriesUnchanged",
    "systemStemsMapIdentityUnchanged",
    "linesUnchanged",
    "linkerFlagsUnchanged",
    "headRelationsUnchanged",
    "siblingBeamRelationsUnchanged",
    "allowedMutation",
    "stopBeforeBLinkerFlagMutation",
    "stopBeforeSiblingBeamLinks",
    "stopBeforeHeadRelationLoop",
];
const SUMMARY_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "branch",
    "vertexAdded",
    "applyReturned",
    "edgeAdded",
    "chordMatches",
    "beamRule",
    "terminal",
    "throwStage",
];
const PAGE_SUMMARY_FIELDS: &[&str] = &[
    "systems",
    "realTransactions",
    "syntheticSupportedCases",
    "envelopeCases",
    "totalTransactions",
    "realNewVertexBranches",
    "realExistingVertexBranches",
    "realEdgesAdded",
    "realReuseCensus",
    "chordStemMatches",
    "isolatedSheetsPerSyntheticCase",
    "stopBeforeBLinkerFlagMutation",
];
const CORPUS_SUMMARY_FIELDS: &[&str] = &[
    "schema",
    "mode",
    "pages",
    "pageRefs",
    "rowCounts",
    "probeSourceSha256",
    "runnerSourceSha256",
    "beamLinkerSourceSha256",
    "linkSourceSha256",
    "sigraphSourceSha256",
    "sigListenerSourceSha256",
    "systemInfoSourceSha256",
    "sheetSourceSha256",
    "basicIndexSourceSha256",
    "entityIndexSourceSha256",
    "glyphIndexSourceSha256",
    "interIndexSourceSha256",
    "abstractEntitySourceSha256",
    "abstractInterSourceSha256",
    "interSourceSha256",
    "stemInterSourceSha256",
    "abstractChordInterSourceSha256",
    "headChordInterSourceSha256",
    "relationSourceSha256",
    "supportSourceSha256",
    "beamStemRelationSourceSha256",
    "beamRestRelationSourceSha256",
    "chordStemRelationSourceSha256",
    "abstractBeamInterSourceSha256",
    "beamInterSourceSha256",
    "beamHookInterSourceSha256",
    "sheetStubSourceSha256",
    "bookSourceSha256",
    "gradleSourceSha256",
    "jgraphtCoreVersion",
    "jgraphtCoreJarSha256",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "createStemFixtureSha256",
    "reuseCheckFixtureSha256",
    "predecessorSwapNegative",
    "emittedBodySha256",
    "emittedBodyLines",
    "emittedBodyBytes",
    "freshRunsPerPage",
    "freshRunsByteIdentical",
    "rawPassSha256",
    "freshJvmPerSystem",
    "compilerJavaProcesses",
    "runtimeJavaProcessesPerPass",
    "runtimeJavaProcesses",
    "totalJavaProcesses",
    "maximumConcurrentJavaProcesses",
    "compilerJavaProcessReaped",
    "runtimeJavaProcessesReaped",
    "foregroundJavaProcessesOnly",
    "backgroundJavaProcessesStarted",
    "opaqueUnrelatedIdsExcludedFromStructuralHashes",
];
const MANIFEST_ENTRY_FIELDS: &[&str] = &[
    "ordinal",
    "page",
    "fixture",
    "rowCounts",
    "systems",
    "realTransactions",
    "syntheticSupportedCases",
    "envelopeCases",
    "totalTransactions",
    "predecessorCompatRows",
    "realNewVertexBranches",
    "realExistingVertexBranches",
    "realEdgesAdded",
    "realReuseCensus",
    "chordStemMatches",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "createStemFixtureSha256",
    "reuseCheckFixtureSha256",
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
    "runtimeJavaProcessesPerPass",
    "runtimeJavaProcesses",
    "totalJavaProcesses",
    "maximumConcurrentJavaProcesses",
    "freshJvmPerSystem",
    "compilerJavaProcessReaped",
    "runtimeJavaProcessesReaped",
    "foregroundJavaProcessesOnly",
    "backgroundJavaProcessesStarted",
    "system1SyntheticBlock",
    "isolatedSheetsPerSyntheticCase",
    "stopBeforeBLinkerFlagMutation",
    "opaqueUnrelatedIdsExcludedFromStructuralHashes",
];
const MANIFEST_SUMMARY_FIELDS: &[&str] = &[
    "schema",
    "entries",
    "probeSourceSha256",
    "runnerSourceSha256",
    "beamLinkerSourceSha256",
    "linkSourceSha256",
    "sigraphSourceSha256",
    "sigListenerSourceSha256",
    "systemInfoSourceSha256",
    "sheetSourceSha256",
    "basicIndexSourceSha256",
    "entityIndexSourceSha256",
    "glyphIndexSourceSha256",
    "interIndexSourceSha256",
    "abstractEntitySourceSha256",
    "abstractInterSourceSha256",
    "interSourceSha256",
    "stemInterSourceSha256",
    "abstractChordInterSourceSha256",
    "headChordInterSourceSha256",
    "relationSourceSha256",
    "supportSourceSha256",
    "beamStemRelationSourceSha256",
    "beamRestRelationSourceSha256",
    "chordStemRelationSourceSha256",
    "abstractBeamInterSourceSha256",
    "beamInterSourceSha256",
    "beamHookInterSourceSha256",
    "sheetStubSourceSha256",
    "bookSourceSha256",
    "gradleSourceSha256",
    "jgraphtCoreVersion",
    "jgraphtCoreJarSha256",
    "corpusBodySha256",
    "corpusBodyLines",
    "corpusBodyBytes",
    "corpusRowCounts",
    "splitFixtureLines",
    "splitFixtureBytes",
    "realSystems",
    "realTransactions",
    "predecessorCompatRows",
    "realNewVertexBranches",
    "realExistingVertexBranches",
    "realEdgesAdded",
    "realReuseCensus",
    "chordStemMatches",
    "syntheticBlocks",
    "syntheticSupportedCases",
    "envelopeCases",
    "totalTransactions",
    "compilerJavaProcesses",
    "runtimeJavaProcesses",
    "totalJavaProcesses",
    "maximumConcurrentJavaProcesses",
    "freshRunsPerPage",
    "freshRunsByteIdentical",
    "freshJvmPerSystem",
    "compilerJavaProcessesReaped",
    "runtimeJavaProcessesReaped",
    "foregroundJavaProcessesOnly",
    "backgroundJavaProcessesStarted",
    "isolatedSheetsPerSyntheticCase",
    "stopBeforeBLinkerFlagMutation",
    "opaqueUnrelatedIdsExcludedFromStructuralHashes",
    "manifestBodySha256",
    "manifestBodyLines",
    "manifestBodyBytes",
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum RowKind {
    Page,
    Baseline,
    PredecessorCompat,
    Frontier,
    Listeners,
    VertexTrace,
    ApplyDecision,
    DuplicateScan,
    EdgeStruct,
    StemIncident,
    RelationCallback,
    BeamIncident,
    Result,
    DeltaGuard,
    Summary,
    PageSummary,
    CorpusSummary,
}

impl RowKind {
    fn suffix(self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::Baseline => "baseline",
            Self::PredecessorCompat => "predecessorcompat",
            Self::Frontier => "frontier",
            Self::Listeners => "listeners",
            Self::VertexTrace => "vertextrace",
            Self::ApplyDecision => "applydecision",
            Self::DuplicateScan => "duplicatescan",
            Self::EdgeStruct => "edgestruct",
            Self::StemIncident => "stemincident",
            Self::RelationCallback => "relationcallback",
            Self::BeamIncident => "beamincident",
            Self::Result => "result",
            Self::DeltaGuard => "deltaguard",
            Self::Summary => "summary",
            Self::PageSummary => "pagesummary",
            Self::CorpusSummary => "corpussummary",
        }
    }

    fn parse(label: &str) -> Result<Self, String> {
        let suffix = label
            .strip_prefix(BASE_APPLY_ROW_PREFIX)
            .ok_or_else(|| format!("unexpected boundary-14 row label {label}"))?;
        match suffix {
            "page" => Ok(Self::Page),
            "baseline" => Ok(Self::Baseline),
            "predecessorcompat" => Ok(Self::PredecessorCompat),
            "frontier" => Ok(Self::Frontier),
            "listeners" => Ok(Self::Listeners),
            "vertextrace" => Ok(Self::VertexTrace),
            "applydecision" => Ok(Self::ApplyDecision),
            "duplicatescan" => Ok(Self::DuplicateScan),
            "edgestruct" => Ok(Self::EdgeStruct),
            "stemincident" => Ok(Self::StemIncident),
            "relationcallback" => Ok(Self::RelationCallback),
            "beamincident" => Ok(Self::BeamIncident),
            "result" => Ok(Self::Result),
            "deltaguard" => Ok(Self::DeltaGuard),
            "summary" => Ok(Self::Summary),
            "pagesummary" => Ok(Self::PageSummary),
            "corpussummary" => Ok(Self::CorpusSummary),
            _ => Err(format!("unknown boundary-14 row family {label}")),
        }
    }

    fn system_rank(self) -> Option<usize> {
        match self {
            Self::Baseline => Some(0),
            Self::PredecessorCompat => Some(1),
            Self::Frontier => Some(2),
            Self::Listeners => Some(3),
            Self::VertexTrace => Some(4),
            Self::ApplyDecision => Some(5),
            Self::DuplicateScan => Some(6),
            Self::EdgeStruct => Some(7),
            Self::StemIncident => Some(8),
            Self::RelationCallback => Some(9),
            Self::BeamIncident => Some(10),
            Self::Result => Some(11),
            Self::DeltaGuard => Some(12),
            Self::Summary => Some(13),
            Self::Page | Self::PageSummary | Self::CorpusSummary => None,
        }
    }

    fn expected_fields(self) -> Option<&'static [&'static str]> {
        match self {
            Self::Page => Some(PAGE_FIELDS),
            Self::Baseline => Some(BASELINE_FIELDS),
            Self::PredecessorCompat => Some(PREDECESSOR_COMPAT_FIELDS),
            Self::Frontier => Some(FRONTIER_FIELDS),
            Self::Listeners => Some(LISTENER_FIELDS),
            Self::VertexTrace => Some(VERTEX_TRACE_FIELDS),
            Self::ApplyDecision => Some(APPLY_DECISION_FIELDS),
            Self::DuplicateScan => Some(DUPLICATE_SCAN_FIELDS),
            Self::EdgeStruct => Some(EDGE_STRUCT_FIELDS),
            Self::StemIncident => Some(STEM_INCIDENT_FIELDS),
            Self::RelationCallback => Some(RELATION_CALLBACK_FIELDS),
            Self::BeamIncident => Some(BEAM_INCIDENT_FIELDS),
            Self::Result => Some(RESULT_FIELDS),
            Self::DeltaGuard => Some(DELTA_GUARD_FIELDS),
            Self::Summary => Some(SUMMARY_FIELDS),
            Self::PageSummary => Some(PAGE_SUMMARY_FIELDS),
            Self::CorpusSummary => Some(CORPUS_SUMMARY_FIELDS),
        }
    }

    fn expected_fields_for_scope(self, scope: Option<&str>) -> Option<&'static [&'static str]> {
        if self == Self::DeltaGuard && scope.is_some_and(|scope| scope != "real") {
            Some(SYNTHETIC_DELTA_GUARD_FIELDS)
        } else {
            self.expected_fields()
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StrictRow {
    kind: RowKind,
    page: String,
    fields: Vec<(String, String)>,
}

impl StrictRow {
    fn parse(line: &str) -> Result<Self, String> {
        let tokens = line.split_whitespace().collect::<Vec<_>>();
        if tokens.is_empty() {
            return Err(format!("malformed boundary-14 row: {line}"));
        }
        let kind = RowKind::parse(tokens[0])?;
        let pair_start = if kind == RowKind::CorpusSummary { 1 } else { 2 };
        if tokens.len() < pair_start || (tokens.len() - pair_start) % 2 != 0 {
            return Err(format!("malformed boundary-14 row: {line}"));
        }
        let mut names = BTreeSet::new();
        let mut fields = Vec::with_capacity((tokens.len() - pair_start) / 2);
        for pair in tokens[pair_start..].chunks_exact(2) {
            if !names.insert(pair[0]) {
                return Err(format!("duplicate field {} in {}", pair[0], tokens[0]));
            }
            fields.push((pair[0].to_owned(), pair[1].to_owned()));
        }
        let scope = fields
            .iter()
            .find_map(|(name, value)| (name == "scope").then_some(value.as_str()));
        if let Some(expected) = kind.expected_fields_for_scope(scope) {
            let actual = fields
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>();
            if actual != expected {
                return Err(format!(
                    "{kind:?} field order differs: expected {expected:?}, got {actual:?}"
                ));
            }
        }
        let page = if kind == RowKind::CorpusSummary {
            fields
                .iter()
                .find_map(|(name, value)| (name == "pageRefs").then(|| value.clone()))
                .ok_or_else(|| "corpus summary lacks pageRefs".to_owned())?
        } else {
            tokens[1].to_owned()
        };
        Ok(Self { kind, page, fields })
    }

    fn value(&self, name: &str) -> Result<&str, String> {
        self.fields
            .iter()
            .find_map(|(field, value)| (field == name).then_some(value.as_str()))
            .ok_or_else(|| format!("{:?} row lacks {name}", self.kind))
    }

    fn system_id(&self) -> Result<usize, String> {
        let system = self
            .value("system")?
            .parse::<usize>()
            .map_err(|error| format!("invalid system ID: {error}"))?;
        if system == 0 {
            return Err("system IDs are one-based".to_owned());
        }
        Ok(system)
    }

    fn transaction_key(&self) -> Result<(&str, &str), String> {
        let scope = self.value("scope")?;
        let case = self.value("case")?;
        match (scope, case) {
            ("real", "-") => Ok((scope, case)),
            ("synthetic" | "envelope", "-") | ("real", _) => Err(format!(
                "invalid boundary-14 scope/case pair {scope}/{case}"
            )),
            ("synthetic" | "envelope", _) => Ok((scope, case)),
            _ => Err(format!("invalid boundary-14 scope {scope}")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StrictFixture {
    page: StrictRow,
    systems: BTreeMap<usize, Vec<StrictRow>>,
    page_summary: StrictRow,
    corpus_summary: StrictRow,
}

impl StrictFixture {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes)
            .map_err(|error| format!("boundary-14 fixture is not UTF-8: {error}"))?;
        if !text.ends_with('\n') {
            return Err("boundary-14 fixture lacks a terminal newline".to_owned());
        }
        let mut schema_count = 0_usize;
        let mut saw_data = false;
        let mut rows = Vec::new();
        for line in text.lines() {
            if line == BASE_APPLY_SCHEMA {
                schema_count += 1;
                if saw_data {
                    return Err("schema marker appears after fixture data".to_owned());
                }
            } else if line.starts_with('#') {
                if saw_data {
                    return Err("unauthenticated trailing fixture comment".to_owned());
                }
            } else if !line.is_empty() {
                saw_data = true;
                rows.push(StrictRow::parse(line)?);
            } else if saw_data {
                return Err("unauthenticated blank line after fixture data".to_owned());
            }
        }
        if schema_count != 1 {
            return Err(format!(
                "boundary-14 fixture has {schema_count} schema markers"
            ));
        }
        if rows.first().map(|row| row.kind) != Some(RowKind::Page) {
            return Err("boundary-14 page row is not first".to_owned());
        }
        let page = rows.remove(0);
        if rows.iter().any(|row| row.page != page.page) {
            return Err("boundary-14 page token drift".to_owned());
        }

        let mut systems = BTreeMap::<usize, Vec<StrictRow>>::new();
        let mut page_summary = None;
        let mut corpus_summary = None;
        let mut trailer_started = false;
        let mut current_system = None;
        for row in rows {
            match row.kind {
                RowKind::Page => return Err("duplicate page row".to_owned()),
                RowKind::PageSummary => {
                    if corpus_summary.is_some() {
                        return Err("page summary appears after corpus summary".to_owned());
                    }
                    if page_summary.replace(row).is_some() {
                        return Err("duplicate page summary".to_owned());
                    }
                    trailer_started = true;
                }
                RowKind::CorpusSummary => {
                    if page_summary.is_none() {
                        return Err("corpus summary appears before page summary".to_owned());
                    }
                    if corpus_summary.replace(row).is_some() {
                        return Err("duplicate corpus summary".to_owned());
                    }
                    trailer_started = true;
                }
                _ => {
                    if trailer_started {
                        return Err("system row appears after fixture trailer".to_owned());
                    }
                    let system_id = row.system_id()?;
                    match current_system {
                        None if system_id != 1 => {
                            return Err("first boundary-14 system is not one".to_owned());
                        }
                        Some(current) if system_id < current || system_id > current + 1 => {
                            return Err("boundary-14 system groups are not contiguous".to_owned());
                        }
                        _ => {}
                    }
                    current_system = Some(system_id);
                    systems.entry(system_id).or_default().push(row);
                }
            }
        }
        if systems.is_empty() {
            return Err("boundary-14 fixture has no systems".to_owned());
        }
        for (expected, actual) in (1..=systems.len()).zip(systems.keys().copied()) {
            if expected != actual {
                return Err("boundary-14 systems are not dense/in order".to_owned());
            }
        }
        for (system_id, system_rows) in &systems {
            validate_system_row_order(*system_id, system_rows)?;
        }
        let page_summary = page_summary.ok_or_else(|| "fixture lacks page summary".to_owned())?;
        let corpus_summary =
            corpus_summary.ok_or_else(|| "fixture lacks terminal corpus summary".to_owned())?;
        Ok(Self {
            page,
            systems,
            page_summary,
            corpus_summary,
        })
    }
}

fn validate_fixture_trailer(fixture: &StrictFixture, bytes: &[u8]) -> Result<(), String> {
    let page_summary = &fixture.page_summary;
    let corpus = &fixture.corpus_summary;
    let systems = fixture.systems.len();
    let expected_mode = page_key_from_token(&fixture.page.page)?;
    if page_summary.page != fixture.page.page
        || parse_usize(page_summary, "systems")? != systems
        || parse_usize(page_summary, "realTransactions")? != systems
        || parse_usize(page_summary, "syntheticSupportedCases")? != 5
        || parse_usize(page_summary, "envelopeCases")? != 4
        || parse_usize(page_summary, "totalTransactions")? != systems + 9
        || parse_usize(page_summary, "realNewVertexBranches")? != systems
        || parse_usize(page_summary, "realExistingVertexBranches")? != 0
        || parse_usize(page_summary, "realEdgesAdded")? != systems
        || parse_usize(page_summary, "realReuseCensus")? != 0
        || parse_usize(page_summary, "chordStemMatches")? != 0
        || !parse_bool(page_summary, "isolatedSheetsPerSyntheticCase")?
        || !parse_bool(page_summary, "stopBeforeBLinkerFlagMutation")?
    {
        return Err("boundary-14 page summary algebra differs".to_owned());
    }
    if corpus.value("schema")? != "stems-beam-vlink-base-apply-v1"
        || corpus.value("mode")? != expected_mode
        || corpus.value("pages")? != "1"
        || corpus.value("pageRefs")? != fixture.page.page
        || corpus.value("jgraphtCoreVersion")? != JGRAPHT_CORE_VERSION
        || corpus.value("predecessorSwapNegative")? != "true"
        || corpus.value("freshRunsPerPage")? != "2"
        || corpus.value("freshRunsByteIdentical")? != "true"
        || corpus.value("freshJvmPerSystem")? != "true"
        || parse_usize(corpus, "compilerJavaProcesses")? != 1
        || parse_usize(corpus, "runtimeJavaProcessesPerPass")? != systems
        || parse_usize(corpus, "runtimeJavaProcesses")? != systems * 2
        || parse_usize(corpus, "totalJavaProcesses")? != systems * 2 + 1
        || parse_usize(corpus, "maximumConcurrentJavaProcesses")? != 1
        || corpus.value("compilerJavaProcessReaped")? != "true"
        || corpus.value("runtimeJavaProcessesReaped")? != "true"
        || corpus.value("foregroundJavaProcessesOnly")? != "true"
        || corpus.value("backgroundJavaProcessesStarted")? != "0"
        || corpus.value("opaqueUnrelatedIdsExcludedFromStructuralHashes")? != "true"
    {
        return Err("boundary-14 corpus execution provenance differs".to_owned());
    }
    for field in [
        "schedulerFixtureSha256",
        "expandFixtureSha256",
        "createStemFixtureSha256",
        "reuseCheckFixtureSha256",
    ] {
        if corpus.value(field)? != fixture.page.value(field)? {
            return Err(format!("page/corpus {field} differs"));
        }
    }
    for (field, relative) in predecessor_paths(expected_mode) {
        validate_provenance_pin(&repo_root().join(relative), corpus.value(field)?)?;
    }
    validate_provenance_pin(
        &repo_root().join(BASE_APPLY_PROBE_SOURCE),
        corpus.value("probeSourceSha256")?,
    )?;
    validate_provenance_pin(
        &repo_root().join(BASE_APPLY_RUNNER_SOURCE),
        corpus.value("runnerSourceSha256")?,
    )?;
    for (field, relative) in SOURCE_PATHS {
        validate_provenance_pin(&repo_root().join(relative), corpus.value(field)?)?;
    }
    validate_jgrapht_core_provenance(corpus.value("jgraphtCoreJarSha256")?)?;

    let marker = format!("{BASE_APPLY_ROW_PREFIX}pagesummary ");
    let starts = bytes
        .windows(marker.len())
        .enumerate()
        .filter_map(|(index, window)| (window == marker.as_bytes()).then_some(index))
        .collect::<Vec<_>>();
    let [body_end] = starts.as_slice() else {
        return Err("page summary marker is not unique".to_owned());
    };
    let body = &bytes[..*body_end];
    let body_hash = sha256_hex(body);
    let body_lines = body.iter().filter(|byte| **byte == b'\n').count();
    if corpus.value("emittedBodySha256")? != body_hash
        || corpus.value("rawPassSha256")? != body_hash
        || parse_usize(corpus, "emittedBodyLines")? != body_lines
        || parse_usize(corpus, "emittedBodyBytes")? != body.len()
    {
        return Err("boundary-14 emitted-body provenance differs".to_owned());
    }
    let mut observed_counts = BTreeMap::<&str, usize>::new();
    for line in std::str::from_utf8(body)
        .map_err(|error| format!("body is not UTF-8: {error}"))?
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        if let Some(label) = line.split_ascii_whitespace().next() {
            *observed_counts.entry(label).or_default() += 1;
        }
    }
    let row_counts = corpus.value("rowCounts")?.split(',').collect::<Vec<_>>();
    let mut seen_labels = BTreeSet::new();
    for token in row_counts {
        let (label, count) = token
            .split_once(':')
            .ok_or_else(|| format!("malformed rowCounts token {token}"))?;
        if !seen_labels.insert(label)
            || count.parse::<usize>().ok() != observed_counts.get(label).copied()
        {
            return Err(format!("rowCounts token differs: {token}"));
        }
    }
    if observed_counts
        .keys()
        .any(|label| label.starts_with(BASE_APPLY_ROW_PREFIX) && !seen_labels.contains(label))
    {
        return Err("rowCounts omits a boundary-14 family".to_owned());
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StrictManifestRow {
    fields: Vec<(String, String)>,
}

impl StrictManifestRow {
    fn parse(line: &str, expected_label: &str) -> Result<Self, String> {
        let tokens = line.split_whitespace().collect::<Vec<_>>();
        if tokens.first().copied() != Some(expected_label)
            || tokens.len() < 3
            || (tokens.len() - 1) % 2 != 0
        {
            return Err(format!("malformed boundary-14 manifest row: {line}"));
        }
        let mut names = BTreeSet::new();
        let mut fields = Vec::with_capacity((tokens.len() - 1) / 2);
        for pair in tokens[1..].chunks_exact(2) {
            if !names.insert(pair[0]) {
                return Err(format!("duplicate manifest field {}", pair[0]));
            }
            fields.push((pair[0].to_owned(), pair[1].to_owned()));
        }
        let expected = if expected_label == BASE_APPLY_MANIFEST_ENTRY {
            MANIFEST_ENTRY_FIELDS
        } else if expected_label == BASE_APPLY_MANIFEST_SUMMARY {
            MANIFEST_SUMMARY_FIELDS
        } else {
            return Err(format!(
                "unknown boundary-14 manifest label {expected_label}"
            ));
        };
        let actual = fields
            .iter()
            .map(|(field, _)| field.as_str())
            .collect::<Vec<_>>();
        if actual != expected {
            return Err(format!(
                "manifest field order differs: expected {expected:?}, got {actual:?}"
            ));
        }
        Ok(Self { fields })
    }

    fn value(&self, name: &str) -> Result<&str, String> {
        self.fields
            .iter()
            .find_map(|(field, value)| (field == name).then_some(value.as_str()))
            .ok_or_else(|| format!("manifest row lacks {name}"))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StrictManifest {
    entries: Vec<StrictManifestRow>,
    summary: StrictManifestRow,
}

impl StrictManifest {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes)
            .map_err(|error| format!("boundary-14 manifest is not UTF-8: {error}"))?;
        if !text.ends_with('\n') {
            return Err("boundary-14 manifest lacks a terminal newline".to_owned());
        }
        let mut schema_count = 0_usize;
        let mut saw_data = false;
        let mut data = Vec::new();
        for line in text.lines() {
            if line == BASE_APPLY_MANIFEST_SCHEMA {
                schema_count += 1;
                if saw_data {
                    return Err("manifest schema appears after data".to_owned());
                }
            } else if line.starts_with('#') {
                if saw_data {
                    return Err("unauthenticated trailing manifest comment".to_owned());
                }
            } else if line.is_empty() {
                if saw_data {
                    return Err("unauthenticated blank line after manifest data".to_owned());
                }
            } else {
                saw_data = true;
                data.push(line);
            }
        }
        if schema_count != 1 {
            return Err(format!(
                "boundary-14 manifest has {schema_count} schema markers"
            ));
        }
        let (summary_line, entry_lines) = data
            .split_last()
            .ok_or_else(|| "boundary-14 manifest has no data".to_owned())?;
        if !summary_line.starts_with(BASE_APPLY_MANIFEST_SUMMARY)
            || entry_lines
                .iter()
                .any(|line| line.starts_with(BASE_APPLY_MANIFEST_SUMMARY))
        {
            return Err("manifest summary is not the unique final row".to_owned());
        }
        let summary = StrictManifestRow::parse(summary_line, BASE_APPLY_MANIFEST_SUMMARY)?;
        let entries = entry_lines
            .iter()
            .map(|line| StrictManifestRow::parse(line, BASE_APPLY_MANIFEST_ENTRY))
            .collect::<Result<Vec<_>, _>>()?;
        if entries.len() != CORPUS_PAGES.len() {
            return Err(format!(
                "manifest has {} entries instead of {}",
                entries.len(),
                CORPUS_PAGES.len()
            ));
        }
        for (index, (entry, expected_page)) in entries.iter().zip(CORPUS_PAGES).enumerate() {
            let ordinal = entry
                .value("ordinal")?
                .parse::<usize>()
                .map_err(|error| format!("invalid manifest ordinal: {error}"))?;
            if ordinal != index || entry.value("page")? != *expected_page {
                return Err("manifest page order differs".to_owned());
            }
            let expected_fixture = format!("stems-beam-vlink-base-apply-{expected_page}.txt");
            if entry.value("fixture")? != expected_fixture {
                return Err("manifest fixture name differs".to_owned());
            }
        }
        if summary.value("schema")? != "stems-beam-vlink-base-apply-manifest-v1"
            || summary.value("entries")? != CORPUS_PAGES.len().to_string()
        {
            return Err("manifest summary schema/count differs".to_owned());
        }
        Ok(Self { entries, summary })
    }
}

#[derive(Clone, Copy, Debug)]
struct BaseApplyFixtureSlices<'a> {
    header: &'a [u8],
    semantic_body: &'a [u8],
    emitted_body: &'a [u8],
}

fn unique_marker(bytes: &[u8], marker: &[u8], description: &str) -> Result<usize, String> {
    let starts = bytes
        .windows(marker.len())
        .enumerate()
        .filter_map(|(index, window)| (window == marker).then_some(index))
        .collect::<Vec<_>>();
    let [start] = starts.as_slice() else {
        return Err(format!("{description} marker is not unique"));
    };
    Ok(*start)
}

fn base_apply_fixture_slices(bytes: &[u8]) -> Result<BaseApplyFixtureSlices<'_>, String> {
    let page_start = unique_marker(
        bytes,
        format!("{BASE_APPLY_ROW_PREFIX}page ").as_bytes(),
        "boundary-14 page",
    )?;
    let trailer_start = unique_marker(
        bytes,
        format!("{BASE_APPLY_ROW_PREFIX}pagesummary ").as_bytes(),
        "boundary-14 page summary",
    )?;
    if page_start == 0 || page_start >= trailer_start {
        return Err("boundary-14 header/body boundaries differ".to_owned());
    }
    let header = &bytes[..page_start];
    let semantic_body = &bytes[page_start..trailer_start];
    if !header.ends_with(b"\n")
        || !semantic_body.ends_with(b"\n")
        || header.iter().filter(|byte| **byte == b'\n').count() != 8
    {
        return Err(
            "boundary-14 split fixture does not have its exact eight-line header".to_owned(),
        );
    }
    Ok(BaseApplyFixtureSlices {
        header,
        semantic_body,
        emitted_body: &bytes[..trailer_start],
    })
}

fn manifest_usize(row: &StrictManifestRow, field: &str) -> Result<usize, String> {
    row.value(field)?
        .parse::<usize>()
        .map_err(|error| format!("invalid manifest {field}: {error}"))
}

fn manifest_bool(row: &StrictManifestRow, field: &str) -> Result<bool, String> {
    match row.value(field)? {
        "true" => Ok(true),
        "false" => Ok(false),
        value => Err(format!("invalid manifest {field} boolean {value}")),
    }
}

fn parse_named_row_counts(value: &str) -> Result<Vec<(String, usize)>, String> {
    let mut seen = BTreeSet::new();
    let mut counts = Vec::new();
    for token in value.split(',') {
        let (label, count) = token
            .split_once(':')
            .ok_or_else(|| format!("malformed named row count {token}"))?;
        if label.is_empty() || !seen.insert(label) {
            return Err(format!("duplicate/empty named row count {label}"));
        }
        counts.push((
            label.to_owned(),
            count
                .parse::<usize>()
                .map_err(|error| format!("invalid row count {token}: {error}"))?,
        ));
    }
    if counts.is_empty() {
        return Err("empty named row counts".to_owned());
    }
    Ok(counts)
}

fn semantic_row_counts(bytes: &[u8]) -> Result<Vec<(String, usize)>, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| format!("semantic body is not UTF-8: {error}"))?;
    let mut counts = BTreeMap::<String, usize>::new();
    for line in text.lines() {
        let label = line
            .split_ascii_whitespace()
            .next()
            .ok_or_else(|| "empty semantic row".to_owned())?;
        if !label.starts_with(BASE_APPLY_ROW_PREFIX) {
            return Err(format!("non-boundary-14 semantic row {label}"));
        }
        *counts.entry(label.to_owned()).or_default() += 1;
    }
    let canonical = [
        RowKind::Page,
        RowKind::Baseline,
        RowKind::PredecessorCompat,
        RowKind::Frontier,
        RowKind::Listeners,
        RowKind::VertexTrace,
        RowKind::ApplyDecision,
        RowKind::DuplicateScan,
        RowKind::EdgeStruct,
        RowKind::StemIncident,
        RowKind::RelationCallback,
        RowKind::BeamIncident,
        RowKind::Result,
        RowKind::DeltaGuard,
        RowKind::Summary,
    ];
    let mut ordered = Vec::with_capacity(canonical.len());
    for kind in canonical {
        let label = format!("{BASE_APPLY_ROW_PREFIX}{}", kind.suffix());
        let count = counts
            .remove(&label)
            .ok_or_else(|| format!("semantic body omits canonical family {label}"))?;
        ordered.push((label, count));
    }
    if !counts.is_empty() {
        return Err(format!(
            "semantic body has noncanonical families {:?}",
            counts.keys().collect::<Vec<_>>()
        ));
    }
    Ok(ordered)
}

fn format_named_row_counts(counts: &[(String, usize)]) -> String {
    counts
        .iter()
        .map(|(label, count)| format!("{label}:{count}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn validate_base_apply_manifest_corpus(bytes: &[u8]) -> Result<StrictManifest, String> {
    let manifest = StrictManifest::parse(bytes)?;
    let summary = &manifest.summary;

    for (field, relative) in [
        ("probeSourceSha256", BASE_APPLY_PROBE_SOURCE),
        ("runnerSourceSha256", BASE_APPLY_RUNNER_SOURCE),
    ]
    .into_iter()
    .chain(SOURCE_PATHS.iter().copied())
    {
        if summary.value(field)? != actual_file_sha256(relative)? {
            return Err(format!(
                "manifest {field} does not hash the active checkout"
            ));
        }
    }
    if summary.value("jgraphtCoreVersion")? != JGRAPHT_CORE_VERSION {
        return Err("manifest JGraphT version differs".to_owned());
    }
    validate_jgrapht_core_provenance(summary.value("jgraphtCoreJarSha256")?)?;

    let summary_marker = format!("{BASE_APPLY_MANIFEST_SUMMARY} ");
    let summary_start = unique_marker(
        bytes,
        summary_marker.as_bytes(),
        "boundary-14 manifest summary",
    )?;
    let summary_suffix = &bytes[summary_start..];
    if summary_suffix.iter().filter(|byte| **byte == b'\n').count() != 1
        || !summary_suffix.ends_with(b"\n")
    {
        return Err("manifest summary is not the exact terminal row".to_owned());
    }
    let manifest_body = &bytes[..summary_start];
    if !manifest_body.ends_with(b"\n")
        || summary.value("manifestBodySha256")? != sha256_hex(manifest_body)
        || summary.value("manifestBodySha256")? != BASE_APPLY_MANIFEST_BODY_SHA256
        || manifest_usize(summary, "manifestBodyLines")?
            != manifest_body.iter().filter(|byte| **byte == b'\n').count()
        || manifest_usize(summary, "manifestBodyLines")? != BASE_APPLY_MANIFEST_BODY_LINES
        || manifest_usize(summary, "manifestBodyBytes")? != manifest_body.len()
        || manifest_usize(summary, "manifestBodyBytes")? != BASE_APPLY_MANIFEST_BODY_BYTES
    {
        return Err("boundary-14 manifest-body fingerprint differs".to_owned());
    }

    let mut common_header = None::<Vec<u8>>;
    let mut normalized_corpus = Vec::<u8>::new();
    let mut row_order = None::<Vec<String>>;
    let mut aggregate_counts = BTreeMap::<String, usize>::new();
    let mut split_fixture_lines = 0_usize;
    let mut split_fixture_bytes = 0_usize;
    let mut real_systems = 0_usize;
    let mut real_transactions = 0_usize;
    let mut predecessor_compat_rows = 0_usize;
    let mut real_new_vertices = 0_usize;
    let mut real_existing_vertices = 0_usize;
    let mut real_edges = 0_usize;
    let mut real_reuse = 0_usize;
    let mut chord_matches = 0_usize;
    let mut supported_cases = 0_usize;
    let mut envelope_cases = 0_usize;
    let mut total_transactions = 0_usize;
    let mut compiler_processes = 0_usize;
    let mut runtime_processes = 0_usize;
    let mut total_processes = 0_usize;

    for (ordinal, (entry, page_key)) in manifest.entries.iter().zip(CORPUS_PAGES).enumerate() {
        if manifest_usize(entry, "ordinal")? != ordinal || entry.value("page")? != *page_key {
            return Err("boundary-14 manifest entry order differs".to_owned());
        }
        let expected_fixture = format!("stems-beam-vlink-base-apply-{page_key}.txt");
        if entry.value("fixture")? != expected_fixture {
            return Err(format!("{page_key} manifest fixture name differs"));
        }
        let fixture_path = repo_root().join("rust/oracle").join(&expected_fixture);
        let fixture_bytes = std::fs::read(&fixture_path)
            .map_err(|error| format!("{}: {error}", fixture_path.display()))?;
        if entry.value("fixtureSha256")? != sha256_hex(&fixture_bytes)
            || manifest_usize(entry, "fixtureLines")?
                != fixture_bytes.iter().filter(|byte| **byte == b'\n').count()
            || manifest_usize(entry, "fixtureBytes")? != fixture_bytes.len()
        {
            return Err(format!("{page_key} full-fixture fingerprint differs"));
        }
        let fixture = StrictFixture::parse(&fixture_bytes)?;
        if page_key_from_token(&fixture.page.page)? != *page_key {
            return Err(format!("{page_key} fixture page identity differs"));
        }
        validate_fixture_trailer(&fixture, &fixture_bytes)?;
        validate_fixture_algebra(&fixture)?;
        let slices = base_apply_fixture_slices(&fixture_bytes)?;
        let actual_counts = semantic_row_counts(slices.semantic_body)?;
        let declared_counts = parse_named_row_counts(entry.value("rowCounts")?)?;
        if declared_counts != actual_counts {
            return Err(format!(
                "{page_key} manifest row counts differ: declared {declared_counts:?}, actual {actual_counts:?}"
            ));
        }
        if entry.value("rowCounts")? != fixture.corpus_summary.value("rowCounts")? {
            return Err(format!(
                "{page_key} manifest/fixture row-count join differs"
            ));
        }
        let body_hash = sha256_hex(slices.emitted_body);
        if entry.value("emittedBodySha256")? != body_hash
            || entry.value("emittedBodySha256")?
                != fixture.corpus_summary.value("emittedBodySha256")?
            || entry.value("rawPassSha256")? != body_hash
            || entry.value("rawPassSha256")? != fixture.corpus_summary.value("rawPassSha256")?
        {
            return Err(format!("{page_key} emitted-body SHA join differs"));
        }
        if manifest_usize(entry, "emittedBodyLines")?
            != slices
                .emitted_body
                .iter()
                .filter(|byte| **byte == b'\n')
                .count()
            || manifest_usize(entry, "emittedBodyBytes")? != slices.emitted_body.len()
        {
            return Err(format!("{page_key} emitted-body size join differs"));
        }

        if let Some(header) = &common_header {
            if header.as_slice() != slices.header {
                return Err(format!("{page_key} common eight-line header differs"));
            }
        } else {
            common_header = Some(slices.header.to_vec());
            normalized_corpus.extend_from_slice(slices.header);
        }
        let labels = actual_counts
            .iter()
            .map(|(label, _)| label.clone())
            .collect::<Vec<_>>();
        if let Some(expected) = &row_order {
            if &labels != expected {
                return Err(format!("{page_key} row-count family order differs"));
            }
        } else {
            row_order = Some(labels);
        }
        for (label, count) in actual_counts {
            *aggregate_counts.entry(label).or_default() += count;
        }
        normalized_corpus.extend_from_slice(slices.semantic_body);

        let page_summary = &fixture.page_summary;
        let systems = fixture.systems.len();
        let entry_census = [
            ("systems", systems),
            (
                "realTransactions",
                parse_usize(page_summary, "realTransactions")?,
            ),
            ("syntheticSupportedCases", 5),
            ("envelopeCases", 4),
            (
                "totalTransactions",
                parse_usize(page_summary, "totalTransactions")?,
            ),
            ("predecessorCompatRows", systems),
            (
                "realNewVertexBranches",
                parse_usize(page_summary, "realNewVertexBranches")?,
            ),
            (
                "realExistingVertexBranches",
                parse_usize(page_summary, "realExistingVertexBranches")?,
            ),
            (
                "realEdgesAdded",
                parse_usize(page_summary, "realEdgesAdded")?,
            ),
            (
                "realReuseCensus",
                parse_usize(page_summary, "realReuseCensus")?,
            ),
            (
                "chordStemMatches",
                parse_usize(page_summary, "chordStemMatches")?,
            ),
        ];
        for (field, expected) in entry_census {
            if manifest_usize(entry, field)? != expected {
                return Err(format!("{page_key} manifest {field} differs"));
            }
        }
        let compat_count = fixture
            .systems
            .values()
            .flat_map(|rows| rows.iter())
            .filter(|row| row.kind == RowKind::PredecessorCompat)
            .count();
        if compat_count != systems {
            return Err(format!("{page_key} predecessor-compat census differs"));
        }
        for field in [
            "schedulerFixtureSha256",
            "expandFixtureSha256",
            "createStemFixtureSha256",
            "reuseCheckFixtureSha256",
        ] {
            if entry.value(field)? != fixture.page.value(field)?
                || entry.value(field)? != fixture.corpus_summary.value(field)?
            {
                return Err(format!("{page_key} manifest {field} join differs"));
            }
        }
        for (field, _) in [
            ("probeSourceSha256", BASE_APPLY_PROBE_SOURCE),
            ("runnerSourceSha256", BASE_APPLY_RUNNER_SOURCE),
        ]
        .into_iter()
        .chain(SOURCE_PATHS.iter().copied())
        {
            if fixture.corpus_summary.value(field)? != summary.value(field)? {
                return Err(format!(
                    "{page_key} source pin {field} differs from manifest"
                ));
            }
        }
        if fixture.corpus_summary.value("jgraphtCoreVersion")?
            != summary.value("jgraphtCoreVersion")?
            || fixture.corpus_summary.value("jgraphtCoreJarSha256")?
                != summary.value("jgraphtCoreJarSha256")?
        {
            return Err(format!("{page_key} JGraphT manifest join differs"));
        }

        let corpus = &fixture.corpus_summary;
        for (field, expected) in [
            ("freshRunsPerPage", 2),
            ("compilerJavaProcesses", 1),
            ("runtimeJavaProcessesPerPass", systems),
            ("runtimeJavaProcesses", systems * 2),
            ("totalJavaProcesses", systems * 2 + 1),
            ("maximumConcurrentJavaProcesses", 1),
            ("backgroundJavaProcessesStarted", 0),
        ] {
            if manifest_usize(entry, field)? != expected
                || entry.value(field)? != corpus.value(field)?
            {
                return Err(format!("{page_key} process field {field} differs"));
            }
        }
        for (entry_field, corpus_field) in [
            ("freshRunsByteIdentical", "freshRunsByteIdentical"),
            ("freshJvmPerSystem", "freshJvmPerSystem"),
            ("compilerJavaProcessReaped", "compilerJavaProcessReaped"),
            ("runtimeJavaProcessesReaped", "runtimeJavaProcessesReaped"),
            ("foregroundJavaProcessesOnly", "foregroundJavaProcessesOnly"),
            (
                "opaqueUnrelatedIdsExcludedFromStructuralHashes",
                "opaqueUnrelatedIdsExcludedFromStructuralHashes",
            ),
        ] {
            if !manifest_bool(entry, entry_field)?
                || entry.value(entry_field)? != corpus.value(corpus_field)?
            {
                return Err(format!("{page_key} process boolean {entry_field} differs"));
            }
        }
        for field in [
            "system1SyntheticBlock",
            "isolatedSheetsPerSyntheticCase",
            "stopBeforeBLinkerFlagMutation",
        ] {
            if !manifest_bool(entry, field)? {
                return Err(format!("{page_key} manifest guard {field} differs"));
            }
        }

        split_fixture_lines += fixture_bytes.iter().filter(|byte| **byte == b'\n').count();
        split_fixture_bytes += fixture_bytes.len();
        real_systems += systems;
        real_transactions += manifest_usize(entry, "realTransactions")?;
        predecessor_compat_rows += compat_count;
        real_new_vertices += manifest_usize(entry, "realNewVertexBranches")?;
        real_existing_vertices += manifest_usize(entry, "realExistingVertexBranches")?;
        real_edges += manifest_usize(entry, "realEdgesAdded")?;
        real_reuse += manifest_usize(entry, "realReuseCensus")?;
        chord_matches += manifest_usize(entry, "chordStemMatches")?;
        supported_cases += manifest_usize(entry, "syntheticSupportedCases")?;
        envelope_cases += manifest_usize(entry, "envelopeCases")?;
        total_transactions += manifest_usize(entry, "totalTransactions")?;
        compiler_processes += manifest_usize(entry, "compilerJavaProcesses")?;
        runtime_processes += manifest_usize(entry, "runtimeJavaProcesses")?;
        total_processes += manifest_usize(entry, "totalJavaProcesses")?;
    }

    let count_order = row_order.ok_or_else(|| "manifest reconstructed no rows".to_owned())?;
    let aggregate_named = count_order
        .into_iter()
        .map(|label| {
            let count = aggregate_counts[&label];
            (label, count)
        })
        .collect::<Vec<_>>();
    if summary.value("corpusRowCounts")? != format_named_row_counts(&aggregate_named)
        || summary.value("corpusBodySha256")? != sha256_hex(&normalized_corpus)
        || summary.value("corpusBodySha256")? != BASE_APPLY_CORPUS_BODY_SHA256
        || manifest_usize(summary, "corpusBodyLines")?
            != normalized_corpus
                .iter()
                .filter(|byte| **byte == b'\n')
                .count()
        || manifest_usize(summary, "corpusBodyLines")? != BASE_APPLY_CORPUS_BODY_LINES
        || manifest_usize(summary, "corpusBodyBytes")? != normalized_corpus.len()
        || manifest_usize(summary, "corpusBodyBytes")? != BASE_APPLY_CORPUS_BODY_BYTES
        || manifest_usize(summary, "splitFixtureLines")? != split_fixture_lines
        || manifest_usize(summary, "splitFixtureLines")? != BASE_APPLY_SPLIT_FIXTURE_LINES
        || manifest_usize(summary, "splitFixtureBytes")? != split_fixture_bytes
        || manifest_usize(summary, "splitFixtureBytes")? != BASE_APPLY_SPLIT_FIXTURE_BYTES
    {
        return Err("boundary-14 reconstructed corpus fingerprint differs".to_owned());
    }
    for (field, expected) in [
        ("realSystems", real_systems),
        ("realTransactions", real_transactions),
        ("predecessorCompatRows", predecessor_compat_rows),
        ("realNewVertexBranches", real_new_vertices),
        ("realExistingVertexBranches", real_existing_vertices),
        ("realEdgesAdded", real_edges),
        ("realReuseCensus", real_reuse),
        ("chordStemMatches", chord_matches),
        ("syntheticBlocks", CORPUS_PAGES.len()),
        ("syntheticSupportedCases", supported_cases),
        ("envelopeCases", envelope_cases),
        ("totalTransactions", total_transactions),
        ("compilerJavaProcesses", compiler_processes),
        ("runtimeJavaProcesses", runtime_processes),
        ("totalJavaProcesses", total_processes),
        ("maximumConcurrentJavaProcesses", 1),
        ("freshRunsPerPage", 2),
        ("backgroundJavaProcessesStarted", 0),
    ] {
        if manifest_usize(summary, field)? != expected {
            return Err(format!("manifest aggregate {field} differs"));
        }
    }
    for field in [
        "freshRunsByteIdentical",
        "freshJvmPerSystem",
        "compilerJavaProcessesReaped",
        "runtimeJavaProcessesReaped",
        "foregroundJavaProcessesOnly",
        "isolatedSheetsPerSyntheticCase",
        "stopBeforeBLinkerFlagMutation",
        "opaqueUnrelatedIdsExcludedFromStructuralHashes",
    ] {
        if !manifest_bool(summary, field)? {
            return Err(format!("manifest aggregate guard {field} differs"));
        }
    }
    Ok(manifest)
}

fn validate_system_row_order(system_id: usize, rows: &[StrictRow]) -> Result<(), String> {
    let mut offset = 0;
    let mut transaction_keys = BTreeSet::new();
    let mut transaction_order = Vec::new();
    while offset < rows.len() {
        if rows[offset].kind != RowKind::Baseline {
            return Err(format!(
                "system {system_id} transaction does not start with baseline"
            ));
        }
        let key = rows[offset].transaction_key()?;
        let owned_key = (key.0.to_owned(), key.1.to_owned());
        if !transaction_keys.insert(owned_key.clone()) {
            return Err(format!(
                "system {system_id} repeats transaction {}/{}",
                owned_key.0, owned_key.1
            ));
        }
        transaction_order.push(owned_key.clone());
        if transaction_keys.len() == 1 && owned_key != ("real".to_owned(), "-".to_owned()) {
            return Err(format!("system {system_id} real transaction is not first"));
        }
        let end = rows[offset + 1..]
            .iter()
            .position(|row| row.kind == RowKind::Baseline)
            .map_or(rows.len(), |relative| offset + 1 + relative);
        validate_transaction_row_order(system_id, &owned_key, &rows[offset..end])?;
        offset = end;
    }
    let mut expected_order = vec![("real".to_owned(), "-".to_owned())];
    if system_id == 1 {
        expected_order.extend(
            SYSTEM_ONE_SUPPLEMENTAL_CASES
                .iter()
                .map(|(scope, case)| ((*scope).to_owned(), (*case).to_owned())),
        );
    }
    if transaction_order != expected_order {
        return Err(format!(
            "system {system_id} transaction order differs: expected {expected_order:?}, got {transaction_order:?}"
        ));
    }
    Ok(())
}

fn validate_transaction_row_order(
    system_id: usize,
    key: &(String, String),
    rows: &[StrictRow],
) -> Result<(), String> {
    let mut prior_rank = None;
    let mut counts = BTreeMap::<RowKind, usize>::new();
    for row in rows {
        if row.transaction_key()? != (key.0.as_str(), key.1.as_str()) {
            return Err(format!(
                "system {system_id} transaction {}/{} is not contiguous",
                key.0, key.1
            ));
        }
        let rank = row
            .kind
            .system_rank()
            .ok_or_else(|| format!("system {system_id} contains a non-system row"))?;
        if prior_rank.is_some_and(|prior| rank < prior) {
            return Err(format!(
                "system {system_id} transaction {}/{} row order regressed",
                key.0, key.1
            ));
        }
        prior_rank = Some(rank);
        *counts.entry(row.kind).or_default() += 1;
    }
    for kind in [
        RowKind::Baseline,
        RowKind::Frontier,
        RowKind::Listeners,
        RowKind::VertexTrace,
        RowKind::ApplyDecision,
        RowKind::EdgeStruct,
        RowKind::RelationCallback,
        RowKind::Result,
        RowKind::DeltaGuard,
        RowKind::Summary,
    ] {
        if counts.get(&kind).copied() != Some(1) {
            return Err(format!(
                "system {system_id} transaction {}/{} does not have one {kind:?} row",
                key.0, key.1
            ));
        }
    }
    let expected_compat = usize::from(key.0 == "real");
    if counts
        .get(&RowKind::PredecessorCompat)
        .copied()
        .unwrap_or(0)
        != expected_compat
    {
        return Err(format!(
            "system {system_id} transaction {}/{} predecessor-compat count differs",
            key.0, key.1
        ));
    }
    Ok(())
}

fn validate_fixture_algebra(fixture: &StrictFixture) -> Result<(), String> {
    if parse_usize(&fixture.page, "systems")? != fixture.systems.len() {
        return Err("page system count differs from parsed system groups".to_owned());
    }
    if fixture.page.value("headless")? != "true" {
        return Err("compact v1 fixture is not headless".to_owned());
    }
    let page_key = page_key_from_token(&fixture.page.page)?;
    for (field, relative) in predecessor_paths(page_key) {
        let embedded = fixture.page.value(field)?;
        let actual = actual_file_sha256(&relative)?;
        if embedded != actual {
            return Err(format!("active predecessor provenance drift at {field}"));
        }
    }
    for (system, rows) in &fixture.systems {
        for block in transaction_slices(rows) {
            validate_transaction_algebra(*system, block)?;
        }
    }
    Ok(())
}

fn transaction_slices(rows: &[StrictRow]) -> Vec<&[StrictRow]> {
    let starts = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| (row.kind == RowKind::Baseline).then_some(index))
        .collect::<Vec<_>>();
    starts
        .iter()
        .enumerate()
        .map(|(index, start)| {
            let end = starts.get(index + 1).copied().unwrap_or(rows.len());
            &rows[*start..end]
        })
        .collect()
}

fn one_row(rows: &[StrictRow], kind: RowKind) -> Result<&StrictRow, String> {
    let matches = rows
        .iter()
        .filter(|row| row.kind == kind)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [row] => Ok(row),
        _ => Err(format!("expected exactly one {kind:?} row")),
    }
}

fn rows_of_kind(rows: &[StrictRow], kind: RowKind) -> Vec<&StrictRow> {
    rows.iter().filter(|row| row.kind == kind).collect()
}

fn parse_usize(row: &StrictRow, field: &str) -> Result<usize, String> {
    row.value(field)?
        .parse::<usize>()
        .map_err(|error| format!("invalid {field} in {:?}: {error}", row.kind))
}

fn parse_i32(row: &StrictRow, field: &str) -> Result<i32, String> {
    row.value(field)?
        .parse::<i32>()
        .map_err(|error| format!("invalid {field} in {:?}: {error}", row.kind))
}

fn parse_bool(row: &StrictRow, field: &str) -> Result<bool, String> {
    match row.value(field)? {
        "true" => Ok(true),
        "false" => Ok(false),
        value => Err(format!("invalid boolean {field}={value} in {:?}", row.kind)),
    }
}

fn require_same(
    left: &StrictRow,
    left_field: &str,
    right: &StrictRow,
    right_field: &str,
) -> Result<(), String> {
    let left_value = left.value(left_field)?;
    let right_value = right.value(right_field)?;
    if left_value != right_value {
        return Err(format!(
            "cross-row join differs: {:?}.{left_field}={left_value}, {:?}.{right_field}={right_value}",
            left.kind, right.kind
        ));
    }
    Ok(())
}

fn require_token(value: &str, allowed: &[&str], label: &str) -> Result<(), String> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!("invalid {label} token {value}"))
    }
}

fn parse_sig_edge_alias(value: &str) -> Result<usize, String> {
    value
        .strip_prefix("sig-edge:")
        .ok_or_else(|| format!("invalid graph relation identity {value}"))?
        .parse::<usize>()
        .map_err(|error| format!("invalid graph relation ordinal {value}: {error}"))
}

fn sha256_rows(rows: impl IntoIterator<Item = String>) -> String {
    let mut bytes = Vec::new();
    for row in rows {
        bytes.extend_from_slice(row.as_bytes());
        bytes.push(b'\n');
    }
    sha256_hex(&bytes)
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_transaction_algebra(system: usize, rows: &[StrictRow]) -> Result<(), String> {
    let baseline = one_row(rows, RowKind::Baseline)?;
    let frontier = one_row(rows, RowKind::Frontier)?;
    let listeners = one_row(rows, RowKind::Listeners)?;
    let vertex = one_row(rows, RowKind::VertexTrace)?;
    let decision = one_row(rows, RowKind::ApplyDecision)?;
    let edge = one_row(rows, RowKind::EdgeStruct)?;
    let callback = one_row(rows, RowKind::RelationCallback)?;
    let result = one_row(rows, RowKind::Result)?;
    let guard = one_row(rows, RowKind::DeltaGuard)?;
    let summary = one_row(rows, RowKind::Summary)?;
    let scope = baseline.value("scope")?;
    let case = baseline.value("case")?;
    let context = format!("system {system} {scope}/{case}");

    for row in rows {
        if row.system_id()? != system
            || row.value("plan")? != baseline.value("plan")?
            || row.value("scope")? != scope
            || row.value("case")? != case
        {
            return Err(format!("{context} transaction key drift"));
        }
    }

    require_same(frontier, "freshLinkAlias", edge, "freshDraftIdentity")?;
    require_same(frontier, "freshLinkAlias", result, "retainedDraftIdentity")?;
    require_same(frontier, "grade", result, "retainedDraftGrade")?;
    require_same(frontier, "finalStemAlias", baseline, "stemAlias")?;
    require_same(frontier, "finalStemAlias", decision, "targetAlias")?;
    require_same(frontier, "partnerAlias", decision, "targetAlias")?;
    require_same(frontier, "partnerInterId", baseline, "stemInterId")?;
    require_same(baseline, "beamAlias", decision, "sourceAlias")?;
    require_same(baseline, "beamInterId", decision, "sourceInterId")?;
    require_same(frontier, "portion", callback, "portionBefore")?;
    require_same(frontier, "portion", callback, "portionAfter")?;
    require_same(frontier, "extension", callback, "extensionBefore")?;
    require_same(frontier, "extension", callback, "extensionAfter")?;
    require_same(vertex, "branch", result, "branch")?;
    require_same(vertex, "branch", summary, "branch")?;
    require_same(result, "terminal", summary, "terminal")?;
    require_same(result, "applyReturn", edge, "applyReturn")?;
    require_same(result, "applyReturn", summary, "applyReturned")?;
    require_same(result, "finalStemAlias", baseline, "stemAlias")?;
    require_same(result, "finalStemInterId", decision, "targetInterId")?;
    require_same(result, "finalBeamAbnormal", callback, "beamAbnormalAfter")?;
    require_same(result, "finalBeamVip", baseline, "beamVip")?;
    require_same(result, "finalBeamRemoved", baseline, "beamRemoved")?;
    require_same(
        result,
        "finalBeamGroupVertexOrdinal",
        baseline,
        "beamGroupVertexOrdinal",
    )?;
    require_same(
        result,
        "finalBeamGroupStateHash",
        baseline,
        "beamGroupStateHash",
    )?;
    require_same(
        guard,
        "beamGroupVertexOrdinalBefore",
        baseline,
        "beamGroupVertexOrdinal",
    )?;
    require_same(
        guard,
        "beamGroupVertexOrdinalAfter",
        baseline,
        "beamGroupVertexOrdinal",
    )?;
    require_same(
        guard,
        "beamGroupStateHashBefore",
        baseline,
        "beamGroupStateHash",
    )?;
    require_same(
        guard,
        "beamGroupStateHashAfter",
        baseline,
        "beamGroupStateHash",
    )?;
    require_same(result, "allocator", vertex, "allocatorAfter")?;
    require_same(result, "interIndexHash", guard, "interIndexHashAfter")?;
    require_same(result, "sigVertexHash", guard, "sigVertexHashAfter")?;
    require_same(result, "sigEdgeHash", guard, "sigEdgeHashAfter")?;
    require_same(baseline, "allocator", guard, "allocatorBefore")?;
    require_same(baseline, "interIndexHash", guard, "interIndexHashBefore")?;
    require_same(baseline, "sigVertexHash", guard, "sigVertexHashBefore")?;
    require_same(baseline, "sigEdgeHash", guard, "sigEdgeHashBefore")?;

    if frontier.value("outgoing")? != "true"
        || frontier.value("relationClass")? != "BeamStemRelation"
        || frontier.value("predecessorTerminal")? != "ReadyBeforeSigMutation"
    {
        return Err(format!(
            "{context} does not join the accepted outgoing BeamStem draft"
        ));
    }
    require_token(
        frontier.value("portion")?,
        &["LEFT", "CENTER", "RIGHT"],
        "draft portion",
    )?;
    if decision.value("relationRuntimeClass")? != "org.audiveris.omr.sig.relation.BeamStemRelation"
    {
        return Err(format!("{context} relation runtime class differs"));
    }
    if parse_usize(baseline, "nextSharedId")?
        != parse_usize(baseline, "allocator")?
            .checked_add(1)
            .ok_or_else(|| format!("{context} allocator exhausted"))?
    {
        return Err(format!("{context} next shared ID is not allocator+1"));
    }
    if parse_usize(baseline, "interIndexScanned")? != parse_usize(baseline, "interIndex")?
        || parse_usize(baseline, "sigVertexScanned")? != parse_usize(baseline, "sigVertices")?
        || parse_usize(baseline, "glyphActiveBeamIdMatches")? != 0
        || parse_usize(baseline, "glyphOriginalBeamIdMatches")? != 0
    {
        return Err(format!("{context} beam/index exhaustive scan differs"));
    }
    if parse_bool(baseline, "beamRemoved")? {
        if baseline.value("interIndexBeamIndexOrdinal")? != "-"
            || parse_usize(baseline, "interIndexBeamObjectMatches")? != 0
            || parse_usize(baseline, "interIndexBeamIdMatches")? != 0
            || parse_usize(baseline, "sigBeamObjectMatches")? != 0
            || baseline.value("sigBeamVertexOrdinal")? != "-"
        {
            return Err(format!(
                "{context} removed beam retained live index/SIG membership"
            ));
        }
    } else if parse_usize(baseline, "interIndexBeamObjectMatches")? != 1
        || parse_usize(baseline, "interIndexBeamIdMatches")? != 1
        || parse_usize(baseline, "sigBeamObjectMatches")? != 1
        || parse_usize(baseline, "interIndexBeamIndexOrdinal")?
            >= parse_usize(baseline, "interIndex")?
        || parse_usize(baseline, "sigBeamVertexOrdinal")? >= parse_usize(baseline, "sigVertices")?
    {
        return Err(format!(
            "{context} live beam/index exhaustive lookup differs"
        ));
    }
    if baseline.value("beamRemoved")? != decision.value("sourceRemoved")? {
        return Err(format!("{context} beam removed snapshot/read differs"));
    }
    match (
        baseline.value("beamGroupVertexOrdinal")?,
        baseline.value("beamGroupStateHash")?,
    ) {
        ("-", "-") => {}
        (ordinal, hash)
            if ordinal
                .parse::<usize>()
                .is_ok_and(|value| value < parse_usize(baseline, "sigVertices").unwrap_or(0))
                && is_lower_sha256(hash) => {}
        _ => return Err(format!("{context} beam-group identity/state differs")),
    }
    if baseline.value("nextIdVipConfigured")? != "false" && scope == "real" {
        return Err(format!("{context} escapes the compact non-VIP envelope"));
    }
    if listeners.value("standardSingleSigListener")? != "true" && scope != "envelope" {
        return Err(format!(
            "{context} lacks the supported sole SigListener topology"
        ));
    }

    validate_duplicate_algebra(rows, baseline, decision, edge)?;
    validate_incident_algebra(
        rows, baseline, frontier, vertex, decision, edge, callback, result,
    )?;
    validate_branch_and_terminal_algebra(
        scope, case, baseline, vertex, decision, edge, callback, result, guard, summary,
    )?;
    Ok(())
}

fn validate_duplicate_algebra(
    rows: &[StrictRow],
    baseline: &StrictRow,
    decision: &StrictRow,
    edge: &StrictRow,
) -> Result<(), String> {
    let duplicate_rows = rows_of_kind(rows, RowKind::DuplicateScan);
    let scanned = parse_usize(decision, "sourceOutgoingScanned")?;
    if parse_usize(decision, "pairEdges")? != duplicate_rows.len() {
        return Err("duplicate pair row count differs".to_owned());
    }
    let mut class_checks = 0;
    let mut unread = 0;
    let mut first_match = None;
    let mut prior_source_ordinal = None;
    let mut pair_hash_rows = Vec::new();
    for (pair_ordinal, row) in duplicate_rows.iter().enumerate() {
        if parse_usize(row, "pairOrdinal")? != pair_ordinal {
            return Err("duplicate pair ordinals are not dense".to_owned());
        }
        let source_ordinal = parse_usize(row, "sourceOutgoingOrdinal")?;
        if source_ordinal >= scanned
            || prior_source_ordinal.is_some_and(|prior| prior >= source_ordinal)
        {
            return Err("duplicate source-outgoing chronology differs".to_owned());
        }
        prior_source_ordinal = Some(source_ordinal);
        let graph_alias = row.value("globalRelationIdentity")?;
        let graph_ordinal = parse_sig_edge_alias(graph_alias)?;
        if graph_ordinal >= parse_usize(baseline, "sigEdges")?
            || row.value("relationObjectIdentity")? != format!("graph-object:{graph_ordinal}")
        {
            return Err("duplicate graph/object identity domains differ".to_owned());
        }
        let read = parse_bool(row, "classCheckRead")?;
        let matches = parse_bool(row, "matchesRuntimeClass")?;
        if read {
            class_checks += 1;
        } else {
            unread += 1;
        }
        if matches {
            if !read
                || row.value("relationClass")? != decision.value("relationRuntimeClass")?
                || row.value("action")? != "SelectBreak"
                || first_match.is_some()
            {
                return Err("duplicate first-match evidence differs".to_owned());
            }
            first_match = Some((graph_alias, row.value("relationObjectIdentity")?));
        } else if read {
            if row.value("action")? != "Continue" || first_match.is_some() {
                return Err("duplicate examined chronology differs".to_owned());
            }
        } else if row.value("action")? != "UnreadAfterBreak" || first_match.is_none() {
            return Err("duplicate lazy unread suffix differs".to_owned());
        }
        pair_hash_rows.push(format!(
            "{pair_ordinal}:{source_ordinal}:{graph_alias}:{}",
            row.value("relationClass")?
        ));
    }
    if parse_usize(decision, "classChecksRead")? != class_checks
        || parse_usize(decision, "unreadSuffix")? != unread
    {
        return Err("duplicate read census differs".to_owned());
    }
    match first_match {
        Some((graph, object)) => {
            if decision.value("firstMatchIdentity")? != graph
                || decision.value("firstMatchObjectIdentity")? != object
                || decision.value("action")? != "SuppressExistingRelation"
                || edge.value("existingRelationIdentity")? != graph
                || edge.value("existingRelationObjectIdentity")? != object
            {
                return Err("duplicate selected edge cross-row join differs".to_owned());
            }
        }
        None => {
            if decision.value("firstMatchIdentity")? != "-"
                || decision.value("firstMatchObjectIdentity")? != "-"
                || edge.value("existingRelationIdentity")? != "-"
                || edge.value("existingRelationObjectIdentity")? != "-"
            {
                return Err("absent duplicate carries a selected identity".to_owned());
            }
        }
    }
    match decision.value("duplicateLookup")? {
        "ExhaustiveOutgoingThenLazyPairClass" => {
            let outgoing_hash = decision.value("sourceOutgoingProvenanceSha256")?;
            if !is_lower_sha256(outgoing_hash)
                || decision.value("pairProvenanceSha256")? != sha256_rows(pair_hash_rows)
            {
                return Err("duplicate exhaustive provenance differs".to_owned());
            }
        }
        "NotRead" | "NotReadAfterVertexThrow" => {
            if !duplicate_rows.is_empty()
                || decision.value("sourceOutgoingProvenanceSha256")? != "NotRead"
                || decision.value("pairProvenanceSha256")? != "NotRead"
            {
                return Err("duplicate NotRead certificate carries scan evidence".to_owned());
            }
        }
        "MissingEndpoint" => {
            if !duplicate_rows.is_empty()
                || decision.value("sourceOutgoingProvenanceSha256")? != "NotRead"
                || decision.value("pairProvenanceSha256")? != "NotRead"
            {
                return Err("missing-endpoint duplicate certificate differs".to_owned());
            }
        }
        value => return Err(format!("invalid duplicate lookup state {value}")),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)] // Cross-joins the exact fixed Java row family.
fn validate_incident_algebra(
    rows: &[StrictRow],
    baseline: &StrictRow,
    frontier: &StrictRow,
    vertex: &StrictRow,
    decision: &StrictRow,
    edge: &StrictRow,
    callback: &StrictRow,
    result: &StrictRow,
) -> Result<(), String> {
    let stem_rows = rows_of_kind(rows, RowKind::StemIncident);
    let beam_rows = rows_of_kind(rows, RowKind::BeamIncident);
    validate_phase_order(&stem_rows)?;
    validate_phase_order(&beam_rows)?;
    validate_selected_incident_mirrors(
        &stem_rows,
        &beam_rows,
        baseline.value("beamAlias")?,
        baseline.value("stemAlias")?,
    )?;

    for (phase, state_field, count_field, hash_field) in [
        (
            "Before",
            "preStemScanState",
            "preStemIncidentCount",
            "preStemIncidentHash",
        ),
        (
            "AfterCallback",
            "stemScanState",
            "stemIncidentCount",
            "stemIncidentHash",
        ),
    ] {
        let phase_rows = stem_rows
            .iter()
            .copied()
            .filter(|row| row.value("phase") == Ok(phase))
            .collect::<Vec<_>>();
        if parse_usize(callback, count_field)? != phase_rows.len() {
            return Err(format!("{phase} stem incident count differs"));
        }
        let state = callback.value(state_field)?;
        if phase_rows.is_empty() && state == "NotRead" {
            if callback.value(hash_field)? != sha256_rows(Vec::<String>::new()) {
                return Err(format!("{phase} NotRead stem hash differs"));
            }
        } else {
            require_token(
                state,
                &["ExhaustiveIncomingThenOutgoing", "MissingVertex"],
                "stem scan state",
            )?;
            validate_incident_ordinals(&phase_rows)?;
            let tokens = phase_rows
                .iter()
                .map(|row| {
                    Ok(format!(
                        "{}:{}:{}:{}:{}:{}:{}:{}",
                        row.value("incidentOrdinal")?,
                        row.value("direction")?,
                        row.value("directionOrdinal")?,
                        row.value("globalRelationIdentity")?,
                        row.value("relationClass")?,
                        row.value("oppositeInterId")?,
                        row.value("oppositeVertexOrdinal")?,
                        row.value("chordStemMatch")?
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?;
            if callback.value(hash_field)? != sha256_rows(tokens) {
                return Err(format!("{phase} stem incident hash differs"));
            }
            for row in phase_rows {
                validate_incident_identity(row, baseline, vertex, edge, result)?;
                if parse_bool(row, "chordStemMatch")? {
                    return Err("compact v1 admits no ChordStem callback matches".to_owned());
                }
                if row.value("action")? != "ObserveOnly" {
                    return Err("zero-Chord stem scan has a mutation action".to_owned());
                }
            }
        }
    }
    if callback.value("chordMatches")? != "0" || callback.value("chordInvalidations")? != "0" {
        return Err("compact v1 chord census is nonzero".to_owned());
    }

    let stem_before = stem_rows
        .iter()
        .copied()
        .filter(|row| row.value("phase") == Ok("Before"))
        .collect::<Vec<_>>();
    let beam_before = beam_rows
        .iter()
        .copied()
        .filter(|row| row.value("phase") == Ok("Before"))
        .collect::<Vec<_>>();
    if decision.value("duplicateLookup")? == "ExhaustiveOutgoingThenLazyPairClass" {
        let (outgoing_count, outgoing_hash) = beam_outgoing_provenance(&beam_before)?;
        if parse_usize(decision, "sourceOutgoingScanned")? != outgoing_count
            || decision.value("sourceOutgoingProvenanceSha256")? != outgoing_hash
        {
            return Err("source-outgoing exhaustive provenance differs".to_owned());
        }
    }
    for pair in rows_of_kind(rows, RowKind::DuplicateScan) {
        let graph = pair.value("globalRelationIdentity")?;
        let object = pair.value("relationObjectIdentity")?;
        let class = pair.value("relationClass")?;
        let beam_matches = beam_before
            .iter()
            .filter(|row| {
                row.value("globalRelationIdentity") == Ok(graph)
                    && row.value("relationObjectIdentity") == Ok(object)
                    && row.value("relationClass") == Ok(class)
                    && row.value("direction") == Ok("Outgoing")
                    && row.value("directionOrdinal") == pair.value("sourceOutgoingOrdinal")
            })
            .count();
        let stem_matches = stem_before
            .iter()
            .filter(|row| {
                row.value("globalRelationIdentity") == Ok(graph)
                    && row.value("relationObjectIdentity") == Ok(object)
                    && row.value("relationClass") == Ok(class)
                    && row.value("direction") == Ok("Incoming")
            })
            .count();
        if beam_matches != 1 || stem_matches != 1 {
            return Err("directed-pair relation does not join both pre-edge incidences".to_owned());
        }
    }

    let is_hook = baseline.value("beamClass")? == "BeamHookInter";
    for (phase, state_field, count_field, hash_field) in [
        (
            "Before",
            "preBeamScanState",
            "preBeamIncidentCount",
            "preBeamIncidentHash",
        ),
        (
            "AfterCallback",
            "beamScanState",
            "beamIncidentCount",
            "beamIncidentHash",
        ),
    ] {
        let phase_rows = beam_rows
            .iter()
            .copied()
            .filter(|row| row.value("phase") == Ok(phase))
            .collect::<Vec<_>>();
        if parse_usize(callback, count_field)? != phase_rows.len() {
            return Err(format!("{phase} beam incident count differs"));
        }
        let state = callback.value(state_field)?;
        if phase_rows.is_empty() && state == "NotRead" {
            if callback.value(hash_field)? != sha256_rows(Vec::<String>::new()) {
                return Err(format!("{phase} NotRead beam hash differs"));
            }
            continue;
        }
        validate_incident_ordinals(&phase_rows)?;
        let tokens = phase_rows
            .iter()
            .map(|row| {
                Ok(format!(
                    "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
                    row.value("incidentOrdinal")?,
                    row.value("direction")?,
                    row.value("directionOrdinal")?,
                    row.value("globalRelationIdentity")?,
                    row.value("relationClass")?,
                    row.value("readState")?,
                    row.value("oppositeInterId")?,
                    row.value("oppositeVertexOrdinal")?,
                    row.value("relevant")?,
                    row.value("portion")?,
                    row.value("contribution")?
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        if callback.value(hash_field)? != sha256_rows(tokens) {
            return Err(format!("{phase} beam incident hash differs"));
        }
        let (requested, left, right) = validate_beam_scan_rows(&phase_rows, is_hook)?;
        if phase == "AfterCallback" {
            if callback.value("requestedAbnormal")? != requested.to_string() {
                return Err("callback requested-abnormal result differs".to_owned());
            }
            let expected_rule = if is_hook {
                "HookHasAnyBeamStem"
            } else {
                "FullBeamNeedsLeftAndRight"
            };
            if callback.value("beamRule")? != expected_rule
                || callback.value("beamAbnormalAfter")? != requested.to_string()
                || callback.value("left")?
                    != left.map_or_else(|| "-".to_owned(), |value| value.to_string())
                || callback.value("right")?
                    != right.map_or_else(|| "-".to_owned(), |value| value.to_string())
            {
                return Err("beam callback rule/result differs".to_owned());
            }
        }
        for row in phase_rows {
            validate_incident_identity(row, baseline, vertex, edge, result)?;
        }
    }
    for phase in ["Before", "AfterCallback"] {
        let phase_stem = stem_rows
            .iter()
            .copied()
            .filter(|row| row.value("phase") == Ok(phase))
            .collect::<Vec<_>>();
        let phase_beam = beam_rows
            .iter()
            .copied()
            .filter(|row| row.value("phase") == Ok(phase))
            .collect::<Vec<_>>();
        validate_selected_endpoint_incidence_mirrors(
            &phase_stem,
            &phase_beam,
            baseline.value("beamAlias")?,
            baseline.value("stemAlias")?,
        )?;
    }
    let callback_completed = callback.value("outcome")?.starts_with("Completed");
    let new_graph = edge.value("graphRelationIdentity")?;
    let new_object = edge.value("freshDraftIdentity")?;
    let post_new_stem = stem_rows
        .iter()
        .filter(|row| {
            row.value("phase") == Ok("AfterCallback")
                && row.value("globalRelationIdentity") == Ok(new_graph)
                && row.value("relationObjectIdentity") == Ok(new_object)
                && row.value("direction") == Ok("Incoming")
        })
        .count();
    let post_new_beam = beam_rows
        .iter()
        .filter(|row| {
            row.value("phase") == Ok("AfterCallback")
                && row.value("globalRelationIdentity") == Ok(new_graph)
                && row.value("relationObjectIdentity") == Ok(new_object)
                && row.value("direction") == Ok("Outgoing")
        })
        .count();
    if callback_completed {
        if post_new_stem != 1 || post_new_beam != 1 {
            return Err("new relation is absent/duplicated in post-callback incidence".to_owned());
        }
        validate_pre_post_incident_retention(&stem_rows, new_graph, true)?;
        validate_pre_post_incident_retention(&beam_rows, new_graph, false)?;
    } else if post_new_stem != 0 || post_new_beam != 0 {
        return Err("unexecuted callback carries post-callback incidence".to_owned());
    }
    let mut endpoint_identities = Vec::new();
    if baseline.value("sigBeamVertexOrdinal")? != "-" {
        endpoint_identities.push((
            baseline.value("beamAlias")?.to_owned(),
            parse_i32(baseline, "beamInterId")?,
            parse_usize(baseline, "sigBeamVertexOrdinal")?,
        ));
    }
    if vertex.value("branch")? == "NewIdZero" || baseline.value("sigStemVertexOrdinal")? != "-" {
        endpoint_identities.push((
            baseline.value("stemAlias")?.to_owned(),
            parse_i32(result, "finalStemInterId")?,
            if vertex.value("branch")? == "NewIdZero" {
                parse_usize(baseline, "sigVertices")?
            } else {
                parse_usize(baseline, "sigStemVertexOrdinal")?
            },
        ));
    }
    for row in stem_rows.iter().chain(&beam_rows) {
        endpoint_identities.push((
            row.value("oppositeAlias")?.to_owned(),
            parse_i32(row, "oppositeInterId")?,
            parse_usize(row, "oppositeVertexOrdinal")?,
        ));
    }
    validate_endpoint_bijection(endpoint_identities)?;
    require_same(frontier, "portion", callback, "portionBefore")?;
    Ok(())
}

fn validate_selected_incident_mirrors(
    stem_rows: &[&StrictRow],
    beam_rows: &[&StrictRow],
    beam_alias: &str,
    stem_alias: &str,
) -> Result<(), String> {
    fn key(row: &StrictRow) -> Result<(String, String, String, String, String), String> {
        let class = row.value("relationClass")?;
        Ok((
            row.value("phase")?.to_owned(),
            row.value("globalRelationIdentity")?.to_owned(),
            row.value("relationObjectIdentity")?.to_owned(),
            class.to_owned(),
            format!("{:?}", parse_query_relation_kind(class)),
        ))
    }

    let mut from_stem = BTreeMap::<_, usize>::new();
    for row in stem_rows
        .iter()
        .copied()
        .filter(|row| row.value("oppositeAlias") == Ok(beam_alias))
    {
        if row.value("direction")? != "Incoming" {
            return Err("selected beam/stem relation is not incoming at the stem".to_owned());
        }
        *from_stem.entry(key(row)?).or_default() += 1;
    }
    let mut from_beam = BTreeMap::<_, usize>::new();
    for row in beam_rows
        .iter()
        .copied()
        .filter(|row| row.value("oppositeAlias") == Ok(stem_alias))
    {
        if row.value("direction")? != "Outgoing" {
            return Err("selected beam/stem relation is not outgoing at the beam".to_owned());
        }
        *from_beam.entry(key(row)?).or_default() += 1;
    }
    if from_stem.values().any(|count| *count != 1)
        || from_beam.values().any(|count| *count != 1)
        || from_stem != from_beam
    {
        return Err(
            "selected beam/stem relation lacks one exact inverse-direction incident mirror"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_selected_endpoint_incidence_mirrors(
    stem_rows: &[&StrictRow],
    beam_rows: &[&StrictRow],
    beam_alias: &str,
    stem_alias: &str,
) -> Result<(), String> {
    for stem in stem_rows {
        let graph = stem.value("globalRelationIdentity")?;
        let matching = beam_rows
            .iter()
            .copied()
            .filter(|beam| beam.value("globalRelationIdentity") == Ok(graph))
            .collect::<Vec<_>>();
        if matching.is_empty() {
            if stem.value("oppositeAlias")? == beam_alias {
                return Err("selected endpoint incident mirror is missing".to_owned());
            }
            continue;
        }
        let [beam] = matching.as_slice() else {
            return Err("selected endpoint incident mirror is duplicated".to_owned());
        };
        if stem.value("oppositeAlias")? != beam_alias
            || beam.value("oppositeAlias")? != stem_alias
            || stem.value("relationObjectIdentity")? != beam.value("relationObjectIdentity")?
            || stem.value("relationClass")? != beam.value("relationClass")?
            || stem.value("direction")? == beam.value("direction")?
        {
            return Err("selected endpoint incident mirror payload differs".to_owned());
        }
    }
    for beam in beam_rows {
        if beam.value("oppositeAlias")? == stem_alias
            && !stem_rows.iter().any(|stem| {
                stem.value("globalRelationIdentity") == beam.value("globalRelationIdentity")
            })
        {
            return Err("selected endpoint incident mirror is missing".to_owned());
        }
    }
    Ok(())
}

fn beam_outgoing_provenance(rows: &[&StrictRow]) -> Result<(usize, String), String> {
    let outgoing = rows
        .iter()
        .copied()
        .filter(|row| row.value("direction") == Ok("Outgoing"))
        .collect::<Vec<_>>();
    let tokens = outgoing
        .iter()
        .map(|row| {
            Ok(format!(
                "{}:{}",
                row.value("globalRelationIdentity")?,
                row.value("relationClass")?
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((outgoing.len(), sha256_rows(tokens)))
}

fn validate_endpoint_bijection(
    endpoint_identities: Vec<(String, i32, usize)>,
) -> Result<(), String> {
    let mut by_alias = BTreeMap::<String, (i32, usize)>::new();
    let mut by_inter_id = BTreeMap::<i32, (usize, String)>::new();
    let mut by_vertex = BTreeMap::<usize, (i32, String)>::new();
    for (alias, inter_id, vertex) in endpoint_identities {
        if inter_id <= 0
            || by_alias
                .insert(alias.clone(), (inter_id, vertex))
                .is_some_and(|prior| prior != (inter_id, vertex))
            || by_inter_id
                .insert(inter_id, (vertex, alias.clone()))
                .is_some_and(|prior| prior != (vertex, alias.clone()))
            || by_vertex
                .insert(vertex, (inter_id, alias.clone()))
                .is_some_and(|prior| prior != (inter_id, alias.clone()))
        {
            return Err(
                "incident aliases, positive Inter IDs, and vertex ordinals are not bijective"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

fn validate_pre_post_incident_retention(
    rows: &[&StrictRow],
    new_graph: &str,
    stem_side: bool,
) -> Result<(), String> {
    let before = rows
        .iter()
        .copied()
        .filter(|row| row.value("phase") == Ok("Before"))
        .collect::<Vec<_>>();
    let after = rows
        .iter()
        .copied()
        .filter(|row| row.value("phase") == Ok("AfterCallback"))
        .collect::<Vec<_>>();
    if after.len() != before.len() + 1 {
        return Err("post-callback incidence is not baseline plus one".to_owned());
    }
    let insertion = if stem_side {
        before
            .iter()
            .position(|row| row.value("direction") == Ok("Outgoing"))
            .unwrap_or(before.len())
    } else {
        before.len()
    };
    let mut expected_graph_order = before
        .iter()
        .map(|row| row.value("globalRelationIdentity").map(str::to_owned))
        .collect::<Result<Vec<_>, _>>()?;
    expected_graph_order.insert(insertion, new_graph.to_owned());
    let actual_graph_order = after
        .iter()
        .map(|row| row.value("globalRelationIdentity").map(str::to_owned))
        .collect::<Result<Vec<_>, _>>()?;
    if actual_graph_order != expected_graph_order {
        return Err("post-callback incident insertion order differs".to_owned());
    }
    for old in before {
        let matches = after
            .iter()
            .filter(|row| {
                row.value("globalRelationIdentity") == old.value("globalRelationIdentity")
            })
            .collect::<Vec<_>>();
        let [new] = matches.as_slice() else {
            return Err("old incident was dropped or duplicated after callback".to_owned());
        };
        for field in [
            "direction",
            "directionOrdinal",
            "globalRelationIdentity",
            "relationObjectIdentity",
            "relationClass",
            "oppositeAlias",
            "oppositeInterId",
            "oppositeVertexOrdinal",
        ] {
            if old.value(field)? != new.value(field)? {
                return Err(format!("old incident payload drift at {field}"));
            }
        }
        if stem_side {
            for field in ["chordStemMatch", "action"] {
                if old.value(field)? != new.value(field)? {
                    return Err(format!("old stem incident payload drift at {field}"));
                }
            }
        } else {
            for field in ["readState", "relevant", "portion", "contribution"] {
                if old.value(field)? != new.value(field)? {
                    return Err(format!("old beam incident payload drift at {field}"));
                }
            }
        }
    }
    Ok(())
}

fn validate_phase_order(rows: &[&StrictRow]) -> Result<(), String> {
    let mut after = false;
    for row in rows {
        match row.value("phase")? {
            "Before" if !after => {}
            "AfterCallback" => after = true,
            value => return Err(format!("incident phase order differs at {value}")),
        }
    }
    Ok(())
}

fn validate_incident_ordinals(rows: &[&StrictRow]) -> Result<(), String> {
    let mut incoming = 0;
    let mut outgoing = 0;
    let mut saw_outgoing = false;
    for (incident, row) in rows.iter().enumerate() {
        if parse_usize(row, "incidentOrdinal")? != incident {
            return Err("incident ordinals are not dense".to_owned());
        }
        match row.value("direction")? {
            "Incoming" if !saw_outgoing => {
                if parse_usize(row, "directionOrdinal")? != incoming {
                    return Err("incoming incident ordinals are not dense".to_owned());
                }
                incoming += 1;
            }
            "Outgoing" => {
                saw_outgoing = true;
                if parse_usize(row, "directionOrdinal")? != outgoing {
                    return Err("outgoing incident ordinals are not dense".to_owned());
                }
                outgoing += 1;
            }
            value => return Err(format!("invalid/delayed incident direction {value}")),
        }
    }
    Ok(())
}

fn validate_incident_identity(
    row: &StrictRow,
    baseline: &StrictRow,
    vertex: &StrictRow,
    edge: &StrictRow,
    result: &StrictRow,
) -> Result<(), String> {
    let graph_alias = row.value("globalRelationIdentity")?;
    let graph_ordinal = parse_sig_edge_alias(graph_alias)?;
    let post_edge_count =
        parse_usize(baseline, "sigEdges")? + usize::from(parse_bool(edge, "draftInGraphAfter")?);
    if graph_ordinal >= post_edge_count {
        return Err("incident graph identity is outside the post graph".to_owned());
    }
    let expected_object = if graph_alias == edge.value("graphRelationIdentity")? {
        edge.value("freshDraftIdentity")?.to_owned()
    } else {
        format!("graph-object:{graph_ordinal}")
    };
    if row.value("relationObjectIdentity")? != expected_object {
        return Err("incident graph/object relation identity differs".to_owned());
    }
    let opposite_id = parse_i32(row, "oppositeInterId")?;
    if opposite_id <= 0 {
        return Err("incident opposite Inter ID is not positive".to_owned());
    }
    let opposite_ordinal = parse_usize(row, "oppositeVertexOrdinal")?;
    if opposite_ordinal > parse_usize(baseline, "sigVertices")? {
        return Err("incident opposite vertex ordinal is outside the post graph".to_owned());
    }
    let alias = row.value("oppositeAlias")?;
    if alias == baseline.value("beamAlias")? {
        if row.value("oppositeInterId")? != baseline.value("beamInterId")?
            || row.value("oppositeVertexOrdinal")? != baseline.value("sigBeamVertexOrdinal")?
        {
            return Err("beam opposite endpoint certificate differs".to_owned());
        }
        if opposite_ordinal >= parse_usize(baseline, "sigVertices")? {
            return Err("baseline beam endpoint uses an appended vertex ordinal".to_owned());
        }
    } else if alias == baseline.value("stemAlias")? {
        if row.value("oppositeInterId")? != result.value("finalStemInterId")? {
            return Err("selected-stem opposite endpoint ID differs".to_owned());
        }
        let expected_ordinal = if vertex.value("branch")? == "NewIdZero" {
            parse_usize(baseline, "sigVertices")?
        } else {
            parse_usize(baseline, "sigStemVertexOrdinal")?
        };
        if opposite_ordinal != expected_ordinal {
            return Err("selected-stem opposite vertex ordinal differs".to_owned());
        }
        let is_fresh_post = row.value("phase")? == "AfterCallback"
            && graph_alias == edge.value("graphRelationIdentity")?;
        if vertex.value("branch")? == "NewIdZero" && !is_fresh_post {
            return Err("appended stem appears outside the fresh post-edge row".to_owned());
        }
    } else if let Some(inter_id) = alias.strip_prefix("inter:") {
        if inter_id != row.value("oppositeInterId")?
            || opposite_ordinal >= parse_usize(baseline, "sigVertices")?
        {
            return Err("other-Inter opposite alias/ID/ordinal differs".to_owned());
        }
    } else {
        return Err(format!("unknown incident opposite alias {alias}"));
    }
    Ok(())
}

fn validate_beam_scan_rows(
    rows: &[&StrictRow],
    is_hook: bool,
) -> Result<(bool, Option<bool>, Option<bool>), String> {
    if is_hook {
        let mut found = false;
        for row in rows {
            if row.value("portion")? != "-" {
                return Err("hook scan fabricated a BeamPortion read".to_owned());
            }
            let class_matches =
                row.value("relationClass")? == "org.audiveris.omr.sig.relation.BeamStemRelation";
            if found {
                if row.value("readState")? != "UnreadAfterBreak"
                    || parse_bool(row, "relevant")?
                    || row.value("contribution")? != "None"
                {
                    return Err("hook unread suffix carries examined evidence".to_owned());
                }
            } else {
                if row.value("readState")? != "ExaminedClassOnly"
                    || parse_bool(row, "relevant")? != class_matches
                {
                    return Err("hook class-only scan differs".to_owned());
                }
                if class_matches {
                    found = true;
                    if row.value("contribution")? != "FirstBeamStem" {
                        return Err("hook first BeamStem contribution differs".to_owned());
                    }
                } else if row.value("contribution")? != "None" {
                    return Err("hook nonmatch contributed".to_owned());
                }
            }
        }
        return Ok((!found, None, None));
    }

    let mut left = false;
    let mut right = false;
    for row in rows {
        let relevant_class = matches!(
            row.value("relationClass")?,
            "org.audiveris.omr.sig.relation.BeamStemRelation"
                | "org.audiveris.omr.sig.relation.BeamRestRelation"
        );
        let portion = row.value("portion")?;
        if relevant_class {
            if portion == "-" {
                if row.value("readState")? != "ExaminedClassOnly" || parse_bool(row, "relevant")? {
                    return Err(
                        "raw BeamStem/BeamRest null portion fabricated a portion read".to_owned(),
                    );
                }
            } else {
                if row.value("readState")? != "ExaminedClassAndPortion" {
                    return Err(
                        "raw BeamStem/BeamRest portion accessor was not evidenced".to_owned()
                    );
                }
                require_token(portion, &["LEFT", "CENTER", "RIGHT"], "raw beam portion")?;
                if !parse_bool(row, "relevant")? {
                    return Err("raw BeamStem/BeamRest row did not read its portion".to_owned());
                }
            }
        } else if row.value("readState")? != "ExaminedClassOnly"
            || portion != "-"
            || parse_bool(row, "relevant")?
        {
            return Err("unrelated raw beam row fabricated portion evidence".to_owned());
        }
        let contribution = match portion {
            "LEFT" => {
                left = true;
                "Left"
            }
            "RIGHT" => {
                right = true;
                "Right"
            }
            "CENTER" | "-" => "None",
            _ => unreachable!("portion domain checked above"),
        };
        if row.value("contribution")? != contribution {
            return Err("raw beam portion contribution differs".to_owned());
        }
    }
    Ok((!left || !right, Some(left), Some(right)))
}

#[allow(clippy::too_many_arguments)]
fn validate_branch_and_terminal_algebra(
    scope: &str,
    case: &str,
    baseline: &StrictRow,
    vertex: &StrictRow,
    decision: &StrictRow,
    edge: &StrictRow,
    callback: &StrictRow,
    result: &StrictRow,
    guard: &StrictRow,
    summary: &StrictRow,
) -> Result<(), String> {
    let branch = vertex.value("branch")?;
    require_token(branch, &["NewIdZero", "ExistingPositive"], "vertex branch")?;
    let new_vertex = branch == "NewIdZero";
    let success = scope == "real" || matches!(case, "ExistingFullSuccess" | "ExistingHookSuccess");
    let duplicate_suppressed = case == "ExistingDuplicateSuppress";
    let source_suppressed = case == "SourceRemovedSuppress";
    let target_suppressed = case == "TargetRemovedSuppress";
    let missing_target = case == "MissingPositiveTargetThrows";
    let vertex_throw = case == "NewVertexListenerThrows";
    let early_edge_throw = case == "EarlyEdgeListenerThrows";
    let later_edge_throw = case == "LaterEdgeListenerThrows";
    if !success
        && !duplicate_suppressed
        && !source_suppressed
        && !target_suppressed
        && !missing_target
        && !vertex_throw
        && !early_edge_throw
        && !later_edge_throw
    {
        return Err(format!("unknown boundary-14 case {scope}/{case}"));
    }
    if new_vertex != (scope == "real" || vertex_throw) {
        return Err("vertex branch differs from case domain".to_owned());
    }

    let allocator_before = parse_i32(baseline, "allocator")?;
    let allocator_after = parse_i32(vertex, "allocatorAfter")?;
    let vertex_count_before = parse_usize(vertex, "vertexCountBefore")?;
    let inter_count_before = parse_usize(vertex, "interIndexBefore")?;
    if vertex_count_before != parse_usize(baseline, "sigVertices")?
        || inter_count_before != parse_usize(baseline, "interIndex")?
    {
        return Err("vertex trace baseline counts differ".to_owned());
    }
    if new_vertex {
        let assigned = parse_i32(vertex, "assignedId")?;
        if parse_i32(baseline, "stemInterId")? != 0
            || assigned != allocator_before + 1
            || allocator_after != assigned
            || parse_usize(vertex, "interIndexAfter")? != inter_count_before + 1
            || parse_usize(vertex, "vertexCountAfter")? != vertex_count_before + 1
            || parse_usize(vertex, "interIndexStemObjectMatchesAfterVertex")? != 1
            || vertex.value("interIndexAssignedIdMatchesAfterVertex")? != "1"
            || parse_usize(vertex, "sigStemObjectMatchesAfterVertex")? != 1
            || parse_usize(vertex, "sigStemVertexOrdinalAfterVertex")? != vertex_count_before
            || parse_usize(vertex, "vertexOrdinal")? != vertex_count_before
            || vertex.value("callAttempted")? != "true"
            || vertex.value("registerAttempted")? != "true"
            || vertex.value("vipMembershipRead")? != "true"
            || vertex.value("vipConfigured")? != "false"
        {
            return Err("new-ID vertex registration prefix differs".to_owned());
        }
        if baseline.value("interIndexStemIndexOrdinal")? != "-"
            || parse_usize(baseline, "interIndexStemObjectMatches")? != 0
            || parse_usize(baseline, "interIndexStemIdMatches")? != 0
            || parse_usize(baseline, "glyphActiveStemIdMatches")? != 0
            || parse_usize(baseline, "glyphOriginalStemIdMatches")? != 0
            || parse_usize(baseline, "interIndexGeneratedNextIdMatches")? != 0
            || baseline.value("nextIdInter")? != "-"
            || baseline.value("nextIdGlyphActive")? != "-"
            || baseline.value("nextIdGlyphOriginal")? != "-"
        {
            return Err("ID-zero/next-ID registry lookup differs".to_owned());
        }
        if vertex_throw {
            if vertex.value("vertexCallReturn")? != "-"
                || vertex.value("vertexEventCompleted")? != "false"
                || vertex.value("stemSigAttachedAfterVertex")? != "false"
                || vertex.value("stemAddedCompleted")? != "false"
                || vertex.value("abnormalRequest")? != "NotReached"
                || vertex.value("abnormalChanged")? != "NotReached"
                || vertex.value("throwStage")? != "VertexAdd"
                || result.value("finalStemSigAttached")? != "false"
            {
                return Err("early vertex-listener partial commit differs".to_owned());
            }
        } else if vertex.value("vertexCallReturn")? != "true"
            || vertex.value("vertexEventCompleted")? != "true"
            || vertex.value("stemSigAttachedAfterVertex")? != "true"
            || vertex.value("stemAddedCompleted")? != "true"
            || vertex.value("abnormalRequest")? != "true"
            || vertex.value("abnormalChanged")? != "true"
            || vertex.value("throwStage")? != "-"
            || result.value("finalStemSigAttached")? != "true"
        {
            return Err("completed new vertex callback differs".to_owned());
        }
        if parse_i32(result, "finalStemInterId")? != assigned {
            return Err("assigned/final StemInter ID differs".to_owned());
        }
    } else {
        if allocator_after != allocator_before
            || vertex.value("assignedId")? != "-"
            || parse_usize(vertex, "interIndexAfter")? != inter_count_before
            || parse_usize(vertex, "vertexCountAfter")? != vertex_count_before
            || vertex.value("callAttempted")? != "false"
            || vertex.value("registerAttempted")? != "false"
            || vertex.value("vipMembershipRead")? != "false"
            || vertex.value("vertexCallReturn")? != "-"
            || vertex.value("vertexEventCompleted")? != "NotRead"
            || vertex.value("stemAddedCompleted")? != "NotRead"
            || vertex.value("abnormalRequest")? != "NotRead"
            || vertex.value("abnormalChanged")? != "NotRead"
        {
            return Err("existing-positive branch mutated the allocator/vertex set".to_owned());
        }
        if parse_i32(result, "finalStemInterId")? != parse_i32(baseline, "stemInterId")? {
            return Err("existing final StemInter ID differs".to_owned());
        }
        if decision.value("targetInterId")? != baseline.value("stemInterId")? {
            return Err("existing Link target ID differs from baseline".to_owned());
        }
        if missing_target {
            if baseline.value("interIndexStemIndexOrdinal")? != "-"
                || parse_usize(baseline, "interIndexStemObjectMatches")? != 0
                || parse_usize(baseline, "sigStemObjectMatches")? != 0
                || result.value("finalStemSigAttached")? != "false"
                || vertex.value("stemSigAttachedAfterVertex")? != "false"
            {
                return Err("missing-positive target unexpectedly exists".to_owned());
            }
        } else if target_suppressed {
            if baseline.value("interIndexStemIndexOrdinal")? != "-"
                || parse_usize(baseline, "interIndexStemObjectMatches")? != 0
                || parse_usize(baseline, "interIndexStemIdMatches")? != 0
                || parse_usize(baseline, "sigStemObjectMatches")? != 0
                || baseline.value("sigStemVertexOrdinal")? != "-"
                || result.value("finalStemSigAttached")? != "true"
                || vertex.value("stemSigAttachedAfterVertex")? != "true"
            {
                return Err("removed target retained live index/SIG membership".to_owned());
            }
        } else if parse_usize(baseline, "interIndexStemIndexOrdinal")?
            >= parse_usize(baseline, "interIndex")?
            || parse_usize(baseline, "interIndexStemObjectMatches")? != 1
            || parse_usize(baseline, "interIndexStemIdMatches")? != 1
            || parse_usize(baseline, "glyphActiveStemIdMatches")? != 0
            || parse_usize(baseline, "glyphOriginalStemIdMatches")? != 0
            || parse_usize(baseline, "sigStemObjectMatches")? != 1
            || result.value("finalStemSigAttached")? != "true"
            || vertex.value("stemSigAttachedAfterVertex")? != "true"
        {
            return Err("existing-positive target certificate differs".to_owned());
        }
    }

    let edge_added = success || early_edge_throw || later_edge_throw;
    let callback_completed = success || later_edge_throw;
    if parse_bool(edge, "draftInGraphBefore")?
        || parse_bool(edge, "draftInGraphAfter")? != edge_added
        || parse_bool(summary, "edgeAdded")? != edge_added
    {
        return Err("draft graph insertion census differs".to_owned());
    }
    if edge_added {
        let graph_ordinal = parse_usize(baseline, "sigEdges")?;
        let graph_alias = format!("sig-edge:{graph_ordinal}");
        if edge.value("graphRelationIdentity")? != graph_alias
            || parse_usize(edge, "globalEdgeOrdinal")? != graph_ordinal
            || edge.value("sourceOutgoingOrdinal")? != decision.value("sourceOutgoingScanned")?
            || !parse_bool(edge, "globalInserted")?
            || !parse_bool(edge, "sourceOutgoingInserted")?
            || !parse_bool(edge, "targetIncomingInserted")?
            || result.value("graphRelationIdentity")? != graph_alias
        {
            return Err("structural edge append differs".to_owned());
        }
    } else if edge.value("graphRelationIdentity")? != "-"
        || edge.value("globalEdgeOrdinal")? != "-"
        || edge.value("sourceOutgoingOrdinal")? != "-"
        || edge.value("targetIncomingOrdinal")? != "-"
        || parse_bool(edge, "globalInserted")?
        || parse_bool(edge, "sourceOutgoingInserted")?
        || parse_bool(edge, "targetIncomingInserted")?
        || result.value("graphRelationIdentity")? != "-"
    {
        return Err("unattached draft carries graph identity/mutation".to_owned());
    }
    let expected_callback = if success {
        "Completed"
    } else if later_edge_throw {
        "CompletedBeforeLaterListenerThrow"
    } else {
        "NotRun"
    };
    if edge.value("callbackOutcome")? != expected_callback
        || parse_bool(callback, "beamCheckCalled")? != callback_completed
    {
        return Err("relation callback completion differs".to_owned());
    }
    if callback_completed {
        if callback.value("stemScanState")? != "ExhaustiveIncomingThenOutgoing"
            || callback.value("beamScanState")?
                != if baseline.value("beamClass")? == "BeamHookInter" {
                    "LazyIncomingThenOutgoing"
                } else {
                    "ExhaustiveIncomingThenOutgoing"
                }
        {
            return Err("completed callback scan state differs".to_owned());
        }
    } else if callback.value("stemScanState")? != "NotRead"
        || callback.value("beamScanState")? != "NotRead"
        || callback.value("beamRule")? != "NotRead"
    {
        return Err("suppressed/thrown callback carries executed scan evidence".to_owned());
    }

    let (apply_return, terminal, throw_stage) = if success {
        ("true", "ReadyBeforeBLinkerFlagMutation", "-")
    } else if duplicate_suppressed || source_suppressed || target_suppressed {
        ("false", "ReadyBeforeBLinkerFlagMutation", "-")
    } else if vertex_throw {
        ("-", "EnvelopeThrownAtVertexAdd", "VertexAdd")
    } else if early_edge_throw {
        ("-", "EnvelopeThrownAtEarlyEdgeListener", "LinkApply")
    } else if later_edge_throw {
        ("-", "EnvelopeThrownAtLaterEdgeListener", "LinkApply")
    } else {
        ("-", "EnvelopeThrownAtEdgeInsert", "LinkApply")
    };
    if edge.value("applyReturn")? != apply_return
        || result.value("terminal")? != terminal
        || summary.value("throwStage")? != throw_stage
        || parse_bool(summary, "vertexAdded")? != new_vertex
    {
        return Err("terminal/result summary differs".to_owned());
    }
    let expected_action = if source_suppressed {
        "SuppressSourceRemoved"
    } else if target_suppressed {
        "SuppressTargetRemoved"
    } else if duplicate_suppressed {
        "SuppressExistingRelation"
    } else if vertex_throw {
        "NotReached"
    } else {
        "Add"
    };
    if decision.value("action")? != expected_action {
        return Err("Link.applyTo decision differs".to_owned());
    }
    if parse_bool(decision, "sourceRemovedRead")? == vertex_throw
        || (source_suppressed && !parse_bool(decision, "sourceRemoved")?)
        || (source_suppressed && parse_bool(decision, "targetRemovedRead")?)
        || (target_suppressed && !parse_bool(decision, "targetRemovedRead")?)
        || (target_suppressed && decision.value("targetRemoved")? != "true")
    {
        return Err("removed-endpoint short-circuit differs".to_owned());
    }
    if parse_bool(edge, "edgeEventCompleted")? != success
        || result.value("retainedDraftGrade")? == "-"
    {
        return Err("edge event/retained draft evidence differs".to_owned());
    }
    for field in [
        "stemGeometryUnchanged",
        "beamGeometryUnchanged",
        "beamGroupUnchanged",
        "glyphRegistriesUnchanged",
        "stopBeforeBLinkerFlagMutation",
        "stopBeforeSiblingBeamLinks",
        "stopBeforeHeadRelationLoop",
    ] {
        if guard.value(field)? != "true" {
            return Err(format!("boundary guard {field} is not true"));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
struct ProjectedCompactQueries {
    inter_index: NativeStemsBeamInterIndexApplyState,
    sheet_edit: NativeStemsBeamSheetEditState,
    certificate: NativeStemsBeamVLinkBaseApplyCertificate,
}

fn project_compact_queries(
    page: &StrictRow,
    rows: &[StrictRow],
) -> Result<ProjectedCompactQueries, String> {
    let baseline = one_row(rows, RowKind::Baseline)?;
    let frontier = one_row(rows, RowKind::Frontier)?;
    let vertex = one_row(rows, RowKind::VertexTrace)?;
    let decision = one_row(rows, RowKind::ApplyDecision)?;
    let edge = one_row(rows, RowKind::EdgeStruct)?;
    let callback = one_row(rows, RowKind::RelationCallback)?;
    if baseline.value("scope")? == "envelope" {
        return Err(
            "Java throw-prefix envelopes are intentionally unsupported by production v1".to_owned(),
        );
    }
    let baseline_count = parse_usize(baseline, "interIndex")?;
    let beam_lookup = match baseline.value("interIndexBeamIndexOrdinal")? {
        "-" => NativeStemsBeamInterIndexLookup::Absent,
        _ => NativeStemsBeamInterIndexLookup::PresentSameObject {
            index_ordinal: parse_usize(baseline, "interIndexBeamIndexOrdinal")?,
            inter_id: parse_i32(baseline, "beamInterId")?,
            vip: parse_bool(baseline, "beamVip")?,
            object_matches: parse_usize(baseline, "interIndexBeamObjectMatches")?,
            inter_id_matches: parse_usize(baseline, "interIndexBeamIdMatches")?,
            glyph_active_matches: parse_usize(baseline, "glyphActiveBeamIdMatches")?,
            glyph_original_matches: parse_usize(baseline, "glyphOriginalBeamIdMatches")?,
        },
    };
    let stem_lookup = match baseline.value("interIndexStemIndexOrdinal")? {
        "-" => NativeStemsBeamInterIndexLookup::Absent,
        _ => NativeStemsBeamInterIndexLookup::PresentSameObject {
            index_ordinal: parse_usize(baseline, "interIndexStemIndexOrdinal")?,
            inter_id: parse_i32(baseline, "stemInterId")?,
            vip: parse_bool(baseline, "stemVip")?,
            object_matches: parse_usize(baseline, "interIndexStemObjectMatches")?,
            inter_id_matches: parse_usize(baseline, "interIndexStemIdMatches")?,
            glyph_active_matches: parse_usize(baseline, "glyphActiveStemIdMatches")?,
            glyph_original_matches: parse_usize(baseline, "glyphOriginalStemIdMatches")?,
        },
    };
    let next_id_lookup = if vertex.value("branch")? == "NewIdZero" {
        NativeStemsBeamNextPersistentIdLookup::VacantAndNotVip {
            persistent_id: parse_i32(baseline, "nextSharedId")?,
            inter_id_matches: parse_usize(baseline, "interIndexGeneratedNextIdMatches")?,
            glyph_active_matches: usize::from(baseline.value("nextIdGlyphActive")? != "-"),
            glyph_original_matches: usize::from(baseline.value("nextIdGlyphOriginal")? != "-"),
            configured_vip_matches: usize::from(parse_bool(baseline, "nextIdVipConfigured")?),
        }
    } else {
        NativeStemsBeamNextPersistentIdLookup::NotRead
    };
    let inter_index = NativeStemsBeamInterIndexApplyState {
        baseline_entry_count: baseline_count,
        baseline_provenance_sha256: baseline.value("interIndexHash")?.to_owned(),
        beam_lookup,
        stem_lookup,
        next_id_lookup,
        appended_entries: Vec::new(),
    };

    let draft_alias = frontier.value("freshLinkAlias")?;
    let plan = parse_usize(frontier, "plan")?;
    let directed_rows = rows_of_kind(rows, RowKind::DuplicateScan);
    let relations = directed_rows
        .iter()
        .map(|row| {
            let class_read = match (
                parse_bool(row, "classCheckRead")?,
                parse_bool(row, "matchesRuntimeClass")?,
            ) {
                (true, true) => NativeStemsBeamPairClassRead::ExaminedMatchBreak,
                (true, false) => NativeStemsBeamPairClassRead::ExaminedContinue,
                (false, false) => NativeStemsBeamPairClassRead::UnreadAfterBreak,
                (false, true) => return Err("unread pair row claims a class match".to_owned()),
            };
            Ok(NativeStemsBeamDirectedPairRelation {
                pair_ordinal: parse_usize(row, "pairOrdinal")?,
                source_outgoing_ordinal: parse_usize(row, "sourceOutgoingOrdinal")?,
                graph_relation_identity: parse_sig_edge_alias(
                    row.value("globalRelationIdentity")?,
                )?,
                relation_object_identity: parse_relation_object_identity(
                    row.value("relationObjectIdentity")?,
                    draft_alias,
                    plan,
                )?,
                relation_class: row.value("relationClass")?.to_owned(),
                kind: parse_query_relation_kind(row.value("relationClass")?),
                class_read,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let directed_pair_scan = NativeStemsBeamDirectedPairScan {
        source_outgoing_scanned: parse_usize(decision, "sourceOutgoingScanned")?,
        source_outgoing_provenance: parse_query_provenance(
            decision.value("sourceOutgoingProvenanceSha256")?,
        )?,
        query_relation_count: parse_usize(decision, "pairEdges")?,
        pair_provenance: parse_query_provenance(decision.value("pairProvenanceSha256")?)?,
        relations,
    };
    let stem_incident_before =
        project_stem_incident_scan(rows, callback, "Before", draft_alias, plan)?;
    let stem_incident_after =
        project_stem_incident_scan(rows, callback, "AfterCallback", draft_alias, plan)?;
    let beam_incident_before =
        project_beam_incident_scan(rows, baseline, callback, "Before", draft_alias, plan)?;
    let beam_incident_after =
        project_beam_incident_scan(rows, baseline, callback, "AfterCallback", draft_alias, plan)?;
    let certificate = NativeStemsBeamVLinkBaseApplyCertificate {
        system_id: baseline.system_id()?,
        headless: parse_bool(page, "headless")?,
        listener_topology: NativeStemsBeamSigListenerTopology::SoleStandardSigListener,
        endpoint_identity:
            audiveris_omr::native_stems_beam_vlink_base_apply::NativeStemsBeamCertificateEndpointIdentity::JavaPersistentInterId,
        directed_pair_scan,
        stem_incident_before,
        stem_incident_after,
        beam_incident_before,
        beam_incident_after,
        chord_stem_matches: parse_usize(callback, "chordMatches")?,
        fresh_relation_object_identity: parse_relation_object_identity(
            edge.value("freshDraftIdentity")?,
            draft_alias,
            plan,
        )?,
        fresh_relation_graph_matches: usize::from(parse_bool(edge, "draftInGraphBefore")?),
    };
    Ok(ProjectedCompactQueries {
        inter_index,
        sheet_edit: NativeStemsBeamSheetEditState {
            stub_modified: parse_bool(baseline, "stubModified")?,
            book_modified: parse_bool(baseline, "bookModifiedRaw")?,
            book_dirty: parse_bool(baseline, "bookDirty")?,
        },
        certificate,
    })
}

fn parse_relation_object_identity(
    value: &str,
    draft_alias: &str,
    plan: usize,
) -> Result<NativeStemsBeamRelationObjectIdentity, String> {
    if value == draft_alias {
        return Ok(NativeStemsBeamRelationObjectIdentity::FreshDraft(plan));
    }
    value
        .strip_prefix("graph-object:")
        .ok_or_else(|| format!("invalid relation-object identity {value}"))?
        .parse::<usize>()
        .map(NativeStemsBeamRelationObjectIdentity::GraphObject)
        .map_err(|error| format!("invalid relation-object identity {value}: {error}"))
}

fn parse_query_provenance(value: &str) -> Result<NativeStemsBeamQueryProvenance, String> {
    match value {
        "NotRead" | "MissingEndpoint" => Ok(NativeStemsBeamQueryProvenance::NotRead),
        _ if is_lower_sha256(value) => Ok(NativeStemsBeamQueryProvenance::ExhaustiveSha256(
            value.to_owned(),
        )),
        _ => Err(format!("unsupported query provenance {value}")),
    }
}

fn parse_query_relation_kind(class: &str) -> NativeStemsBeamQueryRelationKind {
    match class.rsplit('.').next().unwrap_or(class) {
        "BeamStemRelation" => NativeStemsBeamQueryRelationKind::BeamStem,
        "BeamRestRelation" => NativeStemsBeamQueryRelationKind::BeamRest,
        "ChordStemRelation" => NativeStemsBeamQueryRelationKind::ChordStem,
        _ => NativeStemsBeamQueryRelationKind::Other,
    }
}

fn parse_incident_direction(value: &str) -> Result<NativeStemsBeamIncidentDirection, String> {
    match value {
        "Incoming" => Ok(NativeStemsBeamIncidentDirection::Incoming),
        "Outgoing" => Ok(NativeStemsBeamIncidentDirection::Outgoing),
        _ => Err(format!("invalid incident direction {value}")),
    }
}

fn parse_incident_opposite(
    row: &StrictRow,
    baseline: &StrictRow,
) -> Result<NativeStemsBeamIncidentOpposite, String> {
    let alias = row.value("oppositeAlias")?;
    if alias == baseline.value("beamAlias")? {
        Ok(NativeStemsBeamIncidentOpposite::Beam)
    } else if alias == baseline.value("stemAlias")? {
        Ok(NativeStemsBeamIncidentOpposite::Stem)
    } else if alias.starts_with("inter:") {
        Ok(NativeStemsBeamIncidentOpposite::OtherInter)
    } else {
        Err(format!("unknown incident opposite alias {alias}"))
    }
}

fn project_stem_incident_scan(
    rows: &[StrictRow],
    callback: &StrictRow,
    phase: &str,
    draft_alias: &str,
    plan: usize,
) -> Result<NativeStemsBeamStemIncidentScan, String> {
    let baseline = one_row(rows, RowKind::Baseline)?;
    let (state_field, count_field, hash_field) = if phase == "Before" {
        (
            "preStemScanState",
            "preStemIncidentCount",
            "preStemIncidentHash",
        )
    } else {
        ("stemScanState", "stemIncidentCount", "stemIncidentHash")
    };
    let state = match callback.value(state_field)? {
        "NotRead" => NativeStemsBeamStemIncidentScanState::NotRead,
        "MissingVertex" => NativeStemsBeamStemIncidentScanState::MissingVertex,
        "ExhaustiveIncomingThenOutgoing" => {
            NativeStemsBeamStemIncidentScanState::ExhaustiveIncomingThenOutgoing
        }
        value => return Err(format!("invalid stem incident state {value}")),
    };
    let relations = rows_of_kind(rows, RowKind::StemIncident)
        .into_iter()
        .filter(|row| row.value("phase") == Ok(phase))
        .map(|row| {
            Ok(NativeStemsBeamStemIncidentRelation {
                incident_ordinal: parse_usize(row, "incidentOrdinal")?,
                direction: parse_incident_direction(row.value("direction")?)?,
                direction_ordinal: parse_usize(row, "directionOrdinal")?,
                graph_relation_identity: parse_sig_edge_alias(
                    row.value("globalRelationIdentity")?,
                )?,
                relation_object_identity: parse_relation_object_identity(
                    row.value("relationObjectIdentity")?,
                    draft_alias,
                    plan,
                )?,
                relation_class: row.value("relationClass")?.to_owned(),
                kind: parse_query_relation_kind(row.value("relationClass")?),
                opposite_vertex_ordinal: parse_usize(row, "oppositeVertexOrdinal")?,
                opposite: parse_incident_opposite(row, baseline)?,
                opposite_inter_id: parse_i32(row, "oppositeInterId")?,
                chord_stem_match: parse_bool(row, "chordStemMatch")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(NativeStemsBeamStemIncidentScan {
        state,
        query_relation_count: parse_usize(callback, count_field)?,
        query_provenance_sha256: callback.value(hash_field)?.to_owned(),
        relations,
    })
}

fn project_beam_incident_scan(
    rows: &[StrictRow],
    baseline: &StrictRow,
    callback: &StrictRow,
    phase: &str,
    draft_alias: &str,
    plan: usize,
) -> Result<NativeStemsBeamBeamIncidentScan, String> {
    let (state_field, count_field, hash_field) = if phase == "Before" {
        (
            "preBeamScanState",
            "preBeamIncidentCount",
            "preBeamIncidentHash",
        )
    } else {
        ("beamScanState", "beamIncidentCount", "beamIncidentHash")
    };
    let rule = if matches!(callback.value(state_field)?, "NotRead" | "MissingVertex") {
        NativeStemsBeamBeamIncidentRule::NotRead
    } else if baseline.value("beamClass")? == "BeamHookInter" {
        NativeStemsBeamBeamIncidentRule::HookHasAnyBeamStem
    } else {
        NativeStemsBeamBeamIncidentRule::RawBeamLeftAndRight
    };
    let relations = rows_of_kind(rows, RowKind::BeamIncident)
        .into_iter()
        .filter(|row| row.value("phase") == Ok(phase))
        .map(|row| {
            let beam_portion = match row.value("portion")? {
                "-" => None,
                "LEFT" => Some(NativeBeamPortion::Left),
                "CENTER" => Some(NativeBeamPortion::Center),
                "RIGHT" => Some(NativeBeamPortion::Right),
                value => return Err(format!("invalid incident BeamPortion {value}")),
            };
            Ok(NativeStemsBeamBeamIncidentRelation {
                incident_ordinal: parse_usize(row, "incidentOrdinal")?,
                direction: parse_incident_direction(row.value("direction")?)?,
                direction_ordinal: parse_usize(row, "directionOrdinal")?,
                graph_relation_identity: parse_sig_edge_alias(
                    row.value("globalRelationIdentity")?,
                )?,
                relation_object_identity: parse_relation_object_identity(
                    row.value("relationObjectIdentity")?,
                    draft_alias,
                    plan,
                )?,
                relation_class: row.value("relationClass")?.to_owned(),
                kind: parse_query_relation_kind(row.value("relationClass")?),
                opposite_vertex_ordinal: parse_usize(row, "oppositeVertexOrdinal")?,
                opposite: parse_incident_opposite(row, baseline)?,
                opposite_inter_id: parse_i32(row, "oppositeInterId")?,
                read: match row.value("readState")? {
                    "ExaminedClassOnly" | "ExaminedClassAndPortion" => {
                        NativeStemsBeamBeamIncidentRead::Examined
                    }
                    "UnreadAfterBreak" => NativeStemsBeamBeamIncidentRead::UnreadAfterBreak,
                    value => return Err(format!("invalid beam incident read state {value}")),
                },
                relevant: parse_bool(row, "relevant")?,
                beam_portion,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(NativeStemsBeamBeamIncidentScan {
        rule,
        query_relation_count: parse_usize(callback, count_field)?,
        query_provenance_sha256: callback.value(hash_field)?.to_owned(),
        relations,
    })
}

// Boundary 14 deliberately rebuilds the public boundary-12 and boundary-13
// products.  The predecessor fixtures are used only as exhaustive Java
// certificates for live registry/graph state; scheduler, plan, candidate,
// createStem, and checkLink products are all re-executed through their public
// Rust APIs below.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PredecessorRow {
    family: String,
    page: String,
    fields: BTreeMap<String, String>,
}

impl PredecessorRow {
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
struct PredecessorFixture {
    rows: Vec<PredecessorRow>,
}

impl PredecessorFixture {
    fn parse(text: &str, schema: &str) -> Result<Self, String> {
        if text.lines().filter(|line| *line == schema).count() != 1 {
            return Err(format!("{schema} must occur exactly once"));
        }
        let mut rows = Vec::new();
        for line in text
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
            if tokens.len() < 2 {
                return Err("malformed predecessor row".to_owned());
            }
            // Corpus-summary rows intentionally have no page token and are
            // provenance-checked by their already-active predecessor gates.
            if !tokens[1].contains('#') {
                continue;
            }
            if (tokens.len() - 2) % 2 != 0 {
                return Err(format!("malformed {} row", tokens[0]));
            }
            let mut fields = BTreeMap::new();
            for pair in tokens[2..].chunks_exact(2) {
                if fields
                    .insert(pair[0].to_owned(), pair[1].to_owned())
                    .is_some()
                {
                    return Err(format!("{} row repeats {}", tokens[0], pair[0]));
                }
            }
            rows.push(PredecessorRow {
                family: tokens[0].to_owned(),
                page: tokens[1].to_owned(),
                fields,
            });
        }
        if rows.is_empty() {
            return Err("predecessor fixture has no page rows".to_owned());
        }
        let page = &rows[0].page;
        if rows.iter().any(|row| row.page != *page) {
            return Err("predecessor fixture mixes page identities".to_owned());
        }
        Ok(Self { rows })
    }

    fn one(&self, family: &str, system_id: usize) -> Result<&PredecessorRow, String> {
        let matches = self
            .rows
            .iter()
            .filter(|row| {
                row.family == family
                    && row
                        .fields
                        .get("system")
                        .and_then(|value| value.parse::<usize>().ok())
                        == Some(system_id)
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [row] => Ok(row),
            _ => Err(format!(
                "system {system_id} expected one {family}, found {}",
                matches.len()
            )),
        }
    }

    fn all(&self, family: &str, system_id: usize) -> Vec<&PredecessorRow> {
        self.rows
            .iter()
            .filter(|row| {
                row.family == family
                    && row
                        .fields
                        .get("system")
                        .and_then(|value| value.parse::<usize>().ok())
                        == Some(system_id)
            })
            .collect()
    }

    fn one_scoped(
        &self,
        family: &str,
        system_id: usize,
        scope: &str,
        case_name: &str,
    ) -> Result<&PredecessorRow, String> {
        let matches = self
            .all(family, system_id)
            .into_iter()
            .filter(|row| row.fields.get("scope").map(String::as_str) == Some(scope))
            .filter(|row| row.fields.get("case").map(String::as_str) == Some(case_name))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [row] => Ok(row),
            _ => Err(format!(
                "system {system_id} expected one {scope}/{case_name} {family}, found {}",
                matches.len()
            )),
        }
    }
}

struct NativePredecessorPage {
    grid: GridLinesRecognition,
    sig: Option<NativeSigRecognition>,
    stem_seeds: NativeStemSeedRecognition,
    beam_stumps: NativeStemsBeamStumpRecognition,
    beam_vlinkers: NativeStemsBeamVLinkerRecognition,
    beam_builders: NativeStemsBeamBuilderRecognition,
    plans: NativeStemsBeamLinkPlanRecognition,
    scheduler: NativeStemsBeamSchedulerRecognition,
}

fn native_predecessor_page(image: &str) -> NativePredecessorPage {
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
    let sig = (image == "chula.png").then(|| {
        assemble_native_sig(&grid, &headers, &beams, &ledgers, &heads)
            .unwrap_or_else(|error| panic!("{image}: native SIG assembly failed: {error}"))
    });
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
    NativePredecessorPage {
        grid,
        sig,
        stem_seeds,
        beam_stumps,
        beam_vlinkers,
        beam_builders,
        plans,
        scheduler,
    }
}

fn predecessor_attempt(
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

fn parse_stem_line(value: &str, label: &str) -> Result<NativeStemLine, String> {
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

fn parse_stem_point(value: &str, label: &str) -> Result<NativeStemPoint, String> {
    let coordinates = value.split(':').collect::<Vec<_>>();
    if coordinates.len() != 2 {
        return Err(format!("{label} point does not have two coordinates"));
    }
    Ok(NativeStemPoint {
        x: parse_hex_double(coordinates[0], label)?,
        y: parse_hex_double(coordinates[1], label)?,
    })
}

fn parse_relation_impacts(value: &str) -> Result<(f64, f64), String> {
    let impacts = parse_predecessor_list(value)?;
    if impacts.len() != 2 {
        return Err(format!("relation impacts cardinality differs: {value}"));
    }
    let parse = |token: &str, expected: &str| -> Result<f64, String> {
        let (name, rest) = token
            .split_once(':')
            .ok_or_else(|| format!("malformed relation impact {token}"))?;
        let (impact, weight) = rest
            .split_once(":w=")
            .ok_or_else(|| format!("malformed relation impact weight {token}"))?;
        if name != expected {
            return Err(format!("relation impact order differs: {token}"));
        }
        let expected_weight: f64 = if expected == "xOutGap" { 1.0 } else { 4.0 };
        if parse_hex_double(weight, "relation impact weight")?.to_bits()
            != expected_weight.to_bits()
        {
            return Err(format!("relation impact weight differs: {token}"));
        }
        parse_hex_double(impact, "relation impact")
    };
    Ok((parse(impacts[0], "xOutGap")?, parse(impacts[1], "yGap")?))
}

fn parse_predecessor_list(value: &str) -> Result<Vec<&str>, String> {
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

fn parse_java_rectangle(value: &str) -> Result<JavaRectangle, String> {
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
    let coordinate_limit = match orientation {
        Orientation::Horizontal => width,
        Orientation::Vertical => height,
    };
    let mut pixels = vec![BACKGROUND; width * height];
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
    RunTable::from_pixels(orientation, width, height, &pixels)
        .map_err(|error| format!("run table reconstruction failed: {error}"))
}

fn run_table_sha256(table: &RunTable) -> String {
    let orientation = match table.orientation() {
        Orientation::Horizontal => "HORIZONTAL",
        Orientation::Vertical => "VERTICAL",
    };
    let mut bytes = format!("{orientation} {} {}\n", table.width(), table.height()).into_bytes();
    for sequence in 0..table.sequence_count() {
        let mut row = sequence.to_string();
        for run in table.sequence(sequence).unwrap_or_default() {
            write!(row, " {}:{}", run.start, run.length).expect("String writes cannot fail");
        }
        row.push('\n');
        bytes.extend_from_slice(row.as_bytes());
    }
    sha256_hex(&bytes)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SelectedGlyphCertificate {
    alias: usize,
    glyph_id: i32,
    bounds: Bounds,
    run_sha256: String,
}

fn parse_selected_glyph(value: &str) -> Result<SelectedGlyphCertificate, String> {
    let fields = value.split(':').collect::<Vec<_>>();
    if fields.len() != 10 || fields[0] != "glyph" || fields[4] != "g" {
        return Err(format!("invalid selected glyph: {value}"));
    }
    let alias = fields[1]
        .parse::<usize>()
        .map_err(|_| format!("invalid selected alias: {value}"))?;
    if !matches!(fields[2], "active" | "original-only") {
        return Err(format!("invalid selected membership: {value}"));
    }
    let glyph_id = fields[3]
        .strip_prefix("id=")
        .and_then(|field| field.parse::<i32>().ok())
        .filter(|id| *id > 0)
        .ok_or_else(|| format!("invalid selected glyph ID: {value}"))?;
    if usize::try_from(glyph_id).ok() != Some(alias) {
        return Err(format!("selected alias/ID differs: {value}"));
    }
    Ok(SelectedGlyphCertificate {
        alias,
        glyph_id,
        bounds: parse_bounds(&fields[5..9].join(":"))?,
        run_sha256: parse_lower_hex(fields[9], 64, "glyph run SHA-256")?.to_owned(),
    })
}

fn parse_glyph_alias(value: &str) -> Result<usize, String> {
    value
        .strip_prefix("glyph:")
        .and_then(|field| field.parse::<usize>().ok())
        .filter(|alias| *alias > 0)
        .ok_or_else(|| format!("invalid glyph alias: {value}"))
}

fn fixed_glyph(
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

fn independently_build_candidate(
    attempt: &NativeStemsBeamLinkPlanAttempt,
) -> Result<NativeStemsBeamFixedGlyphContent, String> {
    let first = attempt
        .glyphs
        .first()
        .ok_or_else(|| "ready attempt has no glyphs".to_owned())?;
    if attempt.glyphs.len() == 1 {
        return Ok(fixed_glyph(
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
    Ok(fixed_glyph(bounds, run_table.weight(), run_table))
}

fn create_stem_checker_context(page: &NativePredecessorPage) -> NativeStemsBeamStemCheckerContext {
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

fn create_stem_state_from_predecessor(
    fixture: &PredecessorFixture,
    scheduler: &audiveris_omr::native_stems_beam_scheduler::NativeStemsBeamSchedulerSystem,
    attempt: &NativeStemsBeamLinkPlanAttempt,
) -> Result<NativeStemsBeamVLinkTransactionState, String> {
    let system_id = scheduler.system_id;
    if !scheduler.deferred_line_deltas.is_empty() {
        return Err(format!(
            "system {system_id} has unsupported committed-prefix deltas"
        ));
    }
    let baseline = fixture.one("stemsbeamcreatestembaseline", system_id)?;
    let frontier_row = fixture.one("stemsbeamcreatestemfrontier", system_id)?;
    let lookup = fixture.one("stemsbeamcreatestemlookup", system_id)?;
    let frontier = match &scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => return Err(format!("system {system_id} is not createStem-ready")),
    };
    let selected = parse_predecessor_list(frontier_row.value("selectedGlyphRefs")?)?
        .into_iter()
        .map(parse_selected_glyph)
        .collect::<Result<Vec<_>, _>>()?;
    if selected.len() != attempt.glyphs.len() {
        return Err(format!("system {system_id} selected glyph count differs"));
    }
    let mut selected_glyph_bindings = Vec::with_capacity(selected.len());
    for (certificate, glyph) in selected.iter().zip(&attempt.glyphs) {
        let content = fixed_glyph(
            glyph.bounds,
            glyph.weight,
            glyph.structural_key.run_table.clone(),
        );
        if certificate.bounds != content.bounds
            || certificate.run_sha256 != run_table_sha256(&content.run_table)
        {
            return Err(format!(
                "system {system_id} selected glyph structure differs"
            ));
        }
        selected_glyph_bindings.push(NativeStemsBeamSelectedGlyphBinding {
            reference: glyph.reference,
            canonical_alias: certificate.alias,
            glyph_id: certificate.glyph_id,
            content,
        });
    }
    let candidate = fixed_glyph(
        parse_bounds(lookup.value("candidateBounds")?)?,
        lookup.usize("candidateWeight")?,
        parse_run_table(lookup.value("candidateRunTable")?)?,
    );
    if independently_build_candidate(attempt)? != candidate {
        return Err(format!(
            "system {system_id} independently rebuilt candidate differs"
        ));
    }
    let glyph_lookup = match lookup.value("lookup")? {
        "Absent" => NativeStemsBeamExhaustiveGlyphLookup::Absent,
        "Present" => NativeStemsBeamExhaustiveGlyphLookup::Present {
            canonical_alias: parse_glyph_alias(lookup.value("presentAlias")?)?,
            glyph_id: lookup.i32("presentId")?,
            active_in_index: match lookup.value("presentActive")? {
                "true" => true,
                "false" => false,
                value => return Err(format!("invalid presentActive {value}")),
            },
        },
        value => return Err(format!("invalid glyph lookup {value}")),
    };
    if lookup.value("systemStemLookup")? != "Absent" {
        return Err(format!(
            "system {system_id} compact createStem fixture has a baseline system stem"
        ));
    }
    let allocator = baseline.i32("allocator")?;
    Ok(NativeStemsBeamVLinkTransactionState {
        scope: if system_id == 1 {
            NativeStemsBeamVLinkTransactionScope::SharedSheetFirstFrontier { system_id }
        } else {
            NativeStemsBeamVLinkTransactionScope::IsolatedFreshSheetFrontier { system_id }
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
            system_id,
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

fn replay_create_stem_predecessor(
    page: &NativePredecessorPage,
    fixture: &PredecessorFixture,
    system_id: usize,
) -> Result<
    (
        NativeStemsBeamVLinkTransaction,
        NativeStemsBeamVLinkTransactionState,
    ),
    String,
> {
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
    let attempt = predecessor_attempt(plans, frontier.plan.plan_ordinal)?;
    let create_frontier = fixture.one("stemsbeamcreatestemfrontier", system_id)?;
    let create_expand = fixture.one("stemsbeamcreatestemexpand", system_id)?;
    if create_frontier.usize("plan")? != frontier.plan.plan_ordinal
        || create_frontier.usize("builder")? != frontier.plan.builder_ordinal
        || create_frontier.i32("stemProfile")? != frontier.plan.stem_profile
        || create_frontier.i32("linkProfile")? != plans.link_profile
        || create_expand.usize("relations")? != attempt.relations.len()
        || create_expand.usize("glyphs")? != attempt.glyphs.len()
        || parse_stem_line(create_frontier.value("lineBefore")?, "lineBefore")?
            != attempt.stored_theoretical_line_before
        || parse_stem_line(create_expand.value("lineAfter")?, "lineAfter")?
            != attempt.stored_theoretical_line_after
        || (create_expand.value("lineChanged")? == "true")
            != attempt.stored_theoretical_line_would_mutate
    {
        return Err(format!("system {system_id} createStem frontier differs"));
    }
    let mut state = create_stem_state_from_predecessor(fixture, scheduler, attempt)?;
    let transaction = apply_native_stems_beam_vlink_create_stem_transaction(
        scheduler,
        builders,
        plans,
        &mut state,
        &create_stem_checker_context(page),
    )
    .map_err(|error| format!("system {system_id} createStem failed: {error}"))?;
    let result = fixture.one("stemsbeamcreatestemresult", system_id)?;
    let delta = fixture.one("stemsbeamcreatestemdelta", system_id)?;
    let registration = match transaction.registration.action {
        NativeStemsBeamGlyphRegistrationAction::Reused {
            reinserted_into_active_index: false,
        } => "ReuseActive",
        NativeStemsBeamGlyphRegistrationAction::Reused {
            reinserted_into_active_index: true,
        } => "ReinsertOriginal",
        NativeStemsBeamGlyphRegistrationAction::Registered => "New",
    };
    let disposition = match transaction.disposition {
        NativeStemsBeamCreateStemDisposition::CreatedChecked { .. } => "CreatedChecked",
        NativeStemsBeamCreateStemDisposition::CreatedArtificial { .. } => "CreatedArtificial",
        NativeStemsBeamCreateStemDisposition::Reused { .. } => "Reused",
        NativeStemsBeamCreateStemDisposition::Rejected => "Rejected",
    };
    let stem = transaction
        .stem
        .as_ref()
        .ok_or_else(|| format!("system {system_id} createStem returned null"))?;
    let grade = match &stem.grade {
        NativeStemsBeamStemGrade::Checked(check) => check.grade,
        NativeStemsBeamStemGrade::Artificial(grade) => *grade,
    };
    if transaction.plan != frontier.plan
        || transaction.candidate != independently_build_candidate(attempt)?
        || transaction.registration.alias_order != NativeStemsBeamGlyphAliasOrder::JavaGlyphId
        || transaction.registration.canonical_alias
            != parse_glyph_alias(result.value("registeredAlias")?)?
        || transaction.registration.glyph_id != result.i32("registeredGlyphId")?
        || transaction.registration.post_union_size != result.usize("postUnionSize")?
        || registration != result.value("registration")?
        || disposition != result.value("disposition")?
        || disposition != "CreatedChecked"
        || stem.glyph_id != transaction.registration.glyph_id
        || stem.glyph_content != transaction.candidate
        || stem.inter_id.is_some()
        || stem.sig_attached
        || stem.abnormal
        || result.i32("returnedStemInterId")? != 0
        || parse_hex_double(result.value("stemGrade")?, "stemGrade")?.to_bits() != grade.to_bits()
        || parse_stem_line(result.value("stemMedian")?, "stemMedian")? != stem.geometry.median
        || parse_hex_double(result.value("stemMeanThickness")?, "stemMeanThickness")?.to_bits()
            != stem.geometry.mean_thickness.to_bits()
        || parse_java_rectangle(result.value("stemBounds")?)? != stem.geometry.ribbon_bounds
        || result.value("stemAbnormal")? != "false"
        || result.value("stemSigAttached")? != "false"
        || state.glyph_index.exhaustive_lookup.is_some()
        || state.system_stems.exhaustive_lookup.is_some()
        || state.glyph_index.persistent_ids.sheet_last_id != delta.i32("allocatorAfter")?
        || state
            .system_stems
            .known_stems
            .iter()
            .filter(|known| *known == stem)
            .count()
            != 1
        || transaction.sig_vertex_mutation_count != 0
        || transaction.sig_relation_mutation_count != 0
        || transaction.linker_flag_mutation_count != 0
    {
        return Err(format!("system {system_id} createStem projection differs"));
    }
    Ok((transaction, state))
}

struct ReplayedBoundaryThirteen {
    create_transaction: NativeStemsBeamVLinkTransaction,
    transaction_state: NativeStemsBeamVLinkTransactionState,
    live_state: NativeStemsBeamVLinkReuseLiveState,
    relation_parameters: NativeStemsBeamRelationParameters,
    reuse_check: NativeStemsBeamVLinkReuseCheck,
}

fn validate_boundary_thirteen_continuity(
    rows: &[StrictRow],
    reuse_fixture: &PredecessorFixture,
) -> Result<(), String> {
    let baseline = one_row(rows, RowKind::Baseline)?;
    let frontier = one_row(rows, RowKind::Frontier)?;
    let compat = one_row(rows, RowKind::PredecessorCompat)?;
    let system_id = baseline.system_id()?;
    let reuse_baseline = reuse_fixture.one("stemsbeamvlinkreusecheckbaseline", system_id)?;
    let reuse_guard = reuse_fixture.one("stemsbeamvlinkreusecheckguard", system_id)?;
    for (base_field, reuse_field) in [
        ("allocator", "allocator"),
        ("glyphActive", "glyphActive"),
        ("glyphActiveHash", "glyphActiveHash"),
        ("glyphOriginals", "glyphOriginals"),
        ("glyphOriginalsHash", "glyphOriginalsHash"),
        ("interIndex", "interIndex"),
        ("noStaff", "noStaff"),
        ("lineStateHash", "lineStateHash"),
        ("relationInputHash", "relationInputHash"),
    ] {
        if baseline.value(base_field)? != reuse_baseline.value(reuse_field)? {
            return Err(format!(
                "system {system_id} boundary-13 continuity {base_field} differs"
            ));
        }
    }
    for (base_field, guard_field) in [
        ("allocator", "allocatorAfter"),
        ("glyphActiveHash", "glyphActiveHashAfter"),
        ("glyphOriginalsHash", "glyphOriginalsHashAfter"),
        ("lineStateHash", "lineStateHashAfter"),
        ("relationInputHash", "relationInputHashAfter"),
    ] {
        if baseline.value(base_field)? != reuse_guard.value(guard_field)? {
            return Err(format!(
                "system {system_id} boundary-13 guard continuity {base_field} differs"
            ));
        }
    }
    for (compat_field, reuse_row, reuse_field) in [
        ("legacyInterIndexHash", reuse_guard, "interIndexHashAfter"),
        ("legacySigVertices", reuse_baseline, "sigVertices"),
        ("legacySigEdges", reuse_baseline, "sigEdges"),
        ("legacySigHash", reuse_guard, "sigHashAfter"),
        (
            "legacySigRelationStateHash",
            reuse_guard,
            "sigRelationStateHashAfter",
        ),
        ("legacySystemStems", reuse_baseline, "systemStems"),
        ("legacySystemStemsHash", reuse_guard, "systemStemsHashAfter"),
        ("legacyLinkerStateHash", reuse_guard, "linkerStateHashAfter"),
    ] {
        if compat.value(compat_field)? != reuse_row.value(reuse_field)? {
            return Err(format!(
                "system {system_id} predecessor-compat {compat_field} differs"
            ));
        }
    }
    if compat.value("scope")? != "real"
        || compat.value("case")? != "-"
        || compat.value("phase")? != "Before"
        || compat.value("algorithm")? != "Boundary13FrozenV1"
    {
        return Err(format!(
            "system {system_id} predecessor-compat domain differs"
        ));
    }
    if frontier.value("predecessorJoin")? != "Exact"
        || frontier.value("predecessorTerminal")? != "ReadyBeforeSigMutation"
    {
        return Err(format!(
            "system {system_id} predecessor terminal continuity differs"
        ));
    }
    Ok(())
}

fn replay_boundary_thirteen(
    page: &NativePredecessorPage,
    create_fixture: &PredecessorFixture,
    reuse_fixture: &PredecessorFixture,
    system_id: usize,
) -> Result<ReplayedBoundaryThirteen, String> {
    let (create_transaction, transaction_state) =
        replay_create_stem_predecessor(page, create_fixture, system_id)?;
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
    let attempt = predecessor_attempt(plans, frontier.plan.plan_ordinal)?;
    // Real reuse-entry rows predate the later supplemental scope/case
    // columns. They are the only unscoped rows in this predecessor family;
    // supplemental entries use a distinct synthetic row label.
    let reuse_rows = reuse_fixture.all("stemsbeamvlinkreusecheckreuseentry", system_id);
    if reuse_rows.len() != attempt.relations.len() {
        return Err(format!("system {system_id} reuse-entry count differs"));
    }
    let entries = reuse_rows
        .into_iter()
        .zip(&attempt.relations)
        .map(|(row, relation)| {
            if row.value("sLinked")? != "false"
                || row.value("lookupState")? != "NotRead"
                || row.value("scanState")? != "NotRead"
                || row.usize("mapOrdinal")? != relation.map_ordinal
            {
                return Err(format!(
                    "system {system_id} real reuse entry differs from all-unlinked envelope"
                ));
            }
            Ok(NativeStemsBeamReuseEntryEvidence {
                map_ordinal: relation.map_ordinal,
                corner: relation.corner,
                observation: NativeStemsBeamReuseEntryObservation::Examined {
                    s_linker_linked: false,
                    head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::NotRead,
                },
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let live_state = NativeStemsBeamVLinkReuseLiveState {
        system_id,
        live_sig_stems: Vec::new(),
        evaluation: NativeStemsBeamVLinkReuseLiveEvaluation::Entries(entries),
    };
    let context = reuse_fixture.one_scoped(
        "stemsbeamvlinkreusecheckcheckcontext",
        system_id,
        "real",
        "-",
    )?;
    let relation_parameters = NativeStemsBeamRelationParameters {
        interline: context.i32("interline")?,
        main_stem_thickness: context
            .value("scaleStemThickness")?
            .parse()
            .map_err(|_| format!("system {system_id} invalid scale stem thickness"))?,
        profile: context.i32("profile")?,
        x_in_gap_maximum_profile0: parse_hex_double(
            context.value("portionXInP0")?,
            "portion x-in profile zero",
        )?,
        x_out_gap_maximum: parse_hex_double(context.value("xOutMax")?, "x-out maximum")?,
        y_gap_maximum: parse_hex_double(context.value("yMax")?, "y maximum")?,
        x_weight: parse_hex_double(context.value("xWeight")?, "x weight")?,
        y_weight: parse_hex_double(context.value("yWeight")?, "y weight")?,
        intrinsic_ratio: parse_hex_double(context.value("intrinsicRatio")?, "intrinsic ratio")?,
        minimum_grade: parse_hex_double(context.value("minGrade")?, "minimum grade")?,
    };
    let state_before = transaction_state.clone();
    let transaction_before = create_transaction.clone();
    let reuse_check = evaluate_native_stems_beam_vlink_reuse_check(
        scheduler,
        plans,
        stumps,
        vlinkers,
        &create_transaction,
        &transaction_state,
        &live_state,
        relation_parameters,
    )
    .map_err(|error| format!("system {system_id} reuse/check failed: {error}"))?;
    if transaction_state != state_before
        || create_transaction != transaction_before
        || reuse_check.reuse_disposition != NativeStemsBeamReuseDisposition::AllUnlinked
        || reuse_check.reuse_trace.len() != attempt.relations.len()
        || reuse_check.persistent_id_mutation_count != 0
        || reuse_check.system_stem_mutation_count != 0
        || reuse_check.sig_vertex_mutation_count != 0
        || reuse_check.sig_relation_mutation_count != 0
        || reuse_check.linker_flag_mutation_count != 0
        || !matches!(
            reuse_check.outcome,
            NativeStemsBeamVLinkReuseCheckOutcome::ReadyBeforeSigMutation { .. }
        )
    {
        return Err(format!("system {system_id} boundary-13 projection differs"));
    }
    Ok(ReplayedBoundaryThirteen {
        create_transaction,
        transaction_state,
        live_state,
        relation_parameters,
        reuse_check,
    })
}

fn parse_optional_usize(value: &str, label: &str) -> Result<Option<usize>, String> {
    if value == "-" {
        Ok(None)
    } else {
        value
            .parse()
            .map(Some)
            .map_err(|_| format!("invalid {label}: {value}"))
    }
}

fn parse_native_portion(value: &str) -> Result<NativeBeamPortion, String> {
    match value {
        "LEFT" => Ok(NativeBeamPortion::Left),
        "CENTER" => Ok(NativeBeamPortion::Center),
        "RIGHT" => Ok(NativeBeamPortion::Right),
        _ => Err(format!("invalid beam portion {value}")),
    }
}

fn build_real_base_state(
    page: &NativePredecessorPage,
    oracle_page: &StrictRow,
    rows: &[StrictRow],
    predecessor: &ReplayedBoundaryThirteen,
) -> Result<NativeStemsBeamVLinkBaseApplyState, String> {
    let baseline = one_row(rows, RowKind::Baseline)?;
    let frontier_row = one_row(rows, RowKind::Frontier)?;
    let system_id = baseline.system_id()?;
    if baseline.value("scope")? != "real" || baseline.value("case")? != "-" {
        return Err("real state builder received a supplemental block".to_owned());
    }
    let scheduler = page
        .scheduler
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or_else(|| format!("missing scheduler system {system_id}"))?;
    let stumps = page
        .beam_stumps
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or_else(|| format!("missing stump system {system_id}"))?;
    let frontier = match &scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => return Err(format!("system {system_id} lacks V transaction frontier")),
    };
    let beam = stumps
        .beams_by_abscissa
        .iter()
        .find(|beam| beam.source == frontier.beam)
        .ok_or_else(|| format!("system {system_id} lacks starting beam"))?;
    let final_stem = predecessor
        .reuse_check
        .final_stem
        .as_ref()
        .ok_or_else(|| format!("system {system_id} boundary 13 lacks final stem"))?;
    let NativeStemsBeamVLinkReuseCheckOutcome::ReadyBeforeSigMutation { relation } =
        &predecessor.reuse_check.outcome
    else {
        return Err(format!("system {system_id} boundary 13 is not ready"));
    };
    let (x_impact, y_impact) = parse_relation_impacts(frontier_row.value("impacts")?)?;
    if frontier.plan.plan_ordinal != parse_usize(frontier_row, "plan")?
        || predecessor.reuse_check.plan != frontier.plan
        || relation.beam != beam.source
        || relation.partner_stem_identity != final_stem.stem_identity
        || relation.partner_inter_id != final_stem.inter_id
        || relation.outgoing != parse_bool(frontier_row, "outgoing")?
        || relation.beam_portion != parse_native_portion(frontier_row.value("portion")?)?
        || relation.dx.to_bits()
            != parse_hex_double(frontier_row.value("dx")?, "frontier dx")?.to_bits()
        || relation.dy.to_bits()
            != parse_hex_double(frontier_row.value("dy")?, "frontier dy")?.to_bits()
        || relation.grade.to_bits()
            != parse_hex_double(frontier_row.value("grade")?, "frontier grade")?.to_bits()
        || relation.x_impact.to_bits() != x_impact.to_bits()
        || relation.y_impact.to_bits() != y_impact.to_bits()
        || relation.extension_point
            != parse_stem_point(frontier_row.value("extension")?, "frontier extension")?
        || frontier_row.value("partnerInterId")? != final_stem.inter_id.unwrap_or(0).to_string()
        || frontier_row.value("predecessorJoin")? != "Exact"
        || frontier_row.value("predecessorTerminal")? != "ReadyBeforeSigMutation"
    {
        return Err(format!(
            "system {system_id} boundary-13/frontier join differs"
        ));
    }
    let compact = project_compact_queries(oracle_page, rows)?;
    let beam_vertex_ordinal = parse_usize(baseline, "sigBeamVertexOrdinal")?;
    let stem_inter_id = final_stem.inter_id;
    let stem_vertex_ordinal = parse_optional_usize(
        baseline.value("sigStemVertexOrdinal")?,
        "stem vertex ordinal",
    )?;
    if stem_vertex_ordinal.is_some() != final_stem.sig_attached {
        return Err(format!("system {system_id} stem vertex attachment differs"));
    }
    if parse_i32(baseline, "stemInterId")? != final_stem.inter_id.unwrap_or(0)
        || parse_bool(baseline, "stemSigAttached")? != final_stem.sig_attached
        || parse_bool(baseline, "stemAbnormal")? != final_stem.abnormal
    {
        return Err(format!(
            "system {system_id} final stem/baseline state differs"
        ));
    }
    let beam_group = match (
        parse_optional_usize(
            baseline.value("beamGroupVertexOrdinal")?,
            "beam group vertex ordinal",
        )?,
        baseline.value("beamGroupStateHash")?,
    ) {
        (None, "-") => None,
        (Some(sig_vertex_ordinal), hash) if is_lower_sha256(hash) => {
            Some(NativeStemsBeamGroupRuntimeState {
                sig_vertex_ordinal,
                state_sha256: hash.to_owned(),
            })
        }
        _ => return Err(format!("system {system_id} beam group evidence differs")),
    };
    let stem_vertex = match (stem_inter_id, stem_vertex_ordinal) {
        (None, None) => NativeStemsBeamSigVertexLookup::Absent,
        (Some(inter_id), Some(vertex_ordinal)) => {
            NativeStemsBeamSigVertexLookup::PresentSameObject {
                vertex_ordinal,
                sig_vertex_identity: vertex_ordinal,
                inter_id,
                object_matches: parse_usize(baseline, "sigStemObjectMatches")?,
            }
        }
        _ => return Err(format!("system {system_id} stem vertex evidence differs")),
    };
    Ok(NativeStemsBeamVLinkBaseApplyState {
        transaction_state: predecessor.transaction_state.clone(),
        inter_index: compact.inter_index,
        sig: NativeStemsBeamSigApplyState {
            system_id,
            baseline_vertex_count: parse_usize(baseline, "sigVertices")?,
            baseline_vertex_provenance_sha256: baseline.value("sigVertexHash")?.to_owned(),
            baseline_relation_count: parse_usize(baseline, "sigEdges")?,
            baseline_relation_provenance_sha256: baseline.value("sigEdgeHash")?.to_owned(),
            beam_vertex: NativeStemsBeamSigVertexLookup::PresentSameObject {
                vertex_ordinal: beam_vertex_ordinal,
                sig_vertex_identity: beam_vertex_ordinal,
                inter_id: parse_i32(baseline, "beamInterId")?,
                object_matches: parse_usize(baseline, "sigBeamObjectMatches")?,
            },
            stem_vertex,
            appended_vertices: Vec::new(),
            appended_relations: Vec::new(),
            listener_topology: NativeStemsBeamSigListenerTopology::SoleStandardSigListener,
            beam: NativeStemsBeamVLinkBeamRuntimeState {
                source: beam.source,
                sig_vertex_identity: Some(beam_vertex_ordinal),
                inter_id: parse_i32(baseline, "beamInterId")?,
                inter_indexed: true,
                sig_system_id: system_id,
                removed: parse_bool(baseline, "beamRemoved")?,
                vip: parse_bool(baseline, "beamVip")?,
                abnormal: parse_bool(baseline, "beamAbnormal")?,
                stump_group_ordinal: beam.group_ordinal,
                beam_group,
            },
            stem: NativeStemsBeamVLinkStemRuntimeState {
                stem_identity: final_stem.stem_identity,
                sig_vertex_identity: stem_vertex_ordinal,
                inter_indexed: stem_inter_id.is_some(),
                sig_system_id: final_stem.sig_attached.then_some(system_id),
                removed: parse_bool(baseline, "stemRemoved")?,
                vip: parse_bool(baseline, "stemVip")?,
                abnormal: parse_bool(baseline, "stemAbnormal")?,
            },
        },
        sheet_edit: compact.sheet_edit,
        certificate: Some(compact.certificate),
        committed: None,
        committed_apply: None,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BeamKind {
    Raw,
    Hook,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BeamPortion {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EdgeDirection {
    Incoming,
    Outgoing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelationKind {
    BeamStem { portion: Option<BeamPortion> },
    BeamRest { portion: Option<BeamPortion> },
    ChordStem,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct IncidentEdge {
    graph_relation_identity: usize,
    relation_object_identity: usize,
    direction: EdgeDirection,
    opposite_object_identity: usize,
    kind: RelationKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PersistentIds {
    sheet_last_id: i32,
    glyph_index_last_id: i32,
    inter_index_last_id: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LiveInter {
    object_identity: usize,
    inter_id: Option<i32>,
    sig_system_id: Option<usize>,
    in_sig: bool,
    in_inter_index: bool,
    removed: bool,
    abnormal: bool,
    vip: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ModificationFlags {
    sheet_stub_modified: bool,
    book_modified: bool,
    book_dirty: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FreshBeamStemDraft {
    relation_object_identity: usize,
    grade: f64,
    portion: Option<BeamPortion>,
    extension_present: bool,
    outgoing: bool,
    attached: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct IndependentApplyState {
    system_id: usize,
    persistent_ids: PersistentIds,
    inter_index_count: usize,
    inter_index_slots: BTreeMap<i32, usize>,
    sig_vertex_count: usize,
    sig_edge_count: usize,
    next_inter_id_is_vip: bool,
    beam_kind: BeamKind,
    beam: LiveInter,
    stem: LiveInter,
    beam_incidents: Vec<IncidentEdge>,
    stem_incidents: Vec<IncidentEdge>,
    relation: FreshBeamStemDraft,
    flags: ModificationFlags,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IndependentDisposition {
    Added,
    SuppressedSourceRemoved,
    SuppressedTargetRemoved,
    SuppressedExistingBeamStem { graph_relation_identity: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VertexAction {
    Registered {
        inter_id: i32,
        sig_vertex_ordinal: usize,
    },
    AlreadyLive {
        inter_id: i32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IndependentMutation {
    PersistentIdAllocated(i32),
    StemInterIdAssigned(i32),
    InterIndexMapped(i32),
    StemVipSet,
    SigVertexInserted(usize),
    StemSigAttached,
    StemRemovedCleared,
    StemAbnormalSet,
    ModificationFlagsSet,
    SigEdgeInserted {
        graph_relation_identity: usize,
        relation_object_identity: usize,
    },
    RelationCallbackDispatched,
    BeamAbnormalEvaluated,
    BeamAbnormalSet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct IndependentDuplicateScanEntry {
    pair_ordinal: usize,
    source_outgoing_ordinal: usize,
    graph_relation_identity: usize,
    relation_object_identity: usize,
    class_check: Option<bool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IndependentBeamReadState {
    ExaminedClassOnly,
    ExaminedClassAndPortion,
    UnreadAfterBreak,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IndependentBeamContribution {
    None,
    Left,
    Right,
    FirstBeamStem,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct IndependentBeamScanEntry {
    incident_ordinal: usize,
    direction_ordinal: usize,
    graph_relation_identity: usize,
    relation_object_identity: usize,
    direction: EdgeDirection,
    read_state: IndependentBeamReadState,
    relevant: bool,
    /// Exact portion returned by `getBeamPortion`, when that accessor ran.
    /// Hook evaluation uses only `Class.isInstance` and must leave this empty.
    portion_read: Option<BeamPortion>,
    contribution: IndependentBeamContribution,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndependentBeamTrace {
    rows: Vec<IndependentBeamScanEntry>,
    left: Option<bool>,
    right: Option<bool>,
    requested_abnormal: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct IndependentApplyResult {
    disposition: IndependentDisposition,
    vertex_action: VertexAction,
    apply_returned: bool,
    continuation_support_grade: f64,
    attached_graph_relation_identity: Option<usize>,
    attached_relation_object_identity: Option<usize>,
    source_removed_read: bool,
    target_removed_read: Option<bool>,
    source_outgoing_scanned: usize,
    duplicate_scan: Vec<IndependentDuplicateScanEntry>,
    pre_beam_trace: IndependentBeamTrace,
    post_beam_trace: Option<IndependentBeamTrace>,
    post_stem_incidents: Vec<IncidentEdge>,
    post_beam_incidents: Vec<IncidentEdge>,
    mutations: Vec<IndependentMutation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IndependentApplyError {
    InvalidSystem,
    AllocatorViewsDiffer,
    PersistentIdExhausted,
    InvalidBeam,
    InvalidStem,
    InterIndexCollision,
    IncidentOrder,
    DuplicateRelationIdentity,
    InvalidDraft,
    UnsupportedChordCallback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JavaEnvelopeCase {
    MissingPositiveTarget,
    NewVertexListener,
    EarlyEdgeListener,
    LaterEdgeListener,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JavaThrowStage {
    VertexAdd,
    LinkApply,
}

#[derive(Clone, Debug, PartialEq)]
struct JavaEnvelopePrefix {
    throw_stage: JavaThrowStage,
    vertex_returned: Option<bool>,
    apply_returned: Option<bool>,
    callback_completed: bool,
    mutations: Vec<IndependentMutation>,
}

fn apply_independent_base_transaction(
    state: &mut IndependentApplyState,
) -> Result<IndependentApplyResult, IndependentApplyError> {
    let mut candidate = state.clone();
    let result = apply_independent_base_transaction_inner(&mut candidate)?;
    *state = candidate;
    Ok(result)
}

fn apply_independent_base_transaction_inner(
    state: &mut IndependentApplyState,
) -> Result<IndependentApplyResult, IndependentApplyError> {
    validate_common_state(state)?;
    let pre_beam_trace = independent_beam_trace(state.beam_kind, &state.beam_incidents);
    let mut mutations = Vec::new();
    let vertex_action = materialize_stem_vertex(state, &mut mutations)?;

    if state.beam.removed {
        return Ok(suppressed_result(
            state,
            vertex_action,
            IndependentDisposition::SuppressedSourceRemoved,
            None,
            0,
            Vec::new(),
            pre_beam_trace,
            mutations,
        ));
    }
    if state.stem.removed {
        return Ok(suppressed_result(
            state,
            vertex_action,
            IndependentDisposition::SuppressedTargetRemoved,
            Some(true),
            0,
            Vec::new(),
            pre_beam_trace,
            mutations,
        ));
    }

    let (source_outgoing_scanned, duplicate_scan, existing_graph_relation_identity) =
        independent_duplicate_scan(state);
    if let Some(graph_relation_identity) = existing_graph_relation_identity {
        return Ok(suppressed_result(
            state,
            vertex_action,
            IndependentDisposition::SuppressedExistingBeamStem {
                graph_relation_identity,
            },
            Some(false),
            source_outgoing_scanned,
            duplicate_scan,
            pre_beam_trace,
            mutations,
        ));
    }

    validate_callback_state(state)?;
    let (graph_relation_identity, relation_object_identity) =
        insert_beam_stem_edge_structural(state, &mut mutations);
    let post_beam_trace = dispatch_relation_callback(state, &mut mutations)?;

    Ok(IndependentApplyResult {
        disposition: IndependentDisposition::Added,
        vertex_action,
        apply_returned: true,
        continuation_support_grade: state.relation.grade,
        attached_graph_relation_identity: Some(graph_relation_identity),
        attached_relation_object_identity: Some(relation_object_identity),
        source_removed_read: true,
        target_removed_read: Some(false),
        source_outgoing_scanned,
        duplicate_scan,
        pre_beam_trace,
        post_beam_trace: Some(post_beam_trace),
        post_stem_incidents: state.stem_incidents.clone(),
        post_beam_incidents: state.beam_incidents.clone(),
        mutations,
    })
}

fn insert_beam_stem_edge_structural(
    state: &mut IndependentApplyState,
    mutations: &mut Vec<IndependentMutation>,
) -> (usize, usize) {
    let graph_relation_identity = state.sig_edge_count;
    let relation_object_identity = state.relation.relation_object_identity;
    state.sig_edge_count += 1;
    state.relation.attached = true;
    mutations.push(IndependentMutation::SigEdgeInserted {
        graph_relation_identity,
        relation_object_identity,
    });

    let new_stem_edge = IncidentEdge {
        graph_relation_identity,
        relation_object_identity,
        direction: EdgeDirection::Incoming,
        opposite_object_identity: state.beam.object_identity,
        kind: RelationKind::BeamStem {
            portion: state.relation.portion,
        },
    };
    insert_at_end_of_incoming(&mut state.stem_incidents, new_stem_edge);
    let new_beam_edge = IncidentEdge {
        graph_relation_identity,
        relation_object_identity,
        direction: EdgeDirection::Outgoing,
        opposite_object_identity: state.stem.object_identity,
        kind: RelationKind::BeamStem {
            portion: state.relation.portion,
        },
    };
    state.beam_incidents.push(new_beam_edge);
    (graph_relation_identity, relation_object_identity)
}

fn dispatch_relation_callback(
    state: &mut IndependentApplyState,
    mutations: &mut Vec<IndependentMutation>,
) -> Result<IndependentBeamTrace, IndependentApplyError> {
    mutations.push(IndependentMutation::RelationCallbackDispatched);

    // Java's callback scans the already-inserted BeamStem edge but finds no
    // ChordStem match in this compact v1 envelope.
    if state
        .stem_incidents
        .iter()
        .any(|edge| edge.kind == RelationKind::ChordStem)
    {
        return Err(IndependentApplyError::UnsupportedChordCallback);
    }

    let post_beam_trace = independent_beam_trace(state.beam_kind, &state.beam_incidents);
    let beam_abnormal = post_beam_trace.requested_abnormal;
    mutations.push(IndependentMutation::BeamAbnormalEvaluated);
    if beam_abnormal != state.beam.abnormal {
        state.beam.abnormal = beam_abnormal;
        mutations.push(IndependentMutation::BeamAbnormalSet);
        set_modified_flags(state, mutations);
    }
    Ok(post_beam_trace)
}

/// Replay Java's deliberately unsupported exception envelope without rollback.
///
/// Production v1 rejects these listener/topology cases before mutation. The
/// oracle nevertheless records them so the gate can prove the exact Java
/// prefix that would survive an exception.
fn apply_java_envelope_prefix(
    state: &mut IndependentApplyState,
    case: JavaEnvelopeCase,
) -> Result<JavaEnvelopePrefix, IndependentApplyError> {
    match case {
        JavaEnvelopeCase::MissingPositiveTarget => {
            validate_java_envelope_source_and_draft(state)?;
            if state.stem.inter_id.is_none_or(|id| id <= 0)
                || state.stem.sig_system_id.is_some()
                || state.stem.in_sig
                || state.stem.in_inter_index
            {
                return Err(IndependentApplyError::InvalidStem);
            }
            Ok(JavaEnvelopePrefix {
                throw_stage: JavaThrowStage::LinkApply,
                vertex_returned: None,
                apply_returned: None,
                callback_completed: false,
                mutations: Vec::new(),
            })
        }
        JavaEnvelopeCase::NewVertexListener => {
            validate_common_state(state)?;
            if state.stem.inter_id.is_some() {
                return Err(IndependentApplyError::InvalidStem);
            }
            let mut mutations = Vec::new();
            let inter_id = register_stem_before_vertex_event(state, &mut mutations)?;
            let vertex_ordinal = state.sig_vertex_count;
            state.sig_vertex_count += 1;
            // JGraphT mutates the graph before it dispatches vertex listeners.
            // The injected listener precedes SigListener, so the Inter's SIG
            // pointer and StemInter.added abnormal callback remain untouched.
            state.stem.in_sig = true;
            mutations.push(IndependentMutation::SigVertexInserted(vertex_ordinal));
            debug_assert_eq!(state.stem.inter_id, Some(inter_id));
            Ok(JavaEnvelopePrefix {
                throw_stage: JavaThrowStage::VertexAdd,
                vertex_returned: None,
                apply_returned: None,
                callback_completed: false,
                mutations,
            })
        }
        JavaEnvelopeCase::EarlyEdgeListener | JavaEnvelopeCase::LaterEdgeListener => {
            validate_common_state(state)?;
            if state.stem.inter_id.is_none() {
                return Err(IndependentApplyError::InvalidStem);
            }
            if state.beam.removed || state.stem.removed {
                return Err(IndependentApplyError::InvalidStem);
            }
            let (_, _, duplicate) = independent_duplicate_scan(state);
            if duplicate.is_some() {
                return Err(IndependentApplyError::InvalidDraft);
            }
            validate_callback_state(state)?;
            let mut mutations = Vec::new();
            insert_beam_stem_edge_structural(state, &mut mutations);
            let callback_completed = case == JavaEnvelopeCase::LaterEdgeListener;
            if callback_completed {
                dispatch_relation_callback(state, &mut mutations)?;
            }
            Ok(JavaEnvelopePrefix {
                throw_stage: JavaThrowStage::LinkApply,
                vertex_returned: None,
                apply_returned: None,
                callback_completed,
                mutations,
            })
        }
    }
}

fn validate_java_envelope_source_and_draft(
    state: &IndependentApplyState,
) -> Result<(), IndependentApplyError> {
    if state.system_id == 0 {
        return Err(IndependentApplyError::InvalidSystem);
    }
    if state.persistent_ids.sheet_last_id != state.persistent_ids.glyph_index_last_id
        || state.persistent_ids.sheet_last_id != state.persistent_ids.inter_index_last_id
    {
        return Err(IndependentApplyError::AllocatorViewsDiffer);
    }
    if state.beam.inter_id.is_none_or(|id| id <= 0)
        || state.beam.sig_system_id != Some(state.system_id)
        || !state.beam.in_sig
        || !state.beam.in_inter_index
    {
        return Err(IndependentApplyError::InvalidBeam);
    }
    if state.relation.relation_object_identity == 0
        || !state.relation.grade.is_finite()
        || state.relation.grade < 0.0
        || state.relation.portion.is_none()
        || !state.relation.extension_present
        || !state.relation.outgoing
        || state.relation.attached
    {
        return Err(IndependentApplyError::InvalidDraft);
    }
    validate_incident_order(&state.beam_incidents)?;
    validate_incident_order(&state.stem_incidents)
}

fn register_stem_before_vertex_event(
    state: &mut IndependentApplyState,
    mutations: &mut Vec<IndependentMutation>,
) -> Result<i32, IndependentApplyError> {
    let inter_id = state
        .persistent_ids
        .sheet_last_id
        .checked_add(1)
        .filter(|id| *id > 0)
        .ok_or(IndependentApplyError::PersistentIdExhausted)?;
    if state.inter_index_slots.contains_key(&inter_id) {
        return Err(IndependentApplyError::InterIndexCollision);
    }
    state.persistent_ids = PersistentIds {
        sheet_last_id: inter_id,
        glyph_index_last_id: inter_id,
        inter_index_last_id: inter_id,
    };
    mutations.push(IndependentMutation::PersistentIdAllocated(inter_id));
    state.stem.inter_id = Some(inter_id);
    mutations.push(IndependentMutation::StemInterIdAssigned(inter_id));
    state
        .inter_index_slots
        .insert(inter_id, state.stem.object_identity);
    state.inter_index_count += 1;
    state.stem.in_inter_index = true;
    mutations.push(IndependentMutation::InterIndexMapped(inter_id));
    if state.next_inter_id_is_vip && !state.stem.vip {
        state.stem.vip = true;
        mutations.push(IndependentMutation::StemVipSet);
    }
    Ok(inter_id)
}

fn validate_common_state(state: &IndependentApplyState) -> Result<(), IndependentApplyError> {
    if state.system_id == 0 {
        return Err(IndependentApplyError::InvalidSystem);
    }
    if state.persistent_ids.sheet_last_id != state.persistent_ids.glyph_index_last_id
        || state.persistent_ids.sheet_last_id != state.persistent_ids.inter_index_last_id
    {
        return Err(IndependentApplyError::AllocatorViewsDiffer);
    }
    if state.beam.inter_id.is_none_or(|id| id <= 0)
        || state.beam.sig_system_id != Some(state.system_id)
        || (state.beam.removed
            && (state.beam.in_sig || state.beam.in_inter_index || !state.beam_incidents.is_empty()))
        || (!state.beam.removed && (!state.beam.in_sig || !state.beam.in_inter_index))
    {
        return Err(IndependentApplyError::InvalidBeam);
    }
    if state.relation.relation_object_identity == 0
        || !state.relation.grade.is_finite()
        || state.relation.grade < 0.0
        || state.relation.portion.is_none()
        || !state.relation.extension_present
        || !state.relation.outgoing
        || state.relation.attached
    {
        return Err(IndependentApplyError::InvalidDraft);
    }
    validate_incident_order(&state.beam_incidents)?;
    validate_incident_order(&state.stem_incidents)?;
    let mut object_by_graph = BTreeMap::new();
    let mut graph_by_object = BTreeMap::new();
    for edge in state.beam_incidents.iter().chain(&state.stem_incidents) {
        if edge.graph_relation_identity >= state.sig_edge_count
            || object_by_graph
                .insert(edge.graph_relation_identity, edge.relation_object_identity)
                .is_some_and(|prior| prior != edge.relation_object_identity)
            || graph_by_object
                .insert(edge.relation_object_identity, edge.graph_relation_identity)
                .is_some_and(|prior| prior != edge.graph_relation_identity)
        {
            return Err(IndependentApplyError::DuplicateRelationIdentity);
        }
    }
    if state
        .beam_incidents
        .iter()
        .chain(&state.stem_incidents)
        .any(|edge| edge.relation_object_identity == state.relation.relation_object_identity)
    {
        return Err(IndependentApplyError::InvalidDraft);
    }
    for beam_edge in &state.beam_incidents {
        if let Some(stem_edge) = state
            .stem_incidents
            .iter()
            .find(|edge| edge.graph_relation_identity == beam_edge.graph_relation_identity)
        {
            if beam_edge.direction != EdgeDirection::Outgoing
                || stem_edge.direction != EdgeDirection::Incoming
                || beam_edge.relation_object_identity != stem_edge.relation_object_identity
                || beam_edge.opposite_object_identity != state.stem.object_identity
                || stem_edge.opposite_object_identity != state.beam.object_identity
                || beam_edge.kind != stem_edge.kind
            {
                return Err(IndependentApplyError::DuplicateRelationIdentity);
            }
        }
    }
    Ok(())
}

fn materialize_stem_vertex(
    state: &mut IndependentApplyState,
    mutations: &mut Vec<IndependentMutation>,
) -> Result<VertexAction, IndependentApplyError> {
    if let Some(inter_id) = state.stem.inter_id {
        if inter_id <= 0 {
            return Err(IndependentApplyError::InvalidStem);
        }
        if state.stem.removed
            && (state.stem.sig_system_id != Some(state.system_id)
                || state.stem.in_sig
                || state.stem.in_inter_index
                || !state.stem_incidents.is_empty())
        {
            return Err(IndependentApplyError::InvalidStem);
        }
        if !state.stem.removed
            && (state.stem.sig_system_id != Some(state.system_id)
                || !state.stem.in_sig
                || !state.stem.in_inter_index
                || state.inter_index_slots.get(&inter_id) != Some(&state.stem.object_identity))
        {
            return Err(IndependentApplyError::InvalidStem);
        }
        return Ok(VertexAction::AlreadyLive { inter_id });
    }
    if state.stem.sig_system_id.is_some()
        || state.stem.in_sig
        || state.stem.in_inter_index
        || !state.stem_incidents.is_empty()
    {
        return Err(IndependentApplyError::InvalidStem);
    }
    let inter_id = state
        .persistent_ids
        .sheet_last_id
        .checked_add(1)
        .filter(|id| *id > 0)
        .ok_or(IndependentApplyError::PersistentIdExhausted)?;
    if state.inter_index_slots.contains_key(&inter_id) {
        return Err(IndependentApplyError::InterIndexCollision);
    }
    state.persistent_ids = PersistentIds {
        sheet_last_id: inter_id,
        glyph_index_last_id: inter_id,
        inter_index_last_id: inter_id,
    };
    mutations.push(IndependentMutation::PersistentIdAllocated(inter_id));
    state.stem.inter_id = Some(inter_id);
    mutations.push(IndependentMutation::StemInterIdAssigned(inter_id));
    state
        .inter_index_slots
        .insert(inter_id, state.stem.object_identity);
    state.inter_index_count += 1;
    state.stem.in_inter_index = true;
    mutations.push(IndependentMutation::InterIndexMapped(inter_id));
    if state.next_inter_id_is_vip && !state.stem.vip {
        state.stem.vip = true;
        mutations.push(IndependentMutation::StemVipSet);
    }
    let vertex_ordinal = state.sig_vertex_count;
    state.sig_vertex_count += 1;
    state.stem.in_sig = true;
    mutations.push(IndependentMutation::SigVertexInserted(vertex_ordinal));
    state.stem.sig_system_id = Some(state.system_id);
    mutations.push(IndependentMutation::StemSigAttached);
    if state.stem.removed {
        state.stem.removed = false;
        mutations.push(IndependentMutation::StemRemovedCleared);
    }
    if !state.stem.abnormal {
        state.stem.abnormal = true;
        mutations.push(IndependentMutation::StemAbnormalSet);
        set_modified_flags(state, mutations);
    }
    Ok(VertexAction::Registered {
        inter_id,
        sig_vertex_ordinal: vertex_ordinal,
    })
}

fn set_modified_flags(state: &mut IndependentApplyState, mutations: &mut Vec<IndependentMutation>) {
    state.flags = ModificationFlags {
        sheet_stub_modified: true,
        book_modified: true,
        book_dirty: true,
    };
    mutations.push(IndependentMutation::ModificationFlagsSet);
}

fn independent_duplicate_scan(
    state: &IndependentApplyState,
) -> (usize, Vec<IndependentDuplicateScanEntry>, Option<usize>) {
    let mut source_outgoing_ordinal = 0;
    let mut first_match = None;
    let mut rows = Vec::new();
    for edge in &state.beam_incidents {
        if edge.direction != EdgeDirection::Outgoing {
            continue;
        }
        let current_outgoing_ordinal = source_outgoing_ordinal;
        source_outgoing_ordinal += 1;
        if edge.opposite_object_identity != state.stem.object_identity {
            continue;
        }
        let matches = matches!(edge.kind, RelationKind::BeamStem { .. });
        let class_check = first_match.is_none().then_some(matches);
        if class_check == Some(true) {
            first_match = Some(edge.graph_relation_identity);
        }
        rows.push(IndependentDuplicateScanEntry {
            pair_ordinal: rows.len(),
            source_outgoing_ordinal: current_outgoing_ordinal,
            graph_relation_identity: edge.graph_relation_identity,
            relation_object_identity: edge.relation_object_identity,
            class_check,
        });
    }
    (source_outgoing_ordinal, rows, first_match)
}

fn validate_callback_state(state: &IndependentApplyState) -> Result<(), IndependentApplyError> {
    if state.stem.sig_system_id != Some(state.system_id)
        || !state.stem.in_sig
        || state.beam.sig_system_id != Some(state.system_id)
    {
        return Err(IndependentApplyError::InvalidStem);
    }
    if state
        .stem_incidents
        .iter()
        .any(|edge| edge.kind == RelationKind::ChordStem)
    {
        return Err(IndependentApplyError::UnsupportedChordCallback);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)] // Mirrors one exact Java suppression trace record.
fn suppressed_result(
    state: &IndependentApplyState,
    vertex_action: VertexAction,
    disposition: IndependentDisposition,
    target_removed_read: Option<bool>,
    source_outgoing_scanned: usize,
    duplicate_scan: Vec<IndependentDuplicateScanEntry>,
    pre_beam_trace: IndependentBeamTrace,
    mutations: Vec<IndependentMutation>,
) -> IndependentApplyResult {
    IndependentApplyResult {
        disposition,
        vertex_action,
        apply_returned: false,
        continuation_support_grade: state.relation.grade,
        attached_graph_relation_identity: None,
        attached_relation_object_identity: None,
        source_removed_read: true,
        target_removed_read,
        source_outgoing_scanned,
        duplicate_scan,
        pre_beam_trace,
        post_beam_trace: None,
        post_stem_incidents: state.stem_incidents.clone(),
        post_beam_incidents: state.beam_incidents.clone(),
        mutations,
    }
}

fn validate_incident_order(edges: &[IncidentEdge]) -> Result<(), IndependentApplyError> {
    let mut saw_outgoing = false;
    let mut last_incoming = None;
    let mut last_outgoing = None;
    let mut graph_identities = BTreeSet::new();
    let mut object_identities = BTreeSet::new();
    for edge in edges {
        if !graph_identities.insert(edge.graph_relation_identity)
            || !object_identities.insert(edge.relation_object_identity)
        {
            return Err(IndependentApplyError::DuplicateRelationIdentity);
        }
        match edge.direction {
            EdgeDirection::Incoming if saw_outgoing => {
                return Err(IndependentApplyError::IncidentOrder);
            }
            EdgeDirection::Incoming => {
                if last_incoming.is_some_and(|prior| prior >= edge.graph_relation_identity) {
                    return Err(IndependentApplyError::IncidentOrder);
                }
                last_incoming = Some(edge.graph_relation_identity);
            }
            EdgeDirection::Outgoing => {
                if last_outgoing.is_some_and(|prior| prior >= edge.graph_relation_identity) {
                    return Err(IndependentApplyError::IncidentOrder);
                }
                last_outgoing = Some(edge.graph_relation_identity);
                saw_outgoing = true;
            }
        }
    }
    Ok(())
}

fn insert_at_end_of_incoming(edges: &mut Vec<IncidentEdge>, edge: IncidentEdge) {
    let insertion = edges
        .iter()
        .position(|candidate| candidate.direction == EdgeDirection::Outgoing)
        .unwrap_or(edges.len());
    edges.insert(insertion, edge);
}

fn independent_beam_trace(kind: BeamKind, edges: &[IncidentEdge]) -> IndependentBeamTrace {
    let mut incoming_ordinal = 0;
    let mut outgoing_ordinal = 0;
    let direction_ordinals = edges
        .iter()
        .map(|edge| match edge.direction {
            EdgeDirection::Incoming => {
                let ordinal = incoming_ordinal;
                incoming_ordinal += 1;
                ordinal
            }
            EdgeDirection::Outgoing => {
                let ordinal = outgoing_ordinal;
                outgoing_ordinal += 1;
                ordinal
            }
        })
        .collect::<Vec<_>>();

    if kind == BeamKind::Hook {
        let mut found = false;
        let rows = edges
            .iter()
            .zip(direction_ordinals)
            .enumerate()
            .map(|(incident_ordinal, (edge, direction_ordinal))| {
                let examined = !found;
                let relevant = examined && matches!(edge.kind, RelationKind::BeamStem { .. });
                if relevant {
                    found = true;
                }
                IndependentBeamScanEntry {
                    incident_ordinal,
                    direction_ordinal,
                    graph_relation_identity: edge.graph_relation_identity,
                    relation_object_identity: edge.relation_object_identity,
                    direction: edge.direction,
                    read_state: if examined {
                        IndependentBeamReadState::ExaminedClassOnly
                    } else {
                        IndependentBeamReadState::UnreadAfterBreak
                    },
                    relevant,
                    // `BeamHookInter.checkAbnormal` delegates to
                    // `SIGraph.hasRelation`; it never calls getBeamPortion.
                    portion_read: None,
                    contribution: if relevant {
                        IndependentBeamContribution::FirstBeamStem
                    } else {
                        IndependentBeamContribution::None
                    },
                }
            })
            .collect();
        return IndependentBeamTrace {
            rows,
            left: None,
            right: None,
            requested_abnormal: !found,
        };
    }

    let mut left = false;
    let mut right = false;
    let rows = edges
        .iter()
        .zip(direction_ordinals)
        .enumerate()
        .map(|(incident_ordinal, (edge, direction_ordinal))| {
            let portion = match edge.kind {
                RelationKind::BeamStem { portion } | RelationKind::BeamRest { portion } => portion,
                RelationKind::ChordStem | RelationKind::Other => None,
            };
            let contribution = match portion {
                Some(BeamPortion::Left) => {
                    left = true;
                    IndependentBeamContribution::Left
                }
                Some(BeamPortion::Right) => {
                    right = true;
                    IndependentBeamContribution::Right
                }
                Some(BeamPortion::Center) | None => IndependentBeamContribution::None,
            };
            IndependentBeamScanEntry {
                incident_ordinal,
                direction_ordinal,
                graph_relation_identity: edge.graph_relation_identity,
                relation_object_identity: edge.relation_object_identity,
                direction: edge.direction,
                read_state: if portion.is_some() {
                    IndependentBeamReadState::ExaminedClassAndPortion
                } else {
                    IndependentBeamReadState::ExaminedClassOnly
                },
                relevant: portion.is_some(),
                portion_read: portion,
                contribution,
            }
        })
        .collect();
    IndependentBeamTrace {
        rows,
        left: Some(left),
        right: Some(right),
        requested_abnormal: !left || !right,
    }
}

fn independent_portion(value: &str) -> Result<Option<BeamPortion>, String> {
    match value {
        "-" => Ok(None),
        "LEFT" => Ok(Some(BeamPortion::Left)),
        "CENTER" => Ok(Some(BeamPortion::Center)),
        "RIGHT" => Ok(Some(BeamPortion::Right)),
        _ => Err(format!("invalid independent beam portion {value}")),
    }
}

fn independent_relation_object_identity(
    value: &str,
    draft_alias: &str,
    plan: usize,
) -> Result<usize, String> {
    match parse_relation_object_identity(value, draft_alias, plan)? {
        NativeStemsBeamRelationObjectIdentity::GraphObject(identity) => identity
            .checked_add(1)
            .ok_or_else(|| "relation-object identity overflow".to_owned()),
        NativeStemsBeamRelationObjectIdentity::FreshDraft(plan) => 1_000_000_usize
            .checked_add(plan)
            .ok_or_else(|| "fresh-draft identity overflow".to_owned()),
    }
}

fn independent_relation_kind(class: &str, portion: Option<BeamPortion>) -> RelationKind {
    match class.rsplit('.').next().unwrap_or(class) {
        "BeamStemRelation" => RelationKind::BeamStem { portion },
        "BeamRestRelation" => RelationKind::BeamRest { portion },
        "ChordStemRelation" => RelationKind::ChordStem,
        _ => RelationKind::Other,
    }
}

fn project_independent_incidents(
    rows: &[StrictRow],
    phase: &str,
    endpoint: RowKind,
    draft_alias: &str,
    plan: usize,
) -> Result<Vec<IncidentEdge>, String> {
    let frontier = one_row(rows, RowKind::Frontier)?;
    let source = rows_of_kind(rows, endpoint)
        .into_iter()
        .filter(|row| row.value("phase") == Ok(phase))
        .collect::<Vec<_>>();
    let beam_rows = rows_of_kind(rows, RowKind::BeamIncident);
    source
        .into_iter()
        .map(|row| {
            let graph_identity = parse_sig_edge_alias(row.value("globalRelationIdentity")?)?;
            let portion = if row.value("relationObjectIdentity")? == draft_alias {
                // Hook abnormality scans intentionally do not call
                // getBeamPortion, but the fresh relation object still retains
                // the exact checked draft portion.
                independent_portion(frontier.value("portion")?)?
            } else if endpoint == RowKind::BeamIncident {
                independent_portion(row.value("portion")?)?
            } else {
                beam_rows
                    .iter()
                    .find(|beam| {
                        beam.value("phase") == Ok(phase)
                            && beam.value("globalRelationIdentity")
                                == row.value("globalRelationIdentity")
                    })
                    .map(|beam| independent_portion(beam.value("portion")?))
                    .transpose()?
                    .flatten()
            };
            Ok(IncidentEdge {
                graph_relation_identity: graph_identity,
                relation_object_identity: independent_relation_object_identity(
                    row.value("relationObjectIdentity")?,
                    draft_alias,
                    plan,
                )?,
                direction: match row.value("direction")? {
                    "Incoming" => EdgeDirection::Incoming,
                    "Outgoing" => EdgeDirection::Outgoing,
                    value => return Err(format!("invalid independent direction {value}")),
                },
                // Dense vertex ordinal is an exact object identity for this
                // independent model. It is intentionally unrelated to Java's
                // persistent Inter ID.
                opposite_object_identity: parse_usize(row, "oppositeVertexOrdinal")? + 1,
                kind: independent_relation_kind(row.value("relationClass")?, portion),
            })
        })
        .collect()
}

fn build_independent_real_state(rows: &[StrictRow]) -> Result<IndependentApplyState, String> {
    let baseline = one_row(rows, RowKind::Baseline)?;
    let frontier = one_row(rows, RowKind::Frontier)?;
    let system_id = baseline.system_id()?;
    let plan = parse_usize(frontier, "plan")?;
    let draft_alias = frontier.value("freshLinkAlias")?;
    let beam_vertex = parse_optional_usize(
        baseline.value("sigBeamVertexOrdinal")?,
        "independent beam vertex",
    )?;
    let existing_stem_vertex = parse_optional_usize(
        baseline.value("sigStemVertexOrdinal")?,
        "independent stem vertex",
    )?;
    let beam_object_identity = beam_vertex.map_or(1_900_001, |ordinal| ordinal + 1);
    let stem_vertex = existing_stem_vertex.unwrap_or(parse_usize(baseline, "sigVertices")?);
    let beam_inter_id = parse_i32(baseline, "beamInterId")?;
    let stem_inter_id = match parse_i32(baseline, "stemInterId")? {
        0 => None,
        value if value > 0 => Some(value),
        value => return Err(format!("invalid independent stem Inter ID {value}")),
    };
    let beam_indexed = baseline.value("interIndexBeamIndexOrdinal")? != "-";
    let stem_indexed = baseline.value("interIndexStemIndexOrdinal")? != "-";
    let mut inter_index_slots = BTreeMap::new();
    if beam_indexed {
        inter_index_slots.insert(beam_inter_id, beam_object_identity);
    }
    if let Some(inter_id) = stem_inter_id.filter(|_| stem_indexed) {
        inter_index_slots.insert(inter_id, stem_vertex + 1);
    }
    Ok(IndependentApplyState {
        system_id,
        persistent_ids: PersistentIds {
            sheet_last_id: parse_i32(baseline, "allocator")?,
            glyph_index_last_id: parse_i32(baseline, "allocator")?,
            inter_index_last_id: parse_i32(baseline, "allocator")?,
        },
        inter_index_count: parse_usize(baseline, "interIndex")?,
        inter_index_slots,
        sig_vertex_count: parse_usize(baseline, "sigVertices")?,
        sig_edge_count: parse_usize(baseline, "sigEdges")?,
        next_inter_id_is_vip: parse_bool(baseline, "nextIdVipConfigured")?,
        beam_kind: match baseline.value("beamClass")? {
            "BeamInter" => BeamKind::Raw,
            "BeamHookInter" => BeamKind::Hook,
            value => return Err(format!("invalid independent beam class {value}")),
        },
        beam: LiveInter {
            object_identity: beam_object_identity,
            inter_id: Some(beam_inter_id),
            // Java `removeVertex` removes graph membership but does not clear
            // the Inter object's SIG pointer.
            sig_system_id: Some(system_id),
            in_sig: beam_vertex.is_some(),
            in_inter_index: beam_indexed,
            removed: parse_bool(baseline, "beamRemoved")?,
            abnormal: parse_bool(baseline, "beamAbnormal")?,
            vip: parse_bool(baseline, "beamVip")?,
        },
        stem: LiveInter {
            object_identity: stem_vertex + 1,
            inter_id: stem_inter_id,
            sig_system_id: parse_bool(baseline, "stemSigAttached")?.then_some(system_id),
            in_sig: existing_stem_vertex.is_some(),
            in_inter_index: stem_indexed,
            removed: parse_bool(baseline, "stemRemoved")?,
            abnormal: parse_bool(baseline, "stemAbnormal")?,
            vip: parse_bool(baseline, "stemVip")?,
        },
        beam_incidents: project_independent_incidents(
            rows,
            "Before",
            RowKind::BeamIncident,
            draft_alias,
            plan,
        )?,
        stem_incidents: project_independent_incidents(
            rows,
            "Before",
            RowKind::StemIncident,
            draft_alias,
            plan,
        )?,
        relation: FreshBeamStemDraft {
            relation_object_identity: independent_relation_object_identity(
                draft_alias,
                draft_alias,
                plan,
            )?,
            grade: parse_hex_double(frontier.value("grade")?, "draft grade")?,
            portion: independent_portion(frontier.value("portion")?)?,
            extension_present: frontier.value("extension")? != "-",
            outgoing: parse_bool(frontier, "outgoing")?,
            attached: false,
        },
        flags: ModificationFlags {
            sheet_stub_modified: parse_bool(baseline, "stubModified")?,
            book_modified: parse_bool(baseline, "bookModifiedRaw")?,
            book_dirty: parse_bool(baseline, "bookDirty")?,
        },
    })
}

fn independent_read_state(value: &str) -> Result<IndependentBeamReadState, String> {
    match value {
        "ExaminedClassOnly" => Ok(IndependentBeamReadState::ExaminedClassOnly),
        "ExaminedClassAndPortion" => Ok(IndependentBeamReadState::ExaminedClassAndPortion),
        "UnreadAfterBreak" => Ok(IndependentBeamReadState::UnreadAfterBreak),
        _ => Err(format!("invalid independent beam read state {value}")),
    }
}

fn independent_contribution(value: &str) -> Result<IndependentBeamContribution, String> {
    match value {
        "None" => Ok(IndependentBeamContribution::None),
        "Left" => Ok(IndependentBeamContribution::Left),
        "Right" => Ok(IndependentBeamContribution::Right),
        "FirstBeamStem" => Ok(IndependentBeamContribution::FirstBeamStem),
        _ => Err(format!("invalid independent beam contribution {value}")),
    }
}

fn assert_independent_beam_trace_matches_java(
    rows: &[StrictRow],
    phase: &str,
    trace: &IndependentBeamTrace,
) -> Result<(), String> {
    let frontier = one_row(rows, RowKind::Frontier)?;
    let callback = one_row(rows, RowKind::RelationCallback)?;
    let plan = parse_usize(frontier, "plan")?;
    let draft_alias = frontier.value("freshLinkAlias")?;
    let oracle_rows = rows_of_kind(rows, RowKind::BeamIncident)
        .into_iter()
        .filter(|row| row.value("phase") == Ok(phase))
        .collect::<Vec<_>>();
    if trace.rows.len() != oracle_rows.len() {
        return Err(format!("{phase} independent beam trace length differs"));
    }
    for (expected, row) in trace.rows.iter().zip(oracle_rows) {
        let direction = match row.value("direction")? {
            "Incoming" => EdgeDirection::Incoming,
            "Outgoing" => EdgeDirection::Outgoing,
            value => return Err(format!("invalid independent direction {value}")),
        };
        if expected.incident_ordinal != parse_usize(row, "incidentOrdinal")?
            || expected.direction_ordinal != parse_usize(row, "directionOrdinal")?
            || expected.graph_relation_identity
                != parse_sig_edge_alias(row.value("globalRelationIdentity")?)?
            || expected.relation_object_identity
                != independent_relation_object_identity(
                    row.value("relationObjectIdentity")?,
                    draft_alias,
                    plan,
                )?
            || expected.direction != direction
            || expected.read_state != independent_read_state(row.value("readState")?)?
            || expected.relevant != parse_bool(row, "relevant")?
            || expected.portion_read != independent_portion(row.value("portion")?)?
            || expected.contribution != independent_contribution(row.value("contribution")?)?
        {
            return Err(format!("{phase} independent beam trace row differs"));
        }
    }
    if phase == "AfterCallback" {
        let left = match callback.value("left")? {
            "-" => None,
            _ => Some(parse_bool(callback, "left")?),
        };
        let right = match callback.value("right")? {
            "-" => None,
            _ => Some(parse_bool(callback, "right")?),
        };
        if trace.left != left
            || trace.right != right
            || trace.requested_abnormal != parse_bool(callback, "requestedAbnormal")?
        {
            return Err("independent beam callback result differs".to_owned());
        }
    } else {
        let expected_state = callback.value("preBeamScanState")?;
        match expected_state {
            "NotRead" | "MissingVertex" if !trace.rows.is_empty() => {
                return Err(format!("{expected_state} pre-beam scan carries rows"));
            }
            "NotRead"
            | "MissingVertex"
            | "ExhaustiveIncomingThenOutgoing"
            | "LazyIncomingThenOutgoing" => {}
            value => return Err(format!("invalid pre-beam scan state {value}")),
        }
    }
    Ok(())
}

fn assert_independent_incidents_match_java(
    rows: &[StrictRow],
    phase: &str,
    endpoint: RowKind,
    actual: &[IncidentEdge],
) -> Result<(), String> {
    let frontier = one_row(rows, RowKind::Frontier)?;
    let expected = project_independent_incidents(
        rows,
        phase,
        endpoint,
        frontier.value("freshLinkAlias")?,
        parse_usize(frontier, "plan")?,
    )?;
    if actual != expected {
        let baseline = one_row(rows, RowKind::Baseline)?;
        return Err(format!(
            "{} {phase} {endpoint:?} independent incidence differs: actual={actual:?} expected={expected:?}",
            baseline.value("case")?,
        ));
    }
    Ok(())
}

fn assert_independent_matches_java(
    rows: &[StrictRow],
    result: &IndependentApplyResult,
    state: &IndependentApplyState,
) -> Result<(), String> {
    let vertex = one_row(rows, RowKind::VertexTrace)?;
    let decision = one_row(rows, RowKind::ApplyDecision)?;
    let edge = one_row(rows, RowKind::EdgeStruct)?;
    let callback = one_row(rows, RowKind::RelationCallback)?;
    let oracle_result = one_row(rows, RowKind::Result)?;
    let frontier = one_row(rows, RowKind::Frontier)?;
    let baseline = one_row(rows, RowKind::Baseline)?;
    let expected_disposition = match decision.value("action")? {
        "Add" => IndependentDisposition::Added,
        "SuppressSourceRemoved" => IndependentDisposition::SuppressedSourceRemoved,
        "SuppressTargetRemoved" => IndependentDisposition::SuppressedTargetRemoved,
        "SuppressExistingRelation" => IndependentDisposition::SuppressedExistingBeamStem {
            graph_relation_identity: parse_sig_edge_alias(decision.value("firstMatchIdentity")?)?,
        },
        value => return Err(format!("unsupported independent action {value}")),
    };
    let expected_vertex = match vertex.value("branch")? {
        "NewIdZero" => VertexAction::Registered {
            inter_id: parse_i32(vertex, "assignedId")?,
            sig_vertex_ordinal: parse_usize(vertex, "vertexOrdinal")?,
        },
        "ExistingPositive" => VertexAction::AlreadyLive {
            inter_id: parse_i32(vertex, "idRead")?,
        },
        value => return Err(format!("unsupported independent vertex branch {value}")),
    };
    let expected_graph = match edge.value("graphRelationIdentity")? {
        "-" => None,
        value => Some(parse_sig_edge_alias(value)?),
    };
    let expected_object = expected_graph
        .map(|_| {
            independent_relation_object_identity(
                edge.value("freshDraftIdentity")?,
                frontier.value("freshLinkAlias")?,
                parse_usize(frontier, "plan")?,
            )
        })
        .transpose()?;
    let target_removed_read = parse_bool(decision, "targetRemovedRead")?;
    let expected_target_removed = target_removed_read
        .then(|| parse_bool(decision, "targetRemoved"))
        .transpose()?;
    let duplicate_rows = rows_of_kind(rows, RowKind::DuplicateScan);
    if result.duplicate_scan.len() != duplicate_rows.len() {
        return Err("independent duplicate scan length differs".to_owned());
    }
    for (actual, row) in result.duplicate_scan.iter().zip(duplicate_rows) {
        let class_read = parse_bool(row, "classCheckRead")?;
        let matches = parse_bool(row, "matchesRuntimeClass")?;
        let expected_class = class_read.then_some(matches);
        if actual.pair_ordinal != parse_usize(row, "pairOrdinal")?
            || actual.source_outgoing_ordinal != parse_usize(row, "sourceOutgoingOrdinal")?
            || actual.graph_relation_identity
                != parse_sig_edge_alias(row.value("globalRelationIdentity")?)?
            || actual.relation_object_identity
                != independent_relation_object_identity(
                    row.value("relationObjectIdentity")?,
                    frontier.value("freshLinkAlias")?,
                    parse_usize(frontier, "plan")?,
                )?
            || actual.class_check != expected_class
        {
            return Err("independent duplicate scan row differs".to_owned());
        }
    }
    assert_independent_beam_trace_matches_java(rows, "Before", &result.pre_beam_trace)?;
    let retained_phase = if result.post_beam_trace.is_some() {
        "AfterCallback"
    } else {
        "Before"
    };
    if let Some(trace) = &result.post_beam_trace {
        assert_independent_beam_trace_matches_java(rows, "AfterCallback", trace)?;
    } else if callback.value("beamScanState")? != "NotRead" {
        return Err("suppressed independent result lacks a NotRead callback".to_owned());
    }
    assert_independent_incidents_match_java(
        rows,
        retained_phase,
        RowKind::StemIncident,
        &result.post_stem_incidents,
    )?;
    assert_independent_incidents_match_java(
        rows,
        retained_phase,
        RowKind::BeamIncident,
        &result.post_beam_incidents,
    )?;
    let expected_mutations = expected_independent_mutations(rows)?;
    if result.disposition != expected_disposition
        || result.vertex_action != expected_vertex
        || result.apply_returned != parse_bool(edge, "applyReturn")?
        || result.continuation_support_grade.to_bits()
            != parse_hex_double(oracle_result.value("retainedDraftGrade")?, "retained grade")?
                .to_bits()
        || result.attached_graph_relation_identity != expected_graph
        || result.attached_relation_object_identity != expected_object
        || result.source_removed_read != parse_bool(decision, "sourceRemovedRead")?
        || result.target_removed_read != expected_target_removed
        || result.source_outgoing_scanned != parse_usize(decision, "sourceOutgoingScanned")?
        || result.mutations != expected_mutations
        || state.persistent_ids.sheet_last_id != parse_i32(oracle_result, "allocator")?
        || state.persistent_ids.glyph_index_last_id != parse_i32(oracle_result, "allocator")?
        || state.persistent_ids.inter_index_last_id != parse_i32(oracle_result, "allocator")?
        || state.inter_index_count != parse_usize(vertex, "interIndexAfter")?
        || state.sig_vertex_count != parse_usize(vertex, "vertexCountAfter")?
        || state.sig_edge_count
            != parse_usize(baseline, "sigEdges")? + usize::from(expected_graph.is_some())
        || state.stem.inter_id != Some(parse_i32(oracle_result, "finalStemInterId")?)
        || state.stem.sig_system_id.is_some() != parse_bool(oracle_result, "finalStemSigAttached")?
        || state.stem.in_sig
            != (baseline.value("sigStemVertexOrdinal")? != "-"
                || vertex.value("branch")? == "NewIdZero")
        || state.stem.in_inter_index
            != (baseline.value("interIndexStemIndexOrdinal")? != "-"
                || vertex.value("branch")? == "NewIdZero")
        || state.stem.vip != parse_bool(oracle_result, "finalStemVip")?
        || state.stem.abnormal != parse_bool(oracle_result, "finalStemAbnormal")?
        || state.stem.removed != parse_bool(vertex, "stemRemovedAfter")?
        || state.beam.abnormal != parse_bool(oracle_result, "finalBeamAbnormal")?
        || state.beam.vip != parse_bool(oracle_result, "finalBeamVip")?
        || state.beam.removed != parse_bool(oracle_result, "finalBeamRemoved")?
        || state.beam.sig_system_id != Some(baseline.system_id()?)
        || state.beam.in_sig != (baseline.value("sigBeamVertexOrdinal")? != "-")
        || state.beam.in_inter_index != (baseline.value("interIndexBeamIndexOrdinal")? != "-")
        || state.relation.grade.to_bits()
            != parse_hex_double(oracle_result.value("retainedDraftGrade")?, "retained grade")?
                .to_bits()
        || state.relation.portion != independent_portion(frontier.value("portion")?)?
        || state.relation.extension_present != (frontier.value("extension")? != "-")
        || state.relation.outgoing != parse_bool(frontier, "outgoing")?
        || state.relation.attached != expected_graph.is_some()
        || state.flags.sheet_stub_modified != parse_bool(oracle_result, "stubModified")?
        || state.flags.book_modified != parse_bool(oracle_result, "bookModifiedRaw")?
        || state.flags.book_dirty != parse_bool(oracle_result, "bookDirty")?
    {
        return Err("independent replay differs from Java rows".to_owned());
    }
    Ok(())
}

fn expected_independent_mutations(rows: &[StrictRow]) -> Result<Vec<IndependentMutation>, String> {
    let vertex = one_row(rows, RowKind::VertexTrace)?;
    let edge = one_row(rows, RowKind::EdgeStruct)?;
    let callback = one_row(rows, RowKind::RelationCallback)?;
    let mut expected = Vec::new();
    if vertex.value("branch")? == "NewIdZero" {
        let inter_id = parse_i32(vertex, "assignedId")?;
        expected.extend([
            IndependentMutation::PersistentIdAllocated(inter_id),
            IndependentMutation::StemInterIdAssigned(inter_id),
            IndependentMutation::InterIndexMapped(inter_id),
        ]);
        if parse_bool(vertex, "stemVipAfter")? != parse_bool(vertex, "stemVipBefore")? {
            expected.push(IndependentMutation::StemVipSet);
        }
        expected.extend([
            IndependentMutation::SigVertexInserted(parse_usize(vertex, "vertexOrdinal")?),
            IndependentMutation::StemSigAttached,
        ]);
        if parse_bool(vertex, "stemRemovedBefore")? != parse_bool(vertex, "stemRemovedAfter")? {
            expected.push(IndependentMutation::StemRemovedCleared);
        }
        if parse_bool(vertex, "stemAbnormalBefore")? != parse_bool(vertex, "stemAbnormalAfter")? {
            expected.push(IndependentMutation::StemAbnormalSet);
            expected.push(IndependentMutation::ModificationFlagsSet);
        }
    }
    if edge.value("graphRelationIdentity")? != "-" {
        let graph_relation_identity = parse_sig_edge_alias(edge.value("graphRelationIdentity")?)?;
        let relation_object_identity = independent_relation_object_identity(
            edge.value("freshDraftIdentity")?,
            edge.value("freshDraftIdentity")?,
            parse_usize(edge, "plan")?,
        )?;
        expected.push(IndependentMutation::SigEdgeInserted {
            graph_relation_identity,
            relation_object_identity,
        });
        if edge.value("callbackOutcome")? != "NotRun" {
            expected.push(IndependentMutation::RelationCallbackDispatched);
            expected.push(IndependentMutation::BeamAbnormalEvaluated);
            if parse_bool(callback, "beamAbnormalBefore")?
                != parse_bool(callback, "beamAbnormalAfter")?
            {
                expected.push(IndependentMutation::BeamAbnormalSet);
                expected.push(IndependentMutation::ModificationFlagsSet);
            }
        }
    }
    Ok(expected)
}

fn assert_java_envelope_matches_rows(
    rows: &[StrictRow],
    prefix: &JavaEnvelopePrefix,
    state: &IndependentApplyState,
) -> Result<(), String> {
    let baseline = one_row(rows, RowKind::Baseline)?;
    let vertex = one_row(rows, RowKind::VertexTrace)?;
    let edge = one_row(rows, RowKind::EdgeStruct)?;
    let callback = one_row(rows, RowKind::RelationCallback)?;
    let result = one_row(rows, RowKind::Result)?;
    let summary = one_row(rows, RowKind::Summary)?;
    let expected_stage = match summary.value("throwStage")? {
        "VertexAdd" => JavaThrowStage::VertexAdd,
        "LinkApply" => JavaThrowStage::LinkApply,
        value => return Err(format!("invalid Java envelope throw stage {value}")),
    };
    let expected_callback = edge.value("callbackOutcome")? == "CompletedBeforeLaterListenerThrow";
    let edge_added = parse_bool(summary, "edgeAdded")?;
    let expected_graph = if edge_added {
        Some(parse_sig_edge_alias(edge.value("graphRelationIdentity")?)?)
    } else {
        None
    };
    let expected_mutations = expected_envelope_mutations(rows, expected_callback)?;
    let expected_relation_object = if edge_added {
        Some(independent_relation_object_identity(
            edge.value("freshDraftIdentity")?,
            edge.value("freshDraftIdentity")?,
            parse_usize(edge, "plan")?,
        )?)
    } else {
        None
    };
    let structural_edge_present =
        expected_graph
            .zip(expected_relation_object)
            .is_some_and(|(graph, object)| {
                state.stem_incidents.iter().any(|incident| {
                    incident.graph_relation_identity == graph
                        && incident.relation_object_identity == object
                        && incident.direction == EdgeDirection::Incoming
                        && incident.opposite_object_identity == state.beam.object_identity
                }) && state.beam_incidents.iter().any(|incident| {
                    incident.graph_relation_identity == graph
                        && incident.relation_object_identity == object
                        && incident.direction == EdgeDirection::Outgoing
                        && incident.opposite_object_identity == state.stem.object_identity
                })
            });
    if prefix.throw_stage != expected_stage
        || prefix.callback_completed != expected_callback
        || prefix.vertex_returned.is_some()
        || prefix.apply_returned.is_some()
        || prefix.mutations != expected_mutations
        || state.persistent_ids.sheet_last_id != parse_i32(result, "allocator")?
        || state.inter_index_count != parse_usize(vertex, "interIndexAfter")?
        || state.sig_vertex_count != parse_usize(vertex, "vertexCountAfter")?
        || state.sig_edge_count != parse_usize(baseline, "sigEdges")? + usize::from(edge_added)
        || state.stem.inter_id
            != match parse_i32(result, "finalStemInterId")? {
                0 => None,
                id => Some(id),
            }
        || state.stem.sig_system_id.is_some() != parse_bool(result, "finalStemSigAttached")?
        || state.stem.vip != parse_bool(result, "finalStemVip")?
        || state.stem.abnormal != parse_bool(result, "finalStemAbnormal")?
        || state.beam.abnormal != parse_bool(result, "finalBeamAbnormal")?
        || state.flags.sheet_stub_modified != parse_bool(result, "stubModified")?
        || state.flags.book_modified != parse_bool(result, "bookModifiedRaw")?
        || state.flags.book_dirty != parse_bool(result, "bookDirty")?
        || state.relation.attached != edge_added
        || structural_edge_present != edge_added
        || edge_added
            && expected_graph
                != Some(
                    state
                        .sig_edge_count
                        .checked_sub(1)
                        .ok_or_else(|| "edge prefix underflow".to_owned())?,
                )
        || parse_bool(callback, "beamCheckCalled")? != expected_callback
    {
        return Err(format!(
            "Java envelope {} prefix differs from independent replay",
            baseline.value("case")?
        ));
    }
    Ok(())
}

fn expected_envelope_mutations(
    rows: &[StrictRow],
    callback_completed: bool,
) -> Result<Vec<IndependentMutation>, String> {
    let vertex = one_row(rows, RowKind::VertexTrace)?;
    let edge = one_row(rows, RowKind::EdgeStruct)?;
    let callback = one_row(rows, RowKind::RelationCallback)?;
    let mut expected = Vec::new();
    if vertex.value("throwStage")? == "VertexAdd" {
        let inter_id = parse_i32(vertex, "assignedId")?;
        expected.extend([
            IndependentMutation::PersistentIdAllocated(inter_id),
            IndependentMutation::StemInterIdAssigned(inter_id),
            IndependentMutation::InterIndexMapped(inter_id),
        ]);
        if parse_bool(vertex, "stemVipAfter")? != parse_bool(vertex, "stemVipBefore")? {
            expected.push(IndependentMutation::StemVipSet);
        }
        expected.push(IndependentMutation::SigVertexInserted(parse_usize(
            vertex,
            "vertexOrdinal",
        )?));
    }
    if edge.value("graphRelationIdentity")? != "-" {
        expected.push(IndependentMutation::SigEdgeInserted {
            graph_relation_identity: parse_sig_edge_alias(edge.value("graphRelationIdentity")?)?,
            relation_object_identity: independent_relation_object_identity(
                edge.value("freshDraftIdentity")?,
                edge.value("freshDraftIdentity")?,
                parse_usize(edge, "plan")?,
            )?,
        });
        if callback_completed {
            expected.extend([
                IndependentMutation::RelationCallbackDispatched,
                IndependentMutation::BeamAbnormalEvaluated,
            ]);
            if parse_bool(callback, "beamAbnormalBefore")?
                != parse_bool(callback, "beamAbnormalAfter")?
            {
                expected.push(IndependentMutation::BeamAbnormalSet);
                expected.push(IndependentMutation::ModificationFlagsSet);
            }
        }
    }
    Ok(expected)
}

fn expected_production_operations(
    rows: &[StrictRow],
    stem_identity: usize,
) -> Result<Vec<NativeStemsBeamVLinkBaseApplyOperation>, String> {
    let vertex = one_row(rows, RowKind::VertexTrace)?;
    let edge = one_row(rows, RowKind::EdgeStruct)?;
    let callback = one_row(rows, RowKind::RelationCallback)?;
    let mut operations = Vec::new();
    if vertex.value("branch")? == "NewIdZero" {
        let inter_id = parse_i32(vertex, "assignedId")?;
        let sig_vertex_identity = parse_usize(vertex, "vertexOrdinal")?;
        operations.extend([
            NativeStemsBeamVLinkBaseApplyOperation::SharedPersistentIdAdvanced {
                before: parse_i32(vertex, "allocatorBefore")?,
                after: inter_id,
            },
            NativeStemsBeamVLinkBaseApplyOperation::StemInterIdAssigned {
                stem_identity,
                inter_id,
            },
            NativeStemsBeamVLinkBaseApplyOperation::InterIndexInserted {
                stem_identity,
                inter_id,
            },
            NativeStemsBeamVLinkBaseApplyOperation::SigVertexInserted {
                sig_vertex_identity,
            },
            NativeStemsBeamVLinkBaseApplyOperation::SigVertexEventDispatched,
            NativeStemsBeamVLinkBaseApplyOperation::StandardSigListenerVertexCallbackCompleted,
            NativeStemsBeamVLinkBaseApplyOperation::StemSigAttached {
                system_id: vertex.system_id()?,
            },
            NativeStemsBeamVLinkBaseApplyOperation::StemAddedCallbackStarted,
            NativeStemsBeamVLinkBaseApplyOperation::StemRemovedCleared {
                before: parse_bool(vertex, "stemRemovedBefore")?,
            },
            NativeStemsBeamVLinkBaseApplyOperation::StemAbnormalSet {
                before: parse_bool(vertex, "stemAbnormalBefore")?,
                after: parse_bool(vertex, "stemAbnormalAfter")?,
            },
        ]);
        if parse_bool(vertex, "stemAbnormalBefore")? != parse_bool(vertex, "stemAbnormalAfter")? {
            operations.extend([
                NativeStemsBeamVLinkBaseApplyOperation::SheetStubModifiedSetTrue,
                NativeStemsBeamVLinkBaseApplyOperation::BookModifiedSetTrue,
                NativeStemsBeamVLinkBaseApplyOperation::BookDirtySetTrue,
            ]);
        }
        operations.push(NativeStemsBeamVLinkBaseApplyOperation::StemAddedCallbackCompleted);
    } else if vertex.value("branch")? != "ExistingPositive" {
        return Err(format!(
            "unsupported operation branch {}",
            vertex.value("branch")?
        ));
    }
    if edge.value("graphRelationIdentity")? != "-" {
        let graph_relation_identity = parse_sig_edge_alias(edge.value("graphRelationIdentity")?)?;
        operations.extend([
            NativeStemsBeamVLinkBaseApplyOperation::SigGlobalRelationInserted {
                graph_relation_identity,
            },
            NativeStemsBeamVLinkBaseApplyOperation::BeamOutgoingRelationInserted {
                graph_relation_identity,
            },
            NativeStemsBeamVLinkBaseApplyOperation::StemIncomingRelationInserted {
                graph_relation_identity,
            },
            NativeStemsBeamVLinkBaseApplyOperation::SigEdgeEventDispatched {
                graph_relation_identity,
            },
            NativeStemsBeamVLinkBaseApplyOperation::StandardSigListenerEdgeCallbackStarted,
            NativeStemsBeamVLinkBaseApplyOperation::BeamStemRelationCallbackStarted,
            NativeStemsBeamVLinkBaseApplyOperation::StemChordIncidentScanCompleted {
                incident_relation_count: parse_usize(callback, "stemIncidentCount")?,
                chord_stem_matches: parse_usize(callback, "chordMatches")?,
            },
        ]);
        if parse_bool(callback, "beamAbnormalBefore")? != parse_bool(callback, "beamAbnormalAfter")?
        {
            operations.extend([
                NativeStemsBeamVLinkBaseApplyOperation::BeamAbnormalSet {
                    before: parse_bool(callback, "beamAbnormalBefore")?,
                    after: parse_bool(callback, "beamAbnormalAfter")?,
                },
                NativeStemsBeamVLinkBaseApplyOperation::SheetStubModifiedSetTrue,
                NativeStemsBeamVLinkBaseApplyOperation::BookModifiedSetTrue,
                NativeStemsBeamVLinkBaseApplyOperation::BookDirtySetTrue,
            ]);
        }
        operations.extend([
            NativeStemsBeamVLinkBaseApplyOperation::BeamStemRelationCallbackCompleted,
            NativeStemsBeamVLinkBaseApplyOperation::StandardSigListenerEdgeCallbackCompleted,
        ]);
    }
    Ok(operations)
}

fn independently_derive_production_state_after(
    rows: &[StrictRow],
    before: &NativeStemsBeamVLinkBaseApplyState,
    key: NativeStemsBeamVLinkBaseApplyKey,
) -> Result<NativeStemsBeamVLinkBaseApplyState, String> {
    let vertex = one_row(rows, RowKind::VertexTrace)?;
    let decision = one_row(rows, RowKind::ApplyDecision)?;
    let edge = one_row(rows, RowKind::EdgeStruct)?;
    let result = one_row(rows, RowKind::Result)?;
    let frontier = one_row(rows, RowKind::Frontier)?;
    let mut expected = before.clone();
    let stem_identity = expected.sig.stem.stem_identity;
    if vertex.value("branch")? == "NewIdZero" {
        let inter_id = parse_i32(vertex, "assignedId")?;
        let sig_vertex_identity = parse_usize(vertex, "vertexOrdinal")?;
        expected.transaction_state.glyph_index.persistent_ids = NativeStemsBeamPersistentIdState {
            sheet_last_id: inter_id,
            glyph_index_last_id: inter_id,
            inter_index_last_id: inter_id,
        };
        let mut known = expected
            .transaction_state
            .system_stems
            .known_stems
            .iter_mut()
            .filter(|known| known.stem_identity == stem_identity)
            .collect::<Vec<_>>();
        if known.len() != 1 {
            return Err("independent state derivation lacks one selected system stem".to_owned());
        }
        known[0].inter_id = Some(inter_id);
        known[0].sig_attached = true;
        known[0].abnormal = parse_bool(result, "finalStemAbnormal")?;
        expected
            .inter_index
            .appended_entries
            .push(NativeStemsBeamInterIndexAppend {
                index_ordinal: expected.inter_index.baseline_entry_count,
                stem_identity,
                inter_id,
                vip: parse_bool(result, "finalStemVip")?,
            });
        expected.inter_index.stem_lookup = NativeStemsBeamInterIndexLookup::PresentSameObject {
            index_ordinal: expected.inter_index.baseline_entry_count,
            inter_id,
            vip: parse_bool(result, "finalStemVip")?,
            object_matches: 1,
            inter_id_matches: 1,
            glyph_active_matches: 0,
            glyph_original_matches: 0,
        };
        expected.inter_index.next_id_lookup =
            NativeStemsBeamNextPersistentIdLookup::OccupiedByAppendedStem {
                persistent_id: inter_id,
                stem_identity,
            };
        expected
            .sig
            .appended_vertices
            .push(NativeStemsBeamSigVertexAppend {
                vertex_ordinal: expected.sig.baseline_vertex_count,
                sig_vertex_identity,
                stem_identity,
                inter_id,
            });
        expected.sig.stem_vertex = NativeStemsBeamSigVertexLookup::PresentSameObject {
            vertex_ordinal: expected.sig.baseline_vertex_count,
            sig_vertex_identity,
            inter_id,
            object_matches: 1,
        };
        expected.sig.stem.sig_vertex_identity = Some(sig_vertex_identity);
        expected.sig.stem.inter_indexed = true;
        expected.sig.stem.sig_system_id = Some(key.system_id);
        expected.sig.stem.removed = parse_bool(vertex, "stemRemovedAfter")?;
        expected.sig.stem.vip = parse_bool(result, "finalStemVip")?;
        expected.sig.stem.abnormal = parse_bool(result, "finalStemAbnormal")?;
    } else if vertex.value("branch")? != "ExistingPositive" {
        return Err(format!(
            "unsupported independent state branch {}",
            vertex.value("branch")?
        ));
    }
    if edge.value("graphRelationIdentity")? != "-" {
        let graph_relation_identity = parse_sig_edge_alias(edge.value("graphRelationIdentity")?)?;
        let stem_vertex_identity = expected
            .sig
            .stem
            .sig_vertex_identity
            .ok_or_else(|| "added edge lacks derived stem vertex".to_owned())?;
        expected
            .sig
            .appended_relations
            .push(NativeStemsBeamSigRelationState {
                graph_relation_identity,
                relation_object_identity: before
                    .certificate
                    .as_ref()
                    .ok_or_else(|| "prestate lacks certificate".to_owned())?
                    .fresh_relation_object_identity,
                source_vertex_identity: expected
                    .sig
                    .beam
                    .sig_vertex_identity
                    .ok_or_else(|| "added edge lacks a live beam vertex".to_owned())?,
                target_vertex_identity: stem_vertex_identity,
                kind: NativeStemsBeamSigRelationKind::BeamStem {
                    beam_portion: Some(parse_native_portion(frontier.value("portion")?)?),
                },
            });
    }
    expected.sig.beam.abnormal = parse_bool(result, "finalBeamAbnormal")?;
    expected.sheet_edit = NativeStemsBeamSheetEditState {
        stub_modified: parse_bool(result, "stubModified")?,
        book_modified: parse_bool(result, "bookModifiedRaw")?,
        book_dirty: parse_bool(result, "bookDirty")?,
    };
    expected.certificate = None;
    expected.committed = Some(key);
    expected.committed_apply = Some(NativeStemsBeamVLinkBaseApplyCommit {
        disposition: match decision.value("action")? {
            "Add" => NativeStemsBeamVLinkBaseApplyDisposition::Added {
                graph_relation_identity: parse_sig_edge_alias(
                    edge.value("graphRelationIdentity")?,
                )?,
            },
            "SuppressSourceRemoved" => {
                NativeStemsBeamVLinkBaseApplyDisposition::SuppressedSourceRemoved
            }
            "SuppressTargetRemoved" => {
                NativeStemsBeamVLinkBaseApplyDisposition::SuppressedTargetRemoved
            }
            "SuppressExistingRelation" => {
                NativeStemsBeamVLinkBaseApplyDisposition::SuppressedExistingBeamStem {
                    graph_relation_identity: parse_sig_edge_alias(
                        decision.value("firstMatchIdentity")?,
                    )?,
                }
            }
            value => return Err(format!("unsupported production action {value}")),
        },
        beam_portion: parse_native_portion(frontier.value("portion")?)?,
    });
    Ok(expected)
}

fn assert_production_matches_java(
    rows: &[StrictRow],
    transaction: &audiveris_omr::native_stems_beam_vlink_base_apply::NativeStemsBeamVLinkBaseApplyTransaction,
    expected_key: NativeStemsBeamVLinkBaseApplyKey,
    expected_relation: &audiveris_omr::native_stems_beam_vlink_reuse_check::NativeStemsBeamRelationDraft,
    state_before: &NativeStemsBeamVLinkBaseApplyState,
    state: &NativeStemsBeamVLinkBaseApplyState,
) -> Result<(), String> {
    let baseline = one_row(rows, RowKind::Baseline)?;
    let frontier = one_row(rows, RowKind::Frontier)?;
    let vertex = one_row(rows, RowKind::VertexTrace)?;
    let decision = one_row(rows, RowKind::ApplyDecision)?;
    let edge = one_row(rows, RowKind::EdgeStruct)?;
    let callback = one_row(rows, RowKind::RelationCallback)?;
    let result = one_row(rows, RowKind::Result)?;
    let expected_vertex = match vertex.value("branch")? {
        "NewIdZero" => NativeStemsBeamVLinkVertexAction::RegisteredAndAdded {
            inter_id: parse_i32(vertex, "assignedId")?,
            sig_vertex_identity: parse_usize(vertex, "vertexOrdinal")?,
        },
        "ExistingPositive" => NativeStemsBeamVLinkVertexAction::SkippedPositiveInterId,
        value => return Err(format!("unsupported production vertex branch {value}")),
    };
    let expected_disposition = match decision.value("action")? {
        "Add" => NativeStemsBeamVLinkBaseApplyDisposition::Added {
            graph_relation_identity: parse_sig_edge_alias(edge.value("graphRelationIdentity")?)?,
        },
        "SuppressSourceRemoved" => {
            NativeStemsBeamVLinkBaseApplyDisposition::SuppressedSourceRemoved
        }
        "SuppressTargetRemoved" => {
            NativeStemsBeamVLinkBaseApplyDisposition::SuppressedTargetRemoved
        }
        "SuppressExistingRelation" => {
            NativeStemsBeamVLinkBaseApplyDisposition::SuppressedExistingBeamStem {
                graph_relation_identity: parse_sig_edge_alias(
                    decision.value("firstMatchIdentity")?,
                )?,
            }
        }
        value => return Err(format!("unsupported production action {value}")),
    };
    let expected_graph = match edge.value("graphRelationIdentity")? {
        "-" => None,
        value => Some(parse_sig_edge_alias(value)?),
    };
    let expected_grade = parse_hex_double(result.value("retainedDraftGrade")?, "retained grade")?;
    let expected_outcome = NativeStemsBeamVLinkBaseApplyOutcome::ReadyBeforeBLinkerFlagMutation {
        apply_returned: parse_bool(edge, "applyReturn")?,
        continuation_support_grade: expected_grade,
    };
    let initial_stem = state_before
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|known| known.stem_identity == state_before.sig.stem.stem_identity)
        .ok_or_else(|| "prestate lacks selected system stem".to_owned())?;
    let expected_state =
        independently_derive_production_state_after(rows, state_before, expected_key)?;
    let final_stem = expected_state
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|known| known.stem_identity == state_before.sig.stem.stem_identity)
        .ok_or_else(|| "expected state lacks selected system stem".to_owned())?;
    let expected_relation_object = state_before
        .certificate
        .as_ref()
        .ok_or_else(|| "prestate lacks compact certificate".to_owned())?
        .fresh_relation_object_identity;
    let directed_pair_ids = rows_of_kind(rows, RowKind::DuplicateScan)
        .into_iter()
        .map(|row| parse_sig_edge_alias(row.value("globalRelationIdentity")?))
        .collect::<Result<Vec<_>, String>>()?;
    let stem_incident_ids = if expected_graph.is_some() {
        rows_of_kind(rows, RowKind::StemIncident)
            .into_iter()
            .filter(|row| row.value("phase") == Ok("AfterCallback"))
            .map(|row| parse_sig_edge_alias(row.value("globalRelationIdentity")?))
            .collect::<Result<Vec<_>, String>>()?
    } else {
        Vec::new()
    };
    let target_removed = if parse_bool(decision, "targetRemovedRead")? {
        Some(parse_bool(decision, "targetRemoved")?)
    } else {
        None
    };
    let expected_beam_trace = if expected_graph.is_none() {
        NativeStemsBeamVLinkBeamAbnormalTrace::NotReadSuppressed
    } else if baseline.value("beamClass")? == "BeamHookInter" {
        NativeStemsBeamVLinkBeamAbnormalTrace::HookAnyBeamStem {
            incident_relation_count: parse_usize(callback, "beamIncidentCount")?,
            relations_read: rows_of_kind(rows, RowKind::BeamIncident)
                .into_iter()
                .filter(|row| row.value("phase") == Ok("AfterCallback"))
                .filter(|row| row.value("readState") != Ok("UnreadAfterBreak"))
                .count(),
            before: parse_bool(callback, "beamAbnormalBefore")?,
            after: parse_bool(callback, "beamAbnormalAfter")?,
        }
    } else {
        NativeStemsBeamVLinkBeamAbnormalTrace::RawBeamSides {
            incident_relation_count: parse_usize(callback, "beamIncidentCount")?,
            left_found: parse_bool(callback, "left")?,
            right_found: parse_bool(callback, "right")?,
            before: parse_bool(callback, "beamAbnormalBefore")?,
            after: parse_bool(callback, "beamAbnormalAfter")?,
        }
    };
    let new_vertex = vertex.value("branch")? == "NewIdZero";
    let edge_added = expected_graph.is_some();
    if transaction.key != expected_key
        || transaction.key.system_id != baseline.system_id()?
        || transaction.key.plan.plan_ordinal != parse_usize(frontier, "plan")?
        || transaction.stem_before != *initial_stem
        || transaction.stem_after != *final_stem
        || transaction.fresh_relation_object_identity != expected_relation_object
        || transaction.fresh_relation != *expected_relation
        || transaction.fresh_relation.grade.to_bits() != expected_grade.to_bits()
        || transaction.continuation_support_grade.to_bits() != expected_grade.to_bits()
        || transaction.vertex_action != expected_vertex
        || transaction.apply_disposition != expected_disposition
        || transaction.apply_returned != parse_bool(edge, "applyReturn")?
        || transaction.graph_relation_identity != expected_graph
        || transaction.removed_reads.source_removed != parse_bool(decision, "sourceRemoved")?
        || transaction.removed_reads.target_removed != target_removed
        || transaction.removed_reads.directed_pair_relations_read
            != parse_usize(decision, "classChecksRead")?
        || transaction.directed_pair_graph_relation_identities != directed_pair_ids
        || transaction.stem_after.inter_id != Some(parse_i32(result, "finalStemInterId")?)
        || transaction.stem_after.sig_attached != parse_bool(result, "finalStemSigAttached")?
        || transaction.stem_after.abnormal != parse_bool(result, "finalStemAbnormal")?
        || transaction.sheet_edit_after.stub_modified != parse_bool(result, "stubModified")?
        || transaction.sheet_edit_after.book_modified != parse_bool(result, "bookModifiedRaw")?
        || transaction.sheet_edit_after.book_dirty != parse_bool(result, "bookDirty")?
        || transaction.callback.called != parse_bool(callback, "beamCheckCalled")?
        || transaction.callback.extension_preserved != edge_added
        || transaction.callback.beam_portion_preserved != edge_added
        || transaction.callback.stem_incident_graph_relation_identities != stem_incident_ids
        || transaction.callback.chord_stem_matches != parse_usize(callback, "chordMatches")?
        || transaction.callback.chord_cache_invalidation_count
            != parse_usize(callback, "chordInvalidations")?
        || transaction.callback.beam_abnormal != expected_beam_trace
        || transaction.consumed_certificate
            != *state_before
                .certificate
                .as_ref()
                .ok_or_else(|| "prestate lacks consumed certificate".to_owned())?
        || transaction.operations
            != expected_production_operations(rows, transaction.stem_before.stem_identity)?
        || transaction.sheet_edit_before != state_before.sheet_edit
        || transaction.sheet_edit_before.stub_modified != parse_bool(baseline, "stubModified")?
        || transaction.sheet_edit_before.book_modified != parse_bool(baseline, "bookModifiedRaw")?
        || transaction.sheet_edit_before.book_dirty != parse_bool(baseline, "bookDirty")?
        || transaction.persistent_id_mutation_count != usize::from(new_vertex)
        || transaction.inter_index_mutation_count != usize::from(new_vertex)
        || transaction.sig_vertex_mutation_count != usize::from(new_vertex)
        || transaction.sig_relation_mutation_count != usize::from(edge_added)
        || transaction.stem_abnormal_mutation_count
            != usize::from(
                new_vertex
                    && parse_bool(vertex, "stemAbnormalBefore")?
                        != parse_bool(vertex, "stemAbnormalAfter")?,
            )
        || transaction.beam_abnormal_mutation_count
            != usize::from(
                edge_added
                    && parse_bool(callback, "beamAbnormalBefore")?
                        != parse_bool(callback, "beamAbnormalAfter")?,
            )
        || transaction.outcome != expected_outcome
        || transaction.state_after.as_ref() != state
        || state != &expected_state
        || state.certificate.is_some()
        || state.committed != Some(transaction.key)
        || state
            .transaction_state
            .glyph_index
            .persistent_ids
            .sheet_last_id
            != parse_i32(result, "allocator")?
        || state.sig.beam.abnormal != parse_bool(result, "finalBeamAbnormal")?
        || state.sig.beam.vip != parse_bool(result, "finalBeamVip")?
        || state.sig.beam.removed != parse_bool(result, "finalBeamRemoved")?
        || state.sig.beam.beam_group != state_before.sig.beam.beam_group
        || transaction.beam_group_mutation_count != 0
        || transaction.linker_flag_mutation_count != 0
        || transaction.sibling_link_mutation_count != 0
        || transaction.head_link_mutation_count != 0
    {
        return Err("production boundary-14 transaction differs from Java rows".to_owned());
    }
    Ok(())
}

fn project_real_base_system(
    page_key: &str,
    page: &NativePredecessorPage,
    oracle_page: &StrictRow,
    rows: &[StrictRow],
    create_fixture: &PredecessorFixture,
    reuse_fixture: &PredecessorFixture,
) -> Result<(), String> {
    let baseline = one_row(rows, RowKind::Baseline)?;
    let system_id = baseline.system_id()?;
    validate_boundary_thirteen_continuity(rows, reuse_fixture)?;
    let predecessor = replay_boundary_thirteen(page, create_fixture, reuse_fixture, system_id)?;
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

    let mut independent_state = build_independent_real_state(rows)?;
    let independent = apply_independent_base_transaction(&mut independent_state)
        .map_err(|error| format!("system {system_id} independent base apply: {error:?}"))?;
    assert_independent_matches_java(rows, &independent, &independent_state)?;

    let mut production_state = build_real_base_state(page, oracle_page, rows, &predecessor)?;
    let expected_key = NativeStemsBeamVLinkBaseApplyKey {
        system_id: predecessor.reuse_check.system_id,
        invocation_ordinal: predecessor.reuse_check.invocation_ordinal,
        plan: predecessor.reuse_check.plan,
    };
    let NativeStemsBeamVLinkReuseCheckOutcome::ReadyBeforeSigMutation {
        relation: expected_relation,
    } = &predecessor.reuse_check.outcome
    else {
        return Err(format!("system {system_id} predecessor is not ready"));
    };
    let use_native_sig = page_key == "chula" && system_id == 1;
    let mut native_commit = None;
    if use_native_sig {
        let native_sig = page
            .sig
            .as_ref()
            .ok_or_else(|| format!("missing native SIG recognition for {page_key}"))?
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .ok_or_else(|| format!("missing native SIG system {system_id}"))?
            .clone();
        let native_bindings = page
            .sig
            .as_ref()
            .ok_or_else(|| format!("missing native SIG recognition for {page_key}"))?
            .bindings
            .iter()
            .find(|bindings| bindings.system_id == system_id)
            .ok_or_else(|| format!("missing native SIG bindings {system_id}"))?
            .clone();
        let stem_identity = predecessor
            .reuse_check
            .final_stem
            .as_ref()
            .ok_or_else(|| format!("system {system_id} predecessor lacks final stem"))?
            .stem_identity;
        let stem_vertex = native_bindings.stem_vertices.get(&stem_identity).copied();
        production_state.certificate = Some(
            project_native_stems_beam_vlink_base_apply_certificate(
                &native_sig,
                &native_bindings,
                expected_relation.beam,
                stem_vertex,
                expected_relation,
                predecessor.reuse_check.plan,
            )
            .map_err(|error| format!("system {system_id} native query projection: {error}"))?,
        );
        let vertex_count = native_sig.vertices.len();
        let edge_count = native_sig.edges.len();
        native_commit = Some((native_sig, native_bindings, vertex_count, edge_count));
    }
    let production_state_before = production_state.clone();
    let transaction = if let Some((native_sig, native_bindings, _, _)) = &mut native_commit {
        apply_native_stems_beam_vlink_base_transaction_to_native_sig(
            scheduler,
            plans,
            stumps,
            vlinkers,
            &predecessor.create_transaction,
            &predecessor.live_state,
            predecessor.relation_parameters,
            &predecessor.reuse_check,
            &mut production_state,
            native_sig,
            native_bindings,
        )
    } else {
        apply_native_stems_beam_vlink_base_transaction(
            scheduler,
            plans,
            stumps,
            vlinkers,
            &predecessor.create_transaction,
            &predecessor.live_state,
            predecessor.relation_parameters,
            &predecessor.reuse_check,
            &mut production_state,
        )
    }
    .map_err(|error| format!("system {system_id} production base apply: {error}"))?;
    assert_production_matches_java(
        rows,
        &transaction,
        expected_key,
        expected_relation,
        &production_state_before,
        &production_state,
    )?;
    if let Some((native_sig, native_bindings, vertex_count, edge_count)) = native_commit {
        let committed_edge = transaction
            .graph_relation_identity
            .and_then(|ordinal| native_sig.edges.get(ordinal));
        if native_sig.vertices.len() != vertex_count + transaction.sig_vertex_mutation_count
            || native_sig.edges.len() != edge_count + transaction.sig_relation_mutation_count
            || transaction.consumed_certificate.endpoint_identity
                != NativeStemsBeamCertificateEndpointIdentity::NativeVertexOneBased
            || native_bindings
                .stem_vertices
                .get(&transaction.stem_after.stem_identity)
                .is_none_or(|vertex| {
                    native_sig.vertex(vertex.0).is_none_or(|stem| {
                        stem.kind != audiveris_omr::native_sig::NativeSigInterKind::Stem
                    })
                })
            || committed_edge.is_none_or(|edge| {
                edge.kind != audiveris_omr::native_sig::NativeSigRelationKind::BeamStem
                    || edge.origin
                        != audiveris_omr::native_sig::NativeSigRelationOrigin::BeamVBaseDraft {
                            plan_ordinal: predecessor.reuse_check.plan.plan_ordinal,
                        }
                    || edge.beam_portion != Some(expected_relation.beam_portion)
                    || edge.support.is_none_or(|support| {
                        support.grade.to_bits() != expected_relation.grade.to_bits()
                    })
            })
        {
            return Err(format!(
                "system {system_id} native SIG commit differs from B14 transaction"
            ));
        }
    }

    let production_disposition = match transaction.apply_disposition {
        NativeStemsBeamVLinkBaseApplyDisposition::Added { .. } => IndependentDisposition::Added,
        NativeStemsBeamVLinkBaseApplyDisposition::SuppressedSourceRemoved => {
            IndependentDisposition::SuppressedSourceRemoved
        }
        NativeStemsBeamVLinkBaseApplyDisposition::SuppressedTargetRemoved => {
            IndependentDisposition::SuppressedTargetRemoved
        }
        NativeStemsBeamVLinkBaseApplyDisposition::SuppressedExistingBeamStem {
            graph_relation_identity,
        } => IndependentDisposition::SuppressedExistingBeamStem {
            graph_relation_identity,
        },
    };
    let production_vertex = match transaction.vertex_action {
        NativeStemsBeamVLinkVertexAction::RegisteredAndAdded {
            inter_id,
            sig_vertex_identity,
        } => VertexAction::Registered {
            inter_id,
            sig_vertex_ordinal: sig_vertex_identity,
        },
        NativeStemsBeamVLinkVertexAction::SkippedPositiveInterId => VertexAction::AlreadyLive {
            inter_id: transaction
                .stem_after
                .inter_id
                .ok_or_else(|| "skipped vertex lacks positive Inter ID".to_owned())?,
        },
    };
    if production_disposition != independent.disposition
        || production_vertex != independent.vertex_action
        || transaction.apply_returned != independent.apply_returned
        || transaction.continuation_support_grade.to_bits()
            != independent.continuation_support_grade.to_bits()
        || transaction.graph_relation_identity != independent.attached_graph_relation_identity
        || transaction.stem_after.inter_id != independent_state.stem.inter_id
        || transaction.stem_after.sig_attached != independent_state.stem.in_sig
        || transaction.stem_after.abnormal != independent_state.stem.abnormal
        || production_state.sig.beam.abnormal != independent_state.beam.abnormal
        || production_state.sheet_edit.stub_modified != independent_state.flags.sheet_stub_modified
        || production_state.sheet_edit.book_modified != independent_state.flags.book_modified
        || production_state.sheet_edit.book_dirty != independent_state.flags.book_dirty
        || production_state.sig.appended_relations.len()
            != usize::from(independent.attached_graph_relation_identity.is_some())
    {
        return Err(format!(
            "system {system_id} production/independent base-apply state differs"
        ));
    }
    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ActiveJGraphTCoreArtifact {
    Absent { artifact_root: Option<PathBuf> },
    Present(PathBuf),
}

fn active_jgrapht_core_artifact() -> Result<ActiveJGraphTCoreArtifact, String> {
    let gradle_root = std::env::var_os("GRADLE_USER_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".gradle")));
    let Some(gradle_root) = gradle_root else {
        return Ok(ActiveJGraphTCoreArtifact::Absent {
            artifact_root: None,
        });
    };
    locate_jgrapht_core_artifact(&gradle_root)
}

fn locate_jgrapht_core_artifact(gradle_root: &Path) -> Result<ActiveJGraphTCoreArtifact, String> {
    let artifact_root = gradle_root
        .join("caches/modules-2/files-2.1/org.jgrapht/jgrapht-core")
        .join(JGRAPHT_CORE_VERSION);
    let entries = match std::fs::read_dir(&artifact_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match std::fs::symlink_metadata(&artifact_root) {
                Ok(_) => {
                    return Err(format!(
                        "JGraphT cache {} exists but cannot be followed",
                        artifact_root.display()
                    ));
                }
                Err(metadata_error) if metadata_error.kind() == std::io::ErrorKind::NotFound => {}
                Err(metadata_error) => {
                    return Err(format!(
                        "cannot inspect JGraphT cache {}: {metadata_error}",
                        artifact_root.display()
                    ));
                }
            }
            return Ok(ActiveJGraphTCoreArtifact::Absent {
                artifact_root: Some(artifact_root),
            });
        }
        Err(error) => {
            return Err(format!(
                "cannot read JGraphT cache {}: {error}",
                artifact_root.display()
            ));
        }
    };
    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "cannot enumerate JGraphT cache {}: {error}",
                artifact_root.display()
            )
        })?;
        let candidate = entry.path().join(JGRAPHT_CORE_JAR);
        match std::fs::metadata(&candidate) {
            Ok(metadata) if metadata.is_file() => candidates.push(candidate),
            Ok(_) => {
                return Err(format!(
                    "JGraphT artifact candidate {} is not a file",
                    candidate.display()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match std::fs::symlink_metadata(&candidate) {
                    Ok(_) => {
                        return Err(format!(
                            "JGraphT artifact candidate {} exists but cannot be followed",
                            candidate.display()
                        ));
                    }
                    Err(metadata_error)
                        if metadata_error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(metadata_error) => {
                        return Err(format!(
                            "cannot inspect JGraphT artifact {}: {metadata_error}",
                            candidate.display()
                        ));
                    }
                }
            }
            Err(error) => {
                return Err(format!(
                    "cannot inspect JGraphT artifact {}: {error}",
                    candidate.display()
                ));
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    match candidates.as_slice() {
        [] => Ok(ActiveJGraphTCoreArtifact::Absent {
            artifact_root: Some(artifact_root),
        }),
        [path] => Ok(ActiveJGraphTCoreArtifact::Present(path.clone())),
        _ => Err(format!(
            "expected one active {JGRAPHT_CORE_JAR}, found {}",
            candidates.len()
        )),
    }
}

fn validate_jgrapht_gradle_declaration() -> Result<(), String> {
    let path = repo_root().join("app/build.gradle");
    let source = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read provenance path {}: {error}", path.display()))?;
    let declarations = source
        .lines()
        .filter(|line| line.trim() == JGRAPHT_CORE_GRADLE_DECLARATION)
        .count();
    if declarations != 1 {
        return Err(format!(
            "expected one exact {JGRAPHT_CORE_GRADLE_DECLARATION} declaration, found {declarations}"
        ));
    }
    Ok(())
}

fn validate_active_jgrapht_core_artifact(
    artifact: &ActiveJGraphTCoreArtifact,
    expected_sha256: &str,
) -> Result<(), String> {
    match artifact {
        ActiveJGraphTCoreArtifact::Absent { .. } => Ok(()),
        ActiveJGraphTCoreArtifact::Present(path) => validate_provenance_pin(path, expected_sha256),
    }
}

fn validate_jgrapht_core_provenance(
    expected_sha256: &str,
) -> Result<ActiveJGraphTCoreArtifact, String> {
    if expected_sha256 != JGRAPHT_CORE_JAR_SHA256 {
        return Err("JGraphT core fixture/manifest SHA differs from the audited pin".to_owned());
    }
    validate_jgrapht_gradle_declaration()?;
    let artifact = active_jgrapht_core_artifact()?;
    validate_active_jgrapht_core_artifact(&artifact, expected_sha256)?;
    Ok(artifact)
}

fn actual_file_sha256(relative: &str) -> Result<String, String> {
    let path = repo_root().join(relative);
    let bytes = std::fs::read(&path)
        .map_err(|error| format!("cannot read provenance path {}: {error}", path.display()))?;
    Ok(sha256_hex(&bytes))
}

fn validate_provenance_pin(path: &Path, expected: &str) -> Result<(), String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("cannot read provenance path {}: {error}", path.display()))?;
    let actual = sha256_hex(&bytes);
    if actual != expected {
        return Err(format!("{} provenance SHA differs", path.display()));
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

fn base_state() -> IndependentApplyState {
    IndependentApplyState {
        system_id: 1,
        persistent_ids: PersistentIds {
            sheet_last_id: 40,
            glyph_index_last_id: 40,
            inter_index_last_id: 40,
        },
        inter_index_count: 2,
        inter_index_slots: BTreeMap::from([(11, 100)]),
        sig_vertex_count: 2,
        sig_edge_count: 7,
        next_inter_id_is_vip: false,
        beam_kind: BeamKind::Raw,
        beam: LiveInter {
            object_identity: 100,
            inter_id: Some(11),
            sig_system_id: Some(1),
            in_sig: true,
            in_inter_index: true,
            removed: false,
            abnormal: true,
            vip: false,
        },
        stem: LiveInter {
            object_identity: 200,
            inter_id: None,
            sig_system_id: None,
            in_sig: false,
            in_inter_index: false,
            removed: false,
            abnormal: false,
            vip: false,
        },
        beam_incidents: vec![IncidentEdge {
            graph_relation_identity: 3,
            relation_object_identity: 30,
            direction: EdgeDirection::Outgoing,
            opposite_object_identity: 300,
            kind: RelationKind::BeamRest {
                portion: Some(BeamPortion::Right),
            },
        }],
        stem_incidents: Vec::new(),
        relation: FreshBeamStemDraft {
            relation_object_identity: 70,
            grade: 0.75,
            portion: Some(BeamPortion::Left),
            extension_present: true,
            outgoing: true,
            attached: false,
        },
        flags: ModificationFlags {
            sheet_stub_modified: false,
            book_modified: false,
            book_dirty: false,
        },
    }
}

fn scaffold_row(kind: RowKind, system: Option<usize>) -> String {
    scaffold_transaction_row(kind, system, "real", "-")
}

fn scaffold_transaction_row(
    kind: RowKind,
    system: Option<usize>,
    scope: &str,
    case: &str,
) -> String {
    let fields = kind
        .expected_fields_for_scope(Some(scope))
        .expect("scaffold uses only frozen core row families");
    let mut row = if kind == RowKind::CorpusSummary {
        format!("{BASE_APPLY_ROW_PREFIX}{}", kind.suffix())
    } else {
        format!("{BASE_APPLY_ROW_PREFIX}{} chula.png#1", kind.suffix())
    };
    for field in fields {
        let value = match *field {
            "system" => system.expect("system row").to_string(),
            "plan" => "7".to_owned(),
            "scope" => scope.to_owned(),
            "case" => case.to_owned(),
            "pageRefs" => "chula.png#1".to_owned(),
            _ => "x".to_owned(),
        };
        row.push(' ');
        row.push_str(field);
        row.push(' ');
        row.push_str(&value);
    }
    row.push('\n');
    row
}

fn scaffold_manifest_row(label: &str, fields: &[&str], updates: &[(&str, String)]) -> String {
    let mut row = label.to_owned();
    for field in fields {
        let value = updates
            .iter()
            .find_map(|(name, value)| (*name == *field).then_some(value.as_str()))
            .unwrap_or("x");
        row.push(' ');
        row.push_str(field);
        row.push(' ');
        row.push_str(value);
    }
    row.push('\n');
    row
}

fn semantic_scaffold_row(kind: RowKind, updates: &[(&str, &str)]) -> StrictRow {
    let line = scaffold_transaction_row(kind, Some(1), "synthetic", "ExistingFullSuccess");
    let mut row = StrictRow::parse(line.trim_end()).expect("semantic scaffold row parses");
    for (name, value) in updates {
        let slot = row
            .fields
            .iter_mut()
            .find_map(|(field, value)| (field == name).then_some(value))
            .unwrap_or_else(|| panic!("semantic scaffold lacks {name}"));
        *slot = (*value).to_owned();
    }
    row
}

#[test]
fn strict_scaffold_parser_pins_family_order_and_fail_closed_syntax() {
    let ordered = [
        RowKind::Baseline,
        RowKind::PredecessorCompat,
        RowKind::Frontier,
        RowKind::Listeners,
        RowKind::VertexTrace,
        RowKind::ApplyDecision,
        RowKind::DuplicateScan,
        RowKind::EdgeStruct,
        RowKind::StemIncident,
        RowKind::RelationCallback,
        RowKind::BeamIncident,
        RowKind::Result,
        RowKind::DeltaGuard,
        RowKind::Summary,
    ];
    let mut fixture = format!("# Java boundary 14 scaffold\n{BASE_APPLY_SCHEMA}\n");
    fixture.push_str(&scaffold_row(RowKind::Page, None));
    for kind in ordered {
        fixture.push_str(&scaffold_row(kind, Some(1)));
    }
    for (scope, case) in SYSTEM_ONE_SUPPLEMENTAL_CASES {
        for kind in ordered {
            if kind == RowKind::PredecessorCompat {
                continue;
            }
            fixture.push_str(&scaffold_transaction_row(kind, Some(1), scope, case));
        }
    }
    fixture.push_str(&scaffold_row(RowKind::PageSummary, None));
    fixture.push_str(&scaffold_row(RowKind::CorpusSummary, None));
    let parsed = StrictFixture::parse(fixture.as_bytes()).expect("strict scaffold fixture parses");
    assert_eq!(parsed.systems.len(), 1);

    let duplicate_field = fixture.replace("system 1 plan 7", "system 1 system 1 plan 7");
    assert!(StrictFixture::parse(duplicate_field.as_bytes()).is_err());
    let result_row = scaffold_row(RowKind::Result, Some(1));
    let regressed = fixture.replace(
        &result_row,
        &format!(
            "{result_row}{}",
            scaffold_row(RowKind::BeamIncident, Some(1))
        ),
    );
    assert!(StrictFixture::parse(regressed.as_bytes()).is_err());
    let trailing = format!("{fixture}# unauthenticated\n");
    assert!(StrictFixture::parse(trailing.as_bytes()).is_err());
    let trailing_blank = format!("{fixture}\n");
    assert!(StrictFixture::parse(trailing_blank.as_bytes()).is_err());

    let out_of_order = fixture.replace("system 1 plan 7", "system 2 plan 7");
    assert!(StrictFixture::parse(out_of_order.as_bytes()).is_err());
    let case_reordered =
        fixture.replacen("case ExistingFullSuccess", "case ExistingHookSuccess", 1);
    assert!(StrictFixture::parse(case_reordered.as_bytes()).is_err());
}

#[test]
fn strict_manifest_scaffold_pins_page_order_and_terminal_summary() {
    let mut manifest = format!("# boundary 14 manifest scaffold\n{BASE_APPLY_MANIFEST_SCHEMA}\n");
    for (ordinal, page) in CORPUS_PAGES.iter().enumerate() {
        manifest.push_str(&scaffold_manifest_row(
            BASE_APPLY_MANIFEST_ENTRY,
            MANIFEST_ENTRY_FIELDS,
            &[
                ("ordinal", ordinal.to_string()),
                ("page", (*page).to_owned()),
                ("fixture", format!("stems-beam-vlink-base-apply-{page}.txt")),
            ],
        ));
    }
    let summary = scaffold_manifest_row(
        BASE_APPLY_MANIFEST_SUMMARY,
        MANIFEST_SUMMARY_FIELDS,
        &[
            (
                "schema",
                "stems-beam-vlink-base-apply-manifest-v1".to_owned(),
            ),
            ("entries", CORPUS_PAGES.len().to_string()),
        ],
    );
    manifest.push_str(&summary);
    let parsed = StrictManifest::parse(manifest.as_bytes()).expect("strict manifest parses");
    assert_eq!(parsed.entries.len(), 8);

    let reordered = manifest.replace("page chula", "page allegretto-first");
    assert!(StrictManifest::parse(reordered.as_bytes()).is_err());
    let trailing = format!("{manifest}# unauthenticated\n");
    assert!(StrictManifest::parse(trailing.as_bytes()).is_err());
    let duplicated_summary = format!("{manifest}{summary}");
    assert!(StrictManifest::parse(duplicated_summary.as_bytes()).is_err());
}

#[test]
fn semantic_negative_pins_cover_hook_reads_provenance_retention_and_endpoint_bijection() {
    let hook = semantic_scaffold_row(
        RowKind::BeamIncident,
        &[
            ("phase", "AfterCallback"),
            ("incidentOrdinal", "0"),
            ("direction", "Outgoing"),
            ("directionOrdinal", "0"),
            (
                "relationClass",
                "org.audiveris.omr.sig.relation.BeamStemRelation",
            ),
            ("readState", "ExaminedClassOnly"),
            ("relevant", "true"),
            ("portion", "-"),
            ("contribution", "FirstBeamStem"),
        ],
    );
    assert_eq!(
        validate_beam_scan_rows(&[&hook], true),
        Ok((false, None, None))
    );
    let mut fabricated_hook_portion = hook.clone();
    fabricated_hook_portion
        .fields
        .iter_mut()
        .find(|(name, _)| name == "portion")
        .expect("portion field")
        .1 = "LEFT".to_owned();
    assert!(validate_beam_scan_rows(&[&fabricated_hook_portion], true).is_err());

    let raw_other = semantic_scaffold_row(
        RowKind::BeamIncident,
        &[
            ("phase", "Before"),
            ("incidentOrdinal", "0"),
            ("direction", "Outgoing"),
            ("directionOrdinal", "0"),
            ("globalRelationIdentity", "sig-edge:1"),
            (
                "relationClass",
                "org.audiveris.omr.sig.relation.HeadStemRelation",
            ),
            ("readState", "ExaminedClassOnly"),
            ("relevant", "false"),
            ("portion", "-"),
            ("contribution", "None"),
        ],
    );
    let raw_stem = semantic_scaffold_row(
        RowKind::BeamIncident,
        &[
            ("phase", "Before"),
            ("incidentOrdinal", "1"),
            ("direction", "Outgoing"),
            ("directionOrdinal", "1"),
            ("globalRelationIdentity", "sig-edge:2"),
            (
                "relationClass",
                "org.audiveris.omr.sig.relation.BeamStemRelation",
            ),
            ("readState", "ExaminedClassAndPortion"),
            ("relevant", "true"),
            ("portion", "LEFT"),
            ("contribution", "Left"),
        ],
    );
    assert_eq!(
        validate_beam_scan_rows(&[&raw_other, &raw_stem], false),
        Ok((true, Some(true), Some(false)))
    );
    let raw_null_support = semantic_scaffold_row(
        RowKind::BeamIncident,
        &[
            ("phase", "Before"),
            ("incidentOrdinal", "2"),
            ("direction", "Outgoing"),
            ("directionOrdinal", "2"),
            ("globalRelationIdentity", "sig-edge:3"),
            (
                "relationClass",
                "org.audiveris.omr.sig.relation.BeamRestRelation",
            ),
            ("readState", "ExaminedClassOnly"),
            ("relevant", "false"),
            ("portion", "-"),
            ("contribution", "None"),
        ],
    );
    assert_eq!(
        validate_beam_scan_rows(&[&raw_null_support], false),
        Ok((true, Some(false), Some(false)))
    );
    let mut fabricated_null_read = raw_null_support.clone();
    fabricated_null_read
        .fields
        .iter_mut()
        .find(|(name, _)| name == "readState")
        .expect("readState field")
        .1 = "ExaminedClassAndPortion".to_owned();
    assert!(validate_beam_scan_rows(&[&fabricated_null_read], false).is_err());
    let (_, ordered_hash) = beam_outgoing_provenance(&[&raw_other, &raw_stem]).unwrap();
    let (_, swapped_hash) = beam_outgoing_provenance(&[&raw_stem, &raw_other]).unwrap();
    assert_ne!(
        ordered_hash, swapped_hash,
        "source order is hash-significant"
    );

    assert!(
        validate_endpoint_bijection(vec![
            ("beam:1".to_owned(), 11, 2),
            ("stem:1".to_owned(), 12, 3),
        ])
        .is_ok()
    );
    assert!(
        validate_endpoint_bijection(vec![
            ("beam:1".to_owned(), 11, 2),
            ("other:alias".to_owned(), 12, 2),
        ])
        .is_err()
    );
    assert!(
        validate_endpoint_bijection(vec![
            ("beam:1".to_owned(), 11, 2),
            ("other:alias".to_owned(), 11, 3),
        ])
        .is_err()
    );

    let selected_stem_incident = semantic_scaffold_row(
        RowKind::StemIncident,
        &[
            ("phase", "Before"),
            ("direction", "Incoming"),
            ("globalRelationIdentity", "sig-edge:4"),
            ("relationObjectIdentity", "graph-object:4"),
            (
                "relationClass",
                "org.audiveris.omr.sig.relation.BeamStemRelation",
            ),
            ("oppositeAlias", "beam:1"),
        ],
    );
    let selected_beam_incident = semantic_scaffold_row(
        RowKind::BeamIncident,
        &[
            ("phase", "Before"),
            ("direction", "Outgoing"),
            ("globalRelationIdentity", "sig-edge:4"),
            ("relationObjectIdentity", "graph-object:4"),
            (
                "relationClass",
                "org.audiveris.omr.sig.relation.BeamStemRelation",
            ),
            ("oppositeAlias", "stem:1"),
        ],
    );
    assert!(
        validate_selected_incident_mirrors(
            &[&selected_stem_incident],
            &[&selected_beam_incident],
            "beam:1",
            "stem:1",
        )
        .is_ok()
    );
    assert!(
        validate_selected_incident_mirrors(&[&selected_stem_incident], &[], "beam:1", "stem:1",)
            .is_err()
    );
    let mut same_direction = selected_beam_incident.clone();
    same_direction
        .fields
        .iter_mut()
        .find(|(name, _)| name == "direction")
        .expect("direction field")
        .1 = "Incoming".to_owned();
    assert!(
        validate_selected_incident_mirrors(
            &[&selected_stem_incident],
            &[&same_direction],
            "beam:1",
            "stem:1",
        )
        .is_err()
    );
    let mut mismatched_object = selected_beam_incident.clone();
    mismatched_object
        .fields
        .iter_mut()
        .find(|(name, _)| name == "relationObjectIdentity")
        .expect("relation object field")
        .1 = "graph-object:5".to_owned();
    assert!(
        validate_selected_incident_mirrors(
            &[&selected_stem_incident],
            &[&mismatched_object],
            "beam:1",
            "stem:1",
        )
        .is_err()
    );

    let old_before = semantic_scaffold_row(
        RowKind::StemIncident,
        &[
            ("phase", "Before"),
            ("incidentOrdinal", "0"),
            ("direction", "Incoming"),
            ("directionOrdinal", "0"),
            ("globalRelationIdentity", "sig-edge:0"),
            ("relationObjectIdentity", "graph-object:0"),
            ("relationClass", "example.OldRelation"),
            ("oppositeAlias", "inter:7"),
            ("oppositeInterId", "7"),
            ("oppositeVertexOrdinal", "0"),
            ("chordStemMatch", "false"),
            ("action", "ObserveOnly"),
        ],
    );
    let mut old_after = old_before.clone();
    old_after
        .fields
        .iter_mut()
        .find(|(name, _)| name == "phase")
        .unwrap()
        .1 = "AfterCallback".to_owned();
    let fresh_after = semantic_scaffold_row(
        RowKind::StemIncident,
        &[
            ("phase", "AfterCallback"),
            ("incidentOrdinal", "1"),
            ("direction", "Incoming"),
            ("directionOrdinal", "1"),
            ("globalRelationIdentity", "sig-edge:1"),
            ("relationObjectIdentity", "synthetic-draft:case"),
            (
                "relationClass",
                "org.audiveris.omr.sig.relation.BeamStemRelation",
            ),
            ("oppositeAlias", "beam:1"),
            ("oppositeInterId", "11"),
            ("oppositeVertexOrdinal", "2"),
            ("chordStemMatch", "false"),
            ("action", "ObserveOnly"),
        ],
    );
    assert!(
        validate_pre_post_incident_retention(
            &[&old_before, &old_after, &fresh_after],
            "sig-edge:1",
            true,
        )
        .is_ok()
    );
    let mut drifted_after = old_after;
    drifted_after
        .fields
        .iter_mut()
        .find(|(name, _)| name == "relationObjectIdentity")
        .unwrap()
        .1 = "graph-object:9".to_owned();
    assert!(
        validate_pre_post_incident_retention(
            &[&old_before, &drifted_after, &fresh_after],
            "sig-edge:1",
            true,
        )
        .is_err()
    );

    let beam_mirror = semantic_scaffold_row(
        RowKind::BeamIncident,
        &[
            ("phase", "AfterCallback"),
            ("incidentOrdinal", "0"),
            ("direction", "Outgoing"),
            ("directionOrdinal", "0"),
            ("globalRelationIdentity", "sig-edge:1"),
            ("relationObjectIdentity", "synthetic-draft:case"),
            (
                "relationClass",
                "org.audiveris.omr.sig.relation.BeamStemRelation",
            ),
            ("oppositeAlias", "stem:1"),
            ("oppositeInterId", "12"),
            ("oppositeVertexOrdinal", "3"),
        ],
    );
    assert!(
        validate_selected_endpoint_incidence_mirrors(
            &[&fresh_after],
            &[&beam_mirror],
            "beam:1",
            "stem:1",
        )
        .is_ok()
    );
    assert!(
        validate_selected_endpoint_incidence_mirrors(&[&fresh_after], &[], "beam:1", "stem:1",)
            .is_err()
    );
    let mut same_direction = beam_mirror;
    same_direction
        .fields
        .iter_mut()
        .find(|(name, _)| name == "direction")
        .expect("direction field")
        .1 = "Incoming".to_owned();
    assert!(
        validate_selected_endpoint_incidence_mirrors(
            &[&fresh_after],
            &[&same_direction],
            "beam:1",
            "stem:1",
        )
        .is_err()
    );
}

#[test]
fn independent_new_vertex_edge_and_raw_callback_follow_java_order() {
    let mut state = base_state();
    let result = apply_independent_base_transaction(&mut state).expect("valid new-stem apply");
    assert_eq!(result.disposition, IndependentDisposition::Added);
    assert_eq!(
        result.vertex_action,
        VertexAction::Registered {
            inter_id: 41,
            sig_vertex_ordinal: 2,
        }
    );
    assert_eq!(state.persistent_ids.sheet_last_id, 41);
    assert_eq!(state.inter_index_slots.get(&41), Some(&200));
    assert_eq!(state.sig_vertex_count, 3);
    assert_eq!(state.sig_edge_count, 8);
    assert_eq!(result.attached_graph_relation_identity, Some(7));
    assert_eq!(result.attached_relation_object_identity, Some(70));
    assert!(result.apply_returned);
    assert_eq!(
        result.continuation_support_grade.to_bits(),
        0.75_f64.to_bits()
    );
    assert!(state.stem.abnormal);
    assert!(!state.beam.abnormal);
    assert!(state.flags.sheet_stub_modified);
    assert_eq!(
        result.post_stem_incidents,
        vec![IncidentEdge {
            graph_relation_identity: 7,
            relation_object_identity: 70,
            direction: EdgeDirection::Incoming,
            opposite_object_identity: 100,
            kind: RelationKind::BeamStem {
                portion: Some(BeamPortion::Left),
            },
        }]
    );
}

#[test]
fn independent_suppression_retains_unattached_fresh_grade() {
    let mut source_removed = base_state();
    source_removed.beam.removed = true;
    source_removed.beam.in_sig = false;
    source_removed.beam.in_inter_index = false;
    source_removed.inter_index_slots.remove(&11);
    source_removed.beam_incidents.clear();
    let result = apply_independent_base_transaction(&mut source_removed)
        .expect("source-removed suppression is ordinary Java behavior");
    assert_eq!(
        result.disposition,
        IndependentDisposition::SuppressedSourceRemoved
    );
    // addVertex precedes Link.applyTo's removed-source check.
    assert_eq!(source_removed.stem.inter_id, Some(41));
    assert_eq!(source_removed.sig_edge_count, 7);
    assert!(!source_removed.relation.attached);
    assert!(result.source_removed_read);
    assert_eq!(result.target_removed_read, None);
    assert!(result.duplicate_scan.is_empty());
    assert_eq!(
        result.continuation_support_grade.to_bits(),
        0.75_f64.to_bits()
    );

    let mut duplicate = base_state();
    duplicate.stem = LiveInter {
        object_identity: 200,
        inter_id: Some(22),
        sig_system_id: Some(1),
        in_sig: true,
        in_inter_index: true,
        removed: false,
        abnormal: false,
        vip: false,
    };
    duplicate.inter_index_slots.insert(22, 200);
    duplicate.beam_incidents.push(IncidentEdge {
        graph_relation_identity: 4,
        relation_object_identity: 40,
        direction: EdgeDirection::Outgoing,
        opposite_object_identity: 200,
        kind: RelationKind::BeamStem {
            portion: Some(BeamPortion::Center),
        },
    });
    duplicate.beam_incidents.push(IncidentEdge {
        graph_relation_identity: 6,
        relation_object_identity: 60,
        direction: EdgeDirection::Outgoing,
        opposite_object_identity: 200,
        kind: RelationKind::Other,
    });
    duplicate.stem_incidents.push(IncidentEdge {
        graph_relation_identity: 4,
        relation_object_identity: 40,
        direction: EdgeDirection::Incoming,
        opposite_object_identity: 100,
        kind: RelationKind::BeamStem {
            portion: Some(BeamPortion::Center),
        },
    });
    let allocator_before = duplicate.persistent_ids;
    let result = apply_independent_base_transaction(&mut duplicate)
        .expect("existing BeamStem suppression is ordinary Java behavior");
    assert_eq!(
        result.disposition,
        IndependentDisposition::SuppressedExistingBeamStem {
            graph_relation_identity: 4,
        }
    );
    assert_eq!(duplicate.persistent_ids, allocator_before);
    assert_eq!(duplicate.sig_edge_count, 7);
    assert!(!duplicate.relation.attached);
    assert_eq!(result.target_removed_read, Some(false));
    assert_eq!(result.source_outgoing_scanned, 3);
    assert_eq!(result.duplicate_scan.len(), 2);
    assert_eq!(result.duplicate_scan[0].source_outgoing_ordinal, 1);
    assert_eq!(result.duplicate_scan[0].class_check, Some(true));
    assert_eq!(result.duplicate_scan[1].source_outgoing_ordinal, 2);
    assert_eq!(result.duplicate_scan[1].class_check, None);
}

#[test]
fn independent_existing_vertex_preserves_allocator_and_endpoint_order() {
    let mut state = base_state();
    state.stem = LiveInter {
        object_identity: 200,
        inter_id: Some(22),
        sig_system_id: Some(1),
        in_sig: true,
        in_inter_index: true,
        removed: false,
        abnormal: false,
        vip: false,
    };
    state.inter_index_slots.insert(22, 200);
    state.stem_incidents = vec![
        IncidentEdge {
            graph_relation_identity: 1,
            relation_object_identity: 10,
            direction: EdgeDirection::Incoming,
            opposite_object_identity: 400,
            kind: RelationKind::Other,
        },
        IncidentEdge {
            graph_relation_identity: 2,
            relation_object_identity: 20,
            direction: EdgeDirection::Outgoing,
            opposite_object_identity: 500,
            kind: RelationKind::Other,
        },
    ];
    let allocator_before = state.persistent_ids;
    let result =
        apply_independent_base_transaction(&mut state).expect("existing stem add is valid");
    assert_eq!(
        result.vertex_action,
        VertexAction::AlreadyLive { inter_id: 22 }
    );
    assert_eq!(state.persistent_ids, allocator_before);
    assert_eq!(
        result
            .post_stem_incidents
            .iter()
            .map(|edge| edge.graph_relation_identity)
            .collect::<Vec<_>>(),
        vec![1, 7, 2],
        "the new incoming edge precedes the existing outgoing suffix"
    );

    let mut target_removed = base_state();
    target_removed.stem = LiveInter {
        object_identity: 200,
        inter_id: Some(22),
        sig_system_id: Some(1),
        in_sig: false,
        in_inter_index: false,
        removed: true,
        abnormal: false,
        vip: false,
    };
    let before = target_removed.clone();
    let result = apply_independent_base_transaction(&mut target_removed)
        .expect("target-removed suppression is ordinary Java behavior");
    assert_eq!(
        result.disposition,
        IndependentDisposition::SuppressedTargetRemoved
    );
    assert_eq!(result.target_removed_read, Some(true));
    assert!(result.duplicate_scan.is_empty());
    assert_eq!(target_removed, before);
    assert!(!target_removed.relation.attached);
}

#[test]
fn independent_hook_and_fail_closed_callback_domains_are_pinned() {
    let mut hook = base_state();
    hook.beam_kind = BeamKind::Hook;
    hook.beam_incidents.clear();
    let result = apply_independent_base_transaction(&mut hook).expect("hook add is valid");
    assert_eq!(result.disposition, IndependentDisposition::Added);
    assert!(!hook.beam.abnormal, "a hook with any BeamStem is normal");
    let hook_trace = result
        .post_beam_trace
        .expect("successful hook dispatch evaluates abnormal state");
    assert_eq!(hook_trace.left, None);
    assert_eq!(hook_trace.right, None);
    assert!(
        hook_trace.rows.iter().all(|row| row.portion_read.is_none()),
        "BeamHookInter.hasRelation checks classes only and never reads portions"
    );
    assert_eq!(
        hook_trace.rows.last().map(|row| row.contribution),
        Some(IndependentBeamContribution::FirstBeamStem)
    );

    let mut chord = base_state();
    chord.stem = LiveInter {
        object_identity: 200,
        inter_id: Some(22),
        sig_system_id: Some(1),
        in_sig: true,
        in_inter_index: true,
        removed: false,
        abnormal: false,
        vip: false,
    };
    chord.inter_index_slots.insert(22, 200);
    chord.stem_incidents.push(IncidentEdge {
        graph_relation_identity: 5,
        relation_object_identity: 50,
        direction: EdgeDirection::Incoming,
        opposite_object_identity: 400,
        kind: RelationKind::ChordStem,
    });
    let before = chord.clone();
    assert_eq!(
        apply_independent_base_transaction(&mut chord),
        Err(IndependentApplyError::UnsupportedChordCallback)
    );
    assert_eq!(
        chord, before,
        "Rust validation errors commit no Java prefix"
    );

    let mut allocator_drift = base_state();
    allocator_drift.persistent_ids.inter_index_last_id -= 1;
    let before = allocator_drift.clone();
    assert_eq!(
        apply_independent_base_transaction(&mut allocator_drift),
        Err(IndependentApplyError::AllocatorViewsDiffer)
    );
    assert_eq!(allocator_drift, before);
}

#[test]
fn independent_java_envelope_replays_partial_commits_without_claiming_production_support() {
    let mut missing = base_state();
    missing.stem.inter_id = Some(1_500_000_001);
    let before = missing.clone();
    let prefix = apply_java_envelope_prefix(&mut missing, JavaEnvelopeCase::MissingPositiveTarget)
        .expect("Java reaches JGraphT's missing-target failure");
    assert_eq!(prefix.throw_stage, JavaThrowStage::LinkApply);
    assert!(!prefix.callback_completed);
    assert!(prefix.mutations.is_empty());
    assert_eq!(missing, before);

    let mut vertex_listener = base_state();
    let prefix =
        apply_java_envelope_prefix(&mut vertex_listener, JavaEnvelopeCase::NewVertexListener)
            .expect("Java retains the pre-SigListener vertex prefix");
    assert_eq!(prefix.throw_stage, JavaThrowStage::VertexAdd);
    assert_eq!(vertex_listener.persistent_ids.sheet_last_id, 41);
    assert_eq!(vertex_listener.stem.inter_id, Some(41));
    assert!(vertex_listener.stem.in_inter_index);
    assert!(vertex_listener.stem.in_sig, "the graph vertex was inserted");
    assert_eq!(
        vertex_listener.stem.sig_system_id, None,
        "the earlier listener prevented SigListener from assigning the SIG pointer"
    );
    assert!(!vertex_listener.stem.abnormal);
    assert_eq!(vertex_listener.flags, base_state().flags);
    assert!(!vertex_listener.relation.attached);

    let existing = || {
        let mut state = base_state();
        state.stem = LiveInter {
            object_identity: 200,
            inter_id: Some(22),
            sig_system_id: Some(1),
            in_sig: true,
            in_inter_index: true,
            removed: false,
            abnormal: false,
            vip: false,
        };
        state.inter_index_slots.insert(22, 200);
        state
    };
    let mut early_edge = existing();
    let prefix = apply_java_envelope_prefix(&mut early_edge, JavaEnvelopeCase::EarlyEdgeListener)
        .expect("Java retains the structural edge before the early listener throw");
    assert_eq!(prefix.throw_stage, JavaThrowStage::LinkApply);
    assert!(!prefix.callback_completed);
    assert!(early_edge.relation.attached);
    assert_eq!(early_edge.sig_edge_count, 8);
    assert_eq!(early_edge.stem_incidents.len(), 1);
    assert_eq!(early_edge.beam_incidents.len(), 2);
    assert!(early_edge.beam.abnormal);
    assert_eq!(early_edge.flags, base_state().flags);

    let mut later_edge = existing();
    let prefix = apply_java_envelope_prefix(&mut later_edge, JavaEnvelopeCase::LaterEdgeListener)
        .expect("Java retains both structural edge and standard callback prefix");
    assert!(prefix.callback_completed);
    assert!(later_edge.relation.attached);
    assert!(!later_edge.beam.abnormal);
    assert!(later_edge.flags.sheet_stub_modified);
    assert!(
        prefix
            .mutations
            .contains(&IndependentMutation::RelationCallbackDispatched)
    );
}

#[test]
fn active_source_and_predecessor_provenance_helpers_reject_drift() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    for relative in [BASE_APPLY_PROBE_SOURCE, BASE_APPLY_RUNNER_SOURCE]
        .into_iter()
        .chain(
            SOURCE_PATHS
                .iter()
                .chain(PREDECESSOR_PATHS)
                .map(|(_, relative)| *relative),
        )
    {
        let actual = actual_file_sha256(relative).expect("active provenance file is readable");
        assert_eq!(actual.len(), 64);
        let path = repo_root().join(relative);
        assert!(validate_provenance_pin(&path, &actual).is_ok());
        assert!(validate_provenance_pin(&path, &"0".repeat(64)).is_err());
    }
    let artifact = validate_jgrapht_core_provenance(JGRAPHT_CORE_JAR_SHA256)
        .expect("declared and frozen JGraphT provenance is valid");
    if let ActiveJGraphTCoreArtifact::Present(jgrapht) = artifact {
        assert_eq!(
            jgrapht.file_name().and_then(|name| name.to_str()),
            Some(JGRAPHT_CORE_JAR)
        );
        assert!(validate_provenance_pin(&jgrapht, JGRAPHT_CORE_JAR_SHA256).is_ok());
        assert!(validate_provenance_pin(&jgrapht, &"0".repeat(64)).is_err());
    }
    assert!(validate_jgrapht_core_provenance(&"0".repeat(64)).is_err());
}

#[test]
fn jgrapht_cache_absence_and_present_corruption_are_distinguished() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock follows the Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "audiveris-b14-jgrapht-{}-{nonce}",
        std::process::id()
    ));
    let absent = locate_jgrapht_core_artifact(&root).expect("missing cache is an explicit mode");
    assert!(matches!(
        absent,
        ActiveJGraphTCoreArtifact::Absent {
            artifact_root: Some(_)
        }
    ));
    assert!(validate_active_jgrapht_core_artifact(&absent, JGRAPHT_CORE_JAR_SHA256).is_ok());

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let broken_root = root.join("broken-root");
        let broken_artifact_root = broken_root
            .join("caches/modules-2/files-2.1/org.jgrapht/jgrapht-core")
            .join(JGRAPHT_CORE_VERSION);
        std::fs::create_dir_all(
            broken_artifact_root
                .parent()
                .expect("artifact root has a parent"),
        )
        .expect("create broken-root cache parent");
        symlink("missing-version-directory", &broken_artifact_root)
            .expect("create broken artifact-root symlink");
        assert!(locate_jgrapht_core_artifact(&broken_root).is_err());

        let broken_candidate_root = root.join("broken-candidate");
        let broken_candidate = broken_candidate_root
            .join("caches/modules-2/files-2.1/org.jgrapht/jgrapht-core")
            .join(JGRAPHT_CORE_VERSION)
            .join("entry")
            .join(JGRAPHT_CORE_JAR);
        std::fs::create_dir_all(
            broken_candidate
                .parent()
                .expect("artifact candidate has a parent"),
        )
        .expect("create broken-candidate cache parent");
        symlink("missing-jgrapht-core.jar", &broken_candidate)
            .expect("create broken artifact symlink");
        assert!(locate_jgrapht_core_artifact(&broken_candidate_root).is_err());
    }

    let normal_root = root.join("normal");
    let first = normal_root
        .join("caches/modules-2/files-2.1/org.jgrapht/jgrapht-core")
        .join(JGRAPHT_CORE_VERSION)
        .join("first")
        .join(JGRAPHT_CORE_JAR);
    std::fs::create_dir_all(first.parent().expect("artifact has a parent"))
        .expect("create isolated fake Gradle cache");
    std::fs::write(&first, b"not the audited JGraphT artifact")
        .expect("write isolated corrupt artifact");
    let corrupt = locate_jgrapht_core_artifact(&normal_root).expect("one artifact is discoverable");
    assert!(matches!(corrupt, ActiveJGraphTCoreArtifact::Present(_)));
    assert!(validate_active_jgrapht_core_artifact(&corrupt, JGRAPHT_CORE_JAR_SHA256).is_err());

    let second = normal_root
        .join("caches/modules-2/files-2.1/org.jgrapht/jgrapht-core")
        .join(JGRAPHT_CORE_VERSION)
        .join("second")
        .join(JGRAPHT_CORE_JAR);
    std::fs::create_dir_all(second.parent().expect("artifact has a parent"))
        .expect("create second isolated fake cache entry");
    std::fs::write(&second, b"another artifact").expect("write second isolated artifact");
    assert!(locate_jgrapht_core_artifact(&normal_root).is_err());

    std::fs::remove_dir_all(&root).expect("remove isolated fake Gradle cache");
}

fn project_frozen_base_apply_page(page_key: &str, bytes: &[u8]) -> Result<usize, String> {
    let parsed = StrictFixture::parse(bytes)?;
    if page_key_from_token(&parsed.page.page)? != page_key || parsed.systems.is_empty() {
        return Err(format!("{page_key} fixture page/system identity differs"));
    }
    validate_fixture_trailer(&parsed, bytes)?;
    validate_fixture_algebra(&parsed)?;
    let create_path =
        repo_root().join(format!("rust/oracle/stems-beam-create-stem-{page_key}.txt"));
    let reuse_path = repo_root().join(format!(
        "rust/oracle/stems-beam-vlink-reuse-check-{page_key}.txt"
    ));
    let create_text = std::fs::read_to_string(&create_path)
        .map_err(|error| format!("{}: {error}", create_path.display()))?;
    let reuse_text = std::fs::read_to_string(&reuse_path)
        .map_err(|error| format!("{}: {error}", reuse_path.display()))?;
    let create = PredecessorFixture::parse(&create_text, CREATE_STEM_SCHEMA)?;
    let reuse = PredecessorFixture::parse(&reuse_text, REUSE_CHECK_SCHEMA)?;
    let native = native_predecessor_page(page_image_name(page_key)?);
    let mut real_systems = 0;
    for rows in parsed.systems.values() {
        for block in transaction_slices(rows) {
            let baseline = one_row(block, RowKind::Baseline)?;
            match baseline.value("scope")? {
                "real" => {
                    project_compact_queries(&parsed.page, block)?;
                    project_real_base_system(
                        page_key,
                        &native,
                        &parsed.page,
                        block,
                        &create,
                        &reuse,
                    )?;
                    real_systems += 1;
                }
                "synthetic" => {
                    // The isolated Java graph has no honest boundary-13 native
                    // predecessor. Replay it independently instead of fabricating
                    // a product merely to invoke the public top-level API.
                    let mut state = build_independent_real_state(block)?;
                    let result =
                        apply_independent_base_transaction(&mut state).map_err(|error| {
                            format!(
                                "{page_key} {} supported replay: {error:?}",
                                baseline.value("case").unwrap_or("?")
                            )
                        })?;
                    assert_independent_matches_java(block, &result, &state)?;
                }
                "envelope" => {
                    let case = match baseline.value("case")? {
                        "MissingPositiveTargetThrows" => JavaEnvelopeCase::MissingPositiveTarget,
                        "NewVertexListenerThrows" => JavaEnvelopeCase::NewVertexListener,
                        "EarlyEdgeListenerThrows" => JavaEnvelopeCase::EarlyEdgeListener,
                        "LaterEdgeListenerThrows" => JavaEnvelopeCase::LaterEdgeListener,
                        value => return Err(format!("unknown Java envelope case {value}")),
                    };
                    let mut state = build_independent_real_state(block)?;
                    let prefix = apply_java_envelope_prefix(&mut state, case).map_err(|error| {
                        format!(
                            "{page_key} {} envelope replay: {error:?}",
                            baseline.value("case").unwrap_or("?")
                        )
                    })?;
                    assert_java_envelope_matches_rows(block, &prefix, &state)?;
                }
                value => return Err(format!("unknown boundary-14 scope {value}")),
            }
        }
    }
    if real_systems != parsed.systems.len() {
        return Err(format!("{page_key} real system count differs"));
    }
    Ok(real_systems)
}

#[test]
fn boundary_fourteen_manifest_reconstructs_the_full_corpus() {
    let path = repo_root().join(BASE_APPLY_MANIFEST);
    validate_provenance_pin(&path, BASE_APPLY_MANIFEST_SHA256).expect("frozen manifest SHA");
    let bytes = std::fs::read(path).expect("frozen manifest is installed");
    assert_eq!(
        bytes.iter().filter(|byte| **byte == b'\n').count(),
        BASE_APPLY_MANIFEST_LINES
    );
    assert_eq!(bytes.len(), BASE_APPLY_MANIFEST_BYTES);
    let parsed = validate_base_apply_manifest_corpus(&bytes)
        .expect("frozen manifest and split corpus validate exactly");
    assert_eq!(parsed.entries.len(), CORPUS_PAGES.len());

    let mut projected_systems = 0_usize;
    for entry in &parsed.entries {
        let page = entry.value("page").expect("strict manifest page");
        let fixture_path = repo_root().join("rust/oracle").join(
            entry
                .value("fixture")
                .expect("strict manifest fixture name"),
        );
        let fixture = std::fs::read(&fixture_path).expect("manifest split fixture is installed");
        let projected = project_frozen_base_apply_page(page, &fixture)
            .unwrap_or_else(|error| panic!("exact {page} projection: {error}"));
        assert_eq!(
            projected,
            manifest_usize(entry, "systems").expect("strict manifest systems")
        );
        projected_systems += projected;
    }
    assert_eq!(projected_systems, 30);

    let trailing_comment = [bytes.as_slice(), b"# unauthenticated\n"].concat();
    assert!(validate_base_apply_manifest_corpus(&trailing_comment).is_err());

    let manifest_text = String::from_utf8(bytes.clone()).expect("manifest is UTF-8");
    let corpus_sha = parsed
        .summary
        .value("corpusBodySha256")
        .expect("strict corpus SHA");
    let corrupted_corpus = manifest_text.replacen(
        &format!("corpusBodySha256 {corpus_sha}"),
        &format!("corpusBodySha256 {}", "0".repeat(64)),
        1,
    );
    assert!(StrictManifest::parse(corrupted_corpus.as_bytes()).is_ok());
    assert!(validate_base_apply_manifest_corpus(corrupted_corpus.as_bytes()).is_err());

    let chula_path = repo_root().join("rust/oracle/stems-beam-vlink-base-apply-chula.txt");
    let mut chula = std::fs::read_to_string(chula_path).expect("Chula fixture is UTF-8");
    let field = "sourceOutgoingProvenanceSha256 ";
    let value_start = chula.find(field).expect("Chula outgoing provenance") + field.len();
    let value_end = chula[value_start..]
        .find(' ')
        .map_or(chula.len(), |offset| value_start + offset);
    assert_eq!(value_end - value_start, 64);
    chula.replace_range(value_start..value_end, &"0".repeat(64));
    let corrupted_query = StrictFixture::parse(chula.as_bytes())
        .expect("well-formed corrupted query fixture still parses");
    assert!(validate_fixture_algebra(&corrupted_query).is_err());
}
