//! Independent gate scaffold for `BeamLinker.VLinker.linkSiblings`.
//!
//! Boundary 16 begins immediately after the selected B-linker's shared
//! `linked` cell was assigned by boundary 15.  It executes the complete
//! sibling-beam loop, including graph callbacks and sibling-linker flag
//! writes, and stops before the first head-relation-loop read.  The model in
//! this file is deliberately independent from production so that the frozen
//! Java rows and the native transaction have two separately reviewable
//! projections.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use audiveris_image::beam_structure::Segment;
use audiveris_omr::{
    native_stems_beam_builders::{
        NativeStemsBeamBuilder, NativeStemsBeamBuilderItemKind, NativeStemsBeamBuilderTargetRef,
    },
    native_stems_beam_stumps::NativeStemsBeamSource,
    native_stems_beam_vlink_base_apply::{
        NativeStemsBeamBeamIncidentRead, NativeStemsBeamBeamIncidentRule,
        NativeStemsBeamGroupRuntimeState, NativeStemsBeamIncidentDirection,
        NativeStemsBeamIncidentOpposite, NativeStemsBeamQueryRelationKind,
        NativeStemsBeamSheetEditState, NativeStemsBeamSigListenerTopology,
        NativeStemsBeamSigRelationKind, NativeStemsBeamVLinkBeamRuntimeState,
    },
    native_stems_beam_vlink_head_links::{
        NativeStemsBeamHeadAbnormalScan, NativeStemsBeamHeadAppendedRelation,
        NativeStemsBeamHeadDirectedPairScan, NativeStemsBeamHeadDirtySubject,
        NativeStemsBeamHeadIncidentClassRead, NativeStemsBeamHeadIncidentOpposite,
        NativeStemsBeamHeadIncidentRelation, NativeStemsBeamHeadIncidentScan,
        NativeStemsBeamHeadLinkBranch, NativeStemsBeamHeadLinkHeadRef,
        NativeStemsBeamHeadLinkHeadState, NativeStemsBeamHeadLinkStepCertificate,
        NativeStemsBeamHeadPairClassRead, NativeStemsBeamHeadPairRelation,
        NativeStemsBeamHeadPlanDraft, NativeStemsBeamHeadQueryRelationKind,
        NativeStemsBeamHeadRelationObjectIdentity, NativeStemsBeamHeadSLinkerCell,
        NativeStemsBeamHeadSLinkerRef, NativeStemsBeamHeadSourceOutgoingRelation,
        NativeStemsBeamVLinkHeadLinksCertificate, NativeStemsBeamVLinkHeadLinksOperation,
        NativeStemsBeamVLinkHeadLinksOutcome, NativeStemsBeamVLinkHeadLinksState,
        NativeStemsBeamVLinkHeadLinksTransaction,
        apply_native_stems_beam_vlink_head_links_transaction,
    },
    native_stems_beam_vlink_sibling_links::{
        NativeStemsBeamSiblingAppendedRelation, NativeStemsBeamSiblingBLinkerCell,
        NativeStemsBeamSiblingBeamAbnormalTrace, NativeStemsBeamSiblingBeamIncidentRelation,
        NativeStemsBeamSiblingBeamIncidentScan, NativeStemsBeamSiblingBranch,
        NativeStemsBeamSiblingBuilderAction, NativeStemsBeamSiblingBuilderItemRead,
        NativeStemsBeamSiblingBuilderLinkerIdentity, NativeStemsBeamSiblingBuilderLinkerRead,
        NativeStemsBeamSiblingBuilderLookupRow, NativeStemsBeamSiblingBuilderLookupScan,
        NativeStemsBeamSiblingBuilderLookupState, NativeStemsBeamSiblingBuilderLookupTiming,
        NativeStemsBeamSiblingBuilderSourceRead, NativeStemsBeamSiblingGeometryTrace,
        NativeStemsBeamSiblingGlyphIdentity, NativeStemsBeamSiblingGroupMemberTrace,
        NativeStemsBeamSiblingGroupRelation, NativeStemsBeamSiblingGroupRuntimeState,
        NativeStemsBeamSiblingGroupScan, NativeStemsBeamSiblingGroupTarget,
        NativeStemsBeamSiblingGroupTargetEvidence, NativeStemsBeamSiblingLiveBeam,
        NativeStemsBeamSiblingPairClassRead, NativeStemsBeamSiblingPairRelation,
        NativeStemsBeamSiblingPairScan, NativeStemsBeamSiblingQueryProvenance,
        NativeStemsBeamSiblingRelationObjectIdentity, NativeStemsBeamSiblingSourceOutgoingRelation,
        NativeStemsBeamSiblingStemIncidentRelation, NativeStemsBeamSiblingStemIncidentScan,
        NativeStemsBeamSiblingStepCertificate, NativeStemsBeamVLinkSiblingLinksCertificate,
        NativeStemsBeamVLinkSiblingLinksOperation, NativeStemsBeamVLinkSiblingLinksOutcome,
        NativeStemsBeamVLinkSiblingLinksState, NativeStemsBeamVLinkSiblingLinksTransaction,
        apply_native_stems_beam_vlink_sibling_links_transaction,
    },
    native_stems_beam_vlinkers::{NativeStemsBeamBLinkerRef, NativeStemsBeamVLinkerRef},
    stems_step::{NativeBeamPortion, NativeStemHeadSide, NativeStemPoint, NativeStemVerticalSide},
};

const HEAD_LINKS_FIXTURE_SCHEMA: &str = "# schema: stems-beam-vlink-head-links-v1";
const HEAD_LINKS_PROBE_SOURCE_PATH: &str = "rust/oracle/java/StemsBeamVLinkHeadLinksProbe.java";
const HEAD_LINKS_RUNNER_SOURCE_PATH: &str = "rust/oracle/java/run-stems-beam-vlink-head-links.sh";
const HEAD_LINKS_MANIFEST_SCHEMA: &str = "# schema: stems-beam-vlink-head-links-manifest-v1";
const HEAD_LINKS_MANIFEST_PATH: &str = "rust/oracle/stems-beam-vlink-head-links-manifest.txt";
const HEAD_LINKS_MANIFEST_OVERRIDE_ENV: &str = "AUDIVERIS_B17_HEAD_LINKS_MANIFEST";
const HEAD_LINKS_MANIFEST_ENTRY_LABEL: &str = "stemsbeamvlinkheadlinksmanifestentry";
const HEAD_LINKS_MANIFEST_SUMMARY_LABEL: &str = "stemsbeamvlinkheadlinksmanifestsummary";
const HEAD_LINKS_MANIFEST_SHA256: &str =
    "87b1f5fb459551cb247f4702449128f35d94ac5ee738d764e25e523dd21955ab";
const HEAD_LINKS_MANIFEST_LINES: usize = 10;
const HEAD_LINKS_MANIFEST_BYTES: usize = 35_839;
const HEAD_LINKS_MANIFEST_BODY_SHA256: &str =
    "a7934a066b47654b56184e6506825d9f1f5986d96f25b3eb52b2281308185a08";
const HEAD_LINKS_MANIFEST_BODY_LINES: usize = 9;
const HEAD_LINKS_MANIFEST_BODY_BYTES: usize = 25_997;
const HEAD_LINKS_NORMALIZED_CORPUS_SHA256: &str =
    "b57ec3f2bf401fce6d6d62c7522285dd3288b35b40d7c5c453468cf5dde4ce48";
const HEAD_LINKS_NORMALIZED_CORPUS_LINES: usize = 1_583;
const HEAD_LINKS_NORMALIZED_CORPUS_BYTES: usize = 785_671;
const HEAD_LINKS_SPLIT_EMITTED_BODY_SHA256: &str =
    "044631a9dc5177b3fbe074a03cc031f52cb6087b3ea3491377f820d633b44d01";
const HEAD_LINKS_SPLIT_EMITTED_BODY_LINES: usize = 1_639;
const HEAD_LINKS_SPLIT_EMITTED_BODY_BYTES: usize = 790_438;
const HEAD_LINKS_SPLIT_FIXTURE_SHA256: &str =
    "6e9abd60f5274622bd9638cc6e1cd6c489ee5fdc36ec96769507ef9f16f418aa";
const HEAD_LINKS_SPLIT_FIXTURE_LINES: usize = 1_655;
const HEAD_LINKS_SPLIT_FIXTURE_BYTES: usize = 873_975;
const HEAD_LINKS_FIXTURE_HEADER: &[&str] = &[
    "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) beam VLink head-links oracle.",
    "# schema: stems-beam-vlink-head-links-v1",
    "# Frozen scheduler through sibling-links predecessors are replayed and joined exactly.",
    "# Head plans follow the original LinkedHashMap first-insertion order with latest payload values.",
    "# Each entry writes its shared SLinker before the directed HeadStem duplicate lookup.",
    "# Missing pairs mutate the existing plan draft, insert it directly, and run the synchronous callback.",
    "# Isolated manual, chord, duplicate, and exception evidence is gate-only, not production-equivalent.",
    "# Stop is after return true and immediately before the caller outer BLinker assignment.",
];

#[path = "common/b15_hydration.rs"]
mod b15_hydration;

const FIXTURE_SCHEMA: &str = "# schema: stems-beam-vlink-sibling-links-v1";
const FIXTURE_OVERRIDE_ENV: &str = "AUDIVERIS_B16_SIBLING_LINKS_FIXTURE";
const CORPUS_PAGES: [(&str, &str); 8] = [
    ("chula", "chula.png"),
    ("allegretto", "allegretto.png"),
    ("batuque", "batuque.png"),
    ("carmen", "carmen.png"),
    ("cucaracha", "cucaracha.png"),
    ("hove", "hove.png"),
    ("zizi", "zizi.png"),
    ("BachInvention5", "BachInvention5.jpg"),
];
const BOUNDARY_FIFTEEN_MANIFEST_PATH: &str =
    "rust/oracle/stems-beam-vlink-b-linker-flag-manifest.txt";
const BOUNDARY_FIFTEEN_MANIFEST_SHA256: &str =
    "c7032ac4871188ef0cf48ac63d99996e78a0e163bf1470d3be84c5e9b10d1d92";
const BOUNDARY_FIFTEEN_GATE_PATH: &str =
    "rust/crates/audiveris-omr/tests/native_stems_beam_vlink_b_linker_flag.rs";
const BOUNDARY_FIFTEEN_GATE_SHA256: &str =
    "41601603e4845602135bfeba98ff69e7820c5b0914891bc1f56930052554c0e5";
const BOUNDARY_FIFTEEN_FIXTURE_PATH: &str = "rust/oracle/stems-beam-vlink-b-linker-flag-chula.txt";
const BOUNDARY_FIFTEEN_FIXTURE_SHA256: &str =
    "85681437af5e7a5b3c5fc220fe7ced7299516b9de8c4d95a6c651dd5ebf926d6";
const PROBE_SOURCE_PATH: &str = "rust/oracle/java/StemsBeamVLinkSiblingLinksProbe.java";
const RUNNER_SOURCE_PATH: &str = "rust/oracle/java/run-stems-beam-vlink-sibling-links.sh";
const MANIFEST_SCHEMA: &str = "# schema: stems-beam-vlink-sibling-links-manifest-v1";
const MANIFEST_PATH: &str = "rust/oracle/stems-beam-vlink-sibling-links-manifest.txt";
const MANIFEST_OVERRIDE_ENV: &str = "AUDIVERIS_B16_SIBLING_LINKS_MANIFEST";
const MANIFEST_ENTRY_LABEL: &str = "stemsbeamvlinksiblinglinksmanifestentry";
const MANIFEST_SUMMARY_LABEL: &str = "stemsbeamvlinksiblinglinksmanifestsummary";
const MANIFEST_SHA256: &str = "6dcca78c13facf7fa9ee29506eab2961d1410babf396930724dce16f5474e29d";
const MANIFEST_LINES: usize = 10;
const MANIFEST_BYTES: usize = 31_471;
const MANIFEST_BODY_SHA256: &str =
    "c5d44bf655814aac1a297d4ad67fe401291449e231d581d11c812e197ef0fba0";
const MANIFEST_BODY_LINES: usize = 9;
const MANIFEST_BODY_BYTES: usize = 23_218;
const NORMALIZED_CORPUS_SHA256: &str =
    "c6a62f9b98ce55eda2bd142b083a2ff6b14d08dab6b1a2ce3c1a0d643d5efd66";
const NORMALIZED_CORPUS_LINES: usize = 717;
const NORMALIZED_CORPUS_BYTES: usize = 580_329;
const SPLIT_FIXTURE_LINES: usize = 789;
const SPLIT_FIXTURE_BYTES: usize = 654_858;

const FIXTURE_HEADER: &[&str] = &[
    "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) beam VLink sibling-links oracle.",
    FIXTURE_SCHEMA,
    "# Frozen scheduler/expand/createStem/reuse-check/base-apply/B-linker-flag predecessors are replayed and joined.",
    "# Group members follow outgoing Containment insertion order then Java stable top-down Double.compare sorting.",
    "# Siblings execute serially: glyph skip, directed duplicate scan, lazy shorter geometry, edge callback, item lookup, flag write.",
    "# Fresh BeamStem callbacks require exhaustive zero ChordStem matches in compact v1.",
    "# Java exception cases are envelope-only evidence and are not production-equivalent transactions.",
    "# Stop is after linkSiblings returns and immediately before the head-relation entry-set loop.",
];

const COMMON_FIELDS: &[&str] = &["system", "plan", "scope", "case"];
const PAGE_FIELDS: &[&str] = &[
    "systems",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "createStemFixtureSha256",
    "reuseCheckFixtureSha256",
    "baseApplyFixtureSha256",
    "bLinkerFlagFixtureSha256",
    "executionMode",
    "groupOrder",
    "incidentOrder",
    "headless",
    "methodDispatch",
    "stop",
];
const PREDECESSOR_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "join",
    "b15TransactionRows",
    "b15TransactionEvidenceSha256",
    "b15ResultRowSha256",
    "b15GuardRowSha256",
    "b15SummaryRowSha256",
    "predecessorTerminal",
    "applyReturn",
    "supportGrade",
    "stemAlias",
    "stemInterId",
    "baseBeamAlias",
    "targetBAlias",
    "triggeringVAlias",
    "targetBLinked",
];
const BASELINE_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "baseBeamAlias",
    "baseBeamClass",
    "baseBeamInterId",
    "baseBeamVertexOrdinal",
    "baseBeamGlyphIdentity",
    "baseBeamGlyph",
    "baseBeamMedian",
    "baseBeamHeight",
    "baseBeamAbnormal",
    "stemAlias",
    "stemInterId",
    "stemMedian",
    "stemAbnormal",
    "cachedMedianSameIdentity",
    "refPt",
    "yDir",
    "skewedVertical",
    "groupAlias",
    "groupClass",
    "groupInterId",
    "groupVertexOrdinal",
    "groupRemoved",
    "groupVip",
    "groupAbnormal",
    "groupStateHashBefore",
    "groupObjectStateHash",
    "groupOutgoingScanned",
    "groupQueryProvenanceSha256",
    "groupMembers",
    "selectedBeforeBaseRemoval",
    "baseRemoved",
    "siblings",
    "maxBeamSideDx",
    "maxShorterRatio",
    "interline",
    "xInGapMaximum0",
    "maxDxRint",
    "baseCross",
    "baseLength",
    "supportGradeSource",
    "supportGradeRead",
    "supportGrade",
    "graphVertices",
    "graphEdgesBefore",
    "graphVertexHashBefore",
    "graphEdgeHashBefore",
    "listenerTopologyHash",
    "soleSigListener",
    "stemIncidentState",
    "stemIncidentRows",
    "chordStemMatches",
    "stemIncidentHash",
    "builderItems",
    "builderItemsHash",
    "arenaStateHashBefore",
    "relationInputHashBefore",
];
const GROUP_MEMBER_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "groupOutgoingOrdinal",
    "graphRelationIdentity",
    "relationObjectIdentity",
    "relationClass",
    "containmentMatch",
    "targetAlias",
    "targetRuntimeClass",
    "targetReadByGetMembers",
    "targetEvidence",
    "targetInterId",
    "targetVertexOrdinal",
    "memberOrdinal",
    "beamRuntimeClass",
    "beamInterId",
    "interIndexOrdinal",
    "interIndexObjectMatches",
    "interIndexIdMatches",
    "sigMembership",
    "sigSystemId",
    "beamRemoved",
    "beamVip",
    "beamAbnormal",
    "beamGroupVertexOrdinal",
    "beamGroupStateHash",
    "beamGroupObjectStateHash",
    "median",
    "height",
    "glyphIdentity",
    "glyph",
    "verticalCross",
    "leftLimit",
    "rightLimit",
    "inclusiveLeft",
    "inclusiveRight",
    "selected",
    "sortedOrdinal",
    "baseIdentity",
    "removeAction",
];
const SIBLING_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "siblingOrdinal",
    "sortedOrdinal",
    "beamAlias",
    "beamClass",
    "beamInterId",
    "beamVertexOrdinal",
    "glyphIdentity",
    "glyph",
    "baseGlyphSameIdentity",
    "pairState",
    "sourceOutgoingScanned",
    "sourceOutgoingProvenanceSha256",
    "pairRows",
    "pairProvenanceSha256",
    "firstBeamStemIdentity",
    "branch",
];
const SOURCE_OUTGOING_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "siblingOrdinal",
    "sourceOutgoingOrdinal",
    "graphRelationIdentity",
    "relationObjectIdentity",
    "runtimeClass",
];
const PAIR_RELATION_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "siblingOrdinal",
    "pairOrdinal",
    "sourceOutgoingOrdinal",
    "graphRelationIdentity",
    "relationObjectIdentity",
    "runtimeClass",
    "classRead",
    "matches",
    "action",
];
const GEOMETRY_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "siblingOrdinal",
    "stemMedian",
    "baseMedian",
    "siblingMedian",
    "baseCross",
    "siblingCross",
    "baseLength",
    "siblingLength",
    "ratio",
    "maxShorterRatio",
    "shorterInclusive",
    "dyRead",
    "dy",
    "yDir",
    "product",
    "wrongSideStrict",
    "extension",
    "beamHalfHeight",
    "xInGapMaximum0",
    "maxDxRint",
    "leftThreshold",
    "rightThreshold",
    "strictLeft",
    "strictRight",
    "portion",
    "gradeSource",
    "gradeRead",
    "grade",
    "branch",
];
const EDGE_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "siblingOrdinal",
    "eventOrdinal",
    "freshRelationIdentity",
    "relationClass",
    "sourceAlias",
    "sourceInterId",
    "targetAlias",
    "targetInterId",
    "insertionReturn",
    "returnObservedByCaller",
    "graphRelationIdentity",
    "graphInsertionOrdinal",
    "sourceOutgoingOrdinal",
    "targetIncomingOrdinal",
    "extension",
    "portion",
    "grade",
];
const STEM_INCIDENT_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "siblingOrdinal",
    "eventOrdinal",
    "incidentOrdinal",
    "direction",
    "directionOrdinal",
    "graphRelationIdentity",
    "relationObjectIdentity",
    "runtimeClass",
    "classRead",
    "oppositeReadByGetChords",
    "oppositeEvidence",
    "oppositeAlias",
    "oppositeInterId",
    "oppositeVertexOrdinal",
    "chordStemMatch",
];
const BEAM_INCIDENT_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "siblingOrdinal",
    "eventOrdinal",
    "incidentOrdinal",
    "direction",
    "directionOrdinal",
    "graphRelationIdentity",
    "relationObjectIdentity",
    "runtimeClass",
    "classRead",
    "oppositeReadByCheckAbnormal",
    "oppositeEvidence",
    "oppositeAlias",
    "oppositeInterId",
    "oppositeVertexOrdinal",
    "readState",
    "relevant",
    "portion",
    "contribution",
];
const CALLBACK_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "siblingOrdinal",
    "eventOrdinal",
    "relationAdded",
    "extensionPrepopulated",
    "portionPrepopulated",
    "defaultExtensionBranchRead",
    "defaultPortionBranchRead",
    "stemIncidentState",
    "stemIncidentRows",
    "chordStemMatches",
    "stemIncidentHash",
    "beamIncidentState",
    "beamRule",
    "beamIncidentRows",
    "beamIncidentHash",
    "requestedAbnormal",
    "beamAbnormalBefore",
    "beamAbnormalAfter",
    "abnormalChanged",
    "dirtyBefore",
    "dirtyAfter",
];
const LINKER_LOOKUP_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "siblingOrdinal",
    "itemOrdinal",
    "runtimeClass",
    "linkerRead",
    "sourceRead",
    "linkerAlias",
    "linkerRuntimeClass",
    "sourceAlias",
    "sourceInterId",
    "identityMatch",
    "action",
];
const LINKER_FLAG_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "siblingOrdinal",
    "eventOrdinal",
    "lookupState",
    "selectedAlias",
    "selectedRuntimeClass",
    "selectedSourceAlias",
    "sharedCell",
    "observerAliases",
    "linkedBefore",
    "linkedAfter",
    "closedBefore",
    "closedAfter",
    "requested",
    "writeCount",
    "valueChangeCount",
    "transition",
];
const SIBLING_RESULT_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "siblingOrdinal",
    "branch",
    "edgeCommitted",
    "linkerLookupRead",
    "linkerLookupState",
    "linkerLookupTiming",
    "linkerLookupRows",
    "linkerLookupHash",
    "linkerSelectedAlias",
    "edgePrefixCount",
    "flagPrefixCount",
    "terminal",
];
const RESULT_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "terminal",
    "applyReturn",
    "supportGrade",
    "stemAlias",
    "stemInterId",
    "siblings",
    "committedEdges",
    "committedEdgeAliases",
    "committedFlags",
    "committedBCells",
    "eventCount",
    "groupStateHashAfter",
    "headRelationLoopRead",
];
const DELTA_GUARD_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "allocatorBefore",
    "allocatorAfter",
    "glyphActiveHashBefore",
    "glyphActiveHashAfter",
    "glyphOriginalsHashBefore",
    "glyphOriginalsHashAfter",
    "interIndexCountBefore",
    "interIndexCountAfter",
    "sigVertexCountBefore",
    "sigVertexCountAfter",
    "sigEdgeCountBefore",
    "sigEdgeCountAfter",
    "systemStemsHashBefore",
    "systemStemsHashAfter",
    "lineHashBefore",
    "lineHashAfter",
    "arenaTopologyHashBefore",
    "arenaTopologyHashAfter",
    "builderItemsHashBefore",
    "builderItemsHashAfter",
    "relationInputHashBefore",
    "relationInputHashAfter",
    "groupStateHashBefore",
    "groupStateHashAfter",
    "allowedMutations",
    "zeroChordStem",
    "baseBeamStemUnchanged",
    "unrelatedRelationsUnchanged",
    "unrelatedLinkerFlagsUnchanged",
    "headRelationLoopRead",
    "stopBeforeHeadRelationLoop",
];
const SUMMARY_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "groupRows",
    "siblings",
    "sameGlyph",
    "existingBeamStem",
    "shorterWrongSide",
    "linked",
    "edgesAdded",
    "linkerWrites",
    "events",
    "chordStemMatches",
    "terminal",
];
const PAGE_SUMMARY_FIELDS: &[&str] = &[
    "systems",
    "realTransactions",
    "supportedSyntheticCases",
    "envelopeCases",
    "totalTransactions",
    "groupRows",
    "siblingCandidates",
    "sameGlyph",
    "existingBeamStem",
    "shorterWrongSide",
    "linked",
    "edgesAdded",
    "linkerWrites",
    "events",
    "chordStemMatches",
    "stopBeforeHeadRelationLoop",
];
const SYNTHETIC_CASE_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "join",
    "sourceRealB15EvidenceSha256",
    "construction",
    "baseRuntimeClass",
    "siblingRuntimeClass",
    "stemAttachedBefore",
    "baseMedian",
    "siblingMedian",
    "stemMedian",
    "yDir",
    "sameGlyphIdentity",
    "pairState",
    "baseCross",
    "siblingCross",
    "baseLength",
    "siblingLength",
    "ratioRead",
    "ratio",
    "maxShorterRatio",
    "shorterInclusive",
    "dyRead",
    "dy",
    "product",
    "wrongSideStrict",
    "extension",
    "portion",
    "supportGrade",
    "branch",
    "builderItems",
    "lookupState",
    "siblingBLinkedBefore",
    "siblingBLinkedAfter",
    "writeCount",
    "valueChangeCount",
    "graphEdgesBefore",
    "graphEdgesAfter",
    "freshRelationIdentity",
    "callbackCompleted",
    "stemIncidentState",
    "stemIncidentRows",
    "chordStemMatches",
    "beamIncidentState",
    "beamRule",
    "beamAbnormalBefore",
    "beamAbnormalAfter",
    "dirtyBefore",
    "dirtyAfter",
    "throwClass",
    "throwStage",
    "eventCount",
    "terminal",
];
const SYNTHETIC_EVENT_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "eventOrdinal",
    "kind",
    "relationIdentity",
];
const SYNTHETIC_GUARD_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "graphDelta",
    "allowedMutations",
    "baseBeamUnchanged",
    "stemGeometryUnchanged",
    "groupObjectUnchanged",
    "zeroChordStem",
    "isolatedOnly",
    "productionEquivalent",
    "enclosingRealSheetUnchanged",
    "headRelationLoopRead",
    "terminal",
];
const CORPUS_SUMMARY_FIELDS: &[&str] = &[
    "schema",
    "mode",
    "pages",
    "pageRefs",
    "rowCounts",
    "pageInputSha256",
    "probeSourceSha256",
    "runnerSourceSha256",
    "effectiveClasspathSha256",
    "jdkReleaseSha256",
    "javaExecutableSha256",
    "javaJpegLibrarySha256",
    "javaModulesSha256",
    "javaVmLibrarySha256",
    "javaAwtLibrarySha256",
    "javaAwtLwawtLibrarySha256",
    "javaArchitecture",
    "javaRuntimeVersion",
    "javaVmVariant",
    "javaImageType",
    "beamLinkerClassSha256",
    "bLinkerClassSha256",
    "vLinkerClassSha256",
    "stemLinkerClassSha256",
    "beamLinkerSourceSha256",
    "stemLinkerSourceSha256",
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
    "smallBeamInterSourceSha256",
    "beamBeamRelationSourceSha256",
    "sheetStubSourceSha256",
    "bookSourceSha256",
    "horizontalSideSourceSha256",
    "verticalSideSourceSha256",
    "stemBuilderSourceSha256",
    "stemItemSourceSha256",
    "stemsRetrieverSourceSha256",
    "beamGroupInterSourceSha256",
    "ensembleHelperSourceSha256",
    "containmentSourceSha256",
    "abstractStemConnectionSourceSha256",
    "beamPortionSourceSha256",
    "scaleSourceSha256",
    "skewSourceSha256",
    "staffSourceSha256",
    "staffLineSourceSha256",
    "lineInfoSourceSha256",
    "lineUtilSourceSha256",
    "gradleSourceSha256",
    "jgraphtCoreVersion",
    "jgraphtCoreJarSha256",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "createStemFixtureSha256",
    "reuseCheckFixtureSha256",
    "baseApplyFixtureSha256",
    "baseApplyManifestSha256",
    "bLinkerFlagFixtureSha256",
    "bLinkerFlagManifestSha256",
    "predecessorReplay",
    "querySerialization",
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
    "concurrencyScope",
    "compilerJavaProcessReaped",
    "runtimeJavaProcessesReaped",
    "foregroundJavaProcessesOnly",
    "backgroundJavaProcessesStarted",
    "realTransactions",
    "supportedSyntheticCases",
    "envelopeCases",
    "totalTransactions",
    "siblingCandidates",
    "sameGlyph",
    "existingBeamStem",
    "shorterWrongSide",
    "linked",
    "edgesAdded",
    "linkerWrites",
    "chordStemMatches",
    "system1SyntheticBlock",
    "addEdgeReturnedFalseEvidence",
    "envelopeEvidenceScope",
    "stopBeforeHeadRelationLoop",
];
const MANIFEST_ENTRY_FIELDS: &[&str] = &[
    "ordinal",
    "page",
    "fixture",
    "rowCounts",
    "systems",
    "realTransactions",
    "supportedSyntheticCases",
    "envelopeCases",
    "totalTransactions",
    "groupRows",
    "groupGlyphRows",
    "nullGroupGlyphs",
    "siblingCandidates",
    "sameGlyph",
    "existingBeamStem",
    "shorterWrongSide",
    "linked",
    "edgesAdded",
    "linkerWrites",
    "events",
    "chordStemMatches",
    "pageInputSha256",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "createStemFixtureSha256",
    "reuseCheckFixtureSha256",
    "baseApplyFixtureSha256",
    "baseApplyManifestSha256",
    "bLinkerFlagFixtureSha256",
    "bLinkerFlagManifestSha256",
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
    "concurrencyScope",
    "freshJvmPerSystem",
    "compilerJavaProcessReaped",
    "runtimeJavaProcessesReaped",
    "foregroundJavaProcessesOnly",
    "backgroundJavaProcessesStarted",
    "system1SyntheticBlock",
    "addEdgeReturnedFalseEvidence",
    "envelopeEvidenceScope",
    "stopBeforeHeadRelationLoop",
];
const MANIFEST_SUMMARY_FIELDS: &[&str] = &[
    "schema",
    "entries",
    "probeSourceSha256",
    "runnerSourceSha256",
    "effectiveClasspathSha256",
    "jdkReleaseSha256",
    "javaExecutableSha256",
    "javaJpegLibrarySha256",
    "javaModulesSha256",
    "javaVmLibrarySha256",
    "javaAwtLibrarySha256",
    "javaAwtLwawtLibrarySha256",
    "javaArchitecture",
    "javaRuntimeVersion",
    "javaVmVariant",
    "javaImageType",
    "beamLinkerClassSha256",
    "bLinkerClassSha256",
    "vLinkerClassSha256",
    "stemLinkerClassSha256",
    "beamLinkerSourceSha256",
    "stemLinkerSourceSha256",
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
    "smallBeamInterSourceSha256",
    "beamBeamRelationSourceSha256",
    "sheetStubSourceSha256",
    "bookSourceSha256",
    "horizontalSideSourceSha256",
    "verticalSideSourceSha256",
    "stemBuilderSourceSha256",
    "stemItemSourceSha256",
    "stemsRetrieverSourceSha256",
    "beamGroupInterSourceSha256",
    "ensembleHelperSourceSha256",
    "containmentSourceSha256",
    "abstractStemConnectionSourceSha256",
    "beamPortionSourceSha256",
    "scaleSourceSha256",
    "skewSourceSha256",
    "staffSourceSha256",
    "staffLineSourceSha256",
    "lineInfoSourceSha256",
    "lineUtilSourceSha256",
    "gradleSourceSha256",
    "jgraphtCoreVersion",
    "jgraphtCoreJarSha256",
    "baseApplyManifestSha256",
    "bLinkerFlagManifestSha256",
    "predecessorReplay",
    "querySerialization",
    "sharedHeaderSha256",
    "sharedHeaderLines",
    "sharedHeaderBytes",
    "corpusBodySha256",
    "corpusBodyLines",
    "corpusBodyBytes",
    "corpusRowCounts",
    "corpusReconstruction",
    "semanticRows",
    "splitFixtureLines",
    "splitFixtureBytes",
    "realSystems",
    "realTransactions",
    "groupRows",
    "groupGlyphRows",
    "nullGroupGlyphs",
    "realSiblingCandidates",
    "realSameGlyph",
    "realExistingBeamStem",
    "realShorterWrongSide",
    "realLinked",
    "realEdgesAdded",
    "realLinkerWrites",
    "realEvents",
    "chordStemMatches",
    "syntheticBlocks",
    "supportedSyntheticCases",
    "envelopeCases",
    "isolatedCases",
    "totalTransactions",
    "sameGlyph",
    "existingBeamStem",
    "shorterWrongSide",
    "linked",
    "edgesAdded",
    "linkerWrites",
    "events",
    "compilerJavaProcesses",
    "runtimeJavaProcesses",
    "totalJavaProcesses",
    "maximumConcurrentJavaProcesses",
    "concurrencyScope",
    "freshRunsPerPage",
    "freshRunsByteIdentical",
    "freshJvmPerSystem",
    "compilerJavaProcessesReaped",
    "runtimeJavaProcessesReaped",
    "foregroundJavaProcessesOnly",
    "backgroundJavaProcessesStarted",
    "system1SyntheticBlock",
    "addEdgeReturnedFalseEvidence",
    "supplementalEvidenceScope",
    "stopBeforeHeadRelationLoop",
    "manifestBodySha256",
    "manifestBodyLines",
    "manifestBodyBytes",
];
const CORPUS_SOURCE_PINS: &[(&str, &str)] = &[
    (
        "beamLinkerSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/stem/BeamLinker.java",
    ),
    (
        "stemLinkerSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/stem/StemLinker.java",
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
        "smallBeamInterSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/inter/SmallBeamInter.java",
    ),
    (
        "beamBeamRelationSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/relation/BeamBeamRelation.java",
    ),
    (
        "sheetStubSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/SheetStub.java",
    ),
    (
        "bookSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/Book.java",
    ),
    (
        "horizontalSideSourceSha256",
        "app/src/main/java/org/audiveris/omr/util/HorizontalSide.java",
    ),
    (
        "verticalSideSourceSha256",
        "app/src/main/java/org/audiveris/omr/util/VerticalSide.java",
    ),
    (
        "stemBuilderSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/stem/StemBuilder.java",
    ),
    (
        "stemItemSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/stem/StemItem.java",
    ),
    (
        "stemsRetrieverSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/stem/StemsRetriever.java",
    ),
    (
        "beamGroupInterSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/inter/BeamGroupInter.java",
    ),
    (
        "ensembleHelperSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/inter/EnsembleHelper.java",
    ),
    (
        "containmentSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/relation/Containment.java",
    ),
    (
        "abstractStemConnectionSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/relation/AbstractStemConnection.java",
    ),
    (
        "beamPortionSourceSha256",
        "app/src/main/java/org/audiveris/omr/sig/relation/BeamPortion.java",
    ),
    (
        "scaleSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/Scale.java",
    ),
    (
        "skewSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/Skew.java",
    ),
    (
        "staffSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/Staff.java",
    ),
    (
        "staffLineSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/StaffLine.java",
    ),
    (
        "lineInfoSourceSha256",
        "app/src/main/java/org/audiveris/omr/sheet/grid/LineInfo.java",
    ),
    (
        "lineUtilSourceSha256",
        "app/src/main/java/org/audiveris/omr/math/LineUtil.java",
    ),
    ("gradleSourceSha256", "app/build.gradle"),
];
const CORPUS_GLOBAL_FIXTURE_PINS: &[(&str, &str)] = &[
    (
        "baseApplyManifestSha256",
        "rust/oracle/stems-beam-vlink-base-apply-manifest.txt",
    ),
    ("bLinkerFlagManifestSha256", BOUNDARY_FIFTEEN_MANIFEST_PATH),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RowKind {
    Page,
    Predecessor,
    Baseline,
    GroupMember,
    Sibling,
    SourceOutgoing,
    PairRelation,
    Geometry,
    Edge,
    StemIncident,
    BeamIncident,
    Callback,
    LinkerLookup,
    LinkerFlag,
    SiblingResult,
    Result,
    DeltaGuard,
    Summary,
    SyntheticCase,
    SyntheticEvent,
    SyntheticGuard,
    PageSummary,
    CorpusSummary,
}

impl RowKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Page => "stemsbeamvlinksiblinglinkspage",
            Self::Predecessor => "stemsbeamvlinksiblinglinkspredecessor",
            Self::Baseline => "stemsbeamvlinksiblinglinksbaseline",
            Self::GroupMember => "stemsbeamvlinksiblinglinksgroupmember",
            Self::Sibling => "stemsbeamvlinksiblinglinkssibling",
            Self::SourceOutgoing => "stemsbeamvlinksiblinglinkssourceoutgoing",
            Self::PairRelation => "stemsbeamvlinksiblinglinkspairrelation",
            Self::Geometry => "stemsbeamvlinksiblinglinksgeometry",
            Self::Edge => "stemsbeamvlinksiblinglinksedge",
            Self::StemIncident => "stemsbeamvlinksiblinglinksstemincident",
            Self::BeamIncident => "stemsbeamvlinksiblinglinksbeamincident",
            Self::Callback => "stemsbeamvlinksiblinglinkscallback",
            Self::LinkerLookup => "stemsbeamvlinksiblinglinkslinkerlookup",
            Self::LinkerFlag => "stemsbeamvlinksiblinglinkslinkerflag",
            Self::SiblingResult => "stemsbeamvlinksiblinglinkssiblingresult",
            Self::Result => "stemsbeamvlinksiblinglinksresult",
            Self::DeltaGuard => "stemsbeamvlinksiblinglinksdeltaguard",
            Self::Summary => "stemsbeamvlinksiblinglinkssummary",
            Self::SyntheticCase => "stemsbeamvlinksiblinglinkssyntheticcase",
            Self::SyntheticEvent => "stemsbeamvlinksiblinglinkssyntheticevent",
            Self::SyntheticGuard => "stemsbeamvlinksiblinglinkssyntheticguard",
            Self::PageSummary => "stemsbeamvlinksiblinglinkspagesummary",
            Self::CorpusSummary => "stemsbeamvlinksiblinglinkscorpussummary",
        }
    }

    fn parse(label: &str) -> Option<Self> {
        let suffix = label.strip_prefix("stemsbeamvlinksiblinglinks")?;
        Some(match suffix {
            "page" => Self::Page,
            "predecessor" => Self::Predecessor,
            "baseline" => Self::Baseline,
            "groupmember" => Self::GroupMember,
            "sibling" => Self::Sibling,
            "sourceoutgoing" => Self::SourceOutgoing,
            "pairrelation" => Self::PairRelation,
            "geometry" => Self::Geometry,
            "edge" => Self::Edge,
            "stemincident" => Self::StemIncident,
            "beamincident" => Self::BeamIncident,
            "callback" => Self::Callback,
            "linkerlookup" => Self::LinkerLookup,
            "linkerflag" => Self::LinkerFlag,
            "siblingresult" => Self::SiblingResult,
            "result" => Self::Result,
            "deltaguard" => Self::DeltaGuard,
            "summary" => Self::Summary,
            "syntheticcase" => Self::SyntheticCase,
            "syntheticevent" => Self::SyntheticEvent,
            "syntheticguard" => Self::SyntheticGuard,
            "pagesummary" => Self::PageSummary,
            "corpussummary" => Self::CorpusSummary,
            _ => return None,
        })
    }

    const fn expected_fields(self) -> &'static [&'static str] {
        match self {
            Self::Page => PAGE_FIELDS,
            Self::Predecessor => PREDECESSOR_FIELDS,
            Self::Baseline => BASELINE_FIELDS,
            Self::GroupMember => GROUP_MEMBER_FIELDS,
            Self::Sibling => SIBLING_FIELDS,
            Self::SourceOutgoing => SOURCE_OUTGOING_FIELDS,
            Self::PairRelation => PAIR_RELATION_FIELDS,
            Self::Geometry => GEOMETRY_FIELDS,
            Self::Edge => EDGE_FIELDS,
            Self::StemIncident => STEM_INCIDENT_FIELDS,
            Self::BeamIncident => BEAM_INCIDENT_FIELDS,
            Self::Callback => CALLBACK_FIELDS,
            Self::LinkerLookup => LINKER_LOOKUP_FIELDS,
            Self::LinkerFlag => LINKER_FLAG_FIELDS,
            Self::SiblingResult => SIBLING_RESULT_FIELDS,
            Self::Result => RESULT_FIELDS,
            Self::DeltaGuard => DELTA_GUARD_FIELDS,
            Self::Summary => SUMMARY_FIELDS,
            Self::SyntheticCase => SYNTHETIC_CASE_FIELDS,
            Self::SyntheticEvent => SYNTHETIC_EVENT_FIELDS,
            Self::SyntheticGuard => SYNTHETIC_GUARD_FIELDS,
            Self::PageSummary => PAGE_SUMMARY_FIELDS,
            Self::CorpusSummary => CORPUS_SUMMARY_FIELDS,
        }
    }
}

const CORE_ROW_KINDS: [RowKind; 21] = [
    RowKind::Page,
    RowKind::Predecessor,
    RowKind::Baseline,
    RowKind::GroupMember,
    RowKind::Sibling,
    RowKind::SourceOutgoing,
    RowKind::PairRelation,
    RowKind::Geometry,
    RowKind::Edge,
    RowKind::StemIncident,
    RowKind::BeamIncident,
    RowKind::Callback,
    RowKind::LinkerLookup,
    RowKind::LinkerFlag,
    RowKind::SiblingResult,
    RowKind::Result,
    RowKind::DeltaGuard,
    RowKind::Summary,
    RowKind::SyntheticCase,
    RowKind::SyntheticEvent,
    RowKind::SyntheticGuard,
];

fn core_row_counts(rows: &[StrictRow]) -> String {
    CORE_ROW_KINDS
        .iter()
        .map(|kind| {
            format!(
                "{}:{}",
                kind.label(),
                rows.iter().filter(|row| row.kind == *kind).count()
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StrictRow {
    kind: RowKind,
    page: String,
    fields: Vec<(String, String)>,
}

impl StrictRow {
    fn parse(line: &str) -> Result<Self, String> {
        let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
        let label = tokens.first().ok_or_else(|| "row is empty".to_owned())?;
        let kind = RowKind::parse(label).ok_or_else(|| format!("unknown row label {label}"))?;
        let (page, field_tokens) = if kind == RowKind::CorpusSummary {
            ("", &tokens[1..])
        } else {
            (
                *tokens
                    .get(1)
                    .ok_or_else(|| "row lacks its implicit page token".to_owned())?,
                &tokens[2..],
            )
        };
        if field_tokens.len() % 2 != 0 {
            return Err("row does not contain ordered key/value pairs".to_owned());
        }
        let mut names = BTreeSet::new();
        let mut fields = Vec::with_capacity(field_tokens.len() / 2);
        for pair in field_tokens.chunks_exact(2) {
            if !names.insert(pair[0]) {
                return Err(format!("duplicate field {}", pair[0]));
            }
            fields.push((pair[0].to_owned(), pair[1].to_owned()));
        }
        let actual = fields
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>();
        if actual != kind.expected_fields() {
            return Err(format!(
                "wrong field order for {kind:?}: expected {:?}, got {actual:?}",
                kind.expected_fields()
            ));
        }
        Ok(Self {
            kind,
            page: page.to_owned(),
            fields,
        })
    }

    fn value(&self, name: &str) -> Result<&str, String> {
        self.fields
            .iter()
            .find_map(|(field, value)| (field == name).then_some(value.as_str()))
            .ok_or_else(|| format!("missing {name} in {:?}", self.kind))
    }

    fn usize(&self, name: &str) -> Result<usize, String> {
        self.value(name)?
            .parse()
            .map_err(|error| format!("invalid {name} in {:?}: {error}", self.kind))
    }

    fn i64(&self, name: &str) -> Result<i64, String> {
        self.value(name)?
            .parse()
            .map_err(|error| format!("invalid {name} in {:?}: {error}", self.kind))
    }

    fn bool(&self, name: &str) -> Result<bool, String> {
        match self.value(name)? {
            "true" => Ok(true),
            "false" => Ok(false),
            value => Err(format!("invalid Boolean {name}={value} in {:?}", self.kind)),
        }
    }

    fn key(&self) -> Result<TransactionKey, String> {
        if matches!(
            self.kind,
            RowKind::Page | RowKind::PageSummary | RowKind::CorpusSummary
        ) {
            return Err("page row has no transaction key".to_owned());
        }
        let actual = self
            .fields
            .iter()
            .take(COMMON_FIELDS.len())
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>();
        if actual != COMMON_FIELDS {
            return Err(format!("transaction prefix differs in {:?}", self.kind));
        }
        Ok(TransactionKey {
            page: self.page.clone(),
            system: self.usize("system")?,
            plan: self.usize("plan")?,
            scope: self.value("scope")?.to_owned(),
            case_name: self.value("case")?.to_owned(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ManifestRow {
    label: String,
    fields: Vec<(String, String)>,
}

impl ManifestRow {
    fn parse(line: &str, expected_label: &str, expected_fields: &[&str]) -> Result<Self, String> {
        let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
        let label = tokens
            .first()
            .ok_or_else(|| "manifest row is empty".to_owned())?;
        if *label != expected_label {
            return Err(format!(
                "manifest row label differs: expected {expected_label}, got {label}"
            ));
        }
        let field_tokens = &tokens[1..];
        if field_tokens.len() % 2 != 0 {
            return Err("manifest row does not contain ordered key/value pairs".to_owned());
        }
        let fields = field_tokens
            .chunks_exact(2)
            .map(|pair| (pair[0].to_owned(), pair[1].to_owned()))
            .collect::<Vec<_>>();
        let actual = fields
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>();
        if actual != expected_fields {
            return Err(format!(
                "wrong field order for {expected_label}: expected {expected_fields:?}, got {actual:?}"
            ));
        }
        let unique = actual.iter().copied().collect::<BTreeSet<_>>();
        if unique.len() != actual.len() {
            return Err(format!("duplicate field in {expected_label}"));
        }
        Ok(Self {
            label: (*label).to_owned(),
            fields,
        })
    }

    fn value(&self, name: &str) -> Result<&str, String> {
        self.fields
            .iter()
            .find_map(|(field, value)| (field == name).then_some(value.as_str()))
            .ok_or_else(|| format!("missing {name} in {}", self.label))
    }

    fn usize(&self, name: &str) -> Result<usize, String> {
        self.value(name)?
            .parse()
            .map_err(|error| format!("invalid {name} in {}: {error}", self.label))
    }

    fn bool(&self, name: &str) -> Result<bool, String> {
        match self.value(name)? {
            "true" => Ok(true),
            "false" => Ok(false),
            value => Err(format!("invalid Boolean {name}={value} in {}", self.label)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TransactionKey {
    page: String,
    system: usize,
    plan: usize,
    scope: String,
    case_name: String,
}

fn parse_scaffold_fixture(text: &str) -> Result<Vec<StrictRow>, String> {
    if !text.as_bytes().ends_with(b"\n") {
        return Err("fixture must end with one newline".to_owned());
    }
    let lines = text.lines().collect::<Vec<_>>();
    if lines.len() <= FIXTURE_HEADER.len() || &lines[..FIXTURE_HEADER.len()] != FIXTURE_HEADER {
        return Err("fixture header differs".to_owned());
    }
    lines[FIXTURE_HEADER.len()..]
        .iter()
        .enumerate()
        .map(|(index, line)| {
            if line.is_empty() || line.starts_with('#') {
                return Err(format!(
                    "unexpected blank/comment at semantic line {}",
                    index + FIXTURE_HEADER.len() + 1
                ));
            }
            StrictRow::parse(line)
                .map_err(|error| format!("line {}: {error}", index + FIXTURE_HEADER.len() + 1))
        })
        .collect()
}

fn java_printf_fields(source: &str, kind: RowKind) -> Result<Vec<String>, String> {
    if kind == RowKind::PageSummary {
        return Err("page summary is emitted by the runner, not Java".to_owned());
    }
    let marker = format!("\"{} ", kind.label());
    let start = source
        .find(&marker)
        .ok_or_else(|| format!("Java probe lacks {:?} printf", kind))?;
    let mut format_text = String::new();
    for line in source[start..].lines() {
        let Some(first_quote) = line.find('"') else {
            continue;
        };
        let last_quote = line
            .rfind('"')
            .ok_or_else(|| format!("unterminated Java format literal for {kind:?}"))?;
        if last_quote == first_quote {
            return Err(format!("empty Java format literal for {kind:?}"));
        }
        format_text.push_str(&line[first_quote + 1..last_quote]);
        if line[first_quote + 1..last_quote].contains("%n") {
            break;
        }
    }
    let format_text = format_text.replace("%n", "");
    let tokens = format_text.split_ascii_whitespace().collect::<Vec<_>>();
    if tokens.first().copied() != Some(kind.label()) || tokens.get(1).copied() != Some("%s") {
        return Err(format!("Java {:?} format lacks label/page prefix", kind));
    }
    if (tokens.len() - 2) % 2 != 0 {
        return Err(format!("Java {:?} format is not key/value shaped", kind));
    }
    let mut fields = if kind == RowKind::Page {
        Vec::new()
    } else {
        COMMON_FIELDS
            .iter()
            .map(|field| (*field).to_owned())
            .collect()
    };
    fields.extend(tokens[2..].chunks_exact(2).map(|pair| pair[0].to_owned()));
    Ok(fields)
}

fn runner_corpus_printf_fields(source: &str) -> Result<Vec<String>, String> {
    let prefix = format!("printf '{} ", RowKind::CorpusSummary.label());
    let line = source
        .lines()
        .find(|line| line.starts_with(&prefix))
        .ok_or_else(|| "runner lacks corpus-summary printf".to_owned())?;
    let first_quote = line
        .find('\'')
        .ok_or_else(|| "runner corpus printf lacks opening quote".to_owned())?;
    let last_quote = line
        .rfind('\'')
        .filter(|last| *last > first_quote)
        .ok_or_else(|| "runner corpus printf lacks closing quote".to_owned())?;
    let tokens = line[first_quote + 1..last_quote]
        .split_ascii_whitespace()
        .collect::<Vec<_>>();
    if tokens.first().copied() != Some(RowKind::CorpusSummary.label())
        || (tokens.len() - 1) % 2 != 0
    {
        return Err("runner corpus printf is not key/value shaped".to_owned());
    }
    let mut fields = tokens[1..]
        .chunks_exact(2)
        .map(|pair| pair[0].to_owned())
        .collect::<Vec<_>>();
    let insertion = fields
        .iter()
        .position(|field| field == "stemLinkerClassSha256")
        .ok_or_else(|| "runner corpus printf lacks class/source boundary".to_owned())?
        + 1;
    fields.splice(
        insertion..insertion,
        CORPUS_SOURCE_PINS
            .iter()
            .map(|(field, _)| (*field).to_owned()),
    );
    Ok(fields)
}

struct RowCursor<'a> {
    rows: &'a [StrictRow],
    index: usize,
}

impl<'a> RowCursor<'a> {
    fn new(rows: &'a [StrictRow], index: usize) -> Self {
        Self { rows, index }
    }

    fn peek_kind(&self) -> Option<RowKind> {
        self.rows.get(self.index).map(|row| row.kind)
    }

    fn take(&mut self, kind: RowKind, key: &TransactionKey) -> Result<&'a StrictRow, String> {
        let row = self
            .rows
            .get(self.index)
            .ok_or_else(|| format!("expected {kind:?}, reached end of fixture"))?;
        if row.kind != kind {
            return Err(format!(
                "expected {kind:?} at semantic row {}, found {:?}",
                self.index, row.kind
            ));
        }
        if &row.key()? != key {
            return Err(format!(
                "transaction key drift at semantic row {}",
                self.index
            ));
        }
        self.index += 1;
        Ok(row)
    }
}

#[derive(Clone, Debug)]
struct ParsedTransaction {
    key: TransactionKey,
    predecessor: StrictRow,
    rows: Vec<StrictRow>,
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_rows(rows: impl IntoIterator<Item = String>) -> String {
    let mut bytes = Vec::new();
    for row in rows {
        bytes.extend_from_slice(row.as_bytes());
        bytes.push(b'\n');
    }
    sha256_hex(&bytes)
}

fn joined_row_token(row: &StrictRow, fields: &[&str]) -> Result<String, String> {
    fields
        .iter()
        .map(|field| row.value(field))
        .collect::<Result<Vec<_>, _>>()
        .map(|values| values.join(":"))
}

fn require_ordinal(row: &StrictRow, field: &str, expected: usize) -> Result<(), String> {
    if row.usize(field)? != expected {
        return Err(format!(
            "{field} chronology differs in {:?}: expected {expected}",
            row.kind
        ));
    }
    Ok(())
}

fn validate_page_row(row: &StrictRow) -> Result<(), String> {
    if row.kind != RowKind::Page
        || row.usize("systems")? == 0
        || row.value("executionMode")? != "foregroundJvmPerSystem"
        || row.value("groupOrder")? != "OutgoingContainmentThenStableTopDown"
        || row.value("incidentOrder")? != "IncomingThenOutgoing"
        || !row.bool("headless")?
        || row.value("methodDispatch")? != "ExactPrivateBeamLinkerVLinkerLinkSiblings"
        || row.value("stop")? != "ReadyBeforeHeadRelationLoop"
    {
        return Err("page execution envelope differs".to_owned());
    }
    for field in [
        "schedulerFixtureSha256",
        "expandFixtureSha256",
        "createStemFixtureSha256",
        "reuseCheckFixtureSha256",
        "baseApplyFixtureSha256",
        "bLinkerFlagFixtureSha256",
    ] {
        if !is_lower_sha256(row.value(field)?) {
            return Err(format!("page {field} is not lowercase SHA-256"));
        }
    }
    Ok(())
}

fn validate_predecessor_row(row: &StrictRow) -> Result<(), String> {
    if row.value("join")? != "FullBoundary15Replay"
        || row.usize("b15TransactionRows")? == 0
        || !is_lower_sha256(row.value("b15TransactionEvidenceSha256")?)
        || !is_lower_sha256(row.value("b15ResultRowSha256")?)
        || !is_lower_sha256(row.value("b15GuardRowSha256")?)
        || !is_lower_sha256(row.value("b15SummaryRowSha256")?)
        || row.value("predecessorTerminal")? != "ReadyBeforeSiblingBeamLinks"
        || !row.bool("targetBLinked")?
        || row.value("stemAlias")?.is_empty()
        || row.usize("stemInterId")? == 0
        || row.value("baseBeamAlias")?.is_empty()
        || row.value("targetBAlias")?.is_empty()
        || row.value("triggeringVAlias")?.is_empty()
    {
        return Err("Boundary-15 predecessor row differs".to_owned());
    }
    parse_hex_bits(row.value("supportGrade")?)?;
    Ok(())
}

fn parse_hex_bits(value: &str) -> Result<u64, String> {
    let (java_hex, raw) = value
        .split_once('/')
        .ok_or_else(|| format!("missing raw bits in {value}"))?;
    if java_hex.is_empty() || raw.len() != 16 {
        return Err(format!("invalid Java hex/raw token {value}"));
    }
    let bits = u64::from_str_radix(raw, 16)
        .map_err(|error| format!("invalid raw f64 bits in {value}: {error}"))?;
    if !f64::from_bits(bits).is_finite() {
        return Err(format!("non-finite compact f64 token {value}"));
    }
    Ok(bits)
}

fn parse_f64(value: &str) -> Result<f64, String> {
    Ok(f64::from_bits(parse_hex_bits(value)?))
}

fn parse_point(value: &str) -> Result<NativeStemPoint, String> {
    let values = value.split(':').collect::<Vec<_>>();
    let [x, y] = values.as_slice() else {
        return Err(format!("invalid point token {value}"));
    };
    Ok(NativeStemPoint {
        x: parse_f64(x)?,
        y: parse_f64(y)?,
    })
}

fn parse_segment(value: &str) -> Result<Segment, String> {
    let values = value.split(':').collect::<Vec<_>>();
    let [x1, y1, x2, y2] = values.as_slice() else {
        return Err(format!("invalid segment token {value}"));
    };
    Ok(Segment {
        x1: parse_f64(x1)?,
        y1: parse_f64(y1)?,
        x2: parse_f64(x2)?,
        y2: parse_f64(y2)?,
    })
}

fn parse_i32_field(row: &StrictRow, field: &str) -> Result<i32, String> {
    row.value(field)?
        .parse()
        .map_err(|error| format!("invalid {field} in {:?}: {error}", row.kind))
}

fn parse_optional_usize(value: &str) -> Result<Option<usize>, String> {
    if value == "-" {
        Ok(None)
    } else {
        value
            .parse()
            .map(Some)
            .map_err(|error| format!("invalid optional usize {value}: {error}"))
    }
}

fn parse_optional_i32(value: &str) -> Result<Option<i32>, String> {
    if value == "-" {
        Ok(None)
    } else {
        value
            .parse()
            .map(Some)
            .map_err(|error| format!("invalid optional i32 {value}: {error}"))
    }
}

fn parse_optional_bool(value: &str) -> Result<Option<bool>, String> {
    match value {
        "-" => Ok(None),
        "true" => Ok(Some(true)),
        "false" => Ok(Some(false)),
        _ => Err(format!("invalid optional Boolean {value}")),
    }
}

include!("common/b16_hydration.rs");

fn validate_baseline_row(row: &StrictRow, predecessor: &StrictRow) -> Result<(), String> {
    let y_dir = row
        .value("yDir")?
        .parse::<i32>()
        .map_err(|error| format!("invalid yDir: {error}"))?;
    if row.value("baseBeamAlias")? != predecessor.value("baseBeamAlias")?
        || row.value("stemAlias")? != predecessor.value("stemAlias")?
        || row.value("stemInterId")? != predecessor.value("stemInterId")?
        || row.value("supportGrade")? != predecessor.value("supportGrade")?
        || !matches!(
            row.value("baseBeamClass")?,
            "org.audiveris.omr.sig.inter.BeamInter"
                | "org.audiveris.omr.sig.inter.BeamHookInter"
                | "org.audiveris.omr.sig.inter.SmallBeamInter"
        )
        || row.value("groupClass")? != "org.audiveris.omr.sig.inter.BeamGroupInter"
        || !row.bool("cachedMedianSameIdentity")?
        || !row.bool("baseRemoved")?
        || row.value("supportGradeSource")? != "FreshBaseBeamStemDraft"
        || !row.bool("supportGradeRead")?
        || !row.bool("soleSigListener")?
        || row.usize("chordStemMatches")? != 0
        || !matches!(y_dir, -1 | 1)
        || row.usize("groupMembers")? < row.usize("siblings")? + 1
        || row.usize("selectedBeforeBaseRemoval")? != row.usize("siblings")? + 1
        || row.usize("groupOutgoingScanned")? < row.usize("groupMembers")?
        || row.usize("stemIncidentRows")? == 0
        || row.usize("builderItems")? == 0
    {
        return Err("baseline join/domain differs".to_owned());
    }
    for field in [
        "groupStateHashBefore",
        "groupObjectStateHash",
        "groupQueryProvenanceSha256",
        "graphVertexHashBefore",
        "graphEdgeHashBefore",
        "listenerTopologyHash",
        "stemIncidentHash",
        "builderItemsHash",
        "arenaStateHashBefore",
        "relationInputHashBefore",
    ] {
        if !is_lower_sha256(row.value(field)?) {
            return Err(format!("baseline {field} is not lowercase SHA-256"));
        }
    }
    for field in [
        "baseBeamHeight",
        "maxShorterRatio",
        "xInGapMaximum0",
        "baseLength",
        "supportGrade",
    ] {
        parse_hex_bits(row.value(field)?)?;
    }
    Ok(())
}

fn validate_group_rows(baseline: &StrictRow, rows: &[&StrictRow]) -> Result<(), String> {
    if rows.len() != baseline.usize("groupOutgoingScanned")? {
        return Err("group outgoing row count differs".to_owned());
    }
    let mut member_ordinal = 0;
    let mut selected = 0;
    let mut base = 0;
    for (ordinal, row) in rows.iter().enumerate() {
        require_ordinal(row, "groupOutgoingOrdinal", ordinal)?;
        let containment = row.bool("containmentMatch")?;
        if containment {
            require_ordinal(row, "memberOrdinal", member_ordinal)?;
            member_ordinal += 1;
            if row.value("targetReadByGetMembers")? != "true"
                || row.value("targetEvidence")? != "GetMembersRead"
                || row.value("beamRuntimeClass")? != row.value("targetRuntimeClass")?
                || row.value("beamInterId")? != row.value("targetInterId")?
                || row.value("sigMembership")? != "true"
                || row.value("beamRemoved")? != "false"
            {
                return Err("containment/member live projection differs".to_owned());
            }
            for field in ["beamGroupStateHash", "beamGroupObjectStateHash"] {
                if !is_lower_sha256(row.value(field)?) {
                    return Err(format!("group member {field} is not lowercase SHA-256"));
                }
            }
            selected += usize::from(row.bool("selected")?);
            if row.bool("baseIdentity")? {
                base += 1;
                if row.value("removeAction")? != "RemoveFirstBase" || !row.bool("selected")? {
                    return Err("base group member removal differs".to_owned());
                }
            } else if row.value("removeAction")? != "Retain" {
                return Err("non-base group member action differs".to_owned());
            }
        } else if row.value("memberOrdinal")? != "-"
            || row.value("targetReadByGetMembers")? != "false"
            || row.value("targetEvidence")? != "GraphReconstruction"
            || row.value("beamRuntimeClass")? != "-"
            || row.value("selected")? != "-"
        {
            return Err("non-containment group reconstruction differs".to_owned());
        }
    }
    if member_ordinal != baseline.usize("groupMembers")?
        || selected != baseline.usize("selectedBeforeBaseRemoval")?
        || base != 1
    {
        return Err("group member/select/base census differs".to_owned());
    }
    let hash = sha256_rows(rows.iter().map(|row| {
        joined_row_token(
            row,
            &[
                "groupOutgoingOrdinal",
                "graphRelationIdentity",
                "relationObjectIdentity",
                "relationClass",
                "targetAlias",
                "targetRuntimeClass",
                "targetInterId",
                "targetVertexOrdinal",
                "containmentMatch",
                "memberOrdinal",
            ],
        )
        .expect("validated group row fields")
    }));
    if hash != baseline.value("groupQueryProvenanceSha256")? {
        return Err("group query provenance hash differs".to_owned());
    }
    Ok(())
}

fn validate_source_rows(sibling: &StrictRow, rows: &[&StrictRow]) -> Result<(), String> {
    if rows.len() != sibling.usize("sourceOutgoingScanned")? {
        return Err("source-outgoing row count differs".to_owned());
    }
    for (ordinal, row) in rows.iter().enumerate() {
        require_ordinal(row, "sourceOutgoingOrdinal", ordinal)?;
    }
    let expected = sibling.value("sourceOutgoingProvenanceSha256")?;
    if rows.is_empty() && sibling.value("pairState")? == "NotRead" {
        if expected != "NotRead" {
            return Err("non-read source-outgoing provenance is not literal NotRead".to_owned());
        }
    } else {
        let hash = sha256_rows(rows.iter().map(|row| {
            joined_row_token(
                row,
                &[
                    "sourceOutgoingOrdinal",
                    "graphRelationIdentity",
                    "relationObjectIdentity",
                    "runtimeClass",
                ],
            )
            .expect("validated source-outgoing fields")
        }));
        if hash != expected {
            return Err("source-outgoing provenance hash differs".to_owned());
        }
    }
    Ok(())
}

fn validate_pair_rows(sibling: &StrictRow, rows: &[&StrictRow]) -> Result<(), String> {
    if rows.len() != sibling.usize("pairRows")? {
        return Err("directed-pair row count differs".to_owned());
    }
    let mut first_match = None;
    for (ordinal, row) in rows.iter().enumerate() {
        require_ordinal(row, "pairOrdinal", ordinal)?;
        let matches = row.bool("matches")?;
        let class_read = row.bool("classRead")?;
        match (first_match, class_read, matches, row.value("action")?) {
            (None, true, false, "Continue") => {}
            (None, true, true, "SelectBreak") => first_match = Some(ordinal),
            (Some(_), false, false, "UnreadAfterBreak") => {}
            _ => return Err("directed-pair lazy class-read trace differs".to_owned()),
        }
    }
    let expected = sibling.value("pairProvenanceSha256")?;
    if rows.is_empty() && sibling.value("pairState")? == "NotRead" {
        if expected != "NotRead" {
            return Err("non-read pair provenance is not literal NotRead".to_owned());
        }
    } else {
        let hash = sha256_rows(rows.iter().map(|row| {
            joined_row_token(
                row,
                &[
                    "pairOrdinal",
                    "sourceOutgoingOrdinal",
                    "graphRelationIdentity",
                    "relationObjectIdentity",
                    "runtimeClass",
                ],
            )
            .expect("validated pair fields")
        }));
        if hash != expected {
            return Err("directed-pair provenance hash differs".to_owned());
        }
    }
    let first_identity = first_match.map_or("-", |ordinal| {
        rows[ordinal]
            .value("graphRelationIdentity")
            .expect("validated graph identity")
    });
    if sibling.value("firstBeamStemIdentity")? != first_identity {
        return Err("first directed BeamStem identity differs".to_owned());
    }
    Ok(())
}

fn validate_geometry_row(
    geometry: &StrictRow,
    sibling: &StrictRow,
    baseline: &StrictRow,
) -> Result<(), String> {
    let branch = sibling.value("branch")?;
    if geometry.value("branch")? != branch
        || geometry.value("stemMedian")? != baseline.value("stemMedian")?
        || geometry.value("baseMedian")? != baseline.value("baseBeamMedian")?
        || geometry.value("baseCross")? != baseline.value("baseCross")?
        || geometry.value("baseLength")? != baseline.value("baseLength")?
        || geometry.value("maxShorterRatio")? != baseline.value("maxShorterRatio")?
        || geometry.value("yDir")? != baseline.value("yDir")?
        || geometry.value("xInGapMaximum0")? != baseline.value("xInGapMaximum0")?
        || geometry.value("maxDxRint")? != baseline.value("maxDxRint")?
    {
        return Err("sibling geometry/baseline join differs".to_owned());
    }
    for field in ["baseLength", "siblingLength", "ratio", "maxShorterRatio"] {
        parse_hex_bits(geometry.value(field)?)?;
    }
    let shorter = geometry.bool("shorterInclusive")?;
    if geometry.bool("dyRead")? != shorter
        || (shorter
            && (geometry.value("dy")? == "-"
                || geometry.value("product")? == "-"
                || geometry.value("wrongSideStrict")? == "-"))
        || (!shorter
            && (geometry.value("dy")? != "-"
                || geometry.value("product")? != "-"
                || geometry.value("wrongSideStrict")? != "-"))
    {
        return Err("lazy shorter-geometry read trace differs".to_owned());
    }
    if shorter {
        parse_hex_bits(geometry.value("dy")?)?;
        parse_hex_bits(geometry.value("product")?)?;
    }
    match branch {
        "ShorterWrongSide"
            if shorter
                && geometry.bool("wrongSideStrict")?
                && geometry.value("extension")? == "-"
                && geometry.value("beamHalfHeight")? == "-"
                && geometry.value("leftThreshold")? == "-"
                && geometry.value("rightThreshold")? == "-"
                && geometry.value("strictLeft")? == "-"
                && geometry.value("strictRight")? == "-"
                && geometry.value("portion")? == "-"
                && geometry.value("gradeSource")? == "NotRead"
                && geometry.value("gradeRead")? == "false"
                && geometry.value("grade")? == "-" => {}
        "Linked"
            if (!shorter || !geometry.bool("wrongSideStrict")?)
                && geometry.value("extension")? != "-"
                && geometry.value("beamHalfHeight")? != "-"
                && geometry.value("leftThreshold")? != "-"
                && geometry.value("rightThreshold")? != "-"
                && matches!(geometry.value("portion")?, "LEFT" | "CENTER" | "RIGHT")
                && geometry.value("gradeSource")? == "FreshBaseBeamStemDraft"
                && geometry.bool("gradeRead")?
                && geometry.value("grade")? == baseline.value("supportGrade")? =>
        {
            for field in ["beamHalfHeight", "leftThreshold", "rightThreshold", "grade"] {
                parse_hex_bits(geometry.value(field)?)?;
            }
            geometry.bool("strictLeft")?;
            geometry.bool("strictRight")?;
        }
        _ => return Err("branch-specific geometry laziness differs".to_owned()),
    }
    Ok(())
}

fn validate_edge_row(
    edge: &StrictRow,
    sibling: &StrictRow,
    predecessor: &StrictRow,
    geometry: &StrictRow,
) -> Result<(), String> {
    let expected_draft = format!(
        "sibling-draft:{}:{}",
        predecessor.value("plan")?,
        sibling.value("siblingOrdinal")?
    );
    if edge.value("freshRelationIdentity")? != expected_draft
        || edge.value("relationClass")? != "org.audiveris.omr.sig.relation.BeamStemRelation"
        || edge.value("sourceAlias")? != sibling.value("beamAlias")?
        || edge.value("sourceInterId")? != sibling.value("beamInterId")?
        || edge.value("targetAlias")? != predecessor.value("stemAlias")?
        || edge.value("targetInterId")? != predecessor.value("stemInterId")?
        || !edge.bool("insertionReturn")?
        || edge.bool("returnObservedByCaller")?
        || edge.value("graphRelationIdentity")?
            != format!("sig-edge:{}", edge.value("graphInsertionOrdinal")?)
        || edge.value("extension")? != geometry.value("extension")?
        || edge.value("portion")? != geometry.value("portion")?
        || edge.value("grade")? != predecessor.value("supportGrade")?
    {
        return Err("fresh sibling edge payload differs".to_owned());
    }
    parse_hex_bits(edge.value("grade")?)?;
    Ok(())
}

fn validate_incident_chronology(rows: &[&StrictRow]) -> Result<(), String> {
    let mut incoming = 0;
    let mut outgoing = 0;
    let mut saw_outgoing = false;
    for (incident, row) in rows.iter().enumerate() {
        require_ordinal(row, "incidentOrdinal", incident)?;
        match row.value("direction")? {
            "Incoming" if !saw_outgoing => {
                require_ordinal(row, "directionOrdinal", incoming)?;
                incoming += 1;
            }
            "Outgoing" => {
                saw_outgoing = true;
                require_ordinal(row, "directionOrdinal", outgoing)?;
                outgoing += 1;
            }
            _ => return Err("incident incoming/outgoing chronology differs".to_owned()),
        }
    }
    Ok(())
}

fn validate_callback_rows(
    callback: &StrictRow,
    edge: &StrictRow,
    stem_rows: &[&StrictRow],
    beam_rows: &[&StrictRow],
) -> Result<(), String> {
    validate_incident_chronology(stem_rows)?;
    validate_incident_chronology(beam_rows)?;
    let callback_event = callback.usize("eventOrdinal")?;
    if callback_event != edge.usize("eventOrdinal")? + 1
        || stem_rows
            .iter()
            .chain(beam_rows)
            .any(|row| row.usize("eventOrdinal") != Ok(callback_event))
        || !callback.bool("relationAdded")?
        || !callback.bool("extensionPrepopulated")?
        || !callback.bool("portionPrepopulated")?
        || callback.bool("defaultExtensionBranchRead")?
        || callback.bool("defaultPortionBranchRead")?
        || callback.usize("stemIncidentRows")? != stem_rows.len()
        || callback.usize("beamIncidentRows")? != beam_rows.len()
        || callback.usize("chordStemMatches")? != 0
    {
        return Err("callback envelope/count/event chronology differs".to_owned());
    }
    for row in stem_rows {
        let chord = row.bool("chordStemMatch")?;
        if !row.bool("classRead")?
            || row.bool("oppositeReadByGetChords")? != chord
            || row.value("oppositeEvidence")?
                != if chord {
                    "GetChordsRead"
                } else {
                    "GraphReconstruction"
                }
        {
            return Err("stem callback read/evidence trace differs".to_owned());
        }
    }
    for row in beam_rows {
        let class_read = row.value("readState")? != "UnreadAfterBreak";
        if row.bool("classRead")? != class_read
            || row.bool("oppositeReadByCheckAbnormal")?
            || row.value("oppositeEvidence")? != "GraphReconstruction"
        {
            return Err("beam callback read/evidence trace differs".to_owned());
        }
    }
    let stem_hash = sha256_rows(stem_rows.iter().map(|row| {
        joined_row_token(
            row,
            &[
                "incidentOrdinal",
                "direction",
                "directionOrdinal",
                "graphRelationIdentity",
                "relationObjectIdentity",
                "runtimeClass",
                "oppositeAlias",
                "oppositeInterId",
                "oppositeVertexOrdinal",
                "chordStemMatch",
            ],
        )
        .expect("validated stem incident fields")
    }));
    let beam_hash = sha256_rows(beam_rows.iter().map(|row| {
        joined_row_token(
            row,
            &[
                "incidentOrdinal",
                "direction",
                "directionOrdinal",
                "graphRelationIdentity",
                "relationObjectIdentity",
                "runtimeClass",
                "oppositeAlias",
                "oppositeInterId",
                "oppositeVertexOrdinal",
                "readState",
                "relevant",
                "portion",
                "contribution",
            ],
        )
        .expect("validated beam incident fields")
    }));
    if callback.value("stemIncidentHash")? != stem_hash
        || callback.value("beamIncidentHash")? != beam_hash
    {
        return Err("callback incident provenance hash differs".to_owned());
    }
    Ok(())
}

fn validate_lookup_rows(result: &StrictRow, rows: &[&StrictRow]) -> Result<(), String> {
    if result.usize("linkerLookupRows")? != rows.len() {
        return Err("builder lookup row count differs".to_owned());
    }
    let mut selected = false;
    for (ordinal, row) in rows.iter().enumerate() {
        require_ordinal(row, "itemOrdinal", ordinal)?;
        match (selected, row.value("action")?) {
            (false, "Continue") => {}
            (false, "SelectBreak") => selected = true,
            (true, "UnreadAfterBreak") => {}
            _ => return Err("builder first-source-identity scan differs".to_owned()),
        }
    }
    let hash = sha256_rows(rows.iter().map(|row| {
        joined_row_token(
            row,
            &[
                "itemOrdinal",
                "runtimeClass",
                "linkerRead",
                "sourceRead",
                "linkerAlias",
                "linkerRuntimeClass",
                "sourceAlias",
                "sourceInterId",
                "identityMatch",
                "action",
            ],
        )
        .expect("validated builder lookup fields")
    }));
    if result.value("linkerLookupHash")? != hash {
        return Err("builder lookup provenance hash differs".to_owned());
    }
    Ok(())
}

fn parse_list(value: &str) -> Result<Vec<&str>, String> {
    if value == "-" {
        return Ok(Vec::new());
    }
    let body = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| format!("invalid list token {value}"))?;
    if body.is_empty() {
        return Err("empty list must use '-'".to_owned());
    }
    Ok(body.split(',').collect())
}

fn validate_linker_flag_row(flag: &StrictRow, sibling: &StrictRow) -> Result<(), String> {
    match flag.value("lookupState")? {
        "FirstSourceIdentityMatch" => {
            let selected = flag.value("selectedAlias")?;
            let observers = parse_list(flag.value("observerAliases")?)?;
            if flag.value("selectedSourceAlias")? != sibling.value("beamAlias")?
                || flag.value("sharedCell")? != format!("bcell:{selected}")
                || observers.first().copied() != Some(selected)
                || !flag.bool("linkedAfter")?
                || flag.value("closedBefore")? != flag.value("closedAfter")?
                || !flag.bool("requested")?
                || flag.usize("writeCount")? != 1
                || flag.usize("valueChangeCount")? != usize::from(!flag.bool("linkedBefore")?)
            {
                return Err("selected sibling B-linker shared-cell trace differs".to_owned());
            }
        }
        "ExhaustiveNoMatch"
            if flag.value("eventOrdinal")? == "-"
                && flag.value("selectedAlias")? == "-"
                && flag.value("observerAliases")? == "-"
                && flag.value("requested")? == "NotRead"
                && flag.usize("writeCount")? == 0
                && flag.usize("valueChangeCount")? == 0
                && flag.value("transition")? == "NoLinkerNoWrite" => {}
        _ => return Err("sibling linker-flag outcome differs".to_owned()),
    }
    Ok(())
}

fn validate_delta_guard(
    guard: &StrictRow,
    baseline: &StrictRow,
    result: &StrictRow,
    edges_added: usize,
    beam_abnormal_changes: usize,
) -> Result<(), String> {
    let unchanged_pairs = [
        ("allocatorBefore", "allocatorAfter"),
        ("glyphActiveHashBefore", "glyphActiveHashAfter"),
        ("glyphOriginalsHashBefore", "glyphOriginalsHashAfter"),
        ("interIndexCountBefore", "interIndexCountAfter"),
        ("sigVertexCountBefore", "sigVertexCountAfter"),
        ("systemStemsHashBefore", "systemStemsHashAfter"),
        ("lineHashBefore", "lineHashAfter"),
        ("arenaTopologyHashBefore", "arenaTopologyHashAfter"),
        ("builderItemsHashBefore", "builderItemsHashAfter"),
        ("relationInputHashBefore", "relationInputHashAfter"),
    ];
    if unchanged_pairs
        .iter()
        .any(|(before, after)| guard.value(before) != guard.value(after))
        || guard.usize("sigEdgeCountBefore")? != baseline.usize("graphEdgesBefore")?
        || guard.usize("sigEdgeCountAfter")? != guard.usize("sigEdgeCountBefore")? + edges_added
        || guard.value("groupStateHashBefore")? != baseline.value("groupStateHashBefore")?
        || guard.value("groupStateHashAfter")? != result.value("groupStateHashAfter")?
        || !is_lower_sha256(guard.value("groupStateHashBefore")?)
        || !is_lower_sha256(guard.value("groupStateHashAfter")?)
        || ((beam_abnormal_changes == 0)
            != (guard.value("groupStateHashBefore")? == guard.value("groupStateHashAfter")?))
        || guard.value("allowedMutations")?
            != "FreshSiblingBeamStemEdgesBeamAbnormalDirtySelectedBCells"
        || !guard.bool("zeroChordStem")?
        || !guard.bool("baseBeamStemUnchanged")?
        || !guard.bool("unrelatedRelationsUnchanged")?
        || !guard.bool("unrelatedLinkerFlagsUnchanged")?
        || guard.bool("headRelationLoopRead")?
        || !guard.bool("stopBeforeHeadRelationLoop")?
    {
        return Err("transaction delta guard differs".to_owned());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default)]
struct SupplementalCensus {
    supported: usize,
    envelope: usize,
    siblings: usize,
    same_glyph: usize,
    existing_beam_stem: usize,
    shorter_wrong_side: usize,
    linked: usize,
    edges: usize,
    flags: usize,
    events: usize,
}

#[derive(Clone, Copy)]
struct SupplementalCaseSpec {
    name: &'static str,
    scope: &'static str,
    sibling_class: &'static str,
    branch: &'static str,
    pair_state: &'static str,
    ratio_read: bool,
    shorter: Option<bool>,
    fresh_edge: bool,
    graph_delta: i64,
    include_linker: bool,
    write: bool,
    linked_before: bool,
    throw_class: &'static str,
    throw_stage: &'static str,
}

const SUPPLEMENTAL_CASES: &[SupplementalCaseSpec] = &[
    SupplementalCaseSpec {
        name: "SameGlyph",
        scope: "synthetic",
        sibling_class: "org.audiveris.omr.sig.inter.BeamInter",
        branch: "SameGlyph",
        pair_state: "NotReadSameGlyph",
        ratio_read: false,
        shorter: None,
        fresh_edge: false,
        graph_delta: 0,
        include_linker: true,
        write: false,
        linked_before: false,
        throw_class: "-",
        throw_stage: "-",
    },
    SupplementalCaseSpec {
        name: "ExistingBeamStem",
        scope: "synthetic",
        sibling_class: "org.audiveris.omr.sig.inter.BeamInter",
        branch: "ExistingBeamStem",
        pair_state: "ExistingBeamStem",
        ratio_read: false,
        shorter: None,
        fresh_edge: false,
        graph_delta: 0,
        include_linker: true,
        write: false,
        linked_before: false,
        throw_class: "-",
        throw_stage: "-",
    },
    SupplementalCaseSpec {
        name: "ShorterWrongSide",
        scope: "synthetic",
        sibling_class: "org.audiveris.omr.sig.inter.BeamInter",
        branch: "ShorterWrongSide",
        pair_state: "Absent",
        ratio_read: true,
        shorter: Some(true),
        fresh_edge: false,
        graph_delta: 0,
        include_linker: true,
        write: false,
        linked_before: false,
        throw_class: "-",
        throw_stage: "-",
    },
    SupplementalCaseSpec {
        name: "LinkedBeam",
        scope: "synthetic",
        sibling_class: "org.audiveris.omr.sig.inter.BeamInter",
        branch: "Linked",
        pair_state: "Absent",
        ratio_read: true,
        shorter: Some(false),
        fresh_edge: true,
        graph_delta: 1,
        include_linker: true,
        write: true,
        linked_before: false,
        throw_class: "-",
        throw_stage: "-",
    },
    SupplementalCaseSpec {
        name: "LinkedSmallBeam",
        scope: "synthetic",
        sibling_class: "org.audiveris.omr.sig.inter.SmallBeamInter",
        branch: "Linked",
        pair_state: "Absent",
        ratio_read: true,
        shorter: Some(false),
        fresh_edge: true,
        graph_delta: 1,
        include_linker: true,
        write: true,
        linked_before: false,
        throw_class: "-",
        throw_stage: "-",
    },
    SupplementalCaseSpec {
        name: "LinkedHook",
        scope: "synthetic",
        sibling_class: "org.audiveris.omr.sig.inter.BeamHookInter",
        branch: "Linked",
        pair_state: "Absent",
        ratio_read: true,
        shorter: Some(false),
        fresh_edge: true,
        graph_delta: 1,
        include_linker: true,
        write: true,
        linked_before: false,
        throw_class: "-",
        throw_stage: "-",
    },
    SupplementalCaseSpec {
        name: "LinkedNoBLinker",
        scope: "synthetic",
        sibling_class: "org.audiveris.omr.sig.inter.BeamInter",
        branch: "Linked",
        pair_state: "Absent",
        ratio_read: true,
        shorter: Some(false),
        fresh_edge: true,
        graph_delta: 1,
        include_linker: false,
        write: false,
        linked_before: false,
        throw_class: "-",
        throw_stage: "-",
    },
    SupplementalCaseSpec {
        name: "LinkedIdempotentBCell",
        scope: "synthetic",
        sibling_class: "org.audiveris.omr.sig.inter.BeamInter",
        branch: "Linked",
        pair_state: "Absent",
        ratio_read: true,
        shorter: Some(false),
        fresh_edge: true,
        graph_delta: 1,
        include_linker: true,
        write: true,
        linked_before: true,
        throw_class: "-",
        throw_stage: "-",
    },
    SupplementalCaseSpec {
        name: "ThrowBeforeInsertion",
        scope: "envelope",
        sibling_class: "org.audiveris.omr.sig.inter.BeamInter",
        branch: "Linked",
        pair_state: "Absent",
        ratio_read: true,
        shorter: Some(false),
        fresh_edge: false,
        graph_delta: 0,
        include_linker: true,
        write: false,
        linked_before: false,
        throw_class: "java.lang.IllegalArgumentException",
        throw_stage: "AddEdgeBeforeInsertion",
    },
    SupplementalCaseSpec {
        name: "ThrowDuringCallback",
        scope: "envelope",
        sibling_class: "org.audiveris.omr.sig.inter.BeamInter",
        branch: "Linked",
        pair_state: "Absent",
        ratio_read: true,
        shorter: Some(false),
        fresh_edge: true,
        graph_delta: 1,
        include_linker: true,
        write: false,
        linked_before: false,
        throw_class: "org.audiveris.omr.rustport.StemsBeamVLinkSiblingLinksProbe$SyntheticListenerException",
        throw_stage: "LaterListenerAfterRelationCallback",
    },
];

fn validate_supplemental_case(
    row: &StrictRow,
    spec: SupplementalCaseSpec,
    real_predecessor: &StrictRow,
) -> Result<(), String> {
    let key = row.key()?;
    let expected_terminal = if spec.scope == "envelope" {
        format!("Threw:{}", spec.throw_stage)
    } else {
        "ReadyBeforeHeadRelationLoop".to_owned()
    };
    let expected_lookup = match spec.name {
        "LinkedBeam" | "LinkedSmallBeam" | "LinkedHook" | "LinkedIdempotentBCell" => {
            "FirstSourceIdentityMatch"
        }
        "LinkedNoBLinker" => "ExhaustiveNoMatch",
        _ => "NotRead",
    };
    let expected_events = usize::from(spec.fresh_edge) * 2
        + usize::from(spec.write)
        + usize::from(spec.scope == "envelope");
    if key.system != 1
        || key.plan != real_predecessor.usize("plan")?
        || key.scope != spec.scope
        || key.case_name != spec.name
        || row.value("join")? != "IsolatedBoundary15Replay"
        || row.value("sourceRealB15EvidenceSha256")?
            != real_predecessor.value("b15TransactionEvidenceSha256")?
        || row.value("construction")? != "RealBookSheetSystemSIG"
        || row.value("baseRuntimeClass")? != "org.audiveris.omr.sig.inter.BeamInter"
        || row.value("siblingRuntimeClass")? != spec.sibling_class
        || row.bool("stemAttachedBefore")? != (spec.name != "ThrowBeforeInsertion")
        || row.value("yDir")? != "1"
        || row.bool("sameGlyphIdentity")? != (spec.branch == "SameGlyph")
        || row.value("pairState")? != spec.pair_state
        || row.bool("ratioRead")? != spec.ratio_read
        || row.value("branch")? != spec.branch
        || row.usize("builderItems")? != usize::from(spec.include_linker)
        || row.value("lookupState")? != expected_lookup
        || row.bool("siblingBLinkedBefore")? != spec.linked_before
        || row.bool("siblingBLinkedAfter")? != (spec.linked_before || spec.write)
        || row.usize("writeCount")? != usize::from(spec.write)
        || row.usize("valueChangeCount")? != usize::from(spec.write && !spec.linked_before)
        || row.i64("graphEdgesAfter")? - row.i64("graphEdgesBefore")? != spec.graph_delta
        || (row.value("freshRelationIdentity")? != "-") != spec.fresh_edge
        || row.bool("callbackCompleted")? != spec.fresh_edge
        || row.usize("chordStemMatches")? != 0
        || row.value("throwClass")? != spec.throw_class
        || row.value("throwStage")? != spec.throw_stage
        || row.usize("eventCount")? != expected_events
        || row.value("terminal")? != expected_terminal
        || row.value("supportGrade")? != real_predecessor.value("supportGrade")?
    {
        return Err(format!("isolated case {} topology differs", spec.name));
    }
    for field in [
        "baseLength",
        "siblingLength",
        "maxShorterRatio",
        "supportGrade",
    ] {
        parse_hex_bits(row.value(field)?)?;
    }
    if spec.ratio_read {
        parse_hex_bits(row.value("ratio")?)?;
    } else if row.value("ratio")? != "-" {
        return Err(format!("isolated case {} eagerly read ratio", spec.name));
    }
    match spec.shorter {
        None if row.value("shorterInclusive")? == "-"
            && !row.bool("dyRead")?
            && row.value("dy")? == "-"
            && row.value("product")? == "-"
            && row.value("wrongSideStrict")? == "-" => {}
        Some(true)
            if row.bool("shorterInclusive")?
                && row.bool("dyRead")?
                && row.bool("wrongSideStrict")? =>
        {
            parse_hex_bits(row.value("dy")?)?;
            parse_hex_bits(row.value("product")?)?;
        }
        Some(false)
            if !row.bool("shorterInclusive")?
                && !row.bool("dyRead")?
                && row.value("dy")? == "-"
                && row.value("product")? == "-"
                && row.value("wrongSideStrict")? == "-" => {}
        _ => {
            return Err(format!(
                "isolated case {} shorter laziness differs",
                spec.name
            ));
        }
    }
    if spec.branch == "Linked" {
        if row.value("extension")? == "-"
            || !matches!(row.value("portion")?, "LEFT" | "CENTER" | "RIGHT")
        {
            return Err(format!("isolated case {} link geometry differs", spec.name));
        }
    } else if row.value("extension")? != "-" || row.value("portion")? != "-" {
        return Err(format!(
            "isolated case {} eagerly read link geometry",
            spec.name
        ));
    }
    if spec.fresh_edge {
        if row.value("stemIncidentState")? != "ExhaustiveIncomingThenOutgoing"
            || row.usize("stemIncidentRows")? == 0
            || row.value("beamIncidentState")? == "NotRead"
            || row.value("beamRule")? == "NotRead"
        {
            return Err(format!("isolated case {} callback scan differs", spec.name));
        }
    } else if row.value("stemIncidentState")? != "NotRead"
        || row.usize("stemIncidentRows")? != 0
        || row.value("beamIncidentState")? != "NotRead"
        || row.value("beamRule")? != "NotRead"
    {
        return Err(format!("isolated case {} eagerly read callback", spec.name));
    }
    row.bool("beamAbnormalBefore")?;
    row.bool("beamAbnormalAfter")?;
    if row.value("dirtyBefore")?.is_empty() || row.value("dirtyAfter")?.is_empty() {
        return Err(format!(
            "isolated case {} lacks dirty-state evidence",
            spec.name
        ));
    }
    Ok(())
}

fn validate_supplemental_rows(
    cursor: &mut RowCursor<'_>,
    page: &StrictRow,
    parsed: &[ParsedTransaction],
) -> Result<SupplementalCensus, String> {
    let real_predecessor = parsed
        .iter()
        .find(|transaction| transaction.key.scope == "real" && transaction.key.system == 1)
        .map(|transaction| &transaction.predecessor)
        .ok_or_else(|| "supplemental cases lack the real system-1 predecessor".to_owned())?;
    let mut census = SupplementalCensus::default();
    for spec in SUPPLEMENTAL_CASES {
        let case = cursor
            .rows
            .get(cursor.index)
            .ok_or_else(|| format!("missing isolated case {}", spec.name))?;
        if case.kind != RowKind::SyntheticCase || case.page != page.page {
            return Err(format!("isolated case order differs before {}", spec.name));
        }
        let key = case.key()?;
        validate_supplemental_case(case, *spec, real_predecessor)?;
        cursor.index += 1;

        let mut event_ordinal = 0;
        let mut event_kinds = Vec::new();
        while cursor.peek_kind() == Some(RowKind::SyntheticEvent) {
            let event = cursor.take(RowKind::SyntheticEvent, &key)?;
            require_ordinal(event, "eventOrdinal", event_ordinal)?;
            event_ordinal += 1;
            let kind = event.value("kind")?;
            if !matches!(
                kind,
                "SigEdgeInserted" | "RelationCallbackCompleted" | "BLinkerLinkedAssigned" | "Throw"
            ) {
                return Err(format!("unknown isolated event kind {kind}"));
            }
            event_kinds.push((kind, event.value("relationIdentity")?));
        }
        if event_ordinal != case.usize("eventCount")? {
            return Err(format!("isolated case {} event count differs", spec.name));
        }
        let fresh = case.value("freshRelationIdentity")?;
        let mut expected_events = Vec::new();
        if spec.fresh_edge {
            expected_events.push(("SigEdgeInserted", fresh));
            expected_events.push(("RelationCallbackCompleted", fresh));
        }
        if spec.write {
            expected_events.push(("BLinkerLinkedAssigned", "-"));
        }
        if spec.scope == "envelope" {
            expected_events.push(("Throw", fresh));
        }
        if event_kinds != expected_events {
            return Err(format!("isolated case {} event prefix differs", spec.name));
        }

        let guard = cursor.take(RowKind::SyntheticGuard, &key)?;
        if guard.i64("graphDelta")? != spec.graph_delta
            || guard.value("allowedMutations")?
                != "FreshSiblingBeamStemBeamAbnormalDirtySelectedBCell"
            || !guard.bool("baseBeamUnchanged")?
            || !guard.bool("stemGeometryUnchanged")?
            || !guard.bool("groupObjectUnchanged")?
            || !guard.bool("zeroChordStem")?
            || !guard.bool("isolatedOnly")?
            || guard.bool("productionEquivalent")?
            || !guard.bool("enclosingRealSheetUnchanged")?
            || guard.bool("headRelationLoopRead")?
            || guard.value("terminal")? != case.value("terminal")?
        {
            return Err(format!("isolated case {} guard differs", spec.name));
        }

        census.supported += usize::from(spec.scope == "synthetic");
        census.envelope += usize::from(spec.scope == "envelope");
        census.siblings += 1;
        census.same_glyph += usize::from(spec.branch == "SameGlyph");
        census.existing_beam_stem += usize::from(spec.branch == "ExistingBeamStem");
        census.shorter_wrong_side += usize::from(spec.branch == "ShorterWrongSide");
        census.linked += usize::from(spec.branch == "Linked");
        census.edges += usize::from(spec.fresh_edge);
        census.flags += usize::from(spec.write);
        census.events += event_ordinal;
    }
    Ok(census)
}

fn validate_core_rows(rows: &[StrictRow]) -> Result<Vec<ParsedTransaction>, String> {
    let page = rows
        .first()
        .ok_or_else(|| "fixture has no semantic rows".to_owned())?;
    validate_page_row(page)?;
    if rows.iter().skip(1).any(|row| row.kind == RowKind::Page) {
        return Err("fixture contains more than one page row".to_owned());
    }
    let mut cursor = RowCursor::new(rows, 1);
    let mut parsed = Vec::new();
    let mut real_systems = BTreeSet::new();
    let mut real_transactions = 0;
    let mut supported_synthetic_cases = 0;
    let mut envelope_cases = 0;
    let mut total_group_rows = 0;
    let mut total_siblings = 0;
    let mut total_branches = BTreeMap::from([
        ("SameGlyph", 0usize),
        ("ExistingBeamStem", 0),
        ("ShorterWrongSide", 0),
        ("Linked", 0),
    ]);
    let mut total_edges = 0;
    let mut total_flags = 0;
    let mut total_events = 0;
    let mut supplemental = None;
    while cursor.peek_kind() == Some(RowKind::Predecessor) {
        let transaction_start = cursor.index;
        let first = &rows[cursor.index];
        if first.kind != RowKind::Predecessor {
            return Err(format!(
                "transaction does not start with predecessor at semantic row {}",
                cursor.index
            ));
        }
        let key = first.key()?;
        if key.page != page.page {
            return Err("transaction page differs from page row".to_owned());
        }
        let predecessor = cursor.take(RowKind::Predecessor, &key)?;
        validate_predecessor_row(predecessor)?;
        let baseline = cursor.take(RowKind::Baseline, &key)?;
        validate_baseline_row(baseline, predecessor)?;

        let mut group_rows = Vec::new();
        for _ in 0..baseline.usize("groupOutgoingScanned")? {
            group_rows.push(cursor.take(RowKind::GroupMember, &key)?);
        }
        validate_group_rows(baseline, &group_rows)?;

        let mut branch_counts = BTreeMap::from([
            ("SameGlyph", 0usize),
            ("ExistingBeamStem", 0),
            ("ShorterWrongSide", 0),
            ("Linked", 0),
        ]);
        let mut committed_edges = 0;
        let mut committed_flags = 0;
        let mut beam_abnormal_changes = 0;
        let mut event_count = 0;
        for sibling_ordinal in 0..baseline.usize("siblings")? {
            let sibling = cursor.take(RowKind::Sibling, &key)?;
            require_ordinal(sibling, "siblingOrdinal", sibling_ordinal)?;
            let branch = sibling.value("branch")?;
            let count = branch_counts
                .get_mut(branch)
                .ok_or_else(|| format!("unknown sibling branch {branch}"))?;
            *count += 1;

            let mut source_rows = Vec::new();
            for _ in 0..sibling.usize("sourceOutgoingScanned")? {
                let row = cursor.take(RowKind::SourceOutgoing, &key)?;
                require_ordinal(row, "siblingOrdinal", sibling_ordinal)?;
                source_rows.push(row);
            }
            validate_source_rows(sibling, &source_rows)?;

            let mut pair_rows = Vec::new();
            for _ in 0..sibling.usize("pairRows")? {
                let row = cursor.take(RowKind::PairRelation, &key)?;
                require_ordinal(row, "siblingOrdinal", sibling_ordinal)?;
                pair_rows.push(row);
            }
            validate_pair_rows(sibling, &pair_rows)?;

            let geometry = match branch {
                "SameGlyph" | "ExistingBeamStem" => None,
                "ShorterWrongSide" | "Linked" => {
                    let row = cursor.take(RowKind::Geometry, &key)?;
                    require_ordinal(row, "siblingOrdinal", sibling_ordinal)?;
                    validate_geometry_row(row, sibling, baseline)?;
                    Some(row)
                }
                _ => unreachable!("branch domain checked above"),
            };

            let mut lookup_rows = Vec::new();
            let mut linker_flag = None;
            if branch == "Linked" {
                let edge = cursor.take(RowKind::Edge, &key)?;
                require_ordinal(edge, "siblingOrdinal", sibling_ordinal)?;
                if edge.usize("eventOrdinal")? != event_count {
                    return Err("edge global event ordinal differs".to_owned());
                }
                validate_edge_row(
                    edge,
                    sibling,
                    predecessor,
                    geometry.expect("Linked branch has geometry"),
                )?;
                committed_edges += 1;
                event_count += 1;

                let mut stem_rows = Vec::new();
                while cursor.peek_kind() == Some(RowKind::StemIncident) {
                    let row = cursor.take(RowKind::StemIncident, &key)?;
                    require_ordinal(row, "siblingOrdinal", sibling_ordinal)?;
                    stem_rows.push(row);
                }
                let mut beam_rows = Vec::new();
                while cursor.peek_kind() == Some(RowKind::BeamIncident) {
                    let row = cursor.take(RowKind::BeamIncident, &key)?;
                    require_ordinal(row, "siblingOrdinal", sibling_ordinal)?;
                    beam_rows.push(row);
                }
                let callback = cursor.take(RowKind::Callback, &key)?;
                require_ordinal(callback, "siblingOrdinal", sibling_ordinal)?;
                validate_callback_rows(callback, edge, &stem_rows, &beam_rows)?;
                beam_abnormal_changes += usize::from(callback.bool("abnormalChanged")?);
                event_count += 1;

                while cursor.peek_kind() == Some(RowKind::LinkerLookup) {
                    let row = cursor.take(RowKind::LinkerLookup, &key)?;
                    require_ordinal(row, "siblingOrdinal", sibling_ordinal)?;
                    lookup_rows.push(row);
                }
                let flag = cursor.take(RowKind::LinkerFlag, &key)?;
                require_ordinal(flag, "siblingOrdinal", sibling_ordinal)?;
                validate_linker_flag_row(flag, sibling)?;
                if flag.value("lookupState")? == "FirstSourceIdentityMatch" {
                    if flag.usize("eventOrdinal")? != event_count {
                        return Err("linker-flag global event ordinal differs".to_owned());
                    }
                    event_count += 1;
                    committed_flags += 1;
                }
                linker_flag = Some(flag);
            }

            let sibling_result = cursor.take(RowKind::SiblingResult, &key)?;
            require_ordinal(sibling_result, "siblingOrdinal", sibling_ordinal)?;
            if sibling_result.value("branch")? != branch
                || sibling_result.value("terminal")? != "Continue"
                || sibling_result.bool("edgeCommitted")? != (branch == "Linked")
                || sibling_result.bool("linkerLookupRead")? != (branch == "Linked")
                || sibling_result.usize("edgePrefixCount")? != committed_edges
                || sibling_result.usize("flagPrefixCount")? != committed_flags
            {
                return Err("sibling terminal/prefix census differs".to_owned());
            }
            if branch == "Linked" {
                validate_lookup_rows(sibling_result, &lookup_rows)?;
                let flag = linker_flag.expect("Linked branch has linker-flag row");
                if sibling_result.value("linkerLookupState")? != flag.value("lookupState")?
                    || sibling_result.value("linkerSelectedAlias")?
                        != flag.value("selectedAlias")?
                {
                    return Err("sibling lookup/result join differs".to_owned());
                }
            } else if sibling_result.value("linkerLookupState")? != "NotRead"
                || sibling_result.value("linkerLookupTiming")? != "NotRead"
                || sibling_result.usize("linkerLookupRows")? != 0
                || sibling_result.value("linkerLookupHash")? != "NotRead"
                || sibling_result.value("linkerSelectedAlias")? != "-"
            {
                return Err("non-linked sibling lookup was not lazy".to_owned());
            }
        }

        let result = cursor.take(RowKind::Result, &key)?;
        let guard = cursor.take(RowKind::DeltaGuard, &key)?;
        let summary = cursor.take(RowKind::Summary, &key)?;
        if result.value("terminal")? != "ReadyBeforeHeadRelationLoop"
            || result.value("supportGrade")? != predecessor.value("supportGrade")?
            || result.value("stemAlias")? != predecessor.value("stemAlias")?
            || result.value("stemInterId")? != predecessor.value("stemInterId")?
            || result.usize("siblings")? != baseline.usize("siblings")?
            || result.usize("committedEdges")? != committed_edges
            || result.usize("committedFlags")? != committed_flags
            || result.usize("eventCount")? != event_count
            || result.bool("headRelationLoopRead")?
            || summary.usize("groupRows")? != group_rows.len()
            || summary.usize("siblings")? != baseline.usize("siblings")?
            || summary.usize("sameGlyph")? != branch_counts["SameGlyph"]
            || summary.usize("existingBeamStem")? != branch_counts["ExistingBeamStem"]
            || summary.usize("shorterWrongSide")? != branch_counts["ShorterWrongSide"]
            || summary.usize("linked")? != branch_counts["Linked"]
            || summary.usize("edgesAdded")? != committed_edges
            || summary.usize("linkerWrites")? != committed_flags
            || summary.usize("events")? != event_count
            || summary.usize("chordStemMatches")? != 0
            || summary.value("terminal")? != "ReadyBeforeHeadRelationLoop"
        {
            return Err("transaction result/summary census differs".to_owned());
        }
        validate_delta_guard(
            guard,
            baseline,
            result,
            committed_edges,
            beam_abnormal_changes,
        )?;
        if key.scope == "real" && key.case_name == "-" {
            real_transactions += 1;
            real_systems.insert(key.system);
        } else {
            return Err(format!(
                "core transaction is not a real whole-page transaction: {:?}",
                key
            ));
        }
        total_group_rows += group_rows.len();
        total_siblings += baseline.usize("siblings")?;
        for branch in [
            "SameGlyph",
            "ExistingBeamStem",
            "ShorterWrongSide",
            "Linked",
        ] {
            total_branches.insert(branch, total_branches[branch] + branch_counts[branch]);
        }
        total_edges += committed_edges;
        total_flags += committed_flags;
        total_events += event_count;
        parsed.push(ParsedTransaction {
            key,
            predecessor: predecessor.clone(),
            rows: rows[transaction_start..cursor.index].to_vec(),
        });
        if cursor.peek_kind() == Some(RowKind::SyntheticCase) {
            if supplemental.is_some() {
                return Err("fixture contains more than one isolated system-1 block".to_owned());
            }
            supplemental = Some(validate_supplemental_rows(&mut cursor, page, &parsed)?);
        }
    }
    if parsed.is_empty() {
        return Err("fixture has no transactions".to_owned());
    }
    let supplemental = supplemental
        .ok_or_else(|| "fixture lacks its isolated system-1 supplemental block".to_owned())?;
    supported_synthetic_cases += supplemental.supported;
    envelope_cases += supplemental.envelope;
    total_siblings += supplemental.siblings;
    total_branches.insert(
        "SameGlyph",
        total_branches["SameGlyph"] + supplemental.same_glyph,
    );
    total_branches.insert(
        "ExistingBeamStem",
        total_branches["ExistingBeamStem"] + supplemental.existing_beam_stem,
    );
    total_branches.insert(
        "ShorterWrongSide",
        total_branches["ShorterWrongSide"] + supplemental.shorter_wrong_side,
    );
    total_branches.insert("Linked", total_branches["Linked"] + supplemental.linked);
    total_edges += supplemental.edges;
    total_flags += supplemental.flags;
    total_events += supplemental.events;
    if cursor.peek_kind() == Some(RowKind::PageSummary) {
        let summary = &rows[cursor.index];
        cursor.index += 1;
        if summary.page != page.page
            || summary.usize("systems")? != page.usize("systems")?
            || summary.usize("systems")? != real_systems.len()
            || summary.usize("realTransactions")? != real_transactions
            || summary.usize("supportedSyntheticCases")? != supported_synthetic_cases
            || summary.usize("envelopeCases")? != envelope_cases
            || summary.usize("totalTransactions")?
                != parsed.len() + supported_synthetic_cases + envelope_cases
            || summary.usize("groupRows")? != total_group_rows
            || summary.usize("siblingCandidates")? != total_siblings
            || summary.usize("sameGlyph")? != total_branches["SameGlyph"]
            || summary.usize("existingBeamStem")? != total_branches["ExistingBeamStem"]
            || summary.usize("shorterWrongSide")? != total_branches["ShorterWrongSide"]
            || summary.usize("linked")? != total_branches["Linked"]
            || summary.usize("edgesAdded")? != total_edges
            || summary.usize("linkerWrites")? != total_flags
            || summary.usize("events")? != total_events
            || summary.usize("chordStemMatches")? != 0
            || !summary.bool("stopBeforeHeadRelationLoop")?
        {
            return Err("page-summary census differs".to_owned());
        }
    } else {
        return Err("fixture lacks its page-summary trailer".to_owned());
    }
    if cursor.peek_kind() == Some(RowKind::CorpusSummary) {
        cursor.index += 1;
    } else {
        return Err("fixture lacks its corpus-summary trailer".to_owned());
    }
    if cursor.index != rows.len() {
        return Err(format!(
            "unexpected semantic row after core/page summary: {:?}",
            rows[cursor.index].kind
        ));
    }
    Ok(parsed)
}

fn raw_fields(line: &str) -> Result<(String, BTreeMap<String, String>), String> {
    let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
    let page = tokens
        .get(1)
        .ok_or_else(|| "predecessor row lacks page token".to_owned())?;
    if (tokens.len() - 2) % 2 != 0 {
        return Err("predecessor row lacks key/value pairs".to_owned());
    }
    let mut fields = BTreeMap::new();
    for pair in tokens[2..].chunks_exact(2) {
        if fields
            .insert(pair[0].to_owned(), pair[1].to_owned())
            .is_some()
        {
            return Err(format!("duplicate predecessor field {}", pair[0]));
        }
    }
    Ok(((*page).to_owned(), fields))
}

fn required_map<'a>(fields: &'a BTreeMap<String, String>, name: &str) -> Result<&'a str, String> {
    fields
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| format!("missing predecessor field {name}"))
}

fn validate_manifest_fixture_entry(key: &str, expected_sha256: &str) -> Result<(), String> {
    let manifest = std::fs::read_to_string(repo_root().join(BOUNDARY_FIFTEEN_MANIFEST_PATH))
        .map_err(|error| format!("cannot read Boundary-15 manifest: {error}"))?;
    let matches = manifest
        .lines()
        .filter(|line| line.starts_with("stemsbeamvlinkblinkerflagmanifestentry "))
        .filter_map(|line| {
            let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
            ((tokens.len() - 1) % 2 == 0).then(|| {
                tokens[1..]
                    .chunks_exact(2)
                    .map(|pair| (pair[0].to_owned(), pair[1].to_owned()))
                    .collect::<BTreeMap<_, _>>()
            })
        })
        .filter(|fields| fields.get("page").is_some_and(|page| page == key))
        .collect::<Vec<_>>();
    let [entry] = matches.as_slice() else {
        return Err(format!("Boundary-15 manifest lacks one {key} entry"));
    };
    if required_map(entry, "fixture")? != format!("stems-beam-vlink-b-linker-flag-{key}.txt")
        || required_map(entry, "fixtureSha256")? != expected_sha256
    {
        return Err(format!("Boundary-15 {key} manifest fixture pin differs"));
    }
    Ok(())
}

fn validate_manifest_fixture_pin() -> Result<(), String> {
    validate_manifest_fixture_entry("chula", BOUNDARY_FIFTEEN_FIXTURE_SHA256)
}

fn validate_boundary_fifteen_bundle(
    transaction: &ParsedTransaction,
    fixture: &str,
) -> Result<(), String> {
    let predecessor = &transaction.predecessor;
    let mut ordered = Vec::new();
    let mut result_row = None;
    let mut guard_row = None;
    let mut summary_row = None;
    let mut b15_predecessor = None;
    for line in fixture.lines() {
        if !line.starts_with("stemsbeamvlinkblinkerflag")
            || line.starts_with("stemsbeamvlinkblinkerflagpage ")
            || line.starts_with("stemsbeamvlinkblinkerflagpagesummary ")
            || line.starts_with("stemsbeamvlinkblinkerflagcorpussummary ")
        {
            continue;
        }
        let (page, fields) = raw_fields(line)?;
        if page != transaction.key.page
            || required_map(&fields, "scope")? != "real"
            || required_map(&fields, "system")? != transaction.key.system.to_string()
        {
            continue;
        }
        if required_map(&fields, "plan")? != transaction.key.plan.to_string()
            || required_map(&fields, "case")? != "-"
        {
            return Err("Boundary-15 real system row key differs".to_owned());
        }
        ordered.push(line.to_owned());
        if line.starts_with("stemsbeamvlinkblinkerflagpredecessor ") {
            if b15_predecessor.replace(fields).is_some() {
                return Err("duplicate Boundary-15 predecessor row".to_owned());
            }
        } else if line.starts_with("stemsbeamvlinkblinkerflagresult ") {
            if result_row.replace((line, fields)).is_some() {
                return Err("duplicate Boundary-15 result row".to_owned());
            }
        } else if line.starts_with("stemsbeamvlinkblinkerflagdeltaguard ") {
            if guard_row.replace(line).is_some() {
                return Err("duplicate Boundary-15 guard row".to_owned());
            }
        } else if line.starts_with("stemsbeamvlinkblinkerflagsummary ")
            && summary_row.replace(line).is_some()
        {
            return Err("duplicate Boundary-15 summary row".to_owned());
        }
    }
    let b15_predecessor =
        b15_predecessor.ok_or_else(|| "missing Boundary-15 predecessor row".to_owned())?;
    let (result_raw, result) =
        result_row.ok_or_else(|| "missing Boundary-15 result row".to_owned())?;
    let guard_raw = guard_row.ok_or_else(|| "missing Boundary-15 guard row".to_owned())?;
    let summary_raw = summary_row.ok_or_else(|| "missing Boundary-15 summary row".to_owned())?;
    let base_beam_alias = required_map(&result, "bAlias")?
        .split_once(":b:")
        .map(|(beam, _)| beam)
        .ok_or_else(|| "Boundary-15 B alias lacks beam prefix".to_owned())?;
    if predecessor.usize("b15TransactionRows")? != ordered.len()
        || predecessor.value("b15TransactionEvidenceSha256")?
            != sha256_rows(ordered.iter().cloned())
        || predecessor.value("b15ResultRowSha256")? != sha256_rows([result_raw.to_owned()])
        || predecessor.value("b15GuardRowSha256")? != sha256_rows([guard_raw.to_owned()])
        || predecessor.value("b15SummaryRowSha256")? != sha256_rows([summary_raw.to_owned()])
        || predecessor.value("predecessorTerminal")? != required_map(&result, "terminal")?
        || predecessor.value("applyReturn")? != required_map(&result, "applyReturn")?
        || predecessor.value("supportGrade")? != required_map(&result, "supportGrade")?
        || predecessor.value("stemAlias")? != required_map(&result, "stemAlias")?
        || predecessor.value("stemInterId")? != required_map(&result, "stemInterId")?
        || predecessor.value("baseBeamAlias")? != base_beam_alias
        || predecessor.value("targetBAlias")? != required_map(&result, "bAlias")?
        || predecessor.value("triggeringVAlias")? != required_map(&b15_predecessor, "vAlias")?
        || predecessor.value("targetBLinked")? != required_map(&result, "linked")?
        || predecessor.value("targetBLinked")? != "true"
    {
        return Err("Boundary-15 canonical all-row bundle/full terminal join differs".to_owned());
    }
    Ok(())
}

fn boundary_fifteen_linked_before(
    fixture: &str,
    transaction: &ParsedTransaction,
) -> Result<bool, String> {
    let matches = fixture
        .lines()
        .filter(|line| line.starts_with("stemsbeamvlinkblinkerflagtarget "))
        .filter_map(|line| raw_fields(line).ok())
        .filter(|(page, fields)| {
            page == &transaction.key.page
                && fields
                    .get("system")
                    .is_some_and(|value| value == &transaction.key.system.to_string())
                && fields
                    .get("plan")
                    .is_some_and(|value| value == &transaction.key.plan.to_string())
                && fields.get("scope").is_some_and(|value| value == "real")
                && fields.get("case").is_some_and(|value| value == "-")
        })
        .collect::<Vec<_>>();
    let [(_, fields)] = matches.as_slice() else {
        return Err(format!(
            "Boundary-15 fixture has {} target rows for {:?}",
            matches.len(),
            transaction.key
        ));
    };
    match required_map(fields, "linkedBefore")? {
        "true" => Ok(true),
        "false" => Ok(false),
        value => Err(format!("invalid Boundary-15 linkedBefore {value}")),
    }
}

fn validate_boundary_fifteen_predecessors(
    page_row: &StrictRow,
    transactions: &[ParsedTransaction],
) -> Result<(), String> {
    let (page_key, _) = corpus_page_for_token(&page_row.page)?;
    let fixture_path = boundary_fifteen_fixture_path(page_key);
    let fixture_sha256 = read_sha256(&fixture_path)?;
    if read_sha256(BOUNDARY_FIFTEEN_MANIFEST_PATH)? != BOUNDARY_FIFTEEN_MANIFEST_SHA256
        || read_sha256(BOUNDARY_FIFTEEN_GATE_PATH)? != BOUNDARY_FIFTEEN_GATE_SHA256
        || page_row.value("bLinkerFlagFixtureSha256")? != fixture_sha256
    {
        return Err("Boundary-15 manifest/gate/fixture source pin differs".to_owned());
    }
    validate_manifest_fixture_entry(page_key, &fixture_sha256)?;
    let fixture = std::fs::read_to_string(repo_root().join(&fixture_path))
        .map_err(|error| format!("cannot read Boundary-15 {page_key} fixture: {error}"))?;
    let b15_page = fixture
        .lines()
        .find(|line| line.starts_with("stemsbeamvlinkblinkerflagpage "))
        .ok_or_else(|| "Boundary-15 fixture lacks page row".to_owned())?;
    let (b15_page_name, b15_page_fields) = raw_fields(b15_page)?;
    if b15_page_name != page_row.page {
        return Err("Boundary-15/B16 page identity differs".to_owned());
    }
    for field in [
        "schedulerFixtureSha256",
        "expandFixtureSha256",
        "createStemFixtureSha256",
        "reuseCheckFixtureSha256",
        "baseApplyFixtureSha256",
    ] {
        if page_row.value(field)? != required_map(&b15_page_fields, field)? {
            return Err(format!("Boundary-15/B16 {field} predecessor pin differs"));
        }
    }
    for transaction in transactions {
        validate_boundary_fifteen_bundle(transaction, &fixture)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct BeamId(usize);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct StemId(usize);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LinkerCellId(usize);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LinkerId(usize);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Vertex {
    Beam(BeamId),
    Stem(StemId),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Line {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BeamRuntimeClass {
    Beam,
    Hook,
    SmallBeam,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BeamPortion {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelationPayload {
    BeamStem {
        portion: BeamPortion,
        grade_bits: u64,
    },
    BeamRest {
        portion: BeamPortion,
    },
    ChordStem,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GraphEdge {
    relation_identity: usize,
    source: Vertex,
    target: Vertex,
    payload: RelationPayload,
}

#[derive(Clone, Debug, PartialEq)]
struct BeamState {
    runtime_class: BeamRuntimeClass,
    /// Java object identity. `None == None` deliberately models two null
    /// glyphs satisfying `b.getGlyph() == beam.getGlyph()`.
    glyph_identity: Option<usize>,
    median: Line,
    height: f64,
    abnormal: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BuilderItem {
    NonLinker,
    Linker {
        source: BeamId,
        linker: LinkerId,
        linked_cell: LinkerCellId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AddEdgeBehavior {
    Added,
    ReturnedFalse,
    ThrowBeforeInsertion,
    ThrowDuringCallback,
}

#[derive(Clone, Debug, PartialEq)]
struct IndependentState {
    vertices: BTreeSet<Vertex>,
    /// JGraphT global insertion order. Incident and directed-pair scans are
    /// stable filters over this order in the independent model.
    edges: Vec<GraphEdge>,
    beams: BTreeMap<BeamId, BeamState>,
    builder_items: Vec<BuilderItem>,
    linked_cells: BTreeMap<LinkerCellId, bool>,
    /// Exact sibling B-linker first, then its V children in TOP/BOTTOM order.
    /// The selected ordinary `LinkerItem` owns this shared cell directly.
    linked_cell_observers: BTreeMap<LinkerCellId, Vec<LinkerId>>,
    stub_modified: bool,
    book_modified: bool,
    book_dirty: bool,
    next_relation_identity: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct IndependentInput {
    base_beam: BeamId,
    stem: StemId,
    ref_point: Point,
    skewed_vertical: Line,
    stem_median: Line,
    group_members: Vec<BeamId>,
    max_beam_side_dx: f64,
    max_shorter_ratio: f64,
    portion_max_dx: i32,
    y_dir: i32,
    continuation_support_grade_bits: u64,
    add_edge_behavior: BTreeMap<BeamId, AddEdgeBehavior>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SiblingBranch {
    SameGlyph,
    ExistingBeamStem,
    ShorterWrongSide,
    Linked,
}

const fn native_branch(branch: SiblingBranch) -> NativeStemsBeamSiblingBranch {
    match branch {
        SiblingBranch::SameGlyph => NativeStemsBeamSiblingBranch::SameGlyph,
        SiblingBranch::ExistingBeamStem => NativeStemsBeamSiblingBranch::ExistingBeamStem,
        SiblingBranch::ShorterWrongSide => NativeStemsBeamSiblingBranch::ShorterWrongSide,
        SiblingBranch::Linked => NativeStemsBeamSiblingBranch::Linked,
    }
}

fn assert_public_baseline_projection(transaction: &NativeStemsBeamVLinkSiblingLinksTransaction) {
    let _ = (
        &transaction.cached_base_median,
        transaction.cached_base_median_same_identity,
        &transaction.group_runtime,
        transaction.base_cross,
        transaction.base_length,
    );
    for sibling in &transaction.siblings {
        let _ = (
            &sibling.builder_lookup,
            sibling.closed_before,
            sibling.closed_after,
        );
    }
    for operation in &transaction.operations {
        if let NativeStemsBeamVLinkSiblingLinksOperation::BLinkerLinkedAssigned {
            ordered_observer_v_linkers,
            closed_before,
            closed_after,
            ..
        } = operation
        {
            let _ = (ordered_observer_v_linkers, closed_before, closed_after);
        }
    }
}

fn assert_public_state_projection(
    state: &NativeStemsBeamVLinkSiblingLinksState,
    live: &NativeStemsBeamSiblingLiveBeam,
) {
    let _ = (
        &state.b_linker_flag_state_after,
        live.inter_index_ordinal,
        live.inter_index_object_matches,
        live.inter_index_id_matches,
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ThrowStage {
    AddEdgeBeforeInsertion,
    RelationCallbackAfterInsertion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Terminal {
    ReadyBeforeHeadRelationLoop,
    Threw(ThrowStage),
}

#[derive(Clone, Debug, PartialEq)]
struct DiscoveryTrace {
    group_ordinal: usize,
    beam: BeamId,
    cross: Point,
    within_margin: bool,
    sorted_ordinal: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
struct PairRelationTrace {
    pair_ordinal: usize,
    relation_identity: usize,
    payload: RelationPayload,
    class_read: bool,
    matched_beam_stem: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct GeometryTrace {
    base_cross: Point,
    sibling_cross: Point,
    base_length: f64,
    sibling_length: f64,
    ratio: f64,
    shorter_branch_read: bool,
    dy: Option<f64>,
    dy_times_y_dir: Option<f64>,
    extension: Option<Point>,
    left_threshold: Option<f64>,
    right_threshold: Option<f64>,
    portion: Option<BeamPortion>,
    grade_bits: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
struct CallbackTrace {
    stem_incident_relation_identities: Vec<usize>,
    beam_incident_relation_identities: Vec<usize>,
    extension_was_populated: bool,
    portion_was_populated: bool,
    chord_stem_matches: usize,
    abnormal_before: bool,
    abnormal_after: bool,
    stub_modified_before: bool,
    stub_modified_after: bool,
    book_modified_before: bool,
    book_modified_after: bool,
    book_dirty_before: bool,
    book_dirty_after: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LinkerLookupTrace {
    examined_item_ordinals: Vec<usize>,
    matched_item_ordinal: Option<usize>,
    unread_suffix: usize,
    selected_linker: Option<LinkerId>,
    linked_cell: Option<LinkerCellId>,
    linked_before: Option<bool>,
    linked_after: Option<bool>,
    write_count: usize,
    value_change_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct SiblingTrace {
    sibling_ordinal: usize,
    beam: BeamId,
    pair_relations: Vec<PairRelationTrace>,
    geometry: Option<GeometryTrace>,
    callback: Option<CallbackTrace>,
    linker_lookup: Option<LinkerLookupTrace>,
    branch: SiblingBranch,
    first_event_ordinal: usize,
    event_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SerialEvent {
    EdgeInserted {
        sibling_ordinal: usize,
        relation_identity: usize,
    },
    CallbackCompleted {
        sibling_ordinal: usize,
        relation_identity: usize,
    },
    LinkerFlagAssigned {
        sibling_ordinal: usize,
        selected_linker: LinkerId,
        cell: LinkerCellId,
        ordered_observers: Vec<LinkerId>,
        before: bool,
        after: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct IndependentTransaction {
    ref_point: Point,
    discoveries: Vec<DiscoveryTrace>,
    sorted_siblings_before_base_removal: Vec<BeamId>,
    base_removal_index: Option<usize>,
    sibling_traces: Vec<SiblingTrace>,
    events: Vec<SerialEvent>,
    terminal: Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IndependentError {
    InvalidDirection,
    MissingBaseBeam,
    MissingStem,
    MissingGroupBeam,
    InvalidRelationAllocator,
    ChordBearingStem,
    MissingLinkerCell,
}

fn java_double_bits(value: f64) -> u64 {
    if value.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        value.to_bits()
    }
}

fn java_double_compare(left: f64, right: f64) -> Ordering {
    if left < right {
        Ordering::Less
    } else if left > right {
        Ordering::Greater
    } else {
        (java_double_bits(left) as i64).cmp(&(java_double_bits(right) as i64))
    }
}

fn intersection(a: Line, b: Line) -> Point {
    // Preserve the operation order in Java LineUtil.intersection. Keeping the
    // intermediates named also makes exact-bit divergences diagnosable.
    let denominator = ((a.x1 - a.x2) * (b.y1 - b.y2)) - ((a.y1 - a.y2) * (b.x1 - b.x2));
    let v12 = (a.x1 * a.y2) - (a.y1 * a.x2);
    let v34 = (b.x1 * b.y2) - (b.y1 * b.x2);
    let x = ((v12 * (b.x1 - b.x2)) - ((a.x1 - a.x2) * v34)) / denominator;
    let y = ((v12 * (b.y1 - b.y2)) - ((a.y1 - a.y2) * v34)) / denominator;
    Point { x, y }
}

fn beam_portion(median: Line, x: f64, max_dx: i32) -> BeamPortion {
    let max_dx = f64::from(max_dx);
    if x < median.x1 + max_dx {
        BeamPortion::Left
    } else if x > median.x2 - max_dx {
        BeamPortion::Right
    } else {
        BeamPortion::Center
    }
}

fn preflight(input: &IndependentInput, state: &IndependentState) -> Result<(), IndependentError> {
    if !matches!(input.y_dir, -1 | 1) {
        return Err(IndependentError::InvalidDirection);
    }
    if !state.beams.contains_key(&input.base_beam) {
        return Err(IndependentError::MissingBaseBeam);
    }
    if !state.vertices.contains(&Vertex::Stem(input.stem)) {
        return Err(IndependentError::MissingStem);
    }
    if input
        .group_members
        .iter()
        .any(|beam| !state.beams.contains_key(beam))
    {
        return Err(IndependentError::MissingGroupBeam);
    }
    if state.next_relation_identity == 0
        || state
            .edges
            .iter()
            .any(|edge| edge.relation_identity >= state.next_relation_identity)
    {
        return Err(IndependentError::InvalidRelationAllocator);
    }
    for item in &state.builder_items {
        let BuilderItem::Linker {
            linker,
            linked_cell,
            ..
        } = item
        else {
            continue;
        };
        let Some(observers) = state.linked_cell_observers.get(linked_cell) else {
            return Err(IndependentError::MissingLinkerCell);
        };
        if !state.linked_cells.contains_key(linked_cell)
            || observers.first() != Some(linker)
            || observers.iter().collect::<BTreeSet<_>>().len() != observers.len()
        {
            return Err(IndependentError::MissingLinkerCell);
        }
    }
    // Compact v1 intentionally excludes the substantial chord cache mutation
    // branch. This checks live state, not a predecessor aggregate.
    if state.edges.iter().any(|edge| {
        edge.payload == RelationPayload::ChordStem
            && (edge.source == Vertex::Stem(input.stem) || edge.target == Vertex::Stem(input.stem))
    }) {
        return Err(IndependentError::ChordBearingStem);
    }
    Ok(())
}

fn directed_pair_trace(
    state: &IndependentState,
    sibling: BeamId,
    stem: StemId,
) -> Vec<PairRelationTrace> {
    let mut traces = Vec::new();
    let mut class_read = true;
    for edge in state
        .edges
        .iter()
        .filter(|edge| edge.source == Vertex::Beam(sibling) && edge.target == Vertex::Stem(stem))
    {
        let matched_beam_stem =
            class_read && matches!(edge.payload, RelationPayload::BeamStem { .. });
        traces.push(PairRelationTrace {
            pair_ordinal: traces.len(),
            relation_identity: edge.relation_identity,
            payload: edge.payload,
            class_read,
            matched_beam_stem,
        });
        if matched_beam_stem {
            class_read = false;
        }
    }
    traces
}

fn complete_relation_callback(
    sibling: BeamId,
    stem: StemId,
    state: &mut IndependentState,
) -> CallbackTrace {
    let beam_before = state.beams[&sibling].clone();
    let stub_before = state.stub_modified;
    let book_modified_before = state.book_modified;
    let book_dirty_before = state.book_dirty;
    let stem_incident = state
        .edges
        .iter()
        .filter(|edge| edge.source == Vertex::Stem(stem) || edge.target == Vertex::Stem(stem))
        .map(|edge| edge.relation_identity)
        .collect::<Vec<_>>();
    let chord_stem_matches = state
        .edges
        .iter()
        .filter(|edge| {
            edge.payload == RelationPayload::ChordStem
                && (edge.source == Vertex::Stem(stem) || edge.target == Vertex::Stem(stem))
        })
        .count();
    let beam_incident_edges = state
        .edges
        .iter()
        .filter(|edge| edge.source == Vertex::Beam(sibling) || edge.target == Vertex::Beam(sibling))
        .collect::<Vec<_>>();
    let beam_incident = beam_incident_edges
        .iter()
        .map(|edge| edge.relation_identity)
        .collect::<Vec<_>>();
    let abnormal_after = match beam_before.runtime_class {
        BeamRuntimeClass::Hook => !beam_incident_edges
            .iter()
            .any(|edge| matches!(edge.payload, RelationPayload::BeamStem { .. })),
        BeamRuntimeClass::Beam | BeamRuntimeClass::SmallBeam => {
            let mut left = false;
            let mut right = false;
            for edge in &beam_incident_edges {
                let portion = match edge.payload {
                    RelationPayload::BeamStem { portion, .. }
                    | RelationPayload::BeamRest { portion } => portion,
                    RelationPayload::ChordStem | RelationPayload::Other => continue,
                };
                left |= portion == BeamPortion::Left;
                right |= portion == BeamPortion::Right;
            }
            !left || !right
        }
    };
    if abnormal_after != beam_before.abnormal {
        state
            .beams
            .get_mut(&sibling)
            .expect("validated beam")
            .abnormal = abnormal_after;
        state.stub_modified = true;
        state.book_modified = true;
        state.book_dirty = true;
    }
    CallbackTrace {
        stem_incident_relation_identities: stem_incident,
        beam_incident_relation_identities: beam_incident,
        extension_was_populated: true,
        portion_was_populated: true,
        chord_stem_matches,
        abnormal_before: beam_before.abnormal,
        abnormal_after,
        stub_modified_before: stub_before,
        stub_modified_after: state.stub_modified,
        book_modified_before,
        book_modified_after: state.book_modified,
        book_dirty_before,
        book_dirty_after: state.book_dirty,
    }
}

fn lookup_and_assign_linker(
    sibling: BeamId,
    state: &mut IndependentState,
) -> Result<LinkerLookupTrace, IndependentError> {
    let mut examined = Vec::new();
    let mut matched = None;
    for (ordinal, item) in state.builder_items.iter().enumerate() {
        examined.push(ordinal);
        if matches!(item, BuilderItem::Linker { source, .. } if *source == sibling) {
            matched = Some((ordinal, *item));
            break;
        }
    }
    let Some((
        ordinal,
        BuilderItem::Linker {
            linker,
            linked_cell,
            ..
        },
    )) = matched
    else {
        return Ok(LinkerLookupTrace {
            examined_item_ordinals: examined,
            matched_item_ordinal: None,
            unread_suffix: 0,
            selected_linker: None,
            linked_cell: None,
            linked_before: None,
            linked_after: None,
            write_count: 0,
            value_change_count: 0,
        });
    };
    let Some(linked) = state.linked_cells.get_mut(&linked_cell) else {
        return Err(IndependentError::MissingLinkerCell);
    };
    let before = *linked;
    *linked = true;
    Ok(LinkerLookupTrace {
        examined_item_ordinals: examined,
        matched_item_ordinal: Some(ordinal),
        unread_suffix: state.builder_items.len() - ordinal - 1,
        selected_linker: Some(linker),
        linked_cell: Some(linked_cell),
        linked_before: Some(before),
        linked_after: Some(true),
        write_count: 1,
        value_change_count: usize::from(!before),
    })
}

fn apply_independent(
    input: &IndependentInput,
    state: &mut IndependentState,
) -> Result<IndependentTransaction, IndependentError> {
    preflight(input, state)?;
    let base = state.beams[&input.base_beam].clone();
    let base_cross = intersection(input.stem_median, base.median);
    let base_length = base.median.x2 - base.median.x1;
    let mut discoveries = Vec::with_capacity(input.group_members.len());
    let mut accepted = Vec::<(usize, BeamId, Point)>::new();
    for (group_ordinal, beam_id) in input.group_members.iter().copied().enumerate() {
        let beam = &state.beams[&beam_id];
        let cross = intersection(input.skewed_vertical, beam.median);
        let within = beam.median.x1 - input.max_beam_side_dx <= cross.x
            && cross.x <= beam.median.x2 + input.max_beam_side_dx;
        if within {
            accepted.push((group_ordinal, beam_id, cross));
        }
        discoveries.push(DiscoveryTrace {
            group_ordinal,
            beam: beam_id,
            cross,
            within_margin: within,
            sorted_ordinal: None,
        });
    }
    accepted.sort_by(|left, right| java_double_compare(left.2.y, right.2.y));
    for (sorted_ordinal, (group_ordinal, _, _)) in accepted.iter().enumerate() {
        discoveries[*group_ordinal].sorted_ordinal = Some(sorted_ordinal);
    }
    let sorted_siblings_before_base_removal = accepted
        .iter()
        .map(|(_, beam, _)| *beam)
        .collect::<Vec<_>>();
    let base_removal_index = accepted
        .iter()
        .position(|(_, beam, _)| *beam == input.base_beam);
    if let Some(index) = base_removal_index {
        accepted.remove(index);
    }

    let mut sibling_traces = Vec::with_capacity(accepted.len());
    let mut events = Vec::new();
    let mut terminal = Terminal::ReadyBeforeHeadRelationLoop;
    for (sibling_ordinal, (_, sibling, _)) in accepted.into_iter().enumerate() {
        let first_event_ordinal = events.len();
        let sibling_state = state.beams[&sibling].clone();
        if sibling_state.glyph_identity == base.glyph_identity {
            sibling_traces.push(SiblingTrace {
                sibling_ordinal,
                beam: sibling,
                pair_relations: Vec::new(),
                geometry: None,
                callback: None,
                linker_lookup: None,
                branch: SiblingBranch::SameGlyph,
                first_event_ordinal,
                event_count: 0,
            });
            continue;
        }
        let pair_relations = directed_pair_trace(state, sibling, input.stem);
        if pair_relations
            .iter()
            .any(|relation| relation.matched_beam_stem)
        {
            sibling_traces.push(SiblingTrace {
                sibling_ordinal,
                beam: sibling,
                pair_relations,
                geometry: None,
                callback: None,
                linker_lookup: None,
                branch: SiblingBranch::ExistingBeamStem,
                first_event_ordinal,
                event_count: 0,
            });
            continue;
        }

        let sibling_cross = intersection(input.stem_median, sibling_state.median);
        let sibling_length = sibling_state.median.x2 - sibling_state.median.x1;
        let ratio = sibling_length / base_length;
        let shorter_branch_read = ratio <= input.max_shorter_ratio;
        let (dy, dy_times_y_dir) = if shorter_branch_read {
            let dy = sibling_cross.y - base_cross.y;
            (Some(dy), Some(dy * f64::from(input.y_dir)))
        } else {
            (None, None)
        };
        let mut geometry = GeometryTrace {
            base_cross,
            sibling_cross,
            base_length,
            sibling_length,
            ratio,
            shorter_branch_read,
            dy,
            dy_times_y_dir,
            extension: None,
            left_threshold: None,
            right_threshold: None,
            portion: None,
            grade_bits: None,
        };
        if matches!(dy_times_y_dir, Some(product) if product < 0.0) {
            sibling_traces.push(SiblingTrace {
                sibling_ordinal,
                beam: sibling,
                pair_relations,
                geometry: Some(geometry),
                callback: None,
                linker_lookup: None,
                branch: SiblingBranch::ShorterWrongSide,
                first_event_ordinal,
                event_count: 0,
            });
            continue;
        }

        let extension = Point {
            x: sibling_cross.x,
            y: sibling_cross.y - f64::from(input.y_dir) * (sibling_state.height / 2.0),
        };
        let portion = beam_portion(sibling_state.median, sibling_cross.x, input.portion_max_dx);
        geometry.extension = Some(extension);
        geometry.left_threshold = Some(sibling_state.median.x1 + f64::from(input.portion_max_dx));
        geometry.right_threshold = Some(sibling_state.median.x2 - f64::from(input.portion_max_dx));
        geometry.portion = Some(portion);
        geometry.grade_bits = Some(input.continuation_support_grade_bits);

        let relation_identity = state.next_relation_identity;
        state.next_relation_identity += 1;
        let behavior = input
            .add_edge_behavior
            .get(&sibling)
            .copied()
            .unwrap_or(AddEdgeBehavior::Added);
        if behavior == AddEdgeBehavior::ThrowBeforeInsertion {
            terminal = Terminal::Threw(ThrowStage::AddEdgeBeforeInsertion);
            sibling_traces.push(SiblingTrace {
                sibling_ordinal,
                beam: sibling,
                pair_relations,
                geometry: Some(geometry),
                callback: None,
                linker_lookup: None,
                branch: SiblingBranch::Linked,
                first_event_ordinal,
                event_count: 0,
            });
            break;
        }
        if behavior == AddEdgeBehavior::ReturnedFalse {
            let linker_lookup = lookup_and_assign_linker(sibling, state)?;
            if let (Some(selected_linker), Some(cell), Some(before), Some(after)) = (
                linker_lookup.selected_linker,
                linker_lookup.linked_cell,
                linker_lookup.linked_before,
                linker_lookup.linked_after,
            ) {
                events.push(SerialEvent::LinkerFlagAssigned {
                    sibling_ordinal,
                    selected_linker,
                    cell,
                    ordered_observers: state.linked_cell_observers[&cell].clone(),
                    before,
                    after,
                });
            }
            sibling_traces.push(SiblingTrace {
                sibling_ordinal,
                beam: sibling,
                pair_relations,
                geometry: Some(geometry),
                callback: None,
                linker_lookup: Some(linker_lookup),
                branch: SiblingBranch::Linked,
                first_event_ordinal,
                event_count: events.len() - first_event_ordinal,
            });
            continue;
        }
        state.edges.push(GraphEdge {
            relation_identity,
            source: Vertex::Beam(sibling),
            target: Vertex::Stem(input.stem),
            payload: RelationPayload::BeamStem {
                portion,
                grade_bits: input.continuation_support_grade_bits,
            },
        });
        events.push(SerialEvent::EdgeInserted {
            sibling_ordinal,
            relation_identity,
        });
        if behavior == AddEdgeBehavior::ThrowDuringCallback {
            terminal = Terminal::Threw(ThrowStage::RelationCallbackAfterInsertion);
            sibling_traces.push(SiblingTrace {
                sibling_ordinal,
                beam: sibling,
                pair_relations,
                geometry: Some(geometry),
                callback: None,
                linker_lookup: None,
                branch: SiblingBranch::Linked,
                first_event_ordinal,
                event_count: 1,
            });
            break;
        }
        let callback = complete_relation_callback(sibling, input.stem, state);
        events.push(SerialEvent::CallbackCompleted {
            sibling_ordinal,
            relation_identity,
        });
        let linker_lookup = lookup_and_assign_linker(sibling, state)?;
        if let (Some(selected_linker), Some(cell), Some(before), Some(after)) = (
            linker_lookup.selected_linker,
            linker_lookup.linked_cell,
            linker_lookup.linked_before,
            linker_lookup.linked_after,
        ) {
            events.push(SerialEvent::LinkerFlagAssigned {
                sibling_ordinal,
                selected_linker,
                cell,
                ordered_observers: state.linked_cell_observers[&cell].clone(),
                before,
                after,
            });
        }
        sibling_traces.push(SiblingTrace {
            sibling_ordinal,
            beam: sibling,
            pair_relations,
            geometry: Some(geometry),
            callback: Some(callback),
            linker_lookup: Some(linker_lookup),
            branch: SiblingBranch::Linked,
            first_event_ordinal,
            event_count: events.len() - first_event_ordinal,
        });
    }
    Ok(IndependentTransaction {
        ref_point: input.ref_point,
        discoveries,
        sorted_siblings_before_base_removal,
        base_removal_index,
        sibling_traces,
        events,
        terminal,
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn corpus_page_for_token(page: &str) -> Result<(&'static str, &'static str), String> {
    CORPUS_PAGES
        .iter()
        .copied()
        .find(|(_, image)| page == format!("{image}#1"))
        .ok_or_else(|| format!("unknown Boundary-16 page token {page}"))
}

fn boundary_sixteen_fixture_path(key: &str) -> String {
    format!("rust/oracle/stems-beam-vlink-sibling-links-{key}.txt")
}

fn boundary_seventeen_fixture_path(key: &str) -> String {
    format!("rust/oracle/stems-beam-vlink-head-links-{key}.txt")
}

fn boundary_fifteen_fixture_path(key: &str) -> String {
    format!("rust/oracle/stems-beam-vlink-b-linker-flag-{key}.txt")
}

fn predecessor_fixture_path(family: &str, key: &str) -> String {
    format!("rust/oracle/{family}-{key}.txt")
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

fn read_sha256(relative: &str) -> Result<String, String> {
    let path = repo_root().join(relative);
    let bytes = std::fs::read(&path)
        .map_err(|error| format!("cannot read provenance path {}: {error}", path.display()))?;
    Ok(sha256_hex(&bytes))
}

fn validate_corpus_summary(rows: &[StrictRow], text: &str) -> Result<(), String> {
    let page = rows
        .first()
        .filter(|row| row.kind == RowKind::Page)
        .ok_or_else(|| "corpus validation lacks its page row".to_owned())?;
    let page_summaries = rows
        .iter()
        .filter(|row| row.kind == RowKind::PageSummary)
        .collect::<Vec<_>>();
    let corpus_summaries = rows
        .iter()
        .filter(|row| row.kind == RowKind::CorpusSummary)
        .collect::<Vec<_>>();
    let [page_summary] = page_summaries.as_slice() else {
        return Err("fixture does not contain exactly one page summary".to_owned());
    };
    let [corpus] = corpus_summaries.as_slice() else {
        return Err("fixture does not contain exactly one corpus summary".to_owned());
    };
    let (page_key, image) = corpus_page_for_token(&page.page)?;
    if rows.last() != Some(*corpus)
        || corpus.value("schema")? != "stems-beam-vlink-sibling-links-v1"
        || corpus.value("mode")? != page_key
        || corpus.usize("pages")? != 1
        || corpus.value("pageRefs")? != page.page
        || page.page != format!("{image}#1")
    {
        return Err("corpus page/schema envelope differs".to_owned());
    }

    let expected_row_counts = core_row_counts(rows);
    if corpus.value("rowCounts")? != expected_row_counts {
        return Err("corpus ordered core-family row counts differ".to_owned());
    }

    if corpus.value("pageInputSha256")? != read_sha256(&format!("data/examples/{image}"))?
        || corpus.value("probeSourceSha256")? != read_sha256(PROBE_SOURCE_PATH)?
        || corpus.value("runnerSourceSha256")? != read_sha256(RUNNER_SOURCE_PATH)?
    {
        return Err("corpus page/probe/runner source pin differs".to_owned());
    }
    for (field, path) in CORPUS_SOURCE_PINS {
        if corpus.value(field)? != read_sha256(path)? {
            return Err(format!("corpus active source pin differs for {field}"));
        }
    }
    for (field, path) in CORPUS_GLOBAL_FIXTURE_PINS {
        if corpus.value(field)? != read_sha256(path)? {
            return Err(format!("corpus predecessor pin differs for {field}"));
        }
    }
    for (field, family) in [
        ("schedulerFixtureSha256", "stems-beam-scheduler"),
        ("expandFixtureSha256", "stems-beam-expand"),
        ("createStemFixtureSha256", "stems-beam-create-stem"),
        ("reuseCheckFixtureSha256", "stems-beam-vlink-reuse-check"),
        ("baseApplyFixtureSha256", "stems-beam-vlink-base-apply"),
    ] {
        let path = predecessor_fixture_path(family, page_key);
        if corpus.value(field)? != read_sha256(&path)? {
            return Err(format!("corpus predecessor pin differs for {field}"));
        }
    }
    let b15_path = boundary_fifteen_fixture_path(page_key);
    if corpus.value("bLinkerFlagFixtureSha256")? != read_sha256(&b15_path)? {
        return Err("corpus Boundary-15 fixture pin differs".to_owned());
    }
    for field in [
        "schedulerFixtureSha256",
        "expandFixtureSha256",
        "createStemFixtureSha256",
        "reuseCheckFixtureSha256",
        "baseApplyFixtureSha256",
        "bLinkerFlagFixtureSha256",
    ] {
        if corpus.value(field)? != page.value(field)? {
            return Err(format!("page/corpus predecessor join differs for {field}"));
        }
    }
    for field in [
        "effectiveClasspathSha256",
        "jdkReleaseSha256",
        "javaExecutableSha256",
        "javaJpegLibrarySha256",
        "javaModulesSha256",
        "javaVmLibrarySha256",
        "javaAwtLibrarySha256",
        "javaAwtLwawtLibrarySha256",
        "beamLinkerClassSha256",
        "bLinkerClassSha256",
        "vLinkerClassSha256",
        "stemLinkerClassSha256",
        "jgraphtCoreJarSha256",
    ] {
        if !is_lower_sha256(corpus.value(field)?) {
            return Err(format!("corpus runtime {field} is not lowercase SHA-256"));
        }
    }
    if corpus.value("javaArchitecture")? != "aarch64"
        || corpus.value("javaRuntimeVersion")? != "25.0.3+9-LTS"
        || corpus.value("javaVmVariant")? != "Hotspot"
        || corpus.value("javaImageType")? != "JDK"
        || corpus.value("jgraphtCoreVersion")? != "1.5.2"
        || corpus.value("jgraphtCoreJarSha256")?
            != "dfa596e9f0d0838f1b5e81dd0cd60e3a76c2c290ac25a0a029ffde58cf5e4c14"
        || corpus.value("predecessorReplay")? != "FullBoundary15TypedReplayAndExactJavaRowJoin"
        || corpus.value("querySerialization")? != "UTF8ColonTokensLF-LazyNotReadLiteral"
    {
        return Err("corpus runtime/predecessor constants differ".to_owned());
    }

    let page_summary_marker = format!("{} ", RowKind::PageSummary.label());
    let body_end = text
        .match_indices(&page_summary_marker)
        .find_map(|(index, _)| {
            (index == 0 || text.as_bytes().get(index.wrapping_sub(1)) == Some(&b'\n'))
                .then_some(index)
        })
        .ok_or_else(|| "fixture lacks a line-aligned page summary".to_owned())?;
    let emitted_body = &text.as_bytes()[..body_end];
    let emitted_body_sha = sha256_hex(emitted_body);
    let emitted_body_lines = emitted_body.iter().filter(|byte| **byte == b'\n').count();
    if corpus.value("emittedBodySha256")? != emitted_body_sha
        || corpus.value("rawPassSha256")? != emitted_body_sha
        || corpus.usize("emittedBodyLines")? != emitted_body_lines
        || corpus.usize("emittedBodyBytes")? != emitted_body.len()
    {
        return Err("corpus two-pass emitted-body evidence differs".to_owned());
    }

    let systems = page.usize("systems")?;
    if corpus.usize("freshRunsPerPage")? != 2
        || !corpus.bool("freshRunsByteIdentical")?
        || !corpus.bool("freshJvmPerSystem")?
        || corpus.usize("compilerJavaProcesses")? != 1
        || corpus.usize("runtimeJavaProcessesPerPass")? != systems
        || corpus.usize("runtimeJavaProcesses")? != 2 * systems
        || corpus.usize("totalJavaProcesses")? != 2 * systems + 1
        || corpus.usize("maximumConcurrentJavaProcesses")? != 1
        || corpus.value("concurrencyScope")? != "Boundary16RunnerLockedInvocation"
        || !corpus.bool("compilerJavaProcessReaped")?
        || !corpus.bool("runtimeJavaProcessesReaped")?
        || !corpus.bool("foregroundJavaProcessesOnly")?
        || corpus.usize("backgroundJavaProcessesStarted")? != 0
    {
        return Err("corpus fresh-process/two-pass census differs".to_owned());
    }

    for field in [
        "realTransactions",
        "supportedSyntheticCases",
        "envelopeCases",
        "totalTransactions",
        "siblingCandidates",
        "sameGlyph",
        "existingBeamStem",
        "shorterWrongSide",
        "linked",
        "edgesAdded",
        "linkerWrites",
        "chordStemMatches",
    ] {
        if corpus.value(field)? != page_summary.value(field)? {
            return Err(format!("page/corpus census join differs for {field}"));
        }
    }
    if !corpus.bool("system1SyntheticBlock")?
        || corpus.value("addEdgeReturnedFalseEvidence")? != "IndependentModelOnlyNoStockJavaFixture"
        || corpus.value("envelopeEvidenceScope")? != "JavaOnlyNotProductionEquivalent"
        || !corpus.bool("stopBeforeHeadRelationLoop")?
    {
        return Err("corpus synthetic/envelope/terminal evidence differs".to_owned());
    }
    Ok(())
}

fn validate_real_public_transactions(
    page: &StrictRow,
    transactions: &[ParsedTransaction],
) -> Result<(), String> {
    for transaction in transactions {
        let _ = hydrate_real_boundary_sixteen(page, transaction)?;
    }
    Ok(())
}

fn exactly_one_row(rows: &[StrictRow], kind: RowKind) -> Result<&StrictRow, String> {
    let matches = rows
        .iter()
        .filter(|row| row.kind == kind)
        .collect::<Vec<_>>();
    let [row] = matches.as_slice() else {
        return Err(format!(
            "fixture contains {} {kind:?} rows, expected one",
            matches.len()
        ));
    };
    Ok(row)
}

fn compare_manifest_and_strict_fields(
    manifest: &ManifestRow,
    strict: &StrictRow,
    fields: &[&str],
) -> Result<(), String> {
    for field in fields {
        if manifest.value(field)? != strict.value(field)? {
            return Err(format!(
                "manifest/fixture {} join differs for {field}",
                manifest.value("page").unwrap_or("summary")
            ));
        }
    }
    Ok(())
}

fn validate_boundary_sixteen_manifest(path: &std::path::Path) -> Result<(), String> {
    let manifest_bytes =
        std::fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let manifest = std::str::from_utf8(&manifest_bytes)
        .map_err(|error| format!("{} is not UTF-8: {error}", path.display()))?;
    if !manifest_bytes.ends_with(b"\n") {
        return Err("Boundary-16 manifest must end with one newline".to_owned());
    }
    let lines = manifest.lines().collect::<Vec<_>>();
    if sha256_hex(&manifest_bytes) != MANIFEST_SHA256
        || lines.len() != MANIFEST_LINES
        || manifest_bytes.len() != MANIFEST_BYTES
        || MANIFEST_ENTRY_FIELDS.len() != 54
        || MANIFEST_SUMMARY_FIELDS.len() != 128
        || lines.len() != CORPUS_PAGES.len() + 2
        || lines.first().copied() != Some(MANIFEST_SCHEMA)
    {
        return Err("Boundary-16 manifest schema/line envelope differs".to_owned());
    }
    let entries = lines[1..=CORPUS_PAGES.len()]
        .iter()
        .map(|line| ManifestRow::parse(line, MANIFEST_ENTRY_LABEL, MANIFEST_ENTRY_FIELDS))
        .collect::<Result<Vec<_>, _>>()?;
    let summary = ManifestRow::parse(
        lines
            .last()
            .ok_or_else(|| "Boundary-16 manifest lacks summary".to_owned())?,
        MANIFEST_SUMMARY_LABEL,
        MANIFEST_SUMMARY_FIELDS,
    )?;

    let expected_header = format!("{}\n", FIXTURE_HEADER.join("\n"));
    let mut normalized_corpus = expected_header.as_bytes().to_vec();
    let mut all_rows = Vec::new();
    let mut split_fixture_lines = 0;
    let mut split_fixture_bytes = 0;
    let mut semantic_rows = 0;
    let mut first_corpus = None;
    let mut real_systems = 0;
    let mut real_transactions = 0;
    let mut group_rows_total = 0;
    let mut group_glyph_rows_total = 0;
    let mut null_group_glyphs_total = 0;
    let mut real_sibling_candidates = 0;
    let mut real_same_glyph = 0;
    let mut real_existing_beam_stem = 0;
    let mut real_shorter_wrong_side = 0;
    let mut real_linked = 0;
    let mut real_edges_added = 0;
    let mut real_linker_writes = 0;
    let mut real_events = 0;
    let mut chord_stem_matches = 0;
    let mut supported_synthetic_cases = 0;
    let mut envelope_cases = 0;
    let mut total_transactions = 0;
    let mut same_glyph = 0;
    let mut existing_beam_stem = 0;
    let mut shorter_wrong_side = 0;
    let mut linked = 0;
    let mut edges_added = 0;
    let mut linker_writes = 0;
    let mut events = 0;
    let mut compiler_java_processes = 0;
    let mut runtime_java_processes = 0;
    let mut total_java_processes = 0;
    let mut maximum_concurrent_java_processes = 0;

    for (ordinal, ((page_key, _), entry)) in CORPUS_PAGES.iter().zip(&entries).enumerate() {
        let expected_fixture = format!("stems-beam-vlink-sibling-links-{page_key}.txt");
        if entry.usize("ordinal")? != ordinal
            || entry.value("page")? != *page_key
            || entry.value("fixture")? != expected_fixture
        {
            return Err(format!("manifest entry {ordinal} identity/order differs"));
        }
        let fixture_path = repo_root().join("rust/oracle").join(&expected_fixture);
        let fixture_bytes = std::fs::read(&fixture_path)
            .map_err(|error| format!("cannot read {}: {error}", fixture_path.display()))?;
        let fixture = std::str::from_utf8(&fixture_bytes)
            .map_err(|error| format!("{} is not UTF-8: {error}", fixture_path.display()))?;
        let rows = parse_scaffold_fixture(fixture)?;
        let transactions = validate_core_rows(&rows)?;
        validate_corpus_summary(&rows, fixture)?;
        validate_boundary_fifteen_predecessors(&rows[0], &transactions)?;
        let page = exactly_one_row(&rows, RowKind::Page)?;
        let page_summary = exactly_one_row(&rows, RowKind::PageSummary)?;
        let corpus = exactly_one_row(&rows, RowKind::CorpusSummary)?;
        first_corpus.get_or_insert_with(|| corpus.clone());

        if entry.value("rowCounts")? != core_row_counts(&rows)
            || entry.usize("systems")? != page.usize("systems")?
        {
            return Err(format!("manifest {page_key} row/system census differs"));
        }
        compare_manifest_and_strict_fields(
            entry,
            page_summary,
            &[
                "realTransactions",
                "supportedSyntheticCases",
                "envelopeCases",
                "totalTransactions",
                "groupRows",
                "siblingCandidates",
                "sameGlyph",
                "existingBeamStem",
                "shorterWrongSide",
                "linked",
                "edgesAdded",
                "linkerWrites",
                "events",
                "chordStemMatches",
            ],
        )?;
        compare_manifest_and_strict_fields(
            entry,
            corpus,
            &[
                "pageInputSha256",
                "schedulerFixtureSha256",
                "expandFixtureSha256",
                "createStemFixtureSha256",
                "reuseCheckFixtureSha256",
                "baseApplyFixtureSha256",
                "baseApplyManifestSha256",
                "bLinkerFlagFixtureSha256",
                "bLinkerFlagManifestSha256",
                "emittedBodySha256",
                "emittedBodyLines",
                "emittedBodyBytes",
                "rawPassSha256",
                "freshRunsPerPage",
                "freshRunsByteIdentical",
                "compilerJavaProcesses",
                "runtimeJavaProcessesPerPass",
                "runtimeJavaProcesses",
                "totalJavaProcesses",
                "maximumConcurrentJavaProcesses",
                "concurrencyScope",
                "freshJvmPerSystem",
                "compilerJavaProcessReaped",
                "runtimeJavaProcessesReaped",
                "foregroundJavaProcessesOnly",
                "backgroundJavaProcessesStarted",
                "system1SyntheticBlock",
                "addEdgeReturnedFalseEvidence",
                "envelopeEvidenceScope",
                "stopBeforeHeadRelationLoop",
            ],
        )?;

        let group_rows = rows
            .iter()
            .filter(|row| row.kind == RowKind::GroupMember)
            .collect::<Vec<_>>();
        let group_glyph_rows = group_rows
            .iter()
            .filter(|row| {
                row.value("containmentMatch") == Ok("true") && row.value("glyph") != Ok("-")
            })
            .count();
        let null_group_glyphs = group_rows
            .iter()
            .filter(|row| row.value("glyph") == Ok("null"))
            .count();
        if entry.usize("groupRows")? != group_rows.len()
            || entry.usize("groupGlyphRows")? != group_glyph_rows
            || entry.usize("nullGroupGlyphs")? != null_group_glyphs
        {
            return Err(format!("manifest {page_key} group/glyph census differs"));
        }

        let fixture_lines = fixture_bytes.iter().filter(|byte| **byte == b'\n').count();
        if entry.value("fixtureSha256")? != sha256_hex(&fixture_bytes)
            || entry.usize("fixtureLines")? != fixture_lines
            || entry.usize("fixtureBytes")? != fixture_bytes.len()
        {
            return Err(format!("manifest {page_key} fixture identity differs"));
        }
        if !fixture.starts_with(&expected_header) {
            return Err(format!("manifest {page_key} shared header differs"));
        }
        for line in fixture.lines().skip(FIXTURE_HEADER.len()) {
            if line.starts_with(RowKind::PageSummary.label())
                || line.starts_with(RowKind::CorpusSummary.label())
            {
                continue;
            }
            normalized_corpus.extend_from_slice(line.as_bytes());
            normalized_corpus.push(b'\n');
            semantic_rows += 1;
        }
        split_fixture_lines += fixture_lines;
        split_fixture_bytes += fixture_bytes.len();
        all_rows.extend(rows.iter().cloned());

        real_systems += entry.usize("systems")?;
        real_transactions += entry.usize("realTransactions")?;
        group_rows_total += group_rows.len();
        group_glyph_rows_total += group_glyph_rows;
        null_group_glyphs_total += null_group_glyphs;
        supported_synthetic_cases += entry.usize("supportedSyntheticCases")?;
        envelope_cases += entry.usize("envelopeCases")?;
        total_transactions += entry.usize("totalTransactions")?;
        same_glyph += entry.usize("sameGlyph")?;
        existing_beam_stem += entry.usize("existingBeamStem")?;
        shorter_wrong_side += entry.usize("shorterWrongSide")?;
        linked += entry.usize("linked")?;
        edges_added += entry.usize("edgesAdded")?;
        linker_writes += entry.usize("linkerWrites")?;
        events += entry.usize("events")?;
        chord_stem_matches += entry.usize("chordStemMatches")?;
        compiler_java_processes += entry.usize("compilerJavaProcesses")?;
        runtime_java_processes += entry.usize("runtimeJavaProcesses")?;
        total_java_processes += entry.usize("totalJavaProcesses")?;
        maximum_concurrent_java_processes =
            maximum_concurrent_java_processes.max(entry.usize("maximumConcurrentJavaProcesses")?);
        for row in rows.iter().filter(|row| row.kind == RowKind::Summary) {
            real_sibling_candidates += row.usize("siblings")?;
            real_same_glyph += row.usize("sameGlyph")?;
            real_existing_beam_stem += row.usize("existingBeamStem")?;
            real_shorter_wrong_side += row.usize("shorterWrongSide")?;
            real_linked += row.usize("linked")?;
            real_edges_added += row.usize("edgesAdded")?;
            real_linker_writes += row.usize("linkerWrites")?;
            real_events += row.usize("events")?;
        }
    }

    if summary.value("schema")? != "stems-beam-vlink-sibling-links-manifest-v1"
        || summary.usize("entries")? != entries.len()
    {
        return Err("manifest summary schema/entry count differs".to_owned());
    }
    let first_corpus = first_corpus.ok_or_else(|| "manifest has no corpus rows".to_owned())?;
    for field in MANIFEST_SUMMARY_FIELDS
        .iter()
        .skip(2)
        .take_while(|field| **field != "sharedHeaderSha256")
    {
        let expected = first_corpus.value(field)?;
        if summary.value(field)? != expected {
            return Err(format!("manifest shared provenance differs for {field}"));
        }
        for (page_key, _) in CORPUS_PAGES {
            let path = repo_root().join(boundary_sixteen_fixture_path(page_key));
            let fixture = std::fs::read_to_string(&path)
                .map_err(|error| format!("cannot reread {}: {error}", path.display()))?;
            let rows = parse_scaffold_fixture(&fixture)?;
            if exactly_one_row(&rows, RowKind::CorpusSummary)?.value(field)? != expected {
                return Err(format!("manifest per-page provenance differs for {field}"));
            }
        }
    }

    if summary.value("sharedHeaderSha256")? != sha256_hex(expected_header.as_bytes())
        || summary.usize("sharedHeaderLines")? != FIXTURE_HEADER.len()
        || summary.usize("sharedHeaderBytes")? != expected_header.len()
        || summary.value("corpusBodySha256")? != sha256_hex(&normalized_corpus)
        || summary.usize("corpusBodyLines")?
            != normalized_corpus
                .iter()
                .filter(|byte| **byte == b'\n')
                .count()
        || summary.usize("corpusBodyBytes")? != normalized_corpus.len()
        || summary.value("corpusRowCounts")? != core_row_counts(&all_rows)
        || summary.value("corpusReconstruction")? != "SharedHeaderOnceThenPageSemanticRows"
        || summary.usize("semanticRows")? != semantic_rows
        || summary.usize("splitFixtureLines")? != split_fixture_lines
        || summary.usize("splitFixtureBytes")? != split_fixture_bytes
        || sha256_hex(&normalized_corpus) != NORMALIZED_CORPUS_SHA256
        || normalized_corpus
            .iter()
            .filter(|byte| **byte == b'\n')
            .count()
            != NORMALIZED_CORPUS_LINES
        || normalized_corpus.len() != NORMALIZED_CORPUS_BYTES
        || split_fixture_lines != SPLIT_FIXTURE_LINES
        || split_fixture_bytes != SPLIT_FIXTURE_BYTES
    {
        return Err("manifest corpus reconstruction differs".to_owned());
    }

    for (field, actual) in [
        ("realSystems", real_systems),
        ("realTransactions", real_transactions),
        ("groupRows", group_rows_total),
        ("groupGlyphRows", group_glyph_rows_total),
        ("nullGroupGlyphs", null_group_glyphs_total),
        ("realSiblingCandidates", real_sibling_candidates),
        ("realSameGlyph", real_same_glyph),
        ("realExistingBeamStem", real_existing_beam_stem),
        ("realShorterWrongSide", real_shorter_wrong_side),
        ("realLinked", real_linked),
        ("realEdgesAdded", real_edges_added),
        ("realLinkerWrites", real_linker_writes),
        ("realEvents", real_events),
        ("chordStemMatches", chord_stem_matches),
        ("syntheticBlocks", entries.len()),
        ("supportedSyntheticCases", supported_synthetic_cases),
        ("envelopeCases", envelope_cases),
        ("isolatedCases", supported_synthetic_cases + envelope_cases),
        ("totalTransactions", total_transactions),
        ("sameGlyph", same_glyph),
        ("existingBeamStem", existing_beam_stem),
        ("shorterWrongSide", shorter_wrong_side),
        ("linked", linked),
        ("edgesAdded", edges_added),
        ("linkerWrites", linker_writes),
        ("events", events),
        ("compilerJavaProcesses", compiler_java_processes),
        ("runtimeJavaProcesses", runtime_java_processes),
        ("totalJavaProcesses", total_java_processes),
        (
            "maximumConcurrentJavaProcesses",
            maximum_concurrent_java_processes,
        ),
    ] {
        if summary.usize(field)? != actual {
            return Err(format!("manifest aggregate {field} differs"));
        }
    }
    if summary.value("concurrencyScope")? != "Boundary16RunnerLockedInvocation"
        || summary.usize("freshRunsPerPage")? != 2
        || !summary.bool("freshRunsByteIdentical")?
        || !summary.bool("freshJvmPerSystem")?
        || !summary.bool("compilerJavaProcessesReaped")?
        || !summary.bool("runtimeJavaProcessesReaped")?
        || !summary.bool("foregroundJavaProcessesOnly")?
        || summary.usize("backgroundJavaProcessesStarted")? != 0
        || !summary.bool("system1SyntheticBlock")?
        || summary.value("addEdgeReturnedFalseEvidence")?
            != "IndependentModelOnlyNoStockJavaFixture"
        || summary.value("supplementalEvidenceScope")?
            != "IsolatedJavaGateOnlyNotProductionEquivalent"
        || !summary.bool("stopBeforeHeadRelationLoop")?
    {
        return Err("manifest execution/supplemental constants differ".to_owned());
    }

    let summary_marker = format!("{MANIFEST_SUMMARY_LABEL} ");
    let body_end = manifest
        .match_indices(&summary_marker)
        .find_map(|(index, _)| {
            (index == 0 || manifest_bytes.get(index.wrapping_sub(1)) == Some(&b'\n'))
                .then_some(index)
        })
        .ok_or_else(|| "manifest lacks a line-aligned summary".to_owned())?;
    let body = &manifest_bytes[..body_end];
    if summary.value("manifestBodySha256")? != sha256_hex(body)
        || summary.usize("manifestBodyLines")? != body.iter().filter(|byte| **byte == b'\n').count()
        || summary.usize("manifestBodyBytes")? != body.len()
        || sha256_hex(body) != MANIFEST_BODY_SHA256
        || body.iter().filter(|byte| **byte == b'\n').count() != MANIFEST_BODY_LINES
        || body.len() != MANIFEST_BODY_BYTES
    {
        return Err("manifest self-pinned body differs".to_owned());
    }
    Ok(())
}

fn validate_installed_boundary_sixteen_fixture(path: &std::path::Path) -> Result<(), String> {
    let bytes =
        std::fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{} is not UTF-8: {error}", path.display()))?;
    let rows = parse_scaffold_fixture(text)?;
    let transactions = validate_core_rows(&rows)?;
    validate_corpus_summary(&rows, text)?;
    validate_boundary_fifteen_predecessors(&rows[0], &transactions)?;
    validate_real_public_transactions(&rows[0], &transactions)?;
    Ok(())
}

const HEAD_LINKS_COMMON_FIELDS: &[&str] = &["system", "plan", "scope", "case"];
const HEAD_LINKS_ENTRY_COMMON_FIELDS: &[&str] = &["system", "plan", "scope", "case", "mapOrdinal"];
const HEAD_LINKS_PAGE_FIELDS: &[&str] = &[
    "systems",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "createStemFixtureSha256",
    "reuseCheckFixtureSha256",
    "baseApplyFixtureSha256",
    "bLinkerFlagFixtureSha256",
    "siblingLinksFixtureSha256",
    "executionMode",
    "relationOrder",
    "incidentOrder",
    "headless",
    "methodDispatch",
    "stop",
];
const HEAD_LINKS_PREDECESSOR_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "join",
    "b16TransactionRows",
    "b16TransactionEvidenceSha256",
    "b16ResultRowSha256",
    "b16GuardRowSha256",
    "b16SummaryRowSha256",
    "predecessorTerminal",
    "stemAlias",
    "stemInterId",
    "relationInputHash",
    "javaPredecessorStateSha256",
    "proofDomain",
];
const HEAD_LINKS_BASELINE_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "interline",
    "neutralStemLength",
    "relationEntries",
    "relationInputHash",
    "headPlanStateSha256",
    "stemAlias",
    "stemInterId",
    "stemManual",
    "stemAbnormal",
    "graphVertices",
    "graphVertexSha256",
    "graphEdges",
    "graphEdgeSha256",
    "interIndexEntries",
    "interIndexSha256",
    "builderItems",
    "builderItemsSha256",
    "lastIndex",
    "maxIndex",
    "listenerTopologySha256",
    "soleSigListener",
    "headless",
    "stubModified",
    "bookModified",
    "bookDirty",
    "eventStart",
];
const HEAD_LINKS_HEAD_ENTRY_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "mapOrdinal",
    "cAlias",
    "cRuntimeClass",
    "evidenceTiming",
    "sAlias",
    "sRuntimeClass",
    "horizontalSide",
    "verticalSide",
    "observerAliases",
    "headAlias",
    "headSigOrdinal",
    "headXOrdinal",
    "headRuntimeClass",
    "headInterId",
    "headIndexOrdinal",
    "headIndexObjectMatches",
    "headIndexIdMatches",
    "headVertexOrdinal",
    "headVertexObjectMatches",
    "headSigSystemId",
    "shape",
    "small",
    "stemHead",
    "glyph",
    "staff",
    "center",
    "manual",
    "removed",
    "vip",
    "abnormalBefore",
    "sLinkedBefore",
    "sClosedBefore",
    "planDraftIdentity",
    "draftRuntimeClass",
    "draftManual",
    "draftHeadSide",
    "draftExtension",
    "draftConsistencyBeforeState",
    "draftConsistencyBeforeValue",
    "draftDx",
    "draftDy",
    "draftGrade",
    "draftImpacts",
    "draftGraphMatches",
    "branch",
];
const HEAD_LINKS_S_WRITE_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "mapOrdinal",
    "eventOrdinal",
    "receiverAlias",
    "receiverRuntimeClass",
    "declaringClass",
    "requested",
    "before",
    "after",
    "writeCount",
    "valueChangeCount",
    "closedBefore",
    "closedAfter",
    "observerAliases",
    "completed",
];
const HEAD_LINKS_SOURCE_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "mapOrdinal",
    "sourceOutgoingOrdinal",
    "graphRelationIdentity",
    "relationObjectIdentity",
    "runtimeClass",
    "targetVertexOrdinal",
];
const HEAD_LINKS_PAIR_RELATION_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "mapOrdinal",
    "pairOrdinal",
    "sourceOutgoingOrdinal",
    "graphRelationIdentity",
    "relationObjectIdentity",
    "runtimeClass",
    "classRead",
    "matches",
    "action",
];
const HEAD_LINKS_PAIR_SCAN_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "mapOrdinal",
    "state",
    "sourceOutgoingCount",
    "sourceOutgoingSha256",
    "pairCount",
    "pairSha256",
    "selectedGraphRelationIdentity",
    "selectedRelationObjectIdentity",
    "selectedRuntimeClass",
];
const HEAD_LINKS_CONSISTENCY_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "mapOrdinal",
    "eventOrdinal",
    "shapeRead",
    "shape",
    "small",
    "stemMedianRead",
    "stemMedian",
    "interlineRead",
    "interline",
    "scaledStemLength",
    "neutralStemLength",
    "ratio",
    "consistencyBeforeState",
    "consistencyBeforeValue",
    "consistencyAfterState",
    "consistencyAfterValue",
    "debugEnabled",
    "completed",
];
const HEAD_LINKS_EDGE_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "mapOrdinal",
    "eventOrdinal",
    "graphRelationIdentity",
    "relationObjectIdentity",
    "runtimeClass",
    "sourceAlias",
    "sourceInterId",
    "sourceVertexOrdinal",
    "targetAlias",
    "targetInterId",
    "targetVertexOrdinal",
    "graphInsertionOrdinal",
    "insertionReturned",
    "callbackSynchronous",
];
const HEAD_LINKS_INCIDENT_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "mapOrdinal",
    "incidentOrdinal",
    "direction",
    "directionOrdinal",
    "graphRelationIdentity",
    "relationObjectIdentity",
    "runtimeClass",
    "oppositeAlias",
    "oppositeInterId",
    "oppositeVertexOrdinal",
    "classRead",
    "matches",
    "action",
];
const HEAD_LINKS_INCIDENT_SCAN_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "mapOrdinal",
    "state",
    "incidentCount",
    "incidentSha256",
    "requestedAbnormal",
];
const HEAD_LINKS_CALLBACK_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "mapOrdinal",
    "eventOrdinal",
    "relationObjectIdentity",
    "headSideWasNull",
    "headSideAfter",
    "extensionWasNull",
    "extensionAfter",
    "relationManual",
    "headManualRead",
    "headManual",
    "stemManualRead",
    "stemManual",
    "chordBranch",
    "headScanState",
    "headAbnormalRequested",
    "stemScanState",
    "stemAbnormalRequested",
    "headAbnormalBefore",
    "headAbnormalAfter",
    "stemAbnormalBefore",
    "stemAbnormalAfter",
    "stubModifiedBefore",
    "stubModifiedAfter",
    "bookModifiedBefore",
    "bookModifiedAfter",
    "bookDirtyBefore",
    "bookDirtyAfter",
    "completed",
];
const HEAD_LINKS_ENTRY_RESULT_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "mapOrdinal",
    "branch",
    "sLinkedAfter",
    "sClosedAfter",
    "draftConsistencyAfterState",
    "draftConsistencyAfterValue",
    "graphRelationIdentity",
    "relationObjectIdentity",
    "insertionReturned",
    "callbackCompleted",
    "headAbnormalAfter",
    "stemAbnormalAfter",
    "relationStateBeforeSha256",
    "relationStateAfterSha256",
];
const HEAD_LINKS_REMAINDER_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "lastIndex",
    "maxIndex",
    "builderItemCount",
    "comparisonEvaluated",
    "remainderPresent",
    "splitBody",
    "splitCalls",
    "returnedTrue",
];
const HEAD_LINKS_RESULT_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "headEntries",
    "duplicateEntries",
    "relationsInserted",
    "sWriteCount",
    "sValueChangeCount",
    "consistencyWriteCount",
    "headAbnormalChangeCount",
    "stemAbnormalChangeCount",
    "dirtyCascadeCount",
    "sheetEditMutationCount",
    "eventCount",
    "returnedTrue",
    "terminal",
    "relationInputHash",
    "headPlanStateSha256",
    "stateAfterSha256",
];
const HEAD_LINKS_GUARD_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "allocatorUnchanged",
    "glyphRegistryUnchanged",
    "interIndexIdentityUnchanged",
    "sigVertexIdentityOrderUnchanged",
    "baselineEdgeIdentityOrderUnchanged",
    "appendedEdgesExactly",
    "systemStemsMapIdentityUnchanged",
    "beamLinesUnchanged",
    "builderItemsUnchanged",
    "relationMapIdentityOrderUnchanged",
    "legacyRelationInputPayloadHashChanged",
    "onlySelectedSCellsMayChange",
    "onlyHeadStemAbnormalMayChange",
    "manualChordBranchRead",
    "splitCalls",
    "outerBLinkerAssignmentRead",
    "stopBeforeOuterBLinkerAssignment",
    "headPlanStateChangedByConsistency",
];
const HEAD_LINKS_SUMMARY_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "headEntries",
    "duplicateEntries",
    "relationsInserted",
    "sWrites",
    "returnedTrue",
    "terminal",
];
const HEAD_LINKS_SYNTHETIC_CASE_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "join",
    "sourceRealB16EvidenceSha256",
    "construction",
    "shape",
    "small",
    "stemAttachedBefore",
    "soleSigListener",
    "sLinkedBefore",
    "sLinkedAfter",
    "sClosedBefore",
    "sClosedAfter",
    "pairState",
    "selectedGraphRelationIdentity",
    "selectedRuntimeClass",
    "draftConsistencyBefore",
    "draftConsistencyAfter",
    "scaledStemLength",
    "neutralStemLength",
    "expectedConsistency",
    "headSideBefore",
    "headSideAfter",
    "extensionBefore",
    "extensionAfter",
    "draftAttachedBefore",
    "draftAttachedAfter",
    "relationManual",
    "headManual",
    "stemManual",
    "sideFallbackTaken",
    "extensionFallbackTaken",
    "manualBranchRead",
    "chordBranchRead",
    "headAbnormalRead",
    "stemAbnormalRead",
    "graphEdgesBefore",
    "graphEdgesAfter",
    "headStemBefore",
    "headStemAfter",
    "chordStemBefore",
    "chordStemAfter",
    "oldChordStemRetained",
    "newChordStemCount",
    "chordTargetsNewStem",
    "addEdgeReturned",
    "callbackState",
    "headAbnormalBefore",
    "headAbnormalAfter",
    "stemAbnormalBefore",
    "stemAbnormalAfter",
    "dirtyBefore",
    "dirtyAfter",
    "throwClass",
    "throwStage",
    "eventCount",
    "terminal",
];
const HEAD_LINKS_SYNTHETIC_EVENT_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "eventOrdinal",
    "kind",
    "relationIdentity",
];
const HEAD_LINKS_SYNTHETIC_GUARD_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "graphDelta",
    "allowedMutations",
    "headPayloadUnchanged",
    "stemPayloadUnchanged",
    "closedFlagsUnchanged",
    "unrelatedGraphPreserved",
    "isolatedOnly",
    "productionEquivalent",
    "enclosingRealSheetUnchanged",
    "outerBLinkerAssignmentRead",
    "terminal",
];
const HEAD_LINKS_PAGE_SUMMARY_FIELDS: &[&str] = &[
    "systems",
    "realTransactions",
    "supportedSyntheticCases",
    "envelopeCases",
    "isolatedCases",
    "totalTransactions",
    "headEntries",
    "duplicateEntries",
    "relationsInserted",
    "sWrites",
    "sValueChanges",
    "consistencyWrites",
    "headAbnormalChanges",
    "stemAbnormalChanges",
    "dirtyCascades",
    "sheetEditMutations",
    "realEvents",
    "isolatedEvents",
    "isolatedGraphDelta",
    "isolatedThrows",
    "isolatedManualCases",
    "chordRewires",
    "stopBeforeOuterBLinkerAssignment",
];
const HEAD_LINKS_CORPUS_SUMMARY_FIELDS: &[&str] = &[
    "schema",
    "mode",
    "pages",
    "pageRefs",
    "rowCounts",
    "pageInputSha256",
    "probeSourceSha256",
    "runnerSourceSha256",
    "effectiveClasspathSha256",
    "jdkReleaseSha256",
    "javaExecutableSha256",
    "javaJpegLibrarySha256",
    "javaModulesSha256",
    "javaVmLibrarySha256",
    "javaAwtLibrarySha256",
    "javaAwtLwawtLibrarySha256",
    "javaArchitecture",
    "javaRuntimeVersion",
    "javaVmVariant",
    "javaImageType",
    "beamLinkerClassSha256",
    "bLinkerClassSha256",
    "vLinkerClassSha256",
    "stemLinkerClassSha256",
    "headLinkerClassSha256",
    "sLinkerClassSha256",
    "cLinkerClassSha256",
    "headInterClassSha256",
    "stemInterClassSha256",
    "headStemRelationClassSha256",
    "headChordInterClassSha256",
    "chordStemRelationClassSha256",
    "partClassSha256",
    "sigraphClassSha256",
    "beamLinkerSourceSha256",
    "stemLinkerSourceSha256",
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
    "smallBeamInterSourceSha256",
    "beamBeamRelationSourceSha256",
    "sheetStubSourceSha256",
    "bookSourceSha256",
    "horizontalSideSourceSha256",
    "verticalSideSourceSha256",
    "stemBuilderSourceSha256",
    "stemItemSourceSha256",
    "stemsRetrieverSourceSha256",
    "beamGroupInterSourceSha256",
    "ensembleHelperSourceSha256",
    "containmentSourceSha256",
    "abstractStemConnectionSourceSha256",
    "beamPortionSourceSha256",
    "scaleSourceSha256",
    "skewSourceSha256",
    "staffSourceSha256",
    "staffLineSourceSha256",
    "lineInfoSourceSha256",
    "lineUtilSourceSha256",
    "headLinkerSourceSha256",
    "headInterSourceSha256",
    "headStemRelationSourceSha256",
    "partSourceSha256",
    "gradleSourceSha256",
    "jgraphtCoreVersion",
    "jgraphtCoreJarSha256",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "createStemFixtureSha256",
    "reuseCheckFixtureSha256",
    "baseApplyFixtureSha256",
    "baseApplyManifestSha256",
    "bLinkerFlagFixtureSha256",
    "bLinkerFlagManifestSha256",
    "siblingLinksFixtureSha256",
    "siblingLinksManifestSha256",
    "predecessorReplay",
    "querySerialization",
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
    "concurrencyScope",
    "compilerJavaProcessReaped",
    "runtimeJavaProcessesReaped",
    "foregroundJavaProcessesOnly",
    "backgroundJavaProcessesStarted",
    "realTransactions",
    "supportedSyntheticCases",
    "envelopeCases",
    "isolatedCases",
    "totalTransactions",
    "headEntries",
    "duplicateEntries",
    "relationsInserted",
    "sWrites",
    "sValueChanges",
    "consistencyWrites",
    "headAbnormalChanges",
    "stemAbnormalChanges",
    "dirtyCascades",
    "sheetEditMutations",
    "realEvents",
    "isolatedEvents",
    "isolatedGraphDelta",
    "isolatedThrows",
    "isolatedManualCases",
    "chordRewires",
    "system1IsolatedBlock",
    "supportedSyntheticEvidenceScope",
    "envelopeEvidenceScope",
    "stopBeforeOuterBLinkerAssignment",
];
const HEAD_LINKS_MANIFEST_ENTRY_FIELDS: &[&str] = &[
    "ordinal",
    "page",
    "fixture",
    "rowCounts",
    "systems",
    "realTransactions",
    "supportedSyntheticCases",
    "envelopeCases",
    "isolatedCases",
    "totalTransactions",
    "headEntries",
    "duplicateEntries",
    "relationsInserted",
    "sWrites",
    "sValueChanges",
    "consistencyWrites",
    "headAbnormalChanges",
    "stemAbnormalChanges",
    "dirtyCascades",
    "sheetEditMutations",
    "realEvents",
    "isolatedEvents",
    "isolatedGraphDelta",
    "isolatedThrows",
    "isolatedManualCases",
    "chordRewires",
    "pageInputSha256",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "createStemFixtureSha256",
    "reuseCheckFixtureSha256",
    "baseApplyFixtureSha256",
    "baseApplyManifestSha256",
    "bLinkerFlagFixtureSha256",
    "bLinkerFlagManifestSha256",
    "siblingLinksFixtureSha256",
    "siblingLinksManifestSha256",
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
    "concurrencyScope",
    "freshJvmPerSystem",
    "compilerJavaProcessReaped",
    "runtimeJavaProcessesReaped",
    "foregroundJavaProcessesOnly",
    "backgroundJavaProcessesStarted",
    "system1IsolatedBlock",
    "supportedSyntheticEvidenceScope",
    "envelopeEvidenceScope",
    "stopBeforeOuterBLinkerAssignment",
];
const HEAD_LINKS_MANIFEST_SUMMARY_FIELDS: &[&str] = &[
    "schema",
    "entries",
    "probeSourceSha256",
    "runnerSourceSha256",
    "effectiveClasspathSha256",
    "jdkReleaseSha256",
    "javaExecutableSha256",
    "javaJpegLibrarySha256",
    "javaModulesSha256",
    "javaVmLibrarySha256",
    "javaAwtLibrarySha256",
    "javaAwtLwawtLibrarySha256",
    "javaArchitecture",
    "javaRuntimeVersion",
    "javaVmVariant",
    "javaImageType",
    "beamLinkerClassSha256",
    "bLinkerClassSha256",
    "vLinkerClassSha256",
    "stemLinkerClassSha256",
    "headLinkerClassSha256",
    "sLinkerClassSha256",
    "cLinkerClassSha256",
    "headInterClassSha256",
    "stemInterClassSha256",
    "headStemRelationClassSha256",
    "headChordInterClassSha256",
    "chordStemRelationClassSha256",
    "partClassSha256",
    "sigraphClassSha256",
    "beamLinkerSourceSha256",
    "stemLinkerSourceSha256",
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
    "smallBeamInterSourceSha256",
    "beamBeamRelationSourceSha256",
    "sheetStubSourceSha256",
    "bookSourceSha256",
    "horizontalSideSourceSha256",
    "verticalSideSourceSha256",
    "stemBuilderSourceSha256",
    "stemItemSourceSha256",
    "stemsRetrieverSourceSha256",
    "beamGroupInterSourceSha256",
    "ensembleHelperSourceSha256",
    "containmentSourceSha256",
    "abstractStemConnectionSourceSha256",
    "beamPortionSourceSha256",
    "scaleSourceSha256",
    "skewSourceSha256",
    "staffSourceSha256",
    "staffLineSourceSha256",
    "lineInfoSourceSha256",
    "lineUtilSourceSha256",
    "headLinkerSourceSha256",
    "headInterSourceSha256",
    "headStemRelationSourceSha256",
    "partSourceSha256",
    "gradleSourceSha256",
    "jgraphtCoreVersion",
    "jgraphtCoreJarSha256",
    "baseApplyManifestSha256",
    "bLinkerFlagManifestSha256",
    "siblingLinksManifestSha256",
    "predecessorReplay",
    "querySerialization",
    "sharedHeaderSha256",
    "sharedHeaderLines",
    "sharedHeaderBytes",
    "corpusBodySha256",
    "corpusBodyLines",
    "corpusBodyBytes",
    "corpusRowCounts",
    "corpusReconstruction",
    "semanticRows",
    "splitEmittedBodySha256",
    "splitEmittedBodyLines",
    "splitEmittedBodyBytes",
    "splitFixtureSha256",
    "splitFixtureLines",
    "splitFixtureBytes",
    "realSystems",
    "realTransactions",
    "realHeadEntries",
    "realDuplicateEntries",
    "realRelationsInserted",
    "realSWrites",
    "realSValueChanges",
    "realConsistencyWrites",
    "realHeadAbnormalChanges",
    "realStemAbnormalChanges",
    "realDirtyCascades",
    "realSheetEditMutations",
    "realEvents",
    "syntheticBlocks",
    "supportedSyntheticCases",
    "envelopeCases",
    "isolatedCases",
    "totalTransactions",
    "isolatedEvents",
    "isolatedGraphDelta",
    "isolatedThrows",
    "isolatedManualCases",
    "chordRewires",
    "compilerJavaProcesses",
    "runtimeJavaProcesses",
    "totalJavaProcesses",
    "maximumConcurrentJavaProcesses",
    "concurrencyScope",
    "freshRunsPerPage",
    "freshRunsByteIdentical",
    "freshJvmPerSystem",
    "compilerJavaProcessesReaped",
    "runtimeJavaProcessesReaped",
    "foregroundJavaProcessesOnly",
    "backgroundJavaProcessesStarted",
    "system1IsolatedBlock",
    "supportedSyntheticEvidenceScope",
    "envelopeEvidenceScope",
    "stopBeforeOuterBLinkerAssignment",
    "manifestBodySha256",
    "manifestBodyLines",
    "manifestBodyBytes",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HeadRowKind {
    Page,
    Predecessor,
    Baseline,
    HeadEntry,
    SLinkerWrite,
    SourceOutgoing,
    PairRelation,
    PairScan,
    Consistency,
    Edge,
    HeadIncident,
    HeadIncidentScan,
    StemIncident,
    StemIncidentScan,
    Callback,
    EntryResult,
    Remainder,
    Result,
    DeltaGuard,
    Summary,
    SyntheticCase,
    SyntheticEvent,
    SyntheticGuard,
    PageSummary,
    CorpusSummary,
}

impl HeadRowKind {
    fn label(self) -> &'static str {
        match self {
            Self::Page => "stemsbeamvlinkheadlinkspage",
            Self::Predecessor => "stemsbeamvlinkheadlinkspredecessor",
            Self::Baseline => "stemsbeamvlinkheadlinksbaseline",
            Self::HeadEntry => "stemsbeamvlinkheadlinksheadentry",
            Self::SLinkerWrite => "stemsbeamvlinkheadlinksslinkerwrite",
            Self::SourceOutgoing => "stemsbeamvlinkheadlinkssourceoutgoing",
            Self::PairRelation => "stemsbeamvlinkheadlinkspairrelation",
            Self::PairScan => "stemsbeamvlinkheadlinkspairscan",
            Self::Consistency => "stemsbeamvlinkheadlinksconsistency",
            Self::Edge => "stemsbeamvlinkheadlinksedge",
            Self::HeadIncident => "stemsbeamvlinkheadlinksheadincident",
            Self::HeadIncidentScan => "stemsbeamvlinkheadlinksheadincidentscan",
            Self::StemIncident => "stemsbeamvlinkheadlinksstemincident",
            Self::StemIncidentScan => "stemsbeamvlinkheadlinksstemincidentscan",
            Self::Callback => "stemsbeamvlinkheadlinkscallback",
            Self::EntryResult => "stemsbeamvlinkheadlinksentryresult",
            Self::Remainder => "stemsbeamvlinkheadlinksremainder",
            Self::Result => "stemsbeamvlinkheadlinksresult",
            Self::DeltaGuard => "stemsbeamvlinkheadlinksdeltaguard",
            Self::Summary => "stemsbeamvlinkheadlinkssummary",
            Self::SyntheticCase => "stemsbeamvlinkheadlinkssyntheticcase",
            Self::SyntheticEvent => "stemsbeamvlinkheadlinkssyntheticevent",
            Self::SyntheticGuard => "stemsbeamvlinkheadlinkssyntheticguard",
            Self::PageSummary => "stemsbeamvlinkheadlinkspagesummary",
            Self::CorpusSummary => "stemsbeamvlinkheadlinkscorpussummary",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        Some(match label {
            "stemsbeamvlinkheadlinkspage" => Self::Page,
            "stemsbeamvlinkheadlinkspredecessor" => Self::Predecessor,
            "stemsbeamvlinkheadlinksbaseline" => Self::Baseline,
            "stemsbeamvlinkheadlinksheadentry" => Self::HeadEntry,
            "stemsbeamvlinkheadlinksslinkerwrite" => Self::SLinkerWrite,
            "stemsbeamvlinkheadlinkssourceoutgoing" => Self::SourceOutgoing,
            "stemsbeamvlinkheadlinkspairrelation" => Self::PairRelation,
            "stemsbeamvlinkheadlinkspairscan" => Self::PairScan,
            "stemsbeamvlinkheadlinksconsistency" => Self::Consistency,
            "stemsbeamvlinkheadlinksedge" => Self::Edge,
            "stemsbeamvlinkheadlinksheadincident" => Self::HeadIncident,
            "stemsbeamvlinkheadlinksheadincidentscan" => Self::HeadIncidentScan,
            "stemsbeamvlinkheadlinksstemincident" => Self::StemIncident,
            "stemsbeamvlinkheadlinksstemincidentscan" => Self::StemIncidentScan,
            "stemsbeamvlinkheadlinkscallback" => Self::Callback,
            "stemsbeamvlinkheadlinksentryresult" => Self::EntryResult,
            "stemsbeamvlinkheadlinksremainder" => Self::Remainder,
            "stemsbeamvlinkheadlinksresult" => Self::Result,
            "stemsbeamvlinkheadlinksdeltaguard" => Self::DeltaGuard,
            "stemsbeamvlinkheadlinkssummary" => Self::Summary,
            "stemsbeamvlinkheadlinkssyntheticcase" => Self::SyntheticCase,
            "stemsbeamvlinkheadlinkssyntheticevent" => Self::SyntheticEvent,
            "stemsbeamvlinkheadlinkssyntheticguard" => Self::SyntheticGuard,
            "stemsbeamvlinkheadlinkspagesummary" => Self::PageSummary,
            "stemsbeamvlinkheadlinkscorpussummary" => Self::CorpusSummary,
            _ => return None,
        })
    }

    fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Page => HEAD_LINKS_PAGE_FIELDS,
            Self::Predecessor => HEAD_LINKS_PREDECESSOR_FIELDS,
            Self::Baseline => HEAD_LINKS_BASELINE_FIELDS,
            Self::HeadEntry => HEAD_LINKS_HEAD_ENTRY_FIELDS,
            Self::SLinkerWrite => HEAD_LINKS_S_WRITE_FIELDS,
            Self::SourceOutgoing => HEAD_LINKS_SOURCE_FIELDS,
            Self::PairRelation => HEAD_LINKS_PAIR_RELATION_FIELDS,
            Self::PairScan => HEAD_LINKS_PAIR_SCAN_FIELDS,
            Self::Consistency => HEAD_LINKS_CONSISTENCY_FIELDS,
            Self::Edge => HEAD_LINKS_EDGE_FIELDS,
            Self::HeadIncident | Self::StemIncident => HEAD_LINKS_INCIDENT_FIELDS,
            Self::HeadIncidentScan | Self::StemIncidentScan => HEAD_LINKS_INCIDENT_SCAN_FIELDS,
            Self::Callback => HEAD_LINKS_CALLBACK_FIELDS,
            Self::EntryResult => HEAD_LINKS_ENTRY_RESULT_FIELDS,
            Self::Remainder => HEAD_LINKS_REMAINDER_FIELDS,
            Self::Result => HEAD_LINKS_RESULT_FIELDS,
            Self::DeltaGuard => HEAD_LINKS_GUARD_FIELDS,
            Self::Summary => HEAD_LINKS_SUMMARY_FIELDS,
            Self::SyntheticCase => HEAD_LINKS_SYNTHETIC_CASE_FIELDS,
            Self::SyntheticEvent => HEAD_LINKS_SYNTHETIC_EVENT_FIELDS,
            Self::SyntheticGuard => HEAD_LINKS_SYNTHETIC_GUARD_FIELDS,
            Self::PageSummary => HEAD_LINKS_PAGE_SUMMARY_FIELDS,
            Self::CorpusSummary => HEAD_LINKS_CORPUS_SUMMARY_FIELDS,
        }
    }
}

#[derive(Clone, Debug)]
struct HeadStrictRow {
    kind: HeadRowKind,
    page: String,
    values: BTreeMap<String, String>,
}

impl HeadStrictRow {
    fn value(&self, field: &str) -> Result<&str, String> {
        self.values
            .get(field)
            .map(String::as_str)
            .ok_or_else(|| format!("missing Boundary-17 {field} field"))
    }

    fn usize(&self, field: &str) -> Result<usize, String> {
        self.value(field)?
            .parse()
            .map_err(|error| format!("invalid Boundary-17 {field}: {error}"))
    }

    fn bool(&self, field: &str) -> Result<bool, String> {
        match self.value(field)? {
            "true" => Ok(true),
            "false" => Ok(false),
            value => Err(format!("invalid Boundary-17 Boolean {field}={value}")),
        }
    }

    fn i32(&self, field: &str) -> Result<i32, String> {
        self.value(field)?
            .parse()
            .map_err(|error| format!("invalid Boundary-17 {field}: {error}"))
    }

    fn key(&self) -> Result<TransactionKey, String> {
        Ok(TransactionKey {
            page: self.page.clone(),
            system: self.usize("system")?,
            plan: self.usize("plan")?,
            scope: self.value("scope")?.to_owned(),
            case_name: self.value("case")?.to_owned(),
        })
    }
}

fn parse_head_links_rows(text: &str) -> Result<Vec<HeadStrictRow>, String> {
    let mut lines = text.lines();
    let header = lines.by_ref().take(8).collect::<Vec<_>>();
    if header.len() != 8 || header.get(1).copied() != Some(HEAD_LINKS_FIXTURE_SCHEMA) {
        return Err("Boundary-17 fixture header/schema differs".to_owned());
    }
    let mut rows = Vec::new();
    for (offset, line) in lines.enumerate() {
        if line.is_empty() {
            return Err(format!("empty Boundary-17 semantic line {}", offset + 9));
        }
        let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
        if tokens.len() < 2 {
            return Err(format!("short Boundary-17 semantic line {}", offset + 9));
        }
        let kind = HeadRowKind::from_label(tokens[0]).ok_or_else(|| {
            format!(
                "unknown Boundary-17 row {} at line {}",
                tokens[0],
                offset + 9
            )
        })?;
        let expected = kind.fields();
        let pair_start = if kind == HeadRowKind::CorpusSummary {
            1
        } else {
            2
        };
        if tokens.len() != pair_start + expected.len() * 2 {
            return Err(format!(
                "Boundary-17 {kind:?} line {} has {} tokens, expected {}",
                offset + 9,
                tokens.len(),
                pair_start + expected.len() * 2
            ));
        }
        let mut values = BTreeMap::new();
        for (ordinal, pair) in tokens[pair_start..].chunks_exact(2).enumerate() {
            if pair[0] != expected[ordinal] {
                return Err(format!(
                    "Boundary-17 {kind:?} line {} field {} is {}, expected {}",
                    offset + 9,
                    ordinal,
                    pair[0],
                    expected[ordinal]
                ));
            }
            if values
                .insert(pair[0].to_owned(), pair[1].to_owned())
                .is_some()
            {
                return Err(format!(
                    "duplicate Boundary-17 field {} at line {}",
                    pair[0],
                    offset + 9
                ));
            }
        }
        rows.push(HeadStrictRow {
            kind,
            page: if kind == HeadRowKind::CorpusSummary {
                "-".to_owned()
            } else {
                tokens[1].to_owned()
            },
            values,
        });
    }
    Ok(rows)
}

#[derive(Clone, Debug)]
struct ParsedHeadEntry {
    head: HeadStrictRow,
    s_write: HeadStrictRow,
    source_outgoing: Vec<HeadStrictRow>,
    pair_relations: Vec<HeadStrictRow>,
    pair_scan: HeadStrictRow,
    consistency: Option<HeadStrictRow>,
    edge: Option<HeadStrictRow>,
    head_incidents: Vec<HeadStrictRow>,
    head_incident_scan: Option<HeadStrictRow>,
    stem_incidents: Vec<HeadStrictRow>,
    stem_incident_scan: Option<HeadStrictRow>,
    callback: Option<HeadStrictRow>,
    result: HeadStrictRow,
}

#[derive(Clone, Debug)]
struct ParsedHeadTransaction {
    key: TransactionKey,
    predecessor: HeadStrictRow,
    baseline: HeadStrictRow,
    entries: Vec<ParsedHeadEntry>,
    remainder: HeadStrictRow,
    result: HeadStrictRow,
    guard: HeadStrictRow,
    summary: HeadStrictRow,
}

#[derive(Clone, Debug)]
struct ParsedSyntheticHeadCase {
    key: TransactionKey,
    case: HeadStrictRow,
    events: Vec<HeadStrictRow>,
    guard: HeadStrictRow,
}

fn require_head_key(row: &HeadStrictRow, key: &TransactionKey) -> Result<(), String> {
    if &row.key()? != key {
        return Err(format!(
            "Boundary-17 transaction key drift at {:?}",
            row.kind
        ));
    }
    Ok(())
}

fn parse_real_head_transactions(
    rows: &[HeadStrictRow],
) -> Result<
    (
        &HeadStrictRow,
        Vec<ParsedHeadTransaction>,
        Vec<ParsedSyntheticHeadCase>,
    ),
    String,
> {
    let (page, semantic) = rows
        .split_first()
        .ok_or_else(|| "Boundary-17 fixture lacks page row".to_owned())?;
    if page.kind != HeadRowKind::Page {
        return Err("Boundary-17 first semantic row is not page".to_owned());
    }
    let mut index = 0;
    let mut transactions: Vec<ParsedHeadTransaction> = Vec::new();
    let mut supplemental = Vec::new();
    while index < semantic.len() {
        if semantic[index].kind == HeadRowKind::SyntheticCase {
            if transactions.len() != 1 || transactions[0].key.system != 1 {
                return Err(
                    "Boundary-17 supplemental block is not immediately after system 1".to_owned(),
                );
            }
            let case = semantic[index].clone();
            let key = case.key()?;
            if key.system != 1 || key.plan != transactions[0].key.plan {
                return Err("Boundary-17 supplemental key differs from system 1".to_owned());
            }
            index += 1;
            let mut events = Vec::new();
            while semantic.get(index).is_some_and(|row| {
                row.kind == HeadRowKind::SyntheticEvent
                    && row.values.get("scope") == Some(&key.scope)
                    && row.values.get("case") == Some(&key.case_name)
            }) {
                let row = semantic[index].clone();
                require_head_key(&row, &key)?;
                if row.usize("eventOrdinal")? != events.len() {
                    return Err(format!(
                        "Boundary-17 supplemental {} event order differs",
                        key.case_name
                    ));
                }
                events.push(row);
                index += 1;
            }
            let guard = semantic
                .get(index)
                .ok_or_else(|| format!("Boundary-17 supplemental {} lacks guard", key.case_name))?
                .clone();
            if guard.kind != HeadRowKind::SyntheticGuard {
                return Err(format!(
                    "Boundary-17 supplemental {} guard order differs",
                    key.case_name
                ));
            }
            require_head_key(&guard, &key)?;
            index += 1;
            supplemental.push(ParsedSyntheticHeadCase {
                key,
                case,
                events,
                guard,
            });
            continue;
        }
        let predecessor = semantic[index].clone();
        if predecessor.kind != HeadRowKind::Predecessor {
            return Err(format!(
                "Boundary-17 expected predecessor at transaction row {index}, found {:?}",
                predecessor.kind
            ));
        }
        let key = predecessor.key()?;
        if key.scope != "real" || key.case_name != "-" {
            break;
        }
        index += 1;
        let baseline = semantic
            .get(index)
            .ok_or_else(|| "Boundary-17 transaction lacks baseline".to_owned())?
            .clone();
        if baseline.kind != HeadRowKind::Baseline {
            return Err("Boundary-17 predecessor is not followed by baseline".to_owned());
        }
        require_head_key(&baseline, &key)?;
        index += 1;
        let expected_entries = baseline.usize("relationEntries")?;
        let mut entries = Vec::with_capacity(expected_entries);
        for map_ordinal in 0..expected_entries {
            let head = semantic
                .get(index)
                .ok_or_else(|| "Boundary-17 transaction lacks head entry".to_owned())?
                .clone();
            if head.kind != HeadRowKind::HeadEntry || head.usize("mapOrdinal")? != map_ordinal {
                return Err(format!("Boundary-17 head entry {map_ordinal} order drift"));
            }
            require_head_key(&head, &key)?;
            index += 1;
            let s_write = semantic
                .get(index)
                .ok_or_else(|| "Boundary-17 head entry lacks S write".to_owned())?
                .clone();
            if s_write.kind != HeadRowKind::SLinkerWrite
                || s_write.usize("mapOrdinal")? != map_ordinal
            {
                return Err(format!("Boundary-17 S write {map_ordinal} order drift"));
            }
            require_head_key(&s_write, &key)?;
            index += 1;

            let mut source_outgoing = Vec::new();
            while semantic.get(index).is_some_and(|row| {
                row.kind == HeadRowKind::SourceOutgoing
                    && row.values.get("mapOrdinal") == Some(&map_ordinal.to_string())
            }) {
                let row = semantic[index].clone();
                require_head_key(&row, &key)?;
                if row.usize("sourceOutgoingOrdinal")? != source_outgoing.len() {
                    return Err(format!(
                        "Boundary-17 source-outgoing ordinal drift at map {map_ordinal}"
                    ));
                }
                source_outgoing.push(row);
                index += 1;
            }
            let mut pair_relations = Vec::new();
            while semantic.get(index).is_some_and(|row| {
                row.kind == HeadRowKind::PairRelation
                    && row.values.get("mapOrdinal") == Some(&map_ordinal.to_string())
            }) {
                let row = semantic[index].clone();
                require_head_key(&row, &key)?;
                if row.usize("pairOrdinal")? != pair_relations.len() {
                    return Err(format!(
                        "Boundary-17 pair ordinal drift at map {map_ordinal}"
                    ));
                }
                pair_relations.push(row);
                index += 1;
            }
            let pair_scan = semantic
                .get(index)
                .ok_or_else(|| "Boundary-17 entry lacks pair scan".to_owned())?
                .clone();
            if pair_scan.kind != HeadRowKind::PairScan
                || pair_scan.usize("mapOrdinal")? != map_ordinal
            {
                return Err(format!("Boundary-17 pair scan {map_ordinal} order drift"));
            }
            require_head_key(&pair_scan, &key)?;
            if pair_scan.usize("sourceOutgoingCount")? != source_outgoing.len()
                || pair_scan.usize("pairCount")? != pair_relations.len()
            {
                return Err(format!("Boundary-17 pair scan {map_ordinal} count drift"));
            }
            index += 1;

            let linked = head.value("branch")? == "Linked";
            let mut consistency = None;
            let mut edge = None;
            let mut head_incidents = Vec::new();
            let mut head_incident_scan = None;
            let mut stem_incidents = Vec::new();
            let mut stem_incident_scan = None;
            let mut callback = None;
            if linked {
                for (kind, slot) in [
                    (HeadRowKind::Consistency, &mut consistency),
                    (HeadRowKind::Edge, &mut edge),
                ] {
                    let row = semantic
                        .get(index)
                        .ok_or_else(|| format!("Boundary-17 linked entry lacks {kind:?}"))?
                        .clone();
                    if row.kind != kind || row.usize("mapOrdinal")? != map_ordinal {
                        return Err(format!(
                            "Boundary-17 linked entry {map_ordinal} {kind:?} order drift"
                        ));
                    }
                    require_head_key(&row, &key)?;
                    *slot = Some(row);
                    index += 1;
                }
                while semantic.get(index).is_some_and(|row| {
                    row.kind == HeadRowKind::HeadIncident
                        && row.values.get("mapOrdinal") == Some(&map_ordinal.to_string())
                }) {
                    let row = semantic[index].clone();
                    require_head_key(&row, &key)?;
                    if row.usize("incidentOrdinal")? != head_incidents.len() {
                        return Err(format!(
                            "Boundary-17 head incident ordinal drift at map {map_ordinal}"
                        ));
                    }
                    head_incidents.push(row);
                    index += 1;
                }
                let row = semantic
                    .get(index)
                    .ok_or_else(|| "Boundary-17 linked entry lacks head scan".to_owned())?
                    .clone();
                if row.kind != HeadRowKind::HeadIncidentScan
                    || row.usize("mapOrdinal")? != map_ordinal
                {
                    return Err(format!("Boundary-17 head scan {map_ordinal} order drift"));
                }
                require_head_key(&row, &key)?;
                if row.usize("incidentCount")? != head_incidents.len() {
                    return Err(format!("Boundary-17 head scan {map_ordinal} count drift"));
                }
                head_incident_scan = Some(row);
                index += 1;
                while semantic.get(index).is_some_and(|row| {
                    row.kind == HeadRowKind::StemIncident
                        && row.values.get("mapOrdinal") == Some(&map_ordinal.to_string())
                }) {
                    let row = semantic[index].clone();
                    require_head_key(&row, &key)?;
                    if row.usize("incidentOrdinal")? != stem_incidents.len() {
                        return Err(format!(
                            "Boundary-17 stem incident ordinal drift at map {map_ordinal}"
                        ));
                    }
                    stem_incidents.push(row);
                    index += 1;
                }
                let row = semantic
                    .get(index)
                    .ok_or_else(|| "Boundary-17 linked entry lacks stem scan".to_owned())?
                    .clone();
                if row.kind != HeadRowKind::StemIncidentScan
                    || row.usize("mapOrdinal")? != map_ordinal
                {
                    return Err(format!("Boundary-17 stem scan {map_ordinal} order drift"));
                }
                require_head_key(&row, &key)?;
                if row.usize("incidentCount")? != stem_incidents.len() {
                    return Err(format!("Boundary-17 stem scan {map_ordinal} count drift"));
                }
                stem_incident_scan = Some(row);
                index += 1;
                let row = semantic
                    .get(index)
                    .ok_or_else(|| "Boundary-17 linked entry lacks callback".to_owned())?
                    .clone();
                if row.kind != HeadRowKind::Callback || row.usize("mapOrdinal")? != map_ordinal {
                    return Err(format!("Boundary-17 callback {map_ordinal} order drift"));
                }
                require_head_key(&row, &key)?;
                callback = Some(row);
                index += 1;
            } else if head.value("branch")? != "ExistingHeadStem" {
                return Err(format!(
                    "unknown Boundary-17 branch {}",
                    head.value("branch")?
                ));
            }
            let result = semantic
                .get(index)
                .ok_or_else(|| "Boundary-17 entry lacks result".to_owned())?
                .clone();
            if result.kind != HeadRowKind::EntryResult
                || result.usize("mapOrdinal")? != map_ordinal
                || result.value("branch")? != head.value("branch")?
            {
                return Err(format!(
                    "Boundary-17 entry result {map_ordinal} order drift"
                ));
            }
            require_head_key(&result, &key)?;
            index += 1;
            entries.push(ParsedHeadEntry {
                head,
                s_write,
                source_outgoing,
                pair_relations,
                pair_scan,
                consistency,
                edge,
                head_incidents,
                head_incident_scan,
                stem_incidents,
                stem_incident_scan,
                callback,
                result,
            });
        }
        let take_terminal =
            |index: &mut usize, expected: HeadRowKind| -> Result<HeadStrictRow, String> {
                let row = semantic
                    .get(*index)
                    .ok_or_else(|| format!("Boundary-17 transaction lacks {expected:?}"))?
                    .clone();
                if row.kind != expected {
                    return Err(format!(
                        "Boundary-17 expected {expected:?}, found {:?}",
                        row.kind
                    ));
                }
                require_head_key(&row, &key)?;
                *index += 1;
                Ok(row)
            };
        let remainder = take_terminal(&mut index, HeadRowKind::Remainder)?;
        let result = take_terminal(&mut index, HeadRowKind::Result)?;
        let guard = take_terminal(&mut index, HeadRowKind::DeltaGuard)?;
        let summary = take_terminal(&mut index, HeadRowKind::Summary)?;
        transactions.push(ParsedHeadTransaction {
            key,
            predecessor,
            baseline,
            entries,
            remainder,
            result,
            guard,
            summary,
        });
    }
    Ok((page, transactions, supplemental))
}

fn parse_head_side(value: &str) -> Result<NativeStemHeadSide, String> {
    match value {
        "LEFT" => Ok(NativeStemHeadSide::Left),
        "RIGHT" => Ok(NativeStemHeadSide::Right),
        _ => Err(format!("invalid Boundary-17 horizontal side {value}")),
    }
}

fn parse_head_vertical(value: &str) -> Result<NativeStemVerticalSide, String> {
    match value {
        "TOP" => Ok(NativeStemVerticalSide::Top),
        "BOTTOM" => Ok(NativeStemVerticalSide::Bottom),
        _ => Err(format!("invalid Boundary-17 vertical side {value}")),
    }
}

fn parse_head_center(value: &str) -> Result<(i32, i32), String> {
    let values = value.split(':').collect::<Vec<_>>();
    let [x, y] = values.as_slice() else {
        return Err(format!("invalid Boundary-17 head center {value}"));
    };
    Ok((
        x.parse()
            .map_err(|error| format!("invalid Boundary-17 center x: {error}"))?,
        y.parse()
            .map_err(|error| format!("invalid Boundary-17 center y: {error}"))?,
    ))
}

fn parse_head_relation_object(
    value: &str,
) -> Result<NativeStemsBeamHeadRelationObjectIdentity, String> {
    if let Some(value) = value.strip_prefix("sig-relation-object:") {
        return value
            .parse()
            .map(NativeStemsBeamHeadRelationObjectIdentity::GraphObject)
            .map_err(|error| format!("invalid Boundary-17 graph object identity: {error}"));
    }
    if let Some(value) = value.strip_prefix("base-draft:") {
        return value
            .parse()
            .map(NativeStemsBeamHeadRelationObjectIdentity::BaseDraft)
            .map_err(|error| format!("invalid Boundary-17 base draft identity: {error}"));
    }
    if let Some(value) = value.strip_prefix("sibling-draft:") {
        let values = value.split(':').collect::<Vec<_>>();
        let [plan, sibling] = values.as_slice() else {
            return Err(format!("invalid Boundary-17 sibling draft {value}"));
        };
        return Ok(NativeStemsBeamHeadRelationObjectIdentity::SiblingDraft {
            plan_ordinal: plan
                .parse()
                .map_err(|error| format!("invalid sibling draft plan: {error}"))?,
            sibling_ordinal: sibling
                .parse()
                .map_err(|error| format!("invalid sibling draft ordinal: {error}"))?,
        });
    }
    if let Some(value) = value.strip_prefix("head-draft:") {
        let values = value.split(':').collect::<Vec<_>>();
        let [plan, map] = values.as_slice() else {
            return Err(format!("invalid Boundary-17 head draft {value}"));
        };
        return Ok(NativeStemsBeamHeadRelationObjectIdentity::HeadDraft {
            plan_ordinal: plan
                .parse()
                .map_err(|error| format!("invalid head draft plan: {error}"))?,
            map_ordinal: map
                .parse()
                .map_err(|error| format!("invalid head draft map ordinal: {error}"))?,
        });
    }
    Err(format!("unknown Boundary-17 relation object {value}"))
}

fn head_query_kind(class: &str) -> NativeStemsBeamHeadQueryRelationKind {
    if class == "org.audiveris.omr.sig.relation.HeadStemRelation" {
        NativeStemsBeamHeadQueryRelationKind::HeadStem
    } else {
        NativeStemsBeamHeadQueryRelationKind::Other
    }
}

fn parse_head_pair_read(row: &HeadStrictRow) -> Result<NativeStemsBeamHeadPairClassRead, String> {
    match (
        row.bool("classRead")?,
        row.bool("matches")?,
        row.value("action")?,
    ) {
        (true, false, "Continue") => Ok(NativeStemsBeamHeadPairClassRead::ExaminedContinue),
        (true, true, "SelectBreak") => Ok(NativeStemsBeamHeadPairClassRead::ExaminedMatchBreak),
        (false, false, "UnreadAfterBreak") => {
            Ok(NativeStemsBeamHeadPairClassRead::UnreadAfterBreak)
        }
        values => Err(format!(
            "invalid Boundary-17 pair class-read tuple {values:?}"
        )),
    }
}

fn parse_head_incident_read(
    row: &HeadStrictRow,
) -> Result<NativeStemsBeamHeadIncidentClassRead, String> {
    match (
        row.bool("classRead")?,
        row.bool("matches")?,
        row.value("action")?,
    ) {
        (true, false, "Continue") => Ok(NativeStemsBeamHeadIncidentClassRead::ExaminedContinue),
        (true, true, "SelectBreak") => Ok(NativeStemsBeamHeadIncidentClassRead::ExaminedMatchBreak),
        (false, false, "UnreadAfterBreak") => {
            Ok(NativeStemsBeamHeadIncidentClassRead::UnreadAfterBreak)
        }
        values => Err(format!(
            "invalid Boundary-17 incident class-read tuple {values:?}"
        )),
    }
}

fn parse_head_direction(value: &str) -> Result<NativeStemsBeamIncidentDirection, String> {
    match value {
        "Incoming" => Ok(NativeStemsBeamIncidentDirection::Incoming),
        "Outgoing" => Ok(NativeStemsBeamIncidentDirection::Outgoing),
        _ => Err(format!("invalid Boundary-17 incident direction {value}")),
    }
}

fn head_ref_from_entry(
    row: &HeadStrictRow,
    relation: &audiveris_omr::native_stems_beam_link_plans::NativeStemsBeamHeadRelation,
) -> Result<NativeStemsBeamHeadLinkHeadRef, String> {
    if row.usize("headSigOrdinal")? != relation.corner.sig_ordinal
        || row.usize("headXOrdinal")? != relation.corner.x_ordinal
        || parse_head_side(row.value("horizontalSide")?)? != relation.corner.horizontal
        || parse_head_vertical(row.value("verticalSide")?)? != relation.corner.vertical
        || row.value("cAlias")?
            != format!(
                "h:{}:{}:{}",
                relation.corner.x_ordinal,
                row.value("horizontalSide")?,
                row.value("verticalSide")?
            )
        || row.value("headAlias")? != format!("head:{}", relation.corner.x_ordinal)
    {
        return Err(format!(
            "Boundary-17 map {} head/corner alias differs from typed plan",
            relation.map_ordinal
        ));
    }
    Ok(NativeStemsBeamHeadLinkHeadRef {
        reference: relation.corner.head,
        sig_ordinal: relation.corner.sig_ordinal,
        x_ordinal: relation.corner.x_ordinal,
    })
}

fn head_plan_attempt(
    hydrated: &HydratedBoundarySixteen,
) -> Result<&audiveris_omr::native_stems_beam_link_plans::NativeStemsBeamLinkPlanAttempt, String> {
    let plan = hydrated.transaction.key.plan;
    let builder = hydrated
        .predecessor
        .plans
        .builders
        .get(plan.builder_ordinal)
        .ok_or_else(|| "Boundary-17 plan builder ordinal is absent".to_owned())?;
    let attempts = builder
        .attempts
        .iter()
        .filter(|attempt| attempt.stem_profile == plan.stem_profile)
        .collect::<Vec<_>>();
    let [attempt] = attempts.as_slice() else {
        return Err(format!(
            "Boundary-17 plan has {} matching stem-profile attempts",
            attempts.len()
        ));
    };
    Ok(attempt)
}

fn hydrate_boundary_sixteen_for_head(
    head_page: &HeadStrictRow,
    transaction: &ParsedHeadTransaction,
) -> Result<HydratedBoundarySixteen, String> {
    let (page_key, _) = corpus_page_for_token(&head_page.page)?;
    let path = boundary_sixteen_fixture_path(page_key);
    let bytes = std::fs::read(repo_root().join(&path))
        .map_err(|error| format!("cannot read Boundary-16 {page_key} fixture: {error}"))?;
    if head_page.value("siblingLinksFixtureSha256")? != sha256_hex(&bytes) {
        return Err("Boundary-17 page does not pin its exact Boundary-16 fixture".to_owned());
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| format!("Boundary-16 {page_key} fixture is not UTF-8: {error}"))?;
    let rows = parse_scaffold_fixture(text)?;
    let b16_transactions = validate_core_rows(&rows)?;
    validate_corpus_summary(&rows, text)?;
    validate_boundary_fifteen_predecessors(&rows[0], &b16_transactions)?;
    let matches = b16_transactions
        .iter()
        .filter(|candidate| {
            candidate.key.system == transaction.key.system
                && candidate.key.plan == transaction.key.plan
                && candidate.key.scope == "real"
                && candidate.key.case_name == "-"
        })
        .collect::<Vec<_>>();
    let [b16] = matches.as_slice() else {
        return Err(format!(
            "Boundary-17 key has {} exact Boundary-16 transaction matches",
            matches.len()
        ));
    };

    let mut ordered_lines = Vec::new();
    let mut result_line = None;
    let mut guard_line = None;
    let mut summary_line = None;
    for line in text.lines() {
        if !line.starts_with("stemsbeamvlinksiblinglinks")
            || line.starts_with("stemsbeamvlinksiblinglinkspage ")
            || line.starts_with("stemsbeamvlinksiblinglinkspagesummary ")
            || line.starts_with("stemsbeamvlinksiblinglinkscorpussummary ")
            || line.contains("synthetic")
        {
            continue;
        }
        let row = StrictRow::parse(line)?;
        let Ok(key) = row.key() else {
            continue;
        };
        if key.system != transaction.key.system
            || key.plan != transaction.key.plan
            || key.scope != "real"
            || key.case_name != "-"
        {
            continue;
        }
        ordered_lines.push(line.to_owned());
        match row.kind {
            RowKind::Result => result_line = Some(line),
            RowKind::DeltaGuard => guard_line = Some(line),
            RowKind::Summary => summary_line = Some(line),
            _ => {}
        }
    }
    let row_sha = |line: Option<&str>, family: &str| -> Result<String, String> {
        line.map(|line| sha256_rows([line.to_owned()]))
            .ok_or_else(|| format!("Boundary-16 transaction lacks {family} row"))
    };
    let result_sha = row_sha(result_line, "result")?;
    let guard_sha = row_sha(guard_line, "guard")?;
    let summary_sha = row_sha(summary_line, "summary")?;
    if transaction.predecessor.value("join")? != "FullBoundary16Replay"
        || transaction.predecessor.usize("b16TransactionRows")? != ordered_lines.len()
        || transaction
            .predecessor
            .value("b16TransactionEvidenceSha256")?
            != sha256_rows(ordered_lines)
        || transaction.predecessor.value("b16ResultRowSha256")? != result_sha
        || transaction.predecessor.value("b16GuardRowSha256")? != guard_sha
        || transaction.predecessor.value("b16SummaryRowSha256")? != summary_sha
        || transaction.predecessor.value("predecessorTerminal")? != "ReadyBeforeHeadRelationLoop"
        || transaction.predecessor.value("proofDomain")? != "JavaOpaqueGuardRustFullTypedReplay"
    {
        return Err("Boundary-17 exact Boundary-16 row bundle join differs".to_owned());
    }
    let b16_result = only_transaction_row(b16, RowKind::Result)?;
    let b16_guard = only_transaction_row(b16, RowKind::DeltaGuard)?;
    if transaction.predecessor.value("stemAlias")? != b16_result.value("stemAlias")?
        || transaction.predecessor.value("stemInterId")? != b16_result.value("stemInterId")?
        || transaction.predecessor.value("relationInputHash")?
            != b16_guard.value("relationInputHashAfter")?
        || transaction.baseline.value("relationInputHash")?
            != transaction.predecessor.value("relationInputHash")?
    {
        return Err("Boundary-17 semantic Boundary-16 predecessor join differs".to_owned());
    }
    hydrate_real_boundary_sixteen(&rows[0], b16)
}

fn parse_head_outgoing_rows(
    entry: &ParsedHeadEntry,
) -> Result<Vec<NativeStemsBeamHeadSourceOutgoingRelation>, String> {
    entry
        .source_outgoing
        .iter()
        .map(|row| {
            Ok(NativeStemsBeamHeadSourceOutgoingRelation {
                source_outgoing_ordinal: row.usize("sourceOutgoingOrdinal")?,
                graph_relation_identity: parse_sig_edge(row.value("graphRelationIdentity")?)?,
                relation_object_identity: parse_head_relation_object(
                    row.value("relationObjectIdentity")?,
                )?,
                relation_class: row.value("runtimeClass")?.to_owned(),
                target_vertex_identity: row.usize("targetVertexOrdinal")?,
            })
        })
        .collect()
}

fn parse_head_pair_rows(
    entry: &ParsedHeadEntry,
) -> Result<Vec<NativeStemsBeamHeadPairRelation>, String> {
    entry
        .pair_relations
        .iter()
        .map(|row| {
            let relation_class = row.value("runtimeClass")?.to_owned();
            Ok(NativeStemsBeamHeadPairRelation {
                pair_ordinal: row.usize("pairOrdinal")?,
                source_outgoing_ordinal: row.usize("sourceOutgoingOrdinal")?,
                graph_relation_identity: parse_sig_edge(row.value("graphRelationIdentity")?)?,
                relation_object_identity: parse_head_relation_object(
                    row.value("relationObjectIdentity")?,
                )?,
                kind: head_query_kind(&relation_class),
                relation_class,
                class_read: parse_head_pair_read(row)?,
            })
        })
        .collect()
}

fn parse_head_incident_rows(
    rows: &[HeadStrictRow],
    head_aliases: &BTreeMap<String, NativeStemsBeamHeadLinkHeadRef>,
    stem_alias: &str,
) -> Result<Vec<NativeStemsBeamHeadIncidentRelation>, String> {
    rows.iter()
        .map(|row| {
            let opposite_alias = row.value("oppositeAlias")?.to_owned();
            let opposite = if opposite_alias == stem_alias {
                NativeStemsBeamHeadIncidentOpposite::Stem
            } else if let Some(head) = head_aliases.get(&opposite_alias) {
                NativeStemsBeamHeadIncidentOpposite::Head(*head)
            } else {
                NativeStemsBeamHeadIncidentOpposite::OtherInter
            };
            let relation_class = row.value("runtimeClass")?.to_owned();
            Ok(NativeStemsBeamHeadIncidentRelation {
                incident_ordinal: row.usize("incidentOrdinal")?,
                direction: parse_head_direction(row.value("direction")?)?,
                direction_ordinal: row.usize("directionOrdinal")?,
                graph_relation_identity: parse_sig_edge(row.value("graphRelationIdentity")?)?,
                relation_object_identity: parse_head_relation_object(
                    row.value("relationObjectIdentity")?,
                )?,
                kind: head_query_kind(&relation_class),
                relation_class,
                opposite,
                opposite_alias,
                opposite_inter_id: row.i32("oppositeInterId")?,
                opposite_vertex_identity: row.usize("oppositeVertexOrdinal")?,
                class_read: parse_head_incident_read(row)?,
            })
        })
        .collect()
}

fn parse_head_incident_scan(
    rows: &[HeadStrictRow],
    summary: &HeadStrictRow,
    head_aliases: &BTreeMap<String, NativeStemsBeamHeadLinkHeadRef>,
    stem_alias: &str,
) -> Result<NativeStemsBeamHeadIncidentScan, String> {
    if summary.usize("incidentCount")? != rows.len()
        || !is_lower_sha256(summary.value("incidentSha256")?)
    {
        return Err("Boundary-17 incident scan count/provenance differs".to_owned());
    }
    Ok(NativeStemsBeamHeadIncidentScan {
        query_relation_count: rows.len(),
        query_provenance_sha256: summary.value("incidentSha256")?.to_owned(),
        relations: parse_head_incident_rows(rows, head_aliases, stem_alias)?,
    })
}

fn project_head_links_state(
    transaction: &ParsedHeadTransaction,
    hydrated: &HydratedBoundarySixteen,
) -> Result<NativeStemsBeamVLinkHeadLinksState, String> {
    let attempt = head_plan_attempt(hydrated)?;
    if attempt.relations.len() != transaction.entries.len()
        || transaction.baseline.usize("relationEntries")? != attempt.relations.len()
        || transaction.key.system != hydrated.transaction.key.system_id
        || transaction.key.plan != hydrated.transaction.key.plan.plan_ordinal
    {
        return Err("Boundary-17 typed plan/transaction cardinality differs".to_owned());
    }

    let mut live_heads = Vec::<NativeStemsBeamHeadLinkHeadState>::new();
    let mut s_linker_cells = Vec::<NativeStemsBeamHeadSLinkerCell>::new();
    let mut head_aliases = BTreeMap::new();
    for (map_ordinal, (entry, relation)) in transaction
        .entries
        .iter()
        .zip(&attempt.relations)
        .enumerate()
    {
        if relation.map_ordinal != map_ordinal
            || entry.head.usize("mapOrdinal")? != map_ordinal
            || entry.head.value("headRuntimeClass")? != "org.audiveris.omr.sig.inter.HeadInter"
            || entry.head.value("cRuntimeClass")?
                != "org.audiveris.omr.sheet.stem.HeadLinker$SLinker$CLinker"
            || entry.head.value("sRuntimeClass")?
                != "org.audiveris.omr.sheet.stem.HeadLinker$SLinker"
            || entry.head.value("evidenceTiming")? != "BeforeEntryMutationSnapshot"
        {
            return Err(format!(
                "Boundary-17 map {map_ordinal} runtime/order differs"
            ));
        }
        let head_ref = head_ref_from_entry(&entry.head, relation)?;
        let head_alias = entry.head.value("headAlias")?.to_owned();
        if let Some(previous) = head_aliases.insert(head_alias.clone(), head_ref)
            && previous != head_ref
        {
            return Err("Boundary-17 head alias resolves to multiple typed heads".to_owned());
        }
        if !live_heads.iter().any(|head| head.reference == head_ref) {
            let state = NativeStemsBeamHeadLinkHeadState {
                reference: head_ref,
                alias: head_alias,
                runtime_class: entry.head.value("headRuntimeClass")?.to_owned(),
                inter_id: entry.head.i32("headInterId")?,
                inter_index_ordinal: entry.head.usize("headIndexOrdinal")?,
                inter_index_object_matches: entry.head.usize("headIndexObjectMatches")?,
                inter_index_id_matches: entry.head.usize("headIndexIdMatches")?,
                sig_vertex_identity: entry.head.usize("headVertexOrdinal")?,
                sig_object_matches: entry.head.usize("headVertexObjectMatches")?,
                sig_system_id: entry.head.usize("headSigSystemId")?,
                removed: entry.head.bool("removed")?,
                vip: entry.head.bool("vip")?,
                manual: entry.head.bool("manual")?,
                abnormal: entry.head.bool("abnormalBefore")?,
                center: parse_head_center(entry.head.value("center")?)?,
                shape: entry.head.value("shape")?.to_owned(),
                is_small: entry.head.bool("small")?,
                is_stem_head: entry.head.bool("stemHead")?,
            };
            if state.center != relation.check.head_center {
                return Err(format!(
                    "Boundary-17 map {map_ordinal} head center differs from plan"
                ));
            }
            live_heads.push(state);
        }

        let s_ref = NativeStemsBeamHeadSLinkerRef {
            head: head_ref,
            horizontal: relation.corner.horizontal,
        };
        let expected_s_alias = format!(
            "s:{}:{}",
            relation.corner.x_ordinal,
            entry.head.value("horizontalSide")?
        );
        let observer_tokens = parse_list(entry.head.value("observerAliases")?)?;
        let expected_observers = [
            format!(
                "h:{}:{}:TOP",
                relation.corner.x_ordinal,
                entry.head.value("horizontalSide")?
            ),
            format!(
                "h:{}:{}:BOTTOM",
                relation.corner.x_ordinal,
                entry.head.value("horizontalSide")?
            ),
        ];
        if entry.head.value("sAlias")? != expected_s_alias
            || observer_tokens
                != expected_observers
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
            || entry.s_write.value("receiverAlias")? != expected_s_alias
            || entry.s_write.value("observerAliases")? != entry.head.value("observerAliases")?
        {
            return Err(format!(
                "Boundary-17 map {map_ordinal} shared S/C topology differs"
            ));
        }
        if !s_linker_cells.iter().any(|cell| cell.reference == s_ref) {
            s_linker_cells.push(NativeStemsBeamHeadSLinkerCell {
                reference: s_ref,
                linked: entry.head.bool("sLinkedBefore")?,
                closed: entry.head.bool("sClosedBefore")?,
                ordered_observer_corners: vec![
                    audiveris_omr::native_stems_beam_reachability::NativeStemsBeamHeadCornerRef {
                        vertical: NativeStemVerticalSide::Top,
                        ..relation.corner
                    },
                    audiveris_omr::native_stems_beam_reachability::NativeStemsBeamHeadCornerRef {
                        vertical: NativeStemVerticalSide::Bottom,
                        ..relation.corner
                    },
                ],
            });
        }

        let expected_object = NativeStemsBeamHeadRelationObjectIdentity::HeadDraft {
            plan_ordinal: hydrated.transaction.key.plan.plan_ordinal,
            map_ordinal,
        };
        let row_object = parse_head_relation_object(entry.head.value("planDraftIdentity")?)?;
        if row_object != expected_object
            || entry.head.value("draftRuntimeClass")?
                != "org.audiveris.omr.sig.relation.HeadStemRelation"
            || entry.head.bool("draftManual")?
            || entry.head.value("draftConsistencyBeforeState")? != "Unset"
            || entry.head.value("draftConsistencyBeforeValue")? != "-"
            || entry.head.usize("draftGraphMatches")? != 0
            || parse_head_side(entry.head.value("draftHeadSide")?)?
                != relation.check.derived_horizontal
            || parse_point(entry.head.value("draftExtension")?)?
                != relation
                    .check
                    .extension_point
                    .ok_or_else(|| format!("Boundary-17 map {map_ordinal} plan lacks extension"))?
            || parse_f64(entry.head.value("draftDx")?)?.to_bits() != relation.check.dx.to_bits()
            || parse_f64(entry.head.value("draftDy")?)?.to_bits() != relation.check.dy.to_bits()
            || parse_f64(entry.head.value("draftGrade")?)?.to_bits()
                != relation.check.grade.to_bits()
        {
            return Err(format!(
                "Boundary-17 map {map_ordinal} plan draft differs from typed plan"
            ));
        }
    }

    let stem_alias = transaction.baseline.value("stemAlias")?;
    let mut steps = Vec::with_capacity(transaction.entries.len());
    for (map_ordinal, (entry, relation)) in transaction
        .entries
        .iter()
        .zip(&attempt.relations)
        .enumerate()
    {
        let head_ref = head_ref_from_entry(&entry.head, relation)?;
        let s_ref = NativeStemsBeamHeadSLinkerRef {
            head: head_ref,
            horizontal: relation.corner.horizontal,
        };
        let directed_pair = NativeStemsBeamHeadDirectedPairScan {
            source_outgoing_scanned: entry.pair_scan.usize("sourceOutgoingCount")?,
            source_outgoing_provenance_sha256: entry
                .pair_scan
                .value("sourceOutgoingSha256")?
                .to_owned(),
            source_outgoing_relations: parse_head_outgoing_rows(entry)?,
            query_relation_count: entry.pair_scan.usize("pairCount")?,
            pair_provenance_sha256: entry.pair_scan.value("pairSha256")?.to_owned(),
            relations: parse_head_pair_rows(entry)?,
        };
        if !is_lower_sha256(&directed_pair.source_outgoing_provenance_sha256)
            || !is_lower_sha256(&directed_pair.pair_provenance_sha256)
        {
            return Err(format!(
                "Boundary-17 map {map_ordinal} pair provenance is not SHA-256"
            ));
        }
        let linked = entry.head.value("branch")? == "Linked";
        let (
            expected_consistency_after,
            consistency_debug_enabled,
            add_edge_returned,
            head_incident_after,
            stem_incident_after,
        ) = if linked {
            let consistency = entry
                .consistency
                .as_ref()
                .ok_or_else(|| "Boundary-17 linked entry lacks consistency".to_owned())?;
            let head_scan = entry
                .head_incident_scan
                .as_ref()
                .ok_or_else(|| "Boundary-17 linked entry lacks head incident scan".to_owned())?;
            let head_abnormal = match head_scan.value("state")? {
                "NotReadNonStemHead" => {
                    if !entry.head_incidents.is_empty()
                        || head_scan.usize("incidentCount")? != 0
                        || head_scan.bool("requestedAbnormal")?
                    {
                        return Err("Boundary-17 unread head scan has read evidence".to_owned());
                    }
                    NativeStemsBeamHeadAbnormalScan::NotReadNonStemHead
                }
                "LazyIncomingThenOutgoing" => {
                    NativeStemsBeamHeadAbnormalScan::Read(parse_head_incident_scan(
                        &entry.head_incidents,
                        head_scan,
                        &head_aliases,
                        stem_alias,
                    )?)
                }
                value => return Err(format!("invalid Boundary-17 head scan state {value}")),
            };
            let stem_scan = entry
                .stem_incident_scan
                .as_ref()
                .ok_or_else(|| "Boundary-17 linked entry lacks stem incident scan".to_owned())?;
            if stem_scan.value("state")? != "LazyIncomingThenOutgoing" {
                return Err("Boundary-17 stem callback scan is not lazy incident order".to_owned());
            }
            (
                Some(parse_f64(consistency.value("consistencyAfterValue")?)?),
                Some(consistency.bool("debugEnabled")?),
                Some(
                    entry
                        .edge
                        .as_ref()
                        .ok_or_else(|| "Boundary-17 linked entry lacks edge".to_owned())?
                        .bool("insertionReturned")?,
                ),
                Some(head_abnormal),
                Some(parse_head_incident_scan(
                    &entry.stem_incidents,
                    stem_scan,
                    &head_aliases,
                    stem_alias,
                )?),
            )
        } else {
            if entry.pair_scan.value("state")? != "FirstHeadStemMatch" {
                return Err("Boundary-17 duplicate branch lacks first-match query".to_owned());
            }
            (None, None, None, None, None)
        };
        if linked && entry.pair_scan.value("state")? != "ExhaustiveNoMatch" {
            return Err("Boundary-17 linked branch pair query is not exhaustive".to_owned());
        }
        steps.push(NativeStemsBeamHeadLinkStepCertificate {
            map_ordinal,
            corner: relation.corner,
            s_linker: s_ref,
            plan_draft: NativeStemsBeamHeadPlanDraft {
                relation_object_identity: NativeStemsBeamHeadRelationObjectIdentity::HeadDraft {
                    plan_ordinal: hydrated.transaction.key.plan.plan_ordinal,
                    map_ordinal,
                },
                relation_class: entry.head.value("draftRuntimeClass")?.to_owned(),
                relation: relation.clone(),
                manual: entry.head.bool("draftManual")?,
                head_side_before: Some(parse_head_side(entry.head.value("draftHeadSide")?)?),
                extension_point_before: Some(parse_point(entry.head.value("draftExtension")?)?),
                consistency_before: None,
                graph_matches_before: entry.head.usize("draftGraphMatches")?,
            },
            directed_pair,
            expected_consistency_after,
            consistency_debug_enabled,
            add_edge_returned,
            head_incident_after,
            stem_incident_after,
        });
    }

    let listener_topology = if transaction.baseline.bool("soleSigListener")? {
        NativeStemsBeamSigListenerTopology::SoleStandardSigListener
    } else {
        return Err("Boundary-17 compact fixture lacks sole SigListener".to_owned());
    };
    let sheet_edit = NativeStemsBeamSheetEditState {
        stub_modified: transaction.baseline.bool("stubModified")?,
        book_modified: transaction.baseline.bool("bookModified")?,
        book_dirty: transaction.baseline.bool("bookDirty")?,
    };
    Ok(NativeStemsBeamVLinkHeadLinksState {
        sibling_links_state_before: hydrated.state_before.clone(),
        sibling_links_state_after: hydrated.state_after.clone(),
        live_heads,
        s_linker_cells,
        stem_manual: transaction.baseline.bool("stemManual")?,
        stem_abnormal: transaction.baseline.bool("stemAbnormal")?,
        appended_relations: Vec::new(),
        sheet_edit,
        certificate: Some(NativeStemsBeamVLinkHeadLinksCertificate {
            system_id: transaction.key.system,
            headless: transaction.baseline.bool("headless")?,
            listener_topology,
            interline: transaction.baseline.i32("interline")?,
            neutral_stem_length: parse_f64(transaction.baseline.value("neutralStemLength")?)?,
            steps,
        }),
        committed: None,
    })
}

fn head_appended_from_rows(
    entry: &ParsedHeadEntry,
    relation: &audiveris_omr::native_stems_beam_link_plans::NativeStemsBeamHeadRelation,
    hydrated: &HydratedBoundarySixteen,
) -> Result<NativeStemsBeamHeadAppendedRelation, String> {
    let edge = entry
        .edge
        .as_ref()
        .ok_or_else(|| "Boundary-17 linked entry lacks edge row".to_owned())?;
    let consistency = entry
        .consistency
        .as_ref()
        .ok_or_else(|| "Boundary-17 linked entry lacks consistency row".to_owned())?;
    Ok(NativeStemsBeamHeadAppendedRelation {
        graph_relation_identity: parse_sig_edge(edge.value("graphRelationIdentity")?)?,
        relation_object_identity: parse_head_relation_object(
            edge.value("relationObjectIdentity")?,
        )?,
        source_head: head_ref_from_entry(&entry.head, relation)?,
        source_vertex_identity: edge.usize("sourceVertexOrdinal")?,
        target_stem_identity: hydrated.transaction.stem_after.stem_identity,
        target_vertex_identity: edge.usize("targetVertexOrdinal")?,
        relation: relation.clone(),
        consistency: parse_f64(consistency.value("consistencyAfterValue")?)?,
    })
}

fn expected_head_operations(
    transaction: &ParsedHeadTransaction,
    hydrated: &HydratedBoundarySixteen,
) -> Result<Vec<NativeStemsBeamVLinkHeadLinksOperation>, String> {
    let attempt = head_plan_attempt(hydrated)?;
    let mut operations = Vec::new();
    for (map_ordinal, (entry, relation)) in transaction
        .entries
        .iter()
        .zip(&attempt.relations)
        .enumerate()
    {
        let head_ref = head_ref_from_entry(&entry.head, relation)?;
        let s_ref = NativeStemsBeamHeadSLinkerRef {
            head: head_ref,
            horizontal: relation.corner.horizontal,
        };
        operations.push(
            NativeStemsBeamVLinkHeadLinksOperation::SLinkerLinkedAssigned {
                map_ordinal,
                target: s_ref,
                before: entry.s_write.bool("before")?,
                after: entry.s_write.bool("after")?,
                closed_before: entry.s_write.bool("closedBefore")?,
                closed_after: entry.s_write.bool("closedAfter")?,
            },
        );
        let matched = entry
            .pair_relations
            .iter()
            .any(|row| row.bool("matches").unwrap_or(false));
        let relations_read = entry
            .pair_relations
            .iter()
            .filter(|row| row.bool("classRead").unwrap_or(false))
            .count();
        operations.push(
            NativeStemsBeamVLinkHeadLinksOperation::DirectedPairLookupCompleted {
                map_ordinal,
                relations_read,
                matched,
            },
        );
        if entry.head.value("branch")? == "ExistingHeadStem" {
            continue;
        }
        let consistency = entry
            .consistency
            .as_ref()
            .ok_or_else(|| "Boundary-17 linked entry lacks consistency".to_owned())?;
        let consistency_value = parse_f64(consistency.value("consistencyAfterValue")?)?;
        operations.push(
            NativeStemsBeamVLinkHeadLinksOperation::HeadStemConsistencyAssigned {
                map_ordinal,
                value: consistency_value,
            },
        );
        let edge = entry
            .edge
            .as_ref()
            .ok_or_else(|| "Boundary-17 linked entry lacks edge".to_owned())?;
        let graph_relation_identity = parse_sig_edge(edge.value("graphRelationIdentity")?)?;
        operations.extend([
            NativeStemsBeamVLinkHeadLinksOperation::SigGlobalRelationInserted {
                map_ordinal,
                graph_relation_identity,
            },
            NativeStemsBeamVLinkHeadLinksOperation::HeadOutgoingRelationInserted {
                map_ordinal,
                graph_relation_identity,
            },
            NativeStemsBeamVLinkHeadLinksOperation::StemIncomingRelationInserted {
                map_ordinal,
                graph_relation_identity,
            },
            NativeStemsBeamVLinkHeadLinksOperation::SigEdgeEventDispatched {
                map_ordinal,
                graph_relation_identity,
            },
            NativeStemsBeamVLinkHeadLinksOperation::StandardSigListenerEdgeCallbackStarted {
                map_ordinal,
            },
            NativeStemsBeamVLinkHeadLinksOperation::HeadStemRelationCallbackStarted { map_ordinal },
        ]);
        let callback = entry
            .callback
            .as_ref()
            .ok_or_else(|| "Boundary-17 linked entry lacks callback".to_owned())?;
        operations.push(
            NativeStemsBeamVLinkHeadLinksOperation::AutomaticManualFlagsRead {
                map_ordinal,
                relation_manual_read: true,
                relation_manual: callback.bool("relationManual")?,
                head_manual_read: callback.bool("headManualRead")?,
                head_manual: callback.bool("headManual")?,
                stem_manual_read: callback.bool("stemManualRead")?,
                stem_manual: callback.bool("stemManual")?,
                chord_branch_read: callback.value("chordBranch")? != "NotReadAuto",
            },
        );
        let head_before = callback.bool("headAbnormalBefore")?;
        let head_after = callback.bool("headAbnormalAfter")?;
        if callback.value("headScanState")? == "NotReadNonStemHead" {
            operations.push(
                NativeStemsBeamVLinkHeadLinksOperation::HeadAbnormalScanNotReadNonStemHead {
                    map_ordinal,
                },
            );
        } else {
            let reads = entry
                .head_incidents
                .iter()
                .filter(|row| row.bool("classRead").unwrap_or(false))
                .count();
            operations.push(
                NativeStemsBeamVLinkHeadLinksOperation::HeadAbnormalScanCompleted {
                    map_ordinal,
                    relations_read: reads,
                },
            );
            operations.push(
                NativeStemsBeamVLinkHeadLinksOperation::HeadAbnormalAssigned {
                    map_ordinal,
                    before: head_before,
                    after: head_after,
                },
            );
            if head_before != head_after {
                operations.extend([
                    NativeStemsBeamVLinkHeadLinksOperation::SheetStubModifiedSetTrue {
                        map_ordinal,
                        subject: NativeStemsBeamHeadDirtySubject::Head,
                    },
                    NativeStemsBeamVLinkHeadLinksOperation::BookModifiedSetTrue {
                        map_ordinal,
                        subject: NativeStemsBeamHeadDirtySubject::Head,
                    },
                    NativeStemsBeamVLinkHeadLinksOperation::BookDirtySetTrue {
                        map_ordinal,
                        subject: NativeStemsBeamHeadDirtySubject::Head,
                    },
                ]);
            }
        }
        let stem_reads = entry
            .stem_incidents
            .iter()
            .filter(|row| row.bool("classRead").unwrap_or(false))
            .count();
        let stem_before = callback.bool("stemAbnormalBefore")?;
        let stem_after = callback.bool("stemAbnormalAfter")?;
        operations.push(
            NativeStemsBeamVLinkHeadLinksOperation::StemAbnormalScanCompleted {
                map_ordinal,
                relations_read: stem_reads,
            },
        );
        operations.push(
            NativeStemsBeamVLinkHeadLinksOperation::StemAbnormalAssigned {
                map_ordinal,
                before: stem_before,
                after: stem_after,
            },
        );
        if stem_before != stem_after {
            operations.extend([
                NativeStemsBeamVLinkHeadLinksOperation::SheetStubModifiedSetTrue {
                    map_ordinal,
                    subject: NativeStemsBeamHeadDirtySubject::Stem,
                },
                NativeStemsBeamVLinkHeadLinksOperation::BookModifiedSetTrue {
                    map_ordinal,
                    subject: NativeStemsBeamHeadDirtySubject::Stem,
                },
                NativeStemsBeamVLinkHeadLinksOperation::BookDirtySetTrue {
                    map_ordinal,
                    subject: NativeStemsBeamHeadDirtySubject::Stem,
                },
            ]);
        }
        operations.extend([
            NativeStemsBeamVLinkHeadLinksOperation::HeadStemRelationCallbackCompleted {
                map_ordinal,
            },
            NativeStemsBeamVLinkHeadLinksOperation::StandardSigListenerEdgeCallbackCompleted {
                map_ordinal,
            },
        ]);
    }
    operations.extend([
        NativeStemsBeamVLinkHeadLinksOperation::RemainderCompared {
            last_index: transaction.remainder.i32("lastIndex")?,
            max_index: transaction.remainder.i32("maxIndex")?,
            less_than: transaction.remainder.bool("remainderPresent")?,
            split_mutation_count: transaction.remainder.usize("splitCalls")?,
        },
        NativeStemsBeamVLinkHeadLinksOperation::VLinkerReturnedTrue,
    ]);
    Ok(operations)
}

fn assert_head_public_matches_rows(
    transaction: &ParsedHeadTransaction,
    hydrated: &HydratedBoundarySixteen,
    state_before: &NativeStemsBeamVLinkHeadLinksState,
    state_after: &NativeStemsBeamVLinkHeadLinksState,
    public: &NativeStemsBeamVLinkHeadLinksTransaction,
) -> Result<(), String> {
    let attempt = head_plan_attempt(hydrated)?;
    let consumed = state_before
        .certificate
        .as_ref()
        .ok_or_else(|| "Boundary-17 projected state lacks certificate".to_owned())?;
    let linked_entries = transaction
        .entries
        .iter()
        .filter(|entry| entry.head.value("branch") == Ok("Linked"))
        .count();
    let duplicate_entries = transaction.entries.len() - linked_entries;
    let head_changes = transaction
        .entries
        .iter()
        .filter_map(|entry| entry.callback.as_ref())
        .filter(|callback| {
            callback.bool("headAbnormalBefore").ok() != callback.bool("headAbnormalAfter").ok()
        })
        .count();
    let stem_changes = transaction
        .entries
        .iter()
        .filter_map(|entry| entry.callback.as_ref())
        .filter(|callback| {
            callback.bool("stemAbnormalBefore").ok() != callback.bool("stemAbnormalAfter").ok()
        })
        .count();
    let s_value_changes = transaction
        .entries
        .iter()
        .filter(|entry| entry.s_write.usize("valueChangeCount") == Ok(1))
        .count();
    let head_assignments = transaction
        .entries
        .iter()
        .filter(|entry| entry.callback.is_some() && entry.head.bool("stemHead").unwrap_or(false))
        .count();
    let expected_appended = transaction
        .entries
        .iter()
        .zip(&attempt.relations)
        .filter(|(entry, _)| entry.head.value("branch") == Ok("Linked"))
        .map(|(entry, relation)| head_appended_from_rows(entry, relation, hydrated))
        .collect::<Result<Vec<_>, _>>()?;
    let expected_graph_ids = expected_appended
        .iter()
        .map(|relation| relation.graph_relation_identity)
        .collect::<Vec<_>>();
    let expected_s = attempt
        .relations
        .iter()
        .map(|relation| NativeStemsBeamHeadSLinkerRef {
            head: NativeStemsBeamHeadLinkHeadRef {
                reference: relation.corner.head,
                sig_ordinal: relation.corner.sig_ordinal,
                x_ordinal: relation.corner.x_ordinal,
            },
            horizontal: relation.corner.horizontal,
        })
        .collect::<Vec<_>>();
    let expected_operations = expected_head_operations(transaction, hydrated)?;
    if public.key != hydrated.transaction.key
        || public.consumed_certificate != *consumed
        || public.continuation_support_grade.to_bits()
            != hydrated.transaction.continuation_support_grade.to_bits()
        || public.state_after.as_ref() != state_after
        || public.appended_graph_relation_identities != expected_graph_ids
        || public.assigned_s_linkers != expected_s
        || public.s_linker_write_count != transaction.entries.len()
        || public.s_linker_value_change_count != s_value_changes
        || public.consistency_mutation_count != linked_entries
        || public.sig_relation_mutation_count != linked_entries
        || public.head_abnormal_assignment_count != head_assignments
        || public.head_abnormal_mutation_count != head_changes
        || public.stem_abnormal_assignment_count != linked_entries
        || public.stem_abnormal_mutation_count != stem_changes
        || public.dirty_cascade_count != head_changes + stem_changes
        || public.sheet_edit_mutation_count != transaction.result.usize("sheetEditMutationCount")?
        || public.event_count != transaction.result.usize("eventCount")?
        || public.sibling_link_mutation_count != 0
        || public.head_link_mutation_count != linked_entries
        || public.last_index != transaction.remainder.i32("lastIndex")?
        || public.max_index != transaction.remainder.i32("maxIndex")?
        || public.remainder_less_than != transaction.remainder.bool("remainderPresent")?
        || public.split_mutation_count != 0
        || !public.returned_true
        || public.operations != expected_operations
        || state_after.appended_relations != expected_appended
        || state_after.certificate.is_some()
        || state_after.committed != Some(public.key)
    {
        return Err(
            "public Boundary-17 transaction/state header differs from Java rows".to_owned(),
        );
    }
    match public.outcome {
        NativeStemsBeamVLinkHeadLinksOutcome::ReturnedTrueBeforeOuterBLinkerAssignment {
            stem_identity,
            continuation_support_grade,
        } if stem_identity == hydrated.transaction.stem_after.stem_identity
            && continuation_support_grade.to_bits()
                == public.continuation_support_grade.to_bits() => {}
        _ => return Err("public Boundary-17 terminal outcome differs".to_owned()),
    }
    if public.steps.len() != transaction.entries.len() {
        return Err("public Boundary-17 trace cardinality differs".to_owned());
    }
    let mut serial_stem_abnormal = transaction.baseline.bool("stemAbnormal")?;
    for (map_ordinal, ((trace, entry), relation)) in public
        .steps
        .iter()
        .zip(&transaction.entries)
        .zip(&attempt.relations)
        .enumerate()
    {
        let expected_head = head_ref_from_entry(&entry.head, relation)?;
        let expected_appended = if entry.head.value("branch")? == "Linked" {
            Some(head_appended_from_rows(entry, relation, hydrated)?)
        } else {
            None
        };
        let expected_branch = if expected_appended.is_some() {
            NativeStemsBeamHeadLinkBranch::Linked
        } else {
            NativeStemsBeamHeadLinkBranch::ExistingHeadStem
        };
        let consistency_after = entry
            .consistency
            .as_ref()
            .map(|row| parse_f64(row.value("consistencyAfterValue").unwrap()))
            .transpose()?;
        let callback = entry.callback.as_ref();
        let expected_graph_relation = if let Some(edge) = &entry.edge {
            edge.value("graphRelationIdentity")?
        } else {
            entry.pair_scan.value("selectedGraphRelationIdentity")?
        };
        let expected_consistency_state = if consistency_after.is_some() {
            "Set"
        } else {
            "Unset"
        };
        let expected_consistency_value = entry
            .consistency
            .as_ref()
            .map(|row| row.value("consistencyAfterValue"))
            .transpose()?
            .unwrap_or("-");
        let expected_insertion_returned = entry
            .edge
            .as_ref()
            .map(|row| row.value("insertionReturned"))
            .transpose()?
            .unwrap_or("NotRead");
        let expected_head_abnormal_after = callback
            .map(|row| row.value("headAbnormalAfter"))
            .transpose()?
            .unwrap_or(entry.head.value("abnormalBefore")?);
        if let Some(callback) = callback {
            if callback.bool("stemAbnormalBefore")? != serial_stem_abnormal {
                return Err(format!(
                    "Boundary-17 entry {map_ordinal} stem abnormal serial prefix differs"
                ));
            }
            serial_stem_abnormal = callback.bool("stemAbnormalAfter")?;
        }
        let expected_stem_abnormal_after = serial_stem_abnormal.to_string();
        if entry.result.value("branch")? != entry.head.value("branch")?
            || entry.result.value("sLinkedAfter")? != entry.s_write.value("after")?
            || entry.result.value("sClosedAfter")? != entry.s_write.value("closedAfter")?
            || entry.result.value("draftConsistencyAfterState")? != expected_consistency_state
            || entry.result.value("draftConsistencyAfterValue")? != expected_consistency_value
            || entry.result.value("graphRelationIdentity")? != expected_graph_relation
            || entry.result.value("relationObjectIdentity")?
                != entry.head.value("planDraftIdentity")?
            || entry.result.value("insertionReturned")? != expected_insertion_returned
            || entry.result.bool("callbackCompleted")? != callback.is_some()
            || entry.result.value("headAbnormalAfter")? != expected_head_abnormal_after
            || entry.result.value("stemAbnormalAfter")? != expected_stem_abnormal_after
            || !is_lower_sha256(entry.result.value("relationStateBeforeSha256")?)
            || !is_lower_sha256(entry.result.value("relationStateAfterSha256")?)
            || (expected_appended.is_some()
                == (entry.result.value("relationStateBeforeSha256")?
                    == entry.result.value("relationStateAfterSha256")?))
        {
            return Err(format!(
                "Boundary-17 entry result {map_ordinal} differs from its serial trace"
            ));
        }
        if trace.map_ordinal != map_ordinal
            || trace.s_write_event_ordinal != entry.s_write.usize("eventOrdinal")?
            || trace.consistency_event_ordinal
                != entry
                    .consistency
                    .as_ref()
                    .map(|row| row.usize("eventOrdinal"))
                    .transpose()?
            || trace.edge_event_ordinal
                != entry
                    .edge
                    .as_ref()
                    .map(|row| row.usize("eventOrdinal"))
                    .transpose()?
            || trace.callback_event_ordinal
                != callback.map(|row| row.usize("eventOrdinal")).transpose()?
            || trace.corner != relation.corner
            || trace.head != expected_head
            || trace.s_linker.head != expected_head
            || trace.s_linked_before != entry.s_write.bool("before")?
            || trace.s_linked_after != entry.s_write.bool("after")?
            || trace.s_closed_before != entry.s_write.bool("closedBefore")?
            || trace.s_closed_after != entry.s_write.bool("closedAfter")?
            || trace.branch != expected_branch
            || trace.directed_pair_relations_read
                != entry
                    .pair_relations
                    .iter()
                    .filter(|row| row.bool("classRead").unwrap_or(false))
                    .count()
            || trace.consistency_before.is_some()
            || trace.consistency_after.map(f64::to_bits) != consistency_after.map(f64::to_bits)
            || trace.default_head_side_branch_read != callback.map(|_| false)
            || trace.default_extension_branch_read != callback.map(|_| false)
            || trace.manual_chord_branch_read != callback.map(|_| false)
            || trace.appended_relation != expected_appended
            || trace.add_edge_returned != callback.map(|_| true)
            || trace.callback_completed != callback.is_some()
            || trace.head_abnormal_before
                != callback
                    .map(|row| row.bool("headAbnormalBefore"))
                    .transpose()?
            || trace.head_abnormal_after
                != callback
                    .map(|row| row.bool("headAbnormalAfter"))
                    .transpose()?
            || trace.stem_abnormal_before
                != callback
                    .map(|row| row.bool("stemAbnormalBefore"))
                    .transpose()?
            || trace.stem_abnormal_after
                != callback
                    .map(|row| row.bool("stemAbnormalAfter"))
                    .transpose()?
        {
            return Err(format!(
                "public Boundary-17 trace {map_ordinal} differs from Java rows"
            ));
        }
    }
    if transaction.result.usize("headEntries")? != transaction.entries.len()
        || transaction.result.usize("duplicateEntries")? != duplicate_entries
        || transaction.result.usize("relationsInserted")? != linked_entries
        || transaction.result.usize("sWriteCount")? != transaction.entries.len()
        || transaction.result.usize("sValueChangeCount")? != s_value_changes
        || transaction.result.usize("consistencyWriteCount")? != linked_entries
        || transaction.result.usize("headAbnormalChangeCount")? != head_changes
        || transaction.result.usize("stemAbnormalChangeCount")? != stem_changes
        || transaction.result.usize("dirtyCascadeCount")? != head_changes + stem_changes
        || !transaction.result.bool("returnedTrue")?
        || transaction.result.value("terminal")? != "ReturnedTrueBeforeOuterBLinkerAssignment"
        || transaction.summary.usize("headEntries")? != transaction.entries.len()
        || transaction.summary.usize("duplicateEntries")? != duplicate_entries
        || transaction.summary.usize("relationsInserted")? != linked_entries
        || transaction.summary.usize("sWrites")? != transaction.entries.len()
        || !transaction.summary.bool("returnedTrue")?
        || transaction.summary.value("terminal")? != "ReturnedTrueBeforeOuterBLinkerAssignment"
        || transaction.remainder.usize("builderItemCount")?
            != transaction.baseline.usize("builderItems")?
        || !transaction.remainder.bool("comparisonEvaluated")?
        || transaction.remainder.value("splitBody")? != "CommentOnly"
        || transaction.remainder.usize("splitCalls")? != 0
        || !transaction.remainder.bool("returnedTrue")?
        || !transaction.guard.bool("stopBeforeOuterBLinkerAssignment")?
        || transaction.guard.bool("outerBLinkerAssignmentRead")?
        || transaction.guard.usize("splitCalls")? != 0
    {
        return Err("Boundary-17 result/remainder/guard algebra differs".to_owned());
    }
    Ok(())
}

fn validate_isolated_head_cases(
    supplemental: &[ParsedSyntheticHeadCase],
    system_one: &ParsedHeadTransaction,
) -> Result<(), String> {
    const CASES: &[&str] = &[
        "Duplicate",
        "SmallHeadConsistency",
        "NullSideExtension",
        "ManualNoChord",
        "ManualChordRewire",
        "PreInsertThrow",
        "CallbackThrow",
    ];
    if supplemental.len() != CASES.len() {
        return Err(format!(
            "Boundary-17 supplemental case count is {}, expected {}",
            supplemental.len(),
            CASES.len()
        ));
    }
    for (ordinal, (block, expected_name)) in supplemental.iter().zip(CASES).enumerate() {
        let case = &block.case;
        let duplicate = *expected_name == "Duplicate";
        let small = *expected_name == "SmallHeadConsistency";
        let null_fallback = *expected_name == "NullSideExtension";
        let manual = matches!(*expected_name, "ManualNoChord" | "ManualChordRewire");
        let chord = *expected_name == "ManualChordRewire";
        let pre_insert_throw = *expected_name == "PreInsertThrow";
        let callback_throw = *expected_name == "CallbackThrow";
        let successful_insert = !duplicate && !pre_insert_throw && !callback_throw;
        let expected_scope = if duplicate || small {
            "synthetic"
        } else {
            "envelope"
        };
        let expected_terminal = if pre_insert_throw {
            "Threw:AddEdgeBeforeInsertion"
        } else if callback_throw {
            "Threw:HeadCheckAbnormalDuringRelationCallback"
        } else {
            "IsolatedEntryCompleted"
        };
        let expected_callback = if duplicate {
            "NotReadDuplicate"
        } else if pre_insert_throw {
            "NotReadPreInsertionThrow"
        } else if callback_throw {
            "StartedThrewInHeadAbnormal"
        } else {
            "Completed"
        };
        let expected_events: Vec<&str> = match *expected_name {
            "Duplicate" => vec!["SLinkerLinkedAssigned", "DirectedPairLookupCompleted"],
            "ManualChordRewire" => vec![
                "SLinkerLinkedAssigned",
                "DirectedPairLookupCompleted",
                "ConsistencyAssigned",
                "SigEdgeInserted",
                "HeadStemCallbackStarted",
                "OldChordStemRemoved",
                "NewChordStemInserted",
                "HeadStemCallbackCompleted",
            ],
            "PreInsertThrow" => vec![
                "SLinkerLinkedAssigned",
                "DirectedPairLookupCompleted",
                "ConsistencyAssigned",
                "Throw",
            ],
            "CallbackThrow" => vec![
                "SLinkerLinkedAssigned",
                "DirectedPairLookupCompleted",
                "ConsistencyAssigned",
                "SigEdgeInserted",
                "HeadStemCallbackStarted",
                "Throw",
            ],
            _ => vec![
                "SLinkerLinkedAssigned",
                "DirectedPairLookupCompleted",
                "ConsistencyAssigned",
                "SigEdgeInserted",
                "HeadStemCallbackStarted",
                "HeadStemCallbackCompleted",
            ],
        };
        if block.key.case_name != *expected_name
            || block.key.scope != expected_scope
            || block.key.system != 1
            || block.key.plan != system_one.key.plan
            || case.value("join")? != "IsolatedBoundary16Replay"
            || case.value("sourceRealB16EvidenceSha256")?
                != system_one
                    .predecessor
                    .value("b16TransactionEvidenceSha256")?
            || case.value("construction")? != "RealBookSheetSystemSIGHeadLinker"
            || case.value("shape")?
                != if small {
                    "NOTEHEAD_BLACK_SMALL"
                } else {
                    "NOTEHEAD_BLACK"
                }
            || case.bool("small")? != small
            || case.bool("stemAttachedBefore")? == pre_insert_throw
            || !case.bool("soleSigListener")?
            || case.bool("sLinkedBefore")?
            || !case.bool("sLinkedAfter")?
            || case.bool("sClosedBefore")?
            || case.bool("sClosedAfter")?
            || case.value("pairState")?
                != if duplicate {
                    "FirstHeadStemMatch"
                } else {
                    "ExhaustiveNoMatch"
                }
            || (case.value("selectedGraphRelationIdentity")? == "-") == duplicate
            || case.value("selectedRuntimeClass")?
                != if duplicate {
                    "org.audiveris.omr.sig.relation.HeadStemRelation"
                } else {
                    "-"
                }
            || case.value("draftConsistencyBefore")? != "Unset"
            || (case.value("draftConsistencyAfter")? == "Unset") != duplicate
            || (case.value("scaledStemLength")? == "NotRead") != duplicate
            || (case.value("expectedConsistency")? == "NotRead") != duplicate
            || case.bool("draftAttachedBefore")?
            || case.bool("draftAttachedAfter")? != (!duplicate && !pre_insert_throw)
            || case.bool("relationManual")? != manual
            || case.bool("headManual")?
            || case.bool("stemManual")?
            || case.bool("sideFallbackTaken")? != null_fallback
            || case.bool("extensionFallbackTaken")? != null_fallback
            || case.bool("manualBranchRead")? != (!duplicate && !pre_insert_throw)
            || case.bool("chordBranchRead")? != manual
            || case.bool("headAbnormalRead")? != (!duplicate && !pre_insert_throw)
            || case.bool("stemAbnormalRead")? != successful_insert
            || case.usize("graphEdgesAfter")? as i64 - case.usize("graphEdgesBefore")? as i64
                != if duplicate || pre_insert_throw { 0 } else { 1 }
            || case.usize("headStemAfter")? as i64 - case.usize("headStemBefore")? as i64
                != if duplicate || pre_insert_throw { 0 } else { 1 }
            || case.usize("chordStemAfter")? != case.usize("chordStemBefore")?
            || case.bool("oldChordStemRetained")?
            || case.usize("newChordStemCount")? != usize::from(chord)
            || case.bool("chordTargetsNewStem")? != chord
            || case.value("addEdgeReturned")? != if successful_insert { "true" } else { "NotRead" }
            || case.value("callbackState")? != expected_callback
            || case.bool("headAbnormalBefore")? == duplicate
            || case.bool("headAbnormalAfter")? != (!duplicate && !successful_insert)
            || case.bool("stemAbnormalBefore")? == duplicate
            || case.bool("stemAbnormalAfter")? != (!duplicate && !successful_insert)
            || case.value("dirtyBefore")? != "false:false:false"
            || case.value("dirtyAfter")?
                != if successful_insert {
                    "true:true:true"
                } else {
                    "false:false:false"
                }
            || case.value("throwClass")?
                != if pre_insert_throw {
                    "java.lang.IllegalArgumentException"
                } else if callback_throw {
                    "org.audiveris.omr.rustport.StemsBeamVLinkHeadLinksProbe$SyntheticHeadCallbackException"
                } else {
                    "-"
                }
            || case.value("throwStage")?
                != if pre_insert_throw {
                    "AddEdgeBeforeInsertion"
                } else if callback_throw {
                    "HeadCheckAbnormalDuringRelationCallback"
                } else {
                    "-"
                }
            || case.usize("eventCount")? != expected_events.len()
            || case.value("terminal")? != expected_terminal
            || block.events.len() != expected_events.len()
        {
            return Err(format!(
                "Boundary-17 supplemental {ordinal} ({expected_name}) branch/state differs"
            ));
        }
        if !duplicate {
            let scaled = parse_f64(case.value("scaledStemLength")?)?;
            let neutral = parse_f64(case.value("neutralStemLength")?)?;
            let expected = if small {
                1.0 / (scaled / neutral)
            } else {
                scaled / neutral
            };
            if parse_f64(case.value("draftConsistencyAfter")?)?.to_bits() != expected.to_bits()
                || parse_f64(case.value("expectedConsistency")?)?.to_bits() != expected.to_bits()
                || neutral.to_bits()
                    != parse_f64(system_one.baseline.value("neutralStemLength")?)?.to_bits()
            {
                return Err(format!(
                    "Boundary-17 supplemental {expected_name} consistency algebra differs"
                ));
            }
        }
        if null_fallback {
            if case.value("headSideBefore")? != "-"
                || case.value("headSideAfter")? != "RIGHT"
                || case.value("extensionBefore")? != "-"
                || case.value("extensionAfter")? == "-"
            {
                return Err("Boundary-17 null fallback payload differs".to_owned());
            }
        } else if case.value("headSideBefore")? != "RIGHT"
            || case.value("headSideAfter")? != "RIGHT"
            || case.value("extensionBefore")? == "-"
            || case.value("extensionAfter")? != case.value("extensionBefore")?
        {
            return Err(format!(
                "Boundary-17 supplemental {expected_name} prepopulated payload differs"
            ));
        }
        for (event, expected_kind) in block.events.iter().zip(&expected_events) {
            if event.value("kind")? != *expected_kind {
                return Err(format!(
                    "Boundary-17 supplemental {expected_name} event order differs"
                ));
            }
        }
        let guard = &block.guard;
        if guard.i32("graphDelta")? != if duplicate || pre_insert_throw { 0 } else { 1 }
            || guard.value("allowedMutations")?
                != "SelectedSSharedCellDraftConsistencyHeadStemCallbackManualChordRewireAbnormalDirty"
            || !guard.bool("headPayloadUnchanged")?
            || !guard.bool("stemPayloadUnchanged")?
            || !guard.bool("closedFlagsUnchanged")?
            || !guard.bool("unrelatedGraphPreserved")?
            || !guard.bool("isolatedOnly")?
            || guard.bool("productionEquivalent")?
            || !guard.bool("enclosingRealSheetUnchanged")?
            || guard.bool("outerBLinkerAssignmentRead")?
            || guard.value("terminal")? != expected_terminal
        {
            return Err(format!(
                "Boundary-17 supplemental {expected_name} guard differs"
            ));
        }
    }
    Ok(())
}

fn validate_head_links_trailers(
    text: &str,
    body_rows: &[HeadStrictRow],
    page: &HeadStrictRow,
    transactions: &[ParsedHeadTransaction],
    supplemental: &[ParsedSyntheticHeadCase],
    page_summary: &HeadStrictRow,
    corpus_summary: &HeadStrictRow,
) -> Result<(), String> {
    const BODY_KINDS: &[HeadRowKind] = &[
        HeadRowKind::Page,
        HeadRowKind::Predecessor,
        HeadRowKind::Baseline,
        HeadRowKind::HeadEntry,
        HeadRowKind::SLinkerWrite,
        HeadRowKind::SourceOutgoing,
        HeadRowKind::PairRelation,
        HeadRowKind::PairScan,
        HeadRowKind::Consistency,
        HeadRowKind::Edge,
        HeadRowKind::HeadIncident,
        HeadRowKind::HeadIncidentScan,
        HeadRowKind::StemIncident,
        HeadRowKind::StemIncidentScan,
        HeadRowKind::Callback,
        HeadRowKind::EntryResult,
        HeadRowKind::Remainder,
        HeadRowKind::Result,
        HeadRowKind::DeltaGuard,
        HeadRowKind::Summary,
        HeadRowKind::SyntheticCase,
        HeadRowKind::SyntheticEvent,
        HeadRowKind::SyntheticGuard,
    ];
    if page_summary.page != page.page {
        return Err("Boundary-17 page summary refers to another page".to_owned());
    }
    let head_entries = transactions
        .iter()
        .map(|transaction| transaction.entries.len())
        .sum::<usize>();
    let duplicate_entries = transactions
        .iter()
        .flat_map(|transaction| &transaction.entries)
        .filter(|entry| entry.head.value("branch") == Ok("ExistingHeadStem"))
        .count();
    let relations_inserted = head_entries - duplicate_entries;
    let s_value_changes = transactions
        .iter()
        .flat_map(|transaction| &transaction.entries)
        .map(|entry| entry.s_write.usize("valueChangeCount"))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum::<usize>();
    let head_abnormal_changes = transactions
        .iter()
        .flat_map(|transaction| &transaction.entries)
        .filter_map(|entry| entry.callback.as_ref())
        .map(|callback| {
            Ok(usize::from(
                callback.bool("headAbnormalBefore")? != callback.bool("headAbnormalAfter")?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?
        .into_iter()
        .sum::<usize>();
    let stem_abnormal_changes = transactions
        .iter()
        .flat_map(|transaction| &transaction.entries)
        .filter_map(|entry| entry.callback.as_ref())
        .map(|callback| {
            Ok(usize::from(
                callback.bool("stemAbnormalBefore")? != callback.bool("stemAbnormalAfter")?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?
        .into_iter()
        .sum::<usize>();
    let sheet_edit_mutations = transactions
        .iter()
        .map(|transaction| transaction.result.usize("sheetEditMutationCount"))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum::<usize>();
    let real_events = transactions
        .iter()
        .map(|transaction| transaction.result.usize("eventCount"))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum::<usize>();
    let supported = supplemental
        .iter()
        .filter(|case| case.key.scope == "synthetic")
        .count();
    let envelopes = supplemental.len() - supported;
    let isolated_events = supplemental
        .iter()
        .map(|case| case.events.len())
        .sum::<usize>();
    let isolated_graph_delta = supplemental
        .iter()
        .map(|case| case.guard.i32("graphDelta"))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum::<i32>();
    let isolated_throws = supplemental
        .iter()
        .filter(|case| case.case.value("throwClass") != Ok("-"))
        .count();
    let isolated_manual = supplemental
        .iter()
        .map(|case| case.case.bool("relationManual"))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|manual| *manual)
        .count();
    let chord_rewires = supplemental
        .iter()
        .map(|case| case.case.usize("newChordStemCount"))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum::<usize>();
    let total_transactions = transactions.len() + supplemental.len();
    let census = [
        ("systems", transactions.len().to_string()),
        ("realTransactions", transactions.len().to_string()),
        ("supportedSyntheticCases", supported.to_string()),
        ("envelopeCases", envelopes.to_string()),
        ("isolatedCases", supplemental.len().to_string()),
        ("totalTransactions", total_transactions.to_string()),
        ("headEntries", head_entries.to_string()),
        ("duplicateEntries", duplicate_entries.to_string()),
        ("relationsInserted", relations_inserted.to_string()),
        ("sWrites", head_entries.to_string()),
        ("sValueChanges", s_value_changes.to_string()),
        ("consistencyWrites", relations_inserted.to_string()),
        ("headAbnormalChanges", head_abnormal_changes.to_string()),
        ("stemAbnormalChanges", stem_abnormal_changes.to_string()),
        (
            "dirtyCascades",
            (head_abnormal_changes + stem_abnormal_changes).to_string(),
        ),
        ("sheetEditMutations", sheet_edit_mutations.to_string()),
        ("realEvents", real_events.to_string()),
        ("isolatedEvents", isolated_events.to_string()),
        ("isolatedGraphDelta", isolated_graph_delta.to_string()),
        ("isolatedThrows", isolated_throws.to_string()),
        ("isolatedManualCases", isolated_manual.to_string()),
        ("chordRewires", chord_rewires.to_string()),
    ];
    for (field, expected) in &census {
        if page_summary.value(field)? != expected {
            return Err(format!("Boundary-17 page summary {field} differs"));
        }
        if *field != "systems" && corpus_summary.value(field)? != expected {
            return Err(format!("Boundary-17 corpus summary {field} differs"));
        }
    }
    if !page_summary.bool("stopBeforeOuterBLinkerAssignment")?
        || corpus_summary.value("schema")? != "stems-beam-vlink-head-links-v1"
        || corpus_summary.usize("pages")? != 1
        || corpus_summary.value("pageRefs")? != page.page
        || corpus_summary.value("predecessorReplay")?
            != "FullBoundary16TypedReplayAndExactJavaRowJoin"
        || corpus_summary.value("querySerialization")? != "UTF8ColonTokensLF-LazyNotReadLiteral"
        || corpus_summary.usize("freshRunsPerPage")? != 2
        || !corpus_summary.bool("freshRunsByteIdentical")?
        || !corpus_summary.bool("freshJvmPerSystem")?
        || corpus_summary.usize("compilerJavaProcesses")? != 1
        || corpus_summary.usize("runtimeJavaProcessesPerPass")? != transactions.len()
        || corpus_summary.usize("runtimeJavaProcesses")? != 2 * transactions.len()
        || corpus_summary.usize("totalJavaProcesses")? != 2 * transactions.len() + 1
        || corpus_summary.usize("maximumConcurrentJavaProcesses")? != 1
        || corpus_summary.value("concurrencyScope")? != "Boundary17RunnerLockedInvocation"
        || !corpus_summary.bool("compilerJavaProcessReaped")?
        || !corpus_summary.bool("runtimeJavaProcessesReaped")?
        || !corpus_summary.bool("foregroundJavaProcessesOnly")?
        || corpus_summary.usize("backgroundJavaProcessesStarted")? != 0
        || !corpus_summary.bool("system1IsolatedBlock")?
        || corpus_summary.value("supportedSyntheticEvidenceScope")?
            != "IsolatedJavaGateOnlyNotProductionEquivalent"
        || corpus_summary.value("envelopeEvidenceScope")?
            != "IsolatedJavaGateOnlyNotProductionEquivalent"
        || !corpus_summary.bool("stopBeforeOuterBLinkerAssignment")?
    {
        return Err("Boundary-17 terminal corpus contract differs".to_owned());
    }
    let (page_key, page_file) = corpus_page_for_token(&page.page)?;
    if corpus_summary.value("mode")? != page_key {
        return Err("Boundary-17 corpus mode differs from its page token".to_owned());
    }
    let row_counts = BODY_KINDS
        .iter()
        .map(|kind| {
            format!(
                "{}:{}",
                kind.label(),
                body_rows.iter().filter(|row| row.kind == *kind).count()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    if corpus_summary.value("rowCounts")? != row_counts {
        return Err("Boundary-17 rowCounts differs from the authenticated body".to_owned());
    }
    let marker = format!("\n{} {} ", HeadRowKind::PageSummary.label(), page.page);
    let marker_offset = text
        .rfind(&marker)
        .ok_or_else(|| "Boundary-17 page summary marker is absent".to_owned())?;
    let body = &text.as_bytes()[..marker_offset + 1];
    if corpus_summary.value("emittedBodySha256")? != sha256_hex(body)
        || corpus_summary.value("rawPassSha256")? != sha256_hex(body)
        || corpus_summary.usize("emittedBodyLines")? != body.iter().filter(|b| **b == b'\n').count()
        || corpus_summary.usize("emittedBodyBytes")? != body.len()
    {
        return Err("Boundary-17 emitted-body provenance differs".to_owned());
    }
    let expected_fixture_fields = [
        "schedulerFixtureSha256",
        "expandFixtureSha256",
        "createStemFixtureSha256",
        "reuseCheckFixtureSha256",
        "baseApplyFixtureSha256",
        "bLinkerFlagFixtureSha256",
        "siblingLinksFixtureSha256",
    ];
    for field in expected_fixture_fields {
        if corpus_summary.value(field)? != page.value(field)? {
            return Err(format!("Boundary-17 page/corpus {field} join differs"));
        }
    }
    let current_pins = [
        ("probeSourceSha256", HEAD_LINKS_PROBE_SOURCE_PATH),
        ("runnerSourceSha256", HEAD_LINKS_RUNNER_SOURCE_PATH),
        (
            "baseApplyManifestSha256",
            "rust/oracle/stems-beam-vlink-base-apply-manifest.txt",
        ),
        ("bLinkerFlagManifestSha256", BOUNDARY_FIFTEEN_MANIFEST_PATH),
        ("siblingLinksManifestSha256", MANIFEST_PATH),
    ];
    for (field, path) in current_pins {
        if corpus_summary.value(field)? != read_sha256(path)? {
            return Err(format!("Boundary-17 active {field} differs"));
        }
    }
    if corpus_summary.value("pageInputSha256")?
        != read_sha256(&format!("data/examples/{page_file}"))?
        || corpus_summary.value("jgraphtCoreVersion")? != "1.5.2"
        || corpus_summary.value("jgraphtCoreJarSha256")?
            != "dfa596e9f0d0838f1b5e81dd0cd60e3a76c2c290ac25a0a029ffde58cf5e4c14"
    {
        return Err("Boundary-17 page/JGraphT provenance differs".to_owned());
    }
    for field in HEAD_LINKS_CORPUS_SUMMARY_FIELDS
        .iter()
        .filter(|field| field.ends_with("Sha256"))
    {
        let value = corpus_summary.value(field)?;
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(format!("Boundary-17 {field} is not lowercase SHA-256"));
        }
    }
    Ok(())
}

fn validate_head_links_public_body(text: &str) -> Result<(), String> {
    let rows = parse_head_links_rows(text)?;
    let (corpus_summary, preceding) = rows
        .split_last()
        .ok_or_else(|| "Boundary-17 fixture lacks its corpus summary".to_owned())?;
    let (page_summary, body_rows) = preceding
        .split_last()
        .ok_or_else(|| "Boundary-17 fixture lacks its page summary".to_owned())?;
    if page_summary.kind != HeadRowKind::PageSummary
        || corpus_summary.kind != HeadRowKind::CorpusSummary
    {
        return Err("Boundary-17 fixture lacks unique ordered terminal summaries".to_owned());
    }
    let (page, transactions, supplemental) = parse_real_head_transactions(body_rows)?;
    if page.value("stop")? != "ReturnedTrueBeforeOuterBLinkerAssignment"
        || page.value("relationOrder")? != "LinkedHashMapFirstInsertionLatestPayload"
        || page.value("incidentOrder")? != "IncomingThenOutgoing"
        || !page.bool("headless")?
        || page.usize("systems")? != transactions.len()
    {
        return Err("Boundary-17 page contract differs".to_owned());
    }
    let system_one = transactions
        .iter()
        .find(|transaction| transaction.key.system == 1)
        .ok_or_else(|| "Boundary-17 fixture lacks real system 1".to_owned())?;
    validate_isolated_head_cases(&supplemental, system_one)?;
    for transaction in &transactions {
        let hydrated = hydrate_boundary_sixteen_for_head(page, transaction)?;
        let mut state = project_head_links_state(transaction, &hydrated)?;
        let before = state.clone();
        let public = apply_native_stems_beam_vlink_head_links_transaction(
            &hydrated.predecessor.scheduler,
            &hydrated.predecessor.plans,
            &hydrated.predecessor.stumps,
            &hydrated.predecessor.vlinkers,
            &hydrated.predecessor.reachability,
            &hydrated.predecessor.builder,
            &hydrated.predecessor.create_transaction,
            &hydrated.predecessor.reuse_live_state,
            hydrated.predecessor.relation_parameters,
            &hydrated.predecessor.reuse_check,
            &hydrated.predecessor.base_apply,
            &hydrated.predecessor.transaction,
            &hydrated.transaction,
            &mut state,
        )
        .map_err(|error| {
            format!(
                "system {} production Boundary-17 apply failed: {error}",
                transaction.key.system
            )
        })?;
        assert_head_public_matches_rows(transaction, &hydrated, &before, &state, &public)?;
    }
    validate_head_links_trailers(
        text,
        body_rows,
        page,
        &transactions,
        &supplemental,
        page_summary,
        corpus_summary,
    )?;
    Ok(())
}

fn compare_head_manifest_and_strict_fields(
    manifest: &ManifestRow,
    strict: &HeadStrictRow,
    fields: &[&str],
) -> Result<(), String> {
    for field in fields {
        if manifest.value(field)? != strict.value(field)? {
            return Err(format!(
                "Boundary-17 manifest/fixture {} join differs for {field}",
                manifest.value("page").unwrap_or("summary")
            ));
        }
    }
    Ok(())
}

fn validate_boundary_seventeen_manifest(path: &std::path::Path) -> Result<(), String> {
    const BODY_KINDS: &[HeadRowKind] = &[
        HeadRowKind::Page,
        HeadRowKind::Predecessor,
        HeadRowKind::Baseline,
        HeadRowKind::HeadEntry,
        HeadRowKind::SLinkerWrite,
        HeadRowKind::SourceOutgoing,
        HeadRowKind::PairRelation,
        HeadRowKind::PairScan,
        HeadRowKind::Consistency,
        HeadRowKind::Edge,
        HeadRowKind::HeadIncident,
        HeadRowKind::HeadIncidentScan,
        HeadRowKind::StemIncident,
        HeadRowKind::StemIncidentScan,
        HeadRowKind::Callback,
        HeadRowKind::EntryResult,
        HeadRowKind::Remainder,
        HeadRowKind::Result,
        HeadRowKind::DeltaGuard,
        HeadRowKind::Summary,
        HeadRowKind::SyntheticCase,
        HeadRowKind::SyntheticEvent,
        HeadRowKind::SyntheticGuard,
    ];
    const PAGE_COUNTER_FIELDS: &[&str] = &[
        "systems",
        "realTransactions",
        "supportedSyntheticCases",
        "envelopeCases",
        "isolatedCases",
        "totalTransactions",
        "headEntries",
        "duplicateEntries",
        "relationsInserted",
        "sWrites",
        "sValueChanges",
        "consistencyWrites",
        "headAbnormalChanges",
        "stemAbnormalChanges",
        "dirtyCascades",
        "sheetEditMutations",
        "realEvents",
        "isolatedEvents",
        "isolatedGraphDelta",
        "isolatedThrows",
        "isolatedManualCases",
        "chordRewires",
        "stopBeforeOuterBLinkerAssignment",
    ];
    const CORPUS_ENTRY_FIELDS: &[&str] = &[
        "rowCounts",
        "pageInputSha256",
        "schedulerFixtureSha256",
        "expandFixtureSha256",
        "createStemFixtureSha256",
        "reuseCheckFixtureSha256",
        "baseApplyFixtureSha256",
        "baseApplyManifestSha256",
        "bLinkerFlagFixtureSha256",
        "bLinkerFlagManifestSha256",
        "siblingLinksFixtureSha256",
        "siblingLinksManifestSha256",
        "emittedBodySha256",
        "emittedBodyLines",
        "emittedBodyBytes",
        "rawPassSha256",
        "freshRunsPerPage",
        "freshRunsByteIdentical",
        "compilerJavaProcesses",
        "runtimeJavaProcessesPerPass",
        "runtimeJavaProcesses",
        "totalJavaProcesses",
        "maximumConcurrentJavaProcesses",
        "concurrencyScope",
        "freshJvmPerSystem",
        "compilerJavaProcessReaped",
        "runtimeJavaProcessesReaped",
        "foregroundJavaProcessesOnly",
        "backgroundJavaProcessesStarted",
        "system1IsolatedBlock",
        "supportedSyntheticEvidenceScope",
        "envelopeEvidenceScope",
        "stopBeforeOuterBLinkerAssignment",
    ];
    const AGGREGATE_FIELDS: &[(&str, &str)] = &[
        ("realSystems", "systems"),
        ("realTransactions", "realTransactions"),
        ("realHeadEntries", "headEntries"),
        ("realDuplicateEntries", "duplicateEntries"),
        ("realRelationsInserted", "relationsInserted"),
        ("realSWrites", "sWrites"),
        ("realSValueChanges", "sValueChanges"),
        ("realConsistencyWrites", "consistencyWrites"),
        ("realHeadAbnormalChanges", "headAbnormalChanges"),
        ("realStemAbnormalChanges", "stemAbnormalChanges"),
        ("realDirtyCascades", "dirtyCascades"),
        ("realSheetEditMutations", "sheetEditMutations"),
        ("realEvents", "realEvents"),
        ("supportedSyntheticCases", "supportedSyntheticCases"),
        ("envelopeCases", "envelopeCases"),
        ("isolatedCases", "isolatedCases"),
        ("totalTransactions", "totalTransactions"),
        ("isolatedEvents", "isolatedEvents"),
        ("isolatedGraphDelta", "isolatedGraphDelta"),
        ("isolatedThrows", "isolatedThrows"),
        ("isolatedManualCases", "isolatedManualCases"),
        ("chordRewires", "chordRewires"),
    ];

    let manifest_bytes =
        std::fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let manifest = std::str::from_utf8(&manifest_bytes)
        .map_err(|error| format!("{} is not UTF-8: {error}", path.display()))?;
    if !manifest_bytes.ends_with(b"\n") {
        return Err("Boundary-17 manifest must end with one newline".to_owned());
    }
    let lines = manifest.lines().collect::<Vec<_>>();
    if sha256_hex(&manifest_bytes) != HEAD_LINKS_MANIFEST_SHA256
        || lines.len() != HEAD_LINKS_MANIFEST_LINES
        || manifest_bytes.len() != HEAD_LINKS_MANIFEST_BYTES
        || HEAD_LINKS_MANIFEST_ENTRY_FIELDS.len() != 61
        || HEAD_LINKS_MANIFEST_SUMMARY_FIELDS.len() != 144
        || lines.len() != CORPUS_PAGES.len() + 2
        || lines.first().copied() != Some(HEAD_LINKS_MANIFEST_SCHEMA)
    {
        return Err("Boundary-17 manifest schema/line envelope differs".to_owned());
    }
    let entries = lines[1..=CORPUS_PAGES.len()]
        .iter()
        .map(|line| {
            ManifestRow::parse(
                line,
                HEAD_LINKS_MANIFEST_ENTRY_LABEL,
                HEAD_LINKS_MANIFEST_ENTRY_FIELDS,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let summary = ManifestRow::parse(
        lines
            .last()
            .ok_or_else(|| "Boundary-17 manifest lacks summary".to_owned())?,
        HEAD_LINKS_MANIFEST_SUMMARY_LABEL,
        HEAD_LINKS_MANIFEST_SUMMARY_FIELDS,
    )?;

    let expected_header = format!("{}\n", HEAD_LINKS_FIXTURE_HEADER.join("\n"));
    let mut normalized_corpus = expected_header.as_bytes().to_vec();
    let mut split_emitted_body = Vec::new();
    let mut split_fixture = Vec::new();
    let mut row_totals = vec![0usize; BODY_KINDS.len()];
    let mut semantic_rows = 0usize;
    let mut strict_corpora = Vec::new();
    let mut aggregate_totals = BTreeMap::<&str, usize>::new();
    let mut compiler_java_processes = 0usize;
    let mut runtime_java_processes = 0usize;
    let mut total_java_processes = 0usize;
    let mut maximum_concurrent_java_processes = 0usize;

    for (ordinal, ((page_key, _), entry)) in CORPUS_PAGES.iter().zip(&entries).enumerate() {
        let expected_fixture = format!("stems-beam-vlink-head-links-{page_key}.txt");
        if entry.usize("ordinal")? != ordinal
            || entry.value("page")? != *page_key
            || entry.value("fixture")? != expected_fixture
        {
            return Err(format!(
                "Boundary-17 manifest entry {ordinal} identity/order differs"
            ));
        }
        let fixture_path = repo_root().join("rust/oracle").join(&expected_fixture);
        let fixture_bytes = std::fs::read(&fixture_path)
            .map_err(|error| format!("cannot read {}: {error}", fixture_path.display()))?;
        let fixture = std::str::from_utf8(&fixture_bytes)
            .map_err(|error| format!("{} is not UTF-8: {error}", fixture_path.display()))?;
        if !fixture_bytes.ends_with(b"\n") || !fixture.starts_with(&expected_header) {
            return Err(format!(
                "Boundary-17 manifest {page_key} fixture envelope differs"
            ));
        }
        validate_head_links_public_body(fixture)?;
        let rows = parse_head_links_rows(fixture)?;
        let (corpus_summary, preceding) = rows
            .split_last()
            .ok_or_else(|| format!("Boundary-17 manifest {page_key} lacks corpus summary"))?;
        let (page_summary, body_rows) = preceding
            .split_last()
            .ok_or_else(|| format!("Boundary-17 manifest {page_key} lacks page summary"))?;
        if page_summary.kind != HeadRowKind::PageSummary
            || corpus_summary.kind != HeadRowKind::CorpusSummary
        {
            return Err(format!(
                "Boundary-17 manifest {page_key} terminal row order differs"
            ));
        }
        let (page, _, _) = parse_real_head_transactions(body_rows)?;
        if corpus_summary.value("mode")? != *page_key
            || page_summary.page != page.page
            || entry.value("rowCounts")? != corpus_summary.value("rowCounts")?
        {
            return Err(format!(
                "Boundary-17 manifest {page_key} page identity differs"
            ));
        }
        compare_head_manifest_and_strict_fields(entry, page_summary, PAGE_COUNTER_FIELDS)?;
        compare_head_manifest_and_strict_fields(entry, corpus_summary, CORPUS_ENTRY_FIELDS)?;

        let fixture_lines = fixture_bytes.iter().filter(|byte| **byte == b'\n').count();
        if entry.value("fixtureSha256")? != sha256_hex(&fixture_bytes)
            || entry.usize("fixtureLines")? != fixture_lines
            || entry.usize("fixtureBytes")? != fixture_bytes.len()
        {
            return Err(format!(
                "Boundary-17 manifest {page_key} fixture identity differs"
            ));
        }
        let marker = format!(
            "\n{} {} ",
            HeadRowKind::PageSummary.label(),
            page_summary.page
        );
        let body_end = fixture
            .rfind(&marker)
            .ok_or_else(|| format!("Boundary-17 manifest {page_key} body marker is absent"))?
            + 1;
        let emitted_body = &fixture_bytes[..body_end];
        if entry.value("emittedBodySha256")? != sha256_hex(emitted_body)
            || entry.value("rawPassSha256")? != sha256_hex(emitted_body)
            || entry.usize("emittedBodyLines")?
                != emitted_body.iter().filter(|byte| **byte == b'\n').count()
            || entry.usize("emittedBodyBytes")? != emitted_body.len()
        {
            return Err(format!(
                "Boundary-17 manifest {page_key} emitted-body identity differs"
            ));
        }
        normalized_corpus.extend_from_slice(&emitted_body[expected_header.len()..]);
        split_emitted_body.extend_from_slice(emitted_body);
        split_fixture.extend_from_slice(&fixture_bytes);
        semantic_rows += body_rows.len();
        for (index, kind) in BODY_KINDS.iter().enumerate() {
            row_totals[index] += body_rows.iter().filter(|row| row.kind == *kind).count();
        }
        for (_, page_field) in AGGREGATE_FIELDS {
            *aggregate_totals.entry(page_field).or_default() += page_summary.usize(page_field)?;
        }
        compiler_java_processes += corpus_summary.usize("compilerJavaProcesses")?;
        runtime_java_processes += corpus_summary.usize("runtimeJavaProcesses")?;
        total_java_processes += corpus_summary.usize("totalJavaProcesses")?;
        maximum_concurrent_java_processes = maximum_concurrent_java_processes
            .max(corpus_summary.usize("maximumConcurrentJavaProcesses")?);
        strict_corpora.push(corpus_summary.clone());
    }

    if summary.value("schema")? != "stems-beam-vlink-head-links-manifest-v1"
        || summary.usize("entries")? != entries.len()
    {
        return Err("Boundary-17 manifest summary schema/entry count differs".to_owned());
    }
    let first_corpus = strict_corpora
        .first()
        .ok_or_else(|| "Boundary-17 manifest has no corpus summaries".to_owned())?;
    for field in HEAD_LINKS_MANIFEST_SUMMARY_FIELDS
        .iter()
        .skip(2)
        .take_while(|field| **field != "sharedHeaderSha256")
    {
        let expected = first_corpus.value(field)?;
        if summary.value(field)? != expected
            || strict_corpora
                .iter()
                .any(|corpus| corpus.value(field) != Ok(expected))
        {
            return Err(format!(
                "Boundary-17 manifest shared provenance differs for {field}"
            ));
        }
    }
    let corpus_row_counts = BODY_KINDS
        .iter()
        .zip(&row_totals)
        .map(|(kind, count)| format!("{}:{count}", kind.label()))
        .collect::<Vec<_>>()
        .join(",");
    if summary.value("sharedHeaderSha256")? != sha256_hex(expected_header.as_bytes())
        || summary.usize("sharedHeaderLines")? != HEAD_LINKS_FIXTURE_HEADER.len()
        || summary.usize("sharedHeaderBytes")? != expected_header.len()
        || summary.value("corpusBodySha256")? != sha256_hex(&normalized_corpus)
        || summary.usize("corpusBodyLines")?
            != normalized_corpus
                .iter()
                .filter(|byte| **byte == b'\n')
                .count()
        || summary.usize("corpusBodyBytes")? != normalized_corpus.len()
        || summary.value("corpusRowCounts")? != corpus_row_counts
        || summary.value("corpusReconstruction")? != "SharedHeaderOnceThenPageSemanticRows"
        || summary.usize("semanticRows")? != semantic_rows
        || summary.value("splitEmittedBodySha256")? != sha256_hex(&split_emitted_body)
        || summary.usize("splitEmittedBodyLines")?
            != split_emitted_body
                .iter()
                .filter(|byte| **byte == b'\n')
                .count()
        || summary.usize("splitEmittedBodyBytes")? != split_emitted_body.len()
        || summary.value("splitFixtureSha256")? != sha256_hex(&split_fixture)
        || summary.usize("splitFixtureLines")?
            != split_fixture.iter().filter(|byte| **byte == b'\n').count()
        || summary.usize("splitFixtureBytes")? != split_fixture.len()
        || sha256_hex(&normalized_corpus) != HEAD_LINKS_NORMALIZED_CORPUS_SHA256
        || normalized_corpus
            .iter()
            .filter(|byte| **byte == b'\n')
            .count()
            != HEAD_LINKS_NORMALIZED_CORPUS_LINES
        || normalized_corpus.len() != HEAD_LINKS_NORMALIZED_CORPUS_BYTES
        || sha256_hex(&split_emitted_body) != HEAD_LINKS_SPLIT_EMITTED_BODY_SHA256
        || split_emitted_body
            .iter()
            .filter(|byte| **byte == b'\n')
            .count()
            != HEAD_LINKS_SPLIT_EMITTED_BODY_LINES
        || split_emitted_body.len() != HEAD_LINKS_SPLIT_EMITTED_BODY_BYTES
        || sha256_hex(&split_fixture) != HEAD_LINKS_SPLIT_FIXTURE_SHA256
        || split_fixture.iter().filter(|byte| **byte == b'\n').count()
            != HEAD_LINKS_SPLIT_FIXTURE_LINES
        || split_fixture.len() != HEAD_LINKS_SPLIT_FIXTURE_BYTES
    {
        return Err("Boundary-17 manifest corpus reconstruction differs".to_owned());
    }

    for (summary_field, page_field) in AGGREGATE_FIELDS {
        if summary.usize(summary_field)?
            != aggregate_totals
                .get(page_field)
                .copied()
                .unwrap_or_default()
        {
            return Err(format!(
                "Boundary-17 manifest aggregate {summary_field} differs"
            ));
        }
    }
    if summary.usize("syntheticBlocks")? != entries.len()
        || summary.usize("compilerJavaProcesses")? != compiler_java_processes
        || summary.usize("runtimeJavaProcesses")? != runtime_java_processes
        || summary.usize("totalJavaProcesses")? != total_java_processes
        || summary.usize("maximumConcurrentJavaProcesses")? != maximum_concurrent_java_processes
        || summary.value("concurrencyScope")? != "Boundary17RunnerLockedInvocation"
        || summary.usize("freshRunsPerPage")? != 2
        || !summary.bool("freshRunsByteIdentical")?
        || !summary.bool("freshJvmPerSystem")?
        || !summary.bool("compilerJavaProcessesReaped")?
        || !summary.bool("runtimeJavaProcessesReaped")?
        || !summary.bool("foregroundJavaProcessesOnly")?
        || summary.usize("backgroundJavaProcessesStarted")? != 0
        || !summary.bool("system1IsolatedBlock")?
        || summary.value("supportedSyntheticEvidenceScope")?
            != "IsolatedJavaGateOnlyNotProductionEquivalent"
        || summary.value("envelopeEvidenceScope")? != "IsolatedJavaGateOnlyNotProductionEquivalent"
        || !summary.bool("stopBeforeOuterBLinkerAssignment")?
    {
        return Err("Boundary-17 manifest execution/evidence constants differ".to_owned());
    }

    let summary_marker = format!("{HEAD_LINKS_MANIFEST_SUMMARY_LABEL} ");
    let body_matches = manifest
        .match_indices(&summary_marker)
        .filter(|(index, _)| {
            *index == 0 || manifest_bytes.get(index.wrapping_sub(1)) == Some(&b'\n')
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if body_matches.len() != 1 {
        return Err("Boundary-17 manifest lacks one line-aligned summary".to_owned());
    }
    let body = &manifest_bytes[..body_matches[0]];
    if summary.value("manifestBodySha256")? != sha256_hex(body)
        || summary.usize("manifestBodyLines")? != body.iter().filter(|byte| **byte == b'\n').count()
        || summary.usize("manifestBodyBytes")? != body.len()
        || sha256_hex(body) != HEAD_LINKS_MANIFEST_BODY_SHA256
        || body.iter().filter(|byte| **byte == b'\n').count() != HEAD_LINKS_MANIFEST_BODY_LINES
        || body.len() != HEAD_LINKS_MANIFEST_BODY_BYTES
    {
        return Err("Boundary-17 manifest self-pinned body differs".to_owned());
    }
    Ok(())
}

#[test]
fn installed_boundary_seventeen_prefix_is_strictly_replayed() {
    let mut installed = 0;
    let mut saw_missing = false;
    for (key, _) in CORPUS_PAGES {
        let path = repo_root().join(boundary_seventeen_fixture_path(key));
        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() => {
                assert!(
                    !saw_missing,
                    "installed Boundary-17 corpus has a gap before {key}"
                );
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
                validate_head_links_public_body(&text)
                    .unwrap_or_else(|error| panic!("{key} exact replay: {error}"));
                installed += 1;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => saw_missing = true,
            Ok(_) => panic!("{} is not a file", path.display()),
            Err(error) => panic!("cannot inspect {}: {error}", path.display()),
        }
    }
    assert!(installed > 0, "Boundary-17 installed prefix is empty");
}

#[test]
fn installed_boundary_seventeen_manifest_is_exactly_reconstructed_and_pinned() {
    let path = std::env::var_os(HEAD_LINKS_MANIFEST_OVERRIDE_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join(HEAD_LINKS_MANIFEST_PATH));
    validate_boundary_seventeen_manifest(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

fn head_java_printf_fields(source: &str, kind: HeadRowKind) -> Result<Vec<String>, String> {
    let dynamic_incident = matches!(
        kind,
        HeadRowKind::HeadIncident
            | HeadRowKind::HeadIncidentScan
            | HeadRowKind::StemIncident
            | HeadRowKind::StemIncidentScan
    );
    let dynamic_label = match kind {
        HeadRowKind::HeadIncident | HeadRowKind::StemIncident => {
            "stemsbeamvlinkheadlinks%sincident"
        }
        HeadRowKind::HeadIncidentScan | HeadRowKind::StemIncidentScan => {
            "stemsbeamvlinkheadlinks%sincidentscan"
        }
        _ => kind.label(),
    };
    let marker = format!("\"{dynamic_label} ");
    let start = source
        .find(&marker)
        .ok_or_else(|| format!("Java probe lacks Boundary-17 {kind:?} printf"))?;
    let mut format_text = String::new();
    for line in source[start..].lines() {
        let Some(first_quote) = line.find('"') else {
            continue;
        };
        let last_quote = line
            .rfind('"')
            .ok_or_else(|| format!("unterminated Boundary-17 {kind:?} format literal"))?;
        format_text.push_str(&line[first_quote + 1..last_quote]);
        if line[first_quote + 1..last_quote].contains("%n") {
            break;
        }
    }
    let format_text = format_text.replace("%n", "");
    let tokens = format_text.split_ascii_whitespace().collect::<Vec<_>>();
    if tokens.first().copied() != Some(dynamic_label)
        || tokens.get(1).copied() != Some("%s")
        || (dynamic_incident && !tokens[0].contains("%s"))
    {
        return Err(format!(
            "Boundary-17 {kind:?} format lacks label/page prefix"
        ));
    }
    if (tokens.len() - 2) % 2 != 0 {
        return Err(format!(
            "Boundary-17 {kind:?} format is not key/value shaped"
        ));
    }
    let mut fields = match kind {
        HeadRowKind::Page => Vec::new(),
        HeadRowKind::Predecessor
        | HeadRowKind::Baseline
        | HeadRowKind::Remainder
        | HeadRowKind::Result
        | HeadRowKind::DeltaGuard
        | HeadRowKind::Summary => HEAD_LINKS_COMMON_FIELDS
            .iter()
            .map(|field| (*field).to_owned())
            .collect(),
        HeadRowKind::SyntheticCase | HeadRowKind::SyntheticEvent | HeadRowKind::SyntheticGuard => {
            HEAD_LINKS_COMMON_FIELDS
                .iter()
                .map(|field| (*field).to_owned())
                .collect()
        }
        _ => HEAD_LINKS_ENTRY_COMMON_FIELDS
            .iter()
            .map(|field| (*field).to_owned())
            .collect(),
    };
    fields.extend(tokens[2..].chunks_exact(2).map(|pair| pair[0].to_owned()));
    Ok(fields)
}

fn head_runner_printf_fields(source: &str, kind: HeadRowKind) -> Result<Vec<String>, String> {
    if !matches!(kind, HeadRowKind::PageSummary | HeadRowKind::CorpusSummary) {
        return Err("Boundary-17 runner field extraction requires a trailer kind".to_owned());
    }
    let prefix = format!("printf '{} ", kind.label());
    let line = source
        .lines()
        .find(|line| line.starts_with(&prefix))
        .ok_or_else(|| format!("Boundary-17 runner lacks {kind:?} printf"))?;
    let first_quote = line
        .find('\'')
        .ok_or_else(|| format!("Boundary-17 {kind:?} printf lacks opening quote"))?;
    let last_quote = line
        .rfind('\'')
        .filter(|last| *last > first_quote)
        .ok_or_else(|| format!("Boundary-17 {kind:?} printf lacks closing quote"))?;
    let tokens = line[first_quote + 1..last_quote]
        .split_ascii_whitespace()
        .collect::<Vec<_>>();
    let pair_start = if kind == HeadRowKind::CorpusSummary {
        1
    } else {
        2
    };
    if tokens.first().copied() != Some(kind.label()) || (tokens.len() - pair_start) % 2 != 0 {
        return Err(format!(
            "Boundary-17 {kind:?} printf is not ordered key/value data"
        ));
    }
    let mut fields = tokens[pair_start..]
        .chunks_exact(2)
        .map(|pair| pair[0].to_owned())
        .collect::<Vec<_>>();
    if kind == HeadRowKind::CorpusSummary {
        let insertion = fields
            .iter()
            .position(|field| field == "sigraphClassSha256")
            .ok_or_else(|| "Boundary-17 corpus printf lacks class/source boundary".to_owned())?
            + 1;
        let mut source_fields = CORPUS_SOURCE_PINS[..CORPUS_SOURCE_PINS.len() - 1]
            .iter()
            .map(|(field, _)| (*field).to_owned())
            .collect::<Vec<_>>();
        source_fields.extend(
            [
                "headLinkerSourceSha256",
                "headInterSourceSha256",
                "headStemRelationSourceSha256",
                "partSourceSha256",
            ]
            .map(str::to_owned),
        );
        source_fields.push("gradleSourceSha256".to_owned());
        fields.splice(insertion..insertion, source_fields);
    }
    Ok(fields)
}

#[test]
fn boundary_seventeen_java_core_printf_fields_match_strict_arrays() {
    let source = std::fs::read_to_string(repo_root().join(HEAD_LINKS_PROBE_SOURCE_PATH))
        .expect("read Boundary-17 Java probe");
    assert!(repo_root().join(HEAD_LINKS_RUNNER_SOURCE_PATH).is_file());
    for kind in [
        HeadRowKind::Page,
        HeadRowKind::Predecessor,
        HeadRowKind::Baseline,
        HeadRowKind::HeadEntry,
        HeadRowKind::SLinkerWrite,
        HeadRowKind::SourceOutgoing,
        HeadRowKind::PairRelation,
        HeadRowKind::PairScan,
        HeadRowKind::Consistency,
        HeadRowKind::Edge,
        HeadRowKind::HeadIncident,
        HeadRowKind::HeadIncidentScan,
        HeadRowKind::StemIncident,
        HeadRowKind::StemIncidentScan,
        HeadRowKind::Callback,
        HeadRowKind::EntryResult,
        HeadRowKind::Remainder,
        HeadRowKind::Result,
        HeadRowKind::DeltaGuard,
        HeadRowKind::Summary,
        HeadRowKind::SyntheticCase,
        HeadRowKind::SyntheticEvent,
        HeadRowKind::SyntheticGuard,
    ] {
        assert_eq!(
            head_java_printf_fields(&source, kind).unwrap(),
            kind.fields(),
            "Boundary-17 {kind:?} Java/Rust field drift"
        );
    }
    let runner = std::fs::read_to_string(repo_root().join(HEAD_LINKS_RUNNER_SOURCE_PATH))
        .expect("read Boundary-17 oracle runner");
    for kind in [HeadRowKind::PageSummary, HeadRowKind::CorpusSummary] {
        assert_eq!(
            head_runner_printf_fields(&runner, kind).unwrap(),
            kind.fields(),
            "Boundary-17 {kind:?} runner/Rust field drift"
        );
    }
}

#[test]
fn installed_boundary_sixteen_prefix_is_strictly_replayed() {
    if let Some(path) = std::env::var_os(FIXTURE_OVERRIDE_ENV).map(PathBuf::from) {
        validate_installed_boundary_sixteen_fixture(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        return;
    }
    let mut installed = 0;
    let mut saw_missing = false;
    for (key, _) in CORPUS_PAGES {
        let path = repo_root().join(boundary_sixteen_fixture_path(key));
        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() => {
                assert!(
                    !saw_missing,
                    "installed Boundary-16 corpus has a gap before {key}"
                );
                validate_installed_boundary_sixteen_fixture(&path)
                    .unwrap_or_else(|error| panic!("{key} exact replay: {error}"));
                installed += 1;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => saw_missing = true,
            Ok(_) => panic!("{} is not a file", path.display()),
            Err(error) => panic!("cannot inspect {}: {error}", path.display()),
        }
    }
    assert!(installed > 0, "Boundary-16 installed prefix is empty");
}

#[test]
fn installed_boundary_sixteen_manifest_is_exactly_reconstructed_and_pinned() {
    let path = std::env::var_os(MANIFEST_OVERRIDE_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join(MANIFEST_PATH));
    validate_boundary_sixteen_manifest(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

#[test]
fn frozen_java_printf_field_arrays_match_the_strict_parser() {
    let source = std::fs::read_to_string(repo_root().join(PROBE_SOURCE_PATH))
        .expect("Boundary-16 Java probe source");
    for kind in [
        RowKind::Page,
        RowKind::Predecessor,
        RowKind::Baseline,
        RowKind::GroupMember,
        RowKind::Sibling,
        RowKind::SourceOutgoing,
        RowKind::PairRelation,
        RowKind::Geometry,
        RowKind::Edge,
        RowKind::StemIncident,
        RowKind::BeamIncident,
        RowKind::Callback,
        RowKind::LinkerLookup,
        RowKind::LinkerFlag,
        RowKind::SiblingResult,
        RowKind::Result,
        RowKind::DeltaGuard,
        RowKind::Summary,
        RowKind::SyntheticCase,
        RowKind::SyntheticEvent,
        RowKind::SyntheticGuard,
    ] {
        assert_eq!(
            java_printf_fields(&source, kind).expect("Java printf field extraction"),
            kind.expected_fields(),
            "{kind:?} parser/source field order differs"
        );
    }
    let runner = std::fs::read_to_string(repo_root().join(RUNNER_SOURCE_PATH))
        .expect("Boundary-16 oracle runner source");
    assert_eq!(
        runner_corpus_printf_fields(&runner).expect("runner corpus printf field extraction"),
        CORPUS_SUMMARY_FIELDS,
        "CorpusSummary parser/runner field order differs"
    );
}

#[test]
fn boundary_fifteen_fixture_manifest_and_typed_gate_are_exactly_pinned() {
    assert_eq!(
        read_sha256(BOUNDARY_FIFTEEN_MANIFEST_PATH).expect("Boundary-15 manifest"),
        BOUNDARY_FIFTEEN_MANIFEST_SHA256
    );
    assert_eq!(
        read_sha256(BOUNDARY_FIFTEEN_FIXTURE_PATH).expect("Boundary-15 Chula fixture"),
        BOUNDARY_FIFTEEN_FIXTURE_SHA256
    );
    assert_eq!(
        read_sha256(BOUNDARY_FIFTEEN_GATE_PATH).expect("Boundary-15 typed replay gate"),
        BOUNDARY_FIFTEEN_GATE_SHA256
    );
    validate_manifest_fixture_pin().expect("Boundary-15 manifest split pin");
}

fn horizontal(x1: f64, x2: f64, y: f64) -> Line {
    Line {
        x1,
        y1: y,
        x2,
        y2: y,
    }
}

fn serial_fixture() -> (IndependentInput, IndependentState) {
    let base = BeamId(10);
    let same_glyph = BeamId(11);
    let existing = BeamId(12);
    let shorter_wrong_side = BeamId(13);
    let full_link = BeamId(14);
    let hook_link = BeamId(15);
    let stem = StemId(20);
    let beams = BTreeMap::from([
        (
            base,
            BeamState {
                runtime_class: BeamRuntimeClass::Beam,
                glyph_identity: Some(100),
                median: horizontal(0.0, 10.0, 10.0),
                height: 4.0,
                abnormal: true,
            },
        ),
        (
            same_glyph,
            BeamState {
                runtime_class: BeamRuntimeClass::Beam,
                glyph_identity: Some(100),
                median: horizontal(0.0, 10.0, 0.0),
                height: 4.0,
                abnormal: true,
            },
        ),
        (
            existing,
            BeamState {
                runtime_class: BeamRuntimeClass::Beam,
                glyph_identity: Some(102),
                median: horizontal(0.0, 10.0, 25.0),
                height: 4.0,
                abnormal: true,
            },
        ),
        (
            shorter_wrong_side,
            BeamState {
                runtime_class: BeamRuntimeClass::Beam,
                glyph_identity: Some(103),
                median: horizontal(1.0, 9.0, 5.0),
                height: 4.0,
                abnormal: true,
            },
        ),
        (
            full_link,
            BeamState {
                runtime_class: BeamRuntimeClass::SmallBeam,
                glyph_identity: Some(104),
                median: horizontal(0.0, 10.0, 15.0),
                height: 4.0,
                abnormal: true,
            },
        ),
        (
            hook_link,
            BeamState {
                runtime_class: BeamRuntimeClass::Hook,
                glyph_identity: Some(105),
                median: horizontal(0.0, 10.0, 20.0),
                height: 2.0,
                abnormal: true,
            },
        ),
    ]);
    let vertices = beams
        .keys()
        .copied()
        .map(Vertex::Beam)
        .chain([Vertex::Stem(stem)])
        .collect();
    let grade_bits = 0.75_f64.to_bits();
    let state = IndependentState {
        vertices,
        edges: vec![
            GraphEdge {
                relation_identity: 1,
                source: Vertex::Beam(existing),
                target: Vertex::Stem(stem),
                payload: RelationPayload::Other,
            },
            GraphEdge {
                relation_identity: 2,
                source: Vertex::Beam(existing),
                target: Vertex::Stem(stem),
                payload: RelationPayload::BeamStem {
                    portion: BeamPortion::Left,
                    grade_bits,
                },
            },
            GraphEdge {
                relation_identity: 3,
                source: Vertex::Beam(existing),
                target: Vertex::Stem(stem),
                payload: RelationPayload::Other,
            },
            GraphEdge {
                relation_identity: 4,
                source: Vertex::Beam(full_link),
                target: Vertex::Beam(base),
                payload: RelationPayload::BeamRest {
                    portion: BeamPortion::Left,
                },
            },
        ],
        beams,
        builder_items: vec![
            BuilderItem::NonLinker,
            BuilderItem::Linker {
                source: full_link,
                linker: LinkerId(10),
                linked_cell: LinkerCellId(1),
            },
            BuilderItem::Linker {
                source: full_link,
                linker: LinkerId(20),
                linked_cell: LinkerCellId(2),
            },
            BuilderItem::Linker {
                source: hook_link,
                linker: LinkerId(30),
                linked_cell: LinkerCellId(3),
            },
        ],
        linked_cells: BTreeMap::from([
            (LinkerCellId(1), false),
            (LinkerCellId(2), false),
            (LinkerCellId(3), true),
        ]),
        linked_cell_observers: BTreeMap::from([
            (
                LinkerCellId(1),
                vec![LinkerId(10), LinkerId(11), LinkerId(13)],
            ),
            (LinkerCellId(2), vec![LinkerId(20), LinkerId(21)]),
            (
                LinkerCellId(3),
                vec![LinkerId(30), LinkerId(31), LinkerId(32)],
            ),
        ]),
        stub_modified: false,
        book_modified: false,
        book_dirty: false,
        next_relation_identity: 5,
    };
    let input = IndependentInput {
        base_beam: base,
        stem,
        ref_point: Point { x: 5.0, y: 10.0 },
        skewed_vertical: Line {
            x1: 5.0,
            y1: -100.0,
            x2: 5.0,
            y2: 100.0,
        },
        stem_median: Line {
            x1: 5.0,
            y1: -100.0,
            x2: 5.0,
            y2: 100.0,
        },
        // Deliberately not top-to-bottom. Discovery must sort stably by
        // intersection ordinate before removing only the first base identity.
        group_members: vec![
            existing,
            hook_link,
            base,
            shorter_wrong_side,
            same_glyph,
            full_link,
        ],
        max_beam_side_dx: 2.0,
        max_shorter_ratio: 0.8,
        portion_max_dx: 2,
        y_dir: 1,
        continuation_support_grade_bits: grade_bits,
        add_edge_behavior: BTreeMap::new(),
    };
    (input, state)
}

#[test]
fn public_boundary_shape_pins_four_branches_and_head_loop_terminal() {
    assert_eq!(
        [
            SiblingBranch::SameGlyph,
            SiblingBranch::ExistingBeamStem,
            SiblingBranch::ShorterWrongSide,
            SiblingBranch::Linked,
        ]
        .map(native_branch),
        [
            NativeStemsBeamSiblingBranch::SameGlyph,
            NativeStemsBeamSiblingBranch::ExistingBeamStem,
            NativeStemsBeamSiblingBranch::ShorterWrongSide,
            NativeStemsBeamSiblingBranch::Linked,
        ]
    );
    let outcome = NativeStemsBeamVLinkSiblingLinksOutcome::ReadyBeforeHeadRelationLoop {
        stem_identity: 17,
        continuation_support_grade: 0.75,
    };
    let NativeStemsBeamVLinkSiblingLinksOutcome::ReadyBeforeHeadRelationLoop {
        stem_identity,
        continuation_support_grade,
    } = outcome;
    assert_eq!(stem_identity, 17);
    assert_eq!(continuation_support_grade.to_bits(), 0.75_f64.to_bits());

    let _entry_point = apply_native_stems_beam_vlink_sibling_links_transaction;
    let _certificate_binding: Option<NativeStemsBeamVLinkSiblingLinksCertificate> = None;
    let _state_binding: Option<NativeStemsBeamVLinkSiblingLinksState> = None;
    let _transaction_binding: Option<NativeStemsBeamVLinkSiblingLinksTransaction> = None;
    let _baseline_projection: fn(&NativeStemsBeamVLinkSiblingLinksTransaction) =
        assert_public_baseline_projection;
    let _state_projection: fn(
        &NativeStemsBeamVLinkSiblingLinksState,
        &NativeStemsBeamSiblingLiveBeam,
    ) = assert_public_state_projection;
}

#[test]
fn supplemental_case_matrix_is_eight_supported_then_two_envelopes() {
    assert_eq!(
        SUPPLEMENTAL_CASES
            .iter()
            .map(|spec| spec.name)
            .collect::<Vec<_>>(),
        [
            "SameGlyph",
            "ExistingBeamStem",
            "ShorterWrongSide",
            "LinkedBeam",
            "LinkedSmallBeam",
            "LinkedHook",
            "LinkedNoBLinker",
            "LinkedIdempotentBCell",
            "ThrowBeforeInsertion",
            "ThrowDuringCallback",
        ]
    );
    assert_eq!(
        SUPPLEMENTAL_CASES
            .iter()
            .filter(|spec| spec.scope == "synthetic")
            .count(),
        8
    );
    assert_eq!(
        SUPPLEMENTAL_CASES
            .iter()
            .filter(|spec| spec.scope == "envelope")
            .count(),
        2
    );
    assert_eq!(
        SUPPLEMENTAL_CASES
            .iter()
            .filter(|spec| spec.sibling_class.ends_with("SmallBeamInter"))
            .map(|spec| spec.name)
            .collect::<Vec<_>>(),
        ["LinkedSmallBeam"]
    );
}

#[test]
fn independent_model_preserves_branches_callback_and_flag_interleaving() {
    let (input, mut state) = serial_fixture();
    let transaction = apply_independent(&input, &mut state).expect("independent sibling loop");
    assert_eq!(transaction.ref_point, input.ref_point);
    assert_eq!(transaction.terminal, Terminal::ReadyBeforeHeadRelationLoop);
    assert_eq!(
        transaction.sorted_siblings_before_base_removal,
        vec![
            BeamId(11),
            BeamId(13),
            BeamId(10),
            BeamId(14),
            BeamId(15),
            BeamId(12),
        ]
    );
    assert_eq!(transaction.base_removal_index, Some(2));
    assert_eq!(
        transaction
            .sibling_traces
            .iter()
            .map(|trace| trace.branch)
            .collect::<Vec<_>>(),
        vec![
            SiblingBranch::SameGlyph,
            SiblingBranch::ShorterWrongSide,
            SiblingBranch::Linked,
            SiblingBranch::Linked,
            SiblingBranch::ExistingBeamStem,
        ]
    );
    let shorter = &transaction.sibling_traces[1];
    let shorter_geometry = shorter.geometry.as_ref().expect("shorter geometry");
    assert_eq!(shorter_geometry.ratio.to_bits(), 0.8_f64.to_bits());
    assert!(shorter_geometry.shorter_branch_read);
    assert_eq!(shorter_geometry.dy, Some(-5.0));
    assert_eq!(shorter_geometry.dy_times_y_dir, Some(-5.0));
    assert_eq!(shorter_geometry.extension, None);
    assert_eq!(shorter_geometry.portion, None);
    assert_eq!(shorter_geometry.grade_bits, None);

    let full = &transaction.sibling_traces[2];
    assert_eq!(full.first_event_ordinal, 0);
    assert_eq!(full.event_count, 3);
    assert_eq!(
        full.geometry.as_ref().and_then(|geometry| geometry.portion),
        Some(BeamPortion::Center)
    );
    let full_callback = full.callback.as_ref().expect("full callback");
    assert_eq!(full_callback.beam_incident_relation_identities, vec![4, 5]);
    assert_eq!(
        (full_callback.abnormal_before, full_callback.abnormal_after),
        (true, true)
    );
    let full_lookup = full.linker_lookup.as_ref().expect("full lookup");
    assert_eq!(full_lookup.examined_item_ordinals, vec![0, 1]);
    assert_eq!(full_lookup.matched_item_ordinal, Some(1));
    assert_eq!(full_lookup.unread_suffix, 2);
    assert_eq!(
        (full_lookup.write_count, full_lookup.value_change_count),
        (1, 1)
    );

    let hook = &transaction.sibling_traces[3];
    assert_eq!(hook.first_event_ordinal, 3);
    assert_eq!(hook.event_count, 3);
    let hook_callback = hook.callback.as_ref().expect("hook callback");
    assert_eq!(
        (hook_callback.abnormal_before, hook_callback.abnormal_after),
        (true, false)
    );
    assert!(!hook_callback.stub_modified_before);
    assert!(hook_callback.stub_modified_after);
    assert!(hook_callback.book_modified_after);
    assert!(hook_callback.book_dirty_after);
    let hook_lookup = hook.linker_lookup.as_ref().expect("hook lookup");
    assert_eq!(hook_lookup.examined_item_ordinals, vec![0, 1, 2, 3]);
    assert_eq!(hook_lookup.matched_item_ordinal, Some(3));
    assert_eq!(
        (hook_lookup.write_count, hook_lookup.value_change_count),
        (1, 0)
    );

    let existing = &transaction.sibling_traces[4];
    assert_eq!(existing.pair_relations.len(), 3);
    assert!(existing.pair_relations[0].class_read);
    assert!(!existing.pair_relations[0].matched_beam_stem);
    assert!(existing.pair_relations[1].class_read);
    assert!(existing.pair_relations[1].matched_beam_stem);
    assert!(!existing.pair_relations[2].class_read);
    assert!(!existing.pair_relations[2].matched_beam_stem);
    assert_eq!(
        transaction.events,
        vec![
            SerialEvent::EdgeInserted {
                sibling_ordinal: 2,
                relation_identity: 5,
            },
            SerialEvent::CallbackCompleted {
                sibling_ordinal: 2,
                relation_identity: 5,
            },
            SerialEvent::LinkerFlagAssigned {
                sibling_ordinal: 2,
                selected_linker: LinkerId(10),
                cell: LinkerCellId(1),
                ordered_observers: vec![LinkerId(10), LinkerId(11), LinkerId(13)],
                before: false,
                after: true,
            },
            SerialEvent::EdgeInserted {
                sibling_ordinal: 3,
                relation_identity: 6,
            },
            SerialEvent::CallbackCompleted {
                sibling_ordinal: 3,
                relation_identity: 6,
            },
            SerialEvent::LinkerFlagAssigned {
                sibling_ordinal: 3,
                selected_linker: LinkerId(30),
                cell: LinkerCellId(3),
                ordered_observers: vec![LinkerId(30), LinkerId(31), LinkerId(32)],
                before: true,
                after: true,
            },
        ]
    );
    assert_eq!(state.edges.len(), 6);
    assert!(state.linked_cells[&LinkerCellId(1)]);
    assert!(!state.linked_cells[&LinkerCellId(2)]);
    assert!(state.linked_cells[&LinkerCellId(3)]);
}

#[test]
fn java_double_order_is_stable_and_canonicalizes_nan() {
    assert_eq!(java_double_compare(-0.0, 0.0), Ordering::Less);
    assert_eq!(java_double_compare(0.0, -0.0), Ordering::Greater);
    assert_eq!(java_double_compare(f64::NEG_INFINITY, -0.0), Ordering::Less);
    let negative_nan = f64::from_bits(0xfff0_0000_0000_0001);
    let payload_nan = f64::from_bits(0x7ff0_0000_0000_0042);
    assert_eq!(
        java_double_compare(negative_nan, payload_nan),
        Ordering::Equal
    );
    let mut stable = vec![(0, payload_nan), (1, negative_nan), (2, payload_nan)];
    stable.sort_by(|left, right| java_double_compare(left.1, right.1));
    assert_eq!(
        stable.into_iter().map(|(id, _)| id).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}

fn one_sibling_fixture() -> (IndependentInput, IndependentState) {
    let base = BeamId(1);
    let sibling = BeamId(2);
    let stem = StemId(1);
    let beams = BTreeMap::from([
        (
            base,
            BeamState {
                runtime_class: BeamRuntimeClass::Beam,
                glyph_identity: None,
                median: horizontal(0.0, 10.0, 0.0),
                height: 4.0,
                abnormal: true,
            },
        ),
        (
            sibling,
            BeamState {
                runtime_class: BeamRuntimeClass::Hook,
                glyph_identity: Some(2),
                median: horizontal(0.0, 10.0, 10.0),
                height: 2.0,
                abnormal: true,
            },
        ),
    ]);
    let state = IndependentState {
        vertices: BTreeSet::from([
            Vertex::Beam(base),
            Vertex::Beam(sibling),
            Vertex::Stem(stem),
        ]),
        edges: Vec::new(),
        beams,
        builder_items: vec![BuilderItem::Linker {
            source: sibling,
            linker: LinkerId(1),
            linked_cell: LinkerCellId(1),
        }],
        linked_cells: BTreeMap::from([(LinkerCellId(1), false)]),
        linked_cell_observers: BTreeMap::from([(LinkerCellId(1), vec![LinkerId(1), LinkerId(2)])]),
        stub_modified: false,
        book_modified: false,
        book_dirty: false,
        next_relation_identity: 1,
    };
    let input = IndependentInput {
        base_beam: base,
        stem,
        ref_point: Point { x: 5.0, y: 0.0 },
        skewed_vertical: Line {
            x1: 5.0,
            y1: -100.0,
            x2: 5.0,
            y2: 100.0,
        },
        stem_median: Line {
            x1: 5.0,
            y1: -100.0,
            x2: 5.0,
            y2: 100.0,
        },
        group_members: vec![base, sibling],
        max_beam_side_dx: 2.0,
        max_shorter_ratio: 0.8,
        portion_max_dx: 2,
        y_dir: 1,
        continuation_support_grade_bits: 1.0_f64.to_bits(),
        add_edge_behavior: BTreeMap::new(),
    };
    (input, state)
}

#[test]
fn add_edge_return_and_throw_prefixes_are_not_atomic() {
    let (mut input, initial) = one_sibling_fixture();
    input
        .add_edge_behavior
        .insert(BeamId(2), AddEdgeBehavior::ReturnedFalse);
    let mut returned_false = initial.clone();
    let transaction =
        apply_independent(&input, &mut returned_false).expect("ignored false addEdge result");
    assert_eq!(transaction.terminal, Terminal::ReadyBeforeHeadRelationLoop);
    assert!(returned_false.edges.is_empty());
    assert_eq!(
        transaction.events,
        vec![SerialEvent::LinkerFlagAssigned {
            sibling_ordinal: 0,
            selected_linker: LinkerId(1),
            cell: LinkerCellId(1),
            ordered_observers: vec![LinkerId(1), LinkerId(2)],
            before: false,
            after: true,
        }]
    );

    input
        .add_edge_behavior
        .insert(BeamId(2), AddEdgeBehavior::ThrowBeforeInsertion);
    let mut before_insert = initial.clone();
    let transaction = apply_independent(&input, &mut before_insert).expect("throw envelope");
    assert_eq!(
        transaction.terminal,
        Terminal::Threw(ThrowStage::AddEdgeBeforeInsertion)
    );
    assert!(transaction.events.is_empty());
    assert!(before_insert.edges.is_empty());
    assert!(!before_insert.linked_cells[&LinkerCellId(1)]);

    input
        .add_edge_behavior
        .insert(BeamId(2), AddEdgeBehavior::ThrowDuringCallback);
    let mut callback_throw = initial;
    let transaction = apply_independent(&input, &mut callback_throw).expect("callback envelope");
    assert_eq!(
        transaction.terminal,
        Terminal::Threw(ThrowStage::RelationCallbackAfterInsertion)
    );
    assert_eq!(callback_throw.edges.len(), 1);
    assert_eq!(
        transaction.events,
        vec![SerialEvent::EdgeInserted {
            sibling_ordinal: 0,
            relation_identity: 1,
        }]
    );
    assert!(!callback_throw.linked_cells[&LinkerCellId(1)]);
}

#[test]
fn compact_model_fails_closed_on_live_chords_and_malformed_state() {
    let (input, initial) = one_sibling_fixture();
    let mut chord_state = initial.clone();
    chord_state.edges.push(GraphEdge {
        relation_identity: 1,
        source: Vertex::Beam(BeamId(1)),
        target: Vertex::Stem(StemId(1)),
        payload: RelationPayload::ChordStem,
    });
    chord_state.next_relation_identity = 2;
    let before = chord_state.clone();
    assert_eq!(
        apply_independent(&input, &mut chord_state),
        Err(IndependentError::ChordBearingStem)
    );
    assert_eq!(chord_state, before);

    let mut invalid_direction = input.clone();
    invalid_direction.y_dir = 0;
    let mut state = initial.clone();
    assert_eq!(
        apply_independent(&invalid_direction, &mut state),
        Err(IndependentError::InvalidDirection)
    );
    assert_eq!(state, initial);

    let mut missing_base = input.clone();
    missing_base.base_beam = BeamId(99);
    let mut state = initial.clone();
    assert_eq!(
        apply_independent(&missing_base, &mut state),
        Err(IndependentError::MissingBaseBeam)
    );

    let mut missing_stem_state = initial.clone();
    missing_stem_state
        .vertices
        .remove(&Vertex::Stem(input.stem));
    let missing_stem_before = missing_stem_state.clone();
    assert_eq!(
        apply_independent(&input, &mut missing_stem_state),
        Err(IndependentError::MissingStem)
    );
    assert_eq!(missing_stem_state, missing_stem_before);

    let mut missing_group = input.clone();
    missing_group.group_members.push(BeamId(99));
    let mut state = initial.clone();
    assert_eq!(
        apply_independent(&missing_group, &mut state),
        Err(IndependentError::MissingGroupBeam)
    );

    let mut invalid_allocator = initial.clone();
    invalid_allocator.next_relation_identity = 0;
    let invalid_allocator_before = invalid_allocator.clone();
    assert_eq!(
        apply_independent(&input, &mut invalid_allocator),
        Err(IndependentError::InvalidRelationAllocator)
    );
    assert_eq!(invalid_allocator, invalid_allocator_before);

    let mut missing_cell = initial.clone();
    missing_cell.linked_cells.clear();
    let missing_cell_before = missing_cell.clone();
    assert_eq!(
        apply_independent(&input, &mut missing_cell),
        Err(IndependentError::MissingLinkerCell)
    );
    assert_eq!(missing_cell, missing_cell_before);
}
