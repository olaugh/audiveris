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
any input we accept must decode identically.

The comparison is deliberately one-directional: libjpeg is only invoked on
inputs our decoder already accepted. Running it the other way means catching
libjpeg's fatal errors, which it reports through a handler that must not
return, and the usual `setjmp`/`longjmp` for that is not sound across Rust
frames -- an earlier version of this target crashed in the harness rather than
in either decoder. Our decoder is the stricter of the two, so an input we
accept and libjpeg rejects would abort the run, and that is itself a finding
worth triaging.

```sh
cargo +nightly fuzz run matches_libjpeg -- -max_total_time=900
```

## Findings so far

Four panics, all from malformed input, all now fixed and kept as ordinary
tests in `tests/data/regressions` so they are covered on stable:

- two overflowed the inverse transform on coefficient/quantizer products that a
  corrupt file can present. Fixed by making the transform's arithmetic wrap,
  which is also what libjpeg's C does, so a damaged image still decodes the same
  way it does there. Saturating or checked arithmetic would have diverged.
- one carried a Huffman table whose DC symbol exceeded the standard's magnitude
  range, overflowing a shift. Now rejected as `InvalidMagnitudeCategory`.
- one declared a frame header shorter than its own fixed fields, so reading the
  component count indexed past the segment. Now `Truncated`.

Five sample divergences found by `matches_libjpeg`, all fixed:

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

- **Resynchronisation after mid-scan corruption.** 142 extraneous bytes before
  the end-of-image marker moved 496 samples. This was the long tail flagged
  when the decoder was proposed, and it was the only one that did not yield to
  inspection: both sides had to be instrumented and diffed block by block,
  which narrowed 496 differing samples to a single coefficient in one block of
  one MCU. Two causes, both places where libjpeg is more permissive than the
  standard reads. When an AC run pushes the coefficient index past the end of
  a block, libjpeg still consumes that coefficient's magnitude bits -- breaking
  out early leaves the bit reader a field behind, and every block after it
  decodes from the wrong offset. And it still *stores* the stranded value,
  because `jpeg_natural_order` carries sixteen padding entries that all point
  at coefficient 63, commented "extra entries for safety in decoder". Fixing
  the first took 496 samples to 59; the second took 59 to zero. The fixture is
  `../tests/data/corrupt-resync-80x80-420.jpg`.

- **Restart markers.** A restart interval of one, and a segment that runs out of
  data, moved 1511 of 12288 samples. libjpeg does two things here that a plain
  skip-to-the-next-`RSTn` does not: it expects a *numbered* marker and runs a
  resynchronisation policy when it meets the wrong one, and a successful restart
  **clears** its out-of-data flag. Without the second, a short segment renders
  flat grey to the end of the image instead of recovering at the next marker.
  Fixture `../tests/data/restart-resync-64x64-420.jpg`.

- **Integer widths in the inverse transform.** A 1x9 4:2:2 file whose chroma
  coefficients approach 8191 differed in every chroma sample and no luma one.
  The coefficients were identical on both sides, which is what ruled out the
  entropy decoder. libjpeg dequantizes in `int`, carries the butterflies in
  `JLONG` -- `long`, so 64 bits here -- and narrows back to `int` for the
  inter-pass workspace and the range-limit index. Doing all of it in 32 bits is
  indistinguishable until a product passes 2^31.
  Fixture `../tests/data/wide-coefficients-1x9-422.jpg`.

Four **accept/reject** divergences, found by the same target but reported
differently: libjpeg's error handler exits the process, so they surface as
`fuzz target exited` rather than as an assertion. Each was a validation libjpeg
performs and this decoder skipped -- a scan naming a component the frame never
declared, a trailing marker with an impossible length, a Huffman table whose DC
symbols exceed the magnitude range, and a second start-of-image. All four were
confirmed against Java's ImageIO, which raises `IIOException` on every one, and
are now pinned by `../../../oracle/jpeg-verdicts.txt`.

Two lessons worth keeping:

- A bitstream desync makes a local mistake look global. Sample diffs will not
  localize it; a coefficient diff will, because it is still local there. The
  converse also holds -- when the coefficients agree and the samples do not, the
  entropy decoder is exonerated and the arithmetic is the suspect.
- "Which files decode" is half of parity and no sample comparison can see it.
  Running the oracle for accept/reject separately is what turned four silent
  wrong answers into four errors.

Crashes land in `artifacts/`. Copy new ones into `tests/data/regressions` so the
fix stays covered without a nightly toolchain.
