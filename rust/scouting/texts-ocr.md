# OCR / TEXTS parity strategy — risk scout report

All paths below are relative to `/Users/john/sources/aug04-rubigo/audiveris` unless
absolute. `app/...` line numbers refer to the Java 5.11 baseline tree in this checkout.

## VERDICT

**Adopt strategy (a) — FIXTURE — with high confidence.** Java invokes Tesseract 5.5.2
in-process through bytedeco/JavaCPP prebuilt native binaries, in *legacy* engine mode
(`OEM_TESSERACT_ONLY`), and the whole-sheet OCR input is a derived image whose pixels
depend on every prior stage — both facts make (b) LINK a fight against a foreign
binary's floating-point behaviour for zero porting value, while (a) cuts at an
interface Audiveris itself already defines (`OCR.recognize`, one Java class,
`List<TextLine>` out). The Rust side has *already committed to this shape*: the
lifecycle port `rust/crates/audiveris-omr/src/texts_step.rs` declares an injected
`ExternalTexts` seam. What (a) needs is (1) a recorder probe on the Java side, (2) a
richer neutral OCR payload (baseline, font attributes, per-char boxes — the current
`NeutralOcrWord` is too thin), and (3) the discipline that the fixture covers **two**
batch call sites, not one: TEXTS (whole sheet) and CURVES (rehearsal enclosures).
Measured this session: the full Java pipeline through TEXTS runs headless on this
machine once two environment gaps are closed (symbol population in the probe harness,
and a legacy-capable `eng.traineddata`), and the corpus yields ~134 sentences / ~252 words
/ 262 lyric items of OCR-derived inters — enough to grade against, small enough to
record — with raw OCR output measured bit-identical across two JVM sessions on this
machine.

## MEASURED (this session, this machine)

### How Java invokes Tesseract

- Bridge: **bytedeco/JavaCPP**, not tess4j, not CLI. Imports of
  `org.bytedeco.tesseract.TessBaseAPI`, `ResultIterator`, leptonica `PIX` —
  `app/src/main/java/org/audiveris/omr/text/tesseract/TesseractOrder.java:35-46`.
- Version pins: `tessVersion = '5.5.2'`, `leptVersion = '1.87.0'`, `jcppVersion =
  '1.5.13'` — `app/build.gradle:23-25`; platform-classified native jars pulled per
  target OS at `app/build.gradle:105-106,119-120`.
- Engine mode: **`OEM_TESSERACT_ONLY`** (legacy, non-LSTM) —
  `TesseractOrder.java:412` (`api.Init(ocrFolder.toString(), langSpec,
  OEM_TESSERACT_ONLY)`).
- Page segmentation: `MULTI_BLOCK → PSM_AUTO`, `SINGLE_BLOCK → PSM_SINGLE_BLOCK`
  (`TesseractOCR.java:204-214`); a `forceSingleBlock` constant can override
  (`TesseractOCR.java:376-378`, default false).
- Image handoff: the `BufferedImage` is encoded to an **in-memory TIFF** via the
  registered jai-imageio TIFF writer (`TesseractOrder.java:98-104, 480-516`) and read
  back with leptonica `pixReadMemTiff` (`TesseractOrder.java:165-167`). So Tesseract
  sees a PIX decoded from that TIFF, not the Java raster directly.
- Variables/settings: optional char whitelist/blacklist, **both empty by default**
  (`TesseractOrder.java:420-432, 562-568`); `SetSourceResolution(70)` — the
  `typicalImageResolution` constant defaults to 70 dpi (`TesseractOrder.java:437-440,
  557-560`); then `SetPageSegMode`, `AnalyseLayout()`, `Recognize(null)`
  (`TesseractOrder.java:443-447`).
- Result walk: `ResultIterator` at `RIL_TEXTLINE` / `RIL_WORD` / `RIL_SYMBOL`
  (`TesseractOrder.java:314-380`). Per word: bounding box, UTF-8 value, baseline
  (`Baseline()` → `Line2D`), `Confidence(RIL_WORD)/100.0`, and
  `WordFontAttributes` mapped to `FontInfo` (bold/italic/underlined/monospace/
  serif/smallcaps/pointSize; fontName deliberately dropped as unreliable,
  `TesseractOrder.java:262-303`). **Words with no font attributes are skipped
  entirely** (`TesseractOrder.java:342-349`). Per symbol: bounding box + value; a
  multi-char symbol (e.g. ligature "sz") is split into equal-width `TextChar`s
  (`TesseractOrder.java:527-547`). Lines whose value is blank are dropped
  (`TesseractOrder.java:370`).
- Languages: spec comes from `sheet.getStub().getOcrLanguages()`
  (`SheetScanner.java:173`); the application default is **`eng`**
  (`app/src/main/java/org/audiveris/omr/text/Language.java:170-171`).

### What image TEXTS feeds to OCR

`SheetScanner.getCleanImage()` (`SheetScanner.java:121-153`): starts from
`Picture.SourceKey.NO_STAFF` (staff-line-free binary-derived gray buffer), then
`TextsCleaner.eraseInters()` erases every good inter recognised so far, fills each
staff core area, re-thresholds at 127, and erases border glyphs touching staff cores
(`SheetScanner.java:276-313`). `OcrUtil.scan` then pads a **10-pixel white margin**
(TYPE_BYTE_GRAY) around the image and translates coordinates back afterwards
(`app/src/main/java/org/audiveris/omr/text/OcrUtil.java:110-136, 162-165`). One OCR
call per sheet — whole page, MULTI_BLOCK (`SheetScanner.java:177`).

### Every call site of the OCR API (complete enumeration)

`OCR.recognize` is reached only through `OcrUtil.scan` (`OcrUtil.java:136`). Callers
of `OcrUtil.scan`:

1. **`SheetScanner.scanSheet` — TEXTS step, batch pipeline.**
   `TextsStep.doProlog` → `scanner.scanSheet()` (`TextsStep.java:87-102`,
   `SheetScanner.java:163-183`). One whole-sheet MULTI_BLOCK call.
2. **`BlockScanner.scanBuffer` — SINGLE_BLOCK, per-region**
   (`app/src/main/java/org/audiveris/omr/text/BlockScanner.java:84-94`). Two callers:
   - **`RehearsalsBuilder.createInter` — CURVES step, batch pipeline**
     (`app/src/main/java/org/audiveris/omr/sheet/curve/RehearsalsBuilder.java:186`),
     reached from `Curves.buildCurves` → `buildRehearsals()`
     (`app/src/main/java/org/audiveris/omr/sheet/curve/Curves.java:160-162`,
     `CurvesStep.java:54`). Input: a crop of the **BINARY** source around a candidate
     rehearsal-mark enclosure, with the enclosure strokes erased
     (`RehearsalsBuilder.java:169-189`). Conditional — only fires when four segments
     form an enclosure box.
   - **`InterController.addText` — GUI-only** (manual "assign text" action,
     `app/src/main/java/org/audiveris/omr/sig/ui/InterController.java:335-355`). Not
     part of batch parity.
3. Other `OCR` interface methods: `getSupportedLanguages()` spins up its own
   `TessBaseAPI.Init` against the tessdata folder (`TesseractOCR.java:153-184`) and is
   called on **every** scan via `supports()` (`OcrUtil.java:105`,
   `TesseractOCR.java:317-347`), plus GUI surfaces (`Language.java:286`,
   `MainGui.java:573`, `BookParameters.java:498`). `getMinConfidence()` (0.65
   constant, `TesseractOCR.java:189-193, 384-387`) is consumed by Audiveris logic
   (`TextBuilder.java:1382`) — that constant is port logic, not engine output.

So the fixture seam must fake three things: `isAvailable()`, `supports(langSpec)`
(trivially true/recorded), and `recognize(...)` (recorded lines) — for **two batch
stages** (TEXTS and CURVES). No OCR call exists in SYMBOLS, LINKS, or lyrics
processing; lyrics are re-*processed* (never re-OCR'd) from the same TEXTS lines.

- Pipeline order fact: TEXTS runs **before** MEASURES/CHORDS/CURVES
  (`app/src/main/java/org/audiveris/omr/step/OmrStep.java:55-73`), so the CURVES
  rehearsal OCR happens later and against a different source (BINARY crop), i.e. its
  fixture entries are independent of the TEXTS entry.

### What downstream consumes from OCR (the seam payload)

Recorded per word (constructor `TextWord.java:134-` via `TesseractOrder.getLines`):
`bounds` (Rectangle), `value` (String), `baseline` (Line2D), `confidence` (0..1
double), `FontInfo` (6 booleans + pointSize; fontName null —
`app/src/main/java/org/audiveris/omr/text/FontInfo.java:66-98`), and the `TextChar`
list (per-char bounds + value). All of it is load-bearing downstream:

- char boxes: sub-word splitting via `WordScanner`/`OcrScanner`
  (`TextLine.java:806-808`) and word-glyph construction (`TextBuilder.java:573`);
- confidence: word validity gate (`TextWord.checkValidity`, `TextWord.java:264-290`)
  and italic majority vote (`TextBuilder.java:1370-1400`);
- baseline: line sorting by skew-corrected ordinate (`OcrUtil.java:143`,
  `TextLine.java:872`), lyric/staff mapping;
- font attrs + pointSize: `word.adjustFont()` (`OcrUtil.java:145`,
  `TextWord.java:250-254`) and role guessing.

Everything after `OCR.recognize` returns — sorting, `adjustFont`, `TextBuilder`'s
entire system-level processing (`TextBuilder.java:959-`), role assignment, lyric
handling — is pure Audiveris logic and **is** a port target. The narrowest clean cut
is exactly `OCR.recognize(sheet, image, topLeft, langSpec, layoutMode, label) →
List<TextLine>` (`app/src/main/java/org/audiveris/omr/text/OCR.java:87-92`).

### Tessdata: where it comes from, and what this machine has

- Lookup order: `$TESSDATA_PREFIX`, else `<CONFIG_FOLDER>/tessdata` (created if
  absent) — `TesseractOCR.java:117-147`; on macOS CONFIG_FOLDER is
  `~/Library/Application Support/AudiverisLtd/audiveris`
  (`app/src/main/java/org/audiveris/omr/WellKnowns.java:148, 449`).
- Traineddata is **not bundled**; it is downloaded interactively from the GitHub
  `tesseract-ocr/tessdata` repository by a GUI task
  (`app/src/main/java/org/audiveris/omr/text/tesseract/Languages.java:89-91,167-296` —
  uses `OmrGui`, so batch never triggers it).
- **This machine:** `~/Library/Application Support/AudiverisLtd/audiveris/tessdata`
  exists and is **empty** (created 2025-07-14; a prior run made the folder, nothing
  downloaded). `TESSDATA_PREFIX` is unset. Homebrew has
  `/opt/homebrew/share/tessdata/eng.traineddata`, but `combine_tessdata -d` shows it
  is **LSTM-only** (4.1 MB, components 17-23; no `inttemp`/`normproto`) — i.e.
  tessdata_fast, which **cannot** satisfy `OEM_TESSERACT_ONLY`. The combined
  legacy+LSTM `eng.traineddata` from `tesseract-ocr/tessdata` (23,466,654 bytes,
  components 1-23 incl. legacy `inttemp`) fetched to the session scratchpad does work.

### Corpus exercise (live JVM, JDK 25, headless)

Two environment gaps had to be closed to reach TEXTS headless, and both are findings:

1. **First run failed before OCR, in HEADS template building**: `TemplateSymbol. No
   symbol for NOTEHEAD_BLACK_SMALL in family Bravura` then NPE
   (`app/src/main/java/org/audiveris/omr/ui/symbol/TemplateSymbol.java:95-109`).
   Root cause: small-head symbols exist only in the prepopulated symbol map
   (`Symbols.mapSmall`, `Symbols.java:208-212, 270`), and population happens only via
   `MusicFont.checkMusicFont()` → `populateAllSymbols()` (`MusicFont.java:514-516,
   683-688`), which is called from `Main.main` (`Main.java:269`) and `MainGui` — the
   `SigProbe` harness (`rust/oracle/java/org/audiveris/omr/rustport/SigProbe.java`)
   drives `Book` directly and skipped it. Adding one `MusicFont.checkMusicFont()`
   call to the probe fixes it (verified by rerunning; see harness note below on how
   that line ended up committed). **Any HEADS-or-later oracle capture needs this
   call in the harness.** Tesseract itself was *never* the headless blocker.
2. **Tessdata**: with `TESSDATA_PREFIX` pointing at the scratchpad copy of the
   combined `eng.traineddata`, Tesseract 5.5.2 loaded and ran headless in-process
   (log: `ocrFolder: .../scratchpad/tessdata`, `Lang file: eng.traineddata bytes:
   23466654`), producing words with value/bounds/confidence/baseline/font attrs,
   e.g. `TextWord{ "CHULA" bounds[988,133,225,42] conf:0.91 base[988,172]-[1213,175]
   FontInfo{Serif S-66}}`.

TEXTS-stage inter counts (SigProbe at TEXTS, per page):

| page | SentenceInter | WordInter | other text |
| --- | --- | --- | --- |
| chula.png | 5 | 11 | — (9 raw OCR lines) |
| allegretto.png | 35 | 52 | 1 MetronomeInter |
| BachInvention5.jpg | 7 | 12 | — |
| batuque.png | 10 | 25 | — |
| carmen.png | 22 | 41 | 1 ChordNameInter, 1 MetronomeInter |
| cucaracha.png | 5 | 14 | — |
| D0392410-1.256.png | 10 | 16 | 3 LyricLineInter, 32 LyricItemInter |
| Dichterliebe01.pdf | 11 | 24 | 2 LyricLineInter, 37 LyricItemInter |
| hove.png | 12 | 17 | 4 LyricLineInter, 27 LyricItemInter |
| SchbAvMaSample.pdf | 14 | 29 | 9 LyricLineInter, 166 LyricItemInter |
| zizi.png | 3 | 11 | 1 MetronomeInter |

Totals: ~134 sentences, ~252 words, 262 lyric items, 18 lyric lines, 3 metronome,
1 chord name across the 11 pages probed (the 9-page grading corpus plus the two
PDFs' first sheets). Every page has text; four are lyric-bearing. LOAD→TEXTS wall
time ~12-25 s/page in the probe (chula 12.4 s, BachInvention5 24.7 s); the OCR call
itself is on the order of a second.

**Same-machine determinism (measured): chula OCR'd twice in different JVM sessions
— all 37 raw `TextWord` debug records identical in value, bounds, baseline,
confidence, char codes, and font attributes.** The only difference was the
Audiveris-assigned `glyph#` ids on the 11 mapped words, uniformly offset by +52
because the second run's JVM had processed other sheets first — a reminder that
absolute glyph/inter ids are session state, not recognition output, and oracle
comparisons must not key on them.

**CURVES call-site exercise (measured): carmen.png and allegretto.png driven to
CURVES.** `RehearsalsBuilder` found one candidate segment pair on carmen and
rejected it before OCR (`No rehearsal left leg below ...`); zero `glyph-`-labelled
OCR orders, zero `RehearsalInter` on both pages. So the second batch call site is
live code but dormant on at least these two corpus pages; the remaining pages are
unmeasured at CURVES. The fixture seam should cover it regardless, with a
fail-loud miss.

**Harness note:** while this scouting ran, a concurrent session committed
`f6c638562`, which absorbed the temporary `MusicFont.checkMusicFont()` line added
to `SigProbe.java` (now at `SigProbe.java:76-78` with a "TEMPORARY (ocr-risk
scouting)" comment). The call is the *correct permanent* fix for any HEADS-or-later
capture, but the comment should be reworded to say so.

## INFERENCE

- **The fixture is only valid while every stage upstream of the OCR call is
  bit-identical to the capture run.** The TEXTS input image is NO_STAFF minus every
  good inter from GRID..CUE_BEAMS plus staff-core fills; one differing pixel anywhere
  upstream silently invalidates the recording. This is exactly the guarantee the
  port's graded-stage discipline already provides, but the seam should *enforce* it:
  key each fixture entry by a **hash of the submitted image pixels** (+ langSpec +
  PSM). A replay that reaches the seam with a non-matching hash must fail loudly, not
  serve stale text. That converts the dependency from an assumption into a checked
  invariant, and it also handles call-site identity for the CURVES per-enclosure
  calls (each crop hashes differently) without inventing an ordering key.
- The `getSupportedLanguages`/`supports` path must be part of the seam contract
  (return `{eng} ⊇ codes`), else the Rust TEXTS prolog takes the `Missing support`
  branch (`OcrUtil.java:105-108`) and produces an empty-but-different result.
- The empty-tessdata behaviour is itself a parity case worth one fixture: with no
  traineddata, `supports()` warns and `scan` returns an empty list, and TEXTS
  completes with zero text inters — the port must reproduce that shape of "OCR
  unavailable", since real users hit it (this machine did).
- Rust seam status: `texts_step.rs` already routes everything through `trait
  ExternalTexts` (`rust/crates/audiveris-omr/src/texts_step.rs:217-231`:
  `is_ocr_available`, `scan_sheet`, `process_system_texts`), and `curves_step.rs`
  keeps `BuildRehearsals` behind its injected visual seam
  (`rust/crates/audiveris-omr/src/curves_step.rs:23,35`). But the current
  `NeutralOcrWord` carries only `id/value/bounds/confidence_milli`
  (`texts_step.rs:32-38`) — **no baseline, no font attributes, no per-char boxes** —
  so as TextBuilder logic moves from the `process_system_texts` seam into native
  Rust, the neutral payload must grow to the full recorded tuple or sub-word
  splitting/adjustFont/italic-vote cannot be ported bit-exactly.
- Fixture regeneration when the corpus grows: rerun the recorder probe for the new
  pages only — recordings are per-page and independent. The capture must pin (and the
  fixture header should embed): traineddata SHA (the 23,466,654-byte combined eng),
  bytedeco tesseract version string (5.5.2-1.5.13), capture platform, and the
  upstream-stage oracle commit it was captured against.

## GENERAL KNOWLEDGE (clearly labelled; not verified in this repo)

- Tesseract's legacy engine (`OEM_TESSERACT_ONLY`) has no RNG and, unlike the LSTM
  path, no OpenMP parallelism in recognition, so it is reproducible for a *fixed
  binary + traineddata + input image*. It is **not** guaranteed stable across
  binaries: classifier scores are floats, and compiler/SIMD differences (x86 SSE vs
  arm64 NEON, -O flags, libm variants) can flip near-tie character decisions. The
  port's own history already shows this class of bug at much smaller scale (the
  `java_hypot` libm divergence CI caught on the ubuntu leg, per `rust/HANDOFF.md`).
- bytedeco ships prebuilt per-platform natives (the `:$targetOS` classifiers above),
  built with bytedeco's own flags. A Rust `tesseract-sys` build would link a
  *differently compiled* 5.5.2 — so strategy (b) is not "same engine", it is "same
  sources, different binary", plus a re-implementation of the jai-imageio
  TIFF-encode → `pixReadMemTiff` handoff. Byte-identical output is plausible on most
  words and unprovable in general; every divergence would be undebuggable noise in a
  bit-exact grading scheme.
- The macOS-arm64 vs linux-x64 bytedeco binaries may *already* disagree with each
  other on marginal words, meaning "Java parity" for (b) is ill-defined until you
  also pin which platform's Java run is the oracle. Strategy (a) sidesteps this by
  definition: the recording *is* the oracle.

## PLAN (strategy a)

1. **Recorder probe (Java, ~1 day).** A `TextsOcrProbe` next to `SigProbe` under
   `rust/oracle/java/`: call `MusicFont.checkMusicFont()` (the missing harness call),
   run to TEXTS and CURVES, and — via a small `OCR` wrapper delegating to
   `TesseractOCR.getInstance()` — dump, per `recognize` call: SHA-256 of the
   submitted image pixels (post-margin), langSpec, PSM, label, and every raw
   `TextLine`/`TextWord`/`TextChar` (value, bounds, baseline, confidence, 6 font
   booleans + pointSize) **before** `OcrUtil`'s sort/adjustFont. Output
   `rust/oracle/texts-ocr.txt` in the existing oracle-file style. Requires the
   combined legacy `eng.traineddata` (pin its SHA in `manifest.sha256`;
   `TESSDATA_PREFIX` in the capture recipe).
2. **Seam in Rust (~1-2 days).** Extend `NeutralOcrWord`/`NeutralOcrLine` in
   `texts_step.rs` with baseline, font attributes, and char boxes; add a
   `FixtureOcr` implementing the OCR side of `ExternalTexts::scan_sheet` (and the
   CURVES rehearsal scan when that stage arrives) by hash lookup into
   `texts-ocr.txt`, erroring on a miss. `is_ocr_available`/`supports` come from the
   fixture header.
3. **Grade TEXTS (bulk of the work, and it is TextBuilder, not OCR).** Port
   `TextBuilder`/`TextLine`/`TextWord` processing natively (validity checks, merge,
   role guessing, lyric mapping — `TextBuilder.java` is ~1400 lines plus
   `TextLine`/`TextWord` logic; estimate 1-2 weeks) and grade end-of-TEXTS SIG
   against `sigProbe` output, exactly like BEAMS. The OCR fixture makes this a
   deterministic, CI-safe gate on both legs (no Tesseract, no traineddata in CI).
4. **Parity definition to state in PORTING.md:** "TEXTS parity = bit-exact
   reproduction of Audiveris's processing of a recorded Tesseract 5.5.2 output,
   keyed to bit-exact input images; the engine itself is an external fixture, like
   the JVM was for earlier oracles." Full-path OCR via linking stays available later
   as a *product* decision (it is how a shipped Rust Audiveris would run) — the
   fixture seam is exactly where a real `tesseract-sys` binding would plug in, so (a)
   is a prerequisite of (b), not a fork away from it.

## What could still bite us

- **Silent fixture staleness** if upstream stages change after capture — mitigated by
  the image-hash key (fail loud), but only if the key really covers the pixels, the
  langSpec, and the PSM.
- **CURVES rehearsal OCR is conditional and corpus-dependent**: if no corpus page
  triggers an enclosure, the fixture has no entry and the first page that does
  trigger one fails at replay (that is the correct behaviour — capture then).
  Currently unmeasured which of the 9 pages trigger it.
- **GUI-only OCR** (`InterController.addText`) is outside batch parity; if the port
  ever grows an interactive mode, that call site needs a live engine or a policy.
- **`getSupportedLanguages` side effects**: it Init's the API against the folder on
  every scan via `supports()`; a naive port that skips it changes nothing observable,
  but if logging/behaviour parity at the message level ever matters, note it.
- **Traineddata provenance**: the GitHub `tessdata` repo files are mutable refs;
  record the SHA-256 of the exact `eng.traineddata` used at capture, or a future
  regeneration quietly captures against different language data.
- **Multi-sheet books / new corpus items with non-`eng` specs**: `getOcrLanguages`
  is a book/sheet parameter; fixtures must record the langSpec they were captured
  with and replay must refuse a mismatch.
- **The probe harness gap**: any Java-side capture at HEADS or later must include the
  `MusicFont.checkMusicFont()` call; until that lands in `SigProbe` permanently,
  every future capture attempt will rediscover the NPE.
