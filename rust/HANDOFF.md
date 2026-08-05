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

The port performs native page recognition through GRID. `audiveris-cli -batch
-step SCALE|GRID <image>` runs `LOAD -> BINARY -> SCALE -> GRID` on a real
raster, with every threshold derived from the measured sheet scale.

Against a live Java 5.11 oracle across all nine `data/examples` pages:

| Output | Status |
| --- | --- |
| Binary raster | 9/9 pages bit-identical |
| Staff abscissae | 65/65 exact |
| Barline abscissae | 420/420 exact |
| Completed staff-line endpoints | 1300/1300 exact |
| Sheet SIG | all 420 barline inters and 184 connectors promoted; 7 median and 21 grade residuals remain, diagnosed below |
| JPEG decoding | bit-exact against the libjpeg Audiveris bundles, on 130 fixtures and 140 sampling combinations |

`cargo fmt --all --check`, strict Clippy, and `cargo test --workspace` are green.

## Open threads, in the order worth taking them

### 1. Staff-line filament assembly (FIXED)

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

What is left is 6 grade residuals across four pages, and they are **not** the
oracle's own precision -- Java persists grades to three decimals, which
`SIG_GRADE_PRECISION` already absorbs at 5e-4, so a 0.0036-0.0048 delta sits an
order of magnitude above the artifact floor.

They are also not scattered. Of 420 barlines across 65 staves, the six that
differ are **every one of them the leftmost or rightmost barline of its staff**,
and no interior barline differs anywhere in the corpus:

```
BachInvention5.jpg  staff  1 x 1917.0   bars  194..1917 (3)  RIGHTMOST
D0392410-1.256.png  staff  8 x  269.5   bars  270..2774 (6)  LEFTMOST
D0392410-1.256.png  staff 10 x 2773.0   bars  268..2773 (6)  RIGHTMOST
carmen.png          staff  3 x 2402.5   bars  166..2402 (7)  RIGHTMOST
carmen.png          staff 10 x 2412.5   bars  173..2412 (8)  RIGHTMOST
cucaracha.png       staff  6 x  156.0   bars  156..2399 (7)  LEFTMOST
```

Each is an intrinsic-grade difference with the contextual grade following it, so
there is one cause: something in the staff-vertical impacts measures differently
at a staff's extreme abscissa. The prime suspect is the chunk or serif lookup,
which is the one part of the peak measurement reaching past the staff's own line
data into extrapolated geometry -- the same family as the bug above, via a
different consumer. `bars_logic.rs` around the `half_line` serif bounds is where
to start, and `AUDIVERIS_DEBUG_PURGE`-style per-impact tracing on both runtimes
is the way to close it.

### 2. PDF ingest (uncertain piece done; decoders next)

Audiveris renders PDF pages through PDFBox with
`renderImageWithDPI(page, 300, ImageType.GRAY)` under `ANTIALIAS_OFF` and
`INTERPOLATION_BICUBIC` (`ImageLoading.PdfboxLoader.getImage`). So this is
reproducing a rasterizer, not writing a set of decoders, and the sequencing was
to settle the rasterizer first.

**That is done.** `crates/audiveris-pdf` reproduces Java2D's bicubic image
transform bit for bit: 7 geometries by 8 scales, including the three real IMSLP
ratios, upscale, downscale, identity and 1x1 -- 112 of 112 cases identical,
pinned by `oracle/java2d-bicubic.txt` from `oracle/java/Java2dBicubicProbe.java`.
Note the probe needs no particular JDK, unlike the JPEG oracle, because no image
codec is involved.

Five things had to be right, none guessable from "bicubic": the Mitchell
-Netravali kernel with `A = -0.5`; its 513-entry table whose tail above index 384
is *derived* so each group of four sums to one rather than evaluated from the
polynomial; fixed-point arithmetic with coefficients scaled by 256, a `1 << 15`
rounding bias and a `>> 16` with saturation (OpenJDK ships the integer variant of
three); 32.32 fixed-point coordinate stepping with a half-pixel subtraction
before both the gather and the interpolation; and branchless sign-bit edge
clamping that duplicates the border row or column. Also: destination pixels whose
centre maps outside the source are never written, which is why a page render has
a black margin rather than an extrapolated one.

**What is left is the mechanical part**, in rough order:

1. PDF object, xref (classic and stream), object-stream and page-tree parsing.
2. `FlateDecode` with PNG/TIFF predictors, `LZWDecode`, `RunLengthDecode`,
   `ASCIIHexDecode`, `ASCII85Decode`. `DCTDecode` is already done in
   `audiveris-jpeg`.
3. `CCITTFaxDecode` G3 1D/2D and G4 -- what most of the corpus actually uses.
4. `JBIG2Decode` generic region with its arithmetic coder. Bounded but meaty;
   PDFBox delegates to `jbig2-imageio`, so that plugin is the reference rather
   than the spec.
5. The page-render plumbing: content-stream CTM, `ImageType.GRAY` conversion,
   and the destination's initial state.

**Subsampling: answered, and the page composition with it.** PDFBox does not
subsample -- `PDImageXObject.getImage()` on a real IMSLP page returns the full
2315x2848 raster and the render is 2479x3299. But the naive model, "scale the
source to fill the destination", is wrong by 22.5% of pixels, and the reason is
worth knowing before writing any of the above. The page content stream is:

```
q
515 0 0 633.5724 45 74.2138 cm
/Im0 Do
Q
```

The image occupies 515 x 633.5724 pt at offset (45, 74.2138) on a 595 x 792 pt
page -- it does *not* fill it, and the render carries white margins. So the
transform handed to Java2D is a scale *and* a translate, and the scale is a
**downscale** of about 0.927, not the 1.07 the page-to-image ratio suggests.

Two further details, each of which would break every sample comparison on its
own:

- **PDFBox computes the DPI scale in `float`.** `792 * (300/72f)` is
  `3299.999874`, so the page floors to 3299 pixels; in `double` it is exactly
  3300. The observed render is 3299. A one-pixel page changes every destination
  coordinate.
- `PDFStreamEngine.getGraphicsState().getCurrentTransformationMatrix()` reported
  identity for this image in a naive subclass. Read the content stream, or
  verify the engine subclass against it, before trusting a CTM.

`draw_bicubic` is that generalization, and fed PDFBox's own composed
`AffineTransform` it reproduces **eight of twelve real IMSLP pages bit-exactly**,
covering every case the corpus turns on: offset placements with upscale, and
0.5-0.53 downscales of 6109x7676 G4 scans.

Reading the transform rather than deriving it is what closed it. Composed in
closed form the horizontal terms were already bit-identical, but `scale_y`
differed in its last bits, which alone put 924 samples wrong across 13
destination rows. `oracle/java/` has no probe for this yet; the one used was a
`PageDrawer` subclass overriding `drawImage`, printing
`getGraphics().getTransform()` concatenated with
`new AffineTransform(ctm)` -> `scale(1.0/w, -1.0/h)` -> `translate(0, -h)`.
**Promote that probe into `oracle/java/` and pin these pages** -- it is the only
way this stays honest once decoders start landing.

**Surveying the whole corpus is what caught the real gap.** Across all 189 pages
of the seven sampled sources: every page has `rot=0` and exactly one image draw,
scales run 0.5 to 1.0, and 188 of 189 sources are single-band gray -- but **70
draws carry a shear**, up to 1.2e-2 or about 0.7 degrees, because the scans were
deskewed by baking a rotation into the content stream's `cm`. Twelve pages of
spot-testing had missed that regime completely, and the axis-aligned `Placement`
could not express it at all. `Placement` is now a full six-term affine and both
sheared SIBLEY pages are exact.

Two things still open, neither a transform problem:

- One page decodes to `TYPE_INT_RGB`, three bands; the crate is single-band.
  Only 1 of 189 draws is colour, so this is low priority but real.
- Three pages: near-identity scale (0.99995927), sub-pixel vertical offset,
  destination a pixel smaller than the source. **Measured, not guessed:**
  PDFBox's render on those pages is byte-identical to the decoded source, with
  zero intermediate greys in 7.5M samples, so Java2D did not interpolate at all.
  This code cannot match that -- the source coordinate's fraction drifts to ~0.1
  across the page, giving bicubic weights near `[-10, 250, 17, -1]`, which must
  produce greys on a bitonal edge.

  Ruled out so far: the images are not stencils, have no `/Interpolate`, and are
  1-bit `DeviceGray` exactly like the pages that do reproduce;
  `DrawImage.tryCopyOrScale`'s copy branch needs the destination within
  `MAX_TX_ERROR` (1e-4) of the source size and this is 0.1 out; and
  `renderImageScale` returns false for anything but nearest-neighbour. So the
  responsible branch is still unlocated -- read `SunGraphics2D.drawImage` and
  `DrawImage.transformImage` (the `x, y, extraAT` overload at line 139 has its
  own `closeToInteger` copy path) before writing any trigger condition. About 10
  of 189 draws are in this regime, so it matters but is not the common case.

Test sources are the twelve PDFs in the `imslp-pseudo` repo's
`manifests/acquired_scans.json`, each with a download URL.

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
