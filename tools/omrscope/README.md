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

Needs Qt 6 (`brew install qt`) and, for the Java side, a JDK 25 — `JAVA_HOME`
if set, otherwise the sibling `jdk25` the oracles use. The Rust side needs the
release binary:

```sh
cd ../../rust && cargo build --release -p audiveris-cli
```

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
are selected (or the newest completed selected engine otherwise). Selecting a
row freezes that stage's Page, Inters, and Raw output snapshots. A later
failure does not discard an earlier completed snapshot, which makes the first
divergent stage inspectable. Pressing **Run** starts a fresh attempt and clears
the prior timeline; retention is within one attempt, not a run-history store.

This is deliberately **completed-stage** streaming, not a claim of live
per-item recognition. A page changes after `GRID`, `HEADERS`, `STEM_SEEDS`,
`BEAMS`, `LEDGERS`, or `HEADS` completes; items inside a running stage are not
sent to the window.

The ordinary `audiveris-cli ... -json` output remains the schema-1 JSONL
interchange contract. The viewer opts into the additive `-stream-json` mode:
`@omrscope` schema-1 control markers bracket the unchanged JSON stage document.
That separation keeps existing JSONL readers compatible and lets the viewer
receive boundaries and elapsed times without redefining recognition payloads.
Java uses the equivalent probe stream privately through `omrscope`; its normal
oracle output remains unchanged.

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
  considered and rejected. Scroll to zoom about the cursor, drag to pan.
- **Inters** — every interpretation side by side with its grade, its contextual
  grade, and the impacts the grade is a weighted geometric mean of. Only
  disagreement is coloured; agreement should be quiet.
- **Stage timeline** — the Rust and Java state for each stage and a selector for
  its immutable completed snapshot. It reports stage boundaries, not
  intra-stage item progress.
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
