# Fuzzing the JPEG decoder

Requires a nightly toolchain and `cargo-fuzz`:

```sh
rustup toolchain install nightly --component llvm-tools-preview
cargo install cargo-fuzz --locked
```

Two targets, for the two ways this decoder can be wrong.

## `decode_never_panics`

Any byte string must decode or return an error, never unwind. The decoder reads
untrusted scans, so a malformed file must not be able to take the process down.

```sh
cargo +nightly fuzz run decode_never_panics -- -max_total_time=900
```

## `matches_libjpeg`

The one that matters for parity. Panic-freedom would not catch a decoder that
quietly produces the wrong image, so libjpeg runs alongside as the oracle and
any input both accept must decode identically. libjpeg's `error_exit` is
redirected through `longjmp`, so an input libjpeg rejects is an ordinary skip
rather than a process exit.

```sh
cargo +nightly fuzz run matches_libjpeg -- -max_total_time=900
```

## Findings so far

Three panics, all from malformed input, all now fixed and kept as ordinary
tests in `tests/data/regressions` so they are covered on stable:

- two overflowed the inverse transform on coefficient/quantizer products that a
  corrupt file can present. Fixed by making the transform's arithmetic wrap,
  which is also what libjpeg's C does, so a damaged image still decodes the same
  way it does there. Saturating or checked arithmetic would have diverged.
- one carried a Huffman table whose DC symbol exceeded the standard's magnitude
  range, overflowing a shift. Now rejected as `InvalidMagnitudeCategory`.

No sample divergence from libjpeg has been found.

Crashes land in `artifacts/`. Copy new ones into `tests/data/regressions` so the
fix stays covered without a nightly toolchain.
