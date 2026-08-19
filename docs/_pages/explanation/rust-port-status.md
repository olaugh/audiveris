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

The first forty-five semantic `STEMS` boundaries are production-shaped and graded (the eighteenth through twentieth under the fast-evidence policy documented in `rust/PORTING.md`; the twenty-first through twenty-fifth use fresh post-SIDES JVM fixtures, with the twenty-third and twenty-fourth adding later-frontier evidence for unchanged production code and the twenty-fifth adding a bounded atomic STUMPS driver; the twenty-sixth removes and resumes past one real competing hook from an explicitly reconstructed Allegretto checkpoint; the twenty-seventh enters the first typed post-STUMPS head frontier, the twenty-eighth atomically applies its bounded single-item, nonrecursive `CreatedChecked` mutation, the twenty-ninth carries the next two prelinked-success heads plus their ordered shared-stem closure writes, the thirtieth through thirty-second carry three further prelinked-success heads to index 6, the thirty-third consumes the first later BottomOnly head C-link to index 8, the thirty-fourth through forty-third carry ten further prelinked-success heads to index 18, the forty-fourth consumes that first both-open frontier through bounded two-item LEFT/BOTTOM C-link geometry to index 19, and the forty-fifth carries its prelinked successor and ordered shared-stem closures to index 20).
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
allocator/union state, the sparse 16-row selected-base Java identity authority, native predecessor carriage plus wider coverage for the reconstructed Allegretto linked-S/hook-removal path, wider-corpus STUMPS authority and branch coverage, and later STEMS phases
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
| 5 | `HEADERS` | **Native and published** | `recognize_native_headers` composes clef, key, and time columns in Java order from live GRID state alone. All nine pages and 65 staves match for starts/stops and selected evidence, including 34 keys, 17 times, and all 30 downstream erase rectangles. Schema 1 publishes selected inters, lifecycle/classifier evidence, staff ranges, and system-owned erases. Header SIG grades are carried per inter rather than only per key: each key alter retains its own pitched grade with `intrinsicRatio` applied, and a key's grade is the mean of its members. Chula system 1's ten header grades now match Java bit for bit, closing the measured SIG grade ledger at 221/221. | Widen the header corpus and complete the remaining recognizer integration. |
| 6 | `STEM_SEEDS` | **Native and published** | `recognize_native_stem_seeds` composes live GRID and HEADERS state through lag selection, vertical `StickFactory`, staff/header gating, the concrete checker, fixed-glyph materialization, and free-glyph ownership. Across 30 systems, all 2,425 raw candidates, 422 header skips, 2,003 checks, 97 rejects, and 1,906 accepted glyphs match Java, including bit-exact grades and complete run-table digests. Schema 1 publishes accepted seeds in production order with geometry and exact checker/materialization evidence. The BEAMS adapter and CLI validate and preserve every accepted per-system identity and median. | Widen beyond profile 1 and add tablature/no-staff skip cases. |
| 7 | `BEAMS` | **Native and published** | Native GRID -> HEADERS -> STEM_SEEDS composition feeds the spot chain, system dispatch, beam creation, measured extension, hooks, grouping, and schema-1 output. A fresh-JVM Java counterfactual over 803 final beam/hook inters, 493 groups, and one multiple rest proves actual seeds change zero records on the original eight pages. D039 adds the natural acceptance case: one system-2 beam changes, with endpoint, height, six impacts, and grade bit-exact to Java. The original gate still matches 2,739 spots, 30 erases, and 787/787 raw beams. Production retains exact group memberships and now runs the real MultipleRest pass from a freshly recomputed staff projector: Bach system 6 replaces source ordinal 182 with median, grade, height, staff, and two-serif evidence exact to Java; the retained start/stop pitch is a port-pinned intermediate, since Java's oracle publishes the rest's grade and bounds but never its pitch. | Allocate stable SIG/glyph/relation identities for the retained MultipleRest and serifs, then grade small beams and widen the corpus. |
| 8 | `LEDGERS` | **Native and published** | Native composition consumes GRID's `NO_STAFF`, curved staff/system geometry, and the oracle-free BEAMS result after MultipleRest source-beam deletion. Schema 1 includes all seven impacts, live exclusions, and curved inferred paths. All 581 final Java inters and 95 inferred paths on the eight beam sheets match after sheet-wide one-sigma post-analysis and rebuild. Every final live ledger now retains its exact positioned fixed glyph raster from the referenced filtered sections; Chula's per-system section dispatch is also exact at 2,042/591/961. Ledger grades are now gated on raw f64 bit patterns rather than the nine-decimal fixture: all eight of Chula's system-1 ledgers match Java bit for bit, after correcting `y_at_x_ext` to evaluate the staff-line spline the way `LineInfo.yAt` does. | Widen beyond the example corpus. |
| 9 | `HEADS` | **Native and published** | The complete production entry point composes live GRID, HEADERS, STEM_SEEDS, BEAMS, and LEDGERS state through prolog, template lookup, seed and range glyph creation, staff duplicate/overlap handling, attachment, small-beam arbitration, and tally analysis. The eight-page top-level differential matches all 3,609 heads entering the epilog, 62 duplicate removals, 2,725 overlap exclusions, 3,547 post-duplicate heads, 191 beam inputs and registered glyphs, 10,053 ordered beam checks by exact per-system hash, 26 head removals, 3,521 final heads, 1,451 tally inputs, and 18 scale rows. Schema 1 publishes identity-free final-head provenance, exact glyph evidence, beam decisions, counts, and scale rows. | Widen the published corpus. |
| 10 | `STEMS` | **Components graded** | Forty-five exact production boundaries consume live final HEADS, GRID, BEAMS, and STEM_SEEDS state. The first nine own constructor, stump, reachability, and builder preparation; boundaries 10-20 grade scheduler planning and exact base/sibling/head SIG mutation plus B/S shared-cell effects, boundary 21 enters STUMPS, boundaries 22-24 execute/resume the first three transactions, and boundary 25 atomically drives the remaining four to typed post-STUMPS completion; boundary 26 removes and resumes past one real competing hook from a reconstructed Allegretto checkpoint; boundaries 27-45 carry the typed head phase through three bounded C-link mutations and the intervening prelinked-success continuations to `current_index=20`, with no unlinked head. Boundary 44's third mutation is exact only for its two-item LEFT/BOTTOM geometry; Boundary 45 carries the following prelinked-success closure without changing graph or stem counts. Chula system 1 runs all 32 SIDES transactions, reaches explicit `SidesExhausted` at 253 vertices / 331 edges, then carries all seven STUMPS transactions to 260/353, 39 Stem bindings, 68 linked B cells, and 83 linked S cells after 92 scheduler events. Boundaries 38-45 use separate snapshot-minimized Java derivatives: predecessor orders reconstruct state without persisted full intermediate snapshots, and only the target order is emitted. Plan 404 is the first natural two-glyph compound candidate in this carried prefix. The exact 32 SIDES plan/B-linker tuples and 29 sibling-write lists match Java after native return. A bounded Allegretto reconstruction grades real graph-derived linked-S B13 selection, unread-suffix behavior, and the first competing-hook removal; transactions 1-27 are not natively replayed by those gates. A one-time first-STEMS bridge removes per-frontier glyph evidence from transactions 3-32; its 1,650-entry persistent snapshot and a sparse 16-entry selected-base Java identity bridge remain disclosed. | Replace those remaining authorities, carry the Allegretto predecessor natively, widen linked-S, hook-removal, and STUMPS corpus coverage, carry the remaining ordered head queue from the both-open order-20 frontier, actually-unlinked retry, and broader C-linker shapes, and carry the remaining STEMS phases. |
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
| Visual classifier core | **Components graded** | Frozen model parsing/inference, features, stable ranking, and glyph construction are native. The ART lookup-table math reproduces the measured OpenJDK/HotSpot paths: all 12 frozen key-alter vectors match Java at all 110 inputs, including all 99 ART moments. Remaining size/noise gates, `ShapeChecker`, user overrides, and later-stage integration are not complete. |
| `.omr` persistence | **Components graded** | Opaque round-trip and typed views cover the measured book/sheet metadata and ownership structures. Full native recognition output is not yet an end-user replacement for Java. |
| CLI, JSON, and live comparison | **JSON published through `HEADS`; completed-stage viewer live** | Real images and PDFs compose GRID -> HEADERS -> STEM_SEEDS -> BEAMS -> LEDGERS -> HEADS in native Java order for the applicable JSON target; GRID keeps its text report. Ordinary `-json` remains the schema-1 JSONL interface. The opt-in `-stream-json` protocol adds flushed boundary markers around unchanged completed-stage documents for `omrscope`, which starts Java and Rust independently, retains every completed snapshot, and lets the user select it. It deliberately provides no intra-stage or per-item stream. The Page/Inters UI graphically highlights an inspected table row without native table selection, offers opt-in filtered-row highlighting, and shows only engine-local relation edges whose endpoints resolve in the selected snapshot. HEADS documents retain all upstream products and add identity-free final heads, complete seed/range provenance, exact head glyphs, source-resolved beam decisions, counts, and tally-scale rows. `omrscope` consumes bounds-only headers, both median forms, and accepted top-level stem seeds; it refuses rejected or incomplete seed geometry rather than inventing coordinates. |
| Manual Java score preview | **Inspection only; not a parity gate** | A separate Score tab explicitly runs one selected Java sheet through PAGE, validates its single local MusicXML/MXL artifact, and renders it with locally installed Verovio to SVG. Sheets requiring sibling multi-page artifacts are rejected rather than guessed. It is not part of recognition streaming, which still stops at HEADS, and it makes no Java/Rust visual or semantic comparison claim. Future Rust MusicXML will use the same renderer path. |
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

1. Replace the disclosed first-STEMS persistent snapshot and sparse 16-row selected-base Java identity authority; transactions 3-32 already need no per-frontier glyph rows.
2. Replace the reconstructed Allegretto transaction-28 predecessor with native-carried state and widen graph-derived B13 linked-S and hook-removal coverage.
3. Continue from Boundary 45 at head index 20, beginning with the measured but unexecuted both-open x74 / SIG 19 frontier: carry the remaining ordered phase-1 queue, then implement actually-unlinked and rather-good retry/no-link closure, phase-2 append retries, generic multi-item/recursive C-linkers, and remaining head branches; also widen STUMPS and competing-hook coverage beyond their single-system/checkpoint evidence.
4. Expose `recognize_native_stems` once the full scheduler path runs from native products.
5. Allocate stable MultipleRest/serif identities, grade small-beam pages, and widen the published recognition corpus.
6. Add end-to-end MusicXML differential grading after `PAGE` is meaningful.

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

[porting]: https://github.com/olaugh/audiveris/blob/master/rust/PORTING.md
[handoff]: https://github.com/olaugh/audiveris/blob/master/rust/HANDOFF.md
