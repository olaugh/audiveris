# Porting contract and status

Potential upstream Java defects and source/documentation inconsistencies found
during differential work are tracked in
[`AUDIVERIS_UPSTREAM_FINDINGS.md`](AUDIVERIS_UPSTREAM_FINDINGS.md). The port
preserves confirmed Java behavior until an upstream change is deliberately
re-frozen.

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

## Boundary evidence: full and fast modes

STEMS is ported as a chain of exact boundaries, each replaying its predecessors and
stopping one step further along. Two evidence levels are sanctioned; the parity claim
is identical in both, only the amount of corroborating detail differs.

**Full evidence** is what boundaries 1-17 carry: all eight beam sheets, two fresh-JVM
runs required to be byte-identical, per-row predecessor SHA-256 chaining, hex-exact
double bit patterns, and isolated synthetic envelopes for Java branches the corpus does
not reach.

**Fast evidence** may be used for a boundary whose behaviour is small and whose state
effects are already modelled by its predecessors. It keeps everything that can falsify
parity and drops what only corroborates it:

- kept: the Java oracle from the frozen baseline; the exact Rust gate against it; count
  assertions on every comparison loop; one page-level predecessor hash so a stale
  predecessor cannot go unnoticed; both CI legs.
- dropped: per-row predecessor hash chains; synthetic envelopes for unreached branches;
  hex bit patterns for values that are integers, booleans or identities.
- reduced: two representative sheets rather than eight, and a single JVM run.

Adjacent boundaries that form one control-flow unit may be ported together under fast
evidence rather than separately.

**Checkpoints restore full evidence.** At least every fourth boundary, and always before
leaving a stage, regenerate all eight sheets from two fresh JVMs, require byte-identical
output, and run the whole gate. A divergence introduced under fast evidence is found
here, and the blast radius is the boundaries since the previous checkpoint.

Fast evidence is a deliberate speed/latency trade made by the project owner, not a
lowering of the parity bar: a fast-evidence boundary is still bit-exact against Java on
the sheets it freezes, or it does not land.

### Replay-on-frozen: validating re-implemented emitters and replayed machinery

When a boundary needs to reproduce machinery that already ran at an earlier frontier --
an oracle emitter written fresh, a replayed executor, a ported helper -- do not argue
its correctness; **run it against the earlier frontier first, where frozen fixtures
already exist, and require byte-identity with the frozen rows before trusting it on the
new frontier.** A fresh emitter that reproduces transaction 1's frozen rows exactly is
proven to encode the same field semantics, value formats, and ordering as the original;
only then may its output for transaction 2 be frozen as a new oracle.

This turns the largest silent risk in oracle extension -- an emitter that produces
plausible but subtly different evidence -- into a mechanical check that fails loudly.
The same principle already guards probe extension: a probe that extends a predecessor
re-emits the predecessor's rows verbatim, and the runner byte-compares them against the
frozen predecessor fixture before writing anything new (see the Boundary 18/19/20
runners). Prefer this over review-by-eye every time a frozen twin exists to compare
against; where none exists, say so in the fixture header rather than implying the
stronger check ran.

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
| STEMS chain sequencing | Fed the frozen pass's per-transaction `linkSiblings` writes, the resume chain runs chula system 1's 32 transactions in Java's exact order, closing the 53-vs-32 gap. Wiring it found a defect in `resume_native_stems_beam_scheduler_after_transaction`: the completed transaction's linked B linkers were folded in after the forward walk rather than before, so the walk could re-run a side that transaction had just linked. Fixed; the frozen corpus and resume gates are unchanged by it |
| STEMS SIDES pass model | `StemsBeamSidesLoopProbe` runs the pass to exhaustion and records each transaction's `linkSiblings` writes and the pre-pass linked set. chula system 1: 32 transactions, 21 skips, every skip explained by an earlier sibling write, nothing linked before the pass. 32 + 21 = 53 matches the unfed chain exactly, so a chain carrying each transaction's Boundary-16 sibling result forward needs no bootstrap for this. `the_sides_pass_accounts_for_every_skipped_side` asserts the closure. Two fresh JVM passes byte-identical |
| STEMS SIG bootstrap evidence (superseded as architecture) | `AUDIVERIS_SIG_SNAPSHOT_OUT` records chula system 1's 221 vertices / 202 edges in JGraphT insertion order, and `sig_snapshot_derives_the_incident_scans_java_recorded` proves that one global order determines every `edgesOf` query. Production `NativeSigSystem` assembly now reproduces the structural hashes and the carried B12-B19 path reaches the full SIDES terminal, so this file is frozen expected evidence rather than a self-drive blocker. Wider systems still need complete BEAMS-group products |
| Native SIG through HEADS and into STEMS | `assemble_native_sig` owns the insertion-ordered per-system GRID-through-HEADS graph; chula system 1 reproduces Java's 221 vertex / 202 edge structural hashes exactly. `advance_native_stems_beam_sides_transaction` atomically advances one frontier: scheduler, latest B14/transaction state, SIG/bindings, and persistent B/S cells swap only after B19 succeeds. Repeated calls execute all 32 native SIDES transactions and reach explicit `SidesExhausted` at 253 vertices / 331 edges, 32 Stem bindings, 61 linked/open B cells, and 68 linked/open S cells. Exact plan/B-linker order and all 29 sibling-write lists match the frozen Java pass only after the native terminal is returned; its 21 `AlreadyLinked` skips therefore come entirely from earlier native B16 writes. The carrier then enters chula system 1's 34-beam STUMPS worklist and preserves structural-side-before-linked precedence. Boundaries 22-24 call the same production carrier for plans 147, 622, and 404, reaching 256/340, 35 Stem bindings, B64/S74 before resuming at worklist index 3 to beam SIG 28 / stump 1 / plan 508. Boundary 24 adds the first natural two-glyph compound candidate in this carried STUMPS prefix without changing production code. Boundary 25 adds a bounded atomic batch driver and carries plans 508, 28, 330, and 251 to the typed post-STUMPS terminal at 260/353, 39 Stem bindings, B68/S83. A typed first-STEMS bridge maps the 1,058 system-1-visible native modeled objects into one disclosed 1,650-entry persistent snapshot, retains 592 opaque fingerprint-only entries, and drives transactions 3-32 without per-frontier glyph rows or exhaustive scans. It does not invent Java IDs or use opaque entries for equality/absence. B14 consumes only the 16 distinct selected-base identity rows used by the 32 transactions instead of all 48 live beams; those rows still disclose Java persistent Inter ID, sorted InterIndex ordinal, and VIP, while source/vertex, group, removal, abnormal state, geometry, and graph queries are native-owned. The graph-derived B13 path is now gated on one real later linked-S reconstruction: Allegretto system 1 transaction 28 traverses HeadStem edge 229, selects the modeled attached StemInter with Java ID 2227, and leaves the second entry unread; the gate explicitly reconstructs rather than natively carries transactions 1-27. Transactions 1-2, the persistent ID/allocator/union snapshot, and the sparse identity bridge remain fixture-backed. Boundary 26 also removes and resumes past one real competing hook from an explicitly reconstructed Allegretto post-transaction-28 checkpoint. Boundary 27 validates all 102 live heads and persistent S cells, preserves Java's stable reverse-grade order, and transfers the exact post-STUMPS carrier into a typed first head-origin C-link frontier without mutation. Boundary 28 atomically applies that frontier's bounded one-item, nonrecursive `CreatedChecked` mutation and stops before head index 1. Boundaries 29-32 carry head orders 1-5 through prelinked success and twelve ordered shared-stem closure writes to `current_index=6`, still with no unlinked head. The path does not yet own the remaining head queue, an actually-unlinked retry, or broader C-linker shapes, general sheet/book dirty state, native predecessor carriage and wider linked-S or hook-removal coverage, wider-corpus STUMPS authority and branch coverage, or every corpus BEAMS group |
| Continuous integration | `.github/workflows/rust-port.yml` runs fmt, Clippy with `-D warnings`, and `cargo test --workspace` on `ubuntu-latest` and `macos-latest` -- two architectures as well as two systems, which is the axis that caught the host-dependent libjpeg reference below. `rust-toolchain.toml` pins the channel because `-D warnings` makes an unpinned Clippy a source of failures with no code change. The PDF corpus test skips in CI (its 20 MB of IMSLP scans are not fetched) and the last step re-runs it with `--nocapture` so a skip cannot read as a pass; nothing Java-backed runs there |
| Live Java/Rust vectors | 73 canonical fixtures, including composed StaffProjector, recursive clusters, bar-column construction/start selection, production `LagManager.dispatchRuns`, `Book.updateScores` regrouping, live `SystemInfo.buildRef` ownership, a composed GRID output boundary, production SIG contextual grading, exact sheet-skew transforms, raw-raster `retrieveLines`, raster-fitted mutable endpoints, raw alignment/connection discovery, exact `StaffFilament.fillHoles` mutation, all 149 raw grades for a fixed classifier feature vector, an asymmetric point-list MixGlyphDescriptor feature vector, and Java-order RunTable coordinate/feature extraction with absolute offset |
| Oracle asset manifest | classifier, 6 fonts, and 8 image fixtures SHA-256-frozen |
| Differential testkit | deterministic sorted vectors and first-difference diagnostics used by `xtask`; bounded fixture roots |
| Structured output and live comparison | Ordinary `-json` emits the unchanged schema-1 document per requested sheet through HEADS. The opt-in `-stream-json` viewer protocol adds flushed `@omrscope` schema-1 boundary markers around those unchanged documents, yielding immutable **completed-stage** snapshots from GRID through HEADS; it does not expose item-by-item or intra-stage recognition. `omrscope` runs Rust and Java independently and concurrently, retains/selects each completed snapshot, and keeps the ordinary JSONL and Java oracle outputs compatible. Its Page/Inters inspection surface now highlights the inspected pair without native table selection, can opt into highlighting all filtered rows, and can draw engine-local relation edges only when both endpoint IDs resolve uniquely in that selected engine snapshot; it never infers cross-engine graph edges. GRID's byte path remains unchanged; later documents add selected clef/key/time inters with lifecycle/classifier evidence, accepted stem seeds with exact checker/materialization evidence, system-owned header erases, horizontal beam/ledger geometry, impacts, beam groups, live ledger exclusions, curved ledger-line paths, and identity-free final heads. HEADS retains every upstream product and publishes seed/range provenance, exact glyph bounds/weight/run digest, source-resolved beam decisions, counts, and Java-order tally-scale rows without fabricating SIG or glyph IDs. Text after GRID remains explicitly unsupported. `omrscope` parses bounds-only header inters and both median forms, adapts accepted top-level stem seeds into the common display/pairing model without inventing schema IDs, ignores rejected seeds, and rejects incomplete geometry. A separate manual Score tab runs one selected Java sheet through PAGE, validates its explicitly produced single local MusicXML/MXL artifact, and renders it to local Verovio SVG pages; a sheet requiring sibling multi-page artifacts is rejected rather than guessed. That preview is not semantic parity and does not make PAGE or MusicXML native: Rust PAGE/MusicXML remains unimplemented. Future Rust output will use this same renderer path for an honest side-by-side artifact view. The workspace carries no serialization dependency |
| Rust workspace | The workspace now contains eighty-four exact STEMS boundaries. Boundaries 44, 46, 53, and 60 consume two-item LEFT/BOTTOM continuation heads from both-open/unlinked frontiers; Boundary 62 consumes a bounded single-item LEFT/BOTTOM C-link and moves SIG to 685/696 and system stems to 46. Boundaries 63-75 reconcile x14, x18, x97, x6, x30, x43, x25, x83, x57, x40, x89, x52, and x35 against existing Stems 2340, 2372, 2373, 2348, 2357, 2350, 2356, 2358, 2374, 2350, 2359, 2344, and 2369 without graph allocation; Boundary 76 adds the first returned-false LEFT undef, and Boundaries 77-82 and 84 reconcile x19, x15, x84, x11, x68, x21, and x92 against existing Stems 2361, 2360, 2366, 2349, 2347, 2341, and 2342 while carrying that undefined LEFT side; Boundary 79's three-head shared stem re-writes x86's already-closed cells without a value change. Boundary 83 consumes the both-open x62 frontier by reusing existing Stem 2381 through one appended HeadStem relation and closing sibling x63, and Boundary 84 reaches `current_index=59` with no unlinked head. Boundaries 54-59 and 61 use the unchanged generic continuation for four-write x85, two-write x10/x101/x16/x88/x50, and zero-write x34 prelinked closures. The separate v18-v58 Java derivatives are snapshot-minimized; v28-v58 use the reduced heap-safe shape introduced after the default v28 full-snapshot probe exhausted heap. The focused Boundary-84 gate, full 14-test sibling suite, strict workspace Clippy, formatting, and diff checks are green. Geometry remains bounded to authenticated single- or two-item LEFT/BOTTOM cases, including the x74-specific one-ulp downward and x2-specific one-ulp upward line translations; generic retry, actually-unlinked/no-link, and broader C-link geometry remain open. Boundary 28 consumes Boundary 27's typed first head frontier through one atomic single-item, nonrecursive `CreatedChecked` C-link mutation. Boundary 26 adds one bounded atomic competing-hook removal and SIDES resume from a reconstructed Allegretto-system-1 post-transaction-28 checkpoint; predecessor transactions 1-27 are not natively carried. `5f75f8708` (including Boundary 43) remains the current remote CI baseline: Rust run 32217412749 passed all 12 shards and Build & Test run 32217412751 passed with no failure or cancellation. The production pipeline owns the complete pre-STEMS SIG through HEADS; its chula-system-1 221-vertex / 202-edge structural hashes are bit-exact to Java. CI repeats formatting, strict Clippy, and workspace tests on Ubuntu and macOS |
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
| GRID staff-free image | `Picture.getSource(NO_STAFF)`: the binary raster with every staff line's glyph painted white, then vertical runs. Erased by the ink assigned to each line rather than a geometric band, so what survives is exactly what GRID could not explain as a staff line. Bit-exact on all nine example pages against `oracle/grid-nostaff.txt`, the JPEG included. Getting there required running GRID's own `CleanStaffLines` stage, which the driver had never invoked -- lines stayed filaments with no registered glyph, so nothing was erased -- and building the table inside `rebuild_horizontal_lag` from the sheet's persistent glyphs, mirroring `Picture`'s lazy build from the glyphs `simplifyLines` just created. Everything downstream that reads ink rather than geometry starts here: LEDGERS, BEAMS, `SheetScanner`, `KeyExtractor` |
| GRID coordinator | exact `GridStep.doit` order through builder, real persistent-line conversion, staff cleanup, and score regrouping; concrete raster lags, raw primary/secondary clusters, projectors, complete BarsRetriever, and all 11 line-completion stages; final contextualized bar-system ownership flows directly into completion without replay or injected bar-tail collaborators; measured skew/slope, discarded candidates, source-order systems, page/reference ownership, audits, and failure prefixes are explicit and tested |
| GRID target geometry | source-guided target-line deskew mapping with exact live sloped-line parity plus immutable cycle-free page/system/staff containers |
| Binary raster parity | `rust/oracle/grid-binary.txt` pins the FNV-1a-64 of Java's BINARY raster for all nine example pages, taken from `sheet#1/BINARY.png` inside the saved `.omr`. Eight of nine reproduce Java **bit for bit**; every later GRID comparison rests on that. The ninth, `BachInvention5.jpg`, is the corpus's only JPEG and is the single root cause of every remaining GRID divergence -- see the row below |
| JPEG decoding | **Ported, pure Rust, bit-exact.** `audiveris-jpeg` decodes baseline sequential 8-bit Huffman JPEG (1 or 3 components; 4:4:4, 4:2:2, 4:2:0) reproducing libjpeg sample for sample, because Java's `ImageIO` reader is libjpeg-backed and the ecosystem's pure-Rust decoders are not: `zune-jpeg` differs from libjpeg on 0.7% of samples for a grayscale file, 5.1% for 4:2:0, and `jpeg-decoder` on 2.7% / 5.1%. Measuring per stage showed the inverse DCT already differs before any colour or upsampling work, which ruled out a small patch to either crate. Reproduced here: the integer IDCT's fixed-point multipliers, its two-pass descaling and rounding, libjpeg's wrap-around sample range-limit table, the fixed-point YCbCr to RGB tables with the green channel's single deferred descale, and the fancy triangle-filter upsamplers with their asymmetric per-half biases. Progressive, arithmetic, 12-bit, and CMYK are refused rather than approximated. `tests/parity.rs` runs libjpeg-turbo alongside and requires every sample to match, over 110 fixtures spanning the sampling factors, odd dimensions, 1x1, restart markers, truncation, and mid-scan corruption, plus the full corpus page. Verified to build for `wasm32-unknown-unknown`. **Differential fuzzing (`fuzz/matches_libjpeg`) then found five further sample divergences and four accept/reject ones, all fixed**, and not one of them was in an algorithm. Three were about *which* algorithm libjpeg picks or how wide its integers are: it declines the fancy upsamplers when a plane is at most two samples wide, and it dequantizes in `int` but transforms in `JLONG` (64-bit here), which only shows on coefficients near the coding limit. Two were error recovery: an over-long AC run still consumes its magnitude bits and still stores the stranded coefficient at index 63, and a successful restart clears the out-of-data flag, so a short segment resumes instead of greying out to the end of the image. The accept/reject cases -- a scan naming an undeclared component, a trailing marker with an impossible length, a Huffman table whose DC symbols exceed the magnitude range, a second SOI, a frame header shorter than its own fields, an oversized dimension, a reserved frame marker -- are pinned by `rust/oracle/jpeg-verdicts.txt`, recorded from Java's ImageIO, because no sample comparison can see them: the port was producing images where Audiveris raises `IIOException`. A sweep over all 254 marker bytes and all 140 legal sampling combinations now agrees with Java exactly; the sampling sweep also found 4:4:0 unimplemented and its vertical triangle filter's asymmetric bias wrong once written. **There are two libjpegs, and picking the wrong one cost a day.** Audiveris reads through Java's ImageIO, whose reader is libjpeg -- but which libjpeg depends on the JDK build: Temurin statically links the bundled 6b, Ubuntu's OpenJDK links the system libjpeg-turbo. Turbo added a vertical fancy upsampler 6b lacks, so they disagree on every 4:4:0 image; and turbo widened the transform's intermediates from `INT32` (which OpenJDK defines as `int` on LP64) to 64-bit `JLONG`, so they disagree wherever a product passes 2^31. Audiveris ships a bundled JRE, so **6b is the target**. Two fixes made against the turbo oracle -- widening the transform, adding the vertical filter -- were right for turbo and wrong for Audiveris; reverting both closed the last divergences. `rust/oracle/jpeg-verdicts.txt`, generated on the same Temurin JDK 25 as the other oracles, now records Java's verdict *and* an FNV-1a-64 of its raster for all 129 fixtures, and every accepted one reproduces it exactly, as do all 140 sampling combinations. `TURBO_DIVERGENCES` in `tests/parity.rs` names the eleven fixtures where turbo is the side that differs, so the faster differential test skips them rather than chasing them. **There is also more than one turbo**: `mozjpeg-sys` compiles SIMD whenever the host allows it -- unconditionally on aarch64, on x86 only with `nasm` installed -- and mozjpeg's SIMD routines disagree with mozjpeg's own scalar C on damaged input, which made the reference depend on the build host and the same commit pass on one machine and fail on another. On `corrupt-resync-80x80-420.jpg` the scalar build returns `011e68ce7a923ae5`, which is Java's recorded raster and the port's; the NEON build returns `a5649ea51e999926`, 1032 of 19200 samples apart. The dev-dependency now sets `default-features = false`, which is also the more faithful reference, since libjpeg 6b has no SIMD at all -- and drops `nasm` from the build requirements, which is part of why the two-OS CI matrix is clean |
| GRID production parameters | scale-derived `production_grid_parameters` reproduces every Java `Parameters` derivation for the raster GRID chain (LagManager partition, FilamentFactory, ClustersRetriever, lines coordinator, completion, bars coordinator) with Java `rint` ties-to-even semantics, line-thickness-based fields, and percentile comb bounds; chula-scale values locked by unit test |
| SCALE oracle parity | `<line>` and `<interline>` min/main/max plus beam thickness match the live Java oracle on all nine example pages, percentiles included; the GRID comb bounds derive from those percentiles |
| PDF ingest, wired | `ingest::Loader` is Java's `ImageLoading.Loader`: an input is a book of sheets, ids are one-based, dispatch is on the `.pdf` extension rather than magic bytes as `getLoader` does, and a PDF page is rendered through the port's own rasterizer. `audiveris-cli -batch -step GRID score.pdf` runs, processing every sheet or the `-sheets` subset. `crates/audiveris-image/tests/pdf_ingest.rs` pins the seam end to end -- all 189 corpus sheets reach binarization with the same FNV-1a-64 as PDFBox's rendered page -- which is a different claim from the PDF crate's own and is where `Picture.adjustImageFormat`'s maximum-channel rule could otherwise have hidden a conversion. `oracle/grid-pdf.txt` grades the *recognition*, not only the render: eleven corpus sheets through live Java GRID, with staff geometry and all 392 promoted barlines -- shape, width, frozen, staff-end, median, intrinsic and contextual grade -- reproduced exactly, grades at 1e-9 rather than the example corpus's 5e-4 because it reads the live SIG rather than persisted three decimals. The sheets span the render regimes by construction (JBIG2 with and without shear, CCITT plain and inverted, the one Indexed three-band page, and one of the ten nearest-neighbour `ScaledBlit` pages), and four are sheets Java refuses for want of regularly spaced lines, asserted as refusals so the port cannot silently recognise what Java rejects |
| Native recognition entry | `audiveris_omr::recognize` runs LOAD-BINARY-SCALE, the GRID staff-line slice (run partition, horizontal lag, measure-then-cluster primary passes, staff candidates), and a per-staff `StaffProjector` producing graded bar peaks, all from production scale-derived parameters; `audiveris-cli -batch -step SCALE|GRID` prints reports. Chula is locked against a live Java oracle: slope 0.00792, 6 standard staves in 3 systems with system-1 indentation, staff extents within 3 px, and 58/58 Java barlines covered by projector peaks. The peak graph then adds Java `findAllAlignments` across staves, `buildBarSticks` registering one vertical filament per peak from the initial grid lags, and `findConnections` promoting alignments on the corridor gap/white-ratio test, from which staff systems are derived: system grouping matches the live Java oracle on all nine example pages, including single-staff and three-staff systems. The `BarsRetriever` purges then run per system (left-of-staff, unaligned, curved/short, width, C-clef, and column stages, after `purgeAlignments`), followed by `purgeExtendingPeaks` fed with each peak's bar-stick bounds. Each staff's opening bar is marked `STAFF_LEFT_END` as Java's projector does, taking the rightmost peak at or before the staff edge so a brace or bracket is never mistaken for it. **Barline totals match the live Java oracle exactly on all nine example pages**, and chula matches position by position on all 58 barlines across its six staves |
| Native GRID executor chain | `recognize_grid_lines` composes and runs the full ported decorator chain `ProductionCompleteLines -> ProductionProcessBars -> RawProductionRetrieveLines` through `HeadlessRasterGridBuilder`, as `HeadlessGridExecutor::from_completed_raw_bars_complete_lines` does. `retrieveLines` runs inside the chain and republishes its staffs, so `ProductionProcessBars` validates the separately derived `BarsSystemState` values against the ported line retrieval before purging. `ProductionProcessBars` gained Java's `purgeExtendingPeaks`, which the ported stage had skipped because `BarsSystemState` does not carry bar-filament bounds. All ten `completeLines` stages run on every example page |
| GRID completeLines parity | `rust/oracle/grid-completed-lines.txt` pins a live Java 5.11 (JDK 25) run's staff abscissae and completed staff-line endpoints for all nine example pages, captured from `StaffFilament`'s start/stop points right after the final `fillHoles`. **Do not diff against `sheet#N.xml`:** those points pass through `StaffLine.simplifyPoints` and one-decimal rounding, which shifts every ordinate about half a pixel and reads exactly like a systematic pixel-center bug. All 1300 endpoint components reproduce Java to the fixture's six-decimal precision. The six former exceptions were the JPEG decoder; the amplification path is worth remembering, since a single flipped binary pixel changed one line's member section count, changed its spline slope, and reordered the sort inside `Staff.getEndingSlope` -- which discards the extremes and averages the middle three -- landing the staff's ending slope 1.7x off |
| GRID barline parity | `rust/oracle/grid-barlines.txt` pins the same live Java run's surviving barline abscissae per staff for all nine example pages, read from each `StaffProjector`'s peak list after the full `BarsRetriever.process`. 420 barlines across 65 staves, all reproducing Java's abscissa exactly. The earlier corpus check compared only per-page totals, which a compensating pair of errors would satisfy -- and in fact did hide the one exception: `BachInvention5.jpg` staff 10's second barline is 745 against Java's 744, with the page total still matching at 46. That was the JPEG decoder, and it is now fixed: all 420 abscissae are exact. That staff also carries one of the completed-line endpoint residuals, and both look like one-pixel peak-refinement disagreements on the corpus's only JPEG. Open |
| GRID sheet SIG | `recognize_grid_lines` drives `HeadlessGridExecutor`, so the GRID step produces a populated sheet: staffs with recorded barlines, and a SIG of promoted barline and connector inters carrying median, width, intrinsic grade, contextual grade, frozen flag, and staff-end marks. `rust/oracle/grid-sig.txt` pins Java's persisted `sheet#1.xml` SIG for all nine pages. All 420 barline inters and all 184 connectors are promoted, matching Java's counts exactly, and **every median now matches Java exactly on every page**. Getting there took two wrong attributions before the right one. It was blamed on the JPEG decoder, which it survived; then on `createInters`, which reproduces Java's median formula exactly and reads `StaffPeak`'s `final` top and bottom, so the residual had to be a staff *line* residual. Instrumenting both runtimes at `createInters` showed `BachInvention5.jpg` staff 11 with two single-section stubs where Java had full-width lines. The ink was there and correctly clustered all along: `StaffCandidate` recorded only each line's *primary* filament id, and the projector resolved that id against the factory map, which returns the filament as it was **before** any cluster merge. A line seeded by a short fragment keeps that fragment's id while gaining the incoming sections, so the lookup returned the fragment and the projector read a flat line. The candidate now carries the cluster's merged line filaments and both consumers use them. That closed all seven median residuals across the corpus, dropped the grade residuals from 21 to 6, and took `BachInvention5.jpg`'s worst grade delta from 0.18 to 0.004. **The last six are now closed too, and every barline inter reproduces Java's intrinsic and contextual grade on every page**, so `SIG_PAGE_LEDGER` is zeros throughout. They were one wrong rounding mode: `StaffProjector.computeProjection` bounds each column by `firstLine.yAt(x)`/`lastLine.yAt(x)`, `StaffFilament.yAt(int)` is `(int) Math.rint(...)`, and `Math.rint` rounds a half to even where Rust's `f64::round` rounds it away from zero -- so an ordinate landing exactly on a half moved the projection's vertical bound by one row and its pixel count by up to one. All six were the leftmost or rightmost barline of their staff because that is where a line is extrapolated past its defining points, and an extrapolated straight line hits a half far more often than a spline through real ink. Three earlier hypotheses from source reading were all wrong; what settled it was `oracle/java/StaffImpactsProbe.java`, which prints the six staff-vertical impacts behind every promoted barline and showed that in all six cases exactly one integer differed by exactly one, across two different terms and both signs -- which points at the projection rather than at either consumer. Braces and connector medians/widths are still not compared, and are gaps rather than omissions. `SIG_PAGE_LEDGER` records the per-page counts as exact equalities |
| GRID processBars ordering | The SIG diff caught a real ordering bug. `process_bars_system` bundles the prefix, some purges, `partitionWidths`, and `createInters` into one call, so `createInters` ran before `purgeExtendingPeaks` and peaks that purge drops still received barline inters -- 50 promoted against Java's 46 on one page. `ProductionProcessBars` now runs Java's own staged order when the caller supplies the projector evidence: `process_bars_through_too_far_left`, `process_bars_after_braces`, `process_bars_peak_purges`, `process_bars_right_ends_and_c_clefs`, `process_bars_widths_and_inters`, `process_bars_connections_and_groups`, rejoined through `BarsCoordinatorResult::from_staged`. Callers holding only a `BarsSystemState` still get the bundled call |
| GRID staff limits | Java's three abscissa refinements are wired: the start column sets LEFT in the prefix, `verifyLinesRoot` may push it right on single-staff systems, and `refineRightEnds` sets RIGHT. All three now travel with the prepared bars handoff as `PreparedBarsSystem::staff_limits`, and `ProductionCompleteLines` adopts them before `defineEndPoints` pins each line ending at `staff.getAbscissa(side)`. **All 65 staff abscissae match the live Java oracle exactly on all nine example pages.** Two bugs surfaced getting there: `detectStartColumns`' blank test was called as `hasStandardBlank(staff_left, peak.start())` instead of Java's `hasStandardBlank(peak.getStop(), xLeft)` -- the range is directional, so the arguments are load-bearing -- and each system's `PeakGraph` was rebuilt from scratch with alignments only, dropping every `findConnections` promotion, which left `BarColumn::is_fully_connected` false for every column so no start column was ever found. The system graph is now the sheet-wide graph's induced subgraph, as in Java, which keeps one graph from `findAllAlignments` through `purgeAlignments` to `createSystems` |
| Floating-point parity | Java has been strict IEEE 754 everywhere since 17 (JEP 306) and rustc does not enable fast-math, reassociation, or implicit FMA contraction, so basic arithmetic and `sqrt` agree bitwise. Transcendentals remain an explicit compatibility surface. The accepted STEM_SEEDS oracle stores hexadecimal grades and exposed repeated one-ulp `pow` differences from the platform libm, so `audiveris-core::java_math::java_positive_pow` directly ports the narrowed OpenJDK fdlibm path used by positive weighted geometric means; all 2,003 checked stem grades match bit-for-bit. The classifier's ART LUT independently measured platform drift in `hypot`, `atan2`, `cos`, and `sin`, so `audiveris-classifier::art_math` now reproduces the narrow OpenJDK fdlibm and HotSpot AArch64 intrinsic paths actually used by the frozen Temurin 25 oracle. Full-domain operation hashes and all 12 frozen 110-value key-alter vectors match Java bit for bit. Other `Math.*` calls remain ported only when a measured oracle requires them |
| PDF ingest | Audiveris renders every PDF page through PDFBox with `renderImageWithDPI(page, 300, ImageType.GRAY)` under `ANTIALIAS_OFF` and `INTERPOLATION_BICUBIC`, so a native ingest reproduces a rasterizer rather than a set of decoders. Measured on real IMSLP sources, every page is a single full-page **bilevel** image (CCITTFax G4 or JBIG2, no `DCTDecode` in seven sampled files) resampled to 8-bit gray at a non-integer ratio, with all 256 gray levels present and 2-4% of pixels interpolated -- and those feed adaptive binarization, where one count flips a pixel and GRID amplifies it. The rasterizer was therefore built and verified first, before any parsing. **`crates/audiveris-pdf` now reproduces Java2D's bicubic image transform bit for bit**, across 7 geometries by 8 scales including the three real IMSLP ratios, upscale, downscale, identity and 1x1: 112 of 112 cases identical, pinned by `rust/oracle/java2d-bicubic.txt`. Five things had to be right and none is implied by "bicubic": the Mitchell-Netravali kernel at `A = -0.5`; a 513-entry table whose tail above 384 is derived so each group of four sums to one rather than evaluated; integer fixed-point arithmetic with a `1 << 15` bias and `>> 16` saturating store, which is the variant OpenJDK ships of three it carries; 32.32 coordinate stepping with a half-pixel subtraction before both gather and interpolation; and branchless sign-bit edge clamping. Destination pixels whose centre maps outside the source are never written. **Reading the file is also done.** `document.rs`, `lexer.rs`, `object.rs`, `filter.rs`, `flate.rs`, `ccitt.rs`, and `jbig2/` reproduce PDFBox on all 189 pages of the seven sampled IMSLP sources: page count, media box, crop box and rotation exact; image geometry, depth and filter chain exact; the raw stream bytes exact by hash; and every decoded filter chain exact by hash -- 93 CCITT G4, 95 JBIG2, 1 Flate. Everything is ported from PDFBox's and jbig2-imageio's own sources rather than the specifications, for the same reason the JPEG work targets libjpeg 6b: the target is the bytes Java produces, including the leniencies. `/Length` is often a lie and PDFBox scans for `endstream`; the CCITT decoder is TwelveMonkeys' as PDFBox vendors it, with three behaviours in neither T.4 nor T.6; `FlateDecode` keeps the prefix of a corrupt stream; JBIG2's arithmetic decoder reads -1 rather than 0xFF past the end of its data. JBIG2 scope was set by measuring what the corpus actually uses -- three segment types, no globals -- so Huffman, refinement, halftones and striped pages are refused by name rather than half-written. **Samples decode exactly too.** `raster.rs` is PDFBox's `SampledImageReader`: bit unpacking, `/Decode`, and the colour space, graded against the oracle's fourth depth -- `PDImage.getImage()`'s raster, hashed band-interleaved and row-major -- on all 189 images, 188 one-band gray and one three-band RGB. Scope was measured rather than assumed, as JBIG2's was: the corpus holds exactly four image shapes (177 one-bit `DeviceGray`, 11 the same with `/Decode [1 0]`, one 4-bit `Indexed` over `DeviceRGB`, no `/ImageMask`, no colour-key `/Mask`), and anything else is refused by name. Three behaviours are load-bearing: `from1Bit` returns a **one-band** `TYPE_BYTE_GRAY` image before any colour space runs, a row short of its stride ends the image and leaves the remainder black rather than truncating the raster, and the indexed palette round-trips every byte through `byte / 255f` and `(int)(x * 255f)` in `float` with a truncating cast. **The content stream and the draw transform are exact too.** `content.rs` interprets the page's operators and `affine.rs` ports `java.awt.geom.AffineTransform`, so all 189 draws reproduce the six-term transform Java2D receives at the oracle's full 17 significant digits, the sign of every zero included, and all 189 rendered page sizes match. The operator set was probed rather than assumed: exactly four operators over two page shapes, `cm Do` on 36 pages and `q cm Do Q` on 153, with anything else refused by name. Three float questions decide it. The CTM is a **`float`** matrix, so a `cm` operand of `633.5724` is really `633.57238769531250`. The DPI scale is a **`float`** division, so a 792 pt page renders 3299 pixels tall and not 3300. And `AffineTransform` is a **state machine** whose branches drop terms known to be zero -- which is a no-op for every double except `-0.0`. The page transform reaches `concatenate`'s scale-only case, computing `m10 = T10 * m11` with `T10` at `+0.0` and `m11` negative from the y flip, so Java's answer is `-0.0` on all 189 draws and a closed-form composition's is `+0.0`. That is why the state machine is ported rather than the algebra. `AUDIVERIS_PDF_CORPUS=/path cargo test --release -p audiveris-pdf --test corpus` prints `checked 189 pages, 189 images, 189 filter chains, 189 rasters, 189 draws; still unimplemented: {}`, and skips loudly without the corpus. **The page render is exact on all 189 pages, which closes PDF ingest.** `render.rs` composes the whole chain into a `TYPE_BYTE_GRAY` destination cleared to `Color.WHITE`, and the primitive-selection question is answered -- not by Java2D. `DrawImage.renderImageScale`, the only route to a `ScaledBlit`, returns false unless the interpolation hint is nearest-neighbour, so under bicubic no transform reaches it; the hint is what changes, and **PDFBox changes it** in `PageDrawer.drawImage` whenever `isScaledUp` holds, restoring the hints straight after (which is why an earlier probe saw `Bicubic` before and after the draw). The ported predicate selects exactly 10 of 189 draws, independently matching what `-Dsun.java2d.trace=count` counted. A second PDFBox path must stay off and is fragile: `drawBufferedImage` pre-scales through `Image.getScaledInstance(SCALE_SMOOTH)` below `imageDownscalingOptimizationThreshold` (default 0.5), but only when `KEY_RENDERING` is `VALUE_RENDER_QUALITY`, which Audiveris never sets -- if it ever does, the resampler changes and `transform.rs` stops describing the output. The ten scaled-up draws are done as well: `scaledblit.rs` ports OpenJDK's `ScaledBlit`, which is not a general nearest-neighbour resampler -- it re-derives the source origin exactly at every power-of-two tile boundary to bound fixed-point drift, and rounds with `ceil(x - 0.5)` rather than `floor(x + 0.5)`. `render.rs` also ports `DrawImage`'s `tryCopyOrScale` corner test, refusing the plain-`Blit` and sheared-nearest cases by name. The corpus's one three-band `Indexed` image is handled too, and its order matters: `renderImageXform` transforms into an `IntArgbPre` intermediate and only then blits to `ByteGray`, so each channel is interpolated in colour and the reduction to gray happens after, through OpenJDK's fixed-point luma `(77r + 150g + 29b + 128) / 256` rather than a colour-space conversion -- which is also why a gray source round-trips untouched. The whole result is exact on the seven measured sources rather than complete, with every unmeasured shape refused by name. HANDOFF.md section 2 carries the detail. PDFBox does **not** subsample -- but the naive "scale source to destination" model is wrong by 22.5% of pixels: a real IMSLP page places its image at `515 0 0 633.5724 45 74.2138 cm` on a 595x792 pt page, so it does not fill the page, the render carries white margins, and the actual scale is a 0.927 *downscale*. PDFBox also computes its DPI scale in `float`, which is why a 792 pt page renders 3299 pixels tall and not 3300. `scale_bicubic` therefore needs generalizing to an affine placement, following OpenJDK's `xbase`/`dxdx` derivation, and verifying against a real page |
| `.omr` persistence | opaque round trip plus lossless typed `book.xml`/per-sheet views, explicit stub/member states, pipeline status, sheet input provenance, compatibility attributes, page references and links, order-derived systems, part/staff configuration, logical parts, score-root metadata, sheet selection, legacy beam/OCR metadata, and book interline/beam/OCR/lyrics parameters; absent, inherited, and explicit values remain distinct |
| Visual classifier core | native immutable parser/inference for the frozen bundled 110→149→149 `BasicClassifier` model, including Java normalization, bias-first sigmoid layers, point-list `MixGlyphDescriptor` ART/geometric/aspect extraction, and Java-order native RunTable adaptation; the ART LUT now reproduces the frozen Temurin 25 `Math.hypot`/`atan2`/`cos`/`sin` bits, and all 12 frozen key-alter vectors match Java at all 110 inputs. `rank_evaluations` ports Java's `AbstractClassifier.evaluate` ranking and minimum-grade policy, including the stable sort that keeps model-label order on equal grades, and `glyph_factory::build_glyph_components` builds glyphs from runs. What remains is Java's glyph-size/noise gate, `ShapeChecker`, user overrides, and MusicFont metrics, plus wider recognizer integration. **MusicFont is deferred, not dropped** -- the port targets full parity -- and it is the clef *geometry* that needs it rather than the classification: `getSymbolBounds` needs `TextLayout.getBounds()` and `ShapeSymbol.computeCentroidOffset` rasterises the glyph to an antialiased alpha image for an alpha-weighted centroid. That is Java2D's native, hinted rasteriser, which unlike the bicubic transform is not fully specified in Java source, so it may end up graded against a stated tolerance rather than a hash. It sits under HEADERS' clef geometry and under all of HEADS. See HANDOFF.md for the measured price: on chula the header erase it enables is worth 5 spurious clef-sized beam candidates out of 100 and costs zero real beams |
| HEADERS recognition stage | `recognize_native_headers` is the production GRID-only composition of `HeaderBuilder` plus native clef, key, and time sourcing/lifecycles in Java system order. It derives system/staff/part and bar-group state, header starts, specific interlines, and ordered good-connected browse bars from GRID; applies exact ranges, proposal ordering, pitch maps, grade/context selection, exclusions, stop propagation, cleanup, and ownership; and returns typed candidates, selected IDs, final headers, system time values, and beam `HeaderErase`s. The nine-page gate calls this entry point before reading Java records and matches all 65 staves, 34 selected keys, 17 selected times, and 30 erases. Missing geometry and nested classifier/run-table/column failures remain typed errors rather than zero defaults |
| STEM_SEEDS recognition stage | `recognize_native_stem_seeds` composes completed GRID and oracle-free HEADERS state through production lag selection, the vertical `StickFactory`, closest-staff/header gates, the concrete `StemChecker`, original fixed-glyph materialization, the minimum-grade decision, `VERTICAL_SEED` grouping, and system free-glyph ownership. All 30 selected vertical/sticker vectors match Java by full digest; all 2,425 raw candidates match bit-for-bit; and the next boundary is exact for 422 header skips, 2,003 checked candidates, 97 rejects, and 1,906 accepted/materialized seeds. The gate compares staff/header decisions, seven raw values, weights, normalized impacts, Clean counts, bit-exact aggregate grades, glyph bounds/weight/line geometry, and every cropped run table by count and digest. Java's unusual thickening mutation remains preserved. A narrowed OpenJDK fdlibm positive `pow` port closes the one-ulp grade differences that platform libm produced. Schema 1 publishes accepted seeds in production `{system, ordinal}` order with geometry, grade, exact check evidence, run count, and hexadecimal digest; no process-global glyph or SIG ID is invented. `native_stem_seeds_for_beams` validates system order, decision/free identity, unique ordinals, group/free flags, and Java-int bounds before adapting exact medians into BEAMS; all 1,906 corpus seeds convert in production order, and the BEAMS/LEDGERS CLI now consumes and republishes them. Profile 1 is the only measured profile, with no tablature or no-staff skip case |
| Beam spot chain | `SpotsBuilder.buildSheetSpots` is eight transforms deep before a single beam is considered -- stem-run removal, a median, a gaussian, a header erase, the morphological closing, a threshold, a run table, connected components -- and Audiveris caches only the last. **All eight match Java bit for bit on chula**, pinned by `rust/oracle/beam-spots.txt`: five buffer digests, two thresholds, the run table, and all 305 spot glyphs by bounds, weight and rounded centroid. Production BEAMS now also preserves the same closed buffer's threshold-170 vertical `HEAD_SPOTS` table before applying the threshold-140 BEAMS table; its Java table size, run pixels, and independently thresholded pixels match on all eight pages. Graded end to end any one of the eight failing would look exactly like the other seven failing, so `SpotsProbe` drives SpotsBuilder's own private methods reflectively and dumps each as it comes out; it checks itself by emitting `getBuffer`'s digest beside the same three stages driven one at a time. This also retired a quiet risk: `median.rs` had been in the tree since GRID and had never been graded, because nothing had reached the step that uses it. New are `gaussian.rs`, `spots.rs`, and a length filter on `RunTable::from_pixels`. Two things about the gaussian are load-bearing and neither is implied by "gaussian blur": its sigma is pinned at 1 regardless of radius, so the radius only decides where the kernel truncates -- three taps at Audiveris's radius of 1 -- and every step is `f32` including the accumulator, because the taps are normalised in `f32` and so do not sum to exactly one, and that residue decides which side of a rounding boundary a pixel lands on. `Math.exp` was the one place a 1-2 ulp difference from Rust's libm could have shown; it did not. The header erase is the rung that reaches outside BEAMS -- it needs `Staff.getHeaderStop`, which HEADERS produces and the port does not have -- so the rectangles are an explicit input and the oracle pins the closing both with and without them, which prices the dependency instead of hiding it: without it the closed buffer is `51039d31f0b6a48b` where Java has `f646e28702d82c1b` |
| Grayscale morphology | `audiveris_image::morphology` ports `StructureElement`'s circular element and `MorphoProcessor::close`, which is the first thing BEAMS does to a page and the one piece the port had no module for at all. **Bit-exact against Java**: twelve structuring elements cell for cell and offset vector for offset vector, the closing over two generated buffers at six radii, the 24x16 pair pixel for pixel, and the closing over chula's 4.8-million-pixel NO_STAFF page at three radii including the 4.3 that `SpotsBuilder` derives from its beam thickness of 12. Pinned by `rust/oracle/morphology.txt`, generated by `oracle/java/MorphoProbe.java`. The gate is a *unit* one, which the earlier scoping had assumed was impossible: Audiveris caches the closed buffer nowhere -- `Picture.SourceKey` has no spot entry and the image is a local in `buildSheetSpots` -- so `oracle/beams-chula.txt` grades it only through the beams it eventually produces, which cannot tell a wrong element from a wrong recogniser. Calling `MorphoProcessor.close` from the probe instead sidesteps the cache entirely. Nothing needed fixing on the Rust side; every oracle assertion passed on the first run. The arithmetic is stranger than the result: `getMinMax` adds 255 to every sample before taking a maximum, masks to a byte, and the caller subtracts 255 again, and the two cancel exactly -- but the cancellation depends on the padding, where an out-of-bounds sample contributes 0 to a dilation and 255 to an erosion, which is what keeps a closing from eating the page border. Java's two interleaved loops, which run the erosion a whole element behind the dilation, are reproduced rather than collapsed into two passes, with a test pinning that they agree so the collapse can be done later against a test rather than an argument. Only the circular element and `close` are ported: the other shapes and the histogram-based `fclose`/`fopen` are unreachable from Audiveris. The oracle also pins the four buffers `SpotsBuilder.getBuffer` passes through -- stem-run removal, median, gaussian, and the closing of all three -- which morphology does not need and the rest of the step does |
| BEAMS complete | **The step's whole output is reproduced exactly on chula: 91/91 beams, 31/31 hooks, 60/60 beam groups, and nothing spurious in either direction.** All four stages are wired -- `createBeams`, `extendBeams`, `buildHooks`, `BeamGroupInter.populateSystem` -- and each is graded against Java's own SIG at the end of the step rather than against an intermediate. `buildHooks` supplies the 11 hooks `createBeamInters` does not: Java runs it over the spots that produced *no* beam, so a spot `checkBeamGlyph` refused is still a hook candidate, and the overlap test runs against a `rawSystemBeams` list that grows as the pass adds to it. Grouping is per system, which is load-bearing -- run globally over the page it merges beams across a boundary Java never compares, and 60 groups become 48. `recognize_native_beams_with_stem_seeds` supplies every accepted per-system seed to `extendToStem` in Java stage order, while the old entry point remains an explicit empty-seed compatibility wrapper. `oracle/beam-stem-seeds.txt` is the independent production counterfactual: every page and mode runs in a fresh JVM; hiding only `VERTICAL_SEED` visibility after the real STEM_SEEDS step changes none of 803 final beam/hook inters, 493 groups, or the one multiple rest across the original 30 systems. Seeded and hidden 30-row states are byte-identical across two passes (SHA-256 `acca06864acfb212ea690b05987ab662668a2b2bf5fb6d4c86a26f32681fc6bf`; fixture SHA-256 `283490cf3dc06afd7b65d3c8ca7c956b6e2b0372d43a0615edf89df469c8d785`). `oracle/beam-stem-seeds-d039.txt` closes the acceptance branch on a natural page: D039 system 2 replaces exactly one unextended beam with its seed-extended form, while all other beams, hooks and group counts stay fixed; median, height, grade, and all six impacts compare by exact double bits. Matching it required Java `LineUtil.intersection`'s determinant operation order and the existing OpenJDK-compatible positive `pow` path in beam grading. The focused fixture SHA-256 is `991f3b4c56d4e9b5bb466657bffe931d6d0736daf759dd010964c82b01853f18`, with paired summary FNV `5acbd8b3dd4d1405` |
| BEAMS beam creation | `beam_inters.rs` is `createBeamInters`: the jitter impact, the six-term grade, and the hook/beam pair each item can produce. **All 111 raw beams on chula match Java exactly** -- class, shape, median, height, all six impacts and the grade, to nine decimals -- and, more to the point, **every one of the 91 beams Java's BEAMS step finishes with is produced here**. `oracle/beams-chula.txt` is the SIG three stages downstream, after extension, hooks and grouping, so agreeing with it from per-spot recognition alone is the real measure of whether the recogniser is right. The remaining gap is exactly `buildHooks`: 20 of Java's 31 hooks come from here and the other 11 are added later, and the test asserts that count rather than tolerating it. Three details were load-bearing. Impacts are **clamped to [0, 1]** by `GradeImpacts.setImpact` before the grade is taken -- an item wider than the *hook* thresholds expect yields a width impact of 1.79, and without the clamp 110 of 111 grades came out plausible and too high. The jitter is computed **once per structure** from the outermost lines and shared by every item in it, not measured per item, and it uses the *integer* section centre where `retrieveItems` uses the `double` one. And Java runs `createBeams` once per system over that system's own spot group, so a spot straddling two systems is recognised twice and lands in two SIGs -- which is why 99 distinct beams are Java's 111 |
| BEAMS spot recognition | The chain from a spot glyph to a beam structure is native and graded per spot against `rust/oracle/beam-structures.txt`. `beam_parameters.rs` derives `BeamsBuilder.ItemParameters` and `Parameters` -- **all 28 scaled constants match Java exactly**, which matters because the beam kernel already in the tree took them as inputs and had only ever been given test values. Two scalings are in play and they differ: `toPixels` rounds with `Math.rint` and `toPixelsDouble` does not round at all. `maxExtensionToSpot` is 0.5 of a 21-pixel interline, exactly 10.5, so it is 10 under `rint` and 11 under `round` -- one constant on one page is enough to prove the helper. `beam_recognizer.rs` is `checkBeamGlyph`: the six named refusals Java applies before any structure exists, plus `getWidth`, `extendMiddleLines`, `Glyph.getMeanThickness` and `Glyph.getCenterLine`. **All 305 spot verdicts on chula match Java**, across five distinct refusal reasons over 211 refusals, and so does every measurement each threshold tests -- mean thickness, mean border distance, structure width and slope gap, to nine decimals. Of the 107 spots that produce a structure, every border line and every item matches except **four values on three spots**, pinned exactly in the test rather than tolerated. Those four are one bug: three are a last-digit difference in a border median, and the fourth is that difference deciding a containment test, so the port's item starts one section late at x=500 where Java's starts at 499. The arithmetic is not the cause -- Java's sums are over integer border points and so are exact in `f64` whatever the order, and the remaining `hypot` and division are worth a couple of ulps, three orders of magnitude below what is seen -- so the point *set* differs by a point, which puts it in `border_lines` and its section purge. The probe already dumps each section's polygon for that reason: `retrieveItems` asks whether a section's centre lies on the median, and Java asks that of the section's **polygon**, whose outline puts a run's far edge at `stop + 1`, not of its bounding box |
| BEAMS recognition stage | headless lifecycle, concrete morphology/threshold/run evidence, native connected-component glyphs, system dispatch, candidate ordering/retry/extensions/group orchestration, multiple-rest replacement, BeamStructure border/core/belt impacts, hook/group/extension/serif evidence; remaining seams are classifier/materialization and listed raster-geometric internals |
| GRID → HEADERS → STEM_SEEDS → BEAMS composition | `recognize_native_headers` consumes GRID alone; `recognize_native_stem_seeds` consumes GRID plus those headers; and `recognize_native_beams_with_stem_seeds` consumes their accepted free glyphs plus the exact header erases. BEAMS measures the graded uncleaned-NO_STAFF `maxStem`, runs the spot chain, dispatches through system areas/bounds, and creates, extends with all per-system seeds, hooks, and groups beams. Production now retains every system's group creation and member relation-insertion order, with member ordinals indexing raw beams then hooks; legacy counts and schema output are unchanged. The chained corpus gates cover all eight beam sheets: 2,739 spots, 30 erases, 1,906 seeds, and 787/787 raw beams exact by system, geometry, grade, and all six impacts; final beams/hooks and per-system group counts now match after Java's one Bach source beam is removed by `MultipleRestsBuilder`. D039 proves the retained system-2 memberships change even when the group count does not. The production MultipleRest adapter preserves SIG order and Java's inclusive minimum-length, ±0.2 pitch, staff/tablature, NaN, and two-serif gates; recomputes the fresh BEAMS-time `StaffProjector` from completed persistent splines plus original BINARY; and retains exact pre/post beam state plus an identity-free descriptor. Bach system 6 source ordinal 182 matches frozen median, grade, height, staff, pitch, and serif evidence bit-for-bit. Stable MultipleRest/serif/glyph/relation identity allocation remains the graph-materialization seam. Java records are read only after recognition for grading. The legacy BEAMS function remains an explicit empty-seed compatibility wrapper. A measured small-beam scale errors explicitly because no corpus sheet grades that class. |
| LEDGERS recognition stage | complete native composition from GRID `NO_STAFF`, oracle-free HEADERS/BEAMS, curved staff/system geometry, and BEAMS' post-MultipleRest beams plus hooks through raw zones, all gates/seven impacts, overlap reduction, glyph/SIG materialization, exclusions, staff ownership, sheet-wide `LedgersPostAnalysis`, and recursive external ledger-line construction. All 581 final Java inters and 95 inferred ledger-line paths on the eight beam sheets are exact; inters match by system, staff/index, median, thickness, seven impacts, and grade, while lines match by staff/index and cumulative curved-path translation. Every final non-removed inter now retains a 1:1 exact positioned fixed glyph: referenced filtered horizontal sections are painted into minimal bounds using Java's width-versus-height orientation rule, with no median reconstruction. Chula's trace is 9,915 filtered runs, 4,052 sections, Java-exact system dispatch counts of 2,042/591/961, 104 horizontal StickFactory filaments, 19 builder survivors, one statistical reject, and 18 final inters. Rebuilds preserve Java's removed-inter tombstones. Schema-1 serialization and CLI wiring are published; broader grading remains |
| HEADS recognition stage | dependency-light lifecycle, native prolog, transient spot dispatch contract, ordered classifier mutations, glyph/inter/SIG/staff ownership, checked/fatal prefixes, cleanup, and quorum scale implemented. `recognize_native_heads_prolog` composes live upstream products: GRID's original binary, persistent staff lines and curved system areas; BEAMS' retained threshold-170 `HEAD_SPOTS`; LEDGERS' exact final fixed glyphs; and STEM_SEEDS' accepted free vertical glyphs. It validates system/staff/ledger/seed ownership and order, builds the BINARY erasure and Chamfer-3 table, extracts transient components in `GlyphFactory` order, and applies area plus inclusive horizontal dispatch. `oracle/java/HeadsPrologProbe.java` independently drives Java through LEDGERS and calls the actual distance/spot builders in source order; `oracle/heads-prolog.txt` freezes 55 staves/275 persistent line glyphs, 581 final ledgers, 1,906 accepted seeds, 2,790 full components and 3,097 references across 30 system dispatches on eight pages. Two all-fresh-JVM passes are byte-identical (SHA-256 `31e6166b0e2e8e7ae38909cca31d0a1709f8acc40f2812727509ea0bfb0a8422`). Rust now matches every HEAD_SPOTS table, post-erasure BINARY mask, signed-i32 distance value, complete cropped component RunTable, and dispatch ordinal exactly. The sole first-run mismatch was Bach component 693 at x=1916: `SystemInfo.updateCoordinates` stores `width = right - left + 1` and `getRight()` returns `left + width`, one beyond the maximum staff abscissa. `SystemBounds::java_right` now centralizes that convention for BEAMS, LEDGERS, STEM_SEEDS, and HEADS. BEAMS now runs the native `BlackHeadSizer` on its actual threshold-140 components before dispatch, retains exact source/closed evidence and populations, derives the Bravura music-font scale, and publishes all per-staff head point sizes to downstream Rust. The eight-page gate matches every one of 2,739 inputs and decisions, 936 singles, 5 stacks, 470 core samples, every population bit pattern, and all 55 staff point sizes; the fixture SHA-256 is `49408a3fc31857f107efb65ead37f63fd2e6dfe159f3fdd6215c89ed233199a9`. One-line/drum switch suppression remains ungraded. The template oracle freezes five actual point sizes, four active normal-staff shapes, 192 anchors, and 27,207 exact keyed pixels (SHA-256 `84c39208891530965f5d9ce71ff9b79cf373c101f4da8036059cdbf25e2a2ea6`). The native model/evaluator preserves Java's geometry, anchor rounding, factory order, 6/1/4 weights, UNKNOWN/bounds skips, comparison, accumulation, and empty fallback. A deterministic generator deduplicates the records into a 105,021-byte production asset containing 20 unique templates, 120 anchors, and 17,094 keys; its strict decoder consumes `include_bytes!`, never oracle text or font rasterization. The exhaustive gate expands all eight page catalogs and matches all 32 templates/192 anchors/27,207 records. Production HEADS maps the retained point sizes by `(system, staff)`, recomputes them from the music-font scale, skips only tablature, and retains all 55 exact catalog selections in Java order. The scanner oracle (SHA-256 `c137725c110755229c6b693410077b8c1933d7d70b63ed49dd7b3330a385d886`) freezes all 1,767 scanner geometries and 3,534 seed/range schedules across those 55 staves. Production now composes exact persistent-line splines and fixed-ledger glyph axes, staff/index ledger ownership (including reuse and x order), HEADERS stops, catalogs, and builder parameters. The exhaustive gate matches every parameter, source, raw axis bit, farther-ledger list, open flag, x/y offset, ordered shape set, source/range bound (including four inverted empty ranges), and every full/range theoretical ordinate. It exposed that Java ledger endpoints use `Glyph`'s uncentered `BasicLine.toDouble`, not `getCenterLine`, and the native port now carries both explicitly. Part ID/range metadata is not claimed: production GRID still bypasses brace detection, while all graded scanner staffs are non-merged, so the operational merged flag matches but brace ownership awaits the upstream GRID slice. A separate fresh-JVM base-slice oracle (SHA-256 `82d87324be1d2eef2a14be4c8cc68be332e9f76311eeb4b6dedd1c74d3c96ee3`) freezes all 1,334 competing-shape candidates and their three rejection branches, 847 accepted competitors, 533 bar/connector candidates, 474 frozen bars, exact semantic Area bands, and the ordered seed/spot/competitor/bar rectangle slices for all 1,767 contexts. It proves final BEAMS state is required: Bach system 6 replaces one source beam with the corpus's MultipleRest before HEADS. The native slice kernel now ports Java's signed/overflowing half-open `Rectangle`, curved staff/ledger bands with exact extrema and `below + 1`, straight vertical-ribbon bounds, empty-area behavior, and source-order frozen-area intersection. Production now materializes all 1,906 ordinate-sorted seed rectangles and 3,097 head-spot rectangles, builds all three semantic bands for every scanner, and matches the entire eight-page seed/spot differential: 1,455 nonempty seed slices with 15,343 references and 1,455 nonempty spot slices with 6,759 references. A separate GRID adapter matches 528 source-order bar/connector candidates and all 474 frozen obstacles by class, shape, staff, median/thickness bits, stable ordinate order, and Java integer Area bounds; the oracle's other five candidates are unfrozen Hove `DUMMY_BARLINE`s injected later by HEADERS and never enter the consumed frozen pool. Frozen-bar composition matches all 1,767 scanners exactly: 552 slices are nonempty and retain 5,060 references in Java's stable-by-ordinate pool order, with no x sort. The production competitor adapter reconstructs and filters all 1,334 live GRID/BEAMS/MultipleRest/serif candidates in Java SIG order, then stably sorts all 847 acceptances by ordinate. The gate pins class, shape, staff, bounds/Area, median/thickness/height bits, intrinsic and best grade bits, `isGood`, frozen state, maximum-stem and minimum-beam thresholds, vertical floors, beam-group member widths, every decision, and accepted ordinal. Replacing the remaining staff/bar `powf` calls with the existing OpenJDK-compatible positive-power kernel closed the last one-ULP GRID grade seam without tolerance. Final competitor slicing also matches every scanner: 408 slices are nonempty and retain 1,944 accepted-pool references after semantic Area intersection and Java's stable abscissa sort. The normal-staff corpus now continues through exact seed/range evaluation, glyph materialization, dynamic head competitors, pair predicates, staff purge/attachment, contextual beam arbitration, and final tally analysis. Stable MultipleRest/serif graph identities, drum/one-line shape maps, and the unavailable tremolo/small-beam/small-hook producers remain explicit wider-scope seams rather than fabricated state; the MLP classifier is not part of HEADS |
| HEADS template lookup primitives | exact integer full/slim bounds at every anchor, Java wrapping coordinate arithmetic and `Rectangle.translate` overflow recovery, hole-only distance-table evaluation, and clamped `Template.impactOf`/one-impact `HeadInter` grade conversion are native and focused-tested. Semantic bar/competitor Area overlap, seed/range orchestration, glyph materialization, and the complete epilog are composed below |
| HEADS seed-pass oracle | a fresh JVM per page drives the real `NoteHeadsBuilder.processStaff(staff, true)` path, appends each staff's returned heads to the live competitor pool in production order, and stops before range lookup. The compact fixture hashes all 61,372 seed/side/shape searches and retains all 3,435 provisional candidates plus all 3,435 final glyph-backed heads over 55 staves/30 systems/8 pages. Two complete regenerations are byte-identical (SHA-256 `aca3cd20941846ae0eab9b4c1e56b3c9959afb6ed649519888b854e2b68f0414`); `--full-trace` retains every diagnostic search row. The native seed path is exact through glyph materialization and tally storage |
| HEADS seed lookup | Java2D positive-area overlap for the current straight vertical ribbons and horizontal parallelograms is native, including edge/corner-only exclusion, slopes, negative extents, degenerate/non-finite paths, and non-wrapping double rectangle maxima. `recognize_native_heads_seed_lookup` composes every retained HEADS prerequisite through Java's seed/side/shape and y/x loops, first bar/competitor gates, strict best replacement, nominal abandon, black-to-void hole check, minimum-grade conversion, and provisional tally dx. The exhaustive eight-page differential matches all 55 staff kernel hashes, 61,372 ordered searches, outcome/performance counts, and all 3,435 provisional candidates at raw-bit precision. The glyph compositor then matches every final seed head and tally without fabricating Java identities |
| HEADS seed glyphs | the pure `retrieve_head_glyph` port expands a provisional slim box to the full template, visits zero-distance keys in factory order against the original BINARY raster, applies Java wrapping coordinates and in-image gates, crops inclusive foreground bounds, and creates the same positioned vertical RunTable before replacing inter bounds. The production compositor validates catalog/topology/provenance, drops null retrievals, and retains dense Java return order without allocating process-global IDs. All 3,435 corpus candidates survive exactly; final shape/pitch/grade/impact bits, provisional and final bounds, glyph weight/run digest, good decision, and post-retrieval side tally match every Java `headseedhead` row |
| HEADS range-pass oracle | `oracle/java/run-heads-range-pass.sh` drives the real seed then range halves of `NoteHeadsBuilder` for every non-tablature corpus staff and stops before duplicate/overlap purge. The compact fixture retains 6,759 ordered spot slices, seed heads, 3,550 post-aggregation candidates, all 174 final range heads, and per-staff identity-free hashes for 921,558 scan positions, 3,119,882 template attempts, and 34,101 raw candidates. It records 5,389 safety skips, 3,376 seed conflicts, and zero empty-glyph drops across 55 staves/30 systems/8 pages. Two fresh-JVM generations are byte-identical (SHA-256 `35a8d063d557979b9d5e948c279a6228c42ffd3fb5a7784d236779b490740770`); `--full-trace` expands the three hashed diagnostic classes. The entire native range path is exact through glyph materialization |
| HEADS range lookup | `recognize_native_heads_range_lookup` composes the exact prolog, scanner geometry, template catalogs, transient spot components, frozen bars, and accepted non-head competitors through Java's range half before aggregation. It reproduces `Rectangle.grow` spot relevance, Chamfer-3 safety skips/jumps, black-versus-hollow shape choice, MIDDLE_LEFT y-only evaluation, overlap gates, strict best replacement, nominal abandonment, black-to-void conversion, weak stemless rejection, minimum-grade construction, and compact online hashing without retaining 3.1 million attempt records. The permanent eight-page differential matches every staff's spot/scan/attempt/raw-candidate hash, scanner count including four empty inverted ranges, outcome/performance partitions, and exact totals of 6,759 spots, 921,558 scans, 3,119,882 attempts, and 34,101 provisional candidates. Five focused tests pin Java rectangle growth, clipped relevance, stemless thresholds, FNV chaining, and hexadecimal double formatting |
| HEADS range post-processing | `head_range_postprocess` ports Java's pure `aggregateMatches`, `overlapSeed`, and `filterSeedConflicts` contracts without allocating interpretation identities. Aggregation uses stable reverse `Double.compare` grade order, fixes each aggregate at its first member's `center2D`, and chooses the first inclusive-`maxTemplateDx` group. Conflict filtering preserves Java `Rectangle.intersects`, `GeoUtil.iou` signed `int` overflow, non-wrapping `getMaxX`, first-match and early-break behavior, and inclusive 0.1 IoU/grade-margin gates. Nine adversarial tests pin ties, NaNs, signed zero, group order, threshold equality, malformed order, negative rectangles, and area overflow. A strict compact-fixture differential additionally validates aggregate ordinal/main/member invariants and the exact 0..34,100 raw-member partition, then replays every one of 3,376 first seed conflicts and 174 retained candidates with SIG provenance, bounds, grade, and independently calculated IoU bits. The compact schema omits nonqualifying scanner-Area membership, so it deliberately does not claim to replay raw aggregation or the complete competitor slice |
| HEADS range glyphs | `retrieve_native_heads_range_glyphs` composes raw candidates independently per scanner, reconstructs Java's live base-plus-current/prior-staff-seed competitor pool, intersects good competitors with the retained exact curved semantic band, and stably x-sorts the slice. It applies native aggregation and conflict filtering, records every qualifying seed with live/slice provenance, retrieves original-BINARY template glyphs, and preserves compact/final dense order without fabricating Java identities. The eight-page gate matches all 3,550 post-aggregation candidates, their aggregate main/member provenance, all 3,376 conflict drops, zero glyph-empty drops, and all 174 final range heads by raw source/attempt, shape, pitch, grade/impact bits, provisional/final bounds, glyph weight/run digest, good decision, and order |
| HEADS post-range oracle | a separate fresh-JVM probe follows `NoteHeadsBuilder.buildHeads` after real seed/range lookup: full-abscissa sort, production duplicate removal, overlap-exclusion insertion, tally purge, stemless boost, idempotent staff attachment, system small-beam arbitration, then `HeadsStep.doEpilog` image discard and sheet tally analysis. HEADS has no linking operation. The compact fixture freezes 3,609 inputs, 62 duplicate removals, 2,725 overlap exclusions, 3,547 post-duplicate staff heads, all 191 small-beam inputs, 26 ordered arbitration decisions, zero purged beams, 26 beam-defeated heads, 3,521 final heads, 1,451 live scale inputs, and 18 exact scale rows over 55 staves/30 systems/8 pages. Hidden ordered inputs, 15,336 staff pair checks, tallies, and 10,053 beam checks are count/hash committed and available with `--full-trace`. Two fresh-JVM runs are byte-identical (SHA-256 `e893c2327a9afa937035559f1a5be170a22148dd6655e8ffb6297c75bff5f6ba`, body SHA-256 `1420841aaeaafecb07664acbc26b752f3c7154fec073d863170c9ed77a1628f7`) |
| HEADS staff purge kernel | `head_purge::purge_staff_heads` ports the shared full-abscissa nested loop once for duplicate removal and overlap-exclusion insertion. It preserves wrapping x/y comparator subtraction, relative Java-ID tie order, stable equal keys, positive-area Rectangle gating, wrapping inclusive xMax, strict `EPSILON`, NaN comparison behavior, removed-state skips, left-loop continuation, and `purgedEquals` seed-tally preference and complementary-tally replication. Twelve adversarial tests cover those branches. `head_pair_predicates` supplies the complete Java `AbstractInter.isSameAs`, `Glyph.isIdentical`, `Rectangle.intersection`, and `HeadInter.overlaps` semantics, including exact run tables, staff identity, `Math.rint` pitch, inclusive width gates, strict area gate, integer overflow, and NaN behavior; nine tests pin the boundary cases |
| HEADS small-beam purge | `head_small_beam_purge::purge_small_beams` ports `NoteHeadsBuilder.purgeSmallBeams`: all four beam shapes, strict integer width selection, SIG-order beams, stable ordinate-only head sorting, exact filled-parallelogram intersection, wrapping inclusive bottom, strict grade arbitration, NaN/equality else branch, and iterator removals that affect later scans. The production adapter now consumes live competitor, beam-group, MultipleRest, and head records. It recomputes Java contextual grades dynamically with coefficient 3 / ratio 4, raw hook-beam exclusions, reverse-grade compatible partitions, and both prior beam removals and removals made during arbitration. BEAMS retains the fixed vertical glyph that Java rebuilds from `NO_STAFF` inside each final raw-beam/hook parallelogram, including extension and merge products; all 191 consumed glyph bounds, weights, and run digests match the compact oracle. The complete gate matches all 191 beam inputs, 10,053 ordered checks by exact per-system hash, 26 head removals, and zero beam removals |
| HEADS post-range corpus reader | `parse_heads_post_range_corpus` provides a typed, identity-free model of every retained purge, attachment, beam, final-head, scale, and summary record. It validates complete/body SHA-256, reconstructible FNV category hashes, count arithmetic and ordinals, tally rows, and the staff-head to purged/final-head multiset transition. Production now recreates the compact initial-head and pair-check streams from live products and matches their committed summaries |
| HEADS tally analysis | `analyze_head_seed_tallies` ports `HeadSeedTally.analyze` as an identity-free pure kernel: ignore removed entries, group by Java shape enum and LEFT/RIGHT side order, preserve insertion-order binary64 addition in each Population, apply the inclusive quorum of 10, and emit shape-then-side scale entries. Four adversarial tests pin quorum, removed filtering, enum order, non-associative accumulation, and signed zero. The compact Java differential replays all 1,451 surviving samples across eight pages and matches all 18 emitted mean dx values by raw bits |
| HEADS complete native recognition | `recognize_native_heads` owns and retains every production boundary from live GRID, HEADERS, STEM_SEEDS, BEAMS, and LEDGERS through prolog, scanners/pools/slices, seed/range lookup and glyphs, complete epilog, and tally analysis, with an explicit error variant for every fallible stage. The eight-page top-level differential now calls this entry point directly and is exact for 3,609 inputs, 62 duplicates, 2,725 overlaps, 3,547 post-duplicate heads, 191 beam inputs, 10,053 ordered checks by exact per-system hash, 26 removed heads, 3,521 finals, 1,451 tally inputs, and 18 scale rows. Final heads retain explicit `is_vip` evidence; the current native creation path emits false, and future true inputs must be handled or rejected explicitly. `-step HEADS -json` publishes the existing schema-1 result plus all upstream products without fabricated Java identities. HEADS is native, graded, and published; the first semantic STEMS boundary composes it directly |
| STEMS head-corner boundary | `materialize_native_stems_head_corners` is the first production semantic STEMS compositor. It consumes the complete owned HEADS result and live STEM_SEEDS system parameters, removes beam-defeated heads, preserves the surviving stem-capable heads in SIG insertion order, and exposes Java's stable abscissa and reverse-grade index permutations without allocating Java IDs. For every head it resolves the real staff-selected Bravura catalog, reconstructs `Template.getBounds`, rounds the four `CLinker` anchor offsets, applies the analyzed LEFT/RIGHT head-seed correction, and derives exact profile/interline inside/outside limits. `StemsHeadCornerProbe` drives real HEADS in a fresh JVM per page and freezes the identity-free boundary immediately before `retrieveStump`; two eight-page runs are byte-identical. The exact differential matches 30 systems, 3,521 heads, all 14,084 constructor-order corners, every SIG/x ordering ordinal, head/template/glyph field, and every uncorrected/reference/outside/inside double bit. Probe source SHA-256 is `4180de0596c3580fbef45ee12b6ec05f0dee17ef9e7267531e62efabb28d9c40`, emitted-body SHA-256 is `485544ae74a08d2a4d5c2a0de0030db67eec0086bd370d4eb6e2680917d0572a`, and the complete fixture SHA-256 is `26f9fff81c6207957dab6f42bf7d1650682ae9fca5de46e7b9a7dc46f20fd94b`. Existing-seed retrieval and no-stem purge continue in the following graded boundary; section-built stump materialization, linker geometry, glyph registration, and SIG mutation remain |
| STEMS no-stem purge and existing-seed selection | `materialize_native_stems_head_seeds` continues from live GRID, STEM_SEEDS, and head-corner products to the last read-only boundary before `CLinker.buildStump()`. It reconstructs Java's top/bottom/middle connected-bar ribbons in stable minimum-x order, purges the free-glyph pool in insertion order using `Area(seedBox) ∩ noStem` bounding-box ratios, derives each fixed glyph's exact run-table `BasicLine.toCenterLine`, uses the populated system Area bounds for the head vicinity, intersects each corner seed rectangle, stably sorts by OpenJDK `Line2D.ptSegDistSq`, and preserves determinant `LineUtil.xAtY`, `Math.round`, `Math.rint`, horizontal, and standout semantics. The eight-page row differential matches 30 systems, 1,906 source seeds, 1,749 kept, 157 purged, 483 no-stem areas, all 29,394 purge visits, 3,521 heads, 14,084 corners, 36,736 neighbor rows, 7,114 candidates, 7,005 visited candidates, 4,182 selected seeds, and 9,902 explicit section-fallback results, including raw bits for every reproducible double. Java2D path winding/segment/path hashes are intentionally projected out while exact area/intersection bounds and all downstream decisions remain compared. Two fresh-JVM oracle runs are byte-identical. Probe source SHA-256 is `d4ab3d3145673bbeb09194e01a227e2ec3422429a3fb6b079359519af8bea115`, emitted-body SHA-256 is `fea0a7a0012a27592c570f7fced0ba9f9e9955b71c196255835a6e5da88965e8`, and complete fixture SHA-256 is `19387924d0d7aaaabf07b0859b353c7fa8d3e3c5d10e8edec8e1d4287b1ace31`. Section-built stump materialization/registration, linker geometry, and SIG mutation remain |
| STEMS section-built head stumps | `materialize_native_stems_head_stumps` executes every `HeadLinker.CLinker.buildStump()` fallback from the live corner and existing-seed products. It dispatches the complete persistent VLAG by integer section centroid, intersects the exact integer-height/double-width stump rectangle per run, stably sorts by integer bounds-center distance, reproduces OpenJDK polygon containment and `SectionCompound` set/width behavior, preserves the shifted signed subsection oddity, paints the tight fixed glyph with Java's orientation rule, and registers every nonempty candidate before the standout gate. A sheet-global exact `(bounds, RunTable)` arena provides stable canonical handles and retains registration events even for rejected candidates. The eight-page projected row differential matches all 9,902 fallbacks, 18,398 sections and compound steps, 3,660 subsection attempts, 969 empty builds, and 8,933 candidates: 758 accepted, 8,175 rejected, 5,591 new, and 3,342 reused, including every owned bounds, raw double, fixed-run digest, registration class, and alias. Probe source SHA-256 is `101cea60bc9445407333b31121d6bd774e72a7148f8f53faa865468a35105e59`, emitted-body SHA-256 is `15e72fc97b017e475ce1bd03f396329ca15fddd1a114e2ab033e7cc091bf563a`, and complete fixture SHA-256 is `dd0247fbd992c7ec40351040efd336f98c8efa88bab0eef10c744430252e966e`. Beam-side stump construction is frozen in the same oracle (five registrations) but not yet a Rust production product; beam/head linker geometry and SIG mutation remain |
| STEMS BeamLinker stump preparation | `materialize_native_stems_beam_stumps` is the fourth production semantic STEMS boundary. It consumes live post-HEADS beams/hooks, the kept STEM_SEEDS product, and each system's complete VLAG, then reproduces constructor-time `BeamLinker.retrieveStumps()`: exact seed-area geometry, stable cross-x ordering, duplicate purge, LEFT/RIGHT side classification, missing-side construction, direction gating, exact fixed-glyph registration/reuse, final stump/side ordering, and the tremolo predicate. The eight-page exact differential covers 30 systems, 803 constructors, 1,606 sides, 3,934 neighbors, 1,820 seed inputs, 1,087 purge comparisons (5 removals and 1,082 breaks), 1,305 side seeds, and 301 builds (4 empty sections, 154 zero compounds, and 143 candidates). Direction checking accepts 6 and rejects 137; registration produces 5 new glyphs and one canonical reuse; 447 sections and 447 compound steps yield 1,821 final stumps, 1,311 final side stumps, and zero tremolos. Probe source SHA-256 is `98c19499ca486fda8ddec92f18f9e3de54f27041987b011220babbf202dc0039`, runner source SHA-256 is `08964909fa4b7f26ac12c451cfe3a40e4c1ec6cf7ecc2524a2fa11b959175679`, emitted-body SHA-256 is `18e6431ad73d05f8a72eb1f8e82b8ab047279e2cdc54d0545d7acf3e6bab0899`, and complete fixture SHA-256 is `902478763d2897eb0d3f031a0895bee7d91a5a7bf8acf8188bf752273e149f14`. The claim ends before `equipStumps`/`equipOrphanSides`; the following row closes that constructor topology and reachability boundary |
| STEMS BeamLinker B/V construction and reachability | `materialize_native_stems_beam_vlinkers` is the fifth production semantic STEMS boundary. It consumes GRID, BEAMS, kept STEM_SEEDS, and beam-stump products; replays the sequential live beam population; creates stump and orphan BLinkers with exact per-beam IDs, side maps, direction order, reference points, and stopping-head sides; folds system/staff/Part limits into the raw lookup quadrilateral/theoretical line; performs every closer-beam group/good/hook/intersection/alignment action and stable sort; optionally rebuilds against the selected opposite border; and intersects the final Area with neighbor seed bounds in insertion order. The exact eight-page differential matches 803 constructors, 2,116 BLinkers (1,821 stump and 295 orphan), 2,417 VLinkers (1,827 stump and 590 orphan; 1,389 TOP and 1,028 BOTTOM), 2,860 Part folds, 9,186 alien candidates with 1,094 survivors and 703 limiters/rebuilds, and 12,491 seed checks with 2,169 reachable. The prerequisite live GRID wire retains detached brace filaments for exact two-staff Part ownership, and registered beam glyphs now use pinned-OpenJDK `Area`/`Order1` point crossings. Two fresh-JVM fixtures are byte-identical at 46,946 lines / 18,307,148 bytes. Probe, runner, emitted-body, and complete-fixture SHA-256 are `fbc5dace791c84e82db5ff870fb4bcc23e06f29b54619865f19448c0f016a5c2`, `38e723c15bec6d67c4b856fc40a40d3ee0e4835f466c0c917715c792e6fa1c75`, `bd43baa197540107e27d2ac97098dbb9df6d6bea1003888ee3625c69e21e60bf`, and `77cfa1f1d9b6e3f8917ff44db7e3f643ffca690bd639d8a5a93f6fea208a8388`. The claim ends before HeadLinker construction and source-ordered `inspectVLinkers`, where cross-beam anchors mutate other BeamLinkers before head filtering |
| STEMS source-ordered beam/head reachability | `materialize_native_stems_beam_reachability` is the sixth production semantic STEMS boundary. After every HeadLinker exists, it traverses the 803 live beams, each BLinker arena in insertion order, and each TOP/BOTTOM VLinker; skips 29 anchors appended before their target beam's turn; executes 4,960 sibling scans and 1,617 eligible cross-beam searches; scans 5,354 BLinker candidates with Java's strict first-tie and inclusive reuse threshold; creates 145 anchors and reuses 1,472 BLinkers, including 215 anchor reuses; and retains both immediate beam-end and final 2,261-BLinker arena snapshots. `filterHeads` runs only after all beam mutations for each V, scans 158,886 stable head rows, retains 5,739 candidates, rejects 46 for distance, checks 11,386 CLinker corners, drops 531 void-head wrong sides, and accepts 5,059 CLinkers. B targets precede C targets exactly. The gate proves zero competing removals, small heads, small beams, and size drops, so generalized small-head support is not claimed. Seed snapshots remain unchanged. The corrected two-pass fixture is byte-identical at 232,460 lines / 61,411,164 bytes; probe, runner, emitted-body, and complete-fixture SHA-256 are `39ed0694f7c31593f157b5f250f8bfa4f006984e3b491a877903d64d810edd7b`, `61801362bc7328cfb3e90f7460016e333d776ee964d39cc296f60cf6edac33f1`, `470827ebc19065890c41c10016511e77eeefc851823bb8587f7537c7e7db23cf`, and `9c3f6d17fa6806cba9b01f3922aca34a220d21dc1a5269723e151a025c693221`. The next row reaches `StemBuilder` item retrieval and seed/chunk mutation |
| STEMS beam-origin `StemBuilder` construction | `materialize_native_stems_beam_builders` is the seventh production semantic STEMS boundary. It drives each of the 2,417 beam-origin VLinkers through the actual source-ordered `inspect(maxProfile)`, complete `StemBuilder` constructor return, and V `sb` assignment. The constructor recomputes its direction from its theoretical line rather than reusing the V direction: the only divergence is Carmen system 2 / builder 56, yielding 1,390 TOP and 1,027 BOTTOM builders. It removes 215 of 2,169 seeds, retaining 1,954; reduces 6,676 targets to 6,670 (1,617 B and 5,053 C); creates 1,442 chunk glyph registrations (799 new, 643 canonical reuse); removes 175 chunks; and retains 9,419 final items with 12,085 length rows. The bounded registry records zero external members and zero unmodeled reuse, which does not claim global glyph novelty. Its JDK 25 mini-TimSort audit records 18 comparator cycles and 2,503 equivalence inconsistencies; only lists up to the observed maxima of 11 target and 14 final items are modeled, and a list of 32 or more fails closed. All 35,419 builder checks report zero SIG, system-stem, linker, C-builder, and unexpected-builder mutations. The emitted body is 91,211 lines / 29,195,732 bytes and the complete fixture 91,212 lines / 29,197,924 bytes. Probe, runner, emitted-body, and complete-fixture SHA-256 are `c320870ea130e5156124b111e34c918fa4f640595109ac44b8a4de89b732d178`, `adc2647152b925a2a81fe580a240b4c8be05fca3148ef3d3df29d73577e72806`, `da4226ee2227d6369054fbce2de4252c72347242253a335132883d9cf871bd22`, and `a3708e0436184dac5aa63fdb43c70cf05252fa7dbbfd7e9a2d746082e22f2180` |
| STEMS head-corner reachability | `materialize_native_stems_head_corner_reachability` is the eighth production semantic STEMS boundary, green in the normal two-test gate with zero ignored tests; the semantic differential ran in 33.18 seconds. Across eight pages / 30 systems it visits 3,521 standard black/void stem-capable heads and all 14,084 corners; 36,736 ordered seed scans produce 1,340 assignments, 1,007,081 compact head scans produce 4,566 C targets, and 9,015 sibling scans produce 8,120 B targets. It writes all 14,084 C seed lists and appends 1,687 head-origin anchors, so the final B-linker algebra is 2,116 constructor entries + 145 beam-origin anchors + 1,687 head-origin anchors = 3,948. Targets preserve C-before-B order. The 16,501 builder checks comprise the preceding 2,417 V assignments plus 14,084 still-null C builders; forbidden SIG, link-state, and page-persistent registry mutation counts are zero. Scope is limited to standard black/void stem-capable heads and does not implement small-head truncation. The reachability-only beam prefix intentionally omits prior beam `StemBuilder` registry mutations because this boundary does not read them; the ninth boundary resumes the actual beam-builder registry timeline. Source inspection confirmed `BeamGroupInter.getMembers()` allocates a fresh list, while the hardened probe still clones and audits every group's identity/order. The fixture is 79,216 lines / 37,478,914 bytes; probe, runner, emitted-body, and fixture SHA-256 are `7bac85a2e878d67ccecab9866428a8068b83d1453c2249f49b0c18ae6a17b39f`, `e9016abb44a500e242b81364531b775fe6b724cddf697cfc0bd4cfe21af0f75d`, `b3f10b53346adac1309d12fa2d245840a88b02c17e399e88d7e5e36f0358889b`, and `537cae86c19de20af35a246e03b6edd7f324d0f08c5768b319ed0557a7e28921` |
| STEMS head-origin `StemBuilder` construction | `materialize_native_stems_head_builders` is the ninth exact production semantic STEMS boundary. It resumes the real page-persistent chronology after the eighth boundary and constructs all 14,084 C-origin builders in system/head/corner order, after the already graded 2,417 beam builders. The exact eight-page / 30-system gate replays 8,939 stump registrations (5,581 new / 3,358 reuse), 1,442 beam registrations (796 new / 646 reuse), and 19,295 head-chunk registrations (4,619 new / 14,676 reuse), including eight stump chronology action changes and three head-to-later-beam reuse/action changes. Its builders scan 15,953,076 vertical and 14,436,784 horizontal sections, accept 34,526 and 23,787, retain 45,938 filament members, produce 29,120 final items with 165 inserted gaps, and reproduce all 70,420 profile-0-through-4 lengths. The gate matches all 42,252 JDK 25 small-list sort audits (8 cycles / 319 equivalence findings); retrieve-seed, target, or final lists of 32 or more fail closed, and the frozen maxima are 2 / 7 / 13. The corpus uses inspect profile 1 with no divergence; production rejects an inspect/system-profile mismatch. It contains no VIP heads, but the native branch deliberately preserves Java's VIP-only removal bug: all 6,087 low-remain non-VIP chunks stay. The shared `VerticalStickFactory` also preserves Java's processed-without-compound rule, so a thickening side can still be reused later as an isolated sticker. The bounded registry contains only structurally projectable live glyphs after MultipleRest replacement and does not claim page-global Java `GlyphIndex` identity. The gate proves zero SIG, system-stem, link, or unexpected-builder mutations; the seam stops before expansion/linking. The split fixtures total 593,749 lines / 171,932,512 bytes. Manifest, probe, and runner SHA-256 are `21d8d11beb4a8895759198f17a45a981a66f9554c9559d1711db09f3db7b764e`, `364ad5d74f15c9cbaf77b67da987f6bc3a309c0bd5c80093f34185d6c4ceadd9`, and `215410766e419685c6cf3a5c9c8f2c8e7ac39b0f02ef18780f4a67450ae91b37`; the eight full fixture hashes are `c001dd763ccd8849c6d95379d45ce15f94e6ce7d8bf364e7a9b408f072ff645c`, `195a65e77f321aa45758d19e7448f7f1c1458918858a64099936e741d0a456b0`, `8320a7b4e645620784d66f67ad7b8e5cee866c72a30e310102f4726542a498bf`, `43d94ddb7af2ebd36c29dae70446b27189d9b045afcf4724d79566e2608ff03a`, `87c8b9ba51361a777d0529fa8a397263cafce593bd552ddbbf1fe5408758ed21`, `c098170dc32bc1773c1d5319a459cbf3b4ba93fa076626a78f2aaa9fbaffcbc4`, `745b90cb61b637ab829c9495dd379709479fd7cfbe59b1cdfef73807523cac43`, and `66b77dd58f4cf3ac3b8e3971695bb7aab953f95e44cd6ba69625efb7450aa6a6`. The normal full native semantic-stream gate passed twice, independently in 84.48 seconds and in root verification in 88.93 seconds; strict integration-test Clippy is green |
| STEMS beam-origin `VLinker.expand`/link-plan boundary | `materialize_native_stems_beam_link_plans` is the tenth exact production semantic boundary. It evaluates each inspected non-anchor beam builder independently for profiles 0 through its construction maximum (3 for a stump, 4 for a side), with the effective system link profile, and stops immediately before `StemBuilder.createStem`. This is an immutable matrix, not the later scheduler: canonical beam/hook Glyph identity, live Exclusion/competing-hook topology, and therefore the selected attempt remain out of scope. The exact eight-page gate covers 30 systems, 2,417 builders, and 11,573 plans: 2,903 `NoHeadTarget`, 289 `ExpandFailed`, 2 `NoRelations`, 58 `NoGlyphs`, and 8,321 `ReadyForCreateStem`, with 18,345 final relations and 12,523 final Glyph entries. It records 578 gap decisions, 9,869 separation checks, 18,416 relation attempts, and 37,683 Glyph updates. Java produces 3,226 downward shared `theoLine`/current-attachment mutations, 49 rollback-line divergences, two dynamic relation-side mismatches, and zero forbidden graph/index/linker/builder mutations; the immutable Rust product emits every would-be delta without changing its predecessors. Profile 4 contradicts its terminal-head javadoc in 9 no-stop, 632 beyond-last-stop, and 645 at-last-stop ready rows. The split page fixtures total 120,724 lines / 104,056,316 bytes; the combined emitted body has 120,646 lines (120,636 semantic rows plus the 10-line shared header) and SHA-256 `ac0fcb9880dbf720c8b73e6baf02867d05e0f2d5a62f208f52e9fa7d5c764966`. Manifest, probe, and runner SHA-256 are `f511b049cf5e32de6fb0151a36a1385efb78b4965fd704c7545eaef8522a2f87`, `2a5e107f947e140e030f3cc1dff06105ab730af3e41381e76f5f8113a17b0fa2`, and `a73ed3977662427062b8d81ac8796ffa54d51daa2f97ea1f109a3d606d0c13b7`. Eleven focused unit tests and every semantic row pass; independent and root full-gate runs completed in 32.25 and 32.41 seconds, and strict integration-test Clippy is green |
| STEMS deterministic beam-scheduler frontier | `materialize_native_stems_beam_scheduler_frontiers` is the eleventh exact production semantic boundary. Per system it reconstructs the live beam/hook SIG order, 651 page-global canonical live Glyph aliases, 78 ordered live raw hook/full-beam Exclusions, the first identity-equal competing hook, and Java's stable reverse-integer-width order across 803 beams and 322 adjacent width ties. It then replays LEFT/RIGHT side order, TOP/BOTTOM V order, target prechecks, side/stump profile choice, and local worklist removal against the frozen tenth-boundary plans. The eight-page / 30-system corpus reaches 56 attempts: 26 empty-target precheck skips remove 14 failed beams only from the local scheduler worklists, and each system's first invoked plan is one of 30 `ReadyForCreateStem` rows. `ReadyForCreateStem` is feasibility evidence, not link success: every system stops at a typed `AwaitingVLinkTransaction` before applying the plan. Fourteen pending downward line/current-attachment deltas are published but not applied. The corpus invokes zero known-false plans and reaches zero stump rows, hook-removal transactions, retry frontiers, or completed systems; the product still models and fails closed around those later paths. It performs zero `createStem`, GlyphIndex, `systemStems`, SIG/relation, link-flag, stored-line, or other persistent mutation. The emitted body is 998 lines / 460,651 bytes—993 semantic rows plus the five-line shared header—with SHA-256 `8ff44c35d8c1e2334c56c4d7e546fdaacbcb2964a1ab6103168f25346e041ff1`. Manifest, probe, and runner SHA-256 are `b6b77cdead537a70b482ae7ef5d5c8312cc5993529382f1f39fb4779afa7abb2`, `afb5c564a474bc0c227b9fdc886cf892c60ae39aa62c1d93cef8aaf610b90fba`, and `2d5609b5c5ef713aa3fda6467d000fad89cd8147e97d1541b5060305b414c99e`. Eight focused production units pass; the normal integration suite is 3 passed / 0 failed / 1 ignored in 31.09 seconds, covering parser drift, expand-fixture provenance, and the full exact gate. The independent root full gate passes in 31.41 seconds, and strict integration-test Clippy is green |
| STEMS first awaited beam-V `createStem` transaction | `apply_native_stems_beam_vlink_create_stem_transaction` is the twelfth exact production semantic boundary. It resumes each system's first `AwaitingVLinkTransaction`, commits any preceding deferred and pending aliased line/current-attachment deltas, selects the singleton Glyph or constructs the exact vertical compound, performs candidate-specific exhaustive structural GlyphIndex registration/reuse, checks structural `systemStems` identity, runs the exact `StemChecker`, and returns/inserts the reused, checked, or artificial stem as Java does. The Glyph certificate is one-shot for that exact candidate. Production supports `systemStems` Present/reuse inside `createStem`; only the compact v1 real-fixture loader refuses to hydrate a Present system-stem certificate, independently of the later VLinker head-side stem-reuse loop. Across eight pages / 30 systems and transactions, the candidates split 15 compound objects with pre-registration ID 0 / 15 singletons; 14 line deltas commit; every Glyph lookup is Present and active and therefore `ReuseActive`; every real system-stem lookup is Absent; and every real result is `CreatedChecked`. The gate bit-compares each returned median endpoint, mean thickness, and integer vertical-ribbon bound; all returned Inter IDs are 0, abnormal flags are false, and SIG attachments are null. Glyph/Inter allocator, SIG, relation, and link-flag deltas are all zero. Only the eight system-1 transactions are true sheet-first chronology: the other 22 deliberately use an isolated fresh-sheet/system JVM and are evidence for that system frontier, not serial page-global ID chronology. The seam stops before the VLinker head-side stem-reuse loop, `BeamStemRelation.checkLink`, SIG vertex/edge mutation, relations, or linker flags. The real corpus does not exercise new/reinsert registration, artificial creation, rejection, or existing-system-stem reuse; focused synthetics cover those paths. The reconstructed body is 261 lines / 153,517 bytes—256 semantic rows plus the five-line shared header—with SHA-256 `0c8c51e1c170a0dc3ec7e5910e6dca63a82f7d8fe6699b585c9556f183b359dc`. Manifest, probe, runner, and manifest-body SHA-256 are `b7e6fe6e7dc2f5eeba106133c930249f20e2c75d764704252289724bbe28c3e0`, `36fecabe18d7713c823ce6990dae717e78997354a9ae0b142cba55f7d75004f3`, `6d95ff62d0acb502d531d6fb2aea0382fcb9dcb8fdd871fb7b0e2fba2ffb1de8`, and `67d983b056548118015f5b7d18a9e2772860e08e0d2ab076118b25a9678c40af` (9 lines / 5,691 bytes). Eleven focused production units pass; the active exact/synthetic gate is 5 passed / 0 failed / 0 ignored in 31.98 seconds, and strict library/integration-test Clippy is green |
| STEMS beam-V head-side reuse and `BeamStemRelation.checkLink` | `evaluate_native_stems_beam_vlink_reuse_check` is the thirteenth exact, read-only production semantic boundary. It joins the scheduler, expand-plan, and committed `createStem` products; begins with the returned `StemInter`; walks the relation `LinkedHashMap` in insertion order; lazily reads each C-linker's shared S-linker linked flag; preserves `HeadInter.getSideStems()` relation order, Java's missing-side-set invariant, first-unique break, and unread suffix; then reproduces public `BeamStemRelation.checkLink`, including beam-border intersection bits, strict beam-portion tests, `Math.rint` max-dx, the scale stem-thickness x-gap half-width, endpoint-derived y gap, raw and clamped 1/4-weight impacts, intrinsic ratio 1, inclusive grade 0.1, and extension point. Across eight pages / 30 first transactions, 65 relation entries are all unlinked, so there are zero head-side scans, live scan stems, or real reuse selections; all 30 relation checks accept and none reject. That original first-frontier census remains zero-reuse. A separate Allegretto-derived reconstruction exercises the linked branch: system 1 transaction 28 / plan 25 traverses HeadStem edge 229, selects the modeled attached StemInter with Java ID 2227, and leaves relation-map entry 1 unread. The projector derives Java's exact snapshot/projection hashes from explicitly reconstructed native SIG/binding/S-cell/system-stem inputs before the bounded fixture is opened, and leaves those inputs unchanged; the gate does not replay native transactions 1-27. Each page's system-1 output adds one explicitly isolated synthetic SIG block with positive-ID attached stem targets and actual `HeadStemRelation` traversal; the eight blocks exactly cover zero, unique, and multiple side-stem cardinalities, first-unique lazy break, missing-map behavior, portion ULPs, threshold equality, parallel/intersection non-finites, and zero mutation. The boundary stops before conditional `SIG.addVertex`, base BeamStem link application, linker flags, sibling beams, or relation-loop/head links. The concatenated body is 601 lines / 472,445 bytes (553 semantic rows plus 48 repeated page headers), SHA-256 `76a6d20865a5a372bb6485ff6debeb0c435b64d1f92cf5ee07e1fbe0cf61418f`. Manifest, probe, runner, and manifest-body SHA-256 are `4ab7078b760daca6691fcc03e8f29684ec4c976f918d747cb2047f01accd0559`, `3ab243141f6eda3028885e3d73946c129e62554d5abc14658ca6e786f38650b0`, `1b4913e1fc8f2665383635fac3e7c3c16f7de369ff8da5db4b4fe57e1b29ac21`, and `58259448c36c5c684cbfef2215eb124a2ca62e5aae8f12d1a73510345687fb6d` (9 lines / 9,202 bytes). The manifest also pins `BeamLinker`, `BeamStemRelation`, `HeadInter`, `HeadStemRelation`, `LineUtil`, `Scale`, `AbstractConnection`, `SupportImpacts`, `Support`, `GradeImpacts`, and `GradeUtil` at `131f91f6605ecf03463ef4b6021a461240f99d7dfe2b1a1b94b0213d158d1747`, `3ceff58fa9b298d97f325372d0e5a9b363755f3ad47cac7b66b07bd8d1e735f1`, `ce32f3497972606ec696f59928e51bc9b057e74f13dcbc7306a73f7c46d99fda`, `f8828725da97dc44d9bb350adbb8e1055eb73934d0d0386e54e8d95994070eef`, `3644b4c4ffd627bf554c8dd4045ba273f2cb7f7a6e938d8d68c45540844405cb`, `25ab64d3a18063bd5cc5249c05c649e3cff27c79b69ce3a501515329276fecfa`, `bd11a796c1d176f42b087e31c23bffb004eca7cad4749a0f36ddff3573265f81`, `8bbdaa99a990ded65c69aee9e99e8eb0deb82506a6c620d78ab3f4372953a8f3`, `8b6171dd1b98b842e8defcd9758e6003d315534ac3ae79864ccc2309e94ad4af`, `f0b90aad2d26675f4518153e6395d8c528960b146f1a32ca5d272d5297d7e840`, and `e7fedd800456c64d7906ba252ee5e6a3881ab9dc3cf4da07a7a0913dbbcb6597`. Two byte-identical passes per page used 8 compiler plus 60 runtime foreground/reaped JVMs with maximum Java concurrency 1 and no background process. Eight focused production tests and all 8 exact integration tests pass; the latter finish in 32.66 seconds under independent root verification. The new lane is a separate fixture—10 lines / 2,566 bytes, SHA-256 `287175a58717874882bc6487f7d59ea86a22e44cadcac003ee99a36606e5ab34` (five semantic rows plus summary)—and is not included in the original 601-line corpus/hash. Strict library/test Clippy and global formatting are green |
| STEMS beam-V base SIG/BeamStem application | `apply_native_stems_beam_vlink_base_transaction` is the fourteenth exact production semantic boundary. It consumes boundary 13's accepted `ReadyBeforeSigMutation` result; for an ID-zero selected stem it executes the conditional `SIG.addVertex(stem)` path through shared InterIndex allocation/VIP lookup, SIG vertex insertion and source order, `setSig`, the sole `SigListener`, `StemInter.added`, abnormal-state change, and SheetStub/Book modified/dirty propagation, while an already attached positive-ID stem skips that prefix. It then applies the fresh base `BeamStemRelation` with `Link.applyTo(beam)`, preserving source/target-removed suppression, outgoing duplicate scan order, separate draft-object and graph-relation identities, JGraphT edge ordering, and synchronous callbacks. `BeamStemRelation.added` performs the post-edge incident scan and zero-`ChordStemRelation` certificate, then beam abnormal checking: full beams read ordered BeamStem/BeamRest portions, while hooks use the class-only any-BeamStem rule without reading a portion. Java's ignored `Link.applyTo` boolean and no-rollback exception prefixes are explicit. The terminal is `ReadyBeforeBLinkerFlagMutation`: `getBLinker().setLinked(true)`, sibling-beam linking, and the head-relation loop remain out of scope. All 30 real transactions are `NewIdZero`, insert one vertex and one base BeamStem edge, reuse no stem, and find zero ChordStem matches. Each page adds five supported and four envelope-only transactions on truly isolated sheets/SIGs, for 40 supported and 32 partial-prefix cases total; these are isolated branch/failure evidence, not real-corpus or blanket production-equivalence claims. The normalized corpus is 1,314 lines / 1,185,901 bytes—one shared eight-line header plus 1,306 semantic rows—with SHA-256 `ece76c038ef1b2017d2f356dd6ead59379376ffc5ab0306e8c5e8c34a9471e53`; split fixtures total 1,386 lines / 1,227,749 bytes. Manifest, probe, runner, and manifest-body SHA-256 are `5da20f701d38bf9b81c6000ed4e8aba4fadd285c85d81753ef4a862f0a4875bc`, `2139f0f5c2aba399d2eb8bc10ccbc2ec1221ce00ae2fdeb50782c80622f982e3`, `88091fd27bef445f7045b721a6258da9652bac2f68d1ced277bbe82c1640d9b5`, and `8bbd189d9c7e82702ce8513347841cfe5aff2f96f8b39bf9dd07e05bea4e6b35` (9 lines / 16,479 bytes). The manifest pins all four predecessor fixtures per page, the complete active Java/Gradle source set, and JGraphT core 1.5.2 at `dfa596e9f0d0838f1b5e81dd0cd60e3a76c2c290ac25a0a029ffde58cf5e4c14`; seam-critical `BeamLinker`, `Link`, `SIGraph`, `SigListener`, `BasicIndex`, `InterIndex`, `StemInter`, and `BeamStemRelation` hashes are `131f91f6605ecf03463ef4b6021a461240f99d7dfe2b1a1b94b0213d158d1747`, `e27734fa0f4273db91527ed969ef1881605cda32eb970bb464ea037b0f0ed34e`, `6b6ff3172d1f194566a7f59aa2c854cb62ea9c4deab79a43b6b0b85e1d4c4c2f`, `19b42c96257bd78fc9d4bc428242590ae01832b395aebdeefe26e081ceadc08d`, `7c747248365477c9381d004891e88f96273c0796a26f7417192fdaaeac8d3707`, `830ee77262bd9b631d352e49ddc150055e621ad9cd76c2a0671fc2233b662b7a`, `bcdb1b67694f45de89a9ad8712222e77af7c6e29247f5edd487d8dcabd11eeec`, and `3ceff58fa9b298d97f325372d0e5a9b363755f3ad47cac7b66b07bd8d1e735f1`. Two byte-identical passes per page used 8 compiler plus 60 runtime foreground/reaped JVMs, maximum Java concurrency 1, and no background Java. Twenty focused production tests and all 10 exact integration tests pass; the latter finish in 33.87 seconds. The full library suite is 623 passed / 0 failed / 2 ignored in 12.47 seconds; strict Clippy, global formatting, diff-check, and oracle `sh -n` are green |
| STEMS beam-V B-linker shared flag assignment | `apply_native_stems_beam_vlink_b_linker_flag_transaction` is the fifteenth exact production semantic boundary. It clones and reruns the complete boundary-14 transaction from its exact pre-state before trusting the supplied terminal, then executes Java's plain `getBLinker().setLinked(true)` assignment on the scheduler-selected outer B object. The result retains `Link.applyTo`'s ignored boolean and fresh relation grade, exposes the exact target B and `EnumMap` TOP-then-BOTTOM V observers, distinguishes one assignment from a false-to-true value change, and stops at `ReadyBeforeSiblingBeamLinks` with sibling/head/linker side effects still zero. All 30 real transactions are false-to-true writes. Their live Java census contains 3,948 B entries: 2,116 frozen constructor entries plus 1,832 later dynamic anchors, which remain full-arena guard evidence rather than fabricated production state. Eight system-1 blocks contribute 32 explicitly isolated `UnsafeExactClassNoGeometry` setter-and-shared-cell-only envelopes: 24 false-to-true, 8 idempotent true-to-true, and 8 with the prior apply return false. The normalized corpus is 4,562 lines / 2,535,981 bytes with SHA-256 `6125665f38d894f6b05a24651f56f0a38c01e2acc2a7d18167a4175d5ae81c34`; split fixtures total 4,634 lines / 2,590,657 bytes. Manifest, manifest-body, probe, runner, and effective-classpath SHA-256 are `c7032ac4871188ef0cf48ac63d99996e78a0e163bf1470d3be84c5e9b10d1d92` (10 lines / 24,897 bytes), `3f332e7751d5de73e296294ccc6882ff6a578d0328b8c0d717c96666ffbb3e4d` (9 lines / 18,910 bytes), `b4c750370bebda13e66c49a8cc88756cb677ebf04f77d7dae883cb373fe431a8`, `066a5ee494c583bdc7e9df1fc6e282015afc7663968b5e0a836219e545d14c24`, and `fd4e52c2275675a53459dff2b2e2d89636f3c5fb6ab5a1f7be65f74157663fb3`. Two byte-identical passes per page used 8 compiler plus 60 runtime foreground/reaped JVMs with maximum runner-scoped Java concurrency 1. Seven focused production tests and the shared 5/5 hydration regression pass; the latter finishes in 126.03 seconds |
| STEMS beam-V sibling BeamStem links | `apply_native_stems_beam_vlink_sibling_links_transaction` is the sixteenth exact production semantic boundary. It independently reruns and exact-joins Boundary 15, then executes the whole serial `linkSiblings(stem, grade)` call: reconstructs the live BeamGroup outgoing-Containment order, performs Java's stable top-down intersection sort and base removal, preserves glyph-object identity skips and first-runtime-class duplicate lookup, lazily reads the shorter-beam ordinate branch, installs each fresh `BeamStemRelation`, runs the synchronous zero-`ChordStemRelation` callback and raw-beam/hook abnormal rule, and performs the first source-identity `StemBuilder.items` lookup before assigning the selected sibling B-linker's shared cell. State and trace preserve relation/object identities, exact query hashes, abnormal/SheetStub/Book changes, group-member post-state, and serial edge-callback-flag interleaving. All 30 real transactions cover 58 non-null native-glyph group members and 11 sibling candidates; all 11 take `Linked`, add an edge, complete the callback, and write a B cell, with zero same-glyph, duplicate, shorter-wrong-side, or ChordStem cases. Eight repeated isolated blocks add 64 supported cases—`SameGlyph`, existing relation, shorter wrong side, full/small/hook links, no B linker, and idempotent B cell—and 16 Java throw envelopes; these are supplemental gate evidence, not production-equivalent real transactions. The terminal is `ReadyBeforeHeadRelationLoop`. The normalized corpus is 717 lines / 580,329 bytes—one eight-line / 753-byte shared header plus 709 semantic rows—with SHA-256 `c6a62f9b98ce55eda2bd142b083a2ff6b14d08dab6b1a2ce3c1a0d643d5efd66`; split fixtures total 789 lines / 654,858 bytes. Manifest, manifest-body, probe, runner, and effective-classpath SHA-256 are `6dcca78c13facf7fa9ee29506eab2961d1410babf396930724dce16f5474e29d` (10 lines / 31,471 bytes), `c5d44bf655814aac1a297d4ad67fe401291449e231d581d11c812e197ef0fba0` (9 lines / 23,218 bytes), `a3ee02cf29f5a8a7c70bd7b2e064d7a1ff0fee2d120bde3b2088c7f2db98eda0`, `9d2535980f191105d912ec2e07c99e3f06f55b1c406a68da610f1685ec07e1a5`, and `fd4e52c2275675a53459dff2b2e2d89636f3c5fb6ab5a1f7be65f74157663fb3`. Two byte-identical passes per page used 8 compiler plus 60 runtime foreground/reaped JVMs—68 total—with maximum runner-scoped Java concurrency 1 and no background Java. Twenty-two focused production tests and all 10 full exact integration tests pass; the latter finish in 126.68 seconds. The full library suite is 652 passed / 0 failed / 2 ignored in 11.92 seconds |
| STEMS beam-V head relations | `apply_native_stems_beam_vlink_head_links_transaction` is the seventeenth exact production semantic boundary. It independently reruns and exact-joins Boundary 16, then executes the insertion-ordered head-relation map: every entry assigns its shared S-linker cell, performs the complete directed head-to-stem duplicate scan, and either preserves the existing relation or lazily computes consistency on the existing plan draft before inserting its `HeadStemRelation` and running the synchronous head-then-stem abnormal/dirty callback. Compact production requires exact live endpoints, standard listener topology, prepopulated head side/extension, and non-manual relation/head/stem state; manual chord rewiring and Java fault prefixes remain isolated gate evidence. The remainder comparison is retained without its commented-out split, and the terminal is `ReturnedTrueBeforeOuterBLinkerAssignment`. All 30 real transactions contain 65 entries: zero duplicates, 65 inserts, 65 S-cell writes, 65 consistency writes, and 260 ordered events. Eight isolated blocks add 16 supported and 40 envelope transactions—56 total / 304 events—including 40 graph deltas, 16 throws, 16 manual cases, and 8 chord rewires without claiming production equivalence. The normalized corpus is 1,583 lines / 785,671 bytes with SHA-256 `b57ec3f2bf401fce6d6d62c7522285dd3288b35b40d7c5c453468cf5dde4ce48`; emitted split bodies are 1,639 lines / 790,438 bytes with SHA-256 `044631a9dc5177b3fbe074a03cc031f52cb6087b3ea3491377f820d633b44d01`, and full split fixtures are 1,655 lines / 873,975 bytes with SHA-256 `6e9abd60f5274622bd9638cc6e1cd6c489ee5fdc36ec96769507ef9f16f418aa`. Manifest, manifest-body, probe, runner, and effective-classpath SHA-256 are `87b1f5fb459551cb247f4702449128f35d94ac5ee738d764e25e523dd21955ab` (10 lines / 35,839 bytes), `a7934a066b47654b56184e6506825d9f1f5986d96f25b3eb52b2281308185a08` (9 lines / 25,997 bytes), `3e6dd42af58f074d6f9a146dd00c3573fc4c79c445eda629bc82f93d175df61a`, `932084cef5c8d5b700cdda1ce3ddb48e5454fe8f65775a9d7fed52070c7a1d42`, and `fd4e52c2275675a53459dff2b2e2d89636f3c5fb6ab5a1f7be65f74157663fb3`. Two byte-identical passes per page used 8 compiler plus 60 runtime foreground/reaped JVMs—68 total—with maximum runner-scoped Java concurrency 1 and no background Java. Twenty-four focused production tests and all 13 full exact integration tests pass; the latter finish in 148.82 seconds, the standalone manifest validator passes 1/1 in 129.11 seconds, and the full library suite is 676 passed / 0 failed / 2 ignored in 12.18 seconds |
| STEMS SIDES-to-STUMPS entry | `continue_native_stems_beam_sides_carrier_into_stumps` is the twenty-first exact production semantic boundary. From chula system 1's explicit `SidesExhausted` terminal it walks the 34 retained beams in Java order. Beam SIG 12 begins Java event 0; stump 0 is both structural-side and linked, so structural precedence produces event 1. Unlinked stump 1 reaches plan 147 at `BEAM_SEED` profile 3 / link profile 1 and stops before `createStem` at Java event 2 with two relations, one glyph, and no line change. Native emits two scheduler events and returns that attempt as its typed frontier. The real prefix has no pure already-linked skip or known-false plan; a focused synthetic covers the linked-only guard. The refreshed fixture is 10 lines / 3,134 bytes, SHA-256 `ef8f180110a409f85167ee1cc0f641c210144d6e5b5c737d5d8eb69e82d47bcb` (five semantic rows plus summary); only its probe provenance changed for Boundary 28. Graph, shared cells, and registries are unchanged |
| STEMS first STUMPS transaction and resume | `advance_native_stems_beam_stumps_transaction_from_first_stems_bridge` is the twenty-second exact production semantic boundary. It atomically executes chula system 1's beam SIG 12 / `beam:12:b:1` / plan 147 frontier through B12-B17 and resumes STUMPS without a SIDES-only outer B18 assignment. Java reports glyph 310 `ReuseActive`, `CreatedChecked`, two `AllUnlinked` reads, Stem Inter ID 2372, no siblings, two heads, and `outerAssignment=false`. Native adds dense stem identity 32 and relation identity 331, reaches 254 vertices / 334 edges with 33 Stem bindings, 62 linked B cells, and 70 linked S cells, then stops at worklist index 1, beam SIG 22 / `beam:22:b:1` / plan 622. The refreshed six-row-plus-summary fixture is 11 lines / 2,619 bytes with SHA-256 `b1a312ddc690911b916971081ce21ea1c2211283df174a2175094ace7c144d5e`; probe, runner, emitted-body, and semantic-pass SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `f9ca026d03873ab5c40c568a926455e0555d343540d522258d87757a1cc28f0c`, `db9a2fd99746dfbc2ae3b5eed643a374e79dabc26a79101b05779cfba25ee5a4`, and `5997662c47fb5be7cc61079baecb10f2986c89b05a7c0c97b937596dbc5009d6` |
| STEMS second STUMPS transaction and resume | Boundary 23 generalizes the unchanged production carrier by calling it again from Boundary 22's mutated terminal. Chula system 1 plan 622, beam SIG 22 / `beam:22:b:1` / TOP, runs B12-B17 with no outer B18. Java reports glyph 321 `ReuseActive`, `CreatedChecked`, two `AllUnlinked` reads, Stem Inter ID 2373, no siblings, two heads, and `outerAssignment=false`. Native adds dense stem identity 33 and relation identity 334, reaches 255 vertices / 337 edges with 34 Stem bindings, 63 linked B cells, and 72 linked S cells, then skips structural-and-linked `beam:22:b:2` and `beam:16:b:0` and stops at worklist index 2, `beam:16:b:1` / plan 404. That next frontier has profile 3 / link profile 1, two heads, last index 3, two relations, two glyphs, and no line change. Its refreshed six-row-plus-summary fixture is 11 lines / 2,712 bytes with SHA-256 `4e54cc848116597ad563fd9038e102a135ff606660775e09142c8c8564567173`; probe, runner, emitted-body, semantic-pass, and init-script SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `1812529f72a86e4b96b7d08d09f98a1157d9feb862296cd19e95de5caddded11`, `716db362ee56e43a0375d8cf0efb0c88cd0af67de5707926bc4b713505201187`, `07b6dc29043c6b63bd1f9f9e15822270ca3169e8662207c7cbbf67a06d8579a6`, and `08d332af997d502fd32afb8b6257243d5ef41e87fa0001f90f3680c17394acd2` |
| STEMS third STUMPS transaction and resume | Boundary 24 adds third-frontier compound-candidate evidence for the unchanged production carrier. Plan 404 on beam SIG 16 / `beam:16:b:1` / TOP combines Java glyph IDs 303 and 2156; their union equals active modeled glyph 303 at ordinal 972, so `ReuseActive` changes neither registry nor allocator. Java returns `CreatedChecked` Stem Inter 2374 after two `AllUnlinked` reads, adds no siblings and two heads, uses base edge 337, marks B linked, and records `outerAssignment=false`. Native adds dense stem identity 34 and reaches 256 vertices / 340 edges with 35 Stem bindings, 64 linked B cells, and 74 linked S cells. Resume skips structural-and-linked `beam:16:b:2` and `beam:28:b:0`, then stops at worklist index 3 on `beam:28:b:1` / plan 508. That next frontier has profile 3 / link profile 1, two heads, last index 3, two relations, two glyphs, and no line change. Its six-row-plus-summary fixture is 11 lines / 2,709 bytes with SHA-256 `e7409462ec43f5cde89ffdeafb0c5bb59586c37fff1506086d9c5fa770b30490`; probe, runner, emitted-body, and semantic-pass SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `f2a41ca0069873274e443c978d0e84d56c49d67fa3387ef06346995dd2d587c1`, `3e66a99fe44495915fbb8c15f7285a7c9a5ae4340df60b7766968c3e214a1bc7`, and `ee1acaf3b1742346913ce3e9ed32430d3a4b24277537f0ed8e941d530ee6935b`. The refreshed linked-S fixture SHA-256 is `287175a58717874882bc6487f7d59ea86a22e44cadcac003ee99a36606e5ab34`. Boundary 25 below closes the remaining chula-system-1 STUMPS worklist; Boundary 26 then covers one reconstructed-predecessor hook removal, while native Allegretto predecessor carriage, other systems, and full STEMS remain out of scope |
| STEMS bounded STUMPS completion | `drive_native_stems_beam_stumps_from_first_stems_bridge` is the twenty-fifth exact production semantic boundary. It repeats the validated one-frontier operation on a shadow carrier and commits the whole batch only at a positive limit or typed post-STUMPS completion. Zero rejects unchanged, a one-transaction limit commits plan 508 and returns plan 28, and a missing later `beam:32:b:1` cell rolls back all earlier shadow transactions. From Boundary 24, chula system 1 completes the remaining four transactions in plan order 508/28/330/251. Java reports glyphs 308/305/302/300, `ReuseActive`, `CreatedChecked` Stem IDs 2375-2378, `AllUnlinked` reads 2/2/3/2, base edges 340/343/346/350, no siblings, and head counts 2/2/3/2. Native reaches the post-STUMPS terminal after all seven transactions and 92 scheduler events at 260 vertices / 353 edges, 39 Stem bindings, B68/S83. The fresh fixture is 87 lines / 19,184 bytes—82 semantic rows plus summary—with SHA-256 `81fecf842495ddc93792b0ed5acf5641231181f172acd4e5cbf3bc57565f0cd2`; probe, runner, emitted-body, and semantic-pass SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `2c6f9aaf39ae8ec2420104f15a3f6a2784f4eb4f229b0b23a7963ab5aade5717`, `946c160f4759ee3edb093c3cc1e5394965409f64e1b516b1ebcbbbfe009f49e4`, and `a629a2d63d223f28264c3fdc4dc20941e082402c27d75c2c6d884e2ce8282d08`. This is chula system 1's STUMPS scheduler completion, not full STEMS; wider-corpus authority/branches, hook removal beyond the single reconstructed Allegretto checkpoint, other systems, and later phases remain open |
| STEMS bounded competing-hook removal and resume | `remove_native_stems_beam_competing_hook_and_resume` is the twenty-sixth exact production semantic boundary. From an explicitly reconstructed Allegretto-system-1 post-transaction-28 checkpoint—not native execution of transactions 1-27—it consumes Java event 64 at work index 19, where Beam SIG 25 has LEFT and RIGHT linked and competes with same-glyph BeamHook SIG 24. Java removes Inter 907 from the active SIG while retaining its SIG attachment and persistent InterIndex representation; group `[21,24,25]` becomes `[21,25]`, while the local worklist and linked-B set stay unchanged. Native tombstones vertex 56, removes its five incident Containment/BeamBeam/Exclusion/two BeamStem edges and active source binding, and resumes to `SidesExhausted`. Active graph counts move 202/232 to 201/227; Java exhausts at visible event 110, while native emits 54 continuation events and ends at 143 internal events. Missing Exclusion evidence rejects atomically. The 32-line / 4,195-byte predecessor fixture is SHA-256 `d173f1c475245980cad02bbf4624987d787fb293e5419d21444729f18bf7c8f8`; the 9-line / 4,336-byte result fixture is `d4c5decf03eaab893c79b2cb7ebd0378f13ac019acc007a38718105c75eacc71`. Probe, runner, emitted-body, and semantic-pass SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `3b9e0e28c9c2de75266c676a880dfe636bef885591ce12ed832640b8c72dd845`, `52432167156b75e4754259ae6c2a634e87788f028e85e6ea14754859e12ccb1f`, and `2cc4ad8e0aadf29b8055ce34c32b703c033c45880bef24ff26a707b6b6f0d3c5`. Native predecessor carriage, hook removal beyond this checkpoint, wider-corpus STUMPS, dirty-state ownership, other systems, and later STEMS remain open |
| STEMS first post-STUMPS head-phase frontier | `begin_native_stems_head_linking_phase1` is the twenty-seventh exact production semantic boundary. From the exact native chula-system-1 `Completed` beam carrier it validates common system/binding identity, Java's stable reverse-grade permutation, all 102 live graded head bindings, and exhaustive duplicate-free S cells with exact observer order, then clones the unchanged 260/353 carrier. Head 0 is SIG ordinal 45 / Java Inter 1375, grade bits `0x3fe917c3b8207578`; STRICT stem profile 0, link profile 1, and `append=false` begin with empty unlinked/undefined collections. LEFT is open/unlinked with TOP/BOTTOM false/false; RIGHT is open/unlinked with true/false, selecting `h:38:RIGHT:TOP` and returning `AwaitingHeadCLinkTransaction`. Incoherent terminal/system/binding/order/head/S-cell/builder inputs fail closed; dual-corner choice, close-head/gap recursion, retry/closure, phase-2 append, and `CLinker.link` mutation remain unported. The fixture, expanded through Boundary 32, is 16 lines / 12,880 bytes with eleven semantic rows plus summary, SHA-256 `91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`; probe, runner, emitted-body, and semantic-pass SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`, `dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and `e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398`. Boundary 28 below consumes the selected corner; later head iteration and retry remain open |
| STEMS first two prelinked-head continuations and closure | `continue_native_stems_head_linking_phase1` is the twenty-ninth exact production semantic boundary. Starting at Boundary 28's committed `current_index=1`, two calls revalidate the completed carrier, reverse-grade queue, live head bindings, and exhaustive S-cell topology. Head order 1 (x90 / SIG 23 / Java Inter 1331) succeeds via its prelinked LEFT side; both RIGHT STRICT corners are false, then shared Stem 2359 closes x89 LEFT and RIGHT in order, two false-to-true writes. Head order 2 (x81 / SIG 33 / Java Inter 1351) similarly succeeds prelinked and shared Stem 2371 closes x79 LEFT/RIGHT then x80 LEFT/RIGHT, four false-to-true writes. Both return true, record no unlinked head, and preserve SIG/glyph/stem/allocator/relation/linked state; native reaches `current_index=3`, `frontier_consumed=true`, before x20 / SIG 65. Missing closure topology or invalid consumed-frontier state rejects atomically. Later queue entries, a later C-link mutation, an actually unlinked head and rather-good retry/no-link closure, phase-2 append, and broader branches remain open. The current expanded schema-v6 fixture is 16 lines / 12,880 bytes with eleven semantic rows plus summary, SHA-256 `91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`; probe, runner, emitted-body, and semantic-pass SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`, `dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and `e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398` |
| STEMS third prelinked-head continuation and closure | Boundary 30 reuses `continue_native_stems_head_linking_phase1` for head order 3 (x20 / SIG 65 / Java Inter 1419). LEFT is prelinked and both RIGHT STRICT corners are false; Java returns true and shared Stem 2361 closes x19 LEFT then RIGHT, two ordered false-to-true writes with no unlinked insertion. Native reaches `current_index=4`, `frontier_consumed=true`, before x36 / SIG 69 / Java Inter 1427. Graph, registry, stem, allocator, relation, and linked state remain unchanged apart from the two closed S cells; missing closure topology rejects atomically. This is one further bounded prelinked-success case, not full phase-1 iteration or retry coverage. The current expanded schema-v6 fixture is 16 lines / 12,880 bytes with eleven semantic rows plus summary, SHA-256 `91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`; probe, runner, emitted-body, and semantic-pass SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`, `dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and `e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398` |
| STEMS fourth prelinked-head continuation and closure | Boundary 31 reuses `continue_native_stems_head_linking_phase1` for head order 4 (x36 / SIG 69 / Java Inter 1427, grade bits `0x3fe8e37718100f0c`). LEFT is prelinked and both RIGHT STRICT corners are false; Java returns true and shared Stem 2369 closes x35 LEFT then RIGHT, two ordered false-to-true writes, `closedValueChanges=2`, and `unlinkedCount=0`. Native reaches `current_index=5`, `frontier_consumed=true`, before x99 / SIG 61 / Java Inter 1411, grade bits `0x3fe8b9e1faa76070`. Graph, registry, stem, allocator, relation, and linked state remain unchanged apart from the two closed S cells; missing closure topology rejects atomically. This is one further bounded prelinked-success case, not the remaining queue, a later C-link mutation, actually-unlinked/retry behavior, phase-2 append, or broader branch coverage. The schema-v6 fixture is 16 lines / 12,880 bytes with eleven semantic rows plus summary, SHA-256 `91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`; probe, runner, emitted-body, and semantic-pass SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`, `dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and `e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398` |
| STEMS fifth prelinked-head continuation and closure | Boundary 32 reuses `continue_native_stems_head_linking_phase1` for head order 5 (x99 / SIG 61 / Java Inter 1411). Java returns true through the prelinked-success path and shared Stem 2365 closes x98 LEFT then RIGHT, two ordered false-to-true writes with no unlinked insertion. Native reaches `current_index=6`, `frontier_consumed=true`, before x22 / SIG 12 / Java Inter 1309. Graph, registry, stem, allocator, relation, and linked state remain unchanged apart from the two closed S cells; missing closure topology rejects atomically. This is one further bounded prelinked-success case, not the remaining queue, a later C-link mutation, actually-unlinked/retry behavior, phase-2 append, or broader branch coverage. The schema-v6 fixture is 16 lines / 12,880 bytes with eleven semantic rows plus summary, SHA-256 `91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`; probe, runner, emitted-body, and semantic-pass SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`, `dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and `e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398` |
| Later recognition stages | the remaining dependency-light lifecycles are native for `STEMS`, `REDUCTION`, `CUE_BEAMS`, `TEXTS`, `MEASURES`, `CHORDS`, `CURVES`, `SYMBOLS`, `LINKS`, `RHYTHMS`, and `PAGE`; STEMS has eighty-four exact semantic components. Chula system 1 now runs all 32 SIDES transactions through B12-B19 from carried state, reaches exact `SidesExhausted` at 253/331, carries all seven STUMPS transactions to typed completion at 260/353, enters the typed first head-origin C-link frontier, applies eight bounded C-link mutations, twenty-two bounded existing-stem reconciliations, and the intervening prelinked-success continuations through order 58, including one returned-false LEFT undef at order 50, to `current_index=59` without any unlinked head. The third through sixth mutations are bounded two-item LEFT/BOTTOM geometry; the seventh is bounded single-item LEFT/BOTTOM evidence, and the eighth reuses an existing stem through one appended HeadStem relation, not general multi-item/recursive coverage. A bounded later Allegretto reconstruction exercises one B13 linked-S selection and the first real competing-hook removal; native predecessor carriage remains open. Next is replacement of the disclosed first-STEMS snapshot and sparse 16-row selected-base Java identity authority, native predecessor carriage and wider linked-S/hook-removal coverage, wider-corpus STUMPS authority and branch coverage, remaining head iteration from x100 / SIG 42 at index 59, actually-unlinked/no-link and generic retry, phase-2 iteration, and broader C-link shapes, then `recognize_native_stems` |
| MusicXML differential suite | queued |
| Swing UI | explicitly out of the initial headless milestone |

Boundary 30 extends the unchanged head-phase continuation through head order 3. Starting
at `current_index=3`, x20 / SIG 65 / Java Inter 1419 is prelinked on LEFT; both RIGHT
STRICT corners are false, so Java returns true and shared Stem 2361 closes x19 LEFT then
RIGHT (two ordered false-to-true writes) with no unlinked-head insertion. Native reaches
`current_index=4` before x36 / SIG 69 / Java Inter 1427, preserving graph, registry,
stem, allocator, relation, and linked state apart from those two S-cell closures. The
current expanded schema-v6 fixture is 16 lines / 12,880 bytes, SHA-256
`91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`; probe, runner,
emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`,
`dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and
`e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398`. Missing closure
topology still rejects atomically; actually-unlinked/retry, later C-link, phase-2
append, and broader head branches remain open.

Boundary 33 consumes the first later C-link frontier without weakening the initial
frontier API. At head order 7 (x76 / SIG 97 / Java Inter 1483), Java selects LEFT/BOTTOM
(`BottomOnly`), reuses active glyph 319, and creates checked Stem Inter 2380 with one
HeadStem relation. The native atomic continuation moves 679/690 to 680/691, adds Stem
binding 41, links the LEFT S cell, and advances from `current_index=7` to 8 before
x95 / SIG 100 / Java Inter 1489; corrupt frontier or glyph authority rolls back.
The v7 derivative fixture is 20 lines / 18,778 bytes, SHA-256
`8df7d36e780e90e569fcc37144bd48ff43e5b647f9cdc240d899ee10386b153d`; runner,
transformed probe, emitted body, and semantic pass are
`87a12b97b6d9c79e6c0d346f8187b426505ab5e0e7785bd07a5984a03a18c197`,
`93c6771d55b814cff4155d4065d94a322767df9a668033bc7f2e5ea1ea7f6edd`,
`06285da43ff0b5a1f3644c4468570a10f24c0c8f2b8173e9e7d1e268284704d6`, and
`68d581d84f21a79c41df3d4ebf6a856cc0dee266288512e4cd1e44bb3260fa0c`.
Remaining queue entries, actual-unlinked/retry and phase-2 append, multi-item or
recursive C-linking, wider systems/corpus, and full STEMS remain open.

Boundary 34 reuses the same continuation for head order 8 (x95 / SIG 100 / Java Inter
1489). LEFT is prelinked, RIGHT STRICT is `Neither`, and shared Stem 2364 closes x91
LEFT/RIGHT followed by x94 LEFT/RIGHT (four ordered value changes, no unlinked head).
Native reaches `current_index=9` before x42 / SIG 93 / Java Inter 1475 with no graph,
registry, stem, allocator, or relation changes beyond those S cells. The v8 fixture
SHA-256 is `82eca291e69ec27e49903d31b1da408f68962469780a1f706f3f979564e8aebb`; runner,
transformed probe, emitted body, and semantic pass are
`4d3be4619b7fbe5f5ca39e4065914fe7bb2a56dcbbfb6ae67c95cf444140edfc`,
`fe2bd835c8359810099881288608bc0055336f1ebb77e6715aa2946570181867`,
`a5460ce6a40756092d2e2dc91975ac5c2665c480370249084faa141d7b45eca8`, and
`062721eabd59d1d4b4bc5d4c18b3d6ee8e510c68d76473278e6cb60c5e2f7597`.

Boundary 35 reuses the same continuation for head order 9 (x42 / SIG 93 / Java Inter
1475). Shared Stem 2352 closes x41 LEFT then RIGHT, two ordered writes with no
unlinked head; native keeps SIG 680/691 and Stem bindings 41 and reaches
`current_index=10` before x65 / SIG 95 / Java Inter 1479. The v9 fixture SHA-256 is
`b0d3c67f9b76a56a528d8a962f3f2bc54710616f2e86650ac8500e750534ff2c`; runner,
transformed probe, emitted body, and semantic pass are
`368724efe73e194aff024d68204d758d089d81511e9bbfaa4dfb9ef9516f4c48`,
`caf6a7f25cc36cbe7480c7cb798a8c900bbd526fa7e4071d625724045bb88af5`,
`dbe0a891add7c613c340dfaee983a75c97b20cba4744ca19619e10cd9f7a78f5`, and
`8e271204197c0d84afe4948a94f6723f6a419cd9611495aa8ca74fb7731bbf95`.

Boundary 36 reuses the continuation for head order 10 (x65 / SIG 95 / Java Inter
1479). Shared Stem 2346 closes x64 LEFT then RIGHT, two ordered writes with no
unlinked head; native keeps SIG 680/691 and Stem bindings 41 and reaches
`current_index=11` before x46 / SIG 57 / Java Inter 1403. The v10 fixture SHA-256 is
`7b0bf32fcf75cf792eb67c2c8a52ae9702de215078a54bea7edc7cde853869d0`; runner,
transformed probe, emitted body, and semantic pass are
`ddf5b4c3f6d726c3e7d91de33d077930ff29254f1e7e84751ee391614978c464`,
`e7cf9dd3ceed19c3e387eabffb587005acb01725434776fe39501605ce4cd4af`,
`cbab4a06edd591e068007152dbb623d206a29c450aa2f9a153c75010fa184658`, and
`d5cd5dbed69852e48add157efd936ba8501879c30a023e730e0825c38825b712`.

Boundary 37 reuses the continuation for head order 11 (x46 / SIG 57 / Java Inter
1403). Shared Stem 2377 closes x44 LEFT/RIGHT then x45 LEFT/RIGHT, four ordered writes
with no unlinked head; native keeps SIG 680/691 and Stem bindings 41 and reaches
`current_index=12` before x55 / SIG 79 / Java Inter 1447. The v11 fixture SHA-256 is
`cad1527e556481a073ead938094de9edce09954e366bf5608ebc57a30ef946a3`; runner,
transformed probe, emitted body, and semantic pass are
`f05ea06f61193785a84440b457b4e79b10e7d88e765b81bce51d6f996beb1702`,
`24f67a53e407909d07e1fc12bb2e180b15e6dfcf74983d52d1326cff906284ca`,
`a0716a3379db5d268419624a193d6a6d1dc0105f78ff56fecd44fa70272165e4`, and
`eefa750fd63fa91fec84c2fd9afc62b82d51081da606a0687496a111f5059602`.

Boundary 38 reuses the continuation for head order 12 (x55 / SIG 79 / Java Inter
1447, grade bits `0x3fe847463fc14b09`). Shared Stem 2362 closes x51 LEFT/RIGHT then
x54 LEFT/RIGHT, four ordered writes with no unlinked head; native keeps SIG 680/691
and Stem bindings 41 and reaches `current_index=13` before x53 / SIG 3 / Java Inter
1291. The separate schema-v12 derivative is 12 lines / 8,273 bytes with seven semantic
rows plus summary, SHA-256
`e8b19156d29722a74b41e6d07d1591edd78b3077844f6be7268fa78754a1acd2`; runner,
transformed probe, emitted body, and semantic pass are
`74b6ba4f84c046ae2ca08e270ce9726acee42a14f4b639282bfbccd3c8b654d1`,
`7b8f232f56d92f83966311478de6b0255820d6d00c9aa4dbb3f0f9351c43abc6`,
`ab41455ece56d8cce145f1105a417315be379f3c6d644efca539d008db1c099a`, and
`ad4dd95c5b9c12f101a8c2420cca76902e7cc7571b3277bfbd879a6ba4bcda67`.
This derivative deliberately executes orders 1-11 only to reconstruct the predecessor
without emitting or persisting their full snapshots; only order 12 is emitted, keeping
the replay below the full-snapshot heap limit. Two fresh runs are byte-identical and
the base v11 fixture/runner remain pinned. This is bounded order-12 evidence, not
independent snapshot evidence for every predecessor or completion of the remaining
queue, actually-unlinked/retry behavior, phase-2 append, or broader C-link branches.

Boundary 43 reuses the continuation for head order 17 (x48 / SIG 29 / Java Inter 1343,
grade bits `0x3fe80cc40bda9d4c`). Shared Stem 2351 closes x47 LEFT then RIGHT, two
ordered writes with no unlinked head; native keeps SIG 680/691 and Stem bindings 41 and
reaches `current_index=18` before x63 / SIG 17 / Java Inter 1319. That next head starts
with both sides open/unlinked (`LEFT:false:false,RIGHT:false:false`); Boundary 43 does
not execute it. The separate schema-v17 derivative is 12 lines / 8,194 bytes with seven
semantic rows plus summary, SHA-256
`8e4909edc2196f2baff6f517693f9a9af50405cf85fc88bcf3e771711bae2b4b`;
runner, transformed probe, emitted body, and semantic pass are
`84c176b45ec8adb7af8e0ab1014acabfe8c57c2e6b3cbbe5e8bbd0e971823196`,
`b139149dd41b5581d96344617c2f52b49a85f085f011ff4b556b237f58765342`,
`2362b903486db2d4ddbc14aeeeb54761205bdd06a206875ef0c131a7a22e5fdd`, and
`c89f5a49456af435e2fb508e0ccbbd5a7b8fd9877616534cb7136ccd0ff84ecf`.
This derivative deliberately executes orders 1-16 only to reconstruct the predecessor
without emitting or persisting their full snapshots; only order 17 is emitted, keeping
the replay below the full-snapshot heap limit. Two fresh runs are byte-identical and
the base v16 fixture/runner remain pinned. This is bounded order-17 evidence, not
independent snapshot evidence for every predecessor or coverage of order 18,
actually-unlinked/retry behavior, phase-2 append, or broader C-link branches.

Boundary 44 consumes head order 18 (x63 / SIG 17 / Java Inter 1319, grade bits
`0x3fe8009e50c15bf8`) from the both-open/unlinked frontier. Java selects LEFT/BOTTOM,
expands a two-item builder (`lastIndex=maxIndex=1`) from active glyphs 328 and 2063,
reuses canonical glyph 328 without reinsertion, and creates checked Stem Inter 2381
with one HeadStem relation. Native creates dense Stem identity 41, moves SIG 680/691 to
681/692 and Stem bindings 41 to 42, links the LEFT S cell, records no unlinked head or
closure write, and reaches `current_index=19` before x69 / SIG 76 / Java Inter 1441.

The bounded geometry matches Java's RunTable centroid accumulation order and directly
interpolates the theoretical stem line's x coordinate at the centroid y before line
translation. Generic multi-item/recursive geometry, other corners, `reuseStem`, and
retry/no-link behavior remain ungraded. The focused Boundary-44 gate and full 14-test
sibling suite are green; formatting, strict all-target Clippy, and diff checks pass. The
snapshot-minimized schema-v18 derivative
is 14 lines / 11,751 bytes with nine semantic rows plus summary; orders 1-17 reconstruct
state without persisted full snapshots, while order 18 emits the C-link envelope/result
and continuation. Fixture, runner, transformed probe, emitted-body, and semantic-pass
SHA-256 are `4972836c5e2718f9441a007840cfc5100caa95a12dc349d7822c0695ad0f5b2b`,
`3bea814e71ba13374130351d0f5cc057779e5676e402e7b43b5c4ee4a263e332`,
`4e15aa27d982b6ea848b5a7349819e3db7300349dded652f859492abe2ea7460`,
`499b791dc34d2ca59666bbab20e4ca15a9dd335260d4714dbdd9042ed00456cd`, and
`7045d9060ea8e6d930b94d28e79e3e6d8d0cc0bb0b57bb20c64a3780b876bcb3`;
fragment source SHA-256 is
`f56fdd58606c3d5101ebea1690162b38f9db6a18f89a4fe0e441cedff1bac36c`.

Boundary 45 carries head order 19 (x69 / SIG 76 / Java Inter 1441, grade bits
`0x3fe7fe09c1461c49`). LEFT is already linked and RIGHT is open/unlinked, so Java
reports `SkipAlreadyLinked` then `Neither`. Shared Stem 2347 closes x68 LEFT then
RIGHT through two ordered false-to-true writes. Native records no unlinked head, keeps
SIG 681/692, system stems 42, and the relation hash unchanged, and reaches
`current_index=20` before x74 / SIG 19 / Java Inter 1323 (grade bits
`0x3fe7f8f93b5cf200`), whose sides are both open/unlinked. Order 20 is not executed.

The focused Boundary-45 gate and full 14-test sibling suite are green; formatting,
strict all-target Clippy, and diff checks pass. The snapshot-minimized schema-v19
derivative is 15 lines / 13,004 bytes with ten semantic rows plus summary: orders 1-18
reconstruct state without emitted or persisted full snapshots, while order 19 alone
emits the new continuation. Fixture, runner, transformed probe, emitted-body, and
semantic-pass SHA-256 are
`6d415102995fd1fda8057fab27b0f2a3a6cb2367cbcce52269009f377bf672ae`,
`b79cb0c5cba1d3b1275dd943d7945722a5f025281686362d6b40a311d3ad5335`,
`e94082b8faa8a8c26e70b00acd42bc091e7c9333317caa5299f6d18083cba781`,
`3ae97b86466a49fafbe07f5c32d5641824099e677131fff14aee3797f61cc3a9`, and
`9628fefbc7e1c88ab184aa711e329b9606e4d57252965428b9f3f33e96852a31`;
the Java fragment source remains pinned by
`f56fdd58606c3d5101ebea1690162b38f9db6a18f89a4fe0e441cedff1bac36c`.
This is bounded order-19 evidence, not independent predecessor snapshots or coverage
of order 20, actually-unlinked/retry, phase-2 append, generic multi-item/recursive
C-linkers, or broader corpus/system behavior.

Boundary 46 consumes head order 20 (x74 / SIG 19 / Java Inter 1323, grade bits
`0x3fe7f8f93b5cf200`) from a both-open/unlinked frontier. Java selects LEFT/BOTTOM,
expands a two-item builder (`lastIndex=maxIndex=1`) from active glyphs 332 and 2301,
reuses canonical glyph 332, and creates checked Stem Inter 2382 with one HeadStem
relation. Native creates dense Stem identity 42, moves SIG 681/692 to 682/693 and
system stems 42 to 43, links LEFT, records no closure write or unlinked head, and
reaches `current_index=21` before x28 / SIG 55 / Java Inter 1399 (grade bits
`0x3fe7e38e38e38e39`), whose LEFT side is linked and RIGHT remains open/unlinked.

Geometry remains case-bounded: the authenticated two-item centroid/interpolation path
applies Java `nextDown` to both translated x coordinates only at x74. Generic
multi-item/recursive geometry and other corner shapes remain open. The focused
Boundary-46 gate and full 14-test sibling suite are green; formatting, strict
all-target Clippy, and diff checks pass. The snapshot-minimized schema-v20 derivative
is 16 lines / 14,117 bytes with eleven semantic rows plus summary; orders 1-19
reconstruct without emitted or persisted full snapshots and only order 20 emits the
C-link envelope/result and continuation. Fixture/runner/probe/body/semantic pins are
`be6a820b3740105e4fdddeb0e9ec475d1dd3ebc8611fd7be555cf55957dfe4a4`,
`54468f53de6c0d1d931e391640642f55ce6c4733721df569ef6f10ef93704497`,
`40ced3035bdb19298e925b499edce42365aca66586abe7f8756847f32a1abd82`,
`3b1f4c53462e4ff8241863e73c90043d813cf5709cd0f3c809858659d7261564`, and
`dbd1c398b3ab3565a75ab9ed6dfa276b3493c52a6dd22a9a54ad09dc5e89e4d5`;
fragment source is pinned by
`5fa3ac22fe21091c313135909f13c793be575fd460f0af3349345ba8ede9ab3e`.
This is bounded order-20 evidence, not independent predecessor snapshots or coverage
of order 21, actually-unlinked/retry, phase-2 append, generic multi-item/recursive
C-linkers, or broader corpus/system behavior.

Boundary 47 carries head order 21 (x28 / SIG 55 / Java Inter 1399, grade bits
`0x3fe7e38e38e38e39`). Its authenticated LEFT/BOTTOM C-link envelope finds active
glyph 300 already owned by Stem Inter 2378, with two planned relations and one glyph.
Java leaves allocator 2382 unchanged and adds no vertex, edge, or system stem. Phase-1
continuation observes LEFT already linked and RIGHT `Neither`, then closes x27 / SIG 54
LEFT and RIGHT through two ordered writes. Native keeps SIG 682/693 and system stems
43, records no unlinked head, and reaches `current_index=22` before x4 / SIG 7 / Java
Inter 1299 (grade bits `0x3fe7dcd4cd6e88ba`), whose LEFT side is linked and RIGHT
remains open/unlinked.

The wrapper authenticates only this existing-stem retry and its graph-derived closure;
generic retry and no-link behavior remain open. The focused Boundary-47 gate and full
14-test sibling suite are green; formatting, strict all-target Clippy, and diff checks
pass. The snapshot-minimized schema-v21 derivative is 17 lines / 14,834 bytes with
twelve semantic rows plus summary; orders 1-20 reconstruct without emitted or
persisted full snapshots and only order 21 emits the retry envelope/result and
continuation. Fixture/runner/probe/body/semantic pins are
`9505955ce7e3322cbfaea818d0d42b5873fa78b1f5e1941756bcc44efcb04f55`,
`8cbd5d1de2e6e6b2b77d4ba94d99eb9f5813503a4afb960bb7511d0b92999ccd`,
`186e9fb81f3b39d1591b23b5f94c565152bfc81dc1d0e4781d460b1126f3ac4a`,
`9ea8929d70f49d8a39636ffece251ad1e13b3a443cdce57b62138f6ef0075293`, and
`a372eb0884f3679e62797343800beb70e8099c14267067f6d141f8c359216611`;
fragment source is pinned by
`f6a36215a86d9af177447069be271b0c4a84e4f8f56789d27769c161710c3629`.
This is bounded order-21 evidence, not independent predecessor snapshots or coverage
of order 22, actually-unlinked/no-link, phase-2 append, generic retry or
multi-item/recursive C-linkers, or broader corpus/system behavior.

Boundary 48 carries head order 22 (x4 / SIG 7 / Java Inter 1299, grade bits
`0x3fe7dcd4cd6e88ba`). Its authenticated LEFT/BOTTOM envelope has
`lastIndex=maxIndex=2`, two planned relations, and active glyphs 315 and 2142; canonical
glyph 315 is already owned by SIG-attached Stem Inter 2354. Java leaves allocator 2382
unchanged and adds no vertex, edge, glyph, or system stem. Continuation observes LEFT
already linked and RIGHT `Neither`, closes x3 / SIG 6 LEFT then RIGHT through two
ordered writes, and reaches `current_index=23` before x78 / SIG 39 / Java Inter 1363
(grade bits `0x3fe7d236c1f8e275`). Native keeps SIG 682/693 and system stems 43 and
records no unlinked head.

The wrapper authenticates only this retry and its graph-derived closure, including the
presence and SIG attachment of Stem 2354/glyph 315; generic retry and no-link remain
open. The focused Boundary-48 gate and full 14-test sibling suite are green;
formatting, strict all-target Clippy, and diff checks pass. The snapshot-minimized
schema-v22 derivative is 18 lines / 16,188 bytes with thirteen semantic rows plus
summary; orders 1-21 reconstruct without emitted or persisted full snapshots and only
order 22 emits the retry envelope/result and continuation. Fixture/runner/probe/body/
semantic pins are
`e7bd66417228bf8fed7fe0c04d904e81ade4026fb00b4c17270b73947f85a1a4`,
`be1091ab266ea190a507291351f50bec4842f50003c75fb048f6bb96537ceebc`,
`fc6ada7afdc64f1e42f9fbf0c1f9353138a02ec285d24697fc68a90d49c3dfc7`,
`23d5da366efe5ce9d1bee9e7c5e3201677faef273075e23af68332a5e1f7e4bb`, and
`62c5ac9c30ea6bf3666cdb567bfa52d6d0a857578a5146ac91927f08adfa8c6a`;
the corrected fragment source is pinned by
`576406fb3bd8bf9503ca883480bc55b217b3c6bc99ca440ef702774d3a2ca950`.
This is bounded order-22 evidence, not independent predecessor snapshots or coverage
of order 23, actually-unlinked/no-link, phase-2 append, generic retry or broader
C-linkers, or broader corpus/system behavior.

Boundary 49 adds no production operation. The existing continuation carries head order
23 (x78 / SIG 39 / Java Inter 1363, grade bits `0x3fe7d236c1f8e275`). LEFT is
already linked and RIGHT is `Neither`; incident Stem 2370 joins x77 / SIG 38 and x78 /
SIG 39 on LEFT, so x77 LEFT and RIGHT close through two ordered writes. Native keeps
SIG 682/693 and system stems 43, records no unlinked head, and reaches
`current_index=24` before x93 / SIG 25 / Java Inter 1335 (grade bits
`0x3fe7d1c13d1c13d2`), whose LEFT is linked and RIGHT remains open/unlinked.

This is evidence for the unchanged generic prelinked-success path, not a new retry
implementation. The focused Boundary-49 gate and full 14-test sibling suite are green;
formatting, strict all-target Clippy, and diff checks pass. The snapshot-minimized
schema-v23 derivative is 19 lines / 17,401 bytes with fourteen semantic rows plus
summary; orders 1-22 reconstruct without emitted or persisted full snapshots and only
order 23 emits the closure and continuation. Fixture/runner/probe/body/semantic pins
are `20731b3ff52e2512407f17c00329e16f015aaedba7bf5c91ec1b0b9907c58e68`,
`b945062e6c069c975f738ee066bc42107b4b6af599b5097abd0423bbb232aa25`,
`8c6694e8f0c9d293db056b515f51d5393b6a9a860d002e4033cafc8881f768af`,
`e8032bae1ffee2113b72b8a359d5c25cc219a84f1bd6a89485632138db42540f`, and
`093645a4a4ffe760113cfd15776c7e0eb61381b405b66a2f5789999a29927f38`;
the shared v22/v23 fragment source remains pinned by
`576406fb3bd8bf9503ca883480bc55b217b3c6bc99ca440ef702774d3a2ca950`.
This is bounded order-23 evidence, not independent predecessor snapshots or coverage
of order 24, actually-unlinked/no-link, phase-2 append, generic retry or broader
C-linkers, or broader corpus/system behavior.

Boundary 50 adds no production operation. The existing continuation carries order 24
(x93 / SIG 25 / Java Inter 1335, grade bits `0x3fe7d1c13d1c13d2`). LEFT is already
linked and RIGHT is `Neither`; incident Stem 2342 joins x92 / SIG 24 and x93 / SIG 25
on LEFT, so x92 LEFT then RIGHT close through two ordered writes. Native keeps SIG
682/693 and system stems 43, records no unlinked head, and reaches
`current_index=25` before x59 / SIG 74 / Java Inter 1437 (grade bits
`0x3fe7c31e7e01c29a`), whose LEFT is linked and RIGHT remains open/unlinked.

This is further evidence for the unchanged generic prelinked-success path, not a new
retry implementation. The focused Boundary-50 gate and full 14-test sibling suite are
green; formatting, strict all-target Clippy, and diff checks pass. The
snapshot-minimized schema-v24 derivative is 20 lines / 18,614 bytes with fifteen
semantic rows plus summary; orders 1-23 reconstruct without emitted or persisted full
snapshots and only order 24 emits the closure and continuation. Fixture/runner/probe/
body/semantic pins are
`56684be47b32b49e3d6f3c1440a9f3062a6bdcdec28fa0554cc6f2be80242b6c`,
`2d2a7b2b58f674bdf3db3716a6e66eac1b9d56694df7c79d7ec91ff7cb629293`,
`24f9bab608b05b89f0a28198b19827cfc0d241a0fd558298564e24f868b30872`,
`65d329d75ac1d9fff1fba2d13b9b418346645bbbcf3637061f95901039a0fac5`, and
`15cadab070e039fdb0753fcb57cc0e1aeb9012d0d19773eb701a47fc982d582e`;
the shared v22-v24 fragment source remains pinned by
`576406fb3bd8bf9503ca883480bc55b217b3c6bc99ca440ef702774d3a2ca950`.
This is bounded order-24 evidence, not independent predecessor snapshots or coverage
of order 25, actually-unlinked/no-link, phase-2 append, generic retry or broader
C-linkers, or broader corpus/system behavior.

Boundary 51 adds no production operation. The existing continuation carries order 25
(x59 / SIG 74 / Java Inter 1437, grade bits `0x3fe7c31e7e01c29a`). LEFT is already
linked and RIGHT is `Neither`; incident Stem 2363 joins x58 / SIG 73 and x59 / SIG 74
on LEFT, so x58 LEFT then RIGHT close through two ordered writes. Native keeps SIG
682/693 and system stems 43, records no unlinked head, and reaches
`current_index=26` before x61 / SIG 31 / Java Inter 1347 (grade bits
`0x3fe7b8475abaafaf`), whose LEFT is linked and RIGHT remains open/unlinked.

This is further evidence for the unchanged generic prelinked-success path, not a new
retry implementation. The focused Boundary-51 gate and full 14-test sibling suite are
green; formatting, strict all-target Clippy, and diff checks pass. The
snapshot-minimized schema-v25 derivative is 21 lines / 19,854 bytes with sixteen
semantic rows plus summary; orders 1-24 reconstruct without emitted or persisted full
snapshots and only order 25 emits the closure and continuation. Fixture/runner/probe/
body/semantic pins are
`39ccb74b6231aa2ce3f77a41adb59d18ae64c736598917523f4c4f8835722d2d`,
`d9bb5989503627cf7486f6c3286ffe78754a1a089d1d18087fef1e6d15389c68`,
`d30b66790a5b3b9cfc3aa9da27908aa90a1018d6a7fedd0f7c7029e0f6cbb69d`,
`c361bb73ac81783c8b0862490582fb4a6384ca98b845a2f85ccbf42c77da02f2`, and
`34d99daf8ee4b8b670c52a4ea28cf1bfae406f2bbb9904e5595208f0b0188fc8`;
the shared v22-v25 fragment source remains pinned by
`576406fb3bd8bf9503ca883480bc55b217b3c6bc99ca440ef702774d3a2ca950`.
This is bounded order-25 evidence, not independent predecessor snapshots or coverage
of order 26, actually-unlinked/no-link, phase-2 append, generic retry or broader
C-linkers, or broader corpus/system behavior.

Boundary 52 adds no production operation. The existing continuation carries order 26
(x61 / SIG 31 / Java Inter 1347, grade bits `0x3fe7b8475abaafaf`). LEFT is already
linked and RIGHT is `Neither`; incident Stem 2345 joins x60 / SIG 30 and x61 / SIG 31
on LEFT, so x60 LEFT then RIGHT close through two ordered writes. Native keeps SIG
682/693 and system stems 43, records no unlinked head, and reaches
`current_index=27` before x33 / SIG 26 / Java Inter 1337 (grade bits
`0x3fe7a22f6f5852b0`), whose sides are both open/unlinked; this boundary does not
execute that next frontier.

This is further evidence for the unchanged generic prelinked-success path, not a new
retry implementation. The focused Boundary-52 gate and full 14-test sibling suite are
green; formatting, strict all-target Clippy, and diff checks pass. The
snapshot-minimized schema-v26 derivative is 22 lines / 21,096 bytes with seventeen
semantic rows plus summary; orders 1-25 reconstruct without emitted or persisted full
snapshots and only order 26 emits the closure and continuation. Fixture/runner/probe/
body/semantic pins are
`a5e6a9cb07b49ecf1753fbe10ba709a63d274dce5393887acddc123e55342c36`,
`afe60083e9b34076c7aab0106216eb5dac7ba689c63ef388112f7b700f842ed0`,
`d794e14d3715c64e7e9b3364fbf1a29389a4bd327da577e7313ce0de4eafdaa8`,
`8220b597632c878f90e6ebb8bf4f84ac4beda6a2458c07056663075520ff2f73`, and
`da5cfb3439d4efec0cbd64299cf037927ab4cea76a20c1c740bdee0780916a49`;
the shared v22-v26 fragment source remains pinned by
`576406fb3bd8bf9503ca883480bc55b217b3c6bc99ca440ef702774d3a2ca950`.
This is bounded order-26 evidence, not independent predecessor snapshots or coverage
of order 27, actually-unlinked/no-link, phase-2 append, generic retry or broader
C-linkers, or broader corpus/system behavior.

Boundary 53 consumes order 27 (x33 / SIG 26 / Java Inter 1337, grade bits
`0x3fe7a22f6f5852b0`) from a both-open/unlinked frontier. Java selects LEFT/BOTTOM,
expands a two-item builder (`lastIndex=maxIndex=1`) from active glyphs 314 and 2219,
reuses canonical glyph 314, and creates checked Stem Inter 2383 with one HeadStem
relation. Native creates dense Stem identity 43, moves SIG 682/693 to 683/694 and
system stems 43 to 44, records no closure write or unlinked head, and reaches
`current_index=28` before x85 / SIG 87 / Java Inter 1463 (grade bits
`0x3fe79e7f455ba48d`), whose LEFT is linked and RIGHT remains open/unlinked.

Geometry remains bounded to this two-item LEFT/BOTTOM case. The focused Boundary-53
gate and full 14-test sibling suite are green; formatting, strict all-target Clippy,
and diff checks pass. The snapshot-minimized schema-v27 derivative is 25 lines / 25,740
bytes with twenty semantic rows plus summary; orders 1-26 reconstruct without emitted
or persisted full snapshots and only order 27 emits its C-link envelope/result and
continuation. Fixture/runner/probe/body/semantic pins are
`1ba59491992fdd7bd2355e2617b437b84433d3c449cc8f7606cdc0a1e70ac0aa`,
`f2c1942b3ff6f00a75bb876b6d6d4b53ba2d999bcb5ddaeb88f6dc86850fcdc5`,
`5f4c5a69c9fe5e87f23eff31b1524e80459a04a298689609fa80ef142f1cd9c6`,
`bd006771fb4878072bb24f54cc22efd507dd5114d5e60fccff76479b2cb25c1c`, and
`1033282335cace626465424615847b3e190c718f25acf2fd70e1a6a2d50ec7d7`;
fragment source is pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-27 evidence, not independent predecessor snapshots or coverage
of order 28, actually-unlinked/no-link, phase-2 append, generic retry or broader
C-linkers, or broader corpus/system behavior.

Boundary 54 adds no production operation. The existing continuation carries order 28
(x85 / SIG 87 / Java Inter 1463, grade bits `0x3fe79e7f455ba48d`). LEFT is already
linked and RIGHT is `Neither`; incident Stem 2366 joins x84, x85, and x86 on LEFT, so
x84 LEFT/RIGHT and x86 LEFT/RIGHT close through four ordered writes. Native keeps SIG
683/694 and system stems 44, records no unlinked head, and reaches
`current_index=29` before x10 / SIG 9 / Java Inter 1303 (grade bits
`0x3fe79713252eb76a`), whose LEFT is linked and RIGHT remains open/unlinked.

The default full-snapshot order-28 oracle exhausted the JVM heap. The replacement runs
orders 1-27 as mutations without snapshots and emits only the authenticated order-0
baseline/C-link evidence plus the order-28 closure row; it does not independently
snapshot-oracle the predecessor sequence. The focused Boundary-54 gate and full
14-test sibling suite are green; formatting, strict all-target Clippy, and diff checks
pass. The schema-v28 derivative is 12 lines / 8,381 bytes with seven semantic rows plus
summary. Fixture/runner/probe/body/semantic pins are
`6f30a5cb8706fb0445b5eb84cee2896dfa1b85236f6870a97177714672ef10b7`,
`ec1985d786f0c984f5a09a461008911f12777229b0a08eb71b7e36a39d548d82`,
`d2e07d5dacf3e22ec20a3f53c8e4543763982eec3e88eac1ac8e8e3368422cc2`,
`5a4675dca2831e93c61a028a6d189deed21115e9588e06b1293c37968fd2bef5`, and
`b4d16e19a892bfb0537f8b7b629e43617687f19794a2cf13332a0e69cdd4e1fd`;
the shared v27/v28 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-28 evidence, not coverage of order 29, actually-unlinked/no-link,
phase-2 append, generic retry or broader C-linkers, or broader corpus/system behavior.

Boundary 55 adds no production operation. The existing continuation carries order 29
(x10 / SIG 9 / Java Inter 1303, grade bits `0x3fe79713252eb76a`). LEFT is already
linked and RIGHT is `Neither`; incident Stem 2355 joins x9 and x10 on LEFT, so x9 LEFT
then RIGHT close through two ordered writes. Native keeps SIG 683/694 and system stems
44, records no unlinked head, and reaches `current_index=30` before x101 / SIG 43 /
Java Inter 1371 (grade bits `0x3fe79406c6921d2e`), whose LEFT is linked and RIGHT
remains open/unlinked.

The v29 oracle retains v28's heap-safe minimized shape: orders 1-28 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-29
closure row are emitted. It does not independently snapshot-oracle the predecessor
sequence. The focused Boundary-55 gate and full 14-test sibling suite are green;
formatting, strict all-target Clippy, and diff checks pass. The schema-v29 derivative
is 12 lines / 8,292 bytes with seven semantic rows plus summary. Fixture/runner/probe/
body/semantic pins are
`a88b9fd3c27133c3c8bdcc839308365557c0e95c2ac3ea83fe348dc0d1ffa270`,
`0ae5afb409d11eef138ed62bb8adbefb04eabfa99c0581cad7a6952ecb5e1d4c`,
`79ddfc2cf532474ff902156eb66c2655ec242ac0a73884fd67bfc74afb6521ca`,
`410d0c1e04f4c7dfb1b4b83ed0953da53e52605df72e08351231e302027ca84a`, and
`32c937944c1c015c79ca4993dd299bef9c32ef39e7be71c800ca98d21ccd5cde`;
the shared v27-v29 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-29 evidence, not coverage of order 30, actually-unlinked/no-link,
phase-2 append, generic retry or broader C-linkers, or broader corpus/system behavior.

Boundary 56 adds no production operation. The existing continuation carries order 30
(x101 / SIG 43 / Java Inter 1371, grade bits `0x3fe79406c6921d2e`). LEFT is already
linked and RIGHT is `Neither`; incident Stem 2343 joins x100 and x101 on LEFT, so x100
LEFT then RIGHT close through two ordered writes. Native keeps SIG 683/694 and system
stems 44, records no unlinked head, and reaches `current_index=31` before x16 / SIG 81
/ Java Inter 1451 (grade bits `0x3fe75f1fc300149f`), whose LEFT is linked and RIGHT
remains open/unlinked.

The v30 oracle retains v28's heap-safe minimized shape: orders 1-29 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-30
closure row are emitted. It does not independently snapshot-oracle the predecessor
sequence. The focused Boundary-56 gate and full 14-test sibling suite are green;
formatting, strict all-target Clippy, and diff checks pass. The schema-v30 derivative
is 12 lines / 8,306 bytes with seven semantic rows plus summary. Fixture/runner/probe/
body/semantic pins are
`c4bde8384b872a03d7f9d7ecd87fdea60dc93a5b418ca831c8dbe5d8c3aa729d`,
`d8f55efad82e15eb8b45c52ac8f99031c00ea0dd7143bc30c7c607fc103e71cf`,
`a8b50543359666567a01d503f46616d113feee03ac60828104d2b52efc558812`,
`8eebd2a60dfdaf3896a31d7200525fc70667bed42ee8cbcc0076830bae74bd40`, and
`803635259310df2794ac302b43a7b8286c95fb117f6f19177071c1ce25d484a9`;
the shared v27-v30 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-30 evidence, not coverage of order 31, actually-unlinked/no-link,
phase-2 append, generic retry or broader C-linkers, or broader corpus/system behavior.

Boundary 57 adds no production operation. The existing continuation carries order 31
(x16 / SIG 81 / Java Inter 1451, grade bits `0x3fe75f1fc300149f`). LEFT is already
linked and RIGHT is `Neither`; incident Stem 2360 joins x15 and x16 on LEFT, so x15
LEFT then RIGHT close through two ordered writes. Native keeps SIG 683/694 and system
stems 44, records no unlinked head, and reaches `current_index=32` before x34 / SIG 77
/ Java Inter 1443 (grade bits `0x3fe75353cd1ba641`), whose LEFT is linked and RIGHT
remains open/unlinked.

The v31 oracle retains v28's heap-safe minimized shape: orders 1-30 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-31
closure row are emitted. It does not independently snapshot-oracle the predecessor
sequence. The focused Boundary-57 gate and full 14-test sibling suite are green;
formatting, strict all-target Clippy, and diff checks pass. The schema-v31 derivative
is 12 lines / 8,302 bytes with seven semantic rows plus summary. Fixture/runner/probe/
body/semantic pins are
`ab58a7bf7d5a2265fbd8cc2a18ee0595b7d288935469cf27f91e01ace9397b00`,
`e7b8cd3bc87ff55969aee203b6027f7af572428cf91d442f94ea58e8f82d3e42`,
`231028452d789e78ec96e5dc1c2f8ccabe88d85ac59aa9f990e18a0775d44404`,
`34baf86107a36d017519d7ac0f0011a0eb8d67f93a5d9b2d95f55ccf0784dcc4`, and
`3d123d0fcd70cdcdc3436a1ffca7b85ecac9e1a350c6a83368f91175e35eb4e4`;
the shared v27-v31 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-31 evidence, not coverage of order 32, actually-unlinked/no-link,
phase-2 append, generic retry or broader C-linkers, or broader corpus/system behavior.

Boundary 58 adds no production operation. The existing continuation carries order 32
(x34 / SIG 77 / Java Inter 1443, grade bits `0x3fe75353cd1ba641`). LEFT is already
linked and RIGHT is `Neither`; incident Stem 2368 contains only x34 on LEFT, so Java
returns with no closure writes or changed linker values. Native keeps SIG 683/694 and
system stems 44, records no unlinked head, and reaches `current_index=33` before x88 /
SIG 84 / Java Inter 1457 (grade bits `0x3fe73605f8f111a6`), whose LEFT is linked and
RIGHT remains open/unlinked.

The v32 oracle retains v28's heap-safe minimized shape: orders 1-31 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-32
no-op closure row are emitted. It does not independently snapshot-oracle the
predecessor sequence. The focused Boundary-58 gate and full 14-test sibling suite are
green; formatting, strict all-target Clippy, and diff checks pass. The schema-v32
derivative is 12 lines / 8,230 bytes with seven semantic rows plus summary. Fixture/
runner/probe/body/semantic pins are
`cceda3e1b00ccf9e4ca5f701c71a0a4da4e764488e192bf056ea645f11ad72c4`,
`fecd661b0c9b9e03f17c9eba3482a86b7f2ae381e49ac93bbbcbfea4756c3cd8`,
`d1b3d61c46bfdfe540d33ae751d0006c4518142d8410277d8e4016a4b29b1fe5`,
`77810c34e97279aef05feaa043df82a7ab4ba1566edc5933f38ff80608f10191`, and
`23a5e25617ef107a7f4b2b85ddb977d8d8b164d0203477d9f995d2d90df55bf5`;
the shared v27-v32 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-32 evidence, not coverage of order 33, actually-unlinked/no-link,
phase-2 append, generic retry or broader C-linkers, or broader corpus/system behavior.

Boundary 59 adds no production operation. The existing continuation carries order 33
(x88 / SIG 84 / Java Inter 1457, grade bits `0x3fe73605f8f111a6`). LEFT is already
linked and RIGHT is `Neither`; incident Stem 2367 joins x87 and x88 on LEFT, so x87
LEFT then RIGHT close through two ordered writes. Native keeps SIG 683/694 and system
stems 44, records no unlinked head, and reaches `current_index=34` before x2 / SIG 36
/ Java Inter 1357 (grade bits `0x3fe71d98bc61a5b3`), whose LEFT and RIGHT are both
open/unlinked.

The v33 oracle retains v28's heap-safe minimized shape: orders 1-32 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-33
closure row are emitted. It does not independently snapshot-oracle the predecessor
sequence. The focused Boundary-59 gate and full 14-test sibling suite are green;
formatting, strict all-target Clippy, and diff checks pass. The schema-v33 derivative
is 12 lines / 8,302 bytes with seven semantic rows plus summary. Fixture/runner/probe/
body/semantic pins are
`a058341d3f661be4a677206c7a067f39a0785ae5adeed96be7d7073541fe2982`,
`472e88ea561df7db9280c5ec79a2ea8a5204783d3ffec8894455adcf5b342692`,
`2e4a07c2efbdf0e43bb92f9bd6213cd6faf3e2a0e39eed610f11e13e15e42d72`,
`2c212c41b06dc509b217ced1e3c0bedfd6c538f3684a705eeafc3e60ff33aed4`, and
`8aec2db3857f6d2d8dc60bdb381ce3cfb8a16a1e441efd348854489bbcc53b43`;
the shared v27-v33 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-33 evidence, not coverage of order 34, its both-open C-link
geometry, actually-unlinked/no-link, phase-2 append, generic retry, or broader
corpus/system behavior.

Boundary 60 consumes the authenticated both-open order-34 frontier for x2 / SIG 36 /
Java Inter 1357 (grade bits `0x3fe71d98bc61a5b3`). Its LEFT/BOTTOM C-link selects
active glyphs 322 and 1946, reuses glyph 322 as the modeled candidate, creates Java
Stem 2384 / native Stem identity 44, and adds the Inter1357-to-Stem2384 relation.
Native advances SIG 683/694 to 684/695 and system stems 44 to 45, records no unlinked
head or closure write, and reaches `current_index=35` before x50 / SIG 72 / Java Inter
1433 (grade bits `0x3fe6dc9c073bac4e`), whose LEFT is linked and RIGHT remains
open/unlinked.

The measured geometry correction is deliberately narrow: Java rounds both translated
stem-line x coordinates one representable step above direct native interpolation, so
`java_next_up` applies only to the authenticated x2 frontier. The v34 oracle retains
the heap-safe minimized shape: orders 1-33 mutate without snapshots, while order-0
authentication and the order-34 frontier/result/continuation are emitted. It does not
independently snapshot-oracle the predecessor sequence. The focused Boundary-60 gate
and full 14-test sibling suite are green; formatting, strict all-target Clippy, and
diff checks pass.

The schema-v34 derivative is 14 lines / 11,693 bytes with nine semantic rows plus
summary. Fixture/runner/probe/body/semantic pins are
`b67514520fa848fd9758d0bdc740d2be4600c723ac341b57fced42f4657103a8`,
`60b4cc5a9e0a9fe5c6d4a8bb1b03bfadf065259c07bc124c6587b3d7a9c3a93f`,
`4cec5bfe6379e31701b7e4ea4f2ad98a8d36680daefa7a8a8d9d4c179d2c6777`,
`85d05cca18e6b15414729404191bd84d0729bf657678b0dfaa626ab72b915ae4`, and
`486957d6a77dce18fc15bd92761e5624e6b8edef9705d35403f00419b011b4dd`;
the shared v27-v34 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-34 evidence, not coverage of order 35, generic multi-item or
recursive C-link geometry, actually-unlinked/no-link, phase-2 append, generic retry,
or broader corpus/system behavior.

Boundary 61 adds no production operation. The existing continuation carries order 35
(x50 / SIG 72 / Java Inter 1433, grade bits `0x3fe6dc9c073bac4e`). LEFT is already
linked and RIGHT is `Neither`; incident Stem 2353 joins x49 and x50 on LEFT, so x49
LEFT then RIGHT close through two ordered writes. Native keeps SIG 684/695 and system
stems 45, records no unlinked head, and reaches `current_index=36` before x23 / SIG 14
/ Java Inter 1313 (grade bits `0x3fe6bf73ff00cd94`), whose LEFT and RIGHT are both
open/unlinked.

The v35 oracle retains the heap-safe minimized shape: orders 1-34 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-35
closure row are emitted. It does not independently snapshot-oracle the predecessor
sequence. The focused Boundary-61 gate and full 14-test sibling suite are green;
formatting, strict all-target Clippy, and diff checks pass. The schema-v35 derivative
is 12 lines / 8,302 bytes with seven semantic rows plus summary. Fixture/runner/probe/
body/semantic pins are
`2721b843514ce7a695fdacc797addd21597bd604b39168fe63533ecfc01bd55b`,
`74aec11451cb5933938b3bc82876ddfdb9e4bdab295e472644698c68d2cbc5ea`,
`611a02c34f4690031db91ce7ccced19ef6a1d7ec3d6da0dd81333f07aa315b42`,
`12d32f9193480ac9772e735735210b3689266458f4dad379a72131bb9024cc84`, and
`992372127972f17882a8f672653b9b4530497d06c8c6a43f6209ad6c8e22a1dd`;
the shared v27-v35 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-35 evidence, not coverage of order 36, its both-open C-link
geometry, actually-unlinked/no-link, phase-2 append, generic retry, or broader
corpus/system behavior.

Boundary 62 carries order 36 (x23 / SIG 14 / Java Inter 1313, grade bits
`0x3fe6bf73ff00cd94`) through its LEFT/BOTTOM C-link. Active glyph 324 is reused to
create Java Stem 2385 / native Stem identity 46 and relation edge 1313. Native moves
SIG 684/695 to 685/696 and system stems 45 to 46, records no closure write or
unlinked head, and reaches `current_index=37` before x14 / SIG 1 / Java Inter 1287
(grade bits `0x3fe6b52921e6cda3`), whose LEFT is linked and RIGHT remains open/unlinked.

The v36 oracle keeps orders 1-35 as mutations without snapshots and emits only
authenticated order-0 baseline/C-link evidence plus the order-36 frontier, result,
and continuation. It is not independent predecessor-snapshot evidence. The focused
Boundary-62 gate and full 14-test sibling suite are green; formatting, strict
all-target Clippy, and diff checks pass. The schema-v36 derivative is 14 lines /
11,600 bytes with nine semantic rows plus summary. Fixture/runner/probe/body/semantic
pins are `7d7d0d17e51c03a145bdff3a739da3aaaa05fb0c5bba20cd9a46468742eb26e7`,
`3176407de9bdd88f167e925a2b901f811f230f6b83c5a120ddf031a42ec49fd4`,
`582922fe7442de97a34732791352550e0026d9cf16cae36d633266eb15273aba`,
`7f61d1814c2542ae95f54515aa97a8f35ed3be2905e87336c15a83a0d8c6489b`, and
`f3e073ba83536e4afc1c0ea13a5933f199cfad57f1b82b6974f5abb9081039bd`;
the shared v27-v36 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-36 single-item LEFT/BOTTOM evidence, not coverage of order 37,
generic multi-item or recursive C-link geometry, actually-unlinked/no-link, phase-2
append, generic retry, or broader corpus/system behavior.

Boundary 63 carries order 37 (x14 / SIG 1 / Java Inter 1287, grade bits
`0x3fe6b52921e6cda3`). LEFT is already linked and its four-relation LEFT/BOTTOM
candidate resolves active glyph 294 to existing Stem 2340, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is `Neither`; incident Stem 2340 joins
x13 and x14 on LEFT, so x13 LEFT then RIGHT close through two ordered writes. Native
records no unlinked head and reaches `current_index=38` before x18 / SIG 4 / Java
Inter 1293 (grade bits `0x3fe6b1ad86c7d182`), whose LEFT is linked and RIGHT remains
open/unlinked.

The v37 oracle keeps orders 1-36 as mutations without snapshots and emits only
authenticated order-0 baseline/C-link evidence plus the order-37 frontier, result,
and continuation. It is not independent predecessor-snapshot evidence. The focused
Boundary-63 gate and full 14-test sibling suite are green; formatting, strict
all-target Clippy, and diff checks pass. The schema-v37 derivative is 14 lines /
12,303 bytes with nine semantic rows plus summary. Fixture/runner/probe/body/semantic
pins are `5af8e1928df00217e1780e2e6e0d057c4202b0f1cf46f25d5d889678c5fdf2b8`,
`2fac40e0bf6f49186a994bae499aa371be8bee2152297d325bae067c3f8d5bc1`,
`58ed9ebbd2fa05e9e52349b5ad42195a8f9fe534b46e088e6be7dd850d6ab1bb`,
`4c69d4c1740899bf4c71dbc895f022882f87d772233642f638ba3ecdc4db3fb1`, and
`fcb9fec2a764e9ab06d6b91ca856a8832ef236754dcb45ba345ae3f8f7280d90`;
the shared v27-v37 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-37 existing-stem reconciliation evidence, not coverage of order
38, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

Boundary 64 carries order 38 (x18 / SIG 4 / Java Inter 1293, grade bits
`0x3fe6b1ad86c7d182`). LEFT is already linked and its two-relation LEFT/BOTTOM
candidate resolves active glyph 310 to existing Stem 2372, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is `Neither`; incident Stem 2372 joins
x17 and x18 on LEFT, so x17 LEFT then RIGHT close through two ordered writes. Native
records no unlinked head and reaches `current_index=39` before x97 / SIG 34 / Java
Inter 1353 (grade bits `0x3fe666c6bb717a2e`), whose LEFT is linked and RIGHT remains
open/unlinked.

The v38 oracle keeps orders 1-37 as mutations without snapshots and emits only
authenticated order-0 baseline/C-link evidence plus the order-38 frontier, result,
and continuation. It is not independent predecessor-snapshot evidence. The focused
Boundary-64 gate and full 14-test sibling suite are green; formatting, strict
all-target Clippy, and diff checks pass. The schema-v38 derivative is 14 lines /
11,312 bytes with nine semantic rows plus summary. Fixture/runner/probe/body/semantic
pins are `98c8d3c19d50df531d756d6fd50ddbc9f07ce7db24bea47849fff731d5271b0f`,
`ad2edbfdf046db3a27b67d81da23f6f30d254cde9c91eb92063df72da10c7551`,
`8da7b91134b4ae654461eecd7f4f5009e3fe205f140663dad836b0820465a214`,
`64a375a90ec14b1e4735027c53a2f650774eb22f8ec6cc4884dacddf008ef859`, and
`57e46879aca3fc5a02851b590a14347df4535beff0b7c97855d42afe95155422`;
the shared v27-v38 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-38 existing-stem reconciliation evidence, not coverage of order
39, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

Boundary 65 carries order 39 (x97 / SIG 34 / Java Inter 1353, grade bits
`0x3fe666c6bb717a2e`). LEFT is already linked and its two-relation LEFT/BOTTOM
candidate resolves active glyph 321 to existing Stem 2373, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is `Neither`; incident Stem 2373 joins
x96 and x97 on LEFT, so x96 LEFT then RIGHT close through two ordered writes. Native
records no unlinked head and reaches `current_index=40` before x6 / SIG 89 / Java
Inter 1467 (grade bits `0x3fe65e4f5c70ff04`), whose LEFT is linked and RIGHT remains
open/unlinked.

The v39 oracle keeps orders 1-38 as mutations without snapshots and emits only
authenticated order-0 baseline/C-link evidence plus the order-39 frontier, result,
and continuation. It is not independent predecessor-snapshot evidence. The focused
Boundary-65 gate and full 14-test sibling suite are green; formatting, strict
all-target Clippy, and diff checks pass. The schema-v39 derivative is 14 lines /
11,315 bytes with nine semantic rows plus summary. Fixture/runner/probe/body/semantic
pins are `771b7816918d098e66fa1c599df1a68bfb3e24d1724ea6f701ba3bcc59b031fa`,
`bf7855c0d53d59cea3593de72f51f7272168f488e65148267ebd55e9f70110c7`,
`990556c3e12f99826c6ca92596045d44cec482263c76040613b8afc1bfd796d8`,
`6f3518552f431fd0108d3c64efc6d5c2a99cd57ff841f8cbcc2987ecb80c6090`, and
`2d51ffd86926e5a39870f9e5d1222d359f28121a4f5e9ccda9b072e5fd94b73b`;
the shared v27-v39 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-39 existing-stem reconciliation evidence, not coverage of order
40, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

Boundary 66 carries order 40 (x6 / SIG 89 / Java Inter 1467, grade bits
`0x3fe65e4f5c70ff04`). LEFT is already linked and its three-relation LEFT/BOTTOM
candidate resolves active glyph 290 to existing Stem 2348, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is `Neither`; incident Stem 2348 joins
x5 and x6 on LEFT, so x5 LEFT then RIGHT close through two ordered writes. Native
records no unlinked head and reaches `current_index=41` before x30 / SIG 67 / Java
Inter 1423 (grade bits `0x3fe63a0d1316bff0`), whose LEFT is linked and RIGHT remains
open/unlinked.

The v40 oracle keeps orders 1-39 as mutations without snapshots and emits only
authenticated order-0 baseline/C-link evidence plus the order-40 frontier, result,
and continuation. It is not independent predecessor-snapshot evidence. The focused
Boundary-66 gate and full 14-test sibling suite are green; formatting, strict
all-target Clippy, and diff checks pass. The schema-v40 derivative is 14 lines /
11,761 bytes with nine semantic rows plus summary. Fixture/runner/probe/body/semantic
pins are `26e4a2ecbd547829c573c4c7737331e4773f6faf64581ecfdf380a6b87283fa9`,
`7caaaf046770aafb327359fc587ed54509a83ec867a90a8c53cd254b2de5cb45`,
`36408206fc9d1f7640b1464ff9a95be6039ce77e21485891f0f889dd0cf52f84`,
`9be2634c8582ff4f023e17313aa9b91524b542d07c3c69363906b1d1e05acaa6`, and
`fa014228f89fbba214adaa1525ae8206de28f919ac71b334e2da01587f399db8`;
the shared v27-v40 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-40 existing-stem reconciliation evidence, not coverage of order
41, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

Boundary 67 carries order 41 (x30 / SIG 67 / Java Inter 1423, grade bits
`0x3fe63a0d1316bff0`). LEFT is already linked and its four-relation LEFT/BOTTOM
candidate resolves active glyph 313 to existing Stem 2357, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is `Neither`; incident Stem 2357 joins
x29 and x30 on LEFT, so x29 LEFT then RIGHT close through two ordered writes. Native
records no unlinked head and reaches `current_index=42` before x43 / SIG 48 / Java
Inter 1385 (grade bits `0x3fe5f802e7abc18c`), whose LEFT is linked and RIGHT remains
open/unlinked.

The v41 oracle keeps orders 1-40 as mutations without snapshots and emits only
authenticated order-0 baseline/C-link evidence plus the order-41 frontier, result,
and continuation. It is not independent predecessor-snapshot evidence. The focused
Boundary-67 gate is green 1/1; the full 14-test sibling suite, strict workspace/
all-target/all-features Clippy, formatting, and diff checks also pass. The schema-v41
derivative is 14 lines / 12,312 bytes with nine semantic rows plus summary.
Fixture/runner/probe/body/semantic pins are
`7bb4ebb479617804363078144c55570d1c76229de551492c7cb14050641f1962`,
`62be1da6161918739869d9aff57dd324b2145e6bfd6a96eb16fa8c64660c6a12`,
`0af9969baa054555c868a1f98c15010301280c31367d6b57985f7c6ce97a22b1`,
`44c4abe29f383cb0dd40f1e5777731d8384a1f97500f5f7d050d205cc48adf28`, and
`5165580a8c154740a61992c286ecd74fedf663a8cd177b0607c832033fca5827`;
the base v40 runner/fixture remain pinned by
`7caaaf046770aafb327359fc587ed54509a83ec867a90a8c53cd254b2de5cb45` and
`26e4a2ecbd547829c573c4c7737331e4773f6faf64581ecfdf380a6b87283fa9`,
and the shared v27-v41 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-41 existing-stem reconciliation evidence, not coverage of order
42, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

Boundary 68 carries order 42 (x43 / SIG 48 / Java Inter 1385, grade bits
`0x3fe5f802e7abc18c`). LEFT is already linked and its two-relation LEFT/BOTTOM
candidate resolves active glyph 326 to existing Stem 2350, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is `Neither`; incident Stem 2350 joins
x39, x40, and x43 on LEFT, so x39 LEFT then RIGHT and x40 LEFT then RIGHT close through
four ordered writes. Native records no unlinked head and reaches `current_index=43`
before x25 / SIG 91 / Java Inter 1471 (grade bits `0x3fe5db5645fe3490`), whose LEFT
is linked and RIGHT remains open/unlinked.

The v42 oracle keeps orders 1-41 as mutations without snapshots and emits only
authenticated order-0 baseline/C-link evidence plus the order-42 frontier, result,
and continuation. It is not independent predecessor-snapshot evidence. The focused
Boundary-68 gate is green 1/1; the full 14-test sibling suite, strict workspace
Clippy, formatting, and diff checks also pass. The schema-v42 derivative is 14 lines /
11,783 bytes with nine semantic rows plus summary. Fixture/runner/probe/body/semantic
pins are `64b55e449e38f7af6ed47c1ca026236772a277ac8c5917bc5eaea397125b332c`,
`b3e7d0f4399584faa4a180b87dcc95259114cb4821dd0fcee404739a577c31c0`,
`94231862206d686a7a0319ef2bcd6caca1516fbfd6e39a1281f24312d3c9ea04`,
`75faf38ec5983a45709de96fc20d02bed6fb03a8e77539aa6fbf067e78c9b612`, and
`4a8c708ec941174ed73d3fc12e5b8d71107a4b2628fb7755e14677a86bde83ae`;
the base v41 runner/fixture remain pinned by
`62be1da6161918739869d9aff57dd324b2145e6bfd6a96eb16fa8c64660c6a12` and
`7bb4ebb479617804363078144c55570d1c76229de551492c7cb14050641f1962`,
and the shared v27-v42 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-42 existing-stem reconciliation evidence, not coverage of order
43, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

Boundary 69 carries order 43 (x25 / SIG 91 / Java Inter 1471, grade bits
`0x3fe5db5645fe3490`). LEFT is already linked and its three-relation LEFT/BOTTOM
candidate resolves active glyph 292 to existing Stem 2356, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is `Neither`; incident Stem 2356 joins
x24 and x25 on LEFT, so x24 LEFT then RIGHT close through two ordered writes. Native
records no unlinked head and reaches `current_index=44` before x83 / SIG 21 / Java
Inter 1327 (grade bits `0x3fe5b836536dd665`), whose LEFT is linked and RIGHT remains
open/unlinked.

The v43 oracle keeps orders 1-42 as mutations without snapshots and emits only
authenticated order-0 baseline/C-link evidence plus the order-43 frontier, result,
and continuation. It is not independent predecessor-snapshot evidence. The focused
Boundary-69 gate is green 1/1; the full 14-test sibling suite, strict workspace
Clippy, formatting, and diff checks also pass. The schema-v43 derivative is 14 lines /
11,885 bytes with nine semantic rows plus summary. Fixture/runner/probe/body/semantic
pins are `dc5f7ce12d292a13cc149e7df0249703323df92de9054daf5eff52783b32919d`,
`421c8cd9b3a9208b509ce511077c6656faeac230212deff7ab797f6ffec73d75`,
`0f82e9cdee52ef1d8ac25870941d9437e4827c5ac0d6aca0af99934999fba250`,
`27ebf1a55921c288191423e18e6be2ed4f22c1a2d610b365a5809faa5606bbb3`, and
`45edd5fcc989fdc663f1d95ad379c07b165c058143d6c90c840b04d67dbf5bc3`;
the base v42 runner/fixture remain pinned by
`b3e7d0f4399584faa4a180b87dcc95259114cb4821dd0fcee404739a577c31c0` and
`64b55e449e38f7af6ed47c1ca026236772a277ac8c5917bc5eaea397125b332c`,
and the shared v27-v43 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-43 existing-stem reconciliation evidence, not coverage of order
44, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

Boundary 70 carries order 44 (x83 / SIG 21 / Java Inter 1327, grade bits
`0x3fe5b836536dd665`). LEFT is already linked and its two-relation LEFT/BOTTOM
candidate resolves active glyph 301 to existing Stem 2358, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is `Neither`; incident Stem 2358 joins
x82 and x83 on LEFT, so x82 LEFT then RIGHT close through two ordered writes. Native
records no unlinked head and reaches `current_index=45` before x57 / SIG 5 / Java
Inter 1295 (grade bits `0x3fe593d56730c827`), whose LEFT is linked and RIGHT remains
open/unlinked.

The v44 oracle keeps orders 1-43 as mutations without snapshots and emits only
authenticated order-0 baseline/C-link evidence plus the order-44 frontier, result,
and continuation. It is not independent predecessor-snapshot evidence. The focused
Boundary-70 gate is green 1/1; the full 14-test sibling suite, strict workspace
Clippy, formatting, and diff checks also pass. The schema-v44 derivative is 14 lines /
11,456 bytes with nine semantic rows plus summary. Fixture/runner/probe/body/semantic
pins are `1d5c98477377e64e95a659fa04ed8d8331e02d5e87962811b790ff80f0315515`,
`ee0fb2771acf9693f47814e4abe1de1d7e6434a748178fad32ea823d5e3797d7`,
`85bf12d76e49fdd036806441f505ac2e7db446d90fc0c3452c5c2c7a78997676`,
`9b702d865c4e09f400849a7296cf06d0c5750761bc20dc6fdb58a92cd9a3b8aa`, and
`906b8a4f98dafe6e7d937144a251f153a75b37332852108e141b485050dcdf9a`;
the base v43 runner/fixture remain pinned by
`421c8cd9b3a9208b509ce511077c6656faeac230212deff7ab797f6ffec73d75` and
`dc5f7ce12d292a13cc149e7df0249703323df92de9054daf5eff52783b32919d`,
and the shared v27-v44 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-44 existing-stem reconciliation evidence, not coverage of order
45, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

Boundary 71 carries order 45 (x57 / SIG 5 / Java Inter 1295, grade bits
`0x3fe593d56730c827`). LEFT is already linked and its two-relation LEFT/BOTTOM
candidate resolves active glyph 303 to existing Stem 2374, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is `Neither`; incident Stem 2374 joins
x56 and x57 on LEFT, so x56 LEFT then RIGHT close through two ordered writes. Native
records no unlinked head and reaches `current_index=46` before x40 / SIG 27 / Java
Inter 1339 (grade bits `0x3fe3aa2e83097210`), whose LEFT is linked/closed and RIGHT is
unlinked/closed.

The v45 oracle keeps orders 1-44 as mutations without snapshots and emits only
authenticated order-0 baseline/C-link evidence plus the order-45 frontier, result,
and continuation. It is not independent predecessor-snapshot evidence. The focused
Boundary-71 gate is green 1/1; the full 14-test sibling suite, strict workspace
Clippy, formatting, and diff checks also pass. The schema-v45 derivative is 14 lines /
11,415 bytes with nine semantic rows plus summary. Fixture/runner/probe/body/semantic
pins are `f70a5aeee405899ee2e9bf3be6957ffa657c6f0bcd5bc5d84ab0fc0288b19073`,
`ee0b1141ee872ac784c60e43062c7f8ae98e26730ef596d3fb9c110c520de728`,
`a00ee470231fa732748b5106eba841530f02502f78c13396caf5034de66326e6`,
`3d9f7cd89ed218227e46f04d15a6a525680137f55845ca1c77640db16b4cca93`, and
`6fb75ce2c4b10b2feaf8b98fd569eb035d7be817a448fdf4c5cf5239cf8eded8`;
the base v44 runner/fixture remain pinned by
`ee0fb2771acf9693f47814e4abe1de1d7e6434a748178fad32ea823d5e3797d7` and
`1d5c98477377e64e95a659fa04ed8d8331e02d5e87962811b790ff80f0315515`,
and the shared v27-v45 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-45 existing-stem reconciliation evidence, not coverage of order
46, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

Boundary 72 carries order 46 (x40 / SIG 27 / Java Inter 1339, grade bits
`0x3fe3aa2e83097210`). LEFT is already linked and closed; its two-relation LEFT/BOTTOM
candidate resolves active glyph 326 to existing Stem 2350, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is closed. Incident Stem 2350 joins
x39, x40, and x43 on LEFT. Java emits ordered x39 LEFT/RIGHT true-to-true writes, then
x43 LEFT/RIGHT false-to-true writes; `closedValueChanges=2`. Native records no
unlinked head and reaches `current_index=47` before x89 / SIG 22 / Java Inter 1329
(grade bits `0x3fd6ac9dfd130464`), whose LEFT is linked/closed and RIGHT is
unlinked/closed.

The v46 oracle keeps orders 1-45 as mutations without snapshots and emits only
authenticated order-0 baseline/C-link evidence plus the order-46 frontier, result,
and continuation. It is not independent predecessor-snapshot evidence. The focused
Boundary-72 gate is green 1/1; the full 14-test sibling suite, strict workspace
Clippy, formatting, and diff checks also pass. The schema-v46 derivative is 14 lines /
11,504 bytes with nine semantic rows plus summary. Fixture/runner/probe/body/semantic
pins are `017cfeddc3faeedda3aca5308c82251135bd0c3308854385f77271cb7fc76f8d`,
`aaff9b381d5268c42f9688658071bcafd31736c7987c21a79726eb516483fa78`,
`c9a1def226782df3853dcd8a9df987b7889006ac38041d1a2f8998e8a2105a69`,
`89b8031f853144e07b8789ec9a0ba6d49aa9fd0dce12d2c7298e5036a0ecb4f2`, and
`46281d8c0cedf474303443b88b1823e035ee556074595c3a0c4c506cda181db3`;
the base v45 runner/fixture remain pinned by
`ee0b1141ee872ac784c60e43062c7f8ae98e26730ef596d3fb9c110c520de728` and
`f70a5aeee405899ee2e9bf3be6957ffa657c6f0bcd5bc5d84ab0fc0288b19073`,
and the shared v27-v46 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-46 existing-stem reconciliation evidence, not coverage of order
47, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

Boundary 73 carries order 47 (x89 / SIG 22 / Java Inter 1329, grade bits
`0x3fd6ac9dfd130464`). LEFT is already linked and closed; its one-relation LEFT/BOTTOM
candidate resolves active glyph 304 to existing Stem 2359, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is closed. Incident Stem 2359 joins
x89 and x90 on LEFT. Java closes x90 LEFT then RIGHT through two ordered false-to-true
writes, with exact `closedValueChanges=2`. Native records no unlinked head and reaches
`current_index=48` before x52 / SIG 2 / Java Inter 1289 (grade bits
`0x3fd5af02eef9418a`), whose LEFT is linked/closed and RIGHT is unlinked/closed.

The v47 oracle keeps orders 1-46 as mutations without snapshots and emits only
authenticated order-0 baseline/C-link evidence plus the order-47 frontier, result,
and continuation. It is not independent predecessor-snapshot evidence. The focused
Boundary-73 gate is green 1/1; the full 14-test sibling suite, strict workspace
Clippy, formatting, and diff checks also pass. The schema-v47 derivative is 14 lines /
10,882 bytes with nine semantic rows plus summary. Fixture/runner/probe/body/semantic
pins are `5a7989434b78dbd6ea72f113cd9f66078ae8e9c3acabb8980ecdb7577120de39`,
`7a9605cf09f1d78f899423a816c0c6adc2b121786f56c69c271b41da5527f6ab`,
`ecd26aba8d5c02fb695cf68ed9006d7f80e002e8287eba941f1dec3655b85a70`,
`9b32571ee576c45644622c26be3c0966b8bee1260c6b4e7e27aa2e8d04686d73`, and
`497efa9c299608de7fbecda7f48531baee96be1fd9cf79435c3ffc289a8aafc9`;
the base v46 runner/fixture remain pinned by
`aaff9b381d5268c42f9688658071bcafd31736c7987c21a79726eb516483fa78` and
`017cfeddc3faeedda3aca5308c82251135bd0c3308854385f77271cb7fc76f8d`,
and the shared v27-v47 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-47 existing-stem reconciliation evidence, not coverage of order
48, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

Boundary 74 carries order 48 (x52 / SIG 2 / Java Inter 1289, grade bits
`0x3fd5af02eef9418a`). Its linked-and-closed LEFT four-relation candidate resolves
glyph 296 to existing Stem 2344; RIGHT is closed. Java takes `SkipAlreadyLinked` and
`SkipClosed`, closes x53 LEFT then RIGHT, and reports `closedValueChanges=2`. Native
makes no graph mutation and reaches `current_index=49` before x35 / SIG 68 / Inter
1425 (`0x3fd525fff19ec48c`). The snapshot-minimized v48 gate is focused/full/Clippy/fmt/diff
green; it is not independent predecessor evidence. Fixture/runner/probe/body/semantic
pins are `acc3436794b0ea828dbd689adfd072b6844125007131ee4207d9d4402c90cd5d`,
`925536d8d119102e5a74a3690b2286bde856bd476151243806d68a049aa40fdb`,
`af7f62ae73911530d863cbf8e4f2ee8bb3d019cfb556185e5fca334cad8a318d`,
`aa738347bf8581a87c5293e9b549261946b9adfef21c3e07e7d37ebdb21e2907`, and
`1183c4dce1c645a0ee070f1bd12d8796b22d9f0bde91c9421c3bc75db833a80f`;
base v47 runner/fixture are `7a9605cf09f1d78f899423a816c0c6adc2b121786f56c69c271b41da5527f6ab`
and `5a7989434b78dbd6ea72f113cd9f66078ae8e9c3acabb8980ecdb7577120de39`.

Boundary 75 carries order 49 (x35 / SIG 68 / Java Inter 1425, grade bits
`0x3fd525fff19ec48c`). Its linked-and-closed LEFT one-relation HeadStem candidate
resolves glyph 316 to existing Stem 2369; RIGHT is closed. Java takes
`SkipAlreadyLinked` plus `SkipClosed`, closes x36 LEFT then RIGHT, and reports
`closedValueChanges=2`. Native makes no graph mutation and reaches
`current_index=50` before x32 / SIG 50 / Java Inter 1389 (grade bits
`0x3fd520322f6aeb9d`), whose two sides are open/unlinked.

The snapshot-minimized v49 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`ef4bfca2696caba71b227fb15d36b85adfe34ca519e7d87419d86f2605664147`,
`da0ed259bc25e03b3624b96a5078661497fb713f08ca183dd3624bd16e74f406`,
`1848e0a3c06790d58816837b5c60f871be05125037a720e4a228efa42731b5dd`,
`d79d89bb20f662409759dfb08279d7735dd4050b53926e28f286c2e8319120e9`, and
`734556edff9e5a81b65fb66ad4bc23e6c66f1bbf152eadd130d2a9acaf61f6ad`;
base v48 runner/fixture are `925536d8d119102e5a74a3690b2286bde856bd476151243806d68a049aa40fdb`
and `acc3436794b0ea828dbd689adfd072b6844125007131ee4207d9d4402c90cd5d`.
This is bounded order-49 existing-stem evidence, not order 50 C-link behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

Boundary 76 carries order 50 (x32 / SIG 50 / Java Inter 1389, grade bits
`0x3fd520322f6aeb9d`). Its LEFT/BOTTOM frontier has one HeadStem relation and two
glyph rows (314 + 2219), resolving active glyph 314 to existing Stem 2383. LEFT is
`Both` and RIGHT is `TopOnly`; Java returns false with `undefs=[LEFT]`, zero
closure writes, zero unlinked additions, and no graph, registry, or linker mutation.
Native reaches `current_index=51` before x19 / SIG 64 / Java Inter 1417 (grade bits
`0x3fd51434ea56eeb4`). This is the first measured returned-false undef in the carried
queue, not generic no-link/retry coverage.

The snapshot-minimized v50 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`00021c6593a39641e87864cb950a06651a797a76899686c23f9faa009e0d275d`,
`edff8b0e4715bb198bc70f2cdaac9320d8940f66a417ff9c7cad532cc96a5910`,
`2a97dd755fa5b2b48d76dd914b24ddcf66e1db0597d124630d49cb56edefde99`,
`776e842e93ea29c4779edc08efa9ab7772741a203a6524fef59307d1021137fe`, and
`5b134ffb2ec0756647be00a38de6dcb1efabafb9a000872dfeedd8d30f1b7c7c`;
base v49 runner/fixture remain `da0ed259bc25e03b3624b96a5078661497fb713f08ca183dd3624bd16e74f406`
and `ef4bfca2696caba71b227fb15d36b85adfe34ca519e7d87419d86f2605664147`.

Boundary 77 carries order 51 (x19 / SIG 64 / Java Inter 1417, grade bits
`0x3fd51434ea56eeb4`). Its linked-and-closed LEFT one-relation HeadStem candidate
resolves glyph 299 to existing Stem 2361; RIGHT is closed. Java takes
`SkipAlreadyLinked` plus `SkipClosed`, closes x20 LEFT then RIGHT, and reports
`closedValueChanges=2`; the order-50 undefined LEFT side stays carried and
unchanged. Native makes no graph mutation and reaches `current_index=52` before
x15 / SIG 80 / Java Inter 1449 (grade bits `0x3fd4eef3f5487510`).

The snapshot-minimized v51 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`778c06a6697f96a439146e06d85d9d182d1f7659ef19a15fd9204aef3455546e`,
`55f522ebc3bff281f113715e86137e5feac2d74f513907b8361903cdf3b1a828`,
`537f15c3d73d79eea77e08b9a72b89fa0a4b54bf2c6c61e951371ea161e42c2e`,
`ddd236253a96af0b8932e6a873b9d5fe4086c16db1adc86da733ba34ae2dd93a`, and
`1a1662346825b66eb64cf50c7ef8ab0699c1ae776321dc0514214c590eec8c4c`;
base v50 runner/fixture remain `edff8b0e4715bb198bc70f2cdaac9320d8940f66a417ff9c7cad532cc96a5910`
and `00021c6593a39641e87864cb950a06651a797a76899686c23f9faa009e0d275d`.
This is bounded order-51 existing-stem evidence, not order 52 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

Boundary 78 carries order 52 (x15 / SIG 80 / Java Inter 1449, grade bits
`0x3fd4eef3f5487510`). Its linked-and-closed LEFT frontier carries one HeadStem
and one BeamStem relation and resolves glyph 329 to existing Stem 2360; RIGHT is
closed. Java takes `SkipAlreadyLinked` plus `SkipClosed`, closes x16 LEFT then
RIGHT, and reports `closedValueChanges=2`; the order-50 undefined LEFT side
stays carried and unchanged. Native makes no graph mutation and reaches
`current_index=53` before x84 / SIG 86 / Java Inter 1461 (grade bits
`0x3fd4c6c06694da1c`).

The snapshot-minimized v52 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`ff3bbe3fdf9ba0e6140b8105ab46e4c65972d4f4f013d39f90eade2270b64224`,
`c314a1da865f91ce57128468a77dff85b7dd20719427c9119ca29057331728a6`,
`053a7ed7993314e846561a404bf093e836211373b51a552ac50c62ca40b0c355`,
`bc0b579b830bc4c1ccf057f85efa02b6226f25ade19d62aca28344ff9f0c35da`, and
`19d22e7ed4d7319494f3b734e9aed996cb37b9d4b4a27abaa93ccbec3f073e0e`;
base v51 runner/fixture remain `55f522ebc3bff281f113715e86137e5feac2d74f513907b8361903cdf3b1a828`
and `778c06a6697f96a439146e06d85d9d182d1f7659ef19a15fd9204aef3455546e`.
This is bounded order-52 existing-stem evidence, not order 53 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

Boundary 79 carries order 53 (x84 / SIG 86 / Java Inter 1461, grade bits
`0x3fd4c6c06694da1c`). Its linked-and-closed LEFT frontier carries two relations
and resolves glyph 320 (candidateIdBefore 320) to existing Stem 2366, which is
shared by three heads (x84, x85, x86). Java takes `SkipAlreadyLinked` plus
`SkipClosed`, closes x85 LEFT then RIGHT, re-writes x86's already-closed cells
without a value change, and reports `closedValueChanges=2` over four closure
writes; the order-50 undefined LEFT side stays carried and unchanged. Native
makes no graph mutation and reaches `current_index=54` before x11 / SIG 62 /
Java Inter 1413 (grade bits `0x3fd474edcf4c89da`).

The snapshot-minimized v53 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`58671e0a19695e626a633a8963683d310083e3bb18ee9d419a69d1db0267be76`,
`cdf67c2e57e5afcbb7e7030d0cf80a2ce50302778032154b5ced066f234a2611`,
`c153c5cc6c2dbc02486c179ad04ddeea04cb331eefa345bf1059476ee7d0ba43`,
`575945ccaeb5f2d2288fd8d0cbff7978849bc8b017d39a8876bcef102c09a1a5`, and
`12ce9cd028a62c77be4f68fab944a998e495b161a24e0f4ac43e6522b23aeb62`;
base v52 runner/fixture remain `c314a1da865f91ce57128468a77dff85b7dd20719427c9119ca29057331728a6`
and `ff3bbe3fdf9ba0e6140b8105ab46e4c65972d4f4f013d39f90eade2270b64224`.
This is bounded order-53 existing-stem evidence, not order 54 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

Boundary 80 carries order 54 (x11 / SIG 62 / Java Inter 1413, grade bits
`0x3fd474edcf4c89da`). Its linked-and-closed LEFT frontier carries four
relations and resolves glyph 312 to existing Stem 2349; RIGHT is closed. Java
takes `SkipAlreadyLinked` plus `SkipClosed`, closes x12 LEFT then RIGHT, and
reports `closedValueChanges=2`; the order-50 undefined LEFT side stays carried
and unchanged. Native makes no graph mutation and reaches `current_index=55`
before x68 / SIG 75 / Java Inter 1439 (grade bits `0x3fd454aaa59250ca`).

The snapshot-minimized v54 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`c8b6567ddac7269d126846e036fc7e4fbb8a9c430a7b110006ff8e980ad85305`,
`1fec8bbf7c5561150d9d69079275fccc1879f5d61756f8c525957c0ef90b16ca`,
`e3eb0126cd446a3c67e808c08004775e32aa7e5bd4e78d5d262ad9fafff89abd`,
`4c8613b927af254cbaa6afdcf4e99dc99a2f149381985fd74b8f15f2b20d0286`, and
`e77cc8596f1540ab17b7894abcbf017cd2e3c79cffc4ae974011910e2081ec96`;
base v53 runner/fixture remain `cdf67c2e57e5afcbb7e7030d0cf80a2ce50302778032154b5ced066f234a2611`
and `58671e0a19695e626a633a8963683d310083e3bb18ee9d419a69d1db0267be76`.
This is bounded order-54 existing-stem evidence, not order 55 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

Boundary 81 carries order 55 (x68 / SIG 75 / Java Inter 1439, grade bits
`0x3fd454aaa59250ca`). Its linked-and-closed LEFT one-relation HeadStem candidate
resolves glyph 331 to existing Stem 2347; RIGHT is closed. Java takes
`SkipAlreadyLinked` plus `SkipClosed`, closes x69 LEFT then RIGHT, and reports
`closedValueChanges=2`; the order-50 undefined LEFT side stays carried and
unchanged. Native makes no graph mutation and reaches `current_index=56` before
x21 / SIG 11 / Java Inter 1307 (grade bits `0x3fd438cb1438cb15`).

The snapshot-minimized v55 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`8ed542577f68000705cd7d166c8d848e48c138ae9f6e8fad0de10a499e46c0ff`,
`8dfffd31e6db65348598433cc0e19683d80be55705e9d843bd88e564deb5ca67`,
`6557794a5daf7335e05a6de21f7a9479aa2a601d151163086a224d3b64cfea9d`,
`fceb2bca866912731d6ab81165fd03254f2c602d3937af196ddfff002d8beb60`, and
`354f56967e0b75c4948321dc0153e041851c47c3cbe1507843abca8de249992a`;
base v54 runner/fixture remain `1fec8bbf7c5561150d9d69079275fccc1879f5d61756f8c525957c0ef90b16ca`
and `c8b6567ddac7269d126846e036fc7e4fbb8a9c430a7b110006ff8e980ad85305`.
This is bounded order-55 existing-stem evidence, not order 56 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

Boundary 82 carries order 56 (x21 / SIG 11 / Java Inter 1307, grade bits
`0x3fd438cb1438cb15`). Its linked-and-closed LEFT frontier carries four
relations and resolves glyph 323 (candidateIdBefore 323) to existing Stem 2341;
RIGHT is closed. Java takes `SkipAlreadyLinked` plus `SkipClosed`, closes x22
LEFT then RIGHT, and reports `closedValueChanges=2`; the order-50 undefined
LEFT side stays carried and unchanged. Native makes no graph mutation and
reaches `current_index=57` before x62 / SIG 16 / Java Inter 1317 (grade bits
`0x3fd4131337c4d540`), whose two sides are both open/unlinked.

The snapshot-minimized v56 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`e463b46a707b8c534a1896bf47d7668c580f38c27b2947ccdfb12fb7984e2cc5`,
`b6a0b6cd9a618052e02da16022a44fe3b218626100852bb6f702ceedc09f3387`,
`24e6dd4f7975c419cc473be22f351f1d4687c76b2fcb0100eeb52080b0fb924f`,
`6ca99c74758506f6173cc8e9b21d323002c98ebcaa1bb34732091fcd10b4a43a`, and
`3ed440230114439aa352bfe883e6befb404e26b26235b98e060860b6841422e8`;
base v55 runner/fixture remain `8dfffd31e6db65348598433cc0e19683d80be55705e9d843bd88e564deb5ca67`
and `8ed542577f68000705cd7d166c8d848e48c138ae9f6e8fad0de10a499e46c0ff`.
This is bounded order-56 existing-stem evidence, not order 57's both-open
C-link behavior, no-link/retry, phase 2, broader geometry, or wider-corpus
coverage.

Boundary 83 carries order 57 (x62 / SIG 16 / Java Inter 1317, grade bits
`0x3fd4131337c4d540`). Both sides start open: LEFT evaluates BottomOnly and
RIGHT Neither, so the LEFT/BOTTOM C-link expands one seed-plus-chunk builder
whose candidate resolves to active glyph 328, already materialized as Stem
2381. Java's `createStem` reuses that stem instead of allocating: exactly one
HeadStem relation is appended (SIG edges 696 to 697), x62's LEFT cells link,
and sibling x63's cells close inside the C-link transaction with phase-level
`closedValueChanges=0`. No vertex, allocator, ID, registry, or system-stem
mutation occurs, and the order-50 undefined LEFT side stays carried. Native
reaches `current_index=58` before x92 / SIG 24 / Java Inter 1333 (grade bits
`0x3fd3e2be2be2be2c`).

The snapshot-minimized v57 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`c8b44bab7c0e75e74755e3ca2d29f46b72b90cde0e406524fb395214a6fe25d5`,
`a108389f5e9465fb4483ecec852e38dc7985676a1d9f1feb8dcb392b32559fbc`,
`ef9f4a596d53db3ab7007d382f16319f44985be90c60f788aa854fb8f7379d5d`,
`b9e3309cdeea3f2bc59abe296a3d33738e09ab79021fb710259d5b608e8557ac`, and
`790274c39fb5e7b4637d2f7d26a62559c89264c265a19710f36a8351d9f454fe`;
base v56 runner/fixture remain `b6a0b6cd9a618052e02da16022a44fe3b218626100852bb6f702ceedc09f3387`
and `e463b46a707b8c534a1896bf47d7668c580f38c27b2947ccdfb12fb7984e2cc5`.
This is bounded order-57 existing-stem C-link evidence, not order 58 behavior,
generic reuse geometry, no-link/retry, phase 2, or wider-corpus coverage.

Boundary 84 carries order 58 (x92 / SIG 24 / Java Inter 1333, grade bits
`0x3fd3e2be2be2be2c`). Its linked-and-closed LEFT frontier carries three
relations and resolves glyph 298 to existing Stem 2342; RIGHT is closed. Java
takes `SkipAlreadyLinked` plus `SkipClosed`, closes x93 LEFT then RIGHT, and
reports `closedValueChanges=2`; the order-50 undefined LEFT side stays carried
and unchanged. Native makes no graph mutation and reaches `current_index=59`
before x100 / SIG 42 / Java Inter 1369 (grade bits `0x3fd3a0aec9cc7ff8`).

The snapshot-minimized v58 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`a262c7a657a028a7c2e273283176749bc364717837735a391540cb2783a2ed06`,
`9964348b54b3500efda3f1e98b1fcf4e54d9e518de4d416b170a2b1fbe8ea757`,
`23f2d7a80c31898306ce8adcf61be15280aa0457e42988c424a8b2ceee9886d9`,
`6c0e5202b0e2a891c53a4635ec2729d636e0e7758141174c96462e834602d83b`, and
`12d06cc6c5d25d3acf2189827d8bb35fa68d1e6350b7a73777f23d407345a810`;
base v57 runner/fixture remain `a108389f5e9465fb4483ecec852e38dc7985676a1d9f1feb8dcb392b32559fbc`
and `c8b44bab7c0e75e74755e3ca2d29f46b72b90cde0e406524fb395214a6fe25d5`.
This is bounded order-58 existing-stem evidence, not order 59 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.



Boundary 42 reuses the continuation for head order 16 (x8 / SIG 53 / Java Inter 1395,
grade bits `0x3fe81161126880f9`). Shared Stem 2376 closes x7 LEFT then RIGHT, two
ordered writes with no unlinked head; native keeps SIG 680/691 and Stem bindings 41 and
reaches `current_index=17` before x48 / SIG 29 / Java Inter 1343. The separate
schema-v16 derivative is 12 lines / 8,189 bytes with seven semantic rows plus summary,
SHA-256 `04d35bb21c808dc38edd93c0631b3a01af9931efc8f500422646adf8f7123de4`;
runner, transformed probe, emitted body, and semantic pass are
`d6edd52b746acd625c2e516f328c4b43253e23bbbe906ffcdae0b3674eae1dcf`,
`d4dcad17952d2de86de193bd87c3a96916ad7781d67f1ea469180e05e4e106fd`,
`49b97d61e08769b58a449edf2931313f91a5855000fd89e27761330f30a81077`, and
`88ea097c4a003e7493c5d28296cc6dd778486660bb6a1e3eb1bfb5aa71f40f7d`.
This derivative deliberately executes orders 1-15 only to reconstruct the predecessor
without emitting or persisting their full snapshots; only order 16 is emitted, keeping
the replay below the full-snapshot heap limit. Two fresh runs are byte-identical and
the base v15 fixture/runner remain pinned. This is bounded order-16 evidence, not
independent snapshot evidence for every predecessor or completion of the remaining
queue, actually-unlinked/retry behavior, phase-2 append, or broader C-link branches.

Boundary 41 reuses the continuation for head order 15 (x67 / SIG 59 / Java Inter 1407,
grade bits `0x3fe814269b1247c7`). Shared Stem 2375 closes x66 LEFT then RIGHT, two
ordered writes with no unlinked head; native keeps SIG 680/691 and Stem bindings 41 and
reaches `current_index=16` before x8 / SIG 53 / Java Inter 1395. The separate
schema-v15 derivative is 12 lines / 8,191 bytes with seven semantic rows plus summary,
SHA-256 `aae5116a32e0fd77bb9f4a26dc1a8c1cd53a3f3ff35ea01d350c97012a146ca8`;
runner, transformed probe, emitted body, and semantic pass are
`e595eefa74453ecfe9980cb294b80d37d0ff5ad1e2f3e01d88f8801d0f23ca18`,
`98ac227864e84c3693d5368a85adf970512648a9a99c74a2b612a01d4b45d065`,
`1e198195daf91b8d56ebcc2a88a5e97fc2752603f365d0d5cea3145f9a1f1ef2`, and
`55323828f0e4c8e08d85373684f71b7ec9a6f2e75a49278006dae1b8ec673cd9`.
This derivative deliberately executes orders 1-14 only to reconstruct the predecessor
without emitting or persisting their full snapshots; only order 15 is emitted, keeping
the replay below the full-snapshot heap limit. Two fresh runs are byte-identical and
the base v14 fixture/runner remain pinned. This is bounded order-15 evidence, not
independent snapshot evidence for every predecessor or completion of the remaining
queue, actually-unlinked/retry behavior, phase-2 append, or broader C-link branches.

Boundary 40 reuses the continuation for head order 14 (x12 / SIG 63 / Java Inter 1415,
grade bits `0x3fe8187dd5fbfd0c`). Shared Stem 2349 closes x11 LEFT then RIGHT, two
ordered writes with no unlinked head; native keeps SIG 680/691 and Stem bindings 41 and
reaches `current_index=15` before x67 / SIG 59 / Java Inter 1407. The separate
schema-v14 derivative is 12 lines / 8,192 bytes with seven semantic rows plus summary,
SHA-256 `f60e5dff377e5e51038ec061b1ebeec5a5868f4cd51af6b9618377bfa3a12e6a`;
runner, transformed probe, emitted body, and semantic pass are
`6b5e339f8b91db08d4e03edf7ed3b69ea8ab713b98ce95c62a95440a0652ccb9`,
`eea0869093b1c1a262da5da0d7ad914f3dc7b6a8d771a32bc60849687291c834`,
`9ebf233711be059ddee5adf964b6bbbbe44770caef19f5903c8ce9a5a16d1889`, and
`14d0e0c71dff0f40e5745858ad10d615c56463291cf6caa863edd2ebccde0590`.
This derivative deliberately executes orders 1-13 only to reconstruct the predecessor
without emitting or persisting their full snapshots; only order 14 is emitted, keeping
the replay below the full-snapshot heap limit. Two fresh runs are byte-identical and
the base v13 fixture/runner remain pinned. This is bounded order-14 evidence, not
independent snapshot evidence for every predecessor or completion of the remaining
queue, actually-unlinked/retry behavior, phase-2 append, or broader C-link branches.

Boundary 39 reuses the continuation for head order 13 (x53 / SIG 3 / Java Inter 1291,
grade bits `0x3fe83971fb8b04c3`). Shared Stem 2344 closes x52 LEFT then RIGHT, two
ordered writes with no unlinked head; native keeps SIG 680/691 and Stem bindings 41 and
reaches `current_index=14` before x12 / SIG 63 / Java Inter 1415. The separate
schema-v13 derivative is 12 lines / 8,188 bytes with seven semantic rows plus summary,
SHA-256 `ff27fa03e80e44e554d46682c827097ecec1d463abf0c0e131a6ab1beccfbb5e`;
runner, transformed probe, emitted body, and semantic pass are
`675bce84bfa4e76ed78cc72592da9f8fe95571752d424da99bd4be93af7478f8`,
`915bc4a3563943b93fa806a614b835da8e7799732cf8c1c1c7aa9127fc39a61e`,
`84254e3f9dc1e4297b4efaabb30c36d07244ffe3d268cce5097ec14d365ab974`, and
`f2b4a2e49aee6fd27d41470eb38a1bfe541d72688b03bb33d5b3ed3266514519`.
This derivative deliberately executes orders 1-12 only to reconstruct the predecessor
without emitting or persisting their full snapshots; only order 13 is emitted, keeping
the replay below the full-snapshot heap limit. Two fresh runs are byte-identical and
the base v12 fixture/runner remain pinned. This is bounded order-13 evidence, not
independent snapshot evidence for every predecessor or completion of the remaining
queue, actually-unlinked/retry behavior, phase-2 append, or broader C-link branches.
