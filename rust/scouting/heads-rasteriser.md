# HEADS and the rasteriser: what template matching actually consumes

## VERDICT

**Nothing structural to worry about for HEADS: the rasteriser output is consumed only at
template-construction time, is binarised at a fixed threshold, and template construction
depends on nothing but `(MusicFamily, Shape, integer pointSize)` — so every template the
corpus needs can be dumped from the live JVM as finite oracle data, exactly like the
classifier weights and the twelve centroid-offset constants.** At match time the template
is a list of key points whose *only* consumed attribute is a 3-way class (foreground /
background / hole); the chamfer magnitudes stored in the template are never read by
`evaluate`. The sheet-side distance table is pure integer chamfer arithmetic on the binary
image — portable computation, no rendering. The one genuinely new rasteriser consumer
found in the sweep is **not** in HEADS: `PageCleaner` (base of the SYMBOLS-step
`SymbolsFilter` and TEXTS-step `SheetScanner` erasers) paints font glyphs at fractional
positions into the erase buffer; that seam needs its own scout later. The headless probe
failure is a one-line probe bug (missing `MusicFont.populateAllSymbols()`), not a
headless-rendering limitation — batch Java runs HEADS headless in production.

---

## 1. MEASURED: the full path from font to head inter

### 1a. Template construction (build time, per catalog)

`TemplateFactory.Catalog` is keyed by `(MusicFamily, pointSize)` and eagerly builds one
template per head shape:

- `TemplateFactory.getCatalog(family, pointSize)` — app/src/main/java/org/audiveris/omr/image/TemplateFactory.java:158-181
- `Catalog.buildAllTemplates()` loops over **all 24** `ShapeSet.Heads` shapes — TemplateFactory.java:1055-1065; the 24 shapes are 6 motif groups × 4 (glyph/ShapeSet.java:199-312).

`Builder.buildTemplate()` (TemplateFactory.java:816-861) does, in order:

1. `MusicFont.getMusicFont(family, pointSize)` — TemplateFactory.java:820.
2. `new TemplateSymbol(shape, family)` and `symbol.getFatBounds(font)` — the *fat* symbol
   bounds come from `TextLayout.getBounds()` (ui/symbol/TemplateSymbol.java:114-135:
   `symParams.layout.getBounds()`), i.e. the fixed-point font-metric machinery **already
   ported exactly** for HEADERS. Template outer size = symbol size + `room`, where
   `room = 2*ceil(8/3 · pointSize/80)` (TemplateSymbol.java:122-128 with
   `maxRawDistanceFromSymbol = 8`, `defaultPointSize = 80`, TemplateFactory.java:1116-1129).
3. **The only rendering call**: `symbol.buildImage(font)` — TemplateFactory.java:831 →
   `ShapeSymbol.buildImage` (ui/symbol/ShapeSymbol.java:195-221) which paints the glyph
   black on transparent ARGB. `KEY_ANTIALIASING` is set OFF (ShapeSymbol.java:216) but, as
   HANDOFF already established for centroid offsets, `TextLayout.draw` obeys
   `KEY_TEXT_ANTIALIASING`, so the image carries graded alpha (~200 distinct values —
   measured in `rust/oracle/music-font.txt`, e.g. `distinct=196` for F_CLEF).
4. **Immediate binarisation**: `binarize(img, 140)` — TemplateFactory.java:832, threshold
   constant `binarizationThreshold = 140` (TemplateFactory.java:1093-1096). A pixel is
   FORE iff `alpha >= 140 && red < 140`, else BACK (TemplateFactory.java:728-748; the
   paint color is opaque `Color.BLACK`, ui/symbol/OmrFont.java:95). **Graded alpha never
   survives past this line.** The match consumes nothing analog.
5. `getSlimBounds` trims rows/columns with fewer than `minCellPerSide = 2` FORE pixels
   (TemplateFactory.java:400-484, 1121-1124).
6. Key points (lazily, via `Template.getKeyPoints` → `TemplateFactory.retrieveKeyPoints` →
   `Builder.processSymbol(1)`, Template.java:463-470, TemplateFactory.java:603-608,
   919-943): re-render + binarise, integer chamfer-3/4 distance transform
   (`ChamferDistance.Short`, image/ChamferDistance.java:34-36, 247-336 — pure integer
   two-pass computation), flood-fill holes for the 19 `shapesWithHoles`
   (TemplateFactory.java:97-120, 649-720), negate distances inside holes
   (TemplateFactory.java:1010-1020), then keep every cell with raw distance
   `<= rint(8 · pointSize/80)` as a `PixelDistance(x, y, d)` (TemplateFactory.java:793-809,
   585-590).
   - Fallback: if hole flood-fill leaks to the template corner, the symbol is re-processed
     at `2×` resolution and the distance table block-averaged back down with the
     `|dMean|==1 → 0` quantisation (TemplateFactory.java:929-1002). Deterministic, still
     only a function of (family, shape, pointSize).
7. Anchors: 3 basic + 6 stem anchors computed from `slimBounds` plus constants
   `stemDx = -0.1`, `stemDy = -0.2` and per-shape switch tables
   (TemplateFactory.java:196-225, 354-375, 489-510). Pure arithmetic.

### 1b. What a `Template` stores

image/Template.java:87-113: shape, family, pointSize, width, height, `slimBounds`
(Rectangle), `Map<Anchor, Point2D>` offsets, and `List<PixelDistance>` keyPoints — i.e.
**a small integer mask-with-band plus anchor doubles. No image, no alpha.**

### 1c. What matching computes at runtime

- Sheet-side: `DistancesBuilder.buildDistances` (sheet/note/DistancesBuilder.java:103-136)
  takes the BINARY picture, erases staff lines/ledgers/stem-seeds (RunTable
  run-rectangle fills — no glyph rendering), computes `ChamferDistance.Short()
  .computeToFore` over the whole sheet, then re-marks the erased lines as
  `VALUE_UNKNOWN = -1`. All integer, all portable.
- `Template.evaluate(x, y, anchor, distances)` — Template.java:202-249. **The stored
  chamfer magnitude `pix.d` is used only as a 3-way class**: weight = 6 (fore, `d==0`),
  1 (back, `d>0`), 4 (hole, `d<0`) (Template.java:232-233, constants at 726-736);
  `expected = (d==0) ? 0 : 1`; the sheet-side value is likewise collapsed to
  `actual = (dist==0) ? 0 : 1` with `VALUE_UNKNOWN` cells skipped (Template.java:227-236).
  Result = weighted mean of `|actual−expected|`. Floating arithmetic over small integer
  counts — portable.
- `Template.evaluateHole` counts `pix.d < 0` cells vs non-zero sheet distance
  (Template.java:264-305) — again class-only.
- `Template.getForegroundPixels` uses `pix.d == 0` cells (+ optional ByteProcessor
  dilation) — Template.java:364-444.
- Grading: `impactOf(dist) = 1 − dist/0.5` (Template.java:666-669, 742-745); the
  low/high/reallyBad cutoffs 0.40/0.5/1.0 (Template.java:747-755).
- Driving logic (`NoteHeadsBuilder`): staff-line/ledger scanners, seed anchors
  `LEFT_STEM`/`RIGHT_STEM`, range anchor `MIDDLE_LEFT`, x/y offset windows, spot gating,
  aggregation, purges (sheet/note/NoteHeadsBuilder.java:826-906, 1937-2185). Complex but
  entirely geometric/arithmetic; only touches templates through the calls above. The one
  place sheet-side chamfer *magnitude* matters is the skip heuristic
  `distances.getValue(...)/3 > templateHalf` (NoteHeadsBuilder.java:1983-1989) — still
  integer chamfer, portable.
- Anchor rounding: `Template.getOffset` rounds with a ±0.001 epsilon depending on
  `anchor.hSide()` (Template.java:476-491) — port this carefully.

**Consequence:** the per-template payload the Rust side needs is exactly
`{w, h, slimBounds, 9 (or 3) anchor Point2Ds, keyPoints[(x, y, d)]}` — and of `d` only
sign/zero is ever read at runtime (dump raw `d` anyway; it costs one byte).

## 2. MEASURED: where rendering enters, and the binarisation cutoff

Single entry point: `TemplateSymbol.buildImage` inside `buildTemplate`/`processSymbol`
(TemplateFactory.java:831, 926). Binarised immediately at **alpha ≥ 140 AND red < 140**
(TemplateFactory.java:735-744). The match never sees graded alpha. So per template the
rasteriser contributes one binary mask (whose edge pixels depend on whether antialiased
coverage crosses 140/255 ≈ 0.549 — this *is* rasteriser-dependent and is why we dump
rather than recompute).

**Size estimate (INFERENCE, arithmetic from the constants):** at pointSize 80
(interline 20), a NOTEHEAD_BLACK template is ≈ (24+6) × (21+6) ≈ 800 cells; keyPoints =
fore+hole cells plus a ≤ 8/3 ≈ 2.7-px background band ≈ 500-700 points. At 3 bytes/point
≈ 2 KB; breves roughly double. A full 24-shape catalog ≈ 50-100 KB; the whole corpus
needs at most ~10 catalogs → **well under 1 MB of oracle data.**

## 3. MEASURED: the template set is finite — and how pointSize is chosen

Identity is `(family, pointSize)`; the corpus family is **Bravura**
(`rust/oracle/music-font.txt: musicfont.family=Bravura`).

The pointSize chain:

- Per staff: `Staff.getHeadPointSize() = MusicFont.getHeadPointSize(sheetScale,
  specificInterline)` — sheet/Staff.java:1129-1139; consumed at
  NoteHeadsBuilder.java:351-352 (one catalog per staff).
- `MusicFont.getHeadPointSize` (ui/symbol/MusicFont.java:620-634):
  - if the sheet has a `MusicFontScale`: `rint(staffInterline/sheetInterline ·
    musicFontScale.pointSize)`;
  - else fallback `getPointSize(rint(interline · headRatio))` with `headRatio = 1.0`
    (MusicFont.java:703-705) and `getPointSize(i) = 4·i` (ui/symbol/OmrFont.java:214-217).
- `MusicFontScale` is **sheet-content dependent**: set during BEAMS by
  `BlackHeadSizer.measureSingles` when ≥ 20 single black-head spots exist
  (sheet/beam/BlackHeadSizer.java:169-202, quorum at 321-324), via
  `MusicFont.computePointSize(measuredWidth)` — a 2-point secant interpolation on
  `TextLayout.getBounds().getWidth()` of NOTEHEAD_BLACK (MusicFont.java:209-241). Note
  this uses **only the already-ported TextLayout.getBounds machinery**, no rasterising.

So: pointSize is *not* enumerable a priori (it depends on measured head widths), but it
**is** a single integer per (sheet, staff interline), recorded in the sheet `Scale`
(marshalled into `.omr`, sheet/Scale.java:202, 700-710). For the 9-page corpus with
interlines {17, 20, 21} and no small staves, that is **at most 9 distinct pointSizes → at
most 9 × 24 = 216 templates** (fewer if sheets share pointSize; ~4·interline ± a few,
i.e. roughly 65-90). A probe run after BEAMS can print the exact integers.

## 4. MEASURED: template construction depends on nothing else — dumpable

`Builder` holds exactly `(shape, family, pointSize)` (TemplateFactory.java:618-638). The
only other inputs are ConstantSet constants (140, 8, 80, −0.1, −0.2, 2, 2 — 
TemplateFactory.java:1082-1130). **No sheet content, no Scale object, no interline beyond
the integer pointSize reaches template construction. A Java probe can therefore dump every
needed template as oracle data, exactly like the classifier weights.** This is explicit
and load-bearing: it is the entire mitigation.

## 5. MEASURED: why SigProbe dies at HEADS headless

`rust/oracle/java/org/audiveris/omr/rustport/SigProbe.java` never calls
`MusicFont.checkMusicFont()`/`populateAllSymbols()`; production batch mode does
(Main.java:269), the GUI does (ui/MainGui.java:545). Symbol lookup
(`Symbols.getSymbol`, ui/symbol/Symbols.java:124-140) falls back to code-based
`CodedSymbol`s, but the four `*_SMALL` head shapes exist **only** via
`populateSymbols()` (`mapSmall(NOTEHEAD_BLACK_SMALL, NOTEHEAD_BLACK)` etc.,
Symbols.java:270-273). So `Catalog.buildAllTemplates` reaches NOTEHEAD_BLACK_SMALL
(5th of the 24), `font.getSymbol` returns null through the whole backup chain, and
`TemplateSymbol.getParams` NPEs at `symbol.getParams(font)`
(ui/symbol/TemplateSymbol.java:95-114). **It is a probe bug, not a headless limitation** —
one `MusicFont.populateAllSymbols()` call in the probe prolog fixes it, and headless
template dumping is fully feasible (batch transcription does it every day).

## 6. MEASURED: sweep for other rendering consumers (later steps)

- **HeadInter.getTemplate** (sig/inter/HeadInter.java:814-830) and
  `retrieveGlyph` (HeadInter.java:1066-1083) — reuse the same catalog; no new rendering.
- **SYMBOLS step**: `SymbolsFilter.visit(HeadInter)` erases heads via
  `template.getForegroundPixels(..., dilated)` (sheet/symbol/SymbolsFilter.java:475-489) —
  template data again, plus portable `ByteProcessor.dilate`.
- **TEXTS step**: `SheetScanner` same pattern (text/SheetScanner.java:319-331).
- **⚠ PageCleaner** (sheet/PageCleaner.java:146-181, 439-448, 471-479), the base class of
  both erasers above: for *non-head* inters it erases by **painting the shape's font
  symbol** (`fs.symbol.paintSymbol(g, fs.font, center, AREA_CENTER)`) in white at
  enlarged point sizes onto the buffer, at **fractional center coordinates**, before the
  buffer is re-thresholded (e.g. SheetScanner threshold(127) at text/SheetScanner.java:307).
  This is a genuine rasteriser consumer downstream of HEADS (SYMBOLS and TEXTS), and
  unlike templates the drawn position is data-dependent (subpixel), so a fixed per-shape
  mask may not capture it exactly. **This is the next seam to scout — we were right to
  sweep.**
- `glyph/SymbolSample.java:92` (training sample repo) and `classifier/AnnotationsBuilder`
  — not in the transcription pipeline. `Debug.java:91-96` — dev tool. Everything else
  matching `buildImage|TemplateFactory|getForegroundPixels` lives under `ui/`.
- STEMS/REDUCTION/CUE_BEAMS: no template or font-image consumers found (the sweep over
  `sheet/stem`, `sig/` shows only geometric uses; `sig/inter/*` MusicFont references are
  bounds/UI-oriented — worth a five-minute confirm when STEMS parity work starts).

## INFERENCE (clearly separated)

- Corpus template count ≤ 216 and data volume < 1 MB (arithmetic from constants + corpus
  facts; exact integers need a probe run).
- The binarised masks are stable for our purposes because oracles are captured from the
  single pinned JVM (`jdk25/Contents/Home`); OpenJDK ships its own FreeType-based scaler,
  but cross-JVM/cross-version stability is *not* something we rely on — the dump is the
  contract.
- `PageCleaner` subpixel painting is flagged as a risk on reasoning about
  `TextLayout.draw` at fractional positions; its actual pixel effect (1-px fringe on
  erased areas) and whether positions quantise in practice is unmeasured.

## RECOMMENDED PLAN (Rust side)

1. **Fix the probe** (Java, ~30 min): add `MusicFont.populateAllSymbols()` to
   SigProbe/oracle prologs. HEADS becomes gradeable in Java immediately.
2. **Template oracle dump** (Java, ~1 day): after BEAMS on each corpus sheet, for each
   staff's `(Bravura, staff.getHeadPointSize())` dump per shape: width, height,
   slimBounds, all anchor offsets (as exact doubles), and keyPoints `(x, y, rawD)`.
   Also record each sheet's `MusicFontScale.pointSize`. Optionally dump a generous
   pointSize range (e.g. 40-120) so future inputs don't need a JVM — ~2-5 MB.
3. **Port the portable computation** (~1-2 weeks): `ChamferDistance.Short`
   (compute/computeToFore + UNKNOWN semantics), `DistanceTable`, `DistancesBuilder`
   erase-then-transform, `Template.evaluate/evaluateHole/getForegroundPixels/getOffset`
   (mind the ±0.001), `HeadSeedTally/Scale`, and the `NoteHeadsBuilder` scanners — big but
   mechanical; zero rendering.
4. **Port the sizing chain** (~1-2 days): `BlackHeadSizer.measureSingles` +
   `MusicFont.computePointSize` on the already-ported TextLayout.getBounds fixed-point
   path; verify NOTEHEAD_BLACK bounds parity at a few point sizes against the oracle.
5. Later, before SYMBOLS/TEXTS parity: **scout PageCleaner** the way this scout was done.

## WHAT COULD STILL BITE US

1. **PageCleaner font painting at fractional positions** (SYMBOLS, TEXTS) — the one real
   rasteriser dependency left; masks may not be position-independent.
2. **pointSize is data-dependent** — a Rust-only future (no JVM) needs either the ranged
   dump (step 2) or a rasteriser after all; for oracle-graded parity it's a non-issue.
3. **The 2× hole-retry path** (TemplateFactory.java:938-942): dump final keyPoints, never
   recompute; small point sizes are the ones that trigger it.
4. **Family backup chain** (TemplateSymbol.java:97-105): a non-Bravura sheet could pull
   glyphs from a backup family; dump per actual `(family, pointSize)` observed, not per
   assumed family.
5. **Cross-platform oracle capture**: capture template dumps on the same JVM+platform as
   every other oracle (MEMORY note: ubuntu CI leg) — glyph AA coverage near the 140
   threshold is the only thing that could differ, and it changes the mask.
6. **`Template.getOffset` epsilon rounding** and `getKeyPoints` laziness ordering — pure
   porting hazards, both cited above.
