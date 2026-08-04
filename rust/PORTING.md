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
| Live Java/Rust vectors | 73 canonical fixtures, including composed StaffProjector, recursive clusters, bar-column construction/start selection, production `LagManager.dispatchRuns`, `Book.updateScores` regrouping, live `SystemInfo.buildRef` ownership, a composed GRID output boundary, production SIG contextual grading, exact sheet-skew transforms, raw-raster `retrieveLines`, raster-fitted mutable endpoints, raw alignment/connection discovery, exact `StaffFilament.fillHoles` mutation, all 149 raw grades for a fixed classifier feature vector, an asymmetric point-list MixGlyphDescriptor feature vector, and Java-order RunTable coordinate/feature extraction with absolute offset |
| Oracle asset manifest | classifier, 6 fonts, and 8 image fixtures SHA-256-frozen |
| Differential testkit | deterministic sorted vectors and first-difference diagnostics used by `xtask`; bounded fixture roots |
| Rust workspace | 875 tests green at checkpoint 303: core 38, image 506, OMR 310, testkit 6, CLI 4, xtask 5, classifier 6 |
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
| GRID filament factory | core and local-fatness filtering, stable reverse-length traversal, real-gap/overlap merge gates, leftover expansion, and final merge; identity-aware retrieval reproduces Java `FilamentIndex` registration for accepted cores and temporary expansion candidates, including swallowed gaps and sheet-global continuation; exact merge/rejection plus bounded real-page vectors; glyph-index attachment and vertical behavior otherwise queued |
| GRID section tally | stable first-position indexing with explicit sorted/range validation for staff-line sticker retrieval |
| GRID line stickers | owned-member exclusion, stable full-position order, cumulative above/below contact, one-run retention, top-before-bottom adjacent discovery, exact thick/thin dispatch, typed section/filament inclusion decisions, and Java-ordered section assignment with endpoint restoration intent |
| GRID staff pattern scoring | zero-valued foreground matching with fractional interlines, inclusive line span, ties-even placement, and out-of-bounds penalties |
| GRID comb and cluster core | ties-even comb discovery, weighted popular size, stable-ID ownership, exact comb-network fragment following with Java length/tie/traversal semantics, recursive formation, transactional inclusion, general merges, same-size pair pass, short/inconsistent/undesired discard, upper-median acceptable length, two-sided expansion, filament partition, trim, geometry, extrapolation, and an executable stage-ordered retrieval pipeline with optional one-line recovery and ledger-like rejection; live-lag primary and lazy small-interline secondary construction are concrete, preserve original main-interline filament identity/geometry, and keep slope rejects outside the secondary pass as Java does |
| GRID peaks, bars, and systems | raw registry peaks materialize in exact order and proceed through sticks, alignments, pixel connections, fixed-point splits, systems/columns, brace and bracket evidence, every ordered purge/refinement, width partition, vertical/connection inter creation, grouping, bar recording, staff groups, parts, and contextual grades; stable IDs, graph/projector/column/SIG ownership, Java quirks, and non-transactional failure prefixes are covered |
| GRID StaffProjector | scale parameters, raster accumulation, adaptive thresholds, blanks, peak refinement, core validation, multi-rest serif rejection, six-impact grading, brace discovery, and composed neutral process; result, lines-root, and right-end decisions; ordered BarsRetriever registry; graph edges and sheet ownership queued |
| GRID LinesRetriever | exact outer `GridBuilder` and inner `completeLines` stage/exception lifecycles; source-preserving run dispatch, long/short partitions, initial vertical/horizontal lag policies, append-only short-section registration, live-lag primary filament construction, Java-ordered curvature and slope rejection with short-horizontal tolerance, comb-network joining, and concrete `toStaffLine`; the raw path now executes all 11 completion stages in Java order through inner `Finish`: raster endpoints, discarded lines, three live-geometry hole-fill passes, horizontal dispatch, thick/thin inclusion, curvature polishing, isolated sticker inclusion, and crossing-chunk inspection, with measured slope, explicit ordered system ownership, typed audits, and retained mutation prefixes |
| GRID bars and columns | peak grouping/purges including C-clef false bars, width partition, section selection, group links, graph components and chain aggregation, column construction, partial/extension/unaligned purge, start-column selection and validation, bracket-end/middle/serif decisions, cached geometry, ordered within-part connection selection, group/part topology, bar-connection freeze traversal, concrete bar/bracket and connector SIG promotion, exact bar grouping, staff-owned recorded barlines, system-owned group/part plans, detached projector brace candidates, and GRID-specific SIG contextualization; exact column/start and live contextual-grade vectors |
| GRID coordinator | exact `GridStep.doit` order through builder, real persistent-line conversion, staff cleanup, and score regrouping; concrete raster lags, raw primary/secondary clusters, projectors, complete BarsRetriever, and all 11 line-completion stages; final contextualized bar-system ownership flows directly into completion without replay or injected bar-tail collaborators; measured skew/slope, discarded candidates, source-order systems, page/reference ownership, audits, and failure prefixes are explicit and tested |
| GRID target geometry | source-guided target-line deskew mapping with exact live sloped-line parity plus immutable cycle-free page/system/staff containers |
| GRID production parameters | scale-derived `production_grid_parameters` reproduces every Java `Parameters` derivation for the raster GRID chain (LagManager partition, FilamentFactory, ClustersRetriever, lines coordinator, completion, bars coordinator) with Java `rint` ties-to-even semantics, line-thickness-based fields, and percentile comb bounds; chula-scale values locked by unit test |
| Native recognition entry | `audiveris_omr::recognize` runs LOAD-BINARY-SCALE, the GRID staff-line slice (run partition, horizontal lag, measure-then-cluster primary passes, staff candidates), and a per-staff `StaffProjector` producing graded bar peaks, all from production scale-derived parameters; `audiveris-cli -batch -step SCALE|GRID` prints reports. Chula is locked against a live Java oracle: slope 0.00792, 6 standard staves in 3 systems with system-1 indentation, staff extents within 3 px, and 58/58 Java barlines covered by projector peaks. The peak graph then adds Java `findAllAlignments` across staves, `buildBarSticks` registering one vertical filament per peak from the initial grid lags, and `findConnections` promoting alignments on the corridor gap/white-ratio test, from which staff systems are derived: system grouping matches the live Java oracle on all nine example pages, including single-staff and three-staff systems. The `BarsRetriever` purges then run per system (left-of-staff, unaligned, curved/short, width, C-clef, and column stages, after `purgeAlignments`), narrowing chula's 103 raw candidates to 54. **Not yet at parity**: the live Java oracle reports 58 for the same page. The gap is fully localised - every missing peak is a staff's own opening barline, and nothing else diverges; the interior barlines match Java one for one. Marking each opening peak `STAFF_LEFT_END`, which is what exempts it from Java's `purgeTooLeft`, recovers the openings of staves 5 and 6 (52 to 54). Staves 1 to 4 still lose theirs to a second mechanism, most likely the indented-system handling Java logs for system 1. No test asserts barline parity until that is resolved |
| Remaining filters and PDF ingest | queued |
| `.omr` persistence | opaque round trip plus lossless typed `book.xml`/per-sheet views, explicit stub/member states, pipeline status, sheet input provenance, compatibility attributes, page references and links, order-derived systems, part/staff configuration, logical parts, score-root metadata, sheet selection, legacy beam/OCR metadata, and book interline/beam/OCR/lyrics parameters; absent, inherited, and explicit values remain distinct |
| Visual classifier core | native immutable parser/inference for the frozen bundled 110→149→149 `BasicClassifier` model, including Java normalization, bias-first sigmoid layers, point-list `MixGlyphDescriptor` ART/geometric/aspect extraction, and Java-order native RunTable adaptation; Glyph ownership, rank/minimum-grade policy, overrides, and MusicFont metrics remain unported |
| HEADERS recognition stage | headless `StaffHeader`, step lifecycle, `HeaderBuilder`, and native clef/key/time candidate sourcing plus per-staff candidate lifecycles; exact ranges, proposal ordering, pitch maps, grade/context selection, exclusions, stop propagation, cleanup, ownership, and failure prefixes are native around remaining visual classifier seams |
| STEM_SEEDS recognition stage | headless lifecycle, concrete stem-scale histogram/peak/fallback, vertical factory/checker orchestration, and concrete stem checker implemented; raw vertical StickFactory geometry remains an explicit visual seam |
| BEAMS recognition stage | headless lifecycle, concrete morphology/threshold/run evidence, native connected-component glyphs, system dispatch, candidate ordering/retry/extensions/group orchestration, multiple-rest replacement, BeamStructure border/core/belt impacts, hook/group/extension/serif evidence; remaining seams are classifier/materialization and listed raster-geometric internals |
| LEDGERS recognition stage | lifecycle, raw zones/runs/sections, concrete horizontal StickFactory filaments, all gates/seven impacts, overlap reduction, glyph/SIG materialization, exclusions, and staff ownership implemented; visual filter input remains explicit |
| HEADS recognition stage | dependency-light lifecycle, native prolog, transient spot dispatch contract, ordered classifier mutations, glyph/inter/SIG/staff ownership, checked/fatal prefixes, cleanup, and quorum scale implemented; visual spot/classifier internals remain queued |
| Later recognition stages | dependency-light lifecycles are native for `STEMS`, `REDUCTION`, `CUE_BEAMS`, `TEXTS`, `MEASURES`, `CHORDS`, `CURVES`, `SYMBOLS`, `LINKS`, `RHYTHMS`, and `PAGE`; their semantic/visual algorithms remain queued in pipeline order |
| MusicXML differential suite | queued |
| Swing UI | explicitly out of the initial headless milestone |
