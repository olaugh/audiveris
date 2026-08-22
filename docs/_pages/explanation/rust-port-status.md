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

Potential defects and source/documentation inconsistencies discovered in the
Java implementation are kept separately in the
[upstream findings catalog](https://github.com/olaugh/audiveris/blob/master/rust/AUDIVERIS_UPSTREAM_FINDINGS.md),
including the evidence and the Rust parity policy for each finding.

**Current checkpoint:** schema-1 JSON recognition publishes every native stage
through `HEADS`, including accepted STEM_SEEDS and identity-free final heads.
`omrscope` now runs Java and Rust concurrently and retains selectable immutable
snapshots as each completes GRID -> HEADERS -> STEM_SEEDS -> BEAMS -> LEDGERS
-> HEADS. The live view is completed-stage streaming only: it does not claim
intra-stage or per-item recognition events, and its opt-in framing leaves the
ordinary schema-1 JSONL interface unchanged. Its graphical Page/Inters audit
surface highlights an inspected pair, can opt into highlighting filtered rows,
and can show only uniquely resolvable engine-local relation edges; it does not
infer a shared Java/Rust graph. A separate manual Score tab runs one selected
Java sheet through PAGE, validates its single explicit local MusicXML/MXL
artifact, and renders it through locally installed Verovio to SVG pages; a
sheet requiring sibling multi-page artifacts is rejected rather than guessed.
This is an inspection of Java output, not visual or semantic parity; Rust
PAGE/MusicXML remains unimplemented, and a future Rust artifact will use the
same renderer before any comparison is claimed. Native `STEMS` now continues
through no-stem seed purging and existing-seed
selection, exact section-built stump materialization and registration for every
head corner, constructor-time `BeamLinker` stump preparation, and exact
`equipStumps`/`equipOrphanSides` B/V topology, lookup geometry, closer-beam
limiting, and seed reachability for every live beam and hook. It now also
constructs the live head-corner topology and performs source-ordered
`inspectVLinkers` beam/head reachability with exact cross-beam anchor mutation,
then takes every beam-origin VLinker through the real `StemBuilder` constructor
and `sb` assignment in production order. The eighth boundary completes
source-ordered head-corner reachability and head-origin anchor mutation. The
ninth now materializes every CLinker-origin `StemBuilder` exactly against the
real system-interleaved page registry. The tenth exact boundary evaluates every
inspected non-anchor beam builder's isolated `VLinker.expand`/link prefix across
profiles 0 through 3 or 4. The eleventh now reconstructs canonical live
beam/hook Glyph identity and Exclusion/competing-hook topology, then replays the
deterministic per-system scheduler prefix in Java width/SIG, side, and V order.
The twelfth resumes the first typed `AwaitingVLinkTransaction` in every system:
it applies prior/pending aliased line deltas, performs exact singleton/compound
selection, structural GlyphIndex registration and `systemStems` lookup, runs
`StemChecker`, and returns/inserts the Java-equivalent stem. Returned median and
mean-thickness bits, integer vertical-ribbon bounds, ID-zero, non-abnormal, and
SIG-null state are exact. The thirteenth joins that committed result to the
frozen scheduler and expand plan, evaluates VLinker's ordered/lazy head-side
stem-reuse loop, and reproduces public `BeamStemRelation.checkLink` through the
exact relation draft or rejection. All 65 real relation entries are unlinked,
so the 30-system corpus performs zero reuse; eight isolated synthetic SIG blocks
provide the exact zero/unique/multiple reuse and numerical edge coverage. It
stops before SIG mutation. The fourteenth conditionally runs `SIG.addVertex` for
an ID-zero selected stem, then applies the already-checked base BeamStem link
through `Link.applyTo` and its synchronous callbacks. All 30 real first
transactions insert a new stem and add the edge; isolated sheets cover the
existing/suppressed and partial-failure prefixes. The fifteenth boundary then
replays that exact predecessor and executes the scheduler-selected outer
B-linker's one shared `linked = true` assignment. All 30 real writes change
false to true, while 32 explicitly isolated envelopes cover idempotence and an
ignored false base-link return. The sixteenth boundary independently reruns that
state and executes the whole serial sibling-beam loop. All 11 real siblings add
their BeamStem edge, complete the synchronous zero-Chord callback, and write the
selected sibling B cell; 64 supported and 16 envelope-only isolated cases grade
the other branches without claiming real-corpus equivalence. It stops at
`ReadyBeforeHeadRelationLoop`. The seventeenth boundary exact-replays that state
and executes the insertion-ordered head-relation map through shared S-cell
writes, duplicate lookup, consistency mutation, direct HeadStem insertion, and
synchronous callbacks. All 65 real entries insert and no duplicate suppresses;
56 isolated supported/envelope transactions grade the remaining compact and
Java-only prefixes without claiming real-corpus equivalence. It stops at
`ReturnedTrueBeforeOuterBLinkerAssignment`. Boundaries 18 through 20 then grade
the caller's outer assignment, SIDES resume, and second transaction. The port
also owns the complete pre-STEMS SIG through HEADS: chula system 1's 221 vertex
and 202 relation tokens reproduce Java's ordered structural hashes exactly. The
human-readable text report remains at `GRID`. HEADERS is an
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

The first two hundred semantic `STEMS` boundaries are production-shaped and graded. Boundaries 1-134 cover the exact construction, scheduler, mutation, head-linking, phase-2, and generic `finalizeStems` behavior detailed below; Boundaries 135-163 complete and publish Batuque; Boundaries 164-166 complete Chula; Boundaries 167-183 complete Allegretto; Boundaries 184-185 complete Zizi; Boundaries 186-191 complete Carmen; Boundary 192 completes Cucaracha phase 1; and Boundaries 193-200 complete Cucaracha system 1 phase 2. Boundary 200 appends x71 to its existing shared stem and exhausts the native 23-item queue. The next live fail-closed frontier is Cucaracha system 2 phase-2 queue 8 x56/SIG78's real append.
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
`materialize_native_stems_beam_stumps` now reproduces the constructor-time
`BeamLinker.retrieveStumps()` boundary from the live post-HEADS beam and kept
seed products. It preserves seed-area geometry, stable cross-x ordering,
duplicate purging, LEFT/RIGHT side classification, full-VLAG missing-side
construction, direction gating, exact fixed-glyph registration/reuse, final
stump and side order, and the tremolo predicate. Across eight pages and 30
systems the exact gate covers 803 constructors, 1,606 sides, 3,934 neighbors,
1,820 seed inputs, and 1,087 purge comparisons (5 removals and 1,082 breaks).
It retains 1,305 side seeds and makes 301 builds: 4 empty-section results, 154
zero compounds, and 143 candidates. Six candidates pass the direction gate and
137 fail; registration produces 5 new glyphs and one canonical reuse across
447 sections and 447 compound steps. The final products contain 1,821 stumps,
1,311 side stumps, and zero tremolos. Probe, runner, emitted-body, and
complete-fixture SHA-256 values are respectively
`98c19499ca486fda8ddec92f18f9e3de54f27041987b011220babbf202dc0039`,
`08964909fa4b7f26ac12c451cfe3a40e4c1ec6cf7ecc2524a2fa11b959175679`,
`18e6431ad73d05f8a72eb1f8e82b8ab047279e2cdc54d0545d7acf3e6bab0899`, and
`902478763d2897eb0d3f031a0895bee7d91a5a7bf8acf8188bf752273e149f14`.
`materialize_native_stems_beam_vlinkers` closes that constructor boundary. It
replays the sequential live beam population, creates 1,821 stump and 295 orphan
BLinkers in exact per-beam registration order, and creates 1,827 stump plus 590
orphan VLinkers in TOP/BOTTOM enum order. The product retains side maps,
stump-linker order, stopping-head side, system/staff/Part limit folds, raw lookup
quadrilaterals and theoretical lines, every closer-beam filter/sort decision,
the optional rebuilt area, and every neighbor-seed intersection. The exact
eight-page gate matches 2,116 BLinkers and 2,417 VLinkers (1,389 TOP and 1,028
BOTTOM), 2,860 Part folds, 9,186 alien candidates with 1,094 survivors and 703
chosen limiters/rebuilds, and 12,491 seed checks with 2,169 reachable seeds.
Two fresh-JVM runs are byte-identical at 46,946 lines and 18,307,148 bytes.
Probe, runner, emitted-body, and complete-fixture SHA-256 values are respectively
`fbc5dace791c84e82db5ff870fb4bcc23e06f29b54619865f19448c0f016a5c2`,
`38e723c15bec6d67c4b856fc40a40d3ee0e4835f466c0c917715c792e6fa1c75`,
`bd43baa197540107e27d2ac97098dbb9df6d6bea1003888ee3625c69e21e60bf`, and
`77cfa1f1d9b6e3f8917ff44db7e3f643ffca690bd639d8a5a93f6fea208a8388`.
The prerequisite also wires GRID's live detached brace portions into exact
two-staff Part ownership and replaces beam-glyph containment with OpenJDK
`Area`/`Order1` crossings. The claim stops before HeadLinker construction and
the source-ordered `inspectVLinkers` pass, where one VLinker can reuse or append
an anchor BLinker on another beam before head filtering.

`materialize_native_stems_beam_reachability` closes that next read-only
boundary after all HeadLinkers exist. It visits all 803 beams, 2,145 BLinkers
(including 29 already-appended anchors that are skipped), and 2,417 VLinkers in
Java order. Across 4,960 sibling scans and 1,617 eligible cross-beam searches it
scans 5,354 BLinker candidates, reuses 1,472 linkers (215 already-created
anchors), and creates 145 anchors, growing the final arenas from 2,116 to 2,261
BLinkers. Head filtering performs 158,886 stable area scans, retains 5,739 head
candidates, rejects 46 for beam distance, checks 11,386 corners, and accepts
5,059 CLinkers after 531 void-side drops. It proves zero competing-head
removals, zero small heads/beams, and zero size drops on the frozen corpus, so
small-head generality remains explicitly unclaimed. Every output preserves
BLinkers before CLinkers, immediate beam-end and final anchor snapshots, raw
`luYLimit`/quadrilateral/theoretical-line bits and seed immutability. Two
corrected fresh-JVM runs are byte-identical at 232,460 lines
and 61,411,164 bytes. Probe, runner, emitted-body, and complete-fixture SHA-256
values are respectively
`39ed0694f7c31593f157b5f250f8bfa4f006984e3b491a877903d64d810edd7b`,
`61801362bc7328cfb3e90f7460016e333d776ee964d39cc296f60cf6edac33f1`,
`470827ebc19065890c41c10016511e77eeefc851823bb8587f7537c7e7db23cf`, and
`9c3f6d17fa6806cba9b01f3922aca34a220d21dc1a5269723e151a025c693221`.

`materialize_native_stems_beam_builders` closes the seventh boundary: every one
of the 2,417 beam-origin VLinker inspections reaches the actual `StemBuilder`
constructor and V `sb` assignment in production order. The constructor's
direction is distinct from the V direction: exactly one differs (Carmen system
2, builder 56), producing 1,390 TOP and 1,027 BOTTOM builders. It removes 215
of 2,169 seeds, retains 6,670 of 6,676 targets (1,617 B and 5,053 C), makes
1,442 chunk-glyph registrations (799 new, 643 reuse), removes 175 chunks, and
retains 9,419 final items with 12,085 length rows. Its sort audit records 18
comparator cycles and 2,503 equivalence inconsistencies; only the JDK 25
mini-TimSort cases are modeled, with maxima of 11 target and 14 final items and
a fail-closed limit at 32. The registry is bounded: external members and
unmodeled reuse are zero there, not a claim of global glyph novelty. All SIG,
system-stem, linker, C-builder, and unexpected-builder mutations are zero. The
emitted body is 91,211 lines / 29,195,732 bytes and the fixture 91,212 lines /
29,197,924 bytes; probe, runner, body, and fixture SHA-256 values are
`c320870ea130e5156124b111e34c918fa4f640595109ac44b8a4de89b732d178`,
`adc2647152b925a2a81fe580a240b4c8be05fca3148ef3d3df29d73577e72806`,
`da4226ee2227d6369054fbce2de4252c72347242253a335132883d9cf871bd22`, and
`a3708e0436184dac5aa63fdb43c70cf05252fa7dbbfd7e9a2d746082e22f2180`.

`materialize_native_stems_head_corner_reachability` closes the eighth boundary.
Across eight pages and 30 systems it visits 3,521 standard stem-capable
black/void heads and all 14,084 corners in TR/BL/TL/BR order. It turns 36,736
ordered seed scans into 1,340 assignments, compacts 1,007,081 head scans into
4,566 C targets, scans 9,015 sibling members into 8,120 B targets, writes all
14,084 C seed lists, and creates 1,687 head-origin anchors. The final B-linker
algebra is 2,116 constructor entries + 145 beam anchors + 1,687 head anchors =
3,948, with C-before-B target order. All C builders remain null; 16,501 builder
checks cover the preceding 2,417 V assignments plus 14,084 C nulls, with zero
forbidden SIG, link-state, or registry mutation. The scope excludes small-head
truncation. Its reachability-only beam prefix omits the prior beam-builder
registry mutations; the ninth boundary resumes the actual beam-builder registry
timeline. `BeamGroupInter.getMembers()` was confirmed to return a
fresh list; the hardened replay still clones and audits identity/order. The
final native HEADS product now also retains explicit non-VIP evidence for every
head; a future true VIP input must be handled or rejected rather than silently
normalized, because Java's `filterHeadParts` bug changes recognition by VIP.
The fixture is 79,216 lines / 37,478,914 bytes; probe, runner, emitted-body, and
fixture SHA-256 values are
`7bac85a2e878d67ccecab9866428a8068b83d1453c2249f49b0c18ae6a17b39f`,
`e9016abb44a500e242b81364531b775fe6b724cddf697cfc0bd4cfe21af0f75d`,
`b3f10b53346adac1309d12fa2d245840a88b02c17e399e88d7e5e36f0358889b`, and
`537cae86c19de20af35a246e03b6edd7f324d0f08c5768b319ed0557a7e28921`.
The normal CI gate is green: two tests, zero ignored; the semantic differential
completed in 33.18 seconds.

`materialize_native_stems_head_builders` closes the ninth boundary. It resumes
the page-persistent registry after MultipleRest replacement and replays, per
system, all stump attempts, the 2,417 beam builders, and then every head-origin
builder. The bounded registry represents only structurally projectable live
glyphs; it does not claim the contents or IDs of Java's global `GlyphIndex`.
Across eight pages / 30 systems, all 14,084 C builders materialize through
15,953,076 vertical and 14,436,784 horizontal section scans. They create 19,295
filaments from 45,938 members and register them as 4,619 New / 14,676 Reuse,
then retain 29,120 items and insert 165 gaps; the exact gate matches all 70,420
lengths for profiles 0 through 4. The complete chronology also contains
8,939 stump registrations (5,581 New / 3,358 Reuse), 1,442 beam registrations (796 New /
646 Reuse), eight stump action changes, and three head-to-later-beam reuse/action
changes.

The 42,252 JDK 25 small-list sort audits include 8 comparator cycles and 319
equivalence findings. Frozen retrieve-seed / target / final list maxima are
2 / 7 / 13; an input of 32 or more fails closed. Every system uses inspect
profile 1 with no divergence, and production rejects an inspect/system-profile
mismatch. The corpus has no VIP heads, but Java's VIP-only `filterHeadParts`
bug remains exact: 6,087 low-remain non-VIP chunks are kept. The shared vertical
`StickFactory` likewise preserves processed-without-compound semantics, allowing
a thickening side to remain eligible as a later isolated sticker. SIG,
`systemStems`, link-state, and unexpected-builder mutation counts are zero.

The split fixtures total 593,749 lines / 171,932,512 bytes. Manifest, probe,
and runner SHA-256 are
`21d8d11beb4a8895759198f17a45a981a66f9554c9559d1711db09f3db7b764e`,
`364ad5d74f15c9cbaf77b67da987f6bc3a309c0bd5c80093f34185d6c4ceadd9`, and
`215410766e419685c6cf3a5c9c8f2c8e7ac39b0f02ef18780f4a67450ae91b37`.
The normal eight-page full native semantic-stream gate passed independently in
84.48 seconds and again in root verification in 88.93 seconds; strict
integration-test Clippy is green.

`materialize_native_stems_beam_link_plans` closes the tenth boundary. From the
immutable completed builder state, it evaluates each of the 2,417 inspected
non-anchor beam builders at every construction profile—0 through 3 for stumps,
0 through 4 for sides—while retaining the effective system link profile. The
exact eight-page / 30-system gate covers 11,573 plans: 2,903 `NoHeadTarget`,
289 `ExpandFailed`, 2 `NoRelations`, 58 `NoGlyphs`, and 8,321
`ReadyForCreateStem`, with 18,345 final relations and 12,523 final Glyph
entries. It also matches all 578 gap decisions, 9,869 separation checks, 18,416
relation attempts, and 37,683 Glyph updates. Two relation attempts derive a
dynamic head side different from their C corner.

The product makes Java's path-dependent behavior explicit without mutating its
predecessors: 3,226 downward calls would shift the shared V/StemBuilder
theoretical line and current beam attachment, while 49 gap rewinds restore the
Glyph set but retain a bit-different working line. Forbidden graph, index,
linker, and builder mutations are zero. The profile-4 terminal-head javadoc is
not enforced by Java: ready plans split into 9 with no stopping head, 632 that
return beyond the last valid stop, and 645 that return at it.

The split fixtures total 120,724 lines / 104,056,316 bytes. Combined body,
manifest, probe, and runner SHA-256 values are
`ac0fcb9880dbf720c8b73e6baf02867d05e0f2d5a62f208f52e9fa7d5c764966`,
`f511b049cf5e32de6fb0151a36a1385efb78b4965fd704c7545eaef8522a2f87`,
`2a5e107f947e140e030f3cc1dff06105ab730af3e41381e76f5f8113a17b0fa2`, and
`a73ed3977662427062b8d81ac8796ffa54d51daa2f97ea1f109a3d606d0c13b7`.
All 120,646 body lines, comprising 120,636 semantic rows plus the 10-line shared
header, pass in independent 32.25-second and root 32.41-second runs; 11 focused
unit tests and strict integration-test Clippy are green. This isolated matrix
deliberately does not select the live scheduler attempt.

`materialize_native_stems_beam_scheduler_frontiers` closes the eleventh
boundary. Per system it reconstructs exact page-global beam/hook Glyph identity,
the ordered live raw hook/full-beam Exclusions and first matching competitor,
then stably sorts by decreasing integer width while retaining SIG order across
ties. It replays LEFT/RIGHT side order, TOP/BOTTOM V order, target prechecks,
profile choice, and local worklist removal against the frozen expansion plans.
The eight-page corpus covers 30 systems, all 803 beams, 322 width ties, 651
canonical live Glyph aliases, and 78 live hook/full-beam pairs.

There are 56 attempts: 26 empty-target precheck skips remove 14 beams only from
local worklists, then the first invoked plan in each system is one of 30
`ReadyForCreateStem` plans. Ready is not a successful link. Each system instead
stops at a typed `AwaitingVLinkTransaction` before `createStem`; 14 pending
downward stored-line/current-attachment deltas are emitted but not applied. The
corpus invokes zero known-false plans and reaches zero stump rows, hook-removal
transactions, shifted-V retries, or completed systems. It performs zero
GlyphIndex, `systemStems`, SIG/relation, link-flag, stored-line, attachment, or
other persistent mutation.

The combined oracle body has 998 lines / 460,651 bytes—993 semantic rows plus
the five-line shared header—and SHA-256
`8ff44c35d8c1e2334c56c4d7e546fdaacbcb2964a1ab6103168f25346e041ff1`.
Manifest, probe, and runner SHA-256 are
`b6b77cdead537a70b482ae7ef5d5c8312cc5993529382f1f39fb4779afa7abb2`,
`afb5c564a474bc0c227b9fdc886cf892c60ae39aa62c1d93cef8aaf610b90fba`, and
`2d5609b5c5ef713aa3fda6467d000fad89cd8147e97d1541b5060305b414c99e`.
Eight focused production units pass. The normal integration suite is 3 passed /
0 failed / 1 ignored in 31.09 seconds: parser drift, expand-fixture provenance,
and the full exact corpus gate are active, with only the fast Chula diagnostic
ignored. The independent root full gate passes in 31.41 seconds, and strict
integration-test Clippy is green.

`apply_native_stems_beam_vlink_create_stem_transaction` closes the twelfth
boundary. It resumes one first `AwaitingVLinkTransaction`, commits any prior
deferred known-false and selected pending aliased line/current-attachment
deltas, selects the singleton Glyph or constructs the exact vertical compound,
performs structural `GlyphIndex.registerOriginal` and `systemStems` lookup,
runs the exact `StemChecker`, and returns/inserts the reused, checked, or
profile-4 artificial stem. Rejection is a committed-prefix outcome: earlier
line and GlyphIndex changes do not roll back.

The exhaustive GlyphIndex certificate is candidate-specific and one-shot;
bounds and full RunTable content define equality, while hashes are provenance
only. Production also supports `systemStems` Present/reuse inside `createStem`.
Only the compact v1 real-fixture loader refuses to hydrate a Present system-stem
certificate, which is unrelated to VLinker's following head-side stem-reuse
loop.

Across eight pages / 30 systems and transactions, there are 15 compound
candidate objects with ID 0 before registration and 15 singletons. Fourteen
line deltas commit. All 30 exhaustive Glyph lookups are Present and active, so
all register operations are `ReuseActive`; all 30 real system-stem lookups are
Absent; and all 30 results are `CreatedChecked`. Every returned median endpoint
and mean-thickness value matches Java by exact binary64 bits, and every integer
vertical-ribbon bound matches exactly. All returned Inter IDs are 0, abnormal
flags are false, and SIG attachments are null; allocator, SIG, relation, and
link-flag deltas are zero. The real corpus does not exercise new/reinsert
registration, artificial creation, rejection, or existing-system-stem reuse;
focused synthetics cover those branches.

Only system 1 of each page—the eight system-1 transactions—is true sheet-first
chronology. The other 22 transactions use an isolated fresh-sheet/system JVM
and grade that local frontier; they are not a fabricated serial page-global ID
chronology. Every page was generated twice with one foreground JVM at a time,
and both passes are byte-identical.

The reconstructed body is 261 lines / 153,517 bytes—256 semantic rows plus the
five-line shared header—with SHA-256
`0c8c51e1c170a0dc3ec7e5910e6dca63a82f7d8fe6699b585c9556f183b359dc`.
Manifest, probe, runner, and manifest-body SHA-256 are
`b7e6fe6e7dc2f5eeba106133c930249f20e2c75d764704252289724bbe28c3e0`,
`36fecabe18d7713c823ce6990dae717e78997354a9ae0b142cba55f7d75004f3`,
`6d95ff62d0acb502d531d6fb2aea0382fcb9dcb8fdd871fb7b0e2fba2ffb1de8`, and
`67d983b056548118015f5b7d18a9e2772860e08e0d2ab076118b25a9678c40af`;
the manifest body is 9 lines / 5,691 bytes. Eleven focused production units and
the active 5 passed / 0 failed / 0 ignored exact/synthetic gate are green; the
full 30-system run completed in 31.98 seconds, with strict library and
integration-test Clippy green.

`evaluate_native_stems_beam_vlink_reuse_check` closes the thirteenth exact,
read-only boundary. Starting from the committed `createStem` result, it preserves
the relation `LinkedHashMap` order, lazy shared S-linker linked-flag reads,
`HeadInter.getSideStems()` relation order and absent-key invariant, first-unique
reuse break, multiple-stem continuation, and explicit unread suffix. It then
reproduces public `BeamStemRelation.checkLink`: raw intersection bits and
non-finite propagation, strict beam-portion tests, `Math.rint` max-dx, scale
stem thickness in the x-gap half-width, y gap outside the stem-median endpoints,
raw/clamped 1/4-weight impacts, intrinsic ratio 1, inclusive grade 0.1, and the
extension point/outgoing base-relation draft.

Across eight pages / 30 first transactions, all 65 ordered relation entries have
an unlinked S-linker. There are therefore zero head-side scans, live scan stems,
or real reuse selections; all 30 relation checks accept and none reject. This is
an exact real-corpus reuse census of zero. It does not claim that the reuse
branches occurred naturally.

A bounded later-transaction reconstruction supplies one real linked-S path.
Allegretto system 1 transaction 28 / plan 25 traverses the single live HeadStem
edge 229, selects the modeled attached StemInter with Java ID 2227, and breaks
before reading relation-map entry 1. The projector derives Java's exact snapshot
and projection hashes from explicitly reconstructed native SIG, binding, S-cell,
and system-stem inputs before the fixture is opened, and leaves all inputs
unchanged. The gate does not replay native transactions 1-27. This extends
Boundary 13 coverage; it does not claim native predecessor carriage, B14 reuse,
or general linked-S coverage. The separate fixture is 10 lines / 2,566 bytes,
SHA-256 `287175a58717874882bc6487f7d59ea86a22e44cadcac003ee99a36606e5ab34`.

The original first-frontier corpus also retains one system-1
`IsolatedSyntheticSig` block per page.
The eight blocks use actual isolated SIG vertices with positive non-production
IDs, actual `HeadStemRelation` edges, and actual `HeadInter.getSideStems()` calls
without consuming the real sheet allocator or InterIndex. They exactly cover
zero, unique, and multiple side-stem cardinalities, lazy break and absent-map
behavior, accepted/rejected check triples, portion ULPs, threshold equality,
parallel/non-finite intersections, and zero mutation. These blocks are synthetic
branch evidence only. The production boundary itself records zero persistent-ID,
`systemStems`, SIG-vertex, SIG-relation, and linker-flag mutations and stops before
conditional `SIG.addVertex`, base BeamStem link application, sibling beams, or
relation-loop/head links.

The concatenated corpus body is 601 lines / 472,445 bytes—553 semantic rows plus
48 repeated page headers—with SHA-256
`76a6d20865a5a372bb6485ff6debeb0c435b64d1f92cf5ee07e1fbe0cf61418f`.
Manifest, probe, runner, and manifest-body SHA-256 are
`4ab7078b760daca6691fcc03e8f29684ec4c976f918d747cb2047f01accd0559`,
`3ab243141f6eda3028885e3d73946c129e62554d5abc14658ca6e786f38650b0`,
`1b4913e1fc8f2665383635fac3e7c3c16f7de369ff8da5db4b4fe57e1b29ac21`, and
`58259448c36c5c684cbfef2215eb124a2ca62e5aae8f12d1a73510345687fb6d`;
the manifest body is 9 lines / 9,202 bytes. The manifest pins Java
`BeamLinker`, `BeamStemRelation`, `HeadInter`, `HeadStemRelation`, `LineUtil`,
`Scale`, `AbstractConnection`, `SupportImpacts`, `Support`, `GradeImpacts`, and
`GradeUtil` at
`131f91f6605ecf03463ef4b6021a461240f99d7dfe2b1a1b94b0213d158d1747`,
`3ceff58fa9b298d97f325372d0e5a9b363755f3ad47cac7b66b07bd8d1e735f1`,
`ce32f3497972606ec696f59928e51bc9b057e74f13dcbc7306a73f7c46d99fda`,
`f8828725da97dc44d9bb350adbb8e1055eb73934d0d0386e54e8d95994070eef`,
`3644b4c4ffd627bf554c8dd4045ba273f2cb7f7a6e938d8d68c45540844405cb`,
`25ab64d3a18063bd5cc5249c05c649e3cff27c79b69ce3a501515329276fecfa`,
`bd11a796c1d176f42b087e31c23bffb004eca7cad4749a0f36ddff3573265f81`,
`8bbdaa99a990ded65c69aee9e99e8eb0deb82506a6c620d78ab3f4372953a8f3`,
`8b6171dd1b98b842e8defcd9758e6003d315534ac3ae79864ccc2309e94ad4af`,
`f0b90aad2d26675f4518153e6395d8c528960b146f1a32ca5d272d5297d7e840`, and
`e7fedd800456c64d7906ba252ee5e6a3881ab9dc3cf4da07a7a0913dbbcb6597`.
Two byte-identical runs per page used 8 compiler plus 60 runtime foreground and
reaped JVMs, maximum Java concurrency 1, and no background Java process. Eight
focused production tests and all 8 exact integration tests pass; the independent
root gate finishes in 32.66 seconds, with strict boundary-13 library/gate Clippy
and global formatting green.

`apply_native_stems_beam_vlink_base_transaction` closes the fourteenth exact
production boundary at Java's first stateful continuation. Starting from
boundary 13's `ReadyBeforeSigMutation`, an ID-zero selected stem takes the
conditional `SIG.addVertex(stem)` prefix through shared InterIndex allocation,
VIP lookup, source-ordered vertex insertion, `setSig`, the sole `SigListener`,
`StemInter.added`, abnormal-state change, and SheetStub/Book modified and dirty
propagation. An already attached positive-ID stem skips that prefix.

It then applies the fresh checked `BeamStemRelation` to the beam with
`Link.applyTo`. Source/target-removed suppression, the ordered outgoing duplicate
scan, separate draft-object and graph-edge identities, exact endpoint/incidence
mirror joins, raw nullable BeamStem/BeamRest portions, JGraphT insertion order,
and synchronous callbacks remain exact. `BeamStemRelation.added` scans the
post-edge stem incidence and certifies zero `ChordStemRelation` matches in
compact v1, then invokes the beam's virtual abnormal check. Full beams inspect
ordered BeamStem/BeamRest portions; hooks use their class-only any-BeamStem rule
without reading a portion. The ignored Java apply result, actual abnormal/dirty
side effects, and no-rollback exception prefixes are explicit.

All 30 real first-frontier transactions across eight pages are `NewIdZero`:
all 30 conditionally insert a stem, all 30 base BeamStem edges are added, reuse
is zero, and ChordStem matches are zero. Each page also contributes five
supported cases and four envelope-only cases on a truly isolated Sheet/System/SIG,
for 40 supported branch cases and 32 partial-failure prefixes. Those 72 cases
are isolated evidence, not naturally occurring corpus behavior or a blanket
production-equivalence claim.

The normalized corpus is 1,314 lines / 1,185,901 bytes—one shared eight-line
header plus 1,306 semantic rows—with SHA-256
`ece76c038ef1b2017d2f356dd6ead59379376ffc5ab0306e8c5e8c34a9471e53`;
the eight split fixtures total 1,386 lines / 1,227,749 bytes. Manifest, probe,
runner, and manifest-body SHA-256 are
`5da20f701d38bf9b81c6000ed4e8aba4fadd285c85d81753ef4a862f0a4875bc`,
`2139f0f5c2aba399d2eb8bc10ccbc2ec1221ce00ae2fdeb50782c80622f982e3`,
`88091fd27bef445f7045b721a6258da9652bac2f68d1ced277bbe82c1640d9b5`, and
`8bbd189d9c7e82702ce8513347841cfe5aff2f96f8b39bf9dd07e05bea4e6b35`;
the manifest body is 9 lines / 16,479 bytes. It pins all four predecessor
fixtures per page, the complete active Java/Gradle source set, and JGraphT core
1.5.2 at
`dfa596e9f0d0838f1b5e81dd0cd60e3a76c2c290ac25a0a029ffde58cf5e4c14`.
Seam-critical `BeamLinker`, `Link`, `SIGraph`, `SigListener`, `BasicIndex`,
`InterIndex`, `StemInter`, and `BeamStemRelation` source hashes are
`131f91f6605ecf03463ef4b6021a461240f99d7dfe2b1a1b94b0213d158d1747`,
`e27734fa0f4273db91527ed969ef1881605cda32eb970bb464ea037b0f0ed34e`,
`6b6ff3172d1f194566a7f59aa2c854cb62ea9c4deab79a43b6b0b85e1d4c4c2f`,
`19b42c96257bd78fc9d4bc428242590ae01832b395aebdeefe26e081ceadc08d`,
`7c747248365477c9381d004891e88f96273c0796a26f7417192fdaaeac8d3707`,
`830ee77262bd9b631d352e49ddc150055e621ad9cd76c2a0671fc2233b662b7a`,
`bcdb1b67694f45de89a9ad8712222e77af7c6e29247f5edd487d8dcabd11eeec`, and
`3ceff58fa9b298d97f325372d0e5a9b363755f3ad47cac7b66b07bd8d1e735f1`.
Two byte-identical passes per page used 8 compiler plus 60 runtime
foreground/reaped JVMs, maximum Java concurrency 1, and no background Java.
Twenty focused production tests and all 10 full exact integration tests pass;
the latter finish in 33.87 seconds. The full library suite is 623 passed / 0
failed / 2 ignored in 12.47 seconds. Strict Clippy, global formatting,
diff-check, and oracle `sh -n` are green.

`apply_native_stems_beam_vlink_b_linker_flag_transaction` closes the fifteenth
exact production boundary. It starts at `ReadyBeforeBLinkerFlagMutation`, clones
the exact pre-boundary-14 state, independently reruns the whole base-application
transaction, and requires the supplied transaction and resulting state to match
before it writes. It resolves the scheduler-selected outer B-linker plus the
exact TOP-then-BOTTOM `EnumMap` order of every V child observing the same cell.

The Java seam is one unconditional plain `getBLinker().setLinked(true)`
assignment. The result retains the ignored base-link return and fresh draft
support grade, distinguishes one completed write from a false-to-true value
change, and proves zero S-linker, sibling, head, ID, index, SIG, stem, beam, or
sheet-edit mutation. All 30 real transactions change false to true. Their live
Java arena contains 3,948 B entries: 2,116 frozen constructor entries and 1,832
later dynamic anchors. The full arena is oracle/gate guard evidence; compact
production state models only the exact selected shared cell rather than
fabricating unrelated Java objects.

Eight page-local blocks contribute 32 explicitly isolated
`UnsafeExactClassNoGeometry` setter-and-shared-cell-only envelopes: 24
false-to-true, 8 idempotent true-to-true, and 8 retaining `applyReturn=false`.
They do not claim reachable geometry or blanket production equivalence. The
normalized corpus is 4,562 lines / 2,535,981 bytes, SHA-256
`6125665f38d894f6b05a24651f56f0a38c01e2acc2a7d18167a4175d5ae81c34`;
the split fixtures total 4,634 lines / 2,590,657 bytes. Manifest,
manifest-body, probe, runner, and effective-classpath SHA-256 are
`c7032ac4871188ef0cf48ac63d99996e78a0e163bf1470d3be84c5e9b10d1d92`,
`3f332e7751d5de73e296294ccc6882ff6a578d0328b8c0d717c96666ffbb3e4d`,
`b4c750370bebda13e66c49a8cc88756cb677ebf04f77d7dae883cb373fe431a8`,
`066a5ee494c583bdc7e9df1fc6e282015afc7663968b5e0a836219e545d14c24`,
and `fd4e52c2275675a53459dff2b2e2d89636f3c5fb6ab5a1f7be65f74157663fb3`.
The complete manifest is 10 lines / 24,897 bytes and its authenticated body is
9 lines / 18,910 bytes. Two byte-identical passes per page use 8 compiler plus
60 runtime foreground/reaped JVMs with maximum runner-scoped Java concurrency
1 and no background Java process.

Seven focused production tests and the shared 5/5 exact hydration regression
pass; the latter finishes in 126.03 seconds. The terminal is
`ReadyBeforeSiblingBeamLinks`.

`apply_native_stems_beam_vlink_sibling_links_transaction` closes the sixteenth
exact production boundary. It starts from the exact pre-Boundary-15 state,
independently reruns the complete flag transaction, and requires the supplied
transaction and state to match before committing sibling state. It then executes
the whole serial Java `linkSiblings(stem, grade)` call and stops immediately
before the head-relation entry-set loop.

The transaction reconstructs the exhaustive BeamGroup outgoing-Containment scan,
preserves insertion order, performs Java's stable top-down intersection sort,
and removes the base beam by object identity. Per sibling it preserves glyph-
object identity skips, the first runtime-class duplicate break, lazy shorter-
beam ordinate reads, exact extension/portion/grade, and serial graph-relation
identity. A fresh edge synchronously scans the stem with zero `ChordStemRelation`
matches, applies the raw-beam LEFT/RIGHT or hook any-BeamStem abnormal rule, and
records any SheetStub/Book dirty cascade. Only after the callback does the exact
ordered `StemBuilder.items` lookup optionally assign the first source-identical
sibling B-linker cell. Group-member post-state and the complete edge-callback-
flag chronology remain explicit.

The 30 real transactions expose 58 outgoing Containment members, all with
non-null native glyph identities and exact run-table tokens. Their 11 real
sibling candidates all take `Linked`, add an edge, complete a callback, and write
a B cell; real same-glyph, existing-relation, shorter-wrong-side, and ChordStem
counts are zero, and the seam records 33 ordered real events. Eight page-local
isolated blocks add 64 supported cases—`SameGlyph`, existing relation, shorter
wrong side, full/small/hook links, no B linker, and idempotent B cell—and 16 Java
throw envelopes. These are supplemental gate-only branch/failure evidence, not
production-equivalent real transactions; false `addEdge` return remains an
independent-model case because stock Java has no honest live fixture for it.

The normalized corpus is 717 lines / 580,329 bytes—one shared 8-line / 753-byte
header plus 709 semantic rows—with SHA-256
`c6a62f9b98ce55eda2bd142b083a2ff6b14d08dab6b1a2ce3c1a0d643d5efd66`;
the split fixtures total 789 lines / 654,858 bytes. Manifest, manifest-body,
probe, runner, and effective-classpath SHA-256 are
`6dcca78c13facf7fa9ee29506eab2961d1410babf396930724dce16f5474e29d`,
`c5d44bf655814aac1a297d4ad67fe401291449e231d581d11c812e197ef0fba0`,
`a3ee02cf29f5a8a7c70bd7b2e064d7a1ff0fee2d120bde3b2088c7f2db98eda0`,
`9d2535980f191105d912ec2e07c99e3f06f55b1c406a68da610f1685ec07e1a5`,
and `fd4e52c2275675a53459dff2b2e2d89636f3c5fb6ab5a1f7be65f74157663fb3`.
The complete manifest is 10 lines / 31,471 bytes and its authenticated body is 9
lines / 23,218 bytes. Two byte-identical passes per page used 8 compiler plus 60
runtime foreground/reaped JVMs—68 total—with maximum runner-scoped Java
concurrency 1 and no background Java process.

Twenty-two focused production tests and all 10 full exact integration tests
pass; the full gate finishes in 126.68 seconds. The shared Boundary-15 hydration
regression is 5/5 in 126.03 seconds. The full library suite is 652 passed / 0
failed / 2 ignored in 11.92 seconds. The terminal is
`ReadyBeforeHeadRelationLoop`.

`apply_native_stems_beam_vlink_head_links_transaction` closes the seventeenth
exact production boundary. It independently reruns and exact-joins Boundary 16,
then executes the insertion-ordered head-relation map. Each entry writes its
shared parent S-linker cell before the complete directed duplicate query. An
existing `HeadStemRelation` skips every later read; a missing relation lazily
computes and writes consistency on the existing plan draft, inserts that draft,
and runs the synchronous head-then-stem abnormal/dirty callback. Compact
production requires exact live endpoints, sole standard listener topology,
prepopulated head side/extension, and non-manual relation/head/stem state.
Default metadata, manual chord rewiring, and Java fault prefixes remain isolated
gate evidence. The inert remainder comparison is retained, and the method
returns true before its caller assigns the outer B-linker.

The 30 real transactions contain 65 entries, zero duplicates, 65 inserts, 65
S-cell writes, 65 consistency writes, and 260 ordered events. Eight isolated
blocks add 16 supported and 40 envelope transactions—56 total / 304 events—with
40 graph deltas, 16 throws, 16 manual cases, and 8 chord rewires. These are
supplemental branch/failure evidence, not production-equivalent transactions.

The normalized corpus is 1,583 lines / 785,671 bytes with SHA-256
`b57ec3f2bf401fce6d6d62c7522285dd3288b35b40d7c5c453468cf5dde4ce48`.
Emitted split bodies are 1,639 lines / 790,438 bytes with SHA-256
`044631a9dc5177b3fbe074a03cc031f52cb6087b3ea3491377f820d633b44d01`;
full split fixtures are 1,655 lines / 873,975 bytes with SHA-256
`6e9abd60f5274622bd9638cc6e1cd6c489ee5fdc36ec96769507ef9f16f418aa`.
Manifest, manifest-body, probe, runner, and effective-classpath SHA-256 are
`87b1f5fb459551cb247f4702449128f35d94ac5ee738d764e25e523dd21955ab`,
`a7934a066b47654b56184e6506825d9f1f5986d96f25b3eb52b2281308185a08`,
`3e6dd42af58f074d6f9a146dd00c3573fc4c79c445eda629bc82f93d175df61a`,
`932084cef5c8d5b700cdda1ce3ddb48e5454fe8f65775a9d7fed52070c7a1d42`,
and `fd4e52c2275675a53459dff2b2e2d89636f3c5fb6ab5a1f7be65f74157663fb3`.
The complete manifest is 10 lines / 35,839 bytes and its authenticated body is 9
lines / 25,997 bytes. Two byte-identical passes per page used 8 compiler plus 60
runtime foreground/reaped JVMs—68 total—with maximum runner-scoped Java
concurrency 1 and no background Java process.

Twenty-four focused production tests and all 13 full exact integration tests
pass; the full gate finishes in 148.82 seconds, and the standalone manifest
validator passes 1/1 in 129.11 seconds. The full library suite is 685 passed / 0
failed / 2 ignored. The owned SIG now answers fail-closed incoming, outgoing,
incident, and directed-pair queries in Java/JGraphT order; its real chula
base-apply beam scan matches the frozen Java rows exactly. The next production
graph-carriage layer now adds typed stable IDs, dense checked appends,
insertion-order tombstones, abnormal updates, BeamStem portion payloads, and
typed beam-source bindings. Public B14 now consumes its production projector
directly for chula system 1: the compact state, owned SIG, and typed bindings
commit atomically, appending Stem vertex 221 and LEFT BeamStem edge 202 with
the exact support grade and abnormal updates. Endpoint certificates use
explicit one-based native vertex identities rather than a fixture-derived
Java-ID map, while the frozen Java corpus remains unchanged. The first measured
B15+B16 carrier now resolves the native BeamGroup and Stem, derives exact group and
sibling geometry, selects immutable builder items, and serially commits each
edge/callback before its shared-cell write. Sibling 1 observes edge 203 before adding
204; the graph ends at 222 vertices / 205 edges, and the owned cell catalogue records
the B15 base assignment plus `beam:0:b:0` and `beam:1:b:0`. Typed member/abnormal
snapshots replace opaque Java group digests. The carrier commits SIG and cells together,
and invalid input leaves both unchanged. Its 12/12 gate reads the B16 oracle only after
the native result returns. This is chula system 1 transaction 1, not a full self-driving
B16 corpus. The same bounded carrier now initializes persistent S cells from the complete
native head-corner topology and commits B17 edges 205-206 in map order. Native heads
119/120 link to stem 221 with bit-exact consistency; their two LEFT cells become linked,
and the graph ends at 222/207. A late second-entry fault rolls back SIG and S cells.
That B17 seam remains chula system 1 transaction 1 and does not own general dirty-state
effects. The same carrier now crosses B18/B19: native V topology drives
the idempotent outer B write, B16 sibling cells are folded before scheduler walking, and
the exact plan-152 RIGHT-side second frontier is reached. Failed resume leaves the shared
B-cell authority unchanged. Transaction 2's B12 preparation is now production-owned:
the plan derives its line and selected-glyph state, exact native glyph content joins the
disclosed page GlyphIndex bootstrap, and a private dense-history token establishes
`systemStems` completeness. The atomic preparation rejects ambiguous bootstrap evidence,
then B12 reaches ReuseActive / CreatedChecked before any txn2 family oracle is opened.
The bounded B13 projector also validates native head bindings and reads the two plan-152
S cells first; because both are false, it records exact `NotRead` graph lookups and reaches
AllUnlinked / ReadyBeforeSigMutation without oracle rows. A native rollover then folds the
prior InterIndex append, recomputes the 222/207 graph baseline, and commits transaction-2
B14 as Stem vertex 222 plus RIGHT BeamStem edge 207 before comparing the frozen result.
The same persistent arenas then carry B15-B17: sibling edges 208/209 and B cells
`beam:2:b:0`/`beam:3:b:0`, followed by HeadStem edges 210/211 and S cells
`head:21:LEFT`/`head:22:LEFT`. The owned graph reaches 223/212 before the txn2 fixtures
are opened. Transaction 2 then crosses B18/B19 from the same authorities: the outer B
write is idempotent, sibling cells are folded before the scheduler walk, and transaction
3 is reached at plan 618 / `beam:22:b:0` / TOP before the frozen SIDES row is opened.
Transaction 3 now crosses the first changed-base/compound case without transaction rows:
a one-time first-STEMS bridge resolves the plan-618 compound to canonical glyph 298 while
B12 creates a new checked stem. The frozen authority describes all 48 live beams, but the
carrier consumes only the 16 distinct selected bases that reach B14 across the 32
transactions; those rows supply Java Inter ID/InterIndex ordinal/VIP and all graph
and group facts remain native-derived. B14 adds Stem vertex 223/edge 212, B16 adds edge
213 and `beam:41:b:0`, B17 adds edges 214/215 plus two S cells, and B18/B19 reaches plan
627 / `beam:22:b:2` / TOP with the graph at 224/216. The production
`advance_native_stems_beam_sides_transaction` owns each already-awaited frontier as one
clone-and-swap across scheduler, latest B14/transaction state, SIG/bindings, and B/S
cells. Repeated calls now execute all 32 chula-system-1 SIDES transactions and return the
explicit `SidesExhausted` scheduler at 253 vertices / 331 edges, 32 Stem bindings, 61 linked/open B
cells, and 68 linked/open S cells. Exact plan/B-linker order and all 29 sibling-write
lists match Java only after that native terminal exists; all 21 skipped sides are thus
explained by earlier native B16 writes. A late B16 failure leaves the complete carrier
unchanged. The bridge maps the 1,058 system-1-visible native modeled objects into one
disclosed 1,650-entry persistent snapshot and retains 592 opaque fingerprint-only entries.
Transactions 3-32 use it without per-frontier selected-glyph rows or exhaustive scans;
opaque entries never answer equality or absence. Transactions 1-2, persistent IDs and
allocator/union state, native predecessor carriage plus wider coverage for the reconstructed Allegretto linked-S/hook-removal path, wider-corpus STUMPS authority and branch coverage, and later STEMS phases
remain fixture-backed or unimplemented.

The carrier now crosses the SIDES-to-STUMPS seam without persistent mutation. Chula
system 1 supplies 34 retained beams. Beam SIG 12 begins event 0; stump 0 is both a structural
side stump and linked, and Java's structural test wins at event 1. Unlinked stump 1 reaches
plan 147 at `BEAM_SEED` profile 3 / link profile 1 with two relations, one glyph, and no
line change, then event 2 stops at `AwaitingVLinkTransaction` before `createStem`. The
native result represents that Java event-2 attempt as its typed frontier after two
scheduler event records.
The real prefix contains no pure already-linked skip or known-false plan, so those branches
are not claimed as natural coverage. The separate five-row-plus-summary fixture is 10
lines / 3,134 bytes with SHA-256
`ef8f180110a409f85167ee1cc0f641c210144d6e5b5c737d5d8eb69e82d47bcb`; its body,
probe, and runner hashes are pinned in the summary. Graph, B/S cells, and registries
remain unchanged. Boundary 21 is STUMPS entry; Boundary 22 executes and resumes only the
first stump transaction.

That transaction is beam SIG 12 / `beam:12:b:1` / plan 147. The atomic native carrier
runs B12-B17 and resumes without Java's SIDES-only outer B18 assignment. Java reports
glyph 310 `ReuseActive`, `CreatedChecked`, two `AllUnlinked` reads, Stem Inter ID 2372,
zero siblings, two heads, and `outerAssignment=false`. Native adds dense stem identity 32
and relation identity 331, reaching 254 vertices / 334 edges with 33 Stem bindings, 62
linked B cells, and 70 linked S cells. Resume skips two structural-and-linked side stumps
and stops at worklist index 1, beam SIG 22 / `beam:22:b:1` / plan 622 before its
`createStem`. No pure already-linked or known-false event occurs in this real prefix. The
separate six-row-plus-summary fixture is 11 lines / 2,619 bytes with SHA-256
`b1a312ddc690911b916971081ce21ea1c2211283df174a2175094ace7c144d5e`; probe, runner,
emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`f9ca026d03873ab5c40c568a926455e0555d343540d522258d87757a1cc28f0c`,
`db9a2fd99746dfbc2ae3b5eed643a374e79dabc26a79101b05779cfba25ee5a4`, and
`5997662c47fb5be7cc61079baecb10f2986c89b05a7c0c97b937596dbc5009d6`.

Boundary 23 calls that unchanged carrier again from Boundary 22's mutated terminal; it is
second-frontier generalization evidence, not a new production operation. Chula system 1 plan
622 on beam SIG 22 / `beam:22:b:1` / TOP uses Java glyph 321 `ReuseActive`, returns
`CreatedChecked` Stem Inter 2373 after two `AllUnlinked` reads, writes zero sibling and two
head links, and records `outerAssignment=false`. Native adds dense stem identity 33 and
relation identity 334, reaching 255 vertices / 337 edges with 34 Stem bindings, 63 linked B
cells, and 72 linked S cells. Resume skips structural-and-linked `beam:22:b:2` and
`beam:16:b:0`, then stops at worklist index 2 on `beam:16:b:1` / plan 404. The next frontier
has profile 3 / link profile 1, two heads, last index 3, two relations, two glyphs, and no line
change. Its six-row-plus-summary fixture is 11 lines / 2,712 bytes with SHA-256
`4e54cc848116597ad563fd9038e102a135ff606660775e09142c8c8564567173`; probe, runner,
emitted-body, semantic-pass, and init-script SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`1812529f72a86e4b96b7d08d09f98a1157d9feb862296cd19e95de5caddded11`,
`716db362ee56e43a0375d8cf0efb0c88cd0af67de5707926bc4b713505201187`,
`07b6dc29043c6b63bd1f9f9e15822270ca3169e8662207c7cbbf67a06d8579a6`, and
`08d332af997d502fd32afb8b6257243d5ef41e87fa0001f90f3680c17394acd2`.
The refreshed linked-S fixture SHA-256 is
`287175a58717874882bc6487f7d59ea86a22e44cadcac003ee99a36606e5ab34`.
Boundary 24 applies the same carrier a third time and grades the first natural multi-glyph
STUMPS candidate in this carried prefix. Plan 404 on beam SIG 16 / `beam:16:b:1` / TOP
combines Java glyph IDs 303 and 2156; their union equals active modeled glyph 303 at ordinal
972, so `ReuseActive` changes neither registry nor allocator. Java returns `CreatedChecked`
Stem Inter 2374 after two `AllUnlinked` reads, adds zero sibling and two head links, uses base
edge 337, links B, and records `outerAssignment=false`. Native adds dense stem identity 34 and
reaches 256 vertices / 340 edges with 35 Stem bindings, 64 linked B cells, and 74 linked S
cells. Resume skips structural-and-linked `beam:16:b:2` and `beam:28:b:0`, then stops at
worklist index 3 on `beam:28:b:1` / plan 508. The next frontier has profile 3 / link profile 1,
two heads, last index 3, two relations, two glyphs, and no line change. Its six-row-plus-summary
fixture is 11 lines / 2,709 bytes with SHA-256
`e7409462ec43f5cde89ffdeafb0c5bb59586c37fff1506086d9c5fa770b30490`; probe, runner,
emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`f2a41ca0069873274e443c978d0e84d56c49d67fa3387ef06346995dd2d587c1`,
`3e66a99fe44495915fbb8c15f7285a7c9a5ae4340df60b7766968c3e214a1bc7`, and
`ee1acaf3b1742346913ce3e9ed32430d3a4b24277537f0ed8e941d530ee6935b`.
The refreshed linked-S fixture SHA-256 is
`287175a58717874882bc6487f7d59ea86a22e44cadcac003ee99a36606e5ab34`.
Boundary 25 adds `drive_native_stems_beam_stumps_from_first_stems_bridge`, a bounded atomic
driver over the validated one-frontier operation. It runs on a shadow carrier and commits
the whole batch only at a positive caller limit or typed post-STUMPS completion; a later
error rolls every earlier transaction in the call back. From Boundary 24's plan-508
frontier, chula system 1 executes the remaining plans 508, 28, 330, and 251. Java reports
glyphs 308/305/302/300, `ReuseActive`, `CreatedChecked` Stem Inter IDs 2375-2378,
`AllUnlinked` reads 2/2/3/2, base edges 340/343/346/350, zero siblings, and head counts
2/2/3/2. Native uses dense stem identities 35-38 and finishes all seven STUMPS transactions
after 92 scheduler events at 260 vertices / 353 edges, 39 Stem bindings, 68 linked B cells,
and 83 linked S cells. A one-transaction limit commits only plan 508 and returns plan 28;
zero rejects unchanged; a missing later `beam:32:b:1` cell rolls the whole batch back. The
fresh fixture is 87 lines / 19,184 bytes—82 semantic rows plus summary—with SHA-256
`81fecf842495ddc93792b0ed5acf5641231181f172acd4e5cbf3bc57565f0cd2`; probe, runner,
emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`2c6f9aaf39ae8ec2420104f15a3f6a2784f4eb4f229b0b23a7963ab5aade5717`,
`946c160f4759ee3edb093c3cc1e5394965409f64e1b516b1ebcbbbfe009f49e4`, and
`a629a2d63d223f28264c3fdc4dc20941e082402c27d75c2c6d884e2ce8282d08`.
This completes chula system 1's STUMPS worklist, not full STEMS. Its production library
run is 695 passed / 0 failed / 2 ignored, and the full local workspace, formatting, and
strict all-target Clippy gates are green; `5f75f8708` (including Boundary 43) is the current
full-workspace and CI baseline. Rust run 32217412749 passed all 12 shards and Build & Test
run 32217412751 passed, with no failure or cancellation. Wider-corpus authority and branch
coverage, other systems, and later STEMS phases remain open.

Boundary 26 adds `remove_native_stems_beam_competing_hook_and_resume`, an atomic
graph-owning consumer for one typed SIDES hook-removal frontier. Its gate reconstructs
Allegretto system 1 after transaction 28 from the 28 measured B/sibling writes; it does not
claim native execution of transactions 1-27. At Java event 64 / work index 19, Beam SIG 25
has LEFT and RIGHT linked and names same-glyph BeamHook SIG 24 as its competitor. Java
removes Inter 907 from the active SIG while retaining its SIG attachment and persistent
InterIndex representation. Group `[21,24,25]` becomes `[21,25]`; the local worklist and
43-entry linked-B set are unchanged. Native tombstones vertex 56, removes its active source
binding and five incident Containment/BeamBeam/Exclusion/two BeamStem edges, and resumes to
`SidesExhausted`. Active graph counts move 202/232 to 201/227. Java exhausts at visible
event 110; native emits 54 continuation events and ends with 143 internal events. Missing
Exclusion evidence rejects atomically. The 32-line / 4,195-byte predecessor fixture has
SHA-256 `d173f1c475245980cad02bbf4624987d787fb293e5419d21444729f18bf7c8f8`; the
9-line / 4,336-byte result fixture has SHA-256
`d4c5decf03eaab893c79b2cb7ebd0378f13ac019acc007a38718105c75eacc71`. Probe,
runner, emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`3b9e0e28c9c2de75266c676a880dfe636bef885591ce12ed832640b8c72dd845`,
`52432167156b75e4754259ae6c2a634e87788f028e85e6ea14754859e12ccb1f`, and
`2cc4ad8e0aadf29b8055ce34c32b703c033c45880bef24ff26a707b6b6f0d3c5`.
Its production library run is 696 passed / 0 failed / 2 ignored, and the full local
workspace, formatting, and strict all-target Clippy gates are green; `5f75f8708`
remains the current remote CI baseline. Native Allegretto
predecessor carriage, hook removal beyond this checkpoint, wider-corpus STUMPS authority,
general dirty-state ownership, other systems, and later STEMS remain open.

Boundary 27 adds `begin_native_stems_head_linking_phase1`, a typed read-only transfer
from the exact chula-system-1 post-STUMPS carrier into Java's heads-linking phase 1. It
accepts only scheduler `Completed`, validates common system identity and live bindings,
recomputes Java's stable reverse-grade order, and requires all 102 live graded heads plus
the exhaustive duplicate-free persistent S-cell topology and observer order. It clones
the unchanged 260/353 carrier, starts at head index 0 with empty unlinked-head and
undefined-side collections, and uses STRICT stem profile 0 / link profile 1 /
`append=false`. Head 0 is SIG ordinal 45 / Java Inter 1375, grade bits
`0x3fe917c3b8207578`. LEFT is open/unlinked with TOP/BOTTOM false/false; RIGHT is
open/unlinked with true/false, selecting `h:38:RIGHT:TOP` and returning
`AwaitingHeadCLinkTransaction`.

The boundary fails closed on incoherent terminal/system/binding/order/head/S-cell or
bounded builder evidence. Dual-corner selection, close-head/gap recursion, retry and
closure, phase-2 append, and `HeadLinker.CLinker.link` remain outside this read-only
transfer. Boundary 28 below consumes the selected frontier. The shared fixture, now expanded through Boundary 32,
is 16 lines / 12,880 bytes with eleven semantic rows plus summary, SHA-256
`91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`; probe,
runner, emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`,
`dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and
`e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398`.
Boundary 27's full local workspace, formatting, and strict all-target Clippy gates are
green. `5f75f8708` (including Boundary 43) is the current remote-CI baseline: all 12 Rust
shards and the Java build passed without failure or cancellation.

Boundary 28 adds `advance_native_stems_head_single_item_c_link`, an atomic consumer for
the selected `h:38:RIGHT:TOP` frontier. Its nonrecursive builder has exactly one
`StartHeadHalfLinker` with `lastIndex=maxIndex=0`. Canonical glyph 307 is active and
strongly retained, so `ReuseActive` leaves registry counts and hashes unchanged. With
`append=false`, the production path accepts only `CreatedChecked`; it creates native
dense Stem identity 39 / Java Inter ID 2379 and one RIGHT HeadStem relation. The compact
native graph moves 260/353 to 261/354, Stem bindings 39 to 40, and the persistent ID
allocator 2378 to 2379. The selected S cell and queued per-head cache change coherently
from unlinked to linked, taking linked S cells 83 to 84 with zero closed-cell changes.
Java's full graph moves 678/689 to 679/690, an exact normalized delta rather than an
absolute Java/native graph-size equality claim.

The carrier commits `current_index=1` and `frontier_consumed=true`, then stops before
head index 1. Late or corrupt glyph authority rejects atomically. Multi-item expansion,
recursion, gaps and beam relations, `reuseStem`, creation dispositions other than
`CreatedChecked`, duplicate relations, outer head iteration, rather-good retry/no-link
closure, unlinked-head collection, phase-2 append, and recursive tail C-linking remain
open. The current 16-line / 12,880-byte fixture has eleven semantic rows plus summary, SHA-256
`91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`.
Probe, runner, emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`,
`dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and
`e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398`.
Boundary 28's full local workspace, formatting, and strict all-target Clippy gates are
green. `5f75f8708` (including Boundary 43) is the current remote-CI baseline: its exact commit
has reached terminal green.
Broader pre-STEMS SIG assembly remains bounded where later corpus
systems still lack complete BEAMS group products.

Last updated 2026-08-18.

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
| 4 | `GRID` | **Native and published** | Staff lines, systems, bars, connectors, contextual grades, completed line geometry, and `NO_STAFF` pixels. All 65 staves and 420 barlines in the example corpus match Java. The exact `addShortSections` lifecycle now feeds bar/brace construction, and live brace glyph/SIG promotion is part of the owned cross-stage SIG. | Widen the PDF corpus. |
| 5 | `HEADERS` | **Native and published** | `recognize_native_headers` composes clef, key, and time columns in Java order from live GRID state alone. All nine pages and 65 staves match for starts/stops and selected evidence, including 34 keys, 17 times, and all 30 downstream erase rectangles. Clef pitch samples the native staff-line splines at the glyph centroid x, with midpoint geometry only as a fallback, and uses the Bravura F-clef area pitch offset. This changes the measured Graceful Ghost system-1 F clef from spurious `Baritone` to Java-matching `Bass`; all 20 warped and 25 dewarped system crops are free of `Baritone` clefs. One low-resolution full-page page-5 staff remains a wider GRID/preprocessing outlier while its high-resolution crop is `Bass`; page 3 fails earlier in GRID brace processing. Schema 1 publishes selected inters, lifecycle/classifier evidence, staff ranges, and system-owned erases. Header SIG grades are carried per inter rather than only per key: each key alter retains its own pitched grade with `intrinsicRatio` applied, and a key's grade is the mean of its members. Chula system 1's ten header grades now match Java bit for bit, closing the measured SIG grade ledger at 221/221. | Widen the header corpus and complete the remaining recognizer integration. |
| 6 | `STEM_SEEDS` | **Native and published** | `recognize_native_stem_seeds` composes live GRID and HEADERS state through lag selection, vertical `StickFactory`, staff/header gating, the concrete checker, fixed-glyph materialization, and free-glyph ownership. Across 30 systems, all 2,425 raw candidates, 422 header skips, 2,003 checks, 97 rejects, and 1,906 accepted glyphs match Java, including bit-exact grades and complete run-table digests. Schema 1 publishes accepted seeds in production order with geometry and exact checker/materialization evidence. The BEAMS adapter and CLI validate and preserve every accepted per-system identity and median. | Widen beyond profile 1 and add tablature/no-staff skip cases. |
| 7 | `BEAMS` | **Native and published** | Native GRID -> HEADERS -> STEM_SEEDS composition feeds the spot chain, system dispatch, beam creation, measured extension, hooks, grouping, and schema-1 output. A fresh-JVM Java counterfactual over 803 final beam/hook inters, 493 groups, and one multiple rest proves actual seeds change zero records on the original eight pages. D039 adds the natural acceptance case: one system-2 beam changes, with endpoint, height, six impacts, and grade bit-exact to Java. The original gate still matches 2,739 spots, 30 erases, and 787/787 raw beams. Production retains exact group memberships and now runs the real MultipleRest pass from a freshly recomputed staff projector: Bach system 6 replaces source ordinal 182 with median, grade, height, staff, and two-serif evidence exact to Java; the retained start/stop pitch is a port-pinned intermediate, since Java's oracle publishes the rest's grade and bounds but never its pitch. | Allocate stable SIG/glyph/relation identities for the retained MultipleRest and serifs, then grade small beams and widen the corpus. |
| 8 | `LEDGERS` | **Native and published** | Native composition consumes GRID's `NO_STAFF`, curved staff/system geometry, and the oracle-free BEAMS result after MultipleRest source-beam deletion. Schema 1 includes all seven impacts, live exclusions, and curved inferred paths. All 581 final Java inters and 95 inferred paths on the eight beam sheets match after sheet-wide one-sigma post-analysis and rebuild. Every final live ledger now retains its exact positioned fixed glyph raster from the referenced filtered sections; Chula's per-system section dispatch is also exact at 2,042/591/961. Ledger grades are now gated on raw f64 bit patterns rather than the nine-decimal fixture: all eight of Chula's system-1 ledgers match Java bit for bit, after correcting `y_at_x_ext` to evaluate the staff-line spline the way `LineInfo.yAt` does. | Widen beyond the example corpus. |
| 9 | `HEADS` | **Native and published** | The complete production entry point composes live GRID, HEADERS, STEM_SEEDS, BEAMS, and LEDGERS state through prolog, template lookup, seed and range glyph creation, staff duplicate/overlap handling, attachment, small-beam arbitration, and tally analysis. The eight-page top-level differential matches all 3,609 heads entering the epilog, 62 duplicate removals, 2,725 overlap exclusions, 3,547 post-duplicate heads, 191 beam inputs and registered glyphs, 10,053 ordered beam checks by exact per-system hash, 26 head removals, 3,521 final heads, 1,451 tally inputs, and 18 scale rows. Schema 1 publishes identity-free final-head provenance, exact glyph evidence, beam decisions, counts, and scale rows. | Widen the published corpus. |
| 10 | `STEMS` | **Native, graded, and published for Batuque and Zizi; transactionally complete for Chula, Allegretto, and Carmen** | Two hundred exact production boundaries consume live final HEADS, GRID, BEAMS, LEDGERS, HEADERS, and STEM_SEEDS state. Boundaries 160-163 complete Batuque recognition/publication; Boundaries 164-166 complete Chula; Boundaries 167-183 complete Allegretto; Boundaries 184-185 complete Zizi; Boundaries 186-191 complete Carmen; Boundary 192 completes Cucaracha phase 1; Boundaries 193-200 complete Cucaracha system 1 phase 2. | Instrument Cucaracha system 2 phase-2 queue 8 x56/SIG78's real `reuseStem` append, then continue wider-corpus completion. |
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
| Music fonts and header classification | **Ported for current corpus** | 1,624/1,624 header outline-bound sweep values match; clef, key, and time classification is exact on all 65 example staves. Bravura's font-derived F-clef area pitch offset is production-wired, and target pitch samples curved staff lines at each clef's centroid x. Bravura black-notehead widths at arbitrary point sizes and Java's head-width-to-point-size secant are exact and production-wired through every graded staff. |
| Visual classifier core | **Components graded** | Frozen model parsing/inference, features, stable ranking, and glyph construction are native. The ART lookup-table math reproduces the measured OpenJDK/HotSpot paths: all 12 frozen key-alter vectors match Java at all 110 inputs, including all 99 ART moments. Remaining size/noise gates, `ShapeChecker`, user overrides, and later-stage integration are not complete. |
| `.omr` persistence | **Components graded** | Opaque round-trip and typed views cover the measured book/sheet metadata and ownership structures. Full native recognition output is not yet an end-user replacement for Java. |
| CLI, JSON, and live comparison | **JSON published through `STEMS`; completed-stage viewer live** | Real images and PDFs compose GRID -> HEADERS -> STEM_SEEDS -> BEAMS -> LEDGERS -> HEADS -> STEMS in native Java order for applicable JSON targets; GRID keeps its text report. Ordinary `-json` remains schema-1 JSONL. `-stream-json` wraps byte-identical completed-stage payloads in flushed markers without intra-stage streaming. STEMS retains all upstream products and adds terminal native Stem geometry/grades, HeadStem payloads, abnormal/no-stem sets, and undefined sides using explicit system-local identities rather than fabricated Java IDs. |
| Manual Java score preview | **Inspection only; not a parity gate** | A separate Score tab explicitly runs one selected Java sheet through PAGE, validates its single local MusicXML/MXL artifact, and renders it with locally installed Verovio to SVG. Sheets requiring sibling multi-page artifacts are rejected rather than guessed. It is not part of recognition streaming, which now stops at STEMS, and it makes no Java/Rust visual or semantic comparison claim. Future Rust MusicXML will use the same renderer path. |
| MusicXML output | **Rust not ported end to end** | The manual Java preview does not imply Rust PAGE, score assembly, or MusicXML export. The differential export suite remains queued behind semantic page completion. |
| Desktop UI | **Not ported** | Java Swing remains outside the initial headless milestone. |

Boundary 29 continues the same chula-system-1 carrier through two prelinked-success
phase-1 heads. Starting at Boundary 28's committed `current_index=1`,
`continue_native_stems_head_linking_phase1` revalidates the completed carrier,
reverse-grade queue, live head bindings, and persistent S-cell topology on each call.
Head order 1 is x90 / SIG ordinal 23 / Java Inter 1331. LEFT is already linked and both
RIGHT STRICT corners are false, so it returns true and closes x89 LEFT then RIGHT through
shared Stem 2359: two false-to-true writes. Head order 2 is x81 / SIG ordinal 33 / Java
Inter 1351; it likewise returns true through its prelinked LEFT side and closes x79
LEFT/RIGHT then x80 LEFT/RIGHT through shared Stem 2371: four false-to-true writes.
Neither call records an unlinked head.

Native reaches `current_index=3`, `frontier_consumed=true`, before x20 / SIG ordinal 65.
SIG, glyph, stem, allocator, relations, and linked flags stay unchanged; only the six
named closed cells and queue position change. Missing closure topology or invalid
consumed-frontier state rejects atomically. The current expanded schema-v6 fixture is 16 lines / 12,880
bytes with eleven semantic rows plus summary, SHA-256
`91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`.
Probe, runner, emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`,
`dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and
`e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398`.
Later queue entries, a later C-link mutation, an actually unlinked head and rather-good
retry/no-link closure, phase-2 append, and wider branches remain open.

Boundary 30 extends the same unchanged continuation through head order 3. Starting at
`current_index=3`, x20 / SIG ordinal 65 / Java Inter 1419 is prelinked on LEFT; both
RIGHT STRICT corners are false, so Java returns true and shared Stem 2361 closes x19
LEFT then RIGHT (two ordered false-to-true writes) with no unlinked-head insertion.
Native reaches `current_index=4` before x36 / SIG ordinal 69 / Java Inter 1427, with
graph, registry, stem, allocator, relation, and linked state unchanged apart from those
two S-cell closures. The current expanded schema-v6 fixture is 16 lines / 12,880 bytes, SHA-256
`91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`; probe, runner,
emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`,
`dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and
`e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398`. Missing closure
topology still rejects atomically; actually-unlinked/retry, later C-link, phase-2
append, and broader head branches remain open.

Boundary 31 extends the unchanged continuation through head order 4. Starting at
`current_index=4`, x36 / SIG ordinal 69 / Java Inter 1427 / grade bits
`0x3fe8e37718100f0c` is prelinked on LEFT and both RIGHT STRICT corners are false. Java
returns true and shared Stem 2369 closes x35 LEFT then RIGHT, two ordered false-to-true
writes with `closedValueChanges=2` and `unlinkedCount=0`. Native reaches
`current_index=5`, `frontier_consumed=true`, before x99 / SIG ordinal 61 / Java Inter
1411 / grade bits `0x3fe8b9e1faa76070`. Graph, registry, stem, allocator, relation,
and linked state remain unchanged apart from those two closed S cells. Missing closure
topology rejects atomically. The schema-v6 fixture is 16 lines / 12,880 bytes with ten
semantic rows plus summary, SHA-256
`91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`; probe, runner,
emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`,
`dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and
`e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398`.
This is one more bounded prelinked-success case, not the remaining ordered queue, a
later C-link mutation, actually-unlinked/retry behavior, phase-2 append, or broader
head-branch coverage.

Boundary 32 extends the unchanged continuation through head order 5. Starting at
`current_index=5`, x99 / SIG ordinal 61 / Java Inter 1411 returns true through the
prelinked-success path, and shared Stem 2365 closes x98 LEFT then RIGHT in SIG order.
Both writes change false to true and no unlinked head is recorded. Native reaches
`current_index=6`, `frontier_consumed=true`, before x22 / SIG ordinal 12 / Java Inter
1309. Graph, registry, stem, allocator, relation, and linked state remain unchanged
apart from those two closed S cells. Missing closure topology rejects atomically. The
schema-v6 fixture is 16 lines / 12,880 bytes with eleven semantic rows plus summary,
SHA-256 `91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`;
probe, runner, emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`,
`dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and
`e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398`.
This is one more bounded prelinked-success case, not the remaining ordered queue, a
later C-link mutation, actually-unlinked/retry behavior, phase-2 append, or broader
head-branch coverage.

## Next work queue

1. Generalize native carriage beyond Chula and Allegretto system 1 across wider SIDES/STUMPS, linked-S, hook-removal, and head branches.
2. Continue after generic `finalizeStems` with the rather-good profile escalation and `reuseStem` on a system where an append retry actually links; also widen STUMPS and competing-hook coverage beyond their single-system/checkpoint evidence.
3. Expose `recognize_native_stems` once the full scheduler path runs from native products.
4. Allocate stable MultipleRest/serif identities, grade small-beam pages, and widen the published recognition corpus.
5. Add end-to-end MusicXML differential grading after `PAGE` is meaningful.

## Maintenance rule

This page is reviewed with every Rust-port contribution and updated in the same
commit whenever parity, stage composition, product exposure, or the next-work
queue changes. Claims here stay deliberately short and must point back to exact
tests or oracle counts in [`rust/PORTING.md`][porting] and
[`rust/HANDOFF.md`][handoff]. A stage moves to **Native and graded** only when it
consumes native upstream state and the oracle is a grader rather than an input.

[Boundary 44] consumes head order 18 (x63 / SIG 17 / Inter 1319, grade bits
`0x3fe8009e50c15bf8`) from the both-open/unlinked frontier. Java selects LEFT/BOTTOM,
expands a two-item builder (`lastIndex=maxIndex=1`) from glyphs 328 and 2063, reuses
canonical glyph 328 without reinsertion, and creates checked Stem Inter 2381 with one
HeadStem relation. Native creates dense Stem identity 41, moves SIG 680/691 to 681/692
and Stem bindings 41 to 42, links LEFT, records no unlinked head or closure write, and
reaches `current_index=19` before x69 / SIG 76 / Inter 1441.

Its geometry is bounded to this case: Java-order RunTable centroid accumulation and
direct interpolation of the theoretical stem line's x at the centroid y. Generic
multi-item/recursive geometry, other corners, `reuseStem`, and retry/no-link remain
open. The focused gate and full 14-test sibling suite are green; formatting, strict
all-target Clippy, and diff checks pass. The snapshot-minimized schema-v18 derivative is
14 lines / 11,751 bytes with nine semantic
rows plus summary; orders 1-17 reconstruct without persisted full snapshots and order
18 emits its C-link envelope/result and continuation. Fixture/runner/probe/body/semantic
pins are `4972836c5e2718f9441a007840cfc5100caa95a12dc349d7822c0695ad0f5b2b`,
`3bea814e71ba13374130351d0f5cc057779e5676e402e7b43b5c4ee4a263e332`,
`4e15aa27d982b6ea848b5a7349819e3db7300349dded652f859492abe2ea7460`,
`499b791dc34d2ca59666bbab20e4ca15a9dd335260d4714dbdd9042ed00456cd`, and
`7045d9060ea8e6d930b94d28e79e3e6d8d0cc0bb0b57bb20c64a3780b876bcb3`;
fragment source is pinned by
`f56fdd58606c3d5101ebea1690162b38f9db6a18f89a4fe0e441cedff1bac36c`.

[Boundary 45] carries head order 19 (x69 / SIG 76 / Inter 1441, grade bits
`0x3fe7fe09c1461c49`). LEFT is already linked and RIGHT is open/unlinked, so Java
reports `SkipAlreadyLinked` then `Neither`. Shared Stem 2347 closes x68 LEFT then
RIGHT through two ordered false-to-true writes. Native records no unlinked head, keeps
SIG 681/692, system stems 42, and the relation hash unchanged, and reaches
`current_index=20` before x74 / SIG 19 / Inter 1323 (grade bits
`0x3fe7f8f93b5cf200`), whose sides are both open/unlinked. Boundary 45 does not execute
that next head.

The focused gate and full 14-test sibling suite are green; formatting, strict
all-target Clippy, and diff checks pass. The snapshot-minimized schema-v19 derivative
is 15 lines / 13,004 bytes with ten semantic rows plus summary; orders 1-18 reconstruct
without emitted or persisted full snapshots and only order 19 emits the continuation.
Fixture/runner/probe/body/semantic pins are
`6d415102995fd1fda8057fab27b0f2a3a6cb2367cbcce52269009f377bf672ae`,
`b79cb0c5cba1d3b1275dd943d7945722a5f025281686362d6b40a311d3ad5335`,
`e94082b8faa8a8c26e70b00acd42bc091e7c9333317caa5299f6d18083cba781`,
`3ae97b86466a49fafbe07f5c32d5641824099e677131fff14aee3797f61cc3a9`, and
`9628fefbc7e1c88ab184aa711e329b9606e4d57252965428b9f3f33e96852a31`;
fragment source remains pinned by
`f56fdd58606c3d5101ebea1690162b38f9db6a18f89a4fe0e441cedff1bac36c`.
This is bounded order-19 evidence, not independent predecessor snapshots or coverage
of order 20, actually-unlinked/retry, phase-2 append, generic multi-item/recursive
C-linkers, or broader corpus/system behavior.

[Boundary 46] consumes head order 20 (x74 / SIG 19 / Inter 1323, grade bits
`0x3fe7f8f93b5cf200`) from a both-open/unlinked frontier. Java selects LEFT/BOTTOM,
expands a two-item builder (`lastIndex=maxIndex=1`) from active glyphs 332 and 2301,
reuses canonical glyph 332, and creates checked Stem Inter 2382 with one HeadStem
relation. Native creates dense Stem identity 42, moves SIG 681/692 to 682/693 and
system stems 42 to 43, links LEFT, records no closure write or unlinked head, and
reaches `current_index=21` before x28 / SIG 55 / Inter 1399 (grade bits
`0x3fe7e38e38e38e39`), whose LEFT side is linked and RIGHT remains open/unlinked.

Geometry remains bounded to this case: the authenticated two-item
centroid/interpolation path applies Java `nextDown` to both translated x coordinates
only at x74. Generic multi-item/recursive geometry and other corner shapes remain
open. The focused gate and full 14-test sibling suite are green; formatting, strict
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

[Boundary 47] carries head order 21 (x28 / SIG 55 / Inter 1399, grade bits
`0x3fe7e38e38e38e39`). Its authenticated LEFT/BOTTOM C-link envelope finds active
glyph 300 already owned by Stem Inter 2378, with two planned relations and one glyph.
Java leaves allocator 2382 unchanged and adds no vertex, edge, or system stem. Phase-1
continuation observes LEFT already linked and RIGHT `Neither`, then closes x27 / SIG 54
LEFT and RIGHT through two ordered writes. Native keeps SIG 682/693 and system stems
43, records no unlinked head, and reaches `current_index=22` before x4 / SIG 7 / Inter
1299 (grade bits `0x3fe7dcd4cd6e88ba`), whose LEFT side is linked and RIGHT remains
open/unlinked.

The wrapper authenticates only this existing-stem retry and its graph-derived closure;
generic retry and no-link behavior remain open. The focused gate and full 14-test
sibling suite are green; formatting, strict all-target Clippy, and diff checks pass.
The snapshot-minimized schema-v21 derivative is 17 lines / 14,834 bytes with twelve
semantic rows plus summary; orders 1-20 reconstruct without emitted or persisted full
snapshots and only order 21 emits the retry envelope/result and continuation.
Fixture/runner/probe/body/semantic pins are
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

[Boundary 48] carries head order 22 (x4 / SIG 7 / Inter 1299, grade bits
`0x3fe7dcd4cd6e88ba`). Its authenticated LEFT/BOTTOM envelope has
`lastIndex=maxIndex=2`, two planned relations, and active glyphs 315 and 2142; canonical
glyph 315 is already owned by SIG-attached Stem Inter 2354. Java leaves allocator 2382
unchanged and adds no vertex, edge, glyph, or system stem. Continuation observes LEFT
already linked and RIGHT `Neither`, closes x3 / SIG 6 LEFT then RIGHT through two
ordered writes, and reaches `current_index=23` before x78 / SIG 39 / Inter 1363 (grade
bits `0x3fe7d236c1f8e275`). Native keeps SIG 682/693 and system stems 43 and records no
unlinked head.

The wrapper authenticates only this retry and graph-derived closure, including Stem
2354/glyph 315 presence and SIG attachment; generic retry and no-link remain open. The
focused gate and full 14-test sibling suite are green; formatting, strict all-target
Clippy, and diff checks pass. The snapshot-minimized schema-v22 derivative is 18 lines
/ 16,188 bytes with thirteen semantic rows plus summary; orders 1-21 reconstruct
without emitted or persisted full snapshots and only order 22 emits the retry
envelope/result and continuation. Fixture/runner/probe/body/semantic pins are
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

[Boundary 49] adds no production operation. The existing continuation carries order 23
(x78 / SIG 39 / Inter 1363, grade bits `0x3fe7d236c1f8e275`). LEFT is already linked
and RIGHT is `Neither`; incident Stem 2370 joins x77 / SIG 38 and x78 / SIG 39 on LEFT,
so x77 LEFT then RIGHT close through two ordered writes. Native keeps SIG 682/693 and
system stems 43, records no unlinked head, and reaches `current_index=24` before x93 /
SIG 25 / Inter 1335 (grade bits `0x3fe7d1c13d1c13d2`), whose LEFT is linked and RIGHT
remains open/unlinked.

This is evidence for the unchanged generic prelinked-success path, not a new retry
implementation. The focused gate and full 14-test sibling suite are green; formatting,
strict all-target Clippy, and diff checks pass. The snapshot-minimized schema-v23
derivative is 19 lines / 17,401 bytes with fourteen semantic rows plus summary; orders
1-22 reconstruct without emitted or persisted full snapshots and only order 23 emits
the closure and continuation. Fixture/runner/probe/body/semantic pins are
`20731b3ff52e2512407f17c00329e16f015aaedba7bf5c91ec1b0b9907c58e68`,
`b945062e6c069c975f738ee066bc42107b4b6af599b5097abd0423bbb232aa25`,
`8c6694e8f0c9d293db056b515f51d5393b6a9a860d002e4033cafc8881f768af`,
`e8032bae1ffee2113b72b8a359d5c25cc219a84f1bd6a89485632138db42540f`, and
`093645a4a4ffe760113cfd15776c7e0eb61381b405b66a2f5789999a29927f38`;
the shared v22/v23 fragment source remains pinned by
`576406fb3bd8bf9503ca883480bc55b217b3c6bc99ca440ef702774d3a2ca950`.
This is bounded order-23 evidence, not independent predecessor snapshots or coverage
of order 24, actually-unlinked/no-link, phase-2 append, generic retry or broader
C-linkers, or broader corpus/system behavior.

[Boundary 50] adds no production operation. The existing continuation carries order 24
(x93 / SIG 25 / Inter 1335, grade bits `0x3fe7d1c13d1c13d2`). LEFT is already linked
and RIGHT is `Neither`; incident Stem 2342 joins x92 / SIG 24 and x93 / SIG 25 on LEFT,
so x92 LEFT then RIGHT close through two ordered writes. Native keeps SIG 682/693 and
system stems 43, records no unlinked head, and reaches `current_index=25` before x59 /
SIG 74 / Inter 1437 (grade bits `0x3fe7c31e7e01c29a`), whose LEFT is linked and RIGHT
remains open/unlinked.

This is further evidence for the unchanged generic prelinked-success path, not a new
retry implementation. The focused gate and full 14-test sibling suite are green;
formatting, strict all-target Clippy, and diff checks pass. The snapshot-minimized
schema-v24 derivative is 20 lines / 18,614 bytes with fifteen semantic rows plus
summary; orders 1-23 reconstruct without emitted or persisted full snapshots and only
order 24 emits the closure and continuation. Fixture/runner/probe/body/semantic pins
are `56684be47b32b49e3d6f3c1440a9f3062a6bdcdec28fa0554cc6f2be80242b6c`,
`2d2a7b2b58f674bdf3db3716a6e66eac1b9d56694df7c79d7ec91ff7cb629293`,
`24f9bab608b05b89f0a28198b19827cfc0d241a0fd558298564e24f868b30872`,
`65d329d75ac1d9fff1fba2d13b9b418346645bbbcf3637061f95901039a0fac5`, and
`15cadab070e039fdb0753fcb57cc0e1aeb9012d0d19773eb701a47fc982d582e`;
the shared v22-v24 fragment source remains pinned by
`576406fb3bd8bf9503ca883480bc55b217b3c6bc99ca440ef702774d3a2ca950`.
This is bounded order-24 evidence, not independent predecessor snapshots or coverage
of order 25, actually-unlinked/no-link, phase-2 append, generic retry or broader
C-linkers, or broader corpus/system behavior.

[Boundary 51] adds no production operation. The existing continuation carries order 25
(x59 / SIG 74 / Inter 1437, grade bits `0x3fe7c31e7e01c29a`). LEFT is already linked
and RIGHT is `Neither`; incident Stem 2363 joins x58 / SIG 73 and x59 / SIG 74 on LEFT,
so x58 LEFT then RIGHT close through two ordered writes. Native keeps SIG 682/693 and
system stems 43, records no unlinked head, and reaches `current_index=26` before x61 /
SIG 31 / Inter 1347 (grade bits `0x3fe7b8475abaafaf`), whose LEFT is linked and RIGHT
remains open/unlinked.

This is further evidence for the unchanged generic prelinked-success path, not a new
retry implementation. The focused gate and full 14-test sibling suite are green;
formatting, strict all-target Clippy, and diff checks pass. The snapshot-minimized
schema-v25 derivative is 21 lines / 19,854 bytes with sixteen semantic rows plus
summary; orders 1-24 reconstruct without emitted or persisted full snapshots and only
order 25 emits the closure and continuation. Fixture/runner/probe/body/semantic pins
are `39ccb74b6231aa2ce3f77a41adb59d18ae64c736598917523f4c4f8835722d2d`,
`d9bb5989503627cf7486f6c3286ffe78754a1a089d1d18087fef1e6d15389c68`,
`d30b66790a5b3b9cfc3aa9da27908aa90a1018d6a7fedd0f7c7029e0f6cbb69d`,
`c361bb73ac81783c8b0862490582fb4a6384ca98b845a2f85ccbf42c77da02f2`, and
`34d99daf8ee4b8b670c52a4ea28cf1bfae406f2bbb9904e5595208f0b0188fc8`;
the shared v22-v25 fragment source remains pinned by
`576406fb3bd8bf9503ca883480bc55b217b3c6bc99ca440ef702774d3a2ca950`.
This is bounded order-25 evidence, not independent predecessor snapshots or coverage
of order 26, actually-unlinked/no-link, phase-2 append, generic retry or broader
C-linkers, or broader corpus/system behavior.

[Boundary 52] adds no production operation. The existing continuation carries order 26
(x61 / SIG 31 / Inter 1347, grade bits `0x3fe7b8475abaafaf`). LEFT is already linked
and RIGHT is `Neither`; incident Stem 2345 joins x60 / SIG 30 and x61 / SIG 31 on LEFT,
so x60 LEFT then RIGHT close through two ordered writes. Native keeps SIG 682/693 and
system stems 43, records no unlinked head, and reaches `current_index=27` before x33 /
SIG 26 / Inter 1337 (grade bits `0x3fe7a22f6f5852b0`), whose sides are both
open/unlinked; this boundary does not execute that next frontier.

This is further evidence for the unchanged generic prelinked-success path, not a new
retry implementation. The focused gate and full 14-test sibling suite are green;
formatting, strict all-target Clippy, and diff checks pass. The snapshot-minimized
schema-v26 derivative is 22 lines / 21,096 bytes with seventeen semantic rows plus
summary; orders 1-25 reconstruct without emitted or persisted full snapshots and only
order 26 emits the closure and continuation. Fixture/runner/probe/body/semantic pins
are `a5e6a9cb07b49ecf1753fbe10ba709a63d274dce5393887acddc123e55342c36`,
`afe60083e9b34076c7aab0106216eb5dac7ba689c63ef388112f7b700f842ed0`,
`d794e14d3715c64e7e9b3364fbf1a29389a4bd327da577e7313ce0de4eafdaa8`,
`8220b597632c878f90e6ebb8bf4f84ac4beda6a2458c07056663075520ff2f73`, and
`da5cfb3439d4efec0cbd64299cf037927ab4cea76a20c1c740bdee0780916a49`;
the shared v22-v26 fragment source remains pinned by
`576406fb3bd8bf9503ca883480bc55b217b3c6bc99ca440ef702774d3a2ca950`.
This is bounded order-26 evidence, not independent predecessor snapshots or coverage
of order 27, actually-unlinked/no-link, phase-2 append, generic retry or broader
C-linkers, or broader corpus/system behavior.

[Boundary 53] consumes order 27 (x33 / SIG 26 / Inter 1337, grade bits
`0x3fe7a22f6f5852b0`) from a both-open/unlinked frontier. Java selects LEFT/BOTTOM,
expands a two-item builder (`lastIndex=maxIndex=1`) from active glyphs 314 and 2219,
reuses canonical glyph 314, and creates checked Stem Inter 2383 with one HeadStem
relation. Native creates dense Stem identity 43, moves SIG 682/693 to 683/694 and
system stems 43 to 44, records no closure write or unlinked head, and reaches
`current_index=28` before x85 / SIG 87 / Inter 1463 (grade bits
`0x3fe79e7f455ba48d`), whose LEFT is linked and RIGHT remains open/unlinked.

Geometry remains bounded to this two-item LEFT/BOTTOM case. The focused gate and full
14-test sibling suite are green; formatting, strict all-target Clippy, and diff checks
pass. The snapshot-minimized schema-v27 derivative is 25 lines / 25,740 bytes with
twenty semantic rows plus summary; orders 1-26 reconstruct without emitted or
persisted full snapshots and only order 27 emits its C-link envelope/result and
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

[Boundary 54] adds no production operation. The existing continuation carries order 28
(x85 / SIG 87 / Inter 1463, grade bits `0x3fe79e7f455ba48d`). LEFT is already linked
and RIGHT is `Neither`; incident Stem 2366 joins x84, x85, and x86 on LEFT, so x84
LEFT/RIGHT and x86 LEFT/RIGHT close through four ordered writes. Native keeps SIG
683/694 and system stems 44, records no unlinked head, and reaches
`current_index=29` before x10 / SIG 9 / Inter 1303 (grade bits
`0x3fe79713252eb76a`), whose LEFT is linked and RIGHT remains open/unlinked.

The default full-snapshot order-28 oracle exhausted the JVM heap. Its replacement runs
orders 1-27 without snapshots and emits only authenticated order-0 baseline/C-link
evidence plus the order-28 closure row; it does not independently snapshot-oracle the
predecessor sequence. The focused gate and full 14-test sibling suite are green;
formatting, strict all-target Clippy, and diff checks pass. The schema-v28 derivative
is 12 lines / 8,381 bytes with seven semantic rows plus summary. Fixture/runner/probe/
body/semantic pins are
`6f30a5cb8706fb0445b5eb84cee2896dfa1b85236f6870a97177714672ef10b7`,
`ec1985d786f0c984f5a09a461008911f12777229b0a08eb71b7e36a39d548d82`,
`d2e07d5dacf3e22ec20a3f53c8e4543763982eec3e88eac1ac8e8e3368422cc2`,
`5a4675dca2831e93c61a028a6d189deed21115e9588e06b1293c37968fd2bef5`, and
`b4d16e19a892bfb0537f8b7b629e43617687f19794a2cf13332a0e69cdd4e1fd`;
the shared v27/v28 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-28 evidence, not coverage of order 29, actually-unlinked/no-link,
phase-2 append, generic retry or broader C-linkers, or broader corpus/system behavior.

[Boundary 55] adds no production operation. The existing continuation carries order 29
(x10 / SIG 9 / Inter 1303, grade bits `0x3fe79713252eb76a`). LEFT is already linked
and RIGHT is `Neither`; incident Stem 2355 joins x9 and x10 on LEFT, so x9 LEFT then
RIGHT close through two ordered writes. Native keeps SIG 683/694 and system stems 44,
records no unlinked head, and reaches `current_index=30` before x101 / SIG 43 / Inter
1371 (grade bits `0x3fe79406c6921d2e`), whose LEFT is linked and RIGHT remains
open/unlinked.

The v29 oracle retains v28's heap-safe minimized shape: orders 1-28 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-29
closure row are emitted. It does not independently snapshot-oracle the predecessor
sequence. The focused gate and full 14-test sibling suite are green; formatting,
strict all-target Clippy, and diff checks pass. The schema-v29 derivative is 12 lines /
8,292 bytes with seven semantic rows plus summary. Fixture/runner/probe/body/semantic
pins are `a88b9fd3c27133c3c8bdcc839308365557c0e95c2ac3ea83fe348dc0d1ffa270`,
`0ae5afb409d11eef138ed62bb8adbefb04eabfa99c0581cad7a6952ecb5e1d4c`,
`79ddfc2cf532474ff902156eb66c2655ec242ac0a73884fd67bfc74afb6521ca`,
`410d0c1e04f4c7dfb1b4b83ed0953da53e52605df72e08351231e302027ca84a`, and
`32c937944c1c015c79ca4993dd299bef9c32ef39e7be71c800ca98d21ccd5cde`;
the shared v27-v29 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-29 evidence, not coverage of order 30, actually-unlinked/no-link,
phase-2 append, generic retry or broader C-linkers, or broader corpus/system behavior.

[Boundary 56] adds no production operation. The existing continuation carries order 30
(x101 / SIG 43 / Inter 1371, grade bits `0x3fe79406c6921d2e`). LEFT is already linked
and RIGHT is `Neither`; incident Stem 2343 joins x100 and x101 on LEFT, so x100 LEFT
then RIGHT close through two ordered writes. Native keeps SIG 683/694 and system stems
44, records no unlinked head, and reaches `current_index=31` before x16 / SIG 81 /
Inter 1451 (grade bits `0x3fe75f1fc300149f`), whose LEFT is linked and RIGHT remains
open/unlinked.

The v30 oracle retains v28's heap-safe minimized shape: orders 1-29 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-30
closure row are emitted. It does not independently snapshot-oracle the predecessor
sequence. The focused gate and full 14-test sibling suite are green; formatting,
strict all-target Clippy, and diff checks pass. The schema-v30 derivative is 12 lines /
8,306 bytes with seven semantic rows plus summary. Fixture/runner/probe/body/semantic
pins are `c4bde8384b872a03d7f9d7ecd87fdea60dc93a5b418ca831c8dbe5d8c3aa729d`,
`d8f55efad82e15eb8b45c52ac8f99031c00ea0dd7143bc30c7c607fc103e71cf`,
`a8b50543359666567a01d503f46616d113feee03ac60828104d2b52efc558812`,
`8eebd2a60dfdaf3896a31d7200525fc70667bed42ee8cbcc0076830bae74bd40`, and
`803635259310df2794ac302b43a7b8286c95fb117f6f19177071c1ce25d484a9`;
the shared v27-v30 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-30 evidence, not coverage of order 31, actually-unlinked/no-link,
phase-2 append, generic retry or broader C-linkers, or broader corpus/system behavior.

[Boundary 57] adds no production operation. The existing continuation carries order 31
(x16 / SIG 81 / Inter 1451, grade bits `0x3fe75f1fc300149f`). LEFT is already linked
and RIGHT is `Neither`; incident Stem 2360 joins x15 and x16 on LEFT, so x15 LEFT then
RIGHT close through two ordered writes. Native keeps SIG 683/694 and system stems 44,
records no unlinked head, and reaches `current_index=32` before x34 / SIG 77 / Inter
1443 (grade bits `0x3fe75353cd1ba641`), whose LEFT is linked and RIGHT remains
open/unlinked.

The v31 oracle retains v28's heap-safe minimized shape: orders 1-30 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-31
closure row are emitted. It does not independently snapshot-oracle the predecessor
sequence. The focused gate and full 14-test sibling suite are green; formatting,
strict all-target Clippy, and diff checks pass. The schema-v31 derivative is 12 lines /
8,302 bytes with seven semantic rows plus summary. Fixture/runner/probe/body/semantic
pins are `ab58a7bf7d5a2265fbd8cc2a18ee0595b7d288935469cf27f91e01ace9397b00`,
`e7b8cd3bc87ff55969aee203b6027f7af572428cf91d442f94ea58e8f82d3e42`,
`231028452d789e78ec96e5dc1c2f8ccabe88d85ac59aa9f990e18a0775d44404`,
`34baf86107a36d017519d7ac0f0011a0eb8d67f93a5d9b2d95f55ccf0784dcc4`, and
`3d123d0fcd70cdcdc3436a1ffca7b85ecac9e1a350c6a83368f91175e35eb4e4`;
the shared v27-v31 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-31 evidence, not coverage of order 32, actually-unlinked/no-link,
phase-2 append, generic retry or broader C-linkers, or broader corpus/system behavior.

[Boundary 58] adds no production operation. The existing continuation carries order 32
(x34 / SIG 77 / Inter 1443, grade bits `0x3fe75353cd1ba641`). LEFT is already linked
and RIGHT is `Neither`; incident Stem 2368 contains only x34 on LEFT, so Java returns
with no closure writes or changed linker values. Native keeps SIG 683/694 and system
stems 44, records no unlinked head, and reaches `current_index=33` before x88 / SIG 84
/ Inter 1457 (grade bits `0x3fe73605f8f111a6`), whose LEFT is linked and RIGHT
remains open/unlinked.

The v32 oracle retains v28's heap-safe minimized shape: orders 1-31 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-32
no-op closure row are emitted. It does not independently snapshot-oracle the
predecessor sequence. The focused gate and full 14-test sibling suite are green;
formatting, strict all-target Clippy, and diff checks pass. The schema-v32 derivative
is 12 lines / 8,230 bytes with seven semantic rows plus summary. Fixture/runner/probe/
body/semantic pins are `cceda3e1b00ccf9e4ca5f701c71a0a4da4e764488e192bf056ea645f11ad72c4`,
`fecd661b0c9b9e03f17c9eba3482a86b7f2ae381e49ac93bbbcbfea4756c3cd8`,
`d1b3d61c46bfdfe540d33ae751d0006c4518142d8410277d8e4016a4b29b1fe5`,
`77810c34e97279aef05feaa043df82a7ab4ba1566edc5933f38ff80608f10191`, and
`23a5e25617ef107a7f4b2b85ddb977d8d8b164d0203477d9f995d2d90df55bf5`;
the shared v27-v32 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-32 evidence, not coverage of order 33, actually-unlinked/no-link,
phase-2 append, generic retry or broader C-linkers, or broader corpus/system behavior.

[Boundary 59] adds no production operation. The existing continuation carries order 33
(x88 / SIG 84 / Inter 1457, grade bits `0x3fe73605f8f111a6`). LEFT is already linked
and RIGHT is `Neither`; incident Stem 2367 joins x87 and x88 on LEFT, so x87 LEFT then
RIGHT close through two ordered writes. Native keeps SIG 683/694 and system stems 44,
records no unlinked head, and reaches `current_index=34` before x2 / SIG 36 / Inter
1357 (grade bits `0x3fe71d98bc61a5b3`), whose LEFT and RIGHT are both open/unlinked.

The v33 oracle retains v28's heap-safe minimized shape: orders 1-32 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-33
closure row are emitted. It does not independently snapshot-oracle the predecessor
sequence. The focused gate and full 14-test sibling suite are green; formatting,
strict all-target Clippy, and diff checks pass. The schema-v33 derivative is 12 lines /
8,302 bytes with seven semantic rows plus summary. Fixture/runner/probe/body/semantic
pins are `a058341d3f661be4a677206c7a067f39a0785ae5adeed96be7d7073541fe2982`,
`472e88ea561df7db9280c5ec79a2ea8a5204783d3ffec8894455adcf5b342692`,
`2e4a07c2efbdf0e43bb92f9bd6213cd6faf3e2a0e39eed610f11e13e15e42d72`,
`2c212c41b06dc509b217ced1e3c0bedfd6c538f3684a705eeafc3e60ff33aed4`, and
`8aec2db3857f6d2d8dc60bdb381ce3cfb8a16a1e441efd348854489bbcc53b43`;
the shared v27-v33 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-33 evidence, not coverage of order 34, its both-open C-link
geometry, actually-unlinked/no-link, phase-2 append, generic retry, or broader
corpus/system behavior.

[Boundary 60] consumes the authenticated both-open order-34 frontier for x2 / SIG 36 /
Inter 1357 (grade bits `0x3fe71d98bc61a5b3`). Its LEFT/BOTTOM C-link selects active
glyphs 322 and 1946, reuses glyph 322 as the modeled candidate, creates Java Stem 2384
/ native Stem identity 44, and adds the Inter1357-to-Stem2384 relation. Native advances
SIG 683/694 to 684/695 and system stems 44 to 45, records no unlinked head or closure
write, and reaches `current_index=35` before x50 / SIG 72 / Inter 1433 (grade bits
`0x3fe6dc9c073bac4e`), whose LEFT is linked and RIGHT remains open/unlinked.

The measured correction is bounded: Java rounds both translated stem-line x
coordinates one representable step above direct native interpolation, so
`java_next_up` applies only at authenticated x2. The v34 oracle keeps orders 1-33 as
mutations without snapshots and emits only order-0 authentication plus the order-34
frontier/result/continuation; it is not independent predecessor-snapshot evidence.
The focused gate and full 14-test sibling suite are green; formatting, strict
all-target Clippy, and diff checks pass.

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

[Boundary 61] adds no production operation. The existing continuation carries order 35
(x50 / SIG 72 / Inter 1433, grade bits `0x3fe6dc9c073bac4e`). LEFT is already linked
and RIGHT is `Neither`; incident Stem 2353 joins x49 and x50 on LEFT, so x49 LEFT then
RIGHT close through two ordered writes. Native keeps SIG 684/695 and system stems 45,
records no unlinked head, and reaches `current_index=36` before x23 / SIG 14 / Inter
1313 (grade bits `0x3fe6bf73ff00cd94`), whose LEFT and RIGHT are both open/unlinked.

The v35 oracle retains the heap-safe minimized shape: orders 1-34 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-35
closure row are emitted. It does not independently snapshot-oracle the predecessor
sequence. The focused gate and full 14-test sibling suite are green; formatting,
strict all-target Clippy, and diff checks pass. The schema-v35 derivative is 12 lines /
8,302 bytes with seven semantic rows plus summary. Fixture/runner/probe/body/semantic
pins are `2721b843514ce7a695fdacc797addd21597bd604b39168fe63533ecfc01bd55b`,
`74aec11451cb5933938b3bc82876ddfdb9e4bdab295e472644698c68d2cbc5ea`,
`611a02c34f4690031db91ce7ccced19ef6a1d7ec3d6da0dd81333f07aa315b42`,
`12d32f9193480ac9772e735735210b3689266458f4dad379a72131bb9024cc84`, and
`992372127972f17882a8f672653b9b4530497d06c8c6a43f6209ad6c8e22a1dd`;
the shared v27-v35 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-35 evidence, not coverage of order 36, its both-open C-link
geometry, actually-unlinked/no-link, phase-2 append, generic retry, or broader
corpus/system behavior.

[Boundary 62] carries order 36 (x23 / SIG 14 / Inter 1313, grade bits
`0x3fe6bf73ff00cd94`) through its LEFT/BOTTOM C-link. Active glyph 324 is reused to
create Stem 2385 / native Stem identity 46 and relation edge 1313. Native moves SIG
684/695 to 685/696 and system stems 45 to 46, records no closure write or unlinked
head, and reaches `current_index=37` before x14 / SIG 1 / Inter 1287 (grade bits
`0x3fe6b52921e6cda3`), whose LEFT is linked and RIGHT remains open/unlinked.

The v36 oracle retains the heap-safe minimized shape: orders 1-35 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-36
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused gate and full 14-test sibling
suite are green; formatting, strict all-target Clippy, and diff checks pass. The
schema-v36 derivative is 14 lines / 11,600 bytes with nine semantic rows plus summary.
Fixture/runner/probe/body/semantic pins are
`7d7d0d17e51c03a145bdff3a739da3aaaa05fb0c5bba20cd9a46468742eb26e7`,
`3176407de9bdd88f167e925a2b901f811f230f6b83c5a120ddf031a42ec49fd4`,
`582922fe7442de97a34732791352550e0026d9cf16cae36d633266eb15273aba`,
`7f61d1814c2542ae95f54515aa97a8f35ed3be2905e87336c15a83a0d8c6489b`, and
`f3e073ba83536e4afc1c0ea13a5933f199cfad57f1b82b6974f5abb9081039bd`;
the shared v27-v36 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-36 single-item LEFT/BOTTOM evidence, not coverage of order 37,
generic multi-item or recursive C-link geometry, actually-unlinked/no-link, phase-2
append, generic retry, or broader corpus/system behavior.

[Boundary 63] carries order 37 (x14 / SIG 1 / Inter 1287, grade bits
`0x3fe6b52921e6cda3`). LEFT is already linked and its four-relation LEFT/BOTTOM
candidate resolves active glyph 294 to existing Stem 2340, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is `Neither`; incident Stem 2340 joins
x13 and x14 on LEFT, so x13 LEFT then RIGHT close through two ordered writes. Native
records no unlinked head and reaches `current_index=38` before x18 / SIG 4 / Inter
1293 (grade bits `0x3fe6b1ad86c7d182`), whose LEFT is linked and RIGHT remains
open/unlinked.

The v37 oracle retains the heap-safe minimized shape: orders 1-36 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-37
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused gate and full 14-test sibling
suite are green; formatting, strict all-target Clippy, and diff checks pass. The
schema-v37 derivative is 14 lines / 12,303 bytes with nine semantic rows plus summary.
Fixture/runner/probe/body/semantic pins are
`5af8e1928df00217e1780e2e6e0d057c4202b0f1cf46f25d5d889678c5fdf2b8`,
`2fac40e0bf6f49186a994bae499aa371be8bee2152297d325bae067c3f8d5bc1`,
`58ed9ebbd2fa05e9e52349b5ad42195a8f9fe534b46e088e6be7dd850d6ab1bb`,
`4c69d4c1740899bf4c71dbc895f022882f87d772233642f638ba3ecdc4db3fb1`, and
`fcb9fec2a764e9ab06d6b91ca856a8832ef236754dcb45ba345ae3f8f7280d90`;
the shared v27-v37 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-37 existing-stem reconciliation evidence, not coverage of order
38, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

[Boundary 64] carries order 38 (x18 / SIG 4 / Inter 1293, grade bits
`0x3fe6b1ad86c7d182`). LEFT is already linked and its two-relation LEFT/BOTTOM
candidate resolves active glyph 310 to existing Stem 2372, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is `Neither`; incident Stem 2372 joins
x17 and x18 on LEFT, so x17 LEFT then RIGHT close through two ordered writes. Native
records no unlinked head and reaches `current_index=39` before x97 / SIG 34 / Inter
1353 (grade bits `0x3fe666c6bb717a2e`), whose LEFT is linked and RIGHT remains
open/unlinked.

The v38 oracle retains the heap-safe minimized shape: orders 1-37 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-38
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused gate and full 14-test sibling
suite are green; formatting, strict all-target Clippy, and diff checks pass. The
schema-v38 derivative is 14 lines / 11,312 bytes with nine semantic rows plus summary.
Fixture/runner/probe/body/semantic pins are
`98c8d3c19d50df531d756d6fd50ddbc9f07ce7db24bea47849fff731d5271b0f`,
`ad2edbfdf046db3a27b67d81da23f6f30d254cde9c91eb92063df72da10c7551`,
`8da7b91134b4ae654461eecd7f4f5009e3fe205f140663dad836b0820465a214`,
`64a375a90ec14b1e4735027c53a2f650774eb22f8ec6cc4884dacddf008ef859`, and
`57e46879aca3fc5a02851b590a14347df4535beff0b7c97855d42afe95155422`;
the shared v27-v38 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-38 existing-stem reconciliation evidence, not coverage of order
39, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

[Boundary 65] carries order 39 (x97 / SIG 34 / Inter 1353, grade bits
`0x3fe666c6bb717a2e`). LEFT is already linked and its two-relation LEFT/BOTTOM
candidate resolves active glyph 321 to existing Stem 2373, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is `Neither`; incident Stem 2373 joins
x96 and x97 on LEFT, so x96 LEFT then RIGHT close through two ordered writes. Native
records no unlinked head and reaches `current_index=40` before x6 / SIG 89 / Inter
1467 (grade bits `0x3fe65e4f5c70ff04`), whose LEFT is linked and RIGHT remains
open/unlinked.

The v39 oracle retains the heap-safe minimized shape: orders 1-38 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-39
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused gate and full 14-test sibling
suite are green; formatting, strict all-target Clippy, and diff checks pass. The
schema-v39 derivative is 14 lines / 11,315 bytes with nine semantic rows plus summary.
Fixture/runner/probe/body/semantic pins are
`771b7816918d098e66fa1c599df1a68bfb3e24d1724ea6f701ba3bcc59b031fa`,
`bf7855c0d53d59cea3593de72f51f7272168f488e65148267ebd55e9f70110c7`,
`990556c3e12f99826c6ca92596045d44cec482263c76040613b8afc1bfd796d8`,
`6f3518552f431fd0108d3c64efc6d5c2a99cd57ff841f8cbcc2987ecb80c6090`, and
`2d51ffd86926e5a39870f9e5d1222d359f28121a4f5e9ccda9b072e5fd94b73b`;
the shared v27-v39 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-39 existing-stem reconciliation evidence, not coverage of order
40, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

[Boundary 66] carries order 40 (x6 / SIG 89 / Inter 1467, grade bits
`0x3fe65e4f5c70ff04`). LEFT is already linked and its three-relation LEFT/BOTTOM
candidate resolves active glyph 290 to existing Stem 2348, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is `Neither`; incident Stem 2348 joins
x5 and x6 on LEFT, so x5 LEFT then RIGHT close through two ordered writes. Native
records no unlinked head and reaches `current_index=41` before x30 / SIG 67 / Inter
1423 (grade bits `0x3fe63a0d1316bff0`), whose LEFT is linked and RIGHT remains
open/unlinked.

The v40 oracle retains the heap-safe minimized shape: orders 1-39 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-40
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused gate and full 14-test sibling
suite are green; formatting, strict all-target Clippy, and diff checks pass. The
schema-v40 derivative is 14 lines / 11,761 bytes with nine semantic rows plus summary.
Fixture/runner/probe/body/semantic pins are
`26e4a2ecbd547829c573c4c7737331e4773f6faf64581ecfdf380a6b87283fa9`,
`7caaaf046770aafb327359fc587ed54509a83ec867a90a8c53cd254b2de5cb45`,
`36408206fc9d1f7640b1464ff9a95be6039ce77e21485891f0f889dd0cf52f84`,
`9be2634c8582ff4f023e17313aa9b91524b542d07c3c69363906b1d1e05acaa6`, and
`fa014228f89fbba214adaa1525ae8206de28f919ac71b334e2da01587f399db8`;
the shared v27-v40 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-40 existing-stem reconciliation evidence, not coverage of order
41, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

[Boundary 67] carries order 41 (x30 / SIG 67 / Inter 1423, grade bits
`0x3fe63a0d1316bff0`). LEFT is already linked and its four-relation LEFT/BOTTOM
candidate resolves active glyph 313 to existing Stem 2357, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is `Neither`; incident Stem 2357 joins
x29 and x30 on LEFT, so x29 LEFT then RIGHT close through two ordered writes. Native
records no unlinked head and reaches `current_index=42` before x43 / SIG 48 / Inter
1385 (grade bits `0x3fe5f802e7abc18c`), whose LEFT is linked and RIGHT remains
open/unlinked.

The v41 oracle retains the heap-safe minimized shape: orders 1-40 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-41
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused gate is green 1/1; the full
14-test sibling suite, strict workspace/all-target/all-features Clippy, formatting,
and diff checks also pass. The schema-v41 derivative is 14 lines / 12,312 bytes with
nine semantic rows plus summary. Fixture/runner/probe/body/semantic pins are
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

[Boundary 68] carries order 42 (x43 / SIG 48 / Inter 1385, grade bits
`0x3fe5f802e7abc18c`). LEFT is already linked and its two-relation LEFT/BOTTOM
candidate resolves active glyph 326 to existing Stem 2350, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is `Neither`; incident Stem 2350 joins
x39, x40, and x43 on LEFT, so x39 LEFT then RIGHT and x40 LEFT then RIGHT close through
four ordered writes. Native records no unlinked head and reaches `current_index=43`
before x25 / SIG 91 / Inter 1471 (grade bits `0x3fe5db5645fe3490`), whose LEFT is
linked and RIGHT remains open/unlinked.

The v42 oracle retains the heap-safe minimized shape: orders 1-41 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-42
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused gate is green 1/1; the full
14-test sibling suite, strict workspace Clippy, formatting, and diff checks also pass.
The schema-v42 derivative is 14 lines / 11,783 bytes with nine semantic rows plus
summary. Fixture/runner/probe/body/semantic pins are
`64b55e449e38f7af6ed47c1ca026236772a277ac8c5917bc5eaea397125b332c`,
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

[Boundary 69] carries order 43 (x25 / SIG 91 / Inter 1471, grade bits
`0x3fe5db5645fe3490`). LEFT is already linked and its three-relation LEFT/BOTTOM
candidate resolves active glyph 292 to existing Stem 2356, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is `Neither`; incident Stem 2356 joins
x24 and x25 on LEFT, so x24 LEFT then RIGHT close through two ordered writes. Native
records no unlinked head and reaches `current_index=44` before x83 / SIG 21 / Inter
1327 (grade bits `0x3fe5b836536dd665`), whose LEFT is linked and RIGHT remains
open/unlinked.

The v43 oracle retains the heap-safe minimized shape: orders 1-42 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-43
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused gate is green 1/1; the full
14-test sibling suite, strict workspace Clippy, formatting, and diff checks also pass.
The schema-v43 derivative is 14 lines / 11,885 bytes with nine semantic rows plus
summary. Fixture/runner/probe/body/semantic pins are
`dc5f7ce12d292a13cc149e7df0249703323df92de9054daf5eff52783b32919d`,
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

[Boundary 70] carries order 44 (x83 / SIG 21 / Inter 1327, grade bits
`0x3fe5b836536dd665`). LEFT is already linked and its two-relation LEFT/BOTTOM
candidate resolves active glyph 301 to existing Stem 2358, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is `Neither`; incident Stem 2358 joins
x82 and x83 on LEFT, so x82 LEFT then RIGHT close through two ordered writes. Native
records no unlinked head and reaches `current_index=45` before x57 / SIG 5 / Inter
1295 (grade bits `0x3fe593d56730c827`), whose LEFT is linked and RIGHT remains
open/unlinked.

The v44 oracle retains the heap-safe minimized shape: orders 1-43 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-44
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused gate is green 1/1; the full
14-test sibling suite, strict workspace Clippy, formatting, and diff checks also pass.
The schema-v44 derivative is 14 lines / 11,456 bytes with nine semantic rows plus
summary. Fixture/runner/probe/body/semantic pins are
`1d5c98477377e64e95a659fa04ed8d8331e02d5e87962811b790ff80f0315515`,
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

[Boundary 71] carries order 45 (x57 / SIG 5 / Inter 1295, grade bits
`0x3fe593d56730c827`). LEFT is already linked and its two-relation LEFT/BOTTOM
candidate resolves active glyph 303 to existing Stem 2374, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is `Neither`; incident Stem 2374 joins
x56 and x57 on LEFT, so x56 LEFT then RIGHT close through two ordered writes. Native
records no unlinked head and reaches `current_index=46` before x40 / SIG 27 / Inter
1339 (grade bits `0x3fe3aa2e83097210`), whose LEFT is linked/closed and RIGHT is
unlinked/closed.

The v45 oracle retains the heap-safe minimized shape: orders 1-44 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-45
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused gate is green 1/1; the full
14-test sibling suite, strict workspace Clippy, formatting, and diff checks also pass.
The schema-v45 derivative is 14 lines / 11,415 bytes with nine semantic rows plus
summary. Fixture/runner/probe/body/semantic pins are
`f70a5aeee405899ee2e9bf3be6957ffa657c6f0bcd5bc5d84ab0fc0288b19073`,
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

[Boundary 72] carries order 46 (x40 / SIG 27 / Inter 1339, grade bits
`0x3fe3aa2e83097210`). LEFT is already linked and closed; its two-relation LEFT/BOTTOM
candidate resolves active glyph 326 to existing Stem 2350, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is closed. Incident Stem 2350 joins
x39, x40, and x43 on LEFT. Java emits ordered x39 LEFT/RIGHT true-to-true writes, then
x43 LEFT/RIGHT false-to-true writes; `closedValueChanges=2`. Native records no
unlinked head and reaches `current_index=47` before x89 / SIG 22 / Inter 1329 (grade
bits `0x3fd6ac9dfd130464`), whose LEFT is linked/closed and RIGHT is unlinked/closed.

The v46 oracle retains the heap-safe minimized shape: orders 1-45 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-46
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused gate is green 1/1; the full
14-test sibling suite, strict workspace Clippy, formatting, and diff checks also pass.
The schema-v46 derivative is 14 lines / 11,504 bytes with nine semantic rows plus
summary. Fixture/runner/probe/body/semantic pins are
`017cfeddc3faeedda3aca5308c82251135bd0c3308854385f77271cb7fc76f8d`,
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

[Boundary 73] carries order 47 (x89 / SIG 22 / Inter 1329, grade bits
`0x3fd6ac9dfd130464`). LEFT is already linked and closed; its one-relation LEFT/BOTTOM
candidate resolves active glyph 304 to existing Stem 2359, leaving allocator, SIG
685/696, and system stems 46 unchanged. RIGHT is closed. Incident Stem 2359 joins
x89 and x90 on LEFT. Java closes x90 LEFT then RIGHT through two ordered false-to-true
writes, with exact `closedValueChanges=2`. Native records no unlinked head and reaches
`current_index=48` before x52 / SIG 2 / Inter 1289 (grade bits
`0x3fd5af02eef9418a`), whose LEFT is linked/closed and RIGHT is unlinked/closed.

The v47 oracle retains the heap-safe minimized shape: orders 1-46 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-47
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused gate is green 1/1; the full
14-test sibling suite, strict workspace Clippy, formatting, and diff checks also pass.
The schema-v47 derivative is 14 lines / 10,882 bytes with nine semantic rows plus
summary. Fixture/runner/probe/body/semantic pins are
`5a7989434b78dbd6ea72f113cd9f66078ae8e9c3acabb8980ecdb7577120de39`,
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

[Boundary 74] carries order 48 (x52 / SIG 2 / Inter 1289, grade bits
`0x3fd5af02eef9418a`). Its linked-and-closed LEFT four-relation candidate resolves
glyph 296 to existing Stem 2344; RIGHT is closed. Java takes `SkipAlreadyLinked` and
`SkipClosed`, closes x53 LEFT then RIGHT, and reports `closedValueChanges=2`. Native
makes no graph mutation and reaches `current_index=49` before x35 / SIG 68 / Inter
1425 (`0x3fd525fff19ec48c`). The v48 gate is focused/full/Clippy/fmt/diff green and
snapshot-minimized. Fixture/runner/probe/body/semantic pins are
`acc3436794b0ea828dbd689adfd072b6844125007131ee4207d9d4402c90cd5d`,
`925536d8d119102e5a74a3690b2286bde856bd476151243806d68a049aa40fdb`,
`af7f62ae73911530d863cbf8e4f2ee8bb3d019cfb556185e5fca334cad8a318d`,
`aa738347bf8581a87c5293e9b549261946b9adfef21c3e07e7d37ebdb21e2907`, and
`1183c4dce1c645a0ee070f1bd12d8796b22d9f0bde91c9421c3bc75db833a80f`;
base v47 runner/fixture are `7a9605cf09f1d78f899423a816c0c6adc2b121786f56c69c271b41da5527f6ab`
and `5a7989434b78dbd6ea72f113cd9f66078ae8e9c3acabb8980ecdb7577120de39`.

[Boundary 75] carries order 49 (x35 / SIG 68 / Java Inter 1425, grade bits
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

[Boundary 76]

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

[Boundary 77]

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

[Boundary 78]

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

[Boundary 79]

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

[Boundary 80]

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

[Boundary 81]

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

[Boundary 82]

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

[Boundary 83]

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

[Boundary 84]

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

[Boundary 85]

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

[Boundary 86]

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

[Boundary 87]

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

[Boundary 88]

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

[Boundary 89]

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

[Boundary 90]

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

[Boundary 91]

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

[Boundary 92]

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

[Boundary 93]

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

[Boundary 94]

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

[Boundary 95]

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

[Boundary 96]

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

[Boundary 97]

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

[Boundary 98]

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

## Boundary 133: final STEMS cleanup

The v104 differential restores Java's real reverse-grade head list and
retriever-owned undefined-side map, then invokes private `finalizeStems`.
`checkHeadStems` finds no head with multiple stem relations, while
`checkNeededStems` finds only the void heads x32 / SIG 50 and x31 / SIG 47
without a stem. Both were already abnormal. The complete call is therefore a
measured no-op: Java stays at 685 vertices / 706 edges and 46 stems, allocator
2385 does not move, and relation, inter, linker, and SIG hashes are identical.

Rust derives the same census from its completed carrier and fails closed if
the queue is unfinished, a multi-stem cleaner candidate appears, a third
stemless head exists, or either abnormal flag differs. Its corresponding
STEMS projection remains exactly 267 vertices / 370 edges with 46 known stems.
The v104 fixture/runner/probe/body/semantic hashes are
`ad1cf4658b6d4f7f30732681f514fe85f6801d5efce2f9629b9347cf513fe8e5`,
`a36fe02337e974fdbb1119d087026d515020fb319fd7d138e4057e37cabfc639`,
`eb3076ccb85a91f032fe8425ed224a12d9aac66a83533e330436567034efb6b3`,
`5e749de69be552e0446b5a530f7cab9eb3e7fcb05211b6dd99a51eddd4d5fc46`, and
`ee5b0fff2387f4ea4b6c5aaa20835cd07af4619f96be0dc33b3faf780e323669`;
strict base-v103 runner/fixture remain
`9714b52a8ce9941ef0832a3f17689fcbdeb5b14246546770cd001167be618b26`
and `2c192a356f2aa9447c6b0bea5a5979086646390043b2b1333983038d5d3d03c4`.
Boundary 134 below supersedes that restriction while preserving the strict
Chula specialization.

## Boundary 134: generic final STEMS cleanup

The native terminal now implements both private finalizer passes generically.
It rebuilds Java's exclusion-derived stem partitions in stable reverse-grade
order, repeatedly removes the strict lowest target contribution, and keeps a
two-relation pair only for Java's physical canonical LEFT/TOP + RIGHT/BOTTOM
share. Target ratios come exactly from `1 + 10 * relation grade`; canonical
geometry uses carried physical medians, exact head bounds/center, extension
points, the 0.2 dy threshold, and the 0.275 anchor margin. Removal callbacks
recompute head and stem abnormal state before `checkNeededStems` marks every
remaining stemless carrier head abnormal. The transaction mutates a clone and
fails closed on incomplete cursors, graph payload, bindings, or stem geometry.

The frozen Java corpus records real cleaner removals on Allegretto system 1
and Zizi system 2, five retained Zizi canonical shares, and a controlled Chula
stemless-head `false -> true` mutation. Warmup plus two fresh runs are
byte-identical. Fixture/runner/probe/init/body hashes are
`d468cb52f59687604d2204b18aa2364bde12355cb476d007ce205788033b350a`,
`ddcaa94b847de8ed50ffdb9e866717da3e888e223117d8453bf06db55ebaa247`,
`f55cc3fe1f8dc85d817ba84499e407dc759f6710cd815b0eb8007bfca02ac0b1`,
`538f75284a798d4cf96e7f4034bf5368e63f50891f58b712b517fe84f6223006`,
and `6d706ff6e8dc4fc63bb580447b91ddb114d6f0f56544b2902b50438d93d09664`.
The native focused gate also covers repeated `>2` pruning, physical canonical
retention, and the abnormal mutation on authenticated carrier state. Generic
`finalizeStems` is complete.

## Boundary 135: production STEMS preparation

`native_stems::prepare_native_stems` is now the production composition point
immediately before the first mutating SIDES transaction. It consumes completed
live GRID, HEADERS, STEM_SEEDS, BEAMS, LEDGERS, and HEADS products and builds
the head/beam stump, VLinker, reachability, builder, plan, scheduler, and native
SIG state in Java order. A component-only companion supports wider pages whose
upstream BEAMS-group SIG is not yet complete. Both paths fail without returning
partial state.

The exact Chula carrier now uses this production entry rather than a test-local
composition chain. Focused 1/1 and full sibling 14/14 gates pass, along with
strict workspace Clippy, formatting, and diff checks. Boundary 136 removes the
first-STEMS snapshot operationally; the sparse selected-base Java identity
bridge and wider mutation/corpus gaps remain.

## Boundary 136: native STEMS glyph identity

`NativeStemsModeledGlyphRegistry` now owns the exact system-visible modeled
glyph prefix and assigns native identity as canonical ordinal plus one. Exact
bounds, weight, and RunTable content—not Java's page-global glyph number—drive
every lookup. The operational path imports neither the Java allocator/union
watermark nor the 592 opaque first-STEMS fingerprint entries.

After the initial legacy bootstrap transaction, the Chula carrier uses this native
registry through the remaining SIDES pass, all STUMPS transactions, all measured
head C-links, and every existing-stem reconciliation. Java glyph numbers remain
oracle descriptions only. Focused 1/1 (17.34s), full sibling 14/14 (153.26s),
strict workspace all-target/all-feature Clippy (23.72s), formatting, and diff
checks pass.

Transaction 1's fixture bootstrap, the sparse selected-base Java InterIndex
bridge, and the reconstructed Allegretto predecessor remain explicit. The
legacy snapshot fixture/API stays only for isolated compatibility gates.

## Boundary 137: native transaction-2 glyph bootstrap

Transaction 2 now resolves plan 152 directly from
`NativeStemsModeledGlyphRegistry` before any transaction-2 expected fixture is
opened. The test no longer reads or parses the disclosed page-wide glyph
registry file. Native exact content feeds the unchanged graph-derived B13/B14
and B15-B19 path.

Focused 1/1 (13.84s), full sibling 14/14 (149.59s), strict workspace
all-target/all-feature Clippy (12.30s), formatting, and diff checks pass.
Transaction 1's compact fixture state and the sparse selected-base Java
InterIndex bridge remain.

## Boundary 138: native transaction-1 B12/B13 bootstrap

The first shared-sheet frontier now uses
`initialize_native_stems_beam_vlink_first_frontier_state_from_modeled_registry`.
It derives both selected bindings, the V-linker line state, native canonical
identity, and complete empty `systemStems` authority from the live scheduler,
plan, and 1,058-entry modeled registry. Java's 1,650-entry GlyphIndex union,
592 opaque fingerprints, and exhaustive equality scan are no longer execution
inputs. Exact candidate content and checker geometry match while the identity
is native glyph 45 rather than Java glyph 294.

B13 now projects transaction 1's all-unlinked reads from the owned SIG and
persistent S cells. That state carries through B14 and the complete terminal
chain. Focused 1/1 and full sibling 14/14 (147.63s) pass; strict workspace
all-target/all-feature Clippy passes in 26.25s; formatting and diff checks are
clean. The shared persistent allocator and sparse selected-base Java InterIndex
bridge remain explicit.

## Boundary 139: native selected-beam identity

`roll_native_stems_beam_vlink_base_apply_state` now resolves every selected B14
beam directly from `NativeSigSystemBindings`. One-based native vertex identity
supplies the persistent identity, native vertex ordinal supplies local
InterIndex order, and VIP is false in the owned native domain. Production no
longer accepts the sparse bootstrap entries, and the integration gate no longer
opens `stems-beam-inter-index-chula-system1.txt`.

The same 16 distinct selected beams across all 32 SIDES transactions are now an
asserted result rather than an authority. Missing native bindings reject before
mutation. Focused 1/1 and full sibling 14/14 (154.47s) pass; strict workspace
all-target/all-feature Clippy passes in 27.70s; formatting and diff checks are
clean. The first B14 compact state's shared persistent-ID seed and opaque
InterIndex baseline remain the next identity seam.

## Boundary 140: native first-B14 compact state

`initialize_native_stems_beam_vlink_base_apply_state_from_native_sig` derives
the first B14 graph, endpoint, beam-group, certificate, and local InterIndex
state from the owned SIG and bindings. Native insertion order is the local
InterIndex domain: the initial baseline is 221 native vertices instead of
Java's opaque 639-entry sheet index, reaching 223 rather than 641 after three
carried transactions. No B14 compact graph/index snapshot drives execution.

All downstream results remain exact. Focused 1/1 and full sibling 14/14
(150.25s) pass; strict workspace all-target/all-feature Clippy passes in
32.58s; formatting and diff checks are clean. Only the shared persistent-ID
counter remains as a first-B14 identity input.

## Boundary 141: native STEMS persistent identities

The first transaction seeds its shared identity domain immediately after the
1,058 modeled native glyphs. StemInter identities allocate from 1,059 through
1,104 rather than inheriting Java's 2,339 EntityIndex watermark. The initializer
accepts no persistent-ID argument, and bounded continuation guards resolve
existing stems by carried `stem_identity` rather than Java Inter IDs.

The complete 102-head path and generic finalizer remain exact; all three
terminal identity counters equal 1,104. Focused 1/1 and full sibling 14/14
(152.10s) pass; strict workspace all-target/all-feature Clippy passes in
29.78s; formatting and diff checks are clean.

## Boundary 142: production-derived modeled-registry boundary

`NativeStemsModeledGlyphRegistry::from_head_builder_recognition` resolves the
requested system's final production head-builder registry event and validates
that visible canonical-glyph prefix against the complete modeled collection.
The carrier no longer receives an independently supplied visible count; the
gate retains that value only to assert the derived registry length.

Focused 1/1 and full sibling 14/14 (150.54s) pass; strict workspace
all-target/all-feature Clippy passes in 31.23s; formatting and diff checks are
clean. The next slice builds the first carrier transaction as a production
operation, then carries Allegretto transactions 1-27 to the measured linked-S
and hook-removal frontier.

## Boundary 143: atomic first SIDES carrier

`initialize_native_stems_beam_sides_carrier_from_modeled_registry` executes the
first B12-B19 transaction on locally owned SIG, binding, B-cell, and S-cell
state, resumes the scheduler, reconciles the committed Stem runtime, validates
the graph, and returns no partial carrier on failure. Its first-frontier state
constructor now accepts any consistently identified system.

The independent Chula reconstruction matches the returned carrier and trace;
all later transactions consume the production result. Focused 1/1 and full
sibling 14/14 (157.64s) pass; strict workspace all-target/all-feature Clippy
passes in 9.33s; formatting and diff checks are clean. Next is native
Allegretto carriage through transactions 1-27.

## Boundary 144: native Allegretto linked-S and hook-removal carriage

The production carrier now executes Allegretto system 1 SIDES transactions
1-28 from the production-derived modeled registry. Transaction 28 derives its
existing-Stem selection from the owned SIG and persistent S cells, then the
typed hook checkpoint removes the competing BeamHook from the naturally
carried five-edge neighborhood and resumes to `SidesExhausted`. No Java-driven
scheduler reconstruction, artificial Stem vertices, substitute edges, or Java
persistent-ID join remains in this gate.

The carry also generalizes four production rules: completed ready-transaction
line-delta rows are one-shot evidence, callback incident rules follow the live
beam runtime class, B14 rollover accepts a graph-bound existing Stem while
keeping persistent and native-SIG identities distinct, and B17 recomputes
abnormality from either a fresh abnormal or existing normal Stem. Focused
linked-S and hook tests pass; the full sibling suite passes 14/14 in 160.24s;
strict workspace all-target/all-feature Clippy passes in 18.57s; formatting and
diff checks are clean. Production-owned relation parameters and wider-system
carriage remain next.

## Boundary 145: production-derived BeamStem relation parameters

`NativeStemsBeamRelationParameters::from_native_products` derives the system
interline and main Stem thickness from native plan/V-linker products, combines
them with the authenticated frontier profile and the ported Java relation
constants, and rejects incoherent inputs. The carrier context no longer
accepts relation parameters, removing the last strict-fixture value used to
execute Chula and Allegretto SIDES/STUMPS transactions.

The gates compare the derived values with frozen Java context rows only after
derivation. Focused Chula, linked-S, and hook-removal tests pass; the full
sibling suite passes 14/14 in 159.65s; strict workspace all-target/all-feature
Clippy passes in 24.34s; formatting and diff checks are clean. Wider system
carriage is next.

## Boundary 146: production-owned STEMS entry edit state

`NativeStemsBeamSheetEditState::at_stems_entry` internalizes the Java/native
entry invariant that prior graph-building stages have already marked the sheet
stub, book, and book dirty. The first-carrier initializer no longer accepts
those flags from callers. Chula and Allegretto match their former strict B14
state and retain identical carried results.

Focused Chula and Allegretto hook gates pass; the full sibling suite passes
14/14 in 157.49s; strict workspace all-target/all-feature Clippy passes in
22.85s; formatting and diff checks are clean. Wider system carriage is next.

## Boundary 147: production-owned checker and first-system SIDES start

`prepare_native_stems` now owns the page-wide StemChecker context derived from
live GRID and STEM_SEEDS state: `NO_STAFF`, interline, maximum Stem thickness,
the ties-to-even `0.15 * interline` belt margin, sheet skew, Java's exact
`0.8 * 0.1` minimum grade, and the `0.4` artificial-Stem grade.
`initialize_first_system_sides` atomically joins the matching system-1 plans,
builders, stumps, VLinkers, reachability, head corners, SIG/bindings, and
modeled-glyph registry, returning those carried products with the committed
first transaction. No checker or system-local execution input comes from a
fixture.

The new third-page gate starts Batuque system 1 at plan 98. It exactly reuses
glyph 265, creates the checked Stem at grade bits `0x3fe91480f4111904`, uses
BeamStem support grade bits `0x3feefb1fb84ea5fd`, links one sibling, commits
the aggregate two B-cell writes, inserts three HeadStem relations/S-cell
writes, and reaches plan 111. The focused gate passes 1/1 in 4.47s; the full
sibling suite passes 15/15 in 166.67s; strict workspace all-target/all-feature
Clippy passes in 25.18s; formatting and diff checks are clean.

This boundary intentionally initializes only system 1. Later systems need the
shared allocator and modeled registry after all prior-system transactions;
isolated system-2 reconstruction remains rejected. Cross-system carriage is
next, followed by wider SIDES/STUMPS/head branches.

## Boundary 148: production first-system SIDES drive

`NativeStemsPreparedRecognition::drive_first_system_sides` now owns the complete
system-1 transaction loop. It starts from Boundary 147's production-derived
checker, modeled registry, SIG, bindings, and committed first transaction, then
uses the generic modeled-registry advance until the scheduler reports true
`SidesExhausted`.

The immutable system-local builder count is a strict progress bound. Empty
builders, a competing-hook checkpoint, an unexpected STUMPS terminal, or
failure to terminate within that bound rejects the complete result; no partial
carrier can escape as system completion. Batuque system 1 executes 33
transactions and finishes with 222 vertices, 263 edges, 32 Stem bindings,
51/93 linked B cells, 71/186 linked S cells, and 24 beams retained for STUMPS.
The retained and final local worklists are identical and all B/S cells remain
open. Boundary 147's first transaction remains exact against Java; this
terminal vector grades the production driver over the already authenticated
components and is not represented as a new full-chain Java snapshot.

The focused gate passes 1/1 in 3.76s; the full sibling suite passes 15/15 in
159.88s; strict workspace all-target/all-feature Clippy passes in 23.77s;
formatting and diff checks are clean. Cross-system allocator, registry, and
persistent-carrier chronology was therefore the next boundary.

## Boundary 149: exact cross-system registry and allocator handoff

`NativeStemsModeledGlyphRegistry::carry_into_next_system` joins the complete
system-1 modeled prefix with every exact canonical learned by its finished
transaction state, then replays system 2's head-builder constructor events in
production order using full bounds/weight/RunTable equality. Selected candidates
resolve structurally rather than by a precomputed modeled ordinal, which cannot
serve as a page identity after StemInter allocations interleave the shared ID
stream.

The handoff rejects isolated/nonconsecutive state, identity/content collisions,
weak-only entries whose Java liveness is unknown, and a union count not covered
by exact carried content. Sheet, GlyphIndex, and InterIndex allocator views
advance together only on a structural miss; errors leave caller state intact.

Batuque system 1 retains 1,058 structural glyphs while 32 Stem inters advance
the allocator to 1,090. Replaying system 2's 1,125 constructor events yields
1,470 structural glyphs and allocator 1,502. The isolated system-2 model also
has 1,470 glyphs but ends at allocator 1,470, proving why reconstruction would
collapse the 32-ID interleaving gap. Weak-liveness and incomplete-union
counterexamples both reject atomically.

The focused gate passes 1/1 in 3.78s; the full sibling suite passes 15/15 in
157.08s; strict workspace all-target/all-feature Clippy passes in 8.66s;
formatting and diff checks are clean. Building system 2's SIG/bindings/cells
and first serial SIDES carrier was therefore next.

## Boundary 150: first shared-sheet serial SIDES carrier

`NativeStemsPreparedRecognition::initialize_second_system_sides` now drives
system 1 to its true SIDES terminal, applies Boundary 149's exact registry and
allocator handoff, selects system 2's native scheduler/SIG/bindings/products,
and executes its first B12-B19 transaction atomically. The lower-level serial
initializer starts with empty system-local `systemStems` and B/S cells while
retaining the carried page edit state and an explicit `SharedSheetSerial`
scope.

Batuque system 2 enters with 1,470 structural glyphs and persistent allocator
1,502 rather than the isolated registry-length seed 1,470. Its first native
plan is 514 / builder 105 / profile 4 and returns `CreatedChecked` for stem
identity 0. The commit advances the allocator to 1,503, retains union size
1,470, records one exact canonical and one system stem, and yields a fresh
240-vertex / 199-edge SIG with one stem binding, 117 B cells, and 244 S cells.

No new Java full-chain snapshot is claimed: the boundary composes the already
graded constructor, scheduler, B12-B19, registry, and allocator authorities.
The focused gate passes 1/1 in 3.78s; the full sibling suite passes 15/15 in
158.54s; strict workspace all-target/all-feature Clippy passes in 25.68s;
formatting and diff checks are clean. Driving the remaining system-2 SIDES
worklist and widening later-system branches was therefore next.

## Boundary 151: complete Batuque system-2 SIDES drive

The system-agnostic bounded driver now consumes Boundary 150's serial start
through every remaining awaited SIDES frontier. It retains the same immutable
builder-count progress bound and refuses competing-hook, STUMPS-completed, and
malformed terminals without returning partial state.

Batuque system 2 executes 40 `SharedSheetSerial` transactions and reaches true
`SidesExhausted`. The shared allocator advances from 1,502 to 1,542 for 40
system stems; the final SIG has 279 vertices / 349 edges and 40 stem bindings.
Exactly 64/117 B cells and 89/244 S cells are linked, all remain open, and the
33 retained STUMPS items exactly equal the final local worklist.

The focused gate passes 1/1 in 4.15s; the full sibling suite passes 15/15 in
160.94s; strict workspace all-target/all-feature Clippy passes in 24.43s;
formatting and diff checks are clean. Carrying this terminal registry/allocator
into system 3 and widening later-system STUMPS was therefore next.

## Boundary 152: complete three-system Batuque SIDES page

`drive_all_system_sides` now returns only after every consecutive scheduler
system reaches true `SidesExhausted`; each later start consumes the preceding
committed registry, allocator, and edit state. This exposed and closed two
generic gaps: B17 now accepts the ordered successful relation-map subset when
one of four head targets is rejected, and the owned registry performs an exact
page-wide compound equality scan before `registerOriginal`.

All three Batuque systems finish 33 + 40 + 28 transactions. System 3 enters at
registry 1,819 / allocator 1,891, registers absent two-glyph compound 1,915,
and finishes at union 1,820 / allocator 1,920 with 28 stems. Its terminal is
SIG 244/257, B 50/101, S 63/224, and 25 retained STUMPS items equal to the
final worklist. Transaction 7 links three accepted relations from four head
targets; transaction 24 owns the compound absence proof. Weak-only liveness
still rejects atomically.

The focused gate passes 1/1 in 4.07s; the full sibling suite passes 15/15 in
160.98s; strict workspace all-target/all-feature Clippy passes in 20.36s;
formatting and diff checks are clean. Wider-system STUMPS carriage is next.

## Boundary 153: production Batuque system-1 STUMPS completion

The production-prepared system-1 carrier now crosses SIDES→STUMPS and executes
all eight retained transactions atomically. It reaches the typed post-STUMPS
terminal at allocator 1,098, 40 known/bound Stems, SIG 230/297, B 67/93, and S
89/186; all 24 retained beams equal the final local worklist.

This wider page exposed two real authority corrections. B14 rollover now
accepts and strictly validates both the fresh-stem 1/1/1 append shape and the
existing-stem 0/0/1 shape, matching the recorded base edge back to its live
native SIG origin without assuming it is the graph's final edge. B19 STUMPS
resume authenticates B16 sibling B-linker writes against `StemBuilder` items,
the catalogue Java queried, rather than requiring every sibling target to own
a standalone V-linker constructor. Duplicate, primary, unknown, and corrupted
predecessors remain typed failures; a corrupt reused-stem edge is covered by
an unchanged-carrier batch rollback assertion.

Focused Batuque passes 1/1 in 3.95s; the full sibling suite passes 15/15 in
151.29s; strict workspace all-target/all-feature Clippy passes in 19.98s;
formatting and diff checks are clean. Systems 2-3 STUMPS and their shared-page
registry/allocator handoffs remain next.

## Boundary 154: complete three-system Batuque STUMPS page

The page driver now serializes SIDES and STUMPS per system instead of driving
all SIDES systems first. This ensures that system 2's constructor chronology
sees system 1's post-STUMPS allocator, and system 3 likewise sees all earlier
mutations. No partial vector is returned when a later system fails.

Batuque executes 8, 14, and 20 STUMPS transactions. Terminal facts are system
1 allocator 1,098 / 40 stems / SIG 230/297; system 2 carried registry allocator
1,510 then final allocator 1,564 / 54 stems / SIG 293/406; and system 3 carried
registry allocator 1,913 then final allocator 1,962 / 48 stems / SIG 264/339.
The three registry lengths remain 1,058, 1,470, and 1,819, all later
transactions retain `SharedSheetSerial`, and every scheduler reaches a true
completion with identical retained/final worklists.

Focused Batuque passes 1/1 in 4.20s; the full sibling suite passes 15/15 in
154.38s; strict workspace all-target/all-feature Clippy passes in 22.22s;
formatting and diff checks are clean. Page-wide head transfer and wider
head/retry branch closure are next.

## Boundary 155: enter page-wide Batuque head linking

The production page driver now transfers all three completed Batuque STUMPS
carriers into generic phase-1 head linking as one atomic result. It validates
the live SIG, bindings, persistent S cells, and reverse-grade queues, closes
prelinked prefixes of 7/79/48 heads in queues of 93/122/112, and returns the
first C-link frontier for each system at `(staff,head,SIG,x)`
`(1,30,84,56)`, `(1,57,115,108)`, and `(1,57,105,110)`.

Each returned frontier is the single LEFT/BOTTOM choice with no unlinked or
undefined head. The first/last prefix references are `(1,28,82,4)` /
`(1,24,78,86)`, `(1,18,76,13)` / `(1,38,96,84)`, and
`(1,61,108,6)` / `(0,15,14,32)`. Prefix closure evidence is carried
separately from later C-link mutation and is derived only from the native SIG,
stem bindings, and S cells. Dual-corner and rather-good retry/no-link branches
remain fail closed.

Focused Batuque passes 1/1 in 4.30s; the full sibling suite passes 15/15 in
152.96s; strict workspace all-target/all-feature Clippy passes in 23.13s;
formatting and diff checks are clean. Consuming these three C-link frontiers
and continuing their remaining head queues is next.

## Boundary 156: execute the first page-wide head outcomes

`prepare_native_stems` now retains the accepted free-glyph authority needed by
head C-links, and the production page driver consumes all three first
frontiers without order-specific Java identities or measured expansion
indices. Systems 1 and 3 create one Stem and HeadStem edge at x56 and x110,
advancing to indices 8 and 49 with SIG 231/298 and 265/340.

System 2 supplies the first wider-corpus normal C-link rejection. Its
18-pixel start item cannot reach Java's 37-pixel hard tail, so the generic
`linkSides` loop retries eligible profiles/sides, closes both S cells, queues
the head for phase 2, and advances to index 80 without changing SIG 293/406.
The hard target is measured from the selected corner reference point in both
created- and existing-stem paths, matching Java `CLinker.link`.

Focused Batuque passes 1/1 in 4.48s; the full sibling suite passes 15/15 in
154.01s; strict workspace all-target/all-feature Clippy passes in 19.10s;
formatting and diff checks are clean. Remaining head queues, wider reuse and
expansion, and phase-2 append retries remain next.

## Boundary 157: carry every page system to its next head frontier

The production continuation loop now advances each system after its first
mixed outcome through native prelinked closures and defined false results,
stopping only at the next actionable frontier or phase-1 completion. System 1
carries 18 continuations to index 25 at `(staff,head,SIG,x)=(1,34,88,76)`;
system 2 remains at index 80 before `(1,63,121,109)` with its one phase-2
retry head intact; system 3 remains at index 49 before `(0,47,46,111)`.
All three frontiers select LEFT/BOTTOM.

No SIG, registry, allocator, or Stem mutation occurs here. Focused Batuque
passes 1/1 in 4.82s; the full sibling suite passes 15/15 in 163.20s; strict
workspace all-target/all-feature Clippy passes in 25.56s; formatting and diff
checks are clean. Consuming the three next frontiers is next.

## Boundary 158: execute the second page-wide head outcomes

The generic page transaction now preserves carried phase-2 retry/undefined
state while consuming x76, x109, and x111. Systems 1 and 3 create one Stem
vertex and HeadStem edge, advancing to index/SIG 26/232/299 and 50/266/341.
System 2 takes another normal rejected-link closure at x109, advances to index
81 with two ordered phase-2 retry heads, and leaves SIG 293/406 unchanged.
The old early-Chula empty-retry and prefix-equals-index assumptions are gone;
the 18/1/1 prior continuation traces remain attached.

Focused Batuque passes 1/1 in 5.09s; the full sibling suite passes 15/15 in
162.53s; strict workspace all-target/all-feature Clippy passes in 25.76s;
formatting and diff checks are clean. Continuing to the following action
frontiers is next.

## Boundary 159: complete three-system Batuque head phase 1

The production driver now alternates native continuation and action outcomes
until all three reverse-grade queues reach their true terminals. Generic
existing-stem reuse adds only the HeadStem edge and S-cell/sibling closures;
same-stump dual corners remain undefined, while differing/missing stumps take
Java's BOTTOM-on-LEFT or TOP-on-RIGHT standard connection.

System 1 completes 93 heads with prefix/events `7/89`, 2 creates, 2 reuses,
no retry heads, and SIG/stems/allocator `232/301,42,1100`. System 2 completes
122 with `79/44`, 2 no-link outcomes/retry heads, and unchanged
`293/406,54,1564`. System 3 completes 112 with `48/69`, 4 creates, 1 reuse,
2 retry heads/2 undefined sides, and `268/344,52,1966`. Every carrier is
consumed at queue length with phase-two index zero.

Focused Batuque passes 1/1 in 5.11s; the full sibling suite passes 15/15 in
156.59s; strict workspace all-target/all-feature Clippy passes in 25.06s;
formatting and diff checks are clean. Page-wide phase-2 append retry is next.

## Boundary 160: complete Batuque head phase 2 page-wide

`drive_all_system_head_linking_phase2` now composes the complete phase-1 page
drive with every carried phase-2 append retry on local shadows. The queue
authenticator uses native head/S-cell identities and insertion order rather
than Chula ordinals; malformed cursors, duplicate/missing heads or sides, and
an append that reaches Java's real `reuseStem` mutation reject the whole page.

System 1 has no retries. System 2 consumes x108/SIG115 and x109/SIG121: their
already-closed sides evaluate BottomOnly/Neither and BottomOnly/TopOnly,
respectively, both expansions stop before reuse, both calls return false, and
the two ordered `setClosed(true)` attempts per head change zero values. System
3 consumes x107/SIG47 and x108/SIG2: x107 first takes the standard LEFT/BOTTOM
choice and then returns undefined on the RIGHT shared stump; x108 returns
undefined on its LEFT shared stump immediately. Neither changes a side or the
graph. Terminal SIG/stem counts remain `232/301/42`, `293/406/54`, and
`268/344/52`; phase-two indices are `0/2/2` at queue lengths `0/2/2`.

The independent Java page probe reconstructs real Batuque HEADS state, runs
SIDES/STUMPS and both head phases in foreground system order, and freezes all
four rows after a warmup plus two fresh byte-identical passes. Fixture/runner/
probe/init/body SHA-256 are `41992cf6702bc27b918733e6a1a097c22b729c6dfc7fe332e8603c5e6a02983a`,
`b0e79187886052aa20ac15421da2eb5169d541b305ef0f04460dfc05add094d6`,
`7b467c57b65e57aa052296164129ae8c016d82756c9f804d8e1072747b0a76b2`,
`1defbc545668eb711395283bc0d8f9216b7402ad3b0f2f64f93812ac739c495e`,
and `3d30e22eca5ee67647519fed576083a66ed987bd8803376e72c5462f5758d021`.
Focused Batuque passes 1/1 in 5.51s; the full sibling suite passes 15/15 in
152.69s; strict workspace all-target/all-feature Clippy passes in 20.10s;
formatting and diff checks are clean. Page-wide `finalizeStems` is next.

## Boundary 161: finalize Batuque STEMS page-wide

`finalize_all_system_stems` composes the complete page phase-2 carrier with
generic `finalize_native_stems`, evaluates each system on a local shadow, and
publishes no page result unless all three finalizers succeed. A dedicated Java
probe executes the real Batuque SIDES, STUMPS, both head phases, and private
`finalizeStems` in foreground system order.

Java and Rust agree exactly: system 1 checks 93 heads with no abnormal result;
system 2 checks 122 and retains x108/SIG115 plus x109/SIG121 as no-stem
abnormal heads; system 3 checks 112 and retains x107/SIG47 plus x108/SIG2,
with their carried RIGHT/LEFT undefined sides. There are no multiple-stem
heads, removed HeadStem relations, abnormal value changes, graph changes,
allocator changes, or system-Stem changes. Terminal graph/stem counts remain
`232/301/42`, `293/406/54`, and `268/344/52`.

Warmup plus two fresh Java runs are byte-identical. Fixture, runner, probe,
init, and emitted-body SHA-256 are
`ab6377a2b82cc838633b8c0d79732ddd755a68f11a8b7e40dd39baee7d6278d2`,
`7e8b8c557d1d321329c72e62cdd932e0faa304591e14b958171ff7a961342ea1`,
`9b5e9dbefbf400887f49feba934c573d851c67e65b3e43bfaabc86d6f2c36714`,
`e0ff89792bf75286317ef011e079f338696d29cc14918f4a3018307ba4ed9548`,
and `e51e06eb798e3ab6ccaa32ea5db5b88f6285b667fb8162e1777a0faf6c28a3a1`.
Focused Batuque passes 1/1 in 14.17s; the full sibling suite passes 15/15 in
156.66s; strict workspace Clippy passes in 19.88s; formatting and diff checks
are clean. Transactional recognition and schema-1
publication remain next.

## Boundary 162: transactional `recognize_native_stems`

The stage now has a one-call, fail-closed production entry point.
`recognize_native_stems` consumes completed native GRID, HEADERS, STEM_SEEDS,
BEAMS, LEDGERS, and HEADS products; constructs the immutable STEMS products and
native SIG; drives page SIDES/STUMPS, both head-linking phases, and generic
`finalizeStems`; and returns nothing unless every page system reaches its
finalized terminal. `NativeStemsRecognition` owns the complete construction
products and each system's final SIG/registry and transaction traces.

The Batuque gate recomputes the stage through this entry point and requires its
entire result to equal the independently stepped and Java-graded page path.
Boundary 161's fresh deterministic fixture remains the external oracle. No new
transformed evidence, disclosed Java identity, or partial state enters the
recognition result. Focused Batuque passes 1/1 in 13.80s; the full sibling
suite passes 15/15 in 142.75s; strict workspace Clippy passes in 20.01s;
formatting and diff checks are green. Schema-1 ordinary and stream
publication is next.

## Boundary 163: schema-1 STEMS publication

`-step STEMS -json` now composes all seven native stages through the
transactional entry point. The unchanged schema-1 envelope retains every
upstream product and adds one `stems` object containing per-system terminal
summaries, all accepted Stem geometry/thickness/grade rows, HeadStem payloads,
multiple/no-stem/abnormal head sets, and undefined sides. Its identities are
explicitly native and system-local, never fabricated Java `InterIndex` IDs.

Batuque publishes 148 final Stems, 323 live HeadStem relations, 327 checked
heads, and four abnormal no-stem heads. Ordinary and streamed STEMS documents
are byte-identical, and the seven stage markers remain monotonic. The complete
CLI suite passes, including the 17.63s live ordinary/stream gate; all 11 report
tests pass; strict workspace Clippy passes in 12.06s; formatting and diff
checks are green. Wider-corpus branch coverage and exact remote CI remain.



[Boundary 43] extends the same prelinked-success continuation to head order 17 (x48 / SIG
29 / Inter 1343, grade bits `0x3fe80cc40bda9d4c`). Shared Stem 2351 closes x47 LEFT
then RIGHT, two ordered writes, without adding SIG vertices, edges, or system stems;
native reaches `current_index=18` before x63 / SIG 17 / Inter 1319. That next head starts
with both sides open/unlinked (`LEFT:false:false,RIGHT:false:false`); this boundary does
not execute it. Its separate schema-v17 derivative is 12 lines / 8,194 bytes with seven
semantic rows plus summary, SHA-256
`8e4909edc2196f2baff6f517693f9a9af50405cf85fc88bcf3e771711bae2b4b`, with
runner/probe/body/semantic pins `84c176b45ec8adb7af8e0ab1014acabfe8c57c2e6b3cbbe5e8bbd0e971823196`,
`b139149dd41b5581d96344617c2f52b49a85f085f011ff4b556b237f58765342`,
`2362b903486db2d4ddbc14aeeeb54761205bdd06a206875ef0c131a7a22e5fdd`, and
`c89f5a49456af435e2fb508e0ccbbd5a7b8fd9877616534cb7136ccd0ff84ecf`.
Orders 1-16 execute only to reconstruct the predecessor and do not emit or persist full
intermediate snapshots; only order 17 is emitted, keeping deterministic replay under
the full-snapshot heap limit. Two fresh runs are byte-identical and pin the base v16
fixture/runner. This does not independently snapshot-oracle the predecessor states or
cover order 18, actually-unlinked/retry, phase-2 append, later C-link shapes, or broader
systems.

[Boundary 42] extends the same prelinked-success continuation to head order 16 (x8 / SIG
53 / Inter 1395, grade bits `0x3fe81161126880f9`). Shared Stem 2376 closes x7 LEFT
then RIGHT, two ordered writes, without adding SIG vertices, edges, or system stems;
native reaches `current_index=17` before x48 / SIG 29 / Inter 1343. Its separate
schema-v16 derivative is 12 lines / 8,189 bytes with seven semantic rows plus summary,
SHA-256 `04d35bb21c808dc38edd93c0631b3a01af9931efc8f500422646adf8f7123de4`,
with runner/probe/body/semantic pins
`d6edd52b746acd625c2e516f328c4b43253e23bbbe906ffcdae0b3674eae1dcf`,
`d4dcad17952d2de86de193bd87c3a96916ad7781d67f1ea469180e05e4e106fd`,
`49b97d61e08769b58a449edf2931313f91a5855000fd89e27761330f30a81077`, and
`88ea097c4a003e7493c5d28296cc6dd778486660bb6a1e3eb1bfb5aa71f40f7d`.
Orders 1-15 execute only to reconstruct the predecessor and do not emit or persist full
intermediate snapshots; only order 16 is emitted, keeping deterministic replay under
the full-snapshot heap limit. Two fresh runs are byte-identical and pin the base v15
fixture/runner. This does not independently snapshot-oracle the predecessor states or
cover the remaining queue, actually-unlinked/retry, phase-2 append, later C-link shapes,
or broader systems.

[Boundary 41] extends the same prelinked-success continuation to head order 15 (x67 / SIG
59 / Inter 1407, grade bits `0x3fe814269b1247c7`). Shared Stem 2375 closes x66 LEFT
then RIGHT, two ordered writes, without adding SIG vertices, edges, or system stems;
native reaches `current_index=16` before x8 / SIG 53 / Inter 1395. Its separate
schema-v15 derivative is 12 lines / 8,191 bytes with seven semantic rows plus summary,
SHA-256 `aae5116a32e0fd77bb9f4a26dc1a8c1cd53a3f3ff35ea01d350c97012a146ca8`,
with runner/probe/body/semantic pins
`e595eefa74453ecfe9980cb294b80d37d0ff5ad1e2f3e01d88f8801d0f23ca18`,
`98ac227864e84c3693d5368a85adf970512648a9a99c74a2b612a01d4b45d065`,
`1e198195daf91b8d56ebcc2a88a5e97fc2752603f365d0d5cea3145f9a1f1ef2`, and
`55323828f0e4c8e08d85373684f71b7ec9a6f2e75a49278006dae1b8ec673cd9`.
Orders 1-14 execute only to reconstruct the predecessor and do not emit or persist full
intermediate snapshots; only order 15 is emitted, keeping deterministic replay under
the full-snapshot heap limit. Two fresh runs are byte-identical and pin the base v14
fixture/runner. This does not independently snapshot-oracle the predecessor states or
cover the remaining queue, actually-unlinked/retry, phase-2 append, later C-link shapes,
or broader systems.

[Boundary 40] extends the same prelinked-success continuation to head order 14 (x12 / SIG
63 / Inter 1415, grade bits `0x3fe8187dd5fbfd0c`). Shared Stem 2349 closes x11 LEFT
then RIGHT, two ordered writes, without adding SIG vertices, edges, or system stems;
native reaches `current_index=15` before x67 / SIG 59 / Inter 1407. Its separate
schema-v14 derivative is 12 lines / 8,192 bytes with seven semantic rows plus summary,
SHA-256 `f60e5dff377e5e51038ec061b1ebeec5a5868f4cd51af6b9618377bfa3a12e6a`,
with runner/probe/body/semantic pins
`6b5e339f8b91db08d4e03edf7ed3b69ea8ab713b98ce95c62a95440a0652ccb9`,
`eea0869093b1c1a262da5da0d7ad914f3dc7b6a8d771a32bc60849687291c834`,
`9ebf233711be059ddee5adf964b6bbbbe44770caef19f5903c8ce9a5a16d1889`, and
`14d0e0c71dff0f40e5745858ad10d615c56463291cf6caa863edd2ebccde0590`.
Orders 1-13 execute only to reconstruct the predecessor and do not emit or persist full
intermediate snapshots; only order 14 is emitted, keeping deterministic replay under
the full-snapshot heap limit. Two fresh runs are byte-identical and pin the base v13
fixture/runner. This does not independently snapshot-oracle the predecessor states or
cover the remaining queue, actually-unlinked/retry, phase-2 append, later C-link shapes,
or broader systems.

[Boundary 39] extends the same prelinked-success continuation to head order 13 (x53 / SIG
3 / Inter 1291, grade bits `0x3fe83971fb8b04c3`). Shared Stem 2344 closes x52 LEFT
then RIGHT, two ordered writes, without adding SIG vertices, edges, or system stems;
native reaches `current_index=14` before x12 / SIG 63 / Inter 1415. Its separate
schema-v13 derivative is 12 lines / 8,188 bytes with seven semantic rows plus summary,
SHA-256 `ff27fa03e80e44e554d46682c827097ecec1d463abf0c0e131a6ab1beccfbb5e`,
with runner/probe/body/semantic pins
`675bce84bfa4e76ed78cc72592da9f8fe95571752d424da99bd4be93af7478f8`,
`915bc4a3563943b93fa806a614b835da8e7799732cf8c1c1c7aa9127fc39a61e`,
`84254e3f9dc1e4297b4efaabb30c36d07244ffe3d268cce5097ec14d365ab974`, and
`f2b4a2e49aee6fd27d41470eb38a1bfe541d72688b03bb33d5b3ed3266514519`.
Orders 1-12 execute only to reconstruct the predecessor and do not emit or persist full
intermediate snapshots; only order 13 is emitted, keeping deterministic replay under
the full-snapshot heap limit. Two fresh runs are byte-identical and pin the base v12
fixture/runner. This does not independently snapshot-oracle the predecessor states or
cover the remaining queue, actually-unlinked/retry, phase-2 append, later C-link shapes,
or broader systems.

[Boundary 38] extends the same prelinked-success continuation to head order 12 (x55 / SIG
79 / Inter 1447, grade bits `0x3fe847463fc14b09`). Shared Stem 2362 closes x51
LEFT/RIGHT and x54 LEFT/RIGHT, four ordered writes, without adding SIG vertices, edges,
or system stems; native reaches `current_index=13` before x53 / SIG 3 / Inter 1291.
Its separate schema-v12 derivative is 12 lines / 8,273 bytes with seven semantic rows
plus summary, SHA-256
`e8b19156d29722a74b41e6d07d1591edd78b3077844f6be7268fa78754a1acd2`, with
runner/probe/body/semantic pins `74b6ba4f84c046ae2ca08e270ce9726acee42a14f4b639282bfbccd3c8b654d1`,
`7b8f232f56d92f83966311478de6b0255820d6d00c9aa4dbb3f0f9351c43abc6`,
`ab41455ece56d8cce145f1105a417315be379f3c6d644efca539d008db1c099a`, and
`ad4dd95c5b9c12f101a8c2420cca76902e7cc7571b3277bfbd879a6ba4bcda67`.
Orders 1-11 execute only to reconstruct the predecessor and do not emit or persist full
intermediate snapshots; only order 12 is emitted, keeping deterministic replay under
the full-snapshot heap limit. Two fresh runs are byte-identical and pin the base v11
fixture/runner. This does not independently snapshot-oracle the predecessor states or
cover the remaining queue, actually-unlinked/retry, phase-2 append, later C-link shapes,
or broader systems.

[Boundary 37] extends the same prelinked-success continuation to head order 11 (x46 / SIG
57 / Inter 1403). Shared Stem 2377 closes x44 LEFT/RIGHT and x45 LEFT/RIGHT, four
ordered writes, without adding SIG vertices, edges, or system stems; native reaches
`current_index=12` before x55 / SIG 79 / Inter 1447. The v11 fixture SHA-256 is
`cad1527e556481a073ead938094de9edce09954e366bf5608ebc57a30ef946a3`, with
runner/probe/body/semantic pins `f05ea06f61193785a84440b457b4e79b10e7d88e765b81bce51d6f996beb1702`,
`24f67a53e407909d07e1fc12bb2e180b15e6dfcf74983d52d1326cff906284ca`,
`a0716a3379db5d268419624a193d6a6d1dc0105f78ff56fecd44fa70272165e4`, and
`eefa750fd63fa91fec84c2fd9afc62b82d51081da606a0687496a111f5059602`.
Actually-unlinked/retry, phase-2 append, later C-link shapes, and broader corpus/system
coverage remain open.

[Boundary 36] extends the same prelinked-success continuation to head order 10 (x65 / SIG
95 / Inter 1479). Shared Stem 2346 closes x64 LEFT then RIGHT, two ordered writes,
without adding SIG vertices, edges, or system stems; native reaches `current_index=11`
before x46 / SIG 57 / Inter 1403. The v10 fixture SHA-256 is
`7b0bf32fcf75cf792eb67c2c8a52ae9702de215078a54bea7edc7cde853869d0`, with
runner/probe/body/semantic pins `ddf5b4c3f6d726c3e7d91de33d077930ff29254f1e7e84751ee391614978c464`,
`e7cf9dd3ceed19c3e387eabffb587005acb01725434776fe39501605ce4cd4af`,
`cbab4a06edd591e068007152dbb623d206a29c450aa2f9a153c75010fa184658`, and
`d5cd5dbed69852e48add157efd936ba8501879c30a023e730e0825c38825b712`.
Actually-unlinked/retry, phase-2 append, later C-link shapes, and broader corpus/system
coverage remain open.

[Boundary 35] extends the same prelinked-success continuation to head order 9 (x42 / SIG
93 / Inter 1475). Shared Stem 2352 closes x41 LEFT then RIGHT, two ordered writes,
without adding SIG vertices, edges, or system stems; native reaches `current_index=10`
before x65 / SIG 95 / Inter 1479. The v9 fixture SHA-256 is
`b0d3c67f9b76a56a528d8a962f3f2bc54710616f2e86650ac8500e750534ff2c`, with
runner/probe/body/semantic pins `368724efe73e194aff024d68204d758d089d81511e9bbfaa4dfb9ef9516f4c48`,
`caf6a7f25cc36cbe7480c7cb798a8c900bbd526fa7e4071d625724045bb88af5`,
`dbe0a891add7c613c340dfaee983a75c97b20cba4744ca19619e10cd9f7a78f5`, and
`8e271204197c0d84afe4948a94f6723f6a419cd9611495aa8ca74fb7731bbf95`.
Actually-unlinked/retry, phase-2 append, later C-link shapes, and broader corpus/system
coverage remain open.

[Boundary 34] extends the same prelinked-success continuation to head order 8 (x95 / SIG
100 / Inter 1489). Shared Stem 2364 closes x91 LEFT/RIGHT and x94 LEFT/RIGHT, four
ordered writes, no unlinked head, and native reaches `current_index=9` before x42 / SIG
93 / Inter 1475. The v8 fixture SHA-256 is `82eca291e69ec27e49903d31b1da408f68962469780a1f706f3f979564e8aebb`,
with runner/probe/body/semantic pins `4d3be4619b7fbe5f5ca39e4065914fe7bb2a56dcbbfb6ae67c95cf444140edfc`,
`fe2bd835c8359810099881288608bc0055336f1ebb77e6715aa2946570181867`,
`a5460ce6a40756092d2e2dc91975ac5c2665c480370249084faa141d7b45eca8`, and
`062721eabd59d1d4b4bc5d4c18b3d6ee8e510c68d76473278e6cb60c5e2f7597`.
Later retry, phase-2 append, C-link shapes, and broader corpus/system coverage remain open.

[Boundary 33] is a bounded continuation-specific head C-link. From the carried order-7
frontier (x76 / SIG 97 / Inter 1483), Java selects LEFT/BOTTOM `BottomOnly`, reuses
active glyph 319, and creates checked Stem Inter 2380 with one HeadStem relation. Native
atomically advances the dense graph 679/690 to 680/691, adds Stem binding 41, links the
LEFT S cell, and reaches `current_index=8` before x95 / SIG 100 / Inter 1489. The v7
fixture is 20 lines / 18,778 bytes, SHA-256 `8df7d36e780e90e569fcc37144bd48ff43e5b647f9cdc240d899ee10386b153d`,
with runner `87a12b97b6d9c79e6c0d346f8187b426505ab5e0e7785bd07a5984a03a18c197` and
transformed-probe/body/semantic pins `93c6771d55b814cff4155d4065d94a322767df9a668033bc7f2e5ea1ea7f6edd`,
`06285da43ff0b5a1f3644c4468570a10f24c0c8f2b8173e9e7d1e268284704d6`, and
`68d581d84f21a79c41df3d4ebf6a856cc0dee266288512e4cd1e44bb3260fa0c`. Later queue,
actual-unlinked/retry, phase-2 append, recursive/multi-item C-linking, wider corpus,
and full STEMS remain open.

## Boundary 164: Chula system-1 wider reuse composition

The transactional page driver now carries Chula system 1 through its three
wider existing-stem reuse frontiers at orders 67, 70, and 73. It supplies the
owned accepted free-seed glyph slice to the already-graded transactions; no
Java identity table or result fixture enters production. The single-head reuse
at order 72 still runs through the generic path.

Live `chula.png` now clears system 1 and rejects atomically at the newly exposed
system-2 queue-54 x46/SIG94 start/head/chunk expansion. The focused page gate
pins this safe stopping point, and failure diagnostics now include the exact
builder item shape. Frozen oracle data is unchanged. Next is a measured
system-2 transaction and generic wider multi-head reuse dispatch.

## Boundary 165: Chula system-2 wider reuse

Two byte-identical Java passes classify system 2 queue 54 x46/SIG94 as a
LEFT/BottomOnly existing-stem reuse: start + crossed x45 + chunk reuses glyph
376 / StemInter 2285, adds two HeadStem edges, closes x45 and x47, and allocates
nothing. The bounded fixture SHA-256 is
`421c6b99552071e39e6b72a3963f5ac46daf41b3bd0c9a560ea45251868f5c09`.

Native content resolves the candidate to Stem 45/glyph 127 and produces the
same structural transaction. Chula systems 1 and 2 now finish phase 1; system
3 queue 109 x41/SIG122 is the next fail-closed stump-less start-plus-chunk gap.

## Boundary 166: generic stump-less rejection and Chula completion

The snapshot-minimized Java system-3 replay shows x41/SIG122 first attempting
LEFT/TOP. Its chunk-expanded final relation rejects with `lastIndex=-1`, so
Java continues to RIGHT/BOTTOM, reuses active glyph 425 / StemInter 2296, adds
one HeadStem edge, links the RIGHT side, and advances without allocation or
stem/vertex/registry insertion. The three deterministic full-pass hashes are
`d07bfcc6915fae64fb8481be8f6b3aaccc6e768a349e9af8b3ea0c46d90ae142`,
`6cf71daa00322c0e6d20cd745d7d0cf68b2bc7b196a8ab1c3c507bf361ad5c4b`, and
`6260e8b63601ac71e00a253ce2c803f8373a293e183e78983209742c2dd96788`;
the frozen fixture SHA-256 is
`930a9f936f4c5f1eb535e3256e815f44a08f9b96b5aef1fcc52c0c9b28300a15`.

Rust now handles stump-less single-chunk attempts in the generic C-link loop:
a rejected final relation is a non-mutating corner failure, after which the
opposite-side native-content reuse proceeds normally. Chula now completes all
three systems through phase 1, phase 2, generic `finalizeStems`, and
transactional `recognize_native_stems`. The next boundary comes from wider
corpus coverage rather than another Chula head.

## Boundary 167: production Allegretto hook removal

The page-wide SIDES driver now consumes a typed competing-hook checkpoint with
the existing atomic native removal/resume transaction. Returned SIDES and
SIDES+STUMPS systems retain every removal rather than discarding this mutation
authority. Malformed graph, group, binding, exclusion, or scheduler state still
rejects before commit.

Live Allegretto system 1 executes its 28 SIDES transactions and the unchanged
Java-pinned SIG25/SIG24 removal, then exhausts SIDES. Serial page carriage
finishes all three systems through STUMPS with removal counts `[1,0,2]`; every
application removes one vertex, exactly its incident edges, and one group
member. System 1 retains external fixture SHA-256
`d4c5decf03eaab893c79b2cb7ebd0378f13ac019acc007a38718105c75eacc71`;
the two system-3 operations are wider native structural coverage, not newly
frozen Java rows. Focused Allegretto passes 1/1, the full sibling suite passes
17/17 in 147.41s, and strict Clippy, formatting, and diff checks pass.

Production now fails next at Allegretto system 1 HEADS queue 65 x77/SIG14,
LEFT/TOP, whose builder contains a start stump, chunk, and crossed x75 head with
one carried undefined side. That multi-item C-link expansion is the next
wider-corpus seam.

## Boundary 168: Allegretto multi-item existing-stem C-link

The deterministic Java runner replays the real system-1 hook-removal/STUMPS
predecessor and heads 0-64, then emits queue 65 twice byte-identically.
x77/SIG14 LEFT/TOP walks seed glyph 282, chunk 2034, and crossed x75 LEFT/TOP,
reuses StemInter 2236, adds the x77 and x75 HeadStem relations, links both LEFT
cells, and closes x75/x76 without allocator, vertex, registry, or stem insertion.
The fixture/runner/probe/body/semantic hashes are
`0bccd92c0a4305704c5903984ccf9734823bf4879b5aa6f2621595700fa6507d`,
`be1f28c0528721e23ba24e1b8107f5069310d47a1a537945052d2a536a260e74`,
`6ae5fe6eddaf4d802973c191c8d945eac8046a1d398499de79c5eb183a489092`,
`0ea1b9deaa33a644ba432a26bfe6a84391cdee5115bacaa070b71287bb1a3a13`, and
`d8a600e1dff9c81fa9ebc4eadd5fc9119548343070cdcf0225ec1dbc798b3b37`.

Rust now authenticates carried undefined sides independently from the phase-2
unlinked queue (`[x84 LEFT]` versus `[x86,x84]` here), then uses the existing
generic multi-item expansion walk to reuse native Stem 35/glyph 71 and append
the same two relation payloads. Focused 1/1 and full sibling 18/18 (143.22s)
pass. Production advances to queue 79 x82/SIG89 LEFT/TOP, whose start stump,
crossed x80 RIGHT/TOP stump, and chunk form the next fail-closed seam.

## Boundary 169: Allegretto crossed-side created-stem C-link

The deterministic order-79 replay carries the real predecessor through heads
0-78 and emits two byte-identical measurements. x82/SIG89 LEFT/TOP retains its
pre-expansion `canLink` relation, accepts crossed x80 RIGHT/TOP against the
evolving line, and then stops at the rejected trailing chunk. Active glyph 297
has no existing StemInter, so Java creates checked StemInter 2240, adds two
HeadStem edges, changes SIG 637/562 to 638/564 and system stems 39 to 40, links
x82 LEFT plus x80 RIGHT, and closes x80 LEFT/RIGHT.

Rust's generic multi-head transaction now owns the created-checked disposition,
mixed-horizontal carried undefined sides, raw start/head/chunk ordering,
bounded rejected-chunk termination, and initial-relation timing. It creates
native Stem 39 / native persistent Inter 1022 (independent of Java Inter 2240)
with exact grade, bounds, median, and
thickness and commits the same two relation and closure effects atomically.
Fixture/runner/probe/body/semantic hashes are
`63327c13e4ebba1873fb73d5507b5a34369027ca8c6a4abb60f377cebeee69ee`,
`bcbf729291881676df19a79e74a0fb4f2266d09f5c5de0565dedf4420759fd95`,
`b22c21f1b9410ec66aa5445f8aa2f9aa4e4149c02b733abe03617ec6be05c032`,
`e9802845ac23e54fb14617dc21a63ac1a5be0d5b64e998bf0b8cd0ff1a288d62`, and
`ffb9b95199d62bce49a95e044b93f09fd0562b74b8917b069e26da0d793ca452`.
Focused 1/1, full sibling 18/18 (146.26s), strict Clippy, formatting, and diff
checks pass. Production completes Allegretto system 1 and next fails closed in
system 2 at queue 89 x52/SIG43 RIGHT/TOP: start + chunk + two BeamLinkers.

## Boundary 170: generic beam-bearing head-origin C-link

The strict Allegretto system-2 replay mutates heads 0-88 without snapshots and
emits queue 89 twice byte-identically. x52/SIG43 RIGHT/TOP walks a stump-less
start, active chunk glyph 2206, RawBeam 32, and RawBeam 31. Java exits from the
last sibling BeamLinker inside `CLinker.expand`, so the initial x52 HeadStem
relation is retained while the chunk-shifted line drives both BeamStem checks
and checked-stem creation. Java creates StemInter 2386, appends three relations,
links both beam anchors and x52 RIGHT, changes SIG 654/619 to 655/622, and grows
system stems 55 to 56.

Rust now supports beam-bearing builder tails generically. Phase-1 ownership
materializes head-created B-linker anchors after SIDES/STUMPS, BeamStem
geometry uses the opposite contacted beam border, and relation maxima follow
the active Java profile. Native creates dense Stem 55 / native persistent Inter
1483 and matches the HeadStem plus both BeamStem payloads exactly.
Fixture/runner/initializer/probe/body/semantic SHA-256 values are
`dcfec65a778983cc9615786fe7b9bd008677f456ad8d6f276edb3855be46e45a`,
`f36f312b0bc82d8cbd4fc176133339515069743a5786eed54a37f76678795986`,
`9587d9c623beea6c7922dabf6b50cd4d315ed49f4bca28bcc430684362384035`,
`4e111715e281e58c51c724130dca44b6a9c0b3149188e3063f77abd3ab58280e`,
`218d8ecd1a889e0046a49594e675572cd2884bf3f8f3411a0d166b8c3b2cbb21`,
and `01868de57f3a8f5eb42a3496c62cb141d034b85f0fdf0d3859fe37b7337bccae`.
Focused gates and the full sibling suite 19/19 (144.58s) pass; strict Clippy,
formatting, and diff checks pass. Production now fails closed later at system 2
queue 111 x51/SIG36 LEFT/BOTTOM, whose builder is a start stump plus three
sibling HeadHalfLinkers. That generic multi-head expansion is next.

## Boundary 171: generic multi-head hard-tail rejection

The strict Allegretto system-2 replay mutates heads 0-110 without snapshots
and emits queue 111 twice byte-identically. x51/SIG36 LEFT/BOTTOM walks a
built start stump plus sibling x48, x49, and x50 RIGHT/BOTTOM linkers. Java
accepts all four transient HeadStem checks, but the full item span still misses
the hard tail target. `expand` therefore returns `lastIndex=-1`, `link()`
returns false before `createStem`, and active glyph 376 remains without a
StemInter. Allocator 2387, SIG 656/626, system stems 57, and relation state are
unchanged; both x51 S sides close before queue 112 x118/SIG57.

Rust now resolves both free-seed and built-head start stumps from native-owned
state and admits a head-only builder generically when its full span proves this
same hard-tail rejection. Any head-only builder that reaches the target stays
fail-closed pending the successful multi-head transaction. Production now
finishes Allegretto system 2 and stops at system 3 queue 29 x114/SIG76
RIGHT/TOP, whose start and x112 sibling do reach the target.

Fixture/runner/probe/body/semantic SHA-256 values are
`1d2dfdec360fcc575ef9b852cbb6502dc82ee6fa8b951d24914bf0ae1bb66063`,
`55020b3e312fe20cea3913f4e1b8ac849235f8e84753be66ecdc969b6f4b3365`,
`dc7df0af651b851e3d1c67d382f42b961955b4763fe5e92583e6f30a407a832d`,
`a4f9ad50ee8b7b147a02147fbea94959b54e392c6567252a7be4caf6c1a6ef71`,
and `b81c303a6863f2c88dcc93ef442bc526937e708e136e508d4c8021dbb7af4e36`.
The 7-line fixture is 6,947 bytes and pins Boundary 170's runner/fixture
`f36f312b0bc82d8cbd4fc176133339515069743a5786eed54a37f76678795986` /
`dcfec65a778983cc9615786fe7b9bd008677f456ad8d6f276edb3855be46e45a`.
Focused 1/1 and full sibling 19/19 (144.57s) pass with strict Clippy,
formatting, and diff checks. Boundary 170 commit `f87752bbb` is exact-CI green:
Build & Test 32490696521 succeeded and Rust 32490696428 passed all 12 shards.

## Boundary 172: built-stump two-head checked-stem creation

The strict Allegretto system-3 G1 replay carries the real predecessor through
heads 0-28 without snapshots and emits queue 29 twice byte-identically.
x114/SIG76 selects RIGHT/TOP and walks its built start stump plus sibling x112
RIGHT/TOP. Both resolve to active Java glyph 397; Java creates StemInter 2398,
adds the x112 and x114 HeadStem relations, closes x112 LEFT then RIGHT, and
advances to queue 30 x25/SIG4. SIG 644/567 becomes 645/569 and system stems 47
becomes 48. The exact relation grade/dx pairs are
`3fedf3a95000fdef`/`bfa960be99fb9249` and
`3fee0ca606d66e3f`/`bfa834e47b7c3cf4`.

Rust's generic multi-head transaction now resolves both seed-backed and built
head stumps from native-owned content. It creates native Stem 47 / glyph 187 /
persistent Inter 1936 with Java-exact checked grade, ribbon, median, thickness,
and relation payloads; native SIG 264/301 becomes 265/303 and system stems 47
becomes 48. The earlier x112 phase-two worklist entry remains carried, as in
Java, and will be skipped because the new HeadStem relation now exists.

The 12-line / 13,067-byte fixture SHA-256 is
`4cd7ea37b5f57b27012fc52cea377394d2d0aef97954db34dee988ed823b7549`.
Runner, initializer, transformed probe, emitted body, and semantic-pass hashes
are `a6729e51a41222156a53d772bbd64fc9c8223d14fc2eddf4769b213f09670ada`,
`c801a89d512ffc1751c178e41c6dee30a17d559bfe1b6b1822e6bc050f8b91b9`,
`d9e98b372c7baa03cdb0473162127793ef295538c9021bb7f58025d94f2d9731`,
`9b339591efe421f2a73c3c10eee7e8f092bf66f5eae506e0480a2b462e3bf5c9`, and
`b834f6c87d003428b73242a1081835096d9a63c4c36e1af53dc248ed8dad964a`.
Focused 1/1 and full sibling 20/20 (147.78s) pass. Production now fails closed
at system 3 queue 61 x57/SIG99 RIGHT/TOP: a stump-less start, two chunks from
builder 228, and RawBeam 76. Boundary 172 commit `7e87b6c07` is exact-CI green:
Build & Test 32499929575 succeeded and Rust 32499929648 passed all 12 shards.

## Boundary 173: multi-chunk beam-bearing checked-stem creation

The strict Allegretto system-3 G1 replay mutates heads 0-60 without snapshots
and emits queue 61 twice byte-identically. x57/SIG99 selects RIGHT/TOP after
the same transaction first considers LEFT/BOTTOM. The selected builder carries
two active chunk glyphs (Java 410 and 2000) plus RawBeam 76. Java composes a
new 1335:1857:4:92 glyph, creates StemInter 2402, adds one HeadStem and one
BeamStem relation, links beam linker 4, and advances to queue 62 x54/SIG97.
SIG 647/579 becomes 648/581 and system stems 50 becomes 51.

The generic native C-link path now carries every selected chunk in order,
composes their exact run-table union, and asks the native modeled-glyph
registry for an exhaustive content scan before proving a compound absent. It
registers native glyph 1939, creates checked Stem identity 50 / persistent
Inter 1940, and appends HeadStem edge 310 plus BeamStem edge 311. Native SIG
267/310 becomes 268/312, system stems 50 becomes 51, and the carrier advances
to the same next head. Java and Rust intentionally use different local IDs;
the candidate content, checked grade, median, thickness, relation payloads,
linker writes, and continuation are exact.

The 12-line / 12,990-byte fixture SHA-256 is
`de80142ffc78b6dd96b156285c365b1997bdbb7228ae47093f1b244dea04b56e`.
Runner, transformed probe, emitted body, and semantic-pass hashes are
`27d26355c3b58d788d96ddb3d40b3aed4c17fc7c65a0af5c477205df21690f15`,
`3318d3d122240b9e10dee6573ac3fd3c95b99c640ff229405975771ef63c4666`,
`0a8aab562930ad983c0e91fe011a8094c7f039870d10385cc64c9fd74f84a9b9`, and
`462489439a3152a10a9dc65a002845c72acb3672bcf5f81967b34d6bdbc233ff`.
Strict predecessor runner/fixture pins are Boundary 172
`a6729e51a41222156a53d772bbd64fc9c8223d14fc2eddf4769b213f09670ada` /
`4cd7ea37b5f57b27012fc52cea377394d2d0aef97954db34dee988ed823b7549`.

Focused 1/1 and full sibling 20/20 (148.43s) pass with strict all-features
workspace Clippy, formatting, and diff checks. Production crosses queue 61 and
now fails closed at system 3 queue 115 x113/SIG75 RIGHT/TOP. Builder 452 joins
the start head to sibling x108/SIG67 and reaches Java's hard tail target; that
was the apparent next expansion. Boundary 174 below supersedes the diagnosis by
carrying queue 53's missing link. The remote baseline at this boundary was
`7e87b6c07`.

## Boundary 174: generic two-side carriage and corrected no-link frontier

Boundary 173's apparent queue-115 expansion was caused by missing predecessor
state. Java queue 53 x107/SIG80 continues from a successful LEFT C-link into
RIGHT/TOP. LEFT reuses Stem 2394; RIGHT reuses active glyph 397 / Stem 2398 and
plans x107, x116, x117, and x108 HeadStem relations. x117's edge already
exists, so the full call appends the other three plus x107's LEFT edge, links
both x107 sides, propagates the shared RIGHT link to x108/x116/x117, and closes
their related sibling cells.

The generic native dispatcher now evaluates both horizontal sides on one
atomic shadow, retains each successful side transaction, validates
same-content crossed-head stumps, and distinguishes appended relations from
pre-existing edges. It also represents Java's mutated-then-unlinked case: a
first side mutation survives if a later side sees the same stump at TOP and
BOTTOM, records an undefined side, and returns `false`. The production event
model retains the mutation and phase-2 queue entry; Allegretto system 2 queue
103 x85/SIG86 pins this path while the downstream queue-111 oracle remains
exact. Weak heads with no linkable corner now close locally and enter phase 2;
the higher-profile rather-good retry remains fail-closed.

With queue-53 state carried, x108 RIGHT is linked and closed before queue 115.
x113/SIG75 then chooses `Neither` on LEFT and RIGHT, returns `false`, closes its
two local S cells, mutates no SIG relation or system stem, and advances to queue
116 x66/SIG33. The 17-line / 17,020-byte fixture is deterministic across warmup
plus two fresh runs. Fixture/runner/probe/body/semantic SHA-256 values are
`01bda66e6eecf7d46bdd21f3d2d4d8ec977deff9bc51f01b4a3291092680fca2`,
`b3c426db85a5c5402c7e8d5741e249c15905e0f2d8f4888d491ee9783982afa4`,
`4e42bfb4de50ec8a3d14c8c028b435d115f1ec55b9efe59e249120ae5887db12`,
`27bf04be971bb5705170e00646a4440fe3107fd679b4b55bd6be6ca27b0782a4`,
and `fd1a3ca321041ede2ab5d39ffb2742675b19138b5b5082a93f44dbcfed7a6185`.
Strict Boundary-173 runner/fixture pins are
`27d26355c3b58d788d96ddb3d40b3aed4c17fc7c65a0af5c477205df21690f15` /
`de80142ffc78b6dd96b156285c365b1997bdbb7228ae47093f1b244dea04b56e`.

Focused 1/1 and full sibling 20/20 (148.29s) pass with strict all-features
workspace Clippy, formatting, and diff checks. Boundary 174 stops before queue
116. The exact remote baseline is `02f09e64b`: Build & Test 32513292289 and
Rust port 32513292385 both succeeded. Wider-corpus completion remains open.

## Boundary 175: Allegretto system-3 queue-116 prelinked closure

The unchanged generic phase-1 continuation consumes queue 116
x66/SIG33/Inter1743. LEFT is already linked and closed through incident
Stem2380, RIGHT is already closed, and the stem also carries x67/SIG34. Java
returns `true` and closes x67 LEFT then RIGHT, exactly two value changes. SIG
vertices/edges remain 649/593, system stems remain 52, relation state is
unchanged, neither phase-2 worklist changes, and the carrier advances to queue
117 x86/SIG18/Inter1711 with LEFT linked/closed and RIGHT closed. No production
source seam was required.

The 13-line / 16,627-byte minimized fixture is deterministic across warmup plus
two fresh runs. Fixture/runner/probe/body/semantic SHA-256 values are
`cc6b2240cc6f6fa13fa294ef17eb01cae65afc8189fba4e4a244d99d76891a8e`,
`2e2c10929798d25ea10ec0b5912288db59e5feb71f806c784fd60b445fbe89f3`,
`c0aa6ac09a1d1178134e9b0b65ad0b7166a5c77e3e2ed0f85f574b2ffecb81e3`,
`1e7e336ad5b0c7f7315ec97bfa9807c8e04d57233c29b3b4f0014fd1422e68c9`,
and `94d9b566379c926f214a9e37672e1d97a0f5287d2252a48a1d787f7373584564`.
Strict Boundary-174 runner/fixture pins are
`b3c426db85a5c5402c7e8d5741e249c15905e0f2d8f4888d491ee9783982afa4` /
`01bda66e6eecf7d46bdd21f3d2d4d8ec977deff9bc51f01b4a3291092680fca2`.

Focused 1/1 and full sibling 20/20 (151.16s) pass with formatting, strict
all-features workspace Clippy, and diff checks. The exact remote baseline is
Boundary 174 commit `02f09e64b`: Build & Test 32513292289 and Rust port
32513292385 both succeeded.

## Boundary 176: Allegretto system-3 final phase-1 no-op closure

The unchanged generic continuation consumes the last head, queue 117
x86/SIG18/Inter1711. LEFT is linked and closed through Stem2368 and RIGHT is
closed. The same stem carries x84/SIG27 and x85/SIG28, whose four S cells are
already closed; Java returns `true`, emits the four ordered `true->true`
closure writes, and changes zero values. SIG 649/593, system stems 52,
relation/linker hashes, undefined sides, and the phase-2 worklist are
unchanged. Native reaches `current_index=118 == heads.len()` with phase-2 index
zero and the exact retry queue x112/SIG68, x0/SIG19, x14/SIG50, x13/SIG0,
x56/SIG100, x113/SIG75.

The 13-line / 16,544-byte minimized fixture is byte-identical across warmup
plus two fresh runs. Fixture/runner/probe/body/semantic SHA-256 values are
`dbe00a31bf256a2a8c071b755e3c3df4e95e3ecce45f9d7020729ae0705e9caf`,
`088128d72a928ac4a16439e1fa61c857901b793ccbc20e79231c0070e7e50086`,
`f17ce2eead270d2cc2d4390218440f408544b345806d8d683a29451cc90b7c2d`,
`567b8ebb998d7d75e46380c7740e7259454936be517771816aaca4e7369d0478`,
and `69eaf824e4c50b706f2c22c446e465afa966d957a04b2d389ce9a2cad0ba70ad`.
Strict Boundary-175 runner/fixture pins are
`2e2c10929798d25ea10ec0b5912288db59e5feb71f806c784fd60b445fbe89f3` /
`cc6b2240cc6f6fa13fa294ef17eb01cae65afc8189fba4e4a244d99d76891a8e`.

Focused 1/1 and full sibling 20/20 (154.71s) pass with formatting, strict
all-features workspace Clippy, and diff checks. Boundary 175 commit
`ef4ee3e00` is the exact remote baseline: Build & Test 32516450490 and Rust
port 32516450484 both succeeded, with all 12 Rust shards green. Next is phase-2
retry index 0, x112/SIG68.

## Boundary 177: Allegretto full-page x0 early-stop correction

The full foreground-page lifecycle shows that x0/SIG19 is not a phase-2
retry. At phase-1 order 100 its RIGHT/BOTTOM corner accepts the
369:1595:2:48, weight-63 start stump. Java then rejects the next plain chunk
because its centroid exceeds `maxLineGlyphDx = 0.2 * interline` from the
evolving stem line and immediately returns `lastIndex=0` of `maxIndex=1`,
before the final hard-tail/relation recheck. Native now carries accepted C-link
content and line translation incrementally and authenticates that early stop
at this exact system-3 x0 frontier. The x14 and x13 hard-tail failures are
unchanged.

The checked stem matches Java StemInter3170: grade bits
`3fe49d64653090d5`, bounds 368:1595:3:48, median bits
40771723de22d21c:4098ec0000000000:40771f7fd38ffa01:4099ac0000000000,
and width bits `3ff5000000000000`. The Java vertex/edge/system-stem/allocator
counts each advance by one, and native pins the equivalent atomic deltas.
Phase 1 still ends at index 118; the exact retry queue is now five heads:
x112/SIG68, x14/SIG50, x13/SIG0, x56/SIG100, x113/SIG75.

The deterministic full-page oracle is 33 lines / 16,196 bytes and includes
the x0 audit, three baselines, all 25 Java phase-2 retries, and a strict
summary. Fixture/runner/probe/body/semantic SHA-256 values are
`242260a9fe7b873ca8597840ea7253d45d6518742e924496ccc4a14bb2a8c41c`,
`9196aa6841aba9d234c4a82d21185c4ed1367b0329fcfca9930c14f0c6a15331`,
`e2255ffc6ff5c4b73d01afba083fba07cff682f5e4148c36a921d3184c9c952b`,
`d96572e2ca0ca46e55a3a2997a5bc6dc7d1977214068571ac0497b62f94c936b`,
and `d96572e2ca0ca46e55a3a2997a5bc6dc7d1977214068571ac0497b62f94c936b`.
Strict Boundary-176 runner/fixture pins are
`088128d72a928ac4a16439e1fa61c857901b793ccbc20e79231c0070e7e50086` /
`dbe00a31bf256a2a8c071b755e3c3df4e95e3ecce45f9d7020729ae0705e9caf`.
Warmup plus two fresh Java passes are byte-identical.

Focused 1/1 and full sibling 20/20 (157.51s) pass with formatting, strict
all-features workspace Clippy, and diff checks. Boundary 176 commit
`8185667b7` is the exact remote baseline: Build & Test 32519244924 and Rust
port 32519244803 both succeeded, all 12 Rust shards green. Next is phase-2
retry index 0, x112/SIG68.

## Boundary 178: Allegretto system-3 phase-2 retry zero

The generic `append=true` continuation consumes x112/SIG68/Inter1812 without
new production code. It re-evaluates the closed/unlinked LEFT side, finds both
corners unlinkable, then returns `true` on the already linked/closed RIGHT.
Native records ten ordered idempotent shared-stem closures: LEFT then RIGHT for
x114/SIG76, x117/SIG72, x107/SIG80, x116/SIG71, and x108/SIG67. All cells
were already closed, matching Java's empty `sideChanges` and zero changed
values. SIG 267/317, system stems 52, allocator 3170, undefined RIGHT, and the
five-head worklist stay unchanged; `phase_two_index` alone advances to one.

The gate reuses Boundary 177's fixture/runner
`242260a9fe7b873ca8597840ea7253d45d6518742e924496ccc4a14bb2a8c41c` /
`9196aa6841aba9d234c4a82d21185c4ed1367b0329fcfca9930c14f0c6a15331`
and pins the exact Java retry row, including grade bits
`3fe8d8c228e9b518`, Neither/SkipAlreadyLinked decisions, unchanged state, and
preserved undefined RIGHT. Focused 1/1 and full sibling 20/20 (161.95s) pass
with formatting, strict all-features workspace Clippy, and diff checks. The
exact remote baseline is Boundary 178 `e99e93a92`: Build & Test 32528147579
and Rust port 32528147610 both succeeded, all 12 Rust shards green. Next is
retry index 1, x14/SIG50.

## Boundary 179: Allegretto system-3 phase-2 x14 append

The bounded phase-2 transaction now executes the first mutating retry. At
x14/SIG50/Java Inter 1777, LEFT/TOP is linkable but its expansion returns
`-1`; RIGHT/BOTTOM succeeds. The generic C-link parser accepts the measured
start-head, crossed-head, then chunk order, selects native glyph 204 (Java
glyph 414, bounds 550:1581:3:88, weight 194), and reuses existing Stem 3148
(native identity 30 / vertex 247). x15's relation already exists as edge 256;
only x14's missing HeadStem edge 327 is appended. SIG edges advance 317 to
318 while 267 vertices, 52 system stems, allocator 3170, and glyph identities
remain unchanged.

The reused stem's grade and geometry are exact. x14's relation grade/dx/
extension/consistency bits are `3fed98996cac8bf2`, `3f9c4c548b8fedb7`,
`408134a485dee59d:4098840000000000`, and `3ff7f2116a3b35fd`.
Closure visits x15, x18, and x19 LEFT then RIGHT without a value change.
Native restores the exhausted phase-1 cursor, advances `phase_two_index` to
two, and stops before x13/SIG0/Java Inter 1675 (grade bits
`3fc5aea35e22900d`).

The dedicated 6-line / 3,825-byte minimized Java oracle is byte-identical
across warmup plus two fresh passes. Fixture/runner/transform/init/body/input/
base-probe/source/transformed-source SHA-256 values are
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
and diff checks. The exact remote predecessor is Boundary 178 `e99e93a92`:
Build & Test 32528147579 and Rust port 32528147610 both succeeded, all 12 Rust
shards green. Next is retry index 2, x13/SIG0.

## Boundary 180: Allegretto system-3 phase-2 x13 append

The second mutating retry reuses the same authenticated C-link operation at
x13/SIG0/Java Inter 1675. LEFT/TOP expansion fails and RIGHT/BOTTOM succeeds;
native glyph 204 resolves to existing Stem 3148 (identity 30 / vertex 247).
x15 edge 256 remains present and only x13 HeadStem edge 328 is appended, so
SIG edges advance 318 to 319 while vertices 267, stems 52, allocator 3170,
and glyph identity remain unchanged.

The relation grade/dx/extension/consistency bits are
`3fed98996cac8bf2`, `3f9c4c548b8fedb7`,
`408134a485dee59d:4098840000000000`, and `3ff7f2116a3b35fd`.
Closure visits x15, x18, x19, and x14 LEFT then RIGHT idempotently.
`phase_two_index` reaches three before x56/SIG100/Java Inter 1876, grade bits
`3fc5165a40f2ed07`.

The dedicated 6-line / 3,813-byte minimized Java oracle is byte-identical
across warmup plus two fresh runs. Fixture/runner/transform/init/body/probe/
source/transformed-source hashes are
`4ebbaa69132cdee430d38b9b27622ae1e64e0d12554ead8e6a782ab8dcdbde3f`,
`1bdfd26b350170a8f4d17290ea6f336f544b6ee8ee9dc1566bcf00654cd59ac2`,
`42dbccb9b9f05178358c54488aec0d8ae3339aca6083b25b1f73aff069c59a10`,
`c4a870d654f1a60c4fe8be37f63806b676858d659fc220c08d4432f70c6253e9`,
`33c4f489a66eefbb11034857f0d2cb991d47fb7582b943358da25817a1e2d60c`,
`7b467c57b65e57aa052296164129ae8c016d82756c9f804d8e1072747b0a76b2`,
`f51893627e9e1ddaca77daba9166098cfa6d8cc99ff8d094aa9138c13ad78993`,
and `b2106f6b3e20eeedb46bf0e6926dc6b760581edcb6d65fd381401596c65c71ad`.
Boundary 179's direct runner/fixture pins are
`5f530a9fca946f6ed74877713452b7a64fd66f98810654113a700cd6ee61ced3` /
`f8a18f4ac17d036e0f3481983474d3569668437c6d53670b7f454f707baad1ba`.

Focused 1/1, full sibling 20/20 (146.77s), and the canonical workspace suite
pass with formatting, strict all-features workspace Clippy, and diff checks.
Boundary 179
`5fd12958bf65fca9aa78896924ace95b05ec7def` is the exact remote baseline:
Build & Test 32536290867 and Rust port 32536290886 both succeeded, all 12 Rust
shards green. Next is retry index 3, x56/SIG100.

## Boundary 181: Allegretto system-3 phase-2 x56 no-link

The existing generic append continuation consumes x56/SIG100/Java Inter
1876 at retry index 3. Both sides are closed/unlinked; LEFT is TopOnly and
RIGHT is BottomOnly, but both selected expansions return `-1`. Java/native
return `false`, idempotently revisit x56 LEFT then RIGHT, and advance the
phase-two cursor to four without changing SIG 267/319, stems 52, allocator
3170, glyph identities, or undefined sides.

The strict gate uses the existing full-page fixture/runner
`242260a9fe7b873ca8597840ea7253d45d6518742e924496ccc4a14bb2a8c41c` /
`9196aa6841aba9d234c4a82d21185c4ed1367b0329fcfca9930c14f0c6a15331`
and pins the exact Java row: grade `3fc5165a40f2ed07`, TopOnly/BottomOnly,
`returned=false`, no side changes, and unchanged graph/allocator counts.
Focused 1/1 (3.72s), full sibling 20/20 (150.19s), formatting, strict
all-features workspace Clippy, and diff checks pass.

Boundary 179 `5fd12958bf65fca9aa78896924ace95b05ec7def` remains the exact fully green
remote baseline (Build 32536290867; Rust 32536290886, 12/12). Boundary 180
`9dcdb0c179d0af044a79fb4419119f770f5f6ef9` is pushed; Build 32542247629 is
green while Rust 32542247645 was superseded and cancelled. Boundary 181
`4c06c26bf17875c0c16a1f63174b02822dfda0cb` is pushed; Build 32542733505 is
green while Rust 32542733478 remains queued. Next is the final retry, index 4
x113/SIG75.

## Boundary 182: Allegretto system-3 final phase-2 x113 append

The bounded x113 continuation consumes retry index 4 at
x113/SIG75/Java Inter 1826. LEFT is `Neither` and RIGHT is `TopOnly`; the
RIGHT/TOP C-link reuses native glyph 187 (Java glyph 397) and the queue-29
checked Stem, native identity 47 / vertex 264 / Java Inter 3165. Crossed
x108/SIG67 edge 310 remains live and only x113 edge 329 is appended. Native
edges advance 319 to 320 while vertices 267, stems 52, allocator 3170, and
glyph identity remain unchanged.

The new relation grade/dx/extension/consistency bits are
`3fea63f9c75cf906`, `3fb0115caff3c30c`,
`40a12ea2d934ddfe:409dfc0000000000`, and `3ffd1d9afe422d47`.
Closure visits x114, x112, x117, x107, x116, and x108 LEFT then RIGHT; all
twelve writes are idempotent. `phase_two_index` reaches five, exactly the
five-entry queue length.

The 6-line / 3,807-byte minimized Java oracle is byte-identical across warmup
plus two fresh runs. Fixture/runner/transform/init/body-semantic hashes are
`83e4c5671e6e1d489c84d30ff0bd5e01c3b095c68b8562d2f09c42908b49f1af`,
`4f589fb9512f2b7d6467b98c9174b81ec91783a002455ee4c7ae908c1e4aa854`,
`f143d4f4d49d4fc67cb4ebd883768dfc7a7a11fd9cc918d784cc50a41c8ee00f`,
`302235acd663a6ebfeda7bceeaab336e77a990baa152012740aa41925af8b09f`,
and `c1b20ce77aa8cbb727e45dd2a078ef663bd1e59f82b871b26acd26cd417db385`.
The direct Boundary-180 x13 runner/fixture pins are
`1bdfd26b350170a8f4d17290ea6f336f544b6ee8ee9dc1566bcf00654cd59ac2` /
`4ebbaa69132cdee430d38b9b27622ae1e64e0d12554ead8e6a782ab8dcdbde3f`.
Focused 1/1 (3.68s), full sibling 20/20 (148.18s), formatting, strict
all-features workspace Clippy, and diff checks pass.

## Boundary 183: Allegretto system-3 generic `finalizeStems`

The unchanged generic `finalize_native_stems` consumes the exact exhausted
Boundary-182 carrier. It checks 118 heads, identifies x107/SIG80 as the sole
multi-stem head, and identifies x56/SIG100 as the sole stemless and abnormal
head. x112/SIG68 RIGHT remains carried as undefined. Neither Java nor native
removes a HeadStem relation or changes an abnormal flag; SIG 267/320, stems 52,
allocator 3170, and the full carrier remain unchanged.

The full-page three-system Java oracle is byte-identical across warmup plus two
fresh runs. Fixture/runner/probe/init/body hashes are
`cfb9e6011ed29aa30e6e90db6eeae931a3a6533d7339d80519a5ddd650c0ff0c`,
`abafa7d183ae151baa7ed4d8005257c562e0c49fb939fe931a7571994d70d890`,
`9b5e9dbefbf400887f49feba934c573d851c67e65b3e43bfaabc86d6f2c36714`,
`e0ff89792bf75286317ef011e079f338696d29cc14918f4a3018307ba4ed9548`, and
`3add75f32b08d8836817483175425872814f10aa18c0c14bef86e3306dddc8f1`.
The direct Boundary-182 runner/fixture pins are
`4f589fb9512f2b7d6467b98c9174b81ec91783a002455ee4c7ae908c1e4aa854` /
`83e4c5671e6e1d489c84d30ff0bd5e01c3b095c68b8562d2f09c42908b49f1af`.
Focused 1/1 (3.86s), sibling 20/20 (153.23s), formatting, strict
all-target/all-feature workspace Clippy, shell syntax, and diff checks pass.
Boundary 184 below begins wider-corpus generic STEMS completion.

## Boundary 184: Zizi system-1 duplicate-idempotent closure

Zizi system 1 head order 34, x26/SIG106, links LEFT/BOTTOM and RIGHT/TOP
through two distinct stems that both reach x28. Java applies both C-links
before its one closure loop, producing x28 LEFT/RIGHT false-to-true and then
the same two writes true-to-true. The generic native driver now defers each
inner atomic transaction's closed flags/evidence until both sides have linked,
then records the exact four writes and two value changes. SIG vertices 238,
stems 44, and the native allocator are unchanged; edges advance 242 to 244 and
the queue reaches x68/SIG102 at index 35.

The warmup-plus-two Java fixture is byte-identical. Fixture/runner/transform/
init/probe/body hashes are `0970b0dafe3a456d30e72b55a2716205e06caa4a93367e9390f00263139117f6`,
`de07f1e244641a2f9f41379b871595201b5158428e28d0f1701927b7221b7f90`,
`db0196bc8088e45ee550e7cc595f799bdcda079ce595c1bbf70c5994d06965ca`,
`55836b16d632f805b78427fb2c969becffb8f2c97df1c361d47be673fe169ca2`,
`f14692de5a59a0153ed58ded0cf18d5f736e57e327f3cf7fa5e26b9cfe0e3d4e`,
and `670de47539abe7f140f66fe77e812bb53ddc42982fb5a95a712ec56c71d88313`.
Focused 1/1, sibling 21/21, strict Clippy, formatting, shell syntax, and diff
checks pass. `f4629fa1d` is the exact green remote baseline (Build 32545226391;
Rust 32545226371). The next fail-closed frontier is Zizi system 2 queue 107,
x89/SIG64 RIGHT/TOP, builder 356 profile 1/1 with start-half, filament-0 chunk,
and x94/SIG61 LEFT/TOP target-half items; x90/SIG55 LEFT is already undefined.

## Boundary 185: Zizi system-2 crossed-head stump expansion

The generic head C-link operation now walks Java's ordered builder items. A
crossed head relation is checked before its reachable stump changes the stem
line, and an accepted crossed head survives a later plain-chunk rejection.
The same operation appends every accepted crossed HeadStem relation for both
reused and newly created stems. Native also initializes Java's hard-tail
`lastY` from the theoretical line's original P1 before reversing the working
line for upward expansion; that keeps the older Allegretto and Batuque gates
exact without a page-specific exception.

At Zizi system 2 order 23, x94/SIG61 LEFT/BOTTOM accepts crossed x89/SIG64
RIGHT/BOTTOM, selects active glyph 245, rejects the following chunk, and
creates StemInter 1724 with both HeadStem edges. SIG 444/384 becomes 445/386,
system stems 45 becomes 46, and closure writes x89 then x93 LEFT/RIGHT. The
following continuation reaches x86/SIG94. This prelinks the old queue-107 x89
frontier, so transactional `-step STEMS -json` now completes the full Zizi
page.

The strict nine-row fixture is byte-identical across warmup plus two fresh
runs. Runner/init/fixture/probe/overlay/body/semantic hashes are
`33f2ce87e7c727156de4250410052b95dbd209590419c15bb2428be3edec8b9b`,
`46241f0adbc0ef8746240567b2b54d09ffad062962e07f4deee9c745e6b43d97`,
`fb9797eb2039cf3f052f7bd7285a94b737a8771075406f772261deded352be9d`,
`b4375a1d44e7e513a0946520ca146fc84de6dcf8b9c3297c1cb8def09bdb6c5d`,
`f21487398d9ba162b6459f8f5e1265d56ffc6a8a58e6aa514a03553ee3d05df4`,
`5a9c6ad49ca15fb61a765a4334a0cf40868645d8810801dc2f18655829f90954`,
and `d5ad96dee3d46dedcb150d263c9f350cf2353c09cfc5134ef45456b1803f2a43`.
Focused Zizi, preserved Allegretto/Batuque, sibling 22/22, production Zizi,
formatting, strict Clippy, shell syntax, and diff checks pass. `4de83dc30` is
the exact green predecessor (Build 32547802513; Rust 32547802498). Carmen
system 1's dual-corner selection branch is next.

## Boundary 186: Carmen system-1 shared-stump dual corners

The initial head-phase transfer now resolves the live TOP/BOTTOM reachability
stumps whenever both corners can link. Equal non-null stumps queue the
horizontal side and head for phase 2 without choosing a C-link or mutating
graph/stem/cell state; different or missing stumps use Java's ordinary LEFT →
BOTTOM / RIGHT → TOP selection. Carmen system 1 consumes all 45 heads: x39/SIG3
LEFT and x38/SIG2 LEFT become the exact two-entry retry queue while native stays
at 161 vertices, 172 edges, and 18 stems. Java stays at 163/175/18 and retains
both abnormal no-stem heads through finalization.

The warmup-plus-two fixture is byte-identical. Runner/fixture/body hashes are
`070c3febcf34348fc8ce643c17d99757a7845daf4f1379e591a7922b1a0da1b9`,
`28018b4010fc1a08a45569298b06f737164c86398a2e46f277bceb869fedf089`,
and `27c8e7343d2beff061e04cf1f1e9efb18078afee943923aa14ada60a88dc22aa`.
Input/StemsRetriever/probe/init hashes are
`249330d6558d410f64f550180d3a659dd3c9c340dcdcb5ae08e809c273fe2e44`,
`26e95fa09905b39ea0dcae2b65a85b4e4fcb49b772c57f97f332a00c4dc8b9e7`,
`9b5e9dbefbf400887f49feba934c573d851c67e65b3e43bfaabc86d6f2c36714`,
and `e0ff89792bf75286317ef011e079f338696d29cc14918f4a3018307ba4ed9548`;
the Boundary-185 runner/fixture pins are
`33f2ce87e7c727156de4250410052b95dbd209590419c15bb2428be3edec8b9b` /
`fb9797eb2039cf3f052f7bd7285a94b737a8771075406f772261deded352be9d`.
Focused 1/1, sibling 23/23, formatting, strict Clippy, shell syntax, and diff
checks pass. Production Carmen now reaches system 2 queue 70 x13/SIG10
RIGHT/BOTTOM, whose builder is start stump → Gap → chunk; that generic
Gap-aware expansion is next. `425d58e82` is the exact green predecessor (Build
32551514978; Rust 32551514933, 12/12).

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

Java's append-mode `reuseStem(lastIndex)` can choose a different stem from the
one attached to the selected candidate glyph. The generic native transaction
now scans the current C-linker and preceding builder items in Java order,
retains candidate-stem provenance, records the independently selected append
reuse, and targets the new HeadStem relation at that result.

For Carmen system 3 queue 3, x0/SIG3 selects Java glyph 531 (native glyph 218)
and the short candidate Stem 3984 / native identity 41. The ordered scan crosses
x3/SIG13 and instead reuses long Stem 3949 / native identity 6 / vertex 242,
adding edge 324 from native head vertex 133. Vertices stay 279, edges rise
324→325, 43 stems and the allocator stay unchanged, and x3/x6/x7/x1 close
LEFT then RIGHT in Java order with zero value changes. Phase two advances 3→4.

The generic retry driver then exhausts all five Carmen phase-two queues at
2/2, 9/9, 11/11, 5/5, and 3/3. Generic `finalizeStems` checks
45/83/106/93/102 heads with no removed relations or abnormal changes, and the
transactional entry point reproduces the same prepared and finalized page.
Carmen STEMS is therefore transactionally complete across all five systems.

The 6-line / 3,680-byte fixture is byte-identical across warmup plus two fresh
runs. Runner/transform/fixture/body+semantic SHA-256 values are
`667310b7936cc9341aac3e145d19328f43e7777e85fef6cb0480dbe2e4c86c4b`,
`29f9b38aba7393883d1b7ff5aff6035e7fc1d0397d001ed5ded0fe8c64d29774`,
`448af58ab47cbfea66a8cee14f95fb376ebd668692e36afd242e7af4f5cbaad8`,
and `a3d2e45a4f4fce8f4d98047fb1ac914b36c94215cb6180eda35b9f8462a6372f`.
The predecessor runner/fixture remain pinned at `e0bf5408…` / `f9656d9b…`.
Focused 1/1, full sibling 25/25 (151.07s), formatting, strict workspace
Clippy, oracle syntax, and diff checks pass. The next work is the first
fail-closed frontier among Cucaracha, Hove, and BachInvention5.

## Boundary 192: Cucaracha rejected-stem no-link

Java returns false when C-link expansion has a glyph and acceptable HeadStem
relation but `StemBuilder.createStem` returns null. The generic native C-link
loop now maps only a mutation-free `Rejected` result to that existing no-link
path; rejected registration or reinsertion remains fail-closed.

Cucaracha system 2 order 56 is x56/SIG78/Inter1388. RIGHT/BOTTOM selects the
active 1×15 glyph1838 and a grade-1.0 HeadStem relation, but its stem checker
grade is zero. Java adds no graph/stem/glyph object, closes both current S
sides, returns false, and advances to x132/SIG84/Inter1400. Native repeats the
decision over its allowed profiles, changes no SIG/registry/allocator/stem
state, records the same ordered side closures, and advances identically. All
three Cucaracha phase-1 queues now exhaust; the page next fails closed at
system 1 phase-2 queue 6 x25/SIG71's real append.

The 7-line / 5,294-byte fixture is byte-identical across warmup plus two fresh
runs. Runner/init/fixture/body/semantic SHA-256 values are `08eb22aa…`,
`4a664956…`, `51d9d826…`, `34cf5cfb…`, and `9c95af3a…`; the generated probe,
retained-glyph overlay, and ordered predecessor-fixture set are pinned at
`1fa259fd…`, `f2148739…`, and `e365077c…`. Focused 1/1, full sibling 26/26
(150.13s), formatting, strict workspace Clippy, oracle syntax, and diff checks
pass. The remote green baseline remains `425d58e82` pending newer terminal CI.

## Boundary 193: Cucaracha phase-two LEFT reused-stem append

The shared phase-two seam now authenticates either horizontal side and an
ordered set of pre-existing crossed-head relations. Java queue index 6 is
x12/SIG69/Inter1083: LEFT/BOTTOM reuses glyph199/Stem2210, preserves
Inter1173's relation, adds one LEFT HeadStem edge, and keeps vertices, system
stems, and allocator unchanged. Native queue index 6 is x25/SIG71: glyph43
resolves to Stem identity31/vertex225, ordered x22/SIG90 and x32/SIG115
relations remain untouched, and one LEFT edge is appended with no other
mutation. RIGHT/BOTTOM then expands to `-1`; the carrier advances to queue 7
x12/SIG69. The distinct Java/native ordinals remain explicit rather than
being presented as wider HEADS identity parity.

The 10-line / 5,719-byte fixture is byte-identical across warmup plus two
fresh runs. Runner/transform/fixture/body+semantic hashes are `0f47ae8f…`,
`69955a68…`, `b8f37f27…`, and `ec9f2744…`; Boundary 192's runner/fixture are
pinned at `08eb22aa…` / `51d9d826…`. Focused 1/1, full sibling 26/26
(152.74s), final Java replay, formatting, strict workspace Clippy, oracle
syntax, and diff checks pass. Continue at phase-two queue 7 x12/SIG69.

## Boundary 194: Cucaracha consecutive LEFT shared-stem append

Cucaracha system 1 phase-two queue 7 now executes through the generic
LEFT-origin shared-stem seam. Native x12/SIG69 selects LEFT/BOTTOM, resolves
glyph41 to Stem identity32/vertex226, preserves the existing x18/SIG113 LEFT
edge278, and appends one new edge without changing vertices, stems, glyph IDs,
or allocator state. The carrier advances to queue 8 x52/SIG75.

Java queue index7 independently measures x52/SIG75/Inter1095: glyph202
resolves to Stem2205, Inter1185's LEFT relation remains, and only edges
338→339 change. Java/native x/SIG ordinals remain explicitly distinct; this
is queue-position/control/mutation parity.

The 10-line / 5,921-byte fixture is byte-identical across warmup plus two
fresh runs. Runner/transform/fixture/body+semantic hashes are `a816aec9…`,
`009d2479…`, `8c6871cd…`, and `ec71cbcb…`; Boundary 193's runner/fixture/
transform are pinned at `0f47ae8f…` / `b8f37f27…` / `69955a68…`. Focused
1/1, full sibling 26/26 (151.67s), formatting, strict workspace Clippy,
deterministic Java replay, oracle syntax, and diff checks pass. Continue at
phase-two queue 8 x52/SIG75.

## Boundary 195: shifted x52 append and prelinked no-op

Native queue 8 is x52/SIG75 because the wider Rust HEADS carrier has one
additional earlier entry. LEFT/BOTTOM resolves glyph44 to Stem identity27 /
vertex221, preserves x59/SIG119 edge264, and appends one edge with no other
graph, stem, glyph-ID, or allocator mutation. Boundary 194's Java queue-7
fixture is the identity-matched C-link authority for this head.

Native queue 9 x119/SIG110 then uses the generic prelinked path. It skips the
already linked/closed LEFT side, finds neither RIGHT corner linkable, and
returns true. Seven neighboring heads are traversed LEFT then RIGHT; the 14
ordered writes all target already-true flags, so value changes and graph
changes remain zero. Java queue index8 independently confirms the same x119
prelinked no-op. Production reaches queue 10 x42/SIG73.

The 5-line / 2,887-byte fixture is byte-identical across warmup plus two
fresh runs. Runner/transform/fixture/body+semantic hashes are `e1fcae89…`,
`5722bbdc…`, `475c4346…`, and `a7aff1b2…`; Boundary 194's runner/fixture/
transform are pinned at `a816aec9…` / `8c6871cd…` / `009d2479…`. Focused
1/1, full sibling 26/26 (152.74s), formatting, strict workspace Clippy,
deterministic Java replay, oracle syntax, and diff checks pass. Continue at
phase-two queue 10 x42/SIG73.

## Boundary 196: identity-aligned x42 append and six prelinked returns

Native queue 10 x42/SIG73 selects LEFT/BOTTOM, resolves glyph42 to Stem
identity23/vertex217, preserves x39/SIG91 edge257 plus x49/SIG117 edge258,
and appends one edge without other graph or registry mutation. Queues 11-16
(x133, x58, x125, x138, x48, x17) then use the generic prelinked path and
return true without graph or closure-value changes. Production reaches queue
17 x68/SIG76.

Java queue9 independently measures the identity-aligned x42 C-link through
glyph200/Stem2201 and one edge addition; queues10-15 confirm the six no-op
returns. The 16-line / 9,639-byte fixture is byte-identical across warmup plus
two fresh runs. Runner/transform/fixture/body+semantic hashes are
`ff8c906f…`, `aa8a4c50…`, `614570ef…`, and `f88e42d5…`; Boundary 195's pins
are `e1fcae89…` / `475c4346…` / `5722bbdc…`. Focused 1/1, full sibling
26/26 (153.09s), formatting, strict workspace Clippy, deterministic Java
replay, oracle syntax, and diff checks pass. Continue at phase-two queue 17
x68/SIG76.

## Boundary 197: aligned x68 append and x31 prelinked return

Native queue17 x68/SIG76 selects LEFT/BOTTOM, resolves glyph40 to Stem
identity30/vertex224, preserves x70/SIG105 edge283 and x74/SIG120 edge284,
and appends one edge without other graph or registry mutation. Queue18
x31/SIG114 then uses the generic prelinked path with zero changes. Java
queues16-17 independently confirm the aligned x68 append through
glyph198/Stem2208 and x31's following no-op. Production reaches queue19
x14/SIG58.

The 11-line / 7,175-byte fixture is byte-identical across warmup plus two
fresh runs. Runner/transform/fixture/body+semantic hashes are `77a6d85e…`,
`b64a7aae…`, `19b0a62c…`, and `0296416a…`; Boundary 196's pins are
`ff8c906f…` / `614570ef…` / `aa8a4c50…`. Focused 1/1, full sibling 26/26
(153.38s), formatting, strict workspace Clippy, deterministic Java replay,
oracle syntax, and diff checks pass. Continue at phase-two queue19 x14/SIG58.

## Boundary 198: aligned x14 append

Native queue19 x14/SIG58 selects LEFT/BOTTOM, resolves glyph41 to Stem
identity32/vertex226, preserves x8/SIG89 edge319, x13/SIG101 edge320, and
x17/SIG112 edge321, and appends one edge without other graph or registry
mutation. Java queue18 independently confirms the aligned transaction through
glyph199/Stem2210 and its three retained relations. Production reaches queue20
x45/SIG62.

The 10-line / 6,704-byte fixture is byte-identical across warmup plus two
fresh runs. Runner/transform/fixture/body+semantic hashes are `eb79eb1d…`,
`06095681…`, `8363a188…`, and `8c7933fa…`; Boundary 197's pins are
`77a6d85e…` / `19b0a62c…` / `b64a7aae…`. Focused 1/1, full sibling 26/26
(151.75s), formatting, strict workspace Clippy, deterministic Java replay,
oracle syntax, and diff checks pass. Continue at phase-two queue20 x45/SIG62.

## Boundary 199: aligned x45 append and x56 prelinked return

Native queue20 x45/SIG62 selects LEFT/BOTTOM, resolves glyph42 to existing
Stem identity23/vertex217, preserves x43/SIG103 edge323 and x48/SIG116
edge324, and appends one current relation without vertex, stem, glyph-ID, or
allocator mutation. Queue21 x56/SIG82 then returns true through the generic
prelinked path with no graph or closure-value change. Production reaches the
next fail-closed append at queue22 x71/SIG66.

Java queue19 independently confirms x45/SIG62/Inter1069 through
glyph200/Stem2201 and edges342→343; Java queue20 confirms x56/SIG82/Inter1109's
prelinked no-op. Native and Java relation geometry remain independently pinned
at their exact low bits.

The 11-line / 6,787-byte fixture is byte-identical across warmup plus two
fresh runs. Runner/transform/fixture/body+semantic hashes are `29733c6d…`,
`4b3029fe…`, `59f27d58…`, and `37c5c7dd…`; Boundary 198's pins are
`eb79eb1d…` / `8363a188…` / `06095681…`. Focused 1/1, full sibling 26/26
(150.63s), formatting, strict workspace Clippy, deterministic Java replay,
oracle syntax, and diff checks pass. Continue at phase-two queue22 x71/SIG66.



## Boundary 200: Cucaracha system-one phase-two completion

Native queue22 x71/SIG66 selects LEFT/BOTTOM, resolves glyph40 to existing
Stem identity30/vertex224, preserves x70/SIG105 edge283 and x74/SIG120
edge284, and appends one current relation without vertex, system-stem,
glyph-ID, or allocator mutation. Native independently pins relation grade/dx
bits `3fe5554e97cdff05` / `3fbd29be97edf9e8`. The continuation returns true
and advances index22→23, exhausting the complete Cucaracha system-one native
phase-two queue. Production next fails closed at Cucaracha system 2 queue8
x56/SIG78's real `reuseStem` append.

Java queue21 independently confirms x71/SIG66/Inter1077 through
glyph198/Stem2208, retains Inter1155 and Inter1187, and changes only edges
343→344. Its independently computed relation bits are
`3fe5554e97ce0182` / `3fbd29be97edf3cf`; vertices232, system stems38, and
allocator2216 remain unchanged.

The 10-line / 6,284-byte fixture contains seven semantic rows plus summary and
is byte-identical across warmup plus two fresh runs. Runner/transform/fixture/
body+semantic hashes are
`3ad18d6e2db7b60980a27deef414bf54ac86df1fdfc127b26539172b4665e918`,
`a9daae9d492b63c9b9e091f0522bf7e42d270ef113a6f63f5a323066764c0d01`,
`457f8f28ca9a62fd085b27d5e574b1ff71a9f2f211dec9a0a82d4c30432c20d5`,
and `5ce49912b802895b8c9c549ef8b08c92c08f6a8942b6d0bd02f8c3f4a2d12f94`.
Boundary 199's strict pins are `29733c6d…` / `59f27d58…` / `4b3029fe…`.
Focused 1/1, full sibling 26/26 (153.52s), formatting, strict
workspace Clippy, deterministic Java replay, oracle syntax, and diff checks
pass. Continue at Cucaracha system 2 phase-two queue8 x56/SIG78.

[porting]: https://github.com/olaugh/audiveris/blob/master/rust/PORTING.md
[handoff]: https://github.com/olaugh/audiveris/blob/master/rust/HANDOFF.md
