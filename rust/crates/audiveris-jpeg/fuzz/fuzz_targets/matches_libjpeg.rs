#![no_main]

//! Differential fuzzing: whenever libjpeg decodes an input, we must agree with
//! it sample for sample.
//!
//! Panic-freedom alone would not catch a decoder that quietly produces the
//! wrong image, which is the failure that actually matters for parity. libjpeg
//! is the oracle, so anything it accepts and we accept must match exactly, and
//! disagreeing is a crash the fuzzer will minimize for us.

use libfuzzer_sys::fuzz_target;
use std::mem;

/// Decodes with libjpeg-turbo, returning `None` if libjpeg rejects the input.
///
/// libjpeg aborts through `error_exit`, which by default exits the process. The
/// handler below longjmps back instead so a rejected input is an ordinary
/// `None` and never kills the fuzzer.
fn libjpeg_decode(bytes: &[u8]) -> Option<(usize, usize, usize, Vec<u8>)> {
    use mozjpeg_sys::*;

    // A libjpeg error handler that unwinds via setjmp/longjmp.
    struct Jmp {
        error: jpeg_error_mgr,
        buffer: [u8; 512],
    }

    unsafe extern "C" fn on_error(cinfo: &mut jpeg_common_struct) {
        // SAFETY: `err` was installed by us and points at a live `Jmp`.
        unsafe {
            let jmp = (cinfo.err as *mut Jmp).cast::<Jmp>();
            let buffer = std::ptr::addr_of_mut!((*jmp).buffer).cast();
            longjmp(buffer, 1);
        }
    }
    unsafe extern "C" fn on_message(_: &mut jpeg_common_struct) {}

    unsafe extern "C" {
        fn setjmp(env: *mut core::ffi::c_void) -> i32;
        fn longjmp(env: *mut core::ffi::c_void, value: i32) -> !;
    }

    // SAFETY: pointers refer to live locals or to `bytes`, which outlives the
    // decompress object; the object is destroyed on every exit path.
    unsafe {
        let mut jmp: Jmp = mem::zeroed();
        let mut cinfo: jpeg_decompress_struct = mem::zeroed();
        cinfo.common.err = jpeg_std_error(&mut jmp.error);
        (*cinfo.common.err).error_exit = Some(mem::transmute::<_, _>(on_error as *const ()));
        (*cinfo.common.err).output_message = Some(mem::transmute::<_, _>(on_message as *const ()));

        jpeg_create_decompress(&mut cinfo);
        let buffer = std::ptr::addr_of_mut!(jmp.buffer).cast();
        if setjmp(buffer) != 0 {
            jpeg_destroy_decompress(&mut cinfo);
            return None;
        }
        jpeg_mem_src(&mut cinfo, bytes.as_ptr(), bytes.len() as _);
        jpeg_read_header(&mut cinfo, boolean::from(true));
        cinfo.dct_method = J_DCT_METHOD::JDCT_ISLOW;
        cinfo.do_fancy_upsampling = boolean::from(true);
        jpeg_start_decompress(&mut cinfo);

        let width = cinfo.output_width as usize;
        let height = cinfo.output_height as usize;
        let components = cinfo.output_components as usize;
        // Keep the fuzzer away from multi-gigabyte allocations.
        if width.saturating_mul(height).saturating_mul(components) > 8 << 20 {
            jpeg_destroy_decompress(&mut cinfo);
            return None;
        }
        let stride = width * components;
        let mut out = vec![0u8; stride * height];
        while cinfo.output_scanline < cinfo.output_height {
            let offset = cinfo.output_scanline as usize * stride;
            let mut rows = [out.as_mut_ptr().add(offset)];
            jpeg_read_scanlines(&mut cinfo, rows.as_mut_ptr(), 1);
        }
        jpeg_finish_decompress(&mut cinfo);
        jpeg_destroy_decompress(&mut cinfo);
        Some((width, height, components, out))
    }
}

fuzz_target!(|data: &[u8]| {
    let ours = audiveris_jpeg::decode(data);
    let Some((width, height, components, reference)) = libjpeg_decode(data) else {
        // libjpeg rejected it; we are free to accept or reject, only not to
        // crash, which the other target covers.
        return;
    };
    let Ok(ours) = ours else {
        // We refuse some processes libjpeg supports, which is documented and
        // deliberate. Only disagreement on an image we did produce is a bug.
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
