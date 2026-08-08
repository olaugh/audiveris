// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact section-compound fallback for STEMS head-corner stumps.
//!
//! Java reaches this code only when a corner cannot reuse an accepted
//! `VERTICAL_SEED`. The fallback intersects the corner's small search box with
//! the complete system VLAG, performs the peculiar `SectionCompound` width
//! gate, optionally slices the nearest wide section, registers the fixed glyph,
//! and only then applies the vertical-extension gate.

use std::{cmp::Ordering, error::Error, fmt};

use audiveris_image::{
    run_table::{BACKGROUND, FOREGROUND, Orientation, Run, RunTable, RunTableError},
    section::{Bounds, Section},
};

use crate::{
    native_stem_seeds::{NativeStemSeedRecognition, contains_section_centroid},
    native_stems_head_corners::{NativeStemsHeadCornerRecognition, NativeStemsHeadCornerSystem},
    native_stems_head_seeds::NativeStemsHeadSeedRecognition,
    recognize::GridLinesRecognition,
    stems_step::{NativeStemHeadSide, NativeStemRect, NativeStemVerticalSide},
};

const STUMP_AREA_DX_RATIO: f64 = 0.1;
const STUMP_AREA_DY_RATIO: f64 = 0.2;
const MIN_HEAD_STUMP_DY_RATIO: f64 = 0.5;

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadStumpRecognition {
    pub systems: Vec<NativeStemsHeadStumpSystem>,
    pub corner_count: usize,
    pub seed_count: usize,
    pub fallback_count: usize,
    pub empty_build_count: usize,
    pub build_candidate_count: usize,
    pub accepted_build_count: usize,
    pub rejected_build_count: usize,
    pub new_build_count: usize,
    pub reused_build_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadStumpSystem {
    pub system_id: usize,
    /// Complete system VLAG in persistent lag order.
    pub vertical_section_source_ordinals: Vec<usize>,
    pub heads_by_abscissa: Vec<NativeStemsHeadStumpHead>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadStumpHead {
    pub x_ordinal: usize,
    pub sig_ordinal: usize,
    pub corners_in_constructor_order: Vec<NativeStemsHeadStumpCorner>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadStumpCorner {
    pub constructor_ordinal: usize,
    pub outcome: NativeStemsHeadStumpOutcome,
    pub build: Option<NativeStemsHeadStumpBuild>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadStumpOutcome {
    Seed { free_glyph_ordinal: usize },
    Built { canonical_glyph_index: usize },
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadStumpBuild {
    pub area: NativeStemRect,
    pub sections: Vec<NativeStemsHeadStumpSection>,
    pub pre_added_source_ordinal: Option<usize>,
    pub steps: Vec<NativeStemsHeadStumpStep>,
    pub subsection: Option<NativeStemsHeadStumpSubsection>,
    pub compound_weight: usize,
    pub compound_bounds: Option<Bounds>,
    pub candidate: Option<NativeStemsHeadStumpGlyph>,
    pub ref_y: i32,
    pub extension_dy: i32,
    pub stands_out: bool,
    pub canonical_glyph_index: Option<usize>,
    pub registration: Option<NativeStemsHeadStumpRegistration>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadStumpSection {
    pub sort_ordinal: usize,
    pub input_ordinal: usize,
    /// Ordinal within the complete system-dispatched VLAG.
    pub source_ordinal: usize,
    pub bounds: Bounds,
    pub weight: usize,
    pub first_pos: usize,
    pub run_count: usize,
    pub area_center: (usize, usize),
    pub distance: f64,
    pub contains_reference: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadStumpStep {
    pub sort_ordinal: usize,
    pub source_ordinal: usize,
    pub pre_member: bool,
    pub after_add_width: usize,
    pub too_wide: bool,
    pub removed: bool,
    pub final_weight: usize,
    pub final_bounds: Option<Bounds>,
    /// Source ordinals in Java `Section.byFullAbscissa` member order.
    pub member_source_ordinals: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadStumpSubsection {
    pub source_ordinal: usize,
    pub stem_width: i32,
    pub x0: i32,
    pub i0: i32,
    pub x1: i32,
    pub i1: i32,
    pub first_pos: i32,
    pub runs: Vec<Run>,
    pub bounds: Option<Bounds>,
    pub weight: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeStemsHeadStumpGlyph {
    pub bounds: Bounds,
    pub weight: usize,
    pub run_table: RunTable,
}

impl NativeStemsHeadStumpGlyph {
    #[must_use]
    pub fn run_count(&self) -> usize {
        (0..self.run_table.sequence_count())
            .map(|index| self.run_table.sequence(index).map_or(0, <[_]>::len))
            .sum()
    }

    #[must_use]
    pub fn run_digest(&self) -> u64 {
        fixed_run_digest(&self.run_table)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadStumpRegistration {
    New { origin: NativeStemsHeadStumpOrigin },
    Reused { origin: NativeStemsHeadStumpOrigin },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadStumpOrigin {
    StemSeed {
        system_id: usize,
        registered_glyph_ordinal: usize,
    },
    HeadBuild {
        system_id: usize,
        head_ordinal: usize,
        constructor_ordinal: usize,
    },
}

#[derive(Debug)]
pub enum NativeStemsHeadStumpError {
    SystemOrder,
    MissingSystemArea(usize),
    MissingCornerSystem(usize),
    InvalidStemThickness(i32),
    InvalidInterline(i32),
    InvalidGeometry,
    RunTable(RunTableError),
}

impl fmt::Display for NativeStemsHeadStumpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemOrder => formatter.write_str("STEMS head-stump system order differs"),
            Self::MissingSystemArea(system_id) => {
                write!(
                    formatter,
                    "STEMS head-stump system {system_id} has no GRID area"
                )
            }
            Self::MissingCornerSystem(system_id) => {
                write!(
                    formatter,
                    "STEMS head-stump system {system_id} has no corner product"
                )
            }
            Self::InvalidStemThickness(value) => {
                write!(formatter, "invalid main stem thickness {value}")
            }
            Self::InvalidInterline(value) => write!(formatter, "invalid interline {value}"),
            Self::InvalidGeometry => formatter.write_str("invalid section-compound geometry"),
            Self::RunTable(source) => write!(formatter, "invalid stump run table: {source}"),
        }
    }
}

impl Error for NativeStemsHeadStumpError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RunTable(source) => Some(source),
            _ => None,
        }
    }
}

impl From<RunTableError> for NativeStemsHeadStumpError {
    fn from(source: RunTableError) -> Self {
        Self::RunTable(source)
    }
}

/// Build and register every section-based head stump in constructor order.
pub fn materialize_native_stems_head_stumps(
    grid: &GridLinesRecognition,
    stem_seeds: &NativeStemSeedRecognition,
    corners: &NativeStemsHeadCornerRecognition,
    head_seeds: &NativeStemsHeadSeedRecognition,
) -> Result<NativeStemsHeadStumpRecognition, NativeStemsHeadStumpError> {
    let ids = grid
        .peak_graph
        .sig
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    if ids
        != stem_seeds
            .systems
            .iter()
            .map(|system| system.raw.system_id)
            .collect::<Vec<_>>()
        || ids
            != head_seeds
                .systems
                .iter()
                .map(|system| system.system_id)
                .collect::<Vec<_>>()
    {
        return Err(NativeStemsHeadStumpError::SystemOrder);
    }
    if stem_seeds.main_stem_thickness <= 0 {
        return Err(NativeStemsHeadStumpError::InvalidStemThickness(
            stem_seeds.main_stem_thickness,
        ));
    }

    let mut registry = seed_registry(stem_seeds);
    let mut systems = Vec::with_capacity(head_seeds.systems.len());
    let mut totals = [0_usize; 9];
    for seed_system in &head_seeds.systems {
        let system_id = seed_system.system_id;
        let corner_system = corners
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .ok_or(NativeStemsHeadStumpError::MissingCornerSystem(system_id))?;
        if corner_system.interline <= 0 {
            return Err(NativeStemsHeadStumpError::InvalidInterline(
                corner_system.interline,
            ));
        }
        let area = grid
            .system_areas
            .iter()
            .find(|area| area.system_id == system_id)
            .ok_or(NativeStemsHeadStumpError::MissingSystemArea(system_id))?;
        let vertical_section_source_ordinals = grid
            .peak_graph
            .vertical_sections
            .iter()
            .enumerate()
            .filter_map(|(ordinal, section)| {
                let (x, y) = section.centroid();
                contains_section_centroid(area, x as f64, y as f64).then_some(ordinal)
            })
            .collect::<Vec<_>>();

        let mut heads = Vec::with_capacity(seed_system.heads_by_abscissa.len());
        for seed_head in &seed_system.heads_by_abscissa {
            let corner_head = corner_system
                .heads_in_sig_order
                .get(seed_head.sig_ordinal)
                .ok_or(NativeStemsHeadStumpError::InvalidGeometry)?;
            let mut built_corners = Vec::with_capacity(4);
            for (seed_corner, geometry) in seed_head
                .corners_in_constructor_order
                .iter()
                .zip(&corner_head.corners_in_constructor_order)
            {
                totals[0] += 1;
                if let Some(free_glyph_ordinal) = seed_corner.selected_seed_ordinal {
                    totals[1] += 1;
                    built_corners.push(NativeStemsHeadStumpCorner {
                        constructor_ordinal: seed_corner.constructor_ordinal,
                        outcome: NativeStemsHeadStumpOutcome::Seed { free_glyph_ordinal },
                        build: None,
                    });
                    continue;
                }

                totals[2] += 1;
                let mut build = build_stump(
                    grid,
                    &vertical_section_source_ordinals,
                    corner_system,
                    geometry,
                    stem_seeds.main_stem_thickness,
                )?;
                let outcome = if let Some(candidate) = &build.candidate {
                    totals[4] += 1;
                    if build.stands_out {
                        totals[5] += 1;
                    } else {
                        totals[6] += 1;
                    }
                    let (canonical_glyph_index, registration) = register_candidate(
                        &mut registry,
                        candidate,
                        NativeStemsHeadStumpOrigin::HeadBuild {
                            system_id,
                            head_ordinal: seed_head.x_ordinal,
                            constructor_ordinal: seed_corner.constructor_ordinal,
                        },
                    );
                    if matches!(registration, NativeStemsHeadStumpRegistration::New { .. }) {
                        totals[7] += 1;
                    } else {
                        totals[8] += 1;
                    }
                    build.canonical_glyph_index = Some(canonical_glyph_index);
                    build.registration = Some(registration);
                    if build.stands_out {
                        NativeStemsHeadStumpOutcome::Built {
                            canonical_glyph_index,
                        }
                    } else {
                        NativeStemsHeadStumpOutcome::None
                    }
                } else {
                    totals[3] += 1;
                    NativeStemsHeadStumpOutcome::None
                };
                built_corners.push(NativeStemsHeadStumpCorner {
                    constructor_ordinal: seed_corner.constructor_ordinal,
                    outcome,
                    build: Some(build),
                });
            }
            if built_corners.len() != seed_head.corners_in_constructor_order.len() {
                return Err(NativeStemsHeadStumpError::InvalidGeometry);
            }
            heads.push(NativeStemsHeadStumpHead {
                x_ordinal: seed_head.x_ordinal,
                sig_ordinal: seed_head.sig_ordinal,
                corners_in_constructor_order: built_corners,
            });
        }
        systems.push(NativeStemsHeadStumpSystem {
            system_id,
            vertical_section_source_ordinals,
            heads_by_abscissa: heads,
        });
    }

    Ok(NativeStemsHeadStumpRecognition {
        systems,
        corner_count: totals[0],
        seed_count: totals[1],
        fallback_count: totals[2],
        empty_build_count: totals[3],
        build_candidate_count: totals[4],
        accepted_build_count: totals[5],
        rejected_build_count: totals[6],
        new_build_count: totals[7],
        reused_build_count: totals[8],
    })
}

fn build_stump(
    grid: &GridLinesRecognition,
    system_section_ordinals: &[usize],
    corner_system: &NativeStemsHeadCornerSystem,
    corner: &crate::native_stems_head_corners::NativeStemsHeadCorner,
    main_stem_thickness: i32,
) -> Result<NativeStemsHeadStumpBuild, NativeStemsHeadStumpError> {
    let dx_in = f64::from(corner_system.interline) * STUMP_AREA_DX_RATIO;
    let dx_out = dx_in;
    let dy = (f64::from(corner_system.interline) * STUMP_AREA_DY_RATIO).round_ties_even() as i32;
    let x_direction = match corner.horizontal {
        NativeStemHeadSide::Left => -1,
        NativeStemHeadSide::Right => 1,
    };
    let y_direction = match corner.vertical {
        NativeStemVerticalSide::Top => -1,
        NativeStemVerticalSide::Bottom => 1,
    };
    let left = if x_direction > 0 {
        corner.reference.x - dx_in
    } else {
        corner.reference.x - dx_out
    };
    let right = if x_direction > 0 {
        corner.reference.x + dx_out
    } else {
        corner.reference.x + dx_in
    };
    let top = if y_direction > 0 {
        corner.reference.y
    } else {
        corner.reference.y - f64::from(dy)
    };
    let area = NativeStemRect {
        x: left,
        y: top,
        width: right - left,
        height: f64::from(dy),
    };

    let mut intersected = system_section_ordinals
        .iter()
        .enumerate()
        .filter_map(|(source_ordinal, &global_ordinal)| {
            let section = &grid.peak_graph.vertical_sections[global_ordinal];
            section_intersects_rectangle(section, area).then(|| {
                let bounds = section.bounds();
                let area_center = (
                    bounds.x + (bounds.width / 2),
                    bounds.y + (bounds.height / 2),
                );
                SectionCandidate {
                    input_ordinal: 0,
                    source_ordinal,
                    global_ordinal,
                    distance: (area_center.0 as f64 - corner.reference.x).abs(),
                    contains_reference: section_polygon_contains(
                        section,
                        corner.reference.x as i32,
                        corner.reference.y as i32,
                    ),
                    area_center,
                }
            })
        })
        .collect::<Vec<_>>();
    for (input_ordinal, candidate) in intersected.iter_mut().enumerate() {
        candidate.input_ordinal = input_ordinal;
    }
    // `Comparator.comparingDouble` is stable. Rust's slice sort is stable too.
    intersected.sort_by(|left, right| left.distance.total_cmp(&right.distance));

    let sections = intersected
        .iter()
        .enumerate()
        .map(|(sort_ordinal, candidate)| {
            let section = &grid.peak_graph.vertical_sections[candidate.global_ordinal];
            NativeStemsHeadStumpSection {
                sort_ordinal,
                input_ordinal: candidate.input_ordinal,
                source_ordinal: candidate.source_ordinal,
                bounds: section.bounds(),
                weight: section.weight(),
                first_pos: section.first_pos(),
                run_count: section.run_count(),
                area_center: candidate.area_center,
                distance: candidate.distance,
                contains_reference: candidate.contains_reference,
            }
        })
        .collect::<Vec<_>>();

    let mut build = NativeStemsHeadStumpBuild {
        area,
        sections,
        pre_added_source_ordinal: None,
        steps: Vec::new(),
        subsection: None,
        compound_weight: 0,
        compound_bounds: None,
        candidate: None,
        ref_y: corner.reference.y.round_ties_even() as i32,
        extension_dy: 0,
        stands_out: false,
        canonical_glyph_index: None,
        registration: None,
    };
    if intersected.is_empty() {
        return Ok(build);
    }

    let mut members = Vec::<usize>::new();
    if let Some(candidate) = intersected
        .iter()
        .find(|candidate| candidate.contains_reference)
    {
        members.push(candidate.global_ordinal);
        build.pre_added_source_ordinal = Some(candidate.source_ordinal);
    }

    for (sort_ordinal, candidate) in intersected.iter().enumerate() {
        let pre_member = members.contains(&candidate.global_ordinal);
        if !pre_member {
            members.push(candidate.global_ordinal);
        }
        sort_source_members(&mut members, &grid.peak_graph.vertical_sections);
        let after_add_bounds = source_compound_bounds(&members, &grid.peak_graph.vertical_sections)
            .ok_or(NativeStemsHeadStumpError::InvalidGeometry)?;
        let after_add_width = after_add_bounds.width;
        let too_wide = after_add_width > main_stem_thickness as usize;
        let removed = if too_wide {
            let old_len = members.len();
            members.retain(|&ordinal| ordinal != candidate.global_ordinal);
            members.len() != old_len
        } else {
            false
        };
        let final_weight = members
            .iter()
            .map(|&ordinal| grid.peak_graph.vertical_sections[ordinal].weight())
            .sum();
        let final_bounds = source_compound_bounds(&members, &grid.peak_graph.vertical_sections);
        build.steps.push(NativeStemsHeadStumpStep {
            sort_ordinal,
            source_ordinal: candidate.source_ordinal,
            pre_member,
            after_add_width,
            too_wide,
            removed,
            final_weight,
            final_bounds,
            member_source_ordinals: members
                .iter()
                .map(|&global| {
                    system_section_ordinals
                        .iter()
                        .position(|&candidate| candidate == global)
                        .expect("compound member belongs to system VLAG")
                })
                .collect(),
        });
    }

    let mut synthetic = None;
    if members.is_empty() {
        let wide = &intersected[0];
        let section = &grid.peak_graph.vertical_sections[wide.global_ordinal];
        let stem_width = main_stem_thickness;
        let x0 = (corner.reference.x - f64::from(stem_width) / 2.0).round_ties_even() as i32;
        let first_pos = i32::try_from(section.first_pos())
            .map_err(|_| NativeStemsHeadStumpError::InvalidGeometry)?;
        let run_count = i32::try_from(section.run_count())
            .map_err(|_| NativeStemsHeadStumpError::InvalidGeometry)?;
        let i0 = 0.max(x0 - first_pos);
        let x1 = x0 + stem_width;
        let i1 = run_count.min(x1 - first_pos);
        let runs = if i1 > i0 {
            section.runs()[i0 as usize..i1 as usize].to_vec()
        } else {
            Vec::new()
        };
        let owned = OwnedVerticalSection {
            first_pos: x0,
            runs: runs.clone(),
        };
        let bounds = owned.bounds();
        let weight = owned.weight();
        build.subsection = Some(NativeStemsHeadStumpSubsection {
            source_ordinal: wide.source_ordinal,
            stem_width,
            x0,
            i0,
            x1,
            i1,
            first_pos: if runs.is_empty() { 0 } else { x0 },
            runs,
            bounds,
            weight,
        });
        if weight != 0 {
            synthetic = Some(owned);
        }
    }

    build.compound_weight = members
        .iter()
        .map(|&ordinal| grid.peak_graph.vertical_sections[ordinal].weight())
        .sum::<usize>()
        + synthetic.as_ref().map_or(0, OwnedVerticalSection::weight);
    build.compound_bounds = compound_bounds(
        &members,
        &grid.peak_graph.vertical_sections,
        synthetic.as_ref(),
    );
    if build.compound_weight == 0 {
        return Ok(build);
    }
    let candidate = materialize_compound(
        &members,
        &grid.peak_graph.vertical_sections,
        synthetic.as_ref(),
    )?;
    build.extension_dy = if y_direction > 0 {
        i32::try_from(candidate.bounds.y + candidate.bounds.height - 1)
            .map_err(|_| NativeStemsHeadStumpError::InvalidGeometry)?
            - build.ref_y
    } else {
        build.ref_y
            - i32::try_from(candidate.bounds.y)
                .map_err(|_| NativeStemsHeadStumpError::InvalidGeometry)?
    };
    build.stands_out = build.extension_dy
        >= (f64::from(corner_system.interline) * MIN_HEAD_STUMP_DY_RATIO).round_ties_even() as i32;
    build.candidate = Some(candidate);
    Ok(build)
}

#[derive(Clone, Debug)]
struct SectionCandidate {
    input_ordinal: usize,
    source_ordinal: usize,
    global_ordinal: usize,
    distance: f64,
    contains_reference: bool,
    area_center: (usize, usize),
}

#[derive(Clone, Debug)]
struct OwnedVerticalSection {
    first_pos: i32,
    runs: Vec<Run>,
}

impl OwnedVerticalSection {
    fn weight(&self) -> usize {
        self.runs.iter().map(|run| run.length).sum()
    }

    fn bounds(&self) -> Option<Bounds> {
        let min = self.runs.iter().map(|run| run.start).min()?;
        let max = self.runs.iter().map(|run| run.stop()).max()?;
        Some(Bounds {
            x: usize::try_from(self.first_pos).ok()?,
            y: min,
            width: self.runs.len(),
            height: max - min + 1,
        })
    }
}

#[derive(Clone)]
struct RegistryEntry {
    bounds: Bounds,
    run_table: RunTable,
    origin: NativeStemsHeadStumpOrigin,
}

fn seed_registry(stem_seeds: &NativeStemSeedRecognition) -> Vec<RegistryEntry> {
    let mut registry = Vec::new();
    for system in &stem_seeds.systems {
        for (ordinal, glyph) in system.registered_glyphs.iter().enumerate() {
            if !registry.iter().any(|entry: &RegistryEntry| {
                entry.bounds == glyph.bounds && entry.run_table == glyph.run_table
            }) {
                registry.push(RegistryEntry {
                    bounds: glyph.bounds,
                    run_table: glyph.run_table.clone(),
                    origin: NativeStemsHeadStumpOrigin::StemSeed {
                        system_id: system.raw.system_id,
                        registered_glyph_ordinal: ordinal,
                    },
                });
            }
        }
    }
    registry
}

fn register_candidate(
    registry: &mut Vec<RegistryEntry>,
    candidate: &NativeStemsHeadStumpGlyph,
    origin: NativeStemsHeadStumpOrigin,
) -> (usize, NativeStemsHeadStumpRegistration) {
    if let Some((index, entry)) = registry.iter().enumerate().find(|(_, entry)| {
        entry.bounds == candidate.bounds && entry.run_table == candidate.run_table
    }) {
        return (
            index,
            NativeStemsHeadStumpRegistration::Reused {
                origin: entry.origin.clone(),
            },
        );
    }
    let index = registry.len();
    registry.push(RegistryEntry {
        bounds: candidate.bounds,
        run_table: candidate.run_table.clone(),
        origin: origin.clone(),
    });
    (index, NativeStemsHeadStumpRegistration::New { origin })
}

fn section_intersects_rectangle(section: &Section, rectangle: NativeStemRect) -> bool {
    if rectangle.width <= 0.0 || rectangle.height <= 0.0 {
        return false;
    }
    section.runs().iter().enumerate().any(|(offset, run)| {
        let position = section.first_pos() + offset;
        let (x, y, width, height) = match section.orientation() {
            Orientation::Horizontal => (run.start, position, run.length, 1),
            Orientation::Vertical => (position, run.start, 1, run.length),
        };
        rectangle.x + rectangle.width > x as f64
            && rectangle.y + rectangle.height > y as f64
            && rectangle.x < (x + width) as f64
            && rectangle.y < (y + height) as f64
    })
}

fn section_polygon_contains(section: &Section, x: i32, y: i32) -> bool {
    let polygon = section_polygon(section);
    if polygon.len() <= 2 {
        return false;
    }
    let bounds = section.bounds();
    let Ok(left) = i32::try_from(bounds.x) else {
        return false;
    };
    let Ok(top) = i32::try_from(bounds.y) else {
        return false;
    };
    let Ok(right) = i32::try_from(bounds.x + bounds.width) else {
        return false;
    };
    let Ok(bottom) = i32::try_from(bounds.y + bounds.height) else {
        return false;
    };
    if x < left || y < top || x >= right || y >= bottom {
        return false;
    }

    let x = f64::from(x);
    let y = f64::from(y);
    let mut hits = 0;
    let (mut last_x, mut last_y) = polygon[polygon.len() - 1];
    for &(current_x, current_y) in &polygon {
        if current_y != last_y {
            let left_x;
            if current_x < last_x {
                if x >= f64::from(last_x) {
                    last_x = current_x;
                    last_y = current_y;
                    continue;
                }
                left_x = current_x;
            } else {
                if x >= f64::from(current_x) {
                    last_x = current_x;
                    last_y = current_y;
                    continue;
                }
                left_x = last_x;
            }

            let (test1, test2);
            if current_y < last_y {
                if y < f64::from(current_y) || y >= f64::from(last_y) {
                    last_x = current_x;
                    last_y = current_y;
                    continue;
                }
                if x < f64::from(left_x) {
                    hits += 1;
                    last_x = current_x;
                    last_y = current_y;
                    continue;
                }
                test1 = x - f64::from(current_x);
                test2 = y - f64::from(current_y);
            } else {
                if y < f64::from(last_y) || y >= f64::from(current_y) {
                    last_x = current_x;
                    last_y = current_y;
                    continue;
                }
                if x < f64::from(left_x) {
                    hits += 1;
                    last_x = current_x;
                    last_y = current_y;
                    continue;
                }
                test1 = x - f64::from(last_x);
                test2 = y - f64::from(last_y);
            }
            if test1 < test2 / f64::from(last_y - current_y) * f64::from(last_x - current_x) {
                hits += 1;
            }
        }
        last_x = current_x;
        last_y = current_y;
    }
    hits & 1 != 0
}

fn section_polygon(section: &Section) -> Vec<(i32, i32)> {
    let mut oriented = Vec::with_capacity(1 + 4 * section.run_count());
    populate_polygon_side(section, 1, &mut oriented);
    populate_polygon_side(section, -1, &mut oriented);
    oriented
        .into_iter()
        .map(|(coordinate, position)| match section.orientation() {
            Orientation::Horizontal => (coordinate, position),
            Orientation::Vertical => (position, coordinate),
        })
        .collect()
}

fn populate_polygon_side(section: &Section, direction: i32, points: &mut Vec<(i32, i32)>) {
    let run_count = section.run_count() as i32;
    let mut index = if direction > 0 { 0 } else { run_count - 1 };
    let end = if direction > 0 { run_count } else { -1 };
    let mut position = section.first_pos() as i32 + if direction > 0 { 0 } else { run_count };
    let mut previous = -1;
    while index != end {
        let run = section.runs()[index as usize];
        let coordinate = if direction > 0 {
            run.start as i32
        } else {
            (run.stop() + 1) as i32
        };
        if coordinate != previous {
            if previous != -1 {
                points.push((previous, position));
            }
            points.push((coordinate, position));
            previous = coordinate;
        }
        position += direction;
        index += direction;
    }
    points.push((previous, position));
    if direction < 0 {
        points.push((section.runs()[0].start as i32, section.first_pos() as i32));
    }
}

fn sort_source_members(members: &mut [usize], sections: &[Section]) {
    members.sort_by(|&left, &right| full_abscissa_cmp(&sections[left], &sections[right]));
}

fn full_abscissa_cmp(left: &Section, right: &Section) -> Ordering {
    let left_bounds = left.bounds();
    let right_bounds = right.bounds();
    left_bounds
        .x
        .cmp(&right_bounds.x)
        .then_with(|| left_bounds.y.cmp(&right_bounds.y))
        .then_with(|| left.id().cmp(&right.id()))
}

fn source_compound_bounds(members: &[usize], sections: &[Section]) -> Option<Bounds> {
    compound_bounds(members, sections, None)
}

fn compound_bounds(
    members: &[usize],
    sections: &[Section],
    synthetic: Option<&OwnedVerticalSection>,
) -> Option<Bounds> {
    let mut bounds = members.iter().map(|&ordinal| sections[ordinal].bounds());
    let mut union = bounds
        .next()
        .or_else(|| synthetic.and_then(OwnedVerticalSection::bounds))?;
    for bounds in bounds.chain(synthetic.and_then(OwnedVerticalSection::bounds)) {
        union = union_bounds(union, bounds)?;
    }
    Some(union)
}

fn union_bounds(left: Bounds, right: Bounds) -> Option<Bounds> {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    let right_edge = left
        .x
        .checked_add(left.width)?
        .max(right.x.checked_add(right.width)?);
    let bottom = left
        .y
        .checked_add(left.height)?
        .max(right.y.checked_add(right.height)?);
    Some(Bounds {
        x,
        y,
        width: right_edge.checked_sub(x)?,
        height: bottom.checked_sub(y)?,
    })
}

fn materialize_compound(
    members: &[usize],
    sections: &[Section],
    synthetic: Option<&OwnedVerticalSection>,
) -> Result<NativeStemsHeadStumpGlyph, NativeStemsHeadStumpError> {
    let bounds = compound_bounds(members, sections, synthetic)
        .ok_or(NativeStemsHeadStumpError::InvalidGeometry)?;
    let mut pixels = vec![
        BACKGROUND;
        bounds
            .width
            .checked_mul(bounds.height)
            .ok_or(NativeStemsHeadStumpError::InvalidGeometry)?
    ];
    for &ordinal in members {
        paint_section(&sections[ordinal], bounds, &mut pixels)?;
    }
    if let Some(synthetic) = synthetic {
        paint_owned_vertical_section(synthetic, bounds, &mut pixels)?;
    }
    let orientation = if bounds.width > bounds.height {
        Orientation::Horizontal
    } else {
        Orientation::Vertical
    };
    let run_table = RunTable::from_pixels(orientation, bounds.width, bounds.height, &pixels)?;
    let weight = pixels.iter().filter(|&&pixel| pixel == FOREGROUND).count();
    Ok(NativeStemsHeadStumpGlyph {
        bounds,
        weight,
        run_table,
    })
}

fn paint_section(
    section: &Section,
    crop: Bounds,
    pixels: &mut [u8],
) -> Result<(), NativeStemsHeadStumpError> {
    for (offset, run) in section.runs().iter().enumerate() {
        let position = section.first_pos() + offset;
        for coordinate in run.start..=run.stop() {
            let (x, y) = match section.orientation() {
                Orientation::Horizontal => (coordinate, position),
                Orientation::Vertical => (position, coordinate),
            };
            paint_pixel(x, y, crop, pixels)?;
        }
    }
    Ok(())
}

fn paint_owned_vertical_section(
    section: &OwnedVerticalSection,
    crop: Bounds,
    pixels: &mut [u8],
) -> Result<(), NativeStemsHeadStumpError> {
    let first_pos = usize::try_from(section.first_pos)
        .map_err(|_| NativeStemsHeadStumpError::InvalidGeometry)?;
    for (offset, run) in section.runs.iter().enumerate() {
        let x = first_pos + offset;
        for y in run.start..=run.stop() {
            paint_pixel(x, y, crop, pixels)?;
        }
    }
    Ok(())
}

fn paint_pixel(
    x: usize,
    y: usize,
    crop: Bounds,
    pixels: &mut [u8],
) -> Result<(), NativeStemsHeadStumpError> {
    let relative_x = x
        .checked_sub(crop.x)
        .ok_or(NativeStemsHeadStumpError::InvalidGeometry)?;
    let relative_y = y
        .checked_sub(crop.y)
        .ok_or(NativeStemsHeadStumpError::InvalidGeometry)?;
    let index = relative_y
        .checked_mul(crop.width)
        .and_then(|row| row.checked_add(relative_x))
        .filter(|&index| index < pixels.len())
        .ok_or(NativeStemsHeadStumpError::InvalidGeometry)?;
    pixels[index] = FOREGROUND;
    Ok(())
}

fn fixed_run_digest(run_table: &RunTable) -> u64 {
    let orientation = match run_table.orientation() {
        Orientation::Horizontal => "HORIZONTAL",
        Orientation::Vertical => "VERTICAL",
    };
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash_line(
        &mut hash,
        &format!("{orientation} {} {}", run_table.width(), run_table.height()),
    );
    for sequence in 0..run_table.sequence_count() {
        let mut row = sequence.to_string();
        for run in run_table.sequence(sequence).unwrap_or_default() {
            row.push_str(&format!(" {}:{}", run.start, run.length));
        }
        hash_line(&mut hash, &row);
    }
    hash
}

fn hash_line(hash: &mut u64, line: &str) {
    for byte in line.bytes().chain([b'\n']) {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}
