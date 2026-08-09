// SPDX-License-Identifier: AGPL-3.0-or-later

//! Identity-normalized rows for one head-origin `StemBuilder`.
//!
//! Registry registrations and the atomic `stemsheadbuilderend` row live in the
//! outer projector.  This module owns the constructor's dense, read-only
//! evidence, including the reachability rows which Java emits immediately
//! before assigning the builder.

use super::*;

use std::cmp::Ordering;

use audiveris_omr::{
    beam_recognizer::run_table_center_line,
    native_heads_staff_epilog::NativeHeadStaffEpilogOrigin,
    native_stems_beam_stumps::NativeStemsBeamStumpRef,
    native_stems_head_builders::{
        NativeStemsHeadBuilderHeadPartsAction, NativeStemsHeadBuilderSeedAction,
        NativeStemsHeadBuilderSortEntry, NativeStemsHeadBuilderSystem,
    },
    native_stems_head_corner_reachability::{
        NativeStemsHeadCornerReachabilitySystem, NativeStemsHeadFindResult,
        NativeStemsHeadSeedScanAction, NativeStemsHeadStumpRef,
    },
};

/// Project every non-registry row owned by one C-origin builder, stopping at
/// `stemsheadbuilderlength`.  The caller inserts these rows in C inspection
/// order and owns the complete end row.
pub(super) fn dense_builder_rows(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    reach_system: &NativeStemsHeadCornerReachabilitySystem,
    builder: &NativeStemsHeadBuilder,
) -> Vec<String> {
    let stumps = native
        .beam_stumps
        .systems
        .iter()
        .find(|candidate| candidate.system_id == system.system_id)
        .expect("head-builder beam-stump system");
    let (_, corner) = reach_corner(reach_system, builder.start);
    assert_eq!(corner.inspection_ordinal, builder.builder_ordinal);

    let mut rows = Vec::new();
    append_seed_retrieval_rows(
        page,
        native,
        system,
        reach_system,
        builder,
        corner,
        &mut rows,
    );
    append_input_rows(
        page,
        native,
        system,
        reach_system,
        stumps,
        builder,
        corner,
        &mut rows,
    );
    rows.push(builder_row(
        page,
        native,
        system,
        reach_system,
        builder,
        corner,
    ));

    append_section_row(page, native, system, builder, true, &mut rows);
    append_section_row(page, native, system, builder, false, &mut rows);
    append_filament_rows(page, native, system, builder, &mut rows);

    append_alignment_rows(
        page,
        native,
        system,
        builder,
        NativeStemsHeadBuilderAlignmentSubject::Seeds,
        &mut rows,
    );
    append_target_filter_rows(page, native, system, stumps, builder, &mut rows);
    append_sort_rows(
        page,
        native,
        system,
        stumps,
        builder,
        "targets",
        &builder.target_sort,
        &mut rows,
    );
    rows.push(format!(
        "stemsheadbuilderlasthead {page} system {} builder {} lastHeadY - \
         pastSeedDrops 0 pastVSectionDrops 0 pastHSectionDrops 0",
        system.system_id, builder.builder_ordinal,
    ));
    append_seed_filter_rows(page, native, system, builder, &mut rows);
    append_chunk_duplicate_row(page, native, system, builder, &mut rows);
    append_head_parts_rows(page, native, system, reach_system, builder, &mut rows);
    append_alignment_rows(
        page,
        native,
        system,
        builder,
        NativeStemsHeadBuilderAlignmentSubject::Chunks,
        &mut rows,
    );
    append_chunk_filter_rows(page, native, system, builder, &mut rows);
    append_seed_item_rows(page, native, system, stumps, builder, &mut rows);
    append_sort_rows(
        page,
        native,
        system,
        stumps,
        builder,
        "items",
        &builder.sort,
        &mut rows,
    );
    append_item_pre_rows(page, native, system, stumps, builder, &mut rows);
    append_gap_item_length_rows(page, native, system, stumps, builder, &mut rows);
    rows
}

#[allow(clippy::too_many_arguments)]
fn append_seed_retrieval_rows(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    reach_system: &NativeStemsHeadCornerReachabilitySystem,
    builder: &NativeStemsHeadBuilder,
    corner: &NativeStemsHeadReachabilityCorner,
    rows: &mut Vec<String>,
) {
    for scan in &corner.seed_scans {
        let glyph = stem_seed_glyph(native, system.system_id, scan.free_glyph_ordinal);
        let contribution =
            y_overlap_rectangle(corner.y_range, bounds_rectangle(scan.bounds)).max(0);
        let distance = line_point_distance(
            corner.theoretical_line,
            (f64::from(scan.centroid.0), f64::from(scan.centroid.1)),
        );
        rows.push(format!(
            "stemsheadbuilderseedsource {page} system {} builder {} sourceOrdinal {} \
             systemSeedOrdinal {} glyph {} bounds {} contrib {contribution} \
             minContrib {} distance {} maxDistance {} action {}",
            system.system_id,
            builder.builder_ordinal,
            scan.neighbor_ordinal,
            system_seed_ordinal(reach_system, scan.free_glyph_ordinal),
            bounded_glyph_alias(&glyph),
            bounds_token(scan.bounds),
            reach_system.min_seed_contribution,
            java_hex_double(distance),
            java_hex_double(reach_system.max_line_seed_dx),
            seed_scan_action(scan.action),
        ));
    }

    let mut preliminary = corner
        .seed_scans
        .iter()
        .filter_map(|scan| scan.preliminary_ordinal.map(|ordinal| (ordinal, scan)))
        .collect::<Vec<_>>();
    preliminary.sort_by_key(|(ordinal, _)| *ordinal);
    rows.push(sort_audit_row(
        page,
        system.system_id,
        builder.builder_ordinal,
        "retrieveSeeds",
        preliminary.len(),
        0,
        0,
        &[],
    ));
    for (input, (_, scan)) in preliminary.iter().enumerate() {
        let glyph = stem_seed_glyph(native, system.system_id, scan.free_glyph_ordinal);
        rows.push(format!(
            "stemsheadbuilderseedsort {page} system {} builder {} input {input} output {} \
             sourceOrdinal {} systemSeedOrdinal {} glyph {} contrib {}",
            system.system_id,
            builder.builder_ordinal,
            scan.sorted_preliminary_ordinal
                .expect("sorted preliminary seed"),
            scan.neighbor_ordinal,
            system_seed_ordinal(reach_system, scan.free_glyph_ordinal),
            bounded_glyph_alias(&glyph),
            scan.contribution.expect("preliminary seed contribution"),
        ));
    }
    for decision in &corner.seed_overlap_decisions {
        let glyph = stem_seed_glyph(native, system.system_id, decision.free_glyph_ordinal);
        let conflict = decision.first_overlapping_kept_seed.map_or_else(
            || "-".to_owned(),
            |ordinal| bounded_glyph_alias(&stem_seed_glyph(native, system.system_id, ordinal)),
        );
        rows.push(format!(
            "stemsheadbuilderseedoverlap {page} system {} builder {} sortedOrdinal {} \
             glyph {} conflict {conflict} action {}",
            system.system_id,
            builder.builder_ordinal,
            decision.sorted_preliminary_ordinal,
            bounded_glyph_alias(&glyph),
            if decision.action == NativeStemsHeadSeedOverlapAction::Kept {
                "keep"
            } else {
                "remove"
            },
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn append_input_rows(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    _reach_system: &NativeStemsHeadCornerReachabilitySystem,
    stumps: &NativeStemsBeamStumpSystem,
    builder: &NativeStemsHeadBuilder,
    corner: &NativeStemsHeadReachabilityCorner,
    rows: &mut Vec<String>,
) {
    for (ordinal, &free_glyph_ordinal) in corner.assigned_seed_ordinals.iter().enumerate() {
        let glyph = stem_seed_glyph(native, system.system_id, free_glyph_ordinal);
        rows.push(format!(
            "stemsheadbuilderinputseed {page} system {} builder {} ordinal {ordinal} \
             glyph {} bounds {}",
            system.system_id,
            builder.builder_ordinal,
            bounded_glyph_alias(&glyph),
            bounded_bounds_token(glyph.bounds),
        ));
    }

    let mut find_ordinal = 0usize;
    for (ordinal, target) in corner.ordered_targets.iter().enumerate() {
        match target {
            NativeStemsHeadReachabilityTarget::Head(reference) => rows.push(format!(
                "stemsheadbuilderinputtarget {page} system {} builder {} ordinal {ordinal} \
                 kind C alias {} action existing",
                system.system_id,
                builder.builder_ordinal,
                native_c_alias(*reference),
            )),
            NativeStemsHeadReachabilityTarget::Beam(reference) => {
                let find = corner
                    .find_linkers
                    .get(find_ordinal)
                    .expect("beam target find-linker evidence");
                find_ordinal += 1;
                assert_eq!(find.result.reference(), *reference);
                rows.push(format!(
                    "stemsheadbuilderinputtarget {page} system {} builder {} ordinal {ordinal} \
                     kind B alias {} action {} beforeArena {} best {} bestDx {} cross {}",
                    system.system_id,
                    builder.builder_ordinal,
                    b_alias(stumps, *reference),
                    if matches!(find.result, NativeStemsHeadFindResult::Reused(_)) {
                        "reuse"
                    } else {
                        "createAnchor"
                    },
                    find.arena_before.len(),
                    find.selected_before_threshold
                        .map_or_else(|| "-".to_owned(), |value| b_alias(stumps, value)),
                    java_hex_double(find.best_dx),
                    point_token(find.cross),
                ));
            }
        }
    }
    assert_eq!(find_ordinal, corner.find_linkers.len());
}

fn builder_row(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    reach_system: &NativeStemsHeadCornerReachabilitySystem,
    builder: &NativeStemsHeadBuilder,
    corner: &NativeStemsHeadReachabilityCorner,
) -> String {
    let reference = builder.start;
    format!(
        "stemsheadbuilder {page} system {} builder {} head {} cornerOrdinal {} corner {} \
         alias {} hSide {} vSide {} profile {} cYDir {} builderYDir {} \
         directionDiverges {} ref {} theo {} luBounds {} startStump {} \
         modeledRegistryBefore {} bArenaBefore {}",
        system.system_id,
        builder.builder_ordinal,
        reference.x_ordinal,
        corner.inspection_ordinal % 4,
        corner_id(reference),
        native_c_alias(reference),
        head_side_token(reference.horizontal),
        vertical_side_token(reference.vertical),
        builder.max_stem_profile,
        builder.c_y_direction,
        builder.y_direction,
        builder.c_y_direction != builder.y_direction,
        point_token(corner.reference_point),
        line_token(builder.theoretical_line),
        rectangle_token(builder.lookup_bounds),
        builder.start_stump.map_or_else(
            || "-".to_owned(),
            |glyph| glyph_alias_ref(native, system, builder, glyph),
        ),
        modeled_registry_before(native, system.system_id, builder.builder_ordinal),
        b_arena_before(reach_system, builder.builder_ordinal),
    )
}

fn append_section_row(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    builder: &NativeStemsHeadBuilder,
    vertical: bool,
    rows: &mut Vec<String>,
) {
    let scans = if vertical {
        &builder.vertical_sections
    } else {
        &builder.horizontal_sections
    };
    let mut reasons = BTreeMap::<&'static str, usize>::new();
    let mut rejects = Vec::new();
    let mut accepted = scans
        .iter()
        .filter_map(|scan| {
            let action = section_action(scan, vertical);
            *reasons.entry(action).or_default() += 1;
            if scan.accepted {
                Some((
                    scan.accepted_sorted_ordinal
                        .expect("accepted section sorted ordinal"),
                    scan.source_ordinal,
                ))
            } else {
                rejects.extend_from_slice(
                    format!(
                        "{}:{}:{action}\n",
                        scan.source_ordinal,
                        bounds_token(scan.bounds)
                    )
                    .as_bytes(),
                );
                None
            }
        })
        .collect::<Vec<_>>();
    accepted.sort_by_key(|(ordinal, _)| *ordinal);
    let accepted = accepted
        .into_iter()
        .map(|(_, source)| source)
        .collect::<Vec<_>>();
    let reasons = reasons
        .iter()
        .map(|(reason, count)| format!("{reason}:{count}"))
        .collect::<Vec<_>>();
    rows.push(format!(
        "stemsheadbuilder{}section {page} system {} builder {} sourceCount {} \
         acceptedCount {} acceptedSourceOrdinals {} reasons {} rejectSha256 {}",
        if vertical { "v" } else { "h" },
        system.system_id,
        builder.builder_ordinal,
        scans.len(),
        accepted.len(),
        usize_list(&accepted),
        string_list(&reasons),
        sha256_hex(&rejects),
    ));
    let _ = native;
}

fn append_filament_rows(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    builder: &NativeStemsHeadBuilder,
    rows: &mut Vec<String>,
) {
    for filament in &builder.filaments {
        let mut aliases = Vec::new();
        for (ordinal, member) in filament.member_section_source_ordinals.iter().enumerate() {
            let (prefix, sources, sections) = match member.orientation {
                Orientation::Horizontal => (
                    "h",
                    &system.horizontal_section_source_ordinals,
                    &native.grid.peak_graph.horizontal_sections,
                ),
                Orientation::Vertical => (
                    "v",
                    &system.vertical_section_source_ordinals,
                    &native.grid.peak_graph.vertical_sections,
                ),
            };
            let source = *sources
                .get(member.source_ordinal)
                .expect("filament member source ordinal");
            let section = &sections[source];
            let alias = format!("{prefix}:{}", member.source_ordinal);
            aliases.push(alias.clone());
            rows.push(format!(
                "stemsheadbuilderfilamentmember {page} system {} builder {} filament {} \
                 ordinal {ordinal} alias {alias} orientation {} bounds {} weight {} runs {}",
                system.system_id,
                builder.builder_ordinal,
                filament.filament_ordinal,
                orientation_token(member.orientation),
                bounds_token(section.bounds()),
                section.weight(),
                section.run_count(),
            ));
        }
        // `StraightFilament.getCenterLine()` delegates to the same BasicLine
        // pixel regression as the fixed chunk glyph.  Refit the registered
        // run table so square filaments take Java's horizontal denominator
        // branch instead of blindly extending the vertical start/stop pair.
        let registration = builder
            .glyph_registrations
            .get(filament.filament_ordinal)
            .expect("filament glyph registration");
        let fitted = run_table_center_line(
            &registration.run_table,
            i32::try_from(registration.bounds.x).expect("filament x"),
            i32::try_from(registration.bounds.y).expect("filament y"),
        )
        .expect("filament center line");
        let center_line = NativeStemLine {
            start: NativeStemPoint {
                x: fitted.x1,
                y: fitted.y1,
            },
            stop: NativeStemPoint {
                x: fitted.x2,
                y: fitted.y2,
            },
        };
        rows.push(format!(
            "stemsheadbuilderfilament {page} system {} builder {} ordinal {} members {} \
             bounds {} weight {} centerLine {} meanThickness {} meanDistance {} length {}",
            system.system_id,
            builder.builder_ordinal,
            filament.filament_ordinal,
            string_list(&aliases),
            bounds_token(filament.bounds),
            filament.weight,
            line_token(center_line),
            java_hex_double(filament.mean_thickness),
            java_hex_double(filament.mean_distance),
            filament.bounds.height,
        ));
    }
}

fn append_alignment_rows(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    builder: &NativeStemsHeadBuilder,
    subject: NativeStemsHeadBuilderAlignmentSubject,
    rows: &mut Vec<String>,
) {
    let pass = builder
        .alignment
        .iter()
        .find(|pass| pass.subject == subject)
        .expect("head-builder alignment pass");
    let phase = match subject {
        NativeStemsHeadBuilderAlignmentSubject::Seeds => "seed",
        NativeStemsHeadBuilderAlignmentSubject::Chunks => "chunk",
    };
    let inputs = alignment_inputs(system, builder, subject);
    let promoted = promoted_occurrence(native, system, builder, &inputs);

    for (ordinal, check) in pass.comparisons.iter().enumerate() {
        rows.push(format!(
            "stemsheadbuilderalign {page} system {} builder {} phase {phase} ordinal {ordinal} \
             startStump {} promotedOccurrence {} firstOccurrence {} secondOccurrence {} \
             first {} second {} firstDeskew {} secondDeskew {} dy {} maxDy {} \
             dyBypass {} dx {} maxDx {} aligned {} selectedAlienOccurrence {} \
             actualRemovedOccurrence {} alien {} tieRemoveSecond {}",
            system.system_id,
            builder.builder_ordinal,
            builder.start_stump.map_or_else(
                || "-".to_owned(),
                |glyph| glyph_alias_ref(native, system, builder, glyph),
            ),
            promoted.clone().unwrap_or_else(|| "-".to_owned()),
            occurrence_alias(
                check.first,
                builder.start_stump,
                promoted.as_deref(),
                &inputs
            ),
            occurrence_alias(
                check.second,
                builder.start_stump,
                promoted.as_deref(),
                &inputs
            ),
            glyph_alias_ref(native, system, builder, check.first),
            glyph_alias_ref(native, system, builder, check.second),
            tuple_point_token(check.first_deskewed),
            tuple_point_token(check.second_deskewed),
            java_hex_double(check.dy),
            java_hex_double(system.max_stem_alignment_dy),
            check.dy_bypasses_dx,
            optional_double_token(check.dx),
            java_hex_double(system.max_stem_alignment_dx),
            check.aligned,
            check.selected_alien.map_or_else(
                || "-".to_owned(),
                |glyph| occurrence_alias(glyph, builder.start_stump, promoted.as_deref(), &inputs),
            ),
            check.actual_removed_occurrence.map_or_else(
                || "-".to_owned(),
                |glyph| occurrence_alias(glyph, builder.start_stump, promoted.as_deref(), &inputs),
            ),
            check.selected_alien.map_or_else(
                || "-".to_owned(),
                |glyph| glyph_alias_ref(native, system, builder, glyph),
            ),
            check.equal_height_removed_second,
        ));
    }

    let removed = pass
        .removed_structural_keys
        .iter()
        .map(|&glyph| glyph_alias_ref(native, system, builder, glyph))
        .collect::<Vec<_>>();
    let retained = pass
        .retained_occurrences
        .iter()
        .map(|&glyph| occurrence_alias(glyph, builder.start_stump, promoted.as_deref(), &inputs))
        .collect::<Vec<_>>();
    rows.push(format!(
        "stemsheadbuilderalignresult {page} system {} builder {} phase {phase} \
         removedStructuralKeys {} retainedOccurrences {}",
        system.system_id,
        builder.builder_ordinal,
        string_list(&removed),
        string_list(&retained),
    ));
}

fn alignment_inputs(
    system: &NativeStemsHeadBuilderSystem,
    builder: &NativeStemsHeadBuilder,
    subject: NativeStemsHeadBuilderAlignmentSubject,
) -> Vec<(NativeStemsHeadBuilderGlyphRef, String)> {
    match subject {
        NativeStemsHeadBuilderAlignmentSubject::Seeds => builder
            .input_seed_ordinals
            .iter()
            .enumerate()
            .map(|(ordinal, &free_glyph_ordinal)| {
                (
                    NativeStemsHeadBuilderGlyphRef::StemSeed { free_glyph_ordinal },
                    format!("seedInput:{ordinal}"),
                )
            })
            .collect(),
        NativeStemsHeadBuilderAlignmentSubject::Chunks => builder
            .chunks
            .iter()
            .filter(|chunk| {
                !matches!(
                    chunk.action,
                    NativeStemsHeadBuilderChunkAction::SeedStructural
                        | NativeStemsHeadBuilderChunkAction::HeadPartsVipOnly
                )
            })
            .map(|chunk| {
                (
                    chunk.glyph,
                    chunk_event_alias(system.system_id, chunk.glyph),
                )
            })
            .collect(),
    }
}

fn promoted_occurrence(
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    builder: &NativeStemsHeadBuilder,
    inputs: &[(NativeStemsHeadBuilderGlyphRef, String)],
) -> Option<String> {
    let start_ref = builder.start_stump?;
    let start = glyph_data(native, system, builder, start_ref);
    let mut ordered = inputs.iter().collect::<Vec<_>>();
    ordered.sort_by(|(left, _), (right, _)| {
        glyph_order(
            &glyph_data(native, system, builder, *left),
            &glyph_data(native, system, builder, *right),
            builder.y_direction,
        )
    });
    Some(
        ordered
            .into_iter()
            .find(|(glyph, _)| same_glyph(&start, &glyph_data(native, system, builder, *glyph)))
            .map_or_else(
                || "startStumpSynthetic".to_owned(),
                |(_, alias)| alias.clone(),
            ),
    )
}

fn occurrence_alias(
    glyph: NativeStemsHeadBuilderGlyphRef,
    start: Option<NativeStemsHeadBuilderGlyphRef>,
    promoted: Option<&str>,
    inputs: &[(NativeStemsHeadBuilderGlyphRef, String)],
) -> String {
    if start == Some(glyph) {
        return promoted.unwrap_or("startStumpSynthetic").to_owned();
    }
    inputs
        .iter()
        .find(|(candidate, _)| *candidate == glyph)
        .map(|(_, alias)| alias.clone())
        .unwrap_or_else(|| panic!("missing occurrence alias for {glyph:?}"))
}

fn append_target_filter_rows(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    stumps: &NativeStemsBeamStumpSystem,
    builder: &NativeStemsHeadBuilder,
    rows: &mut Vec<String>,
) {
    for (ordinal, decision) in builder.target_filter.iter().enumerate() {
        let (kind, alias) = target_kind_alias(stumps, decision.target);
        rows.push(format!(
            "stemsheadbuildertargetfilter {page} system {} builder {} ordinal {ordinal} \
             kind {kind} alias {alias} stump {} removedByStructuralSeed {} action {}",
            system.system_id,
            builder.builder_ordinal,
            decision.stump.map_or_else(
                || "-".to_owned(),
                |glyph| glyph_alias_ref(native, system, builder, glyph),
            ),
            decision.removed_by_structural_seed,
            if decision.included { "keep" } else { "remove" },
        ));
    }
}

fn append_seed_filter_rows(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    builder: &NativeStemsHeadBuilder,
    rows: &mut Vec<String>,
) {
    let seed_pass = builder
        .alignment
        .iter()
        .find(|pass| pass.subject == NativeStemsHeadBuilderAlignmentSubject::Seeds)
        .expect("seed alignment pass");
    let removed = seed_pass
        .removed_structural_keys
        .iter()
        .map(|&glyph| glyph_data(native, system, builder, glyph))
        .collect::<Vec<_>>();
    for (ordinal, &free_glyph_ordinal) in builder.input_seed_ordinals.iter().enumerate() {
        let reference = NativeStemsHeadBuilderGlyphRef::StemSeed { free_glyph_ordinal };
        let glyph = glyph_data(native, system, builder, reference);
        rows.push(format!(
            "stemsheadbuilderseedfilter {page} system {} builder {} ordinal {ordinal} glyph {} \
             removedStructural {} retainedIdentity {} pastLastHead false",
            system.system_id,
            builder.builder_ordinal,
            bounded_glyph_alias(&glyph),
            removed
                .iter()
                .any(|candidate| same_glyph(candidate, &glyph)),
            builder.seeds_after_filter.contains(&reference),
        ));
    }
}

fn append_chunk_duplicate_row(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    builder: &NativeStemsHeadBuilder,
    rows: &mut Vec<String>,
) {
    let mut counts = BTreeMap::<String, usize>::new();
    for chunk in &builder.chunks {
        *counts
            .entry(glyph_alias_ref(native, system, builder, chunk.glyph))
            .or_default() += 1;
    }
    let mut keys = 0usize;
    let mut extra = 0usize;
    let mut digest = Vec::new();
    for (alias, count) in counts {
        if count <= 1 {
            continue;
        }
        keys += 1;
        extra += count - 1;
        digest.extend_from_slice(format!("{alias}:{count}\n").as_bytes());
    }
    rows.push(format!(
        "stemsheadbuilderchunkduplicates {page} system {} builder {} attempts {} \
         duplicateStructuralKeys {keys} extraOccurrences {extra} duplicateSha256 {}",
        system.system_id,
        builder.builder_ordinal,
        builder.chunks.len(),
        sha256_hex(&digest),
    ));
}

fn append_head_parts_rows(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    reach_system: &NativeStemsHeadCornerReachabilitySystem,
    builder: &NativeStemsHeadBuilder,
    rows: &mut Vec<String>,
) {
    let (head, _) = reach_corner(reach_system, builder.start);
    let head_glyph = source_head_glyph(native, system.system_id, head.reference);
    for chunk in builder
        .chunks
        .iter()
        .filter(|chunk| chunk.action != NativeStemsHeadBuilderChunkAction::SeedStructural)
    {
        let action = match chunk.head_parts_action {
            NativeStemsHeadBuilderHeadPartsAction::Keep => "keep",
            NativeStemsHeadBuilderHeadPartsAction::KeepNonVipJavaBehavior => "keepNonVipBug",
            NativeStemsHeadBuilderHeadPartsAction::RemoveVipOnly => "removeVipOnly",
        };
        rows.push(format!(
            "stemsheadbuilderheadparts {page} system {} builder {} event {} glyph {} head {} \
             yOverlap {} weight {} removed {} remain {} threshold 15 vip {} lowRemain {} \
             action {action}",
            system.system_id,
            builder.builder_ordinal,
            chunk_event_alias(system.system_id, chunk.glyph),
            glyph_alias_ref(native, system, builder, chunk.glyph),
            bounded_glyph_alias(&head_glyph),
            chunk.head_y_overlap,
            glyph_weight(&chunk.run_table),
            chunk.head_pixels_removed,
            chunk.remaining_weight,
            builder.source_is_vip,
            chunk.remaining_weight < 15,
        ));
    }
}

fn append_chunk_filter_rows(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    builder: &NativeStemsHeadBuilder,
    rows: &mut Vec<String>,
) {
    for (input_ordinal, chunk) in builder.chunks.iter().enumerate() {
        let action = match chunk.action {
            NativeStemsHeadBuilderChunkAction::Keep => "keep",
            NativeStemsHeadBuilderChunkAction::SeedStructural => "seedStructural",
            NativeStemsHeadBuilderChunkAction::HeadPartsVipOnly => "headParts",
            NativeStemsHeadBuilderChunkAction::UnalignedStructural => "unalignedStructural",
            NativeStemsHeadBuilderChunkAction::StartFirstStructural => "startFirstStructural",
        };
        rows.push(format!(
            "stemsheadbuilderchunkfilter {page} system {} builder {} inputOrdinal {input_ordinal} \
             event {} glyph {} finalOrdinal {} action {action}",
            system.system_id,
            builder.builder_ordinal,
            chunk_event_alias(system.system_id, chunk.glyph),
            glyph_alias_ref(native, system, builder, chunk.glyph),
            chunk.final_ordinal.map_or(-1, |ordinal| ordinal as i32),
        ));
    }
}

fn append_seed_item_rows(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    stumps: &NativeStemsBeamStumpSystem,
    builder: &NativeStemsHeadBuilder,
    rows: &mut Vec<String>,
) {
    let mut ordinal = 0usize;
    for decision in &builder.seed_filter {
        if decision.action == NativeStemsHeadBuilderSeedAction::AlignmentRemoved {
            continue;
        }
        let glyph = glyph_data(native, system, builder, decision.glyph);
        let duplicate_target =
            if decision.action == NativeStemsHeadBuilderSeedAction::DuplicateTargetIdentity {
                let mut sorted_targets = builder.target_sort.iter().collect::<Vec<_>>();
                sorted_targets.sort_by_key(|entry| entry.after_ordinal);
                sorted_targets
                    .into_iter()
                    .find(|entry| {
                        entry.item.glyph.is_some_and(|stump| {
                            same_glyph(&glyph, &glyph_data(native, system, builder, stump))
                        })
                    })
                    .map(|entry| item_alias(native, system, stumps, builder, &entry.item))
            } else {
                None
            };
        let overlap = decision.action == NativeStemsHeadBuilderSeedAction::OverlapsStart;
        let action = match decision.action {
            NativeStemsHeadBuilderSeedAction::AlignmentRemoved => unreachable!(),
            NativeStemsHeadBuilderSeedAction::DuplicateTargetIdentity => "duplicateTargetIdentity",
            NativeStemsHeadBuilderSeedAction::OverlapsStart => "startYOverlap",
            NativeStemsHeadBuilderSeedAction::ZeroContribution => "zeroContrib",
            NativeStemsHeadBuilderSeedAction::Item => "keep",
        };
        rows.push(format!(
            "stemsheadbuilderseeditemfilter {page} system {} builder {} ordinal {ordinal} \
             glyph {} duplicateTargetIdentity {} duplicateAlias {} startYOverlap {} \
             contrib {} action {action}",
            system.system_id,
            builder.builder_ordinal,
            bounded_glyph_alias(&glyph),
            duplicate_target.is_some(),
            duplicate_target.clone().unwrap_or_else(|| "-".to_owned()),
            if duplicate_target.is_some() {
                false
            } else {
                overlap
            },
            if duplicate_target.is_some() || overlap {
                -1
            } else {
                decision.contribution.expect("seed-item contribution")
            },
        ));
        ordinal += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn append_sort_rows(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    stumps: &NativeStemsBeamStumpSystem,
    builder: &NativeStemsHeadBuilder,
    phase: &str,
    permutation: &[NativeStemsHeadBuilderSortEntry],
    rows: &mut Vec<String>,
) {
    let mut entries = permutation.iter().collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.before_ordinal);
    let (cycles, equivalence, offenders) = comparator_audit(&entries, builder.y_direction);
    rows.push(sort_audit_row(
        page,
        system.system_id,
        builder.builder_ordinal,
        phase,
        entries.len(),
        cycles,
        equivalence,
        &offenders,
    ));
    for entry in entries {
        let item = &entry.item;
        let mut equal_inputs = permutation
            .iter()
            .filter(|other| {
                other.before_ordinal != entry.before_ordinal
                    && item_compare(&other.item, item, builder.y_direction) == 0
            })
            .map(|other| other.before_ordinal)
            .collect::<Vec<_>>();
        equal_inputs.sort_unstable();
        let predecessors = equal_inputs
            .iter()
            .copied()
            .filter(|other| *other < entry.before_ordinal)
            .collect::<Vec<_>>();
        rows.push(format!(
            "stemsheadbuildersort {page} system {} builder {} phase {phase} input {} output {} \
             alias {} kind {} line {} ref {} key1 {} key2 {} equalInputs {} \
             stableEqualPredecessors {}",
            system.system_id,
            builder.builder_ordinal,
            entry.before_ordinal,
            entry.after_ordinal,
            item_alias(native, system, stumps, builder, item),
            item_kind(item.kind),
            line_token(item.line),
            item_linker_reference(native, system.system_id, item)
                .map_or_else(|| "-".to_owned(), point_token),
            java_hex_double(item.line.start.y),
            java_hex_double(item.line.stop.y),
            usize_list(&equal_inputs),
            usize_list(&predecessors),
        ));
    }
}

fn append_item_pre_rows(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    stumps: &NativeStemsBeamStumpSystem,
    builder: &NativeStemsHeadBuilder,
    rows: &mut Vec<String>,
) {
    let start = builder
        .items_before_sort
        .first()
        .expect("head-builder start item");
    append_item_pre_row(page, native, system, stumps, builder, 0, 0, start, rows);
    let mut sorted = builder.sort.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|entry| entry.after_ordinal);
    for entry in sorted {
        append_item_pre_row(
            page,
            native,
            system,
            stumps,
            builder,
            entry.before_ordinal + 1,
            entry.after_ordinal + 1,
            &entry.item,
            rows,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn append_item_pre_row(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    stumps: &NativeStemsBeamStumpSystem,
    builder: &NativeStemsHeadBuilder,
    creation: usize,
    sorted: usize,
    item: &NativeStemsHeadBuilderItem,
    rows: &mut Vec<String>,
) {
    rows.push(format!(
        "stemsheadbuilderitempre {page} system {} builder {} creationOrdinal {creation} \
         sortedOrdinal {sorted} kind {} alias {} line {} glyph {} contrib {}",
        system.system_id,
        builder.builder_ordinal,
        item_kind(item.kind),
        item_alias(native, system, stumps, builder, item),
        line_token(item.line),
        item.glyph.map_or_else(
            || "-".to_owned(),
            |glyph| glyph_alias_ref(native, system, builder, glyph),
        ),
        item.contribution,
    ));
}

fn append_gap_item_length_rows(
    page: &str,
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    stumps: &NativeStemsBeamStumpSystem,
    builder: &NativeStemsHeadBuilder,
    rows: &mut Vec<String>,
) {
    // `NativeStemsHeadBuilder::gaps` records only comparisons after the first
    // item.  Replaying the sorted creation stream is what preserves Java's
    // required ordinal-zero `action initial` row as well.
    let sorted = sorted_pre_items(builder);
    let max_gap = system.gap_map[&builder.max_stem_profile];
    let mut last = None::<NativeStemPoint>;
    for (ordinal, item) in sorted.iter().enumerate() {
        let start = if builder.y_direction > 0 {
            item.line.start
        } else {
            item.line.stop
        };
        let stop = if builder.y_direction > 0 {
            item.line.stop
        } else {
            item.line.start
        };
        let gap = last.map(|prior| f64::from(builder.y_direction) * (start.y - prior.y));
        let action = if last.is_none() {
            "initial"
        } else if gap.is_some_and(|value| value > f64::from(max_gap)) {
            "truncate"
        } else if gap.is_some_and(|value| value > 0.01) {
            "insert"
        } else {
            "contiguous"
        };
        let inserted = (action == "insert").then(|| {
            if builder.y_direction > 0 {
                NativeStemLine {
                    start: last.expect("inserted gap prior point"),
                    stop: start,
                }
            } else {
                NativeStemLine {
                    start,
                    stop: last.expect("inserted gap prior point"),
                }
            }
        });
        rows.push(format!(
            "stemsheadbuildergap {page} system {} builder {} ordinal {ordinal} \
             itemIndex {ordinal} itemAlias {} start {} stop {} lastBefore {} gap {} \
             maxGap {max_gap} epsilon {} action {action} insertedLine {} insertedContrib {}",
            system.system_id,
            builder.builder_ordinal,
            item_alias(native, system, stumps, builder, item),
            point_token(start),
            point_token(stop),
            last.map_or_else(|| "-".to_owned(), point_token),
            optional_double_token(gap),
            java_hex_double(0.01),
            inserted.map_or_else(|| "-".to_owned(), line_token),
            inserted.map_or_else(
                || "-".to_owned(),
                |line| line_bounds(line).height.to_string(),
            ),
        ));
        if action == "truncate" {
            break;
        }
        if last.is_none_or(|prior| f64::from(builder.y_direction) * (stop.y - prior.y) > 0.01) {
            last = Some(stop);
        }
    }

    for (ordinal, item) in builder.items.iter().enumerate() {
        let alias = if item.kind == NativeStemsHeadBuilderItemKind::Gap {
            format!("gap:{ordinal}")
        } else {
            item_alias(native, system, stumps, builder, item)
        };
        rows.push(format!(
            "stemsheadbuilderitem {page} system {} builder {} ordinal {ordinal} kind {} \
             alias {alias} line {} glyph {} contrib {}",
            system.system_id,
            builder.builder_ordinal,
            item_kind(item.kind),
            line_token(item.line),
            item.glyph.map_or_else(
                || "-".to_owned(),
                |glyph| glyph_alias_ref(native, system, builder, glyph),
            ),
            item.contribution,
        ));
    }
    for profile in 0..=4 {
        let length = builder.lengths[&profile];
        rows.push(format!(
            "stemsheadbuilderlength {page} system {} builder {} profile {profile} \
             threshold {} length {length} replayLength {length}",
            system.system_id, builder.builder_ordinal, system.gap_map[&profile],
        ));
    }
}

fn sorted_pre_items(builder: &NativeStemsHeadBuilder) -> Vec<NativeStemsHeadBuilderItem> {
    let mut result = vec![builder.items_before_sort[0].clone()];
    let mut tail = builder.sort.iter().collect::<Vec<_>>();
    tail.sort_by_key(|entry| entry.after_ordinal);
    result.extend(tail.into_iter().map(|entry| entry.item.clone()));
    result
}

fn comparator_audit(
    entries: &[&NativeStemsHeadBuilderSortEntry],
    y_direction: i32,
) -> (usize, usize, Vec<u8>) {
    let mut cycles = 0usize;
    let mut equivalence = 0usize;
    let mut offenders = Vec::new();
    for i in 0..entries.len() {
        for j in i + 1..entries.len() {
            for k in j + 1..entries.len() {
                let ij = item_compare(&entries[i].item, &entries[j].item, y_direction);
                let jk = item_compare(&entries[j].item, &entries[k].item, y_direction);
                let ki = item_compare(&entries[k].item, &entries[i].item, y_direction);
                let ik = item_compare(&entries[i].item, &entries[k].item, y_direction);
                let ji = -ij;
                let kj = -jk;
                let cycle = (ij < 0 && jk < 0 && ki < 0) || (ij > 0 && jk > 0 && ki > 0);
                let inconsistent = (ij == 0 && (ik != jk || -ik != ki))
                    || (jk == 0 && (ji != ki || ij != ik))
                    || (ik == 0 && (ij != kj || ji != jk));
                if cycle || inconsistent {
                    offenders.extend_from_slice(
                        format!("{i}:{j}:{k}:{ij}:{jk}:{ki}:{ik}:{cycle}:{inconsistent}\n")
                            .as_bytes(),
                    );
                }
                cycles += usize::from(cycle);
                equivalence += usize::from(inconsistent);
            }
        }
    }
    (cycles, equivalence, offenders)
}

#[allow(clippy::too_many_arguments)]
fn sort_audit_row(
    page: &str,
    system_id: usize,
    builder_ordinal: usize,
    phase: &str,
    items: usize,
    cycles: usize,
    equivalence: usize,
    offenders: &[u8],
) -> String {
    format!(
        "stemsheadbuildersortaudit {page} system {system_id} builder {builder_ordinal} \
         phase {phase} items {items} strictCycles {cycles} equivalenceInconsistencies \
         {equivalence} offenderSha256 {} jdk25MiniTimSort {}",
        sha256_hex(offenders),
        items < 32,
    )
}

fn item_compare(
    left: &NativeStemsHeadBuilderItem,
    right: &NativeStemsHeadBuilderItem,
    y_direction: i32,
) -> i32 {
    let is_half = |kind| {
        matches!(
            kind,
            NativeStemsHeadBuilderItemKind::StartHeadHalfLinker
                | NativeStemsHeadBuilderItemKind::HeadHalfLinker
        )
    };
    if is_half(left.kind) && is_half(right.kind) {
        return y_direction
            * java_double_compare(
                left.reference_point.expect("left half-linker reference").y,
                right
                    .reference_point
                    .expect("right half-linker reference")
                    .y,
            );
    }
    if y_direction > 0 {
        java_double_compare(left.line.start.y, right.line.start.y)
    } else {
        java_double_compare(right.line.stop.y, left.line.stop.y)
    }
}

fn item_kind(kind: NativeStemsHeadBuilderItemKind) -> &'static str {
    match kind {
        NativeStemsHeadBuilderItemKind::StartHeadHalfLinker => "startC",
        NativeStemsHeadBuilderItemKind::HeadHalfLinker => "C",
        NativeStemsHeadBuilderItemKind::BeamLinker => "B",
        NativeStemsHeadBuilderItemKind::SeedGlyph => "seed",
        NativeStemsHeadBuilderItemKind::ChunkGlyph => "chunk",
        NativeStemsHeadBuilderItemKind::Gap => "gap",
    }
}

fn item_alias(
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    stumps: &NativeStemsBeamStumpSystem,
    builder: &NativeStemsHeadBuilder,
    item: &NativeStemsHeadBuilderItem,
) -> String {
    if item.kind == NativeStemsHeadBuilderItemKind::StartHeadHalfLinker {
        return native_c_alias(builder.start);
    }
    if let Some(target) = item.target {
        return target_kind_alias(stumps, target).1;
    }
    match item.glyph.expect("glyph item") {
        NativeStemsHeadBuilderGlyphRef::StemSeed { free_glyph_ordinal } => {
            let ordinal = builder
                .input_seed_ordinals
                .iter()
                .position(|candidate| *candidate == free_glyph_ordinal)
                .expect("seed item input occurrence");
            format!("seedInput:{ordinal}")
        }
        glyph @ NativeStemsHeadBuilderGlyphRef::Chunk { .. } => {
            chunk_event_alias(system.system_id, glyph)
        }
        glyph => glyph_alias_ref(native, system, builder, glyph),
    }
}

fn item_linker_reference(
    native: &NativeProjectionPage,
    system_id: usize,
    item: &NativeStemsHeadBuilderItem,
) -> Option<NativeStemPoint> {
    let target = item.target?;
    let reach_system = native
        .head_reachability
        .systems
        .iter()
        .find(|candidate| candidate.system_id == system_id)
        .expect("item reachability system");
    match target {
        NativeStemsHeadBuilderTargetRef::Head(reference) => {
            Some(reach_corner(reach_system, reference).1.reference_point)
        }
        NativeStemsHeadBuilderTargetRef::Beam(reference) => reach_system
            .final_beam_arenas
            .iter()
            .find(|arena| arena.beam == reference.beam)
            .and_then(|arena| {
                arena
                    .entries
                    .iter()
                    .find(|entry| entry.reference == reference)
            })
            .map(|entry| entry.reference_point),
    }
}

fn glyph_alias_ref(
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    builder: &NativeStemsHeadBuilder,
    reference: NativeStemsHeadBuilderGlyphRef,
) -> String {
    bounded_glyph_alias(&glyph_data(native, system, builder, reference))
}

fn glyph_data(
    native: &NativeProjectionPage,
    system: &NativeStemsHeadBuilderSystem,
    builder: &NativeStemsHeadBuilder,
    reference: NativeStemsHeadBuilderGlyphRef,
) -> BoundedGlyph {
    match reference {
        NativeStemsHeadBuilderGlyphRef::StemSeed { free_glyph_ordinal } => {
            stem_seed_glyph(native, system.system_id, free_glyph_ordinal)
        }
        NativeStemsHeadBuilderGlyphRef::HeadStump { corner } => {
            let reach_system = native
                .head_reachability
                .systems
                .iter()
                .find(|candidate| candidate.system_id == system.system_id)
                .expect("head-stump reachability system");
            let (_, reach_corner) = reach_corner(reach_system, corner);
            match reach_corner
                .stump
                .as_ref()
                .expect("head-stump glyph reference")
                .source
            {
                NativeStemsHeadStumpRef::Seed { free_glyph_ordinal } => {
                    stem_seed_glyph(native, system.system_id, free_glyph_ordinal)
                }
                NativeStemsHeadStumpRef::Built {
                    head_x_ordinal,
                    constructor_ordinal,
                } => {
                    let stump_system = native
                        .head_stumps
                        .systems
                        .iter()
                        .find(|candidate| candidate.system_id == system.system_id)
                        .expect("head-stump system");
                    let candidate = stump_system
                        .heads_by_abscissa
                        .iter()
                        .find(|head| {
                            head.x_ordinal == head_x_ordinal
                                && head.sig_ordinal == corner.sig_ordinal
                        })
                        .and_then(|head| {
                            head.corners_in_constructor_order.iter().find(|candidate| {
                                candidate.constructor_ordinal == constructor_ordinal
                            })
                        })
                        .and_then(|corner| corner.build.as_ref())
                        .and_then(|build| build.candidate.as_ref())
                        .expect("built head-stump candidate");
                    bounded_glyph(candidate.bounds, candidate.run_table.clone())
                }
            }
        }
        NativeStemsHeadBuilderGlyphRef::BeamStump { b_linker } => {
            let v_system = native
                .beam_vlinkers
                .systems
                .iter()
                .find(|candidate| candidate.system_id == system.system_id)
                .expect("beam V-linker system");
            let stump = v_system
                .constructors
                .iter()
                .find(|constructor| constructor.source == b_linker.beam)
                .and_then(|constructor| {
                    constructor
                        .b_linkers
                        .iter()
                        .find(|candidate| candidate.reference == b_linker)
                })
                .and_then(|candidate| candidate.stump.as_ref())
                .expect("beam-stump glyph reference");
            match stump {
                NativeStemsBeamStumpRef::Seed {
                    free_glyph_ordinal, ..
                } => stem_seed_glyph(native, system.system_id, *free_glyph_ordinal),
                NativeStemsBeamStumpRef::Built {
                    canonical_glyph_index,
                } => {
                    let stump_system = native
                        .beam_stumps
                        .systems
                        .iter()
                        .find(|candidate| candidate.system_id == system.system_id)
                        .expect("beam-stump system");
                    let candidate = stump_system
                        .beams_by_abscissa
                        .iter()
                        .find(|beam| beam.source == b_linker.beam)
                        .and_then(|beam| {
                            beam.sides
                                .iter()
                                .filter_map(|side| side.build.as_ref())
                                .find(|build| {
                                    build.canonical_glyph_index == Some(*canonical_glyph_index)
                                })
                        })
                        .and_then(|build| build.candidate.as_ref())
                        .expect("built beam-stump candidate");
                    bounded_glyph(candidate.bounds, candidate.run_table.clone())
                }
            }
        }
        NativeStemsHeadBuilderGlyphRef::Chunk {
            builder_ordinal,
            filament_ordinal,
        } => {
            let source_builder = system
                .builders
                .iter()
                .find(|candidate| candidate.builder_ordinal == builder_ordinal)
                .unwrap_or(builder);
            let registration = source_builder
                .glyph_registrations
                .get(filament_ordinal)
                .expect("head-builder chunk registration");
            bounded_glyph(registration.bounds, registration.run_table.clone())
        }
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
        .expect("head-builder stem seed");
    bounded_glyph(glyph.bounds, glyph.run_table.clone())
}

fn source_head_glyph(
    native: &NativeProjectionPage,
    system_id: usize,
    reference: audiveris_omr::native_heads_staff_epilog::NativeHeadStaffEpilogRef,
) -> BoundedGlyph {
    let epilog = native
        .heads
        .epilog
        .staff_epilog
        .systems
        .iter()
        .find(|candidate| candidate.system_id == system_id)
        .expect("head-builder head epilog system");
    let staff = epilog
        .staffs
        .get(reference.staff_index)
        .expect("head-builder head staff");
    let head = staff
        .heads
        .get(reference.head_index)
        .expect("head-builder source head");
    let glyph = match head.origin {
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
                    .find(|candidate| candidate.staff_id == staff.staff_id)
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
                    .find(|candidate| candidate.staff_id == staff.staff_id)
            })
            .and_then(|staff| {
                staff
                    .heads
                    .iter()
                    .find(|candidate| candidate.ordinal == ordinal)
            })
            .map(|candidate| &candidate.glyph),
    }
    .expect("head-builder source head glyph");
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

fn bounded_glyph(bounds: Bounds, run_table: RunTable) -> BoundedGlyph {
    BoundedGlyph {
        bounds: BoundedBounds {
            x: i32::try_from(bounds.x).expect("glyph x"),
            y: i32::try_from(bounds.y).expect("glyph y"),
            width: i32::try_from(bounds.width).expect("glyph width"),
            height: i32::try_from(bounds.height).expect("glyph height"),
        },
        run_table,
    }
}

fn same_glyph(left: &BoundedGlyph, right: &BoundedGlyph) -> bool {
    left == right
}

fn glyph_order(left: &BoundedGlyph, right: &BoundedGlyph, y_direction: i32) -> Ordering {
    if y_direction > 0 {
        left.bounds.y.cmp(&right.bounds.y)
    } else {
        (right.bounds.y + right.bounds.height).cmp(&(left.bounds.y + left.bounds.height))
    }
}

fn glyph_weight(table: &RunTable) -> usize {
    (0..table.sequence_count())
        .flat_map(|ordinal| table.sequence(ordinal).unwrap_or_default())
        .map(|run| run.length)
        .sum()
}

fn reach_corner(
    system: &NativeStemsHeadCornerReachabilitySystem,
    reference: NativeStemsHeadCornerRef,
) -> (
    &audiveris_omr::native_stems_head_corner_reachability::NativeStemsHeadReachabilityHead,
    &NativeStemsHeadReachabilityCorner,
) {
    let head = system
        .heads
        .iter()
        .find(|head| {
            head.reference == reference.head
                && head.sig_ordinal == reference.sig_ordinal
                && head.x_ordinal == reference.x_ordinal
        })
        .expect("head-builder reachability head");
    let corner = head
        .corners
        .iter()
        .find(|corner| corner.reference == reference)
        .expect("head-builder reachability corner");
    (head, corner)
}

fn system_seed_ordinal(
    system: &NativeStemsHeadCornerReachabilitySystem,
    free_glyph_ordinal: usize,
) -> usize {
    system
        .kept_seed_ordinals
        .iter()
        .position(|candidate| *candidate == free_glyph_ordinal)
        .expect("system seed ordinal")
}

fn modeled_registry_before(
    native: &NativeProjectionPage,
    system_id: usize,
    builder_ordinal: usize,
) -> usize {
    let first = native
        .head_builders
        .systems
        .iter()
        .flat_map(|system| &system.registry_events)
        .next()
        .map_or(0, |event| event.modeled_count_before);
    let mut count = first;
    for system in &native.head_builders.systems {
        for event in &system.registry_events {
            if system.system_id == system_id
                && matches!(
                    event.occurrence,
                    NativeStemsHeadBuilderRegistryOccurrence::HeadChunk {
                        builder_ordinal: event_builder,
                        ..
                    } if event_builder >= builder_ordinal
                )
            {
                return event.modeled_count_before;
            }
            count = event.modeled_count_after;
        }
        if system.system_id == system_id {
            return count;
        }
    }
    panic!("missing registry system {system_id}")
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
            .map(|reference| reach_corner(system, *reference).1)
            .flat_map(|corner| &corner.find_linkers)
            .filter(|find| matches!(find.result, NativeStemsHeadFindResult::CreatedAnchor(_)))
            .count()
}

fn target_kind_alias(
    stumps: &NativeStemsBeamStumpSystem,
    target: NativeStemsHeadBuilderTargetRef,
) -> (&'static str, String) {
    match target {
        NativeStemsHeadBuilderTargetRef::Head(reference) => ("C", native_c_alias(reference)),
        NativeStemsHeadBuilderTargetRef::Beam(reference) => ("B", b_alias(stumps, reference)),
    }
}

fn chunk_event_alias(system_id: usize, reference: NativeStemsHeadBuilderGlyphRef) -> String {
    let NativeStemsHeadBuilderGlyphRef::Chunk {
        builder_ordinal,
        filament_ordinal,
    } = reference
    else {
        panic!("non-chunk event reference {reference:?}");
    };
    format!("chunkEvent:s{system_id}:c{builder_ordinal}:{filament_ordinal}")
}

fn seed_scan_action(action: NativeStemsHeadSeedScanAction) -> &'static str {
    match action {
        NativeStemsHeadSeedScanAction::OutsideLookup => "outside",
        NativeStemsHeadSeedScanAction::OverlapsStump => "stump",
        NativeStemsHeadSeedScanAction::InsufficientContribution => "contrib",
        NativeStemsHeadSeedScanAction::TooFarFromLine => "distance",
        NativeStemsHeadSeedScanAction::Preliminary => "prelim",
    }
}

fn section_action(
    scan: &audiveris_omr::native_stems_head_builders::NativeStemsHeadBuilderSectionScan,
    vertical: bool,
) -> &'static str {
    if !scan.intersects_lookup {
        "outside"
    } else if scan.width_accepted == Some(false) {
        "wide"
    } else if vertical && scan.stump_overlap_accepted == Some(false) {
        "stumpOverlap"
    } else if scan.before_last_head == Some(false) {
        "pastHead"
    } else if vertical && scan.distance_accepted == Some(false) {
        "distance"
    } else {
        assert!(scan.accepted, "unclassified rejected section scan");
        "accept"
    }
}

fn corner_id(reference: NativeStemsHeadCornerRef) -> &'static str {
    match (reference.vertical, reference.horizontal) {
        (NativeStemVerticalSide::Top, NativeStemHeadSide::Right) => "T-R",
        (NativeStemVerticalSide::Bottom, NativeStemHeadSide::Left) => "B-L",
        (NativeStemVerticalSide::Top, NativeStemHeadSide::Left) => "T-L",
        (NativeStemVerticalSide::Bottom, NativeStemHeadSide::Right) => "B-R",
    }
}

fn tuple_point_token(point: (f64, f64)) -> String {
    format!("{}:{}", java_hex_double(point.0), java_hex_double(point.1))
}

fn usize_list(values: &[usize]) -> String {
    string_list(&values.iter().map(usize::to_string).collect::<Vec<_>>())
}

fn bounded_bounds_token(bounds: BoundedBounds) -> String {
    format!(
        "{}:{}:{}:{}",
        bounds.x, bounds.y, bounds.width, bounds.height
    )
}

fn bounds_rectangle(bounds: Bounds) -> JavaRectangle {
    JavaRectangle::new(
        i32::try_from(bounds.x).expect("bounds x"),
        i32::try_from(bounds.y).expect("bounds y"),
        i32::try_from(bounds.width).expect("bounds width"),
        i32::try_from(bounds.height).expect("bounds height"),
    )
}

fn y_overlap_rectangle(left: JavaRectangle, right: JavaRectangle) -> i32 {
    let top = left.y.max(right.y);
    let bottom = left
        .y
        .wrapping_add(left.height)
        .min(right.y.wrapping_add(right.height));
    bottom.wrapping_sub(top)
}

fn line_point_distance(line: NativeStemLine, point: (f64, f64)) -> f64 {
    let x2 = line.stop.x - line.start.x;
    let y2 = line.stop.y - line.start.y;
    let px = point.0 - line.start.x;
    let py = point.1 - line.start.y;
    let product = (px * x2) + (py * y2);
    let projection_sq = product * product / ((x2 * x2) + (y2 * y2));
    let mut length_sq = (px * px) + (py * py) - projection_sq;
    if length_sq < 0.0 {
        length_sq = 0.0;
    }
    length_sq.sqrt()
}

fn java_double_compare(left: f64, right: f64) -> i32 {
    if left < right {
        -1
    } else if left > right {
        1
    } else {
        let bits = |value: f64| {
            if value.is_nan() {
                0x7ff8_0000_0000_0000_u64
            } else {
                value.to_bits()
            }
        };
        match (bits(left) as i64).cmp(&(bits(right) as i64)) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        }
    }
}

fn line_bounds(line: NativeStemLine) -> JavaRectangle {
    let min_x = line.start.x.min(line.stop.x).floor() as i32;
    let min_y = line.start.y.min(line.stop.y).floor() as i32;
    let max_x = line.start.x.max(line.stop.x).ceil() as i32;
    let max_y = line.start.y.max(line.stop.y).ceil() as i32;
    JavaRectangle::new(min_x, min_y, max_x - min_x, max_y - min_y)
}
