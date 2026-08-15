![](https://github.com/Audiveris/docs/blob/master/images/SplashLogo.png)

# Audiveris, with a Rust port of the recognition engine

This is a fork of [Audiveris][upstream], the open-source Optical Music Recognition
application. The Java tree is unchanged and still builds and runs exactly as upstream's
does. Alongside it, under [`rust/`](rust/), is a port of the recognition engine to Rust.

The port's rule is **parity before progress**: every stage is checked against the
Java it replaces, running live, and a stage is not "done" because it produces
plausible output — it is done when it produces *the same* output.

## Status

Honest version: the port covers **4 of the 20 pipeline stages** and cannot yet
transcribe a note. What it does cover, it reproduces exactly.

| Working | |
| --- | --- |
| `LOAD` → `BINARY` → `SCALE` → `GRID` | staves, systems, staff lines, barlines, skew, scale |
| PNG, JPEG, PDF input | including multi-sheet PDFs |
| Structured JSON output | geometry, confidences, rejected candidates, and the evidence behind each grade |

| Not working | |
| --- | --- |
| `HEADERS` onward | the step lifecycles are ported; the recognition inside them is not |
| MusicXML export | absent, not stubbed |
| Notes, beams, stems, chords, rhythm, text | use the Java application |

Throughput is about **2.3 s per sheet** single-threaded, from PDF bytes to JSON.

## What "exact" means here

Not a tolerance — a hash, or a value-for-value comparison against a live Java run.

- **PDF ingest**: all 189 pages of a seven-source IMSLP corpus, at four depths — raw
  stream bytes, filtered bytes, decoded samples, and the rendered page — reproduce
  PDFBox byte for byte. That includes a from-scratch CCITT G4 and JBIG2 decoder, and
  Java2D's bicubic transform and `ScaledBlit` reproduced from OpenJDK's own loops.
- **JPEG**: sample for sample against libjpeg **6b**, the one Audiveris actually bundles,
  which differs from libjpeg-turbo on damaged input and on 4:4:0.
- **GRID**: 9/9 binary rasters bit-identical, 420/420 barline abscissae, 1300/1300
  completed staff-line endpoints, every SIG grade and contextual grade, and the
  staff-free image on all nine example pages.
- **Recognition on PDF sheets**: 392 promoted barlines across eleven corpus sheets,
  grades compared at 1e-9.

Some of that precision was earned the hard way. Six barline grades were wrong by 0.004
for three sessions; the cause was `Math.rint` rounding a half to even where Rust's
`f64::round` rounds it away from zero, in the one place a staff line is extrapolated
past its own ink.

## Quick start

```sh
cd rust
cargo test --workspace
cargo run --release -p audiveris-cli -- -batch -step GRID ../data/examples/chula.png
```

Structured output, one JSON document per sheet:

```sh
cargo run --release -p audiveris-cli -- -batch -step GRID -json score.pdf
```

Each promoted inter carries its grade, its contextual grade, and the impacts the grade
is a weighted geometric mean of — plus the candidates that were rejected and the named
purge that rejected each. Rejected peaks that reached a named purge retain the same six
intrinsic impacts, so a missed bar can be separated from a location where no candidate
was ever formed. A recogniser that emits only its answer cannot be judged on what it
missed.

### Experimental stem/barline disambiguation

The `codex/barline-precision` branch adds an opt-in post-parity filter for weak
interior peaks that are aligned across staves but have no actual connector ink.
It preserves staff boundaries, double/final-bar siblings, connected bars, and
full-height low-resolution evidence. If removing one weak half leaves a newly
orphaned aligned peer, a narrow 0.02 grade shoulder removes that peer only when
all of its removed partners also lack full-height core/gap evidence. Enable the
current measured cutoff with:

```sh
AUDIVERIS_WEAK_BAR_MIN_GRADE=0.71 \
  cargo run --release -p audiveris-cli -- -batch -step GRID -json score.png
```

The default is off to retain exact Java behavior. On the local synthetic
evaluation this changes ordinary piano barline precision from 97.41% to 100%
without changing recall; deliberately extreme 48-DPI failures show why the
low-resolution preservation clause is necessary. The corpus generator,
physical-stroke ground truth, and full taxonomy live in the separate
`stage-omr-data` repository.

Two additional opt-in research controls target projectively captured pages:

```sh
AUDIVERIS_BAR_MAX_ALIGNMENT_SLOPE=0.16 \
AUDIVERIS_ADAPTIVE_BAR_VERTICAL_SLOPE=1 \
AUDIVERIS_SLOPE_AWARE_BAR_PROJECTION=1 \
AUDIVERIS_SLOPE_RECOVERY_MIN_GRADE=0.72 \
AUDIVERIS_REASSIGN_LEFT_BAR_BOUNDARY=1 \
AUDIVERIS_BRACE_SELF_INCLUSIVE_FALLBACK=1 \
AUDIVERIS_WEAK_BAR_MIN_GRADE=0.71 \
  cargo run --release -p audiveris-cli -- -batch -step GRID -json score.png
```

The first widens the residual slope accepted between peaks after global
deskewing (valid range 0.06–0.25). The adaptive control robustly fits a linear
vertical-slope field across x from at least three pairs of intrinsic-grade
≥0.72 peaks; unlike a blanket tolerance increase, this recovered 12 warped-page
strokes with no new false positives in the 50-page benchmark. The final control
runs a supplemental projection that follows the fitted local vertical field
and retains only unique, high-grade,
full-height candidates; recovered peaks carry a provenance attribute so they
cannot lend double-bar protection to nearby weak ordinary peaks. These controls
confirm projection smear and perspective convergence as missed-bar causes, but
remain experimental: the tested global approximation is not yet as precise as
a per-system projective vertical field and lowered ordinary warped-page
precision in the first global-shear prototype. The current two-pass local-field
version preserves ordinary precision; `AUDIVERIS_SLOPE_RECOVERY_MIN_GRADE`
sets its inclusive intrinsic-grade threshold (0.72 by default).

The boundary-reassignment control targets a different projective failure: a
brace fragment can become the first connected vertical on both piano staves,
while the genuine system-start bars about one interline to the right are later
discarded as unaligned. It replaces the boundary only when both staves offer a
nearby candidate with at least 0.5 core evidence, their normalized offsets
agree within 0.45 interline, and their combined core exceeds the old pair by
at least 0.4. On the independent projective set this changed 2,742 TP / 9 FP /
114 FN to 2,748 / 2 / 108; ordinary, disconnected, and low-DPI unwarped sets
were unchanged. It remains opt-in pending validation on real scans.

The brace fallback addresses the complementary case where a warped brace edge
has already become peak zero. Java's brace lookup searches strictly to its
left, so it can miss the visible brace by only a few pixels and freeze both
outline edges as staff-start barlines. The fallback searches through peak zero,
skips rejected right-hand candidates (for example, a straight clef fragment),
and accepts only a brace filament that begins to the left of the boundary. It
retains the replacement's structural staff-boundary role but suppresses its
publication as a barline. On the independent warped ordinary set it removed
the last 2 false positives (2,748 TP / 0 FP / 108 FN), and on the 56-page
low-DPI set it removed another brace pair (1,074 / 0 / 46), without changing
clean or disconnected recall. GRID JSON now includes `brace_probes`, recording
every lookup window and the exact outcome (`NoCandidate`, width/filament/
height/curvature rejection, boundary-overlap rejection, or acceptance).

## Layout

```
app/            the upstream Java application, unchanged
rust/
  crates/       core, image, omr, cli, pdf, jpeg, classifier, testkit
  oracle/       pinned Java output, and the probes that generate it
  HANDOFF.md    current state, open threads, and what bit whom
  PORTING.md    the porting contract and a per-area status table
tools/omrscope/ a Qt6 window over both engines: run them on the same sheet,
                see where they differ, and see how much is actually ported
```

Start with [`rust/HANDOFF.md`](rust/HANDOFF.md), then
[`rust/PORTING.md`](rust/PORTING.md).

## Verifying against Java

`rust/oracle/` holds Java's answers and the probes that produce them.
`oracle/java/org/audiveris/omr/rustport/SigProbe.java` will dump every inter and
relation any pipeline stage leaves in the SIG, so **a stage nobody has started
porting already has a parity gate waiting**:

```sh
unset JAVA_TOOL_OPTIONS   # a proxy banner on stdout corrupts every parsed line
JAVA_HOME=/path/to/jdk25 ./gradlew --no-daemon -q \
  -I rust/oracle/java/staff-impacts.init.gradle :app:sigProbe \
  -PsigTargets="data/examples/chula.png:1:LEDGERS"
```

Two test suites need data that is not in the repository: the PDF corpus is 20 MB of
third-party IMSLP scans, and the Java oracles need a JDK 25. Both skip loudly rather
than passing quietly when their inputs are absent — a green run that says nothing is
not evidence.

CI runs formatting, Clippy with `-D warnings`, and the full test suite on both
`ubuntu-latest` and `macos-latest`. Two hosts because "bit-exact" is a claim about
every host or it is not a claim.

## Relationship to upstream

The Java tree here is Audiveris 5.11.0 at commit `9e1e55cd`, unmodified. All credit for
the application, the engine and its design belongs to the
[Audiveris project][upstream] and its authors, led by Hervé Bitteur.

This fork adds the Rust port and nothing else. It is not a release channel, it is not
affiliated with the Audiveris project, and if you want to *use* Audiveris you should go
[upstream][upstream] and install a real release.

## License

AGPL-3.0-or-later, the same as upstream Audiveris. The Rust port is a derivative work of
the Java application and is licensed identically; see [LICENSE](LICENSE).

[upstream]: https://github.com/Audiveris/audiveris
