# Head-origin StemBuilder oracle contract

This document describes the frozen schema-1 oracle for the full head-origin
`CLinker` `StemBuilder` constructor boundary. The eight checked-in page
fixtures and their manifest were produced by two byte-identical, fresh-JVM
corpus passes.

## Source chronology

Each page runs in a fresh Temurin JDK 25 JVM and reaches real `HEADS`. For each
system, the probe reconstructs `StemsRetriever.inspectStems()` in source order:

1. install the real `StemsRetriever.Parameters`, `StemChecker`, and purged
   abscissa-sorted vertical-seed pool;
2. create beam linkers in `Inters.byAbscissa` order, including the production
   tremolo rejection;
3. create head linkers in `Inters.byAbscissa` order;
4. invoke every real private beam `VLinker.inspect(maxProfile)` exactly once in
   beam / B-linker / V-map order, so the complete beam-origin filament and glyph
   registry timeline is present;
5. visit heads in stable x order and corners in `HeadCorner.values()` order
   (`TR, BL, TL, BR`), invoking each real private `CLinker.inspect(profile)`
   exactly once. Anchors created by earlier corners are therefore visible to
   later corners exactly as in production.

Head-linker construction order is `LT, LB, RT, RB` (horizontal side outer,
vertical side inner), while inspection order is the distinct
`HeadCorner.values()` sequence `TR, BL, TL, BR`. The eight-page freeze executes
the already-pinned 2,417 beam builders before the corresponding system's C
builders; the Chula checkpoint exercises 354 of them.

The seam ends after the `StemBuilder` constructor returns and the C linker's
`sb` field is assigned. It must not perform linking, create `StemInter`s, mutate
the SIG, or populate `StemsRetriever.systemStems`.

## Bounded identity and registration model

The page registry is structural, not based on process-global Java IDs. Glyphs
are keyed by absolute bounds, RunTable orientation, and a SHA-256 of the exact
run sequence. Identity is still checked in-process wherever Java uses identity.

The semantic page baseline consists only of natively projectable structural
contents: every checked `STEM_SEEDS` registration candidate (accepted or
grade-rejected), every persistent staff-line glyph, every live raw beam or hook
glyph, every ledger glyph, and every recognized head glyph. Equal contents are
deduplicated, sorted structurally, and assigned bounded aliases independent of
Java glyph IDs and unrelated `GlyphIndex` contents. A raw beam replaced by a
multiple rest is not live at this boundary and is excluded.

Registration chronology is page-persistent and system-interleaved. Each
system stages its real stump registration attempts, replays its beam-builder
registration candidates, then runs and registers its head-origin chunks before
the next system. This is significant: three later-system beam actions in the
corpus change from the beam-only chronology because an earlier head chunk is
already registered.

The probe runs every real Java registration attempt through a tracking
`GlyphIndex` seeded from clones of the same bounded baseline. Its New/Reuse
result must equal the independently modeled bounded action; disagreement is an
internal hard failure and is not normalized into a second semantic result.
Reuse with no bounded structural origin is likewise fatal. Every new filament
is paired with its canonical registration result in insertion order, and its
members are checked by identity against the system's vertical or horizontal
section vectors.

## Row schema

The checkpoint probe emits these stable row families:

- page/system: source order, profile, parameters, registry baseline;
- beam chronology: one compact event per real beam-origin builder, with
  filament/glyph deltas and a rolling structural registry hash;
- head/corner start: x ordinal, `HeadCorner.values()` ordinal, C alias,
  geometry, start stump, profile, pre-builder registry and B-arena state;
- dense section scans: explicit accepted source ordinals plus reject counts and
  SHA-256 for all rejected decisions;
- filament/member/glyph registration: every accepted StickFactory product and
  every New/Reuse canonicalization;
- head-parts: exact head-overlap removal count, remaining weight, VIP state,
  and the current Java action. The source's VIP-only removal behavior is
  preserved and asserted;
- item/sort/gap/length: constructor result items with exact lines and
  contributions, comparator diagnostics, gap items, and profiles 0 through 4;
- end/system/page: `sb` assignment, allowed anchor growth, forbidden mutation
  deltas, totals, and rolling hashes.

The frozen rows include a complete independent constructor replay: cached
neighbor-seed scans and overlap decisions, C-before-B target lookup,
occurrence-aware chunk alignment, all three JDK 25 mini-TimSort inputs and
outputs, gap insertion/truncation, and independent profile-0-through-4 length
reconstruction. Empty and singleton sorts still emit an audit row.

The current corpus uses inspect profile 1 in all 30 systems and has no profile
divergence. The native boundary deliberately fails closed if the sheet inspect
profile differs from the system profile. It also fails closed for a sort input
of 32 or more elements rather than claiming parity with the large-array JDK
TimSort path; the frozen maxima are below that boundary.

## Fixture split and size guard

The runner accepts exactly one `<path>:<sheet>` target (defaulting to Chula) and
emits one page body per invocation. The checked-in freeze uses one fixture per
example page plus a manifest of source/body hashes and corpus totals. A hard
100,000,000-byte guard applies to every page body.

Dense full-system section rejects are compacted into reason counts and a digest;
accepted sections, all registrations, all items, and all mutations remain
explicit. If a single page approaches the guard, it will be split at system
boundaries without changing row semantics.

## Frozen corpus

The manifest pins the probe SHA-256
`364ad5d74f15c9cbaf77b67da987f6bc3a309c0bd5c80093f34185d6c4ceadd9`
and runner SHA-256
`215410766e419685c6cf3a5c9c8f2c8e7ac39b0f02ef18780f4a67450ae91b37`.
Its own SHA-256 is
`21d8d11beb4a8895759198f17a45a981a66f9554c9559d1711db09f3db7b764e`.

Across eight pages and 30 systems, the fixtures contain:

- 8,939 real stump registration attempts: 5,581 New and 3,358 Reuse;
- 2,417 beam-origin builders and 1,442 registrations: 796 New and 646 Reuse;
- 14,084 head-origin builders and 19,295 registrations: 4,619 New and
  14,676 Reuse;
- 70,420 independently replayed length rows and 42,252 sort audits;
- eight stump action differences from isolated chronology, and three
  head-to-later-beam reuse/action differences; and
- zero forbidden mutations, direction/profile divergences, unmodeled reuse,
  or duplicate occurrence aliases.

The eight fixtures total 593,749 lines and 171,932,512 bytes. The largest page
is Bach at 53,578,058 bytes, below the per-page guard. The manifest stores each
fixture's exact full/body SHA-256, row/byte counts, page FNV hash, and the corpus
algebra used by the Rust gate.
