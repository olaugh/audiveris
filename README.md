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
intrinsic impacts. Projection-stage core/grade rejections and compact raw
above-threshold ranges are also reported, so a missed bar can be separated into no
projection ridge, failed peak construction, failed core/grade, or a later named purge.
A recogniser that emits only its answer cannot be judged on what it missed.

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

The best validated opt-in configuration for projectively captured piano pages
is:

```sh
AUDIVERIS_STAFF_EDGE_BAR_PROJECTION=1 \
AUDIVERIS_REASSIGN_LEFT_BAR_BOUNDARY=1 \
AUDIVERIS_BRACE_SELF_INCLUSIVE_FALLBACK=1 \
AUDIVERIS_WEAK_BAR_MIN_GRADE=0.71 \
AUDIVERIS_CONNECTED_BAR_MAX_ALIGNMENT_SLOPE=0.10 \
AUDIVERIS_RECOVER_STRONG_WIDE_PARTIAL_COLUMNS=1 \
AUDIVERIS_PROJECTIVE_STAFF_SLOPE=1 \
AUDIVERIS_RECOVER_PAIRED_ZERO_CHUNK_BOUNDARIES=1 \
AUDIVERIS_RECOVER_PAIRED_SUBTHRESHOLD_BOUNDARIES=1 \
AUDIVERIS_GLOBAL_BAR_ALIGNMENT_MATCHING=1 \
  cargo run --release -p audiveris-cli -- -batch -step GRID -json score.png
```

The wider global-alignment, bar-self-calibrated vertical-field, and generic
slope-aware projection variables described below remain diagnostic controls;
they are not part of this retained configuration.

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
sets its inclusive intrinsic-grade threshold (0.74 by default).

The staff-edge projection is piano-specific and avoids the circular failure in
which too few slanted bars survive to estimate their own direction. It measures
`dx/dy` from the left endpoints of each adjacent two-staff grand staff, takes a
robust median, shears one supplemental projection along that direction, and
requires at least three plausible grand-staff pairs, and admits only interior
candidates with core ≥0.9, gap ≥0.8, and grade ≥0.74.
The same robust direction now guides cross-staff alignment; previously a bar
could be strengthened by the oriented projection and then rejected against the
unrelated global staff-line skew.
Boundary recovery is excluded because the same transform straightens brace
flanks. On the fresh 50-page warped audit it changes 2,462 TP / 9 FP / 326 FN
to **2,537 / 9 / 251** with the combined mitigations; the older connected
holdout reaches 2,798 / 2 / 58.
Clean disconnected pages remain 700 / 0 / 0, low-DPI pages retain 1,074 TP / 46
FN, and the independent disconnected warp is unchanged at 425 / 14 / 255. It
is opt-in and should be enabled only for piano-like staff pairing.

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
publication as a barline. Because unconditional fallback removed eight valid
opening bars on rectified pages, the opt-in now activates only when independent
staff-edge geometry has absolute transformed vertical slope at least 0.02, or
when the main interline is at most 11 pixels. This removes all twelve residual
brace flanks in the fresh projective audit and the low-DPI brace pair while
preserving all 2,856 bars in the non-projective control. GRID JSON includes
`brace_probes`, recording every lookup window and exact outcome (`NoCandidate`,
width/filament/height/curvature rejection, boundary-overlap rejection, or
acceptance).

The connected-score alignment control first builds the conservative 0.06
graph, then rebuilds at the requested slope only when at least three concrete
cross-staff connections already establish a connected score. The partial
column control retains either a candidate at least 7 pixels wide with core
impact ≥0.8788, or a five-pixel candidate with core ≥0.60, gap ≥0.9, and both
lateral chunks ≥0.9. The latter balanced-chunk signature recovered five true
bars on the overlapping warped disconnected control and a further bar in the
final fresh connected audit without adding a false positive across the fresh,
held-out, unwarped, or low-DPI controls. Recovered peaks remain subject to
downstream purges and carry `partial_recovered` provenance in JSON. Both
controls remain opt-in pending real-scan validation.

Accepted vertical JSON evidence also includes staff-free and original-binary
lateral-ink measurements, plus separate top/bottom terminal attachment ratios
and maximum extensions. They quantify notehead/beam-like attachment around a
candidate but do not change recognition. The terminal signature catches all
six residual stems in the evaluated 45-page subset, but also catches two true
damaged bars, so it is not a safe numeric veto. The intended next discriminator
is post-HEADS same-staff notehead attachment: all nine labelled residual stem
false positives are near a stem, versus 0 of 1,554 matched true bars. The
current pinned Bravura head catalog must first cover the low point sizes where
these residual errors occur.

On the nine real example rasters, the retained GRID controls remove 24
default-accepted verticals; all 24 intersect a same-staff HEADS notehead graded
at least 0.5. Four surviving interior verticals also intersect high-grade
heads, so proximity alone is not promoted to a veto. A later STEMS-stage rule
should require an actual head--stem relation and protect staff boundaries and
connected bar columns.

HEADS also completed on 11 generated connected pages covered by the pinned
Bravura sizes: all 605 reported bars matched vector truth and none overlapped a
high-grade same-staff head. Other synthetic point sizes currently fail closed,
so this supports the semantic design but does not yet justify enabling it.

On the fresh audit, 244 of the remaining 251 misses occur on ten pages where
staff detection is incomplete. Restricting evaluation to the 38 pages with
exact staff geometry gives **2,139 TP / 0 FP / 7 FN**: 100% precision and
99.674% recall. The remaining stage-local misses are four unaligned strokes,
two below-minimum-grade projection peaks, and one partial column.

The projective-staff control targets that upstream cause. Audiveris normally
compares every long horizontal filament with one global page slope and rejects
a residual above 0.025. In the synthetic perspective captures, staff-perfect
pages have mean top-to-bottom slope spread 0.019, while underdetected pages
average 0.064. `AUDIVERIS_PROJECTIVE_STAFF_SLOPE=1` robustly fits line slope as
a linear function of page ordinate from the 24 longest straight candidates,
then applies the unchanged 0.025 gate to the local residual. On the fresh
warped audit it removes all ten underdetected-staff cases, raises exact staff
recovery from 38/49 to 46/49. With adaptive brace suppression and balanced
partial recovery and global matching, the complete 50-page set reaches
**2,832 TP / 0 FP / 24 FN** (99.160% recall); the older holdout reaches
**2,852 / 0 / 4** (99.860%). The 50
unwarped counterparts remain exactly **2,856 / 0 / 0**, and the low-DPI control
is **1,074 / 0 / 46**. All nine real example rasters emit byte-identical GRID
JSON with the projective option on and off. It remains opt-in because the
current fit assumes planar projective convergence rather than nonlinear page
curl.

The final boundary recovery addresses a discontinuity in the inherited grade:
the weighted geometric mean becomes exactly zero when either side-chunk impact
is zero, even if core, gap, and both edge derivatives are strong. It only
imputes a modest chunk floor when an already accepted peak on the paired piano
staff agrees at the same projectively transformed outer boundary. Two rejected
candidates cannot support each other, because that experiment selected ten
aligned non-bars for only two true bars. The accepted-boundary rule recovers six
fresh and four held-out strokes with no change on clean, low-DPI, or disconnected
controls. It remains an opt-in piano-structure policy.
Three alternating warm 18-worker runs measured 3.05 s median without this
last rule and 3.03 s with it, so its report scan and boundary checks add no
measurable wall-time cost at this scale.

The alignment matcher addresses a separate failure in Java's local conflict
resolution. Each peak independently votes for its best relation; in an
ambiguous two-by-two neighborhood those votes can leave only one of two
noncrossing bar pairs. The opt-in dynamic program chooses the maximum-weight,
order-preserving matching per adjacent staff pair, with concrete ink
connections lexicographically dominant over geometric alignments. It recovers
14 hard-warp and nine disconnected true strokes with no new false positives,
while clean, low-DPI, held-out, and nine real-example results remain unchanged.
Locally selected sheared-projection edges are frozen rather than globally
rematched: an independent 50-page 5-degree/2.5%-perspective stress run showed
that unrestricted rematching could retain two extra sheared impostors. The
hybrid changes that cohort from 2,767/4/89 to **2,778/4/78**, adding eleven true
bars without adding an error. One page has overlapping predicted staves and 11
inverted connection probes; those relations now fail individually instead of
aborting the sheet, yielding 67/68 labelled bars on that page.
A post-freeze fourth 50-page seed at the same severity scores 2,781/5/75; the
two unseen extra-hard seeds combine to **5,559/9/153** (99.838% precision,
97.321% recall). All 1,878 bars below 2 degrees are recovered, versus 129 misses
among 1,338 bars at or above 3.5 degrees; the difficult rotation sign reverses
between seeds, so this is magnitude/capture interaction rather than a fixed
clockwise bias.
Three alternating warm 18-worker runs measured 3.00 s median for local conflict
votes and 3.06 s for global matching, about 2% wall-time overhead.

GRID JSON also reports detrended centerline and core-ink-width residuals for
accepted verticals. They were added to test a rotation-invariant arpeggiation
veto, but remain diagnostic only: the two extra-hard impostors score 0.154/0.308
px and 0.710/0.657 px, while 14 true bars exceed 0.50 px centerline residual and
58 exceed 1.00 px core-width residual across the two independent 50-page
cohorts. The pixel-only rule is therefore rejected; downstream head--stem and
arpeggiation--chord relations are the safer future veto.

`AUDIVERIS_RECOVER_PAIRED_SUBTHRESHOLD_BOUNDARIES=1` addresses a different
failure: both opening strokes of a grand staff can sit below the ordinary
projection threshold after severe capture. It requires both half-threshold
ranges to lie at their detected left edges, align under the fitted projective
vertical field, and sum to one normalized threshold. Applying the same vote in
the interior is forbidden: the audit finds 3,121 aligned nonbar pairs there.
The boundary-only rule recovers 10 strokes on the primary hard set and 14 over
the two unseen extra-hard seeds, with no new false positive. Final results are
**2,842/0/14** on the primary hard set and **5,573/9/139** combined extra-hard
(99.839% precision, 97.567% recall). Clean, older holdout, low-DPI,
disconnected, and nine real-example results are unchanged.
Three warm 18-worker runs measured 3.88 s median both with and without this
boundary vote in the otherwise identical full configuration, so it adds no
measurable wall-time cost on the 50-page cohort.

Forty-five of the 50 fresh hard pages are fully exact. All 14 residual misses occur
on five pages combining the maximum 0.02 perspective setting with at least 3.2
degrees of rotation; the worst page contributes 13 misses. The failure is thus
concentrated at the synthetic stress boundary rather than spread over ordinary
pages.

With 18 page workers, three warm 50-page runs measured median wall time 2.78 s
for default GRID and 3.19 s for the full retained configuration (about 15%
overhead). The supplemental staff-edge raster projection dominates that cost;
the projective line model fits only 24 candidates.

One hard page previously aborted when two derivative candidates refined to the
same `StaffPeakKey`. Rust materialized the candidate list before applying
Java's mutable cursor, so both equal intervals escaped. Accepted projection
intervals are now explicitly non-overlapping and graph vertex insertion follows
Java's idempotent semantics. The recovered page contributes 64 matched bars;
the other 49 reports are byte-for-byte unchanged.

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
