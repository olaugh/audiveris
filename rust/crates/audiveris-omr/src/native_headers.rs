// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production inputs for native HEADERS, derived only from GRID state.
//!
//! The visual clef/key/time recognizers need a precise header start and a
//! staff's first good connected barline after that start.  The corpus harness
//! used to supply both from its Java oracle.  This module closes that input
//! seam: it adapts the live GRID sheet/SIG into [`HeadlessHeaderSystem`] state,
//! runs Java's ported `computeHeaderStarts`, and records the bar starts that
//! `Staff.getBrowseStop` is allowed to use.

use std::{convert::Infallible, error::Error, fmt};

use audiveris_image::{
    bars_logic::VerticalInterKind,
    grid_sig::{GridInterId, GridSigNode, GridSigRelation},
    lines_coordinator::StaffCandidateKind,
};

use crate::{
    clef_parameters::SheetClefParameters,
    header_builder::{
        HeaderBarGroupRelation, HeaderBarline, HeaderBuilderError, HeaderHorizontalSide,
        compute_header_starts,
    },
    headers_step::{HeadlessHeaderStaff, HeadlessHeaderSystem},
    recognize::{GridLinesRecognition, StaffLineGeometry},
    staff_header::HeaderBounds,
};

/// Java `BarlineInter.isGood()`'s hard-coded threshold.
///
/// This deliberately is not `Grades.goodInterGrade` (0.4).  BarlineInter
/// overrides the general predicate with `getGrade() >= 0.6`.
pub const GOOD_BARLINE_GRADE: f64 = 0.6;

/// All production HEADERS positioning inputs for one sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeaderGridContext {
    pub sheet_interline: i32,
    /// Systems in Java/GRID source order.
    pub systems: Vec<NativeHeaderGridSystem>,
}

/// GRID-derived state for one invocation of Java `HeaderBuilder(system)`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeaderGridSystem {
    pub system_id: usize,
    /// Barline/SIG state with `StaffHeader.start` already computed.
    pub header_system: HeadlessHeaderSystem,
    /// Staff inputs in system staff order.
    pub staffs: Vec<NativeHeaderGridStaff>,
}

/// Geometry and browse-stop inputs retained past header-start construction.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeaderGridStaff {
    pub staff_id: usize,
    pub specific_interline: i32,
    pub lines: StaffLineGeometry,
    /// Ordered `BarlineInter.getBounds().x` values accepted by
    /// `Staff.getBrowseStop`: grade at least 0.6 and a bar-connection edge.
    pub good_connected_bar_starts: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeHeaderGridContextError {
    InvalidSheetInterline(i32),
    MissingSystem(usize),
    MissingStaff {
        system_id: usize,
        staff_id: usize,
    },
    MissingStaffLines {
        system_id: usize,
        staff_id: usize,
    },
    InvalidStaffInterline {
        system_id: usize,
        staff_id: usize,
        value: usize,
    },
    MissingStaffLineOrdinate {
        system_id: usize,
        staff_id: usize,
        x: i32,
    },
    MissingBarline {
        system_id: usize,
        staff_id: usize,
        inter_id: usize,
    },
    HeaderStart {
        system_id: usize,
        source: HeaderBuilderError<Infallible>,
    },
}

impl fmt::Display for NativeHeaderGridContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSheetInterline(value) => {
                write!(formatter, "invalid HEADERS sheet interline {value}")
            }
            Self::MissingSystem(id) => write!(formatter, "GRID system {id} is missing"),
            Self::MissingStaff {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "GRID system {system_id} is missing staff {staff_id}"
            ),
            Self::MissingStaffLines {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "GRID system {system_id} staff {staff_id} has no line geometry"
            ),
            Self::InvalidStaffInterline {
                system_id,
                staff_id,
                value,
            } => write!(
                formatter,
                "GRID system {system_id} staff {staff_id} has invalid interline {value}"
            ),
            Self::MissingStaffLineOrdinate {
                system_id,
                staff_id,
                x,
            } => write!(
                formatter,
                "GRID system {system_id} staff {staff_id} has no line ordinate at x={x}"
            ),
            Self::MissingBarline {
                system_id,
                staff_id,
                inter_id,
            } => write!(
                formatter,
                "GRID system {system_id} staff {staff_id} is missing barline inter {inter_id}"
            ),
            Self::HeaderStart { system_id, source } => {
                write!(
                    formatter,
                    "HEADERS system {system_id} start failed: {source}"
                )
            }
        }
    }
}

impl Error for NativeHeaderGridContextError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::HeaderStart { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Build HEADERS' positioning state without consulting a Java oracle.
pub fn derive_native_header_grid_context(
    grid: &GridLinesRecognition,
) -> Result<NativeHeaderGridContext, NativeHeaderGridContextError> {
    let sheet_interline = grid.scale.scale.interline.main;
    if sheet_interline <= 0 {
        return Err(NativeHeaderGridContextError::InvalidSheetInterline(
            sheet_interline,
        ));
    }
    let maximum_clef_end = SheetClefParameters::new(sheet_interline).max_clef_end;
    let mut systems = Vec::with_capacity(grid.peak_graph.systems.len());

    for (system_index, staff_ids) in grid.peak_graph.systems.iter().enumerate() {
        let system_id = system_index + 1;
        let grid_system = grid
            .peak_graph
            .sig
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .ok_or(NativeHeaderGridContextError::MissingSystem(system_id))?;

        let mut header_staffs = Vec::with_capacity(staff_ids.len());
        let mut staff_inputs = Vec::with_capacity(staff_ids.len());
        let mut barlines = Vec::new();
        let sig_vertex_ids = grid_system
            .sig
            .nodes_in_order()
            .map(|(id, _)| id.value())
            .collect();

        for &staff_id in staff_ids {
            let sheet_staff = grid
                .peak_graph
                .sheet_staffs
                .iter()
                .find(|staff| staff.id == staff_id)
                .ok_or(NativeHeaderGridContextError::MissingStaff {
                    system_id,
                    staff_id,
                })?;
            let lines = grid
                .staff_lines
                .iter()
                .find(|lines| lines.staff_id == staff_id)
                .cloned()
                .ok_or(NativeHeaderGridContextError::MissingStaffLines {
                    system_id,
                    staff_id,
                })?;
            let first_line_y_at_left = lines.first_line_y_at(lines.left).ok_or(
                NativeHeaderGridContextError::MissingStaffLineOrdinate {
                    system_id,
                    staff_id,
                    x: lines.left,
                },
            )?;
            let last_line_y_at_left = lines.last_line_y_at(lines.left).ok_or(
                NativeHeaderGridContextError::MissingStaffLineOrdinate {
                    system_id,
                    staff_id,
                    x: lines.left,
                },
            )?;

            let mut header_staff = HeadlessHeaderStaff::new(staff_id);
            header_staff.part_id = part_id_for_staff(grid_system, staff_id);
            header_staff.tablature = sheet_staff.kind == StaffCandidateKind::Tablature;
            header_staff.one_line = sheet_staff.kind == StaffCandidateKind::OneLine;
            header_staff.maximum_clef_end = maximum_clef_end;
            header_staff.left_abscissa = lines.left;
            header_staff.right_abscissa = lines.right;
            header_staff.first_line_y_at_left = first_line_y_at_left;
            header_staff.last_line_y_at_left = last_line_y_at_left;

            let mut good_connected_bar_starts = Vec::new();
            for &inter_id in &sheet_staff.barlines {
                let Some(GridSigNode::Vertical { plan, .. }) = grid_system.sig.node(inter_id)
                else {
                    return Err(NativeHeaderGridContextError::MissingBarline {
                        system_id,
                        staff_id,
                        inter_id: inter_id.value(),
                    });
                };
                let VerticalInterKind::Barline {
                    left_staff_end,
                    right_staff_end,
                    ..
                } = plan.kind
                else {
                    return Err(NativeHeaderGridContextError::MissingBarline {
                        system_id,
                        staff_id,
                        inter_id: inter_id.value(),
                    });
                };
                let bounds = vertical_barline_bounds(plan.median, plan.width);
                let grade = grid_system
                    .sig
                    .node(inter_id)
                    .expect("the vertical node was just resolved")
                    .intrinsic_grade();
                let staff_end = if left_staff_end {
                    Some(HeaderHorizontalSide::Left)
                } else if right_staff_end {
                    Some(HeaderHorizontalSide::Right)
                } else {
                    None
                };
                let id = inter_id.value();
                header_staff.barline_ids.push(id);
                if left_staff_end {
                    header_staff.left_side_barline_id = Some(id);
                }
                if right_staff_end {
                    header_staff.right_side_barline_id = Some(id);
                }
                if is_good_connected_barline(grade, has_bar_connection(&grid_system.sig, inter_id))
                {
                    good_connected_bar_starts.push(bounds.x);
                }
                barlines.push(HeaderBarline {
                    id,
                    bounds: Some(bounds),
                    staff_id: Some(staff_id),
                    dummy: false,
                    grade,
                    staff_end,
                    removed: false,
                });
            }

            let specific_interline = i32::try_from(sheet_staff.interline).map_err(|_| {
                NativeHeaderGridContextError::InvalidStaffInterline {
                    system_id,
                    staff_id,
                    value: sheet_staff.interline,
                }
            })?;
            if specific_interline <= 0 {
                return Err(NativeHeaderGridContextError::InvalidStaffInterline {
                    system_id,
                    staff_id,
                    value: sheet_staff.interline,
                });
            }
            staff_inputs.push(NativeHeaderGridStaff {
                staff_id,
                specific_interline,
                lines,
                good_connected_bar_starts,
            });
            header_staffs.push(header_staff);
        }

        let bar_group_relations = grid_system
            .sig
            .edges()
            .iter()
            .filter_map(|edge| {
                matches!(edge.relation, GridSigRelation::BarGroup { .. }).then_some(
                    HeaderBarGroupRelation {
                        source: edge.source.value(),
                        target: edge.target.value(),
                    },
                )
            })
            .collect();
        let last_inter_id = grid_system
            .sig
            .nodes_in_order()
            .last()
            .map_or(0, |(id, _)| id.value());
        let mut header_system = HeadlessHeaderSystem::new(system_id, header_staffs);
        header_system.last_inter_id = last_inter_id;
        header_system.barlines = barlines;
        header_system.sig_vertex_ids = sig_vertex_ids;
        header_system.bar_group_relations = bar_group_relations;
        compute_header_starts::<Infallible>(&mut header_system)
            .map_err(|source| NativeHeaderGridContextError::HeaderStart { system_id, source })?;

        systems.push(NativeHeaderGridSystem {
            system_id,
            header_system,
            staffs: staff_inputs,
        });
    }

    Ok(NativeHeaderGridContext {
        sheet_interline,
        systems,
    })
}

fn part_id_for_staff(
    system: &crate::grid_executor::HeadlessSystemSigState,
    staff_id: usize,
) -> usize {
    let Ok(staff_id_i32) = i32::try_from(staff_id) else {
        return staff_id;
    };
    system
        .bar_tail
        .parts
        .iter()
        .find(|part| part.first_staff_id <= staff_id_i32 && staff_id_i32 <= part.last_staff_id)
        .and_then(|part| usize::try_from(part.first_staff_id).ok())
        .unwrap_or(staff_id)
}

fn has_bar_connection(sig: &audiveris_image::grid_sig::GridSig, id: GridInterId) -> bool {
    sig.edges().iter().any(|edge| {
        (edge.source == id || edge.target == id)
            && matches!(edge.relation, GridSigRelation::BarConnectionSupport { .. })
    })
}

fn is_good_connected_barline(grade: f64, connected: bool) -> bool {
    grade >= GOOD_BARLINE_GRADE && connected
}

fn vertical_barline_bounds(
    median: audiveris_image::bars_logic::VerticalMedian,
    width: f64,
) -> HeaderBounds {
    let half_width = width / 2.0;
    let x = (median.x - half_width).floor() as i32;
    let right = (median.x + half_width).ceil() as i32;
    let y = median.top.floor() as i32;
    let bottom = median.bottom.ceil() as i32;
    HeaderBounds {
        x,
        y,
        width: right.saturating_sub(x),
        height: bottom.saturating_sub(y),
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::recognize::recognize_grid_lines;

    fn repo_path(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join(relative)
    }

    #[derive(Debug)]
    struct OracleStaffInput {
        id: usize,
        specific_interline: i32,
        header_start: i32,
        good_connected_bars: Vec<i32>,
    }

    fn oracle_header_inputs() -> Vec<(String, i32, Vec<OracleStaffInput>)> {
        let mut pages = Vec::<(String, i32, Vec<OracleStaffInput>)>::new();
        for line in include_str!("../../../oracle/clef-headers.txt").lines() {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            match fields.first().copied() {
                Some("page") => pages.push((
                    fields[1].to_owned(),
                    fields[2].parse().expect("sheet interline"),
                    Vec::new(),
                )),
                Some("staff") => {
                    pages
                        .last_mut()
                        .expect("staff follows page")
                        .2
                        .push(OracleStaffInput {
                            id: fields[1].parse().expect("staff id"),
                            specific_interline: fields[2].parse().expect("staff interline"),
                            header_start: fields[3].parse().expect("header start"),
                            good_connected_bars: Vec::new(),
                        })
                }
                Some("bars") => {
                    let page = pages.last_mut().expect("bars follow page");
                    let id = fields[1].parse::<usize>().expect("staff id");
                    page.2
                        .iter_mut()
                        .find(|staff| staff.id == id)
                        .expect("bars name an existing staff")
                        .good_connected_bars = fields[2..]
                        .iter()
                        .filter_map(|value| value.parse().ok())
                        .collect();
                }
                _ => {}
            }
        }
        pages
    }

    #[test]
    fn good_barline_threshold_is_inclusive_and_connection_is_required() {
        assert!(!is_good_connected_barline(0.6 - f64::EPSILON, true));
        assert!(is_good_connected_barline(0.6, true));
        assert!(is_good_connected_barline(0.9, true));
        assert!(!is_good_connected_barline(0.9, false));
    }

    #[test]
    fn chula_header_inputs_are_grid_only_exact_and_deterministic() {
        let grid = recognize_grid_lines(repo_path("data/examples/chula.png"))
            .expect("chula GRID recognition");
        let one = derive_native_header_grid_context(&grid).expect("HEADERS grid context");
        let two = derive_native_header_grid_context(&grid).expect("repeat derivation");
        assert_eq!(one, two);
        assert_eq!(one.sheet_interline, 21);
        assert_eq!(one.systems.len(), 3);

        let starts = one
            .systems
            .iter()
            .flat_map(|system| &system.header_system.staffs)
            .map(|staff| {
                (
                    staff.id,
                    staff.header.as_ref().expect("computed header").start,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            starts,
            [(1, 203), (2, 202), (3, 89), (4, 87), (5, 84), (6, 83)]
        );

        let good_bars = one
            .systems
            .iter()
            .flat_map(|system| &system.staffs)
            .map(|staff| (staff.staff_id, staff.good_connected_bar_starts.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            good_bars,
            [
                (
                    1,
                    vec![200, 464, 831, 1173, 1544, 1803, 1812, 1828, 1961, 2324]
                ),
                (
                    2,
                    vec![199, 463, 831, 1173, 1544, 1803, 1812, 1827, 1961, 2323]
                ),
                (3, vec![85, 557, 984, 1281, 1291, 1451, 1901, 2323]),
                (4, vec![84, 556, 984, 1281, 1290, 1450, 1900, 2323]),
                (5, vec![81, 606, 976, 1343, 1667, 2032, 2310, 2322]),
                (6, vec![80, 606, 975, 1343, 1666, 2032, 2310, 2321]),
            ]
        );
        assert!(
            one.systems
                .iter()
                .flat_map(|system| &system.staffs)
                .all(|staff| staff.specific_interline == 21)
        );
    }

    #[test]
    fn grid_derives_every_corpus_header_input_the_oracle_recorded() {
        let pages = oracle_header_inputs();
        let mut checked = 0;
        assert_eq!(pages.len(), 9, "all example pages are pinned");

        for (name, sheet_interline, oracle_staffs) in pages {
            let grid = recognize_grid_lines(repo_path(&format!("data/examples/{name}")))
                .unwrap_or_else(|error| panic!("{name}: GRID failed: {error}"));
            let context = derive_native_header_grid_context(&grid)
                .unwrap_or_else(|error| panic!("{name}: HEADERS context failed: {error}"));
            assert_eq!(context.sheet_interline, sheet_interline, "{name}");

            for oracle in oracle_staffs {
                let system = context
                    .systems
                    .iter()
                    .find(|system| {
                        system
                            .header_system
                            .staffs
                            .iter()
                            .any(|staff| staff.id == oracle.id)
                    })
                    .unwrap_or_else(|| panic!("{name}: no system owns staff {}", oracle.id));
                let header_staff = system
                    .header_system
                    .staffs
                    .iter()
                    .find(|staff| staff.id == oracle.id)
                    .expect("the owning system contains the staff");
                let input = system
                    .staffs
                    .iter()
                    .find(|staff| staff.staff_id == oracle.id)
                    .expect("the production context contains the staff");
                assert_eq!(
                    header_staff.header.as_ref().map(|header| header.start),
                    Some(oracle.header_start),
                    "{name} staff {} header start",
                    oracle.id
                );
                assert_eq!(
                    input.specific_interline, oracle.specific_interline,
                    "{name} staff {} specific interline",
                    oracle.id
                );
                assert_eq!(
                    input.good_connected_bar_starts, oracle.good_connected_bars,
                    "{name} staff {} good connected bars",
                    oracle.id
                );
                checked += 1;
            }
        }

        assert_eq!(checked, 65, "every corpus staff was compared");
    }
}
