# Notes for agents working on the Audiveris Rust port

Read `rust/PORTING.md` for parity status, `rust/HANDOFF.md` for where work
stands, and `rust/LINUX-SETUP.md` before running anything that touches the Java
oracle.

## Do not estimate work in human wall-clock time

When asked "how hard would this be", the instinct is to answer in days. That
estimate is calibrated on how long a human would take, and it is wrong by one
to two orders of magnitude for work an agent does itself.

### The incident this comes from

On 2026-08-04, JPEG decoding was going through libjpeg-turbo via C FFI, because
Java's `ImageIO` reader is libjpeg-backed and no pure-Rust decoder reproduces it
sample for sample. The question was whether to write one, to keep the build pure
Rust for a planned WASM target.

The estimate given was **2–5 focused days**, with a list of reasons it would be
hard: the fixed-point IDCT, the range-limit table, the colour tables, the fancy
upsamplers, merged upsampling, progressive block smoothing, and libjpeg's
error-recovery behaviour on damaged files.

The user replied: "I think you can rebuild it in less than an hour."

`crates/audiveris-jpeg` was written, verified sample-for-sample against libjpeg
across 25 generated fixtures and the full 1936×2592 corpus page, wired in, and
the C dependency removed from production code — in **about an hour**. Fuzzing
and three panic fixes followed shortly after.

Keep the comparison honest: the estimate covered baseline *and* progressive; the
hour delivered baseline only, with progressive explicitly refused rather than
approximated. Even adjusting for that, the estimate was off by more than an
order of magnitude, and one of the named hard parts (merged upsampling) turned
out not to apply at all — libjpeg only uses it when fancy upsampling is off,
and the default has it on.

### Why this matters more than being wrong about a number

The estimate did not sit there inertly. It came bundled with a recommendation:
*keep the C dependency.* The reasoning was that a multi-day rewrite was not
worth it against a well-fuzzed C library with a small FFI surface.

That recommendation was wrong, and it was wrong **because** the estimate was
wrong. A user who trusted it would have shipped a C dependency into a WASM
target and never learned it was avoidable in an afternoon.

Bad duration estimates do not just produce bad numbers. They quietly produce bad
architectural advice, and the advice is what gets acted on.

### What to do instead

- **Describe the shape of the work, not its duration.** What is mechanical?
  What needs discovery? What is genuinely uncertain? That transfers; "three
  days" does not.
- **Bias toward attempting over estimating.** For anything bounded and
  verifiable, trying it is often cheaper than the conversation about whether to
  try it.
- **Separate the parts.** "The transform is mechanical; matching libjpeg's
  error recovery on corrupt files is not" is useful. A single number averaged
  over both is not.
- **Never let an unstated duration estimate drive a recommendation.** If a
  recommendation depends on how long something takes, say so explicitly, and
  treat that as a signal to test rather than assert.

### Where scale genuinely does matter

Estimate honestly in the units that actually bind:

- **Context and tokens** — a sweep over the 327k-line Java source is real cost.
- **External wall-clock** — the Gradle build of the Java app, oracle runs across
  the corpus, large downloads. These do not speed up.
- **Human review** — a large diff costs a reviewer real time regardless.
- **Irreversibility and risk.**

The failure mode is specifically: *work the agent performs itself*, estimated in
*human* hours.

## Know what your oracle actually is

A differential test is only as good as the thing on the other side. On
2026-08-05 the JPEG decoder was checked against libjpeg-turbo and matched it to
the sample everywhere. It still did not match Audiveris, because "libjpeg" names
two libraries, and *which one Java uses depends on how the JDK was built*:
Temurin statically links the bundled libjpeg 6b, Ubuntu's OpenJDK package links
the system libjpeg-turbo.

The two differ in ways a port can see. Turbo added a vertical fancy upsampler
6b does not have, so they disagree on ordinary, well-formed 4:4:0 images; and
turbo widened the inverse transform's intermediates from `int` to `long`, so
they disagree wherever a product passes 2^31.

Two fixes had already been made against the turbo oracle -- widening the
transform, adding the vertical filter. Both were correct for turbo and wrong for
the target. Reverting both closed the last divergences rather than opening any.

What to take from it:

- **Name the oracle precisely, including its build.** "libjpeg" was not specific
  enough. Neither is "the JDK" -- check `ldd` on the native library.
- **Verify against the real target periodically, not only the proxy.** The proxy
  is there for speed and locality; if it is never checked against the thing that
  matters, its divergences become the port's.
- **A divergence the proxy reports is a question, not a verdict.** Ask the real
  target before writing the fix.
- **Enumerate the space when it is small.** Two sweeps -- all 254 marker bytes,
  all 140 legal sampling combinations -- found in minutes what fuzzing had been
  surfacing one case per twenty-minute run, and the sampling sweep is what
  exposed 4:4:0 at all.

## What has repeatedly worked on this port

Recorded because it generalizes better than any estimate:

- **Instrument both runtimes and diff, rather than reason about the divergence.**
  Every parity bug this session was found this way and none were found by
  inspection. Revert the Java instrumentation afterwards.
- **Check the input before the algorithms.** Four separate GRID symptom classes
  turned out to be one JPEG decoder difference, found by comparing the binary
  raster Java saves in its `.omr` against ours. That check should have come
  first.
- **Pin oracles as exact equalities, not ceilings.** A `<=` bound hides both
  regressions and fixes. Exact counts forced the ledger to be updated when the
  libjpeg fix silently improved three separate results.
- **Code that has only ever run under tests usually has exactly one thing wrong
  with it.** `RemainingRasterGridStages`, the GRID executor, and
  `is_fully_connected` each did.
