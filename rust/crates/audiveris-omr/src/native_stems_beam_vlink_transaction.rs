// SPDX-License-Identifier: AGPL-3.0-or-later

//! First persistent transaction of a beam-origin `VLinker.link` call.
//!
//! The scheduler frontier and isolated link plan stop immediately before
//! `StemBuilder.createStem`. This module commits the pending aliased line
//! shift, then reproduces only `createStem`: fixed-glyph composition,
//! `GlyphIndex.registerOriginal`, `systemStems` reuse, `StemChecker`, and the
//! checked/artificial/null result. It deliberately stops before the VLinker's
//! stem-reuse loop, `BeamStemRelation.checkLink`, any SIG edit, or any linker
//! flag mutation.
//!
//! A Java `GlyphIndex` lookup is sheet-global. A digest of that index cannot
//! prove `Glyph.equals`, so the public boundary accepts either an exact
//! transaction-known canonical entry or a one-shot exhaustive equality-scan
//! certificate whose candidate contains the full `RunTable`. The certificate
//! makes no claim about unrelated future candidates.

use std::{collections::BTreeSet, error::Error, fmt};

use audiveris_core::basic_line::BasicLine;
use audiveris_image::{
    beam_structure::Segment,
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable, RunTableError},
    section::Bounds,
};

use crate::{
    beam_recognizer::run_table_center_line,
    head_scanner_slices::{JavaRectangle, VerticalRibbonArea},
    native_stems_beam_builders::{
        NativeStemsBeamBuilderGlyphRef, NativeStemsBeamBuilderSystem,
        NativeStemsModeledCanonicalGlyph,
    },
    native_stems_beam_link_plans::{
        NativeStemsBeamLinkPlanAttempt, NativeStemsBeamLinkPlanOutcome,
        NativeStemsBeamLinkPlanSystem, NativeStemsBeamSelectedGlyph,
    },
    native_stems_beam_scheduler::{
        NativeStemsBeamDeferredLineDelta, NativeStemsBeamPlanRef, NativeStemsBeamSchedulerStatus,
        NativeStemsBeamSchedulerSystem,
    },
    native_stems_beam_vlinkers::NativeStemsBeamVLinkerRef,
    native_stems_head_builders::NativeStemsHeadBuilderRecognition,
    stem_seeds_step::{
        NativeStemCheckResult, NativeStemCheckerParameters, NativeStemGeometryError,
        NeutralStemGeometry, check_native_stem,
    },
    stems_step::{NativeStemLine, NativeStemPoint},
};

/// Whether an oracle/state snapshot belongs to a true shared-sheet serial
/// execution or to an explicitly isolated system frontier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamVLinkTransactionScope {
    /// The natural allocator chronology is honest only at the sheet's first
    /// system frontier. Audiveris system IDs are one-based in sheet order.
    SharedSheetFirstFrontier {
        system_id: usize,
    },
    /// A later frontier in the same sheet/system chronology. The allocator,
    /// glyph registry, line states, and `systemStems` entries are carried from
    /// the preceding committed transaction rather than reconstructed.
    SharedSheetSerial {
        system_id: usize,
    },
    IsolatedFreshSheetFrontier {
        system_id: usize,
    },
}

/// Three views of Java's one `Sheet.lastPersistentId` `AtomicInteger`.
/// `GlyphIndex` and `InterIndex` do not own independent allocators.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamPersistentIdState {
    pub sheet_last_id: i32,
    pub glyph_index_last_id: i32,
    pub inter_index_last_id: i32,
}

/// Collision-free fixed `Glyph.equals` content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamFixedGlyphContent {
    pub bounds: Bounds,
    pub weight: usize,
    pub run_table: RunTable,
}

/// A canonical original whose exact content became known to this transaction
/// boundary. `glyph_id` is also its stable normalized object identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamKnownCanonicalGlyph {
    /// Stable oracle alias `glyph:<Java ID>`.
    pub canonical_alias: usize,
    pub glyph_id: i32,
    pub content: NativeStemsBeamFixedGlyphContent,
    pub active_in_index: bool,
    /// False for a rejected transient compound when no selected or previously
    /// known canonical object supplies a strong reference. Java then retains
    /// the registered/reused original through weak indexes only, so a later
    /// lookup requires a fresh liveness scan.
    pub strongly_retained: bool,
}

/// Exact Java object selected by the immutable link plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamSelectedGlyphBinding {
    pub reference: NativeStemsBeamBuilderGlyphRef,
    pub canonical_alias: usize,
    pub glyph_id: i32,
    pub content: NativeStemsBeamFixedGlyphContent,
}

/// One page-bootstrap identity joined to exact native glyph content.
///
/// The bootstrap is deliberately separate from a transaction fixture. The
/// port still does not own the page-wide `GlyphIndex` construction, so callers
/// must disclose this input; once supplied, later frontiers select it by full
/// bounds/weight/RunTable equality rather than by an oracle alias or hash.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamGlyphRegistryBootstrapEntry {
    pub canonical_alias: usize,
    pub glyph_id: i32,
    pub content: NativeStemsBeamFixedGlyphContent,
    pub active_in_index: bool,
    pub strongly_retained: bool,
}

pub const FIRST_STEMS_GLYPH_UNION_SIZE: usize = 1_650;
pub const FIRST_STEMS_LAST_PERSISTENT_ID: i32 = 2_339;
pub const FIRST_STEMS_VISIBLE_MODELED_COUNT: usize = 1_058;
pub const FIRST_STEMS_OPAQUE_GLYPH_COUNT: usize = 592;

/// Audit-only identity for a page glyph whose exact `RunTable` is not modeled.
/// A fingerprint never answers `Glyph.equals`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsFirstGlyphFingerprint {
    pub bounds: Bounds,
    pub run_table_sha256: String,
}

/// One identity in the disclosed first-STEMS Java `GlyphIndex` snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsFirstGlyphSnapshotEntry {
    pub glyph_id: i32,
    pub active_in_index: bool,
    pub live_original: bool,
    pub fingerprint: NativeStemsFirstGlyphFingerprint,
    /// Present only for the system-visible prefix of the native registry.
    pub modeled_canonical_ordinal: Option<usize>,
}

/// One-time page snapshot at chula system 1's first STEMS frontier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsFirstGlyphIndexSnapshot {
    pub system_id: usize,
    pub persistent_ids: NativeStemsBeamPersistentIdState,
    pub union_size: usize,
    pub active_count: usize,
    pub live_original_count: usize,
    pub active_sha256: String,
    pub live_original_sha256: String,
    pub visible_modeled_count: usize,
    pub entries: Vec<NativeStemsFirstGlyphSnapshotEntry>,
}

/// Validated candidate-local bridge from native modeled ordinals to Java IDs.
///
/// Opaque page-global entries remain represented in `snapshot`, but only the
/// exact modeled prefix is eligible for structural lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsFirstGlyphIndexBridge {
    snapshot: NativeStemsFirstGlyphIndexSnapshot,
    modeled: Vec<NativeStemsBeamKnownCanonicalGlyph>,
}

/// Native canonical-glyph identity available at one system's first STEMS
/// frontier.
///
/// Identity is the replay's zero-based canonical ordinal plus one. This is an
/// owned Rust domain derived from exact registration order and `Glyph.equals`
/// content; it does not import Java's sheet-wide `GlyphIndex`, its opaque
/// entries, or its allocator watermark.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsModeledGlyphRegistry {
    system_id: usize,
    persistent_ids: NativeStemsBeamPersistentIdState,
    entries: Vec<NativeStemsBeamGlyphRegistryBootstrapEntry>,
}

/// Exact-content lookup needed by head-origin C-link transactions.
pub trait NativeStemsGlyphRegistryAuthority {
    fn resolve_native_content(
        &self,
        content: &NativeStemsBeamFixedGlyphContent,
    ) -> Result<NativeStemsBeamGlyphRegistryBootstrapEntry, NativeStemsBeamVLinkTransactionError>;

    /// Prove the complete equality lookup needed before Java registers a
    /// newly composed glyph. Snapshot-only authorities deliberately refuse
    /// this negative proof; a production modeled registry overrides it.
    fn exhaustive_native_content_scan(
        &self,
        _content: &NativeStemsBeamFixedGlyphContent,
        _state: &NativeStemsBeamVLinkTransactionState,
    ) -> Result<NativeStemsBeamExhaustiveGlyphEqualsScan, NativeStemsBeamVLinkTransactionError>
    {
        Err(NativeStemsBeamVLinkTransactionError::AwaitingCompleteGlyphRegistry)
    }
}

impl NativeStemsModeledGlyphRegistry {
    fn completed_entries(
        &self,
        completed: &NativeStemsBeamVLinkTransactionState,
    ) -> Result<Vec<NativeStemsBeamGlyphRegistryBootstrapEntry>, NativeStemsBeamVLinkTransactionError>
    {
        validate_transaction_state(completed)?;
        if completed.system_stems.system_id != self.system_id
            || completed.glyph_index.alias_order
                != NativeStemsBeamGlyphAliasOrder::NativeModeledOrdinal
            || completed.glyph_index.persistent_ids.sheet_last_id
                < self.persistent_ids.sheet_last_id
        {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "completed modeled glyph registry header",
            });
        }
        let mut entries = self.entries.clone();
        for known in &completed.glyph_index.known_canonical_glyphs {
            if !known.strongly_retained {
                return Err(NativeStemsBeamVLinkTransactionError::AwaitingCompleteGlyphRegistry);
            }
            let identity = entries
                .iter()
                .position(|entry| entry.glyph_id == known.glyph_id);
            let content = entries
                .iter()
                .position(|entry| entry.content == known.content);
            match (identity, content) {
                (Some(identity), Some(content)) if identity == content => {
                    let entry = &mut entries[identity];
                    if entry.canonical_alias != known.canonical_alias {
                        return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                            phase: "completed modeled glyph registry alias",
                        });
                    }
                    entry.active_in_index = known.active_in_index;
                    entry.strongly_retained = true;
                }
                (None, None) => entries.push(NativeStemsBeamGlyphRegistryBootstrapEntry {
                    canonical_alias: known.canonical_alias,
                    glyph_id: known.glyph_id,
                    content: known.content.clone(),
                    active_in_index: known.active_in_index,
                    strongly_retained: true,
                }),
                _ => {
                    return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                        phase: "completed modeled glyph registry identity/content join",
                    });
                }
            }
        }
        if entries.len() != completed.glyph_index.union_size {
            return Err(NativeStemsBeamVLinkTransactionError::AwaitingCompleteGlyphRegistry);
        }
        Ok(entries)
    }

    /// Materialize the exact page registry after this system's final STEMS
    /// transaction. The result is in Java `GlyphIndex` entity order.
    pub fn completed_registry_entries(
        &self,
        completed: &NativeStemsBeamVLinkTransactionState,
    ) -> Result<Vec<NativeStemsBeamGlyphRegistryBootstrapEntry>, NativeStemsBeamVLinkTransactionError>
    {
        self.completed_entries(completed)
    }

    /// Apply Java REDUCTION's final `GlyphIndex.remove` sweep to the complete
    /// native-modeled registry. Persistent identities and the originals union
    /// remain stable; only active-index membership changes.
    pub fn retain_active_glyph_ids(
        &mut self,
        completed: &mut NativeStemsBeamVLinkTransactionState,
        keep: &BTreeSet<i32>,
    ) -> Result<(Vec<i32>, Vec<i32>), NativeStemsBeamVLinkTransactionError> {
        let mut entries = self.completed_entries(completed)?;
        if keep
            .iter()
            .any(|glyph_id| !entries.iter().any(|entry| entry.glyph_id == *glyph_id))
        {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "modeled glyph cleanup keep identity",
            });
        }
        let retained = entries
            .iter()
            .filter(|entry| entry.active_in_index && keep.contains(&entry.glyph_id))
            .map(|entry| entry.glyph_id)
            .collect::<Vec<_>>();
        let removed = entries
            .iter()
            .filter(|entry| entry.active_in_index && !keep.contains(&entry.glyph_id))
            .map(|entry| entry.glyph_id)
            .collect::<Vec<_>>();
        for entry in &mut entries {
            if entry.active_in_index && !keep.contains(&entry.glyph_id) {
                entry.active_in_index = false;
            }
        }
        for known in &mut completed.glyph_index.known_canonical_glyphs {
            if removed.contains(&known.glyph_id) {
                known.active_in_index = false;
            }
        }
        completed.glyph_index.exhaustive_lookup = None;
        self.persistent_ids = completed.glyph_index.persistent_ids;
        self.entries = entries;
        Ok((retained, removed))
    }

    /// Derive one system's visible canonical prefix from the production
    /// head-builder registry chronology.
    pub fn from_head_builder_recognition(
        system_id: usize,
        recognition: &NativeStemsHeadBuilderRecognition,
    ) -> Result<Self, NativeStemsBeamVLinkTransactionError> {
        let system = recognition
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .ok_or(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "native modeled glyph registry system",
            })?;
        let visible_modeled_count = system
            .registry_events
            .last()
            .map(|event| event.modeled_count_after)
            .ok_or(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "native modeled glyph registry system boundary",
            })?;
        Self::from_modeled_prefix(
            system_id,
            &recognition.modeled_canonical_glyphs,
            visible_modeled_count,
        )
    }

    pub fn from_modeled_prefix(
        system_id: usize,
        modeled_registry: &[NativeStemsModeledCanonicalGlyph],
        visible_modeled_count: usize,
    ) -> Result<Self, NativeStemsBeamVLinkTransactionError> {
        if system_id == 0
            || visible_modeled_count == 0
            || visible_modeled_count > modeled_registry.len()
        {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "native modeled glyph registry header",
            });
        }
        let mut entries = Vec::with_capacity(visible_modeled_count);
        for (ordinal, glyph) in modeled_registry[..visible_modeled_count].iter().enumerate() {
            let content = NativeStemsBeamFixedGlyphContent {
                bounds: glyph.bounds,
                weight: glyph.weight,
                run_table: glyph.run_table.clone(),
            };
            validate_content(&content)?;
            if glyph.modeled_canonical_ordinal != ordinal
                || entries
                    .iter()
                    .any(|entry: &NativeStemsBeamGlyphRegistryBootstrapEntry| {
                        entry.content == content
                    })
            {
                return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                    phase: "native modeled glyph registry entries",
                });
            }
            let glyph_id = i32::try_from(ordinal)
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or(NativeStemsBeamVLinkTransactionError::PersistentIdExhausted)?;
            entries.push(NativeStemsBeamGlyphRegistryBootstrapEntry {
                canonical_alias: ordinal + 1,
                glyph_id,
                content,
                active_in_index: true,
                strongly_retained: true,
            });
        }
        let last_id = i32::try_from(entries.len())
            .map_err(|_| NativeStemsBeamVLinkTransactionError::PersistentIdExhausted)?;
        Ok(Self {
            system_id,
            persistent_ids: NativeStemsBeamPersistentIdState {
                sheet_last_id: last_id,
                glyph_index_last_id: last_id,
                inter_index_last_id: last_id,
            },
            entries,
        })
    }

    #[must_use]
    pub fn system_id(&self) -> usize {
        self.system_id
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn persistent_ids(&self) -> NativeStemsBeamPersistentIdState {
        self.persistent_ids
    }

    /// Carry the exact page registry through one completed system and replay
    /// the next system's constructor registrations in their production order.
    ///
    /// The completed transaction state supplies every native registration
    /// since this registry prefix. Weak-only entries are deliberately refused:
    /// Java may have collected them before the next system, so their equality
    /// presence cannot be inferred without a liveness authority.
    pub fn carry_into_next_system(
        &self,
        completed: &NativeStemsBeamVLinkTransactionState,
        recognition: &NativeStemsHeadBuilderRecognition,
    ) -> Result<Self, NativeStemsBeamVLinkTransactionError> {
        validate_transaction_state(completed)?;
        let next_system_id = self
            .system_id
            .checked_add(1)
            .ok_or(NativeStemsBeamVLinkTransactionError::SystemOrder)?;
        let scope_system = match completed.scope {
            NativeStemsBeamVLinkTransactionScope::SharedSheetFirstFrontier { system_id }
            | NativeStemsBeamVLinkTransactionScope::SharedSheetSerial { system_id } => system_id,
            NativeStemsBeamVLinkTransactionScope::IsolatedFreshSheetFrontier { .. } => {
                return Err(NativeStemsBeamVLinkTransactionError::SystemOrder);
            }
        };
        if scope_system != self.system_id
            || completed.system_stems.system_id != self.system_id
            || completed.glyph_index.alias_order
                != NativeStemsBeamGlyphAliasOrder::NativeModeledOrdinal
            || completed.glyph_index.persistent_ids.sheet_last_id
                < self.persistent_ids.sheet_last_id
            || self.entries.len() > completed.glyph_index.union_size
        {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "cross-system carried registry header",
            });
        }
        let next_system = recognition
            .systems
            .iter()
            .find(|system| system.system_id == next_system_id)
            .ok_or(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "cross-system next head-builder system",
            })?;

        let mut entries = self.entries.clone();
        for known in &completed.glyph_index.known_canonical_glyphs {
            if !known.strongly_retained {
                return Err(NativeStemsBeamVLinkTransactionError::AwaitingCompleteGlyphRegistry);
            }
            let identity_match = entries
                .iter()
                .position(|entry| entry.glyph_id == known.glyph_id);
            let content_match = entries
                .iter()
                .position(|entry| entry.content == known.content);
            match (identity_match, content_match) {
                (Some(identity), Some(content)) if identity == content => {
                    let entry = &mut entries[identity];
                    if entry.canonical_alias != known.canonical_alias {
                        return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                            phase: "cross-system known identity alias",
                        });
                    }
                    entry.active_in_index = known.active_in_index;
                    entry.strongly_retained = true;
                }
                (None, None) => entries.push(NativeStemsBeamGlyphRegistryBootstrapEntry {
                    canonical_alias: known.canonical_alias,
                    glyph_id: known.glyph_id,
                    content: known.content.clone(),
                    active_in_index: known.active_in_index,
                    strongly_retained: true,
                }),
                _ => {
                    return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                        phase: "cross-system known identity/content join",
                    });
                }
            }
        }
        if entries.len() != completed.glyph_index.union_size {
            return Err(NativeStemsBeamVLinkTransactionError::AwaitingCompleteGlyphRegistry);
        }

        let mut persistent_ids = completed.glyph_index.persistent_ids;
        for (event_ordinal, event) in next_system.registry_events.iter().enumerate() {
            if event.system_event_ordinal != event_ordinal {
                return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                    phase: "cross-system registry event order",
                });
            }
            let content = NativeStemsBeamFixedGlyphContent {
                bounds: event.bounds,
                weight: event.weight,
                run_table: event.run_table.clone(),
            };
            validate_content(&content)?;
            let matches = entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.content == content)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [index] => {
                    entries[*index].active_in_index = true;
                    entries[*index].strongly_retained = true;
                }
                [] => {
                    let next_id = persistent_ids
                        .sheet_last_id
                        .checked_add(1)
                        .ok_or(NativeStemsBeamVLinkTransactionError::PersistentIdExhausted)?;
                    persistent_ids = NativeStemsBeamPersistentIdState {
                        sheet_last_id: next_id,
                        glyph_index_last_id: next_id,
                        inter_index_last_id: next_id,
                    };
                    entries.push(NativeStemsBeamGlyphRegistryBootstrapEntry {
                        canonical_alias: usize::try_from(next_id).map_err(|_| {
                            NativeStemsBeamVLinkTransactionError::PersistentIdExhausted
                        })?,
                        glyph_id: next_id,
                        content,
                        active_in_index: true,
                        strongly_retained: true,
                    });
                }
                _ => {
                    return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                        phase: "cross-system registry structural equality",
                    });
                }
            }
        }
        Ok(Self {
            system_id: next_system_id,
            persistent_ids,
            entries,
        })
    }

    /// Prove the page-wide structural lookup for a compound candidate from
    /// the carried registry plus every strongly retained transaction addition.
    pub fn exhaustive_scan(
        &self,
        candidate: &NativeStemsBeamFixedGlyphContent,
        state: &NativeStemsBeamVLinkTransactionState,
    ) -> Result<NativeStemsBeamExhaustiveGlyphEqualsScan, NativeStemsBeamVLinkTransactionError>
    {
        validate_content(candidate)?;
        validate_transaction_state(state)?;
        if state.system_stems.system_id != self.system_id
            || state.glyph_index.alias_order != NativeStemsBeamGlyphAliasOrder::NativeModeledOrdinal
            || state.glyph_index.persistent_ids.sheet_last_id < self.persistent_ids.sheet_last_id
        {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "native modeled exhaustive scan header",
            });
        }
        let mut entries = self.entries.clone();
        for known in &state.glyph_index.known_canonical_glyphs {
            if !known.strongly_retained {
                return Err(NativeStemsBeamVLinkTransactionError::AwaitingCompleteGlyphRegistry);
            }
            let identity_match = entries
                .iter()
                .position(|entry| entry.glyph_id == known.glyph_id);
            let content_match = entries
                .iter()
                .position(|entry| entry.content == known.content);
            match (identity_match, content_match) {
                (Some(identity), Some(content)) if identity == content => {
                    let entry = &mut entries[identity];
                    if entry.canonical_alias != known.canonical_alias {
                        return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                            phase: "native modeled exhaustive known alias",
                        });
                    }
                    entry.active_in_index = known.active_in_index;
                    entry.strongly_retained = true;
                }
                (None, None) => entries.push(NativeStemsBeamGlyphRegistryBootstrapEntry {
                    canonical_alias: known.canonical_alias,
                    glyph_id: known.glyph_id,
                    content: known.content.clone(),
                    active_in_index: known.active_in_index,
                    strongly_retained: true,
                }),
                _ => {
                    return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                        phase: "native modeled exhaustive identity/content join",
                    });
                }
            }
        }
        if entries.len() != state.glyph_index.union_size {
            return Err(NativeStemsBeamVLinkTransactionError::AwaitingCompleteGlyphRegistry);
        }
        let matches = entries
            .iter()
            .filter(|entry| entry.content == *candidate)
            .collect::<Vec<_>>();
        let lookup = match matches.as_slice() {
            [] => NativeStemsBeamExhaustiveGlyphLookup::Absent,
            [entry] => NativeStemsBeamExhaustiveGlyphLookup::Present {
                canonical_alias: entry.canonical_alias,
                glyph_id: entry.glyph_id,
                active_in_index: entry.active_in_index,
            },
            _ => {
                return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                    phase: "native modeled exhaustive equality cardinality",
                });
            }
        };
        let active_count = entries.iter().filter(|entry| entry.active_in_index).count();
        let live_original_count = entries
            .iter()
            .filter(|entry| entry.strongly_retained)
            .count();
        Ok(NativeStemsBeamExhaustiveGlyphEqualsScan {
            candidate: candidate.clone(),
            alias_order: state.glyph_index.alias_order,
            baseline_union_size: entries.len(),
            baseline_active_count: active_count,
            baseline_live_original_count: live_original_count,
            baseline_active_sha256: modeled_registry_sha256(&entries, false),
            baseline_live_original_sha256: modeled_registry_sha256(&entries, true),
            scanned_active_count: active_count,
            scanned_live_original_count: live_original_count,
            equal_active_matches: usize::from(
                matches.first().is_some_and(|entry| entry.active_in_index),
            ),
            equal_original_matches: matches.len(),
            lookup,
        })
    }

    fn selected_bootstrap(
        &self,
        selected: &NativeStemsBeamSelectedGlyph,
    ) -> Result<NativeStemsBeamGlyphRegistryBootstrapEntry, NativeStemsBeamVLinkTransactionError>
    {
        let content = NativeStemsBeamFixedGlyphContent {
            bounds: selected.bounds,
            weight: selected.weight,
            run_table: selected.structural_key.run_table.clone(),
        };
        let matches = self
            .entries
            .iter()
            .filter(|entry| entry.content == content)
            .collect::<Vec<_>>();
        let [entry] = matches.as_slice() else {
            return Err(NativeStemsBeamVLinkTransactionError::AwaitingCompleteGlyphRegistry);
        };
        Ok((*entry).clone())
    }
}

impl NativeStemsGlyphRegistryAuthority for NativeStemsModeledGlyphRegistry {
    fn resolve_native_content(
        &self,
        content: &NativeStemsBeamFixedGlyphContent,
    ) -> Result<NativeStemsBeamGlyphRegistryBootstrapEntry, NativeStemsBeamVLinkTransactionError>
    {
        validate_content(content)?;
        let matches = self
            .entries
            .iter()
            .filter(|entry| entry.content == *content)
            .collect::<Vec<_>>();
        let [entry] = matches.as_slice() else {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "native modeled glyph content",
            });
        };
        Ok((*entry).clone())
    }

    fn exhaustive_native_content_scan(
        &self,
        content: &NativeStemsBeamFixedGlyphContent,
        state: &NativeStemsBeamVLinkTransactionState,
    ) -> Result<NativeStemsBeamExhaustiveGlyphEqualsScan, NativeStemsBeamVLinkTransactionError>
    {
        self.exhaustive_scan(content, state)
    }
}

impl NativeStemsFirstGlyphIndexBridge {
    pub fn from_snapshot(
        snapshot: NativeStemsFirstGlyphIndexSnapshot,
        modeled_registry: &[NativeStemsModeledCanonicalGlyph],
    ) -> Result<Self, NativeStemsBeamVLinkTransactionError> {
        let ids = snapshot.persistent_ids;
        let active_sha256 = first_stems_snapshot_sha256(&snapshot.entries, false);
        let live_original_sha256 = first_stems_snapshot_sha256(&snapshot.entries, true);
        if snapshot.system_id != 1
            || snapshot.union_size != FIRST_STEMS_GLYPH_UNION_SIZE
            || snapshot.entries.len() != snapshot.union_size
            || snapshot.active_count != snapshot.union_size
            || snapshot.live_original_count != snapshot.union_size
            || ids.sheet_last_id != FIRST_STEMS_LAST_PERSISTENT_ID
            || ids.glyph_index_last_id != ids.sheet_last_id
            || ids.inter_index_last_id != ids.sheet_last_id
            || snapshot.visible_modeled_count != FIRST_STEMS_VISIBLE_MODELED_COUNT
            || snapshot.visible_modeled_count > modeled_registry.len()
            || !valid_sha256(&snapshot.active_sha256)
            || !valid_sha256(&snapshot.live_original_sha256)
            || snapshot.active_sha256 != active_sha256
            || snapshot.live_original_sha256 != live_original_sha256
            || !snapshot
                .entries
                .iter()
                .any(|entry| entry.glyph_id == FIRST_STEMS_LAST_PERSISTENT_ID)
        {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "first-STEMS glyph snapshot header",
            });
        }
        let mut modeled: Vec<Option<NativeStemsBeamKnownCanonicalGlyph>> =
            vec![None; snapshot.visible_modeled_count];
        for (index, entry) in snapshot.entries.iter().enumerate() {
            if entry.glyph_id <= 0
                || entry.glyph_id > ids.sheet_last_id
                || !valid_sha256(&entry.fingerprint.run_table_sha256)
                || entry.fingerprint.bounds.width == 0
                || entry.fingerprint.bounds.height == 0
                || snapshot.entries[..index].iter().any(|prior| {
                    prior.glyph_id == entry.glyph_id || prior.fingerprint == entry.fingerprint
                })
            {
                return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                    phase: "first-STEMS glyph snapshot entries",
                });
            }
            if let Some(ordinal) = entry.modeled_canonical_ordinal {
                let Some(native) = modeled_registry.get(ordinal) else {
                    return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                        phase: "first-STEMS modeled cutoff",
                    });
                };
                if ordinal >= snapshot.visible_modeled_count
                    || native.modeled_canonical_ordinal != ordinal
                    || native.bounds != entry.fingerprint.bounds
                    || run_table_sha256(&native.run_table) != entry.fingerprint.run_table_sha256
                    || modeled[ordinal].is_some()
                {
                    return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                        phase: "first-STEMS modeled mapping",
                    });
                }
                modeled[ordinal] = Some(NativeStemsBeamKnownCanonicalGlyph {
                    canonical_alias: entry.glyph_id as usize,
                    glyph_id: entry.glyph_id,
                    content: NativeStemsBeamFixedGlyphContent {
                        bounds: native.bounds,
                        weight: native.weight,
                        run_table: native.run_table.clone(),
                    },
                    active_in_index: entry.active_in_index,
                    strongly_retained: entry.active_in_index,
                });
            }
        }
        if snapshot
            .entries
            .iter()
            .filter(|entry| entry.active_in_index)
            .count()
            != snapshot.active_count
            || snapshot
                .entries
                .iter()
                .filter(|entry| entry.live_original)
                .count()
                != snapshot.live_original_count
            || modeled.iter().any(Option::is_none)
            || snapshot
                .entries
                .iter()
                .filter(|entry| entry.modeled_canonical_ordinal.is_none())
                .count()
                != FIRST_STEMS_OPAQUE_GLYPH_COUNT
        {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "first-STEMS glyph snapshot coverage",
            });
        }
        Ok(Self {
            snapshot,
            modeled: modeled.into_iter().flatten().collect(),
        })
    }

    #[must_use]
    pub fn snapshot(&self) -> &NativeStemsFirstGlyphIndexSnapshot {
        &self.snapshot
    }

    /// Resolve one exact native RunTable against a positive first-STEMS
    /// fingerprint. This does not make opaque entries absence authority: only
    /// a unique digest-and-bounds hit can promote the supplied content.
    pub fn resolve_native_content(
        &self,
        content: &NativeStemsBeamFixedGlyphContent,
    ) -> Result<NativeStemsBeamGlyphRegistryBootstrapEntry, NativeStemsBeamVLinkTransactionError>
    {
        validate_content(content)?;
        let digest = run_table_sha256(&content.run_table);
        let matches = self
            .snapshot
            .entries
            .iter()
            .filter(|entry| {
                entry.fingerprint.bounds == content.bounds
                    && entry.fingerprint.run_table_sha256 == digest
            })
            .collect::<Vec<_>>();
        let [entry] = matches.as_slice() else {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "native-content first-STEMS fingerprint",
            });
        };
        Ok(NativeStemsBeamGlyphRegistryBootstrapEntry {
            canonical_alias: usize::try_from(entry.glyph_id)
                .map_err(|_| NativeStemsBeamVLinkTransactionError::PersistentIdExhausted)?,
            glyph_id: entry.glyph_id,
            content: content.clone(),
            active_in_index: entry.active_in_index,
            strongly_retained: entry.live_original,
        })
    }

    fn selected_bootstrap(
        &self,
        selected: &NativeStemsBeamSelectedGlyph,
    ) -> Result<NativeStemsBeamGlyphRegistryBootstrapEntry, NativeStemsBeamVLinkTransactionError>
    {
        let known = self
            .modeled
            .get(selected.modeled_canonical_ordinal)
            .ok_or(NativeStemsBeamVLinkTransactionError::AwaitingCompleteGlyphRegistry)?;
        let content = NativeStemsBeamFixedGlyphContent {
            bounds: selected.bounds,
            weight: selected.weight,
            run_table: selected.structural_key.run_table.clone(),
        };
        if known.content != content {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "first-STEMS selected modeled identity",
            });
        }
        Ok(NativeStemsBeamGlyphRegistryBootstrapEntry {
            canonical_alias: known.canonical_alias,
            glyph_id: known.glyph_id,
            content,
            active_in_index: known.active_in_index,
            strongly_retained: known.strongly_retained,
        })
    }
}

impl NativeStemsGlyphRegistryAuthority for NativeStemsFirstGlyphIndexBridge {
    fn resolve_native_content(
        &self,
        content: &NativeStemsBeamFixedGlyphContent,
    ) -> Result<NativeStemsBeamGlyphRegistryBootstrapEntry, NativeStemsBeamVLinkTransactionError>
    {
        NativeStemsFirstGlyphIndexBridge::resolve_native_content(self, content)
    }
}

/// Explicit authority for promoting `systemStems` to a complete carried map.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamSystemStemAuthorityProof {
    system_id: usize,
    carried_stem_count: usize,
}

impl NativeStemsBeamSystemStemAuthorityProof {
    /// Establish authority from the disclosed STEMS-entry fact that the map
    /// was empty, joined to a dense native insertion history in `state`.
    pub fn from_empty_stems_entry(
        state: &NativeStemsBeamSystemStemTransactionState,
        baseline_entry_count: usize,
    ) -> Result<Self, NativeStemsBeamVLinkTransactionError> {
        if baseline_entry_count != 0
            || state
                .known_stems
                .iter()
                .enumerate()
                .any(|(identity, stem)| stem.stem_identity != identity)
            || state.next_stem_identity != state.known_stems.len()
        {
            return Err(NativeStemsBeamVLinkTransactionError::SystemStemInvariant {
                phase: "complete-since-empty authority proof",
            });
        }
        Ok(Self {
            system_id: state.system_id,
            carried_stem_count: state.known_stems.len(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamFrontierPreparation {
    pub plan: NativeStemsBeamPlanRef,
    pub v_linker: NativeStemsBeamVLinkerRef,
    pub selected_glyphs: Vec<NativeStemsBeamBuilderGlyphRef>,
    pub known_glyphs_added: usize,
    pub line_state_added: bool,
}

/// Result of exhaustively comparing one exact candidate against every live
/// entry in Java's `GlyphIndex.originals` map.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamExhaustiveGlyphLookup {
    Absent,
    Present {
        canonical_alias: usize,
        glyph_id: i32,
        active_in_index: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamGlyphAliasOrder {
    JavaGlyphId,
    /// Owned identity assigned by the native modeled-canonical registration
    /// order. The zero-based modeled ordinal is represented as ordinal + 1.
    NativeModeledOrdinal,
}

/// Candidate-specific proof for the only sheet-global equality query made by
/// this transaction. Hash strings guard fixture joins; they are never used to
/// decide equality or canonical identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamExhaustiveGlyphEqualsScan {
    pub candidate: NativeStemsBeamFixedGlyphContent,
    pub alias_order: NativeStemsBeamGlyphAliasOrder,
    pub baseline_union_size: usize,
    pub baseline_active_count: usize,
    pub baseline_live_original_count: usize,
    pub baseline_active_sha256: String,
    pub baseline_live_original_sha256: String,
    pub scanned_active_count: usize,
    pub scanned_live_original_count: usize,
    pub equal_active_matches: usize,
    pub equal_original_matches: usize,
    pub lookup: NativeStemsBeamExhaustiveGlyphLookup,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamGlyphIndexTransactionState {
    pub persistent_ids: NativeStemsBeamPersistentIdState,
    pub alias_order: NativeStemsBeamGlyphAliasOrder,
    pub union_size: usize,
    /// Exact entries learned by earlier transactions or the current scan.
    pub known_canonical_glyphs: Vec<NativeStemsBeamKnownCanonicalGlyph>,
    /// One-shot evidence for a candidate not already known exactly.
    pub exhaustive_lookup: Option<NativeStemsBeamExhaustiveGlyphEqualsScan>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkLineState {
    pub v_linker: NativeStemsBeamVLinkerRef,
    pub stored_theoretical_line: NativeStemLine,
    pub builder_line: NativeStemLine,
    pub current_attachment_line: Option<NativeStemLine>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamAppliedLineDeltaSource {
    KnownFalsePrefix { delta_ordinal: usize },
    ReadyTransaction,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamAppliedLineDelta {
    pub system_id: usize,
    pub source: NativeStemsBeamAppliedLineDeltaSource,
    pub delta: NativeStemsBeamDeferredLineDelta,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamStemGrade {
    Checked(NativeStemCheckResult),
    Artificial(f64),
}

impl NativeStemsBeamStemGrade {
    #[must_use]
    pub fn grade(&self) -> f64 {
        match self {
            Self::Checked(check) => check.grade,
            Self::Artificial(grade) => *grade,
        }
    }
}

/// State constructed immediately by either `StemInter` constructor, before
/// `SIGraph.addVertex`: ID zero, no SIG, and not yet abnormal.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamCreatedStemGeometry {
    pub median: NativeStemLine,
    pub mean_thickness: f64,
    pub ribbon_bounds: JavaRectangle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamKnownSystemStem {
    /// Dense native object identity, independent of Java's still-zero Inter ID.
    pub stem_identity: usize,
    pub glyph_id: i32,
    pub glyph_content: NativeStemsBeamFixedGlyphContent,
    pub inter_id: Option<i32>,
    pub grade: NativeStemsBeamStemGrade,
    pub geometry: NativeStemsBeamCreatedStemGeometry,
    pub sig_attached: bool,
    pub abnormal: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamExhaustiveSystemStemLookup {
    Absent,
    Present(Box<NativeStemsBeamKnownSystemStem>),
}

/// Candidate-specific proof for `StemsRetriever.systemStems.get(stemGlyph)`.
/// The Java map is keyed by structural `Glyph.equals`, not object identity.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamExhaustiveSystemStemEqualsScan {
    pub candidate: NativeStemsBeamFixedGlyphContent,
    pub baseline_entry_count: usize,
    pub baseline_sha256: String,
    pub scanned_entry_count: usize,
    pub equal_glyph_matches: usize,
    pub lookup: NativeStemsBeamExhaustiveSystemStemLookup,
}

/// Whether a registry's absences are proofs.
///
/// Both the glyph and system-stem registries answer "does this already exist?"
/// by searching carried entries for matching content, and fall back to a Java
/// exhaustive scan when nothing matches -- because a miss in a partial registry
/// says nothing. That fallback is what stops a chained SIDES pass from running
/// on its own state: every transaction after the first would need its own scan
/// evidence.
///
/// A registry that has carried every entry since an empty baseline is different
/// in kind: a miss there *is* absence. `systemStems` qualifies, because STEMS
/// begins with it empty -- the frozen create-stem baseline records
/// `systemStems 0` -- and only these transactions insert into it.
///
/// The distinction is deliberately explicit rather than inferred from
/// `known_stems.is_empty()` or a count, because claiming it wrongly does not
/// fail: it silently registers a second stem for a glyph that already had one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamRegistryAuthority {
    /// Every entry since an empty baseline is carried, so a content miss proves
    /// absence and no exhaustive scan is required.
    CompleteSinceEmptyBaseline,
    /// The registry may be missing entries; only an exhaustive scan decides.
    RequiresExhaustiveScan,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSystemStemTransactionState {
    pub system_id: usize,
    pub next_stem_identity: usize,
    pub known_stems: Vec<NativeStemsBeamKnownSystemStem>,
    /// Defaults to `RequiresExhaustiveScan` everywhere the state is built from
    /// Java rows, so hydrated paths keep their existing evidence requirement.
    pub authority: NativeStemsBeamRegistryAuthority,
    pub exhaustive_lookup: Option<NativeStemsBeamExhaustiveSystemStemEqualsScan>,
}

/// Mutable Java state needed by precisely one `createStem` transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkTransactionState {
    pub scope: NativeStemsBeamVLinkTransactionScope,
    pub glyph_index: NativeStemsBeamGlyphIndexTransactionState,
    pub selected_glyph_bindings: Vec<NativeStemsBeamSelectedGlyphBinding>,
    pub line_states: Vec<NativeStemsBeamVLinkLineState>,
    pub applied_line_deltas: Vec<NativeStemsBeamAppliedLineDelta>,
    pub system_stems: NativeStemsBeamSystemStemTransactionState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamStemCheckerContext {
    pub no_staff: RunTable,
    pub parameters: NativeStemCheckerParameters,
    pub minimum_stem_grade: f64,
    pub artificial_stem_grade: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamGlyphRegistrationAction {
    Reused { reinserted_into_active_index: bool },
    Registered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamGlyphRegistration {
    pub alias_order: NativeStemsBeamGlyphAliasOrder,
    pub canonical_alias: usize,
    pub glyph_id: i32,
    pub action: NativeStemsBeamGlyphRegistrationAction,
    pub post_union_size: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamCreateStemDisposition {
    Reused { stem_identity: usize },
    CreatedChecked { stem_identity: usize },
    CreatedArtificial { stem_identity: usize },
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamVLinkMutation {
    StoredLineDeltaApplied,
    GlyphReinserted { glyph_id: i32 },
    GlyphRegistered { glyph_id: i32 },
    SystemStemInserted { stem_identity: usize },
    RegistryWeakLivenessBecameUnknown { glyph_id: i32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkTransaction {
    pub system_id: usize,
    pub invocation_ordinal: usize,
    pub plan: NativeStemsBeamPlanRef,
    pub scope: NativeStemsBeamVLinkTransactionScope,
    pub candidate: NativeStemsBeamFixedGlyphContent,
    /// Java ID on the object passed to `registerOriginal`: present for the
    /// existing singleton, absent (Java zero) for a newly built compound.
    pub candidate_glyph_id_before_registration: Option<i32>,
    /// Earlier known-false scheduler attempts whose aliased line writes were
    /// still pending in the supplied state and were committed first.
    pub applied_prefix_line_deltas: Vec<NativeStemsBeamDeferredLineDelta>,
    pub applied_line_delta: Option<NativeStemsBeamDeferredLineDelta>,
    pub registration: NativeStemsBeamGlyphRegistration,
    pub checker_result: Option<NativeStemCheckResult>,
    pub disposition: NativeStemsBeamCreateStemDisposition,
    pub stem: Option<NativeStemsBeamKnownSystemStem>,
    pub mutation_order: Vec<NativeStemsBeamVLinkMutation>,
    /// This boundary ends before all of these Java operations.
    pub sig_vertex_mutation_count: usize,
    pub sig_relation_mutation_count: usize,
    pub linker_flag_mutation_count: usize,
}

/// Origin-neutral `StemBuilder.createStem` result for an already materialized
/// glyph candidate. Beam and head linkers share this Java operation, while
/// their expansion and graph-link phases remain deliberately separate.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsCreateStemCandidateTransaction {
    /// Whether Java actually invoked `StemBuilder.createStem`.
    /// Append-mode head linking first tries `reuseStem(lastIndex)` and can
    /// bypass registration and checking entirely.
    pub invoked: bool,
    pub system_id: usize,
    pub stem_profile: i32,
    pub candidate: NativeStemsBeamFixedGlyphContent,
    pub registration: NativeStemsBeamGlyphRegistration,
    pub checker_result: Option<NativeStemCheckResult>,
    pub disposition: NativeStemsBeamCreateStemDisposition,
    pub stem: Option<NativeStemsBeamKnownSystemStem>,
    pub mutation_order: Vec<NativeStemsBeamVLinkMutation>,
}

/// Apply Java's origin-neutral `StemBuilder.createStem` transaction.
///
/// The caller owns expansion and supplies the exact selected glyph content.
/// Registration, complete-since-empty `systemStems` lookup, stem checking and
/// insertion use the same kernels as the beam-origin transaction.
pub fn apply_native_stems_create_stem_candidate_transaction(
    system_id: usize,
    stem_profile: i32,
    candidate: NativeStemsBeamFixedGlyphContent,
    selected_canonical_identity: Option<(i32, usize)>,
    state: &mut NativeStemsBeamVLinkTransactionState,
    checker: &NativeStemsBeamStemCheckerContext,
) -> Result<NativeStemsCreateStemCandidateTransaction, NativeStemsBeamVLinkTransactionError> {
    if state.system_stems.system_id != system_id || !(0..=4).contains(&stem_profile) {
        return Err(NativeStemsBeamVLinkTransactionError::SystemOrder);
    }
    validate_checker_context(checker)?;
    validate_transaction_state(state)?;
    validate_content(&candidate)?;
    let registration = prepare_registration(&candidate, selected_canonical_identity, state)?;
    let stem_lookup =
        prepare_system_stem_lookup(&candidate, registration.glyph_id, &state.system_stems)?;
    let prepared = match stem_lookup.stem.as_ref() {
        Some(stem) => PreparedCreateStemResult {
            checker_result: None,
            disposition: NativeStemsBeamCreateStemDisposition::Reused {
                stem_identity: stem.stem_identity,
            },
            stem: Some(stem.clone()),
            next_stem_identity: state.system_stems.next_stem_identity,
        },
        None => prepare_new_stem(
            &candidate,
            registration.glyph_id,
            stem_profile,
            state.system_stems.next_stem_identity,
            checker,
        )?,
    };

    let mut mutation_order = Vec::new();
    commit_registration(
        state,
        &candidate,
        &registration,
        &prepared.disposition,
        &mut mutation_order,
    );
    if stem_lookup.consume_certificate {
        state.system_stems.exhaustive_lookup = None;
    }
    if let Some(stem) = stem_lookup.stem
        && !state
            .system_stems
            .known_stems
            .iter()
            .any(|known| known.stem_identity == stem.stem_identity)
    {
        state.system_stems.known_stems.push(stem);
    }
    if matches!(
        prepared.disposition,
        NativeStemsBeamCreateStemDisposition::CreatedChecked { .. }
            | NativeStemsBeamCreateStemDisposition::CreatedArtificial { .. }
    ) {
        let stem = prepared
            .stem
            .clone()
            .expect("created disposition was prepared with a stem");
        state.system_stems.next_stem_identity = prepared.next_stem_identity;
        state.system_stems.known_stems.push(stem.clone());
        mutation_order.push(NativeStemsBeamVLinkMutation::SystemStemInserted {
            stem_identity: stem.stem_identity,
        });
    }

    Ok(NativeStemsCreateStemCandidateTransaction {
        invoked: true,
        system_id,
        stem_profile,
        candidate,
        registration: NativeStemsBeamGlyphRegistration {
            alias_order: state.glyph_index.alias_order,
            canonical_alias: registration.canonical_alias,
            glyph_id: registration.glyph_id,
            action: registration.action,
            post_union_size: registration.post_union_size,
        },
        checker_result: prepared.checker_result,
        disposition: prepared.disposition,
        stem: prepared.stem,
        mutation_order,
    })
}

#[derive(Debug)]
pub enum NativeStemsBeamVLinkTransactionError {
    AwaitingCompleteGlyphRegistry,
    AwaitingCompleteSystemStems,
    SystemOrder,
    NoAwaitingVLinkTransaction {
        system_id: usize,
    },
    InvalidProduct {
        system_id: usize,
        phase: &'static str,
    },
    PersistentAllocatorMismatch,
    PersistentIdExhausted,
    RegistryInvariant {
        phase: &'static str,
    },
    MissingSelectedGlyph {
        reference: NativeStemsBeamBuilderGlyphRef,
    },
    MissingLineState {
        reference: NativeStemsBeamVLinkerRef,
    },
    LineStateMismatch {
        reference: NativeStemsBeamVLinkerRef,
    },
    MissingCommittedLineDelta {
        delta_ordinal: usize,
    },
    DuplicateLineDelta,
    SystemStemInvariant {
        phase: &'static str,
    },
    StemIdentityExhausted,
    Geometry,
    RunTable(RunTableError),
    StemChecker(NativeStemGeometryError),
}

impl fmt::Display for NativeStemsBeamVLinkTransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid beam VLink createStem transaction: {self:?}"
        )
    }
}

impl Error for NativeStemsBeamVLinkTransactionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RunTable(source) => Some(source),
            Self::StemChecker(source) => Some(source),
            _ => None,
        }
    }
}

impl From<RunTableError> for NativeStemsBeamVLinkTransactionError {
    fn from(source: RunTableError) -> Self {
        Self::RunTable(source)
    }
}

/// Prepare one later shared-sheet frontier from carried transaction state.
///
/// This is the production form of the state transition previously assembled
/// by the transaction-2 integration test. It derives the V line state and
/// selected glyph references from the scheduler/plan products, joins selected
/// glyphs to a disclosed page bootstrap by exact content, and promotes the
/// `systemStems` map only with an explicit completeness proof. The update is
/// clone-and-swap: any missing/ambiguous bootstrap entry or invariant failure
/// leaves `state` byte-for-byte unchanged.
pub fn prepare_native_stems_beam_vlink_frontier_state(
    scheduler_system: &NativeStemsBeamSchedulerSystem,
    plan_system: &NativeStemsBeamLinkPlanSystem,
    state: &mut NativeStemsBeamVLinkTransactionState,
    glyph_bootstrap: &[NativeStemsBeamGlyphRegistryBootstrapEntry],
    system_stems_proof: NativeStemsBeamSystemStemAuthorityProof,
) -> Result<NativeStemsBeamFrontierPreparation, NativeStemsBeamVLinkTransactionError> {
    if scheduler_system.system_id != plan_system.system_id
        || state.system_stems.system_id != scheduler_system.system_id
    {
        return Err(NativeStemsBeamVLinkTransactionError::SystemOrder);
    }
    validate_transaction_state(state)?;
    if system_stems_proof.system_id != state.system_stems.system_id
        || system_stems_proof.carried_stem_count != state.system_stems.known_stems.len()
    {
        return Err(NativeStemsBeamVLinkTransactionError::SystemStemInvariant {
            phase: "stale complete-since-empty authority proof",
        });
    }
    let frontier = match &scheduler_system.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => {
            return Err(
                NativeStemsBeamVLinkTransactionError::NoAwaitingVLinkTransaction {
                    system_id: scheduler_system.system_id,
                },
            );
        }
    };
    let (attempt, start_v) = resolve_plan(plan_system, frontier.plan)?;
    if frontier.v_linker != start_v
        || frontier.outcome != NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem
    {
        return Err(NativeStemsBeamVLinkTransactionError::InvalidProduct {
            system_id: scheduler_system.system_id,
            phase: "serial frontier plan/V join",
        });
    }
    let mut shadow = state.clone();
    shadow.scope = NativeStemsBeamVLinkTransactionScope::SharedSheetSerial {
        system_id: scheduler_system.system_id,
    };
    // `ReadyTransaction` entries authenticate B13 in the transaction which
    // applied them. The mutated line state persists, but that one-shot ledger
    // row must not be mistaken for the next scheduler frontier's cumulative
    // known-false prefix. Java can reach this shape when a later SIDES
    // transaction follows an earlier successful V-linker line shift.
    shadow.applied_line_deltas.retain(|applied| {
        applied.source != NativeStemsBeamAppliedLineDeltaSource::ReadyTransaction
    });
    if shadow
        .line_states
        .iter()
        .any(|line| line.v_linker == frontier.v_linker)
    {
        return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
            phase: "serial frontier duplicate V line state",
        });
    }
    shadow.line_states.push(NativeStemsBeamVLinkLineState {
        v_linker: frontier.v_linker,
        stored_theoretical_line: attempt.stored_theoretical_line_before,
        builder_line: attempt.initial_stem_line,
        current_attachment_line: attempt
            .attachment_aliases_stored_theoretical_line
            .then_some(attempt.stored_theoretical_line_before),
    });

    let mut selected_glyphs = Vec::with_capacity(attempt.glyphs.len());
    let mut known_glyphs_added = 0_usize;
    for glyph in &attempt.glyphs {
        let content = NativeStemsBeamFixedGlyphContent {
            bounds: glyph.bounds,
            weight: glyph.weight,
            run_table: glyph.structural_key.run_table.clone(),
        };
        let matches = glyph_bootstrap
            .iter()
            .filter(|entry| entry.content == content)
            .collect::<Vec<_>>();
        let [entry] = matches.as_slice() else {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "serial frontier glyph bootstrap cardinality",
            });
        };
        validate_content(&entry.content)?;
        if entry.glyph_id <= 0
            || entry.canonical_alias != entry.glyph_id as usize
            || entry.glyph_id > shadow.glyph_index.persistent_ids.sheet_last_id
        {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "serial frontier glyph bootstrap identity",
            });
        }
        let selected = NativeStemsBeamSelectedGlyphBinding {
            reference: glyph.reference,
            canonical_alias: entry.canonical_alias,
            glyph_id: entry.glyph_id,
            content: content.clone(),
        };
        let existing_selected = shadow
            .selected_glyph_bindings
            .iter()
            .filter(|binding| binding.reference == glyph.reference)
            .collect::<Vec<_>>();
        match existing_selected.as_slice() {
            [] => shadow.selected_glyph_bindings.push(selected),
            [existing] if **existing == selected => {}
            _ => {
                return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                    phase: "serial frontier selected glyph binding join",
                });
            }
        }
        let known = shadow
            .glyph_index
            .known_canonical_glyphs
            .iter()
            .filter(|known| known.content == content)
            .collect::<Vec<_>>();
        match known.as_slice() {
            [] => {
                shadow.glyph_index.known_canonical_glyphs.push(
                    NativeStemsBeamKnownCanonicalGlyph {
                        canonical_alias: entry.canonical_alias,
                        glyph_id: entry.glyph_id,
                        content,
                        active_in_index: entry.active_in_index,
                        strongly_retained: entry.strongly_retained,
                    },
                );
                known_glyphs_added += 1;
            }
            [known]
                if known.canonical_alias == entry.canonical_alias
                    && known.glyph_id == entry.glyph_id
                    && known.active_in_index == entry.active_in_index
                    && known.strongly_retained == entry.strongly_retained => {}
            _ => {
                return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                    phase: "serial frontier known glyph bootstrap join",
                });
            }
        }
        selected_glyphs.push(glyph.reference);
    }
    shadow.glyph_index.exhaustive_lookup = None;
    shadow.system_stems.exhaustive_lookup = None;
    shadow.system_stems.authority = NativeStemsBeamRegistryAuthority::CompleteSinceEmptyBaseline;
    validate_transaction_state(&shadow)?;
    *state = shadow;
    Ok(NativeStemsBeamFrontierPreparation {
        plan: frontier.plan,
        v_linker: frontier.v_linker,
        selected_glyphs,
        known_glyphs_added,
        line_state_added: true,
    })
}

/// Prepare one frontier from a validated one-time first-STEMS bridge.
///
/// This supplies no absence authority for the opaque page-global suffix. Each
/// selected glyph must resolve through the system-visible modeled prefix.
pub fn prepare_native_stems_beam_vlink_frontier_state_from_first_stems_bridge(
    scheduler_system: &NativeStemsBeamSchedulerSystem,
    plan_system: &NativeStemsBeamLinkPlanSystem,
    state: &mut NativeStemsBeamVLinkTransactionState,
    bridge: &NativeStemsFirstGlyphIndexBridge,
    system_stems_proof: NativeStemsBeamSystemStemAuthorityProof,
) -> Result<NativeStemsBeamFrontierPreparation, NativeStemsBeamVLinkTransactionError> {
    let snapshot = bridge.snapshot();
    let ids = state.glyph_index.persistent_ids;
    if scheduler_system.system_id != snapshot.system_id
        || state.glyph_index.union_size != snapshot.union_size
        || ids.sheet_last_id < snapshot.persistent_ids.sheet_last_id
        || ids.sheet_last_id != ids.glyph_index_last_id
        || ids.sheet_last_id != ids.inter_index_last_id
    {
        return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
            phase: "first-STEMS carried glyph snapshot",
        });
    }
    let frontier = match &scheduler_system.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => {
            return Err(
                NativeStemsBeamVLinkTransactionError::NoAwaitingVLinkTransaction {
                    system_id: scheduler_system.system_id,
                },
            );
        }
    };
    let (attempt, _) = resolve_plan(plan_system, frontier.plan)?;
    let bootstrap = attempt
        .glyphs
        .iter()
        .map(|selected| bridge.selected_bootstrap(selected))
        .collect::<Result<Vec<_>, _>>()?;
    prepare_native_stems_beam_vlink_frontier_state(
        scheduler_system,
        plan_system,
        state,
        &bootstrap,
        system_stems_proof,
    )
}

/// Prepare one frontier from the owned native canonical-glyph registry.
///
/// Unlike the first-STEMS bridge, this path reads no page-wide Java snapshot.
pub fn prepare_native_stems_beam_vlink_frontier_state_from_modeled_registry(
    scheduler_system: &NativeStemsBeamSchedulerSystem,
    plan_system: &NativeStemsBeamLinkPlanSystem,
    state: &mut NativeStemsBeamVLinkTransactionState,
    registry: &NativeStemsModeledGlyphRegistry,
    system_stems_proof: NativeStemsBeamSystemStemAuthorityProof,
) -> Result<NativeStemsBeamFrontierPreparation, NativeStemsBeamVLinkTransactionError> {
    if scheduler_system.system_id != registry.system_id {
        return Err(NativeStemsBeamVLinkTransactionError::SystemOrder);
    }
    let frontier = match &scheduler_system.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => {
            return Err(
                NativeStemsBeamVLinkTransactionError::NoAwaitingVLinkTransaction {
                    system_id: scheduler_system.system_id,
                },
            );
        }
    };
    let (attempt, _) = resolve_plan(plan_system, frontier.plan)?;
    let bootstrap = attempt
        .glyphs
        .iter()
        .map(|selected| registry.selected_bootstrap(selected))
        .collect::<Result<Vec<_>, _>>()?;
    prepare_native_stems_beam_vlink_frontier_state(
        scheduler_system,
        plan_system,
        state,
        &bootstrap,
        system_stems_proof,
    )
}

/// Construct the first shared-sheet B12 frontier from native modeled glyphs.
///
/// The shared native identity domain begins after the modeled registry. This
/// keeps modeled glyph and subsequently allocated StemInter identities unique
/// without importing Java's sheet-wide EntityIndex watermark. The function
/// imports no page-wide GlyphIndex union, aliases, equality scan, selected
/// bindings, line state, `systemStems` snapshot, or persistent-ID seed.
pub fn initialize_native_stems_beam_vlink_first_frontier_state_from_modeled_registry(
    scheduler_system: &NativeStemsBeamSchedulerSystem,
    plan_system: &NativeStemsBeamLinkPlanSystem,
    registry: &NativeStemsModeledGlyphRegistry,
) -> Result<
    (
        NativeStemsBeamFrontierPreparation,
        NativeStemsBeamVLinkTransactionState,
    ),
    NativeStemsBeamVLinkTransactionError,
> {
    let system_id = scheduler_system.system_id;
    if system_id == 0 || plan_system.system_id != system_id || registry.system_id != system_id {
        return Err(NativeStemsBeamVLinkTransactionError::PersistentAllocatorMismatch);
    }
    let last_id = i32::try_from(registry.len())
        .map_err(|_| NativeStemsBeamVLinkTransactionError::PersistentAllocatorMismatch)?;
    let persistent_ids = NativeStemsBeamPersistentIdState {
        sheet_last_id: last_id,
        glyph_index_last_id: last_id,
        inter_index_last_id: last_id,
    };
    let mut state = NativeStemsBeamVLinkTransactionState {
        scope: NativeStemsBeamVLinkTransactionScope::SharedSheetFirstFrontier { system_id },
        glyph_index: NativeStemsBeamGlyphIndexTransactionState {
            persistent_ids,
            alias_order: NativeStemsBeamGlyphAliasOrder::NativeModeledOrdinal,
            union_size: registry.len(),
            known_canonical_glyphs: Vec::new(),
            exhaustive_lookup: None,
        },
        selected_glyph_bindings: Vec::new(),
        line_states: Vec::new(),
        applied_line_deltas: Vec::new(),
        system_stems: NativeStemsBeamSystemStemTransactionState {
            system_id,
            next_stem_identity: 0,
            authority: NativeStemsBeamRegistryAuthority::RequiresExhaustiveScan,
            known_stems: Vec::new(),
            exhaustive_lookup: None,
        },
    };
    let proof =
        NativeStemsBeamSystemStemAuthorityProof::from_empty_stems_entry(&state.system_stems, 0)?;
    let preparation = prepare_native_stems_beam_vlink_frontier_state_from_modeled_registry(
        scheduler_system,
        plan_system,
        &mut state,
        registry,
        proof,
    )?;
    state.scope = NativeStemsBeamVLinkTransactionScope::SharedSheetFirstFrontier { system_id };
    validate_transaction_state(&state)?;
    Ok((preparation, state))
}

/// Construct a later system's first shared-sheet B12 frontier.
///
/// Unlike the sheet's first frontier, the persistent allocator is not the
/// structural-registry length: inter allocations from every completed system
/// remain interleaved in the shared page identity domain. System-local stems
/// still begin from the proven empty map.
pub fn initialize_native_stems_beam_vlink_serial_frontier_state_from_modeled_registry(
    scheduler_system: &NativeStemsBeamSchedulerSystem,
    plan_system: &NativeStemsBeamLinkPlanSystem,
    registry: &NativeStemsModeledGlyphRegistry,
) -> Result<
    (
        NativeStemsBeamFrontierPreparation,
        NativeStemsBeamVLinkTransactionState,
    ),
    NativeStemsBeamVLinkTransactionError,
> {
    let system_id = scheduler_system.system_id;
    let persistent_ids = registry.persistent_ids();
    if system_id <= 1
        || plan_system.system_id != system_id
        || registry.system_id != system_id
        || persistent_ids.sheet_last_id <= 0
        || persistent_ids.sheet_last_id != persistent_ids.glyph_index_last_id
        || persistent_ids.sheet_last_id != persistent_ids.inter_index_last_id
        || usize::try_from(persistent_ids.sheet_last_id)
            .map_err(|_| NativeStemsBeamVLinkTransactionError::PersistentAllocatorMismatch)?
            < registry.len()
    {
        return Err(NativeStemsBeamVLinkTransactionError::PersistentAllocatorMismatch);
    }
    let mut state = NativeStemsBeamVLinkTransactionState {
        scope: NativeStemsBeamVLinkTransactionScope::SharedSheetSerial { system_id },
        glyph_index: NativeStemsBeamGlyphIndexTransactionState {
            persistent_ids,
            alias_order: NativeStemsBeamGlyphAliasOrder::NativeModeledOrdinal,
            union_size: registry.len(),
            known_canonical_glyphs: Vec::new(),
            exhaustive_lookup: None,
        },
        selected_glyph_bindings: Vec::new(),
        line_states: Vec::new(),
        applied_line_deltas: Vec::new(),
        system_stems: NativeStemsBeamSystemStemTransactionState {
            system_id,
            next_stem_identity: 0,
            authority: NativeStemsBeamRegistryAuthority::RequiresExhaustiveScan,
            known_stems: Vec::new(),
            exhaustive_lookup: None,
        },
    };
    let proof =
        NativeStemsBeamSystemStemAuthorityProof::from_empty_stems_entry(&state.system_stems, 0)?;
    let preparation = prepare_native_stems_beam_vlink_frontier_state_from_modeled_registry(
        scheduler_system,
        plan_system,
        &mut state,
        registry,
        proof,
    )?;
    state.scope = NativeStemsBeamVLinkTransactionScope::SharedSheetSerial { system_id };
    validate_transaction_state(&state)?;
    Ok((preparation, state))
}

/// Materialize the current frontier's selected glyph candidate without
/// mutating the registry.
///
/// A serial carrier uses this after selected component bindings are prepared
/// and before installing the disclosed page-wide exhaustive equality result
/// needed by a compound candidate.
pub fn materialize_native_stems_beam_frontier_candidate(
    scheduler_system: &NativeStemsBeamSchedulerSystem,
    plan_system: &NativeStemsBeamLinkPlanSystem,
    state: &NativeStemsBeamVLinkTransactionState,
) -> Result<NativeStemsBeamFixedGlyphContent, NativeStemsBeamVLinkTransactionError> {
    if scheduler_system.system_id != plan_system.system_id
        || state.system_stems.system_id != scheduler_system.system_id
    {
        return Err(NativeStemsBeamVLinkTransactionError::SystemOrder);
    }
    validate_transaction_state(state)?;
    let frontier = match &scheduler_system.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => {
            return Err(
                NativeStemsBeamVLinkTransactionError::NoAwaitingVLinkTransaction {
                    system_id: scheduler_system.system_id,
                },
            );
        }
    };
    let (attempt, start_v) = resolve_plan(plan_system, frontier.plan)?;
    if frontier.v_linker != start_v
        || frontier.outcome != NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem
    {
        return Err(NativeStemsBeamVLinkTransactionError::InvalidProduct {
            system_id: scheduler_system.system_id,
            phase: "frontier candidate plan/V join",
        });
    }
    Ok(materialize_candidate(attempt, state)?.content)
}

impl From<NativeStemGeometryError> for NativeStemsBeamVLinkTransactionError {
    fn from(source: NativeStemGeometryError) -> Self {
        Self::StemChecker(source)
    }
}

/// Commit the first serial persistent prefix at one scheduler frontier.
///
/// All evidence is validated and the checker is evaluated before `state` is
/// changed. Thus a typed boundary error leaves this invocation untouched.
/// Java's ordinary null result is instead [`NativeStemsBeamCreateStemDisposition::Rejected`]
/// and commits the line/registration prefix exactly as Java does.
pub fn apply_native_stems_beam_vlink_create_stem_transaction(
    scheduler_system: &NativeStemsBeamSchedulerSystem,
    builder_system: &NativeStemsBeamBuilderSystem,
    plan_system: &NativeStemsBeamLinkPlanSystem,
    state: &mut NativeStemsBeamVLinkTransactionState,
    checker: &NativeStemsBeamStemCheckerContext,
) -> Result<NativeStemsBeamVLinkTransaction, NativeStemsBeamVLinkTransactionError> {
    let system_id = scheduler_system.system_id;
    if builder_system.system_id != system_id
        || plan_system.system_id != system_id
        || state.system_stems.system_id != system_id
        || builder_system.interline != plan_system.interline
        || checker.parameters.interline != plan_system.interline
        || checker.parameters.maximum_stem_width != builder_system.max_stem_thickness
    {
        return Err(NativeStemsBeamVLinkTransactionError::SystemOrder);
    }
    match state.scope {
        NativeStemsBeamVLinkTransactionScope::SharedSheetFirstFrontier { system_id: first }
            if first == system_id && first == 1 => {}
        NativeStemsBeamVLinkTransactionScope::SharedSheetSerial { system_id: serial }
            if serial == system_id => {}
        NativeStemsBeamVLinkTransactionScope::IsolatedFreshSheetFrontier {
            system_id: isolated,
        } if isolated == system_id => {}
        _ => return Err(NativeStemsBeamVLinkTransactionError::SystemOrder),
    }
    validate_checker_context(checker)?;
    validate_transaction_state(state)?;

    let frontier = match &scheduler_system.status {
        NativeStemsBeamSchedulerStatus::AwaitingVLinkTransaction(frontier) => frontier.as_ref(),
        _ => {
            return Err(
                NativeStemsBeamVLinkTransactionError::NoAwaitingVLinkTransaction { system_id },
            );
        }
    };
    if frontier.outcome != NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem
        || frontier.plan.system_id != system_id
        || frontier.plan.stem_profile < 0
        || frontier.plan.stem_profile > 4
    {
        return Err(NativeStemsBeamVLinkTransactionError::InvalidProduct {
            system_id,
            phase: "scheduler frontier",
        });
    }
    let (attempt, builder_start) = resolve_plan(plan_system, frontier.plan)?;
    let builder = builder_system
        .builders
        .get(frontier.plan.builder_ordinal)
        .ok_or(NativeStemsBeamVLinkTransactionError::InvalidProduct {
            system_id,
            phase: "builder ordinal",
        })?;
    if builder.builder_ordinal != frontier.plan.builder_ordinal
        || builder.start != frontier.v_linker
        || builder_start != frontier.v_linker
        || attempt.stem_profile != frontier.plan.stem_profile
        || attempt.link_profile != plan_system.link_profile
        || attempt.outcome != NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem
        || attempt.glyphs.is_empty()
    {
        return Err(NativeStemsBeamVLinkTransactionError::InvalidProduct {
            system_id,
            phase: "plan/builder join",
        });
    }

    let prefix_line_deltas = validate_line_prefix(
        scheduler_system,
        frontier.invocation_ordinal,
        frontier.v_linker,
        attempt,
        state,
    )?;
    let pending_delta = validate_pending_delta(
        frontier,
        attempt,
        scheduler_system.deferred_line_deltas.len(),
    )?;
    let materialized = materialize_candidate(attempt, state)?;
    let candidate = materialized.content;
    let singleton_identity = materialized.singleton_identity;
    let prepared_registration =
        prepare_registration(&candidate, materialized.selected_canonical_identity, state)?;
    let prepared_stem_lookup = prepare_system_stem_lookup(
        &candidate,
        prepared_registration.glyph_id,
        &state.system_stems,
    )?;

    let prepared_result = match prepared_stem_lookup.stem.as_ref() {
        Some(stem) => PreparedCreateStemResult {
            checker_result: None,
            disposition: NativeStemsBeamCreateStemDisposition::Reused {
                stem_identity: stem.stem_identity,
            },
            stem: Some(stem.clone()),
            next_stem_identity: state.system_stems.next_stem_identity,
        },
        None => prepare_new_stem(
            &candidate,
            prepared_registration.glyph_id,
            frontier.plan.stem_profile,
            state.system_stems.next_stem_identity,
            checker,
        )?,
    };

    // No fallible operation occurs below this point. Mutation order matches
    // Java even though preparation above made the typed boundary fail-closed.
    let mut mutation_order = Vec::new();
    for delta in &prefix_line_deltas {
        apply_line_delta(state, delta);
        state
            .applied_line_deltas
            .push(NativeStemsBeamAppliedLineDelta {
                system_id,
                source: NativeStemsBeamAppliedLineDeltaSource::KnownFalsePrefix {
                    delta_ordinal: delta.delta_ordinal,
                },
                delta: delta.clone(),
            });
        mutation_order.push(NativeStemsBeamVLinkMutation::StoredLineDeltaApplied);
    }
    if let Some(delta) = pending_delta.as_ref() {
        apply_line_delta(state, delta);
        state
            .applied_line_deltas
            .push(NativeStemsBeamAppliedLineDelta {
                system_id,
                source: NativeStemsBeamAppliedLineDeltaSource::ReadyTransaction,
                delta: delta.clone(),
            });
        mutation_order.push(NativeStemsBeamVLinkMutation::StoredLineDeltaApplied);
    }

    commit_registration(
        state,
        &candidate,
        &prepared_registration,
        &prepared_result.disposition,
        &mut mutation_order,
    );
    if prepared_stem_lookup.consume_certificate {
        state.system_stems.exhaustive_lookup = None;
    }
    if let Some(stem) = prepared_stem_lookup.stem
        && !state
            .system_stems
            .known_stems
            .iter()
            .any(|known| known.stem_identity == stem.stem_identity)
    {
        state.system_stems.known_stems.push(stem);
    }
    if matches!(
        prepared_result.disposition,
        NativeStemsBeamCreateStemDisposition::CreatedChecked { .. }
            | NativeStemsBeamCreateStemDisposition::CreatedArtificial { .. }
    ) {
        let stem = prepared_result
            .stem
            .clone()
            .expect("created disposition was prepared with a stem");
        state.system_stems.next_stem_identity = prepared_result.next_stem_identity;
        state.system_stems.known_stems.push(stem.clone());
        mutation_order.push(NativeStemsBeamVLinkMutation::SystemStemInserted {
            stem_identity: stem.stem_identity,
        });
    }

    Ok(NativeStemsBeamVLinkTransaction {
        system_id,
        invocation_ordinal: frontier.invocation_ordinal,
        plan: frontier.plan,
        scope: state.scope,
        candidate,
        candidate_glyph_id_before_registration: singleton_identity.map(|(id, _)| id),
        applied_prefix_line_deltas: prefix_line_deltas,
        applied_line_delta: pending_delta,
        registration: NativeStemsBeamGlyphRegistration {
            alias_order: state.glyph_index.alias_order,
            canonical_alias: prepared_registration.canonical_alias,
            glyph_id: prepared_registration.glyph_id,
            action: prepared_registration.action,
            post_union_size: prepared_registration.post_union_size,
        },
        checker_result: prepared_result.checker_result,
        disposition: prepared_result.disposition,
        stem: prepared_result.stem,
        mutation_order,
        sig_vertex_mutation_count: 0,
        sig_relation_mutation_count: 0,
        linker_flag_mutation_count: 0,
    })
}

#[derive(Clone, Copy)]
struct PreparedRegistration {
    canonical_alias: usize,
    glyph_id: i32,
    action: NativeStemsBeamGlyphRegistrationAction,
    post_union_size: usize,
    consume_certificate: bool,
    known_index: Option<usize>,
    strong_reference_after_rejection: bool,
}

struct MaterializedCandidate {
    content: NativeStemsBeamFixedGlyphContent,
    /// Object identity passed directly to `registerOriginal`; compounds are
    /// newly allocated Java objects and therefore have no pre-registration ID.
    singleton_identity: Option<(i32, usize)>,
    /// A selected registered object whose exact content equals the candidate.
    /// This can exist even for a compound when other selected pixels are a
    /// structural subset of that canonical component.
    selected_canonical_identity: Option<(i32, usize)>,
}

struct PreparedSystemStemLookup {
    stem: Option<NativeStemsBeamKnownSystemStem>,
    consume_certificate: bool,
}

struct PreparedCreateStemResult {
    checker_result: Option<NativeStemCheckResult>,
    disposition: NativeStemsBeamCreateStemDisposition,
    stem: Option<NativeStemsBeamKnownSystemStem>,
    next_stem_identity: usize,
}

fn validate_checker_context(
    checker: &NativeStemsBeamStemCheckerContext,
) -> Result<(), NativeStemsBeamVLinkTransactionError> {
    if checker.parameters.interline <= 0
        || checker.parameters.maximum_stem_width <= 0
        || checker.parameters.belt_margin_dx < 0
        || !checker.parameters.sheet_skew_slope.is_finite()
        || !checker.minimum_stem_grade.is_finite()
        || checker.minimum_stem_grade < 0.0
        || !checker.artificial_stem_grade.is_finite()
        || checker.artificial_stem_grade < 0.0
    {
        return Err(NativeStemsBeamVLinkTransactionError::InvalidProduct {
            system_id: 0,
            phase: "StemChecker context",
        });
    }
    Ok(())
}

pub(crate) fn validate_transaction_state(
    state: &NativeStemsBeamVLinkTransactionState,
) -> Result<(), NativeStemsBeamVLinkTransactionError> {
    let ids = state.glyph_index.persistent_ids;
    if ids.sheet_last_id < 0
        || ids.sheet_last_id != ids.glyph_index_last_id
        || ids.sheet_last_id != ids.inter_index_last_id
        || state.glyph_index.known_canonical_glyphs.len() > state.glyph_index.union_size
    {
        return Err(NativeStemsBeamVLinkTransactionError::PersistentAllocatorMismatch);
    }
    for (index, glyph) in state.glyph_index.known_canonical_glyphs.iter().enumerate() {
        validate_content(&glyph.content)?;
        if glyph.glyph_id <= 0
            || glyph.glyph_id > ids.sheet_last_id
            || glyph.canonical_alias != glyph.glyph_id as usize
            || state.glyph_index.known_canonical_glyphs[..index]
                .iter()
                .any(|other| {
                    other.canonical_alias == glyph.canonical_alias
                        || other.glyph_id == glyph.glyph_id
                        || other.content == glyph.content
                })
        {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "known canonical glyphs",
            });
        }
    }
    for (index, binding) in state.selected_glyph_bindings.iter().enumerate() {
        validate_content(&binding.content)?;
        if binding.glyph_id <= 0
            || binding.glyph_id > ids.sheet_last_id
            || binding.canonical_alias != binding.glyph_id as usize
            || state.selected_glyph_bindings[..index].iter().any(|other| {
                if other.reference == binding.reference {
                    return true;
                }
                let same_canonical_identity = other.glyph_id == binding.glyph_id
                    && other.canonical_alias == binding.canonical_alias;
                let same_content = other.content == binding.content;
                same_canonical_identity != same_content
            })
            || state
                .glyph_index
                .known_canonical_glyphs
                .iter()
                .any(|known| {
                    let same_canonical_identity = known.glyph_id == binding.glyph_id
                        && known.canonical_alias == binding.canonical_alias;
                    let same_content = known.content == binding.content;
                    same_canonical_identity != same_content
                })
        {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "selected glyph bindings",
            });
        }
    }
    if let Some(scan) = &state.glyph_index.exhaustive_lookup {
        validate_glyph_scan(
            scan,
            ids.sheet_last_id,
            state.glyph_index.alias_order,
            state.glyph_index.union_size,
        )?;
        if let NativeStemsBeamExhaustiveGlyphLookup::Present {
            canonical_alias,
            glyph_id,
            active_in_index,
        } = scan.lookup
            && (state
                .glyph_index
                .known_canonical_glyphs
                .iter()
                .any(|known| {
                    let same_canonical_identity =
                        known.glyph_id == glyph_id && known.canonical_alias == canonical_alias;
                    let same_content = known.content == scan.candidate;
                    same_canonical_identity != same_content
                        || (same_canonical_identity && known.active_in_index != active_in_index)
                })
                || state.selected_glyph_bindings.iter().any(|selected| {
                    let same_canonical_identity = selected.glyph_id == glyph_id
                        && selected.canonical_alias == canonical_alias;
                    let same_content = selected.content == scan.candidate;
                    same_canonical_identity != same_content
                }))
        {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "exhaustive/known canonical identity",
            });
        }
    }
    for (index, line) in state.line_states.iter().enumerate() {
        if state.line_states[..index]
            .iter()
            .any(|other| other.v_linker == line.v_linker)
        {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "duplicate V line state",
            });
        }
    }
    validate_system_stems(&state.system_stems, ids.sheet_last_id)?;
    validate_represented_persistent_id_domains(state)?;
    Ok(())
}

fn validate_represented_persistent_id_domains(
    state: &NativeStemsBeamVLinkTransactionState,
) -> Result<(), NativeStemsBeamVLinkTransactionError> {
    // The snapshot is intentionally partial, so only identities represented
    // here can be cross-checked. Repeated views of one canonical Glyph are
    // legitimate; distinct Java Glyph content and IDs are one-to-one.
    let mut glyphs = Vec::new();
    for glyph in &state.glyph_index.known_canonical_glyphs {
        glyphs.push((glyph.glyph_id, &glyph.content));
    }
    for selected in &state.selected_glyph_bindings {
        glyphs.push((selected.glyph_id, &selected.content));
    }
    if let Some(scan) = &state.glyph_index.exhaustive_lookup
        && let NativeStemsBeamExhaustiveGlyphLookup::Present { glyph_id, .. } = scan.lookup
    {
        glyphs.push((glyph_id, &scan.candidate));
    }
    for stem in &state.system_stems.known_stems {
        glyphs.push((stem.glyph_id, &stem.glyph_content));
    }
    if let Some(scan) = &state.system_stems.exhaustive_lookup
        && let NativeStemsBeamExhaustiveSystemStemLookup::Present(stem) = &scan.lookup
    {
        glyphs.push((stem.glyph_id, &stem.glyph_content));
    }
    for (index, (glyph_id, content)) in glyphs.iter().enumerate() {
        if glyphs[..index].iter().any(|(other_id, other_content)| {
            let same_id = other_id == glyph_id;
            let same_content = *other_content == *content;
            same_id != same_content
        }) {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "represented canonical glyph identity",
            });
        }
    }

    // Glyphs and Inters allocate from the same Sheet counter. Positive Inter
    // IDs are unique across distinct native Stem objects and cannot alias any
    // represented Glyph ID. SIG-null stems carry no Inter ID here.
    let mut inters = Vec::new();
    for stem in &state.system_stems.known_stems {
        if let Some(inter_id) = stem.inter_id {
            inters.push((stem.stem_identity, inter_id));
        }
    }
    if let Some(scan) = &state.system_stems.exhaustive_lookup
        && let NativeStemsBeamExhaustiveSystemStemLookup::Present(stem) = &scan.lookup
        && let Some(inter_id) = stem.inter_id
    {
        inters.push((stem.stem_identity, inter_id));
    }
    for (index, (stem_identity, inter_id)) in inters.iter().enumerate() {
        if glyphs.iter().any(|(glyph_id, _)| glyph_id == inter_id)
            || inters[..index].iter().any(|(other_identity, other_id)| {
                let same_native_identity = other_identity == stem_identity;
                let same_java_id = other_id == inter_id;
                same_native_identity != same_java_id
            })
        {
            return Err(NativeStemsBeamVLinkTransactionError::SystemStemInvariant {
                phase: "represented persistent Inter identity",
            });
        }
    }
    Ok(())
}

fn validate_content(
    content: &NativeStemsBeamFixedGlyphContent,
) -> Result<(), NativeStemsBeamVLinkTransactionError> {
    let checked_right = content
        .bounds
        .x
        .checked_add(content.bounds.width)
        .and_then(|right| i32::try_from(right).ok());
    let checked_bottom = content
        .bounds
        .y
        .checked_add(content.bounds.height)
        .and_then(|bottom| i32::try_from(bottom).ok());
    if content.bounds.width == 0
        || content.bounds.height == 0
        || content.run_table.width() != content.bounds.width
        || content.run_table.height() != content.bounds.height
        || content.run_table.weight() == 0
        || content.weight != content.run_table.weight()
        || i32::try_from(content.bounds.x).is_err()
        || i32::try_from(content.bounds.y).is_err()
        || i32::try_from(content.bounds.width).is_err()
        || i32::try_from(content.bounds.height).is_err()
        || checked_right.is_none()
        || checked_bottom.is_none()
    {
        return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
            phase: "fixed glyph content",
        });
    }
    Ok(())
}

fn run_table_sha256(table: &RunTable) -> String {
    let orientation = match table.orientation() {
        Orientation::Horizontal => "HORIZONTAL",
        Orientation::Vertical => "VERTICAL",
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

fn first_stems_snapshot_sha256(
    entries: &[NativeStemsFirstGlyphSnapshotEntry],
    originals: bool,
) -> String {
    let mut rows = entries
        .iter()
        .filter(|entry| {
            if originals {
                entry.live_original
            } else {
                entry.active_in_index
            }
        })
        .map(|entry| {
            let bounds = entry.fingerprint.bounds;
            let token = format!(
                "g:{}:{}:{}:{}:{}",
                bounds.x, bounds.y, bounds.width, bounds.height, entry.fingerprint.run_table_sha256
            );
            if originals {
                format!("{token}:active={}", entry.active_in_index)
            } else {
                token
            }
        })
        .collect::<Vec<_>>();
    rows.sort();
    let mut bytes = Vec::new();
    for row in rows {
        bytes.extend_from_slice(row.as_bytes());
        bytes.push(b'\n');
    }
    sha256_hex(&bytes)
}

fn modeled_registry_sha256(
    entries: &[NativeStemsBeamGlyphRegistryBootstrapEntry],
    originals: bool,
) -> String {
    let mut rows = entries
        .iter()
        .filter(|entry| {
            if originals {
                entry.strongly_retained
            } else {
                entry.active_in_index
            }
        })
        .map(|entry| {
            let bounds = entry.content.bounds;
            format!(
                "{}:{}:{}:{}:{}:{}:{}:{}",
                entry.glyph_id,
                entry.canonical_alias,
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                entry.content.weight,
                run_table_sha256(&entry.content.run_table)
            )
        })
        .collect::<Vec<_>>();
    rows.sort();
    let mut bytes = Vec::new();
    for row in rows {
        bytes.extend_from_slice(row.as_bytes());
        bytes.push(b'\n');
    }
    sha256_hex(&bytes)
}

pub(crate) fn sha256_hex(input: &[u8]) -> String {
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
    let mut padded = input.to_vec();
    let bit_len = u64::try_from(input.len()).expect("glyph fingerprint length fits u64") * 8;
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    let mut digest = [
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
            let low = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let high = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(low)
                .wrapping_add(words[index - 7])
                .wrapping_add(high);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = digest;
        for index in 0..64 {
            let sigma1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let first = h
                .wrapping_add(sigma1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let sigma0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let second = sigma0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(first);
            d = c;
            c = b;
            b = a;
            a = first.wrapping_add(second);
        }
        for (slot, value) in digest.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
    digest.iter().map(|word| format!("{word:08x}")).collect()
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_glyph_scan(
    scan: &NativeStemsBeamExhaustiveGlyphEqualsScan,
    last_id: i32,
    alias_order: NativeStemsBeamGlyphAliasOrder,
    union_size: usize,
) -> Result<(), NativeStemsBeamVLinkTransactionError> {
    validate_content(&scan.candidate)?;
    let maximum_union_size = scan
        .baseline_active_count
        .checked_add(scan.baseline_live_original_count)
        .ok_or(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
            phase: "exhaustive GlyphIndex scan coverage",
        })?;
    if scan.scanned_active_count != scan.baseline_active_count
        || scan.scanned_live_original_count != scan.baseline_live_original_count
        || scan.alias_order != alias_order
        || scan.baseline_union_size != union_size
        || scan.baseline_union_size
            < scan
                .baseline_active_count
                .max(scan.baseline_live_original_count)
        || scan.baseline_union_size > maximum_union_size
        || scan.equal_active_matches > scan.scanned_active_count
        || scan.equal_original_matches > scan.scanned_live_original_count
        || !valid_sha256(&scan.baseline_active_sha256)
        || !valid_sha256(&scan.baseline_live_original_sha256)
    {
        return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
            phase: "exhaustive GlyphIndex scan coverage",
        });
    }
    match scan.lookup {
        NativeStemsBeamExhaustiveGlyphLookup::Absent
            if scan.equal_original_matches == 0 && scan.equal_active_matches == 0 => {}
        NativeStemsBeamExhaustiveGlyphLookup::Present {
            canonical_alias,
            glyph_id,
            active_in_index,
        } if scan.equal_original_matches == 1
            && canonical_alias == glyph_id as usize
            && glyph_id > 0
            && glyph_id <= last_id
            && ((!active_in_index && scan.equal_active_matches == 0)
                || (active_in_index && scan.equal_active_matches == 1)) => {}
        _ => {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "exhaustive GlyphIndex equality result",
            });
        }
    }
    Ok(())
}

fn validate_system_stems(
    state: &NativeStemsBeamSystemStemTransactionState,
    last_id: i32,
) -> Result<(), NativeStemsBeamVLinkTransactionError> {
    for (index, stem) in state.known_stems.iter().enumerate() {
        validate_known_stem(stem, last_id)?;
        if state.known_stems[..index].iter().any(|other| {
            other.stem_identity == stem.stem_identity || other.glyph_content == stem.glyph_content
        }) {
            return Err(NativeStemsBeamVLinkTransactionError::SystemStemInvariant {
                phase: "known system stems",
            });
        }
    }
    if state
        .known_stems
        .iter()
        .any(|stem| stem.stem_identity >= state.next_stem_identity)
    {
        return Err(NativeStemsBeamVLinkTransactionError::SystemStemInvariant {
            phase: "next stem identity",
        });
    }
    if let Some(scan) = &state.exhaustive_lookup {
        validate_content(&scan.candidate)?;
        if scan.scanned_entry_count != scan.baseline_entry_count
            || state.known_stems.len() > scan.baseline_entry_count
            || scan.equal_glyph_matches > scan.scanned_entry_count
            || !valid_sha256(&scan.baseline_sha256)
        {
            return Err(NativeStemsBeamVLinkTransactionError::SystemStemInvariant {
                phase: "exhaustive systemStems scan coverage",
            });
        }
        match &scan.lookup {
            NativeStemsBeamExhaustiveSystemStemLookup::Absent if scan.equal_glyph_matches == 0 => {}
            NativeStemsBeamExhaustiveSystemStemLookup::Present(stem)
                if scan.equal_glyph_matches == 1
                    && stem.glyph_content == scan.candidate
                    && stem.stem_identity < state.next_stem_identity
                    && !state.known_stems.iter().any(|known| {
                        (known.stem_identity == stem.stem_identity
                            || known.glyph_content == stem.glyph_content)
                            && known != stem.as_ref()
                    }) =>
            {
                validate_known_stem(stem, last_id)?;
            }
            _ => {
                return Err(NativeStemsBeamVLinkTransactionError::SystemStemInvariant {
                    phase: "exhaustive systemStems equality result",
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_known_stem(
    stem: &NativeStemsBeamKnownSystemStem,
    last_id: i32,
) -> Result<(), NativeStemsBeamVLinkTransactionError> {
    validate_content(&stem.glyph_content)?;
    let median = stem.geometry.median;
    let median_values = [median.start.x, median.start.y, median.stop.x, median.stop.y];
    let expected_ribbon_bounds = VerticalRibbonArea::new(
        Segment {
            x1: median.start.x,
            y1: median.start.y,
            x2: median.stop.x,
            y2: median.stop.y,
        },
        stem.geometry.mean_thickness,
    )
    .integer_bounds();
    if stem.glyph_id <= 0
        || stem.glyph_id > last_id
        || stem.inter_id.is_some_and(|id| id <= 0 || id > last_id)
        || !median_values.into_iter().all(f64::is_finite)
        || median.start.y >= median.stop.y
        || !stem.geometry.mean_thickness.is_finite()
        || stem.geometry.mean_thickness <= 0.0
        || stem.geometry.ribbon_bounds.is_empty()
        || stem.geometry.ribbon_bounds != expected_ribbon_bounds
        || !valid_stem_grade(&stem.grade)
        || stem.sig_attached != stem.inter_id.is_some()
        || (!stem.sig_attached && stem.abnormal)
    {
        return Err(NativeStemsBeamVLinkTransactionError::SystemStemInvariant {
            phase: "stem payload",
        });
    }
    Ok(())
}

pub(crate) fn valid_stem_grade(grade: &NativeStemsBeamStemGrade) -> bool {
    match grade {
        NativeStemsBeamStemGrade::Artificial(grade) => (0.0..=1.0).contains(grade),
        NativeStemsBeamStemGrade::Checked(check) => {
            let values = [
                check.values.slope,
                check.values.straight,
                check.values.length,
                check.values.clean,
                check.values.black,
                check.values.black_ratio,
                check.values.gap,
            ];
            let impacts = [
                check.impacts.slope,
                check.impacts.straight,
                check.impacts.length,
                check.impacts.clean,
                check.impacts.black,
                check.impacts.black_ratio,
                check.impacts.gap,
            ];
            let counts = check.counts;
            let side_black = counts
                .left
                .checked_add(counts.right)
                .and_then(|sum| sum.checked_add(counts.both))
                .and_then(|sum| sum.checked_add(counts.clean));
            values
                .into_iter()
                .all(|value| value.is_finite() && value >= 0.0)
                && check.values.black_ratio <= 1.0
                && impacts
                    .into_iter()
                    .all(|impact| (0.0..=1.0).contains(&impact))
                && (0.0..=1.0).contains(&check.grade)
                && [
                    counts.largest_gap,
                    counts.white,
                    counts.black,
                    counts.left,
                    counts.right,
                    counts.both,
                    counts.clean,
                ]
                .into_iter()
                .all(|count| count >= 0)
                && side_black == Some(counts.black)
                && counts.largest_gap <= counts.white
        }
    }
}

fn resolve_plan(
    plan_system: &NativeStemsBeamLinkPlanSystem,
    reference: NativeStemsBeamPlanRef,
) -> Result<
    (&NativeStemsBeamLinkPlanAttempt, NativeStemsBeamVLinkerRef),
    NativeStemsBeamVLinkTransactionError,
> {
    let mut plan_ordinal = 0_usize;
    for (builder_ordinal, builder) in plan_system.builders.iter().enumerate() {
        if builder.builder_ordinal != builder_ordinal {
            return Err(NativeStemsBeamVLinkTransactionError::InvalidProduct {
                system_id: plan_system.system_id,
                phase: "plan builder order",
            });
        }
        for attempt in &builder.attempts {
            if plan_ordinal == reference.plan_ordinal {
                if builder_ordinal != reference.builder_ordinal {
                    return Err(NativeStemsBeamVLinkTransactionError::InvalidProduct {
                        system_id: plan_system.system_id,
                        phase: "flattened plan ordinal",
                    });
                }
                return Ok((attempt, builder.start));
            }
            plan_ordinal += 1;
        }
    }
    Err(NativeStemsBeamVLinkTransactionError::InvalidProduct {
        system_id: plan_system.system_id,
        phase: "missing flattened plan",
    })
}

fn validate_line_prefix(
    scheduler: &NativeStemsBeamSchedulerSystem,
    current_invocation_ordinal: usize,
    current_v: NativeStemsBeamVLinkerRef,
    attempt: &NativeStemsBeamLinkPlanAttempt,
    state: &NativeStemsBeamVLinkTransactionState,
) -> Result<Vec<NativeStemsBeamDeferredLineDelta>, NativeStemsBeamVLinkTransactionError> {
    let mut seen_v_linkers = Vec::new();
    let mut previous_invocation_ordinal = None;
    for (ordinal, expected) in scheduler.deferred_line_deltas.iter().enumerate() {
        if expected.delta_ordinal != ordinal
            || expected.invocation_ordinal >= current_invocation_ordinal
            || previous_invocation_ordinal
                .is_some_and(|previous| expected.invocation_ordinal <= previous)
            || expected.plan.system_id != scheduler.system_id
            || expected.before == expected.after
            || seen_v_linkers.contains(&expected.v_linker)
        {
            return Err(NativeStemsBeamVLinkTransactionError::InvalidProduct {
                system_id: scheduler.system_id,
                phase: "known-false line delta order",
            });
        }
        previous_invocation_ordinal = Some(expected.invocation_ordinal);
        seen_v_linkers.push(expected.v_linker);
    }

    let applied_prefix = state
        .applied_line_deltas
        .iter()
        .filter(|applied| applied.system_id == scheduler.system_id)
        .collect::<Vec<_>>();
    if applied_prefix.len() > scheduler.deferred_line_deltas.len() {
        return Err(NativeStemsBeamVLinkTransactionError::DuplicateLineDelta);
    }
    for (ordinal, applied) in applied_prefix.iter().enumerate() {
        let expected = &scheduler.deferred_line_deltas[ordinal];
        match applied.source {
            NativeStemsBeamAppliedLineDeltaSource::KnownFalsePrefix { delta_ordinal }
                if delta_ordinal == ordinal && applied.delta == *expected => {}
            _ => {
                return Err(NativeStemsBeamVLinkTransactionError::DuplicateLineDelta);
            }
        }
    }
    for (ordinal, expected) in scheduler.deferred_line_deltas.iter().enumerate() {
        let line = line_state(state, expected.v_linker)?;
        if ordinal < applied_prefix.len() {
            validate_line_after_delta(line, expected)?;
        } else {
            validate_line_before_delta(line, expected)?;
        }
    }

    if seen_v_linkers.contains(&current_v) {
        return Err(NativeStemsBeamVLinkTransactionError::InvalidProduct {
            system_id: scheduler.system_id,
            phase: "repeated V after known-false line delta",
        });
    }
    let line = line_state(state, current_v)?;
    if line.stored_theoretical_line != attempt.stored_theoretical_line_before
        || line.builder_line != attempt.initial_stem_line
        || (attempt.attachment_aliases_stored_theoretical_line
            && line.current_attachment_line != Some(attempt.stored_theoretical_line_before))
    {
        return Err(NativeStemsBeamVLinkTransactionError::LineStateMismatch {
            reference: current_v,
        });
    }
    Ok(scheduler.deferred_line_deltas[applied_prefix.len()..].to_vec())
}

fn line_state(
    state: &NativeStemsBeamVLinkTransactionState,
    reference: NativeStemsBeamVLinkerRef,
) -> Result<&NativeStemsBeamVLinkLineState, NativeStemsBeamVLinkTransactionError> {
    state
        .line_states
        .iter()
        .find(|line| line.v_linker == reference)
        .ok_or(NativeStemsBeamVLinkTransactionError::MissingLineState { reference })
}

fn validate_line_after_delta(
    line: &NativeStemsBeamVLinkLineState,
    delta: &NativeStemsBeamDeferredLineDelta,
) -> Result<(), NativeStemsBeamVLinkTransactionError> {
    if line.stored_theoretical_line != delta.after
        || (delta.builder_line_aliases && line.builder_line != delta.after)
        || (delta.attachment_aliases && line.current_attachment_line != Some(delta.after))
    {
        return Err(NativeStemsBeamVLinkTransactionError::LineStateMismatch {
            reference: delta.v_linker,
        });
    }
    Ok(())
}

fn validate_line_before_delta(
    line: &NativeStemsBeamVLinkLineState,
    delta: &NativeStemsBeamDeferredLineDelta,
) -> Result<(), NativeStemsBeamVLinkTransactionError> {
    if line.stored_theoretical_line != delta.before
        || (delta.builder_line_aliases && line.builder_line != delta.before)
        || (delta.attachment_aliases && line.current_attachment_line != Some(delta.before))
    {
        return Err(NativeStemsBeamVLinkTransactionError::LineStateMismatch {
            reference: delta.v_linker,
        });
    }
    Ok(())
}

fn validate_pending_delta(
    frontier: &crate::native_stems_beam_scheduler::NativeStemsBeamAwaitingVLinkTransaction,
    attempt: &NativeStemsBeamLinkPlanAttempt,
    expected_delta_ordinal: usize,
) -> Result<Option<NativeStemsBeamDeferredLineDelta>, NativeStemsBeamVLinkTransactionError> {
    let expected_mutation = attempt.stored_theoretical_line_would_mutate;
    let pending = frontier.would_apply_stored_line_delta.as_ref();
    if expected_mutation
        != (attempt.stored_theoretical_line_before != attempt.stored_theoretical_line_after)
        || expected_mutation != pending.is_some()
    {
        return Err(NativeStemsBeamVLinkTransactionError::InvalidProduct {
            system_id: frontier.plan.system_id,
            phase: "pending line delta presence",
        });
    }
    if let Some(delta) = pending {
        if delta.delta_ordinal != expected_delta_ordinal
            || delta.invocation_ordinal != frontier.invocation_ordinal
            || delta.plan != frontier.plan
            || delta.v_linker != frontier.v_linker
            || delta.before != attempt.stored_theoretical_line_before
            || delta.after != attempt.stored_theoretical_line_after
            || delta.builder_line_aliases != attempt.builder_line_aliases_stored_theoretical_line
            || delta.attachment_aliases != attempt.attachment_aliases_stored_theoretical_line
        {
            return Err(NativeStemsBeamVLinkTransactionError::InvalidProduct {
                system_id: frontier.plan.system_id,
                phase: "pending line delta payload",
            });
        }
    }
    Ok(pending.cloned())
}

fn materialize_candidate(
    attempt: &NativeStemsBeamLinkPlanAttempt,
    state: &NativeStemsBeamVLinkTransactionState,
) -> Result<MaterializedCandidate, NativeStemsBeamVLinkTransactionError> {
    let mut selected = Vec::with_capacity(attempt.glyphs.len());
    for glyph in &attempt.glyphs {
        let binding = state
            .selected_glyph_bindings
            .iter()
            .find(|binding| binding.reference == glyph.reference)
            .ok_or(NativeStemsBeamVLinkTransactionError::MissingSelectedGlyph {
                reference: glyph.reference,
            })?;
        if binding.content.bounds != glyph.bounds
            || binding.content.weight != glyph.weight
            || binding.content.bounds.x != glyph.structural_key.left
            || binding.content.bounds.y != glyph.structural_key.top
            || binding.content.run_table != glyph.structural_key.run_table
        {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "selected plan glyph binding",
            });
        }
        selected.push(binding);
    }
    if selected.len() == 1 {
        let identity = (selected[0].glyph_id, selected[0].canonical_alias);
        return Ok(MaterializedCandidate {
            content: selected[0].content.clone(),
            singleton_identity: Some(identity),
            selected_canonical_identity: Some(identity),
        });
    }
    let candidate = build_compound(&selected)?;
    let selected_equal = selected
        .iter()
        .filter(|binding| binding.content == candidate)
        .map(|binding| (binding.glyph_id, binding.canonical_alias))
        .collect::<Vec<_>>();
    if selected_equal.len() > 1 {
        return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
            phase: "duplicate selected canonical content",
        });
    }
    Ok(MaterializedCandidate {
        content: candidate,
        singleton_identity: None,
        selected_canonical_identity: selected_equal.first().copied(),
    })
}

pub(crate) fn build_compound(
    selected: &[&NativeStemsBeamSelectedGlyphBinding],
) -> Result<NativeStemsBeamFixedGlyphContent, NativeStemsBeamVLinkTransactionError> {
    let first = selected
        .first()
        .ok_or(NativeStemsBeamVLinkTransactionError::Geometry)?;
    let mut minimum_x = first.content.bounds.x;
    let mut minimum_y = first.content.bounds.y;
    let mut maximum_x = first
        .content
        .bounds
        .x
        .checked_add(first.content.bounds.width)
        .ok_or(NativeStemsBeamVLinkTransactionError::Geometry)?;
    let mut maximum_y = first
        .content
        .bounds
        .y
        .checked_add(first.content.bounds.height)
        .ok_or(NativeStemsBeamVLinkTransactionError::Geometry)?;
    for glyph in &selected[1..] {
        minimum_x = minimum_x.min(glyph.content.bounds.x);
        minimum_y = minimum_y.min(glyph.content.bounds.y);
        maximum_x = maximum_x.max(
            glyph
                .content
                .bounds
                .x
                .checked_add(glyph.content.bounds.width)
                .ok_or(NativeStemsBeamVLinkTransactionError::Geometry)?,
        );
        maximum_y = maximum_y.max(
            glyph
                .content
                .bounds
                .y
                .checked_add(glyph.content.bounds.height)
                .ok_or(NativeStemsBeamVLinkTransactionError::Geometry)?,
        );
    }
    let bounds = Bounds {
        x: minimum_x,
        y: minimum_y,
        width: maximum_x
            .checked_sub(minimum_x)
            .ok_or(NativeStemsBeamVLinkTransactionError::Geometry)?,
        height: maximum_y
            .checked_sub(minimum_y)
            .ok_or(NativeStemsBeamVLinkTransactionError::Geometry)?,
    };
    let mut pixels = vec![
        BACKGROUND;
        bounds
            .width
            .checked_mul(bounds.height)
            .ok_or(NativeStemsBeamVLinkTransactionError::Geometry)?
    ];
    for glyph in selected {
        for sequence in 0..glyph.content.run_table.sequence_count() {
            for run in glyph
                .content
                .run_table
                .sequence(sequence)
                .unwrap_or_default()
            {
                for coordinate in run.start..=run.stop() {
                    let (local_x, local_y) = match glyph.content.run_table.orientation() {
                        Orientation::Horizontal => (coordinate, sequence),
                        Orientation::Vertical => (sequence, coordinate),
                    };
                    let x = glyph.content.bounds.x - bounds.x + local_x;
                    let y = glyph.content.bounds.y - bounds.y + local_y;
                    pixels[y * bounds.width + x] = FOREGROUND;
                }
            }
        }
    }
    let run_table =
        RunTable::from_pixels(Orientation::Vertical, bounds.width, bounds.height, &pixels)?;
    let content = NativeStemsBeamFixedGlyphContent {
        bounds,
        weight: run_table.weight(),
        run_table,
    };
    validate_content(&content)?;
    Ok(content)
}

fn prepare_registration(
    candidate: &NativeStemsBeamFixedGlyphContent,
    selected_canonical_identity: Option<(i32, usize)>,
    state: &NativeStemsBeamVLinkTransactionState,
) -> Result<PreparedRegistration, NativeStemsBeamVLinkTransactionError> {
    let known_matches = state
        .glyph_index
        .known_canonical_glyphs
        .iter()
        .enumerate()
        .filter(|(_, glyph)| glyph.content == *candidate)
        .collect::<Vec<_>>();
    if known_matches.len() > 1 {
        return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
            phase: "duplicate known canonical content",
        });
    }
    if let Some((index, glyph)) = known_matches.first().copied() {
        if !glyph.strongly_retained && selected_canonical_identity.is_none() {
            return Err(NativeStemsBeamVLinkTransactionError::AwaitingCompleteGlyphRegistry);
        }
        if selected_canonical_identity
            .is_some_and(|(id, alias)| id != glyph.glyph_id || alias != glyph.canonical_alias)
        {
            return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "selected canonical identity",
            });
        }
        return Ok(PreparedRegistration {
            canonical_alias: glyph.canonical_alias,
            glyph_id: glyph.glyph_id,
            action: NativeStemsBeamGlyphRegistrationAction::Reused {
                reinserted_into_active_index: !glyph.active_in_index,
            },
            post_union_size: state.glyph_index.union_size,
            consume_certificate: false,
            known_index: Some(index),
            strong_reference_after_rejection: glyph.strongly_retained
                || selected_canonical_identity.is_some(),
        });
    }

    let scan = state
        .glyph_index
        .exhaustive_lookup
        .as_ref()
        .ok_or(NativeStemsBeamVLinkTransactionError::AwaitingCompleteGlyphRegistry)?;
    if scan.candidate != *candidate {
        return Err(NativeStemsBeamVLinkTransactionError::AwaitingCompleteGlyphRegistry);
    }
    match scan.lookup {
        NativeStemsBeamExhaustiveGlyphLookup::Present {
            canonical_alias,
            glyph_id,
            active_in_index,
        } => {
            if selected_canonical_identity
                .is_some_and(|(id, alias)| id != glyph_id || alias != canonical_alias)
            {
                return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                    phase: "selected exhaustive canonical identity",
                });
            }
            Ok(PreparedRegistration {
                canonical_alias,
                glyph_id,
                action: NativeStemsBeamGlyphRegistrationAction::Reused {
                    reinserted_into_active_index: !active_in_index,
                },
                post_union_size: state.glyph_index.union_size,
                consume_certificate: true,
                known_index: None,
                strong_reference_after_rejection: selected_canonical_identity.is_some(),
            })
        }
        NativeStemsBeamExhaustiveGlyphLookup::Absent => {
            if selected_canonical_identity.is_some() {
                return Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                    phase: "registered selected glyph absent from originals",
                });
            }
            let glyph_id = state
                .glyph_index
                .persistent_ids
                .sheet_last_id
                .checked_add(1)
                .ok_or(NativeStemsBeamVLinkTransactionError::PersistentIdExhausted)?;
            Ok(PreparedRegistration {
                canonical_alias: usize::try_from(glyph_id)
                    .map_err(|_| NativeStemsBeamVLinkTransactionError::PersistentIdExhausted)?,
                glyph_id,
                action: NativeStemsBeamGlyphRegistrationAction::Registered,
                post_union_size: state
                    .glyph_index
                    .union_size
                    .checked_add(1)
                    .ok_or(NativeStemsBeamVLinkTransactionError::PersistentIdExhausted)?,
                consume_certificate: true,
                known_index: None,
                strong_reference_after_rejection: false,
            })
        }
    }
}

fn prepare_system_stem_lookup(
    candidate: &NativeStemsBeamFixedGlyphContent,
    canonical_glyph_id: i32,
    state: &NativeStemsBeamSystemStemTransactionState,
) -> Result<PreparedSystemStemLookup, NativeStemsBeamVLinkTransactionError> {
    let known = state
        .known_stems
        .iter()
        .filter(|stem| stem.glyph_content == *candidate)
        .collect::<Vec<_>>();
    if known.len() > 1 {
        return Err(NativeStemsBeamVLinkTransactionError::SystemStemInvariant {
            phase: "duplicate known structural key",
        });
    }
    if let Some(stem) = known.first() {
        if stem.glyph_id != canonical_glyph_id {
            return Err(NativeStemsBeamVLinkTransactionError::SystemStemInvariant {
                phase: "canonical glyph identity",
            });
        }
        return Ok(PreparedSystemStemLookup {
            stem: Some((*stem).clone()),
            consume_certificate: false,
        });
    }
    if state.authority == NativeStemsBeamRegistryAuthority::CompleteSinceEmptyBaseline {
        // A miss in a registry that has carried everything since empty is
        // absence, and Java's scan would only confirm it.
        return Ok(PreparedSystemStemLookup {
            stem: None,
            consume_certificate: state.exhaustive_lookup.is_some(),
        });
    }
    let scan = state
        .exhaustive_lookup
        .as_ref()
        .ok_or(NativeStemsBeamVLinkTransactionError::AwaitingCompleteSystemStems)?;
    if scan.candidate != *candidate {
        return Err(NativeStemsBeamVLinkTransactionError::AwaitingCompleteSystemStems);
    }
    match &scan.lookup {
        NativeStemsBeamExhaustiveSystemStemLookup::Absent => Ok(PreparedSystemStemLookup {
            stem: None,
            consume_certificate: true,
        }),
        NativeStemsBeamExhaustiveSystemStemLookup::Present(stem) => {
            if stem.glyph_id != canonical_glyph_id {
                return Err(NativeStemsBeamVLinkTransactionError::SystemStemInvariant {
                    phase: "exhaustive canonical glyph identity",
                });
            }
            Ok(PreparedSystemStemLookup {
                stem: Some(stem.as_ref().clone()),
                consume_certificate: true,
            })
        }
    }
}

fn prepare_new_stem(
    candidate: &NativeStemsBeamFixedGlyphContent,
    glyph_id: i32,
    stem_profile: i32,
    stem_identity: usize,
    checker: &NativeStemsBeamStemCheckerContext,
) -> Result<PreparedCreateStemResult, NativeStemsBeamVLinkTransactionError> {
    let near_line = fixed_glyph_near_line(candidate)?;
    let check = check_native_stem(
        &checker.no_staff,
        near_line,
        usize::try_from(glyph_id).map_err(|_| NativeStemsBeamVLinkTransactionError::Geometry)?,
        checker.parameters,
        stem_profile,
    )?;
    let grade = if check.grade >= checker.minimum_stem_grade {
        Some(NativeStemsBeamStemGrade::Checked(check))
    } else if stem_profile == 4 {
        Some(NativeStemsBeamStemGrade::Artificial(
            checker.artificial_stem_grade,
        ))
    } else {
        None
    };
    let Some(grade) = grade else {
        return Ok(PreparedCreateStemResult {
            checker_result: Some(check),
            disposition: NativeStemsBeamCreateStemDisposition::Rejected,
            stem: None,
            next_stem_identity: stem_identity,
        });
    };
    let next_stem_identity = stem_identity
        .checked_add(1)
        .ok_or(NativeStemsBeamVLinkTransactionError::StemIdentityExhausted)?;
    let geometry = created_stem_geometry(candidate)?;
    let checked = matches!(grade, NativeStemsBeamStemGrade::Checked(_));
    let stem = NativeStemsBeamKnownSystemStem {
        stem_identity,
        glyph_id,
        glyph_content: candidate.clone(),
        inter_id: None,
        grade,
        geometry,
        sig_attached: false,
        abnormal: false,
    };
    Ok(PreparedCreateStemResult {
        checker_result: Some(check),
        disposition: if checked {
            NativeStemsBeamCreateStemDisposition::CreatedChecked { stem_identity }
        } else {
            NativeStemsBeamCreateStemDisposition::CreatedArtificial { stem_identity }
        },
        stem: Some(stem),
        next_stem_identity,
    })
}

fn fixed_glyph_near_line(
    content: &NativeStemsBeamFixedGlyphContent,
) -> Result<NeutralStemGeometry, NativeStemsBeamVLinkTransactionError> {
    let mut line = BasicLine::default();
    let horizontal = content.run_table.orientation() == Orientation::Horizontal;
    let left = i32::try_from(content.bounds.x)
        .map_err(|_| NativeStemsBeamVLinkTransactionError::Geometry)?;
    let top = i32::try_from(content.bounds.y)
        .map_err(|_| NativeStemsBeamVLinkTransactionError::Geometry)?;
    for sequence in 0..content.run_table.sequence_count() {
        for run in content.run_table.sequence(sequence).unwrap_or_default() {
            for coordinate in (run.start..=run.stop()).rev() {
                if horizontal {
                    line.include_point(
                        f64::from(left) + coordinate as f64,
                        f64::from(top) + sequence as f64,
                    );
                } else {
                    line.include_point(
                        f64::from(left) + sequence as f64,
                        f64::from(top) + coordinate as f64,
                    );
                }
            }
        }
    }
    let (x_min, x_max, y_min, y_max) = line
        .extents()
        .ok_or(NativeStemsBeamVLinkTransactionError::Geometry)?;
    let coefficients = line
        .coefficients()
        .map_err(|_| NativeStemsBeamVLinkTransactionError::Geometry)?;
    let raw = if coefficients.rather_vertical {
        Segment {
            x1: line
                .x_at_y(y_min)
                .map_err(|_| NativeStemsBeamVLinkTransactionError::Geometry)?,
            y1: y_min,
            x2: line
                .x_at_y(y_max)
                .map_err(|_| NativeStemsBeamVLinkTransactionError::Geometry)?,
            y2: y_max,
        }
    } else {
        Segment {
            x1: x_min,
            y1: line
                .y_at_x(x_min)
                .map_err(|_| NativeStemsBeamVLinkTransactionError::Geometry)?,
            x2: x_max,
            y2: line
                .y_at_x(x_max)
                .map_err(|_| NativeStemsBeamVLinkTransactionError::Geometry)?,
        }
    };
    let (start_x, start_y, stop_x, stop_y) = if raw.y1 <= raw.y2 {
        (raw.x1, raw.y1, raw.x2, raw.y2)
    } else {
        (raw.x2, raw.y2, raw.x1, raw.y1)
    };
    Ok(NeutralStemGeometry {
        start_x,
        start_y,
        stop_x,
        stop_y,
        mean_distance: line
            .mean_distance()
            .map_err(|_| NativeStemsBeamVLinkTransactionError::Geometry)?,
        bounds_x: left,
        bounds_y: top,
        bounds_width: i32::try_from(content.bounds.width)
            .map_err(|_| NativeStemsBeamVLinkTransactionError::Geometry)?,
        bounds_height: i32::try_from(content.bounds.height)
            .map_err(|_| NativeStemsBeamVLinkTransactionError::Geometry)?,
    })
}

fn created_stem_geometry(
    content: &NativeStemsBeamFixedGlyphContent,
) -> Result<NativeStemsBeamCreatedStemGeometry, NativeStemsBeamVLinkTransactionError> {
    let left = i32::try_from(content.bounds.x)
        .map_err(|_| NativeStemsBeamVLinkTransactionError::Geometry)?;
    let top = i32::try_from(content.bounds.y)
        .map_err(|_| NativeStemsBeamVLinkTransactionError::Geometry)?;
    let median = run_table_center_line(&content.run_table, left, top)
        .ok_or(NativeStemsBeamVLinkTransactionError::Geometry)?;
    let mean_thickness = content.weight as f64 / content.bounds.height as f64;
    Ok(NativeStemsBeamCreatedStemGeometry {
        median: stem_line(median),
        mean_thickness,
        ribbon_bounds: VerticalRibbonArea::new(median, mean_thickness).integer_bounds(),
    })
}

fn stem_line(line: Segment) -> NativeStemLine {
    NativeStemLine {
        start: NativeStemPoint {
            x: line.x1,
            y: line.y1,
        },
        stop: NativeStemPoint {
            x: line.x2,
            y: line.y2,
        },
    }
}

fn apply_line_delta(
    state: &mut NativeStemsBeamVLinkTransactionState,
    delta: &NativeStemsBeamDeferredLineDelta,
) {
    let line = state
        .line_states
        .iter_mut()
        .find(|line| line.v_linker == delta.v_linker)
        .expect("line delta was validated before commit");
    line.stored_theoretical_line = delta.after;
    if delta.builder_line_aliases {
        line.builder_line = delta.after;
    }
    if delta.attachment_aliases {
        line.current_attachment_line = Some(delta.after);
    }
}

fn commit_registration(
    state: &mut NativeStemsBeamVLinkTransactionState,
    candidate: &NativeStemsBeamFixedGlyphContent,
    registration: &PreparedRegistration,
    disposition: &NativeStemsBeamCreateStemDisposition,
    mutations: &mut Vec<NativeStemsBeamVLinkMutation>,
) {
    match registration.action {
        NativeStemsBeamGlyphRegistrationAction::Reused {
            reinserted_into_active_index,
        } => {
            let strongly_retained =
                !matches!(disposition, NativeStemsBeamCreateStemDisposition::Rejected)
                    || registration.strong_reference_after_rejection;
            if let Some(index) = registration.known_index {
                let known = &mut state.glyph_index.known_canonical_glyphs[index];
                known.active_in_index = true;
                known.strongly_retained = strongly_retained;
            } else {
                state
                    .glyph_index
                    .known_canonical_glyphs
                    .push(NativeStemsBeamKnownCanonicalGlyph {
                        canonical_alias: registration.canonical_alias,
                        glyph_id: registration.glyph_id,
                        content: candidate.clone(),
                        active_in_index: true,
                        strongly_retained,
                    });
            }
            if reinserted_into_active_index {
                mutations.push(NativeStemsBeamVLinkMutation::GlyphReinserted {
                    glyph_id: registration.glyph_id,
                });
            }
            if !strongly_retained {
                mutations.push(
                    NativeStemsBeamVLinkMutation::RegistryWeakLivenessBecameUnknown {
                        glyph_id: registration.glyph_id,
                    },
                );
            }
        }
        NativeStemsBeamGlyphRegistrationAction::Registered => {
            let ids = &mut state.glyph_index.persistent_ids;
            ids.sheet_last_id = registration.glyph_id;
            ids.glyph_index_last_id = registration.glyph_id;
            ids.inter_index_last_id = registration.glyph_id;
            state.glyph_index.union_size = registration.post_union_size;
            let strongly_retained =
                !matches!(disposition, NativeStemsBeamCreateStemDisposition::Rejected);
            state
                .glyph_index
                .known_canonical_glyphs
                .push(NativeStemsBeamKnownCanonicalGlyph {
                    canonical_alias: registration.canonical_alias,
                    glyph_id: registration.glyph_id,
                    content: candidate.clone(),
                    active_in_index: true,
                    strongly_retained,
                });
            mutations.push(NativeStemsBeamVLinkMutation::GlyphRegistered {
                glyph_id: registration.glyph_id,
            });
            if !strongly_retained {
                mutations.push(
                    NativeStemsBeamVLinkMutation::RegistryWeakLivenessBecameUnknown {
                        glyph_id: registration.glyph_id,
                    },
                );
            }
        }
    }
    if registration.consume_certificate {
        state.glyph_index.exhaustive_lookup = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        native_stems_beam_stumps::NativeStemsBeamSource,
        native_stems_beam_vlinkers::NativeStemsBeamBLinkerRef,
        stem_seeds_step::{NativeStemCounts, NativeStemImpacts},
        stems_step::NativeStemVerticalSide,
    };

    fn content(
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        foreground: &[(usize, usize)],
    ) -> NativeStemsBeamFixedGlyphContent {
        let mut pixels = vec![BACKGROUND; width * height];
        for &(px, py) in foreground {
            pixels[py * width + px] = FOREGROUND;
        }
        let run_table =
            RunTable::from_pixels(Orientation::Vertical, width, height, &pixels).unwrap();
        NativeStemsBeamFixedGlyphContent {
            bounds: Bounds {
                x,
                y,
                width,
                height,
            },
            weight: run_table.weight(),
            run_table,
        }
    }

    fn vertical_content() -> NativeStemsBeamFixedGlyphContent {
        content(
            2,
            1,
            1,
            6,
            &[(0, 0), (0, 1), (0, 2), (0, 3), (0, 4), (0, 5)],
        )
    }

    fn stem_state(
        authority: NativeStemsBeamRegistryAuthority,
        known: Vec<NativeStemsBeamKnownSystemStem>,
    ) -> NativeStemsBeamSystemStemTransactionState {
        NativeStemsBeamSystemStemTransactionState {
            system_id: 1,
            next_stem_identity: known.len(),
            known_stems: known,
            authority,
            exhaustive_lookup: None,
        }
    }

    fn known_stem(
        content: NativeStemsBeamFixedGlyphContent,
        glyph_id: i32,
    ) -> NativeStemsBeamKnownSystemStem {
        let bounds = content.bounds;
        NativeStemsBeamKnownSystemStem {
            stem_identity: 0,
            glyph_id,
            glyph_content: content,
            inter_id: None,
            grade: NativeStemsBeamStemGrade::Artificial(0.5),
            geometry: NativeStemsBeamCreatedStemGeometry {
                median: NativeStemLine {
                    start: NativeStemPoint { x: 0.0, y: 0.0 },
                    stop: NativeStemPoint { x: 0.0, y: 6.0 },
                },
                mean_thickness: 1.0,
                ribbon_bounds: JavaRectangle {
                    x: i32::try_from(bounds.x).unwrap(),
                    y: i32::try_from(bounds.y).unwrap(),
                    width: i32::try_from(bounds.width).unwrap(),
                    height: i32::try_from(bounds.height).unwrap(),
                },
            },
            sig_attached: false,
            abnormal: false,
        }
    }

    /// Absence in a complete registry is a proof; absence in a partial one is
    /// only a question, and the question still demands Java's answer.
    #[test]
    fn only_a_complete_registry_may_answer_a_miss_without_a_scan() {
        let candidate = vertical_content();
        let resolved = prepare_system_stem_lookup(
            &candidate,
            7,
            &stem_state(
                NativeStemsBeamRegistryAuthority::CompleteSinceEmptyBaseline,
                Vec::new(),
            ),
        )
        .expect("a complete registry resolves its own miss");
        assert!(
            resolved.stem.is_none(),
            "a miss in a complete registry is absence"
        );
        assert!(
            matches!(
                prepare_system_stem_lookup(
                    &candidate,
                    7,
                    &stem_state(
                        NativeStemsBeamRegistryAuthority::RequiresExhaustiveScan,
                        Vec::new(),
                    ),
                ),
                Err(NativeStemsBeamVLinkTransactionError::AwaitingCompleteSystemStems)
            ),
            "a partial registry must not invent absence"
        );
    }

    /// Authority changes what a *miss* means, never what a *hit* means: a
    /// carried entry still decides, and still has to agree with the canonical
    /// glyph it claims.
    #[test]
    fn authority_does_not_loosen_a_carried_hit() {
        let candidate = vertical_content();
        for authority in [
            NativeStemsBeamRegistryAuthority::CompleteSinceEmptyBaseline,
            NativeStemsBeamRegistryAuthority::RequiresExhaustiveScan,
        ] {
            let known = vec![known_stem(candidate.clone(), 7)];
            let hit =
                prepare_system_stem_lookup(&candidate, 7, &stem_state(authority, known.clone()))
                    .expect("a carried entry decides under either authority");
            assert!(hit.stem.is_some(), "carried hit lost under {authority:?}");
            assert!(
                matches!(
                    prepare_system_stem_lookup(&candidate, 99, &stem_state(authority, known)),
                    Err(NativeStemsBeamVLinkTransactionError::SystemStemInvariant {
                        phase: "canonical glyph identity",
                    })
                ),
                "identity check skipped under {authority:?}"
            );
        }
    }

    fn absent_glyph_scan(
        candidate: NativeStemsBeamFixedGlyphContent,
    ) -> NativeStemsBeamExhaustiveGlyphEqualsScan {
        NativeStemsBeamExhaustiveGlyphEqualsScan {
            candidate,
            alias_order: NativeStemsBeamGlyphAliasOrder::JavaGlyphId,
            baseline_union_size: 9,
            baseline_active_count: 8,
            baseline_live_original_count: 8,
            baseline_active_sha256: "0".repeat(64),
            baseline_live_original_sha256: "1".repeat(64),
            scanned_active_count: 8,
            scanned_live_original_count: 8,
            equal_active_matches: 0,
            equal_original_matches: 0,
            lookup: NativeStemsBeamExhaustiveGlyphLookup::Absent,
        }
    }

    fn empty_state(
        scan: NativeStemsBeamExhaustiveGlyphEqualsScan,
    ) -> NativeStemsBeamVLinkTransactionState {
        NativeStemsBeamVLinkTransactionState {
            scope: NativeStemsBeamVLinkTransactionScope::SharedSheetFirstFrontier { system_id: 1 },
            glyph_index: NativeStemsBeamGlyphIndexTransactionState {
                persistent_ids: NativeStemsBeamPersistentIdState {
                    sheet_last_id: 20,
                    glyph_index_last_id: 20,
                    inter_index_last_id: 20,
                },
                alias_order: NativeStemsBeamGlyphAliasOrder::JavaGlyphId,
                union_size: 9,
                known_canonical_glyphs: Vec::new(),
                exhaustive_lookup: Some(scan),
            },
            selected_glyph_bindings: Vec::new(),
            line_states: Vec::new(),
            applied_line_deltas: Vec::new(),
            system_stems: NativeStemsBeamSystemStemTransactionState {
                system_id: 1,
                next_stem_identity: 0,
                // Hydrated from Java rows: keep the scan requirement.
                authority: NativeStemsBeamRegistryAuthority::RequiresExhaustiveScan,
                known_stems: Vec::new(),
                exhaustive_lookup: None,
            },
        }
    }

    #[test]
    fn modeled_registry_cleanup_is_ordered_idempotent_and_rejects_unknown_keep_ids() {
        let first = content(2, 1, 1, 2, &[(0, 0), (0, 1)]);
        let second = content(4, 1, 1, 2, &[(0, 0), (0, 1)]);
        let modeled = [first, second]
            .into_iter()
            .enumerate()
            .map(|(ordinal, content)| NativeStemsModeledCanonicalGlyph {
                modeled_canonical_ordinal: ordinal,
                bounds: content.bounds,
                weight: content.weight,
                run_table: content.run_table,
            })
            .collect::<Vec<_>>();
        let mut registry =
            NativeStemsModeledGlyphRegistry::from_modeled_prefix(1, &modeled, modeled.len())
                .expect("modeled registry");
        let mut state = NativeStemsBeamVLinkTransactionState {
            scope: NativeStemsBeamVLinkTransactionScope::SharedSheetFirstFrontier { system_id: 1 },
            glyph_index: NativeStemsBeamGlyphIndexTransactionState {
                persistent_ids: NativeStemsBeamPersistentIdState {
                    sheet_last_id: 2,
                    glyph_index_last_id: 2,
                    inter_index_last_id: 2,
                },
                alias_order: NativeStemsBeamGlyphAliasOrder::NativeModeledOrdinal,
                union_size: 2,
                known_canonical_glyphs: Vec::new(),
                exhaustive_lookup: None,
            },
            selected_glyph_bindings: Vec::new(),
            line_states: Vec::new(),
            applied_line_deltas: Vec::new(),
            system_stems: NativeStemsBeamSystemStemTransactionState {
                system_id: 1,
                next_stem_identity: 0,
                known_stems: Vec::new(),
                authority: NativeStemsBeamRegistryAuthority::CompleteSinceEmptyBaseline,
                exhaustive_lookup: None,
            },
        };
        assert_eq!(
            registry
                .retain_active_glyph_ids(&mut state, &BTreeSet::from([2]))
                .expect("first cleanup"),
            (vec![2], vec![1])
        );
        assert_eq!(
            registry
                .retain_active_glyph_ids(&mut state, &BTreeSet::from([2]))
                .expect("repeated cleanup"),
            (vec![2], Vec::new())
        );
        let before_registry = registry.clone();
        let before_state = state.clone();
        assert!(
            registry
                .retain_active_glyph_ids(&mut state, &BTreeSet::from([3]))
                .is_err()
        );
        assert_eq!(registry, before_registry);
        assert_eq!(state, before_state);
    }

    fn reference() -> NativeStemsBeamVLinkerRef {
        reference_for(0)
    }

    fn reference_for(raw_beam: usize) -> NativeStemsBeamVLinkerRef {
        NativeStemsBeamVLinkerRef {
            b_linker: NativeStemsBeamBLinkerRef {
                beam: NativeStemsBeamSource::RawBeam(raw_beam),
                id: 1,
            },
            side: NativeStemVerticalSide::Bottom,
        }
    }

    fn ready_attempt(line: NativeStemLine) -> NativeStemsBeamLinkPlanAttempt {
        NativeStemsBeamLinkPlanAttempt {
            stem_profile: 4,
            link_profile: 1,
            head_target_count: 1,
            initial_stem_line: line,
            trace: Vec::new(),
            relations: Vec::new(),
            glyphs: Vec::new(),
            expand_last_index: Some(0),
            stopping_head_item_index: None,
            stop_cause: None,
            outcome: NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem,
            relations_past_return_count: 0,
            rollback_line_diverges_from_restored_glyphs: false,
            beam_side_ready_without_stopping_head: false,
            beam_side_ready_beyond_stopping_head: false,
            beam_side_ready_at_stopping_head: true,
            final_stem_line: line,
            stored_theoretical_line_before: line,
            stored_theoretical_line_after: line,
            stored_theoretical_line_would_mutate: false,
            builder_line_aliases_stored_theoretical_line: true,
            attachment_aliases_stored_theoretical_line: true,
            attachment_alias_would_mutate: false,
            registry_mutation_count: 0,
            sig_mutation_count: 0,
            system_stem_mutation_count: 0,
            link_flag_mutation_count: 0,
        }
    }

    fn first_stems_bridge_inputs() -> (
        NativeStemsFirstGlyphIndexSnapshot,
        Vec<NativeStemsModeledCanonicalGlyph>,
    ) {
        let template = vertical_content();
        let digest = run_table_sha256(&template.run_table);
        let modeled = (0..=FIRST_STEMS_VISIBLE_MODELED_COUNT)
            .map(|ordinal| NativeStemsModeledCanonicalGlyph {
                modeled_canonical_ordinal: ordinal,
                bounds: Bounds {
                    x: ordinal + 1,
                    ..template.bounds
                },
                weight: template.weight,
                run_table: template.run_table.clone(),
            })
            .collect::<Vec<_>>();
        let entries = (0..FIRST_STEMS_GLYPH_UNION_SIZE)
            .map(|index| NativeStemsFirstGlyphSnapshotEntry {
                glyph_id: if index + 1 == FIRST_STEMS_GLYPH_UNION_SIZE {
                    FIRST_STEMS_LAST_PERSISTENT_ID
                } else {
                    i32::try_from(index + 1).unwrap()
                },
                active_in_index: true,
                live_original: true,
                fingerprint: NativeStemsFirstGlyphFingerprint {
                    bounds: Bounds {
                        x: index + 1,
                        ..template.bounds
                    },
                    run_table_sha256: if index < FIRST_STEMS_VISIBLE_MODELED_COUNT {
                        digest.clone()
                    } else {
                        sha256_hex(format!("opaque:{index}").as_bytes())
                    },
                },
                modeled_canonical_ordinal: (index < FIRST_STEMS_VISIBLE_MODELED_COUNT)
                    .then_some(index),
            })
            .collect();
        let mut snapshot = NativeStemsFirstGlyphIndexSnapshot {
            system_id: 1,
            persistent_ids: NativeStemsBeamPersistentIdState {
                sheet_last_id: FIRST_STEMS_LAST_PERSISTENT_ID,
                glyph_index_last_id: FIRST_STEMS_LAST_PERSISTENT_ID,
                inter_index_last_id: FIRST_STEMS_LAST_PERSISTENT_ID,
            },
            union_size: FIRST_STEMS_GLYPH_UNION_SIZE,
            active_count: FIRST_STEMS_GLYPH_UNION_SIZE,
            live_original_count: FIRST_STEMS_GLYPH_UNION_SIZE,
            active_sha256: String::new(),
            live_original_sha256: String::new(),
            visible_modeled_count: FIRST_STEMS_VISIBLE_MODELED_COUNT,
            entries,
        };
        snapshot.active_sha256 = first_stems_snapshot_sha256(&snapshot.entries, false);
        snapshot.live_original_sha256 = first_stems_snapshot_sha256(&snapshot.entries, true);
        (snapshot, modeled)
    }

    #[test]
    fn first_stems_bridge_rejects_counts_duplicates_and_future_cutoff() {
        let (snapshot, modeled) = first_stems_bridge_inputs();
        assert!(
            NativeStemsFirstGlyphIndexBridge::from_snapshot(snapshot.clone(), &modeled).is_ok()
        );

        let mut wrong_count = snapshot.clone();
        wrong_count.union_size -= 1;
        assert!(NativeStemsFirstGlyphIndexBridge::from_snapshot(wrong_count, &modeled).is_err());

        let mut duplicate = snapshot.clone();
        duplicate.entries[1].glyph_id = duplicate.entries[0].glyph_id;
        assert!(NativeStemsFirstGlyphIndexBridge::from_snapshot(duplicate, &modeled).is_err());

        let mut corrupt_hash = snapshot.clone();
        corrupt_hash.active_sha256 = "0".repeat(64);
        assert!(NativeStemsFirstGlyphIndexBridge::from_snapshot(corrupt_hash, &modeled).is_err());

        let mut future = snapshot;
        future.entries[FIRST_STEMS_VISIBLE_MODELED_COUNT].modeled_canonical_ordinal =
            Some(FIRST_STEMS_VISIBLE_MODELED_COUNT);
        assert!(NativeStemsFirstGlyphIndexBridge::from_snapshot(future, &modeled).is_err());
    }

    #[test]
    fn first_stems_bridge_selects_exact_modeled_identity() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let (snapshot, modeled) = first_stems_bridge_inputs();
        let bridge = NativeStemsFirstGlyphIndexBridge::from_snapshot(snapshot, &modeled).unwrap();
        let native = &modeled[7];
        let selected = NativeStemsBeamSelectedGlyph {
            reference: NativeStemsBeamBuilderGlyphRef::StemSeed {
                free_glyph_ordinal: 7,
            },
            bounds: native.bounds,
            weight: native.weight,
            structural_key: crate::native_stems_beam_link_plans::NativeStemsBeamLinkGlyphKey {
                left: native.bounds.x,
                top: native.bounds.y,
                run_table: native.run_table.clone(),
            },
            structural_digest: 0,
            modeled_canonical_ordinal: 7,
        };
        let binding = bridge.selected_bootstrap(&selected).unwrap();
        assert_eq!(binding.glyph_id, 8);
        assert_eq!(binding.canonical_alias, 8);
        assert_eq!(binding.content.bounds, native.bounds);

        let mut mismatched = selected;
        mismatched.bounds.x += 1;
        assert!(bridge.selected_bootstrap(&mismatched).is_err());
    }

    #[test]
    fn first_stems_bridge_resolves_only_an_exact_opaque_native_content() {
        let (mut snapshot, modeled) = first_stems_bridge_inputs();
        let opaque_index = FIRST_STEMS_VISIBLE_MODELED_COUNT;
        let mut native = vertical_content();
        native.bounds.x = opaque_index + 1;
        snapshot.entries[opaque_index].fingerprint.run_table_sha256 =
            run_table_sha256(&native.run_table);
        snapshot.active_sha256 = first_stems_snapshot_sha256(&snapshot.entries, false);
        snapshot.live_original_sha256 = first_stems_snapshot_sha256(&snapshot.entries, true);
        let expected_id = snapshot.entries[opaque_index].glyph_id;
        let bridge = NativeStemsFirstGlyphIndexBridge::from_snapshot(snapshot, &modeled).unwrap();

        let binding = bridge.resolve_native_content(&native).unwrap();
        assert_eq!(binding.glyph_id, expected_id);
        assert!(binding.active_in_index);
        assert!(binding.strongly_retained);

        native.bounds.x += 1;
        assert!(bridge.resolve_native_content(&native).is_err());
    }

    #[test]
    fn exhaustive_scan_requires_exact_coverage_and_active_consistency() {
        let candidate = vertical_content();
        let mut scan = absent_glyph_scan(candidate);
        assert!(
            validate_glyph_scan(&scan, 20, NativeStemsBeamGlyphAliasOrder::JavaGlyphId, 9).is_ok()
        );

        scan.baseline_active_sha256 = "A".repeat(64);
        assert!(
            validate_glyph_scan(&scan, 20, NativeStemsBeamGlyphAliasOrder::JavaGlyphId, 9).is_err()
        );
        scan.baseline_active_sha256 = "0".repeat(64);

        scan.lookup = NativeStemsBeamExhaustiveGlyphLookup::Present {
            canonical_alias: 7,
            glyph_id: 7,
            active_in_index: true,
        };
        scan.equal_original_matches = 1;
        assert!(
            validate_glyph_scan(&scan, 20, NativeStemsBeamGlyphAliasOrder::JavaGlyphId, 9).is_err()
        );
        scan.equal_active_matches = 1;
        assert!(
            validate_glyph_scan(&scan, 20, NativeStemsBeamGlyphAliasOrder::JavaGlyphId, 9).is_ok()
        );

        scan.lookup = NativeStemsBeamExhaustiveGlyphLookup::Absent;
        scan.equal_original_matches = 0;
        assert!(
            validate_glyph_scan(&scan, 20, NativeStemsBeamGlyphAliasOrder::JavaGlyphId, 9).is_err()
        );
    }

    #[test]
    fn fixed_content_rejects_i32_extent_overflow_before_checker_rows() {
        let mut candidate = vertical_content();
        candidate.bounds.y = i32::MAX as usize;
        assert!(matches!(
            validate_content(&candidate),
            Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "fixed glyph content"
            })
        ));

        candidate = vertical_content();
        candidate.bounds.x = i32::MAX as usize;
        assert!(matches!(
            validate_content(&candidate),
            Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "fixed glyph content"
            })
        ));
    }

    #[test]
    fn selected_bindings_require_canonical_identity_iff_exact_content() {
        let candidate = vertical_content();
        let mut state = empty_state(absent_glyph_scan(candidate.clone()));
        let first = NativeStemsBeamSelectedGlyphBinding {
            reference: NativeStemsBeamBuilderGlyphRef::StemSeed {
                free_glyph_ordinal: 0,
            },
            canonical_alias: 5,
            glyph_id: 5,
            content: candidate.clone(),
        };
        let same_object_other_reference = NativeStemsBeamSelectedGlyphBinding {
            reference: NativeStemsBeamBuilderGlyphRef::StemSeed {
                free_glyph_ordinal: 1,
            },
            ..first.clone()
        };
        state.selected_glyph_bindings = vec![first.clone(), same_object_other_reference];
        assert!(validate_transaction_state(&state).is_ok());

        let mut same_id_different_content = first.clone();
        same_id_different_content.reference = NativeStemsBeamBuilderGlyphRef::StemSeed {
            free_glyph_ordinal: 2,
        };
        same_id_different_content.content.bounds.x += 1;
        state.selected_glyph_bindings = vec![first.clone(), same_id_different_content];
        assert!(matches!(
            validate_transaction_state(&state),
            Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "selected glyph bindings"
            })
        ));

        let mut same_content_different_id = first.clone();
        same_content_different_id.reference = NativeStemsBeamBuilderGlyphRef::StemSeed {
            free_glyph_ordinal: 3,
        };
        same_content_different_id.glyph_id = 6;
        same_content_different_id.canonical_alias = 6;
        state.selected_glyph_bindings = vec![first, same_content_different_id];
        assert!(matches!(
            validate_transaction_state(&state),
            Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "selected glyph bindings"
            })
        ));

        state.glyph_index.exhaustive_lookup = None;
        state
            .glyph_index
            .known_canonical_glyphs
            .push(NativeStemsBeamKnownCanonicalGlyph {
                canonical_alias: 5,
                glyph_id: 5,
                content: candidate.clone(),
                active_in_index: true,
                strongly_retained: true,
            });
        let selected = NativeStemsBeamSelectedGlyphBinding {
            reference: NativeStemsBeamBuilderGlyphRef::StemSeed {
                free_glyph_ordinal: 4,
            },
            canonical_alias: 5,
            glyph_id: 5,
            content: candidate.clone(),
        };
        state.selected_glyph_bindings = vec![selected.clone()];
        assert!(validate_transaction_state(&state).is_ok());

        let mut known_id_different_content = selected.clone();
        known_id_different_content.content.bounds.x += 1;
        state.selected_glyph_bindings = vec![known_id_different_content];
        assert!(matches!(
            validate_transaction_state(&state),
            Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "selected glyph bindings"
            })
        ));

        let mut known_content_different_id = selected;
        known_content_different_id.glyph_id = 6;
        known_content_different_id.canonical_alias = 6;
        state.selected_glyph_bindings = vec![known_content_different_id];
        assert!(matches!(
            validate_transaction_state(&state),
            Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "selected glyph bindings"
            })
        ));
    }

    #[test]
    fn exhaustive_present_payloads_cannot_contradict_known_identities() {
        let candidate = vertical_content();
        let mut other = candidate.clone();
        other.bounds.x += 1;

        let mut glyph_scan = absent_glyph_scan(candidate.clone());
        glyph_scan.equal_active_matches = 1;
        glyph_scan.equal_original_matches = 1;
        glyph_scan.lookup = NativeStemsBeamExhaustiveGlyphLookup::Present {
            canonical_alias: 5,
            glyph_id: 5,
            active_in_index: true,
        };
        let mut state = empty_state(glyph_scan);
        state
            .glyph_index
            .known_canonical_glyphs
            .push(NativeStemsBeamKnownCanonicalGlyph {
                canonical_alias: 5,
                glyph_id: 5,
                content: other.clone(),
                active_in_index: true,
                strongly_retained: true,
            });
        assert!(matches!(
            validate_transaction_state(&state),
            Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "exhaustive/known canonical identity"
            })
        ));

        state.glyph_index.known_canonical_glyphs[0].content = candidate.clone();
        state.glyph_index.exhaustive_lookup.as_mut().unwrap().lookup =
            NativeStemsBeamExhaustiveGlyphLookup::Present {
                canonical_alias: 6,
                glyph_id: 6,
                active_in_index: true,
            };
        assert!(matches!(
            validate_transaction_state(&state),
            Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "exhaustive/known canonical identity"
            })
        ));

        state.glyph_index.exhaustive_lookup.as_mut().unwrap().lookup =
            NativeStemsBeamExhaustiveGlyphLookup::Present {
                canonical_alias: 5,
                glyph_id: 5,
                active_in_index: true,
            };
        assert!(validate_transaction_state(&state).is_ok());

        state.glyph_index.known_canonical_glyphs[0].active_in_index = false;
        assert!(matches!(
            validate_transaction_state(&state),
            Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "exhaustive/known canonical identity"
            })
        ));
        state.glyph_index.known_canonical_glyphs.clear();
        state
            .selected_glyph_bindings
            .push(NativeStemsBeamSelectedGlyphBinding {
                reference: NativeStemsBeamBuilderGlyphRef::StemSeed {
                    free_glyph_ordinal: 0,
                },
                canonical_alias: 5,
                glyph_id: 5,
                content: other.clone(),
            });
        assert!(matches!(
            validate_transaction_state(&state),
            Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "exhaustive/known canonical identity"
            })
        ));

        state.selected_glyph_bindings.clear();
        state.glyph_index.exhaustive_lookup = None;
        let known_stem = NativeStemsBeamKnownSystemStem {
            stem_identity: 0,
            glyph_id: 4,
            glyph_content: other,
            inter_id: None,
            grade: NativeStemsBeamStemGrade::Artificial(0.5),
            geometry: created_stem_geometry(&candidate).unwrap(),
            sig_attached: false,
            abnormal: false,
        };
        let scanned_stem = NativeStemsBeamKnownSystemStem {
            stem_identity: 0,
            glyph_id: 5,
            glyph_content: candidate.clone(),
            inter_id: None,
            grade: NativeStemsBeamStemGrade::Artificial(0.5),
            geometry: created_stem_geometry(&candidate).unwrap(),
            sig_attached: false,
            abnormal: false,
        };
        state.system_stems.next_stem_identity = 2;
        state.system_stems.known_stems = vec![known_stem];
        state.system_stems.exhaustive_lookup =
            Some(NativeStemsBeamExhaustiveSystemStemEqualsScan {
                candidate: candidate.clone(),
                baseline_entry_count: 1,
                baseline_sha256: "2".repeat(64),
                scanned_entry_count: 1,
                equal_glyph_matches: 1,
                lookup: NativeStemsBeamExhaustiveSystemStemLookup::Present(Box::new(
                    scanned_stem.clone(),
                )),
            });
        assert!(matches!(
            validate_transaction_state(&state),
            Err(NativeStemsBeamVLinkTransactionError::SystemStemInvariant {
                phase: "exhaustive systemStems equality result"
            })
        ));

        state.system_stems.known_stems[0] = NativeStemsBeamKnownSystemStem {
            stem_identity: 1,
            ..scanned_stem.clone()
        };
        assert!(matches!(
            validate_transaction_state(&state),
            Err(NativeStemsBeamVLinkTransactionError::SystemStemInvariant {
                phase: "exhaustive systemStems equality result"
            })
        ));

        state.system_stems.known_stems[0] = scanned_stem;
        assert!(validate_transaction_state(&state).is_ok());
    }

    #[test]
    fn represented_persistent_ids_preserve_java_identity_domains() {
        let candidate = vertical_content();
        let mut other = candidate.clone();
        other.bounds.x += 1;
        let make_stem = |stem_identity, glyph_id, glyph_content, inter_id| {
            let geometry = created_stem_geometry(&glyph_content).unwrap();
            NativeStemsBeamKnownSystemStem {
                stem_identity,
                glyph_id,
                glyph_content,
                inter_id,
                grade: NativeStemsBeamStemGrade::Artificial(0.5),
                geometry,
                sig_attached: inter_id.is_some(),
                abnormal: false,
            }
        };

        let mut state = empty_state(absent_glyph_scan(candidate.clone()));
        state.glyph_index.exhaustive_lookup = None;
        state
            .glyph_index
            .known_canonical_glyphs
            .push(NativeStemsBeamKnownCanonicalGlyph {
                canonical_alias: 5,
                glyph_id: 5,
                content: candidate.clone(),
                active_in_index: true,
                strongly_retained: true,
            });
        state
            .selected_glyph_bindings
            .push(NativeStemsBeamSelectedGlyphBinding {
                reference: NativeStemsBeamBuilderGlyphRef::StemSeed {
                    free_glyph_ordinal: 0,
                },
                canonical_alias: 5,
                glyph_id: 5,
                content: candidate.clone(),
            });
        state.system_stems.next_stem_identity = 1;
        state.system_stems.known_stems = vec![make_stem(0, 5, candidate.clone(), None)];
        assert!(validate_transaction_state(&state).is_ok());

        state.system_stems.known_stems[0].glyph_content = other.clone();
        assert!(matches!(
            validate_transaction_state(&state),
            Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "represented canonical glyph identity"
            })
        ));
        state.system_stems.known_stems[0].glyph_content = candidate.clone();
        state.system_stems.known_stems[0].glyph_id = 6;
        assert!(matches!(
            validate_transaction_state(&state),
            Err(NativeStemsBeamVLinkTransactionError::RegistryInvariant {
                phase: "represented canonical glyph identity"
            })
        ));

        state.system_stems.known_stems[0] = make_stem(0, 5, candidate.clone(), Some(5));
        assert!(matches!(
            validate_transaction_state(&state),
            Err(NativeStemsBeamVLinkTransactionError::SystemStemInvariant {
                phase: "represented persistent Inter identity"
            })
        ));
        state.system_stems.known_stems[0] = make_stem(0, 5, candidate.clone(), Some(8));
        assert!(validate_transaction_state(&state).is_ok());

        state.system_stems.next_stem_identity = 2;
        state
            .system_stems
            .known_stems
            .push(make_stem(1, 6, other, Some(8)));
        assert!(matches!(
            validate_transaction_state(&state),
            Err(NativeStemsBeamVLinkTransactionError::SystemStemInvariant {
                phase: "represented persistent Inter identity"
            })
        ));
        state.system_stems.known_stems[1].inter_id = Some(9);
        assert!(validate_transaction_state(&state).is_ok());

        let repeated = state.system_stems.known_stems[0].clone();
        state.system_stems.exhaustive_lookup =
            Some(NativeStemsBeamExhaustiveSystemStemEqualsScan {
                candidate,
                baseline_entry_count: 2,
                baseline_sha256: "3".repeat(64),
                scanned_entry_count: 2,
                equal_glyph_matches: 1,
                lookup: NativeStemsBeamExhaustiveSystemStemLookup::Present(Box::new(repeated)),
            });
        assert!(validate_transaction_state(&state).is_ok());
    }

    #[test]
    fn rejected_new_compound_commits_shared_id_and_weak_liveness() {
        let candidate = vertical_content();
        let mut state = empty_state(absent_glyph_scan(candidate.clone()));
        let prepared = prepare_registration(&candidate, None, &state).unwrap();
        assert_eq!(prepared.glyph_id, 21);
        assert_eq!(prepared.canonical_alias, 21);
        assert_eq!(prepared.post_union_size, 10);

        let mut mutations = Vec::new();
        commit_registration(
            &mut state,
            &candidate,
            &prepared,
            &NativeStemsBeamCreateStemDisposition::Rejected,
            &mut mutations,
        );
        assert_eq!(
            state.glyph_index.persistent_ids,
            NativeStemsBeamPersistentIdState {
                sheet_last_id: 21,
                glyph_index_last_id: 21,
                inter_index_last_id: 21,
            }
        );
        assert_eq!(state.glyph_index.union_size, 10);
        assert!(!state.glyph_index.known_canonical_glyphs[0].strongly_retained);
        assert!(state.glyph_index.exhaustive_lookup.is_none());
        assert_eq!(
            mutations,
            vec![
                NativeStemsBeamVLinkMutation::GlyphRegistered { glyph_id: 21 },
                NativeStemsBeamVLinkMutation::RegistryWeakLivenessBecameUnknown { glyph_id: 21 }
            ]
        );
        assert!(matches!(
            prepare_registration(&candidate, None, &state),
            Err(NativeStemsBeamVLinkTransactionError::AwaitingCompleteGlyphRegistry)
        ));

        // A later plan can itself strongly retain that exact canonical object.
        // This fresh object evidence restores liveness without another global
        // equality certificate.
        let retained = prepare_registration(&candidate, Some((21, 21)), &state).unwrap();
        let mut retained_mutations = Vec::new();
        commit_registration(
            &mut state,
            &candidate,
            &retained,
            &NativeStemsBeamCreateStemDisposition::Rejected,
            &mut retained_mutations,
        );
        assert!(state.glyph_index.known_canonical_glyphs[0].strongly_retained);
        assert!(retained_mutations.is_empty());
    }

    #[test]
    fn known_singleton_reuses_and_reinserts_without_allocating() {
        let candidate = vertical_content();
        let mut state = empty_state(absent_glyph_scan(candidate.clone()));
        state
            .glyph_index
            .known_canonical_glyphs
            .push(NativeStemsBeamKnownCanonicalGlyph {
                canonical_alias: 12,
                glyph_id: 12,
                content: candidate.clone(),
                active_in_index: false,
                strongly_retained: true,
            });
        let prepared = prepare_registration(&candidate, Some((12, 12)), &state).unwrap();
        assert_eq!(
            prepared.action,
            NativeStemsBeamGlyphRegistrationAction::Reused {
                reinserted_into_active_index: true
            }
        );
        let mut mutations = Vec::new();
        commit_registration(
            &mut state,
            &candidate,
            &prepared,
            &NativeStemsBeamCreateStemDisposition::CreatedChecked { stem_identity: 0 },
            &mut mutations,
        );
        assert_eq!(state.glyph_index.persistent_ids.sheet_last_id, 20);
        assert!(state.glyph_index.known_canonical_glyphs[0].active_in_index);
        assert_eq!(
            mutations,
            vec![NativeStemsBeamVLinkMutation::GlyphReinserted { glyph_id: 12 }]
        );
    }

    #[test]
    fn compound_is_rebuilt_vertically_with_exact_union_pixels() {
        let top = content(3, 2, 1, 2, &[(0, 0), (0, 1)]);
        let bottom = content(3, 5, 1, 2, &[(0, 0), (0, 1)]);
        let first = NativeStemsBeamSelectedGlyphBinding {
            reference: NativeStemsBeamBuilderGlyphRef::Chunk {
                builder_ordinal: 0,
                filament_ordinal: 0,
            },
            canonical_alias: 1,
            glyph_id: 1,
            content: top,
        };
        let second = NativeStemsBeamSelectedGlyphBinding {
            reference: NativeStemsBeamBuilderGlyphRef::Chunk {
                builder_ordinal: 0,
                filament_ordinal: 1,
            },
            canonical_alias: 2,
            glyph_id: 2,
            content: bottom,
        };
        let compound = build_compound(&[&first, &second]).unwrap();
        assert_eq!(
            compound.bounds,
            Bounds {
                x: 3,
                y: 2,
                width: 1,
                height: 5
            }
        );
        assert_eq!(compound.weight, 4);
        assert_eq!(compound.run_table.orientation(), Orientation::Vertical);
        assert_eq!(
            compound.run_table.foreground_points((3, 2)),
            vec![(3, 3), (3, 2), (3, 6), (3, 5)]
        );
    }

    #[test]
    fn checker_and_stem_geometry_use_java_pixel_regressions() {
        let candidate = vertical_content();
        let near_line = fixed_glyph_near_line(&candidate).unwrap();
        assert_eq!((near_line.start_y, near_line.stop_y), (1.0, 6.0));
        assert_eq!(near_line.mean_distance, 0.0);
        let geometry = created_stem_geometry(&candidate).unwrap();
        assert_eq!(geometry.mean_thickness, 1.0);
        assert_eq!(geometry.median.start.y, 1.0);
        assert_eq!(geometry.median.stop.y, 7.0);
        assert_eq!(geometry.ribbon_bounds, JavaRectangle::new(2, 1, 1, 6));

        let mut stem = NativeStemsBeamKnownSystemStem {
            stem_identity: 0,
            glyph_id: 5,
            glyph_content: candidate,
            inter_id: None,
            grade: NativeStemsBeamStemGrade::Artificial(0.5),
            geometry: geometry.clone(),
            sig_attached: false,
            abnormal: false,
        };
        assert!(validate_known_stem(&stem, 20).is_ok());
        stem.geometry.median.start.x = f64::NAN;
        assert!(validate_known_stem(&stem, 20).is_err());
        stem.geometry = geometry.clone();
        stem.geometry.ribbon_bounds.x += 1;
        assert!(validate_known_stem(&stem, 20).is_err());
        stem.geometry = geometry;
        stem.grade = NativeStemsBeamStemGrade::Artificial(f64::NAN);
        assert!(validate_known_stem(&stem, 20).is_err());

        let check = NativeStemCheckResult {
            values: NativeStemImpacts {
                slope: 0.0,
                straight: 0.0,
                length: 2.0,
                clean: 0.5,
                black: 1.0,
                black_ratio: 0.8,
                gap: 0.0,
            },
            impacts: NativeStemImpacts {
                slope: 1.0,
                straight: 1.0,
                length: 0.5,
                clean: 0.5,
                black: 0.5,
                black_ratio: 0.8,
                gap: 1.0,
            },
            counts: NativeStemCounts {
                largest_gap: 0,
                white: 1,
                black: 5,
                left: 1,
                right: 1,
                both: 1,
                clean: 2,
            },
            grade: 0.5,
        };
        stem.grade = NativeStemsBeamStemGrade::Checked(check);
        assert!(validate_known_stem(&stem, 20).is_ok());
        if let NativeStemsBeamStemGrade::Checked(check) = &mut stem.grade {
            check.impacts.slope = 1.1;
        }
        assert!(validate_known_stem(&stem, 20).is_err());
    }

    #[test]
    fn pending_line_delta_updates_every_declared_alias_once() {
        let before = NativeStemLine {
            start: NativeStemPoint { x: 4.0, y: 2.0 },
            stop: NativeStemPoint { x: 4.0, y: 8.0 },
        };
        let after = NativeStemLine {
            start: NativeStemPoint { x: 5.0, y: 2.0 },
            stop: NativeStemPoint { x: 5.0, y: 8.0 },
        };
        let candidate = vertical_content();
        let mut state = empty_state(absent_glyph_scan(candidate));
        state.line_states.push(NativeStemsBeamVLinkLineState {
            v_linker: reference(),
            stored_theoretical_line: before,
            builder_line: before,
            current_attachment_line: Some(before),
        });
        let delta = NativeStemsBeamDeferredLineDelta {
            delta_ordinal: 0,
            invocation_ordinal: 0,
            plan: NativeStemsBeamPlanRef {
                system_id: 1,
                plan_ordinal: 0,
                builder_ordinal: 0,
                stem_profile: 4,
            },
            v_linker: reference(),
            before,
            after,
            builder_line_aliases: true,
            attachment_aliases: true,
        };
        apply_line_delta(&mut state, &delta);
        let line = &state.line_states[0];
        assert_eq!(line.stored_theoretical_line, after);
        assert_eq!(line.builder_line, after);
        assert_eq!(line.current_attachment_line, Some(after));
    }

    #[test]
    fn scheduler_known_false_delta_is_committed_by_this_boundary_once() {
        let prior_before = NativeStemLine {
            start: NativeStemPoint { x: 3.0, y: 1.0 },
            stop: NativeStemPoint { x: 3.0, y: 7.0 },
        };
        let prior_after = NativeStemLine {
            start: NativeStemPoint { x: 4.0, y: 1.0 },
            stop: NativeStemPoint { x: 4.0, y: 7.0 },
        };
        let current_line = NativeStemLine {
            start: NativeStemPoint { x: 8.0, y: 1.0 },
            stop: NativeStemPoint { x: 8.0, y: 7.0 },
        };
        let second_before = NativeStemLine {
            start: NativeStemPoint { x: 5.0, y: 1.0 },
            stop: NativeStemPoint { x: 5.0, y: 7.0 },
        };
        let second_after = NativeStemLine {
            start: NativeStemPoint { x: 6.0, y: 1.0 },
            stop: NativeStemPoint { x: 6.0, y: 7.0 },
        };
        let prior_delta = NativeStemsBeamDeferredLineDelta {
            delta_ordinal: 0,
            invocation_ordinal: 0,
            plan: NativeStemsBeamPlanRef {
                system_id: 1,
                plan_ordinal: 0,
                builder_ordinal: 0,
                stem_profile: 4,
            },
            v_linker: reference_for(1),
            before: prior_before,
            after: prior_after,
            builder_line_aliases: true,
            attachment_aliases: true,
        };
        let second_delta = NativeStemsBeamDeferredLineDelta {
            delta_ordinal: 1,
            invocation_ordinal: 1,
            plan: NativeStemsBeamPlanRef {
                system_id: 1,
                plan_ordinal: 1,
                builder_ordinal: 0,
                stem_profile: 4,
            },
            v_linker: reference_for(2),
            before: second_before,
            after: second_after,
            builder_line_aliases: true,
            attachment_aliases: true,
        };
        let scheduler = NativeStemsBeamSchedulerSystem {
            system_id: 1,
            link_profile: 1,
            glyphs_in_sig_order: Vec::new(),
            live_exclusions: Vec::new(),
            beams_by_reverse_width: Vec::new(),
            prefix_events: Vec::new(),
            deferred_line_deltas: vec![prior_delta.clone(), second_delta.clone()],
            consumed_v_linkers: Vec::new(),
            linked_b_linkers: Vec::new(),
            status: NativeStemsBeamSchedulerStatus::Completed {
                retained_for_stumps: Vec::new(),
                final_local_worklist: Vec::new(),
            },
        };
        let candidate = vertical_content();
        let mut state = empty_state(absent_glyph_scan(candidate));
        state.line_states = vec![
            NativeStemsBeamVLinkLineState {
                v_linker: reference_for(1),
                stored_theoretical_line: prior_before,
                builder_line: prior_before,
                current_attachment_line: Some(prior_before),
            },
            NativeStemsBeamVLinkLineState {
                v_linker: reference_for(2),
                stored_theoretical_line: second_before,
                builder_line: second_before,
                current_attachment_line: Some(second_before),
            },
            NativeStemsBeamVLinkLineState {
                v_linker: reference(),
                stored_theoretical_line: current_line,
                builder_line: current_line,
                current_attachment_line: Some(current_line),
            },
        ];
        let attempt = ready_attempt(current_line);
        let missing = validate_line_prefix(&scheduler, 2, reference(), &attempt, &state).unwrap();
        assert_eq!(missing, vec![prior_delta.clone(), second_delta.clone()]);

        let mut out_of_order = state.clone();
        apply_line_delta(&mut out_of_order, &second_delta);
        out_of_order
            .applied_line_deltas
            .push(NativeStemsBeamAppliedLineDelta {
                system_id: 1,
                source: NativeStemsBeamAppliedLineDeltaSource::KnownFalsePrefix {
                    delta_ordinal: 1,
                },
                delta: second_delta.clone(),
            });
        assert!(matches!(
            validate_line_prefix(&scheduler, 2, reference(), &attempt, &out_of_order),
            Err(NativeStemsBeamVLinkTransactionError::DuplicateLineDelta)
        ));

        apply_line_delta(&mut state, &prior_delta);
        state
            .applied_line_deltas
            .push(NativeStemsBeamAppliedLineDelta {
                system_id: 1,
                source: NativeStemsBeamAppliedLineDeltaSource::KnownFalsePrefix {
                    delta_ordinal: 0,
                },
                delta: prior_delta,
            });
        assert_eq!(
            validate_line_prefix(&scheduler, 2, reference(), &attempt, &state).unwrap(),
            vec![second_delta.clone()]
        );

        apply_line_delta(&mut state, &second_delta);
        state
            .applied_line_deltas
            .push(NativeStemsBeamAppliedLineDelta {
                system_id: 1,
                source: NativeStemsBeamAppliedLineDeltaSource::KnownFalsePrefix {
                    delta_ordinal: 1,
                },
                delta: second_delta,
            });
        assert!(
            validate_line_prefix(&scheduler, 2, reference(), &attempt, &state)
                .unwrap()
                .is_empty()
        );
    }
}
