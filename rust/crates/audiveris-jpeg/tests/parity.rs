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
/// symbol exceeded the standard's magnitude range; one declared a frame header
/// shorter than its own fixed fields. They live here so the fixes are covered
/// without needing a nightly toolchain to run the fuzzer.
///
/// The directory also holds the inputs behind
/// `accepts_and_refuses_the_same_files_java_does`, which this sweep covers for
/// panic-freedom regardless of their verdict.
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

/// Regression for the narrow-plane upsampler fallback.
///
/// libjpeg chooses its fancy two-times upsamplers only when a component's
/// downsampled width exceeds two, and replicates instead below that. This
/// decoder filtered unconditionally, so for 4:2:0 any image at most four pixels
/// wide decoded differently. Differential fuzzing found it on this 3x5 file,
/// where four samples in the last two rows were off by one.
///
/// The file is also picked up by the corpus sweep above; it is named here so a
/// regression points straight at the cause.
#[test]
fn narrow_chroma_planes_replicate_as_libjpeg_does() {
    let path = repo_path("rust/crates/audiveris-jpeg/tests/data/narrow-chroma-3x5-420.jpg");
    let bytes = std::fs::read(&path).expect("narrow-chroma fixture");
    let (width, height, components, reference) = libjpeg_decode(&bytes);
    let decoded = audiveris_jpeg::decode(&bytes).expect("decode");
    assert_eq!(
        (decoded.width, decoded.height, decoded.components),
        (width, height, components)
    );
    assert_identical(
        "narrow-chroma-3x5-420.jpg",
        &decoded.samples,
        &reference,
        width,
        components,
    );
}

/// Files this decoder refuses on purpose, with the reason it refuses them.
///
/// Every other disagreement with `oracle/jpeg-verdicts.txt` is a bug. Keeping
/// the list here rather than in the oracle means the oracle stays a plain
/// recording of what Java does.
const DELIBERATE_REFUSALS: &[(&str, &str)] = &[(
    "unsupported-progressive.bin",
    "progressive JPEG is refused rather than approximated; see the crate docs",
)];

/// Accept or reject must match Java's, file for file.
///
/// This is the half of parity that `assert_identical` cannot reach. A decoder
/// that accepts a file Audiveris rejects produces an image where Audiveris
/// produces an error, and there are no samples to compare because one side has
/// none. Four such files came out of differential fuzzing, each of them a check
/// libjpeg makes and this decoder did not: a scan naming a component the frame
/// never declared, a trailing marker with an impossible length, a Huffman table
/// whose DC symbols exceed the magnitude range, and a second start-of-image.
///
/// Geometry is compared too, so a file that decodes to the wrong size fails here
/// rather than silently downstream.
#[test]
fn accepts_and_refuses_the_same_files_java_does() {
    let oracle = repo_path("rust/oracle/jpeg-verdicts.txt");
    let text = std::fs::read_to_string(&oracle)
        .unwrap_or_else(|error| panic!("{}: {error}", oracle.display()));
    let mut checked = 0usize;
    let mut failures = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let (relative, verdict) = line.split_once('\t').expect("path and verdict");
        let name = relative.rsplit('/').next().unwrap_or(relative);
        let bytes = std::fs::read(repo_path(relative))
            .unwrap_or_else(|error| panic!("{relative}: {error}"));
        let ours = audiveris_jpeg::decode(&bytes);
        checked += 1;

        if let Some((_, reason)) = DELIBERATE_REFUSALS.iter().find(|(file, _)| *file == name) {
            assert!(
                ours.is_err(),
                "{name} is listed as a deliberate refusal ({reason}) but decoded"
            );
            continue;
        }

        match (verdict.split_once(' '), &ours) {
            (Some(("accept", geometry)), Ok(decoded)) => {
                let expected: Vec<usize> = geometry
                    .split_whitespace()
                    .map(|field| field.parse().expect("geometry field"))
                    .collect();
                assert_eq!(
                    vec![decoded.width, decoded.height, decoded.components],
                    expected,
                    "{name}: geometry"
                );
            }
            (Some(("reject", _)), Err(_)) => {}
            (Some(("accept", _)), Err(error)) => {
                failures.push(format!("{name}: Java accepts it, we refused it: {error}"));
            }
            (Some(("reject", message)), Ok(_)) => {
                failures.push(format!(
                    "{name}: Java rejects it ({message}), we decoded it"
                ));
            }
            _ => panic!("{name}: unreadable verdict {verdict:?}"),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(
        checked >= 100,
        "expected the full fixture set, ran {checked}"
    );
}

/// Regression for the integer widths inside the inverse transform.
///
/// libjpeg dequantizes in `int` and then carries the butterflies in `JLONG`,
/// which is `long` -- 64 bits here -- narrowing back to `int` only for the
/// inter-pass workspace and the range-limit index. This decoder used 32 bits
/// throughout, which is indistinguishable until a coefficient is large enough
/// for a product to pass 2^31.
///
/// This 1x9 4:2:2 file does exactly that: its luma is ordinary and matched all
/// along, while its chroma blocks carry coefficients near 8191, and 496 samples
/// -- all of them chroma -- came out wrong. The coefficients themselves were
/// identical on both sides, which is what pointed past the entropy decoder to
/// the arithmetic.
///
/// The file is also picked up by the corpus sweep above; it is named here so a
/// regression points straight at the cause.
#[test]
fn wide_coefficient_products_do_not_wrap_at_thirty_two_bits() {
    let path = repo_path("rust/crates/audiveris-jpeg/tests/data/wide-coefficients-1x9-422.jpg");
    let bytes = std::fs::read(&path).expect("wide-coefficient fixture");
    let (width, height, components, reference) = libjpeg_decode(&bytes);
    let decoded = audiveris_jpeg::decode(&bytes).expect("decode");
    assert_eq!(
        (decoded.width, decoded.height, decoded.components),
        (width, height, components)
    );
    assert_identical(
        "wide-coefficients-1x9-422.jpg",
        &decoded.samples,
        &reference,
        width,
        components,
    );
}

/// Regression for libjpeg's recovery at a restart marker.
///
/// This file sets a restart interval of one and then runs out of scan data
/// early, so the decoder meets restart markers it did not expect. libjpeg
/// handles that with a numbered-marker resynchronisation policy, and -- the part
/// that moved 1511 of 12288 samples here -- a successful restart *clears* its
/// out-of-data flag. A truncated segment therefore stops rendering flat grey the
/// moment a restart marker arrives, rather than to the end of the image.
///
/// The file is also picked up by the corpus sweep above; it is named here so a
/// regression points straight at the cause.
#[test]
fn restart_markers_resynchronise_as_libjpeg_does() {
    let path = repo_path("rust/crates/audiveris-jpeg/tests/data/restart-resync-64x64-420.jpg");
    let bytes = std::fs::read(&path).expect("restart-resync fixture");
    let (width, height, components, reference) = libjpeg_decode(&bytes);
    let decoded = audiveris_jpeg::decode(&bytes).expect("decode");
    assert_eq!(
        (decoded.width, decoded.height, decoded.components),
        (width, height, components)
    );
    assert_identical(
        "restart-resync-64x64-420.jpg",
        &decoded.samples,
        &reference,
        width,
        components,
    );
}

/// A scan header this decoder must refuse because libjpeg refuses it.
///
/// Parity has a second edge besides sample values: the set of files accepted.
/// Accepting one libjpeg rejects means the port produces an image where
/// Audiveris produces an error, and no sample comparison would ever show it --
/// libjpeg is not there to compare against. Differential fuzzing surfaced it as
/// libjpeg's error handler exiting the process, which was the intended signal.
///
/// This file's scan selects component ids the frame never declared. libjpeg
/// calls that fatal (`JERR_BAD_COMPONENT_ID`); this decoder used to ignore the
/// unmatched selector, leave the component's table indices at zero, and decode
/// an image anyway.
#[test]
fn refuses_scan_headers_libjpeg_refuses() {
    let path = repo_path(
        "rust/crates/audiveris-jpeg/tests/data/regressions/scan-selects-unknown-component.bin",
    );
    let bytes = std::fs::read(&path).expect("scan-header fixture");
    assert!(
        matches!(
            audiveris_jpeg::decode(&bytes),
            Err(audiveris_jpeg::JpegError::UnknownScanComponent(_))
        ),
        "a scan naming a component the frame never declared must be refused"
    );
}

/// Regression for libjpeg's behaviour after mid-scan corruption.
///
/// This file carries 142 extraneous bytes before its end-of-image marker.
/// libjpeg reports the corruption and keeps decoding, and it used to be the one
/// case this decoder could not follow: the final MCU row disagreed, some blocks
/// completely.
///
/// Two differences caused it, and both are places where libjpeg is more
/// permissive than a reading of the standard suggests. An AC run can push the
/// coefficient index past the end of the block; libjpeg still consumes that
/// coefficient's magnitude bits, and it still stores the value, because its
/// zig-zag table carries sixteen padding entries that all point at coefficient
/// 63. Stopping the block early instead leaves the bit reader one field behind
/// libjpeg's, and every subsequent block in the scan decodes from the wrong
/// offset -- which is why one corrupt run produced 496 differing samples rather
/// than a handful.
///
/// The file is also picked up by the corpus sweep above; it is named here so a
/// regression points straight at the cause.
#[test]
fn corrupt_resynchronisation_matches_libjpeg() {
    let path = repo_path("rust/crates/audiveris-jpeg/tests/data/corrupt-resync-80x80-420.jpg");
    let bytes = std::fs::read(&path).expect("corrupt-resync fixture");
    let (width, height, components, reference) = libjpeg_decode(&bytes);
    let decoded = audiveris_jpeg::decode(&bytes).expect("decode");
    assert_eq!(
        (decoded.width, decoded.height, decoded.components),
        (width, height, components)
    );
    assert_identical(
        "corrupt-resync-80x80-420.jpg",
        &decoded.samples,
        &reference,
        width,
        components,
    );
}
