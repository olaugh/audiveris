// SPDX-License-Identifier: AGPL-3.0-or-later

//! Surveys barline-form classification over the real-scan parity corpora.
//!
//! Reads the tuned boundary positions from the Python-oracle fixtures,
//! classifies each with `classify_boundary` and `volta_hook`, and prints a
//! per-system listing plus a distribution summary.  A reconnaissance tool,
//! not a test: its output is for human review of the types layer on real
//! scans.
//!
//! Usage:
//!   AUDIVERIS_BARLINE_TUNING_FIXTURES=<workspace root> \
//!     cargo run -p audiveris-image --example classify_barlines

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use audiveris_image::bar_tuning::{
    BarClassificationParameters, BarlineForm, SystemBand, classify_boundary, volta_hook,
};
use audiveris_image::ingest::GrayRaster;

fn label(form: BarlineForm) -> &'static str {
    match form {
        BarlineForm::Unknown => "unknown",
        BarlineForm::Single => "single",
        BarlineForm::Double => "double",
        BarlineForm::Final => "final",
        BarlineForm::RepeatStart => "rptstart",
        BarlineForm::RepeatEnd => "rptend",
        BarlineForm::RepeatBoth => "rptboth",
    }
}

fn main() {
    let workspace = std::env::var("AUDIVERIS_BARLINE_TUNING_FIXTURES")
        .expect("set AUDIVERIS_BARLINE_TUNING_FIXTURES to the workspace root");
    let workspace = PathBuf::from(workspace);
    let oracle = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/py-barline-tuning");
    let mut distribution: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut voltas = 0usize;
    for (fixture, interline) in [
        ("schenker-sonata01.txt", 6.0),
        ("graceful-ghost-rag.txt", 12.0),
    ] {
        println!("== {fixture} (interline {interline}) ==");
        let text = std::fs::read_to_string(oracle.join(fixture)).expect("fixture readable");
        let parameters = BarClassificationParameters::from_scale(interline);
        let mut raster: Option<GrayRaster> = None;
        let mut band = SystemBand {
            left: 0.0,
            right: 0.0,
            top: 0.0,
            bottom: 0.0,
        };
        let mut system_id = String::new();
        for line in text.lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            match fields.first().copied() {
                Some("image") => {
                    let path = workspace.join(fields[1]);
                    let dynamic =
                        image::open(&path).unwrap_or_else(|error| panic!("open {path:?}: {error}"));
                    raster = Some(GrayRaster::from_dynamic(&dynamic));
                }
                Some("system") => {
                    system_id = fields[1].to_string();
                    band = SystemBand {
                        left: fields[2].parse().unwrap(),
                        right: fields[3].parse().unwrap(),
                        top: fields[4].parse().unwrap(),
                        bottom: fields[5].parse().unwrap(),
                    };
                }
                Some("expect_tuned") => {
                    let gray = raster.as_ref().unwrap();
                    let mut row = Vec::new();
                    for field in &fields[1..] {
                        let x: f64 = field.parse().unwrap();
                        let classified = classify_boundary(gray, &band, x, &parameters);
                        let volta = volta_hook(gray, &band, x, &parameters);
                        *distribution.entry(label(classified.form)).or_default() += 1;
                        if volta {
                            voltas += 1;
                        }
                        row.push(format!(
                            "{x:.0}:{}{}",
                            label(classified.form),
                            if volta { "+volta" } else { "" }
                        ));
                    }
                    println!("  {system_id}: {}", row.join(" "));
                }
                _ => {}
            }
        }
    }
    println!("\n== distribution over all tuned boundaries ==");
    for (form, count) in &distribution {
        println!("  {form:9} {count}");
    }
    println!("  volta-evidenced boundaries: {voltas}");
}
