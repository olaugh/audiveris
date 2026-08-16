// SPDX-License-Identifier: AGPL-3.0-or-later

//! Explicit scan-to-canonical geometry for curved music systems.

use audiveris_image::staff_geometry_trace::{StaffTraceColumn, TracedStaffGeometry};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WarpAnchor {
    pub staff_id: usize,
    pub line_index: usize,
    pub scan_y: f64,
    /// Canonical ordinate in staff-space units. Adjacent staff lines differ by 1.
    pub canonical_v: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WarpMeshColumn {
    pub scan_x: f64,
    /// Canonical abscissa in staff-space units, measured by centerline arc length.
    pub canonical_u: f64,
    pub anchors: Vec<WarpAnchor>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SystemGeometryModel {
    pub system_id: usize,
    pub staff_ids: Vec<usize>,
    pub scan_left: i32,
    pub scan_right: i32,
    pub scan_top: f64,
    pub scan_bottom: f64,
    pub canonical_width: f64,
    pub canonical_height: f64,
    pub canonical_interline_pixels: f64,
    pub columns: Vec<WarpMeshColumn>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageGeometryModel {
    pub staves: Vec<TracedStaffGeometry>,
    pub systems: Vec<SystemGeometryModel>,
}

impl SystemGeometryModel {
    /// Map a point from canonical staff-space coordinates back to the scan.
    #[must_use]
    pub fn canonical_to_scan(&self, u: f64, v: f64) -> Option<(f64, f64)> {
        let pair = bracket_columns(&self.columns, u, |column| column.canonical_u)?;
        let du = pair[1].canonical_u - pair[0].canonical_u;
        let ratio = if du == 0.0 {
            0.0
        } else {
            (u - pair[0].canonical_u) / du
        };
        let x = pair[0].scan_x + ratio * (pair[1].scan_x - pair[0].scan_x);
        let left_y = interpolate_anchor(&pair[0].anchors, v, true)?;
        let right_y = interpolate_anchor(&pair[1].anchors, v, true)?;
        Some((x, left_y + ratio * (right_y - left_y)))
    }

    /// Map a scan point into the canonical system coordinate space.
    #[must_use]
    pub fn scan_to_canonical(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let pair = bracket_columns(&self.columns, x, |column| column.scan_x)?;
        let dx = pair[1].scan_x - pair[0].scan_x;
        let ratio = if dx == 0.0 {
            0.0
        } else {
            (x - pair[0].scan_x) / dx
        };
        let u = pair[0].canonical_u + ratio * (pair[1].canonical_u - pair[0].canonical_u);
        let left_v = interpolate_anchor(&pair[0].anchors, y, false)?;
        let right_v = interpolate_anchor(&pair[1].anchors, y, false)?;
        Some((u, left_v + ratio * (right_v - left_v)))
    }
}

fn bracket_columns<T, F>(items: &[T], value: f64, ordinate: F) -> Option<[&T; 2]>
where
    F: Fn(&T) -> f64,
{
    let first = items.first()?;
    let last = items.last()?;
    if items.len() == 1 {
        return Some([first, first]);
    }
    if value <= ordinate(first) {
        return Some([first, &items[1]]);
    }
    if value >= ordinate(last) {
        return Some([&items[items.len() - 2], last]);
    }
    let right = items.partition_point(|item| ordinate(item) < value);
    Some([&items[right - 1], &items[right]])
}

fn interpolate_anchor(anchors: &[WarpAnchor], value: f64, canonical_to_scan: bool) -> Option<f64> {
    let ordinate = |anchor: &WarpAnchor| {
        if canonical_to_scan {
            anchor.canonical_v
        } else {
            anchor.scan_y
        }
    };
    let pair = bracket_columns(anchors, value, ordinate)?;
    let input_left = ordinate(pair[0]);
    let input_right = ordinate(pair[1]);
    let ratio = if input_right == input_left {
        0.0
    } else {
        (value - input_left) / (input_right - input_left)
    };
    let output_left = if canonical_to_scan {
        pair[0].scan_y
    } else {
        pair[0].canonical_v
    };
    let output_right = if canonical_to_scan {
        pair[1].scan_y
    } else {
        pair[1].canonical_v
    };
    Some(output_left + ratio * (output_right - output_left))
}

/// Build an invertible piecewise-linear mesh from dense scan-space staff paths.
///
/// Within each mesh column, scan y and canonical v are related by the ordered
/// staff-line anchors. Between columns, scan x and canonical u interpolate.
/// This supplies both forward and inverse coordinates without pretending that
/// a severe book warp is one global homography.
#[must_use]
pub fn build_page_geometry_model(
    staves: Vec<TracedStaffGeometry>,
    systems: &[Vec<usize>],
) -> PageGeometryModel {
    let models = systems
        .iter()
        .enumerate()
        .filter_map(|(index, staff_ids)| {
            let traces = staff_ids
                .iter()
                .filter_map(|id| staves.iter().find(|trace| trace.staff_id == *id))
                .collect::<Vec<_>>();
            (traces.len() == staff_ids.len() && !traces.is_empty())
                .then(|| build_system(index + 1, staff_ids, &traces))
                .flatten()
        })
        .collect();
    PageGeometryModel {
        staves,
        systems: models,
    }
}

fn build_system(
    system_id: usize,
    staff_ids: &[usize],
    traces: &[&TracedStaffGeometry],
) -> Option<SystemGeometryModel> {
    let scan_left = traces.iter().map(|trace| trace.left).min()?;
    let scan_right = traces.iter().map(|trace| trace.right).max()?;
    let step = traces
        .iter()
        .flat_map(|trace| trace.columns.windows(2))
        .map(|pair| (pair[1].x - pair[0].x).round_ties_even() as i32)
        .filter(|step| *step > 0)
        .min()
        .unwrap_or(2);
    let base_interline = median(
        traces
            .iter()
            .flat_map(|trace| trace.columns.iter().map(|column| column.interline))
            .collect(),
    )?;
    let offsets = canonical_staff_offsets(traces)?;

    let mut scan_xs = Vec::new();
    let mut x = scan_left;
    while x <= scan_right {
        scan_xs.push(f64::from(x));
        let next = x.saturating_add(step);
        if next == x {
            break;
        }
        x = next;
    }
    if scan_xs.last().copied() != Some(f64::from(scan_right)) {
        scan_xs.push(f64::from(scan_right));
    }

    let mut columns = Vec::with_capacity(scan_xs.len());
    let mut canonical_u = 0.0;
    let mut previous_center: Option<(f64, f64)> = None;
    let mut scan_top = f64::INFINITY;
    let mut scan_bottom = f64::NEG_INFINITY;
    for scan_x in scan_xs {
        let mut anchors = Vec::with_capacity(traces.len() * 5);
        for ((trace, &staff_id), &offset) in traces.iter().zip(staff_ids).zip(&offsets) {
            let column = interpolate_column(trace, scan_x)?;
            for line_index in 0..5 {
                let scan_y = column.upper_y + (line_index as f64 * column.interline);
                scan_top = scan_top.min(scan_y);
                scan_bottom = scan_bottom.max(scan_y);
                anchors.push(WarpAnchor {
                    staff_id,
                    line_index,
                    scan_y,
                    canonical_v: offset + line_index as f64,
                });
            }
        }
        anchors.sort_by(|one, two| one.scan_y.total_cmp(&two.scan_y));
        let center = anchors.iter().map(|anchor| anchor.scan_y).sum::<f64>() / anchors.len() as f64;
        if let Some((previous_x, previous_y)) = previous_center {
            canonical_u += ((scan_x - previous_x).hypot(center - previous_y)) / base_interline;
        }
        previous_center = Some((scan_x, center));
        columns.push(WarpMeshColumn {
            scan_x,
            canonical_u,
            anchors,
        });
    }
    let canonical_height = offsets.last().copied()? + 4.0;
    Some(SystemGeometryModel {
        system_id,
        staff_ids: staff_ids.to_vec(),
        scan_left,
        scan_right,
        scan_top,
        scan_bottom,
        canonical_width: columns.last()?.canonical_u,
        canonical_height,
        canonical_interline_pixels: base_interline,
        columns,
    })
}

fn canonical_staff_offsets(traces: &[&TracedStaffGeometry]) -> Option<Vec<f64>> {
    let mut offsets = vec![0.0];
    for pair in traces.windows(2) {
        let left = pair[0].left.max(pair[1].left);
        let right = pair[0].right.min(pair[1].right);
        if left > right {
            return None;
        }
        let samples = pair[0]
            .columns
            .iter()
            .filter(|column| left as f64 <= column.x && column.x <= right as f64)
            .filter_map(|upper| {
                let lower = interpolate_column(pair[1], upper.x)?;
                let spacing = (upper.interline + lower.interline) / 2.0;
                (spacing > 0.0).then_some((lower.upper_y - upper.upper_y) / spacing)
            })
            .collect();
        let gap = median(samples)?.max(5.0);
        offsets.push(offsets.last().copied()? + gap);
    }
    Some(offsets)
}

fn interpolate_column(trace: &TracedStaffGeometry, x: f64) -> Option<StaffTraceColumn> {
    let columns = &trace.columns;
    let first = *columns.first()?;
    let last = *columns.last()?;
    let pair = if x <= first.x {
        [first, *columns.get(1).unwrap_or(&first)]
    } else if x >= last.x {
        [
            *columns
                .get(columns.len().saturating_sub(2))
                .unwrap_or(&last),
            last,
        ]
    } else {
        let right = columns.partition_point(|column| column.x < x);
        [columns[right - 1], columns[right]]
    };
    let dx = pair[1].x - pair[0].x;
    let ratio = if dx == 0.0 { 0.0 } else { (x - pair[0].x) / dx };
    Some(StaffTraceColumn {
        x,
        upper_y: pair[0].upper_y + ratio * (pair[1].upper_y - pair[0].upper_y),
        interline: pair[0].interline + ratio * (pair[1].interline - pair[0].interline),
        confidence: pair[0].confidence + ratio * (pair[1].confidence - pair[0].confidence),
        terminal_coverage: pair[0].terminal_coverage
            + ratio * (pair[1].terminal_coverage - pair[0].terminal_coverage),
    })
}

fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    Some(if values.len() % 2 == 0 {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trace(id: usize, top: f64) -> TracedStaffGeometry {
        let columns = (0..=10)
            .map(|index| StaffTraceColumn {
                x: (index * 10) as f64,
                upper_y: top + (index as f64 * 0.2),
                interline: 10.0,
                confidence: 0.8,
                terminal_coverage: 0.0,
            })
            .collect::<Vec<_>>();
        TracedStaffGeometry {
            staff_id: id,
            seed_left: 0,
            seed_right: 100,
            left: 0,
            right: 100,
            mean_confidence: 0.8,
            terminal_bar_x: None,
            lines: (0..5)
                .map(|line| {
                    columns
                        .iter()
                        .map(|column| (column.x, column.upper_y + (line as f64 * column.interline)))
                        .collect()
                })
                .collect(),
            columns,
        }
    }

    #[test]
    fn mesh_uses_staff_space_and_centerline_arc_length() {
        let model = build_page_geometry_model(vec![trace(1, 20.0), trace(2, 120.0)], &[vec![1, 2]]);
        let system = &model.systems[0];
        assert_eq!(system.canonical_height, 14.0);
        assert!(system.canonical_width > 9.9);
        assert_eq!(system.columns[0].anchors[0].canonical_v, 0.0);
        assert_eq!(system.columns[0].anchors[5].canonical_v, 10.0);
    }

    #[test]
    fn scan_and_canonical_coordinates_round_trip() {
        let model = build_page_geometry_model(vec![trace(1, 20.0), trace(2, 120.0)], &[vec![1, 2]]);
        let system = &model.systems[0];
        for (u, v) in [(0.0, 0.0), (2.75, 3.5), (7.2, 8.25), (10.0, 14.0)] {
            let scan = system.canonical_to_scan(u, v).unwrap();
            let canonical = system.scan_to_canonical(scan.0, scan.1).unwrap();
            assert!(
                (canonical.0 - u).abs() < 1e-9,
                "{u:?} -> {scan:?} -> {canonical:?}"
            );
            assert!(
                (canonical.1 - v).abs() < 1e-9,
                "{v:?} -> {scan:?} -> {canonical:?}"
            );
        }
    }
}
