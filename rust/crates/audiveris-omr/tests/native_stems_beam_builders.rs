// SPDX-License-Identifier: AGPL-3.0-or-later

//! Eight-page differential-gate scaffold for beam-origin `StemBuilder` construction.
//!
//! The frozen boundary starts with the exact `BeamLinker.VLinker.inspect` inputs and
//! ends after `StemBuilder.retrieveAllItems`/`retrieveLengths`.  It grades the
//! constructor's local collection mutations plus sheet-global filament/glyph
//! registration effects, but no stem creation, SIG mutation, link mutation, or
//! head-origin `CLinker` builder.
//!
//! Dense V/H section scans are represented by one row per builder and
//! orientation: ordered accepted ordinals, exhaustive branch totals, and a
//! SHA-256 over every candidate's complete ordered branch/raw-bit record. The
//! Rust projector reconstructs that stream; it never trusts an oracle digest
//! without independently deriving the bytes.
//!
//! Item sorting is not treated as an abstract stable sort. The gate grades
//! each occurrence's exact input/output permutation, independently recomputes
//! the comparator anomaly census, and requires the corrected Java comparator
//! to remain a total preorder across the corpus.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use audiveris_image::{run_table::RunTable, section::Bounds};
use audiveris_omr::{
    head_scanner_slices::JavaRectangle,
    native_headers::recognize_native_headers,
    native_heads::{NativeHeadsRecognition, recognize_native_heads},
    native_ledgers::{NativeLedgerRecognition, recognize_native_ledgers},
    native_stem_seeds::{NativeStemSeedRecognition, recognize_native_stem_seeds},
    native_stems_beam_builders::{
        NativeStemsBeamBuilder, NativeStemsBeamBuilderAlignmentSubject,
        NativeStemsBeamBuilderGlyphRef, NativeStemsBeamBuilderItem, NativeStemsBeamBuilderItemKind,
        NativeStemsBeamBuilderPreBuilderAttachment, NativeStemsBeamBuilderPreBuilderGlyphSource,
        NativeStemsBeamBuilderRecognition, NativeStemsBeamBuilderRegistrationAction,
        NativeStemsBeamBuilderSeedAction, NativeStemsBeamBuilderSortEntry,
        NativeStemsBeamBuilderSystem, NativeStemsBeamBuilderTargetRef,
        materialize_native_stems_beam_builders,
    },
    native_stems_beam_reachability::{
        NativeStemsBeamReachabilityRecognition, materialize_native_stems_beam_reachability,
    },
    native_stems_beam_stumps::{
        NativeStemsBeamSource, NativeStemsBeamStumpRecognition, NativeStemsBeamStumpRef,
        NativeStemsBeamStumpSystem, materialize_native_stems_beam_stumps,
    },
    native_stems_beam_vlinkers::{
        NativeStemsBeamBLinkerRef, NativeStemsBeamVLinkerRecognition, NativeStemsBeamVLinkerSystem,
        materialize_native_stems_beam_vlinkers,
    },
    native_stems_head_corners::{
        NativeStemsHeadCornerRecognition, materialize_native_stems_head_corners,
    },
    native_stems_head_seeds::{
        NativeStemsHeadSeedRecognition, materialize_native_stems_head_seeds,
    },
    native_stems_head_stumps::{
        NativeStemsHeadStumpRecognition, materialize_native_stems_head_stumps,
    },
    recognize::{
        GridLinesRecognition, NativeBeamRecognition, recognize_grid_lines,
        recognize_native_beams_with_stem_seeds,
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
const ORACLE_PATH: &str = "rust/oracle/stems-beam-stem-builders.txt";
const PROBE_PATH: &str = "rust/oracle/java/StemsBeamStemBuilderProbe.java";
const RUNNER_PATH: &str = "rust/oracle/java/run-stems-beam-stem-builders.sh";

const EXPECTED_FIXTURE_SHA256: Option<&str> =
    Some("638dd815d5d110dd67bab202c31ee966c4fd229d6998107cdc3f9483045ffcf1");
const EXPECTED_PROBE_SHA256: Option<&str> =
    Some("acf21ca496ec309138cb530c05827a4e5b639763d7f4cc5cc0bff4d2b8657646");
const EXPECTED_RUNNER_SHA256: Option<&str> =
    Some("54bad840185412fa504a6f093f464e904ae16f0b8e8c2e4e2fac84a294681b8e");
const EXPECTED_BODY_SHA256: Option<&str> =
    Some("be00a4a2c5ee05b92b3fe70b157cbb246ab066ddd9fe89168db63657438672e4");
const EXPECTED_FIXTURE_BYTES: Option<usize> = Some(29_167_131);
const EXPECTED_FIXTURE_LINES: Option<usize> = Some(91_210);

const EXPECTED_CORPUS_FIELDS: &str = concat!(
    "systems 30 beams 803 smallBeams 0 heads 3521 smallHeads 0 initialBs 2116 ",
    "initialVs 2417 bVisits 2145 anchorSkips 29 anchorsCreated 145 finalBs 2261 ",
    "builders 2417 topBuilders 1390 bottomBuilders 1027 directionDivergences 1 ",
    "profile3Builders 512 ",
    "profile4Builders 1905 stumplessStarts 590 inputSeeds 2169 removedSeeds 215 ",
    "retainedSeeds 1954 inputTargets 6676 removedTargets 6 retainedTargets 6670 ",
    "inputBTargets 1617 removedBTargets 0 retainedBTargets 1617 inputCTargets 5059 ",
    "removedCTargets 6 retainedCTargets 5053 alignComparisons 1475 alignRemovals 114 ",
    "alignTieRemoveSecond 3 vSectionScans 2851879 vSectionAccepts 2174 ",
    "vSectionOutside 2834360 vSectionWide 4655 vSectionStumpOverlap 7267 ",
    "vSectionPastHead 2973 vSectionDistance 450 hSectionScans 2615734 ",
    "hSectionAccepts 4524 hSectionOutside 2602329 hSectionWide 6507 ",
    "hSectionPastHead 2374 selectedSectionRows 6698 filaments 1442 filamentMembers 3998 ",
    "filamentRegistrations 1442 externalMembers 0 glyphAttempts 1442 glyphNew 799 ",
    "glyphReuse 643 reuseSeedOrigins 103 reuseBeamStumpOrigins 78 ",
    "reuseHeadStumpOrigins 93 reuseRejectedHeadStumpOrigins 142 ",
    "reusePriorChunkOrigins 464 reuseRawBeamOrigins 0 reuseHeadOrigins 0 ",
    "reuseLedgerOrigins 0 reuseGridOrigins 0 reuseSigOtherOrigins 0 ",
    "reuseUnmodeledOrigins 0 chunkRemovals 175 chunkSeedDrops 103 ",
    "chunkUnalignedDrops 69 chunkStartDrops 3 chunkHeadPartsDrops 0 seedItemDrops 1930 ",
    "preItems 10378 preStart 2417 preB 1617 preC 5053 preSeed 24 preChunk 1267 ",
    "sortAudits 4657 sortRows 14631 sortCycles 0 sortEquivalence 0 ",
    "maxTargetSortItems 11 maxFinalSortItems 14 sortListsAtLeast32 0 gapChecks 9660 ",
    "gapInserts 320 gapTruncations 563 finalItems 9417 finalStart 2417 finalB 1617 ",
    "finalC 4063 finalSeed 2 finalChunk 998 finalGap 320 lengthRows 12085 ",
    "builderChecks 35419 sigMutations 0 systemStemMutations 0 linkMutations 0 ",
    "cBuilderMutations 0 unexpectedBuilderMutations 0",
);

/// Every semantic row family required before the fixture can be frozen.
/// `stemsdiagnosticbeambuilderglyphinventory` is intentionally absent: it
/// inventories Java-global `GlyphIndex` identities that this bounded native
/// boundary neither owns nor fabricates, and it is excluded from oracle row
/// hashes. Registration actions and structural origin memberships below are
/// still graded exactly.
const REQUIRED_ROW_PREFIXES: &[&str] = &[
    "stemsbeambuilderpage ",
    "stemsbeambuildersystem ",
    "stemsbeambuilder ",
    "stemsbeambuilderalign ",
    "stemsbeambuilderseedfilter ",
    "stemsbeambuildertargetfilter ",
    "stemsbeambuildervsection ",
    "stemsbeambuilderhsection ",
    "stemsbeambuilderfilament ",
    "stemsbeambuilderfilamentmember ",
    "stemsbeambuilderglyphreg ",
    "stemsbeambuilderchunkfilter ",
    "stemsbeambuilderseeditemfilter ",
    "stemsbeambuilderitempre ",
    "stemsbeambuildersort ",
    "stemsbeambuildersortaudit ",
    "stemsbeambuildergap ",
    "stemsbeambuilderitem ",
    "stemsbeambuilderlength ",
    "stemsbeambuilderend ",
    "stemsbeambuildersystemsummary ",
    "stemsbeambuilderpagesummary ",
    "stemsbeambuildercorpussummary ",
];

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Totals {
    systems: usize,
    beams: usize,
    small_beams: usize,
    heads: usize,
    small_heads: usize,
    initial_bs: usize,
    initial_vs: usize,
    b_visits: usize,
    anchor_skips: usize,
    anchors_created: usize,
    final_bs: usize,
    vertical_source_sections: usize,
    horizontal_source_sections: usize,
    glyph_registry_baseline_entries: usize,
    glyph_registry_baseline_distinct_contents: usize,
    builders: usize,
    top_builders: usize,
    bottom_builders: usize,
    direction_divergences: usize,
    profile_three_builders: usize,
    profile_four_builders: usize,
    stumpless_starts: usize,
    input_seeds: usize,
    removed_seeds: usize,
    retained_seeds: usize,
    seed_alignment_checks: usize,
    seed_alignment_removals: usize,
    seed_alignment_tie_second_removals: usize,
    last_head_seed_removals: usize,
    duplicate_target_seed_drops: usize,
    start_overlap_seed_drops: usize,
    zero_contribution_seed_drops: usize,
    retained_seed_items: usize,
    input_targets: usize,
    removed_targets: usize,
    retained_targets: usize,
    input_beam_targets: usize,
    input_head_targets: usize,
    removed_beam_targets: usize,
    removed_head_targets: usize,
    retained_beam_targets: usize,
    retained_head_targets: usize,
    alignment_comparisons: usize,
    alignment_removals: usize,
    alignment_tie_second_removals: usize,
    vertical_section_scans: usize,
    vertical_section_area_drops: usize,
    vertical_section_width_drops: usize,
    vertical_section_shorter_stump_overlap_drops: usize,
    vertical_section_last_head_drops: usize,
    vertical_section_distance_drops: usize,
    vertical_section_accepts: usize,
    horizontal_section_scans: usize,
    horizontal_section_area_drops: usize,
    horizontal_section_width_drops: usize,
    horizontal_section_last_head_drops: usize,
    horizontal_section_accepts: usize,
    selected_section_rows: usize,
    filaments: usize,
    filament_members: usize,
    filament_registrations: usize,
    external_filament_members: usize,
    glyph_registration_attempts: usize,
    glyph_registration_distinct_canonical_contents: usize,
    glyph_registration_duplicate_occurrences: usize,
    glyph_registrations_new: usize,
    glyph_registrations_reused: usize,
    glyph_reuses_from_stem_seeds: usize,
    glyph_reuses_from_beam_stumps: usize,
    glyph_reuses_from_head_stumps: usize,
    glyph_reuses_from_rejected_head_stumps: usize,
    glyph_reuses_from_earlier_current_builder_chunks: usize,
    glyph_reuses_from_prior_builder_chunks: usize,
    glyph_reuses_from_raw_beams: usize,
    glyph_reuses_from_heads: usize,
    glyph_reuses_from_ledgers: usize,
    glyph_reuses_from_grid: usize,
    glyph_reuses_from_other_sig: usize,
    /// Oracle origins that cannot be mapped to concrete upstream content.
    oracle_reuse_origin_unresolved: usize,
    /// Honest native novelty flags while the general API baseline is partial.
    modeled_new_incomplete_baseline: usize,
    chunk_removals: usize,
    chunk_seed_equality_drops: usize,
    chunk_head_part_drops: usize,
    chunk_alignment_checks: usize,
    chunk_alignment_removals: usize,
    chunk_start_stump_drops: usize,
    chunk_start_stump_equal_survivors: usize,
    retained_chunks: usize,
    seed_item_drops: usize,
    pre_items: usize,
    pre_start_items: usize,
    pre_beam_items: usize,
    pre_head_items: usize,
    pre_seed_items: usize,
    pre_chunk_items: usize,
    sort_audits: usize,
    sort_rows: usize,
    sort_tie_groups: usize,
    sort_strict_cycles: usize,
    sort_equivalence_inconsistencies: usize,
    max_target_sort_items: usize,
    max_final_sort_items: usize,
    sort_lists_at_least_32: usize,
    gap_checks: usize,
    gap_inserts: usize,
    gap_truncations: usize,
    final_items: usize,
    final_start_items: usize,
    final_beam_items: usize,
    final_head_items: usize,
    final_seed_items: usize,
    final_chunk_items: usize,
    final_gap_items: usize,
    length_rows: usize,
    builder_checks: usize,
    sig_mutations: usize,
    system_stem_mutations: usize,
    link_mutations: usize,
    head_builder_mutations: usize,
    unexpected_builder_mutations: usize,
}

#[allow(dead_code)]
struct NativePage {
    grid: GridLinesRecognition,
    stem_seeds: NativeStemSeedRecognition,
    beams: NativeBeamRecognition,
    ledgers: NativeLedgerRecognition,
    heads: NativeHeadsRecognition,
    corners: NativeStemsHeadCornerRecognition,
    head_seeds: NativeStemsHeadSeedRecognition,
    beam_stumps: NativeStemsBeamStumpRecognition,
    beam_vlinkers: NativeStemsBeamVLinkerRecognition,
    reachability: NativeStemsBeamReachabilityRecognition,
    head_stumps: NativeStemsHeadStumpRecognition,
    builders: NativeStemsBeamBuilderRecognition,
}

impl Totals {
    fn summary_fields(self) -> String {
        format!(
            "beams {} smallBeams {} heads {} smallHeads {} initialBs {} initialVs {} \
             bVisits {} anchorSkips {} anchorsCreated {} finalBs {} builders {} \
             topBuilders {} bottomBuilders {} directionDivergences {} profile3Builders {} \
             profile4Builders {} \
             stumplessStarts {} inputSeeds {} removedSeeds {} retainedSeeds {} \
             inputTargets {} removedTargets {} retainedTargets {} inputBTargets {} \
             removedBTargets {} retainedBTargets {} inputCTargets {} removedCTargets {} \
             retainedCTargets {} alignComparisons {} alignRemovals {} alignTieRemoveSecond {} \
             vSectionScans {} vSectionAccepts {} vSectionOutside {} vSectionWide {} \
             vSectionStumpOverlap {} vSectionPastHead {} vSectionDistance {} hSectionScans {} \
             hSectionAccepts {} hSectionOutside {} hSectionWide {} hSectionPastHead {} \
             selectedSectionRows {} filaments {} filamentMembers {} filamentRegistrations {} \
             externalMembers {} glyphAttempts {} glyphNew {} glyphReuse {} reuseSeedOrigins {} \
             reuseBeamStumpOrigins {} reuseHeadStumpOrigins {} \
             reuseRejectedHeadStumpOrigins {} reusePriorChunkOrigins {} reuseRawBeamOrigins {} \
             reuseHeadOrigins {} reuseLedgerOrigins {} reuseGridOrigins {} reuseSigOtherOrigins {} \
             reuseUnmodeledOrigins {} chunkRemovals {} chunkSeedDrops {} chunkUnalignedDrops {} \
             chunkStartDrops {} chunkHeadPartsDrops 0 seedItemDrops {} preItems {} preStart {} \
             preB {} preC {} preSeed {} preChunk {} sortAudits {} sortRows {} sortCycles {} \
             sortEquivalence {} maxTargetSortItems {} maxFinalSortItems {} \
             sortListsAtLeast32 {} gapChecks {} gapInserts {} gapTruncations {} finalItems {} \
             finalStart {} finalB {} finalC {} finalSeed {} finalChunk {} finalGap {} lengthRows {} \
             builderChecks {} sigMutations {} systemStemMutations {} linkMutations {} \
             cBuilderMutations {} unexpectedBuilderMutations {}",
            self.beams,
            self.small_beams,
            self.heads,
            self.small_heads,
            self.initial_bs,
            self.initial_vs,
            self.b_visits,
            self.anchor_skips,
            self.anchors_created,
            self.final_bs,
            self.builders,
            self.top_builders,
            self.bottom_builders,
            self.direction_divergences,
            self.profile_three_builders,
            self.profile_four_builders,
            self.stumpless_starts,
            self.input_seeds,
            self.removed_seeds,
            self.retained_seeds,
            self.input_targets,
            self.removed_targets,
            self.retained_targets,
            self.input_beam_targets,
            self.removed_beam_targets,
            self.retained_beam_targets,
            self.input_head_targets,
            self.removed_head_targets,
            self.retained_head_targets,
            self.alignment_comparisons,
            self.alignment_removals,
            self.alignment_tie_second_removals,
            self.vertical_section_scans,
            self.vertical_section_accepts,
            self.vertical_section_area_drops,
            self.vertical_section_width_drops,
            self.vertical_section_shorter_stump_overlap_drops,
            self.vertical_section_last_head_drops,
            self.vertical_section_distance_drops,
            self.horizontal_section_scans,
            self.horizontal_section_accepts,
            self.horizontal_section_area_drops,
            self.horizontal_section_width_drops,
            self.horizontal_section_last_head_drops,
            self.selected_section_rows,
            self.filaments,
            self.filament_members,
            self.filament_registrations,
            self.external_filament_members,
            self.glyph_registration_attempts,
            self.glyph_registrations_new,
            self.glyph_registrations_reused,
            self.glyph_reuses_from_stem_seeds,
            self.glyph_reuses_from_beam_stumps,
            self.glyph_reuses_from_head_stumps,
            self.glyph_reuses_from_rejected_head_stumps,
            self.glyph_reuses_from_prior_builder_chunks,
            self.glyph_reuses_from_raw_beams,
            self.glyph_reuses_from_heads,
            self.glyph_reuses_from_ledgers,
            self.glyph_reuses_from_grid,
            self.glyph_reuses_from_other_sig,
            self.oracle_reuse_origin_unresolved,
            self.chunk_removals,
            self.chunk_seed_equality_drops,
            self.chunk_alignment_removals,
            self.chunk_start_stump_drops,
            self.seed_item_drops,
            self.pre_items,
            self.pre_start_items,
            self.pre_beam_items,
            self.pre_head_items,
            self.pre_seed_items,
            self.pre_chunk_items,
            self.sort_audits,
            self.sort_rows,
            self.sort_strict_cycles,
            self.sort_equivalence_inconsistencies,
            self.max_target_sort_items,
            self.max_final_sort_items,
            self.sort_lists_at_least_32,
            self.gap_checks,
            self.gap_inserts,
            self.gap_truncations,
            self.final_items,
            self.final_start_items,
            self.final_beam_items,
            self.final_head_items,
            self.final_seed_items,
            self.final_chunk_items,
            self.final_gap_items,
            self.length_rows,
            self.builder_checks,
            self.sig_mutations,
            self.system_stem_mutations,
            self.link_mutations,
            self.head_builder_mutations,
            self.unexpected_builder_mutations,
        )
    }
}

fn derive_totals(
    native: &NativePage,
    systems: &[&NativeStemsBeamBuilderSystem],
    rows: &[String],
) -> Totals {
    let mut totals = Totals {
        systems: systems.len(),
        ..Totals::default()
    };
    for system in systems {
        let v_linkers = native
            .beam_vlinkers
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system.system_id)
            .expect("summary VLinker system");
        let reachability = native
            .reachability
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system.system_id)
            .expect("summary reachability system");
        let corners = native
            .corners
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system.system_id)
            .expect("summary corner system");
        totals.beams += reachability.beam_inspections.len();
        totals.small_beams += reachability
            .beam_inspections
            .iter()
            .filter(|beam| beam.is_small_beam)
            .count();
        totals.heads += corners.heads_in_sig_order.len();
        totals.initial_bs += v_linkers
            .constructors
            .iter()
            .filter(|beam| beam.survives_constructor_loop)
            .map(|beam| beam.b_linkers.len())
            .sum::<usize>();
        totals.initial_vs += v_linkers
            .constructors
            .iter()
            .filter(|beam| beam.survives_constructor_loop)
            .flat_map(|beam| &beam.b_linkers)
            .map(|b| b.v_linkers.len())
            .sum::<usize>();
        totals.b_visits += reachability
            .beam_inspections
            .iter()
            .map(|beam| beam.b_visits.len())
            .sum::<usize>();
        totals.anchor_skips += reachability
            .beam_inspections
            .iter()
            .flat_map(|beam| &beam.b_visits)
            .filter(|visit| visit.skipped_anchor)
            .count();
        totals.final_bs += reachability
            .final_beam_arenas
            .iter()
            .map(|arena| arena.all_b_linkers.len())
            .sum::<usize>();
        totals.anchors_created += reachability
            .final_beam_arenas
            .iter()
            .flat_map(|arena| &arena.all_b_linkers)
            .filter(|entry| entry.is_anchor)
            .count();
    }

    for row in rows {
        if row.starts_with("stemsbeambuilder ") {
            totals.builders += 1;
            match field_value(row, "builderYDir") {
                "-1" => totals.top_builders += 1,
                "1" => totals.bottom_builders += 1,
                direction => panic!("unknown builder direction {direction}"),
            }
            totals.direction_divergences +=
                usize::from(field_value(row, "directionDiverges") == "true");
            match number(row, "profile") {
                3 => totals.profile_three_builders += 1,
                4 => totals.profile_four_builders += 1,
                profile => panic!("unexpected builder profile {profile}"),
            }
            totals.stumpless_starts += usize::from(field_value(row, "startStump") == "-");
            totals.input_seeds += token_list_len(field_value(row, "inputSeeds"));
            totals.input_beam_targets += token_list_len(field_value(row, "inputBTargets"));
            totals.input_head_targets += token_list_len(field_value(row, "inputCTargets"));
        } else if row.starts_with("stemsbeambuilderseedfilter ") {
            if field_value(row, "retainedIdentity") == "true" {
                totals.retained_seeds += 1;
            } else {
                totals.removed_seeds += 1;
            }
        } else if row.starts_with("stemsbeambuildertargetfilter ") {
            let removed = field_value(row, "action") != "keep";
            match field_value(row, "kind") {
                "B" => {
                    totals.removed_beam_targets += usize::from(removed);
                    totals.retained_beam_targets += usize::from(!removed);
                }
                "C" => {
                    totals.removed_head_targets += usize::from(removed);
                    totals.retained_head_targets += usize::from(!removed);
                }
                kind => panic!("unknown target kind {kind}"),
            }
        } else if row.starts_with("stemsbeambuilderalign ") {
            totals.alignment_comparisons += 1;
            totals.alignment_removals += usize::from(field_value(row, "aligned") == "false");
            totals.alignment_tie_second_removals +=
                usize::from(field_value(row, "tieHeightRemoveSecond") == "true");
        } else if row.starts_with("stemsbeambuildervsection ") {
            totals.vertical_section_scans += number(row, "sourceCount");
            totals.vertical_section_accepts += reason(row, "accept");
            totals.vertical_section_area_drops += reason(row, "outsideArea");
            totals.vertical_section_width_drops += reason(row, "wide");
            totals.vertical_section_shorter_stump_overlap_drops +=
                reason(row, "shorterStumpOverlap");
            totals.vertical_section_last_head_drops += reason(row, "pastLastHead");
            totals.vertical_section_distance_drops += reason(row, "lineDistance");
        } else if row.starts_with("stemsbeambuilderhsection ") {
            totals.horizontal_section_scans += number(row, "sourceCount");
            totals.horizontal_section_accepts += reason(row, "accept");
            totals.horizontal_section_area_drops += reason(row, "outsideArea");
            totals.horizontal_section_width_drops += reason(row, "wide");
            totals.horizontal_section_last_head_drops += reason(row, "pastLastHead");
        } else if row.starts_with("stemsbeambuilderfilamentmember ") {
            totals.filament_members += 1;
        } else if row.starts_with("stemsbeambuilderfilament ") {
            totals.filaments += 1;
            totals.filament_registrations += 1;
        } else if row.starts_with("stemsbeambuilderglyphreg ") {
            totals.glyph_registration_attempts += 1;
            match field_value(row, "result") {
                "New" => totals.glyph_registrations_new += 1,
                "Reuse" => {
                    totals.glyph_registrations_reused += 1;
                    let origins = field_value(row, "originCategories").split(',');
                    for origin in origins {
                        match origin {
                            "stemSeed" => totals.glyph_reuses_from_stem_seeds += 1,
                            "beamStump" => totals.glyph_reuses_from_beam_stumps += 1,
                            "headStump" => totals.glyph_reuses_from_head_stumps += 1,
                            "rejectedHeadStump" => {
                                totals.glyph_reuses_from_rejected_head_stumps += 1;
                            }
                            "priorStemBuilderChunk" => {
                                totals.glyph_reuses_from_prior_builder_chunks += 1;
                            }
                            "rawBeam" => totals.glyph_reuses_from_raw_beams += 1,
                            "head" => totals.glyph_reuses_from_heads += 1,
                            "ledger" => totals.glyph_reuses_from_ledgers += 1,
                            "grid" => totals.glyph_reuses_from_grid += 1,
                            "sigOther" => totals.glyph_reuses_from_other_sig += 1,
                            "unmodeled" => totals.oracle_reuse_origin_unresolved += 1,
                            value => panic!("unknown Reuse origin {value}"),
                        }
                    }
                }
                result => panic!("unknown registration result {result}"),
            }
        } else if row.starts_with("stemsbeambuilderchunkfilter ") {
            match field_value(row, "action") {
                "keep" => {}
                "seedEqual" => {
                    totals.chunk_removals += 1;
                    totals.chunk_seed_equality_drops += 1;
                }
                "unaligned" => {
                    totals.chunk_removals += 1;
                    totals.chunk_alignment_removals += 1;
                }
                "startEqual" => {
                    totals.chunk_removals += 1;
                    totals.chunk_start_stump_drops += 1;
                }
                action => panic!("unknown chunk action {action}"),
            }
        } else if row.starts_with("stemsbeambuilderseeditemfilter ") {
            totals.seed_item_drops += usize::from(field_value(row, "action") != "keep");
        } else if row.starts_with("stemsbeambuilderitempre ") {
            totals.pre_items += 1;
            match field_value(row, "kind") {
                "startHalf" => totals.pre_start_items += 1,
                "B" => totals.pre_beam_items += 1,
                "C" => totals.pre_head_items += 1,
                "seed" => totals.pre_seed_items += 1,
                "chunk" => totals.pre_chunk_items += 1,
                kind => panic!("unknown pre-item kind {kind}"),
            }
        } else if row.starts_with("stemsbeambuildersortaudit ") {
            totals.sort_audits += 1;
            let items = number_last(row, "items");
            totals.sort_strict_cycles += number(row, "strictCycles");
            totals.sort_equivalence_inconsistencies += number(row, "equivalenceInconsistencies");
            match field_value(row, "phase") {
                "targets" => totals.max_target_sort_items = totals.max_target_sort_items.max(items),
                "items" => totals.max_final_sort_items = totals.max_final_sort_items.max(items),
                phase => panic!("unknown sort phase {phase}"),
            }
            totals.sort_lists_at_least_32 += usize::from(items >= 32);
        } else if row.starts_with("stemsbeambuildersort ") {
            totals.sort_rows += 1;
        } else if row.starts_with("stemsbeambuildergap ") {
            totals.gap_checks += 1;
            totals.gap_inserts += usize::from(field_value(row, "action") == "insert");
            totals.gap_truncations += usize::from(field_value(row, "action") == "truncate");
        } else if row.starts_with("stemsbeambuilderitem ") {
            totals.final_items += 1;
            match field_value(row, "kind") {
                "startHalf" => totals.final_start_items += 1,
                "B" => totals.final_beam_items += 1,
                "C" => totals.final_head_items += 1,
                "seed" => totals.final_seed_items += 1,
                "chunk" => totals.final_chunk_items += 1,
                "gap" => totals.final_gap_items += 1,
                kind => panic!("unknown final-item kind {kind}"),
            }
        } else if row.starts_with("stemsbeambuilderlength ") {
            totals.length_rows += 1;
        }
    }
    totals.input_targets = totals.input_beam_targets + totals.input_head_targets;
    totals.removed_targets = totals.removed_beam_targets + totals.removed_head_targets;
    totals.retained_targets = totals.retained_beam_targets + totals.retained_head_targets;
    totals.selected_section_rows =
        totals.vertical_section_accepts + totals.horizontal_section_accepts;
    totals.builder_checks = (3 * totals.builders) + (8 * totals.heads);
    totals
}

fn number(row: &str, name: &str) -> usize {
    field_value(row, name)
        .parse()
        .unwrap_or_else(|_| panic!("non-number {name} in {row}"))
}

fn number_last(row: &str, name: &str) -> usize {
    let words = row.split_whitespace().collect::<Vec<_>>();
    words
        .windows(2)
        .rev()
        .find(|pair| pair[0] == name)
        .and_then(|pair| pair[1].parse().ok())
        .unwrap_or_else(|| panic!("missing numeric {name} in {row}"))
}

fn token_list_len(value: &str) -> usize {
    if value == "-" {
        0
    } else {
        value.split(',').count()
    }
}

fn reason(row: &str, wanted: &str) -> usize {
    field_value(row, "reasons")
        .split(',')
        .find_map(|entry| {
            let (name, value) = entry.split_once(':')?;
            (name == wanted).then(|| value.parse().expect("section reason count"))
        })
        .unwrap_or(0)
}

fn fnv1a_rows(rows: &[String]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for row in rows {
        for byte in row.bytes().chain(std::iter::once(b'\n')) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
    }
    hash
}

#[test]
fn native_stems_beam_builders_match_java_corpus_exactly() {
    let oracle = std::fs::read_to_string(repo_path(ORACLE_PATH))
        .expect("beam-origin StemBuilder oracle fixture");
    let corpus_summary = oracle
        .lines()
        .find(|line| line.starts_with("stemsbeambuildercorpussummary "))
        .expect("beam-origin StemBuilder corpus summary");
    assert!(
        corpus_summary.starts_with(&format!(
            "stemsbeambuildercorpussummary pages {} {EXPECTED_CORPUS_FIELDS} ",
            PAGES.len()
        )),
        "oracle corpus totals changed: {corpus_summary}"
    );
    for prefix in REQUIRED_ROW_PREFIXES {
        assert!(
            oracle.lines().any(|line| line.starts_with(prefix)),
            "oracle is missing required row family {prefix:?}"
        );
    }
    assert_eq!(
        oracle
            .lines()
            .filter(|line| line.starts_with("stemsbeambuilderpage "))
            .count(),
        PAGES.len(),
        "oracle page count"
    );
    for (label, value) in [
        ("fixture SHA-256", EXPECTED_FIXTURE_SHA256),
        ("probe SHA-256", EXPECTED_PROBE_SHA256),
        ("runner SHA-256", EXPECTED_RUNNER_SHA256),
        ("body SHA-256", EXPECTED_BODY_SHA256),
    ] {
        assert!(value.is_some(), "{label} is not frozen");
    }
    assert!(
        EXPECTED_FIXTURE_BYTES.is_some(),
        "fixture byte count is not frozen"
    );
    assert!(
        EXPECTED_FIXTURE_LINES.is_some(),
        "fixture line count is not frozen"
    );
    assert!(repo_path(PROBE_PATH).is_file(), "Java probe is missing");
    assert!(repo_path(RUNNER_PATH).is_file(), "Java runner is missing");

    assert_eq!(oracle.len(), EXPECTED_FIXTURE_BYTES.expect("fixture bytes"));
    assert_eq!(
        oracle.lines().count(),
        EXPECTED_FIXTURE_LINES.expect("fixture lines")
    );
    assert_eq!(
        sha256_hex(oracle.as_bytes()),
        EXPECTED_FIXTURE_SHA256.unwrap()
    );
    assert_eq!(
        sha256_hex(&std::fs::read(repo_path(PROBE_PATH)).expect("probe source")),
        EXPECTED_PROBE_SHA256.unwrap()
    );
    assert_eq!(
        sha256_hex(&std::fs::read(repo_path(RUNNER_PATH)).expect("runner source")),
        EXPECTED_RUNNER_SHA256.unwrap()
    );
    assert_eq!(
        field_value(corpus_summary, "emittedBodySha256"),
        EXPECTED_BODY_SHA256.unwrap()
    );

    for image in PAGES {
        let page = format!("{image}#1");
        let native = native_page(image);
        assert_eq!(
            native.builders.builder_count,
            native
                .builders
                .systems
                .iter()
                .map(|system| system.builders.len())
                .sum::<usize>(),
            "{image}: builder aggregate"
        );
        let actual = native_page_rows(&page, &native);
        let expected = oracle_page_rows(&oracle, &page);
        if actual != expected {
            report_first_mismatches(&page, &actual, &expected);
            panic!("{page}: native beam StemBuilder evidence differs from Java");
        }
    }
}

fn native_page_rows(page: &str, native: &NativePage) -> Vec<String> {
    let mut rows = vec![format!(
        "stemsbeambuilderpage {page} systems {} staves {} family Bravura",
        native.builders.systems.len(),
        native.grid.staves.len(),
    )];
    let mut chunk_aliases = Vec::<(GlyphData, String)>::new();
    let mut glyph_origins = GlyphOriginLedger::from_pre_stems(native);
    for system in &native.builders.systems {
        let system_row_start = rows.len();
        let corners = native
            .corners
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system.system_id)
            .expect("builder corner system");
        let stumps = native
            .beam_stumps
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system.system_id)
            .expect("builder beam-stump system");
        let v_linkers = native
            .beam_vlinkers
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system.system_id)
            .expect("builder VLinker system");
        let reachability = native
            .reachability
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system.system_id)
            .expect("builder reachability system");
        let head_stumps = native
            .head_stumps
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system.system_id)
            .expect("builder head-stump system");
        glyph_origins.replay_pre_builder(native, system, stumps, head_stumps);
        rows.push(system_row(page, system, corners.profile));
        for builder in &system.builders {
            append_builder_rows(
                page,
                native,
                system,
                stumps,
                v_linkers,
                reachability,
                builder,
                &mut chunk_aliases,
                &mut glyph_origins,
                &mut rows,
            );
        }
        let totals = derive_totals(
            native,
            std::slice::from_ref(&system),
            &rows[system_row_start..],
        );
        let hash = fnv1a_rows(&rows[system_row_start..]);
        rows.push(format!(
            "stemsbeambuildersystemsummary {page} system {} systems 1 {} hash {hash:016x}",
            system.system_id,
            totals.summary_fields(),
        ));
    }
    let totals = derive_totals(
        native,
        &native.builders.systems.iter().collect::<Vec<_>>(),
        &rows[1..],
    );
    let hash = fnv1a_rows(&rows[1..]);
    rows.push(format!(
        "stemsbeambuilderpagesummary {page} systems {} {} hash {hash:016x}",
        native.builders.systems.len(),
        totals.summary_fields(),
    ));
    rows
}

#[allow(clippy::too_many_arguments)]
fn append_builder_rows(
    page: &str,
    native: &NativePage,
    system: &NativeStemsBeamBuilderSystem,
    stumps: &NativeStemsBeamStumpSystem,
    v_linkers: &NativeStemsBeamVLinkerSystem,
    reachability: &audiveris_omr::native_stems_beam_reachability::NativeStemsBeamReachabilitySystem,
    builder: &NativeStemsBeamBuilder,
    chunk_aliases: &mut Vec<(GlyphData, String)>,
    glyph_origins: &mut GlyphOriginLedger,
    rows: &mut Vec<String>,
) {
    rows.push(builder_row(
        page,
        native,
        system,
        stumps,
        v_linkers,
        reachability,
        builder,
    ));
    append_alignment_rows(
        page,
        native,
        system,
        v_linkers,
        builder,
        NativeStemsBeamBuilderAlignmentSubject::Seeds,
        rows,
    );
    for (ordinal, decision) in builder.target_filter.iter().enumerate() {
        let (alias, kind) = match decision.target {
            NativeStemsBeamBuilderTargetRef::Beam(reference) => (b_alias(stumps, reference), "B"),
            NativeStemsBeamBuilderTargetRef::Head(reference) => {
                (c_alias(reachability, reference), "C")
            }
        };
        rows.push(format!(
            "stemsbeambuildertargetfilter {page} system {} builder {} ordinal {ordinal} \
             alias {alias} kind {kind} stump {} removedByContent {} action {}",
            system.system_id,
            builder.builder_ordinal,
            decision.stump.map_or_else(
                || "-".to_owned(),
                |glyph| glyph_alias_ref(native, system, v_linkers, builder, glyph)
            ),
            decision.removed_by_content,
            if decision.included {
                "keep"
            } else {
                "removeUnalignedStump"
            },
        ));
    }
    append_sort_rows(
        page,
        native,
        system,
        stumps,
        v_linkers,
        reachability,
        builder,
        "targets",
        &builder.target_sort,
        rows,
    );
    for trace in &builder.seed_trace {
        rows.push(format!(
            "stemsbeambuilderseedfilter {page} system {} builder {} ordinal {} glyph {} \
             unalignedContent {} pastLastHeadContent {} retainedIdentity {}",
            system.system_id,
            builder.builder_ordinal,
            trace.input_ordinal,
            glyph_alias_ref(native, system, v_linkers, builder, trace.glyph),
            !trace.survives_alignment,
            trace.survives_alignment && !trace.survives_last_head_mutation,
            trace.survives_alignment && trace.survives_last_head_mutation,
        ));
    }
    append_section_row(page, native, system, v_linkers, builder, true, rows);
    append_section_row(page, native, system, v_linkers, builder, false, rows);
    append_filament_rows(
        page,
        native,
        system,
        builder,
        chunk_aliases,
        glyph_origins,
        rows,
    );
    append_alignment_rows(
        page,
        native,
        system,
        v_linkers,
        builder,
        NativeStemsBeamBuilderAlignmentSubject::Chunks,
        rows,
    );
    append_chunk_rows(page, native, system, v_linkers, builder, rows);
    append_seed_item_rows(
        page,
        native,
        system,
        stumps,
        v_linkers,
        reachability,
        builder,
        rows,
    );
    if builder.items_before_sort.len() > 1 {
        append_sort_rows(
            page,
            native,
            system,
            stumps,
            v_linkers,
            reachability,
            builder,
            "items",
            &builder.sort,
            rows,
        );
    }
    append_item_pre_rows(
        page,
        native,
        system,
        stumps,
        v_linkers,
        reachability,
        builder,
        rows,
    );
    append_gap_item_length_end_rows(
        page,
        native,
        system,
        stumps,
        v_linkers,
        reachability,
        builder,
        rows,
    );
}

#[allow(clippy::too_many_arguments)]
fn append_alignment_rows(
    page: &str,
    native: &NativePage,
    system: &NativeStemsBeamBuilderSystem,
    v_linkers: &NativeStemsBeamVLinkerSystem,
    builder: &NativeStemsBeamBuilder,
    subject: NativeStemsBeamBuilderAlignmentSubject,
    rows: &mut Vec<String>,
) {
    let pass = builder
        .alignment
        .iter()
        .find(|pass| pass.subject == subject)
        .expect("builder alignment pass");
    let start = builder
        .start_stump
        .map(|reference| glyph_data(native, system, v_linkers, builder, reference));
    let originally_present =
        match subject {
            NativeStemsBeamBuilderAlignmentSubject::Seeds => builder
                .input_seed_ordinals
                .iter()
                .any(|&free_glyph_ordinal| {
                    start.as_ref().is_some_and(|start| {
                        let candidate = glyph_data(
                            native,
                            system,
                            v_linkers,
                            builder,
                            NativeStemsBeamBuilderGlyphRef::StemSeed { free_glyph_ordinal },
                        );
                        same_glyph_content(start, &candidate)
                    })
                }),
            NativeStemsBeamBuilderAlignmentSubject::Chunks => builder.chunks.iter().any(|chunk| {
                !chunk.removed_by_seed_content
                    && start.as_ref().is_some_and(|start| {
                        start.bounds == chunk.bounds && start.run_table == chunk.run_table
                    })
            }),
        };
    let phase = match subject {
        NativeStemsBeamBuilderAlignmentSubject::Seeds => "seed",
        NativeStemsBeamBuilderAlignmentSubject::Chunks => "chunk",
    };
    for (ordinal, check) in pass.comparisons.iter().enumerate() {
        let first = glyph_data(native, system, v_linkers, builder, check.first);
        let second = glyph_data(native, system, v_linkers, builder, check.second);
        rows.push(format!(
            "stemsbeambuilderalign {page} system {} builder {} phase {phase} ordinal {ordinal} \
             startStump {} startOriginallyPresent {originally_present} first {} second {} \
             firstBounds {} secondBounds {} firstCentroid {} secondCentroid {} \
             firstDeskew {} secondDeskew {} dy {} maxDy {} dyBypass {} dx {} maxDx {} \
             aligned {} alien {} tieHeightRemoveSecond {}",
            system.system_id,
            builder.builder_ordinal,
            start.as_ref().map_or_else(|| "-".to_owned(), glyph_alias),
            glyph_alias(&first),
            glyph_alias(&second),
            bounds_token(first.bounds),
            bounds_token(second.bounds),
            tuple_point(check.first_centroid),
            tuple_point(check.second_centroid),
            tuple_point(check.first_deskewed),
            tuple_point(check.second_deskewed),
            java_hex_double(check.dy),
            java_hex_double(system.max_stem_alignment_dy),
            check.dy_bypasses_dx,
            if check.dy_bypasses_dx {
                "-".to_owned()
            } else {
                java_hex_double(check.dx)
            },
            java_hex_double(system.max_stem_alignment_dx),
            check.aligned,
            check.removed.map_or_else(
                || "-".to_owned(),
                |reference| glyph_alias_ref(native, system, v_linkers, builder, reference)
            ),
            check.equal_height_removed_second,
        ));
    }
}

fn append_chunk_rows(
    page: &str,
    native: &NativePage,
    system: &NativeStemsBeamBuilderSystem,
    v_linkers: &NativeStemsBeamVLinkerSystem,
    builder: &NativeStemsBeamBuilder,
    rows: &mut Vec<String>,
) {
    let start = builder
        .start_stump
        .map(|reference| glyph_data(native, system, v_linkers, builder, reference));
    let mut retained = builder
        .chunks
        .iter()
        .enumerate()
        .filter(|(_, chunk)| chunk.retained)
        .collect::<Vec<_>>();
    retained.sort_by_key(|(ordinal, chunk)| {
        if builder.y_direction > 0 {
            (chunk.bounds.y as i64, *ordinal as i64)
        } else {
            (
                -i64::try_from(chunk.bounds.y + chunk.bounds.height).unwrap(),
                *ordinal as i64,
            )
        }
    });
    for (ordinal, chunk) in builder.chunks.iter().enumerate() {
        let glyph = GlyphData {
            bounds: chunk.bounds,
            run_table: chunk.run_table.clone(),
        };
        let start_equal = start
            .as_ref()
            .is_some_and(|start| same_glyph_content(start, &glyph));
        let action = if chunk.removed_by_seed_content {
            "seedEqual"
        } else if chunk.removed_by_alignment_content {
            "unaligned"
        } else if chunk.removed_as_start_content {
            "startEqual"
        } else {
            "keep"
        };
        let final_ordinal = retained
            .iter()
            .position(|(candidate, _)| *candidate == ordinal)
            .map_or(-1, |value| value as i32);
        let (builder_ordinal, filament_ordinal) = match chunk.glyph {
            NativeStemsBeamBuilderGlyphRef::Chunk {
                builder_ordinal,
                filament_ordinal,
            } => (builder_ordinal, filament_ordinal),
            _ => panic!("non-chunk in chunk decisions"),
        };
        rows.push(format!(
            "stemsbeambuilderchunkfilter {page} system {} builder {} ordinal {ordinal} event {} \
             glyph {} seedContentEqual {} unalignedContent {} startContentEqual {start_equal} \
             finalOccurrenceOrdinal {final_ordinal} action {action} headParts NA",
            system.system_id,
            builder.builder_ordinal,
            chunk_event_alias(system.system_id, builder_ordinal, filament_ordinal),
            glyph_alias(&glyph),
            chunk.removed_by_seed_content,
            chunk.removed_by_alignment_content,
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn append_seed_item_rows(
    page: &str,
    native: &NativePage,
    system: &NativeStemsBeamBuilderSystem,
    stumps: &NativeStemsBeamStumpSystem,
    v_linkers: &NativeStemsBeamVLinkerSystem,
    reachability: &audiveris_omr::native_stems_beam_reachability::NativeStemsBeamReachabilitySystem,
    builder: &NativeStemsBeamBuilder,
    rows: &mut Vec<String>,
) {
    let target_items = &builder.items_before_sort[1..1 + builder.targets_after_filter.len()];
    let mut ordinal = 0_usize;
    for decision in &builder.seed_filter {
        if matches!(
            decision.action,
            NativeStemsBeamBuilderSeedAction::AlignmentRemoved
                | NativeStemsBeamBuilderSeedAction::PastLastHead
        ) {
            continue;
        }
        let glyph = glyph_data(native, system, v_linkers, builder, decision.glyph);
        let duplicate = (decision.action
            == NativeStemsBeamBuilderSeedAction::DuplicateTargetIdentity)
            .then(|| {
                target_items
                    .iter()
                    .find(|item| {
                        item.glyph.is_some_and(|reference| {
                            let candidate =
                                glyph_data(native, system, v_linkers, builder, reference);
                            same_glyph_content(&glyph, &candidate)
                        })
                    })
                    .expect("duplicate target seed")
            });
        let overlap = decision.action == NativeStemsBeamBuilderSeedAction::OverlapsStart;
        let action = match decision.action {
            NativeStemsBeamBuilderSeedAction::DuplicateTargetIdentity => "duplicateTargetIdentity",
            NativeStemsBeamBuilderSeedAction::OverlapsStart => "startYOverlap",
            NativeStemsBeamBuilderSeedAction::ZeroContribution => "zeroContrib",
            NativeStemsBeamBuilderSeedAction::Item => "keep",
            NativeStemsBeamBuilderSeedAction::AlignmentRemoved
            | NativeStemsBeamBuilderSeedAction::PastLastHead => unreachable!(),
        };
        rows.push(format!(
            "stemsbeambuilderseeditemfilter {page} system {} builder {} ordinal {ordinal} \
             glyph {} duplicateTargetIdentity {} duplicateAlias {} startYOverlap {} \
             contrib {} action {action}",
            system.system_id,
            builder.builder_ordinal,
            glyph_alias(&glyph),
            duplicate.is_some(),
            duplicate.map_or_else(
                || "-".to_owned(),
                |item| item_alias(
                    native,
                    system,
                    stumps,
                    v_linkers,
                    reachability,
                    builder,
                    item
                )
            ),
            if duplicate.is_some() {
                "-".to_owned()
            } else {
                overlap.to_string()
            },
            if duplicate.is_some() || overlap {
                -1
            } else {
                decision.contribution.expect("seed item contribution")
            },
        ));
        ordinal += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn append_item_pre_rows(
    page: &str,
    native: &NativePage,
    system: &NativeStemsBeamBuilderSystem,
    stumps: &NativeStemsBeamStumpSystem,
    v_linkers: &NativeStemsBeamVLinkerSystem,
    reachability: &audiveris_omr::native_stems_beam_reachability::NativeStemsBeamReachabilitySystem,
    builder: &NativeStemsBeamBuilder,
    rows: &mut Vec<String>,
) {
    let start = &builder.items_before_sort[0];
    append_item_pre_row(
        page,
        native,
        system,
        stumps,
        v_linkers,
        reachability,
        builder,
        0,
        0,
        start,
        rows,
    );
    let mut sorted = builder.sort.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|entry| entry.after_ordinal);
    for entry in sorted {
        append_item_pre_row(
            page,
            native,
            system,
            stumps,
            v_linkers,
            reachability,
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
    native: &NativePage,
    system: &NativeStemsBeamBuilderSystem,
    stumps: &NativeStemsBeamStumpSystem,
    v_linkers: &NativeStemsBeamVLinkerSystem,
    reachability: &audiveris_omr::native_stems_beam_reachability::NativeStemsBeamReachabilitySystem,
    builder: &NativeStemsBeamBuilder,
    creation: usize,
    sorted: usize,
    item: &NativeStemsBeamBuilderItem,
    rows: &mut Vec<String>,
) {
    rows.push(format!(
        "stemsbeambuilderitempre {page} system {} builder {} creationOrdinal {creation} \
         sortedOrdinal {sorted} kind {} alias {} line {} glyph {} contrib {}",
        system.system_id,
        builder.builder_ordinal,
        item_kind(item.kind),
        item_alias(
            native,
            system,
            stumps,
            v_linkers,
            reachability,
            builder,
            item
        ),
        stem_line(item.line),
        item.glyph.map_or_else(
            || "-".to_owned(),
            |reference| glyph_alias_ref(native, system, v_linkers, builder, reference)
        ),
        item.contribution,
    ));
}

#[allow(clippy::too_many_arguments)]
fn append_gap_item_length_end_rows(
    page: &str,
    native: &NativePage,
    system: &NativeStemsBeamBuilderSystem,
    stumps: &NativeStemsBeamStumpSystem,
    v_linkers: &NativeStemsBeamVLinkerSystem,
    reachability: &audiveris_omr::native_stems_beam_reachability::NativeStemsBeamReachabilitySystem,
    builder: &NativeStemsBeamBuilder,
    rows: &mut Vec<String>,
) {
    let mut sorted = vec![builder.items_before_sort[0].clone()];
    let mut sorted_tail = builder.sort.iter().collect::<Vec<_>>();
    sorted_tail.sort_by_key(|entry| entry.after_ordinal);
    sorted.extend(sorted_tail.into_iter().map(|entry| entry.item.clone()));
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
        let gap = last.map(|last| f64::from(builder.y_direction) * (start.y - last.y));
        let action = if last.is_none() {
            "initial"
        } else if gap.is_some_and(|gap| gap > f64::from(max_gap)) {
            "truncate"
        } else if gap.is_some_and(|gap| gap > 0.01) {
            "insert"
        } else {
            "contiguous"
        };
        let inserted = (action == "insert").then(|| {
            if builder.y_direction > 0 {
                NativeStemLine {
                    start: last.unwrap(),
                    stop: start,
                }
            } else {
                NativeStemLine {
                    start,
                    stop: last.unwrap(),
                }
            }
        });
        rows.push(format!(
            "stemsbeambuildergap {page} system {} builder {} ordinal {ordinal} itemIndex {ordinal} \
             itemAlias {} start {} stop {} lastBefore {} gap {} maxGap {max_gap} epsilon {} \
             action {action} insertedLine {} insertedContrib {}",
            system.system_id,
            builder.builder_ordinal,
            item_alias(
                native,
                system,
                stumps,
                v_linkers,
                reachability,
                builder,
                item
            ),
            point(start),
            point(stop),
            last.map_or_else(|| "-".to_owned(), point),
            optional_double_token(gap),
            java_hex_double(0.01),
            inserted.map_or_else(|| "-".to_owned(), stem_line),
            inserted.map_or_else(
                || "-".to_owned(),
                |line| line_bounds(line).height.to_string()
            ),
        ));
        if action == "truncate" {
            break;
        }
        if last.is_none_or(|last| f64::from(builder.y_direction) * (stop.y - last.y) > 0.01) {
            last = Some(stop);
        }
    }
    for (ordinal, item) in builder.items.iter().enumerate() {
        let alias = if item.kind == NativeStemsBeamBuilderItemKind::Gap {
            format!("gap:{ordinal}")
        } else {
            item_alias(
                native,
                system,
                stumps,
                v_linkers,
                reachability,
                builder,
                item,
            )
        };
        rows.push(format!(
            "stemsbeambuilderitem {page} system {} builder {} ordinal {ordinal} kind {} \
             alias {alias} line {} glyph {} contrib {}",
            system.system_id,
            builder.builder_ordinal,
            item_kind(item.kind),
            stem_line(item.line),
            item.glyph.map_or_else(
                || "-".to_owned(),
                |reference| glyph_alias_ref(native, system, v_linkers, builder, reference)
            ),
            item.contribution,
        ));
    }
    for profile in 0..=4 {
        rows.push(format!(
            "stemsbeambuilderlength {page} system {} builder {} profile {profile} threshold {} \
             length {}",
            system.system_id,
            builder.builder_ordinal,
            system.gap_map[&profile],
            builder.lengths[&profile],
        ));
    }
    let output_seeds = builder
        .seeds_after_filter
        .iter()
        .map(|&free_glyph_ordinal| {
            glyph_alias_ref(
                native,
                system,
                v_linkers,
                builder,
                NativeStemsBeamBuilderGlyphRef::StemSeed { free_glyph_ordinal },
            )
        })
        .collect::<Vec<_>>();
    let glyph_delta = builder
        .glyph_registrations
        .iter()
        .filter(|registration| {
            matches!(
                registration.action,
                NativeStemsBeamBuilderRegistrationAction::NewInModeledRegistry { .. }
            )
        })
        .count();
    rows.push(format!(
        "stemsbeambuilderend {page} system {} builder {} outputSeeds {} lastHeadY {} \
         inputTargets {} retainedTargetItems {} filaments {} glyphAttempts {} glyphDelta {} \
         sigVertexDelta {} sigEdgeDelta {} systemStemDelta {} linkMutations {} \
         unexpectedBuilderMutations {} noLinkMutation true noCBuilderMutation true",
        system.system_id,
        builder.builder_ordinal,
        string_list(&output_seeds),
        optional_double_token(builder.last_head_y),
        builder.target_input.len(),
        builder.targets_after_filter.len(),
        builder.filaments.len(),
        builder.glyph_registrations.len(),
        glyph_delta,
        builder.sig_mutation_count,
        builder.sig_mutation_count,
        builder.system_stem_mutation_count,
        builder.link_mutation_count,
        builder.head_builder_mutation_count,
    ));
}

fn append_filament_rows(
    page: &str,
    native: &NativePage,
    system: &NativeStemsBeamBuilderSystem,
    builder: &NativeStemsBeamBuilder,
    chunk_aliases: &mut Vec<(GlyphData, String)>,
    glyph_origins: &mut GlyphOriginLedger,
    rows: &mut Vec<String>,
) {
    for (filament, registration) in builder.filaments.iter().zip(&builder.glyph_registrations) {
        let mut member_aliases = Vec::new();
        for (ordinal, member) in filament.member_section_source_ordinals.iter().enumerate() {
            let (orientation, sources, sections, prefix) = match member.orientation {
                audiveris_image::run_table::Orientation::Horizontal => (
                    "HORIZONTAL",
                    &system.horizontal_section_source_ordinals,
                    &native.grid.peak_graph.horizontal_sections,
                    "h",
                ),
                audiveris_image::run_table::Orientation::Vertical => (
                    "VERTICAL",
                    &system.vertical_section_source_ordinals,
                    &native.grid.peak_graph.vertical_sections,
                    "v",
                ),
            };
            let local = sources
                .iter()
                .position(|&source| source == member.source_ordinal)
                .expect("filament member system section");
            let section = &sections[member.source_ordinal];
            let alias = format!("{prefix}:{local}");
            member_aliases.push(alias.clone());
            rows.push(format!(
                "stemsbeambuilderfilamentmember {page} system {} builder {} filament {} \
                 ordinal {ordinal} alias {alias} orientation {orientation} bounds {} \
                 weight {} runs {}",
                system.system_id,
                builder.builder_ordinal,
                filament.filament_ordinal,
                bounds_token(section.bounds()),
                section.weight(),
                section.run_count(),
            ));
        }
        let glyph = GlyphData {
            bounds: registration.bounds,
            run_table: registration.run_table.clone(),
        };
        let chunk_alias = chunk_aliases
            .iter()
            .find(|(candidate, _)| {
                candidate.bounds == glyph.bounds && candidate.run_table == glyph.run_table
            })
            .map(|(_, alias)| alias.clone())
            .unwrap_or_else(|| {
                let alias = format!(
                    "chunk:s{}:{}:{}",
                    system.system_id, builder.builder_ordinal, filament.filament_ordinal
                );
                chunk_aliases.push((glyph.clone(), alias.clone()));
                alias
            });
        let center_line = NativeStemLine {
            start: NativeStemPoint {
                x: filament.start.0 + 0.5,
                y: filament.start.1,
            },
            stop: NativeStemPoint {
                x: filament.stop.0 + 0.5,
                y: filament.stop.1 + 1.0,
            },
        };
        rows.push(format!(
            "stemsbeambuilderfilament {page} system {} builder {} ordinal {} \
             registrationDelta {} chunkAlias {chunk_alias} members {} bounds {} weight {} \
             centerLine {} start {} stop {} meanThickness {} meanDistance {} length {}",
            system.system_id,
            builder.builder_ordinal,
            filament.filament_ordinal,
            filament.filament_ordinal + 1,
            string_list(&member_aliases),
            bounds_token(filament.bounds),
            filament.weight,
            stem_line(center_line),
            tuple_point(filament.start),
            tuple_point(filament.stop),
            java_hex_double(filament.mean_thickness),
            java_hex_double(filament.mean_distance),
            filament.bounds.height,
        ));
        let (result, origins) = match registration.action {
            NativeStemsBeamBuilderRegistrationAction::NewInModeledRegistry { .. } => {
                assert!(
                    glyph_origins.categories(&glyph).is_none(),
                    "modeled New chunk already has a replayed origin"
                );
                ("New", "newStemBuilderChunk".to_owned())
            }
            NativeStemsBeamBuilderRegistrationAction::ReusedModeledCanonical => (
                "Reuse",
                glyph_origins
                    .categories(&glyph)
                    .expect("modeled Reuse chunk has replayed origin categories"),
            ),
        };
        rows.push(format!(
            "stemsbeambuilderglyphreg {page} system {} builder {} ordinal {} event {} \
             result {result} canonical {} originCategories {origins} bounds {} orientation {} \
             weight {} runs {} sha256 {}",
            system.system_id,
            builder.builder_ordinal,
            filament.filament_ordinal,
            chunk_event_alias(
                system.system_id,
                builder.builder_ordinal,
                filament.filament_ordinal
            ),
            glyph_alias(&glyph),
            bounds_token(registration.bounds),
            orientation_token(registration.run_table.orientation()),
            registration.weight,
            glyph_run_count(&registration.run_table),
            glyph_run_sha256(&registration.run_table),
        ));
        glyph_origins.add_persistent(glyph, "priorStemBuilderChunk");
    }
}

#[allow(clippy::too_many_arguments)]
fn append_section_row(
    page: &str,
    native: &NativePage,
    system: &NativeStemsBeamBuilderSystem,
    v_linkers: &NativeStemsBeamVLinkerSystem,
    builder: &NativeStemsBeamBuilder,
    vertical: bool,
    rows: &mut Vec<String>,
) {
    let scans = if vertical {
        &builder.vertical_sections
    } else {
        &builder.horizontal_sections
    };
    let start_bounds = builder
        .start_stump
        .map(|reference| glyph_data(native, system, v_linkers, builder, reference).bounds);
    let mut digest = Vec::new();
    let mut reasons = BTreeMap::<&'static str, usize>::new();
    let mut accepted = Vec::new();
    for (local_ordinal, scan) in scans.iter().enumerate() {
        let section = if vertical {
            &native.grid.peak_graph.vertical_sections[scan.source_ordinal]
        } else {
            &native.grid.peak_graph.horizontal_sections[scan.source_ordinal]
        };
        let bounds = scan.bounds;
        let width_accepted = scan.intersects_lookup
            && if vertical {
                i32::try_from(bounds.width).is_ok_and(|width| width <= system.max_stem_thickness)
            } else {
                bounds.width <= 1
            };
        let overlap = start_bounds.map(|start| y_overlap(bounds, start));
        let stump_reject = vertical
            && width_accepted
            && overlap.is_some_and(|overlap| overlap > 0)
            && start_bounds.is_some_and(|start| bounds.height < start.height);
        let center = (scan.intersects_lookup && width_accepted && !stump_reject)
            .then(|| section.centroid_2d());
        let last_head_reject = center
            .zip(builder.last_head_y)
            .map(|(center, last)| builder.y_direction * java_double_compare(center.1, last) >= 0);
        let action = if !scan.intersects_lookup {
            "outsideArea"
        } else if !width_accepted {
            "wide"
        } else if stump_reject {
            "shorterStumpOverlap"
        } else if last_head_reject == Some(true) {
            "pastLastHead"
        } else if vertical
            && scan
                .line_distance
                .is_some_and(|distance| distance > system.max_line_section_dx)
        {
            "lineDistance"
        } else {
            "accept"
        };
        *reasons.entry(action).or_default() += 1;
        if action == "accept" {
            accepted.push(local_ordinal);
        }
        let decision = if vertical {
            format!(
                "sourceOrdinal {local_ordinal} alias v:{local_ordinal} bounds {} areaIntersects {} \
                 width {} maxWidth {} widthAccepted {} stumpOverlap {} stumpHeight {} \
                 shorterStumpReject {} center {} lastHeadY {} lastHeadReject {} theoDistance {} \
                 maxLineSectionDx {} action {action}",
                bounds_token(bounds),
                scan.intersects_lookup,
                bounds.width,
                system.max_stem_thickness,
                optional_bool_token(scan.intersects_lookup.then_some(width_accepted)),
                optional_i32_token(width_accepted.then_some(overlap).flatten()),
                optional_usize_token(start_bounds.map(|start| start.height)),
                optional_bool_token(
                    (width_accepted && start_bounds.is_some()).then_some(stump_reject)
                ),
                center.map_or_else(|| "-".to_owned(), tuple_point),
                optional_double_token(builder.last_head_y),
                optional_bool_token(
                    center
                        .and(builder.last_head_y)
                        .map(|_| last_head_reject.unwrap())
                ),
                optional_double_token(if last_head_reject == Some(true) {
                    None
                } else {
                    scan.line_distance
                }),
                java_hex_double(system.max_line_section_dx),
            )
        } else {
            format!(
                "sourceOrdinal {local_ordinal} alias h:{local_ordinal} bounds {} areaIntersects {} \
                 width {} maxWidth 1 widthAccepted {} center {} lastHeadY {} \
                 lastHeadReject {} action {action}",
                bounds_token(bounds),
                scan.intersects_lookup,
                bounds.width,
                optional_bool_token(scan.intersects_lookup.then_some(width_accepted)),
                center.map_or_else(|| "-".to_owned(), tuple_point),
                optional_double_token(builder.last_head_y),
                optional_bool_token(
                    center
                        .and(builder.last_head_y)
                        .map(|_| last_head_reject.unwrap())
                ),
            )
        };
        digest.extend_from_slice(decision.as_bytes());
        digest.push(b'\n');
    }
    let reason_token = if reasons.is_empty() {
        "-".to_owned()
    } else {
        reasons
            .iter()
            .map(|(reason, count)| format!("{reason}:{count}"))
            .collect::<Vec<_>>()
            .join(",")
    };
    rows.push(format!(
        "stemsbeambuilder{}section {page} system {} builder {} sourceCount {} \
         acceptedCount {} acceptedSourceOrdinals {} reasons {reason_token} decisionSha256 {}",
        if vertical { "v" } else { "h" },
        system.system_id,
        builder.builder_ordinal,
        scans.len(),
        accepted.len(),
        usize_list(&accepted),
        sha256_hex(&digest),
    ));
}

#[allow(clippy::too_many_arguments)]
fn append_sort_rows(
    page: &str,
    native: &NativePage,
    system: &NativeStemsBeamBuilderSystem,
    stumps: &NativeStemsBeamStumpSystem,
    v_linkers: &NativeStemsBeamVLinkerSystem,
    reachability: &audiveris_omr::native_stems_beam_reachability::NativeStemsBeamReachabilitySystem,
    builder: &NativeStemsBeamBuilder,
    phase: &str,
    permutation: &[NativeStemsBeamBuilderSortEntry],
    rows: &mut Vec<String>,
) {
    let mut entries = permutation.iter().collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.before_ordinal);
    let mut cycles = 0_usize;
    let mut inconsistencies = 0_usize;
    let mut offender = Vec::new();
    for i in 0..entries.len() {
        for j in i + 1..entries.len() {
            for k in j + 1..entries.len() {
                let ij = item_compare(&entries[i].item, &entries[j].item, builder.y_direction);
                let jk = item_compare(&entries[j].item, &entries[k].item, builder.y_direction);
                let ki = item_compare(&entries[k].item, &entries[i].item, builder.y_direction);
                let ik = item_compare(&entries[i].item, &entries[k].item, builder.y_direction);
                let ji = -ij;
                let kj = -jk;
                let cycle = (ij < 0 && jk < 0 && ki < 0) || (ij > 0 && jk > 0 && ki > 0);
                let inconsistent = (ij == 0 && (ik != jk || -ik != ki))
                    || (jk == 0 && (ji != ki || ij != ik))
                    || (ik == 0 && (ij != kj || ji != jk));
                if cycle || inconsistent {
                    offender.extend_from_slice(
                        format!("{i}:{j}:{k}:{ij}:{jk}:{ki}:{ik}:{cycle}:{inconsistent}\n")
                            .as_bytes(),
                    );
                }
                cycles += usize::from(cycle);
                inconsistencies += usize::from(inconsistent);
            }
        }
    }
    rows.push(format!(
        "stemsbeambuildersortaudit {page} system {} builder {} phase {phase} items {} \
         strictCycles {cycles} equivalenceInconsistencies {inconsistencies} offenderSha256 {}",
        system.system_id,
        builder.builder_ordinal,
        entries.len(),
        sha256_hex(&offender),
    ));
    for entry in entries {
        let item = &entry.item;
        let alias = item_alias(
            native,
            system,
            stumps,
            v_linkers,
            reachability,
            builder,
            item,
        );
        let mut equal_inputs = permutation
            .iter()
            .filter(|other| {
                other.before_ordinal != entry.before_ordinal
                    && item_compare(&other.item, item, builder.y_direction) == 0
            })
            .map(|other| other.before_ordinal)
            .collect::<Vec<_>>();
        equal_inputs.sort_unstable();
        let stable_predecessors = equal_inputs
            .iter()
            .copied()
            .filter(|&other| other < entry.before_ordinal)
            .collect::<Vec<_>>();
        let reference = target_reference_point(v_linkers, reachability, item);
        rows.push(format!(
            "stemsbeambuildersort {page} system {} builder {} phase {phase} input {} \
             output {} alias {alias} kind {} line {} ref {} key1 {} key2 {} \
             equalInputs {} stableEqualPredecessors {}",
            system.system_id,
            builder.builder_ordinal,
            entry.before_ordinal,
            entry.after_ordinal,
            item_kind(item.kind),
            stem_line(item.line),
            reference.map_or_else(|| "-".to_owned(), point),
            java_hex_double(item.line.start.y),
            java_hex_double(item.line.stop.y),
            usize_list(&equal_inputs),
            usize_list(&stable_predecessors),
        ));
    }
    let _ = native;
}

fn item_compare(
    left: &NativeStemsBeamBuilderItem,
    right: &NativeStemsBeamBuilderItem,
    y_direction: i32,
) -> i32 {
    let key = |item: &NativeStemsBeamBuilderItem| {
        if matches!(
            item.kind,
            NativeStemsBeamBuilderItemKind::StartHalfLinker
                | NativeStemsBeamBuilderItemKind::HeadHalfLinker
        ) {
            item.reference_point.expect("half-linker reference").y
        } else if y_direction > 0 {
            item.line.start.y
        } else {
            item.line.stop.y
        }
    };
    y_direction * java_double_compare(key(left), key(right))
}

fn item_kind(kind: NativeStemsBeamBuilderItemKind) -> &'static str {
    match kind {
        NativeStemsBeamBuilderItemKind::StartHalfLinker => "startHalf",
        NativeStemsBeamBuilderItemKind::BeamLinker => "B",
        NativeStemsBeamBuilderItemKind::HeadHalfLinker => "C",
        NativeStemsBeamBuilderItemKind::SeedGlyph => "seed",
        NativeStemsBeamBuilderItemKind::ChunkGlyph => "chunk",
        NativeStemsBeamBuilderItemKind::Gap => "gap",
    }
}

fn item_alias(
    native: &NativePage,
    system: &NativeStemsBeamBuilderSystem,
    stumps: &NativeStemsBeamStumpSystem,
    v_linkers: &NativeStemsBeamVLinkerSystem,
    reachability: &audiveris_omr::native_stems_beam_reachability::NativeStemsBeamReachabilitySystem,
    builder: &NativeStemsBeamBuilder,
    item: &NativeStemsBeamBuilderItem,
) -> String {
    if item.kind == NativeStemsBeamBuilderItemKind::StartHalfLinker {
        return "start".to_owned();
    }
    if let Some(target) = item.target {
        return match target {
            NativeStemsBeamBuilderTargetRef::Beam(reference) => b_alias(stumps, reference),
            NativeStemsBeamBuilderTargetRef::Head(reference) => c_alias(reachability, reference),
        };
    }
    match item.glyph.expect("glyph item reference") {
        NativeStemsBeamBuilderGlyphRef::Chunk {
            builder_ordinal,
            filament_ordinal,
        } => chunk_event_alias(system.system_id, builder_ordinal, filament_ordinal),
        reference @ NativeStemsBeamBuilderGlyphRef::StemSeed { .. } => {
            glyph_alias_ref(native, system, v_linkers, builder, reference)
        }
        reference => format!("{reference:?}:{}", builder.builder_ordinal),
    }
}

fn target_reference_point(
    v_linkers: &NativeStemsBeamVLinkerSystem,
    reachability: &audiveris_omr::native_stems_beam_reachability::NativeStemsBeamReachabilitySystem,
    item: &NativeStemsBeamBuilderItem,
) -> Option<NativeStemPoint> {
    match item.target? {
        NativeStemsBeamBuilderTargetRef::Head(_) => item.reference_point,
        NativeStemsBeamBuilderTargetRef::Beam(reference) => v_linkers
            .constructors
            .iter()
            .find(|beam| beam.source == reference.beam)
            .and_then(|beam| beam.b_linkers.iter().find(|b| b.reference == reference))
            .map(|b| b.reference_point)
            .or_else(|| {
                reachability
                    .final_beam_arenas
                    .iter()
                    .find(|arena| arena.beam == reference.beam)
                    .and_then(|arena| {
                        arena
                            .all_b_linkers
                            .iter()
                            .find(|entry| entry.reference == reference)
                    })
                    .map(|entry| entry.reference_point)
            }),
    }
}

fn system_row(page: &str, system: &NativeStemsBeamBuilderSystem, profile: i32) -> String {
    format!(
        "stemsbeambuildersystem {page} system {} profile {profile} interline {} \
         maxStemThickness {} maxLineSectionDx {} maxStemAlignmentDx {} \
         maxStemAlignmentDy {} minLinkerLength {} minCoreSectionLength 0 \
         minSideRatio {} gap0 {} gap1 {} gap2 {} gap3 {} gap4 {} \
         saveConnections false verticalSections {} horizontalSections {} \
         verticalGlobalOrdinals {} horizontalGlobalOrdinals {}",
        system.system_id,
        system.interline,
        system.max_stem_thickness,
        java_hex_double(system.max_line_section_dx),
        java_hex_double(system.max_stem_alignment_dx),
        java_hex_double(system.max_stem_alignment_dy),
        java_rint(f64::from(system.interline) * 0.85),
        java_hex_double(0.4),
        system.gap_map[&0],
        system.gap_map[&1],
        system.gap_map[&2],
        system.gap_map[&3],
        system.gap_map[&4],
        system.vertical_section_source_ordinals.len(),
        system.horizontal_section_source_ordinals.len(),
        usize_list(&system.vertical_section_source_ordinals),
        usize_list(&system.horizontal_section_source_ordinals),
    )
}

#[allow(clippy::too_many_arguments)]
fn builder_row(
    page: &str,
    native: &NativePage,
    system: &NativeStemsBeamBuilderSystem,
    stumps: &NativeStemsBeamStumpSystem,
    v_linkers: &NativeStemsBeamVLinkerSystem,
    reachability: &audiveris_omr::native_stems_beam_reachability::NativeStemsBeamReachabilitySystem,
    builder: &NativeStemsBeamBuilder,
) -> String {
    let source = builder.start.b_linker.beam;
    let beam = stump_beam(stumps, source);
    let seeds = builder
        .input_seed_ordinals
        .iter()
        .map(|&free_glyph_ordinal| {
            glyph_alias_ref(
                native,
                system,
                v_linkers,
                builder,
                NativeStemsBeamBuilderGlyphRef::StemSeed { free_glyph_ordinal },
            )
        })
        .collect::<Vec<_>>();
    let input_bs = builder
        .target_input
        .iter()
        .filter_map(|target| match target {
            NativeStemsBeamBuilderTargetRef::Beam(reference) => Some(b_alias(stumps, *reference)),
            NativeStemsBeamBuilderTargetRef::Head(_) => None,
        })
        .collect::<Vec<_>>();
    let input_cs = builder
        .target_input
        .iter()
        .filter_map(|target| match target {
            NativeStemsBeamBuilderTargetRef::Beam(_) => None,
            NativeStemsBeamBuilderTargetRef::Head(reference) => {
                Some(c_alias(reachability, *reference))
            }
        })
        .collect::<Vec<_>>();
    format!(
        "stemsbeambuilder {page} system {} inspectOrdinal {} beamSig {} bOrdinal {} \
         vSide {} profile {} vYDir {} builderYDir {} directionDiverges {} \
         theo {} luBounds {} yRange {} \
         startStump {} inputSeeds {} inputBTargets {} inputCTargets {}",
        system.system_id,
        builder.builder_ordinal,
        beam.sig_ordinal,
        builder.start.b_linker.id - 1,
        vertical_side(builder.start.side),
        builder.max_stem_profile,
        builder.v_y_direction,
        builder.y_direction,
        builder.v_y_direction != builder.y_direction,
        stem_line(builder.theoretical_line),
        rectangle(builder.lookup_bounds),
        rectangle(builder.y_range),
        builder.start_stump.map_or_else(
            || "-".to_owned(),
            |reference| glyph_alias_ref(native, system, v_linkers, builder, reference)
        ),
        string_list(&seeds),
        string_list(&input_bs),
        string_list(&input_cs),
    )
}

#[derive(Clone)]
struct GlyphData {
    bounds: Bounds,
    run_table: RunTable,
}

#[derive(Default)]
struct GlyphOriginLedger {
    persistent: Vec<(GlyphData, BTreeSet<&'static str>)>,
    current_system: Vec<(GlyphData, BTreeSet<&'static str>)>,
}

impl GlyphOriginLedger {
    fn from_pre_stems(_native: &NativePage) -> Self {
        Self::default()
    }

    fn replay_pre_builder(
        &mut self,
        native: &NativePage,
        system: &NativeStemsBeamBuilderSystem,
        beam_stumps: &NativeStemsBeamStumpSystem,
        head_stumps: &audiveris_omr::native_stems_head_stumps::NativeStemsHeadStumpSystem,
    ) {
        self.current_system.clear();
        let seed_system = native
            .stem_seeds
            .systems
            .iter()
            .find(|candidate| candidate.raw.system_id == system.system_id)
            .expect("builder seed system");
        let purged_seeds = native
            .head_seeds
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system.system_id)
            .expect("builder purged-seed system");
        for &free_glyph_ordinal in &purged_seeds.kept_seed_ordinals {
            self.add_current(stem_seed_glyph(seed_system, free_glyph_ordinal), "stemSeed");
        }

        // A failed head build creates a distinct unattached Java glyph only
        // when its registration is New. A Reuse failure does not add a
        // rejected provenance label to the already-canonical source object.
        for registration in &system.pre_builder_glyph_registrations {
            if matches!(
                (
                    registration.source,
                    registration.attachment,
                    registration.action
                ),
                (
                    NativeStemsBeamBuilderPreBuilderGlyphSource::HeadStump { .. },
                    NativeStemsBeamBuilderPreBuilderAttachment::RejectedAfterRegistration,
                    NativeStemsBeamBuilderRegistrationAction::NewInModeledRegistry { .. }
                )
            ) {
                self.add_persistent(
                    GlyphData {
                        bounds: registration.bounds,
                        run_table: registration.run_table.clone(),
                    },
                    "rejectedHeadStump",
                );
            }
        }

        // Attached seed stumps do not perform another registration. Replay
        // the final stump topology as well as built candidates so these
        // category memberships are not lost.
        for beam in &beam_stumps.beams_by_abscissa {
            for stump in &beam.stumps {
                let glyph = match &stump.reference {
                    NativeStemsBeamStumpRef::Seed {
                        free_glyph_ordinal, ..
                    } => stem_seed_glyph(seed_system, *free_glyph_ordinal),
                    reference @ NativeStemsBeamStumpRef::Built { .. } => {
                        let candidate = beam
                            .sides
                            .iter()
                            .find(|side| side.final_stump.as_ref() == Some(reference))
                            .and_then(|side| side.build.as_ref())
                            .and_then(|build| build.candidate.as_ref())
                            .expect("attached built beam stump candidate");
                        GlyphData {
                            bounds: candidate.bounds,
                            run_table: candidate.run_table.clone(),
                        }
                    }
                };
                self.add_current(glyph, "beamStump");
            }
        }
        for head in &head_stumps.heads_by_abscissa {
            for corner in &head.corners_in_constructor_order {
                let glyph = match corner.outcome {
                    audiveris_omr::native_stems_head_stumps::NativeStemsHeadStumpOutcome::Seed {
                        free_glyph_ordinal,
                    } => Some(stem_seed_glyph(seed_system, free_glyph_ordinal)),
                    audiveris_omr::native_stems_head_stumps::NativeStemsHeadStumpOutcome::Built {
                        ..
                    } => corner.build.as_ref().and_then(|build| {
                        build.candidate.as_ref().map(|candidate| GlyphData {
                            bounds: candidate.bounds,
                            run_table: candidate.run_table.clone(),
                        })
                    }),
                    audiveris_omr::native_stems_head_stumps::NativeStemsHeadStumpOutcome::None => {
                        None
                    }
                };
                if let Some(glyph) = glyph {
                    self.add_current(glyph, "headStump");
                }
            }
        }
    }

    fn add_persistent(&mut self, glyph: GlyphData, category: &'static str) {
        Self::add_to(&mut self.persistent, glyph, category);
    }

    fn add_current(&mut self, glyph: GlyphData, category: &'static str) {
        Self::add_to(&mut self.current_system, glyph, category);
    }

    fn add_to(
        entries: &mut Vec<(GlyphData, BTreeSet<&'static str>)>,
        glyph: GlyphData,
        category: &'static str,
    ) {
        if let Some((_, categories)) = entries
            .iter_mut()
            .find(|(candidate, _)| same_glyph_content(candidate, &glyph))
        {
            categories.insert(category);
        } else {
            entries.push((glyph, BTreeSet::from([category])));
        }
    }

    fn categories(&self, glyph: &GlyphData) -> Option<String> {
        let mut categories = BTreeSet::new();
        for entries in [&self.persistent, &self.current_system] {
            if let Some((_, matching)) = entries
                .iter()
                .find(|(candidate, _)| same_glyph_content(candidate, glyph))
            {
                categories.extend(matching.iter().copied());
            }
        }
        (!categories.is_empty()).then(|| categories.iter().copied().collect::<Vec<_>>().join(","))
    }
}

fn stem_seed_glyph(
    system: &audiveris_omr::native_stem_seeds::NativeStemSeedSystemRecognition,
    free_glyph_ordinal: usize,
) -> GlyphData {
    let glyph = &system.free_glyphs[free_glyph_ordinal];
    GlyphData {
        bounds: glyph.bounds,
        run_table: glyph.run_table.clone(),
    }
}

fn same_glyph_content(left: &GlyphData, right: &GlyphData) -> bool {
    left.bounds == right.bounds && left.run_table == right.run_table
}

fn glyph_alias_ref(
    native: &NativePage,
    system: &NativeStemsBeamBuilderSystem,
    v_linkers: &NativeStemsBeamVLinkerSystem,
    builder: &NativeStemsBeamBuilder,
    reference: NativeStemsBeamBuilderGlyphRef,
) -> String {
    glyph_alias(&glyph_data(native, system, v_linkers, builder, reference))
}

fn glyph_data(
    native: &NativePage,
    system: &NativeStemsBeamBuilderSystem,
    v_linkers: &NativeStemsBeamVLinkerSystem,
    builder: &NativeStemsBeamBuilder,
    reference: NativeStemsBeamBuilderGlyphRef,
) -> GlyphData {
    match reference {
        NativeStemsBeamBuilderGlyphRef::StemSeed { free_glyph_ordinal } => {
            let seed_system = native
                .stem_seeds
                .systems
                .iter()
                .find(|candidate| candidate.raw.system_id == system.system_id)
                .expect("builder seed system");
            let glyph = &seed_system.free_glyphs[free_glyph_ordinal];
            GlyphData {
                bounds: glyph.bounds,
                run_table: glyph.run_table.clone(),
            }
        }
        NativeStemsBeamBuilderGlyphRef::BeamStump { b_linker } => {
            let b = v_linkers
                .constructors
                .iter()
                .find(|beam| beam.source == b_linker.beam)
                .and_then(|beam| beam.b_linkers.iter().find(|b| b.reference == b_linker))
                .expect("builder beam BLinker");
            match b.stump.as_ref().expect("builder B stump") {
                NativeStemsBeamStumpRef::Seed {
                    free_glyph_ordinal, ..
                } => glyph_data(
                    native,
                    system,
                    v_linkers,
                    builder,
                    NativeStemsBeamBuilderGlyphRef::StemSeed {
                        free_glyph_ordinal: *free_glyph_ordinal,
                    },
                ),
                stump @ NativeStemsBeamStumpRef::Built { .. } => {
                    let beam = native
                        .beam_stumps
                        .systems
                        .iter()
                        .find(|candidate| candidate.system_id == system.system_id)
                        .map(|candidate| stump_beam(candidate, b_linker.beam))
                        .expect("builder stump beam");
                    let candidate = beam
                        .sides
                        .iter()
                        .find(|side| side.final_stump.as_ref() == Some(stump))
                        .and_then(|side| side.build.as_ref())
                        .and_then(|build| build.candidate.as_ref())
                        .expect("built beam stump glyph");
                    GlyphData {
                        bounds: candidate.bounds,
                        run_table: candidate.run_table.clone(),
                    }
                }
            }
        }
        NativeStemsBeamBuilderGlyphRef::HeadStump { corner } => {
            let corner_system = native
                .corners
                .systems
                .iter()
                .find(|candidate| candidate.system_id == system.system_id)
                .expect("builder corner system");
            let constructor_ordinal = corner_system.heads_in_sig_order[corner.sig_ordinal]
                .corners_in_constructor_order
                .iter()
                .find(|candidate| {
                    candidate.horizontal == corner.horizontal
                        && candidate.vertical == corner.vertical
                })
                .expect("builder head corner")
                .constructor_ordinal;
            let stump_system = native
                .head_stumps
                .systems
                .iter()
                .find(|candidate| candidate.system_id == system.system_id)
                .expect("builder head-stump system");
            let stump = stump_system
                .heads_by_abscissa
                .iter()
                .find(|head| head.sig_ordinal == corner.sig_ordinal)
                .and_then(|head| {
                    head.corners_in_constructor_order
                        .iter()
                        .find(|candidate| candidate.constructor_ordinal == constructor_ordinal)
                })
                .expect("builder head stump");
            match &stump.outcome {
                audiveris_omr::native_stems_head_stumps::NativeStemsHeadStumpOutcome::Seed {
                    free_glyph_ordinal,
                } => glyph_data(
                    native,
                    system,
                    v_linkers,
                    builder,
                    NativeStemsBeamBuilderGlyphRef::StemSeed {
                        free_glyph_ordinal: *free_glyph_ordinal,
                    },
                ),
                audiveris_omr::native_stems_head_stumps::NativeStemsHeadStumpOutcome::Built {
                    ..
                } => {
                    let candidate = stump
                        .build
                        .as_ref()
                        .and_then(|build| build.candidate.as_ref())
                        .expect("built head stump glyph");
                    GlyphData {
                        bounds: candidate.bounds,
                        run_table: candidate.run_table.clone(),
                    }
                }
                audiveris_omr::native_stems_head_stumps::NativeStemsHeadStumpOutcome::None => {
                    panic!("missing builder head stump glyph")
                }
            }
        }
        NativeStemsBeamBuilderGlyphRef::Chunk {
            builder_ordinal,
            filament_ordinal,
        } => {
            let source_builder = &system.builders[builder_ordinal];
            let registration = source_builder
                .glyph_registrations
                .iter()
                .find(|registration| {
                    registration.glyph
                        == NativeStemsBeamBuilderGlyphRef::Chunk {
                            builder_ordinal,
                            filament_ordinal,
                        }
                })
                .or_else(|| {
                    builder.glyph_registrations.iter().find(|registration| {
                        registration.glyph
                            == NativeStemsBeamBuilderGlyphRef::Chunk {
                                builder_ordinal,
                                filament_ordinal,
                            }
                    })
                })
                .expect("builder chunk glyph");
            GlyphData {
                bounds: registration.bounds,
                run_table: registration.run_table.clone(),
            }
        }
    }
}

fn stump_beam(
    system: &NativeStemsBeamStumpSystem,
    source: NativeStemsBeamSource,
) -> &audiveris_omr::native_stems_beam_stumps::NativeStemsBeamStumpBeam {
    system
        .beams_by_abscissa
        .iter()
        .find(|beam| beam.source == source)
        .expect("builder beam source")
}

fn b_alias(system: &NativeStemsBeamStumpSystem, reference: NativeStemsBeamBLinkerRef) -> String {
    format!(
        "beam:{}:b:{}",
        stump_beam(system, reference.beam).sig_ordinal,
        reference.id - 1
    )
}

fn c_alias(
    system: &audiveris_omr::native_stems_beam_reachability::NativeStemsBeamReachabilitySystem,
    reference: audiveris_omr::native_stems_beam_reachability::NativeStemsBeamHeadCornerRef,
) -> String {
    let head = system
        .heads_in_abscissa_order
        .iter()
        .find(|head| head.sig_ordinal == reference.sig_ordinal)
        .expect("builder head x ordinal");
    format!(
        "h:{}:{}:{}",
        head.x_ordinal,
        head_side(reference.horizontal),
        vertical_side(reference.vertical)
    )
}

fn native_page(image: &str) -> NativePage {
    let grid = recognize_grid_lines(repo_path(&format!("data/examples/{image}")))
        .unwrap_or_else(|error| panic!("{image}: GRID failed: {error}"));
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
    let reachability =
        materialize_native_stems_beam_reachability(&beams, &beam_stumps, &beam_vlinkers, &corners)
            .unwrap_or_else(|error| panic!("{image}: STEMS beam reachability failed: {error}"));
    let head_stumps =
        materialize_native_stems_head_stumps(&grid, &stem_seeds, &corners, &head_seeds)
            .unwrap_or_else(|error| panic!("{image}: STEMS head stumps failed: {error}"));
    let builders = materialize_native_stems_beam_builders(
        &grid,
        &beams,
        &ledgers,
        &heads,
        &stem_seeds,
        &beam_stumps,
        &beam_vlinkers,
        &corners,
        &head_stumps,
        &reachability,
    )
    .unwrap_or_else(|error| panic!("{image}: STEMS beam builders failed: {error}"));
    NativePage {
        grid,
        stem_seeds,
        beams,
        ledgers,
        heads,
        corners,
        head_seeds,
        beam_stumps,
        beam_vlinkers,
        reachability,
        head_stumps,
        builders,
    }
}

fn glyph_alias(glyph: &GlyphData) -> String {
    format!(
        "g:{}:{}:{}:{}:{}",
        glyph.bounds.x,
        glyph.bounds.y,
        glyph.bounds.width,
        glyph.bounds.height,
        glyph_run_sha256(&glyph.run_table),
    )
}

fn glyph_run_sha256(table: &RunTable) -> String {
    let orientation = match table.orientation() {
        audiveris_image::run_table::Orientation::Horizontal => "HORIZONTAL",
        audiveris_image::run_table::Orientation::Vertical => "VERTICAL",
    };
    let mut bytes = format!("{orientation} {} {}\n", table.width(), table.height()).into_bytes();
    for sequence in 0..table.sequence_count() {
        let mut row = sequence.to_string();
        for run in table.sequence(sequence).unwrap_or_default() {
            row.push_str(&format!(" {}:{}", run.start, run.length));
        }
        row.push('\n');
        bytes.extend_from_slice(row.as_bytes());
    }
    sha256_hex(&bytes)
}

fn glyph_run_count(table: &RunTable) -> usize {
    (0..table.sequence_count())
        .map(|sequence| table.sequence(sequence).unwrap_or_default().len())
        .sum()
}

fn orientation_token(orientation: audiveris_image::run_table::Orientation) -> &'static str {
    match orientation {
        audiveris_image::run_table::Orientation::Horizontal => "HORIZONTAL",
        audiveris_image::run_table::Orientation::Vertical => "VERTICAL",
    }
}

fn chunk_event_alias(system_id: usize, builder: usize, filament: usize) -> String {
    format!("chunkEvent:s{system_id}:{builder}:{filament}")
}

fn head_side(side: NativeStemHeadSide) -> &'static str {
    match side {
        NativeStemHeadSide::Left => "LEFT",
        NativeStemHeadSide::Right => "RIGHT",
    }
}

fn vertical_side(side: NativeStemVerticalSide) -> &'static str {
    match side {
        NativeStemVerticalSide::Top => "TOP",
        NativeStemVerticalSide::Bottom => "BOTTOM",
    }
}

fn rectangle(value: JavaRectangle) -> String {
    format!("{}:{}:{}:{}", value.x, value.y, value.width, value.height)
}

fn bounds_token(value: Bounds) -> String {
    format!("{}:{}:{}:{}", value.x, value.y, value.width, value.height)
}

fn point(value: NativeStemPoint) -> String {
    format!("{}:{}", java_hex_double(value.x), java_hex_double(value.y))
}

fn stem_line(value: NativeStemLine) -> String {
    format!("{}:{}", point(value.start), point(value.stop))
}

fn tuple_point(value: (f64, f64)) -> String {
    format!("{}:{}", java_hex_double(value.0), java_hex_double(value.1))
}

fn optional_bool_token(value: Option<bool>) -> String {
    value.map_or_else(|| "-".to_owned(), |value| value.to_string())
}

fn optional_i32_token(value: Option<i32>) -> String {
    value.map_or_else(|| "-".to_owned(), |value| value.to_string())
}

fn optional_usize_token(value: Option<usize>) -> String {
    value.map_or_else(|| "-".to_owned(), |value| value.to_string())
}

fn optional_double_token(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_owned(), java_hex_double)
}

fn y_overlap(left: Bounds, right: Bounds) -> i32 {
    let top = left.y.max(right.y);
    let bottom = (left.y + left.height).min(right.y + right.height);
    i32::try_from(bottom).unwrap() - i32::try_from(top).unwrap()
}

fn line_bounds(line: NativeStemLine) -> JavaRectangle {
    let min_x = line.start.x.min(line.stop.x).floor() as i32;
    let min_y = line.start.y.min(line.stop.y).floor() as i32;
    let max_x = line.start.x.max(line.stop.x).ceil() as i32;
    let max_y = line.start.y.max(line.stop.y).ceil() as i32;
    JavaRectangle::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

fn usize_list(values: &[usize]) -> String {
    string_list(&values.iter().map(usize::to_string).collect::<Vec<_>>())
}

fn string_list(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_owned()
    } else {
        values.join(",")
    }
}

fn java_rint(value: f64) -> i32 {
    value.round_ties_even() as i32
}

fn java_double_compare(left: f64, right: f64) -> i32 {
    if left < right {
        -1
    } else if left > right {
        1
    } else if left.to_bits() == right.to_bits() {
        0
    } else if left.is_nan() {
        if right.is_nan() { 0 } else { 1 }
    } else if right.is_nan() || left.is_sign_negative() {
        -1
    } else {
        1
    }
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

fn oracle_page_rows(oracle: &str, page: &str) -> Vec<String> {
    oracle
        .lines()
        .filter(|line| line.split_whitespace().nth(1) == Some(page))
        .filter(|line| {
            line.split_whitespace()
                .next()
                .is_some_and(|prefix| prefix.starts_with("stemsbeambuilder"))
        })
        .map(str::to_owned)
        .collect()
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

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}
