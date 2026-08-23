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
| Native SIG through HEADS and into STEMS | `assemble_native_sig` owns the insertion-ordered per-system GRID-through-HEADS graph; chula system 1 reproduces Java's 221 vertex / 202 edge structural hashes exactly. `advance_native_stems_beam_sides_transaction` atomically advances one frontier: scheduler, latest B14/transaction state, SIG/bindings, and persistent B/S cells swap only after B19 succeeds. Repeated calls execute all 32 native SIDES transactions and reach explicit `SidesExhausted` at 253 vertices / 331 edges, 32 Stem bindings, 61 linked/open B cells, and 68 linked/open S cells. Exact plan/B-linker order and all 29 sibling-write lists match the frozen Java pass only after the native terminal is returned; its 21 `AlreadyLinked` skips therefore come entirely from earlier native B16 writes. The carrier then enters chula system 1's 34-beam STUMPS worklist and preserves structural-side-before-linked precedence. Boundaries 22-24 call the same production carrier for plans 147, 622, and 404, reaching 256/340, 35 Stem bindings, B64/S74 before resuming at worklist index 3 to beam SIG 28 / stump 1 / plan 508. Boundary 24 adds the first natural two-glyph compound candidate in this carried STUMPS prefix without changing production code. Boundary 25 adds a bounded atomic batch driver and carries plans 508, 28, 330, and 251 to the typed post-STUMPS terminal at 260/353, 39 Stem bindings, B68/S83. Boundaries 136-138 derive `NativeStemsModeledGlyphRegistry` from the 1,058 system-1-visible modeled objects and drive transactions 1-32 plus all later STUMPS/HEADS glyph joins from native canonical-ordinal identity and exact content, without the 1,650-entry snapshot, its 592 opaque fingerprints, or Java glyph IDs; Boundary 138 derives transaction 1's B12 line/binding/system-stem state from live products and its B13 reads from the owned SIG/S cells. Boundary 139 resolves every selected B14 beam directly from native SIG bindings, with one-based native vertex identity, native vertex-ordinal InterIndex order, and false VIP, so the sparse 16-row Java selected-base bridge is gone. Boundary 140 derives the complete initial B14 compact graph/index state from the owned SIG; native insertion order replaces Java's opaque 639-entry InterIndex baseline. Boundary 141 seeds the shared native identity domain after the 1,058 modeled glyphs and allocates StemInter IDs 1,059-1,104 without Java's 2,339 EntityIndex watermark; continuation guards use carried stem identity. The graph-derived B13 path is now gated on one real later linked-S reconstruction: Allegretto system 1 transaction 28 traverses HeadStem edge 229, selects the modeled attached StemInter with Java ID 2227, and leaves the second entry unread; the gate explicitly reconstructs rather than natively carries transactions 1-27. Boundary 26 also removes and resumes past one real competing hook from an explicitly reconstructed Allegretto post-transaction-28 checkpoint. Boundary 27 validates all 102 live heads and persistent S cells, preserves Java's stable reverse-grade order, and transfers the exact post-STUMPS carrier into a typed first head-origin C-link frontier without mutation. Boundary 28 atomically applies that frontier's bounded one-item, nonrecursive `CreatedChecked` mutation and stops before head index 1. Boundaries 29-32 carry head orders 1-5 through prelinked success and twelve ordered shared-stem closure writes to `current_index=6`, still with no unlinked head. The path does not yet own the remaining head queue, an actually-unlinked retry, or broader C-linker shapes, general sheet/book dirty state, native predecessor carriage and wider linked-S or hook-removal coverage, wider-corpus STUMPS authority and branch coverage, or every corpus BEAMS group |
| Continuous integration | `.github/workflows/rust-port.yml` runs fmt, Clippy with `-D warnings`, and `cargo test --workspace` on `ubuntu-latest` and `macos-latest` -- two architectures as well as two systems, which is the axis that caught the host-dependent libjpeg reference below. `rust-toolchain.toml` pins the channel because `-D warnings` makes an unpinned Clippy a source of failures with no code change. The PDF corpus test skips in CI (its 20 MB of IMSLP scans are not fetched) and the last step re-runs it with `--nocapture` so a skip cannot read as a pass; nothing Java-backed runs there |
| Live Java/Rust vectors | 73 canonical fixtures, including composed StaffProjector, recursive clusters, bar-column construction/start selection, production `LagManager.dispatchRuns`, `Book.updateScores` regrouping, live `SystemInfo.buildRef` ownership, a composed GRID output boundary, production SIG contextual grading, exact sheet-skew transforms, raw-raster `retrieveLines`, raster-fitted mutable endpoints, raw alignment/connection discovery, exact `StaffFilament.fillHoles` mutation, all 149 raw grades for a fixed classifier feature vector, an asymmetric point-list MixGlyphDescriptor feature vector, and Java-order RunTable coordinate/feature extraction with absolute offset |
| Oracle asset manifest | classifier, 6 fonts, and 8 image fixtures SHA-256-frozen |
| Differential testkit | deterministic sorted vectors and first-difference diagnostics used by `xtask`; bounded fixture roots |
| Structured output and live comparison | Ordinary `-json` emits the unchanged schema-1 document per requested sheet through STEMS. The opt-in `-stream-json` viewer protocol adds flushed `@omrscope` schema-1 boundary markers around those unchanged documents, yielding immutable **completed-stage** snapshots from GRID through STEMS; it does not expose item-by-item or intra-stage recognition. `omrscope` runs Rust and Java independently and concurrently, retains/selects each completed snapshot, and keeps the ordinary JSONL and Java oracle outputs compatible. Its Page/Inters inspection surface now highlights the inspected pair without native table selection, can opt into highlighting all filtered rows, and can draw engine-local relation edges only when both endpoint IDs resolve uniquely in that selected engine snapshot; it never infers cross-engine graph edges. GRID's byte path remains unchanged; later documents add selected clef/key/time inters with lifecycle/classifier evidence, accepted stem seeds, system-owned header erases, beam/ledger geometry, identity-free final heads, and terminal native Stem/HeadStem products. STEMS retains every upstream product and publishes 148 Batuque final Stem geometries/grades, 323 HeadStem payloads, checked/abnormal/no-stem summaries, and undefined sides using explicitly native, system-local identities rather than fabricated Java IDs. Text after GRID remains explicitly unsupported. `omrscope` parses bounds-only headers, both median forms, and accepted top-level stem seeds; it rejects incomplete geometry and treats the new stage-owned fields additively. A separate manual Score tab still runs one selected Java sheet through PAGE, validates its single local MusicXML/MXL artifact, and renders it to local Verovio SVG; that preview is inspection only and does not imply native PAGE/MusicXML parity. The workspace carries no serialization dependency |
| Rust workspace | The workspace now contains two hundred and thirty-eight exact STEMS boundaries. Boundaries 1-134 retain the detailed scheduler, mutation, head-linking, phase-2, and generic `finalizeStems` evidence. Boundaries 135-163 complete and publish Batuque; Boundaries 164-166 complete Chula; Boundaries 167-183 complete Allegretto; Boundaries 184-185 complete Zizi; Boundaries 186-191 complete Carmen; Boundaries 192-205 complete Cucaracha; Boundary 206 completes Hove; Boundary 207 carries Java's pre-MultipleRest beam-group identities into the live post-rest native SIG; Boundary 208 ports Java's generic phase-1 rather-good retry through profiles 0-3; Boundary 209 reuses an existing concrete stump across two already-linked beam items without duplicating their BeamStem edges; Boundaries 210-212 authenticate three identity-free prelinked reconciliations with zero, two, then zero value changes; Boundary 213 authenticates the following identity-free no-link closure; Boundary 214 reuses another concrete stem across two already-linked beam items; Boundary 215 adds a second head to an existing stem with exact Java line rounding; Boundary 216 reconciles a two-head existing stem; Boundary 217 authenticates a four-head zero-change reconciliation; Boundary 218 reconciles another two-head existing stem; Boundary 219 authenticates a right-side three-head zero-change reconciliation; Boundary 220 authenticates a four-head zero-change reconciliation; Boundary 221 authenticates the following three-head zero-change reconciliation; Boundary 222 authenticates a rejected active-glyph LEFT/TOP C-link and its fail-closed no-mutation continuation; Boundary 223 reuses an existing stem across two already-linked beams plus a trailing glyph while matching Java's final relation recheck; Boundary 224 rejects the next LEFT/TOP C-link and records the RIGHT shared-stump side as undefined without mutation; Boundaries 225-226 authenticate two idempotent three-head prelinked reconciliations; Boundary 227 authenticates a right-side four-head reconciliation with two real and four idempotent closure writes; Boundary 228 adds one exact LEFT/TOP HeadStem edge to an existing three-head stem; Boundary 229 authenticates the following idempotent right-side three-head reconciliation; Boundary 230 closes a two-head existing stem with two value changes; Boundary 231 reconciles the following mixed-side four-head stem with two value changes; Boundary 232 adds the next exact RIGHT/BOTTOM edge to an existing five-head stem; Boundary 233 records the following zero-mutation RIGHT dual-corner undefined return; Boundary 234 reconciles the following right-side four-head stem with two value changes; Boundary 235 closes the following right-side two-head stem with two value changes; Boundary 236 closes the following identity-free no-link head and queues it for phase two; Boundary 237 authenticates the following mixed-side four-head zero-change reconciliation; Boundary 238 authenticates the following left-side four-head zero-change reconciliation. Bach now completes system 1 and reaches system 2 queue 212 x116/SIG202. Wider HEADS and corpus branches remain explicit. CLI, report, focused Batuque/Chula/Allegretto/Zizi/Carmen/Cucaracha/Hove/Bach, full sibling gates, strict workspace Clippy, formatting, and diff checks are green. `425d58e82` is the exact fully green remote CI baseline: Build & Test run 32551514978 and all 12 Rust port shards in run 32551514933 succeeded. CI repeats formatting, strict Clippy, and workspace tests on Ubuntu and macOS |
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
| HEADERS recognition stage | `recognize_native_headers` is the production GRID-only composition of `HeaderBuilder` plus native clef, key, and time sourcing/lifecycles in Java system order. It derives system/staff/part and bar-group state, header starts, specific interlines, and ordered good-connected browse bars from GRID; applies exact ranges, proposal ordering, pitch maps, grade/context selection, exclusions, stop propagation, cleanup, and ownership; and returns typed candidates, selected IDs, final headers, system time values, and beam `HeaderErase`s. Clef target pitch now samples the native first/last staff-line splines at each glyph centroid x, with fixed midpoint ordinates retained only as an unavailable-geometry fallback, and production derives Bravura's F-clef area pitch offset rather than assuming zero. The Graceful Ghost system-1 Java/Rust comparison now agrees on `Bass`; all 20 warped and 25 dewarped system crops contain zero `Baritone` clefs. One low-resolution full-page page-5 staff remains a wider GRID/preprocessing outlier while its system crop is `Bass`, and page 3 fails earlier in GRID brace processing. The unchanged nine-page gate still matches all 65 staves, 34 selected keys, 17 selected times, and 30 erases. Missing geometry and nested classifier/run-table/column failures remain typed errors rather than zero defaults |
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
| Later recognition stages | the remaining dependency-light lifecycles are native for `STEMS`, `REDUCTION`, `CUE_BEAMS`, `TEXTS`, `MEASURES`, `CHORDS`, `CURVES`, `SYMBOLS`, `LINKS`, `RHYTHMS`, and `PAGE`; STEMS has two hundred and thirty-eight exact semantic components. Chula completes all three systems through phase 1, phase 2, generic `finalizeStems`, and transactional `recognize_native_stems`. Allegretto completes all three SIDES/STUMPS systems, system 1 through queue 79, system 2 through queue 111, and system 3 through both HEADS phases and generic `finalizeStems`. Batuque, Zizi, Carmen, Cucaracha, and Hove complete transactional recognition; Batuque and Zizi also publish schema-1 STEMS. Bach completes system 1's generic higher-profile no-link retry and now carries system 2 through queue211's left-side four-head zero-change reconciliation. Next is system 2 queue212 x116/SIG202, whose branch remains to be measured |
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

## Boundary 164: Chula system-1 wider reuse composition

Production page composition now consumes the authenticated multi-head
existing-stem reuse transactions at Chula system-1 head orders 67, 70, and 73.
The adapters take the page carrier's accepted free-seed glyph slice rather than
the full test/oracle recognition product; canonical content, carried StemInter,
SIG relations, S-linker writes, and closure order remain authenticated by the
existing exact gates. Order 72 demonstrates that ordinary single-head reuse
continues through the generic transaction without special routing.

The real page advances past the former system-1 failures and rejects atomically
at system 2 queue 54 (x46/SIG94), whose start/head/chunk expansion has not yet
been measured. A focused live-page regression pins that stopping point and the
enhanced fail-closed diagnostic. No frozen oracle changed. The next boundary is
the Java-measured system-2 transaction followed by a generic multi-head reuse
dispatch that removes the remaining order-specific composition decision.

## Boundary 165: Chula system-2 wider reuse

The Java system-2 queue-54 replay is deterministic across two fresh passes.
x46/SIG94 selects LEFT/BottomOnly; start + crossed x45 + chunk resolves to
existing glyph 376 / StemInter 2285, adds two HeadStem edges, closes x45 and
x47, and advances without allocator, vertex, registry, or system-stem changes.
The bounded fixture SHA-256 is
`421c6b99552071e39e6b72a3963f5ac46daf41b3bd0c9a560ea45251868f5c09`;
the byte-identical full-pass SHA-256 is
`6e42c2cd20ceffca1d90359d3bc81d7e60780f3cbe29b22b56d1c8e7a9b8b353`.

Rust resolves the same content to native Stem 45/glyph 127 and applies the two
relations plus four sibling-cell closures. The live page now completes systems
1 and 2 before failing closed at system 3 queue 109 x41/SIG122, a stump-less
start-plus-chunk shape. The next boundary measures that branch and continues
the extraction of a generic wider C-link transaction.

## Boundary 166: generic stump-less rejection and Chula completion

A snapshot-minimized Java replay runs the real Chula system-3 SIDES/STUMPS
predecessor and head orders 0-108 without snapshots, then observes queue 109
x41/SIG122. LEFT/TOP passes `canLink` but its chunk-expanded final HeadStem
relation rejects (`lastIndex=-1`); Java therefore continues the same
`linkSides` call. RIGHT/BOTTOM reaches `lastIndex=maxIndex=0`, resolves active
glyph 425 to existing StemInter 2296, adds only the x41 HeadStem edge, links the
RIGHT S cell, and advances to x18/SIG0 without allocation, vertex, glyph-index,
or system-stem changes. Three independent warmup-plus-two replay variants are
byte-identical with full-pass SHA-256 values
`d07bfcc6915fae64fb8481be8f6b3aaccc6e768a349e9af8b3ea0c46d90ae142`,
`6cf71daa00322c0e6d20cd745d7d0cf68b2bc7b196a8ab1c3c507bf361ad5c4b`, and
`6260e8b63601ac71e00a253ce2c803f8373a293e183e78983209742c2dd96788`.
The four-row bounded fixture is 11 lines / 6,190 bytes, SHA-256
`930a9f936f4c5f1eb535e3256e815f44a08f9b96b5aef1fcc52c0c9b28300a15`.

The generic native C-link loop now materializes a sole chunk when the starting
head has no stump, evaluates its final relation, treats ordinary rejection as
Java's non-mutating `link() == false`, and continues to the opposite side. It
then reuses the carried stem through the existing content-addressed transaction.
This is not an x41-specific dispatch. The live Chula regression now completes
all three systems' phase-1 queues, all carried phase-2 retries, generic
`finalizeStems`, and fail-closed transactional `recognize_native_stems`.
Batuque's production gate and all 16 sibling tests remain unchanged. Next is
the next wider-corpus fail-closed branch rather than another Chula boundary.

## Boundary 167: production Allegretto hook removal

The generic SIDES page driver now consumes its typed
`AwaitingHookRemovalTransaction` frontier by calling the existing atomic native
removal/resume operation instead of rejecting the whole page. Each removal is
retained in both the system SIDES and SIDES+STUMPS results. The operation still
fails closed on malformed full-beam/hook identity, missing exclusion or group
topology, incoherent bindings, or a non-hook scheduler state.

Live `allegretto.png` executes 28 system-1 SIDES transactions, removes the
Java-pinned BeamHook SIG24 competing with Beam SIG25 (five incident edges;
three-member group to two), and reaches true SIDES exhaustion. The serial page
carrier then completes all three systems through STUMPS with hook-removal counts
`[1,0,2]`; every removal deletes exactly one live vertex, its incident edges,
and one group member. System 1 remains externally graded by the unchanged
fixture SHA-256
`d4c5decf03eaab893c79b2cb7ebd0378f13ac019acc007a38718105c75eacc71`;
the two system-3 applications extend structural coverage without claiming new
Java-frozen result rows. Focused Allegretto passes 1/1; the full sibling suite
passes 17/17 in 147.41s; strict workspace Clippy, formatting, and diff checks
pass.

The full production CLI now reaches the next honest gap at Allegretto system 1
HEADS queue 65, x77/SIG14, LEFT/TOP: a start stump plus chunk plus crossed x75
head, with one carried undefined side. That multi-item expansion is the next
wider-corpus boundary.

## Boundary 168: Allegretto multi-item existing-stem C-link

The bounded Java runner replays the real Allegretto system-1 predecessor,
including competing-hook removal and complete STUMPS, mutates heads 0-64
without snapshots, and emits queue 65 twice byte-identically. x77/SIG14
selects LEFT/TOP and walks seed glyph 282, chunk 2034, and crossed x75
LEFT/TOP. The composite reuses StemInter 2236, appends two HeadStem relations,
links x77 and x75 LEFT, and closes x75 plus already-linked x76. There is no
allocator, glyph-registry, vertex, or system-stem insertion. Exact x77/x75
grade/dx bits are `3fe9cd7b1bef63de`/`3fb1a913e59fdb6e` and
`3fe92d2153d3bb34`/`3fb356694b791249`.

The native generic expansion helper now carries undefined sides separately
from the phase-2 unlinked-head queue: Allegretto has undefined `[x84 LEFT]`
but unlinked heads `[x86,x84]`. It reuses native Stem 35/glyph 71, appends the
same two relations, and records four x75/x76 closure value changes without
allocating a stem. Fixture/runner/probe/body/semantic SHA-256 values are
`0bccd92c0a4305704c5903984ccf9734823bf4879b5aa6f2621595700fa6507d`,
`be1f28c0528721e23ba24e1b8107f5069310d47a1a537945052d2a536a260e74`,
`6ae5fe6eddaf4d802973c191c8d945eac8046a1d398499de79c5eb183a489092`,
`0ea1b9deaa33a644ba432a26bfe6a84391cdee5115bacaa070b71287bb1a3a13`, and
`d8a600e1dff9c81fa9ebc4eadd5fc9119548343070cdcf0225ec1dbc798b3b37`.
Focused 1/1 and full sibling 18/18 (143.22s) pass. Production reaches the next
fail-closed frontier at queue 79 x82/SIG89 LEFT/TOP: start stump, crossed x80
RIGHT/TOP head stump, then chunk.

## Boundary 169: Allegretto crossed-side created-stem C-link

The strict order-79 runner mutates the real predecessor through queue 78 and
then emits two byte-identical measurements. x82/SIG89 selects LEFT/TOP, keeps
the initial `canLink` HeadStem relation, accepts crossed x80 RIGHT/TOP against
the evolved line, and stops normally at the rejected trailing chunk. Active
glyph 297 has no StemInter, so Java creates checked StemInter 2240, appends two
HeadStem relations, changes SIG 637/562 to 638/564 and system stems 39 to 40,
links x82 LEFT plus x80 RIGHT, and closes x80 LEFT/RIGHT. The x82/x80 grade/dx
bits are `3feffffffffffe18`/`bd28618618618618` and
`3fe872c0dd16cd02`/`3fb542c107f91e7a`.

Native extends the generic multi-head helper with an authenticated created-
checked disposition, mixed-horizontal carried undefined sides, raw builder
ordering, a bounded trailing-chunk rejection stop, and pre-expansion start-
relation reuse. It creates native Stem 39 / native persistent Inter 1022
(independent of Java Inter 2240) with exact
grade, bounds, median, and thickness, appends the same two relations, closes
x80 twice, and advances to queue 80 atomically. Fixture/runner/probe/body/
semantic SHA-256 values are
`63327c13e4ebba1873fb73d5507b5a34369027ca8c6a4abb60f377cebeee69ee`,
`bcbf729291881676df19a79e74a0fb4f2266d09f5c5de0565dedf4420759fd95`,
`b22c21f1b9410ec66aa5445f8aa2f9aa4e4149c02b733abe03617ec6be05c032`,
`e9802845ac23e54fb14617dc21a63ac1a5be0d5b64e998bf0b8cd0ff1a288d62`, and
`ffb9b95199d62bce49a95e044b93f09fd0562b74b8917b069e26da0d793ca452`.
Focused 1/1 and full sibling 18/18 (146.26s) pass; strict workspace Clippy,
formatting, and diff checks pass. Production now completes Allegretto system 1
and fails closed in system 2 at queue 89 x52/SIG43 RIGHT/TOP, whose builder has
a start item, one chunk, and two BeamLinkers. That beam-bearing head-origin
expansion is next.

## Boundary 170: generic beam-bearing head-origin C-link

The strict Allegretto system-2 replay mutates heads 0-88 without snapshots and
emits queue 89 twice byte-identically. x52/SIG43 RIGHT/TOP walks a stump-less
start, active chunk glyph 2206, RawBeam 32, and RawBeam 31. Java returns from
the final sibling BeamLinker inside `CLinker.expand`, retaining the initial
x52 HeadStem relation while evaluating both BeamStem relations and checked-stem
creation from the chunk-shifted line. It creates StemInter 2386, appends one
HeadStem and two BeamStem edges, links both beam anchors plus x52 RIGHT, changes
SIG 654/619 to 655/622, and grows system stems 55 to 56.

Native now generically authenticates beam tails, materializes head-created
B-linker cells at phase-1 ownership transfer, uses the opposite contacted beam
border, and derives BeamStem maxima by active profile. It creates dense Stem 55
/ native persistent Inter 1483 and matches all three relation payloads exactly.
Fixture/runner/initializer/probe/body/semantic SHA-256 values are
`dcfec65a778983cc9615786fe7b9bd008677f456ad8d6f276edb3855be46e45a`,
`f36f312b0bc82d8cbd4fc176133339515069743a5786eed54a37f76678795986`,
`9587d9c623beea6c7922dabf6b50cd4d315ed49f4bca28bcc430684362384035`,
`4e111715e281e58c51c724130dca44b6a9c0b3149188e3063f77abd3ab58280e`,
`218d8ecd1a889e0046a49594e675572cd2884bf3f8f3411a0d166b8c3b2cbb21`,
and `01868de57f3a8f5eb42a3496c62cb141d034b85f0fdf0d3859fe37b7337bccae`.
Focused gates and the full sibling suite 19/19 (144.58s) pass; strict workspace
Clippy, formatting, and diff checks pass. Production advances to system 2
queue 111 x51/SIG36 LEFT/BOTTOM: a start stump followed by three sibling
HeadHalfLinkers. That generic multi-head expansion is next.

## Boundary 171: generic multi-head hard-tail rejection

The strict Allegretto system-2 replay mutates heads 0-110 without snapshots
and emits queue 111 twice byte-identically. x51/SIG36 selects LEFT/BOTTOM and
walks its built start stump plus sibling x48, x49, and x50 RIGHT/BOTTOM head
linkers. All four transient HeadStem checks are accepted, but the complete
item span still misses Java's hard tail target: `lastIndex=-1`, so `link()`
returns false before `createStem`. Active glyph 376 remains without a
StemInter; allocator 2387, SIG 656/626, system stems 57, and relation state are
unchanged. Java closes both x51 S sides and advances to x118/SIG57.

Native now accepts this head-only builder shape generically only for the
provable hard-tail rejection. It resolves either a free-seed stump or a built
head stump from the owned builder registry, evaluates the full item ordinate
span, and returns the ordinary no-link signal before any mutation. A head-only
shape that reaches the hard target remains fail-closed, preventing the
rejection path from silently standing in for a successful multi-head
transaction. The page therefore completes Allegretto system 2 and stops next
at system 3 queue 29 x114/SIG76 RIGHT/TOP, whose start plus sibling x112 reaches
the hard target and needs successful multi-head application.

The 7-line / 6,947-byte fixture SHA-256 is
`1d2dfdec360fcc575ef9b852cbb6502dc82ee6fa8b951d24914bf0ae1bb66063`;
runner, transformed probe, body, and semantic-pass hashes are
`55020b3e312fe20cea3913f4e1b8ac849235f8e84753be66ecdc969b6f4b3365`,
`dc7df0af651b851e3d1c67d382f42b961955b4763fe5e92583e6f30a407a832d`,
`a4f9ad50ee8b7b147a02147fbea94959b54e392c6567252a7be4caf6c1a6ef71`,
and `b81c303a6863f2c88dcc93ef442bc526937e708e136e508d4c8021dbb7af4e36`.
The runner strictly pins Boundary 170's runner/fixture
`f36f312b0bc82d8cbd4fc176133339515069743a5786eed54a37f76678795986` /
`dcfec65a778983cc9615786fe7b9bd008677f456ad8d6f276edb3855be46e45a`.
Focused 1/1 and the full sibling suite 19/19 (144.57s) pass; strict workspace
Clippy, formatting, and diff checks pass. Boundary 170's pushed commit
`f87752bbb` is exact-CI green: Build & Test 32490696521 succeeded and Rust
32490696428 passed all 12 Ubuntu/macOS shards.

## Boundary 172: built-stump two-head checked-stem creation

The strict Allegretto system-3 replay mutates heads 0-28 without snapshots and
measures queue 29 twice byte-identically. x114/SIG76 RIGHT/TOP walks its built
start stump and sibling x112 RIGHT/TOP; both resolve to active Java glyph 397.
Java creates StemInter 2398, appends two exact HeadStem relations, closes x112
LEFT/RIGHT, advances to x25/SIG4, changes SIG 644/567 to 645/569, and grows
system stems 47 to 48.

The generic native expansion now sources both start and crossed built stumps
from the owned pre-builder registry, alongside the existing free-seed route.
Native creates Stem 47 / glyph 187 / persistent Inter 1936 with the exact Java
checked grade, geometry, thickness, and relation payloads; SIG 264/301 becomes
265/303 and system stems 47 becomes 48. The operation is atomic and preserves
Java's earlier phase-two worklist entry for x112 while its new relation makes
that retry a later no-op.

Fixture/runner/init/probe/body/semantic SHA-256 values are
`4cd7ea37b5f57b27012fc52cea377394d2d0aef97954db34dee988ed823b7549`,
`a6729e51a41222156a53d772bbd64fc9c8223d14fc2eddf4769b213f09670ada`,
`c801a89d512ffc1751c178e41c6dee30a17d559bfe1b6b1822e6bc050f8b91b9`,
`d9e98b372c7baa03cdb0473162127793ef295538c9021bb7f58025d94f2d9731`,
`9b339591efe421f2a73c3c10eee7e8f092bf66f5eae506e0480a2b462e3bf5c9`, and
`b834f6c87d003428b73242a1081835096d9a63c4c36e1af53dc248ed8dad964a`.
Focused 1/1 and full sibling 20/20 (147.78s) pass. Production advances to
system 3 queue 61 x57/SIG99 RIGHT/TOP, whose builder has a stump-less start,
two chunks, and RawBeam 76. Boundary 172 commit `7e87b6c07` is exact-CI green:
Build & Test 32499929575 succeeded and Rust 32499929648 passed all 12 shards.

## Boundary 173: multi-chunk beam-bearing checked-stem creation

The strict Allegretto system-3 replay mutates heads 0-60 without snapshots and
measures queue 61 twice byte-identically. x57/SIG99 selects RIGHT/TOP after
the same transaction first considers LEFT/BOTTOM. Its stump-less builder
contains active chunk glyphs 410 and 2000 plus RawBeam 76. Java composes a new
1335:1857:4:92 glyph, creates StemInter 2402, adds one HeadStem and one BeamStem
relation, links beam linker 4, changes SIG 647/579 to 648/581 and system stems
50 to 51, then advances to queue 62 x54/SIG97.

The generic native C-link path now carries each selected chunk in order,
composes the exact run-table union, and obtains negative compound identity
proof from an exhaustive native modeled-glyph scan. It registers glyph 1939,
creates checked Stem identity 50 / persistent Inter 1940, appends HeadStem edge
310 and BeamStem edge 311, changes native SIG 267/310 to 268/312, and reaches
the same continuation. Java and Rust local IDs intentionally differ; content,
checked grade, median, thickness, relation payloads, linker writes, and state
transitions match exactly.

Fixture/runner/probe/body/semantic SHA-256 values are
`de80142ffc78b6dd96b156285c365b1997bdbb7228ae47093f1b244dea04b56e`,
`27d26355c3b58d788d96ddb3d40b3aed4c17fc7c65a0af5c477205df21690f15`,
`3318d3d122240b9e10dee6573ac3fd3c95b99c640ff229405975771ef63c4666`,
`0a8aab562930ad983c0e91fe011a8094c7f039870d10385cc64c9fd74f84a9b9`, and
`462489439a3152a10a9dc65a002845c72acb3672bcf5f81967b34d6bdbc233ff`.
The 12-line fixture is 12,990 bytes and strictly pins Boundary 172 runner
`a6729e51a41222156a53d772bbd64fc9c8223d14fc2eddf4769b213f09670ada`
and fixture
`4cd7ea37b5f57b27012fc52cea377394d2d0aef97954db34dee988ed823b7549`.

Focused 1/1 and full sibling 20/20 (148.43s) pass; strict all-features
workspace Clippy, formatting, and diff checks pass. Production crosses queue
61 and fails closed at system 3 queue 115 x113/SIG75 RIGHT/TOP. Builder 452
joins the start head to sibling x108/SIG67 and reaches Java's hard tail target.
That was the apparent next expansion; Boundary 174 below supersedes the
diagnosis by carrying queue 53's missing link. The remote baseline at this
boundary was `7e87b6c07`.

## Boundary 174: generic two-side carriage and corrected no-link frontier

The queue-115 diagnosis disproves Boundary 173's apparent next branch. Java
does not expand x113/SIG75 at all: its RIGHT/TOP builder first encounters x108,
whose RIGHT side was already linked and closed by the earlier queue-53 x107
transaction. Both queue-115 horizontal sides therefore choose `Neither`; Java
returns `false`, closes x113's two local S cells, changes no SIG relation or
system stem, and advances to queue 116 x66/SIG33.

The missing predecessor mutation came from Java `HeadLinker.linkSides`
continuing from LEFT to RIGHT after a successful first C-link. At queue 53,
x107/SIG80 first reuses Stem 2394 on LEFT, then selects RIGHT/TOP and reuses
active glyph 397 / Stem 2398. The second expansion plans HeadStem relations for
x107, x116, x117, and x108; x117's edge already exists, while the other three
are appended. Together with the LEFT edge, Java adds four edges, links both
x107 sides, propagates the shared RIGHT link to x108/x116/x117, and closes the
related sibling cells. Native now performs the complete horizontal-side loop
atomically, preserves every side transaction, reuses a same-content stump
across crossed heads, and records whether each relation was appended or reused.

The wider loop also exposes Java's legal mutated-then-unlinked return: a first
side can commit before the second side finds the same stump above and below and
returns `false`. That state is now explicit in the production outcome/event
model, retaining the mutation while recording the undefined side and phase-2
queue entry. The sibling regression pins the existing Allegretto system-2
queue-103 x85/SIG86 case and still reaches the exact queue-111 oracle state.
For weak heads with no linkable corner, the generic continuation now performs
Java's local close-and-phase-2-queue operation; rather-good retry remains
fail-closed at higher profile.

The 17-line / 17,020-byte deterministic fixture SHA-256 is
`01bda66e6eecf7d46bdd21f3d2d4d8ec977deff9bc51f01b4a3291092680fca2`.
Runner, transformed probe, emitted body, and semantic-pass SHA-256 values are
`b3c426db85a5c5402c7e8d5741e249c15905e0f2d8f4888d491ee9783982afa4`,
`4e42bfb4de50ec8a3d14c8c028b435d115f1ec55b9efe59e249120ae5887db12`,
`27bf04be971bb5705170e00646a4440fe3107fd679b4b55bd6be6ca27b0782a4`,
and `fd1a3ca321041ede2ab5d39ffb2742675b19138b5b5082a93f44dbcfed7a6185`.
It strictly pins Boundary 173 runner/fixture
`27d26355c3b58d788d96ddb3d40b3aed4c17fc7c65a0af5c477205df21690f15` /
`de80142ffc78b6dd96b156285c365b1997bdbb7228ae47093f1b244dea04b56e`;
warmup plus two fresh semantic runs are byte-identical.

Focused 1/1 and the full sibling suite 20/20 (148.29s) pass, along with strict
all-features workspace Clippy, formatting, and diff checks. The next measured
system-3 head is queue 116 x66/SIG33; Boundary 174 does not execute it. The
exact remote baseline is `02f09e64b`: Build & Test 32513292289 and Rust port
32513292385 both succeeded.

## Boundary 175: system-3 queue-116 prelinked closure

The generic continuation consumes x66/SIG33/Inter1743 with LEFT already linked
and closed through Stem2380 and RIGHT already closed. Stem2380 also carries
x67/SIG34, so Java returns `true` and closes x67 LEFT then RIGHT. Those are the
only two value changes: SIG 649/593, system stems 52, relation state, undefined
sides, and unlinked heads are unchanged. The carrier reaches queue 117
x86/SIG18/Inter1711. No production source change is needed.

The 13-line / 16,627-byte fixture is byte-identical across warmup plus two fresh
runs. Fixture, runner, transformed probe, emitted body, and semantic-pass
SHA-256 values are
`cc6b2240cc6f6fa13fa294ef17eb01cae65afc8189fba4e4a244d99d76891a8e`,
`2e2c10929798d25ea10ec0b5912288db59e5feb71f806c784fd60b445fbe89f3`,
`c0aa6ac09a1d1178134e9b0b65ad0b7166a5c77e3e2ed0f85f574b2ffecb81e3`,
`1e7e336ad5b0c7f7315ec97bfa9807c8e04d57233c29b3b4f0014fd1422e68c9`,
and `94d9b566379c926f214a9e37672e1d97a0f5287d2252a48a1d787f7373584564`.
It strictly pins Boundary-174 runner/fixture
`b3c426db85a5c5402c7e8d5741e249c15905e0f2d8f4888d491ee9783982afa4` /
`01bda66e6eecf7d46bdd21f3d2d4d8ec977deff9bc51f01b4a3291092680fca2`.

Focused 1/1 and full sibling 20/20 (151.16s) pass with formatting, strict
all-features workspace Clippy, and diff checks. The exact remote baseline is
Boundary 174 commit `02f09e64b`: Build & Test 32513292289 and Rust port
32513292385 both succeeded.

## Boundary 176: system-3 final phase-1 no-op closure

Queue 117 x86/SIG18/Inter1711 is the last of 118 phase-1 heads. It is already
linked/closed on LEFT through Stem2368 and closed on RIGHT. Stem2368 also
carries x84/SIG27 and x85/SIG28; all four sibling cells are already closed, so
Java returns `true`, emits four ordered `true->true` writes, changes zero
values, and leaves SIG 649/593, 52 system stems, relation/linker hashes,
undefined sides, and the retry worklist unchanged. Native reaches
`current_index=118`, phase-2 index zero, with retry order x112/SIG68,
x0/SIG19, x14/SIG50, x13/SIG0, x56/SIG100, x113/SIG75.

The 13-line / 16,544-byte fixture is byte-identical across warmup plus two
fresh runs. Fixture, runner, transformed probe, emitted body, and semantic
SHA-256 values are
`dbe00a31bf256a2a8c071b755e3c3df4e95e3ecce45f9d7020729ae0705e9caf`,
`088128d72a928ac4a16439e1fa61c857901b793ccbc20e79231c0070e7e50086`,
`f17ce2eead270d2cc2d4390218440f408544b345806d8d683a29451cc90b7c2d`,
`567b8ebb998d7d75e46380c7740e7259454936be517771816aaca4e7369d0478`,
and `69eaf824e4c50b706f2c22c446e465afa966d957a04b2d389ce9a2cad0ba70ad`.
It strictly pins Boundary-175 runner/fixture
`2e2c10929798d25ea10ec0b5912288db59e5feb71f806c784fd60b445fbe89f3` /
`cc6b2240cc6f6fa13fa294ef17eb01cae65afc8189fba4e4a244d99d76891a8e`.

Focused 1/1 and full sibling 20/20 (154.71s) pass with formatting, strict
all-features workspace Clippy, and diff checks. Boundary 175 commit
`ef4ee3e00` is the exact remote baseline: Build & Test 32516450490 and Rust
port 32516450484 both succeeded, with all 12 Rust shards green. Next is phase-2
retry index 0 x112/SIG68.

## Boundary 177: full-page x0 chunk rejection and retry-queue correction

The full foreground-page Java lifecycle disproves the minimized predecessor's
six-head retry queue. At phase-1 order 100, x0/SIG19 links
RIGHT/BOTTOM from the valid 369:1595:2:48, weight-63 start stump. Java accepts
that stump, then rejects the next plain chunk because its centroid is more than
`0.2 * interline` from the evolving stem line. `CLinker.expand` therefore
returns the preceding `lastIndex=0` of `maxIndex=1` immediately, deliberately
skipping its final hard-tail/relation recheck. The native C-link path now
carries accepted content and its translated line incrementally and reproduces
that early stop under the exact authenticated Allegretto system-3 x0 frontier;
other hard-tail failures, including x14 and x13, remain queued.

The transaction creates Java StemInter3170 from the start stump alone, with
grade bits `3fe49d64653090d5`, bounds 368:1595:3:48, median bits
40771723de22d21c:4098ec0000000000:40771f7fd38ffa01:4099ac0000000000,
and width bits `3ff5000000000000`. Java SIG 266/315, system stems 51, and
allocator 3169 advance by one vertex, one edge, one stem, and one ID. Native's
created checked-stem transaction has the same geometry/grade and corresponding
one-step graph/stem/identity deltas. Phase 1 still ends at index 118, but the
correct retry queue is now exactly x112/SIG68, x14/SIG50, x13/SIG0,
x56/SIG100, x113/SIG75.

The full-page fixture contains 33 lines / 16,196 bytes: one x0 transaction,
three system baselines, all 25 Java phase-2 retry rows, and a strict summary.
Warmup plus two fresh passes are byte-identical. Fixture, runner, transformed
probe, emitted body, and semantic-pass SHA-256 values are
`242260a9fe7b873ca8597840ea7253d45d6518742e924496ccc4a14bb2a8c41c`,
`9196aa6841aba9d234c4a82d21185c4ed1367b0329fcfca9930c14f0c6a15331`,
`e2255ffc6ff5c4b73d01afba083fba07cff682f5e4148c36a921d3184c9c952b`,
`d96572e2ca0ca46e55a3a2997a5bc6dc7d1977214068571ac0497b62f94c936b`,
and `d96572e2ca0ca46e55a3a2997a5bc6dc7d1977214068571ac0497b62f94c936b`.
Strict Boundary-176 runner/fixture pins are
`088128d72a928ac4a16439e1fa61c857901b793ccbc20e79231c0070e7e50086` /
`dbe00a31bf256a2a8c071b755e3c3df4e95e3ecce45f9d7020729ae0705e9caf`.

Focused 1/1 and full sibling 20/20 (157.51s) pass with formatting, strict
all-features workspace Clippy, and diff checks. Boundary 176 commit
`8185667b7` is the exact remote baseline: Build & Test 32519244924 and Rust
port 32519244803 both succeeded, with all 12 Rust shards green. Next is
system-3 phase-2 retry index 0, x112/SIG68.

## Boundary 178: Allegretto system-3 phase-2 retry zero

The existing generic `append=true` continuation consumes x112/SIG68/Inter1812
without a production seam change. Its closed LEFT side is deliberately
re-evaluated in phase 2 and finds neither corner linkable; RIGHT is already
linked/closed, so Java short-circuits `true`. The native ordered closure visits
both sides of x114/SIG76, x117/SIG72, x107/SIG80, x116/SIG71, and x108/SIG67.
All ten writes are idempotent, matching Java's empty `sideChanges` and zero
changed values. SIG 267/317, 52 system stems, allocator 3170, undefined RIGHT,
and the five-head worklist remain unchanged while `phase_two_index` advances
from zero to one.

The strict gate reuses Boundary 177's full-page fixture/runner
`242260a9fe7b873ca8597840ea7253d45d6518742e924496ccc4a14bb2a8c41c` /
`9196aa6841aba9d234c4a82d21185c4ed1367b0329fcfca9930c14f0c6a15331`.
It pins the exact Java retry row, including grade bits `3fe8d8c228e9b518`,
Neither/SkipAlreadyLinked decisions, unchanged sides and graph counts, and
the preserved undefined RIGHT side. Focused 1/1 and full sibling 20/20
(161.95s) pass with formatting, strict all-features workspace Clippy, and diff
checks. Boundary 178 commit `e99e93a92` is the exact remote baseline: Build &
Test 32528147579 and Rust port 32528147610 both succeeded, all 12 Rust shards
green. Next is
phase-2 retry index 1, x14/SIG50, whose real append mutation remains
fail-closed.

## Boundary 179: Allegretto system-3 phase-2 x14 append

The new bounded phase-2 transaction authenticates retry index 1 at
x14/SIG50/Java Inter 1777, evaluates both sides in append mode, and atomically
executes the successful RIGHT/BOTTOM C-link after LEFT/TOP returns `-1`.
The generic C-link parser now handles the measured start-head, crossed-head,
chunk order. It selects native glyph 204 (Java glyph 414,
550:1581:3:88, weight 194), reuses existing Stem 3148 (native identity 30 /
vertex 247), preserves x15's existing relation edge 256, and adds only x14's
HeadStem edge 327. SIG vertices/system stems/allocator stay 267/52/3170 while
SIG edges advance 317 to 318.

Stem geometry and grade are exact. x14's relation grade, dx, extension, and
consistency bits are `3fed98996cac8bf2`, `3f9c4c548b8fedb7`,
`408134a485dee59d:4098840000000000`, and `3ff7f2116a3b35fd`.
The reused stem closes x15, x18, and x19 LEFT then RIGHT idempotently. Native
restores the exhausted phase-1 cursor, advances `phase_two_index` from one to
two, and stops before x13/SIG0/Java Inter 1675 (grade bits
`3fc5aea35e22900d`).

The dedicated 6-line / 3,825-byte minimized Java oracle passes warmup plus two
byte-identical fresh runs. Fixture/runner/transform/init/body/input/base-probe/
source/transformed-source hashes are
`f8a18f4ac17d036e0f3481983474d3569668437c6d53670b7f454f707baad1ba`,
`5f530a9fca946f6ed74877713452b7a64fd66f98810654113a700cd6ee61ced3`,
`69258e54539f10d7771718a8660b2e012db286c4cfdc7285876831da64f77c92`,
`b7c2b721836f8238295dfe0ec01b5add5b1b181a82876fa3420c255a205213b8`,
`cc3d82763e50f425ff96c8551f3e7fdcc3bb55d594a904cb4bb02087f278dd2b`,
`a9207f26b57415d8c54602881316c003319c5593ed8baf4c3af13715c41b3065`,
`7b467c57b65e57aa052296164129ae8c016d82756c9f804d8e1072747b0a76b2`,
`f51893627e9e1ddaca77daba9166098cfa6d8cc99ff8d094aa9138c13ad78993`,
and `76d5028c4756a2cbd01f9f5514639fbea222339755f9deba318749feacfba24a`.
The strict Boundary-177/178 runner/fixture pins are
`9196aa6841aba9d234c4a82d21185c4ed1367b0329fcfca9930c14f0c6a15331` /
`242260a9fe7b873ca8597840ea7253d45d6518742e924496ccc4a14bb2a8c41c`.

Focused 1/1, full sibling 20/20 (163.26s), and the canonical standard-feature
workspace suite pass with formatting, strict all-features workspace Clippy,
and diff checks. Boundary 178 commit `e99e93a92` is the exact remote baseline:
Build & Test 32528147579 and Rust port 32528147610 both succeeded, all 12 Rust
shards green. Next is retry index 2, x13/SIG0.

## Boundary 180: Allegretto system-3 phase-2 x13 append

The bounded x13 transaction factors x14's authenticated shared-stem operation
into one generic helper while retaining exact queue index, head, SIG, and grade
guards per public entry point. At retry index 2, x13/SIG0/Java Inter 1675
selects RIGHT/BOTTOM after LEFT/TOP expansion fails, resolves native glyph 204
and existing Stem 3148 (identity 30 / vertex 247), preserves x15 edge 256,
and appends only x13 HeadStem edge 328. SIG edges move 318 to 319; vertices
267, system stems 52, allocator 3170, and glyph identity do not change.

The relation grade/dx/extension/consistency bits are
`3fed98996cac8bf2`, `3f9c4c548b8fedb7`,
`408134a485dee59d:4098840000000000`, and `3ff7f2116a3b35fd`.
The shared stem visits x15, x18, x19, and x14 LEFT then RIGHT with zero value
changes. `phase_two_index` advances from two to three before
x56/SIG100/Java Inter 1876 (grade bits `3fc5165a40f2ed07`).

The strict 6-line / 3,813-byte minimized oracle passes warmup plus two
byte-identical fresh runs. Fixture/runner/transform/init/body/probe/source/
transformed-source hashes are
`4ebbaa69132cdee430d38b9b27622ae1e64e0d12554ead8e6a782ab8dcdbde3f`,
`1bdfd26b350170a8f4d17290ea6f336f544b6ee8ee9dc1566bcf00654cd59ac2`,
`42dbccb9b9f05178358c54488aec0d8ae3339aca6083b25b1f73aff069c59a10`,
`c4a870d654f1a60c4fe8be37f63806b676858d659fc220c08d4432f70c6253e9`,
`33c4f489a66eefbb11034857f0d2cb991d47fb7582b943358da25817a1e2d60c`,
`7b467c57b65e57aa052296164129ae8c016d82756c9f804d8e1072747b0a76b2`,
`f51893627e9e1ddaca77daba9166098cfa6d8cc99ff8d094aa9138c13ad78993`,
and `b2106f6b3e20eeedb46bf0e6926dc6b760581edcb6d65fd381401596c65c71ad`.
It directly pins Boundary 179's x14 runner/fixture at
`5f530a9fca946f6ed74877713452b7a64fd66f98810654113a700cd6ee61ced3` /
`f8a18f4ac17d036e0f3481983474d3569668437c6d53670b7f454f707baad1ba`.

Focused 1/1, full sibling 20/20 (146.77s), and the canonical workspace suite
pass with formatting, strict all-features workspace Clippy, and diff checks.
Boundary 179 commit
`5fd12958bf65fca9aa78896924ace95b05ec7def` is the exact remote baseline:
Build & Test 32536290867 and Rust port 32536290886 both succeeded, all 12 Rust
shards green. Next is retry index 3, x56/SIG100.

## Boundary 181: Allegretto system-3 phase-2 x56 no-link

The generic append continuation consumes retry index 3 at
x56/SIG100/Java Inter 1876 without new production code. Both carried sides
are closed/unlinked. LEFT is TopOnly and RIGHT is BottomOnly, but both
selected expansions return `-1`, so Java/native return `false`, revisit x56
LEFT then RIGHT idempotently, and advance `phase_two_index` to four. SIG
267/319, stems 52, allocator 3170, glyph identities, and undefined sides are
unchanged.

The existing full-page phase-two fixture/runner
`242260a9fe7b873ca8597840ea7253d45d6518742e924496ccc4a14bb2a8c41c` /
`9196aa6841aba9d234c4a82d21185c4ed1367b0329fcfca9930c14f0c6a15331`
provide the strict Java row: grade `3fc5165a40f2ed07`, exact
TopOnly/BottomOnly decisions, `returned=false`, empty `sideChanges`, and
unchanged graph/allocator counts. Focused 1/1 (3.72s), full sibling 20/20
(150.19s), formatting, strict all-features workspace Clippy, and diff checks
pass.

Boundary 179 `5fd12958bf65fca9aa78896924ace95b05ec7def` remains the exact fully green
remote baseline (Build 32536290867; Rust 32536290886, 12/12). Boundary 180
`9dcdb0c179d0af044a79fb4419119f770f5f6ef9` is pushed; Build 32542247629
is green and Rust 32542247645 was superseded and cancelled. Boundary 181
`4c06c26bf17875c0c16a1f63174b02822dfda0cb` is pushed; Build 32542733505 is
green while Rust 32542733478 remains queued. Next is the final retry, index 4
x113/SIG75.

## Boundary 182: Allegretto system-3 final phase-2 x113 append

`advance_native_stems_head_phase_two_append_c_link_allegretto_system3_x113`
authenticates retry index 4 at x113/SIG75/Java Inter 1826. LEFT is `Neither`
and RIGHT is `TopOnly`; the selected RIGHT/TOP C-link reuses native glyph 187
(Java glyph 397) and the checked stem created at queue 29, native identity 47 /
vertex 264 / Java Stem 3165. It preserves crossed x108/SIG67 edge 310 and adds
only x113 HeadStem edge 329, so native edges advance 319 to 320 while vertices
267, stems 52, allocator 3170, and glyph identity remain unchanged.

The new relation grade/dx/extension/consistency bits are
`3fea63f9c75cf906`, `3fb0115caff3c30c`,
`40a12ea2d934ddfe:409dfc0000000000`, and `3ffd1d9afe422d47`.
Shared-stem closure visits x114, x112, x117, x107, x116, and x108 LEFT then
RIGHT in that order; all twelve writes are idempotent. Native advances
`phase_two_index` from four to five, exactly exhausting the corrected five-head
retry queue.

The dedicated 6-line / 3,807-byte minimized Java oracle is byte-identical
across warmup plus two fresh runs. Fixture/runner/transform/init/body-semantic
SHA-256 values are
`83e4c5671e6e1d489c84d30ff0bd5e01c3b095c68b8562d2f09c42908b49f1af`,
`4f589fb9512f2b7d6467b98c9174b81ec91783a002455ee4c7ae908c1e4aa854`,
`f143d4f4d49d4fc67cb4ebd883768dfc7a7a11fd9cc918d784cc50a41c8ee00f`,
`302235acd663a6ebfeda7bceeaab336e77a990baa152012740aa41925af8b09f`,
and `c1b20ce77aa8cbb727e45dd2a078ef663bd1e59f82b871b26acd26cd417db385`.
The runner directly pins Boundary 180's x13 runner/fixture at
`1bdfd26b350170a8f4d17290ea6f336f544b6ee8ee9dc1566bcf00654cd59ac2` /
`4ebbaa69132cdee430d38b9b27622ae1e64e0d12554ead8e6a782ab8dcdbde3f`.

Focused 1/1 (3.68s), full sibling 20/20 (148.18s), formatting, strict
all-features workspace Clippy, and diff checks pass. Boundary 179 remains the
exact fully green remote baseline.

## Boundary 183: Allegretto system-3 generic `finalizeStems`

The unchanged generic `finalize_native_stems` consumes the exact exhausted
Boundary-182 carrier. It checks 118 heads, reports x107/SIG80 as the sole
multi-stem head, and reports x56/SIG100 as the sole stemless and abnormal head;
x112/SIG68 RIGHT remains the carried undefined side. Java/native remove no
HeadStem relation, change no abnormal flag, and preserve SIG 267/320, stems 52,
allocator 3170, and the complete carrier byte-for-byte.

The full-page Allegretto Java oracle covers all three systems and is
byte-identical across warmup plus two fresh runs. Fixture/runner/probe/init/body
SHA-256 values are
`cfb9e6011ed29aa30e6e90db6eeae931a3a6533d7339d80519a5ddd650c0ff0c`,
`abafa7d183ae151baa7ed4d8005257c562e0c49fb939fe931a7571994d70d890`,
`9b5e9dbefbf400887f49feba934c573d851c67e65b3e43bfaabc86d6f2c36714`,
`e0ff89792bf75286317ef011e079f338696d29cc14918f4a3018307ba4ed9548`, and
`3add75f32b08d8836817483175425872814f10aa18c0c14bef86e3306dddc8f1`.
The direct Boundary-182 predecessor pins are
`4f589fb9512f2b7d6467b98c9174b81ec91783a002455ee4c7ae908c1e4aa854` /
`83e4c5671e6e1d489c84d30ff0bd5e01c3b095c68b8562d2f09c42908b49f1af`.
Focused 1/1 (3.86s), sibling 20/20 (153.23s), formatting, strict
all-target/all-feature workspace Clippy, shell syntax, and diff checks pass.
Boundary 184 below begins wider-corpus generic STEMS completion.

## Boundary 184: Zizi system-1 duplicate-idempotent closure

At Zizi system 1 head order 34, x26/SIG106/Java Inter 1055 links LEFT/BOTTOM
through Stem1690 and RIGHT/TOP through Stem1691; both stems also reach x28.
Java completes both C-links and then writes x28 LEFT/RIGHT false-to-true followed
by the same two cells true-to-true. The generic native two-side driver now
defers inner atomic closure flags/evidence until both side mutations have
committed, then runs the shared-stem closure once. Per-stem duplicate heads are
still suppressed, while the same S cell reached through distinct stems remains
an exact ordered idempotent write.

The transaction preserves 238 vertices, 44 stems, and the native allocator;
edges advance 242 to 244 and the queue reaches 35 before x68/SIG102. The
fixture/runner/transform/init/probe/body hashes are
`0970b0dafe3a456d30e72b55a2716205e06caa4a93367e9390f00263139117f6`,
`de07f1e244641a2f9f41379b871595201b5158428e28d0f1701927b7221b7f90`,
`db0196bc8088e45ee550e7cc595f799bdcda079ce595c1bbf70c5994d06965ca`,
`55836b16d632f805b78427fb2c969becffb8f2c97df1c361d47be673fe169ca2`,
`f14692de5a59a0153ed58ded0cf18d5f736e57e327f3cf7fa5e26b9cfe0e3d4e`,
and `670de47539abe7f140f66fe77e812bb53ddc42982fb5a95a712ec56c71d88313`;
warmup plus two fresh runs are byte-identical. Focused 1/1, sibling 21/21,
formatting, strict workspace Clippy, shell syntax, and diff checks pass.

The live production drive clears system 1 and next fails closed at Zizi system
2 queue 107, x89/SIG64 RIGHT/TOP. Builder 356 profile 1/1 contains the x89
start half-linker, filament-0 chunk, and x94/SIG61 LEFT/TOP target half-linker;
x90/SIG55 LEFT is already undefined. Commit `f4629fa1d` is the exact current
green baseline (Build 32545226391; Rust 32545226371).

## Boundary 185: Zizi system-2 crossed-head stump expansion

The generic head C-link expansion now follows Java's complete ordered item
walk rather than treating only start stumps and chunks as candidate geometry.
Each crossed `HeadHalfLinker` checks its relation against the current line
before contributing its reachable stump; a later plain chunk can then fail
`maxLineGlyphDx` and return the preceding accepted item without discarding the
crossed relation. The generic operation appends every accepted crossed
HeadStem relation to the reused or newly created Stem. Its hard-tail tracker
also matches Java exactly: `lastY` starts at the theoretical line's original
P1 before the working line is reversed for upward expansion. That correction
prevents a remote target endpoint from satisfying the tail before any item is
traversed and preserves the established Allegretto x0 and Batuque x109
branches without sheet-specific logic.

The exact Zizi system-2 order-23 frontier is x94/SIG61/Java Inter 1183
LEFT/BOTTOM. Java accepts x94, then crossed x89/SIG64 RIGHT/BOTTOM, selects
active glyph 245 (`1940:913:4:57`), rejects the following chunk, and creates
StemInter 1724. HeadStem edges 1183->1724 and 1191->1724 take SIG 444/384 to
445/386 and system stems 45 to 46. The transaction closes x89 and x93 LEFT
then RIGHT; the following prelinked continuation revisits x93 and advances to
order 24, x86/SIG94/Java Inter 1253. When production later reaches x89, the
new crossed relation means the former queue-107 failure is already linked.

The strict 9-row-plus-summary Java fixture is byte-identical across warmup plus
two fresh runs. Runner/init/fixture/probe/overlay/body/semantic SHA-256 values
are `33f2ce87e7c727156de4250410052b95dbd209590419c15bb2428be3edec8b9b`,
`46241f0adbc0ef8746240567b2b54d09ffad062962e07f4deee9c745e6b43d97`,
`fb9797eb2039cf3f052f7bd7285a94b737a8771075406f772261deded352be9d`,
`b4375a1d44e7e513a0946520ca146fc84de6dcf8b9c3297c1cb8def09bdb6c5d`,
`f21487398d9ba162b6459f8f5e1265d56ffc6a8a58e6aa514a03553ee3d05df4`,
`5a9c6ad49ca15fb61a765a4334a0cf40868645d8810801dc2f18655829f90954`,
and `d5ad96dee3d46dedcb150d263c9f350cf2353c09cfc5134ef45456b1803f2a43`.
The runner directly pins Boundary 184's runner/fixture at
`de07f1e244641a2f9f41379b871595201b5158428e28d0f1701927b7221b7f90` /
`0970b0dafe3a456d30e72b55a2716205e06caa4a93367e9390f00263139117f6`.

Focused Zizi, preserved Allegretto, and preserved Batuque gates pass; the full
sibling suite passes 22/22 in 156.26s. Production `-step STEMS -json` now
completes Zizi with schema 1, while formatting, strict all-target/all-feature
workspace Clippy, and diff checks are green. `4de83dc30` is the exact fully
green predecessor baseline (Build 32547802513; Rust 32547802498). The next
wider-corpus production drive fails closed at Carmen system 1's unported
dual-corner selection branch.

## Boundary 186: Carmen system-1 shared-stump dual corners

The generic initial phase-1 transfer now evaluates both live reachability
stumps when TOP and BOTTOM can link from the same open side. Equal non-null
stumps reproduce Java's undefined return: native queues the side/head for
phase 2, emits an empty prefix-closure record, and advances without graph,
stem, allocator, or S-cell mutation. Differing or missing stumps choose
BOTTOM for LEFT and TOP for RIGHT. A prefix consisting entirely of completed
heads can now return a consumed queue-terminal carrier.

Carmen system 1 consumes all 45 heads this way. x39/SIG3 LEFT and x38/SIG2
LEFT are queued in Java order; their two corners respectively share native
seed stumps 24 and 25. The native carrier remains 161 vertices / 172 edges /
18 stems at index 45. Java remains 163/175/18 with allocator 3253 and retains
both abnormal no-stem heads through finalization. The deterministic two-row
fixture plus summary pins runner/fixture/body hashes
`070c3febcf34348fc8ce643c17d99757a7845daf4f1379e591a7922b1a0da1b9`,
`28018b4010fc1a08a45569298b06f737164c86398a2e46f277bceb869fedf089`,
and `27c8e7343d2beff061e04cf1f1e9efb18078afee943923aa14ada60a88dc22aa`;
input/StemsRetriever/probe/init are
`249330d6558d410f64f550180d3a659dd3c9c340dcdcb5ae08e809c273fe2e44`,
`26e95fa09905b39ea0dcae2b65a85b4e4fcb49b772c57f97f332a00c4dc8b9e7`,
`9b5e9dbefbf400887f49feba934c573d851c67e65b3e43bfaabc86d6f2c36714`,
and `e0ff89792bf75286317ef011e079f338696d29cc14918f4a3018307ba4ed9548`.
The strict Boundary-185 runner/fixture pins remain
`33f2ce87e7c727156de4250410052b95dbd209590419c15bb2428be3edec8b9b` /
`fb9797eb2039cf3f052f7bd7285a94b737a8771075406f772261deded352be9d`.

Focused 1/1, sibling 23/23 (153.37s), formatting, strict all-target/all-feature
Clippy, shell syntax, and diff checks pass. Production Carmen clears system 1
and next rejects system 2 queue 70 x13/SIG10 RIGHT/BOTTOM: builder 55 is the
ordered 31-pixel start stump, 5-pixel Gap, and 51-pixel filament-0 chunk, with
carried undefined LEFT sides x37/SIG20, x38/SIG24, and x36/SIG23. That
Gap-aware expansion is next. `425d58e82` is the exact green predecessor
(Build 32551514978; Rust 32551514933, 12/12).

## Boundary 187: Carmen system-2 show-stopping gap no-link

The generic head C-link expansion now accepts typed `Gap` items and applies
Java's profile-specific `maxYGap` rule. A gap never advances `lastY`. When
its contribution exceeds the threshold before the hard tail target, expansion
returns no-link immediately with no candidate creation, glyph registration,
allocator change, relation, SIG edge, or system stem. If the hard tail was
already reached, the walk stops at the preceding item. The separate
soft-target/following-glyph shortcut remains explicitly fail-closed until a
deterministic Java transaction authenticates it. Generic no-link closure now
also writes the current head's S cells in Java's LEFT-then-RIGHT EnumMap order.

Carmen system 2 queue 70 is x13/SIG10/Java Inter 2252. Java first rejects
LEFT/TOP and then reaches RIGHT/BOTTOM builder 55: a 31-pixel start stump,
5-pixel Gap, and 51-pixel chunk. The wide gap occurs before the 37-pixel hard
tail. Active glyph 465 / native candidate content `628:1081:3:47` is
observed but not registered or attached. Both attempts return false; x13
LEFT and RIGHT close in order and the head joins phase 2. Native and Java each
preserve their pre-transaction graph, stem, and allocator state and advance to
queue 71, x27/SIG16/Java Inter 2266. Java's independent transaction remains at
1040 vertices, 824 edges, 33 stems, and allocator 3366.

The 4-row-plus-summary fixture is 7 lines / 5,474 bytes and is byte-identical
across warmup plus two fresh runs. Runner/init/fixture/probe/body/semantic
SHA-256 values are
`c0516e21259912bc5ec1b429878dfc5d26b44a1c54076d1cc7eace3cd700194d`,
`cdd0f38b472bd6c29b90d389783e99b16b788578cdb6ab409632c612ad86c5f6`,
`6bf4d983a98070b7d29089ae8771234838697457b7321c0110452651dd5bb0ff`,
`bbd9d309d51dc66c6703127397a72191342a59076af75e84ba039dd0bc846aa9`,
`781c4627ceef9fcf378ee07ef56fefd4d098a99d6a08d50db1961f00d6c39158`,
and
`c3456f9c96304a256b19c3668fe5e77e1c0e889764458e6246554abaa4a6e0d7`.
The runner strictly pins Boundary 186's runner/fixture at
`070c3febcf34348fc8ce643c17d99757a7845daf4f1379e591a7922b1a0da1b9` /
`28018b4010fc1a08a45569298b06f737164c86398a2e46f277bceb869fedf089`
and retains the shared fragment/overlay hashes.

Focused 1/1, full sibling 24/24 (153.04s), formatting, strict
all-target/all-feature workspace Clippy, oracle shell syntax, and diff checks
pass. The atomic production Carmen drive clears system 2 queue 70 and now
fails closed at system 5 queue 62, x71/SIG7 LEFT/TOP, builder 286: start stump,
chunk, then a stump-less crossed x68/SIG0 head relation. That relation-only
crossed-head expansion is the next wider-corpus branch. `425d58e82` remains
the exact fully green remote CI baseline; Boundary 186 is pushed at
`1d8cbb002` but has no visible workflow run yet.
## Boundary 188: complete Carmen head phase 1

Generic head expansion now admits a relation-only crossed
`HeadHalfLinker`: it projects and records the head relation while leaving
candidate raster content unchanged when the item has no glyph. The close-head
predicate now follows Java's recursive Gap rule instead of failing closed. It
measures the concrete diagonal prefix before the Gap, tries the target head's
opposite diagonal recursively with cycle protection, and applies Java's
deliberate true fallback when neither complete diagonal can link. The existing
show-stopping Gap bound remains unchanged.

Carmen system 5 queue 62 is x71/SIG7/Java Inter 2813. LEFT/TOP builder 286
contains the start stump, active glyphs 614 and 3126, the direct x71 relation,
and a relation-only crossed x68/SIG0 target. Java selects both glyphs and both
HeadStem relations, including relation grade bits
`3fe955058d9897c0` for crossed x68, but still ends at
`lastIndex=-1,maxIndex=2`: the walked content falls short of the hard tail.
No candidate, allocator, glyph, vertex, edge, or system-stem state changes.
The false result closes x71 LEFT then RIGHT and advances to queue 63,
x45/SIG95/Java Inter 2990.

The production phase-1 driver now exhausts all five Carmen systems. System 5
retains unlinked heads `[(72,8),(71,7),(47,101)]` and the one undefined side
`(72,8,LEFT)`. The next honest fail-closed frontier is Carmen system 2's
first phase-2 retry: it reaches the still-unported `reuseStem` append path.

The 4-row-plus-summary fixture is 7 lines / 6,051 bytes and is byte-identical
across warmup plus two fresh runs. Runner/init/fixture/probe/body/semantic
SHA-256 values are
`9cdf28ad67460f64ab4273020e177fa82626d8eeb781a0d2b26f4fb4ad48a423`,
`5c66ada545193659e444da598fc0924e7cd5c2463a7cd0db5a8e744431c6af07`,
`6ee7e36c9294bcb861c128f11b25072ba5f7f84dec3f61a00b4df8d282054358`,
`e286786ecf4b8a0eec20bd6b81253f02b1167bc63de1832951da95880e05d979`,
`b786cbfa0d15a8b7da4e46d8b898d3872b284a53378c46d0e62fc4a3d97544bf`,
and
`cd95a20e3c2b0035b8464ebf19d7545edd9c9b1ff2cb871510dd96dfb317c0b3`.
The runner strictly pins Boundary 187's runner/fixture at
`c0516e21259912bc5ec1b429878dfc5d26b44a1c54076d1cc7eace3cd700194d` /
`6bf4d983a98070b7d29089ae8771234838697457b7321c0110452651dd5bb0ff`
and retains the shared fragment, GlyphIndex-source, overlay, and input hashes.

Focused 1/1, full sibling 25/25 (152.58s), formatting, strict
all-target/all-feature workspace Clippy, oracle shell syntax, and diff checks
pass. `425d58e82` remains the exact fully green remote CI baseline; no workflow
run is visible for pushed Boundary 187 commit `2f5b818fc`.

## Boundary 189: Carmen system-2 phase-2 final-relation no-link

Java's phase-2 `CLinker.expand` can reach the hard tail and still return
`-1` when the final start-head relation is rejected. The generic bounded
projection now distinguishes that result from an accepted relation that may
mutate through `reuseStem`. It reconstructs the selected plain chunk
contents, updates the candidate line in Java order, projects the HeadStem
relation, and returns no-link only when the relation is rejected. Richer
stump/crossed-head shapes remain fail-closed.

Carmen system 2 phase-2 queue 0 is x20/SIG43/Java Inter 2318. LEFT/TOP ends
before its hard tail after selecting active glyphs 457 and 3448. RIGHT/BOTTOM
selects glyph 3449 and reaches its hard tail, but the final relation is null.
Java returns false with no side, graph, stem, glyph, relation, or allocator
mutation; native records the current head's idempotent LEFT-then-RIGHT closure
writes and advances the phase-two cursor from 0 to 1. The unchanged generic
operation then consumes the remaining eight no-link retries, completing all
nine Carmen system-2 phase-two entries. The page drive now fails closed at
Carmen system 3 queue 1, x1/SIG53: the first measured successful
`reuseStem` append (queue 0 is a no-link).

The 4-row-plus-summary fixture is 7 lines / 3,089 bytes and is byte-identical
across warmup plus two fresh runs. Runner/transform/fixture/body/semantic
SHA-256 values are
`d3c7fd2c2183a4b296903006938894f0e1204e5f3c6c8d879ee011ad69baa9cb`,
`23914c17f353f0c140474fab16bb9d6fbe62482b42821d21de6d98920ef33b4e`,
`51ffc157e92fafce82f8bdc2797e7cb2947e140a70587cf0c1fe87b7c6e9b5e0`,
`5a7f830ff69b2123011fc5bcb18b9ccfab16b59b263c598e482fa52d8432753d`,
and
`5a7f830ff69b2123011fc5bcb18b9ccfab16b59b263c598e482fa52d8432753d`.
The runner strictly pins Boundary 188's runner/fixture at
`9cdf28ad67460f64ab4273020e177fa82626d8eeb781a0d2b26f4fb4ad48a423` /
`6ee7e36c9294bcb861c128f11b25072ba5f7f84dec3f61a00b4df8d282054358`;
the transformed HeadLinker SHA-256 is
`cb1f310b26ed3b5e29b84fbe3fe72f09768c2dbf3ef369abf9b29d326d4ac931`.

Focused 1/1 and full sibling 25/25 (152.46s) pass. Formatting, strict
all-target/all-feature workspace Clippy, oracle shell syntax, and diff checks
pass. `425d58e82` remains the exact fully green remote CI baseline.

## Boundary 190: Carmen system-3 phase-2 reused-stem append

The bounded phase-2 reused-stem transaction is now shared across authenticated
systems instead of naming Allegretto in its implementation. Carmen system 3
queue 0 x26/SIG54 first returns no-link without mutation. Queue 1
x1/SIG53 then selects RIGHT/BOTTOM (`lastIndex=maxIndex=2`), resolves native
glyph 182 to already attached native Stem identity 6 / vertex 242, and appends
only the missing x1 HeadStem edge 323. Relation grade/dx/consistency bits are
`3fee44da1a6b455d` / `bfa58edf7166c000` / `3ff94e5e0a72f054`.
The crossed x3/SIG13 relation remains the pre-existing edge 198.

The graph changes from 279/323 to 279/324 while 43 system stems and the
allocator remain unchanged. Java-order closure writes x3, x6, and x7
LEFT-then-RIGHT; all six are idempotent. Only x1 RIGHT changes from
unlinked/closed to linked/closed, and the phase-two cursor advances from 1 to
2. The production page driver consumes queue 2's ordinary no-link and now
fails closed precisely at queue 3 x0/SIG3's next real append.

The three-row-plus-summary fixture is 6 lines / 3,915 bytes and is
byte-identical across warmup plus two fresh runs. Runner/retarget-transform/
fixture/body/semantic SHA-256 values are
`e0bf5408f12c652e530990c35bce21ca3ec64bd610d02139919198133dccb4f8`,
`a452fbc760da01105bcd445af2461a6d0fcc7dbfad35fe31ff66d41fc7b2b79e`,
`f9656d9bb2a917fbd059c58c0692803d8d8fd3c714ed95d3ac981d9e3604c8e0`,
`e4774f68f89c64a93d52bda54944a19c9ab992ca5c8eda2741c168ff2c3a496f`,
and
`e4774f68f89c64a93d52bda54944a19c9ab992ca5c8eda2741c168ff2c3a496f`.
The runner pins Boundary 189's runner/fixture at
`d3c7fd2c2183a4b296903006938894f0e1204e5f3c6c8d879ee011ad69baa9cb` /
`51ffc157e92fafce82f8bdc2797e7cb2947e140a70587cf0c1fe87b7c6e9b5e0`
and the reused Allegretto x14 transform/init at
`69258e54539f10d7771718a8660b2e012db286c4cfdc7285876831da64f77c92` /
`b7c2b721836f8238295dfe0ec01b5add5b1b181a82876fa3420c255a205213b8`.

Focused 1/1 and full sibling 25/25 (156.99s) pass. Formatting, strict
all-target/all-feature workspace Clippy (14.82s), oracle shell/AWK syntax,
and diff checks pass. `425d58e82` remains the exact fully green remote CI
baseline.

## Boundary 191: ordered append reuse completes Carmen STEMS

Java's append-mode `reuseStem(lastIndex)` is not necessarily the selected
candidate's already attached stem. It scans the current C-linker and preceding
builder items in order, and may reuse an earlier crossed head's stem instead.
The generic native C-link transaction now performs the same scan from owned
builder and SIG state, retains the selected candidate-stem provenance, records
the independently chosen append reuse, and targets the new HeadStem relation at
that ordered result.

Carmen system 3 phase-2 queue 3 is x0/SIG3/Java Inter 2405. RIGHT/BOTTOM
selects Java glyph 531 (native glyph 218), whose candidate is the short Java
Stem 3984 / native Stem identity 41. Java's ordered scan crosses x3/SIG13 and
instead reuses long Stem 3949; native resolves the same pre-existing edge 198
to Stem identity 6 / vertex 242 and adds edge 324 from native head vertex 133.
The graph moves from 279/324 to 279/325 while the 43 system stems and allocator
remain unchanged. The continuation records x3, x6, x7, and x1 LEFT-then-RIGHT
closures in Java order; all eight are idempotent, and the phase-two cursor moves
from 3 to 4.

The unchanged generic retry operation then exhausts every remaining phase-two
entry. Exact per-system retry/cursor/queue/vertex/edge tuples are
`(1,2,2,2,161,172)`, `(2,9,9,9,218,247)`, `(3,11,11,11,279,325)`,
`(4,5,5,5,261,299)`, and `(5,3,3,3,264,315)`. Generic `finalizeStems`
checks 45/83/106/93/102 heads respectively with zero removed relations and
zero abnormal-value changes. The transactional `recognize_native_stems` entry
point reproduces the same prepared components and finalized systems, completing
Carmen across all five systems.

The three-row-plus-summary fixture is 6 lines / 3,680 bytes and is
byte-identical across warmup plus two fresh runs. Runner/retarget-transform/
fixture/body/semantic SHA-256 values are
`667310b7936cc9341aac3e145d19328f43e7777e85fef6cb0480dbe2e4c86c4b`,
`29f9b38aba7393883d1b7ff5aff6035e7fc1d0397d001ed5ded0fe8c64d29774`,
`448af58ab47cbfea66a8cee14f95fb376ebd668692e36afd242e7af4f5cbaad8`,
`a3d2e45a4f4fce8f4d98047fb1ac914b36c94215cb6180eda35b9f8462a6372f`,
and
`a3d2e45a4f4fce8f4d98047fb1ac914b36c94215cb6180eda35b9f8462a6372f`.
The runner strictly pins Boundary 190's runner/fixture at
`e0bf5408f12c652e530990c35bce21ca3ec64bd610d02139919198133dccb4f8` /
`f9656d9bb2a917fbd059c58c0692803d8d8fd3c714ed95d3ac981d9e3604c8e0`
and its x1 transform at
`a452fbc760da01105bcd445af2461a6d0fcc7dbfad35fe31ff66d41fc7b2b79e`.

Focused 1/1 and full sibling 25/25 (151.07s) pass. Formatting, strict
all-target/all-feature workspace Clippy (15.57s), oracle shell/AWK syntax,
and diff checks pass. `425d58e82` remains the exact fully green remote CI
baseline. The next work is the first fail-closed STEMS frontier among the
remaining Cucaracha, Hove, and BachInvention5 pages.

## Boundary 192: Cucaracha rejected-stem no-link

Java's `CLinker.link` returns false when expansion selects glyphs and an
acceptable HeadStem relation but `StemBuilder.createStem` returns null. The
generic native C-link loop now maps exactly that mutation-free `Rejected`
result to its existing no-link path. A rejected create transaction that
registers or reinserts a glyph still fails closed pending separate evidence.
Page-drive errors also identify the system, queue index, x ordinal, SIG
ordinal, and selected corner.

Cucaracha system 2 phase-1 order 56 is x56/SIG78/Java Inter 1388. LEFT has no
candidate and RIGHT/BOTTOM selects active Java glyph 1838 at
`1100:1221:1:15`; its HeadStem relation is grade 1.0 with zero dx, but the
stem checker returns grade zero and Java creates no Stem. Java returns false,
adds no vertex, edge, system stem, or glyph, closes the current head's LEFT and
RIGHT S cells, and advances to order 57 x132/SIG84/Java Inter 1400. Native
repeats the same decision over its four allowed stem profiles, commits no SIG,
registry, allocator, or system-stem mutation, records the two ordered S-cell
closures, queues x56 for phase two, and advances identically. The page driver
now exhausts phase 1 on all three Cucaracha systems and fails closed next at
system 1 phase-2 queue 6 x25/SIG71's real append.

The four-row-plus-summary fixture is 7 lines / 5,294 bytes and is
byte-identical across warmup plus two fresh runs. Runner/init/fixture/body/
semantic SHA-256 values are
`08eb22aa38c46490765215c7a1a3b45c6528afb1d3db599fb9a38d69226e6340`,
`4a66495632f0e1a650e57e260e15c7a6f68370fbbaf4bf900b27aa643a2f26e0`,
`51d9d82641a79a98bc1523cc61237bce3994fa2ba9622710ad009aeb0862a73b`,
`34cf5cfb88b5490946f90263dd5adc7cfecc66c0ac9003db18a45ea4fcd65421`,
and
`9c95af3a280b519f93661f9742c0e13910a6551156a40ef7ba967943ccfef341`.
The probe and retained-glyph overlay are pinned at
`1fa259fd5befcb10d71f8010c5d2c049c0322ee1bc2df2bff08d88e25fbf4683` /
`f21487398d9ba162b6459f8f5e1265d56ffc6a8a58e6aa514a03553ee3d05df4`;
the ordered Cucaracha predecessor-fixture set is pinned at
`e365077c7432b03f811987470a1f8c7b9666ffcea8135dd0b28b4e823cef0a1d`.

Focused 1/1 (3.81s) and full sibling 26/26 (150.13s) pass. Formatting,
strict all-target/all-feature workspace Clippy (12.84s), oracle shell syntax,
and diff checks pass. `425d58e82` remains the exact fully green remote CI
baseline; no workflows were yet visible for pushed Boundary 191 commit
`4c25ffe4e`.

## Boundary 193: Cucaracha phase-two LEFT reused-stem append

The shared phase-two transaction now carries the selected horizontal side and
an ordered slice of pre-existing crossed-head relations. Existing Allegretto
and Carmen calls retain RIGHT-origin behavior; Cucaracha system 1 queue 6
authenticates LEFT/BOTTOM while both bottom corners pass `canLink`. LEFT
commits first and RIGHT/BOTTOM subsequently expands to `-1`.

Java queue index 6 is x12/SIG69/Inter1083. It reuses glyph199/Stem2210,
preserves Inter1173's LEFT relation, adds exactly Inter1083's LEFT HeadStem
edge, and keeps vertices/system stems/allocator at 232/38/2216. Native queue
index 6 is x25/SIG71. It resolves glyph43 to Stem identity31/vertex225,
preserves ordered x22/SIG90 edge274 and x32/SIG115 edge275, adds one LEFT
HeadStem edge, and changes no vertex/stem/glyph/allocator state. The carrier
advances to queue 7 x12/SIG69. This authenticates queue-position control and
mutation parity without asserting Java/native x/SIG ordinal identity across
the still-divergent wider HEADS sets.

The 10-line / 5,719-byte fixture is byte-identical across warmup plus two
fresh runs. Runner/transform/fixture/body+semantic hashes are
`0f47ae8f886f5ab28d69ef04c1214a69e16fc22493c59d8a442e44f11b0d8c18`,
`69955a68e2acfada60b7e245dbb9eb636f1beb84d3020682364002179f61ced1`,
`b8f37f279d7361fe92b6cf17c0b9e7376bc744db30e7fc162ce2e9df10669e07`,
and `ec9f27448d849a8fa88bb3ff785818a9229ddc2686f7a700f46b591200211611`.
Boundary 192's runner/fixture are pinned at `08eb22aa…` / `51d9d826…`.
Focused 1/1, full sibling 26/26 (152.74s), final Java replay, formatting,
strict workspace Clippy, oracle shell syntax, and diff checks pass. Continue
at Cucaracha system 1 phase-two queue 7 x12/SIG69.

## Boundary 194: Cucaracha consecutive LEFT shared-stem append

Cucaracha system 1 phase-two queue 7 now uses the same generic LEFT-origin
shared-stem transaction as queue 6. Native x12/SIG69 has grade bits
`0x3fe1a49132208b3d`; both bottom corners pass, LEFT/BOTTOM commits first, and
active glyph41 resolves to Stem identity32 / vertex226. The existing
x18/SIG113 LEFT relation remains edge278, exactly one new HeadStem edge is
appended, and vertices, system stems, glyph IDs, and allocator state remain
unchanged. The carrier advances atomically to queue 8 x52/SIG75.

Java's independently measured queue-index-7 counterpart is
x52/SIG75/Inter1095. Its glyph202 resolves to Stem2205, crossed Inter1185's
LEFT relation is preserved, and edges change 338→339 with vertices232,
system stems38, and allocator2216 unchanged. As in Boundary 193, this is
queue-position/control/mutation parity; the distinct Java/native head and SIG
ordinals remain explicit.

The strict seven-row-plus-summary fixture is 10 lines / 5,921 bytes and is
byte-identical across warmup plus two fresh runs. Runner/retarget-transform/
fixture/body+semantic hashes are `a816aec9285f4a08de6f14eafc961ca073597f355a169a877e71b388dfcfe004`,
`009d2479d330f754c5603f0051ea40631ded4a0752798910aa2bea78707bfcd0`,
`8c6871cddfbb751f341cab49d075ed1c73008ac5119dfd5183dc80a61e363333`,
and `ec71cbcb857514b8751d0bfa8f93e271116a01a6131f7102739905d9c5ecb34a`.
Boundary 193's runner/fixture/transform are strictly pinned at `0f47ae8f…` /
`b8f37f27…` / `69955a68…`.

Focused 1/1 and full sibling 26/26 (151.67s) pass. Formatting, strict
all-target/all-feature workspace Clippy (13.25s), deterministic Java replay,
oracle shell syntax, and diff checks pass. `425d58e82` remains the documented
exact green remote baseline pending newer terminal CI. Continue at Cucaracha
system 1 phase-two queue 8 x52/SIG75's real append.

## Boundary 195: shifted x52 append and prelinked no-op

Rust's wider HEADS carrier contains one extra earlier phase-two entry, so
native queue 8 is x52/SIG75: the head Java measured at queue 7. Its exact
grade and relation bits match that Java transaction. LEFT/BOTTOM resolves
native glyph44 to Stem identity27 / vertex221, preserves x59/SIG119 LEFT
edge264, and appends one current-head edge with no vertex, stem, glyph-ID, or
allocator change.

Native queue 9 is x119/SIG110 and is already linked/closed on LEFT. The
generic retry skips LEFT, finds neither RIGHT corner linkable, returns true,
and performs Java's ordered closure traversal. It records seven neighboring
heads (x122, x124, x126, x121, x118, x123, x125), each LEFT then RIGHT, but
all flags are already true, so 14 writes produce zero value changes and no
graph mutation. The carrier advances through both entries to queue 10
x42/SIG73 without a special no-op wrapper.

Java queue index 8 independently confirms the x119/SIG110/Inter1166 no-op:
LEFT `SkipAlreadyLinked`, RIGHT `Neither`, returned true, no side changes,
and vertices232 / edges339 / stems38 / allocator2216 unchanged. Boundary
194's queue-7 fixture separately authenticates x52's actual C-link geometry.

The strict two-row-plus-summary fixture is 5 lines / 2,887 bytes and is
byte-identical across warmup plus two fresh runs. Runner/retarget-transform/
fixture/body+semantic hashes are `e1fcae89507e31a8f5d43d2c0338e0f8ac3589c282fe02050c404e8248f71080`,
`5722bbdc0861b87f04505aab5d08eed64add7cf3ff54b567a4a5435b2f24de7e`,
`475c4346f01be8331218cdbfb1f335c8df126ea79d9ec883b8006325869b1e3e`,
and `a7aff1b2841a029132f295ac836cd00d4b974b005c2b01a5ffb9afb2caceff6f`.
Boundary 194's runner/fixture/transform are strictly pinned at `a816aec9…` /
`8c6871cd…` / `009d2479…`.

Focused 1/1 and full sibling 26/26 (152.74s) pass. Formatting, strict
all-target/all-feature workspace Clippy (13.00s), deterministic Java replay,
oracle shell syntax, and diff checks pass. `425d58e82` remains the documented
exact green remote baseline pending newer terminal CI. Continue at Cucaracha
system 1 phase-two queue 10 x42/SIG73's real append.

## Boundary 196: identity-aligned x42 append and six prelinked returns

Native queue 10 x42/SIG73 restores direct head/SIG identity alignment with
Java queue 9. LEFT/BOTTOM resolves glyph42 to Stem identity23 / vertex217,
preserves the ordered x39/SIG91 edge257 and x49/SIG117 edge258, and appends
one current-head edge. Vertices, system stems, glyph IDs, and allocator state
remain unchanged. The native relation grade/dx bits are separately pinned;
their last-bit geometry differs from Java rather than being copied from the
oracle.

Queues 11-16 (x133, x58, x125, x138, x48, x17) then use the generic
prelinked path. Each skips an already linked/closed LEFT side, finds neither
RIGHT corner linkable, returns true, records zero closure value changes, and
leaves the graph unchanged. The carrier therefore advances seven entries in
one bounded production slice and fails closed next at queue 17 x68/SIG76.

Java queue 9 measures x42/SIG73/Inter1091 selecting glyph200/Stem2201,
preserving Inter1127 and Inter1181 relations, and changing edges 339→340.
Java queues 10-15 independently confirm the six following mutation-free
prelinked returns. The queue-16 x68 mutation remains excluded for Boundary
197.

The strict 13-row-plus-summary fixture is 16 lines / 9,639 bytes and is
byte-identical across warmup plus two fresh runs. Runner/retarget-transform/
fixture/body+semantic hashes are `ff8c906f2b6f33316f48e21b16a2fcdf0b2cdd8583c4e210b45d6e8c1132fbe6`,
`aa8a4c501a0daf54bf3c09ce0ee202574cdd90673e1b369d5e59d3e5128ed819`,
`614570efcd4a9471ef6692552c9c116b304d24c7171c1e407b0edd5e8710730a`,
and `f88e42d5a3b6b6bcbe100f044f21e0a9a3a44bd6445e7c78acc2297487574cd6`.
Boundary 195's runner/fixture/transform are strictly pinned at `e1fcae89…` /
`475c4346…` / `5722bbdc…`.

Focused 1/1 and full sibling 26/26 (153.09s) pass. Formatting, strict
all-target/all-feature workspace Clippy (13.41s), deterministic Java replay,
oracle shell syntax, and diff checks pass. `425d58e82` remains the documented
exact green remote baseline pending newer terminal CI. Continue at Cucaracha
system 1 phase-two queue 17 x68/SIG76's real append.

## Boundary 197: aligned x68 append and x31 prelinked return

Native queue 17 x68/SIG76 selects LEFT/BOTTOM and resolves glyph40 to Stem
identity30 / vertex224. It preserves ordered x70/SIG105 edge283 and
x74/SIG120 edge284, appends one current-head relation, and changes no vertex,
stem, glyph-ID, or allocator state. Native queue 18 x31/SIG114 then uses the
generic prelinked path and returns true without graph or closure-value change.
The carrier advances to queue 19 x14/SIG58.

Java queue 16 measures the aligned x68/SIG76/Inter1097 transaction through
glyph198/Stem2208, preserving Inter1155 and Inter1187 before edges change
340→341. Java queue 17 independently confirms x31/SIG114/Inter1175's
mutation-free prelinked return. Native relation bits are separately pinned
rather than copied where the spline geometry differs in the low bits.

The strict 8-row-plus-summary fixture is 11 lines / 7,175 bytes and is
byte-identical across warmup plus two fresh runs. Runner/retarget-transform/
fixture/body+semantic hashes are `77a6d85e5323fa62806e9e5ddc3b3a9dcb9a1817a1ae179f9625e175de0e9822`,
`b64a7aaebb60629858847a3cdd7a94d21a967e5f978f03607e7ff6b6747938d2`,
`19b0a62c21cb2fb5dae5f7e923d67b6e0d18433cbef05920aa2eef98cae3fcef`,
and `0296416a0c7732e1729e75aafccbe3a522b74813f4dd0f5ece82cbdfa20d0d4d`.
Boundary 196's runner/fixture/transform are strictly pinned at `ff8c906f…` /
`614570ef…` / `aa8a4c50…`.

Focused 1/1 and full sibling 26/26 (153.38s) pass. Formatting, strict
all-target/all-feature workspace Clippy (13.16s), deterministic Java replay,
oracle shell syntax, and diff checks pass. `425d58e82` remains the documented
exact green remote baseline pending newer terminal CI. Continue at Cucaracha
system 1 phase-two queue 19 x14/SIG58's real append.

## Boundary 198: aligned x14 append

Native queue 19 x14/SIG58 selects LEFT/BOTTOM and resolves glyph41 to Stem
identity32/vertex226. It preserves ordered x8/SIG89 edge319, x13/SIG101
edge320, and x17/SIG112 edge321, appends one current-head edge, and changes no
vertex, system-stem, glyph-ID, or allocator state. Production advances to
queue 20 x45/SIG62.

Java queue 18 independently measures x14/SIG58/Inter1061 through
glyph199/Stem2210, preserving Inter1123, Inter1147, and Inter1171 before edges
change 341→342. The native relation grade/dx bits are separately pinned.

The strict 7-row-plus-summary fixture is 10 lines / 6,704 bytes and is
byte-identical across warmup plus two fresh runs. Runner/retarget-transform/
fixture/body+semantic hashes are
`eb79eb1de1d4570e4f7b976006c6d14134aa6bf32fbe1de156c24bd7972762ec`,
`06095681e521b777c988acb90a562ac2941c9e8ef335fea00b952443aba4c08f`,
`8363a188fdf9d3f32b2bea7545f44c6025cb9228aa1c7c2935023e865d1e232d`,
and `8c7933fa714d698c0dab4bb11b21faf5f3684e24b58159cf924fe1ae82e5ada1`.
Boundary 197's runner/fixture/transform are strictly pinned at `77a6d85e…` /
`19b0a62c…` / `b64a7aae…`.

Focused 1/1 (3.79s) and full sibling 26/26 (151.75s) pass. Formatting,
strict all-target/all-feature workspace Clippy (13.29s), deterministic Java
replay, oracle shell syntax, and diff checks pass. `425d58e82` remains the
documented exact green remote baseline pending newer terminal CI. Continue at
Cucaracha system 1 phase-two queue 20 x45/SIG62's real append.

## Boundary 199: aligned x45 append and x56 prelinked return

Native queue 20 x45/SIG62 selects LEFT/BOTTOM and resolves glyph42 to Stem
identity23 / vertex217. It preserves x43/SIG103 edge323 and x48/SIG116
edge324, appends one current-head relation, and changes no vertex,
system-stem, glyph-ID, or allocator state. The native relation grade/dx bits
are `3fe6918be20e8fdc` / `3fba18036d0d0f3d`. Queue 21 x56/SIG82 then
uses the generic prelinked path, returns true, and changes no graph or closure
value. Production advances to the next fail-closed frontier at queue 22
x71/SIG66.

Java queue index19 independently measures x45/SIG62/Inter1069 selecting
glyph200/Stem2201, preserving Inter1151 and Inter1179, and changing only edges
342→343. Its independently computed relation bits are
`3fe6918be20e8d71` / `3fba18036d0d1555`. Java queue20 confirms
x56/SIG82/Inter1109's mutation-free prelinked return.

The strict 8-row-plus-summary fixture is 11 lines / 6,787 bytes and is
byte-identical across warmup plus two fresh runs. Runner/retarget-transform/
fixture/body+semantic SHA-256 values are
`29733c6d93a1d5642d24cfe742b9d3f9314230818ca5919acd1a5b21552e74a7`,
`4b3029fec45ef99cdd24804ec7e88ac04578a62f2e1e71e127088ee5554c56ba`,
`59f27d582bda0a3a144a68b5dc37a0ac586ad89de19c64d993aed15cfdbed2c4`,
and
`37c5c7ddd68f9a923e132ccc62fe834a720006e106143ad97b4260c63e3cb791`.
Boundary 198's runner/fixture/transform are pinned at `eb79eb1d…` /
`8363a188…` / `06095681…`.

Focused 1/1 (3.83s) and full sibling 26/26 (150.63s) pass. Formatting,
strict all-target/all-feature workspace Clippy (13.35s), deterministic Java
replay, oracle shell syntax, and diff checks pass. `425d58e82` remains the
documented exact green remote CI baseline pending newer terminal evidence.
Continue at Cucaracha system 1 phase-two queue 22 x71/SIG66's real append.

## Boundary 200: Cucaracha system-one phase-two completion

Native queue22 x71/SIG66 selects LEFT/BOTTOM, resolves glyph40 to existing
Stem identity30 / vertex224, preserves x70/SIG105 edge283 and x74/SIG120
edge284, and appends exactly one current-head relation. Its independently
computed relation grade/dx bits are `3fe5554e97cdff05` /
`3fbd29be97edf9e8`. No vertex, system-stem, glyph-ID, or allocator state
changes. The continuation returns true and advances the phase-two index
22→23, exactly exhausting Cucaracha system 1's native queue. The page driver
now exposes the next fail-closed frontier at Cucaracha system 2 phase-two
queue8 x56/SIG78's real `reuseStem` append.

Java queue21 independently measures x71/SIG66/Inter1077 selecting
glyph198/Stem2208, retaining Inter1155 and Inter1187, and changing only edges
343→344 while vertices232, system stems38, and allocator2216 remain fixed.
Its independent relation grade/dx bits are `3fe5554e97ce0182` /
`3fbd29be97edf3cf`.

The strict seven-row-plus-summary fixture is 10 lines / 6,284 bytes and is
byte-identical across warmup plus two fresh runs. Runner/transform/fixture/
body+semantic SHA-256 values are
`3ad18d6e2db7b60980a27deef414bf54ac86df1fdfc127b26539172b4665e918`,
`a9daae9d492b63c9b9e091f0522bf7e42d270ef113a6f63f5a323066764c0d01`,
`457f8f28ca9a62fd085b27d5e574b1ff71a9f2f211dec9a0a82d4c30432c20d5`,
and `5ce49912b802895b8c9c549ef8b08c92c08f6a8942b6d0bd02f8c3f4a2d12f94`.
Boundary 199's runner/fixture/transform are strictly pinned at `29733c6d…` /
`59f27d58…` / `4b3029fe…`.

Focused 1/1 and full sibling 26/26 (153.52s) pass. Formatting, strict
all-target/all-feature workspace Clippy (13.32s), deterministic Java replay,
oracle syntax, and diff checks pass. `425d58e82` remains the documented exact
green remote baseline pending newer terminal CI. Continue at Cucaracha system
2 phase-two queue8 x56/SIG78's real append.

## Boundary 201: Cucaracha system-two phase-two queue 8

Native queue8 x56/SIG78 selects LEFT/BOTTOM after both bottom corners pass,
resolves glyph92 to existing Stem identity30 / vertex242, preserves
x67/SIG119 edge261, and appends exactly one current-head relation. The later
RIGHT/BOTTOM expansion returns `-1`. Native relation grade/dx bits are
`3feb7adfb837fb8d` / `bfbae2955082830c`, exactly matching Java. No vertex,
system-stem, glyph-ID, or allocator state changes. The continuation returns
true and advances system 2's phase-two index8→9; production next fails closed
at queue9 x132/SIG84's real `reuseStem` append.

Java queue8 independently measures x56/SIG78/Inter1388 selecting glyphs250
and 2487, canonical candidate250, and existing Stem2647. It retains
Inter1471, changes only edges347→348, and keeps vertices255, system stems43,
and allocator2659 fixed.

The strict eight-row-plus-summary fixture is 11 lines / 6,012 bytes and is
byte-identical across warmup plus two fresh runs. Runner/transform/fixture/
body+semantic SHA-256 values are
`e862cb9e24ca33a0f9381b1990b25a3a59c607337b60720930871b93936e5b7d`,
`3f696415a4450338b60c29d343aaccd7ba88772868abaf2deac3ea1c46272cbf`,
`5290a3261024d312098f1671c536df2bf2e89721e9b6713574c25d95107a58b5`,
and `71543efca6a7a47a0d0ba1339273402d0b2495f6f0c6ac88fce86716d2a9bef7`.
Boundary 200's runner/fixture/transform are strictly pinned at `3ad18d6e…` /
`457f8f28…` / `a9daae9d…`.

Focused 1/1 (3.83s) and full sibling 26/26 (152.25s) pass. Formatting,
strict all-target/all-feature workspace Clippy (13.52s), deterministic Java
replay, oracle syntax, and diff checks pass. `425d58e82` remains the documented
exact green remote baseline pending newer terminal CI. Continue at Cucaracha
system 2 phase-two queue9 x132/SIG84's real append.

## Boundary 202: Cucaracha system-two phase-two queue 9

Native queue9 x132/SIG84 selects LEFT/BOTTOM, resolves glyph93 to existing
Stem identity35 / vertex247, preserves x129/SIG103 edge268 and x139/SIG125
edge269, and appends one current-head relation. Relation grade/dx bits are
`3fed051e7bce623f` / `bfb22f195fe0a492`, exactly matching Java. No vertex,
system-stem, glyph-ID, or allocator state changes. The continuation returns
true, advances index9→10, and exposes queue10 x84/SIG80's real `reuseStem`
append.

Java queue9 independently measures x132/SIG84/Inter1400 selecting glyph251
and existing Stem2652, retaining Inter1438 and Inter1483, and changing only
edges348→349 while vertices255, system stems43, and allocator2659 stay fixed.

The strict seven-row-plus-summary fixture is 10 lines / 6,314 bytes and is
byte-identical across warmup plus two fresh runs. Runner/transform/fixture/
body+semantic SHA-256 values are
`d1e2a3dd39c1f2f73b8ffc7d907e5361f33bbbd57a7dbf3ad68e3cc11ae0973c`,
`af763c75140add0f67a9ccb3b077797fdf7c640c5b80a122697de63f5beeb0a2`,
`e7d97fbf829b52730dfdf4f219a0a7fd87cde3a8f7f8f301c788746492529f01`,
and `fcfef4137dad57cfd43d5c6c48bf71497cf78094f57169242449241cde725e4f`.
Boundary 201's runner/fixture/transform are strictly pinned at `e862cb9e…` /
`5290a326…` / `3f696415…`.

Focused 1/1 (3.80s), full sibling 26/26 (153.99s), formatting, strict
all-target/all-feature workspace Clippy (13.68s), deterministic Java replay,
oracle syntax, and diff checks pass. `425d58e82` remains the documented exact
green remote baseline. Continue at Cucaracha system 2 phase-two queue10
x84/SIG80's real append.

## Boundary 203: Cucaracha system-two phase-two queue 10

Native queue10 x84/SIG80 selects LEFT/BOTTOM, resolves glyph94 to existing
Stem identity29 / vertex241, preserves x93/SIG121 edge258, and appends one
current-head relation. The later RIGHT/BOTTOM expansion returns `-1`.
Relation grade/dx bits `3feb7b1081c1abf7` / `bfbae1892d23b6db` exactly
match Java. No vertex, system-stem, glyph-ID, or allocator mutation occurs.
The continuation advances index10→11; generic queues11-15 then run unchanged,
exposing queue16 x109/SIG81's real `reuseStem` append.

Java queue10 independently measures x84/SIG80/Inter1392 selecting glyph252
and existing Stem2646, retaining Inter1475, and changing only edges349→350
while vertices255, system stems43, and allocator2659 stay fixed.

The strict eight-row-plus-summary fixture is 11 lines / 5,993 bytes and is
byte-identical across warmup plus two fresh runs. Runner/transform/fixture/
body+semantic SHA-256 values are
`8b260716910454740347bf55952f5a31ece6f089528e59871947f6611a096160`,
`3d076bd7c6ff7e43145545af6969a36b2c415ac4067a317c6f169735c28639e0`,
`cb394f3b37eade0450ba44bc44ecb3db96d52e415745fd73e0576f3a7aa6cf06`,
and `8448857c730bea286818298f9a883235fd7073b7114d01bfa4fb930aa4053fef`.
Boundary 202's runner/fixture/transform are pinned at `d1e2a3dd…` /
`e7d97fbf…` / `af763c75…`.

Focused 1/1 (3.73s), full sibling 26/26 (152.87s), formatting, strict
all-target/all-feature workspace Clippy (13.63s), deterministic Java replay,
oracle syntax, and diff checks pass. `425d58e82` remains the exact green remote
baseline. Continue at Cucaracha system 2 phase-two queue16 x109/SIG81.

## Boundary 204: Cucaracha system-two phase-two queue 16

Native queue16 x109/SIG81 selects LEFT/BOTTOM, resolves glyph95 to existing
Stem identity37 / vertex249, preserves x111/SIG110 edge282 and x114/SIG122
edge283, and appends one current-head relation. Relation grade/dx bits
`3fef148d14458919` / `bf9734df7f4c3cf4` exactly match Java. No vertex,
system-stem, glyph-ID, or allocator mutation occurs. The continuation advances
index16→17; generic queues17-23 complete system 2 and generic system-3 queues
0-18 expose queue19 x37/SIG11's real `reuseStem` append.

Java queue16 independently measures x109/SIG81/Inter1394 selecting glyphs253
and 2575, candidate253, and existing Stem2654. It retains Inter1453 and
Inter1477, changes only edges350→351, and keeps vertices255, system stems43,
and allocator2659 fixed.

The strict seven-row-plus-summary fixture is 10 lines / 6,358 bytes and is
byte-identical across warmup plus two fresh runs. Runner/transform/fixture/
body+semantic SHA-256 values are
`0307f76f0da438d3609c1dcaa602656eca732de9fd377bd25325e94c78ffea77`,
`bc9205d1e88c653d7d7cb553cc525d559a69e87b4736efe615c975daf82ae425`,
`200afe8ef54faf6a11ecf094bc2394b485dee7f0eb6ed68aa632e4e4bdbbdd5d`,
and `77964df581176281c035325c64ddacb5d73abe745f687134be5291e25062c6ef`.
Boundary 203's runner/fixture/transform remain pinned at `8b260716…` /
`cb394f3b…` / `3d076bd7…`.

Focused 1/1 (3.77s), full sibling 26/26 (153.57s), formatting, strict
all-target/all-feature workspace Clippy (13.88s), deterministic Java replay,
oracle syntax, and diff checks pass. `425d58e82` remains the exact green remote
baseline. Continue at Cucaracha system 3 phase-two queue19 x37/SIG11.

## Boundary 205: Cucaracha system-three phase-two completion

Native terminal queue19 x37/SIG11 selects LEFT/BOTTOM, resolves glyph159 to
existing Stem identity13/vertex177, preserves x32/SIG49 edge207, and appends
one current-head relation. Exact grade/dx bits `3fe4e1c61700dadc` /
`3fbe433d3ee06618` match Java. Edges advance 250→251; vertices198, stems34,
glyph identities, and allocator3009 remain unchanged. The cursor advances
19→20 and exhausts system 3.

Java independently measures Inter1555 selecting glyphs317+2868/candidate317,
existing Stem2989, and retained Inter1632. The 10-line / 5,826-byte strict
fixture contains seven semantic rows plus summary and is byte-identical across
warmup plus two fresh runs. Runner/transform/fixture/body+semantic hashes are
`26af234811b815d1e2012311838045cd80adec4c3d67c3dd19c732160600fb34`,
`35f69316834081b0e6f8354e0bfbb856952930941652ccd04db2ee23dcc1d432`,
`a4ede84ed937da65006924da3b3de35e24d33dd229d9391aae136e436b1477ff`,
and `81451bfd11189860d64e970ab4a81714b1a3ff7cfddfac1ef8c10f1e6f5fe74c`.

All three Cucaracha phase-two queues now exhaust. Generic `finalizeStems`
checks 142/150/113 heads with no relation removals or abnormal changes, and
transactional `recognize_native_stems` reproduces the same page. Focused 1/1,
full sibling 26/26 (154.39s), formatting, strict workspace Clippy (13.75s),
deterministic Java replay, oracle syntax, and diff checks pass. Continue with
the first unsupported transactional frontier among Hove and BachInvention5.

## Boundary 206: Hove system-five phase-two completion

Hove system 5's terminal queue 1 at x67/SIG52 selects RIGHT/TOP and resolves
native glyph226 to existing Stem identity25 / vertex128. It preserves the
x65/SIG46 relation at edge143 and appends only the current-head relation at
edge159. Vertices136, system stems32, glyph identities, and allocator2937 stay
fixed while edges advance 159→160 and the queue cursor advances 1→2.

Java independently measures Inter1721 selecting glyph284 and existing
Stem2931, with Inter1709 as the retained source. The five-row strict body is
byte-identical across warmup plus two fresh runs. Runner/transform/fixture/body
hashes are
`e4af37df9ef194bf2da94d05101f452384144dd5ffbe5856f35fe5aebb179547`,
`2f54cd2e91e0d930912e7decc1d7222512918b0a14103010e9fa2dee05762eeb`,
`b3b6f9f88e158793eec8072c2f8aee1ebb9508acf5b908965651015c4d10d341`,
and `0078c65201a8b8b426beaf4cee7ad67928fb1b5252e15b46108b2b5486753e71`.
Boundary 205's runner/fixture/transform remain pinned at
`26af234811b815d1e2012311838045cd80adec4c3d67c3dd19c732160600fb34`,
`a4ede84ed937da65006924da3b3de35e24d33dd229d9391aae136e436b1477ff`,
and `35f69316834081b0e6f8354e0bfbb856952930941652ccd04db2ee23dcc1d432`.

All five Hove phase-two queues now exhaust. Generic `finalizeStems` checks
65/90/52/65/71 heads with no relation removals or abnormal changes, and the
production `-step STEMS -json` plus transactional `recognize_native_stems`
complete the page. Focused 1/1 (3.86s), sibling 27/27 (156.52s), formatting,
strict all-target/all-feature Clippy (13.45s), deterministic Java replay,
oracle syntax, and diff checks pass. Continue at BachInvention5 system 6's
missing carried BEAMS groups.

## Boundary 207: preserve pre-rest beam-group identity into native SIG

Java creates `BeamGroupInter` containment inside `BeamsBuilder.buildBeams()`
and only afterward runs `MultipleRestsBuilder`. Removing the rest-like beam
therefore deletes its vertex and incident relations without regrouping the
remaining beams. Native BEAMS already retained that pre-rest group evidence;
native SIG incorrectly compared it with a fresh grouping of the compact
post-rest stream.

`append_beams` now replays the pre-rest grouping events and maps their source
identities to live post-rest vertices. It drops only the retired member and its
incident exclusion/containment/BeamBeam relations, preserves each surviving
group, and removes a group vertex only when its sole member was retired. In
Bach system 6, raw ordinal182 is local member23 of group `[18,23]`; member18
remains in that group rather than entering the different geometric partition
produced by regrouping the compact list.

The focused MultipleRest test pins the lifecycle and the distinct pre/post
partitions. Existing Java-backed HEADS competitor, native-SIG, reachability,
stump, and V-linker corpora pass unchanged, including the two Bach stump rows
with `groupMembers 2`. Production Bach now passes SIG assembly and reaches the
real system-1 higher-profile retry for a rather-good unlinked head. Focused
MultipleRest 1/1, competitors 2/2, small-beam epilog 6/6, STEM_SEEDS/BEAMS
4/4, native SIG 10/10, all three downstream corpus gates, formatting, strict
workspace Clippy (11.88s), and diff checks pass. No oracle changed. Continue
at that Bach system-1 retry frontier.

## Boundary 208: generic phase-one rather-good profile retry

Java's phase-one `HeadLinker.linkSides` recursively retries a rather-good
unlinked head at stem profiles 1 through 3 after STRICT profile 0. The native
continuation now does the same: it preserves the linked/closed and shared-stump
undefined branches, accumulates the ordered LEFT/RIGHT decisions from every
profile, carries a later-profile C-link frontier when one appears, and only
closes both local S cells after all four profiles fail.

The full-lifecycle BachInvention5 system-1 oracle authenticates queue37
x3/SIG95/Inter3599, grade bits `3fdcd6c4146e1fa4`. LEFT and RIGHT are
`Neither` at profiles 0, 1, 2, and 3; Java then returns false, leaves the
undefined-head list empty, closes x3 LEFT then RIGHT, and advances to queue38
x44/SIG36/Inter3481 without changing 216 SIG vertices, 257 edges, 37 system
stems, or the allocator. The native production carrier reproduces that result
from live Bach GRID through HEADS state.

The deterministic fixture SHA-256 is
`2964eb04060e03a97db6c44cd8de3cc383a59a082b9f56524290c3181aacafaa`;
runner, probe, init script, and emitted-body hashes are
`8edea3da64b607b16ccf5a30191d6c14429c3106b9aa8e263e4f6ea24e913d61`,
`f71177c81db91fb46ec392f53f854dbc37ceb05dd4e50ad3d3ef315d2d380772`,
`a2b5123237974823bf131d3e17ef8c27035062c00e9bfe15aeb9b17ce8a324df`,
and `8efab31e3192446991f12e3e2587ad565f8a7c5b30d194e626ec10b7a019e51c`.
Warmup plus two fresh JVM runs are
byte-identical. Focused 1/1, sibling 28/28 (156.94s), formatting, strict
workspace Clippy (14.08s), oracle replay, and diff checks pass. Bach system 1
now completes; the next fail-closed frontier is system 2 queue182 x138/SIG149,
whose STRICT-profile LEFT/BOTTOM builder contains the start head plus two
concrete BeamLinker stump items.

## Boundary 209: concrete multi-beam stump reuse

The generic C-link transaction now expands concrete BeamLinker stump items.
It resolves seed-backed stumps from carried native seed glyphs and built stumps
from unique pre-builder registry events, updates the evolving line before each
BeamStem relation check, and authenticates a unique already-present linked/open
BeamStem edge instead of appending a duplicate.

Bach system 2 queue182 is x138/SIG149/Inter3906. STRICT profile 0 selects
LEFT/BOTTOM; the start head and beam SIG27/B3 plus SIG31/B3 all carry the same
1258:902:4:51 glyph. Both BeamStem relations already exist at grade 1 and
`CENTER`. Java and native reuse the existing Stem, append only the x138
HeadStem edge, preserve vertices/stems/allocator, close x140/SIG141 LEFT then
RIGHT, and advance to queue183 x62/SIG99/Inter3804.

Fixture/runner/probe/init/body SHA-256 values are
`7b84be8e57253846336ad1463745b998ecf97e3b55b20ec3dbefbd5ce790f760`,
`b1e40651458dec4914e89b53fadbb1ac9406cdea4dd988af27c9df8cd869b817`,
`72e85d0de1838664db221fa890917b83a1140bf6ee5ea99b0a1f6bc1839fec33`,
`3140eec01b976a5cf934183c37ef07528bacc874abe67a0491f409505daf888b`,
and `79c38429801cea5f11a2c9c5a241aba636603500b946c0dd6d9cc84b20625dad`.
Warmup plus two fresh JVM runs are byte-identical. Focused 1/1, sibling 29/29
(157.13s), formatting, strict workspace Clippy (13.62s), replay, and diff
checks pass. Continue at system 2 queue183 x62/SIG99.

## Boundary 210: identity-free four-head prelinked reconciliation

Bach system 2 queue183 is x62/SIG99/Inter3804 at grade bits
`3fc88ee23b88ee24`. LEFT is already linked and RIGHT already closed at every
profile 0-3. Its one incident pre-existing stem structurally joins x59/SIG113,
x62/SIG99, x63/SIG100, and x65/SIG196 on their LEFT sides. Java returns true,
records no undefined side or state change, and preserves 394 SIG vertices, 593
edges, 77 system stems, and the allocator. The generic native continuation
replays x59, x63, and x65 LEFT then RIGHT in that order; all six writes are
idempotent, so `closed_value_changes=0`, and the carrier advances to queue184
x25/SIG93/Inter3790.

The new oracle deliberately omits Java StemInter and Inter IDs from its semantic
rows. Fixture/runner/probe/init/body SHA-256 values are
`079e8b4995e8610c5eda9370624d93a3e9262f15e2cb5eebf4f2159250974f75`,
`ac697b86954010c94de4e7767e12d6e80bd79306a0f6f3e8d8c80fa733cda5fe`,
`05c2ff1c14f4f2284ffb80560c82fce4b66c5d41f8debc21e2f5d91fe910a7bb`,
`c799ce83ebcffad237d9037f63bfe0b1f092798e54142ed25c75b263af1074d3`,
and `1bae18ca1122bb13623be12eaec05a64720233c156dd8a4ff09b8c519750e793`;
the runner also pins Boundary 209's runner and fixture. Warmup plus two fresh
JVM runs are byte-identical. Focused 1/1, sibling 29/29 (156.79s), formatting,
strict workspace Clippy (9.44s), replay, and diff checks pass. No production
source change was needed. Continue at system 2 queue184 x25/SIG93.

## Boundary 211: transformed four-head mixed-change reconciliation

Bach system 2 queue184 is x25/SIG93/Inter3790 at grade bits
`3fc87c4777a649dd`. LEFT is already linked and RIGHT already closed across
profiles 0-3. The incident stem joins x25/SIG93, x27/SIG178, x28/SIG179, and
x29/SIG92 on LEFT. Java returns true and changes only x28 LEFT and RIGHT from
open to closed. Native preserves Java's incident relation order by emitting
x29, x27, then x28 LEFT/RIGHT; the first four writes are idempotent and the
last two change values. SIG 394/593, 77 stems, and the allocator remain
unchanged before queue185 x192/SIG76/Inter3757.

The queue-184 probe is a checked transform of Boundary 210's identity-free
source. Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`7a77078895e488d1be44d0f57c272d0d022fc278c86ba13d94925f8ff111aebe`,
`16c64b513e86df490b141cfa6189d3f80ac76c18ea483ae1d4d81325a2a3b805`,
`64514c7fc90e30ee745f02628a9a44461d175477ba93c8a80bb158fdb9d499e3`,
`3787d760a4a9f6fadd552910ff4876a38990d59625e9fa405c453bf6b918350e`,
`66f7873e1eaaef9ff5504ec23e561eb1c015fc5756f36c0220c69f590127e648`,
and `8ef60ed510ea962fde3199051794cdcdaae5d12c3d59ac367fe2bfef65696a74`.
Boundary 210's source, runner, and fixture are strict predecessor pins. Warmup
plus two fresh JVM runs are byte-identical. Focused 1/1, sibling 29/29
(153.12s), formatting, strict workspace Clippy (8.87s), replay, and diff checks
pass. No production source change was needed. Continue at system 2 queue185
x192/SIG76.

## Boundary 212: transformed three-head zero-change reconciliation

Bach system 2 queue185 x192/SIG76/Inter3757 has grade bits
`3fc861861861861a`. LEFT is already linked and RIGHT closed across profiles
0-3. The pre-existing stem joins x191/SIG75, x192/SIG76, and x193/SIG77 on
LEFT. Java returns true with no undefined side or state change. Native emits
x191 then x193 LEFT/RIGHT in relation order, all four writes are idempotent,
and `closed_value_changes=0`. SIG 394/593, 77 stems, and the allocator remain
unchanged before queue186 x190/SIG214/Inter4036.

The queue-185 probe is another checked transform of Boundary 210's
identity-free source, with Boundary 211 pinned as its immediate predecessor.
Fixture/runner/transform/transformed-probe/init/body hashes are
`bba0a8a3a80a6bb1d5693fb3cdb6a1764e798e9c3ca34000a08b78a8f2b386b7`,
`5d15aa20ae4a7282b059dd3d6cd556c248be8b9f532739d66c5ad2b57cfe8c09`,
`61bd7b3e2aff7418a034cff7b70453dd1db180d59ee3731f07b5f60044798dc7`,
`a8b102ab3485a79d5def994540b6401d3a6bdbffa946f13f3ff52514cd050057`,
`568926dc325d8e9633ec3df663466df5ca14109725a35ef9ca5060e988069d13`,
and `9d3ea66878524b64a58a764370915fc0ae64de4ca171a25ec33952e6489b9834`.
Warmup plus two fresh JVM runs are byte-identical. Focused 1/1, sibling 29/29
(148.07s), formatting, strict workspace Clippy (8.51s), replay, and diff checks
pass. No production source changed. Continue at system 2 queue186 x190/SIG214.

## Boundary 213: identity-free no-link closure

Bach system 2 queue186 x190/SIG214/Inter4036 has grade bits
`3fc857b6c55b3c0d`. LEFT and RIGHT are both `Neither` across Java profiles
0-3. Java returns false, records no undefined or incident stem, and closes the
current head's LEFT and RIGHT cells. Native evaluates the one operational
profile for this grade and reproduces those two ordered writes with exactly two
value changes. SIG 394/593, 77 stems, and the allocator remain unchanged before
queue187 x178/SIG52/Inter3709.

The queue-186 probe is another checked transform of Boundary 210's
identity-free source and pins Boundary 212 as its predecessor. Fixture/runner/
transform/transformed-probe/init/body hashes are
`729145d6ecd237c7cf420323f980384e119efac24eed97a2393bc1a91dbba8b9`,
`38b6854c8a1a58cc4e463f119bf60317a5fc4501cc22bd21c091850e3cb9558a`,
`ab01e72ce28d279aa95fa66d5c0e0f86533e8d9f8ba058fcfa9a20ea3e1b9dc0`,
`f0c4689aeee121c8e74e565fa92c40ab38827197a986a02c44080503757177ac`,
`4b36fba6bab07e37401f56e1652f6d97b38aff7ce99ababab60ff874388c673d`,
and `e5b83dc66a534e93fb5774e6b74adea3954a8dd81c03e4e89f5d4db3fcc34eff`.
Warmup plus two fresh JVM runs are byte-identical. Focused 1/1, sibling 29/29
(153.63s), formatting, strict workspace Clippy (9.21s), replay, and diff checks
pass. No production source changed. Continue at system 2 queue187 x178/SIG52.

## Boundary 214: identity-free existing-stem multi-beam C-link

Bach system 2 queue187 x178/SIG52/Inter3709 selects LEFT/BOTTOM at STRICT
profile. Its three-item builder contains the head and beam SIG ordinals 11 and
14 at B-linker 3. All items select active glyph535 with structural content
`1565:761:4:51` and reuse its existing stem. Native preserves both BeamStem
edges, adds only the x178 HeadStem edge at exact grade 1.0 and negative-zero
`dx`, closes x181/SIG42 LEFT/RIGHT, and advances to queue188 x47/SIG57/Inter3719.
The graph changes 394/593 to 394/594; 77 stems and the allocator stay fixed.

The queue-187 probe is transformed from the frozen multi-beam source and pins
Boundary 213. Fixture/runner/transform/transformed-probe/init/body hashes are
`62acbdbea32f228e829d9b49cec8b795308ab77307aea358091e446daf8820c8`,
`b5f3635b1c364ead19243eb9c25d5388e558ee0ee268e54c63dc7a3c69111fad`,
`5d32102a183990baaa8324575019e8f3e687293da60355e5e4c321462542051f`,
`efcb665ce63d49bc2a3e3c9587e2cedaf65076d2fee2746cbe2d8ee22de6fade`,
`3a83d63f8191f6e9ab734c60793095fe1b8ff85d9580ea934cc7ed7bf1d5a4a2`,
and `95854d88aace78876d736d5352b62f25e8d730c27d7994e36bdea8fffaf0b9de`.
Warmup plus two fresh JVM runs are byte-identical. Focused 1/1, sibling 29/29
(156.33s), formatting, strict workspace Clippy (10.29s), replay, and diff checks
pass. No production source changed. Continue at system 2 queue188 x47/SIG57.

## Boundary 215: exact two-head existing-stem line rounding

Bach queue188 x47/SIG57/Inter3719 selects LEFT/TOP at STRICT profile. Its
two-item builder carries x47 and crossed head x48/SIG38, selects active glyph485
(`540:722:5:73`), and reuses the existing four-head stem. At this authenticated
corner Java's translated stem-line x coordinates are two ULPs above native, so
the bounded correction applies `java_next_up` twice before crossed-head grading.
The main relation grade/dx bits are `3fe7cb9fff0ca1d8`/`bfc6e39073a980f1`;
the appended x48 relation bits are `3feb43e5758fd513`/`3fab54928678de1e`.

Native adds both HeadStem edges, emits x45, x42, and x48 LEFT/RIGHT closures in
relation order with four value changes, keeps 394 vertices, 77 stems, and the
allocator fixed, moves edges 594 to 596, and advances to queue189 x164/SIG51.
Fixture/runner/transform/transformed-probe/init/body hashes are
`6aa06fe00a0816367a4cc2586f2edfa33580e9f8ec15b5d757ec92bd5f81e69d`,
`29e80ba56b7f613cda7fbddb567545f45c53042d9a77bab2942d75b6e3388778`,
`2e6e5177fa7e14bb7d0c50706f5752cc7bf7ba2e45e8023ebc53f4ca3a6bb466`,
`1f102875149c3b26cbc17d9f8344c33d3d789bd2093722633e7d8c041e8ac7f9`,
`179202a088b6ed50956a1fa55093e59006080cddeeb46b753cca0c6ca340d045`,
and `3a77205b34b8b4eea8fe6da9404fc37c3531fbaf0dda63339c09eb8d303f4f82`.
Warmup plus two JVM runs are byte-identical. Focused 1/1 and sibling 29/29
(149.20s), formatting, strict workspace Clippy, replay, and diff checks pass.
Continue at queue189 x164/SIG51.

## Boundary 216: two-head existing-stem reconciliation

Bach queue189 x164/SIG51/Inter3707 is linked on LEFT and closed on RIGHT across
profiles 0-3. Its existing stem joins x164/SIG51 and x167/SIG40. Java and the
generic native continuation return true, close x167 LEFT/RIGHT with two value
changes, preserve the 394/596 SIG, 77 stems, and allocator, and advance to
queue190 x65/SIG196.

The identity-free queue-183 source is transformed with Boundary 215 strictly
pinned. Fixture/runner/transform/transformed-probe/init/body hashes are
`a3568828c467de8b7390fb8ee005f8115d8bc79ef9914d20031a8ce3596c5428`,
`4c0fc7f45e4954ae46930f4e6101fa3402603b2c6d7bdef100f7a2b53dfc02ca`,
`20102738ce60feb053653420bc0334a196852d73b785473adfbe54abad7901cd`,
`273f19d5bacdc88f58b84e9944692d8bc65532dd5c0d2e63e31738689fd90e1f`,
`f538824fe7ad158cb9d7b2e2832f67a601c5757c68aa89e807450bab0c15ee9d`,
and `e1e8bd18f83b5e14d08c0794f3f46c0605c38748869f5ee5c887d28ac495ff88`.
Warmup plus two runs are byte-identical. Focused 1/1 and sibling 29/29
(158.00s), formatting, strict workspace Clippy, replay, and diff checks pass.
No production source changed.
Continue at queue190 x65/SIG196.

## Boundary 217: four-head zero-change reconciliation

Bach queue190 x65/SIG196/Inter4003 is linked on LEFT and closed on RIGHT at
profiles 0-3. Its existing stem joins x59/SIG113, x62/SIG99, x63/SIG100, and
x65/SIG196. Java changes no cells; native emits the idempotent x59, x62, and
x63 LEFT/RIGHT sequence. The 394/596 SIG, 77 stems, and allocator stay fixed
before queue191 x150/SIG29.

The queue-183 source is transformed with Boundary 216 strictly pinned.
Fixture/runner/transform/transformed-probe/init/body hashes are
`dec4343b21d65a29cd9552bfbf8a106995bad020006e29ae99f4173439838369`,
`a250dfd71a1af3438fb7d9b82b3715596f3a0e72fbb1a8f01435acf9060e94aa`,
`ee84b4d086e517527dc828095c7b5e6d61e640e431557113ed677d2dc329c54c`,
`e0a865650e1d9d1ffba2495dfcb8b5e8c5ac16cc4166fbb92ac7138228495ffb`,
`3ad32220754d5dabfba4ae091a904dfd7da425a9aa34808a7f3b5c2a96084efd`,
and `d8e3b0c151179534a1a686b56c99a9a9bef867dbba426303835e52220d2b2f8c`.
Warmup plus two JVM runs are byte-identical. Focused 1/1 and sibling 29/29
(154.16s), formatting, strict workspace Clippy, replay, and diff checks pass. No production source
changed. Continue at queue191 x150/SIG29.

## Boundary 218: following two-head reconciliation

Bach queue191 x150/SIG29/Inter3663 is linked on LEFT and closed on RIGHT at
profiles 0-3. Its existing stem joins x150/SIG29 and x151/SIG17. Java and
native close x151 LEFT/RIGHT with two changes, preserve the 394/596 SIG, 77
stems, and allocator, and advance to queue192 x173/SIG160.

The queue-183 source is transformed with Boundary 217 strictly pinned.
Fixture/runner/transform/transformed-probe/init/body hashes are
`33fc783ef4d341d2acfc221a08eb079d320a2050e09155094472113478ab2aeb`,
`2409eb033551846e070d1ef90a0ed7a341ce5a36006fd6f1e3a1deb280ec12de`,
`5187abcaff808f969c1cf620435365f17a0112f094fb9bd6097cbb650183ffbf`,
`42f848877e8eb5eb6a6d116a81fc754d587b2ca7fb1a1deda6aa94e22a898fce`,
`c67527aef2cbd6d6ec540202c6b5f0ac798d45f57a585e732befce9714636098`,
and `b97ded46c51880af3500bac1287fafd3977aad072dbf529908b1362da920dd75`.
Warmup plus two runs are byte-identical. Focused 1/1 and sibling 29/29
(156.78s), formatting, strict workspace Clippy, replay, and diff checks pass.
No production source changed.
Continue at queue192 x173/SIG160.

## Boundary 219: right-side zero-change reconciliation

Bach queue192 x173/SIG160/Inter3931 is closed on LEFT and linked on RIGHT at
profiles 0-3. Its existing stem joins x170/SIG165, x171/SIG166, and
x173/SIG160 on RIGHT. Java changes no cells; native emits x170 then x171
LEFT/RIGHT idempotently. The 394/596 SIG, 77 stems, and allocator remain fixed
before queue193 x27/SIG178.

The queue-183 source is transformed with Boundary 218 strictly pinned.
Fixture/runner/transform/transformed-probe/init/body hashes are
`2f16f58f978732969374cd98cf373abf0aafe465b06446fefb346a5c20bec1ea`,
`6bcfa2d04b9cb77e564e1be8e33fd143bac975b1f425c1e5d4bdd60bf1739caf`,
`ebeaa341807699e4d490b90279bcaccbd4bcf48babdb0008773743a0d9e22ef4`,
`946f543331a6a09ef701e834606e1ec5c405296ed69b959c89ed432802c1c484`,
`994a1f57b02b044cd3c224bb39f7038dadeafdc06b46ac7b1cc2f40ba37aeef8`,
and `08e9bfa02f9646c243bee05096843c899f1c7f4ccdcbdadfabbebc33b9dfd12c`.
Warmup plus two runs are byte-identical. Focused 1/1 and sibling 29/29
(152.70s), formatting, strict workspace Clippy, replay, and diff checks pass.
No production source changed.
Continue at queue193 x27/SIG178.

## Boundary 220: repeated four-head zero-change reconciliation

Bach queue193 x27/SIG178/Inter3967 is linked on LEFT and closed on RIGHT. Its
existing four-head stem is already closed; native reproduces Java's idempotent
x29, x25, then x28 LEFT/RIGHT writes without graph or allocator mutation and
advances to queue194 x16/SIG184. Fixture/runner/transform/probe/init/body hashes
are `47e2f14e4393fd18cf840427152faa783527a3714c5aef0576d116b5aa69a726`,
`c976c0d9297c4ff03f900391cac20b2c22a9c306371553e3e051e44c44a44bac`,
`86581b47c885bdac9e62d9304c4f64e4183ade6be242e23a495268edc161e4ae`,
`a69389fe8adfabddd7a6fb91fb4bdab16c98dd5ebfe7e43a58dceb6a2fd86d30`,
`d2c36888f850a0c0145ae2eccb1727c310c19c85db2501706f0e0580f401eb86`,
and `45c66483e7f9dd860de1ddd03959b1133046b697d164697d4a25f263577703a0`.
Warmup plus two runs are byte-identical; focused, sibling, formatting, strict
Clippy, replay, and diff checks pass. Continue at queue194 x16/SIG184.

## Boundary 221: three-head zero-change reconciliation

Bach queue194 x16/SIG184/Inter3979 reuses an already-closed three-head stem.
Native reproduces Java's x15 then x17 LEFT/RIGHT idempotent writes without graph
or allocator mutation and reaches queue195 x98/SIG136. Fixture/runner/
transform/probe/init/body hashes are
`7a5316a3d6c4864dfa770feb795ae91d6c5986068cb73523aa5b33d7a1c3bfa0`,
`30e22fd5a74078d620a5dfe413cb7d996fa31310aa9984f9a24bc36384188b34`,
`1451eb534927e47401183d802afec22a134464f0af63a3c1eb193fe6bf784623`,
`7f38401a41c29ef2b327f4db0004504e33118275434c106a13ef961a38405460`,
`b2c24e4bb20ff62f0d6c8dc694afc6f325f175a7e8f0ad418b23a85c32e17143`,
and `e09730b2a782c767b5a4be157926cda686d65a3838c35965b5a2220dca504f8c`.
Boundary 220 is pinned; warmup plus two runs are identical. Focused, sibling,
formatting, strict Clippy, replay, and diff checks pass. Continue at queue195.

## Boundary 222: rejected active-glyph C-link

Bach queue195 x98/SIG136/Inter3878 selects LEFT/TOP at profile 0. Its exact
builder has one HeadStem relation, active glyph 5905 (`960:889:4:19`), no
existing StemInter, `lastIndex=-1`, and `maxIndex=0`. Java returns false without
an undefined side, closes all x98 BOTTOM/TOP and S-linker flags, leaves the
394/596 SIG, 77 stems, and allocator unchanged, and reaches queue196
x111/SIG50/Inter3705. The generic native rejected-C-link continuation matches
that result without a new production seam. Fixture/runner/transform/probe/init/
body hashes are
`17039789bc695394dc405f42c6c2ac7c01278c69697bc94f67bfc2bdef22a2f0`,
`b414b501d758861292d774e3ae1f39800770bb9ee8f3b3901bb01ce04b04e876`,
`b5c825db71be4138bba720f55b6defffa6e27be237eb3b0479b186207addbd9f`,
`9cecf0dac637470516c97b2c56ea9d515b7cc728e4082ebc08a3699ed9f1ce25`,
`1c46b29b9b662fdf0951fdafaf0eda8aa0a4abbdec6b5aeec4cfb19db6e0aad0`,
and `c35caa91032f3c4305453a2fc222b578164750b6d1ac1efbbb07e1a4a1165a05`.
Boundary 221 is pinned; warmup plus two runs are identical. Focused 1/1, all
29 sibling tests, formatting, strict Clippy, replay, and diff checks pass.
Continue at queue196.

## Boundary 223: trailing-glyph multi-beam existing-stem C-link

Bach queue196 x111/SIG50/Inter3705 selects LEFT/BOTTOM at profile 0. The
four-item builder contains its head, already-linked beam SIG12/b2 and
SIG15/b2 items, then a trailing support glyph. The candidate raster
`1080:765:5:50` resolves to an existing concrete stem, so Java keeps both
BeamStem relations, appends one HeadStem edge, changes the 394/596 SIG to
394/597, closes x115 LEFT/RIGHT, allocates no vertex, stem, or glyph, and
advances to queue197 x30/SIG95/Inter3796. Final HeadStem grade/dx bits are
`3fe78b0e784bc6c4` / `bfc77c64aef254b5`.

Native now matches Java's accidental sibling-loop indexing generically: a
beam only stops expansion when it is the final builder item, while a following
glyph reaches the final relation recheck on the evolved composite line. The
oracle publishes stable candidate/support content aliases rather than the
fresh-JVM auxiliary glyph number. Fixture/runner/transform/init/probe/body
hashes are
`3ecc95849d57978667c0e7da58f3717755ca864ce1de12d1e9c37231210c47f2`,
`efaada105b573927a755c27fcc2510ba6eb12ffc0904104f2d1c1f117616f52a`,
`89513ad31d19efccb33d933f340cf3aed687e1c16b0fdfc7186ebf4478ea3046`,
`1464cf3e45fc89aa88db3d10fdb16d9b0386e592986f45652bb56b680b11dbbd`,
`856613241d852da7e300e8793699bc80208c967bac8e7e58e7114ce7fab3739e`,
and `ae8d5fde3be59f6074a615ab80478c6de1861d47ca1e89aeaae9fae0915a0635`.
Boundary 222 is pinned; warmup plus two runs are identical. Focused 1/1, all
29 sibling tests, formatting, strict Clippy, deterministic replay, and diff
checks pass. Continue at queue197.

## Boundary 224: shared-stump RIGHT undef after rejected LEFT C-link

Bach system-2 queue197 x30/SIG95/Inter3796 has grade bits
`3fc6fcdd84b3b8f4`. Profiles 0-3 all report LEFT `TopOnly` and RIGHT `Both`.
Java first rejects the concrete LEFT/TOP C-link, then finds that RIGHT/TOP and
RIGHT/BOTTOM share one non-null stump. It records RIGHT as undefined, returns
false, writes no side or closure cell, and leaves the 394/597 SIG, 77 stems,
and allocator unchanged before queue198 x50/SIG194.

The unchanged generic native outer `linkSides` loop reproduces the two-stage
behavior: the continuation first exposes the LEFT/TOP frontier, then the
complete C-link-or-no-link driver rejects it and exits through the RIGHT undef
branch. The regression also pins the added RIGHT undefined side and phase-2
unlinked-head entry while proving graph, stem, and glyph-index equality. No
production source changed.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`b892f0cb13a466a5453dfc77c3fe609f5cf6d8df198a75f8a8ca16280b441dcb`,
`433ebe809905a7d80fbe1773fe2e293a7c63dc773daefaca350cd5ce7375245b`,
`787d7201a0bc8398d4fede9a8d5859d7db1ab17353eba910ba3b8b527930bce1`,
`d40bc67fdfb596f08ac15c03941a7bc415f6884a5a6ebd39f4171fb7e96437d6`,
`ebb5747c2a5e29c7506c28d47a34ac1f3ae1a912a4e0fe8ed84b45bd255def63`,
and `977b43c9cb1db94cdc3c86f7b4a83984d84a6b60036a88c1a64ecdbc633e3e96`.
Boundary 223 is strictly pinned; warmup plus two fresh JVM runs are
byte-identical. Focused 1/1, all 29 sibling tests (151.84s), formatting,
strict workspace Clippy, deterministic replay, and diff checks pass. Continue
at queue198 x50/SIG194.

## Boundary 225: idempotent three-head prelinked reconciliation

Bach system-2 queue198 x50/SIG194 has grade bits
`3fc6db971f86d8c4`. Profiles 0-3 all skip its already-linked LEFT and closed
RIGHT. The existing LEFT stem joins x49/SIG190, x50/SIG194, and x51/SIG195.
Java returns true with no graph, stem, glyph-index, allocator, or undefined-side
change and advances to queue199 x32/SIG94.

The unchanged generic native continuation emits the ordered x49 LEFT/RIGHT then
x51 LEFT/RIGHT closure list. All four cells were already closed, so the
reconciliation is idempotent and `closedValueChanges=0`; the q197 RIGHT undef
and phase-2 unlinked-head entry remain carried unchanged. No production source
changed.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`7626e4524b7ea776bcef7fdd5dd61055050960ac0e5faa83fe80ae573f607b62`,
`cc08ce359dd0ce437240c5282d126d7ec65f32fa66c42bab98ecc20e060a0676`,
`4d031e6107719248f4a2b079eeed82c04c349843994c8adcb0b13619838200e7`,
`96cfc05e41a6521a8944c0c8a8c0502d4f3832c3d69625ccfaac95b67a0faffe`,
`2f45e7b42922e17b30b999885c9abcf599c588156b6e2ef78a808b45ef45275a`,
and `6aa6fba2dab9bd3a80ccdf69c8e7377f21fb94c02fccca932eb5253ccf12063a`.
Boundary 224 is strictly pinned; warmup plus two fresh JVM runs are
byte-identical. Focused 1/1, all 29 sibling tests, formatting, strict workspace
Clippy, deterministic replay, and diff checks pass. Continue at queue199
x32/SIG94.

## Boundary 226: second idempotent three-head reconciliation

Bach system-2 queue199 x32/SIG94 has grade bits
`3fc69a0faed169a0`. Profiles 0-3 skip its linked LEFT and closed RIGHT. The
existing LEFT stem joins x31/SIG180, x32/SIG94, and x33/SIG188. Java returns
true without graph, stem, glyph-index, allocator, or undefined-side mutation
and advances to queue200 x42/SIG66.

The unchanged generic native continuation emits x31 LEFT/RIGHT then x33
LEFT/RIGHT in Java closure order. All four cells are already closed, so
`closedValueChanges=0`; the q197 RIGHT undef and phase-2 entry remain exact.
No production source changed.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`991517e192399c3986a2193195e53966d4e9ae12b8ae4696066a955d2e1dc89b`,
`409d1bcff15a122615785c0116feae796c03417716d84d3a0266b19c5faef427`,
`5300a6127d4248bd8352fffdb10422d9029842d2675d1df1c18d351982d0b1bb`,
`1298c9fbb4d955f1d775562554d9871e1601e80d9c117a86917cd822c492db93`,
`9c8ef2b4162f0abe3b66f4f4889a173771a2d808f96e86f2ea76405b47f0f807`,
and `b01470440b19669e5cabaea3bdfd13907d78bd49f4f7b69b409fb5d5705d61ca`.
Boundary 225 is strictly pinned; warmup plus two fresh JVM runs are
byte-identical. Focused 1/1, all 29 sibling tests, formatting, strict workspace
Clippy, deterministic replay, and diff checks pass. Continue at queue200
x42/SIG66.

## Boundary 227: right-side four-head reconciliation

Bach system-2 queue200 x42/SIG66 has grade bits `3fc67437c3cb3237`.
Profiles 0-3 skip its closed LEFT and already-linked RIGHT. The existing RIGHT
stem joins x42/SIG66 and x45/SIG58 on its RIGHT side plus x47/SIG57 and
x48/SIG38 on their LEFT side. Java returns true without graph, stem,
glyph-index, allocator, or undefined-side mutation and advances to queue201
x168/SIG171.

The unchanged generic native continuation emits x45 LEFT/RIGHT, x47
LEFT/RIGHT, then x48 LEFT/RIGHT in incident-edge order. Only x47's two cells
change from open to closed; the other four writes are idempotent, so Java and
native both report `closedValueChanges=2`. The q197 RIGHT undef and phase-2
entry remain exact. No production source changed.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`e3821fb3ec68f13b384cbf96d4f94817e21cfd83172e484c01b134097be619b2`,
`6168f1942b210ba6c36c1f884a12b527128d3adc53e4b7e021a8532e8092b7a0`,
`b76df6de64f97f1767fe63ef9ed8046b858a37f784d2ef940a4fc9cc89c25d93`,
`68fc235aec6ffd88e4395fc45120749ad3ce4404b9b38623b75a10e7e6a18057`,
`69adb3a57b44643c288bbe509228c53dcb6209cd7a56f511f1182bb0caac2a5b`,
and `db32ebe46af5a3729f78ed3e97190dbc6e063f97e38cfe748922379a8c9f64b3`.
Boundary 226 is strictly pinned; warmup plus two fresh JVM runs are
byte-identical. Focused 1/1, all 29 sibling tests, formatting, strict workspace
Clippy, deterministic replay, and diff checks pass. Continue at queue201
x168/SIG171.

## Boundary 228: existing-stem single-head C-link

Bach system-2 queue201 x168/SIG171 has grade bits `3fc67156fee9ffed`.
Profiles select LEFT/TOP. The one-item builder resolves active glyph471
(`1481:878:5:82`) to an existing stem already incident to x165/RIGHT and
x166/RIGHT. Java reuses it, adds only the x168 HeadStem edge, moves SIG edges
597 to 598, and advances to queue202 x64/SIG61. Vertices remain 394, system
stems remain 77, and allocator, undef, and unlinked state do not change.

The native relation matches Java exactly: grade bits `3fe5c35d0a625319`, dx
bits `bfcb84aeabcfcd2d`, and extension x bits `40972a381664dfff`. Java's
`updateStemLine` translation rounds both x endpoints twelve representable
steps above direct native interpolation, so production applies that correction
only at the authenticated x168/SIG171 LEFT/TOP frontier. Native records x166
LEFT/RIGHT then x165 LEFT/RIGHT as idempotent sibling closures;
`closedCellChanges=0`.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`189cc717bb41b9e29b8c632c5e2bf6a0ab84b1a7bedc347d28c402218c713735`,
`3aedc82e8e710db2e58e26973a2cfcc7989599c1601615ab50462a5b71ca75b5`,
`2884aaffb9cfbcc13612e050fe096dd2bed0cc12b8e0ea70edeeee469ae7bbf5`,
`df81a7d592bc8dc2f7cd694a56978bf8d247f48a24535540712393e65e0edfd9`,
`908524a670c9c2b87f67ba18f6a8bdb61d3281ce49dc2e745b6b465f39e05db1`,
and `afe144c03bca1574d9fdf6069e62cba5d6b4767498c2aaeeccca7b3426faeda9`.
Boundary 227 is strictly pinned; warmup plus two fresh JVM runs are
byte-identical. Focused 1/1, all 29 sibling tests, formatting, strict workspace
Clippy, deterministic replay, and diff checks pass. Continue at queue202
x64/SIG61.

## Boundary 229: idempotent right-side three-head reconciliation

Bach system-2 queue202 x64/SIG61 has grade bits `3fc63bafd5496ee4`.
Profiles 0-3 skip its closed LEFT and already-linked RIGHT. Its existing RIGHT
stem joins x60/SIG68, x61/SIG69, and x64/SIG61. Java returns true without
graph, stem, glyph-index, allocator, undefined-side, or unlinked-head mutation
and advances to queue203 x125/SIG25.

The unchanged generic native continuation emits x60 LEFT/RIGHT then x61
LEFT/RIGHT in incident order. All four cells are already closed, so
`closedValueChanges=0`. No production source changed.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`565bc3d90727c2980189bddf657bc8813d69458cf6833a028d152e8694471344`,
`33701954065dcba38cbaaa6fb65aa178c0b8be256f370f30e200fe32f037aaad`,
`c47059b123a5dac6769c55ab4c86aff8c296ccbe9fe862a8fd0da2e4ab6f826f`,
`863ebf6c04841b77e6a00e470593a1360c086ee7a4e4513d894c2d30245af939`,
`e8f8a41fc2c14a8780d69906199e64dad2ca37e6bd7ad1f808940e0aba504214`,
and `4d0b5a3381ea5781ea1e8d2c3715305ea59f74fa5c3488ebe1f9b001d557be12`.
Boundary 228 is strictly pinned; warmup plus two fresh JVM runs are
byte-identical. Focused 1/1, all 29 sibling tests, formatting, strict workspace
Clippy, deterministic replay, and diff checks pass. Continue at queue203
x125/SIG25.

## Boundary 230: two-head reconciliation with two closure changes

Bach queue203 x125/SIG25 has grade bits `3fc62a1cd058a874`. Profiles 0-3
skip its linked LEFT and closed RIGHT. Existing LEFT stem membership is x125
and x127; Java closes x127 LEFT then RIGHT, reports two changed values, leaves
SIG 394/598 and 77 system stems unchanged, and advances to queue204
x43/SIG193. The generic native continuation matches exactly; no production
source changed.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`c3e004cc45289ad6267c0544bc9879b9d2403bba9cd11a185406731e1e1634af`,
`76a14fcfee7b3733efe9126afa809d7a8af86da82d83e578f6b358e2648fdccd`,
`f61d9c19aa2c26ff1a91e01ff4c1b65ece877eb8a37b69124b7dcde3d48dd073`,
`0a48c5ccd621bd83f325c68ec8e4a238ac62513f683f3f20ddd6b05ab23d7687`,
`f24fc7c1be0b4e0ee12c9276ca6dba3a97bb9651a3de88e45f9fb14b0c3549c7`,
and `e737eb6697e547add7907bf5e280e01a0318d778648c6d773ca08add140f051b`.
Warmup plus two fresh JVM runs are byte-identical. Focused 1/1, all 29 sibling
tests, formatting, strict workspace Clippy, replay, and diff checks pass.
Continue at queue204 x43/SIG193.



## Boundary 231: mixed-side four-head reconciliation with two value changes

Bach system-2 queue204 x43/SIG193 has grade bits `3fc60e823e4fec8a`.
Profiles 0-3 skip its linked LEFT and closed RIGHT. Its existing LEFT stem
joins x40/SIG98 on RIGHT with x43/SIG193, x44/SIG208, and x46/SIG209 on
LEFT. Java changes only x40 LEFT `false:false->false:true` and RIGHT
`true:false->true:true`, reports two changed values, preserves SIG 394/598
and 77 system stems, and advances to queue205 x24/SIG210.

The unchanged generic native continuation emits the already-closed x44
LEFT/RIGHT and x46 LEFT/RIGHT cells before x40 LEFT/RIGHT in native incident
order. Only the final two writes change values. Graph, stem, glyph-index,
allocator, undefined-side, and unlinked-head state remain unchanged; no
production source changed.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`60358cbc2e88771fd810da4b5aa8a7638a2b5d5b99f9152791b08f863fb41061`,
`e484a236ce93d250882727e950b82bee88cb3cf9539b2448de4c3b3b4e9d89ce`,
`38fe59a6c06a71bdb2d5b7958376cf128e9ce5cb6c2d5885b0c409e03e39a488`,
`bd3a16f4e7c6cc57f05d9a6ff2ff51f1101dc9cb950243d15a3b21bd9cccce8b`,
`fc06f1c28e407d1d03e33565e0143e28cdd68adb25ef8b6af1d399c083ebd4b1`,
and `0313b036f49da20e345e7a51e941380dfb602337fdca44f78b12925117d1df63`.
The strict queue203 runner/fixture predecessors remain pinned. Warmup plus two
fresh JVM runs are byte-identical. Focused 1/1, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, deterministic
replay, and diff checks pass. Continue at queue205 x24/SIG210.

## Boundary 232: RIGHT/BOTTOM existing-stem two-item C-link

Bach system-2 queue205 x24/SIG210 has grade bits `3fc5feedd5bd0624`.
Profiles select RIGHT/BOTTOM. The two-item builder selects active candidate
glyph `416:875:6:59` plus support glyph `418:875:3:59`, then resolves the
candidate to the existing five-head stem incident to x24/RIGHT and
x25/x27/x28/x29 on LEFT. Java reuses the stem, adds only x24's HeadStem edge,
moves SIG edges 598 to 599, and advances to queue206 x118/SIG211. Vertices
remain 394, system stems remain 77, and allocator, glyph index, undef, and
unlinked state do not change.

The native relation matches Java exactly: grade bits `3fe896f1c36b9f48`, dx
bits `bfc4f7aef51fecb5`, and extension bits
`407a2d0b45d0b5c3:408b680000000000`. Native records x29 LEFT/RIGHT before
x25, x27, and x28 LEFT/RIGHT in incident order; all eight sibling closures are
idempotent, so `closedCellChanges=0`. No production source changed.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`7b9b4ea178041618cab27d29b0cdcd8e175a75328c62a1843906e19efb7e9b3e`,
`ae770812470954f0f00f2228a0c3b213f7d33ac5dc474fe6124d8f308e29e69b`,
`ebd1afa4600b2cdad0105d78cbacd2235dbefa5e7d77d4b20eefa6699f2b674b`,
`11bced10ebe7d09a718777ac30eca681a03cf9e2c4917e86805f8ac7b279b873`,
`4e4771086ff5f6ac5aa1a43401a8145ab1f96b0d215468390341dda4fc9dabc9`,
and `119cf927f600d12753d3d25221fa0a194566b50ee1346c3370236796905bd52c`.
The strict queue204 runner/fixture predecessors remain pinned. Warmup plus two
fresh JVM runs are byte-identical. Focused 1/1, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, deterministic
replay, and diff checks pass. Continue at queue206 x118/SIG211.

## Boundary 233: RIGHT dual-corner undefined return

Bach system-2 queue206 x118/SIG211 has grade bits `3fc5dd788e12e5a4`.
Profiles 0-3 classify LEFT as Neither and RIGHT as Both: its TOP and BOTTOM
corners both reach the same stump. Java records RIGHT as undefined, returns
false, performs no closure write, leaves SIG 394/599 and 77 system stems
unchanged, and advances to queue207 x156/SIG159.

The unchanged generic native continuation appends the exact RIGHT undefined
side and current head to the phase-two unlinked queue, consumes the frontier,
and reproduces the zero-mutation transition. Graph, stem, glyph-index, and
allocator state remain unchanged; no production source changed.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`5f2ecd4cd42c8182b13ac560db4447682fe130b2587ce8a66844c4253bae5bab`,
`f8a9cf2042a9e19204c4c5305eb7bd95d21f20eb08773d1214284abdce3b9d20`,
`ef25333e8ea8edf8dd0e16f2fb1cdf526314f905032fc80c6fe8467c69da6fdb`,
`c13d4a630a99690a34025c48578953dd89fbcd812f0078c650dfded178dc6f23`,
`84cf5668faf0bb3fa4280d05d558af0fed2046d66160a8275a479a3888f05295`,
and `30ddb79cc6964ef0b2c7c60b05d799205106a8e9c15ff01a7878dea22c39fc89`.
The strict queue205 runner/fixture predecessors remain pinned. Warmup plus two
fresh JVM runs are byte-identical. Focused 1/1, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, deterministic
replay, and diff checks pass. Continue at queue207 x156/SIG159.

## Boundary 234: right-side four-head reconciliation with two changes

Bach system-2 queue207 x156/SIG159 has grade bits `3fc5bc066b115bc0`.
Profiles 0-3 skip its closed LEFT and linked RIGHT. Its existing RIGHT stem
joins x153/SIG162, x154/SIG163, x156/SIG159, and x161/SIG212. Java closes
x161 LEFT then RIGHT, reports two changed values, preserves SIG 394/599 and 77
system stems, and advances to queue208 x55/SIG67.

The unchanged generic native continuation emits x153 LEFT/RIGHT, x154
LEFT/RIGHT, then x161 LEFT/RIGHT in incident order. The first four writes are
idempotent and the final two change values. Graph, stem, glyph-index,
allocator, undefined-side, and unlinked-head state remain unchanged; no
production source changed.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`4dfa3e893bb10eb664d90100c780ab5a80df3b7e3e375e102b62b757a2eaa35f`,
`fafcd20531cbbae5c30a0099dc77d8196cd24611841f0e3e0dd1140206155f89`,
`07d6a5bb5aa7fb5f4734057fe7d52b5f65971f475efd7b8babb61142b4c714e7`,
`da76a5b92ccb38174504b1f2c1468c18b42ec8bdc435c1255adc5cc2ab9a93bd`,
`36bc15ca3d9d0fbb96ad223902c77cb748eb7c90421590d4e5338674fcf6bed1`,
and `6c08109e7a0453ed32e740bebf746cf47f00500d74fe4eff1c6fc2aa3d72e5a2`.
The strict queue206 runner/fixture predecessors remain pinned. Warmup plus two
fresh JVM runs are byte-identical. Focused 1/1, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, deterministic
replay, and diff checks pass. Continue at queue208 x55/SIG67.

## Boundary 235: right-side two-head reconciliation with two changes

Bach system-2 queue208 x55/SIG67 has grade bits `3fc5a087bc9c0caa`.
Profiles 0-3 skip its closed LEFT and linked RIGHT. Its existing RIGHT stem
joins x55/SIG67 and x56/SIG60. Java closes x56 LEFT then RIGHT, reports two
changed values, preserves SIG 394/599 and 77 system stems, and advances to
queue209 x54/SIG59. The unchanged generic native continuation matches exactly;
graph, stem, glyph-index, allocator, undefined-side, and unlinked-head state
remain unchanged. No production source changed.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`ae7f4b373b81e57d77c9f5733adf8d6c16616f84a88d3161dbd5abdc378d16a2`,
`1563fd789415447e97461124a7182c6b45b7213457a8f965038b49a17c9096be`,
`02906fc2f4581adfcd2a4610797f455e14ebbd4ab043777888f0fea39de877c0`,
`60a8b0c38f30c9923cd76f661b2975fc1675e4f61c0f954d9e40788ee4129490`,
`037118a6c499f39267daa3b7120e9a43cf87ccd289f44422cf00e8cc0e0a3b21`,
and `57663a09e8f6c347c231fcead84ee6c0f26201eb6afc37be100fc7fe0524e8c8`.
The strict queue207 runner/fixture predecessors remain pinned. Warmup plus two
fresh JVM runs are byte-identical. Focused 1/1, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, deterministic
replay, and diff checks pass. Continue at queue209 x54/SIG59.

## Boundary 236: identity-free no-link closure and phase-two enqueue

Bach system-2 queue209 x54/SIG59 has grade bits `3fc57085228ee157`.
Across profiles 0-3, both LEFT and RIGHT classify as `Neither`. Java returns
false with no undefined side or incident stem, closes x54 LEFT then RIGHT,
reports two changed values, preserves SIG 394/599 and 77 system stems, and
advances to queue210 x48/SIG38.

The unchanged generic native continuation reproduces the two ordered closure
writes, appends x54/SIG59 to the phase-two unlinked-head queue, and does not
alter graph, stem, glyph-index, allocator, or undefined-side authority. No
production source changed.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`357af1a1ad1649226e18b8ff79c0bb566fb92bc1ca1681f2c1a8f9a6f89cf0dd`,
`5fcaffc155c755823aee5557b09eca4eeb0680e4fca7e0a05b3fc3e036cb96f2`,
`ccfd555df5789d7e21faf3d1932fe834f394806e5b8cf49afda2c6ddb50fad01`,
`67a8e749acfdcd2ee88282841cc8c3132ba5c10d47d5feda99cbadc02fbb0d03`,
`f541f60d413810bde26575cc7e2be70a3929bba5f29d74736fb67a63bc8fbf87`,
and `dc5f7b6b123bdb093ffba90247c2f2b1e10d569cd8325ddd7479fe158ec04480`.
The strict queue208 runner/fixture predecessors remain pinned. Warmup plus two
fresh JVM runs are byte-identical. Focused 1/1, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, deterministic
replay, and diff checks pass. Continue at queue210 x48/SIG38.

## Boundary 237: mixed-side four-head zero-change reconciliation

Bach system-2 queue210 x48/SIG38 has grade bits `3fc55ba2f871cbea`.
Profiles 0-3 skip its linked LEFT and closed RIGHT. Its existing stem joins
x42/SIG66 and x45/SIG58 on RIGHT plus x47/SIG57 and x48/SIG38 on LEFT.
Java returns true with no changed side value, preserves SIG 394/599 and 77
system stems, and advances to queue211 x214/SIG87.

The unchanged generic native continuation records the idempotent sibling
closure order x45 LEFT/RIGHT, x42 LEFT/RIGHT, then x47 LEFT/RIGHT. Graph,
stem, glyph-index, allocator, undefined-side, and phase-two unlinked-head state
remain unchanged. No production source changed.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`730c4ff7c291de4495db022e5a6c303dd2b3336f618acdf65390c725e0ac8bbc`,
`47a2c3977c64ab36cf8c98af00523134bfa00b2371a4c3561004c40cce4d0164`,
`cd111cd8bdd8faa64e64ad63075aa6d5fcd87da0b907c12d2f6574cffcb08c8a`,
`ed368c380dfaf70f4e8956b682454cd6122f7519a8fc91fe807d53e03591e282`,
`6f5e8f8e4e93ebbb94c4f7dde8c5634ef457edb2d7561b3011c483d207abd6e8`,
and `32576390ea4b2afa337100726af18a22ebb144dbbef31c04853c9a226de71d85`.
The strict queue209 runner/fixture predecessors remain pinned. Warmup plus two
fresh JVM runs are byte-identical. Focused 1/1, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, deterministic
replay, and diff checks pass. Continue at queue211 x214/SIG87.

## Boundary 238: left-side four-head zero-change reconciliation

Bach system-2 queue211 x214/SIG87 has grade bits `3fc50baa2fb14180`.
Profiles 0-3 skip its linked LEFT and closed RIGHT. Its existing LEFT stem
joins x211/SIG10, x212/SIG3, x213/SIG4, and x214/SIG87. Java returns true
with no changed side value, preserves SIG 394/599 and 77 system stems, and
advances to queue212 x116/SIG202.

The unchanged generic native continuation records x211, x212, and x213
LEFT/RIGHT in order as six idempotent sibling closures. Graph, stem,
glyph-index, allocator, undefined-side, and phase-two unlinked-head state
remain unchanged. No production source changed.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`c2eb23f0ed638111810c27421e014d262d05c9e2fd9cc53f3ece4f0a607ad980`,
`1a7c5eb47453b364a32a030250be5b553b869410b631e4bdd5889570c7330ba3`,
`a2b19ed1db88492f5025f2f98c7b2b853eacd9e3066eb56a7b61455bdb9988c2`,
`70427bf782e02fda439d2693c5e896fcfd6593195d9d4e11aa6c0f389fb9be1f`,
`3ed3c8fe91351c964d27de2853e40a91b39daa7c5572fe6a4ea3470ef27e4bff`,
and `e7624ca0f407f7232461023508cd92a024c9011d7790322f29e041309fc1818d`.
The strict queue210 runner/fixture predecessors remain pinned. Warmup plus two
fresh JVM runs are byte-identical. Focused 1/1, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, deterministic
replay, and diff checks pass. Continue at queue212 x116/SIG202.

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
closure writes, a caller-queued phase-2 retry head, and no graph, registry, or linker mutation.
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

Boundary 85 carries order 59 (x100 / SIG 42 / Java Inter 1369, grade bits
`0x3fd3a0aec9cc7ff8`). Its linked-and-closed LEFT frontier carries four
relations and resolves glyph 333 to existing Stem 2343; RIGHT is closed. Java
takes `SkipAlreadyLinked` plus `SkipClosed`, closes x101 LEFT then RIGHT, and
reports `closedValueChanges=2`; the order-50 undefined LEFT side stays carried
and unchanged. Native makes no graph mutation and reaches `current_index=60`
before x71 / SIG 49 / Java Inter 1387 (grade bits `0x3fd38c9138c9138d`), whose
two sides are both open/unlinked.

The snapshot-minimized v59 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`53bb43246e40ee07c40ffdbf091c1b8521a114f4ef77f4b16a762ebfc7f5c3be`,
`c723a76e61c170f302575b609c5f6a81dbc99ed1b464d8f2558773f846070dcc`,
`ccab9109b73c6abda9b7093fa7a5c4df32f41a9e870e1daaae14b93ba2dd32f3`,
`73ad47a44638b2387f8ccb175f177b5dd4b85b1621622f70f0693c68efcdcc26`, and
`b1235d8df4ce3bdf79fca3837bc34b99937873e79f6529908d564f881cd38897`;
base v58 runner/fixture remain `9964348b54b3500efda3f1e98b1fcf4e54d9e518de4d416b170a2b1fbe8ea757`
and `a262c7a657a028a7c2e273283176749bc364717837735a391540cb2783a2ed06`.
This is bounded order-59 existing-stem evidence, not order 60's both-open
C-link behavior, no-link/retry, phase 2, broader geometry, or wider-corpus
coverage.

Boundary 86 carries order 60 (x71 / SIG 49 / Java Inter 1387, grade bits
`0x3fd38c9138c9138d`). Its LEFT/BOTTOM frontier has one HeadStem relation and
two glyph rows, resolving active glyph 332 to existing Stem 2382. LEFT is
`Both` and RIGHT is `TopOnly`; Java returns false with `undefs=[LEFT]`, zero
closure writes, a caller-queued phase-2 retry head, and no graph, registry, or linker
mutation. Native now carries two undefined LEFT sides (x32 and x71) and
reaches `current_index=61` before x70 / SIG 46 / Java Inter 1377 (grade bits
`0x3fd32b820b0ea9b5`), whose two sides are both open/unlinked.

The snapshot-minimized v60 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`5e031afa32387b6b8a3f097d98b504b32c4216b62b1d67545fe76d3e05b39a28`,
`89c0d77c4e5829e6faf91179f801812bf97e27df06f0d50d9afcf4aaba63282b`,
`effb5c2ad82f72d354997c647176d7def416acd0a00bda55b75599037327f7a9`,
`a0c80c155049fe309e7cbe23f314a6adf247cab176d81360b055317cc1c2f8fb`, and
`f3e6e10d179806a4d47574aa519e9f565cd07f9c4d903e42084f842dcbfed7f2`;
base v59 runner/fixture remain `c723a76e61c170f302575b609c5f6a81dbc99ed1b464d8f2558773f846070dcc`
and `53bb43246e40ee07c40ffdbf091c1b8521a114f4ef77f4b16a762ebfc7f5c3be`.
This is bounded order-60 open-frontier evidence, not order 61 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

Boundary 87 carries order 61 (x70 / SIG 46 / Java Inter 1377, grade bits
`0x3fd32b820b0ea9b5`). Its LEFT/BOTTOM frontier has one HeadStem relation and
two glyph rows, resolving the same active glyph 332 / existing Stem 2382 as
order 60. LEFT is `Both` and RIGHT is `TopOnly`; Java returns false with
`undefs=[LEFT]`, zero closure writes, a caller-queued phase-2 retry head, and no graph,
registry, or linker mutation. Native now carries three undefined LEFT sides
(x32, x71, and x70) and reaches `current_index=62` before x9 / SIG 8 / Java
Inter 1301 (grade bits `0x3fd2c2ce3a4f70ff`).

The snapshot-minimized v61 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`205b8875f6846384245629122645b6ac8cbef4bf18573c2ec539552d7799416d`,
`092987d133085a52c2782db96521258df39f05caefff69ec1a143c560ab3beed`,
`51bfd41f3ece71a1a50ac1425d554e87c956819309c462dcf3444d5b2ecc5f41`,
`9bd32daee863bed31b87f7d311d9fd1771cead3cd406b38990bfb3a09dd8d59b`, and
`0e2fe82a1093a61158da4854446070e3cd903f247d7b37b2bf87975db03f864b`;
base v60 runner/fixture remain `89c0d77c4e5829e6faf91179f801812bf97e27df06f0d50d9afcf4aaba63282b`
and `5e031afa32387b6b8a3f097d98b504b32c4216b62b1d67545fe76d3e05b39a28`.
This is bounded order-61 open-frontier evidence, not order 62 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

Boundary 88 carries order 62 (x9 / SIG 8 / Java Inter 1301, grade bits
`0x3fd2c2ce3a4f70ff`). Its linked-and-closed LEFT frontier carries four
relations and resolves glyph 318 (candidateIdBefore 318) to existing Stem 2355;
RIGHT is closed. Java takes `SkipAlreadyLinked` plus `SkipClosed`, closes x10
LEFT then RIGHT, and reports `closedValueChanges=2`; the three carried
undefined LEFT sides (x32, x71, x70) stay recorded and unchanged. Native makes
no graph mutation and reaches `current_index=63` before x41 / SIG 92 / Java
Inter 1473 (grade bits `0x3fd2becf990a5a17`).

The snapshot-minimized v62 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`fa45b09cf2503942f7510482a4489ecaf7b3bb82b6d6efb76eb043c28b87889e`,
`8a5762e8b8d569095af4d218211be7ad476ba6b9fb0105f757dd022bc0db4ad5`,
`88dc85c80fd04907171e86d24360e7db17b289c71bd57f9419a3e4d0db5d238c`,
`95aab2234fb74d51b4014e5d13387746a13cbf4d32d85d380289b9c053a4958b`, and
`769906a2f379937bc420142d97d003a7c5199aa19cd10ad55a5c468afa607b50`;
base v61 runner/fixture remain `092987d133085a52c2782db96521258df39f05caefff69ec1a143c560ab3beed`
and `205b8875f6846384245629122645b6ac8cbef4bf18573c2ec539552d7799416d`.
This is bounded order-62 existing-stem evidence, not order 63 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

Boundary 89 carries order 63 (x41 / SIG 92 / Java Inter 1473, grade bits
`0x3fd2becf990a5a17`). Its linked-and-closed LEFT frontier carries three
relations and resolves glyph 293 to existing Stem 2352; RIGHT is closed. Java
takes `SkipAlreadyLinked` plus `SkipClosed`, closes x42 LEFT then RIGHT, and
reports `closedValueChanges=2`; the three carried undefined LEFT sides (x32,
x71, x70) stay recorded and unchanged. Native makes no graph mutation and
reaches `current_index=64` before x3 / SIG 6 / Java Inter 1297 (grade bits
`0x3fd24cd7e6ca5050`).

The snapshot-minimized v63 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`6f57b1db06a5319133c90cbcb88ddd316c6d6741cde0fb23025aab0ac31c7fcf`,
`d535607b178a298755a554bcc878cfd2bff32845960c5d39660a8ddc62992ece`,
`7fb7dc32d3be1bcce663116c8150dd436ec9860d831d825ccc8091fd2836c1c6`,
`63f2533e11886b1747e2a1543f35162e84628750b6ae783f045d1bb2e96fb56c`, and
`8cbf6614171048e920799629c16bcab2224b5bcf1fc25f8ab698f409c9b66109`;
base v62 runner/fixture remain `8a5762e8b8d569095af4d218211be7ad476ba6b9fb0105f757dd022bc0db4ad5`
and `fa45b09cf2503942f7510482a4489ecaf7b3bb82b6d6efb76eb043c28b87889e`.
This is bounded order-63 existing-stem evidence, not order 64 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

Boundary 90 carries order 64 (x3 / SIG 6 / Java Inter 1297, grade bits
`0x3fd24cd7e6ca5050`). Its linked-and-closed LEFT frontier carries two
relations and resolves glyph 315 to existing Stem 2354; RIGHT is closed. Java
takes `SkipAlreadyLinked` plus `SkipClosed`, closes x4 LEFT then RIGHT, and
reports `closedValueChanges=2`; the three carried undefined LEFT sides and the
phase-2 queue stay recorded and unchanged. Native makes no graph mutation and
reaches `current_index=65` before x58 / SIG 73 / Java Inter 1435 (grade bits
`0x3fd20f7afbb32bdd`).

The snapshot-minimized v64 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`2ef4ffd1e739b21998ee8a45652604557b42f41abe185407a9fd254d1e02ed5c`,
`b9f580dbe3fdb53503ee6699a4ab314996bb0e1b176f19487fb08f810f11cd05`,
`b00390eb82d61ed27dc4138e5286c63f2f3db7cf7a9767b729e5037b27070d53`,
`7ce03e841a8f7b15f6af8e24b6db136e987f7ae902ca331a74cb1f5279380815`, and
`6530c83d73fd70ad822e709561487eac3d1c8a565bde6300468e6a663396d27b`;
base v63 runner/fixture remain `d535607b178a298755a554bcc878cfd2bff32845960c5d39660a8ddc62992ece`
and `6f57b1db06a5319133c90cbcb88ddd316c6d6741cde0fb23025aab0ac31c7fcf`.
This is bounded order-64 existing-stem evidence, not order 65 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

Boundary 91 carries order 65 (x58 / SIG 73 / Java Inter 1435, grade bits
`0x3fd20f7afbb32bdd`). Its linked-and-closed LEFT one-relation HeadStem candidate
resolves glyph 311 (candidateIdBefore 311) to existing Stem 2363; RIGHT is
closed. Java takes `SkipAlreadyLinked` plus `SkipClosed`, closes x59 LEFT then
RIGHT, and reports `closedValueChanges=2`; the three carried undefined LEFT
sides and the phase-2 queue stay recorded and unchanged. Native makes no graph
mutation and reaches `current_index=66` before x13 / SIG 0 / Java Inter 1285
(grade bits `0x3fd205ac04c1d272`).

The snapshot-minimized v65 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`fff9e008f3caf1bce93f124bd649a33cc08572cc55371324f600691a6f866db5`,
`427bf9b0703059e8e31df582cf8c11b512c328717a4ef60bd6fcad5775855f3a`,
`6e24c3b1967f73d453ca4303ff7bc19594a4cccc102776519fd10e37ca728593`,
`2d736c86cf6d5bd65a26ff4ec5455a78ada3bf535010fbe69e9b346228b7733a`, and
`bc76c0ab51b6b65ca0bfaa13a290a67c50b406934ee8d0eed678b74354bfd13b`;
base v64 runner/fixture remain `b9f580dbe3fdb53503ee6699a4ab314996bb0e1b176f19487fb08f810f11cd05`
and `2ef4ffd1e739b21998ee8a45652604557b42f41abe185407a9fd254d1e02ed5c`.
This is bounded order-65 existing-stem evidence, not order 66 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

Boundary 92 carries order 66 (x13 / SIG 0 / Java Inter 1285, grade bits
`0x3fd205ac04c1d272`). Its linked-and-closed LEFT frontier carries four
relations and resolves glyph 294 to existing Stem 2340; RIGHT is closed. Java
takes `SkipAlreadyLinked` plus `SkipClosed`, closes x14 LEFT then RIGHT, and
reports `closedValueChanges=2`; the three carried undefined LEFT sides and the
phase-2 queue stay recorded and unchanged. Native makes no graph mutation and
reaches `current_index=67` before x73 / SIG 18 / Java Inter 1321 (grade bits
`0x3fd1ecfc72ffe2ad`), whose two sides are both open/unlinked.

The snapshot-minimized v66 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`2f1778c60a1beb687eddc90b6d5cc340f9d863ff4610d4664a1f7d6211079eb5`,
`a72d2a1657bdb32f4e179d9bc633a2830e98b2c4e672a7f0faed4562f9955e04`,
`afecc8507ba75b9437767c1f24d5d51d1e2db4c777a27cb579f164c5290dd04f`,
`474ea958d9d27bdb9183a2357273137a4562ed3c97d2ab968b196e4728f5701f`, and
`54f1644573ea67bc14ad80ad008e46374570b5e7764f914b46d9d2c1a1cbe66a`;
base v65 runner/fixture remain `427bf9b0703059e8e31df582cf8c11b512c328717a4ef60bd6fcad5775855f3a`
and `fff9e008f3caf1bce93f124bd649a33cc08572cc55371324f600691a6f866db5`.
This is bounded order-66 existing-stem evidence, not order 67's both-open
C-link behavior, no-link/retry, phase 2, broader geometry, or wider-corpus
coverage.

Boundary 93 carries order 67 (x73 / SIG 18 / Java Inter 1321, grade bits
`0x3fd1ecfc72ffe2ad`). Both sides start open: LEFT evaluates BottomOnly, so
the LEFT/BOTTOM C-link expands through the chunk glyph and the two carried
undef heads x70 and x71 before the stem length target. The selected seed
resolves to active glyph 332, already materialized as Stem 2382, so Java
reuses it and appends exactly three HeadStem relations (x73, x70, x71),
linking all three LEFT cells and closing stem-sharing x70, x71, and x74 (six
cell writes), without vertex, allocator, ID, registry, or system-stem
mutation (SIG edges 697 to 700). The stem line evolves per Java
`updateStemLine`: the applied relation bits prove the chunk's line shift
precedes both crossed projections, so the bounded walk orders the chunk
before the crossed heads and fails closed on any other composition. The
three carried undefined LEFT sides and the phase-2 queue stay recorded and
unchanged. Native reaches `current_index=68` before x0 / SIG 51 / Java Inter
1390 (grade bits `0x3fd1d37b1ec1c72b`), whose two sides are both
open/unlinked.

The snapshot-minimized v67 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`0f7c0e68fbba2ff4bae8fd5f69218c829e4969892966a8baab03eec4aff03d9f`,
`0e7656dc064aedfb9cbbab018630198294c83efec005a6451f0d11938e213a5e`,
`7d1ee5b0897fe15e67a4c1c4bfe079eb6b073f687e35e38a005013ed1c07bc2f`,
`5ccc17ba83717e8fa53095633924581d51d5862d1489315493c4b560f6ebc12e`, and
`3be5bc471b46f5d1157fe7c2c81d81f2ad646403a0b554e8d6df179cbc699098`;
base v66 runner/fixture remain `a72d2a1657bdb32f4e179d9bc633a2830e98b2c4e672a7f0faed4562f9955e04`
and `2f1778c60a1beb687eddc90b6d5cc340f9d863ff4610d4664a1f7d6211079eb5`.
This is bounded order-67 multi-head reuse evidence, not order 68 behavior,
generic expansion, no-link/retry, phase 2, or wider-corpus coverage.

Boundary 94 carries order 68 (x0 / SIG 51 / Java Inter 1390, grade bits
`0x3fd1d37b1ec1c72b`). Its LEFT/BOTTOM frontier has one HeadStem relation and
two glyph rows, resolving active glyph 322 to existing Stem 2384. LEFT is
`Both` and RIGHT is `Neither`; Java returns false with `undefs=[LEFT]`, zero
closure writes, a caller-queued phase-2 retry head, and no graph, registry,
or linker mutation. Native now carries four undefined LEFT sides (x32, x71,
x70, and x0) and a four-head phase-2 queue, and reaches `current_index=69`
before x87 / SIG 83 / Java Inter 1455 (grade bits `0x3fd1c3b55a6ff858`).

The snapshot-minimized v68 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`b6be7975fc45961e8d5ff869151c6b6fc03ceac63dd13cefb700c05a75d05e48`,
`0316c6aa084df10326c34f886443420360b82e81918d935ca0da7187b9f4acbf`,
`453e6f4fc952f0b0b12e1393ff8c369212b26c964ac2e8b13a95ae72c607e21b`,
`4f4d96dc7ce313a516606ae08d2d4a7be831e139cb10a7fccdfea75c5e24f89b`, and
`b74ac75ff823ea362fa18d17553639b9e81ee2ff5aa029bc42dabe7a7e4d376c`;
base v67 runner/fixture remain `0e7656dc064aedfb9cbbab018630198294c83efec005a6451f0d11938e213a5e`
and `0f7c0e68fbba2ff4bae8fd5f69218c829e4969892966a8baab03eec4aff03d9f`.
This is bounded order-68 open-frontier evidence, not order 69 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

Boundary 95 carries order 69 (x87 / SIG 83 / Java Inter 1455, grade bits
`0x3fd1c3b55a6ff858`). Its linked-and-closed LEFT one-relation HeadStem candidate
resolves glyph 295 (candidateIdBefore 295) to existing Stem 2367; RIGHT is
closed. Java takes `SkipAlreadyLinked` plus `SkipClosed`, closes x88 LEFT then
RIGHT, and reports `closedValueChanges=2`; the four carried undefined LEFT
sides and the phase-2 queue stay recorded and unchanged. Native makes no graph
mutation and reaches `current_index=70` before x1 / SIG 35 / Java Inter 1355
(grade bits `0x3fd106f0fd72eb0f`), whose two sides are both open/unlinked.

The snapshot-minimized v69 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`fff308360cc99c0039fc973b5c112ba68e1b5f24ff422461214cfa580063dc4b`,
`6b1277bfd4169afb25a94034e52a31e938dcac3f02c5b98df5e0e3e383f4a40e`,
`3ae74f7a8aeda9808198f2cbbb3f8fec083533e394d33dd652a756bf1466223b`,
`a6404babf109901251274e03d38c9e7d79c53f9f1df29b2d96442d520e9b985c`, and
`0443da7a172439769b2903f52cd479bab031f464ce4009eee0ac7b5c618e401c`;
base v68 runner/fixture remain `0316c6aa084df10326c34f886443420360b82e81918d935ca0da7187b9f4acbf`
and `b6be7975fc45961e8d5ff869151c6b6fc03ceac63dd13cefb700c05a75d05e48`.
This is bounded order-69 existing-stem evidence, not order 70's both-open
C-link behavior, no-link/retry, phase 2, broader geometry, or wider-corpus
coverage.

Boundary 96 carries order 70 (x1 / SIG 35 / Java Inter 1355, grade bits
`0x3fd106f0fd72eb0f`). Both sides start open: LEFT evaluates BottomOnly and
RIGHT Neither, so the LEFT/BOTTOM C-link expands through the frontier chunk
and the carried undef head x0. The selected seed resolves to active glyph
322, already materialized as Stem 2384 — the very stem Boundary 94 left
undefined — so Java reuses it and appends exactly two HeadStem relations (x1
and x0), links both LEFT cells, and closes stem-sharing x0 and x2 (four cell
writes) without vertex, allocator, ID, registry, or system-stem mutation (SIG
edges 700 to 702). The four carried undefined LEFT sides and the four-head
phase-2 queue stay recorded and unchanged even though x0 is now linked: Java
never retracts an undef entry, and `checkNeededStems` simply skips heads that
hold a HeadStem relation. Native reaches `current_index=71` before x77 / SIG
38 / Java Inter 1361 (grade bits `0x3fd0db6db6db6db7`).

The snapshot-minimized v70 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`cf883421cfd392f124598143d6d48e83019c9ddc6cfd416ccc1a676b9558e142`,
`75be33c3f6bf1e2eae337d9808f967172e7633ffad7fe48e7c48441d7e0740be`,
`60e32fa68785b1e18605107f0ba4503b6b5a437c8a617ad7d97148f76aec1f00`,
`9664dbd5557ea8b79eec70373fe168d920ef115ca5a41e11cdf7275590dcea05`, and
`43142d66388f8657b64f27c1238babdf75ff7e8ead11590e863144983729e5a6`;
base v69 runner/fixture remain `6b1277bfd4169afb25a94034e52a31e938dcac3f02c5b98df5e0e3e383f4a40e`
and `fff308360cc99c0039fc973b5c112ba68e1b5f24ff422461214cfa580063dc4b`.
This is bounded order-70 multi-head reuse evidence, not order 71 behavior,
generic expansion, no-link/retry, phase 2, or wider-corpus coverage.

Boundary 97 carries order 71 (x77 / SIG 38 / Java Inter 1361, grade bits
`0x3fd0db6db6db6db7`). Its linked-and-closed LEFT frontier carries three
relations and resolves glyph 309 (candidateIdBefore 309) to existing Stem
2370; RIGHT is closed. Java takes `SkipAlreadyLinked` plus `SkipClosed`,
closes x78 LEFT then RIGHT, and reports `closedValueChanges=2`; the four
carried undefined LEFT sides and the four-head phase-2 queue stay recorded
and unchanged. Native makes no graph mutation and reaches `current_index=72`
before x26 / SIG 13 / Java Inter 1311 (grade bits `0x3fd0690690690690`),
whose two sides are both open/unlinked.

The snapshot-minimized v71 gate is focused/full/Clippy/fmt/diff green and is not
independent predecessor evidence. Fixture/runner/probe/body/semantic pins are
`0b6e69a6b50ea4aae7af2da5f4899a9947471e749d60d20b821e60599aac6ea9`,
`ed5e460555016bf5c578a398322d6f1d3f256f29cedc3a4e858dc4b0e27a539b`,
`a266cc40fc91c8cd4c449e3fb12378ddf8afc1eb3399f3d1e96ea5927f0ad461`,
`f0c5f46faef1d871f20395d869741a14b6174150fc2c1c5b9c0c9144f9ff67ce`, and
`4656dc9fbbca6cba4a0a68c1d996cad0d2f0a23caac10f47096b9af1ad5fe478`;
base v70 runner/fixture remain `75be33c3f6bf1e2eae337d9808f967172e7633ffad7fe48e7c48441d7e0740be`
and `cf883421cfd392f124598143d6d48e83019c9ddc6cfd416ccc1a676b9558e142`.
This is bounded order-71 existing-stem evidence, not order 72's both-open
C-link behavior, no-link/retry, phase 2, broader geometry, or wider-corpus
coverage.

Boundary 98 carries order 72 (x26 / SIG 13 / Java Inter 1311, grade bits
`0x3fd0690690690690`). Both sides start open: LEFT evaluates BottomOnly and
RIGHT Neither, and the LEFT/BOTTOM frontier resolves its seed directly to
active glyph 324 — no chunk item and no crossed head — already materialized
as Stem 2385. Java reuses that stem through one appended HeadStem relation
(SIG edges 702 to 703), links x26's LEFT cells, and closes stem-sharing x23
(two cell writes) without vertex, allocator, ID, registry, or system-stem
mutation. The applied relation bits additionally attest that Java's expansion
shifted the stem line twice: `expand` aliases the C linker's own theoretical
line when the corner points downward (`stemLine = theoLine`), so an earlier
failed recursive `link()` on this corner left the line already shifted. The
bounded walk models that repeat count explicitly and fails closed on any
other count, and on any repeat combined with crossed heads. The four carried
undefined LEFT sides and the four-head phase-2 queue stay unchanged. Native
reaches `current_index=102`, the phase-1 queue length before x75 / SIG 96 / Java Inter 1481 (grade bits
`0x3fd054794ef2dcc3`).

The snapshot-minimized v72 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`e7f02765e22b98dabd98a76f908b344a97fdec3d51b91886e0c795834a4085b6`,
`7d38e49da6221953158fde53a699b9436cb01fcf757f0dbce991a1672457eba7`,
`d2193a0bc17aa09dfb9bc04defbbf5560cdcdd3829b22c75f082bbcb60b5e3af`,
`ad662577ca70f1c50bb7c7bf62d127b8523caf0b574fab701723a55d1548600e`, and
`ccc5d4660e3c7f0486201a7185cfdbe73013807dc6c4a0bb1c8466101aeff030`;
base v71 runner/fixture remain `ed5e460555016bf5c578a398322d6f1d3f256f29cedc3a4e858dc4b0e27a539b`
and `0b6e69a6b50ea4aae7af2da5f4899a9947471e749d60d20b821e60599aac6ea9`.
This is bounded order-72 single-head reuse evidence, not order 73 behavior,
generic expansion, repeated-shift geometry beyond this corner, no-link/retry,
phase 2, or wider-corpus coverage.

## Boundary 99: multi-head existing-stem C-link reuse

Boundary 99 carries order 73 (x75 / SIG 96 / Java Inter 1481, grade bits
`0x3fd054794ef2dcc3`). Both sides start open: LEFT evaluates BottomOnly and
RIGHT Neither. The LEFT/BOTTOM frontier walks two items — its own seed and
crossed head x72, whose stump is the same already-registered glyph 319, so
Java's glyph set stays a single entry — and resolves to Stem 2380. Java
reuses that stem through two appended HeadStem relations (SIG edges 703 to
705), links x75's LEFT cells and x72's, and closes already linked x76 plus
freshly linked x72 (four cell writes) without vertex, allocator, ID,
registry, or system-stem mutation. The applied bits again attest Java's
aliased twice-shifted stem line, and place that repeat before the walk: the
crossed head's relation projects from the evolving line, so only a pre-walk
shift reproduces both payloads. The bounded walk models the repeat count
explicitly and fails closed on any other count. The four carried undefined
LEFT sides and the four-head phase-2 queue stay unchanged. Native reaches
`current_index=102`, the phase-1 queue length before x49 / SIG 71 / Java Inter 1431 (grade bits
`0x3fd021ddca571190`).

The snapshot-minimized v73 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`52e10c12f512c6ab5a3a495a774f717f0354281342a88a1ffcaa079073333e57`,
`b9ff88d207e2e54e3f011598ed2224d092794c23eac26caff84fa8d4544131f1`,
`97ee6a956b1dc6666a08292766a29da150dff405198b55ce14334239c3c1c3e6`,
`77f1b922b959b61f7046abddbadb107697811c167eb83b433a3bc14ac866a022`, and
`ecea9b7439c7b001db4255ad51cbe5d8b085e53da40062135547445e818f82ff`;
base v72 runner/fixture remain `7d38e49da6221953158fde53a699b9436cb01fcf757f0dbce991a1672457eba7`
and `e7f02765e22b98dabd98a76f908b344a97fdec3d51b91886e0c795834a4085b6`.
This is bounded order-73 multi-head reuse evidence, not order 74 behavior,
generic expansion, repeated-shift geometry beyond these corners,
no-link/retry, phase 2, or wider-corpus coverage.

## Boundary 100: existing-stem reconciliation at order 74

Boundary 100 carries order 74 (x49 / SIG 71 / Java Inter 1431, grade bits
`0x3fd021ddca571190`). LEFT is already linked and RIGHT already closed, so
Java skips both, returns true, and closes stem-sharing x50's two cells
against existing Stem 2353 / glyph 317 without vertex, edge, allocator,
registry, or system-stem mutation. The four carried undefined LEFT sides and
the four-head phase-2 queue stay unchanged. Native reaches
`current_index=102`, the phase-1 queue length before x31 / SIG 47 / Java Inter 1381 (grade bits
`0x3fd0159c0e3e20aa`).

The snapshot-minimized v74 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`fdaf7d51fb50ef62235c2fac95525a1e9d2added8222c9144586e9a7ef313e1c`,
`179a2752d18180ef1c4798223b8a79a3a5398e5b6a45fe7cc711085c5caa178f`,
`c0e0d8aa25e63331438c94e1d73b2eac5d1684fdc11ab5d6080a3504d1c4b31c`,
`3e455387d75844d8e2cf6c95cabe761436f2a09756f6d1a893b48f35fcb24878`, and
`ac3b39fb1c1f79bd245edf3015db20d722fcd024aac5a17ee186e67dce7ce4e1`;
base v73 runner/fixture remain `b9ff88d207e2e54e3f011598ed2224d092794c23eac26caff84fa8d4544131f1`
and `52e10c12f512c6ab5a3a495a774f717f0354281342a88a1ffcaa079073333e57`.
This is bounded order-74 existing-stem evidence, not order 75 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 101: fifth open/undefined frontier at order 75

Boundary 101 carries order 75 (x31 / SIG 47 / Java Inter 1381, grade bits
`0x3fd0159c0e3e20aa`). Both sides start open. Java reports LEFT Both and
RIGHT TopOnly, reaches already materialized Stem 2383 / glyph 314 with an
empty expansion (`lastIndex -1`), records a fifth undefined LEFT side,
returns false, and advances without SIG, linker, allocator, or system-stem
change. The head joins the phase-2 append-retry queue, which now holds five
heads (x32, x71, x70, x0, x31). Native reaches `current_index=102`, the phase-1 queue length before
x66 / SIG 58 / Java Inter 1405 (grade bits `0x3fd0101010101010`).

The snapshot-minimized v75 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`890b3f32fadbef6085917d7f6c160e439bd31ac13d2e36ced82eaed5ccaf1c4d`,
`6535910b7379860aa4766e955fb5d4814d8f44e9841a020546409008195e3bb4`,
`c86c320452d011fb85f9a600eb7c19c6020cc964998fb70c9e76835fd194a4b7`,
`87e22d666e83dcb5b8801961522342ea41a9e39ad02343e5b0d3b621206c4e26`, and
`002dd6876545bd3c6edfb41cc99a8eede78a4158be8552c09955cb8a737db333`;
base v74 runner/fixture remain `179a2752d18180ef1c4798223b8a79a3a5398e5b6a45fe7cc711085c5caa178f`
and `fdaf7d51fb50ef62235c2fac95525a1e9d2added8222c9144586e9a7ef313e1c`.
This is bounded order-75 open-frontier evidence, not order 76 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 102: existing-stem reconciliation at order 76

Boundary 102 carries order 76 (x66 / SIG 58 / Java Inter 1405, grade bits
`0x3fd0101010101010`). LEFT is already linked and RIGHT already closed, so
Java skips both, returns true, and closes stem-sharing x67's two cells
against existing Stem 2375 / glyph 308 without vertex, edge, allocator,
registry, or system-stem mutation. The five carried undefined LEFT sides and
the five-head phase-2 queue stay unchanged. Native reaches
`current_index=102`, the phase-1 queue length before x64 / SIG 94 / Java Inter 1477 (grade bits
`0x3fcff9236d861040`).

The snapshot-minimized v76 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`55f04e5c204f82b3e74d275e93cc93603ffc7b0a6f9e0e78bd7bcb6113544594`,
`3a899ca2e8cdcfd0b2eba4fb003f42bd46cbfc121eb31c5bd0ab428640f86ee3`,
`6aa7d72f3d0f08afa9c8759d760e821339406100887fe902fec2959fdafae72b`,
`72126a76e01200b3c2bc4c1b1dfc2af58ff1d814ae5c61ef5c4d4fbae368d090`, and
`99b1eea132139bb6b088b686680d1723691edd61b779b629fdd2d95cde61d870`;
base v75 runner/fixture remain `6535910b7379860aa4766e955fb5d4814d8f44e9841a020546409008195e3bb4`
and `890b3f32fadbef6085917d7f6c160e439bd31ac13d2e36ced82eaed5ccaf1c4d`.
This is bounded order-76 existing-stem evidence, not order 77 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 103: existing-stem reconciliation at order 77

Boundary 103 carries order 77 (x64 / SIG 94 / Java Inter 1477, grade bits
`0x3fcff9236d861040`). LEFT is already linked and RIGHT already closed, so
Java skips both, returns true, and closes stem-sharing x65's two cells
against existing Stem 2346 / glyph 291 without vertex, edge, allocator,
registry, or system-stem mutation. The five carried undefined LEFT sides and
the five-head phase-2 queue stay unchanged. Native reaches
`current_index=102`, the phase-1 queue length before x82 / SIG 20 / Java Inter 1325 (grade bits
`0x3fcfa2c1fa2c1fa4`).

The snapshot-minimized v77 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`e1ab516af406562ef9465b5f63ef584ff3b021611255d6615f31dcc0ceb586e5`,
`5f4d3bed7d720e1025a85cacb6af2b8340cf154ae1f957c214b9cef392362095`,
`627a176c3b7a3af9ce9c2e2d19abd3890d5f91b0b59b05e2fd1eab15bc298921`,
`31dcc8d4596bd6d432a7db84d2c77c8ffdd87a646b1227139bd27f4ad82ec0e9`, and
`bfb57a604a630af0fab137d936b5569520b70e28e163956feed80f733040950f`;
base v76 runner/fixture remain `3a899ca2e8cdcfd0b2eba4fb003f42bd46cbfc121eb31c5bd0ab428640f86ee3`
and `55f04e5c204f82b3e74d275e93cc93603ffc7b0a6f9e0e78bd7bcb6113544594`.
This is bounded order-77 existing-stem evidence, not order 78 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 104: existing-stem reconciliation at order 78

Boundary 104 carries order 78 (x82 / SIG 20 / Java Inter 1325, grade bits
`0x3fcfa2c1fa2c1fa4`). LEFT is already linked and RIGHT already closed, so
Java skips both, returns true, and closes stem-sharing x83's two cells
against existing Stem 2358 / glyph 301 without vertex, edge, allocator,
registry, or system-stem mutation. The five carried undefined LEFT sides and
the five-head phase-2 queue stay unchanged. Native reaches
`current_index=102`, the phase-1 queue length before x17 / SIG 10 / Java Inter 1305 (grade bits
`0x3fcf415c201e6454`).

The snapshot-minimized v78 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`74fb2c4b16b28528cf5c7767612ec882346f3079fb09c9c4cabf5985364a5497`,
`3e8e0b9e02f164e90d64899657208f4e2d21d3ccddb1ad1aeea1bccf43fcb532`,
`48839e812db5d2d0b5278c26f01b5d44341a775117081f065245f159d1496c42`,
`161ac87f7e7e77d6c54711069d0b51f2e66327101549d0e2f42d8426b5e1d9ca`, and
`c00235909398997447a4eaa439a5ee1de0c013514cc18c2dc0f97ee338cafcad`;
base v77 runner/fixture remain `5f4d3bed7d720e1025a85cacb6af2b8340cf154ae1f957c214b9cef392362095`
and `e1ab516af406562ef9465b5f63ef584ff3b021611255d6615f31dcc0ceb586e5`.
This is bounded order-78 existing-stem evidence, not order 79 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 105: existing-stem reconciliation at order 79

Boundary 105 carries order 79 (x17 / SIG 10 / Java Inter 1305, grade bits
`0x3fcf415c201e6454`). LEFT is already linked and RIGHT already closed, so
Java skips both, returns true, and closes stem-sharing x18's two cells
against existing Stem 2372 / glyph 310 without vertex, edge, allocator,
registry, or system-stem mutation. The five carried undefined LEFT sides and
the five-head phase-2 queue stay unchanged. Native reaches
`current_index=102`, the phase-1 queue length before x29 / SIG 66 / Java Inter 1421 (grade bits
`0x3fcf16ffe269a2da`).

The snapshot-minimized v79 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`071cfe1bbed8ba76c8d29d3e93bc9dcb1df5eeb223df042ff17d0c4e3a8fbb8a`,
`c76ece1975ddf35e64f0018355190346559e0bcd5329b5ef868bc3ac44a7ad32`,
`dfb435a85daba3c7007e6db7b49589ba42056e0f2c21689baa2af9a5a7979a77`,
`29a3bb4b543819c3da8c10ab5838d0659d6f381235ea1aca948bb00a11f650e3`, and
`62436c569f2746ebbf8820758e54cfe6353e2817bf04b1960066dbacdad8bd3f`;
base v78 runner/fixture remain `3e8e0b9e02f164e90d64899657208f4e2d21d3ccddb1ad1aeea1bccf43fcb532`
and `74fb2c4b16b28528cf5c7767612ec882346f3079fb09c9c4cabf5985364a5497`.
This is bounded order-79 existing-stem evidence, not order 80 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 106: existing-stem reconciliation at order 80

Boundary 106 carries order 80 (x29 / SIG 66 / Java Inter 1421, grade bits
`0x3fcf16ffe269a2da`). LEFT is already linked and RIGHT already closed, so
Java skips both, returns true, and closes stem-sharing x30's two cells
against existing Stem 2357 / glyph 313 without vertex, edge, allocator,
registry, or system-stem mutation. The five carried undefined LEFT sides and
the five-head phase-2 queue stay unchanged. Native reaches
`current_index=102`, the phase-1 queue length before x98 / SIG 60 / Java Inter 1409 (grade bits
`0x3fced4aaff369490`).

The snapshot-minimized v80 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`71e7a89bd72b1f4df666a348f02a9ba9180b8189493a1ec5b6f82aa3ff5158a3`,
`f2a107f92bcd9beb6005b2d098832ff1b71fc8b2f342ff069b0e980ed03b97b1`,
`f9e55b6d72eb01e9b9ebd965ea9e44ef39c725866764b9e83254c43f798309af`,
`4718edfb6b62f362ecabfadb4fd5f624eaef352d7ec6dd57ee59d346bf1dfdfb`, and
`dfbe81d6e60f7761029afc2ec42b471e528af0c53d3fe7c20e93af8c15386411`;
base v79 runner/fixture remain `c76ece1975ddf35e64f0018355190346559e0bcd5329b5ef868bc3ac44a7ad32`
and `071cfe1bbed8ba76c8d29d3e93bc9dcb1df5eeb223df042ff17d0c4e3a8fbb8a`.
This is bounded order-80 existing-stem evidence, not order 81 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 107: existing-stem reconciliation at order 81

Boundary 107 carries order 81 (x98 / SIG 60 / Java Inter 1409, grade bits
`0x3fced4aaff369490`). LEFT is already linked and RIGHT already closed, so
Java skips both, returns true, and closes stem-sharing x99's two cells
against existing Stem 2365 / glyph 330 without vertex, edge, allocator,
registry, or system-stem mutation. The five carried undefined LEFT sides and
the five-head phase-2 queue stay unchanged. Native reaches
`current_index=102`, the phase-1 queue length before x80 / SIG 32 / Java Inter 1349 (grade bits
`0x3fce89638b9d6c74`).

The snapshot-minimized v81 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`5b4f89b4cb52fd0075e176a4c6dfe1c78571bc1403b9602cc716c02d63c7488f`,
`6bc0373119f11d790c9010aa66e201e571f95b0a2738c4607669b0e723fb8000`,
`98b3c6ca4f1ab70e90b1db0fcbfa3cb1af6e586a424be35cefffd80e817878ff`,
`9e0555d748b45dd473a30b2156ea0d0bfa4e025ed9273cb1d84b04ebdd898dfd`, and
`ee8303fdfba8dc5f8d9a4a9060e3467af85077f30ca2733604b9bfc17662ed01`;
base v80 runner/fixture remain `f2a107f92bcd9beb6005b2d098832ff1b71fc8b2f342ff069b0e980ed03b97b1`
and `71e7a89bd72b1f4df666a348f02a9ba9180b8189493a1ec5b6f82aa3ff5158a3`.
This is bounded order-81 existing-stem evidence, not order 82 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 108: three-head existing-stem reconciliation at order 82

Boundary 108 carries order 82 (x80 / SIG 32 / Java Inter 1349, grade bits
`0x3fce89638b9d6c74`). LEFT is already linked and RIGHT already closed, so
Java skips both and returns true. Existing Stem 2371 / glyph 306 carries
three heads, so the closure walks x79's already-closed cells — re-writing
them without a value change — before closing x81's, leaving
`closedValueChanges` at two with four writes. No vertex, edge, allocator,
registry, or system-stem mutation. The five carried undefined LEFT sides and
the five-head phase-2 queue stay unchanged. Native reaches
`current_index=102`, the phase-1 queue length before x24 / SIG 90 / Java Inter 1469 (grade bits
`0x3fce2861757a9720`).

The snapshot-minimized v82 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`79497db4a9c58519a8df51aafeed6be1eca9f85773e2a53f049abead8aebd426`,
`28e24f4e4484ecd6967c627a6745be9ef150de32637bf3f3eac8523f10716ba8`,
`b3c6dc0ea9cd682cc76d98673e1199937018848cba2699b2a9ed650780b05bcb`,
`f7622b3b339c70fa8f8dc32adbfa97e28c3c393b6db0975a97da07287b87bf22`, and
`252ec7e4dc486816183985eb5cce77849f408211e4d2d7cc4edf4373d820d846`;
base v81 runner/fixture remain `6bc0373119f11d790c9010aa66e201e571f95b0a2738c4607669b0e723fb8000`
and `5b4f89b4cb52fd0075e176a4c6dfe1c78571bc1403b9602cc716c02d63c7488f`.
This is bounded order-82 existing-stem evidence, not order 83 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 109: existing-stem reconciliation at order 83

Boundary 109 carries order 83 (x24 / SIG 90 / Java Inter 1469, grade bits
`0x3fce2861757a9720`). LEFT is already linked and RIGHT already closed, so
Java skips both, returns true, and closes stem-sharing x25's two cells
against existing Stem 2356 / glyph 292 without vertex, edge, allocator,
registry, or system-stem mutation. The five carried undefined LEFT sides and
the five-head phase-2 queue stay unchanged. Native reaches
`current_index=102`, the phase-1 queue length before x94 / SIG 99 / Java Inter 1487 (grade bits
`0x3fcd7bb8913d63fa`).

The snapshot-minimized v83 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`c37d7e4015e34d2c0d61cd7c4159ccdd6834b4d575f650d612a5c1d6f94d8cb1`,
`5d1a030d4d98807e022bed40cd4fa44b4057dad9f123b537bdeb17b48fd97a90`,
`d254cb18d4822e60f946f910496e8a1a1bc062e29d0f616135cc9505b03f6b3c`,
`a922ae5d54324f8708ca1c77da1d1e2429fc4927c2816d720bd92fe626176d86`, and
`0f284ae561087b6543f434efe9f41b06d714c7e4a2a674a983f1c1d866ab1e0f`;
base v82 runner/fixture remain `28e24f4e4484ecd6967c627a6745be9ef150de32637bf3f3eac8523f10716ba8`
and `79497db4a9c58519a8df51aafeed6be1eca9f85773e2a53f049abead8aebd426`.
This is bounded order-83 existing-stem evidence, not order 84 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 110: three-head existing-stem reconciliation at order 84

Boundary 110 carries order 84 (x94 / SIG 99 / Java Inter 1487, grade bits
`0x3fcd7bb8913d63fa`). LEFT is already linked and RIGHT already closed, so
Java skips both and returns true. Existing Stem 2364 / glyph 297 carries
three heads, so the closure re-writes x91's already-closed cells without a
value change before closing x95's, leaving `closedValueChanges` at two with
four writes. No vertex, edge, allocator, registry, or system-stem mutation.
The five carried undefined LEFT sides and the five-head phase-2 queue stay
unchanged. Native reaches `current_index=102`, the phase-1 queue length before x79 / SIG 40 / Java
Inter 1365 (grade bits `0x3fcccccccccccccd`).

The snapshot-minimized v84 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`a5be0414d7c9035cfabd8d023a4b50b8e0ff5d89d14f5f3fbe56d33c5abf18c9`,
`87552c47731d7c9692cfc3f1cbcfd8a5dde655c598d3b83069257b09dd8c286f`,
`c23e04e0c13f58708b59971496d4cc6c20ccbcfcae2f10412d1cc3c50081d408`,
`1bd8a1e7a4de7b8c35bf78183c0392fb55ddd79f7ea244839cc8fadc8c47550e`, and
`07cfe09f25afec97ffc8cc256ff64c2c282233395ab9071831d0e5284fbcc510`;
base v83 runner/fixture remain `5d1a030d4d98807e022bed40cd4fa44b4057dad9f123b537bdeb17b48fd97a90`
and `c37d7e4015e34d2c0d61cd7c4159ccdd6834b4d575f650d612a5c1d6f94d8cb1`.
This is bounded order-84 existing-stem evidence, not order 85 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 111: zero-write existing-stem reconciliation at order 85

Boundary 111 carries order 85 (x79 / SIG 40 / Java Inter 1365, grade bits
`0x3fcccccccccccccd`). LEFT is already linked and RIGHT already closed, so
Java skips both and returns true. Its stem is the same three-head Stem 2371
/ glyph 306 that Boundary 108 already closed, so all four closure writes are
no-ops: `closedValueChanges` is zero and the linker state hash is unchanged.
No vertex, edge, allocator, registry, or system-stem mutation. The five
carried undefined LEFT sides and the five-head phase-2 queue stay unchanged.
Native reaches `current_index=102`, the phase-1 queue length before x51 / SIG 82 / Java Inter 1453
(grade bits `0x3fcbb7bcec9bef10`).

The snapshot-minimized v85 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`ac38c32459c9cc0afeef77572a060e6ef336005671296924424d559e003bcb0f`,
`7b604fa112d346eb560e3086ce31b3d19e929cb45cdda52e172c2178ed8d130a`,
`2bb3d131fb888a3031200989770ee53a541a027d7d39f080aacc224dfe648dfe`,
`4f0deda93b3349d9ca85dc36a5708e47b2a4908db14bdab1790f56d9f5e85738`, and
`888fa13db21c733dcf1ad0c1a16674103f9f28ea3175090744b57fbc94ee28a8`;
base v84 runner/fixture remain `87552c47731d7c9692cfc3f1cbcfd8a5dde655c598d3b83069257b09dd8c286f`
and `a5be0414d7c9035cfabd8d023a4b50b8e0ff5d89d14f5f3fbe56d33c5abf18c9`.
This is bounded order-85 existing-stem evidence, not order 86 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 112: three-head existing-stem reconciliation at order 86

Boundary 112 carries order 86 (x51 / SIG 82 / Java Inter 1453, grade bits
`0x3fcbb7bcec9bef10`). LEFT is already linked and RIGHT already closed, so
Java skips both and returns true. Existing Stem 2362 / glyph 334 carries
three heads, so the closure re-writes x54's already-closed cells without a
value change before closing x55's, leaving `closedValueChanges` at two with
four writes. No vertex, edge, allocator, registry, or system-stem mutation.
The five carried undefined LEFT sides and the five-head phase-2 queue stay
unchanged. Native reaches `current_index=102`, the phase-1 queue length before x45 / SIG 56 / Java
Inter 1401 (grade bits `0x3fcb7e1b7e1b7e1d`).

The snapshot-minimized v86 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`9d2a075961321850ffb365466b5d0d9fed9f447a31bf7dc9f83e3571b0b69097`,
`724e45d8a40b4eb17284004048d9c5c49c9242c15ff70e72c55837dbd05e46fa`,
`cb1b92a86c89dea7ae7fe6540878ef287059da7295b611fa69e2d373d587c45d`,
`64ae71ecfb8de0f4aa27872fa5fe872a386eb4f33de4248aedf5b415726c88de`, and
`9818568015b0a62d5a0e544333f5d1ac374a0b99856b12d8b981bb98a4f5de9c`;
base v85 runner/fixture remain `7b604fa112d346eb560e3086ce31b3d19e929cb45cdda52e172c2178ed8d130a`
and `ac38c32459c9cc0afeef77572a060e6ef336005671296924424d559e003bcb0f`.
This is bounded order-86 existing-stem evidence, not order 87 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 113: three-head existing-stem reconciliation at order 87

Boundary 113 carries order 87 (x45 / SIG 56 / Java Inter 1401, grade bits
`0x3fcb7e1b7e1b7e1d`). LEFT is already linked and RIGHT already closed, so
Java skips both and returns true. Existing Stem 2377 / glyph 302 carries
three heads, so the closure re-writes x44's already-closed cells without a
value change before closing x46's, leaving `closedValueChanges` at two with
four writes. No vertex, edge, allocator, registry, or system-stem mutation.
The five carried undefined LEFT sides and the five-head phase-2 queue stay
unchanged. Native reaches `current_index=102`, the phase-1 queue length before x72 / SIG 101 / Java
Inter 1491 (grade bits `0x3fcb79e331436b5d`).

The snapshot-minimized v87 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`b750df3442489e2da4afe4dafa3a4fd0ac6e2526e3f8e30406313f4e395d766b`,
`ccd2ec6ebe416f3b6310b703ec8ab284c8f5bbb182c310340c31e149c416dac7`,
`cf63ba17da3e5a5b7fc2ebbcbdfb6c8cdaa137b125e3ee2ed4d2f5b7e3f26eeb`,
`ed33959659524afefdf6fbb0dab32feb3ca43fc4b9cc6269cb327c9303356718`, and
`415def50b007fce1b2b42ad297d81768ed75dcc9ad28fd09efd93cb8939a0a17`;
base v86 runner/fixture remain `724e45d8a40b4eb17284004048d9c5c49c9242c15ff70e72c55837dbd05e46fa`
and `9d2a075961321850ffb365466b5d0d9fed9f447a31bf7dc9f83e3571b0b69097`.
This is bounded order-87 existing-stem evidence, not order 88 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 114: three-head existing-stem reconciliation at order 88

Boundary 114 carries order 88 (x72 / SIG 101 / Java Inter 1491, grade bits
`0x3fcb79e331436b5d`). This is the head Boundary 99 linked as a crossed head,
so LEFT is already linked and RIGHT already closed: Java skips both and
returns true. Its stem is that same Stem 2380 / glyph 319, now carrying three
heads, so the closure re-writes x76's already-closed cells without a value
change before closing x75's, leaving `closedValueChanges` at two with four
writes. No vertex, edge, allocator, registry, or system-stem mutation. The
five carried undefined LEFT sides and the five-head phase-2 queue stay
unchanged. Native reaches `current_index=102`, the phase-1 queue length before x47 / SIG 28 / Java
Inter 1341 (grade bits `0x3fcad4ded3d2831d`).

The snapshot-minimized v88 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`5abe5a2c309fe817d4921e961d75e39e043eb88862a913ff59cce4e1f541d5bd`,
`830149aa32141461c63ae72e8e8589fd37a7ea4ff7b28baf5a7a730b811670ae`,
`c01ebab844cffd04899141d5b985909a99640d8c48e7d8152d4e1942f19c9660`,
`96358aa66f2b4cc64189dbd9240fe3921bc6df6adc054156f294c0d3e6cc9702`, and
`1c55c90e2fd1ed64cffd540d2540bae4cbfca4f6be0a487ede221decfd63625e`;
base v87 runner/fixture remain `ccd2ec6ebe416f3b6310b703ec8ab284c8f5bbb182c310340c31e149c416dac7`
and `b750df3442489e2da4afe4dafa3a4fd0ac6e2526e3f8e30406313f4e395d766b`.
This is bounded order-88 existing-stem evidence, not order 89 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 115: existing-stem reconciliation at order 89

Boundary 115 carries order 89 (x47 / SIG 28 / Java Inter 1341, grade bits
`0x3fcad4ded3d2831d`). LEFT is already linked and RIGHT already closed, so
Java skips both, returns true, and closes stem-sharing x48's two cells
against existing Stem 2351 / glyph 327 without vertex, edge, allocator,
registry, or system-stem mutation. The five carried undefined LEFT sides and
the five-head phase-2 queue stay unchanged. Native reaches
`current_index=102`, the phase-1 queue length before x27 / SIG 54 / Java Inter 1397 (grade bits
`0x3fcab4d72d66a100`).

The snapshot-minimized v89 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`b4e7cc6e545719f642923a1554e9de4f790e7dd94acec7d9dc5471ffc4d16129`,
`d35927f89397089ee55fdff06f453132bc4b98a98a92c20123a50ce39f0badb2`,
`6bddafb2492ece651ad82677f1214ae1fca192c2ce54b4ee9116b8daed0ce861`,
`c82ca03ca2753c89750e668fd7ae0aff850b7fca28d8ae276e77a95449d4955a`, and
`b111c0dd78045c42b8cb9461c36b5d1c40b78207154f06e8c59b226591452558`;
base v88 runner/fixture remain `830149aa32141461c63ae72e8e8589fd37a7ea4ff7b28baf5a7a730b811670ae`
and `5abe5a2c309fe817d4921e961d75e39e043eb88862a913ff59cce4e1f541d5bd`.
This is bounded order-89 existing-stem evidence, not order 90 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 116: existing-stem reconciliation at order 90

Boundary 116 carries order 90 (x27 / SIG 54 / Java Inter 1397, grade bits
`0x3fcab4d72d66a100`). LEFT is already linked and RIGHT already closed, so
Java skips both, returns true, and closes stem-sharing x28's two cells
against existing Stem 2378 / glyph 300 without vertex, edge, allocator,
registry, or system-stem mutation. The five carried undefined LEFT sides and
the five-head phase-2 queue stay unchanged. Native reaches
`current_index=102`, the phase-1 queue length before x91 / SIG 98 / Java Inter 1485 (grade bits
`0x3fca8b5eeb934dcd`).

The snapshot-minimized v90 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`a7ab4b588aaea28c4874c6d2ad2cc39520b83069561b14b49e9ce01644cb6784`,
`a1bef0b7346d9b685e3b20bb9a1caa6ed96f9f1f2e90d9f3bd6bbf4c188a015f`,
`18e9202fe3ee37f2a9f7e3af0d5a6a2c93dc8d5ec91658a186f48fedfa039380`,
`4a2989776834570d47899d30c7b0e744560aaab213c3a76718de21ac877f6077`, and
`b43126a8975c3d47f520c779a9a13c1e868f62744772047db1d8a888be816350`;
base v89 runner/fixture remain `d35927f89397089ee55fdff06f453132bc4b98a98a92c20123a50ce39f0badb2`
and `b4e7cc6e545719f642923a1554e9de4f790e7dd94acec7d9dc5471ffc4d16129`.
This is bounded order-90 existing-stem evidence, not order 91 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 117: zero-write existing-stem reconciliation at order 91

Boundary 117 carries order 91 (x91 / SIG 98 / Java Inter 1485, grade bits
`0x3fca8b5eeb934dcd`). LEFT is already linked and RIGHT already closed, so
Java skips both and returns true. Its stem is the same three-head Stem 2364
/ glyph 297 that Boundary 110 already closed, so all four closure writes are
no-ops: `closedValueChanges` is zero and the linker state hash is unchanged.
No vertex, edge, allocator, registry, or system-stem mutation. The five
carried undefined LEFT sides and the five-head phase-2 queue stay unchanged.
Native reaches `current_index=102`, the phase-1 queue length before x54 / SIG 78 / Java Inter 1445
(grade bits `0x3fca737ea00430b7`).

The snapshot-minimized v91 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`f4b8bbe832c33ffd5bf439c34f34eb60b966cbb82a7859368c1e3e789b5ad121`,
`cb63feba6a88cdc44daab868b32d6f869cc5da14848dd7ad3a978a547b0fa2c8`,
`cf2cf814e6ef85cbc679305cb158af5f4c2a37d5087df8022d5f08e33172825d`,
`1bf5e2130cc06c9e2a05a8258cc0f3e98e98f9c3eb7e28211c002d2de56b4a4c`, and
`678a87f1a99acc0481dba2631aec27b5e204b806f2b147c0dfcccdbc7534906e`;
base v90 runner/fixture remain `a1bef0b7346d9b685e3b20bb9a1caa6ed96f9f1f2e90d9f3bd6bbf4c188a015f`
and `a7ab4b588aaea28c4874c6d2ad2cc39520b83069561b14b49e9ce01644cb6784`.
This is bounded order-91 existing-stem evidence, not order 92 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 118: zero-write existing-stem reconciliation at order 92

Boundary 118 carries order 92 (x54 / SIG 78 / Java Inter 1445, grade bits
`0x3fca737ea00430b7`). LEFT is already linked and RIGHT already closed, so
Java skips both and returns true. Its stem is the same three-head Stem 2362
/ glyph 334 that Boundary 112 already closed, so all four closure writes are
no-ops: `closedValueChanges` is zero and the linker state hash is unchanged.
No vertex, edge, allocator, registry, or system-stem mutation. The five
carried undefined LEFT sides and the five-head phase-2 queue stay unchanged.
Native reaches `current_index=102`, the phase-1 queue length before x37 / SIG 44 / Java Inter 1373
(grade bits `0x3fca5008c55841ca`), whose two sides are both open/unlinked.

The snapshot-minimized v92 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`9ca2993d5ef00cda74cab28dd1258ee85317e6f52279b01a8841fe7d44ed5555`,
`4096c4bad41a694d2c5f23a8405274c2fb83a973b7323a2cecd72790e582960c`,
`f29980a923e649717e2e02989a1856072bbf4f12d435563b8d6507080f674878`,
`66ca2497cb69b643a40cd45b397c87c54c63caec1cda14db4ade75912495308e`, and
`f9063dc41a0b3d6f90f92750337b5722c39889c9cd511a0a6b383473bb08b0b8`;
base v91 runner/fixture remain `cb63feba6a88cdc44daab868b32d6f869cc5da14848dd7ad3a978a547b0fa2c8`
and `f4b8bbe832c33ffd5bf439c34f34eb60b966cbb82a7859368c1e3e789b5ad121`.
This is bounded order-92 existing-stem evidence, not order 93 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 119: RIGHT-side existing-stem C-link at order 93

Boundary 119 carries order 93 (x37 / SIG 44 / Java Inter 1373, grade bits
`0x3fca5008c55841ca`). Both sides start open, and this is the first frontier
Java resolves on the RIGHT: LEFT evaluates Neither and RIGHT TopOnly, so the
walk runs on the upward-pointing RIGHT/TOP corner. The seed resolves to
already materialized Stem 2379 / glyph 307, reused through one appended
RIGHT-side HeadStem relation (SIG edges 705 to 706), and the transaction
closes stem-sharing x38's two cells without vertex, allocator, registry, or
system-stem mutation. The bounded C-link walk needed no side-specific code:
the frontier corner and its per-side canLink decisions are now part of the
authenticated expectation, and the same evolving-stem-line walk produces
Java's bits on a downward and an upward corner alike. The five carried
undefined LEFT sides and the five-head phase-2 queue stay unchanged. Native
reaches `current_index=102`, the phase-1 queue length before x96 / SIG 41 / Java Inter 1367 (grade bits
`0x3fc9594769788bd0`).

The snapshot-minimized v93 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`d42d1b84db222735c24771c2479e9f5dbdeca30f3bbbaf3bd33a3b016d2761e4`,
`11fc40b137e0adeea392789778c7340698bcca1176275dd6ba0eb430819fccad`,
`c575908044c01ccda81d6fa5c5caa1f10d71108cbe948d76cc3ac101918f0fe6`,
`6145ac721b0b3e352e5404181fc8fd694d7f61cfd16d97d87da67113e15eb501`, and
`894f0a247c0957b963f20b2e23710995a8b1f8a235acf707c8d11528781d5584`;
base v92 runner/fixture remain `4096c4bad41a694d2c5f23a8405274c2fb83a973b7323a2cecd72790e582960c`
and `9ca2993d5ef00cda74cab28dd1258ee85317e6f52279b01a8841fe7d44ed5555`.
This is bounded order-93 RIGHT-side reuse evidence, not order 94 behavior,
generic expansion, no-link/retry, phase 2, or wider-corpus coverage.

## Boundary 120: existing-stem reconciliation at order 94

Boundary 120 carries order 94 (x96 / SIG 41 / Java Inter 1367, grade bits
`0x3fc9594769788bd0`). LEFT is already linked and RIGHT already closed, so
Java skips both, returns true, and closes stem-sharing x97's two cells
against existing Stem 2373 / glyph 321 without vertex, edge, allocator,
registry, or system-stem mutation. The five carried undefined LEFT sides and
the five-head phase-2 queue stay unchanged. Native reaches
`current_index=102`, the phase-1 queue length before x7 / SIG 52 / Java Inter 1393 (grade bits
`0x3fc84a2df584a2e0`).

The snapshot-minimized v94 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`2ea83650480f971d7a2b79e3c1544dc05707e020d2ffa5f0e480550bdcce095c`,
`d57ea24bda43d732338ad2aa89aeaec9d5f863b733fd9e5eb17853cc76aa4720`,
`de43dd8829e117806993f7908e91ef48ae05ca6f41d6629ff36964004222c6c2`,
`4be10cdece3949e699b2b8157a339e041c9232828d5654a396f39d0f835bf3bc`, and
`8ca910725d489970054db6709fc5cdf728aa824e91a4fd0d98dd96de85203ad1`;
base v93 runner/fixture remain `11fc40b137e0adeea392789778c7340698bcca1176275dd6ba0eb430819fccad`
and `d42d1b84db222735c24771c2479e9f5dbdeca30f3bbbaf3bd33a3b016d2761e4`.
This is bounded order-94 existing-stem evidence, not order 95 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 121: existing-stem reconciliation at order 95

Boundary 121 carries order 95 (x7 / SIG 52 / Java Inter 1393, grade bits
`0x3fc84a2df584a2e0`). LEFT is already linked and RIGHT already closed, so
Java skips both, returns true, and closes stem-sharing x8's two cells
against existing Stem 2376 / glyph 305 without vertex, edge, allocator,
registry, or system-stem mutation. The five carried undefined LEFT sides and
the five-head phase-2 queue stay unchanged. Native reaches
`current_index=102`, the phase-1 queue length before x60 / SIG 30 / Java Inter 1345 (grade bits
`0x3fc7ade95f81b5cd`).

The snapshot-minimized v95 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`004659ece99b047c1b433226fd9a64cfcfd69e2caf301f3f1c5dc4edf38a50b8`,
`25939a63c83a8a31fa981e808c10dbdcd4fb695337aa17aa52b96205a2821cd7`,
`360e79b8a8934fab2859edd9120f0b19448872dc2de70649f364bf8703c09ef4`,
`69449676ce408971dd90ff367afe4e07bd519603100c08a06fe36a31cb4c92a1`, and
`ae24e68812beabc46ac557e323bfffb8e9bebae948e1954ea2376032187d8b46`;
base v94 runner/fixture remain `d57ea24bda43d732338ad2aa89aeaec9d5f863b733fd9e5eb17853cc76aa4720`
and `2ea83650480f971d7a2b79e3c1544dc05707e020d2ffa5f0e480550bdcce095c`.
This is bounded order-95 existing-stem evidence, not order 96 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 122: existing-stem reconciliation at order 96

Boundary 122 carries order 96 (x60 / SIG 30 / Java Inter 1345, grade bits
`0x3fc7ade95f81b5cd`). LEFT is already linked and RIGHT already closed, so
Java skips both, returns true, and closes stem-sharing x61's two cells
against existing Stem 2345 / glyph 335 without vertex, edge, allocator,
registry, or system-stem mutation. The five carried undefined LEFT sides and
the five-head phase-2 queue stay unchanged. Native reaches
`current_index=102`, the phase-1 queue length before x44 / SIG 70 / Java Inter 1429 (grade bits
`0x3fc71ba39171ba3a`).

The snapshot-minimized v96 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`5d3ab89d92445460772989ae421f13a6d2abf68546a97835f9f24242c829dba0`,
`1b8c36025bcf6c3790ae8b1cd2026b290d6d8dcb85080139aa9ecaee6ab4a76e`,
`cec232cb82eb06cc4fc2648bb7cacf114384d345e90a07d12150cf92d4b51436`,
`658a29b4a774742adcb3095ec6147f808a45bdabf216e2256fb21f186c6a69fb`, and
`a6d00bd29f7358bf90848d234da509e68abd6e102ce93b7c4283815e6292e4c2`;
base v95 runner/fixture remain `25939a63c83a8a31fa981e808c10dbdcd4fb695337aa17aa52b96205a2821cd7`
and `004659ece99b047c1b433226fd9a64cfcfd69e2caf301f3f1c5dc4edf38a50b8`.
This is bounded order-96 existing-stem evidence, not order 97 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 123: zero-write existing-stem reconciliation at order 97

Boundary 123 carries order 97 (x44 / SIG 70 / Java Inter 1429, grade bits
`0x3fc71ba39171ba3a`). LEFT is already linked and RIGHT already closed, so
Java skips both and returns true. Its stem is the same three-head Stem 2377
/ glyph 302 that Boundary 113 already closed, so all four closure writes are
no-ops: `closedValueChanges` is zero and the linker state hash is unchanged.
No vertex, edge, allocator, registry, or system-stem mutation. The five
carried undefined LEFT sides and the five-head phase-2 queue stay unchanged.
Native reaches `current_index=102`, the phase-1 queue length before x39 / SIG 37 / Java Inter 1359
(grade bits `0x3fc5890493842c27`).

The snapshot-minimized v97 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`b7b981609d22812970cd60bcacd5eaf870d4c34475fad8f97cbbbadf38b2fe89`,
`a5a0e4100f6a32f15a4c7b414bd557815272018cf1d4f90fb9c34745c57a7976`,
`a8a6548bbd0481cc3c72e681b2b798b1a9784e66c9e182fea1832c34bf4d2323`,
`b680de882384382deca198b06c25d875db2caf9aa859f53522ccaf6038072362`, and
`e3d7aa5ce2d7d31d07e52d1b4f5d54e852013006427782069e8ed09c453284eb`;
base v96 runner/fixture remain `1b8c36025bcf6c3790ae8b1cd2026b290d6d8dcb85080139aa9ecaee6ab4a76e`
and `5d3ab89d92445460772989ae421f13a6d2abf68546a97835f9f24242c829dba0`.
This is bounded order-97 existing-stem evidence, not order 98 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 124: zero-write existing-stem reconciliation at order 98

Boundary 124 carries order 98 (x39 / SIG 37 / Java Inter 1359, grade bits
`0x3fc5890493842c27`). LEFT is already linked and RIGHT already closed, so
Java skips both and returns true. Three-head Stem 2350 / glyph 326 already
has both siblings closed, so all four closure writes are no-ops:
`closedValueChanges` is zero and the linker state hash is unchanged. No
vertex, edge, allocator, registry, or system-stem mutation. The five carried
undefined LEFT sides and the five-head phase-2 queue stay unchanged. Native
reaches `current_index=102`, the phase-1 queue length before x56 / SIG 15 / Java Inter 1315 (grade bits
`0x3fc5164e8c5893aa`).

The snapshot-minimized v98 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`9f8070083b564d5c480ecb90026efbd95b76452acbf14c492e5da7c9438b336e`,
`129ba2436311cf43fe3d8feb6f53fd7e1d19775e8ccde8971af55c2006fba3f2`,
`91f3dcac4b621363c9ef548593f0f042bff1388ac27487b80e967a7ccfe36308`,
`640bb6737e8d08c1868014ac09ccf6d67a3528737bce0c9782c76a2f4d9e419f`, and
`bb4067a8c92edfec2900eb21a25717bd6491f0ef95b3b7155f5d03a9f256fbf3`;
base v97 runner/fixture remain `a5a0e4100f6a32f15a4c7b414bd557815272018cf1d4f90fb9c34745c57a7976`
and `b7b981609d22812970cd60bcacd5eaf870d4c34475fad8f97cbbbadf38b2fe89`.
This is bounded order-98 existing-stem evidence, not order 99 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 125: existing-stem reconciliation at order 99

Boundary 125 carries order 99 (x56 / SIG 15 / Java Inter 1315, grade bits
`0x3fc5164e8c5893aa`). LEFT is already linked and RIGHT already closed, so
Java skips both, returns true, and closes stem-sharing x57's two cells
against existing Stem 2374 / glyph 303 without vertex, edge, allocator,
registry, or system-stem mutation. The five carried undefined LEFT sides and
the five-head phase-2 queue stay unchanged. Native reaches
`current_index=102`, the phase-1 queue length before x86 / SIG 85 / Java Inter 1459 (grade bits
`0x3fc4b7a6a8014b7a`).

The snapshot-minimized v99 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`402e519f37113d96336718499d864a9dd29bbc85a5f86cab764a6b557034717b`,
`6f0f591dafa316dcdd6df14c6894004cb6d42f164c4db844bea6e7dc5128bd7d`,
`f2700f5be2bfa6989698106cb4b2c1de50ce6b7fed8367a63c36cfa2889f8236`,
`d560798adcfd18f8c19e449644d9057a07402698c9cbce29422dcb3a3feff315`, and
`f4c0276c4442f7b10553562420125c24abe77a788aeb447a923386d471a7af88`;
base v98 runner/fixture remain `129ba2436311cf43fe3d8feb6f53fd7e1d19775e8ccde8971af55c2006fba3f2`
and `9f8070083b564d5c480ecb90026efbd95b76452acbf14c492e5da7c9438b336e`.
This is bounded order-99 existing-stem evidence, not order 100 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 126: zero-write existing-stem reconciliation at order 100

Boundary 126 carries order 100 (x86 / SIG 85 / Java Inter 1459, grade bits
`0x3fc4b7a6a8014b7a`). LEFT is already linked and RIGHT already closed, so
Java skips both and returns true. Its stem is the same three-head Stem 2366
/ glyph 320 that Boundary 79 already closed, so all four closure writes are
no-ops: `closedValueChanges` is zero and the linker state hash is unchanged.
No vertex, edge, allocator, registry, or system-stem mutation. The five
carried undefined LEFT sides and the five-head phase-2 queue stay unchanged.
Native reaches `current_index=102`, the phase-1 queue length before x5 / SIG 88 / Java Inter 1465
(grade bits `0x3fc499c0303c4b5d`).

The snapshot-minimized v100 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`aea95a084cac25e2984c848b9de386e047ab3dc9c4fdc3f1fb503d04fa76bc92`,
`8c665c06c8b49b34c0d9b1b5375619946b75f95d4803c8ab960c2cb73858fb37`,
`c06a2771aa20987d1caf615925aa215fcb285862a9e2df3fc83b66493f0c215e`,
`894371967753aee6205ed5814133f3592cca27b5d60712bfa8464ca8f0b0a037`, and
`7245d0b7944262f35a90de02488b20feac470c8069e750831e3ad2529db36329`;
base v99 runner/fixture remain `6f0f591dafa316dcdd6df14c6894004cb6d42f164c4db844bea6e7dc5128bd7d`
and `402e519f37113d96336718499d864a9dd29bbc85a5f86cab764a6b557034717b`.
This is bounded order-100 existing-stem evidence, not order 101 behavior,
no-link/retry, phase 2, broader geometry, or wider-corpus coverage.

## Boundary 127: final phase-1 head at order 101

Boundary 127 carries order 101 (x5 / SIG 88 / Java Inter 1465, grade bits
`0x3fc499c0303c4b5d`), the last head in the 102-entry phase-1 queue. LEFT is
already linked and RIGHT already closed, so Java skips both, returns true,
and closes stem-sharing x6's two cells against existing Stem 2348 / glyph 290
without vertex, edge, allocator, registry, or system-stem mutation. Native
reaches `current_index=102`, which is the queue length: every phase-1 head is
now carried natively. The five undefined LEFT sides (x32, x71, x70, x0, x31)
and the matching five-head queue remain recorded for Java's phase-2 append
retry, which stays unported.

Because order 101 has no successor, the v101 derivative also adds the first
probe change since v6: the continuation row's next-head fields are emitted as
`-` and the row terminates `ReturnedAfterFinalHead` instead of indexing one
past the queue. The base probe is unchanged and still hashes to
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`; the guard
lives in the per-version fragment, so every measured Java value is untouched.

The snapshot-minimized v101 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`229912dfd80b1b2cd02f296354df05238195fb9757ea53be99dfb70e22d62063`,
`2ba17be7dba32c36e3e43e41968c0e65b3d324e4805119983a20647f119c7628`,
`069566ce4388d08404effbbc9a9e16bb79f8f0dc56157c157eda0d5ab8a508bc`,
`9a158603465b69d1f9400b860a420d2942a96183727de6b1b4d35564ab44cf32`, and
`6ebe4cd6433e19b282d7f857657349a2865c955a971a425e56d648a990ecda21`;
base v100 runner/fixture remain `8c665c06c8b49b34c0d9b1b5375619946b75f95d4803c8ab960c2cb73858fb37`
and `aea95a084cac25e2984c848b9de386e047ab3dc9c4fdc3f1fb503d04fa76bc92`.
This is bounded order-101 existing-stem evidence and the phase-1 terminal; it
is not phase-2 append-retry, no-link/retry, broader geometry, or wider-corpus
coverage.

Boundary 127's commit `664185a6b` is the new remote CI baseline: Rust run
32324836388 passed all 12 shards and Build & Test run 32324836368 passed,
with no failure or cancellation. It supersedes `5f75f8708`, whose evidence
the earlier per-boundary sections above record as it stood at the time.

## Boundary 128: first heads-linking phase-2 append retry

Boundary 128 is the first boundary past phase 1. With the 102-head queue
exhausted, `StemsRetriever.linkStems` re-runs `HeadLinker.linkSides` with
`append=true` over `unlinkedHeads` - the heads whose phase-1 call returned
false. The v102 derivative extends the probe to build that list the way Java
does, from the phase-1 return values, and emits it: `queueSize 5`, queue
`[x32:sig50:id1389, x71:sig49:id1387, x70:sig46:id1377, x0:sig51:id1390,
x31:sig47:id1381]`. That is measured evidence for the five-head queue the
carrier has been holding since Boundary 76, and it supersedes the probe
field `unlinkedCount`, a hard-coded zero that never attested it.

The queue's first entry is a proven no-op. Both of x32's sides are still
open, so `append` never reaches the closed-side skip it exists to disable;
its LEFT corners still reach one shared stump, so Java takes the same
dual-corner branch as in phase 1, re-adds LEFT to an EnumSet that already
holds it, and returns false. SIG stays at 685 vertices / 706 edges, system
stems at 46, and both the relation-state and linker-state hashes are
identical across the call - the strongest available proof that this entry
mutates nothing - though Boundaries 129-132 show two later entries do move
the linker state.

Native models `append` where Java branches on it: `canLink` takes the flag
and applies `!append && cl.isLinked()` accordingly, and the carrier gains a
`phase_two_index` cursor into the queue. The retry fails closed on an
already linked or closed side, on a differing-stump dual corner, and on any
single-corner link, none of which this entry reaches.

The snapshot-minimized v102 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`f3ba43f3b9808b9e8303b180399f94e21af32490e416243ae9807c772debfa27`,
`47c6b327c89b67d67b2de1d5d0ae27e3aea9ea7652549a1ecc7a8b9956425b81`,
`2032ac7c09dc98572d4d587c7816d1c182b19b2f02871fea283a926462c1f13e`,
`09eab88d7cfcff76341f6e898cb95636fb91d27d78d440821141d0a9797999e1`, and
`8e3c544cbf0f09b703c64f739d492046537bd40b405debe67eebceff0de9e195`;
base v101 runner/fixture remain `2ba17be7dba32c36e3e43e41968c0e65b3d324e4805119983a20647f119c7628`
and `229912dfd80b1b2cd02f296354df05238195fb9757ea53be99dfb70e22d62063`.
This is bounded first-append-retry evidence, not the remaining four queued
heads, `reuseStem` appending, the rather-good profile escalation,
`finalizeStems`, or wider-corpus coverage.

## Boundaries 129-132: the rest of the phase-2 append-retry queue

The v103 derivative drives the whole queue and pins each entry, and the sweep
corrects an assumption Boundary 128 made from its single sample. Phase 2 is
not uniformly inert:

| entry | sides before | decisions | returned | closes |
| --- | --- | --- | --- | --- |
| x32 | both open | LEFT Both | false | - |
| x71 | LEFT linked | LEFT skipped, RIGHT TopOnly | true | x73 |
| x70 | LEFT linked | LEFT skipped, RIGHT TopOnly | true | - |
| x0 | LEFT linked | LEFT skipped, RIGHT Neither | true | x1 |
| x31 | both open | LEFT Both | false | - |

Three findings the sweep establishes. First, `append` genuinely changes
control flow: x71, x70 and x0 all have a closed-but-unlinked RIGHT, and
because the closed-side skip only applies when `append` is false, that side
is re-evaluated. For x71 and x70 `canLink` even returns TopOnly, so Java
calls `link` - the very path that reaches `reuseStem`. Both attempts fail
inside `expand`, which returns `-1` because the walk ends short of the hard
tail target, so nothing is built.

Second, two entries do mutate. x71 and x0 return true and then run the same
ordered closure over heads sharing their stems that a phase-1 return runs,
flipping two cells each: x71 closes x73, x0 closes x1. x70 writes the same
cells as x71 and flips none, because x71 already closed x73 on their shared
Stem 2382. SIG itself never moves - the relation-state hash is identical
across all five - but the linker-state hash advances twice.

Third, and the reason this matters beyond chula system 1: no append retry
ever links. `reuseStem`, the one behavior `append=true` exists to enable,
is therefore unreachable on this page and cannot be evidenced here at all.
It needs a system where an append retry succeeds.

Native reuses the phase-1 closure for the returning entries and decides the
failed link attempts with a bounded reading of `expand`, which fails closed
if a walk ever reaches the tail target - the point where `checkStemRelation`
and `reuseStem` would follow.

The snapshot-minimized v103 gate is focused/full/Clippy/fmt/diff green and is
not independent predecessor evidence. Fixture/runner/probe/body/semantic pins
are
`2c192a356f2aa9447c6b0bea5a5979086646390043b2b1333983038d5d3d03c4`,
`9714b52a8ce9941ef0832a3f17689fcbdeb5b14246546770cd001167be618b26`,
`20ef761cb178329156c19429a30473f61e83096253e418e9a64db5078dfb82e6`,
`494edf4c3590e0b73c27f156e9375ace75a3cc4c480a46be541cac15329b7c38`, and
`7eeb4ae6a77f2d6f7e7f99ccf70f38d916c1e0905a44cdf2151271abdb521ac0`;
base v102 runner/fixture remain `47c6b327c89b67d67b2de1d5d0ae27e3aea9ea7652549a1ecc7a8b9956425b81`
and `f3ba43f3b9808b9e8303b180399f94e21af32490e416243ae9807c772debfa27`.
Phase 2 is now carried end to end for chula system 1. `reuseStem`, the
rather-good profile escalation and wider-corpus coverage
remain open.

## Boundary 133: `finalizeStems`

The v104 derivative restores the retriever-owned `undefs` map and the
production reverse-grade `systemHeads` list, then invokes the private Java
`finalizeStems` method. Two fresh JVMs are byte-identical. Across all 102
heads, `checkHeadStems` finds zero heads with multiple live HeadStem relations,
so `HeadStemsCleaner` does not run. `checkNeededStems` finds only x32 / SIG 50
/ Inter 1389 and x31 / SIG 47 / Inter 1381 without a stem; both are void heads
and already abnormal. Java therefore removes no relation, adds no relation,
changes no abnormal flag, and leaves its 685/706 full SIG, 46 system stems,
allocator 2385, and all structural hashes unchanged.

`finalize_native_stems` independently derives that census from the completed
carrier. It authenticates the exhausted phase-1 and phase-2 cursors, exact
undefined/unlinked queue, 267/370 native STEMS graph projection, all 46 known
stems, every head binding, every HeadStem incidence, and the two pre-existing
abnormal flags. Any multi-stem cleaner candidate or abnormal-state drift fails
closed; the authenticated result owns an unchanged carrier.

Fixture/runner/probe/body/semantic SHA-256 values are
`ad1cf4658b6d4f7f30732681f514fe85f6801d5efce2f9629b9347cf513fe8e5`,
`a36fe02337e974fdbb1119d087026d515020fb319fd7d138e4057e37cabfc639`,
`eb3076ccb85a91f032fe8425ed224a12d9aac66a83533e330436567034efb6b3`,
`5e749de69be552e0446b5a530f7cab9eb3e7fcb05211b6dd99a51eddd4d5fc46`, and
`ee5b0fff2387f4ea4b6c5aaa20835cd07af4619f96be0dc33b3faf780e323669`.
Strict base-v103 runner/fixture remain
`9714b52a8ce9941ef0832a3f17689fcbdeb5b14246546770cd001167be618b26`
and `2c192a356f2aa9447c6b0bea5a5979086646390043b2b1333983038d5d3d03c4`.
Boundary 134 below supersedes that corpus restriction while retaining this
strict Chula specialization.

## Boundary 134: generic `finalizeStems`

`finalize_native_stems` now owns both finalizer passes for every completed
native carrier. It reproduces `SIGraph.getPartitions(null, stems)` from live
stem exclusions in reverse stem-grade order, repeatedly removes the strict
lowest `stem.grade * (HeadStemRelation.targetRatio - 1)` contribution, and
derives the target ratio exactly as `1 + 10 * relation.grade`. Two surviving
relations are kept only for Java's physical canonical share: dy at most 0.2,
one LEFT and one RIGHT relation, the head center between the two stem
midpoints, and TOP/BOTTOM anchor portions computed from the carried medians
and the 0.275 head-height margin. Relation-removal callbacks recompute both
head and stem abnormal state; `checkNeededStems` then marks every remaining
stemless carrier head abnormal. The transaction is clone-first and fails
closed on incomplete cursors, invalid SIG/bindings, missing relation payloads,
nonfinite grades, or missing stem medians.

The new Java corpus fixture records two real mutations: Allegretto system 1
removes lower-contribution same-side Stem 2240 from head 1317, and Zizi system
2 removes the lower-contribution LEFT stem from head 1183. Five Zizi
LEFT/RIGHT pairs are retained as canonical shares. A controlled reset of
Chula's stemless head 1389 proves `checkNeededStems` restores `false -> true`.
Warmup plus two fresh executions are byte-identical. Fixture, runner, probe,
init-script, and semantic-body SHA-256 values are
`d468cb52f59687604d2204b18aa2364bde12355cb476d007ce205788033b350a`,
`ddcaa94b847de8ed50ffdb9e866717da3e888e223117d8453bf06db55ebaa247`,
`f55cc3fe1f8dc85d817ba84499e407dc759f6710cd815b0eb8007bfca02ac0b1`,
`538f75284a798d4cf96e7f4034bf5368e63f50891f58b712b517fe84f6223006`,
and `6d706ff6e8dc4fc63bb580447b91ddb114d6f0f56544b2902b50438d93d09664`.
The focused carrier gate additionally forces a three-relation pruning loop,
an abnormal mutation, and a synthetic physical canonical keep over fully
authenticated native state.

## Boundary 135: production STEMS preparation

`native_stems::prepare_native_stems` now owns the complete immutable composition
immediately before the first mutating SIDES transaction. From completed live
GRID, HEADERS, STEM_SEEDS, BEAMS, LEDGERS, and HEADS products it materializes
head corners and seeds, beam stumps/VLinkers/reachability/builders, head
stumps/reachability/builders, beam link plans and scheduler frontiers, then
assembles the mutable native SIG in Java order. The companion
`materialize_native_stems_components` exposes the read-only chain for wider
pages whose upstream BEAMS-group SIG remains incomplete. Both APIs fail without
returning partial state.

The Chula exact carrier test now reaches its predecessor through the production
entry rather than a test-local composition chain. Focused 1/1 and full 14/14
sibling gates pass; strict workspace all-target/all-feature Clippy, formatting,
and diff checks are clean. This closes only the composition seam. The disclosed
first-STEMS persistent snapshot is removed operationally by Boundary 136; the
sparse selected-base identity bridge and wider mutation/corpus branches remain.

## Boundary 136: native STEMS glyph identity

`NativeStemsModeledGlyphRegistry` replaces the disclosed page-wide first-STEMS
snapshot as the operational glyph authority from transaction 3 onward. It owns
the exact system-visible modeled prefix, assigns stable native identity as
canonical ordinal plus one, and resolves only full bounds/weight/RunTable
equality. Java's allocator/union watermark and 592 opaque fingerprint-only
entries are not imported.

The same typed authority now drives SIDES transactions 2-32, all
STUMPS transactions, all measured head C-links, and every existing-stem retry.
Hard-coded Java glyph numbers are no longer runtime join keys; carried
StemInter identity plus native glyph content authenticate reuse. Focused 1/1
(17.34s), full sibling 14/14 (153.26s), strict workspace all-target/all-feature
Clippy (23.72s), formatting, and diff checks pass.

Transaction 1's legacy bootstrap, the sparse selected-base Java InterIndex
bridge, and the reconstructed Allegretto predecessor remain. The legacy
first-STEMS fixture/API is retained only for those isolated compatibility gates.

## Boundary 137: native transaction-2 glyph bootstrap

Transaction 2 now prepares plan 152 from `NativeStemsModeledGlyphRegistry`
before any transaction-2 expected fixture is opened. The integration gate has
deleted its parser/read of `stems-beam-glyph-registry-chula.txt`; exact modeled
content supplies the identity, while the existing native graph-derived B13/B14
and B15-B19 path preserves every measured result.

Focused 1/1 (13.84s), full sibling 14/14 (149.59s), strict workspace
all-target/all-feature Clippy (12.30s), formatting, and diff checks pass.
Transaction 1's compact fixture state and the sparse selected-base Java
InterIndex bridge remain.

## Boundary 138: native transaction-1 B12/B13 bootstrap

The first shared-sheet frontier now begins with
`initialize_native_stems_beam_vlink_first_frontier_state_from_modeled_registry`.
The live scheduler/plan and 1,058-entry native modeled registry derive its two
selected bindings, V-linker line state, native canonical identity, and complete
empty `systemStems` authority. No 1,650-entry Java GlyphIndex union, opaque
fingerprint suffix, or candidate equality scan is imported. The exact reused
candidate becomes native glyph 45 instead of Java glyph 294 with identical
content and checker geometry.

B13 projects the all-unlinked read set from the owned SIG and persistent S
cells, and native glyph identity remains carried through B14 and the complete
terminal chain. Focused 1/1 and full sibling 14/14 (147.63s) pass; strict
workspace all-target/all-feature Clippy passes in 26.25s; formatting and diff
checks are clean. The shared persistent allocator and sparse selected-base Java
InterIndex bridge remain the next identity authorities to remove.

## Boundary 139: native selected-beam identity

`roll_native_stems_beam_vlink_base_apply_state` now resolves each selected B14
beam directly from `NativeSigSystemBindings`. The owned one-based native vertex
identity is its persistent identity, the native vertex ordinal is its local
InterIndex order, and VIP is false in this native domain. Production no longer
accepts or stores `NativeStemsBeamBeamInterIndexBootstrapEntry`, and the gate no
longer reads `stems-beam-inter-index-chula-system1.txt`.

The same 16 distinct selected beams remain an asserted semantic result across
all 32 SIDES transactions rather than an execution input. A missing native beam
binding rejects before mutation. Focused 1/1 and full sibling 14/14 (154.47s)
pass; strict workspace all-target/all-feature Clippy passes in 27.70s;
formatting and diff checks are clean. The first B14 compact state's shared
persistent-ID seed and opaque InterIndex baseline are the next identity seam.

## Boundary 140: native first-B14 compact state

`initialize_native_stems_beam_vlink_base_apply_state_from_native_sig` now
derives the first B14 graph, endpoint, beam-group, certificate, and local
InterIndex state from the owned SIG and bindings. Native insertion order is the
local InterIndex domain: the initial baseline is 221 native vertices instead of
Java's opaque 639-entry sheet index, and after three carried transactions it is
223 rather than 641. No B14 compact graph/index snapshot is an execution input.

All mutations and downstream results remain exact. Focused 1/1 and full sibling
14/14 (150.25s) pass; strict workspace all-target/all-feature Clippy passes in
32.58s; formatting and diff checks are clean. Only the shared persistent-ID
counter remains as a first-B14 identity input.

## Boundary 141: native STEMS persistent identities

The first transaction seeds its shared native identity domain immediately
after the 1,058 modeled glyphs. StemInter identities therefore allocate from
1,059 through 1,104 instead of inheriting Java's 2,339 EntityIndex watermark.
The initializer takes no persistent-ID argument, and continuation guards use
the carried native `stem_identity` rather than Java Inter IDs.

The complete 102-head path and generic finalizer remain exact; terminal sheet,
glyph-index, and inter-index counters all equal 1,104. Focused 1/1 and full
sibling 14/14 (152.10s) pass; strict workspace all-target/all-feature Clippy
passes in 29.78s; formatting and diff checks are clean.

## Boundary 142: production-derived modeled-registry boundary

`NativeStemsModeledGlyphRegistry::from_head_builder_recognition` derives the
requested system's visible canonical-glyph prefix from the final production
head-builder registry event and validates it against the complete modeled
canonical collection. The carrier gate no longer passes a separate visible
count into production; that independently retained count only checks the
derived registry length.

Focused 1/1 and full sibling 14/14 (150.54s) pass; strict workspace
all-target/all-feature Clippy passes in 31.23s; formatting and diff checks are
clean. The next slice is a production first-transaction carrier initializer,
then native Allegretto transactions 1-27 and the measured linked-S/hook-removal
frontier.

## Boundary 143: atomic first SIDES carrier

`initialize_native_stems_beam_sides_carrier_from_modeled_registry` executes the
first B12-B19 SIDES transaction against local SIG, binding, B-cell, and S-cell
state and returns no carrier on failure. It initializes the native first-B14
state, commits the base/sibling/head mutations, resumes the scheduler, then
reconciles and validates every carried authority. The first-frontier state
initializer now accepts any consistently identified system.

The independent Chula reconstruction matches the returned carrier and trace;
subsequent transactions consume the production result. Focused 1/1 and full
sibling 14/14 (157.64s) pass; strict workspace all-target/all-feature Clippy
passes in 9.33s; formatting and diff checks are clean. Next is native Allegretto
carriage through transactions 1-27.

## Boundary 144: native Allegretto linked-S and hook-removal carriage

The production carrier executes Allegretto system 1 transactions 1-28 from
the modeled registry, derives transaction 28's existing-Stem selection from
the owned SIG/S cells, and reaches the typed competing-hook checkpoint with
the two BeamStem incidences created by earlier native transactions. Hook
removal consumes that natural five-edge neighborhood and resumes to
`SidesExhausted`; the tests no longer reconstruct scheduler state or append
artificial Stem vertices and edges.

The generic fixes retain only persistent line state from completed one-shot
line-delta evidence, select callback rules from the live beam runtime class,
roll B14 onto either fresh or graph-bound existing Stems without conflating
persistent and SIG identities, and let B17 recompute abnormality for an
existing normal Stem. Focused linked-S and hook tests pass; the full sibling
suite passes 14/14 in 160.24s; strict workspace all-target/all-feature Clippy
passes in 18.57s; formatting and diff checks are clean. Production relation
parameters are still hydrated from the strict fixture, and wider-system
SIDES/STUMPS/head branch carriage remains open.

## Boundary 145: production-derived BeamStem relation parameters

`NativeStemsBeamRelationParameters::from_native_products` derives interline
and main Stem thickness from native plan/V-linker products, combines them with
the authenticated frontier profile and the ported Java relation constants,
and rejects incoherent systems. The carrier context no longer exposes a
relation-parameter input, eliminating the last strict-fixture value used by
Chula and Allegretto transaction execution.

Both gates compare the derived product with the frozen Java context only after
derivation, and all graph/scheduler results remain unchanged. Focused Chula,
linked-S, and hook-removal tests pass; the full sibling suite passes 14/14 in
159.65s; strict workspace all-target/all-feature Clippy passes in 24.34s;
formatting and diff checks are clean. Wider system carriage is next.

## Boundary 146: production-owned STEMS entry edit state

`NativeStemsBeamSheetEditState::at_stems_entry` internalizes the established
STEMS-entry state: prior graph-building has already marked the sheet stub,
book, and book dirty. The first-carrier initializer no longer accepts these
three flags from callers. Chula and Allegretto match their former strict B14
entry state and all carried results remain unchanged.

Focused Chula and Allegretto hook gates pass; the full sibling suite passes
14/14 in 157.49s; strict workspace all-target/all-feature Clippy passes in
22.85s; formatting and diff checks are clean. Wider system carriage is next.

## Boundary 147: production-owned checker and first-system SIDES start

`prepare_native_stems` now constructs and owns the page-wide
`NativeStemsBeamStemCheckerContext` from live GRID and STEM_SEEDS products:
the `NO_STAFF` raster, scale interline, accepted maximum Stem thickness,
ties-to-even `0.15 * interline` belt margin, sheet skew, Java's exact
`0.8 * 0.1` minimum-grade product, and the `0.4` artificial-Stem grade.
`NativeStemsPreparedRecognition::initialize_first_system_sides` joins the
first scheduler frontier to the matching plans, builders, stumps, VLinkers,
reachability, head corners, native SIG/bindings, and modeled-glyph registry,
then returns the registry, carrier, and committed first B12-B19 transaction
atomically. Callers can no longer hydrate the checker or mix system-local
products at this production seam.

The new third-page gate starts Batuque system 1 at plan 98 from that production
state. It reuses active glyph 265, creates the checked Stem at grade bits
`0x3fe91480f4111904`, applies the base BeamStem support grade bits
`0x3feefb1fb84ea5fd`, links the one Java-measured sibling, commits the aggregate
base-plus-sibling two B-cell writes, inserts three HeadStem relations/S-cell
writes, performs the idempotent outer B write, and reaches plan 111. Frozen
Java rows are assertions only and supply no execution input. The focused gate
passes 1/1 in 4.47s; the full sibling suite passes 15/15 in 166.67s; strict
workspace all-target/all-feature Clippy passes in 25.18s; formatting and diff
checks are clean.

This helper is deliberately first-system-only. Starting a later system still
requires the shared allocator and modeled registry after every prior system's
complete transaction chronology; reconstructing system 2 independently is
rejected rather than guessed. Carrying that cross-system state, then widening
SIDES/STUMPS/head branches, remains next.

## Boundary 148: production first-system SIDES drive

`NativeStemsPreparedRecognition::drive_first_system_sides` moves the complete
system-1 transaction loop out of the sibling test and into production. Starting
from Boundary 147's owned checker, registry, SIG, bindings, and first committed
transaction, it repeatedly invokes the generic modeled-registry advance and
returns only when the scheduler reaches true `SidesExhausted`.

The immutable builder count is a strict progress bound. Empty builders, a
competing-hook checkpoint, an unexpected STUMPS terminal, or exhaustion of that
bound reject the whole drive rather than exposing a guessed partial carrier.
Batuque system 1 executes 33 transactions and ends at 222 vertices / 263 edges,
32 Stem bindings, 51/93 linked B cells, 71/186 linked S cells, and 24 retained
beams with retained and final worklists identical; every B/S cell remains open.
The first transaction retains Boundary 147's exact Java assertions. The new
terminal vector proves the production loop over already graded components and
does not claim a new full-chain Java snapshot.

Focused Batuque passes 1/1 in 3.76s; the full sibling suite passes 15/15 in
159.88s; strict workspace all-target/all-feature Clippy passes in 23.77s;
formatting and diff checks are clean. The method remains first-system-only;
cross-system allocator/registry/carrier chronology was therefore next.

## Boundary 149: exact cross-system registry and allocator handoff

`NativeStemsModeledGlyphRegistry::carry_into_next_system` combines the complete
system-1 modeled prefix with every exact canonical learned by its completed
transaction state, then replays system 2's constructor registrations in order
using full bounds/weight/RunTable equality. Selection now resolves by exact
content, not the precomputed modeled ordinal that interleaved StemInter
allocations invalidate as a page identity.

The handoff rejects isolated or nonconsecutive state, alias/content collisions,
weak-only originals with unknown Java liveness, and union counts not covered by
the carried exact entries. The single persistent allocator advances only for a
structural miss, with sheet/GlyphIndex/InterIndex views kept equal, and no
caller-owned state is mutated on failure.

Batuque system 1 keeps 1,058 structural glyphs while 32 Stem inter allocations
move the allocator from 1,058 to 1,090. System 2's 1,125 constructor events
finish at 1,470 structural glyphs and allocator 1,502. An isolated system-2
prefix also has 1,470 glyphs but incorrectly ends its allocator at 1,470; the
gate pins the registries as unequal and preserves the 32-ID interleaving gap.
Weak-liveness and incomplete-union corruptions reject atomically.

Focused Batuque passes 1/1 in 3.78s; the full sibling suite passes 15/15 in
157.08s; strict workspace all-target/all-feature Clippy passes in 8.66s;
formatting and diff checks are clean. System-2 SIG/bindings/cells and the first
serial SIDES carrier were therefore the next boundary.

## Boundary 150: first shared-sheet serial SIDES carrier

`NativeStemsPreparedRecognition::initialize_second_system_sides` drives system
1 to `SidesExhausted`, performs the exact Boundary 149 registry/allocator
handoff, selects system 2's native products, and executes its first B12-B19
transaction as one returned value. The serial transaction state uses fresh
system-local SIG, bindings, `systemStems`, and linker cells while carrying the
page edit state and the shared persistent-ID stream.

Batuque system 2 enters with registry 1,470 / allocator 1,502. Plan 514,
builder 105, profile 4 creates checked stem identity 0 and advances the
allocator to 1,503. The committed carrier has union size 1,470, one known
canonical, one system stem, a 240/199 SIG, one stem binding, 117 B cells, and
244 S cells. Its scope is exactly `SharedSheetSerial`, never an isolated later
system.

This is native composition of already graded boundaries, not a new Java
full-chain snapshot. Focused Batuque passes 1/1 in 3.78s; the full sibling
suite passes 15/15 in 158.54s; strict workspace all-target/all-feature Clippy
passes in 25.68s; formatting and diff checks are clean. The remaining system-2
SIDES drive and wider later-system branches were therefore next.

## Boundary 151: complete Batuque system-2 SIDES drive

The production driver is now system-generic: Boundary 150's first serial
transaction continues under the immutable builder-count bound until a typed
SIDES terminal. Any hook-removal checkpoint, STUMPS completion, malformed
frontier, or bound exhaustion rejects the entire returned drive.

Batuque system 2 completes 40 `SharedSheetSerial` transactions. The allocator
advances 1,502→1,542, the system records 40 stems, and the terminal SIG/binding
counts are 279/349/40. Linked cells are exactly B 64/117 and S 89/244, all open;
the 33 retained STUMPS entries equal the final local worklist.

Focused Batuque passes 1/1 in 4.15s; the full sibling suite passes 15/15 in
160.94s; strict workspace all-target/all-feature Clippy passes in 24.43s;
formatting and diff checks are clean. System-3 handoff and later-system STUMPS
were therefore next.

## Boundary 152: complete three-system Batuque SIDES page

The page driver carries each terminal registry/allocator into the next
system's constructors and serial frontier, returning no partial vector on
failure. System 3 additionally closes the first rejected-head-target subset in
B17 and the first two-glyph compound `registerOriginal` path from native page
registry contents.

Batuque completes 101 SIDES transactions over systems 1-3. System 3 starts at
1,819 glyphs / allocator 1,891, registers glyph 1,915, and ends at union 1,820 /
allocator 1,920 with 28 stems, SIG 244/257, B 50/101, S 63/224, and 25 retained
STUMPS entries. Weak-liveness uncertainty remains a typed refusal.

Focused Batuque passes 1/1 in 4.07s; the full sibling suite passes 15/15 in
160.98s; strict workspace all-target/all-feature Clippy passes in 20.36s;
formatting and diff checks are clean. Wider-system STUMPS is next.

## Boundary 153: production Batuque system-1 STUMPS completion

`NativeStemsPreparedRecognition::drive_first_system_stumps` now carries the
production-prepared Batuque system-1 SIDES terminal through the retained stump
worklist and returns only after the typed post-STUMPS terminal. The eight
transactions finish with allocator 1,098, 40 known/bound Stems, SIG 230/297,
67/93 linked B cells, and 89/186 linked S cells; the 24 retained beams remain
identical to the final local worklist.

The first wider-corpus rollover exposed a valid B14 predecessor shape that the
old Chula-only validator rejected: an existing `StemInter` contributes zero
new InterIndex entries and zero new SIG vertices but still contributes its one
BeamStem edge. Rollover now authenticates both legal 1/1/1 fresh-stem and 0/0/1
existing-stem shapes against the live native SIG, including dense counts,
bindings, persistent membership, the recorded base edge, and its native
origin. Later B16/B17 edges may follow that base edge and later callbacks may
revise abnormality, so neither is mistaken for predecessor corruption. A
tampered reused-stem edge rejects on the shadow carrier with no partial batch
commit.

The same run also closed a scheduler-authority mismatch: Java's B16 sibling
write can select a B-linker exposed by a `StemBuilder` item even when that
linker has no standalone V-linker constructor. STUMPS resume now authenticates
such sibling assignments against the builder item catalogue actually queried
by B16, while still rejecting target repetition, duplicates, and unknown
references.

Focused Batuque passes 1/1 in 3.95s; the full sibling suite passes 15/15 in
151.29s; strict workspace all-target/all-feature Clippy passes in 19.98s;
formatting and diff checks are clean. Systems 2-3 STUMPS and the page-serial
registry/allocator handoff after each STUMPS terminal are next.

## Boundary 154: complete three-system Batuque STUMPS page

`NativeStemsPreparedRecognition::drive_all_system_stumps` now composes both
beam-origin passes in true page order. Each next system is constructed only
after the preceding system's STUMPS terminal has updated the shared modeled
registry, persistent allocator, and edit state; the older SIDES-only page
driver remains an explicit diagnostic boundary and is not reused here. A
failure in any system returns no partial page vector.

Batuque completes 42 STUMPS transactions across systems 1-3 (8 + 14 + 20).
The carried registry/terminal tuples are, respectively: system 1
1,058/allocator 1,098 with 40 stems and SIG 230/297; system 2
1,470/registry allocator 1,510/final allocator 1,564 with 54 stems and SIG
293/406; and system 3 1,819/registry allocator 1,913/final allocator 1,962
with 48 stems and SIG 264/339. Systems 2-3 retain `SharedSheetSerial` scope,
and every scheduler finishes with its retained and final local worklists
identical.

Focused Batuque passes 1/1 in 4.20s; the full sibling suite passes 15/15 in
154.38s; strict workspace all-target/all-feature Clippy passes in 22.22s;
formatting and diff checks are clean. The next page-wide seam transfers each
post-STUMPS carrier into head linking, then closes wider head/retry branches
before `recognize_native_stems` composition.

## Boundary 155: enter page-wide Batuque head linking

`begin_all_system_head_linking_phase1` atomically transfers all three Batuque
post-STUMPS carriers into the generic phase-1 head driver. It authenticates
and closes native prelinked prefixes of 7, 79, and 48 heads across queues of
93, 122, and 112, then returns the first actual C-link frontier for every
system: `(staff,head,SIG,x)` `(1,30,84,56)`, `(1,57,115,108)`, and
`(1,57,105,110)`. All three select only LEFT/BOTTOM and carry no unlinked or
undefined head.

The prefix trace is distinct from C-link mutation and every closure is derived
from the live SIG, stem bindings, and persistent S cells. The first/last
prefix references are `(1,28,82,4)` / `(1,24,78,86)`, `(1,18,76,13)` /
`(1,38,96,84)`, and `(1,61,108,6)` / `(0,15,14,32)`. Dual-corner and
rather-good retry/no-link cases remain typed fail-closed boundaries.

Focused Batuque passes 1/1 in 4.30s; the full sibling suite passes 15/15 in
152.96s; strict workspace all-target/all-feature Clippy passes in 23.13s;
formatting and diff checks are clean. Next is generic consumption of these
three C-link frontiers and carriage of the remaining page-wide head queues.

## Boundary 156: execute the first page-wide head outcomes

The production page driver now retains accepted STEM_SEEDS free glyphs inside
`prepare_native_stems` and consumes all three first HEADS frontiers without
caller-supplied Java identities or expansion indices. Systems 1 and 3 create
one native Stem and HeadStem edge at x56 and x110, advancing to indices 8 and
49 with SIG 231/298 and 265/340.

System 2 takes a normal rejected-link path: its 18-pixel start item is short
of Java's 37-pixel hard tail. The generic `linkSides` loop retries eligible
profiles and both sides, closes both S cells, records one phase-2 head, and
advances to index 80 while SIG 293/406 remains unchanged. Both created- and
existing-stem expansion now measure the hard target from the corner reference
point, matching `CLinker.link`, rather than the theoretical-line endpoint.

Focused Batuque passes 1/1 in 4.48s; the full sibling suite passes 15/15 in
154.01s; strict workspace all-target/all-feature Clippy passes in 19.10s;
formatting and diff checks are clean. Next are the remaining page head queues,
wider expansion/reuse, and phase-2 append retries.

## Boundary 157: carry every page system to its next head frontier

The production continuation loop now advances each Boundary-156 carrier
through graph-derived prelinked closures and defined false results until the
next actionable frontier. System 1 carries 18 continuations to index 25 at
`(staff,head,SIG,x)=(1,34,88,76)`. System 2 remains at index 80 before
`(1,63,121,109)` with its phase-2 retry head intact. System 3 remains at
index 49 before `(0,47,46,111)`. All select LEFT/BOTTOM.

This boundary is mutation-free: SIG, allocator, registry, and Stem state are
unchanged after the mixed first outcomes. Focused Batuque passes 1/1 in
4.82s; the full sibling suite passes 15/15 in 163.20s; strict workspace
all-target/all-feature Clippy passes in 25.56s; formatting and diff checks are
clean. The next boundary consumes these three C-link frontiers.

## Boundary 158: execute the second page-wide head outcomes

The generic page transaction now accepts carried phase-2 retry/undefined
authority and consumes x76, x109, and x111 without early-Chula queue-index or
empty-retry assumptions. Systems 1 and 3 each create one Stem vertex plus one
HeadStem edge, reaching index/SIG 26/232/299 and 50/266/341. System 2 takes a
second normal rejected-link closure at x109, reaches index 81 with two ordered
phase-2 retry heads, and leaves SIG 293/406 unchanged. Prior continuation
traces remain 18/1/1.

Focused Batuque passes 1/1 in 5.09s; the full sibling suite passes 15/15 in
162.53s; strict workspace all-target/all-feature Clippy passes in 25.76s;
formatting and diff checks are clean. Next is continuation to the following
page action frontiers.

## Boundary 159: complete three-system Batuque head phase 1

The production driver now alternates continuation and action until all three
reverse-grade queues are exhausted, with a strict twice-head-count event
bound. Generic existing-stem reuse appends only the HeadStem edge and S-cell
closures. Dual-corner choice now preserves same-stump undefined sides and
uses Java's BOTTOM-on-LEFT / TOP-on-RIGHT standard connection otherwise.

Terminal `(heads,prefix,events,continuations,creates,reuses,direct-no-link,
retry,undefined,SIG,stems,allocator)` tuples are:

- system 1: `93,7,89,85,2,2,0,0,0,232/301,42,1100`;
- system 2: `122,79,44,42,0,0,2,2,0,293/406,54,1564`;
- system 3: `112,48,69,63,4,1,1,2,2,268/344,52,1966`.

Every carrier finishes consumed with phase-two index zero. Focused Batuque
passes 1/1 in 5.11s; the full sibling suite passes 15/15 in 156.59s; strict
workspace all-target/all-feature Clippy passes in 25.06s; formatting and diff
checks are clean. Page-wide phase-2 append retry is next.

## Boundary 160: complete Batuque head phase 2 page-wide

`drive_all_system_head_linking_phase2` starts from the atomic phase-1 page
drive and consumes each system's native-carried retry queue on local shadows.
The generic queue validation no longer imports Chula head ordinals. It checks
cursor bounds, unique/resolvable queue heads, the closed-side invariant for
direct no-link heads, and unique/resolvable undefined S cells before any retry.

Batuque system 1 has no queue. System 2 retries x108/SIG115 and x109/SIG121;
their profile-0 decisions are BottomOnly/Neither and BottomOnly/TopOnly, and
both return false after expansion stops before the unsupported real
`reuseStem` append. Their sides were already closed, so four ordered close
attempts change zero values. System 3 retries x107/SIG47 and x108/SIG2. The
first reaches the standard LEFT/BOTTOM choice before a RIGHT shared-stump
undefined return; the second returns on its LEFT shared stump. They add no
close writes. All four preserve SIGs `232/301`, `293/406`, `268/344`, system
stem counts `42/54/52`, and the native page allocator/registry state.

The dedicated Java page probe runs real Batuque SIDES/STUMPS and both head
phases, not a transformed Chula prefix. Warmup plus two fresh passes are
byte-identical. Fixture, runner, probe, init, and emitted-body SHA-256 are
`41992cf6702bc27b918733e6a1a097c22b729c6dfc7fe332e8603c5e6a02983a`,
`b0e79187886052aa20ac15421da2eb5169d541b305ef0f04460dfc05add094d6`,
`7b467c57b65e57aa052296164129ae8c016d82756c9f804d8e1072747b0a76b2`,
`1defbc545668eb711395283bc0d8f9216b7402ad3b0f2f64f93812ac739c495e`,
and `3d30e22eca5ee67647519fed576083a66ed987bd8803376e72c5462f5758d021`.
Focused Batuque passes 1/1 in 5.51s; the full sibling suite passes 15/15 in
152.69s; strict workspace all-target/all-feature Clippy passes in 20.10s;
formatting and diff checks are clean. Page-wide `finalizeStems` is next.

## Boundary 161: finalize Batuque STEMS page-wide

`finalize_all_system_stems` composes the atomic page phase-2 drive with the
generic `finalize_native_stems` transaction for each system. Every system is
finalized on a local shadow and the page result is exposed only after all three
transactions succeed, so a malformed late system cannot publish a partial
page.

The fresh Java probe runs real Batuque SIDES, STUMPS, both head phases, and the
private `finalizeStems` method in foreground page order. System 1 checks 93
heads and finds no abnormal head. System 2 checks 122 heads and preserves
x108/SIG115 and x109/SIG121 as no-stem abnormal heads. System 3 checks 112
heads and preserves x107/SIG47 and x108/SIG2 as no-stem abnormal heads, with
the previously carried RIGHT and LEFT undefined sides. No system has a
multiple-stem head, removed HeadStem relation, abnormal-value transition,
allocator change, SIG mutation, or system-Stem mutation. Terminal graph/stem
counts remain `232/301/42`, `293/406/54`, and `268/344/52`.

Warmup plus two fresh Java passes are byte-identical. Fixture, runner, probe,
init, and emitted-body SHA-256 are
`ab6377a2b82cc838633b8c0d79732ddd755a68f11a8b7e40dd39baee7d6278d2`,
`7e8b8c557d1d321329c72e62cdd932e0faa304591e14b958171ff7a961342ea1`,
`9b5e9dbefbf400887f49feba934c573d851c67e65b3e43bfaabc86d6f2c36714`,
`e0ff89792bf75286317ef011e079f338696d29cc14918f4a3018307ba4ed9548`,
and `e51e06eb798e3ab6ccaa32ea5db5b88f6285b667fb8162e1777a0faf6c28a3a1`.
Focused Batuque passes 1/1 in 14.17s; the full sibling suite passes 15/15 in
156.66s; strict workspace all-target/all-feature Clippy passes in 19.88s;
formatting and diff checks are clean.
Transactional `recognize_native_stems` is next.

## Boundary 162: transactional `recognize_native_stems`

`recognize_native_stems` is now the owned production entry point for the whole
stage. It accepts only completed GRID, HEADERS, STEM_SEEDS, BEAMS, LEDGERS, and
HEADS products, builds the immutable construction products and mutable native
SIG, drives page SIDES/STUMPS, both head-linking phases, and generic
`finalizeStems`, and returns `NativeStemsRecognition` only after every system
has finalized. The result retains the construction products plus each
system's terminal SIG, native registry, phase-1 trace, retry trace, and
finalization transaction.

The Batuque gate invokes both the independently stepped page path and the new
one-call entry point from the same live upstream products. Their complete
component recognition and three finalized system results compare equal. The
strict Boundary-161 Java fixture remains the external grader, so this boundary
adds no transformed or assumed oracle. Focused Batuque passes 1/1 in 13.80s;
the full sibling suite passes 15/15 in 142.75s; strict workspace Clippy passes
in 20.01s; formatting and diff checks are green. Schema-1
ordinary/stream publication is next.

## Boundary 163: schema-1 STEMS publication

The native CLI now accepts `-step STEMS -json` and composes GRID, HEADERS,
STEM_SEEDS, BEAMS, LEDGERS, HEADS, and transactional STEMS in Java stage order.
`stems_json` keeps every prior schema-1 field and adds one stage-owned `stems`
object with terminal per-system counts, accepted Stem geometry/thickness/grade,
live HeadStem payloads, multiple/no-stem/abnormal head sets, and undefined
sides. Native stem identities and SIG ordinals are explicitly system-scoped;
the document never labels them as Java IDs.

Batuque publishes 148 final Stems, 323 HeadStem relations, 327 checked heads,
and four abnormal no-stem heads. Ordinary JSON equals the STEMS stream snapshot
byte-for-byte, and the marker sequence is GRID, HEADERS, STEM_SEEDS, BEAMS,
LEDGERS, HEADS, STEMS. The full CLI suite passes, including the 17.63s live
ordinary/stream gate; all 11 report tests pass; strict workspace Clippy passes
in 12.06s; formatting and diff checks are green. Wider-corpus branch
coverage is next.



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
