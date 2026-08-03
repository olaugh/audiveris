// SPDX-License-Identifier: AGPL-3.0-or-later

//! Raster and run-length primitives for the headless recognition pipeline.

pub mod adaptive;
pub mod chamfer;
pub mod filament;
pub mod filament_comb;
pub mod filament_factory;
pub mod global_filter;
pub mod ingest;
pub mod line_cluster;
pub mod line_sections;
pub mod mask;
pub mod median;
pub mod run_table;
pub mod scale_estimate;
pub mod scale_runs;
pub mod section;
pub mod section_tally;
pub mod staff_pattern;
pub mod target_line;
pub mod watershed;
