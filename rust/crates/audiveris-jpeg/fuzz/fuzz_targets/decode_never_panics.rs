#![no_main]

//! Any byte string must decode or return an error, never panic.
//!
//! The decoder reads untrusted scans, so a malformed file must not be able to
//! take the process down through an index, a slice range, or an overflow.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = audiveris_jpeg::decode(data);
});
