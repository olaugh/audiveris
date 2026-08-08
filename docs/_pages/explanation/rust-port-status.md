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

**Current checkpoint:** native recognition is published through `GRID`.
Downstream BEAMS and LEDGERS JSON serializers now preserve their geometry,
grades, impacts, exclusions, groups, and curved ledger paths. `omrscope` now
understands both their horizontal medians and GRID's vertical medians, but the
CLI is still gated at GRID. HEADERS is now an oracle-free production call from
live GRID state: all 65 staff headers, 34 keys, 17 times, and 30 erase rectangles
match Java. The same call feeds the exact downstream eight-sheet gates for 787
raw beams, 581 final ledger inters, and 95 inferred ledger-line paths. Publishing
those native results through the CLI is the next product boundary.

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
| 5 | `HEADERS` | **Native and graded** | `recognize_native_headers` composes clef, key, and time columns in Java order from live GRID state alone. All nine pages and 65 staves match for starts/stops and selected evidence, including 34 keys, 17 times, and all 30 downstream erase rectangles. | Publish header inters and evidence through schema 1, then widen the corpus. |
| 6 | `STEM_SEEDS` | **Components graded** | Lifecycle, stem-scale histogram/peak/fallback, vertical orchestration, and stem checker. Deterministic Java boundaries pin 2,425 raw `StickFactory` candidates and the subsequent production check: 2,003 checked, 1,906 accepted/materialized seed glyphs, 97 checked rejects, and 422 header-gated skips across 30 systems on eight sheets. | Port raw vertical `StickFactory` geometry against the exact candidate oracle, reproduce the accepted/materialized boundary, then connect seeds to BEAMS. |
| 7 | `BEAMS` | **Native and graded** | Native GRID -> HEADERS composition feeds the spot chain, system dispatch, beam creation, measured beam-to-beam extension, hooks, grouping, and schema-1 serializer. The eight-sheet gate matches 2,739 spots, 30 erases, and 787/787 raw beams. | Wire the CLI, connect stem-seed extension, and grade small beams. Java's later multiple-rest replacement explains the one retained Bach source beam. |
| 8 | `LEDGERS` | **Native and graded** | Native composition consumes GRID's `NO_STAFF`, curved staff/system geometry, and the oracle-free BEAMS result. Its schema-1 serializer includes all seven impacts, live exclusions, and curved inferred paths. All 581 final Java inters and 95 inferred paths on the eight beam sheets match after sheet-wide one-sigma post-analysis and rebuild. | Wire the CLI and widen beyond the example corpus. |
| 9 | `HEADS` | **Components graded** | Prolog, spot dispatch contract, classifier mutation order, ownership, cleanup, and quorum scale. | Port and compose the remaining visual spot/classifier internals. |
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
| Core math and geometry | **Ported and graded** | Histograms, grades, injection, rational/integer helpers, lines, splines, transforms, and scale conversions have parity gates. |
| Raster processing | **Ported and graded** | Run tables, projections, median/Gaussian filters, morphology, thresholding, chamfer distance, watershed, masks, and connected components. |
| Baseline JPEG | **Ported for measured scope** | Pure Rust and bit-exact to Audiveris's bundled libjpeg behavior for supported 8-bit Huffman images. Progressive, arithmetic, 12-bit, and CMYK inputs are refused rather than approximated. |
| PDF ingest and rendering | **Ported for measured corpus** | All 189 pinned pages match PDFBox through filters, rasters, transforms, placement, and rendered grayscale output. Unmeasured PDF shapes are refused by name. |
| Music fonts and header classification | **Ported for current corpus** | 1,624/1,624 outline-bound sweep values match; clef, key, and time classification is exact on all 65 example staves. |
| Visual classifier core | **Components graded** | Frozen model parsing/inference, features, stable ranking, and glyph construction are native. Remaining size/noise gates, `ShapeChecker`, user overrides, and later-stage integration are not complete. |
| `.omr` persistence | **Components graded** | Opaque round-trip and typed views cover the measured book/sheet metadata and ownership structures. Full native recognition output is not yet an end-user replacement for Java. |
| CLI and JSON | **Published through `GRID`** | `LOAD -> BINARY -> SCALE -> GRID` is available on real images and PDFs. HEADERS is now production-native; BEAMS/LEDGERS schema-1 serializers are implemented, and `omrscope` accepts both GRID's vertical and their horizontal median forms without inventing incomplete geometry. Downstream CLI/report wiring remains. |
| MusicXML output | **Not ported end to end** | The differential export suite is queued behind semantic page completion. |
| Desktop UI | **Not ported** | Java Swing remains outside the initial headless milestone. |

## Next work queue

1. Publish the native `HEADERS`, `BEAMS`, and `LEDGERS` results and evidence
   through schema 1 and the CLI.
2. Port raw vertical `StickFactory` geometry against the new 2,425-candidate
   STEM_SEEDS oracle, then connect accepted seeds to beam-to-stem extension.
3. Close the visual classifier seams needed by `HEADS`, then proceed in pipeline
   order through the semantic stages.
4. Add end-to-end MusicXML differential grading after `PAGE` is meaningful.

## Maintenance rule

This page is reviewed with every Rust-port contribution and updated in the same
commit whenever parity, stage composition, product exposure, or the next-work
queue changes. Claims here stay deliberately short and must point back to exact
tests or oracle counts in [`rust/PORTING.md`][porting] and
[`rust/HANDOFF.md`][handoff]. A stage moves to **Native and graded** only when it
consumes native upstream state and the oracle is a grader rather than an input.

[porting]: https://github.com/olaugh/audiveris/blob/master/rust/PORTING.md
[handoff]: https://github.com/olaugh/audiveris/blob/master/rust/HANDOFF.md
