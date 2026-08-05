// SPDX-License-Identifier: AGPL-3.0-or-later

//! The object layer against real IMSLP sources, graded by PDFBox.
//!
//! `oracle/pdf-pages.txt` is what PDFBox makes of the corpus: page geometry,
//! the image behind every draw, the transform Java2D receives, and hashes at
//! four depths -- the image's bytes as they sit in the file, those bytes with
//! their filter chain applied, the samples they decode to, and the rendered
//! page. This checks whatever the port can currently reach, and reports how
//! much of the file it is *not* reaching, so the number moves visibly as each
//! layer lands.
//!
//! # Getting the corpus
//!
//! The sources are 20 MB of scans and are not in the repository. They are
//! listed with download URLs in the `imslp-pseudo` repository's
//! `manifests/acquired_scans.json`. Point `AUDIVERIS_PDF_CORPUS` at a directory
//! holding them and this runs; without it, it reports that it skipped rather
//! than passing quietly, because a parity test that silently does nothing is
//! worse than no test.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use audiveris_pdf::content;
use audiveris_pdf::document::Document;
use audiveris_pdf::filter;
use audiveris_pdf::object::{Name, Object};
use audiveris_pdf::render;

/// One `image` record: what PDFBox found behind a draw.
#[derive(Debug, Clone)]
struct ImageRecord {
    width: u64,
    height: u64,
    bits: u64,
    filters: String,
    raw_length: u64,
    raw_hash: String,
    stream_length: u64,
    stream_hash: String,
    /// `PDImage.getImage()`'s raster: width, height, band count, and an
    /// FNV-1a-64 over every sample band-interleaved and row-major. `None` when
    /// PDFBox itself failed to produce one, which no corpus image does.
    raster: Option<(u64, u64, usize, String)>,
}

/// One `page` record.
#[derive(Debug, Clone)]
struct PageRecord {
    media: [f64; 4],
    crop: [f64; 4],
    rotation: i64,
    /// FNV-1a-64 over the rendered page's samples. The end of the chain.
    fnv: String,
}

/// Everything the oracle says about one page.
#[derive(Debug, Clone, Default)]
struct Expected {
    images: Vec<ImageRecord>,
    page: Option<PageRecord>,
    /// The six-term transform Java2D receives per draw, at 17 significant
    /// digits, read out of a `PageDrawer` subclass.
    draws: Vec<[f64; 6]>,
    /// The rendered page's size in pixels.
    render: Option<(u32, u32)>,
}

/// FNV-1a-64, the hash the probe writes.
fn fnv(data: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Reads `key`'s value from a whitespace-separated record.
fn field<'a>(parts: &'a [&'a str], key: &str) -> Option<&'a str> {
    parts
        .iter()
        .position(|part| *part == key)
        .and_then(|at| parts.get(at + 1).copied())
}

fn numbers(parts: &[&str], key: &str) -> Option<[f64; 4]> {
    let at = parts.iter().position(|part| *part == key)? + 1;
    let mut values = [0f64; 4];
    for (index, slot) in values.iter_mut().enumerate() {
        *slot = parts.get(at + index)?.parse().ok()?;
    }
    Some(values)
}

fn read_oracle(path: &Path) -> BTreeMap<(String, usize), Expected> {
    let text = std::fs::read_to_string(path).expect("the oracle is checked in");
    let mut pages: BTreeMap<(String, usize), Expected> = BTreeMap::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        let [file, index, kind, ..] = parts.as_slice() else {
            continue;
        };
        let key = (
            (*file).to_owned(),
            index.parse::<usize>().expect("a page index"),
        );
        let entry = pages.entry(key).or_default();
        match *kind {
            "image" => {
                let size = parts
                    .iter()
                    .position(|part| *part == "size")
                    .expect("a size");
                entry.images.push(ImageRecord {
                    width: parts[size + 1].parse().expect("a width"),
                    height: parts[size + 2].parse().expect("a height"),
                    bits: field(&parts, "bpc").expect("bpc").parse().expect("a depth"),
                    filters: field(&parts, "filters").expect("filters").to_owned(),
                    raw_length: field(&parts, "raw").expect("raw").parse().unwrap_or(0),
                    raw_hash: parts[parts.iter().position(|p| *p == "raw").expect("raw") + 2]
                        .to_owned(),
                    stream_length: field(&parts, "stream")
                        .expect("stream")
                        .parse()
                        .unwrap_or(0),
                    stream_hash: parts
                        [parts.iter().position(|p| *p == "stream").expect("stream") + 2]
                        .to_owned(),
                    raster: parts.iter().position(|p| *p == "raster").and_then(|at| {
                        Some((
                            parts.get(at + 1)?.parse().ok()?,
                            parts.get(at + 2)?.parse().ok()?,
                            parts.get(at + 3)?.parse().ok()?,
                            (*parts.get(at + 4)?).to_owned(),
                        ))
                    }),
                });
            }
            "draw" => {
                let mut terms = [0f64; 6];
                for (index, key) in ["m00", "m10", "m01", "m11", "m02", "m12"]
                    .iter()
                    .enumerate()
                {
                    terms[index] = field(&parts, key)
                        .expect("a transform term")
                        .parse()
                        .expect("a finite term");
                }
                entry.draws.push(terms);
            }
            "page" => {
                entry.render = parts.iter().position(|p| *p == "render").map(|at| {
                    (
                        parts[at + 1].parse().expect("a render width"),
                        parts[at + 2].parse().expect("a render height"),
                    )
                });
                entry.page = Some(PageRecord {
                    media: numbers(&parts, "media").expect("a media box"),
                    crop: numbers(&parts, "crop").expect("a crop box"),
                    rotation: field(&parts, "rot")
                        .expect("rot")
                        .parse()
                        .expect("a rotation"),
                    fnv: field(&parts, "fnv").expect("fnv").to_owned(),
                });
            }
            _ => {}
        }
    }
    pages
}

/// The image XObjects a page's resources name, in resource-name order.
fn page_images(document: &Document, page: &audiveris_pdf::document::Page) -> Vec<Object> {
    let Some(xobjects) = document
        .get(&page.resources, "XObject")
        .and_then(Object::dictionary)
    else {
        return Vec::new();
    };
    xobjects
        .values()
        .map(|value| document.resolve(value).clone())
        .filter(|object| {
            object
                .dictionary()
                .and_then(|dictionary| dictionary.get(&Name::new("Subtype")))
                == Some(&Object::Name(Name::new("Image")))
        })
        .collect()
}

fn corpus_directory() -> Option<PathBuf> {
    std::env::var_os("AUDIVERIS_PDF_CORPUS").map(PathBuf::from)
}

#[test]
fn reads_the_structure_pdfbox_reads_on_every_corpus_page() {
    let oracle = read_oracle(Path::new("../../oracle/pdf-pages.txt"));
    let Some(directory) = corpus_directory() else {
        eprintln!(
            "skipped: set AUDIVERIS_PDF_CORPUS to a directory of the {} sources named in \
             manifests/acquired_scans.json",
            oracle
                .keys()
                .map(|(file, _)| file.as_str())
                .collect::<std::collections::BTreeSet<_>>()
                .len()
        );
        return;
    };

    let files: std::collections::BTreeSet<String> =
        oracle.keys().map(|(file, _)| file.clone()).collect();
    let mut checked_pages = 0usize;
    let mut checked_images = 0usize;
    let mut checked_filters = 0usize;
    let mut checked_rasters = 0usize;
    let mut checked_draws = 0usize;
    let mut checked_renders = 0usize;
    let mut unimplemented: BTreeMap<String, usize> = BTreeMap::new();
    let mut missing = Vec::new();

    for file in &files {
        let path = directory.join(file);
        if !path.exists() {
            missing.push(file.clone());
            continue;
        }
        let data = std::fs::read(&path).expect("the source is readable");
        let document = Document::parse(&data).unwrap_or_else(|error| panic!("{file}: {error}"));

        let expected_pages = oracle.keys().filter(|(name, _)| name == file).count();
        assert_eq!(
            document.page_count(),
            expected_pages,
            "{file}: page count disagrees with PDFBox"
        );

        for index in 0..document.page_count() {
            let expected = &oracle[&(file.clone(), index)];
            let page = document.page(index).expect("a page");
            let Some(record) = &expected.page else {
                continue;
            };
            // The oracle prints `PDRectangle`'s floats widened to double, so
            // the comparison is exact after narrowing rather than approximate.
            let media = [
                page.media_box.lower_left_x,
                page.media_box.lower_left_y,
                page.media_box.upper_right_x,
                page.media_box.upper_right_y,
            ];
            let crop = [
                page.crop_box.lower_left_x,
                page.crop_box.lower_left_y,
                page.crop_box.upper_right_x,
                page.crop_box.upper_right_y,
            ];
            for (index, (mine, theirs)) in media.iter().zip(record.media).enumerate() {
                assert_eq!(*mine, theirs as f32, "{file} page {index}: media box");
            }
            for (index, (mine, theirs)) in crop.iter().zip(record.crop).enumerate() {
                assert_eq!(*mine, theirs as f32, "{file} page {index}: crop box");
            }
            assert_eq!(
                page.rotation, record.rotation,
                "{file} page {index}: rotation"
            );
            checked_pages += 1;

            // Every corpus page draws exactly one image, so the resource set
            // and the draw records line up without interpreting the content
            // stream. When that stops being true this assertion says so.
            let images = page_images(&document, &page);
            if images.len() != expected.images.len() {
                continue;
            }
            for (object, record) in images.iter().zip(&expected.images) {
                let stream = object.stream().expect("an image is a stream");
                let dictionary = document.flatten(&stream.dictionary);
                assert_eq!(
                    dictionary.get(&Name::new("Width")).and_then(Object::as_i64),
                    Some(record.width as i64),
                    "{file} page {index}: image width"
                );
                assert_eq!(
                    dictionary
                        .get(&Name::new("Height"))
                        .and_then(Object::as_i64),
                    Some(record.height as i64),
                    "{file} page {index}: image height"
                );
                assert_eq!(
                    dictionary
                        .get(&Name::new("BitsPerComponent"))
                        .and_then(Object::as_i64),
                    Some(record.bits as i64),
                    "{file} page {index}: bits per component"
                );
                let names: Vec<String> = filter::names(&dictionary)
                    .iter()
                    .map(|name| String::from_utf8_lossy(name.as_bytes()).into_owned())
                    .collect();
                assert_eq!(
                    names.join(","),
                    record.filters,
                    "{file} page {index}: filter chain"
                );

                // The raw bytes are the object layer's own result: they pin the
                // `/Length` handling, including the three sources that declare
                // zero and force the `endstream` scan.
                assert_eq!(
                    stream.data.len() as u64,
                    record.raw_length,
                    "{file} page {index}: raw stream length"
                );
                assert_eq!(
                    fnv(&stream.data),
                    record.raw_hash,
                    "{file} page {index}: raw stream bytes"
                );
                checked_images += 1;

                match document.stream_data(stream) {
                    Ok(decoded) => {
                        assert_eq!(
                            decoded.len() as u64,
                            record.stream_length,
                            "{file} page {index}: decoded length"
                        );
                        assert_eq!(
                            fnv(&decoded),
                            record.stream_hash,
                            "{file} page {index}: decoded bytes"
                        );
                        checked_filters += 1;
                    }
                    Err(audiveris_pdf::Error::UnsupportedFilter { name }) => {
                        *unimplemented.entry(name).or_default() += 1;
                    }
                    Err(error) => panic!("{file} page {index}: {error}"),
                }

                // The rung above the filter chain: the same bytes turned into
                // samples. Graded separately so that a wrong composed page
                // later cannot be blamed on sample conversion without evidence.
                let Some((width, height, bands, hash)) = &record.raster else {
                    continue;
                };
                match audiveris_pdf::raster::decode(&document, stream) {
                    Ok(raster) => {
                        assert_eq!(
                            (raster.width as u64, raster.height as u64, raster.bands),
                            (*width, *height, *bands),
                            "{file} page {index}: raster geometry"
                        );
                        assert_eq!(
                            raster.samples.len(),
                            raster.width * raster.height * raster.bands,
                            "{file} page {index}: raster sample count"
                        );
                        assert_eq!(
                            fnv(&raster.samples),
                            *hash,
                            "{file} page {index}: raster samples"
                        );
                        checked_rasters += 1;
                    }
                    Err(audiveris_pdf::Error::UnsupportedImage { reason }) => {
                        *unimplemented.entry(format!("image: {reason}")).or_default() += 1;
                    }
                    Err(error) => panic!("{file} page {index}: raster: {error}"),
                }
            }

            // The transform each `Do` draws under: the device transform
            // `PDFRenderer` and `PageDrawer` install, composed with the CTM the
            // content stream built. Compared exactly, not approximately -- the
            // oracle stores 17 significant digits precisely so that the last
            // bits, which are where composing this wrongly goes wrong, are
            // visible.
            match content::device(&page, 300f32 / 72f32) {
                Ok(device) => {
                    if let Some((width, height)) = expected.render {
                        assert_eq!(
                            (device.width, device.height),
                            (width, height),
                            "{file} page {index}: rendered page size"
                        );
                    }
                    let draws = content::image_draws(&document, &page)
                        .unwrap_or_else(|error| panic!("{file} page {index}: {error}"));
                    assert_eq!(
                        draws.len(),
                        expected.draws.len(),
                        "{file} page {index}: draw count"
                    );
                    for (ordinal, (draw, record)) in draws.iter().zip(&expected.draws).enumerate() {
                        let image = &expected.images[ordinal];
                        let composed = content::draw_transform(
                            &device.transform,
                            draw.ctm,
                            image.width as usize,
                            image.height as usize,
                        );
                        for (term, (mine, theirs)) in
                            composed.matrix().iter().zip(record).enumerate()
                        {
                            assert_eq!(
                                mine, theirs,
                                "{file} page {index} draw {ordinal}: term {term}"
                            );
                            // `-0.0 == 0.0`, and the sign of zero is exactly
                            // what the state machine decides, so it is checked
                            // separately or it is not checked at all.
                            assert_eq!(
                                mine.is_sign_negative(),
                                theirs.is_sign_negative(),
                                "{file} page {index} draw {ordinal}: term {term} sign of zero"
                            );
                        }
                        checked_draws += 1;
                    }
                }
                Err(audiveris_pdf::Error::UnsupportedImage { reason }) => {
                    *unimplemented.entry(format!("page: {reason}")).or_default() += 1;
                }
                Err(error) => panic!("{file} page {index}: device: {error}"),
            }

            // The end of the chain: the rendered page Audiveris binarizes.
            match audiveris_pdf::render::page(&document, &page, render::AUDIVERIS_DPI) {
                Ok(rendered) => {
                    assert_eq!(
                        fnv(&rendered.image.samples),
                        record.fnv,
                        "{file} page {index}: rendered page"
                    );
                    checked_renders += 1;
                }
                Err(audiveris_pdf::Error::UnsupportedImage { reason }) => {
                    *unimplemented
                        .entry(format!("render: {reason}"))
                        .or_default() += 1;
                }
                Err(error) => panic!("{file} page {index}: render: {error}"),
            }
        }
    }

    assert!(
        missing.len() < files.len(),
        "AUDIVERIS_PDF_CORPUS holds none of the corpus: {missing:?}"
    );
    // Reported rather than asserted: this is the ledger of what is left, and it
    // should shrink to nothing as the image filters land.
    eprintln!(
        "checked {checked_pages} pages, {checked_images} images, {checked_filters} filter chains, \
         {checked_rasters} rasters, {checked_draws} draws, {checked_renders} renders; \
         still unimplemented: {unimplemented:?}"
    );
    assert!(checked_pages > 0, "no page was checked");
}
