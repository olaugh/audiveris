// SPDX-License-Identifier: AGPL-3.0-or-later

//! Fail-closed differential gate for the per-system beam scheduler frontier.
//!
//! Java is replayed only through deterministic, known-false beam-origin link
//! prefixes.  The first prefix ready to call `StemBuilder.createStem`, or the
//! first side result ready to remove a competing hook, is a typed transaction
//! frontier rather than a claimed success.

#![allow(dead_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    path::PathBuf,
};

use audiveris_image::{
    run_table::{Orientation, RunTable},
    section::Bounds,
};

use audiveris_omr::{
    beam_inters::BeamKind,
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads,
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::{NativeStemSeedRecognition, recognize_native_stem_seeds},
    native_stems_beam_builders::{
        NativeStemsBeamBuilder, NativeStemsBeamBuilderRecognition, NativeStemsBeamBuilderSystem,
        NativeStemsBeamBuilderTargetRef, materialize_native_stems_beam_builders,
    },
    native_stems_beam_link_plans::{
        NativeStemsBeamLinkPlanAttempt, NativeStemsBeamLinkPlanOutcome,
        NativeStemsBeamLinkPlanRecognition, NativeStemsBeamLinkPlanSystem,
        materialize_native_stems_beam_link_plans,
    },
    native_stems_beam_reachability::materialize_native_stems_beam_reachability,
    native_stems_beam_scheduler::{
        NativeStemsBeamAwaitingHookRemovalTransaction, NativeStemsBeamAwaitingVLinkTransaction,
        NativeStemsBeamCanonicalGlyph, NativeStemsBeamCompletedVLinkEvidence,
        NativeStemsBeamDeferredLineDelta, NativeStemsBeamLiveExclusion, NativeStemsBeamPlanRef,
        NativeStemsBeamScheduledBeam, NativeStemsBeamSchedulerEvent, NativeStemsBeamSchedulerPass,
        NativeStemsBeamSchedulerRecognition, NativeStemsBeamSchedulerResumeStatus,
        NativeStemsBeamSchedulerStatus, NativeStemsBeamSchedulerSystem,
        NativeStemsBeamWorklistSnapshot, materialize_native_stems_beam_scheduler_frontiers,
        resume_native_stems_beam_scheduler_after_transaction,
    },
    native_stems_beam_stumps::{
        NativeStemsBeamSource, NativeStemsBeamStumpBeam, NativeStemsBeamStumpRecognition,
        NativeStemsBeamStumpRef, NativeStemsBeamStumpSystem, materialize_native_stems_beam_stumps,
    },
    native_stems_beam_vlinkers::{
        NativeStemsBeamBLinker, NativeStemsBeamBLinkerRef, NativeStemsBeamVLinkerConstructor,
        NativeStemsBeamVLinkerRecognition, NativeStemsBeamVLinkerRef, NativeStemsBeamVLinkerSystem,
        materialize_native_stems_beam_vlinkers,
    },
    native_stems_head_builders::materialize_native_stems_head_builders,
    native_stems_head_corner_reachability::materialize_native_stems_head_corner_reachability,
    native_stems_head_corners::materialize_native_stems_head_corners,
    native_stems_head_seeds::materialize_native_stems_head_seeds,
    native_stems_head_stumps::materialize_native_stems_head_stumps,
    recognize::{
        NativeBeamRecognition, recognize_grid_lines, recognize_native_beams_with_stem_seeds,
    },
    stems_step::{NativeStemHeadSide, NativeStemLine, NativeStemPoint, NativeStemVerticalSide},
};

const BEAM_SEED_PROFILE: i32 = 3;
const BEAM_SIDE_PROFILE: i32 = 4;
const INSPECT_PROFILE: i32 = 1;

const SCHEMA_HEADER: &str = "# schema: stems-beam-scheduler-v1";
const MANIFEST_SCHEMA_HEADER: &str = "# schema: stems-beam-scheduler-manifest-v1";
const MANIFEST_PATH: &str = "rust/oracle/stems-beam-scheduler-manifest.txt";
const PROBE_PATH: &str = "rust/oracle/java/StemsBeamSchedulerProbe.java";
const RUNNER_PATH: &str = "rust/oracle/java/run-stems-beam-scheduler.sh";
const SCHEDULER_FIXTURE_PREFIX: &str = "stems-beam-scheduler-";
const EXPAND_FIXTURE_PREFIX: &str = "stems-beam-expand-";

const EXPECTED_MANIFEST_SHA256: &str =
    "b6b77cdead537a70b482ae7ef5d5c8312cc5993529382f1f39fb4779afa7abb2";
const EXPECTED_PROBE_SHA256: &str =
    "afb5c564a474bc0c227b9fdc886cf892c60ae39aa62c1d93cef8aaf610b90fba";
const EXPECTED_RUNNER_SHA256: &str =
    "2d5609b5c5ef713aa3fda6467d000fad89cd8147e97d1541b5060305b414c99e";
const EXPECTED_CORPUS_BODY_SHA256: &str =
    "8ff44c35d8c1e2334c56c4d7e546fdaacbcb2964a1ab6103168f25346e041ff1";
const EXPECTED_CORPUS_BODY_LINES: usize = 998;
const EXPECTED_CORPUS_BODY_BYTES: usize = 460_651;
const EXPECTED_CORPUS_ROW_COUNTS: [usize; 11] = [8, 30, 803, 56, 14, 14, 0, 30, 0, 30, 8];

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
        fixture: "stems-beam-scheduler-chula.txt",
    },
    PageSpec {
        image: "allegretto.png",
        page: "allegretto.png#1",
        fixture: "stems-beam-scheduler-allegretto.txt",
    },
    PageSpec {
        image: "batuque.png",
        page: "batuque.png#1",
        fixture: "stems-beam-scheduler-batuque.txt",
    },
    PageSpec {
        image: "carmen.png",
        page: "carmen.png#1",
        fixture: "stems-beam-scheduler-carmen.txt",
    },
    PageSpec {
        image: "cucaracha.png",
        page: "cucaracha.png#1",
        fixture: "stems-beam-scheduler-cucaracha.txt",
    },
    PageSpec {
        image: "hove.png",
        page: "hove.png#1",
        fixture: "stems-beam-scheduler-hove.txt",
    },
    PageSpec {
        image: "zizi.png",
        page: "zizi.png#1",
        fixture: "stems-beam-scheduler-zizi.txt",
    },
    PageSpec {
        image: "BachInvention5.jpg",
        page: "BachInvention5.jpg#1",
        fixture: "stems-beam-scheduler-BachInvention5.txt",
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
    "corpusBodySha256",
    "corpusBodyLines",
    "corpusBodyBytes",
    "corpusRowCounts",
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
    row_counts: [usize; 11],
    body_sha256: String,
    body_lines: usize,
    body_bytes: usize,
    fixture_sha256: String,
    fixture_lines: usize,
    fixture_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SchedulerManifest {
    entries: Vec<ManifestEntry>,
    probe_sha256: String,
    runner_sha256: String,
    corpus_body_sha256: String,
    corpus_body_lines: usize,
    corpus_body_bytes: usize,
    corpus_row_counts: [usize; 11],
    manifest_body_sha256: String,
    manifest_body_lines: usize,
    manifest_body_bytes: usize,
}

impl SchedulerManifest {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
        if text.lines().next() != Some(MANIFEST_SCHEMA_HEADER) {
            return Err("beam-scheduler manifest schema header differs".to_owned());
        }
        let mut entries = Vec::new();
        let mut summary = None;
        for (offset, line) in text.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            match line.split_ascii_whitespace().next() {
                Some("stemsbeamschedulermanifestentry") => {
                    if summary.is_some() {
                        return Err(format!(
                            "manifest entry after summary at line {}",
                            offset + 1
                        ));
                    }
                    let values = parse_exact_labeled_row(
                        line,
                        "stemsbeamschedulermanifestentry",
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
                Some("stemsbeamschedulermanifestsummary") => {
                    if summary.is_some() {
                        return Err("duplicate manifest summary".to_owned());
                    }
                    let values = parse_exact_labeled_row(
                        line,
                        "stemsbeamschedulermanifestsummary",
                        MANIFEST_SUMMARY_FIELDS,
                    )?;
                    if values[0] != "stems-beam-scheduler-manifest-v1" {
                        return Err("manifest summary schema differs".to_owned());
                    }
                    summary = Some((
                        parse_usize(values[1], "manifest entries")?,
                        parse_lower_hex(values[2], 64, "probe SHA-256")?.to_owned(),
                        parse_lower_hex(values[3], 64, "runner SHA-256")?.to_owned(),
                        parse_lower_hex(values[4], 64, "corpus body SHA-256")?.to_owned(),
                        parse_usize(values[5], "corpus body lines")?,
                        parse_usize(values[6], "corpus body bytes")?,
                        parse_row_counts(values[7])?,
                        parse_lower_hex(values[8], 64, "manifest body SHA-256")?.to_owned(),
                        parse_usize(values[9], "manifest body lines")?,
                        parse_usize(values[10], "manifest body bytes")?,
                    ));
                }
                Some(family) => return Err(format!("unknown manifest family {family:?}")),
                None => unreachable!("nonempty manifest row"),
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
            manifest_body_sha256,
            manifest_body_lines,
            manifest_body_bytes,
        ) = summary.ok_or_else(|| "missing manifest summary".to_owned())?;
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
                return Err(format!("manifest page order differs at {ordinal}"));
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
            manifest_body_sha256,
            manifest_body_lines,
            manifest_body_bytes,
        })
    }
}

fn source_token(source: NativeStemsBeamSource) -> String {
    match source {
        NativeStemsBeamSource::RawBeam(ordinal) => format!("raw:{ordinal}"),
        NativeStemsBeamSource::Hook(ordinal) => format!("hook:{ordinal}"),
    }
}

fn horizontal_sides() -> [NativeStemHeadSide; 2] {
    [NativeStemHeadSide::Left, NativeStemHeadSide::Right]
}

fn stump_alias(reference: &NativeStemsBeamStumpRef) -> usize {
    match reference {
        NativeStemsBeamStumpRef::Seed {
            canonical_glyph_index,
            ..
        }
        | NativeStemsBeamStumpRef::Built {
            canonical_glyph_index,
        } => *canonical_glyph_index,
    }
}

fn constructor_for(
    system: &NativeStemsBeamVLinkerSystem,
    source: NativeStemsBeamSource,
) -> &NativeStemsBeamVLinkerConstructor {
    system
        .constructors
        .iter()
        .find(|constructor| constructor.source == source)
        .unwrap_or_else(|| panic!("missing V-linker constructor for {}", source_token(source)))
}

fn b_linker_for(
    constructor: &NativeStemsBeamVLinkerConstructor,
    reference: NativeStemsBeamBLinkerRef,
) -> &NativeStemsBeamBLinker {
    constructor
        .b_linkers
        .iter()
        .find(|linker| linker.reference == reference)
        .unwrap_or_else(|| panic!("missing B-linker {reference:?}"))
}

fn builder_for(
    system: &NativeStemsBeamBuilderSystem,
    reference: NativeStemsBeamVLinkerRef,
) -> &NativeStemsBeamBuilder {
    system
        .builders
        .iter()
        .find(|builder| builder.start == reference)
        .unwrap_or_else(|| panic!("missing builder for {reference:?}"))
}

fn plan_for(
    system: &NativeStemsBeamLinkPlanSystem,
    reference: NativeStemsBeamVLinkerRef,
    stem_profile: i32,
) -> (NativeStemsBeamPlanRef, &NativeStemsBeamLinkPlanAttempt) {
    let mut plan_ordinal = 0_usize;
    for builder in &system.builders {
        for attempt in &builder.attempts {
            if builder.start == reference && attempt.stem_profile == stem_profile {
                return (
                    NativeStemsBeamPlanRef {
                        system_id: system.system_id,
                        plan_ordinal,
                        builder_ordinal: builder.builder_ordinal,
                        stem_profile,
                    },
                    attempt,
                );
            }
            plan_ordinal += 1;
        }
    }
    panic!("missing profile {stem_profile} plan for {reference:?}")
}

fn raw_exclusion_key(beams: &NativeBeamRecognition, ordinal: usize) -> Option<usize> {
    let &(system_id, beam) = beams.raw_beams.get(ordinal)?;
    match beam.kind {
        BeamKind::Hook => beams
            .raw_beams
            .get(ordinal + 1)
            .filter(|(other_system, other)| {
                *other_system == system_id
                    && other.kind == BeamKind::Beam
                    && other.item == beam.item
            })
            .map(|_| ordinal),
        BeamKind::Beam => ordinal
            .checked_sub(1)
            .and_then(|previous| beams.raw_beams.get(previous).map(|entry| (previous, entry)))
            .filter(|(_, (other_system, other))| {
                *other_system == system_id
                    && other.kind == BeamKind::Hook
                    && other.item == beam.item
            })
            .map(|(previous, _)| previous),
        BeamKind::SmallBeam => None,
    }
}

fn pair_creation_order(
    beams: &NativeBeamRecognition,
    stump_systems: &[NativeStemsBeamStumpSystem],
) -> Vec<(NativeStemsBeamSource, NativeStemsBeamSource)> {
    let mut pairs = Vec::new();
    for system in stump_systems {
        let mut sig = system.beams_by_abscissa.iter().collect::<Vec<_>>();
        sig.sort_by_key(|beam| beam.sig_ordinal);
        for (sig_ordinal, hook) in sig.iter().enumerate() {
            let NativeStemsBeamSource::RawBeam(hook_ordinal) = hook.source else {
                continue;
            };
            if hook.kind != BeamKind::Hook
                || raw_exclusion_key(beams, hook_ordinal) != Some(hook_ordinal)
            {
                continue;
            }
            let full = NativeStemsBeamSource::RawBeam(hook_ordinal + 1);
            if sig.get(sig_ordinal + 1).map(|beam| beam.source) == Some(full) {
                pairs.push((hook.source, full));
            }
        }
    }
    pairs
}

fn worklist_snapshot(
    pass: NativeStemsBeamSchedulerPass,
    current_index: usize,
    sources: &[NativeStemsBeamSource],
) -> NativeStemsBeamWorklistSnapshot {
    NativeStemsBeamWorklistSnapshot {
        pass,
        current_index,
        sources: sources.to_vec(),
        current: sources[current_index],
        remaining: sources[current_index + 1..].to_vec(),
    }
}

fn deferred_delta(
    delta_ordinal: usize,
    invocation_ordinal: usize,
    plan: NativeStemsBeamPlanRef,
    v_linker: NativeStemsBeamVLinkerRef,
    attempt: &NativeStemsBeamLinkPlanAttempt,
) -> Option<NativeStemsBeamDeferredLineDelta> {
    attempt
        .stored_theoretical_line_would_mutate
        .then_some(NativeStemsBeamDeferredLineDelta {
            delta_ordinal,
            invocation_ordinal,
            plan,
            v_linker,
            before: attempt.stored_theoretical_line_before,
            after: attempt.stored_theoretical_line_after,
            builder_line_aliases: attempt.builder_line_aliases_stored_theoretical_line,
            attachment_aliases: attempt.attachment_aliases_stored_theoretical_line,
        })
}

fn canonical_glyphs(
    stump_system: &NativeStemsBeamStumpSystem,
    v_system: &NativeStemsBeamVLinkerSystem,
    page_representatives: &mut Vec<NativeStemsBeamStumpBeam>,
) -> (
    Vec<NativeStemsBeamCanonicalGlyph>,
    BTreeMap<NativeStemsBeamSource, usize>,
) {
    let mut live = v_system
        .constructors
        .iter()
        .filter(|constructor| constructor.survives_constructor_loop)
        .map(|constructor| {
            stump_system
                .beams_by_abscissa
                .iter()
                .find(|beam| beam.source == constructor.source)
                .unwrap_or_else(|| {
                    panic!(
                        "missing stump beam for live {}",
                        source_token(constructor.source)
                    )
                })
        })
        .collect::<Vec<_>>();
    live.sort_by_key(|beam| beam.sig_ordinal);

    let mut aliases = BTreeMap::new();
    let glyphs = live
        .iter()
        .enumerate()
        .map(|(scheduler_sig_ordinal, beam)| {
            let alias_class = page_representatives
                .iter()
                .position(|candidate| candidate.beam_glyph == beam.beam_glyph)
                .unwrap_or_else(|| {
                    page_representatives.push((*beam).clone());
                    page_representatives.len() - 1
                });
            aliases.insert(beam.source, alias_class);
            NativeStemsBeamCanonicalGlyph {
                source: beam.source,
                scheduler_sig_ordinal,
                pre_tremolo_sig_ordinal: beam.sig_ordinal,
                alias_class,
                bounds: beam.beam_glyph.bounds,
                run_table: beam.beam_glyph.run_table.clone(),
                run_digest: beam.beam_glyph.run_digest(),
            }
        })
        .collect();
    (glyphs, aliases)
}

fn live_exclusions(
    aliases: &BTreeMap<NativeStemsBeamSource, usize>,
    creation_pairs: &[(NativeStemsBeamSource, NativeStemsBeamSource)],
) -> Vec<NativeStemsBeamLiveExclusion> {
    let mut live = Vec::new();
    for (creation_ordinal, &(hook, full)) in creation_pairs.iter().enumerate() {
        if let (Some(&hook_alias), Some(&beam_alias)) = (aliases.get(&hook), aliases.get(&full)) {
            assert_eq!(
                hook_alias, beam_alias,
                "live exclusion endpoints must retain one canonical glyph"
            );
            live.push(NativeStemsBeamLiveExclusion {
                creation_ordinal,
                live_ordinal: live.len(),
                hook,
                beam: full,
                canonical_glyph_alias: hook_alias,
            });
        }
    }
    live
}

fn scheduled_beams(
    stump_system: &NativeStemsBeamStumpSystem,
    glyphs: &[NativeStemsBeamCanonicalGlyph],
    exclusions: &[NativeStemsBeamLiveExclusion],
    link_profile: i32,
) -> Vec<NativeStemsBeamScheduledBeam> {
    let mut live = glyphs
        .iter()
        .map(|glyph| {
            stump_system
                .beams_by_abscissa
                .iter()
                .find(|beam| beam.source == glyph.source)
                .expect("canonical beam source")
        })
        .collect::<Vec<_>>();
    live.sort_by(|one, two| {
        two.bounds
            .width
            .cmp(&one.bounds.width)
            .then_with(|| one.sig_ordinal.cmp(&two.sig_ordinal))
    });
    live.into_iter()
        .enumerate()
        .map(|(width_ordinal, beam)| {
            let glyph = glyphs
                .iter()
                .find(|glyph| glyph.source == beam.source)
                .expect("scheduled canonical glyph");
            let competing_hook = (beam.kind != BeamKind::Hook)
                .then(|| {
                    exclusions
                        .iter()
                        .find_map(|edge| (edge.beam == beam.source).then_some(edge.hook))
                })
                .flatten();
            NativeStemsBeamScheduledBeam {
                width_ordinal,
                source: beam.source,
                scheduler_sig_ordinal: glyph.scheduler_sig_ordinal,
                integer_width: beam.bounds.width,
                kind: beam.kind,
                canonical_glyph_alias: glyph.alias_class,
                competing_hook,
                selected_side_stem_profile: if beam.kind == BeamKind::Hook
                    || competing_hook.is_some()
                {
                    link_profile
                } else {
                    BEAM_SIDE_PROFILE
                },
            }
        })
        .collect()
}

fn gate_scheduler_system(
    stump_system: &NativeStemsBeamStumpSystem,
    v_system: &NativeStemsBeamVLinkerSystem,
    builder_system: &NativeStemsBeamBuilderSystem,
    plan_system: &NativeStemsBeamLinkPlanSystem,
    page_representatives: &mut Vec<NativeStemsBeamStumpBeam>,
    creation_pairs: &[(NativeStemsBeamSource, NativeStemsBeamSource)],
) -> NativeStemsBeamSchedulerSystem {
    let system_id = stump_system.system_id;
    assert_eq!(v_system.system_id, system_id);
    assert_eq!(builder_system.system_id, system_id);
    assert_eq!(plan_system.system_id, system_id);
    let link_profile = stump_system.profile;
    let (glyphs_in_sig_order, aliases) =
        canonical_glyphs(stump_system, v_system, page_representatives);
    let live_exclusions = live_exclusions(&aliases, creation_pairs);
    let beams_by_reverse_width = scheduled_beams(
        stump_system,
        &glyphs_in_sig_order,
        &live_exclusions,
        link_profile,
    );
    let scheduled_by_source = beams_by_reverse_width
        .iter()
        .map(|beam| (beam.source, beam))
        .collect::<BTreeMap<_, _>>();
    let mut worklist = beams_by_reverse_width
        .iter()
        .map(|beam| beam.source)
        .collect::<Vec<_>>();
    let mut prefix_events = Vec::new();
    let mut deferred_line_deltas = Vec::new();
    let mut shifted_v_linkers = Vec::<NativeStemsBeamVLinkerRef>::new();
    let mut invocation_ordinal = 0_usize;
    let mut retained_for_stumps = Vec::new();
    let mut status = None;

    let mut current_index = 0_usize;
    'side_pass: while current_index < worklist.len() {
        let source = worklist[current_index];
        let scheduled = scheduled_by_source[&source];
        let snapshot = worklist_snapshot(
            NativeStemsBeamSchedulerPass::Sides,
            current_index,
            &worklist,
        );
        prefix_events.push(NativeStemsBeamSchedulerEvent::BeamPassStart {
            event_ordinal: prefix_events.len(),
            snapshot: snapshot.clone(),
            kind: scheduled.kind,
            competing_hook: scheduled.competing_hook,
            selected_stem_profile: scheduled.selected_side_stem_profile,
        });
        let constructor = constructor_for(v_system, source);
        let mut linked_sides = Vec::new();
        let mut failed_side = None;

        for side in horizontal_sides() {
            let Some(side_entry) = constructor
                .side_b_linkers
                .iter()
                .find(|entry| entry.side == side)
            else {
                panic!("constructor lacks {side:?} side entry")
            };
            let Some(b_reference) = side_entry.b_linker else {
                prefix_events.push(NativeStemsBeamSchedulerEvent::MissingSideBLinker {
                    event_ordinal: prefix_events.len(),
                    beam: source,
                    side,
                });
                continue;
            };
            let b_linker = b_linker_for(constructor, b_reference);
            let logical_success = if b_linker.v_linkers.is_empty() {
                prefix_events.push(NativeStemsBeamSchedulerEvent::EmptyVLinkerSideSuccess {
                    event_ordinal: prefix_events.len(),
                    beam: source,
                    side,
                    b_linker: b_reference,
                    linked_flag_after: false,
                });
                true
            } else {
                for v_linker in &b_linker.v_linkers {
                    let reference = v_linker.reference;
                    let builder = builder_for(builder_system, reference);
                    if builder.items.iter().all(|item| item.target.is_none()) {
                        prefix_events.push(
                            NativeStemsBeamSchedulerEvent::SideVSkippedEmptyTargets {
                                event_ordinal: prefix_events.len(),
                                beam: source,
                                side,
                                b_linker: b_reference,
                                v_linker: reference,
                                builder_ordinal: builder.builder_ordinal,
                            },
                        );
                        continue;
                    }
                    assert!(
                        !shifted_v_linkers.contains(&reference),
                        "gate refuses repeated {reference:?} after a deferred line write"
                    );
                    let (plan, attempt) =
                        plan_for(plan_system, reference, scheduled.selected_side_stem_profile);
                    if attempt.outcome == NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem {
                        status = Some(NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(
                            Box::new(NativeStemsBeamAwaitingVLinkTransaction {
                                invocation_ordinal,
                                snapshot: snapshot.clone(),
                                beam: source,
                                horizontal_side: Some(side),
                                b_linker: b_reference,
                                v_linker: reference,
                                vertical_side: reference.side,
                                plan,
                                outcome: attempt.outcome,
                                linked_sides_before: linked_sides.clone(),
                                retained_beams_before: retained_for_stumps.clone(),
                                would_apply_stored_line_delta: deferred_delta(
                                    deferred_line_deltas.len(),
                                    invocation_ordinal,
                                    plan,
                                    reference,
                                    attempt,
                                ),
                            }),
                        ));
                        break 'side_pass;
                    }
                    let delta = deferred_delta(
                        deferred_line_deltas.len(),
                        invocation_ordinal,
                        plan,
                        reference,
                        attempt,
                    );
                    let delta_ordinal = delta.as_ref().map(|delta| delta.delta_ordinal);
                    if let Some(delta) = delta {
                        shifted_v_linkers.push(reference);
                        deferred_line_deltas.push(delta);
                    }
                    prefix_events.push(NativeStemsBeamSchedulerEvent::InvokedKnownFalsePlan {
                        event_ordinal: prefix_events.len(),
                        invocation_ordinal,
                        pass: NativeStemsBeamSchedulerPass::Sides,
                        beam: source,
                        horizontal_side: Some(side),
                        b_linker: b_reference,
                        v_linker: reference,
                        plan,
                        outcome: attempt.outcome,
                        deferred_line_delta_ordinal: delta_ordinal,
                    });
                    invocation_ordinal += 1;
                }
                false
            };
            prefix_events.push(NativeStemsBeamSchedulerEvent::SideBLinkerResult {
                event_ordinal: prefix_events.len(),
                beam: source,
                side,
                b_linker: b_reference,
                logical_success,
                linked_flag_after: false,
            });
            if logical_success {
                linked_sides.push(side);
            } else if scheduled.kind != BeamKind::Hook {
                failed_side = Some(side);
                break;
            }
        }

        if scheduled.kind == BeamKind::Hook && linked_sides.is_empty() || failed_side.is_some() {
            worklist.remove(current_index);
            prefix_events.push(
                NativeStemsBeamSchedulerEvent::BeamRemovedFromLocalWorklist {
                    event_ordinal: prefix_events.len(),
                    beam: source,
                    failed_side,
                    worklist_after: worklist.clone(),
                },
            );
            continue;
        }
        if scheduled.kind != BeamKind::Hook
            && linked_sides.len() == 2
            && let Some(competing_hook) = scheduled.competing_hook
        {
            status = Some(
                NativeStemsBeamSchedulerStatus::AwaitingHookRemovalTransaction(Box::new(
                    NativeStemsBeamAwaitingHookRemovalTransaction {
                        snapshot,
                        beam: source,
                        competing_hook,
                        linked_sides,
                        retained_beams_before: retained_for_stumps.clone(),
                    },
                )),
            );
            break;
        }
        prefix_events.push(NativeStemsBeamSchedulerEvent::BeamRetainedForStumps {
            event_ordinal: prefix_events.len(),
            beam: source,
            linked_sides,
        });
        retained_for_stumps.push(source);
        current_index += 1;
    }

    if status.is_none() {
        assert_eq!(worklist, retained_for_stumps);
        'stump_pass: for (stump_index, &source) in worklist.iter().enumerate() {
            let scheduled = scheduled_by_source[&source];
            let snapshot =
                worklist_snapshot(NativeStemsBeamSchedulerPass::Stumps, stump_index, &worklist);
            prefix_events.push(NativeStemsBeamSchedulerEvent::BeamPassStart {
                event_ordinal: prefix_events.len(),
                snapshot: snapshot.clone(),
                kind: scheduled.kind,
                competing_hook: scheduled.competing_hook,
                selected_stem_profile: BEAM_SEED_PROFILE,
            });
            let constructor = constructor_for(v_system, source);
            let stump_beam = stump_system
                .beams_by_abscissa
                .iter()
                .find(|beam| beam.source == source)
                .expect("stump pass beam");
            let side_aliases = stump_beam
                .sides
                .iter()
                .filter_map(|side| {
                    side.final_stump.as_ref().map(|reference| {
                        assert!(
                            stump_beam
                                .stumps
                                .iter()
                                .any(|candidate| candidate.reference == *reference),
                            "side stump {reference:?} does not resolve in the beam stump list"
                        );
                        stump_alias(reference)
                    })
                })
                .collect::<Vec<_>>();
            for &reference in &constructor.stump_v_linkers {
                let b_linker = b_linker_for(constructor, reference.b_linker);
                let stump = b_linker.stump.as_ref().expect("stump V has a stump");
                assert!(
                    stump_beam
                        .stumps
                        .iter()
                        .any(|candidate| candidate.reference == *stump),
                    "stump V {reference:?} does not resolve in the beam stump list"
                );
                let alias = stump_alias(stump);
                if side_aliases.contains(&alias) {
                    prefix_events.push(
                        NativeStemsBeamSchedulerEvent::StumpSkippedStructuralSideGlyph {
                            event_ordinal: prefix_events.len(),
                            beam: source,
                            b_linker: b_linker.reference,
                            v_linker: reference,
                            canonical_stump_alias: alias,
                        },
                    );
                    continue;
                }
                // No successful V transaction can have occurred inside this
                // prefix. Empty-V side successes deliberately leave this flag
                // false, so the Java already-linked branch is unreachable.
                assert!(
                    !shifted_v_linkers.contains(&reference),
                    "gate refuses repeated {reference:?} after a deferred line write"
                );
                let (plan, attempt) = plan_for(plan_system, reference, BEAM_SEED_PROFILE);
                if attempt.outcome == NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem {
                    status = Some(NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(
                        Box::new(NativeStemsBeamAwaitingVLinkTransaction {
                            invocation_ordinal,
                            snapshot,
                            beam: source,
                            horizontal_side: None,
                            b_linker: b_linker.reference,
                            v_linker: reference,
                            vertical_side: reference.side,
                            plan,
                            outcome: attempt.outcome,
                            linked_sides_before: Vec::new(),
                            retained_beams_before: worklist[..stump_index].to_vec(),
                            would_apply_stored_line_delta: deferred_delta(
                                deferred_line_deltas.len(),
                                invocation_ordinal,
                                plan,
                                reference,
                                attempt,
                            ),
                        }),
                    ));
                    break 'stump_pass;
                }
                let delta = deferred_delta(
                    deferred_line_deltas.len(),
                    invocation_ordinal,
                    plan,
                    reference,
                    attempt,
                );
                let delta_ordinal = delta.as_ref().map(|delta| delta.delta_ordinal);
                if let Some(delta) = delta {
                    shifted_v_linkers.push(reference);
                    deferred_line_deltas.push(delta);
                }
                prefix_events.push(NativeStemsBeamSchedulerEvent::InvokedKnownFalsePlan {
                    event_ordinal: prefix_events.len(),
                    invocation_ordinal,
                    pass: NativeStemsBeamSchedulerPass::Stumps,
                    beam: source,
                    horizontal_side: None,
                    b_linker: b_linker.reference,
                    v_linker: reference,
                    plan,
                    outcome: attempt.outcome,
                    deferred_line_delta_ordinal: delta_ordinal,
                });
                invocation_ordinal += 1;
            }
        }
    }

    let status = status.unwrap_or_else(|| NativeStemsBeamSchedulerStatus::Completed {
        retained_for_stumps: retained_for_stumps.clone(),
        final_local_worklist: worklist,
    });
    NativeStemsBeamSchedulerSystem {
        system_id,
        link_profile,
        glyphs_in_sig_order,
        live_exclusions,
        beams_by_reverse_width,
        prefix_events,
        deferred_line_deltas,
        status,
    }
}

fn gate_scheduler_recognition(
    beams: &NativeBeamRecognition,
    beam_stumps: &NativeStemsBeamStumpRecognition,
    beam_vlinkers: &NativeStemsBeamVLinkerRecognition,
    beam_builders: &NativeStemsBeamBuilderRecognition,
    link_plans: &NativeStemsBeamLinkPlanRecognition,
) -> NativeStemsBeamSchedulerRecognition {
    let mut page_representatives = Vec::<NativeStemsBeamStumpBeam>::new();
    let creation_pairs = pair_creation_order(beams, &beam_stumps.systems);
    let mut systems = Vec::with_capacity(beam_stumps.systems.len());
    for stump_system in &beam_stumps.systems {
        let system_id = stump_system.system_id;
        let v_system = beam_vlinkers
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .expect("V-linker scheduler system");
        let builder_system = beam_builders
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .expect("builder scheduler system");
        let plan_system = link_plans
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .expect("plan scheduler system");
        systems.push(gate_scheduler_system(
            stump_system,
            v_system,
            builder_system,
            plan_system,
            &mut page_representatives,
            &creation_pairs,
        ));
    }
    NativeStemsBeamSchedulerRecognition {
        live_beam_count: systems
            .iter()
            .map(|system| system.glyphs_in_sig_order.len())
            .sum(),
        canonical_glyph_class_count: page_representatives.len(),
        live_exclusion_count: systems
            .iter()
            .map(|system| system.live_exclusions.len())
            .sum(),
        prefix_event_count: systems
            .iter()
            .map(|system| system.prefix_events.len())
            .sum(),
        invoked_known_false_plan_count: systems
            .iter()
            .flat_map(|system| &system.prefix_events)
            .filter(|event| {
                matches!(
                    event,
                    NativeStemsBeamSchedulerEvent::InvokedKnownFalsePlan { .. }
                )
            })
            .count(),
        deferred_line_delta_count: systems
            .iter()
            .map(|system| system.deferred_line_deltas.len())
            .sum(),
        completed_system_count: systems
            .iter()
            .filter(|system| {
                matches!(
                    system.status,
                    NativeStemsBeamSchedulerStatus::Completed { .. }
                )
            })
            .count(),
        awaiting_v_link_transaction_count: systems
            .iter()
            .filter(|system| {
                matches!(
                    system.status,
                    NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(_)
                )
            })
            .count(),
        awaiting_hook_removal_transaction_count: systems
            .iter()
            .filter(|system| {
                matches!(
                    system.status,
                    NativeStemsBeamSchedulerStatus::AwaitingHookRemovalTransaction(_)
                )
            })
            .count(),
        forbidden_mutation_count: 0,
        systems,
    }
}

struct NativeSchedulerInputs {
    stem_seeds: NativeStemSeedRecognition,
    beams: NativeBeamRecognition,
    beam_stumps: NativeStemsBeamStumpRecognition,
    beam_vlinkers: NativeStemsBeamVLinkerRecognition,
    beam_builders: NativeStemsBeamBuilderRecognition,
    link_plans: NativeStemsBeamLinkPlanRecognition,
}

fn native_scheduler_inputs(image: &str) -> NativeSchedulerInputs {
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
    let link_plans = materialize_native_stems_beam_link_plans(
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
    NativeSchedulerInputs {
        stem_seeds,
        beams,
        beam_stumps,
        beam_vlinkers,
        beam_builders,
        link_plans,
    }
}

#[test]
#[ignore = "faster Chula diagnostic; the active exact corpus gate is authoritative"]
fn native_stems_beam_scheduler_matches_java_chula_exactly() {
    let root = repo_root();
    let text = std::fs::read_to_string(root.join("rust/oracle/stems-beam-scheduler-chula.txt"))
        .expect("frozen Chula scheduler fixture");
    let fixture = OracleFixture::parse(&text)
        .unwrap_or_else(|error| panic!("invalid Chula scheduler fixture: {error}"));
    let inputs = native_scheduler_inputs("chula.png");
    let independently_replayed = gate_scheduler_recognition(
        &inputs.beams,
        &inputs.beam_stumps,
        &inputs.beam_vlinkers,
        &inputs.beam_builders,
        &inputs.link_plans,
    );
    let actual = materialize_native_stems_beam_scheduler_frontiers(
        &inputs.beams,
        &inputs.beam_stumps,
        &inputs.beam_vlinkers,
        &inputs.beam_builders,
        &inputs.link_plans,
    )
    .expect("native Chula scheduler frontier");
    assert_eq!(actual, independently_replayed);
    let projected = project_native_rows(fixture.page, &inputs, &actual);
    assert_exact_semantic_rows(&fixture, &projected)
        .unwrap_or_else(|error| panic!("Chula scheduler projection: {error}"));
}

#[test]
fn native_stems_beam_scheduler_matches_java_corpus_exactly() {
    let root = repo_root();
    let manifest_bytes =
        std::fs::read(root.join(MANIFEST_PATH)).expect("frozen scheduler manifest is installed");
    assert_eq!(sha256_hex(&manifest_bytes), EXPECTED_MANIFEST_SHA256);
    let manifest = SchedulerManifest::parse(&manifest_bytes)
        .unwrap_or_else(|error| panic!("invalid scheduler manifest: {error}"));
    assert_eq!(manifest.probe_sha256, EXPECTED_PROBE_SHA256);
    assert_eq!(manifest.runner_sha256, EXPECTED_RUNNER_SHA256);
    assert_eq!(manifest.corpus_body_sha256, EXPECTED_CORPUS_BODY_SHA256);
    assert_eq!(manifest.corpus_body_lines, EXPECTED_CORPUS_BODY_LINES);
    assert_eq!(manifest.corpus_body_bytes, EXPECTED_CORPUS_BODY_BYTES);
    assert_eq!(manifest.corpus_row_counts, EXPECTED_CORPUS_ROW_COUNTS);
    assert_eq!(
        sha256_hex(&std::fs::read(root.join(PROBE_PATH)).expect("scheduler probe source")),
        EXPECTED_PROBE_SHA256,
    );
    assert_eq!(
        sha256_hex(&std::fs::read(root.join(RUNNER_PATH)).expect("scheduler runner source")),
        EXPECTED_RUNNER_SHA256,
    );
    let manifest_summary_start = find_bytes(&manifest_bytes, b"stemsbeamschedulermanifestsummary ")
        .expect("scheduler manifest summary row");
    let manifest_body = &manifest_bytes[..manifest_summary_start];
    assert_eq!(sha256_hex(manifest_body), manifest.manifest_body_sha256);
    assert_eq!(line_count(manifest_body), manifest.manifest_body_lines);
    assert_eq!(manifest_body.len(), manifest.manifest_body_bytes);

    let mut corpus_body = Vec::with_capacity(EXPECTED_CORPUS_BODY_BYTES);
    let mut common_header = None::<Vec<u8>>;
    let mut summed_row_counts = [0_usize; 11];
    for entry in &manifest.entries {
        let bytes = std::fs::read(root.join("rust/oracle").join(&entry.fixture))
            .unwrap_or_else(|error| panic!("{}: missing fixture: {error}", entry.page));
        let expand_fixture = expand_fixture_name(&entry.fixture)
            .unwrap_or_else(|error| panic!("{}: {error}", entry.page));
        let expand_fixture_bytes = std::fs::read(root.join("rust/oracle").join(&expand_fixture))
            .unwrap_or_else(|error| {
                panic!(
                    "{}: missing expand fixture {expand_fixture}: {error}",
                    entry.page
                )
            });
        let expand_fixture_sha256 = sha256_hex(&expand_fixture_bytes);
        validate_fixture_algebra(entry, &bytes, &expand_fixture_sha256)
            .unwrap_or_else(|error| panic!("{}: {error}", entry.page));
        let slices =
            fixture_slices(&bytes).unwrap_or_else(|error| panic!("{}: {error}", entry.page));
        if let Some(expected) = &common_header {
            assert_eq!(
                slices.header, expected,
                "{} common fixture header",
                entry.page
            );
        } else {
            common_header = Some(slices.header.to_vec());
            corpus_body.extend_from_slice(slices.header);
        }
        corpus_body.extend_from_slice(slices.semantic);
        for (total, count) in summed_row_counts.iter_mut().zip(entry.row_counts) {
            *total += count;
        }
    }
    assert_eq!(summed_row_counts, EXPECTED_CORPUS_ROW_COUNTS);
    assert_eq!(corpus_body.len(), EXPECTED_CORPUS_BODY_BYTES);
    assert_eq!(line_count(&corpus_body), EXPECTED_CORPUS_BODY_LINES);
    assert_eq!(sha256_hex(&corpus_body), EXPECTED_CORPUS_BODY_SHA256);
    drop(corpus_body);

    for (entry, spec) in manifest.entries.iter().zip(PAGES) {
        let text = std::fs::read_to_string(root.join("rust/oracle").join(spec.fixture))
            .unwrap_or_else(|error| panic!("{}: missing scheduler fixture: {error}", spec.page));
        let fixture = OracleFixture::parse(&text)
            .unwrap_or_else(|error| panic!("{}: invalid scheduler fixture: {error}", spec.page));
        assert_eq!(fixture.page, entry.page);
        let inputs = native_scheduler_inputs(spec.image);
        let independently_replayed = gate_scheduler_recognition(
            &inputs.beams,
            &inputs.beam_stumps,
            &inputs.beam_vlinkers,
            &inputs.beam_builders,
            &inputs.link_plans,
        );
        let actual = materialize_native_stems_beam_scheduler_frontiers(
            &inputs.beams,
            &inputs.beam_stumps,
            &inputs.beam_vlinkers,
            &inputs.beam_builders,
            &inputs.link_plans,
        )
        .unwrap_or_else(|error| panic!("{}: production scheduler: {error}", spec.page));
        assert_eq!(actual, independently_replayed, "{} replay", spec.page);
        let projected = project_native_rows(spec.page, &inputs, &actual);
        assert_exact_semantic_rows(&fixture, &projected)
            .unwrap_or_else(|error| panic!("{}: {error}", spec.page));
    }
}

#[test]
fn beam_scheduler_row_parser_rejects_schema_drift() {
    let page = "stemsbeamschedulerpage chula.png#1 systems 3 staves 6 family Bravura \
                rawBeamHookPairs 20";
    let parsed = parse_row(page, 1).expect("canonical scheduler page row");
    assert_eq!(parsed.family, Family::Page);
    assert_eq!(parsed.page, "chula.png#1");
    assert!(
        parse_row(
            "stemsbeamschedulerpage chula.png#1 systems 3 staves 6 renamed Bravura \
             rawBeamHookPairs 20",
            1,
        )
        .is_err(),
        "renamed field fails closed",
    );
    assert!(
        parse_row("stemsbeamschedulermystery chula.png#1 field value", 1,).is_err(),
        "unknown family fails closed",
    );
    assert!(
        OracleFixture::parse(page).is_err(),
        "missing schema header and hierarchy fail closed",
    );
}

#[test]
fn beam_scheduler_gate_rejects_expand_fixture_provenance_drift() {
    let root = repo_root();
    let manifest_bytes =
        std::fs::read(root.join(MANIFEST_PATH)).expect("frozen scheduler manifest is installed");
    let manifest = SchedulerManifest::parse(&manifest_bytes)
        .unwrap_or_else(|error| panic!("invalid scheduler manifest: {error}"));
    let entry = manifest.entries.first().expect("scheduler manifest entry");
    let fixture = std::fs::read(root.join("rust/oracle").join(&entry.fixture))
        .unwrap_or_else(|error| panic!("{}: missing fixture: {error}", entry.page));
    let wrong_expand_sha256 = "0".repeat(64);

    assert_eq!(
        validate_fixture_algebra(entry, &fixture, &wrong_expand_sha256),
        Err(format!("{} expand-fixture fingerprint differs", entry.page)),
    );
}

const PAGE_FIELDS: &[&str] = &["systems", "staves", "family", "rawBeamHookPairs"];
const SYSTEM_FIELDS: &[&str] = &[
    "system",
    "profile",
    "stubProfile",
    "originalBeamSigOrder",
    "liveBeamSigOrder",
    "inspectionXOrder",
    "reverseWidthOrder",
    "widthTies",
    "liveBeamHookPairs",
    "builders",
    "isolatedPlans",
];
const BEAM_FIELDS: &[&str] = &[
    "system",
    "reverseOrdinal",
    "beamSig",
    "shape",
    "isHook",
    "width",
    "bounds",
    "glyph",
    "liveBeamGlyphAlias",
    "sameGlyphMembers",
    "pairExclusions",
    "competingHook",
    "competingPairCreation",
    "competingPairLive",
    "competingHookGlyphSameIdentity",
];
const ATTEMPT_FIELDS: &[&str] = &[
    "system",
    "event",
    "phase",
    "beamOrder",
    "beamSig",
    "width",
    "hSide",
    "bAlias",
    "vSide",
    "vAlias",
    "builder",
    "stemProfile",
    "linkProfile",
    "targetGate",
    "allTargets",
    "headTargets",
    "plan",
    "outcome",
    "action",
    "lineBefore",
    "lineAfter",
    "lineChanged",
    "dx",
    "builderAliases",
    "attachmentAliases",
    "sameVAttempt",
    "work",
];
const SIDE_FIELDS: &[&str] = &[
    "system",
    "event",
    "beamOrder",
    "beamSig",
    "width",
    "hSide",
    "bAlias",
    "stemProfile",
    "linkProfile",
    "profileReason",
    "action",
    "logicalResult",
    "competingHook",
    "competingHookLocallyRemoved",
    "work",
];
const DECISION_FIELDS: &[&str] = &[
    "system",
    "event",
    "beamOrder",
    "beamSig",
    "isHook",
    "linkedSides",
    "action",
    "workBefore",
];
const STUMP_FIELDS: &[&str] = &[
    "system",
    "event",
    "beamOrder",
    "beamSig",
    "width",
    "bAlias",
    "vSide",
    "vAlias",
    "stump",
    "structuralSideMatches",
    "stemProfile",
    "linkProfile",
    "linkedGuard",
    "action",
    "work",
];
const FRONTIER_FIELDS: &[&str] = &[
    "system",
    "event",
    "type",
    "phase",
    "beamOrder",
    "beamSig",
    "hSide",
    "bAlias",
    "vSide",
    "vAlias",
    "builder",
    "stemProfile",
    "linkProfile",
    "plan",
    "outcome",
    "lineBefore",
    "lineAfter",
    "evidence",
    "before",
    "current",
    "remaining",
];
const COMPLETE_FIELDS: &[&str] = &["system", "event", "type", "work"];
const TOTAL_FIELDS: &[&str] = &[
    "systems",
    "beams",
    "widthTies",
    "liveBeamHookPairs",
    "livePairEndpointViews",
    "selectedCompetitors",
    "locallyRemovedStillSelected",
    "sideRows",
    "attemptRows",
    "beamDecisions",
    "stumpRows",
    "targetPrecheckSkips",
    "invokedV",
    "knownFalseV",
    "knownFalseLineDeltas",
    "knownOrPendingLineDeltas",
    "awaitingV",
    "awaitingHookRemoval",
    "shiftedVRetryFrontiers",
    "emptyVTrue",
    "initiallyLinkedB",
    "locallyRemovedBeams",
    "retainedBeams",
    "structuralSideStumpSkips",
    "linkedStumpSkips",
    "completedSystems",
    "forbiddenPersistentMutations",
];

fn system_summary_fields() -> Vec<&'static str> {
    std::iter::once("system")
        .chain(TOTAL_FIELDS.iter().copied())
        .chain(std::iter::once("hash"))
        .collect()
}

fn page_summary_fields() -> Vec<&'static str> {
    ["systems", "liveBeamGlyphAliases"]
        .into_iter()
        .chain(TOTAL_FIELDS.iter().copied())
        .chain(std::iter::once("hash"))
        .collect()
}

const CORPUS_SUMMARY_FIELDS: &[&str] = &[
    "schema",
    "mode",
    "pages",
    "pageRefs",
    "rowCounts",
    "probeSourceSha256",
    "runnerSourceSha256",
    "expandFixtureSha256",
    "emittedBodySha256",
    "emittedBodyLines",
    "emittedBodyBytes",
    "freshJvmPerPage",
    "runnerJavaProcessReaped",
    "backgroundJavaProcessesStarted",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Family {
    Page,
    System,
    Beam,
    Attempt,
    Side,
    Decision,
    Stump,
    Frontier,
    Complete,
    SystemSummary,
    PageSummary,
    CorpusSummary,
}

impl Family {
    fn parse(token: &str) -> Result<Self, String> {
        match token {
            "stemsbeamschedulerpage" => Ok(Self::Page),
            "stemsbeamschedulersystem" => Ok(Self::System),
            "stemsbeamschedulerbeam" => Ok(Self::Beam),
            "stemsbeamschedulerattempt" => Ok(Self::Attempt),
            "stemsbeamschedulerside" => Ok(Self::Side),
            "stemsbeamschedulerbeamdecision" => Ok(Self::Decision),
            "stemsbeamschedulerstump" => Ok(Self::Stump),
            "stemsbeamschedulerfrontier" => Ok(Self::Frontier),
            "stemsbeamschedulercomplete" => Ok(Self::Complete),
            "stemsbeamschedulersystemsummary" => Ok(Self::SystemSummary),
            "stemsbeamschedulerpagesummary" => Ok(Self::PageSummary),
            "stemsbeamschedulercorpussummary" => Ok(Self::CorpusSummary),
            _ => Err(format!("unsupported beam-scheduler row family {token:?}")),
        }
    }

    fn labels(self) -> Vec<&'static str> {
        match self {
            Self::Page => PAGE_FIELDS.to_vec(),
            Self::System => SYSTEM_FIELDS.to_vec(),
            Self::Beam => BEAM_FIELDS.to_vec(),
            Self::Attempt => ATTEMPT_FIELDS.to_vec(),
            Self::Side => SIDE_FIELDS.to_vec(),
            Self::Decision => DECISION_FIELDS.to_vec(),
            Self::Stump => STUMP_FIELDS.to_vec(),
            Self::Frontier => FRONTIER_FIELDS.to_vec(),
            Self::Complete => COMPLETE_FIELDS.to_vec(),
            Self::SystemSummary => system_summary_fields(),
            Self::PageSummary => page_summary_fields(),
            Self::CorpusSummary => CORPUS_SUMMARY_FIELDS.to_vec(),
        }
    }

    fn row_count_index(self) -> Option<usize> {
        match self {
            Self::Page => Some(0),
            Self::System => Some(1),
            Self::Beam => Some(2),
            Self::Attempt => Some(3),
            Self::Side => Some(4),
            Self::Decision => Some(5),
            Self::Stump => Some(6),
            Self::Frontier => Some(7),
            Self::Complete => Some(8),
            Self::SystemSummary => Some(9),
            Self::PageSummary => Some(10),
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
        let schema_count = text.lines().filter(|line| *line == SCHEMA_HEADER).count();
        if schema_count != 1 {
            return Err(format!(
                "expected one exact schema header {SCHEMA_HEADER:?}, got {schema_count}"
            ));
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
            .ok_or_else(|| "empty beam-scheduler fixture".to_owned())?
            .page;
        if rows
            .iter()
            .filter(|row| row.family != Family::CorpusSummary)
            .any(|row| row.page != page)
        {
            return Err("split scheduler fixture mixes pages".to_owned());
        }
        validate_hierarchy(&rows)?;
        validate_corpus_trailer(page, &rows)?;
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
        let fields = &tokens[1..];
        if fields.len() % 2 != 0 {
            return Err(format!("odd corpus-summary tail at line {line_number}"));
        }
        let page_index = fields
            .chunks_exact(2)
            .position(|pair| pair[0] == "pageRefs")
            .ok_or_else(|| format!("corpus summary lacks pageRefs at line {line_number}"))?;
        (fields[2 * page_index + 1], fields)
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
    let expected = family.labels();
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
    let declared_systems = rows[0].number::<usize>("systems")?;
    let mut current_system = None;
    let mut next_system = 1_usize;
    let mut next_beam = 0_usize;
    let mut next_event = 0_usize;
    let mut beam_sigs = BTreeSet::new();
    let mut in_events = false;
    let mut saw_terminal = false;
    let mut systems = 0_usize;
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
                return Err(format!("duplicate page row at line {}", row.line_number));
            }
            Family::System => {
                if current_system.is_some() || saw_page_summary {
                    return Err(format!("misnested system at line {}", row.line_number));
                }
                let system = row.number::<usize>("system")?;
                if system != next_system {
                    return Err(format!(
                        "non-contiguous system at line {}: {system} != {next_system}",
                        row.line_number
                    ));
                }
                next_system += 1;
                systems += 1;
                current_system = Some(system);
                next_beam = 0;
                next_event = 0;
                beam_sigs.clear();
                in_events = false;
                saw_terminal = false;
            }
            Family::Beam => {
                let system = row.number::<usize>("system")?;
                if current_system != Some(system) || in_events || saw_terminal {
                    return Err(format!("misnested beam at line {}", row.line_number));
                }
                let ordinal = row.number::<usize>("reverseOrdinal")?;
                if ordinal != next_beam {
                    return Err(format!(
                        "non-contiguous reverse beam ordinal at line {}: {ordinal} != {next_beam}",
                        row.line_number
                    ));
                }
                next_beam += 1;
                if !beam_sigs.insert(row.number::<usize>("beamSig")?) {
                    return Err(format!("duplicate beamSig at line {}", row.line_number));
                }
            }
            Family::Attempt
            | Family::Side
            | Family::Decision
            | Family::Stump
            | Family::Frontier
            | Family::Complete => {
                let system = row.number::<usize>("system")?;
                if current_system != Some(system) || saw_terminal {
                    return Err(format!(
                        "event outside current system at line {}",
                        row.line_number
                    ));
                }
                in_events = true;
                let event = row.number::<usize>("event")?;
                if event != next_event {
                    return Err(format!(
                        "non-contiguous event at line {}: {event} != {next_event}",
                        row.line_number
                    ));
                }
                next_event += 1;
                if matches!(row.family, Family::Frontier | Family::Complete) {
                    saw_terminal = true;
                }
            }
            Family::SystemSummary => {
                let system = row.number::<usize>("system")?;
                if current_system != Some(system) || !saw_terminal {
                    return Err(format!(
                        "system summary before exactly one terminal at line {}",
                        row.line_number
                    ));
                }
                current_system = None;
            }
            Family::PageSummary => {
                if current_system.is_some() || systems != declared_systems {
                    return Err(format!("early page summary at line {}", row.line_number));
                }
                if row.number::<usize>("systems")? != declared_systems
                    || row.number::<usize>("forbiddenPersistentMutations")? != 0
                {
                    return Err(format!("invalid page summary at line {}", row.line_number));
                }
                saw_page_summary = true;
            }
            Family::CorpusSummary => {
                if !saw_page_summary || row.value("schema")? != "stems-beam-scheduler-v1" {
                    return Err(format!(
                        "misplaced/invalid corpus summary at line {}",
                        row.line_number
                    ));
                }
                saw_corpus_summary = true;
            }
        }
    }
    if current_system.is_some() || !saw_page_summary || !saw_corpus_summary {
        return Err("fixture ends before its exact summaries".to_owned());
    }
    Ok(())
}

fn parse_row_counts(value: &str) -> Result<[usize; 11], String> {
    let counts = value
        .split(':')
        .map(|token| parse_usize(token, "row count"))
        .collect::<Result<Vec<_>, _>>()?;
    let length = counts.len();
    counts
        .try_into()
        .map_err(|_| format!("rowCounts needs 11 values, got {length}"))
}

fn validate_corpus_trailer(page: &str, rows: &[OracleRow<'_>]) -> Result<(), String> {
    let trailer = rows
        .last()
        .filter(|row| row.family == Family::CorpusSummary)
        .ok_or_else(|| "missing corpus-summary trailer".to_owned())?;
    if trailer.number::<usize>("pages")? != 1
        || trailer.value("pageRefs")? != page
        || trailer.value("freshJvmPerPage")? != "true"
        || trailer.value("runnerJavaProcessReaped")? != "true"
        || trailer.number::<usize>("backgroundJavaProcessesStarted")? != 0
    {
        return Err("split fixture corpus-trailer lifecycle differs".to_owned());
    }
    let expected = parse_row_counts(trailer.value("rowCounts")?)?;
    let mut observed = [0_usize; 11];
    for row in rows {
        if let Some(index) = row.family.row_count_index() {
            observed[index] += 1;
        }
    }
    if observed != expected {
        return Err(format!(
            "rowCounts differ: observed {observed:?}, trailer {expected:?}"
        ));
    }
    for label in [
        "probeSourceSha256",
        "runnerSourceSha256",
        "expandFixtureSha256",
        "emittedBodySha256",
    ] {
        parse_lower_hex(trailer.value(label)?, 64, label)?;
    }
    Ok(())
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
    let trailer_marker = b"stemsbeamschedulercorpussummary ";
    let trailer_start = find_bytes(bytes, trailer_marker)
        .ok_or_else(|| "scheduler fixture lacks corpus-summary trailer".to_owned())?;
    if find_bytes(
        &bytes[trailer_start + trailer_marker.len()..],
        trailer_marker,
    )
    .is_some()
    {
        return Err("scheduler fixture has duplicate corpus-summary trailer".to_owned());
    }
    let body = &bytes[..trailer_start];
    let page_marker = b"stemsbeamschedulerpage ";
    let semantic_start = find_bytes(body, page_marker)
        .ok_or_else(|| "scheduler fixture body lacks page row".to_owned())?;
    if !body.ends_with(b"\n") || !bytes.ends_with(b"\n") {
        return Err("scheduler fixture/body must end in a newline".to_owned());
    }
    Ok(FixtureSlices {
        body,
        header: &body[..semantic_start],
        semantic: &body[semantic_start..],
    })
}

fn expand_fixture_name(scheduler_fixture: &str) -> Result<String, String> {
    let suffix = scheduler_fixture
        .strip_prefix(SCHEDULER_FIXTURE_PREFIX)
        .filter(|suffix| !suffix.is_empty())
        .ok_or_else(|| {
            format!(
                "scheduler fixture name must start with {SCHEDULER_FIXTURE_PREFIX}: \
                 {scheduler_fixture}"
            )
        })?;
    Ok(format!("{EXPAND_FIXTURE_PREFIX}{suffix}"))
}

fn validate_fixture_algebra(
    entry: &ManifestEntry,
    bytes: &[u8],
    expand_fixture_sha256: &str,
) -> Result<(), String> {
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
        return Err(format!("{} emitted body fingerprint differs", entry.page));
    }
    let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let fixture = OracleFixture::parse(text)?;
    if fixture.page != entry.page {
        return Err(format!("{} split fixture page differs", entry.page));
    }
    let mut row_counts = [0_usize; 11];
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
    let page = fixture
        .rows
        .first()
        .filter(|row| row.family == Family::Page)
        .ok_or_else(|| format!("{} lacks page row", entry.page))?;
    let summary = fixture
        .rows
        .iter()
        .find(|row| row.family == Family::PageSummary)
        .ok_or_else(|| format!("{} lacks page summary", entry.page))?;
    if summary.value("hash")? != entry.page_hash
        || page.number::<usize>("systems")? != row_counts[1]
        || summary.number::<usize>("systems")? != row_counts[1]
        || summary.number::<usize>("beams")? != row_counts[2]
        || summary.number::<usize>("attemptRows")? != row_counts[3]
        || summary.number::<usize>("sideRows")? != row_counts[4]
        || summary.number::<usize>("beamDecisions")? != row_counts[5]
        || summary.number::<usize>("stumpRows")? != row_counts[6]
        || summary.number::<usize>("forbiddenPersistentMutations")? != 0
        || row_counts[1] != row_counts[9]
    {
        return Err(format!("{} page-summary row algebra differs", entry.page));
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
    if trailer.value("expandFixtureSha256")? != expand_fixture_sha256 {
        return Err(format!("{} expand-fixture fingerprint differs", entry.page));
    }
    if trailer.value("schema")? != "stems-beam-scheduler-v1"
        || trailer.number::<usize>("pages")? != 1
        || trailer.value("pageRefs")? != entry.page
        || trailer.value("rowCounts")? != row_count_token
        || trailer.value("probeSourceSha256")? != EXPECTED_PROBE_SHA256
        || trailer.value("runnerSourceSha256")? != EXPECTED_RUNNER_SHA256
        || trailer.value("emittedBodySha256")? != entry.body_sha256
        || trailer.number::<usize>("emittedBodyLines")? != entry.body_lines
        || trailer.number::<usize>("emittedBodyBytes")? != entry.body_bytes
        || trailer.value("freshJvmPerPage")? != "true"
        || trailer.value("runnerJavaProcessReaped")? != "true"
        || trailer.number::<usize>("backgroundJavaProcessesStarted")? != 0
    {
        return Err(format!("{} corpus trailer algebra differs", entry.page));
    }
    Ok(())
}

fn bool_token(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn list_token(values: impl IntoIterator<Item = String>) -> String {
    let values = values.into_iter().collect::<Vec<_>>();
    if values.is_empty() {
        "-".to_owned()
    } else {
        format!("[{}]", values.join(","))
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

fn line_delta_token(before: NativeStemLine, after: NativeStemLine) -> String {
    format!(
        "{}:{}:{}:{}",
        hex_double(after.start.x - before.start.x),
        hex_double(after.start.y - before.start.y),
        hex_double(after.stop.x - before.stop.x),
        hex_double(after.stop.y - before.stop.y),
    )
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
        for byte in row.bytes().chain(std::iter::once(b'\n')) {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
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

fn glyph_token(bounds: Bounds, run_table: &RunTable) -> String {
    format!(
        "g:{}:{}:{}:{}:{}",
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        glyph_run_sha256(run_table)
    )
}

fn horizontal_side_token(side: NativeStemHeadSide) -> &'static str {
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

fn phase_token(pass: NativeStemsBeamSchedulerPass) -> &'static str {
    match pass {
        NativeStemsBeamSchedulerPass::Sides => "SIDES",
        NativeStemsBeamSchedulerPass::Stumps => "STUMPS",
    }
}

fn beam_shape_token(kind: BeamKind) -> &'static str {
    match kind {
        BeamKind::Beam => "BEAM",
        BeamKind::Hook => "BEAM_HOOK",
        BeamKind::SmallBeam => "BEAM_SMALL",
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct GateTotals {
    systems: usize,
    beams: usize,
    width_ties: usize,
    live_beam_hook_pairs: usize,
    live_pair_endpoint_views: usize,
    selected_competitors: usize,
    locally_removed_still_selected: usize,
    side_rows: usize,
    attempt_rows: usize,
    beam_decisions: usize,
    stump_rows: usize,
    target_precheck_skips: usize,
    invoked_v: usize,
    known_false_v: usize,
    known_false_line_deltas: usize,
    known_or_pending_line_deltas: usize,
    awaiting_v: usize,
    awaiting_hook_removal: usize,
    shifted_v_retry_frontiers: usize,
    empty_v_true: usize,
    initially_linked_b: usize,
    locally_removed_beams: usize,
    retained_beams: usize,
    structural_side_stump_skips: usize,
    linked_stump_skips: usize,
    completed_systems: usize,
}

impl GateTotals {
    fn include(&mut self, other: &Self) {
        self.systems += other.systems;
        self.beams += other.beams;
        self.width_ties += other.width_ties;
        self.live_beam_hook_pairs += other.live_beam_hook_pairs;
        self.live_pair_endpoint_views += other.live_pair_endpoint_views;
        self.selected_competitors += other.selected_competitors;
        self.locally_removed_still_selected += other.locally_removed_still_selected;
        self.side_rows += other.side_rows;
        self.attempt_rows += other.attempt_rows;
        self.beam_decisions += other.beam_decisions;
        self.stump_rows += other.stump_rows;
        self.target_precheck_skips += other.target_precheck_skips;
        self.invoked_v += other.invoked_v;
        self.known_false_v += other.known_false_v;
        self.known_false_line_deltas += other.known_false_line_deltas;
        self.known_or_pending_line_deltas += other.known_or_pending_line_deltas;
        self.awaiting_v += other.awaiting_v;
        self.awaiting_hook_removal += other.awaiting_hook_removal;
        self.shifted_v_retry_frontiers += other.shifted_v_retry_frontiers;
        self.empty_v_true += other.empty_v_true;
        self.initially_linked_b += other.initially_linked_b;
        self.locally_removed_beams += other.locally_removed_beams;
        self.retained_beams += other.retained_beams;
        self.structural_side_stump_skips += other.structural_side_stump_skips;
        self.linked_stump_skips += other.linked_stump_skips;
        self.completed_systems += other.completed_systems;
    }

    fn fields(&self) -> String {
        format!(
            "systems {} beams {} widthTies {} liveBeamHookPairs {} \
             livePairEndpointViews {} selectedCompetitors {} \
             locallyRemovedStillSelected {} sideRows {} attemptRows {} beamDecisions {} \
             stumpRows {} targetPrecheckSkips {} invokedV {} knownFalseV {} \
             knownFalseLineDeltas {} knownOrPendingLineDeltas {} awaitingV {} \
             awaitingHookRemoval {} shiftedVRetryFrontiers {} emptyVTrue {} initiallyLinkedB {} \
             locallyRemovedBeams {} retainedBeams {} structuralSideStumpSkips {} \
             linkedStumpSkips {} completedSystems {} forbiddenPersistentMutations 0",
            self.systems,
            self.beams,
            self.width_ties,
            self.live_beam_hook_pairs,
            self.live_pair_endpoint_views,
            self.selected_competitors,
            self.locally_removed_still_selected,
            self.side_rows,
            self.attempt_rows,
            self.beam_decisions,
            self.stump_rows,
            self.target_precheck_skips,
            self.invoked_v,
            self.known_false_v,
            self.known_false_line_deltas,
            self.known_or_pending_line_deltas,
            self.awaiting_v,
            self.awaiting_hook_removal,
            self.shifted_v_retry_frontiers,
            self.empty_v_true,
            self.initially_linked_b,
            self.locally_removed_beams,
            self.retained_beams,
            self.structural_side_stump_skips,
            self.linked_stump_skips,
            self.completed_systems,
        )
    }
}

fn stump_system(inputs: &NativeSchedulerInputs, system_id: usize) -> &NativeStemsBeamStumpSystem {
    inputs
        .beam_stumps
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .unwrap_or_else(|| panic!("missing stump system {system_id}"))
}

fn v_system(inputs: &NativeSchedulerInputs, system_id: usize) -> &NativeStemsBeamVLinkerSystem {
    inputs
        .beam_vlinkers
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .unwrap_or_else(|| panic!("missing V-linker system {system_id}"))
}

fn builder_system(
    inputs: &NativeSchedulerInputs,
    system_id: usize,
) -> &NativeStemsBeamBuilderSystem {
    inputs
        .beam_builders
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .unwrap_or_else(|| panic!("missing builder system {system_id}"))
}

fn plan_system(inputs: &NativeSchedulerInputs, system_id: usize) -> &NativeStemsBeamLinkPlanSystem {
    inputs
        .link_plans
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .unwrap_or_else(|| panic!("missing plan system {system_id}"))
}

fn stump_beam(
    system: &NativeStemsBeamStumpSystem,
    source: NativeStemsBeamSource,
) -> &NativeStemsBeamStumpBeam {
    system
        .beams_by_abscissa
        .iter()
        .find(|beam| beam.source == source)
        .unwrap_or_else(|| panic!("missing stump beam {}", source_token(source)))
}

fn scheduled_beam(
    system: &NativeStemsBeamSchedulerSystem,
    source: NativeStemsBeamSource,
) -> &NativeStemsBeamScheduledBeam {
    system
        .beams_by_reverse_width
        .iter()
        .find(|beam| beam.source == source)
        .unwrap_or_else(|| panic!("missing scheduled beam {}", source_token(source)))
}

fn pre_tremolo_sig(system: &NativeStemsBeamStumpSystem, source: NativeStemsBeamSource) -> usize {
    stump_beam(system, source).sig_ordinal
}

fn beam_token_for(system: &NativeStemsBeamStumpSystem, source: NativeStemsBeamSource) -> String {
    format!("beam:{}", pre_tremolo_sig(system, source))
}

fn beam_sources_token(
    system: &NativeStemsBeamStumpSystem,
    sources: impl IntoIterator<Item = NativeStemsBeamSource>,
) -> String {
    list_token(
        sources
            .into_iter()
            .map(|source| beam_token_for(system, source)),
    )
}

fn b_alias(system: &NativeStemsBeamStumpSystem, reference: NativeStemsBeamBLinkerRef) -> String {
    assert!(reference.id > 0, "Java B-linker ids are one-based");
    format!(
        "{}:b:{}",
        beam_token_for(system, reference.beam),
        reference.id - 1
    )
}

fn v_alias(system: &NativeStemsBeamStumpSystem, reference: NativeStemsBeamVLinkerRef) -> String {
    format!(
        "{}:v:{}",
        b_alias(system, reference.b_linker),
        vertical_side_token(reference.side)
    )
}

fn builder_target_counts(builder: &NativeStemsBeamBuilder) -> (usize, usize) {
    let all = builder
        .items
        .iter()
        .filter(|item| item.target.is_some())
        .count();
    let heads = builder
        .items
        .iter()
        .filter(|item| matches!(item.target, Some(NativeStemsBeamBuilderTargetRef::Head(_))))
        .count();
    (all, heads)
}

fn profile_reason(beam: &NativeStemsBeamScheduledBeam) -> &'static str {
    if beam.kind == BeamKind::Hook {
        "Hook"
    } else if beam.competing_hook.is_some() {
        "CompetingHook"
    } else {
        "BeamSide"
    }
}

fn bounds_token(bounds: audiveris_omr::head_scanner_slices::JavaRectangle) -> String {
    format!(
        "{}:{}:{}:{}",
        bounds.x, bounds.y, bounds.width, bounds.height
    )
}

fn raw_pair_count(inputs: &NativeSchedulerInputs) -> usize {
    pair_creation_order(&inputs.beams, &inputs.beam_stumps.systems).len()
}

fn semantic_pair_creation_order(
    inputs: &NativeSchedulerInputs,
) -> Vec<(NativeStemsBeamSource, NativeStemsBeamSource)> {
    pair_creation_order(&inputs.beams, &inputs.beam_stumps.systems)
}

fn semantic_pair_ordinals(
    inputs: &NativeSchedulerInputs,
    scheduler: &NativeStemsBeamSchedulerSystem,
    edge: &NativeStemsBeamLiveExclusion,
) -> (usize, usize) {
    let creation_order = semantic_pair_creation_order(inputs);
    let creation = creation_order
        .iter()
        .position(|pair| *pair == (edge.hook, edge.beam))
        .expect("live pair creation provenance");
    let mut live = scheduler
        .live_exclusions
        .iter()
        .map(|candidate| {
            let ordinal = creation_order
                .iter()
                .position(|pair| *pair == (candidate.hook, candidate.beam))
                .expect("system live pair provenance");
            (ordinal, candidate)
        })
        .collect::<Vec<_>>();
    live.sort_by_key(|(ordinal, _)| *ordinal);
    let live_ordinal = live
        .iter()
        .position(|(_, candidate)| *candidate == edge)
        .expect("live pair ordinal");
    (creation, live_ordinal)
}

fn resolve_stump_glyph<'a>(
    inputs: &'a NativeSchedulerInputs,
    system_id: usize,
    reference: &NativeStemsBeamStumpRef,
) -> (Bounds, &'a RunTable) {
    match reference {
        NativeStemsBeamStumpRef::Seed {
            free_glyph_ordinal, ..
        } => {
            let glyph = inputs
                .stem_seeds
                .systems
                .iter()
                .find(|system| system.raw.system_id == system_id)
                .and_then(|system| system.free_glyphs.get(*free_glyph_ordinal))
                .unwrap_or_else(|| panic!("missing system {system_id} seed {free_glyph_ordinal}"));
            (glyph.bounds, &glyph.run_table)
        }
        NativeStemsBeamStumpRef::Built {
            canonical_glyph_index,
        } => {
            for system in &inputs.beam_stumps.systems {
                for beam in &system.beams_by_abscissa {
                    for side in &beam.sides {
                        let Some(build) = &side.build else { continue };
                        if build.canonical_glyph_index == Some(*canonical_glyph_index) {
                            let glyph = build.candidate.as_ref().unwrap_or_else(|| {
                                panic!(
                                    "built canonical stump {canonical_glyph_index} lacks candidate"
                                )
                            });
                            return (glyph.bounds, &glyph.run_table);
                        }
                    }
                }
            }
            panic!("missing built canonical stump {canonical_glyph_index}")
        }
    }
}

struct ProjectedSystem {
    rows: Vec<String>,
    totals: GateTotals,
}

struct SystemProjector<'a> {
    page: &'a str,
    inputs: &'a NativeSchedulerInputs,
    scheduler: &'a NativeStemsBeamSchedulerSystem,
    stumps: &'a NativeStemsBeamStumpSystem,
    v_linkers: &'a NativeStemsBeamVLinkerSystem,
    builders: &'a NativeStemsBeamBuilderSystem,
    plans: &'a NativeStemsBeamLinkPlanSystem,
    rows: Vec<String>,
    hash: Fnv64,
    totals: GateTotals,
    java_event: usize,
    snapshot: Option<NativeStemsBeamWorklistSnapshot>,
    current_linked_sides: Vec<NativeStemHeadSide>,
    locally_removed: BTreeSet<NativeStemsBeamSource>,
    v_attempts: Vec<(NativeStemsBeamVLinkerRef, usize)>,
}

impl<'a> SystemProjector<'a> {
    fn new(
        page: &'a str,
        inputs: &'a NativeSchedulerInputs,
        scheduler: &'a NativeStemsBeamSchedulerSystem,
    ) -> Self {
        let system_id = scheduler.system_id;
        Self {
            page,
            inputs,
            scheduler,
            stumps: stump_system(inputs, system_id),
            v_linkers: v_system(inputs, system_id),
            builders: builder_system(inputs, system_id),
            plans: plan_system(inputs, system_id),
            rows: Vec::new(),
            hash: Fnv64::default(),
            totals: GateTotals::default(),
            java_event: 0,
            snapshot: None,
            current_linked_sides: Vec::new(),
            locally_removed: BTreeSet::new(),
            v_attempts: Vec::new(),
        }
    }

    fn emit(&mut self, row: String) {
        self.hash.add(&row);
        self.rows.push(row);
    }

    fn current(
        &self,
    ) -> (
        &NativeStemsBeamWorklistSnapshot,
        &NativeStemsBeamScheduledBeam,
    ) {
        let snapshot = self.snapshot.as_ref().expect("scheduler pass snapshot");
        (snapshot, scheduled_beam(self.scheduler, snapshot.current))
    }

    fn work_token(&self) -> String {
        let (snapshot, _) = self.current();
        beam_sources_token(self.stumps, snapshot.sources.iter().copied())
    }

    fn emit_system_and_beams(&mut self) {
        let mut original = self.stumps.beams_by_abscissa.iter().collect::<Vec<_>>();
        original.sort_by_key(|beam| beam.sig_ordinal);
        let original_token = list_token(
            original
                .iter()
                .map(|beam| format!("beam:{}", beam.sig_ordinal)),
        );
        let live_token = list_token(
            self.scheduler
                .glyphs_in_sig_order
                .iter()
                .map(|glyph| format!("beam:{}", glyph.pre_tremolo_sig_ordinal)),
        );
        let inspection_token = beam_sources_token(
            self.stumps,
            self.v_linkers
                .constructors
                .iter()
                .filter(|constructor| constructor.survives_constructor_loop)
                .map(|constructor| constructor.source),
        );
        let reverse_token = beam_sources_token(
            self.stumps,
            self.scheduler
                .beams_by_reverse_width
                .iter()
                .map(|beam| beam.source),
        );
        let width_ties = self
            .scheduler
            .beams_by_reverse_width
            .windows(2)
            .filter(|pair| pair[0].integer_width == pair[1].integer_width)
            .count();
        let isolated_plans = self
            .plans
            .builders
            .iter()
            .map(|builder| builder.attempts.len())
            .sum::<usize>();
        self.emit(format!(
            "stemsbeamschedulersystem {} system {} profile {} stubProfile {} \
             originalBeamSigOrder {} liveBeamSigOrder {} inspectionXOrder {} \
             reverseWidthOrder {} widthTies {} liveBeamHookPairs {} builders {} \
             isolatedPlans {}",
            self.page,
            self.scheduler.system_id,
            self.scheduler.link_profile,
            self.scheduler.link_profile,
            original_token,
            live_token,
            inspection_token,
            reverse_token,
            width_ties,
            self.scheduler.live_exclusions.len(),
            self.builders.builders.len(),
            isolated_plans,
        ));
        self.totals.beams = self.scheduler.beams_by_reverse_width.len();
        self.totals.width_ties = width_ties;
        self.totals.live_beam_hook_pairs = self.scheduler.live_exclusions.len();
        self.totals.live_pair_endpoint_views = 2 * self.scheduler.live_exclusions.len();
        self.totals.selected_competitors = self
            .scheduler
            .beams_by_reverse_width
            .iter()
            .filter(|beam| beam.competing_hook.is_some())
            .count();

        for reverse in &self.scheduler.beams_by_reverse_width {
            let glyph = self
                .scheduler
                .glyphs_in_sig_order
                .iter()
                .find(|glyph| glyph.source == reverse.source)
                .expect("scheduled glyph");
            let stump = stump_beam(self.stumps, reverse.source);
            let same_members = list_token(
                original
                    .iter()
                    .filter(|candidate| candidate.beam_glyph == stump.beam_glyph)
                    .map(|candidate| format!("beam:{}", candidate.sig_ordinal)),
            );
            let pair_exclusions =
                list_token(self.scheduler.live_exclusions.iter().filter_map(|edge| {
                    let opposite = if edge.hook == reverse.source {
                        edge.beam
                    } else if edge.beam == reverse.source {
                        edge.hook
                    } else {
                        return None;
                    };
                    let (creation_ordinal, live_ordinal) =
                        semantic_pair_ordinals(self.inputs, self.scheduler, edge);
                    Some(format!(
                        "pairCreation:{}:pairLive:{}:{}@OVERLAP",
                        creation_ordinal,
                        live_ordinal,
                        beam_token_for(self.stumps, opposite)
                    ))
                }));
            let competing_edge = reverse.competing_hook.and_then(|hook| {
                self.scheduler
                    .live_exclusions
                    .iter()
                    .find(|edge| edge.hook == hook && edge.beam == reverse.source)
            });
            assert_eq!(
                reverse.canonical_glyph_alias, glyph.alias_class,
                "scheduled/canonical glyph alias"
            );
            let competing_ordinals = competing_edge
                .map(|edge| semantic_pair_ordinals(self.inputs, self.scheduler, edge));
            self.emit(format!(
                "stemsbeamschedulerbeam {} system {} reverseOrdinal {} beamSig {} shape {} \
                 isHook {} width {} bounds {} glyph {} liveBeamGlyphAlias beamGlyph:{} \
                 sameGlyphMembers {} pairExclusions {} competingHook {} \
                 competingPairCreation {} competingPairLive {} \
                 competingHookGlyphSameIdentity {}",
                self.page,
                self.scheduler.system_id,
                reverse.width_ordinal,
                glyph.pre_tremolo_sig_ordinal,
                beam_shape_token(reverse.kind),
                bool_token(reverse.kind == BeamKind::Hook),
                reverse.integer_width,
                bounds_token(stump.bounds),
                glyph_token(glyph.bounds, &glyph.run_table),
                glyph.alias_class,
                same_members,
                pair_exclusions,
                reverse
                    .competing_hook
                    .map(|hook| beam_token_for(self.stumps, hook))
                    .unwrap_or_else(|| "-".to_owned()),
                competing_ordinals
                    .map(|(creation, _)| creation.to_string())
                    .unwrap_or_else(|| "-".to_owned()),
                competing_ordinals
                    .map(|(_, live)| live.to_string())
                    .unwrap_or_else(|| "-".to_owned()),
                bool_token(reverse.competing_hook.is_some()),
            ));
        }
    }

    fn emit_attempt(
        &mut self,
        pass: NativeStemsBeamSchedulerPass,
        horizontal_side: Option<NativeStemHeadSide>,
        b_linker: NativeStemsBeamBLinkerRef,
        v_linker: NativeStemsBeamVLinkerRef,
        plan: NativeStemsBeamPlanRef,
        action: &'static str,
    ) {
        let (snapshot, scheduled) = self.current();
        let snapshot = snapshot.clone();
        let scheduled = scheduled.clone();
        let builder = builder_for(self.builders, v_linker);
        assert_eq!(builder.builder_ordinal, plan.builder_ordinal);
        let (all_targets, head_targets) = builder_target_counts(builder);
        let (_, attempt) = plan_for(self.plans, v_linker, plan.stem_profile);
        assert_eq!(plan.system_id, self.scheduler.system_id);
        let invoked = action != "SkipNoTargetLinker";
        let same_v_attempt = if invoked {
            if let Some((_, ordinal)) = self
                .v_attempts
                .iter_mut()
                .find(|(candidate, _)| *candidate == v_linker)
            {
                *ordinal += 1;
                *ordinal
            } else {
                self.v_attempts.push((v_linker, 1));
                1
            }
        } else {
            self.v_attempts
                .iter()
                .find_map(|(candidate, ordinal)| (*candidate == v_linker).then_some(*ordinal))
                .unwrap_or(0)
        };
        let (outcome, before, after, changed, dx, builder_alias, attachment_alias) = if invoked {
            (
                outcome_token(attempt.outcome),
                line_token(attempt.stored_theoretical_line_before),
                line_token(attempt.stored_theoretical_line_after),
                attempt.stored_theoretical_line_would_mutate,
                hex_double(
                    attempt.stored_theoretical_line_after.start.x
                        - attempt.stored_theoretical_line_before.start.x,
                ),
                attempt.builder_line_aliases_stored_theoretical_line,
                attempt.attachment_aliases_stored_theoretical_line,
            )
        } else {
            (
                "-",
                "-".to_owned(),
                "-".to_owned(),
                false,
                hex_double(0.0),
                false,
                false,
            )
        };
        let target_gate = if pass == NativeStemsBeamSchedulerPass::Sides {
            "allTargets"
        } else {
            "none"
        };
        self.emit(format!(
            "stemsbeamschedulerattempt {} system {} event {} phase {} beamOrder {} \
             beamSig {} width {} hSide {} bAlias {} vSide {} vAlias {} builder {} \
             stemProfile {} linkProfile {} targetGate {} allTargets {} headTargets {} \
             plan {} outcome {} action {} lineBefore {} lineAfter {} lineChanged {} dx {} \
             builderAliases {} attachmentAliases {} sameVAttempt {} work {}",
            self.page,
            self.scheduler.system_id,
            self.java_event,
            phase_token(pass),
            snapshot.current_index,
            pre_tremolo_sig(self.stumps, scheduled.source),
            scheduled.integer_width,
            horizontal_side.map(horizontal_side_token).unwrap_or("-"),
            b_alias(self.stumps, b_linker),
            vertical_side_token(v_linker.side),
            v_alias(self.stumps, v_linker),
            plan.builder_ordinal,
            plan.stem_profile,
            self.scheduler.link_profile,
            target_gate,
            all_targets,
            head_targets,
            plan.plan_ordinal,
            outcome,
            action,
            before,
            after,
            bool_token(changed),
            dx,
            bool_token(builder_alias),
            bool_token(attachment_alias),
            same_v_attempt,
            beam_sources_token(self.stumps, snapshot.sources),
        ));
        self.java_event += 1;
        self.totals.attempt_rows += 1;
        if invoked {
            self.totals.invoked_v += 1;
            self.totals.known_or_pending_line_deltas += usize::from(changed);
            if action == "KnownFalseReturn" {
                self.totals.known_false_v += 1;
                self.totals.known_false_line_deltas += usize::from(changed);
            }
        } else {
            self.totals.target_precheck_skips += 1;
        }
    }

    fn emit_side(
        &mut self,
        side: NativeStemHeadSide,
        b_linker: Option<NativeStemsBeamBLinkerRef>,
        action: &'static str,
        logical_result: bool,
    ) {
        let (snapshot, scheduled) = self.current();
        let snapshot = snapshot.clone();
        let scheduled = scheduled.clone();
        let competitor_removed = scheduled
            .competing_hook
            .is_some_and(|hook| self.locally_removed.contains(&hook));
        self.emit(format!(
            "stemsbeamschedulerside {} system {} event {} beamOrder {} beamSig {} width {} \
             hSide {} bAlias {} stemProfile {} linkProfile {} profileReason {} action {} \
             logicalResult {} competingHook {} competingHookLocallyRemoved {} work {}",
            self.page,
            self.scheduler.system_id,
            self.java_event,
            snapshot.current_index,
            pre_tremolo_sig(self.stumps, scheduled.source),
            scheduled.integer_width,
            horizontal_side_token(side),
            b_linker
                .map(|reference| b_alias(self.stumps, reference))
                .unwrap_or_else(|| "-".to_owned()),
            scheduled.selected_side_stem_profile,
            self.scheduler.link_profile,
            profile_reason(&scheduled),
            action,
            bool_token(logical_result),
            scheduled
                .competing_hook
                .map(|hook| beam_token_for(self.stumps, hook))
                .unwrap_or_else(|| "-".to_owned()),
            bool_token(competitor_removed),
            beam_sources_token(self.stumps, snapshot.sources),
        ));
        self.java_event += 1;
        self.totals.side_rows += 1;
    }

    fn emit_decision(&mut self, action: &'static str, linked_sides: &[NativeStemHeadSide]) {
        let (snapshot, scheduled) = self.current();
        let snapshot = snapshot.clone();
        let scheduled = scheduled.clone();
        self.emit(format!(
            "stemsbeamschedulerbeamdecision {} system {} event {} beamOrder {} beamSig {} \
             isHook {} linkedSides {} action {} workBefore {}",
            self.page,
            self.scheduler.system_id,
            self.java_event,
            snapshot.current_index,
            pre_tremolo_sig(self.stumps, scheduled.source),
            bool_token(scheduled.kind == BeamKind::Hook),
            list_token(
                linked_sides
                    .iter()
                    .copied()
                    .map(horizontal_side_token)
                    .map(str::to_owned),
            ),
            action,
            beam_sources_token(self.stumps, snapshot.sources),
        ));
        self.java_event += 1;
        self.totals.beam_decisions += 1;
    }

    fn emit_stump(
        &mut self,
        b_linker: NativeStemsBeamBLinkerRef,
        v_linker: NativeStemsBeamVLinkerRef,
        linked_guard: bool,
        action: &'static str,
    ) {
        let (snapshot, scheduled) = self.current();
        let snapshot = snapshot.clone();
        let scheduled = scheduled.clone();
        let constructor = constructor_for(self.v_linkers, scheduled.source);
        let b = b_linker_for(constructor, b_linker);
        let stump_ref = b.stump.as_ref().expect("stump row B-linker stump");
        let (bounds, run_table) =
            resolve_stump_glyph(self.inputs, self.scheduler.system_id, stump_ref);
        let alias = stump_alias(stump_ref);
        let structural_sides = stump_beam(self.stumps, scheduled.source)
            .sides
            .iter()
            .filter_map(|side| {
                side.final_stump
                    .as_ref()
                    .filter(|reference| stump_alias(reference) == alias)
                    .map(|_| horizontal_side_token(side.side).to_owned())
            });
        self.emit(format!(
            "stemsbeamschedulerstump {} system {} event {} beamOrder {} beamSig {} width {} \
             bAlias {} vSide {} vAlias {} stump {} structuralSideMatches {} stemProfile {} \
             linkProfile {} linkedGuard {} action {} work {}",
            self.page,
            self.scheduler.system_id,
            self.java_event,
            snapshot.current_index,
            pre_tremolo_sig(self.stumps, scheduled.source),
            scheduled.integer_width,
            b_alias(self.stumps, b_linker),
            vertical_side_token(v_linker.side),
            v_alias(self.stumps, v_linker),
            glyph_token(bounds, run_table),
            list_token(structural_sides),
            BEAM_SEED_PROFILE,
            self.scheduler.link_profile,
            bool_token(linked_guard),
            action,
            beam_sources_token(self.stumps, snapshot.sources),
        ));
        self.java_event += 1;
        self.totals.stump_rows += 1;
        match action {
            "SkipStructuralSideStump" => self.totals.structural_side_stump_skips += 1,
            "SkipAlreadyLinked" => self.totals.linked_stump_skips += 1,
            "InvokeVLink" => {}
            _ => panic!("unknown stump action {action}"),
        }
    }

    fn emit_v_frontier(&mut self, transaction: &NativeStemsBeamAwaitingVLinkTransaction) {
        self.snapshot = Some(transaction.snapshot.clone());
        let (_, attempt) = plan_for(
            self.plans,
            transaction.v_linker,
            transaction.plan.stem_profile,
        );
        let snapshot = transaction.snapshot.clone();
        self.emit(format!(
            "stemsbeamschedulerfrontier {} system {} event {} type AwaitingVLinkTransaction \
             phase {} beamOrder {} beamSig {} hSide {} bAlias {} vSide {} vAlias {} \
             builder {} stemProfile {} linkProfile {} plan {} outcome {} lineBefore {} \
             lineAfter {} evidence lastIndex={},relations={},glyphs={} before {} current {} \
             remaining {}",
            self.page,
            self.scheduler.system_id,
            self.java_event,
            phase_token(snapshot.pass),
            snapshot.current_index,
            pre_tremolo_sig(self.stumps, transaction.beam),
            transaction
                .horizontal_side
                .map(horizontal_side_token)
                .unwrap_or("-"),
            b_alias(self.stumps, transaction.b_linker),
            vertical_side_token(transaction.vertical_side),
            v_alias(self.stumps, transaction.v_linker),
            transaction.plan.builder_ordinal,
            transaction.plan.stem_profile,
            self.scheduler.link_profile,
            transaction.plan.plan_ordinal,
            outcome_token(transaction.outcome),
            line_token(attempt.stored_theoretical_line_before),
            line_token(attempt.stored_theoretical_line_after),
            attempt.expand_last_index.expect("ready plan last index"),
            attempt.relations.len(),
            attempt.glyphs.len(),
            beam_sources_token(
                self.stumps,
                snapshot.sources[..snapshot.current_index].iter().copied(),
            ),
            beam_token_for(self.stumps, snapshot.current),
            beam_sources_token(self.stumps, snapshot.remaining),
        ));
        self.java_event += 1;
        self.totals.awaiting_v += 1;
    }

    fn emit_hook_frontier(&mut self, transaction: &NativeStemsBeamAwaitingHookRemovalTransaction) {
        self.snapshot = Some(transaction.snapshot.clone());
        let snapshot = transaction.snapshot.clone();
        self.emit(format!(
            "stemsbeamschedulerfrontier {} system {} event {} type \
             AwaitingHookRemovalTransaction phase SIDES beamOrder {} beamSig {} hSide - \
             bAlias - vSide - vAlias - builder - stemProfile - linkProfile - plan - outcome - \
             lineBefore - lineAfter - evidence competingHook={} before {} current {} remaining {}",
            self.page,
            self.scheduler.system_id,
            self.java_event,
            snapshot.current_index,
            pre_tremolo_sig(self.stumps, transaction.beam),
            beam_token_for(self.stumps, transaction.competing_hook),
            beam_sources_token(
                self.stumps,
                snapshot.sources[..snapshot.current_index].iter().copied(),
            ),
            beam_token_for(self.stumps, snapshot.current),
            beam_sources_token(self.stumps, snapshot.remaining),
        ));
        self.java_event += 1;
        self.totals.awaiting_hook_removal += 1;
    }

    fn process_events(&mut self) {
        for event in &self.scheduler.prefix_events {
            match event {
                NativeStemsBeamSchedulerEvent::BeamPassStart { snapshot, .. } => {
                    assert_eq!(
                        snapshot.current, snapshot.sources[snapshot.current_index],
                        "scheduler snapshot current"
                    );
                    self.snapshot = Some(snapshot.clone());
                    self.current_linked_sides.clear();
                    let scheduled = scheduled_beam(self.scheduler, snapshot.current);
                    if scheduled
                        .competing_hook
                        .is_some_and(|hook| self.locally_removed.contains(&hook))
                    {
                        self.totals.locally_removed_still_selected += 1;
                    }
                }
                NativeStemsBeamSchedulerEvent::MissingSideBLinker { beam, side, .. } => {
                    assert_eq!(*beam, self.current().0.current);
                    self.emit_side(*side, None, "MissingBLinker", false);
                }
                NativeStemsBeamSchedulerEvent::EmptyVLinkerSideSuccess {
                    beam,
                    linked_flag_after,
                    ..
                } => {
                    assert_eq!(*beam, self.current().0.current);
                    assert!(!linked_flag_after, "empty V must not set B linked");
                    self.totals.empty_v_true += 1;
                }
                NativeStemsBeamSchedulerEvent::SideVSkippedEmptyTargets {
                    beam,
                    side,
                    b_linker,
                    v_linker,
                    builder_ordinal,
                    ..
                } => {
                    assert_eq!(*beam, self.current().0.current);
                    let (plan, _) = plan_for(
                        self.plans,
                        *v_linker,
                        scheduled_beam(self.scheduler, *beam).selected_side_stem_profile,
                    );
                    assert_eq!(*builder_ordinal, plan.builder_ordinal);
                    self.emit_attempt(
                        NativeStemsBeamSchedulerPass::Sides,
                        Some(*side),
                        *b_linker,
                        *v_linker,
                        plan,
                        "SkipNoTargetLinker",
                    );
                }
                NativeStemsBeamSchedulerEvent::InvokedKnownFalsePlan {
                    pass,
                    beam,
                    horizontal_side,
                    b_linker,
                    v_linker,
                    plan,
                    outcome,
                    ..
                } => {
                    assert_eq!(*beam, self.current().0.current);
                    assert_ne!(*outcome, NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem);
                    if *pass == NativeStemsBeamSchedulerPass::Stumps {
                        assert!(horizontal_side.is_none());
                        self.emit_stump(*b_linker, *v_linker, false, "InvokeVLink");
                    }
                    self.emit_attempt(
                        *pass,
                        *horizontal_side,
                        *b_linker,
                        *v_linker,
                        *plan,
                        "KnownFalseReturn",
                    );
                }
                NativeStemsBeamSchedulerEvent::SideBLinkerResult {
                    beam,
                    side,
                    b_linker,
                    logical_success,
                    linked_flag_after,
                    ..
                } => {
                    assert_eq!(*beam, self.current().0.current);
                    assert!(!linked_flag_after, "prefix never sets B linked");
                    let constructor = constructor_for(self.v_linkers, *beam);
                    let b = b_linker_for(constructor, *b_linker);
                    let action = if b.v_linkers.is_empty() {
                        "EmptyVMapTrue"
                    } else {
                        "AllInvokedVKnownFalse"
                    };
                    self.emit_side(*side, Some(*b_linker), action, *logical_success);
                    if *logical_success {
                        self.current_linked_sides.push(*side);
                    }
                }
                NativeStemsBeamSchedulerEvent::BeamRetainedForStumps {
                    beam, linked_sides, ..
                } => {
                    assert_eq!(*beam, self.current().0.current);
                    assert_eq!(linked_sides, &self.current_linked_sides);
                    self.emit_decision("RetainKnownTrue", linked_sides);
                    self.totals.retained_beams += 1;
                }
                NativeStemsBeamSchedulerEvent::BeamRemovedFromLocalWorklist {
                    beam,
                    worklist_after,
                    ..
                } => {
                    assert_eq!(*beam, self.current().0.current);
                    let mut expected_after = self.current().0.sources.clone();
                    expected_after.remove(self.current().0.current_index);
                    assert_eq!(*worklist_after, expected_after);
                    let linked_sides = self.current_linked_sides.clone();
                    self.emit_decision("RemoveKnownFalse", &linked_sides);
                    self.locally_removed.insert(*beam);
                    self.totals.locally_removed_beams += 1;
                }
                NativeStemsBeamSchedulerEvent::StumpSkippedStructuralSideGlyph {
                    beam,
                    b_linker,
                    v_linker,
                    ..
                } => {
                    assert_eq!(*beam, self.current().0.current);
                    self.emit_stump(*b_linker, *v_linker, false, "SkipStructuralSideStump");
                }
                NativeStemsBeamSchedulerEvent::StumpSkippedAlreadyLinkedB {
                    beam,
                    b_linker,
                    v_linker,
                    ..
                } => {
                    assert_eq!(*beam, self.current().0.current);
                    self.totals.initially_linked_b += 1;
                    self.emit_stump(*b_linker, *v_linker, true, "SkipAlreadyLinked");
                }
            }
        }
    }

    fn finish(mut self) -> ProjectedSystem {
        self.emit_system_and_beams();
        self.process_events();
        match &self.scheduler.status {
            NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(transaction) => {
                if transaction.snapshot.pass == NativeStemsBeamSchedulerPass::Stumps {
                    self.snapshot = Some(transaction.snapshot.clone());
                    self.emit_stump(
                        transaction.b_linker,
                        transaction.v_linker,
                        false,
                        "InvokeVLink",
                    );
                }
                self.emit_attempt(
                    transaction.snapshot.pass,
                    transaction.horizontal_side,
                    transaction.b_linker,
                    transaction.v_linker,
                    transaction.plan,
                    "AwaitingVLinkTransaction",
                );
                self.emit_v_frontier(transaction);
            }
            NativeStemsBeamSchedulerStatus::AwaitingHookRemovalTransaction(transaction) => {
                self.emit_hook_frontier(transaction);
            }
            NativeStemsBeamSchedulerStatus::Completed {
                final_local_worklist,
                ..
            } => {
                self.emit(format!(
                    "stemsbeamschedulercomplete {} system {} event {} type NoReadyPrefix work {}",
                    self.page,
                    self.scheduler.system_id,
                    self.java_event,
                    beam_sources_token(self.stumps, final_local_worklist.iter().copied()),
                ));
                self.java_event += 1;
                self.totals.completed_systems += 1;
            }
        }
        self.totals.systems = 1;
        let summary = format!(
            "stemsbeamschedulersystemsummary {} system {} {} hash {:016x}",
            self.page,
            self.scheduler.system_id,
            self.totals.fields(),
            self.hash.0,
        );
        self.rows.push(summary);
        ProjectedSystem {
            rows: self.rows,
            totals: self.totals,
        }
    }
}

fn project_native_rows(
    page: &str,
    inputs: &NativeSchedulerInputs,
    scheduler: &NativeStemsBeamSchedulerRecognition,
) -> Vec<String> {
    let mut rows = Vec::new();
    let staves = inputs
        .beam_vlinkers
        .systems
        .iter()
        .map(|system| system.staff_ids.len())
        .sum::<usize>();
    rows.push(format!(
        "stemsbeamschedulerpage {page} systems {} staves {} family Bravura rawBeamHookPairs {}",
        scheduler.systems.len(),
        staves,
        raw_pair_count(inputs),
    ));
    let mut page_hash = Fnv64::default();
    let mut page_totals = GateTotals::default();
    for system in &scheduler.systems {
        let projected = SystemProjector::new(page, inputs, system).finish();
        for row in &projected.rows {
            page_hash.add(row);
        }
        page_totals.include(&projected.totals);
        rows.extend(projected.rows);
    }
    rows.push(format!(
        "stemsbeamschedulerpagesummary {page} systems {} liveBeamGlyphAliases {} {} hash {:016x}",
        scheduler.systems.len(),
        scheduler.canonical_glyph_class_count,
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
            "{} first semantic mismatch at row {}:\nJava: {}\nRust: {}",
            fixture.page,
            index + 1,
            java,
            rust,
        ));
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

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

// ---------------------------------------------------------------------------
// Boundary 19: SIDES-pass resume after the first executed transaction.
// Fast evidence per rust/PORTING.md: chula and BachInvention5, one fresh-JVM
// pass each; the runner required the re-emitted Boundary-17 and Boundary-18
// rows to match their frozen fixtures byte-for-byte before writing anything.
// ---------------------------------------------------------------------------

fn resume_row_field<'a>(row: &'a str, name: &str) -> &'a str {
    let mut tokens = row.split(' ');
    while let Some(token) = tokens.next() {
        if token == name {
            return tokens
                .next()
                .unwrap_or_else(|| panic!("field {name} lacks value: {row}"));
        }
    }
    panic!("row lacks {name}: {row}");
}

fn resume_pre_tremolo_sig(
    system: &NativeStemsBeamSchedulerSystem,
    source: NativeStemsBeamSource,
) -> usize {
    system
        .glyphs_in_sig_order
        .iter()
        .find(|glyph| glyph.source == source)
        .unwrap_or_else(|| panic!("source missing from scheduler glyphs"))
        .pre_tremolo_sig_ordinal
}

fn resume_b_alias(
    system: &NativeStemsBeamSchedulerSystem,
    reference: audiveris_omr::native_stems_beam_vlinkers::NativeStemsBeamBLinkerRef,
) -> String {
    format!(
        "beam:{}:b:{}",
        resume_pre_tremolo_sig(system, reference.beam),
        reference
            .id
            .checked_sub(1)
            .expect("B-linker id is one-based"),
    )
}

#[test]
fn native_stems_beam_scheduler_resume_matches_java_on_corpus() {
    let root = repo_root();
    let mut checked_pages = 0;
    // Every installed sheet, not a fixed pair: a fixture that is frozen but
    // ungraded is the failure this loop exists to prevent. A gap is only
    // tolerated at the end of the corpus order, so a half-installed sheet
    // cannot hide between two graded ones.
    let mut saw_missing = false;
    for page in PAGES {
        let key = page
            .fixture
            .trim_start_matches("stems-beam-scheduler-")
            .trim_end_matches(".txt");
        let image = page.image;
        let path = root.join(format!("rust/oracle/stems-beam-scheduler-resume-{key}.txt"));
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => {
                assert!(
                    !saw_missing,
                    "installed Boundary-19 corpus has a gap before {key}"
                );
                text
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                saw_missing = true;
                continue;
            }
            Err(error) => panic!("cannot read {}: {error}", path.display()),
        };
        let b18_path = root.join(format!(
            "rust/oracle/stems-beam-vlink-outer-blinker-{key}.txt"
        ));
        let b18_bytes = std::fs::read(&b18_path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", b18_path.display()));
        let body: Vec<&str> = text.lines().filter(|line| !line.starts_with('#')).collect();
        let page_row = body.first().expect("resume fixture has a page row");
        assert!(
            page_row.starts_with(&format!("stemsbeamschedulerresumepage {image}#1 ")),
            "{key}: bad page row"
        );
        assert_eq!(
            resume_row_field(page_row, "outerBLinkerFixtureSha256"),
            sha256_hex(&b18_bytes),
            "{key}: resume fixture does not pin its exact Boundary-18 fixture"
        );
        assert_eq!(resume_row_field(page_row, "phaseScope"), "SidesOnly");

        let inputs = native_scheduler_inputs(image);
        let recognition = materialize_native_stems_beam_scheduler_frontiers(
            &inputs.beams,
            &inputs.beam_stumps,
            &inputs.beam_vlinkers,
            &inputs.beam_builders,
            &inputs.link_plans,
        )
        .unwrap_or_else(|error| panic!("{key}: scheduler frontiers failed: {error}"));

        for (index, system) in recognition.systems.iter().enumerate() {
            let system_id = system.system_id;
            let marker = format!(" system {system_id} ");
            let rows: Vec<&str> = body[1..]
                .iter()
                .copied()
                .filter(|row| row.contains(&marker))
                .collect();
            assert!(
                !rows.is_empty(),
                "{key}: no resume rows for system {system_id}"
            );
            let NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(awaiting) = &system.status
            else {
                panic!("{key} system {system_id}: prefix did not stop at a V-link frontier");
            };
            let completed = NativeStemsBeamCompletedVLinkEvidence {
                plan: awaiting.plan,
                b_linker: awaiting.b_linker,
                v_linker: awaiting.v_linker,
                // Boundary 18's gate proves the outer flag is true after every
                // real transaction on this same corpus.
                outer_b_linked_after: true,
            };
            let resume = resume_native_stems_beam_scheduler_after_transaction(
                system,
                &inputs.beam_vlinkers.systems[index],
                &inputs.beam_builders.systems[index],
                &inputs.link_plans.systems[index],
                &completed,
            )
            .unwrap_or_else(|error| panic!("{key} system {system_id}: resume failed: {error}"));

            let mut events = resume.resume_events.iter();
            let mut saw_terminal = false;
            for row in &rows {
                if row.starts_with("stemsbeamschedulerresumeside ") {
                    let Some(NativeStemsBeamSchedulerEvent::SideBLinkerResult {
                        beam,
                        side,
                        b_linker,
                        logical_success,
                        linked_flag_after,
                        ..
                    }) = events.next()
                    else {
                        panic!("{key} system {system_id}: fixture side row without typed event");
                    };
                    assert_eq!(
                        resume_row_field(row, "beamSig").parse::<usize>().unwrap(),
                        resume_pre_tremolo_sig(system, *beam),
                    );
                    assert_eq!(
                        resume_row_field(row, "bAlias"),
                        resume_b_alias(system, *b_linker)
                    );
                    let side_token = match side {
                        NativeStemHeadSide::Left => "LEFT",
                        NativeStemHeadSide::Right => "RIGHT",
                    };
                    assert_eq!(resume_row_field(row, "hSide"), side_token);
                    assert_eq!(
                        resume_row_field(row, "logicalResult"),
                        if *logical_success { "true" } else { "false" },
                    );
                    assert!(
                        *linked_flag_after,
                        "{key} system {system_id}: resumed side must carry the linked flag",
                    );
                } else if row.starts_with("stemsbeamschedulerresumefrontier ") {
                    saw_terminal = true;
                    let NativeStemsBeamSchedulerResumeStatus::AwaitingVLinkTransaction(frontier) =
                        &resume.status
                    else {
                        panic!("{key} system {system_id}: fixture frontier without typed frontier");
                    };
                    assert_eq!(
                        resume_row_field(row, "beamSig").parse::<usize>().unwrap(),
                        resume_pre_tremolo_sig(system, frontier.beam),
                    );
                    assert_eq!(
                        resume_row_field(row, "bAlias"),
                        resume_b_alias(system, frontier.b_linker),
                    );
                    assert_eq!(
                        resume_row_field(row, "vSide"),
                        match frontier.vertical_side {
                            NativeStemVerticalSide::Top => "TOP",
                            NativeStemVerticalSide::Bottom => "BOTTOM",
                        },
                    );
                    assert_eq!(
                        resume_row_field(row, "plan").parse::<usize>().unwrap(),
                        frontier.plan.plan_ordinal,
                    );
                    assert_eq!(
                        resume_row_field(row, "stemProfile").parse::<i32>().unwrap(),
                        frontier.plan.stem_profile,
                    );
                    let plan_system = &inputs.link_plans.systems[index];
                    let attempt = plan_system
                        .builders
                        .iter()
                        .find(|builder| builder.builder_ordinal == frontier.plan.builder_ordinal)
                        .and_then(|builder| {
                            builder
                                .attempts
                                .iter()
                                .find(|attempt| attempt.stem_profile == frontier.plan.stem_profile)
                        })
                        .unwrap_or_else(|| {
                            panic!("{key} system {system_id}: frontier plan missing from matrix")
                        });
                    assert_eq!(
                        resume_row_field(row, "lastIndex").parse::<i32>().unwrap(),
                        attempt
                            .expand_last_index
                            .expect("ready plan has a last index"),
                    );
                    assert_eq!(
                        resume_row_field(row, "relationEntries")
                            .parse::<usize>()
                            .unwrap(),
                        attempt.relations.len(),
                    );
                    assert_eq!(
                        resume_row_field(row, "glyphs").parse::<usize>().unwrap(),
                        attempt.glyphs.len(),
                    );
                    assert_eq!(
                        resume_row_field(row, "lineChanged"),
                        if frontier.would_apply_stored_line_delta.is_some() {
                            "true"
                        } else {
                            "false"
                        },
                    );
                } else if row.starts_with("stemsbeamschedulerresumeexhausted ") {
                    saw_terminal = true;
                    assert!(
                        matches!(
                            resume.status,
                            NativeStemsBeamSchedulerResumeStatus
                                ::SidesExhaustedBeforeSecondFrontier { .. }
                        ),
                        "{key} system {system_id}: fixture exhaustion without typed exhaustion",
                    );
                } else if row.starts_with("stemsbeamschedulerresumesummary ") {
                    let fixture_events = resume_row_field(row, "resumeEvents")
                        .parse::<usize>()
                        .unwrap();
                    assert_eq!(
                        fixture_events,
                        resume.resume_events.len() + 1,
                        "{key} system {system_id}: event count differs (fixture counts its \
                         terminal row; the typed terminal is the status)",
                    );
                    assert_eq!(
                        resume_row_field(row, "secondFrontier") == "true",
                        matches!(
                            resume.status,
                            NativeStemsBeamSchedulerResumeStatus::AwaitingVLinkTransaction(_)
                        ),
                    );
                } else {
                    panic!("{key} system {system_id}: unmodeled resume row kind: {row}");
                }
            }
            assert!(
                saw_terminal,
                "{key} system {system_id}: fixture lacks a terminal row"
            );
            assert!(
                events.next().is_none(),
                "{key} system {system_id}: typed resume produced unfixtured events",
            );
        }
        checked_pages += 1;
    }
    // Count what is on disk rather than trusting a floor: a gate that grades
    // two sheets of eight would satisfy `>= 2` while quietly ignoring six.
    let installed = PAGES
        .iter()
        .filter(|page| {
            let key = page
                .fixture
                .trim_start_matches("stems-beam-scheduler-")
                .trim_end_matches(".txt");
            root.join(format!("rust/oracle/stems-beam-scheduler-resume-{key}.txt"))
                .is_file()
        })
        .count();
    assert!(
        installed >= 2,
        "Boundary-19 has only {installed} sheets frozen"
    );
    assert_eq!(
        checked_pages, installed,
        "Boundary-19 graded {checked_pages} of {installed} installed sheets"
    );
}
