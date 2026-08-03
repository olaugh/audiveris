// SPDX-License-Identifier: AGPL-3.0-or-later

//! Stable-identity adapter for the tail of Java `BarsRetriever.buildBraces`.
//!
//! One compound is converted to a fixed glyph, passed through
//! `GlyphIndex.registerOriginal`, wrapped in a grade-0.8 `BraceInter`, and
//! inserted into its system SIG. The neutral store keeps the shared glyph and
//! inter index order while making Java's intermediate mutation prefix visible.

use std::{collections::BTreeMap, error::Error, fmt};

use audiveris_image::{
    bar_column::StaffId,
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable, RunTableError},
};

use crate::brace_filament::CurvedFilamentCompound;

pub const BRACE_INTRINSIC_GRADE: f64 = 0.8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BraceGlyphId(usize);

impl BraceGlyphId {
    #[must_use]
    pub const fn value(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BraceInterId(usize);

impl BraceInterId {
    #[must_use]
    pub const fn value(self) -> usize {
        self.0
    }
}

/// Java `Glyph(left, top, runTable)` created by `SectionCompound.toGlyph(null)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BraceGlyph {
    pub x: usize,
    pub y: usize,
    pub runs: RunTable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredBraceGlyph {
    pub id: BraceGlyphId,
    pub glyph: BraceGlyph,
}

/// Graph vertex after `SIGraph.addVertex`. `staff_id` is deliberately `None`:
/// unlike ordinary barline inters, Java never calls `braceInter.setStaff`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BraceInterNode {
    pub id: BraceInterId,
    pub glyph: BraceGlyphId,
    pub grade: f64,
    pub system_id: usize,
    pub staff_id: Option<StaffId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BracePromotion {
    pub system_id: usize,
    /// Staff whose BRACE_TOP initiated construction. This is traversal
    /// provenance, not ownership stored on `BraceInter`.
    pub source_staff_id: StaffId,
    pub glyph: BraceGlyphId,
    pub inter: BraceInterId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BraceSigMutation {
    GlyphRegistered(BraceGlyphId),
    GlyphReused(BraceGlyphId),
    InterRegistered(BraceInterId),
    SigVertexAdded {
        system_id: usize,
        inter: BraceInterId,
    },
}

/// Sheet-global original glyph and inter indices plus per-system SIG order.
#[derive(Clone, Debug)]
pub struct BraceSigStore {
    last_glyph_id: usize,
    last_inter_id: usize,
    originals: Vec<RegisteredBraceGlyph>,
    systems: BTreeMap<usize, Vec<BraceInterNode>>,
    mutations: Vec<BraceSigMutation>,
}

impl BraceSigStore {
    #[must_use]
    pub fn new(
        last_glyph_id: usize,
        last_inter_id: usize,
        system_ids: impl IntoIterator<Item = usize>,
    ) -> Self {
        Self {
            last_glyph_id,
            last_inter_id,
            originals: Vec::new(),
            systems: system_ids
                .into_iter()
                .map(|system_id| (system_id, Vec::new()))
                .collect(),
            mutations: Vec::new(),
        }
    }

    #[must_use]
    pub fn originals(&self) -> &[RegisteredBraceGlyph] {
        &self.originals
    }

    #[must_use]
    pub fn system_nodes(&self, system_id: usize) -> Option<&[BraceInterNode]> {
        self.systems.get(&system_id).map(Vec::as_slice)
    }

    #[must_use]
    pub fn mutations(&self) -> &[BraceSigMutation] {
        &self.mutations
    }

    #[must_use]
    pub const fn last_glyph_id(&self) -> usize {
        self.last_glyph_id
    }

    #[must_use]
    pub const fn last_inter_id(&self) -> usize {
        self.last_inter_id
    }

    fn register_original(&mut self, glyph: BraceGlyph) -> Result<BraceGlyphId, BraceSigError> {
        if let Some(original) = self
            .originals
            .iter()
            .find(|original| original.glyph == glyph)
        {
            self.mutations
                .push(BraceSigMutation::GlyphReused(original.id));
            return Ok(original.id);
        }

        let value = self
            .last_glyph_id
            .checked_add(1)
            .ok_or(BraceSigError::GlyphIdExhausted)?;
        let id = BraceGlyphId(value);
        self.last_glyph_id = value;
        self.originals.push(RegisteredBraceGlyph { id, glyph });
        self.mutations.push(BraceSigMutation::GlyphRegistered(id));
        Ok(id)
    }

    fn add_brace_inter(
        &mut self,
        system_id: usize,
        glyph: BraceGlyphId,
    ) -> Result<BraceInterId, BraceSigError> {
        let nodes = self
            .systems
            .get_mut(&system_id)
            .ok_or(BraceSigError::MissingSystem(system_id))?;

        // SIGraph.addVertex registers the interpretation in the sheet-global
        // InterIndex before touching the system graph.
        let value = self
            .last_inter_id
            .checked_add(1)
            .ok_or(BraceSigError::InterIdExhausted)?;
        let id = BraceInterId(value);
        self.last_inter_id = value;
        self.mutations.push(BraceSigMutation::InterRegistered(id));

        nodes.push(BraceInterNode {
            id,
            glyph,
            grade: BRACE_INTRINSIC_GRADE,
            system_id,
            staff_id: None,
        });
        self.mutations.push(BraceSigMutation::SigVertexAdded {
            system_id,
            inter: id,
        });
        Ok(id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BraceSigError {
    RunTable(RunTableError),
    GlyphIdExhausted,
    InterIdExhausted,
    MissingSystem(usize),
}

impl From<RunTableError> for BraceSigError {
    fn from(source: RunTableError) -> Self {
        Self::RunTable(source)
    }
}

impl fmt::Display for BraceSigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RunTable(source) => write!(formatter, "brace glyph run table failed: {source}"),
            Self::GlyphIdExhausted => formatter.write_str("brace glyph ID exhausted"),
            Self::InterIdExhausted => formatter.write_str("brace interpretation ID exhausted"),
            Self::MissingSystem(system_id) => {
                write!(formatter, "missing brace SIG system {system_id}")
            }
        }
    }
}

impl Error for BraceSigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RunTable(source) => Some(source),
            _ => None,
        }
    }
}

/// Execute Java's post-filament tail in source order. A late error keeps any
/// original glyph already registered; an inter-ID failure occurs before the
/// system graph gains a vertex.
pub fn promote_brace_filament(
    filament: &CurvedFilamentCompound,
    system_id: usize,
    source_staff_id: StaffId,
    store: &mut BraceSigStore,
) -> Result<BracePromotion, BraceSigError> {
    let glyph = build_brace_glyph(filament)?;
    let glyph = store.register_original(glyph)?;
    // `new BraceInter(glyph, Grades.intrinsicRatio * 1)` is infallible.
    let inter = store.add_brace_inter(system_id, glyph)?;
    Ok(BracePromotion {
        system_id,
        source_staff_id,
        glyph,
        inter,
    })
}

fn build_brace_glyph(filament: &CurvedFilamentCompound) -> Result<BraceGlyph, BraceSigError> {
    let members = filament.members();
    let first = members
        .first()
        .expect("brace compound boundary rejects empty member sets");
    let first_bounds = first.bounds();
    let mut left = first_bounds.x;
    let mut top = first_bounds.y;
    let mut right = first_bounds.x + first_bounds.width;
    let mut bottom = first_bounds.y + first_bounds.height;
    for member in &members[1..] {
        let bounds = member.bounds();
        left = left.min(bounds.x);
        top = top.min(bounds.y);
        right = right.max(bounds.x + bounds.width);
        bottom = bottom.max(bounds.y + bounds.height);
    }
    let width = right - left;
    let height = bottom - top;
    let mut pixels = vec![BACKGROUND; width * height];
    for member in members {
        for (offset, run) in member.runs().iter().enumerate() {
            match member.orientation() {
                Orientation::Horizontal => {
                    let y = member.first_pos() + offset;
                    for x in run.start..=run.stop() {
                        pixels[(y - top) * width + (x - left)] = FOREGROUND;
                    }
                }
                Orientation::Vertical => {
                    let x = member.first_pos() + offset;
                    for y in run.start..=run.stop() {
                        pixels[(y - top) * width + (x - left)] = FOREGROUND;
                    }
                }
            }
        }
    }
    let orientation = if width > height {
        Orientation::Horizontal
    } else {
        Orientation::Vertical
    };
    Ok(BraceGlyph {
        x: left,
        y: top,
        runs: RunTable::from_pixels(orientation, width, height, &pixels)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brace_filament::{
        BraceFilamentParameters, BraceFilamentPortion, build_brace_filament,
    };
    use audiveris_image::{
        run_table::{Run, RunTable},
        section::{JunctionPolicy, Section, build_sections_from_id},
        staff_peak::StaffPeak,
    };

    fn section(id: usize, x: usize, y: usize, height: usize) -> Section {
        let mut table = RunTable::new(Orientation::Vertical, x + 2, y + height + 1).unwrap();
        table.add_run(x, Run::new(y, height)).unwrap();
        build_sections_from_id(&table, JunctionPolicy::All, id)
            .into_iter()
            .find(|member| member.id() == id)
            .unwrap()
    }

    fn filament() -> CurvedFilamentCompound {
        let top = section(1, 10, 10, 6);
        let bottom = section(2, 11, 20, 6);
        let portions = [
            BraceFilamentPortion {
                peak: StaffPeak::new(StaffId::new(1), 10, 15, 8, 12).unwrap(),
                members: vec![top.clone()],
            },
            BraceFilamentPortion {
                peak: StaffPeak::new(StaffId::new(2), 20, 25, 8, 12).unwrap(),
                members: vec![bottom.clone()],
            },
        ];
        build_brace_filament(
            &portions,
            &[top, bottom],
            BraceFilamentParameters {
                interline: 10,
                segment_length: 20,
                left_margin: 2,
            },
        )
        .unwrap()
    }

    #[test]
    fn registers_glyph_then_inter_then_system_vertex() {
        let mut store = BraceSigStore::new(10, 20, [7]);
        let promotion =
            promote_brace_filament(&filament(), 7, StaffId::new(1), &mut store).unwrap();

        assert_eq!(promotion.glyph.value(), 11);
        assert_eq!(promotion.inter.value(), 21);
        assert_eq!(promotion.source_staff_id, StaffId::new(1));
        let original = &store.originals()[0];
        assert_eq!((original.glyph.x, original.glyph.y), (10, 10));
        assert_eq!(original.glyph.runs.orientation(), Orientation::Vertical);
        let node = store.system_nodes(7).unwrap()[0];
        assert_eq!(node.grade, BRACE_INTRINSIC_GRADE);
        assert_eq!(node.staff_id, None);
        assert_eq!(
            store.mutations(),
            [
                BraceSigMutation::GlyphRegistered(BraceGlyphId(11)),
                BraceSigMutation::InterRegistered(BraceInterId(21)),
                BraceSigMutation::SigVertexAdded {
                    system_id: 7,
                    inter: BraceInterId(21),
                },
            ]
        );
    }

    #[test]
    fn equal_original_is_reused_but_each_brace_gets_a_new_inter() {
        let mut store = BraceSigStore::new(0, 0, [7]);
        for staff in [1, 2] {
            promote_brace_filament(&filament(), 7, StaffId::new(staff), &mut store).unwrap();
        }
        assert_eq!(store.originals().len(), 1);
        assert_eq!(store.last_glyph_id(), 1);
        assert_eq!(store.last_inter_id(), 2);
        assert_eq!(store.system_nodes(7).unwrap().len(), 2);
        assert!(matches!(
            store.mutations()[3],
            BraceSigMutation::GlyphReused(BraceGlyphId(1))
        ));
    }

    #[test]
    fn missing_system_keeps_registered_original_only() {
        let mut store = BraceSigStore::new(4, 8, []);
        assert_eq!(
            promote_brace_filament(&filament(), 7, StaffId::new(1), &mut store),
            Err(BraceSigError::MissingSystem(7))
        );
        assert_eq!(store.last_glyph_id(), 5);
        assert_eq!(store.last_inter_id(), 8);
        assert_eq!(store.originals().len(), 1);
        assert_eq!(
            store.mutations(),
            [BraceSigMutation::GlyphRegistered(BraceGlyphId(5))]
        );
    }

    #[test]
    fn inter_id_failure_keeps_glyph_but_not_graph_vertex() {
        let mut store = BraceSigStore::new(0, usize::MAX, [7]);
        assert_eq!(
            promote_brace_filament(&filament(), 7, StaffId::new(1), &mut store),
            Err(BraceSigError::InterIdExhausted)
        );
        assert_eq!(store.originals().len(), 1);
        assert!(store.system_nodes(7).unwrap().is_empty());
        assert_eq!(
            store.mutations(),
            [BraceSigMutation::GlyphRegistered(BraceGlyphId(1))]
        );
    }
}
