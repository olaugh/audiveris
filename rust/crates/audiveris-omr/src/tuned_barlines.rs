// SPDX-License-Identifier: AGPL-3.0-or-later

//! Runs the barline tuning enhancement over the final GRID output.
//!
//! Adapter between the promoted GRID SIG and the pure engine in
//! `audiveris_image::bar_tuning` (see `docs/barline-tuning-port.md`).  Raw
//! boundaries are never mutated: the pass produces a separate, reviewable
//! hypothesis layer next to the accepted output, exactly like its Python
//! oracle.
//!
//! Boundary construction mirrors the Python exporter
//! (`stage-omr-data/scripts/eval_schenker_measure_checks.py`): accepted
//! barline inters cluster by running-mean abscissa within
//! `max(3.0, 0.9 * interline)`; `support` is the number of distinct staves
//! in a cluster; the grade is the plain intrinsic grade (never the
//! contextual grade); synthetic system-edge entries bracket the interior.
//! That script's page-level `regularize_bar_rows` demotion pass is *not*
//! ported yet, so wired runs see slightly rawer boundaries than the Python
//! pipeline did — a documented difference, not an accident.

use audiveris_image::bar_tuning::{
    BarClassificationParameters, BarTuningParameters, BoundaryDiff, ClassifiedBoundary,
    GeometryCount, ProjectionCandidate, RawBoundary, RawBoundaryKind, SystemBand, SystemBarInput,
    TunedBoundary, classify_boundary, geometry_count_with_voltas, projection_candidates,
    resolve_count, select_boundaries, volta_hook,
};
use audiveris_image::bars_logic::{PeakWidthClass, VerticalInterKind};
use audiveris_image::grid_sig::GridSigNode;
use audiveris_image::ingest::GrayRaster;

use crate::grid_executor::HeadlessGridSigState;
use crate::recognize::SystemBounds;

/// The tuning outcome for one system.
#[derive(Debug, Clone)]
pub struct TunedSystemReport {
    pub system_id: usize,
    /// Why the system was skipped, when it was (`"not_piano_pair"`).
    pub skipped: Option<&'static str>,
    pub raw_count: usize,
    pub tuned_count: usize,
    pub geometry: GeometryCount,
    pub raw_boundaries: Vec<f64>,
    pub tuned_boundaries: Vec<TunedBoundary>,
    pub diff: BoundaryDiff,
    pub candidates: Vec<ProjectionCandidate>,
    /// Candidate positions carrying detected volta (ending) brackets.
    pub volta_hooks: Vec<f64>,
    /// Barline-form classification per tuned boundary, edges included.
    pub forms: Vec<ClassifiedBoundary>,
}

/// The sheet-level tuning product attached to the GRID recognition.
#[derive(Debug, Clone)]
pub struct TunedBarlinesReport {
    pub interline: f64,
    pub systems: Vec<TunedSystemReport>,
}

struct BarInter {
    x: f64,
    staff: usize,
    grade: f64,
    thick: bool,
}

/// Runs the enhancement over every two-staff (piano) system.
#[must_use]
pub fn tune_grid_barlines(
    gray: &GrayRaster,
    sig: &HeadlessGridSigState,
    system_bounds: &[SystemBounds],
    interline: f64,
) -> TunedBarlinesReport {
    let parameters = BarTuningParameters::from_scale(interline);
    let classification = BarClassificationParameters::from_scale(interline);
    let cluster_tolerance = (0.9 * interline).max(3.0);
    let mut systems = Vec::new();
    for system in &sig.systems {
        let Some(bounds) = system_bounds
            .iter()
            .find(|bounds| bounds.system_id == system.system_id)
        else {
            continue;
        };
        let band = SystemBand {
            left: f64::from(bounds.left),
            right: f64::from(bounds.right),
            top: f64::from(bounds.top),
            bottom: f64::from(bounds.bottom),
        };
        // The engine models a two-staff piano system (paired bands plus the
        // inter-staff bridge); other populations are explicitly skipped.
        if system.staff_ids.len() != 2 {
            systems.push(TunedSystemReport {
                system_id: system.system_id,
                skipped: Some("not_piano_pair"),
                raw_count: 0,
                tuned_count: 0,
                geometry: GeometryCount {
                    intervals: 0,
                    certain_bars: 0,
                },
                raw_boundaries: Vec::new(),
                tuned_boundaries: Vec::new(),
                diff: BoundaryDiff::default(),
                candidates: Vec::new(),
                volta_hooks: Vec::new(),
                forms: Vec::new(),
            });
            continue;
        }

        let mut bars: Vec<BarInter> = system
            .sig
            .nodes_in_order()
            .filter_map(|(_, node)| match node {
                GridSigNode::Vertical { plan, .. } => match plan.kind {
                    VerticalInterKind::Barline { width_class, .. } => Some(BarInter {
                        x: plan.median.x,
                        staff: plan.peak.staff_id().value(),
                        grade: node.intrinsic_grade(),
                        thick: matches!(width_class, PeakWidthClass::Thick),
                    }),
                    VerticalInterKind::Bracket(_) => None,
                },
                _ => None,
            })
            .collect();
        bars.sort_by(|a, b| a.x.partial_cmp(&b.x).expect("finite bar positions"));

        // Running-mean clustering, exactly as the Python exporter does it.
        let mut clusters: Vec<Vec<&BarInter>> = Vec::new();
        for bar in &bars {
            match clusters.last_mut() {
                Some(cluster)
                    if (bar.x
                        - cluster.iter().map(|item| item.x).sum::<f64>()
                            / cluster.len() as f64)
                        .abs()
                        <= cluster_tolerance =>
                {
                    cluster.push(bar);
                }
                _ => clusters.push(vec![bar]),
            }
        }
        let mut boundaries: Vec<RawBoundary> = clusters
            .iter()
            .map(|cluster| {
                let mut staves: Vec<usize> = cluster.iter().map(|item| item.staff).collect();
                staves.sort_unstable();
                staves.dedup();
                RawBoundary {
                    x: cluster.iter().map(|item| item.x).sum::<f64>() / cluster.len() as f64,
                    support: staves.len() as u32,
                    max_grade: cluster.iter().map(|item| item.grade).fold(0.0, f64::max),
                    kind: if cluster.iter().any(|item| item.thick) {
                        RawBoundaryKind::Thick
                    } else {
                        RawBoundaryKind::Thin
                    },
                }
            })
            .collect();
        let edge = |x: f64| RawBoundary {
            x,
            support: 2,
            max_grade: 1.0,
            kind: RawBoundaryKind::Edge,
        };
        if boundaries
            .first()
            .is_none_or(|first| first.x > band.left + 10.0)
        {
            boundaries.insert(0, edge(band.left));
        }
        if boundaries
            .last()
            .is_none_or(|last| last.x < band.right - 10.0)
        {
            boundaries.push(edge(band.right));
        }
        let raw_count = boundaries.len().saturating_sub(1).max(1);
        let input = SystemBarInput {
            band,
            boundaries: boundaries.clone(),
        };

        let candidates = projection_candidates(gray, &band, &parameters);
        let volta_hooks: Vec<f64> = candidates
            .iter()
            .map(|candidate| candidate.x)
            .filter(|x| volta_hook(gray, &band, *x, &classification))
            .collect();
        let geometry = geometry_count_with_voltas(&input, &candidates, &volta_hooks, &parameters);
        // No decoded (OCR) count target in pure-Rust runs: the raw interval
        // count is the external fallback, exactly as in the Python build().
        let count = resolve_count(geometry, raw_count);
        let tuned = select_boundaries(&input, &candidates, &[], count, &parameters);
        let raw_x: Vec<f64> = boundaries.iter().map(|boundary| boundary.x).collect();
        let tuned_x: Vec<f64> = tuned.iter().map(|boundary| boundary.x).collect();
        // Python diffs at 6 px on the 6 px-interline Schenker scans; one
        // interline is the scale-free form of that tolerance.
        let diff = audiveris_image::bar_tuning::diff_boundaries(
            &raw_x,
            &tuned_x,
            parameters.geometry_merge,
        );
        let forms = tuned
            .iter()
            .map(|boundary| classify_boundary(gray, &band, boundary.x, &classification))
            .collect();
        systems.push(TunedSystemReport {
            system_id: system.system_id,
            skipped: None,
            raw_count,
            tuned_count: tuned.len().saturating_sub(1),
            geometry,
            raw_boundaries: raw_x,
            tuned_boundaries: tuned,
            diff,
            candidates,
            volta_hooks,
            forms,
        });
    }
    TunedBarlinesReport { interline, systems }
}
