// SPDX-License-Identifier: AGPL-3.0-or-later

//! Sparse page/system/head rows and derived summary certification.

use super::*;

use audiveris_omr::{
    head_template::HeadTemplateShape,
    native_heads_staff_epilog::{
        NativeHeadStaffEpilogHead, NativeHeadStaffEpilogOrigin, NativeHeadStaffEpilogRef,
    },
    native_stems_head_builders::{
        NativeStemsHeadBuilder, NativeStemsHeadBuilderGlyphRef, NativeStemsHeadBuilderSystem,
    },
    native_stems_head_corner_reachability::{
        NativeStemsHeadCornerReachabilitySystem, NativeStemsHeadFindResult,
        NativeStemsHeadReachabilityHead,
    },
};

#[derive(Clone, Debug, Default)]
pub(super) struct SparseSystemProjection {
    pub seed_duplicates: String,
    pub system: String,
    pub heads: BTreeMap<usize, String>,
    pub ends: BTreeMap<usize, String>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct SparseProjection {
    pub page: String,
    pub systems: BTreeMap<usize, SparseSystemProjection>,
}

struct ExpectedSparseRows<'a> {
    page: &'a str,
    systems: BTreeMap<usize, &'a str>,
    seed_duplicates: BTreeMap<usize, &'a str>,
    heads: BTreeMap<(usize, usize), &'a str>,
    ends: BTreeMap<(usize, usize), &'a str>,
    system_summaries: BTreeMap<usize, &'a str>,
}

impl<'a> ExpectedSparseRows<'a> {
    fn parse(bytes: &'a [u8]) -> Self {
        let text = std::str::from_utf8(bytes).expect("head-builder sparse fixture UTF-8");
        let mut page = None;
        let mut systems = BTreeMap::new();
        let mut seed_duplicates = BTreeMap::new();
        let mut heads = BTreeMap::new();
        let mut ends = BTreeMap::new();
        let mut system_summaries = BTreeMap::new();
        for row in text
            .lines()
            .filter(|row| !row.is_empty() && !row.starts_with('#'))
        {
            match row.split_whitespace().next().expect("sparse row family") {
                "stemsheadbuilderpage" => {
                    assert!(page.replace(row).is_none(), "duplicate page row");
                }
                "stemsheadbuildersystem" => {
                    assert!(
                        systems.insert(required_usize(row, "system"), row).is_none(),
                        "duplicate system row"
                    );
                }
                "stemsheadbuilderseedduplicates" => {
                    assert!(
                        seed_duplicates
                            .insert(required_usize(row, "system"), row)
                            .is_none(),
                        "duplicate seed-duplicate row"
                    );
                }
                "stemsheadbuilderhead" => {
                    let key = (
                        required_usize(row, "system"),
                        required_usize(row, "xOrdinal"),
                    );
                    assert!(heads.insert(key, row).is_none(), "duplicate head row");
                }
                "stemsheadbuilderend" => {
                    let key = (
                        required_usize(row, "system"),
                        required_usize(row, "builder"),
                    );
                    assert!(ends.insert(key, row).is_none(), "duplicate end row");
                }
                "stemsheadbuildersystemsummary" => {
                    assert!(
                        system_summaries
                            .insert(required_usize(row, "system"), row)
                            .is_none(),
                        "duplicate system summary row"
                    );
                }
                _ => {}
            }
        }
        Self {
            page: page.expect("missing page row"),
            systems,
            seed_duplicates,
            heads,
            ends,
            system_summaries,
        }
    }
}

pub(super) fn validate_sparse_projection(
    page: &str,
    bytes: &[u8],
    native: &NativeProjectionPage,
    registry: &registry_rows::RegistryProjection,
) -> SparseProjection {
    let expected = ExpectedSparseRows::parse(bytes);
    let projected_page = page_row(page, native);
    assert_eq!(projected_page, expected.page, "{page}: page row");
    let mut projection = SparseProjection {
        page: projected_page,
        systems: BTreeMap::new(),
    };

    let mut head_rows = 0usize;
    let mut end_rows = 0usize;
    for system in &native.head_builders.systems {
        let registry_system = &registry.systems[&system.system_id];
        let mut projected_system = SparseSystemProjection::default();
        let reach_system = native
            .head_reachability
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system.system_id)
            .expect("sparse reachability system");
        let beam_stumps = native
            .beam_stumps
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system.system_id)
            .expect("sparse beam-stump system");
        projected_system.system = system_row(page, system, reach_system, beam_stumps);
        assert_eq!(
            projected_system.system, expected.systems[&system.system_id],
            "{page}: system {} row",
            system.system_id
        );
        projected_system.seed_duplicates = seed_duplicate_row(page, native, reach_system);
        assert_eq!(
            projected_system.seed_duplicates, expected.seed_duplicates[&system.system_id],
            "{page}: system {} seed duplicates",
            system.system_id
        );

        for head in &reach_system.heads {
            let projected = head_row(page, native, reach_system, head);
            assert_eq!(
                projected,
                expected.heads[&(system.system_id, head.x_ordinal)],
                "{page}: system {} head {}",
                system.system_id,
                head.x_ordinal
            );
            assert!(
                projected_system
                    .heads
                    .insert(head.x_ordinal, projected)
                    .is_none()
            );
            head_rows += 1;
        }
        for builder in &system.builders {
            let projected = end_row(
                page,
                native,
                system,
                reach_system,
                builder,
                &registry_system.end_fields[&builder.builder_ordinal],
            );
            assert_eq!(
                projected,
                expected.ends[&(system.system_id, builder.builder_ordinal)],
                "{page}: system {} builder {} end row",
                system.system_id,
                builder.builder_ordinal,
            );
            assert!(
                projected_system
                    .ends
                    .insert(builder.builder_ordinal, projected)
                    .is_none()
            );
            end_rows += 1;
        }
        assert!(
            projection
                .systems
                .insert(system.system_id, projected_system)
                .is_none()
        );
    }
    assert_eq!(expected.systems.len(), native.head_builders.systems.len());
    assert_eq!(
        expected.seed_duplicates.len(),
        native.head_builders.systems.len()
    );
    assert_eq!(
        expected.heads.len(),
        head_rows,
        "{page}: sparse head coverage"
    );
    assert_eq!(expected.ends.len(), end_rows, "{page}: sparse end coverage");
    assert_eq!(
        expected.system_summaries.len(),
        native.head_builders.systems.len(),
        "{page}: sparse system summary coverage"
    );

    projection
}

fn page_row(page: &str, native: &NativeProjectionPage) -> String {
    let family = native
        .beams
        .music_font_scale
        .expect("head-builder music font scale")
        .family;
    format!(
        "stemsheadbuilderpage {page} systems {} staves {} family {family:?}",
        native.head_builders.systems.len(),
        native.grid.peak_graph.sheet_staffs.len(),
    )
}

fn system_row(
    page: &str,
    system: &NativeStemsHeadBuilderSystem,
    reach: &NativeStemsHeadCornerReachabilitySystem,
    stumps: &NativeStemsBeamStumpSystem,
) -> String {
    let beam_sig_order = reach
        .beam_sources_in_sig_order
        .iter()
        .map(|&source| beam_sig_ordinal(stumps, source))
        .collect::<Vec<_>>();
    let beam_x_order = reach
        .beam_sources_by_abscissa
        .iter()
        .map(|&source| beam_sig_ordinal(stumps, source))
        .collect::<Vec<_>>();
    format!(
        "stemsheadbuildersystem {page} system {} profile {} inspectProfile {} interline {} \
         bounds {} beamSigOrder {} beamXOrder {} headSigOrder {} headXOrder {} seeds {} \
         modeledRegistry {} maxStemThickness {} maxLineSectionDx {} maxStemAlignmentDx {} \
         maxStemAlignmentDy {} minCoreSectionLength {} minSideRatio {} gap0 {} gap1 {} gap2 {} \
         gap3 {} gap4 {}",
        system.system_id,
        system.system_profile,
        system.inspect_profile,
        system.interline,
        rectangle_token(reach.system_bounds),
        usize_list(&beam_sig_order),
        usize_list(&beam_x_order),
        usize_list(&reach.head_sig_ordinals),
        usize_list(&reach.head_sig_ordinals_by_abscissa),
        reach.kept_seed_ordinals.len(),
        registry_after_stumps(system),
        system.max_stem_thickness,
        java_hex_double(system.max_line_section_dx),
        java_hex_double(system.max_stem_alignment_dx),
        java_hex_double(system.max_stem_alignment_dy),
        system.minimum_core_section_length,
        java_hex_double(system.minimum_side_ratio),
        system.gap_map[&0],
        system.gap_map[&1],
        system.gap_map[&2],
        system.gap_map[&3],
        system.gap_map[&4],
    )
}

fn registry_after_stumps(system: &NativeStemsHeadBuilderSystem) -> usize {
    let first = system
        .registry_events
        .first()
        .expect("system registry chronology");
    let mut count = first.modeled_count_before;
    for event in &system.registry_events {
        if !matches!(
            event.occurrence,
            NativeStemsHeadBuilderRegistryOccurrence::PreBuilder { .. }
        ) {
            break;
        }
        count = event.modeled_count_after;
    }
    count
}

fn seed_duplicate_row(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadCornerReachabilitySystem,
) -> String {
    let mut counts = BTreeMap::<String, usize>::new();
    for &free_glyph_ordinal in &system.kept_seed_ordinals {
        *counts
            .entry(bounded_glyph_alias(&stem_seed_glyph(
                native,
                system.system_id,
                free_glyph_ordinal,
            )))
            .or_default() += 1;
    }
    let mut duplicate_keys = 0usize;
    let mut extra_occurrences = 0usize;
    let mut digest = Vec::new();
    for (alias, count) in counts {
        if count <= 1 {
            continue;
        }
        duplicate_keys += 1;
        extra_occurrences += count - 1;
        digest.extend_from_slice(format!("{alias}:{count}\n").as_bytes());
    }
    format!(
        "stemsheadbuilderseedduplicates {page} system {} keptSeeds {} \
         duplicateStructuralKeys {duplicate_keys} extraOccurrences {extra_occurrences} \
         duplicateSha256 {}",
        system.system_id,
        system.kept_seed_ordinals.len(),
        sha256_hex(&digest),
    )
}

fn head_row(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadCornerReachabilitySystem,
    head: &NativeStemsHeadReachabilityHead,
) -> String {
    let source = source_head(native, system.system_id, head.reference);
    let glyph = source_head_glyph(native, system.system_id, head.reference, source);
    format!(
        "stemsheadbuilderhead {page} system {} xOrdinal {} sigOrdinal {} staff {} shape {} \
         bounds {} center {}:{} grade {} vip {} glyph {} glyphWeight {} glyphRuns {}",
        system.system_id,
        head.x_ordinal,
        head.sig_ordinal,
        head.staff_id,
        head_shape_token(head.shape),
        rectangle_token(head.bounds),
        head.center.0,
        head.center.1,
        java_hex_double(head.grade),
        source.is_vip,
        bounded_glyph_alias(&glyph),
        glyph_weight(&glyph.run_table),
        glyph_run_count(&glyph.run_table),
    )
}

fn end_row(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    reach: &NativeStemsHeadCornerReachabilitySystem,
    builder: &NativeStemsHeadBuilder,
    registry: &registry_rows::RegistryEndFields,
) -> String {
    let seeds = builder
        .seeds_after_filter
        .iter()
        .map(|reference| match *reference {
            NativeStemsHeadBuilderGlyphRef::StemSeed { free_glyph_ordinal } => bounded_glyph_alias(
                &stem_seed_glyph(native, system.system_id, free_glyph_ordinal),
            ),
            other => panic!("{page}: non-seed retained as mutable seed {other:?}"),
        })
        .collect::<Vec<_>>();
    let before = b_arena_before(reach, builder.builder_ordinal);
    let corner = reach_corner(reach, builder.start);
    let anchors = corner
        .find_linkers
        .iter()
        .filter(|find| matches!(find.result, NativeStemsHeadFindResult::CreatedAnchor(_)))
        .count();

    assert_eq!(builder.filaments.len(), registry.filaments);
    assert_eq!(
        builder.sig_mutation_count, 0,
        "{page}: forbidden SIG mutation"
    );
    assert_eq!(
        builder.beam_arena_mutation_count, 0,
        "{page}: forbidden builder arena mutation"
    );
    format!(
        "stemsheadbuilderend {page} system {} builder {} seeds {} items {} lengths {} filaments {} glyphNew {} glyphReuse {} bArenaBefore {before} bArenaAfter {} anchorsCreated {anchors} sigVertexDelta 0 sigEdgeDelta 0 stemInterDelta 0 systemStemDelta {} linkMutations {} sbAssigned {} modeledRegistryBefore {} modeledRegistryAfter {} registryShaBefore {} registryShaAfter {} registrationActionSha256 {}",
        system.system_id,
        builder.builder_ordinal,
        string_list(&seeds),
        builder.items.len(),
        builder.lengths.len(),
        registry.filaments,
        registry.glyph_new,
        registry.glyph_reuse,
        before + anchors,
        builder.system_stem_mutation_count,
        builder.link_mutation_count,
        builder.c_builder_assignment == builder.start,
        registry.modeled_before,
        registry.modeled_after,
        registry.sha_before,
        registry.sha_after,
        registry.action_sha256,
    )
}

fn source_head(
    native: &NativeProjectionPage,
    system_id: usize,
    reference: NativeHeadStaffEpilogRef,
) -> &NativeHeadStaffEpilogHead {
    native
        .heads
        .epilog
        .staff_epilog
        .systems
        .iter()
        .find(|candidate| candidate.system_id == system_id)
        .and_then(|system| system.staffs.get(reference.staff_index))
        .and_then(|staff| staff.heads.get(reference.head_index))
        .expect("sparse source head")
}

fn source_head_glyph(
    native: &NativeProjectionPage,
    system_id: usize,
    reference: NativeHeadStaffEpilogRef,
    source: &NativeHeadStaffEpilogHead,
) -> BoundedGlyph {
    let staff_id = native
        .heads
        .epilog
        .staff_epilog
        .systems
        .iter()
        .find(|candidate| candidate.system_id == system_id)
        .and_then(|system| system.staffs.get(reference.staff_index))
        .map(|staff| staff.staff_id)
        .expect("sparse source head staff");
    let glyph = match source.origin {
        NativeHeadStaffEpilogOrigin::Seed(ordinal) => native
            .heads
            .seed_glyphs
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system_id)
            .and_then(|system| {
                system
                    .staffs
                    .iter()
                    .find(|staff| staff.staff_id == staff_id)
            })
            .and_then(|staff| {
                staff
                    .heads
                    .iter()
                    .find(|candidate| candidate.ordinal == ordinal)
            })
            .map(|candidate| &candidate.glyph),
        NativeHeadStaffEpilogOrigin::Range(ordinal) => native
            .heads
            .range_glyphs
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system_id)
            .and_then(|system| {
                system
                    .staffs
                    .iter()
                    .find(|staff| staff.staff_id == staff_id)
            })
            .and_then(|staff| {
                staff
                    .heads
                    .iter()
                    .find(|candidate| candidate.ordinal == ordinal)
            })
            .map(|candidate| &candidate.glyph),
    }
    .expect("sparse source head glyph");
    BoundedGlyph {
        bounds: BoundedBounds {
            x: glyph.glyph_bounds.x,
            y: glyph.glyph_bounds.y,
            width: glyph.glyph_bounds.width,
            height: glyph.glyph_bounds.height,
        },
        run_table: glyph.run_table.clone(),
    }
}

fn stem_seed_glyph(
    native: &NativeProjectionPage,
    system_id: usize,
    free_glyph_ordinal: usize,
) -> BoundedGlyph {
    let glyph = native
        .stem_seeds
        .systems
        .iter()
        .find(|candidate| candidate.raw.system_id == system_id)
        .and_then(|system| system.free_glyphs.get(free_glyph_ordinal))
        .expect("sparse stem seed glyph");
    BoundedGlyph {
        bounds: BoundedBounds {
            x: i32::try_from(glyph.bounds.x).expect("seed x"),
            y: i32::try_from(glyph.bounds.y).expect("seed y"),
            width: i32::try_from(glyph.bounds.width).expect("seed width"),
            height: i32::try_from(glyph.bounds.height).expect("seed height"),
        },
        run_table: glyph.run_table.clone(),
    }
}

fn b_arena_before(
    system: &NativeStemsHeadCornerReachabilitySystem,
    builder_ordinal: usize,
) -> usize {
    let baseline = system
        .after_beam_arenas
        .iter()
        .map(|arena| arena.entries.len())
        .sum::<usize>();
    baseline
        + system
            .c_inspection_order
            .iter()
            .take(builder_ordinal)
            .map(|reference| reach_corner(system, *reference))
            .flat_map(|corner| &corner.find_linkers)
            .filter(|find| matches!(find.result, NativeStemsHeadFindResult::CreatedAnchor(_)))
            .count()
}

fn reach_corner(
    system: &NativeStemsHeadCornerReachabilitySystem,
    reference: NativeStemsHeadCornerRef,
) -> &NativeStemsHeadReachabilityCorner {
    system
        .heads
        .iter()
        .find(|head| {
            head.reference == reference.head
                && head.sig_ordinal == reference.sig_ordinal
                && head.x_ordinal == reference.x_ordinal
        })
        .and_then(|head| {
            head.corners
                .iter()
                .find(|corner| corner.reference == reference)
        })
        .expect("sparse reachability corner")
}

fn head_shape_token(shape: HeadTemplateShape) -> &'static str {
    match shape {
        HeadTemplateShape::NoteheadBlack => "NOTEHEAD_BLACK",
        HeadTemplateShape::NoteheadVoid => "NOTEHEAD_VOID",
        HeadTemplateShape::WholeNote => "WHOLE_NOTE",
        HeadTemplateShape::Breve => "BREVE",
    }
}

fn glyph_weight(table: &RunTable) -> usize {
    (0..table.sequence_count())
        .flat_map(|ordinal| table.sequence(ordinal).unwrap_or_default())
        .map(|run| run.length)
        .sum()
}

fn usize_list(values: &[usize]) -> String {
    string_list(&values.iter().map(usize::to_string).collect::<Vec<_>>())
}
