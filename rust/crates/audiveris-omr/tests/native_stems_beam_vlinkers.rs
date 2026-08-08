// SPDX-License-Identifier: AGPL-3.0-or-later

//! Eight-page differential gate for constructor-time STEMS beam VLinkers.

use std::{collections::HashMap, path::PathBuf};

use audiveris_image::{
    beam_structure::Segment,
    run_table::{Orientation, RunTable},
    section::Bounds,
};

use audiveris_omr::{
    beam_inters::{BeamKind, RawBeam},
    head_scanner_slices::JavaRectangle,
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads,
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::{
        NativeStemSeedRecognition, NativeStemSeedSystemRecognition, recognize_native_stem_seeds,
    },
    native_stems_beam_stumps::{
        NativeStemsBeamGlyph, NativeStemsBeamSource, NativeStemsBeamStumpBeam,
        NativeStemsBeamStumpRecognition, NativeStemsBeamStumpRef, NativeStemsBeamStumpSystem,
        materialize_native_stems_beam_stumps,
    },
    native_stems_beam_vlinkers::{
        NativeStemsBeamAlienAction, NativeStemsBeamBLinker, NativeStemsBeamBLinkerOrigin,
        NativeStemsBeamDoubleBounds, NativeStemsBeamLuGeometry, NativeStemsBeamOrphanDecision,
        NativeStemsBeamOrphanOutcome, NativeStemsBeamVLinker, NativeStemsBeamVLinkerConstructor,
        NativeStemsBeamVLinkerSystem, materialize_native_stems_beam_vlinkers,
    },
    native_stems_head_corners::materialize_native_stems_head_corners,
    native_stems_head_seeds::{
        NativeStemsHeadSeedRecognition, NativeStemsHeadSeedSystem,
        materialize_native_stems_head_seeds,
    },
    recognize::{
        NativeBeamRecognition, recognize_grid_lines, recognize_native_beams_with_stem_seeds,
    },
    stems_step::{NativeStemHeadSide, NativeStemLine, NativeStemPoint, NativeStemVerticalSide},
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
const ORACLE_PATH: &str = "rust/oracle/stems-beam-vlinkers.txt";
const PROBE_PATH: &str = "rust/oracle/java/StemsBeamVLinkerProbe.java";
const RUNNER_PATH: &str = "rust/oracle/java/run-stems-beam-vlinkers.sh";

// Filled once the invariant-clean corpus fixture is checked in. Keeping the
// assertions wired now makes freezing these values a data-only follow-up.
const EXPECTED_FIXTURE_SHA256: Option<&str> =
    Some("77cfa1f1d9b6e3f8917ff44db7e3f643ffca690bd639d8a5a93f6fea208a8388");
const EXPECTED_PROBE_SHA256: Option<&str> =
    Some("fbc5dace791c84e82db5ff870fb4bcc23e06f29b54619865f19448c0f016a5c2");
const EXPECTED_RUNNER_SHA256: Option<&str> =
    Some("38e723c15bec6d67c4b856fc40a40d3ee0e4835f466c0c917715c792e6fa1c75");
const EXPECTED_BODY_SHA256: Option<&str> =
    Some("bd43baa197540107e27d2ac97098dbb9df6d6bea1003888ee3625c69e21e60bf");
const EXPECTED_FIXTURE_BYTES: Option<usize> = Some(18_307_148);
const EXPECTED_FIXTURE_LINES: Option<usize> = Some(46_946);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Totals {
    constructors: usize,
    live_beams: usize,
    tremolos: usize,
    parts: usize,
    stump_b: usize,
    orphan_b: usize,
    zero_direction_b: usize,
    b_linkers: usize,
    v_linkers: usize,
    stump_v: usize,
    orphan_v: usize,
    top_v: usize,
    bottom_v: usize,
    side_b_links: usize,
    orphan_checks: usize,
    orphan_existing: usize,
    orphan_created: usize,
    orphan_empty: usize,
    orphan_interior: usize,
    limit_staff_rows: usize,
    geometries: usize,
    alien_lookups: usize,
    alien_candidates: usize,
    alien_group_drops: usize,
    alien_bad_drops: usize,
    alien_hook_drops: usize,
    alien_miss_drops: usize,
    alien_aligned_drops: usize,
    alien_accepted: usize,
    alien_sort_rows: usize,
    alien_shrinks: usize,
    seed_candidates: usize,
    seed_hits: usize,
}

impl Totals {
    fn include(&mut self, other: Self) {
        self.constructors += other.constructors;
        self.live_beams += other.live_beams;
        self.tremolos += other.tremolos;
        self.parts += other.parts;
        self.stump_b += other.stump_b;
        self.orphan_b += other.orphan_b;
        self.zero_direction_b += other.zero_direction_b;
        self.b_linkers += other.b_linkers;
        self.v_linkers += other.v_linkers;
        self.stump_v += other.stump_v;
        self.orphan_v += other.orphan_v;
        self.top_v += other.top_v;
        self.bottom_v += other.bottom_v;
        self.side_b_links += other.side_b_links;
        self.orphan_checks += other.orphan_checks;
        self.orphan_existing += other.orphan_existing;
        self.orphan_created += other.orphan_created;
        self.orphan_empty += other.orphan_empty;
        self.orphan_interior += other.orphan_interior;
        self.limit_staff_rows += other.limit_staff_rows;
        self.geometries += other.geometries;
        self.alien_lookups += other.alien_lookups;
        self.alien_candidates += other.alien_candidates;
        self.alien_group_drops += other.alien_group_drops;
        self.alien_bad_drops += other.alien_bad_drops;
        self.alien_hook_drops += other.alien_hook_drops;
        self.alien_miss_drops += other.alien_miss_drops;
        self.alien_aligned_drops += other.alien_aligned_drops;
        self.alien_accepted += other.alien_accepted;
        self.alien_sort_rows += other.alien_sort_rows;
        self.alien_shrinks += other.alien_shrinks;
        self.seed_candidates += other.seed_candidates;
        self.seed_hits += other.seed_hits;
    }
}

#[derive(Clone, Copy)]
struct GlyphDescriptor<'a> {
    bounds: JavaRectangle,
    weight: usize,
    run_table: &'a RunTable,
}

#[derive(Default)]
struct Aliases {
    group_ordinals: Vec<usize>,
    beam_glyphs: Vec<NativeStemsBeamGlyph>,
    built_stumps: HashMap<usize, usize>,
}

impl Aliases {
    fn preassign(system: &NativeStemsBeamStumpSystem) -> Self {
        let mut aliases = Self::default();
        for beam in &system.beams_by_abscissa {
            if !aliases.group_ordinals.contains(&beam.group_ordinal) {
                aliases.group_ordinals.push(beam.group_ordinal);
            }
            if !aliases.beam_glyphs.contains(&beam.beam_glyph) {
                aliases.beam_glyphs.push(beam.beam_glyph.clone());
            }
        }
        aliases
    }

    fn group(&self, group_ordinal: usize) -> usize {
        self.group_ordinals
            .iter()
            .position(|&candidate| candidate == group_ordinal)
            .expect("preassigned beam group")
    }

    fn beam_glyph(&self, glyph: &NativeStemsBeamGlyph) -> usize {
        self.beam_glyphs
            .iter()
            .position(|candidate| candidate == glyph)
            .expect("preassigned beam glyph")
    }

    fn stump(&mut self, stump: &NativeStemsBeamStumpRef) -> String {
        match stump {
            NativeStemsBeamStumpRef::Seed { kept_ordinal, .. } => {
                format!("kept:{kept_ordinal}")
            }
            NativeStemsBeamStumpRef::Built {
                canonical_glyph_index,
            } => {
                let next = self.built_stumps.len();
                let alias = *self
                    .built_stumps
                    .entry(*canonical_glyph_index)
                    .or_insert(next);
                format!("built:{alias}")
            }
        }
    }
}

#[derive(Clone, Copy)]
struct RowHasher(u64);

impl Default for RowHasher {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl RowHasher {
    fn add(&mut self, row: &str) {
        for byte in row.bytes().chain([b'\n']) {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

#[test]
fn native_stems_beam_vlinkers_match_java_corpus_exactly() {
    let oracle = std::fs::read_to_string(repo_path(ORACLE_PATH)).expect("beam-VLinker oracle");
    assert_corpus_summary(&oracle);

    let mut corpus_totals = Totals::default();
    for image in PAGES {
        let page = format!("{image}#1");
        let (actual, totals) = native_page_rows(image, &page);
        let expected = oracle_projected_rows(&oracle, &page);
        if actual != expected {
            report_first_mismatches(&page, &actual, &expected);
            panic!("{page}: projected native BeamVLinker evidence differs from Java");
        }
        corpus_totals.include(totals);
    }
    assert_known_totals(corpus_totals);
}

fn native_page_rows(image: &str, page: &str) -> (Vec<String>, Totals) {
    let grid = recognize_grid_lines(repo_path(&format!("data/examples/{image}")))
        .unwrap_or_else(|error| panic!("{page}: GRID failed: {error}"));
    let headers = recognize_native_headers(&grid)
        .unwrap_or_else(|error| panic!("{page}: HEADERS failed: {error}"));
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers)
        .unwrap_or_else(|error| panic!("{page}: STEM_SEEDS failed: {error}"));
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .unwrap_or_else(|error| panic!("{page}: BEAMS failed: {error}"));
    let ledgers = recognize_native_ledgers(&grid, &beams)
        .unwrap_or_else(|error| panic!("{page}: LEDGERS failed: {error}"));
    let heads = recognize_native_heads(&grid, &headers, &stem_seeds, &beams, &ledgers)
        .unwrap_or_else(|error| panic!("{page}: HEADS failed: {error}"));
    let corners = materialize_native_stems_head_corners(&heads, &stem_seeds)
        .unwrap_or_else(|error| panic!("{page}: STEMS corners failed: {error}"));
    let head_seeds = materialize_native_stems_head_seeds(&grid, &stem_seeds, &corners)
        .unwrap_or_else(|error| panic!("{page}: STEMS head seeds failed: {error}"));
    let beam_stumps =
        materialize_native_stems_beam_stumps(&grid, &beams, &heads, &stem_seeds, &head_seeds)
            .unwrap_or_else(|error| panic!("{page}: STEMS beam stumps failed: {error}"));
    let vlinkers = materialize_native_stems_beam_vlinkers(&grid, &beams, &stem_seeds, &beam_stumps)
        .unwrap_or_else(|error| panic!("{page}: STEMS beam VLinkers failed: {error}"));

    let mut rows = vec![format!(
        "stemsbeamvlinkerpage {page} systems {} staves {} family {}",
        vlinkers.systems.len(),
        grid.staves.len(),
        beams.music_font_scale.map_or("Bravura", |_| "Bravura"),
    )];
    let mut page_totals = Totals::default();
    let mut page_hash = RowHasher::default();
    for system in &vlinkers.systems {
        let stump_system = system_by_id(&beam_stumps, system.system_id);
        let seed_system = seed_system_by_id(&stem_seeds, system.system_id);
        let kept_system = kept_system_by_id(&head_seeds, system.system_id);
        let mut system_rows = Vec::new();
        let totals = append_system_rows(
            page,
            &beams,
            &stem_seeds,
            system,
            stump_system,
            seed_system,
            kept_system,
            &mut system_rows,
        );
        let mut system_hash = RowHasher::default();
        for row in &system_rows {
            system_hash.add(row);
            page_hash.add(row);
        }
        let summary = system_summary_row(page, system.system_id, totals, system_hash.0);
        page_hash.add(&summary);
        system_rows.push(summary);
        rows.extend(system_rows);
        page_totals.include(totals);
    }
    rows.push(page_summary_row(
        page,
        vlinkers.systems.len(),
        page_totals,
        page_hash.0,
    ));

    assert_eq!(vlinkers.constructor_count, page_totals.constructors);
    assert_eq!(vlinkers.surviving_beam_count, page_totals.live_beams);
    assert_eq!(vlinkers.b_linker_count, page_totals.b_linkers);
    assert_eq!(vlinkers.v_linker_count, page_totals.v_linkers);
    assert_eq!(vlinkers.stump_b_linker_count, page_totals.stump_b);
    assert_eq!(vlinkers.orphan_b_linker_count, page_totals.orphan_b);
    assert_eq!(vlinkers.alien_candidate_count, page_totals.alien_candidates);
    assert_eq!(vlinkers.alien_limiter_count, page_totals.alien_shrinks);
    assert_eq!(
        vlinkers.neighbor_seed_check_count,
        page_totals.seed_candidates
    );
    assert_eq!(vlinkers.reachable_seed_count, page_totals.seed_hits);
    (rows, page_totals)
}

#[allow(clippy::too_many_arguments)]
fn append_system_rows(
    page: &str,
    beams: &NativeBeamRecognition,
    stem_seeds: &NativeStemSeedRecognition,
    system: &NativeStemsBeamVLinkerSystem,
    stump_system: &NativeStemsBeamStumpSystem,
    seed_system: &NativeStemSeedSystemRecognition,
    kept_system: &NativeStemsHeadSeedSystem,
    rows: &mut Vec<String>,
) -> Totals {
    let mut totals = Totals {
        constructors: system.constructors.len(),
        live_beams: system
            .constructors
            .iter()
            .filter(|constructor| constructor.survives_constructor_loop)
            .count(),
        tremolos: system
            .constructors
            .iter()
            .filter(|constructor| constructor.looks_like_tremolo)
            .count(),
        parts: system.parts.len(),
        ..Totals::default()
    };
    rows.push(format!(
        "stemsbeamvlinkersystem {page} system {} profile {} interline {} stemThickness {} bounds {} \
         sourceSeeds {} keptSeeds {} sourceBeams {} skewSlope {} slopeMargin {} vicinityMargin {} \
         mainStemThickness {} halfBeamLuDx {} maxBeamSideDx {} maxBeamGroupDy {} \
         maxBeamSeedDyRatio {}",
        system.system_id,
        system.profile,
        system.interline,
        stem_seeds.main_stem_thickness,
        rectangle(system.system_bounds),
        seed_system.free_glyphs.len(),
        kept_system.kept_seed_ordinals.len(),
        stump_system.beams_by_abscissa.len(),
        java_hex_double(system.global_slope),
        java_hex_double(system.slope_margin),
        system.vicinity_margin,
        system.main_stem_thickness,
        java_hex_double(system.half_beam_lookup_dx),
        system.max_beam_side_dx,
        system.max_beam_group_dy,
        java_hex_double(system.max_beam_seed_dy_ratio),
    ));
    for part in &system.parts {
        let staff_ordinals = part
            .staff_ids
            .iter()
            .map(|&staff| local_staff_ordinal(system, staff).to_string())
            .collect::<Vec<_>>();
        let staff_bounds = part
            .staff_ids
            .iter()
            .map(|&staff_id| rectangle(staff_area_bounds(system, staff_id)))
            .collect::<Vec<_>>();
        rows.push(format!(
            "stemsbeamvlinkerpart {page} system {} ordinal {} staffOrdinals {} \
             staffAreaBounds {} bounds {}",
            system.system_id,
            part.part_ordinal,
            list(&staff_ordinals),
            list(&staff_bounds),
            rectangle(part.bounds),
        ));
    }

    let groups = group_sources(beams, system.system_id, stump_system);
    let mut aliases = Aliases::preassign(stump_system);
    for constructor in &system.constructors {
        let beam = beam_by_source(stump_system, constructor.source);
        let group = &groups[beam.group_ordinal];
        append_constructor_rows(
            page,
            beams,
            system,
            stump_system,
            seed_system,
            kept_system,
            constructor,
            beam,
            group,
            &mut aliases,
            &mut totals,
            rows,
        );
    }
    assert_system_invariants(system, totals);
    totals
}

#[allow(clippy::too_many_arguments)]
fn append_constructor_rows(
    page: &str,
    beams: &NativeBeamRecognition,
    system: &NativeStemsBeamVLinkerSystem,
    stump_system: &NativeStemsBeamStumpSystem,
    seed_system: &NativeStemSeedSystemRecognition,
    kept_system: &NativeStemsHeadSeedSystem,
    constructor: &NativeStemsBeamVLinkerConstructor,
    beam: &NativeStemsBeamStumpBeam,
    group: &[NativeStemsBeamSource],
    aliases: &mut Aliases,
    totals: &mut Totals,
    rows: &mut Vec<String>,
) {
    let raw = raw_beam(beams, constructor.source);
    let side_stump = |side| {
        beam.sides
            .iter()
            .find(|candidate| candidate.side == side)
            .and_then(|candidate| candidate.final_stump.as_ref())
    };
    let left_stump = side_stump(NativeStemHeadSide::Left)
        .map_or_else(|| "-".to_owned(), |stump| aliases.stump(stump));
    let right_stump = side_stump(NativeStemHeadSide::Right)
        .map_or_else(|| "-".to_owned(), |stump| aliases.stump(stump));
    totals.b_linkers += constructor.b_linkers.len();
    totals.side_b_links += constructor
        .side_b_linkers
        .iter()
        .filter(|entry| entry.b_linker.is_some())
        .count();
    rows.push(format!(
        "stemsbeamvlinkerconstructor {page} system {} constructor {} sigOrdinal {} live {} \
         visible {} shape {} bounds {} median {} height {} topBorder {} bottomBorder {} profile {} \
         grade {} good {} hook {} group group:{} groupMembers {} beamGlyph beamglyph:{} \
         beamGlyphBounds {} beamGlyphRuns {} stumps {} sideStumpLeft {} sideStumpRight {} \
         bLinkers {} sideBLeft {} sideBRight {} stumpLinkers {} tremolo {}",
        system.system_id,
        constructor.x_ordinal,
        beam.sig_ordinal,
        constructor.survives_constructor_loop,
        source_list(&constructor.visible_sources, stump_system),
        shape(beam.kind),
        rectangle(beam.bounds),
        segment(beam.median),
        java_hex_double(beam.height),
        segment(beam_border(beam, NativeStemVerticalSide::Top)),
        segment(beam_border(beam, NativeStemVerticalSide::Bottom)),
        beam.beam_profile,
        java_hex_double(raw.grade),
        raw.grade >= 0.35,
        beam.kind == BeamKind::Hook,
        aliases.group(beam.group_ordinal),
        source_list(group, stump_system),
        aliases.beam_glyph(&beam.beam_glyph),
        rectangle(bounds_rectangle(beam.beam_glyph.bounds)),
        glyph_run_token(&beam.beam_glyph.run_table),
        beam.stumps.len(),
        left_stump,
        right_stump,
        constructor.b_linkers.len(),
        side_b_token(constructor, NativeStemHeadSide::Left),
        side_b_token(constructor, NativeStemHeadSide::Right),
        v_ref_list(&constructor.stump_v_linkers),
        constructor.looks_like_tremolo,
    ));

    for decision in &constructor.orphan_decisions {
        append_orphan_row(
            page,
            system,
            stump_system,
            constructor,
            beam,
            decision,
            aliases,
            totals,
            rows,
        );
    }

    let kept_ordinals = kept_system
        .kept_seed_ordinals
        .iter()
        .enumerate()
        .map(|(kept, &free)| (free, kept))
        .collect::<HashMap<_, _>>();
    let mut creation = 0_usize;
    for b_linker in &constructor.b_linkers {
        append_b_row(
            page,
            system.system_id,
            stump_system,
            seed_system,
            beam,
            b_linker,
            aliases,
            totals,
            rows,
        );
        for v_linker in &b_linker.v_linkers {
            append_v_rows(
                page,
                system,
                stump_system,
                seed_system,
                constructor,
                beam,
                group,
                b_linker,
                v_linker,
                creation,
                &kept_ordinals,
                aliases,
                totals,
                rows,
            );
            creation += 1;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn append_orphan_row(
    page: &str,
    system: &NativeStemsBeamVLinkerSystem,
    stump_system: &NativeStemsBeamStumpSystem,
    constructor: &NativeStemsBeamVLinkerConstructor,
    beam: &NativeStemsBeamStumpBeam,
    decision: &NativeStemsBeamOrphanDecision,
    aliases: &Aliases,
    totals: &mut Totals,
    rows: &mut Vec<String>,
) {
    totals.orphan_checks += 1;
    let (action, side_b) = match decision.outcome {
        NativeStemsBeamOrphanOutcome::ExistingSideBLinker => {
            totals.orphan_existing += 1;
            ("existing", side_b_token(constructor, decision.side))
        }
        NativeStemsBeamOrphanOutcome::NoSiblings => {
            totals.orphan_empty += 1;
            ("empty", "-".to_owned())
        }
        NativeStemsBeamOrphanOutcome::InteriorBeam => {
            totals.orphan_interior += 1;
            ("interior", "-".to_owned())
        }
        NativeStemsBeamOrphanOutcome::Created(reference) => {
            totals.orphan_created += 1;
            ("created", b_ref_token(reference.id))
        }
    };
    let existing = matches!(
        decision.outcome,
        NativeStemsBeamOrphanOutcome::ExistingSideBLinker
    );
    let (end, siblings, first, last, first_glyph, last_glyph, glyph_first, glyph_last) = if existing
    {
        (
            "-".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
        )
    } else {
        let first_source = decision.first_sibling;
        let last_source = decision.last_sibling;
        (
            decision.endpoint.map_or_else(|| "-".to_owned(), point),
            source_list(
                &decision
                    .siblings
                    .iter()
                    .map(|sibling| sibling.source)
                    .collect::<Vec<_>>(),
                stump_system,
            ),
            optional_source(first_source, stump_system),
            optional_source(last_source, stump_system),
            optional_beam_glyph(first_source, stump_system, aliases),
            optional_beam_glyph(last_source, stump_system, aliases),
            decision.beam_glyph_is_first.to_string(),
            decision.beam_glyph_is_last.to_string(),
        )
    };
    rows.push(format!(
        "stemsbeamvlinkerorphan {page} system {} beam {} side {} end {} siblings {} first {} \
         last {} selfGlyph beamglyph:{} firstGlyph {} lastGlyph {} glyphFirst {} glyphLast {} \
         action {} sideB {}",
        system.system_id,
        constructor.x_ordinal,
        head_side(decision.side),
        end,
        siblings,
        first,
        last,
        aliases.beam_glyph(&beam.beam_glyph),
        first_glyph,
        last_glyph,
        glyph_first,
        glyph_last,
        action,
        side_b,
    ));
}

#[allow(clippy::too_many_arguments)]
fn append_b_row(
    page: &str,
    system_id: usize,
    stump_system: &NativeStemsBeamStumpSystem,
    seed_system: &NativeStemSeedSystemRecognition,
    beam: &NativeStemsBeamStumpBeam,
    b_linker: &NativeStemsBeamBLinker,
    aliases: &mut Aliases,
    totals: &mut Totals,
    rows: &mut Vec<String>,
) {
    let orphan = matches!(b_linker.origin, NativeStemsBeamBLinkerOrigin::Orphan { .. });
    if orphan {
        totals.orphan_b += 1;
    } else {
        totals.stump_b += 1;
        totals.zero_direction_b += usize::from(b_linker.v_linkers.is_empty());
    }
    let stump = b_linker
        .stump
        .as_ref()
        .map_or_else(|| "-".to_owned(), |stump| aliases.stump(stump));
    let descriptor = b_linker
        .stump
        .as_ref()
        .map(|stump| stump_descriptor(stump, seed_system, stump_system));
    let stump_ordinal = match b_linker.origin {
        NativeStemsBeamBLinkerOrigin::Stump { list_ordinal } => Some(list_ordinal),
        NativeStemsBeamBLinkerOrigin::Orphan { .. } => None,
    };
    let center_line = stump_ordinal
        .map(|ordinal| {
            beam.stumps
                .get(ordinal)
                .expect("stump B list ordinal")
                .directions
                .stump_center_line
        })
        .map_or_else(|| "-".to_owned(), segment);
    rows.push(format!(
        "stemsbeamvlinkerb {page} system {system_id} beam {} ordinal {} id {} mode {} hSide {} \
         stump {} stumpBounds {} stumpWeight {} stumpCenterLine {} stumpRuns {} ref {} anchor false \
         linked false closed false vLinkers {}",
        beam.x_ordinal,
        b_linker.reference.id - 1,
        b_linker.reference.id,
        if orphan { "orphan" } else { "stump" },
        optional_head_side(b_linker.horizontal_side),
        stump,
        descriptor.map_or_else(|| "-".to_owned(), |glyph| rectangle(glyph.bounds)),
        descriptor.map_or(0, |glyph| glyph.weight),
        center_line,
        descriptor.map_or_else(|| "-".to_owned(), |glyph| glyph_run_token(glyph.run_table)),
        point(b_linker.reference_point),
        v_linker_list(b_linker),
    ));
}

#[allow(clippy::too_many_arguments)]
fn append_v_rows(
    page: &str,
    system: &NativeStemsBeamVLinkerSystem,
    stump_system: &NativeStemsBeamStumpSystem,
    seed_system: &NativeStemSeedSystemRecognition,
    constructor: &NativeStemsBeamVLinkerConstructor,
    beam: &NativeStemsBeamStumpBeam,
    group: &[NativeStemsBeamSource],
    b_linker: &NativeStemsBeamBLinker,
    v_linker: &NativeStemsBeamVLinker,
    creation: usize,
    kept_ordinals: &HashMap<usize, usize>,
    aliases: &mut Aliases,
    totals: &mut Totals,
    rows: &mut Vec<String>,
) {
    totals.v_linkers += 1;
    totals.stump_v += usize::from(v_linker.is_stump_linker);
    totals.orphan_v += usize::from(!v_linker.is_stump_linker);
    match v_linker.reference.side {
        NativeStemVerticalSide::Top => totals.top_v += 1,
        NativeStemVerticalSide::Bottom => totals.bottom_v += 1,
    }
    let stump = b_linker
        .stump
        .as_ref()
        .map_or_else(|| "-".to_owned(), |stump| aliases.stump(stump));
    rows.push(format!(
        "stemsbeamvlinkerv {page} system {} beam {} bOrdinal {} bId {} creation {} mode {} \
         hSide {} stump {} vSide {} yDir {} stoppingHeadSide {} ref {} seedCount {}",
        system.system_id,
        constructor.x_ordinal,
        b_linker.reference.id - 1,
        b_linker.reference.id,
        creation,
        if v_linker.is_stump_linker {
            "stump"
        } else {
            "orphan"
        },
        optional_head_side(b_linker.horizontal_side),
        stump,
        vertical_side(v_linker.reference.side),
        v_linker.y_direction,
        head_side(v_linker.stopping_head_side),
        point(b_linker.reference_point),
        v_linker.reachable_seed_ordinals.len(),
    ));

    append_staff_rows(page, system, constructor, b_linker, v_linker, totals, rows);
    append_alien_rows(
        page,
        system,
        stump_system,
        constructor,
        beam,
        group,
        b_linker,
        v_linker,
        totals,
        rows,
    );
    append_geometry_row(
        page,
        system.system_id,
        constructor.x_ordinal,
        b_linker.reference.id - 1,
        v_linker.reference.side,
        "initial",
        &v_linker.initial_geometry,
        totals,
        rows,
    );
    append_geometry_row(
        page,
        system.system_id,
        constructor.x_ordinal,
        b_linker.reference.id - 1,
        v_linker.reference.side,
        "final",
        &v_linker.final_geometry,
        totals,
        rows,
    );
    append_seed_rows(
        page,
        system.system_id,
        constructor.x_ordinal,
        b_linker.reference.id - 1,
        v_linker,
        seed_system,
        kept_ordinals,
        totals,
        rows,
    );
    assert_v_invariants(v_linker);
}

fn append_staff_rows(
    page: &str,
    system: &NativeStemsBeamVLinkerSystem,
    constructor: &NativeStemsBeamVLinkerConstructor,
    b_linker: &NativeStemsBeamBLinker,
    v_linker: &NativeStemsBeamVLinker,
    totals: &mut Totals,
    rows: &mut Vec<String>,
) {
    let evidence = v_linker
        .initial_geometry
        .system_limit
        .as_ref()
        .expect("initial system/part limit evidence");
    rows.push(format!(
        "stemsbeamvlinkerlimit {page} system {} beam {} bOrdinal {} vSide {} beamCenter {}:{} \
         ref {} systemBounds {} systemYLimit {} around {}",
        system.system_id,
        constructor.x_ordinal,
        b_linker.reference.id - 1,
        vertical_side(v_linker.reference.side),
        evidence.beam_center.0,
        evidence.beam_center.1,
        point(b_linker.reference_point),
        rectangle(system.system_bounds),
        java_hex_double(evidence.initial_y_limit),
        usize_list(
            &evidence
                .around_staff_ids
                .iter()
                .map(|&staff| local_staff_ordinal(system, staff))
                .collect::<Vec<_>>()
        ),
    ));
    for (ordinal, fold) in evidence.part_folds.iter().enumerate() {
        totals.limit_staff_rows += 1;
        rows.push(format!(
            "stemsbeamvlinkerlimitstaff {page} system {} beam {} bOrdinal {} vSide {} ordinal {} \
             staffOrdinal {} partOrdinal {} staffBounds {} partBounds {} before {} candidate {} \
             after {}",
            system.system_id,
            constructor.x_ordinal,
            b_linker.reference.id - 1,
            vertical_side(v_linker.reference.side),
            ordinal,
            local_staff_ordinal(system, fold.staff_id),
            fold.part_ordinal,
            rectangle(fold.staff_bounds),
            rectangle(fold.part_bounds),
            java_hex_double(fold.before_y_limit),
            java_hex_double(fold.candidate_y_limit),
            java_hex_double(fold.after_y_limit),
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn append_alien_rows(
    page: &str,
    system: &NativeStemsBeamVLinkerSystem,
    stump_system: &NativeStemsBeamStumpSystem,
    constructor: &NativeStemsBeamVLinkerConstructor,
    beam: &NativeStemsBeamStumpBeam,
    group: &[NativeStemsBeamSource],
    b_linker: &NativeStemsBeamBLinker,
    v_linker: &NativeStemsBeamVLinker,
    totals: &mut Totals,
    rows: &mut Vec<String>,
) {
    let search = &v_linker.closer_search;
    totals.alien_lookups += 1;
    let neighbors = search
        .neighbor_scan
        .iter()
        .filter(|scan| scan.intersects)
        .map(|scan| scan.source)
        .collect::<Vec<_>>();
    rows.push(format!(
        "stemsbeamvlinkeralienlookup {page} system {} beam {} bOrdinal {} vSide {} visible {} \
         beamBox {} systemBox {} fatBox {} neighbors {} groupMembers {} initialTheo {}",
        system.system_id,
        constructor.x_ordinal,
        b_linker.reference.id - 1,
        vertical_side(v_linker.reference.side),
        source_list(&constructor.visible_sources, stump_system),
        rectangle(beam.bounds),
        rectangle(system.system_bounds),
        rectangle(search.fat_bounds),
        source_list(&neighbors, stump_system),
        source_list(group, stump_system),
        stem_line(v_linker.initial_geometry.theoretical_line),
    ));
    for alien in &search.aliens {
        totals.alien_candidates += 1;
        match alien.action {
            NativeStemsBeamAlienAction::GroupMember => totals.alien_group_drops += 1,
            NativeStemsBeamAlienAction::BadGrade => totals.alien_bad_drops += 1,
            NativeStemsBeamAlienAction::Hook => totals.alien_hook_drops += 1,
            NativeStemsBeamAlienAction::NoTheoreticalIntersection => {
                totals.alien_miss_drops += 1;
            }
            NativeStemsBeamAlienAction::AlignedSide => totals.alien_aligned_drops += 1,
            NativeStemsBeamAlienAction::Survives => totals.alien_accepted += 1,
        }
        let candidate = beam_by_source(stump_system, alien.source);
        rows.push(format!(
            "stemsbeamvlinkeralien {page} system {} beam {} bOrdinal {} vSide {} ordinal {} \
             sigOrdinal {} sameGroup {} grade {} good {} hook {} bounds {} median {} intersects {} \
             cross {} dy {} endpoint {} dx {} maxGroupDy {} maxSideDx {} aligned {} action {}",
            system.system_id,
            constructor.x_ordinal,
            b_linker.reference.id - 1,
            vertical_side(v_linker.reference.side),
            alien.neighbor_ordinal,
            candidate.sig_ordinal,
            alien.action == NativeStemsBeamAlienAction::GroupMember,
            java_hex_double(alien.grade),
            alien.grade >= 0.35,
            alien.kind_is_hook,
            rectangle(candidate.bounds),
            segment(candidate.median),
            alien.median_intersects_theoretical,
            optional_point(alien.cross),
            optional_double(alien.absolute_delta_y),
            optional_double(alien.aligned_endpoint_x),
            optional_double(alien.absolute_delta_x),
            system.max_beam_group_dy,
            system.max_beam_side_dx,
            alien.action == NativeStemsBeamAlienAction::AlignedSide,
            alien_action(alien.action),
        ));
    }
    for (ordinal, &source) in search.sorted_survivors.iter().enumerate() {
        totals.alien_sort_rows += 1;
        let alien = search
            .aliens
            .iter()
            .find(|candidate| candidate.source == source)
            .expect("sorted alien evidence");
        let candidate = beam_by_source(stump_system, source);
        rows.push(format!(
            "stemsbeamvlinkeraliensort {page} system {} beam {} bOrdinal {} vSide {} ordinal {} \
             sigOrdinal {} outgoingBorder {} target {} key {}",
            system.system_id,
            constructor.x_ordinal,
            b_linker.reference.id - 1,
            vertical_side(v_linker.reference.side),
            ordinal,
            candidate.sig_ordinal,
            segment(beam_border(candidate, v_linker.reference.side)),
            point(alien.sort_target.expect("survivor target")),
            java_hex_double(alien.sort_key.expect("survivor key")),
        ));
    }
    totals.alien_shrinks += usize::from(search.selected.is_some());
    rows.push(format!(
        "stemsbeamvlinkeralienselected {page} system {} beam {} bOrdinal {} vSide {} selected {} \
         limitSide {} limit {} refinedYLimit {}",
        system.system_id,
        constructor.x_ordinal,
        b_linker.reference.id - 1,
        vertical_side(v_linker.reference.side),
        optional_source(search.selected, stump_system),
        search.selected.map_or("-", |_| {
            vertical_side(opposite_vertical_side(v_linker.reference.side))
        }),
        search
            .selected_limit
            .map_or_else(|| "-".to_owned(), segment),
        search.selected.map_or_else(
            || "-".to_owned(),
            |_| java_hex_double(v_linker.final_geometry.y_limit)
        ),
    ));
}

#[allow(clippy::too_many_arguments)]
fn append_geometry_row(
    page: &str,
    system_id: usize,
    beam_ordinal: usize,
    b_ordinal: usize,
    side: NativeStemVerticalSide,
    phase: &str,
    geometry: &NativeStemsBeamLuGeometry,
    totals: &mut Totals,
    rows: &mut Vec<String>,
) {
    totals.geometries += 1;
    rows.push(format!(
        "stemsbeamvlinkergeometry {page} system {system_id} beam {beam_ordinal} bOrdinal {b_ordinal} \
         vSide {} phase {phase} border {} pl {} pr {} profile {} yGapPixels {} yOffset {} \
         skewSlope {} luSlope {} dSlope {} yLimit {} dy {} quad {} bounds {} bounds2d {} theo {}",
        vertical_side(side),
        segment(geometry.border),
        point(geometry.left_border_point),
        point(geometry.right_border_point),
        geometry.effective_profile,
        geometry.gap_pixels,
        java_hex_double(geometry.y_offset),
        java_hex_double(-geometry.slope),
        java_hex_double(geometry.slope),
        java_hex_double(geometry.delta_slope),
        java_hex_double(geometry.y_limit),
        java_hex_double(geometry.delta_y),
        geometry
            .quadrilateral
            .iter()
            .copied()
            .map(point)
            .collect::<Vec<_>>()
            .join(":"),
        rectangle(geometry.bounds),
        double_bounds(geometry.double_bounds),
        stem_line(geometry.theoretical_line),
    ));
}

#[allow(clippy::too_many_arguments)]
fn append_seed_rows(
    page: &str,
    system_id: usize,
    beam_ordinal: usize,
    b_ordinal: usize,
    v_linker: &NativeStemsBeamVLinker,
    seed_system: &NativeStemSeedSystemRecognition,
    kept_ordinals: &HashMap<usize, usize>,
    totals: &mut Totals,
    rows: &mut Vec<String>,
) {
    for (ordinal, check) in v_linker.seed_checks.iter().enumerate() {
        totals.seed_candidates += 1;
        rows.push(format!(
            "stemsbeamvlinkerseedcandidate {page} system {system_id} beam {beam_ordinal} bOrdinal \
             {b_ordinal} vSide {} ordinal {ordinal} glyph kept:{} bounds {} hit {}",
            vertical_side(v_linker.reference.side),
            kept_ordinals[&check.free_glyph_ordinal],
            rectangle(check.bounds),
            check.intersects_final_area,
        ));
    }
    for (ordinal, &free) in v_linker.reachable_seed_ordinals.iter().enumerate() {
        totals.seed_hits += 1;
        let glyph = &seed_system.free_glyphs[free];
        rows.push(format!(
            "stemsbeamvlinkerseed {page} system {system_id} beam {beam_ordinal} bOrdinal \
             {b_ordinal} vSide {} ordinal {ordinal} glyph kept:{} bounds {}",
            vertical_side(v_linker.reference.side),
            kept_ordinals[&free],
            rectangle(bounds_rectangle(glyph.bounds)),
        ));
    }
}

fn assert_v_invariants(v_linker: &NativeStemsBeamVLinker) {
    let expected_hits = v_linker
        .seed_checks
        .iter()
        .filter(|check| check.intersects_final_area)
        .map(|check| check.free_glyph_ordinal)
        .collect::<Vec<_>>();
    assert_eq!(v_linker.reachable_seed_ordinals, expected_hits);
    let sorted = v_linker
        .closer_search
        .aliens
        .iter()
        .filter_map(|alien| {
            alien
                .survivor_sorted_ordinal
                .map(|ordinal| (ordinal, alien.source))
        })
        .collect::<Vec<_>>();
    let mut ordered = sorted;
    ordered.sort_by_key(|entry| entry.0);
    assert_eq!(
        v_linker.closer_search.sorted_survivors,
        ordered
            .into_iter()
            .map(|(_, source)| source)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        v_linker.closer_search.selected,
        v_linker.closer_search.sorted_survivors.first().copied()
    );
    if v_linker.closer_search.selected.is_none() {
        assert_eq!(v_linker.final_geometry, v_linker.initial_geometry);
    } else {
        assert!(v_linker.final_geometry.system_limit.is_none());
    }
}

fn assert_system_invariants(system: &NativeStemsBeamVLinkerSystem, totals: Totals) {
    for constructor in &system.constructors {
        for (ordinal, b_linker) in constructor.b_linkers.iter().enumerate() {
            assert_eq!(b_linker.reference.id, ordinal + 1);
            assert_eq!(b_linker.reference.beam, constructor.source);
            for v_linker in &b_linker.v_linkers {
                assert_eq!(v_linker.reference.b_linker, b_linker.reference);
            }
        }
        for side in &constructor.side_b_linkers {
            if let Some(reference) = side.b_linker {
                assert!(
                    constructor
                        .b_linkers
                        .iter()
                        .any(|b| b.reference == reference)
                );
            }
        }
        for reference in &constructor.stump_v_linkers {
            let v_linker = constructor
                .b_linkers
                .iter()
                .flat_map(|b| &b.v_linkers)
                .find(|v| v.reference == *reference)
                .expect("stump V reference resolves");
            assert!(v_linker.is_stump_linker);
        }
    }
    assert_eq!(totals.orphan_checks, totals.constructors * 2);
    assert_eq!(
        totals.orphan_checks,
        totals.orphan_existing
            + totals.orphan_created
            + totals.orphan_empty
            + totals.orphan_interior
    );
    assert_eq!(
        totals.side_b_links,
        totals.orphan_existing + totals.orphan_created
    );
    assert_eq!(totals.orphan_b, totals.orphan_created);
    assert_eq!(totals.b_linkers, totals.stump_b + totals.orphan_b);
    assert_eq!(totals.orphan_v, totals.orphan_b * 2);
    assert_eq!(totals.v_linkers, totals.stump_v + totals.orphan_v);
    assert_eq!(totals.v_linkers, totals.top_v + totals.bottom_v);
    assert_eq!(totals.geometries, totals.v_linkers * 2);
    assert_eq!(totals.alien_lookups, totals.v_linkers);
    assert_eq!(
        totals.alien_candidates,
        totals.alien_group_drops
            + totals.alien_bad_drops
            + totals.alien_hook_drops
            + totals.alien_miss_drops
            + totals.alien_aligned_drops
            + totals.alien_accepted
    );
    assert_eq!(totals.alien_sort_rows, totals.alien_accepted);
    assert!(totals.seed_hits <= totals.seed_candidates);
}

fn system_summary_row(page: &str, system_id: usize, totals: Totals, hash: u64) -> String {
    format!(
        "stemsbeamvlinkersystemsummary {page} system {system_id} systems 1 constructors {} \
         survivors {} tremolos {} parts {} stumpBs {} orphanAttempts {} orphanCreated {} Bs {} \
         Vs {} stumpVs {} initialGeometries {} rebuiltGeometries {} alienNeighborScans {} \
         alienCandidates {} alienSurvivors {} alienLimiters {} seedChecks {} reachableSeeds {} \
         orphanBs {} zeroDirectionBs {} orphanVs {} topVs {} bottomVs {} sideBLinks {} \
         orphanChecks {} orphanExisting {} orphanEmpty {} orphanInterior {} partFoldRows {} \
         finalGeometries {} geometryRows {} alienLookups {} alienCandidateRows {} \
         alienGroupDrops {} alienBadDrops {} alienHookDrops {} alienMissDrops {} \
         alienAlignedDrops {} alienAcceptedRows {} alienSortRows {} alienShrinkRows {} \
         seedCandidateRows {} seedHitRows {} hash {hash:016x}",
        totals.constructors,
        totals.live_beams,
        totals.tremolos,
        totals.parts,
        totals.stump_b,
        totals.orphan_checks - totals.orphan_existing,
        totals.orphan_created,
        totals.b_linkers,
        totals.v_linkers,
        totals.stump_v,
        totals.v_linkers,
        totals.alien_shrinks,
        totals.alien_candidates,
        totals.alien_candidates,
        totals.alien_accepted,
        totals.alien_shrinks,
        totals.seed_candidates,
        totals.seed_hits,
        totals.orphan_b,
        totals.zero_direction_b,
        totals.orphan_v,
        totals.top_v,
        totals.bottom_v,
        totals.side_b_links,
        totals.orphan_checks,
        totals.orphan_existing,
        totals.orphan_empty,
        totals.orphan_interior,
        totals.limit_staff_rows,
        totals.v_linkers,
        totals.geometries,
        totals.alien_lookups,
        totals.alien_candidates,
        totals.alien_group_drops,
        totals.alien_bad_drops,
        totals.alien_hook_drops,
        totals.alien_miss_drops,
        totals.alien_aligned_drops,
        totals.alien_accepted,
        totals.alien_sort_rows,
        totals.alien_shrinks,
        totals.seed_candidates,
        totals.seed_hits,
    )
}

fn page_summary_row(page: &str, systems: usize, totals: Totals, hash: u64) -> String {
    format!(
        "stemsbeamvlinkerpagesummary {page} systems {systems} constructors {} survivors {} \
         tremolos {} parts {} stumpBs {} orphanAttempts {} orphanCreated {} Bs {} Vs {} stumpVs {} \
         initialGeometries {} rebuiltGeometries {} alienNeighborScans {} alienCandidates {} \
         alienSurvivors {} alienLimiters {} seedChecks {} reachableSeeds {} orphanBs {} \
         zeroDirectionBs {} orphanVs {} topVs {} bottomVs {} sideBLinks {} orphanChecks {} \
         orphanExisting {} orphanEmpty {} orphanInterior {} partFoldRows {} finalGeometries {} \
         geometryRows {} alienLookups {} alienCandidateRows {} alienGroupDrops {} alienBadDrops {} \
         alienHookDrops {} alienMissDrops {} alienAlignedDrops {} alienAcceptedRows {} alienSortRows {} \
         alienShrinkRows {} seedCandidateRows {} seedHitRows {} hash {hash:016x}",
        totals.constructors,
        totals.live_beams,
        totals.tremolos,
        totals.parts,
        totals.stump_b,
        totals.orphan_checks - totals.orphan_existing,
        totals.orphan_created,
        totals.b_linkers,
        totals.v_linkers,
        totals.stump_v,
        totals.v_linkers,
        totals.alien_shrinks,
        totals.alien_candidates,
        totals.alien_candidates,
        totals.alien_accepted,
        totals.alien_shrinks,
        totals.seed_candidates,
        totals.seed_hits,
        totals.orphan_b,
        totals.zero_direction_b,
        totals.orphan_v,
        totals.top_v,
        totals.bottom_v,
        totals.side_b_links,
        totals.orphan_checks,
        totals.orphan_existing,
        totals.orphan_empty,
        totals.orphan_interior,
        totals.limit_staff_rows,
        totals.v_linkers,
        totals.geometries,
        totals.alien_lookups,
        totals.alien_candidates,
        totals.alien_group_drops,
        totals.alien_bad_drops,
        totals.alien_hook_drops,
        totals.alien_miss_drops,
        totals.alien_aligned_drops,
        totals.alien_accepted,
        totals.alien_sort_rows,
        totals.alien_shrinks,
        totals.seed_candidates,
        totals.seed_hits,
    )
}

fn assert_known_totals(totals: Totals) {
    assert_eq!(totals.constructors, 803);
    assert_eq!(totals.live_beams, 803);
    assert_eq!(totals.tremolos, 0);
    assert_eq!(totals.parts, 30);
    assert_eq!(totals.stump_b, 1_821);
    assert_eq!(totals.orphan_b, 295);
    assert_eq!(totals.b_linkers, 2_116);
    assert_eq!(totals.v_linkers, 2_417);
    assert_eq!(totals.stump_v, 1_827);
    assert_eq!(totals.orphan_v, 590);
    assert_eq!(totals.top_v, 1_389);
    assert_eq!(totals.bottom_v, 1_028);
    assert_eq!(totals.limit_staff_rows, 2_860);
    assert_eq!(totals.geometries, 4_834);
    assert_eq!(totals.alien_lookups, 2_417);
    assert_eq!(totals.alien_candidates, 9_186);
    assert_eq!(totals.alien_group_drops, 4_738);
    assert_eq!(totals.alien_bad_drops, 38);
    assert_eq!(totals.alien_hook_drops, 501);
    assert_eq!(totals.alien_miss_drops, 2_812);
    assert_eq!(totals.alien_aligned_drops, 3);
    assert_eq!(totals.alien_accepted, 1_094);
    assert_eq!(totals.alien_sort_rows, 1_094);
    assert_eq!(totals.alien_shrinks, 703);
    assert_eq!(totals.seed_candidates, 12_491);
    assert_eq!(totals.seed_hits, 2_169);
}

fn assert_corpus_summary(oracle: &str) {
    let summary = oracle
        .lines()
        .find(|line| line.starts_with("stemsbeamvlinkercorpussummary "))
        .expect("beam-VLinker corpus summary");
    for (field, expected) in [
        ("pages", "8"),
        ("systems", "30"),
        ("constructors", "803"),
        ("survivors", "803"),
        ("tremolos", "0"),
        ("parts", "30"),
        ("stumpBs", "1821"),
        ("orphanCreated", "295"),
        ("Bs", "2116"),
        ("Vs", "2417"),
        ("stumpVs", "1827"),
        ("initialGeometries", "2417"),
        ("rebuiltGeometries", "703"),
        ("alienNeighborScans", "9186"),
        ("alienCandidates", "9186"),
        ("alienSurvivors", "1094"),
        ("alienLimiters", "703"),
        ("seedChecks", "12491"),
        ("reachableSeeds", "2169"),
        ("orphanBs", "295"),
        ("orphanVs", "590"),
        ("topVs", "1389"),
        ("bottomVs", "1028"),
        ("partFoldRows", "2860"),
        ("finalGeometries", "2417"),
        ("geometryRows", "4834"),
        ("alienLookups", "2417"),
        ("alienCandidateRows", "9186"),
        ("alienGroupDrops", "4738"),
        ("alienBadDrops", "38"),
        ("alienHookDrops", "501"),
        ("alienMissDrops", "2812"),
        ("alienAlignedDrops", "3"),
        ("alienAcceptedRows", "1094"),
        ("alienSortRows", "1094"),
        ("alienShrinkRows", "703"),
        ("seedCandidateRows", "12491"),
        ("seedHitRows", "2169"),
    ] {
        assert_eq!(
            field_value(summary, field),
            expected,
            "summary field {field}"
        );
    }
    if let Some(expected) = EXPECTED_PROBE_SHA256 {
        assert_eq!(field_value(summary, "probeSourceSha256"), expected);
        assert_eq!(
            sha256_hex(&std::fs::read(repo_path(PROBE_PATH)).expect("probe source")),
            expected
        );
    }
    if let Some(expected) = EXPECTED_RUNNER_SHA256 {
        assert_eq!(field_value(summary, "runnerSourceSha256"), expected);
        assert_eq!(
            sha256_hex(&std::fs::read(repo_path(RUNNER_PATH)).expect("oracle runner")),
            expected
        );
    }
    if let Some(expected) = EXPECTED_BODY_SHA256 {
        assert_eq!(field_value(summary, "emittedBodySha256"), expected);
    }
    if let Some(expected) = EXPECTED_FIXTURE_SHA256 {
        assert_eq!(sha256_hex(oracle.as_bytes()), expected);
    }
    if let Some(expected) = EXPECTED_FIXTURE_BYTES {
        assert_eq!(oracle.len(), expected);
    }
    if let Some(expected) = EXPECTED_FIXTURE_LINES {
        assert_eq!(oracle.lines().count(), expected);
    }
}

fn oracle_projected_rows(oracle: &str, page: &str) -> Vec<String> {
    oracle
        .lines()
        .filter(|line| line.split_whitespace().nth(1) == Some(page))
        .filter_map(project_oracle_row)
        .collect()
}

fn project_oracle_row(row: &str) -> Option<String> {
    let prefix = row.split_whitespace().next()?;
    match prefix {
        "stemsbeamvlinkerpage"
        | "stemsbeamvlinkersystem"
        | "stemsbeamvlinkerpart"
        | "stemsbeamvlinkerconstructor"
        | "stemsbeamvlinkerb"
        | "stemsbeamvlinkerv"
        | "stemsbeamvlinkerlimit"
        | "stemsbeamvlinkerlimitstaff"
        | "stemsbeamvlinkergeometry"
        | "stemsbeamvlinkeralienlookup"
        | "stemsbeamvlinkeralien"
        | "stemsbeamvlinkeraliensort"
        | "stemsbeamvlinkeralienselected"
        | "stemsbeamvlinkerseedcandidate"
        | "stemsbeamvlinkerseed"
        | "stemsbeamvlinkersystemsummary"
        | "stemsbeamvlinkerpagesummary" => Some(row.to_owned()),
        "stemsbeamvlinkerorphan" => {
            if field_value(row, "action") == "existing" {
                let mut normalized = row.to_owned();
                for field in [
                    "end",
                    "siblings",
                    "first",
                    "last",
                    "firstGlyph",
                    "lastGlyph",
                    "glyphFirst",
                    "glyphLast",
                ] {
                    normalized = replace_field(&normalized, field, "-");
                }
                Some(normalized)
            } else {
                Some(row.to_owned())
            }
        }
        // Java2D Area may canonicalize PathIterator segment order. Raw quad,
        // bounds and every downstream intersection decision above are graded.
        "stemsbeamvlinkerpath" | "stemsbeamvlinkerareapath" => None,
        _ => None,
    }
}

fn stump_descriptor<'a>(
    stump: &NativeStemsBeamStumpRef,
    seed_system: &'a NativeStemSeedSystemRecognition,
    stump_system: &'a NativeStemsBeamStumpSystem,
) -> GlyphDescriptor<'a> {
    match stump {
        NativeStemsBeamStumpRef::Seed {
            free_glyph_ordinal, ..
        } => {
            let glyph = &seed_system.free_glyphs[*free_glyph_ordinal];
            GlyphDescriptor {
                bounds: bounds_rectangle(glyph.bounds),
                weight: glyph.weight,
                run_table: &glyph.run_table,
            }
        }
        NativeStemsBeamStumpRef::Built {
            canonical_glyph_index,
        } => {
            let glyph = stump_system
                .beams_by_abscissa
                .iter()
                .flat_map(|beam| &beam.sides)
                .filter_map(|side| side.build.as_ref())
                .find(|build| build.canonical_glyph_index == Some(*canonical_glyph_index))
                .and_then(|build| build.candidate.as_ref())
                .expect("built stump glyph");
            GlyphDescriptor {
                bounds: bounds_rectangle(glyph.bounds),
                weight: glyph.weight,
                run_table: &glyph.run_table,
            }
        }
    }
}

fn group_sources(
    beams: &NativeBeamRecognition,
    system_id: usize,
    stump_system: &NativeStemsBeamStumpSystem,
) -> Vec<Vec<NativeStemsBeamSource>> {
    let members = beams
        .raw_beams
        .iter()
        .enumerate()
        .filter_map(|(ordinal, (owner, _))| {
            (*owner == system_id).then_some(NativeStemsBeamSource::RawBeam(ordinal))
        })
        .chain(
            beams
                .hooks
                .iter()
                .enumerate()
                .filter_map(|(ordinal, (owner, _))| {
                    (*owner == system_id).then_some(NativeStemsBeamSource::Hook(ordinal))
                }),
        )
        .collect::<Vec<_>>();
    beams
        .group_memberships
        .iter()
        .find(|state| state.system_id == system_id)
        .expect("beam group system")
        .groups
        .iter()
        .map(|group| {
            group
                .iter()
                .map(|&ordinal| members[ordinal])
                // HEADS can remove a beam after the BEAMS grouping snapshot.
                // Java's live `BeamGroupInter.getMembers()` observes the SIG
                // after those removals, so identities absent from the stump
                // boundary cannot appear in the projected group stream.
                .filter(|source| {
                    stump_system
                        .beams_by_abscissa
                        .iter()
                        .any(|beam| beam.source == *source)
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn raw_beam(beams: &NativeBeamRecognition, source: NativeStemsBeamSource) -> &RawBeam {
    match source {
        NativeStemsBeamSource::RawBeam(ordinal) => &beams.raw_beams[ordinal].1,
        NativeStemsBeamSource::Hook(ordinal) => &beams.hooks[ordinal].1,
    }
}

fn system_by_id(
    recognition: &NativeStemsBeamStumpRecognition,
    system_id: usize,
) -> &NativeStemsBeamStumpSystem {
    recognition
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .expect("beam stump system")
}

fn seed_system_by_id(
    recognition: &NativeStemSeedRecognition,
    system_id: usize,
) -> &NativeStemSeedSystemRecognition {
    recognition
        .systems
        .iter()
        .find(|system| system.raw.system_id == system_id)
        .expect("stem seed system")
}

fn kept_system_by_id(
    recognition: &NativeStemsHeadSeedRecognition,
    system_id: usize,
) -> &NativeStemsHeadSeedSystem {
    recognition
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .expect("kept seed system")
}

fn beam_by_source(
    system: &NativeStemsBeamStumpSystem,
    source: NativeStemsBeamSource,
) -> &NativeStemsBeamStumpBeam {
    system
        .beams_by_abscissa
        .iter()
        .find(|beam| beam.source == source)
        .expect("beam source")
}

fn source_list(sources: &[NativeStemsBeamSource], system: &NativeStemsBeamStumpSystem) -> String {
    usize_list(
        &sources
            .iter()
            .map(|&source| beam_by_source(system, source).sig_ordinal)
            .collect::<Vec<_>>(),
    )
}

fn optional_source(
    source: Option<NativeStemsBeamSource>,
    system: &NativeStemsBeamStumpSystem,
) -> String {
    source.map_or_else(
        || "-".to_owned(),
        |source| beam_by_source(system, source).sig_ordinal.to_string(),
    )
}

fn optional_beam_glyph(
    source: Option<NativeStemsBeamSource>,
    system: &NativeStemsBeamStumpSystem,
    aliases: &Aliases,
) -> String {
    source.map_or_else(
        || "-".to_owned(),
        |source| {
            format!(
                "beamglyph:{}",
                aliases.beam_glyph(&beam_by_source(system, source).beam_glyph)
            )
        },
    )
}

fn local_staff_ordinal(system: &NativeStemsBeamVLinkerSystem, staff_id: usize) -> usize {
    system
        .staff_ids
        .iter()
        .position(|&candidate| candidate == staff_id)
        .expect("system staff")
}

fn staff_area_bounds(system: &NativeStemsBeamVLinkerSystem, staff_id: usize) -> JavaRectangle {
    system
        .constructors
        .iter()
        .flat_map(|constructor| &constructor.b_linkers)
        .flat_map(|b_linker| &b_linker.v_linkers)
        .filter_map(|v_linker| v_linker.initial_geometry.system_limit.as_ref())
        .flat_map(|limit| &limit.part_folds)
        .find(|fold| fold.staff_id == staff_id)
        .map(|fold| fold.staff_bounds)
        .expect("staff area retained by a system-limit fold")
}

fn side_b_token(
    constructor: &NativeStemsBeamVLinkerConstructor,
    side: NativeStemHeadSide,
) -> String {
    constructor
        .side_b_linkers
        .iter()
        .find(|entry| entry.side == side)
        .and_then(|entry| entry.b_linker)
        .map_or_else(|| "-".to_owned(), |reference| b_ref_token(reference.id))
}

fn b_ref_token(id: usize) -> String {
    format!("b:{}", id - 1)
}

fn v_ref_list(
    references: &[audiveris_omr::native_stems_beam_vlinkers::NativeStemsBeamVLinkerRef],
) -> String {
    list(
        &references
            .iter()
            .map(|reference| {
                format!(
                    "b:{}:{}",
                    reference.b_linker.id - 1,
                    vertical_side(reference.side)
                )
            })
            .collect::<Vec<_>>(),
    )
}

fn v_linker_list(b_linker: &NativeStemsBeamBLinker) -> String {
    list(
        &b_linker
            .v_linkers
            .iter()
            .map(|v| {
                format!(
                    "b:{}:{}",
                    b_linker.reference.id - 1,
                    vertical_side(v.reference.side)
                )
            })
            .collect::<Vec<_>>(),
    )
}

fn glyph_run_token(table: &RunTable) -> String {
    let mut count = 0_usize;
    let mut hash = RowHasher::default();
    hash.add(&format!(
        "{} {} {}",
        orientation(table.orientation()),
        table.width(),
        table.height()
    ));
    for sequence in 0..table.sequence_count() {
        let mut row = sequence.to_string();
        for run in table.sequence(sequence).expect("run sequence") {
            count += 1;
            row.push_str(&format!(" {}:{}", run.start, run.length));
        }
        hash.add(&row);
    }
    format!("{count}:{:016x}", hash.0)
}

fn beam_border(beam: &NativeStemsBeamStumpBeam, side: NativeStemVerticalSide) -> Segment {
    let dy = if side == NativeStemVerticalSide::Top {
        -beam.height / 2.0
    } else {
        beam.height / 2.0
    };
    Segment {
        x1: beam.median.x1,
        y1: beam.median.y1 + dy,
        x2: beam.median.x2,
        y2: beam.median.y2 + dy,
    }
}

fn opposite_vertical_side(side: NativeStemVerticalSide) -> NativeStemVerticalSide {
    match side {
        NativeStemVerticalSide::Top => NativeStemVerticalSide::Bottom,
        NativeStemVerticalSide::Bottom => NativeStemVerticalSide::Top,
    }
}

fn shape(kind: BeamKind) -> &'static str {
    match kind {
        BeamKind::Beam => "BEAM",
        BeamKind::Hook => "BEAM_HOOK",
        BeamKind::SmallBeam => "BEAM_SMALL",
    }
}

fn head_side(side: NativeStemHeadSide) -> &'static str {
    match side {
        NativeStemHeadSide::Left => "LEFT",
        NativeStemHeadSide::Right => "RIGHT",
    }
}

fn optional_head_side(side: Option<NativeStemHeadSide>) -> &'static str {
    side.map_or("-", head_side)
}

fn vertical_side(side: NativeStemVerticalSide) -> &'static str {
    match side {
        NativeStemVerticalSide::Top => "TOP",
        NativeStemVerticalSide::Bottom => "BOTTOM",
    }
}

fn alien_action(action: NativeStemsBeamAlienAction) -> &'static str {
    match action {
        NativeStemsBeamAlienAction::GroupMember => "group",
        NativeStemsBeamAlienAction::BadGrade => "bad",
        NativeStemsBeamAlienAction::Hook => "hook",
        NativeStemsBeamAlienAction::NoTheoreticalIntersection => "miss",
        NativeStemsBeamAlienAction::AlignedSide => "aligned",
        NativeStemsBeamAlienAction::Survives => "accept",
    }
}

fn orientation(orientation: Orientation) -> &'static str {
    match orientation {
        Orientation::Horizontal => "HORIZONTAL",
        Orientation::Vertical => "VERTICAL",
    }
}

fn bounds_rectangle(bounds: Bounds) -> JavaRectangle {
    JavaRectangle {
        x: i32::try_from(bounds.x).expect("glyph x fits i32"),
        y: i32::try_from(bounds.y).expect("glyph y fits i32"),
        width: i32::try_from(bounds.width).expect("glyph width fits i32"),
        height: i32::try_from(bounds.height).expect("glyph height fits i32"),
    }
}

fn rectangle(rectangle: JavaRectangle) -> String {
    format!(
        "{}:{}:{}:{}",
        rectangle.x, rectangle.y, rectangle.width, rectangle.height
    )
}

fn double_bounds(bounds: NativeStemsBeamDoubleBounds) -> String {
    format!(
        "{}:{}:{}:{}",
        java_hex_double(bounds.x),
        java_hex_double(bounds.y),
        java_hex_double(bounds.width),
        java_hex_double(bounds.height),
    )
}

fn point(point: NativeStemPoint) -> String {
    format!("{}:{}", java_hex_double(point.x), java_hex_double(point.y))
}

fn optional_point(value: Option<NativeStemPoint>) -> String {
    value.map_or_else(|| "-".to_owned(), point)
}

fn segment(line: Segment) -> String {
    format!(
        "{}:{}:{}:{}",
        java_hex_double(line.x1),
        java_hex_double(line.y1),
        java_hex_double(line.x2),
        java_hex_double(line.y2),
    )
}

fn stem_line(line: NativeStemLine) -> String {
    format!("{}:{}", point(line.start), point(line.stop))
}

fn optional_double(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_owned(), java_hex_double)
}

fn list(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_owned()
    } else {
        values.join(",")
    }
}

fn usize_list(values: &[usize]) -> String {
    list(&values.iter().map(usize::to_string).collect::<Vec<_>>())
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

fn replace_field(row: &str, name: &str, replacement: &str) -> String {
    let mut words = row
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let index = words
        .iter()
        .position(|word| word == name)
        .unwrap_or_else(|| panic!("missing {name} in {row}"));
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
    let mut shown = 0;
    for index in 0..actual.len().max(expected.len()) {
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
