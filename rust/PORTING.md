# Porting contract and status

## Frozen source baseline

- Audiveris source: `9e1e55cd2746037d059345881c53e6a6754bffbd` (`5.11.0`)
- Java command: `JAVA_HOME=../jdk25/Contents/Home ./gradlew :app:test --no-daemon`
- Baseline on 2026-08-02: 212 tests, 0 failures, 0 errors, 1 skipped
- Production source inventory: 991 Java files, about 327,673 lines

The checked-in Java tests are necessary but not sufficient. They cover only a
small fraction of the recognizer. Every recognition stage also needs golden and
differential tests against the frozen Java executable.

## Compatibility gates

1. Rust unit tests port the observable assertions from the corresponding Java tests.
2. Neutral fixtures compare deterministic Java and Rust results, including errors.
3. `.omr` input/output remains versioned and round-trip safe. Unknown XML and ZIP
   members must be preserved until the Rust schema is complete.
4. Stage outputs are compared in pipeline order so downstream agreement cannot hide
   an upstream mismatch.
5. Whole-score MusicXML and recognition metrics are evaluated separately from unit
   test parity.

## Pipeline

`LOAD -> BINARY -> SCALE -> GRID -> HEADERS -> STEM_SEEDS -> BEAMS -> LEDGERS ->`
`HEADS -> STEMS -> REDUCTION -> CUE_BEAMS -> TEXTS -> MEASURES -> CHORDS -> CURVES ->`
`SYMBOLS -> LINKS -> RHYTHMS -> PAGE`

The first implementation slice ports dependency-light contracts used throughout the
pipeline: natural-number specifications, rational arithmetic, online populations,
arrangement generation, the pipeline-step enum, and CLI parsing.

## Current status

| Area | State |
| --- | --- |
| Java oracle | frozen, executable verifier green |
| Live Java/Rust vectors | 36 canonical cross-runtime fixtures matching |
| Oracle asset manifest | classifier, 6 fonts, and 8 image fixtures SHA-256-frozen |
| Differential testkit | deterministic sorted vectors and first-difference diagnostics used by `xtask`; bounded fixture roots |
| Rust workspace | building |
| Core utility slice | implemented with parity tests |
| Histogram, grades, injection solver | implemented with parity tests |
| Least-squares line geometry | implemented with parity tests |
| Natural spline geometry | line/quadratic/natural-cubic interpolation and horizontal evaluation; exact live vector at declared 1e-14 boundary |
| CLI parameter slice | implemented with parity tests |
| Binary run-table primitives | implemented with parity tests |
| Median filter and chamfer distance transform | implemented with parity tests |
| Global threshold, alpha compositing, polygon masks | implemented with parity tests |
| Gray-level watershed flooding | implemented; exact live Java/Rust vector |
| Threshold/median/chamfer/run differential fixtures | exact Java/Rust match |
| PNG/JPEG raster load and max-channel grayscale | implemented; canonical PNG exact |
| Adaptive local thresholding | implemented; exact synthetic and two full-page masks |
| SCALE vertical runs and black/combo histograms | inputs exact; decision stage implemented |
| SCALE integer functions and range primitives | implemented with Java edge semantics |
| SCALE derivative peak finder | implemented; exercised by full-page Java oracle |
| SCALE line, interline, and beam estimate | exact match on 4 pages and key branch cases |
| GRID section construction | neutral horizontal/vertical sections, four junction policies, exact synthetic and full-page Chula lag topology/statistics |
| GRID staff-filament geometry | section compounds, probe centroids, true length, thickness, endpoints, positions, slopes; exact live synthetic vector |
| Remaining filters and PDF ingest | queued |
| `.omr` persistence | opaque round trip plus fresh lossless typed `book.xml`/per-sheet views and explicit stub/member resolution states |
| Recognition stages | queued in pipeline order |
| MusicXML differential suite | queued |
| Swing UI | explicitly out of the initial headless milestone |
