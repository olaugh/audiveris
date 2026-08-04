// SPDX-License-Identifier: AGPL-3.0-or-later

//! Differential test: every sample must equal libjpeg's.
//!
//! This is the whole justification for the crate. The decoder claims to
//! reproduce libjpeg exactly, and the only way to hold that claim honest is to
//! run libjpeg alongside it and require equality, so libjpeg-turbo is a
//! dev-dependency here and nowhere else.

use std::{mem, path::Path};

/// Decodes with libjpeg-turbo using libjpeg's own defaults.
///
/// `JDCT_ISLOW` and fancy upsampling are libjpeg's defaults; they are set
/// explicitly because they are exactly what parity depends on.
#[allow(
    unsafe_code,
    reason = "the reference decoder is a C library; this is test-only FFI"
)]
fn libjpeg_decode(bytes: &[u8]) -> (usize, usize, usize, Vec<u8>) {
    use mozjpeg_sys::{
        J_DCT_METHOD, boolean, jpeg_create_decompress, jpeg_decompress_struct,
        jpeg_destroy_decompress, jpeg_error_mgr, jpeg_finish_decompress, jpeg_mem_src,
        jpeg_read_header, jpeg_read_scanlines, jpeg_start_decompress, jpeg_std_error,
    };
    // SAFETY: every pointer refers to a live local or to `bytes`, which outlives
    // the decompress object; the struct is zeroed before initialization and
    // destroyed before return.
    unsafe {
        let mut err: jpeg_error_mgr = mem::zeroed();
        let mut cinfo: jpeg_decompress_struct = mem::zeroed();
        cinfo.common.err = jpeg_std_error(&mut err);
        jpeg_create_decompress(&mut cinfo);
        jpeg_mem_src(&mut cinfo, bytes.as_ptr(), bytes.len() as _);
        jpeg_read_header(&mut cinfo, boolean::from(true));
        cinfo.dct_method = J_DCT_METHOD::JDCT_ISLOW;
        cinfo.do_fancy_upsampling = boolean::from(true);
        jpeg_start_decompress(&mut cinfo);
        let width = cinfo.output_width as usize;
        let height = cinfo.output_height as usize;
        let components = cinfo.output_components as usize;
        let stride = width * components;
        let mut buffer = vec![0u8; stride * height];
        while cinfo.output_scanline < cinfo.output_height {
            let offset = cinfo.output_scanline as usize * stride;
            let mut rows = [buffer.as_mut_ptr().add(offset)];
            jpeg_read_scanlines(&mut cinfo, rows.as_mut_ptr(), 1);
        }
        jpeg_finish_decompress(&mut cinfo);
        jpeg_destroy_decompress(&mut cinfo);
        (width, height, components, buffer)
    }
}

fn repo_path(relative: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}

/// Reports the first disagreement with enough context to find it.
fn assert_identical(name: &str, ours: &[u8], reference: &[u8], width: usize, components: usize) {
    assert_eq!(ours.len(), reference.len(), "{name}: sample count");
    let mismatches = ours.iter().zip(reference).filter(|(a, b)| a != b).count();
    if mismatches == 0 {
        return;
    }
    let (index, (ours, theirs)) = ours
        .iter()
        .zip(reference)
        .enumerate()
        .find(|(_, (a, b))| a != b)
        .expect("a mismatch was counted");
    let pixel = index / components;
    panic!(
        "{name}: {mismatches} of {} samples differ from libjpeg; first at pixel \
         ({}, {}) component {} -- ours {ours}, libjpeg {theirs}",
        reference.len(),
        pixel % width,
        pixel / width,
        index % components,
    );
}

#[test]
fn decodes_the_example_corpus_jpeg_exactly_as_libjpeg_does() {
    let path = repo_path("data/examples/BachInvention5.jpg");
    let bytes = std::fs::read(&path).expect("example JPEG");
    let (width, height, components, reference) = libjpeg_decode(&bytes);

    let decoded = audiveris_jpeg::decode(&bytes).expect("decode");
    assert_eq!(
        (decoded.width, decoded.height, decoded.components),
        (width, height, components),
        "geometry"
    );
    assert_identical(
        "BachInvention5.jpg",
        &decoded.samples,
        &reference,
        width,
        components,
    );
}

/// Synthetic JPEGs covering the sampling factors the decoder claims.
///
/// Odd dimensions are deliberate: the edge cases of the triangle filter and the
/// partial trailing MCU are where an upsampler is most likely to disagree.
#[test]
fn decodes_generated_jpegs_exactly_across_sampling_factors() {
    let directory = repo_path("rust/crates/audiveris-jpeg/tests/data");
    let mut checked = 0usize;
    let Ok(entries) = std::fs::read_dir(&directory) else {
        panic!("missing generated fixtures at {}", directory.display());
    };
    let mut names: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "jpg"))
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no generated fixtures found");

    for path in names {
        let bytes = std::fs::read(&path).expect("fixture");
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let (width, _, components, reference) = libjpeg_decode(&bytes);
        let decoded = audiveris_jpeg::decode(&bytes)
            .unwrap_or_else(|error| panic!("{name}: decode failed: {error}"));
        assert_identical(&name, &decoded.samples, &reference, width, components);
        checked += 1;
    }
    assert!(checked >= 8, "expected the full fixture set, ran {checked}");
}

#[test]
fn rejects_processes_it_cannot_reproduce() {
    let path = repo_path("rust/crates/audiveris-jpeg/tests/data/unsupported-progressive.bin");
    let Ok(bytes) = std::fs::read(&path) else {
        return;
    };
    assert!(
        matches!(
            audiveris_jpeg::decode(&bytes),
            Err(audiveris_jpeg::JpegError::UnsupportedProcess(_))
        ),
        "a progressive JPEG must be refused, not approximated"
    );
}

/// Inputs that once panicked, kept as ordinary tests.
///
/// Each came out of `fuzz/fuzz_targets/decode_never_panics`, minimized by
/// libFuzzer. Two overflowed the inverse transform on coefficient and quantizer
/// products a malformed file can present; one carried a Huffman table whose DC
/// symbol exceeded the standard's magnitude range. They live here so the fixes
/// are covered without needing a nightly toolchain to run the fuzzer.
#[test]
fn fuzz_regressions_decode_or_error_without_panicking() {
    let directory = repo_path("rust/crates/audiveris-jpeg/tests/data/regressions");
    let entries = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()));
    let mut checked = 0usize;
    for entry in entries.filter_map(Result::ok) {
        let bytes = std::fs::read(entry.path()).expect("regression input");
        // The contract is only that this returns rather than unwinds.
        let _ = audiveris_jpeg::decode(&bytes);
        checked += 1;
    }
    assert!(
        checked >= 3,
        "expected the saved regressions, found {checked}"
    );
}

/// Pins the one known divergence from libjpeg so its scope cannot grow quietly.
///
/// Found by `fuzz/fuzz_targets/matches_libjpeg`. The chroma plane here is three
/// rows -- odd -- and the last two output rows disagree with libjpeg by one in
/// a few samples. The decode itself is not implicated: libjpeg's raw
/// downsampled planes, read back through `raw_data_out`, are identical to this
/// decoder's, so the fault is in the vertical context of the fancy 2x2
/// upsampler at the bottom edge.
///
/// The assertions below are deliberately not "this must match". They bound what
/// is wrong: only the last two rows, only by one. If a change makes it exact the
/// test fails and says so, and if a change makes it worse the test fails too.
#[test]
fn known_chroma_upsampling_divergence_stays_bounded() {
    let path = repo_path(
        "rust/crates/audiveris-jpeg/tests/data/known-divergence/chroma-odd-height-3x5-420.jpg",
    );
    let bytes = std::fs::read(&path).expect("known-divergence fixture");
    let (width, height, components, reference) = libjpeg_decode(&bytes);
    let decoded = audiveris_jpeg::decode(&bytes).expect("decode");

    let mut differing = 0usize;
    for (index, (ours, theirs)) in decoded.samples.iter().zip(&reference).enumerate() {
        if ours == theirs {
            continue;
        }
        differing += 1;
        let row = index / components / width;
        assert!(
            ours.abs_diff(*theirs) <= 1,
            "divergence grew past one count at row {row}"
        );
        assert!(
            row + 2 >= height,
            "divergence reached row {row}, outside the last two rows"
        );
    }
    assert_eq!(
        differing, 4,
        "the known divergence changed size; if it is now zero the upsampler's \
         bottom-edge context has been fixed and this test should be replaced by \
         an exact assertion"
    );
}
