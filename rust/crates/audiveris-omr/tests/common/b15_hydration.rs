use super::{repo_root, sha256_hex};
use std::{collections::BTreeMap, fmt::Write as _};

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
        NativeStemsBeamBuilderRecognition, NativeStemsBeamBuilderSystem,
        materialize_native_stems_beam_builders,
    },
    native_stems_beam_link_plans::{
        NativeStemsBeamLinkPlanAttempt, NativeStemsBeamLinkPlanRecognition,
        NativeStemsBeamLinkPlanSystem, materialize_native_stems_beam_link_plans,
    },
    native_stems_beam_reachability::{
        NativeStemsBeamReachabilityRecognition, NativeStemsBeamReachabilitySystem,
        materialize_native_stems_beam_reachability,
    },
    native_stems_beam_scheduler::{
        NativeStemsBeamSchedulerRecognition, NativeStemsBeamSchedulerStatus,
        NativeStemsBeamSchedulerSystem, materialize_native_stems_beam_scheduler_frontiers,
    },
    native_stems_beam_stumps::{
        NativeStemsBeamStumpRecognition, NativeStemsBeamStumpSystem,
        materialize_native_stems_beam_stumps,
    },
    native_stems_beam_vlink_b_linker_flag::{
        NativeStemsBeamVLinkBLinkerFlagState, NativeStemsBeamVLinkBLinkerFlagTransaction,
        apply_native_stems_beam_vlink_b_linker_flag_transaction,
    },
    native_stems_beam_vlink_base_apply::{
        NativeStemsBeamBeamIncidentRead, NativeStemsBeamBeamIncidentRelation,
        NativeStemsBeamBeamIncidentRule, NativeStemsBeamBeamIncidentScan,
        NativeStemsBeamDirectedPairRelation, NativeStemsBeamDirectedPairScan,
        NativeStemsBeamGroupRuntimeState, NativeStemsBeamIncidentDirection,
        NativeStemsBeamIncidentOpposite, NativeStemsBeamInterIndexAppend,
        NativeStemsBeamInterIndexApplyState, NativeStemsBeamInterIndexLookup,
        NativeStemsBeamNextPersistentIdLookup, NativeStemsBeamPairClassRead,
        NativeStemsBeamQueryProvenance, NativeStemsBeamQueryRelationKind,
        NativeStemsBeamRelationObjectIdentity, NativeStemsBeamSheetEditState,
        NativeStemsBeamSigApplyState, NativeStemsBeamSigListenerTopology,
        NativeStemsBeamSigRelationKind, NativeStemsBeamSigRelationState,
        NativeStemsBeamSigVertexAppend, NativeStemsBeamSigVertexLookup,
        NativeStemsBeamStemIncidentRelation, NativeStemsBeamStemIncidentScan,
        NativeStemsBeamStemIncidentScanState, NativeStemsBeamVLinkBaseApplyCertificate,
        NativeStemsBeamVLinkBaseApplyDisposition, NativeStemsBeamVLinkBaseApplyKey,
        NativeStemsBeamVLinkBaseApplyOperation, NativeStemsBeamVLinkBaseApplyOutcome,
        NativeStemsBeamVLinkBaseApplyState, NativeStemsBeamVLinkBeamAbnormalTrace,
        NativeStemsBeamVLinkBeamRuntimeState, NativeStemsBeamVLinkStemRuntimeState,
        NativeStemsBeamVLinkVertexAction, apply_native_stems_beam_vlink_base_transaction,
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
        NativeStemsBeamBLinkerRef, NativeStemsBeamVLinkerRecognition, NativeStemsBeamVLinkerRef,
        NativeStemsBeamVLinkerSystem, materialize_native_stems_beam_vlinkers,
    },
    native_stems_head_builders::materialize_native_stems_head_builders,
    native_stems_head_corner_reachability::materialize_native_stems_head_corner_reachability,
    native_stems_head_corners::{
        NativeStemsHeadCornerRecognition, NativeStemsHeadCornerSystem,
        materialize_native_stems_head_corners,
    },
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RowKind {
    Page,
    Baseline,
    Frontier,
    PredecessorCompat,
    VertexTrace,
    ApplyDecision,
    DuplicateScan,
    EdgeStruct,
    StemIncident,
    RelationCallback,
    BeamIncident,
    Result,
}

impl RowKind {
    fn parse(label: &str) -> Option<Self> {
        let suffix = label.strip_prefix("stemsbeamvlinkbaseapply")?;
        Some(match suffix {
            "page" => Self::Page,
            "baseline" => Self::Baseline,
            "frontier" => Self::Frontier,
            "predecessorcompat" => Self::PredecessorCompat,
            "vertextrace" => Self::VertexTrace,
            "applydecision" => Self::ApplyDecision,
            "duplicatescan" => Self::DuplicateScan,
            "edgestruct" => Self::EdgeStruct,
            "stemincident" => Self::StemIncident,
            "relationcallback" => Self::RelationCallback,
            "beamincident" => Self::BeamIncident,
            "result" => Self::Result,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StrictRow {
    kind: RowKind,
    page: String,
    fields: BTreeMap<String, String>,
}

impl StrictRow {
    fn parse(line: &str) -> Result<Option<Self>, String> {
        let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
        let Some(kind) = tokens.first().and_then(|label| RowKind::parse(label)) else {
            return Ok(None);
        };
        if tokens.len() < 2 || (tokens.len() - 2) % 2 != 0 {
            return Err("malformed boundary-14 hydration row".to_owned());
        }
        let mut fields = BTreeMap::new();
        for pair in tokens[2..].chunks_exact(2) {
            if fields
                .insert(pair[0].to_owned(), pair[1].to_owned())
                .is_some()
            {
                return Err(format!("boundary-14 row repeats {}", pair[0]));
            }
        }
        Ok(Some(Self {
            kind,
            page: tokens[1].to_owned(),
            fields,
        }))
    }

    fn value(&self, field: &str) -> Result<&str, String> {
        self.fields
            .get(field)
            .map(String::as_str)
            .ok_or_else(|| format!("boundary-14 {:?} lacks {field}", self.kind))
    }

    fn system_id(&self) -> Result<usize, String> {
        parse_usize(self, "system")
    }
}

fn parse_real_rows(text: &str, system_id: usize) -> Result<(StrictRow, Vec<StrictRow>), String> {
    let mut page = None;
    let mut rows = Vec::new();
    for line in text.lines() {
        let Some(row) = StrictRow::parse(line)? else {
            continue;
        };
        if row.kind == RowKind::Page {
            if page.replace(row).is_some() {
                return Err("duplicate boundary-14 page row".to_owned());
            }
            continue;
        }
        if row
            .fields
            .get("system")
            .and_then(|value| value.parse().ok())
            == Some(system_id)
            && row.fields.get("scope").map(String::as_str) == Some("real")
            && row.fields.get("case").map(String::as_str) == Some("-")
        {
            rows.push(row);
        }
    }
    Ok((
        page.ok_or_else(|| "boundary-14 fixture lacks page row".to_owned())?,
        rows,
    ))
}

fn one_row(rows: &[StrictRow], kind: RowKind) -> Result<&StrictRow, String> {
    let matches = rows
        .iter()
        .filter(|row| row.kind == kind)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [row] => Ok(row),
        _ => Err(format!(
            "expected one boundary-14 {kind:?}, found {}",
            matches.len()
        )),
    }
}

fn rows_of_kind(rows: &[StrictRow], kind: RowKind) -> Vec<&StrictRow> {
    rows.iter().filter(|row| row.kind == kind).collect()
}

fn parse_usize(row: &StrictRow, field: &str) -> Result<usize, String> {
    row.value(field)?
        .parse()
        .map_err(|_| format!("invalid boundary-14 usize {field}"))
}

fn parse_i32(row: &StrictRow, field: &str) -> Result<i32, String> {
    row.value(field)?
        .parse()
        .map_err(|_| format!("invalid boundary-14 i32 {field}"))
}

fn parse_bool(row: &StrictRow, field: &str) -> Result<bool, String> {
    match row.value(field)? {
        "true" => Ok(true),
        "false" => Ok(false),
        value => Err(format!("invalid boundary-14 bool {field}: {value}")),
    }
}

fn parse_sig_edge_alias(value: &str) -> Result<usize, String> {
    value
        .strip_prefix("sig-edge:")
        .ok_or_else(|| format!("invalid SIG edge alias {value}"))?
        .parse()
        .map_err(|_| format!("invalid SIG edge alias {value}"))
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

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

pub(super) struct NativePredecessorPage {
    pub(super) grid: GridLinesRecognition,
    pub(super) stem_seeds: NativeStemSeedRecognition,
    pub(super) beam_stumps: NativeStemsBeamStumpRecognition,
    pub(super) beam_vlinkers: NativeStemsBeamVLinkerRecognition,
    pub(super) head_corners: NativeStemsHeadCornerRecognition,
    pub(super) beam_reachability: NativeStemsBeamReachabilityRecognition,
    pub(super) beam_builders: NativeStemsBeamBuilderRecognition,
    pub(super) plans: NativeStemsBeamLinkPlanRecognition,
    pub(super) scheduler: NativeStemsBeamSchedulerRecognition,
    // The historical all-page B15/B17 replay does not require an assembled
    // SIG, and some wider systems still lack complete native BEAMS groups.
    // Later-carriage gates require this explicitly and fail if it is absent.
    #[allow(dead_code)]
    pub(super) sig: Option<NativeSigRecognition>,
}

pub(super) fn native_predecessor_page(image: &str) -> NativePredecessorPage {
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
    let sig = assemble_native_sig(&grid, &headers, &beams, &ledgers, &heads).ok();
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
        stem_seeds,
        beam_stumps,
        beam_vlinkers,
        head_corners: corners,
        beam_reachability,
        beam_builders,
        plans,
        scheduler,
        sig,
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

/// The glyph-registry bootstrap's join key. The Java probe computes this exact
/// digest for every glyph in the page registry, so content matches across the
/// two sides without either trusting the other's identities.
// Used by the Boundary-20 gate's self-driving path; the other gates that
// include this file have no page registry to join.
#[allow(dead_code)]
pub(super) fn run_table_digest(table: &RunTable) -> String {
    run_table_sha256(table)
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

/// Exposed for the self-drive spike: the checker context is a page property,
/// identical at every frontier, so a chained transaction can reuse it.
#[allow(dead_code)]
pub(super) fn checker_context_for_page(
    page: &NativePredecessorPage,
) -> NativeStemsBeamStemCheckerContext {
    create_stem_checker_context(page)
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
    })
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

/// Boundary 15 reads the trailing projection fields; Boundary 16 also consumes
/// the predecessor products retained at the front of this shared bundle.
#[allow(dead_code)]
pub(super) struct HydratedBoundaryFifteen {
    pub scheduler: NativeStemsBeamSchedulerSystem,
    pub plans: NativeStemsBeamLinkPlanSystem,
    pub stumps: NativeStemsBeamStumpSystem,
    pub vlinkers: NativeStemsBeamVLinkerSystem,
    pub head_corners: NativeStemsHeadCornerSystem,
    pub reachability: NativeStemsBeamReachabilitySystem,
    pub builder: NativeStemsBeamBuilderSystem,
    pub create_transaction: NativeStemsBeamVLinkTransaction,
    pub reuse_live_state: NativeStemsBeamVLinkReuseLiveState,
    pub relation_parameters: NativeStemsBeamRelationParameters,
    pub reuse_check: NativeStemsBeamVLinkReuseCheck,
    pub transaction: NativeStemsBeamVLinkBLinkerFlagTransaction,
    pub state_before: NativeStemsBeamVLinkBLinkerFlagState,
    pub state_after: NativeStemsBeamVLinkBLinkerFlagState,
    pub base_apply:
        audiveris_omr::native_stems_beam_vlink_base_apply::NativeStemsBeamVLinkBaseApplyTransaction,
    pub target: NativeStemsBeamBLinkerRef,
    pub triggering: NativeStemsBeamVLinkerRef,
    pub ordered_observers: Vec<NativeStemsBeamVLinkerRef>,
}

#[allow(dead_code)] // The Boundary-16/17 gates call this; the txn2 gate uses run_real_on_page.
pub(super) fn run_real(
    image: &str,
    system_id: usize,
    base_apply_text: &str,
    create_text: &str,
    reuse_text: &str,
    linked_before: bool,
) -> Result<HydratedBoundaryFifteen, String> {
    let page = native_predecessor_page(image);
    run_real_on_page(
        &page,
        system_id,
        base_apply_text,
        create_text,
        reuse_text,
        linked_before,
    )
}

/// Boundary-20 entry: identical to `run_real` but on a caller-supplied page whose
/// scheduler status may already be advanced to a later frontier.
pub(super) fn run_real_on_page(
    page: &NativePredecessorPage,
    system_id: usize,
    base_apply_text: &str,
    create_text: &str,
    reuse_text: &str,
    linked_before: bool,
) -> Result<HydratedBoundaryFifteen, String> {
    let create_fixture = PredecessorFixture::parse(create_text, CREATE_STEM_SCHEMA)?;
    let reuse_fixture = PredecessorFixture::parse(reuse_text, REUSE_CHECK_SCHEMA)?;
    let (oracle_page, rows) = parse_real_rows(base_apply_text, system_id)?;
    let predecessor = replay_boundary_thirteen(page, &create_fixture, &reuse_fixture, system_id)?;
    validate_boundary_thirteen_continuity(&rows, &reuse_fixture)?;
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
    let reachability = page
        .beam_reachability
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or_else(|| format!("missing reachability system {system_id}"))?;
    let builder = page
        .beam_builders
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or_else(|| format!("missing builder system {system_id}"))?;
    let frontier = match &scheduler.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => return Err(format!("system {system_id} lacks V transaction frontier")),
    };
    let mut base_state = build_real_base_state(page, &oracle_page, &rows, &predecessor)?;
    let base_state_before = base_state.clone();
    let base_apply = apply_native_stems_beam_vlink_base_transaction(
        scheduler,
        plans,
        stumps,
        vlinkers,
        &predecessor.create_transaction,
        &predecessor.live_state,
        predecessor.relation_parameters,
        &predecessor.reuse_check,
        &mut base_state,
    )
    .map_err(|error| format!("system {system_id} boundary-14 replay failed: {error}"))?;
    if base_apply.state_after.as_ref() != &base_state {
        return Err(format!(
            "system {system_id} boundary-14 returned state differs"
        ));
    }
    let expected_key = NativeStemsBeamVLinkBaseApplyKey {
        system_id: predecessor.reuse_check.system_id,
        invocation_ordinal: predecessor.reuse_check.invocation_ordinal,
        plan: predecessor.reuse_check.plan,
    };
    let NativeStemsBeamVLinkReuseCheckOutcome::ReadyBeforeSigMutation {
        relation: expected_relation,
    } = &predecessor.reuse_check.outcome
    else {
        return Err(format!(
            "system {system_id} boundary-13 predecessor is not ready"
        ));
    };
    assert_production_matches_java(
        &rows,
        &base_apply,
        expected_key,
        expected_relation,
        &base_state_before,
        &base_state,
    )?;
    let target = frontier.b_linker;
    let triggering = frontier.v_linker;
    let target_matches = vlinkers
        .constructors
        .iter()
        .flat_map(|constructor| &constructor.b_linkers)
        .filter(|linker| linker.reference == target)
        .collect::<Vec<_>>();
    let [target_linker] = target_matches.as_slice() else {
        return Err(format!("system {system_id} target B cardinality differs"));
    };
    let ordered_observers = target_linker
        .v_linkers
        .iter()
        .map(|linker| linker.reference)
        .collect::<Vec<_>>();
    let mut state = NativeStemsBeamVLinkBLinkerFlagState {
        system_id,
        base_apply_state_before: base_state_before,
        target_b_linker: target,
        linked: linked_before,
        committed: None,
    };
    let state_before = state.clone();
    let transaction = apply_native_stems_beam_vlink_b_linker_flag_transaction(
        scheduler,
        plans,
        stumps,
        vlinkers,
        &predecessor.create_transaction,
        &predecessor.live_state,
        predecessor.relation_parameters,
        &predecessor.reuse_check,
        &base_apply,
        &mut state,
    )
    .map_err(|error| format!("system {system_id} boundary-15 apply failed: {error}"))?;
    Ok(HydratedBoundaryFifteen {
        scheduler: scheduler.clone(),
        plans: plans.clone(),
        stumps: stumps.clone(),
        vlinkers: vlinkers.clone(),
        head_corners: page
            .head_corners
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .ok_or_else(|| format!("system {system_id} head corners missing"))?
            .clone(),
        reachability: reachability.clone(),
        builder: builder.clone(),
        create_transaction: predecessor.create_transaction,
        reuse_live_state: predecessor.live_state,
        relation_parameters: predecessor.relation_parameters,
        reuse_check: predecessor.reuse_check,
        transaction,
        state_before,
        state_after: state,
        base_apply,
        target,
        triggering,
        ordered_observers,
    })
}
