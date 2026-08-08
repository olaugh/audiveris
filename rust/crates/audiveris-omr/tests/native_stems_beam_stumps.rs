// SPDX-License-Identifier: AGPL-3.0-or-later

//! Eight-page differential gate for constructor-time STEMS beam stumps.

use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
};

use audiveris_image::{
    beam_structure::Segment,
    run_table::Orientation,
    section::{Bounds, Section},
};

use audiveris_omr::{
    beam_inters::BeamKind,
    beam_recognizer::run_table_center_line,
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads,
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::recognize_native_stem_seeds,
    native_stems_beam_stumps::{
        NativeStemsBeamArea, NativeStemsBeamRegistration, NativeStemsBeamSeedPurgeAction,
        NativeStemsBeamSource, NativeStemsBeamStumpBeam, NativeStemsBeamStumpRecognition,
        NativeStemsBeamStumpRef, materialize_native_stems_beam_stumps,
    },
    native_stems_head_corners::materialize_native_stems_head_corners,
    native_stems_head_seeds::materialize_native_stems_head_seeds,
    recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds},
    stems_step::{NativeBeamPortion, NativeStemHeadSide, NativeStemVerticalSide},
};

const PAGES: &[&str] = &[
    "chula.png",
    "allegretto.png",
    "batuque.png",
    "carmen.png",
    "cucaracha.png",
    "hove.png",
    "zizi.png",
    "BachInvention5.jpg",
];
const ORACLE_PATH: &str = "rust/oracle/stems-beam-stumps.txt";

#[derive(Default, Debug, Eq, PartialEq)]
struct Totals {
    systems: usize,
    constructors: usize,
    sides: usize,
    neighbors: usize,
    seed_inputs: usize,
    purge_comparisons: usize,
    purge_removals: usize,
    purge_breaks: usize,
    side_seeds: usize,
    build_attempts: usize,
    empty_sections: usize,
    zero_compounds: usize,
    candidates: usize,
    direction_accepted: usize,
    direction_rejected: usize,
    registrations: usize,
    new_builds: usize,
    reused_builds: usize,
    section_rows: usize,
    steps: usize,
    final_stumps: usize,
    final_side_stumps: usize,
    tremolos: usize,
}

#[test]
fn native_stems_beam_stumps_match_java_corpus_exactly() {
    let oracle = std::fs::read_to_string(repo_path(ORACLE_PATH)).expect("beam-stump oracle");
    let summary = oracle
        .lines()
        .find(|line| line.starts_with("stemsbeamstumpcorpussummary "))
        .expect("beam-stump corpus summary");
    assert_eq!(field_value(summary, "pages"), "8");
    assert_eq!(field_value(summary, "systems"), "30");
    assert_eq!(field_value(summary, "constructors"), "803");
    assert_eq!(field_value(summary, "sides"), "1606");
    assert_eq!(field_value(summary, "neighbors"), "3934");
    assert_eq!(field_value(summary, "seedInputs"), "1820");
    assert_eq!(field_value(summary, "purgeComparisons"), "1087");
    assert_eq!(field_value(summary, "purgeRemovals"), "5");
    assert_eq!(field_value(summary, "purgeBreaks"), "1082");
    assert_eq!(field_value(summary, "sideSeeds"), "1305");
    assert_eq!(field_value(summary, "buildAttempts"), "301");
    assert_eq!(field_value(summary, "emptySections"), "4");
    assert_eq!(field_value(summary, "zeroCompounds"), "154");
    assert_eq!(field_value(summary, "candidates"), "143");
    assert_eq!(field_value(summary, "directionAccepted"), "6");
    assert_eq!(field_value(summary, "directionRejected"), "137");
    assert_eq!(field_value(summary, "registrations"), "5");
    assert_eq!(field_value(summary, "newBuilds"), "5");
    assert_eq!(field_value(summary, "reusedBuilds"), "1");
    assert_eq!(field_value(summary, "sectionRows"), "447");
    assert_eq!(field_value(summary, "steps"), "447");
    assert_eq!(field_value(summary, "finalStumps"), "1821");
    assert_eq!(field_value(summary, "finalSideStumps"), "1311");
    assert_eq!(field_value(summary, "tremolos"), "0");
    assert_eq!(
        field_value(summary, "probeSourceSha256"),
        "98c19499ca486fda8ddec92f18f9e3de54f27041987b011220babbf202dc0039"
    );
    assert_eq!(
        field_value(summary, "runnerSourceSha256"),
        "08964909fa4b7f26ac12c451cfe3a40e4c1ec6cf7ecc2524a2fa11b959175679"
    );
    assert_eq!(
        field_value(summary, "emittedBodySha256"),
        "18e6431ad73d05f8a72eb1f8e82b8ab047279e2cdc54d0545d7acf3e6bab0899"
    );
    assert_eq!(
        sha256_hex(oracle.as_bytes()),
        "902478763d2897eb0d3f031a0895bee7d91a5a7bf8acf8188bf752273e149f14"
    );
    assert_eq!(oracle.len(), 3_684_480);
    assert_eq!(oracle.lines().count(), 15_279);

    let mut totals = Totals::default();
    for image in PAGES {
        let grid = recognize_grid_lines(repo_path(&format!("data/examples/{image}")))
            .unwrap_or_else(|error| panic!("{image}: GRID failed: {error}"));
        let headers = recognize_native_headers(&grid)
            .unwrap_or_else(|error| panic!("{image}: HEADERS failed: {error}"));
        let stem_seeds = recognize_native_stem_seeds(&grid, &headers)
            .unwrap_or_else(|error| panic!("{image}: STEM_SEEDS failed: {error}"));
        let beams =
            recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
                .unwrap_or_else(|error| panic!("{image}: BEAMS failed: {error}"));
        let ledgers = recognize_native_ledgers(&grid, &beams)
            .unwrap_or_else(|error| panic!("{image}: LEDGERS failed: {error}"));
        let heads = recognize_native_heads(&grid, &headers, &stem_seeds, &beams, &ledgers)
            .unwrap_or_else(|error| panic!("{image}: HEADS failed: {error}"));
        let corners = materialize_native_stems_head_corners(&heads, &stem_seeds)
            .unwrap_or_else(|error| panic!("{image}: STEMS corners failed: {error}"));
        let head_seeds = materialize_native_stems_head_seeds(&grid, &stem_seeds, &corners)
            .unwrap_or_else(|error| panic!("{image}: STEMS head seeds failed: {error}"));
        let stumps =
            materialize_native_stems_beam_stumps(&grid, &beams, &heads, &stem_seeds, &head_seeds)
                .unwrap_or_else(|error| panic!("{image}: STEMS beam stumps failed: {error}"));

        let page = format!("{image}#1");
        let actual = native_projected_rows(&page, &grid, &beams, &stem_seeds, &head_seeds, &stumps);
        let expected = oracle_projected_rows(&oracle, &page);
        if actual != expected {
            report_first_mismatches(&page, &actual, &expected);
            panic!("{page}: projected native beam-stump evidence differs from Java");
        }
        include_totals(&mut totals, &stumps);
    }

    assert_eq!(
        totals,
        Totals {
            systems: 30,
            constructors: 803,
            sides: 1_606,
            neighbors: 3_934,
            seed_inputs: 1_820,
            purge_comparisons: 1_087,
            purge_removals: 5,
            purge_breaks: 1_082,
            side_seeds: 1_305,
            build_attempts: 301,
            empty_sections: 4,
            zero_compounds: 154,
            candidates: 143,
            direction_accepted: 6,
            direction_rejected: 137,
            registrations: 5,
            new_builds: 5,
            reused_builds: 1,
            section_rows: 447,
            steps: 447,
            final_stumps: 1_821,
            final_side_stumps: 1_311,
            tremolos: 0,
        }
    );
}

fn native_projected_rows(
    page: &str,
    grid: &audiveris_omr::recognize::GridLinesRecognition,
    source_beams: &audiveris_omr::recognize::NativeBeamRecognition,
    stem_seeds: &audiveris_omr::native_stem_seeds::NativeStemSeedRecognition,
    head_seeds: &audiveris_omr::native_stems_head_seeds::NativeStemsHeadSeedRecognition,
    stumps: &NativeStemsBeamStumpRecognition,
) -> Vec<String> {
    let mut rows = Vec::new();
    for system in &stumps.systems {
        let system_id = system.system_id;
        let source_seed_system = stem_seeds
            .systems
            .iter()
            .find(|candidate| candidate.raw.system_id == system_id)
            .expect("source seed system");
        let purge_system = head_seeds
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system_id)
            .expect("purged seed system");
        let bounds = system.system_bounds;
        rows.push(format!(
            "stemsbeamstumpsystem {page} system {system_id} profile {} interline {} \
             stemThickness {} maxStemThickness {} bounds {}:{}:{}:{} sourceSeeds {} keptSeeds {} \
             verticalSections {} beams {} maxBeamSeedDx {} maxBeamSeedDyRatio {} \
             minBeamStemsDx {} minBeamStumpDy {}",
            system.profile,
            system.interline,
            stem_seeds.main_stem_thickness,
            system.maximum_stem_thickness,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            source_seed_system.free_glyphs.len(),
            purge_system.kept_seed_ordinals.len(),
            system.vertical_section_source_ordinals.len(),
            system.beams_by_abscissa.len(),
            java_hex_double(f64::from(system.max_beam_seed_dx)),
            java_hex_double(0.25),
            system.min_beam_stems_dx,
            system.min_beam_stump_dy,
        ));

        let kept_ordinals = purge_system
            .kept_seed_ordinals
            .iter()
            .enumerate()
            .map(|(kept, &free)| (free, kept))
            .collect::<HashMap<_, _>>();
        let mut aliases = HashMap::<usize, usize>::new();
        let mut boundary_registrations = HashMap::<usize, usize>::new();
        let mut registration_ordinal = 0_usize;
        for beam in &system.beams_by_abscissa {
            let new_count = beam
                .sides
                .iter()
                .filter_map(|side| side.build.as_ref())
                .filter(|build| {
                    matches!(
                        build.registration,
                        Some(NativeStemsBeamRegistration::New { .. })
                    )
                })
                .count();
            rows.push(format!(
                "stemsbeamstumpbeam {page} system {system_id} ordinal {} sigOrdinal {} shape {} \
                 bounds {} median {} height {} beamProfile {} effectiveProfile {} yGapPixels {} \
                 seedDy {} seedMedian {} seedHeight {} groupMembers {} neighbors {} seedAreaBounds {} \
                 registrations {new_count} width {} tremoloWidth {} stumps {} sideStumps {} \
                 looksLikeTremolo {}",
                beam.x_ordinal,
                beam.sig_ordinal,
                shape(beam.kind),
                rectangle(beam.bounds),
                line(beam.median),
                java_hex_double(beam.height),
                beam.beam_profile,
                beam.effective_profile,
                beam.seed_y_gap,
                java_hex_double(beam.seed_area_dy),
                line(beam.seed_area.median),
                java_hex_double(beam.seed_area.height),
                live_group_size(system, source_beams, beam),
                beam.neighbor_seed_ordinals.len(),
                area_bounds(beam.seed_area),
                java_hex_double(beam.median.x2 - beam.median.x1),
                ((beam.median.x2 - beam.median.x1)
                    - (f64::from(system.interline) * 1.35))
                    .abs()
                    <= f64::from(system.interline) * 0.25,
                beam.stumps.len(),
                beam.sides
                    .iter()
                    .filter(|side| side.final_stump.is_some())
                    .count(),
                beam.looks_like_tremolo,
            ));
            for (ordinal, &free) in beam.neighbor_seed_ordinals.iter().enumerate() {
                let glyph = &source_seed_system.free_glyphs[free];
                let center = run_table_center_line(
                    &glyph.run_table,
                    glyph.bounds.x as i32,
                    glyph.bounds.y as i32,
                )
                .expect("seed center line");
                rows.push(format!(
                    "stemsbeamstumpneighbor {page} system {system_id} beam {} ordinal {ordinal} \
                     keptOrdinal {} bounds {} weight {} centerLine {}",
                    beam.x_ordinal,
                    kept_ordinals[&free],
                    bounds_string(glyph.bounds),
                    glyph.weight,
                    line(center),
                ));
            }
            for seed in &beam.intersected_seeds {
                rows.push(format!(
                    "stemsbeamstumpseed {page} system {system_id} beam {} sortOrdinal {} \
                     inputOrdinal {} keptOrdinal {} bounds {} crossX {} centerDistanceSq {}",
                    beam.x_ordinal,
                    seed.sorted_ordinal,
                    seed.pre_sort_ordinal,
                    kept_ordinals[&seed.free_glyph_ordinal],
                    rectangle(seed.bounds),
                    java_hex_double(seed.intersection.0),
                    java_hex_double(seed.distance_to_seed_segment_sq),
                ));
            }
            for (ordinal, step) in beam.purge_steps.iter().enumerate() {
                rows.push(format!(
                    "stemsbeamstumppurge {page} system {system_id} beam {} ordinal {ordinal} \
                     i {} j {} leftKept {} rightKept {} x1 {} x2 {} dx {} minDx {} yOverlap {} \
                     leftHeight {} rightHeight {} leftDistanceSq {} rightDistanceSq {} action {} \
                     survivors {}",
                    beam.x_ordinal,
                    step.first_index,
                    step.second_index,
                    kept_ordinals[&step.first_free_glyph_ordinal],
                    kept_ordinals[&step.second_free_glyph_ordinal],
                    java_hex_double(step.first_intersection_x),
                    java_hex_double(step.second_intersection_x),
                    java_hex_double(step.delta_x),
                    system.min_beam_stems_dx,
                    step.vertical_overlap,
                    step.first_height,
                    step.second_height,
                    java_hex_double(step.first_distance_sq),
                    java_hex_double(step.second_distance_sq),
                    purge_action(step.action),
                    kept_list(&step.remaining_seed_ordinals, &kept_ordinals),
                ));
            }
            if !beam.surviving_seed_ordinals.is_empty() {
                for side in &beam.sides {
                    rows.push(format!(
                        "stemsbeamstumpsideclass {page} system {system_id} beam {} side {} \
                         keptOrdinal {} crossX {} portion {} selected {}",
                        beam.x_ordinal,
                        side_name(side.side),
                        kept_ordinals[&side.edge_seed_ordinal.expect("surviving edge seed")],
                        java_hex_double(side.edge_intersection_x.expect("edge x")),
                        portion_name(side.edge_portion.expect("edge portion")),
                        side.classified_seed_ordinal.is_some(),
                    ));
                }
            }

            let mut new_rows = Vec::new();
            let mut phase_ordinal = 0_usize;
            for side in &beam.sides {
                if let Some(free) = side.classified_seed_ordinal {
                    let handle = stump_handle(side.final_stump.as_ref().expect("seed stump"));
                    let alias = canonical_alias(&mut aliases, handle);
                    rows.push(format!(
                        "stemsbeamstumpside {page} system {system_id} beam {} side {} mode seed \
                         keptOrdinal {} canonicalAlias {alias}",
                        beam.x_ordinal,
                        side_name(side.side),
                        kept_ordinals[&free],
                    ));
                    continue;
                }
                let build = side.build.as_ref().expect("missing side build");
                let registration = match (&build.registration, build.canonical_glyph_index) {
                    (Some(NativeStemsBeamRegistration::New { .. }), Some(handle)) => {
                        let ordinal = registration_ordinal;
                        registration_ordinal += 1;
                        boundary_registrations.insert(handle, ordinal);
                        let candidate = build.candidate.as_ref().expect("new candidate");
                        new_rows.push((ordinal, phase_ordinal, candidate));
                        phase_ordinal += 1;
                        format!("new:reg:{ordinal}")
                    }
                    (Some(NativeStemsBeamRegistration::Reused { .. }), Some(handle)) => {
                        boundary_registrations.get(&handle).map_or_else(
                            || "reuse:pre".to_owned(),
                            |ordinal| format!("reuse:reg:{ordinal}"),
                        )
                    }
                    (None, _) => "none".to_owned(),
                    _ => panic!("registration without canonical handle"),
                };
                let canonical = build
                    .canonical_glyph_index
                    .map(|handle| canonical_alias(&mut aliases, handle).to_string())
                    .unwrap_or_else(|| "-".to_owned());
                rows.push(format!(
                    "stemsbeamstumpbuild {page} system {system_id} beam {} side {} areaBounds {} \
                     refX {} stumpMedian {} stumpHeight {} sections {} steps {} compoundWeight {} compoundBounds {} candidate {} \
                     directions {} registration {registration} canonicalAlias {canonical}",
                    beam.x_ordinal,
                    side_name(side.side),
                    area_bounds(build.area),
                    java_hex_double(build.reference_x),
                    line(build.area.median),
                    java_hex_double(build.area.height),
                    build.sections.len(),
                    build.steps.len(),
                    build.compound_weight,
                    optional_bounds(build.compound_bounds),
                    build.candidate.as_ref().map_or_else(
                        || "none".to_owned(),
                        |candidate| format!(
                            "{}:{}:{}:{:016x}",
                            bounds_string(candidate.bounds),
                            candidate.weight,
                            candidate.run_count(),
                            candidate.run_digest(),
                        ),
                    ),
                    build.directions.as_ref().map_or("-", |evidence| {
                        direction_token(evidence.directions.as_deref())
                    }),
                ));
                for section in &build.sections {
                    let source = source_section(grid, system, section.source_ordinal);
                    rows.push(format!(
                        "stemsbeamstumpsection {page} system {system_id} beam {} side {} \
                         sortOrdinal {} inputOrdinal {} sourceOrdinal {} bounds {} weight {} \
                         firstPos {} runs {}:{:016x} areaCenter {}:{} distance {}",
                        beam.x_ordinal,
                        side_name(side.side),
                        section.sorted_ordinal,
                        section.pre_sort_ordinal,
                        section.source_ordinal,
                        bounds_string(section.bounds),
                        section.weight,
                        section.first_pos,
                        section.run_count,
                        section_run_hash(source),
                        section.area_center.0,
                        section.area_center.1,
                        java_hex_double(section.distance),
                    ));
                }
                for step in &build.steps {
                    rows.push(format!(
                        "stemsbeamstumpstep {page} system {system_id} beam {} side {} sortOrdinal {} \
                         sourceOrdinal {} afterAddWidth {} tooWide {} finalWeight {} finalBounds {} \
                         members {}",
                        beam.x_ordinal,
                        side_name(side.side),
                        step.sorted_ordinal,
                        step.source_ordinal,
                        step.after_add_width,
                        step.too_wide,
                        step.final_weight,
                        optional_bounds(step.final_bounds),
                        ordinal_list(&step.member_source_ordinals),
                    ));
                }
                if let Some(evidence) = &build.directions {
                    let source_to_sig = system
                        .beams_by_abscissa
                        .iter()
                        .map(|candidate| (candidate.source, candidate.sig_ordinal))
                        .collect::<BTreeMap<_, _>>();
                    let self_glyph_first = evidence.beam_glyph_is_top_extreme;
                    let self_glyph_last = evidence.beam_glyph_is_bottom_extreme;
                    let internal = evidence.directions.is_none() && !evidence.siblings.is_empty();
                    let x = evidence.stump_center.0;
                    rows.push(format!(
                        "stemsbeamstumpdirections {page} system {system_id} beam {} side {} \
                         siblings {} firstSig {} lastSig {} selfGlyphFirst {self_glyph_first} \
                         selfGlyphLast {self_glyph_last} internal {internal} x {} centerLine {} \
                         topBorderY {} bottomBorderY {} dyTop {} dyBottom {} minDy {} directions {}",
                        beam.x_ordinal,
                        side_name(side.side),
                        string_list(
                            evidence
                                .siblings
                                .iter()
                                .map(|sibling| source_to_sig[&sibling.source].to_string()),
                        ),
                        evidence
                            .top_extreme
                            .map_or_else(|| "-".to_owned(), |source| source_to_sig[&source].to_string()),
                        evidence.bottom_extreme.map_or_else(
                            || "-".to_owned(),
                            |source| source_to_sig[&source].to_string(),
                        ),
                        java_hex_double(x),
                        line(evidence.stump_center_line),
                        optional_double(evidence.top_border_y),
                        optional_double(evidence.bottom_border_y),
                        optional_double(evidence.top_dy),
                        optional_double(evidence.bottom_dy),
                        system.min_beam_stump_dy,
                        direction_token(evidence.directions.as_deref()),
                    ));
                }
            }
            for (ordinal, phase, candidate) in new_rows {
                rows.push(format!(
                    "stemsbeamstumpreg {page} system {system_id} beam {} ordinal {ordinal} \
                     phase beam:{} phaseOrdinal {phase} bounds {} weight {} runs {}:{:016x}",
                    beam.x_ordinal,
                    beam.x_ordinal,
                    bounds_string(candidate.bounds),
                    candidate.weight,
                    candidate.run_count(),
                    candidate.run_digest(),
                ));
            }
            for stump in &beam.stumps {
                let handle = stump_handle(&stump.reference);
                let (glyph_bounds, glyph_weight, glyph_run_count, glyph_run_digest) =
                    stump_descriptor(&stump.reference, source_seed_system, system);
                rows.push(format!(
                    "stemsbeamstumpfinal {page} system {system_id} beam {} ordinal {} origin {} \
                     canonicalAlias {} bounds {} weight {} runs {}:{:016x}",
                    beam.x_ordinal,
                    stump.list_ordinal,
                    stump_origin(&stump.reference, &kept_ordinals, &boundary_registrations),
                    canonical_alias(&mut aliases, handle),
                    bounds_string(glyph_bounds),
                    glyph_weight,
                    glyph_run_count,
                    glyph_run_digest,
                ));
            }
            for side in &beam.sides {
                let (origin, alias) = side.final_stump.as_ref().map_or_else(
                    || ("-".to_owned(), "-".to_owned()),
                    |reference| {
                        let handle = stump_handle(reference);
                        (
                            stump_origin(reference, &kept_ordinals, &boundary_registrations),
                            canonical_alias(&mut aliases, handle).to_string(),
                        )
                    },
                );
                rows.push(format!(
                    "stemsbeamstumpfinalside {page} system {system_id} beam {} side {} origin \
                     {origin} canonicalAlias {alias}",
                    beam.x_ordinal,
                    side_name(side.side),
                ));
            }
        }
    }
    rows
}

fn oracle_projected_rows(oracle: &str, page: &str) -> Vec<String> {
    oracle
        .lines()
        .filter(|line| line.split_whitespace().nth(1) == Some(page))
        .filter_map(project_oracle_row)
        .collect()
}

fn project_oracle_row(line: &str) -> Option<String> {
    let words = line.split_whitespace().collect::<Vec<_>>();
    let prefix = words.first().copied()?;
    match prefix {
        "stemsbeamstumpsystem" => Some(format!(
            "{prefix} {} system {} profile {} interline {} stemThickness {} maxStemThickness {} \
             bounds {} sourceSeeds {} keptSeeds {} verticalSections {} beams {} maxBeamSeedDx {} \
             maxBeamSeedDyRatio {} minBeamStemsDx {} minBeamStumpDy {}",
            words[1],
            field(&words, "system"),
            field(&words, "profile"),
            field(&words, "interline"),
            field(&words, "stemThickness"),
            field(&words, "maxStemThickness"),
            field(&words, "bounds"),
            field(&words, "sourceSeeds"),
            field(&words, "keptSeeds"),
            field(&words, "verticalSections"),
            field(&words, "beams"),
            field(&words, "maxBeamSeedDx"),
            field(&words, "maxBeamSeedDyRatio"),
            field(&words, "minBeamStemsDx"),
            field(&words, "minBeamStumpDy"),
        )),
        "stemsbeamstumppurge" => Some(format!(
            "{prefix} {} system {} beam {} ordinal {} i {} j {} leftKept {} rightKept {} x1 {} x2 {} \
             dx {} minDx {} yOverlap {} leftHeight {} rightHeight {} leftDistanceSq {} \
             rightDistanceSq {} action {} survivors {}",
            words[1],
            field(&words, "system"),
            field(&words, "beam"),
            field(&words, "ordinal"),
            field(&words, "i"),
            field(&words, "j"),
            field(&words, "leftKept"),
            field(&words, "rightKept"),
            field(&words, "x1"),
            field(&words, "x2"),
            field(&words, "dx"),
            field(&words, "minDx"),
            field(&words, "yOverlap"),
            field(&words, "leftHeight"),
            field(&words, "rightHeight"),
            field(&words, "leftDistanceSq"),
            field(&words, "rightDistanceSq"),
            field(&words, "action"),
            field(&words, "survivors"),
        )),
        "stemsbeamstumpbuild" => {
            let registration = field(&words, "registration");
            let registration = if registration.starts_with("reuse:pre:") {
                "reuse:pre"
            } else {
                registration
            };
            Some(replace_field(line, "registration", registration))
        }
        "stemsbeamstumpfinal" | "stemsbeamstumpfinalside" => {
            let origin = field(&words, "origin");
            let origin = if origin.starts_with("pre:") {
                "pre"
            } else {
                origin
            };
            Some(replace_field(line, "origin", origin))
        }
        "stemsbeamstumpbeam"
        | "stemsbeamstumpneighbor"
        | "stemsbeamstumpseed"
        | "stemsbeamstumpsideclass"
        | "stemsbeamstumpside"
        | "stemsbeamstumpsection"
        | "stemsbeamstumpstep"
        | "stemsbeamstumpdirections"
        | "stemsbeamstumpreg" => Some(line.to_owned()),
        _ => None,
    }
}

fn include_totals(totals: &mut Totals, recognition: &NativeStemsBeamStumpRecognition) {
    totals.systems += recognition.systems.len();
    for beam in recognition
        .systems
        .iter()
        .flat_map(|system| &system.beams_by_abscissa)
    {
        totals.constructors += 1;
        totals.sides += beam.sides.len();
        totals.neighbors += beam.neighbor_seed_ordinals.len();
        totals.seed_inputs += beam.intersected_seeds.len();
        totals.purge_comparisons += beam.purge_steps.len();
        totals.purge_breaks += beam
            .purge_steps
            .iter()
            .filter(|step| step.action == NativeStemsBeamSeedPurgeAction::BreakAtMinimumDx)
            .count();
        totals.purge_removals += beam
            .purge_steps
            .iter()
            .filter(|step| step.action != NativeStemsBeamSeedPurgeAction::BreakAtMinimumDx)
            .count();
        totals.side_seeds += beam
            .sides
            .iter()
            .filter(|side| side.classified_seed_ordinal.is_some())
            .count();
        totals.final_stumps += beam.stumps.len();
        totals.final_side_stumps += beam
            .sides
            .iter()
            .filter(|side| side.final_stump.is_some())
            .count();
        totals.tremolos += usize::from(beam.looks_like_tremolo);
        for build in beam.sides.iter().filter_map(|side| side.build.as_ref()) {
            totals.build_attempts += 1;
            totals.section_rows += build.sections.len();
            totals.steps += build.steps.len();
            if build.sections.is_empty() {
                totals.empty_sections += 1;
            } else if build.compound_weight == 0 {
                totals.zero_compounds += 1;
            }
            if build.candidate.is_some() {
                totals.candidates += 1;
                match &build.registration {
                    Some(NativeStemsBeamRegistration::New { .. }) => {
                        totals.direction_accepted += 1;
                        totals.registrations += 1;
                        totals.new_builds += 1;
                    }
                    Some(NativeStemsBeamRegistration::Reused { .. }) => {
                        totals.direction_accepted += 1;
                        totals.reused_builds += 1;
                    }
                    None => totals.direction_rejected += 1,
                }
            }
        }
    }
}

fn live_group_size(
    system: &audiveris_omr::native_stems_beam_stumps::NativeStemsBeamStumpSystem,
    beams: &audiveris_omr::recognize::NativeBeamRecognition,
    beam: &NativeStemsBeamStumpBeam,
) -> usize {
    let mut members = beams
        .raw_beams
        .iter()
        .enumerate()
        .filter_map(|(ordinal, (owner, _))| {
            (*owner == system.system_id).then_some(NativeStemsBeamSource::RawBeam(ordinal))
        })
        .chain(
            beams
                .hooks
                .iter()
                .enumerate()
                .filter_map(|(ordinal, (owner, _))| {
                    (*owner == system.system_id).then_some(NativeStemsBeamSource::Hook(ordinal))
                }),
        );
    let member_pool = members.by_ref().collect::<Vec<_>>();
    let live = system
        .beams_by_abscissa
        .iter()
        .map(|beam| beam.source)
        .collect::<Vec<_>>();
    beams
        .group_memberships
        .iter()
        .find(|groups| groups.system_id == system.system_id)
        .expect("beam groups")
        .groups[beam.group_ordinal]
        .iter()
        .filter_map(|&ordinal| member_pool.get(ordinal))
        .filter(|source| live.contains(source))
        .count()
}

fn stump_descriptor(
    reference: &NativeStemsBeamStumpRef,
    seeds: &audiveris_omr::native_stem_seeds::NativeStemSeedSystemRecognition,
    system: &audiveris_omr::native_stems_beam_stumps::NativeStemsBeamStumpSystem,
) -> (Bounds, usize, usize, u64) {
    match reference {
        NativeStemsBeamStumpRef::Seed {
            free_glyph_ordinal, ..
        } => {
            let seed = &seeds.free_glyphs[*free_glyph_ordinal];
            (
                seed.bounds,
                seed.weight,
                seed.run_count(),
                seed.run_digest(),
            )
        }
        NativeStemsBeamStumpRef::Built {
            canonical_glyph_index,
        } => {
            let glyph = system
                .beams_by_abscissa
                .iter()
                .flat_map(|beam| &beam.sides)
                .filter_map(|side| side.build.as_ref())
                .find(|build| build.canonical_glyph_index == Some(*canonical_glyph_index))
                .and_then(|build| build.candidate.as_ref())
                .expect("built stump glyph");
            (
                glyph.bounds,
                glyph.weight,
                glyph.run_count(),
                glyph.run_digest(),
            )
        }
    }
}

fn stump_handle(reference: &NativeStemsBeamStumpRef) -> usize {
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

fn stump_origin(
    reference: &NativeStemsBeamStumpRef,
    kept: &HashMap<usize, usize>,
    registrations: &HashMap<usize, usize>,
) -> String {
    match reference {
        NativeStemsBeamStumpRef::Seed {
            free_glyph_ordinal, ..
        } => format!("kept:{}", kept[free_glyph_ordinal]),
        NativeStemsBeamStumpRef::Built {
            canonical_glyph_index,
        } => registrations
            .get(canonical_glyph_index)
            .map_or_else(|| "pre".to_owned(), |ordinal| format!("reg:{ordinal}")),
    }
}

fn source_section<'a>(
    grid: &'a audiveris_omr::recognize::GridLinesRecognition,
    system: &audiveris_omr::native_stems_beam_stumps::NativeStemsBeamStumpSystem,
    source_ordinal: usize,
) -> &'a Section {
    &grid.peak_graph.vertical_sections[system.vertical_section_source_ordinals[source_ordinal]]
}

fn shape(kind: BeamKind) -> &'static str {
    match kind {
        BeamKind::Beam => "BEAM",
        BeamKind::Hook => "BEAM_HOOK",
        BeamKind::SmallBeam => "BEAM_SMALL",
    }
}

fn side_name(side: NativeStemHeadSide) -> &'static str {
    match side {
        NativeStemHeadSide::Left => "LEFT",
        NativeStemHeadSide::Right => "RIGHT",
    }
}

fn portion_name(portion: NativeBeamPortion) -> &'static str {
    match portion {
        NativeBeamPortion::Left => "LEFT",
        NativeBeamPortion::Center => "CENTER",
        NativeBeamPortion::Right => "RIGHT",
    }
}

fn purge_action(action: NativeStemsBeamSeedPurgeAction) -> &'static str {
    match action {
        NativeStemsBeamSeedPurgeAction::BreakAtMinimumDx => "break",
        NativeStemsBeamSeedPurgeAction::RemoveFirstForHeight => "removeLeftOverlap",
        NativeStemsBeamSeedPurgeAction::RemoveSecondForHeight => "removeRightOverlap",
        NativeStemsBeamSeedPurgeAction::RemoveFirstForDistance => "removeLeftDistance",
        NativeStemsBeamSeedPurgeAction::RemoveSecondForDistance => "removeRightDistance",
    }
}

fn direction_token(directions: Option<&[NativeStemVerticalSide]>) -> &'static str {
    match directions {
        None => "null",
        Some([]) => "none",
        Some([NativeStemVerticalSide::Top]) => "TOP",
        Some([NativeStemVerticalSide::Bottom]) => "BOTTOM",
        Some([NativeStemVerticalSide::Top, NativeStemVerticalSide::Bottom]) => "TOP,BOTTOM",
        Some(_) => panic!("invalid beam stump direction order"),
    }
}

fn canonical_alias(aliases: &mut HashMap<usize, usize>, handle: usize) -> usize {
    let next = aliases.len();
    *aliases.entry(handle).or_insert(next)
}

fn kept_list(free: &[usize], kept: &HashMap<usize, usize>) -> String {
    if free.is_empty() {
        "-".to_owned()
    } else {
        free.iter()
            .map(|ordinal| kept[ordinal].to_string())
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn ordinal_list(ordinals: &[usize]) -> String {
    if ordinals.is_empty() {
        "-".to_owned()
    } else {
        ordinals
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn string_list(values: impl IntoIterator<Item = String>) -> String {
    let values = values.into_iter().collect::<Vec<_>>();
    if values.is_empty() {
        "-".to_owned()
    } else {
        values.join(",")
    }
}

fn rectangle(rectangle: audiveris_omr::head_scanner_slices::JavaRectangle) -> String {
    format!(
        "{}:{}:{}:{}",
        rectangle.x, rectangle.y, rectangle.width, rectangle.height
    )
}

fn bounds_string(bounds: Bounds) -> String {
    format!(
        "{}:{}:{}:{}",
        bounds.x, bounds.y, bounds.width, bounds.height
    )
}

fn optional_bounds(bounds: Option<Bounds>) -> String {
    bounds.map_or_else(|| "-".to_owned(), bounds_string)
}

fn line(line: Segment) -> String {
    format!(
        "{}:{}:{}:{}",
        java_hex_double(line.x1),
        java_hex_double(line.y1),
        java_hex_double(line.x2),
        java_hex_double(line.y2),
    )
}

fn area_bounds(area: NativeStemsBeamArea) -> String {
    let half = area.height / 2.0;
    let left = area.median.x1.min(area.median.x2);
    let right = area.median.x1.max(area.median.x2);
    let top = (area.median.y1 - half).min(area.median.y2 - half);
    let bottom = (area.median.y1 + half).max(area.median.y2 + half);
    format!(
        "{}:{}:{}:{}",
        java_hex_double(left),
        java_hex_double(top),
        java_hex_double(right - left),
        java_hex_double(bottom - top),
    )
}

fn optional_double(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_owned(), java_hex_double)
}

fn section_run_hash(section: &Section) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let orientation = match section.orientation() {
        Orientation::Horizontal => "HORIZONTAL",
        Orientation::Vertical => "VERTICAL",
    };
    hash_row(&mut hash, &format!("{orientation} {}", section.first_pos()));
    for run in section.runs() {
        hash_row(&mut hash, &format!("{}:{}", run.start, run.length));
    }
    hash
}

fn hash_row(hash: &mut u64, row: &str) {
    for byte in row.bytes().chain([b'\n']) {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn field<'a>(words: &'a [&str], name: &str) -> &'a str {
    let index = words
        .iter()
        .position(|&word| word == name)
        .unwrap_or_else(|| panic!("missing {name} in {words:?}"));
    words[index + 1]
}

fn field_value<'a>(row: &'a str, name: &str) -> &'a str {
    let mut words = row.split_whitespace();
    while let Some(word) = words.next() {
        if word == name {
            return words
                .next()
                .unwrap_or_else(|| panic!("missing value for {name}"));
        }
    }
    panic!("missing {name} in {row}")
}

fn replace_field(line: &str, name: &str, replacement: &str) -> String {
    let mut words = line
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let index = words
        .iter()
        .position(|word| word == name)
        .unwrap_or_else(|| panic!("missing {name} in {line}"));
    words[index + 1] = replacement.to_owned();
    words.join(" ")
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

fn report_first_mismatches(page: &str, actual: &[String], expected: &[String]) {
    let count = actual.len().max(expected.len());
    let mut shown = 0;
    for index in 0..count {
        let actual = actual.get(index).map(String::as_str).unwrap_or("<missing>");
        let expected = expected
            .get(index)
            .map(String::as_str)
            .unwrap_or("<missing>");
        if actual != expected {
            eprintln!("{page} row {index}\n  actual:   {actual}\n  expected: {expected}");
            shown += 1;
            if shown == 8 {
                break;
            }
        }
    }
}

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}
