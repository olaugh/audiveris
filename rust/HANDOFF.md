# Rust port takeover

This is the continuation record for the source-guided Audiveris Rust port. Read
`PORTING.md` first, then this file. The port is an AGPL-3.0-or-later derivative and
is intentionally parallel to the unchanged Java production tree.

## Repository state

- Repository: `/Users/john/sources/jul10-charter/omr/tools/audiveris`
- Branch: `codex/rust-port`; the current work is on `claude/rust-port-takeover`,
  branched from `8418c6a` and pushed to `github.com/olaugh/audiveris`. Rebase
  onto `codex/rust-port` if that has moved.
- Java baseline: Audiveris 5.11.0, source commit
  `9e1e55cd2746037d059345881c53e6a6754bffbd`
- Rust workspace: `rust/`
- JDK 25: `/Users/john/sources/jul10-charter/omr/tools/jdk25/Contents/Home`
- Java test baseline: 39 suites, 212 executions, 0 failures, 0 errors, 1 skip

The Java checkout has 991 production files and about 327,673 lines. Its unit suite
does not run the 20-stage recognizer, save an asserted `.omr`, or compare MusicXML.
Do not equate either Java or Rust unit-test success with recognition parity.

## Current status (read this first)

The CLI performs native page recognition through GRID. The corpus harness now
continues the same native state through HEADERS and BEAMS; CLI/JSON publication
of those two stages remains separate wiring work. `audiveris-cli -batch -step
SCALE|GRID <image>` runs `LOAD -> BINARY -> SCALE -> GRID` on a real raster,
with every threshold derived from the measured sheet scale.

Against a live Java 5.11 oracle across all nine `data/examples` pages:

| Output | Status |
| --- | --- |
| Binary raster | 9/9 pages bit-identical |
| Staff abscissae | 65/65 exact |
| Barline abscissae | 420/420 exact |
| Completed staff-line endpoints | 1300/1300 exact |
| Sheet SIG | all 420 barline inters and 184 connectors promoted; every median and every intrinsic and contextual grade exact on every page |
| Beam spot chain | all 8 transforms bit-identical, and all 305 of chula's spot glyphs by bounds, weight and centroid |
| Beam recognition | **787/787 raw beams across 8 sheets** -- system ownership, geometry, all six impacts, and grade exact from native GRID + HEADERS inputs. 7 of 8 sheets exact through the end of BEAMS |
| JPEG decoding | bit-exact against the libjpeg Audiveris bundles, on 130 fixtures and 140 sampling combinations |
| PDF reading | 189/189 corpus pages: geometry, image structure, raw stream bytes, and **every filter chain** byte-identical to PDFBox |

| Spot-to-system dispatch | 2739/2739 spot centroids on 8 sheets, exact |
| Stem scale | Java's `maxStem` on all 8 sheets, from the uncleaned raster |
| Symbol centroids | 6 header clefs, pinned bit-exact |
| Symbol outline bounds | 1624/1624 swept values on 14 shapes x 116 sizes, exact |
| Clef classification | 65/65 corpus staves: shape, symbol box and `clefStop` exact |
| Key classification | 65/65 staves: presence, fifths, union box and `keyStop` exact |
| Time classification | 65/65 staves: presence, value, symbol box and `timeStop` exact |
| Final `header.stop` | 65/65 staves exact; all **30** system header erases exact |
| Native beam composition | 2739 spots, 30 header erases, 787 raw beams, final beams/hooks and per-system group counts graded on all 8 sheets |
| Native ledger composition | chula: 9915 filtered runs, 4052 sections, 104 candidates, 19 builder survivors, one post-analysis rejection, and all 18 final Java inters exact by system, staff/index, median, thickness, 7 impacts, and grade |

`recognize_native_beams` now consumes only the native GRID report and HEADERS'
`HeaderErase` list: it measures `maxStem`, runs the spot chain, dispatches by
native system areas/bounds, then creates, extends, hooks and groups beams. The
oracle is the grader, no longer an input. This remains a corpus/test path until
the result is published through `-json` and the CLI stage driver.

`recognize_native_ledgers` now consumes that native BEAMS result plus GRID's
`NO_STAFF`, curved per-staff lines/areas, and system areas/bounds. It preserves
Java's distinct beam contracts: every beam/hook participates in the early
section purge, while only good full `BeamInter`s participate in the later
filament-middle purge. On chula, the native builder reproduced every Java inter
exposed by the first comparison exactly, including their seven impacts. A
compact LEDGERS-only probe corrected that incomplete result: the general SIG
probe output had been truncated to system 3, hiding nine earlier final inters.
The full Java path has 19 builder survivors; `LedgersPostAnalysis` computes
sheet-wide unbiased delta/height populations, rejects one outlier, removes its
filament, and rebuilds system 1, leaving 18 final inters. Rust now reproduces
all 18 exactly.

`cargo fmt --all --check`, strict Clippy, and `cargo test --workspace` are green
locally under the pinned toolchain, and the whole workspace runs in about 45
seconds since the dev profile went to `opt-level = 2`.

**Nothing is unverified by CI as of run `31130993732`**, which went green on
both legs with a full step list. That closes the `opt-level = 2` dev profile,
which spent a day unverified: GitHub Actions was in a major outage when it
landed (2026-08-06, incident from 15:22 UTC) and both its runs died in *Set up
job* before checkout. The second `java_hypot` fix in `beam_structure` was
likewise unverified for a while and was closed by run `31116910296`, green on
both legs including the `ubuntu-latest` one it exists for, since it fixes a
divergence macOS cannot see.

A note on reading Actions during that outage, since it cost time twice: runs
were being *created late*, not skipped. Two pushes appeared to trigger nothing
for over an hour, then both runs turned up at once and one immediately cancelled
the other by concurrency group -- which is correct behaviour, not a failure.
Wait and re-list before concluding a push did not trigger CI.

### The baseline JDK, which the repository does not carry either

`manifest.sha256` pins **Temurin 25.0.3+9**, and `xtask` looks for it at
`../jdk25/Contents/Home` relative to the repo -- i.e. a sibling of the checkout,
outside it. A fresh machine will not have it, and `brew install --cask
temurin@25` gives whatever 25.x is current (25.0.4 at time of writing), not the
pinned build. Fetch the exact one:

```sh
curl -sSL -o /tmp/t25.tar.gz https://github.com/adoptium/temurin25-binaries/releases/download/jdk-25.0.3%2B9/OpenJDK25U-jdk_aarch64_mac_hotspot_25.0.3_9.tar.gz
echo "7baab4d69a15554e119b86ff78d40e3fdc28819b5b322955c913cebfe3f6a37c  /tmp/t25.tar.gz" | shasum -a 256 -c -
mkdir -p ../jdk25 && tar -xzf /tmp/t25.tar.gz -C ../jdk25 --strip-components=1
```

That is the aarch64 macOS build; swap the asset name for other hosts, and get
the checksum from
`https://api.adoptium.net/v3/assets/release_name/eclipse/jdk-25.0.3%2B9`.

### Reproducing the PDF work on a fresh machine

The PDF parity test and its oracle need two things the repository does not carry.

**The corpus.** Seven of the twelve PDFs in the `imslp-pseudo` repo's
`manifests/acquired_scans.json`, downloaded to any directory, named by the
basename of the URL. `oracle/pdf-pages.txt` records which seven by name. Then:

```sh
AUDIVERIS_PDF_CORPUS=/path/to/pdfs cargo test -p audiveris-pdf --test corpus -- --nocapture
```

It prints `checked 189 pages, 189 images, 189 filter chains, 189 rasters,
189 draws, 189 renders; still unimplemented: {}`. Without the variable it
prints that it skipped, so a green run that says nothing is not evidence.

**PDFBox, only to regenerate the oracle.** It is not a Rust dependency; the
checked-in `oracle/pdf-pages.txt` is enough to run the test. To regenerate, take
the classpath from the Audiveris app and follow the header of
`oracle/java/PdfPageProbe.java`. On a JDK newer than the Gradle build's target,
`JAVA_HOME` has to point at JDK 25 or `:app:compileJava` fails with "invalid
source release"; and `JAVA_TOOL_OPTIONS` must be cleared, because a proxy banner
on stdout corrupts every parsed oracle.

The whole 189-page test takes about two minutes in release, half of it the
raster depth. Run it in release; in debug it is not worth waiting for.

Reproduced on a second machine, from a fresh clone and the seven sources
re-downloaded from the manifest URLs and checked against their `content_id`
SHA-256: `checked 189 pages, 189 images, 189 filter chains; still
unimplemented: {}` -- the raster count arrived after. The oracle names those files by the URL basename
**truncated to twenty characters**, which is what the directory has to contain.

## Two things that drifted, and neither was the port

Both surfaced on the first run of the gates on a second machine. Neither was a
behaviour change in this workspace, and both are the same shape: something the
tests are measured *against* moved.

### Which libjpeg, again -- now a build-time question too

`audiveris-jpeg`'s differential test decodes every fixture with `mozjpeg-sys`
alongside the port and requires equality. That reference turned out to depend on
the *host*: `mozjpeg-sys` compiles SIMD whenever it can -- unconditionally on
aarch64, and on x86 only when `nasm` is installed -- and mozjpeg's SIMD routines
disagree with mozjpeg's **own scalar C** on damaged input. Measured on
`corrupt-resync-80x80-420.jpg`: the scalar build returns `011e68ce7a923ae5`,
which is both Java's recorded raster and the port's, while the NEON build
returns `a5649ea51e999926`, 1032 of 19200 samples apart. So the same commit
passed on a machine without SIMD and failed on one with it, and the port was
never the side that moved.

The dev-dependency now sets `default-features = false`, dropping `nasm_simd`.
That is the right reference on the merits and not merely the convenient one:
libjpeg 6b, the library Audiveris actually reads through, has no SIMD at all.
It also removes `nasm` from the build requirements, which is part of why the
two-OS matrix is clean. `TURBO_DIVERGENCES` still names the eleven fixtures
where turbo genuinely differs from 6b; this was a different axis and does not
belong in that list.

### Three loops Clippy 1.96 rejects

`clippy::while_let_loop` began firing at 1.96.0 on `loop { let ... else {
break }; }`, which is the shape three ported decoder loops use -- two in
`ccitt.rs`, one in `jbig2/text.rs`. They are `while let` loops now; the
behaviour is unchanged and the corpus still reads 189/189. The rewrite is not
the point. The point is that a gate of `-D warnings` makes every Clippy release
a potential source of red on unchanged code, which is what the toolchain pin
below is for.

## Continuous integration

`.github/workflows/rust-port.yml` runs formatting, Clippy with `-D warnings`,
and `cargo test --workspace` on `ubuntu-latest` and `macos-latest`, on pushes
and pull requests that touch `rust/**` or `data/**`. It is separate from the
Gradle `build-and-test.yml`, which builds the untouched Java tree.

Two operating systems is not box-ticking. The matrix also spans two
architectures -- `macos-latest` is aarch64, `ubuntu-latest` x86_64 -- and that
is the axis the libjpeg divergence above sits on.

**What CI does not cover, and why.** The PDF corpus test needs 20 MB of
third-party IMSLP scans, so CI leaves `AUDIVERIS_PDF_CORPUS` unset and the test
skips. Making CI depend on a scan host's availability would buy coverage with
flakiness. The last workflow step therefore re-runs that one test with
`--nocapture`, so the log states which of the two it did rather than letting a
silent skip read as a pass. Nothing Java-backed runs in CI either: `xtask
baseline` and `xtask vectors` need JDK 25 and the parent OMR checkout, and the
oracle files they produce are checked in.

### The toolchain is pinned, deliberately

`rust/rust-toolchain.toml` pins the channel, for the reason the three rewritten
loops demonstrate: with `-D warnings` as a gate, an unpinned toolchain means a
commit that was green when written fails later with no change to the code.

Bump the channel in its own commit, with the lint fallout in that same commit.
Non-rustup installations ignore the file, so it binds CI and rustup users
without disturbing a Homebrew or distribution `cargo`.

## Structured output

`audiveris-cli -batch -step GRID -json <input>` emits one JSON document per
sheet, one per line. This is the interchange format, not a debug dump, and it
is shaped for two consumers that do not exist yet: an evaluation harness
comparing several OMR systems, and a repair loop that proposes corrections.

Three decisions worth keeping:

- **The envelope names its producer and schema.** A consensus front end diffing
  Audiveris against another system needs to know whose output it holds. The
  geometry and labels are meant to be comparable across producers; everything
  Audiveris-shaped sits under each inter's `evidence`, where a reader can
  consume it per-producer or ignore it.
- **`evidence.impacts` is the reason this exists.** A grade is a weighted
  geometric mean of six terms and the product alone is not diagnosable. Those
  six terms are what located the `rint`/`round` divergence that three rounds of
  source reading missed; a consumer can only use them if they are emitted.
- **`image.gray_digest` is a provenance stamp.** For a PDF it equals the
  FNV-1a-64 of the page PDFBox rendered, which the ingest test asserts, so two
  producers' outputs can be checked for having seen the same pixels before
  their disagreements are attributed to recognition.

**Candidates, not just answers.** `inters` are what survived, each with its
grade, contextual grade and six impacts. `candidates` are what did not: every
peak a `BarsRetriever` purge removed, with its span and the named stage that
removed it -- `PartialColumn`, `Unaligned`, `CClef`, `ExtendingBottom` and the
rest. A recogniser that emits only its answer cannot be judged on what it
missed, and `Unaligned` versus `CClef` are very different claims about the same
absent barline. Carmen, for instance, promotes 109 inters over 70 rejected
candidates.

That list is deliberately *not* advertised as a complete n-best: a peak that
never reached the purges, because it failed core validation or graded below
`Grades.minInterGrade`, is not in it. Widening it is a schema change, not a bug
fix.

Numbers are emitted at full `f64` precision, since exactness against Java is
the only property that makes them checkable. `-json` is a port extension and is
stripped before `Parameters` parsing, which mirrors Java's CLI and is pinned by
tests against it.

## The stage oracle, and how to grade a stage you have not ported yet

`oracle/java/SigProbe.java` prints every inter and relation a step leaves in
Java's SIG: identity, class, shape, staff, bounds, intrinsic and contextual
grade, frozen flag, and the impacts, with impact *names* taken from the inter's
own `GradeImpacts` so a head's terms and a barline's terms both print without
the probe knowing either. Output is sorted by inter id, so two runs diff
cleanly.

This exists because every stage so far got a bespoke probe, which is fine once
and a tax every time after. It is shape-agnostic: **a stage nobody has started
porting already has a parity gate waiting**, which is what lets several people
take different stages without each first building a way to check their work.

```sh
unset JAVA_TOOL_OPTIONS
JAVA_HOME=/path/to/jdk25/Contents/Home ./gradlew --no-daemon -q \
  -I rust/oracle/java/staff-impacts.init.gradle :app:sigProbe \
  -PsigTargets="data/examples/chula.png:1:LEDGERS"
```

Arguments are `<path>:<sheet>:<STEP>`, the sheet counted from one.

**Two things that will bite you.** `JAVA_TOOL_OPTIONS` must be cleared or a
proxy banner on stdout corrupts every parsed line. And Audiveris running from
`.class` files resolves its read-only resources as `Paths.get("res")`, relative
to the *process working directory*, while they live in `app/res/` -- so the
task runs from `app/` and absolutises the page arguments against the project
root to compensate. Without that, anything from HEADERS onward dies on a
missing `basic-classifier.zip` with an error that does not mention paths.

**How far it reaches, measured on chula:**

| Step | Inters in Java's SIG |
| --- | --- |
| GRID | 84 |
| HEADERS | 113 |
| STEM_SEEDS | 113 |
| BEAMS | 295 |
| LEDGERS | 313 |
| HEADS | reachable: the null-symbol failure was the probe skipping `MusicFont.checkMusicFont()`, now fixed |

So every stage the port is next to work on is gradeable today. HEADS is not,
and the reason is the MusicFont seam PORTING.md already lists as unported:
head recognition template-matches against font-derived symbols, so the port
needs MusicFont metrics before HEADS means anything -- in Java *or* in Rust.
That is a real ordering constraint, not a probe defect.

The narrower probes stay: `GridPdfProbe` generates the committed
`oracle/grid-pdf.txt`, and `StaffImpactsProbe` is the one that found the
`rint` bug.

## NO_STAFF is done; LEDGERS still needs two more inputs

**The staff-free image reproduces Java exactly on all nine example pages**,
including the JPEG. `oracle/grid-nostaff.txt` pins the FNV-1a-64 of Java's
`Picture.getSource(NO_STAFF)` and `no_staff.rs` matches every one.

The erasing was never the hard part. What was missing is that
`recognize_grid_lines` ran only `GridStepStage::BuildGrid` -- **GRID's own
`CleanStaffLines` stage had never run in the driver**, so every staff line was
still a `Filament`, no glyph had been registered, and there was nothing to
erase. The stage now runs, and the ordering that makes it work is Java's:
`rebuild_horizontal_lag` builds the table itself from the sheet's persistent
glyphs, exactly as `rebuildHLag` reads `Picture.getSource(NO_STAFF)` and
`Picture` builds it lazily from the glyphs `simplifyLines` just created. A
caller may still supply a table -- the fixtures do -- and then it is used as is.

Running that stage also changed nothing it should not: the barline,
completed-line-endpoint and SIG oracles all still pass.

One non-finding, recorded so it is not re-opened: the port's NO_STAFF digest
initially equalled chula's *gray* digest, which looks like the adaptive filter
returning its input. It is not. `oracle/grid-binary.txt` records the same
`2179468ede9f7ec6` for Java's BINARY raster, because chula.png is already
bilevel.

### Staff areas: done, and they exposed a containment bug

Every one of the 1209 lattice points on chula now agrees with Java's
`getClosestStaff`. `oracle/grid-closest-staff.txt` holds them.

The gate is behavioural on purpose: a `java.awt.geom.Area` is not worth
serialising, and nothing reads one directly -- `getClosestStaff` asks whether an
area *contains* a point and then breaks ties by distance. Grading that exercises
containment and the tie-break together, and it found three real divergences that
a structural comparison would have missed entirely.

**`Area.contains` is half-open, and the port had it exclusive.** `java.awt.Shape`
defines insideness so that a point on the boundary is inside when the space
immediately adjacent in the increasing-x direction is -- so the **left and north**
edges belong to the area and the **right and south** edges do not. The port
excluded all four. Two existing tests asserted the exclusive behaviour as though
it were Java's; both were wrong. Settled with a five-line `jshell` script rather
than by argument:

```
new Area(new Rectangle2D.Double(0, 0, 100, 100))
  contains(0,0) true   contains(50,0) true    contains(0,50) true
  contains(100,50) false   contains(50,100) false
```

No system test caught it because system areas are sampled well inside their
bounds. Staff areas reach the sheet edge -- a staff's north boundary is `y = 0`
when nothing is above it -- and there the exclusive rule assigned the point to
no staff at all.

**`StaffLine.yAt` does not extrapolate the spline.** Outside the line's own
abscissa range Java extrapolates along the straight chord between the line's two
endpoints, and uses the spline only inside. That difference only shows beyond
the notated staff, which is where two staff areas both contain a point and the
distance decides. Using spline extrapolation there was worth 14 of the 1209.

**`Staff.distanceTo` returns an `int`.** `getClosestStaff` compares
`(int) doubleDistanceTo(point)`, so distances within a pixel of each other tie
and the strict `<` leaves the earlier staff holding the point.

One thing is deliberately not reproduced. Java reads
`SystemInfo.getAreaEnd(LEFT/RIGHT)` and notes it "may not be known yet"; this
port never computes system area ends, so it passes zero and the intersection is
skipped, leaving each staff spanning the sheet. That is what Java does with
unknown ends, and it is what the lattice confirms -- but if system area ends are
ever computed, this has to start reading them.

### The builder

`build_population_staff_areas` is Java's `StaffManager.computeStaffArea`: a
horizontal slice between the staves above and below, intersected with the
containing system's area ends. Two things in it are Java's rather than
simplifications, and both are pinned by tests -- there is **no vertical
margin**, unlike a system area, and Java's guard
`(left != 0) || ((right != 0) && (right != sheetWidth))` means an unknown pair
of ends leaves the staff spanning the sheet while a left without a right yields
a *negative-width* slice and therefore an empty area. That last one is left to
fall out of `contains` rather than special-cased, because it is what Java does.

The neighbour walks are now shared. `SystemManager.vertNeighbors` and
`StaffManager.vertNeighbors` have identical bodies in Java, so the port's
`vertical_neighbors`/`horizontal_neighbor` are generic over a small `Placed`
trait rather than transcribed twice, which is one fewer place to drift.

**The production wiring is closed.** `GridLinesRecognition.staff_lines` now
publishes each staff's curved first and last line, while `staff_areas` publishes
the corresponding closest-staff area. `native_ledgers.rs` consumes both
directly; it no longer substitutes the per-system boundary collection.

The gate is already generated and is behavioural rather than structural, since
a `java.awt.geom.Area` is not worth serialising and the only consumer is
`getClosestStaff`. `SigProbe` emits a `closest <x> <y> <staff>` record over a
64-pixel lattice; chula gives 1209 points across its six staves, every one
assigned, and the native closest-staff gate matches all of them.

### What LEDGERS still needs

The former input and post-analysis blockers are closed. `native_ledgers.rs`
composes the real native GRID and BEAMS products, and `ledgers-chula.txt`
grades all 18 final Java inters to nine decimals. Three details were
load-bearing:

1. `LedgersFilter` removes sections intersecting **any** `AbstractBeamInter`,
   hooks included. `LedgersBuilder` separately removes candidate middles only
   under good full `BeamInter`s. These cannot share one beam list.
2. Candidate checks use `StraightFilament`'s inclusive-pixel endpoints, but a
   materialized `LedgerInter` gets `Glyph.getCenterLine()` on the glyph contour:
   the right edge is exclusive and rows are centred at `y + 0.5`. Java also
   uses endpoint midpoint for rough containment and bounds centre for the
   staff-line reference; conflating those points perturbs every pitch impact.
3. Java tests a beam's geometric area against `Section.getBounds()`, not
   sampled section pixels. The post-analysis populations use the unbiased
   standard deviation and Java's floor/ceil integer checks; a ledger reused in
   several staff-map entries contributes every observation but only its last
   entry supplies the identity-keyed filter record.

The native builder has 19 chula survivors. The sheet-wide post-analysis rejects
one, removes its candidate filament, and rebuilds system 1; all 18 final inters
then match Java by ownership, geometry, thickness, seven impacts, and grade.
What remains is final inferred ledger-line construction, the same exact gate on
the other example pages, and CLI/JSON publication before calling the stage
native and graded.

## BEAMS: scoped, and its first seam is grayscale morphology (CLOSED)

Closed. `audiveris_image::morphology` ports `StructureElement`'s circular
element and `MorphoProcessor::close`, and both are bit-exact against Java --
every structuring element cell for cell, and the closing digest for digest,
including on chula's 4.8-million-pixel staff-free page at the radius BEAMS
would actually use.

The end-to-end gate the section below anticipated turned out not to be needed.
`oracle/java/MorphoProbe.java` calls `MorphoProcessor.close` directly instead of
going through `SpotsBuilder`, so the closing is graded on its own rather than
through the beams it eventually produces, and `oracle/morphology.txt` pins:

- twelve structuring elements as pictures and offset vectors, not digests, so a
  disk that is one cell wrong says which cell;
- the closing over two generated buffers -- formulas rather than fixtures, so a
  port rebuilds the inputs -- at six radii, with the 24x16 pair dumped pixel for
  pixel;
- the closing over chula's NO_STAFF buffer at three radii, including 4.3, the
  one `SpotsBuilder` derives from its beam thickness of 12.

It also pins the four buffers `SpotsBuilder.getBuffer` passes through --
stem-run removal, median, gaussian, and the closing of all three -- which
morphology does not need but the rest of the step does. **That is the next
slice**: `RunTableFactory.LengthFilter`, then `Picture.medianFiltered` and
`gaussianFiltered`, each already having a digest to answer to.

Only the circular element and `close` are ported. The other element shapes and
the histogram-based `fclose`/`fopen` are unreachable from Audiveris and were
deliberately left out.

What follows is the scoping that led here, kept for the parts still true.

### The original scoping

LEDGERS' three inputs are now NO_STAFF (done), staff areas (done), and BEAMS.
BEAMS is a stage rather than an input, and it is largely ported already --
6613 lines across `beams_step.rs`, `beam_structure.rs`, `beam_extension.rs`,
`beam_hooks.rs` and `beam_groups.rs`, covering candidate ordering, the
border/core/belt impacts, hooks, grouping and multiple rests.

It is driven through a `VisualBeams` trait with eight methods, and the native
kernel behind most of them already exists. Encouragingly,
`NativeBeamKernelConfig.pixel_filter` is documented as "Java
`Picture.SourceKey.NO_STAFF`, not the morphologically closed spot image" -- so
the piece finished two commits ago is exactly what it wants.

**The seam that is genuinely missing is morphology.** `close_beam_spots` is
Java's `SpotsBuilder.close`:

```java
final double diameter = beam * constants.beamCircleDiameterRatio.getValue();
final float radius = (float) (diameter - 1) / 2;
final StructureElement se = new StructureElement(0, 1, radius, new int[]{0, 0});
new MorphoProcessor(se).close(buffer);
```

a grayscale closing with a circular structuring element sized from the measured
beam thickness. `StructureElement` and `MorphoProcessor` are 717 and 446 lines
of Java, and at the time the port had no morphology module at all.

`oracle/beams-chula.txt` still pins Java's 91 beams and 31 hooks with the six
impacts each grade is built from (`wdth`, `minH`, `maxH`, `core`, `belt`,
`jit`), and remains the gate for the recogniser above the closing.

## BEAMS (CLOSED)

The step's whole output is reproduced exactly on chula -- **91/91 beams, 31/31
hooks, 60/60 beam groups, nothing spurious** -- graded against Java's own SIG at
the end of the step, not against an intermediate.

All four stages are wired: `createBeams`, `extendBeams`, `buildHooks`,
`BeamGroupInter.populateSystem`. Two things about the last two are worth
carrying forward. `buildHooks` runs over the spots that produced *no* beam, so a
spot `checkBeamGlyph` refused is still a hook candidate -- that is where 11 of
the 31 hooks come from -- and its overlap test runs against a list that grows as
the pass adds to it. Grouping is **per system**: run globally over the page it
merges beams across a boundary Java never compares, and 60 groups become 48.

One measured limitation. `extendBeams` is wired with no stem seeds, which
disables `extendToStem`, because STEM_SEEDS' vertical geometry is not ported.
Comparing beam medians before and after the stage across the eight example
sheets and 30 systems, `extendBeams` fires **once** -- a merge on
BachInvention5's sixth system, which is `extendToBeam` and is wired --
and `extendToStem` and `extendToSpot` never fire at all. It closes when
STEM_SEEDS lands.

The header erase remains the other open input, and it is priced in the section
below: five spurious clef-sized candidates out of 100 on chula, zero real beams.

## BEAMS native corpus path (CLOSED; CLI/JSON publication remains)

The beam pipeline is exact -- 787 of 787 raw beams across the eight example
sheets -- and now runs end to end from native GRID and HEADERS inputs. The three
inputs that originally kept it oracle-fed are all closed:

1. ~~**`scale.getMaxStem()`**~~ **(closed, `2982cef69`)**, for the stem-run
   removal that opens the spot chain. Not cheap for the reason first given --
   `StemScaler.getBuffer` cleans the raster before counting -- but the cleaning
   turned out not to move the mode on any of the eight sheets. See "Next
   session: start here".
2. ~~**`Staff.getHeaderStop()`**~~ **(closed)**, for `eraseHeaderAreas`.
   HEADERS now supplies all 30 native rectangles. Measured cost of omitting the
   chula rectangles remains five spurious clef-sized candidates and zero real
   beams, so this input is not optional even though that page loses no true beam.
3. ~~**System areas**~~ **(closed, `2982cef69`)**, for `dispatchSheetSpots`.
   `GridLinesRecognition` now carries `system_areas` and `system_bounds`, graded
   over 2739 centroids. Wiring rather than a port, as predicted, with one
   correction: the dispatch reads two different left/right pairs, not one.
   Without it a spot cannot be assigned to a system, and the system decides
   which spots each `BeamsBuilder` sees, what `buildHooks` searches, and how
   `BeamGroupInter.populateSystem` partitions -- grouping run sheet-wide instead
   of per system turns chula's 60 groups into 48.

`recognize_native_beams` is the honest composition boundary. What remains here
is output integration: publish its beam/hook/group records through `-json` and
the CLI stage driver. A measured small-beam scale is refused loudly because no
example sheet grades that class; none of the eight beam sheets has one.

## Push to `master` only

`claude/rust-port-takeover` was merged into `master` and the two have been
identical since. Pushing every commit to both fired **four** CI runs per commit
-- two of them on `ubuntu-latest`, competing for the same hosted runners -- and
half of it was duplicated work on an identical tree.

So: push to `master`. The branch is left where it is rather than deleted, and
can be fast-forwarded when it is actually wanted.

This is worth knowing when reading a red run, too. Three consecutive `ubuntu`
failures during this work were GitHub infrastructure rather than code --
`Service Unavailable` resolving the actions, an HTTP timeout, and "the job was
not acquired by Runner of type hosted" -- each failing in *Set up job*, before
checkout. The `macos` leg of the same runs passed. A short run duration used to
be the tell; once the dev profile went to `opt-level = 2` a genuine full run is
only two or three minutes, so read the step list instead.

## MusicFont: deferred with a price, not dropped

The port targets full parity -- every stage, including this one. What follows
is an ordering argument, not a scope cut.

**It is the geometry that needs the font, not the classifier.** `ClefBuilder`
calls the classifier with a null `ShapeChecker`, and both the bundled model and
`rank_evaluations` are already ported. Two font calls are the blocker:

- `getSymbolBounds` needs `TextLayout.getBounds()`, the glyph outline at a
  point size;
- placing that box needs `ShapeSymbol.computeCentroidOffset`, which rasterises
  the glyph to an **antialiased alpha image** and takes an alpha-weighted
  centroid.

That is Java2D's font rasteriser: native, hinted. Unlike the bicubic image
transform -- which was ported bit-exact because OpenJDK's `ScaledBlit` and
`TransformHelper` fully specify it in Java -- this one hands off to native
code, so bit-exactness is not something a reimplementation can promise up
front. Expect to need differential probing against the live JVM, and expect
the honest answer to possibly be a stated tolerance rather than a hash.

**What sits on it.** `header.stop` comes from `maxClefOffset`, which comes from
`staff.setClefStop`, which comes from that font-derived box. So MusicFont is
under HEADERS' clef geometry *and* under all of HEADS, where Java itself cannot
reach the step without it.

**What it is worth to BEAMS, measured rather than assumed.**
`header_erase_cost_is_measured_not_assumed` runs chula's whole spot chain both
ways:

```
header erase: 305 spots /  95 accepted
without:      333 spots / 100 accepted
only with erase (lost without it): []
only without erase (spurious):     5, at x=111..232, sized 33x16, 34x17, 32x16
```

Five clef-sized false positives in the header region, and zero real beams. That
is why BEAMS goes first -- not because the erase does not matter. The ratio is
pinned by that test, and the test fails if the erase ever becomes load-bearing
for a real beam. It should be re-measured on other pages rather than assumed to
hold.

## Historical plan that led to native HEADERS and BEAMS

This list is retained as the dependency record. Items 1 and 2 are closed, and
the native corpus path in item 3 is closed; only publication remains there.

1. ~~**The CFF/OTF outline parser**~~, which the MusicFont thread below shows is the
   one piece with no shortcut left. `rust/oracle/music-font.txt` is already its
   grading oracle. The JDK question that used to head this list is answered: the
   sweep is bit-identical under OpenJDK 26.0.1 and Temurin 25.0.3+9.
   (CI is clean too -- run `31134170478`, both legs, full step list.)
2. ~~**The header erase**~~, which was the *only* thing between the beam pipeline
   and running natively end to end. It is `Staff.getHeaderStop()`, so it is
   HEADERS, so it is MusicFont -- see the MusicFont thread below. It shows up
   twice, in `SpotsBuilder.eraseHeaderAreas` and again inside
   `StemScaler.getBuffer`, and both are the same dependency. Closed by the
   65/65 header chain and the 30/30 erase grade.
3. **Beams into `-json`**, then into omrscope's Page and Inters tabs, which
   currently show GRID-level inters only and so cannot display any beam work.
   The native recognition input is now ready; only publication remains.
4. **LEDGERS post-analysis**, now that native GRID/HEADERS/BEAMS-to-builder
   composition and its first exact gate are closed.

### Closed here: `scale.getMaxStem()`

Not cheap for the reason previously given, and the previous note was wrong about
why. `compute_stem_scale` was indeed already graded -- but `StemScaler.getBuffer`
does not count runs in NO_STAFF. It erases barline and connector inters, erases
each system header (`useHeader` defaults to **true**, and `eraseSystemHeader`
reads `getHeaderStop`), and paints white outside the core staff paths.

Rather than port all three, it was measured: the uncleaned NO_STAFF raster
reproduces Java's `maxStem` on all eight sheets, including the two where Java
says 5 rather than 4. A mode over ~10^5 runs is not a statistic a tail moves.
`stem_scale_from_the_uncleaned_no_staff_is_measured_not_assumed` asserts it per
sheet, so the first page that needs the cleaning names itself.

### Closed here: system areas

`GridLinesRecognition` now carries `system_areas` and `system_bounds`.
`build_population_system_areas` was already written and graded; what was missing
was that `dispatchSheetSpots` reads **two** left/right pairs -- the area's, which
are midpoints to its neighbours, and the system's own staff extremes.

Graded over all **2739 spot centroids on all eight sheets**, exact, as a pure
function of the centroid -- which keeps it independent of the spot chain that
cannot yet produce those centroids natively. Dropping the abscissa test alone
moves 5 of the 2739, one being the carmen top-right spot that invents a beam.

## HEADERS: what it actually needs (measured 2026-08-06)

`HeadersStep.doSystem` is one line: `new HeaderBuilder(system).processHeader()`,
producing clef, key and time per staff. `getHeaderStop()` -- the thing beams and
`StemScaler` both want -- falls out of that.

### The classifier is not a blocker: it is already ported and graded

An earlier note here listed the classifier as the first two items of work. That
was wrong -- `crates/audiveris-classifier` already carries all of it:

- `mix_glyph_features`, the 110-value `MixGlyphDescriptor`: the 20x5 ART moment
  grid (`F001`..`F194`) from `BasicARTExtractor` with its LUT and bilinear
  interpolation, plus `weight width height n20 n11 n02 n30 n21 n12 n03 aspect`
  from `GeometricMoments`. Java's traversal orders are preserved deliberately,
  including the backwards two-pass accumulation and `coeffImag -=`;
- `BasicClassifier`, parsing `app/res/basic-classifier.zip` (a 110-149-149
  single-hidden-layer MLP in plain XML, *not* a deep net) and running
  `NeuralNetwork.forward`'s last-index-down accumulation order;
- `rank_evaluations`, Java's `byReverseGrade` sort with `Double.compare` NaN
  canonicalization, the min-grade break and duplicate-shape suppression.

Graded against the live Java oracle in `RustParityProbe`. `ClefBuilder` calls
the classifier with a null `ShapeChecker`, so nothing more is needed from it.

### MusicFont is the whole of what is left, and it splits in two

Two font-derived quantities reach the SIG, and they are *not* the same problem.

**1. `getSymbolBounds` -> `TextLayout.getBounds()`: ported, and the arithmetic
is not what it looked like.** `MusicFont.getPointSize(interline)` is exactly
`4 * interline`, so the point size is not a source of slop. The law, recovered
from a 116-interline sweep of all six clefs (`rust/oracle/music-font.txt`):

> each *edge* of the box is independently rounded to a 1/64 px grid, after
> scaling a per-shape em-unit outline extreme by the point size.

Per-edge, not per-dimension -- the width is `right - left` **after** both have
been quantized, which is why widths alone never fit a clean law. The grid is
**1/64**, not 1/32: at 1/32, 924 of the 2784 swept values are off-grid; at 1/64,
none are.

An earlier note here claimed the em constants looked like integers over
`unitsPerEm = 1000`, so the whole thing could be pinned as four numbers per
shape. **That is wrong and the sweep is what caught it.** The fitted constants
are near-integers but not integers (F_CLEF's right edge is 684.00025/1000), as
they must be: a glyph bbox includes cubic Bezier *extrema*, which are irrational
in general even when every control point is on the grid. Fitting them from
measurements leaves an interval about 1e-6 em wide, and that is not tight
enough. Concretely, 22 of the 24 clef edges are reproduced exactly at all 116
sizes, and the other two fail identically:

```
G_CLEF / G_CLEF_8VB, top edge, interline 17 (point size 68):
  fitted em constant   -1097.99961/1000
  law predicts         -74.65625
  Java gives           -74.671875     (off by exactly 1/64)
  the product lands at -4778.4943, which is 0.006 from the -4778.5 tie boundary
```

So the failure is not the law, it is the precision of a *fitted* constant near a
rounding tie -- and interline 17 is an ordinary sheet, not a contrived one. The
exact constants have to come from the font. That means a real CFF/OTF outline
parser with correct Bezier extrema: specified work, no pixels painted, but not
avoidable by pinning.

**2. `ShapeSymbol.getCentroid` -> `computeCentroidOffset`: pin it, do not port
it.** This walks the alpha channel of a *rendered* glyph and takes an
alpha-weighted centroid.

A hypothesis worth recording as refuted, because it looks right in the source:
`buildImage` sets `KEY_ANTIALIASING` to `VALUE_ANTIALIAS_OFF`, which suggests
the alpha channel is binary and the centroid is just a coverage-mask mean. It is
not. The measurement found **~200 distinct alpha values** in each rendered clef.
`KEY_ANTIALIASING` governs *shape* rendering, while the symbol is drawn via
`TextLayout.draw`, which obeys `KEY_TEXT_ANTIALIASING` -- left at the platform
default, and on. This is antialiasing coverage, and reproducing it exactly does
mean reproducing Java2D's text rasteriser.

What makes that not matter: **the offset is a constant, not sheet data.**
`computeImage` renders at the fixed `SampleRepository.STANDARD_INTERLINE = 20`
regardless of the font it was asked for, so the offset depends on
`(family, shape)` and on nothing else. Measured at seven interlines from 10 to
48, the returned offsets are bit-identical:

```
F_CLEF          -0.03884001107392759, -0.13394309933117415
G_CLEF           0.00205725082845354,  0.01888306366816295
G_CLEF_8VA       0.01336868758375387,  0.04381791002555180
G_CLEF_8VB       0.00052491308994185, -0.01328569918562944
C_CLEF          -0.06580471127152870, -0.01731409766015940
PERCUSSION_CLEF -0.02309224973860641, -0.01249250593760271
```

Six numbers per shape-set for Bravura, and that is the entire header-clef table.
Pinning them is the same move as shipping the classifier's trained weights: data
rather than logic. Note the quirk that makes this safe -- `ClefBuilder` passes
`MusicFont.getPointSize(...)` where an *interline* is expected, so the symbol it
retrieves is sized wrongly; it does not matter, because the offset ignores the
size entirely.

Re-measure with `./gradlew -I rust/oracle/parity.init.gradle :app:musicFontScout`
(`MusicFontScout.java`), which writes the rows in `rust/oracle/music-font.txt`.
It needs `workingDir = app/`, since `WellKnowns.RES_URI` is `Paths.get("res")`
outside a jar. Default family is `Bravura` (`Bravura.otf`), in `app/res/`.

**On the first row of that file being the JDK.** Every other oracle here is Java
arithmetic, which is specified and portable. These values are not: they come out
of Java2D's font machinery, so they are only as portable as the runtime that
made them, and that needs checking rather than assuming.

*JDK axis: checked, and clean.* The sweep was captured under both **OpenJDK
26.0.1** and the baseline **Temurin 25.0.3+9**. All 711 value rows are
bit-identical; the only line that differs is the one naming the runtime. The
checked-in copy is the Temurin one. So the twelve centroid offsets are safe to
pin, and `getPointSize`, the 1/64 grid and the per-edge rounding do not move
across a major JDK version.

*Print the values with `Double.toString`, never `%.17f`.* The first capture used
`%.17f`, which is seventeen digits **after the point** -- only sixteen
significant digits at these magnitudes, one short of what a `double` can need.
It silently truncated: G_CLEF's x read `0.00205725082845354` where the value is
`0.0020572508284535385`. Harmless to the eventual pixel, and exactly the kind of
thing a port graded on bit-exactness should not be quietly carrying.

*Platform axis: not checked, and CI will not check it for you.* This was
measured on macOS/aarch64 only. `TextLayout.getBounds()` is outline-derived and
ought to be portable, but `centroidOffset` comes from **glyph rasterisation**,
which does not go through Marlin -- it goes through the platform font scaler,
FreeType on Linux against CoreText on macOS. That is exactly the axis the CI
matrix exists for, and exactly the axis it cannot see here, because the Java
oracle runs only locally: a Rust test asserting pinned constants compares them
against themselves and passes on `ubuntu-latest` whatever Java would have said
there. If the offsets ever need to hold on Linux, that has to be measured on
Linux. Until then, treat them as macOS-derived constants that happen to be
JDK-stable.

### Java's font scaler is fixed-point, and that is the whole story

The last four of the 696 swept values refused every floating-point model, and
chasing them turned out to be the most useful thing in this thread.

Scaling the exact font-unit box by `pointSize / unitsPerEm` in `f64` and
rounding each edge to 1/64 gets **692 of 696**. The four misses are G_CLEF and
G_CLEF_8VB -- the two glyphs whose top is at 1098 font units -- at interlines 17
and 108. They cannot be fixed by a better constant: interline 17 needs
`max_y >= 1098.0018` and interline 108 needs `max_y < 1097.9999`, so no single
value satisfies both, and Java is therefore not linear in size here.

Things ruled out by measuring rather than by argument:

- *A bad outline.* Asked at point size 1000 (scale exactly 1), Java's own
  `getGlyphOutline` returns integer coordinates that match the Rust parse
  segment for segment, including the `(455, -1098)` endpoint that sets the top.
- *Hinting.* The deviation is 1/128 of a pixel. A hint snap moves an edge by a
  half or whole pixel, not by 0.008.
- *`float` arithmetic.* Five different f32 orderings were tried; every one
  reproduces the f64 answer, because the gap needed is ~14 float ulps.
- *Control-point pre-quantization.* All six clef extremes are at *on-curve*
  points, which round identically either way.

What it is: Java's scaler does FreeType's integer fixed-point arithmetic, and
rounds **twice**.

```
scale_16_16 = FT_DivFix(pointSize * 64, unitsPerEm)   // = (a<<16 + b/2) / b
coord_26_6  = FT_MulFix(font_units, scale_16_16)      // = (a*b + 0x8000) >> 16
```

Two roundings can land one 1/64 step away from a single rounding of the exact
product, and those four rows are exactly that. This model reproduces **696 of
696** with no exceptions. The lesson generalises: any other Java2D font quantity
this port needs is likely fixed-point too, so reach for `FT_MulFix` before
reaching for a tolerance.

**One deliberate gap.** Every clef box is set by an on-curve point, so all six
are whole font units. A box set by a curve *interior* would expose an ordering
question -- Java quantizes points to 26.6 and solves for extrema there, which is
not the same as solving in font units and scaling after -- and nothing in the
sweep grades it. `layout_bounds` returns `FontError::UngradedOutline` rather
than guessing. The first shape that needs it (heads, most likely) has to extend
the sweep first.

### The tolerance, if a rasteriser is ever wanted anyway

`ClefBuilder` uses the offset only as
`rint(box.getCenterX() + box.getWidth() * offsetX)`, so an error in `offsetX`
smaller than `0.5 / box.width` rounds away. Measured over the clef shapes at
interlines 10..48 that budget is 0.0037 (widest, interline 48) to 0.033
(narrowest, PERCUSSION at interline 10), typically ~0.008 at corpus interlines.
Since the offset is a mean over 1300-2900 covered pixels normalised by a ~55px
image width, that is roughly half a pixel of allowed centroid drift -- a loose
budget for a rasteriser, though not a guaranteed one, since a value landing near
a `.5` boundary has no margin at all. Pinning avoids the question.

### Order from here

1. ~~`TextLayout.getBounds()` for the six header clefs.~~ **Done** -- a CFF/OTF
   parser in `crates/audiveris-music-font`, graded 696/696 against the sweep.
   The interline 17 row was indeed the one that discriminated, but not for the
   reason predicted; see below.
2. ~~The pinned `(family, shape) -> centroidOffset` table.~~ **Done** --
   `crates/audiveris-music-font`, which also carries `getCentroid`'s
   `rint(centre + size * offset)` and `getPointSize`. The offsets are compared
   to `music-font.txt` **by bit pattern**, not by tolerance: `Double.toString`
   and Rust's `f64` parser both guarantee shortest-round-trip, so an exact match
   is available and anything weaker would hide a transcription slip. Perturbing
   one constant's last decimal digit fails that test and only that test.
3. `ClefBuilder`. **Partly done**: `clef_classifier.rs` implements the
   production `ClefShapeClassifier` -- noise gate, features, MLP, Java's
   rank-then-filter order, and `getSymbolBounds`. What is *not* done is the
   grading, and that is the next real task; see below.
4. `KeyBuilder` and `TimeBuilder`, whose columns are also already written.
5. `getHeaderStop()`, which closes the beam and `StemScaler` erase dependency.

### `clef_classifier` is wired but ungraded, which is not the same as done

`clef_column.rs` always had the `ClefShapeClassifier` seam; the only
implementation was a test double returning `glyph.bounds` as the symbol box.
That is now a real one, and every piece it composes is separately graded -- the
110 features and the MLP against `RustParityProbe`, the font box 696/696 against
the sweep.

**The composition itself is not.** Its unit tests cover the noise gate, the
rank-then-filter order, the drum-staff shape set, and the two independent
roundings in `getSymbolBounds` (`rint(w/2)` is not `rint(w)/2`; C_CLEF at
interline 23 is the case that separates them). None of that is evidence that
Rust picks the same clef as Java on a real page.

That oracle now exists: `rust/oracle/clef-headers.txt`, 65 staves across the
nine corpus pages, every one of them carrying a clef (52 `G_CLEF`, 10 `F_CLEF`,
3 `G_CLEF_8VB`). Each line has the staff's specific interline, the header start
and stop, `clefStop`, the shape, the raw grade, the symbol box, and the glyph
box and weight -- the last so that a shape disagreement can be told apart from a
part-assembly disagreement.

`ClefProbe` drives each sheet to HEADERS **in-process** rather than parsing a
saved `.omr`, which is what lets it read live `StaffHeader` objects. Getting
there needs three things that are not obvious and cost most of the time:

- `new Book(inputPath)`, not `Book.createBook(path)` -- the latter treats the
  path as a *target* `.omr` and makes stub creation try to browse a PNG as a zip;
- a stub built directly (`new SheetStub(book, 1)`), since `book.createStubs()`
  reaches for `Main.getCli()`;
- a batch `CLI` installed into `Main`'s private static field by reflection,
  because `reachStep` consults it for `isSave()` and the output folder.

It runs from `app/` (fonts) and reads the corpus as `../data/examples`.

**One of the three risks turned out not to be gradeable here.** The claim above
that the corpus contains sheets with two staff sizes is wrong: on all 65 staves
`getSpecificInterline()` equals the sheet interline, so the
specific-versus-sheet interline split that `ClefBuilder` and
`MusicFont.getPointSize` disagree about is *never exercised*. A Rust port that
used the wrong one of the two would pass this oracle. That needs either a sheet
with small staves added to the corpus or a targeted synthetic case; until then
it is an untested divergence, not a covered one.

The two remaining risks the oracle does cover:

1. **The glyph the classifier is handed.** Java classifies a `Glyph` assembled
   from header parts, and the descriptor reads its `RunTable` with the glyph's
   own origin. If part assembly differs at all, every feature differs -- which
   is why the glyph box and weight are in the oracle.
2. **`ClefInter.kindOf`,** which maps shape plus glyph centre to a `ClefKind`,
   and which `clef_column` reimplements as `clef_kind` + `target_pitch`.

**What is still missing is the comparison, not the oracle.** Nothing reads this
file yet. Do not report clefs as ported until a Rust test reads
`clef-headers.txt` and matches all 65 staves.

#### Where that work actually stands, and the one thing blocking it

More of it was already built than expected. `NativeClefProposalRecognizer`
already takes `sources: BTreeMap<usize, RunTable>` -- per-staff NO_STAFF crops
-- plus contexts and parameters, and is generic over `ClefShapeClassifier`, so
`BundledClefClassifier` drops straight in. `build_clef_lookup_contexts` already
ports `getOuterRect`/`getInnerRect`; `glyph_factory.rs` already ports
`GlyphFactory.buildGlyphs`; `near_graph` and `connected_sets` already port
`Glyphs.buildLinks` and the connectivity pass. So there is no missing algorithm,
only a missing driver.

`clef_parameters.rs` is the first piece of it and is done: `ClefBuilder.Parameters`
with its two-interline split intact, `Scale.Fraction` as `rint(interline * v)`
and `Scale.AreaFraction` as `rint(interline^2 * v)`. Interline 21 -- the corpus'
most common -- lands on two rounding ties at once (94.5 and 10.5), and both go
to even, so a port using `round()` fails rather than half-passes.

**Staff-line geometry: closed.** `GridLinesRecognition` now carries
`staff_lines: Vec<StaffLineGeometry>`, each with the first and last line splines
plus `first_line_y_at(x)` / `last_line_y_at(x)` (Java `LineInfo.yAt(int)`,
`rint`ed). The splines were being computed inside GRID and dropped; nothing new
is derived. Outside a spline's abscissa range these return `None` rather than
extrapolating along the global slope as Java does -- deliberate, so a caller
that strays outside a staff names itself instead of receiving an invented
ordinate. Nothing in HEADERS should stray: its abscissae are the middle of a
staff's own browse range.

**A bug surfaced by doing that, now fixed.** `build_clef_lookup_contexts`
evaluated each neighbour's gutter from *one scalar ordinate per staff*, but
Java's `getOuterRect` reads a neighbour's line at the **current** staff's
`xMid`. Those coincide only when staves are parallel and aligned. There is now
`build_clef_lookup_contexts_at`, taking a `StaffLineOrdinates` resolver, with
the old signature kept as the flat approximation the headless tests use.

The regression test matters more than it looks: it is built so the sloped
neighbour's gutter binds *only once sloped* -- flat, it lands below the
`aboveStaff` limit and is invisible. That is exactly how this divergence would
have hidden in production, and the first version of the test missed it for that
reason.

**The driver and the comparison now exist**, in
`crates/audiveris-omr/tests/clef_headers_corpus.rs`: it runs GRID on each of the
nine pages, builds the lookup contexts from the published splines, assembles
`NativeClefProposalRecognizer` over `BundledClefClassifier`, and compares. **All
65 staves match Java on shape and on the symbol box.**

It supplies Java's header start rather than computing it, and grades only what
the clef stage does with it -- the same isolation the spot-dispatch test used.
`compute_header_starts` needs its own oracle; grading it from this one would be
circular, since it is an input here.

### The missing centroid correction, which the corpus found immediately

The first run failed on **53 of 65 staves with every shape, width, height and
ordinate correct and only the abscissa out, by 1 to 3 pixels.** That is about as
precise a diagnosis as a failure can hand you, and it pointed straight at the
one step `clef_classifier` had left out: `registerClefs` slides the box by
`dx = glyphCentroid.x - symbolCentroid.x` *after* `getSymbolBounds` has centred
it. Two different centres are involved -- the glyph's **area** centre positions
the box, then the glyph's **mass** centroid corrects it -- and Java's own
comment explains why: unerased staff-line chunks shift the ink sideways.

Note what this says about the unit tests. `clef_classifier` had tests for the
noise gate, the rank-then-filter order, the drum shape set and the two
independent roundings, and all of them passed against code missing an entire
step. The corpus caught it on the first run.

### `clefStop`: closed, and the first explanation was wrong

Computing it as `glyph.getBounds().intersection(clefBox)` reproduced 56 of 65,
with all nine misses on bass staves. The note here previously blamed that on
`registerClefs` setting `clefStop` from the candidate at index 0 while
`selectClef` later picks a different one by contextual grade. **That was wrong.**
Extending `ClefProbe` to emit every registered candidate showed exactly one per
staff, with contextual grade equal to intrinsic -- so there was never a
competing candidate to disagree about.

The real cause is that `Staff.getClefStop()` does not return what
`setClefStop` stored:

```java
public Integer getClefStop () {
    if (header.clef != null) {
        Rectangle bounds = header.clef.getBounds();
        return (bounds.x + bounds.width) - 1;      // the glyph is not consulted
    }
    if ((header.clefRange != null) && header.clefRange.valid) {
        return header.clefRange.getStop();          // the stored value, as fallback
    }
    return null;
}
```

`registerClefs` does compute an intersection and store it on the clef *range*,
but once a header clef exists that stored value is never read; the getter
recomputes from the clef's own bounds and ignores the glyph. The stored form
survives only for a staff whose clef was never selected.

This is a quiet difference rather than a loud one: the two agree whenever the
glyph is at least as wide as its symbol, which held on 56 of 65 staves. The nine
that disagreed were all bass clefs, whose ink is narrower than the `F_CLEF`
symbol. `clefStop` is now asserted on all 65.

Two lessons worth carrying. A getter that recomputes rather than returning the
stored field is exactly the kind of thing a port reads past; when a value has
both a setter and a getter, read the getter. And a hypothesis that explains the
*pattern* of failures -- "all nine are bass clefs" -- can still be the wrong
mechanism, so it is worth the ten minutes to test it before writing it down as
fact.

## The two back-half risks, scouted (2026-08-07): neither blocks the project

Both were investigated by dedicated scouts with file:line evidence; the full
reports are `rust/scouting/heads-rasteriser.md` and `rust/scouting/texts-ocr.md`.
Summary of what matters for planning:

### HEADS templates: pinnable data, no rasteriser port needed

Rendering enters template construction at exactly one point and is **binarised
immediately** (alpha >= 140); graded coverage never survives, and the runtime
match reads each keypoint only as a 3-way fore/back/hole class with integer
weights. Templates depend on **(family, shape, integer pointSize) alone** --
the pointSize is sheet-derived (measured black-head widths, secant interpolation
over the already-ported `TextLayout.getBounds`) but collapses to one integer per
staff. Whole-corpus template set: at most ~216 entries, under 1 MB -- dump them
from a Java probe as oracle data, the classifier-weights move again. The
sheet-side chamfer matching is pure integer arithmetic.

**Carried risk, do not lose it:** `PageCleaner` (the SYMBOLS/TEXTS eraser base)
paints font glyphs at *fractional* positions into the erase buffer. That is the
one genuine rasteriser dependency left, it is downstream of HEADS, and it needs
its own scout before SYMBOLS is attempted.

### TEXTS OCR: fixture strategy confirmed by live measurement

Java binds Tesseract **5.5.2 in-process** (bytedeco), legacy engine
`OEM_TESSERACT_ONLY`, PSM_AUTO for the sheet scan; input is the NO_STAFF-derived
buffer with good inters erased, round-tripped through in-memory TIFF at
resolution 70. The narrowest clean seam is `OCR.recognize -> List<TextLine>`,
and the **complete** call-site set is: the TEXTS sheet scan, CURVES'
`RehearsalsBuilder` (a second batch stage the fixture must cover), and one
GUI-only path. Rust already has the matching `ExternalTexts` seam; its
`NeutralOcrWord` must grow baseline/font/char fields.

Two live facts that settle the strategy. **Determinism:** the same sheet OCR'd
in two JVM sessions produced bit-identical raw TextWords. **Feasibility:** with
a legacy-capable `eng.traineddata` (23.5 MB, from `tesseract-ocr/tessdata`; not
bundled with Audiveris, and Homebrew's is LSTM-only and useless to the legacy
engine), all corpus sheets ran headless to end of TEXTS: ~134 sentences, ~252
words, 262 lyric items.

Plan as recommended: a recorder probe keyed by `(image-pixel SHA, langSpec,
PSM) -> raw TextLines`, a Rust `FixtureOcr` that fails loudly on a key miss --
which converts "fixture valid only if upstream stages are bit-identical" from an
assumption into a checked invariant -- then the actual port is `TextBuilder`.
Linking Tesseract from Rust would mean fighting a differently-compiled binary's
float behaviour for zero port value, and the fixture seam is where a real
binding would plug in later anyway.

## KEY and TIME: measured before starting (2026-08-07)

### How much of the corpus exercises them

Worth knowing before planning: a stage with two examples on nine pages needs a
different approach from one with sixty. `ClefProbe` now emits `key` and `time`
rows per staff, absence included.

```
key:  34 of 65 staves     fifths -3 (x12), 2 (x10), -2 (x6), -1 (x6)
time: 17 of 65 staves     COMMON_TIME (x7), TIME_TWO_FOUR (x7),
                          TIME_THREE_FOUR (x2), one with a null Shape
```

Both are well exercised. The null-shape time is not a defect: `TimePairInter`
and `TimeCustomInter` carry no single `Shape` and are described only by their
rational, so the oracle emits `getTimeRational()` alongside the shape.

### The font layer is already done for KEY, and half done for TIME

The scout now sweeps eleven shapes rather than six -- the clefs plus `FLAT`,
`NATURAL`, `SHARP`, `COMMON_TIME` and `CUT_TIME` -- and every one of them
behaves exactly as the clefs did:

- all eleven centroid offsets are size-independent, checked at seven interlines
  and emitted only on agreement, so they are pinned as constants;
- **1276 of 1276** swept outline boxes match, the same `FT_DivFix`/`FT_MulFix`
  fixed-point law, no exceptions;
- the `UngradedOutline` guard never fired, so every one of these boxes is set by
  an on-curve point and the curve-interior ordering question stays theoretical.

So `KeyBuilder` needs nothing further from the font.

### TIME: composite layout and classifier in place (updated 2026-08-08)

The num/den stacking is ported: `num_den_dimension` measures both digit layouts
with the graded `layout_bounds`, separates their centres by
`2 * getStaffInterline(font)`, and `rint`s the raw composite rectangle once at
the end, as `ShapeSymbol.getDimension` does. Two quirks are load-bearing and
pinned by tests:

- `getStaffInterline` is `rint((pointSize + 2) / 4.0)` -- the `+ 2` puts every
  standard size on a rounding **tie**, so interline 21 answers 22 while 20
  answers 20, and the num/den gap inherits the parity. The expected value was
  written wrong on first try again; the tie table is not intuition-safe.
- The composite box is centred with **integer-division** halves (`dim/2`), not
  `rint(dim/2.0)` -- Java builds a `Dimension` first, so an odd height loses
  its half downward.

The result cross-checks against reality before any driver exists:
`num_den_dimension(2, 4, il 21)` is exactly the `(36, 87)` box Java's HEADERS
stores on every `TIME_TWO_FOUR` staff of the corpus.

The sweep now grades **14 shapes x 116 sizes** (the three corpus digits joined
it, centroid offsets pinned). `time_classifier.rs` fills the
`HeaderTimeShapeClassifier` seam: noise gate, rank-then-filter over the full
label set, `WholeTimes`/`PartialTimes` mappings, and the two symbol-bounds
constructions -- `AbstractInter`'s rint-halved font box for COMMON/CUT, the
int-halved composite dimension for num/den shapes. The time classifier reads
the **staff-specific** interline, as clefs do; keys read the sheet's -- all
three choices recorded at their seams.

Multi-digit numbers (`TIME_TWELVE`, `TIME_SIXTEEN`, `TIME_TWELVE_EIGHT`) error
loudly (`FontError::UnsupportedNumber`) rather than guess: their boxes need
glyph-advance composition that nothing grades yet, and a silent skip would
consume rank slots differently from Java. None appear on the corpus.

**TIME is closed: 65/65** -- presence/absence on all staves, the agreed value
(specific shape, numerator, denominator), the symbol box, and `timeStop`
(recomputed from bounds, the predicted getter shadow confirmed).

The driver taught three things worth keeping:

1. **The header runs per real system, not per page.** TIME demands every staff
   of a *system* agree on a value; modelling a page as one system let staves
   without a time veto the ones with -- the first run found no time anywhere.
   The test now iterates GRID's `peak_graph.systems` and scopes columns, clef
   and key stop propagation, and grading to each system, which is also what
   Java's `HeaderBuilder` does. Keys stayed 65/65 through the restructure.
2. **Java's browse windows are barline-limited.** `getRoi` caps its stop with
   `Staff.getBrowseStop`, the first *good, connected* barline. Without that cut
   the ROI runs past the header into the first measure, and the classifier
   happily called batuque's opening notes a 3/4 at grade 0.23 on both staves of
   system 2 -- consistent, so the column accepted it. The oracle now emits
   `bars` rows (good+connected barline abscissae per staff) and the driver
   applies the cut; on batuque staff 3 the window shrinks to a 22 px sliver
   with no viable start, exactly Java's outcome.
3. **`selectClefs` runs after TIME**, so keys browse from the *stored* clef
   range stop, not the recomputed getter value. The test now uses Java's true
   order; keys still grade 65/65.

The `pair_ids` seam (numerator x denominator pairing needs pre-allocated inter
ids) is satisfied by a deterministic discovery pass that replays the exact
classification sequence and harvests the ids -- documented in the test.

`tests/header_corpus.rs` (renamed from `key_headers_corpus.rs`) now grades the
complete header chain and is enforced by CI.

### `header.stop` itself: 65/65, and a fourth getter shadow

The final header stop -- what `Staff.getHeaderStop()` serves to
`SpotsBuilder.eraseHeaderAreas` and `StemScaler.getBuffer` -- is now graded on
every staff, closing the value BEAMS has waited on since the first session.

The first run had **exactly the seventeen time-bearing staves off by exactly
+1**, which is as clean as a diagnosis gets. `HeaderTimeColumn.retrieveTime`
computes its system offset from `Staff.getTimeStop()` -- the *getter*, which
answers the inclusive right edge of `header.time`'s bounds -- while
`setTimeStop` had stored the exclusive `x + width` a moment earlier. Fourth
instance of the store/getter shadow (`getClefStop`, `getKeyStop`,
`getTimeStop`-for-reading, now `getTimeStop`-for-the-offset). The rule stands:
**when Java exposes a field through a getter, the port must call the getter
everywhere Java does.** `StaffHeader::time_stop()` now exists and the column's
return uses it; two unit fixtures that had pinned the exclusive convention were
corrected by the corpus.

**Retired: the beams header-erase caveat.** `header_corpus.rs` now computes the
erase rectangle Java's `SpotsBuilder.eraseHeaderAreas` uses -- system area left,
the first headered staff's `header.stop`, the system's first/last staff lines at
that abscissa -- **entirely from native values**, and grades it against the
`erase` rows of `beam-spots.txt`: all 30 systems across the eight beam sheets
match exactly, with a count assertion so the comparison cannot silently cover
nothing. The `header_erase_cost_is_measured_not_assumed` measurement (5 spurious
clef-sized candidates on chula without the erase) remains as documentation of
what the erase is worth, but the caveat it guarded -- "beams cannot run natively
because the erase needs HEADERS" -- is gone: the spot chain's `HeaderErase`
inputs are now producible without a Java oracle.

### Native BEAMS end to end: closed

`recognize_native_beams` composes the previously isolated pieces from the
native GRID report plus that native `HeaderErase` list: uncleaned-NO_STAFF
`maxStem`, the complete spot chain, system-area/bounds dispatch, per-spot beam
recognition, beam extension, hooks, and per-system grouping. The oracle is used
only after the result exists.

`native_grid_headers_and_beams_match_java_on_every_beam_sheet` grades all eight
beam sheets against both `beam-structures.txt` and `beams-sig.txt`:

- 2739 native spot components and all 30 native header erases reach BEAMS;
- 787/787 raw beams match by system, median, height, grade, and all six impacts;
- final beams and hooks match by system, integer bounds, grade, and all six
  impacts, and every per-system group count matches;
- the sole final-SIG difference remains the already-explained
  BachInvention5 system-6 source beam `(1183,2377,104,11)`, which Java's
  subsequent `MultipleRestsBuilder` replaces with a `MultipleRestInter`.

The older beam test used `BTreeSet` keys and therefore reported 190 versus 189
BeamInter impact vectors on Bach; three duplicates on each side were silently
collapsed. The new gate is a multiset: the honest counts are 193 native
pre-replacement versus 192 in Java's final SIG, differing by exactly that one
source beam.

One correction to the preceding handoff: it said 29 erase systems. The oracle
has 30 rows (3+3+3+5+3+5+2+6), and both the header and end-to-end tests assert
30. LEDGERS now reaches its native builder; its sheet-wide post-analysis is the
next recognition tail. CLI/JSON beam publication can proceed independently.


### KEY: the classifier seam is filled

`key_classifier.rs` implements `KeyShapeClassifier` -- the noise gate, the 110
features, the bundled MLP and Java's ranking, filtered to the two alteration
shapes. It is **simpler than its clef counterpart**: `KeyShapeEvaluation` carries
no bounds, so this seam touches no font at all; `KeyBuilder` places alterations
from slice geometry it already has.

Two details worth not copying wrong:

- `KeyExtractor` hands the classifier **`sheet.getInterline()`**, where
  `ClefBuilder` hands it `staff.getSpecificInterline()`. Both feed the same
  descriptor, so on a mixed-size sheet the two stages normalize the *same glyph*
  differently. That is Java's behaviour, not a tidy-up opportunity.
- `NATURAL` is not an alteration here. A key is built from sharps or flats;
  naturals appear only as a *cancel*, on `KeyBuilder`'s own path. Mapping it
  would let a cancel be counted as a key member.

### A latent divergence the clefStop finding predicted

`getClefStop()` recomputing rather than returning the stored value is not a
curiosity confined to the clef stage: `KeyColumn` uses it to pick the key's
browse start, `browseStart = clefStop + 1`. Rust's `retrieve_keys` was reading
`clef_range.precise_stop()` -- the *stored* value -- so on the nine bass-clef
staves where the two forms differ, the key stage would have begun browsing one
or two pixels off. Nothing had run far enough to notice.

`StaffHeader::clef_stop()` now ports the getter, and `retrieve_keys` uses it.

Fixing it exposed a second, smaller divergence. Java's `getClefStop()` reads the
stored stop **only when `clefRange.valid`**, and `setClefStop` sets stop and
valid together; the old Rust path ignored `valid` entirely. A `key_column`
fixture had been constructing a range with a stop but no valid flag -- a state
the pipeline cannot reach -- and passing because of that leniency. The fixture
now mirrors `setClefStop`.

Worth drawing the general lesson, since this is the third instance: when Java
exposes a field through a getter that does anything other than return it, the
port must call the getter everywhere Java does, not just where the difference
was first noticed.

### Two divergences fixed before the driver, both found by reading Java

Building the driver means filling `NativeKeyStaffContext`, and two of its fields
turn out not to mean what the Rust code assumes.

**1. `interline` is doing the work of two different Java values.** It is read in
two places: at the classifier call, where Java uses `sheet.getInterline()`, and
in the pitch computation, where Java uses the staff's own geometry. One field
cannot be both on a mixed-size sheet. It should be split into a
`classifier_interline` and whatever the pitch needs -- see below -- rather than
having the driver pick one and be wrong somewhere.

**2. The measured pitch is not an interline formula in Java.** The native code
computes

```
measured_pitch = 2 * (centroid_y - staff_mid_y) / interline
```

whereas `Staff.pitchPositionOf` computes, for a point inside the staff,

```
((lines - 1) * (2y - bottom - top)) / (bottom - top)
```

with `top` and `bottom` being `getFirstLine().yAt(x)` and `getLastLine().yAt(x)`
-- the *measured* line ordinates at that abscissa. The two agree exactly when
`bottom - top == 4 * interline`, and a real staff's lines are never separated by
exactly four nominal interlines at every x. So the native formula is an
approximation of Java's, and the error grows with how far the staff departs from
nominal -- precisely the sloped and warped staves where a key alteration's pitch
is most likely to sit near a boundary.

This is the same shape as the clef gutter bug: a scalar standing in for a value
Java evaluates from the splines at a given abscissa. The fix is the same too --
`StaffLineGeometry` is already published and already carries `first_line_y_at`
and `last_line_y_at`, so the key context should take the ordinates rather than
an interline.

Neither is visible on this corpus, since every staff has the sheet interline and
the pitch differences are sub-boundary. That is an argument for fixing them now
rather than after a green run makes them look settled.

**Both are now fixed.** `NativeKeyStaffContext.interline` became
`classifier_interline`, named for its one remaining use, plus a `line_count`.
The pitch comes from a new `StaffPitchGeometry` trait, threaded through
`NativeKeyProposalRecognizer`, which answers `(first line y, last line y)` at an
abscissa as *doubles* -- Java reads the spline with `yAt(double)`, not the
`rint`ed `yAt(int)` that `getOuterRect` uses, so the clef-side
`StaffLineOrdinates` would have been the wrong trait to reuse.

`pitch_position_of` falls back to the old interline form only when the splines
cannot answer at that abscissa, which for a key alteration means the glyph sat
outside its own staff's horizontal extent -- degenerate rather than routine. The
regression test uses lines 79.2 px apart where four nominal interlines would be
80, a 1% departure that is unremarkable on a scan, and asserts the two formulas
disagree there and agree at exactly 80.

### The driver's remaining inputs are now ported

`key_parameters.rs` also carries the three values the driver needs beyond the
extractor set: `max_header_width` (the `projectionWidth` Java passes to
`retrieveKeys`, sheet-scaled 15.0), `max_slice_distance` (0.5), and
`browse_envelope`.

**`browse_envelope` reproduces a bug in Java on purpose.**
`KeyBuilder.getBrowseRect` loops `x` from `xMin` to `xMax` and then evaluates
`staff.getFirstLine().yAt(xMin)` *inside* the loop -- `xMin`, not `x`. The sweep
does nothing; the envelope is decided entirely by the ordinates at `xMin`. The
Rust signature takes those two ordinates rather than a range, so it states the
fact instead of hiding a dead loop. If Audiveris ever fixes it the envelope
widens on sloped staves, and this has to change with it.

Also worth a note for whoever ports the next stage: **interline 21 is the
corpus' most common value and it lands on a rounding tie for three separate
constants** -- `maxClefEnd` (94.5), `yCoreMargin` (10.5) and `maxSliceDist`
(10.5). `Math.rint` sends all three to even. A port using `round()` fails all
three, and I got the expected value wrong on two of them while writing the tests.

### The projection-peak port, which closed every catalogued residual at once

The decisive structural fact, read out of `KeyBuilder.process` rather than
guessed: **Java does not classify its way to a key signature, it counts its way
there.** The staff-free projection is walked for stem-like peaks, the signature
(count and shape family) is inferred from peak count and *spacing* -- sharps two
stems per item, flats one, spacing thresholds deciding which -- and only then is
one slice allocated per expected alteration. Classification happens inside that
structure: candidates from subset enumeration are assigned **best-per-slice** by
rounded centroid, and slices still empty get a second extraction pass at a lower
grade floor with the neighbouring slices' chosen glyphs *erased from the crop*
(`KeyRoi.getSlicePixels`, `cropNeighbors`).

`key_peaks.rs` carries the pipeline as pure functions over `IntegerFunction` --
browseArea, checkSpace, createPeak (with `isStemLike` injected, since it alone
touches the raster), mergePeaks, purgeLightPeaks, inferSignature,
checkPeakDeltas, refineSignature, refineShapeStop, computeStarts,
allocateSlices -- reusing the HiLo finder audiveris-core already had from SCALE.
`classify_key_shapes` was rewritten onto it, with Java's two grade floors
(`keyAlterMinGrade1`/`2` over intrinsic: 0.125 and 0.0125), the `purgeParts`
quirks (`bounds.x == xMax` drop, cap 8 by descending weight), the
`embracesSlicePeaks` gate (half-open on the right, and peak centres are
half-integers), and the trailing-space check for single-item candidates.

Grade history: **34/34 key-bearing failing -> 29 -> 28 -> 20 -> 3 of 65.**
The fixture lesson repeated twice while porting: bare synthetic stems fail
`refineSignature`'s flat-trail requirement exactly as Java would fail them --
the unit fixtures now draw bowls -- and `mergePeaks` joins only truly adjacent
peaks (`min - prevMax <= 1` is zero blank columns).

### Closed: the last three staves, and the bug that hid in an `int`

The first two were Java's **third** extraction pass, `fillMissingAlters`, now
ported inside `check_with_clefs_and_fill`: once the best *compatible* clef is
chosen (a single alteration whose pitch misses its expected position by more
than the delta budget invalidates the whole clef, grades read intrinsic-scaled),
every slice still empty -- or whose pitched grade fell under `keyAlterMinGrade1`
-- is hunted once more in a **pitch window**: the slice rectangle re-centred on
the alteration's theoretical ordinate, `stdGlyphHeight` tall, phase-2 grade
floor, neighbours cropped. Clef supports reach the recognizer via
`with_clef_supports`; with none supplied the pass is skipped, as Java skips it
for a staff with no competing clef.

The last staff was the best find of the stage. The port computed the window
pitch as `expected - areaPitchOffset(FLAT)` = 3 - 1.0559 = 1.944, faithfully to
the formula's *intent*. Java's `KeySlice.setPitchRect` writes it as

```java
int pitch = clefPitches[getId() - 1];
pitch -= AbstractPitchedInter.getAreaPitchOffset(keyShape);
```

-- a compound assignment on an `int`, which Java **silently narrows**: the
result is `(int) 1.944 = 1`, truncated toward zero. The hunt window therefore
sits a full fractional-offset higher than the arithmetic suggests, 7 px at
interline 17. Reproducing the truncation closed the staff; "fixing" it would
diverge on every flat key. This was pinned by measurement, not inspection: the
per-alter boxes added to `ClefProbe` for the purpose showed Java's third alter
at (152,2352,15,42) against the port's (152,2359,15,42) -- same window height,
7 px placement difference -- and Java's `getAreaPitchOffset(FLAT)` probe value
(1.0559375) matched the Rust font derivation **bit-exactly**, eliminating every
suspect but the pitch itself.

The corpus test now asserts presence/absence, fifths, the union box and
`keyStop` on all 65 staves, and is no longer `#[ignore]`d -- it is CI's problem
to keep it green from here.

### Superseded record of the earlier findings

### The driver, and what it found before the pipeline landed

`tests/key_headers_corpus.rs` assembles the whole chain -- GRID, the clef stage,
then the key stage -- and **chains** rather than isolating: `browseStart` comes
from the clef stage's own `clefStop`, as `KeyColumn` does, so the join is under
test too. Only the header start is still supplied from the oracle.

**First finding, now fixed: subset enumeration.** `group_key_parts` merged every
part within `maxPartGap` into one compound. Java's `GlyphCluster.decompose()`
enumerates *subsets* of each connected set. At interline 21 the gap is 31.5 px
and key sharps sit about 20 px apart, so a whole signature collapsed into one
glyph whose width exceeded `maxGlyphWidth` (42 px) and was rejected -- **no key
was found anywhere on the corpus.** `enumerate_key_subsets` now mirrors the clef
side's walk: connected sets in left-abscissa order, seeds by descending weight,
depth-first growth. Order matters, because downstream keeps only the first
`maximum_alters` results.

Two deliberate differences from the clef walk. It prunes on width *and* height,
because the key adapter's `isTooLarge` tests both while the clef adapter tests
only height. And `maximum_component_gap` is now `f64`: it feeds a chamfer
distance that Java compares in double.

That took the grade from **34 of 34 key-bearing staves failing** to **29 of 65
disagreeing**.

**Second finding, fixed: the flat pitch.** `AlterInter.computePitch` treats
flats unlike sharps, and the native code used one formula for both. A sharp's
pitch is simply its mass centroid's. A flat's is the **average of two
heuristics**: the mass-centroid pitch plus `flatMassPitchOffset` (0.65), and the
**area-centre** pitch plus `getAreaPitchOffset(FLAT)`. Java's own comment calls
both heuristic; they exist because a flat's bowl hangs below the line it belongs
to while a sharp straddles it.

`getAreaPitchOffset` is **font-derived**, not tabulated, and the ported font
metrics compute it directly: one pitch step is
`(five-line staff height - one-line height) / 8` measured at point size 200, and
the offset is `(-box.y - box.height / 2)` over that. `STAFF_LINE` (U+E010) and
`STAFF_FIVE_LINES` (U+E01A) are in the codepoint table for this reason -- they
are not musical symbols, they are the font's own ruler.

Note the two points are read differently on purpose: `glyph.getCentroid()`
returns a **rounded** `Point`, so the mass pitch is taken at integer
coordinates, while `getCenter2D()` is exact.

**Third finding, fixed: the candidate purge.** Enumerating subsets without
Java's `KeyExtractor.purgeCandidates` defeats itself. Java sorts candidates by
decreasing grade and drops every *later* one that shares a part, so the best
reading of a piece of ink wins outright. Without it, carmen staff 1 kept both
the correct flat at grade 0.97 **and** an overlapping subset of the same ink at
0.147, forming a two-flat signature whose second alteration sat at pitch 0.113
where -3 was expected -- and the whole key died. Enumerating subsets and purging
them are two halves of one mechanism; porting either alone is worse than porting
neither.

The count cap and the purge must also run in Java's order: purge first, then
truncate. Capping during collection keeps whichever overlapping subsets happened
to be enumerated first, which is exactly what the purge exists to decide.

**Progress: `34/34 key-bearing failing -> 29 -> 28 -> 20 of 65 disagreeing`**, so
45 staves now match Java outright.

**Two residuals, debugged. They are the same missing machinery.**

Instrumenting the extraction on both, and reading the component lists directly:

*BachInvention5 staff 1.* Java's key is `(271, 359, 46, 76)` -- three flats. The
port produces exactly **one candidate**, so the purge is not over-reaching after
all; the other two flats never become candidates:

```
PART 0 left=271 top=381 w=17 h=45 weight=351   -> accepted, grade 0.767
PART 1 left=286 top=359 w= 7 h=43 weight=201   -> GATE-REJECT: width 7 < minGlyphWidth 8.5
PART 2 left=295 top=380 w=23 h=55 weight=454   -> passes the gate, never classified as FLAT
```

The ink is *fragmented*: part 1 is a 7-pixel-wide splinter. The subset `{1,2}`
that would reunite it spans y 359..435, 76 px, over `maxGlyphHeight` (64.6), so
the enumeration prunes it -- and Java's `isTooLarge` would prune it too. So Java
is not finding these in the first pass either.

*carmen staff 1.* The port's box is `(358,451,21,51)` against Java's
`(359,451,20,51)` -- one pixel wider on the left, same right edge. The trace
shows this is **the connected component itself**, `PART 1 left=358 w=21`, not a
compounding artefact. So the difference is upstream of everything ported so far.

**Both point at `KeyBuilder`'s slice phase, which is not ported.** After the
first pass, Java builds `KeyRoi` slices from the candidates found and then calls
`extractAlter` again per slice, with two things the first pass does not have: a
*lower* grade floor (`Grades.keyAlterMinGrade2`) and `cropNeighbors = true`,
which removes pixels belonging to adjacent slices before rebuilding the glyph.
That is exactly the mechanism that would recover a fragmented flat on a poor
scan, and exactly the mechanism that would shave one pixel off a glyph's left
edge where it abuts its neighbour.

Note BachInvention5 is the corpus' only JPEG and its only 17-interline sheet --
the sheet where fragmentation is most likely, which is consistent.

So the next step is not another tweak to the enumeration or the purge; it is
`KeyRoi`, `KeySlice` and the second `extractAlter` pass. `NeutralKeySlice`
already exists to hang them on.

**The residuals as measured:**

1. *Seven boxes one pixel wide on the left* -- `x - 1`, `width + 1`, identical
   right edge, ordinate and height. The key itself is correct. A stray
   low-weight component joining the compound on its left is the obvious suspect:
   `minPartWeight` is only 4 px at interline 21.
2. *Eleven staves on BachInvention5 where the port finds one alteration and Java
   finds three.* Java reads 46x76 boxes; the port reads about 17x46 -- one flat,
   not a three-flat signature. This is the corpus' only 17-interline sheet. The
   likely mechanism is the purge over-reaching: a subset spanning two adjacent
   flats can outscore either flat alone, and keeping it removes both
   individuals. If so, Java is protected by something the port still lacks --
   most plausibly `KeyRoi`'s slice structure, which constrains where an
   alteration may begin, so a two-flat subset never competes as a single
   alteration at all.

**Superseded, for the record: one alteration per slice.** This is what instrumentation
was for, and it answered cleanly. On carmen staff 1 the classifier identifies
the flat correctly -- box (358,451,21,51) at grade 0.97, against Java's
(359,451,20,51) -- and the pitch check now passes it at 0.631 against an
expected 0. The key is *still* rejected, because a **second** alter is proposed:
an overlapping larger subset, (349,451,30,65), which the classifier also calls a
flat at grade 0.147. Together they form a 2-flat signature whose second
alteration sits at pitch 0.113 where the second flat is expected at -3, so the
whole candidate dies.

Java does not accumulate every accepted subset. `KeyRoi` divides the browse
range into slices and `keepCandidate` retains only the **best glyph per slice**,
so two overlapping subsets of the same ink compete and one wins. The native code
appends both. Porting that reduction is the next step, and `NeutralKeySlice`
already exists to hang it on.

The residue before that lands:

```
24  no key where Java found one   -- of which 23 are FLAT keys (-1, -2, -3)
 4  key found, box off by 1 px in x or width
 1  key found where Java found none
```

Flats are systematically missing and sharps are not: 23 of the 24 absences are
flat signatures, and the single sharp absence is one staff of one page. Size is
not the cause -- a one-flat key measures 20 x 51 against bounds of 10.5..42 wide
and up to 79.8 tall.

So the lead is flat-specific. First place to look is the pitch: the positions a
flat occupies differ from a sharp's, so `maximum_delta_pitch_one`/`_four` and
the pitch each candidate is measured against are the suspects. Second is
structural -- Java runs **two `ShapeBuilder` passes**, one per key shape, each
with its own ROI and slices, and the native code iterates `[Flat, Sharp]` inside
a single pass. Whether that is faithful is worth checking directly rather than
assumed.

Leave the four 1-pixel box differences until the flats are found; a changed
candidate set will move them anyway.

## Open threads, in the order worth taking them

### 1. Staff-line filament assembly and SIG grades (CLOSED)

Closed. Every median residual in the SIG is gone -- seven across the corpus,
including chula's -- and the grade residuals dropped from 21 to 6 with
`BachInvention5.jpg`'s worst falling from 0.18 to 0.004.

The cause was not where two earlier notes put it. `createInters` reproduces
Java's median formula exactly, and `StaffPeak`'s top and bottom are `final`, so
the residual had to be a staff *line* residual -- and it was: staff 11 carried
two single-section stubs where Java had full-width lines.

But the ink was present and correctly clustered the whole time. `StaffCandidate`
recorded only each line's *primary* filament id, and the projector resolved that
id against the filament factory map, which returns the filament as it was
**before** any cluster merge. When a cluster absorbs another, the resident line
keeps its primary id and gains the incoming sections, so a line seeded by a short
fragment resolved back to that fragment alone and the projector read a flat line.
Staff abscissae were unaffected because `left`/`right` were already computed from
the merged geometry, which is why nothing else caught it.

`StaffCandidate` now carries `line_filaments`, the cluster's merged line
filaments, and both consumers in `recognize.rs` use them instead of resolving
ids. `StaffCandidate`'s `PartialEq` is hand-written to skip the new field, which
is derived data rather than identity.

Closed completely. **Every barline inter now reproduces Java's intrinsic and
contextual grade on all nine example pages**, alongside the medians and the
core fields, so `SIG_PAGE_LEDGER` is zeros throughout.

The last six were one wrong rounding mode.

`StaffProjector.computeProjection` bounds each column by
`firstLine.yAt(x)` and `lastLine.yAt(x)`, and `StaffFilament.yAt(int x)` is
`(int) Math.rint(yAt((double) x))`. **`Math.rint` rounds a half to even; Rust's
`f64::round` rounds it away from zero.** The port used `round`. The two differ
only when the ordinate lands exactly on a half, and then by one row -- which
moves the projection's vertical bound by one and its accumulated pixel count by
up to one. One character of difference, `round` to `round_ties_even`.

That explains the signature that made this look structural. Six of 420
barlines differed, every one of them the leftmost or rightmost of its staff,
because that is where a staff line is extrapolated past its defining points --
and an extrapolated straight line lands on a half far more often than a fitted
spline through real ink does.

#### How it was found, and what it says about diagnosing by inspection

Two earlier rounds of source reading produced three hypotheses, and all three
were wrong: the chunk thresholds, `getChunk`'s out-of-image guard, and "the
staff-vertical impacts measure something differently at an extreme abscissa".
The residual was in none of them. It was in the *input* to the impacts.

What settled it in one run was measuring rather than reasoning.
`oracle/java/StaffImpactsProbe.java` prints the six impacts behind every
promoted barline, and diffing them against the Rust diagnostic showed that in
all six cases **exactly one integer differed, by exactly one**:

```
                       term          Rust        Java     as a fraction
BachInvention5 st 1    left chunk    0.913043    0.869565    21/23 vs 20/23
D0392410       st 8    stop deriv    0.864865    0.891892    32/37 vs 33/37
D0392410       st10    start deriv   0.837838    0.810811    31/37 vs 30/37
carmen         st 3    left chunk    0.923077    0.884615    24/26 vs 23/26
carmen         st10    left chunk    0.875000    0.916667    21/24 vs 22/24
cucaracha      st 6    right chunk   0.840000    0.880000    21/25 vs 22/25
```

Two different terms, both signs, always ±1 on an integer read from the
projection. That points at the projection itself rather than at either
consumer, which is what a hypothesis about the chunk lookup could never have
reached.

The probe needs no change to the production tree, twice over:
`AbstractInter.getImpacts()` keeps the `GradeImpacts` the peak was built with,
so the promoted SIG still carries them. It does have to drive the step engine
itself rather than use Audiveris's `-run` hook, because that hook fires after
the book is stored and its sheets disposed, and reloading `sheet#1.xml` gives
back inters whose impacts are `null` -- the XML persists only the product. The
probe sets `Main.cli` reflectively for the same reason `-run` exists: the step
engine reads it mid-step.

Run it as:

```sh
unset JAVA_TOOL_OPTIONS
JAVA_HOME=/path/to/jdk25/Contents/Home ./gradlew --no-daemon -q \
  -I rust/oracle/java/staff-impacts.init.gradle :app:staffImpactsProbe \
  -PimpactPages="data/examples/carmen.png data/examples/cucaracha.png"
```

`diagnose_sig_grade_residuals` in `recognize.rs` prints the Rust half.

### 2. PDF ingest (reading is done; rendering is what is left)

Audiveris renders PDF pages through PDFBox with
`renderImageWithDPI(page, 300, ImageType.GRAY)` under `ANTIALIAS_OFF` and
`INTERPOLATION_BICUBIC` (`ImageLoading.PdfboxLoader.getImage`). So this is
reproducing a rasterizer, not writing a set of decoders, and the sequencing was
to settle the rasterizer first, then the file format, then the composition.

**Two of the three are done.**

#### The oracle

`oracle/java/PdfPageProbe.java` -> `oracle/pdf-pages.txt`. It renders the corpus
through the exact call Audiveris makes and pins, per page:

- `image` -- each drawn XObject's declared geometry and filter chain, an
  FNV-1a-64 of its **raw bytes as they sit in the file**, an FNV-1a-64 of those
  bytes with the **filter chain applied**, and one of the decoded raster.
- `draw` -- the six-term `AffineTransform` Java2D receives, at 17 significant
  digits, read out of a `PageDrawer` subclass.
- `page` -- the boxes, the rotation, the rendered size, and an FNV-1a-64 of the
  rendered page.

Four depths, so each layer can be finished and graded before the layer above it
exists. That is what made the rest of this quick. Run it with
`-Dlogback.configurationFile=rust/oracle/java/logback-quiet.xml`, or PDFBox's
own diagnostics land in the data -- and read them, because each one names a
leniency the port has to reproduce.

The corpus is not in the repository: 20 MB of scans, listed with download URLs
in the `imslp-pseudo` repo's `manifests/acquired_scans.json`. Point
`AUDIVERIS_PDF_CORPUS` at a directory holding them and
`cargo test -p audiveris-pdf --test corpus` runs; without it, it prints that it
skipped rather than passing quietly.

#### The rasterizer: done

`transform.rs` reproduces Java2D's bicubic image transform bit for bit: 112 of
112 synthetic cases, pinned by `oracle/java2d-bicubic.txt`. Five things had to
be right, none guessable from "bicubic": the Mitchell-Netravali kernel with
`A = -0.5`; its 513-entry table whose tail above index 384 is *derived* so each
group of four sums to one rather than evaluated from the polynomial;
fixed-point arithmetic with coefficients scaled by 256, a `1 << 15` rounding
bias and a `>> 16` with saturation; 32.32 fixed-point coordinate stepping with a
half-pixel subtraction before both the gather and the interpolation; and
branchless sign-bit edge clamping that duplicates the border row or column.
Also: destination pixels whose centre maps outside the source are never written,
which is why a page render has a black margin rather than an extrapolated one.

#### Reading the file: done, 189 of 189

`document.rs`, `lexer.rs`, `object.rs`, `filter.rs`, `flate.rs`, `ccitt.rs`,
`jbig2/`. Against PDFBox on all 189 pages of the seven sampled sources:

| Layer | Result |
| --- | --- |
| Page count, media box, crop box, rotation | 189/189 exact |
| Image geometry, depth, filter chain | 189/189 exact |
| Raw stream bytes, by hash | 189/189 exact |
| Decoded stream bytes, by hash | **189/189 exact** (93 CCITT G4, 95 JBIG2, 1 Flate) |
| Image samples, by hash | **189/189 exact** (188 one-band gray, 1 three-band RGB) |
| Rendered page size in pixels | 189/189 exact |
| The transform Java2D receives, all six terms | **189/189 exact**, sign of zero included |
| The rendered page, by hash | **189/189 exact** |

Everything is ported from PDFBox's and jbig2-imageio's own source, fetched as
`-sources.jar` from Maven Central, rather than from the specifications. Same
reasoning as libjpeg 6b versus turbo: the target is the bytes Java produces.
The places that cost *output* rather than merely robustness, all commented at
their sites:

- **`/Length` is often a lie.** Three of the seven sources declare `/Length 0`
  on streams that are not empty. PDFBox logs "Suspicious stream length" and
  scans for `endstream`; the port validates the declared length by checking what
  follows it and falls back the same way. The raw-bytes hash is what pins this.
- **CCITT is TwelveMonkeys' decoder as PDFBox vendors it**, and three of its
  behaviours are in neither T.4 nor T.6. An unrecognised two-dimensional mode
  code *restarts the mode read* instead of failing. A run code that decodes to a
  negative value returns the full row width. A row that meets the end of the
  data is dropped whole, not truncated. Also `/Rows` is discarded when the image
  dictionary carries a `/Height`, and with `/K` at zero PDFBox *sniffs* the
  first twenty bytes for an end-of-line code to choose between T.4 and modified
  Huffman.
- **`FlateDecode` keeps the prefix of a corrupt stream.** Pinned against PDFBox
  at all eleven truncation points of a test stream, where the two agree exactly.
- **JBIG2's arithmetic decoder reads -1 past the end of its data**, not the
  0xFF the standard specifies, and folds it into a `long` code register. A
  damaged stream diverges from a standards-following decoder immediately.
- **JBIG2 output is the page bitmap inverted**, with the bits past each row's
  width cleared, because the raster's colour model has index zero as black.

**JBIG2 scope was set by measuring.** Dumping segment types across all 95 JBIG2
images found exactly three -- page information, one arithmetic symbol
dictionary, one immediate text region -- with flag words 0 and 16, and no
globals. So Huffman coding, refinement, halftones, striped pages and standalone
generic region segments are refused *by name* rather than half-written. The
generic region decoding procedure itself is complete (templates 0-3, adaptive
pixels, typical prediction), because every symbol bitmap goes through it. Do the
same measurement before extending it: `Document` can now extract the streams, so
a twenty-line probe answers "what does this actually use" in a minute.

#### Samples: done, 189 of 189

`raster.rs` is `SampledImageReader`: decoded bytes to samples, which is the rung
between the filter chain and the page. It was worth doing before anything above
it because the oracle *already recorded it* -- `PDImage.getImage()`'s raster,
hashed band-interleaved and row-major -- so it cost a grader rather than a new
oracle, and it splits sample conversion off from geometry. When the composed
page is first wrong, the half it is wrong in is already settled.

Scope was measured the way JBIG2's was. Across all 189 images there are exactly
four shapes: 177 one-bit `DeviceGray` with no `/Decode`, 11 the same with
`/Decode [1 0]`, and one 4-bit `Indexed` over `DeviceRGB`. No `/ImageMask`, no
colour-key `/Mask`. Anything else is refused by name through
`Error::UnsupportedImage`.

Three things in it are load-bearing and none is implied by "unpack the bits":

- **`from1Bit` returns one band, not three.** For `DeviceGray` PDFBox builds a
  `TYPE_BYTE_GRAY` image and returns it before any colour space runs, so the
  page later draws a gray source into a gray destination. That is what makes
  the `ScaledBlit(ByteGray, SrcNoEa, ByteGray)` trace in item 4 below legible.
- **A short row ends the image and leaves the rest black**, rather than
  truncating the raster: PDFBox logs "premature EOF, image will be incomplete"
  and breaks, keeping the rows it has.
- **The indexed palette is not a byte copy.** `initRgbColorTable` sends every
  entry through `byte / 255f` and back through `(int)(x * 255f)`, in `float`
  with a truncating cast. It is written the long way for that reason.

#### The content stream and the draw transform: done, 189 of 189

`content.rs` and `affine.rs`. Every one of the 189 draws now reproduces the
six-term transform Java2D receives, exactly, at the oracle's full 17
significant digits -- **and the sign of every zero**, which is checked
separately because `-0.0 == 0.0` would let a wrong answer pass.

The operator set was probed rather than assumed, and the probe paid for itself
twice. There are exactly four operators and exactly two page shapes:

```
  36 x  cm Do
 153 x  q cm Do Q
```

So 36 pages never push a graphics state at all: they concatenate straight onto
the initial CTM. Anything outside that set is refused by name, because
silently skipping an operator that moves the CTM would misplace the image and
read like a rasterizer bug.

Three float questions decide the answer, and the third was the one that had
already cost a debugging round:

- **The CTM is a `float` matrix.** PDFBox's `Matrix` holds `float[9]` and `cm`
  multiplies in `float`, so a `cm` operand of `633.5724` is really
  `633.57238769531250`. `content::Matrix` is `f32` for that reason and widens
  only where `createAffineTransform` does.
- **The DPI scale is a `float` division.** `renderImageWithDPI` passes
  `dpi / 72f`, so a 792 pt page renders **3299** pixels tall, not 3300. The
  `page` records' `render` size now grades this on every page.
- **`AffineTransform` is a state machine, and it is load-bearing.** Every
  mutator dispatches on cached bits describing which of translate, scale and
  shear are present, and the branches are not algebraically identical -- they
  drop terms known to be zero. Dropping `+ 0.0` is a no-op for every double
  except `-0.0`. The page transform reaches `concatenate`'s scale-only case,
  which computes `m10 = T10 * m11` with `T10` at `+0.0` and `m11` negative from
  the y flip, giving **`-0.0`** -- which is what the oracle records on all 189
  draws, and what a closed-form composition gets wrong. `affine.rs` ports the
  state machine rather than the algebra for exactly this reason, and its tests
  pin both the `-0.0` and the closed-form counter-case.

#### Composing the page: done, 189 of 189

`render.rs` runs the whole chain and **every** page reproduces Java's rendered
raster bit for bit. That closes PDF ingest: all four depths the oracle records
are now graded, and all four are exact on all 189 pages. The destination is the easy half: `ImageType.GRAY` is
`TYPE_BYTE_GRAY`, one band, and `renderImage` clears it to `Color.WHITE` first,
so an unwritten pixel is 255 and the margins stay white.

**Java2D's primitive selection was the open question, and the answer is not in
Java2D.** The note here previously pointed at `DrawImage`'s `transformState`
ladder. That ladder is a dead end: `DrawImage.renderImageScale`, the only route
to a `ScaledBlit`, opens with

```
// Currently only NEAREST_NEIGHBOR interpolation is implemented
// for ScaledBlit operations.
if (interpType != AffineTransformOp.TYPE_NEAREST_NEIGHBOR) return false;
```

so under a bicubic hint no transform whatsoever reaches a `ScaledBlit`. What
changes is the hint, and **PDFBox changes it**, per draw, in
`PageDrawer.drawImage`:

```
boolean isScaledUp =
    bim.getWidth()  <= abs(round(ctm.getScalingFactorX() * xformScalingFactorX)) ||
    bim.getHeight() <= abs(round(ctm.getScalingFactorY() * xformScalingFactorY));
if (isScaledUp) graphics.setRenderingHint(KEY_INTERPOLATION, NEAREST_NEIGHBOR);
```

and restores the hints straight after, which is exactly why the earlier probe
found the hint reading `Bicubic` both before and after. The port computes the
same predicate and it selects **exactly 10 of 189 draws**, independently
matching what `-Dsun.java2d.trace=count` counted. That agreement is the
evidence; the rule was derived from source and the count was measured before
the two were compared.

**A second PDFBox path has to stay switched off, and it is fragile.**
`drawBufferedImage` abandons `drawImage` entirely and pre-scales through
`Image.getScaledInstance(w, h, SCALE_SMOOTH)` -- a different resampler --
when a scale falls below `imageDownscalingOptimizationThreshold`, which
defaults to **0.5**. Corpus pages scale by about 0.93, so the threshold is not
what saves us; the branch also demands
`VALUE_RENDER_QUALITY.equals(getRenderingHint(KEY_RENDERING))`, and Audiveris's
hints carry only `ANTIALIASING` and `INTERPOLATION`. If Audiveris ever sets
`KEY_RENDERING`, or renders where a scale drops under 0.5 with that hint
present, the resampler changes and none of `transform.rs` describes the output.
`render::hints_reach_the_downscaling_workaround` states the condition so it can
be checked rather than remembered.

The ten scaled-up draws are done too. `scaledblit.rs` ports OpenJDK's
`ScaledBlit`, and all ten reproduce Java exactly. It is worth knowing why a
general nearest-neighbour resampler would not have: the loop steps the source
coordinate in fixed point and accumulates error linearly, so rather than widen
the arithmetic OpenJDK **re-derives the source origin exactly at the start of
every tile**, with `findpow2tilesize` choosing a power-of-two tile small enough
to bound the drift. The result is nearest-neighbour with a periodic exact
resynchronisation. Its rounding is `ceil(x - 0.5)`, a round-half-*down*, not
the `floor(x + 0.5)` that `Math.round` gives. The destination bounds are found
by `refine`, which searches rather than solves, because the forward and inverse
mappings are not exact inverses in floating point.

`render.rs` also ports `DrawImage`'s `tryCopyOrScale` ladder, which transforms
three source corners and decides from those rather than from the matrix. The
plain-`Blit` case and the sheared-nearest case are refused by name; no corpus
draw reaches either.

The corpus's one three-band page is done too, and its order is the part worth
remembering: `renderImageXform` transforms into an `IntArgbPre` intermediate and
only then blits that to `ByteGray`, so **each channel is interpolated in colour
and the reduction to gray happens after**, not before. The reduction is
OpenJDK's fixed-point luma from `ByteGray.h`,
`(77r + 150g + 29b + 128) / 256` -- not a colour-space conversion. That formula
is also why a gray source survives the same round trip untouched: at
`r == g == b == v` it is `(256v + 128) / 256`, which is `v` for every byte.

#### Wired into the load path

`audiveris-cli -batch -step GRID score.pdf` works. `ingest::Loader` is Java's
`ImageLoading.Loader`: an input is a **book of sheets**, not an image, sheet ids
are one-based, and only a PDF supplies more than one. `-sheets` selects a
subset; an empty selection is every sheet.

Two details are Java's rather than convenient:

- **The dispatch is on the file extension**, case-insensitively, not on magic
  bytes. `ImageLoading.getLoader` tests `.pdf` and sends everything else to
  ImageIO, so a PDF named `.png` fails there -- and sniffing the header here
  would make the port accept an input Audiveris rejects.
- **`-sheets` consumes every following non-`-` token and fails on one that is
  not a sheet spec.** That is `CLI.IntArrayOptionHandler`, which calls
  `NaturalSpec.decode` on each and lets it throw, so `-sheets 2 score.pdf` is an
  error in Java too. Put inputs before `-sheets`.

`crates/audiveris-image/tests/pdf_ingest.rs` pins the seam, and is deliberately
redundant with the PDF crate's own corpus test: for all 189 sheets, the raster
the load path hands binarization has the same FNV-1a-64 as the page PDFBox
rendered. The two crates prove different things -- one that the render is right,
the other that the ingest does not then change it -- and the gap between them is
where `Picture.adjustImageFormat`'s maximum-channel rule would have hidden a
conversion. It is the identity here only because `max(v, v, v)` is `v`.

A first real run: page 2 of `IMSLP00709-Schumann_.pdf` reaches GRID with 12
staves in 6 systems and 112 barlines, in about two seconds. Nothing grades that
yet -- see below.

#### Recognition on a PDF sheet is graded too

`oracle/grid-pdf.txt` closes the gap that "the pixels are right" left open.
Eleven corpus sheets run through GRID in live Java, and the port reproduces
**all of it**: staff geometry, and all 392 promoted barlines with their shape,
width, frozen flag, staff-end mark, median, intrinsic grade and contextual
grade. Grades are compared at **1e-9**, not the 5e-4 the example corpus uses,
because this oracle reads the live SIG rather than the three decimals
`sheet#N.xml` persists.

The sheets span the render regimes deliberately rather than by sampling: JBIG2
with and without shear, CCITT plain and with `/Decode [1 0]`, the one Indexed
three-band page, and a sheet from `IMSLP57453`, whose ten pages all take the
nearest-neighbour `ScaledBlit` instead of the bicubic transform.

Four of the eleven are sheets Java **refuses** -- covers and title pages, where
it raises `No regularly spaced lines found` rather than returning an empty
sheet. That is recorded rather than skipped, and asserted: the port has to fail
on the same sheets. Getting a refusal where Java recognises ten staves would be
just as wrong as the reverse.

Regenerate with `oracle/java/GridPdfProbe.java`, whose arguments are
`<path>:<sheet>` with the sheet counted from one.

#### What is left

1. **Widen the corpus.** Everything here is graded against seven IMSLP sources
   whose 189 pages contain exactly four image shapes and four content-stream
   operators. Every refusal is by name, so a new source fails loudly rather than
   silently, but the honest description of the current state is "exact on what
   was measured", not "complete".
3. **Regenerating the oracle needs a JDK**, and this machine has none. The
   checked-in `oracle/pdf-pages.txt` is enough to run the test, but if the
   probe ever changes, that has to happen where Java is.

Use `-Dsun.java2d.trace=count` first for anything of this kind. It answered in
one run what two rounds of source reading did not.

### 3. Progressive JPEG (deliberately deferred)

Audiveris accepts progressive JPEG; the port refuses it with a clear error. No
IMSLP source exercises it -- the corpus is bitonal PDFs -- and the corpus JPEG is
baseline. Revisit only when a real file hits it. Shape of the work is in the
`audiveris-jpeg` crate docs.

## Green checkpoints

Every commit below was independently formatted, tested, clippy-clean with warnings
denied, and passed `git diff --check` before commit.

1. `d5ef29dd` — Cargo workspace, AGPL/port contract, pipeline enum, natural specs,
   rational arithmetic, population statistics, arrangements, and CLI parser.
2. `7a8cd034` — frozen JSON Java baseline and executable `xtask` JUnit verifier.
3. `ef1d67bd` — histogram, contextual grades, and brute-force injection solver.
4. `9797e9bb` — horizontal/vertical least-squares `BasicLine` geometry.
5. `fc4c9197` — oriented binary run tables, RLE conversion, union, purge, trim,
   raster conversion, and query behavior.
6. `6ad10fba` — chamfer distance transforms and Audiveris median-gray filtering.
7. `941fc15a` — inclusive global thresholding, alpha-over-white compositing, and
   polygon-mask enumeration.
8. `a54a559e` — gray-level watershed segmentation with basin and watershed-line tests.
9. `9fd992f3` — live Java probe and exact canonical Rust comparison across 12 utility,
   geometry, assignment, run-table, and pipeline-order vectors.
10. `8f65b5a5` — exact cross-runtime threshold, median, chamfer, and run-extraction
    image vectors.
11. `354e1d8d` — SHA-256 oracle manifest for the classifier, fonts, and image fixtures.
12. `c0c39f9f` — PNG/JPEG raster loading with Audiveris max-channel grayscale semantics
    and an exact full-page Java/Rust PNG digest.
13. `2e7a95c2` — integral-image adaptive binarization with exact synthetic and full-page
    Java/Rust mask comparisons.
14. `428fb6d5` — exact vertical-run input parity and source-guided black/combo run
    histograms for the first `SCALE` boundary.
15. `a264e8b1` — takeover record refreshed through the exact SCALE input boundary.
16. `3804a957` — Java-compatible integer functions and range primitives.
17. `9775d53c` — live `IntegerFunction` differential vector.
18. `1abc585c` and `1efc7ead` — derivative hysteresis peak finder plus terminal-range
    behavior.
19. `0dc07283` and `92d6a1ec` — line/interline/beam SCALE decisions and locked crate
    dependency.
20. `87b6a4e3` — real production `ScaleBuilder` versus Rust full-page Chula parity,
    including exact peaks, histogram areas, and beam decisions.
21. `257d819e` — bounded opaque `.omr` ZIP inventory and content-equivalent round trip,
    preserving unknown members and rejecting unsafe or duplicate paths.
22. `79bbfc7d` — exact production Java/Rust gray-level watershed vector.
23. `a03c4d80` — lossless read-only `book.xml` metadata view with exact source bytes.
24. `21126e72` — four-page SCALE parity covering dual interlines, extrapolated beams,
    and low-quorum beam acceptance at the configured distance boundary.
25. `2ace02ba` — neutral GRID section construction with all four junction policies and
    an exact synthetic Java/Rust topology vector.
26. `e0809435` — lossless read-only per-sheet XML metadata view while retaining every
    original byte and leaving SIG content opaque.
27. `66ebf2ef` — exact full-page Chula GRID run-dispatch and horizontal/vertical lag
    section parity.
28. `504fed58` — dependency-free parity testkit with deterministic vectors,
    first-difference diagnostics, and bounded fixture-root resolution.
29. `3ac3f75e` — the live oracle harness now uses the parity testkit and rejects
    malformed or duplicate vector lines.
30. `61f94c4b` — source-guided natural line, quadratic, and cubic spline geometry.
31. `fe18009c` — neutral GRID staff-filament metrics and probe/spline geometry, plus
    exact live Java/Rust spline and filament vectors.
32. `cf68ee56` — archive-level typed `book.xml`/per-sheet access with explicit
    undeclared, missing, present, and malformed-member states.
33. `6a76eb9a` — scoped `FilamentFactory` core filtering and stable non-overlap
    grouping, plus an exact live Java/Rust merge/rejection vector.
34. `638b2989` — section pixel ROI moments and Java-compatible horizontal/vertical
    contact semantics needed by filament probes and expansion.
35. `113a7da3` — source-compatible `StaffPattern` scoring for idealized GRID lines.
36. `b5fb5227` — exact horizontal overlap sampling, thickness, consistency, space,
    slope, and expansion-contact compatibility for filament grouping.
37. `4affaca2` — lossless typed reading of persisted sheet-step completion lists,
    sharing the recognition pipeline's single `OmrStep` type.
38. `1fa21844` — bounded real-page Chula filament-factory digest with exact live
    Java/Rust parity.
39. `db964fb9` — position-indexed section tally used by later staff-line sticker
    retrieval, with explicit sorted/range validation.
40. `cb27da40` — live production-Java overlap vector proving one filament merge and
    one displaced-overlap rejection.
41. `3e256a16` — lossless typed sheet input path and image-rank provenance with an
    atomic fail-closed view and preserved book-level fallback state.
42. `2377ab99` — local section-fatness probes and the complete neutral horizontal
    factory lifecycle: initial merge, leftover expansion, and final merge.
43. `61cea1f2` — corrected the original synthetic Rust factory fixture to use the
    production Java scale-derived thresholds exposed by the new bounds prefilter.
44. `4fa4cac0` — source-guided staff-line sticker filtering with owned-member
    exclusion, stable full-position ordering, cumulative adjacent contact, and the
    Java strict connection threshold.
45. `e2a76e54` — lossless typed sheet version and invalidity attributes, preserving
    absent and explicitly persisted states with JAXB boolean spellings.
46. `2d8e2f9c` — live Java/Rust `StaffPattern` vector covering fractional interlines,
    ties-even placement, inclusive line thickness, empty foreground, and bounds.
47. `a18681c7` — direct page-reference metadata in persisted order, including page
    IDs, movement starts, measure-ID deltas, and fail-closed typed validation.
48. `cb2fc1d9` — neutral stable-ID `FilamentComb` state, ancestor lookup, append
    ordering, ordinates, and processed-state behavior without Java object cycles.
49. `d205596a` — early `LineCluster` membership, absorption lineage, bounds, mean
    true length, and Java-style vertical/horizontal point extrapolation.
50. `5a5c8b6a` — source-guided target-line mapping from ideal deskewed coordinates
    back to physical filament points, including orthogonal offsets.
51. `237680d0` — ordered cluster endpoints and Java-compatible indexed filament
    inclusion with overlap midpoint, probe thickness, and atomic rejection.
52. `2d58cc6e` — live Java/Rust line-cluster vector for ordered positions,
    absorption, bounds, mean true length, and both extrapolation branches.
53. `5beb9bb5` — optional direct page time-rational metadata with raw JAXB integer
    semantics and lossless opaque retention of nested page content.
54. `cdb0c4dc` — live Java/Rust target-line vector across a sloped filament,
    endpoint/midpoint mapping, orthogonal offsets, and extrapolation.
55. `c7dbcd18` — immutable, cycle-free target page/system/staff containers with
    stable IDs, append-order preservation, ownership, and geometry validation.
56. `ee562e3e` — direct page systems in persisted order with Java's derived
    one-based `SystemRef` identity; part/staff content remains opaque.
57. `6c0584e3` — live Java/Rust indexed line-cluster inclusion vector covering
    overlap midpoint, exact thickness acceptance, rejection atomicity, and endpoints.
58. `4351f852` — ordered direct part references with persisted name, logical ID,
    manual state, and Java's derived zero-based part index.
59. `85df1d76` — source-guided regular filament-comb discovery across interior
    sample columns with ties-even spacing and inclusive interline bounds.
60. `549ab8db` — neutral fixed-slot bar-column state, mean geometry, start/brace/full
    status, overwrite behavior, and explicit connection relations.
61. `7311c915` — Java-compatible weighted popular-comb-size selection, including
    the histogram's lower-bucket tie behavior.
62. `1d0ee9ed` — neutral bar alignment/connection impacts, identity, ordering, and
    exact connection-preferred contextual `bestOf` selection.
63. `be225960` — ordered current and deprecated staff-configuration persistence
    variants without normalizing raw JAXB integer and boolean states.
64. `1bd4bdc3` — live production-Java bar-column vector using real staff peaks,
    graph relations, overwrite/cache invalidation, and status transitions.
65. `b1849e37` — source-guided line-cluster merging and absorption across compatible
    clusters while preserving stable identities and lineage.
66. `50d22e4f` — source-guided line-cluster trimming with deterministic side removal
    and cluster geometry updates.
67. `7e87fe61` — lossless typed score page-link persistence, including movement and
    page identity metadata.
68. `ca02fe74` — source-guided median geometry for connected bar alignments.
69. `9888733a` — live Java/Rust comb-discovery vector covering sampled columns and
    regular staff candidates.
70. `34c82630` — neutral `StaffPeak` value semantics, ordering, geometry, and flags.
71. `e77fb6e0` — lossless typed logical-part persistence in score order.
72. `818c3e6e` — neutral stable-ID `PeakGraph` storage without Java object cycles.
73. `c4deea44` — lossless typed score-root metadata while retaining unknown XML.
74. `495b0ef2` — source-guided `PeakGraph` connection and adjacency queries.
75. `cef45219` — lossless typed sheet-selection persistence.
76. `2651fdd6` — neutral `PartGroup` value semantics and hierarchy metadata.
77. `ae387c1c` — source-guided purging of incompatible peak alignments.
78. `df3bb9c7` — deterministic incident-edge queries over the neutral `PeakGraph`.
79. `957dc146` — lossless typed legacy beam metadata from persisted archives.
80. `a8cf4ae6` — source-guided brace-alignment checks over peak-graph geometry.
81. `53341825` — lossless typed legacy OCR metadata from persisted archives.
82. `9bbe2b7f` — live Java/Rust line-cluster lifecycle vector spanning merge and trim.
83. `4d67b856` — dependency-light `ShortProjection` storage and indexed access.
84. `e46b9ad5` — source-guided StaffProjector derivative-threshold computation.
85. `132df1ed` — live Java/Rust short-projection vector.
86. `68734e9b` — lossless typed book interline parameters with inherited and explicit
    states kept distinct.
87. `c8b83bdf` — source-guided StaffProjector blank-column selection.
88. `9bc82cd7` — lossless typed book beam parameters.
89. `6ed30bad` — lossless typed book OCR parameters.
90. `2f08078a` — live Java/Rust StaffProjector derivative-threshold vector.
91. `69c7f5f8` — source-guided StaffProjector peak-side refinement.
92. `194346bc` — live Java/Rust StaffProjector blank-selection vector.
93. `9d1607f7` — lossless typed book lyrics switches, preserving absent, inherited,
    explicit-false, and explicit-true states.
94. `72a7f8d4` — source-guided StaffProjector peak-candidate construction.
95. `cdcdd4e1` — live Java/Rust StaffProjector peak-side refinement vector.
96. `89ffa5ef` — live Java/Rust StaffProjector peak-candidate construction vector.
97. `9ba3dedb` — source-guided StaffProjector core-pixel validation.
98. `4a02e713` — live Java/Rust StaffProjector core-pixel validation vector.
99. `5977ee01` — source-guided StaffProjector impact grading and neutral peak promotion.
100. `e2b9b1d4` — source-guided StaffProjector browse/find range orchestration with
     acceptance-controlled cursor advancement.
101. `195de90b` — source-guided StaffProjector brace discovery and neutral brace peak.
102. `2e2da81b` — regression for continued scanning after an over-wide rejected range.
103. `d7c982b6` — live Java/Rust StaffProjector range-scanning vector.
104. `ba7ce4b2` — BarsRetriever adjacent-peak grouping.
105. `4f74e3aa` — neutral filament/cluster ownership registry.
106. `9fafce02` — BarsRetriever left-peak purge decisions.
107. `283d39b7` — transactional recursive comb/cluster inclusion.
108. `65e95e2f` — live Java/Rust StaffProjector brace vector.
109. `73f72f19` — StaffProjector raster-column accumulation.
110. `aeb9544a` — BarsRetriever start and brace purge decisions.
111. `8d7fea8f` — live recursive cluster-coordination vector.
112. `9af7a885` — neutral StaffProjector composition through graded peaks and brace lookup.
113. `9bb044db` — stable cluster formation from comb seeds.
114. `f8998f0d` — StaffProjector lines-root correction decision.
115. `2966d9a1` — live composed StaffProjector vector.
116. `10aea1f7` — live lines-root correction vector.
117. `bc1ef467` — bar-filament section preselection.
118. `fff6c947` — StaffProjector result mutation and right-end decisions.
119. `98ae08ed` — line-cluster merge compatibility kernel.
120. `41ac300f` — BarsRetriever VLAG/HLAG section-width filtering.
121. `7ae6815b` — StaffProjector multi-rest serif scan.
122. `c476c8fb` — StaffProjector core thickness and line thresholds.
123. `26075897` — ordered repeated line-cluster merge orchestration.
124. `d3b72603` — BarsRetriever isolated/grouped thin/thick width partitioning.
125. `3ef67e68` — StaffProjector scale-derived parameter construction.
126. `3a15306d` — partial bar-column purge selection.
127. `a983c2b6` — barline group-relation decisions.
128. `fdf5e043` — extending bar-peak purge selection.
129. `db773e5f` — raster-to-neutral-peak StaffProjector process orchestration.
130. `4aa2e5fe` — live StaffProjector result-operation vector.
131. `84ef60f1` — same-size cluster pair pass and short-cluster discard behavior.
132. `bf5b9b5d` — initial start-bar-column candidate selection.
133. `cab56e0c` — ordered BarsRetriever/StaffProjector registry and graph-vertex intents.
134. `24e4f07c` — connected peak-chain aggregation into bar columns.
135. `6f98719d` — direction-neutral peak-graph connected components.
136. `552acf2a` — inconsistent cluster destruction and ownership cleanup.
137. `74760c3c` — graph-component conversion to stable scalar bar chains.
138. `21c4c880` — multi-staff unaligned-peak purge selection.
139. `84651a74` — composed peak-graph-to-bar-column construction.
140. `a463cc8f` — atomic start-column staff-line validation.
141. `363a5d9b` — true brace-group part decision.
142. `1ca7abe5` — standard typed errors for the BarsRetriever seam.
143. `8825ca43` — ordered two-sided cluster expansion with isolated filaments.
144. `6aeaf78c` — rustfmt normalization of cluster-pair fixtures.
145. `4a43e358` — live Java/Rust bar-column construction and start selection vector.
146. `37f88ecb` — ordered within-part connection-edge selection.
147. `9bd76cd9` — desired-size cluster destruction, acceptable length, and filament partition.
148. `57be85fa` — brace-aware part creation planning with Java overlap truncation.
149. `c1a2a947` — bracket, square, and brace group topology state machine.
150. `2d0329c9` — ordinate-ordered cluster trimming and ownership cleanup.
151. `cf6ecc40` — C-clef false-bar suppression with exact scan/index behavior.
152. `b85252db` — bracket-middle propagation across concrete peak connections.
153. `38e11f34` — transactional, stage-ordered neutral cluster retrieval pipeline.
154. `88038a1e` — bracket-end detection with injected extension and serif evidence.
155. `0f391920` — neutral vertical bar/bracket interpretation geometry and kinds.
156. `78b32c79` — neutral bar/bracket connector plans and good-grade extension gate.
157. `52559a4b` — stage-ordered cluster passes into typed staff candidates.
158. `1ee7133e` — exact bar-extension pixel and overflow arithmetic regression.
159. `34cbfd43` — bracket-serif lookup rectangle construction.
160. `414d8106` — Java-order bar-connection component freeze traversal.
161. `361656c3` — stable distance/weight selection of serif compounds.
162. `2a170c3f` — transactional neutral BarsRetriever stage coordinator.
163. `71823e49` — merged two-staff/eleven-line part classification.
164. `5411d5e7` — transactional headless LinesRetriever/BarsRetriever GRID join.
165. `af9cf6cf` — exact outer GridBuilder order and Java exception semantics.
166. `b57618fb` — source-preserving GRID run dispatch with ties-even thresholding.
167. `5acc18ec` — long-vertical and long/short-horizontal run-table partitioning.
168. `19035959` — initial vertical-shift and horizontal-ratio lag construction.
169. `2f702ae9` — append-only short-section registration with lag-global IDs.
170. `fce93241` — production Java/Rust GRID run-dispatch differential vector.
171. `4f5ab233` — exact thick/thin horizontal section dispatch.
172. `69bad0f0` — ordered adjacent one-run sticker discovery.
173. `67162af1` — exact internal `completeLines` lifecycle and failure semantics.
174. `e6c0df9c` — typed staff-line section inclusion decision.
175. `d0c5636d` — typed discarded-filament inclusion decision.
176. `0edbd7b1` — ties-even StaffFilament hole insertion planning.
177. `133c1244` — two-sided neighboring-line hole-point interpolation.
178. `5713195a` — Java-ordered section inclusion traversal and assignment plan.
179. `70977909` — Java endpoint jitter-search sequence and boundary handling.
180. `dbc9a099` — discarded-filament traversal and ownership mutation.
181. `f2c9928d` — complete staff-line endpoint retrieval.
182. `2b582d74` — exact curved-filament curvature polishing.
183. `aa4d05b8` — production `GridStep.doit` lifecycle and failure order.
184. `cd419f76` — `StaffLineCleaner` simplify/remove/rebuild/populate lifecycle.
185. `81c2213e` — `Book.createScores` and `Book.updateScores` topology.
186. `50bb6423` — real-pixel crossing-chunk inspection and removal.
187. `1a145861` — `Staff.simplifyLines` lifecycle and partial-failure mutation.
188. `f5f85dae` — live Java/Rust score-regrouping differential fixture.
189. `428e722d` — no-staff horizontal-lag rebuild and reset semantics.
190. `9a8fc090` — system/page population and section ownership.
191. `c02ab205` — concrete filament glyph registration and persistent staff-line conversion.
192. `b2882109` — curved GRID system areas and side-by-side slicing.
193. `04370090` — `SystemInfo.buildRef` soft-reference identity and ownership.
194. `cec9a53e` — page allocation wired to fresh system references and backlinks.
195. `43ecff8f` — live Java/Rust `SystemInfo.buildRef` differential vector.
196. `47cd7873` — concrete GRID bar/bracket SIG identities, relations, and freezing.
197. `9be6dce6` — exact removal of original staff sections and runs from the GRID lag.
198. `4788c1db` — concrete headless GRID sheet/page/reference/score executor state.
199. `6b62cba8` — promoted barline grouping with exact gap and partial-failure behavior.
200. `4c9c2985` — glyph-backed persistent lines and ordered GRID SIG ownership attachment.
201. `a72a910c` — concrete GRID raster lag creation and short-section stages.
202. `a61466e3` — partial raster-lag handoff after swallowed and step failures.
203. `4bcc75b2` — sheet-owned installation of completed and partial raster prefixes.
204. `ac5f0c94` — production-backed prepared line-cluster retrieval and staff materialization.
205. `39392d64` — production-backed prepared bar-system processing and global edge remapping.
206. `8c51f6b2` — production-backed prepared line completion state and lifecycle.
207. `d37b227e` — exact composed Java/Rust GRID output-boundary vector.
208. `a44e2a77` — concrete staff bar ownership and system group/part tail.
209. `4c053118` — detached StaffProjector brace-candidate ownership.
210. `304d53c7` — GRID SIG contextual grading in final system order.
211. `efd64567` — live production Java/Rust SIG contextual-grade vector.
212. `6c0cf709` — exact Java comb-network fragment following.
213. `d1714e2e` — primary cluster-pass construction from a live horizontal lag.
214. `6a7443d4` — Java-ordered curvature and slope rejection.
215. `73702157` — live-lag production `RetrieveLines` and staff handoff.
216. `8d879240` — concrete raw-raster sheet-aware GRID executor constructor.
217. `cd8a3583` — raw filament rejection before comb sampling and clustering.
218. `fc1e8338` — Java `FilamentIndex` creation identities and swallowed gaps.
219. `d48742c5` — measured raw slope, fallback handoff, and short-filament parity.
220. `01130871` — measured raw GRID slope documented at the executor boundary.
221. `eca69716` — exact sheet skew applied across downstream GRID geometry.
222. `62ac6567` — lazy small-interline raw cluster pass with preserved identities.
223. `380af50e` — positive, negative, and zero Java/Rust skew-transform vector.
224. `14050774` — Java-ordered final discarded-line population carried into completion.
225. `c0712ba7` — live-raster staff projector construction with exact deskew centers.
226. `c0b91f75` — raw projector registry materialized into the peak-graph boundary.
227. `ad7ce242` — concrete raster-fitted `DefineEndPoints` completion collaborator.
228. `36094408` — resolved endpoints installed into mutable filament spline geometry.
229. `9696f615` — VLAG/HLAG raw bar sticks, section attachment, and curvature marking.
230. `2b70107f` — concrete discarded-filament inclusion, ownership, and recomputation.
231. `b94bc88e` — exact raw-raster `retrieveLines` Java/Rust differential vector.
232. `1955b867` — skew-aware raw `findAllAlignments` traversal and relations.
233. `0d68e795` — exact Java/Rust raster-fitted endpoint and mutated-spline vector.
234. `d4d40a4f` — pixel-backed raw bar connections and relation replacement order.
235. `80b27163` — targeted single-pair alignment and connection helpers for splitting.
236. `32f83337` — exact Java/Rust raw alignment discovery differential vector.
237. `f05db960` — concrete initial staff-filament hole filling and spline regeneration.
238. `9b1baf9b` — fixed-point merged-bar split and post-success alignment purge kernel.
239. `a33b86fd` — exact Java/Rust pixel-backed connection differential vector.
240. `c49b8628` — raw split subfilaments, rediscovery, connection, and purge integration.
241. `b5d54b66` — shared concrete thick/thin section inclusion completion stages.
242. `88225193` — raw peak-graph system grouping and initial column construction.
243. `416f7878` — prepared staff-filament curvature polishing and retained failure prefix.
244. `4666b99b` — exact pre-brace column/start/purge coordinator prefix.
245. `b1a2345b` — raw bar processing bridged to the brace-evidence boundary.
246. `de0f387b` — exact Java/Rust `StaffFilament.fillHoles` differential vector.
247. `14906986` — all three prepared hole-fill invocations over live geometry.
248. `9c44d9f5` — brace-portion evidence gates, windows, and replacement intents.
249. `ba4f0453` — non-transactional mistaken-first-bar replacement mutation.
250. `4840bf42` — prepared one-pixel staff-sticker inclusion and endpoint preservation.
251. `05de4f60` — brace polygon selection and compound curved-filament construction.
252. `4b8856ee` — prepared crossing-chunk inspection, removal, and recomputation.
253. `76e6c3c2` — brace glyph registration and ordered system-SIG promotion.
254. `309877e3` — dependency-light headless `HEADERS` step and `StaffHeader` boundary.
255. `5127409c` — injected headless `HeaderBuilder` shell and mutation lifecycle.
256. `03a65cb4` — complete raw 11-stage line-completion composition.
257. `5381b34b` — raw post-brace purge and exact lines-root correction.
258. `5e4df552` — neutral clef-column orchestration and injected recognition boundary.
259. `e01716f8` — raw bracket-end and bracket-middle detection.
260. `4bf97f99` — neutral key-column orchestration and global offset selection.
261. `f58eac74` — neutral header-time column orchestration.
262. `ebcc4a13` — raw left, unaligned, and extending peak purges.
263. `e6c4bf73` — automatic ordered `ProcessBars` ownership handoff to completion.
264. `f16a9c4e` — per-staff clef candidate lifecycle around injected proposals.
265. `9290188f` — per-staff key-signature candidate lifecycle and pitch maps.
266. `942cf2ce` — raw right-end refinement and C-clef false-bar purge.
267. `89a57818` — whole and paired header-time candidate lifecycle.
268. `f5bcb361` — raw width partition and vertical bar/bracket inter creation.
269. `06ca0e80` — headless `STEM_SEEDS` step lifecycle.
270. `3c66c442` — concrete stem-width histogram, peaks, fallback, and scale mutation.
271. `4fd4d55d` — bar-connection inter creation and bar grouping.
272. `66dce292` — vertical stem-seed factory/checker boundary and mutation order.
273. `a074a473` — staff bar recording and part-group construction.
274. `46ffb7ad` — headless `BEAMS` step lifecycle.
275. `8aee84dc` — raw part construction and contextualization, completing BarsRetriever.
276. `02bfab02` — concrete beam-spot morphology, thresholds, runs, and dispatch.
277. `8046eafb` — per-system beam candidate orchestration and grouping order.
278. `fb5f4f9c` — direct final bar-tail ownership into all line-completion stages.
279. `397c5f4a` — multiple-rest selection and ordered SIG replacement lifecycle.
280. `a7d46b29` — headless `LEDGERS` step lifecycle.
281. `d13d32ea` — native beam-spot connected components and glyph registration.
282. `e3aa7e71` — raw ledger zoning, filtering, grading, and overlap reduction.
283. `ad50df70` — concrete ledger StickFactory filament geometry.
284. `b5a7e36c` — headless `HEADS` step lifecycle and ownership order.
285. `807095ac` — beam-structure borders, splitting, and core/belt raster impacts.
286. `192c628a` — ledger glyph/SIG materialization, exclusions, and staff ownership.
287. `ee2aab98` — headless `STEMS` lifecycle.
288. `a812f1b0` — native beam impacts at the classifier seam.
289. `c82eb969` — native heads prolog.
290. `538c804a` and `fc42ae52` — beam-extension evidence and seam exposure.
291. `5401e360` — headless `REDUCTION` lifecycle.
292. `e56b11a6` and `b276c0ce` — native stem retrieval orchestration and concrete stem checker.
293. `7e9b7a90`, `be7313d0`, and `a45c54de` — hook evidence, Java-compatible positive-area intersections, and seam exposure.
294. `9203e13c` — headless `CUE_BEAMS` lifecycle.
295. `3715c8a2` — native stem-link geometry kernel.
296. `979d7791` and `8d7a83d4` — native beam-group geometry and seam exposure.
297. `bbb51002`, `5832be3c`, `517c0d49`, `bca50fbb`, `7b8a942a`, `81e201bf`, `9cba2956`, and `bd24daf2` — dependency-light headless lifecycles for `MEASURES` through `PAGE` in pipeline order.
298. `26382f6b` and `3d265640` — native multiple-rest serif evidence and seam exposure.
299. `be184be8`, `602c23c7`, and `a685b5cf` — native header clef, key, and time candidate sourcing.
300. `ade15e54` — immutable bundled `BasicClassifier` model parser and 110→149→149 sigmoid inference core.
301. `f7bdcbd1` — live Java oracle for all 149 raw grades of a fixed 110-value classifier input; the isolated probe loads the frozen bundled artifact explicitly.
302. `77149f6a` — native point-list `MixGlyphDescriptor` extraction: 99 ART modules, 10 geometric values, and aspect, with an asymmetric live Java oracle.
303. `dd563914` — Java-order RunTable foreground traversal and absolute-offset adapter into classifier features, with a live coordinate-and-feature vector.

At checkpoint 303 the Rust workspace executes 875 tests:

- `audiveris-core`: 38
- `audiveris-image`: 506
- `audiveris-omr`: 310
- `audiveris-testkit`: 6
- `audiveris-cli`: 4
- `xtask`: 5
- `audiveris-classifier`: 6

The live Java/Rust oracle compares 73 canonical vectors at this checkpoint. Since
checkpoint 64 it added exact vectors for comb discovery, line-cluster lifecycle,
short projections, StaffProjector derivative thresholds, blank selection, peak-side
refinement, peak-candidate construction, core-pixel validation, range scanning,
brace discovery, composed projection, lines-root correction, recursive cluster
coordination, and StaffProjector result operations.
The latest vector additionally drives production Java and Rust through connected
bar-chain aggregation, column geometry/connectivity, and initial start selection.
The newest vector invokes production Java `LagManager.dispatchRuns` and matches Rust
on preservation of the source table, the long-vertical partition, and the reoriented
short-vertical pixels used for horizontal staff processing.
The latest vector additionally executes production Java `Book.updateScores` and the
Rust topology port across a movement-boundary removal, reinsertion, and following-score
merge, matching both the initial two-score grouping and final one-score result exactly.
The newest vector freezes production `StaffFilament.fillHoles`, including ties-to-even
insertion, neighbor interpolation and fallback, defining-point order, and regenerated
spline position/slope.

The bundled classifier is now parsed and evaluated natively without a Java runtime:
model XML, normalization vectors, labels, and the two bias-first sigmoid layers are
validated and held immutable. This is deliberately only the inference core. Raw glyph
feature extraction (`BasicARTMoments` and geometric moments), Java candidate sorting/
minimum-grade policy, user overrides, and MusicFont metrics remain separate seams. A
Java-backed fixed-feature oracle now verifies every raw output grade. The point-list
extractor now produces the complete `MixGlyphDescriptor` input layout from foreground
coordinates, matching a live asymmetric Java vector. Native RunTable foreground pixels
now flow through the same descriptor with Java sequence/run/pixel order and absolute
offset semantics. Ranking/minimum-grade policy, user overrides, and MusicFont metrics
remain separate; do not represent it as a complete visual classifier.

SCALE matches on Chula plus three parent-corpus pages: K545 exercises a small-interline
population, Essen rejects a weak beam and extrapolates, and Josquin accepts a weak beam
exactly at the two-pixel distance threshold. Commit `27dbfeb6` briefly encoded the wrong
out-of-domain combo behavior; `87b6a4e3` corrects it and freezes the Java behavior in
both a focused test and the full-page vector. GRID now matches both a branch-heavy
synthetic section fixture and the real Chula page through run dispatch, long-run
purging, both lag policies, and every section's run content digest.
The next GRID boundary also matches Java for compound bounds, weight, its historical
true-length hole arithmetic, thickness, endpoint probes, five spline positions/slopes,
and range checks. Floating spline output is explicitly canonicalized at `1e-14` because
HotSpot and Rust differ by one ULP in one quadratic expression.
The factory slice now also matches Java's core/local-fatness filtering, stable
reverse-length traversal, successful/rejected real-gap merges, and every horizontal overlap gate:
sample placement, ordinate delta, combined/individual probe thickness, consistency,
internal space, slope, and expansion contact. Its full neutral lifecycle now includes
leftover selection, fixed grown-box filtering, repeated attachment, and the final merge.
A bounded digest covers real Chula page sections without turning the oracle into an
unbounded production run. Glyph/index ownership and vertical filaments remain outside.
The lossless `book.xml` view now exposes absent-versus-empty persisted step lists and
the latest completed stage while preserving all original bytes and rejecting unknown
or duplicate step tokens.
Direct sheet input path and positive image rank are also typed atomically; an absent
input remains distinct because Java then falls back to the book-level source.
The same lossless view now exposes sheet compatibility attributes and direct page
references while leaving nested SIG content opaque. GRID additionally has the
dependency-light sticker filter, comb state, regular comb discovery, and ordered
line-cluster core. Cluster merge, absorption, trimming, geometry, and the combined
lifecycle now have exact live Java parity. Recursive cluster construction, general
merge orchestration, and the same-size pair pass are now ported with transactional
stable-ID ownership. Cluster consistency destruction and two-sided isolated-filament
expansion are also ported, followed by desired-size destruction, trimming, and
unclustered-filament partitioning. The neutral cluster pipeline now composes the Java
stage order transactionally through optional consistency, second expansion, one-line
recovery, and false-ledger rejection. Glyph creation, SIG integration, and UI behavior
remain outside.
Target-line deskew mapping begins the neutral destination geometry used later in GRID
cleanup.
Target-line mapping now has exact live parity on a sloped source, and the surrounding
page/system/staff target containers preserve source order without recreating Java's
object cycles. The `.omr` view derives order-only system references exactly as Java
does rather than inventing persisted IDs.
Regular vertical comb sampling feeds the neutral comb representation, and both comb
discovery and the line-cluster lifecycle have exact production-Java vectors. Bar
columns have exact parity across fixed slots, cached means, overwrite invalidation,
full/start/brace status, and concrete graph connectivity. BarsRetriever now also has
neutral C-clef purging, bracket-end and bracket-middle decisions, group/part topology,
serif geometry/selection, connection-component freezing, and bar/bracket inter
geometry/type plans. A transactional coordinator now composes column construction,
start validation, partial/left/unaligned/C-clef purges, related-column deletion, width
classification, and interpretation planning with rollback on missing evidence. Neutral `StaffPeak`,
`PartGroup`, and stable-ID `PeakGraph` types now cover graph storage, incident and
connection queries, alignment purge, median connection geometry, and brace checks
without recreating Java object cycles. Concrete sheet-owned SIG state now registers
bar/bracket glyph and inter identities, peak backlinks, connector nodes and relations,
connection freezing, and grouped-barline edges. It preserves Java's system-major
vertical/group passes, global connection-edge order, per-connection catches, and
ordinary-error prefix mutation. The post-group tail now records barline IDs on concrete
staff state and stores group/part plans on concrete system state in Java order. Detached
`StaffProjector.getBracePeak()` candidates remain separately owned when absent from the
ordinary peak list, and the final system-ordered pass contextualizes every GRID SIG node
from intrinsic grades without changing topology or frozen state. A live Java/Rust vector
freezes the unequal support-chain arithmetic, ignored relations, insertion order, and
state preservation.

The neutral LinesRetriever path now constructs primary filaments from the live horizontal
lag, applies Java's curvature purge, stable reverse-length slope estimate, asymmetric
short-horizontal tolerance, and slope purge before comb sampling, then executes Java's
comb-network fragment joining and main cluster pass. The coordinator retains the optional
small-interline pass over ID-sorted primary discards and Java's buildStaves
purge/layout/right-indentation sequence. It returns typed standard,
one-line, and tablature staff candidates with median sides and small/short flags while
keeping curvature and slope rejects distinct. Slope rejects remain available for later
fallback; curvature rejects do not. The identity-aware factory registers every accepted
core and temporary expansion candidate in Java creation order, preserves swallowed gaps,
and accepts the next sheet-global `FilamentIndex` ID from its caller.

The headless GRID coordinator now joins that staff-candidate output to the transactional
BarsRetriever coordinator in production order. The production outer lifecycle continues
through staff-line simplification, lag-section removal, no-staff horizontal-lag rebuild,
system population, and movement-aware score regrouping. System population now preserves
Java's clear-first/non-transactional failure behavior, horizontal and vertical section
ownership order, indentation traversal, physical page/PageRef allocation, and report
maxima. Curved line/quadratic/cubic staff boundaries now reproduce neighbor expansion,
vertical margins, strict containment, reversed south paths, and side-by-side midpoint
slicing under production's x-monotone staff-spline invariant. The concrete executor now
invokes `StaffFilament.toStaffLine`, registers the union glyph before +0.5 ordinate
adjustment and exact iterative spline simplification, and stores the persistent line.
Its clear-first loop also preserves Java's unusual conversion-failure prefix: converted
lines and glyphs remain while the current and later originals are detached. `SystemInfo.buildRef`
preserves fresh-reference replacement, shared backlinks, physical part/staff order, exact
`StaffConfig` defaults, separate PageRef append, and Java partial mutation on collaborator
failure, and those references are now wired into page allocation, sheet state, and score
regrouping. A stage-owned raster builder now concretely creates both initial lags, adds
short sections, and installs every completed prefix into the sheet on success, swallowed
failure, or step failure. Prepared cluster, bar-system, and completion adapters call the
production-backed Rust coordinators and preserve their outputs across the sheet-aware
driver. An additive raw `RetrieveLines` adapter now builds primary and lazy small-
interline states from that live lag, materializes a staff handoff, and the concrete raw-
raster executor installs the staff, raster prefix, measured skew, and ordered slope-
reject fallback filaments into sheet state. The measured slope replaces any caller
placeholder during line purge/layout. The secondary pass retries only primary discards,
preserving Java's separate slope-reject lifecycle. Completion receives the authoritative
final cluster rejects followed by every original slope reject, with typed provenance and
exact failure prefixes. `DefineEndPoints` now performs the live raster pattern search and
mutates filament endpoints, spline cache, and bounds; `IncludeDiscardedFilaments` performs
the stable system traversal, inclusion test, section steal, `partOf` assignment, and
endpoint recomputation. Initial hole filling preserves cluster-position interpolation,
virtual-point fallback, point-before-spline partial mutation, and old-spline retention on
failure. Thick and thin candidate sections share the exact stable, ID-indexed batched
inclusion core with explicit systems and once-per-line recomputation. Curvature polishing,
later hole/sticker passes, crossing inspection, and several transactional exceptional paths
remain, so this is not yet a claim that raw-page GRID is fully behaviorally equivalent.

The StaffProjector slice now composes scale-derived parameters, raster accumulation,
`ShortProjection`, derivative thresholds, blanks, candidate refinement, core-pixel
validation, multi-rest serif rejection, six-impact grading, brace discovery, and
neutral peak output. Result-list, lines-root, and right-end decisions are also ported,
and the BarsRetriever registry preserves retained-staff/projector order and unique
graph-vertex intents. Downstream SIG promotion, detached brace ownership, and GRID
contextual grading are now concrete. An additive raw adapter constructs each projector
from prepared staff geometry and the live zero-foreground raster, applies Java rounding,
and attaches the exact stored deskew center to ordinary and detached-brace peaks before
registry insertion. Registry peaks now enter a real peak graph, acquire bar sticks from
VLAG then HLAG sections, receive curvature/brace classification, and run Java's raw-
endpoint/skew-aware alignment discovery without prematurely purging competing edges.
They then undergo pixel-backed connection promotion, fixed-point merged-group splitting,
targeted edge rediscovery, and the correctly delayed alignment conflict purge. Multi-staff
system construction and the remaining completion collaborators are the next boundaries.

The newest composed differential constructs the same two-system synthetic sheet in live
Java and Rust. It matches the swallowed `PROCESS_BARS` prefix, 15 persistent staff glyphs
and their geometry digest, five bar glyphs, semantic SIG nodes/relations/freezing/grades,
two physical pages and reference backlinks, and two score movements. This closes the
newly attached ownership boundary exactly, but is not a raw-image recognition fixture.

The `.omr` view now continues through ordered score page links, logical parts, score-root
metadata, sheet selection, legacy beam/OCR metadata, and book interline/beam/OCR/lyrics
parameters in addition to page, system, part, and staff configuration data. Parameter
views preserve absent, inherited, and explicit integer/string/boolean states, including
explicit false versus true. Legacy `<line-count>` remains distinct from current JAXB;
unknown XML and archive members remain byte-preserved.

A one-off read-only audit also opened, parsed, re-encoded, and byte-compared every member
of three real Audiveris 5.11.0 archives: Essen (115,350 uncompressed bytes), K545
(898,147), and Schumann Op. 48 No. 2 (1,547,112). Each had four members and one sheet;
tightened resource limits rejected all three. The disposable audit executable was not
retained, so this is evidence, not yet a checked-in regression.

## Verify before editing

From `rust/`:

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- baseline
cargo run -p xtask -- vectors
cargo run -p xtask -- manifest
```

To rerun Java rather than inspect its current XML reports:

```sh
cargo run -p xtask -- baseline --run-java
```

Both Java-running commands resolve the sibling JDK automatically when `JAVA_HOME`
is absent. `vectors` compiles its probe against the real frozen Audiveris classes;
it does not duplicate production Java implementations in the harness.

## Design decisions to preserve

- Headless recognition comes first. Do not port Swing package structure into Rust.
- Java is the behavioral oracle until each stage passes differential fixtures.
- Rust crate boundaries follow data flow, not Java's cyclic packages.
- Use tagged enums and stable IDs for SIG `Inter`/`Relation` types; do not reproduce
  the Java inheritance graph.
- Keep exact topology and integer classifications strict. Use declared tolerances
  only for floating grades, geometry, fonts, OCR boxes, and PDF rasterization.
- Compare canonical semantic MusicXML graphs, not XML bytes or ZIP member order.
- Preserve unknown `.omr` ZIP members, XML nodes, attributes, IDs, and IDREFs in the
  initial read-only compatibility layer.
- Parity reproduces Java behavior, including Java errors. Accuracy improvement is a
  separate held-out gate and requires an explicit divergence waiver.

## Next implementation slices

Commit each slice separately after the full verification block above.

1. Port Java candidate ranking/minimum-grade policy independently, then add a narrow
   connected-component/Glyph ownership adapter around the existing RunTable path. Do not
   couple this work to MusicFont sizing or a recognition-stage behavior change.
2. Complete the remaining concrete visual seams in `HEADERS`, `STEM_SEEDS`, `BEAMS`,
   `LEDGERS`, and `HEADS`, stopping at the first new raw-image differential boundary.
3. Extend `.omr` typing only through bounded read-only views that preserve every
   unknown byte and distinguish absent, malformed, and undeclared members explicitly.
4. Migrate future stage snapshots onto `audiveris-testkit` incrementally; keep the
   current vector ordering stable while its key-aware diagnostics catch schema drift.
5. Add Tesseract data to the oracle manifest when its resolved runtime location is
   known; the bundled classifier, fonts, JDK metadata, and image fixtures are frozen.
6. Freeze or vendor the three parent-corpus SCALE pages before expecting `xtask vectors`
   to work in a standalone Audiveris clone; today those vectors deliberately resolve
   `../../data/synth/...` from this parent OMR checkout.
7. Port deeper semantic behavior in `OmrStep` order; stop comparison at the first
   differing stage so later agreement cannot hide an upstream mismatch.

## Differential fixture plan

Use canonical PNGs for algorithm parity. Treat PDF rasterization as a separate tolerant
gate. Deep cases should include `chula`, `BachInvention5`, rotated `SchbAvMaSample`,
multi-page `Dichterliebe`, `zizi`, `allegretto`, and `carmen` from `data/examples`, plus
Papillons and a held-out IMSLP set.

For each stage record stable, sorted neutral data:

- page dimensions and scale;
- binary mask hash, black count, runs, and sections;
- systems, staves, measures, and coordinate frames;
- every interpretation's shape, bounds, grade, staff/system/measure, and semantic data;
- every SIG relation and exclusion/support decision;
- classifier top-k vector and OCR output where applicable.

Final gates are semantic MusicXML equality, bidirectional `.omr` compatibility, held-out
accuracy/non-regression, and performance. The Java UI is not part of the initial
production-sidecar milestone.

## Incremental-commit rule

Never leave the branch depending on an uncommitted multi-stage rewrite. A commit message
must identify the ported behavior, and `PORTING.md` must be updated in the same commit.
If interrupted mid-slice, reset nothing: leave the last green commit intact and describe
the uncommitted failure at the top of this file before handing off.

## Next slice: line completion (started, not finished)

`recognize_grid_lines` drives GRID stage by stage and now matches the Java
oracle on staff geometry, slope, systems, and barlines for every example page.
It deliberately does **not** go through `HeadlessGridExecutor`; it calls the
subsystems directly. Line completion is the point where that shortcut runs out,
so this is the wiring the next slice has to build.

### What blocks it

`complete_lines` (`line_completion.rs:37`) runs eleven stages against a
`LineCompletionExecutor`. The production chain for those stages already exists:
`production_line_completion(parameters)` (`prepared_completion.rs:211`) composes
DefineEndPoints, IncludeDiscardedFilaments, FillHolesInitial, IncludeSections,
PolishCurvatures, IncludeStickers, and InspectCrossingChunks in Java order, and
`production_grid_parameters` already derives every parameter it needs
(`completion`, `maximum_thin_weight`, `inspect_crossing_chunks`). Those three
fields are currently derived and unused.

The chain is reached through
`HeadlessGridExecutor::from_completed_raw_bars_complete_lines`
(`grid_executor.rs:774`), whose `downstream` argument must implement
`RemainingRasterGridStages` (`raster_grid_builder.rs:86`): `retrieve_lines`,
`process_bars`, and the remaining stage hooks.

**This is now partly done.** `ProductionRasterStages`
(`crates/audiveris-omr/src/production_stages.rs`) is the first production
implementation of that trait; before it, the only one was the `RasterStages`
test double at `grid_executor.rs:1417`, so the raster-executor path had never
run outside tests.

`retrieve_lines` is real: it performs the measure-then-cluster primary passes
and staff retrieval through the builder, and a test drives `build_grid_info`
end to end on chula, getting the same six staves and the same measured slope as
the direct driver. `process_bars` and `complete_lines` record their stage and
return.

**Do not extend that struct; migrate off it.** Its `retrieve_lines` duplicates
`RawProductionRetrieveLines` (`prepared_lines.rs:345`), which already implements
the same stage and additionally handles the small-interline secondary pass,
retained sloped filaments, and the raw metadata handoff. The ported shape is the
decorator chain `RawProductionRetrieveLines -> ProductionProcessBars ->
ProductionCompleteLines`, composed as
`HeadlessGridExecutor::from_completed_raw_bars_complete_lines` does.

The one thing blocking a straight drop-in is `ProductionProcessBars::new`
(`prepared_bars.rs:100`): it takes an already-built `Vec<BarsSystemState>`
rather than deriving one, and those states need projectors, graded peaks,
alignments, and connections that the chain does not produce.
`recognize_grid_lines` does produce them and is oracle-matched on every example
page. So the migration is: keep that derivation, feed its `BarsSystemState`
values into `ProductionProcessBars`, and let `ProductionCompleteLines` carry the
already-ported completion chain.

### Suggested order

1. `retrieve_lines` is done. Move the projector, alignments, sticks,
   connections, and the two purge entry points from `recognize_grid_lines` into
   `process_bars` the same way. The logic is already written and
   oracle-verified, so this is a re-shaping, not new recognition code. Keep
   `recognize_grid_lines` working off the new stages so the existing barline and
   system parity tests keep guarding the move.
2. Build the `HeadlessGridSheet` and `HeadlessGridBook` initial state. The exact
   required fields, and which are overwritten by handoffs rather than
   pre-filled, are enumerated per field in the raster-path tests around
   `grid_executor.rs:1942-2043`; `sheet_number`, `no_staff_table`, `max_fore`,
   `ledger_thickness`, and the `population` geometry/boundaries/systems must be
   supplied, while `staffs`, `horizontal_lag`, `vertical_lag`, and `skew` are
   installed by the handoffs.
3. Call `from_completed_raw_bars_complete_lines` and run the executor, then
   assert the eleven completion stages ran in Java order and that staff lines
   gained endpoints and filled holes.
4. Oracle check: compare completed staff-line endpoints against Java's
   `sheet#1.xml` staff `left`/`right` and line points. Java's values are already
   known to sit within about three pixels of the current raw geometry, so
   completion should close that gap rather than move it.

Keep `AUDIVERIS_DEBUG_PURGE=1` in mind: it prints per-peak removal stages on the
Rust side, and the same diagnosis on the Java side is a temporary log in
`StaffProjector.removePeak` that walks the stack for the calling `purge*` method
(reverted after use, easy to reapply).
