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

use audiveris_image::{beam_structure::Segment, section::Bounds};
use audiveris_omr::{
    head_scanner_slices::JavaRectangle,
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads,
    native_ledgers::recognize_native_ledgers,
    native_sig::{
        NativeSigBounds, NativeSigEdge, NativeSigHeadStemPayload, NativeSigInterKind,
        NativeSigRelationKind, NativeSigRelationOrigin, NativeSigSupport, NativeSigVertex,
        NativeSigVertexId, assemble_native_sig,
    },
    native_stem_seeds::recognize_native_stem_seeds,
    native_stems_beam_builders::{
        NativeStemsBeamBuilder, NativeStemsBeamBuilderItemKind, NativeStemsBeamBuilderTargetRef,
        NativeStemsModeledCanonicalGlyph,
    },
    native_stems_beam_link_plans::NativeStemsBeamLinkPlanAttempt,
    native_stems_beam_scheduler::{
        NativeStemsBeamAwaitingVLinkTransaction, NativeStemsBeamCompletedVLinkEvidence,
        NativeStemsBeamPlanRef, NativeStemsBeamSchedulerEvent, NativeStemsBeamSchedulerPass,
        NativeStemsBeamSchedulerResumeStatus, NativeStemsBeamSchedulerStatus,
        NativeStemsBeamSchedulerStumpsStatus, NativeStemsBeamWorklistSnapshot,
        resume_native_stems_beam_scheduler_after_transaction,
    },
    native_stems_beam_sides::{
        NativeStemsBeamSidesCarrier, NativeStemsBeamSidesContext,
        advance_native_stems_beam_sides_transaction_from_first_stems_bridge,
        advance_native_stems_beam_stumps_transaction_from_first_stems_bridge,
        advance_native_stems_head_single_item_c_link, begin_native_stems_head_linking_phase1,
        continue_native_stems_beam_sides_carrier_into_stumps,
        continue_native_stems_head_linking_phase1,
        drive_native_stems_beam_stumps_from_first_stems_bridge,
        remove_native_stems_beam_competing_hook_and_resume,
    },
    native_stems_beam_stumps::NativeStemsBeamSource,
    native_stems_beam_vlink_b_linker_flag::{
        NativeStemsBeamVLinkBLinkerFlagState,
        apply_native_stems_beam_vlink_b_linker_flag_transaction,
    },
    native_stems_beam_vlink_base_apply::{
        NativeStemsBeamBeamIncidentRead, NativeStemsBeamBeamIncidentRule,
        NativeStemsBeamBeamInterIndexBootstrapEntry, NativeStemsBeamGroupRuntimeState,
        NativeStemsBeamIncidentDirection, NativeStemsBeamIncidentOpposite,
        NativeStemsBeamQueryRelationKind, NativeStemsBeamSheetEditState,
        NativeStemsBeamSigListenerTopology, NativeStemsBeamSigRelationKind,
        NativeStemsBeamVLinkBaseRolloverAuthority, NativeStemsBeamVLinkBeamRuntimeState,
        apply_native_stems_beam_vlink_base_transaction_to_native_sig,
        roll_native_stems_beam_vlink_base_apply_state,
    },
    native_stems_beam_vlink_head_links::{
        NativeStemsBeamHeadLinkBranch,
        apply_native_stems_beam_vlink_head_transaction_to_native_sig,
        initialize_native_stems_beam_s_linker_cells,
    },
    native_stems_beam_vlink_outer_b_linker::apply_native_stems_beam_outer_and_resume_transaction,
    native_stems_beam_vlink_reuse_check::{
        NativeStemsBeamHeadStemLookupEvidence, NativeStemsBeamReuseDisposition,
        NativeStemsBeamReuseEntryObservation, NativeStemsBeamVLinkReuseLiveEvaluation,
        evaluate_native_stems_beam_vlink_reuse_check,
        project_native_stems_beam_vlink_reuse_live_state,
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
        apply_native_stems_beam_vlink_sibling_transaction_to_native_sig,
        initialize_native_stems_beam_b_linker_cells,
    },
    native_stems_beam_vlink_transaction::{
        NativeStemsBeamCreateStemDisposition, NativeStemsBeamCreatedStemGeometry,
        NativeStemsBeamFixedGlyphContent, NativeStemsBeamGlyphRegistrationAction,
        NativeStemsBeamGlyphRegistryBootstrapEntry, NativeStemsBeamKnownSystemStem,
        NativeStemsBeamPersistentIdState, NativeStemsBeamRegistryAuthority,
        NativeStemsBeamStemGrade, NativeStemsBeamSystemStemAuthorityProof,
        NativeStemsBeamSystemStemTransactionState, NativeStemsFirstGlyphFingerprint,
        NativeStemsFirstGlyphIndexBridge, NativeStemsFirstGlyphIndexSnapshot,
        NativeStemsFirstGlyphSnapshotEntry, apply_native_stems_beam_vlink_create_stem_transaction,
        materialize_native_stems_beam_frontier_candidate,
        prepare_native_stems_beam_vlink_frontier_state,
        prepare_native_stems_beam_vlink_frontier_state_from_first_stems_bridge,
    },
    native_stems_beam_vlinkers::{NativeStemsBeamBLinkerRef, NativeStemsBeamVLinkerRef},
    recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds},
    stems_step::{
        NativeBeamPortion, NativeStemHeadSide, NativeStemLine, NativeStemPoint,
        NativeStemVerticalSide,
    },
};

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
const FIRST_STEMS_GLYPH_REGISTRY_SHA256: &str =
    "7311235c38b0667b249749a0e7e6ade278ce92a05ca4200529eb67d73bf1de1c";
const FIRST_STEMS_GLYPH_REGISTRY_LINES: usize = 1_658;
const FIRST_STEMS_GLYPH_REGISTRY_BYTES: usize = 226_926;
const FIRST_STEMS_GLYPH_ACTIVE_SHA256: &str =
    "dae5de3eabc2fb8d19613abcc8b24f4d865bcd55ca1ec6533faae30792692642";
const FIRST_STEMS_GLYPH_ORIGINALS_SHA256: &str =
    "38f4861501e8099dedcb36b0ff9cf615f156ec5b929dffe31c51906a15362af0";
const BEAM_INTER_INDEX_SHA256: &str =
    "fde4daebadc5c7158fa8e83dcbd4ac0ca6381c614876b6fe48408ec2e245064e";
const BEAM_INTER_INDEX_LINES: usize = 52;
const BEAM_INTER_INDEX_BYTES: usize = 6_259;
const EXECUTED_BASE_BEAM_SIG_ORDINALS: [usize; 16] = [
    12, 15, 16, 19, 20, 21, 22, 28, 29, 30, 31, 32, 33, 34, 35, 36,
];

fn first_stems_glyph_bridge(
    registry_text: &str,
    modeled_registry: &[NativeStemsModeledCanonicalGlyph],
    visible_modeled_count: usize,
) -> NativeStemsFirstGlyphIndexBridge {
    assert_eq!(registry_text.len(), FIRST_STEMS_GLYPH_REGISTRY_BYTES);
    assert_eq!(
        registry_text.lines().count(),
        FIRST_STEMS_GLYPH_REGISTRY_LINES
    );
    assert_eq!(
        sha256_hex(registry_text.as_bytes()),
        FIRST_STEMS_GLYPH_REGISTRY_SHA256
    );
    assert!(
        registry_text
            .lines()
            .any(|line| line == "# schema: stems-beam-glyph-registry-v1")
    );
    assert_eq!(visible_modeled_count, 1_058);
    assert!(modeled_registry.len() > visible_modeled_count);

    let page_rows = registry_text
        .lines()
        .filter(|line| line.starts_with("stemsbeamglyphregistrypage "))
        .collect::<Vec<_>>();
    let [page] = page_rows.as_slice() else {
        panic!(
            "first-STEMS registry page-row cardinality: {}",
            page_rows.len()
        );
    };
    let page_tokens = page.split_ascii_whitespace().collect::<Vec<_>>();
    assert_eq!(
        page_tokens,
        [
            "stemsbeamglyphregistrypage",
            "active",
            "1650",
            "originals",
            "1650",
            "entries",
            "1650",
            "activeHash",
            FIRST_STEMS_GLYPH_ACTIVE_SHA256,
            "originalsHash",
            FIRST_STEMS_GLYPH_ORIGINALS_SHA256,
        ]
    );

    let mut visible_by_content = modeled_registry[..visible_modeled_count]
        .iter()
        .map(|glyph| {
            assert!(glyph.modeled_canonical_ordinal < visible_modeled_count);
            (
                format!(
                    "g:{}:{}:{}:{}:{}",
                    glyph.bounds.x,
                    glyph.bounds.y,
                    glyph.bounds.width,
                    glyph.bounds.height,
                    b15_hydration::run_table_digest(&glyph.run_table)
                ),
                glyph.modeled_canonical_ordinal,
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(visible_by_content.len(), visible_modeled_count);

    let mut entries = Vec::new();
    let mut ids = BTreeSet::new();
    let mut fingerprints = BTreeSet::new();
    for line in registry_text
        .lines()
        .filter(|line| line.starts_with("stemsbeamglyphregistryentry "))
    {
        let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
        let [
            "stemsbeamglyphregistryentry",
            "id",
            glyph_id,
            "active",
            "true",
            "content",
            content,
        ] = tokens.as_slice()
        else {
            panic!("malformed first-STEMS registry entry: {line}");
        };
        let glyph_id = glyph_id.parse::<i32>().expect("positive registry glyph ID");
        assert!((1..=2_339).contains(&glyph_id));
        assert!(
            ids.insert(glyph_id),
            "duplicate registry glyph ID {glyph_id}"
        );
        let parts = content.split(':').collect::<Vec<_>>();
        let ["g", x, y, width, height, run_table_sha256] = parts.as_slice() else {
            panic!("malformed first-STEMS glyph fingerprint: {content}");
        };
        let fingerprint = NativeStemsFirstGlyphFingerprint {
            bounds: Bounds {
                x: x.parse().expect("registry glyph x"),
                y: y.parse().expect("registry glyph y"),
                width: width.parse().expect("registry glyph width"),
                height: height.parse().expect("registry glyph height"),
            },
            run_table_sha256: (*run_table_sha256).to_owned(),
        };
        assert!(
            fingerprints.insert((*content).to_owned()),
            "duplicate registry glyph content {content}"
        );
        entries.push(NativeStemsFirstGlyphSnapshotEntry {
            glyph_id,
            active_in_index: true,
            // The page row says all 1,650 entries are live originals, and all
            // 1,650 individual entries are active at this exact baseline.
            live_original: true,
            fingerprint,
            modeled_canonical_ordinal: visible_by_content.remove(*content),
        });
    }
    assert_eq!(entries.len(), 1_650);
    assert!(
        visible_by_content.is_empty(),
        "visible native glyph lacks persistent binding"
    );
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry.modeled_canonical_ordinal.is_some())
            .count(),
        visible_modeled_count
    );
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry.modeled_canonical_ordinal.is_none())
            .count(),
        592
    );

    NativeStemsFirstGlyphIndexBridge::from_snapshot(
        NativeStemsFirstGlyphIndexSnapshot {
            system_id: 1,
            persistent_ids: NativeStemsBeamPersistentIdState {
                sheet_last_id: 2_339,
                glyph_index_last_id: 2_339,
                inter_index_last_id: 2_339,
            },
            union_size: 1_650,
            active_count: 1_650,
            live_original_count: 1_650,
            active_sha256: FIRST_STEMS_GLYPH_ACTIVE_SHA256.to_owned(),
            live_original_sha256: FIRST_STEMS_GLYPH_ORIGINALS_SHA256.to_owned(),
            visible_modeled_count,
            entries,
        },
        modeled_registry,
    )
    .expect("validated one-time first-STEMS GlyphIndex bridge")
}

fn glyph_bootstrap_for_attempt(
    attempt: &NativeStemsBeamLinkPlanAttempt,
    registry_text: &str,
) -> Vec<NativeStemsBeamGlyphRegistryBootstrapEntry> {
    attempt
        .glyphs
        .iter()
        .map(|glyph| {
            let content_key = format!(
                "g:{}:{}:{}:{}:{}",
                glyph.bounds.x,
                glyph.bounds.y,
                glyph.bounds.width,
                glyph.bounds.height,
                b15_hydration::run_table_digest(&glyph.structural_key.run_table)
            );
            let matches = registry_text
                .lines()
                .filter(|line| line.starts_with("stemsbeamglyphregistryentry "))
                .filter_map(|line| {
                    let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
                    let value = |name: &str| {
                        tokens
                            .iter()
                            .position(|token| *token == name)
                            .and_then(|index| tokens.get(index + 1))
                            .copied()
                    };
                    (value("content") == Some(content_key.as_str())).then(|| {
                        (
                            value("id").expect("registry id").parse::<i32>().unwrap(),
                            value("active") == Some("true"),
                        )
                    })
                })
                .collect::<Vec<_>>();
            let [(glyph_id, active_in_index)] = matches.as_slice() else {
                panic!("selected glyph bootstrap cardinality: {}", matches.len());
            };
            NativeStemsBeamGlyphRegistryBootstrapEntry {
                canonical_alias: usize::try_from(*glyph_id).expect("positive glyph ID"),
                glyph_id: *glyph_id,
                content: NativeStemsBeamFixedGlyphContent {
                    bounds: glyph.bounds,
                    weight: glyph.weight,
                    run_table: glyph.structural_key.run_table.clone(),
                },
                active_in_index: *active_in_index,
                strongly_retained: *active_in_index,
            }
        })
        .collect()
}

const BOUNDARY_FIFTEEN_GATE_PATH: &str =
    "rust/crates/audiveris-omr/tests/native_stems_beam_vlink_b_linker_flag.rs";
const BOUNDARY_FIFTEEN_GATE_SHA256: &str =
    "9085f461c143615f47659d3b5f0a760ad9bdfc0098084ef6e376d8a32afe61b6";
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

/// Boundary 16 computes the sibling writes the SIDES pass recorded.
///
/// The resume chain needs, per transaction, which *other* B linkers `linkSiblings` left
/// linked -- that is what retires the 21 sides Java skips, and feeding it from the oracle
/// makes the chain stop exactly where Java stops. This checks the port can produce those
/// writes itself, by running the production Boundary-16 apply and comparing its
/// `sibling_b_linker_cells` against the aliases the full-pass probe recorded for the same
/// transaction.
///
/// Replay-on-frozen, per rust/PORTING.md: the derivation is proven on the transaction that
/// already has frozen evidence before it is trusted on the 31 that do not.
#[test]
fn boundary_sixteen_derives_the_sibling_writes_the_pass_recorded() {
    let text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-vlink-sibling-links-chula.txt"),
    )
    .expect("frozen sibling-links fixture");
    let rows = parse_scaffold_fixture(&text).expect("fixture parses");
    let transactions = validate_core_rows(&rows).expect("core rows");
    let transaction = transactions
        .iter()
        .find(|candidate| candidate.key.system == 1)
        .expect("chula system 1 transaction");
    let hydrated =
        hydrate_real_boundary_sixteen(&rows[0], transaction).expect("boundary 16 applies");

    let sig_of: std::collections::BTreeMap<_, _> = hydrated
        .predecessor
        .stumps
        .beams_by_abscissa
        .iter()
        .map(|beam| (beam.source, beam.sig_ordinal))
        .collect();
    let alias =
        |reference: &audiveris_omr::native_stems_beam_vlinkers::NativeStemsBeamBLinkerRef| {
            format!(
                "beam:{}:b:{}",
                sig_of.get(&reference.beam).expect("sig ordinal"),
                reference.id - 1
            )
        };

    let mut derived: Vec<String> = hydrated
        .state_after
        .sibling_b_linker_cells
        .iter()
        .filter(|cell| cell.linked)
        .map(|cell| alias(&cell.reference))
        .collect();
    derived.sort();
    derived.dedup();

    // What Java's full pass recorded for the same transaction.
    let pass = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-sides-pass-chula-system1.txt"),
    )
    .expect("frozen SIDES pass");
    let first = pass
        .lines()
        .find(|line| {
            line.starts_with("stemsbeamsidesloopsibling ") && line.contains(" transaction 1 ")
        })
        .expect("the pass records a first transaction");
    let mut recorded: Vec<String> = first
        .split(" aliases ")
        .nth(1)
        .map(|list| list.trim().split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    recorded.sort();

    assert!(
        !recorded.is_empty(),
        "the first transaction should write sibling cells; if it stopped doing so the \
         comparison below would prove nothing"
    );
    assert_eq!(
        derived, recorded,
        "Boundary 16's sibling cells differ from what Java's pass recorded for the same \
         transaction"
    );
    println!(
        "boundary 16 derived {} sibling writes, matching Java's pass: {derived:?}",
        derived.len()
    );
}

/// The first measured linked-S branch uses native Allegretto plan topology and
/// the owned SIG/S-cell read path.  The compact Java result is not opened until
/// that read has selected the persistent stem and proved the suffix unread.
#[test]
fn allegretto_transaction_28_linked_s_is_graph_derived_before_oracle_read() {
    let page = b15_hydration::native_predecessor_page("allegretto.png");
    let plans = &page.plans.systems[0];
    let mut plan_ordinal = 0_usize;
    let mut selected = None;
    for builder in &plans.builders {
        for attempt in &builder.attempts {
            if plan_ordinal == 25 {
                selected = Some((builder, attempt));
            }
            plan_ordinal += 1;
        }
    }
    let (builder, attempt) = selected.expect("native Allegretto plan 25");
    assert_eq!(attempt.relations.len(), 2);
    assert_eq!(attempt.stem_profile, 1);
    assert_eq!(builder.start.side, NativeStemVerticalSide::Bottom);
    assert_eq!(
        attempt
            .relations
            .iter()
            .map(|relation| (
                relation.corner.x_ordinal,
                relation.corner.horizontal,
                relation.corner.vertical,
            ))
            .collect::<Vec<_>>(),
        vec![
            (2, NativeStemHeadSide::Right, NativeStemVerticalSide::Top,),
            (3, NativeStemHeadSide::Right, NativeStemVerticalSide::Top,),
        ]
    );

    let mut scheduler = page.scheduler.systems[0].clone();
    let beam = builder.start.b_linker.beam;
    let beam_sig = page.beam_stumps.systems[0]
        .beams_by_abscissa
        .iter()
        .find(|candidate| candidate.source == beam)
        .expect("plan-25 beam in native stump catalogue")
        .sig_ordinal;
    assert_eq!(beam_sig, 25);
    scheduler.status = NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(Box::new(
        NativeStemsBeamAwaitingVLinkTransaction {
            invocation_ordinal: 28,
            snapshot: NativeStemsBeamWorklistSnapshot {
                pass: NativeStemsBeamSchedulerPass::Sides,
                current_index: 0,
                sources: vec![beam],
                current: beam,
                remaining: Vec::new(),
            },
            beam,
            horizontal_side: Some(NativeStemHeadSide::Right),
            b_linker: builder.start.b_linker,
            v_linker: builder.start,
            vertical_side: builder.start.side,
            plan: NativeStemsBeamPlanRef {
                system_id: 1,
                plan_ordinal: 25,
                builder_ordinal: builder.builder_ordinal,
                stem_profile: attempt.stem_profile,
            },
            outcome: attempt.outcome,
            linked_sides_before: Vec::new(),
            retained_beams_before: Vec::new(),
            would_apply_stored_line_delta: None,
        },
    ));

    let assembled = page.sig.as_ref().expect("native Allegretto SIG");
    let mut sig = assembled.systems[0].clone();
    let mut bindings = assembled.bindings[0].clone();
    let stem_vertex = NativeSigVertexId(sig.vertices.len());
    sig.append_vertex(NativeSigVertex {
        ordinal: stem_vertex.0,
        active: true,
        removed: false,
        kind: NativeSigInterKind::Stem,
        shape: Some("STEM".to_owned()),
        grade: 0.5,
        bounds: NativeSigBounds {
            x: 602,
            y: 10,
            width: 3,
            height: 90,
        },
        abnormal: false,
        beam_geometry: None,
    })
    .expect("modeled persistent stem vertex");
    bindings
        .bind_stem(0, stem_vertex)
        .expect("modeled persistent stem binding");

    // Java's selected relation has global identity 229. Preserve every native
    // baseline edge, then represent unrelated predecessor insertions as
    // tombstones so the one live measured HeadStem relation occupies that
    // identity without entering this bounded gate's semantic scan.
    assert!(sig.edges.len() <= 229);
    let tombstone = sig.edges[0];
    while sig.edges.len() < 229 {
        let mut edge = tombstone;
        edge.ordinal = sig.edges.len();
        edge.active = false;
        sig.edges.push(edge);
    }
    let first_corner = attempt.relations[0].corner;
    let head_vertex = bindings.head_vertices[&first_corner.head];
    sig.append_edge(NativeSigEdge {
        ordinal: 229,
        active: true,
        source: head_vertex.0,
        target: stem_vertex.0,
        kind: NativeSigRelationKind::HeadStem,
        origin: NativeSigRelationOrigin::BeamVHeadDraft {
            plan_ordinal: 15,
            map_ordinal: 0,
        },
        support: Some(NativeSigSupport {
            grade: 0.5,
            bar_connection_impacts: None,
        }),
        beam_portion: None,
        stem_extension: None,
        head_stem: Some(NativeSigHeadStemPayload {
            dx: 0.0,
            dy: 0.0,
            head_side: NativeStemHeadSide::Right,
            extension_point: NativeStemPoint { x: 603.0, y: 10.0 },
            consistency: 1.0,
            manual: false,
        }),
    })
    .expect("measured predecessor HeadStem relation");

    let glyph = &attempt.glyphs[0];
    let system_stems = NativeStemsBeamSystemStemTransactionState {
        system_id: 1,
        next_stem_identity: 1,
        known_stems: vec![NativeStemsBeamKnownSystemStem {
            stem_identity: 0,
            glyph_id: 266,
            glyph_content: NativeStemsBeamFixedGlyphContent {
                bounds: glyph.bounds,
                weight: glyph.weight,
                run_table: glyph.structural_key.run_table.clone(),
            },
            inter_id: Some(2227),
            grade: NativeStemsBeamStemGrade::Artificial(0.5),
            geometry: NativeStemsBeamCreatedStemGeometry {
                median: NativeStemLine {
                    start: NativeStemPoint { x: 603.0, y: 10.0 },
                    stop: NativeStemPoint { x: 603.0, y: 100.0 },
                },
                mean_thickness: 3.0,
                ribbon_bounds: JavaRectangle {
                    x: 602,
                    y: 10,
                    width: 3,
                    height: 90,
                },
            },
            sig_attached: true,
            abnormal: false,
        }],
        authority: NativeStemsBeamRegistryAuthority::CompleteSinceEmptyBaseline,
        exhaustive_lookup: None,
    };
    let mut s_cells = initialize_native_stems_beam_s_linker_cells(&page.head_corners.systems[0])
        .expect("native Allegretto S-cell topology");
    let selected_cell = s_cells
        .iter_mut()
        .find(|cell| {
            cell.reference.head.reference == first_corner.head
                && cell.reference.horizontal == first_corner.horizontal
        })
        .expect("plan-25 shared S cell");
    selected_cell.linked = true;

    let sig_before = sig.clone();
    let bindings_before = bindings.clone();
    let cells_before = s_cells.clone();
    let stems_before = system_stems.clone();
    let actual = project_native_stems_beam_vlink_reuse_live_state(
        &sig,
        &bindings,
        &scheduler,
        plans,
        &s_cells,
        &system_stems,
    )
    .expect("graph-derived Allegretto linked-S B13 read");
    assert_eq!(sig, sig_before);
    assert_eq!(bindings, bindings_before);
    assert_eq!(s_cells, cells_before);
    assert_eq!(system_stems, stems_before);
    let NativeStemsBeamVLinkReuseLiveEvaluation::Entries(entries) = &actual.evaluation else {
        panic!("accepted plan must inspect its relations");
    };
    let [first, second] = entries.as_slice() else {
        panic!("plan 25 must retain two relation positions");
    };
    let NativeStemsBeamReuseEntryObservation::Examined {
        s_linker_linked: true,
        head_stem_lookup: NativeStemsBeamHeadStemLookupEvidence::Exhaustive(scan),
    } = &first.observation
    else {
        panic!("first relation must exhaustively scan the linked S cell");
    };
    assert_eq!((scan.scanned_relation_count, scan.edges.len()), (1, 1));
    assert!(matches!(
        second.observation,
        NativeStemsBeamReuseEntryObservation::UnreadAfterSelection
    ));
    let snapshot_hash = scan.provenance_sha256.clone();
    let projection_hash = scan
        .java_projection_sha256(NativeStemHeadSide::Right, &actual.live_sig_stems)
        .expect("Java projection hash");

    // Expected-only rows and their source pins are opened after the native
    // graph read and its read-only/termination properties are established.
    let fixture_path = repo_root().join("rust/oracle/stems-beam-linked-s-allegretto-system1.txt");
    let fixture = std::fs::read_to_string(&fixture_path).expect("bounded linked-S fixture");
    assert_eq!(
        sha256_hex(fixture.as_bytes()),
        "6822c637f104bf0d0b8c2c61384c2d8df1fdfee34bb23518a66e5745da2ebf93"
    );
    let data = fixture
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>();
    assert_eq!(data.len(), 6);
    let entry_rows = data
        .iter()
        .filter(|line| line.starts_with("stemsbeamlinkedsentry "))
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(entry_rows.len(), 2);
    let field = |line: &str, name: &str| {
        let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
        let position = tokens
            .iter()
            .position(|token| *token == name)
            .unwrap_or_else(|| panic!("missing {name} in {line}"));
        tokens[position + 1].to_owned()
    };
    assert_eq!(field(entry_rows[0], "mapOrdinal"), "0");
    assert_eq!(field(entry_rows[0], "cAlias"), "h:2:RIGHT:TOP");
    assert_eq!(field(entry_rows[0], "sLinked"), "true");
    assert_eq!(field(entry_rows[0], "incidentEdges"), "1");
    assert_eq!(field(entry_rows[0], "matchingEdges"), "1");
    assert_eq!(field(entry_rows[0], "distinctSideStems"), "1");
    assert_eq!(field(entry_rows[0], "headSnapshotHash"), snapshot_hash);
    assert_eq!(field(entry_rows[0], "projectionHash"), projection_hash);
    assert_eq!(field(entry_rows[0], "action"), "SelectBreak");
    assert_eq!(field(entry_rows[1], "mapOrdinal"), "1");
    assert_eq!(field(entry_rows[1], "conditionRead"), "false");
    assert_eq!(field(entry_rows[1], "action"), "UnreadAfterBreak");
    let result = data
        .iter()
        .find(|line| line.starts_with("stemsbeamlinkedsresult "))
        .expect("linked-S result row");
    assert_eq!(field(result, "outcome"), "Selected");
    assert_eq!(field(result, "selectedMapOrdinal"), "0");
    assert_eq!(field(result, "entriesRead"), "1");
    assert_eq!(field(result, "unreadFrom"), "1");
    assert_eq!(field(result, "finalStemInterId"), "2227");
    let summary = data.last().expect("linked-S summary");
    assert_eq!(
        field(summary, "probeSourceSha256"),
        sha256_hex(
            &std::fs::read(repo_root().join("rust/oracle/java/StemsBeamSidesLoopProbe.java"))
                .expect("probe source")
        )
    );
    assert_eq!(
        field(summary, "runnerSourceSha256"),
        sha256_hex(
            &std::fs::read(repo_root().join("rust/oracle/java/run-stems-beam-linked-s.sh"))
                .expect("runner source")
        )
    );
    assert_eq!(field(summary, "freshRuns"), "2");
    assert_eq!(field(summary, "freshRunsByteIdentical"), "true");
    assert_eq!(field(summary, "stopBeforeSigAddVertex"), "true");
}

/// Direct atomic gate at the first real Java competing-hook checkpoint.
///
/// This deliberately reconstructs the measured transaction-28 boundary; it
/// does not claim that native code executed Allegretto transactions 1..28.
/// Java's per-transaction sibling writes are input authority for reaching the
/// typed scheduler frontier. The mutation/result fixture stays unopened until
/// the native graph mutation and continuation have returned.
#[test]
fn allegretto_hook_removal_checkpoint_is_atomic_and_reaches_sides_exhaustion() {
    let predecessor_path =
        repo_root().join("rust/oracle/stems-beam-hook-removal-predecessor-allegretto-system1.txt");
    let predecessor_text =
        std::fs::read_to_string(&predecessor_path).expect("hook scheduler predecessor");
    assert_eq!(
        sha256_hex(predecessor_text.as_bytes()),
        "4993f9ac47e6a0dc2a88e9eab739b72703bf0eb27813c9327b9d1c14e9a47b99"
    );
    let predecessor_rows = predecessor_text
        .lines()
        .filter(|line| line.starts_with("stemsbeamsidesloopsibling "))
        .collect::<Vec<_>>();
    assert_eq!(predecessor_rows.len(), 28);

    let base_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-vlink-base-apply-allegretto.txt"),
    )
    .expect("Allegretto B14 predecessor");
    let create_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-create-stem-allegretto.txt"),
    )
    .expect("Allegretto B12 predecessor");
    let reuse_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-vlink-reuse-check-allegretto.txt"),
    )
    .expect("Allegretto B13 predecessor");
    let hydrated = b15_hydration::run_real(
        "allegretto.png",
        1,
        &base_text,
        &create_text,
        &reuse_text,
        false,
    )
    .expect("native Allegretto checkpoint products");
    let sig_of = hydrated
        .stumps
        .beams_by_abscissa
        .iter()
        .map(|beam| (beam.source, beam.sig_ordinal))
        .collect::<BTreeMap<_, _>>();
    let source_of = sig_of
        .iter()
        .map(|(source, ordinal)| (*ordinal, *source))
        .collect::<BTreeMap<_, _>>();
    let alias_of = |reference: NativeStemsBeamBLinkerRef| {
        format!("beam:{}:b:{}", sig_of[&reference.beam], reference.id - 1)
    };
    let reference_of = |alias: &str| {
        let fields = alias.split(':').collect::<Vec<_>>();
        assert_eq!(fields.len(), 4);
        assert_eq!(fields[0], "beam");
        assert_eq!(fields[2], "b");
        NativeStemsBeamBLinkerRef {
            beam: source_of[&fields[1].parse::<usize>().expect("beam ordinal")],
            id: fields[3].parse::<usize>().expect("B ordinal") + 1,
        }
    };

    // Reproduce only the scheduler input checkpoint from Java's recorded
    // transaction results. No mutation-result row has been opened yet.
    let mut scheduler = hydrated.scheduler.clone();
    for (index, row) in predecessor_rows.iter().enumerate() {
        let NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) = &scheduler.status
        else {
            panic!(
                "scheduler stopped before predecessor transaction {}",
                index + 1
            );
        };
        let tokens = row.split_ascii_whitespace().collect::<Vec<_>>();
        let field = |name: &str| {
            let position = tokens
                .iter()
                .position(|token| *token == name)
                .unwrap_or_else(|| panic!("missing {name}: {row}"));
            tokens[position + 1]
        };
        assert_eq!(field("transaction").parse::<usize>().unwrap(), index + 1);
        assert_eq!(field("bAlias"), alias_of(frontier.b_linker));
        assert_eq!(field("ownLinked"), "true");
        let sibling_count = field("siblings").parse::<usize>().unwrap();
        let siblings = row
            .split(" aliases ")
            .nth(1)
            .map(|aliases| aliases.split(',').map(reference_of).collect::<Vec<_>>())
            .unwrap_or_default();
        assert_eq!(siblings.len(), sibling_count);
        let completed = NativeStemsBeamCompletedVLinkEvidence {
            plan: frontier.plan,
            b_linker: frontier.b_linker,
            v_linker: frontier.v_linker,
            outer_b_linked_after: true,
            sibling_linked_b_linkers: siblings,
        };
        scheduler = *resume_native_stems_beam_scheduler_after_transaction(
            &scheduler,
            &hydrated.vlinkers,
            &hydrated.builder,
            &hydrated.plans,
            &completed,
        )
        .expect("real scheduler predecessor write")
        .advanced_system;
    }
    let awaiting = match &scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingHookRemovalTransaction(awaiting) => {
            awaiting.as_ref().clone()
        }
        _ => panic!("28 real Java transaction results did not reach hook removal"),
    };
    assert_eq!(sig_of[&awaiting.beam], 25);
    assert_eq!(sig_of[&awaiting.competing_hook], 24);
    assert_eq!(awaiting.snapshot.current_index, 19);
    // Native keeps more internal prefix events than Java's probe-visible
    // resume counter; both domains are pinned below at their boundary.
    assert_eq!(scheduler.prefix_events.len(), 89);

    // Reconstruct the checkpoint's live hook neighborhood from native initial
    // topology plus the two measured BeamStem incidences created earlier in
    // the carried pass. Unrelated intermediate insertions remain out of scope.
    let assembled = b15_hydration::native_predecessor_page("allegretto.png")
        .sig
        .expect("native Allegretto SIG");
    let mut sig = assembled.systems[0].clone();
    let mut bindings = assembled.bindings[0].clone();
    let hook_vertex = bindings.beam_vertices[&awaiting.competing_hook];
    let initial_hook_incidents = sig
        .incident_edges(hook_vertex.0)
        .expect("initial hook incidents");
    assert_eq!(initial_hook_incidents.len(), 3);
    for kind in [
        NativeSigRelationKind::Containment,
        NativeSigRelationKind::BeamBeam,
        NativeSigRelationKind::Exclusion,
    ] {
        assert_eq!(
            initial_hook_incidents
                .iter()
                .filter(|edge| edge.kind == kind)
                .count(),
            1
        );
    }
    for stem_identity in 0..2 {
        let stem_vertex = NativeSigVertexId(sig.vertices.len());
        sig.append_vertex(NativeSigVertex {
            ordinal: stem_vertex.0,
            active: true,
            removed: false,
            kind: NativeSigInterKind::Stem,
            shape: Some("STEM".to_owned()),
            grade: 0.5,
            bounds: NativeSigBounds {
                x: 603,
                y: 653,
                width: 3,
                height: 96,
            },
            abnormal: false,
            beam_geometry: None,
        })
        .expect("checkpoint stem vertex");
        bindings
            .bind_stem(stem_identity, stem_vertex)
            .expect("checkpoint stem binding");
        sig.append_edge(NativeSigEdge {
            ordinal: sig.edges.len(),
            active: true,
            source: hook_vertex.0,
            target: stem_vertex.0,
            kind: NativeSigRelationKind::BeamStem,
            origin: NativeSigRelationOrigin::BeamVSiblingDraft {
                plan_ordinal: 13 + (2 * stem_identity),
                sibling_ordinal: 0,
            },
            support: Some(NativeSigSupport {
                grade: 0.5,
                bar_connection_impacts: None,
            }),
            beam_portion: Some(if stem_identity == 0 {
                NativeBeamPortion::Right
            } else {
                NativeBeamPortion::Left
            }),
            stem_extension: Some(NativeStemPoint { x: 603.0, y: 653.0 }),
            head_stem: None,
        })
        .expect("checkpoint BeamStem incidence");
    }
    assert_eq!(sig.incident_edges(hook_vertex.0).unwrap().len(), 5);

    let mut b_cells = initialize_native_stems_beam_b_linker_cells(&hydrated.reachability)
        .expect("Allegretto B-cell arena");
    for cell in &mut b_cells {
        cell.linked = scheduler.linked_b_linkers.contains(&cell.reference);
    }
    let s_cells = initialize_native_stems_beam_s_linker_cells(&hydrated.head_corners)
        .expect("Allegretto S-cell arena");
    let context = NativeStemsBeamSidesContext {
        plans: &hydrated.plans,
        builders: &hydrated.builder,
        stumps: &hydrated.stumps,
        vlinkers: &hydrated.vlinkers,
        reachability: &hydrated.reachability,
        head_corners: &hydrated.head_corners,
        checker: &b15_hydration::checker_context_for_page(&b15_hydration::native_predecessor_page(
            "allegretto.png",
        )),
        relation_parameters: hydrated.relation_parameters,
    };
    let mut carrier = NativeStemsBeamSidesCarrier {
        scheduler,
        latest_base_apply: hydrated.state_before.base_apply_state_before.clone(),
        sig,
        bindings,
        b_cells,
        s_cells,
        beam_inter_index: Vec::new(),
        configured_inter_vip_ids: Vec::new(),
    };
    let carrier_before = carrier.clone();
    let before_snapshot = awaiting.snapshot.clone();

    let mut missing_exclusion = carrier.clone();
    let exclusion = missing_exclusion
        .sig
        .incident_edges(hook_vertex.0)
        .unwrap()
        .into_iter()
        .find(|edge| edge.kind == NativeSigRelationKind::Exclusion)
        .expect("full/hook exclusion")
        .ordinal;
    missing_exclusion
        .sig
        .remove_edge(audiveris_omr::native_sig::NativeSigEdgeId(exclusion))
        .unwrap();
    let corrupt_before = missing_exclusion.clone();
    let error = remove_native_stems_beam_competing_hook_and_resume(&mut missing_exclusion, context)
        .expect_err("missing exclusion must reject atomically");
    assert_eq!(error.stage, "hook-removal-incident");
    assert_eq!(missing_exclusion, corrupt_before);

    let actual = remove_native_stems_beam_competing_hook_and_resume(&mut carrier, context)
        .expect("atomic native hook removal and continuation");
    assert_eq!(actual.removed_edges.len(), 5);
    assert_eq!(
        actual.active_vertex_count_before - actual.active_vertex_count_after,
        1
    );
    assert_eq!(
        actual.active_edge_count_before - actual.active_edge_count_after,
        5
    );
    assert_eq!(actual.group_members_before.len(), 3);
    assert_eq!(actual.group_members_after.len(), 2);
    assert_eq!(carrier.latest_base_apply, carrier_before.latest_base_apply);
    assert_eq!(carrier.b_cells, carrier_before.b_cells);
    assert_eq!(carrier.s_cells, carrier_before.s_cells);
    assert_eq!(carrier.beam_inter_index, carrier_before.beam_inter_index);
    assert_eq!(
        carrier.bindings.beam_vertices.len() + 1,
        carrier_before.bindings.beam_vertices.len()
    );
    assert!(
        !carrier
            .bindings
            .beam_vertices
            .contains_key(&awaiting.competing_hook)
    );
    let NativeStemsBeamSchedulerStatus::SidesExhausted {
        final_local_worklist,
        ..
    } = &carrier.scheduler.status
    else {
        panic!("hook removal did not exhaust the remaining SIDES worklist");
    };
    assert_eq!(
        final_local_worklist,
        &before_snapshot.sources[..=before_snapshot.current_index]
    );
    assert_eq!(actual.resume.resume_events.len(), 54);
    assert_eq!(carrier.scheduler.prefix_events.len(), 143);
    assert!(matches!(
        actual.resume.resume_events.first(),
        Some(NativeStemsBeamSchedulerEvent::CompetingHookRemoved {
            event_ordinal: 89,
            ..
        })
    ));

    // Only now read mutation expectations. The scheduler predecessor above is
    // input authority; these rows cannot influence the native return.
    let expected_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-hook-removal-allegretto-system1.txt"),
    )
    .expect("expected-only hook removal fixture");
    assert_eq!(
        sha256_hex(expected_text.as_bytes()),
        "e857a469d2f24b33a8758feff6157731a241fbef93a3f63d7f2c7403e59642b4"
    );
    let expected = expected_text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>();
    assert_eq!(expected.len(), 5);
    let field = |line: &str, name: &str| {
        let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
        let position = tokens
            .iter()
            .position(|token| *token == name)
            .unwrap_or_else(|| panic!("missing {name}: {line}"));
        tokens[position + 1].to_owned()
    };
    let frontier = expected[0];
    let result = expected[1];
    assert_eq!(
        field(frontier, "beamSig").parse::<usize>().unwrap(),
        sig_of[&actual.beam]
    );
    assert_eq!(
        field(frontier, "hookSig").parse::<usize>().unwrap(),
        sig_of[&actual.competing_hook]
    );
    assert_eq!(field(frontier, "workIndex"), "19");
    assert_eq!(field(frontier, "sigVertices"), "202");
    assert_eq!(field(frontier, "sigEdges"), "232");
    assert_eq!(field(result, "sigVertices"), "201");
    assert_eq!(field(result, "sigEdges"), "227");
    assert_eq!(field(result, "hookVertexMatches"), "0");
    assert_eq!(field(result, "hookIncidentAfter"), "0");
    assert_eq!(field(result, "workUnchanged"), "true");
    assert_eq!(field(result, "linkedBUnchanged"), "true");
    assert_eq!(field(result, "terminal"), "RemovedCompetingHook");
    assert_eq!(field(expected[2], "event"), "110");
    assert_eq!(
        field(expected[2], "terminal"),
        "SidesWorklistExhaustedBeforeSecondFrontier"
    );
    assert_eq!(field(expected[3], "sidesExhausted"), "true");
    assert_eq!(
        field(expected[4], "schedulerPredecessorFixtureSha256"),
        sha256_hex(predecessor_text.as_bytes())
    );
    assert_eq!(field(expected[4], "freshRunsByteIdentical"), "true");
}

/// The first measured transaction now derives B16 from the owned SIG and
/// typed products.  Java rows are opened only after the complete native
/// graph/cell result exists.
#[test]
fn native_carrier_drives_full_sides_pass_before_oracle_read() {
    let base_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-vlink-base-apply-chula.txt"),
    )
    .expect("frozen B14 predecessor");
    let create_text =
        std::fs::read_to_string(repo_root().join("rust/oracle/stems-beam-create-stem-chula.txt"))
            .expect("frozen B12 predecessor");
    let reuse_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-vlink-reuse-check-chula.txt"),
    )
    .expect("frozen B13 predecessor");
    let hydrated =
        b15_hydration::run_real("chula.png", 1, &base_text, &create_text, &reuse_text, false)
            .expect("native predecessors through B15");
    let grid = recognize_grid_lines(repo_root().join("data/examples/chula.png"))
        .expect("GRID recognition");
    let headers = recognize_native_headers(&grid).expect("HEADERS recognition");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS recognition");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS recognition");
    let ledgers = recognize_native_ledgers(&grid, &beams).expect("LEDGERS recognition");
    let heads = recognize_native_heads(&grid, &headers, &stem_seeds, &beams, &ledgers)
        .expect("HEADS recognition");
    let assembled = assemble_native_sig(&grid, &headers, &beams, &ledgers, &heads)
        .expect("native SIG assembly");
    let mut sig = assembled
        .systems
        .iter()
        .find(|system| system.system_id == 1)
        .expect("system 1 SIG")
        .clone();
    let mut bindings = assembled
        .bindings
        .iter()
        .find(|bindings| bindings.system_id == 1)
        .expect("system 1 bindings")
        .clone();

    // Freeze the complete native catalogue before opening the disclosed
    // persistent-ID rows. Fixture topology can validate this catalogue, but
    // cannot select a source, vertex, or runtime class for it.
    let native_beam_catalogue = hydrated
        .stumps
        .beams_by_abscissa
        .iter()
        .map(|beam| {
            let vertex_id = bindings
                .beam_vertices
                .get(&beam.source)
                .copied()
                .expect("native beam binding");
            let vertex = sig.vertex(vertex_id.0).expect("live native beam vertex");
            assert!(matches!(
                vertex.kind,
                NativeSigInterKind::Beam
                    | NativeSigInterKind::BeamHook
                    | NativeSigInterKind::SmallBeam
            ));
            (
                beam.sig_ordinal,
                (beam.source, vertex_id.0, vertex.kind.java_class()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(native_beam_catalogue.len(), 48);
    assert_eq!(
        native_beam_catalogue.keys().copied().collect::<Vec<_>>(),
        (0..48).collect::<Vec<_>>()
    );
    assert_eq!(
        native_beam_catalogue
            .values()
            .filter(|(_, _, class)| *class == "BeamInter")
            .count(),
        31
    );
    assert_eq!(
        native_beam_catalogue
            .values()
            .filter(|(_, _, class)| *class == "BeamHookInter")
            .count(),
        17
    );

    let beam_index_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-inter-index-chula-system1.txt"),
    )
    .expect("disclosed sparse beam InterIndex authority");
    assert_eq!(beam_index_text.len(), BEAM_INTER_INDEX_BYTES);
    assert_eq!(beam_index_text.lines().count(), BEAM_INTER_INDEX_LINES);
    assert_eq!(
        sha256_hex(beam_index_text.as_bytes()),
        BEAM_INTER_INDEX_SHA256
    );
    assert!(
        beam_index_text
            .lines()
            .any(|line| line == "# schema: stems-beam-inter-index-v1")
    );
    let mut seen_fixture_ordinals = BTreeSet::new();
    let mut seen_inter_ids = BTreeSet::new();
    let mut seen_index_ordinals = BTreeSet::new();
    let mut beam_bootstrap = Vec::new();
    for line in beam_index_text
        .lines()
        .filter(|line| line.starts_with("stemsbeaminterindexentry "))
    {
        let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
        let [
            "stemsbeaminterindexentry",
            "chula.png#1",
            "system",
            "1",
            "beamSig",
            beam_sig,
            "vertex",
            fixture_vertex,
            "interId",
            inter_id,
            "indexOrdinal",
            index_ordinal,
            "vip",
            "false",
            "class",
            fixture_class,
        ] = tokens.as_slice()
        else {
            panic!("malformed beam InterIndex authority row: {line}");
        };
        let beam_sig = beam_sig.parse::<usize>().expect("beam SIG ordinal");
        let fixture_vertex = fixture_vertex.parse::<usize>().expect("beam vertex");
        let inter_id = inter_id.parse::<i32>().expect("beam Inter ID");
        let index_ordinal = index_ordinal.parse::<usize>().expect("beam index ordinal");
        assert!(seen_fixture_ordinals.insert(beam_sig));
        assert!(seen_inter_ids.insert(inter_id));
        assert!(seen_index_ordinals.insert(index_ordinal));
        let &(source, native_vertex, native_class) = native_beam_catalogue
            .get(&beam_sig)
            .expect("fixture beam exists in prior native catalogue");
        assert_eq!(fixture_vertex, native_vertex);
        assert_eq!(*fixture_class, native_class);
        assert_eq!(index_ordinal, 113 + beam_sig);
        if EXECUTED_BASE_BEAM_SIG_ORDINALS.contains(&beam_sig) {
            beam_bootstrap.push(NativeStemsBeamBeamInterIndexBootstrapEntry {
                source,
                inter_id,
                index_ordinal,
                vip: false,
            });
        }
    }
    assert_eq!(
        seen_fixture_ordinals.into_iter().collect::<Vec<_>>(),
        (0..48).collect::<Vec<_>>()
    );
    assert_eq!(seen_inter_ids.len(), 48);
    assert_eq!(seen_inter_ids.first(), Some(&905));
    assert_eq!(seen_inter_ids.last(), Some(&989));
    assert_eq!(
        seen_index_ordinals.into_iter().collect::<Vec<_>>(),
        (113..161).collect::<Vec<_>>()
    );
    assert_eq!(beam_bootstrap.len(), EXECUTED_BASE_BEAM_SIG_ORDINALS.len());
    assert_eq!(
        beam_bootstrap
            .iter()
            .map(|entry| {
                native_beam_catalogue
                    .iter()
                    .find_map(|(ordinal, (source, _, _))| {
                        (*source == entry.source).then_some(*ordinal)
                    })
                    .expect("sparse beam source in native catalogue")
            })
            .collect::<Vec<_>>(),
        EXECUTED_BASE_BEAM_SIG_ORDINALS
    );
    assert!(beam_index_text.lines().any(|line| {
        line == "stemsbeaminterindexsummary chula.png#1 system 1 interIndexEntries 639 beams 48 rowsSha256 1550d098eadde838a3b66ec0a3e12dfdbe121189829a1718a78e95295622038b"
    }));
    drop(beam_index_text);

    let mut base_state = hydrated.state_before.base_apply_state_before.clone();
    let native_base = apply_native_stems_beam_vlink_base_transaction_to_native_sig(
        &hydrated.scheduler,
        &hydrated.plans,
        &hydrated.stumps,
        &hydrated.vlinkers,
        &hydrated.create_transaction,
        &hydrated.reuse_live_state,
        hydrated.relation_parameters,
        &hydrated.reuse_check,
        &mut base_state,
        &mut sig,
        &mut bindings,
    )
    .expect("native B14 commit");
    // The native graph deliberately uses one-based native vertex identities
    // where the legacy replay certificate retains Java EntityIndex IDs.  Join
    // the complete mutation result in the shared semantic domain instead of
    // pretending those identity domains are interchangeable.
    assert_eq!(native_base.key, hydrated.base_apply.key);
    assert_eq!(native_base.stem_before, hydrated.base_apply.stem_before);
    assert_eq!(native_base.stem_after, hydrated.base_apply.stem_after);
    assert_eq!(
        native_base.fresh_relation,
        hydrated.base_apply.fresh_relation
    );
    assert_eq!(
        native_base.graph_relation_identity,
        hydrated.base_apply.graph_relation_identity
    );
    assert_eq!(
        native_base.apply_disposition,
        hydrated.base_apply.apply_disposition
    );
    assert_eq!(native_base.callback, hydrated.base_apply.callback);
    assert_eq!(native_base.operations, hydrated.base_apply.operations);
    assert_eq!(native_base.outcome, hydrated.base_apply.outcome);
    assert_eq!((sig.vertices.len(), sig.edges.len()), (222, 203));

    let mut cells = initialize_native_stems_beam_b_linker_cells(&hydrated.reachability)
        .expect("complete native B-cell arena");
    let post_b14_sig = sig.clone();
    let pre_b15_cells = cells.clone();
    let actual = apply_native_stems_beam_vlink_sibling_transaction_to_native_sig(
        &mut sig,
        &bindings,
        &hydrated.scheduler,
        &hydrated.stumps,
        &hydrated.vlinkers,
        &hydrated.reachability,
        &hydrated.builder,
        &native_base,
        &hydrated.transaction,
        &mut cells,
    )
    .expect("native B15+B16 carrier commit");

    assert_eq!((sig.vertices.len(), sig.edges.len()), (222, 205));
    assert!(!actual.base_linked_before);
    assert!(actual.base_linked_after);
    assert_eq!(
        actual.graph.appended_edges,
        vec![
            audiveris_omr::native_sig::NativeSigEdgeId(203),
            audiveris_omr::native_sig::NativeSigEdgeId(204)
        ]
    );
    assert_eq!(actual.siblings.len(), 2);
    assert!(
        actual
            .siblings
            .iter()
            .all(|step| step.branch == NativeStemsBeamSiblingBranch::Linked)
    );
    assert_eq!(actual.b_linker_write_count, 3);
    assert_eq!(actual.b_linker_value_change_count, 3);
    assert_eq!(
        sig.incident_edges(221)
            .expect("native stem incidents")
            .iter()
            .map(|edge| edge.ordinal)
            .collect::<Vec<_>>(),
        vec![202, 203, 204]
    );

    // Carry the same owned SIG and B-cell authority through B17 before any
    // Boundary-16 or Boundary-17 oracle row is opened.
    let mut s_cells = initialize_native_stems_beam_s_linker_cells(&hydrated.head_corners)
        .expect("complete native S-cell arena");
    let post_b16_sig = sig.clone();
    let pre_b17_s_cells = s_cells.clone();
    let head_actual = apply_native_stems_beam_vlink_head_transaction_to_native_sig(
        &mut sig,
        &bindings,
        &hydrated.scheduler,
        &hydrated.plans,
        &hydrated.builder,
        &hydrated.head_corners,
        &hydrated.reachability,
        &hydrated.transaction,
        &actual,
        &cells,
        &mut s_cells,
    )
    .expect("native B17 graph/S-cell carrier commit");
    assert_eq!((sig.vertices.len(), sig.edges.len()), (222, 207));
    assert_eq!(head_actual.plan_ordinal, 143);
    assert_eq!(head_actual.steps.len(), 2);
    assert!(
        head_actual
            .steps
            .iter()
            .all(|step| step.branch == NativeStemsBeamHeadLinkBranch::Linked)
    );
    assert_eq!(
        head_actual
            .steps
            .iter()
            .map(|step| (step.head_vertex.0, step.stem_vertex.0))
            .collect::<Vec<_>>(),
        vec![(119, 221), (120, 221)]
    );
    assert_eq!(
        head_actual
            .appended_edges
            .iter()
            .map(|edge| edge.0)
            .collect::<Vec<_>>(),
        vec![205, 206]
    );
    assert_eq!(head_actual.s_linker_write_count, 2);
    assert_eq!(head_actual.s_linker_value_change_count, 2);
    assert_eq!(head_actual.head_abnormal_value_change_count, 2);
    assert_eq!(head_actual.stem_abnormal_value_change_count, 1);
    assert_eq!(head_actual.dirty_cascade_assignment_count, 3);
    assert_eq!((head_actual.last_index, head_actual.max_index), (5, 5));
    assert!(!head_actual.remainder_less_than);
    assert!(head_actual.returned_true);
    assert_eq!(
        head_actual
            .steps
            .iter()
            .map(|step| step.consistency.expect("linked consistency").to_bits())
            .collect::<Vec<_>>(),
        vec![0x3ffc_9249_2492_4925; 2]
    );
    assert_eq!(
        head_actual.steps[0]
            .stem_incident_after
            .as_ref()
            .expect("first stem callback")
            .iter()
            .map(|edge| edge.0)
            .collect::<Vec<_>>(),
        vec![202, 203, 204, 205]
    );
    assert_eq!(
        head_actual.steps[1]
            .stem_incident_after
            .as_ref()
            .expect("second stem callback")
            .iter()
            .map(|edge| edge.0)
            .collect::<Vec<_>>(),
        vec![202, 203, 204, 205, 206]
    );
    assert!(!sig.vertices[119].abnormal);
    assert!(!sig.vertices[120].abnormal);
    assert!(!sig.vertices[221].abnormal);

    let outer_resume = apply_native_stems_beam_outer_and_resume_transaction(
        &hydrated.scheduler,
        &hydrated.vlinkers,
        &hydrated.builder,
        &hydrated.plans,
        &hydrated.reachability,
        &hydrated.transaction,
        &actual,
        &head_actual,
        &mut cells,
    )
    .expect("native B18/B19 carrier resume");
    assert!(outer_resume.outer.linked_before);
    assert!(outer_resume.outer.linked_after);
    assert_eq!(outer_resume.outer.linked_value_change_count, 0);
    let NativeStemsBeamSchedulerResumeStatus::AwaitingVLinkTransaction(second) =
        &outer_resume.resume.status
    else {
        panic!("native B19 did not reach the second frontier");
    };
    assert_eq!(second.plan.plan_ordinal, 152);
    assert_eq!(
        second.horizontal_side,
        Some(audiveris_omr::stems_step::NativeStemHeadSide::Right)
    );
    assert!(
        outer_resume
            .resume
            .advanced_system
            .linked_b_linkers
            .contains(&actual.base_b_linker)
    );
    assert!(actual.assigned_b_linkers.iter().all(|reference| {
        outer_resume
            .resume
            .advanced_system
            .linked_b_linkers
            .contains(reference)
    }));

    // Carry transaction 2 through a graph-derived B14 rollover before any
    // transaction-2 family fixture is opened. The page GlyphIndex bootstrap
    // is the one disclosed authority still outside native recognition.
    let second_scheduler = &outer_resume.resume.advanced_system;
    let NativeStemsBeamSchedulerResumeStatus::AwaitingVLinkTransaction(second) =
        &outer_resume.resume.status
    else {
        unreachable!()
    };
    let second_attempt = hydrated
        .plans
        .builders
        .iter()
        .flat_map(|builder| &builder.attempts)
        .nth(second.plan.plan_ordinal)
        .expect("second plan attempt");
    let registry_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-glyph-registry-chula.txt"),
    )
    .expect("disclosed page GlyphIndex bootstrap");
    let bootstrap = glyph_bootstrap_for_attempt(second_attempt, &registry_text);
    let mut second_transaction_state = native_base.state_after.transaction_state.clone();
    let authority = NativeStemsBeamSystemStemAuthorityProof::from_empty_stems_entry(
        &second_transaction_state.system_stems,
        0,
    )
    .expect("dense carried systemStems");
    prepare_native_stems_beam_vlink_frontier_state(
        second_scheduler,
        &hydrated.plans,
        &mut second_transaction_state,
        &bootstrap,
        authority,
    )
    .expect("native transaction-2 preparation");
    let second_create = apply_native_stems_beam_vlink_create_stem_transaction(
        second_scheduler,
        &hydrated.builder,
        &hydrated.plans,
        &mut second_transaction_state,
        &b15_hydration::checker_context_for_page(&b15_hydration::native_predecessor_page(
            "chula.png",
        )),
    )
    .expect("native transaction-2 B12");
    let second_live = project_native_stems_beam_vlink_reuse_live_state(
        &sig,
        &bindings,
        second_scheduler,
        &hydrated.plans,
        &s_cells,
        &second_transaction_state.system_stems,
    )
    .expect("native transaction-2 B13 live state");
    let second_reuse = evaluate_native_stems_beam_vlink_reuse_check(
        second_scheduler,
        &hydrated.plans,
        &hydrated.stumps,
        &hydrated.vlinkers,
        &second_create,
        &second_transaction_state,
        &second_live,
        hydrated.relation_parameters,
    )
    .expect("native transaction-2 B13");
    let mut second_sig = sig.clone();
    let mut second_bindings = bindings.clone();
    let mut second_base_state = roll_native_stems_beam_vlink_base_apply_state(
        &native_base.state_after,
        &second_transaction_state,
        &second_reuse,
        &second_sig,
        &second_bindings,
        NativeStemsBeamVLinkBaseRolloverAuthority {
            stump_system: &hydrated.stumps,
            beam_inter_index: &beam_bootstrap,
            configured_inter_vip_ids: &[],
        },
    )
    .expect("native transaction-2 B14 rollover");
    assert_eq!(
        (
            second_base_state.inter_index.baseline_entry_count,
            second_base_state.sig.baseline_vertex_count,
            second_base_state.sig.baseline_relation_count,
        ),
        (640, 222, 207)
    );
    let second_base = apply_native_stems_beam_vlink_base_transaction_to_native_sig(
        second_scheduler,
        &hydrated.plans,
        &hydrated.stumps,
        &hydrated.vlinkers,
        &second_create,
        &second_live,
        hydrated.relation_parameters,
        &second_reuse,
        &mut second_base_state,
        &mut second_sig,
        &mut second_bindings,
    )
    .expect("native transaction-2 B14 commit");
    assert_eq!(second_base.key.plan.plan_ordinal, 152);
    assert_eq!(
        (second_sig.vertices.len(), second_sig.edges.len()),
        (223, 208)
    );
    assert_eq!(second_bindings.stem_vertices[&1].0, 222);
    assert_eq!(second_base.graph_relation_identity, Some(207));
    assert_eq!(
        second_base.fresh_relation.grade.to_bits(),
        0x3fee_e2b2_e530_80f0
    );
    assert_eq!(
        second_base.fresh_relation.extension_point.x.to_bits(),
        0x4088_5975_1290_d133
    );
    assert_eq!(
        second_base.fresh_relation.extension_point.y.to_bits(),
        0x407c_b1ed_e751_1cca
    );

    let second_base_state_before = roll_native_stems_beam_vlink_base_apply_state(
        &native_base.state_after,
        &second_transaction_state,
        &second_reuse,
        &sig,
        &bindings,
        NativeStemsBeamVLinkBaseRolloverAuthority {
            stump_system: &hydrated.stumps,
            beam_inter_index: &beam_bootstrap,
            configured_inter_vip_ids: &[],
        },
    )
    .expect("repeatable transaction-2 B14 rollover input");
    let second_target = second.b_linker;
    let second_target_linked = cells
        .iter()
        .find(|cell| cell.reference == second_target)
        .expect("transaction-2 target B cell")
        .linked;
    assert!(!second_target_linked);
    let mut second_flag_state = NativeStemsBeamVLinkBLinkerFlagState {
        system_id: 1,
        base_apply_state_before: second_base_state_before,
        target_b_linker: second_target,
        linked: second_target_linked,
        committed: None,
    };
    let second_flag = apply_native_stems_beam_vlink_b_linker_flag_transaction(
        second_scheduler,
        &hydrated.plans,
        &hydrated.stumps,
        &hydrated.vlinkers,
        &second_create,
        &second_live,
        hydrated.relation_parameters,
        &second_reuse,
        &second_base,
        &mut second_flag_state,
    )
    .expect("native transaction-2 B15");
    let mut second_cells = cells.clone();
    let second_siblings = apply_native_stems_beam_vlink_sibling_transaction_to_native_sig(
        &mut second_sig,
        &second_bindings,
        second_scheduler,
        &hydrated.stumps,
        &hydrated.vlinkers,
        &hydrated.reachability,
        &hydrated.builder,
        &second_base,
        &second_flag,
        &mut second_cells,
    )
    .expect("native transaction-2 B16");
    assert_eq!(
        second_siblings
            .graph
            .appended_edges
            .iter()
            .map(|edge| edge.0)
            .collect::<Vec<_>>(),
        vec![208, 209]
    );
    let mut second_s_cells = s_cells.clone();
    let second_heads = apply_native_stems_beam_vlink_head_transaction_to_native_sig(
        &mut second_sig,
        &second_bindings,
        second_scheduler,
        &hydrated.plans,
        &hydrated.builder,
        &hydrated.head_corners,
        &hydrated.reachability,
        &second_flag,
        &second_siblings,
        &second_cells,
        &mut second_s_cells,
    )
    .expect("native transaction-2 B17");
    assert_eq!(
        (second_sig.vertices.len(), second_sig.edges.len()),
        (223, 212)
    );
    assert_eq!(
        second_heads
            .appended_edges
            .iter()
            .map(|edge| edge.0)
            .collect::<Vec<_>>(),
        vec![210, 211]
    );
    assert_eq!(
        second_heads
            .steps
            .iter()
            .map(|step| (step.head_vertex.0, step.stem_vertex.0))
            .collect::<Vec<_>>(),
        vec![(130, 222), (131, 222)]
    );
    assert_eq!(second_heads.s_linker_write_count, 2);
    assert_eq!(second_heads.s_linker_value_change_count, 2);
    assert_eq!((second_heads.last_index, second_heads.max_index), (4, 4));
    assert!(second_heads.returned_true);
    let second_sig_ordinals = hydrated
        .stumps
        .beams_by_abscissa
        .iter()
        .map(|beam| (beam.source, beam.sig_ordinal))
        .collect::<BTreeMap<_, _>>();
    let second_b_alias = |reference: NativeStemsBeamBLinkerRef| {
        format!(
            "beam:{}:b:{}",
            second_sig_ordinals[&reference.beam],
            reference.id - 1
        )
    };
    assert_eq!(second_b_alias(second_siblings.base_b_linker), "beam:12:b:2");
    assert_eq!(
        second_siblings
            .assigned_b_linkers
            .iter()
            .copied()
            .map(second_b_alias)
            .collect::<Vec<_>>(),
        vec!["beam:2:b:0", "beam:3:b:0"]
    );
    let second_outer_resume = apply_native_stems_beam_outer_and_resume_transaction(
        second_scheduler,
        &hydrated.vlinkers,
        &hydrated.builder,
        &hydrated.plans,
        &hydrated.reachability,
        &second_flag,
        &second_siblings,
        &second_heads,
        &mut second_cells,
    )
    .expect("native transaction-2 B18/B19");
    let NativeStemsBeamSchedulerResumeStatus::AwaitingVLinkTransaction(third) =
        &second_outer_resume.resume.status
    else {
        panic!("transaction 2 did not reach a third frontier");
    };
    assert_eq!(third.plan.plan_ordinal, 618);
    let third_alias = second_b_alias(third.b_linker);
    assert_eq!(third_alias, "beam:22:b:0");
    assert_eq!(third.vertical_side, NativeStemVerticalSide::Top);

    // Transaction 3 is the first changed-base-beam and compound-glyph case.
    // Consume the disclosed first-STEMS page snapshot once, join its persistent
    // identities to the system-1-visible native registry prefix, and drop the
    // raw rows before any carried transaction runs.
    let third_scheduler = &second_outer_resume.resume.advanced_system;
    let checker_page = b15_hydration::native_predecessor_page("chula.png");
    let visible_modeled_count = checker_page.first_system_visible_modeled_count;
    let bridge = first_stems_glyph_bridge(
        &registry_text,
        &checker_page.modeled_canonical_glyphs,
        visible_modeled_count,
    );
    assert!(hydrated.plans.builders.iter().all(|builder| {
        builder
            .attempts
            .iter()
            .flat_map(|attempt| &attempt.glyphs)
            .all(|glyph| glyph.modeled_canonical_ordinal < visible_modeled_count)
    }));
    drop(registry_text);

    // Retain an independently prepared transaction-3 state for the beam-index
    // authority negative below. It uses the same bridge, not a frontier row.
    let mut third_transaction_state = second_base.state_after.transaction_state.clone();
    let third_authority = NativeStemsBeamSystemStemAuthorityProof::from_empty_stems_entry(
        &third_transaction_state.system_stems,
        0,
    )
    .expect("transaction-3 dense carried systemStems");
    prepare_native_stems_beam_vlink_frontier_state_from_first_stems_bridge(
        third_scheduler,
        &hydrated.plans,
        &mut third_transaction_state,
        &bridge,
        third_authority,
    )
    .expect("native transaction-3 bridge preparation");
    let third_candidate = materialize_native_stems_beam_frontier_candidate(
        third_scheduler,
        &hydrated.plans,
        &third_transaction_state,
    )
    .expect("native transaction-3 compound candidate");
    assert!(
        third_transaction_state
            .selected_glyph_bindings
            .iter()
            .any(|selected| {
                selected.content == third_candidate
                    && selected.glyph_id == 298
                    && selected.canonical_alias == 298
            })
    );
    third_transaction_state.glyph_index.exhaustive_lookup = None;

    // The production carrier owns the same transaction as one clone-and-swap
    // operation. All plan-specific glyph selection now comes from the bridge.
    let checker = b15_hydration::checker_context_for_page(&checker_page);
    let carrier_before = NativeStemsBeamSidesCarrier {
        scheduler: (**third_scheduler).clone(),
        latest_base_apply: (*second_base.state_after).clone(),
        sig: second_sig.clone(),
        bindings: second_bindings.clone(),
        b_cells: second_cells.clone(),
        s_cells: second_s_cells.clone(),
        beam_inter_index: beam_bootstrap.clone(),
        configured_inter_vip_ids: Vec::new(),
    };
    let context = NativeStemsBeamSidesContext {
        plans: &hydrated.plans,
        builders: &hydrated.builder,
        stumps: &hydrated.stumps,
        vlinkers: &hydrated.vlinkers,
        reachability: &hydrated.reachability,
        head_corners: &hydrated.head_corners,
        checker: &checker,
        relation_parameters: hydrated.relation_parameters,
    };
    let mut corrupt_bridge_carrier = carrier_before.clone();
    corrupt_bridge_carrier
        .latest_base_apply
        .transaction_state
        .glyph_index
        .union_size -= 1;
    let corrupt_bridge_before = corrupt_bridge_carrier.clone();
    let bridge_before = bridge.clone();
    advance_native_stems_beam_sides_transaction_from_first_stems_bridge(
        &mut corrupt_bridge_carrier,
        context,
        &bridge,
    )
    .expect_err("carried GlyphIndex union mismatch must reject the bridge transaction");
    assert_eq!(corrupt_bridge_carrier, corrupt_bridge_before);
    assert_eq!(bridge, bridge_before);

    // Beam 19 is not selected until transaction 31. A sparse authority can
    // therefore carry the first 30 transactions, but the first missing-ID
    // lookup must reject atomically rather than borrowing another beam's ID.
    let late_source = native_beam_catalogue[&19].0;
    let mut missing_late_carrier = carrier_before.clone();
    missing_late_carrier
        .beam_inter_index
        .retain(|entry| entry.source != late_source);
    assert_eq!(missing_late_carrier.beam_inter_index.len(), 15);
    for transaction_ordinal in 3..31 {
        advance_native_stems_beam_sides_transaction_from_first_stems_bridge(
            &mut missing_late_carrier,
            context,
            &bridge,
        )
        .unwrap_or_else(|error| {
            panic!("sparse authority stopped before transaction {transaction_ordinal}: {error}")
        });
    }
    let audiveris_omr::native_stems_beam_scheduler::NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(
        late_frontier,
    ) = &missing_late_carrier.scheduler.status
    else {
        panic!("sparse authority did not reach the late missing beam");
    };
    assert_eq!(late_frontier.b_linker.beam, late_source);
    let missing_late_before = missing_late_carrier.clone();
    let bridge_before = bridge.clone();
    advance_native_stems_beam_sides_transaction_from_first_stems_bridge(
        &mut missing_late_carrier,
        context,
        &bridge,
    )
    .expect_err("late selected beam without persistent identity must reject");
    assert_eq!(missing_late_carrier, missing_late_before);
    assert_eq!(bridge, bridge_before);

    let mut carrier = carrier_before.clone();
    let carried_third = advance_native_stems_beam_sides_transaction_from_first_stems_bridge(
        &mut carrier,
        context,
        &bridge,
    )
    .expect("atomic native transaction-3 bridge carrier");
    assert_eq!(
        (carrier.sig.vertices.len(), carrier.sig.edges.len()),
        (224, 216)
    );
    assert_eq!(carrier.bindings.stem_vertices[&2].0, 223);
    assert_eq!(carried_third.base.graph_relation_identity, Some(212));
    assert_eq!(
        carried_third
            .siblings
            .graph
            .appended_edges
            .iter()
            .map(|edge| edge.0)
            .collect::<Vec<_>>(),
        vec![213]
    );
    assert_eq!(
        carried_third
            .heads
            .appended_edges
            .iter()
            .map(|edge| edge.0)
            .collect::<Vec<_>>(),
        vec![214, 215]
    );
    assert_eq!(
        carrier
            .latest_base_apply
            .transaction_state
            .system_stems
            .known_stems
            .len(),
        3
    );
    assert_eq!(carried_third.create.registration.glyph_id, 298);
    assert_eq!(
        carried_third.create.registration.action,
        NativeStemsBeamGlyphRegistrationAction::Reused {
            reinserted_into_active_index: false
        }
    );
    assert_eq!(
        carried_third.create.disposition,
        NativeStemsBeamCreateStemDisposition::CreatedChecked { stem_identity: 2 }
    );
    assert_eq!(
        carrier
            .latest_base_apply
            .transaction_state
            .glyph_index
            .union_size,
        1650
    );
    assert_eq!(
        carrier.latest_base_apply.inter_index.baseline_entry_count,
        641
    );
    assert_eq!(
        carrier.latest_base_apply.inter_index.appended_entries.len(),
        1
    );
    assert_eq!(carrier.b_cells.iter().filter(|cell| cell.linked).count(), 8);
    assert_eq!(carrier.s_cells.iter().filter(|cell| cell.linked).count(), 6);
    assert!(
        carrier
            .latest_base_apply
            .transaction_state
            .system_stems
            .known_stems
            .iter()
            .all(|stem| stem.sig_attached
                && stem.abnormal
                    == carrier.sig.vertices[carrier.bindings.stem_vertices[&stem.stem_identity].0]
                        .abnormal)
    );
    let NativeStemsBeamSchedulerResumeStatus::AwaitingVLinkTransaction(carried_fourth) =
        &carried_third.outer_resume.resume.status
    else {
        panic!("atomic transaction 3 did not reach the fourth frontier");
    };
    assert_eq!(carried_fourth.plan.plan_ordinal, 627);

    // Invoke the same production bridge carrier again from committed state.
    let carried_fourth = advance_native_stems_beam_sides_transaction_from_first_stems_bridge(
        &mut carrier,
        context,
        &bridge,
    )
    .expect("second atomic bridge carrier invocation for transaction 4");
    assert_eq!(carried_fourth.flag.key.plan.plan_ordinal, 627);
    assert_eq!(
        (carrier.sig.vertices.len(), carrier.sig.edges.len()),
        (225, 221)
    );
    assert_eq!(carrier.bindings.stem_vertices[&3].0, 224);
    assert_eq!(carried_fourth.base.graph_relation_identity, Some(216));
    assert_eq!(
        carried_fourth
            .siblings
            .graph
            .appended_edges
            .iter()
            .map(|edge| edge.0)
            .collect::<Vec<_>>(),
        vec![217, 218]
    );
    assert_eq!(
        carried_fourth
            .heads
            .appended_edges
            .iter()
            .map(|edge| edge.0)
            .collect::<Vec<_>>(),
        vec![219, 220]
    );
    assert_eq!(
        carrier.b_cells.iter().filter(|cell| cell.linked).count(),
        11
    );
    assert_eq!(carrier.s_cells.iter().filter(|cell| cell.linked).count(), 8);
    let NativeStemsBeamSchedulerResumeStatus::AwaitingVLinkTransaction(carried_fifth) =
        &carried_fourth.outer_resume.resume.status
    else {
        panic!("second atomic carrier invocation did not reach transaction 5");
    };
    assert_eq!(carried_fifth.plan.plan_ordinal, 400);
    assert_eq!(second_b_alias(carried_fifth.b_linker), "beam:16:b:0");
    assert_eq!(carried_fifth.vertical_side, NativeStemVerticalSide::Top);

    let mut repeated = vec![carried_third.clone(), carried_fourth.clone()];
    for transaction_ordinal in 5..=32 {
        let frontier = match &carrier.scheduler.status {
            audiveris_omr::native_stems_beam_scheduler::NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(
                frontier,
            ) => frontier.clone(),
            status => panic!(
                "carrier stopped before transaction {transaction_ordinal}: {status:?}"
            ),
        };
        let transaction = advance_native_stems_beam_sides_transaction_from_first_stems_bridge(
            &mut carrier,
            context,
            &bridge,
        )
        .unwrap_or_else(|error| {
            panic!(
                "transaction {transaction_ordinal} plan {} failed: {error}",
                frontier.plan.plan_ordinal
            )
        });
        repeated.push(transaction);
    }
    assert_eq!(repeated.len(), 30);
    let audiveris_omr::native_stems_beam_scheduler::NativeStemsBeamSchedulerStatus::SidesExhausted {
        retained_for_stumps,
        final_local_worklist,
    } = &carrier.scheduler.status
    else {
        panic!("32 native carrier transactions did not exhaust SIDES");
    };
    assert_eq!(retained_for_stumps, final_local_worklist);
    assert_eq!(
        (carrier.sig.vertices.len(), carrier.sig.edges.len()),
        (253, 331)
    );
    assert_eq!(carrier.bindings.stem_vertices.len(), 32);
    assert_eq!(
        carrier.b_cells.iter().filter(|cell| cell.linked).count(),
        61
    );
    assert_eq!(
        carrier.s_cells.iter().filter(|cell| cell.linked).count(),
        68
    );
    assert!(carrier.b_cells.iter().all(|cell| !cell.closed));
    assert!(carrier.s_cells.iter().all(|cell| !cell.closed));

    // Leave the authenticated SIDES terminal through the production STUMPS
    // continuation before opening the expected prefix. The scheduler's linked
    // set must be a bijective view of the 61 persistent true B cells.
    let sides_terminal_carrier = carrier.clone();
    let mut corrupt_linked_b = sides_terminal_carrier.clone();
    corrupt_linked_b
        .b_cells
        .iter_mut()
        .find(|cell| cell.linked)
        .expect("linked B cell at SIDES terminal")
        .linked = false;
    let corrupt_before = corrupt_linked_b.clone();
    continue_native_stems_beam_sides_carrier_into_stumps(&mut corrupt_linked_b, context)
        .expect_err("linked-B scheduler/cell disagreement must reject atomically");
    assert_eq!(corrupt_linked_b, corrupt_before);

    let stumps_actual = continue_native_stems_beam_sides_carrier_into_stumps(&mut carrier, context)
        .expect("native post-SIDES STUMPS prefix");
    assert_eq!(stumps_actual.stump_events.len(), 2);
    let NativeStemsBeamSchedulerEvent::BeamPassStart {
        snapshot,
        selected_stem_profile,
        ..
    } = &stumps_actual.stump_events[0]
    else {
        panic!("STUMPS prefix must begin with the retained beam");
    };
    assert_eq!(snapshot.pass, NativeStemsBeamSchedulerPass::Stumps);
    assert_eq!(snapshot.current_index, 0);
    assert_eq!(*selected_stem_profile, 3);
    let NativeStemsBeamSchedulerEvent::StumpSkippedStructuralSideGlyph {
        beam,
        b_linker,
        v_linker,
        ..
    } = stumps_actual.stump_events[1]
    else {
        panic!("first stump must take structural-side precedence");
    };
    assert_eq!(beam, snapshot.current);
    assert_eq!(second_b_alias(b_linker), "beam:12:b:0");
    assert_eq!(v_linker.side, NativeStemVerticalSide::Top);
    assert!(
        sides_terminal_carrier
            .scheduler
            .linked_b_linkers
            .contains(&b_linker),
        "the structural-side branch must win while the same B cell is linked"
    );
    assert!(
        sides_terminal_carrier
            .b_cells
            .iter()
            .any(|cell| cell.reference == b_linker && cell.linked),
        "the persistent B-cell authority must agree that the structural stump is linked"
    );
    let NativeStemsBeamSchedulerStumpsStatus::AwaitingVLinkTransaction(stump_frontier) =
        &stumps_actual.status
    else {
        panic!("measured STUMPS prefix must stop at a V-link transaction");
    };
    assert_eq!(
        stump_frontier.snapshot.pass,
        NativeStemsBeamSchedulerPass::Stumps
    );
    assert_eq!(stump_frontier.snapshot.current_index, 0);
    assert_eq!(stump_frontier.beam, snapshot.current);
    assert_eq!(second_b_alias(stump_frontier.b_linker), "beam:12:b:1");
    assert_eq!(stump_frontier.horizontal_side, None);
    assert_eq!(stump_frontier.vertical_side, NativeStemVerticalSide::Top);
    assert_eq!(stump_frontier.plan.plan_ordinal, 147);
    assert_eq!(stump_frontier.plan.stem_profile, 3);
    let stump_attempt = hydrated
        .plans
        .builders
        .iter()
        .flat_map(|builder| builder.attempts.iter())
        .nth(stump_frontier.plan.plan_ordinal)
        .expect("native STUMPS plan 147");
    assert_eq!(stump_attempt.stem_profile, 3);
    assert_eq!(stump_attempt.link_profile, 1);
    assert_eq!(stump_attempt.head_target_count, 2);
    assert_eq!(stump_attempt.expand_last_index, Some(2));
    assert_eq!(stump_attempt.relations.len(), 2);
    assert_eq!(stump_attempt.glyphs.len(), 1);
    assert!(!stump_attempt.stored_theoretical_line_would_mutate);
    assert_eq!(carrier.scheduler, *stumps_actual.advanced_system);
    assert_eq!(
        carrier.latest_base_apply,
        sides_terminal_carrier.latest_base_apply
    );
    assert_eq!(carrier.sig, sides_terminal_carrier.sig);
    assert_eq!(carrier.bindings, sides_terminal_carrier.bindings);
    assert_eq!(carrier.b_cells, sides_terminal_carrier.b_cells);
    assert_eq!(carrier.s_cells, sides_terminal_carrier.s_cells);
    assert_eq!(
        carrier.beam_inter_index,
        sides_terminal_carrier.beam_inter_index
    );

    // Execute the first STUMPS frontier and resume the same retained worklist
    // before either the prefix or transaction expected rows are opened.
    let awaiting_stump = carrier.clone();
    let mut missing_stump_b = awaiting_stump.clone();
    missing_stump_b
        .b_cells
        .retain(|cell| cell.reference != stump_frontier.b_linker);
    let missing_stump_b_before = missing_stump_b.clone();
    let bridge_before = bridge.clone();
    advance_native_stems_beam_stumps_transaction_from_first_stems_bridge(
        &mut missing_stump_b,
        context,
        &bridge,
    )
    .expect_err("missing first-STUMPS B cell must reject atomically");
    assert_eq!(missing_stump_b, missing_stump_b_before);
    assert_eq!(bridge, bridge_before);

    let first_stump = advance_native_stems_beam_stumps_transaction_from_first_stems_bridge(
        &mut carrier,
        context,
        &bridge,
    )
    .expect("native first STUMPS transaction and resume");
    assert_eq!(first_stump.flag.key.plan.plan_ordinal, 147);
    assert_eq!(first_stump.create.registration.glyph_id, 310);
    assert_eq!(
        first_stump.create.registration.action,
        NativeStemsBeamGlyphRegistrationAction::Reused {
            reinserted_into_active_index: false
        }
    );
    assert_eq!(
        first_stump.create.disposition,
        NativeStemsBeamCreateStemDisposition::CreatedChecked { stem_identity: 32 }
    );
    assert_eq!(
        first_stump.reuse.reuse_disposition,
        NativeStemsBeamReuseDisposition::AllUnlinked
    );
    assert_eq!(first_stump.reuse.reuse_trace.len(), 2);
    assert_eq!(first_stump.base.graph_relation_identity, Some(331));
    assert!(first_stump.base.apply_returned);
    assert!(first_stump.flag.linked_after);
    assert!(first_stump.siblings.assigned_b_linkers.is_empty());
    assert_eq!(first_stump.heads.appended_edges.len(), 2);
    assert_eq!(
        (carrier.sig.vertices.len(), carrier.sig.edges.len()),
        (254, 334)
    );
    assert_eq!(carrier.bindings.stem_vertices.len(), 33);
    assert_eq!(
        carrier.b_cells.iter().filter(|cell| cell.linked).count(),
        62
    );
    assert_eq!(
        carrier.s_cells.iter().filter(|cell| cell.linked).count(),
        70
    );
    let NativeStemsBeamSchedulerStumpsStatus::AwaitingVLinkTransaction(next_stump) =
        &first_stump.resume.status
    else {
        panic!("first STUMPS transaction must resume at a second frontier");
    };
    assert_eq!(
        next_stump.snapshot.pass,
        NativeStemsBeamSchedulerPass::Stumps
    );
    assert_eq!(next_stump.snapshot.current_index, 1);
    assert_eq!(second_b_alias(next_stump.b_linker), "beam:22:b:1");
    assert_eq!(next_stump.vertical_side, NativeStemVerticalSide::Top);
    assert_eq!(next_stump.plan.plan_ordinal, 622);
    assert_eq!(carrier.scheduler, *first_stump.resume.advanced_system);
    assert_eq!(bridge, bridge_before);

    let mut missing_second_stump_b = carrier.clone();
    missing_second_stump_b
        .b_cells
        .retain(|cell| cell.reference != next_stump.b_linker);
    let missing_second_stump_b_before = missing_second_stump_b.clone();
    advance_native_stems_beam_stumps_transaction_from_first_stems_bridge(
        &mut missing_second_stump_b,
        context,
        &bridge,
    )
    .expect_err("missing second-STUMPS B cell must reject atomically");
    assert_eq!(missing_second_stump_b, missing_second_stump_b_before);
    assert_eq!(bridge, bridge_before);

    let second_stump = advance_native_stems_beam_stumps_transaction_from_first_stems_bridge(
        &mut carrier,
        context,
        &bridge,
    )
    .expect("native second STUMPS transaction and resume");
    assert_eq!(second_stump.flag.key.plan.plan_ordinal, 622);
    assert_eq!(second_stump.create.registration.glyph_id, 321);
    assert_eq!(
        second_stump.create.registration.action,
        NativeStemsBeamGlyphRegistrationAction::Reused {
            reinserted_into_active_index: false
        }
    );
    assert_eq!(
        second_stump.create.disposition,
        NativeStemsBeamCreateStemDisposition::CreatedChecked { stem_identity: 33 }
    );
    assert_eq!(
        second_stump.reuse.reuse_disposition,
        NativeStemsBeamReuseDisposition::AllUnlinked
    );
    assert_eq!(second_stump.reuse.reuse_trace.len(), 2);
    assert_eq!(second_stump.base.graph_relation_identity, Some(334));
    assert!(second_stump.base.apply_returned);
    assert!(second_stump.flag.linked_after);
    assert!(second_stump.siblings.assigned_b_linkers.is_empty());
    assert_eq!(second_stump.heads.appended_edges.len(), 2);
    assert_eq!(
        (carrier.sig.vertices.len(), carrier.sig.edges.len()),
        (255, 337)
    );
    assert_eq!(carrier.bindings.stem_vertices.len(), 34);
    assert_eq!(
        carrier.b_cells.iter().filter(|cell| cell.linked).count(),
        63
    );
    assert_eq!(
        carrier.s_cells.iter().filter(|cell| cell.linked).count(),
        72
    );
    let NativeStemsBeamSchedulerStumpsStatus::AwaitingVLinkTransaction(third_stump) =
        &second_stump.resume.status
    else {
        panic!("second STUMPS transaction must resume at a third frontier");
    };
    assert_eq!(
        third_stump.snapshot.pass,
        NativeStemsBeamSchedulerPass::Stumps
    );
    assert_eq!(third_stump.snapshot.current_index, 2);
    assert_eq!(second_b_alias(third_stump.b_linker), "beam:16:b:1");
    assert_eq!(third_stump.horizontal_side, None);
    assert_eq!(third_stump.vertical_side, NativeStemVerticalSide::Top);
    assert_eq!(third_stump.plan.plan_ordinal, 404);
    assert_eq!(third_stump.plan.stem_profile, 3);
    let third_stump_attempt = hydrated
        .plans
        .builders
        .iter()
        .flat_map(|builder| builder.attempts.iter())
        .nth(third_stump.plan.plan_ordinal)
        .expect("native STUMPS plan 404");
    assert_eq!(third_stump_attempt.glyphs.len(), 2);
    assert_eq!(carrier.scheduler, *second_stump.resume.advanced_system);
    assert_eq!(bridge, bridge_before);

    let mut missing_third_stump_b = carrier.clone();
    missing_third_stump_b
        .b_cells
        .retain(|cell| cell.reference != third_stump.b_linker);
    let missing_third_stump_b_before = missing_third_stump_b.clone();
    advance_native_stems_beam_stumps_transaction_from_first_stems_bridge(
        &mut missing_third_stump_b,
        context,
        &bridge,
    )
    .expect_err("missing third-STUMPS B cell must reject atomically");
    assert_eq!(missing_third_stump_b, missing_third_stump_b_before);
    assert_eq!(bridge, bridge_before);

    let third_stump_transaction =
        advance_native_stems_beam_stumps_transaction_from_first_stems_bridge(
            &mut carrier,
            context,
            &bridge,
        )
        .expect("native third STUMPS transaction and resume");
    assert_eq!(third_stump_transaction.flag.key.plan.plan_ordinal, 404);
    assert_eq!(third_stump_transaction.create.registration.glyph_id, 303);
    assert_eq!(
        third_stump_transaction.create.registration.action,
        NativeStemsBeamGlyphRegistrationAction::Reused {
            reinserted_into_active_index: false
        }
    );
    assert_eq!(
        third_stump_transaction.create.disposition,
        NativeStemsBeamCreateStemDisposition::CreatedChecked { stem_identity: 34 }
    );
    assert_eq!(
        third_stump_transaction.reuse.reuse_disposition,
        NativeStemsBeamReuseDisposition::AllUnlinked
    );
    assert_eq!(third_stump_transaction.reuse.reuse_trace.len(), 2);
    assert_eq!(
        third_stump_transaction.base.graph_relation_identity,
        Some(337)
    );
    assert!(third_stump_transaction.base.apply_returned);
    assert!(third_stump_transaction.flag.linked_after);
    assert!(
        third_stump_transaction
            .siblings
            .assigned_b_linkers
            .is_empty()
    );
    assert_eq!(third_stump_transaction.heads.appended_edges.len(), 2);
    assert_eq!(
        (carrier.sig.vertices.len(), carrier.sig.edges.len()),
        (256, 340)
    );
    assert_eq!(carrier.bindings.stem_vertices.len(), 35);
    assert_eq!(
        carrier.b_cells.iter().filter(|cell| cell.linked).count(),
        64
    );
    assert_eq!(
        carrier.s_cells.iter().filter(|cell| cell.linked).count(),
        74
    );
    let NativeStemsBeamSchedulerStumpsStatus::AwaitingVLinkTransaction(fourth_stump) =
        &third_stump_transaction.resume.status
    else {
        panic!("third STUMPS transaction must resume at a fourth frontier");
    };
    assert_eq!(
        fourth_stump.snapshot.pass,
        NativeStemsBeamSchedulerPass::Stumps
    );
    assert_eq!(fourth_stump.snapshot.current_index, 3);
    assert_eq!(second_b_alias(fourth_stump.b_linker), "beam:28:b:1");
    assert_eq!(fourth_stump.horizontal_side, None);
    assert_eq!(fourth_stump.vertical_side, NativeStemVerticalSide::Top);
    assert_eq!(fourth_stump.plan.plan_ordinal, 508);
    assert_eq!(fourth_stump.plan.stem_profile, 3);
    assert_eq!(
        carrier.scheduler,
        *third_stump_transaction.resume.advanced_system
    );
    assert_eq!(bridge, bridge_before);

    let mut zero_limit_carrier = carrier.clone();
    let zero_limit_before = zero_limit_carrier.clone();
    drive_native_stems_beam_stumps_from_first_stems_bridge(
        &mut zero_limit_carrier,
        context,
        &bridge,
        0,
    )
    .expect_err("zero STUMPS drive limit must reject atomically");
    assert_eq!(zero_limit_carrier, zero_limit_before);

    let mut late_missing_b_carrier = carrier.clone();
    late_missing_b_carrier
        .b_cells
        .retain(|cell| second_b_alias(cell.reference) != "beam:32:b:1");
    let late_missing_b_before = late_missing_b_carrier.clone();
    drive_native_stems_beam_stumps_from_first_stems_bridge(
        &mut late_missing_b_carrier,
        context,
        &bridge,
        100,
    )
    .expect_err("late missing STUMPS B cell must roll back earlier shadow transactions");
    assert_eq!(late_missing_b_carrier, late_missing_b_before);

    let mut bounded_stumps_carrier = carrier.clone();
    let bounded_stumps = drive_native_stems_beam_stumps_from_first_stems_bridge(
        &mut bounded_stumps_carrier,
        context,
        &bridge,
        1,
    )
    .expect("one-transaction STUMPS drive");
    assert_eq!(bounded_stumps.transactions.len(), 1);
    assert_eq!(
        bounded_stumps.transactions[0].flag.key.plan.plan_ordinal,
        508
    );
    let NativeStemsBeamSchedulerStumpsStatus::AwaitingVLinkTransaction(bounded_next) =
        &bounded_stumps.status
    else {
        panic!("bounded STUMPS drive must stop at its next frontier");
    };
    assert_eq!(bounded_next.plan.plan_ordinal, 28);
    assert_eq!(bounded_next.snapshot.current_index, 4);
    assert_eq!(second_b_alias(bounded_next.b_linker), "beam:29:b:1");
    assert_eq!(
        (
            bounded_stumps_carrier.sig.vertices.len(),
            bounded_stumps_carrier.sig.edges.len()
        ),
        (257, 343)
    );

    let complete_stumps =
        drive_native_stems_beam_stumps_from_first_stems_bridge(&mut carrier, context, &bridge, 100)
            .expect("native STUMPS suffix through completion");
    assert_eq!(complete_stumps.transactions.len(), 4);
    assert_eq!(
        complete_stumps
            .transactions
            .iter()
            .map(|transaction| transaction.flag.key.plan.plan_ordinal)
            .collect::<Vec<_>>(),
        [508, 28, 330, 251]
    );
    assert_eq!(
        complete_stumps
            .transactions
            .iter()
            .map(|transaction| transaction.create.registration.glyph_id)
            .collect::<Vec<_>>(),
        [308, 305, 302, 300]
    );
    assert!(complete_stumps.transactions.iter().all(|transaction| {
        transaction.create.registration.action
            == NativeStemsBeamGlyphRegistrationAction::Reused {
                reinserted_into_active_index: false,
            }
    }));
    assert_eq!(
        complete_stumps
            .transactions
            .iter()
            .map(|transaction| transaction.create.disposition)
            .collect::<Vec<_>>(),
        (35..=38)
            .map(
                |stem_identity| NativeStemsBeamCreateStemDisposition::CreatedChecked {
                    stem_identity,
                }
            )
            .collect::<Vec<_>>()
    );
    assert!(complete_stumps.transactions.iter().all(|transaction| {
        transaction.reuse.reuse_disposition == NativeStemsBeamReuseDisposition::AllUnlinked
            && transaction.flag.linked_after
            && transaction.siblings.assigned_b_linkers.is_empty()
    }));
    assert_eq!(
        complete_stumps
            .transactions
            .iter()
            .map(|transaction| transaction.base.graph_relation_identity)
            .collect::<Vec<_>>(),
        [Some(340), Some(343), Some(346), Some(350)]
    );
    assert_eq!(
        complete_stumps
            .transactions
            .iter()
            .map(|transaction| transaction.heads.appended_edges.len())
            .collect::<Vec<_>>(),
        [2, 2, 3, 2]
    );
    assert!(matches!(
        complete_stumps.status,
        NativeStemsBeamSchedulerStumpsStatus::Completed { .. }
    ));
    assert_eq!(
        (carrier.sig.vertices.len(), carrier.sig.edges.len()),
        (260, 353)
    );
    assert_eq!(carrier.bindings.stem_vertices.len(), 39);
    assert_eq!(
        carrier.b_cells.iter().filter(|cell| cell.linked).count(),
        68
    );
    assert_eq!(
        carrier.s_cells.iter().filter(|cell| cell.linked).count(),
        83
    );
    assert_eq!(bridge, bridge_before);

    // Transfer the complete native-owned post-STUMPS state into the first
    // head-phase decision before opening any head-phase expected row.
    let post_stumps_before = carrier.clone();
    let mut head_phase = begin_native_stems_head_linking_phase1(
        &carrier,
        &checker_page.head_corners.systems[0],
        &checker_page.head_builders.systems[0],
        &hydrated.plans,
    )
    .expect("native post-STUMPS head-phase frontier");
    assert_eq!(carrier, post_stumps_before);
    assert_eq!(head_phase.beam_state, carrier);
    assert_eq!(head_phase.heads.len(), 102);
    assert_eq!(head_phase.current_index, 0);
    assert!(head_phase.unlinked_heads.is_empty());
    assert!(head_phase.undefined_sides.is_empty());
    assert_eq!(head_phase.frontier.head, head_phase.heads[0].reference);
    assert_eq!(head_phase.frontier.stem_profile, 0);
    assert_eq!(head_phase.frontier.link_profile, 1);
    assert!(!head_phase.frontier.append);
    assert_eq!(head_phase.frontier.side_decisions.len(), 2);
    assert_eq!(
        head_phase
            .frontier
            .side_decisions
            .iter()
            .map(|decision| {
                (
                    decision.side,
                    decision.linked_before,
                    decision.closed_before,
                    decision.top_can_link,
                    decision.bottom_can_link,
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (
                NativeStemHeadSide::Left,
                false,
                false,
                Some(false),
                Some(false)
            ),
            (
                NativeStemHeadSide::Right,
                false,
                false,
                Some(true),
                Some(false)
            ),
        ]
    );

    // Execute the selected C linker atomically before opening any head-phase
    // expected row. A late glyph-authority corruption must reject without
    // leaking any graph, registry, allocator, or shared-cell mutation.
    let head_phase_entry = head_phase.clone();
    let head_transaction = advance_native_stems_head_single_item_c_link(
        &mut head_phase,
        &checker_page.head_corners.systems[0],
        &checker_page.head_reachability.systems[0],
        &checker_page.stem_seeds.systems[0],
        &checker_page.head_builders.systems[0],
        &hydrated.plans,
        &checker,
        &bridge,
    )
    .expect("native first-head C-link transaction");
    assert_eq!(head_transaction.corner.x_ordinal, 38);
    assert_eq!(head_transaction.corner.sig_ordinal, 45);
    assert_eq!(
        head_transaction.corner.horizontal,
        NativeStemHeadSide::Right
    );
    assert_eq!(
        head_transaction.corner.vertical,
        NativeStemVerticalSide::Top
    );
    assert_eq!(
        (head_transaction.last_index, head_transaction.max_index),
        (0, 0)
    );
    assert!(head_transaction.relation.accepted);
    assert_eq!(
        head_transaction.relation.derived_horizontal,
        NativeStemHeadSide::Right
    );
    assert_eq!(
        head_transaction.relation.grade.to_bits(),
        0x3fee_3eb4_ae84_ca9d
    );
    assert_eq!(
        head_transaction.relation.dx.to_bits(),
        0xbfa5_d942_375d_430c
    );
    assert_eq!(head_transaction.relation.dy.to_bits(), 0);
    assert_eq!(
        head_transaction
            .relation
            .extension_point
            .expect("accepted head relation extension")
            .x
            .to_bits(),
        0x4091_d5d6_e666_8034
    );
    assert_eq!(head_transaction.create.registration.glyph_id, 307);
    assert_eq!(
        head_transaction.create.registration.action,
        NativeStemsBeamGlyphRegistrationAction::Reused {
            reinserted_into_active_index: false
        }
    );
    let stem_identity = match head_transaction.create.disposition {
        NativeStemsBeamCreateStemDisposition::CreatedChecked { stem_identity } => stem_identity,
        ref other => panic!("first head created unexpected stem: {other:?}"),
    };
    let created = head_transaction
        .create
        .stem
        .as_ref()
        .expect("checked first-head stem");
    let NativeStemsBeamStemGrade::Checked(created_check) = &created.grade else {
        panic!("first head stem is not classifier-checked");
    };
    assert_eq!(created_check.grade.to_bits(), 0x3fe9_3554_3bd3_1399);
    assert_eq!(
        created.geometry.ribbon_bounds,
        JavaRectangle {
            x: 1140,
            y: 319,
            width: 4,
            height: 92,
        }
    );
    assert!(!created.abnormal);
    assert_eq!(head_transaction.stem_vertex.0, 260);
    assert_eq!(head_transaction.head_stem_edge.0, 353);
    assert_eq!(head_transaction.s_linker.head.x_ordinal, 38);
    assert_eq!(
        head_transaction.s_linker.horizontal,
        NativeStemHeadSide::Right
    );
    assert!(!head_transaction.s_linked_before);
    assert!(head_transaction.s_linked_after);
    assert_eq!(head_transaction.closed_cell_changes, 0);
    assert_eq!(head_phase.current_index, 1);
    assert!(head_phase.frontier_consumed);
    assert_eq!(
        (
            head_phase.beam_state.sig.vertices.len(),
            head_phase.beam_state.sig.edges.len()
        ),
        (261, 354)
    );
    assert_eq!(head_phase.beam_state.bindings.stem_vertices.len(), 40);
    assert_eq!(
        head_phase
            .beam_state
            .s_cells
            .iter()
            .filter(|cell| cell.linked)
            .count(),
        84
    );
    let queued_right = head_phase.heads[0]
        .sides
        .iter()
        .find(|cell| cell.reference.horizontal == NativeStemHeadSide::Right)
        .expect("queued first-head RIGHT S cell");
    assert!(queued_right.linked);
    assert!(!queued_right.closed);
    let final_stem = head_phase
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems
        .iter()
        .find(|stem| stem.stem_identity == stem_identity)
        .expect("attached first-head stem in persistent systemStems");
    assert_eq!(final_stem.inter_id, Some(2379));
    assert!(final_stem.sig_attached);

    // Boundary 29: carry the successful first-head mutation through Java's next
    // two prelinked-success queue entries and their shared-stem closure writes.
    let head_after_first = head_phase.clone();
    let second_continuation = continue_native_stems_head_linking_phase1(
        &head_phase,
        &checker_page.head_corners.systems[0],
        &checker_page.head_builders.systems[0],
        &hydrated.plans,
    )
    .expect("native second-head phase-1 continuation");
    assert_eq!(head_phase, head_after_first);
    assert_eq!(second_continuation.processed_head.sig_ordinal, 23);
    assert_eq!(second_continuation.returned_linked, Some(true));
    assert_eq!(second_continuation.closed_value_changes, 2);
    assert_eq!(
        second_continuation
            .closed_s_linkers
            .iter()
            .map(|cell| (cell.head.x_ordinal, cell.horizontal))
            .collect::<Vec<_>>(),
        vec![
            (89, NativeStemHeadSide::Left),
            (89, NativeStemHeadSide::Right),
        ]
    );
    let second_head_phase = (*second_continuation.state_after).clone();
    assert_eq!(second_head_phase.current_index, 2);
    assert!(second_head_phase.frontier_consumed);
    assert!(second_head_phase.unlinked_heads.is_empty());
    assert_eq!(
        second_head_phase.heads[1]
            .sides
            .iter()
            .map(|cell| { (cell.reference.horizontal, cell.linked, cell.closed,) })
            .collect::<Vec<_>>(),
        vec![
            (NativeStemHeadSide::Left, true, false),
            (NativeStemHeadSide::Right, false, false),
        ]
    );
    let closed_x89 = second_head_phase
        .heads
        .iter()
        .find(|head| head.reference.x_ordinal == 89)
        .expect("head sharing second head's prelinked stem");
    assert!(closed_x89.sides.iter().all(|cell| cell.closed));

    let third_continuation = continue_native_stems_head_linking_phase1(
        &second_head_phase,
        &checker_page.head_corners.systems[0],
        &checker_page.head_builders.systems[0],
        &hydrated.plans,
    )
    .expect("native third-head phase-1 continuation");
    assert_eq!(third_continuation.processed_head.sig_ordinal, 33);
    assert_eq!(third_continuation.returned_linked, Some(true));
    assert_eq!(third_continuation.closed_value_changes, 4);
    assert_eq!(
        third_continuation
            .closed_s_linkers
            .iter()
            .map(|cell| (cell.head.x_ordinal, cell.horizontal))
            .collect::<Vec<_>>(),
        vec![
            (79, NativeStemHeadSide::Left),
            (79, NativeStemHeadSide::Right),
            (80, NativeStemHeadSide::Left),
            (80, NativeStemHeadSide::Right),
        ]
    );
    let third_head_phase = (*third_continuation.state_after).clone();
    assert_eq!(third_head_phase.current_index, 3);
    assert!(third_head_phase.frontier_consumed);
    assert!(third_head_phase.unlinked_heads.is_empty());
    assert_eq!(third_head_phase.heads[3].reference.x_ordinal, 20);
    assert_eq!(third_head_phase.heads[3].reference.sig_ordinal, 65);

    let fourth_continuation = continue_native_stems_head_linking_phase1(
        &third_head_phase,
        &checker_page.head_corners.systems[0],
        &checker_page.head_builders.systems[0],
        &hydrated.plans,
    )
    .expect("native fourth-head phase-1 continuation");
    assert_eq!(fourth_continuation.processed_head.sig_ordinal, 65);
    assert_eq!(fourth_continuation.returned_linked, Some(true));
    assert_eq!(fourth_continuation.closed_value_changes, 2);
    assert_eq!(
        fourth_continuation
            .closed_s_linkers
            .iter()
            .map(|cell| (cell.head.x_ordinal, cell.horizontal))
            .collect::<Vec<_>>(),
        vec![
            (19, NativeStemHeadSide::Left),
            (19, NativeStemHeadSide::Right)
        ]
    );
    let fourth_head_phase = (*fourth_continuation.state_after).clone();
    assert_eq!(fourth_head_phase.current_index, 4);
    assert!(fourth_head_phase.frontier_consumed);
    assert!(fourth_head_phase.unlinked_heads.is_empty());
    assert_eq!(fourth_head_phase.heads[4].reference.x_ordinal, 36);
    assert_eq!(fourth_head_phase.heads[4].reference.sig_ordinal, 69);

    // Boundary 31: carry the next prelinked-success queue entry.  This is
    // intentionally still the unchanged phase-1 continuation path: head x36
    // closes both S cells of its shared Stem 2369 and stops before head x99.
    let fifth_continuation = continue_native_stems_head_linking_phase1(
        &fourth_head_phase,
        &checker_page.head_corners.systems[0],
        &checker_page.head_builders.systems[0],
        &hydrated.plans,
    )
    .expect("native fifth-head phase-1 continuation");
    assert_eq!(fifth_continuation.processed_head.sig_ordinal, 69);
    assert_eq!(fifth_continuation.returned_linked, Some(true));
    assert_eq!(fifth_continuation.closed_value_changes, 2);
    assert_eq!(
        fifth_continuation
            .closed_s_linkers
            .iter()
            .map(|cell| (cell.head.x_ordinal, cell.horizontal))
            .collect::<Vec<_>>(),
        vec![
            (35, NativeStemHeadSide::Left),
            (35, NativeStemHeadSide::Right)
        ]
    );
    let fifth_head_phase = (*fifth_continuation.state_after).clone();
    assert_eq!(fifth_head_phase.current_index, 5);
    assert!(fifth_head_phase.frontier_consumed);
    assert!(fifth_head_phase.unlinked_heads.is_empty());
    assert_eq!(fifth_head_phase.heads[5].reference.x_ordinal, 99);
    assert_eq!(fifth_head_phase.heads[5].reference.sig_ordinal, 61);

    let mut corrupt_closure = head_phase.clone();
    let x89_ref = corrupt_closure
        .heads
        .iter()
        .find(|head| head.reference.x_ordinal == 89)
        .expect("closure target x89")
        .reference
        .reference;
    corrupt_closure
        .beam_state
        .bindings
        .head_vertices
        .remove(&x89_ref);
    let corrupt_closure_before = corrupt_closure.clone();
    assert!(
        continue_native_stems_head_linking_phase1(
            &corrupt_closure,
            &checker_page.head_corners.systems[0],
            &checker_page.head_builders.systems[0],
            &hydrated.plans,
        )
        .is_err()
    );
    assert_eq!(corrupt_closure, corrupt_closure_before);
    let mut invalid_second_head = head_phase.clone();
    invalid_second_head.current_index = 0;
    let invalid_second_head_before = invalid_second_head.clone();
    assert!(
        continue_native_stems_head_linking_phase1(
            &invalid_second_head,
            &checker_page.head_corners.systems[0],
            &checker_page.head_builders.systems[0],
            &hydrated.plans,
        )
        .is_err()
    );
    assert_eq!(invalid_second_head, invalid_second_head_before);

    let mut corrupted = head_phase_entry.clone();
    let mut corrupt_canonical = head_phase
        .beam_state
        .latest_base_apply
        .transaction_state
        .glyph_index
        .known_canonical_glyphs
        .iter()
        .find(|glyph| glyph.glyph_id == head_transaction.selected_glyph_id)
        .expect("selected head seed was promoted into carried canonical state")
        .clone();
    corrupt_canonical.glyph_id += 1;
    corrupted
        .beam_state
        .latest_base_apply
        .transaction_state
        .glyph_index
        .known_canonical_glyphs
        .push(corrupt_canonical);
    let corrupted_before = corrupted.clone();
    assert!(
        advance_native_stems_head_single_item_c_link(
            &mut corrupted,
            &checker_page.head_corners.systems[0],
            &checker_page.head_reachability.systems[0],
            &checker_page.stem_seeds.systems[0],
            &checker_page.head_builders.systems[0],
            &hydrated.plans,
            &checker,
            &bridge,
        )
        .is_err()
    );
    assert_eq!(corrupted, corrupted_before);

    // Expected-only STUMPS rows are opened after the production continuation
    // and its atomic/coherence guards have returned.
    let stumps_prefix_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-stumps-prefix-chula-system1.txt"),
    )
    .expect("expected-only STUMPS prefix");
    assert_eq!(
        sha256_hex(stumps_prefix_text.as_bytes()),
        "b1c43f29ee909643707033f79abc166e90da72368ac248d9dae752c764da0dfb"
    );
    let stumps_rows = stumps_prefix_text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>();
    assert_eq!(stumps_rows.len(), 6);
    assert!(stumps_rows[0].starts_with(
        "stemsbeamstumpsprefixbaseline chula.png#1 system 1 retained 34 work [beam:12,"
    ));
    assert!(stumps_rows[0].contains(" linkedB 61 "));
    let between = |row: &str, start: &str, end: &str| {
        row.split_once(start)
            .and_then(|(_, suffix)| suffix.split_once(end).map(|(value, _)| value))
            .expect("strict STUMPS baseline field")
            .to_owned()
    };
    let expected_work_field = between(stumps_rows[0], " work [", "] linkedB ");
    let expected_work = expected_work_field
        .split(',')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let actual_work = snapshot
        .sources
        .iter()
        .map(|source| {
            let ordinal = hydrated
                .stumps
                .beams_by_abscissa
                .iter()
                .find(|beam| beam.source == *source)
                .expect("retained STUMPS beam in native stump catalogue")
                .sig_ordinal;
            format!("beam:{ordinal}")
        })
        .collect::<Vec<_>>();
    assert_eq!(actual_work, expected_work);
    let expected_linked_field = between(stumps_rows[0], " linkedAliases [", "] sigVertices ");
    let expected_linked = expected_linked_field
        .split(',')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut actual_linked = sides_terminal_carrier
        .scheduler
        .linked_b_linkers
        .iter()
        .copied()
        .map(second_b_alias)
        .collect::<Vec<_>>();
    actual_linked.sort();
    assert_eq!(actual_linked, expected_linked);
    assert_eq!(
        stumps_rows[1],
        "stemsbeamstumpsprefixbeam chula.png#1 system 1 event 0 beamOrdinal 0 beamSig 12 stumpLinkers 3 sideStumps 2"
    );
    assert!(
        stumps_rows[2].contains(
            "event 1 beamOrdinal 0 beamSig 12 stumpOrdinal 0 bAlias beam:12:b:0 vSide TOP"
        )
    );
    assert!(stumps_rows[2].ends_with("sideStump true linkedBefore true action SkipSideStump"));
    assert!(
        stumps_rows[3].contains(
            "event 2 beamOrdinal 0 beamSig 12 stumpOrdinal 1 bAlias beam:12:b:1 vSide TOP"
        )
    );
    assert!(stumps_rows[3].ends_with(
        "plan 147 stemProfile 3 linkProfile 1 headTargets 2 lastIndex 2 relations 2 glyphs 1 lineChanged false action AwaitingVLinkTransaction"
    ));
    assert_eq!(
        stumps_rows[4],
        "stemsbeamstumpsprefixterminal chula.png#1 system 1 events 3 beamOrdinal 0 beamSig 12 stumpOrdinal 1 bAlias beam:12:b:1 vSide TOP plan 147 terminal AwaitingVLinkTransaction stopBeforeCreateStem true"
    );
    assert!(
        stumps_rows[5]
            .contains("schema stems-beam-stumps-prefix-v1 page chula.png#1 system 1 rows 5")
    );
    assert!(stumps_rows[5].contains(" sidesRowsByteIdentical true "));
    assert!(
        stumps_rows[5]
            .ends_with("freshRuns 2 freshRunsByteIdentical true stopBeforeCreateStem true")
    );
    let summary_tokens = stumps_rows[5].split_ascii_whitespace().collect::<Vec<_>>();
    let summary_value = |name: &str| {
        summary_tokens
            .iter()
            .position(|token| *token == name)
            .and_then(|index| summary_tokens.get(index + 1))
            .copied()
            .expect("strict STUMPS summary field")
    };
    assert_eq!(
        summary_value("probeSourceSha256"),
        sha256_hex(
            &std::fs::read(repo_root().join("rust/oracle/java/StemsBeamSidesLoopProbe.java"))
                .expect("active STUMPS probe source")
        )
    );
    assert_eq!(
        summary_value("runnerSourceSha256"),
        sha256_hex(
            &std::fs::read(repo_root().join("rust/oracle/java/run-stems-beam-stumps-prefix.sh"))
                .expect("active STUMPS runner source")
        )
    );
    assert_eq!(
        summary_value("sidesFixtureSha256"),
        sha256_hex(
            &std::fs::read(repo_root().join("rust/oracle/stems-beam-sides-pass-chula-system1.txt"))
                .expect("frozen predecessor SIDES fixture")
        )
    );
    let emitted_body = format!("{}\n", stumps_rows[..5].join("\n"));
    assert_eq!(
        summary_value("emittedBodySha256"),
        sha256_hex(emitted_body.as_bytes())
    );

    let stumps_transaction_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-stumps-transaction-chula-system1.txt"),
    )
    .expect("expected-only first-STUMPS transaction");
    assert_eq!(
        sha256_hex(stumps_transaction_text.as_bytes()),
        "267659af2190ca7e6901a9803cfd85440f9d981ac67afe2f638e5ca63372a999"
    );
    let transaction_rows = stumps_transaction_text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>();
    assert_eq!(transaction_rows.len(), 7);
    assert_eq!(
        transaction_rows[0],
        "stemsbeamstumpstxnresult chula.png#1 system 1 transaction 1 plan 147 beamSig 12 bAlias beam:12:b:1 vSide TOP registration ReuseActive disposition CreatedChecked registeredGlyphId 310 reuseOutcome AllUnlinked reuseEntriesRead 2 finalStemInterId 2372 baseNewVertex true baseGraphRelation sig-edge:331 bLinkedAfter true siblings 0 siblingAliases - heads 2 sigVertices 672 sigEdges 670 systemStems 33 outerAssignment false"
    );
    assert!(
        transaction_rows[1].contains(
            "event 3 beamOrdinal 0 beamSig 12 stumpOrdinal 2 bAlias beam:12:b:2 vSide TOP"
        )
    );
    assert!(transaction_rows[1].ends_with("sideStump true linkedBefore true action SkipSideStump"));
    assert_eq!(
        transaction_rows[2],
        "stemsbeamstumpstxnresumebeam chula.png#1 system 1 event 4 beamOrdinal 1 beamSig 22 stumpLinkers 3 sideStumps 2"
    );
    assert!(
        transaction_rows[3].contains(
            "event 5 beamOrdinal 1 beamSig 22 stumpOrdinal 0 bAlias beam:22:b:0 vSide TOP"
        )
    );
    assert!(transaction_rows[3].ends_with("sideStump true linkedBefore true action SkipSideStump"));
    assert!(
        transaction_rows[4].contains(
            "event 6 beamOrdinal 1 beamSig 22 stumpOrdinal 1 bAlias beam:22:b:1 vSide TOP"
        )
    );
    assert!(transaction_rows[4].ends_with(
        "plan 622 stemProfile 3 linkProfile 1 headTargets 2 lastIndex 2 relations 2 glyphs 1 lineChanged false action AwaitingVLinkTransaction"
    ));
    assert_eq!(
        transaction_rows[5],
        "stemsbeamstumpstxnresumeterminal chula.png#1 system 1 events 7 transactions 1 beamOrdinal 1 beamSig 22 stumpOrdinal 1 bAlias beam:22:b:1 vSide TOP plan 622 terminal AwaitingVLinkTransaction stopBeforeCreateStem true"
    );
    assert!(
        transaction_rows[6]
            .contains("schema stems-beam-stumps-transaction-v1 page chula.png#1 system 1 rows 6")
    );
    assert!(transaction_rows[6].contains(" sidesRowsByteIdentical true "));
    assert!(
        transaction_rows[6]
            .ends_with("freshRuns 2 freshRunsByteIdentical true stopBeforeSecondCreateStem true")
    );
    let transaction_summary_tokens = transaction_rows[6]
        .split_ascii_whitespace()
        .collect::<Vec<_>>();
    let transaction_summary_value = |name: &str| {
        transaction_summary_tokens
            .iter()
            .position(|token| *token == name)
            .and_then(|index| transaction_summary_tokens.get(index + 1))
            .copied()
            .expect("strict first-STUMPS transaction summary field")
    };
    assert_eq!(
        transaction_summary_value("probeSourceSha256"),
        summary_value("probeSourceSha256")
    );
    assert_eq!(
        transaction_summary_value("runnerSourceSha256"),
        summary_value("runnerSourceSha256")
    );
    assert_eq!(
        transaction_summary_value("prefixFixtureSha256"),
        sha256_hex(stumps_prefix_text.as_bytes())
    );
    assert_eq!(
        transaction_summary_value("sidesFixtureSha256"),
        summary_value("sidesFixtureSha256")
    );
    let transaction_body = format!("{}\n", transaction_rows[..6].join("\n"));
    assert_eq!(
        transaction_summary_value("emittedBodySha256"),
        sha256_hex(transaction_body.as_bytes())
    );

    let second_stumps_transaction_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-stumps-second-transaction-chula-system1.txt"),
    )
    .expect("expected-only second-STUMPS transaction");
    assert_eq!(
        sha256_hex(second_stumps_transaction_text.as_bytes()),
        "3cba09c13c555e56ea4ad1f65b0ba5610e5bacdb81e73fcc144473b3f3dce0f2"
    );
    let second_transaction_rows = second_stumps_transaction_text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>();
    assert_eq!(second_transaction_rows.len(), 7);
    assert_eq!(
        second_transaction_rows[0],
        "stemsbeamstumpstxnresult chula.png#1 system 1 transaction 2 plan 622 beamSig 22 bAlias beam:22:b:1 vSide TOP registration ReuseActive disposition CreatedChecked registeredGlyphId 321 reuseOutcome AllUnlinked reuseEntriesRead 2 finalStemInterId 2373 baseNewVertex true baseGraphRelation sig-edge:334 bLinkedAfter true siblings 0 siblingAliases - heads 2 sigVertices 673 sigEdges 673 systemStems 34 outerAssignment false"
    );
    assert!(
        second_transaction_rows[1].contains(
            "event 7 beamOrdinal 1 beamSig 22 stumpOrdinal 2 bAlias beam:22:b:2 vSide TOP"
        )
    );
    assert!(
        second_transaction_rows[1]
            .ends_with("sideStump true linkedBefore true action SkipSideStump")
    );
    assert_eq!(
        second_transaction_rows[2],
        "stemsbeamstumpstxnresumebeam chula.png#1 system 1 event 8 beamOrdinal 2 beamSig 16 stumpLinkers 3 sideStumps 2"
    );
    assert!(
        second_transaction_rows[3].contains(
            "event 9 beamOrdinal 2 beamSig 16 stumpOrdinal 0 bAlias beam:16:b:0 vSide TOP"
        )
    );
    assert!(
        second_transaction_rows[3]
            .ends_with("sideStump true linkedBefore true action SkipSideStump")
    );
    assert!(
        second_transaction_rows[4].contains(
            "event 10 beamOrdinal 2 beamSig 16 stumpOrdinal 1 bAlias beam:16:b:1 vSide TOP"
        )
    );
    assert!(second_transaction_rows[4].ends_with(
        "plan 404 stemProfile 3 linkProfile 1 headTargets 2 lastIndex 3 relations 2 glyphs 2 lineChanged false action AwaitingVLinkTransaction"
    ));
    assert_eq!(
        second_transaction_rows[5],
        "stemsbeamstumpstxnresumeterminal chula.png#1 system 1 events 11 transactions 2 beamOrdinal 2 beamSig 16 stumpOrdinal 1 bAlias beam:16:b:1 vSide TOP plan 404 terminal AwaitingVLinkTransaction stopBeforeCreateStem true"
    );
    assert!(second_transaction_rows[6].contains(
        "schema stems-beam-stumps-second-transaction-v1 page chula.png#1 system 1 rows 6"
    ));
    assert!(second_transaction_rows[6].contains(" sidesRowsByteIdentical true "));
    assert!(second_transaction_rows[6].contains(" firstTransactionPrefixByteIdentical true "));
    assert!(
        second_transaction_rows[6]
            .ends_with("freshRuns 2 freshRunsByteIdentical true stopBeforeThirdCreateStem true")
    );
    let second_summary_tokens = second_transaction_rows[6]
        .split_ascii_whitespace()
        .collect::<Vec<_>>();
    let second_summary_value = |name: &str| {
        second_summary_tokens
            .iter()
            .position(|token| *token == name)
            .and_then(|index| second_summary_tokens.get(index + 1))
            .copied()
            .expect("strict second-STUMPS transaction summary field")
    };
    assert_eq!(
        second_summary_value("probeSourceSha256"),
        summary_value("probeSourceSha256")
    );
    assert_eq!(
        second_summary_value("runnerSourceSha256"),
        sha256_hex(
            &std::fs::read(repo_root().join("rust/oracle/java/run-stems-beam-stumps-second.sh"))
                .expect("active second-STUMPS runner source")
        )
    );
    assert_eq!(
        second_summary_value("firstTransactionFixtureSha256"),
        sha256_hex(stumps_transaction_text.as_bytes())
    );
    assert_eq!(
        second_summary_value("sidesFixtureSha256"),
        summary_value("sidesFixtureSha256")
    );
    let second_transaction_body = format!("{}\n", second_transaction_rows[..6].join("\n"));
    assert_eq!(
        second_summary_value("emittedBodySha256"),
        sha256_hex(second_transaction_body.as_bytes())
    );

    let third_stumps_transaction_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-stumps-third-transaction-chula-system1.txt"),
    )
    .expect("expected-only third-STUMPS transaction");
    assert_eq!(
        sha256_hex(third_stumps_transaction_text.as_bytes()),
        "bd1fac9822659da8dbfd5159257c3f8005d96fb915dd1439773ac42183e4e321"
    );
    let third_transaction_rows = third_stumps_transaction_text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>();
    assert_eq!(third_transaction_rows.len(), 7);
    assert_eq!(
        third_transaction_rows[0],
        "stemsbeamstumpstxnresult chula.png#1 system 1 transaction 3 plan 404 beamSig 16 bAlias beam:16:b:1 vSide TOP registration ReuseActive disposition CreatedChecked registeredGlyphId 303 reuseOutcome AllUnlinked reuseEntriesRead 2 finalStemInterId 2374 baseNewVertex true baseGraphRelation sig-edge:337 bLinkedAfter true siblings 0 siblingAliases - heads 2 sigVertices 674 sigEdges 676 systemStems 35 outerAssignment false"
    );
    assert!(
        third_transaction_rows[1].contains(
            "event 11 beamOrdinal 2 beamSig 16 stumpOrdinal 2 bAlias beam:16:b:2 vSide TOP"
        )
    );
    assert!(
        third_transaction_rows[1]
            .ends_with("sideStump true linkedBefore true action SkipSideStump")
    );
    assert_eq!(
        third_transaction_rows[2],
        "stemsbeamstumpstxnresumebeam chula.png#1 system 1 event 12 beamOrdinal 3 beamSig 28 stumpLinkers 3 sideStumps 2"
    );
    assert!(
        third_transaction_rows[3].contains(
            "event 13 beamOrdinal 3 beamSig 28 stumpOrdinal 0 bAlias beam:28:b:0 vSide TOP"
        )
    );
    assert!(
        third_transaction_rows[3]
            .ends_with("sideStump true linkedBefore true action SkipSideStump")
    );
    assert!(
        third_transaction_rows[4].contains(
            "event 14 beamOrdinal 3 beamSig 28 stumpOrdinal 1 bAlias beam:28:b:1 vSide TOP"
        )
    );
    assert!(third_transaction_rows[4].ends_with(
        "plan 508 stemProfile 3 linkProfile 1 headTargets 2 lastIndex 3 relations 2 glyphs 2 lineChanged false action AwaitingVLinkTransaction"
    ));
    assert_eq!(
        third_transaction_rows[5],
        "stemsbeamstumpstxnresumeterminal chula.png#1 system 1 events 15 transactions 3 beamOrdinal 3 beamSig 28 stumpOrdinal 1 bAlias beam:28:b:1 vSide TOP plan 508 terminal AwaitingVLinkTransaction stopBeforeCreateStem true"
    );
    assert!(third_transaction_rows[6].contains(
        "schema stems-beam-stumps-third-transaction-v1 page chula.png#1 system 1 rows 6"
    ));
    assert!(third_transaction_rows[6].contains(" sidesRowsByteIdentical true "));
    assert!(third_transaction_rows[6].contains(" secondTransactionPrefixByteIdentical true "));
    assert!(
        third_transaction_rows[6]
            .ends_with("freshRuns 2 freshRunsByteIdentical true stopBeforeFourthCreateStem true")
    );
    let third_summary_tokens = third_transaction_rows[6]
        .split_ascii_whitespace()
        .collect::<Vec<_>>();
    let third_summary_value = |name: &str| {
        third_summary_tokens
            .iter()
            .position(|token| *token == name)
            .and_then(|index| third_summary_tokens.get(index + 1))
            .copied()
            .expect("strict third-STUMPS transaction summary field")
    };
    assert_eq!(
        third_summary_value("probeSourceSha256"),
        summary_value("probeSourceSha256")
    );
    assert_eq!(
        third_summary_value("runnerSourceSha256"),
        sha256_hex(
            &std::fs::read(repo_root().join("rust/oracle/java/run-stems-beam-stumps-third.sh"))
                .expect("active third-STUMPS runner source")
        )
    );
    assert_eq!(
        third_summary_value("secondTransactionFixtureSha256"),
        sha256_hex(second_stumps_transaction_text.as_bytes())
    );
    assert_eq!(
        third_summary_value("sidesFixtureSha256"),
        summary_value("sidesFixtureSha256")
    );
    let third_transaction_body = format!("{}\n", third_transaction_rows[..6].join("\n"));
    assert_eq!(
        third_summary_value("emittedBodySha256"),
        sha256_hex(third_transaction_body.as_bytes())
    );

    let complete_stumps_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-stumps-complete-chula-system1.txt"),
    )
    .expect("expected-only complete STUMPS suffix");
    assert_eq!(
        sha256_hex(complete_stumps_text.as_bytes()),
        "054ed437739a86f981d0579b4161b52e3983cb75a952f9e39817f4fdb039ffb1"
    );
    let complete_rows = complete_stumps_text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>();
    assert_eq!(complete_rows.len(), 83);
    let complete_results = complete_rows
        .iter()
        .copied()
        .filter(|line| line.starts_with("stemsbeamstumpstxnresult "))
        .collect::<Vec<_>>();
    assert_eq!(complete_results.len(), 4);
    assert_eq!(
        complete_results[0],
        "stemsbeamstumpstxnresult chula.png#1 system 1 transaction 4 plan 508 beamSig 28 bAlias beam:28:b:1 vSide TOP registration ReuseActive disposition CreatedChecked registeredGlyphId 308 reuseOutcome AllUnlinked reuseEntriesRead 2 finalStemInterId 2375 baseNewVertex true baseGraphRelation sig-edge:340 bLinkedAfter true siblings 0 siblingAliases - heads 2 sigVertices 675 sigEdges 679 systemStems 36 outerAssignment false"
    );
    assert_eq!(
        complete_results[1],
        "stemsbeamstumpstxnresult chula.png#1 system 1 transaction 5 plan 28 beamSig 29 bAlias beam:29:b:1 vSide TOP registration ReuseActive disposition CreatedChecked registeredGlyphId 305 reuseOutcome AllUnlinked reuseEntriesRead 2 finalStemInterId 2376 baseNewVertex true baseGraphRelation sig-edge:343 bLinkedAfter true siblings 0 siblingAliases - heads 2 sigVertices 676 sigEdges 682 systemStems 37 outerAssignment false"
    );
    assert_eq!(
        complete_results[2],
        "stemsbeamstumpstxnresult chula.png#1 system 1 transaction 6 plan 330 beamSig 32 bAlias beam:32:b:1 vSide TOP registration ReuseActive disposition CreatedChecked registeredGlyphId 302 reuseOutcome AllUnlinked reuseEntriesRead 3 finalStemInterId 2377 baseNewVertex true baseGraphRelation sig-edge:346 bLinkedAfter true siblings 0 siblingAliases - heads 3 sigVertices 677 sigEdges 686 systemStems 38 outerAssignment false"
    );
    assert_eq!(
        complete_results[3],
        "stemsbeamstumpstxnresult chula.png#1 system 1 transaction 7 plan 251 beamSig 31 bAlias beam:31:b:1 vSide TOP registration ReuseActive disposition CreatedChecked registeredGlyphId 300 reuseOutcome AllUnlinked reuseEntriesRead 2 finalStemInterId 2378 baseNewVertex true baseGraphRelation sig-edge:350 bLinkedAfter true siblings 0 siblingAliases - heads 2 sigVertices 678 sigEdges 689 systemStems 39 outerAssignment false"
    );
    assert_eq!(
        complete_rows
            .iter()
            .filter(|line| line.starts_with("stemsbeamstumpstxnresumebeam "))
            .count(),
        30
    );
    let complete_steps = complete_rows
        .iter()
        .copied()
        .filter(|line| line.starts_with("stemsbeamstumpstxnresumestep "))
        .collect::<Vec<_>>();
    assert_eq!(complete_steps.len(), 47);
    assert_eq!(
        complete_steps
            .iter()
            .filter(|line| line.ends_with("action SkipSideStump"))
            .count(),
        44
    );
    assert_eq!(
        complete_steps
            .iter()
            .filter(|line| line.ends_with("action AwaitingVLinkTransaction"))
            .count(),
        3
    );
    assert!(complete_steps.iter().all(|line| {
        line.ends_with("action SkipSideStump") || line.ends_with("action AwaitingVLinkTransaction")
    }));
    assert_eq!(
        complete_rows[81],
        "stemsbeamstumpstxnresumeterminal chula.png#1 system 1 events 92 transactions 7 terminal Completed stopBeforeCreateStem true"
    );
    assert!(complete_rows[82].contains(
        "schema stems-beam-stumps-complete-v1 page chula.png#1 system 1 rows 82 transactions 4"
    ));
    assert!(complete_rows[82].contains(" sidesRowsByteIdentical true "));
    assert!(complete_rows[82].contains(" thirdTransactionPrefixByteIdentical true "));
    assert!(
        complete_rows[82].ends_with("freshRuns 2 freshRunsByteIdentical true terminal Completed")
    );
    let complete_summary_tokens = complete_rows[82]
        .split_ascii_whitespace()
        .collect::<Vec<_>>();
    let complete_summary_value = |name: &str| {
        complete_summary_tokens
            .iter()
            .position(|token| *token == name)
            .and_then(|index| complete_summary_tokens.get(index + 1))
            .copied()
            .expect("strict complete-STUMPS summary field")
    };
    assert_eq!(
        complete_summary_value("probeSourceSha256"),
        summary_value("probeSourceSha256")
    );
    assert_eq!(
        complete_summary_value("runnerSourceSha256"),
        sha256_hex(
            &std::fs::read(repo_root().join("rust/oracle/java/run-stems-beam-stumps-complete.sh"))
                .expect("active complete-STUMPS runner source")
        )
    );
    assert_eq!(
        complete_summary_value("thirdTransactionFixtureSha256"),
        sha256_hex(third_stumps_transaction_text.as_bytes())
    );
    assert_eq!(
        complete_summary_value("sidesFixtureSha256"),
        summary_value("sidesFixtureSha256")
    );
    let complete_body = format!("{}\n", complete_rows[..82].join("\n"));
    assert_eq!(
        complete_summary_value("emittedBodySha256"),
        sha256_hex(complete_body.as_bytes())
    );

    let head_phase_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-head-phase-prefix-chula-system1.txt"),
    )
    .expect("expected-only post-STUMPS head-phase prefix");
    assert_eq!(
        sha256_hex(head_phase_text.as_bytes()),
        "181d4bfcb5f2fe0a6442ee6826e74a10703039f567f00df7e33133ca4e15e798"
    );
    let head_phase_rows = head_phase_text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>();
    assert_eq!(head_phase_rows.len(), 11);
    let head_field = |line: &str, name: &str| {
        let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
        let index = tokens
            .iter()
            .position(|token| *token == name)
            .unwrap_or_else(|| panic!("missing head-phase field {name}: {line}"));
        tokens[index + 1].to_owned()
    };
    let second_java = head_phase_rows[6];
    assert_eq!(head_field(second_java, "headOrder"), "1");
    assert_eq!(
        head_field(second_java, "headSig").parse::<usize>().unwrap(),
        second_continuation.processed_head.sig_ordinal
    );
    assert_eq!(head_field(second_java, "headX"), "90");
    assert_eq!(head_field(second_java, "headInterId"), "1331");
    assert_eq!(
        head_field(second_java, "decisions"),
        "[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither]"
    );
    assert_eq!(head_field(second_java, "returned"), "true");
    assert_eq!(second_continuation.returned_linked, Some(true));
    assert_eq!(
        head_field(second_java, "closedValueChanges")
            .parse::<usize>()
            .unwrap(),
        second_continuation.closed_value_changes
    );
    assert_eq!(
        head_field(second_java, "closureWrites"),
        "[x89:sig22:LEFT:false->true,x89:sig22:RIGHT:false->true]"
    );
    assert_eq!(head_field(second_java, "unlinkedCount"), "0");
    assert_eq!(head_field(second_java, "nextHeadOrder"), "2");
    assert_eq!(head_field(second_java, "nextHeadX"), "81");
    assert_eq!(head_field(second_java, "nextHeadSig"), "33");
    assert_eq!(head_field(second_java, "nextHeadInterId"), "1351");

    let third_java = head_phase_rows[7];
    assert_eq!(head_field(third_java, "headOrder"), "2");
    assert_eq!(
        head_field(third_java, "headSig").parse::<usize>().unwrap(),
        third_continuation.processed_head.sig_ordinal
    );
    assert_eq!(head_field(third_java, "headX"), "81");
    assert_eq!(head_field(third_java, "headInterId"), "1351");
    assert_eq!(
        head_field(third_java, "grade"),
        "0x1.901efd26d99b1p-1/3fe901efd26d99b1"
    );
    assert_eq!(
        head_field(third_java, "decisions"),
        "[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither]"
    );
    assert_eq!(head_field(third_java, "returned"), "true");
    assert_eq!(third_continuation.returned_linked, Some(true));
    assert_eq!(
        head_field(third_java, "closedValueChanges")
            .parse::<usize>()
            .unwrap(),
        third_continuation.closed_value_changes
    );
    assert_eq!(
        head_field(third_java, "closureWrites"),
        "[x79:sig40:LEFT:false->true,x79:sig40:RIGHT:false->true,x80:sig32:LEFT:false->true,x80:sig32:RIGHT:false->true]"
    );
    assert_eq!(head_field(third_java, "unlinkedCount"), "0");
    assert_eq!(head_field(third_java, "nextHeadOrder"), "3");
    assert_eq!(head_field(third_java, "nextHeadX"), "20");
    assert_eq!(head_field(third_java, "nextHeadSig"), "65");
    assert_eq!(head_field(third_java, "nextHeadInterId"), "1419");
    assert_eq!(
        head_field(third_java, "nextGrade"),
        "0x1.8e97b8a9fa8cap-1/3fe8e97b8a9fa8ca"
    );

    let fourth_java = head_phase_rows[8];
    assert_eq!(head_field(fourth_java, "headOrder"), "3");
    assert_eq!(
        head_field(fourth_java, "headSig").parse::<usize>().unwrap(),
        fourth_continuation.processed_head.sig_ordinal
    );
    assert_eq!(head_field(fourth_java, "headX"), "20");
    assert_eq!(head_field(fourth_java, "headInterId"), "1419");
    assert_eq!(
        head_field(fourth_java, "grade"),
        "0x1.8e97b8a9fa8cap-1/3fe8e97b8a9fa8ca"
    );
    assert_eq!(
        head_field(fourth_java, "decisions"),
        "[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither]"
    );
    assert_eq!(head_field(fourth_java, "returned"), "true");
    assert_eq!(fourth_continuation.returned_linked, Some(true));
    assert_eq!(
        head_field(fourth_java, "closedValueChanges")
            .parse::<usize>()
            .unwrap(),
        fourth_continuation.closed_value_changes
    );
    assert_eq!(
        head_field(fourth_java, "closureWrites"),
        "[x19:sig64:LEFT:false->true,x19:sig64:RIGHT:false->true]"
    );
    assert_eq!(head_field(fourth_java, "unlinkedCount"), "0");
    assert_eq!(head_field(fourth_java, "nextHeadOrder"), "4");
    assert_eq!(head_field(fourth_java, "nextHeadX"), "36");
    assert_eq!(head_field(fourth_java, "nextHeadSig"), "69");
    assert_eq!(head_field(fourth_java, "nextHeadInterId"), "1427");

    let fifth_java = head_phase_rows[9];
    assert_eq!(head_field(fifth_java, "headOrder"), "4");
    assert_eq!(
        head_field(fifth_java, "headSig").parse::<usize>().unwrap(),
        fifth_continuation.processed_head.sig_ordinal
    );
    assert_eq!(head_field(fifth_java, "headX"), "36");
    assert_eq!(head_field(fifth_java, "headInterId"), "1427");
    assert_eq!(
        head_field(fifth_java, "grade"),
        "0x1.8e37718100f0cp-1/3fe8e37718100f0c"
    );
    assert_eq!(
        head_field(fifth_java, "decisions"),
        "[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither]"
    );
    assert_eq!(head_field(fifth_java, "returned"), "true");
    assert_eq!(fifth_continuation.returned_linked, Some(true));
    assert_eq!(
        head_field(fifth_java, "closedValueChanges")
            .parse::<usize>()
            .unwrap(),
        fifth_continuation.closed_value_changes
    );
    assert_eq!(
        head_field(fifth_java, "closureWrites"),
        "[x35:sig68:LEFT:false->true,x35:sig68:RIGHT:false->true]"
    );
    assert_eq!(head_field(fifth_java, "unlinkedCount"), "0");
    assert_eq!(head_field(fifth_java, "nextHeadOrder"), "5");
    assert_eq!(head_field(fifth_java, "nextHeadX"), "99");
    assert_eq!(head_field(fifth_java, "nextHeadSig"), "61");
    assert_eq!(head_field(fifth_java, "nextHeadInterId"), "1411");
    let native_fifth_next_sides = format!(
        "[{}]",
        fifth_head_phase.heads[5]
            .sides
            .iter()
            .map(|cell| {
                let side = match cell.reference.horizontal {
                    NativeStemHeadSide::Left => "LEFT",
                    NativeStemHeadSide::Right => "RIGHT",
                };
                format!("{}:{}:{}", side, cell.linked, cell.closed)
            })
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(head_field(fifth_java, "nextSides"), native_fifth_next_sides);

    let frontier = head_phase_rows[1];
    assert_eq!(head_field(frontier, "headOrder"), "0");
    assert_eq!(
        head_field(frontier, "headSig").parse::<usize>().unwrap(),
        head_phase.frontier.head.sig_ordinal
    );
    assert_eq!(head_field(frontier, "headInterId"), "1375");
    assert_eq!(
        head_field(frontier, "stemProfile").parse::<i32>().unwrap(),
        head_phase.frontier.stem_profile
    );
    assert_eq!(
        head_field(frontier, "linkProfile").parse::<i32>().unwrap(),
        head_phase.frontier.link_profile
    );
    assert_eq!(head_field(frontier, "append"), "false");
    assert_eq!(
        head_field(frontier, "sides"),
        "[LEFT:false:false,RIGHT:false:false]"
    );
    assert_eq!(
        head_field(frontier, "decisions"),
        "[LEFT:top=false:bottom=false:branch=Neither,RIGHT:top=true:bottom=false:branch=TopOnly]"
    );
    assert_eq!(head_field(frontier, "selectedC"), "h:38:RIGHT:TOP");
    assert_eq!(head_phase.frontier.next_corner.x_ordinal, 38);
    assert_eq!(head_phase.frontier.next_corner.sig_ordinal, 45);
    assert_eq!(
        head_phase.frontier.next_corner.horizontal,
        NativeStemHeadSide::Right
    );
    assert_eq!(
        head_phase.frontier.next_corner.vertical,
        NativeStemVerticalSide::Top
    );
    assert_eq!(head_phase.heads[0].grade.to_bits(), 0x3fe9_17c3_b820_7578);
    assert_eq!(
        head_field(frontier, "terminal"),
        "AwaitingHeadCLinkTransaction"
    );
    let expand = head_phase_rows[2];
    assert_eq!(head_field(expand, "cAlias"), "h:38:RIGHT:TOP");
    assert_eq!(
        head_field(expand, "lastIndex"),
        head_transaction.last_index.to_string()
    );
    assert_eq!(
        head_field(expand, "maxIndex"),
        head_transaction.max_index.to_string()
    );
    assert_eq!(head_field(expand, "relations"), "1");
    assert_eq!(head_field(expand, "glyphs"), "1");
    assert_eq!(head_field(expand, "candidateIdBefore"), "307");
    assert_eq!(head_field(expand, "existingGlyph"), "glyph:307");
    assert_eq!(head_field(expand, "existingActive"), "true");
    assert_eq!(head_field(expand, "existingStem"), "-");
    assert_eq!(head_field(expand, "terminal"), "ReadyForHeadCreateStem");
    let java_result = head_phase_rows[3];
    assert_eq!(head_field(java_result, "relationsBefore"), "0");
    assert_eq!(head_field(java_result, "relationsAfter"), "1");
    assert_eq!(head_field(java_result, "linked"), "true");
    assert_eq!(head_field(java_result, "sigVerticesBefore"), "678");
    assert_eq!(head_field(java_result, "sigVerticesAfter"), "679");
    assert_eq!(head_field(java_result, "sigEdgesBefore"), "689");
    assert_eq!(head_field(java_result, "sigEdgesAfter"), "690");
    assert_eq!(head_field(java_result, "systemStemsBefore"), "39");
    assert_eq!(head_field(java_result, "systemStemsAfter"), "40");
    assert_eq!(
        head_field(java_result, "terminal"),
        "ReturnedBeforeSecondHead"
    );
    assert_eq!(head_field(java_result, "dirtyBefore"), "true:true:true");
    assert_eq!(head_field(java_result, "dirtyAfter"), "true:true:true");
    assert_eq!(head_field(java_result, "nextHeadOrder"), "1");
    assert_eq!(head_field(java_result, "nextHeadSig"), "23");
    assert_eq!(head_field(java_result, "nextHeadInterId"), "1331");
    assert_eq!(
        head_field(java_result, "nextSides"),
        "[LEFT:true:false,RIGHT:false:false]"
    );
    assert_eq!(
        head_field(java_result, "nextHeadSig")
            .parse::<usize>()
            .expect("numeric next head SIG ordinal"),
        second_continuation.processed_head.sig_ordinal
    );
    let native_next_sides = format!(
        "[{}]",
        second_head_phase.heads[1]
            .sides
            .iter()
            .map(|cell| {
                let side = match cell.reference.horizontal {
                    NativeStemHeadSide::Left => "LEFT",
                    NativeStemHeadSide::Right => "RIGHT",
                };
                format!("{}:{}:{}", side, cell.linked, cell.closed)
            })
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(head_field(java_result, "nextSides"), native_next_sides);
    let create = head_phase_rows[4];
    assert_eq!(head_field(create, "registeredAlias"), "glyph:307");
    assert_eq!(head_field(create, "registeredId"), "307");
    assert_eq!(head_field(create, "registration"), "ReuseActive");
    assert_eq!(head_field(create, "disposition"), "CreatedChecked");
    assert_eq!(head_field(create, "stemId"), "2379");
    assert_eq!(head_field(create, "stemVertex"), "260");
    assert_eq!(head_field(create, "allocatorBefore"), "2378");
    assert_eq!(head_field(create, "allocatorAfter"), "2379");
    assert_eq!(head_field(create, "systemStemsBefore"), "39");
    assert_eq!(head_field(create, "systemStemsAfter"), "40");
    assert_eq!(head_field(create, "interIndexBefore"), "678");
    assert_eq!(head_field(create, "interIndexAfter"), "679");
    let apply = head_phase_rows[5];
    assert_eq!(head_field(apply, "linked"), "true");
    assert_eq!(head_field(apply, "addedVertices"), "1");
    assert_eq!(head_field(apply, "addedEdges"), "1");
    assert_eq!(
        head_field(apply, "terminal"),
        "ReturnedHeadCLinkTransaction"
    );
    let head_summary = head_phase_rows[10];
    assert_eq!(
        head_field(head_summary, "schema"),
        "stems-head-phase-prefix-v5"
    );
    assert_eq!(head_field(head_summary, "rows"), "10");
    assert_eq!(head_field(head_summary, "freshRuns"), "2");
    assert_eq!(head_field(head_summary, "freshRunsByteIdentical"), "true");
    assert_eq!(
        head_field(head_summary, "probeSourceSha256"),
        sha256_hex(
            &std::fs::read(repo_root().join("rust/oracle/java/StemsBeamSidesLoopProbe.java"))
                .expect("active shared head-phase probe")
        )
    );
    assert_eq!(
        head_field(head_summary, "runnerSourceSha256"),
        sha256_hex(
            &std::fs::read(repo_root().join("rust/oracle/java/run-stems-head-phase-prefix.sh"))
                .expect("active head-phase runner")
        )
    );
    assert_eq!(
        head_field(head_summary, "completeStumpsFixtureSha256"),
        sha256_hex(complete_stumps_text.as_bytes())
    );

    let all_siblings = std::iter::once(&actual)
        .chain(std::iter::once(&second_siblings))
        .chain(repeated.iter().map(|transaction| &transaction.siblings))
        .collect::<Vec<_>>();
    assert_eq!(all_siblings.len(), 32);
    assert_eq!(
        all_siblings
            .iter()
            .map(|transaction| {
                hydrated
                    .stumps
                    .beams_by_abscissa
                    .iter()
                    .find(|beam| beam.source == transaction.base_b_linker.beam)
                    .expect("executed base beam in native stump catalogue")
                    .sig_ordinal
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
        EXECUTED_BASE_BEAM_SIG_ORDINALS
    );
    assert_eq!(
        all_siblings
            .iter()
            .map(|transaction| transaction.assigned_b_linkers.len())
            .sum::<usize>(),
        29
    );
    assert_eq!(
        all_siblings
            .iter()
            .map(|transaction| transaction.b_linker_write_count)
            .sum::<usize>(),
        61
    );
    let all_heads = std::iter::once(&head_actual)
        .chain(std::iter::once(&second_heads))
        .chain(repeated.iter().map(|transaction| &transaction.heads))
        .collect::<Vec<_>>();
    assert_eq!(
        all_heads
            .iter()
            .map(|transaction| transaction.s_linker_write_count)
            .sum::<usize>(),
        68
    );

    // Expected sequencing is deliberately opened only after the native loop
    // has returned and the owned graph/cell terminal has been authenticated.
    let sides_pass_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-sides-pass-chula-system1.txt"),
    )
    .expect("expected-only SIDES pass");
    let expected_steps = sides_pass_text
        .lines()
        .filter(|line| line.starts_with("stemsbeamsidesloopstep ") && line.contains(" step 0 "))
        .map(|line| {
            let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
            let value = |name: &str| {
                tokens
                    .iter()
                    .position(|token| *token == name)
                    .and_then(|index| tokens.get(index + 1))
                    .copied()
                    .expect("SIDES step field")
            };
            (
                value("plan").parse::<usize>().unwrap(),
                value("bAlias").to_owned(),
            )
        })
        .collect::<Vec<_>>();
    let actual_steps = std::iter::once((
        native_base.key.plan.plan_ordinal,
        second_b_alias(actual.base_b_linker),
    ))
    .chain(std::iter::once((
        second_flag.key.plan.plan_ordinal,
        second_b_alias(second_siblings.base_b_linker),
    )))
    .chain(repeated.iter().map(|transaction| {
        (
            transaction.flag.key.plan.plan_ordinal,
            second_b_alias(transaction.siblings.base_b_linker),
        )
    }))
    .collect::<Vec<_>>();
    assert_eq!(actual_steps, expected_steps);

    let expected_siblings = sides_pass_text
        .lines()
        .filter(|line| line.starts_with("stemsbeamsidesloopsibling "))
        .map(|line| {
            let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
            let value = |name: &str| {
                tokens
                    .iter()
                    .position(|token| *token == name)
                    .and_then(|index| tokens.get(index + 1))
                    .copied()
                    .expect("SIDES sibling field")
            };
            let count = value("siblings").parse::<usize>().unwrap();
            let aliases = if count == 0 {
                Vec::new()
            } else {
                value("aliases").split(',').map(str::to_owned).collect()
            };
            (value("bAlias").to_owned(), aliases)
        })
        .collect::<Vec<_>>();
    let actual_sibling_rows = all_siblings
        .iter()
        .map(|transaction| {
            (
                second_b_alias(transaction.base_b_linker),
                transaction
                    .assigned_b_linkers
                    .iter()
                    .copied()
                    .map(second_b_alias)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(actual_sibling_rows, expected_siblings);
    assert!(sides_pass_text.lines().any(|line| {
        line == "stemsbeamsidesloopcensus chula.png#1 system 1 freshTransactions 31 secondFrontier true sidesExhausted true"
    }));

    // A B16 failure occurs after provisional B12-B15 work, but no part of the
    // caller's complete carrier can escape the shadow.
    let assigned = carried_third
        .siblings
        .assigned_b_linkers
        .first()
        .copied()
        .expect("transaction-3 sibling cell");
    let mut late_failure = carrier_before.clone();
    late_failure
        .b_cells
        .retain(|cell| cell.reference != assigned);
    let late_failure_before = late_failure.clone();
    let bridge_before = bridge.clone();
    advance_native_stems_beam_sides_transaction_from_first_stems_bridge(
        &mut late_failure,
        context,
        &bridge,
    )
    .expect_err("missing sibling B cell must reject the bridge carrier transaction");
    assert_eq!(late_failure, late_failure_before);
    assert_eq!(bridge, bridge_before);

    let third_create = apply_native_stems_beam_vlink_create_stem_transaction(
        third_scheduler,
        &hydrated.builder,
        &hydrated.plans,
        &mut third_transaction_state,
        &b15_hydration::checker_context_for_page(&b15_hydration::native_predecessor_page(
            "chula.png",
        )),
    )
    .expect("native transaction-3 B12");
    let third_live = project_native_stems_beam_vlink_reuse_live_state(
        &second_sig,
        &second_bindings,
        third_scheduler,
        &hydrated.plans,
        &second_s_cells,
        &third_transaction_state.system_stems,
    )
    .expect("native transaction-3 B13 live state");
    let third_reuse = evaluate_native_stems_beam_vlink_reuse_check(
        third_scheduler,
        &hydrated.plans,
        &hydrated.stumps,
        &hydrated.vlinkers,
        &third_create,
        &third_transaction_state,
        &third_live,
        hydrated.relation_parameters,
    )
    .expect("native transaction-3 B13");
    let mut third_sig = second_sig.clone();
    let mut third_bindings = second_bindings.clone();
    let missing_third_beam = beam_bootstrap
        .iter()
        .copied()
        .filter(|entry| entry.source != third.b_linker.beam)
        .collect::<Vec<_>>();
    roll_native_stems_beam_vlink_base_apply_state(
        &second_base.state_after,
        &third_transaction_state,
        &third_reuse,
        &third_sig,
        &third_bindings,
        NativeStemsBeamVLinkBaseRolloverAuthority {
            stump_system: &hydrated.stumps,
            beam_inter_index: &missing_third_beam,
            configured_inter_vip_ids: &[],
        },
    )
    .expect_err("changed beam requires complete page InterIndex authority");
    assert_eq!(third_sig, second_sig);
    assert_eq!(third_bindings, second_bindings);
    let mut third_base_state = roll_native_stems_beam_vlink_base_apply_state(
        &second_base.state_after,
        &third_transaction_state,
        &third_reuse,
        &third_sig,
        &third_bindings,
        NativeStemsBeamVLinkBaseRolloverAuthority {
            stump_system: &hydrated.stumps,
            beam_inter_index: &beam_bootstrap,
            configured_inter_vip_ids: &[],
        },
    )
    .expect("native changed-beam transaction-3 B14 rollover");
    let third_base_state_before = third_base_state.clone();
    assert_eq!(
        (
            third_base_state.inter_index.baseline_entry_count,
            third_base_state.sig.baseline_vertex_count,
            third_base_state.sig.baseline_relation_count,
        ),
        (641, 223, 212)
    );
    let third_base = apply_native_stems_beam_vlink_base_transaction_to_native_sig(
        third_scheduler,
        &hydrated.plans,
        &hydrated.stumps,
        &hydrated.vlinkers,
        &third_create,
        &third_live,
        hydrated.relation_parameters,
        &third_reuse,
        &mut third_base_state,
        &mut third_sig,
        &mut third_bindings,
    )
    .expect("native transaction-3 B14");
    assert_eq!(
        (third_sig.vertices.len(), third_sig.edges.len()),
        (224, 213)
    );
    assert_eq!(third_bindings.stem_vertices[&2].0, 223);
    assert_eq!(third_base.graph_relation_identity, Some(212));
    let third_target_linked = second_cells
        .iter()
        .find(|cell| cell.reference == third.b_linker)
        .expect("transaction-3 target B cell")
        .linked;
    assert!(!third_target_linked);
    let mut third_flag_state = NativeStemsBeamVLinkBLinkerFlagState {
        system_id: 1,
        base_apply_state_before: third_base_state_before,
        target_b_linker: third.b_linker,
        linked: third_target_linked,
        committed: None,
    };
    let third_flag = apply_native_stems_beam_vlink_b_linker_flag_transaction(
        third_scheduler,
        &hydrated.plans,
        &hydrated.stumps,
        &hydrated.vlinkers,
        &third_create,
        &third_live,
        hydrated.relation_parameters,
        &third_reuse,
        &third_base,
        &mut third_flag_state,
    )
    .expect("native transaction-3 B15");
    let mut third_cells = second_cells.clone();
    let third_siblings = apply_native_stems_beam_vlink_sibling_transaction_to_native_sig(
        &mut third_sig,
        &third_bindings,
        third_scheduler,
        &hydrated.stumps,
        &hydrated.vlinkers,
        &hydrated.reachability,
        &hydrated.builder,
        &third_base,
        &third_flag,
        &mut third_cells,
    )
    .expect("native transaction-3 B16");
    assert_eq!(
        third_siblings
            .graph
            .appended_edges
            .iter()
            .map(|edge| edge.0)
            .collect::<Vec<_>>(),
        vec![213]
    );
    assert_eq!(
        third_siblings
            .assigned_b_linkers
            .iter()
            .copied()
            .map(second_b_alias)
            .collect::<Vec<_>>(),
        vec!["beam:41:b:0"]
    );
    let mut third_s_cells = second_s_cells.clone();
    let third_heads = apply_native_stems_beam_vlink_head_transaction_to_native_sig(
        &mut third_sig,
        &third_bindings,
        third_scheduler,
        &hydrated.plans,
        &hydrated.builder,
        &hydrated.head_corners,
        &hydrated.reachability,
        &third_flag,
        &third_siblings,
        &third_cells,
        &mut third_s_cells,
    )
    .expect("native transaction-3 B17");
    assert_eq!(
        (third_sig.vertices.len(), third_sig.edges.len()),
        (224, 216)
    );
    assert_eq!(
        third_heads
            .appended_edges
            .iter()
            .map(|edge| edge.0)
            .collect::<Vec<_>>(),
        vec![214, 215]
    );
    assert_eq!(third_heads.s_linker_write_count, 2);
    assert_eq!(third_heads.s_linker_value_change_count, 2);
    let third_outer_resume = apply_native_stems_beam_outer_and_resume_transaction(
        third_scheduler,
        &hydrated.vlinkers,
        &hydrated.builder,
        &hydrated.plans,
        &hydrated.reachability,
        &third_flag,
        &third_siblings,
        &third_heads,
        &mut third_cells,
    )
    .expect("native transaction-3 B18/B19");
    let NativeStemsBeamSchedulerResumeStatus::AwaitingVLinkTransaction(fourth) =
        &third_outer_resume.resume.status
    else {
        panic!("transaction 3 did not reach a fourth frontier");
    };
    assert_eq!(fourth.plan.plan_ordinal, 627);
    assert_eq!(second_b_alias(fourth.b_linker), "beam:22:b:2");
    assert_eq!(fourth.vertical_side, NativeStemVerticalSide::Top);
    assert_eq!(carried_third.create, third_create);
    assert_eq!(carried_third.reuse_live_state, third_live);
    assert_eq!(carried_third.reuse, third_reuse);
    assert_eq!(carried_third.siblings, third_siblings);
    assert_eq!(carried_third.heads, third_heads);
    assert_eq!(carried_third.outer_resume, third_outer_resume);

    let txn2_sibling_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-txn2-sibling-links-chula.txt"),
    )
    .expect("expected-only transaction-2 B16 fixture");
    let txn2_sibling_result = txn2_sibling_text
        .lines()
        .find(|line| {
            line.starts_with("stemsbeamvlinksiblinglinksresult ")
                && line.contains(" system 1 plan 152 scope real case - ")
        })
        .expect("transaction-2 B16 result");
    assert!(txn2_sibling_result.contains(
        "siblings 2 committedEdges 2 committedEdgeAliases [sig-edge:208,sig-edge:209] committedFlags 2 committedBCells [beam:2:b:0,beam:3:b:0] eventCount 6"
    ));
    let txn2_head_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-txn2-head-links-chula.txt"),
    )
    .expect("expected-only transaction-2 B17 fixture");
    let txn2_head_result = txn2_head_text
        .lines()
        .find(|line| {
            line.starts_with("stemsbeamvlinkheadlinksresult ")
                && line.contains(" system 1 plan 152 scope real case - ")
        })
        .expect("transaction-2 B17 result");
    assert!(txn2_head_result.contains(
        "headEntries 2 duplicateEntries 0 relationsInserted 2 sWriteCount 2 sValueChangeCount 2 consistencyWriteCount 2 headAbnormalChangeCount 2 stemAbnormalChangeCount 1 dirtyCascadeCount 3"
    ));
    assert!(sides_pass_text.lines().any(|line| {
        line.starts_with("stemsbeamsidesloopstep ")
            && line.contains(
                " system 1 step 2 beamSig 22 bAlias beam:22:b:0 vSide TOP plan 618 stemProfile 4 linked true",
            )
    }));

    let txn2_base_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-txn2-base-apply-chula.txt"),
    )
    .expect("expected-only transaction-2 B14 fixture");
    let txn2_result = txn2_base_text
        .lines()
        .find(|line| {
            line.starts_with("stemsbeamvlinkbaseapplyresult ")
                && line.contains(" system 1 plan 152 scope real case - ")
        })
        .expect("transaction-2 B14 result");
    assert!(txn2_result.contains(
        "branch NewIdZero retainedDraftIdentity draft:152 retainedDraftGrade 0x1.ee2b2e53080fp-1/3feee2b2e53080f0 applyReturn true graphRelationIdentity sig-edge:207"
    ));
    assert!(txn2_result.contains(
        "finalStemInterId 2341 finalStemSigAttached true finalStemVip false finalStemAbnormal true finalBeamAbnormal false"
    ));

    let mut invalid_scheduler = hydrated.scheduler.clone();
    invalid_scheduler.status =
        audiveris_omr::native_stems_beam_scheduler::NativeStemsBeamSchedulerStatus::Completed {
            retained_for_stumps: Vec::new(),
            final_local_worklist: Vec::new(),
        };
    let before_failed_resume = cells.clone();
    assert!(
        apply_native_stems_beam_outer_and_resume_transaction(
            &invalid_scheduler,
            &hydrated.vlinkers,
            &hydrated.builder,
            &hydrated.plans,
            &hydrated.reachability,
            &hydrated.transaction,
            &actual,
            &head_actual,
            &mut cells,
        )
        .is_err()
    );
    assert_eq!(cells, before_failed_resume);

    // A late second-entry binding failure proves the first provisional S write,
    // edge, and abnormal callbacks cannot escape the clone-and-swap carrier.
    let mut rollback_sig = post_b16_sig.clone();
    let mut rollback_s_cells = pre_b17_s_cells.clone();
    let mut invalid_bindings = bindings.clone();
    let second_head = head_actual.steps[1].corner.head;
    invalid_bindings.head_vertices.remove(&second_head);
    assert!(
        apply_native_stems_beam_vlink_head_transaction_to_native_sig(
            &mut rollback_sig,
            &invalid_bindings,
            &hydrated.scheduler,
            &hydrated.plans,
            &hydrated.builder,
            &hydrated.head_corners,
            &hydrated.reachability,
            &hydrated.transaction,
            &actual,
            &cells,
            &mut rollback_s_cells,
        )
        .is_err()
    );
    assert_eq!(rollback_sig, post_b16_sig);
    assert_eq!(rollback_s_cells, pre_b17_s_cells);

    // Boundary-17 is expected-only from here onward. Compare the frozen
    // native-domain facts without importing Java aliases or persistent IDs.
    let head_text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-vlink-head-links-chula.txt"),
    )
    .expect("frozen B17 fixture");
    let real = |prefix: &str| {
        head_text
            .lines()
            .filter(|line| {
                line.starts_with(prefix) && line.contains(" system 1 plan 143 scope real case - ")
            })
            .collect::<Vec<_>>()
    };
    let entries = real("stemsbeamvlinkheadlinksheadentry ");
    let edges = real("stemsbeamvlinkheadlinksedge ");
    let callbacks = real("stemsbeamvlinkheadlinkscallback ");
    assert_eq!((entries.len(), edges.len(), callbacks.len()), (2, 2, 2));
    for (ordinal, (native, java)) in head_actual.steps.iter().zip(entries).enumerate() {
        assert!(java.contains(&format!(" mapOrdinal {ordinal} ")));
        assert!(java.contains(&format!(" headVertexOrdinal {} ", native.head_vertex.0)));
        assert!(java.contains(" sLinkedBefore false sClosedBefore false "));
        assert!(java.contains(" draftHeadSide LEFT "));
    }
    for (ordinal, (native, java)) in head_actual.steps.iter().zip(edges).enumerate() {
        assert!(java.contains(&format!(" mapOrdinal {ordinal} ")));
        assert!(java.contains(&format!(
            " graphInsertionOrdinal {} ",
            native.appended_edge.expect("linked edge").0
        )));
        assert!(java.contains(&format!(" sourceVertexOrdinal {} ", native.head_vertex.0)));
        assert!(java.contains(" targetVertexOrdinal 221 "));
    }
    assert!(callbacks[0].contains(
        " headAbnormalBefore true headAbnormalAfter false stemAbnormalBefore true stemAbnormalAfter false "
    ));
    assert!(callbacks[1].contains(
        " headAbnormalBefore true headAbnormalAfter false stemAbnormalBefore false stemAbnormalAfter false "
    ));
    let result = real("stemsbeamvlinkheadlinksresult ");
    assert_eq!(result.len(), 1);
    assert!(result[0].contains(
        " headEntries 2 duplicateEntries 0 relationsInserted 2 sWriteCount 2 sValueChangeCount 2 consistencyWriteCount 2 headAbnormalChangeCount 2 stemAbnormalChangeCount 1 dirtyCascadeCount 3 "
    ));

    // From this point onward the frozen B16 fixture is expected-only.
    let text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-vlink-sibling-links-chula.txt"),
    )
    .expect("frozen B16 fixture");
    let rows = parse_scaffold_fixture(&text).expect("B16 fixture parses");
    let transactions = validate_core_rows(&rows).expect("B16 core rows");
    let expected = transactions
        .iter()
        .find(|transaction| transaction.key.system == 1)
        .expect("system 1 B16 rows");
    let expected_members = expected
        .rows
        .iter()
        .filter(|row| row.kind == RowKind::GroupMember)
        .collect::<Vec<_>>();
    assert_eq!(expected_members.len(), actual.group_members.len());
    for (native, java) in actual.group_members.iter().zip(expected_members) {
        assert_eq!(native.member_ordinal, java.usize("memberOrdinal").unwrap());
        assert_eq!(
            native.cross,
            parse_point(java.value("verticalCross").unwrap()).unwrap()
        );
        assert_eq!(
            native.left_limit.to_bits(),
            parse_f64(java.value("leftLimit").unwrap())
                .unwrap()
                .to_bits()
        );
        assert_eq!(
            native.right_limit.to_bits(),
            parse_f64(java.value("rightLimit").unwrap())
                .unwrap()
                .to_bits()
        );
        assert_eq!(native.selected, java.bool("selected").unwrap());
        assert_eq!(
            native.sorted_ordinal,
            parse_optional_usize(java.value("sortedOrdinal").unwrap()).unwrap()
        );
        assert_eq!(native.removed_as_base, java.bool("baseIdentity").unwrap());
    }
    for native in &actual.siblings {
        let geometry =
            sibling_transaction_rows(expected, RowKind::Geometry, native.sibling_ordinal)
                .expect("one geometry row");
        let [geometry] = geometry.as_slice() else {
            panic!("expected one geometry row");
        };
        assert_eq!(
            native.geometry.as_ref().expect("linked geometry"),
            &expected_geometry_from_row(geometry).expect("expected geometry")
        );
    }
    assert!(actual.group_state_before[0].abnormal);
    assert!(!actual.group_state_after[0].abnormal);
    assert!(actual.group_state_before[1].abnormal);
    assert!(actual.group_state_after[1].abnormal);

    let sig_ordinal = hydrated
        .stumps
        .beams_by_abscissa
        .iter()
        .map(|beam| (beam.source, beam.sig_ordinal))
        .collect::<BTreeMap<_, _>>();
    let alias = |reference: NativeStemsBeamBLinkerRef| {
        format!(
            "beam:{}:b:{}",
            sig_ordinal[&reference.beam],
            reference.id - 1
        )
    };
    assert_eq!(alias(actual.base_b_linker), "beam:12:b:0");
    assert_eq!(
        actual
            .assigned_b_linkers
            .iter()
            .copied()
            .map(alias)
            .collect::<Vec<_>>(),
        vec!["beam:0:b:0", "beam:1:b:0"]
    );

    // Even malformed persistent-cell authority leaves both carrier products
    // byte-for-byte at the supplied pre-call state.
    let mut rollback_sig = post_b14_sig.clone();
    let mut rollback_cells = pre_b15_cells.clone();
    let missing = actual.assigned_b_linkers[1];
    rollback_cells.retain(|cell| cell.reference != missing);
    let invalid_before = rollback_cells.clone();
    assert!(
        apply_native_stems_beam_vlink_sibling_transaction_to_native_sig(
            &mut rollback_sig,
            &bindings,
            &hydrated.scheduler,
            &hydrated.stumps,
            &hydrated.vlinkers,
            &hydrated.reachability,
            &hydrated.builder,
            &native_base,
            &hydrated.transaction,
            &mut rollback_cells,
        )
        .is_err()
    );
    assert_eq!(rollback_sig, post_b14_sig);
    assert_eq!(rollback_cells, invalid_before);
}
