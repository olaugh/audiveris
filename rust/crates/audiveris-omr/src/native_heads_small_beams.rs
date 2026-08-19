// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production adapter from the native HEADS competitor pool to the exact
//! `NoteHeadsBuilder.purgeSmallBeams` kernel.
//!
//! `NativeHeadsCompetitorSystem::candidates_in_sig_order` is the retained SIG
//! insertion stream after BEAMS and multiple-rest replacement. This adapter
//! selects its beam shapes without consulting HEADS competitor acceptance:
//! Java's epilog queries all live `ShapeSet.Beams` nodes, including a beam that
//! was not good enough to protect template lookup. Width filtering remains in
//! the pure kernel so its strict integer cutoff is exercised in production.

use std::{collections::BTreeMap, error::Error, fmt};

use audiveris_core::grade::contextual;

use crate::{
    beam_inters::BeamKind,
    beam_veto::BeamVetoScale,
    head_scanner_slices::JavaRectangle,
    head_small_beam_purge::{
        HeadBeamShape, SmallBeamPurgeBeam, SmallBeamPurgeHead, SmallBeamPurgeResult,
        purge_small_beams, purge_small_beams_with_contextual_grades,
    },
    native_heads_competitors::{
        NativeHeadsCompetitorGeometry, NativeHeadsCompetitorPool, NativeHeadsCompetitorShape,
        NativeHeadsCompetitorSource, NativeHeadsCompetitorSystem,
    },
    recognize::NativeBeamRecognition,
};

/// Head input independent of the concrete native staff-epilog record type.
///
/// Callers retain their own identity in `provenance`; the adapter never
/// invents a Java `InterIndex` value. Input order must be system SIG insertion
/// order, because Java's stable ordinate sort uses that order to break ties.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsSmallBeamHead<Provenance> {
    pub provenance: Provenance,
    pub bounds: JavaRectangle,
    pub grade: f64,
    pub removed: bool,
}

/// Caller-owned head stream for one system in a pool-wide invocation.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsSmallBeamHeadSystem<Provenance> {
    pub system_id: usize,
    pub heads_in_sig_order: Vec<NativeHeadsSmallBeamHead<Provenance>>,
}

/// Exact native source retained for each beam input passed to the kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeHeadsSmallBeamBeamProvenance {
    pub source: NativeHeadsCompetitorSource,
    pub source_ordinal: usize,
}

/// One system's adapted inputs and pure arbitration result.
///
/// `head_provenance` aligns with every head-index field in `arbitration`;
/// `beam_provenance` aligns with every beam-index field. The beam vector
/// contains every live beam-shape candidate in SIG order, not only the narrow
/// candidates listed by `arbitration.candidate_beam_indices`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsSmallBeamSystemResult<Provenance> {
    pub system_id: usize,
    pub min_beam_width: i32,
    pub head_provenance: Vec<Provenance>,
    pub head_inputs: Vec<SmallBeamPurgeHead>,
    pub beam_provenance: Vec<NativeHeadsSmallBeamBeamProvenance>,
    pub beam_inputs: Vec<SmallBeamPurgeBeam>,
    pub arbitration: SmallBeamPurgeResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeHeadsSmallBeamError {
    MissingHeadSystem(usize),
    DuplicateHeadSystem(usize),
    UnexpectedHeadSystem(usize),
    NonHorizontalBeam {
        system_id: usize,
        source_ordinal: usize,
        shape: NativeHeadsCompetitorShape,
    },
    MissingBeamGroupSystem(usize),
    InvalidBeamGroupMember {
        system_id: usize,
        member_ordinal: usize,
    },
    MissingBeamContext {
        system_id: usize,
        source: NativeHeadsCompetitorSource,
    },
}

impl fmt::Display for NativeHeadsSmallBeamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHeadSystem(system_id) => {
                write!(
                    formatter,
                    "missing HEADS epilog input for system {system_id}"
                )
            }
            Self::DuplicateHeadSystem(system_id) => {
                write!(
                    formatter,
                    "duplicate HEADS epilog input for system {system_id}"
                )
            }
            Self::UnexpectedHeadSystem(system_id) => {
                write!(
                    formatter,
                    "HEADS epilog input names unknown system {system_id}"
                )
            }
            Self::NonHorizontalBeam {
                system_id,
                source_ordinal,
                shape,
            } => write!(
                formatter,
                "system {system_id} SIG source {source_ordinal} has beam shape {shape:?} without horizontal geometry"
            ),
            Self::MissingBeamGroupSystem(system_id) => {
                write!(
                    formatter,
                    "missing native beam groups for system {system_id}"
                )
            }
            Self::InvalidBeamGroupMember {
                system_id,
                member_ordinal,
            } => write!(
                formatter,
                "system {system_id} beam group names invalid member {member_ordinal}"
            ),
            Self::MissingBeamContext { system_id, source } => write!(
                formatter,
                "system {system_id} has no contextual beam grade for {source:?}"
            ),
        }
    }
}

impl Error for NativeHeadsSmallBeamError {}

/// Adapt and arbitrate one native competitor system.
pub fn purge_native_heads_small_beams_system<Provenance: Clone>(
    system: &NativeHeadsCompetitorSystem,
    recognition: &NativeBeamRecognition,
    heads_in_sig_order: &[NativeHeadsSmallBeamHead<Provenance>],
    beam_veto_scale: Option<BeamVetoScale>,
) -> Result<NativeHeadsSmallBeamSystemResult<Provenance>, NativeHeadsSmallBeamError> {
    let contextual_grades = beam_contextual_grades(recognition, system.system_id, &[])?;
    let mut result = purge_native_heads_small_beams_system_with_contextual_grades(
        system,
        heads_in_sig_order,
        &contextual_grades,
        beam_veto_scale,
    )?;
    let provenance = result.beam_provenance.clone();
    let arbitration = purge_small_beams_with_contextual_grades(
        &result.head_inputs,
        &result.beam_inputs,
        system.min_beam_width,
        |beam_index, beam_removed, _| {
            let removed_sources = provenance
                .iter()
                .enumerate()
                .filter_map(|(index, beam)| beam_removed[index].then_some(beam.source))
                .collect::<Vec<_>>();
            beam_contextual_grades(recognition, system.system_id, &removed_sources)
                .expect("validated native beam groups remain valid after removals")
                .into_iter()
                .find_map(|(source, grade)| {
                    (source == provenance[beam_index].source).then_some(grade)
                })
                .expect("candidate beam retains its contextual-grade record")
        },
    );
    for (beam, grade) in result
        .beam_inputs
        .iter_mut()
        .zip(&arbitration.computed_contextual_grades)
    {
        if let Some(grade) = grade {
            beam.contextual_grade = *grade;
        }
    }
    result.arbitration = arbitration;
    Ok(result)
}

/// Adapt one system using an already-computed source/contextual-grade table.
///
/// This is the narrow test and replay seam. Production callers should use
/// [`purge_native_heads_small_beams_system`], which derives this table from
/// live native `BeamGroupInter` membership and exclusions.
pub fn purge_native_heads_small_beams_system_with_contextual_grades<Provenance: Clone>(
    system: &NativeHeadsCompetitorSystem,
    heads_in_sig_order: &[NativeHeadsSmallBeamHead<Provenance>],
    contextual_grades: &[(NativeHeadsCompetitorSource, f64)],
    beam_veto_scale: Option<BeamVetoScale>,
) -> Result<NativeHeadsSmallBeamSystemResult<Provenance>, NativeHeadsSmallBeamError> {
    let heads = heads_in_sig_order
        .iter()
        .map(|head| SmallBeamPurgeHead {
            bounds: head.bounds,
            grade: head.grade,
            removed: head.removed,
        })
        .collect::<Vec<_>>();

    let mut beams = Vec::new();
    let mut beam_provenance = Vec::new();
    for candidate in &system.candidates_in_sig_order {
        let shape = match candidate.shape {
            NativeHeadsCompetitorShape::Beam => HeadBeamShape::Beam,
            NativeHeadsCompetitorShape::BeamHook => HeadBeamShape::BeamHook,
            NativeHeadsCompetitorShape::BeamSmall => HeadBeamShape::BeamSmall,
            _ => continue,
        };
        let NativeHeadsCompetitorGeometry::Horizontal { median, height } = candidate.geometry
        else {
            return Err(NativeHeadsSmallBeamError::NonHorizontalBeam {
                system_id: system.system_id,
                source_ordinal: candidate.source_ordinal,
                shape: candidate.shape,
            });
        };
        // Credible-beam gate (enhancement, None = Java): a sub-scale beam is
        // withheld from arbitration entirely, so it can neither delete a head
        // nor be deleted itself; both hypotheses stay for later resolution.
        if beam_veto_scale.is_some_and(|scale| {
            !scale.credible(
                f64::from(candidate.bounds.width),
                f64::from(candidate.bounds.height),
            )
        }) {
            continue;
        }

        beams.push(SmallBeamPurgeBeam {
            shape,
            bounds: candidate.bounds,
            median,
            height,
            contextual_grade: contextual_grades
                .iter()
                .find_map(|(source, grade)| (*source == candidate.source).then_some(*grade))
                .ok_or(NativeHeadsSmallBeamError::MissingBeamContext {
                    system_id: system.system_id,
                    source: candidate.source,
                })?,
            removed: false,
        });
        beam_provenance.push(NativeHeadsSmallBeamBeamProvenance {
            source: candidate.source,
            source_ordinal: candidate.source_ordinal,
        });
    }

    let arbitration = purge_small_beams(&heads, &beams, system.min_beam_width);
    Ok(NativeHeadsSmallBeamSystemResult {
        system_id: system.system_id,
        min_beam_width: system.min_beam_width,
        head_provenance: heads_in_sig_order
            .iter()
            .map(|head| head.provenance.clone())
            .collect(),
        head_inputs: heads,
        beam_provenance,
        beam_inputs: beams,
        arbitration,
    })
}

/// Adapt and arbitrate every system of a native competitor pool in pool order.
///
/// The head-system relation is intentionally strict. Silently treating a
/// missing system as headless would turn every narrow beam into an unexplained
/// survivor and weaken the end-to-end oracle boundary.
pub fn purge_native_heads_small_beams<Provenance: Clone>(
    pool: &NativeHeadsCompetitorPool,
    recognition: &NativeBeamRecognition,
    head_systems: &[NativeHeadsSmallBeamHeadSystem<Provenance>],
    beam_veto_scale: Option<BeamVetoScale>,
) -> Result<Vec<NativeHeadsSmallBeamSystemResult<Provenance>>, NativeHeadsSmallBeamError> {
    let mut heads_by_system = BTreeMap::new();
    for heads in head_systems {
        if heads_by_system
            .insert(heads.system_id, heads.heads_in_sig_order.as_slice())
            .is_some()
        {
            return Err(NativeHeadsSmallBeamError::DuplicateHeadSystem(
                heads.system_id,
            ));
        }
    }

    for system_id in heads_by_system.keys().copied() {
        if !pool
            .systems
            .iter()
            .any(|system| system.system_id == system_id)
        {
            return Err(NativeHeadsSmallBeamError::UnexpectedHeadSystem(system_id));
        }
    }

    pool.systems
        .iter()
        .map(|system| {
            let heads = heads_by_system.get(&system.system_id).copied().ok_or(
                NativeHeadsSmallBeamError::MissingHeadSystem(system.system_id),
            )?;
            purge_native_heads_small_beams_system(system, recognition, heads, beam_veto_scale)
        })
        .collect()
}

const BEAM_BEAM_SUPPORT_COEFFICIENT: f64 = 3.0;

#[derive(Clone, Copy, Debug)]
struct ContextBeam {
    source: NativeHeadsCompetitorSource,
    grade: f64,
    removed: bool,
    exclusion_key: Option<usize>,
}

fn beam_contextual_grades(
    recognition: &NativeBeamRecognition,
    system_id: usize,
    removed_sources: &[NativeHeadsCompetitorSource],
) -> Result<Vec<(NativeHeadsCompetitorSource, f64)>, NativeHeadsSmallBeamError> {
    let removed_raw = recognition
        .multiple_rests
        .iter()
        .map(|rest| rest.source_beam_ordinal)
        .collect::<Vec<_>>();
    // Group member ordinals use all raw system beams followed by all secondary
    // hooks. Raw ordinals remain global source identities.
    let members = recognition
        .raw_beams
        .iter()
        .enumerate()
        .filter(|(_, (id, _))| *id == system_id)
        .map(|(ordinal, (_, beam))| ContextBeam {
            source: NativeHeadsCompetitorSource::RawBeam(ordinal),
            grade: beam.grade,
            removed: removed_raw.contains(&ordinal),
            exclusion_key: raw_hook_beam_exclusion_key(recognition, ordinal),
        })
        .chain(
            recognition
                .hooks
                .iter()
                .enumerate()
                .filter(|(_, (id, _))| *id == system_id)
                .map(|(ordinal, (_, beam))| ContextBeam {
                    source: NativeHeadsCompetitorSource::Hook(ordinal),
                    grade: beam.grade,
                    removed: false,
                    exclusion_key: None,
                }),
        )
        .collect::<Vec<_>>();
    let groups = recognition
        .group_memberships
        .iter()
        .find(|membership| membership.system_id == system_id)
        .ok_or(NativeHeadsSmallBeamError::MissingBeamGroupSystem(system_id))?;
    let mut grades = Vec::new();

    for group in &groups.groups {
        let mut live = Vec::new();
        for &member_ordinal in group {
            let member = members.get(member_ordinal).copied().ok_or(
                NativeHeadsSmallBeamError::InvalidBeamGroupMember {
                    system_id,
                    member_ordinal,
                },
            )?;
            if !member.removed && !removed_sources.contains(&member.source) {
                live.push(member);
            }
        }
        for focus in &live {
            grades.push((focus.source, contextual_beam_grade(*focus, &live)));
        }
    }
    Ok(grades)
}

/// Initial `createBeamInters` inserts an exclusion only for the hook and beam
/// created from the same item. They are adjacent in the global raw stream,
/// hook first. Secondary `buildHooks` products are never in such a pair.
pub(crate) fn raw_hook_beam_exclusion_key(
    recognition: &NativeBeamRecognition,
    ordinal: usize,
) -> Option<usize> {
    let &(system_id, beam) = recognition.raw_beams.get(ordinal)?;
    match beam.kind {
        BeamKind::Hook => recognition
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
            .and_then(|previous| {
                recognition
                    .raw_beams
                    .get(previous)
                    .map(|entry| (previous, entry))
            })
            .filter(|(_, (other_system, other))| {
                *other_system == system_id
                    && other.kind == BeamKind::Hook
                    && other.item == beam.item
            })
            .map(|(previous, _)| previous),
        BeamKind::SmallBeam => None,
    }
}

fn contextual_beam_grade(focus: ContextBeam, group: &[ContextBeam]) -> f64 {
    let mut partners = group
        .iter()
        .copied()
        .filter(|partner| {
            partner.source != focus.source && !beam_members_are_exclusive(focus, *partner)
        })
        .collect::<Vec<_>>();
    if partners.is_empty() {
        return focus.grade;
    }

    // Java getPartitions mutates the support-partner list with the stable
    // `Inters.byReverseGrade` ordering before accumulating contributions.
    partners.sort_by(|left, right| java_double_compare(right.grade, left.grade));
    let partitions = java_largest_non_conflicting_partitions(&partners);
    let mut best_contextual = 0.0;
    for partition in partitions {
        let mut contribution = 0.0;
        for partner_index in partition {
            contribution += partners[partner_index].grade * BEAM_BEAM_SUPPORT_COEFFICIENT;
        }
        best_contextual = java_math_max(best_contextual, contextual(focus.grade, contribution));
    }
    best_contextual
}

fn beam_members_are_exclusive(one: ContextBeam, two: ContextBeam) -> bool {
    one.exclusion_key.is_some() && one.exclusion_key == two.exclusion_key
}

fn java_largest_non_conflicting_partitions(partners: &[ContextBeam]) -> Vec<Vec<usize>> {
    let count = partners.len();
    let mut concurrents = vec![Vec::new(); count];
    let mut conflict = false;
    for left in 0..count {
        for right in (left + 1)..count {
            if beam_members_are_exclusive(partners[left], partners[right]) {
                concurrents[left].push(right);
                conflict = true;
            }
        }
    }
    if !conflict {
        return vec![(0..count).collect()];
    }

    let mut sequences = vec![vec![0_i8; count]];
    for index in 0..count {
        let existing_count = sequences.len();
        for sequence_index in 0..existing_count {
            if sequences[sequence_index][index] == -1 {
                continue;
            }
            sequences[sequence_index][index] = 1;
            if !concurrents[index].is_empty() {
                let mut alternative = sequences[sequence_index].clone();
                alternative[index] = 0;
                sequences.push(alternative);
                for &concurrent in &concurrents[index] {
                    sequences[sequence_index][concurrent] = -1;
                }
            }
        }
    }
    sequences
        .into_iter()
        .map(|sequence| {
            sequence
                .into_iter()
                .enumerate()
                .filter_map(|(index, selected)| (selected == 1).then_some(index))
                .collect()
        })
        .collect()
}

fn java_double_compare(one: f64, two: f64) -> std::cmp::Ordering {
    if one < two {
        return std::cmp::Ordering::Less;
    }
    if one > two {
        return std::cmp::Ordering::Greater;
    }
    let one_bits = java_double_to_long_bits(one) as i64;
    let two_bits = java_double_to_long_bits(two) as i64;
    one_bits.cmp(&two_bits)
}

fn java_double_to_long_bits(value: f64) -> u64 {
    if value.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        value.to_bits()
    }
}

fn java_math_max(one: f64, two: f64) -> f64 {
    if one.is_nan() || two.is_nan() {
        return f64::NAN;
    }
    if one == 0.0 && two == 0.0 {
        return if one.is_sign_positive() || two.is_sign_positive() {
            0.0
        } else {
            -0.0
        };
    }
    if one >= two { one } else { two }
}

#[cfg(test)]
mod contextual_tests {
    use super::*;

    fn member(source: usize, grade: f64, exclusion_key: Option<usize>) -> ContextBeam {
        ContextBeam {
            source: NativeHeadsCompetitorSource::RawBeam(source),
            grade,
            removed: false,
            exclusion_key,
        }
    }

    #[test]
    fn group_support_uses_ratio_four_and_java_contextual_arithmetic() {
        let group = [member(0, 0.5, None), member(1, 0.25, None)];
        let expected = contextual(0.5, 0.25 * 3.0);
        assert_eq!(
            contextual_beam_grade(group[0], &group).to_bits(),
            expected.to_bits()
        );
    }

    #[test]
    fn excluded_focus_partner_has_no_beam_beam_support() {
        let group = [member(0, 0.5, Some(7)), member(1, 0.9, Some(7))];
        assert_eq!(
            contextual_beam_grade(group[0], &group).to_bits(),
            0.5_f64.to_bits()
        );
    }

    #[test]
    fn mutually_exclusive_partners_form_java_reverse_grade_partitions() {
        let group = [
            member(0, 0.2, None),
            member(1, 0.7, Some(8)),
            member(2, 0.4, Some(8)),
            member(3, 0.1, None),
        ];
        let expected = contextual(0.2, (0.7 * 3.0) + (0.1 * 3.0));
        assert_eq!(
            contextual_beam_grade(group[0], &group).to_bits(),
            expected.to_bits()
        );
    }
}
