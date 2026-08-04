#![no_main]

//! Differential fuzzing: anything this decoder accepts must decode exactly as
//! libjpeg does.
//!
//! Panic-freedom alone would not catch a decoder that quietly produces the
//! wrong image, which is the failure that actually matters for parity.
//!
//! The check runs in one direction on purpose. libjpeg is only invoked on
//! inputs our decoder already accepted, so the invariant is "we never silently
//! produce an image that differs from libjpeg's". Running it the other way
//! would mean catching libjpeg's fatal errors, which it reports through a
//! handler that must not return; the usual `setjmp`/`longjmp` trick for that is
//! not sound across Rust frames, and an earlier version of this target crashed
//! in the harness rather than in either decoder.
//!
//! Our decoder is markedly stricter than libjpeg -- it validates markers,
//! tables, dimensions, sampling factors, and magnitude categories, and refuses
//! whole processes -- so inputs we accept should be ones libjpeg accepts. If
//! that ever stops holding, libjpeg's default handler exits and libFuzzer
//! reports it, which is itself worth triaging: it would mean we accept
//! something libjpeg rejects.

use libfuzzer_sys::fuzz_target;
use std::mem;

/// Largest image the reference decoder is allowed to allocate, so a mutated
/// size field cannot turn into a multi-gigabyte allocation.
const MAX_REFERENCE_SAMPLES: usize = 8 << 20;

/// Decodes with libjpeg-turbo using libjpeg's own defaults.
///
/// `JDCT_ISLOW` and fancy upsampling are those defaults; they are set
/// explicitly because they are exactly what parity depends on.
#[allow(
    unsafe_code,
    reason = "the reference decoder is a C library; this is test-only FFI"
)]
fn libjpeg_decode(bytes: &[u8]) -> Option<(usize, usize, usize, Vec<u8>)> {
    use mozjpeg_sys::{
        J_DCT_METHOD, boolean, jpeg_common_struct, jpeg_create_decompress, jpeg_decompress_struct,
        jpeg_destroy_decompress, jpeg_error_mgr, jpeg_finish_decompress, jpeg_mem_src,
        jpeg_read_header, jpeg_read_scanlines, jpeg_start_decompress, jpeg_std_error,
    };

    /// Swallows libjpeg's corrupt-data warnings, which are not failures and
    /// would otherwise flood the fuzzer's output.
    unsafe extern "C-unwind" fn quiet(_: &mut jpeg_common_struct) {}

    // SAFETY: every pointer refers to a live local or to `bytes`, which outlives
    // the decompress object; the object is destroyed on both exit paths.
    unsafe {
        let mut error: jpeg_error_mgr = mem::zeroed();
        let mut cinfo: jpeg_decompress_struct = mem::zeroed();
        cinfo.common.err = jpeg_std_error(&mut error);
        error.output_message = Some(quiet);

        jpeg_create_decompress(&mut cinfo);
        jpeg_mem_src(&mut cinfo, bytes.as_ptr(), bytes.len() as _);
        jpeg_read_header(&mut cinfo, boolean::from(true));
        cinfo.dct_method = J_DCT_METHOD::JDCT_ISLOW;
        cinfo.do_fancy_upsampling = boolean::from(true);
        jpeg_start_decompress(&mut cinfo);

        let width = cinfo.output_width as usize;
        let height = cinfo.output_height as usize;
        let components = cinfo.output_components as usize;
        if width.saturating_mul(height).saturating_mul(components) > MAX_REFERENCE_SAMPLES {
            jpeg_destroy_decompress(&mut cinfo);
            return None;
        }

        let stride = width * components;
        let mut samples = vec![0u8; stride * height];
        while cinfo.output_scanline < cinfo.output_height {
            let offset = cinfo.output_scanline as usize * stride;
            let mut rows = [samples.as_mut_ptr().add(offset)];
            jpeg_read_scanlines(&mut cinfo, rows.as_mut_ptr(), 1);
        }
        jpeg_finish_decompress(&mut cinfo);
        jpeg_destroy_decompress(&mut cinfo);
        Some((width, height, components, samples))
    }
}

fuzz_target!(|data: &[u8]| {
    // Only images we produced are compared. Refusals are the other target's
    // business and are a documented, deliberate part of this decoder.
    let Ok(ours) = audiveris_jpeg::decode(data) else {
        return;
    };
    if ours
        .width
        .saturating_mul(ours.height)
        .saturating_mul(ours.components)
        > MAX_REFERENCE_SAMPLES
    {
        return;
    }
    let Some((width, height, components, reference)) = libjpeg_decode(data) else {
        return;
    };

    assert_eq!(
        (ours.width, ours.height, ours.components),
        (width, height, components),
        "geometry disagrees with libjpeg"
    );
    assert!(
        ours.samples == reference,
        "sample mismatch against libjpeg on a {width}x{height}x{components} image"
    );
});
