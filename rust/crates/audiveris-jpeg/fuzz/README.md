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

Three sample divergences found by `matches_libjpeg`. Two are fixed:

- **Narrow planes.** libjpeg picks its fancy two-times upsamplers only when a
  component's downsampled width exceeds two, and replicates instead below that.
  This decoder filtered unconditionally, so for 4:2:0 any image at most four
  pixels wide decoded differently. Reproducing libjpeg means reproducing which
  method it selects, not only the arithmetic of each method -- reading the
  transform code would never have shown this, because the transform was right.
- **Truncated scans.** Once the data runs out libjpeg stops decoding and leaves
  the remaining blocks zeroed, a flat mid-grey. Padding with zero bits and
  carrying on produces plausible-looking noise instead. Scans are full of
  truncated files, so this one matters.

One is open and bounded by a test rather than blessed:

- **Resynchronisation after mid-scan corruption.** Where libjpeg resumes after
  extraneous bytes is not reproduced, and blocks in the affected MCU row
  disagree. This is the long tail flagged when the decoder was proposed:
  matching libjpeg on clean input is mechanical, matching its recovery is not.
  The fixture is `../tests/data/corrupt-resync-80x80-420.bin`; the fuzzer will
  keep rediscovering this class, so minimize new crashes against it before
  assuming they are separate.

Crashes land in `artifacts/`. Copy new ones into `tests/data/regressions` so the
fix stays covered without a nightly toolchain.
