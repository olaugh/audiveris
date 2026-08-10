// Shared tests-only Boundary-16 row-to-native hydration and public replay mapper.
//
// This file is textually included by the Boundary-16 and Boundary-17 integration
// gates so the frozen sibling-links fixture has one projector implementation.

fn parse_optional_string(value: &str) -> Option<String> {
    (value != "-").then(|| value.to_owned())
}

fn parse_sig_edge(value: &str) -> Result<usize, String> {
    value
        .strip_prefix("sig-edge:")
        .ok_or_else(|| format!("invalid SIG edge identity {value}"))?
        .parse()
        .map_err(|error| format!("invalid SIG edge identity {value}: {error}"))
}

fn parse_relation_object(
    value: &str,
) -> Result<NativeStemsBeamSiblingRelationObjectIdentity, String> {
    if let Some(identity) = value.strip_prefix("sig-relation-object:") {
        return identity
            .parse()
            .map(NativeStemsBeamSiblingRelationObjectIdentity::GraphObject)
            .map_err(|error| format!("invalid graph relation object {value}: {error}"));
    }
    if let Some(plan) = value.strip_prefix("base-draft:") {
        return plan
            .parse()
            .map(NativeStemsBeamSiblingRelationObjectIdentity::BaseDraft)
            .map_err(|error| format!("invalid base draft object {value}: {error}"));
    }
    if let Some(suffix) = value.strip_prefix("sibling-draft:") {
        let values = suffix.split(':').collect::<Vec<_>>();
        let [plan, sibling] = values.as_slice() else {
            return Err(format!("invalid sibling draft object {value}"));
        };
        return Ok(NativeStemsBeamSiblingRelationObjectIdentity::SiblingDraft {
            plan_ordinal: plan
                .parse()
                .map_err(|error| format!("invalid sibling draft plan {value}: {error}"))?,
            sibling_ordinal: sibling
                .parse()
                .map_err(|error| format!("invalid sibling draft ordinal {value}: {error}"))?,
        });
    }
    Err(format!("unknown relation object identity {value}"))
}

fn parse_portion(value: &str) -> Result<Option<NativeBeamPortion>, String> {
    Ok(match value {
        "-" => None,
        "LEFT" => Some(NativeBeamPortion::Left),
        "CENTER" => Some(NativeBeamPortion::Center),
        "RIGHT" => Some(NativeBeamPortion::Right),
        _ => return Err(format!("invalid beam portion {value}")),
    })
}

fn parse_direction(value: &str) -> Result<NativeStemsBeamIncidentDirection, String> {
    match value {
        "Incoming" => Ok(NativeStemsBeamIncidentDirection::Incoming),
        "Outgoing" => Ok(NativeStemsBeamIncidentDirection::Outgoing),
        _ => Err(format!("invalid incident direction {value}")),
    }
}

fn coarse_relation_kind(class: &str) -> NativeStemsBeamQueryRelationKind {
    match class {
        "org.audiveris.omr.sig.relation.BeamStemRelation" => {
            NativeStemsBeamQueryRelationKind::BeamStem
        }
        "org.audiveris.omr.sig.relation.BeamRestRelation" => {
            NativeStemsBeamQueryRelationKind::BeamRest
        }
        "org.audiveris.omr.sig.relation.ChordStemRelation" => {
            NativeStemsBeamQueryRelationKind::ChordStem
        }
        _ => NativeStemsBeamQueryRelationKind::Other,
    }
}

fn beam_source_from_alias(
    alias: &str,
    hydrated: &b15_hydration::HydratedBoundaryFifteen,
) -> Result<NativeStemsBeamSource, String> {
    let sig_ordinal = alias
        .strip_prefix("beam:")
        .ok_or_else(|| format!("invalid beam alias {alias}"))?
        .parse::<usize>()
        .map_err(|error| format!("invalid beam alias {alias}: {error}"))?;
    let matches = hydrated
        .stumps
        .beams_by_abscissa
        .iter()
        .filter(|beam| beam.sig_ordinal == sig_ordinal)
        .collect::<Vec<_>>();
    let [beam] = matches.as_slice() else {
        return Err(format!(
            "beam alias {alias} has {} stump sig_ordinal matches",
            matches.len()
        ));
    };
    Ok(beam.source)
}

fn parse_b_linker_alias(
    alias: &str,
    hydrated: &b15_hydration::HydratedBoundaryFifteen,
) -> Result<NativeStemsBeamBLinkerRef, String> {
    let (beam_alias, ordinal) = alias
        .rsplit_once(":b:")
        .ok_or_else(|| format!("invalid B-linker alias {alias}"))?;
    let ordinal = ordinal
        .parse::<usize>()
        .map_err(|error| format!("invalid B-linker alias {alias}: {error}"))?;
    Ok(NativeStemsBeamBLinkerRef {
        beam: beam_source_from_alias(beam_alias, hydrated)?,
        id: ordinal
            .checked_add(1)
            .ok_or_else(|| format!("B-linker ordinal overflow in {alias}"))?,
    })
}

fn parse_v_linker_alias(
    alias: &str,
    hydrated: &b15_hydration::HydratedBoundaryFifteen,
) -> Result<NativeStemsBeamVLinkerRef, String> {
    let (b_alias, side) = alias
        .rsplit_once(":v:")
        .ok_or_else(|| format!("invalid V-linker alias {alias}"))?;
    let side = match side {
        "TOP" => NativeStemVerticalSide::Top,
        "BOTTOM" => NativeStemVerticalSide::Bottom,
        value => return Err(format!("invalid V-linker side {value}")),
    };
    Ok(NativeStemsBeamVLinkerRef {
        b_linker: parse_b_linker_alias(b_alias, hydrated)?,
        side,
    })
}

fn glyph_identity(
    identity: &str,
    token: &str,
) -> Result<Option<NativeStemsBeamSiblingGlyphIdentity>, String> {
    match (identity, token) {
        ("null-glyph", "null") => Ok(None),
        (identity, token) => {
            let object_identity = identity
                .strip_prefix("group-glyph:")
                .ok_or_else(|| format!("invalid group glyph identity {identity}"))?
                .parse()
                .map_err(|error| format!("invalid group glyph identity {identity}: {error}"))?;
            if token == "null" || token.is_empty() {
                return Err(format!("non-null glyph identity {identity} lacks content"));
            }
            Ok(Some(NativeStemsBeamSiblingGlyphIdentity {
                object_identity,
                token: token.to_owned(),
            }))
        }
    }
}

fn only_transaction_row(
    transaction: &ParsedTransaction,
    kind: RowKind,
) -> Result<&StrictRow, String> {
    let matches = transaction
        .rows
        .iter()
        .filter(|row| row.kind == kind)
        .collect::<Vec<_>>();
    let [row] = matches.as_slice() else {
        return Err(format!(
            "transaction {:?} has {} {kind:?} rows",
            transaction.key,
            matches.len()
        ));
    };
    Ok(row)
}

fn sibling_transaction_rows(
    transaction: &ParsedTransaction,
    kind: RowKind,
    sibling_ordinal: usize,
) -> Result<Vec<&StrictRow>, String> {
    transaction
        .rows
        .iter()
        .filter(|row| row.kind == kind)
        .filter(|row| row.usize("siblingOrdinal") == Ok(sibling_ordinal))
        .map(Ok)
        .collect()
}

fn only_sibling_transaction_row(
    transaction: &ParsedTransaction,
    kind: RowKind,
    sibling_ordinal: usize,
) -> Result<&StrictRow, String> {
    let matches = sibling_transaction_rows(transaction, kind, sibling_ordinal)?;
    let [row] = matches.as_slice() else {
        return Err(format!(
            "transaction {:?} sibling {sibling_ordinal} has {} {kind:?} rows",
            transaction.key,
            matches.len()
        ));
    };
    Ok(row)
}

struct ProjectedRealGroup {
    base_glyph: Option<NativeStemsBeamSiblingGlyphIdentity>,
    scan: NativeStemsBeamSiblingGroupScan,
    live_members: Vec<NativeStemsBeamSiblingLiveBeam>,
}

fn query_provenance(value: &str) -> Result<NativeStemsBeamSiblingQueryProvenance, String> {
    if value == "NotRead" {
        Ok(NativeStemsBeamSiblingQueryProvenance::NotRead)
    } else if is_lower_sha256(value) {
        Ok(NativeStemsBeamSiblingQueryProvenance::ExhaustiveSha256(
            value.to_owned(),
        ))
    } else {
        Err(format!("invalid sibling query provenance {value}"))
    }
}

fn incident_opposite(alias: &str, stem_alias: &str) -> NativeStemsBeamIncidentOpposite {
    if alias.starts_with("beam:") {
        NativeStemsBeamIncidentOpposite::Beam
    } else if alias == stem_alias {
        NativeStemsBeamIncidentOpposite::Stem
    } else {
        NativeStemsBeamIncidentOpposite::OtherInter
    }
}

fn same_segment_bits(left: Segment, right: Segment) -> bool {
    left.x1.to_bits() == right.x1.to_bits()
        && left.y1.to_bits() == right.y1.to_bits()
        && left.x2.to_bits() == right.x2.to_bits()
        && left.y2.to_bits() == right.y2.to_bits()
}

fn project_real_group(
    transaction: &ParsedTransaction,
    hydrated: &b15_hydration::HydratedBoundaryFifteen,
) -> Result<ProjectedRealGroup, String> {
    let baseline = only_transaction_row(transaction, RowKind::Baseline)?;
    let base_glyph = glyph_identity(
        baseline.value("baseBeamGlyphIdentity")?,
        baseline.value("baseBeamGlyph")?,
    )?;
    let rows = transaction
        .rows
        .iter()
        .filter(|row| row.kind == RowKind::GroupMember)
        .collect::<Vec<_>>();
    let mut relations = Vec::with_capacity(rows.len());
    let mut members = Vec::new();
    let mut live_members = Vec::new();
    for row in rows {
        let containment = row.bool("containmentMatch")?;
        let target = if containment {
            NativeStemsBeamSiblingGroupTarget::Beam(beam_source_from_alias(
                row.value("targetAlias")?,
                hydrated,
            )?)
        } else {
            NativeStemsBeamSiblingGroupTarget::OtherInter
        };
        relations.push(NativeStemsBeamSiblingGroupRelation {
            outgoing_ordinal: row.usize("groupOutgoingOrdinal")?,
            graph_relation_identity: parse_sig_edge(row.value("graphRelationIdentity")?)?,
            relation_object_identity: parse_relation_object(row.value("relationObjectIdentity")?)?,
            relation_class: row.value("relationClass")?.to_owned(),
            containment_match: containment,
            target,
            target_read_by_get_members: row.bool("targetReadByGetMembers")?,
            target_evidence: match row.value("targetEvidence")? {
                "GetMembersRead" => NativeStemsBeamSiblingGroupTargetEvidence::GetMembersRead,
                "GraphReconstruction" => {
                    NativeStemsBeamSiblingGroupTargetEvidence::GraphReconstruction
                }
                value => return Err(format!("invalid group target evidence {value}")),
            },
            target_alias: row.value("targetAlias")?.to_owned(),
            target_class: row.value("targetRuntimeClass")?.to_owned(),
            target_inter_id: parse_i32_field(row, "targetInterId")?,
            target_vertex_identity: row.usize("targetVertexOrdinal")?,
            member_ordinal: parse_optional_usize(row.value("memberOrdinal")?)?,
        });
        if !containment {
            continue;
        }
        let NativeStemsBeamSiblingGroupTarget::Beam(source) = target else {
            unreachable!("containment target projected as beam")
        };
        let stump_matches = hydrated
            .stumps
            .beams_by_abscissa
            .iter()
            .filter(|beam| beam.source == source)
            .collect::<Vec<_>>();
        let [stump] = stump_matches.as_slice() else {
            return Err(format!(
                "group source {source:?} has {} stump rows",
                stump_matches.len()
            ));
        };
        let emitted_median = parse_segment(row.value("median")?)?;
        if !same_segment_bits(emitted_median, stump.median)
            || parse_f64(row.value("height")?)?.to_bits() != stump.height.to_bits()
        {
            return Err(format!(
                "group member {} stump geometry differs",
                row.value("targetAlias")?
            ));
        }
        let beam_group = NativeStemsBeamGroupRuntimeState {
            sig_vertex_ordinal: row.usize("beamGroupVertexOrdinal")?,
            state_sha256: row.value("beamGroupStateHash")?.to_owned(),
        };
        let runtime = NativeStemsBeamVLinkBeamRuntimeState {
            source,
            sig_vertex_identity: Some(row.usize("targetVertexOrdinal")?),
            inter_id: parse_i32_field(row, "beamInterId")?,
            inter_indexed: row.bool("sigMembership")?,
            sig_system_id: row.usize("sigSystemId")?,
            removed: row.bool("beamRemoved")?,
            vip: row.bool("beamVip")?,
            abnormal: row.bool("beamAbnormal")?,
            stump_group_ordinal: stump.group_ordinal,
            beam_group: Some(beam_group),
        };
        let member_glyph = glyph_identity(row.value("glyphIdentity")?, row.value("glyph")?)?;
        live_members.push(NativeStemsBeamSiblingLiveBeam {
            source,
            alias: row.value("targetAlias")?.to_owned(),
            runtime,
            inter_index_ordinal: row.usize("interIndexOrdinal")?,
            inter_index_object_matches: row.usize("interIndexObjectMatches")?,
            inter_index_id_matches: row.usize("interIndexIdMatches")?,
            glyph: member_glyph,
        });
        let cross = parse_point(row.value("verticalCross")?)?;
        let left_limit = parse_f64(row.value("leftLimit")?)?;
        let right_limit = parse_f64(row.value("rightLimit")?)?;
        let inclusive_left = left_limit <= cross.x;
        let inclusive_right = cross.x <= right_limit;
        if row.bool("inclusiveLeft")? != inclusive_left
            || row.bool("inclusiveRight")? != inclusive_right
            || row.bool("selected")? != (inclusive_left && inclusive_right)
        {
            return Err(format!(
                "group member {} inclusive selection differs",
                row.value("targetAlias")?
            ));
        }
        members.push(NativeStemsBeamSiblingGroupMemberTrace {
            member_ordinal: row.usize("memberOrdinal")?,
            source,
            cross,
            left_limit,
            right_limit,
            selected: row.bool("selected")?,
            sorted_ordinal: parse_optional_usize(row.value("sortedOrdinal")?)?,
            removed_as_base: row.bool("baseIdentity")?
                && row.value("removeAction")? == "RemoveFirstBase",
        });
    }
    let base_source = beam_source_from_alias(baseline.value("baseBeamAlias")?, hydrated)?;
    let base_member = live_members
        .iter()
        .find(|member| member.source == base_source)
        .ok_or_else(|| "base beam is absent from projected live group".to_owned())?;
    if base_member.runtime != hydrated.base_apply.state_after.sig.beam
        || base_member.glyph != base_glyph
    {
        return Err("projected base live member differs from Boundary-14 state".to_owned());
    }
    Ok(ProjectedRealGroup {
        base_glyph,
        scan: NativeStemsBeamSiblingGroupScan {
            query_relation_count: baseline.usize("groupOutgoingScanned")?,
            query_provenance_sha256: baseline.value("groupQueryProvenanceSha256")?.to_owned(),
            relations,
            members,
        },
        live_members,
    })
}

fn project_pair_scan(
    transaction: &ParsedTransaction,
    sibling: &StrictRow,
) -> Result<NativeStemsBeamSiblingPairScan, String> {
    let sibling_ordinal = sibling.usize("siblingOrdinal")?;
    let source_rows =
        sibling_transaction_rows(transaction, RowKind::SourceOutgoing, sibling_ordinal)?;
    let pair_rows = sibling_transaction_rows(transaction, RowKind::PairRelation, sibling_ordinal)?;
    let source_outgoing_relations = source_rows
        .iter()
        .map(|row| {
            Ok(NativeStemsBeamSiblingSourceOutgoingRelation {
                source_outgoing_ordinal: row.usize("sourceOutgoingOrdinal")?,
                graph_relation_identity: parse_sig_edge(row.value("graphRelationIdentity")?)?,
                relation_object_identity: parse_relation_object(
                    row.value("relationObjectIdentity")?,
                )?,
                relation_class: row.value("runtimeClass")?.to_owned(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let relations = pair_rows
        .iter()
        .map(|row| {
            let class_read = match row.value("action")? {
                "Continue" => NativeStemsBeamSiblingPairClassRead::ExaminedContinue,
                "SelectBreak" => NativeStemsBeamSiblingPairClassRead::ExaminedMatchBreak,
                "UnreadAfterBreak" => NativeStemsBeamSiblingPairClassRead::UnreadAfterBreak,
                value => return Err(format!("invalid pair action {value}")),
            };
            Ok(NativeStemsBeamSiblingPairRelation {
                pair_ordinal: row.usize("pairOrdinal")?,
                source_outgoing_ordinal: row.usize("sourceOutgoingOrdinal")?,
                graph_relation_identity: parse_sig_edge(row.value("graphRelationIdentity")?)?,
                relation_object_identity: parse_relation_object(
                    row.value("relationObjectIdentity")?,
                )?,
                relation_class: row.value("runtimeClass")?.to_owned(),
                kind: coarse_relation_kind(row.value("runtimeClass")?),
                class_read,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(NativeStemsBeamSiblingPairScan {
        source_outgoing_scanned: sibling.usize("sourceOutgoingScanned")?,
        source_outgoing_provenance: query_provenance(
            sibling.value("sourceOutgoingProvenanceSha256")?,
        )?,
        source_outgoing_relations,
        query_relation_count: sibling.usize("pairRows")?,
        pair_provenance: query_provenance(sibling.value("pairProvenanceSha256")?)?,
        relations,
    })
}

fn project_stem_incident_scan(
    transaction: &ParsedTransaction,
    sibling_ordinal: usize,
    callback: &StrictRow,
    stem_alias: &str,
) -> Result<NativeStemsBeamSiblingStemIncidentScan, String> {
    if callback.value("stemIncidentState")? != "ExhaustiveIncomingThenOutgoingAtCallback" {
        return Err("real sibling callback stem incident state differs".to_owned());
    }
    let rows = sibling_transaction_rows(transaction, RowKind::StemIncident, sibling_ordinal)?;
    let relations = rows
        .iter()
        .map(|row| {
            let opposite_alias = row.value("oppositeAlias")?;
            Ok(NativeStemsBeamSiblingStemIncidentRelation {
                incident_ordinal: row.usize("incidentOrdinal")?,
                direction: parse_direction(row.value("direction")?)?,
                direction_ordinal: row.usize("directionOrdinal")?,
                graph_relation_identity: parse_sig_edge(row.value("graphRelationIdentity")?)?,
                relation_object_identity: parse_relation_object(
                    row.value("relationObjectIdentity")?,
                )?,
                relation_class: row.value("runtimeClass")?.to_owned(),
                kind: coarse_relation_kind(row.value("runtimeClass")?),
                opposite_vertex_identity: row.usize("oppositeVertexOrdinal")?,
                opposite: incident_opposite(opposite_alias, stem_alias),
                opposite_alias: opposite_alias.to_owned(),
                opposite_inter_id: parse_i32_field(row, "oppositeInterId")?,
                chord_stem_match: row.bool("chordStemMatch")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(NativeStemsBeamSiblingStemIncidentScan {
        query_relation_count: callback.usize("stemIncidentRows")?,
        query_provenance_sha256: callback.value("stemIncidentHash")?.to_owned(),
        relations,
    })
}

fn project_beam_incident_scan(
    transaction: &ParsedTransaction,
    sibling_ordinal: usize,
    callback: &StrictRow,
    edge: &StrictRow,
    stem_alias: &str,
) -> Result<NativeStemsBeamSiblingBeamIncidentScan, String> {
    let rows = sibling_transaction_rows(transaction, RowKind::BeamIncident, sibling_ordinal)?;
    let fresh_graph_identity = parse_sig_edge(edge.value("graphRelationIdentity")?)?;
    let fresh_portion = parse_portion(edge.value("portion")?)?
        .ok_or_else(|| "fresh sibling edge lacks beam portion".to_owned())?;
    let relations = rows
        .iter()
        .map(|row| {
            let graph_relation_identity = parse_sig_edge(row.value("graphRelationIdentity")?)?;
            let projected_portion = parse_portion(row.value("portion")?)?;
            let relation_class = row.value("runtimeClass")?;
            let kind_portion = if graph_relation_identity == fresh_graph_identity
                && relation_class == "org.audiveris.omr.sig.relation.BeamStemRelation"
            {
                Some(fresh_portion)
            } else {
                projected_portion
            };
            let kind = match relation_class {
                "org.audiveris.omr.sig.relation.BeamStemRelation" => {
                    NativeStemsBeamSigRelationKind::BeamStem {
                        beam_portion: kind_portion,
                    }
                }
                "org.audiveris.omr.sig.relation.BeamRestRelation" => {
                    NativeStemsBeamSigRelationKind::BeamRest {
                        beam_portion: kind_portion,
                    }
                }
                "org.audiveris.omr.sig.relation.ChordStemRelation" => {
                    NativeStemsBeamSigRelationKind::ChordStem
                }
                _ => NativeStemsBeamSigRelationKind::Other,
            };
            let opposite_alias = row.value("oppositeAlias")?;
            Ok(NativeStemsBeamSiblingBeamIncidentRelation {
                incident_ordinal: row.usize("incidentOrdinal")?,
                direction: parse_direction(row.value("direction")?)?,
                direction_ordinal: row.usize("directionOrdinal")?,
                graph_relation_identity,
                relation_object_identity: parse_relation_object(
                    row.value("relationObjectIdentity")?,
                )?,
                relation_class: relation_class.to_owned(),
                kind,
                opposite_vertex_identity: row.usize("oppositeVertexOrdinal")?,
                opposite: incident_opposite(opposite_alias, stem_alias),
                opposite_alias: opposite_alias.to_owned(),
                opposite_inter_id: parse_i32_field(row, "oppositeInterId")?,
                read: match row.value("readState")? {
                    "ExaminedClassOnly" | "ExaminedClassAndPortion" => {
                        NativeStemsBeamBeamIncidentRead::Examined
                    }
                    "UnreadAfterBreak" => NativeStemsBeamBeamIncidentRead::UnreadAfterBreak,
                    value => return Err(format!("invalid beam incident read state {value}")),
                },
                relevant: row.bool("relevant")?,
                beam_portion: projected_portion,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let (rule, expected_state) = match callback.value("beamRule")? {
        "HookHasAnyBeamStem" => (
            NativeStemsBeamBeamIncidentRule::HookHasAnyBeamStem,
            "LazyIncomingThenOutgoing",
        ),
        "FullBeamNeedsLeftAndRight" => (
            NativeStemsBeamBeamIncidentRule::RawBeamLeftAndRight,
            "ExhaustiveIncomingThenOutgoing",
        ),
        value => return Err(format!("invalid sibling beam rule {value}")),
    };
    if callback.value("beamIncidentState")? != expected_state {
        return Err("real sibling callback beam incident state differs".to_owned());
    }
    Ok(NativeStemsBeamSiblingBeamIncidentScan {
        rule,
        query_relation_count: callback.usize("beamIncidentRows")?,
        query_provenance_sha256: callback.value("beamIncidentHash")?.to_owned(),
        relations,
    })
}

fn real_builder(
    hydrated: &b15_hydration::HydratedBoundaryFifteen,
) -> Result<&NativeStemsBeamBuilder, String> {
    let matches = hydrated
        .builder
        .builders
        .iter()
        .filter(|builder| builder.start == hydrated.triggering)
        .collect::<Vec<_>>();
    let [builder] = matches.as_slice() else {
        return Err(format!(
            "triggering V-linker has {} builder matches",
            matches.len()
        ));
    };
    Ok(builder)
}

fn project_builder_lookup(
    transaction: &ParsedTransaction,
    sibling_ordinal: usize,
    sibling_result: &StrictRow,
    flag: &StrictRow,
    hydrated: &b15_hydration::HydratedBoundaryFifteen,
    builder: &NativeStemsBeamBuilder,
) -> Result<NativeStemsBeamSiblingBuilderLookupScan, String> {
    let rows = sibling_transaction_rows(transaction, RowKind::LinkerLookup, sibling_ordinal)?;
    if rows.len() != builder.items.len() {
        return Err(format!(
            "sibling {sibling_ordinal} lookup rows {} differ from builder items {}",
            rows.len(),
            builder.items.len()
        ));
    }
    let projected_rows = rows
        .iter()
        .zip(&builder.items)
        .map(|(row, item)| {
            let action = match row.value("action")? {
                "Continue" => NativeStemsBeamSiblingBuilderAction::Continue,
                "SelectBreak" => NativeStemsBeamSiblingBuilderAction::SelectBreak,
                "UnreadAfterBreak" => NativeStemsBeamSiblingBuilderAction::UnreadAfterBreak,
                value => return Err(format!("invalid builder action {value}")),
            };
            let linker_read = match row.value("linkerRead")? {
                "NotRead" => NativeStemsBeamSiblingBuilderLinkerRead::NotRead,
                "NotLinkerItem" => NativeStemsBeamSiblingBuilderLinkerRead::NotLinkerItem,
                "ReadLinker" => NativeStemsBeamSiblingBuilderLinkerRead::ReadLinker,
                value => return Err(format!("invalid builder linker read {value}")),
            };
            let source_read = match row.value("sourceRead")? {
                "NotRead" => NativeStemsBeamSiblingBuilderSourceRead::NotRead,
                "ReadSource" => NativeStemsBeamSiblingBuilderSourceRead::ReadSource,
                value => return Err(format!("invalid builder source read {value}")),
            };
            let read = match (action, linker_read) {
                (NativeStemsBeamSiblingBuilderAction::UnreadAfterBreak, _) => {
                    NativeStemsBeamSiblingBuilderItemRead::UnreadAfterBreak
                }
                (_, NativeStemsBeamSiblingBuilderLinkerRead::NotLinkerItem) => {
                    NativeStemsBeamSiblingBuilderItemRead::NotALinker
                }
                (NativeStemsBeamSiblingBuilderAction::SelectBreak, _) => {
                    NativeStemsBeamSiblingBuilderItemRead::ExaminedSelectBreak
                }
                (NativeStemsBeamSiblingBuilderAction::Continue, _) => {
                    NativeStemsBeamSiblingBuilderItemRead::ExaminedContinue
                }
            };
            let linker = if linker_read == NativeStemsBeamSiblingBuilderLinkerRead::ReadLinker {
                Some(match item.kind {
                    NativeStemsBeamBuilderItemKind::StartHalfLinker => {
                        NativeStemsBeamSiblingBuilderLinkerIdentity::StartVLinker
                    }
                    NativeStemsBeamBuilderItemKind::BeamLinker => {
                        let Some(NativeStemsBeamBuilderTargetRef::Beam(reference)) = item.target
                        else {
                            return Err("BeamLinker item lacks beam target".to_owned());
                        };
                        NativeStemsBeamSiblingBuilderLinkerIdentity::BeamBLinker(reference)
                    }
                    NativeStemsBeamBuilderItemKind::HeadHalfLinker => {
                        NativeStemsBeamSiblingBuilderLinkerIdentity::HeadCLinker
                    }
                    NativeStemsBeamBuilderItemKind::SeedGlyph
                    | NativeStemsBeamBuilderItemKind::ChunkGlyph
                    | NativeStemsBeamBuilderItemKind::Gap => {
                        return Err("non-linker builder item was read as linker".to_owned());
                    }
                })
            } else {
                None
            };
            let source_beam = if source_read == NativeStemsBeamSiblingBuilderSourceRead::ReadSource
            {
                match item.kind {
                    NativeStemsBeamBuilderItemKind::StartHalfLinker => {
                        Some(builder.start.b_linker.beam)
                    }
                    NativeStemsBeamBuilderItemKind::BeamLinker => {
                        let Some(NativeStemsBeamBuilderTargetRef::Beam(reference)) = item.target
                        else {
                            return Err("BeamLinker item lacks beam source".to_owned());
                        };
                        Some(reference.beam)
                    }
                    NativeStemsBeamBuilderItemKind::HeadHalfLinker => None,
                    NativeStemsBeamBuilderItemKind::SeedGlyph
                    | NativeStemsBeamBuilderItemKind::ChunkGlyph
                    | NativeStemsBeamBuilderItemKind::Gap => {
                        return Err("non-linker builder item read a source".to_owned());
                    }
                }
            } else {
                None
            };
            Ok(NativeStemsBeamSiblingBuilderLookupRow {
                item_ordinal: row.usize("itemOrdinal")?,
                item_kind: item.kind,
                linker,
                source_beam,
                read,
                runtime_class: parse_optional_string(row.value("runtimeClass")?),
                linker_read,
                source_read,
                linker_alias: parse_optional_string(row.value("linkerAlias")?),
                linker_runtime_class: parse_optional_string(row.value("linkerRuntimeClass")?),
                source_alias: parse_optional_string(row.value("sourceAlias")?),
                source_inter_id: parse_optional_i32(row.value("sourceInterId")?)?,
                identity_match: parse_optional_bool(row.value("identityMatch")?)?,
                action,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let selected_b_linker = match flag.value("selectedAlias")? {
        "-" => None,
        alias => Some(parse_b_linker_alias(alias, hydrated)?),
    };
    let state = match flag.value("lookupState")? {
        "FirstSourceIdentityMatch" => {
            NativeStemsBeamSiblingBuilderLookupState::FirstSourceIdentityMatch
        }
        "ExhaustiveNoMatch" => NativeStemsBeamSiblingBuilderLookupState::ExhaustiveNoMatch,
        value => return Err(format!("invalid builder lookup state {value}")),
    };
    Ok(NativeStemsBeamSiblingBuilderLookupScan {
        state,
        timing: NativeStemsBeamSiblingBuilderLookupTiming::ReconstructedFromImmutableItems,
        query_item_count: sibling_result.usize("linkerLookupRows")?,
        query_provenance_sha256: sibling_result.value("linkerLookupHash")?.to_owned(),
        rows: projected_rows,
        selected_b_linker,
        selected_alias: parse_optional_string(flag.value("selectedAlias")?),
    })
}

struct ProjectedRealSteps {
    steps: Vec<NativeStemsBeamSiblingStepCertificate>,
    initial_b_linker_cells: Vec<NativeStemsBeamSiblingBLinkerCell>,
}

fn project_real_steps(
    transaction: &ParsedTransaction,
    hydrated: &b15_hydration::HydratedBoundaryFifteen,
) -> Result<ProjectedRealSteps, String> {
    let builder = real_builder(hydrated)?;
    let stem_alias = transaction.predecessor.value("stemAlias")?;
    let siblings = transaction
        .rows
        .iter()
        .filter(|row| row.kind == RowKind::Sibling)
        .collect::<Vec<_>>();
    let mut steps = Vec::with_capacity(siblings.len());
    let mut initial_cells = Vec::new();
    let mut serial_cells: Vec<NativeStemsBeamSiblingBLinkerCell> = Vec::new();
    for sibling in siblings {
        let sibling_ordinal = sibling.usize("siblingOrdinal")?;
        let source = beam_source_from_alias(sibling.value("beamAlias")?, hydrated)?;
        let directed_pair = project_pair_scan(transaction, sibling)?;
        let linked = sibling.value("branch")? == "Linked";
        let (stem_incident_after, beam_incident_after, chord_stem_matches, builder_lookup) =
            if linked {
                let edge =
                    only_sibling_transaction_row(transaction, RowKind::Edge, sibling_ordinal)?;
                let callback =
                    only_sibling_transaction_row(transaction, RowKind::Callback, sibling_ordinal)?;
                let sibling_result = only_sibling_transaction_row(
                    transaction,
                    RowKind::SiblingResult,
                    sibling_ordinal,
                )?;
                let flag = only_sibling_transaction_row(
                    transaction,
                    RowKind::LinkerFlag,
                    sibling_ordinal,
                )?;
                let lookup = project_builder_lookup(
                    transaction,
                    sibling_ordinal,
                    sibling_result,
                    flag,
                    hydrated,
                    builder,
                )?;
                if let Some(reference) = lookup.selected_b_linker {
                    let linked_before = flag.bool("linkedBefore")?;
                    let closed_before = flag.bool("closedBefore")?;
                    let closed_after = flag.bool("closedAfter")?;
                    if closed_before != closed_after {
                        return Err(format!(
                            "sibling {sibling_ordinal} B-linker closed state changed"
                        ));
                    }
                    if let Some(cell) = serial_cells
                        .iter_mut()
                        .find(|cell| cell.reference == reference)
                    {
                        if cell.linked != linked_before || cell.closed != closed_before {
                            return Err(format!(
                                "sibling {sibling_ordinal} serial B-cell before-state differs"
                            ));
                        }
                        cell.linked = true;
                    } else {
                        let initial = NativeStemsBeamSiblingBLinkerCell {
                            reference,
                            linked: linked_before,
                            closed: closed_before,
                        };
                        initial_cells.push(initial.clone());
                        serial_cells.push(NativeStemsBeamSiblingBLinkerCell {
                            linked: true,
                            ..initial
                        });
                    }
                }
                (
                    Some(project_stem_incident_scan(
                        transaction,
                        sibling_ordinal,
                        callback,
                        stem_alias,
                    )?),
                    Some(project_beam_incident_scan(
                        transaction,
                        sibling_ordinal,
                        callback,
                        edge,
                        stem_alias,
                    )?),
                    callback.usize("chordStemMatches")?,
                    Some(lookup),
                )
            } else {
                (None, None, 0, None)
            };
        steps.push(NativeStemsBeamSiblingStepCertificate {
            sibling_ordinal,
            source,
            directed_pair,
            stem_incident_after,
            beam_incident_after,
            chord_stem_matches,
            builder_lookup,
        });
    }
    Ok(ProjectedRealSteps {
        steps,
        initial_b_linker_cells: initial_cells,
    })
}

fn validate_real_baseline_geometry(
    baseline: &StrictRow,
    hydrated: &b15_hydration::HydratedBoundaryFifteen,
) -> Result<(), String> {
    let b_matches = hydrated
        .vlinkers
        .constructors
        .iter()
        .flat_map(|constructor| &constructor.b_linkers)
        .filter(|linker| linker.reference == hydrated.target)
        .collect::<Vec<_>>();
    let [b_linker] = b_matches.as_slice() else {
        return Err(format!(
            "base B-linker has {} constructor matches",
            b_matches.len()
        ));
    };
    let v_matches = b_linker
        .v_linkers
        .iter()
        .filter(|linker| linker.reference == hydrated.triggering)
        .collect::<Vec<_>>();
    let [v_linker] = v_matches.as_slice() else {
        return Err(format!(
            "triggering V-linker has {} child matches",
            v_matches.len()
        ));
    };
    let base_source = beam_source_from_alias(baseline.value("baseBeamAlias")?, hydrated)?;
    let stump_matches = hydrated
        .stumps
        .beams_by_abscissa
        .iter()
        .filter(|beam| beam.source == base_source)
        .collect::<Vec<_>>();
    let [base_stump] = stump_matches.as_slice() else {
        return Err(format!(
            "base beam has {} stump matches",
            stump_matches.len()
        ));
    };
    let stem_line = &hydrated.transaction.stem_after.geometry.median;
    let stem_segment = Segment {
        x1: stem_line.start.x,
        y1: stem_line.start.y,
        x2: stem_line.stop.x,
        y2: stem_line.stop.y,
    };
    let expected_vertical = Segment {
        x1: b_linker.reference_point.x,
        y1: b_linker.reference_point.y,
        x2: b_linker.reference_point.x - (1_000.0 * hydrated.reachability.global_slope),
        y2: b_linker.reference_point.y + 1_000.0,
    };
    let base_runtime = &hydrated.base_apply.state_after.sig.beam;
    if parse_point(baseline.value("refPt")?)? != b_linker.reference_point
        || parse_i32_field(baseline, "yDir")? != v_linker.y_direction
        || !same_segment_bits(
            parse_segment(baseline.value("skewedVertical")?)?,
            expected_vertical,
        )
        || !same_segment_bits(parse_segment(baseline.value("stemMedian")?)?, stem_segment)
        || !same_segment_bits(
            parse_segment(baseline.value("baseBeamMedian")?)?,
            base_stump.median,
        )
        || parse_f64(baseline.value("baseBeamHeight")?)?.to_bits() != base_stump.height.to_bits()
        || parse_i32_field(baseline, "baseBeamInterId")? != base_runtime.inter_id
        || baseline.usize("baseBeamVertexOrdinal")?
            != base_runtime
                .sig_vertex_identity
                .ok_or_else(|| "base beam lacks live vertex".to_owned())?
        || baseline.bool("baseBeamAbnormal")? != base_runtime.abnormal
        || baseline.bool("stemAbnormal")? != hydrated.transaction.stem_after.abnormal
        || parse_i32_field(baseline, "interline")? != hydrated.stumps.interline
        || parse_i32_field(baseline, "maxBeamSideDx")? != hydrated.reachability.max_beam_side_dx
    {
        return Err("real baseline geometry/runtime projection differs".to_owned());
    }
    Ok(())
}

fn project_real_state(
    page: &StrictRow,
    transaction: &ParsedTransaction,
    hydrated: &b15_hydration::HydratedBoundaryFifteen,
) -> Result<NativeStemsBeamVLinkSiblingLinksState, String> {
    let baseline = only_transaction_row(transaction, RowKind::Baseline)?;
    validate_real_baseline_geometry(baseline, hydrated)?;
    let result = only_transaction_row(transaction, RowKind::Result)?;
    let group = project_real_group(transaction, hydrated)?;
    let projected_steps = project_real_steps(transaction, hydrated)?;
    let cached_base_median = parse_segment(baseline.value("baseBeamMedian")?)?;
    let group_runtime = NativeStemsBeamSiblingGroupRuntimeState {
        alias: baseline.value("groupAlias")?.to_owned(),
        runtime_class: baseline.value("groupClass")?.to_owned(),
        inter_id: parse_i32_field(baseline, "groupInterId")?,
        sig_vertex_identity: baseline.usize("groupVertexOrdinal")?,
        removed: baseline.bool("groupRemoved")?,
        vip: baseline.bool("groupVip")?,
        abnormal: baseline.bool("groupAbnormal")?,
        member_state_sha256_before: baseline.value("groupStateHashBefore")?.to_owned(),
        member_state_sha256: baseline.value("groupStateHashBefore")?.to_owned(),
        object_state_sha256: baseline.value("groupObjectStateHash")?.to_owned(),
    };
    let certificate = NativeStemsBeamVLinkSiblingLinksCertificate {
        system_id: transaction.key.system,
        headless: page.bool("headless")?,
        listener_topology: NativeStemsBeamSigListenerTopology::SoleStandardSigListener,
        interline: parse_i32_field(baseline, "interline")?,
        x_in_gap_maximum_profile0: parse_f64(baseline.value("xInGapMaximum0")?)?,
        portion_maximum_dx: parse_i32_field(baseline, "maxDxRint")?,
        max_beam_side_dx: parse_i32_field(baseline, "maxBeamSideDx")?,
        max_shorter_ratio: parse_f64(baseline.value("maxShorterRatio")?)?,
        base_glyph: group.base_glyph.clone(),
        group_scan: group.scan,
        expected_group_member_state_sha256_after: result.value("groupStateHashAfter")?.to_owned(),
        steps: projected_steps.steps,
    };
    let base_apply_state_after = hydrated.base_apply.state_after.as_ref().clone();
    let sheet_edit = base_apply_state_after.sheet_edit;
    Ok(NativeStemsBeamVLinkSiblingLinksState {
        b_linker_flag_state_before: hydrated.state_before.clone(),
        b_linker_flag_state_after: hydrated.state_after.clone(),
        base_apply_state_after,
        cached_base_median,
        cached_base_median_same_identity: baseline.bool("cachedMedianSameIdentity")?,
        group_runtime,
        base_glyph: group.base_glyph,
        stem_alias: baseline.value("stemAlias")?.to_owned(),
        live_group_members: group.live_members,
        sibling_b_linker_cells: projected_steps.initial_b_linker_cells,
        appended_relations: Vec::new(),
        sheet_edit,
        certificate: Some(certificate),
        committed: None,
    })
}

fn expected_geometry_from_row(
    row: &StrictRow,
) -> Result<NativeStemsBeamSiblingGeometryTrace, String> {
    let linked = row.value("branch")? == "Linked";
    let dy_read = row.bool("dyRead")?;
    Ok(NativeStemsBeamSiblingGeometryTrace {
        base_cross: parse_point(row.value("baseCross")?)?,
        sibling_cross: parse_point(row.value("siblingCross")?)?,
        base_length: parse_f64(row.value("baseLength")?)?,
        sibling_length: parse_f64(row.value("siblingLength")?)?,
        length_ratio: parse_f64(row.value("ratio")?)?,
        shorter_or_equal: row.bool("shorterInclusive")?,
        delta_y: dy_read
            .then(|| parse_f64(row.value("dy").expect("dy field")))
            .transpose()?,
        directed_delta_y: dy_read
            .then(|| parse_f64(row.value("product").expect("product field")))
            .transpose()?,
        wrong_side: if row.value("wrongSideStrict")? == "-" {
            None
        } else {
            Some(row.bool("wrongSideStrict")?)
        },
        extension_point: linked
            .then(|| parse_point(row.value("extension").expect("extension field")))
            .transpose()?,
        portion_maximum_dx: linked
            .then(|| parse_i32_field(row, "maxDxRint"))
            .transpose()?,
        left_threshold: linked
            .then(|| parse_f64(row.value("leftThreshold").expect("left threshold field")))
            .transpose()?,
        right_threshold: linked
            .then(|| parse_f64(row.value("rightThreshold").expect("right threshold field")))
            .transpose()?,
        beam_portion: if linked {
            parse_portion(row.value("portion")?)?
        } else {
            None
        },
        support_grade: linked
            .then(|| parse_f64(row.value("grade").expect("grade field")))
            .transpose()?,
    })
}

fn expected_beam_abnormal_trace(
    transaction: &ParsedTransaction,
    sibling_ordinal: usize,
    callback: &StrictRow,
) -> Result<NativeStemsBeamSiblingBeamAbnormalTrace, String> {
    let rows = sibling_transaction_rows(transaction, RowKind::BeamIncident, sibling_ordinal)?;
    let before = callback.bool("beamAbnormalBefore")?;
    let after = callback.bool("beamAbnormalAfter")?;
    Ok(match callback.value("beamRule")? {
        "HookHasAnyBeamStem" => NativeStemsBeamSiblingBeamAbnormalTrace::HookAnyBeamStem {
            incident_relation_count: rows.len(),
            relations_read: rows
                .iter()
                .filter(|row| row.value("readState") != Ok("UnreadAfterBreak"))
                .count(),
            before,
            after,
        },
        "FullBeamNeedsLeftAndRight" => NativeStemsBeamSiblingBeamAbnormalTrace::RawBeamSides {
            incident_relation_count: rows.len(),
            left_found: rows
                .iter()
                .any(|row| row.value("contribution") == Ok("Left")),
            right_found: rows
                .iter()
                .any(|row| row.value("contribution") == Ok("Right")),
            before,
            after,
        },
        value => return Err(format!("invalid sibling beam rule {value}")),
    })
}

fn expected_observers(
    flag: &StrictRow,
    hydrated: &b15_hydration::HydratedBoundaryFifteen,
) -> Result<Vec<NativeStemsBeamVLinkerRef>, String> {
    let selected = flag.value("selectedAlias")?;
    let aliases = parse_list(flag.value("observerAliases")?)?;
    if aliases.first().copied() != Some(selected) {
        return Err("B-linker observer list lacks its parent first".to_owned());
    }
    aliases[1..]
        .iter()
        .map(|alias| parse_v_linker_alias(alias, hydrated))
        .collect()
}

fn sheet_edit_token(state: NativeStemsBeamSheetEditState) -> String {
    format!(
        "{}:{}:{}",
        state.stub_modified, state.book_modified, state.book_dirty
    )
}

fn assert_public_transaction_matches_rows(
    transaction: &ParsedTransaction,
    hydrated: &b15_hydration::HydratedBoundaryFifteen,
    state_before: &NativeStemsBeamVLinkSiblingLinksState,
    state_after: &NativeStemsBeamVLinkSiblingLinksState,
    public: &NativeStemsBeamVLinkSiblingLinksTransaction,
) -> Result<(), String> {
    let baseline = only_transaction_row(transaction, RowKind::Baseline)?;
    let result = only_transaction_row(transaction, RowKind::Result)?;
    let summary = only_transaction_row(transaction, RowKind::Summary)?;
    let consumed = state_before
        .certificate
        .as_ref()
        .ok_or_else(|| "projected real state lacks a certificate".to_owned())?;
    let base_source = beam_source_from_alias(baseline.value("baseBeamAlias")?, hydrated)?;
    if public.key != hydrated.transaction.key
        || public.key.system_id != transaction.key.system
        || public.key.plan.plan_ordinal != transaction.key.plan
        || public.stem_after != hydrated.transaction.stem_after
        || public.state_after.as_ref() != state_after
        || public.consumed_certificate != *consumed
        || public.base_beam != base_source
        || !same_segment_bits(
            public.cached_base_median,
            parse_segment(baseline.value("baseBeamMedian")?)?,
        )
        || public.cached_base_median_same_identity != baseline.bool("cachedMedianSameIdentity")?
        || public.continuation_support_grade.to_bits()
            != parse_f64(transaction.predecessor.value("supportGrade")?)?.to_bits()
        || public.base_cross != parse_point(baseline.value("baseCross")?)?
        || public.base_length.to_bits() != parse_f64(baseline.value("baseLength")?)?.to_bits()
        || public.group_members != consumed.group_scan.members
        || public.group_member_state_sha256_before != baseline.value("groupStateHashBefore")?
        || public.group_member_state_sha256_after != result.value("groupStateHashAfter")?
        || public.group_runtime.member_state_sha256_before
            != baseline.value("groupStateHashBefore")?
        || public.group_runtime.member_state_sha256 != result.value("groupStateHashAfter")?
        || public.group_runtime != state_after.group_runtime
        || public.siblings.len() != baseline.usize("siblings")?
        || public.sig_relation_mutation_count != result.usize("committedEdges")?
        || public.sibling_link_mutation_count != result.usize("committedEdges")?
        || public.b_linker_write_count != result.usize("committedFlags")?
        || public.head_link_mutation_count != 0
    {
        return Err(
            "public Boundary-16 transaction header/state differs from Java rows".to_owned(),
        );
    }
    match public.outcome {
        NativeStemsBeamVLinkSiblingLinksOutcome::ReadyBeforeHeadRelationLoop {
            stem_identity,
            continuation_support_grade,
        } if stem_identity == hydrated.transaction.stem_after.stem_identity
            && continuation_support_grade.to_bits()
                == public.continuation_support_grade.to_bits() => {}
        _ => return Err("public Boundary-16 terminal differs from Java rows".to_owned()),
    }

    let sibling_rows = transaction
        .rows
        .iter()
        .filter(|row| row.kind == RowKind::Sibling)
        .collect::<Vec<_>>();
    let expected_sibling_sources = sibling_rows
        .iter()
        .map(|row| beam_source_from_alias(row.value("beamAlias")?, hydrated))
        .collect::<Result<Vec<_>, String>>()?;
    if public.sibling_sources != expected_sibling_sources {
        return Err("public sibling source order differs from Java rows".to_owned());
    }

    let mut expected_operations = Vec::new();
    let mut expected_relations = Vec::new();
    let mut expected_appended_ids = Vec::new();
    let mut expected_assigned = Vec::new();
    let mut expected_edge_aliases = Vec::new();
    let mut expected_b_aliases = Vec::new();
    let mut expected_value_changes = 0;
    let mut expected_abnormal_changes = 0;
    let mut expected_cells_after = state_before.sibling_b_linker_cells.clone();
    let mut expected_live_after = state_before.live_group_members.clone();
    let mut expected_sheet_edit = state_before.sheet_edit;
    for (sibling, (row, step)) in public
        .siblings
        .iter()
        .zip(sibling_rows.iter().zip(&consumed.steps))
    {
        let sibling_ordinal = row.usize("siblingOrdinal")?;
        let expected_branch = match row.value("branch")? {
            "SameGlyph" => NativeStemsBeamSiblingBranch::SameGlyph,
            "ExistingBeamStem" => NativeStemsBeamSiblingBranch::ExistingBeamStem,
            "ShorterWrongSide" => NativeStemsBeamSiblingBranch::ShorterWrongSide,
            "Linked" => NativeStemsBeamSiblingBranch::Linked,
            value => return Err(format!("invalid sibling branch {value}")),
        };
        let first_match = step
            .directed_pair
            .relations
            .iter()
            .position(|relation| relation.kind == NativeStemsBeamQueryRelationKind::BeamStem);
        let expected_pair_reads =
            first_match.map_or(step.directed_pair.relations.len(), |index| index + 1);
        if sibling.sibling_ordinal != sibling_ordinal
            || sibling.source != step.source
            || sibling.branch != expected_branch
            || sibling.same_glyph_identity != row.bool("baseGlyphSameIdentity")?
            || sibling.directed_pair_relations_read != expected_pair_reads
            || sibling.builder_lookup != step.builder_lookup
        {
            return Err(format!(
                "public sibling {sibling_ordinal} branch/pair/lookup trace differs"
            ));
        }
        if expected_branch == NativeStemsBeamSiblingBranch::SameGlyph
            || expected_branch == NativeStemsBeamSiblingBranch::ExistingBeamStem
        {
            if sibling.geometry.is_some()
                || sibling.relation.is_some()
                || sibling.beam_abnormal != NativeStemsBeamSiblingBeamAbnormalTrace::NotRead
            {
                return Err(format!(
                    "public sibling {sibling_ordinal} eagerly projected a lazy branch"
                ));
            }
            continue;
        }
        let geometry_row =
            only_sibling_transaction_row(transaction, RowKind::Geometry, sibling_ordinal)?;
        if sibling.geometry.as_ref() != Some(&expected_geometry_from_row(geometry_row)?) {
            return Err(format!("public sibling {sibling_ordinal} geometry differs"));
        }
        if expected_branch == NativeStemsBeamSiblingBranch::ShorterWrongSide {
            if sibling.relation.is_some()
                || sibling.beam_abnormal != NativeStemsBeamSiblingBeamAbnormalTrace::NotRead
            {
                return Err(format!(
                    "public sibling {sibling_ordinal} wrong-side mutation differs"
                ));
            }
            continue;
        }

        let edge = only_sibling_transaction_row(transaction, RowKind::Edge, sibling_ordinal)?;
        let callback =
            only_sibling_transaction_row(transaction, RowKind::Callback, sibling_ordinal)?;
        let flag = only_sibling_transaction_row(transaction, RowKind::LinkerFlag, sibling_ordinal)?;
        let source_member = state_before
            .live_group_members
            .iter()
            .find(|member| member.source == sibling.source)
            .ok_or_else(|| format!("sibling {sibling_ordinal} lacks live member"))?;
        let relation = NativeStemsBeamSiblingAppendedRelation {
            graph_relation_identity: parse_sig_edge(edge.value("graphRelationIdentity")?)?,
            relation_object_identity: parse_relation_object(edge.value("freshRelationIdentity")?)?,
            source: sibling.source,
            source_vertex_identity: source_member
                .runtime
                .sig_vertex_identity
                .ok_or_else(|| "sibling lacks live vertex".to_owned())?,
            target_stem_identity: public.stem_after.stem_identity,
            target_vertex_identity: state_before
                .base_apply_state_after
                .sig
                .stem
                .sig_vertex_identity
                .ok_or_else(|| "target stem lacks live vertex".to_owned())?,
            extension_point: parse_point(edge.value("extension")?)?,
            beam_portion: parse_portion(edge.value("portion")?)?
                .ok_or_else(|| "linked edge lacks portion".to_owned())?,
            grade: parse_f64(edge.value("grade")?)?,
        };
        let stem_scan = step
            .stem_incident_after
            .as_ref()
            .ok_or_else(|| "linked step lacks stem scan".to_owned())?;
        let beam_scan = step
            .beam_incident_after
            .as_ref()
            .ok_or_else(|| "linked step lacks beam scan".to_owned())?;
        let fresh_stem_rows = stem_scan
            .relations
            .iter()
            .filter(|row| row.graph_relation_identity == relation.graph_relation_identity)
            .collect::<Vec<_>>();
        let fresh_beam_rows = beam_scan
            .relations
            .iter()
            .filter(|row| row.graph_relation_identity == relation.graph_relation_identity)
            .collect::<Vec<_>>();
        let ([fresh_stem], [fresh_beam]) = (fresh_stem_rows.as_slice(), fresh_beam_rows.as_slice())
        else {
            return Err(format!(
                "sibling {sibling_ordinal} fresh incidence cardinality differs"
            ));
        };
        if fresh_stem.direction != NativeStemsBeamIncidentDirection::Incoming
            || fresh_beam.direction != NativeStemsBeamIncidentDirection::Outgoing
            || fresh_stem.direction_ordinal != edge.usize("targetIncomingOrdinal")?
            || fresh_beam.direction_ordinal != edge.usize("sourceOutgoingOrdinal")?
        {
            return Err(format!(
                "sibling {sibling_ordinal} fresh local incidence ordinal differs"
            ));
        }
        if sibling.relation.as_ref() != Some(&relation)
            || sibling.stem_incident_graph_relation_identities
                != stem_scan
                    .relations
                    .iter()
                    .map(|row| row.graph_relation_identity)
                    .collect::<Vec<_>>()
            || sibling.beam_abnormal
                != expected_beam_abnormal_trace(transaction, sibling_ordinal, callback)?
        {
            return Err(format!(
                "public sibling {sibling_ordinal} edge/callback trace differs"
            ));
        }
        expected_relations.push(relation.clone());
        expected_appended_ids.push(relation.graph_relation_identity);
        expected_edge_aliases.push(edge.value("graphRelationIdentity")?);
        expected_operations.extend([
            NativeStemsBeamVLinkSiblingLinksOperation::SigGlobalRelationInserted {
                sibling_ordinal,
                graph_relation_identity: relation.graph_relation_identity,
            },
            NativeStemsBeamVLinkSiblingLinksOperation::BeamOutgoingRelationInserted {
                sibling_ordinal,
                graph_relation_identity: relation.graph_relation_identity,
            },
            NativeStemsBeamVLinkSiblingLinksOperation::StemIncomingRelationInserted {
                sibling_ordinal,
                graph_relation_identity: relation.graph_relation_identity,
            },
            NativeStemsBeamVLinkSiblingLinksOperation::SigEdgeEventDispatched {
                sibling_ordinal,
                graph_relation_identity: relation.graph_relation_identity,
            },
            NativeStemsBeamVLinkSiblingLinksOperation::StandardSigListenerEdgeCallbackStarted {
                sibling_ordinal,
            },
            NativeStemsBeamVLinkSiblingLinksOperation::BeamStemRelationCallbackStarted {
                sibling_ordinal,
            },
            NativeStemsBeamVLinkSiblingLinksOperation::StemChordIncidentScanCompleted {
                sibling_ordinal,
                incident_relation_count: callback.usize("stemIncidentRows")?,
                chord_stem_matches: callback.usize("chordStemMatches")?,
            },
        ]);
        let abnormal_before = callback.bool("beamAbnormalBefore")?;
        let abnormal_after = callback.bool("beamAbnormalAfter")?;
        let abnormal_changed = callback.bool("abnormalChanged")?;
        if callback.value("dirtyBefore")? != sheet_edit_token(expected_sheet_edit) {
            return Err(format!(
                "sibling {sibling_ordinal} dirty before-state differs"
            ));
        }
        let member = expected_live_after
            .iter_mut()
            .find(|member| member.source == sibling.source)
            .ok_or_else(|| format!("sibling {sibling_ordinal} abnormal member missing"))?;
        if member.runtime.abnormal != abnormal_before
            || callback.bool("requestedAbnormal")? != abnormal_after
            || abnormal_changed != (abnormal_before != abnormal_after)
        {
            return Err(format!(
                "sibling {sibling_ordinal} abnormal callback cursor differs"
            ));
        }
        member.runtime.abnormal = abnormal_after;
        if abnormal_changed {
            expected_abnormal_changes += 1;
            expected_sheet_edit.stub_modified = true;
            expected_sheet_edit.book_modified = true;
            expected_sheet_edit.book_dirty = true;
            expected_operations.extend([
                NativeStemsBeamVLinkSiblingLinksOperation::BeamAbnormalSet {
                    sibling_ordinal,
                    before: callback.bool("beamAbnormalBefore")?,
                    after: callback.bool("beamAbnormalAfter")?,
                },
                NativeStemsBeamVLinkSiblingLinksOperation::SheetStubModifiedSetTrue {
                    sibling_ordinal,
                },
                NativeStemsBeamVLinkSiblingLinksOperation::BookModifiedSetTrue { sibling_ordinal },
                NativeStemsBeamVLinkSiblingLinksOperation::BookDirtySetTrue { sibling_ordinal },
            ]);
        }
        if callback.value("dirtyAfter")? != sheet_edit_token(expected_sheet_edit) {
            return Err(format!(
                "sibling {sibling_ordinal} dirty after-state differs"
            ));
        }
        expected_operations.extend([
            NativeStemsBeamVLinkSiblingLinksOperation::BeamStemRelationCallbackCompleted {
                sibling_ordinal,
            },
            NativeStemsBeamVLinkSiblingLinksOperation::StandardSigListenerEdgeCallbackCompleted {
                sibling_ordinal,
            },
        ]);
        match flag.value("lookupState")? {
            "FirstSourceIdentityMatch" => {
                let reference = parse_b_linker_alias(flag.value("selectedAlias")?, hydrated)?;
                expected_assigned.push(reference);
                expected_b_aliases.push(flag.value("selectedAlias")?);
                expected_value_changes += flag.usize("valueChangeCount")?;
                let cell = expected_cells_after
                    .iter_mut()
                    .find(|cell| cell.reference == reference)
                    .ok_or_else(|| format!("selected sibling {sibling_ordinal} cell missing"))?;
                if cell.linked != flag.bool("linkedBefore")?
                    || cell.closed != flag.bool("closedBefore")?
                {
                    return Err(format!(
                        "sibling {sibling_ordinal} independent B-cell cursor differs"
                    ));
                }
                cell.linked = true;
                expected_operations.push(
                    NativeStemsBeamVLinkSiblingLinksOperation::BLinkerLinkedAssigned {
                        sibling_ordinal,
                        target: reference,
                        ordered_observer_v_linkers: expected_observers(flag, hydrated)?,
                        before: flag.bool("linkedBefore")?,
                        after: flag.bool("linkedAfter")?,
                        closed_before: flag.bool("closedBefore")?,
                        closed_after: flag.bool("closedAfter")?,
                    },
                );
                if sibling.selected_b_linker != Some(reference)
                    || sibling.linked_before != Some(flag.bool("linkedBefore")?)
                    || sibling.linked_after != Some(flag.bool("linkedAfter")?)
                    || sibling.closed_before != Some(flag.bool("closedBefore")?)
                    || sibling.closed_after != Some(flag.bool("closedAfter")?)
                {
                    return Err(format!(
                        "public sibling {sibling_ordinal} B-cell trace differs"
                    ));
                }
            }
            "ExhaustiveNoMatch" => {
                if sibling.selected_b_linker.is_some()
                    || sibling.linked_before.is_some()
                    || sibling.linked_after.is_some()
                    || sibling.closed_before.is_some()
                    || sibling.closed_after.is_some()
                {
                    return Err(format!(
                        "public sibling {sibling_ordinal} no-match flag trace differs"
                    ));
                }
            }
            value => return Err(format!("invalid sibling flag lookup state {value}")),
        }
    }

    for member in &mut expected_live_after {
        member
            .runtime
            .beam_group
            .as_mut()
            .ok_or_else(|| "expected live group member lacks group state".to_owned())?
            .state_sha256 = result.value("groupStateHashAfter")?.to_owned();
    }
    let sheet_edit_mutations =
        usize::from(state_before.sheet_edit.stub_modified != expected_sheet_edit.stub_modified)
            + usize::from(
                state_before.sheet_edit.book_modified != expected_sheet_edit.book_modified,
            )
            + usize::from(state_before.sheet_edit.book_dirty != expected_sheet_edit.book_dirty);
    let result_edge_aliases = parse_list(result.value("committedEdgeAliases")?)?;
    let result_b_aliases = parse_list(result.value("committedBCells")?)?;
    let mut expected_group_runtime = state_before.group_runtime.clone();
    expected_group_runtime.member_state_sha256 = result.value("groupStateHashAfter")?.to_owned();

    if public.operations != expected_operations
        || public.appended_graph_relation_identities != expected_appended_ids
        || public.assigned_b_linkers != expected_assigned
        || public.beam_abnormal_mutation_count != expected_abnormal_changes
        || public.sheet_edit_mutation_count != sheet_edit_mutations
        || public.b_linker_value_change_count != expected_value_changes
        || result_edge_aliases != expected_edge_aliases
        || result_b_aliases != expected_b_aliases
        || state_after.appended_relations != expected_relations
        || state_after.sibling_b_linker_cells != expected_cells_after
        || state_after.live_group_members != expected_live_after
        || state_after.sheet_edit != expected_sheet_edit
        || !same_segment_bits(
            state_after.cached_base_median,
            state_before.cached_base_median,
        )
        || state_after.cached_base_median_same_identity
            != state_before.cached_base_median_same_identity
        || state_after.base_glyph != state_before.base_glyph
        || state_after.stem_alias != state_before.stem_alias
        || state_after.group_runtime != expected_group_runtime
        || public.group_runtime != expected_group_runtime
        || state_after.certificate.is_some()
        || state_after.committed != Some(public.key)
        || state_after.b_linker_flag_state_before != state_before.b_linker_flag_state_before
        || state_after.b_linker_flag_state_after != state_before.b_linker_flag_state_after
        || state_after.base_apply_state_after != state_before.base_apply_state_after
        || state_after.group_runtime.member_state_sha256 != result.value("groupStateHashAfter")?
        || summary.usize("edgesAdded")? != public.sig_relation_mutation_count
        || summary.usize("linkerWrites")? != public.b_linker_write_count
    {
        return Err("public Boundary-16 mutation/operation/state projection differs".to_owned());
    }
    let group_hash_changed =
        baseline.value("groupStateHashBefore")? != result.value("groupStateHashAfter")?;
    if (expected_abnormal_changes == 0) == group_hash_changed {
        return Err("group member hash transition differs from callback mutations".to_owned());
    }
    for member in &state_after.live_group_members {
        if member
            .runtime
            .beam_group
            .as_ref()
            .is_none_or(|group| group.state_sha256 != result.value("groupStateHashAfter").unwrap())
        {
            return Err("live group member did not receive post-state hash".to_owned());
        }
    }
    Ok(())
}

#[allow(dead_code)]
struct HydratedBoundarySixteen {
    predecessor: b15_hydration::HydratedBoundaryFifteen,
    state_before: NativeStemsBeamVLinkSiblingLinksState,
    state_after: NativeStemsBeamVLinkSiblingLinksState,
    transaction: NativeStemsBeamVLinkSiblingLinksTransaction,
}

/// Boundary-20 entry: hydrate Boundary 16 at a frontier other than the first.
///
/// Identical to `hydrate_real_boundary_sixteen` except that the caller supplies
/// the predecessor evidence and a native page whose scheduler already stands at
/// the wanted frontier, rather than the frozen first-frontier fixtures being
/// looked up by page key.
// Used by the Boundary-17 gate's second-frontier replay; the sibling-links
// gate includes this file too and does not need it.
#[allow(dead_code, clippy::too_many_arguments)]
fn hydrate_real_boundary_sixteen_on_page(
    page: &StrictRow,
    transaction: &ParsedTransaction,
    native_page: &b15_hydration::NativePredecessorPage,
    b15_text: &str,
    base_apply_text: &str,
    create_text: &str,
    reuse_text: &str,
) -> Result<HydratedBoundarySixteen, String> {
    let linked_before = boundary_fifteen_linked_before(b15_text, transaction)?;
    let predecessor = b15_hydration::run_real_on_page(
        native_page,
        transaction.key.system,
        base_apply_text,
        create_text,
        reuse_text,
        linked_before,
    )?;
    let mut state_after = project_real_state(page, transaction, &predecessor)?;
    let state_before = state_after.clone();
    let public = apply_native_stems_beam_vlink_sibling_links_transaction(
        &predecessor.scheduler,
        &predecessor.plans,
        &predecessor.stumps,
        &predecessor.vlinkers,
        &predecessor.reachability,
        &predecessor.builder,
        &predecessor.create_transaction,
        &predecessor.reuse_live_state,
        predecessor.relation_parameters,
        &predecessor.reuse_check,
        &predecessor.base_apply,
        &predecessor.transaction,
        &mut state_after,
    )
    .map_err(|error| {
        format!(
            "system {} production Boundary-16 apply failed at this frontier: {error}",
            transaction.key.system
        )
    })?;
    assert_public_transaction_matches_rows(
        transaction,
        &predecessor,
        &state_before,
        &state_after,
        &public,
    )?;
    Ok(HydratedBoundarySixteen {
        predecessor,
        state_before,
        state_after,
        transaction: public,
    })
}

fn hydrate_real_boundary_sixteen(
    page: &StrictRow,
    transaction: &ParsedTransaction,
) -> Result<HydratedBoundarySixteen, String> {
    let (page_key, image) = corpus_page_for_token(&page.page)?;
    let b15_path = boundary_fifteen_fixture_path(page_key);
    let base_apply_path = predecessor_fixture_path("stems-beam-vlink-base-apply", page_key);
    let create_path = predecessor_fixture_path("stems-beam-create-stem", page_key);
    let reuse_path = predecessor_fixture_path("stems-beam-vlink-reuse-check", page_key);
    let b15_fixture = std::fs::read_to_string(repo_root().join(&b15_path))
        .map_err(|error| format!("cannot read Boundary-15 {page_key} fixture: {error}"))?;
    let base_apply_text = std::fs::read_to_string(repo_root().join(&base_apply_path))
        .map_err(|error| format!("cannot read {page_key} base-apply fixture: {error}"))?;
    let create_text = std::fs::read_to_string(repo_root().join(&create_path))
        .map_err(|error| format!("cannot read {page_key} create-stem fixture: {error}"))?;
    let reuse_text = std::fs::read_to_string(repo_root().join(&reuse_path))
        .map_err(|error| format!("cannot read {page_key} reuse-check fixture: {error}"))?;
    let linked_before = boundary_fifteen_linked_before(&b15_fixture, transaction)?;
    let predecessor = b15_hydration::run_real(
        image,
        transaction.key.system,
        &base_apply_text,
        &create_text,
        &reuse_text,
        linked_before,
    )?;
    let mut state_after = project_real_state(page, transaction, &predecessor)?;
    let state_before = state_after.clone();
    let public = apply_native_stems_beam_vlink_sibling_links_transaction(
        &predecessor.scheduler,
        &predecessor.plans,
        &predecessor.stumps,
        &predecessor.vlinkers,
        &predecessor.reachability,
        &predecessor.builder,
        &predecessor.create_transaction,
        &predecessor.reuse_live_state,
        predecessor.relation_parameters,
        &predecessor.reuse_check,
        &predecessor.base_apply,
        &predecessor.transaction,
        &mut state_after,
    )
    .map_err(|error| {
        format!(
            "system {} production Boundary-16 apply failed: {error}",
            transaction.key.system
        )
    })?;
    assert_public_transaction_matches_rows(
        transaction,
        &predecessor,
        &state_before,
        &state_after,
        &public,
    )?;
    Ok(HydratedBoundarySixteen {
        predecessor,
        state_before,
        state_after,
        transaction: public,
    })
}
