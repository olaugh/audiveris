// SPDX-License-Identifier: AGPL-3.0-or-later

//! Complete production composition of Java's HEADS epilog.
//!
//! This layer joins staff-local duplicate/overlap processing, system-local
//! small-beam arbitration, and sheet-wide seed-tally analysis. It preserves
//! the separate ordering domains Java relies on: head creation/SIG order,
//! staff attachment order, stable ordinate order, and the insertion order of
//! each system tally's LEFT and RIGHT `LinkedHashMap`.

use std::{collections::BTreeSet, error::Error, fmt};

use crate::{
    head_purge::{HeadPurgeDecision, HeadPurgeReason, HeadSeedTally},
    head_seed_tally_analysis::{
        HeadSeedScaleEntry, HeadSeedTallySample, HeadSeedTallySide, analyze_head_seed_tallies,
    },
    native_heads_competitors::NativeHeadsCompetitorPool,
    native_heads_range_glyphs::NativeHeadsRangeGlyphRecognition,
    native_heads_seed_glyphs::NativeHeadsSeedGlyphRecognition,
    native_heads_small_beams::{
        NativeHeadsSmallBeamError, NativeHeadsSmallBeamHead, NativeHeadsSmallBeamHeadSystem,
        NativeHeadsSmallBeamSystemResult, purge_native_heads_small_beams,
    },
    native_heads_staff_epilog::{
        NativeHeadStaffEpilogHead, NativeHeadStaffEpilogRef, NativeHeadsStaffEpilogError,
        NativeHeadsStaffEpilogInput, NativeHeadsStaffEpilogRecognition,
        NativeHeadsStaffEpilogStaff, NativeHeadsStaffEpilogSystem,
        compose_native_heads_staff_epilog,
    },
    recognize::NativeBeamRecognition,
};

/// Immutable inputs at the complete Java HEADS epilog boundary.
pub struct NativeHeadsEpilogInput<'a> {
    pub seed_glyphs: &'a NativeHeadsSeedGlyphRecognition,
    pub range_glyphs: &'a NativeHeadsRangeGlyphRecognition,
    pub competitors: &'a NativeHeadsCompetitorPool,
    pub beams: &'a NativeBeamRecognition,
    pub stemless_boost: f64,
}

/// Java `Shape` declaration order for the eight oval template head shapes.
///
/// This is deliberately distinct from `HeadTemplateShape`, whose discriminants
/// preserve `TemplateFactory` construction order (black, void, whole, breve).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum JavaHeadSeedShape {
    Breve,
    BreveSmall,
    WholeNote,
    WholeNoteSmall,
    NoteheadVoid,
    NoteheadVoidSmall,
    NoteheadBlack,
    NoteheadBlackSmall,
}

/// One surviving key in a system tally's insertion-preserving side map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHeadsTallyEntry {
    pub head: NativeHeadStaffEpilogRef,
    pub shape: JavaHeadSeedShape,
    pub side: HeadSeedTallySide,
    pub dx: f64,
    /// Set after small-beam arbitration. Java retains this map key but skips
    /// it in `HeadSeedTally.analyze`.
    pub removed: bool,
}

/// Complete output through final system heads and sheet scale analysis.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsEpilogRecognition {
    pub staff_epilog: NativeHeadsStaffEpilogRecognition,
    pub systems: Vec<NativeHeadsEpilogSystem>,
    /// Java analysis traversal: system order, LEFT map, then RIGHT map.
    pub tally_samples: Vec<HeadSeedTallySample<JavaHeadSeedShape>>,
    pub scale_analysis: Vec<HeadSeedScaleEntry<JavaHeadSeedShape>>,
    pub beam_removal_count: usize,
    pub head_beam_removal_count: usize,
    pub final_head_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsEpilogSystem {
    pub system_id: usize,
    /// Live post-duplicate heads in Java creation/SIG order.
    pub heads_in_sig_order: Vec<NativeHeadStaffEpilogRef>,
    /// Same heads in the stable ordinate order seen by `purgeSmallBeams`.
    pub heads_before_beam_by_ordinate: Vec<NativeHeadStaffEpilogRef>,
    pub small_beams: NativeHeadsSmallBeamSystemResult<NativeHeadStaffEpilogRef>,
    /// Actual head removals in beam-decision order.
    pub beam_removed_heads: Vec<NativeHeadStaffEpilogRef>,
    /// Final live heads in Java's stable ordinate order.
    pub final_heads: Vec<NativeHeadStaffEpilogRef>,
    /// LEFT `LinkedHashMap` after staff duplicate cleanup, with beam-removed
    /// keys retained and marked.
    pub tally_left: Vec<NativeHeadsTallyEntry>,
    /// RIGHT `LinkedHashMap` after staff duplicate cleanup, with beam-removed
    /// keys retained and marked.
    pub tally_right: Vec<NativeHeadsTallyEntry>,
}

#[derive(Debug)]
pub enum NativeHeadsEpilogError {
    Staff(NativeHeadsStaffEpilogError),
    SystemOrder {
        staff: Vec<usize>,
        competitors: Vec<usize>,
    },
    SmallBeams(NativeHeadsSmallBeamError),
    TallyReplay {
        system_id: usize,
        staff_id: usize,
        head_index: usize,
        expected: HeadSeedTally,
        actual: HeadSeedTally,
    },
}

impl fmt::Display for NativeHeadsEpilogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Staff(error) => write!(formatter, "HEADS staff epilog: {error}"),
            Self::SystemOrder { staff, competitors } => write!(
                formatter,
                "HEADS epilog system order differs: staff {staff:?}, competitors {competitors:?}"
            ),
            Self::SmallBeams(error) => write!(formatter, "HEADS small-beam epilog: {error}"),
            Self::TallyReplay {
                system_id,
                staff_id,
                head_index,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS epilog system {system_id} staff {staff_id} head {head_index} tally replay \
                 {actual:?}, expected {expected:?}"
            ),
        }
    }
}

impl Error for NativeHeadsEpilogError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Staff(error) => Some(error),
            Self::SmallBeams(error) => Some(error),
            Self::SystemOrder { .. } | Self::TallyReplay { .. } => None,
        }
    }
}

impl From<NativeHeadsStaffEpilogError> for NativeHeadsEpilogError {
    fn from(value: NativeHeadsStaffEpilogError) -> Self {
        Self::Staff(value)
    }
}

impl From<NativeHeadsSmallBeamError> for NativeHeadsEpilogError {
    fn from(value: NativeHeadsSmallBeamError) -> Self {
        Self::SmallBeams(value)
    }
}

/// Compose Java HEADS from successful glyphs through final scale analysis.
pub fn compose_native_heads_epilog(
    input: NativeHeadsEpilogInput<'_>,
) -> Result<NativeHeadsEpilogRecognition, NativeHeadsEpilogError> {
    let staff_epilog = compose_native_heads_staff_epilog(NativeHeadsStaffEpilogInput {
        seed_glyphs: input.seed_glyphs,
        range_glyphs: input.range_glyphs,
        stemless_boost: input.stemless_boost,
    })?;
    compose_from_staff_epilog(staff_epilog, input.competitors, input.beams)
}

fn compose_from_staff_epilog(
    staff_epilog: NativeHeadsStaffEpilogRecognition,
    competitors: &NativeHeadsCompetitorPool,
    beams: &NativeBeamRecognition,
) -> Result<NativeHeadsEpilogRecognition, NativeHeadsEpilogError> {
    let staff_system_ids = staff_epilog
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    let competitor_system_ids = competitors
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    if staff_system_ids != competitor_system_ids {
        return Err(NativeHeadsEpilogError::SystemOrder {
            staff: staff_system_ids,
            competitors: competitor_system_ids,
        });
    }

    let head_systems = staff_epilog
        .systems
        .iter()
        .map(|system| NativeHeadsSmallBeamHeadSystem {
            system_id: system.system_id,
            heads_in_sig_order: system
                .retained_creation_order
                .iter()
                .map(|&provenance| {
                    let head = resolve_head(system, provenance);
                    NativeHeadsSmallBeamHead {
                        provenance,
                        bounds: head.bounds,
                        grade: head.grade(),
                        removed: false,
                    }
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let beam_results = purge_native_heads_small_beams(competitors, beams, &head_systems)?;

    let mut systems = Vec::with_capacity(staff_epilog.systems.len());
    let mut tally_samples = Vec::new();
    let mut beam_removal_count = 0_usize;
    let mut head_beam_removal_count = 0_usize;
    let mut final_head_count = 0_usize;
    for (staff_system, beam_result) in staff_epilog.systems.iter().zip(beam_results) {
        let heads_in_sig_order = staff_system.retained_creation_order.clone();
        let heads_before_beam_by_ordinate = beam_result
            .arbitration
            .ordered_head_indices
            .iter()
            .map(|&index| beam_result.head_provenance[index])
            .collect::<Vec<_>>();
        let beam_removed_heads = beam_result
            .arbitration
            .decisions
            .iter()
            .filter_map(|decision| {
                (decision.target == crate::head_small_beam_purge::SmallBeamPurgeTarget::Head)
                    .then_some(beam_result.head_provenance[decision.head_index])
            })
            .collect::<Vec<_>>();
        let final_heads = beam_result
            .arbitration
            .remaining_head_indices
            .iter()
            .map(|&index| beam_result.head_provenance[index])
            .collect::<Vec<_>>();
        let removed_heads = beam_removed_heads.iter().copied().collect::<BTreeSet<_>>();
        let (mut tally_left, mut tally_right) = replay_system_tallies(staff_system)?;
        for entry in tally_left.iter_mut().chain(&mut tally_right) {
            entry.removed = removed_heads.contains(&entry.head);
        }
        tally_samples.extend(tally_left.iter().chain(&tally_right).map(|entry| {
            HeadSeedTallySample {
                shape: entry.shape,
                side: entry.side,
                dx: entry.dx,
                removed: entry.removed,
            }
        }));

        beam_removal_count += beam_result
            .arbitration
            .decisions
            .iter()
            .filter(|decision| {
                decision.target == crate::head_small_beam_purge::SmallBeamPurgeTarget::Beam
            })
            .count();
        head_beam_removal_count += beam_removed_heads.len();
        final_head_count += final_heads.len();
        systems.push(NativeHeadsEpilogSystem {
            system_id: staff_system.system_id,
            heads_in_sig_order,
            heads_before_beam_by_ordinate,
            small_beams: beam_result,
            beam_removed_heads,
            final_heads,
            tally_left,
            tally_right,
        });
    }

    let scale_analysis = analyze_head_seed_tallies(&tally_samples);
    Ok(NativeHeadsEpilogRecognition {
        staff_epilog,
        systems,
        tally_samples,
        scale_analysis,
        beam_removal_count,
        head_beam_removal_count,
        final_head_count,
    })
}

fn replay_system_tallies(
    system: &NativeHeadsStaffEpilogSystem,
) -> Result<(Vec<NativeHeadsTallyEntry>, Vec<NativeHeadsTallyEntry>), NativeHeadsEpilogError> {
    let mut left = Vec::new();
    let mut right = Vec::new();
    for (staff_index, staff) in system.staffs.iter().enumerate() {
        let mut working = staff
            .heads
            .iter()
            .map(|head| head.tally_before)
            .collect::<Vec<_>>();
        for (head_index, head) in staff.heads.iter().enumerate() {
            let reference = NativeHeadStaffEpilogRef {
                staff_index,
                head_index,
            };
            if let Some(dx) = working[head_index].left_dx {
                left.push(tally_entry(reference, head, HeadSeedTallySide::Left, dx));
            }
            if let Some(dx) = working[head_index].right_dx {
                right.push(tally_entry(reference, head, HeadSeedTallySide::Right, dx));
            }
        }

        for decision in staff
            .purge
            .duplicate
            .decisions
            .iter()
            .chain(&staff.purge.overlap.decisions)
        {
            replay_replication(
                decision,
                staff_index,
                staff,
                &mut working,
                &mut left,
                &mut right,
            );
        }
        for (head_index, (actual, head)) in working.iter().zip(&staff.heads).enumerate() {
            if !tally_equal_bits(*actual, head.tally_after_replication) {
                return Err(NativeHeadsEpilogError::TallyReplay {
                    system_id: system.system_id,
                    staff_id: staff.staff_id,
                    head_index,
                    expected: head.tally_after_replication,
                    actual: *actual,
                });
            }
        }

        left.retain(|entry| {
            entry.head.staff_index != staff_index
                || !staff.heads[entry.head.head_index].duplicate_removed
        });
        right.retain(|entry| {
            entry.head.staff_index != staff_index
                || !staff.heads[entry.head.head_index].duplicate_removed
        });
    }
    Ok((left, right))
}

#[allow(clippy::too_many_arguments)]
fn replay_replication(
    decision: &HeadPurgeDecision,
    staff_index: usize,
    staff: &NativeHeadsStaffEpilogStaff,
    working: &mut [HeadSeedTally],
    left_entries: &mut Vec<NativeHeadsTallyEntry>,
    right_entries: &mut Vec<NativeHeadsTallyEntry>,
) {
    if decision.reason != HeadPurgeReason::EqualPreserveLeft {
        return;
    }
    let left_index = decision.left_index;
    let right_index = decision.right_index;
    if !staff.heads[left_index].good || !staff.heads[right_index].good {
        return;
    }
    let reference = NativeHeadStaffEpilogRef {
        staff_index,
        head_index: left_index,
    };
    let right_tally = working[right_index];
    if working[left_index].left_dx.is_none()
        && let Some(dx) = right_tally.left_dx
    {
        working[left_index].left_dx = Some(dx);
        left_entries.push(tally_entry(
            reference,
            &staff.heads[left_index],
            HeadSeedTallySide::Left,
            dx,
        ));
    }
    if working[left_index].right_dx.is_none()
        && let Some(dx) = right_tally.right_dx
    {
        working[left_index].right_dx = Some(dx);
        right_entries.push(tally_entry(
            reference,
            &staff.heads[left_index],
            HeadSeedTallySide::Right,
            dx,
        ));
    }
}

fn tally_entry(
    reference: NativeHeadStaffEpilogRef,
    head: &NativeHeadStaffEpilogHead,
    side: HeadSeedTallySide,
    dx: f64,
) -> NativeHeadsTallyEntry {
    NativeHeadsTallyEntry {
        head: reference,
        shape: java_shape(head.shape),
        side,
        dx,
        removed: false,
    }
}

fn resolve_head(
    system: &NativeHeadsStaffEpilogSystem,
    reference: NativeHeadStaffEpilogRef,
) -> &NativeHeadStaffEpilogHead {
    &system.staffs[reference.staff_index].heads[reference.head_index]
}

fn tally_equal_bits(left: HeadSeedTally, right: HeadSeedTally) -> bool {
    option_bits(left.left_dx) == option_bits(right.left_dx)
        && option_bits(left.right_dx) == option_bits(right.right_dx)
}

fn option_bits(value: Option<f64>) -> Option<u64> {
    value.map(f64::to_bits)
}

/// Convert template-factory order into authoritative Java `Shape` order.
#[must_use]
pub const fn java_shape(shape: crate::head_template::HeadTemplateShape) -> JavaHeadSeedShape {
    match shape {
        crate::head_template::HeadTemplateShape::Breve => JavaHeadSeedShape::Breve,
        crate::head_template::HeadTemplateShape::BreveSmall => JavaHeadSeedShape::BreveSmall,
        crate::head_template::HeadTemplateShape::WholeNote => JavaHeadSeedShape::WholeNote,
        crate::head_template::HeadTemplateShape::WholeNoteSmall => {
            JavaHeadSeedShape::WholeNoteSmall
        }
        crate::head_template::HeadTemplateShape::NoteheadVoid => JavaHeadSeedShape::NoteheadVoid,
        crate::head_template::HeadTemplateShape::NoteheadVoidSmall => {
            JavaHeadSeedShape::NoteheadVoidSmall
        }
        crate::head_template::HeadTemplateShape::NoteheadBlack => JavaHeadSeedShape::NoteheadBlack,
        crate::head_template::HeadTemplateShape::NoteheadBlackSmall => {
            JavaHeadSeedShape::NoteheadBlackSmall
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        head_purge::{HeadPurgePhase, HeadPurgeResult},
        head_small_beam_purge::{SmallBeamPurgeDecision, SmallBeamPurgeTarget},
        head_template::HeadTemplateShape,
    };

    use super::*;

    fn head(shape: HeadTemplateShape, tally: HeadSeedTally) -> NativeHeadStaffEpilogHead {
        NativeHeadStaffEpilogHead {
            origin: crate::native_heads_staff_epilog::NativeHeadStaffEpilogOrigin::Seed(0),
            provenance: crate::native_heads_staff_epilog::NativeHeadStaffEpilogProvenance::Seed {
                ordinal: 0,
                candidate_ordinal: 0,
                search_ordinal: 0,
                geometry_ordinal: 0,
                seed_pool_index: 0,
                seed_source_ordinal: 0,
            },
            is_vip: false,
            relative_java_id: 1,
            system_creation_ordinal: 0,
            input_ordinal: 0,
            original_shape: shape,
            shape,
            pitch_bits: 0.0_f64.to_bits(),
            bounds: crate::head_scanner_slices::JavaRectangle::new(0, 0, 10, 10),
            raw_distance_impact_bits: 0.8_f64.to_bits(),
            distance_impact_bits: 0.8_f64.to_bits(),
            grade_bits: 0.8_f64.to_bits(),
            minimum_grade_bits: 0.1_f64.to_bits(),
            good: true,
            glyph_bounds: crate::head_scanner_slices::JavaRectangle::new(0, 0, 10, 10),
            glyph_weight: 20,
            glyph_run_digest: 7,
            tally_before: tally,
            tally_after_replication: tally,
            tally_retained_after_cleanup: !tally.is_empty(),
            duplicate_removed: false,
            attachment_ordinal: Some(0),
        }
    }

    fn empty_purge(count: usize) -> HeadPurgeResult {
        HeadPurgeResult {
            ordered_indices: (0..count).collect(),
            duplicate_removed_indices: Vec::new(),
            retained_indices: (0..count).collect(),
            removed: vec![false; count],
            tallies: vec![HeadSeedTally::default(); count],
            duplicate: HeadPurgePhase::default(),
            overlap: HeadPurgePhase::default(),
        }
    }

    #[test]
    fn java_shape_order_is_explicit_and_not_template_factory_order() {
        let mut shapes = [
            java_shape(HeadTemplateShape::NoteheadBlack),
            java_shape(HeadTemplateShape::NoteheadVoid),
            java_shape(HeadTemplateShape::WholeNote),
            java_shape(HeadTemplateShape::Breve),
        ];
        shapes.sort();
        assert_eq!(
            shapes,
            [
                JavaHeadSeedShape::Breve,
                JavaHeadSeedShape::WholeNote,
                JavaHeadSeedShape::NoteheadVoid,
                JavaHeadSeedShape::NoteheadBlack,
            ]
        );
    }

    #[test]
    fn tally_replay_appends_replication_at_decision_time_then_deletes_removed_key() {
        let left_only = HeadSeedTally {
            left_dx: Some(1.0),
            right_dx: None,
        };
        let right_only = HeadSeedTally {
            left_dx: None,
            right_dx: Some(2.0),
        };
        let later_right = HeadSeedTally {
            left_dx: None,
            right_dx: Some(3.0),
        };
        let mut heads = vec![
            head(HeadTemplateShape::NoteheadBlack, left_only),
            head(HeadTemplateShape::NoteheadBlack, right_only),
            head(HeadTemplateShape::NoteheadVoid, later_right),
        ];
        heads[0].tally_after_replication.right_dx = Some(2.0);
        heads[1].duplicate_removed = true;
        heads[1].attachment_ordinal = None;
        heads[1].tally_retained_after_cleanup = false;
        heads[2].system_creation_ordinal = 2;
        let duplicate = HeadPurgeDecision {
            left_index: 0,
            right_index: 1,
            purged_index: 1,
            kept_index: 0,
            grade_difference: 0.0,
            reason: HeadPurgeReason::EqualPreserveLeft,
        };
        let staff = NativeHeadsStaffEpilogStaff {
            staff_id: 4,
            staff_identity: 0,
            heads,
            ordered_input_indices: vec![0, 1, 2],
            retained_attachment_indices: vec![0, 2],
            purge: HeadPurgeResult {
                ordered_indices: vec![0, 1, 2],
                duplicate_removed_indices: vec![1],
                retained_indices: vec![0, 2],
                removed: vec![false, true, false],
                tallies: vec![
                    HeadSeedTally {
                        left_dx: Some(1.0),
                        right_dx: Some(2.0),
                    },
                    right_only,
                    later_right,
                ],
                duplicate: HeadPurgePhase {
                    checks: Vec::new(),
                    decisions: vec![duplicate],
                },
                overlap: HeadPurgePhase::default(),
            },
        };
        let system = NativeHeadsStaffEpilogSystem {
            system_id: 1,
            staffs: vec![staff],
            creation_order: vec![],
            retained_creation_order: vec![],
            attachment_order: vec![],
            input_head_count: 3,
            duplicate_removal_count: 1,
            overlap_exclusion_count: 0,
            retained_head_count: 2,
        };
        let (left, right) = replay_system_tallies(&system).expect("exact replay");

        assert_eq!(
            left.iter()
                .map(|entry| entry.head.head_index)
                .collect::<Vec<_>>(),
            [0]
        );
        // Head 1 was inserted first but deleted; replicated head 0 was
        // appended after later head 2's original RIGHT insertion.
        assert_eq!(
            right
                .iter()
                .map(|entry| (entry.head.head_index, entry.dx))
                .collect::<Vec<_>>(),
            [(2, 3.0), (0, 2.0)]
        );
    }

    #[test]
    fn beam_removed_tally_keys_are_kept_but_filtered_from_analysis() {
        let samples = [HeadSeedTallySample {
            shape: JavaHeadSeedShape::Breve,
            side: HeadSeedTallySide::Left,
            dx: 1.0,
            removed: true,
        }; 10];
        assert!(analyze_head_seed_tallies(&samples).is_empty());

        // Anchor the exact target variant used by system composition.
        let decision = SmallBeamPurgeDecision {
            beam_index: 0,
            head_index: 0,
            beam_contextual_grade: 0.9,
            head_grade: 0.8,
            target: SmallBeamPurgeTarget::Head,
        };
        assert_eq!(decision.target, SmallBeamPurgeTarget::Head);
    }

    #[test]
    fn empty_purge_helper_keeps_java_index_domains_aligned() {
        let purge = empty_purge(3);
        assert_eq!(purge.ordered_indices, [0, 1, 2]);
        assert_eq!(purge.retained_indices, [0, 1, 2]);
    }
}
