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
| Live Java/Rust vectors | 62 canonical fixtures, including composed StaffProjector, recursive clusters, bar-column construction/start selection, production `LagManager.dispatchRuns`, `Book.updateScores` regrouping, and live `SystemInfo.buildRef` ownership |
| Oracle asset manifest | classifier, 6 fonts, and 8 image fixtures SHA-256-frozen |
| Differential testkit | deterministic sorted vectors and first-difference diagnostics used by `xtask`; bounded fixture roots |
| Rust workspace | 517 tests green at checkpoint 200: core 38, image 360, OMR 104, testkit 6, CLI 4, xtask 5 |
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
| GRID filament factory | core and local-fatness filtering, stable reverse-length traversal, real-gap/overlap merge gates, leftover expansion, and final merge; exact merge/rejection plus bounded real-page vectors; glyph/index and vertical behavior queued |
| GRID section tally | stable first-position indexing with explicit sorted/range validation for staff-line sticker retrieval |
| GRID line stickers | owned-member exclusion, stable full-position order, cumulative above/below contact, one-run retention, top-before-bottom adjacent discovery, exact thick/thin dispatch, typed section/filament inclusion decisions, and Java-ordered section assignment with endpoint restoration intent |
| GRID staff pattern scoring | zero-valued foreground matching with fractional interlines, inclusive line span, ties-even placement, and out-of-bounds penalties |
| GRID comb and cluster core | ties-even comb discovery, weighted popular size, stable-ID ownership, recursive formation, transactional inclusion, general merges, same-size pair pass, short/inconsistent/undesired discard, upper-median acceptable length, two-sided expansion, filament partition, trim, geometry, extrapolation, and an executable stage-ordered retrieval pipeline with optional one-line recovery and ledger-like rejection; glyph finalization queued |
| GRID peaks and peak graph | neutral `StaffPeak`, `PartGroup`, and stable-ID graph storage; incident/connection queries, alignment purge, connection median geometry, brace checks, stable glyph/inter identities, peak backlinks, connector relations, freezing, and bar-group relations |
| GRID StaffProjector | scale parameters, raster accumulation, adaptive thresholds, blanks, peak refinement, core validation, multi-rest serif rejection, six-impact grading, brace discovery, and composed neutral process; result, lines-root, and right-end decisions; ordered BarsRetriever registry; graph edges and sheet ownership queued |
| GRID LinesRetriever | exact outer `GridBuilder` and inner `completeLines` stage/exception lifecycles; source-preserving run dispatch, long/short partitions, initial vertical/horizontal lag policies, append-only short-section registration, sticker classification/assignment, StaffFilament hole insertion/interpolation, full endpoint retrieval, discarded traversal, curvature polishing, and crossing inspection; transactional cluster passes and typed staff candidates; concrete `toStaffLine` now builds/registers the union glyph, applies the +0.5 ordinate, performs Java spline simplification, and assigns the glyph |
| GRID bars and columns | peak grouping/purges including C-clef false bars, width partition, section selection, group links, graph components and chain aggregation, column construction, partial/extension/unaligned purge, start-column selection and validation, bracket-end/middle/serif decisions, cached geometry, ordered within-part connection selection, group/part topology, bar-connection freeze traversal, concrete bar/bracket and connector SIG promotion, and exact bar grouping; exact column/start vector; the post-group `recordBars`/group/part/contextualize tail remains queued |
| GRID coordinator | exact `GridStep.doit` order through builder, real filament-to-registered-glyph persistent-line conversion, staff-line cleanup, and score regrouping; Java-compatible outer exception semantics plus a separate transactional LinesRetriever-before-BarsRetriever join; StaffLineCleaner lifecycle and no-staff horizontal-lag rebuild; source-order system population, curved areas/slicing, section ownership, indentation, physical-page/PageRef allocation, report maxima, and `SystemInfo.buildRef` soft-reference identity/ordering/defaults; concrete sheet/page/SIG state attachment and Java partial-failure prefixes; the low-level raster `GridBuildExecutor` remains supplied rather than concrete |
| GRID target geometry | source-guided target-line deskew mapping with exact live sloped-line parity plus immutable cycle-free page/system/staff containers |
| Remaining filters and PDF ingest | queued |
| `.omr` persistence | opaque round trip plus lossless typed `book.xml`/per-sheet views, explicit stub/member states, pipeline status, sheet input provenance, compatibility attributes, page references and links, order-derived systems, part/staff configuration, logical parts, score-root metadata, sheet selection, legacy beam/OCR metadata, and book interline/beam/OCR/lyrics parameters; absent, inherited, and explicit values remain distinct |
| Recognition stages | queued in pipeline order |
| MusicXML differential suite | queued |
| Swing UI | explicitly out of the initial headless milestone |
