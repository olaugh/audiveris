// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light port of Java `StickFactory`.
//!
//! This is the section-to-`StraightFilament` geometry used by LEDGERS and raw
//! STEM_SEEDS. It preserves graph/source ordering, core registration, side
//! thickening and one-run sticker attachment. Glyph and SIG interpretation
//! ownership belongs to the later OMR materializer.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use audiveris_core::basic_line::{BasicLine, LineError};

use crate::{
    filament::{FilamentError, StaffFilament},
    run_table::Orientation,
    section::{Bounds, Section},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HorizontalStickParameters {
    pub interline: usize,
    pub maximum_stick_thickness: usize,
    pub minimum_core_section_length: usize,
    pub minimum_side_ratio: f64,
}

/// Parameters for Java `StickFactory(VERTICAL, ...)`, used by
/// `VerticalsBuilder.retrieveCandidates`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VerticalStickParameters {
    pub interline: usize,
    pub maximum_stick_thickness: usize,
    pub minimum_core_section_length: usize,
    pub minimum_side_ratio: f64,
}

/// One vertical `StraightFilament`, including the horizontal one-pixel
/// stickers Java appends after thickening.
#[derive(Clone, Debug)]
pub struct IdentifiedVerticalStick {
    id: u64,
    sections: Vec<Section>,
}

impl IdentifiedVerticalStick {
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// Java `SectionCompound.getMembers()` order: absolute x, y, then the
    /// lag-local section id, even when orientations are mixed.
    #[must_use]
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    #[must_use]
    pub fn weight(&self) -> usize {
        self.sections.iter().map(Section::weight).sum()
    }

    pub fn straight_geometry(&self) -> Result<StraightStickGeometry, StraightStickError> {
        vertical_straight_geometry(self)
    }
}

#[derive(Clone, Debug)]
pub struct VerticalStickResult {
    survivors: Vec<IdentifiedVerticalStick>,
    creation_ids: Vec<u64>,
    next_creation_id: u64,
}

impl VerticalStickResult {
    #[must_use]
    pub fn survivors(&self) -> &[IdentifiedVerticalStick] {
        &self.survivors
    }

    #[must_use]
    pub fn creation_ids(&self) -> &[u64] {
        &self.creation_ids
    }

    #[must_use]
    pub const fn next_creation_id(&self) -> u64 {
        self.next_creation_id
    }
}

#[derive(Clone, Debug)]
pub struct VerticalStickOutcome {
    pub result: VerticalStickResult,
    /// Java mutations completed before a checked geometry/identity failure.
    pub error: Option<StraightStickError>,
}

#[derive(Clone, Debug)]
pub struct IdentifiedStraightStick {
    id: u64,
    filament: StaffFilament,
}

impl IdentifiedStraightStick {
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub const fn filament(&self) -> &StaffFilament {
        &self.filament
    }

    pub fn straight_geometry(&self) -> Result<StraightStickGeometry, StraightStickError> {
        straight_geometry(&self.filament)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StraightStickGeometry {
    pub bounds: Bounds,
    pub start: (f64, f64),
    pub stop: (f64, f64),
    pub mean_thickness: f64,
    pub mean_distance: f64,
}

#[derive(Clone, Debug)]
pub struct HorizontalStickResult {
    survivors: Vec<IdentifiedStraightStick>,
    creation_ids: Vec<u64>,
    next_creation_id: u64,
}

impl HorizontalStickResult {
    #[must_use]
    pub fn survivors(&self) -> &[IdentifiedStraightStick] {
        &self.survivors
    }

    #[must_use]
    pub fn creation_ids(&self) -> &[u64] {
        &self.creation_ids
    }

    #[must_use]
    pub const fn next_creation_id(&self) -> u64 {
        self.next_creation_id
    }
}

#[derive(Clone, Debug)]
pub struct HorizontalStickOutcome {
    pub result: HorizontalStickResult,
    /// Java mutations completed before a checked geometry/identity failure.
    pub error: Option<StraightStickError>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StraightStickError {
    InvalidFirstId,
    InvalidParameters,
    IdentityOverflow,
    Filament(FilamentError),
    Line(LineError),
}

impl From<FilamentError> for StraightStickError {
    fn from(value: FilamentError) -> Self {
        Self::Filament(value)
    }
}

impl From<LineError> for StraightStickError {
    fn from(value: LineError) -> Self {
        Self::Line(value)
    }
}

impl fmt::Display for StraightStickError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFirstId => formatter.write_str("first stick ID must be positive"),
            Self::InvalidParameters => formatter.write_str("invalid horizontal stick parameters"),
            Self::IdentityOverflow => formatter.write_str("horizontal stick identity overflow"),
            Self::Filament(source) => write!(formatter, "stick filament failed: {source}"),
            Self::Line(source) => write!(formatter, "stick line failed: {source}"),
        }
    }
}

impl Error for StraightStickError {}

#[derive(Clone)]
struct LinkedSection {
    section: Section,
    sources: Vec<usize>,
    targets: Vec<usize>,
    processed: bool,
    compound_id: Option<u64>,
}

pub struct HorizontalStickFactory {
    parameters: HorizontalStickParameters,
}

impl HorizontalStickFactory {
    #[must_use]
    pub const fn new(parameters: HorizontalStickParameters) -> Self {
        Self { parameters }
    }

    /// Java `retrieveSticks` for horizontal sections, including registration
    /// before thickening. Failures return the exact visible prefix.
    #[must_use]
    pub fn retrieve_sticks(
        &self,
        sections: &[Section],
        first_creation_id: u64,
    ) -> HorizontalStickOutcome {
        let mut result = HorizontalStickResult {
            survivors: Vec::new(),
            creation_ids: Vec::new(),
            next_creation_id: first_creation_id,
        };
        if first_creation_id == 0 {
            return HorizontalStickOutcome {
                result,
                error: Some(StraightStickError::InvalidFirstId),
            };
        }
        if self.parameters.interline == 0
            || self.parameters.maximum_stick_thickness == 0
            || self.parameters.minimum_core_section_length == 0
            || !self.parameters.minimum_side_ratio.is_finite()
            || self.parameters.minimum_side_ratio < 0.0
        {
            return HorizontalStickOutcome {
                result,
                error: Some(StraightStickError::InvalidParameters),
            };
        }

        let mut linked = build_graph(sections);
        let mut cores = linked
            .iter()
            .enumerate()
            .filter(|(_, linked)| {
                linked.section.orientation() == Orientation::Horizontal
                    && linked.section.run_count() <= self.parameters.maximum_stick_thickness
                    && linked.section.length(Orientation::Horizontal)
                        >= self.parameters.minimum_core_section_length
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        // Java Collections.sort is stable and compares only decreasing length.
        cores.sort_by_key(|index| {
            std::cmp::Reverse(linked[*index].section.length(Orientation::Horizontal))
        });

        for core in cores {
            if linked[core].processed {
                continue;
            }
            let id = result.next_creation_id;
            let Some(next) = id.checked_add(1) else {
                return HorizontalStickOutcome {
                    result,
                    error: Some(StraightStickError::IdentityOverflow),
                };
            };
            result.next_creation_id = next;
            result.creation_ids.push(id);
            let mut filament = match StaffFilament::new(self.parameters.interline) {
                Ok(filament) => filament,
                Err(source) => {
                    return HorizontalStickOutcome {
                        result,
                        error: Some(source.into()),
                    };
                }
            };
            if let Err(source) = filament.add_section(linked[core].section.clone()) {
                return HorizontalStickOutcome {
                    result,
                    error: Some(source.into()),
                };
            }
            linked[core].processed = true;
            linked[core].compound_id = Some(id);
            if let Err(source) = self.thicken(&mut filament, id, &mut linked) {
                result
                    .survivors
                    .push(IdentifiedStraightStick { id, filament });
                return HorizontalStickOutcome {
                    result,
                    error: Some(source.into()),
                };
            }
            if let Err(source) = add_stickers(&mut filament, id, &mut linked) {
                result
                    .survivors
                    .push(IdentifiedStraightStick { id, filament });
                return HorizontalStickOutcome {
                    result,
                    error: Some(source.into()),
                };
            }
            result
                .survivors
                .push(IdentifiedStraightStick { id, filament });
        }
        HorizontalStickOutcome {
            result,
            error: None,
        }
    }

    fn thicken(
        &self,
        filament: &mut StaffFilament,
        filament_id: u64,
        linked: &mut [LinkedSection],
    ) -> Result<(), FilamentError> {
        let mut finished = [false, false];
        loop {
            let mut grown = false;
            for (side_index, reverse) in [true, false].into_iter().enumerate() {
                if finished[side_index] {
                    continue;
                }
                let mean_thickness = java_rint(mean_thickness(filament)?) as usize;
                let side_sections = side_sections(filament, filament_id, linked, reverse);
                let mut all_neighbors = BTreeSet::new();
                let mut contributions = vec![0_usize; linked.len()];
                let mut total = 0_usize;
                let mut count = 0_usize;
                for side in side_sections {
                    let side_run = if reverse {
                        linked[side].section.first_run()
                    } else {
                        linked[side].section.last_run()
                    };
                    // Java removes rejected entries from the live neighbor list.
                    let original = neighbors(linked, side, reverse).to_vec();
                    let mut retained = Vec::new();
                    for neighbor in original {
                        let thickness = linked[neighbor].section.run_count();
                        if linked[neighbor].processed
                            || thickness + mean_thickness > self.parameters.maximum_stick_thickness
                        {
                            continue;
                        }
                        retained.push(neighbor);
                        let length = linked[neighbor].section.length(Orientation::Horizontal);
                        count += length;
                        total += thickness * length;
                        let neighbor_run = if reverse {
                            linked[neighbor].section.last_run()
                        } else {
                            linked[neighbor].section.first_run()
                        };
                        contributions[neighbor] +=
                            usize::try_from(side_run.common_length(neighbor_run)).unwrap_or(0);
                    }
                    *neighbors_mut(linked, side, reverse) = retained.clone();
                    all_neighbors.extend(retained);
                }

                let mut accepted = all_neighbors.into_iter().collect::<Vec<_>>();
                let mut common_length = 0_usize;
                if count != 0 {
                    let sibling_mean = java_rint(total as f64 / count as f64) as usize;
                    accepted.retain(|neighbor| {
                        if linked[*neighbor].section.run_count() > sibling_mean {
                            false
                        } else {
                            common_length += contributions[*neighbor];
                            true
                        }
                    });
                }
                accepted.sort_by_key(|index| section_coordinate_key(&linked[*index].section));
                let ratio = common_length as f64 / filament.bounds()?.width as f64;
                if ratio >= self.parameters.minimum_side_ratio {
                    for neighbor in accepted {
                        filament.add_section(linked[neighbor].section.clone())?;
                        linked[neighbor].processed = true;
                        linked[neighbor].compound_id = Some(filament_id);
                    }
                    grown = true;
                } else {
                    finished[side_index] = true;
                }
            }
            if !grown
                || java_rint(mean_thickness(filament)?) as usize
                    >= self.parameters.maximum_stick_thickness
            {
                break;
            }
        }
        Ok(())
    }
}

pub struct VerticalStickFactory {
    parameters: VerticalStickParameters,
}

impl VerticalStickFactory {
    #[must_use]
    pub const fn new(parameters: VerticalStickParameters) -> Self {
        Self { parameters }
    }

    /// Java `retrieveSticks(verticals, horizontalStickers)`. The vertical
    /// input must already be in lag/full-position order, as `SystemInfo`
    /// exposes it.
    #[must_use]
    pub fn retrieve_sticks(
        &self,
        vertical_sections: &[Section],
        horizontal_stickers: &[Section],
        first_creation_id: u64,
    ) -> VerticalStickOutcome {
        let mut result = VerticalStickResult {
            survivors: Vec::new(),
            creation_ids: Vec::new(),
            next_creation_id: first_creation_id,
        };
        if first_creation_id == 0 {
            return VerticalStickOutcome {
                result,
                error: Some(StraightStickError::InvalidFirstId),
            };
        }
        if self.parameters.interline == 0
            || self.parameters.maximum_stick_thickness == 0
            || !self.parameters.minimum_side_ratio.is_finite()
            || self.parameters.minimum_side_ratio < 0.0
            || vertical_sections
                .iter()
                .any(|section| section.orientation() != Orientation::Vertical)
            || horizontal_stickers
                .iter()
                .any(|section| section.orientation() != Orientation::Horizontal)
        {
            return VerticalStickOutcome {
                result,
                error: Some(StraightStickError::InvalidParameters),
            };
        }

        let mut linked = build_graph(vertical_sections);
        let mut cores = linked
            .iter()
            .enumerate()
            .filter(|(_, linked)| {
                linked.section.run_count() <= self.parameters.maximum_stick_thickness
                    && linked.section.length(Orientation::Vertical)
                        >= self.parameters.minimum_core_section_length
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        // Java's object sort is stable and compares only decreasing length.
        cores.sort_by_key(|index| {
            std::cmp::Reverse(linked[*index].section.length(Orientation::Vertical))
        });
        let stickers_by_x = opposite_stickers(horizontal_stickers);

        for core in cores {
            if linked[core].processed {
                continue;
            }
            let id = result.next_creation_id;
            let Some(next) = id.checked_add(1) else {
                return VerticalStickOutcome {
                    result,
                    error: Some(StraightStickError::IdentityOverflow),
                };
            };
            result.next_creation_id = next;
            result.creation_ids.push(id);
            let mut sections = Vec::new();
            add_vertical_member(&mut sections, linked[core].section.clone());
            linked[core].processed = true;
            linked[core].compound_id = Some(id);
            thicken_vertical(&self.parameters, &mut sections, id, &mut linked);
            add_vertical_stickers(
                &mut sections,
                id,
                &mut linked,
                horizontal_stickers,
                &stickers_by_x,
            );
            result
                .survivors
                .push(IdentifiedVerticalStick { id, sections });
        }

        VerticalStickOutcome {
            result,
            error: None,
        }
    }
}

fn thicken_vertical(
    parameters: &VerticalStickParameters,
    sections: &mut Vec<Section>,
    filament_id: u64,
    linked: &mut [LinkedSection],
) {
    let mut finished = [false, false];
    loop {
        let mut grown = false;
        for (side_index, reverse) in [true, false].into_iter().enumerate() {
            if finished[side_index] {
                continue;
            }
            let mean_thickness = java_rint(vertical_mean_thickness(sections)) as usize;
            let sides = vertical_side_sections(sections, filament_id, linked, reverse);
            // Java uses TreeSet(Section.byCoordinate). Equal start coordinates
            // compare equal; the first encountered section survives.
            let mut all_neighbors = BTreeMap::<usize, usize>::new();
            let mut contributions = vec![0_usize; linked.len()];
            let mut total = 0_usize;
            let mut count = 0_usize;
            for side in sides {
                let side_run = if reverse {
                    linked[side].section.first_run()
                } else {
                    linked[side].section.last_run()
                };
                let original = neighbors(linked, side, reverse).to_vec();
                let mut retained = Vec::new();
                for neighbor in original {
                    let thickness = linked[neighbor].section.run_count();
                    if linked[neighbor].processed
                        || thickness + mean_thickness > parameters.maximum_stick_thickness
                    {
                        continue;
                    }
                    retained.push(neighbor);
                    let length = linked[neighbor].section.length(Orientation::Vertical);
                    count += length;
                    total += thickness * length;
                    let neighbor_run = if reverse {
                        linked[neighbor].section.last_run()
                    } else {
                        linked[neighbor].section.first_run()
                    };
                    contributions[neighbor] +=
                        usize::try_from(side_run.common_length(neighbor_run)).unwrap_or(0);
                }
                *neighbors_mut(linked, side, reverse) = retained.clone();
                for neighbor in retained {
                    all_neighbors
                        .entry(linked[neighbor].section.start_coord())
                        .or_insert(neighbor);
                }
            }

            let mut accepted = all_neighbors.into_values().collect::<Vec<_>>();
            let mut common_length = 0_usize;
            if count != 0 {
                let sibling_mean = java_rint(total as f64 / count as f64) as usize;
                accepted.retain(|neighbor| {
                    if linked[*neighbor].section.run_count() > sibling_mean {
                        false
                    } else {
                        common_length += contributions[*neighbor];
                        true
                    }
                });
            }
            let ratio = common_length as f64 / vertical_bounds(sections).height as f64;
            if ratio >= parameters.minimum_side_ratio {
                for neighbor in accepted {
                    add_vertical_member(sections, linked[neighbor].section.clone());
                    linked[neighbor].processed = true;
                }
                grown = true;
            } else {
                finished[side_index] = true;
            }
        }
        if !grown
            || java_rint(vertical_mean_thickness(sections)) as usize
                >= parameters.maximum_stick_thickness
        {
            break;
        }
    }
}

fn vertical_side_sections(
    sections: &[Section],
    filament_id: u64,
    linked: &[LinkedSection],
    reverse: bool,
) -> Vec<usize> {
    sections
        .iter()
        .filter(|section| section.orientation() == Orientation::Vertical)
        .filter_map(|section| {
            let index = linked
                .iter()
                .position(|candidate| candidate.section == *section)?;
            (!neighbors(linked, index, reverse)
                .iter()
                .any(|neighbor| linked[*neighbor].compound_id == Some(filament_id)))
            .then_some(index)
        })
        .collect()
}

#[derive(Clone, Copy)]
enum VerticalSticker {
    Main(usize),
    Opposite(usize),
}

fn add_vertical_stickers(
    sections: &mut Vec<Section>,
    filament_id: u64,
    linked: &mut [LinkedSection],
    opposite: &[Section],
    opposite_by_x: &BTreeMap<usize, Vec<usize>>,
) {
    let members = sections.clone();
    let mut stickers = Vec::<VerticalSticker>::new();
    for reverse in [true, false] {
        for member in &members {
            let Some(index) = linked
                .iter()
                .position(|candidate| candidate.section == *member)
            else {
                continue;
            };
            for neighbor in neighbors(linked, index, reverse).iter().copied() {
                // Java deliberately checks the `compound` link here, not the
                // processed flag. A section accepted while thickening is
                // marked processed without having its compound assigned, so
                // a later filament can still reuse it as an isolated sticker.
                if linked[neighbor].compound_id.is_none()
                    && linked[neighbor].section.run_count() == 1
                    && neighbors(linked, neighbor, reverse).is_empty()
                {
                    push_vertical_sticker(
                        &mut stickers,
                        VerticalSticker::Main(neighbor),
                        linked,
                        opposite,
                    );
                }
            }

            let run = if reverse {
                member.first_run()
            } else {
                member.last_run()
            };
            let x = if reverse {
                member.first_pos().checked_sub(1)
            } else {
                member.last_pos().checked_add(1)
            };
            if let Some(indices) = x.and_then(|x| opposite_by_x.get(&x)) {
                for &sticker in indices {
                    if section_intersects_vertical_strip(&opposite[sticker], run.start, run.stop())
                    {
                        push_vertical_sticker(
                            &mut stickers,
                            VerticalSticker::Opposite(sticker),
                            linked,
                            opposite,
                        );
                    }
                }
            }
        }
    }
    for sticker in stickers {
        match sticker {
            VerticalSticker::Main(index) => {
                add_vertical_member(sections, linked[index].section.clone());
                linked[index].processed = true;
                linked[index].compound_id = Some(filament_id);
            }
            VerticalSticker::Opposite(index) => {
                add_vertical_member(sections, opposite[index].clone());
            }
        }
    }
}

fn push_vertical_sticker(
    stickers: &mut Vec<VerticalSticker>,
    candidate: VerticalSticker,
    linked: &[LinkedSection],
    opposite: &[Section],
) {
    let section = |sticker: VerticalSticker| match sticker {
        VerticalSticker::Main(index) => &linked[index].section,
        VerticalSticker::Opposite(index) => &opposite[index],
    };
    let key = compound_member_key(section(candidate));
    if !stickers
        .iter()
        .any(|existing| compound_member_key(section(*existing)) == key)
    {
        stickers.push(candidate);
    }
}

fn opposite_stickers(sections: &[Section]) -> BTreeMap<usize, Vec<usize>> {
    let mut indices = (0..sections.len()).collect::<Vec<_>>();
    indices.sort_by_key(|index| sections[*index].start_coord());
    let mut by_x = BTreeMap::<usize, Vec<usize>>::new();
    for index in indices {
        by_x.entry(sections[index].start_coord())
            .or_default()
            .push(index);
    }
    by_x
}

fn section_intersects_vertical_strip(section: &Section, y_start: usize, y_stop: usize) -> bool {
    section.runs().iter().enumerate().any(|(offset, run)| {
        let position = section.first_pos() + offset;
        match section.orientation() {
            Orientation::Horizontal => position >= y_start && position <= y_stop && run.length != 0,
            Orientation::Vertical => {
                run.start <= y_stop && run.stop() >= y_start && section.run_count() != 0
            }
        }
    })
}

fn compound_member_key(section: &Section) -> (usize, usize, usize) {
    let bounds = section.bounds();
    (bounds.x, bounds.y, section.id())
}

fn add_vertical_member(sections: &mut Vec<Section>, section: Section) {
    let key = compound_member_key(&section);
    if !sections
        .iter()
        .any(|member| compound_member_key(member) == key)
    {
        sections.push(section);
        sections.sort_by_key(compound_member_key);
    }
}

fn vertical_bounds(sections: &[Section]) -> Bounds {
    let first = sections
        .first()
        .expect("a vertical stick has a core")
        .bounds();
    let mut min_x = first.x;
    let mut min_y = first.y;
    let mut max_x = first.x + first.width - 1;
    let mut max_y = first.y + first.height - 1;
    for section in &sections[1..] {
        let bounds = section.bounds();
        min_x = min_x.min(bounds.x);
        min_y = min_y.min(bounds.y);
        max_x = max_x.max(bounds.x + bounds.width - 1);
        max_y = max_y.max(bounds.y + bounds.height - 1);
    }
    Bounds {
        x: min_x,
        y: min_y,
        width: max_x - min_x + 1,
        height: max_y - min_y + 1,
    }
}

fn vertical_mean_thickness(sections: &[Section]) -> f64 {
    sections.iter().map(Section::weight).sum::<usize>() as f64
        / vertical_bounds(sections).height as f64
}

fn vertical_straight_geometry(
    filament: &IdentifiedVerticalStick,
) -> Result<StraightStickGeometry, StraightStickError> {
    let bounds = vertical_bounds(&filament.sections);
    let mut line = BasicLine::default();
    for section in &filament.sections {
        for (offset, run) in section.runs().iter().enumerate() {
            let position = section.first_pos() + offset;
            for coordinate in run.start..=run.stop() {
                let (x, y) = match section.orientation() {
                    Orientation::Horizontal => (coordinate, position),
                    Orientation::Vertical => (position, coordinate),
                };
                line.include_point(x as f64, y as f64);
            }
        }
    }
    let top = bounds.y as f64;
    let bottom = (bounds.y + bounds.height - 1) as f64;
    let start = if filament.weight() <= 1 {
        (bounds.x as f64, top)
    } else {
        (line.x_at_y(top)?, top)
    };
    let stop = if filament.weight() <= 1 {
        start
    } else {
        (line.x_at_y(bottom)?, bottom)
    };
    Ok(StraightStickGeometry {
        bounds,
        start,
        stop,
        mean_thickness: vertical_mean_thickness(&filament.sections),
        mean_distance: if filament.weight() <= 1 {
            0.0
        } else {
            line.mean_distance()?
        },
    })
}

fn build_graph(sections: &[Section]) -> Vec<LinkedSection> {
    let mut linked = sections
        .iter()
        .cloned()
        .map(|section| LinkedSection {
            section,
            sources: Vec::new(),
            targets: Vec::new(),
            processed: false,
            compound_id: None,
        })
        .collect::<Vec<_>>();
    for source in 0..linked.len() {
        let next_position = linked[source].section.last_pos() + 1;
        let predecessor = linked[source].section.last_run();
        let mut targets = (0..linked.len())
            .filter(|target| linked[*target].section.first_pos() == next_position)
            .collect::<Vec<_>>();
        targets.sort_by_key(|target| section_coordinate_key(&linked[*target].section));
        for target in targets {
            let successor = linked[target].section.first_run();
            if successor.start > predecessor.stop() {
                break;
            }
            if successor.stop() >= predecessor.start {
                linked[source].targets.push(target);
                linked[target].sources.push(source);
            }
        }
    }
    linked
}

fn side_sections(
    filament: &StaffFilament,
    filament_id: u64,
    linked: &[LinkedSection],
    reverse: bool,
) -> Vec<usize> {
    filament
        .sections()
        .iter()
        .filter_map(|section| {
            let index = linked
                .iter()
                .position(|candidate| candidate.section == *section)?;
            (!neighbors(linked, index, reverse)
                .iter()
                .any(|neighbor| linked[*neighbor].compound_id == Some(filament_id)))
            .then_some(index)
        })
        .collect()
}

fn add_stickers(
    filament: &mut StaffFilament,
    filament_id: u64,
    linked: &mut [LinkedSection],
) -> Result<(), FilamentError> {
    let mut stickers = Vec::new();
    for reverse in [true, false] {
        let members = filament.sections().to_vec();
        for member in members {
            let Some(index) = linked
                .iter()
                .position(|candidate| candidate.section == member)
            else {
                continue;
            };
            for neighbor in neighbors(linked, index, reverse).iter().copied() {
                if !linked[neighbor].processed
                    && linked[neighbor].section.run_count() == 1
                    && neighbors(linked, neighbor, reverse).is_empty()
                    && !stickers.contains(&neighbor)
                {
                    stickers.push(neighbor);
                }
            }
        }
    }
    for sticker in stickers {
        filament.add_section(linked[sticker].section.clone())?;
        linked[sticker].processed = true;
        linked[sticker].compound_id = Some(filament_id);
    }
    Ok(())
}

fn neighbors(linked: &[LinkedSection], index: usize, reverse: bool) -> &[usize] {
    if reverse {
        &linked[index].sources
    } else {
        &linked[index].targets
    }
}

fn neighbors_mut(linked: &mut [LinkedSection], index: usize, reverse: bool) -> &mut Vec<usize> {
    if reverse {
        &mut linked[index].sources
    } else {
        &mut linked[index].targets
    }
}

fn section_coordinate_key(section: &Section) -> (usize, usize, usize) {
    (section.start_coord(), section.first_pos(), section.id())
}

fn mean_thickness(filament: &StaffFilament) -> Result<f64, FilamentError> {
    Ok(filament.weight() as f64 / filament.bounds()?.width as f64)
}

fn straight_geometry(
    filament: &StaffFilament,
) -> Result<StraightStickGeometry, StraightStickError> {
    let bounds = filament.bounds()?;
    let mut line = BasicLine::default();
    for section in filament.sections() {
        for (offset, run) in section.runs().iter().enumerate() {
            let y = (section.first_pos() + offset) as f64;
            for x in run.start..=run.stop() {
                line.include_point(x as f64, y);
            }
        }
    }
    let left = bounds.x as f64;
    let right = (bounds.x + bounds.width - 1) as f64;
    let start = if filament.weight() <= 1 {
        (left, bounds.y as f64)
    } else {
        (left, line.y_at_x(left)?)
    };
    let stop = if filament.weight() <= 1 {
        start
    } else {
        (right, line.y_at_x(right)?)
    };
    Ok(StraightStickGeometry {
        bounds,
        start,
        stop,
        mean_thickness: mean_thickness(filament)?,
        mean_distance: if filament.weight() <= 1 {
            0.0
        } else {
            line.mean_distance()?
        },
    })
}

fn java_rint(value: f64) -> i32 {
    value.round_ties_even() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        run_table::{Run, RunTable},
        section::{JunctionPolicy, build_sections_from_id},
    };

    fn sections() -> Vec<Section> {
        let mut table = RunTable::new(Orientation::Horizontal, 40, 8).unwrap();
        // Core A and B in Java full-position order.
        assert!(table.add_run(2, Run::new(0, 10)).unwrap());
        assert!(table.add_run(2, Run::new(20, 8)).unwrap());
        // Full-width side for A, then a one-pixel sticker.
        assert!(table.add_run(3, Run::new(0, 10)).unwrap());
        assert!(table.add_run(4, Run::new(0, 1)).unwrap());
        build_sections_from_id(&table, JunctionPolicy::Shift { max_shift: 0 }, 1)
    }

    fn factory() -> HorizontalStickFactory {
        HorizontalStickFactory::new(HorizontalStickParameters {
            interline: 10,
            maximum_stick_thickness: 3,
            minimum_core_section_length: 6,
            minimum_side_ratio: 0.9,
        })
    }

    #[test]
    fn stable_core_registration_precedes_thickening_and_sticker_attachment() {
        let outcome = factory().retrieve_sticks(&sections(), 7);

        assert_eq!(outcome.error, None);
        assert_eq!(outcome.result.creation_ids(), [7, 8]);
        assert_eq!(outcome.result.next_creation_id(), 9);
        assert_eq!(outcome.result.survivors().len(), 2);
        let first = &outcome.result.survivors()[0];
        assert_eq!(first.id(), 7);
        assert_eq!(
            first
                .filament()
                .sections()
                .iter()
                .map(Section::id)
                .collect::<Vec<_>>(),
            vec![1, 3]
        );
        let second = &outcome.result.survivors()[1];
        assert_eq!(second.id(), 8);
        assert_eq!(
            second
                .filament()
                .sections()
                .iter()
                .map(Section::id)
                .collect::<Vec<_>>(),
            vec![2]
        );
        let geometry = first.straight_geometry().unwrap();
        assert_eq!(geometry.bounds.x, 0);
        assert_eq!(geometry.bounds.width, 10);
        assert_eq!(geometry.start.0, 0.0);
        assert_eq!(geometry.stop.0, 9.0);
        assert!((geometry.mean_thickness - 2.1).abs() < 1e-12);
    }

    #[test]
    fn equal_length_cores_keep_source_order() {
        let source = sections();
        let mut equal = vec![source[1].clone(), source[0].clone()];
        // Both are isolated, equal-length core probes.
        let mut table = RunTable::new(Orientation::Horizontal, 40, 2).unwrap();
        assert!(table.add_run(0, Run::new(30, 10)).unwrap());
        equal.push(
            build_sections_from_id(&table, JunctionPolicy::Shift { max_shift: 0 }, 20).remove(0),
        );
        // Drop the shorter first entry, leaving deliberately reversed equal cores.
        equal.remove(0);
        let outcome = factory().retrieve_sticks(&equal, 1);

        assert_eq!(outcome.error, None);
        assert_eq!(outcome.result.creation_ids(), [1, 2]);
        assert_eq!(
            outcome.result.survivors()[0].filament().sections()[0].id(),
            source[0].id()
        );
        assert_eq!(
            outcome.result.survivors()[1].filament().sections()[0].id(),
            20
        );
    }

    #[test]
    fn identity_overflow_returns_registered_survivor_prefix() {
        let outcome = factory().retrieve_sticks(&sections(), u64::MAX - 1);

        assert_eq!(outcome.error, Some(StraightStickError::IdentityOverflow));
        assert_eq!(outcome.result.creation_ids(), [u64::MAX - 1]);
        assert_eq!(outcome.result.survivors().len(), 1);
        assert_eq!(outcome.result.survivors()[0].id(), u64::MAX - 1);
        assert_eq!(outcome.result.next_creation_id(), u64::MAX);
    }

    #[test]
    fn invalid_parameters_fail_before_registration() {
        let factory = HorizontalStickFactory::new(HorizontalStickParameters {
            interline: 0,
            maximum_stick_thickness: 3,
            minimum_core_section_length: 6,
            minimum_side_ratio: 0.9,
        });
        let outcome = factory.retrieve_sticks(&sections(), 1);
        assert_eq!(outcome.error, Some(StraightStickError::InvalidParameters));
        assert!(outcome.result.creation_ids().is_empty());
        assert!(outcome.result.survivors().is_empty());
    }

    #[test]
    fn vertical_factory_accepts_zero_minimum_core_for_stem_builder_chunks() {
        let mut table = RunTable::new(Orientation::Vertical, 2, 12).unwrap();
        assert!(table.add_run(0, Run::new(1, 10)).unwrap());
        let vertical = build_sections_from_id(&table, JunctionPolicy::Shift { max_shift: 0 }, 1);
        let factory = VerticalStickFactory::new(VerticalStickParameters {
            interline: 10,
            maximum_stick_thickness: 2,
            minimum_core_section_length: 0,
            minimum_side_ratio: 0.4,
        });

        let outcome = factory.retrieve_sticks(&vertical, &[], 1);

        assert_eq!(outcome.error, None);
        assert_eq!(outcome.result.creation_ids(), [1]);
        assert_eq!(outcome.result.survivors().len(), 1);
    }

    #[test]
    fn vertical_sticker_uses_java_compound_state_not_processed_state() {
        let mut table = RunTable::new(Orientation::Vertical, 2, 5).unwrap();
        assert!(table.add_run(0, Run::new(0, 4)).unwrap());
        assert!(table.add_run(1, Run::new(1, 1)).unwrap());
        let sections = build_sections_from_id(&table, JunctionPolicy::Shift { max_shift: 0 }, 1);
        assert_eq!(sections.len(), 2);
        let mut linked = build_graph(&sections);
        linked[1].processed = true;
        assert_eq!(linked[1].compound_id, None);
        let mut members = vec![sections[0].clone()];

        add_vertical_stickers(&mut members, 7, &mut linked, &[], &BTreeMap::new());

        assert_eq!(members.iter().map(Section::id).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(linked[1].compound_id, Some(7));
    }
}
