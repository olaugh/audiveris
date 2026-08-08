// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production-shaped composition of Java HEADS staff postprocessing.
//!
//! Seed heads are created first for a staff, followed by range heads. Java
//! then sorts that combined list by full abscissa, removes exact duplicates,
//! records overlap exclusions without removing either endpoint, purges tally
//! entries belonging to duplicate removals, and attaches the survivors to the
//! staff in the sorted order. This module preserves both creation and sorted
//! identities so downstream tally and small-beam composition does not have to
//! reconstruct Java object provenance from geometry.

use std::{error::Error, fmt};

use crate::{
    head_glyph_retrieval::RetrievedHeadGlyph,
    head_pair_predicates::{
        HeadGlyphIdentity, HeadOverlapInput, HeadSameAsInput, head_inter_overlaps, inter_is_same_as,
    },
    head_purge::{
        HeadPurgeInput, HeadPurgeOrigin, HeadPurgeResult, HeadSeedTally, purge_staff_heads,
    },
    head_scanner_slices::JavaRectangle,
    head_template::{HeadTemplateBounds, HeadTemplateShape},
    native_heads_range_glyphs::{
        NativeHeadRangeGlyph, NativeHeadsRangeGlyphRecognition, NativeHeadsRangeGlyphStaff,
    },
    native_heads_seed_glyphs::{
        NativeHeadSeedGlyph, NativeHeadsSeedGlyphRecognition, NativeHeadsSeedGlyphStaff,
    },
};

/// Immutable inputs at the Java `buildHeads` post-range boundary.
pub struct NativeHeadsStaffEpilogInput<'a> {
    pub seed_glyphs: &'a NativeHeadsSeedGlyphRecognition,
    pub range_glyphs: &'a NativeHeadsRangeGlyphRecognition,
}

/// All staff-purge results and system-level traversal orders.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsStaffEpilogRecognition {
    pub systems: Vec<NativeHeadsStaffEpilogSystem>,
    pub input_head_count: usize,
    pub duplicate_removal_count: usize,
    pub overlap_exclusion_count: usize,
    pub retained_head_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsStaffEpilogSystem {
    pub system_id: usize,
    pub staffs: Vec<NativeHeadsStaffEpilogStaff>,
    /// All successfully registered heads in Java creation order.
    pub creation_order: Vec<NativeHeadStaffEpilogRef>,
    /// Creation order after true duplicate removals.
    pub retained_creation_order: Vec<NativeHeadStaffEpilogRef>,
    /// Java `staff.addNote` traversal: staff order, then full-abscissa order.
    pub attachment_order: Vec<NativeHeadStaffEpilogRef>,
    pub input_head_count: usize,
    pub duplicate_removal_count: usize,
    pub overlap_exclusion_count: usize,
    pub retained_head_count: usize,
}

/// Stable reference into one system's staff-local creation-order head vectors.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativeHeadStaffEpilogRef {
    pub staff_index: usize,
    pub head_index: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsStaffEpilogStaff {
    pub staff_id: usize,
    /// Dense token used to model Java staff object reference identity.
    pub staff_identity: usize,
    /// Heads in Java creation order: successful seeds, then successful ranges.
    pub heads: Vec<NativeHeadStaffEpilogHead>,
    /// Creation-order indices in Java `Inters.byFullAbscissa` input order.
    pub ordered_input_indices: Vec<usize>,
    /// Creation-order indices traversed by Java `staff.addNote`.
    pub retained_attachment_indices: Vec<usize>,
    /// Full check/decision/tally trace from the duplicate and overlap passes.
    pub purge: HeadPurgeResult,
}

/// Compact origin printed by the frozen Java post-range probe.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NativeHeadStaffEpilogOrigin {
    Seed(usize),
    Range(usize),
}

/// Full upstream provenance retained beyond the compact origin label.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NativeHeadStaffEpilogProvenance {
    Seed {
        ordinal: usize,
        candidate_ordinal: usize,
        search_ordinal: usize,
        geometry_ordinal: usize,
        seed_pool_index: usize,
        seed_source_ordinal: usize,
    },
    Range {
        ordinal: usize,
        candidate_ordinal: usize,
        raw_ordinal: usize,
        attempt_ordinal: usize,
        geometry_ordinal: usize,
    },
}

/// One head keyed by its creation-order index in its containing staff.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadStaffEpilogHead {
    pub origin: NativeHeadStaffEpilogOrigin,
    pub provenance: NativeHeadStaffEpilogProvenance,
    /// Monotonic order among heads successfully added in this system. Actual
    /// Java IDs can contain gaps from glyph/other-entity registration, but
    /// this has the exact relative order used by `byFullAbscissa` ties.
    pub relative_java_id: i32,
    pub system_creation_ordinal: usize,
    /// Position after Java's full-abscissa sort and before duplicate removal.
    pub input_ordinal: usize,
    pub original_shape: HeadTemplateShape,
    pub shape: HeadTemplateShape,
    pub pitch_bits: u64,
    pub bounds: JavaRectangle,
    pub raw_distance_impact_bits: u64,
    pub distance_impact_bits: u64,
    pub grade_bits: u64,
    pub minimum_grade_bits: u64,
    pub good: bool,
    pub glyph_bounds: JavaRectangle,
    pub glyph_weight: usize,
    pub glyph_run_digest: u64,
    /// Seed tally at creation time (always empty for range heads).
    pub tally_before: HeadSeedTally,
    /// Tally after equal-grade replication in both purge passes.
    pub tally_after_replication: HeadSeedTally,
    /// Java `HeadSeedTally.purgeRemovedHeads` retains this head's tally key.
    pub tally_retained_after_cleanup: bool,
    pub duplicate_removed: bool,
    /// Ordinal in `retained_attachment_indices`, or `None` when removed.
    pub attachment_ordinal: Option<usize>,
}

impl NativeHeadStaffEpilogHead {
    #[must_use]
    pub fn pitch(&self) -> f64 {
        f64::from_bits(self.pitch_bits)
    }

    #[must_use]
    pub fn grade(&self) -> f64 {
        f64::from_bits(self.grade_bits)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeHeadsStaffEpilogError {
    SystemOrder {
        seed: Vec<usize>,
        range: Vec<usize>,
    },
    StaffOrder {
        system_id: usize,
        seed: Vec<usize>,
        range: Vec<usize>,
    },
    HeadIdOverflow {
        system_id: usize,
        creation_ordinal: usize,
    },
    SourceOrdinal {
        system_id: usize,
        staff_id: usize,
        origin: &'static str,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for NativeHeadsStaffEpilogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemOrder { seed, range } => write!(
                formatter,
                "HEADS staff epilog system order differs: seed {seed:?}, range {range:?}"
            ),
            Self::StaffOrder {
                system_id,
                seed,
                range,
            } => write!(
                formatter,
                "HEADS staff epilog system {system_id} staff order differs: seed {seed:?}, range \
                 {range:?}"
            ),
            Self::HeadIdOverflow {
                system_id,
                creation_ordinal,
            } => write!(
                formatter,
                "HEADS staff epilog system {system_id} creation ordinal {creation_ordinal} exceeds \
                 Java int relative IDs"
            ),
            Self::SourceOrdinal {
                system_id,
                staff_id,
                origin,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS staff epilog system {system_id} staff {staff_id} {origin} ordinal {actual}, \
                 expected {expected}"
            ),
        }
    }
}

impl Error for NativeHeadsStaffEpilogError {}

/// Compose Java's staff-local HEADS epilog through note-attachment order.
pub fn compose_native_heads_staff_epilog(
    input: NativeHeadsStaffEpilogInput<'_>,
) -> Result<NativeHeadsStaffEpilogRecognition, NativeHeadsStaffEpilogError> {
    let seed_system_ids = input
        .seed_glyphs
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    let range_system_ids = input
        .range_glyphs
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    if seed_system_ids != range_system_ids {
        return Err(NativeHeadsStaffEpilogError::SystemOrder {
            seed: seed_system_ids,
            range: range_system_ids,
        });
    }

    let mut systems = Vec::with_capacity(input.seed_glyphs.systems.len());
    let mut next_staff_identity = 0_usize;
    let mut total_inputs = 0_usize;
    let mut total_duplicates = 0_usize;
    let mut total_overlaps = 0_usize;
    let mut total_retained = 0_usize;
    for (seed_system, range_system) in input
        .seed_glyphs
        .systems
        .iter()
        .zip(&input.range_glyphs.systems)
    {
        let seed_staff_ids = seed_system
            .staffs
            .iter()
            .map(|staff| staff.staff_id)
            .collect::<Vec<_>>();
        let range_staff_ids = range_system
            .staffs
            .iter()
            .map(|staff| staff.staff_id)
            .collect::<Vec<_>>();
        if seed_staff_ids != range_staff_ids {
            return Err(NativeHeadsStaffEpilogError::StaffOrder {
                system_id: seed_system.system_id,
                seed: seed_staff_ids,
                range: range_staff_ids,
            });
        }

        let mut staffs = Vec::with_capacity(seed_system.staffs.len());
        let mut creation_order = Vec::new();
        let mut relative_java_id = 0_i32;
        for (staff_index, (seed_staff, range_staff)) in seed_system
            .staffs
            .iter()
            .zip(&range_system.staffs)
            .enumerate()
        {
            let staff_identity = next_staff_identity;
            next_staff_identity = next_staff_identity.wrapping_add(1);
            let staff = compose_staff(
                seed_system.system_id,
                seed_staff,
                range_staff,
                staff_identity,
                &mut relative_java_id,
                creation_order.len(),
            )?;
            creation_order.extend((0..staff.heads.len()).map(|head_index| {
                NativeHeadStaffEpilogRef {
                    staff_index,
                    head_index,
                }
            }));
            staffs.push(staff);
        }

        let mut retained_creation_order = Vec::new();
        let mut attachment_order = Vec::new();
        let mut system_duplicates = 0_usize;
        let mut system_overlaps = 0_usize;
        for (staff_index, staff) in staffs.iter().enumerate() {
            system_duplicates += staff.purge.duplicate.decisions.len();
            system_overlaps += staff.purge.overlap.decisions.len();
            for (head_index, head) in staff.heads.iter().enumerate() {
                if !head.duplicate_removed {
                    retained_creation_order.push(NativeHeadStaffEpilogRef {
                        staff_index,
                        head_index,
                    });
                }
            }
            attachment_order.extend(staff.retained_attachment_indices.iter().map(|&head_index| {
                NativeHeadStaffEpilogRef {
                    staff_index,
                    head_index,
                }
            }));
        }
        let input_head_count = creation_order.len();
        let retained_head_count = retained_creation_order.len();
        total_inputs += input_head_count;
        total_duplicates += system_duplicates;
        total_overlaps += system_overlaps;
        total_retained += retained_head_count;
        systems.push(NativeHeadsStaffEpilogSystem {
            system_id: seed_system.system_id,
            staffs,
            creation_order,
            retained_creation_order,
            attachment_order,
            input_head_count,
            duplicate_removal_count: system_duplicates,
            overlap_exclusion_count: system_overlaps,
            retained_head_count,
        });
    }

    Ok(NativeHeadsStaffEpilogRecognition {
        systems,
        input_head_count: total_inputs,
        duplicate_removal_count: total_duplicates,
        overlap_exclusion_count: total_overlaps,
        retained_head_count: total_retained,
    })
}

#[allow(clippy::too_many_arguments)]
fn compose_staff(
    system_id: usize,
    seed_staff: &NativeHeadsSeedGlyphStaff,
    range_staff: &NativeHeadsRangeGlyphStaff,
    staff_identity: usize,
    relative_java_id: &mut i32,
    first_system_creation_ordinal: usize,
) -> Result<NativeHeadsStaffEpilogStaff, NativeHeadsStaffEpilogError> {
    let capacity = seed_staff.heads.len() + range_staff.heads.len();
    let mut heads = Vec::with_capacity(capacity);
    let mut glyphs = Vec::with_capacity(capacity);
    let mut purge_inputs = Vec::with_capacity(capacity);

    for (expected, head) in seed_staff.heads.iter().enumerate() {
        if head.ordinal != expected {
            return Err(NativeHeadsStaffEpilogError::SourceOrdinal {
                system_id,
                staff_id: seed_staff.staff_id,
                origin: "seed",
                expected,
                actual: head.ordinal,
            });
        }
        *relative_java_id =
            relative_java_id
                .checked_add(1)
                .ok_or(NativeHeadsStaffEpilogError::HeadIdOverflow {
                    system_id,
                    creation_ordinal: first_system_creation_ordinal + heads.len(),
                })?;
        push_seed_head(
            head,
            *relative_java_id,
            first_system_creation_ordinal + heads.len(),
            &mut heads,
            &mut glyphs,
            &mut purge_inputs,
        );
    }
    for (expected, head) in range_staff.heads.iter().enumerate() {
        if head.ordinal != expected {
            return Err(NativeHeadsStaffEpilogError::SourceOrdinal {
                system_id,
                staff_id: range_staff.staff_id,
                origin: "range",
                expected,
                actual: head.ordinal,
            });
        }
        *relative_java_id =
            relative_java_id
                .checked_add(1)
                .ok_or(NativeHeadsStaffEpilogError::HeadIdOverflow {
                    system_id,
                    creation_ordinal: first_system_creation_ordinal + heads.len(),
                })?;
        push_range_head(
            head,
            *relative_java_id,
            first_system_creation_ordinal + heads.len(),
            &mut heads,
            &mut glyphs,
            &mut purge_inputs,
        );
    }

    let purge = purge_staff_heads(
        &purge_inputs,
        |left, right| {
            inter_is_same_as(
                &same_as_input(&heads[left], glyphs[left]),
                &same_as_input(&heads[right], glyphs[right]),
            )
        },
        |left, right| {
            head_inter_overlaps(
                overlap_input(&heads[left], staff_identity),
                overlap_input(&heads[right], staff_identity),
            )
        },
    );

    let mut input_ordinals = vec![0_usize; heads.len()];
    for (input_ordinal, &creation_index) in purge.ordered_indices.iter().enumerate() {
        input_ordinals[creation_index] = input_ordinal;
    }
    let mut attachment_ordinals = vec![None; heads.len()];
    for (attachment_ordinal, &creation_index) in purge.retained_indices.iter().enumerate() {
        attachment_ordinals[creation_index] = Some(attachment_ordinal);
    }
    let mut duplicate_removed = vec![false; heads.len()];
    for &creation_index in &purge.duplicate_removed_indices {
        duplicate_removed[creation_index] = true;
    }
    for (creation_index, head) in heads.iter_mut().enumerate() {
        head.input_ordinal = input_ordinals[creation_index];
        head.tally_after_replication = purge.tallies[creation_index];
        head.tally_retained_after_cleanup =
            !duplicate_removed[creation_index] && !purge.tallies[creation_index].is_empty();
        head.duplicate_removed = duplicate_removed[creation_index];
        head.attachment_ordinal = attachment_ordinals[creation_index];
    }

    Ok(NativeHeadsStaffEpilogStaff {
        staff_id: seed_staff.staff_id,
        staff_identity,
        heads,
        ordered_input_indices: purge.ordered_indices.clone(),
        retained_attachment_indices: purge.retained_indices.clone(),
        purge,
    })
}

fn push_seed_head<'a>(
    source: &'a NativeHeadSeedGlyph,
    relative_java_id: i32,
    system_creation_ordinal: usize,
    heads: &mut Vec<NativeHeadStaffEpilogHead>,
    glyphs: &mut Vec<&'a RetrievedHeadGlyph>,
    purge_inputs: &mut Vec<HeadPurgeInput<HeadTemplateShape>>,
) {
    let tally = HeadSeedTally {
        left_dx: source.tally_left(),
        right_dx: source.tally_right(),
    };
    push_head(
        NativeHeadStaffEpilogOrigin::Seed(source.ordinal),
        NativeHeadStaffEpilogProvenance::Seed {
            ordinal: source.ordinal,
            candidate_ordinal: source.candidate_ordinal,
            search_ordinal: source.search_ordinal,
            geometry_ordinal: source.geometry_ordinal,
            seed_pool_index: source.seed_pool_index,
            seed_source_ordinal: source.seed_source_ordinal,
        },
        relative_java_id,
        system_creation_ordinal,
        source.original_shape,
        source.shape,
        source.pitch_bits,
        source.bounds,
        source.raw_distance_impact_bits,
        source.distance_impact_bits,
        source.grade_bits,
        source.minimum_grade_bits,
        source.good,
        &source.glyph,
        tally,
        HeadPurgeOrigin::Seed,
        heads,
        glyphs,
        purge_inputs,
    );
}

fn push_range_head<'a>(
    source: &'a NativeHeadRangeGlyph,
    relative_java_id: i32,
    system_creation_ordinal: usize,
    heads: &mut Vec<NativeHeadStaffEpilogHead>,
    glyphs: &mut Vec<&'a RetrievedHeadGlyph>,
    purge_inputs: &mut Vec<HeadPurgeInput<HeadTemplateShape>>,
) {
    push_head(
        NativeHeadStaffEpilogOrigin::Range(source.ordinal),
        NativeHeadStaffEpilogProvenance::Range {
            ordinal: source.ordinal,
            candidate_ordinal: source.candidate_ordinal,
            raw_ordinal: source.raw_ordinal,
            attempt_ordinal: source.attempt_ordinal,
            geometry_ordinal: source.geometry_ordinal,
        },
        relative_java_id,
        system_creation_ordinal,
        source.original_shape,
        source.shape,
        source.pitch_bits,
        source.bounds,
        source.raw_distance_impact_bits,
        source.distance_impact_bits,
        source.grade_bits,
        source.minimum_grade_bits,
        source.good,
        &source.glyph,
        HeadSeedTally::default(),
        HeadPurgeOrigin::Range,
        heads,
        glyphs,
        purge_inputs,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_head<'a>(
    origin: NativeHeadStaffEpilogOrigin,
    provenance: NativeHeadStaffEpilogProvenance,
    relative_java_id: i32,
    system_creation_ordinal: usize,
    original_shape: HeadTemplateShape,
    shape: HeadTemplateShape,
    pitch_bits: u64,
    bounds: HeadTemplateBounds,
    raw_distance_impact_bits: u64,
    distance_impact_bits: u64,
    grade_bits: u64,
    minimum_grade_bits: u64,
    good: bool,
    glyph: &'a RetrievedHeadGlyph,
    tally: HeadSeedTally,
    purge_origin: HeadPurgeOrigin,
    heads: &mut Vec<NativeHeadStaffEpilogHead>,
    glyphs: &mut Vec<&'a RetrievedHeadGlyph>,
    purge_inputs: &mut Vec<HeadPurgeInput<HeadTemplateShape>>,
) {
    let bounds = rectangle(bounds);
    let glyph_bounds = rectangle(glyph.glyph_bounds);
    heads.push(NativeHeadStaffEpilogHead {
        origin,
        provenance,
        relative_java_id,
        system_creation_ordinal,
        input_ordinal: usize::MAX,
        original_shape,
        shape,
        pitch_bits,
        bounds,
        raw_distance_impact_bits,
        distance_impact_bits,
        grade_bits,
        minimum_grade_bits,
        good,
        glyph_bounds,
        glyph_weight: glyph.weight,
        glyph_run_digest: glyph.run_digest,
        tally_before: tally,
        tally_after_replication: tally,
        tally_retained_after_cleanup: false,
        duplicate_removed: false,
        attachment_ordinal: None,
    });
    glyphs.push(glyph);
    purge_inputs.push(HeadPurgeInput {
        java_id: relative_java_id,
        origin: purge_origin,
        bounds,
        grade: f64::from_bits(grade_bits),
        shape,
        good,
        tally,
        removed: false,
    });
}

fn same_as_input<'a>(
    head: &NativeHeadStaffEpilogHead,
    glyph: &'a RetrievedHeadGlyph,
) -> HeadSameAsInput<'a, HeadTemplateShape> {
    HeadSameAsInput {
        shape: head.shape,
        bounds: Some(head.bounds),
        glyph: Some(HeadGlyphIdentity::from(glyph)),
    }
}

fn overlap_input(head: &NativeHeadStaffEpilogHead, staff_identity: usize) -> HeadOverlapInput {
    HeadOverlapInput {
        staff_identity: Some(staff_identity),
        pitch: head.pitch(),
        bounds: head.bounds,
    }
}

const fn rectangle(bounds: HeadTemplateBounds) -> JavaRectangle {
    JavaRectangle::new(bounds.x, bounds.y, bounds.width, bounds.height)
}

#[cfg(test)]
mod tests {
    use audiveris_image::run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable};

    use crate::{
        head_template::HeadTemplateAnchor,
        native_heads_range_glyphs::{
            NativeHeadsRangeGlyphRecognition, NativeHeadsRangeGlyphStaff,
            NativeHeadsRangeGlyphSystem,
        },
        native_heads_seed_glyphs::{
            NativeHeadsSeedGlyphRecognition, NativeHeadsSeedGlyphStaff, NativeHeadsSeedGlyphSystem,
        },
        native_heads_seed_lookup::NativeHeadSeedSide,
    };

    use super::*;

    fn glyph(bounds: HeadTemplateBounds, pixels: &[u8]) -> RetrievedHeadGlyph {
        let run_table =
            RunTable::from_pixels(Orientation::Vertical, 2, 2, pixels).expect("valid test glyph");
        RetrievedHeadGlyph {
            template_bounds: bounds,
            foreground_bounds: HeadTemplateBounds {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
            },
            glyph_bounds: bounds,
            weight: run_table.weight(),
            run_digest: 7,
            run_table,
            new_inter_bounds: bounds,
        }
    }

    fn seed(
        ordinal: usize,
        x: i32,
        shape: HeadTemplateShape,
        grade: f64,
        tally: HeadSeedTally,
    ) -> NativeHeadSeedGlyph {
        let bounds = HeadTemplateBounds {
            x,
            y: 10,
            width: 10,
            height: 10,
        };
        NativeHeadSeedGlyph {
            ordinal,
            candidate_ordinal: 100 + ordinal,
            search_ordinal: 200 + ordinal,
            geometry_ordinal: 300 + ordinal,
            seed_pool_index: 400 + ordinal,
            seed_source_ordinal: 500 + ordinal,
            side: NativeHeadSeedSide::Left,
            anchor: HeadTemplateAnchor::LeftStem,
            original_shape: shape,
            shape,
            x,
            y: 10,
            distance_bits: 0.1_f64.to_bits(),
            pitch_bits: 0.0_f64.to_bits(),
            provisional_bounds: bounds,
            bounds,
            raw_distance_impact_bits: 0.9_f64.to_bits(),
            distance_impact_bits: 0.9_f64.to_bits(),
            grade_bits: grade.to_bits(),
            minimum_grade_bits: 0.1_f64.to_bits(),
            good: true,
            glyph: glyph(bounds, &[FOREGROUND; 4]),
            tally_left_bits: tally.left_dx.map(f64::to_bits),
            tally_right_bits: tally.right_dx.map(f64::to_bits),
        }
    }

    fn range(ordinal: usize, x: i32, shape: HeadTemplateShape, grade: f64) -> NativeHeadRangeGlyph {
        let bounds = HeadTemplateBounds {
            x,
            y: 10,
            width: 10,
            height: 10,
        };
        NativeHeadRangeGlyph {
            ordinal,
            candidate_ordinal: 600 + ordinal,
            raw_ordinal: 700 + ordinal,
            attempt_ordinal: 800 + ordinal,
            geometry_ordinal: 900 + ordinal,
            original_shape: shape,
            shape,
            pitch_bits: 0.0_f64.to_bits(),
            provisional_bounds: bounds,
            bounds,
            raw_distance_impact_bits: 0.8_f64.to_bits(),
            distance_impact_bits: 0.8_f64.to_bits(),
            grade_bits: grade.to_bits(),
            minimum_grade_bits: 0.1_f64.to_bits(),
            good: true,
            glyph: glyph(bounds, &[FOREGROUND; 4]),
        }
    }

    fn inputs(
        seed_heads: Vec<NativeHeadSeedGlyph>,
        range_heads: Vec<NativeHeadRangeGlyph>,
    ) -> (
        NativeHeadsSeedGlyphRecognition,
        NativeHeadsRangeGlyphRecognition,
    ) {
        let seed_count = seed_heads.len();
        let range_count = range_heads.len();
        (
            NativeHeadsSeedGlyphRecognition {
                systems: vec![NativeHeadsSeedGlyphSystem {
                    system_id: 3,
                    staffs: vec![NativeHeadsSeedGlyphStaff {
                        staff_id: 7,
                        candidate_count: seed_count,
                        dropped_count: 0,
                        heads: seed_heads,
                    }],
                    candidate_count: seed_count,
                    dropped_count: 0,
                    head_count: seed_count,
                }],
                candidate_count: seed_count,
                dropped_count: 0,
                head_count: seed_count,
            },
            NativeHeadsRangeGlyphRecognition {
                systems: vec![NativeHeadsRangeGlyphSystem {
                    system_id: 3,
                    staffs: vec![NativeHeadsRangeGlyphStaff {
                        staff_id: 7,
                        scans: Vec::new(),
                        candidates: Vec::new(),
                        heads: range_heads,
                        seed_head_count: seed_count,
                        candidate_count: range_count,
                        seed_conflict_count: 0,
                        glyph_empty_count: 0,
                    }],
                    candidate_count: range_count,
                    seed_conflict_count: 0,
                    glyph_empty_count: 0,
                    head_count: range_count,
                }],
                candidate_count: range_count,
                seed_conflict_count: 0,
                glyph_empty_count: 0,
                head_count: range_count,
            },
        )
    }

    #[test]
    fn composes_duplicates_overlaps_tallies_and_all_orders() {
        let (seed_glyphs, range_glyphs) = inputs(
            vec![
                seed(
                    0,
                    10,
                    HeadTemplateShape::NoteheadBlack,
                    0.9,
                    HeadSeedTally {
                        left_dx: Some(1.25),
                        right_dx: None,
                    },
                ),
                seed(
                    1,
                    10,
                    HeadTemplateShape::NoteheadBlack,
                    0.9,
                    HeadSeedTally {
                        left_dx: None,
                        right_dx: Some(2.5),
                    },
                ),
            ],
            vec![
                range(0, 11, HeadTemplateShape::NoteheadVoid, 0.8),
                range(1, 40, HeadTemplateShape::NoteheadBlack, 0.7),
            ],
        );
        let result = compose_native_heads_staff_epilog(NativeHeadsStaffEpilogInput {
            seed_glyphs: &seed_glyphs,
            range_glyphs: &range_glyphs,
        })
        .expect("valid epilog");

        assert_eq!(result.input_head_count, 4);
        assert_eq!(result.duplicate_removal_count, 1);
        assert_eq!(result.overlap_exclusion_count, 1);
        assert_eq!(result.retained_head_count, 3);
        let system = &result.systems[0];
        let staff = &system.staffs[0];
        assert_eq!(staff.ordered_input_indices, vec![0, 1, 2, 3]);
        assert_eq!(staff.retained_attachment_indices, vec![0, 2, 3]);
        assert_eq!(staff.heads[0].origin, NativeHeadStaffEpilogOrigin::Seed(0));
        assert_eq!(staff.heads[0].grade(), 0.9);
        assert_eq!(staff.heads[2].origin, NativeHeadStaffEpilogOrigin::Range(0));
        assert!(staff.heads[1].duplicate_removed);
        assert_eq!(staff.heads[1].attachment_ordinal, None);
        assert_eq!(
            staff.heads[0].tally_after_replication,
            HeadSeedTally {
                left_dx: Some(1.25),
                right_dx: Some(2.5),
            }
        );
        assert!(staff.heads[0].tally_retained_after_cleanup);
        assert!(!staff.heads[1].tally_retained_after_cleanup);
        assert_eq!(
            system.retained_creation_order,
            vec![
                NativeHeadStaffEpilogRef {
                    staff_index: 0,
                    head_index: 0,
                },
                NativeHeadStaffEpilogRef {
                    staff_index: 0,
                    head_index: 2,
                },
                NativeHeadStaffEpilogRef {
                    staff_index: 0,
                    head_index: 3,
                },
            ]
        );
        assert_eq!(system.attachment_order, system.retained_creation_order);
    }

    #[test]
    fn preserves_creation_ids_while_exposing_sorted_input_and_attachment_ordinals() {
        let (seed_glyphs, range_glyphs) = inputs(
            vec![seed(
                0,
                20,
                HeadTemplateShape::NoteheadBlack,
                0.9,
                HeadSeedTally::default(),
            )],
            vec![range(0, 0, HeadTemplateShape::NoteheadBlack, 0.8)],
        );
        let result = compose_native_heads_staff_epilog(NativeHeadsStaffEpilogInput {
            seed_glyphs: &seed_glyphs,
            range_glyphs: &range_glyphs,
        })
        .expect("valid epilog");
        let staff = &result.systems[0].staffs[0];

        assert_eq!(staff.heads[0].relative_java_id, 1);
        assert_eq!(staff.heads[1].relative_java_id, 2);
        assert_eq!(staff.ordered_input_indices, vec![1, 0]);
        assert_eq!(staff.heads[1].input_ordinal, 0);
        assert_eq!(staff.heads[0].input_ordinal, 1);
        assert_eq!(staff.retained_attachment_indices, vec![1, 0]);
    }

    #[test]
    fn rejects_mismatched_system_and_source_orders() {
        let (seed_glyphs, mut range_glyphs) = inputs(Vec::new(), Vec::new());
        range_glyphs.systems[0].system_id = 4;
        assert!(matches!(
            compose_native_heads_staff_epilog(NativeHeadsStaffEpilogInput {
                seed_glyphs: &seed_glyphs,
                range_glyphs: &range_glyphs,
            }),
            Err(NativeHeadsStaffEpilogError::SystemOrder { .. })
        ));

        let (mut seed_glyphs, range_glyphs) = inputs(
            vec![seed(
                0,
                10,
                HeadTemplateShape::NoteheadBlack,
                0.9,
                HeadSeedTally::default(),
            )],
            Vec::new(),
        );
        seed_glyphs.systems[0].staffs[0].heads[0].ordinal = 9;
        assert!(matches!(
            compose_native_heads_staff_epilog(NativeHeadsStaffEpilogInput {
                seed_glyphs: &seed_glyphs,
                range_glyphs: &range_glyphs,
            }),
            Err(NativeHeadsStaffEpilogError::SourceOrdinal {
                origin: "seed",
                expected: 0,
                actual: 9,
                ..
            })
        ));
    }

    #[test]
    fn duplicate_predicate_uses_exact_glyph_run_table_not_only_geometry() {
        let (seed_glyphs, mut range_glyphs) = inputs(
            vec![seed(
                0,
                10,
                HeadTemplateShape::NoteheadBlack,
                0.9,
                HeadSeedTally::default(),
            )],
            vec![range(0, 10, HeadTemplateShape::NoteheadBlack, 0.8)],
        );
        range_glyphs.systems[0].staffs[0].heads[0].glyph.run_table = RunTable::from_pixels(
            Orientation::Vertical,
            2,
            2,
            &[FOREGROUND, BACKGROUND, FOREGROUND, FOREGROUND],
        )
        .expect("different glyph");
        range_glyphs.systems[0].staffs[0].heads[0].glyph.weight = 3;
        let result = compose_native_heads_staff_epilog(NativeHeadsStaffEpilogInput {
            seed_glyphs: &seed_glyphs,
            range_glyphs: &range_glyphs,
        })
        .expect("valid epilog");

        assert_eq!(result.duplicate_removal_count, 0);
        assert_eq!(result.overlap_exclusion_count, 1);
        assert_eq!(result.retained_head_count, 2);
    }
}
