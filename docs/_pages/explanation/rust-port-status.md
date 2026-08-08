---
layout: default
title: Rust port status
parent: Explanation
nav_order: 2
has_toc: true
---

# Audiveris Rust port status
{: .no_toc }

This page is the concise roadmap for the bit-exact Rust port of Audiveris.
Java 5.11 remains the behavioral oracle: a feature is described as graded only
when a deterministic Java/Rust comparison covers it.

**Current checkpoint:** schema-1 JSON recognition publishes every native stage
through `HEADS`, including accepted STEM_SEEDS and identity-free final heads;
native `STEMS` now continues through no-stem seed purging and existing-seed
selection plus exact section-built stump materialization and registration for
every head corner;
the human-readable text
report remains at `GRID`. HEADERS is an
oracle-free production call from live GRID state: all 65 staff headers, 34 keys,
17 times, and 30 erase rectangles match Java. The CLI now composes GRID ->
HEADERS -> STEM_SEEDS -> BEAMS -> LEDGERS -> HEADS in Java order into the exact
eight-sheet gates: 1,906 accepted seeds, 787 raw beams, 581 final ledger inters,
and 95 inferred ledger-line paths. JSON preserves selected header
evidence, system-owned erases, beam/ledger geometry and impacts, exclusions,
groups, and curved paths; `omrscope` accepts both median forms and bounds-only
header symbols and adapts accepted top-level stem seeds into its comparison
model. Native STEM_SEEDS composes GRID and HEADERS through the exact
2,425-candidate factory, 2,003 checks, and 1,906 accepted glyphs, and publishes
those accepted glyphs without inventing SIG or glyph IDs. A composed BEAMS
entry point now supplies all 1,906 seeds to beam-to-stem lookup; the measured
corpus output remains unchanged because no example beam accepts that extension.
A seeded-vs-hidden Java counterfactual confirms zero changes across all 30 systems.
That conclusion is now deliberately scoped to those original eight pages:
`D0392410-1.256.png` supplies a natural success where system 2 extends exactly
one beam to an accepted seed, and Rust matches Java's changed endpoint, height,
six impacts, and grade bit for bit.
The same BEAMS pass now retains Java's threshold-170 vertical `HEAD_SPOTS`
table for HEADS; its size and two independent pixel digests are exact on all
eight beam pages. LEDGERS now likewise carries the exact positioned fixed
glyph for every final non-removed ledger, built from its referenced filtered
sections in Java order and orientation. The production HEADS prolog now
composes those products with GRID's persistent staff lines, original binary,
system areas, and accepted STEM_SEEDS, and reaches the live distance-table and
transient-spot boundary on Chula without fixture-shaped inputs. An independent
fresh-JVM Java prolog oracle now freezes all eight pages—55 staves, 581 ledgers,
1,906 seeds, 2,790 components, and 30 dispatch systems—and the composed Rust
differential gate is now exact for every post-erasure BINARY
pixel, signed distance value, component run, and all 3,097 system-dispatch
references. Its only initial mismatch exposed Java's one-pixel-beyond-staff
`SystemInfo.getRight()` convention, now shared by every native consumer.
BEAMS now also retains the previously missing BlackHeadSizer side effect:
all 2,739 threshold-140 inputs and decisions, 936 singles, 5 stacks, 470 core
samples, eight sheet font sizes, and all 55 staff head point sizes match Java.
BEAMS now retains each system's exact group/member insertion order instead of
only its count. Its production MultipleRest pass now rebuilds Java's fresh
BEAMS-time staff projector from completed splines and the original binary,
replaces Bach system 6's one source beam exactly, and exposes both pre- and
post-replacement beam state; LEDGERS consumes the latter. HEADS now builds all
seed and head-spot pools plus the complete 474-member frozen GRID
bar/connector pool. It also reconstructs all 1,334 live competing-shape
candidates and Java's exact filtering decisions, leaving the same 847 accepted
competitors in stable ordinate order. Across all 1,767 scanners, every band,
seed/competitor Area bound, and ordered base slice matches Java: seed, spot,
frozen-bar, and competitor slices retain 15,343, 6,759, 5,060, and 1,944
references respectively. Template lookup now also has exact integer full/slim
bounds, Java `Rectangle.translate` overflow, hole-only evaluation, and
distance-to-impact/head-grade conversion. A fresh-JVM seed-pass oracle now
drives the real `processStaff(staff, true)` half of `NoteHeadsBuilder` in
production order. It hashes all 61,372 seed/side/shape searches and retains all
3,435 provisional candidates plus the 3,435 heads that survive glyph retrieval
across 55 staves and 30 systems. Its compact 3.83 MB fixture is byte-identical
across two generations (SHA-256
`aca3cd20941846ae0eab9b4c1e56b3c9959afb6ed649519888b854e2b68f0414`);
the full per-search trace remains an opt-in diagnostic. The active boundary is
now native too. A reusable Java2D-equivalent overlap kernel preserves
positive-area `Area.intersects(Rectangle2D)` behavior for the straight vertical
ribbons and horizontal parallelograms present in the current competitor pool.
The composed seed lookup matches all 55 staff hashes, all 61,372 ordered
searches, every outcome/performance partition, and all 3,435 provisional
candidates. Candidate order, selected offsets and pivots, shape conversion,
slim bounds, pitch, distance, impact, and grade all match at raw-double-bit
precision. `HeadInter.retrieveGlyph` is now native as a separate pure kernel:
it maps original BINARY foreground under zero-distance template keys into the
minimal vertical RunTable, then updates the head bounds. The composed gate
matches all 3,435 Java survivors by final order, provenance, pre/post bounds,
glyph weight and run digest, good-head decision, and LEFT/RIGHT tally values.
No Java process-global glyph or SIG ID is fabricated. The next HEADS boundary
is now frozen independently: 6,759 retained spot slices drive 921,558 scan
positions and 3,119,882 template attempts, yielding 34,101 raw candidates,
3,550 after aggregation, and 174 final heads after 3,376 seed conflicts and zero
empty-glyph drops. Its compact 6.48 MB fixture is byte-identical across two fresh-JVM
generations (SHA-256
`35a8d063d557979b9d5e948c279a6228c42ffd3fb5a7784d236779b490740770`);
the three large diagnostic row classes remain available with `--full-trace`.
The pure post-processing kernel is native too: it preserves Java's stable
reverse-grade aggregation around fixed first-member centers, signed/overflowing
rectangle IoU, and inclusive seed-conflict gates. Its compact corpus gate checks
the complete 34,101-member aggregate partition plus all 3,376 first conflicts
and 174 retained candidates, including conflict provenance and IoU bits. The
streaming range kernel is exact as well: all 6,759 spot slices, 921,558 x visits,
3,119,882 shape attempts, and 34,101 raw candidates match four independent
per-staff Java hashes. Production now composes those candidates per scanner
through the exact curved competitor band, aggregation, all seed-conflict
evidence, and original-BINARY glyph retrieval. Every one of 3,550 compact
candidates and 174 final range heads matches Java; all 3,376 drops are seed
conflicts and no glyph is empty. The following full HEADS epilog is frozen too:
3,609 seed-plus-range inputs lose 62 true duplicates, retain 2,725 overlap
exclusions over 3,547 staff heads, then lose 26 heads (and zero beams) in
small-beam arbitration, leaving 3,521 final heads and 18 analyzed seed-scale
rows. The 4.08 MB compact fixture retains the 1,451 live tally inputs plus all
191 small-beam inputs and 26 ordered arbitration decisions; 10,053 hidden beam
checks remain count/hash committed. It is deterministic across fresh JVMs
(SHA-256
`e893c2327a9afa937035559f1a5be170a22148dd6655e8ffb6297c75bff5f6ba`).
Native tally analysis preserves Java grouping, insertion-order binary64 sums,
inclusive quorum, and shape/side order; all 18 scale values match raw bits.
The shared purge loop is native independently: it preserves full-abscissa
ordering, true duplicate removal, overlap-only exclusions, grade ties, and
seed-tally preference/replication. Its exact glyph-aware `isSameAs` and
staff/pitch/ratio-aware head-overlap predicates are native too. A separate
small-beam kernel preserves SIG/ordinate order, exact parallelogram geometry,
strict width/grade gates, iterator removals, and Java NaN/overflow behavior.
The typed compact-fixture reader validates both SHA-256 commitments, every
reconstructible FNV stream, hierarchy, and staff-to-final-head multiset.

Production now composes that epilog directly from the live seed, range,
competitor, and BEAMS products. The staff pass and complete native epilog match
all 3,609 inputs, 62 duplicate removals, 2,725 overlap exclusions, 3,547
post-duplicate heads, 191 beam inputs, all 10,053 ordered beam checks by exact
per-system hash, 26
head removals, 3,521 final heads, 1,451 tally inputs, and 18 scale rows. Beam
contextual grades reproduce Java's coefficient-3/ratio-4 support arithmetic,
hook/beam exclusions, reverse-grade compatible partitions, MultipleRest and
prior arbitration removals, and dynamic recomputation. Production also retains
the exact fixed glyph Java rebuilds from `NO_STAFF` for every raw beam and hook;
all 191 narrow-beam glyph bounds, weights, and run digests match. The single
owned HEADS entry point is now the path graded by the eight-page differential,
and the CLI publishes its final heads, provenance, decisions, counts, and scale
rows without fabricating Java IDs.

The first three semantic `STEMS` boundaries are now production-shaped and graded.
`materialize_native_stems_head_corners` consumes the owned final HEADS product
plus live STEM_SEEDS parameters, retains final stem-capable heads in SIG order,
and exposes Java's stable abscissa and reverse-grade permutations without
inventing identities. It selects each staff's real Bravura template and applies
the exact sheet tally correction and profile/interline limits to the four
constructor-order head corners. An eight-page fresh-JVM oracle is exact for 30
systems, 3,521 heads, and all 14,084 reference/outside/inside corner points at
raw-double-bit precision. `materialize_native_stems_head_seeds` then reconstructs
the 483 connected-bar no-stem ribbons, purges the 1,906 free seeds in Java
insertion order, derives each fixed glyph's exact `BasicLine.toCenterLine`, and
performs the production vicinity, seed-area, stable-distance, horizontal, and
standout gates. The eight-page differential matches 1,749 kept and 157 purged
seeds, all 29,394 purge visits, 36,736 neighbor rows, 7,114 sorted candidates,
7,005 visited candidates, 4,182 selected seeds, and 9,902 explicit
section-fallback outcomes across all 14,084 corners. The fixture is
byte-identical across fresh-JVM runs (SHA-256
`19387924d0d7aaaabf07b0859b353c7fa8d3e3c5d10e8edec8e1d4287b1ace31`).
`materialize_native_stems_head_stumps` then consumes the 9,902 fallbacks against
each system's complete vertical lag. It preserves positive-area per-run
intersection, stable distance order, integer polygon containment, the repeated
pre-added-member width gate, shifted subsection extraction, tight fixed-glyph
rasterization, registration-before-standout, exact reuse, and corner aliasing.
The projected row differential is exact for 18,398 section and compound steps,
3,660 subsection attempts, 969 empty builds, and 8,933 registered candidates:
758 accepted, 8,175 rejected, 5,591 new, and 3,342 reused. The Java fixture is
byte-identical across fresh-JVM runs (SHA-256
`dd0247fbd992c7ec40351040efd336f98c8efa88bab0eef10c744430252e966e`).
Beam/head linker geometry and SIG mutation remain after this boundary.

Last updated 2026-08-07.

---

## On this page
{: .no_toc .text-delta }

1. TOC
{:toc}

## How to read the status

| Status | Meaning |
| :--- | :--- |
| **Native and published** | The Rust stage runs from real upstream Rust state and is exposed by the current CLI or JSON report. |
| **Native and graded** | The complete recognition path runs from real upstream Rust state and matches the Java corpus, but is not yet exposed by the CLI or JSON report. |
| **Components graded** | Substantial algorithms and contracts are native and oracle-tested, but at least one real input, output, or composition seam remains. |
| **Lifecycle only** | The stage ordering, state transitions, and failure contracts exist; most semantic or visual recognition work remains. |
| **Not ported** | No production-shaped Rust replacement exists yet. |

An isolated unit test or a headless stage shell is not enough to call a stage
ported. The distinction matters most after `BEAMS`, where much of the lifecycle
is present but the musical interpretation is not.

## Recognition pipeline

| # | Audiveris step | Status | What is ported | What remains |
| -: | :--- | :--- | :--- | :--- |
| 1 | `LOAD` | **Native and published** | PNG, the measured baseline-JPEG scope, and PDF page loading feed native grayscale rasters. | Broaden deliberately refused image variants only when a real input requires them. |
| 2 | `BINARY` | **Native and published** | Global and adaptive thresholding, filters, masks, runs, and full-page raster parity. | No known corpus gap. |
| 3 | `SCALE` | **Native and published** | Line, interline, beam, histogram, derivative, and decision logic are measured from the page. | Small-beam recognition needs a graded corpus case before downstream use. |
| 4 | `GRID` | **Native and published** | Staff lines, systems, bars, connectors, contextual grades, completed line geometry, and `NO_STAFF` pixels. All 65 staves and 420 barlines in the example corpus match Java. | Wire the already separated brace path before claiming brace-derived multi-staff Part ownership; then widen the PDF corpus. |
| 5 | `HEADERS` | **Native and published** | `recognize_native_headers` composes clef, key, and time columns in Java order from live GRID state alone. All nine pages and 65 staves match for starts/stops and selected evidence, including 34 keys, 17 times, and all 30 downstream erase rectangles. Schema 1 publishes selected inters, lifecycle/classifier evidence, staff ranges, and system-owned erases. | Widen the corpus. |
| 6 | `STEM_SEEDS` | **Native and published** | `recognize_native_stem_seeds` composes live GRID and HEADERS state through lag selection, vertical `StickFactory`, staff/header gating, the concrete checker, fixed-glyph materialization, and free-glyph ownership. Across 30 systems, all 2,425 raw candidates, 422 header skips, 2,003 checks, 97 rejects, and 1,906 accepted glyphs match Java, including bit-exact grades and complete run-table digests. Schema 1 publishes accepted seeds in production order with geometry and exact checker/materialization evidence. The BEAMS adapter and CLI validate and preserve every accepted per-system identity and median. | Widen beyond profile 1 and add tablature/no-staff skip cases. |
| 7 | `BEAMS` | **Native and published** | Native GRID -> HEADERS -> STEM_SEEDS composition feeds the spot chain, system dispatch, beam creation, measured extension, hooks, grouping, and schema-1 output. A fresh-JVM Java counterfactual over 803 final beam/hook inters, 493 groups, and one multiple rest proves actual seeds change zero records on the original eight pages. D039 adds the natural acceptance case: one system-2 beam changes, with endpoint, height, six impacts, and grade bit-exact to Java. The original gate still matches 2,739 spots, 30 erases, and 787/787 raw beams. Production retains exact group memberships and now runs the real MultipleRest pass from a freshly recomputed staff projector: Bach system 6 replaces source ordinal 182 with exact median, grade, height, staff, pitch, and two-serif evidence. | Allocate stable SIG/glyph/relation identities for the retained MultipleRest and serifs, then grade small beams and widen the corpus. |
| 8 | `LEDGERS` | **Native and published** | Native composition consumes GRID's `NO_STAFF`, curved staff/system geometry, and the oracle-free BEAMS result after MultipleRest source-beam deletion. Schema 1 includes all seven impacts, live exclusions, and curved inferred paths. All 581 final Java inters and 95 inferred paths on the eight beam sheets match after sheet-wide one-sigma post-analysis and rebuild. Every final live ledger now retains its exact positioned fixed glyph raster from the referenced filtered sections; Chula's per-system section dispatch is also exact at 2,042/591/961. | Widen beyond the example corpus. |
| 9 | `HEADS` | **Native and published** | The complete production entry point composes live GRID, HEADERS, STEM_SEEDS, BEAMS, and LEDGERS state through prolog, template lookup, seed and range glyph creation, staff duplicate/overlap handling, attachment, small-beam arbitration, and tally analysis. The eight-page top-level differential matches all 3,609 heads entering the epilog, 62 duplicate removals, 2,725 overlap exclusions, 3,547 post-duplicate heads, 191 beam inputs and registered glyphs, 10,053 ordered beam checks by exact per-system hash, 26 head removals, 3,521 final heads, 1,451 tally inputs, and 18 scale rows. Schema 1 publishes identity-free final-head provenance, exact glyph evidence, beam decisions, counts, and scale rows. | Widen the published corpus. |
| 10 | `STEMS` | **Components graded** | Three production boundaries consume live final HEADS, GRID, and STEM_SEEDS state without Java IDs. They match 3,521 heads and all 14,084 corners; purge 1,906 seeds to 1,749 survivors; make 4,182 existing-seed selections; and execute all 9,902 section fallbacks through 18,398 exact section/compound steps, 3,660 subsection attempts, 8,933 registered candidates, and exact new/reuse/accept/reject outcomes. | Port beam/head linker geometry and graph mutation. |
| 11 | `REDUCTION` | **Lifecycle only** | Dependency-light lifecycle and contracts. | Semantic reduction rules. |
| 12 | `CUE_BEAMS` | **Lifecycle only** | Dependency-light lifecycle and contracts. | Cue-beam recognition and linking. |
| 13 | `TEXTS` | **Lifecycle only** | Dependency-light lifecycle and contracts. | OCR, roles, language handling, and SIG materialization. |
| 14 | `MEASURES` | **Lifecycle only** | Dependency-light lifecycle and contracts. | Measure construction and consistency logic. |
| 15 | `CHORDS` | **Lifecycle only** | Dependency-light lifecycle and contracts. | Chord construction and semantic linking. |
| 16 | `CURVES` | **Lifecycle only** | Dependency-light lifecycle and contracts. | Slur, wedge, ending, and curve recognition. |
| 17 | `SYMBOLS` | **Lifecycle only** | Dependency-light lifecycle and contracts. | General symbol classification, checking, and linking. |
| 18 | `LINKS` | **Lifecycle only** | Dependency-light lifecycle and contracts. | Complete relation discovery and conflict handling. |
| 19 | `RHYTHMS` | **Lifecycle only** | Dependency-light lifecycle and contracts. | Voice, slot, duration, and rhythm solving. |
| 20 | `PAGE` | **Lifecycle only** | Dependency-light lifecycle and contracts. | Final page/score assembly and export-ready semantics. |

## Foundations and product surfaces

| Area | Status | Notes |
| :--- | :--- | :--- |
| Rust workspace and CI | **Ported** | Pinned toolchain; formatting, strict Clippy, and workspace tests run on macOS and Ubuntu. |
| Core math and geometry | **Ported and graded** | Histograms, grades, injection, rational/integer helpers, lines, splines, transforms, scale conversions, and the OpenJDK positive-base `pow` path needed for bit-exact weighted grades have parity gates. |
| Raster processing | **Ported and graded** | Run tables, projections, median/Gaussian filters, morphology, thresholding, chamfer distance, watershed, masks, and connected components. |
| Baseline JPEG | **Ported for measured scope** | Pure Rust and bit-exact to Audiveris's bundled libjpeg behavior for supported 8-bit Huffman images. Progressive, arithmetic, 12-bit, and CMYK inputs are refused rather than approximated. |
| PDF ingest and rendering | **Ported for measured corpus** | All 189 pinned pages match PDFBox through filters, rasters, transforms, placement, and rendered grayscale output. Unmeasured PDF shapes are refused by name. |
| Music fonts and header classification | **Ported for current corpus** | 1,624/1,624 header outline-bound sweep values match; clef, key, and time classification is exact on all 65 example staves. Bravura black-notehead widths at arbitrary point sizes and Java's head-width-to-point-size secant are exact and production-wired through every graded staff. |
| Visual classifier core | **Components graded** | Frozen model parsing/inference, features, stable ranking, and glyph construction are native. Remaining size/noise gates, `ShapeChecker`, user overrides, and later-stage integration are not complete. |
| `.omr` persistence | **Components graded** | Opaque round-trip and typed views cover the measured book/sheet metadata and ownership structures. Full native recognition output is not yet an end-user replacement for Java. |
| CLI and JSON | **JSON published through `HEADS`** | Real images and PDFs compose GRID -> HEADERS -> STEM_SEEDS -> BEAMS -> LEDGERS -> HEADS in native Java order for the applicable JSON target; GRID keeps its text report. HEADS documents retain all upstream products and add identity-free final heads, complete seed/range provenance, exact head glyphs, source-resolved beam decisions, counts, and tally-scale rows. `omrscope` consumes bounds-only headers, both median forms, and accepted top-level stem seeds; it refuses rejected or incomplete seed geometry rather than inventing coordinates. |
| MusicXML output | **Not ported end to end** | The differential export suite is queued behind semantic page completion. |
| Desktop UI | **Not ported** | Java Swing remains outside the initial headless milestone. |

## Next work queue

1. Continue STEMS through beam/head linker geometry and SIG relation mutation, preserving stable canonical stump handles and stopping at independently gradeable boundaries.
2. Allocate stable MultipleRest/serif SIG, glyph, and relation identities without changing the graded decision state.
3. Wire GRID's production brace path so Part ownership and merged-staff behavior are exact.
4. Grade additional small-beam pages and widen the published HEADERS/BEAMS/LEDGERS/HEADS corpus.
5. Add end-to-end MusicXML differential grading after `PAGE` is meaningful.

## Maintenance rule

This page is reviewed with every Rust-port contribution and updated in the same
commit whenever parity, stage composition, product exposure, or the next-work
queue changes. Claims here stay deliberately short and must point back to exact
tests or oracle counts in [`rust/PORTING.md`][porting] and
[`rust/HANDOFF.md`][handoff]. A stage moves to **Native and graded** only when it
consumes native upstream state and the oracle is a grader rather than an input.

[porting]: https://github.com/olaugh/audiveris/blob/master/rust/PORTING.md
[handoff]: https://github.com/olaugh/audiveris/blob/master/rust/HANDOFF.md
