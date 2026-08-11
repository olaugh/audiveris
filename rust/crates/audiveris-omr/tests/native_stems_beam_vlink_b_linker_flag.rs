//! Independent gate for `BeamLinker.VLinker.link`'s single
//! `getBLinker().setLinked(true)` boundary.
//!
//! The Java mutation is one assignment to the enclosing `BLinker.linked`
//! cell.  Every child V-linker observes that same cell.  This gate keeps the
//! model deliberately separate from the production implementation so the
//! frozen Java rows can later be compared with two independent projections.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use audiveris_omr::{
    native_stems_beam_stumps::NativeStemsBeamSource,
    native_stems_beam_vlink_b_linker_flag::{
        NativeStemsBeamVLinkBLinkerFlagOperation, NativeStemsBeamVLinkBLinkerFlagOutcome,
        NativeStemsBeamVLinkBLinkerFlagState, NativeStemsBeamVLinkBLinkerFlagTransaction,
        apply_native_stems_beam_vlink_b_linker_flag_transaction,
    },
    native_stems_beam_vlink_base_apply::NativeStemsBeamVLinkBaseApplyTransaction,
    native_stems_beam_vlinkers::{NativeStemsBeamBLinkerRef, NativeStemsBeamVLinkerRef},
    stems_step::NativeStemVerticalSide,
};

const FIXTURE_SCHEMA: &str = "# schema: stems-beam-vlink-b-linker-flag-v1";
const CORPUS_PAGES: [&str; 8] = [
    "chula",
    "allegretto",
    "batuque",
    "carmen",
    "cucaracha",
    "hove",
    "zizi",
    "BachInvention5",
];
const FLAG_MANIFEST_SCHEMA: &str = "# schema: stems-beam-vlink-b-linker-flag-manifest-v1";
const FLAG_MANIFEST_PATH: &str = "rust/oracle/stems-beam-vlink-b-linker-flag-manifest.txt";
const FLAG_MANIFEST_SHA256: &str =
    "c7032ac4871188ef0cf48ac63d99996e78a0e163bf1470d3be84c5e9b10d1d92";
const FLAG_MANIFEST_BODY_SHA256: &str =
    "3f332e7751d5de73e296294ccc6882ff6a578d0328b8c0d717c96666ffbb3e4d";
const FLAG_CORPUS_SHA256: &str = "6125665f38d894f6b05a24651f56f0a38c01e2acc2a7d18167a4175d5ae81c34";
const FLAG_CORPUS_LINES: usize = 4_562;
const FLAG_CORPUS_BYTES: usize = 2_535_981;
const FLAG_SPLIT_LINES: usize = 4_634;
const FLAG_SPLIT_BYTES: usize = 2_590_657;
const MANIFEST_ENTRY_FIELDS: &[&str] = &[
    "ordinal",
    "page",
    "fixture",
    "rowCounts",
    "systems",
    "realTransactions",
    "syntheticCases",
    "totalTransactions",
    "realFalseToTrue",
    "realIdempotent",
    "syntheticFalseToTrue",
    "syntheticIdempotent",
    "realArenaEntries",
    "frozenEntries",
    "dynamicAnchors",
    "isolatedExactClassCells",
    "applyReturnFalseCases",
    "pageInputSha256",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "createStemFixtureSha256",
    "reuseCheckFixtureSha256",
    "baseApplyFixtureSha256",
    "baseApplyManifestSha256",
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
    "syntheticCellConstruction",
    "syntheticEvidenceScope",
    "syntheticGeometryRead",
    "predecessorSwapNegative",
    "baseEvidenceMeaning",
    "dynamicAllBArenaGuardOnly",
    "stopBeforeSiblingBeamLinks",
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
    "sheetStubSourceSha256",
    "bookSourceSha256",
    "horizontalSideSourceSha256",
    "verticalSideSourceSha256",
    "gradleSourceSha256",
    "jgraphtCoreVersion",
    "jgraphtCoreJarSha256",
    "baseApplyManifestSha256",
    "corpusBodySha256",
    "corpusBodyLines",
    "corpusBodyBytes",
    "corpusRowCounts",
    "corpusReconstruction",
    "sharedHeaderLines",
    "semanticRows",
    "splitFixtureLines",
    "splitFixtureBytes",
    "realSystems",
    "realTransactions",
    "realFalseToTrue",
    "realIdempotent",
    "realArenaEntries",
    "frozenEntries",
    "dynamicAnchors",
    "syntheticBlocks",
    "syntheticCases",
    "syntheticFalseToTrue",
    "syntheticIdempotent",
    "isolatedExactClassCells",
    "applyReturnFalseCases",
    "totalTransactions",
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
    "syntheticCellConstruction",
    "syntheticEvidenceScope",
    "syntheticGeometryRead",
    "predecessorSwapNegative",
    "baseEvidenceMeaning",
    "dynamicAllBArenaGuardOnly",
    "stopBeforeSiblingBeamLinks",
    "manifestBodySha256",
    "manifestBodyLines",
    "manifestBodyBytes",
];
const FIXTURE_HEADER: &[&str] = &[
    "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) beam VLink B-linker-flag oracle.",
    FIXTURE_SCHEMA,
    "# Frozen scheduler/expand/createStem/reuse-check/base-apply predecessors are replayed and joined exactly.",
    "# The concrete receiver is exact BeamLinker$BLinker; no subclass override is admitted.",
    "# One plain void assignment writes the parent B linked field observed by all its V children.",
    "# Head S/C linkers do not share this cell and are guard-only unchanged evidence.",
    "# Stop is immediately after setLinked(true), before the first linkSiblings read.",
    "# System 1 adds isolated exact-class false, idempotent, false-return, and two-child certificates.",
];
const SYNTHETIC_CASES: &[&str] = &[
    "InitialFalse",
    "IdempotentTrue",
    "ApplyReturnFalse",
    "TwoChildrenShared",
];
const PROBE_SOURCE: &str = "rust/oracle/java/StemsBeamVLinkBLinkerFlagProbe.java";
const RUNNER_SOURCE: &str = "rust/oracle/java/run-stems-beam-vlink-b-linker-flag.sh";
const BASE_APPLY_MANIFEST: &str = "rust/oracle/stems-beam-vlink-base-apply-manifest.txt";
const BASE_APPLY_GATE_SOURCE: &str =
    "rust/crates/audiveris-omr/tests/native_stems_beam_vlink_base_apply.rs";
const BASE_APPLY_GATE_SOURCE_SHA256: &str =
    "73e35761e2b0df55d4609448efd2a2535d674ff720148c7d90f220b8994b5353";
const JGRAPHT_CORE_VERSION: &str = "1.5.2";
const JGRAPHT_CORE_JAR: &str = "jgrapht-core-1.5.2.jar";
const JGRAPHT_CORE_JAR_SHA256: &str =
    "dfa596e9f0d0838f1b5e81dd0cd60e3a76c2c290ac25a0a029ffde58cf5e4c14";
const JDK_RELEASE_SHA256: &str = "0cac9d5b21cd5a251ecb5064526a1d2b38d80ddfffc0531d0d5b765ac0117e08";
const JAVA_EXECUTABLE_SHA256: &str =
    "0a1eea36b7899323b32caab6f1d0e416ad7208792b076391278062efab4b15d8";
const JAVA_JPEG_LIBRARY_SHA256: &str =
    "ab2e9b8a49e053ff9c7c2ca11fbbf7fdf82e42ece1f2f768fbef0f02130dc958";
const JAVA_MODULES_SHA256: &str =
    "0c2bf8eb97ddd398588f2c7038e0cc4e3708ec1e5c683d0554c8a49549966a5e";
const JAVA_VM_LIBRARY_SHA256: &str =
    "260f5d38172bd53ee206baddd9b43c06e040ea6a6203440824e658f94556203b";
const JAVA_AWT_LIBRARY_SHA256: &str =
    "701305c6c4d3967a46831e1ee59780105e644973ad904497e220d12413848312";
const JAVA_AWT_LWAWT_LIBRARY_SHA256: &str =
    "5302653069a3157d4f21dea234b1ff17757930647736d78958fcb999572c3e46";
const BEAM_LINKER_CLASS_SHA256: &str =
    "cce4ad51e766cc6a7ed0e06234c41abedbdad3c83ab1123a17da9601c73640b7";
const B_LINKER_CLASS_SHA256: &str =
    "f934e6d4f96258ec19ac5ae59ec4425a502a620653895239b4789f3e3f7798a0";
const V_LINKER_CLASS_SHA256: &str =
    "ad68fa1774655ba9daeea85575dd9cd19da1323029e1261c0b54a9b92f8562d0";
const STEM_LINKER_CLASS_SHA256: &str =
    "74244246274f00ad582c122b3154a60d66527a3db36897abe435ec28f31c1438";
const EFFECTIVE_CLASSPATH_SHA256: &str =
    "fd4e52c2275675a53459dff2b2e2d89636f3c5fb6ab5a1f7be65f74157663fb3";
const JGRAPHT_CORE_GRADLE_DECLARATION: &str = "\"org.jgrapht:jgrapht-core:1.5.2\",";
const SOURCE_PATHS: &[(&str, &str)] = &[
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
    ("gradleSourceSha256", "app/build.gradle"),
];

const PAGE_FIELDS: &[&str] = &[
    "systems",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "createStemFixtureSha256",
    "reuseCheckFixtureSha256",
    "baseApplyFixtureSha256",
    "executionMode",
    "arenaOrder",
    "childOrder",
    "headless",
    "setterDispatch",
    "syntheticCellConstruction",
    "syntheticEvidenceScope",
    "syntheticGeometryRead",
];
const PREDECESSOR_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "join",
    "baseCase",
    "predecessorTerminal",
    "baseFrontierRowSha256",
    "baseResultRowSha256",
    "baseGuardRowSha256",
    "baseSummaryRowSha256",
    "baseEvidenceSha256",
    "applyReturn",
    "supportGrade",
    "stemAlias",
    "stemInterId",
    "bAlias",
    "vAlias",
];
const BASELINE_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "arenaScope",
    "arenaEntries",
    "frozenEntries",
    "dynamicAnchors",
    "noChild",
    "child1",
    "child2",
    "topologyHash",
    "stateHashBefore",
    "linkedCountBefore",
    "linkedAliasesBefore",
    "linkedSetHashBefore",
    "closedCountBefore",
    "closedAliasesBefore",
    "targetMatches",
    "targetFrozenMatch",
];
const B_ENTRY_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "arenaOrdinal",
    "constructorOrdinal",
    "beamSig",
    "bOrdinal",
    "bJavaId",
    "bAlias",
    "origin",
    "runtimeClass",
    "hSide",
    "anchor",
    "stump",
    "ref",
    "childCount",
    "childAliases",
    "linkedBefore",
    "linkedAfter",
    "closedBefore",
    "closedAfter",
    "target",
    "action",
];
const TARGET_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "bAlias",
    "vAlias",
    "bRefBeamSig",
    "bRefJavaId",
    "vSide",
    "receiverRuntimeClass",
    "setterDeclaringClass",
    "getterDeclaringClass",
    "getterResolvedAlias",
    "sameIdentity",
    "sharedCell",
    "sLinkerRead",
    "orderedChildAliases",
    "linkedBefore",
    "linkedAfter",
    "closedBefore",
    "closedAfter",
];
const OBSERVATION_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "observationOrdinal",
    "role",
    "alias",
    "vSide",
    "current",
    "sharedCell",
    "sameCell",
    "ownsLinkedField",
    "linkedBefore",
    "linkedAfter",
    "closedBefore",
    "closedAfter",
    "readMethod",
];
const WRITE_TRACE_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "getter",
    "getterCompleted",
    "receiverAlias",
    "setter",
    "requested",
    "before",
    "after",
    "assignmentAttempted",
    "assignmentCompleted",
    "writeCount",
    "valueChangeCount",
    "transition",
    "return",
    "setterBodyValidationReads",
    "setterBodyCallbacks",
    "setterBodyListeners",
    "setterBodyAllocations",
    "plainNonVolatile",
    "throwStage",
    "exception",
];
const RESULT_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "terminal",
    "applyReturn",
    "supportGrade",
    "supportGradeRead",
    "stemAlias",
    "stemInterId",
    "bAlias",
    "linked",
    "writeCount",
    "valueChangeCount",
    "stateHashAfter",
    "linkedCountAfter",
    "linkedAliasesAfter",
    "linkedSetHashAfter",
    "linkSiblingsCalled",
];
const DELTA_GUARD_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "guardDomain",
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
    "sigRelationHashBefore",
    "sigRelationHashAfter",
    "systemStemsHashBefore",
    "systemStemsHashAfter",
    "lineHashBefore",
    "lineHashAfter",
    "arenaTopologyHashBefore",
    "arenaTopologyHashAfter",
    "allBIdentityOrderUnchanged",
    "allBClosedUnchanged",
    "unrelatedBLinkedUnchanged",
    "headLinkerFlagsUnchanged",
    "noCellOtherThanSelectedChanged",
    "allowedMutation",
    "supportGradeRead",
    "siblingDiscoveryRead",
    "stopBeforeSiblingBeamLinks",
    "stopBeforeHeadRelationLoop",
];
const SUMMARY_FIELDS: &[&str] = &[
    "system",
    "plan",
    "scope",
    "case",
    "transition",
    "writeCount",
    "valueChangeCount",
    "observations",
    "arenaEntries",
    "dynamicAnchors",
    "applyReturn",
    "terminal",
];
const PAGE_SUMMARY_FIELDS: &[&str] = &[
    "systems",
    "realTransactions",
    "syntheticCases",
    "totalTransactions",
    "realFalseToTrue",
    "realIdempotent",
    "syntheticFalseToTrue",
    "syntheticIdempotent",
    "realArenaEntries",
    "frozenEntries",
    "dynamicAnchors",
    "isolatedExactClassCells",
    "applyReturnFalseCases",
    "stopBeforeSiblingBeamLinks",
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
    "sheetStubSourceSha256",
    "bookSourceSha256",
    "horizontalSideSourceSha256",
    "verticalSideSourceSha256",
    "gradleSourceSha256",
    "jgraphtCoreVersion",
    "jgraphtCoreJarSha256",
    "schedulerFixtureSha256",
    "expandFixtureSha256",
    "createStemFixtureSha256",
    "reuseCheckFixtureSha256",
    "baseApplyFixtureSha256",
    "baseApplyManifestSha256",
    "predecessorSwapNegative",
    "baseEvidenceMeaning",
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
    "dynamicAllBArenaGuardOnly",
    "stopBeforeSiblingBeamLinks",
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum BeamSource {
    Raw(usize),
    Hook(usize),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct BLinkerRef {
    beam: BeamSource,
    /// Java's one-based `allBLinkers` insertion ID.
    id: usize,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum VerticalSide {
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct VLinkerRef {
    b_linker: BLinkerRef,
    side: VerticalSide,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BLinkerArenaEntry {
    reference: BLinkerRef,
    /// Dynamic anchors have no V-linker child and can never be this seam's
    /// receiver, but they remain part of Java's exhaustive live arena.
    is_anchor: bool,
    /// Exact Java `EnumMap<VerticalSide,VLinker>.values()` order.
    child_v_linkers: Vec<VLinkerRef>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CommitKey {
    system_id: usize,
    invocation_ordinal: usize,
    plan_ordinal: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndependentFlagState {
    system_id: usize,
    linked_b_linkers: BTreeSet<BLinkerRef>,
    committed: Option<CommitKey>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndependentWrite {
    b_linker: BLinkerRef,
    triggering_v_linker: VLinkerRef,
    shared_v_linkers: Vec<VLinkerRef>,
    before: bool,
    after: bool,
    write_count: usize,
    value_change_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct IndependentTransaction {
    key: CommitKey,
    apply_returned: bool,
    continuation_support_grade: f64,
    write: IndependentWrite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IndependentError {
    WrongSystem,
    InvalidArena,
    InvalidLinkedSet,
    MissingTrigger,
    AlreadyCommitted,
}

fn validate_arena(
    system_id: usize,
    arena: &[BLinkerArenaEntry],
    state: &IndependentFlagState,
) -> Result<(), IndependentError> {
    if system_id == 0 || state.system_id != system_id {
        return Err(IndependentError::WrongSystem);
    }

    let mut references = BTreeSet::new();
    let mut next_id_by_beam = BTreeMap::<BeamSource, usize>::new();
    for entry in arena {
        if entry.reference.id == 0 || !references.insert(entry.reference) {
            return Err(IndependentError::InvalidArena);
        }
        let next_id = next_id_by_beam.entry(entry.reference.beam).or_insert(1);
        if entry.reference.id != *next_id {
            return Err(IndependentError::InvalidArena);
        }
        *next_id += 1;

        if entry.child_v_linkers.len() > 2 || (entry.is_anchor && !entry.child_v_linkers.is_empty())
        {
            return Err(IndependentError::InvalidArena);
        }
        let expected_sides: Vec<_> = [VerticalSide::Top, VerticalSide::Bottom]
            .into_iter()
            .filter(|side| {
                entry
                    .child_v_linkers
                    .iter()
                    .any(|child| child.side == *side)
            })
            .collect();
        if entry
            .child_v_linkers
            .iter()
            .any(|child| child.b_linker != entry.reference)
            || entry
                .child_v_linkers
                .iter()
                .map(|child| child.side)
                .collect::<Vec<_>>()
                != expected_sides
        {
            return Err(IndependentError::InvalidArena);
        }
    }
    if !state
        .linked_b_linkers
        .iter()
        .all(|reference| references.contains(reference))
    {
        return Err(IndependentError::InvalidLinkedSet);
    }
    Ok(())
}

fn apply_independent(
    arena: &[BLinkerArenaEntry],
    state: &mut IndependentFlagState,
    key: CommitKey,
    triggering_v_linker: VLinkerRef,
    apply_returned: bool,
    continuation_support_grade: f64,
) -> Result<IndependentTransaction, IndependentError> {
    validate_arena(key.system_id, arena, state)?;
    if state.committed.is_some() {
        return Err(IndependentError::AlreadyCommitted);
    }
    let entry = arena
        .iter()
        .find(|entry| entry.reference == triggering_v_linker.b_linker)
        .filter(|entry| entry.child_v_linkers.contains(&triggering_v_linker))
        .ok_or(IndependentError::MissingTrigger)?;

    // Java's setter has no callback, allocation, graph access, conditional,
    // or return value.  The assignment occurs even when the preceding
    // `Link.applyTo` returned false.
    let before = state.linked_b_linkers.contains(&entry.reference);
    state.linked_b_linkers.insert(entry.reference);
    state.committed = Some(key);
    Ok(IndependentTransaction {
        key,
        apply_returned,
        continuation_support_grade,
        write: IndependentWrite {
            b_linker: entry.reference,
            triggering_v_linker,
            shared_v_linkers: entry.child_v_linkers.clone(),
            before,
            after: true,
            write_count: 1,
            value_change_count: usize::from(!before),
        },
    })
}

fn native_b_linker(reference: BLinkerRef) -> NativeStemsBeamBLinkerRef {
    NativeStemsBeamBLinkerRef {
        beam: match reference.beam {
            BeamSource::Raw(ordinal) => NativeStemsBeamSource::RawBeam(ordinal),
            BeamSource::Hook(ordinal) => NativeStemsBeamSource::Hook(ordinal),
        },
        id: reference.id,
    }
}

fn native_v_linker(reference: VLinkerRef) -> NativeStemsBeamVLinkerRef {
    NativeStemsBeamVLinkerRef {
        b_linker: native_b_linker(reference.b_linker),
        side: match reference.side {
            VerticalSide::Top => NativeStemVerticalSide::Top,
            VerticalSide::Bottom => NativeStemVerticalSide::Bottom,
        },
    }
}

fn assert_public_write_shape(
    public: &NativeStemsBeamVLinkBLinkerFlagOperation,
    independent: &IndependentWrite,
) {
    let NativeStemsBeamVLinkBLinkerFlagOperation::BLinkerLinkedAssigned {
        target_b_linker,
        triggering_v_linker,
        ordered_observer_v_linkers,
        before,
        after,
    } = public;
    assert_eq!(*target_b_linker, native_b_linker(independent.b_linker));
    assert_eq!(
        *triggering_v_linker,
        native_v_linker(independent.triggering_v_linker)
    );
    assert_eq!(
        *ordered_observer_v_linkers,
        independent
            .shared_v_linkers
            .iter()
            .copied()
            .map(native_v_linker)
            .collect::<Vec<_>>()
    );
    assert_eq!((*before, *after), (independent.before, independent.after));
}

struct PublicProjectionExpected<'a> {
    target: NativeStemsBeamBLinkerRef,
    triggering: NativeStemsBeamVLinkerRef,
    ordered_observers: &'a [NativeStemsBeamVLinkerRef],
}

fn assert_public_transaction_matches_replay(
    transaction: &NativeStemsBeamVLinkBLinkerFlagTransaction,
    state_before: &NativeStemsBeamVLinkBLinkerFlagState,
    state_after: &NativeStemsBeamVLinkBLinkerFlagState,
    base_apply: &NativeStemsBeamVLinkBaseApplyTransaction,
    replay: &OracleFlagReplay,
    expected: PublicProjectionExpected<'_>,
) -> Result<(), String> {
    let PublicProjectionExpected {
        target,
        triggering,
        ordered_observers,
    } = expected;
    let operation = match transaction.operations.as_slice() {
        [operation] => operation,
        _ => return Err("public boundary-15 operation count differs".into()),
    };
    let NativeStemsBeamVLinkBLinkerFlagOperation::BLinkerLinkedAssigned {
        target_b_linker,
        triggering_v_linker,
        ordered_observer_v_linkers,
        before,
        after,
    } = operation;
    let (outcome_stem, outcome_grade) = match transaction.outcome {
        NativeStemsBeamVLinkBLinkerFlagOutcome::ReadyBeforeSiblingBeamLinks {
            stem_identity,
            continuation_support_grade,
        } => (stem_identity, continuation_support_grade),
    };
    if transaction.key != base_apply.key
        || transaction.key.system_id != replay.system_id
        || transaction.key.plan.plan_ordinal != replay.plan_ordinal
        || transaction.target_b_linker != target
        || transaction.triggering_v_linker != triggering
        || transaction.ordered_observer_v_linkers != ordered_observers
        || *target_b_linker != target
        || *triggering_v_linker != triggering
        || ordered_observer_v_linkers != ordered_observers
        || (*before, *after) != (replay.linked_before, replay.linked_after)
        || transaction.linked_before != replay.linked_before
        || transaction.linked_after != replay.linked_after
        || transaction.linked_write_count != replay.write_count
        || transaction.linked_value_change_count != replay.value_change_count
        || transaction.b_linker_flag_mutation_count != replay.value_change_count
        || transaction.s_linker_flag_mutation_count != 0
        || transaction.persistent_id_mutation_count != 0
        || transaction.inter_index_mutation_count != 0
        || transaction.sig_vertex_mutation_count != 0
        || transaction.sig_relation_mutation_count != 0
        || transaction.stem_mutation_count != 0
        || transaction.beam_mutation_count != 0
        || transaction.sheet_edit_mutation_count != 0
        || transaction.sibling_link_mutation_count != 0
        || transaction.head_link_mutation_count != 0
        || transaction.apply_returned != replay.apply_returned
        || transaction.apply_returned != base_apply.apply_returned
        || transaction.continuation_support_grade.to_bits()
            != replay.continuation_support_grade_bits
        || transaction.continuation_support_grade.to_bits()
            != base_apply.continuation_support_grade.to_bits()
        || transaction.stem_after != base_apply.stem_after
        || transaction.stem_after.inter_id != Some(replay.stem_inter_id)
        || outcome_stem != base_apply.stem_after.stem_identity
        || outcome_grade.to_bits() != replay.continuation_support_grade_bits
        || transaction.state_after.as_ref() != state_after
        || state_after.system_id != replay.system_id
        || state_after.base_apply_state_before != state_before.base_apply_state_before
        || state_after.target_b_linker != target
        || state_after.linked != replay.linked_after
        || state_after.committed != Some(base_apply.key)
        || state_before.system_id != replay.system_id
        || state_before.target_b_linker != target
        || state_before.linked != replay.linked_before
        || state_before.committed.is_some()
    {
        return Err("public boundary-15 transaction differs from independent/Java replay".into());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RowKind {
    Page,
    Predecessor,
    Baseline,
    BEntry,
    Target,
    Observation,
    WriteTrace,
    Result,
    DeltaGuard,
    Summary,
    PageSummary,
    CorpusSummary,
}

impl RowKind {
    fn parse(label: &str) -> Option<Self> {
        let suffix = label.strip_prefix("stemsbeamvlinkblinkerflag")?;
        Some(match suffix {
            "page" => Self::Page,
            "predecessor" => Self::Predecessor,
            "baseline" => Self::Baseline,
            "bentry" => Self::BEntry,
            "target" => Self::Target,
            "observation" => Self::Observation,
            "writetrace" => Self::WriteTrace,
            "result" => Self::Result,
            "deltaguard" => Self::DeltaGuard,
            "summary" => Self::Summary,
            "pagesummary" => Self::PageSummary,
            "corpussummary" => Self::CorpusSummary,
            _ => return None,
        })
    }

    fn expected_fields(self) -> Option<&'static [&'static str]> {
        match self {
            Self::Page => Some(PAGE_FIELDS),
            Self::Predecessor => Some(PREDECESSOR_FIELDS),
            Self::Baseline => Some(BASELINE_FIELDS),
            Self::BEntry => Some(B_ENTRY_FIELDS),
            Self::Target => Some(TARGET_FIELDS),
            Self::Observation => Some(OBSERVATION_FIELDS),
            Self::WriteTrace => Some(WRITE_TRACE_FIELDS),
            Self::Result => Some(RESULT_FIELDS),
            Self::DeltaGuard => Some(DELTA_GUARD_FIELDS),
            Self::Summary => Some(SUMMARY_FIELDS),
            Self::PageSummary => Some(PAGE_SUMMARY_FIELDS),
            Self::CorpusSummary => Some(CORPUS_SUMMARY_FIELDS),
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
        let tokens: Vec<_> = line.split_ascii_whitespace().collect();
        if tokens.is_empty() {
            return Err("row is empty".into());
        }
        let kind = RowKind::parse(tokens[0]).ok_or_else(|| "unknown row label".to_string())?;
        let (page, field_start) = if kind == RowKind::CorpusSummary {
            (String::new(), 1)
        } else {
            if tokens.len() < 2 {
                return Err("row must contain a page token".into());
            }
            (tokens[1].to_string(), 2)
        };
        if (tokens.len() - field_start) % 2 != 0 {
            return Err("row must contain ordered key/value pairs".into());
        }
        let mut names = BTreeSet::new();
        let mut fields = Vec::with_capacity((tokens.len() - field_start) / 2);
        for pair in tokens[field_start..].chunks_exact(2) {
            if !names.insert(pair[0]) {
                return Err(format!("duplicate field {}", pair[0]));
            }
            fields.push((pair[0].to_string(), pair[1].to_string()));
        }
        if let Some(expected) = kind.expected_fields() {
            let actual = fields
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>();
            if actual != expected {
                return Err(format!(
                    "wrong field order for {kind:?}: expected {expected:?}, got {actual:?}"
                ));
            }
        }
        Ok(Self { kind, page, fields })
    }

    fn value(&self, name: &str) -> Result<&str, String> {
        self.fields
            .iter()
            .find_map(|(field, value)| (field == name).then_some(value.as_str()))
            .ok_or_else(|| format!("missing field {name}"))
    }

    fn usize(&self, name: &str) -> Result<usize, String> {
        self.value(name)?
            .parse()
            .map_err(|_| format!("invalid usize field {name}"))
    }

    fn bool(&self, name: &str) -> Result<bool, String> {
        match self.value(name)? {
            "true" => Ok(true),
            "false" => Ok(false),
            value => Err(format!("invalid boolean field {name}: {value}")),
        }
    }

    fn common_key(&self) -> Result<(usize, usize, &str, &str), String> {
        Ok((
            self.usize("system")?,
            self.usize("plan")?,
            self.value("scope")?,
            self.value("case")?,
        ))
    }
}

fn parse_scaffold_fixture(text: &str) -> Result<Vec<StrictRow>, String> {
    if !text.ends_with('\n') {
        return Err("fixture must end with one newline".into());
    }
    let lines = text.lines().collect::<Vec<_>>();
    if lines.len() <= FIXTURE_HEADER.len() || lines[..FIXTURE_HEADER.len()] != *FIXTURE_HEADER {
        return Err("wrong or missing exact fixture header".into());
    }
    let mut rows = Vec::new();
    for line in &lines[FIXTURE_HEADER.len()..] {
        if line.is_empty() || line.starts_with('#') {
            return Err("empty/comment/trailing unauthenticated data is forbidden".into());
        }
        rows.push(StrictRow::parse(line)?);
    }
    if rows.len() < 3
        || rows.first().map(|row| row.kind) != Some(RowKind::Page)
        || rows.get(rows.len() - 2).map(|row| row.kind) != Some(RowKind::PageSummary)
        || rows.last().map(|row| row.kind) != Some(RowKind::CorpusSummary)
        || rows[..rows.len() - 2]
            .iter()
            .any(|row| matches!(row.kind, RowKind::PageSummary | RowKind::CorpusSummary))
    {
        return Err("fixture page/trailer row order differs".into());
    }
    let page = &rows[0].page;
    if rows[..rows.len() - 1].iter().any(|row| &row.page != page)
        || rows.last().unwrap().value("pageRefs")? != page
    {
        return Err("fixture page identity differs across rows".into());
    }
    Ok(rows)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ManifestRow {
    fields: Vec<(String, String)>,
}

impl ManifestRow {
    fn parse(line: &str, label: &str, expected: &[&str]) -> Result<Self, String> {
        let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
        if tokens.first().copied() != Some(label) || (tokens.len() - 1) % 2 != 0 {
            return Err(format!("invalid manifest row {label}"));
        }
        let fields = tokens[1..]
            .chunks_exact(2)
            .map(|pair| (pair[0].to_string(), pair[1].to_string()))
            .collect::<Vec<_>>();
        let actual = fields
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>();
        if actual != expected || actual.iter().collect::<BTreeSet<_>>().len() != actual.len() {
            return Err(format!("manifest {label} field order differs"));
        }
        Ok(Self { fields })
    }

    fn value(&self, name: &str) -> Result<&str, String> {
        self.fields
            .iter()
            .find_map(|(field, value)| (field == name).then_some(value.as_str()))
            .ok_or_else(|| format!("missing manifest field {name}"))
    }

    fn usize(&self, name: &str) -> Result<usize, String> {
        self.value(name)?
            .parse()
            .map_err(|_| format!("invalid manifest usize {name}"))
    }

    fn bool(&self, name: &str) -> Result<bool, String> {
        match self.value(name)? {
            "true" => Ok(true),
            "false" => Ok(false),
            value => Err(format!("invalid manifest boolean {name}: {value}")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StrictFlagManifest {
    entries: Vec<ManifestRow>,
    summary: ManifestRow,
    body_len: usize,
}

impl StrictFlagManifest {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        if !bytes.ends_with(b"\n") {
            return Err("manifest must have a terminal newline".into());
        }
        let text = std::str::from_utf8(bytes).map_err(|_| "manifest is not UTF-8")?;
        let lines = text.lines().collect::<Vec<_>>();
        if lines.len() != CORPUS_PAGES.len() + 2 || lines[0] != FLAG_MANIFEST_SCHEMA {
            return Err("manifest schema/row count differs".into());
        }
        let entries = lines[1..=CORPUS_PAGES.len()]
            .iter()
            .map(|line| {
                ManifestRow::parse(
                    line,
                    "stemsbeamvlinkblinkerflagmanifestentry",
                    MANIFEST_ENTRY_FIELDS,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (ordinal, (entry, page)) in entries.iter().zip(CORPUS_PAGES).enumerate() {
            if entry.usize("ordinal")? != ordinal || entry.value("page")? != page {
                return Err("manifest page order differs".into());
            }
        }
        let summary = ManifestRow::parse(
            lines.last().unwrap(),
            "stemsbeamvlinkblinkerflagmanifestsummary",
            MANIFEST_SUMMARY_FIELDS,
        )?;
        let summary_marker = "stemsbeamvlinkblinkerflagmanifestsummary ";
        let body_len = text
            .find(summary_marker)
            .filter(|offset| {
                text[*offset + summary_marker.len()..]
                    .find(summary_marker)
                    .is_none()
            })
            .ok_or_else(|| "manifest summary is not unique/terminal".to_string())?;
        Ok(Self {
            entries,
            summary,
            body_len,
        })
    }
}

fn one_row(rows: &[StrictRow], kind: RowKind) -> Result<&StrictRow, String> {
    let matches = rows
        .iter()
        .filter(|row| row.kind == kind)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [row] => Ok(row),
        _ => Err(format!(
            "expected one {kind:?} row, found {}",
            matches.len()
        )),
    }
}

fn transaction_blocks(rows: &[StrictRow]) -> Result<Vec<&[StrictRow]>, String> {
    let semantic = &rows[1..rows.len() - 2];
    let mut blocks = Vec::new();
    let mut start = 0;
    while start < semantic.len() {
        if semantic[start].kind != RowKind::Predecessor {
            return Err("transaction block does not begin with predecessor".into());
        }
        let end = semantic[start..]
            .iter()
            .position(|row| row.kind == RowKind::Summary)
            .map(|offset| start + offset + 1)
            .ok_or_else(|| "transaction block lacks summary".to_string())?;
        let block = &semantic[start..end];
        if block[1..block.len() - 1]
            .iter()
            .any(|row| matches!(row.kind, RowKind::Predecessor | RowKind::Summary))
        {
            return Err("nested predecessor/summary in transaction block".into());
        }
        let key = block[0].common_key()?;
        if block.iter().any(|row| row.common_key().ok() != Some(key)) {
            return Err("transaction row key differs within block".into());
        }
        blocks.push(block);
        start = end;
    }
    Ok(blocks)
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
        return Err("empty bracketed list must use '-'".into());
    }
    let values = body.split(',').collect::<Vec<_>>();
    if values.iter().any(|value| value.is_empty()) {
        return Err(format!("invalid list token {value}"));
    }
    Ok(values)
}

fn list_token(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        format!("[{}]", values.join(","))
    }
}

fn parse_hex_double_bits(value: &str) -> Result<u64, String> {
    let (_, bits) = value
        .rsplit_once('/')
        .ok_or_else(|| format!("invalid exact-double token {value}"))?;
    if bits.len() != 16 || !bits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("invalid exact-double bits {value}"));
    }
    u64::from_str_radix(bits, 16).map_err(|_| format!("invalid exact-double bits {value}"))
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_rows<T: AsRef<str>>(rows: &[T]) -> String {
    let mut bytes = Vec::new();
    for row in rows {
        bytes.extend_from_slice(row.as_ref().as_bytes());
        bytes.push(b'\n');
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct OracleArenaEntry {
    arena_ordinal: usize,
    constructor_ordinal: usize,
    beam_sig: String,
    b_ordinal: usize,
    java_id: usize,
    alias: String,
    origin: String,
    runtime_class: String,
    h_side: String,
    anchor: bool,
    stump: String,
    reference_point: String,
    child_aliases: Vec<String>,
    linked_before: bool,
    linked_after: bool,
    closed_before: bool,
    closed_after: bool,
    target: bool,
}

impl OracleArenaEntry {
    fn parse(row: &StrictRow) -> Result<Self, String> {
        Ok(Self {
            arena_ordinal: row.usize("arenaOrdinal")?,
            constructor_ordinal: row.usize("constructorOrdinal")?,
            beam_sig: row.value("beamSig")?.to_string(),
            b_ordinal: row.usize("bOrdinal")?,
            java_id: row.usize("bJavaId")?,
            alias: row.value("bAlias")?.to_string(),
            origin: row.value("origin")?.to_string(),
            runtime_class: row.value("runtimeClass")?.to_string(),
            h_side: row.value("hSide")?.to_string(),
            anchor: row.bool("anchor")?,
            stump: row.value("stump")?.to_string(),
            reference_point: row.value("ref")?.to_string(),
            child_aliases: parse_list(row.value("childAliases")?)?
                .into_iter()
                .map(str::to_string)
                .collect(),
            linked_before: row.bool("linkedBefore")?,
            linked_after: row.bool("linkedAfter")?,
            closed_before: row.bool("closedBefore")?,
            closed_after: row.bool("closedAfter")?,
            target: row.bool("target")?,
        })
    }

    fn topology_token(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:h={}:anchor={}:stump={}:ref={}:children={}:target={}",
            self.arena_ordinal,
            self.constructor_ordinal,
            self.beam_sig,
            self.b_ordinal,
            self.java_id,
            self.alias,
            self.origin,
            self.runtime_class,
            self.h_side,
            self.anchor,
            self.stump,
            self.reference_point,
            list_token(&self.child_aliases),
            self.target,
        )
    }

    fn state_token(&self, after: bool) -> String {
        format!(
            "{}:linked={}:closed={}",
            self.topology_token(),
            if after {
                self.linked_after
            } else {
                self.linked_before
            },
            if after {
                self.closed_after
            } else {
                self.closed_before
            }
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
struct OracleFlagReplay {
    system_id: usize,
    plan_ordinal: usize,
    scope: String,
    case_name: String,
    target_alias: String,
    trigger_alias: String,
    ordered_observer_aliases: Vec<String>,
    linked_before: bool,
    linked_after: bool,
    write_count: usize,
    value_change_count: usize,
    apply_returned: bool,
    continuation_support_grade_bits: u64,
    stem_alias: String,
    stem_inter_id: i32,
    arena_entries: usize,
    frozen_entries: usize,
    dynamic_anchors: usize,
}

fn validate_and_replay_block(rows: &[StrictRow]) -> Result<OracleFlagReplay, String> {
    let predecessor = one_row(rows, RowKind::Predecessor)?;
    let baseline = one_row(rows, RowKind::Baseline)?;
    let target = one_row(rows, RowKind::Target)?;
    let write = one_row(rows, RowKind::WriteTrace)?;
    let result = one_row(rows, RowKind::Result)?;
    let guard = one_row(rows, RowKind::DeltaGuard)?;
    let summary = one_row(rows, RowKind::Summary)?;
    let key = predecessor.common_key()?;
    let (system_id, plan_ordinal, scope, case_name) = key;
    let b_rows = rows
        .iter()
        .filter(|row| row.kind == RowKind::BEntry)
        .collect::<Vec<_>>();
    let observation_rows = rows
        .iter()
        .filter(|row| row.kind == RowKind::Observation)
        .collect::<Vec<_>>();
    let expected_kinds = rows.iter().map(|row| row.kind).collect::<Vec<_>>();
    let b_start = 2;
    let target_index = b_start + b_rows.len();
    let observation_end = target_index + 1 + observation_rows.len();
    if expected_kinds.first() != Some(&RowKind::Predecessor)
        || expected_kinds.get(1) != Some(&RowKind::Baseline)
        || expected_kinds[b_start..target_index]
            .iter()
            .any(|kind| *kind != RowKind::BEntry)
        || expected_kinds.get(target_index) != Some(&RowKind::Target)
        || expected_kinds[target_index + 1..observation_end]
            .iter()
            .any(|kind| *kind != RowKind::Observation)
        || expected_kinds.get(observation_end) != Some(&RowKind::WriteTrace)
        || expected_kinds.get(observation_end + 1) != Some(&RowKind::Result)
        || expected_kinds.get(observation_end + 2) != Some(&RowKind::DeltaGuard)
        || expected_kinds.get(observation_end + 3) != Some(&RowKind::Summary)
        || observation_end + 4 != rows.len()
    {
        return Err("transaction family order/cardinality differs".into());
    }

    let entries = b_rows
        .iter()
        .map(|row| OracleArenaEntry::parse(row))
        .collect::<Result<Vec<_>, _>>()?;
    if entries.len() != baseline.usize("arenaEntries")? || entries.is_empty() {
        return Err("arena entry count differs".into());
    }
    let mut aliases = BTreeSet::new();
    let mut previous: Option<&OracleArenaEntry> = None;
    for (ordinal, (entry, row)) in entries.iter().zip(&b_rows).enumerate() {
        if entry.arena_ordinal != ordinal
            || entry.java_id != entry.b_ordinal + 1
            || !aliases.insert(entry.alias.as_str())
            || entry.runtime_class != "org.audiveris.omr.sheet.stem.BeamLinker$BLinker"
            || entry.child_aliases.len() > 2
            || row.usize("childCount")? != entry.child_aliases.len()
            || entry.closed_before != entry.closed_after
            || row.value("action")?
                != if entry.target {
                    "TargetSharedFlagWrite"
                } else {
                    "Unchanged"
                }
        {
            return Err(format!("invalid B arena entry {ordinal}"));
        }
        let expected_children = ["TOP", "BOTTOM"]
            .into_iter()
            .filter(|side| {
                entry
                    .child_aliases
                    .iter()
                    .any(|alias| alias == &format!("{}:v:{side}", entry.alias))
            })
            .map(|side| format!("{}:v:{side}", entry.alias))
            .collect::<Vec<_>>();
        if entry.child_aliases != expected_children {
            return Err(format!(
                "B arena child order/ownership differs at {ordinal}"
            ));
        }
        match (scope, entry.origin.as_str()) {
            ("real", "FrozenVLinker")
                if !entry.anchor
                    && !entry.child_aliases.is_empty()
                    && entry.beam_sig.parse::<usize>().is_ok()
                    && entry.alias == format!("beam:{}:b:{}", entry.beam_sig, entry.b_ordinal) => {}
            ("real", "DynamicAnchor")
                if entry.anchor
                    && entry.child_aliases.is_empty()
                    && entry.beam_sig.parse::<usize>().is_ok()
                    && entry.alias == format!("beam:{}:b:{}", entry.beam_sig, entry.b_ordinal) => {}
            ("synthetic", "IsolatedSyntheticCell")
                if !entry.anchor
                    && !entry.child_aliases.is_empty()
                    && entry.beam_sig == "-"
                    && entry.constructor_ordinal == 0
                    && entry.b_ordinal == 0
                    && entry.java_id == 1
                    && entry.alias == format!("synthetic-b:{case_name}")
                    && entry.h_side == "-"
                    && entry.stump == "-"
                    && entry.reference_point
                        == "0x1.4p2/4014000000000000:0x0.0p0/0000000000000000" => {}
            _ => return Err(format!("B arena origin/domain differs at {ordinal}")),
        }
        if let Some(left) = previous {
            if entry.constructor_ordinal < left.constructor_ordinal
                || (entry.constructor_ordinal == left.constructor_ordinal
                    && (entry.beam_sig != left.beam_sig || entry.b_ordinal != left.b_ordinal + 1))
                || (entry.constructor_ordinal != left.constructor_ordinal && entry.b_ordinal != 0)
            {
                return Err(format!("B arena constructor order differs at {ordinal}"));
            }
        }
        previous = Some(entry);
    }
    let targets = entries
        .iter()
        .filter(|entry| entry.target)
        .collect::<Vec<_>>();
    let [target_entry] = targets.as_slice() else {
        return Err("target B entry is not unique".into());
    };
    let expected_linked_before = match (scope, case_name) {
        ("real", "-")
        | ("synthetic", "InitialFalse" | "ApplyReturnFalse" | "TwoChildrenShared") => false,
        ("synthetic", "IdempotentTrue") => true,
        _ => return Err("unsupported B-linker transaction scope/case".into()),
    };
    let expected_target_children = usize::from(case_name == "TwoChildrenShared") + 1;
    if target_entry.linked_before != expected_linked_before
        || (scope == "real"
            && (target_entry.origin != "FrozenVLinker"
                || target_entry.anchor
                || target_entry.b_ordinal != 0
                || target_entry.java_id != 1
                || target_entry.h_side != "LEFT"
                || target_entry.child_aliases.len() != 1))
        || (scope == "synthetic" && target_entry.child_aliases.len() != expected_target_children)
    {
        return Err("target B predecessor/topology domain differs".into());
    }
    if baseline.usize("targetMatches")? != 1
        || baseline.bool("targetFrozenMatch")? != (scope == "real")
        || target.value("bAlias")? != target_entry.alias
        || target.value("getterResolvedAlias")? != target_entry.alias
        || target.value("bRefBeamSig")? != target_entry.beam_sig
        || target.usize("bRefJavaId")? != target_entry.java_id
        || parse_list(target.value("orderedChildAliases")?)?
            != target_entry
                .child_aliases
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        || !target_entry
            .child_aliases
            .iter()
            .any(|alias| alias == target.value("vAlias").unwrap_or(""))
        || target.value("receiverRuntimeClass")?
            != "org.audiveris.omr.sheet.stem.BeamLinker$BLinker"
        || target.value("setterDeclaringClass")?
            != "org.audiveris.omr.sheet.stem.BeamLinker$BLinker"
        || target.value("getterDeclaringClass")?
            != "org.audiveris.omr.sheet.stem.BeamLinker$BLinker$VLinker"
        || !target.bool("sameIdentity")?
        || target.value("sharedCell")? != "EnclosingBLinkerLinked"
        || target.value("sLinkerRead")? != "NotRead"
        || target.bool("linkedBefore")? != target_entry.linked_before
        || target.bool("linkedAfter")? != target_entry.linked_after
        || target.bool("closedBefore")? != target_entry.closed_before
        || target.bool("closedAfter")? != target_entry.closed_after
    {
        return Err("target B/V identity or shared-cell join differs".into());
    }
    let target_side = target
        .value("vAlias")?
        .rsplit_once(":v:")
        .map(|(_, side)| side)
        .ok_or_else(|| "target V alias lacks side".to_string())?;
    if target.value("vSide")? != target_side || !matches!(target_side, "TOP" | "BOTTOM") {
        return Err("target V side differs".into());
    }

    let topology_rows = entries
        .iter()
        .map(OracleArenaEntry::topology_token)
        .collect::<Vec<_>>();
    let before_state_rows = entries
        .iter()
        .map(|entry| entry.state_token(false))
        .collect::<Vec<_>>();
    let after_state_rows = entries
        .iter()
        .map(|entry| entry.state_token(true))
        .collect::<Vec<_>>();
    let linked_before = entries
        .iter()
        .filter(|entry| entry.linked_before)
        .map(|entry| entry.alias.clone())
        .collect::<Vec<_>>();
    let linked_after = entries
        .iter()
        .filter(|entry| entry.linked_after)
        .map(|entry| entry.alias.clone())
        .collect::<Vec<_>>();
    let closed_before = entries
        .iter()
        .filter(|entry| entry.closed_before)
        .map(|entry| entry.alias.clone())
        .collect::<Vec<_>>();
    if !closed_before.is_empty() || entries.iter().any(|entry| entry.closed_after) {
        return Err("B-linker closed flags must remain false in the supported envelope".into());
    }
    let frozen = entries
        .iter()
        .filter(|entry| entry.origin == "FrozenVLinker")
        .count();
    let anchors = entries
        .iter()
        .filter(|entry| entry.origin == "DynamicAnchor")
        .count();
    let child_counts = [0, 1, 2].map(|count| {
        entries
            .iter()
            .filter(|entry| entry.child_aliases.len() == count)
            .count()
    });
    if baseline.value("arenaScope")?
        != if scope == "real" {
            "ExhaustiveLiveAllBLinkers"
        } else {
            "ExhaustiveIsolatedAllBLinkers"
        }
        || baseline.usize("frozenEntries")? != frozen
        || baseline.usize("dynamicAnchors")? != anchors
        || baseline.usize("noChild")? != child_counts[0]
        || baseline.usize("child1")? != child_counts[1]
        || baseline.usize("child2")? != child_counts[2]
        || baseline.value("topologyHash")? != sha256_rows(&topology_rows)
        || baseline.value("stateHashBefore")? != sha256_rows(&before_state_rows)
        || baseline.usize("linkedCountBefore")? != linked_before.len()
        || parse_list(baseline.value("linkedAliasesBefore")?)?
            != linked_before.iter().map(String::as_str).collect::<Vec<_>>()
        || baseline.value("linkedSetHashBefore")? != sha256_rows(&linked_before)
        || baseline.usize("closedCountBefore")? != closed_before.len()
        || parse_list(baseline.value("closedAliasesBefore")?)?
            != closed_before.iter().map(String::as_str).collect::<Vec<_>>()
        || result.value("stateHashAfter")? != sha256_rows(&after_state_rows)
        || result.usize("linkedCountAfter")? != linked_after.len()
        || parse_list(result.value("linkedAliasesAfter")?)?
            != linked_after.iter().map(String::as_str).collect::<Vec<_>>()
        || result.value("linkedSetHashAfter")? != sha256_rows(&linked_after)
    {
        return Err("independent B arena census/hash differs".into());
    }
    for entry in &entries {
        if entry.linked_after != (entry.linked_before || entry.target)
            || (!entry.target && entry.linked_before != entry.linked_after)
        {
            return Err("independent shared-cell transition differs".into());
        }
    }

    let expected_observers = std::iter::once(target_entry.alias.as_str())
        .chain(target_entry.child_aliases.iter().map(String::as_str))
        .collect::<Vec<_>>();
    if observation_rows.len() != expected_observers.len() {
        return Err("observation count differs from parent plus ordered children".into());
    }
    for (ordinal, (row, alias)) in observation_rows.iter().zip(&expected_observers).enumerate() {
        let parent = ordinal == 0;
        let current = *alias == target.value("vAlias")?;
        let side = if parent {
            "-"
        } else {
            alias.rsplit_once(":v:").unwrap().1
        };
        let role = if parent {
            "ParentB"
        } else if current {
            "CurrentV"
        } else {
            "OtherV"
        };
        if row.usize("observationOrdinal")? != ordinal
            || row.value("role")? != role
            || row.value("alias")? != *alias
            || row.value("vSide")? != side
            || row.bool("current")? != current
            || row.value("sharedCell")? != format!("bcell:{}", target_entry.alias)
            || !row.bool("sameCell")?
            || row.bool("ownsLinkedField")? != parent
            || row.bool("linkedBefore")? != target_entry.linked_before
            || row.bool("linkedAfter")? != target_entry.linked_after
            || row.bool("closedBefore")? != target_entry.closed_before
            || row.bool("closedAfter")? != target_entry.closed_after
            || row.value("readMethod")?
                != if parent {
                    "BLinker.isLinked"
                } else {
                    "VLinker.isLinked"
                }
        {
            return Err(format!("shared-cell observation {ordinal} differs"));
        }
    }

    let value_change_count = usize::from(!target_entry.linked_before);
    let transition = if target_entry.linked_before {
        "TrueToTrueIdempotent"
    } else {
        "FalseToTrue"
    };
    if write.value("getter")? != "getBLinker"
        || !write.bool("getterCompleted")?
        || write.value("receiverAlias")? != target_entry.alias
        || write.value("setter")? != "setLinked"
        || !write.bool("requested")?
        || write.bool("before")? != target_entry.linked_before
        || !write.bool("after")?
        || !write.bool("assignmentAttempted")?
        || !write.bool("assignmentCompleted")?
        || write.usize("writeCount")? != 1
        || write.usize("valueChangeCount")? != value_change_count
        || write.value("transition")? != transition
        || write.value("return")? != "void"
        || write.usize("setterBodyValidationReads")? != 0
        || write.usize("setterBodyCallbacks")? != 0
        || write.usize("setterBodyListeners")? != 0
        || write.usize("setterBodyAllocations")? != 0
        || !write.bool("plainNonVolatile")?
        || write.value("throwStage")? != "-"
        || write.value("exception")? != "-"
    {
        return Err("exact Java setter trace differs".into());
    }

    let apply_returned = predecessor.bool("applyReturn")?;
    let grade_bits = parse_hex_double_bits(predecessor.value("supportGrade")?)?;
    if predecessor.value("predecessorTerminal")? != "ReadyBeforeBLinkerFlagMutation"
        || result.value("terminal")? != "ReadyBeforeSiblingBeamLinks"
        || result.bool("applyReturn")? != apply_returned
        || result.value("supportGrade")? != predecessor.value("supportGrade")?
        || parse_hex_double_bits(result.value("supportGrade")?)? != grade_bits
        || result.bool("supportGradeRead")?
        || result.value("stemAlias")? != predecessor.value("stemAlias")?
        || result.usize("stemInterId")? != predecessor.usize("stemInterId")?
        || result.value("bAlias")? != target_entry.alias
        || !result.bool("linked")?
        || result.usize("writeCount")? != 1
        || result.usize("valueChangeCount")? != value_change_count
        || result.bool("linkSiblingsCalled")?
    {
        return Err("result/predecessor seam join differs".into());
    }
    for (before, after) in [
        ("allocatorBefore", "allocatorAfter"),
        ("glyphActiveHashBefore", "glyphActiveHashAfter"),
        ("glyphOriginalsHashBefore", "glyphOriginalsHashAfter"),
        ("interIndexHashBefore", "interIndexHashAfter"),
        ("sigHashBefore", "sigHashAfter"),
        ("sigRelationHashBefore", "sigRelationHashAfter"),
        ("systemStemsHashBefore", "systemStemsHashAfter"),
        ("lineHashBefore", "lineHashAfter"),
        ("arenaTopologyHashBefore", "arenaTopologyHashAfter"),
    ] {
        if guard.value(before)? != guard.value(after)? {
            return Err(format!("delta guard {before}/{after} differs"));
        }
    }
    if guard.value("guardDomain")?
        != if scope == "real" {
            "RealCommittedSheet"
        } else {
            "IsolatedCellPlusEnclosingRealUnchanged"
        }
        || guard.value("arenaTopologyHashBefore")? != baseline.value("topologyHash")?
        || !guard.bool("allBIdentityOrderUnchanged")?
        || !guard.bool("allBClosedUnchanged")?
        || !guard.bool("unrelatedBLinkedUnchanged")?
        || !guard.bool("headLinkerFlagsUnchanged")?
        || !guard.bool("noCellOtherThanSelectedChanged")?
        || guard.value("allowedMutation")?
            != if value_change_count == 1 {
                "SelectedBLinkedFalseToTrue"
            } else {
                "IdempotentWriteNoValueDelta"
            }
        || guard.bool("supportGradeRead")?
        || guard.bool("siblingDiscoveryRead")?
        || !guard.bool("stopBeforeSiblingBeamLinks")?
        || !guard.bool("stopBeforeHeadRelationLoop")?
        || summary.value("transition")? != transition
        || summary.usize("writeCount")? != 1
        || summary.usize("valueChangeCount")? != value_change_count
        || summary.usize("observations")? != observation_rows.len()
        || summary.usize("arenaEntries")? != entries.len()
        || summary.usize("dynamicAnchors")? != anchors
        || summary.bool("applyReturn")? != apply_returned
        || summary.value("terminal")? != "ReadyBeforeSiblingBeamLinks"
    {
        return Err("guard/summary algebra differs".into());
    }

    Ok(OracleFlagReplay {
        system_id,
        plan_ordinal,
        scope: scope.to_string(),
        case_name: case_name.to_string(),
        target_alias: target_entry.alias.clone(),
        trigger_alias: target.value("vAlias")?.to_string(),
        ordered_observer_aliases: target_entry.child_aliases.clone(),
        linked_before: target_entry.linked_before,
        linked_after: target_entry.linked_after,
        write_count: 1,
        value_change_count,
        apply_returned,
        continuation_support_grade_bits: grade_bits,
        stem_alias: result.value("stemAlias")?.to_string(),
        stem_inter_id: result
            .value("stemInterId")?
            .parse()
            .map_err(|_| "invalid stemInterId".to_string())?,
        arena_entries: entries.len(),
        frozen_entries: frozen,
        dynamic_anchors: anchors,
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn page_key(page: &str) -> Result<&'static str, String> {
    match page {
        "chula.png#1" => Ok("chula"),
        "allegretto.png#1" => Ok("allegretto"),
        "batuque.png#1" => Ok("batuque"),
        "carmen.png#1" => Ok("carmen"),
        "cucaracha.png#1" => Ok("cucaracha"),
        "hove.png#1" => Ok("hove"),
        "zizi.png#1" => Ok("zizi"),
        "BachInvention5.jpg#1" => Ok("BachInvention5"),
        _ => Err(format!("unknown page token {page}")),
    }
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
        value => Err(format!("unknown boundary-15 page key {value}")),
    }
}

fn validate_base_apply_gate_source_sha256(actual: &str) -> Result<(), String> {
    if actual != BASE_APPLY_GATE_SOURCE_SHA256 {
        return Err("copied boundary-14 hydration source has drifted".into());
    }
    Ok(())
}

fn validate_base_apply_gate_source_pin() -> Result<(), String> {
    validate_base_apply_gate_source_sha256(&read_sha256(BASE_APPLY_GATE_SOURCE)?)
}

fn predecessor_paths(key: &str) -> [(String, String); 5] {
    [
        (
            "schedulerFixtureSha256".into(),
            format!("rust/oracle/stems-beam-scheduler-{key}.txt"),
        ),
        (
            "expandFixtureSha256".into(),
            format!("rust/oracle/stems-beam-expand-{key}.txt"),
        ),
        (
            "createStemFixtureSha256".into(),
            format!("rust/oracle/stems-beam-create-stem-{key}.txt"),
        ),
        (
            "reuseCheckFixtureSha256".into(),
            format!("rust/oracle/stems-beam-vlink-reuse-check-{key}.txt"),
        ),
        (
            "baseApplyFixtureSha256".into(),
            format!("rust/oracle/stems-beam-vlink-base-apply-{key}.txt"),
        ),
    ]
}

fn read_sha256(relative: &str) -> Result<String, String> {
    let path = repo_root().join(relative);
    let bytes = std::fs::read(&path)
        .map_err(|error| format!("cannot read provenance path {}: {error}", path.display()))?;
    Ok(sha256_hex(&bytes))
}

fn validate_file_sha256(relative: &str, expected: &str) -> Result<(), String> {
    if !is_lower_sha256(expected) || read_sha256(relative)? != expected {
        return Err(format!("{relative} provenance SHA differs"));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ActiveJGraphTCoreArtifact {
    Absent,
    Present(PathBuf),
}

fn locate_jgrapht_core_artifact(root: Option<&Path>) -> Result<ActiveJGraphTCoreArtifact, String> {
    let Some(root) = root else {
        return Ok(ActiveJGraphTCoreArtifact::Absent);
    };
    let artifact_root = root
        .join("caches/modules-2/files-2.1/org.jgrapht/jgrapht-core")
        .join(JGRAPHT_CORE_VERSION);
    let entries = match std::fs::read_dir(&artifact_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ActiveJGraphTCoreArtifact::Absent);
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
        let entry = entry.map_err(|error| format!("cannot enumerate JGraphT cache: {error}"))?;
        let candidate = entry.path().join(JGRAPHT_CORE_JAR);
        match std::fs::metadata(&candidate) {
            Ok(metadata) if metadata.is_file() => candidates.push(candidate),
            Ok(_) => return Err(format!("{} is not a file", candidate.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if std::fs::symlink_metadata(&candidate).is_ok() {
                    return Err(format!("{} is an unreadable link", candidate.display()));
                }
            }
            Err(error) => return Err(format!("cannot inspect {}: {error}", candidate.display())),
        }
    }
    candidates.sort();
    match candidates.as_slice() {
        [] => Ok(ActiveJGraphTCoreArtifact::Absent),
        [path] => Ok(ActiveJGraphTCoreArtifact::Present(path.clone())),
        _ => Err(format!(
            "expected at most one active {JGRAPHT_CORE_JAR}, found {}",
            candidates.len()
        )),
    }
}

fn validate_jgrapht_provenance(expected: &str) -> Result<(), String> {
    if expected != JGRAPHT_CORE_JAR_SHA256 {
        return Err("JGraphT fixture SHA differs from audited artifact".into());
    }
    let gradle = std::fs::read_to_string(repo_root().join("app/build.gradle"))
        .map_err(|error| format!("cannot read app/build.gradle: {error}"))?;
    if gradle
        .lines()
        .filter(|line| line.trim() == JGRAPHT_CORE_GRADLE_DECLARATION)
        .count()
        != 1
    {
        return Err("JGraphT Gradle declaration is not exact and unique".into());
    }
    let gradle_root = std::env::var_os("GRADLE_USER_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".gradle")));
    if let ActiveJGraphTCoreArtifact::Present(path) =
        locate_jgrapht_core_artifact(gradle_root.as_deref())?
    {
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        if sha256_hex(&bytes) != expected {
            return Err("active JGraphT artifact SHA differs".into());
        }
    }
    Ok(())
}

fn raw_fields(line: &str) -> Result<BTreeMap<String, String>, String> {
    let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
    if tokens.len() < 2 || (tokens.len() - 2) % 2 != 0 {
        return Err("predecessor row does not contain page plus field pairs".into());
    }
    let mut fields = BTreeMap::new();
    for pair in tokens[2..].chunks_exact(2) {
        if fields.insert(pair[0].into(), pair[1].into()).is_some() {
            return Err(format!("duplicate predecessor field {}", pair[0]));
        }
    }
    Ok(fields)
}

fn base_apply_row<'a>(
    text: &'a str,
    family: &str,
    system_id: usize,
    scope: &str,
    case_name: &str,
) -> Result<(&'a str, BTreeMap<String, String>), String> {
    let label = format!("stemsbeamvlinkbaseapply{family} ");
    let matches = text
        .lines()
        .filter(|line| line.starts_with(&label))
        .filter_map(|line| {
            let fields = raw_fields(line).ok()?;
            (fields.get("system")?.parse().ok() == Some(system_id)
                && fields.get("scope").map(String::as_str) == Some(scope)
                && fields.get("case").map(String::as_str) == Some(case_name))
            .then_some((line, fields))
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [(line, fields)] => Ok((*line, fields.clone())),
        _ => Err(format!(
            "expected one base-apply {scope}/{case_name} {family}, found {}",
            matches.len()
        )),
    }
}

fn required_map<'a>(fields: &'a BTreeMap<String, String>, name: &str) -> Result<&'a str, String> {
    fields
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| format!("missing predecessor field {name}"))
}

fn validate_base_apply_bundle(rows: &[StrictRow], base_apply: &str) -> Result<(), String> {
    let predecessor = one_row(rows, RowKind::Predecessor)?;
    let (system_id, plan, scope, case_name) = predecessor.common_key()?;
    let expected_base_case = match (scope, case_name) {
        ("real", "-") => "-",
        ("synthetic", "InitialFalse" | "IdempotentTrue" | "TwoChildrenShared") => {
            "ExistingFullSuccess"
        }
        ("synthetic", "ApplyReturnFalse") => "ExistingDuplicateSuppress",
        _ => return Err("unsupported boundary-15 scope/case".into()),
    };
    if predecessor.value("join")?
        != if scope == "real" {
            "FullBoundary14Replay"
        } else {
            "IsolatedBoundary14Replay"
        }
        || predecessor.value("baseCase")? != expected_base_case
    {
        return Err("boundary-14 replay join/base case differs".into());
    }
    let base_scope = if scope == "real" { "real" } else { "synthetic" };
    let mut bundles = Vec::new();
    let mut values = Vec::new();
    for (family, pin) in [
        ("frontier", "baseFrontierRowSha256"),
        ("result", "baseResultRowSha256"),
        ("deltaguard", "baseGuardRowSha256"),
        ("summary", "baseSummaryRowSha256"),
    ] {
        let (line, fields) = base_apply_row(
            base_apply,
            family,
            system_id,
            base_scope,
            expected_base_case,
        )?;
        if required_map(&fields, "plan")?.parse::<usize>().ok() != Some(plan) {
            return Err(format!("base-apply {family} plan differs"));
        }
        let hash = sha256_rows(&[line]);
        if predecessor.value(pin)? != hash {
            return Err(format!("base-apply {family} row SHA differs"));
        }
        bundles.push(hash);
        values.push(fields);
    }
    if predecessor.value("baseEvidenceSha256")? != sha256_rows(&bundles)
        || predecessor.value("predecessorTerminal")? != required_map(&values[1], "terminal")?
        || predecessor.bool("applyReturn")? != (required_map(&values[1], "applyReturn")? == "true")
        || predecessor.value("supportGrade")? != required_map(&values[1], "retainedDraftGrade")?
        || predecessor.value("stemAlias")? != required_map(&values[1], "finalStemAlias")?
        || predecessor.value("stemInterId")? != required_map(&values[1], "finalStemInterId")?
    {
        return Err("boundary-14 row bundle/result join differs".into());
    }
    if scope == "real" {
        let expected_v = format!(
            "{}:v:{}",
            required_map(&values[0], "bAlias")?,
            required_map(&values[0], "vSide")?
        );
        if predecessor.value("bAlias")? != required_map(&values[0], "bAlias")?
            || predecessor.value("vAlias")? != expected_v
        {
            return Err("real B/V alias does not join boundary-14 frontier".into());
        }
    }
    Ok(())
}

fn manifest_fixture_pin<'a>(manifest: &'a str, key: &str) -> Result<&'a str, String> {
    let matches = manifest
        .lines()
        .filter(|line| line.starts_with("stemsbeamvlinkbaseapplymanifestentry "))
        .filter_map(|line| {
            let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
            let fields = tokens[1..]
                .chunks_exact(2)
                .map(|pair| (pair[0], pair[1]))
                .collect::<BTreeMap<_, _>>();
            if fields.get("page") == Some(&key) {
                fields.get("fixtureSha256").copied()
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [pin] => Ok(*pin),
        _ => Err(format!("expected one base-apply manifest entry for {key}")),
    }
}

fn validate_fixture_provenance(rows: &[StrictRow], text: &str) -> Result<(), String> {
    let page = one_row(rows, RowKind::Page)?;
    let page_summary = one_row(rows, RowKind::PageSummary)?;
    let corpus = one_row(rows, RowKind::CorpusSummary)?;
    let key = page_key(&page.page)?;
    let systems = page.usize("systems")?;
    let image = page_image_name(key)?;
    let page_input_sha = read_sha256(&format!("data/examples/{image}"))?;
    if page.value("executionMode")? != "foregroundJvmPerSystem"
        || page.value("arenaOrder")? != "BeamConstructorThenAllBInsertion"
        || page.value("childOrder")? != "EnumMapTOPThenBOTTOM"
        || !page.bool("headless")?
        || page.value("setterDispatch")? != "ExactRuntimeClassBeamLinkerBLinker"
        || page.value("syntheticCellConstruction")? != "UnsafeExactClassNoGeometry"
        || page.value("syntheticEvidenceScope")? != "SetterAndSharedCellOnly"
        || page.bool("syntheticGeometryRead")?
        || corpus.value("schema")? != "stems-beam-vlink-b-linker-flag-v1"
        || corpus.value("mode")? != key
        || corpus.usize("pages")? != 1
        || corpus.value("pageRefs")? != page.page
        || corpus.value("pageInputSha256")? != page_input_sha
    {
        return Err("page/corpus execution identity differs".into());
    }
    for (field, relative) in predecessor_paths(key) {
        let actual = read_sha256(&relative)?;
        if page.value(&field)? != actual || corpus.value(&field)? != actual {
            return Err(format!("{field} does not hash active predecessor"));
        }
    }
    let base_manifest = std::fs::read_to_string(repo_root().join(BASE_APPLY_MANIFEST))
        .map_err(|error| format!("cannot read base-apply manifest: {error}"))?;
    let base_manifest_sha = read_sha256(BASE_APPLY_MANIFEST)?;
    if corpus.value("baseApplyManifestSha256")? != base_manifest_sha
        || manifest_fixture_pin(&base_manifest, key)? != corpus.value("baseApplyFixtureSha256")?
    {
        return Err("base-apply split/manifest provenance differs".into());
    }
    for (field, relative) in [
        ("probeSourceSha256", PROBE_SOURCE),
        ("runnerSourceSha256", RUNNER_SOURCE),
    ]
    .into_iter()
    .chain(SOURCE_PATHS.iter().copied())
    {
        validate_file_sha256(relative, corpus.value(field)?)?;
    }
    if corpus.value("jgraphtCoreVersion")? != JGRAPHT_CORE_VERSION {
        return Err("JGraphT version differs".into());
    }
    validate_jgrapht_provenance(corpus.value("jgraphtCoreJarSha256")?)?;
    for (field, expected) in [
        ("jdkReleaseSha256", JDK_RELEASE_SHA256),
        ("javaExecutableSha256", JAVA_EXECUTABLE_SHA256),
        ("javaJpegLibrarySha256", JAVA_JPEG_LIBRARY_SHA256),
        ("javaModulesSha256", JAVA_MODULES_SHA256),
        ("javaVmLibrarySha256", JAVA_VM_LIBRARY_SHA256),
        ("javaAwtLibrarySha256", JAVA_AWT_LIBRARY_SHA256),
        ("javaAwtLwawtLibrarySha256", JAVA_AWT_LWAWT_LIBRARY_SHA256),
    ] {
        if corpus.value(field)? != expected {
            return Err(format!("{field} differs from the frozen Temurin image"));
        }
    }
    if corpus.value("javaArchitecture")? != "aarch64"
        || corpus.value("javaRuntimeVersion")? != "25.0.3+9-LTS"
        || corpus.value("javaVmVariant")? != "Hotspot"
        || corpus.value("javaImageType")? != "JDK"
    {
        return Err("frozen Temurin runtime identity differs".into());
    }
    if corpus.value("effectiveClasspathSha256")? != EFFECTIVE_CLASSPATH_SHA256 {
        return Err("effectiveClasspathSha256 differs from the frozen classpath".into());
    }
    for (field, expected) in [
        ("beamLinkerClassSha256", BEAM_LINKER_CLASS_SHA256),
        ("bLinkerClassSha256", B_LINKER_CLASS_SHA256),
        ("vLinkerClassSha256", V_LINKER_CLASS_SHA256),
        ("stemLinkerClassSha256", STEM_LINKER_CLASS_SHA256),
    ] {
        if corpus.value(field)? != expected {
            return Err(format!("{field} differs from the frozen loaded bytecode"));
        }
    }

    let marker = "stemsbeamvlinkblinkerflagpagesummary ";
    let body_start = text
        .find(marker)
        .filter(|start| text[*start + marker.len()..].find(marker).is_none())
        .ok_or_else(|| "page-summary marker is not unique".to_string())?;
    let body = &text.as_bytes()[..body_start];
    let body_lines = body.iter().filter(|byte| **byte == b'\n').count();
    if corpus.value("emittedBodySha256")? != sha256_hex(body)
        || corpus.value("rawPassSha256")? != sha256_hex(body)
        || corpus.usize("emittedBodyLines")? != body_lines
        || corpus.usize("emittedBodyBytes")? != body.len()
    {
        return Err("emitted body hash/line/byte provenance differs".into());
    }
    let semantic = &rows[..rows.len() - 2];
    let families = [
        RowKind::Page,
        RowKind::Predecessor,
        RowKind::Baseline,
        RowKind::BEntry,
        RowKind::Target,
        RowKind::Observation,
        RowKind::WriteTrace,
        RowKind::Result,
        RowKind::DeltaGuard,
        RowKind::Summary,
    ];
    let row_counts = families
        .iter()
        .map(|kind| {
            let label = match kind {
                RowKind::Page => "stemsbeamvlinkblinkerflagpage",
                RowKind::Predecessor => "stemsbeamvlinkblinkerflagpredecessor",
                RowKind::Baseline => "stemsbeamvlinkblinkerflagbaseline",
                RowKind::BEntry => "stemsbeamvlinkblinkerflagbentry",
                RowKind::Target => "stemsbeamvlinkblinkerflagtarget",
                RowKind::Observation => "stemsbeamvlinkblinkerflagobservation",
                RowKind::WriteTrace => "stemsbeamvlinkblinkerflagwritetrace",
                RowKind::Result => "stemsbeamvlinkblinkerflagresult",
                RowKind::DeltaGuard => "stemsbeamvlinkblinkerflagdeltaguard",
                RowKind::Summary => "stemsbeamvlinkblinkerflagsummary",
                _ => unreachable!(),
            };
            format!(
                "{label}:{}",
                semantic.iter().filter(|row| row.kind == *kind).count()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    if corpus.value("rowCounts")? != row_counts
        || !corpus.bool("predecessorSwapNegative")?
        || corpus.value("baseEvidenceMeaning")? != "OrderedFourRowShaBundleNotTypedState"
        || corpus.usize("freshRunsPerPage")? != 2
        || !corpus.bool("freshRunsByteIdentical")?
        || !corpus.bool("freshJvmPerSystem")?
        || corpus.usize("compilerJavaProcesses")? != 1
        || corpus.usize("runtimeJavaProcessesPerPass")? != systems
        || corpus.usize("runtimeJavaProcesses")? != systems * 2
        || corpus.usize("totalJavaProcesses")? != systems * 2 + 1
        || corpus.usize("maximumConcurrentJavaProcesses")? != 1
        || corpus.value("concurrencyScope")? != "Boundary15RunnerLockedInvocation"
        || !corpus.bool("compilerJavaProcessReaped")?
        || !corpus.bool("runtimeJavaProcessesReaped")?
        || !corpus.bool("foregroundJavaProcessesOnly")?
        || corpus.usize("backgroundJavaProcessesStarted")? != 0
        || !corpus.bool("dynamicAllBArenaGuardOnly")?
        || !corpus.bool("stopBeforeSiblingBeamLinks")?
        || page_summary.usize("systems")? != systems
    {
        return Err("runner/trailer provenance algebra differs".into());
    }
    Ok(())
}

fn validate_fixture(text: &str) -> Result<Vec<OracleFlagReplay>, String> {
    let rows = parse_scaffold_fixture(text)?;
    validate_fixture_provenance(&rows, text)?;
    let page = one_row(&rows, RowKind::Page)?;
    let systems = page.usize("systems")?;
    let key = page_key(&page.page)?;
    let base_path = predecessor_paths(key)[4].1.clone();
    let base_apply = std::fs::read_to_string(repo_root().join(&base_path))
        .map_err(|error| format!("cannot read {base_path}: {error}"))?;
    let blocks = transaction_blocks(&rows)?;
    let mut expected = Vec::new();
    expected.push((1, "real", "-"));
    expected.extend(
        SYNTHETIC_CASES
            .iter()
            .map(|case_name| (1, "synthetic", *case_name)),
    );
    expected.extend((2..=systems).map(|system| (system, "real", "-")));
    if blocks.len() != expected.len() {
        return Err(format!(
            "expected {} transaction blocks, found {}",
            expected.len(),
            blocks.len()
        ));
    }
    let mut replays = Vec::new();
    for (block, expected_key) in blocks.into_iter().zip(expected) {
        let key = block[0].common_key()?;
        if (key.0, key.2, key.3) != expected_key {
            return Err(format!("transaction block order differs at {key:?}"));
        }
        validate_base_apply_bundle(block, &base_apply)?;
        let replay = validate_and_replay_block(block)?;
        if replay.system_id != key.0
            || replay.plan_ordinal != key.1
            || replay.scope != key.2
            || replay.case_name != key.3
            || replay.target_alias.is_empty()
            || replay.trigger_alias.is_empty()
            || !replay
                .ordered_observer_aliases
                .iter()
                .any(|alias| alias == &replay.trigger_alias)
            || !replay.linked_after
            || replay.write_count != 1
            || replay.value_change_count != usize::from(!replay.linked_before)
            || replay.apply_returned != (replay.case_name != "ApplyReturnFalse")
            || !f64::from_bits(replay.continuation_support_grade_bits).is_finite()
            || replay.stem_alias.is_empty()
            || replay.stem_inter_id == 0
        {
            return Err("independent replay terminal domain differs".into());
        }
        replays.push(replay);
    }
    let real = replays
        .iter()
        .filter(|replay| replay.scope == "real")
        .count();
    let synthetic = replays
        .iter()
        .filter(|replay| replay.scope == "synthetic")
        .count();
    let page_summary = one_row(&rows, RowKind::PageSummary)?;
    let real_arena = replays
        .iter()
        .filter(|replay| replay.scope == "real")
        .map(|replay| replay.arena_entries)
        .sum::<usize>();
    let frozen = replays
        .iter()
        .filter(|replay| replay.scope == "real")
        .map(|replay| replay.frozen_entries)
        .sum::<usize>();
    let anchors = replays
        .iter()
        .filter(|replay| replay.scope == "real")
        .map(|replay| replay.dynamic_anchors)
        .sum::<usize>();
    if real != systems
        || synthetic != SYNTHETIC_CASES.len()
        || page_summary.usize("realTransactions")? != real
        || page_summary.usize("syntheticCases")? != synthetic
        || page_summary.usize("totalTransactions")? != replays.len()
        || page_summary.usize("realFalseToTrue")? != real
        || page_summary.usize("realIdempotent")? != 0
        || page_summary.usize("syntheticFalseToTrue")? != 3
        || page_summary.usize("syntheticIdempotent")? != 1
        || page_summary.usize("realArenaEntries")? != real_arena
        || page_summary.usize("frozenEntries")? != frozen
        || page_summary.usize("dynamicAnchors")? != anchors
        || page_summary.usize("isolatedExactClassCells")? != 4
        || page_summary.usize("applyReturnFalseCases")? != 1
        || !page_summary.bool("stopBeforeSiblingBeamLinks")?
    {
        return Err("page summary census differs".into());
    }
    Ok(replays)
}

fn parse_row_counts(value: &str) -> Result<Vec<(String, usize)>, String> {
    value
        .split(',')
        .map(|item| {
            let (label, count) = item
                .split_once(':')
                .ok_or_else(|| "manifest rowCounts item lacks colon".to_string())?;
            let count = count
                .parse()
                .map_err(|_| "manifest rowCounts count is invalid".to_string())?;
            Ok((label.to_string(), count))
        })
        .collect()
}

fn validate_flag_manifest(bytes: &[u8]) -> Result<(), String> {
    let manifest = StrictFlagManifest::parse(bytes)?;
    let summary = &manifest.summary;
    if summary.value("schema")? != "stems-beam-vlink-b-linker-flag-manifest-v1"
        || summary.usize("entries")? != CORPUS_PAGES.len()
    {
        return Err("manifest summary identity differs".into());
    }
    let body = &bytes[..manifest.body_len];
    if summary.value("manifestBodySha256")? != sha256_hex(body)
        || summary.value("manifestBodySha256")? != FLAG_MANIFEST_BODY_SHA256
        || summary.usize("manifestBodyLines")? != body.iter().filter(|byte| **byte == b'\n').count()
        || summary.usize("manifestBodyBytes")? != body.len()
    {
        return Err("manifest body fingerprint differs".into());
    }

    let mut normalized = Vec::new();
    let mut shared_header: Option<Vec<u8>> = None;
    let mut split_lines = 0usize;
    let mut split_bytes = 0usize;
    let mut semantic_rows = 0usize;
    let mut row_order = Vec::<String>::new();
    let mut row_totals = BTreeMap::<String, usize>::new();
    let mut aggregate = BTreeMap::<&str, usize>::new();
    let aggregate_fields = [
        "realTransactions",
        "realFalseToTrue",
        "realIdempotent",
        "realArenaEntries",
        "frozenEntries",
        "dynamicAnchors",
        "syntheticCases",
        "syntheticFalseToTrue",
        "syntheticIdempotent",
        "isolatedExactClassCells",
        "applyReturnFalseCases",
        "totalTransactions",
        "compilerJavaProcesses",
        "runtimeJavaProcesses",
        "totalJavaProcesses",
    ];
    let common_fields = MANIFEST_SUMMARY_FIELDS
        .iter()
        .copied()
        .skip_while(|field| *field != "probeSourceSha256")
        .take_while(|field| *field != "corpusBodySha256")
        .collect::<Vec<_>>();

    for (entry, page_key) in manifest.entries.iter().zip(CORPUS_PAGES) {
        let expected_fixture = format!("stems-beam-vlink-b-linker-flag-{page_key}.txt");
        if entry.value("fixture")? != expected_fixture {
            return Err(format!("{page_key} manifest fixture name differs"));
        }
        let fixture_path = repo_root().join("rust/oracle").join(&expected_fixture);
        let fixture = std::fs::read(&fixture_path)
            .map_err(|error| format!("cannot read {}: {error}", fixture_path.display()))?;
        let text = std::str::from_utf8(&fixture)
            .map_err(|_| format!("{page_key} fixture is not UTF-8"))?;
        let rows = parse_scaffold_fixture(text)?;
        validate_fixture_provenance(&rows, text)?;
        let page = one_row(&rows, RowKind::Page)?;
        let page_summary = one_row(&rows, RowKind::PageSummary)?;
        let corpus = one_row(&rows, RowKind::CorpusSummary)?;
        let fixture_lines = fixture.iter().filter(|byte| **byte == b'\n').count();
        if entry.value("fixtureSha256")? != sha256_hex(&fixture)
            || entry.usize("fixtureLines")? != fixture_lines
            || entry.usize("fixtureBytes")? != fixture.len()
        {
            return Err(format!("{page_key} full fixture fingerprint differs"));
        }
        split_lines += fixture_lines;
        split_bytes += fixture.len();

        let line_bytes = fixture
            .split_inclusive(|byte| *byte == b'\n')
            .collect::<Vec<_>>();
        if line_bytes.len() != fixture_lines || line_bytes.len() < FIXTURE_HEADER.len() + 3 {
            return Err(format!("{page_key} split fixture line structure differs"));
        }
        let header = line_bytes[..FIXTURE_HEADER.len()].concat();
        if let Some(expected) = &shared_header {
            if *expected != header {
                return Err("split fixture headers differ".into());
            }
        } else {
            normalized.extend_from_slice(&header);
            shared_header = Some(header);
        }
        let semantic = line_bytes[FIXTURE_HEADER.len()..line_bytes.len() - 2].concat();
        normalized.extend_from_slice(&semantic);
        semantic_rows += line_bytes.len() - FIXTURE_HEADER.len() - 2;

        let counts = parse_row_counts(corpus.value("rowCounts")?)?;
        if entry.value("rowCounts")? != corpus.value("rowCounts")? {
            return Err(format!("{page_key} manifest rowCounts join differs"));
        }
        if row_order.is_empty() {
            row_order = counts.iter().map(|(label, _)| label.clone()).collect();
        } else if counts.iter().map(|(label, _)| label).ne(row_order.iter()) {
            return Err("split rowCounts family order differs".into());
        }
        for (label, count) in counts {
            *row_totals.entry(label).or_default() += count;
        }

        for field in [
            "systems",
            "realTransactions",
            "syntheticCases",
            "totalTransactions",
            "realFalseToTrue",
            "realIdempotent",
            "syntheticFalseToTrue",
            "syntheticIdempotent",
            "realArenaEntries",
            "frozenEntries",
            "dynamicAnchors",
            "isolatedExactClassCells",
            "applyReturnFalseCases",
        ] {
            if entry.value(field)? != page_summary.value(field)? {
                return Err(format!("{page_key} manifest page census {field} differs"));
            }
        }
        for field in [
            "pageInputSha256",
            "schedulerFixtureSha256",
            "expandFixtureSha256",
            "createStemFixtureSha256",
            "reuseCheckFixtureSha256",
            "baseApplyFixtureSha256",
            "baseApplyManifestSha256",
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
            "predecessorSwapNegative",
            "baseEvidenceMeaning",
            "dynamicAllBArenaGuardOnly",
            "stopBeforeSiblingBeamLinks",
        ] {
            if entry.value(field)? != corpus.value(field)? {
                return Err(format!("{page_key} manifest corpus join {field} differs"));
            }
        }
        for (entry_field, page_field) in [
            ("syntheticCellConstruction", "syntheticCellConstruction"),
            ("syntheticEvidenceScope", "syntheticEvidenceScope"),
            ("syntheticGeometryRead", "syntheticGeometryRead"),
        ] {
            if entry.value(entry_field)? != page.value(page_field)? {
                return Err(format!("{page_key} synthetic scope join differs"));
            }
        }
        if !entry.bool("system1SyntheticBlock")? {
            return Err(format!("{page_key} lacks its system-1 synthetic block"));
        }
        for field in &common_fields {
            if corpus.value(field)? != summary.value(field)? {
                return Err(format!("{page_key} common provenance {field} differs"));
            }
        }
        for field in aggregate_fields {
            *aggregate.entry(field).or_default() += entry.usize(field)?;
        }
    }

    let corpus_counts = row_order
        .iter()
        .map(|label| format!("{label}:{}", row_totals[label]))
        .collect::<Vec<_>>()
        .join(",");
    if summary.value("corpusBodySha256")? != sha256_hex(&normalized)
        || summary.value("corpusBodySha256")? != FLAG_CORPUS_SHA256
        || summary.usize("corpusBodyLines")?
            != normalized.iter().filter(|byte| **byte == b'\n').count()
        || summary.usize("corpusBodyLines")? != FLAG_CORPUS_LINES
        || summary.usize("corpusBodyBytes")? != normalized.len()
        || summary.usize("corpusBodyBytes")? != FLAG_CORPUS_BYTES
        || summary.value("corpusRowCounts")? != corpus_counts
        || summary.value("corpusReconstruction")? != "SharedHeaderOnceThenPageSemanticRows"
        || summary.usize("sharedHeaderLines")? != FIXTURE_HEADER.len()
        || summary.usize("semanticRows")? != semantic_rows
        || summary.usize("splitFixtureLines")? != split_lines
        || summary.usize("splitFixtureLines")? != FLAG_SPLIT_LINES
        || summary.usize("splitFixtureBytes")? != split_bytes
        || summary.usize("splitFixtureBytes")? != FLAG_SPLIT_BYTES
    {
        return Err("normalized corpus reconstruction differs".into());
    }
    for (entry_field, summary_field) in [
        ("realTransactions", "realTransactions"),
        ("realFalseToTrue", "realFalseToTrue"),
        ("realIdempotent", "realIdempotent"),
        ("realArenaEntries", "realArenaEntries"),
        ("frozenEntries", "frozenEntries"),
        ("dynamicAnchors", "dynamicAnchors"),
        ("syntheticCases", "syntheticCases"),
        ("syntheticFalseToTrue", "syntheticFalseToTrue"),
        ("syntheticIdempotent", "syntheticIdempotent"),
        ("isolatedExactClassCells", "isolatedExactClassCells"),
        ("applyReturnFalseCases", "applyReturnFalseCases"),
        ("totalTransactions", "totalTransactions"),
        ("compilerJavaProcesses", "compilerJavaProcesses"),
        ("runtimeJavaProcesses", "runtimeJavaProcesses"),
        ("totalJavaProcesses", "totalJavaProcesses"),
    ] {
        if summary.usize(summary_field)? != aggregate[entry_field] {
            return Err(format!("manifest aggregate {summary_field} differs"));
        }
    }
    if summary.usize("realSystems")? != aggregate["realTransactions"]
        || summary.usize("syntheticBlocks")? != CORPUS_PAGES.len()
        || summary.usize("maximumConcurrentJavaProcesses")? != 1
        || summary.value("concurrencyScope")? != "Boundary15RunnerLockedInvocation"
        || summary.usize("freshRunsPerPage")? != 2
        || !summary.bool("freshRunsByteIdentical")?
        || !summary.bool("freshJvmPerSystem")?
        || !summary.bool("compilerJavaProcessesReaped")?
        || !summary.bool("runtimeJavaProcessesReaped")?
        || !summary.bool("foregroundJavaProcessesOnly")?
        || summary.usize("backgroundJavaProcessesStarted")? != 0
        || !summary.bool("system1SyntheticBlock")?
        || summary.value("syntheticCellConstruction")? != "UnsafeExactClassNoGeometry"
        || summary.value("syntheticEvidenceScope")? != "SetterAndSharedCellOnly"
        || summary.bool("syntheticGeometryRead")?
        || !summary.bool("predecessorSwapNegative")?
        || summary.value("baseEvidenceMeaning")? != "OrderedFourRowShaBundleNotTypedState"
        || !summary.bool("dynamicAllBArenaGuardOnly")?
        || !summary.bool("stopBeforeSiblingBeamLinks")?
    {
        return Err("manifest aggregate guard differs".into());
    }
    Ok(())
}

#[path = "common/b15_hydration.rs"]
mod b14_hydration;

fn two_child_arena() -> Vec<BLinkerArenaEntry> {
    let b = BLinkerRef {
        beam: BeamSource::Raw(3),
        id: 1,
    };
    vec![BLinkerArenaEntry {
        reference: b,
        is_anchor: false,
        child_v_linkers: vec![
            VLinkerRef {
                b_linker: b,
                side: VerticalSide::Top,
            },
            VLinkerRef {
                b_linker: b,
                side: VerticalSide::Bottom,
            },
        ],
    }]
}

#[test]
fn independent_shared_b_linker_cell_pins_change_idempotence_and_apply_return() {
    let arena = two_child_arena();
    let trigger = arena[0].child_v_linkers[0];
    let key = CommitKey {
        system_id: 1,
        invocation_ordinal: 7,
        plan_ordinal: 143,
    };
    let mut false_state = IndependentFlagState {
        system_id: 1,
        linked_b_linkers: BTreeSet::new(),
        committed: None,
    };
    let changed = apply_independent(&arena, &mut false_state, key, trigger, false, 0.75)
        .expect("false-to-true assignment");
    assert!(
        !changed.apply_returned,
        "the ignored Link.applyTo result is retained"
    );
    assert_eq!(
        changed.continuation_support_grade.to_bits(),
        0.75f64.to_bits()
    );
    assert_eq!(changed.write.shared_v_linkers, arena[0].child_v_linkers);
    assert_eq!((changed.write.before, changed.write.after), (false, true));
    assert_eq!(
        (changed.write.write_count, changed.write.value_change_count),
        (1, 1)
    );
    let public_write = NativeStemsBeamVLinkBLinkerFlagOperation::BLinkerLinkedAssigned {
        target_b_linker: native_b_linker(changed.write.b_linker),
        triggering_v_linker: native_v_linker(changed.write.triggering_v_linker),
        ordered_observer_v_linkers: changed
            .write
            .shared_v_linkers
            .iter()
            .copied()
            .map(native_v_linker)
            .collect(),
        before: changed.write.before,
        after: changed.write.after,
    };
    assert_public_write_shape(&public_write, &changed.write);
    let public_outcome = NativeStemsBeamVLinkBLinkerFlagOutcome::ReadyBeforeSiblingBeamLinks {
        stem_identity: 4,
        continuation_support_grade: changed.continuation_support_grade,
    };
    assert!(matches!(
        public_outcome,
        NativeStemsBeamVLinkBLinkerFlagOutcome::ReadyBeforeSiblingBeamLinks {
            stem_identity: 4,
            continuation_support_grade
        } if continuation_support_grade.to_bits()
            == changed.continuation_support_grade.to_bits()
    ));
    let _public_entry = apply_native_stems_beam_vlink_b_linker_flag_transaction;
    let _exact_projection = assert_public_transaction_matches_replay;
    let _state_binding: Option<NativeStemsBeamVLinkBLinkerFlagState> = None;
    let _transaction_binding: Option<NativeStemsBeamVLinkBLinkerFlagTransaction> = None;

    let mut true_state = IndependentFlagState {
        system_id: 1,
        linked_b_linkers: BTreeSet::from([arena[0].reference]),
        committed: None,
    };
    let idempotent = apply_independent(&arena, &mut true_state, key, trigger, true, 0.75)
        .expect("true-to-true assignment");
    assert_eq!(
        (idempotent.write.before, idempotent.write.after),
        (true, true)
    );
    assert_eq!(
        (
            idempotent.write.write_count,
            idempotent.write.value_change_count
        ),
        (1, 0)
    );
}

#[test]
fn independent_model_fails_closed_before_mutation() {
    let arena = two_child_arena();
    let key = CommitKey {
        system_id: 1,
        invocation_ordinal: 7,
        plan_ordinal: 143,
    };
    let mut state = IndependentFlagState {
        system_id: 1,
        linked_b_linkers: BTreeSet::new(),
        committed: None,
    };
    let before = state.clone();
    let missing = VLinkerRef {
        b_linker: arena[0].reference,
        side: VerticalSide::Bottom,
    };
    let mut incomplete = arena.clone();
    incomplete[0].child_v_linkers.pop();
    assert_eq!(
        apply_independent(&incomplete, &mut state, key, missing, true, 1.0),
        Err(IndependentError::MissingTrigger)
    );
    assert_eq!(state, before);

    let mut arena_with_unrelated_anchor = arena.clone();
    arena_with_unrelated_anchor.push(BLinkerArenaEntry {
        reference: BLinkerRef {
            beam: BeamSource::Hook(9),
            id: 1,
        },
        is_anchor: true,
        child_v_linkers: Vec::new(),
    });
    let mut anchor_state = before.clone();
    assert!(
        apply_independent(
            &arena_with_unrelated_anchor,
            &mut anchor_state,
            key,
            arena[0].child_v_linkers[0],
            true,
            1.0
        )
        .is_ok(),
        "unrelated dynamic anchors have no child but remain valid arena entries"
    );

    state.linked_b_linkers.insert(BLinkerRef {
        beam: BeamSource::Hook(9),
        id: 1,
    });
    let invalid_before = state.clone();
    assert_eq!(
        apply_independent(
            &arena,
            &mut state,
            key,
            arena[0].child_v_linkers[0],
            true,
            1.0
        ),
        Err(IndependentError::InvalidLinkedSet)
    );
    assert_eq!(state, invalid_before);
}

#[test]
fn strict_parser_scaffold_rejects_schema_duplicate_fields_and_trailing_data() {
    let valid = concat!(
        "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) beam VLink B-linker-flag oracle.\n",
        "# schema: stems-beam-vlink-b-linker-flag-v1\n",
        "# Frozen scheduler/expand/createStem/reuse-check/base-apply predecessors are replayed and joined exactly.\n",
        "# The concrete receiver is exact BeamLinker$BLinker; no subclass override is admitted.\n",
        "# One plain void assignment writes the parent B linked field observed by all its V children.\n",
        "# Head S/C linkers do not share this cell and are guard-only unchanged evidence.\n",
        "# Stop is immediately after setLinked(true), before the first linkSiblings read.\n",
        "# System 1 adds isolated exact-class false, idempotent, false-return, and two-child certificates.\n",
        "stemsbeamvlinkblinkerflagpage chula.png#1 systems 3 ",
        "schedulerFixtureSha256 a expandFixtureSha256 b createStemFixtureSha256 c ",
        "reuseCheckFixtureSha256 d baseApplyFixtureSha256 e ",
        "executionMode foregroundJvmPerSystem arenaOrder BeamConstructorThenAllBInsertion ",
        "childOrder EnumMapTOPThenBOTTOM headless true ",
        "setterDispatch ExactRuntimeClassBeamLinkerBLinker ",
        "syntheticCellConstruction UnsafeExactClassNoGeometry ",
        "syntheticEvidenceScope SetterAndSharedCellOnly ",
        "syntheticGeometryRead false\n",
        "stemsbeamvlinkblinkerflagresult chula.png#1 system 1 plan 143 scope real case - ",
        "terminal ReadyBeforeSiblingBeamLinks applyReturn true supportGrade grade ",
        "supportGradeRead false stemAlias created:143 stemInterId 2340 ",
        "bAlias beam:12:b:0 linked true writeCount 1 valueChangeCount 1 ",
        "stateHashAfter hash linkedCountAfter 1 linkedAliasesAfter [beam:12:b:0] ",
        "linkedSetHashAfter hash linkSiblingsCalled false\n",
        "stemsbeamvlinkblinkerflagpagesummary chula.png#1 systems 3 realTransactions 3 ",
        "syntheticCases 4 totalTransactions 7 realFalseToTrue 3 realIdempotent 0 ",
        "syntheticFalseToTrue 3 syntheticIdempotent 1 realArenaEntries 1 ",
        "frozenEntries 1 dynamicAnchors 0 isolatedExactClassCells 4 ",
        "applyReturnFalseCases 1 stopBeforeSiblingBeamLinks true\n",
        "stemsbeamvlinkblinkerflagcorpussummary schema stems-beam-vlink-b-linker-flag-v1 ",
        "mode chula pages 1 pageRefs chula.png#1 rowCounts rows ",
        "pageInputSha256 input probeSourceSha256 a runnerSourceSha256 b ",
        "effectiveClasspathSha256 cp jdkReleaseSha256 jdk javaExecutableSha256 java ",
        "javaJpegLibrarySha256 jpeg ",
        "javaModulesSha256 modules javaVmLibrarySha256 vm ",
        "javaAwtLibrarySha256 awt javaAwtLwawtLibrarySha256 lwawt ",
        "javaArchitecture aarch64 javaRuntimeVersion 25.0.3+9-LTS ",
        "javaVmVariant Hotspot javaImageType JDK ",
        "beamLinkerClassSha256 beamclass bLinkerClassSha256 bclass ",
        "vLinkerClassSha256 vclass stemLinkerClassSha256 stemclass ",
        "beamLinkerSourceSha256 c ",
        "stemLinkerSourceSha256 d linkSourceSha256 e sigraphSourceSha256 f ",
        "sigListenerSourceSha256 g systemInfoSourceSha256 h sheetSourceSha256 i ",
        "basicIndexSourceSha256 j entityIndexSourceSha256 k glyphIndexSourceSha256 l ",
        "interIndexSourceSha256 m abstractEntitySourceSha256 n abstractInterSourceSha256 o ",
        "interSourceSha256 p stemInterSourceSha256 q abstractChordInterSourceSha256 r ",
        "headChordInterSourceSha256 s relationSourceSha256 t supportSourceSha256 u ",
        "beamStemRelationSourceSha256 v beamRestRelationSourceSha256 w ",
        "chordStemRelationSourceSha256 x abstractBeamInterSourceSha256 y ",
        "beamInterSourceSha256 z beamHookInterSourceSha256 aa sheetStubSourceSha256 ab ",
        "bookSourceSha256 ac horizontalSideSourceSha256 horizontal ",
        "verticalSideSourceSha256 vertical gradleSourceSha256 ad ",
        "jgraphtCoreVersion 1.5.2 ",
        "jgraphtCoreJarSha256 ae schedulerFixtureSha256 af expandFixtureSha256 ag ",
        "createStemFixtureSha256 ah reuseCheckFixtureSha256 ai baseApplyFixtureSha256 aj ",
        "baseApplyManifestSha256 ak predecessorSwapNegative true ",
        "baseEvidenceMeaning OrderedFourRowShaBundleNotTypedState emittedBodySha256 al ",
        "emittedBodyLines 1 emittedBodyBytes 1 freshRunsPerPage 2 ",
        "freshRunsByteIdentical true rawPassSha256 am freshJvmPerSystem true ",
        "compilerJavaProcesses 1 runtimeJavaProcessesPerPass 3 runtimeJavaProcesses 6 ",
        "totalJavaProcesses 7 maximumConcurrentJavaProcesses 1 ",
        "concurrencyScope Boundary15RunnerLockedInvocation compilerJavaProcessReaped true ",
        "runtimeJavaProcessesReaped true foregroundJavaProcessesOnly true ",
        "backgroundJavaProcessesStarted 0 dynamicAllBArenaGuardOnly true ",
        "stopBeforeSiblingBeamLinks true\n",
    );
    let rows = parse_scaffold_fixture(valid).expect("strict scaffold fixture");
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[1].kind, RowKind::Result);
    assert_eq!(rows[1].page, "chula.png#1");
    assert_eq!(
        rows[1].value("terminal").unwrap(),
        "ReadyBeforeSiblingBeamLinks"
    );

    assert!(parse_scaffold_fixture(&valid.replacen(FIXTURE_SCHEMA, "# schema: wrong", 1)).is_err());
    assert!(parse_scaffold_fixture(&valid.replace("systems 3", "systems 3 systems 3")).is_err());
    assert!(parse_scaffold_fixture(&format!("{valid}# unauthenticated\n")).is_err());
    assert!(parse_scaffold_fixture(valid.trim_end()).is_err());
}

#[test]
fn copied_boundary_fourteen_hydration_lane_is_source_pinned_and_rejects_drift() {
    validate_base_apply_gate_source_pin().expect("active boundary-14 gate source pin");
    assert!(
        validate_base_apply_gate_source_sha256(
            "0d9d44f7e7d6f3a5e22325048d8b0fe6965bd7203e3e2b6e285ae16b9586351f"
        )
        .is_err()
    );
}

fn assert_installed_page_is_strictly_replayed(key: &str) {
    let path = repo_root().join(format!(
        "rust/oracle/stems-beam-vlink-b-linker-flag-{key}.txt"
    ));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let replays = validate_fixture(&text)
        .unwrap_or_else(|error| panic!("exact {key} B-linker flag fixture: {error}"));
    validate_base_apply_gate_source_pin().expect("active boundary-14 hydration source pin");

    let page = parse_scaffold_fixture(&text)
        .unwrap_or_else(|error| panic!("strict {key} fixture: {error}"))
        .into_iter()
        .find(|row| row.kind == RowKind::Page)
        .unwrap_or_else(|| panic!("missing {key} page row"));
    assert_eq!(page_key(&page.page).expect("known page key"), key);
    assert_eq!(replays.len(), page.usize("systems").unwrap() + 4);
    let predecessor_paths = predecessor_paths(key);
    let create_text = std::fs::read_to_string(repo_root().join(&predecessor_paths[2].1))
        .expect("active createStem predecessor");
    let reuse_text = std::fs::read_to_string(repo_root().join(&predecessor_paths[3].1))
        .expect("active reuse-check predecessor");
    let base_apply_text = std::fs::read_to_string(repo_root().join(&predecessor_paths[4].1))
        .expect("active base-apply predecessor");
    let image = page_image_name(key).expect("known page image");
    let mut projected_real_systems = 0;
    for replay in replays.iter().filter(|replay| replay.scope == "real") {
        let hydrated = b14_hydration::run_real(
            image,
            replay.system_id,
            &base_apply_text,
            &create_text,
            &reuse_text,
            replay.linked_before,
        )
        .unwrap_or_else(|error| {
            panic!(
                "{} system {} exact predecessor hydration: {error}",
                key, replay.system_id
            )
        });
        assert_public_transaction_matches_replay(
            &hydrated.transaction,
            &hydrated.state_before,
            &hydrated.state_after,
            &hydrated.base_apply,
            replay,
            PublicProjectionExpected {
                target: hydrated.target,
                triggering: hydrated.triggering,
                ordered_observers: &hydrated.ordered_observers,
            },
        )
        .unwrap_or_else(|error| {
            panic!(
                "{} system {} production/independent/Java projection: {error}",
                key, replay.system_id
            )
        });
        projected_real_systems += 1;
    }
    assert_eq!(projected_real_systems, page.usize("systems").unwrap());
}

#[test]
fn installed_eight_page_corpus_is_strictly_replayed() {
    let manifest_path = repo_root().join(FLAG_MANIFEST_PATH);
    let manifest = std::fs::read(&manifest_path)
        .unwrap_or_else(|error| panic!("{}: {error}", manifest_path.display()));
    assert_eq!(sha256_hex(&manifest), FLAG_MANIFEST_SHA256);
    validate_flag_manifest(&manifest).expect("exact boundary-15 manifest/corpus");
    let mut trailing = manifest.clone();
    trailing.extend_from_slice(b"# unauthenticated\n");
    assert!(StrictFlagManifest::parse(&trailing).is_err());
    let manifest_text = String::from_utf8(manifest).expect("manifest is UTF-8");
    let drifted = manifest_text.replacen(
        "corpusReconstruction SharedHeaderOnceThenPageSemanticRows",
        "corpusReconstruction UnauthenticatedReconstruction",
        1,
    );
    assert!(StrictFlagManifest::parse(drifted.as_bytes()).is_ok());
    assert!(validate_flag_manifest(drifted.as_bytes()).is_err());
    for key in CORPUS_PAGES {
        assert_installed_page_is_strictly_replayed(key);
    }
}
