# omrscope

A Qt6 window over the Audiveris Rust port: run both engines on the same sheet,
see what each made of it, and see how much of the pipeline is actually ported.
Both engines run concurrently and publish a retained, immutable result whenever
they finish a native recognition stage from `GRID` through `HEADS`.

Built for one person's use. It is a debugging instrument and a piece of
documentation that cannot go stale, because it reads the engines rather than
describing them.

## Build

```sh
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build -j8
./build/omrscope /path/to/audiveris
```

Needs Qt 6 including SvgWidgets (`brew install qt`) and, for the Java side, a
JDK 25 — `JAVA_HOME` if set, otherwise the sibling `jdk25` the oracles use.
The Rust side needs the release binary:

```sh
cd ../../rust && cargo build --release -p audiveris-cli
```

The optional **Score** tab also needs a local Verovio executable (`brew install
verovio`). Set `OMRSCOPE_VEROVIO=/absolute/path/to/verovio` to select a
particular installed build. omrscope never downloads or substitutes a renderer:
an invalid explicit path is an actionable error, not a fallback to some other
version on `PATH`.

## Headless

The same runners and parsers, with no window, so a run can be checked from a
terminal:

```sh
./build/omrscope /path/to/audiveris --compare data/examples/chula.png
```

```
rust: 84 inters, 45 rejected, 86 relations, 6 staves, 141.3 ms (process wall clock)
java: 87 inters, 0 rejected, 86 relations, 6 staves, 1481.4 ms (in-process reachStep only)
paired 58, agree 58, differ 0, only-one-side 29, not comparable 26
```

Exit code is 0 when nothing differs, 2 when something does.

## Live stage comparison

The window starts the Rust and Java producers independently. Each producer
emits a completed-stage snapshot in pipeline order. The **Through stage**
selector offers `GRID`, `HEADERS`, `STEM_SEEDS`, `BEAMS`, `LEDGERS`, and
`HEADS`; the **Recognition stage timeline** shows `Stage`, `Rust`, `Java`, and
`Comparison`. It follows the newest completed common stage when both engines
are selected (or the newest completed selected engine otherwise). Clicking a
timeline row or choosing a **Snapshot stage** number and pressing **Inspect
stage** freezes that stage's Page, Inters, and Raw output snapshots. The visual
timeline is exposed to accessibility as one flat, named client; the separate
numeric stage chooser reaches every snapshot without entering its dynamic row
hierarchy. Like the Inters grid, the visual timeline is mouse-only; keyboard
and accessibility use go through the numeric chooser and **Inspect stage**.
A later failure does not discard an earlier completed snapshot,
which makes the first divergent stage inspectable. Pressing **Run** starts a
fresh attempt and clears
the prior timeline; retention is within one attempt, not a run-history store.

This is deliberately **completed-stage** streaming, not a claim of live
per-item recognition. A page changes after `GRID`, `HEADERS`, `STEM_SEEDS`,
`BEAMS`, `LEDGERS`, or `HEADS` completes; items inside a running stage are not
sent to the window.

## Inspecting an inter

The **Inters** table and **Page** view are two views of the same filtered,
same-stage pairing. Click a cell to inspect its row and centre the reported
Rust and/or Java geometry while remaining on **Inters**; a cyan outline marks
the inspected row without using native table selection. The visual table is
deliberately mouse-only and cannot take keyboard focus. For keyboard and
accessibility use without exposing Qt's unstable native table row/cell
hierarchy, choose any one-based visible row in **Inspect row** and press
**Inspect row**. The visual table is exposed as one flat, named client; the
separate row chooser can reach every filtered row without entering it.
Then click **Page** to see the labelled overlay for each engine. It
never re-pairs by ID, looks into a different streamed stage, or makes a
cross-engine graph edge while doing that. A row without any reported geometry
(such as a connector) remains the inspected row but is
not assigned a made-up position on the page.

**Highlight visible rows** is opt-in. It paints a translucent yellow underlay
for all rows that the current kind filter leaves visible; the ordinary green,
blue, and red agreement strokes remain above it. **Graph edges** is also
opt-in. It draws dashed, engine-coloured SIG edges only when the producer's
`system + source id + target id` resolves to exactly one drawable inter at the
selected stage. Missing, duplicate, identity-free, or geometry-free endpoints
are omitted rather than guessed.

The ordinary `audiveris-cli ... -json` output remains the schema-1 JSONL
interchange contract. The viewer opts into the additive `-stream-json` mode:
`@omrscope` schema-1 control markers bracket the unchanged JSON stage document.
That separation keeps existing JSONL readers compatible and lets the viewer
receive boundaries and elapsed times without redefining recognition payloads.
Java uses the equivalent probe stream privately through `omrscope`; its normal
oracle output remains unchanged.

## Score export and engraving

**Score** is a separate, manual Java PAGE request, reached only by pressing
**Export & render Java**. It uses the current input and sheet selector, exports
one Java MusicXML artifact to a fresh private temporary directory, validates
the reported canonical path, byte count, and SHA-256 locally, then asks the
same installed Verovio executable for SVG pages. The selected sheet must yield
one Audiveris score-page artifact; a result that would require sibling
`.mvtN` exports is rejected as `unsupported_multipage` rather than guessed.
This does not limit how many engraved SVG pages Verovio may produce from an
accepted artifact. The tab reports the artifact path, format, bytes, digest,
renderer executable/version, and page count; it supports page navigation,
fit-relative zoom, and saving a copy of the Java MusicXML artifact.

This is deliberately not triggered by **Run** or by a `HEADS` snapshot. It
does not make ordinary recognition streaming reach `PAGE`, and it must not be
read as visual or semantic parity: Verovio is engraving Java's artifact only.
The Rust panel states the truth directly: `PAGE` and MusicXML export are not
implemented, so there is no Rust score artifact to render or compare.

Artifacts stay available until the next explicit score request or application
exit. Cancel waits for the active Java export or Verovio process to terminate
before the temporary directory can be replaced; closing the window likewise
cancels and reaps active score work.

## Reading the numbers

**The two timings are not the same measurement, and the tool never puts them in
one column.** Rust is process wall clock, whose startup is a few milliseconds.
Java is measured *inside* `SigProbe`, around `reachStep` alone, because Gradle
and JVM startup take tens of seconds and would swamp any comparison. The ratio
shown is engine against engine.

**"Not comparable" is not a disagreement.** Connectors and braces have no
abscissa to match on: the port's `ConnectionInterPlan` carries neither a median
nor a width, and brace peaks stay detached from SIG promotion. Those are known
gaps, listed in `rust/PORTING.md`, and counting them as mismatches would be
alarming and wrong. They are labelled instead.

**"Only one side" is worth looking at.** On chula it is Java's 26 connectors and
its braces, for the reason above. Anything else in that bucket is a real
finding.

## Tabs

- **Page** — the sheet with both engines drawn over it. Green where they agree,
  blue for Rust only, red for Java only, dashed amber for a candidate the port
  considered and rejected. An inspected table row gets labelled Rust/Java
  overlays; optional filtered-row underlays and engine-local graph edges add
  context without changing those agreement colours. Scroll to zoom about the
  cursor, drag to pan.
- **Inters** — every interpretation side by side with its grade, its contextual
  grade, and the impacts the grade is a weighted geometric mean of. Only
  disagreement is coloured; agreement should be quiet.
- **Stage timeline** — the Rust and Java state for each stage and a selector for
  its immutable completed snapshot. It reports stage boundaries, not
  intra-stage item progress.
- **Score** — an explicit Java PAGE-to-MusicXML export and local Verovio SVG
  preview. It identifies the Java artifact and renderer version, but makes no
  engraving/parity claim; Rust PAGE/MusicXML remains unavailable.
- **Port status** — the twenty pipeline stages and which are native, which have
  only their lifecycle ported, and what blocks each. This one is a hand-kept
  snapshot rather than parsed from `PORTING.md`, because a dashboard that
  silently mis-parses prose is worse than one you have to update.
- **Raw output** — exactly what each engine printed, for when the parse is the
  thing in doubt.

## Rendering caveat

PDF sheets are rasterised by **Qt**, not by the port, purely so there is
something to draw on. It is not evidence about ingest — `audiveris-pdf`'s corpus
test is what grades that, against PDFBox, on 189 pages.
