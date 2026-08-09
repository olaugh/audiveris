// SPDX-License-Identifier: AGPL-3.0-or-later

use super::*;

use audiveris_omr::{
    native_stems_beam_builders::{
        NativeStemsBeamBuilderPreBuilderAttachment,
        NativeStemsBeamBuilderPreBuilderGlyphRegistration,
    },
    native_stems_beam_reachability::{NativeStemsBeamFindResult, NativeStemsBeamTargetAction},
    native_stems_head_builders::{
        NativeStemsHeadBuilderRegistryEvent, NativeStemsHeadBuilderSystem,
    },
};

#[derive(Clone, Debug)]
pub(super) struct RegistryEndFields {
    pub filaments: usize,
    pub glyph_new: usize,
    pub glyph_reuse: usize,
    pub modeled_before: usize,
    pub modeled_after: usize,
    pub sha_before: String,
    pub sha_after: String,
    pub action_sha256: String,
}

#[derive(Clone, Debug, Default)]
pub(super) struct RegistrySystemProjection {
    pub stump_rows: Vec<String>,
    pub boundary: String,
    pub beam_rows: BTreeMap<usize, Vec<String>>,
    pub beam_chron_rows: BTreeMap<usize, String>,
    pub glyph_rows: BTreeMap<usize, Vec<String>>,
    pub end_fields: BTreeMap<usize, RegistryEndFields>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct RegistryProjection {
    pub baseline: String,
    pub systems: BTreeMap<usize, RegistrySystemProjection>,
}

struct ExpectedRegistryRows<'a> {
    baseline: &'a str,
    boundaries: BTreeMap<usize, &'a str>,
    stumps: BTreeMap<(usize, usize), &'a str>,
    beams: BTreeMap<(usize, usize, usize), &'a str>,
    beam_chron: BTreeMap<(usize, usize), &'a str>,
    heads: BTreeMap<(usize, usize, usize), &'a str>,
    ends: BTreeMap<(usize, usize), &'a str>,
}

impl<'a> ExpectedRegistryRows<'a> {
    fn parse(bytes: &'a [u8]) -> Self {
        let text = std::str::from_utf8(bytes).expect("head-builder fixture UTF-8");
        let mut value = Self {
            baseline: "",
            boundaries: BTreeMap::new(),
            stumps: BTreeMap::new(),
            beams: BTreeMap::new(),
            beam_chron: BTreeMap::new(),
            heads: BTreeMap::new(),
            ends: BTreeMap::new(),
        };
        for row in text
            .lines()
            .filter(|row| !row.is_empty() && !row.starts_with('#'))
        {
            match row.split_whitespace().next().expect("fixture row family") {
                "stemsheadbuilderoriginbaseline" => {
                    assert!(value.baseline.is_empty(), "duplicate origin baseline");
                    value.baseline = row;
                }
                "stemsheadbuilderregistryboundary" => {
                    let key = required_usize(row, "system");
                    assert!(value.boundaries.insert(key, row).is_none());
                }
                "stemsheadbuilderstumpreg" => {
                    let key = (required_usize(row, "system"), required_usize(row, "event"));
                    assert!(value.stumps.insert(key, row).is_none());
                }
                "stemsheadbuilderbeamreg" => {
                    let key = (
                        required_usize(row, "system"),
                        required_usize(row, "builder"),
                        required_usize(row, "ordinal"),
                    );
                    assert!(value.beams.insert(key, row).is_none());
                }
                "stemsheadbuilderbeamchron" => {
                    let key = (
                        required_usize(row, "system"),
                        required_usize(row, "ordinal"),
                    );
                    assert!(value.beam_chron.insert(key, row).is_none());
                }
                "stemsheadbuilderglyphreg" => {
                    let key = (
                        required_usize(row, "system"),
                        required_usize(row, "builder"),
                        required_usize(row, "ordinal"),
                    );
                    assert!(value.heads.insert(key, row).is_none());
                }
                "stemsheadbuilderend" => {
                    let key = (
                        required_usize(row, "system"),
                        required_usize(row, "builder"),
                    );
                    assert!(value.ends.insert(key, row).is_none());
                }
                _ => {}
            }
        }
        assert!(!value.baseline.is_empty(), "missing origin baseline");
        value
    }
}

pub(super) fn validate_registry_projection(
    page: &str,
    bytes: &[u8],
    native: &NativeProjectionPage,
) -> RegistryProjection {
    let expected = ExpectedRegistryRows::parse(bytes);
    validate_event_glyph_shapes(page, native, &expected);
    let mut full = BoundedOriginRegistry::from_pre_stems(native);
    let mut beam_only = full.clone();
    let mut full_native_aliases = NativeOrdinalAliases::default();
    let mut beam_only_native_aliases = NativeOrdinalAliases::default();
    let baseline = baseline_row(page, native, &full);
    assert_eq!(baseline, expected.baseline, "{page}: origin baseline row");
    let mut projection = RegistryProjection {
        baseline,
        systems: BTreeMap::new(),
    };

    let mut stump_rows = 0usize;
    let mut beam_rows = 0usize;
    let mut head_rows = 0usize;
    for (system_index, system) in native.head_builders.systems.iter().enumerate() {
        let beam_system = &native.beam_builders.systems[system_index];
        let beam_stumps = &native.beam_stumps.systems[system_index];
        let seed_system = &native.stem_seeds.systems[system_index];
        let kept_seed_system = &native.head_seeds.systems[system_index];
        assert_eq!(system.system_id, beam_system.system_id);
        assert_eq!(system.system_id, beam_stumps.system_id);
        assert_eq!(system.system_id, seed_system.raw.system_id);
        assert_eq!(system.system_id, kept_seed_system.system_id);
        let mut projected_system = RegistrySystemProjection::default();

        for &free_glyph_ordinal in &kept_seed_system.kept_seed_ordinals {
            let glyph = seed_glyph(seed_system, free_glyph_ordinal);
            full.label(&glyph, "stemSeed");
            beam_only.label(&glyph, "stemSeed");
        }

        let count_before_stumps = full.count();
        let sha_before_stumps = full.history_sha256();
        let mut event_cursor = 0usize;
        for pre_builder in &beam_system.pre_builder_glyph_registrations {
            let event = system
                .registry_events
                .get(event_cursor)
                .expect("missing pre-builder registry event");
            let projected = validate_stump_event(
                page,
                system,
                beam_stumps,
                pre_builder,
                event,
                &mut full,
                &mut beam_only,
                expected
                    .stumps
                    .get(&(system.system_id, pre_builder.event_ordinal))
                    .copied()
                    .expect("missing stump fixture row"),
            );
            projected_system.stump_rows.push(projected);
            let glyph = event_glyph(event);
            full_native_aliases.observe(
                page,
                "full registry",
                event.modeled_canonical_ordinal,
                &glyph,
            );
            beam_only_native_aliases.observe(
                page,
                "beam-only registry",
                pre_builder.modeled_canonical_ordinal,
                &glyph,
            );
            event_cursor += 1;
            stump_rows += 1;
        }
        let boundary = expected
            .boundaries
            .get(&system.system_id)
            .copied()
            .expect("missing registry boundary row");
        assert_token(boundary, "phase", "afterStumps", page);
        assert_usize(boundary, "modeledBefore", count_before_stumps, page);
        assert_usize(boundary, "modeledAfter", full.count(), page);
        assert_token(boundary, "shaBefore", &sha_before_stumps, page);
        assert_token(boundary, "shaAfter", &full.history_sha256(), page);
        projected_system.boundary = format!(
            "stemsheadbuilderregistryboundary {page} system {} phase afterStumps modeledBefore {count_before_stumps} modeledAfter {} shaBefore {sha_before_stumps} shaAfter {}",
            system.system_id,
            full.count(),
            full.history_sha256(),
        );
        assert_eq!(
            projected_system.boundary, boundary,
            "{page}: registry boundary row"
        );

        let beam_reach = native
            .beam_reachability
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system.system_id)
            .expect("beam chronology reachability system");
        let initial_b_count = beam_reach
            .final_beam_arenas
            .iter()
            .map(|arena| arena.initial_b_count)
            .sum::<usize>();
        let mut created_so_far = 0usize;
        let beam_inspections = beam_reach
            .beam_inspections
            .iter()
            .flat_map(|inspection| &inspection.b_visits)
            .filter(|visit| !visit.skipped_anchor)
            .flat_map(|visit| &visit.v_inspections)
            .collect::<Vec<_>>();
        assert_eq!(
            beam_inspections.len(),
            beam_system.builders.len(),
            "{page}: beam V-inspection/builder coverage"
        );

        for (beam_builder, inspection) in beam_system.builders.iter().zip(beam_inspections) {
            assert_eq!(
                beam_builder.start, inspection.reference,
                "{page}: beam inspection order"
            );
            let count_before = full.count();
            let sha_before = full.history_sha256();
            let mut actions = Vec::new();
            let mut news = 0usize;
            let mut reuses = 0usize;
            for (ordinal, registration) in beam_builder.glyph_registrations.iter().enumerate() {
                let event = system
                    .registry_events
                    .get(event_cursor)
                    .expect("missing beam registry event");
                let projected = validate_beam_event(
                    page,
                    system,
                    beam_builder.builder_ordinal,
                    ordinal,
                    registration,
                    event,
                    &mut full,
                    &mut beam_only,
                    expected
                        .beams
                        .get(&(system.system_id, beam_builder.builder_ordinal, ordinal))
                        .copied()
                        .expect("missing beam registration fixture row"),
                    &mut actions,
                    &mut news,
                    &mut reuses,
                );
                projected_system
                    .beam_rows
                    .entry(beam_builder.builder_ordinal)
                    .or_default()
                    .push(projected);
                let glyph = event_glyph(event);
                full_native_aliases.observe(
                    page,
                    "full registry",
                    event.modeled_canonical_ordinal,
                    &glyph,
                );
                beam_only_native_aliases.observe(
                    page,
                    "beam-only registry",
                    registration.modeled_canonical_ordinal,
                    &glyph,
                );
                event_cursor += 1;
                beam_rows += 1;
            }
            let row = expected
                .beam_chron
                .get(&(system.system_id, beam_builder.builder_ordinal))
                .copied()
                .expect("missing beam chronology fixture row");
            assert_usize(row, "filaments", news + reuses, page);
            assert_usize(row, "glyphNew", news, page);
            assert_usize(row, "glyphReuse", reuses, page);
            assert_usize(row, "modeledRegistryBefore", count_before, page);
            assert_usize(row, "modeledRegistryAfter", full.count(), page);
            assert_token(row, "registryShaBefore", &sha_before, page);
            assert_token(row, "registryShaAfter", &full.history_sha256(), page);
            assert_token(row, "registrationActionSha256", &sha256_hex(&actions), page);
            let created = inspection
                .beam_decisions
                .iter()
                .filter(|decision| {
                    matches!(
                        &decision.action,
                        NativeStemsBeamTargetAction::FindLinker(find)
                            if matches!(find.result, NativeStemsBeamFindResult::CreatedAnchor(_))
                    )
                })
                .count();
            let b_before = initial_b_count + created_so_far;
            let b_after = b_before + created;
            created_so_far += created;
            let projected = format!(
                "stemsheadbuilderbeamchron {page} system {} ordinal {} beamSig {} bAlias {} vSide {} profile {} filaments {} glyphNew {news} glyphReuse {reuses} bArenaBefore {b_before} bArenaAfter {b_after} modeledRegistryBefore {count_before} modeledRegistryAfter {} registryShaBefore {sha_before} registryShaAfter {} registrationActionSha256 {}",
                system.system_id,
                beam_builder.builder_ordinal,
                beam_sig_ordinal(beam_stumps, beam_builder.start.b_linker.beam),
                b_alias(beam_stumps, beam_builder.start.b_linker),
                vertical_side_token(beam_builder.start.side),
                beam_builder.max_stem_profile,
                news + reuses,
                full.count(),
                full.history_sha256(),
                sha256_hex(&actions),
            );
            assert_eq!(projected, row, "{page}: beam chronology row");
            assert!(
                projected_system
                    .beam_chron_rows
                    .insert(beam_builder.builder_ordinal, projected)
                    .is_none(),
                "duplicate beam chronology row"
            );
        }
        assert_eq!(
            projected_system.beam_chron_rows.len(),
            beam_system.builders.len(),
            "{page}: beam inspection coverage"
        );

        for builder in &system.builders {
            let count_before = full.count();
            let sha_before = full.history_sha256();
            let mut actions = Vec::new();
            let mut news = 0usize;
            let mut reuses = 0usize;
            for (ordinal, registration) in builder.glyph_registrations.iter().enumerate() {
                let event = system
                    .registry_events
                    .get(event_cursor)
                    .expect("missing head registry event");
                let projected = validate_head_event(
                    page,
                    system,
                    builder.builder_ordinal,
                    ordinal,
                    registration,
                    event,
                    &mut full,
                    expected
                        .heads
                        .get(&(system.system_id, builder.builder_ordinal, ordinal))
                        .copied()
                        .expect("missing head registration fixture row"),
                    &mut actions,
                    &mut news,
                    &mut reuses,
                );
                projected_system
                    .glyph_rows
                    .entry(builder.builder_ordinal)
                    .or_default()
                    .push(projected);
                full_native_aliases.observe(
                    page,
                    "full registry",
                    event.modeled_canonical_ordinal,
                    &event_glyph(event),
                );
                event_cursor += 1;
                head_rows += 1;
            }
            let row = expected
                .ends
                .get(&(system.system_id, builder.builder_ordinal))
                .copied()
                .expect("missing builder-end fixture row");
            assert_usize(row, "filaments", news + reuses, page);
            assert_usize(row, "glyphNew", news, page);
            assert_usize(row, "glyphReuse", reuses, page);
            assert_usize(row, "modeledRegistryBefore", count_before, page);
            assert_usize(row, "modeledRegistryAfter", full.count(), page);
            assert_token(row, "registryShaBefore", &sha_before, page);
            assert_token(row, "registryShaAfter", &full.history_sha256(), page);
            assert_token(row, "registrationActionSha256", &sha256_hex(&actions), page);
            assert!(
                projected_system
                    .end_fields
                    .insert(
                        builder.builder_ordinal,
                        RegistryEndFields {
                            filaments: news + reuses,
                            glyph_new: news,
                            glyph_reuse: reuses,
                            modeled_before: count_before,
                            modeled_after: full.count(),
                            sha_before,
                            sha_after: full.history_sha256(),
                            action_sha256: sha256_hex(&actions),
                        },
                    )
                    .is_none(),
                "duplicate builder registry end"
            );
        }
        assert_eq!(
            event_cursor,
            system.registry_events.len(),
            "{page} system {} registry event chronology",
            system.system_id
        );
        assert!(
            projection
                .systems
                .insert(system.system_id, projected_system)
                .is_none(),
            "duplicate projected registry system"
        );
    }
    assert_eq!(
        stump_rows,
        expected.stumps.len(),
        "{page} stump row coverage"
    );
    assert_eq!(beam_rows, expected.beams.len(), "{page} beam row coverage");
    assert_eq!(head_rows, expected.heads.len(), "{page} head row coverage");
    projection
}

fn validate_event_glyph_shapes(
    page: &str,
    native: &NativeProjectionPage,
    expected: &ExpectedRegistryRows<'_>,
) {
    for system in &native.head_builders.systems {
        for event in &system.registry_events {
            let row = match event.occurrence {
                NativeStemsHeadBuilderRegistryOccurrence::PreBuilder { event_ordinal, .. } => {
                    expected
                        .stumps
                        .get(&(system.system_id, event_ordinal))
                        .copied()
                        .expect("missing stump fixture shape")
                }
                NativeStemsHeadBuilderRegistryOccurrence::BeamChunk {
                    builder_ordinal,
                    filament_ordinal,
                } => expected
                    .beams
                    .get(&(system.system_id, builder_ordinal, filament_ordinal))
                    .copied()
                    .expect("missing beam fixture shape"),
                NativeStemsHeadBuilderRegistryOccurrence::HeadChunk {
                    builder_ordinal,
                    filament_ordinal,
                } => expected
                    .heads
                    .get(&(system.system_id, builder_ordinal, filament_ordinal))
                    .copied()
                    .expect("missing head fixture shape"),
            };
            let glyph = event_glyph(event);
            let members = match event.occurrence {
                NativeStemsHeadBuilderRegistryOccurrence::HeadChunk {
                    builder_ordinal,
                    filament_ordinal,
                } => format!(
                    "{:?}",
                    system.builders[builder_ordinal].filaments[filament_ordinal]
                        .member_section_source_ordinals
                ),
                _ => "-".to_owned(),
            };
            assert_eq!(
                required_value(row, "bounds"),
                bounded_bounds_token(glyph.bounds),
                "{page}: registry event {:?} glyph bounds; native filament members {members}",
                event.occurrence
            );
            validate_glyph_tokens(page, row, &glyph, event.weight);
        }
    }
}

/// Native canonical ordinals are dense insertion-order handles. Oracle
/// canonical aliases deliberately use a sorted structural baseline, so those
/// numeric ordinals are not comparable across implementations. What is
/// semantic is a stable bijection between each native handle and structural
/// glyph key within one registry chronology.
#[derive(Default)]
struct NativeOrdinalAliases {
    by_ordinal: BTreeMap<usize, String>,
    by_glyph: BTreeMap<String, usize>,
}

impl NativeOrdinalAliases {
    fn observe(&mut self, page: &str, registry: &str, ordinal: usize, glyph: &BoundedGlyph) {
        let glyph = bounded_glyph_alias(glyph);
        if let Some(previous) = self.by_ordinal.insert(ordinal, glyph.clone()) {
            assert_eq!(
                previous, glyph,
                "{page}: {registry} native ordinal {ordinal} changed structural identity"
            );
        }
        if let Some(previous) = self.by_glyph.insert(glyph.clone(), ordinal) {
            assert_eq!(
                previous, ordinal,
                "{page}: {registry} structural glyph {glyph} acquired two native ordinals"
            );
        }
    }
}

fn baseline_row(
    page: &str,
    native: &NativeProjectionPage,
    registry: &BoundedOriginRegistry,
) -> String {
    let baseline = &native.head_builders.registry_baseline;
    assert!(
        !baseline.complete_java_glyph_index,
        "{page}: bounded baseline unexpectedly claims the global Java index"
    );
    format!(
        "stemsheadbuilderoriginbaseline {page} rawSeedCandidates {} checkedSeedCandidates {} checkedSeedStructuralKeys {} modeledRegistry {} registrySha256 {}",
        baseline.stem_seed_raw_candidates,
        baseline.stem_seed_glyphs,
        baseline.stem_seed_unique_contents,
        registry.count(),
        registry.history_sha256(),
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_stump_event(
    page: &str,
    system: &NativeStemsHeadBuilderSystem,
    beam_stumps: &NativeStemsBeamStumpSystem,
    pre_builder: &NativeStemsBeamBuilderPreBuilderGlyphRegistration,
    event: &NativeStemsHeadBuilderRegistryEvent,
    full: &mut BoundedOriginRegistry,
    beam_only: &mut BoundedOriginRegistry,
    row: &str,
) -> String {
    let NativeStemsHeadBuilderRegistryOccurrence::PreBuilder {
        event_ordinal,
        source,
    } = event.occurrence
    else {
        panic!("{page}: stump chronology reached non-stump event");
    };
    assert_eq!(event_ordinal, pre_builder.event_ordinal);
    assert_eq!(source, pre_builder.source);
    assert_eq!(event.system_event_ordinal, pre_builder.event_ordinal);
    assert_eq!(event.bounds, pre_builder.bounds);
    assert_eq!(event.run_table, pre_builder.run_table);
    let glyph = event_glyph(event);
    let (kind, source_token, category, attachment_category) = match source {
        NativeStemsBeamBuilderPreBuilderGlyphSource::BeamStump {
            beam_x_ordinal,
            beam,
            side,
        } => (
            "Beam",
            format!(
                "beam:{beam_x_ordinal}:sig:{}:side:{}",
                beam_sig_ordinal(beam_stumps, beam),
                head_side_token(side)
            ),
            "beamStumpCandidate",
            match pre_builder.attachment {
                NativeStemsBeamBuilderPreBuilderAttachment::Attached => "beamStump",
                NativeStemsBeamBuilderPreBuilderAttachment::RejectedAfterRegistration => {
                    "rejectedBeamStump"
                }
            },
        ),
        NativeStemsBeamBuilderPreBuilderGlyphSource::HeadStump {
            head_x_ordinal,
            head_sig_ordinal,
            constructor_ordinal,
        } => (
            "Head",
            format!("head:{head_x_ordinal}:sig:{head_sig_ordinal}:ctor:{constructor_ordinal}"),
            "headStumpCandidate",
            match pre_builder.attachment {
                NativeStemsBeamBuilderPreBuilderAttachment::Attached => "headStump",
                NativeStemsBeamBuilderPreBuilderAttachment::RejectedAfterRegistration => {
                    "rejectedHeadStump"
                }
            },
        ),
    };
    let history_source = format!("stump:{}:{event_ordinal}:{source_token}", system.system_id);
    let before = full.count();
    let registration = full.register(glyph.clone(), category, &history_source);
    let isolated = beam_only.register(glyph.clone(), category, &history_source);
    validate_native_event(page, event, before, &registration);
    assert_eq!(event.action, registration_action(registration.reuse));
    assert_eq!(
        pre_builder.action,
        registration_action(isolated.reuse),
        "{page}: beam-only stump source action"
    );
    assert_eq!(event.isolated_beam_action, None);
    full.label(&glyph, attachment_category);
    beam_only.label(&glyph, attachment_category);

    assert_usize(row, "event", event_ordinal, page);
    assert_token(row, "kind", kind, page);
    assert_token(row, "source", &source_token, page);
    assert_token(row, "result", action_token(registration.reuse), page);
    assert_token(row, "beamOnlyResult", action_token(isolated.reuse), page);
    assert_bool(
        row,
        "actionDiff",
        registration.reuse != isolated.reuse,
        page,
    );
    assert_token(row, "canonicalAlias", &registration.canonical_alias, page);
    assert_token(row, "canonical", &bounded_glyph_alias(&glyph), page);
    assert_token(
        row,
        "preOriginCategories",
        &registration.prior_categories,
        page,
    );
    assert_token(
        row,
        "attachment",
        match pre_builder.attachment {
            NativeStemsBeamBuilderPreBuilderAttachment::Attached => "Attached",
            NativeStemsBeamBuilderPreBuilderAttachment::RejectedAfterRegistration => {
                "RejectedAfterRegistration"
            }
        },
        page,
    );
    validate_glyph_tokens(page, row, &glyph, event.weight);
    let projected = format!(
        "stemsheadbuilderstumpreg {page} system {} event {event_ordinal} kind {kind} source {source_token} result {} beamOnlyResult {} actionDiff {} canonicalAlias {} canonical {} preOriginCategories {} attachment {} bounds {} orientation {} weight {} runs {} sha256 {}",
        system.system_id,
        action_token(registration.reuse),
        action_token(isolated.reuse),
        registration.reuse != isolated.reuse,
        registration.canonical_alias,
        bounded_glyph_alias(&glyph),
        registration.prior_categories,
        match pre_builder.attachment {
            NativeStemsBeamBuilderPreBuilderAttachment::Attached => "Attached",
            NativeStemsBeamBuilderPreBuilderAttachment::RejectedAfterRegistration => {
                "RejectedAfterRegistration"
            }
        },
        bounded_bounds_token(glyph.bounds),
        orientation_token(glyph.run_table.orientation()),
        event.weight,
        glyph_run_count(&glyph.run_table),
        glyph_run_sha256(&glyph.run_table),
    );
    assert_eq!(projected, row, "{page}: stump registration row");
    projected
}

#[allow(clippy::too_many_arguments)]
fn validate_beam_event(
    page: &str,
    system: &NativeStemsHeadBuilderSystem,
    builder_ordinal: usize,
    ordinal: usize,
    source_registration: &audiveris_omr::native_stems_beam_builders::NativeStemsBeamBuilderGlyphRegistration,
    event: &NativeStemsHeadBuilderRegistryEvent,
    full: &mut BoundedOriginRegistry,
    beam_only: &mut BoundedOriginRegistry,
    row: &str,
    actions: &mut Vec<u8>,
    news: &mut usize,
    reuses: &mut usize,
) -> String {
    assert_eq!(
        event.occurrence,
        NativeStemsHeadBuilderRegistryOccurrence::BeamChunk {
            builder_ordinal,
            filament_ordinal: ordinal,
        },
        "{page}: beam occurrence"
    );
    assert_eq!(event.bounds, source_registration.bounds);
    assert_eq!(event.run_table, source_registration.run_table);
    let glyph = event_glyph(event);
    let source = format!("beam:{}:{builder_ordinal}:{ordinal}", system.system_id);
    let before = full.count();
    let registration = full.register(glyph.clone(), "beamBuilderChunk", &source);
    let isolated = beam_only.register(glyph.clone(), "beamBuilderChunk", &source);
    validate_native_event(page, event, before, &registration);
    assert_eq!(event.action, registration_action(registration.reuse));
    assert_eq!(
        event.isolated_beam_action,
        Some(registration_action(isolated.reuse)),
        "{page}: isolated beam action"
    );
    assert_eq!(
        event.action_changed_from_isolated_beam,
        registration.reuse != isolated.reuse,
        "{page}: beam action change"
    );
    assert_eq!(
        source_registration.action,
        registration_action(isolated.reuse),
        "{page}: beam-only source action"
    );
    *reuses += usize::from(registration.reuse);
    *news += usize::from(!registration.reuse);
    actions.extend_from_slice(
        format!(
            "{}:{}:{}\n",
            action_token(registration.reuse),
            bounded_glyph_alias(&glyph),
            registration.prior_categories
        )
        .as_bytes(),
    );

    assert_token(row, "result", action_token(registration.reuse), page);
    assert_token(row, "canonicalAlias", &registration.canonical_alias, page);
    assert_token(row, "canonical", &bounded_glyph_alias(&glyph), page);
    assert_token(
        row,
        "preOriginCategories",
        &registration.prior_categories,
        page,
    );
    assert_token(row, "beamOnlyResult", action_token(isolated.reuse), page);
    assert_token(
        row,
        "beamOnlyPreOriginCategories",
        &isolated.prior_categories,
        page,
    );
    let changed = registration.reuse != isolated.reuse;
    assert_bool(row, "actionDiff", changed, page);
    assert_bool(
        row,
        "headToLaterBeamReuse",
        changed
            && registration
                .prior_categories
                .split(',')
                .any(|category| category == "headBuilderChunk"),
        page,
    );
    validate_glyph_tokens(page, row, &glyph, event.weight);
    let changed = registration.reuse != isolated.reuse;
    let head_to_later = changed
        && registration
            .prior_categories
            .split(',')
            .any(|category| category == "headBuilderChunk");
    let projected = format!(
        "stemsheadbuilderbeamreg {page} system {} builder {builder_ordinal} ordinal {ordinal} result {} canonicalAlias {} canonical {} preOriginCategories {} beamOnlyResult {} beamOnlyPreOriginCategories {} actionDiff {changed} headToLaterBeamReuse {head_to_later} bounds {} orientation {} weight {} runs {} sha256 {}",
        system.system_id,
        action_token(registration.reuse),
        registration.canonical_alias,
        bounded_glyph_alias(&glyph),
        registration.prior_categories,
        action_token(isolated.reuse),
        isolated.prior_categories,
        bounded_bounds_token(glyph.bounds),
        orientation_token(glyph.run_table.orientation()),
        event.weight,
        glyph_run_count(&glyph.run_table),
        glyph_run_sha256(&glyph.run_table),
    );
    assert_eq!(projected, row, "{page}: beam registration row");
    projected
}

#[allow(clippy::too_many_arguments)]
fn validate_head_event(
    page: &str,
    system: &NativeStemsHeadBuilderSystem,
    builder_ordinal: usize,
    ordinal: usize,
    source_registration: &audiveris_omr::native_stems_head_builders::NativeStemsHeadBuilderGlyphRegistration,
    event: &NativeStemsHeadBuilderRegistryEvent,
    full: &mut BoundedOriginRegistry,
    row: &str,
    actions: &mut Vec<u8>,
    news: &mut usize,
    reuses: &mut usize,
) -> String {
    assert_eq!(
        event.occurrence,
        NativeStemsHeadBuilderRegistryOccurrence::HeadChunk {
            builder_ordinal,
            filament_ordinal: ordinal,
        },
        "{page}: head occurrence"
    );
    assert_eq!(event.bounds, source_registration.bounds);
    assert_eq!(event.run_table, source_registration.run_table);
    let glyph = event_glyph(event);
    let source = format!("head:{}:{builder_ordinal}:{ordinal}", system.system_id);
    let before = full.count();
    let registration = full.register(glyph.clone(), "headBuilderChunk", &source);
    validate_native_event(page, event, before, &registration);
    assert_eq!(event.action, registration_action(registration.reuse));
    assert_eq!(source_registration.action, event.action);
    assert_eq!(
        source_registration.modeled_canonical_ordinal,
        event.modeled_canonical_ordinal
    );
    *reuses += usize::from(registration.reuse);
    *news += usize::from(!registration.reuse);
    actions.extend_from_slice(
        format!(
            "{}:{}:{}\n",
            action_token(registration.reuse),
            bounded_glyph_alias(&glyph),
            registration.prior_categories
        )
        .as_bytes(),
    );

    assert_eq!(
        required_value(row, "result"),
        action_token(registration.reuse),
        "{page}: head registration result at system {} builder {builder_ordinal} ordinal {ordinal}; native event {} prior categories {}; earlier matching events {:?}",
        system.system_id,
        event.system_event_ordinal,
        registration.prior_categories,
        system
            .registry_events
            .iter()
            .take_while(|candidate| { candidate.system_event_ordinal < event.system_event_ordinal })
            .filter(|candidate| {
                candidate.bounds == event.bounds && candidate.run_table == event.run_table
            })
            .map(|candidate| (candidate.system_event_ordinal, candidate.occurrence))
            .collect::<Vec<_>>()
    );
    assert_token(row, "canonicalAlias", &registration.canonical_alias, page);
    assert_token(row, "canonical", &bounded_glyph_alias(&glyph), page);
    assert_token(
        row,
        "originCategories",
        &registration.prior_categories,
        page,
    );
    validate_glyph_tokens(page, row, &glyph, event.weight);
    let projected = format!(
        "stemsheadbuilderglyphreg {page} system {} builder {builder_ordinal} ordinal {ordinal} result {} canonicalAlias {} canonical {} originCategories {} bounds {} orientation {} weight {} runs {} sha256 {}",
        system.system_id,
        action_token(registration.reuse),
        registration.canonical_alias,
        bounded_glyph_alias(&glyph),
        registration.prior_categories,
        bounded_bounds_token(glyph.bounds),
        orientation_token(glyph.run_table.orientation()),
        event.weight,
        glyph_run_count(&glyph.run_table),
        glyph_run_sha256(&glyph.run_table),
    );
    assert_eq!(projected, row, "{page}: head registration row");
    projected
}

fn validate_native_event(
    page: &str,
    event: &NativeStemsHeadBuilderRegistryEvent,
    before: usize,
    registration: &BoundedRegistration,
) {
    assert_eq!(
        event.modeled_count_before, before,
        "{page}: registry before"
    );
    assert_eq!(
        event.modeled_count_after,
        before + usize::from(!registration.reuse),
        "{page}: registry after"
    );
    validate_dense_native_ordinal(
        page,
        "full registry event",
        event.modeled_canonical_ordinal,
        before,
        registration.reuse,
    );
}

fn validate_dense_native_ordinal(
    page: &str,
    scope: &str,
    ordinal: usize,
    count_before: usize,
    reuse: bool,
) {
    if reuse {
        assert!(
            ordinal < count_before,
            "{page}: {scope} reused ordinal {ordinal} outside count {count_before}"
        );
    } else {
        assert_eq!(
            ordinal, count_before,
            "{page}: {scope} new ordinal must append to dense registry"
        );
    }
}

fn validate_glyph_tokens(page: &str, row: &str, glyph: &BoundedGlyph, weight: usize) {
    assert_token(row, "bounds", &bounded_bounds_token(glyph.bounds), page);
    assert_token(
        row,
        "orientation",
        orientation_token(glyph.run_table.orientation()),
        page,
    );
    assert_usize(row, "weight", weight, page);
    assert_usize(row, "runs", glyph_run_count(&glyph.run_table), page);
    assert_token(row, "sha256", &glyph_run_sha256(&glyph.run_table), page);
}

fn seed_glyph(
    system: &audiveris_omr::native_stem_seeds::NativeStemSeedSystemRecognition,
    free_glyph_ordinal: usize,
) -> BoundedGlyph {
    let glyph = &system.free_glyphs[free_glyph_ordinal];
    BoundedGlyph {
        bounds: BoundedBounds {
            x: i32::try_from(glyph.bounds.x).expect("seed glyph x"),
            y: i32::try_from(glyph.bounds.y).expect("seed glyph y"),
            width: i32::try_from(glyph.bounds.width).expect("seed glyph width"),
            height: i32::try_from(glyph.bounds.height).expect("seed glyph height"),
        },
        run_table: glyph.run_table.clone(),
    }
}

fn event_glyph(event: &NativeStemsHeadBuilderRegistryEvent) -> BoundedGlyph {
    BoundedGlyph {
        bounds: BoundedBounds {
            x: i32::try_from(event.bounds.x).expect("registry glyph x"),
            y: i32::try_from(event.bounds.y).expect("registry glyph y"),
            width: i32::try_from(event.bounds.width).expect("registry glyph width"),
            height: i32::try_from(event.bounds.height).expect("registry glyph height"),
        },
        run_table: event.run_table.clone(),
    }
}

fn registration_action(reuse: bool) -> NativeStemsBeamBuilderRegistrationAction {
    if reuse {
        NativeStemsBeamBuilderRegistrationAction::ReusedModeledCanonical
    } else {
        NativeStemsBeamBuilderRegistrationAction::NewInModeledRegistry {
            global_novelty: audiveris_omr::native_stems_beam_builders::NativeStemsBeamBuilderGlobalNovelty::UnresolvedIncompleteBaseline,
        }
    }
}

fn action_token(reuse: bool) -> &'static str {
    if reuse { "Reuse" } else { "New" }
}

fn bounded_bounds_token(bounds: BoundedBounds) -> String {
    format!(
        "{}:{}:{}:{}",
        bounds.x, bounds.y, bounds.width, bounds.height
    )
}

fn assert_token(row: &str, field: &str, actual: &str, page: &str) {
    assert_eq!(
        required_value(row, field),
        actual,
        "{page}: registry field {field}: {row}"
    );
}

fn assert_usize(row: &str, field: &str, actual: usize, page: &str) {
    assert_eq!(
        required_usize(row, field),
        actual,
        "{page}: registry field {field}: {row}"
    );
}

fn assert_bool(row: &str, field: &str, actual: bool, page: &str) {
    assert_eq!(
        required_bool(row, field),
        actual,
        "{page}: registry field {field}: {row}"
    );
}
