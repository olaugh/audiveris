// SPDX-License-Identifier: AGPL-3.0-or-later

//! The seam: what the OMR load path hands binarization for a PDF sheet.
//!
//! `crates/audiveris-pdf` proves its renders equal PDFBox's. This proves the
//! *ingest* does not then change them. Those are different claims, and the gap
//! between them is exactly where a stray conversion would hide -- Audiveris
//! puts every loaded image through `Picture.adjustImageFormat`'s
//! maximum-channel rule, which is the identity on a gray raster only because
//! `max(v, v, v)` is `v`.
//!
//! So the assertion is deliberately end-to-end and deliberately redundant with
//! the PDF crate's own: for every page of every corpus source,
//! `Loader::image(id).fnv1a64()` must equal the `fnv` the Java probe recorded
//! for the rendered page. Same hash function, same sample order, opposite ends
//! of the port.
//!
//! Needs `AUDIVERIS_PDF_CORPUS`, like the PDF crate's corpus test; without it
//! this reports that it skipped rather than passing quietly.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use audiveris_image::ingest::Loader;

/// `(file, page index)` to the rendered page's FNV-1a-64 and size.
fn oracle(path: &Path) -> BTreeMap<(String, usize), (u32, u32, u64)> {
    let text = std::fs::read_to_string(path).expect("the oracle is checked in");
    let mut pages = BTreeMap::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        let [file, index, "page", ..] = parts.as_slice() else {
            continue;
        };
        let field = |key: &str| {
            parts
                .iter()
                .position(|part| *part == key)
                .and_then(|at| parts.get(at + 1).copied())
        };
        let render = parts
            .iter()
            .position(|part| *part == "render")
            .expect("a render size");
        pages.insert(
            (
                (*file).to_owned(),
                index.parse::<usize>().expect("a page index"),
            ),
            (
                parts[render + 1].parse().expect("a width"),
                parts[render + 2].parse().expect("a height"),
                u64::from_str_radix(field("fnv").expect("fnv"), 16).expect("a hash"),
            ),
        );
    }
    pages
}

#[test]
fn every_corpus_sheet_reaches_binarization_as_pdfbox_rendered_it() {
    let expected =
        oracle(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/pdf-pages.txt"));
    let Some(directory) = std::env::var_os("AUDIVERIS_PDF_CORPUS").map(PathBuf::from) else {
        eprintln!("skipped: set AUDIVERIS_PDF_CORPUS to the corpus directory");
        return;
    };

    let files: std::collections::BTreeSet<&String> =
        expected.keys().map(|(file, _)| file).collect();
    let mut checked = 0usize;
    for file in files {
        let path = directory.join(file);
        if !path.exists() {
            continue;
        }
        let loader = Loader::open(&path).unwrap_or_else(|error| panic!("{file}: {error}"));
        let pages = expected.keys().filter(|(name, _)| name == file).count();
        assert_eq!(loader.image_count(), pages, "{file}: sheet count");

        for index in 0..pages {
            let (width, height, hash) = expected[&(file.clone(), index)];
            // Sheet ids are one-based, as `Loader.getImage(int id)` is.
            let raster = loader
                .image(index + 1)
                .unwrap_or_else(|error| panic!("{file} sheet {}: {error}", index + 1));
            assert_eq!(
                (raster.width() as u32, raster.height() as u32),
                (width, height),
                "{file} sheet {}: geometry",
                index + 1
            );
            assert_eq!(
                raster.fnv1a64(),
                hash,
                "{file} sheet {}: samples differ from PDFBox's rendered page",
                index + 1
            );
            checked += 1;
        }
    }
    eprintln!("checked {checked} sheets through the OMR load path");
    assert!(checked > 0, "the corpus directory held none of the sources");
}

#[test]
fn a_sheet_id_outside_the_input_is_refused_rather_than_clamped() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/examples/chula.png");
    let loader = Loader::open(&path).expect("the example page loads");
    assert_eq!(loader.image_count(), 1, "a raster holds one sheet");
    assert!(loader.image(1).is_ok());
    for id in [0usize, 2, 99] {
        let error = loader.image(id).expect_err("outside the range");
        assert!(
            matches!(
                error,
                audiveris_image::ingest::LoadError::NoSuchImage { .. }
            ),
            "id {id} gave {error}"
        );
    }
}
