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
through `LEDGERS`, including accepted STEM_SEEDS; the human-readable text
report remains at `GRID`. HEADERS is an
oracle-free production call from live GRID state: all 65 staff headers, 34 keys,
17 times, and 30 erase rectangles match Java. The CLI now composes GRID ->
HEADERS -> STEM_SEEDS -> BEAMS -> LEDGERS in Java order into the exact
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
| 4 | `GRID` | **Native and published** | Staff lines, systems, bars, connectors, parts, contextual grades, completed line geometry, and `NO_STAFF` pixels. All 65 staves and 420 barlines in the example corpus match Java. | No known example-corpus gap; continue widening the PDF corpus. |
| 5 | `HEADERS` | **Native and published** | `recognize_native_headers` composes clef, key, and time columns in Java order from live GRID state alone. All nine pages and 65 staves match for starts/stops and selected evidence, including 34 keys, 17 times, and all 30 downstream erase rectangles. Schema 1 publishes selected inters, lifecycle/classifier evidence, staff ranges, and system-owned erases. | Widen the corpus. |
| 6 | `STEM_SEEDS` | **Native and published** | `recognize_native_stem_seeds` composes live GRID and HEADERS state through lag selection, vertical `StickFactory`, staff/header gating, the concrete checker, fixed-glyph materialization, and free-glyph ownership. Across 30 systems, all 2,425 raw candidates, 422 header skips, 2,003 checks, 97 rejects, and 1,906 accepted glyphs match Java, including bit-exact grades and complete run-table digests. Schema 1 publishes accepted seeds in production order with geometry and exact checker/materialization evidence. The BEAMS adapter and CLI validate and preserve every accepted per-system identity and median. | Widen beyond profile 1 and add tablature/no-staff skip cases. |
| 7 | `BEAMS` | **Native and published** | Native GRID -> HEADERS -> STEM_SEEDS composition feeds the spot chain, system dispatch, beam creation, measured extension, hooks, grouping, and schema-1 output. A fresh-JVM Java counterfactual over 803 final beam/hook inters, 493 groups, and one multiple rest proves actual seeds change zero records on the original eight pages. D039 adds the natural acceptance case: one system-2 beam changes, with endpoint, height, six impacts, and grade bit-exact to Java. The original gate still matches 2,739 spots, 30 erases, and 787/787 raw beams. | Grade small beams and widen the corpus. Java's later multiple-rest replacement explains the one retained Bach source beam. |
| 8 | `LEDGERS` | **Native and published** | Native composition consumes GRID's `NO_STAFF`, curved staff/system geometry, and the oracle-free BEAMS result. Schema 1 includes all seven impacts, live exclusions, and curved inferred paths. All 581 final Java inters and 95 inferred paths on the eight beam sheets match after sheet-wide one-sigma post-analysis and rebuild. Every final live ledger now retains its exact positioned fixed glyph raster from the referenced filtered sections; Chula's per-system section dispatch is also exact at 2,042/591/961. | Widen beyond the example corpus. |
| 9 | `HEADS` | **Components graded** | Prolog, spot dispatch contract, classifier mutation order, ownership, cleanup, and quorum scale. `recognize_native_heads_prolog` composes real GRID, BEAMS, LEDGERS, and STEM_SEEDS state into BINARY erasure, Chamfer-3 distance values, 2,790 transient components, and 3,097 per-system spot references; all eight pages match Java exactly through the `NoteHeadsBuilder` boundary. BEAMS' BlackHeadSizer side effect and all 55 staff point sizes are exact. Five native point-size catalogs replay all 32 page-local templates, 192 anchors, and 27,207 keyed pixels, and production HEADS retains each non-tablature staff's exact selection. A native scanner kernel now covers Java's parameters, x/y order, normal shape sets, open predicate, and theoretical ordinate branches. Its strict fixture gate validates all 1,767 geometries and 3,534 schedules without yet claiming production geometry parity. | Connect production staff/ledger adapters and grade the scanner context, then evaluation and interpretation. Competitor/bar slicing and one-line/drum sizing suppression remain separate gates. |
| 10 | `STEMS` | **Lifecycle only** | Dependency-light lifecycle and contracts. | Semantic and visual recognition. |
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
| CLI and JSON | **JSON published through `LEDGERS`** | Real images and PDFs compose GRID -> HEADERS -> STEM_SEEDS -> BEAMS -> LEDGERS in native Java order for the applicable JSON target; GRID keeps its text report. BEAMS and LEDGERS documents retain stem scale and accepted seeds alongside selected headers, system-owned erases, beams, ledgers, relations, groups, and curved paths. `omrscope` consumes bounds-only headers, both median forms, and accepted top-level stem seeds; it refuses rejected or incomplete seed geometry rather than inventing coordinates. |
| MusicXML output | **Not ported end to end** | The differential export suite is queued behind semantic page completion. |
| Desktop UI | **Not ported** | Java Swing remains outside the initial headless milestone. |

## Next work queue

1. Connect and grade production HEADS staff/ledger geometry against the native scanner kernel.
2. Connect seed/range evaluation and interpretation to each scanner context.
3. Grade small-beam pages and widen the published HEADERS/BEAMS/LEDGERS corpus.
4. Complete HEADS scanning/interpretation, then proceed in pipeline order.
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
