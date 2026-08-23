# Rust port takeover

This is the continuation record for the source-guided Audiveris Rust port. Read
`PORTING.md` first, then this file. The port is an AGPL-3.0-or-later derivative and
is intentionally parallel to the unchanged Java production tree.

Maintainer-ready Java defects and compatibility risks found during the port are
tracked in `AUDIVERIS_UPSTREAM_FINDINGS.md`; update that catalog whenever a
production-source issue is confirmed.

## Repository state

- Repository: `/Users/john/sources/aug04-rubigo/audiveris`
- Branch: `rust-port-continue`; completed checkpoints are pushed to
  `github.com/olaugh/audiveris` on `master`, and only there -- pushing the same
  commits to a second branch doubles CI for nothing.
- Java baseline: Audiveris 5.11.0, source commit
  `9e1e55cd2746037d059345881c53e6a6754bffbd`
- Rust workspace: `rust/`
- JDK 25: a sibling of the checkout, `../jdk25/Contents/Home` -- currently
  `/Users/john/sources/aug04-rubigo/jdk25/Contents/Home`. `xtask` resolves it that
  way whenever `JAVA_HOME` is unset (`rust/xtask/src/main.rs:222`), so the relative
  position is the contract and the absolute path is only today's instance.
- Java test baseline: 39 suites, 212 executions, 0 failures, 0 errors, 1 skip

The Java checkout has 991 production files and about 327,673 lines. Its unit suite
does not run the 20-stage recognizer, save an asserted `.omr`, or compare MusicXML.
Do not equate either Java or Rust unit-test success with recognition parity.

## Current status (read this first)

The CLI now performs native schema-1 JSON recognition through STEMS. GRID's
human-readable report remains unchanged; HEADERS through STEMS require `-json`
and compose in Java stage order rather than accepting invented downstream
inputs. STEMS runs GRID -> HEADERS -> STEM_SEEDS -> BEAMS -> LEDGERS -> HEADS
-> STEMS, retains every upstream product, and adds identity-honest final Stem
geometry/grades, HeadStem links, abnormal/no-stem evidence, and terminal counts
without fabricating Java SIG, Inter, or Glyph IDs.

`omrscope` now compares the two producers while they run: Rust and Java start
independently, each publishes an immutable snapshot once it completes GRID,
HEADERS, STEM_SEEDS, BEAMS, LEDGERS, HEADS, or STEMS, and the viewer can select any
retained snapshot. This is stage-boundary visibility only -- neither producer
claims to stream individual recognition items while a stage is executing. The
opt-in Rust `-stream-json` framing adds flushed `@omrscope` markers around the
existing schema-1 documents; ordinary `-json` output and the ordinary Java
oracle probe output remain compatible.

The Page/Inters inspector now gives those retained snapshots a graphical audit
surface: inspecting a table row highlights the corresponding paired
interpretations on the page, an opt-in control highlights all filtered rows,
and optional graph edges are drawn only from an engine's own relations whose
endpoint IDs resolve uniquely in that same selected snapshot. It does not
manufacture cross-engine graph topology.
The separate manual Score tab is deliberately outside that streaming pipeline:
it runs one selected Java sheet through PAGE, validates the explicit local
MusicXML/MXL artifact, then asks locally installed Verovio to make SVG pages.
A sheet requiring Audiveris's sibling multi-page artifacts is rejected rather
than guessed. The Java engraving is a convenient artifact preview, not a
visual or semantic Java/Rust parity claim. Rust PAGE, score assembly, and
MusicXML output remain unimplemented; when they exist, the Rust artifact must
pass through this same renderer before the tab can become a comparison surface.

`recognize_native_headers` now closes the integration issue that audit found:
it accepts only live GRID state, derives real `HeadlessHeaderSystem` bar/group
ownership, header starts, specific interlines, and connected-bar browse limits,
then composes clef, key, and time columns in Java order. Java records are read
only after that call for grading. All nine example pages and 65 staves match,
including 34 keys, 17 times, and 30 downstream erase rectangles. Selected
header inters, ranges, evidence, and system-owned erase rectangles are now
published by `audiveris-cli -batch -step HEADERS -json <image>` and retained in
the BEAMS and LEDGERS documents.

A Graceful Ghost Rag follow-up closed a real-scan F-clef pitch divergence that
the original 65-staff corpus did not exercise. Clef target pitch now samples the
first and last native staff-line splines at the glyph centroid x, matching
Java's `staff.pitchPositionOf(center)`; the old header-midpoint ordinates are an
explicit fallback only when spline evaluation is unavailable. Production also
derives Bravura's F-clef area pitch offset instead of assuming zero. On the
measured system-1 crop Java and Rust now classify the same x=103/y=250/w=29/h=53
glyph as `Bass`, and all 20 warped plus all 25 dewarped Graceful Ghost system
crops contain zero `Baritone` clefs. The frozen 65-staff clef corpus and the
full native HEADERS differential remain unchanged. One low-resolution full-page
staff on page 5 still reaches `Baritone` while its high-resolution system crop
is `Bass`, which is retained as a wider GRID/preprocessing geometry issue;
full-page page 3 still fails earlier during GRID brace processing.

Against a live Java 5.11 oracle across all nine `data/examples` pages:

| Output | Status |
| --- | --- |
| Binary raster | 9/9 pages bit-identical |
| Staff abscissae | 65/65 exact |
| Barline abscissae | 420/420 exact |
| Completed staff-line endpoints | 1300/1300 exact |
| Sheet SIG | all 420 barline inters and 184 connectors promoted; every median and every intrinsic and contextual grade exact on every page |
| Beam spot chain | all 8 transforms bit-identical, and all 305 of chula's spot glyphs by bounds, weight and centroid |
| Beam recognition | **787/787 raw beams across 8 sheets** -- system ownership, geometry, all six impacts, and grade exact after production HEADERS runs from GRID alone. 7 of 8 sheets exact through the end of BEAMS |
| JPEG decoding | bit-exact against the libjpeg Audiveris bundles, on 130 fixtures and 140 sampling combinations |
| PDF reading | 189/189 corpus pages: geometry, image structure, raw stream bytes, and **every filter chain** byte-identical to PDFBox |

| Spot-to-system dispatch | 2739/2739 spot centroids on 8 sheets, exact |
| Stem scale | Java's `maxStem` on all 8 sheets, from the uncleaned raster |
| Symbol centroids | 6 header clefs, pinned bit-exact |
| Symbol outline bounds | 1624/1624 swept values on 14 shapes x 116 sizes, exact |
| Clef classification | 65/65 corpus staves: shape, symbol box and `clefStop` exact |
| Key classification | 65/65 staves: presence, fifths, union box and `keyStop` exact |
| Time classification | 65/65 staves: presence, value, symbol box and `timeStop` exact |
| Final `header.stop` | 65/65 staves exact; all **30** system header erases exact |
| Native beam composition | 2739 spots, 30 header erases, 787 raw beams, final beams/hooks and per-system group counts graded on all 8 sheets; production now retains each ordered group membership rather than only its count |
| HEAD_SPOTS handoff | threshold-170 vertical RunTable retained by production BEAMS; Java size and two independent pixel digests exact on all 8 sheets |
| Native ledger composition | all 581 final Java inters and 95 inferred ledger-line paths across the 8 beam sheets are exact; chula traces 9915 runs → 4052 sections → 104 candidates → 19 builder survivors → 18 final inters |
| Final ledger glyphs | every non-removed ledger retains a 1:1 positioned fixed raster built from its referenced filtered sections; no median approximation |
| Registered beam glyphs consumed by HEADS | all 191 narrow-beam bounds, weights, and vertical run digests exact after Java-equivalent `NO_STAFF` masking inside each final parallelogram |
| Complete native HEADS | the owned production entry point is exact for 3,609 epilog inputs, 62 duplicate removals, 2,725 overlap exclusions, 26 beam-defeated heads, 3,521 final heads, 1,451 tally inputs, and 18 scale rows; schema-1 CLI publication is live |
| First semantic STEMS boundary | the production head-corner compositor consumes live HEADS/STEM_SEEDS products and matches 3,521 heads plus all 14,084 constructor-order reference/outside/inside corner points across 30 systems at exact double bits; it stops before stump lookup and mutation |
| STEMS no-stem purge and existing-seed boundary | the production compositor consumes live GRID/STEM_SEEDS/head-corner products, matches 1,906 seeds -> 1,749 kept / 157 purged, 29,394 purge visits, 36,736 neighbors, 7,114 candidates, 4,182 selections, and 9,902 explicit section fallbacks across all 14,084 corners; it stops before section-built stump registration |
| STEMS section-built head-stump boundary | the production compositor consumes all 9,902 explicit fallbacks, matches 18,398 section/compound steps and 3,660 subsection attempts, and reproduces all 8,933 registrations: 758 accepted / 8,175 rejected and 5,591 new / 3,342 reused, with stable canonical aliases |
| STEMS BeamLinker stump-preparation boundary | the production compositor matches constructor-time `retrieveStumps()` for 803 BeamLinkers and all 1,606 sides: 3,934 neighbors, 1,820 seed inputs, 1,087 purge comparisons, 301 missing-side builds, 6 direction-accepted builds (5 new registrations and 1 canonical reuse), 1,821 final stumps, 1,311 final side stumps, and zero tremolos; it stops before `equipStumps`/`equipOrphanSides` creates B/V linker topology and geometry |
| STEMS BeamLinker B/V-construction boundary | the production compositor matches sequential constructor-time `equipStumps`/`equipOrphanSides` for all 803 BeamLinkers: 2,116 BLinkers, 2,417 VLinkers, 2,860 Part folds, 9,186 closer-beam candidates with 703 limiter rebuilds, and 12,491 final-area seed checks with 2,169 reachable; it stops before HeadLinkers and source-ordered cross-beam anchor mutation |
| STEMS source-ordered beam/head reachability boundary | the production compositor visits all 2,417 VLinkers in Java order, performs 1,617 cross-beam searches, creates 145 anchors and reuses 215 anchors, preserves immediate/final B arenas, and accepts 5,059 ordered head corners after exact area/distance/void-side filtering; it precedes the beam-origin builders |
| STEMS beam-origin `StemBuilder` boundary | every beam-origin V inspection reaches the actual source-ordered `StemBuilder` constructor and V `sb` assignment for 2,417 builders. The direction differs from its V only for Carmen system 2 / builder 56 (1,390 TOP / 1,027 BOTTOM builders); 2,169 seeds become 1,954, 6,676 targets become 6,670 (1,617 B / 5,053 C), 1,442 chunk glyph registrations yield 799 new and 643 reuse, 175 chunks are removed, and 9,419 final items yield 12,085 length rows. The bounded registry has zero external/unmodeled reuse without claiming global novelty; SIG/system-stem/link/C-builder/unexpected mutation counts are all zero |
| STEMS head-corner reachability boundary | `materialize_native_stems_head_corner_reachability` is production and exact across 8 pages / 30 systems / 3,521 heads / 14,084 corners. It assigns 1,340 seeds, retains 4,566 C and 8,120 B targets in C-before-B order, writes every C seed list, and creates 1,687 head-origin anchors for 3,948 final BLinkers. Its 16,501 checks preserve the 2,417 V builders and keep all 14,084 C builders null, with zero forbidden SIG/link/registry mutation; the normal gate has 2 tests and 0 ignored |
| STEMS head-origin `StemBuilder` boundary | `materialize_native_stems_head_builders` is the ninth exact boundary: all 14,084 C-origin builders materialize after the 2,417 beam builders in the real system-interleaved registry chronology. The full stream matches 19,295 head registrations (4,619 new / 14,676 reuse), 29,120 items, 165 gaps, 70,420 profile lengths, and 42,252 sort audits; SIG/system-stem/link/unexpected mutations are zero. The corpus is inspect-profile 1 with no profile divergence or VIP heads; profile divergence and JDK sort inputs at 32 fail closed, while Java's 6,087 non-VIP low-remain keeps and processed-without-compound sticker rule remain live. The normal eight-page gate passed twice (84.48s / 88.93s), and strict integration-test Clippy is green |
| STEMS beam-origin `VLinker.expand`/link-plan boundary | `materialize_native_stems_beam_link_plans` is the tenth exact boundary: an immutable matrix evaluates profiles 0 through 3/4 for every inspected non-anchor beam builder and stops before `StemBuilder.createStem`. The full eight-page gate matches 11,573 plans, 18,345 final relations, 12,523 final Glyph entries, and all 120,646 body lines (120,636 semantic rows plus the 10-line shared header). It exposes 3,226 downward shared-line/current-attachment deltas, 49 rollback-line divergences, two dynamic-side mismatches, the profile-4 terminal-head 9/632/645 partition, and zero forbidden mutations without applying scheduler state. Eleven focused units, two full runs (32.25s / 32.41s), and strict integration-test Clippy are green |
| STEMS deterministic beam-scheduler frontier | `materialize_native_stems_beam_scheduler_frontiers` is the eleventh exact boundary: it reconstructs 651 page-global canonical live Glyph aliases and 78 live raw hook/full-beam Exclusions, then replays all 803 beams in stable reverse-width/SIG order across 322 width ties. The 30 systems reach 56 attempts: 26 empty-target skips remove 14 beams from local worklists, then 30 `ReadyForCreateStem` plans stop as typed `AwaitingVLinkTransaction` frontiers. Ready is feasibility, not success. Fourteen pending downward line/attachment deltas remain unapplied; known-false invocation, stump, hook-removal, retry, completion, and persistent-mutation counts are zero. Eight focused units, three active integration tests with one fast diagnostic ignored, the 31.09-second integration run, the independent 31.41-second root full gate, and strict integration-test Clippy are green |
| STEMS first awaited beam-V `createStem` transaction | `apply_native_stems_beam_vlink_create_stem_transaction` is the twelfth exact boundary: it applies prior/pending aliased line deltas and exactly executes candidate construction, structural GlyphIndex registration, structural `systemStems` lookup, `StemChecker`, and checked/artificial/reused return. Across 30 transactions it matches 15 compound candidates with pre-registration object ID 0, 15 singletons, 14 line changes, 30 active `ReuseActive` Glyph lookups, 30 Absent real system-stem lookups, and 30 `CreatedChecked` results. Returned median/mean-thickness bits and vertical-ribbon integer bounds are exact; every returned Inter is ID 0, non-abnormal, and SIG-null. Allocator/SIG/relation/link-flag deltas are zero. Eleven focused units and the 5/5 31.98-second gate are green |
| STEMS beam-V reuse/check boundary | `evaluate_native_stems_beam_vlink_reuse_check` is the thirteenth exact, read-only boundary: it preserves the ordered/lazy head-side stem-reuse loop and exact `BeamStemRelation.checkLink`. The original 65 first-frontier C entries remain all unlinked, with 30 accepted checks and zero reuse. A later Allegretto-derived reconstruction exercises the linked-S branch: transaction 28 / plan 25 traverses HeadStem edge 229, selects the modeled attached StemInter with Java ID 2227, and leaves relation-map entry 1 unread after the unique break. Eight isolated synthetic SIG blocks cover the remaining reuse and numerical branches. It stops at `ReadyBeforeSigMutation` |
| STEMS beam-V base SIG/BeamStem application | `apply_native_stems_beam_vlink_base_transaction` is the fourteenth exact boundary: it conditionally executes `SIG.addVertex` and then applies the checked base BeamStem relation with exact index/SIG ordering, duplicate suppression, JGraphT callbacks, abnormal/dirty effects, and partial prefixes. All 30 real rows are `NewIdZero` with vertex and edge added, reuse zero, and zero ChordStem matches; 40 supported and 32 envelope cases are isolated evidence, not production-equivalence. Twenty focused units and the 10/10 33.87-second gate are green; it stops at `ReadyBeforeBLinkerFlagMutation` |
| STEMS beam-V B-linker shared flag assignment | `apply_native_stems_beam_vlink_b_linker_flag_transaction` is the fifteenth exact boundary: it independently reruns boundary 14 from its exact pre-state, resolves the scheduler-selected outer B and its TOP-then-BOTTOM V observers, and executes one plain `linked = true` assignment while retaining the ignored base-link return and fresh draft grade. All 30 real writes change false to true across a guarded Java arena of 3,948 entries (2,116 frozen + 1,832 dynamic anchors). Eight isolated blocks add 32 setter/shared-cell-only Unsafe exact-class envelopes: 24 false-to-true, 8 idempotent, and 8 with `applyReturn=false`. Seven focused units and the shared 5/5 126.03-second hydration regression are green; it stops at `ReadyBeforeSiblingBeamLinks` |
| STEMS beam-V sibling BeamStem links | `apply_native_stems_beam_vlink_sibling_links_transaction` is the sixteenth exact boundary: it exact-replays Boundary 15 and executes the complete serial sibling loop through group ordering, identity/duplicate/shorter branches, fresh BeamStem edge callbacks, zero-Chord validation, beam abnormal/dirty effects, and optional sibling B-cell writes. The 30 real transactions expose 58 non-null native-glyph group members and 11 siblings; all 11 link, add one edge, and write one B cell. Eight isolated blocks add 64 supported branch cases and 16 Java throw envelopes without claiming production equivalence. Twenty-two focused units and the 10/10 126.68-second full exact gate are green; it stops at `ReadyBeforeHeadRelationLoop` |
| STEMS beam-V head relations | `apply_native_stems_beam_vlink_head_links_transaction` is the seventeenth exact boundary: it exact-replays Boundary 16 and executes the insertion-ordered head map through unconditional shared S-cell assignment, exact directed duplicate lookup, lazy consistency mutation of the existing plan draft, direct HeadStem insertion, and synchronous head/stem abnormal and dirty callbacks. The 30 real transactions contain 65 entries, zero duplicates, 65 inserts, 65 S-cell and consistency writes, and 260 ordered events. Eight isolated blocks add 16 supported and 40 envelope transactions without claiming production equivalence. Twenty-four focused units, the 13/13 148.82-second full exact gate, and the 1/1 129.11-second manifest validator are green; it stops at `ReturnedTrueBeforeOuterBLinkerAssignment` |
| STEMS outer B-linker assignment (fast evidence) | `apply_native_stems_beam_vlink_outer_b_linker_transaction` is the eighteenth exact boundary, the first under the fast-evidence policy in `rust/PORTING.md`: it executes the caller seam in `BLinker.link` after `VLinker.link` returns true - one idempotent `setLinked(true)` on the outer B-linker (the same shared cell Boundary 15 wrote from inside `VLinker.link`), plus the certified lexical-parent identity and the EnumMap loop-resumption facts. The frozen fast corpus is chula and BachInvention5 (9 of the 30 real transactions), generated by a single fresh-JVM pass whose runner required the re-emitted Boundary-17 rows to match the frozen head-links fixture byte-for-byte; the Rust gate re-pins that fixture by SHA-256, replays Boundaries 12-17 through the production functions, and matches every row field. All nine transactions are idempotent single-V writes with zero value changes; multi-V ordering, skip counting, and every refusal path are unit-covered. It stops at `AssignedOuterBLinkerBeforeNextVIteration`. The checkpoint has since raised it to full evidence: all eight sheets, two byte-identical fresh-JVM passes each, and the gate asserts it grades every installed sheet |
| STEMS scheduler SIDES resume (fast evidence) | `resume_native_stems_beam_scheduler_after_transaction` is the nineteenth exact boundary: after Boundary 18, the SIDES worklist resumes from the suspended position with exact Java loop semantics - remaining V linkers of the completed B, the side result the outer assignment determined (`SideBLinkerResult` with `linked_flag_after` true), remaining sides and beams - and stops at the second `ReadyForCreateStem` frontier as a typed `AwaitingVLinkTransaction`, or reports SIDES exhaustion. Scope is SIDES only; the STUMPS continuation is a later boundary. On chula and BachInvention5 all nine systems reach a RIGHT-side second frontier on the same beam, and every resumed expand outcome equals the frozen Boundary-10 matrix even though Java ran it against the post-transaction SIG - an equivalence the gate checks per row, so a page where the mutation changes an outcome fails loudly. The probe re-emits Boundary-17 and Boundary-18 rows and the runner requires byte-identity with their frozen fixtures. The checkpoint has since raised it to full evidence: all eight sheets under two byte-identical passes, with the gate asserting it grades every installed sheet. The scheduler-only all-success chain reaches SIDES exhaustion on most systems and typed competing-hook frontiers on allegretto 1 and 3; Boundary 26 separately gates the first real Allegretto-system-1 removal from a reconstructed predecessor. The multi-V continuation occurs three times across two systems (batuque 1, BachInvention5 6) - multi-V B linkers are common but are almost never the frontier's own side linker, so that assertion is a deliberate floor of one and a failure there means the corpus stopped covering it. A failed link and an already-linked B linker are driven by re-running a real frontier with the one bit flipped, which shortens chula system 1 from 53 chained transactions to 36 |
| STEMS SIDES-pass chain (historical diagnosis, now closed natively) | The early synthetic chain ran 53 transactions against Java's 32 because it carried no B16 sibling writes; the 21-transaction excess exactly matched Java's 21 `AlreadyLinked` skips. The production carrier now computes and carries all 29 sibling-write lists itself, executes Java's exact 32 plan/B-linker tuples, and reaches SIDES exhaustion. `scheduler_resume_chain_composes_without_repeating_a_v_linker` remains the narrow composition proof; `native_carrier_drives_full_sides_pass_before_oracle_read` is the current owned-state closure |
| STEMS self-driving chain: original blocker survey (superseded operationally) | This dated survey correctly identified registry completeness as the core blocker. Boundary 136 replaces the disclosed 1,650-entry first-STEMS snapshot with `NativeStemsModeledGlyphRegistry` for transactions 2 through the terminal, and Boundary 138 extends the same exact-content/native-ordinal authority through transaction 1. All SIDES, STUMPS, and HEADS glyph joins now avoid the 592 opaque Java fingerprints and page-wide union watermark. Boundaries 139-143 remove the sparse selected-base bridge, derive first-B14 compact state from the owned SIG, replace Java's shared allocator seed with the native modeled-glyph identity domain, derive each system-visible registry boundary from production head-builder chronology, and atomically create the first B12-B19 SIDES carrier |
| STEMS chain self-drive: full chula beam passes carried | `advance_native_stems_beam_sides_transaction` owns each already-awaited B12-B19 transaction as an atomic shadow/swap over scheduler, latest B14/transaction state, SIG/bindings, and persistent B/S cells. Repeated calls execute all 32 chula-system-1 SIDES transactions, carry all 29 B16 sibling-write lists, and reach the explicit `SidesExhausted` state at the native 253-vertex / 331-edge terminal with 61 linked B and 68 linked S cells. Exact plan/B order and sibling aliases match Java only after return. `advance_native_stems_beam_sides_transaction_from_modeled_registry` drives transactions 2-32 from owned native modeled-glyph identity without candidate-specific Java evidence; Boundary 138 extends native modeled identity through transaction 1's B12/B13 path. B14 consumes a sparse 16-entry identity bridge for the distinct selected base beams rather than all 48 live beams; those entries still disclose Java Inter ID, sorted InterIndex ordinal, and VIP, while native stump/SIG products own every graph fact. The graph-derived B13 projector is additionally gated on one measured later linked-S reconstruction at Allegretto system 1 transaction 28; native carriage of its predecessor transactions remains open. The bounded atomic STUMPS driver carries all seven chula-system-1 stump transactions through the typed post-STUMPS terminal; Boundaries 27-133 then carry the typed head phase through thirteen bounded C-link mutations, fifty-six bounded existing-stem reconciliations, and the intervening prelinked-success continuations plus Boundaries 76, 86, 87, 94, and 101's returned-false LEFT undefs to `current_index=102`, the phase-1 queue length; the five returned-false heads (x32, x71, x70, x0, x31) are queued for Java's phase-2 append retry per StemsRetriever's caller loop, and Boundary 128 measures that queue directly, superseding the probe's hard-coded `unlinkedCount` zero; Boundary 133 authenticates that exact Chula no-op terminal, and Boundary 134 completes generic `finalizeStems` with real cleaner removals, canonical keeps, callback semantics, and the abnormal epilog; the dual-corner undef branch authenticates Java's shared-stump guard and fails closed on the unported differing-stump standard connection; Boundary 83's eighth C-link is the first that reuses an existing stem (2381) through one appended HeadStem relation and closes sibling x63 without vertex or allocator mutation, Boundary 93's ninth C-link reuses Stem 2382 through three appended HeadStem relations, linking x73 plus the carried undef heads x70 and x71 and closing x74, Boundary 96's tenth reuses Stem 2384 through two appended relations, linking x1 plus the carried undef head x0 and closing x2, and Boundary 98's eleventh reuses Stem 2385 through one appended relation with no chunk or crossed head, closing x23 and modeling Java's aliased twice-shifted stem line, Boundary 99's twelfth reuses Stem 2380 through two appended relations, linking x75 plus crossed head x72 and closing x76 and x72, and Boundary 119's thirteenth is the first RIGHT-side frontier, reusing Stem 2379 through one appended RIGHT-side relation and closing x38. Boundaries 38-133 use separate snapshot-minimized order-specific Java derivatives, not full predecessor-snapshot oracles. Boundaries 44, 46, 53, and 60 consume two-item LEFT/BOTTOM C-link frontiers; Boundary 62 consumes a bounded single-item LEFT/BOTTOM C-link. Boundaries 46 and 60 preserve the x74-specific one-ulp downward and x2-specific one-ulp upward line translations, while Boundaries 47-48 reconcile x28 and x4 against existing Stems 2378 and 2354 and Boundaries 63-75 reconcile x14, x18, x97, x6, x30, x43, x25, x83, x57, x40, x89, x52, and x35 against existing Stems 2340, 2372, 2373, 2348, 2357, 2350, 2356, 2358, 2374, 2350, 2359, 2344, and 2369, and Boundaries 77-82, 84, 85, 88-92, 95, 97, 100, 102-118, and 120-127 reconcile x19, x15, x84, x11, x68, x21, x92, x100, x9, x41, x3, x58, x13, x87, x77, x49, x66, x64, x82, x17, x29, x98, x80, x24, x94, x79, x51, x45, x72, x47, x27, x91, x54, x96, x7, x60, x44, x39, x56, x86, and x5 against existing Stems 2361, 2360, 2366, 2349, 2347, 2341, 2342, 2343, 2355, 2352, 2354, 2363, 2340, 2367, 2370, 2353, 2375, 2346, 2358, 2372, 2357, 2365, 2371, 2356, 2364, 2371, 2362, 2377, 2380, 2351, 2378, 2364, 2362, 2373, 2376, 2345, 2377, 2350, 2374, 2366, and 2348 while carrying the carried undefined LEFT sides, all without graph allocation; Boundaries 79, 108, 110, 112, 113, and 114's three-head shared stems re-write x86's, x79's, x91's, x54's, x44's, and x76's already-closed cells without a value change, and Boundaries 111, 117, 118, 123, and 124 re-write all four of Stem 2371's, Stem 2364's, Stem 2362's, Stem 2377's, Stem 2350's, and Stem 2366's sibling cells with no value change at all. Boundaries 49-52, 54-59, and 61 add no production operation: the generic continuation carries prelinked closures, with v28-v104 using the reduced heap-safe oracle shape and Boundary 58 adding the first zero-write closure in that suffix. Broader geometry, actually-unlinked/no-link, and generic retry remain open. Boundary 26 originally removed and resumed past the first real Allegretto competing hook from an explicitly reconstructed post-transaction-28 checkpoint; Boundary 144 now carries Allegretto transactions 1-28 natively to that checkpoint. Boundary 147 additionally makes `prepare_native_stems` own the page-wide checker and starts Batuque system 1 from production-prepared state; Boundary 148 moves the complete Batuque system-1 SIDES transaction loop into production and reaches true `SidesExhausted` fail-closed; Boundary 149 carries the exact page registry and shared persistent allocator through Batuque's system-2 constructor chronology without isolated identity reconstruction; Boundary 150 executes system 2's first `SharedSheetSerial` B12-B19 transaction with fresh system-local state and the carried page allocator; Boundary 151 drives all 40 system-2 SIDES transactions to true `SidesExhausted`; Boundary 152 carries and drives system 3, accepts the exact head-relation subset, and registers the first native compound glyph, completing all three Batuque SIDES systems. Early registry corruption, missing identities/cells, weak liveness, malformed hook topology, or incoherent head products fail closed. Page-wide STUMPS, successful append reuse, wider C-link shapes, and broader linked-S/hook-removal coverage remain |
| What finishing STEMS needed, measured 2026-08-12 (diagnosis now implemented) | This dated diagnosis correctly identified the absence of a production SIG as the cross-stage blocker. `NativeSigSystem` now assembles the exact GRID-through-HEADS graph with native identities and the production carrier runs chula system 1's full SIDES and STUMPS beam passes, so “the port owns no SIG,” “transaction 2 remains,” and “an iterative driver remains” are no longer current. There is still no `recognize_native_stems`: native carriage of the reconstructed Allegretto predecessor and wider linked-S/hook-removal coverage, wider-corpus STUMPS authority and branch coverage, general dirty state, and wider BEAMS-group coverage remain |
| STEMS SIDES-to-STUMPS entry | `continue_native_stems_beam_sides_carrier_into_stumps` is the twenty-first exact production boundary. It accepts only the explicit `SidesExhausted` terminal and walks chula system 1's 34 retained beams in Java STUMPS order. Beam SIG 12 starts Java event 0. Its stump 0 is both a structural side stump and already linked; Java's structural test wins at event 1. Stump 1 is unlinked, and plan 147 at `BEAM_SEED` profile 3 / link profile 1 reaches `AwaitingVLinkTransaction` at Java event 2 with two relations, one glyph, and no line change. Native returns two scheduler event records plus that typed frontier and stops before `createStem`; graph, B/S cells, and registries are unchanged. This real prefix contains no pure already-linked skip or known-false plan; a focused synthetic unit covers the linked-only guard without claiming production equivalence. The refreshed 10-line fixture retains the same five semantic rows plus summary and now has SHA-256 `ef8f180110a409f85167ee1cc0f641c210144d6e5b5c737d5d8eb69e82d47bcb` after its probe provenance changed for Boundary 28 |
| STEMS first STUMPS transaction and resume | `advance_native_stems_beam_stumps_transaction_from_first_stems_bridge` is the twenty-second exact production boundary. It atomically executes chula system 1's first stump frontier, beam SIG 12 / `beam:12:b:1` / plan 147, through B12-B17 and resumes the retained STUMPS worklist without Java's SIDES-only outer B18 assignment. Java reports glyph 310 `ReuseActive`, `CreatedChecked`, two `AllUnlinked` reads, final Stem Inter ID 2372, zero siblings, two heads, and `outerAssignment=false`. Native adds dense stem identity 32 and relation identity 331, reaches 254 vertices / 334 edges with 33 Stem bindings, 62 linked B cells, and 70 linked S cells, then skips two structural-and-linked side stumps and stops at worklist index 1, beam SIG 22 / `beam:22:b:1` / plan 622. The refreshed 11-line / 2,619-byte fixture retains six semantic rows plus summary and has SHA-256 `b1a312ddc690911b916971081ce21ea1c2211283df174a2175094ace7c144d5e`; its probe SHA-256 is `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf` |
| STEMS second STUMPS transaction and resume | Boundary 23 is second-frontier generalization evidence for the unchanged `advance_native_stems_beam_stumps_transaction_from_first_stems_bridge`, not a new production operation. A second call carries beam SIG 22 / `beam:22:b:1` / plan 622 through B12-B17 with no outer B18. Java reports glyph 321 `ReuseActive`, `CreatedChecked`, two `AllUnlinked` reads, final Stem Inter ID 2373, zero siblings, two heads, and `outerAssignment=false`. Native adds dense stem identity 33 and relation identity 334, reaching 255 vertices / 337 edges with 34 Stem bindings, 63 linked B cells, and 72 linked S cells. Resume skips structural-and-linked `beam:22:b:2` and `beam:16:b:0`, then stops at worklist index 2 on `beam:16:b:1` / plan 404, profile 3 / link profile 1, with two heads, last index 3, two relations, two glyphs, and no line change. The refreshed six-row-plus-summary fixture is 11 lines / 2,712 bytes with SHA-256 `4e54cc848116597ad563fd9038e102a135ff606660775e09142c8c8564567173`; its probe SHA-256 is `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf` |
| STEMS third STUMPS transaction and resume | Boundary 24 is third-frontier compound-candidate evidence for the unchanged production carrier. Plan 404 on beam SIG 16 / `beam:16:b:1` / TOP combines Java glyph IDs 303 and 2156; their union equals active modeled glyph 303 at ordinal 972, so registration is `ReuseActive` with no registry or allocator change. Java returns `CreatedChecked` Stem Inter 2374 after two `AllUnlinked` reads, writes zero siblings and two heads, uses base edge 337, marks B linked, and records `outerAssignment=false`. Native adds dense stem identity 34, reaches 256 vertices / 340 edges with 35 Stem bindings, 64 linked B cells, and 74 linked S cells. Resume skips structural-and-linked `beam:16:b:2` and `beam:28:b:0`, then stops at worklist index 3 on `beam:28:b:1` / plan 508; profile 3 / link profile 1 yields two heads, last index 3, two relations, two glyphs, and no line change. The separate six-row-plus-summary fixture is 11 lines / 2,709 bytes with SHA-256 `e7409462ec43f5cde89ffdeafb0c5bb59586c37fff1506086d9c5fa770b30490`; probe, runner, emitted-body, and semantic-pass SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `f2a41ca0069873274e443c978d0e84d56c49d67fa3387ef06346995dd2d587c1`, `3e66a99fe44495915fbb8c15f7285a7c9a5ae4340df60b7766968c3e214a1bc7`, and `ee1acaf3b1742346913ce3e9ed32430d3a4b24277537f0ed8e941d530ee6935b`. The refreshed linked-S fixture SHA-256 is `287175a58717874882bc6487f7d59ea86a22e44cadcac003ee99a36606e5ab34`. Boundary 25 below closes the remaining chula-system-1 STUMPS worklist; Boundary 26 then covers one reconstructed-predecessor competing-hook removal, while native Allegretto predecessor carriage, other systems, and full STEMS completion remain open |
| STEMS bounded STUMPS completion | `drive_native_stems_beam_stumps_from_first_stems_bridge` is the twenty-fifth exact production boundary. It repeats the validated one-frontier transaction on a shadow carrier, commits the whole batch only at a positive caller limit or typed post-STUMPS completion, and rolls back all earlier shadow transactions if a later frontier fails. From Boundary 24's plan-508 frontier, chula system 1 executes the remaining plans 508, 28, 330, and 251. Java reports glyphs 308/305/302/300, `ReuseActive`, `CreatedChecked` Stem Inter IDs 2375-2378, `AllUnlinked` reads 2/2/3/2, base edges 340/343/346/350, zero siblings, and head counts 2/2/3/2. Native uses dense stem identities 35-38 and finishes all seven STUMPS transactions after 92 scheduler events at 260 vertices / 353 edges, 39 Stem bindings, 68 linked B cells, and 83 linked S cells. A one-transaction limit commits only plan 508 and returns plan 28; zero rejects unchanged; a missing later `beam:32:b:1` cell rolls the entire batch back. The 87-line / 19,184-byte fixture contains 82 semantic rows plus summary, SHA-256 `81fecf842495ddc93792b0ed5acf5641231181f172acd4e5cbf3bc57565f0cd2`; probe, runner, emitted-body, and semantic-pass SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `2c6f9aaf39ae8ec2420104f15a3f6a2784f4eb4f229b0b23a7963ab5aade5717`, `946c160f4759ee3edb093c3cc1e5394965409f64e1b516b1ebcbbbfe009f49e4`, and `a629a2d63d223f28264c3fdc4dc20941e082402c27d75c2c6d884e2ce8282d08`. This completes only chula system 1's STUMPS worklist; wider-corpus branches, other systems, and full STEMS completion remain open |
| STEMS bounded competing-hook removal and resume | `remove_native_stems_beam_competing_hook_and_resume` is the twenty-sixth exact production boundary. From an explicitly reconstructed Allegretto-system-1 post-transaction-28 checkpoint—not native execution of transactions 1-27—it consumes the typed hook-removal frontier at Java event 64 / work index 19. Full Beam SIG 25 has LEFT and RIGHT linked and competes with same-glyph BeamHook SIG 24. Java removes Inter 907 from the active SIG but leaves it attached to the SIG object and represented in InterIndex state; the three-member group `[21,24,25]` survives as `[21,25]`. Native tombstones vertex 56, removes its five incident Containment/BeamBeam/Exclusion/two BeamStem edges and active beam-source binding, preserves the worklist and linked-B set, and resumes the remaining work to `SidesExhausted`. Active graph counts move 202/232 to 201/227; Java reaches event 110, while native records 54 continuation events and ends with 143 internal events. Missing Exclusion evidence rejects atomically. The 32-line / 4,195-byte predecessor fixture has SHA-256 `d173f1c475245980cad02bbf4624987d787fb293e5419d21444729f18bf7c8f8`; the 9-line / 4,336-byte result fixture has SHA-256 `d4c5decf03eaab893c79b2cb7ebd0378f13ac019acc007a38718105c75eacc71`. Probe, runner, result-body, and semantic-pass SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `3b9e0e28c9c2de75266c676a880dfe636bef885591ce12ed832640b8c72dd845`, `52432167156b75e4754259ae6c2a634e87788f028e85e6ea14754859e12ccb1f`, and `2cc4ad8e0aadf29b8055ce34c32b703c033c45880bef24ff26a707b6b6f0d3c5`. Native Allegretto predecessor carriage, hook-removal coverage beyond this checkpoint, other systems, and full STEMS completion remain open |
| STEMS first post-STUMPS head-phase frontier | `begin_native_stems_head_linking_phase1` is the twenty-seventh exact production boundary. It accepts only the native chula-system-1 `Completed` beam carrier, validates system/binding identity, Java's stable reverse-grade permutation, all 102 live graded head bindings, and the exhaustive duplicate-free S-cell topology with observer order, then clones the complete 260/353 carrier without mutation. Head order 0 is SIG ordinal 45 / Java Inter 1375, grade bits `0x3fe917c3b8207578`; STRICT stem profile 0, link profile 1, and `append=false` begin with empty unlinked/undefined collections. LEFT is open/unlinked with TOP/BOTTOM false/false; RIGHT is open/unlinked with true/false, selecting `h:38:RIGHT:TOP` and returning `AwaitingHeadCLinkTransaction`. Missing or mismatched terminal/system/binding/order/head/S-cell/builder evidence fails closed; dual-corner choice, close-head/gap recursion, no-link retry, and every `CLinker.link` mutation remain outside this boundary. Its fixture, expanded through Boundary 32, is 16 lines / 12,880 bytes with eleven semantic rows plus summary, SHA-256 `91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`; probe, runner, emitted-body, and semantic-pass SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`, `dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and `e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398`. Boundary 28 below consumes that selected frontier; later head iteration and retries remain outside this read-only transfer |
| STEMS first head-origin C-link transaction | `advance_native_stems_head_single_item_c_link` is the twenty-eighth exact production boundary. It atomically consumes Boundary 27's authenticated `AwaitingHeadCLinkTransaction` for `h:38:RIGHT:TOP`. The bounded nonrecursive builder contains exactly one `StartHeadHalfLinker` at `lastIndex=maxIndex=0`; glyph 307 is strong and active, so `ReuseActive` leaves registry counts/hashes unchanged, and the production path accepts only `CreatedChecked`. Native creates dense Stem identity 39 / Java Inter ID 2379, moves the compact graph from 260/353 to 261/354, grows Stem bindings 39 to 40, and advances the persistent allocator from 2378 to 2379. The HeadStem relation links the RIGHT S cell and updates the queued per-head cache coherently, taking linked S cells 83 to 84 with zero closed-cell changes. The carrier commits at `current_index=1` with `frontier_consumed=true` and stops before processing head index 1. Late or corrupt glyph authority rejects atomically. Multi-item expansion, recursion, dispositions other than `CreatedChecked`, `reuseStem`, duplicate relations, outer head iteration, rather-good retry/no-link closure, phase-2 append, and recursive tail linking remain outside this boundary. The expanded 16-line / 12,880-byte fixture contains eleven semantic rows plus summary, SHA-256 `91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`; probe, runner, emitted-body, and semantic-pass SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`, `dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and `e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398` |
| STEMS first two prelinked-head continuations and closure | `continue_native_stems_head_linking_phase1` is the twenty-ninth exact production boundary. Starting from Boundary 28's committed `current_index=1`, two calls revalidate the completed carrier, stable reverse-grade queue, live head bindings, and exhaustive persistent S-cell topology. Head order 1 (x90 / SIG ordinal 23 / Java Inter 1331) succeeds through its already-linked LEFT side, both open RIGHT STRICT corners are false, and Java's shared-stem closure writes both x89 S cells false-to-true: two ordered writes and two value changes. Head order 2 (x81 / SIG ordinal 33 / Java Inter 1351) follows the same prelinked-success path and closes both sides of x79 then x80 in SIG order: four ordered writes and four value changes. Both return true, add no unlinked head, and leave SIG/glyph/stem/allocator/link state unchanged apart from the closed S cells. Native reaches `current_index=3`, `frontier_consumed=true`, before x20 / SIG ordinal 65. Missing closure topology or invalid consumed-frontier state rejects atomically. Later queue entries, a later C-link mutation, an actually unlinked head and rather-good retry/no-link closure, phase-2 append, and broader head branches remain open. The expanded schema-v6 fixture is 16 lines / 12,880 bytes with eleven semantic rows plus summary, SHA-256 `91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`; probe, runner, emitted-body, and semantic-pass SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`, `dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and `e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398` |
| STEMS third prelinked-head continuation and closure | Boundary 30 calls the unchanged `continue_native_stems_head_linking_phase1` for head order 3 (x20 / SIG ordinal 65 / Java Inter 1419). LEFT is prelinked and both open RIGHT STRICT corners are false, so Java returns true and shared Stem 2361 closes x19 LEFT then RIGHT: two ordered false-to-true writes with no unlinked-head insertion. Native reaches `current_index=4`, `frontier_consumed=true`, before x36 / SIG ordinal 69 / Java Inter 1427. Graph, registry, stem, allocator, relation, and linked state remain unchanged apart from those two closed S cells; missing closure topology rejects atomically. This is one further prelinked-success case, not full head iteration or retry coverage. The expanded schema-v6 fixture is 16 lines / 12,880 bytes with eleven semantic rows plus summary, SHA-256 `91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`; probe, runner, emitted-body, and semantic-pass SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`, `dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and `e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398` |
| STEMS fourth prelinked-head continuation and closure | Boundary 31 calls the unchanged `continue_native_stems_head_linking_phase1` for head order 4 (x36 / SIG ordinal 69 / Java Inter 1427, grade bits `0x3fe8e37718100f0c`). LEFT is prelinked and both open RIGHT STRICT corners are false, so Java returns true and shared Stem 2369 closes x35 LEFT then RIGHT: two ordered false-to-true writes with `closedValueChanges=2` and `unlinkedCount=0`. Native reaches `current_index=5`, `frontier_consumed=true`, before x99 / SIG ordinal 61 / Java Inter 1411, grade bits `0x3fe8b9e1faa76070`. Graph, registry, stem, allocator, relation, and linked state remain unchanged apart from those two closed S cells; missing closure topology rejects atomically. This is one further prelinked-success case, not full head iteration, a later C-link mutation, actually-unlinked/retry coverage, phase-2 append, or broader head-branch coverage. The expanded schema-v6 fixture is 16 lines / 12,880 bytes with eleven semantic rows plus summary, SHA-256 `91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`; probe, runner, emitted-body, and semantic-pass SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`, `dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and `e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398` |
| STEMS fifth prelinked-head continuation and closure | Boundary 32 calls the unchanged `continue_native_stems_head_linking_phase1` for head order 5 (x99 / SIG ordinal 61 / Java Inter 1411). Java returns true through the prelinked-success path and shared Stem 2365 closes x98 LEFT then RIGHT: two ordered false-to-true writes with no unlinked-head insertion. Native reaches `current_index=6`, `frontier_consumed=true`, before x22 / SIG ordinal 12 / Java Inter 1309. Graph, registry, stem, allocator, relation, and linked state remain unchanged apart from those two closed S cells; missing closure topology rejects atomically. This is one further prelinked-success case, not the remaining queue, a later C-link mutation, actually-unlinked/retry behavior, phase-2 append, or broader head-branch coverage. The expanded schema-v6 fixture is 16 lines / 12,880 bytes with eleven semantic rows plus summary, SHA-256 `91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`; probe, runner, emitted-body, and semantic-pass SHA-256 are `d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`, `8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`, `dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and `e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398` |
| STEMS Boundary 16 computes the sibling writes itself | `boundary_sixteen_derives_the_sibling_writes_the_pass_recorded` first proved the production B16 result on chula transaction 1: `beam:0:b:0` and `beam:1:b:0`. The full native SIDES carrier now extends that proof through all 32 transactions and independently produces all 29 recorded sibling writes; carriage is no longer the open gap |
| STEMS chain: fed Java's sibling writes, it stops where Java stops (historical control) | `the_chain_fed_javas_sibling_writes_stops_where_java_stops` remains the expected-fed control that exposed the resume-order defect: completed-transaction B16 writes must be folded before walking forward. The current production gate no longer feeds those writes; it computes them and matches the same 32-transaction terminal after return |
| STEMS SIDES pass: the skip model closes exactly | Measuring Java's whole pass rather than its frontier settles what a self-driving chain is missing. `StemsBeamSidesLoopProbe` now records, per transaction, which *other* B linkers `linkSiblings` left linked, plus the set already linked before the pass. On chula system 1: **32 transactions execute, 21 sides are skipped through `BLinker.link`'s `isLinked()` early return, and all 21 were linked by an earlier transaction's `linkSiblings`** -- with zero overlap between executed and sibling-linked B linkers. 32 + 21 = 53, exactly what the synthetic chain runs when it skips nothing, so the gap is fully accounted. **Nothing is linked before the pass begins**, so this is entirely tracked state and needs no bootstrap -- unlike the SIG and the GlyphIndex. That last point cost two wrong intermediate answers: sampling the baseline at the first *counted* transaction reported 3 already-linked linkers, and moving it one earlier still reported 2, because the probe's fresh-transaction counter does not count the first transaction and its sibling writes were therefore never recorded. Both figures were instrumentation artifacts; the true baseline is empty and the handoff's original account was right. `the_sides_pass_accounts_for_every_skipped_side` asserts the closure, so a regenerated fixture that breaks it fails loudly. Frozen for chula system 1 under two byte-identical fresh JVM passes; the remaining systems come with the boundary that consumes it |
| STEMS SIG bootstrap evidence (historical, production replacement landed) | The ordered 221-vertex / 202-edge chula-system-1 snapshot proved that one global JGraphT order determines every incident scan: filter global edges to a vertex, incoming before outgoing. Production now rebuilds those same structural hashes from GRID through HEADS and carries native SIDES mutations to a 253-vertex / 331-edge terminal (last edge ID 330), so the snapshot is an oracle gate rather than the current architecture. The terminal counts are native invariants; the frozen SIDES oracle grades scheduling and sibling writes, not a final Java graph hash. Broader systems still need complete BEAMS-group products |
| STEMS second-frontier transaction (fast evidence) | Boundary 20 executes the second frontier's full transaction. The probe (`StemsBeamSecondTransactionProbe`) runs the boundary 12-18 chain at frontier 2 in fresh-evidence mode with every frozen-frontier-1 join disabled and every predecessor hash self-computed from its own emitted rows; per rust/PORTING.md replay-on-frozen, every fresh emitter (create, reuse, base, flag, sibling) must reproduce the frozen first-transaction rows byte-for-byte in the same JVM run before the txn2 rows are written, which surfaced and fixed five lineage hash divergences (b12 interIndex/sig/systemStems, b13 relationState/linkerState token enrichment). The runner splits stdout into eight per-family `stems-beam-txn2-*` fixtures per page after recovering the txn1 head/outer/resume rows as order-preserving subsequences that must match their frozen fixtures. The Rust gate patches the native scheduler to the Boundary-19 second frontier and replays boundaries 12-15 through the production applies against the txn2 evidence (the full typed b14 comparison included), then applies Boundary 18 and matches the txn2 outer rows; chula+Bach, nine systems. The checkpoint has since paid this down: all eight sheets are frozen under two byte-identical passes, and `boundaries_sixteen_and_seventeen_replay_at_the_second_frontier` gives boundaries 16 and 17 their full typed production replay at frontier 2, reusing the first frontier's parsers through `parse_core_transactions` so both frontiers are held to one implementation |

`recognize_native_beams` consumes the GRID report and the `HeaderErase` list
returned by `recognize_native_headers`: it measures `maxStem`, runs the spot
chain, dispatches by system areas/bounds, then creates, extends, hooks and groups
beams. The corpus now calls that production HEADERS entry point before reading
`clef-headers.txt`; the Java file is solely a grader. Missing staff geometry,
ordinates, system areas, or nested visual failures are explicit typed errors
rather than zero or empty fallbacks.

`recognize_native_ledgers` now consumes that native BEAMS result plus GRID's
`NO_STAFF`, curved per-staff lines/areas, and system areas/bounds. It preserves
Java's distinct beam contracts: every beam/hook participates in the early
section purge, while only good full `BeamInter`s participate in the later
filament-middle purge. On chula, the native builder reproduced every Java inter
exposed by the first comparison exactly, including their seven impacts. A
compact LEDGERS-only probe corrected that incomplete result: the general SIG
probe output had been truncated to system 3, hiding nine earlier final inters.
The full Java path has 19 builder survivors; `LedgersPostAnalysis` computes
sheet-wide unbiased delta/height populations, rejects one outlier, removes its
filament, and rebuilds system 1, leaving 18 final inters. Rust now reproduces
all 18 exactly. The widened order-independent count/digest gate now covers all
eight beam sheets, all 581 final inters, and all 95 inferred ledger-line paths
exactly. Chula's Java `LedgersStep.Context.sectionMap` has 2,042, 591, and 961
filtered section references in systems 1–3; the corpus assertion now reflects
`SystemInfo.getRight()`'s inclusive extra column instead of the older Rust-only
2,039/577/961 counts. `beams_json` and `ledgers_json` now publish the downstream geometry,
grades, complete impact vectors, live ledger exclusions, group counts, and
curved inferred ledger paths without changing GRID's schema-1 byte path. The
serializers and native stage inputs are wired through
`audiveris-cli -batch -step LEDGERS -json <image>`; a corpus beyond the examples
remains.

`cargo fmt --all --check`, strict Clippy, and `cargo test --workspace` are green
locally under the pinned toolchain. The full suite includes several independent
eight-page image-pipeline differentials, including complete HEADS and the exact
semantic STEMS boundaries described below. The seventeenth beam-V head-links
boundary has 24/24 focused production units and 13/13 full exact integration
tests; the 30-system plus isolated supplemental gate finishes in 148.82 seconds,
and the standalone manifest validator passes 1/1 in 129.11 seconds. The full
library suite is 676 passed / 0 failed / 2 ignored in 12.18 seconds.
Strict library/gate Clippy, global formatting, diff-check, and oracle shell syntax
checks are green.

The accepted STEM_SEEDS boundary is now native and exact.
`StemSeedsProbe` reaches HEADERS, installs the production `StemScaler` result,
and reflectively invokes only `VerticalsBuilder.retrieveCandidates()` -- never
`checkVerticals()`. `oracle/stem-seeds.txt` records exact system inputs and all
2,425 raw `StickFactory` candidates across eight sheets and 30 systems, including
mixed-orientation member order, bounds, weight, endpoints, thickness, and mean
distance with hexadecimal doubles. `recognize_raw_stem_seed_candidates` now
reproduces all 30 input section vectors by full digest and all 2,425 candidates
bit-for-bit from completed GRID state. It retains GRID's original vertical lag
and rebuilt NO_STAFF horizontal lag, dispatches original sections with Java's
system-area/bounds tests, and ports vertical thickening and opposite stickers.
One Java mutation was load-bearing: a side section accepted during thickening is
marked processed but is not assigned the filament compound link; only cores and
stickers receive that link. Two pinned-JDK oracle runs were byte-identical; the
corpus FNV is `d6ac0c99a5093beb` and fixture SHA-256 is
`c2ae9a9fe6a593072ede7f98de9073346ff006ccf48a8d5016c58ed4899cebd0`.
`recognize_native_stem_seeds` now continues from that raw result and
oracle-free HEADERS state through Java's closest-staff selection, header and
tablature gates, concrete `StemChecker`, original fixed-glyph registration,
minimum-grade gate, `VERTICAL_SEED` grouping, and system free-glyph ownership.
`StemSeedGlyphsProbe` invokes the actual private
`checkVerticals(Collection)`, captures the private `SeedCheckSuite` values,
normalized impacts, weights, Clean side effects, grades, and threshold, then
reads the actual free glyphs and hashes their complete cropped run tables.
Across the same corpus, all 422 header skips, 2,003 checks, 97 rejects, and
1,906 accepted/materialized glyphs match. The gate checks decisions, all seven
raw values/weights/impacts, Clean black/white/gap counts, bit-exact aggregate
grades, glyph geometry, run counts, and run-table digests. Two full Java runs
were byte-identical; the corpus FNV is
`541b1354720a0d35` and fixture SHA-256 is
`2e0455b7985a4e9fe68da25a020a0d1fc9f9e2161e6f1a5025e3c69dd1624953`.
Only profile 1 is exercised, with no tablature or no-staff skip case. These two
fixtures now grade raw factory output and accepted materialization separately.
The aggregate grade initially differed by one ULP even though every input and
impact matched. Java's weighted geometric mean follows OpenJDK fdlibm while
Rust's platform `pow` does not. `audiveris-core::java_math::java_positive_pow`
is a narrowed direct port for non-negative bases and finite exponents; it makes
all 2,003 grades bit-exact and carries a frozen residual regression.
`audiveris-cli -batch -step STEM_SEEDS -json` exposes the accepted free glyphs
in production `{system, ordinal}` order. It publishes geometry, grade, all
checker values/weights/impacts/counts, materialization indices, run count, and
a 16-digit run-table digest without inventing Java's process-global glyph IDs.
`native_stem_seeds_for_beams` now validates exact system order, accepted
decision/free-glyph identity, unique raw ordinals, group/free flags, and bounds
before mapping the glyph bounds and exact start/stop median to BEAMS'
`ExtensionGlyph`. `recognize_native_beams_with_stem_seeds` preserves Java's
create -> extend with per-system seeds -> hooks -> groups order. All 1,906
accepted glyphs cross that boundary on the eight-sheet gate. The compatibility
entry point remains explicitly seed-free, while the BEAMS/LEDGERS CLI uses the
composed entry point and carries stem scale plus accepted seeds forward.
`BeamStemSeedsProbe` independently prices this dependency inside Java. Each
page and mode runs in a fresh JVM, reaches the real STEM_SEEDS step, then runs
BEAMS either untouched or after hiding only `VERTICAL_SEED` free-glyph
visibility. The two states are byte-identical for all 30 systems: 1,906 input
seeds, 803 final beam/hook inters, 493 groups, one multiple rest, zero changed
records. Two complete passes have state-row SHA-256
`acca06864acfb212ea690b05987ab662668a2b2bf5fb6d4c86a26f32681fc6bf`;
`oracle/beam-stem-seeds.txt` has SHA-256
`283490cf3dc06afd7b65d3c8ca7c956b6e2b0372d43a0615edf89df469c8d785`.
The probe must snapshot every system before hiding anything because adjacent
systems can share the same registered Java `Glyph` object.

The acceptance branch is no longer merely wired: D039 is the natural positive
case. On `data/examples/D0392410-1.256.png`, system 2 has 76 accepted seed
inputs and replaces exactly one unextended beam when those seeds are visible;
the other three systems and every hook/group count remain unchanged. The Rust
regression compares the changed and replaced beam's median, height, grade, and
all six impacts to Java by exact `f64` bits. The focused paired oracle covers
465 seeds and 69 beams across four systems, has summary FNV
`5acbd8b3dd4d1405`, and fixture SHA-256
`991f3b4c56d4e9b5bb466657bffe931d6d0736daf759dd010964c82b01853f18`.
It found two one-ULP errors hidden by the original zero-change corpus:
intersection now preserves Java `LineUtil`'s determinant operation order, and
beam grading uses the OpenJDK-compatible positive `pow` implementation rather
than platform `powf`.

HEADS' first production dependency now survives the BEAMS boundary.
`NativeBeamRecognition.head_spot_runs` is the exact vertical RunTable Java
stores as `Picture.TableKey.HEAD_SPOTS`: the shared closed gray buffer is
thresholded at 170 and saved before the existing 140 BEAMS threshold. The
eight-page gate checks Java's table size and both the published run pixels and
an independently thresholded buffer digest from `oracle/beam-spots.txt`; all
are exact. This does not yet make HEADS native: persistent staff-line glyphs,
final ledger glyphs, and accepted vertical seed glyphs still need to be
composed into `NativeHeadsPrologRaster` before the visual classifier seam.

The ledger half of that raster handoff is now concrete.
`NativeLedgerRecognition.ledger_glyphs` is parallel to `ledgers()` in final
materializer insertion order and carries system/staff/inter/glyph/filament
identity, minimal absolute bounds, and the cropped fixed RunTable. Production
resolves each surviving glyph's exact `section_ids` against LEDGERS' filtered
horizontal sections, paints those pixels, and uses Java's orientation rule
(`width > height` is horizontal, otherwise vertical). Empty, missing, and
non-horizontal section sets are errors. HEADS can therefore erase the actual
ledger glyph pixels; reconstructing a band from the fitted median would be an
unmeasured approximation.

The complete production adapter now reaches the next honest HEADS boundary.
`native_heads::recognize_native_heads_prolog` accepts GRID, BEAMS, LEDGERS,
and STEM_SEEDS outputs, validates system order plus staff/ledger/seed ownership,
and constructs `NativeHeadsPrologRaster` without oracle inputs. GRID supplies
the original BINARY RunTable, persistent staff lines, and curved system areas;
BEAMS supplies retained HEAD_SPOTS; LEDGERS supplies the final fixed glyphs;
STEM_SEEDS supplies accepted free vertical glyphs. The result exposes the
post-erasure BINARY pixels, Chamfer-3 values, transient components in factory
order, and per-system zero-based spot ordinals after the area and inclusive
horizontal tests. A real Chula run composes the whole native chain and reaches
the `NoteHeadsBuilder` boundary. The independent Java prolog corpus comparison
is the next gate; template scanning and head interpretation remain beyond it.

That independent side is now frozen in `oracle/heads-prolog.txt`.
`HeadsPrologProbe` runs each of the eight beam pages through real Java LEDGERS
in its own fresh JVM, then calls `DistancesBuilder.buildDistances()` and
`HeadSpotsBuilder.getSpots()` in HEADS source order. It records exact upstream
paint inputs, the threshold-170 table, post-erasure BINARY mask, signed-i32
Chamfer table, every unsorted `GlyphFactory` component with its complete
cropped runs, and every production system-dispatch ordinal. Totals are 55
staves/275 line glyphs, 581 ledgers, 1,906 seeds, 2,790 components, and 30
systems. Two complete passes are byte-identical at SHA-256
`31e6166b0e2e8e7ae38909cca31d0a1709f8acc40f2812727509ea0bfb0a8422`.
The checked-in runner uses direct `javac` plus the saved runtime classpath to
avoid Gradle snapshotting the locally duplicated build outputs. The Rust
comparison is now exact as well.

The Rust gate drives the same eight pages through native GRID -> HEADERS ->
STEM_SEEDS -> BEAMS -> LEDGERS -> HEADS prolog and checks every threshold-170
table, post-erasure BINARY pixel, signed-i32 Chamfer value, component
bounds/weight/centroid/cropped runs, and all 3,097 dispatch references. The
only initial mismatch was Bach component 693 at center x=1916. Java includes
it because `SystemInfo.updateCoordinates` stores
`width = maxStaffRight - left + 1` and `getRight()` returns `left + width`, one
pixel beyond the greatest staff abscissa. The port had stored the staff extreme
correctly but several direct-getter consumers forgot the extra pixel.
`SystemBounds::java_right()` now names the distinction and is used by BEAMS,
LEDGERS, STEM_SEEDS, and HEADS; all old corpus gates remain exact. HEADS is
therefore graded through the real `NoteHeadsBuilder` boundary.

The next blocker is upstream state BEAMS currently discards. Java
`SpotsBuilder` runs `BlackHeadSizer`, which selects `Scale.MusicFontScale` and
ultimately `Staff.getHeadPointSize()`; `NoteHeadsBuilder` uses that point size
to select its `TemplateFactory` catalog. HEADS does **not** use the native MLP
classifier. The font-metric half is now native in `audiveris-music-font`:
Bravura `NOTEHEAD_BLACK` (`U+E0A4`) has exact arbitrary-point-size Java2D
outline widths, `MusicFont.computePointSize`'s two-sample secant and `Math.rint`
order are ported, and the sheet-to-staff point-size interpolation (including
the no-scale fallback) is explicit. Pinned Temurin 25 rows cover arbitrary
sizes, the complete interpolation, and its near-zero fallback.

The `black_head_sizer` kernel is now a production BEAMS side effect. It preserves
the two `checkSpot` passes, optional head-oriented close and component rebuild,
single/stack/unclassified decisions, discovery order, stable width sort at the
20-single quorum, `[n/4, 3n/4)` core, and Java-compatible unbiased populations
through `BlackHeadScale`. Its typed output deliberately does not invent Java's
process-global glyph ids, but retains exact source and sole closed components
even for post-close rejects. `recognize_native_beams` now runs it on the actual
threshold-140 components before system dispatch, derives the Bravura
`MusicFontScale`, and retains every GRID staff's selected head point size.

The independent half of that grade is frozen in
`oracle/black-head-sizer.txt`. `BlackHeadSizerProbe` reaches real STEM_SEEDS in
a fresh JVM per page, lets production `SpotsBuilder.buildSpots` install the
sizing side effect, then reflectively replays the private checks/close only to
explain every decision. Across eight pages it records all 2,739 threshold-140
inputs (distinct from 2,790 saved threshold-170 HEAD_SPOTS), 1,402 initial
accepts, 1,289 one-component closes, 113 zero-component closes, 936 singles,
5 stacks, 470 middle-half samples, eight sheet music-font sizes, and all 55
staff point sizes. Every input and closed component carries complete geometry,
centroid, run count, and cropped-run digest; populations use hexadecimal
doubles. Two complete fresh-JVM passes are byte-identical. The 4,767-line
fixture has SHA-256
`49408a3fc31857f107efb65ead37f63fd2e6dfe159f3fdd6215c89ed233199a9`.
The Rust comparison drives the same eight pages through native GRID -> HEADERS
-> BEAMS and matches every one of those candidate rows, source and closed
component run table, decision, discovery/sort/core order, population bit
pattern, sheet point size, and staff point size exactly. Totals remain 2,739
inputs, 936 singles, 5 stacks, 470 core samples, and 55 staves. Existing raw
beam, HEAD_SPOTS, LEDGERS, and HEADS-prolog gates remain unchanged; BEAMS now
retains rather than discards the state `NoteHeadsBuilder` uses to select its
catalog. The one-line-staff and drum-notation switches are not yet represented
in the production entry point and remain outside this eight-page grade.

The following HEADS dependency is already frozen independently in
`oracle/head-template-catalog.txt`. `HeadTemplateCatalogProbe` reaches real
HEADS in a fresh JVM per page, so every catalog comes from the actual
BEAMS-derived `Staff.getHeadPointSize()`. The eight pages use five point sizes
(`78`, `83`, `84`, `85`, `87`) and exactly four normal-staff scan shapes:
`NOTEHEAD_BLACK`, `NOTEHEAD_VOID`, `WHOLE_NOTE`, and `BREVE`. The fixture holds
55 staff-to-catalog usages, 32 page-local template records, 192 exact anchor
offsets/rounded offsets, and all 27,207 keyed `PixelDistance` records with
signed values and raw `f64` bits. Two fresh-JVM passes are byte-identical; the
27,606-line fixture SHA-256 is
`84c39208891530965f5d9ce71ff9b79cf373c101f4da8036059cdbf25e2a2ea6`.
Port the catalog representation and pure `Template.evaluate` consumer from
these records; do not regenerate templates approximately from a rendered font.

The dependency-light representation and evaluator are now native in
`head_template`. The four active shapes, nine anchor kinds, template/slim
geometry, ordered anchors, ordered signed `PixelDistance` keys, and four-record
factory-order catalog all have validated typed constructors. `getOffset`
preserves Java's asymmetric left/center/right `Math.round` expressions, and
`evaluate` preserves foreground/background/hole weights (`6/1/4`),
out-of-image and `VALUE_UNKNOWN` skips, zero-vs-nonzero comparison, accumulation
order, and `Double.MAX_VALUE` empty fallback. Malformed catalog data, tables,
coordinates, and anchors fail by typed error. Five focused tests use exact
point-size-84 Chula geometry, raw distances, and anchors. Font rasterization is
deliberately not guessed.

The complete frozen catalog is now production data as well. The deterministic
generator validates the fresh-JVM text oracle and deduplicates identical
`(Bravura, pointSize)` payloads into a versioned 105,021-byte checked-in binary
asset (SHA-256
`601a91da1359fc69633c496f2bc6860958e5dd3d861deb5495592beabf83077c`).
Production uses `include_bytes!` and a strict typed decoder; it does not parse
the oracle text or invoke a font rasterizer. Point sizes 78/83/84/85/87 provide
20 unique templates, 120 precise anchors, and 17,094 signed keys. The
integration gate intentionally expands the deduplication again and compares
all eight page catalogs: 32 templates, 192 anchors (precise bits and Java
rounding), and 27,207 key records are exact. The next slice is per-staff catalog
selection and scanner geometry. Per-staff selection is now production-wired:
HEADS owns the decoded catalogs once and retains a compact selection for every
non-tablature staff in system/staff order. It maps point-size records by owner
rather than assuming BEAMS vector order, validates owner/interline identity,
recomputes each size from the retained sheet music-font scale, rejects an
unpinned size instead of choosing a neighbor, and records the exact catalog
ordinal. All 55 graded staff selections compose transitively from the exact
BlackHeadSizer and catalog gates. Scanner-context construction is the next
boundary.

That boundary is independently frozen now in
`oracle/heads-scanner-context.txt` (SHA-256
`c137725c110755229c6b693410077b8c1933d7d70b63ed49dd7b3330a385d886`).
`HeadsScannerContextProbe` reaches real LEDGERS, runs the real HEADS prolog,
prepares `NoteHeadsBuilder` through its immutable system fields, and constructs
the private seed/range scanners without calling lookup or mutating the SIG.
Two all-fresh-JVM eight-page passes are byte-identical. The fixture covers 30
systems, 55 standard staves, 1,767 geometries (605 staff-line and 1,162 ledger)
and 3,534 schedules. It retains exact builder parameters; catalog selections;
x/y offsets; scanner shape order; staff/ledger source geometry; farther-ledger
axes; and complete theoretical-ordinate/range vectors as reversible RLE. The
probe asserts seed/range static geometry is identical. Competitor and frozen-bar
`Area` slicing is deliberately a later sub-gate rather than being implied by
this corpus.

The dependency-light scanner kernel and its fixture gate are now native.
`head_scanner.rs` derives Java's exact builder parameters, preserves the
`0,+1,-1,+2,-2...` stem window and directional ordinate windows, retains the
normal all/stem/hollow `EnumSet` order, decides open scanners, and implements
every `getTheoreticalOrdinate` branch over abstract staff-line and ledger
adapters with Java ties-to-even rounding. Five focused tests cover all three
frozen parameter regimes and every ordinate branch. The separate integration
test strictly parses all scanner-context row kinds, bit-checks Java doubles,
recomputes nested FNV summaries, expands RLE lengths, and validates all 8 pages,
30 systems, 55 staves, 1,767 geometries, and 3,534 schedules. This is an honest
kernel/fixture boundary: production staff/ledger adapters and their exhaustive
comparison are the next slice, and no scanner-context parity is claimed yet.

That production comparison is now complete for the oracle's operational
scanner geometry. `native_heads_scanner.rs` joins live GRID, HEADERS, LEDGERS,
STEM_SEEDS, and HEADS-prolog products; `head_scanner_geometry.rs` owns exact
persistent staff splines and ledger axes. The eight-page integration gate
matches all 1,767 contexts and 3,534 schedule rows: every parameter and ordered
offset/shape list, staff/ledger source, raw axis bit, farther-ledger list,
source/range bound (including four inverted empty ranges), and every expanded
theoretical ordinate. Ledger membership is read from
`LedgerMaterializer::staff_inter_ids`, preserving x order and legitimate
multi-index/staff reuse. The first implementation incorrectly used inter
creation identity. A second mismatch established that `Glyph.getStartPoint`
and `getStopPoint` use the cached uncentered `BasicLine.toDouble`, whereas
`Glyph.getCenterLine` extends and offsets the fit; Rust now exposes both exact
forms.

The same gate found an upstream boundary that is recorded rather than hidden:
Java gives each two-staff example system one brace-created Part, but production
Rust GRID passes `brace_search: None` and never runs its already separated
brace-continuation path, so its `bar_tail.parts` are singletons. Scanner Part
ID/range metadata is therefore not published or claimed. All 55 graded staves
are non-merged, so the operational merged decision and every resulting scanner
schedule are still exact. Port production brace detection before claiming Part
ownership or grading a merged grand staff. Tablature skips do not require a
header/Part join; drum and one-line scanners fail explicitly until the DrumSet
pitch-to-shape mapping is ported. Competitor/frozen-bar `Area` slicing remains
the next HEADS-local oracle.

That next immutable boundary is now frozen separately, without overstating the
dynamic lookup path. `HeadsScannerContextProbe --slices` reaches LEDGERS and
the real HEADS prolog in one fresh JVM per page, prepares the builder exactly
through the start of `buildHeads`, and records each seed-mode scanner before
lookup can create a head or mutate the SIG. Two complete passes are
byte-identical (SHA-256
`82d87324be1d2eef2a14be4c8cc68be332e9f76311eeb4b6dedd1c74d3c96ee3`).
The strict Rust gate validates 1,334 competing-shape candidates: 847 accepted,
29 not-good, 430 rejected by the vertical-width guard, and 28 rejected because
their beam group has no long member. It also freezes all 533 bar/connector
candidates, 474 frozen areas, three raw-bit semantic bands and ordered
seed/spot/competitor/bar rectangle slices for each of the 1,767 scanner
contexts. This deliberately does not serialize Java2D `Area` path internals.

The oracle also converts one previously documented BEAMS difference into a
hard HEADS dependency. Bach system 6's accepted competitor pool contains the
`MultipleRestInter` created after `BeamsBuilder`; the source beam is already
removed, while both generated vertical serifs are present but rejected as too
thin. Production Rust now performs that decision from real state. It rebuilds
Java's fresh BEAMS-time `StaffProjector` from original BINARY plus completed
persistent staff splines/thicknesses, applies the source-order length,
staff/tablature, endpoint-pitch, and two-serif gates, and retains an
identity-free descriptor alongside explicit pre- and post-replacement beam
lists. Bach's sole replacement is source ordinal 182 in system 6/staff 12;
its bounds, median, grade, height, pitches, and both peaks are pinned to the
frozen HEADS evidence. Group memberships remain pre-replacement because Java
groups first; LEDGERS now consumes post-replacement beams because Java deletes
the source before that step. Stable MultipleRest/serif/glyph/relation identities
and serif-glyph reconstruction remain graph-materialization work.

The first production-shaped slice geometry and two production pools are also
native. `JavaRectangle` ports positive-area half-open intersection, signed dimensions, and Java's
wrapping overflow behavior. `HeadScannerBand` builds staff-spline and ledger
bands with the source's explicit `below + 1`, resolves quadratic/cubic extrema
without polyline flattening, and intersects source-ordered vertical areas;
`VerticalRibbonArea` reproduces the integer bounds of the straight GRID
barline/connector ribbon. Production now builds every ordinate-sorted seed and
head-spot rectangle pool, all three bands, and the exact seed/spot slices for
all 1,767 contexts. The eight-page differential covers 1,906 seeds and 3,097
spots, with 1,455 nonempty seed slices/15,343 references and 1,455 nonempty spot
slices/6,759 references. A separate GRID adapter matches 528 source-order bar
and connector candidates and all 474 frozen obstacles by class, shape, staff,
raw median/thickness bits, stable ordinate order, and integer Area bounds. The
other five raw oracle candidates are unfrozen Hove dummy bars created later by
HEADERS, so they cannot enter the consumed frozen pool. Production now also
composes every scanner's semantic bar band with that frozen pool. The exhaustive
gate matches all 1,767 slices exactly: 552 are nonempty and retain 5,060 obstacle
references in Java's stable-by-ordinate order; `getBarAreas` performs no x sort.
The production competitor adapter now reconstructs all 1,334 current GRID,
BEAMS, MultipleRest, and serif candidates in Java SIG order, including exact
class/shape/staff/geometry, intrinsic and best grade bits, frozen state,
`isGood`, maximum-stem and minimum-beam thresholds, vertical floors, beam-group
member widths, and all decisions. It retains the same 847 acceptances and stable
ordinate order. The exact grade gate exposed the last one-ULP GRID seam:
`StaffVerticalImpacts` and bar-alignment impacts still called the platform
`powf`; both now reuse the port's OpenJDK-compatible positive-base `Math.pow`
kernel. Competitor slicing is exact across the same 1,767 scanner contexts: 408
slices are nonempty and retain 1,944 accepted-pool indices after semantic Area
intersection and Java's stable-by-abscissa sort. The fixture remains the base/pre-lookup
slice: range scanners later see seed-created `HeadInter`
instances, and actual template rectangles use Java2D area intersection. Those
decisions belong to the evaluation oracle, not this immutable constructor gate.

The pure template side of that next gate is now native. `HeadTemplate` exposes
Java's integer full and slim bounds at every anchor, including wrapping
subtraction and `java.awt.Rectangle.translate` overflow recovery; its main
evaluator now uses the same wrapping coordinates. Hole-only evaluation skips
out-of-image and `VALUE_UNKNOWN` cells, returns zero when no usable hole pixel
exists, and preserves the exact white/expected ratio. `Template.impactOf` and
the one-impact `HeadInter` grade/minimum constants are explicit, including
`GradeImpacts` clamping. Nine focused template tests pin anchors, overflow,
invalid distance state, exact ratio/grade bits, and all existing weighted
foreground/background/hole branches. Scanner orchestration and semantic
competitor/bar overlap remain the active boundary.

That active boundary now has a production oracle rather than a synthetic
fixture. `oracle/java/run-heads-seed-pass.sh` reaches real LEDGERS in one fresh
JVM per page, prepares `NoteHeadsBuilder`, and calls the real private
`processStaff(staff, true)` for every non-tablature staff in Java order. The
probe independently replays the private evaluation/create-inter calls to expose
provenance, restores the builder's performance counters before production, and
asserts the traced counters and predicted final heads against the real call.
After each staff it appends returned heads to `systemCompetitors` and performs
Java's stable ordinate sort; range lookup and later purges never run. The
default compact fixture hashes all 61,372 seed/side/shape searches, retains all
3,435 minimum-grade provisional candidates and all 3,435 final glyph-backed
heads, and records 55 staff, 30 system, and 8 page summaries. Two fresh-JVM
generations are byte-identical at 3,831,005 bytes and SHA-256
`aca3cd20941846ae0eab9b4c1e56b3c9959afb6ed649519888b854e2b68f0414`;
`--full-trace` emits the otherwise omitted per-search diagnostic rows. The
identity-free `kernelHash` deliberately normalizes every minimum-grade result
to `provisional`, so Rust can grade the pre-glyph kernel without claiming the
next glyph/SIG boundary.

That pre-glyph kernel is now exact. `head_template_overlap.rs` ports the
positive-area semantics of Java2D `Area.intersects(Rectangle2D)` for the
straight vertical ribbons and horizontal parallelograms used by the frozen bar
and competitor pools; edge/corner contact, slopes, reversed extents,
degenerate/non-finite paths, and double rectangle maxima are focused-tested.
`recognize_native_heads_seed_lookup` composes the prolog, scanner geometry,
catalogs, seed pools, frozen bars, competitors, and both ordered slice products.
It preserves seed/LEFT-then-RIGHT/shape/y-offset/x-offset order, exact stem-axis
intersection and `Math.rint`, first overlap gates, strict best replacement,
nominal abandon, black-to-void hole evaluation, minimum-grade construction,
and provisional tally dx. The exhaustive differential matches all 55 staff
`kernelHash` values, all 61,372 searches and outcome/performance partitions,
and every one of 3,435 provisional candidates by order, selected offset/pivot,
shape, pitch, bounds, distance, impact, and grade bits. Tablature staffs are
validated but omitted as in Java. The result deliberately stops before
`HeadInter.retrieveGlyph`, SIG insertion, and identity-keyed tally storage.

The glyph half of that boundary is now composed too. `head_glyph_retrieval.rs`
ports `HeadInter.retrieveGlyph`: derive the full template box from provisional
slim bounds with Java wrapping arithmetic, visit exact-zero template keys in
factory order against GRID's original BINARY pixels, return null if none are
foreground, crop inclusive point bounds, build a minimal vertical RunTable,
and replace the inter bounds with the positioned glyph bounds. The
`native_heads_seed_glyphs` compositor validates system/staff/catalog and
candidate/search provenance, preserves dense successful-head order, drops null
retrievals, and records the good-head side tally only after successful glyph
creation. The same eight-page differential proves all 3,435 provisional
candidates survive and matches every final Java row by order, provenance,
shape, selected distance, pitch, grade/impact bits, pre/post bounds, glyph
weight/run digest, good flag, and LEFT/RIGHT tally. Java's process-global glyph
and SIG IDs are intentionally not fabricated; range lookup is the next
algorithmic boundary.

That boundary is now frozen before its Rust implementation. The new
`oracle/java/run-heads-range-pass.sh` performs the real seed half, adds those
heads to the live ordinate-sorted competitor pool, and then calls the real
range half for every non-tablature staff. Its independent replay exposes the
scan/safety/shape/evaluation decisions, aggregation groups, dynamic seed-head
conflicts, and predicted glyph construction while restoring the builder's
counters before the production call and checking the resulting heads against
the live SIG insertion order. Across eight pages, 30 systems, and 55 staves it
records 6,759 ordered spot slices, 921,558 scan positions, 5,389 safety skips,
3,119,882 template attempts, 34,101 raw candidates, 3,550 post-aggregation
candidates, 3,376 seed conflicts, zero empty-glyph drops, and 174 final range
heads. The default 6,480,068-byte
fixture hashes and omits the three high-volume diagnostic row classes per staff;
`--full-trace` emits them. Two fresh-JVM compact generations are byte-identical
at SHA-256
`35a8d063d557979b9d5e948c279a6228c42ffd3fb5a7784d236779b490740770`,
and the emitted body hash is
`46e62aaafff97ca4c239c1dcd925308e0ebb706c67d4d7cab8f8669549f11a05`.
The active implementation boundary is the streaming range scan plus stable
grade aggregation and seed-conflict filtering; staff duplicate/overlap purge
still follows it.

The two list-level post-processing operations are now native independently of
the scanner. `head_range_postprocess.rs` reproduces Java's stable reverse
`Double.compare` grade sort, fixed first-member aggregate centers, first-group
and inclusive-`maxTemplateDx` choice, then filters aggregated mains against the
abscissa-sorted seed heads. Its rectangle path keeps `Rectangle.intersects`,
signed overflowing `int` area arithmetic in `GeoUtil.iou`, non-wrapping
`getMaxX`, first qualifying seed, early break, and inclusive 0.1 IoU and grade
margin. Nine adversarial tests cover equal grades and ordering, canonical NaN,
signed zero, exact thresholds, invalid rectangle dimensions, violated sort
preconditions, and overflow. The production range compositor now feeds this
kernel from every scanner and retrieves the 174 final glyphs.

The compact range fixture now grades that pure post-processing across the
whole corpus as far as its retained schema permits. The integration test
strictly parses and reconciles all rows and summaries, validates aggregate
ordinal/main/member ordering and the exact 0..34,100 raw-member partition, then
replays all 3,376 first seed conflicts and 174 retained candidates. Each first
conflict is checked by accumulated seed-head SIG provenance, bounds, grade, and
independently computed Java-overflow IoU bits. The fixture omits full curved
scanner-Area membership and nonqualifying seed heads; the test therefore builds
the exact decision-complete x-order from the exhaustive conflict evidence and
does not claim to reconstruct the raw aggregation input. A naive all-good
system pool was explicitly rejected after it created one false Bach conflict.

The production scanner half is now exact. `native_heads_range_lookup.rs`
streams Java's range search from the retained prolog/scanner/pool state: exact
`Rectangle.grow` spot intervals, Chamfer-3 safety checks and two-half-width
jumps, black-versus-hollow shape schedules, MIDDLE_LEFT y-only evaluation,
semantic bar/competitor overlap, strict best replacement, nominal abandonment,
black-to-void conversion, weak stemless gating, and provisional construction.
It retains only the 34,101 raw candidates while feeding canonical records into
four online FNV streams, so the 3,119,882 attempts do not become production
memory. The permanent eight-page gate matches all 55 staff spot, scan, attempt,
and raw-candidate hashes plus every scanner/count/performance/outcome partition:
6,759 retained spot rows, 921,558 x visits, 5,389 safety skips, 3,119,882 shape
attempts, and 34,101 raw candidates. Four inverted source ranges remain explicit
empty scanner invocations, as in Java. Seed-created `HeadInter`s are present in
Java's dynamic competitor pool but `Scanner.overlap` deliberately skips them;
they first matter at `filterSeedConflicts`.

`native_heads_range_glyphs.rs` now completes that composition. In system/staff
order it accumulates current and prior seed heads into the live competitor
pool, preserves stable ordinate order, intersects good competitors with each
scanner's exact retained curved semantic band, and stably sorts the slice by x.
It aggregates that scanner's raw candidates, records every qualifying seed
conflict with live/slice provenance, and retrieves surviving glyphs from the
original BINARY raster. The permanent eight-page differential matches all
3,550 compact candidates and aggregate main/member provenance, all 3,376
conflicts, zero empty-glyph drops, and all 174 final range heads by raw
source/attempt, shape, pitch, grade/impact bits, provisional/final bounds,
glyph weight/run digest, good decision, and dense order. No global Java glyph
or SIG ID is invented. The staff/system epilog below is now composed from these
live products rather than remaining an isolated frozen boundary.

The boundary after range glyphs is frozen in a separate deterministic oracle.
`oracle/java/run-heads-post-range.sh` manually follows the exact remainder of
`NoteHeadsBuilder.buildHeads` around real private production calls: combine and
full-abscissa-sort seed/range heads, remove duplicates, insert overlap
exclusions, purge discarded tally entries, apply the zero-valued stemless boost,
attach surviving notes, and run system small-beam arbitration. It then executes
the actual `HeadsStep.doEpilog` image discard and `HeadSeedTally.analyze`; HEADS
has no linking phase. Across eight pages it records 3,609 inputs, 62 duplicate
removals, 2,725 overlap exclusions, 3,547 post-duplicate staff heads, all 191
small-beam inputs, 26 ordered arbitration decisions, zero beam removals, 26
head removals, 3,521 final heads, and 18 analyzed scale rows. The default
4,076,279-byte fixture retains those decisions and survivors plus 1,451
identity-free live scale inputs. All 15,336 staff pair checks and 10,053 beam
checks remain count/hash committed; `--full-trace` exposes them. Two fresh-JVM
generations are byte-identical at SHA-256
`e893c2327a9afa937035559f1a5be170a22148dd6655e8ffb6297c75bff5f6ba`,
with emitted body SHA-256
`1420841aaeaafecb07664acbc26b752f3c7154fec073d863170c9ed77a1628f7`.

`head_seed_tally_analysis.rs` now ports the final sheet-scale computation over
that retained stream. It ignores removed heads, groups by Java `Shape` enum and
LEFT/RIGHT `EnumMap` order, preserves each insertion-ordered Population's
binary64 additions, and emits only buckets meeting the inclusive quorum of ten.
Four adversarial tests cover ordering, quorum, removed entries, non-associative
sums, and signed zero; the eight-page differential consumes all 1,451 samples
and matches every one of the 18 Java mean-dx raw bit patterns.

The common staff purge loop is now native as a pure decision kernel.
`head_purge.rs` stably applies Java's `Inters.byFullAbscissa` ordering with
wrapping comparator subtraction and relative ID tie order, positive-area
rectangle gates, wrapping inclusive xMax break, removed-state skips, and the
left-loop continuation when the left head loses. Duplicate mode performs true
removals; overlap mode records exclusion decisions without removing either
head. Equal grades use the strict `EPSILON` branch and reproduce
`purgedEquals`: prefer a head with seed tally, then shape/bounds identity, and
replicate complementary LEFT/RIGHT tallies only between two good identical
heads. Twelve adversarial tests include NaN, overflow, pre-removed inputs, and
multi-decision overlap behavior. `head_pair_predicates.rs` now supplies the
complete caller context: shape/bounds and full positioned RunTable identity for
`AbstractInter.isSameAs`/`Glyph.isIdentical`, plus staff reference identity,
`Math.rint` integer pitch, OpenJDK long-edge Rectangle intersection, inclusive
0.2/0.8 width gates, strict 0.25 area gate, wrapping products, and NaN behavior
for `HeadInter.overlaps`. Nine adversarial tests pin those semantics.

`head_small_beam_purge.rs` ports the system-level arbitration as a pure kernel.
It filters all Java beam shapes by the strict integer `minBeamWidth` gate,
preserves beam SIG order and stable head ordinate order, uses the exact filled
horizontal-parallelogram intersection, and reproduces wrapping beam bottoms,
strict contextual-grade comparison, NaN/equality head removal, and both
iterator-removal effects. The production adapter now supplies live competitor,
beam-group, MultipleRest, and head records. Contextual grades use Java's
coefficient 3 / ratio 4 support contribution, exclude the raw hook/beam pairs,
partition compatible partners in reverse-grade order, honor MultipleRest and
earlier arbitration removals, and are recomputed after each beam removal.

`heads_post_range_corpus.rs` parses the compact fixture into typed staff,
system, beam, head, and scale records. It validates both SHA-256 commitments,
all reconstructible FNV streams, hierarchy/count arithmetic, ordinals, tally
rows, and the identity-free staff-head to purged/final-head multiset. The live
compositor now recreates the deliberately compact initial-head and pair-check
streams and matches their committed summary hashes.

`compose_native_heads_staff_epilog` now combines the production seed and range
glyphs in Java staff order, applies the exact duplicate/overlap predicates,
purges duplicate tallies, and attaches survivors. `compose_native_heads_epilog`
then consumes that staff state plus the live competitor and BEAMS products for
system arbitration and sheet-wide tally analysis. The top-level eight-page gate
is exact for all 3,609 inputs, 62 duplicate removals, 2,725 overlap exclusions,
3,547 post-duplicate heads, 191 beam inputs, 10,053 ordered beam checks and
hashes, 26 head removals, 3,521 final heads, 1,451 tally inputs, and 18 scale
rows. `recognize_native_heads` now owns that full production chain, and the
eight-page gate calls it directly rather than manually recreating its stages.
BEAMS also retains the exact fixed vertical glyph Java rebuilds from `NO_STAFF`
for every final raw beam and hook; all 191 HEADS-consumed glyph bounds, weights,
and run digests match. `audiveris-cli -batch -step HEADS -json <image>` publishes
the complete identity-free result plus its upstream products. HEADS is therefore
native, graded, and published.

The first production STEMS slice now continues directly from that owned HEADS
result. `materialize_native_stems_head_corners` filters the live SIG-order heads
after beam removal, retains stable abscissa and reverse-grade permutations, and
resolves each head's actual staff-selected Bravura template. It reproduces the
four `HeadLinker.CLinker` constructor corners, template bounds and rounded
anchors, sheet head-seed side correction, and profile/interline inside/outside
limits without allocating Java inter or glyph IDs. `StemsHeadCornerProbe` runs
real HEADS in a fresh JVM for each of the eight pages and stops immediately
before `CLinker.retrieveStump()`. Two complete oracle runs were byte-identical;
the Rust differential matches 30 systems, 3,521 heads, and 14,084 corners row
for row, including every head/template/glyph field and raw double bit. The
probe source SHA-256 is
`4180de0596c3580fbef45ee12b6ec05f0dee17ef9e7267531e62efabb28d9c40`, the
body SHA-256 is
`485544ae74a08d2a4d5c2a0de0030db67eec0086bd370d4eb6e2680917d0572a`, and
the complete fixture SHA-256 is
`26f9fff81c6207957dab6f42bf7d1650682ae9fca5de46e7b9a7dc46f20fd94b`.
Existing-seed selection and no-stem purging are native and exact: the eight-page gate
matches 1,906 input seeds, 1,749 survivors, 157 purges, 483 no-stem areas,
29,394 purge visits, 36,736 neighbor rows, 7,114 sorted candidates, 7,005
visited candidates, 4,182 selections, and 9,902 explicit fallback requests.
The port derives `Glyph.getCenterLine()` from each fixed run table rather than
reusing the distinct start/stop line, and it uses `SystemInfo.getArea().getBounds()`
rather than staff extrema for the vicinity rectangle. The deterministic Java
fixture SHA-256 is
`19387924d0d7aaaabf07b0859b353c7fa8d3e3c5d10e8edec8e1d4287b1ace31`.
`materialize_native_stems_head_stumps` now closes the next mutating boundary.
It dispatches the complete system VLAG rather than the narrower seed-builder
section subset, preserves run-box intersection, stable bounds-center distance,
integer point containment, repeated pre-member width checks, and the signed
shift in `getSubSection()`. It paints exact tight fixed glyphs, registers before
the standout decision, and canonicalizes exact `(bounds, RunTable)` identities
without merging transient STUMP groups. The projected eight-page gate matches
18,398 section rows and compound steps, 3,660 subsection attempts, 969 empty
builds, and 8,933 candidates: 758 accepted, 8,175 rejected, 5,591 new, and 3,342
reused. The Java probe also freezes the preceding 803 BeamLinkers and five beam
side-stump registrations; those beam-side constructions are deliberately not
claimed by the current Rust product. The fixture SHA-256 is
`dd0247fbd992c7ec40351040efd336f98c8efa88bab0eef10c744430252e966e`.
`materialize_native_stems_beam_stumps` now closes the fourth production semantic
STEMS boundary. From the live post-HEADS beams/hooks, kept STEM_SEEDS product,
and complete per-system VLAG it reproduces constructor-time
`BeamLinker.retrieveStumps()`: seed-area geometry, stable cross-x ordering,
duplicate purge, LEFT/RIGHT side classification, full-VLAG missing-side builds,
direction gating, exact fixed-glyph registration/reuse, final stump and side
order, and the tremolo predicate. The exact gate spans eight pages and 30
systems: 803 constructors, 1,606 sides, 3,934 neighbors, 1,820 seed inputs, and
1,087 purge comparisons split into 5 removals and 1,082 breaks. It retains
1,305 side seeds and executes 301 builds: 4 empty-section results, 154 zero
compounds, and 143 candidates. Direction checking accepts 6 and rejects 137;
registration yields 5 new glyphs plus one canonical reuse. All 447 sections and
447 compound steps match, as do the 1,821 final stumps, 1,311 final side stumps,
and zero tremolos. Probe, runner, emitted-body, and complete-fixture SHA-256 are
`98c19499ca486fda8ddec92f18f9e3de54f27041987b011220babbf202dc0039`,
`08964909fa4b7f26ac12c451cfe3a40e4c1ec6cf7ecc2524a2fa11b959175679`,
`18e6431ad73d05f8a72eb1f8e82b8ab047279e2cdc54d0545d7acf3e6bab0899`, and
`902478763d2897eb0d3f031a0895bee7d91a5a7bf8acf8188bf752273e149f14`.
`materialize_native_stems_beam_vlinkers` closes the fifth production semantic
STEMS boundary. It replays sequential constructor visibility and exact
`equipStumps`/`equipOrphanSides` creation order, producing 1,821 stump plus 295
orphan BLinkers and 1,827 stump plus 590 orphan VLinkers. Every side map,
reference point, TOP/BOTTOM direction, stopping-head side, staff/Part limit
fold, raw lookup quadrilateral, theoretical line, closer-alien action and stable
sort key, chosen opposite-border rebuild, and final neighbor-seed decision is
retained. The eight-page row differential matches all 2,116 BLinkers, 2,417
VLinkers, 2,860 Part folds, 9,186 alien candidates (4,738 same-group drops, 38
bad beams, 501 hooks, 2,812 misses, 3 aligned-side drops, and 1,094 survivors),
703 limiter rebuilds, and 12,491 seed checks with 2,169 reachable. The fixture
is byte-identical across fresh JVMs at 46,946 lines / 18,307,148 bytes. Probe,
runner, emitted-body, and complete-fixture SHA-256 are
`fbc5dace791c84e82db5ff870fb4bcc23e06f29b54619865f19448c0f016a5c2`,
`38e723c15bec6d67c4b856fc40a40d3ee0e4835f466c0c917715c792e6fa1c75`,
`bd43baa197540107e27d2ac97098dbb9df6d6bea1003888ee3625c69e21e60bf`, and
`77cfa1f1d9b6e3f8917ff44db7e3f643ffca690bd639d8a5a93f6fea208a8388`.
The prerequisite live GRID path now retains detached brace filaments and drives
exact two-staff Part ownership; registered beam glyphs use pinned OpenJDK
`Area`/`Order1` crossings rather than a determinant shortcut. That constructor claim
ends before HeadLinkers and source-ordered `inspectVLinkers`; the next boundary
must preserve B-before-C reachability and cross-beam anchor reuse/append order.

`materialize_native_stems_beam_reachability` closes the sixth production
semantic STEMS boundary after all HeadLinkers exist. The global traversal covers
803 beam starts, 2,145 BLinker
visits, 29 anchor skips, and all 2,417 VLinkers. Its 4,960 sibling scans yield
1,617 eligible cross-beam searches and 5,354 candidate BLinker visits. Strict
first-tie and inclusive-threshold semantics reuse 1,472 BLinkers, including
215 anchors, while 145 new anchors grow the final arenas from 2,116 to 2,261
entries. The product retains immediate beam-end and final snapshots so later
backward appends remain distinct. `filterHeads` runs after beam mutation for
each VLinker: 158,886 stable scans produce 5,739 area hits, 46 distance drops,
11,386 corner checks, 531 void-side drops, and 5,059 accepted CLinkers, always
after the ordered B targets. The corpus has zero competing-head removals, small
heads, small beams, or size drops; those zeroes are graded and generalized
small-head support is not claimed. All 2,417 seed snapshots remain unchanged.
The signed-zero-corrected fixture is byte-identical across two fresh JVM runs at
232,460 lines /
61,411,164 bytes. Probe, runner, emitted-body, and complete-fixture SHA-256 are
`39ed0694f7c31593f157b5f250f8bfa4f006984e3b491a877903d64d810edd7b`,
`61801362bc7328cfb3e90f7460016e333d776ee964d39cc296f60cf6edac33f1`,
`470827ebc19065890c41c10016511e77eeefc851823bb8587f7537c7e7db23cf`, and
`9c3f6d17fa6806cba9b01f3922aca34a220d21dc1a5269723e151a025c693221`.

`materialize_native_stems_beam_builders` closes the seventh production semantic
STEMS boundary. It replays each beam-origin `VLinker.inspect(maxProfile)` once
in production order, reaches the actual `StemBuilder` constructor return, and
records its V `sb` assignment for all 2,417 builders. A builder recomputes its
direction from the theoretical line rather than inheriting the V direction; the
only divergence is Carmen system 2 / builder 56, leaving 1,390 TOP and 1,027
BOTTOM builders. The constructor removes 215 of 2,169 seeds, retains 1,954,
and reduces 6,676 targets to 6,670: all 1,617 B targets and 5,053 C targets.
It registers 1,442 chunk glyphs (799 new and 643 canonical reuse), removes 175
chunks, retains 9,419 final items, and emits 12,085 length rows. The bounded
registry records zero external members and zero unmodeled reuse; this is not a
global glyph-novelty claim. Its JDK 25 mini-TimSort-only audit records 18
comparator cycles and 2,503 equivalence inconsistencies. Target lists reach 11
items and final lists 14; an observed list length of 32 or greater fails closed.
All 35,419 builder checks record zero SIG, system-stem, linker, C-builder, and
unexpected-builder mutations. The emitted body is 91,211 lines / 29,195,732
bytes and the complete fixture 91,212 lines / 29,197,924 bytes. Probe, runner,
emitted-body, and complete-fixture SHA-256 are
`c320870ea130e5156124b111e34c918fa4f640595109ac44b8a4de89b732d178`,
`adc2647152b925a2a81fe580a240b4c8be05fca3148ef3d3df29d73577e72806`,
`da4226ee2227d6369054fbce2de4252c72347242253a335132883d9cf871bd22`, and
`a3708e0436184dac5aa63fdb43c70cf05252fa7dbbfd7e9a2d746082e22f2180`.

`materialize_native_stems_head_corner_reachability` closes the eighth
production semantic STEMS boundary. Across eight pages and 30 systems it
visits 3,521 standard black/void stem-capable heads and all 14,084 CLinker
corners in TR/BL/TL/BR inspection order. The native pass reproduces 36,736
ordered seed scans and assigns 1,340, compacts 1,007,081 ordered head scans into
4,566 C targets, and scans 9,015 sibling members into 8,120 B targets. It writes
all 14,084 C seed lists and appends 1,687 head-origin anchors. The final
B-linker algebra is exact: 2,116 constructor entries + 145 beam-origin anchors
+ 1,687 head-origin anchors = 3,948. Each result preserves C-before-B target
order.

The 16,501 builder checks cover the preceding 2,417 assigned V builders plus
all 14,084 still-null C builders. Forbidden SIG, link-state, and page-persistent
registry mutation counts are zero. Scope remains limited to the standard
black/void stem-capable heads in the corpus; small-head truncation is not
implemented. The reachability-only beam prefix intentionally omits prior beam
`StemBuilder` local/registry mutations because C reachability does not read
them. The ninth boundary resumes the actual registry timeline from the
beam-builder product. An audit of the replay sort confirmed
`BeamGroupInter.getMembers()` returns a fresh list; the hardened probe still
clones it and snapshots/asserts every group member identity/order throughout.

The fixture is 79,216 lines / 37,478,914 bytes. Probe, runner, emitted-body,
and fixture SHA-256 are
`7bac85a2e878d67ccecab9866428a8068b83d1453c2249f49b0c18ae6a17b39f`,
`e9016abb44a500e242b81364531b775fe6b724cddf697cfc0bd4cfe21af0f75d`,
`b3f10b53346adac1309d12fa2d245840a88b02c17e399e88d7e5e36f0358889b`, and
`537cae86c19de20af35a246e03b6edd7f324d0f08c5768b319ed0557a7e28921`.
The normal CI gate is green with two tests and zero ignored; its semantic run
completed in 33.18 seconds. The final native HEADS product now carries explicit
`is_vip = false` evidence from its
current creation path; consumers must handle or fail closed on any future true
value because Java's VIP-only `filterHeadParts` behavior is semantic.

`materialize_native_stems_head_builders` closes the ninth production semantic
STEMS boundary. It resumes the actual page registry instead of the eighth
boundary's reachability-only prefix: for each of 30 systems it replays stump
registrations, all real beam-origin constructor registrations, then every
head-origin C builder before moving to the next system. The bounded baseline is
structural and contains only live glyphs after MultipleRest replacement; it is
not a claim about unrelated entries or Java IDs in the global `GlyphIndex`.
Across the eight pages, 8,939 stump attempts yield 5,581 New / 3,358 Reuse,
1,442 beam registrations yield 796 New / 646 Reuse, and 19,295 head registrations
yield 4,619 New / 14,676 Reuse. The chronology matters: it exposes eight stump
action differences from isolated order and three later-beam action/reuse changes
caused by earlier head chunks.

All 14,084 C-origin builders materialize in head-x then TR/BL/TL/BR inspection
order, split evenly between 7,042 TOP and 7,042 BOTTOM, with no direction
divergence. The constructor scans 15,953,076 vertical and 14,436,784 horizontal
sections, accepts 34,526 and 23,787, builds 19,295 filaments from 45,938 members,
and retains 29,120 final items. Its 35,424 gap checks insert 165 gaps and record
6,469 truncations; the exact gate reproduces all 70,420 profile-0-through-4
lengths from the independent Java replay. Its 42,252 sort rows retain all 8
comparator cycles and 319 equivalence findings. The three frozen list maxima are
2 retrieve-seed, 7 target,
and 13 final items; production rejects any JDK sort input of 32 or more rather
than claiming the unported large-array merge path.

The corpus uses inspect profile 1 in every system and has zero profile
divergences; production rejects a page inspect profile that differs from the
effective system profile. It contains no VIP or small heads. The implementation
nevertheless retains Java's `filterHeadParts` bug exactly: all 6,087 chunks below
the remaining-weight threshold stay for non-VIP heads, whereas only VIP inputs
take the removal branch. The shared vertical `StickFactory` also preserves the
source's processed-without-compound distinction: a side accepted while
thickening is processed but has no compound link and can therefore be reused by
a later filament as an isolated sticker. SIG, `systemStems`, link state, and
unexpected-builder mutation counts are all zero. The seam ends after the C
linker's `sb` assignment, before `VLinker.expand`, `StemBuilder.createStem`, or
any relation/SIG mutation.

Two fresh-JVM corpus passes produced byte-identical split fixtures totaling
593,749 lines / 171,932,512 bytes. Manifest, probe, and runner SHA-256 are
`21d8d11beb4a8895759198f17a45a981a66f9554c9559d1711db09f3db7b764e`,
`364ad5d74f15c9cbaf77b67da987f6bc3a309c0bd5c80093f34185d6c4ceadd9`, and
`215410766e419685c6cf3a5c9c8f2c8e7ac39b0f02ef18780f4a67450ae91b37`.
The eight full fixture SHA-256 values, in manifest order (Chula, Allegretto,
Batuque, Carmen, Cucaracha, Hove, Zizi, Bach), are
`c001dd763ccd8849c6d95379d45ce15f94e6ce7d8bf364e7a9b408f072ff645c`,
`195a65e77f321aa45758d19e7448f7f1c1458918858a64099936e741d0a456b0`,
`8320a7b4e645620784d66f67ad7b8e5cee866c72a30e310102f4726542a498bf`,
`43d94ddb7af2ebd36c29dae70446b27189d9b045afcf4724d79566e2608ff03a`,
`87c8b9ba51361a777d0529fa8a397263cafce593bd552ddbbf1fe5408758ed21`,
`c098170dc32bc1773c1d5319a459cbf3b4ba93fa076626a78f2aaa9fbaffcbc4`,
`745b90cb61b637ab829c9495dd379709479fd7cfbe59b1cdfef73807523cac43`, and
`66b77dd58f4cf3ac3b8e3971695bb7aab953f95e44cd6ba69625efb7450aa6a6`.
The normal eight-page full native semantic-stream gate is green twice:
84.48 seconds in the independent run and 88.93 seconds in root verification.
Strict integration-test Clippy is also green. That ninth seam ends before the
beam-origin `VLinker.expand`/link-plan prefix described next; persistent linking
and SIG mutation remain separate later boundaries.

`materialize_native_stems_beam_link_plans` closes that tenth production
semantic STEMS boundary. It starts from the immutable, completed beam- and
head-origin builder products and evaluates each inspected non-anchor beam V
builder independently for `stemProfile=0..constructionMax`: 0 through 3 for a
center/stump BLinker and 0 through 4 for a side BLinker. `linkProfile` is the
effective system profile (1 throughout the frozen corpus). Outcome precedence
is source-faithful: `NoHeadTarget`, `ExpandFailed`, `NoRelations`, `NoGlyphs`,
then `ReadyForCreateStem`. The boundary returns immediately before
`StemBuilder.createStem`; it does not select a live scheduler attempt and does
not allocate or mutate a stem, GlyphIndex, `systemStems`, relation graph, or
link flags.

The exact eight-page / 30-system gate covers all 2,417 builders and 11,573
profile plans. Outcomes split into 2,903 `NoHeadTarget`, 289 `ExpandFailed`, 2
`NoRelations`, 58 `NoGlyphs`, and 8,321 `ReadyForCreateStem`; final products
retain 18,345 ordered relations and 12,523 structurally keyed Glyph entries.
The replay grades 578 gap rows (289 fail / 192 rewind / 97 continue), 9,869
separation rows (2 rewind), 18,416 relation attempts (18,345 accepted / 71
rejected), and 37,683 Glyph updates (12,582 insertions / 23,965 content-equal
skips / 1,136 null-Glyph calls). Dynamic `HeadStemRelation` side derivation
disagrees with the encountered C corner twice, both on rejected Chula attempts.

Two Java asymmetries are now explicit product data rather than hidden side
effects. Downward expansion aliases the local working line to the VLinker's
stored `theoLine`, its `StemBuilder` line, and, when current, the beam's
`theo-<B id>` attachment. The corpus has 3,226 such shared-line mutations, all
mirrored by the current attachment; the immutable Rust matrix emits exact
before/after bits and deltas for a later serial scheduler to apply. Gap or
separation rewind restores the returned Glyph set and index but not the local
line or relation map. No corpus relation lies beyond the returned index, but 49
gap rewinds retain a bit-different line after Glyph restoration (maximum
residual `0x1.0p-39`). Forbidden graph, index, linker, builder, and predecessor
mutations are all zero.

The source's `BEAM_SIDE` terminal contract is also measured rather than
normalized. Its javadoc requires a correct-side ending head, yet `expand`
returns `maxIndex` after ordinary exhaustion. Of 1,286 ready profile-4 plans, 9
have no stopping head, 632 return beyond their last valid stopping head, and
645 return at it. Compatibility mode preserves that 9 / 632 / 645 partition;
the maintainer-facing catalog records both this mismatch and the shared-line
alias/path-dependence risk.

The eight split page fixtures total 120,724 lines / 104,056,316 bytes. Their
combined emitted body contains 120,646 lines / 104,048,204 bytes with SHA-256
`ac0fcb9880dbf720c8b73e6baf02867d05e0f2d5a62f208f52e9fa7d5c764966`.
Manifest, probe, runner, and manifest-body SHA-256 are
`f511b049cf5e32de6fb0151a36a1385efb78b4965fd704c7545eaef8522a2f87`,
`2a5e107f947e140e030f3cc1dff06105ab730af3e41381e76f5f8113a17b0fa2`,
`a73ed3977662427062b8d81ac8796ffa54d51daa2f97ea1f109a3d606d0c13b7`, and
`a9430038e430c62e887cc1993bd695f20802f1921ebb753bf944c17ee714b304`.
The eight fixture SHA-256 values, in manifest order (Chula, Allegretto,
Batuque, Carmen, Cucaracha, Hove, Zizi, Bach), are
`78dc57b476a1dd87656e6ec3e2322a7c6734b3d83159ccf374042f6b269e6e06`,
`deae3d40c8161f1969b59c00a1ca94232b78e0a6617f1aa27fae74e9eefc190b`,
`9379eb2b076e180bf2ced42836fe7b9a030770659218b37b00ab8d3dbbd3f368`,
`8a8d23598b392b0517413d072bea674a3328b7462e3901df01273535044f7da4`,
`eaf7b87b4bb2eb69e5d30702b522db15a70837e19b9b6e6de3924a72a4bb1677`,
`15a4c7185bfd7b397f1993cc59857c0fcf42067ccd4e6cce10285a4d7766c123`,
`5db2b5fe7519e889ca020238db79f5ba7684491acd22edf4fd82e09ef555d660`, and
`7eecdd0f5924e17db08d6c40cbc01e383a2d6a1df2e83efd2592d4ce73eb8409`.
Eleven focused unit tests and all 120,646 body lines—120,636 semantic rows plus
the 10-line shared header—pass; independent and root full native runs completed
in 32.25 and 32.41 seconds, and strict integration-test Clippy is green.

`materialize_native_stems_beam_scheduler_frontiers` closes the eleventh
production semantic STEMS boundary. It resumes the live deterministic prefix
of `StemsRetriever.linkStems` independently for each system, before the first
operation whose result depends on persistent recognition state. The product
reconstructs beam/hook SIG order, exact page-global `(bounds, RunTable)` Glyph
identity, ordered live raw hook/full-beam Exclusions, and the first qualifying
identity-equal competing hook. It then uses Java's stable decreasing integer-
width sort (retaining SIG order on ties), LEFT then RIGHT side order, TOP then
BOTTOM V order, target precheck, and the exact side-profile rule: the system
profile for hooks or full beams with a competitor, otherwise `BEAM_SIDE` profile
4. Stump scheduling is represented too, including structural side-stump
containment, but no frozen system reaches it before its transaction frontier.

Across eight pages and 30 systems, the scheduler sees all 803 beams, 322
adjacent width ties, 651 page-global canonical live Glyph aliases, and 78 live
raw hook/full-beam Exclusions. It records 56 attempts. Twenty-six attempts are
empty-target precheck skips and invoke no isolated plan; they make 14 beams fail
side linking and disappear only from the local scheduler worklists, not from
the live SIG. The next invoked plan in every system is one of 30
`ReadyForCreateStem` plans, so all 30 systems stop at a typed
`AwaitingVLinkTransaction` in the side pass. `ReadyForCreateStem` means only
that the frozen expand/link prefix reached the call site: it is not a successful
V link and does not predict the result of `createStem` or later relation work.

Fourteen of the awaited downward calls carry a pending stored-theoretical-line
and current-attachment delta. The frontier publishes each exact delta but
applies none. No known-false plan is invoked on this corpus, so there are zero
already-deferred known-false line deltas; there are also zero stump rows,
`AwaitingHookRemovalTransaction` frontiers, shifted-V retries, or completed
systems. The typed product retains those distinctions and fails closed rather
than crossing a retry or competing-hook removal whose inputs would already have
changed. It performs zero `StemBuilder.createStem`, GlyphIndex registration,
`systemStems` insertion, SIG vertex/edge/removal, relation/link-flag, stored-line,
attachment, or other persistent mutation.

The eight split fixtures total 1,041 lines / 467,955 bytes. Their combined
emitted body is 998 lines / 460,651 bytes—993 semantic rows plus the five-line
shared header—with SHA-256
`8ff44c35d8c1e2334c56c4d7e546fdaacbcb2964a1ab6103168f25346e041ff1`.
Manifest, probe, runner, and manifest-body SHA-256 are
`b6b77cdead537a70b482ae7ef5d5c8312cc5993529382f1f39fb4779afa7abb2`,
`afb5c564a474bc0c227b9fdc886cf892c60ae39aa62c1d93cef8aaf610b90fba`,
`2d5609b5c5ef713aa3fda6467d000fad89cd8147e97d1541b5060305b414c99e`, and
`58a53b185d6e178314ce37014a1b29410278fe2691a5704891412f937fe49f84`.
The eight fixture SHA-256 values, in manifest order (Chula, Allegretto,
Batuque, Carmen, Cucaracha, Hove, Zizi, Bach), are
`5b2c92667118829af8c012c885a576205dc7184a1d6a1cb6440dd83a84561240`,
`ce8abcc8831c634620171c6652ad77c5ddb0091a3e709e3df66f2cdf2e6314c3`,
`f6b25b2e83a075542696bdadf68cb59536a52eb4fcab4f0f320affdad6515161`,
`50c59f895bccf01ee3f560c441819fdae4e9cf338bc2b5d06724f0b364ecb654`,
`9547f949810b7f80a112e02ea481fd79fc94e4fdd7b2e7969180c0195a426703`,
`01b448ad3007fa5323d78c4f4d386facada2b05410134b787b4e0ed8218a91f5`,
`92c32245ce076ce5bec0e4905f5308f87e8bf36d3511f80b5231e2bb22506412`, and
`ace8244c5ca9921c8841afb2507577cf45a36756c8b0587eec089bbc669b00e9`.
Eight focused production unit tests pass. The normal integration suite is 3
passed / 0 failed / 1 ignored in 31.09 seconds: the active tests are the
fail-closed schema-drift parser, expand-fixture provenance regression, and full
eight-page exact corpus gate, while only the faster Chula diagnostic is ignored.
The independent root full gate passes in 31.41 seconds, and strict
integration-test Clippy is green.

`apply_native_stems_beam_vlink_create_stem_transaction` closes the twelfth
production semantic STEMS boundary. It resumes exactly one first
`AwaitingVLinkTransaction`, applies any prior deferred known-false line delta
and the selected attempt's pending stored-theoretical-line/current-attachment
delta with their Java aliasing, then executes the `StemBuilder.createStem`
prefix. It selects a singleton Glyph or paints the exact vertical compound,
performs `GlyphIndex.registerOriginal` structural lookup/reuse/reinsertion and
shared sheet allocator handling, checks structural `systemStems` identity,
runs the exact `StemChecker`, and inserts a checked or profile-4 artificial
stem in `systemStems`. Rejection is a successful committed-prefix result: line
and GlyphIndex mutations are not rolled back.

The structural GlyphIndex evidence is a candidate-specific, one-shot exhaustive
certificate. Bounds and full RunTable content—not its provenance hash—define
equality. A different candidate requires a new certificate. Production
`createStem` supports a `systemStems`-Present lookup and returns the existing
stem inside this seam. Only the compact v1 real-fixture loader refuses to
hydrate Present system-stem evidence; that fixture limitation is independent
of the following VLinker head-side stem-reuse loop.

Across eight pages / 30 systems and transactions, 15 candidates are compound
objects with ID 0 before registration and 15 are singletons. Fourteen pending
line deltas commit. All 30 exhaustive Glyph lookups are Present and active, so
all register operations are `ReuseActive`; all 30 exhaustive `systemStems`
lookups are Absent; and all 30 transactions return `CreatedChecked`, insert the
new checked stem, and expose returned Inter ID 0. The gate bit-compares every
returned median endpoint, mean-thickness value, and integer bound of the
vertical ribbon; all returned stems are non-abnormal and have no SIG attached.
The shared allocator and InterIndex, SIG, relation, and link-flag deltas are
zero. The exact boundary stops before the VLinker head-side reuse loop,
`BeamStemRelation.checkLink`, SIG vertex/edge mutation, BeamStem relations, or
linker flags.

The chronology claim is intentionally narrow. Only system 1 of each page—the
eight system-1 transactions—is a true sheet-first run. Each of the other 22
transactions comes from an isolated fresh-sheet/system JVM targeted directly
at that system. Those rows are exact evidence for the local system frontier,
not a fabricated serial page-global Java ID chronology. The runner starts one
foreground JVM at a time and reaps it before the next system.

The reconstructed eight-page body is 261 lines / 153,517 bytes: 256 semantic
rows plus the five-line shared header. Its SHA-256 is
`0c8c51e1c170a0dc3ec7e5910e6dca63a82f7d8fe6699b585c9556f183b359dc`.
Manifest, probe, runner, and manifest-body SHA-256 are
`b7e6fe6e7dc2f5eeba106133c930249f20e2c75d764704252289724bbe28c3e0`,
`36fecabe18d7713c823ce6990dae717e78997354a9ae0b142cba55f7d75004f3`,
`6d95ff62d0acb502d531d6fb2aea0382fcb9dcb8fdd871fb7b0e2fba2ffb1de8`, and
`67d983b056548118015f5b7d18a9e2772860e08e0d2ab076118b25a9678c40af`;
the manifest body is 9 lines / 5,691 bytes. The active exact/synthetic gate is
5 passed / 0 failed / 0 ignored in 31.98 seconds, and eleven focused production
unit tests pass. The corpus itself does not exercise new or reinserted Glyph
registration, artificial creation, rejection, or existing-system-stem reuse;
focused synthetic tests cover those branches and the fail-closed certificates.

`evaluate_native_stems_beam_vlink_reuse_check` closes the thirteenth exact
production semantic STEMS boundary. This continuation is read-only: it joins
the frozen scheduler, expand-plan, and committed `createStem` products, starts
with the returned `StemInter`, and reproduces `BeamLinker.VLinker.link` from the
head-side reuse loop through public `BeamStemRelation.checkLink`. It stops before
the first SIG or relation mutation.

The reuse evaluator preserves the relation `LinkedHashMap` insertion order and
lazy Java reads. For each C entry it observes the shared S-linker's linked flag;
only a linked entry reads the S-linker's horizontal side and that head's
`HeadInter.getSideStems()` map. It preserves actual `HeadStemRelation` iteration
order, the absent-key Java null-dereference invariant, the first unique side
stem selection and break, multiple-stem continuation, and an explicitly unread
suffix. Dense native stem identity remains distinct from Java `Inter` ID and
Glyph identity. Any scanned live stem must be SIG-attached with a positive Java
ID; two distinct stems are still allowed to share one canonical Glyph.

The following relation check reproduces the beam-border/intersection
calculation by exact binary64 bits, including non-finite propagation; strict
beam-portion inequalities; the `Math.rint`-derived maximum x gap and scale
stem-thickness x-gap half-width; y gap outside the stem-median endpoints; raw
and clamped x/y impacts with weights 1/4;
`SupportImpacts` intrinsic ratio 1; inclusive minimum grade 0.1; and the
extension point and outgoing base-relation draft. Rejection returns no draft.
The result records initial and final stem payloads, every evaluated or unread
reuse entry, the complete numerical check trace, and zero persistent-ID,
`systemStems`, SIG-vertex, SIG-relation, and linker-flag mutations.

The real eight-page / 30-system corpus contains 30 first transactions and 65
ordered relation entries. Every S-linker linked flag is false, so Java performs
zero head-side scans, exposes zero live scan stems, and selects zero reused
stems. All 30 `BeamStemRelation.checkLink` calls accept and none reject. This is
an exact zero-reuse census, not evidence that the reuse branches occur on real
pages.

A bounded later-transaction reconstruction supplies that real branch without
changing the first-frontier census. Allegretto system 1 transaction 28 / plan
25 reads the linked `h:2:RIGHT:TOP` shared S cell, traverses the single live
HeadStem edge 229 in native SIG order, and selects the modeled attached StemInter
with Java ID 2227. The
Java snapshot/projection SHA-256 values are
`08b72a351a5ad443cbadb12f040dbb74e42ff6c031ef796f4dc563b502279a63` and
`46502fb158aa90d31c7594ec55686d4a2d1e796eebf55b0c7dfdc63f223abff6`.
The first unique selection breaks immediately, so relation-map entry 1 is
explicitly unread. Production derives the edge scan, HeadStem side, persistent
stem payload, first-reference catalogue, and both hashes from the owned graph,
bindings, S cells, and `systemStems` inputs before opening the five-row frozen
fixture; the projector leaves all inputs unchanged. The gate reconstructs this
predecessor state explicitly rather than replaying native transactions 1-27.
It stops at B13 and does not claim native predecessor carriage, B14
reuse/application, or general corpus reuse.
The separate fixture is 10 lines / 2,566 bytes with SHA-256
`287175a58717874882bc6487f7d59ea86a22e44cadcac003ee99a36606e5ab34`
(five semantic rows plus summary); it is not included in the original 601-line
Boundary-13 corpus hash.

The original first-frontier corpus also retains one separate system-1
`IsolatedSyntheticSig` block per page.
Those eight blocks use actual isolated SIG vertices, positive non-production
Java IDs, actual `HeadStemRelation` edges, and actual
`HeadInter.getSideStems()` calls without consuming the sheet allocator or
InterIndex. Per block, two distinct attached stems share one Glyph identity and
exercise zero, unique, and multiple side-stem cardinalities, lazy first-unique
selection, the missing-side-map failure, accepted/rejected check triples, four
beam-portion ULP cases, inclusive grade threshold, parallel-line propagation,
the fixed non-finite intersection, and isolated-graph plus real-sheet
zero-mutation guards. These are exact synthetic branch certificates; they are
not projected as real-corpus reuse.

The concatenated corpus body is 601 lines / 472,445 bytes: 553 semantic rows
plus 48 repeated six-line page headers. Its SHA-256 is
`76a6d20865a5a372bb6485ff6debeb0c435b64d1f92cf5ee07e1fbe0cf61418f`.
The eight complete split fixtures total 609 lines / 490,188 bytes. Manifest,
probe, runner, and manifest-body SHA-256 are
`4ab7078b760daca6691fcc03e8f29684ec4c976f918d747cb2047f01accd0559`,
`3ab243141f6eda3028885e3d73946c129e62554d5abc14658ca6e786f38650b0`,
`1b4913e1fc8f2665383635fac3e7c3c16f7de369ff8da5db4b4fe57e1b29ac21`, and
`58259448c36c5c684cbfef2215eb124a2ca62e5aae8f12d1a73510345687fb6d`;
the manifest body is 9 lines / 9,202 bytes. The eight full fixture SHA-256 values
in manifest order (Chula, Allegretto, Batuque, Carmen, Cucaracha, Hove, Zizi,
Bach Invention 5) are
`a80973509f1d46ea714a9423b25e990b569232ec2c06f91328a309742ae39692`,
`63ba4972e00fbc1cf55024b9a96ee40d2a43e6f7102f97a6a5218efa9b08d9af`,
`f70ececab743807a697908d02a94faa3badb976cc9b7ab5b80af5ca7921c768e`,
`a95cee883dd9ce619fe8ce277bbefaba08dc45ce71f56994b9e2cf22508368f9`,
`8acb8e38fc489270056e661b6fd9c475be178fd4cd2709ee3373b4df96045345`,
`8e2edaf3fa8769a813eaf219ac578c4fbdd3ce3de5f8ea152b8f0e12315b07a5`,
`050a3969ca0655149241888b4d649996a7ab41966e75ef32a8fdb9f5303100b7`, and
`959d5471527a2a6993a4f67f524c0fe0fc6fa457b05969aee3513ca466a538fe`.

The manifest pins Java `BeamLinker`, `BeamStemRelation`, `HeadInter`,
`HeadStemRelation`, `LineUtil`, `Scale`, `AbstractConnection`, `SupportImpacts`,
`Support`, `GradeImpacts`, and `GradeUtil` at
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
Every page was generated twice byte-identically. The runner used 8 compiler and
60 runtime foreground/reaped JVMs, maximum Java concurrency 1, and no background
process. Eight focused production tests pass with 597 filtered; all 8 exact
integration tests pass in 32.66 seconds under independent root verification.
Strict boundary-13 library/gate Clippy and global formatting are green.

`apply_native_stems_beam_vlink_base_transaction` closes the fourteenth exact
production semantic STEMS boundary. It begins at boundary 13's accepted
`ReadyBeforeSigMutation` result. When the selected stem has ID zero, it executes
the conditional `SIG.addVertex(stem)` prefix: shared InterIndex allocation and
VIP membership lookup, SIG vertex insertion in source order, `setSig`, the sole
`SigListener`, `StemInter.added`, its abnormal-state transition, and the
resulting SheetStub/Book modified and dirty propagation. A positive-ID,
already-attached stem skips that prefix.

The boundary then calls the base `Link.applyTo(beam)` with the fresh checked
`BeamStemRelation`. It preserves source/target-removed lazy suppression, the
source-outgoing duplicate scan and first same-runtime-class `isInstance` match,
separate fresh relation-object and graph-relation identities, exact
endpoint/incidence mirror joins, raw nullable BeamStem/BeamRest portions,
JGraphT edge insertion order, and synchronous callback order. The Java return
value is ignored by the caller,
so the continuation retains the fresh draft grade even when insertion is
suppressed. `BeamStemRelation.added` runs after edge insertion: it scans the
stem's incident relations and finds zero `ChordStemRelation` matches in compact
v1, then dispatches the beam's virtual `checkAbnormal`. Full beams scan ordered
BeamStem/BeamRest portions; hooks use the class-only any-BeamStem rule and do
not read a portion. Stem/beam abnormal requests and actual changes, allocator,
index and SIG deltas, listener/callback prefixes, and SheetStub/Book flags are
all explicit. Four isolated listener/callback/missing-target fault envelopes
retain Java's partial commits and no-rollback behavior without granting that
unsupported fault authority to production.

All 30 real first-frontier transactions across eight pages are `NewIdZero`:
Java inserts all 30 stems, adds all 30 base BeamStem edges, reuses zero stems,
and finds zero ChordStem matches. The 30 `predecessorcompat` certificates join
the frozen boundary-13 pre-mutation state despite the newer structural hash
domains. Each page's system-1 output also contains five supported cases and four
envelope-only cases on truly isolated Sheet/System/SIG instances. The corpus
therefore holds 40 supported branch cases and 32 failure-prefix envelopes. They
are isolated evidence, not naturally occurring corpus behavior or a blanket
production-equivalence claim.

The normalized corpus is 1,314 lines / 1,185,901 bytes: one shared eight-line
header plus 1,306 semantic rows, with SHA-256
`ece76c038ef1b2017d2f356dd6ead59379376ffc5ab0306e8c5e8c34a9471e53`.
The eight complete split fixtures total 1,386 lines / 1,227,749 bytes. Manifest,
probe, runner, and manifest-body SHA-256 are
`5da20f701d38bf9b81c6000ed4e8aba4fadd285c85d81753ef4a862f0a4875bc`,
`2139f0f5c2aba399d2eb8bc10ccbc2ec1221ce00ae2fdeb50782c80622f982e3`,
`88091fd27bef445f7045b721a6258da9652bac2f68d1ced277bbe82c1640d9b5`, and
`8bbd189d9c7e82702ce8513347841cfe5aff2f96f8b39bf9dd07e05bea4e6b35`;
the manifest body is 9 lines / 16,479 bytes. The eight complete fixture hashes,
in manifest order (Chula, Allegretto, Batuque, Carmen, Cucaracha, Hove, Zizi,
Bach Invention 5), are
`fa52b363ee03cf25b86c082fe69d9d9f8ad6449744e8d96d58f50353eb4dfa3a`,
`3ce50d5897774d9fff28f8148c5567407df9ad2ddee58de734e8e6c472659cf9`,
`a3eaaec9621958fc27bc781b917ce8f157898792bd837986c248f06e3b590c84`,
`2c0bb4e8b4fe0ed0b521aeb6e9a1e2354bdec0895e0d0795268cf675ec235356`,
`5eb50879dedcd435f552d8a63b21fec9a9ef5f4c290abde21be93baf5eeb26ca`,
`6fe4c59eb013fd733881ab7dade3ca22c2519f04c07eab58625fa01ab282c006`,
`d931f3e7eea1704925fcaf4278527ae35b71eda2b70d63fdd682e6602799bd00`, and
`d1961d7bd5db98ace4012e12dc8f04a7f60e045252cf934ce5b9764a7d9feee3`.

The manifest pins every page's scheduler, expand, `createStem`, and reuse/check
predecessor fixture; the complete active Java/Gradle source set; and JGraphT
core 1.5.2 at
`dfa596e9f0d0838f1b5e81dd0cd60e3a76c2c290ac25a0a029ffde58cf5e4c14`.
The seam-critical `BeamLinker`, `Link`, `SIGraph`, `SigListener`, `BasicIndex`,
`InterIndex`, `StemInter`, and `BeamStemRelation` source SHA-256 values are
`131f91f6605ecf03463ef4b6021a461240f99d7dfe2b1a1b94b0213d158d1747`,
`e27734fa0f4273db91527ed969ef1881605cda32eb970bb464ea037b0f0ed34e`,
`6b6ff3172d1f194566a7f59aa2c854cb62ea9c4deab79a43b6b0b85e1d4c4c2f`,
`19b42c96257bd78fc9d4bc428242590ae01832b395aebdeefe26e081ceadc08d`,
`7c747248365477c9381d004891e88f96273c0796a26f7417192fdaaeac8d3707`,
`830ee77262bd9b631d352e49ddc150055e621ad9cd76c2a0671fc2233b662b7a`,
`bcdb1b67694f45de89a9ad8712222e77af7c6e29247f5edd487d8dcabd11eeec`, and
`3ceff58fa9b298d97f325372d0e5a9b363755f3ad47cac7b66b07bd8d1e735f1`.
Every page was generated twice byte-identically with 8 compiler plus 60 runtime
foreground/reaped JVMs, maximum Java concurrency 1, and no background Java.
Twenty focused production tests and all 10 exact integration tests pass; the
gate finishes in 33.87 seconds. The full library suite is 623 passed / 0 failed /
2 ignored in 12.47 seconds. Strict Clippy, global formatting, diff-check, and
oracle `sh -n` are green.

`apply_native_stems_beam_vlink_b_linker_flag_transaction` closes the fifteenth
exact production semantic STEMS boundary. It starts from boundary 14's
`ReadyBeforeBLinkerFlagMutation`, clones the exact pre-boundary-14 state, reruns
the full base-application transaction, and requires the supplied transaction and
resulting state to match before touching the flag. It then resolves the
scheduler-selected outer B-linker and the exact TOP-then-BOTTOM order of every V
child that observes its shared cell.

The Java seam is one unconditional plain assignment,
`getBLinker().setLinked(true)`. It executes even when the base link's ignored
`applyTo` return is false and even when the cell is already true. The native
transaction therefore retains the prior apply return and fresh draft support
grade, records exactly one attempted/completed write, distinguishes that from
the false-to-true value-change count, and leaves S-linker flags, sibling links,
head links, IDs, indexes, SIG, stems, beams, and sheet-edit state unchanged.

All 30 real first-frontier transactions change false to true. The corresponding
live Java arena census is 3,948 B entries: 2,116 frozen constructor entries plus
1,832 dynamic anchors created after the frozen topology. Those unrelated and
dynamic objects are independently guarded by the oracle/gate; compact
production state models only the scheduler-selected shared cell and does not
pretend to hydrate the whole live arena. Eight page-local blocks add 32 isolated
`UnsafeExactClassNoGeometry` envelopes whose declared scope is setter and shared
cell only: 24 false-to-true, 8 idempotent true-to-true, and 8 whose retained
boundary-14 apply return is false. They are not reachable-geometry or blanket
production-equivalence claims.

The normalized eight-page corpus is 4,562 lines / 2,535,981 bytes, SHA-256
`6125665f38d894f6b05a24651f56f0a38c01e2acc2a7d18167a4175d5ae81c34`;
the split fixtures total 4,634 lines / 2,590,657 bytes. Manifest,
manifest-body, probe, runner, and effective-classpath SHA-256 are
`c7032ac4871188ef0cf48ac63d99996e78a0e163bf1470d3be84c5e9b10d1d92`,
`3f332e7751d5de73e296294ccc6882ff6a578d0328b8c0d717c96666ffbb3e4d`,
`b4c750370bebda13e66c49a8cc88756cb677ebf04f77d7dae883cb373fe431a8`,
`066a5ee494c583bdc7e9df1fc6e282015afc7663968b5e0a836219e545d14c24`,
and `fd4e52c2275675a53459dff2b2e2d89636f3c5fb6ab5a1f7be65f74157663fb3`.
The complete manifest is 10 lines / 24,897 bytes; its authenticated body is 9
lines / 18,910 bytes. Two byte-identical passes per page use 8 compiler and 60
runtime foreground/reaped JVMs, maximum Java concurrency 1 within the declared
runner lock scope, and no background Java process.

Seven focused production tests and the shared 5/5 exact hydration regression
pass; the latter finishes in 126.03 seconds. The terminal is
`ReadyBeforeSiblingBeamLinks`.

`apply_native_stems_beam_vlink_sibling_links_transaction` closes the sixteenth
exact production semantic STEMS boundary. It starts from an exact pre-Boundary-15
state, independently reruns the complete flag transaction, and requires the
supplied Boundary-15 transaction and resulting state to match before touching
sibling state. It then executes the complete Java `linkSiblings(stem, grade)`
call and stops immediately before the head-relation entry-set loop.

The transaction reconstructs `BeamGroupInter.getMembers()` from the exhaustive
outgoing-Containment scan, preserves its insertion order, performs Java's stable
top-down intersection sort, and removes the base beam by object identity. Each
sibling is processed serially: exact glyph-object identity can skip it; the
directed pair lookup stops at the first existing runtime `BeamStemRelation`; the
shorter-beam ordinate is read only when the inclusive 0.8 ratio predicate holds;
and a surviving sibling receives a fresh relation with exact extension, portion,
and preserved base-draft grade. The synchronous callback retains incoming-then-
outgoing incident order, requires zero `ChordStemRelation` matches in compact v1,
uses the raw-beam LEFT/RIGHT or hook any-BeamStem abnormal rule, and records any
SheetStub/Book dirty cascade. Only after that callback does the exact ordered
`StemBuilder.items` scan select the first source-identical sibling B-linker and
assign its shared cell. The state/result retain the complete serial
edge-callback-flag chronology and exact group-member post-state.

Across all 30 real transactions, the live group scans contain 58 Containment
members. All 58 have non-null native glyph identities and exact run-table tokens.
There are 11 real sibling candidates; all 11 take `Linked`, add one BeamStem edge,
complete one callback, and write one B-linker cell. Real same-glyph, existing-
relation, shorter-wrong-side, and ChordStem counts are zero; the 11 relations and
callbacks produce 33 ordered seam events. Eight page-local isolated blocks add
64 supported cases—`SameGlyph`, existing relation, shorter wrong side, full,
small, and hook beam links, no B linker, and an idempotent B cell—and 16 Java
throw envelopes. The supplemental cases are gate-only branch/failure evidence,
not production-equivalent real transactions; a false `addEdge` return remains
independent-model evidence because stock Java provides no honest live fixture.

The normalized corpus is 717 lines / 580,329 bytes: one shared 8-line / 753-byte
header plus 709 semantic rows. Its SHA-256 is
`c6a62f9b98ce55eda2bd142b083a2ff6b14d08dab6b1a2ce3c1a0d643d5efd66`;
the eight split fixtures total 789 lines / 654,858 bytes. Manifest,
manifest-body, probe, runner, and effective-classpath SHA-256 are
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
pass; the full gate finishes in 126.68 seconds. The full library suite is 652
passed / 0 failed / 2 ignored in 11.92 seconds. The terminal is
`ReadyBeforeHeadRelationLoop`.

`apply_native_stems_beam_vlink_head_links_transaction` closes the seventeenth
exact production semantic STEMS boundary. It independently reruns the complete
Boundary-16 transaction from its typed predecessor and requires the supplied
transaction and resulting state to match before touching head-link state. It
then executes the insertion-ordered Java head-relation map and stops after the
method returns true, immediately before the caller assigns the outer B-linker.

Each entry writes the shared parent S-linker cell before reading the graph. The
complete source-outgoing snapshot and exact head-to-stem filter preserve the
first assignable `HeadStemRelation` break. An existing relation skips every
later read. A missing relation lazily reads small-head and stem-length evidence,
mutates consistency on the already-created plan draft, inserts that same draft,
and synchronously runs the relation callback. Compact production requires exact
live endpoints, sole standard listener topology, prepopulated head side and
extension, and non-manual relation/head/stem state. It preserves head-then-stem
abnormal scans and ordered SheetStub/Book cascades; default metadata, manual
chord rewiring, and Java failure prefixes are isolated gate evidence. The final
`lastIndex < maxIndex` comparison remains explicit, its commented-out split does
not mutate state, and the method returns true.

Across all 30 real transactions, 65 ordered entries perform zero duplicate
suppression, 65 relation inserts, 65 S-cell writes, 65 consistency writes, and
260 ordered events. Eight isolated blocks add 16 supported and 40 envelope
transactions—56 total / 304 events—with 40 graph deltas, 16 throws, 16 manual
cases, and 8 chord rewires. These supplemental cases grade supported branches
and Java-only failure/manual prefixes without claiming production equivalence.

The normalized corpus is 1,583 lines / 785,671 bytes with SHA-256
`b57ec3f2bf401fce6d6d62c7522285dd3288b35b40d7c5c453468cf5dde4ce48`.
Emitted split bodies are 1,639 lines / 790,438 bytes with SHA-256
`044631a9dc5177b3fbe074a03cc031f52cb6087b3ea3491377f820d633b44d01`;
the full split fixtures are 1,655 lines / 873,975 bytes with SHA-256
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
pass; the full gate finishes in 148.82 seconds and the standalone manifest
validator passes 1/1 in 129.11 seconds. The full library suite is 676 passed / 0
failed / 2 ignored in 12.18 seconds. The terminal is
`ReturnedTrueBeforeOuterBLinkerAssignment`. The next exact seam is the caller's
outer B-linker assignment and scheduling, followed separately by later scheduler
attempts, stump linking, and competing-hook removal.

**The earlier retained-prerequisite checkpoint remains verified by CI as of
Rust run `31254538949` and Java run `31254538976`**, both green on
both legs with a full step list. That closes the `opt-level = 2` dev profile,
which spent a day unverified: GitHub Actions was in a major outage when it
landed (2026-08-06, incident from 15:22 UTC) and both its runs died in *Set up
job* before checkout. The second `java_hypot` fix in `beam_structure` was
likewise unverified for a while and was closed by run `31116910296`, green on
both legs including the `ubuntu-latest` one it exists for, since it fixes a
divergence macOS cannot see.

A note on reading Actions during that outage, since it cost time twice: runs
were being *created late*, not skipped. Two pushes appeared to trigger nothing
for over an hour, then both runs turned up at once and one immediately cancelled
the other by concurrency group -- which is correct behaviour, not a failure.
Wait and re-list before concluding a push did not trigger CI.

### The baseline JDK, which the repository does not carry either

`manifest.sha256` pins **Temurin 25.0.3+9**, and `xtask` looks for it at
`../jdk25/Contents/Home` relative to the repo -- i.e. a sibling of the checkout,
outside it. A fresh machine will not have it, and `brew install --cask
temurin@25` gives whatever 25.x is current (25.0.4 at time of writing), not the
pinned build. Fetch the exact one:

```sh
curl -sSL -o /tmp/t25.tar.gz https://github.com/adoptium/temurin25-binaries/releases/download/jdk-25.0.3%2B9/OpenJDK25U-jdk_aarch64_mac_hotspot_25.0.3_9.tar.gz
echo "7baab4d69a15554e119b86ff78d40e3fdc28819b5b322955c913cebfe3f6a37c  /tmp/t25.tar.gz" | shasum -a 256 -c -
mkdir -p ../jdk25 && tar -xzf /tmp/t25.tar.gz -C ../jdk25 --strip-components=1
```

That is the aarch64 macOS build; swap the asset name for other hosts, and get
the checksum from
`https://api.adoptium.net/v3/assets/release_name/eclipse/jdk-25.0.3%2B9`.

### Reproducing the PDF work on a fresh machine

The PDF parity test and its oracle need two things the repository does not carry.

**The corpus.** Seven of the twelve PDFs in the `imslp-pseudo` repo's
`manifests/acquired_scans.json`, downloaded to any directory, named by the
basename of the URL. `oracle/pdf-pages.txt` records which seven by name. Then:

```sh
AUDIVERIS_PDF_CORPUS=/path/to/pdfs cargo test -p audiveris-pdf --test corpus -- --nocapture
```

It prints `checked 189 pages, 189 images, 189 filter chains, 189 rasters,
189 draws, 189 renders; still unimplemented: {}`. Without the variable it
prints that it skipped, so a green run that says nothing is not evidence.

**PDFBox, only to regenerate the oracle.** It is not a Rust dependency; the
checked-in `oracle/pdf-pages.txt` is enough to run the test. To regenerate, take
the classpath from the Audiveris app and follow the header of
`oracle/java/PdfPageProbe.java`. On a JDK newer than the Gradle build's target,
`JAVA_HOME` has to point at JDK 25 or `:app:compileJava` fails with "invalid
source release"; and `JAVA_TOOL_OPTIONS` must be cleared, because a proxy banner
on stdout corrupts every parsed oracle.

The whole 189-page test takes about two minutes in release, half of it the
raster depth. Run it in release; in debug it is not worth waiting for.

Reproduced on a second machine, from a fresh clone and the seven sources
re-downloaded from the manifest URLs and checked against their `content_id`
SHA-256: `checked 189 pages, 189 images, 189 filter chains; still
unimplemented: {}` -- the raster count arrived after. The oracle names those files by the URL basename
**truncated to twenty characters**, which is what the directory has to contain.

## Two things that drifted, and neither was the port

Both surfaced on the first run of the gates on a second machine. Neither was a
behaviour change in this workspace, and both are the same shape: something the
tests are measured *against* moved.

### Which libjpeg, again -- now a build-time question too

`audiveris-jpeg`'s differential test decodes every fixture with `mozjpeg-sys`
alongside the port and requires equality. That reference turned out to depend on
the *host*: `mozjpeg-sys` compiles SIMD whenever it can -- unconditionally on
aarch64, and on x86 only when `nasm` is installed -- and mozjpeg's SIMD routines
disagree with mozjpeg's **own scalar C** on damaged input. Measured on
`corrupt-resync-80x80-420.jpg`: the scalar build returns `011e68ce7a923ae5`,
which is both Java's recorded raster and the port's, while the NEON build
returns `a5649ea51e999926`, 1032 of 19200 samples apart. So the same commit
passed on a machine without SIMD and failed on one with it, and the port was
never the side that moved.

The dev-dependency now sets `default-features = false`, dropping `nasm_simd`.
That is the right reference on the merits and not merely the convenient one:
libjpeg 6b, the library Audiveris actually reads through, has no SIMD at all.
It also removes `nasm` from the build requirements, which is part of why the
two-OS matrix is clean. `TURBO_DIVERGENCES` still names the eleven fixtures
where turbo genuinely differs from 6b; this was a different axis and does not
belong in that list.

### Three loops Clippy 1.96 rejects

`clippy::while_let_loop` began firing at 1.96.0 on `loop { let ... else {
break }; }`, which is the shape three ported decoder loops use -- two in
`ccitt.rs`, one in `jbig2/text.rs`. They are `while let` loops now; the
behaviour is unchanged and the corpus still reads 189/189. The rewrite is not
the point. The point is that a gate of `-D warnings` makes every Clippy release
a potential source of red on unchanged code, which is what the toolchain pin
below is for.

## Continuous integration

`.github/workflows/rust-port.yml` runs formatting, Clippy with `-D warnings`,
and `cargo test --workspace` on `ubuntu-latest` and `macos-latest`, on pushes
and pull requests that touch `rust/**` or `data/**`. It is separate from the
Gradle `build-and-test.yml`, which builds the untouched Java tree.

Two operating systems is not box-ticking. The matrix also spans two
architectures -- `macos-latest` is aarch64, `ubuntu-latest` x86_64 -- and that
is the axis the libjpeg divergence above sits on.

**What CI does not cover, and why.** The PDF corpus test needs 20 MB of
third-party IMSLP scans, so CI leaves `AUDIVERIS_PDF_CORPUS` unset and the test
skips. Making CI depend on a scan host's availability would buy coverage with
flakiness. The last workflow step therefore re-runs that one test with
`--nocapture`, so the log states which of the two it did rather than letting a
silent skip read as a pass. Nothing Java-backed runs in CI either: `xtask
baseline` and `xtask vectors` need JDK 25 and the parent OMR checkout, and the
oracle files they produce are checked in.

### The toolchain is pinned, deliberately

`rust/rust-toolchain.toml` pins the channel, for the reason the three rewritten
loops demonstrate: with `-D warnings` as a gate, an unpinned toolchain means a
commit that was green when written fails later with no change to the code.

Bump the channel in its own commit, with the lint fallout in that same commit.
Non-rustup installations ignore the file, so it binds CI and rustup users
without disturbing a Homebrew or distribution `cargo`.

## Structured output

`audiveris-cli -batch -step GRID|HEADERS|STEM_SEEDS|BEAMS|LEDGERS|HEADS -json
<input>` emits one JSON document per sheet, one per line. This is the
interchange format, not a debug dump. It is shaped for an evaluation harness
comparing several OMR systems and a repair loop that proposes corrections.

For the desktop comparison tool only, adding `-stream-json` turns stdout into
an additive line protocol. It flushes `@omrscope` schema-1 control markers for
run and completed/failed stage boundaries, with the ordinary unchanged
schema-1 document between a completed stage's start and completion markers.
It emits one complete snapshot after each of GRID -> HEADERS -> STEM_SEEDS ->
BEAMS -> LEDGERS -> HEADS up to the requested target. This is deliberately not
an item-event feed: no partial or per-item recognition results are promised.
Existing `-json` consumers must continue using the unframed default output.

The checked-in `omrscope` consumer accepts the three schema-1 geometry forms:
HEADERS' bounds-only `x/y/width/height` symbols, GRID's vertical
`x/top/bottom` medians, and STEM_SEEDS/BEAMS/LEDGERS' endpoint
`x1/y1/x2/y2` medians. Accepted top-level stem seeds are adapted into the
viewer's common comparison record without inventing a schema ID; rejected
records are ignored. It deliberately leaves incomplete bounds or medians
absent instead of letting Qt turn missing coordinates into zero, and it uses
the geometry center for its abscissa display and Java/Rust pairing. The parser
behavior has a CTest regression, and downstream publication supplies all three
forms.

HEADERS documents add selected clef/key/time inters with their bounds, grades,
contextual grades, and lifecycle/classifier evidence, plus staff ranges and
system-owned erase rectangles. STEM_SEEDS documents add accepted free glyphs
in raw-candidate order with complete check and materialization evidence. They
remain a top-level stage product rather than false SIG inters, and
`{system, ordinal}` is their stable identity. BEAMS and LEDGERS retain the
HEADERS and STEM_SEEDS state and append their stage products. The CLI runs the
composed seed-aware BEAMS entry point before either serializer. GRID still calls
the writer with no downstream products. After removing only `stem_scale` and
`stem_seeds`, the canonical prior Chula content is unchanged: BEAMS SHA-256
`f0866ce1ff0bd46d9b1093cc00e45c7ecd1692651f7e77a58c0e16ea81de9711`
and LEDGERS SHA-256
`4f67b6c2f98f51bc0e8bc5ded7dad501b875de3d8defa3906604650d5f023fdc`.

Three decisions worth keeping:

- **The envelope names its producer and schema.** A consensus front end diffing
  Audiveris against another system needs to know whose output it holds. The
  geometry and labels are meant to be comparable across producers; everything
  Audiveris-shaped sits under each inter's `evidence`, where a reader can
  consume it per-producer or ignore it.
- **`evidence.impacts` is the reason this exists.** A grade is a weighted
  geometric mean of six terms and the product alone is not diagnosable. Those
  six terms are what located the `rint`/`round` divergence that three rounds of
  source reading missed; a consumer can only use them if they are emitted.
- **`image.gray_digest` is a provenance stamp.** For a PDF it equals the
  FNV-1a-64 of the page PDFBox rendered, which the ingest test asserts, so two
  producers' outputs can be checked for having seen the same pixels before
  their disagreements are attributed to recognition.

**Candidates, not just answers.** `inters` are what survived, each with its
grade, contextual grade and six impacts. `candidates` are what did not: every
peak a `BarsRetriever` purge removed, with its span and the named stage that
removed it -- `PartialColumn`, `Unaligned`, `CClef`, `ExtendingBottom` and the
rest. A recogniser that emits only its answer cannot be judged on what it
missed, and `Unaligned` versus `CClef` are very different claims about the same
absent barline. Carmen, for instance, promotes 109 inters over 70 rejected
candidates.

That list is deliberately *not* advertised as a complete n-best: a peak that
never reached the purges, because it failed core validation or graded below
`Grades.minInterGrade`, is not in it. Widening it is a schema change, not a bug
fix.

Numbers are emitted at full `f64` precision, since exactness against Java is
the only property that makes them checkable. `-json` is a port extension and is
stripped before `Parameters` parsing, which mirrors Java's CLI and is pinned by
tests against it.

## The stage oracle, and how to grade a stage you have not ported yet

`oracle/java/SigProbe.java` prints every inter and relation a step leaves in
Java's SIG: identity, class, shape, staff, bounds, intrinsic and contextual
grade, frozen flag, and the impacts, with impact *names* taken from the inter's
own `GradeImpacts` so a head's terms and a barline's terms both print without
the probe knowing either. Output is sorted by inter id, so two runs diff
cleanly.

This exists because every stage so far got a bespoke probe, which is fine once
and a tax every time after. It is shape-agnostic: **a stage nobody has started
porting already has a parity gate waiting**, which is what lets several people
take different stages without each first building a way to check their work.

```sh
unset JAVA_TOOL_OPTIONS
JAVA_HOME=/path/to/jdk25/Contents/Home ./gradlew --no-daemon -q \
  -I rust/oracle/java/staff-impacts.init.gradle :app:sigProbe \
  -PsigTargets="data/examples/chula.png:1:LEDGERS"
```

Arguments are `<path>:<sheet>:<STEP>`, the sheet counted from one.

**Two things that will bite you.** `JAVA_TOOL_OPTIONS` must be cleared or a
proxy banner on stdout corrupts every parsed line. And Audiveris running from
`.class` files resolves its read-only resources as `Paths.get("res")`, relative
to the *process working directory*, while they live in `app/res/` -- so the
task runs from `app/` and absolutises the page arguments against the project
root to compensate. Without that, anything from HEADERS onward dies on a
missing `basic-classifier.zip` with an error that does not mention paths.

**How far it reaches, measured on chula:**

| Step | Inters in Java's SIG |
| --- | --- |
| GRID | 84 |
| HEADERS | 113 |
| STEM_SEEDS | 113 |
| BEAMS | 295 |
| LEDGERS | 313 |
| HEADS | reachable: the null-symbol failure was the probe skipping `MusicFont.checkMusicFont()`, now fixed |

So every stage the port is next to work on is gradeable today. HEADS is not,
and the reason is the MusicFont seam PORTING.md already lists as unported:
head recognition template-matches against font-derived symbols, so the port
needs MusicFont metrics before HEADS means anything -- in Java *or* in Rust.
That is a real ordering constraint, not a probe defect.

The narrower probes stay: `GridPdfProbe` generates the committed
`oracle/grid-pdf.txt`, and `StaffImpactsProbe` is the one that found the
`rint` bug.

## NO_STAFF is done; LEDGERS still needs two more inputs

**The staff-free image reproduces Java exactly on all nine example pages**,
including the JPEG. `oracle/grid-nostaff.txt` pins the FNV-1a-64 of Java's
`Picture.getSource(NO_STAFF)` and `no_staff.rs` matches every one.

The erasing was never the hard part. What was missing is that
`recognize_grid_lines` ran only `GridStepStage::BuildGrid` -- **GRID's own
`CleanStaffLines` stage had never run in the driver**, so every staff line was
still a `Filament`, no glyph had been registered, and there was nothing to
erase. The stage now runs, and the ordering that makes it work is Java's:
`rebuild_horizontal_lag` builds the table itself from the sheet's persistent
glyphs, exactly as `rebuildHLag` reads `Picture.getSource(NO_STAFF)` and
`Picture` builds it lazily from the glyphs `simplifyLines` just created. A
caller may still supply a table -- the fixtures do -- and then it is used as is.

Running that stage also changed nothing it should not: the barline,
completed-line-endpoint and SIG oracles all still pass.

One non-finding, recorded so it is not re-opened: the port's NO_STAFF digest
initially equalled chula's *gray* digest, which looks like the adaptive filter
returning its input. It is not. `oracle/grid-binary.txt` records the same
`2179468ede9f7ec6` for Java's BINARY raster, because chula.png is already
bilevel.

### Staff areas: done, and they exposed a containment bug

Every one of the 1209 lattice points on chula now agrees with Java's
`getClosestStaff`. `oracle/grid-closest-staff.txt` holds them.

The gate is behavioural on purpose: a `java.awt.geom.Area` is not worth
serialising, and nothing reads one directly -- `getClosestStaff` asks whether an
area *contains* a point and then breaks ties by distance. Grading that exercises
containment and the tie-break together, and it found three real divergences that
a structural comparison would have missed entirely.

**`Area.contains` is half-open, and the port had it exclusive.** `java.awt.Shape`
defines insideness so that a point on the boundary is inside when the space
immediately adjacent in the increasing-x direction is -- so the **left and north**
edges belong to the area and the **right and south** edges do not. The port
excluded all four. Two existing tests asserted the exclusive behaviour as though
it were Java's; both were wrong. Settled with a five-line `jshell` script rather
than by argument:

```
new Area(new Rectangle2D.Double(0, 0, 100, 100))
  contains(0,0) true   contains(50,0) true    contains(0,50) true
  contains(100,50) false   contains(50,100) false
```

No system test caught it because system areas are sampled well inside their
bounds. Staff areas reach the sheet edge -- a staff's north boundary is `y = 0`
when nothing is above it -- and there the exclusive rule assigned the point to
no staff at all.

**`StaffLine.yAt` does not extrapolate the spline.** Outside the line's own
abscissa range Java extrapolates along the straight chord between the line's two
endpoints, and uses the spline only inside. That difference only shows beyond
the notated staff, which is where two staff areas both contain a point and the
distance decides. Using spline extrapolation there was worth 14 of the 1209.

**`Staff.distanceTo` returns an `int`.** `getClosestStaff` compares
`(int) doubleDistanceTo(point)`, so distances within a pixel of each other tie
and the strict `<` leaves the earlier staff holding the point.

One thing is deliberately not reproduced. Java reads
`SystemInfo.getAreaEnd(LEFT/RIGHT)` and notes it "may not be known yet"; this
port never computes system area ends, so it passes zero and the intersection is
skipped, leaving each staff spanning the sheet. That is what Java does with
unknown ends, and it is what the lattice confirms -- but if system area ends are
ever computed, this has to start reading them.

### The builder

`build_population_staff_areas` is Java's `StaffManager.computeStaffArea`: a
horizontal slice between the staves above and below, intersected with the
containing system's area ends. Two things in it are Java's rather than
simplifications, and both are pinned by tests -- there is **no vertical
margin**, unlike a system area, and Java's guard
`(left != 0) || ((right != 0) && (right != sheetWidth))` means an unknown pair
of ends leaves the staff spanning the sheet while a left without a right yields
a *negative-width* slice and therefore an empty area. That last one is left to
fall out of `contains` rather than special-cased, because it is what Java does.

The neighbour walks are now shared. `SystemManager.vertNeighbors` and
`StaffManager.vertNeighbors` have identical bodies in Java, so the port's
`vertical_neighbors`/`horizontal_neighbor` are generic over a small `Placed`
trait rather than transcribed twice, which is one fewer place to drift.

**The production wiring is closed.** `GridLinesRecognition.staff_lines` now
publishes each staff's curved first and last line, while `staff_areas` publishes
the corresponding closest-staff area. `native_ledgers.rs` consumes both
directly; it no longer substitutes the per-system boundary collection.

The gate is already generated and is behavioural rather than structural, since
a `java.awt.geom.Area` is not worth serialising and the only consumer is
`getClosestStaff`. `SigProbe` emits a `closest <x> <y> <staff>` record over a
64-pixel lattice; chula gives 1209 points across its six staves, every one
assigned, and the native closest-staff gate matches all of them.

### What LEDGERS still needs

The former input, post-analysis, and final-line blockers are closed.
`native_ledgers.rs` composes the real native GRID and BEAMS products, and
`ledgers-chula.txt` grades all 18 final Java inters to nine decimals. Three
details were load-bearing:

1. `LedgersFilter` removes sections intersecting **any** `AbstractBeamInter`,
   hooks included. `LedgersBuilder` separately removes candidate middles only
   under good full `BeamInter`s. These cannot share one beam list.
2. Candidate checks use `StraightFilament`'s inclusive-pixel endpoints, but a
   materialized `LedgerInter` gets `Glyph.getCenterLine()` on the glyph contour:
   the right edge is exclusive and rows are centred at `y + 0.5`. Java also
   uses endpoint midpoint for rough containment and bounds centre for the
   staff-line reference; conflating those points perturbs every pitch impact.
3. Java tests a beam's geometric area against `Section.getBounds()`, not
   sampled section pixels. The post-analysis populations use the unbiased
   standard deviation and Java's floor/ceil integer checks; a ledger reused in
   several staff-map entries contributes every observation but only its last
   entry supplies the identity-keyed filter record.

The native builder has 19 chula survivors. The sheet-wide post-analysis rejects
one, removes its candidate filament, and rebuilds system 1; all 18 final inters
then match Java by ownership, geometry, thickness, seven impacts, and grade.
The same exact gate covers all 581 final inters across the eight beam sheets.
Java's rebuild never resurrects an inter already removed by overlap reduction;
carrying those tombstones closed the last six extras on cucaracha, hove, and
BachInvention5. `buildAllLedgerLines` is native too: it recursively translates
the curved outer staff line by each index's mean ledger offset, and all 95 paths
match Java. The stage is native, graded, and published. What remains is a gate
beyond the example corpus.

## BEAMS: scoped, and its first seam is grayscale morphology (CLOSED)

Closed. `audiveris_image::morphology` ports `StructureElement`'s circular
element and `MorphoProcessor::close`, and both are bit-exact against Java --
every structuring element cell for cell, and the closing digest for digest,
including on chula's 4.8-million-pixel staff-free page at the radius BEAMS
would actually use.

The end-to-end gate the section below anticipated turned out not to be needed.
`oracle/java/MorphoProbe.java` calls `MorphoProcessor.close` directly instead of
going through `SpotsBuilder`, so the closing is graded on its own rather than
through the beams it eventually produces, and `oracle/morphology.txt` pins:

- twelve structuring elements as pictures and offset vectors, not digests, so a
  disk that is one cell wrong says which cell;
- the closing over two generated buffers -- formulas rather than fixtures, so a
  port rebuilds the inputs -- at six radii, with the 24x16 pair dumped pixel for
  pixel;
- the closing over chula's NO_STAFF buffer at three radii, including 4.3, the
  one `SpotsBuilder` derives from its beam thickness of 12.

It also pins the four buffers `SpotsBuilder.getBuffer` passes through --
stem-run removal, median, gaussian, and the closing of all three -- which
morphology does not need but the rest of the step does. **That is the next
slice**: `RunTableFactory.LengthFilter`, then `Picture.medianFiltered` and
`gaussianFiltered`, each already having a digest to answer to.

Only the circular element and `close` are ported. The other element shapes and
the histogram-based `fclose`/`fopen` are unreachable from Audiveris and were
deliberately left out.

What follows is the scoping that led here, kept for the parts still true.

### The original scoping

LEDGERS' three inputs are now NO_STAFF (done), staff areas (done), and BEAMS.
BEAMS is a stage rather than an input, and it is largely ported already --
6613 lines across `beams_step.rs`, `beam_structure.rs`, `beam_extension.rs`,
`beam_hooks.rs` and `beam_groups.rs`, covering candidate ordering, the
border/core/belt impacts, hooks, grouping and multiple rests.

It is driven through a `VisualBeams` trait with eight methods, and the native
kernel behind most of them already exists. Encouragingly,
`NativeBeamKernelConfig.pixel_filter` is documented as "Java
`Picture.SourceKey.NO_STAFF`, not the morphologically closed spot image" -- so
the piece finished two commits ago is exactly what it wants.

**The seam that is genuinely missing is morphology.** `close_beam_spots` is
Java's `SpotsBuilder.close`:

```java
final double diameter = beam * constants.beamCircleDiameterRatio.getValue();
final float radius = (float) (diameter - 1) / 2;
final StructureElement se = new StructureElement(0, 1, radius, new int[]{0, 0});
new MorphoProcessor(se).close(buffer);
```

a grayscale closing with a circular structuring element sized from the measured
beam thickness. `StructureElement` and `MorphoProcessor` are 717 and 446 lines
of Java, and at the time the port had no morphology module at all.

`oracle/beams-chula.txt` still pins Java's 91 beams and 31 hooks with the six
impacts each grade is built from (`wdth`, `minH`, `maxH`, `core`, `belt`,
`jit`), and remains the gate for the recogniser above the closing.

## BEAMS (CLOSED)

The step's whole output is reproduced exactly on chula -- **91/91 beams, 31/31
hooks, 60/60 beam groups, nothing spurious** -- graded against Java's own SIG at
the end of the step, not against an intermediate.

All four stages are wired: `createBeams`, `extendBeams`, `buildHooks`,
`BeamGroupInter.populateSystem`. Two things about the last two are worth
carrying forward. `buildHooks` runs over the spots that produced *no* beam, so a
spot `checkBeamGlyph` refused is still a hook candidate -- that is where 11 of
the 31 hooks come from -- and its overlap test runs against a list that grows as
the pass adds to it. Grouping is **per system**: run globally over the page it
merges beams across a boundary Java never compares, and 60 groups become 48.

`extendBeams` now has two honest entry points. The compatibility wrapper passes
no seeds and disables only `extendToStem`; the composed production entry point
passes every accepted per-system STEM_SEEDS glyph in source order. Comparing
them across the eight example sheets and 30 systems, all 1,906 seeds are live
but the output is identical: `extendBeams` fires **once**, a merge on
BachInvention5's sixth system (`extendToBeam`), while `extendToStem` and
`extendToSpot` never succeed. The existing synthetic kernel gate covers the
accepted stem mode. `oracle/beam-stem-seeds.txt` separately proves the same
zero-effect fact in production Java over 803 final beam/hook inters, 493 groups,
and one multiple rest; none of their geometry, grades, impacts, membership, or
rest state changes when seed visibility is removed. A natural successful page
remains open; the BEAMS/LEDGERS CLI composition is now seed-aware.

The header erase remains the other open input, and it is priced in the section
below: five spurious clef-sized candidates out of 100 on chula, zero real beams.

## BEAMS native corpus path and CLI/JSON publication (CLOSED)

The beam pipeline is exact -- 787 of 787 raw beams across the eight example
sheets -- and now runs end to end from native GRID and HEADERS inputs. The three
inputs that originally kept it oracle-fed are all closed:

1. ~~**`scale.getMaxStem()`**~~ **(closed, `2982cef69`)**, for the stem-run
   removal that opens the spot chain. Not cheap for the reason first given --
   `StemScaler.getBuffer` cleans the raster before counting -- but the cleaning
   turned out not to move the mode on any of the eight sheets. See "Next
   session: start here".
2. ~~**`Staff.getHeaderStop()`**~~ **(closed)**, for `eraseHeaderAreas`.
   HEADERS now supplies all 30 native rectangles. Measured cost of omitting the
   chula rectangles remains five spurious clef-sized candidates and zero real
   beams, so this input is not optional even though that page loses no true beam.
3. ~~**System areas**~~ **(closed, `2982cef69`)**, for `dispatchSheetSpots`.
   `GridLinesRecognition` now carries `system_areas` and `system_bounds`, graded
   over 2739 centroids. Wiring rather than a port, as predicted, with one
   correction: the dispatch reads two different left/right pairs, not one.
   Without it a spot cannot be assigned to a system, and the system decides
   which spots each `BeamsBuilder` sees, what `buildHooks` searches, and how
   `BeamGroupInter.populateSystem` partitions -- grouping run sheet-wide instead
   of per system turns chula's 60 groups into 48.

`recognize_native_beams` is the honest composition boundary. What remains here
is output integration: publish its beam/hook/group records through `-json` and
the CLI stage driver. A measured small-beam scale is refused loudly because no
example sheet grades that class; none of the eight beam sheets has one.

## Push to `master` only

`claude/rust-port-takeover` was merged into `master` and the two have been
identical since. Pushing every commit to both fired **four** CI runs per commit
-- two of them on `ubuntu-latest`, competing for the same hosted runners -- and
half of it was duplicated work on an identical tree.

So: push to `master`. The branch is left where it is rather than deleted, and
can be fast-forwarded when it is actually wanted.

This is worth knowing when reading a red run, too. Three consecutive `ubuntu`
failures during this work were GitHub infrastructure rather than code --
`Service Unavailable` resolving the actions, an HTTP timeout, and "the job was
not acquired by Runner of type hosted" -- each failing in *Set up job*, before
checkout. The `macos` leg of the same runs passed. A short run duration used to
be the tell; once the dev profile went to `opt-level = 2` a genuine full run is
only two or three minutes, so read the step list instead.

## MusicFont: deferred with a price, not dropped

The port targets full parity -- every stage, including this one. What follows
is an ordering argument, not a scope cut.

**It is the geometry that needs the font, not the classifier.** `ClefBuilder`
calls the classifier with a null `ShapeChecker`, and both the bundled model and
`rank_evaluations` are already ported. Two font calls are the blocker:

- `getSymbolBounds` needs `TextLayout.getBounds()`, the glyph outline at a
  point size;
- placing that box needs `ShapeSymbol.computeCentroidOffset`, which rasterises
  the glyph to an **antialiased alpha image** and takes an alpha-weighted
  centroid.

That is Java2D's font rasteriser: native, hinted. Unlike the bicubic image
transform -- which was ported bit-exact because OpenJDK's `ScaledBlit` and
`TransformHelper` fully specify it in Java -- this one hands off to native
code, so bit-exactness is not something a reimplementation can promise up
front. Expect to need differential probing against the live JVM, and expect
the honest answer to possibly be a stated tolerance rather than a hash.

**What sits on it.** `header.stop` comes from `maxClefOffset`, which comes from
`staff.setClefStop`, which comes from that font-derived box. So MusicFont is
under HEADERS' clef geometry *and* under all of HEADS, where Java itself cannot
reach the step without it.

**What it is worth to BEAMS, measured rather than assumed.**
`header_erase_cost_is_measured_not_assumed` runs chula's whole spot chain both
ways:

```
header erase: 305 spots /  95 accepted
without:      333 spots / 100 accepted
only with erase (lost without it): []
only without erase (spurious):     5, at x=111..232, sized 33x16, 34x17, 32x16
```

Five clef-sized false positives in the header region, and zero real beams. That
is why BEAMS goes first -- not because the erase does not matter. The ratio is
pinned by that test, and the test fails if the erase ever becomes load-bearing
for a real beam. It should be re-measured on other pages rather than assumed to
hold.

## Historical plan that led to native HEADERS and BEAMS

This list is retained as the dependency record. Items 1 through 3 are closed.

1. ~~**The CFF/OTF outline parser**~~, which the MusicFont thread below shows is the
   one piece with no shortcut left. `rust/oracle/music-font.txt` is already its
   grading oracle. The JDK question that used to head this list is answered: the
   sweep is bit-identical under OpenJDK 26.0.1 and Temurin 25.0.3+9.
   (CI is clean too -- run `31134170478`, both legs, full step list.)
2. ~~**The header erase**~~, which was the *only* thing between the beam pipeline
   and running natively end to end. It is `Staff.getHeaderStop()`, so it is
   HEADERS, so it is MusicFont -- see the MusicFont thread below. It shows up
   twice, in `SpotsBuilder.eraseHeaderAreas` and again inside
   `StemScaler.getBuffer`, and both are the same dependency. Closed by the
   65/65 header chain and the 30/30 erase grade.
3. ~~**Beams into `-json`**, then into omrscope's Page and Inters tabs.~~ The
   CLI now publishes HEADERS, BEAMS, and LEDGERS in schema 1, and the consumer
   parses and pairs horizontal beam/ledger medians as well as GRID's vertical
   form.
4. **LEDGERS post-analysis**, now that native GRID/HEADERS/BEAMS-to-builder
   composition and its first exact gate are closed.

### Closed here: `scale.getMaxStem()`

Not cheap for the reason previously given, and the previous note was wrong about
why. `compute_stem_scale` was indeed already graded -- but `StemScaler.getBuffer`
does not count runs in NO_STAFF. It erases barline and connector inters, erases
each system header (`useHeader` defaults to **true**, and `eraseSystemHeader`
reads `getHeaderStop`), and paints white outside the core staff paths.

Rather than port all three, it was measured: the uncleaned NO_STAFF raster
reproduces Java's `maxStem` on all eight sheets, including the two where Java
says 5 rather than 4. A mode over ~10^5 runs is not a statistic a tail moves.
`stem_scale_from_the_uncleaned_no_staff_is_measured_not_assumed` asserts it per
sheet, so the first page that needs the cleaning names itself.

### Closed here: system areas

`GridLinesRecognition` now carries `system_areas` and `system_bounds`.
`build_population_system_areas` was already written and graded; what was missing
was that `dispatchSheetSpots` reads **two** left/right pairs -- the area's, which
are midpoints to its neighbours, and the system's own staff extremes.

Graded over all **2739 spot centroids on all eight sheets**, exact, as a pure
function of the centroid -- which keeps it independent of the spot chain that
cannot yet produce those centroids natively. Dropping the abscissa test alone
moves 5 of the 2739, one being the carmen top-right spot that invents a beam.

## HEADERS: what it actually needs (measured 2026-08-06)

`HeadersStep.doSystem` is one line: `new HeaderBuilder(system).processHeader()`,
producing clef, key and time per staff. `getHeaderStop()` -- the thing beams and
`StemScaler` both want -- falls out of that.

### The classifier is not a blocker: it is already ported and graded

An earlier note here listed the classifier as the first two items of work. That
was wrong -- `crates/audiveris-classifier` already carries all of it:

- `mix_glyph_features`, the 110-value `MixGlyphDescriptor`: the 20x5 ART moment
  grid (`F001`..`F194`) from `BasicARTExtractor` with its LUT and bilinear
  interpolation, plus `weight width height n20 n11 n02 n30 n21 n12 n03 aspect`
  from `GeometricMoments`. Java's traversal orders are preserved deliberately,
  including the backwards two-pass accumulation and `coeffImag -=`;
- `BasicClassifier`, parsing `app/res/basic-classifier.zip` (a 110-149-149
  single-hidden-layer MLP in plain XML, *not* a deep net) and running
  `NeuralNetwork.forward`'s last-index-down accumulation order;
- `rank_evaluations`, Java's `byReverseGrade` sort with `Double.compare` NaN
  canonicalization, the min-grade break and duplicate-shape suppression.

Graded against the live Java oracle in `RustParityProbe`. `ClefBuilder` calls
the classifier with a null `ShapeChecker`, so nothing more is needed from it.

### MusicFont is the whole of what is left, and it splits in two

Two font-derived quantities reach the SIG, and they are *not* the same problem.

**1. `getSymbolBounds` -> `TextLayout.getBounds()`: ported, and the arithmetic
is not what it looked like.** `MusicFont.getPointSize(interline)` is exactly
`4 * interline`, so the point size is not a source of slop. The law, recovered
from a 116-interline sweep of all six clefs (`rust/oracle/music-font.txt`):

> each *edge* of the box is independently rounded to a 1/64 px grid, after
> scaling a per-shape em-unit outline extreme by the point size.

Per-edge, not per-dimension -- the width is `right - left` **after** both have
been quantized, which is why widths alone never fit a clean law. The grid is
**1/64**, not 1/32: at 1/32, 924 of the 2784 swept values are off-grid; at 1/64,
none are.

An earlier note here claimed the em constants looked like integers over
`unitsPerEm = 1000`, so the whole thing could be pinned as four numbers per
shape. **That is wrong and the sweep is what caught it.** The fitted constants
are near-integers but not integers (F_CLEF's right edge is 684.00025/1000), as
they must be: a glyph bbox includes cubic Bezier *extrema*, which are irrational
in general even when every control point is on the grid. Fitting them from
measurements leaves an interval about 1e-6 em wide, and that is not tight
enough. Concretely, 22 of the 24 clef edges are reproduced exactly at all 116
sizes, and the other two fail identically:

```
G_CLEF / G_CLEF_8VB, top edge, interline 17 (point size 68):
  fitted em constant   -1097.99961/1000
  law predicts         -74.65625
  Java gives           -74.671875     (off by exactly 1/64)
  the product lands at -4778.4943, which is 0.006 from the -4778.5 tie boundary
```

So the failure is not the law, it is the precision of a *fitted* constant near a
rounding tie -- and interline 17 is an ordinary sheet, not a contrived one. The
exact constants have to come from the font. That means a real CFF/OTF outline
parser with correct Bezier extrema: specified work, no pixels painted, but not
avoidable by pinning.

**2. `ShapeSymbol.getCentroid` -> `computeCentroidOffset`: pin it, do not port
it.** This walks the alpha channel of a *rendered* glyph and takes an
alpha-weighted centroid.

A hypothesis worth recording as refuted, because it looks right in the source:
`buildImage` sets `KEY_ANTIALIASING` to `VALUE_ANTIALIAS_OFF`, which suggests
the alpha channel is binary and the centroid is just a coverage-mask mean. It is
not. The measurement found **~200 distinct alpha values** in each rendered clef.
`KEY_ANTIALIASING` governs *shape* rendering, while the symbol is drawn via
`TextLayout.draw`, which obeys `KEY_TEXT_ANTIALIASING` -- left at the platform
default, and on. This is antialiasing coverage, and reproducing it exactly does
mean reproducing Java2D's text rasteriser.

What makes that not matter: **the offset is a constant, not sheet data.**
`computeImage` renders at the fixed `SampleRepository.STANDARD_INTERLINE = 20`
regardless of the font it was asked for, so the offset depends on
`(family, shape)` and on nothing else. Measured at seven interlines from 10 to
48, the returned offsets are bit-identical:

```
F_CLEF          -0.03884001107392759, -0.13394309933117415
G_CLEF           0.00205725082845354,  0.01888306366816295
G_CLEF_8VA       0.01336868758375387,  0.04381791002555180
G_CLEF_8VB       0.00052491308994185, -0.01328569918562944
C_CLEF          -0.06580471127152870, -0.01731409766015940
PERCUSSION_CLEF -0.02309224973860641, -0.01249250593760271
```

Six numbers per shape-set for Bravura, and that is the entire header-clef table.
Pinning them is the same move as shipping the classifier's trained weights: data
rather than logic. Note the quirk that makes this safe -- `ClefBuilder` passes
`MusicFont.getPointSize(...)` where an *interline* is expected, so the symbol it
retrieves is sized wrongly; it does not matter, because the offset ignores the
size entirely.

Re-measure with `./gradlew -I rust/oracle/parity.init.gradle :app:musicFontScout`
(`MusicFontScout.java`), which writes the rows in `rust/oracle/music-font.txt`.
It needs `workingDir = app/`, since `WellKnowns.RES_URI` is `Paths.get("res")`
outside a jar. Default family is `Bravura` (`Bravura.otf`), in `app/res/`.

**On the first row of that file being the JDK.** Every other oracle here is Java
arithmetic, which is specified and portable. These values are not: they come out
of Java2D's font machinery, so they are only as portable as the runtime that
made them, and that needs checking rather than assuming.

*JDK axis: checked, and clean.* The sweep was captured under both **OpenJDK
26.0.1** and the baseline **Temurin 25.0.3+9**. All 711 value rows are
bit-identical; the only line that differs is the one naming the runtime. The
checked-in copy is the Temurin one. So the twelve centroid offsets are safe to
pin, and `getPointSize`, the 1/64 grid and the per-edge rounding do not move
across a major JDK version.

*Print the values with `Double.toString`, never `%.17f`.* The first capture used
`%.17f`, which is seventeen digits **after the point** -- only sixteen
significant digits at these magnitudes, one short of what a `double` can need.
It silently truncated: G_CLEF's x read `0.00205725082845354` where the value is
`0.0020572508284535385`. Harmless to the eventual pixel, and exactly the kind of
thing a port graded on bit-exactness should not be quietly carrying.

*Platform axis: not checked, and CI will not check it for you.* This was
measured on macOS/aarch64 only. `TextLayout.getBounds()` is outline-derived and
ought to be portable, but `centroidOffset` comes from **glyph rasterisation**,
which does not go through Marlin -- it goes through the platform font scaler,
FreeType on Linux against CoreText on macOS. That is exactly the axis the CI
matrix exists for, and exactly the axis it cannot see here, because the Java
oracle runs only locally: a Rust test asserting pinned constants compares them
against themselves and passes on `ubuntu-latest` whatever Java would have said
there. If the offsets ever need to hold on Linux, that has to be measured on
Linux. Until then, treat them as macOS-derived constants that happen to be
JDK-stable.

### Java's font scaler is fixed-point, and that is the whole story

The last four of the 696 swept values refused every floating-point model, and
chasing them turned out to be the most useful thing in this thread.

Scaling the exact font-unit box by `pointSize / unitsPerEm` in `f64` and
rounding each edge to 1/64 gets **692 of 696**. The four misses are G_CLEF and
G_CLEF_8VB -- the two glyphs whose top is at 1098 font units -- at interlines 17
and 108. They cannot be fixed by a better constant: interline 17 needs
`max_y >= 1098.0018` and interline 108 needs `max_y < 1097.9999`, so no single
value satisfies both, and Java is therefore not linear in size here.

Things ruled out by measuring rather than by argument:

- *A bad outline.* Asked at point size 1000 (scale exactly 1), Java's own
  `getGlyphOutline` returns integer coordinates that match the Rust parse
  segment for segment, including the `(455, -1098)` endpoint that sets the top.
- *Hinting.* The deviation is 1/128 of a pixel. A hint snap moves an edge by a
  half or whole pixel, not by 0.008.
- *`float` arithmetic.* Five different f32 orderings were tried; every one
  reproduces the f64 answer, because the gap needed is ~14 float ulps.
- *Control-point pre-quantization.* All six clef extremes are at *on-curve*
  points, which round identically either way.

What it is: Java's scaler does FreeType's integer fixed-point arithmetic, and
rounds **twice**.

```
scale_16_16 = FT_DivFix(pointSize * 64, unitsPerEm)   // = (a<<16 + b/2) / b
coord_26_6  = FT_MulFix(font_units, scale_16_16)      // = (a*b + 0x8000) >> 16
```

Two roundings can land one 1/64 step away from a single rounding of the exact
product, and those four rows are exactly that. This model reproduces **696 of
696** with no exceptions. The lesson generalises: any other Java2D font quantity
this port needs is likely fixed-point too, so reach for `FT_MulFix` before
reaching for a tolerance.

**One deliberate gap.** Every clef box is set by an on-curve point, so all six
are whole font units. A box set by a curve *interior* would expose an ordering
question -- Java quantizes points to 26.6 and solves for extrema there, which is
not the same as solving in font units and scaling after -- and nothing in the
sweep grades it. `layout_bounds` returns `FontError::UngradedOutline` rather
than guessing. The first shape that needs it (heads, most likely) has to extend
the sweep first.

### The tolerance, if a rasteriser is ever wanted anyway

`ClefBuilder` uses the offset only as
`rint(box.getCenterX() + box.getWidth() * offsetX)`, so an error in `offsetX`
smaller than `0.5 / box.width` rounds away. Measured over the clef shapes at
interlines 10..48 that budget is 0.0037 (widest, interline 48) to 0.033
(narrowest, PERCUSSION at interline 10), typically ~0.008 at corpus interlines.
Since the offset is a mean over 1300-2900 covered pixels normalised by a ~55px
image width, that is roughly half a pixel of allowed centroid drift -- a loose
budget for a rasteriser, though not a guaranteed one, since a value landing near
a `.5` boundary has no margin at all. Pinning avoids the question.

### Order from here

1. ~~`TextLayout.getBounds()` for the six header clefs.~~ **Done** -- a CFF/OTF
   parser in `crates/audiveris-music-font`, graded 696/696 against the sweep.
   The interline 17 row was indeed the one that discriminated, but not for the
   reason predicted; see below.
2. ~~The pinned `(family, shape) -> centroidOffset` table.~~ **Done** --
   `crates/audiveris-music-font`, which also carries `getCentroid`'s
   `rint(centre + size * offset)` and `getPointSize`. The offsets are compared
   to `music-font.txt` **by bit pattern**, not by tolerance: `Double.toString`
   and Rust's `f64` parser both guarantee shortest-round-trip, so an exact match
   is available and anything weaker would hide a transcription slip. Perturbing
   one constant's last decimal digit fails that test and only that test.
3. `ClefBuilder`. **Partly done**: `clef_classifier.rs` implements the
   production `ClefShapeClassifier` -- noise gate, features, MLP, Java's
   rank-then-filter order, and `getSymbolBounds`. What is *not* done is the
   grading, and that is the next real task; see below.
4. `KeyBuilder` and `TimeBuilder`, whose columns are also already written.
5. `getHeaderStop()`, which closes the beam and `StemScaler` erase dependency.

### `clef_classifier` is wired but ungraded, which is not the same as done

`clef_column.rs` always had the `ClefShapeClassifier` seam; the only
implementation was a test double returning `glyph.bounds` as the symbol box.
That is now a real one, and every piece it composes is separately graded -- the
110 features and the MLP against `RustParityProbe`, the font box 696/696 against
the sweep.

**The composition itself is not.** Its unit tests cover the noise gate, the
rank-then-filter order, the drum-staff shape set, and the two independent
roundings in `getSymbolBounds` (`rint(w/2)` is not `rint(w)/2`; C_CLEF at
interline 23 is the case that separates them). None of that is evidence that
Rust picks the same clef as Java on a real page.

That oracle now exists: `rust/oracle/clef-headers.txt`, 65 staves across the
nine corpus pages, every one of them carrying a clef (52 `G_CLEF`, 10 `F_CLEF`,
3 `G_CLEF_8VB`). Each line has the staff's specific interline, the header start
and stop, `clefStop`, the shape, the raw grade, the symbol box, and the glyph
box and weight -- the last so that a shape disagreement can be told apart from a
part-assembly disagreement.

`ClefProbe` drives each sheet to HEADERS **in-process** rather than parsing a
saved `.omr`, which is what lets it read live `StaffHeader` objects. Getting
there needs three things that are not obvious and cost most of the time:

- `new Book(inputPath)`, not `Book.createBook(path)` -- the latter treats the
  path as a *target* `.omr` and makes stub creation try to browse a PNG as a zip;
- a stub built directly (`new SheetStub(book, 1)`), since `book.createStubs()`
  reaches for `Main.getCli()`;
- a batch `CLI` installed into `Main`'s private static field by reflection,
  because `reachStep` consults it for `isSave()` and the output folder.

It runs from `app/` (fonts) and reads the corpus as `../data/examples`.

**One of the three risks turned out not to be gradeable here.** The claim above
that the corpus contains sheets with two staff sizes is wrong: on all 65 staves
`getSpecificInterline()` equals the sheet interline, so the
specific-versus-sheet interline split that `ClefBuilder` and
`MusicFont.getPointSize` disagree about is *never exercised*. A Rust port that
used the wrong one of the two would pass this oracle. That needs either a sheet
with small staves added to the corpus or a targeted synthetic case; until then
it is an untested divergence, not a covered one.

The two remaining risks the oracle does cover:

1. **The glyph the classifier is handed.** Java classifies a `Glyph` assembled
   from header parts, and the descriptor reads its `RunTable` with the glyph's
   own origin. If part assembly differs at all, every feature differs -- which
   is why the glyph box and weight are in the oracle.
2. **`ClefInter.kindOf`,** which maps shape plus glyph centre to a `ClefKind`,
   and which `clef_column` reimplements as `clef_kind` + `target_pitch`.

**What is still missing is the comparison, not the oracle.** Nothing reads this
file yet. Do not report clefs as ported until a Rust test reads
`clef-headers.txt` and matches all 65 staves.

#### Where that work actually stands, and the one thing blocking it

More of it was already built than expected. `NativeClefProposalRecognizer`
already takes `sources: BTreeMap<usize, RunTable>` -- per-staff NO_STAFF crops
-- plus contexts and parameters, and is generic over `ClefShapeClassifier`, so
`BundledClefClassifier` drops straight in. `build_clef_lookup_contexts` already
ports `getOuterRect`/`getInnerRect`; `glyph_factory.rs` already ports
`GlyphFactory.buildGlyphs`; `near_graph` and `connected_sets` already port
`Glyphs.buildLinks` and the connectivity pass. So there is no missing algorithm,
only a missing driver.

`clef_parameters.rs` is the first piece of it and is done: `ClefBuilder.Parameters`
with its two-interline split intact, `Scale.Fraction` as `rint(interline * v)`
and `Scale.AreaFraction` as `rint(interline^2 * v)`. Interline 21 -- the corpus'
most common -- lands on two rounding ties at once (94.5 and 10.5), and both go
to even, so a port using `round()` fails rather than half-passes.

**Staff-line geometry: closed.** `GridLinesRecognition` now carries
`staff_lines: Vec<StaffLineGeometry>`, each with the first and last line splines
plus `first_line_y_at(x)` / `last_line_y_at(x)` (Java `LineInfo.yAt(int)`,
`rint`ed). The splines were being computed inside GRID and dropped; nothing new
is derived. Outside a spline's abscissa range these return `None` rather than
extrapolating along the global slope as Java does -- deliberate, so a caller
that strays outside a staff names itself instead of receiving an invented
ordinate. Nothing in HEADERS should stray: its abscissae are the middle of a
staff's own browse range.

**A bug surfaced by doing that, now fixed.** `build_clef_lookup_contexts`
evaluated each neighbour's gutter from *one scalar ordinate per staff*, but
Java's `getOuterRect` reads a neighbour's line at the **current** staff's
`xMid`. Those coincide only when staves are parallel and aligned. There is now
`build_clef_lookup_contexts_at`, taking a `StaffLineOrdinates` resolver, with
the old signature kept as the flat approximation the headless tests use.

The regression test matters more than it looks: it is built so the sloped
neighbour's gutter binds *only once sloped* -- flat, it lands below the
`aboveStaff` limit and is invisible. That is exactly how this divergence would
have hidden in production, and the first version of the test missed it for that
reason.

**The driver and the comparison now exist**, in
`crates/audiveris-omr/tests/clef_headers_corpus.rs`: it runs GRID on each of the
nine pages, builds the lookup contexts from the published splines, assembles
`NativeClefProposalRecognizer` over `BundledClefClassifier`, and compares. **All
65 staves match Java on shape and on the symbol box.**

It supplies Java's header start rather than computing it, and grades only what
the clef stage does with it -- the same isolation the spot-dispatch test used.
`compute_header_starts` needs its own oracle; grading it from this one would be
circular, since it is an input here.

### The missing centroid correction, which the corpus found immediately

The first run failed on **53 of 65 staves with every shape, width, height and
ordinate correct and only the abscissa out, by 1 to 3 pixels.** That is about as
precise a diagnosis as a failure can hand you, and it pointed straight at the
one step `clef_classifier` had left out: `registerClefs` slides the box by
`dx = glyphCentroid.x - symbolCentroid.x` *after* `getSymbolBounds` has centred
it. Two different centres are involved -- the glyph's **area** centre positions
the box, then the glyph's **mass** centroid corrects it -- and Java's own
comment explains why: unerased staff-line chunks shift the ink sideways.

Note what this says about the unit tests. `clef_classifier` had tests for the
noise gate, the rank-then-filter order, the drum shape set and the two
independent roundings, and all of them passed against code missing an entire
step. The corpus caught it on the first run.

### `clefStop`: closed, and the first explanation was wrong

Computing it as `glyph.getBounds().intersection(clefBox)` reproduced 56 of 65,
with all nine misses on bass staves. The note here previously blamed that on
`registerClefs` setting `clefStop` from the candidate at index 0 while
`selectClef` later picks a different one by contextual grade. **That was wrong.**
Extending `ClefProbe` to emit every registered candidate showed exactly one per
staff, with contextual grade equal to intrinsic -- so there was never a
competing candidate to disagree about.

The real cause is that `Staff.getClefStop()` does not return what
`setClefStop` stored:

```java
public Integer getClefStop () {
    if (header.clef != null) {
        Rectangle bounds = header.clef.getBounds();
        return (bounds.x + bounds.width) - 1;      // the glyph is not consulted
    }
    if ((header.clefRange != null) && header.clefRange.valid) {
        return header.clefRange.getStop();          // the stored value, as fallback
    }
    return null;
}
```

`registerClefs` does compute an intersection and store it on the clef *range*,
but once a header clef exists that stored value is never read; the getter
recomputes from the clef's own bounds and ignores the glyph. The stored form
survives only for a staff whose clef was never selected.

This is a quiet difference rather than a loud one: the two agree whenever the
glyph is at least as wide as its symbol, which held on 56 of 65 staves. The nine
that disagreed were all bass clefs, whose ink is narrower than the `F_CLEF`
symbol. `clefStop` is now asserted on all 65.

Two lessons worth carrying. A getter that recomputes rather than returning the
stored field is exactly the kind of thing a port reads past; when a value has
both a setter and a getter, read the getter. And a hypothesis that explains the
*pattern* of failures -- "all nine are bass clefs" -- can still be the wrong
mechanism, so it is worth the ten minutes to test it before writing it down as
fact.

## The two back-half risks, scouted (2026-08-07): neither blocks the project

Both were investigated by dedicated scouts with file:line evidence; the full
reports are `rust/scouting/heads-rasteriser.md` and `rust/scouting/texts-ocr.md`.
Summary of what matters for planning:

### HEADS templates: pinnable data, no rasteriser port needed

Rendering enters template construction at exactly one point and is **binarised
immediately** (alpha >= 140); graded coverage never survives, and the runtime
match reads each keypoint only as a 3-way fore/back/hole class with integer
weights. Templates depend on **(family, shape, integer pointSize) alone** --
the pointSize is sheet-derived (measured black-head widths, secant interpolation
over the already-ported `TextLayout.getBounds`) but collapses to one integer per
staff. Whole-corpus template set: at most ~216 entries, under 1 MB -- dump them
from a Java probe as oracle data, the classifier-weights move again. The
sheet-side chamfer matching is pure integer arithmetic.

**Carried risk, do not lose it:** `PageCleaner` (the SYMBOLS/TEXTS eraser base)
paints font glyphs at *fractional* positions into the erase buffer. That is the
one genuine rasteriser dependency left, it is downstream of HEADS, and it needs
its own scout before SYMBOLS is attempted.

### TEXTS OCR: fixture strategy confirmed by live measurement

Java binds Tesseract **5.5.2 in-process** (bytedeco), legacy engine
`OEM_TESSERACT_ONLY`, PSM_AUTO for the sheet scan; input is the NO_STAFF-derived
buffer with good inters erased, round-tripped through in-memory TIFF at
resolution 70. The narrowest clean seam is `OCR.recognize -> List<TextLine>`,
and the **complete** call-site set is: the TEXTS sheet scan, CURVES'
`RehearsalsBuilder` (a second batch stage the fixture must cover), and one
GUI-only path. Rust already has the matching `ExternalTexts` seam; its
`NeutralOcrWord` must grow baseline/font/char fields.

Two live facts that settle the strategy. **Determinism:** the same sheet OCR'd
in two JVM sessions produced bit-identical raw TextWords. **Feasibility:** with
a legacy-capable `eng.traineddata` (23.5 MB, from `tesseract-ocr/tessdata`; not
bundled with Audiveris, and Homebrew's is LSTM-only and useless to the legacy
engine), all corpus sheets ran headless to end of TEXTS: ~134 sentences, ~252
words, 262 lyric items.

Plan as recommended: a recorder probe keyed by `(image-pixel SHA, langSpec,
PSM) -> raw TextLines`, a Rust `FixtureOcr` that fails loudly on a key miss --
which converts "fixture valid only if upstream stages are bit-identical" from an
assumption into a checked invariant -- then the actual port is `TextBuilder`.
Linking Tesseract from Rust would mean fighting a differently-compiled binary's
float behaviour for zero port value, and the fixture seam is where a real
binding would plug in later anyway.

## KEY and TIME: measured before starting (2026-08-07)

### How much of the corpus exercises them

Worth knowing before planning: a stage with two examples on nine pages needs a
different approach from one with sixty. `ClefProbe` now emits `key` and `time`
rows per staff, absence included.

```
key:  34 of 65 staves     fifths -3 (x12), 2 (x10), -2 (x6), -1 (x6)
time: 17 of 65 staves     COMMON_TIME (x7), TIME_TWO_FOUR (x7),
                          TIME_THREE_FOUR (x2), one with a null Shape
```

Both are well exercised. The null-shape time is not a defect: `TimePairInter`
and `TimeCustomInter` carry no single `Shape` and are described only by their
rational, so the oracle emits `getTimeRational()` alongside the shape.

### The font layer is already done for KEY, and half done for TIME

The scout now sweeps eleven shapes rather than six -- the clefs plus `FLAT`,
`NATURAL`, `SHARP`, `COMMON_TIME` and `CUT_TIME` -- and every one of them
behaves exactly as the clefs did:

- all eleven centroid offsets are size-independent, checked at seven interlines
  and emitted only on agreement, so they are pinned as constants;
- **1276 of 1276** swept outline boxes match, the same `FT_DivFix`/`FT_MulFix`
  fixed-point law, no exceptions;
- the `UngradedOutline` guard never fired, so every one of these boxes is set by
  an on-curve point and the curve-interior ordering question stays theoretical.

So `KeyBuilder` needs nothing further from the font.

### TIME: composite layout and classifier in place (updated 2026-08-08)

The num/den stacking is ported: `num_den_dimension` measures both digit layouts
with the graded `layout_bounds`, separates their centres by
`2 * getStaffInterline(font)`, and `rint`s the raw composite rectangle once at
the end, as `ShapeSymbol.getDimension` does. Two quirks are load-bearing and
pinned by tests:

- `getStaffInterline` is `rint((pointSize + 2) / 4.0)` -- the `+ 2` puts every
  standard size on a rounding **tie**, so interline 21 answers 22 while 20
  answers 20, and the num/den gap inherits the parity. The expected value was
  written wrong on first try again; the tie table is not intuition-safe.
- The composite box is centred with **integer-division** halves (`dim/2`), not
  `rint(dim/2.0)` -- Java builds a `Dimension` first, so an odd height loses
  its half downward.

The result cross-checks against reality before any driver exists:
`num_den_dimension(2, 4, il 21)` is exactly the `(36, 87)` box Java's HEADERS
stores on every `TIME_TWO_FOUR` staff of the corpus.

The sweep now grades **14 shapes x 116 sizes** (the three corpus digits joined
it, centroid offsets pinned). `time_classifier.rs` fills the
`HeaderTimeShapeClassifier` seam: noise gate, rank-then-filter over the full
label set, `WholeTimes`/`PartialTimes` mappings, and the two symbol-bounds
constructions -- `AbstractInter`'s rint-halved font box for COMMON/CUT, the
int-halved composite dimension for num/den shapes. The time classifier reads
the **staff-specific** interline, as clefs do; keys read the sheet's -- all
three choices recorded at their seams.

Multi-digit numbers (`TIME_TWELVE`, `TIME_SIXTEEN`, `TIME_TWELVE_EIGHT`) error
loudly (`FontError::UnsupportedNumber`) rather than guess: their boxes need
glyph-advance composition that nothing grades yet, and a silent skip would
consume rank slots differently from Java. None appear on the corpus.

**TIME is closed: 65/65** -- presence/absence on all staves, the agreed value
(specific shape, numerator, denominator), the symbol box, and `timeStop`
(recomputed from bounds, the predicted getter shadow confirmed).

The driver taught three things worth keeping:

1. **The header runs per real system, not per page.** TIME demands every staff
   of a *system* agree on a value; modelling a page as one system let staves
   without a time veto the ones with -- the first run found no time anywhere.
   The test now iterates GRID's `peak_graph.systems` and scopes columns, clef
   and key stop propagation, and grading to each system, which is also what
   Java's `HeaderBuilder` does. Keys stayed 65/65 through the restructure.
2. **Java's browse windows are barline-limited.** `getRoi` caps its stop with
   `Staff.getBrowseStop`, the first *good, connected* barline. Without that cut
   the ROI runs past the header into the first measure, and the classifier
   happily called batuque's opening notes a 3/4 at grade 0.23 on both staves of
   system 2 -- consistent, so the column accepted it. The oracle now emits
   `bars` rows (good+connected barline abscissae per staff) and the driver
   applies the cut; on batuque staff 3 the window shrinks to a 22 px sliver
   with no viable start, exactly Java's outcome.
3. **`selectClefs` runs after TIME**, so keys browse from the *stored* clef
   range stop, not the recomputed getter value. The test now uses Java's true
   order; keys still grade 65/65.

The `pair_ids` seam (numerator x denominator pairing needs pre-allocated inter
ids) is satisfied by a deterministic discovery pass that replays the exact
classification sequence and harvests the ids -- documented in the test.

`tests/header_corpus.rs` (renamed from `key_headers_corpus.rs`) now grades the
complete header chain and is enforced by CI.

### `header.stop` itself: 65/65, and a fourth getter shadow

The final header stop -- what `Staff.getHeaderStop()` serves to
`SpotsBuilder.eraseHeaderAreas` and `StemScaler.getBuffer` -- is now graded on
every staff, closing the value BEAMS has waited on since the first session.

The first run had **exactly the seventeen time-bearing staves off by exactly
+1**, which is as clean as a diagnosis gets. `HeaderTimeColumn.retrieveTime`
computes its system offset from `Staff.getTimeStop()` -- the *getter*, which
answers the inclusive right edge of `header.time`'s bounds -- while
`setTimeStop` had stored the exclusive `x + width` a moment earlier. Fourth
instance of the store/getter shadow (`getClefStop`, `getKeyStop`,
`getTimeStop`-for-reading, now `getTimeStop`-for-the-offset). The rule stands:
**when Java exposes a field through a getter, the port must call the getter
everywhere Java does.** `StaffHeader::time_stop()` now exists and the column's
return uses it; two unit fixtures that had pinned the exclusive convention were
corrected by the corpus.

**Retired: the beams header-erase caveat.** `header_corpus.rs` now computes the
erase rectangle Java's `SpotsBuilder.eraseHeaderAreas` uses -- system area left,
the first headered staff's `header.stop`, the system's first/last staff lines at
that abscissa -- **entirely from native values**, and grades it against the
`erase` rows of `beam-spots.txt`: all 30 systems across the eight beam sheets
match exactly, with a count assertion so the comparison cannot silently cover
nothing. The `header_erase_cost_is_measured_not_assumed` measurement (5 spurious
clef-sized candidates on chula without the erase) remains as documentation of
what the erase is worth, but the caveat it guarded -- "beams cannot run natively
because the erase needs HEADERS" -- is gone: the spot chain's `HeaderErase`
inputs are now producible without a Java oracle.

### Native BEAMS end to end: closed

`recognize_native_beams` composes the previously isolated pieces from the
native GRID report plus that native `HeaderErase` list: uncleaned-NO_STAFF
`maxStem`, the complete spot chain, system-area/bounds dispatch, per-spot beam
recognition, beam extension, hooks, and per-system grouping. The oracle is used
only after the result exists.

`native_grid_headers_and_beams_match_java_on_every_beam_sheet` grades all eight
beam sheets against both `beam-structures.txt` and `beams-sig.txt`:

- 2739 native spot components and all 30 native header erases reach BEAMS;
- 787/787 raw beams match by system, median, height, grade, and all six impacts;
- final beams and hooks match by system, integer bounds, grade, and all six
  impacts, and every per-system group count matches;
- the sole final-SIG difference remains the already-explained
  BachInvention5 system-6 source beam `(1183,2377,104,11)`, which Java's
  subsequent `MultipleRestsBuilder` replaces with a `MultipleRestInter`.

The older beam test used `BTreeSet` keys and therefore reported 190 versus 189
BeamInter impact vectors on Bach; three duplicates on each side were silently
collapsed. The new gate is a multiset: the honest counts are 193 native
pre-replacement versus 192 in Java's final SIG, differing by exactly that one
source beam.

One correction to the preceding handoff: it said 29 erase systems. The oracle
has 30 rows (3+3+3+5+3+5+2+6), and both the header and end-to-end tests assert
30. LEDGERS now reaches its native builder; its sheet-wide post-analysis is the
next recognition tail. CLI/JSON beam publication can proceed independently.


### KEY: the classifier seam is filled

`key_classifier.rs` implements `KeyShapeClassifier` -- the noise gate, the 110
features, the bundled MLP and Java's ranking, filtered to the two alteration
shapes. It is **simpler than its clef counterpart**: `KeyShapeEvaluation` carries
no bounds, so this seam touches no font at all; `KeyBuilder` places alterations
from slice geometry it already has.

Two details worth not copying wrong:

- `KeyExtractor` hands the classifier **`sheet.getInterline()`**, where
  `ClefBuilder` hands it `staff.getSpecificInterline()`. Both feed the same
  descriptor, so on a mixed-size sheet the two stages normalize the *same glyph*
  differently. That is Java's behaviour, not a tidy-up opportunity.
- `NATURAL` is not an alteration here. A key is built from sharps or flats;
  naturals appear only as a *cancel*, on `KeyBuilder`'s own path. Mapping it
  would let a cancel be counted as a key member.

### A latent divergence the clefStop finding predicted

`getClefStop()` recomputing rather than returning the stored value is not a
curiosity confined to the clef stage: `KeyColumn` uses it to pick the key's
browse start, `browseStart = clefStop + 1`. Rust's `retrieve_keys` was reading
`clef_range.precise_stop()` -- the *stored* value -- so on the nine bass-clef
staves where the two forms differ, the key stage would have begun browsing one
or two pixels off. Nothing had run far enough to notice.

`StaffHeader::clef_stop()` now ports the getter, and `retrieve_keys` uses it.

Fixing it exposed a second, smaller divergence. Java's `getClefStop()` reads the
stored stop **only when `clefRange.valid`**, and `setClefStop` sets stop and
valid together; the old Rust path ignored `valid` entirely. A `key_column`
fixture had been constructing a range with a stop but no valid flag -- a state
the pipeline cannot reach -- and passing because of that leniency. The fixture
now mirrors `setClefStop`.

Worth drawing the general lesson, since this is the third instance: when Java
exposes a field through a getter that does anything other than return it, the
port must call the getter everywhere Java does, not just where the difference
was first noticed.

### Two divergences fixed before the driver, both found by reading Java

Building the driver means filling `NativeKeyStaffContext`, and two of its fields
turn out not to mean what the Rust code assumes.

**1. `interline` is doing the work of two different Java values.** It is read in
two places: at the classifier call, where Java uses `sheet.getInterline()`, and
in the pitch computation, where Java uses the staff's own geometry. One field
cannot be both on a mixed-size sheet. It should be split into a
`classifier_interline` and whatever the pitch needs -- see below -- rather than
having the driver pick one and be wrong somewhere.

**2. The measured pitch is not an interline formula in Java.** The native code
computes

```
measured_pitch = 2 * (centroid_y - staff_mid_y) / interline
```

whereas `Staff.pitchPositionOf` computes, for a point inside the staff,

```
((lines - 1) * (2y - bottom - top)) / (bottom - top)
```

with `top` and `bottom` being `getFirstLine().yAt(x)` and `getLastLine().yAt(x)`
-- the *measured* line ordinates at that abscissa. The two agree exactly when
`bottom - top == 4 * interline`, and a real staff's lines are never separated by
exactly four nominal interlines at every x. So the native formula is an
approximation of Java's, and the error grows with how far the staff departs from
nominal -- precisely the sloped and warped staves where a key alteration's pitch
is most likely to sit near a boundary.

This is the same shape as the clef gutter bug: a scalar standing in for a value
Java evaluates from the splines at a given abscissa. The fix is the same too --
`StaffLineGeometry` is already published and already carries `first_line_y_at`
and `last_line_y_at`, so the key context should take the ordinates rather than
an interline.

Neither is visible on this corpus, since every staff has the sheet interline and
the pitch differences are sub-boundary. That is an argument for fixing them now
rather than after a green run makes them look settled.

**Both are now fixed.** `NativeKeyStaffContext.interline` became
`classifier_interline`, named for its one remaining use, plus a `line_count`.
The pitch comes from a new `StaffPitchGeometry` trait, threaded through
`NativeKeyProposalRecognizer`, which answers `(first line y, last line y)` at an
abscissa as *doubles* -- Java reads the spline with `yAt(double)`, not the
`rint`ed `yAt(int)` that `getOuterRect` uses, so the clef-side
`StaffLineOrdinates` would have been the wrong trait to reuse.

`pitch_position_of` falls back to the old interline form only when the splines
cannot answer at that abscissa, which for a key alteration means the glyph sat
outside its own staff's horizontal extent -- degenerate rather than routine. The
regression test uses lines 79.2 px apart where four nominal interlines would be
80, a 1% departure that is unremarkable on a scan, and asserts the two formulas
disagree there and agree at exactly 80.

### The driver's remaining inputs are now ported

`key_parameters.rs` also carries the three values the driver needs beyond the
extractor set: `max_header_width` (the `projectionWidth` Java passes to
`retrieveKeys`, sheet-scaled 15.0), `max_slice_distance` (0.5), and
`browse_envelope`.

**`browse_envelope` reproduces a bug in Java on purpose.**
`KeyBuilder.getBrowseRect` loops `x` from `xMin` to `xMax` and then evaluates
`staff.getFirstLine().yAt(xMin)` *inside* the loop -- `xMin`, not `x`. The sweep
does nothing; the envelope is decided entirely by the ordinates at `xMin`. The
Rust signature takes those two ordinates rather than a range, so it states the
fact instead of hiding a dead loop. If Audiveris ever fixes it the envelope
widens on sloped staves, and this has to change with it.

Also worth a note for whoever ports the next stage: **interline 21 is the
corpus' most common value and it lands on a rounding tie for three separate
constants** -- `maxClefEnd` (94.5), `yCoreMargin` (10.5) and `maxSliceDist`
(10.5). `Math.rint` sends all three to even. A port using `round()` fails all
three, and I got the expected value wrong on two of them while writing the tests.

### The projection-peak port, which closed every catalogued residual at once

The decisive structural fact, read out of `KeyBuilder.process` rather than
guessed: **Java does not classify its way to a key signature, it counts its way
there.** The staff-free projection is walked for stem-like peaks, the signature
(count and shape family) is inferred from peak count and *spacing* -- sharps two
stems per item, flats one, spacing thresholds deciding which -- and only then is
one slice allocated per expected alteration. Classification happens inside that
structure: candidates from subset enumeration are assigned **best-per-slice** by
rounded centroid, and slices still empty get a second extraction pass at a lower
grade floor with the neighbouring slices' chosen glyphs *erased from the crop*
(`KeyRoi.getSlicePixels`, `cropNeighbors`).

`key_peaks.rs` carries the pipeline as pure functions over `IntegerFunction` --
browseArea, checkSpace, createPeak (with `isStemLike` injected, since it alone
touches the raster), mergePeaks, purgeLightPeaks, inferSignature,
checkPeakDeltas, refineSignature, refineShapeStop, computeStarts,
allocateSlices -- reusing the HiLo finder audiveris-core already had from SCALE.
`classify_key_shapes` was rewritten onto it, with Java's two grade floors
(`keyAlterMinGrade1`/`2` over intrinsic: 0.125 and 0.0125), the `purgeParts`
quirks (`bounds.x == xMax` drop, cap 8 by descending weight), the
`embracesSlicePeaks` gate (half-open on the right, and peak centres are
half-integers), and the trailing-space check for single-item candidates.

Grade history: **34/34 key-bearing failing -> 29 -> 28 -> 20 -> 3 of 65.**
The fixture lesson repeated twice while porting: bare synthetic stems fail
`refineSignature`'s flat-trail requirement exactly as Java would fail them --
the unit fixtures now draw bowls -- and `mergePeaks` joins only truly adjacent
peaks (`min - prevMax <= 1` is zero blank columns).

### Closed: the last three staves, and the bug that hid in an `int`

The first two were Java's **third** extraction pass, `fillMissingAlters`, now
ported inside `check_with_clefs_and_fill`: once the best *compatible* clef is
chosen (a single alteration whose pitch misses its expected position by more
than the delta budget invalidates the whole clef, grades read intrinsic-scaled),
every slice still empty -- or whose pitched grade fell under `keyAlterMinGrade1`
-- is hunted once more in a **pitch window**: the slice rectangle re-centred on
the alteration's theoretical ordinate, `stdGlyphHeight` tall, phase-2 grade
floor, neighbours cropped. Clef supports reach the recognizer via
`with_clef_supports`; with none supplied the pass is skipped, as Java skips it
for a staff with no competing clef.

The last staff was the best find of the stage. The port computed the window
pitch as `expected - areaPitchOffset(FLAT)` = 3 - 1.0559 = 1.944, faithfully to
the formula's *intent*. Java's `KeySlice.setPitchRect` writes it as

```java
int pitch = clefPitches[getId() - 1];
pitch -= AbstractPitchedInter.getAreaPitchOffset(keyShape);
```

-- a compound assignment on an `int`, which Java **silently narrows**: the
result is `(int) 1.944 = 1`, truncated toward zero. The hunt window therefore
sits a full fractional-offset higher than the arithmetic suggests, 7 px at
interline 17. Reproducing the truncation closed the staff; "fixing" it would
diverge on every flat key. This was pinned by measurement, not inspection: the
per-alter boxes added to `ClefProbe` for the purpose showed Java's third alter
at (152,2352,15,42) against the port's (152,2359,15,42) -- same window height,
7 px placement difference -- and Java's `getAreaPitchOffset(FLAT)` probe value
(1.0559375) matched the Rust font derivation **bit-exactly**, eliminating every
suspect but the pitch itself.

The corpus test now asserts presence/absence, fifths, the union box and
`keyStop` on all 65 staves, and is no longer `#[ignore]`d -- it is CI's problem
to keep it green from here.

### Superseded record of the earlier findings

### The driver, and what it found before the pipeline landed

`tests/key_headers_corpus.rs` assembles the whole chain -- GRID, the clef stage,
then the key stage -- and **chains** rather than isolating: `browseStart` comes
from the clef stage's own `clefStop`, as `KeyColumn` does, so the join is under
test too. Only the header start is still supplied from the oracle.

**First finding, now fixed: subset enumeration.** `group_key_parts` merged every
part within `maxPartGap` into one compound. Java's `GlyphCluster.decompose()`
enumerates *subsets* of each connected set. At interline 21 the gap is 31.5 px
and key sharps sit about 20 px apart, so a whole signature collapsed into one
glyph whose width exceeded `maxGlyphWidth` (42 px) and was rejected -- **no key
was found anywhere on the corpus.** `enumerate_key_subsets` now mirrors the clef
side's walk: connected sets in left-abscissa order, seeds by descending weight,
depth-first growth. Order matters, because downstream keeps only the first
`maximum_alters` results.

Two deliberate differences from the clef walk. It prunes on width *and* height,
because the key adapter's `isTooLarge` tests both while the clef adapter tests
only height. And `maximum_component_gap` is now `f64`: it feeds a chamfer
distance that Java compares in double.

That took the grade from **34 of 34 key-bearing staves failing** to **29 of 65
disagreeing**.

**Second finding, fixed: the flat pitch.** `AlterInter.computePitch` treats
flats unlike sharps, and the native code used one formula for both. A sharp's
pitch is simply its mass centroid's. A flat's is the **average of two
heuristics**: the mass-centroid pitch plus `flatMassPitchOffset` (0.65), and the
**area-centre** pitch plus `getAreaPitchOffset(FLAT)`. Java's own comment calls
both heuristic; they exist because a flat's bowl hangs below the line it belongs
to while a sharp straddles it.

`getAreaPitchOffset` is **font-derived**, not tabulated, and the ported font
metrics compute it directly: one pitch step is
`(five-line staff height - one-line height) / 8` measured at point size 200, and
the offset is `(-box.y - box.height / 2)` over that. `STAFF_LINE` (U+E010) and
`STAFF_FIVE_LINES` (U+E01A) are in the codepoint table for this reason -- they
are not musical symbols, they are the font's own ruler.

Note the two points are read differently on purpose: `glyph.getCentroid()`
returns a **rounded** `Point`, so the mass pitch is taken at integer
coordinates, while `getCenter2D()` is exact.

**Third finding, fixed: the candidate purge.** Enumerating subsets without
Java's `KeyExtractor.purgeCandidates` defeats itself. Java sorts candidates by
decreasing grade and drops every *later* one that shares a part, so the best
reading of a piece of ink wins outright. Without it, carmen staff 1 kept both
the correct flat at grade 0.97 **and** an overlapping subset of the same ink at
0.147, forming a two-flat signature whose second alteration sat at pitch 0.113
where -3 was expected -- and the whole key died. Enumerating subsets and purging
them are two halves of one mechanism; porting either alone is worse than porting
neither.

The count cap and the purge must also run in Java's order: purge first, then
truncate. Capping during collection keeps whichever overlapping subsets happened
to be enumerated first, which is exactly what the purge exists to decide.

**Progress: `34/34 key-bearing failing -> 29 -> 28 -> 20 of 65 disagreeing`**, so
45 staves now match Java outright.

**Two residuals, debugged. They are the same missing machinery.**

Instrumenting the extraction on both, and reading the component lists directly:

*BachInvention5 staff 1.* Java's key is `(271, 359, 46, 76)` -- three flats. The
port produces exactly **one candidate**, so the purge is not over-reaching after
all; the other two flats never become candidates:

```
PART 0 left=271 top=381 w=17 h=45 weight=351   -> accepted, grade 0.767
PART 1 left=286 top=359 w= 7 h=43 weight=201   -> GATE-REJECT: width 7 < minGlyphWidth 8.5
PART 2 left=295 top=380 w=23 h=55 weight=454   -> passes the gate, never classified as FLAT
```

The ink is *fragmented*: part 1 is a 7-pixel-wide splinter. The subset `{1,2}`
that would reunite it spans y 359..435, 76 px, over `maxGlyphHeight` (64.6), so
the enumeration prunes it -- and Java's `isTooLarge` would prune it too. So Java
is not finding these in the first pass either.

*carmen staff 1.* The port's box is `(358,451,21,51)` against Java's
`(359,451,20,51)` -- one pixel wider on the left, same right edge. The trace
shows this is **the connected component itself**, `PART 1 left=358 w=21`, not a
compounding artefact. So the difference is upstream of everything ported so far.

**Both point at `KeyBuilder`'s slice phase, which is not ported.** After the
first pass, Java builds `KeyRoi` slices from the candidates found and then calls
`extractAlter` again per slice, with two things the first pass does not have: a
*lower* grade floor (`Grades.keyAlterMinGrade2`) and `cropNeighbors = true`,
which removes pixels belonging to adjacent slices before rebuilding the glyph.
That is exactly the mechanism that would recover a fragmented flat on a poor
scan, and exactly the mechanism that would shave one pixel off a glyph's left
edge where it abuts its neighbour.

Note BachInvention5 is the corpus' only JPEG and its only 17-interline sheet --
the sheet where fragmentation is most likely, which is consistent.

So the next step is not another tweak to the enumeration or the purge; it is
`KeyRoi`, `KeySlice` and the second `extractAlter` pass. `NeutralKeySlice`
already exists to hang them on.

**The residuals as measured:**

1. *Seven boxes one pixel wide on the left* -- `x - 1`, `width + 1`, identical
   right edge, ordinate and height. The key itself is correct. A stray
   low-weight component joining the compound on its left is the obvious suspect:
   `minPartWeight` is only 4 px at interline 21.
2. *Eleven staves on BachInvention5 where the port finds one alteration and Java
   finds three.* Java reads 46x76 boxes; the port reads about 17x46 -- one flat,
   not a three-flat signature. This is the corpus' only 17-interline sheet. The
   likely mechanism is the purge over-reaching: a subset spanning two adjacent
   flats can outscore either flat alone, and keeping it removes both
   individuals. If so, Java is protected by something the port still lacks --
   most plausibly `KeyRoi`'s slice structure, which constrains where an
   alteration may begin, so a two-flat subset never competes as a single
   alteration at all.

**Superseded, for the record: one alteration per slice.** This is what instrumentation
was for, and it answered cleanly. On carmen staff 1 the classifier identifies
the flat correctly -- box (358,451,21,51) at grade 0.97, against Java's
(359,451,20,51) -- and the pitch check now passes it at 0.631 against an
expected 0. The key is *still* rejected, because a **second** alter is proposed:
an overlapping larger subset, (349,451,30,65), which the classifier also calls a
flat at grade 0.147. Together they form a 2-flat signature whose second
alteration sits at pitch 0.113 where the second flat is expected at -3, so the
whole candidate dies.

Java does not accumulate every accepted subset. `KeyRoi` divides the browse
range into slices and `keepCandidate` retains only the **best glyph per slice**,
so two overlapping subsets of the same ink compete and one wins. The native code
appends both. Porting that reduction is the next step, and `NeutralKeySlice`
already exists to hang it on.

The residue before that lands:

```
24  no key where Java found one   -- of which 23 are FLAT keys (-1, -2, -3)
 4  key found, box off by 1 px in x or width
 1  key found where Java found none
```

Flats are systematically missing and sharps are not: 23 of the 24 absences are
flat signatures, and the single sharp absence is one staff of one page. Size is
not the cause -- a one-flat key measures 20 x 51 against bounds of 10.5..42 wide
and up to 79.8 tall.

So the lead is flat-specific. First place to look is the pitch: the positions a
flat occupies differ from a sharp's, so `maximum_delta_pitch_one`/`_four` and
the pitch each candidate is measured against are the suspects. Second is
structural -- Java runs **two `ShapeBuilder` passes**, one per key shape, each
with its own ROI and slices, and the native code iterates `[Flat, Sharp]` inside
a single pass. Whether that is faithful is worth checking directly rather than
assumed.

Leave the four 1-pixel box differences until the flats are found; a changed
candidate set will move them anyway.

## Open threads, in the order worth taking them

### 1. Staff-line filament assembly and SIG grades (CLOSED)

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

Closed completely. **Every barline inter now reproduces Java's intrinsic and
contextual grade on all nine example pages**, alongside the medians and the
core fields, so `SIG_PAGE_LEDGER` is zeros throughout.

The last six were one wrong rounding mode.

`StaffProjector.computeProjection` bounds each column by
`firstLine.yAt(x)` and `lastLine.yAt(x)`, and `StaffFilament.yAt(int x)` is
`(int) Math.rint(yAt((double) x))`. **`Math.rint` rounds a half to even; Rust's
`f64::round` rounds it away from zero.** The port used `round`. The two differ
only when the ordinate lands exactly on a half, and then by one row -- which
moves the projection's vertical bound by one and its accumulated pixel count by
up to one. One character of difference, `round` to `round_ties_even`.

That explains the signature that made this look structural. Six of 420
barlines differed, every one of them the leftmost or rightmost of its staff,
because that is where a staff line is extrapolated past its defining points --
and an extrapolated straight line lands on a half far more often than a fitted
spline through real ink does.

#### How it was found, and what it says about diagnosing by inspection

Two earlier rounds of source reading produced three hypotheses, and all three
were wrong: the chunk thresholds, `getChunk`'s out-of-image guard, and "the
staff-vertical impacts measure something differently at an extreme abscissa".
The residual was in none of them. It was in the *input* to the impacts.

What settled it in one run was measuring rather than reasoning.
`oracle/java/StaffImpactsProbe.java` prints the six impacts behind every
promoted barline, and diffing them against the Rust diagnostic showed that in
all six cases **exactly one integer differed, by exactly one**:

```
                       term          Rust        Java     as a fraction
BachInvention5 st 1    left chunk    0.913043    0.869565    21/23 vs 20/23
D0392410       st 8    stop deriv    0.864865    0.891892    32/37 vs 33/37
D0392410       st10    start deriv   0.837838    0.810811    31/37 vs 30/37
carmen         st 3    left chunk    0.923077    0.884615    24/26 vs 23/26
carmen         st10    left chunk    0.875000    0.916667    21/24 vs 22/24
cucaracha      st 6    right chunk   0.840000    0.880000    21/25 vs 22/25
```

Two different terms, both signs, always ±1 on an integer read from the
projection. That points at the projection itself rather than at either
consumer, which is what a hypothesis about the chunk lookup could never have
reached.

The probe needs no change to the production tree, twice over:
`AbstractInter.getImpacts()` keeps the `GradeImpacts` the peak was built with,
so the promoted SIG still carries them. It does have to drive the step engine
itself rather than use Audiveris's `-run` hook, because that hook fires after
the book is stored and its sheets disposed, and reloading `sheet#1.xml` gives
back inters whose impacts are `null` -- the XML persists only the product. The
probe sets `Main.cli` reflectively for the same reason `-run` exists: the step
engine reads it mid-step.

Run it as:

```sh
unset JAVA_TOOL_OPTIONS
JAVA_HOME=/path/to/jdk25/Contents/Home ./gradlew --no-daemon -q \
  -I rust/oracle/java/staff-impacts.init.gradle :app:staffImpactsProbe \
  -PimpactPages="data/examples/carmen.png data/examples/cucaracha.png"
```

`diagnose_sig_grade_residuals` in `recognize.rs` prints the Rust half.

### 2. PDF ingest (reading is done; rendering is what is left)

Audiveris renders PDF pages through PDFBox with
`renderImageWithDPI(page, 300, ImageType.GRAY)` under `ANTIALIAS_OFF` and
`INTERPOLATION_BICUBIC` (`ImageLoading.PdfboxLoader.getImage`). So this is
reproducing a rasterizer, not writing a set of decoders, and the sequencing was
to settle the rasterizer first, then the file format, then the composition.

**Two of the three are done.**

#### The oracle

`oracle/java/PdfPageProbe.java` -> `oracle/pdf-pages.txt`. It renders the corpus
through the exact call Audiveris makes and pins, per page:

- `image` -- each drawn XObject's declared geometry and filter chain, an
  FNV-1a-64 of its **raw bytes as they sit in the file**, an FNV-1a-64 of those
  bytes with the **filter chain applied**, and one of the decoded raster.
- `draw` -- the six-term `AffineTransform` Java2D receives, at 17 significant
  digits, read out of a `PageDrawer` subclass.
- `page` -- the boxes, the rotation, the rendered size, and an FNV-1a-64 of the
  rendered page.

Four depths, so each layer can be finished and graded before the layer above it
exists. That is what made the rest of this quick. Run it with
`-Dlogback.configurationFile=rust/oracle/java/logback-quiet.xml`, or PDFBox's
own diagnostics land in the data -- and read them, because each one names a
leniency the port has to reproduce.

The corpus is not in the repository: 20 MB of scans, listed with download URLs
in the `imslp-pseudo` repo's `manifests/acquired_scans.json`. Point
`AUDIVERIS_PDF_CORPUS` at a directory holding them and
`cargo test -p audiveris-pdf --test corpus` runs; without it, it prints that it
skipped rather than passing quietly.

#### The rasterizer: done

`transform.rs` reproduces Java2D's bicubic image transform bit for bit: 112 of
112 synthetic cases, pinned by `oracle/java2d-bicubic.txt`. Five things had to
be right, none guessable from "bicubic": the Mitchell-Netravali kernel with
`A = -0.5`; its 513-entry table whose tail above index 384 is *derived* so each
group of four sums to one rather than evaluated from the polynomial;
fixed-point arithmetic with coefficients scaled by 256, a `1 << 15` rounding
bias and a `>> 16` with saturation; 32.32 fixed-point coordinate stepping with a
half-pixel subtraction before both the gather and the interpolation; and
branchless sign-bit edge clamping that duplicates the border row or column.
Also: destination pixels whose centre maps outside the source are never written,
which is why a page render has a black margin rather than an extrapolated one.

#### Reading the file: done, 189 of 189

`document.rs`, `lexer.rs`, `object.rs`, `filter.rs`, `flate.rs`, `ccitt.rs`,
`jbig2/`. Against PDFBox on all 189 pages of the seven sampled sources:

| Layer | Result |
| --- | --- |
| Page count, media box, crop box, rotation | 189/189 exact |
| Image geometry, depth, filter chain | 189/189 exact |
| Raw stream bytes, by hash | 189/189 exact |
| Decoded stream bytes, by hash | **189/189 exact** (93 CCITT G4, 95 JBIG2, 1 Flate) |
| Image samples, by hash | **189/189 exact** (188 one-band gray, 1 three-band RGB) |
| Rendered page size in pixels | 189/189 exact |
| The transform Java2D receives, all six terms | **189/189 exact**, sign of zero included |
| The rendered page, by hash | **189/189 exact** |

Everything is ported from PDFBox's and jbig2-imageio's own source, fetched as
`-sources.jar` from Maven Central, rather than from the specifications. Same
reasoning as libjpeg 6b versus turbo: the target is the bytes Java produces.
The places that cost *output* rather than merely robustness, all commented at
their sites:

- **`/Length` is often a lie.** Three of the seven sources declare `/Length 0`
  on streams that are not empty. PDFBox logs "Suspicious stream length" and
  scans for `endstream`; the port validates the declared length by checking what
  follows it and falls back the same way. The raw-bytes hash is what pins this.
- **CCITT is TwelveMonkeys' decoder as PDFBox vendors it**, and three of its
  behaviours are in neither T.4 nor T.6. An unrecognised two-dimensional mode
  code *restarts the mode read* instead of failing. A run code that decodes to a
  negative value returns the full row width. A row that meets the end of the
  data is dropped whole, not truncated. Also `/Rows` is discarded when the image
  dictionary carries a `/Height`, and with `/K` at zero PDFBox *sniffs* the
  first twenty bytes for an end-of-line code to choose between T.4 and modified
  Huffman.
- **`FlateDecode` keeps the prefix of a corrupt stream.** Pinned against PDFBox
  at all eleven truncation points of a test stream, where the two agree exactly.
- **JBIG2's arithmetic decoder reads -1 past the end of its data**, not the
  0xFF the standard specifies, and folds it into a `long` code register. A
  damaged stream diverges from a standards-following decoder immediately.
- **JBIG2 output is the page bitmap inverted**, with the bits past each row's
  width cleared, because the raster's colour model has index zero as black.

**JBIG2 scope was set by measuring.** Dumping segment types across all 95 JBIG2
images found exactly three -- page information, one arithmetic symbol
dictionary, one immediate text region -- with flag words 0 and 16, and no
globals. So Huffman coding, refinement, halftones, striped pages and standalone
generic region segments are refused *by name* rather than half-written. The
generic region decoding procedure itself is complete (templates 0-3, adaptive
pixels, typical prediction), because every symbol bitmap goes through it. Do the
same measurement before extending it: `Document` can now extract the streams, so
a twenty-line probe answers "what does this actually use" in a minute.

#### Samples: done, 189 of 189

`raster.rs` is `SampledImageReader`: decoded bytes to samples, which is the rung
between the filter chain and the page. It was worth doing before anything above
it because the oracle *already recorded it* -- `PDImage.getImage()`'s raster,
hashed band-interleaved and row-major -- so it cost a grader rather than a new
oracle, and it splits sample conversion off from geometry. When the composed
page is first wrong, the half it is wrong in is already settled.

Scope was measured the way JBIG2's was. Across all 189 images there are exactly
four shapes: 177 one-bit `DeviceGray` with no `/Decode`, 11 the same with
`/Decode [1 0]`, and one 4-bit `Indexed` over `DeviceRGB`. No `/ImageMask`, no
colour-key `/Mask`. Anything else is refused by name through
`Error::UnsupportedImage`.

Three things in it are load-bearing and none is implied by "unpack the bits":

- **`from1Bit` returns one band, not three.** For `DeviceGray` PDFBox builds a
  `TYPE_BYTE_GRAY` image and returns it before any colour space runs, so the
  page later draws a gray source into a gray destination. That is what makes
  the `ScaledBlit(ByteGray, SrcNoEa, ByteGray)` trace in item 4 below legible.
- **A short row ends the image and leaves the rest black**, rather than
  truncating the raster: PDFBox logs "premature EOF, image will be incomplete"
  and breaks, keeping the rows it has.
- **The indexed palette is not a byte copy.** `initRgbColorTable` sends every
  entry through `byte / 255f` and back through `(int)(x * 255f)`, in `float`
  with a truncating cast. It is written the long way for that reason.

#### The content stream and the draw transform: done, 189 of 189

`content.rs` and `affine.rs`. Every one of the 189 draws now reproduces the
six-term transform Java2D receives, exactly, at the oracle's full 17
significant digits -- **and the sign of every zero**, which is checked
separately because `-0.0 == 0.0` would let a wrong answer pass.

The operator set was probed rather than assumed, and the probe paid for itself
twice. There are exactly four operators and exactly two page shapes:

```
  36 x  cm Do
 153 x  q cm Do Q
```

So 36 pages never push a graphics state at all: they concatenate straight onto
the initial CTM. Anything outside that set is refused by name, because
silently skipping an operator that moves the CTM would misplace the image and
read like a rasterizer bug.

Three float questions decide the answer, and the third was the one that had
already cost a debugging round:

- **The CTM is a `float` matrix.** PDFBox's `Matrix` holds `float[9]` and `cm`
  multiplies in `float`, so a `cm` operand of `633.5724` is really
  `633.57238769531250`. `content::Matrix` is `f32` for that reason and widens
  only where `createAffineTransform` does.
- **The DPI scale is a `float` division.** `renderImageWithDPI` passes
  `dpi / 72f`, so a 792 pt page renders **3299** pixels tall, not 3300. The
  `page` records' `render` size now grades this on every page.
- **`AffineTransform` is a state machine, and it is load-bearing.** Every
  mutator dispatches on cached bits describing which of translate, scale and
  shear are present, and the branches are not algebraically identical -- they
  drop terms known to be zero. Dropping `+ 0.0` is a no-op for every double
  except `-0.0`. The page transform reaches `concatenate`'s scale-only case,
  which computes `m10 = T10 * m11` with `T10` at `+0.0` and `m11` negative from
  the y flip, giving **`-0.0`** -- which is what the oracle records on all 189
  draws, and what a closed-form composition gets wrong. `affine.rs` ports the
  state machine rather than the algebra for exactly this reason, and its tests
  pin both the `-0.0` and the closed-form counter-case.

#### Composing the page: done, 189 of 189

`render.rs` runs the whole chain and **every** page reproduces Java's rendered
raster bit for bit. That closes PDF ingest: all four depths the oracle records
are now graded, and all four are exact on all 189 pages. The destination is the easy half: `ImageType.GRAY` is
`TYPE_BYTE_GRAY`, one band, and `renderImage` clears it to `Color.WHITE` first,
so an unwritten pixel is 255 and the margins stay white.

**Java2D's primitive selection was the open question, and the answer is not in
Java2D.** The note here previously pointed at `DrawImage`'s `transformState`
ladder. That ladder is a dead end: `DrawImage.renderImageScale`, the only route
to a `ScaledBlit`, opens with

```
// Currently only NEAREST_NEIGHBOR interpolation is implemented
// for ScaledBlit operations.
if (interpType != AffineTransformOp.TYPE_NEAREST_NEIGHBOR) return false;
```

so under a bicubic hint no transform whatsoever reaches a `ScaledBlit`. What
changes is the hint, and **PDFBox changes it**, per draw, in
`PageDrawer.drawImage`:

```
boolean isScaledUp =
    bim.getWidth()  <= abs(round(ctm.getScalingFactorX() * xformScalingFactorX)) ||
    bim.getHeight() <= abs(round(ctm.getScalingFactorY() * xformScalingFactorY));
if (isScaledUp) graphics.setRenderingHint(KEY_INTERPOLATION, NEAREST_NEIGHBOR);
```

and restores the hints straight after, which is exactly why the earlier probe
found the hint reading `Bicubic` both before and after. The port computes the
same predicate and it selects **exactly 10 of 189 draws**, independently
matching what `-Dsun.java2d.trace=count` counted. That agreement is the
evidence; the rule was derived from source and the count was measured before
the two were compared.

**A second PDFBox path has to stay switched off, and it is fragile.**
`drawBufferedImage` abandons `drawImage` entirely and pre-scales through
`Image.getScaledInstance(w, h, SCALE_SMOOTH)` -- a different resampler --
when a scale falls below `imageDownscalingOptimizationThreshold`, which
defaults to **0.5**. Corpus pages scale by about 0.93, so the threshold is not
what saves us; the branch also demands
`VALUE_RENDER_QUALITY.equals(getRenderingHint(KEY_RENDERING))`, and Audiveris's
hints carry only `ANTIALIASING` and `INTERPOLATION`. If Audiveris ever sets
`KEY_RENDERING`, or renders where a scale drops under 0.5 with that hint
present, the resampler changes and none of `transform.rs` describes the output.
`render::hints_reach_the_downscaling_workaround` states the condition so it can
be checked rather than remembered.

The ten scaled-up draws are done too. `scaledblit.rs` ports OpenJDK's
`ScaledBlit`, and all ten reproduce Java exactly. It is worth knowing why a
general nearest-neighbour resampler would not have: the loop steps the source
coordinate in fixed point and accumulates error linearly, so rather than widen
the arithmetic OpenJDK **re-derives the source origin exactly at the start of
every tile**, with `findpow2tilesize` choosing a power-of-two tile small enough
to bound the drift. The result is nearest-neighbour with a periodic exact
resynchronisation. Its rounding is `ceil(x - 0.5)`, a round-half-*down*, not
the `floor(x + 0.5)` that `Math.round` gives. The destination bounds are found
by `refine`, which searches rather than solves, because the forward and inverse
mappings are not exact inverses in floating point.

`render.rs` also ports `DrawImage`'s `tryCopyOrScale` ladder, which transforms
three source corners and decides from those rather than from the matrix. The
plain-`Blit` case and the sheared-nearest case are refused by name; no corpus
draw reaches either.

The corpus's one three-band page is done too, and its order is the part worth
remembering: `renderImageXform` transforms into an `IntArgbPre` intermediate and
only then blits that to `ByteGray`, so **each channel is interpolated in colour
and the reduction to gray happens after**, not before. The reduction is
OpenJDK's fixed-point luma from `ByteGray.h`,
`(77r + 150g + 29b + 128) / 256` -- not a colour-space conversion. That formula
is also why a gray source survives the same round trip untouched: at
`r == g == b == v` it is `(256v + 128) / 256`, which is `v` for every byte.

#### Wired into the load path

`audiveris-cli -batch -step GRID score.pdf` works. `ingest::Loader` is Java's
`ImageLoading.Loader`: an input is a **book of sheets**, not an image, sheet ids
are one-based, and only a PDF supplies more than one. `-sheets` selects a
subset; an empty selection is every sheet.

Two details are Java's rather than convenient:

- **The dispatch is on the file extension**, case-insensitively, not on magic
  bytes. `ImageLoading.getLoader` tests `.pdf` and sends everything else to
  ImageIO, so a PDF named `.png` fails there -- and sniffing the header here
  would make the port accept an input Audiveris rejects.
- **`-sheets` consumes every following non-`-` token and fails on one that is
  not a sheet spec.** That is `CLI.IntArrayOptionHandler`, which calls
  `NaturalSpec.decode` on each and lets it throw, so `-sheets 2 score.pdf` is an
  error in Java too. Put inputs before `-sheets`.

`crates/audiveris-image/tests/pdf_ingest.rs` pins the seam, and is deliberately
redundant with the PDF crate's own corpus test: for all 189 sheets, the raster
the load path hands binarization has the same FNV-1a-64 as the page PDFBox
rendered. The two crates prove different things -- one that the render is right,
the other that the ingest does not then change it -- and the gap between them is
where `Picture.adjustImageFormat`'s maximum-channel rule would have hidden a
conversion. It is the identity here only because `max(v, v, v)` is `v`.

A first real run: page 2 of `IMSLP00709-Schumann_.pdf` reaches GRID with 12
staves in 6 systems and 112 barlines, in about two seconds. Nothing grades that
yet -- see below.

#### Recognition on a PDF sheet is graded too

`oracle/grid-pdf.txt` closes the gap that "the pixels are right" left open.
Eleven corpus sheets run through GRID in live Java, and the port reproduces
**all of it**: staff geometry, and all 392 promoted barlines with their shape,
width, frozen flag, staff-end mark, median, intrinsic grade and contextual
grade. Grades are compared at **1e-9**, not the 5e-4 the example corpus uses,
because this oracle reads the live SIG rather than the three decimals
`sheet#N.xml` persists.

The sheets span the render regimes deliberately rather than by sampling: JBIG2
with and without shear, CCITT plain and with `/Decode [1 0]`, the one Indexed
three-band page, and a sheet from `IMSLP57453`, whose ten pages all take the
nearest-neighbour `ScaledBlit` instead of the bicubic transform.

Four of the eleven are sheets Java **refuses** -- covers and title pages, where
it raises `No regularly spaced lines found` rather than returning an empty
sheet. That is recorded rather than skipped, and asserted: the port has to fail
on the same sheets. Getting a refusal where Java recognises ten staves would be
just as wrong as the reverse.

Regenerate with `oracle/java/GridPdfProbe.java`, whose arguments are
`<path>:<sheet>` with the sheet counted from one.

#### What is left

1. **Widen the corpus.** Everything here is graded against seven IMSLP sources
   whose 189 pages contain exactly four image shapes and four content-stream
   operators. Every refusal is by name, so a new source fails loudly rather than
   silently, but the honest description of the current state is "exact on what
   was measured", not "complete".
3. **Regenerating the oracle needs a JDK**, and this machine has none. The
   checked-in `oracle/pdf-pages.txt` is enough to run the test, but if the
   probe ever changes, that has to happen where Java is.

Use `-Dsun.java2d.trace=count` first for anything of this kind. It answered in
one run what two rounds of source reading did not.

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

## Port-owned SIG: the slice map (started 2026-08-12)

The blocker for a headless STEMS is that the port owns no SIG. This is the plan, written
down before starting so an interrupted session resumes rather than re-derives.

**The verification target already exists.** `rust/oracle/stems-beam-sig-snapshot-chula-system1.txt`
holds Java's 221 vertices and 202 edges for chula system 1 in JGraphT insertion order, frozen
under two byte-identical passes. Measured, the insertion order **is stage order**:

| ordinals | stage | contents |
| --- | --- | --- |
| 0-32 | GRID | BraceInter 1, BarlineInter 22, BarConnectorInter 10 |
| 33-42 | HEADERS | ClefInter 2, KeyAlterInter 4, KeyInter 2, TimeWholeInter 2 |
| 43-110 | BEAMS | BeamHookInter 17 and BeamInter 31 interleaved, then BeamGroupInter 20 |
| 111-118 | LEDGERS | LedgerInter 8 |
| 119-220 | HEADS | HeadInter 102 |

Edges, 202 total: Exclusion 68, Containment 52, BeamBeamRelation 44, NoExclusion 20,
BarConnectionRelation 10, BarGroupRelation 4, KeyAltersRelation 2, ClefKeyRelation 2.

So the SIG can be built one stage at a time, each slice checked against its own ordinal range
rather than needing the whole graph at once.

**What already exists.** `GridSig` (`audiveris-image/src/grid_sig.rs`) is a real SIG: it assigns
sequential inter ids, holds `nodes: BTreeMap<GridInterId, GridSigNode>` (id order == insertion
order) and `edges: Vec<GridSigEdge>` in insertion order, with relations NoExclusion,
BarConnectionSupport and BarGroup. `HeadlessGridSigState` carries it per system. So "the port
owns no SIG" is true across stages but **not** of GRID -- GRID's is the seed to grow.

**Slice 1 (DONE 2026-08-12): GRID's SIG already agrees with Java, bar the brace.** No plumbing was needed after all: `PeakGraphReport` already
carries `pub sig: HeadlessGridSigState`, so the SIG is reachable as
`grid.peak_graph.sig` today. `grid_sig_matches_javas_opening_vertices` shows chula system 1
holds **32 of Java's 33 opening vertices in the same order** -- 22 verticals then 10
connectors, matching Java's counts and its all-verticals-before-any-connector ordering.
The single gap is the **brace at ordinal 0**, which the port keeps in `brace_sig.rs` rather
than in `GridSig`, exactly as the caveat predicted. The test asserts the gap
(`nodes.len() == opening.len() - 1`) so it fails the moment the merge lands.

**Slice 1b (DONE 2026-08-13): the brace promotes in production, 33/33.** Java's
`buildBraces` now runs in the production GRID path (`recognize.rs`, immediately after the
live brace stage and before `ProductionProcessBars` consumes the systems -- which is Java's
own position for it, after topology completion and before `purgeLeftOfBraces` applies).
`build_and_promote_system_braces` went `pub` with a doc note that its fallback
middle-promotion never mutates on this path because `complete_detached_brace_topology`
already set every middle. `all_brace_sections` comes from `sections_by_width` at
`maximum_section_width` (Java's cached `getSectionsByWidth(maxBraceThickness=1.0
interline)`); the per-peak members come from the live stage's retained
`DetachedBraceFilamentEvidence.members`, materialized from the lags by `(lag, id)`. The
filament parameters are `interline`, `segment_length = rint(1.0*interline)`,
`left_margin = rint(0.5*interline)` (Java `braceLeftMargin`). `PeakGraphReport` gains
`brace_promotions: Vec<BracePromotion>` and `brace_sig: BraceSigStore`.
`grid_sig_matches_javas_opening_vertices` now asserts **brace + GridSig == all 33 of
Java's opening vertices** for chula system 1, brace first. The store's glyph/inter ids are
its own (Java ids are not fabricated); the snapshot's structural token carries no id, so
ordinal + content is the comparison, which is the point of the exercise.

**Slice 2 (subsequently completed): HEADERS, ordinals 33-42.** Measured from the snapshot, chula system 1:
vertex order is both clefs (staff 1 then 2), then staff 1's two KeyAlters followed by its
KeyInter, then staff 2's, then both TimeWholes -- i.e. Java's column order (clef column,
key column, time column), not per-staff order. Eight edges: per staff,
`KeyAltersRelation(alter1 -> alter2)`, `Containment(key -> alter1)`,
`Containment(key -> alter2)`, `ClefKeyRelation(clef -> key)`. The TimeWholes have no
header-internal edges. `recognize_native_headers` already emits the selected clef/key/time
inters with lifecycle evidence; the slice is appending them to the per-system vertex list
after GRID's 33 and materializing those four relations per staff, then extending
`native_sig_baseline.rs` to assert ordinals 33-42 and the edge set.

**Measured 2026-08-13, and it simplifies everything: the baseline SIG has ZERO cross-stage
edges.** Every stage's subgraph is closed -- GRID 34 edges, HEADERS 8, BEAMS 102, LEDGERS 0,
HEADS 58, total 202. So the merged SIG is pure concatenation of complete per-stage
subgraphs: no stage needs another stage's identities to build its edges, and each slice can
be verified in isolation. (STEMS is exactly the stage that introduces the first cross-stage
edges -- beam-stem, head-stem -- which is also why it needs the whole graph.) GridSig's
system-1 edge count is already 34, matching Java's GRID subgraph exactly.

**Slice 3 scouting (2026-08-13):** the SIG-shaped record already exists inside
`beams_step.rs`: `NeutralBeamSystem` carries `inters: Vec<NeutralBeamInter>` in SIG source
order (ids + kinds Beam/SmallBeam/BeamHook/MultipleRest/VerticalSerif) **and**
`relations: Vec<NeutralBeamRelation>` plus `beam_group_ids` -- i.e. exactly the
creation-order interleave and the edge record the slice needs, maintained by deltas as the
step runs (hooks probe TOP then BOTTOM per beam in SIG source order, beams_step.rs:933).
`NativeBeamRecognition` does NOT surface it -- it exposes separate beams/hooks lists and
group index sets. Before wiring anything, answer: is `beams_step` the production path of
`recognize_native_beams`, or a differential harness around a `visual` (Java-fed) twin? The
`self.visual.build_hooks(...)` shape suggests the latter. If it is a harness, slice 3 means
surfacing the *native* half's final NeutralBeamSystem; if production already runs it, slice
3 is another PeakGraphReport-style plumbing job. Same lesson as slices 1 and 1b either way:
the machinery exists, only the surfacing is missing.

**Slice 3, measured to the rule level (2026-08-13).** `beams_step`'s `VisualBeams` seam has
only a test `FakeVisual` implementor, so it is a lifecycle skeleton, not the production
path; production is the direct path in `recognize.rs` (`recognize_native_beams`). Creation
order is already surfaced there: per system, `raw_beams` (browse+extend order -- the
`HBHB...` interleave, where each browse spot can yield a hook and a beam) followed by
`hooks` (the per-beam TOP/BOTTOM probe pass -- the trailing `HHHHHHH` run at ordinals
84-90). Groups are `group_memberships`. What is NOT surfaced, and not derivable from the
surfaced products: **(a)** the 10 `Exclusion` edges pair adjacent ordinals ((43,44),
(45,46)...) -- hook-vs-beam alternative readings of the same spot, decided at browse time;
**(b)** the 44 `BeamBeamRelation` edges are all within-group but are neither all pairs
(54) nor consecutive members (18) -- they are Java's geometric neighbour checks inside
`BeamGroupInter` construction. Both decisions are made inside the port's browse and
grouping code and discarded. Slice 3 is therefore: record those two pair sets where they
are decided, surface them plus creation order on `NativeBeamRecognition`, then assert
ordinals 43-110 and the 102 edges. Group sizes on chula system 1, for the test:
[1x9, 2, 3x5, 4x3, 5x2] summing 48.

**Slices 3 and 4 DONE (2026-08-13).** BEAMS: the order divergence was component iteration
-- Java sorts each system's beam spots by `Glyphs.byFullOrdinate` (top, then left; its
comment says abscissa, the code wins) while the port browsed components in extraction
order. One sort where `build_glyph_components` returns
(`components.sort_by_key(|c| (c.top, c.left))`, recognize.rs) makes browse order Java's
registration order; the full workspace suite passed on it unchanged. With that, all of
BEAMS derives exactly: 48 members in insertion order, 20 groups, Exclusion = same-item
hook-then-beam adjacency (10), BeamBeamRelation = all within-group pairs minus excluded
pairs (54-10=44). LEDGERS: the port's final glyph bounds are NOT the SIG bounds --
`LedgerInter.computeArea` (Java :235) sets bounds from
`AreaUtil.horizontalParallelogram(median, thickness)`, so only ink that fills the
parallelogram coincides (4 of chula's 8). The materialized inters carry median+thickness;
applying `Rectangle2D.getBounds()` semantics (floor min, ceil max) reproduces Java's 8
ledger vertices exactly, in creation order (per staff, per line index -- not abscissa).
**Slice 5 DONE (2026-08-13): all five slices now derive.** HEADS needed zero new
recording, the fifth time in a row: `heads_in_sig_order` is Java's creation/SIG order,
`beam_removed_heads` are the vertices the beam purge takes back out, and each staff's
`purge.overlap.decisions` carry the (purged, kept) pairs Java joins with an OVERLAP
exclusion when `doRemove` is false. One trap cost a round: decision indices are *creation*
indices -- the purge resolves `ordered_indices[position]` back before recording -- not
input ordinals, and mapping them through `input_ordinal` scatters the pairs. 102 heads and
58 exclusions match exactly on chula system 1.

**The token gate (capstone) is scoped and skeletoned.** The full structural token is
`class:shape=..:grade=hex(grade):bounds=x:y:w:h:removed=..:abnormal=..:manual=..:
implicit=..:profile=..`, plus `:median=hex(x1):hex(y1):hex(x2):hex(y2):height=hex(h)` for
beams/hooks (probe `interStructuralToken`); `hex` is Java `Double.toHexString + "/" +
bits` and the port already has `java_hex_double` (private copies in several files -- reuse
one). Confirmed from the snapshot inventory: flags removed/manual/implicit always false at
this baseline, profile always 0, **abnormal=true exactly for beams, hooks and heads** (no
stem yet); BeamGroupInter shape=null grade=1.0 bounds=union of members; BraceInter
grade=0.8 (intrinsicRatio); KeyInter shape=null bounds=union of its alters. At that
checkpoint the remaining work was to wire and bit-match the per-class grades. That work
is now complete: `assembled_sig_rebuilds_javas_structural_hashes` is an active test in
`native_sig_baseline.rs`, and it rebuilds Java's vertex and edge hashes from the
production-owned SIG. `vertexHash` is SHA-256 over newline-terminated `ordinal:token`
rows, per GraphOrder.

**Token-gate convergence, measured (2026-08-13, `grade_sources_bit_match_java`,
#[ignore]d diagnostic in native_sig_baseline.rs):** of the 221 grades, **182 are already
bit-exact** -- all 102 heads (`grade_bits`), all 48 beams/hooks (`RawBeam.grade`), all 32
barlines/connectors (GridSig `intrinsic_grade`). Clefs: both bit-exact
(candidate `grade`). Times: one exact, one off by last-ulps. **Keys: SIG grade =
candidate.grade x 0.8** (intrinsicRatio -- confirmed numerically, 0.5252/0.6565 = 0.8000).
Alters: source not yet wired (per-alter grades live in key_column's pitched alters, not on
the slice). **Ledgers: 5/8 bit-exact, 3 drift by 13-21 ulp** -- the port's grade formula
associates differently from Java somewhere; find the exact op order in the ledger
impacts-to-grade path. Brace = 0.8 constant, groups = 1.0 constant (verify bits when
wiring). So the remaining convergence work is: 3 ledger ulp-chases, 1 time ulp-chase, the
x0.8 key rule, 4 alter grade sources, 2 constants -- then the token renderer and the hash.

**The ledger ulp-chase, resolved to a named root cause (2026-08-13).** The drift is NOT in
the impact inputs and NOT in the aggregation order. `ledger_ulp_drift_isolation`
(#[ignore]d) re-aggregates each ledger from its stored impacts with computeGrade's exact
formula and reproduces the stored grade bit-for-bit -- so the port is self-consistent, and
a perturbation search that seemed to finger impact[2] was a red herring: with weight 4 it
is merely the finest dial (any ~2^-49 aggregate offset can be expressed through it).
Java's interpolation (`Check.java:262`, `(v-low)/range` then `GradeUtil.clamp` to [0,1])
and bounds match the port's. What remains is the **pow implementation itself**: the
aggregate runs 8 pow calls (`grade^weight` x7, then `global^(1/totalWeight)`); HotSpot's
`Math.pow` is the fdlibm-derived dpow stub while Rust's `powf` is Apple libm, they differ
by ~1 ulp on specific operands, and three ledgers plus one time signature happen to hit
such operands. The frozen ledgers fixture stores grades at 9 decimals, which is why the
old exact gate never saw it. **The pow hypothesis was tested and REFUTED (same day):** `java_pow` (fdlibm e_pow, now
at `src/java_math.rs` with sanity vectors) produces bit-identical results to Apple libm's
`powf` on every operand these grades hit, so pow is not the drift source. The reaggregation
check was also circular -- port impacts reproduce the port total by construction. **The
drift therefore lives in the impact GRADES the port computes**, upstream of aggregation,
and the decisive next step is Java-side per-impact bits: extend the ledgers probe to emit
each impact grade as hex (the frozen fixture's 9 decimals cannot distinguish an ulp),
regenerate chula, and diff impact-by-impact. `java_math.rs` stays -- it cost little, its
vectors document host libm agreement, and the token gate may yet need it for another
class's operands.

**The ledger drift is PINNED to the dy checks (2026-08-13).** `LedgerImpactBits.java`
(now in rust/oracle/java; needs the CLI-reflection preamble, EpsilonGC, probe classpath)
runs the pipeline to LEDGERS and prints every final ledger's grade and per-impact grades
as raw f64 bits. Diffed against the port: **for the three drifted ledgers (x=2132, 1718,
2228) impacts i0-i4 are bit-identical and i5+i6 -- the two dy checks,
`|start/stop.y - y_target|/interline` -- each differ by ~140 ulp**; the five clean
ledgers are bit-identical on all seven. Both endpoints shift together, so the culprit is
the shared term: `y_target` (Java: `yRef + signum(index)*interline`) or the filament
endpoint y, differing by an ulp or two and amplified by the near-cancellation in the
subtraction. Next: print the check VALUES (not grades) both sides for one drifted ledger,
then compare the port's y_target/endpoint computation to LedgersBuilder's line 480 region
op by op. This is minutes of work with the probe now in the tree.

**The ledger ulp-chase is CLOSED: `y_at_x_ext` was not `LineInfo.yAt` (2026-08-13).**
The op-by-op comparison the previous entry called for did not need the probe at all --
reading the Java was enough. `y_target` is `yRef + signum(index)*interline`, and for
every one of chula's eight system-1 ledgers `index` is -1, so `yRef` is
`staffLine.yAt(stick.getCenter2D().getX())` and the previous-ledger branch of
`getYReference` never runs. `StaffLine.yAt` (`sheet/StaffLine.java:345`) is two branches,
and `StaffBoundary::y_at_x_ext` matched neither:

- **In range**, Java calls `getSpline().yAtX(x)` -- `GeoPath.yAtX`, which takes the
  *convenience* parameter `t = (x - p1.x) / (p2.x - p1.x)` and evaluates the Bezier in y
  alone. The port instead called `y_at_x`, a 64-step **bisection that inverts x(t) on the
  true curve**. Those agree mathematically and differ by a few ulp on a curved segment.
  The port already had the faithful evaluation as `geopath_y_at_x` (written for
  `Staff.buildAllLedgerLines`); `y_at_x_ext` simply was not calling it.
- **Out of range**, Java forms `sl = dy/dx` first and multiplies second. The port had
  folded it to `start.y + (dy * (x - start.x)) / dx` -- multiply first, divide second,
  which moves the last ulp.

Fixing both to follow `StaffLine.yAt` op for op takes chula system-1 ledgers from
**5/8 to 8/8 bit-exact** against Java's stored `LedgerInter` grade bits, with no change
to the impact formulas, the aggregation, or `java_pow`. This retroactively confirms the
pow refutation: the drift was in the impact *inputs*, upstream of aggregation, exactly
where the previous entry placed it.

The lesson generalizes past ledgers: **"walks the spline" is not one operation.** Java has
three ways to ask a staff line for an ordinate -- `GeoPath.yAtX`'s convenience parameter,
true-curve inversion, and the endpoint chord -- and the port has all three. A doc comment
naming the Java method is not evidence the body implements it; only a bit-exact
comparison is.

**The header grades: alters wired, and `LineInfo.yAt` bites a second time (2026-08-13).**
Picking up the token-gate convergence after the ledger fix, the remaining gap was entirely
the 10 header vertices (2 clefs, 4 key alters, 2 keys, 2 times). Three things closed most
of it, each measured rather than assumed:

1. **The alter grades were never missing, only discarded.** `pitched_key_grades` computes
   them (Java `computePitchedGrades`) and `lifecycle_key_candidate` kept only their mean as
   the key's `grade`. `NeutralKeySlice` now retains `alter_grade` per slice, which is
   exactly what Java's `KeyBuilder.applyPitchImpact` assigns with `alter.setGrade(
   pitchedGrades[i])`.
2. **The alter's base grade must carry `intrinsicRatio`.** Java's `computePitchedGrades`
   scales `alter.getGrade()` -- the *inter's* grade -- not the raw classifier evaluation,
   and an inter's grade has the 0.8 in it. The port stored the raw evaluation, so every key
   grade was exactly 0.8x off. `KeyAlterClassifierProposal.classifier_grade` is renamed
   `inter_grade` and gets `* parameters.intrinsic_ratio` at creation, precisely where
   `header_time_builder` already applies the same ratio -- which is why the times were
   already right. The min-grade filter still compares raw evaluations, as Java's does
   (`MINIMUM_KEY_GRADE` is pre-divided by the ratio for that reason), and `slice.grade` is a
   max-selection, so scaling cannot reorder it.
3. **`KeyInter`'s grade is the plain mean of its members' grades**, summed in slice order.
   Measured, not assumed: Java's two alters mean exactly to Java's key grade on both staves.
   Scaling the port's precomputed mean by 0.8 instead lands the value to ten decimals but
   not to the bit, because the two associate differently.

That took the headers 3/10 -> 4/10 bit-exact with all ten finally agreeing to ten decimals.
The last structural gap was **the same `LineInfo.yAt` confusion as the ledgers, in a second
place**: `GridPitch::line_span_at` fed `Staff.pitchPositionOf` from
`first_line.y_at_x(x)` -- the true-curve bisection -- where Java reads
`getFirstLine().yAt(x)`. It also returned `Option`, falling back to a nominal-interline
approximation off the ends, where Java simply extrapolates along the endpoint chord.
Switching it to `y_at_x_ext` shifts the measured pitch into agreement and takes the headers
to **6/10**: staff 2 is now entirely bit-exact, alters and key together.

**At that checkpoint four grades out of 221 differed**, all on chula system 1: staff 1's two key alters
(header[2], header[3]), its key (header[4], which is just their mean and cannot be exact
until they are), and one of the two times (header[9]).

**And that residue is now isolated to the glyph classifier, by elimination (2026-08-13).**
`KeyAlterPitchBits.java` (`:app:keyAlterPitchProbe` in `staff-impacts.init.gradle`) prints
every `KeyAlterInter`'s grade and measured pitch as raw bits, together with the staff-line
ordinates and reference points behind them. Three `#[ignore]`d diagnostics in
`native_sig_baseline.rs` walk the chain against it, and every term matches **bit for bit**:

- `key_alter_line_ordinates_against_java` -- `first_line`/`last_line` ordinates at each
  alter's centroid abscissa, all four rows ulp +0. So `y_at_x_ext` is right.
- `key_alter_pitch_chain_against_java` -- `massPitch` and `geoPitch` through
  `Staff.pitchPositionOf`, all ulp +0.
- `key_alter_measured_pitch_against_java` -- the full flat mix
  `0.5 * ((mass + flatMassPitchOffset) + (geo + areaPitchOffset))`, all ulp +0.

The pitched grade is `inter_grade * (1 - delta/maxDeltaPitch)`. `delta` follows from a
measured pitch that is now exact; `maxDeltaPitch` depends only on the alter count, and both
staves' keys have two alters, so it is the same number for all four -- and staff 2's two
alters come out exactly right. Every input to that multiply is therefore verified except
one, so **the divergence is the classifier evaluation for staff 1's two flat glyphs**.

That is a different class of problem from everything else in this chase, and worth saying
plainly: it is the shape classifier's own numerics, not geometry. Note the heads are
bit-exact, but heads go through template matching rather than the MLP, so they are no
evidence about this path. The next step is a probe on the evaluation itself -- print
Java's raw `Evaluation.grade` for those two glyphs and compare against the port's
classifier -- rather than any further work in the key code, which is now exhausted.

Java's side of that comparison is frozen in **`oracle/key-alter-pitch.txt`**: all 12 alters'
grade, measured pitch, ordinates and reference points, *and* each one's full 110-input
`MixGlyphDescriptor` feature vector, as raw bits. No Gradle run is needed to repeat it.

**MEASURED, and it is the ART moments -- not `exp` (2026-08-13).** That comparison has now
been run, and it answers the question the paragraph below poses. `NeutralKeySlice` retains
`alter_raster`, the exact glyph the classifier evaluated (the same reason a final ledger
keeps its raster), and `key_alter_features_against_java` diffs the port's
`mix_glyph_features_from_run_table` against the frozen Java vectors:

- **77 to 90 of the 110 features differ, by 1 to 5 ulp, on all four chula system-1 alters
  -- including staff 2's, whose final grades are bit-exact.**
- Every differing index is **<= 98**. With `ARTMoments.ANGULAR = 20` and `RADIAL = 5`,
  `artCount = 99`, so indices 0-98 are the ART moments, 99-108 the ten geometric moments,
  and 109 the aspect. **The geometric moments and the aspect are bit-exact every time; only
  the ART moments drift.**

So the root cause is the **ART moment computation**, and the sigmoid is exonerated: features
differ on all four alters while only two final grades do, which is exactly what a sigmoid
does to a 1-ulp input perturbation -- usually it rounds away, occasionally it does not. That
also explains why clefs and staff 2 look clean: they are not evidence of correct features,
only of perturbations that happened to vanish.

**CLOSED -- 221/221 SIG grades are now bit-exact (2026-08-13).** The op-by-op comparison
continued through `BasicARTExtractor`, its 101x101 LUT, and the running Temurin 25 math
paths. The ART loops, interpolation, normalization, and accumulation order already matched.
The divergence was entirely in the LUT's transcendental inputs: platform `hypot`, `atan2`,
`cos`, and `sin` do not reproduce the Java runtime's bits. The narrow ART math adapter now
uses OpenJDK fdlibm's `hypot`/`atan2` paths and HotSpot AArch64's fused `Math.cos`/`Math.sin`
instruction schedule, including its medium-range pi/2 reduction. One source detail is
load-bearing: the cosine kernel retains the **original argument's high word** after
reduction when choosing its `qx` rounding branch; recomputing that word from the reduced
argument changes the final ulp.

This was measured at each operation, not inferred from a generic libm resemblance. Hashes
over all 7,825 in-unit-circle LUT cells now match Java for radius, angle, radial basis,
angular cosine, and angular sine. The frozen end-to-end diagnostic then reports **all 110
features bit-exact for all 12 key alters**, including the four chula system-1 alters that
first exposed the drift. `grade_sources_bit_match_java` moves the header ledger from 6/10
to **10/10**, closing staff 1's two alters, their mean `KeyInter`, and the remaining
`TimeWholeInter`; the complete measured SIG grade ledger is now **221/221**.

**The fallback trap remains worth keeping:** `NeuralNetwork.forward` (`math/NeuralNetwork.java:287`)
ends each unit in `sigmoid`, which is `1.0d / (1.0d + Math.exp(-val))`
(`NeuralNetwork.java:558`). `audiveris-classifier`'s `forward` mirrors it exactly, including
the reverse-index accumulation Java uses, and ends in `1.0 / (1.0 + (-sum).exp())`. So the
two candidate sources are the **feature vector** and **`exp`** -- and clefs go through this
same classifier and *are* bit-exact, as are staff 2's flats, so whatever it is, it is
operand-specific rather than a broken layer. Discriminate by comparing the feature vectors
first: if they are bit-identical, the residue is the network arithmetic.

If it does turn out to be `exp`, **do not reach for fdlibm by analogy with `java_pow`.**
`Math.exp` is not `StrictMath.exp`: HotSpot intrinsifies it (`_dexp`, derived from Intel's
LIBM) on both x86-64 and AArch64, and `Math` is only specified to 1 ulp, so the intrinsic is
free to differ from fdlibm and does. Porting `e_exp.c` would therefore reproduce the wrong
oracle -- exactly the "know what your oracle actually is" failure this file already records
for libjpeg. Establish what the running JDK's `Math.exp` actually returns for the operands
in question before writing any replacement, and note that `java_pow` was itself refuted as
a drift source once, so an ulp-level libm hypothesis deserves a measurement and not an
assumption.

An aside worth keeping: `/private/tmp/audiveris-probe.classpath`, which every
`oracle/java/run-*.sh` depends on, was cleaned out of `/private/tmp` mid-session. It is a
cache, not a checked-in artifact, and nothing documents how to rebuild it. Going through
`staff-impacts.init.gradle` instead sidesteps it entirely, since Gradle resolves the
runtime classpath itself; prefer that route for new probes.

**Master's CI was red for six commits, and the cause was a shared vector (2026-08-13).**
The "Rust port" workflow failed on `native_black_head_sizing_matches_java_on_every_beam_sheet`,
shard 0, on both ubuntu and macos, from `501df761a` through `6ca009d2b`. It was never a
platform or fixture problem: `c64c15873` added
`components.sort_by_key(|c| (c.top, c.left))` in `recognize_native_beams_impl` to put beam
creation into Java's `Glyphs.byFullOrdinate` browse order -- correct in itself -- but it
sorted **in place, ahead of `measure_black_heads(spots: &components)`**. Java's
`SpotsBuilder.buildSpots` hands `BlackHeadSizer` the glyph list *as built*, and only the
later per-system `BeamsBuilder.buildBeams` re-sorts, so the port's sizer started seeing the
topmost spot first (chula ordinal 0 became the y=123 `width_low` sliver instead of Java's
y=1845 `width_high` component). Row counts still matched, which is why it read as an
ordering defect rather than a selection one.

The fix keeps `components` in extraction order and sorts a borrowed view for the beam
browse only. Both gates now hold at once: the sizer matches Java again, and
`beams_products_derive_javas_beam_ordinals` -- the gate the sort was introduced for --
still passes. `sort_by_key` is stable, so equal `(top, left)` keeps extraction order.

Two process points worth keeping. `c64c15873`'s message asserts "the full workspace suite
passes on it unchanged"; it did not, and the claim went unchecked because every CI run
between the last green (`78bf90d40`) and `501df761a` was **cancelled** by the next push.
Superseding pushes hide the run that would have caught this, so a red master can persist
for hours while each individual commit looks fine. And **an in-place sort of a vector that
more than one consumer reads is a silent behavioural change to all of them** -- prefer
sorting a view at the point of use.

**What the fix moved elsewhere, and why it was re-pinned rather than reverted.** One
other gate changed: `bach_system_six_produces_one_identity_free_multiple_rest_replacement`
asserts the Bach system-6 multiple rest's `stop_pitch` bits, and they shifted by 3987 ulp
(`0x3fac76cdf933c1d3` -> `0x3fac76cdf933b240`, ~0.0555939070417). `start_pitch` did not
move. That constant is a **port self-snapshot, not a Java-verified value**: it appears
nowhere but that test, and the Java oracle for this rest
(`oracle/heads-scanner-slices.txt`) publishes its grade `0x3fe3dacf882d0517` and bounds
`1183:2377:104:11` -- both asserted in the same test, both still passing -- but never its
pitch. Java reaches pitch through `Staff.pitchPositionOf` (`Staff.java:1692`), which reads
`getFirstLine().yAt(x)` and `getLastLine().yAt(x)`, i.e. exactly the method corrected
here, so the new value is the more faithful one. A pitch this near zero is a
near-cancellation, which is why a one-ulp ordinate becomes thousands of ulp; and pitch is
consumed only as `abs() > MULTIPLE_REST_MAX_ABSOLUTE_PITCH`, so no shift of this size can
change a decision. The rest is still produced with identical ordinal, staff, grade,
median, bounds and serif evidence. The web status page claimed this rest matched Java on
"pitch"; that word was removed, since the oracle never carried it.

The one genuinely red gate on this branch, `native_black_head_sizing_matches_java_on_every_beam_sheet`,
**predates this work**: it fails identically at 6ca009d2b with the change reverted, and
`chula.png` and `oracle/black-head-sizer.txt` are byte-identical between this worktree and
the parent checkout, so it is not a worktree artifact. Its chula candidate row 0 is a
different component entirely (10x40 `width_low` vs the expected 137x13 `width_high`) at
equal row counts, so it is an ordering or selection defect in the sizer, unrelated to
ordinates. It needs its own slice.

**Open, and deliberately not fixed in this slice:** `StaffBoundary::y_at_x` (the
bisection) carries a comment claiming `Staff.distanceTo` calls `LineInfo.yAt(x)`, "which
walks the spline". That is the same conflation -- `distanceTo` reaches `GeoPath.yAtX`
too, so the bisection is likely wrong there as well. It was left alone here because its
remaining callers (`recognize.rs:2716/2724/5432`, `native_stems_beam_stumps.rs:1510`)
have their own gates and deserve their own graded slice. **This prediction has since paid
out once**: `GridPitch::line_span_at` was another such caller, and switching it to
`y_at_x_ext` moved two header alters and a key into bit-exactness (see the header-grade
entry above). Treat every remaining `y_at_x` call as a suspect until its Java counterpart
is read: the question is always which of Java's three ordinate routes it stands for. Note that most `y_at_x_ext`
consumers immediately `round_ties_even() as i32`, which is why this ulp bug could hide
in barline and stem-seed geometry for so long while showing up in a ledger grade.

**The port now owns the SIG through HEADS.** `assemble_native_sig` concatenates the live
GRID, HEADERS, BEAMS, LEDGERS, and HEADS products into one typed graph per system with
stable native vertex and edge insertion ordinals. The chula-system-1 gate renders all 221
full vertex tokens and all 202 full relation tokens from that graph and reproduces Java's
ordered SHA-256 values exactly: vertex `c7a84a6eca6e49477f1bd26f9e93f066d7add49cc1e922b06f86cfd33e9646e6`,
edge `9d55bb9b9db317bbf70d45d25f8ea9aeca8f92b310c19258bc6043ee95630a50`.
This work found a lifecycle omission rather than hiding it in the renderer: native GRID
had not appended `LinesRetriever.addShortSections`'s horizontal sections before
`BarsRetriever`, which left the brace glyph too narrow. Running that stage in Java order
changes the native brace glyph to x=173 / width=22; `BraceInter.getBounds`'s exact
staff-line extrapolation then closes the last token. The assembled edge product retains
support grades and the four BarConnection impacts, not merely endpoint/kind triples.
`NativeSigSystem` now also owns fail-closed `incoming_edges`, `outgoing_edges`,
`incident_edges`, and directed-pair queries. They preserve global insertion order, with
incident reads in Java/JGraphT's incoming-then-outgoing order; a production-only gate
rebuilds the real chula base-apply beam scan exactly and rejects missing vertices.
The graph is now a carrier rather than a read-only snapshot: typed vertex/edge IDs,
dense checked appends, tombstoned vertex/edge removal, abnormal-state updates, and an
integrity validator preserve identities across later removals. Dynamic BeamStem edges
retain beam portion, extension point, and native draft lineage; assembly publishes typed
beam-source and surviving-beam-group vertex bindings. The first carried stem/BeamStem
append produces vertex 221 / edge 202 and the
exact `[54,55,56,57,58,202]` beam incident order.

The next blocker is no longer discovering, reconstructing, or querying the baseline
ordering or carrying graph mutations. B14 now has the first production-owned certificate
projector: given only the native SIG, typed beam binding, B13 relation draft, and plan, it
derives the complete directed-pair and pre/post stem/beam incident scans without Java
rows. `owned_sig_projects_the_first_b14_queries_without_java_rows` projects first, then
reads Java and compares the full post-callback query after canonicalizing away Java's
sheet-global Inter IDs; ordinals 54-58 plus fresh 202, direction, class, lazy-read state,
relevance, endpoint vertex, and LEFT portion are exact. Public B14 now consumes that
certificate directly through
`apply_native_stems_beam_vlink_base_transaction_to_native_sig`: the compact replay state,
owned graph, and bindings are cloned and committed atomically; chula system 1 appends the
exact Stem vertex 221 and LEFT BeamStem edge 202, updates abnormal state, and still matches
the frozen Java transaction. Certificate endpoint identities explicitly distinguish the
legacy Java EntityIndex domain from one-based native vertices, so no fixture-derived ID
map crosses into production. This gate is deliberately chula-system-1-only. Its then-open
Bach system-6 BEAMS-group gap is subsequently closed by Boundary 207 without changing
this historical B14 evidence.

The graph portion of B16 now also derives and commits natively for that first transaction.
`project_native_stems_beam_vlink_sibling_graph` resolves BeamGroup 0 and Stem 143 from
typed bindings, reads group edges `[52,53,54,116,119]`, and simulates the two sibling
links serially on a clone. Sibling 0 appends edge 203; sibling 1's stem scan then sees 203
before appending 204. Source-outgoing, directed-pair, post-stem, and post-beam edge
chronology matches the frozen Java rows only after the native result has returned. The atomic graph wrapper
ends at 222 vertices / 205 edges with `BeamVSiblingDraft {143,0/1}` provenance and exact
grade/LEFT plus typed extension payloads; missing group bindings, a removed base edge, and duplicate
drafts fail closed, with rollback proven. That graph-only projector remains the narrow
query/mutation primitive used by the carrier below.

The first measured B15+B16 transaction is now carried beyond that graph-only layer in
owned typed state. The carrier initializes the complete B-linker cell catalogue exposed
by native reachability, applies B15's `beam:12:b:0` false-to-true assignment, derives
BeamGroup membership and geometry from the owned SIG/stump/V-linker products, selects
each immutable builder item natively, and preserves Java's edge/callback -> builder
lookup -> shared-cell write order. It commits edges 203/204 together with
`beam:0:b:0` and `beam:1:b:0` false-to-true, replacing opaque Java group digests with
typed member vertex/abnormal snapshots. SIG and B cells use one clone-and-swap commit;
invalid carrier input leaves both unchanged. The 12/12 B16 gate opens the frozen B16
rows only after the native result exists and compares group order, all geometry bits,
abnormal changes, edge chronology, and cell aliases. This is bounded to chula system 1
transaction 1.

That same native carrier now continues through the first measured B17 head loop. SIG
assembly publishes authoritative head-reference bindings, while the complete native
head-corner product initializes two persistent S cells per head with TOP/BOTTOM observer
order. Plan 143 writes `head:13:LEFT` and `head:14:LEFT` false-to-true, appends HeadStem
edges 205/206 from native vertices 119/120 to stem 221, retains full typed relation
payload and exact `0x3ffc924924924925` consistency, and performs the serial head/stem
abnormal callbacks. The owned graph ends at 222 vertices / 207 edges; a late missing
second-head binding proves that the provisional first S write, edge, and abnormal changes
all roll back together. `native_carrier_drives_full_sides_pass_before_oracle_read`
opens B16/B17 rows only after the native result exists. This is still chula system 1,
transaction 1, and graph/S-cell scope: sheet/book dirty state and the Java-fixture glyph
bootstrap remain outside the claim.

The first transaction's owned authority now crosses B18 and B19 as well.
`apply_native_stems_beam_outer_and_resume_transaction` derives the selected B's ordered
V facts from native constructor/reachability products, performs the exact idempotent
outer `setLinked(true)` against the same B cell B15/B16 mutated, folds B16's two sibling
cells into the scheduler before walking, and reaches plan 152's RIGHT-side second
frontier. A deliberately invalid post-outer scheduler state proves the cell shadow is
not committed when resume fails. This establishes the honest transition into transaction
2 from the 222/207 graph plus persistent B/S cells.

Transaction 2's B12 preparation is now production-owned rather than handwritten in its
gate. `prepare_native_stems_beam_vlink_frontier_state` takes the carried transaction
state plus the actual plan-152 scheduler frontier, derives its line state, joins its one
selected glyph to the disclosed page-level GlyphIndex bootstrap by full native content,
and promotes `systemStems` only with a completeness token tied to the dense history from
the empty STEMS-entry baseline. The clone-and-swap preparation rejects ambiguous glyph
evidence without mutation; B12 then independently reaches ReuseActive / CreatedChecked
before the txn2 family fixtures are opened. B13 now also has a bounded native live-state
projector: it validates the plan heads through owned bindings and reads the persistent
S cells first. Plan 152's two cells are false, so it emits two exact `NotRead` graph
lookups and independently reaches `AllUnlinked` / `ReadyBeforeSigMutation` before the
txn2 oracle is opened. `roll_native_stems_beam_vlink_base_apply_state` then folds the
prior InterIndex append into a 640-entry native lineage, recomputes the 222/207 baseline
from the owned graph, and creates a fresh one-shot B14 state without txn2 rows. Native
B14 appends Stem vertex 222 and RIGHT BeamStem edge 207, assigns persistent ID 2341, and
matches the frozen grade/extension/result only after returning. The same persistent B/S
arenas then carry B15-B17 without txn2 rows: B16 appends sibling edges 208/209 and links
`beam:2:b:0` plus `beam:3:b:0`; B17 appends HeadStem edges 210/211 from native heads
130/131, links `head:21:LEFT` plus `head:22:LEFT`, and leaves the owned graph at 223/212.
Only then are the frozen B14/B16/B17 results opened. Transaction 2 then crosses the
existing native B18/B19 seam against those same authorities: the outer write is
idempotent, B16's sibling cells are folded before scheduler walking, and the next typed
frontier is plan 618 / `beam:22:b:0` / TOP. The frozen full-pass row is opened only after
that result exists.

Transaction 3 removes the earlier same-base-beam restriction. The frozen
`stems-beam-inter-index-chula-system1.txt` fixture contains all 48 live system-1 beams
(SHA-256 `fde4daebadc5c7158fa8e83dcbd4ac0ca6381c614876b6fe48408ec2e245064e`,
52 lines / 6,259 bytes). The carrier consumes only the 16 distinct selected base-beam
rows that actually reach B14 across the 32 transactions; each supplies Java persistent Inter
ID, sorted InterIndex ordinal, and VIP while native stump/SIG products resolve source,
vertex, group, removal, abnormal state, geometry, and every query. Plan 618's two selected glyphs are materialized
natively. `NativeStemsModeledGlyphRegistry` maps the 1,058 system-1-visible
modeled objects into stable native canonical-ordinal identity and resolves the
compound candidate by exact content without reading the disclosed 1,650-entry
snapshot or its 592 opaque entries. B14 appends Stem
vertex 223 and edge 212, B16 appends
sibling edge 213 and links `beam:41:b:0`, B17 appends HeadStem edges 214/215 and links
the two new head-side S cells, and B18/B19 reaches plan 627 / `beam:22:b:2` / TOP.
The graph is 224/216. `advance_native_stems_beam_sides_transaction` performs one
already-awaited transaction as a production clone-and-swap across scheduler, latest
B14/transaction state, SIG/bindings, and persistent B/S cells. Repeated calls now drive
the rest of chula system 1's SIDES pass from that same authority. The native SIDES terminal is
253 vertices / 331 edges with 32 dense Stem bindings, 61 linked/open B cells, and 68
linked/open S cells. Its 32 plan/B-linker tuples and all 29 sibling-write lists match the
frozen Java pass exactly, and the scheduler reaches the explicit `SidesExhausted` state
with the same 34-beam retained worklist that seeds Java's STUMPS pass. The expected pass
is opened only after the native terminal
exists. Removing the identity row first needed by transaction 31 fails before B14 mutation
and leaves the carrier and glyph bridge unchanged; removing a later sibling
cell rejects B16 after provisional B12-B15 work while leaving the complete carrier
unchanged. Transactions 2-32 use the native registry without per-frontier
selected-glyph rows or exhaustive scans. Only transaction 1 remains
fixture-hydrated; the sparse selected-base Java identity bridge, native carriage
plus wider coverage for the reconstructed Allegretto linked-S/hook-removal path,
general dirty state, and wider-corpus STUMPS authority and branch coverage remain outside this bounded
chula-system-1 claim.

The next scheduler boundary enters that retained STUMPS worklist without persistent
mutation. Beam SIG 12 begins at event 0. Its first stump has both `sideStump=true` and
`linkedBefore=true`, and the structural-side skip wins at event 1 exactly as Java orders
the tests. Its second stump is unlinked; plan 147 at `BEAM_SEED` profile 3 and link profile
1 produces two relations, one glyph, and no stored-line change, then Java event 2 stops at
`AwaitingVLinkTransaction` before `createStem`. Native represents that attempt as the typed
frontier after two scheduler event records.
The ten-line / 3,134-byte fixture contains five semantic rows plus its summary and has
SHA-256 `ef8f180110a409f85167ee1cc0f641c210144d6e5b5c737d5d8eb69e82d47bcb`;
its body, probe, and runner hashes are pinned in the summary. The real prefix contains no
pure already-linked event and no known-false plan before its frontier, so those paths are
not claimed as real-corpus coverage. Graph, shared B/S cells, and registries remain
unchanged.

The twenty-second boundary executes that first stump transaction through B12-B17 and resumes
without the SIDES caller's outer B18 assignment. Java reuses active glyph 310, creates checked
Stem Inter 2372 after two `AllUnlinked` reads, adds no sibling links and two head links, and
records `outerAssignment=false`. Native adds dense stem identity 32 and relation identity 331,
leaving the graph at 254 vertices / 334 edges with 33 Stem bindings, 62 linked/open B cells,
and 70 linked/open S cells. Resume skips beam SIG 12 stump 2 and beam SIG 22 stump 0 as
structural side stumps even though both are linked, then stops at worklist index 1 on
`beam:22:b:1` / plan 622 before its `createStem`. No pure already-linked or known-false event
occurs in this real prefix. The separate six-row-plus-summary fixture is 11 lines / 2,619 bytes
with SHA-256 `b1a312ddc690911b916971081ce21ea1c2211283df174a2175094ace7c144d5e`.
Probe, runner, emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`f9ca026d03873ab5c40c568a926455e0555d343540d522258d87757a1cc28f0c`,
`db9a2fd99746dfbc2ae3b5eed643a374e79dabc26a79101b05779cfba25ee5a4`, and
`5997662c47fb5be7cc61079baecb10f2986c89b05a7c0c97b937596dbc5009d6`.

Boundary 23 applies that unchanged generic carrier a second time from the mutated first-STUMPS
terminal; it is second-frontier generalization evidence, not a new production operation. Java's
beam SIG 22 / `beam:22:b:1` / plan 622 transaction uses glyph 321 `ReuseActive`, returns
`CreatedChecked` Stem Inter 2373 after two `AllUnlinked` reads, writes no sibling and two head
links, and records `outerAssignment=false`. Native adds dense stem identity 33 and relation
identity 334, reaching 255 vertices / 337 edges, 34 Stem bindings, 63 linked/open B cells, and
72 linked/open S cells. Resume skips structural-and-linked `beam:22:b:2` and `beam:16:b:0`,
then stops at worklist index 2 on `beam:16:b:1` / plan 404 before `createStem`; profile 3 / link
profile 1 yields two heads, last index 3, two relations, two glyphs, and no line change. The
separate six-row-plus-summary fixture is 11 lines / 2,712 bytes with SHA-256
`4e54cc848116597ad563fd9038e102a135ff606660775e09142c8c8564567173`.
Probe, runner, emitted-body, semantic-pass, and init-script SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`1812529f72a86e4b96b7d08d09f98a1157d9feb862296cd19e95de5caddded11`,
`716db362ee56e43a0375d8cf0efb0c88cd0af67de5707926bc4b713505201187`,
`07b6dc29043c6b63bd1f9f9e15822270ca3169e8662207c7cbbf67a06d8579a6`, and
`08d332af997d502fd32afb8b6257243d5ef41e87fa0001f90f3680c17394acd2`.
The refreshed linked-S fixture SHA-256 is
`287175a58717874882bc6487f7d59ea86a22e44cadcac003ee99a36606e5ab34`.

Boundary 24 applies the same production carrier a third time and grades the first natural
multi-glyph STUMPS candidate in this carried prefix. Plan 404 on beam SIG 16 /
`beam:16:b:1` / TOP combines Java glyph IDs 303 and 2156; their union equals active modeled
glyph 303 at ordinal 972, so `ReuseActive` changes neither registry nor allocator. Java returns
`CreatedChecked` Stem Inter 2374 after two `AllUnlinked` reads, adds no sibling links and two
head links, uses base edge 337, links B, and records `outerAssignment=false`. Native adds dense
stem identity 34 and reaches 256 vertices / 340 edges, 35 Stem bindings, 64 linked/open B cells,
and 74 linked/open S cells. Resume skips structural-and-linked `beam:16:b:2` and
`beam:28:b:0`, then stops at worklist index 3 on `beam:28:b:1` / plan 508 before `createStem`;
profile 3 / link profile 1 yields two heads, last index 3, two relations, two glyphs, and no line
change. The six-row-plus-summary fixture is 11 lines / 2,709 bytes with SHA-256
`e7409462ec43f5cde89ffdeafb0c5bb59586c37fff1506086d9c5fa770b30490`.
Probe, runner, emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`f2a41ca0069873274e443c978d0e84d56c49d67fa3387ef06346995dd2d587c1`,
`3e66a99fe44495915fbb8c15f7285a7c9a5ae4340df60b7766968c3e214a1bc7`, and
`ee1acaf3b1742346913ce3e9ed32430d3a4b24277537f0ed8e941d530ee6935b`.
The refreshed linked-S fixture SHA-256 is
`287175a58717874882bc6487f7d59ea86a22e44cadcac003ee99a36606e5ab34`.

Boundary 25 adds `drive_native_stems_beam_stumps_from_first_stems_bridge`, a bounded
atomic driver over that one-frontier operation. It runs on a shadow carrier and commits
the whole batch only after consuming a positive caller limit or reaching typed
post-STUMPS completion; a later error rolls every earlier transaction in the call back.
From Boundary 24's plan-508 frontier, chula system 1 executes the remaining plans 508,
28, 330, and 251. Java reports glyphs 308/305/302/300, `ReuseActive`, `CreatedChecked`
Stem Inter IDs 2375-2378, `AllUnlinked` reads 2/2/3/2, base edges 340/343/346/350,
zero siblings, and head counts 2/2/3/2. Native uses dense stem identities 35-38 and
finishes all seven STUMPS transactions after 92 scheduler events at 260 vertices / 353
edges, 39 Stem bindings, 68 linked/open B cells, and 83 linked/open S cells. A limit of
one commits only plan 508 and returns plan 28; zero rejects unchanged; removing the later
`beam:32:b:1` cell makes the whole multi-transaction call fail atomically. The fresh
fixture is 87 lines / 19,184 bytes—82 semantic rows plus summary—with SHA-256
`81fecf842495ddc93792b0ed5acf5641231181f172acd4e5cbf3bc57565f0cd2`.
Probe, runner, emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`2c6f9aaf39ae8ec2420104f15a3f6a2784f4eb4f229b0b23a7963ab5aade5717`,
`946c160f4759ee3edb093c3cc1e5394965409f64e1b516b1ebcbbbfe009f49e4`, and
`a629a2d63d223f28264c3fdc4dc20941e082402c27d75c2c6d884e2ce8282d08`.
The Boundary-25 production library run is 695 passed / 0 failed / 2 ignored, and the
full local workspace, formatting, and strict all-target Clippy gates are green;
`5f75f8708` (including Boundary 43) is the current CI baseline: Rust run 32217412749 passed all 12
shards and Build & Test run 32217412751 passed, with no failure or cancellation.
This completes chula system 1's STUMPS worklist, not full STEMS. Wider-corpus authority
and branch coverage, other systems, and later STEMS phases remain open.

Boundary 26 adds `remove_native_stems_beam_competing_hook_and_resume`, an atomic
graph-owning consumer for one typed SIDES hook-removal frontier. Its gate deliberately
reconstructs Allegretto system 1's state after transaction 28 from 28 measured B/sibling
writes; it does **not** claim native execution of predecessor transactions 1-27. At Java
event 64 / work index 19, Beam SIG 25 has both logical sides linked and names same-glyph
BeamHook SIG 24 as its competitor. Java removes Inter 907 from the active SIG while its
SIG attachment and persistent InterIndex state remain represented. The group changes
from `[21,24,25]` to `[21,25]`; the local worklist and 43-entry linked-B set are unchanged.
Native tombstones vertex 56, removes its active source binding and all five incident
relations (Containment, BeamBeam, Exclusion, and two BeamStem), then resumes the remaining
SIDES work to `SidesExhausted`. Active graph counts move 202/232 to 201/227. Java exhausts
at visible event 110; native emits 54 continuation events and ends at 143 internal events.
A missing Exclusion rejects without changing the supplied carrier. The predecessor
fixture is 32 lines / 4,195 bytes, SHA-256
`d173f1c475245980cad02bbf4624987d787fb293e5419d21444729f18bf7c8f8`; the result
fixture is 9 lines / 4,336 bytes, SHA-256
`d4c5decf03eaab893c79b2cb7ebd0378f13ac019acc007a38718105c75eacc71`.
Probe, runner, emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`3b9e0e28c9c2de75266c676a880dfe636bef885591ce12ed832640b8c72dd845`,
`52432167156b75e4754259ae6c2a634e87788f028e85e6ea14754859e12ccb1f`, and
`2cc4ad8e0aadf29b8055ce34c32b703c033c45880bef24ff26a707b6b6f0d3c5`.
The Boundary-26 production library run is 696 passed / 0 failed / 2 ignored, and the
full local workspace, formatting, and strict all-target Clippy gates are green;
`5f75f8708` remains the current remote CI baseline. Native
Allegretto predecessor carriage, hook removal beyond this checkpoint, wider-corpus
STUMPS authority, general dirty-state ownership, other systems, and later STEMS remain.

Boundary 27 adds `begin_native_stems_head_linking_phase1`, the typed read-only transfer
from chula system 1's exact native post-STUMPS carrier into Java's first heads-linking
phase. It accepts only scheduler `Completed`, validates common system identity and live
SIG bindings, recomputes Java's stable reverse-grade order, and requires all 102 live
graded heads plus the exhaustive duplicate-free persistent S-cell catalogue and exact
TOP-then-BOTTOM observer order. The returned carrier owns an unchanged clone of the
260-vertex / 353-edge beam state, starts at head index 0 with empty unlinked-head and
undefined-side collections, and exposes STRICT stem profile 0 / link profile 1 /
`append=false`. The first head is SIG ordinal 45 / Java Inter 1375 at grade bits
`0x3fe917c3b8207578`. LEFT is open and unlinked with TOP/BOTTOM `false/false`; RIGHT is
open and unlinked with `true/false`, so native selects `h:38:RIGHT:TOP` and returns
`AwaitingHeadCLinkTransaction` before calling `HeadLinker.CLinker.link`.

Terminal, system, binding, order, head, S-cell, observer, builder-length, and gap-map
incoherence fail closed. The deliberately bounded `canLink` prefix also rejects the
unported dual-corner and close-head/gap-recursion branches, and it does not claim
rather-good retry, no-link closure, phase-2 append, or any SIG/GlyphIndex/system-stem/
shared-cell mutation. Boundary 28 below now consumes that selected frontier. The shared
fixture is now expanded through Boundary 32 to 16 lines / 12,880 bytes—eleven semantic rows plus
summary—with SHA-256
`91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`.
Probe, runner, emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`,
`dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and
`e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398`.
Boundary 27's full local workspace, formatting, and strict all-target Clippy gates are
green. `5f75f8708` (including Boundary 43) is the current remote CI baseline: all 12 Rust
shards and the Java build passed without failure or cancellation.

Boundary 28 adds `advance_native_stems_head_single_item_c_link`, an atomic consumer for
Boundary 27's selected `h:38:RIGHT:TOP` frontier. The builder is deliberately bounded to
one nonrecursive `StartHeadHalfLinker` with `lastIndex=maxIndex=0`. Its retained vertical
seed resolves to canonical glyph 307, already active and strongly retained, so Java and
native both report `ReuseActive` without changing registry counts or hashes. With
`append=false`, production accepts only `CreatedChecked`: it creates native dense Stem
identity 39 / Java Inter ID 2379 with checked-grade bits `0x3fe935543bd31399`, normal
attached bounds `1140:319:4:92`, then inserts one HeadStem relation with grade bits
`0x3fee3eb4ae84ca9d`, dx bits `0xbfa5d942375d430c`, zero dy, RIGHT side, and extension-x
bits `0x4091d5d6e6668034`.

The compact native graph moves 260/353 to 261/354, Stem bindings 39 to 40, and the
persistent ID allocator 2378 to 2379. The selected RIGHT S cell changes false to true and
the queued per-head cache changes with it; linked S cells move 83 to 84 while the cell
remains open and `closed_cell_changes=0`. Java's full graph independently moves 678/689
to 679/690, so the exact normalized mutation is claimed without equating absolute Java
and compact-native graph sizes. The carrier commits `current_index=1` and
`frontier_consumed=true`, then stops before processing head index 1. Late or corrupt
glyph authority rejects atomically with the carrier unchanged.

This boundary does not generalize beyond the measured single-item, nonrecursive,
`CreatedChecked` path. Multi-item expansion, gaps and beam relations, `reuseStem`, other
creation dispositions, duplicate relations, outer remaining-head iteration, rather-good
retry/no-link closure, unlinked-head collection, phase-2 append, and recursive tail
C-linking remain open. The head-prefix fixture is now 16 lines / 12,880 bytes—ten
semantic rows plus summary—with SHA-256
`91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`.
Probe, runner, emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`,
`dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and
`e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398`.
Boundary 28's full local workspace, formatting, and strict all-target Clippy gates are
green. `5f75f8708` (including Boundary 43) is the current remote-CI baseline: its exact commit
has reached terminal green.

The bridge constructor fail-closes on the exact 1,650/1,058/592/2,339 census, duplicate
IDs or fingerprints, non-dense/future modeled ordinals, malformed hashes, and any mismatch
between a modeled RunTable and its frozen fingerprint. Only exact modeled content can
answer equality; the 592 opaque page-global entries are membership/audit evidence, never
absence authority. The raw registry text is consumed and dropped before transaction 3,
and both early bridge corruption and a late B16 fault preserve carrier state atomically.

**Slice 3 (BEAMS, ordinals 43-110), measured:** 48 hooks/beams interleaved in detection
order (`HBHBHB...` -- the hook usually precedes its beam), then all 20 BeamGroupInters.
Edges, all internal: 48 Containment (each group contains its members: 31 beams, 17 hooks),
44 BeamBeamRelation (13 beam-beam, 12 hook-beam, 12 beam-hook, 7 hook-hook), 10 Exclusion
(hook vs beam). `recognize_native_beams` already produces beams, hooks and groups.

**Slices 4-5 (implemented):** LEDGERS and HEADS now append in exact order, publish native
head bindings, and feed the owned STEMS graph. B14-B17 graph evidence is computed from
that graph for the measured carrier. What remains is registry authority and later
scheduler paths, not another SIG assembly pass.

Boundary 29 continues phase 1 from Boundary 28's committed head frontier through two
prelinked-success heads. `continue_native_stems_head_linking_phase1` revalidates the
completed chula carrier, stable reverse-grade order, live head bindings, and exhaustive
persistent S-cell topology on each call. Head order 1 is x90 / SIG ordinal 23 / Java
Inter 1331. LEFT is already linked and both open RIGHT STRICT corners are false, so the
call returns true and closes both sides of the other head sharing Stem 2359: x89 LEFT,
then x89 RIGHT, both false-to-true. That is two ordered writes and two value changes.
Head order 2 is x81 / SIG ordinal 33 / Java Inter 1351. It follows the same prelinked
success path and closes both sides of the two other heads sharing Stem 2371: x79 LEFT,
x79 RIGHT, x80 LEFT, x80 RIGHT, all false-to-true. That is four ordered writes and four
value changes. Neither continuation records an unlinked head.

Both calls leave SIG, glyph, stem, allocator, relations, and linked flags unchanged;
only the six named closed-cell values and the ordered queue position change. Native
reaches `current_index=3`, `frontier_consumed=true`, before x20 / SIG ordinal 65 / Java
Inter 1419. Missing shared-stem/HeadStem/binding/S-cell closure topology or invalid
consumed-frontier state rejects atomically. The current expanded schema-v6 fixture is 16 lines /
12,880 bytes—eleven semantic rows plus summary—with SHA-256
`91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`.
Probe, runner, emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`,
`dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and
`e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398`.
Later queue entries, a later C-link mutation, an actually unlinked head and rather-good
retry/no-link closure, phase-2 append, and broader head branches remain open.

Boundary 30 extends the same unchanged continuation through head order 3. Starting at
`current_index=3`, x20 / SIG ordinal 65 / Java Inter 1419 has a prelinked LEFT side and
both open RIGHT STRICT corners false. Java returns true and scans shared Stem 2361 in
SIG order, closing x19 LEFT then RIGHT (two false-to-true writes), with no unlinked-head
entry. Native reaches `current_index=4`, `frontier_consumed=true`, before x36 / SIG
ordinal 69 / Java Inter 1427; graph, registry, stem, allocator, relation, and linked
state remain unchanged apart from those two S-cell closures. Missing closure topology
still rejects atomically. The current expanded schema-v6 fixture is 16 lines / 12,880 bytes with ten
semantic rows plus summary, SHA-256
`91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`; probe, runner,
emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`,
`dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and
`e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398`.

Boundary 31 extends that unchanged continuation through head order 4. Starting at
`current_index=4`, x36 / SIG ordinal 69 / Java Inter 1427 / grade bits
`0x3fe8e37718100f0c` has a prelinked LEFT side and both open RIGHT STRICT corners false.
Java returns true and scans shared Stem 2369 in SIG order, closing x35 LEFT then RIGHT;
both writes change false to true, `closedValueChanges=2`, and `unlinkedCount=0`. Native
reaches `current_index=5`, `frontier_consumed=true`, before x99 / SIG ordinal 61 / Java
Inter 1411 / grade bits `0x3fe8b9e1faa76070`. Graph, registry, stem, allocator,
relation, and linked state remain unchanged apart from those two closed S cells. Missing
closure topology still rejects atomically. The schema-v6 fixture is 16 lines / 12,880
bytes with eleven semantic rows plus summary, SHA-256
`91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`; probe, runner,
emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`,
`dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and
`e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398`.
This remains one bounded prelinked-success case: the remaining queue, a later C-link
mutation, actually-unlinked/retry behavior, phase-2 append, and broader branches remain
open.

Boundary 32 extends the unchanged continuation through head order 5. Starting at
`current_index=5`, x99 / SIG ordinal 61 / Java Inter 1411 returns true through the
prelinked-success path, and shared Stem 2365 closes x98 LEFT then RIGHT in SIG order.
Both writes change false to true and no unlinked head is recorded. Native reaches
`current_index=6`, `frontier_consumed=true`, before x22 / SIG ordinal 12 / Java Inter
1309. Graph, registry, stem, allocator, relation, and linked state remain unchanged
apart from those two closed S cells. Missing closure topology still rejects atomically.
The schema-v6 fixture is 16 lines / 12,880 bytes with eleven semantic rows plus summary,
SHA-256 `91541fc08786b8d81b6f6c26d68d83214276a3e68bcdd488f5607a135438aff8`;
probe, runner, emitted-body, and semantic-pass SHA-256 are
`d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf`,
`8bdd41abb42b23187f2b7380a39a77d2218d996e6b8edcf6c3697a91dfe1e3b3`,
`dedc03783647ab198966cc87d1bfc491e990ad17c66564b3c0fe00a5231ba310`, and
`e98f8181cce2d0bae08fda7617d63c313180ad2d8464902d870c189cafe4a398`.
This remains one bounded prelinked-success case: the remaining queue, a later C-link
mutation, actually-unlinked/retry behavior, phase-2 append, and broader branches remain
open.

## Boundary 33: continuation-specific head C-link

`advance_native_stems_head_continuation_c_link` is the next bounded production boundary. From the carried chula-system-1 phase-1 frontier at head order 7 (x76 / SIG 97 / Java Inter 1483), it authenticates the LEFT/BOTTOM `BottomOnly` choice, reuses active glyph 319, and atomically applies the one-item, nonrecursive head-origin C-link. Java creates checked Stem Inter 2380 with one HeadStem relation; native advances the dense graph from 679/690 to 680/691, Stem bindings from 40 to 41, links the LEFT S cell, and advances `current_index=7` to 8 before x95 / SIG 100 / Java Inter 1489. The registry is unchanged and rollback rejects a mismatched continuation frontier. The v7 derivative fixture is 20 lines / 18,778 bytes, SHA-256 `8df7d36e780e90e569fcc37144bd48ff43e5b647f9cdc240d899ee10386b153d`; runner `87a12b97b6d9c79e6c0d346f8187b426505ab5e0e7785bd07a5984a03a18c197`, transformed probe `93c6771d55b814cff4155d4065d94a322767df9a668033bc7f2e5ea1ea7f6edd`, emitted body `06285da43ff0b5a1f3644c4468570a10f24c0c8f2b8173e9e7d1e268284704d6`, and semantic pass `68d581d84f21a79c41df3d4ebf6a856cc0dee266288512e4cd1e44bb3260fa0c`. Remaining head queue, actually-unlinked/retry, phase-2 append, multi-item/recursive C-linking, broader systems/corpus, and full STEMS remain open.

## Boundary 34: eighth prelinked head continuation

Boundary 34 reuses `continue_native_stems_head_linking_phase1` from Boundary 33's
`current_index=8` state. Head order 8 (x95 / SIG 100 / Java Inter 1489) is prelinked on
LEFT; both RIGHT STRICT corners are false, and shared Stem 2364 closes x91 LEFT/RIGHT
then x94 LEFT/RIGHT in Java order. Native records four value changes, no unlinked head,
and reaches `current_index=9` before x42 / SIG 93 / Java Inter 1475 without graph,
registry, stem, allocator, or relation mutation beyond those S-cell closures. The v8
fixture is SHA-256 `82eca291e69ec27e49903d31b1da408f68962469780a1f706f3f979564e8aebb`;
runner, transformed probe, emitted body, and semantic pass are
`4d3be4619b7fbe5f5ca39e4065914fe7bb2a56dcbbfb6ae67c95cf444140edfc`,
`fe2bd835c8359810099881288608bc0055336f1ebb77e6715aa2946570181867`,
`a5460ce6a40756092d2e2dc91975ac5c2665c480370249084faa141d7b45eca8`, and
`062721eabd59d1d4b4bc5d4c18b3d6ee8e510c68d76473278e6cb60c5e2f7597`.
Actual-unlinked/retry, phase-2 append, later C-link shapes, and broader corpus/system
coverage remain open.

## Boundary 35: ninth prelinked head continuation

Boundary 35 reuses the unchanged head continuation at order 9 (x42 / SIG 93 / Java
Inter 1475). LEFT is prelinked and RIGHT is `Neither`; shared Stem 2352 closes x41
LEFT then RIGHT, with two ordered value changes and no unlinked insertion. SIG remains
680/691 and Stem bindings remain 41; native advances to `current_index=10` before x65 /
SIG 95 / Java Inter 1479. The v9 fixture is SHA-256
`b0d3c67f9b76a56a528d8a962f3f2bc54710616f2e86650ac8500e750534ff2c`; runner,
transformed probe, emitted body, and semantic pass are
`368724efe73e194aff024d68204d758d089d81511e9bbfaa4dfb9ef9516f4c48`,
`caf6a7f25cc36cbe7480c7cb798a8c900bbd526fa7e4071d625724045bb88af5`,
`dbe0a891add7c613c340dfaee983a75c97b20cba4744ca19619e10cd9f7a78f5`, and
`8e271204197c0d84afe4948a94f6723f6a419cd9611495aa8ca74fb7731bbf95`.
Actually-unlinked/retry, phase-2 append, later C-link shapes, and broader corpus/system
coverage remain open.

## Boundary 36: tenth prelinked head continuation

Boundary 36 reuses the unchanged continuation at head order 10 (x65 / SIG 95 / Java
Inter 1479). LEFT is prelinked and RIGHT is `Neither`; shared Stem 2346 closes x64
LEFT then RIGHT, with two ordered value changes and no unlinked insertion. SIG remains
680/691 and Stem bindings remain 41; native reaches `current_index=11` before x46 /
SIG 57 / Java Inter 1403. The v10 fixture is SHA-256
`7b0bf32fcf75cf792eb67c2c8a52ae9702de215078a54bea7edc7cde853869d0`; runner,
transformed probe, emitted body, and semantic pass are
`ddf5b4c3f6d726c3e7d91de33d077930ff29254f1e7e84751ee391614978c464`,
`e7cf9dd3ceed19c3e387eabffb587005acb01725434776fe39501605ce4cd4af`,
`cbab4a06edd591e068007152dbb623d206a29c450aa2f9a153c75010fa184658`, and
`d5cd5dbed69852e48add157efd936ba8501879c30a023e730e0825c38825b712`.
Actual-unlinked/retry, phase-2 append, later C-link shapes, and broader corpus/system
coverage remain open.

## Boundary 37: eleventh prelinked head continuation

Boundary 37 reuses the unchanged continuation at head order 11 (x46 / SIG 57 / Java
Inter 1403). LEFT is prelinked and RIGHT is `Neither`; shared Stem 2377 closes x44
LEFT/RIGHT then x45 LEFT/RIGHT, four ordered value changes and no unlinked insertion.
SIG remains 680/691 and Stem bindings remain 41; native reaches `current_index=12`
before x55 / SIG 79 / Java Inter 1447. The v11 fixture is SHA-256
`cad1527e556481a073ead938094de9edce09954e366bf5608ebc57a30ef946a3`; runner,
transformed probe, emitted body, and semantic pass are
`f05ea06f61193785a84440b457b4e79b10e7d88e765b81bce51d6f996beb1702`,
`24f67a53e407909d07e1fc12bb2e180b15e6dfcf74983d52d1326cff906284ca`,
`a0716a3379db5d268419624a193d6a6d1dc0105f78ff56fecd44fa70272165e4`, and
`eefa750fd63fa91fec84c2fd9afc62b82d51081da606a0687496a111f5059602`.
Actually-unlinked/retry, phase-2 append, later C-link shapes, and broader corpus/system
coverage remain open.

## Boundary 38: twelfth prelinked head continuation

Boundary 38 reuses the unchanged continuation at head order 12 (x55 / SIG 79 / Java
Inter 1447, grade bits `0x3fe847463fc14b09`). LEFT is prelinked and RIGHT is
`Neither`; Java returns true and shared Stem 2362 closes x51 LEFT/RIGHT then x54
LEFT/RIGHT, four ordered value changes and no unlinked insertion. SIG remains 680/691
and Stem bindings remain 41; native advances from `current_index=12` to 13 before x53 /
SIG 3 / Java Inter 1291 (grade bits `0x3fe83971fb8b04c3`), whose next-side state is
`LEFT:true:false,RIGHT:false:false`.

The separate 12-line / 8,273-byte schema-v12 derivative contains seven semantic rows
plus summary and is intentionally snapshot-minimized: orders 1-11 execute `linkSides`
only to reconstruct the predecessor, without emitting or persisting their full
intermediate snapshots; only order 12 is emitted. This keeps deterministic replay
within the full-snapshot heap limit, while still pinning the base v11 fixture and runner
and requiring two fresh JVM runs to be byte-identical. Fixture, runner, transformed
probe, emitted-body, and semantic-pass SHA-256 are
`e8b19156d29722a74b41e6d07d1591edd78b3077844f6be7268fa78754a1acd2`,
`74b6ba4f84c046ae2ca08e270ce9726acee42a14f4b639282bfbccd3c8b654d1`,
`7b8f232f56d92f83966311478de6b0255820d6d00c9aa4dbb3f0f9351c43abc6`,
`ab41455ece56d8cce145f1105a417315be379f3c6d644efca539d008db1c099a`, and
`ad4dd95c5b9c12f101a8c2420cca76902e7cc7571b3277bfbd879a6ba4bcda67`.
This proves the bounded order-12 continuation, not independently snapshot-oracled
predecessor states or the remaining phase-1 queue. Actually-unlinked/retry, phase-2
append, later C-link shapes, and broader corpus/system coverage remain open.

## Boundary 39: thirteenth prelinked head continuation

Boundary 39 reuses the unchanged continuation at head order 13 (x53 / SIG 3 / Java
Inter 1291, grade bits `0x3fe83971fb8b04c3`). LEFT is prelinked and RIGHT is
`Neither`; Java returns true and shared Stem 2344 closes x52 LEFT then RIGHT, two
ordered value changes and no unlinked insertion. SIG remains 680/691 and Stem bindings
remain 41; native advances from `current_index=13` to 14 before x12 / SIG 63 / Java
Inter 1415 (grade bits `0x3fe8187dd5fbfd0c`), with next-side state
`LEFT:true:false,RIGHT:false:false`.

The separate 12-line / 8,188-byte schema-v13 derivative contains seven semantic rows
plus summary and is intentionally snapshot-minimized: orders 1-12 execute `linkSides`
only to reconstruct the predecessor, without emitting or persisting their full
intermediate snapshots; only order 13 is emitted. This keeps deterministic replay
within the full-snapshot heap limit, while still pinning the base v12 fixture and runner
and requiring two fresh JVM runs to be byte-identical. Fixture, runner, transformed
probe, emitted-body, and semantic-pass SHA-256 are
`ff27fa03e80e44e554d46682c827097ecec1d463abf0c0e131a6ab1beccfbb5e`,
`675bce84bfa4e76ed78cc72592da9f8fe95571752d424da99bd4be93af7478f8`,
`915bc4a3563943b93fa806a614b835da8e7799732cf8c1c1c7aa9127fc39a61e`,
`84254e3f9dc1e4297b4efaabb30c36d07244ffe3d268cce5097ec14d365ab974`, and
`f2b4a2e49aee6fd27d41470eb38a1bfe541d72688b03bb33d5b3ed3266514519`.
This proves the bounded order-13 continuation, not independently snapshot-oracled
predecessor states or the remaining phase-1 queue. Actually-unlinked/retry, phase-2
append, later C-link shapes, and broader corpus/system coverage remain open.

## Boundary 40: fourteenth prelinked head continuation

Boundary 40 reuses the unchanged continuation at head order 14 (x12 / SIG 63 / Java
Inter 1415, grade bits `0x3fe8187dd5fbfd0c`). LEFT is prelinked and RIGHT is
`Neither`; Java returns true and shared Stem 2349 closes x11 LEFT then RIGHT, two
ordered value changes and no unlinked insertion. SIG remains 680/691 and Stem bindings
remain 41; native advances from `current_index=14` to 15 before x67 / SIG 59 / Java
Inter 1407 (grade bits `0x3fe814269b1247c7`), with next-side state
`LEFT:true:false,RIGHT:false:false`.

The separate 12-line / 8,192-byte schema-v14 derivative contains seven semantic rows
plus summary and is intentionally snapshot-minimized: orders 1-13 execute `linkSides`
only to reconstruct the predecessor, without emitting or persisting their full
intermediate snapshots; only order 14 is emitted. This keeps deterministic replay
within the full-snapshot heap limit, while still pinning the base v13 fixture and runner
and requiring two fresh JVM runs to be byte-identical. Fixture, runner, transformed
probe, emitted-body, and semantic-pass SHA-256 are
`f60e5dff377e5e51038ec061b1ebeec5a5868f4cd51af6b9618377bfa3a12e6a`,
`6b5e339f8b91db08d4e03edf7ed3b69ea8ab713b98ce95c62a95440a0652ccb9`,
`eea0869093b1c1a262da5da0d7ad914f3dc7b6a8d771a32bc60849687291c834`,
`9ebf233711be059ddee5adf964b6bbbbe44770caef19f5903c8ce9a5a16d1889`, and
`14d0e0c71dff0f40e5745858ad10d615c56463291cf6caa863edd2ebccde0590`.
This proves the bounded order-14 continuation, not independently snapshot-oracled
predecessor states or the remaining phase-1 queue. Actually-unlinked/retry, phase-2
append, later C-link shapes, and broader corpus/system coverage remain open.

## Boundary 41: fifteenth prelinked head continuation

Boundary 41 reuses the unchanged continuation at head order 15 (x67 / SIG 59 / Java
Inter 1407, grade bits `0x3fe814269b1247c7`). LEFT is prelinked and RIGHT is
`Neither`; Java returns true and shared Stem 2375 closes x66 LEFT then RIGHT, two
ordered value changes and no unlinked insertion. SIG remains 680/691 and Stem bindings
remain 41; native advances from `current_index=15` to 16 before x8 / SIG 53 / Java
Inter 1395 (grade bits `0x3fe81161126880f9`), with next-side state
`LEFT:true:false,RIGHT:false:false`.

The separate 12-line / 8,191-byte schema-v15 derivative contains seven semantic rows
plus summary and is intentionally snapshot-minimized: orders 1-14 execute `linkSides`
only to reconstruct the predecessor, without emitting or persisting their full
intermediate snapshots; only order 15 is emitted. This keeps deterministic replay
within the full-snapshot heap limit, while still pinning the base v14 fixture and runner
and requiring two fresh JVM runs to be byte-identical. Fixture, runner, transformed
probe, emitted-body, and semantic-pass SHA-256 are
`aae5116a32e0fd77bb9f4a26dc1a8c1cd53a3f3ff35ea01d350c97012a146ca8`,
`e595eefa74453ecfe9980cb294b80d37d0ff5ad1e2f3e01d88f8801d0f23ca18`,
`98ac227864e84c3693d5368a85adf970512648a9a99c74a2b612a01d4b45d065`,
`1e198195daf91b8d56ebcc2a88a5e97fc2752603f365d0d5cea3145f9a1f1ef2`, and
`55323828f0e4c8e08d85373684f71b7ec9a6f2e75a49278006dae1b8ec673cd9`.
This proves the bounded order-15 continuation, not independently snapshot-oracled
predecessor states or the remaining phase-1 queue. Actually-unlinked/retry, phase-2
append, later C-link shapes, and broader corpus/system coverage remain open.

## Boundary 42: sixteenth prelinked head continuation

Boundary 42 reuses the unchanged continuation at head order 16 (x8 / SIG 53 / Java
Inter 1395, grade bits `0x3fe81161126880f9`). LEFT is prelinked and RIGHT is
`Neither`; Java returns true and shared Stem 2376 closes x7 LEFT then RIGHT, two
ordered value changes and no unlinked insertion. SIG remains 680/691 and Stem bindings
remain 41; native advances from `current_index=16` to 17 before x48 / SIG 29 / Java
Inter 1343 (grade bits `0x3fe80cc40bda9d4c`), with next-side state
`LEFT:true:false,RIGHT:false:false`.

The separate 12-line / 8,189-byte schema-v16 derivative contains seven semantic rows
plus summary and is intentionally snapshot-minimized: orders 1-15 execute `linkSides`
only to reconstruct the predecessor, without emitting or persisting their full
intermediate snapshots; only order 16 is emitted. This keeps deterministic replay
within the full-snapshot heap limit, while still pinning the base v15 fixture and runner
and requiring two fresh JVM runs to be byte-identical. Fixture, runner, transformed
probe, emitted-body, and semantic-pass SHA-256 are
`04d35bb21c808dc38edd93c0631b3a01af9931efc8f500422646adf8f7123de4`,
`d6edd52b746acd625c2e516f328c4b43253e23bbbe906ffcdae0b3674eae1dcf`,
`d4dcad17952d2de86de193bd87c3a96916ad7781d67f1ea469180e05e4e106fd`,
`49b97d61e08769b58a449edf2931313f91a5855000fd89e27761330f30a81077`, and
`88ea097c4a003e7493c5d28296cc6dd778486660bb6a1e3eb1bfb5aa71f40f7d`.
This proves the bounded order-16 continuation, not independently snapshot-oracled
predecessor states or the remaining phase-1 queue. Actually-unlinked/retry, phase-2
append, later C-link shapes, and broader corpus/system coverage remain open.

## Boundary 43: seventeenth prelinked head continuation

Boundary 43 reuses the unchanged continuation at head order 17 (x48 / SIG 29 / Java
Inter 1343, grade bits `0x3fe80cc40bda9d4c`). LEFT is prelinked and RIGHT is
`Neither`; Java returns true and shared Stem 2351 closes x47 LEFT then RIGHT, two
ordered value changes and no unlinked insertion. SIG remains 680/691 and Stem bindings
remain 41; native advances from `current_index=17` to 18 before x63 / SIG 17 / Java
Inter 1319 (grade bits `0x3fe8009e50c15bf8`). Unlike the preceding continuation
frontiers, that next head begins with both sides open and unlinked:
`LEFT:false:false,RIGHT:false:false`.

The separate 12-line / 8,194-byte schema-v17 derivative contains seven semantic rows
plus summary and is intentionally snapshot-minimized: orders 1-16 execute `linkSides`
only to reconstruct the predecessor, without emitting or persisting their full
intermediate snapshots; only order 17 is emitted. This keeps deterministic replay
within the full-snapshot heap limit, while still pinning the base v16 fixture and runner
and requiring two fresh JVM runs to be byte-identical. Fixture, runner, transformed
probe, emitted-body, and semantic-pass SHA-256 are
`8e4909edc2196f2baff6f517693f9a9af50405cf85fc88bcf3e771711bae2b4b`,
`84c176b45ec8adb7af8e0ab1014acabfe8c57c2e6b3cbbe5e8bbd0e971823196`,
`b139149dd41b5581d96344617c2f52b49a85f085f011ff4b556b237f58765342`,
`2362b903486db2d4ddbc14aeeeb54761205bdd06a206875ef0c131a7a22e5fdd`, and
`c89f5a49456af435e2fb508e0ccbbd5a7b8fd9877616534cb7136ccd0ff84ecf`.
This proves the bounded order-17 continuation, not independently snapshot-oracled
predecessor states or the order-18 behavior. Actually-unlinked/retry, phase-2 append,
later C-link shapes, and broader corpus/system coverage remain open.

## Boundary 44: first open/unlinked continuation C-link

Boundary 44 consumes head order 18 (x63 / SIG 17 / Java Inter 1319, grade bits
`0x3fe8009e50c15bf8`) from the both-open/unlinked frontier. Java selects LEFT/BOTTOM
(`BottomOnly`), expands a two-item builder (`lastIndex=maxIndex=1`) from active glyphs
328 and 2063, reuses canonical glyph 328 without reinsertion, and creates checked Stem
Inter 2381 with one HeadStem relation. Native creates dense Stem identity 41, moves SIG
680/691 to 681/692 and Stem bindings 41 to 42, links the LEFT S cell, records no
unlinked head or closure write, and advances to `current_index=19` before x69 / SIG 76 /
Java Inter 1441 (grade bits `0x3fe7fe09c1461c49`).

The geometry is deliberately bounded to this two-item continuation: the native path
matches Java's RunTable centroid accumulation order and directly interpolates the
theoretical stem line's x coordinate at the centroid y before translating the line.
That does not establish generic multi-item/recursive geometry, other corner shapes,
`reuseStem`, or retry/no-link behavior. The focused Boundary-44 gate and full 14-test
sibling suite are green; formatting, strict all-target Clippy, and diff checks pass.

The 14-line / 11,751-byte schema-v18 derivative contains nine semantic rows plus summary
and remains snapshot-minimized: orders 1-17 reconstruct the predecessor without emitted
or persisted full snapshots; order 18 alone emits its C-link envelope/result and
continuation. Fixture, runner, transformed probe, emitted-body, and semantic-pass
SHA-256 are `4972836c5e2718f9441a007840cfc5100caa95a12dc349d7822c0695ad0f5b2b`,
`3bea814e71ba13374130351d0f5cc057779e5676e402e7b43b5c4ee4a263e332`,
`4e15aa27d982b6ea848b5a7349819e3db7300349dded652f859492abe2ea7460`,
`499b791dc34d2ca59666bbab20e4ca15a9dd335260d4714dbdd9042ed00456cd`, and
`7045d9060ea8e6d930b94d28e79e3e6d8d0cc0bb0b57bb20c64a3780b876bcb3`;
the Java fragment source is pinned by
`f56fdd58606c3d5101ebea1690162b38f9db6a18f89a4fe0e441cedff1bac36c`.
Remaining phase-1 iteration, actually-unlinked/retry, phase-2 append, broader C-link
geometry, and broader corpus/system coverage remain open.

## Boundary 45: post-C-link prelinked continuation

Boundary 45 carries head order 19 (x69 / SIG 76 / Java Inter 1441, grade bits
`0x3fe7fe09c1461c49`). Its LEFT side is already linked and RIGHT is open/unlinked, so
Java reports `SkipAlreadyLinked` then `Neither`. Shared Stem 2347, incident with x68 /
SIG 75 / Inter 1439 and x69 / SIG 76 / Inter 1441, closes x68 LEFT then RIGHT through
two ordered false-to-true writes. Native records no unlinked head, keeps SIG 681/692,
system stems 42, and the relation hash unchanged, and advances to `current_index=20`
before x74 / SIG 19 / Java Inter 1323 (grade bits `0x3fe7f8f93b5cf200`), whose LEFT
and RIGHT sides are both open/unlinked. Boundary 45 does not execute that next head.

The focused Boundary-45 gate and full 14-test sibling suite are green; formatting,
strict all-target Clippy, and diff checks pass. The 15-line / 13,004-byte schema-v19
derivative contains ten semantic rows plus summary and remains snapshot-minimized:
orders 1-18 reconstruct the predecessor without emitted or persisted full snapshots,
while order 19 alone emits the new continuation. Fixture, runner, transformed probe,
emitted-body, and semantic-pass SHA-256 are
`6d415102995fd1fda8057fab27b0f2a3a6cb2367cbcce52269009f377bf672ae`,
`b79cb0c5cba1d3b1275dd943d7945722a5f025281686362d6b40a311d3ad5335`,
`e94082b8faa8a8c26e70b00acd42bc091e7c9333317caa5299f6d18083cba781`,
`3ae97b86466a49fafbe07f5c32d5641824099e677131fff14aee3797f61cc3a9`, and
`9628fefbc7e1c88ab184aa711e329b9606e4d57252965428b9f3f33e96852a31`;
the Java fragment source remains pinned by
`f56fdd58606c3d5101ebea1690162b38f9db6a18f89a4fe0e441cedff1bac36c`.
This is bounded order-19 evidence, not independent predecessor snapshots or evidence
for order 20, actually-unlinked/retry, phase-2 append, generic multi-item/recursive
C-linkers, or broader corpus/system coverage.

## Boundary 46: second open/unlinked continuation C-link

Boundary 46 consumes head order 20 (x74 / SIG 19 / Java Inter 1323, grade bits
`0x3fe7f8f93b5cf200`) from a both-open/unlinked frontier. Java selects LEFT/BOTTOM,
expands a two-item builder (`lastIndex=maxIndex=1`) from active glyphs 332 and 2301,
reuses canonical glyph 332, and creates checked Stem Inter 2382 with one HeadStem
relation. Native creates dense Stem identity 42, moves SIG 681/692 to 682/693 and
system stems 42 to 43, links LEFT, records no closure write or unlinked head, and
advances to `current_index=21` before x28 / SIG 55 / Java Inter 1399 (grade bits
`0x3fe7e38e38e38e39`), whose LEFT side is linked and RIGHT remains open/unlinked.

The geometry remains deliberately case-bounded. It uses the authenticated two-item
centroid/interpolation path from Boundary 44, then applies Java `nextDown` to both
translated x coordinates only at the x74 frontier; generic multi-item/recursive
geometry and other corner shapes are not established. The focused Boundary-46 gate
and full 14-test sibling suite are green; formatting, strict all-target Clippy, and
diff checks pass.

The 16-line / 14,117-byte schema-v20 derivative contains eleven semantic rows plus
summary and remains snapshot-minimized: orders 1-19 reconstruct the predecessor
without emitted or persisted full snapshots, while order 20 emits its C-link
envelope/result and continuation. Fixture, runner, transformed probe, emitted-body,
and semantic-pass SHA-256 are
`be6a820b3740105e4fdddeb0e9ec475d1dd3ebc8611fd7be555cf55957dfe4a4`,
`54468f53de6c0d1d931e391640642f55ce6c4733721df569ef6f10ef93704497`,
`40ced3035bdb19298e925b499edce42365aca66586abe7f8756847f32a1abd82`,
`3b1f4c53462e4ff8241863e73c90043d813cf5709cd0f3c809858659d7261564`, and
`dbd1c398b3ab3565a75ab9ed6dfa276b3493c52a6dd22a9a54ad09dc5e89e4d5`;
the v20 fragment source is pinned by
`5fa3ac22fe21091c313135909f13c793be575fd460f0af3349345ba8ede9ab3e`.
This is bounded order-20 evidence, not independent predecessor snapshots or coverage
of order 21, actually-unlinked/retry, phase-2 append, generic multi-item/recursive
C-linkers, or broader corpus/system behavior.

## Boundary 47: existing-stem retry and reconciliation

Boundary 47 carries head order 21 (x28 / SIG 55 / Java Inter 1399, grade bits
`0x3fe7e38e38e38e39`). Its authenticated LEFT/BOTTOM C-link envelope finds active
glyph 300 already owned by Stem Inter 2378, with two planned relations and one glyph.
Java allocates no Inter, adds no vertex, edge, or system stem, and leaves allocator
2382 unchanged. Phase-1 continuation observes LEFT already linked, RIGHT `Neither`,
and closes x27 / SIG 54 LEFT then RIGHT through two ordered false-to-true writes.
Native keeps SIG 682/693 and system stems 43, records no unlinked head, and advances to
`current_index=22` before x4 / SIG 7 / Java Inter 1299 (grade bits
`0x3fe7dcd4cd6e88ba`), whose LEFT side is linked and RIGHT remains open/unlinked.

The wrapper authenticates only this existing-stem retry frontier and its graph-derived
closure, failing closed on queue or head mismatch; it does not establish generic retry
or no-link behavior. The focused Boundary-47 gate and full 14-test sibling suite are
green; formatting, strict all-target Clippy, and diff checks pass.

The 17-line / 14,834-byte schema-v21 derivative contains twelve semantic rows plus
summary and remains snapshot-minimized: orders 1-20 reconstruct the predecessor
without emitted or persisted full snapshots, while order 21 emits the existing-stem
retry envelope/result and continuation. Fixture, runner, transformed probe,
emitted-body, and semantic-pass SHA-256 are
`9505955ce7e3322cbfaea818d0d42b5873fa78b1f5e1941756bcc44efcb04f55`,
`8cbd5d1de2e6e6b2b77d4ba94d99eb9f5813503a4afb960bb7511d0b92999ccd`,
`186e9fb81f3b39d1591b23b5f94c565152bfc81dc1d0e4781d460b1126f3ac4a`,
`9ea8929d70f49d8a39636ffece251ad1e13b3a443cdce57b62138f6ef0075293`, and
`a372eb0884f3679e62797343800beb70e8099c14267067f6d141f8c359216611`;
the v21 fragment source is pinned by
`f6a36215a86d9af177447069be271b0c4a84e4f8f56789d27769c161710c3629`.
This is bounded order-21 evidence, not independent predecessor snapshots or coverage
of order 22, actually-unlinked/no-link, phase-2 append, generic retry or
multi-item/recursive C-linkers, or broader corpus/system behavior.

## Boundary 48: second existing-stem retry and reconciliation

Boundary 48 carries head order 22 (x4 / SIG 7 / Java Inter 1299, grade bits
`0x3fe7dcd4cd6e88ba`). Its authenticated LEFT/BOTTOM C-link envelope has
`lastIndex=maxIndex=2`, two planned relations, and active glyphs 315 and 2142; canonical
glyph 315 is already owned by SIG-attached Stem Inter 2354. Java leaves allocator 2382
unchanged and adds no vertex, edge, glyph, or system stem. Phase-1 continuation
observes LEFT already linked and RIGHT `Neither`, then closes x3 / SIG 6 LEFT and RIGHT
through two ordered false-to-true writes. Native keeps SIG 682/693 and system stems 43,
records no unlinked head, and advances to `current_index=23` before x78 / SIG 39 /
Java Inter 1363 (grade bits `0x3fe7d236c1f8e275`), whose LEFT side is linked and RIGHT
remains open/unlinked.

The wrapper authenticates only this second existing-stem retry and its graph-derived
closure, failing closed if Stem 2354/glyph 315 is missing or detached; it does not
establish generic retry or no-link behavior. The focused Boundary-48 gate and full
14-test sibling suite are green; formatting, strict all-target Clippy, and diff checks
pass.

The 18-line / 16,188-byte schema-v22 derivative contains thirteen semantic rows plus
summary and remains snapshot-minimized: orders 1-21 reconstruct the predecessor
without emitted or persisted full snapshots, while order 22 emits the existing-stem
retry envelope/result and continuation. Fixture, runner, transformed probe,
emitted-body, and semantic-pass SHA-256 are
`e7bd66417228bf8fed7fe0c04d904e81ade4026fb00b4c17270b73947f85a1a4`,
`be1091ab266ea190a507291351f50bec4842f50003c75fb048f6bb96537ceebc`,
`fc6ada7afdc64f1e42f9fbf0c1f9353138a02ec285d24697fc68a90d49c3dfc7`,
`23d5da366efe5ce9d1bee9e7c5e3201677faef273075e23af68332a5e1f7e4bb`, and
`62c5ac9c30ea6bf3666cdb567bfa52d6d0a857578a5146ac91927f08adfa8c6a`;
the corrected v22 fragment source is pinned by
`576406fb3bd8bf9503ca883480bc55b217b3c6bc99ca440ef702774d3a2ca950`.
This is bounded order-22 evidence, not independent predecessor snapshots or coverage
of order 23, actually-unlinked/no-link, phase-2 append, generic retry or broader
C-linkers, or broader corpus/system behavior.

## Boundary 49: generic prelinked closure continuation

Boundary 49 adds no production operation. The existing phase-1 continuation carries
head order 23 (x78 / SIG 39 / Java Inter 1363, grade bits
`0x3fe7d236c1f8e275`). LEFT is already linked and RIGHT is `Neither`; incident Stem
2370 joins x77 / SIG 38 and x78 / SIG 39 on LEFT, so Java closes x77 LEFT then RIGHT
through two ordered false-to-true writes. Native records no unlinked head, keeps SIG
682/693 and system stems 43 unchanged, and advances to `current_index=24` before x93 /
SIG 25 / Java Inter 1335 (grade bits `0x3fe7d1c13d1c13d2`), whose LEFT side is linked
and RIGHT remains open/unlinked.

This is additional evidence for the unchanged generic prelinked-success path, not a
new retry implementation. The focused Boundary-49 gate and full 14-test sibling suite
are green; formatting, strict all-target Clippy, and diff checks pass.

The 19-line / 17,401-byte schema-v23 derivative contains fourteen semantic rows plus
summary and remains snapshot-minimized: orders 1-22 reconstruct the predecessor
without emitted or persisted full snapshots, while order 23 emits only the prelinked
closure and continuation. Fixture, runner, transformed probe, emitted-body, and
semantic-pass SHA-256 are
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

## Boundary 50: second generic prelinked closure continuation

Boundary 50 adds no production operation. The existing phase-1 continuation carries
head order 24 (x93 / SIG 25 / Java Inter 1335, grade bits
`0x3fe7d1c13d1c13d2`). LEFT is already linked and RIGHT is `Neither`; incident Stem
2342 joins x92 / SIG 24 and x93 / SIG 25 on LEFT, so Java closes x92 LEFT then RIGHT
through two ordered false-to-true writes. Native records no unlinked head, keeps SIG
682/693 and system stems 43 unchanged, and advances to `current_index=25` before x59 /
SIG 74 / Java Inter 1437 (grade bits `0x3fe7c31e7e01c29a`), whose LEFT side is linked
and RIGHT remains open/unlinked.

This is further evidence for the unchanged generic prelinked-success path, not a new
retry implementation. The focused Boundary-50 gate and full 14-test sibling suite are
green; formatting, strict all-target Clippy, and diff checks pass.

The 20-line / 18,614-byte schema-v24 derivative contains fifteen semantic rows plus
summary and remains snapshot-minimized: orders 1-23 reconstruct the predecessor
without emitted or persisted full snapshots, while order 24 emits only the prelinked
closure and continuation. Fixture, runner, transformed probe, emitted-body, and
semantic-pass SHA-256 are
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

## Boundary 51: third generic prelinked closure continuation

Boundary 51 adds no production operation. The existing phase-1 continuation carries
head order 25 (x59 / SIG 74 / Java Inter 1437, grade bits
`0x3fe7c31e7e01c29a`). LEFT is already linked and RIGHT is `Neither`; incident Stem
2363 joins x58 / SIG 73 and x59 / SIG 74 on LEFT, so Java closes x58 LEFT then RIGHT
through two ordered false-to-true writes. Native records no unlinked head, keeps SIG
682/693 and system stems 43 unchanged, and advances to `current_index=26` before x61 /
SIG 31 / Java Inter 1347 (grade bits `0x3fe7b8475abaafaf`), whose LEFT side is linked
and RIGHT remains open/unlinked.

This is further evidence for the unchanged generic prelinked-success path, not a new
retry implementation. The focused Boundary-51 gate and full 14-test sibling suite are
green; formatting, strict all-target Clippy, and diff checks pass.

The 21-line / 19,854-byte schema-v25 derivative contains sixteen semantic rows plus
summary and remains snapshot-minimized: orders 1-24 reconstruct the predecessor
without emitted or persisted full snapshots, while order 25 emits only the prelinked
closure and continuation. Fixture, runner, transformed probe, emitted-body, and
semantic-pass SHA-256 are
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

## Boundary 52: fourth generic prelinked closure continuation

Boundary 52 adds no production operation. The existing phase-1 continuation carries
head order 26 (x61 / SIG 31 / Java Inter 1347, grade bits
`0x3fe7b8475abaafaf`). LEFT is already linked and RIGHT is `Neither`; incident Stem
2345 joins x60 / SIG 30 and x61 / SIG 31 on LEFT, so Java closes x60 LEFT then RIGHT
through two ordered false-to-true writes. Native records no unlinked head, keeps SIG
682/693 and system stems 43 unchanged, and advances to `current_index=27` before x33 /
SIG 26 / Java Inter 1337 (grade bits `0x3fe7a22f6f5852b0`), whose LEFT and RIGHT sides
are both open/unlinked. Boundary 52 does not execute that next frontier.

This is further evidence for the unchanged generic prelinked-success path, not a new
retry implementation. The focused Boundary-52 gate and full 14-test sibling suite are
green; formatting, strict all-target Clippy, and diff checks pass.

The 22-line / 21,096-byte schema-v26 derivative contains seventeen semantic rows plus
summary and remains snapshot-minimized: orders 1-25 reconstruct the predecessor
without emitted or persisted full snapshots, while order 26 emits only the prelinked
closure and continuation. Fixture, runner, transformed probe, emitted-body, and
semantic-pass SHA-256 are
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

## Boundary 53: third both-open two-item C-link

Boundary 53 consumes head order 27 (x33 / SIG 26 / Java Inter 1337, grade bits
`0x3fe7a22f6f5852b0`) from a both-open/unlinked frontier. Java selects LEFT/BOTTOM,
expands a two-item builder (`lastIndex=maxIndex=1`) from active glyphs 314 and 2219,
reuses canonical glyph 314, and creates checked Stem Inter 2383 with one HeadStem
relation. Native creates dense Stem identity 43, moves SIG 682/693 to 683/694 and
system stems 43 to 44, links LEFT, records no closure write or unlinked head, and
advances to `current_index=28` before x85 / SIG 87 / Java Inter 1463 (grade bits
`0x3fe79e7f455ba48d`), whose LEFT side is linked and RIGHT remains open/unlinked.

Geometry remains bounded to this authenticated two-item LEFT/BOTTOM case; it does not
establish generic multi-item/recursive geometry or other corner shapes. The focused
Boundary-53 gate and full 14-test sibling suite are green; formatting, strict
all-target Clippy, and diff checks pass.

The 25-line / 25,740-byte schema-v27 derivative contains twenty semantic rows plus
summary and remains snapshot-minimized: orders 1-26 reconstruct the predecessor
without emitted or persisted full snapshots, while order 27 emits its C-link
envelope/result and continuation. Fixture, runner, transformed probe, emitted-body,
and semantic-pass SHA-256 are
`1ba59491992fdd7bd2355e2617b437b84433d3c449cc8f7606cdc0a1e70ac0aa`,
`f2c1942b3ff6f00a75bb876b6d6d4b53ba2d999bcb5ddaeb88f6dc86850fcdc5`,
`5f4c5a69c9fe5e87f23eff31b1524e80459a04a298689609fa80ef142f1cd9c6`,
`bd006771fb4878072bb24f54cc22efd507dd5114d5e60fccff76479b2cb25c1c`, and
`1033282335cace626465424615847b3e190c718f25acf2fd70e1a6a2d50ec7d7`;
the v27 fragment source is pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-27 evidence, not independent predecessor snapshots or coverage
of order 28, actually-unlinked/no-link, phase-2 append, generic retry or broader
C-linkers, or broader corpus/system behavior.

## Boundary 54: minimized four-write prelinked closure

Boundary 54 adds no production operation. The existing phase-1 continuation carries
head order 28 (x85 / SIG 87 / Java Inter 1463, grade bits
`0x3fe79e7f455ba48d`). LEFT is already linked and RIGHT is `Neither`; incident Stem
2366 joins x84 / SIG 86, x85 / SIG 87, and x86 / SIG 85 on LEFT. Java closes x84 LEFT
then RIGHT and x86 LEFT then RIGHT through four ordered false-to-true writes. Native
records no unlinked head, keeps SIG 683/694 and system stems 44 unchanged, and advances
to `current_index=29` before x10 / SIG 9 / Java Inter 1303 (grade bits
`0x3fe79713252eb76a`), whose LEFT side is linked and RIGHT remains open/unlinked.

The default full-snapshot order-28 oracle exhausted the JVM heap. The replacement is
deliberately minimized: orders 1-27 mutate without snapshots, and only the authenticated
order-0 baseline/C-link evidence plus the order-28 closure row are emitted. This does
not independently snapshot-oracle the predecessor sequence. The focused Boundary-54
gate and full 14-test sibling suite are green; formatting, strict all-target Clippy,
and diff checks pass.

The 12-line / 8,381-byte schema-v28 derivative contains seven semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `6f30a5cb8706fb0445b5eb84cee2896dfa1b85236f6870a97177714672ef10b7`,
`ec1985d786f0c984f5a09a461008911f12777229b0a08eb71b7e36a39d548d82`,
`d2e07d5dacf3e22ec20a3f53c8e4543763982eec3e88eac1ac8e8e3368422cc2`,
`5a4675dca2831e93c61a028a6d189deed21115e9588e06b1293c37968fd2bef5`, and
`b4d16e19a892bfb0537f8b7b629e43617687f19794a2cf13332a0e69cdd4e1fd`;
the shared v27/v28 fragment source is pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-28 evidence, not coverage of order 29, actually-unlinked/no-link,
phase-2 append, generic retry or broader C-linkers, or broader corpus/system behavior.

## Boundary 55: minimized two-write prelinked closure

Boundary 55 adds no production operation. The existing phase-1 continuation carries
head order 29 (x10 / SIG 9 / Java Inter 1303, grade bits
`0x3fe79713252eb76a`). LEFT is already linked and RIGHT is `Neither`; incident Stem
2355 joins x9 / SIG 8 and x10 / SIG 9 on LEFT, so Java closes x9 LEFT then RIGHT
through two ordered false-to-true writes. Native records no unlinked head, keeps SIG
683/694 and system stems 44 unchanged, and advances to `current_index=30` before x101
/ SIG 43 / Java Inter 1371 (grade bits `0x3fe79406c6921d2e`), whose LEFT side is linked
and RIGHT remains open/unlinked.

The v29 oracle retains v28's heap-safe minimized shape: orders 1-28 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-29
closure row are emitted. It does not independently snapshot-oracle the predecessor
sequence. The focused Boundary-55 gate and full 14-test sibling suite are green;
formatting, strict all-target Clippy, and diff checks pass.

The 12-line / 8,292-byte schema-v29 derivative contains seven semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `a88b9fd3c27133c3c8bdcc839308365557c0e95c2ac3ea83fe348dc0d1ffa270`,
`0ae5afb409d11eef138ed62bb8adbefb04eabfa99c0581cad7a6952ecb5e1d4c`,
`79ddfc2cf532474ff902156eb66c2655ec242ac0a73884fd67bfc74afb6521ca`,
`410d0c1e04f4c7dfb1b4b83ed0953da53e52605df72e08351231e302027ca84a`, and
`32c937944c1c015c79ca4993dd299bef9c32ef39e7be71c800ca98d21ccd5cde`;
the shared v27-v29 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-29 evidence, not coverage of order 30, actually-unlinked/no-link,
phase-2 append, generic retry or broader C-linkers, or broader corpus/system behavior.

## Boundary 56: next minimized two-write prelinked closure

Boundary 56 adds no production operation. The existing phase-1 continuation carries
head order 30 (x101 / SIG 43 / Java Inter 1371, grade bits
`0x3fe79406c6921d2e`). LEFT is already linked and RIGHT is `Neither`; incident Stem
2343 joins x100 / SIG 42 and x101 / SIG 43 on LEFT, so Java closes x100 LEFT then
RIGHT through two ordered false-to-true writes. Native records no unlinked head, keeps
SIG 683/694 and system stems 44 unchanged, and advances to `current_index=31` before
x16 / SIG 81 / Java Inter 1451 (grade bits `0x3fe75f1fc300149f`), whose LEFT side is
linked and RIGHT remains open/unlinked.

The v30 oracle retains v28's heap-safe minimized shape: orders 1-29 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-30
closure row are emitted. It does not independently snapshot-oracle the predecessor
sequence. The focused Boundary-56 gate and full 14-test sibling suite are green;
formatting, strict all-target Clippy, and diff checks pass.

The 12-line / 8,306-byte schema-v30 derivative contains seven semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `c4bde8384b872a03d7f9d7ecd87fdea60dc93a5b418ca831c8dbe5d8c3aa729d`,
`d8f55efad82e15eb8b45c52ac8f99031c00ea0dd7143bc30c7c607fc103e71cf`,
`a8b50543359666567a01d503f46616d113feee03ac60828104d2b52efc558812`,
`8eebd2a60dfdaf3896a31d7200525fc70667bed42ee8cbcc0076830bae74bd40`, and
`803635259310df2794ac302b43a7b8286c95fb117f6f19177071c1ce25d484a9`;
the shared v27-v30 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-30 evidence, not coverage of order 31, actually-unlinked/no-link,
phase-2 append, generic retry or broader C-linkers, or broader corpus/system behavior.

## Boundary 57: another minimized two-write prelinked closure

Boundary 57 adds no production operation. The existing phase-1 continuation carries
head order 31 (x16 / SIG 81 / Java Inter 1451, grade bits
`0x3fe75f1fc300149f`). LEFT is already linked and RIGHT is `Neither`; incident Stem
2360 joins x15 / SIG 80 and x16 / SIG 81 on LEFT, so Java closes x15 LEFT then RIGHT
through two ordered false-to-true writes. Native records no unlinked head, keeps SIG
683/694 and system stems 44 unchanged, and advances to `current_index=32` before x34
/ SIG 77 / Java Inter 1443 (grade bits `0x3fe75353cd1ba641`), whose LEFT side is linked
and RIGHT remains open/unlinked.

The v31 oracle retains v28's heap-safe minimized shape: orders 1-30 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-31
closure row are emitted. It does not independently snapshot-oracle the predecessor
sequence. The focused Boundary-57 gate and full 14-test sibling suite are green;
formatting, strict all-target Clippy, and diff checks pass.

The 12-line / 8,302-byte schema-v31 derivative contains seven semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `ab58a7bf7d5a2265fbd8cc2a18ee0595b7d288935469cf27f91e01ace9397b00`,
`e7b8cd3bc87ff55969aee203b6027f7af572428cf91d442f94ea58e8f82d3e42`,
`231028452d789e78ec96e5dc1c2f8ccabe88d85ac59aa9f990e18a0775d44404`,
`34baf86107a36d017519d7ac0f0011a0eb8d67f93a5d9b2d95f55ccf0784dcc4`, and
`3d123d0fcd70cdcdc3436a1ffca7b85ecac9e1a350c6a83368f91175e35eb4e4`;
the shared v27-v31 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-31 evidence, not coverage of order 32, actually-unlinked/no-link,
phase-2 append, generic retry or broader C-linkers, or broader corpus/system behavior.

## Boundary 58: minimized zero-write prelinked closure

Boundary 58 adds no production operation. The existing phase-1 continuation carries
head order 32 (x34 / SIG 77 / Java Inter 1443, grade bits
`0x3fe75353cd1ba641`). LEFT is already linked and RIGHT is `Neither`; incident Stem
2368 contains only x34 on LEFT, so Java returns with no closure writes or changed
linker values. Native records no unlinked head, keeps SIG 683/694 and system stems 44
unchanged, and advances to `current_index=33` before x88 / SIG 84 / Java Inter 1457
(grade bits `0x3fe73605f8f111a6`), whose LEFT side is linked and RIGHT remains
open/unlinked.

The v32 oracle retains v28's heap-safe minimized shape: orders 1-31 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-32
no-op closure row are emitted. It does not independently snapshot-oracle the
predecessor sequence. The focused Boundary-58 gate and full 14-test sibling suite are
green; formatting, strict all-target Clippy, and diff checks pass.

The 12-line / 8,230-byte schema-v32 derivative contains seven semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `cceda3e1b00ccf9e4ca5f701c71a0a4da4e764488e192bf056ea645f11ad72c4`,
`fecd661b0c9b9e03f17c9eba3482a86b7f2ae381e49ac93bbbcbfea4756c3cd8`,
`d1b3d61c46bfdfe540d33ae751d0006c4518142d8410277d8e4016a4b29b1fe5`,
`77810c34e97279aef05feaa043df82a7ab4ba1566edc5933f38ff80608f10191`, and
`23a5e25617ef107a7f4b2b85ddb977d8d8b164d0203477d9f995d2d90df55bf5`;
the shared v27-v32 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-32 evidence, not coverage of order 33, actually-unlinked/no-link,
phase-2 append, generic retry or broader C-linkers, or broader corpus/system behavior.

## Boundary 59: minimized two-write closure before a both-open frontier

Boundary 59 adds no production operation. The existing phase-1 continuation carries
head order 33 (x88 / SIG 84 / Java Inter 1457, grade bits
`0x3fe73605f8f111a6`). LEFT is already linked and RIGHT is `Neither`; incident Stem
2367 joins x87 / SIG 83 and x88 / SIG 84 on LEFT, so Java closes x87 LEFT then RIGHT
through two ordered false-to-true writes. Native records no unlinked head, keeps SIG
683/694 and system stems 44 unchanged, and advances to `current_index=34` before x2 /
SIG 36 / Java Inter 1357 (grade bits `0x3fe71d98bc61a5b3`), whose LEFT and RIGHT sides
are both open/unlinked.

The v33 oracle retains v28's heap-safe minimized shape: orders 1-32 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-33
closure row are emitted. It does not independently snapshot-oracle the predecessor
sequence. The focused Boundary-59 gate and full 14-test sibling suite are green;
formatting, strict all-target Clippy, and diff checks pass.

The 12-line / 8,302-byte schema-v33 derivative contains seven semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `a058341d3f661be4a677206c7a067f39a0785ae5adeed96be7d7073541fe2982`,
`472e88ea561df7db9280c5ec79a2ea8a5204783d3ffec8894455adcf5b342692`,
`2e4a07c2efbdf0e43bb92f9bd6213cd6faf3e2a0e39eed610f11e13e15e42d72`,
`2c212c41b06dc509b217ced1e3c0bedfd6c538f3684a705eeafc3e60ff33aed4`, and
`8aec2db3857f6d2d8dc60bdb381ce3cfb8a16a1e441efd348854489bbcc53b43`;
the shared v27-v33 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-33 evidence, not coverage of order 34, its both-open C-link
geometry, actually-unlinked/no-link, phase-2 append, generic retry, or broader
corpus/system behavior.

## Boundary 60: bounded x2 two-item LEFT/BOTTOM C-link

Boundary 60 consumes the authenticated both-open order-34 frontier for x2 / SIG 36 /
Java Inter 1357 (grade bits `0x3fe71d98bc61a5b3`). The LEFT/BOTTOM C-link selects
active glyphs 322 and 1946, reuses glyph 322 as the modeled candidate, creates Java
Stem 2384 / native Stem identity 44, and adds the Inter1357-to-Stem2384 relation.
Native advances SIG 683/694 to 684/695 and system stems 44 to 45, records no unlinked
head or closure write, and reaches `current_index=35` before x50 / SIG 72 / Java Inter
1433 (grade bits `0x3fe6dc9c073bac4e`), whose LEFT is linked and RIGHT remains
open/unlinked.

The x2 geometry needs one measured, bounded correction: after direct centroid/
intersection translation, Java's two translated stem-line x coordinates are each one
representable step above the native interpolation, so `java_next_up` is applied only
for the authenticated x2 frontier. This is not a generic C-link geometry rule.

The v34 oracle retains the heap-safe minimized shape: orders 1-33 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-34
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused Boundary-60 gate and full
14-test sibling suite are green; formatting, strict all-target Clippy, and diff checks
pass.

The 14-line / 11,693-byte schema-v34 derivative contains nine semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `b67514520fa848fd9758d0bdc740d2be4600c723ac341b57fced42f4657103a8`,
`60b4cc5a9e0a9fe5c6d4a8bb1b03bfadf065259c07bc124c6587b3d7a9c3a93f`,
`4cec5bfe6379e31701b7e4ea4f2ad98a8d36680daefa7a8a8d9d4c179d2c6777`,
`85d05cca18e6b15414729404191bd84d0729bf657678b0dfaa626ab72b915ae4`, and
`486957d6a77dce18fc15bd92761e5624e6b8edef9705d35403f00419b011b4dd`;
the shared v27-v34 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-34 evidence, not coverage of order 35, generic multi-item or
recursive C-link geometry, actually-unlinked/no-link, phase-2 append, generic retry,
or broader corpus/system behavior.

## Boundary 61: minimized two-write closure after x2

Boundary 61 adds no production operation. The existing phase-1 continuation carries
head order 35 (x50 / SIG 72 / Java Inter 1433, grade bits
`0x3fe6dc9c073bac4e`). LEFT is already linked and RIGHT is `Neither`; incident Stem
2353 joins x49 / SIG 71 and x50 / SIG 72 on LEFT, so Java closes x49 LEFT then RIGHT
through two ordered false-to-true writes. Native records no unlinked head, keeps SIG
684/695 and system stems 45 unchanged, and advances to `current_index=36` before x23 /
SIG 14 / Java Inter 1313 (grade bits `0x3fe6bf73ff00cd94`), whose LEFT and RIGHT sides
are both open/unlinked.

The v35 oracle retains the heap-safe minimized shape: orders 1-34 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-35
closure row are emitted. It does not independently snapshot-oracle the predecessor
sequence. The focused Boundary-61 gate and full 14-test sibling suite are green;
formatting, strict all-target Clippy, and diff checks pass.

The 12-line / 8,302-byte schema-v35 derivative contains seven semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `2721b843514ce7a695fdacc797addd21597bd604b39168fe63533ecfc01bd55b`,
`74aec11451cb5933938b3bc82876ddfdb9e4bdab295e472644698c68d2cbc5ea`,
`611a02c34f4690031db91ce7ccced19ef6a1d7ec3d6da0dd81333f07aa315b42`,
`12d32f9193480ac9772e735735210b3689266458f4dad379a72131bb9024cc84`, and
`992372127972f17882a8f672653b9b4530497d06c8c6a43f6209ad6c8e22a1dd`;
the shared v27-v35 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-35 evidence, not coverage of order 36, its both-open C-link
geometry, actually-unlinked/no-link, phase-2 append, generic retry, or broader
corpus/system behavior.

## Boundary 62: bounded single-item C-link at x23

Boundary 62 carries head order 36 (x23 / SIG 14 / Java Inter 1313, grade bits
`0x3fe6bf73ff00cd94`). Its LEFT/BOTTOM C-link selects and reuses active glyph 324,
creates Java Stem 2385 / native Stem identity 46, and adds the
Inter1313-to-Stem2385 relation as edge 1313. Native advances SIG 684/695 to 685/696
and system stems 45 to 46, links the LEFT side without a closure write or unlinked
head, and reaches `current_index=37` before x14 / SIG 1 / Java Inter 1287 (grade bits
`0x3fe6b52921e6cda3`), whose LEFT is linked and RIGHT remains open/unlinked.

The v36 oracle retains the heap-safe minimized shape: orders 1-35 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-36
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused Boundary-62 gate and full
14-test sibling suite are green; formatting, strict all-target Clippy, and diff checks
pass.

The 14-line / 11,600-byte schema-v36 derivative contains nine semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `7d7d0d17e51c03a145bdff3a739da3aaaa05fb0c5bba20cd9a46468742eb26e7`,
`3176407de9bdd88f167e925a2b901f811f230f6b83c5a120ddf031a42ec49fd4`,
`582922fe7442de97a34732791352550e0026d9cf16cae36d633266eb15273aba`,
`7f61d1814c2542ae95f54515aa97a8f35ed3be2905e87336c15a83a0d8c6489b`, and
`f3e073ba83536e4afc1c0ea13a5933f199cfad57f1b82b6974f5abb9081039bd`;
the shared v27-v36 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-36 single-item LEFT/BOTTOM evidence, not coverage of order 37,
generic multi-item or recursive C-link geometry, actually-unlinked/no-link, phase-2
append, generic retry, or broader corpus/system behavior.

## Boundary 63: existing-stem reconciliation at x14

Boundary 63 carries head order 37 (x14 / SIG 1 / Java Inter 1287, grade bits
`0x3fe6b52921e6cda3`). Its LEFT side is already linked and its four-relation
LEFT/BOTTOM candidate resolves active glyph 294 to existing Stem 2340, so allocator,
SIG 685/696, and system-stem count 46 remain unchanged. RIGHT is `Neither`; incident
Stem 2340 joins x13 / SIG 0 and x14 / SIG 1 on LEFT, so Java closes x13 LEFT then
RIGHT through two ordered false-to-true writes. Native records no unlinked head and
reaches `current_index=38` before x18 / SIG 4 / Java Inter 1293 (grade bits
`0x3fe6b1ad86c7d182`), whose LEFT is linked and RIGHT remains open/unlinked.

The v37 oracle retains the heap-safe minimized shape: orders 1-36 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-37
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused Boundary-63 gate and full
14-test sibling suite are green; formatting, strict all-target Clippy, and diff checks
pass.

The 14-line / 12,303-byte schema-v37 derivative contains nine semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `5af8e1928df00217e1780e2e6e0d057c4202b0f1cf46f25d5d889678c5fdf2b8`,
`2fac40e0bf6f49186a994bae499aa371be8bee2152297d325bae067c3f8d5bc1`,
`58ed9ebbd2fa05e9e52349b5ad42195a8f9fe534b46e088e6be7dd850d6ab1bb`,
`4c69d4c1740899bf4c71dbc895f022882f87d772233642f638ba3ecdc4db3fb1`, and
`fcb9fec2a764e9ab06d6b91ca856a8832ef236754dcb45ba345ae3f8f7280d90`;
the shared v27-v37 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-37 existing-stem reconciliation evidence, not coverage of order
38, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

## Boundary 64: second consecutive existing-stem reconciliation

Boundary 64 carries head order 38 (x18 / SIG 4 / Java Inter 1293, grade bits
`0x3fe6b1ad86c7d182`). Its LEFT side is already linked and its two-relation
LEFT/BOTTOM candidate resolves active glyph 310 to existing Stem 2372, so allocator,
SIG 685/696, and system-stem count 46 remain unchanged. RIGHT is `Neither`; incident
Stem 2372 joins x17 / SIG 10 and x18 / SIG 4 on LEFT, so Java closes x17 LEFT then
RIGHT through two ordered false-to-true writes. Native records no unlinked head and
reaches `current_index=39` before x97 / SIG 34 / Java Inter 1353 (grade bits
`0x3fe666c6bb717a2e`), whose LEFT is linked and RIGHT remains open/unlinked.

The v38 oracle retains the heap-safe minimized shape: orders 1-37 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-38
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused Boundary-64 gate and full
14-test sibling suite are green; formatting, strict all-target Clippy, and diff checks
pass.

The 14-line / 11,312-byte schema-v38 derivative contains nine semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `98c8d3c19d50df531d756d6fd50ddbc9f07ce7db24bea47849fff731d5271b0f`,
`ad2edbfdf046db3a27b67d81da23f6f30d254cde9c91eb92063df72da10c7551`,
`8da7b91134b4ae654461eecd7f4f5009e3fe205f140663dad836b0820465a214`,
`64a375a90ec14b1e4735027c53a2f650774eb22f8ec6cc4884dacddf008ef859`, and
`57e46879aca3fc5a02851b590a14347df4535beff0b7c97855d42afe95155422`;
the shared v27-v38 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-38 existing-stem reconciliation evidence, not coverage of order
39, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

## Boundary 65: third consecutive existing-stem reconciliation

Boundary 65 carries head order 39 (x97 / SIG 34 / Java Inter 1353, grade bits
`0x3fe666c6bb717a2e`). Its LEFT side is already linked and its two-relation
LEFT/BOTTOM candidate resolves active glyph 321 to existing Stem 2373, so allocator,
SIG 685/696, and system-stem count 46 remain unchanged. RIGHT is `Neither`; incident
Stem 2373 joins x96 / SIG 41 and x97 / SIG 34 on LEFT, so Java closes x96 LEFT then
RIGHT through two ordered false-to-true writes. Native records no unlinked head and
reaches `current_index=40` before x6 / SIG 89 / Java Inter 1467 (grade bits
`0x3fe65e4f5c70ff04`), whose LEFT is linked and RIGHT remains open/unlinked.

The v39 oracle retains the heap-safe minimized shape: orders 1-38 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-39
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused Boundary-65 gate and full
14-test sibling suite are green; formatting, strict all-target Clippy, and diff checks
pass.

The 14-line / 11,315-byte schema-v39 derivative contains nine semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `771b7816918d098e66fa1c599df1a68bfb3e24d1724ea6f701ba3bcc59b031fa`,
`bf7855c0d53d59cea3593de72f51f7272168f488e65148267ebd55e9f70110c7`,
`990556c3e12f99826c6ca92596045d44cec482263c76040613b8afc1bfd796d8`,
`6f3518552f431fd0108d3c64efc6d5c2a99cd57ff841f8cbcc2987ecb80c6090`, and
`2d51ffd86926e5a39870f9e5d1222d359f28121a4f5e9ccda9b072e5fd94b73b`;
the shared v27-v39 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-39 existing-stem reconciliation evidence, not coverage of order
40, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

## Boundary 66: fourth consecutive existing-stem reconciliation

Boundary 66 carries head order 40 (x6 / SIG 89 / Java Inter 1467, grade bits
`0x3fe65e4f5c70ff04`). Its LEFT side is already linked and its three-relation
LEFT/BOTTOM candidate resolves active glyph 290 to existing Stem 2348, so allocator,
SIG 685/696, and system-stem count 46 remain unchanged. RIGHT is `Neither`; incident
Stem 2348 joins x5 / SIG 88 and x6 / SIG 89 on LEFT, so Java closes x5 LEFT then
RIGHT through two ordered false-to-true writes. Native records no unlinked head and
reaches `current_index=41` before x30 / SIG 67 / Java Inter 1423 (grade bits
`0x3fe63a0d1316bff0`), whose LEFT is linked and RIGHT remains open/unlinked.

The v40 oracle retains the heap-safe minimized shape: orders 1-39 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-40
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused Boundary-66 gate and full
14-test sibling suite are green; formatting, strict all-target Clippy, and diff checks
pass.

The 14-line / 11,761-byte schema-v40 derivative contains nine semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `26e4a2ecbd547829c573c4c7737331e4773f6faf64581ecfdf380a6b87283fa9`,
`7caaaf046770aafb327359fc587ed54509a83ec867a90a8c53cd254b2de5cb45`,
`36408206fc9d1f7640b1464ff9a95be6039ce77e21485891f0f889dd0cf52f84`,
`9be2634c8582ff4f023e17313aa9b91524b542d07c3c69363906b1d1e05acaa6`, and
`fa014228f89fbba214adaa1525ae8206de28f919ac71b334e2da01587f399db8`;
the shared v27-v40 fragment source remains pinned by
`4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c`.
This is bounded order-40 existing-stem reconciliation evidence, not coverage of order
41, actually-unlinked/no-link, phase-2 append, generic retry, broader C-link geometry,
or broader corpus/system behavior.

## Boundary 67: fifth consecutive existing-stem reconciliation

Boundary 67 carries head order 41 (x30 / SIG 67 / Java Inter 1423, grade bits
`0x3fe63a0d1316bff0`). Its LEFT side is already linked and its four-relation
LEFT/BOTTOM candidate resolves active glyph 313 to existing Stem 2357, so allocator,
SIG 685/696, and system-stem count 46 remain unchanged. RIGHT is `Neither`; incident
Stem 2357 joins x29 / SIG 66 and x30 / SIG 67 on LEFT, so Java closes x29 LEFT then
RIGHT through two ordered false-to-true writes. Native records no unlinked head and
reaches `current_index=42` before x43 / SIG 48 / Java Inter 1385 (grade bits
`0x3fe5f802e7abc18c`), whose LEFT is linked and RIGHT remains open/unlinked.

The v41 oracle retains the heap-safe minimized shape: orders 1-40 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-41
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused Boundary-67 gate is green 1/1;
the full 14-test sibling suite, strict workspace/all-target/all-features Clippy,
formatting, and diff checks also pass.

The 14-line / 12,312-byte schema-v41 derivative contains nine semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `7bb4ebb479617804363078144c55570d1c76229de551492c7cb14050641f1962`,
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

## Boundary 68: sixth consecutive existing-stem reconciliation

Boundary 68 carries head order 42 (x43 / SIG 48 / Java Inter 1385, grade bits
`0x3fe5f802e7abc18c`). Its LEFT side is already linked and its two-relation
LEFT/BOTTOM candidate resolves active glyph 326 to existing Stem 2350, so allocator,
SIG 685/696, and system-stem count 46 remain unchanged. RIGHT is `Neither`; incident
Stem 2350 joins x39 / SIG 37, x40 / SIG 27, and x43 / SIG 48 on LEFT, so Java closes
x39 LEFT then RIGHT and x40 LEFT then RIGHT through four ordered false-to-true writes.
Native records no unlinked head and reaches `current_index=43` before x25 / SIG 91 /
Java Inter 1471 (grade bits `0x3fe5db5645fe3490`), whose LEFT is linked and RIGHT
remains open/unlinked.

The v42 oracle retains the heap-safe minimized shape: orders 1-41 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-42
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused Boundary-68 gate is green 1/1;
the full 14-test sibling suite, strict workspace Clippy, formatting, and diff checks
also pass.

The 14-line / 11,783-byte schema-v42 derivative contains nine semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `64b55e449e38f7af6ed47c1ca026236772a277ac8c5917bc5eaea397125b332c`,
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

## Boundary 69: seventh consecutive existing-stem reconciliation

Boundary 69 carries head order 43 (x25 / SIG 91 / Java Inter 1471, grade bits
`0x3fe5db5645fe3490`). LEFT is already linked and its three-relation LEFT/BOTTOM
candidate resolves active glyph 292 to existing Stem 2356, so allocator, SIG 685/696,
and system-stem count 46 remain unchanged. RIGHT is `Neither`; incident Stem 2356
joins x24 / SIG 90 and x25 / SIG 91 on LEFT, so Java closes x24 LEFT then RIGHT
through two ordered false-to-true writes. Native records no unlinked head and reaches
`current_index=44` before x83 / SIG 21 / Java Inter 1327 (grade bits
`0x3fe5b836536dd665`), whose LEFT is linked and RIGHT remains open/unlinked.

The v43 oracle retains the heap-safe minimized shape: orders 1-42 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-43
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused Boundary-69 gate is green 1/1;
the full 14-test sibling suite, strict workspace Clippy, formatting, and diff checks
also pass.

The 14-line / 11,885-byte schema-v43 derivative contains nine semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `dc5f7ce12d292a13cc149e7df0249703323df92de9054daf5eff52783b32919d`,
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

## Boundary 70: eighth consecutive existing-stem reconciliation

Boundary 70 carries head order 44 (x83 / SIG 21 / Java Inter 1327, grade bits
`0x3fe5b836536dd665`). LEFT is already linked and its two-relation LEFT/BOTTOM
candidate resolves active glyph 301 to existing Stem 2358, so allocator, SIG 685/696,
and system-stem count 46 remain unchanged. RIGHT is `Neither`; incident Stem 2358
joins x82 / SIG 20 and x83 / SIG 21 on LEFT, so Java closes x82 LEFT then RIGHT
through two ordered false-to-true writes. Native records no unlinked head and reaches
`current_index=45` before x57 / SIG 5 / Java Inter 1295 (grade bits
`0x3fe593d56730c827`), whose LEFT is linked and RIGHT remains open/unlinked.

The v44 oracle retains the heap-safe minimized shape: orders 1-43 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-44
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused Boundary-70 gate is green 1/1;
the full 14-test sibling suite, strict workspace Clippy, formatting, and diff checks
also pass.

The 14-line / 11,456-byte schema-v44 derivative contains nine semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `1d5c98477377e64e95a659fa04ed8d8331e02d5e87962811b790ff80f0315515`,
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

## Boundary 71: ninth consecutive existing-stem reconciliation

Boundary 71 carries head order 45 (x57 / SIG 5 / Java Inter 1295, grade bits
`0x3fe593d56730c827`). LEFT is already linked and its two-relation LEFT/BOTTOM
candidate resolves active glyph 303 to existing Stem 2374, so allocator, SIG 685/696,
and system-stem count 46 remain unchanged. RIGHT is `Neither`; incident Stem 2374
joins x56 / SIG 15 and x57 / SIG 5 on LEFT, so Java closes x56 LEFT then RIGHT through
two ordered false-to-true writes. Native records no unlinked head and reaches
`current_index=46` before x40 / SIG 27 / Java Inter 1339 (grade bits
`0x3fe3aa2e83097210`), whose LEFT is linked/closed and RIGHT is unlinked/closed.

The v45 oracle retains the heap-safe minimized shape: orders 1-44 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-45
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused Boundary-71 gate is green 1/1;
the full 14-test sibling suite, strict workspace Clippy, formatting, and diff checks
also pass.

The 14-line / 11,415-byte schema-v45 derivative contains nine semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `f70a5aeee405899ee2e9bf3be6957ffa657c6f0bcd5bc5d84ab0fc0288b19073`,
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

## Boundary 72: tenth consecutive existing-stem reconciliation

Boundary 72 carries head order 46 (x40 / SIG 27 / Java Inter 1339, grade bits
`0x3fe3aa2e83097210`). LEFT is already linked and already closed; its two-relation
LEFT/BOTTOM candidate resolves active glyph 326 to existing Stem 2350, so allocator,
SIG 685/696, and system-stem count 46 remain unchanged. RIGHT is closed. Incident Stem
2350 joins x39 / SIG 37, x40 / SIG 27, and x43 / SIG 48 on LEFT. Java still emits the
ordered x39 LEFT and RIGHT true-to-true writes, then x43 LEFT and RIGHT false-to-true
writes; `closedValueChanges=2`. Native records no unlinked head and reaches
`current_index=47` before x89 / SIG 22 / Java Inter 1329 (grade bits
`0x3fd6ac9dfd130464`), whose LEFT is linked/closed and RIGHT is unlinked/closed.

The v46 oracle retains the heap-safe minimized shape: orders 1-45 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-46
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused Boundary-72 gate is green 1/1;
the full 14-test sibling suite, strict workspace Clippy, formatting, and diff checks
also pass.

The 14-line / 11,504-byte schema-v46 derivative contains nine semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `017cfeddc3faeedda3aca5308c82251135bd0c3308854385f77271cb7fc76f8d`,
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

## Boundary 73: eleventh consecutive existing-stem reconciliation

Boundary 73 carries head order 47 (x89 / SIG 22 / Java Inter 1329, grade bits
`0x3fd6ac9dfd130464`). LEFT is already linked and closed; its one-relation LEFT/BOTTOM
candidate resolves active glyph 304 to existing Stem 2359, so allocator, SIG 685/696,
and system-stem count 46 remain unchanged. RIGHT is closed. Incident Stem 2359 joins
x89 / SIG 22 and x90 / SIG 23 on LEFT. Java closes x90 LEFT then RIGHT through two
ordered false-to-true writes, with exact `closedValueChanges=2`. Native records no
unlinked head and reaches `current_index=48` before x52 / SIG 2 / Java Inter 1289
(grade bits `0x3fd5af02eef9418a`), whose LEFT is linked/closed and RIGHT is
unlinked/closed.

The v47 oracle retains the heap-safe minimized shape: orders 1-46 mutate without
snapshots, and only authenticated order-0 baseline/C-link evidence plus the order-47
frontier, result, and continuation rows are emitted. It does not independently
snapshot-oracle the predecessor sequence. The focused Boundary-73 gate is green 1/1;
the full 14-test sibling suite, strict workspace Clippy, formatting, and diff checks
also pass.

The 14-line / 10,882-byte schema-v47 derivative contains nine semantic rows plus
summary. Fixture, runner, transformed probe, emitted-body, and semantic-pass SHA-256
are `5a7989434b78dbd6ea72f113cd9f66078ae8e9c3acabb8980ecdb7577120de39`,
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

## Boundary 74: twelfth consecutive existing-stem reconciliation

Boundary 74 carries order 48 (x52 / SIG 2 / Java Inter 1289, grade bits
`0x3fd5af02eef9418a`). LEFT is already linked and closed; its four-relation
LEFT/BOTTOM candidate resolves active glyph 296 to existing Stem 2344. RIGHT is
closed, so Java takes `SkipAlreadyLinked` plus `SkipClosed`, closes x53 LEFT then
RIGHT, and reports exact `closedValueChanges=2`. Native adds no graph, registry, or
system-stem state, records no unlinked head, and reaches `current_index=49` before
x35 / SIG 68 / Java Inter 1425 (grade bits `0x3fd525fff19ec48c`).

The snapshot-minimized v48 oracle mutates orders 1-47 without snapshots and is not
independent predecessor-snapshot evidence. Focused 1/1, full 14/14, strict workspace
Clippy, formatting, and diff checks pass. The 14-line / 12,148-byte fixture has SHA
`acc3436794b0ea828dbd689adfd072b6844125007131ee4207d9d4402c90cd5d`;
runner/probe/body/semantic pins are
`925536d8d119102e5a74a3690b2286bde856bd476151243806d68a049aa40fdb`,
`af7f62ae73911530d863cbf8e4f2ee8bb3d019cfb556185e5fca334cad8a318d`,
`aa738347bf8581a87c5293e9b549261946b9adfef21c3e07e7d37ebdb21e2907`, and
`1183c4dce1c645a0ee070f1bd12d8796b22d9f0bde91c9421c3bc75db833a80f`;
base v47 runner/fixture remain `7a9605cf09f1d78f899423a816c0c6adc2b121786f56c69c271b41da5527f6ab`
and `5a7989434b78dbd6ea72f113cd9f66078ae8e9c3acabb8980ecdb7577120de39`.

This is bounded order-48 existing-stem evidence, not later queue, no-link/retry,
phase-2, broader geometry, or wider-corpus coverage.

## Boundary 75: thirteenth consecutive existing-stem reconciliation

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

## Boundary 76: first carried returned-false undef

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

## Boundary 77: sixteenth existing-stem reconciliation

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

## Boundary 78: seventeenth existing-stem reconciliation

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

## Boundary 79: eighteenth existing-stem reconciliation, three-head stem

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

## Boundary 80: nineteenth existing-stem reconciliation

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

## Boundary 81: twentieth existing-stem reconciliation

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

## Boundary 82: twenty-first existing-stem reconciliation

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

## Boundary 83: first existing-stem C-link reuse

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

## Boundary 84: twenty-second existing-stem reconciliation

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

## Boundary 85: twenty-third existing-stem reconciliation

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

## Boundary 86: second carried returned-false undef

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

## Boundary 87: third carried returned-false undef

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

## Boundary 88: twenty-fourth existing-stem reconciliation

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

## Boundary 89: twenty-fifth existing-stem reconciliation

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

## Boundary 90: twenty-sixth existing-stem reconciliation

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

## Boundary 91: twenty-seventh existing-stem reconciliation

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

## Boundary 92: twenty-eighth existing-stem reconciliation

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

## Boundary 93: first multi-head existing-stem C-link reuse

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

## Boundary 94: fourth carried returned-false undef

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

## Boundary 95: twenty-ninth existing-stem reconciliation

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

## Boundary 96: second multi-head existing-stem C-link reuse

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

## Boundary 97: thirtieth existing-stem reconciliation

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

## Boundary 98: single-head existing-stem C-link reuse

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
remaining rather-good profile escalation and wider-corpus coverage
remain open.

## Boundary 133: `finalizeStems`

Boundary 133 carries the completed phase-2 terminal through the third and
final private call in `StemsRetriever.process`. The v104 probe restores the
retriever-owned `undefs` map and reverse-grade `systemHeads` list before
invoking `finalizeStems`, avoiding the earlier head-link probe's local-map and
x-order shortcuts. Two fresh JVMs are byte-identical.

The exact Chula system-1 census makes both finalizer substeps no-ops.
`checkHeadStems` visits 102 heads and finds zero heads with multiple
`HeadStemRelation`s, so `HeadStemsCleaner` is never constructed.
`checkNeededStems` finds exactly two stemless stem-capable heads, x32 / SIG 50
/ Inter 1389 and x31 / SIG 47 / Inter 1381. Both are `NOTEHEAD_VOID` and both
are already abnormal, so even their abnormal values do not change. The full
Java SIG remains 685 vertices / 706 edges, system stems remain 46, allocator
2385 is unchanged, and SIG, relation, inter, and linker hashes are identical
before and after the call.

Native authenticates the exhausted 102-head phase-1 carrier, all five phase-2
entries, the exact undefined/unlinked queue, its 267-vertex / 370-edge native
STEMS graph projection, all 46 system stems, every head binding, every live
HeadStem incidence, and the two already-abnormal void heads. It fails closed
on any unfinished queue, multi-stem candidate, extra stemless head, or abnormal
state drift, and otherwise returns an unchanged carrier.

The v104 fixture/runner/probe/body/semantic SHA-256 values are
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

## Boundary 134: generic `finalizeStems`

The terminal is now generic over every completed native head carrier.
`checkHeadStems` reconstructs Java's exclusion-derived partitions in stable
reverse stem-grade order, prunes the strict lowest target contribution until
at most two relations remain, and either preserves Java's physical
LEFT/TOP + RIGHT/BOTTOM canonical share or removes the remaining worst
relation. The port uses the live SIG support grade (`targetRatio = 1 + 10 *
grade`), carried stem medians, exact integer head center, relation extension
points, dy threshold 0.2, and anchor-height ratio 0.275. Each relation removal
also runs the Java-equivalent head/stem abnormal callbacks. The following
`checkNeededStems` pass marks all stemless carrier heads abnormal. Every change
is clone-first; incomplete terminals and incomplete graph/geometry evidence
fail closed.

Executable Java evidence covers real Allegretto and Zizi removals, five Zizi
canonical keeps, and a controlled Chula `checkNeededStems` false-to-true
mutation. Warmup and two fresh runs are byte-identical. Fixture/runner/probe/
init/body hashes are
`d468cb52f59687604d2204b18aa2364bde12355cb476d007ce205788033b350a`,
`ddcaa94b847de8ed50ffdb9e866717da3e888e223117d8453bf06db55ebaa247`,
`f55cc3fe1f8dc85d817ba84499e407dc759f6710cd815b0eb8007bfca02ac0b1`,
`538f75284a798d4cf96e7f4034bf5368e63f50891f58b712b517fe84f6223006`,
and `6d706ff6e8dc4fc63bb580447b91ddb114d6f0f56544b2902b50438d93d09664`.
The native focused gate additionally forces the `>2` pruning loop, exact
canonical physical geometry, and abnormal mutation on authenticated carrier
state. Generic `finalizeStems` is complete; remaining STEMS work is outside
this terminal.

## Boundary 135: production STEMS preparation

`native_stems::prepare_native_stems` is now the owned production composition
boundary immediately before the first mutating SIDES transaction. It consumes
completed live GRID, HEADERS, STEM_SEEDS, BEAMS, LEDGERS, and HEADS products,
then materializes head corners, head seeds, beam stumps and VLinkers, beam and
head reachability, beam and head builders, link plans, scheduler frontiers, and
the mutable native SIG in Java order. `materialize_native_stems_components`
exposes the same read-only component chain for wider pages whose upstream
BEAMS-group SIG is not complete. Both calls are side-effect free and return no
partial carrier on failure.

The exact Chula downstream carrier gate now obtains its predecessor through
`prepare_native_stems`, rather than reconstructing this sequence inside test
code. The focused gate passes 1/1, the full sibling suite passes 14/14, and
workspace all-target/all-feature Clippy, formatting, and diff checks are clean.
This boundary removes a test-only composition seam; Boundary 136 removes the
page-wide first-STEMS snapshot from the operational path. The sparse 16-row
selected-base Java identity bridge and later branch/corpus gaps remain.

## Boundary 136: native STEMS glyph identity

`NativeStemsModeledGlyphRegistry` now derives one owned, system-scoped glyph
identity domain from the exact modeled registration prefix. Its stable identity
is canonical ordinal plus one, and every lookup requires full bounds, weight,
and RunTable equality. It imports neither Java's sheet-wide `GlyphIndex`, its
allocator/union watermark, nor the 592 opaque fingerprint-only entries.

The Chula carrier switches to this registry after the initial legacy bootstrap
transaction. Transactions 2-32, all seven STUMPS transactions, every measured
head-origin C-link, and every existing-stem reconciliation now carry native
glyph identity. Existing stems authenticate through their carried StemInter and
native content; Java glyph numbers remain oracle descriptions, not join keys.
The focused carrier gate passes 1/1 in 17.34s, the full sibling suite passes
14/14 in 153.26s, strict workspace all-target/all-feature Clippy passes in
23.72s, and formatting and diff checks are clean.

This does not yet replace transaction 1's fixture-backed bootstrap, the
sparse selected-base Java InterIndex bridge, or the reconstructed Allegretto
predecessor. The legacy first-STEMS fixture/API remains for those isolated
gates, but the production-shaped transaction-3-through-terminal path no longer
reads it.

## Boundary 137: native transaction-2 glyph bootstrap

The second SIDES frontier now calls
`prepare_native_stems_beam_vlink_frontier_state_from_modeled_registry` directly.
The integration gate no longer opens or parses
`stems-beam-glyph-registry-chula.txt`; native exact content selects the plan-152
glyph before any transaction-2 expected fixture is read. Transaction 2 then
continues through the existing graph-derived B13/B14 and B15-B19 chain without
changing its SIG, relation, scheduler, or closure results.

Focused 1/1 (13.84s), full sibling 14/14 (149.59s), strict workspace
all-target/all-feature Clippy (12.30s), formatting, and diff checks pass. Only
transaction 1 still begins from the legacy compact fixture state; the sparse
selected-base Java InterIndex bridge also remains.

## Boundary 138: native transaction-1 B12/B13 bootstrap

`initialize_native_stems_beam_vlink_first_frontier_state_from_modeled_registry`
now constructs the first shared-sheet B12 state from the live scheduler/plan
frontier and the owned 1,058-entry modeled registry. It derives both selected
bindings, the V-linker line state, native canonical identity, and authoritative
empty `systemStems` without importing Java's 1,650-entry GlyphIndex union,
592 opaque fingerprints, or exhaustive equality scans. The reused candidate is
native glyph 45 rather than Java glyph 294, while exact content and checker
geometry remain identical.

Transaction 1's B13 live state is then projected from the owned SIG and
persistent S cells and matches the Java all-unlinked result before B14. That
native identity survives B14 and the complete carried SIDES/STUMPS/head/finalize
path unchanged. Focused 1/1 passes; the full sibling suite passes 14/14 in
147.63s; strict workspace all-target/all-feature Clippy passes in 26.25s; and
formatting and diff checks are clean. The shared allocator and sparse 16-row
selected-base Java InterIndex identity bridge remain explicit.

## Boundary 139: native selected-beam identity

`roll_native_stems_beam_vlink_base_apply_state` now resolves each B14 beam
directly from `NativeSigSystemBindings`. Its persistent identity is the
one-based native vertex identity, its local InterIndex order is the native
vertex ordinal, and VIP is false in the owned native domain. Production no
longer accepts, stores, or scans `NativeStemsBeamBeamInterIndexBootstrapEntry`;
the carrier has no beam-identity or configured-Java-VIP collections, and the
integration gate no longer opens `stems-beam-inter-index-chula-system1.txt`.

The same 16 distinct base beams still occur across all 32 SIDES transactions,
but they are an asserted semantic result rather than an input authority.
Missing native beam bindings reject before mutation. Focused 1/1 and full
sibling 14/14 (154.47s) pass; strict workspace all-target/all-feature Clippy
passes in 27.70s; formatting and diff checks are clean. The first B14 compact
state still supplies the shared persistent-ID seed and opaque InterIndex
baseline; that is the next identity seam.

## Boundary 140: native first-B14 compact state

`initialize_native_stems_beam_vlink_base_apply_state_from_native_sig` now
constructs the first B14 graph, endpoint, beam-group, certificate, and local
InterIndex state from the owned SIG and bindings. Native SIG insertion order is
the local InterIndex domain: the initial baseline is 221 native vertices rather
than Java's opaque 639-entry sheet index, and after three carried transactions
the baseline is 223 rather than 641. The base-apply fixture no longer supplies
the operational compact state.

Every B14 mutation and downstream SIDES/STUMPS/HEADS result remains unchanged.
Focused 1/1 passes; the full sibling suite passes 14/14 in 150.25s; strict
workspace all-target/all-feature Clippy passes in 32.58s; formatting and diff
checks are clean. The shared persistent-ID counter is now the only remaining
first-B14 identity input.

## Boundary 141: native STEMS persistent identities

The first transaction now seeds its shared identity domain directly after the
1,058 modeled native glyphs. Stem identities allocate as 1,059 through 1,104
across SIDES, STUMPS, and head C-links instead of inheriting Java's 2,339
EntityIndex watermark. The initializer no longer accepts a persistent-ID
argument, and bounded continuation guards resolve existing stems by carried
`stem_identity` rather than Java Inter IDs.

The full 102-head path and generic finalizer remain unchanged; terminal sheet,
glyph-index, and inter-index counters all equal 1,104. Focused 1/1 and full
sibling 14/14 (152.10s) pass; strict workspace all-target/all-feature Clippy
passes in 29.78s; formatting and diff checks are clean. The first-STEMS glyph,
selected-beam, SIG, local InterIndex, and persistent-ID authorities are now
native end to end.

## Boundary 142: production-derived modeled-registry boundary

`NativeStemsModeledGlyphRegistry::from_head_builder_recognition` now derives a
system's visible canonical-glyph prefix from the production head-builder
chronology. It resolves the requested system, reads the final
`registry_events.modeled_count_after` boundary, and validates that boundary
against the complete canonical modeled-glyph collection. The Chula carrier no
longer supplies a separate visible-count execution input; the old count remains
only as an independent assertion in the gate.

Focused 1/1 and full sibling 14/14 (150.54s) pass; strict workspace
all-target/all-feature Clippy passes in 31.23s; formatting and diff checks are
clean. The next boundary can therefore build Allegretto's first carrier from
the same production head-builder/SIG products before carrying transactions
1-27 to its measured linked-S and competing-hook frontier.

## Boundary 143: atomic first SIDES carrier

`initialize_native_stems_beam_sides_carrier_from_modeled_registry` now owns the
first SIDES transaction end to end. From an awaiting scheduler plus native SIG,
bindings, modeled registry, and immutable STEMS products, it initializes the
B/S cell arenas, executes B12 through B19 on local state, reconciles the new
Stem against the committed SIG, validates graph and bindings, and returns the
first carrier and exact transaction trace only after every step succeeds.

The first-frontier state constructor is system-generic rather than hard-coded
to system 1. The Chula gate independently reconstructs transaction 1, proves
the returned scheduler/SIG/bindings/B/S cells and trace match, and then drives
all later transactions from the production initializer's carrier. Focused 1/1
and full sibling 14/14 (157.64s) pass; strict workspace all-target/all-feature
Clippy passes in 9.33s; formatting and diff checks are clean.

## Boundary 144: native Allegretto linked-S and hook-removal carriage

The production carrier now executes Allegretto system 1 SIDES transactions
1-28 from the production-derived modeled registry. Transaction 28 / plan 25
selects the already-attached Stem through the owned HeadStem edge and leaves
the second relation unread; the following typed checkpoint removes the
competing BeamHook from its naturally carried five-edge neighborhood and
resumes to `SidesExhausted`. The gates no longer reconstruct the predecessor,
append artificial Stem vertices, tombstone substitute edges, or join on Java
persistent Inter IDs.

This carry exposed four generic assumptions and removed them: completed
`ReadyTransaction` line-delta ledger entries are one-shot evidence rather than
later known-false prefix input; callback incident rules follow the live
`BeamHook` runtime class even when source provenance is `RawBeam`; B14 rollover
accepts a graph-bound existing Stem as well as a fresh one while preserving
the separate persistent-ID and native-SIG identity domains; and B17 accepts an
existing normal Stem before recomputing callback abnormality. Focused linked-S
and hook-removal tests pass, the full sibling suite passes 14/14 in 160.24s,
strict workspace all-target/all-feature Clippy passes in 18.57s, and formatting
and diff checks are clean. Relation-check constants are still hydrated from
the existing strict fixture; wider-system carriage remains next.

## Boundary 145: production-derived BeamStem relation parameters

`NativeStemsBeamRelationParameters::from_native_products` now derives the
system interline and main Stem thickness from the native link-plan/V-linker
products, takes the authenticated frontier profile, and supplies the ported
Java `BeamStemRelation` constants (x-in 0.5, x-out 0.2, y 4.0, weights 1/4,
intrinsic ratio 1, minimum grade 0.1). `NativeStemsBeamSidesContext` no longer
accepts caller-provided relation parameters, so neither the first transaction
nor later SIDES/STUMPS transactions can be driven by a fixture value.

The Chula and Allegretto gates independently compare the derived product with
their frozen Java context rows before executing; their full carried results
remain unchanged. Focused Chula, linked-S, and hook-removal gates pass; the
full sibling suite passes 14/14 in 159.65s; strict workspace
all-target/all-feature Clippy passes in 24.34s; formatting and diff checks are
clean. Wider system carriage is now the next input/branch boundary.

## Boundary 146: production-owned STEMS entry edit state

`NativeStemsBeamSheetEditState::at_stems_entry` now owns the Java/native entry
invariant that the earlier graph-building stages have already marked the sheet
stub, book, and book dirty. The atomic first-carrier initializer no longer
accepts this mutable state from callers. Chula and Allegretto independently
match the former strict B14 entry state and retain identical transaction and
terminal results.

Focused Chula and Allegretto hook gates pass; the full sibling suite passes
14/14 in 157.49s; strict workspace all-target/all-feature Clippy passes in
22.85s; formatting and diff checks are clean. The first carrier now has no
fixture-derived execution input; wider system carriage remains next.

## Boundary 147: production-owned checker and first-system SIDES start

`prepare_native_stems` now owns the page-wide StemChecker context instead of
leaving it to test hydration. Live GRID and STEM_SEEDS products supply
`NO_STAFF`, interline, maximum Stem thickness, the ties-to-even
`0.15 * interline` belt margin, sheet skew, Java's exact `0.8 * 0.1`
minimum-grade product, and the `0.4` artificial-Stem grade.

`NativeStemsPreparedRecognition::initialize_first_system_sides` resolves the
first scheduler system's plans, builders, stumps, VLinkers, reachability, head
corners, SIG, bindings, and modeled-glyph registry as one coherent production
join. It returns the registry, carrier, and committed first B12-B19 transaction
together, preventing caller-supplied checker values or cross-system product
mixing.

The new third-page proof starts Batuque system 1 at plan 98. Native reuses
active glyph 265, creates the checked Stem at grade bits
`0x3fe91480f4111904`, applies support grade bits `0x3feefb1fb84ea5fd`,
links the one Java-measured sibling, commits two transaction-wide B-cell
writes (base plus sibling), inserts three HeadStem relations/S-cell writes,
performs the idempotent outer write, and reaches plan 111. Frozen Java rows are
assertions only and do not feed execution. The focused gate passes 1/1 in
4.47s; the full sibling suite passes 15/15 in 166.67s; strict workspace
all-target/all-feature Clippy passes in 25.18s; formatting and diff checks are
clean.

This is deliberately a first-system initializer. Later systems must inherit
the allocator and modeled registry committed by every preceding system's full
transaction chronology; an isolated system-2 reconstruction is rejected
rather than guessed. That cross-system carriage is the next boundary.

## Boundary 148: production first-system SIDES drive

`NativeStemsPreparedRecognition::drive_first_system_sides` now owns the
system-1 transaction loop rather than leaving it in the sibling test. It starts
from Boundary 147's production checker, modeled registry, SIG, bindings, and
first committed transaction, then repeatedly calls the generic modeled-registry
advance until the scheduler reports the true `SidesExhausted` terminal.

The immutable system-local builder count is a strict progress bound. A
competing-hook checkpoint, an unexpected STUMPS completion, an empty builder
set, or failure to reach the SIDES terminal within that bound rejects the whole
drive; no partial carrier is returned as a completed system.

The Batuque production proof executes 33 transactions and finishes with
222 SIG vertices, 263 edges, 32 Stem bindings, 51 of 93 linked B cells, 71 of
186 linked S cells, and 24 beams retained for STUMPS. The retained and final
local worklists are identical, and every B/S cell remains open at the SIDES
terminal. Boundary 147's first transaction remains checked against its exact
Java rows; this new terminal vector proves the production driver over the
already graded generic components and is not presented as a new full-chain Java
snapshot.

The focused gate passes 1/1 in 3.76s; the full sibling suite passes 15/15 in
159.88s; strict workspace all-target/all-feature Clippy passes in 23.77s;
formatting and diff checks are clean. The helper remains deliberately
first-system-only. Cross-system allocator, registry, and persistent carrier
chronology was therefore the next boundary.

## Boundary 149: exact cross-system registry and allocator handoff

`NativeStemsModeledGlyphRegistry::carry_into_next_system` now joins the full
system-1 modeled prefix to every exact canonical entry learned by its completed
transaction state, then replays the next head-builder system's constructor
registrations in production order using complete bounds/weight/RunTable
equality. Selected glyphs consequently resolve by structural content rather
than by the precomputed modeled ordinal, which is not a valid page identity
after interleaved StemInter allocations.

The handoff refuses isolated-frontier state, nonconsecutive systems,
identity/content alias collisions, weak-only originals whose Java liveness is
unknown, and any union count not covered by exact carried contents. It advances
all three views of the one shared persistent allocator only when the next
constructor registration is structurally absent. Caller-owned state remains
unchanged on every failure.

For Batuque, system 1 begins and ends with 1,058 structural glyphs while 32
created Stem inters advance the persistent allocator from 1,058 to 1,090.
Replaying system 2's 1,125 constructor events yields 1,470 structural glyphs
and allocator 1,502. An isolated system-2 prefix also contains 1,470 glyphs but
incorrectly assigns allocator 1,470; the new gate pins that these registries are
not equal and preserves the 32 inter-ID gap. Weak-liveness and deliberately
incomplete-union variants both reject atomically.

The focused gate passes 1/1 in 3.78s; the full sibling suite passes 15/15 in
157.08s; strict workspace all-target/all-feature Clippy passes in 8.66s;
formatting and diff checks are clean. This boundary carries page registry and
allocator authority only; constructing the system-2 SIG/bindings/cells and
first serial SIDES carrier was therefore next.

## Boundary 150: first shared-sheet serial SIDES carrier

`NativeStemsPreparedRecognition::initialize_second_system_sides` now drives
system 1 to `SidesExhausted`, carries Boundary 149's exact registry, allocator,
and page edit state, selects system 2's production SIG/bindings/products, and
executes the first B12-B19 transaction atomically. The serial initializer uses
an explicit `SharedSheetSerial` transaction state with empty system-local
`systemStems` and fresh B/S cells; it never seeds a later system from the
registry length.

Batuque enters system 2 at registry 1,470 / allocator 1,502. Plan 514,
builder 105, profile 4 returns `CreatedChecked` for stem identity 0, advancing
the allocator to 1,503. The committed carrier retains union size 1,470 and one
known canonical/system stem, with a 240-vertex / 199-edge SIG, one stem
binding, 117 B cells, and 244 S cells.

This boundary composes previously graded native authorities and claims no new
Java full-chain snapshot. The focused gate passes 1/1 in 3.78s; the full
sibling suite passes 15/15 in 158.54s; strict workspace
all-target/all-feature Clippy passes in 25.68s; formatting and diff checks are
clean. Driving the remaining system-2 SIDES worklist was therefore next.

## Boundary 151: complete Batuque system-2 SIDES drive

The bounded SIDES driver now accepts any production `NativeStemsSystemSidesStart`.
Boundary 150's serial first transaction therefore continues through the exact
same clone-and-commit loop, builder-count progress bound, and typed terminal
checks as system 1. Hook-removal, STUMPS-completed, malformed, and exhausted-bound
states still fail closed.

Batuque system 2 executes 40 `SharedSheetSerial` transactions and reaches true
`SidesExhausted`. Its allocator advances from 1,502 to 1,542 for 40 system stems;
the final SIG has 279 vertices / 349 edges and 40 stem bindings. Linked cells are
64/117 B and 89/244 S, every cell remains open, and the 33 retained STUMPS items
exactly equal the final local worklist.

The focused gate passes 1/1 in 4.15s; the full sibling suite passes 15/15 in
160.94s; strict workspace all-target/all-feature Clippy passes in 24.43s;
formatting and diff checks are clean. Carrying this terminal into system 3 and
widening later-system STUMPS were therefore next.

## Boundary 152: complete three-system Batuque SIDES page

`NativeStemsPreparedRecognition::drive_all_system_sides` now carries each
completed registry, allocator, and edit state into the next consecutive system
and returns only after all systems reach true `SidesExhausted`. System 3 exposed
two generic branches: B17 must iterate the accepted relation-map subset rather
than require every head target to link, and a multi-glyph candidate needs an
owned exhaustive page-registry equality scan before registration.

Batuque completes 33 + 40 + 28 transactions. System 3 enters at registry 1,819
/ allocator 1,891, registers absent compound glyph 1,915, and finishes at union
1,820 / allocator 1,920 with 28 stems. Its terminal has SIG 244/257, B 50/101,
S 63/224, and 25 retained STUMPS entries equal to the final worklist. The
relation-subset gate pins three links from four targets; weak-only registry
liveness rejects without mutation.

The focused gate passes 1/1 in 4.07s; the full sibling suite passes 15/15 in
160.98s; strict workspace all-target/all-feature Clippy passes in 20.36s;
formatting and diff checks are clean. Wider-system STUMPS carriage is next.

## Boundary 153: production Batuque system-1 STUMPS completion

`NativeStemsPreparedRecognition::drive_first_system_stumps` now takes the
production-prepared system-1 SIDES terminal through its complete retained
STUMPS worklist. The eight atomic B12-B17/resume transactions finish at
allocator 1,098, 40 known/bound Stems, SIG 230/297, B 67/93, and S 89/186;
the scheduler reaches `Completed` with the same 24 sources it retained at the
SIDES terminal.

The first Batuque stump rollover reuses an existing StemInter. Its valid B14
shape has zero appended InterIndex entries, zero appended SIG vertices, and
one appended BeamStem edge, unlike Chula's fresh-stem 1/1/1 shape. The native
rollover validator now authenticates either exact shape against owned
transaction state, bindings, and the live SIG. It permits later B16/B17 edges
after the recorded base edge and does not confuse later callback abnormality
changes with B14 corruption. The focused gate corrupts the reused-stem edge
and proves the entire bounded STUMPS batch remains unchanged on rejection.

The run also proved that a B16 sibling write may name a B-linker owned by a
`StemBuilder` item without a standalone V-linker constructor. B19 now checks
sibling references against that builder catalogue, while preserving strict
primary-linker, duplicate, and unknown-reference refusals.

Focused Batuque passes 1/1 in 3.95s; the full sibling suite passes 15/15 in
151.29s; strict workspace all-target/all-feature Clippy passes in 19.98s;
formatting and diff checks are clean. Systems 2-3 STUMPS and the shared-page
registry/allocator handoff after each completed STUMPS pass are next.

## Boundary 154: complete three-system Batuque STUMPS page

`NativeStemsPreparedRecognition::drive_all_system_stumps` now runs each
system's SIDES and STUMPS passes before constructing the next system. The
next-system registry is derived from the preceding post-STUMPS transaction
state, so shared page IDs cannot silently fork from the earlier SIDES-only
terminal. The page vector is published only after all systems complete.

The production drive executes 42 STUMPS transactions: 8, 14, and 20. System 1
finishes at allocator 1,098 / 40 stems / SIG 230/297. System 2 starts from
registry allocator 1,510 and finishes at 1,564 / 54 stems / SIG 293/406.
System 3 starts from registry allocator 1,913 and finishes at 1,962 / 48 stems
/ SIG 264/339. Registry lengths remain 1,058, 1,470, and 1,819; systems 2-3
retain `SharedSheetSerial`; every retained worklist equals its completed local
worklist.

Focused Batuque passes 1/1 in 4.20s; the full sibling suite passes 15/15 in
154.38s; strict workspace all-target/all-feature Clippy passes in 22.22s;
formatting and diff checks are clean. The next boundary transfers these three
post-STUMPS carriers into page-wide head linking.

## Boundary 155: enter page-wide Batuque head linking

`NativeStemsPreparedRecognition::begin_all_system_head_linking_phase1` now
transfers every completed Batuque STUMPS carrier into the generic phase-1 head
driver. The page result is atomic: it is published only after all three
systems validate their live SIG, bindings, persistent S cells, reverse-grade
head queue, and first actionable frontier.

The generalized entry driver closes the prelinked prefix from native graph
state before returning a C-link frontier. Systems 1-3 carry queues of
93/122/112 heads, close prefixes of 7/79/48 heads, and stop at,
respectively, staff/head/SIG/x `(1,30,84,56)`, `(1,57,115,108)`, and
`(1,57,105,110)`. Each frontier is the single LEFT/BOTTOM choice with no
unlinked or undefined head. The first/last prefix heads are system 1
`(1,28,82,4)` / `(1,24,78,86)`, system 2 `(1,18,76,13)` /
`(1,38,96,84)`, and system 3 `(1,61,108,6)` / `(0,15,14,32)`.
Prelinked closures use the live SIG, stem bindings, and S cells and are
recorded separately from any later C-link mutation. Dual-corner ambiguity
and the unported rather-good retry/no-link branches still fail closed.

Focused Batuque passes 1/1 in 4.30s; the full sibling suite passes 15/15 in
152.96s; strict workspace all-target/all-feature Clippy passes in 23.13s;
formatting and diff checks are clean. The next boundary consumes these three
first C-link frontiers and carries the remaining page-wide head queues.

## Boundary 156: execute the first page-wide head outcomes

`advance_all_system_first_head_frontiers` now owns the accepted STEM_SEEDS
free glyphs retained by `prepare_native_stems` and executes each system's
first head frontier atomically. Queue ordinals, selected corners, and bounded
builder extents come from native state rather than order-specific Java IDs or
caller-supplied expansion indices.

Batuque systems 1 and 3 consume x56 and x110 LEFT/BOTTOM and each append one
Stem vertex plus one HeadStem edge. Their carriers advance to indices 8 and
49 at SIG 231/298 and 265/340. System 2 exercises the first wider-corpus
normal `CLinker.link` rejection: its 18-pixel start item cannot reach Java's
37-pixel hard tail. The generic `linkSides` loop tries eligible profiles and
sides, then closes both S cells, adds the head to the phase-2 queue, and
advances to index 80 without changing SIG 293/406. The hard target is now
correctly measured from the selected corner reference point in both created-
and existing-stem paths.

Focused Batuque passes 1/1 in 4.48s; the full sibling suite passes 15/15 in
154.01s; strict workspace all-target/all-feature Clippy passes in 19.10s;
formatting and diff checks are clean. Remaining page queues, wider expansion
and reuse, and phase-2 append retries remain next.

## Boundary 157: carry every page system to its next head frontier

`continue_all_system_heads_to_next_frontier` now owns the mutation-free
continuation loop after Boundary 156's mixed outcomes. It repeatedly applies
native graph-derived prelinked closures or defined false continuations and
stops only at the next actionable C-link frontier or a true phase-1 terminal.
The page result remains atomic and preserves every carried retry head.

Batuque system 1 crosses 18 continuations to index 25 before staff/head/SIG/x
`(1,34,88,76)`. System 2 stops immediately at index 80 before
`(1,63,121,109)` while retaining its one phase-2 retry head. System 3 stops
at index 49 before `(0,47,46,111)`. All three next frontiers are
LEFT/BOTTOM; no SIG, registry, allocator, or stem mutation occurs in this
continuation boundary.

Focused Batuque passes 1/1 in 4.82s; the full sibling suite passes 15/15 in
163.20s; strict workspace all-target/all-feature Clippy passes in 25.56s;
formatting and diff checks are clean. Consuming these three frontiers is next.

## Boundary 158: execute the second page-wide head outcomes

The generic page transaction now consumes Boundary 157's x76, x109, and x111
frontiers while preserving carried phase-2 retry/undefined collections. The
old early-Chula assumptions that a C-link must occur before any retry head and
that the initial prefix length must equal the queue index are gone; shadow
mutation still commits only on a complete typed outcome.

Systems 1 and 3 create one Stem vertex and HeadStem edge at x76 and x111,
advancing to indices 26 and 50 with SIG 232/299 and 266/341. System 2's x109
candidate takes the generic rejected-link closure, advances to index 81, and
appends a second phase-2 retry head while SIG 293/406 remains unchanged. Its
two earlier/current retry heads and both closed-cell writes are retained in
order. The 18/1/1 prior continuation traces remain attached to the results.

Focused Batuque passes 1/1 in 5.09s; the full sibling suite passes 15/15 in
162.53s; strict workspace all-target/all-feature Clippy passes in 25.76s;
formatting and diff checks are clean. Continuing the mixed carriers to their
next action frontiers is next.

## Boundary 159: complete three-system Batuque head phase 1

`drive_all_system_head_linking_phase1` now alternates native continuation and
action outcomes until every reverse-grade queue reaches its true terminal. A
per-system `2 * head_count` event bound prevents silent looping, and any
unsupported branch rejects the entire page shadow.

The generic C-link core now reuses an already attached native Stem by adding
only the HeadStem edge, linking the selected S cell, and closing sibling
heads; vertex, allocator, registry, and system-Stem state remain unchanged.
Dual TOP/BOTTOM reachability now follows Java exactly: one shared non-null
stump records an undefined side, while differing/missing stumps choose BOTTOM
on LEFT or TOP on RIGHT.

Batuque terminals are:

- system 1: 93/93 heads, prefix 7, 89 events (85 continuations, 2 creates,
  2 reuses), no retry/undefined heads, SIG 232/301, 42 stems, allocator 1,100;
- system 2: 122/122 heads, prefix 79, 44 events (42 continuations, 2 no-link
  outcomes), 2 retry heads, SIG 293/406, 54 stems, allocator 1,564;
- system 3: 112/112 heads, prefix 48, 69 events (63 continuations, 4 creates,
  1 reuse, 1 direct no-link), 2 retry heads/2 undefined sides, SIG 268/344,
  52 stems, allocator 1,966.

All carriers finish consumed with `phase_two_index=0`. Focused Batuque passes
1/1 in 5.11s; the full sibling suite passes 15/15 in 156.59s; strict workspace
all-target/all-feature Clippy passes in 25.06s; formatting and diff checks are
clean. Page-wide phase-2 append retry is next.

## Boundary 160: complete Batuque head phase 2 page-wide

`drive_all_system_head_linking_phase2` atomically composes the complete
phase-1 page drive with each system's ordered append-retry queue. Queue
authentication is now native and corpus-independent: cursor, unique head and
undefined-side identities, completed-head membership, and closed direct
no-link sides are checked before a local shadow advances. A real Java
`reuseStem` append still rejects the page fail-closed rather than approximating
its mutation.

System 1 has no retries. System 2 consumes x108/SIG115 and x109/SIG121 with
BottomOnly/Neither and BottomOnly/TopOnly decisions. Both return false; Java's
final close loop re-writes their already-closed LEFT/RIGHT cells, so the native
event stream records four ordered writes and zero value changes. System 3
consumes x107/SIG47 and x108/SIG2. The first attempts standard LEFT/BOTTOM then
returns undefined at the RIGHT shared stump; the second returns undefined at
LEFT immediately. Neither writes a cell. Page terminals preserve SIG/stem
counts `232/301/42`, `293/406/54`, and `268/344/52`, with phase-two indices
equal to queue lengths `0/2/2`.

The new Java page oracle reconstructs Batuque from actual HEADS, executes real
SIDES/STUMPS and both head phases in foreground system order, and emits all
four retry rows. Warmup plus two fresh passes are byte-identical. Fixture,
runner, probe, init, and body SHA-256 are
`41992cf6702bc27b918733e6a1a097c22b729c6dfc7fe332e8603c5e6a02983a`,
`b0e79187886052aa20ac15421da2eb5169d541b305ef0f04460dfc05add094d6`,
`7b467c57b65e57aa052296164129ae8c016d82756c9f804d8e1072747b0a76b2`,
`1defbc545668eb711395283bc0d8f9216b7402ad3b0f2f64f93812ac739c495e`,
and `3d30e22eca5ee67647519fed576083a66ed987bd8803376e72c5462f5758d021`.
Focused Batuque passes 1/1 in 5.51s; the full sibling suite passes 15/15 in
152.69s; strict workspace all-target/all-feature Clippy passes in 20.10s;
formatting and diff checks are clean. Page-wide `finalizeStems` is next.

## Boundary 161: finalize Batuque STEMS page-wide

`finalize_all_system_stems` composes the complete two-phase page carrier with
generic `finalize_native_stems`, using per-system shadows and withholding the
page result until every finalizer succeeds. The new Java page probe executes
real Batuque SIDES, STUMPS, both head phases, and private `finalizeStems` in
foreground system order.

System 1 checks 93 heads with no abnormal result. System 2 checks 122 and
preserves x108/SIG115 plus x109/SIG121 as no-stem abnormal heads. System 3
checks 112 and preserves x107/SIG47 plus x108/SIG2, along with their carried
RIGHT/LEFT undefined sides. Multiple-stem sets, HeadStem removals, abnormal
value changes, graph changes, allocator changes, and system-Stem changes are
all empty. Terminal graph/stem counts remain `232/301/42`, `293/406/54`, and
`268/344/52`.

Warmup plus two fresh Java passes are byte-identical. Fixture, runner, probe,
init, and body SHA-256 are
`ab6377a2b82cc838633b8c0d79732ddd755a68f11a8b7e40dd39baee7d6278d2`,
`7e8b8c557d1d321329c72e62cdd932e0faa304591e14b958171ff7a961342ea1`,
`9b5e9dbefbf400887f49feba934c573d851c67e65b3e43bfaabc86d6f2c36714`,
`e0ff89792bf75286317ef011e079f338696d29cc14918f4a3018307ba4ed9548`,
and `e51e06eb798e3ab6ccaa32ea5db5b88f6285b667fb8162e1777a0faf6c28a3a1`.
Focused Batuque passes 1/1 in 14.17s; the full sibling suite passes 15/15 in
156.66s; strict workspace Clippy passes in 19.88s; formatting and diff checks
are clean. The next boundary is the transactional
`recognize_native_stems` entry point, followed by schema-1 publication and
wider-corpus branch coverage.

## Boundary 162: transactional `recognize_native_stems`

The complete native stage now has a fail-closed production entry point.
`recognize_native_stems` consumes live completed GRID, HEADERS, STEM_SEEDS,
BEAMS, LEDGERS, and HEADS products, prepares all immutable construction state
and native SIGs, drives page-wide SIDES/STUMPS, both head phases, and generic
`finalizeStems`, and exposes `NativeStemsRecognition` only after every system
has finalized. Its owned result retains the construction products and each
system's terminal SIG/registry plus phase-1, retry, and finalization traces.

The Batuque integration gate calls both the independently stepped page path
and the new one-call entry point and requires their full component and system
products to compare equal. Boundary 161's fresh Java page-finalization fixture
remains the external oracle; no new transformed fixture or Java identity is
introduced. Focused Batuque passes 1/1 in 13.80s; the full sibling suite passes
15/15 in 142.75s; strict workspace Clippy passes in 20.01s; formatting and diff
checks are green. Ordinary and stream schema-1 STEMS
publication is next.

## Boundary 163: schema-1 STEMS publication

`stems_json` extends the existing hand-written schema-1 document without
changing any earlier product. `-step STEMS -json` now composes the seven native
stages through transactional `recognize_native_stems`, retains the complete
HEADS payload, and adds exactly one stage-owned `stems` object. Each system
publishes terminal summaries, all accepted Stem medians/bounds/thicknesses/
grades, HeadStem payloads, multiple/no-stem/abnormal head sets, and carried
undefined sides. Native stem identities and SIG ordinals are explicitly named
and system-scoped; no value is presented as a Java `InterIndex` ID.

Batuque publishes 148 final Stems, 323 live HeadStem relations, 327 checked
heads, and four abnormal no-stem heads across three systems. The ordinary JSON
document is byte-identical to the immutable STEMS snapshot emitted between
`stage_started` and `stage_completed`; stage markers remain ordered GRID,
HEADERS, STEM_SEEDS, BEAMS, LEDGERS, HEADS, STEMS. The full CLI suite passes;
its live ordinary/stream contract takes 17.63s. All 11 report tests pass;
strict workspace Clippy passes in 12.06s; formatting and diff checks are green. Wider-corpus
SIDES/STUMPS/head branch coverage and exact remote CI remain next.

## Next implementation slices

Commit each slice separately after the full verification block above.

1. Compose Boundary 161's atomic page finalization into transactional
   `recognize_native_stems`, preserving the shared page identity state and
   exposing no partial stage on any system failure. Then publish the completed
   native STEMS product through the existing schema-1 ordinary and stream JSON
   contracts.
2. Continue Bach after Boundary 261. The production page driver now dispatches
   system-3 queue-3 x96/SIG166 and queue-5 x146/SIG56 measured RIGHT/BOTTOM
   reuse-stem appends; the full Bach CLI reaches the next uninstrumented
   system-3 queue-7 reuse-stem branch (x28/SIG50). Instrument that Java branch,
   add its strict fixture,
   then continue the transactional page driver. The native generic `finalizeStems` acceptance gate remains
   recorded for the exhausted system-2 carrier, with Java's matching census:
   one multiple-stem head before cleanup, zero after, 12 no-stem/abnormal
   heads, one removed HeadStem relation, and zero abnormal changes. Continue the
   remaining Bach systems and widen bounded STUMPS/competing-hook evidence
   before claiming full-corpus `recognize_native_stems` coverage.
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

## Boundary 164: Chula system-1 wider reuse composition

The page-wide phase-1 driver now routes the three already-authenticated wider
existing-stem expansions at Chula system-1 orders 67, 70, and 73 through their
owned native transactions before resuming the ordinary generic loop. The
production adapter consumes only the reduced accepted free-seed glyph state
that `NativeStemsComponentRecognition` owns; it does not hydrate a Java glyph
index or consult a result fixture. Single-head reuse remains on the generic
path (order 72 passes without a special dispatch).

The real Chula page now completes system 1 and fails closed at the next genuine
gap: system 2 queue 54, x46/SIG94, a wider start/head/chunk expansion. The
focused page-composition regression proves the old order-67, order-70, and
order-73 failures are gone and that no partially completed page is returned.
The richer expansion diagnostic records system, queue, head, selected corner,
builder/profile, and item composition. No Java oracle or frozen fixture changed.
Next: instrument the system-2 queue-54 Java transaction and replace the bounded
routing with a generic existing-stem multi-head dispatcher as wider cases make
the invariant explicit.

## Boundary 165: Chula system-2 wider reuse

A fresh Java system-2 replay (warmup plus two byte-identical passes) measures
queue 54 x46/SIG94 as LEFT/BottomOnly. Its start, crossed x45 linker, and chunk
resolve to existing Java glyph 376 / StemInter 2285. Java allocates nothing,
adds HeadStem edges for x46 and x45, closes stem-sharing x45 and x47, and
advances to x31/SIG36. The bounded four-row fixture is SHA-256
`421c6b99552071e39e6b72a3963f5ac46daf41b3bd0c9a560ea45251868f5c09`;
both complete pass outputs are SHA-256
`6e42c2cd20ceffca1d90359d3bc81d7e60780f3cbe29b22b56d1c8e7a9b8b353`.

Production resolves that candidate through native content to Stem identity 45
/ native glyph 127, appends the two relations, and performs the same four
closure changes without Java IDs or allocation. Chula systems 1 and 2 now both
finish phase 1; the page rejects next at system 3 queue 109 x41/SIG122, whose
start linker has no stump and is followed by one chunk. The focused live-page
gate pins the new atomic stopping point. Next: measure and port that stump-less
frontier, then extract a generic wider-reuse dispatcher from the accumulated
system-1/system-2 cases.

## Boundary 166: generic stump-less rejection and complete Chula STEMS

The system-3 order-109 Java predecessor was replayed through real production
SIDES/STUMPS and heads 0-108 without snapshots. x41/SIG122 first tries
LEFT/TOP: its single chunk shifts the line, but the final HeadStem projection
rejects and returns `lastIndex=-1`. Java continues to RIGHT/BOTTOM, where
active glyph 425 resolves to existing StemInter 2296; one x41 HeadStem edge is
added, the RIGHT S cell links, and head order 110 x18/SIG0 is next. No
allocator, vertex, registry, or system-stem change occurs. The three
byte-identical replay hashes are
`d07bfcc6915fae64fb8481be8f6b3aaccc6e768a349e9af8b3ea0c46d90ae142`,
`6cf71daa00322c0e6d20cd745d7d0cf68b2bc7b196a8ab1c3c507bf361ad5c4b`, and
`6260e8b63601ac71e00a253ce2c803f8373a293e183e78983209742c2dd96788`;
the frozen fixture SHA-256 is
`930a9f936f4c5f1eb535e3256e815f44a08f9b96b5aef1fcc52c0c9b28300a15`.

Production now handles stump-less single-chunk starts generically. A rejected
final relation is an ordinary non-mutating corner failure, so the same
`linkSides` loop proceeds to the next horizontal side and reuses the carried
stem through native content identity. The live Chula gate completes every
phase-1 head in all three systems, phase 2, generic `finalizeStems`, and the
transactional `recognize_native_stems` entry point. Batuque and the complete
16-test sibling suite remain green. Continue at the next wider-corpus
fail-closed branch; Chula no longer supplies the current HEADS blocker.

## Boundary 167: production Allegretto hook removal

`NativeStemsPreparedRecognition::drive_system_sides_start` now treats
`AwaitingHookRemovalTransaction` as an executable typed frontier. It invokes
`remove_native_stems_beam_competing_hook_and_resume`, records the atomic result,
and continues from the returned scheduler state; it no longer rejects a page
merely because the already-ported hook branch is reached. Both SIDES and
SIDES+STUMPS result authorities retain their ordered removal transactions.

The live production Allegretto gate proves system 1 executes 28 SIDES
transactions, removes the exact Java-pinned BeamHook SIG24 competing with Beam
SIG25 (five incident edges and one member from a three-member group), and
reaches `SidesExhausted`. Serial carriage completes systems 1-3 through STUMPS
with hook-removal counts `[1,0,2]`; every removal changes exactly one live
vertex, its incident-edge count, and one group member. The system-1 fixture is
unchanged at SHA-256
`d4c5decf03eaab893c79b2cb7ebd0378f13ac019acc007a38718105c75eacc71`.
The two system-3 removals extend native structural coverage but are not claimed
as separately frozen Java result rows.

Focused Allegretto passes 1/1. The full sibling suite passes 17/17 in 147.41s;
strict all-targets/all-features workspace Clippy, formatting, and diff checks
pass. The production CLI now fails closed later, at Allegretto system 1 HEADS
queue 65 x77/SIG14 LEFT/TOP. Its builder is start stump + chunk + crossed x75
head, with one carried undefined side. Measure and generalize that multi-item
C-link expansion next.

## Boundary 168: Allegretto multi-item existing-stem C-link

A snapshot-minimized Java replay now executes Allegretto system 1 from the
real hook-removal and complete-STUMPS predecessor through heads 0-64 without
snapshots, then measures queue 65 twice byte-identically. x77/SIG14 selects
LEFT/TOP. Its builder walks active seed glyph 282, chunk glyph 2034, and the
crossed x75 LEFT/TOP linker; the composite resolves to existing StemInter
2236. Java allocates no glyph, Inter, vertex, or system stem. It appends the
x77 and x75 HeadStem relations with exact grade/dx bits
`3fe9cd7b1bef63de`/`3fb1a913e59fdb6e` and
`3fe92d2153d3bb34`/`3fb356694b791249`, links both LEFT cells, and closes x75
and the previously linked x76. The 12-line / 12,565-byte fixture SHA-256 is
`0bccd92c0a4305704c5903984ccf9734823bf4879b5aa6f2621595700fa6507d`;
runner, transformed probe, body, and semantic-pass hashes are
`be1f28c0528721e23ba24e1b8107f5069310d47a1a537945052d2a536a260e74`,
`6ae5fe6eddaf4d802973c191c8d945eac8046a1d398499de79c5eb183a489092`,
`0ea1b9deaa33a644ba432a26bfe6a84391cdee5115bacaa070b71287bb1a3a13`, and
`d8a600e1dff9c81fa9ebc4eadd5fc9119548343070cdcf0225ec1dbc798b3b37`.

Native reuses the existing generic expansion walk, but now authenticates the
carried undefined-side and phase-2 unlinked-head lists independently. That is
required here: x84 contributes the sole carried undefined LEFT side, while
the unlinked queue is x86 then x84. The live regression reconstructs queue 65,
pins both relation payloads, proves zero vertex/stem allocation and four x75/x76
closure value changes, and preserves both carried lists. Focused 1/1 and the
full sibling suite 18/18 (143.22s) pass. Production now advances beyond queue
65 and fails closed next at queue 79 x82/SIG89 LEFT/TOP, whose builder contains
a start stump, crossed x80 RIGHT/TOP head stump, and chunk. Measure that wider
cross-side expansion next.

## Boundary 169: Allegretto crossed-side created-stem C-link

The strict Allegretto replay now carries the real system-1 predecessor through
heads 0-78 without snapshots and measures queue 79 twice byte-identically.
x82/SIG89 selects LEFT/TOP. Java walks active seed glyph 297, accepts the
crossed x80 RIGHT/TOP head, and then stops normally when the trailing chunk
fails `maxLineGlyphDx`. The initial x82 relation is the relation retained from
the pre-expansion `canLink` check; x80 is projected from the evolved line.
Their exact grade/dx bits are `3feffffffffffe18`/`bd28618618618618` and
`3fe872c0dd16cd02`/`3fb542c107f91e7a`.

The selected canonical has no existing StemInter, so Java allocates checked
StemInter 2240 with bounds `2299:692:3:47`, grade bits
`3fe42c27698e7250`, and mean-thickness bits `3ff51b3bea3677d4`;
SIG 637/562 becomes 638/564 and system stems 39 becomes 40. Native now supports
that created-stem disposition inside the generic multi-head transaction,
including mixed carried undefined sides (`x84 LEFT`, `x80 RIGHT`, `x57 LEFT`,
`x58 LEFT`), raw start/head/chunk ordering, the bounded rejected-chunk stop,
and pre-expansion start-relation timing. It creates dense Stem 39 / native
persistent Inter 1022 (not Java's unrelated ID 2240), appends the x82 LEFT and
x80 RIGHT HeadStem edges, closes x80 LEFT
then RIGHT, and reaches queue 80 without disturbing the carried phase-2 list.

The 12-line / 12,840-byte fixture SHA-256 is
`63327c13e4ebba1873fb73d5507b5a34369027ca8c6a4abb60f377cebeee69ee`;
runner, transformed probe, body, and semantic-pass hashes are
`bcbf729291881676df19a79e74a0fb4f2266d09f5c5de0565dedf4420759fd95`,
`b22c21f1b9410ec66aa5445f8aa2f9aa4e4149c02b733abe03617ec6be05c032`,
`e9802845ac23e54fb14617dc21a63ac1a5be0d5b64e998bf0b8cd0ff1a288d62`, and
`ffb9b95199d62bce49a95e044b93f09fd0562b74b8917b069e26da0d793ca452`.
Focused 1/1, full sibling 18/18 (146.26s), strict all-targets/all-features
workspace Clippy, formatting, and diff checks pass. The production CLI now
completes Allegretto system 1 and fails closed next in system 2 at queue 89
x52/SIG43 RIGHT/TOP, whose builder is start + chunk + two BeamLinkers. Measure
that beam-bearing head-origin expansion next. Broader corpus completion and
fresh remote CI remain open.

## Boundary 170: generic beam-bearing head-origin C-link

The snapshot-minimized Allegretto system-2 runner replays the real predecessor
through heads 0-88 without retaining their snapshots, then measures queue 89
twice byte-identically. x52/SIG43 selects RIGHT/TOP. Java walks a stump-less
start, active chunk glyph 2206, RawBeam 32, and RawBeam 31. The last sibling
BeamLinker returns from inside `CLinker.expand`, so Java deliberately retains
the initial x52 HeadStem relation while using the chunk-shifted stem line for
both BeamStem checks and checked-stem creation. The exact HeadStem grade/dx
bits are `3feffffffffffe92`/`bd22492492492492`; the RawBeam 32 and 31
BeamStem grade/dx bits are `3fef678964cad0c6`/`3f8b8adbbfa33cf4` and
`3fef5192bafb730a`/`3f8f57759e0eaaab`.

Java creates StemInter 2386 from glyph 2206, appends one HeadStem plus two
BeamStem edges, links `beam:2:b:9`, `beam:1:b:9`, and x52 RIGHT, and changes
SIG 654/619 to 655/622 and system stems 55 to 56. Native now handles this as a
generic beam-bearing builder tail: phase-1 initialization materializes the
head-created B-linker anchors that Java appended after the SIDES/STUMPS carrier,
the relation evaluator converts head-to-beam TOP into the contacted BOTTOM beam
border, and BeamStem maxima follow Java's active profile (`xOut=0.15` and
`yGap=0.8` at profile 0). It creates dense Stem 55 / native persistent Inter
1483, appends the same three exact relation payloads, and commits both native B
cells plus the parent S cell atomically before queue 90.

The 7-line / 8,234-byte fixture SHA-256 is
`dcfec65a778983cc9615786fe7b9bd008677f456ad8d6f276edb3855be46e45a`;
runner, system-2 initializer, transformed probe, body, and semantic-pass hashes
are `f36f312b0bc82d8cbd4fc176133339515069743a5786eed54a37f76678795986`,
`9587d9c623beea6c7922dabf6b50cd4d315ed49f4bca28bcc430684362384035`,
`4e111715e281e58c51c724130dca44b6a9c0b3149188e3063f77abd3ab58280e`,
`218d8ecd1a889e0046a49594e675572cd2884bf3f8f3411a0d166b8c3b2cbb21`,
and `01868de57f3a8f5eb42a3496c62cb141d034b85f0fdf0d3859fe37b7337bccae`.
Focused gates pass, the full sibling suite passes 19/19 in 144.58s, and strict
all-targets/all-features workspace Clippy, formatting, and diff checks pass.
Production now advances through queue 89 and fails closed at system 2 queue 111
x51/SIG36 LEFT/BOTTOM, whose builder is a start stump followed by three sibling
HeadHalfLinkers (x48, x49, and x50). That generic multi-head expansion is next;
broader corpus completion and fresh remote CI remain open.

## Boundary 171: generic multi-head hard-tail rejection

The strict Allegretto system-2 replay now carries the real predecessor through
heads 0-110 without snapshots and measures queue 111 twice byte-identically.
x51/SIG36 selects LEFT/BOTTOM. Its built start stump and sibling x48, x49, and
x50 RIGHT/BOTTOM linkers produce four accepted transient HeadStem checks, but
the complete span still misses the hard tail target. Java returns
`lastIndex=-1` before `createStem`; active glyph 376 has no StemInter, allocator
2387 is unchanged, SIG 656/626 and system stems 57 are unchanged, and no
relation is applied. Both x51 S sides close and the queue advances to
x118/SIG57 at index 112.

The production C-link loop now authenticates a start stump followed only by
distinct sibling heads and resolves built stumps from the owned pre-builder
registry, alongside the existing free-seed path. It computes the complete item
ordinate span and converts only a proven hard-tail miss into Java's ordinary
no-link result. If that span reaches the hard target, the branch remains
explicitly fail-closed rather than pretending the successful expansion is a
rejection. This makes the rule generic and keeps all state atomic.

The frozen fixture is 7 lines / 6,947 bytes, SHA-256
`1d2dfdec360fcc575ef9b852cbb6502dc82ee6fa8b951d24914bf0ae1bb66063`.
Runner, transformed probe, emitted body, and semantic-pass hashes are
`55020b3e312fe20cea3913f4e1b8ac849235f8e84753be66ecdc969b6f4b3365`,
`dc7df0af651b851e3d1c67d382f42b961955b4763fe5e92583e6f30a407a832d`,
`a4f9ad50ee8b7b147a02147fbea94959b54e392c6567252a7be4caf6c1a6ef71`,
and `b81c303a6863f2c88dcc93ef442bc526937e708e136e508d4c8021dbb7af4e36`.
Strict predecessor pins are Boundary 170 runner
`f36f312b0bc82d8cbd4fc176133339515069743a5786eed54a37f76678795986`
and fixture
`dcfec65a778983cc9615786fe7b9bd008677f456ad8d6f276edb3855be46e45a`.

Focused 1/1 and full sibling 19/19 (144.57s) pass; strict all-features
workspace Clippy, formatting, and diff checks pass. Boundary 170 commit
`f87752bbb` is exact-CI green: Build & Test 32490696521 succeeded and Rust
32490696428 passed all 12 Ubuntu/macOS shards. Production completes Allegretto
system 2 and fails closed next at system 3 queue 29 x114/SIG76 RIGHT/TOP. Its
builder 456 is a built start stump plus x112/SIG68 RIGHT/TOP, and its span
reaches the hard target. Measure and port that successful two-head application
next; broader corpus completion remains open.

## Boundary 172: built-stump two-head checked-stem creation

The strict Allegretto system-3 G1 replay carries the real predecessor through
heads 0-28 without snapshots and emits queue 29 twice byte-identically.
x114/SIG76 selects RIGHT/TOP. Its built start stump and sibling x112
RIGHT/TOP both resolve to active Java glyph 397, so Java creates checked
StemInter 2398, appends x112 then x114 HeadStem relations, closes x112 LEFT
then RIGHT, and advances to queue 30 x25/SIG4. SIG 644/567 becomes 645/569 and
system stems 47 becomes 48. The relation grade/dx pairs are exactly
`3fedf3a95000fdef`/`bfa960be99fb9249` and
`3fee0ca606d66e3f`/`bfa834e47b7c3cf4`.

Production now resolves `Built` head stumps from the owned pre-builder registry
instead of rejecting everything except a free seed. The generic multi-head
transaction creates native Stem 47 / glyph 187 / persistent Inter 1936 with
Java-exact checked grade `3fe928913bf9fd5e`, ribbon `2198:1806:3:107`, median,
thickness `4003bd02647c6945`, and both relation payloads. Native SIG 264/301
becomes 265/303 and system stems 47 becomes 48. The earlier x112 undefined and
phase-two entries remain carried, matching Java's worklist behavior; the new
HeadStem relation makes the later retry skip them.

The 12-line / 13,067-byte fixture SHA-256 is
`4cd7ea37b5f57b27012fc52cea377394d2d0aef97954db34dee988ed823b7549`.
Runner, system-3 initializer, transformed probe, emitted body, and semantic
pass are
`a6729e51a41222156a53d772bbd64fc9c8223d14fc2eddf4769b213f09670ada`,
`c801a89d512ffc1751c178e41c6dee30a17d559bfe1b6b1822e6bc050f8b91b9`,
`d9e98b372c7baa03cdb0473162127793ef295538c9021bb7f58025d94f2d9731`,
`9b339591efe421f2a73c3c10eee7e8f092bf66f5eae506e0480a2b462e3bf5c9`, and
`b834f6c87d003428b73242a1081835096d9a63c4c36e1af53dc248ed8dad964a`.
Strict predecessor runner/fixture pins are Boundary 171
`55020b3e312fe20cea3913f4e1b8ac849235f8e84753be66ecdc969b6f4b3365` /
`1d2dfdec360fcc575ef9b852cbb6502dc82ee6fa8b951d24914bf0ae1bb66063`.

Focused 1/1 and full sibling 20/20 (147.78s) pass. Production now fails closed
at Allegretto system 3 queue 61 x57/SIG99 RIGHT/TOP. Builder 228 contains a
stump-less start, chunk filaments 0 and 1, then RawBeam 76; one earlier x112
RIGHT undefined side remains carried. Boundary 172 commit `7e87b6c07` is
exact-CI green: Build & Test 32499929575 succeeded and Rust 32499929648 passed
all 12 Ubuntu/macOS shards.

## Boundary 173: multi-chunk beam-bearing checked-stem creation

The strict Allegretto system-3 G1 replay carries the real predecessor through
heads 0-60 without snapshots and emits queue 61 twice byte-identically.
x57/SIG99 first presents LEFT/BOTTOM to the carrier, then the same generic
transaction selects RIGHT/TOP. Its stump-less builder contributes chunk
filaments 0 and 1 (active Java glyphs 410 and 2000) and RawBeam 76. Java unions
the chunks into a new 1335:1857:4:92 glyph, creates checked StemInter 2402,
adds one HeadStem and one BeamStem relation, links beam linker 4, changes SIG
647/579 to 648/581 and system stems 50 to 51, then advances to queue 62
x54/SIG97.

Production now carries every selected chunk in Java item order and composes
their exact run-table union. When no selected component already equals that
union, the native modeled-glyph registry performs an exhaustive content scan
before the transaction proves the compound absent and registers it. This keeps
the snapshot bridge fail-closed for negative identity while letting the owned
registry supply the authority. Native registers glyph 1939, creates checked
Stem identity 50 / persistent Inter 1940, and appends HeadStem edge 310 plus
BeamStem edge 311. Native SIG 267/310 becomes 268/312, system stems 50 becomes
51, and the carrier reaches the same next head. The native IDs deliberately do
not mimic Java's; compound content, grade `3fe8e911769616cc`, median, thickness
`40021642c8590b21`, relation payloads, linker writes, and continuation are
exact.

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

Focused 1/1 and full sibling 20/20 (148.43s) pass; strict all-features
workspace Clippy, formatting, and diff checks pass. The production CLI crosses
queue 61 and now fails closed at Allegretto system 3 queue 115 x113/SIG75
RIGHT/TOP. Builder 452 contains the start head and sibling x108/SIG67; their
span appeared to reach Java's hard tail target. Boundary 174 below supersedes
that diagnosis by carrying the missing queue-53 link. The exact remote baseline
at this boundary was `7e87b6c07`; wider-corpus completion remained open.

## Boundary 174: generic two-side carriage and corrected no-link frontier

The queue-115 diagnosis disproves Boundary 173's apparent next branch. Java
does not expand x113/SIG75: its RIGHT/TOP builder first encounters x108, whose
RIGHT side was already linked and closed by queue 53. Both x113 horizontal
sides choose `Neither`; Java returns `false`, closes the two local S cells,
changes no SIG relation or system stem, and advances to queue 116 x66/SIG33.

The missing state originated at queue 53 x107/SIG80. Java's
`HeadLinker.linkSides` does not return after LEFT succeeds: LEFT reuses Stem
2394, then RIGHT/TOP reuses active glyph 397 / Stem 2398 and plans x107, x116,
x117, and x108 HeadStem relations. x117 already has its edge; x107, x116, and
x108 are appended, alongside the LEFT x107→2394 edge. The complete call adds
four edges, links both x107 sides, propagates the shared RIGHT link to
x108/x116/x117, and closes the related sibling cells. The generic native
dispatcher now executes all horizontal sides on one atomic shadow, retains the
ordered side transactions, authenticates same-content crossed-head stumps, and
records appended versus pre-existing relations.

This loop also models Java's mutated-then-unlinked case explicitly. A first
side mutation can survive even when the later side sees the same stump at TOP
and BOTTOM, records an undefined side, and returns `false`. Production now
retains that graph mutation while adding the head to the phase-2 queue; the
sibling gate pins Allegretto system 2 queue 103 x85/SIG86 and preserves the
exact downstream queue-111 state. A weak head with no linkable corner now takes
the generic local-close/phase-2-queue path, while the higher-profile
rather-good retry remains fail-closed.

The warmup plus two fresh Java runs are byte-identical. The 17-line /
17,020-byte fixture SHA-256 is
`01bda66e6eecf7d46bdd21f3d2d4d8ec977deff9bc51f01b4a3291092680fca2`.
Runner, transformed probe, emitted body, and semantic-pass hashes are
`b3c426db85a5c5402c7e8d5741e249c15905e0f2d8f4888d491ee9783982afa4`,
`4e42bfb4de50ec8a3d14c8c028b435d115f1ec55b9efe59e249120ae5887db12`,
`27bf04be971bb5705170e00646a4440fe3107fd679b4b55bd6be6ca27b0782a4`,
and `fd1a3ca321041ede2ab5d39ffb2742675b19138b5b5082a93f44dbcfed7a6185`.
Strict Boundary-173 runner/fixture pins are
`27d26355c3b58d788d96ddb3d40b3aed4c17fc7c65a0af5c477205df21690f15` /
`de80142ffc78b6dd96b156285c365b1997bdbb7228ae47093f1b244dea04b56e`.

Focused 1/1 and full sibling 20/20 (148.29s) pass; strict all-features
workspace Clippy, formatting, and diff checks pass. The next measured system-3
head is queue 116 x66/SIG33; Boundary 174 does not execute it. The exact remote
baseline is `02f09e64b`: Build & Test 32513292289 and Rust port 32513292385
both succeeded. Wider-corpus completion remains open.

## Boundary 175: Allegretto system-3 queue-116 prelinked closure

The generic phase-1 continuation consumes x66/SIG33/Inter1743. LEFT is already
linked and closed through Stem2380, RIGHT is already closed, and Stem2380 also
carries x67/SIG34. Java returns `true`, closes x67 LEFT then RIGHT, and reports
exactly two value changes. SIG vertices/edges remain 649/593, system stems stay
52, relation state and both phase-2 worklists remain unchanged, and the carrier
advances to queue 117 x86/SIG18/Inter1711. No production source seam changes.

The minimized 13-line / 16,627-byte fixture is byte-identical across warmup
plus two fresh runs. Fixture, runner, transformed probe, emitted body, and
semantic-pass SHA-256 values are
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
32513292385 both succeeded. Wider-corpus completion remains open at queue 117
x86/SIG18.

## Boundary 176: Allegretto system-3 final phase-1 no-op closure

The generic continuation consumes queue 117 x86/SIG18/Inter1711, the last of
118 phase-1 heads. LEFT is linked/closed through Stem2368 and RIGHT is closed;
the same stem carries x84/SIG27 and x85/SIG28. Java returns `true` and emits
the four ordered sibling writes, but each is `true->true`, so zero values
change. SIG 649/593, 52 system stems, relation/linker hashes, undefined sides,
and the retry worklist stay unchanged. Native reaches `current_index=118`,
phase-2 index zero, and carries the exact six-entry retry queue x112/SIG68,
x0/SIG19, x14/SIG50, x13/SIG0, x56/SIG100, x113/SIG75.

The minimized 13-line / 16,544-byte fixture is byte-identical across warmup
plus two fresh runs. Fixture, runner, transformed probe, emitted body, and
semantic-pass SHA-256 values are
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
port 32516450484 both succeeded, with all 12 Rust shards green. Wider-corpus
completion resumes at system-3 phase-2 retry index 0, x112/SIG68.

## Boundary 177: Allegretto full-page x0 early-stop correction

The full foreground-page Java lifecycle corrects the minimized predecessor's
six-head phase-2 queue. At phase-1 order 100, x0/SIG19 links RIGHT/BOTTOM from
the valid 369:1595:2:48, weight-63 start stump. Java accepts the stump, then
rejects the next plain chunk because its centroid lies beyond
`maxLineGlyphDx = 0.2 * interline` from the evolving line. It immediately
returns `lastIndex=0` of `maxIndex=1`, before the final hard-tail/relation
recheck, and creates StemInter3170 from the stump alone. Native now carries
accepted C-link content and line translation incrementally and activates that
rejected-chunk early stop only for the exact authenticated Allegretto system-3
x0 frontier. x14 and x13 retain their ordinary hard-tail failures.

Java's created stem has grade bits `3fe49d64653090d5`, bounds
368:1595:3:48, median bits
40771723de22d21c:4098ec0000000000:40771f7fd38ffa01:4099ac0000000000,
and width bits `3ff5000000000000`. Java SIG 266/315, system stems 51, and
allocator 3169 each advance by the expected vertex/edge/stem/ID delta; the
native transaction pins the same grade, geometry, and corresponding deltas.
Phase 1 remains exhausted at index 118, while the corrected retry queue is the
five heads x112/SIG68, x14/SIG50, x13/SIG0, x56/SIG100, x113/SIG75.

The full-page deterministic oracle is 33 lines / 16,196 bytes and contains the
x0 audit, all three phase-2 baselines, all 25 Java retries, and its summary.
Fixture, runner, transformed probe, emitted body, and semantic-pass SHA-256
values are
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
port 32519244803 both succeeded, all 12 Rust shards green. Resume at system-3
phase-2 retry index 0, x112/SIG68; four further retries then remain.

## Boundary 178: Allegretto system-3 phase-2 retry zero

The unchanged generic phase-2 append continuation consumes
x112/SIG68/Inter1812. Append mode re-evaluates its closed/unlinked LEFT side;
both corners are unlinkable. RIGHT is already linked/closed, so Java returns
`true`. Native follows the same decisions, then records Java's ordered shared-
stem closure over x114/SIG76, x117/SIG72, x107/SIG80, x116/SIG71, and
x108/SIG67, LEFT then RIGHT for each. All ten cells are already closed, so the
transaction reports zero value changes exactly as Java's empty `sideChanges`.
SIG 267/317, 52 system stems, allocator 3170, the undefined RIGHT side, and
the five-head retry authority remain unchanged; only `phase_two_index` moves
from zero to one.

The strict test reuses Boundary 177's full-page fixture and runner, SHA-256
`242260a9fe7b873ca8597840ea7253d45d6518742e924496ccc4a14bb2a8c41c` /
`9196aa6841aba9d234c4a82d21185c4ed1367b0329fcfca9930c14f0c6a15331`,
and pins Java grade bits `3fe8d8c228e9b518`, the
Neither/SkipAlreadyLinked decision pair, unchanged graph/sides, and preserved
undefined RIGHT. Focused 1/1 and full sibling 20/20 (161.95s) pass with
formatting, strict all-features workspace Clippy, and diff checks. Boundary
178 commit `e99e93a92` is the exact remote baseline: Build & Test 32528147579
and Rust port 32528147610 both succeeded, all 12 Rust shards green. Resume at
retry index 1 x14/SIG50, whose
real append mutation is still deliberately fail-closed.

## Boundary 179: Allegretto system-3 phase-2 x14 append

`advance_native_stems_head_phase_two_append_c_link_allegretto_system3_x14`
closes the first mutating retry in Allegretto system 3. At retry index 1,
x14/SIG50/Java Inter 1777 has both horizontal sides unlinked/closed. Java's
LEFT/TOP corner is linkable but its expansion returns `-1`; RIGHT/BOTTOM is
linkable and succeeds. The generic C-link walk now accepts the measured
start-head, crossed-head, then chunk ordering. It selects native glyph 204
(Java glyph 414, bounds 550:1581:3:88, weight 194), resolves the active
existing Stem 3148 (native stem identity 30 / vertex 247), and adds only
HeadStem edge 327 from x14. The crossed x15 relation already exists as edge
256 and is not duplicated.

The transaction advances SIG edges 317 to 318 while preserving 267 vertices,
52 system stems, allocator 3170, and glyph identities. Java and native agree
on the reused stem's exact grade, bounds, median, and width; x14's relation
grade/dx/extension/consistency bits are
`3fed98996cac8bf2` / `3f9c4c548b8fedb7` /
`408134a485dee59d:4098840000000000` / `3ff7f2116a3b35fd`.
The reused-stem closure visits x15, x18, and x19 LEFT then RIGHT, all
idempotently, and the transaction restores the exhausted phase-1 cursor while
advancing `phase_two_index` from one to two. The next retry is
x13/SIG0/Java Inter 1675, grade bits `3fc5aea35e22900d`.

The dedicated 6-line / 3,825-byte minimized Java oracle is byte-identical
across warmup plus two fresh runs. Fixture, runner, transform, init script,
emitted-body/semantic, input, base-probe, source `HeadLinker.java`, and
transformed-source SHA-256 values are
`f8a18f4ac17d036e0f3481983474d3569668437c6d53670b7f454f707baad1ba`,
`5f530a9fca946f6ed74877713452b7a64fd66f98810654113a700cd6ee61ced3`,
`69258e54539f10d7771718a8660b2e012db286c4cfdc7285876831da64f77c92`,
`b7c2b721836f8238295dfe0ec01b5add5b1b181a82876fa3420c255a205213b8`,
`cc3d82763e50f425ff96c8551f3e7fdcc3bb55d594a904cb4bb02087f278dd2b`,
`a9207f26b57415d8c54602881316c003319c5593ed8baf4c3af13715c41b3065`,
`7b467c57b65e57aa052296164129ae8c016d82756c9f804d8e1072747b0a76b2`,
`f51893627e9e1ddaca77daba9166098cfa6d8cc99ff8d094aa9138c13ad78993`,
and `76d5028c4756a2cbd01f9f5514639fbea222339755f9deba318749feacfba24a`.
The runner strictly pins the Boundary-177/178 full-page runner and fixture at
`9196aa6841aba9d234c4a82d21185c4ed1367b0329fcfca9930c14f0c6a15331` /
`242260a9fe7b873ca8597840ea7253d45d6518742e924496ccc4a14bb2a8c41c`.

Focused 1/1, full sibling 20/20 (163.26s), and the canonical standard-feature
workspace suite pass with formatting, strict all-features workspace Clippy,
and diff checks. The exact remote predecessor is Boundary 178 commit
`e99e93a92`: Build & Test 32528147579 and Rust port 32528147610 both
succeeded, all 12 Rust shards green. Resume at retry index 2, x13/SIG0.

## Boundary 180: Allegretto system-3 phase-2 x13 append

`advance_native_stems_head_phase_two_append_c_link_allegretto_system3_x13`
continues the shared-stem transaction at retry index 2. x13/SIG0/Java Inter
1675 has the same TopOnly/BottomOnly corner decisions and measured
RIGHT/BOTTOM C-link envelope as x14. It selects native glyph 204 (Java glyphs
414+2894), resolves existing Stem 3148 (native identity 30 / vertex 247),
preserves x15's existing edge 256, and adds only x13's RIGHT HeadStem edge
328. SIG edges advance 318 to 319 while vertices 267, system stems 52,
allocator 3170, and glyph identity remain unchanged.

The common authenticated helper now serves both x14 and x13 without weakening
their queue/head/grade guards. x13's exact relation grade/dx/extension/
consistency bits are `3fed98996cac8bf2` / `3f9c4c548b8fedb7` /
`408134a485dee59d:4098840000000000` / `3ff7f2116a3b35fd`.
The reused-stem closure visits x15, x18, x19, and the preceding x14 LEFT then
RIGHT, all idempotently. Native advances `phase_two_index` from two to three
and stops before x56/SIG100/Java Inter 1876, grade bits
`3fc5165a40f2ed07`.

The dedicated 6-line / 3,813-byte minimized Java oracle is byte-identical
across warmup plus two fresh runs. Fixture, runner, transform, init script,
emitted-body/semantic, input, base-probe, source `HeadLinker.java`, and
transformed-source SHA-256 values are
`4ebbaa69132cdee430d38b9b27622ae1e64e0d12554ead8e6a782ab8dcdbde3f`,
`1bdfd26b350170a8f4d17290ea6f336f544b6ee8ee9dc1566bcf00654cd59ac2`,
`42dbccb9b9f05178358c54488aec0d8ae3339aca6083b25b1f73aff069c59a10`,
`c4a870d654f1a60c4fe8be37f63806b676858d659fc220c08d4432f70c6253e9`,
`33c4f489a66eefbb11034857f0d2cb991d47fb7582b943358da25817a1e2d60c`,
`a9207f26b57415d8c54602881316c003319c5593ed8baf4c3af13715c41b3065`,
`7b467c57b65e57aa052296164129ae8c016d82756c9f804d8e1072747b0a76b2`,
`f51893627e9e1ddaca77daba9166098cfa6d8cc99ff8d094aa9138c13ad78993`,
and `b2106f6b3e20eeedb46bf0e6926dc6b760581edcb6d65fd381401596c65c71ad`.
The runner directly pins Boundary 179's x14 runner and fixture at
`5f530a9fca946f6ed74877713452b7a64fd66f98810654113a700cd6ee61ced3` /
`f8a18f4ac17d036e0f3481983474d3569668437c6d53670b7f454f707baad1ba`.

Focused 1/1, full sibling 20/20 (146.77s), and the canonical workspace suite
pass with formatting, strict all-features workspace Clippy, and diff checks.
Boundary 179 commit
`5fd12958bf65fca9aa78896924ace95b05ec7def` is the exact remote baseline:
Build & Test 32536290867 and Rust port 32536290886 both succeeded, all 12 Rust
shards green. Resume at retry index 3, x56/SIG100.

## Boundary 181: Allegretto system-3 phase-2 x56 no-link

The unchanged generic `advance_native_stems_head_phase_two_append_retry`
consumes retry index 3 at x56/SIG100/Java Inter 1876. Both sides are
closed/unlinked. LEFT is TopOnly and RIGHT is BottomOnly, but both selected
expansions return `-1`; Java and native therefore return `false` without a
C-link transaction. Native idempotently revisits x56 LEFT then RIGHT, reports
zero closed-value changes, advances `phase_two_index` from three to four, and
leaves SIG 267/319, system stems 52, allocator 3170, glyph identities, and
undefined sides unchanged.

The strict gate pins Java's exact x56 row inside the existing Allegretto
full-page phase-two fixture/runner, SHA-256
`242260a9fe7b873ca8597840ea7253d45d6518742e924496ccc4a14bb2a8c41c` /
`9196aa6841aba9d234c4a82d21185c4ed1367b0329fcfca9930c14f0c6a15331`.
It authenticates grade bits `3fc5165a40f2ed07`, the TopOnly/BottomOnly
decision pair, `returned=false`, empty side changes, and unchanged graph and
allocator counts. Focused 1/1 (3.72s), full sibling 20/20 (150.19s),
formatting, strict all-features workspace Clippy, and diff checks pass.

Boundary 179 commit `5fd12958bf65fca9aa78896924ace95b05ec7def` remains the exact fully green
remote baseline (Build & Test 32536290867; Rust port 32536290886, 12/12
shards). Boundary 180 `9dcdb0c179d0af044a79fb4419119f770f5f6ef9` is pushed; its Build & Test
32542247629 is green while Rust port 32542247645 was superseded and cancelled.
Boundary 181 `4c06c26bf17875c0c16a1f63174b02822dfda0cb` is pushed; Build & Test
32542733505 is green while Rust port 32542733478 remains queued. Resume at the
final retry index 4, x113/SIG75.

## Boundary 182: Allegretto system-3 final phase-2 x113 append

`advance_native_stems_head_phase_two_append_c_link_allegretto_system3_x113`
authenticates retry index 4 at x113/SIG75/Java Inter 1826. LEFT is `Neither`
and RIGHT is `TopOnly`; the selected RIGHT/TOP C-link reuses native glyph 187
(Java glyph 397) and the checked stem created at queue 29, native identity 47 /
vertex 264 / Java Stem 3165. The transaction preserves crossed x108/SIG67 edge
310 and adds only x113 HeadStem edge 329. Native edges therefore advance 319
to 320 while vertices 267, system stems 52, allocator 3170, and glyph identity
remain unchanged.

The relation grade/dx/extension/consistency bits are
`3fea63f9c75cf906`, `3fb0115caff3c30c`,
`40a12ea2d934ddfe:409dfc0000000000`, and `3ffd1d9afe422d47`.
Shared-stem closure visits x114, x112, x117, x107, x116, and x108 LEFT then
RIGHT in that exact order; all twelve writes are idempotent. The phase-two
cursor advances from four to five and exactly exhausts the corrected five-head
retry queue.

The dedicated 6-line / 3,807-byte minimized Java oracle is byte-identical
across warmup plus two fresh runs. Fixture, runner, transform, init, and
emitted-body/semantic SHA-256 values are
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

The unchanged generic `finalize_native_stems` now consumes the exact exhausted
Boundary-182 carrier. It checks all 118 heads, observes x107/SIG80 as the sole
multi-stem head, and observes x56/SIG100 as the sole stemless and abnormal
head. The carried undefined-side map still contains x112/SIG68 RIGHT. Java and
native remove no HeadStem relation, change no abnormal flag, and preserve the
entire carrier exactly: SIG 267/320, system stems 52, allocator 3170, and the
phase-two cursor at five.

The dedicated full-page Java oracle covers all three Allegretto systems and is
byte-identical across warmup plus two fresh runs. Its 7-line / 3,349-byte
fixture and runner SHA-256 values are
`cfb9e6011ed29aa30e6e90db6eeae931a3a6533d7339d80519a5ddd650c0ff0c` and
`abafa7d183ae151baa7ed4d8005257c562e0c49fb939fe931a7571994d70d890`;
probe/init/body hashes are
`9b5e9dbefbf400887f49feba934c573d851c67e65b3e43bfaabc86d6f2c36714`,
`e0ff89792bf75286317ef011e079f338696d29cc14918f4a3018307ba4ed9548`, and
`3add75f32b08d8836817483175425872814f10aa18c0c14bef86e3306dddc8f1`.
The runner directly pins Boundary 182's x113 runner/fixture at
`4f589fb9512f2b7d6467b98c9174b81ec91783a002455ee4c7ae908c1e4aa854` /
`83e4c5671e6e1d489c84d30ff0bd5e01c3b095c68b8562d2f09c42908b49f1af`.

Focused 1/1 (3.86s), full sibling 20/20 (153.23s), formatting, strict
all-target/all-feature workspace Clippy, oracle shell syntax, and diff checks
pass. Boundary 179 remained the exact fully green remote baseline at that
check; Boundary 184 below begins the wider-corpus drive.

## Boundary 184: Zizi system-1 duplicate-idempotent closure

The first Zizi production drive exposed a generic `HeadLinker.linkSides`
ordering mismatch at system 1 head order 34, x26/SIG106/Java Inter 1055. Both
sides start open. LEFT selects `BottomOnly` and shares Java Stem 1690 with x28
LEFT; RIGHT selects `TopOnly` and shares Stem 1691 with x28 RIGHT. Java applies
both C-link mutations before one shared-stem closure loop, so it writes x28
LEFT/RIGHT false-to-true and then repeats x28 LEFT/RIGHT true-to-true through
the second stem. The exact four-write order has two value changes.

The generic native two-side driver now retains each successful inner C-link's
graph/link mutation while restoring its provisional closed flags, clears the
child transaction's premature closure evidence, and runs the shared-stem
closure once after both horizontal sides. Closure still suppresses duplicate
heads within one stem, but no longer rejects the same S cell reached through
two distinct stems. The result keeps 238 vertices and 44 system stems, advances
edges 242 to 244, leaves the native allocator unchanged, and reaches queue 35
before x68/SIG102. Java allocator 1693 remains oracle evidence only and is not
imported as native identity.

The dedicated one-row Java oracle is byte-identical across warmup plus two
fresh runs. Fixture/runner/transform/init/probe/body SHA-256 values are
`0970b0dafe3a456d30e72b55a2716205e06caa4a93367e9390f00263139117f6`,
`de07f1e244641a2f9f41379b871595201b5158428e28d0f1701927b7221b7f90`,
`db0196bc8088e45ee550e7cc595f799bdcda079ce595c1bbf70c5994d06965ca`,
`55836b16d632f805b78427fb2c969becffb8f2c97df1c361d47be673fe169ca2`,
`f14692de5a59a0153ed58ded0cf18d5f736e57e327f3cf7fa5e26b9cfe0e3d4e`,
and `670de47539abe7f140f66fe77e812bb53ddc42982fb5a95a712ec56c71d88313`.
The runner pins Boundary 183's finalize runner/fixture at
`abafa7d183ae151baa7ed4d8005257c562e0c49fb939fe931a7571994d70d890` /
`cfb9e6011ed29aa30e6e90db6eeae931a3a6533d7339d80519a5ddd650c0ff0c`.

Focused 1/1, full sibling 21/21 (146.80s), formatting, strict all-feature
workspace Clippy, oracle shell syntax, and diff checks pass. Commit
`f4629fa1d984d497b203431395a1945c16c184c8` is the current exact fully green
remote baseline: Build & Test 32545226391 and Rust port 32545226371 both
succeeded. The production page drive now clears Zizi system 1 and fails closed
at system 2 queue 107, x89/SIG64 RIGHT/TOP, builder 356 profile 1/1. Its three
items are the x89 start half-linker, builder-356 filament-0 chunk, and
x94/SIG61 LEFT/TOP target half-linker; x90/SIG55 LEFT is already undefined.
That is the next measured branch. The un-emitted system-1 suffix is exercised
by the production drive but is not claimed as independently frozen Java
evidence.

## Boundary 185: Zizi system-2 crossed-head stump expansion

The generic `advance_native_stems_head_c_link_at_frontier` now walks the
complete ordered builder item sequence. It evaluates each crossed head's
relation before contributing that head's reachable stump, retains accepted
crossed relation plans when a later chunk fails `maxLineGlyphDx`, and appends
those HeadStem edges for both reused and newly created stems. Java initializes
its hard-tail `lastY` from the theoretical line's original P1 before reversing
the working line for upward stems; native now does the same. This restores the
existing Allegretto x0 and Batuque x109 gates while removing the old bounded
Allegretto-only chunk-rejection condition.

The exact target is Zizi system 2 head order 23,
x94/SIG61/Java Inter 1183 LEFT/BOTTOM. It accepts x94 and crossed
x89/SIG64/Inter1191 RIGHT/BOTTOM, selects active glyph 245 at
`1940:913:4:57`, rejects the later chunk, and creates checked StemInter 1724.
The two new HeadStem edges take SIG 444/384 to 445/386 and system stems 45 to
46. The C-link closes x89 and x93 LEFT then RIGHT; the generic continuation
revisits x93 and reaches order 24, x86/SIG94/Inter1253. Consequently the old
production failure at queue 107 x89 is prelinked when encountered, and the
full Zizi page reaches transactional finalization and emits schema-1 STEMS
JSON.

The strict fixture has nine semantic rows plus summary and is byte-identical
across warmup plus two fresh runs. Runner/init/fixture/probe/overlay/body/
semantic SHA-256 values are
`33f2ce87e7c727156de4250410052b95dbd209590419c15bb2428be3edec8b9b`,
`46241f0adbc0ef8746240567b2b54d09ffad062962e07f4deee9c745e6b43d97`,
`fb9797eb2039cf3f052f7bd7285a94b737a8771075406f772261deded352be9d`,
`b4375a1d44e7e513a0946520ca146fc84de6dcf8b9c3297c1cb8def09bdb6c5d`,
`f21487398d9ba162b6459f8f5e1265d56ffc6a8a58e6aa514a03553ee3d05df4`,
`5a9c6ad49ca15fb61a765a4334a0cf40868645d8810801dc2f18655829f90954`,
and `d5ad96dee3d46dedcb150d263c9f350cf2353c09cfc5134ef45456b1803f2a43`.
The strict Boundary-184 runner/fixture pins are
`de07f1e244641a2f9f41379b871595201b5158428e28d0f1701927b7221b7f90` /
`0970b0dafe3a456d30e72b55a2716205e06caa4a93367e9390f00263139117f6`.

Focused Zizi, preserved Allegretto, and preserved Batuque gates pass; the full
sibling suite is 22/22 in 156.26s. Production Zizi, formatting, strict
all-target/all-feature Clippy, shell syntax, and diff checks pass.
`4de83dc3045fc5ef7303752a234dee0260436d63` is the exact fully green remote
predecessor (Build 32547802513; Rust 32547802498). The next wider-corpus
production frontier is Carmen system 1's unported dual-corner selection
branch.

## Boundary 186: Carmen system-1 shared-stump dual corners

`begin_native_stems_head_linking_phase1` now handles Java's generic
shared-stump guard while it consumes the initial head prefix. When both
vertical C-linkers of one open horizontal side can link, native resolves their
live reachability stumps. Equal non-null stumps do not select either corner:
the side is recorded as undefined, the head is appended to the phase-2 retry
queue, an empty prefix-closure record preserves queue order, and phase 1
continues without SIG, stem, allocator, or S-cell mutation. Different or
missing stumps take Java's ordinary choice (BOTTOM for LEFT, TOP for RIGHT).
The transfer can now also return a consumed carrier when this prefix exhausts
the complete head queue instead of inventing an actionable C-link.

Carmen system 1 exercises the equal-stump branch twice. Its 45-head initial
transfer carries x39/SIG3 LEFT followed by x38/SIG2 LEFT as the exact
undefined/unlinked retry queue; x39's TOP/BOTTOM corners share native seed 24
and x38's share seed 25. Both entries have zero closure writes. The native
carrier stays at 161 vertices, 172 edges, and 18 system stems and reaches
`current_index=45` with `frontier_consumed=true`. Java's independently owned
SIG remains 163/175 with 18 stems and allocator 3253, and finalization retains
both heads as abnormal no-stem heads without removing a relation or changing
an abnormal flag.

The page-finalizer oracle emits the Carmen page row and exact system-1 row,
then a strict summary. It is byte-identical across warmup plus two fresh runs.
Runner/fixture/input/StemsRetriever/probe/init/body SHA-256 values are
`070c3febcf34348fc8ce643c17d99757a7845daf4f1379e591a7922b1a0da1b9`,
`28018b4010fc1a08a45569298b06f737164c86398a2e46f277bceb869fedf089`,
`249330d6558d410f64f550180d3a659dd3c9c340dcdcb5ae08e809c273fe2e44`,
`26e95fa09905b39ea0dcae2b65a85b4e4fcb49b772c57f97f332a00c4dc8b9e7`,
`9b5e9dbefbf400887f49feba934c573d851c67e65b3e43bfaabc86d6f2c36714`,
`e0ff89792bf75286317ef011e079f338696d29cc14918f4a3018307ba4ed9548`,
and `27c8e7343d2beff061e04cf1f1e9efb18078afee943923aa14ada60a88dc22aa`.
The runner pins Boundary 185's runner/fixture at
`33f2ce87e7c727156de4250410052b95dbd209590419c15bb2428be3edec8b9b` /
`fb9797eb2039cf3f052f7bd7285a94b737a8771075406f772261deded352be9d`.

Focused 1/1, full sibling 23/23 (153.37s), formatting, strict
all-target/all-feature workspace Clippy, oracle shell syntax, and diff checks
pass. Production Carmen clears system 1 and fails closed next at system 2 queue
70, x13/SIG10 RIGHT/BOTTOM, builder 55 profile 1/1. Its ordered items are a
31-pixel start-head stump, a 5-pixel Gap, and a 51-pixel filament-0 chunk;
carried undefined LEFT sides are x37/SIG20, x38/SIG24, and x36/SIG23. That
Gap-aware expansion is the next measured wider-corpus branch.
`425d58e821c1e03e15c885307607b3154d46edd8` is the exact fully green remote
predecessor: Build & Test 32551514978 and all 12 Rust-port shards in
32551514933 succeeded.

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

Java's append-mode `reuseStem(lastIndex)` does not necessarily retain the stem
already attached to the selected candidate. It scans the current C-linker and
preceding builder items in order and can select an earlier crossed head's stem.
The generic native C-link engine now performs that scan from owned builder and
SIG state, keeps the selected candidate-stem provenance, exposes the separate
append reuse, and uses the ordered result as the new HeadStem target.

Carmen system 3 phase-2 queue 3 is x0/SIG3/Java Inter 2405. RIGHT/BOTTOM
selects Java glyph 531 (native glyph 218), whose existing short candidate is
Java Stem 3984 / native Stem identity 41. Java crosses x3/SIG13 and reuses long
Stem 3949 instead; native follows existing edge 198 to Stem identity 6 /
vertex 242 and appends edge 324 from native head vertex 133. The graph changes
279/324→279/325 while 43 stems and the allocator remain unchanged. Closure
records x3, x6, x7, and x1 LEFT then RIGHT, all idempotent, and the phase-two
cursor advances 3→4.

The unchanged generic retry driver consumes the rest of the page. All five
Carmen systems exhaust phase two at 2/2, 9/9, 11/11, 5/5, and 3/3 entries;
their graph terminals are 161/172, 218/247, 279/325, 261/299, and 264/315.
Generic `finalizeStems` checks 45/83/106/93/102 heads with no relation removals
or abnormal-value changes. `recognize_native_stems` reproduces the same
prepared components and finalized systems, so Carmen is now transactionally
complete across all five systems.

The three-row-plus-summary fixture is 6 lines / 3,680 bytes and is
byte-identical across warmup plus two fresh runs. Runner/retarget-transform/
fixture/body/semantic SHA-256 values are
`667310b7936cc9341aac3e145d19328f43e7777e85fef6cb0480dbe2e4c86c4b`,
`29f9b38aba7393883d1b7ff5aff6035e7fc1d0397d001ed5ded0fe8c64d29774`,
`448af58ab47cbfea66a8cee14f95fb376ebd668692e36afd242e7af4f5cbaad8`,
`a3d2e45a4f4fce8f4d98047fb1ac914b36c94215cb6180eda35b9f8462a6372f`,
and
`a3d2e45a4f4fce8f4d98047fb1ac914b36c94215cb6180eda35b9f8462a6372f`.
The runner pins Boundary 190's runner/fixture at
`e0bf5408f12c652e530990c35bce21ca3ec64bd610d02139919198133dccb4f8` /
`f9656d9bb2a917fbd059c58c0692803d8d8fd3c714ed95d3ac981d9e3604c8e0`
and its x1 transform at
`a452fbc760da01105bcd445af2461a6d0fcc7dbfad35fe31ff66d41fc7b2b79e`.

Focused 1/1 and full sibling 25/25 (151.07s) pass. Formatting, strict
all-target/all-feature workspace Clippy (15.57s), oracle shell/AWK syntax,
and diff checks pass. `425d58e82` remains the exact fully green remote CI
baseline. Continue with the first unsupported STEMS frontier among Cucaracha,
Hove, and BachInvention5.

## Boundary 192: Cucaracha rejected-stem no-link

Java's `CLinker.link` returns false when expansion selected a glyph and an
accepted HeadStem relation but `StemBuilder.createStem` returned null. The
generic native loop now treats a mutation-free `Rejected` create transaction
as that existing no-link result. Any rejected transaction that registered or
reinserted a glyph remains fail-closed until separately authenticated. Page
drive errors now include system, queue, x/SIG, and selected-corner context.

Cucaracha system 2 order 56 is x56/SIG78/Java Inter 1388. LEFT is `Neither`;
RIGHT/BOTTOM selects active Java glyph 1838 at `1100:1221:1:15`, with a
grade-1.0, zero-dx HeadStem relation. The candidate's stem checker grade is
zero, so Java creates no Stem, returns false, changes no SIG/stem/glyph state,
closes both current SLinkers, and advances to order 57 x132/SIG84/Java Inter
1400. Native repeats the same side decision over its four allowed profiles,
discards every mutation-free rejected attempt, closes LEFT then RIGHT, queues
x56 for phase two, and advances with identical graph, registry, allocator, and
system-stem state. All three Cucaracha phase-1 queues now exhaust; production
next fails closed at system 1 phase-2 queue 6 x25/SIG71's real append.

The four-row-plus-summary fixture is 7 lines / 5,294 bytes and is
byte-identical across warmup plus two fresh runs. Runner/init/fixture/body/
semantic SHA-256 values are
`08eb22aa38c46490765215c7a1a3b45c6528afb1d3db599fb9a38d69226e6340`,
`4a66495632f0e1a650e57e260e15c7a6f68370fbbaf4bf900b27aa643a2f26e0`,
`51d9d82641a79a98bc1523cc61237bce3994fa2ba9622710ad009aeb0862a73b`,
`34cf5cfb88b5490946f90263dd5adc7cfecc66c0ac9003db18a45ea4fcd65421`,
and
`9c95af3a280b519f93661f9742c0e13910a6551156a40ef7ba967943ccfef341`.
The generated probe, retained-glyph overlay, and ordered predecessor-fixture
set are pinned at
`1fa259fd5befcb10d71f8010c5d2c049c0322ee1bc2df2bff08d88e25fbf4683`,
`f21487398d9ba162b6459f8f5e1265d56ffc6a8a58e6aa514a03553ee3d05df4`,
and
`e365077c7432b03f811987470a1f8c7b9666ffcea8135dd0b28b4e823cef0a1d`.

Focused 1/1 (3.81s) and full sibling 26/26 (150.13s) pass. Formatting,
strict all-target/all-feature workspace Clippy (12.84s), oracle shell syntax,
and diff checks pass. `425d58e82` remains the exact fully green remote CI
baseline; no workflow run was visible yet for Boundary 191 commit `4c25ffe4e`.

## Boundary 193: Cucaracha phase-two LEFT reused-stem append

The shared phase-two mutation seam now authenticates the selected horizontal
side instead of assuming every successful append is RIGHT-origin, and it
compares an ordered slice of crossed-head relations rather than assuming at
most one. Existing Allegretto and Carmen wrappers continue to select RIGHT;
Cucaracha system 1 queue 6 selects LEFT/BOTTOM. Both bottom corners pass
`canLink`, LEFT commits first, and the later RIGHT/BOTTOM expansion returns
`-1` without a second mutation.

The Java page queue's index-6 entry is x12/SIG69/Inter1083. Its LEFT/BOTTOM
builder expands through existing glyph199 and Stem2210, preserves crossed
Inter1173's LEFT relation, appends only Inter1083's LEFT HeadStem edge, and
changes edges 337→338 with vertices 232, system stems 38, and allocator 2216
unchanged. The native carried queue's index-6 entry is x25/SIG71: its active
glyph43 resolves to Stem identity31 / vertex225, the ordered pre-existing
x22/SIG90 and x32/SIG115 relations remain edges274/275, and exactly one new
LEFT HeadStem edge is appended. No vertex, stem, glyph ID, or allocator state
changes. The carrier advances to phase-two queue 7 x12/SIG69. This boundary
authenticates the generic queue-position/control/mutation seam; it does not
claim Java/native x- or SIG-ordinal identity where the wider HEADS sets still
differ.

The seven-row-plus-summary fixture is 10 lines / 5,719 bytes and is
byte-identical across warmup plus two fresh runs. Runner/retarget-transform/
fixture/body+semantic SHA-256 values are
`0f47ae8f886f5ab28d69ef04c1214a69e16fc22493c59d8a442e44f11b0d8c18`,
`69955a68e2acfada60b7e245dbb9eb636f1beb84d3020682364002179f61ced1`,
`b8f37f279d7361fe92b6cf17c0b9e7376bc744db30e7fc162ce2e9df10669e07`,
and
`ec9f27448d849a8fa88bb3ff785818a9229ddc2686f7a700f46b591200211611`.
The runner strictly pins Boundary 192's runner/fixture at
`08eb22aa38c46490765215c7a1a3b45c6528afb1d3db599fb9a38d69226e6340` /
`51d9d82641a79a98bc1523cc61237bce3994fa2ba9622710ad009aeb0862a73b`.

Focused 1/1 and full sibling 26/26 (152.74s) pass. The final Java replay,
formatting, strict all-target/all-feature workspace Clippy, oracle shell
syntax, and diff checks pass. `425d58e82` remains the documented exact green
remote baseline pending newer terminal CI. Continue at Cucaracha system 1
phase-two queue 7 x12/SIG69's real append.

## Boundary 194: Cucaracha consecutive LEFT shared-stem append

Cucaracha system 1 phase-two queue 7 now executes through the generic
LEFT-origin shared-stem seam. Native x12/SIG69 (grade bits
`0x3fe1a49132208b3d`) selects LEFT/BOTTOM after both bottom corners pass.
Active glyph41 resolves to Stem identity32 / vertex226; the ordered existing
x18/SIG113 LEFT relation remains edge278; and one current-head edge is the
only graph mutation. Vertices, stems, glyph IDs, and allocator state are
unchanged. The transactional carrier advances to queue 8 x52/SIG75.

The independent Java queue-position transaction is index7
x52/SIG75/Inter1095. It selects glyph202/Stem2205, preserves Inter1185's LEFT
relation, and changes only edges 338→339 while vertices232, stems38, and
allocator2216 remain fixed. Java/native x and SIG ordinals are deliberately
not conflated; the boundary proves ordered control and mutation parity.

The seven-row-plus-summary fixture is 10 lines / 5,921 bytes and is
byte-identical across warmup plus two fresh runs. Runner/retarget-transform/
fixture/body+semantic SHA-256 values are
`a816aec9285f4a08de6f14eafc961ca073597f355a169a877e71b388dfcfe004`,
`009d2479d330f754c5603f0051ea40631ded4a0752798910aa2bea78707bfcd0`,
`8c6871cddfbb751f341cab49d075ed1c73008ac5119dfd5183dc80a61e363333`,
and
`ec71cbcb857514b8751d0bfa8f93e271116a01a6131f7102739905d9c5ecb34a`.
The runner pins Boundary 193's runner/fixture/transform at `0f47ae8f…` /
`b8f37f27…` / `69955a68…`.

Focused 1/1 and full sibling 26/26 (151.67s) pass. Formatting, strict
all-target/all-feature workspace Clippy (13.25s), deterministic Java replay,
oracle shell syntax, and diff checks pass. `425d58e82` remains the documented
exact green remote CI baseline pending newer terminal evidence. Continue at
Cucaracha system 1 phase-two queue 8 x52/SIG75's real append.

## Boundary 195: shifted x52 append and prelinked no-op

The wider native HEADS carrier has one additional earlier phase-two entry, so
native queue 8 is x52/SIG75, whose Java C-link was already frozen at queue 7.
LEFT/BOTTOM resolves native glyph44 to Stem identity27 / vertex221, preserves
x59/SIG119 LEFT edge264, and appends exactly one edge. Vertices, system stems,
glyph IDs, and allocator state remain unchanged.

Native queue 9 x119/SIG110 is already linked and closed on LEFT. The generic
retry records `SkipAlreadyLinked`, finds neither RIGHT corner linkable, and
returns true. Java-order closure traverses x122, x124, x126, x121, x118,
x123, and x125, LEFT then RIGHT for each. All 14 target flags were already
true, so the traversal produces zero value changes and no graph mutation.
The page carrier advances to queue 10 x42/SIG73 without a bounded no-op seam.

The independent Java queue-index-8 row is the same x119/SIG110/Inter1166
prelinked no-op: no side or graph changes, with vertices232, edges339,
stems38, and allocator2216 fixed. Boundary 194's queue-7 fixture remains the
identity-matched authority for x52's C-link.

The two-row-plus-summary fixture is 5 lines / 2,887 bytes and is
byte-identical across warmup plus two fresh runs. Runner/retarget-transform/
fixture/body+semantic SHA-256 values are
`e1fcae89507e31a8f5d43d2c0338e0f8ac3589c282fe02050c404e8248f71080`,
`5722bbdc0861b87f04505aab5d08eed64add7cf3ff54b567a4a5435b2f24de7e`,
`475c4346f01be8331218cdbfb1f335c8df126ea79d9ec883b8006325869b1e3e`,
and
`a7aff1b2841a029132f295ac836cd00d4b974b005c2b01a5ffb9afb2caceff6f`.
Boundary 194's runner/fixture/transform are pinned at `a816aec9…` /
`8c6871cd…` / `009d2479…`.

Focused 1/1 and full sibling 26/26 (152.74s) pass. Formatting, strict
all-target/all-feature workspace Clippy (13.00s), deterministic Java replay,
oracle shell syntax, and diff checks pass. `425d58e82` remains the documented
exact green remote CI baseline pending newer terminal evidence. Continue at
Cucaracha system 1 phase-two queue 10 x42/SIG73's real append.

## Boundary 196: identity-aligned x42 append and six prelinked returns

Native queue 10 x42/SIG73 selects LEFT/BOTTOM and resolves glyph42 to Stem
identity23 / vertex217. It preserves x39/SIG91 edge257 and x49/SIG117
edge258, appends one current-head relation, and changes no vertex, stem,
glyph-ID, or allocator state. Its native relation grade/dx bits are pinned
independently from Java's nearly identical geometry.

Native queues 11-16 (x133, x58, x125, x138, x48, x17) all run through the
generic prelinked path: LEFT is linked/closed, neither RIGHT corner can link,
the retry returns true, and closure traversal changes no values or graph
objects. Production advances directly to the next real append at queue 17
x68/SIG76.

Java queue index9 is the identity-aligned x42/SIG73/Inter1091 transaction. It
selects glyph200/Stem2201, preserves Inter1127 and Inter1181, and adds exactly
one edge (339→340). Java queue indices10-15 confirm the following six no-op
returns. Queue16 x68 is deliberately excluded for the next boundary.

The 13-row-plus-summary fixture is 16 lines / 9,639 bytes and is
byte-identical across warmup plus two fresh runs. Runner/retarget-transform/
fixture/body+semantic SHA-256 values are
`ff8c906f2b6f33316f48e21b16a2fcdf0b2cdd8583c4e210b45d6e8c1132fbe6`,
`aa8a4c501a0daf54bf3c09ce0ee202574cdd90673e1b369d5e59d3e5128ed819`,
`614570efcd4a9471ef6692552c9c116b304d24c7171c1e407b0edd5e8710730a`,
and
`f88e42d5a3b6b6bcbe100f044f21e0a9a3a44bd6445e7c78acc2297487574cd6`.
Boundary 195's runner/fixture/transform are pinned at `e1fcae89…` /
`475c4346…` / `5722bbdc…`.

Focused 1/1 and full sibling 26/26 (153.09s) pass. Formatting, strict
all-target/all-feature workspace Clippy (13.41s), deterministic Java replay,
oracle shell syntax, and diff checks pass. `425d58e82` remains the documented
exact green remote CI baseline pending newer terminal evidence. Continue at
Cucaracha system 1 phase-two queue 17 x68/SIG76's real append.

## Boundary 197: aligned x68 append and x31 prelinked return

Native queue 17 x68/SIG76 selects LEFT/BOTTOM, resolves glyph40 to Stem
identity30 / vertex224, preserves x70/SIG105 edge283 and x74/SIG120 edge284,
and appends one current-head relation. Vertices, system stems, glyph IDs, and
allocator state remain unchanged. Queue 18 x31/SIG114 then runs through the
generic prelinked path with zero graph and closure-value changes. Production
reaches queue 19 x14/SIG58.

Java queue index16 is the aligned x68/SIG76/Inter1097 transaction. It selects
glyph198/Stem2208, preserves Inter1155 and Inter1187, and changes only edges
340→341. Java queue17 confirms x31/SIG114/Inter1175's prelinked no-op. The
native relation's exact low-bit geometry remains an independent assertion.

The 8-row-plus-summary fixture is 11 lines / 7,175 bytes and is
byte-identical across warmup plus two fresh runs. Runner/retarget-transform/
fixture/body+semantic SHA-256 values are
`77a6d85e5323fa62806e9e5ddc3b3a9dcb9a1817a1ae179f9625e175de0e9822`,
`b64a7aaebb60629858847a3cdd7a94d21a967e5f978f03607e7ff6b6747938d2`,
`19b0a62c21cb2fb5dae5f7e923d67b6e0d18433cbef05920aa2eef98cae3fcef`,
and
`0296416a0c7732e1729e75aafccbe3a522b74813f4dd0f5ece82cbdfa20d0d4d`.
Boundary 196's runner/fixture/transform are pinned at `ff8c906f…` /
`614570ef…` / `aa8a4c50…`.

Focused 1/1 and full sibling 26/26 (153.38s) pass. Formatting, strict
all-target/all-feature workspace Clippy (13.16s), deterministic Java replay,
oracle shell syntax, and diff checks pass. `425d58e82` remains the documented
exact green remote CI baseline pending newer terminal evidence. Continue at
Cucaracha system 1 phase-two queue 19 x14/SIG58's real append.

## Boundary 198: aligned x14 append

Native queue 19 x14/SIG58 selects LEFT/BOTTOM, resolves glyph41 to Stem
identity32 / vertex226, and appends exactly one current-head relation. The
existing x8/SIG89 edge319, x13/SIG101 edge320, and x17/SIG112 edge321 remain
ordered and unchanged; vertex, system-stem, glyph-ID, and allocator state do
not change. Production advances to the next fail-closed frontier at queue 20
x45/SIG62.

Java queue index18 independently measures x14/SIG58/Inter1061 selecting
glyph199/Stem2210, preserving Inter1123, Inter1147, and Inter1171, and changing
only edges 341→342. The native relation grade/dx bits
`3fe8362324f5276f` / `3fb5e15152b5f6db` are asserted directly.

The 7-row-plus-summary fixture is 10 lines / 6,704 bytes and is byte-identical
across warmup plus two fresh runs. Runner/retarget-transform/fixture/
body+semantic SHA-256 values are
`eb79eb1de1d4570e4f7b976006c6d14134aa6bf32fbe1de156c24bd7972762ec`,
`06095681e521b777c988acb90a562ac2941c9e8ef335fea00b952443aba4c08f`,
`8363a188fdf9d3f32b2bea7545f44c6025cb9228aa1c7c2935023e865d1e232d`,
and
`8c7933fa714d698c0dab4bb11b21faf5f3684e24b58159cf924fe1ae82e5ada1`.
Boundary 197's runner/fixture/transform are pinned at `77a6d85e…` /
`19b0a62c…` / `b64a7aae…`.

Focused 1/1 (3.79s) and full sibling 26/26 (151.75s) pass. Formatting,
strict all-target/all-feature workspace Clippy (13.29s), deterministic Java
replay, oracle shell syntax, and diff checks pass. `425d58e82` remains the
documented exact green remote CI baseline pending newer terminal evidence.
Continue at Cucaracha system 1 phase-two queue 20 x45/SIG62's real append.

## Boundary 199: aligned x45 append and x56 prelinked return

Native queue 20 x45/SIG62 selects LEFT/BOTTOM, resolves glyph42 to existing
Stem identity23 / vertex217, preserves x43/SIG103 edge323 and x48/SIG116
edge324, and appends one current relation. No vertex, system-stem, glyph-ID,
or allocator state changes. Native independently computes relation grade/dx
bits `3fe6918be20e8fdc` / `3fba18036d0d0f3d`. Queue 21 x56/SIG82 then
returns true through the generic prelinked path with zero graph or closure
value changes. The transactional page driver now reaches the next real append
at fail-closed queue 22 x71/SIG66.

Java queue19 measures the aligned x45/SIG62/Inter1069 append through
glyph200/Stem2201, retains Inter1151 and Inter1179, and changes edges 342→343.
Its independent relation bits are `3fe6918be20e8d71` /
`3fba18036d0d1555`. Java queue20 confirms x56/SIG82/Inter1109's prelinked
no-op.

The strict fixture is 11 lines / 6,787 bytes with eight semantic rows plus
summary and is byte-identical across warmup plus two fresh runs. Runner,
transform, fixture, and body/semantic hashes are
`29733c6d93a1d5642d24cfe742b9d3f9314230818ca5919acd1a5b21552e74a7`,
`4b3029fec45ef99cdd24804ec7e88ac04578a62f2e1e71e127088ee5554c56ba`,
`59f27d582bda0a3a144a68b5dc37a0ac586ad89de19c64d993aed15cfdbed2c4`,
and `37c5c7ddd68f9a923e132ccc62fe834a720006e106143ad97b4260c63e3cb791`.
Boundary 198's runner/fixture/transform remain strictly pinned at
`eb79eb1d…` / `8363a188…` / `06095681…`.

Focused 1/1 (3.83s), full sibling 26/26 (150.63s), formatting, strict
all-target/all-feature workspace Clippy (13.35s), deterministic Java replay,
oracle syntax, and diff checks pass. `425d58e82` remains the documented exact
green remote baseline pending newer terminal CI. Continue at Cucaracha system
1 phase-two queue 22 x71/SIG66's real append.

## Boundary 200: Cucaracha system-one phase-two completion

Native queue22 x71/SIG66 selects LEFT/BOTTOM, resolves glyph40 to existing
Stem identity30 / vertex224, preserves x70/SIG105 edge283 and x74/SIG120
edge284, and appends exactly one current-head relation. Native relation
grade/dx bits are independently pinned at `3fe5554e97cdff05` /
`3fbd29be97edf9e8`. No vertex, system-stem, glyph-ID, or allocator state
changes. The continuation returns true and advances phase-two index22→23,
equal to the 23-item queue length and therefore completing Cucaracha system 1
phase 2. The page driver now fails closed at Cucaracha system 2 phase-two
queue8 x56/SIG78's real `reuseStem` append.

Java queue21 independently measures x71/SIG66/Inter1077 through
glyph198/Stem2208, retains Inter1155 and Inter1187, and changes edges343→344
without changing vertices232, system stems38, or allocator2216. Java's
independent relation grade/dx bits are `3fe5554e97ce0182` /
`3fbd29be97edf3cf`.

The strict seven-row-plus-summary fixture is 10 lines / 6,284 bytes and is
byte-identical across warmup plus two fresh runs. Runner/transform/fixture/
body+semantic hashes are
`3ad18d6e2db7b60980a27deef414bf54ac86df1fdfc127b26539172b4665e918`,
`a9daae9d492b63c9b9e091f0522bf7e42d270ef113a6f63f5a323066764c0d01`,
`457f8f28ca9a62fd085b27d5e574b1ff71a9f2f211dec9a0a82d4c30432c20d5`,
and `5ce49912b802895b8c9c549ef8b08c92c08f6a8942b6d0bd02f8c3f4a2d12f94`.
Boundary 199's runner/fixture/transform remain pinned at `29733c6d…` /
`59f27d58…` / `4b3029fe…`.

Focused 1/1 and full sibling 26/26 (153.52s) pass. Formatting, strict
all-target/all-feature workspace Clippy (13.32s), deterministic Java replay,
oracle syntax, and diff checks pass. `425d58e82` remains the documented exact
green remote CI baseline pending newer terminal evidence. Continue at
Cucaracha system 2 phase-two queue8 x56/SIG78's real append.

## Boundary 201: Cucaracha system-two phase-two queue 8

Native queue8 x56/SIG78 selects LEFT/BOTTOM after both bottom corners pass,
resolves glyph92 to existing Stem identity30 / vertex242, preserves
x67/SIG119 edge261, and appends exactly one current-head relation. The later
RIGHT/BOTTOM expansion returns `-1`. Native relation grade/dx bits are
`3feb7adfb837fb8d` / `bfbae2955082830c`, exactly matching Java. No vertex,
system-stem, glyph-ID, or allocator state changes. The continuation returns
true and advances system 2's phase-two index8→9. Production next fails closed
at queue9 x132/SIG84's real `reuseStem` append.

Java queue8 independently measures x56/SIG78/Inter1388 selecting glyphs250
and 2487, canonical candidate250, and existing Stem2647. It retains
Inter1471, changes only edges347→348, and keeps vertices255, system stems43,
and allocator2659 fixed.

The strict eight-row-plus-summary fixture is 11 lines / 6,012 bytes and is
byte-identical across warmup plus two fresh runs. Runner/transform/fixture/
body+semantic hashes are
`e862cb9e24ca33a0f9381b1990b25a3a59c607337b60720930871b93936e5b7d`,
`3f696415a4450338b60c29d343aaccd7ba88772868abaf2deac3ea1c46272cbf`,
`5290a3261024d312098f1671c536df2bf2e89721e9b6713574c25d95107a58b5`,
and `71543efca6a7a47a0d0ba1339273402d0b2495f6f0c6ac88fce86716d2a9bef7`.
Boundary 200's runner/fixture/transform remain pinned at `3ad18d6e…` /
`457f8f28…` / `a9daae9d…`.

Focused 1/1 (3.83s), full sibling 26/26 (152.25s), formatting, strict
all-target/all-feature workspace Clippy (13.52s), deterministic Java replay,
oracle syntax, and diff checks pass. `425d58e82` remains the documented exact
green remote CI baseline pending newer terminal evidence. Continue at
Cucaracha system 2 phase-two queue9 x132/SIG84's real append.

## Boundary 202: Cucaracha system-two phase-two queue 9

Native queue9 x132/SIG84 selects LEFT/BOTTOM, resolves glyph93 to existing
Stem identity35 / vertex247, preserves x129/SIG103 edge268 and x139/SIG125
edge269, and appends one current-head relation. Relation grade/dx bits are
`3fed051e7bce623f` / `bfb22f195fe0a492`, exactly matching Java. No vertex,
system-stem, glyph-ID, or allocator mutation occurs. The continuation returns
true, advances index9→10, and exposes queue10 x84/SIG80's real `reuseStem`
append.

Java queue9 independently measures x132/SIG84/Inter1400 selecting glyph251
and existing Stem2652, retaining Inter1438 and Inter1483, and changing only
edges348→349 while vertices255, system stems43, and allocator2659 stay fixed.

The strict seven-row-plus-summary fixture is 10 lines / 6,314 bytes and is
byte-identical across warmup plus two fresh runs. Runner/transform/fixture/
body+semantic hashes are
`d1e2a3dd39c1f2f73b8ffc7d907e5361f33bbbd57a7dbf3ad68e3cc11ae0973c`,
`af763c75140add0f67a9ccb3b077797fdf7c640c5b80a122697de63f5beeb0a2`,
`e7d97fbf829b52730dfdf4f219a0a7fd87cde3a8f7f8f301c788746492529f01`,
and `fcfef4137dad57cfd43d5c6c48bf71497cf78094f57169242449241cde725e4f`.
Boundary 201's runner/fixture/transform remain pinned at `e862cb9e…` /
`5290a326…` / `3f696415…`.

Focused 1/1 (3.80s), full sibling 26/26 (153.99s), formatting, strict
all-target/all-feature workspace Clippy (13.68s), deterministic Java replay,
oracle syntax, and diff checks pass. `425d58e82` remains the documented exact
green remote CI baseline. Continue at Cucaracha system 2 phase-two queue10
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
body+semantic hashes are
`8b260716910454740347bf55952f5a31ece6f089528e59871947f6611a096160`,
`3d076bd7c6ff7e43145545af6969a36b2c415ac4067a317c6f169735c28639e0`,
`cb394f3b37eade0450ba44bc44ecb3db96d52e415745fd73e0576f3a7aa6cf06`,
and `8448857c730bea286818298f9a883235fd7073b7114d01bfa4fb930aa4053fef`.
Boundary 202's runner/fixture/transform remain pinned at `d1e2a3dd…` /
`e7d97fbf…` / `af763c75…`.

Focused 1/1 (3.73s), full sibling 26/26 (152.87s), formatting, strict
all-target/all-feature workspace Clippy (13.63s), deterministic Java replay,
oracle syntax, and diff checks pass. `425d58e82` remains the exact green remote
CI baseline. Continue at Cucaracha system 2 phase-two queue16 x109/SIG81.

## Boundary 204: Cucaracha system-two phase-two queue 16

Native queue16 x109/SIG81 selects LEFT/BOTTOM, resolves glyph95 to existing
Stem identity37 / vertex249, preserves x111/SIG110 edge282 and x114/SIG122
edge283, and appends one current-head relation. The later RIGHT/BOTTOM
expansion returns `-1`. Relation grade/dx bits `3fef148d14458919` /
`bf9734df7f4c3cf4` exactly match Java. No vertex, system-stem, glyph-ID, or
allocator mutation occurs. The continuation advances index16→17; generic
queues17-23 complete system 2, and generic system-3 queues0-18 expose queue19
x37/SIG11's real `reuseStem` append.

Java queue16 independently measures x109/SIG81/Inter1394 selecting glyphs253
and 2575, candidate253, and existing Stem2654. It retains Inter1453 and
Inter1477, changes only edges350→351, and keeps vertices255, system stems43,
and allocator2659 fixed.

The strict seven-row-plus-summary fixture is 10 lines / 6,358 bytes and is
byte-identical across warmup plus two fresh runs. Runner/transform/fixture/
body+semantic hashes are
`0307f76f0da438d3609c1dcaa602656eca732de9fd377bd25325e94c78ffea77`,
`bc9205d1e88c653d7d7cb553cc525d559a69e87b4736efe615c975daf82ae425`,
`200afe8ef54faf6a11ecf094bc2394b485dee7f0eb6ed68aa632e4e4bdbbdd5d`,
and `77964df581176281c035325c64ddacb5d73abe745f687134be5291e25062c6ef`.
Boundary 203's runner/fixture/transform remain pinned at `8b260716…` /
`cb394f3b…` / `3d076bd7…`.

Focused 1/1 (3.77s), full sibling 26/26 (153.57s), formatting, strict
all-target/all-feature workspace Clippy (13.88s), deterministic Java replay,
oracle syntax, and diff checks pass. `425d58e82` remains the exact green remote
CI baseline. Continue at Cucaracha system 3 phase-two queue19 x37/SIG11.

## Boundary 205: Cucaracha system-three phase-two completion

Native terminal queue19 x37/SIG11 selects LEFT/BOTTOM, resolves native glyph159
to existing Stem identity13 / vertex177, preserves x32/SIG49 edge207, and
appends one current-head relation. Relation grade/dx bits
`3fe4e1c61700dadc` / `3fbe433d3ee06618` exactly match Java. No vertex,
system-stem, glyph-ID, or allocator mutation occurs; edges250→251 and the
phase-two cursor advances 19→20, equal to the carried queue length.

Java independently measures x37/SIG11/Inter1555 selecting glyphs317 and 2868,
candidate317, and existing Stem2989. It retains Inter1632, changes only
edges250→251, and keeps vertices198, system stems34, and allocator3009 fixed.
The carried native identities are deliberately native rather than copied from
those Java EntityIndex values.

This terminal closes all three Cucaracha phase-two queues. Generic
`finalizeStems` checks 142/150/113 heads with zero relation removals or
abnormal-value changes, and transactional `recognize_native_stems` reproduces
the same prepared components and finalized systems. Cucaracha is therefore
transactionally complete end to end, though it is not added to the narrower
schema-1 publication corpus by this boundary.

The strict seven-row-plus-summary fixture is 10 lines / 5,826 bytes and is
byte-identical across warmup plus two fresh runs. Runner/transform/fixture/
body+semantic hashes are
`26af234811b815d1e2012311838045cd80adec4c3d67c3dd19c732160600fb34`,
`35f69316834081b0e6f8354e0bfbb856952930941652ccd04db2ee23dcc1d432`,
`a4ede84ed937da65006924da3b3de35e24d33dd229d9391aae136e436b1477ff`,
and `81451bfd11189860d64e970ab4a81714b1a3ff7cfddfac1ef8c10f1e6f5fe74c`.
Boundary 204's runner/fixture/transform remain pinned at `0307f76f…` /
`200afe8e…` / `bc9205d1…`.

Focused 1/1 (4.38s), full sibling 26/26 (154.39s), formatting, strict
all-target/all-feature workspace Clippy (13.75s), deterministic Java replay,
oracle syntax, and diff checks pass. `425d58e82` remains the exact green remote
CI baseline. Continue with the first unsupported transactional STEMS frontier
among Hove and BachInvention5.

## Boundary 206: Hove system-five phase-two completion

Native terminal queue1 x67/SIG52 selects RIGHT/TOP, resolves native glyph226
to existing Stem identity25 / vertex128, preserves x65/SIG46 edge143, and
appends the current-head relation at edge159. Relation grade/dx bits
`3fefab115e072942` / `3f6fc4514038cccd` exactly match Java. Vertices136,
system stems32, glyph IDs, and allocator2937 remain fixed; edges advance
159→160 and the phase-two cursor advances 1→2, exhausting system 5.

Java independently measures x67/SIG52/Inter1721 selecting glyph284 and
existing Stem2931. It retains Inter1709 and changes only the one edge and the
x67 RIGHT side flag. Native identities remain independently derived from
carried Rust state rather than copied from Java's EntityIndex.

This terminal closes all five Hove phase-two queues. Generic `finalizeStems`
checks 65/90/52/65/71 heads with zero relation removals or abnormal-value
changes, transactional `recognize_native_stems` reproduces the same prepared
components and finalized systems, and production `-step STEMS -json` succeeds
for Hove.

The strict five-row-plus-summary fixture is 8 lines / 3,627 bytes and is
byte-identical across warmup plus two fresh runs. Runner/transform/fixture/
body+semantic hashes are
`e4af37df9ef194bf2da94d05101f452384144dd5ffbe5856f35fe5aebb179547`,
`2f54cd2e91e0d930912e7decc1d7222512918b0a14103010e9fa2dee05762eeb`,
`b3b6f9f88e158793eec8072c2f8aee1ebb9508acf5b908965651015c4d10d341`,
and `0078c65201a8b8b426beaf4cee7ad67928fb1b5252e15b46108b2b5486753e71`.
Boundary 205's runner/fixture/transform remain pinned at
`26af234811b815d1e2012311838045cd80adec4c3d67c3dd19c732160600fb34`,
`a4ede84ed937da65006924da3b3de35e24d33dd229d9391aae136e436b1477ff`,
and `35f69316834081b0e6f8354e0bfbb856952930941652ccd04db2ee23dcc1d432`.

Focused 1/1 (3.86s), full sibling 27/27 (156.52s), formatting, strict
all-target/all-feature workspace Clippy (13.45s), deterministic Java replay,
oracle syntax, and diff checks pass. `425d58e82` remains the exact green remote
CI baseline. Continue at BachInvention5 system 6's missing carried BEAMS
groups, the first remaining unsupported transactional STEMS frontier.

## Boundary 207: preserve pre-rest beam-group identity into native SIG

The Bach system-6 failure was a lifecycle mismatch, not missing geometry.
Java's `BeamsBuilder.buildBeams()` creates `BeamGroupInter` containment before
`BeamsStep` invokes `MultipleRestsBuilder`; deleting the rest-like beam removes
that vertex and its incident containment, but does not regroup the survivors.
Native BEAMS already retained the correct pre-rest group evidence, while
`append_beams` incorrectly reran geometric grouping on the compact post-rest
beam stream and rejected system 6.

Native SIG assembly now replays the authenticated pre-rest group event stream,
maps each source identity onto the live post-rest vertex stream, omits only the
retired member and its incident relations, preserves surviving group vertices,
containment and BeamBeam relations, and removes a group only if its sole member
was retired. Bach raw beam ordinal182 is system-local member23 of pre-rest
group `[18,23]`; the live group keeps member18 instead of being recomputed into
the different post-rest partition.

The focused MultipleRest gate pins that pre/post grouping difference. Existing
Java-backed competitor, native-SIG, reachability, stump, and V-linker corpora
all pass unchanged; in particular the two affected Bach stump rows retain
`groupMembers 2`. Production Bach now passes BEAMS/SIG assembly and stops at
the first real STEMS gap in system 1: a rather-good unlinked head requiring the
higher-profile retry branch.

Focused MultipleRest 1/1, HEADS competitors 2/2, small-beam epilog 6/6,
STEM_SEEDS/BEAMS 4/4, native SIG 10/10 (five explicit diagnostics ignored),
and the three downstream Java corpus gates pass. Formatting, strict
all-target/all-feature workspace Clippy (11.88s), and diff checks are clean.
No oracle changed: existing Java fixtures detected and grade this lifecycle
fix. `425d58e82` remains the exact green remote CI baseline. Continue at Bach
system 1's higher-profile rather-good unlinked-head retry.

## Boundary 208: generic phase-one rather-good profile retry

Java recursively calls `HeadLinker.linkSides` with stem profiles 1, 2, and 3
when STRICT profile 0 leaves a rather-good head unlinked. The generic native
phase-one continuation now preserves that control flow. Every profile appends
its ordered LEFT/RIGHT decisions, linked and closed sides retain their skip
semantics, the dual-corner shared-stump branch still records an undefined side,
and a later successful profile becomes the authenticated C-link frontier.
Only exhaustion of profiles 0 through 3 closes both local S cells and queues
the head for phase 2.

The full-lifecycle Bach system-1 fixture measures queue37 x3/SIG95/Inter3599
at grade bits `3fdcd6c4146e1fa4`. Both sides are `Neither` at every profile.
Java returns false, records no undefined side, closes x3 LEFT then RIGHT, and
advances to x44/SIG36/Inter3481 with SIG 216/257, system stems37, and allocator
all unchanged. Native derives the same result from production GRID, HEADERS,
STEM_SEEDS, BEAMS, LEDGERS, HEADS, SIG, and builder state.

Fixture/runner/probe/init/body SHA-256 values are
`2964eb04060e03a97db6c44cd8de3cc383a59a082b9f56524290c3181aacafaa`,
`8edea3da64b607b16ccf5a30191d6c14429c3106b9aa8e263e4f6ea24e913d61`,
`f71177c81db91fb46ec392f53f854dbc37ceb05dd4e50ad3d3ef315d2d380772`,
`a2b5123237974823bf131d3e17ef8c27035062c00e9bfe15aeb9b17ce8a324df`,
and `8efab31e3192446991f12e3e2587ad565f8a7c5b30d194e626ec10b7a019e51c`.
Warmup plus two fresh JVM runs are byte-identical. Focused 1/1, full sibling
28/28 (156.94s), formatting, strict all-target/all-feature workspace Clippy
(14.08s), deterministic replay, and diff checks pass. `425d58e82` remains the
exact green remote CI baseline. Bach system 1 now completes; continue at
system 2 phase-one queue182 x138/SIG149, where STRICT profile 0 selects
LEFT/BOTTOM and the builder contains the start head plus two concrete
BeamLinker stumps.

## Boundary 209: reuse a concrete multi-beam stump without duplicate edges

The generic native C-link transaction now consumes concrete `BeamLinker`
stump items instead of accepting only target-only beam placeholders. Seed
stumps resolve through the carried free-glyph table, built stumps resolve
through the unique native pre-builder registry event, and each BeamStem
relation is evaluated against the evolving stem line immediately after its
own item, matching Java's builder order. A unique already-present BeamStem
edge is authenticated rather than duplicated; its B cell and scheduler flag
must agree and remain open, while an absent relation still follows the
existing append-and-link path.

The full-lifecycle Bach system-2 oracle authenticates queue182
x138/SIG149/Inter3906 at STRICT stem profile 0. LEFT/BOTTOM expands the start
head plus beam SIG27/B3 and SIG31/B3. All three items share glyph
`g:1258:902:4:51:914abffc6b78ac27eb996e0ab3a118a381eee00ebff10ecd7bd1b661842e2b06`;
the two BeamStem relations are already present, linked, open, grade 1, and
`CENTER`. Java reuses the existing candidate Stem, adds only the HeadStem
edge from x138, leaves 394 vertices / 77 system stems / allocator unchanged,
advances edges 592→593, closes native x140/SIG141 LEFT then RIGHT, and reaches
queue183 x62/SIG99/Inter3804. Native reproduces the same structural mutation
without importing Java's process-local Stem or allocator identities.

Fixture/runner/probe/init/body SHA-256 values are
`7b84be8e57253846336ad1463745b998ecf97e3b55b20ec3dbefbd5ce790f760`,
`b1e40651458dec4914e89b53fadbb1ac9406cdea4dd988af27c9df8cd869b817`,
`72e85d0de1838664db221fa890917b83a1140bf6ee5ea99b0a1f6bc1839fec33`,
`3140eec01b976a5cf934183c37ef07528bacc874abe67a0491f409505daf888b`,
and `79c38429801cea5f11a2c9c5a241aba636603500b946c0dd6d9cc84b20625dad`.
Warmup plus two fresh JVM runs are byte-identical. Focused 1/1, full sibling
29/29 (157.13s), formatting, strict all-target/all-feature workspace Clippy
(13.62s), deterministic replay, and diff checks pass. `425d58e82` remains the
exact green remote CI baseline. Continue at Bach system 2 queue183
x62/SIG99; its branch and builder geometry remain to be measured.

## Boundary 210: identity-free four-head prelinked reconciliation

The full-lifecycle Bach system-2 oracle now measures queue183
x62/SIG99/Inter3804 at grade bits `3fc88ee23b88ee24`. LEFT is already linked
and RIGHT already closed at all profiles 0-3. One pre-existing incident stem
structurally joins x59/SIG113, x62/SIG99, x63/SIG100, and x65/SIG196 on LEFT.
Java returns true, records no undefined side or changed S-cell, and preserves
394 vertices, 593 edges, 77 system stems, and the allocator.

The generic native phase-one continuation writes x59 LEFT/RIGHT, x63
LEFT/RIGHT, and x65 LEFT/RIGHT in that order. All six cells were already true,
so `closed_value_changes=0`; the carrier advances to queue184
x25/SIG93/Inter3790. The frozen semantic rows intentionally name only x/SIG
ordinals and `existingStem`, not Java's process-local StemInter or Inter IDs.

Fixture/runner/probe/init/body SHA-256 values are
`079e8b4995e8610c5eda9370624d93a3e9262f15e2cb5eebf4f2159250974f75`,
`ac697b86954010c94de4e7767e12d6e80bd79306a0f6f3e8d8c80fa733cda5fe`,
`05c2ff1c14f4f2284ffb80560c82fce4b66c5d41f8debc21e2f5d91fe910a7bb`,
`c799ce83ebcffad237d9037f63bfe0b1f092798e54142ed25c75b263af1074d3`,
and `1bae18ca1122bb13623be12eaec05a64720233c156dd8a4ff09b8c519750e793`.
The runner also pins Boundary 209's runner and fixture. Warmup plus two fresh
JVM runs are byte-identical. Focused 1/1, full sibling 29/29 (156.79s),
formatting, strict all-target/all-feature workspace Clippy (9.44s), oracle
replay, and diff checks pass. No production source change was needed.
`425d58e82` remains the exact green remote CI baseline. Continue at Bach system
2 queue184 x25/SIG93; its branch and builder geometry remain to be measured.

## Boundary 211: transformed four-head mixed-change reconciliation

Bach system-2 queue184 is x25/SIG93/Inter3790, grade bits
`3fc87c4777a649dd`. LEFT is already linked and RIGHT closed across profiles
0-3. Its pre-existing stem structurally joins x25/SIG93, x27/SIG178,
x28/SIG179, and x29/SIG92 on LEFT. Java returns true and changes only x28
LEFT from linked/open to linked/closed and x28 RIGHT from unlinked/open to
unlinked/closed; SIG 394/593, 77 stems, and the allocator do not change.

The generic native continuation preserves the incident edge order rather than
the sorted structural display: x29 LEFT/RIGHT, x27 LEFT/RIGHT, then x28
LEFT/RIGHT. The first four writes are idempotent, the last two change values,
and the carrier advances to queue185 x192/SIG76/Inter3757.

Boundary 211 derives its probe mechanically from Boundary 210's identity-free
source. Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`7a77078895e488d1be44d0f57c272d0d022fc278c86ba13d94925f8ff111aebe`,
`16c64b513e86df490b141cfa6189d3f80ac76c18ea483ae1d4d81325a2a3b805`,
`64514c7fc90e30ee745f02628a9a44461d175477ba93c8a80bb158fdb9d499e3`,
`3787d760a4a9f6fadd552910ff4876a38990d59625e9fa405c453bf6b918350e`,
`66f7873e1eaaef9ff5504ec23e561eb1c015fc5756f36c0220c69f590127e648`,
and `8ef60ed510ea962fde3199051794cdcdaae5d12c3d59ac367fe2bfef65696a74`.
The Boundary 210 source, runner, and fixture are exact predecessor pins.
Warmup plus two fresh JVM runs are byte-identical. Focused 1/1, full sibling
29/29 (153.12s), formatting, strict all-target/all-feature workspace Clippy
(8.87s), replay, and diff checks pass. No production source changed.
`425d58e82` remains the exact green remote CI baseline. Continue at Bach system
2 queue185 x192/SIG76; its branch and builder geometry remain to be measured.

## Boundary 212: transformed three-head zero-change reconciliation

Bach system-2 queue185 x192/SIG76/Inter3757 has grade bits
`3fc861861861861a`. LEFT is already linked and RIGHT closed at profiles 0-3.
Its existing stem structurally joins x191/SIG75, x192/SIG76, and x193/SIG77
on LEFT. Java returns true without undefined sides or cell changes; 394
vertices, 593 edges, 77 system stems, and the allocator stay fixed.

Native emits x191 LEFT/RIGHT then x193 LEFT/RIGHT in relation order. All four
writes are idempotent, `closed_value_changes=0`, and the carrier advances to
queue186 x190/SIG214/Inter4036.

The queue-185 probe is a mechanical transform of Boundary 210's identity-free
source and pins Boundary 211 as its predecessor. Fixture/runner/transform/
transformed-probe/init/body SHA-256 values are
`bba0a8a3a80a6bb1d5693fb3cdb6a1764e798e9c3ca34000a08b78a8f2b386b7`,
`5d15aa20ae4a7282b059dd3d6cd556c248be8b9f532739d66c5ad2b57cfe8c09`,
`61bd7b3e2aff7418a034cff7b70453dd1db180d59ee3731f07b5f60044798dc7`,
`a8b102ab3485a79d5def994540b6401d3a6bdbffa946f13f3ff52514cd050057`,
`568926dc325d8e9633ec3df663466df5ca14109725a35ef9ca5060e988069d13`,
and `9d3ea66878524b64a58a764370915fc0ae64de4ca171a25ec33952e6489b9834`.
Warmup plus two fresh JVM runs are byte-identical. Focused 1/1, full sibling
29/29 (148.07s), formatting, strict all-target/all-feature workspace Clippy
(8.51s), replay, and diff checks pass. No production source changed.
`425d58e82` remains the exact green remote CI baseline. Continue at Bach system
2 queue186 x190/SIG214; its branch and builder geometry remain to be measured.

## Boundary 213: identity-free no-link closure

Bach system-2 queue186 x190/SIG214/Inter4036 has grade bits
`3fc857b6c55b3c0d`. Neither side can link at profiles 0-3. Java returns false,
records no undefined head or incident stem, closes x190 LEFT then RIGHT, and
preserves 394 vertices, 593 edges, 77 system stems, and the allocator. Native's
generic continuation evaluates the single operational profile for this grade,
reproduces the two ordered closures with `closed_value_changes=2`, and advances
to queue187 x178/SIG52/Inter3709 without graph mutation.

The queue-186 probe is a mechanical transform of Boundary 210's identity-free
source and pins Boundary 212 as its predecessor. Fixture/runner/transform/
transformed-probe/init/body SHA-256 values are
`729145d6ecd237c7cf420323f980384e119efac24eed97a2393bc1a91dbba8b9`,
`38b6854c8a1a58cc4e463f119bf60317a5fc4501cc22bd21c091850e3cb9558a`,
`ab01e72ce28d279aa95fa66d5c0e0f86533e8d9f8ba058fcfa9a20ea3e1b9dc0`,
`f0c4689aeee121c8e74e565fa92c40ab38827197a986a02c44080503757177ac`,
`4b36fba6bab07e37401f56e1652f6d97b38aff7ce99ababab60ff874388c673d`,
and `e5b83dc66a534e93fb5774e6b74adea3954a8dd81c03e4e89f5d4db3fcc34eff`.
Warmup plus two fresh JVM runs are byte-identical. Focused 1/1, full sibling
29/29 (153.63s), formatting, strict all-target/all-feature workspace Clippy
(9.21s), replay, and diff checks pass. No production source changed.
`425d58e82` remains the exact green remote CI baseline. Continue at Bach system
2 queue187 x178/SIG52; its branch and builder geometry remain to be measured.

## Boundary 214: identity-free existing-stem multi-beam C-link

Bach system-2 queue187 x178/SIG52/Inter3709 has grade bits
`3fc8482d71f2693a`. Java selects LEFT/BOTTOM at STRICT profile. The builder has
three items: the start head and two already-linked beam targets at beam SIG
ordinals 11 and 14, both local B-linker 3. All three items select active glyph
535 with structural content `1565:761:4:51` and reuse its existing stem.

Native structurally resolves the same glyph and stem, preserves both existing
BeamStem edges, and adds only the x178 HeadStem edge. The relation has exact
grade `1.0` and negative-zero `dx`. It closes x181/SIG42 LEFT then RIGHT, adds
two value changes, preserves 394 vertices, 77 stems, and the allocator, moves
the edge count 593 to 594, and advances to queue188 x47/SIG57/Inter3719.

The queue-187 probe is a checked transform of the frozen multi-beam probe and
pins Boundary 213 as its predecessor. Fixture/runner/transform/transformed-
probe/init/body SHA-256 values are
`62acbdbea32f228e829d9b49cec8b795308ab77307aea358091e446daf8820c8`,
`b5f3635b1c364ead19243eb9c25d5388e558ee0ee268e54c63dc7a3c69111fad`,
`5d32102a183990baaa8324575019e8f3e687293da60355e5e4c321462542051f`,
`efcb665ce63d49bc2a3e3c9587e2cedaf65076d2fee2746cbe2d8ee22de6fade`,
`3a83d63f8191f6e9ab734c60793095fe1b8ff85d9580ea934cc7ed7bf1d5a4a2`,
and `95854d88aace78876d736d5352b62f25e8d730c27d7994e36bdea8fffaf0b9de`.
Warmup plus two fresh JVM runs are byte-identical. Focused 1/1, full sibling
29/29 (156.33s), formatting, strict all-target/all-feature workspace Clippy
(10.29s), replay, and diff checks pass. No production source changed.
`425d58e82` remains the exact green remote CI baseline. Continue at Bach system
2 queue188 x47/SIG57; its branch and builder geometry remain to be measured.

## Boundary 215: two-head existing-stem C-link with exact Java line rounding

Bach system-2 queue188 x47/SIG57/Inter3719 has grade bits
`3fc83f6ac882908d`. Java selects LEFT/TOP at STRICT profile. The builder carries
the start head plus crossed head x48/SIG38, both selecting active glyph485 with
structural content `540:722:5:73` and reusing its existing four-head stem.

At this authenticated x47/SIG57 LEFT/TOP corner, Java's translated stem-line x
coordinates are exactly two representable values above the native translation.
The bounded native correction applies `java_next_up` twice before evaluating
the crossed-head relation. It reproduces the main relation grade/dx bits
`3fe7cb9fff0ca1d8`/`bfc6e39073a980f1` and crossed relation grade/dx bits
`3feb43e5758fd513`/`3fab54928678de1e`, then appends both HeadStem edges.

The transaction closes x45, x42, and x48 LEFT/RIGHT in relation order, reports
four changed cells, preserves 394 vertices, 77 stems, and the allocator, moves
the edge count 594 to 596, and advances to queue189 x164/SIG51.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`6aa06fe00a0816367a4cc2586f2edfa33580e9f8ec15b5d757ec92bd5f81e69d`,
`29e80ba56b7f613cda7fbddb567545f45c53042d9a77bab2942d75b6e3388778`,
`2e6e5177fa7e14bb7d0c50706f5752cc7bf7ba2e45e8023ebc53f4ca3a6bb466`,
`1f102875149c3b26cbc17d9f8344c33d3d789bd2093722633e7d8c041e8ac7f9`,
`179202a088b6ed50956a1fa55093e59006080cddeeb46b753cca0c6ca340d045`,
and `3a77205b34b8b4eea8fe6da9404fc37c3531fbaf0dda63339c09eb8d303f4f82`.
Boundary 214 remains the strict predecessor. Warmup plus two fresh JVM runs are
byte-identical. Focused 1/1 and sibling 29/29 (149.20s) pass; formatting,
strict workspace Clippy, replay, and diff checks are green. `425d58e82` remains
the exact green remote CI baseline. Continue at queue189 x164/SIG51.

## Boundary 216: two-head existing-stem reconciliation

Bach system-2 queue189 x164/SIG51/Inter3707 has grade bits
`3fc824ed2e835f84`. LEFT is already linked and RIGHT closed at Java profiles
0-3. Its existing stem structurally joins x164/SIG51 and x167/SIG40 on LEFT.
Java returns true, closes x167 LEFT then RIGHT, changes two cells, and preserves
394 vertices, 596 edges, 77 stems, and the allocator. Native's generic
continuation reproduces the transaction and advances to queue190 x65/SIG196.

The queue-189 probe is a checked identity-free transform of the frozen
queue-183 source with Boundary 215 as strict predecessor. Fixture/runner/
transform/transformed-probe/init/body SHA-256 values are
`a3568828c467de8b7390fb8ee005f8115d8bc79ef9914d20031a8ce3596c5428`,
`4c0fc7f45e4954ae46930f4e6101fa3402603b2c6d7bdef100f7a2b53dfc02ca`,
`20102738ce60feb053653420bc0334a196852d73b785473adfbe54abad7901cd`,
`273f19d5bacdc88f58b84e9944692d8bc65532dd5c0d2e63e31738689fd90e1f`,
`f538824fe7ad158cb9d7b2e2832f67a601c5757c68aa89e807450bab0c15ee9d`,
and `e1e8bd18f83b5e14d08c0794f3f46c0605c38748869f5ee5c887d28ac495ff88`.
Warmup plus two fresh JVM runs are byte-identical. The focused gate passes 1/1;
the sibling suite passes 29/29 (158.00s); formatting, strict workspace Clippy,
replay, and diff checks are green. No production source changed. `425d58e82` remains the exact green
remote CI baseline. Continue at queue190 x65/SIG196.

## Boundary 217: four-head zero-change reconciliation

Bach system-2 queue190 x65/SIG196/Inter4003 has grade bits
`3fc8238122f6952a`. LEFT is already linked and RIGHT closed at profiles 0-3.
Its existing stem structurally joins x59/SIG113, x62/SIG99, x63/SIG100, and
x65/SIG196 on LEFT. Java returns true with no changed cells; native emits the
idempotent x59, x62, and x63 LEFT/RIGHT writes in relation order. Both preserve
the 394/596 SIG, 77 stems, and allocator before queue191 x150/SIG29.

The identity-free queue-183 source is transformed with Boundary 216 strictly
pinned. Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`dec4343b21d65a29cd9552bfbf8a106995bad020006e29ae99f4173439838369`,
`a250dfd71a1af3438fb7d9b82b3715596f3a0e72fbb1a8f01435acf9060e94aa`,
`ee84b4d086e517527dc828095c7b5e6d61e640e431557113ed677d2dc329c54c`,
`e0a865650e1d9d1ffba2495dfcb8b5e8c5ac16cc4166fbb92ac7138228495ffb`,
`3ad32220754d5dabfba4ae091a904dfd7da425a9aa34808a7f3b5c2a96084efd`,
and `d8e3b0c151179534a1a686b56c99a9a9bef867dbba426303835e52220d2b2f8c`.
Warmup plus two fresh JVM runs are byte-identical. Focused 1/1 and sibling
29/29 (154.16s), formatting, strict workspace Clippy, replay, and diff checks pass. No
production source changed. `425d58e82` remains the exact green remote CI
baseline. Continue at queue191 x150/SIG29.

## Boundary 218: following two-head reconciliation

Bach system-2 queue191 x150/SIG29/Inter3663 has grade bits
`3fc7409d669fa2fd`. LEFT is linked and RIGHT closed at profiles 0-3. The
existing stem joins x150/SIG29 and x151/SIG17 on LEFT. Java and native return
true, close x151 LEFT/RIGHT with two value changes, preserve the 394/596 SIG,
77 stems, and allocator, and advance to queue192 x173/SIG160.

The queue-183 source is transformed with Boundary 217 strictly pinned.
Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`33fc783ef4d341d2acfc221a08eb079d320a2050e09155094472113478ab2aeb`,
`2409eb033551846e070d1ef90a0ed7a341ce5a36006fd6f1e3a1deb280ec12de`,
`5187abcaff808f969c1cf620435365f17a0112f094fb9bd6097cbb650183ffbf`,
`42f848877e8eb5eb6a6d116a81fc754d587b2ca7fb1a1deda6aa94e22a898fce`,
`c67527aef2cbd6d6ec540202c6b5f0ac798d45f57a585e732befce9714636098`,
and `b97ded46c51880af3500bac1287fafd3977aad072dbf529908b1362da920dd75`.
Warmup plus two fresh JVM runs are byte-identical. Focused 1/1 and sibling
29/29 (156.78s), formatting, strict workspace Clippy, replay, and diff checks pass. No
production source changed. `425d58e82` remains the exact green remote CI
baseline. Continue at queue192 x173/SIG160.

## Boundary 219: right-side zero-change reconciliation

Bach system-2 queue192 x173/SIG160/Inter3931 has grade bits
`3fc7376bd270bc10`. LEFT is closed and RIGHT already linked across profiles
0-3. Its existing stem joins x170/SIG165, x171/SIG166, and x173/SIG160 on
RIGHT. Java returns true without value changes; native emits the idempotent
x170 then x171 LEFT/RIGHT writes, preserves the 394/596 SIG, 77 stems, and
allocator, and advances to queue193 x27/SIG178.

The queue-183 source is transformed with Boundary 218 strictly pinned.
Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`2f16f58f978732969374cd98cf373abf0aafe465b06446fefb346a5c20bec1ea`,
`6bcfa2d04b9cb77e564e1be8e33fd143bac975b1f425c1e5d4bdd60bf1739caf`,
`ebeaa341807699e4d490b90279bcaccbd4bcf48babdb0008773743a0d9e22ef4`,
`946f543331a6a09ef701e834606e1ec5c405296ed69b959c89ed432802c1c484`,
`994a1f57b02b044cd3c224bb39f7038dadeafdc06b46ac7b1cc2f40ba37aeef8`,
and `08e9bfa02f9646c243bee05096843c899f1c7f4ccdcbdadfabbebc33b9dfd12c`.
Warmup plus two fresh JVM runs are byte-identical. Focused 1/1 and sibling
29/29 (152.70s), formatting, strict workspace Clippy, replay, and diff checks pass. No
production source changed. `425d58e82` remains the exact green remote CI
baseline. Continue at queue193 x27/SIG178.

## Boundary 220: repeated four-head zero-change reconciliation

Bach system-2 queue193 x27/SIG178/Inter3967 has grade bits
`3fc7297297297297`. LEFT is linked and RIGHT closed at profiles 0-3. Its
existing stem joins x25/SIG93, x27/SIG178, x28/SIG179, and x29/SIG92. Java
changes no cells; native emits x29, x25, then x28 LEFT/RIGHT in relation order.
The 394/596 SIG, 77 stems, and allocator remain fixed before queue194
x16/SIG184. Fixture/runner/transform/probe/init/body hashes are
`47e2f14e4393fd18cf840427152faa783527a3714c5aef0576d116b5aa69a726`,
`c976c0d9297c4ff03f900391cac20b2c22a9c306371553e3e051e44c44a44bac`,
`86581b47c885bdac9e62d9304c4f64e4183ade6be242e23a495268edc161e4ae`,
`a69389fe8adfabddd7a6fb91fb4bdab16c98dd5ebfe7e43a58dceb6a2fd86d30`,
`d2c36888f850a0c0145ae2eccb1727c310c19c85db2501706f0e0580f401eb86`,
and `45c66483e7f9dd860de1ddd03959b1133046b697d164697d4a25f263577703a0`.
Warmup plus two JVM runs are byte-identical. Focused, sibling, formatting,
strict Clippy, replay, and diff checks pass. No production source changed.
Continue at queue194 x16/SIG184.

## Boundary 221: three-head zero-change reconciliation

Bach system-2 queue194 x16/SIG184/Inter3979 has grade bits
`3fc72898e7071d70`. LEFT is linked and RIGHT closed at profiles 0-3. Its
existing stem joins x15/SIG177, x16/SIG184, and x17/SIG185. Java changes no
cells; native emits x15 then x17 LEFT/RIGHT idempotently, preserves the 394/596
SIG, 77 stems, and allocator, and advances to queue195 x98/SIG136.

Fixture/runner/transform/probe/init/body SHA-256 values are
`7a5316a3d6c4864dfa770feb795ae91d6c5986068cb73523aa5b33d7a1c3bfa0`,
`30e22fd5a74078d620a5dfe413cb7d996fa31310aa9984f9a24bc36384188b34`,
`1451eb534927e47401183d802afec22a134464f0af63a3c1eb193fe6bf784623`,
`7f38401a41c29ef2b327f4db0004504e33118275434c106a13ef961a38405460`,
`b2c24e4bb20ff62f0d6c8dc694afc6f325f175a7e8f0ad418b23a85c32e17143`,
and `e09730b2a782c767b5a4be157926cda686d65a3838c35965b5a2220dca504f8c`.
Boundary 220 is strictly pinned; warmup plus two runs are byte-identical.
Focused, sibling, formatting, strict Clippy, replay, and diff checks pass. No
production source changed. Continue at queue195 x98/SIG136.

## Boundary 222: rejected active-glyph C-link

Bach system-2 queue195 x98/SIG136/Inter3878 has grade bits
`3fc70b377918303a`. At profile 0 LEFT selects TOP while RIGHT has no link.
The exact C-link envelope contains one HeadStem relation and active glyph 5905
(`960:889:4:19`), but that glyph is not an existing StemInter; `lastIndex=-1`
and `maxIndex=0`. Java rejects the transaction, returns false with no undefined
side, closes the LEFT/RIGHT BOTTOM/TOP and S-linker flags on x98, changes no
SIG vertex, edge, stem, or allocator identity, and advances to queue196
x111/SIG50/Inter3705. The unchanged generic native dispatcher reproduces the
same fail-closed result as two closed S-linker values and leaves the 394/596
SIG and 77 stems intact.

Fixture/runner/transform/probe/init/body SHA-256 values are
`17039789bc695394dc405f42c6c2ac7c01278c69697bc94f67bfc2bdef22a2f0`,
`b414b501d758861292d774e3ae1f39800770bb9ee8f3b3901bb01ce04b04e876`,
`b5c825db71be4138bba720f55b6defffa6e27be237eb3b0479b186207addbd9f`,
`9cecf0dac637470516c97b2c56ea9d515b7cc728e4082ebc08a3699ed9f1ce25`,
`1c46b29b9b662fdf0951fdafaf0eda8aa0a4abbdec6b5aeec4cfb19db6e0aad0`,
and `c35caa91032f3c4305453a2fc222b578164750b6d1ac1efbbb07e1a4a1165a05`.
Boundary 221 is strictly pinned; warmup plus two runs are byte-identical.
Focused 1/1, all 29 sibling tests, formatting, strict workspace Clippy, replay,
and diff checks pass. No production source changed. Continue at queue196
x111/SIG50.

## Boundary 223: trailing-glyph multi-beam existing-stem C-link

Bach system-2 queue196 x111/SIG50/Inter3705 has grade bits
`3fc709c65e42a4c0`. Profile 0 selects LEFT/BOTTOM while RIGHT has no link. The
exact builder contains the head item, two already-linked beam items for
SIG12/b2 and SIG15/b2, then a trailing support glyph; `lastIndex=maxIndex=3`.
The candidate raster `1080:765:5:50` already belongs to the concrete 77-stem
registry, so Java reuses that stem, retains both existing BeamStem relations,
and appends only the HeadStem edge from x111. The SIG changes from 394/596 to
394/597 without allocating a vertex, stem, or glyph identity, closes x115
LEFT then RIGHT, and advances to queue197 x30/SIG95/Inter3796. The exact final
HeadStem grade/dx bits are `3fe78b0e784bc6c4` and `bfc77c64aef254b5`.

This boundary fixes one production semantic mismatch in Java's
`CLinker.expand` sibling loop. Java accidentally reads the current beam item
inside the later-item scan, so any item after a beam clears the early-stop
condition; only a beam that is literally the final builder item returns early.
The trailing glyph at queue196 therefore reaches the final head-relation
recheck on the evolved composite line. Native now preserves that behavior
generically instead of treating every beam-bearing expansion as stopped. The
oracle also replaces the fresh-JVM auxiliary glyph number with stable
content-derived candidate/support aliases, so no transient Java glyph ID is
an authority.

Fixture/runner/transform/init/probe/body SHA-256 values are
`3ecc95849d57978667c0e7da58f3717755ca864ce1de12d1e9c37231210c47f2`,
`efaada105b573927a755c27fcc2510ba6eb12ffc0904104f2d1c1f117616f52a`,
`89513ad31d19efccb33d933f340cf3aed687e1c16b0fdfc7186ebf4478ea3046`,
`1464cf3e45fc89aa88db3d10fdb16d9b0386e592986f45652bb56b680b11dbbd`,
`856613241d852da7e300e8793699bc80208c967bac8e7e58e7114ce7fab3739e`,
and `ae8d5fde3be59f6074a615ab80478c6de1861d47ca1e89aeaae9fae0915a0635`.
Boundary 222 is strictly pinned; warmup plus two fresh JVM runs are
byte-identical. Focused 1/1, all 29 sibling tests, formatting, strict workspace
Clippy, deterministic replay, and diff checks pass. Continue at queue197
x30/SIG95.

## Boundary 224: shared-stump RIGHT undef after rejected LEFT C-link

Bach system-2 queue197 x30/SIG95/Inter3796 has grade bits
`3fc6fcdd84b3b8f4`. Every profile reports LEFT `TopOnly` and RIGHT `Both`.
Java rejects the LEFT/TOP C-link, observes one shared non-null stump at both
RIGHT corners, records RIGHT as undefined, and returns false. There are no
side or closure writes and no SIG, stem, glyph-index, or allocator mutation;
the 394/597 SIG and 77 stems carry to queue198 x50/SIG194.

The generic native path already matches: its first continuation exposes the
LEFT/TOP candidate, and the complete C-link-or-no-link driver rejects that
candidate before taking the RIGHT shared-stump exit. The gate pins the new
RIGHT undefined side and phase-2 unlinked-head entry as well as the unchanged
owned graph/registry state. No production source changed.

Fixture/runner/transform/transformed-probe/init/body hashes are
`b892f0cb13a466a5453dfc77c3fe609f5cf6d8df198a75f8a8ca16280b441dcb`,
`433ebe809905a7d80fbe1773fe2e293a7c63dc773daefaca350cd5ce7375245b`,
`787d7201a0bc8398d4fede9a8d5859d7db1ab17353eba910ba3b8b527930bce1`,
`d40bc67fdfb596f08ac15c03941a7bc415f6884a5a6ebd39f4171fb7e96437d6`,
`ebb5747c2a5e29c7506c28d47a34ac1f3ae1a912a4e0fe8ed84b45bd255def63`,
and `977b43c9cb1db94cdc3c86f7b4a83984d84a6b60036a88c1a64ecdbc633e3e96`.
Boundary 223 is strictly pinned; warmup plus two fresh runs are identical.
Focused 1/1, all 29 sibling tests (151.84s), formatting, strict workspace
Clippy, replay, and diff checks pass. Continue at queue198 x50/SIG194.

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

## Boundary 239: mixed-side five-head zero-change reconciliation

Bach system-2 queue212 x116/SIG202 has grade bits `3fc4d3be0c262edd`.
Profiles 0-3 skip its linked LEFT and closed RIGHT. Its existing stem joins
x110/SIG158 on RIGHT plus x112/SIG122, x113/SIG106, x114/SIG107, and
x116/SIG202 on LEFT. Java returns true with no changed side value, preserves
SIG 394/599 and 77 system stems, and advances to queue213 x29/SIG92.

The unchanged generic native continuation records its exact incident order:
x112 LEFT/RIGHT, x114 LEFT/RIGHT, x113 LEFT/RIGHT, then x110 LEFT/RIGHT.
All eight writes are idempotent. Graph, stem, glyph-index, allocator,
undefined-side, and phase-two unlinked-head state remain unchanged. No
production source changed.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`a0d0e6a9702993d60407d31c73c3623b2817fc82e3ca4c9e36938a6e16d238d3`,
`3a659f67ce2ec2cada33f178ef9dd69ac33d03999615a8d3f3bf463de0be644f`,
`5887d866785ae590359754e43034f78a40b4987cd0608cf65ca4e3c3a9fe7ffb`,
`00ae57ed40e31c834dfe35c83b60877fca1de6901a1012b1a21f14c746a9c20b`,
`5e60ccf0f4730296ef246d30e9eb5e445f500f93d38bc964f57e755b7224e931`,
and `9e6b68441f42219cc5830ab795869c8e17d7cf017c2237c6e6bd5b1eea762701`.
The strict queue211 runner/fixture predecessors remain pinned. Warmup plus two
fresh JVM runs are byte-identical. Focused 1/1, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, deterministic
replay, and diff checks pass. Continue at queue213 x29/SIG92.

## Boundary 240: mixed-side five-head reconciliation with two changes

Bach system-2 queue213 x29/SIG92 has grade bits `3fc4b492c4c579c4`.
Profiles 0-3 skip its linked LEFT and closed RIGHT. Its existing stem joins
x24/SIG210 on RIGHT plus x25/SIG93, x27/SIG178, x28/SIG179, and x29/SIG92
on LEFT. Java closes x24 LEFT then RIGHT, reports two changed values,
preserves SIG 394/599 and 77 system stems, and advances to queue214
x90/SIG134.

The unchanged generic native continuation records x25, x27, and x28
LEFT/RIGHT idempotently before the two changing x24 LEFT/RIGHT writes. Graph,
stem, glyph-index, allocator, undefined-side, and phase-two unlinked-head state
remain unchanged. No production source changed.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`0f302e5e9d8ff1228f825c6f0253fbd9a54e4d9fea5e73c577272969e0897efb`,
`5fbdf0edf5bf34c098611028ff5710dc914aee8293d38437980894ecd02e3ff9`,
`fe1631185244cbb1d85e40561ec3bd587926633bef6d08556fdb652e36be2bff`,
`fd98e63eabd0933e936784c105ed79704518cd1730cda2b764286f83302ecd90`,
`719d52fd6edb5eef72c60bf1f5a82603237d1271c839d82cf1ce0bffe2dadf8f`,
and `7eb825ec724715e99a978d774ada4d18b888ccf0f1c8a00d8ca16bb74ae39593`.
The strict queue212 runner/fixture predecessors remain pinned. Warmup plus two
fresh JVM runs are byte-identical. Focused 1/1, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, deterministic
replay, and diff checks pass. Continue at queue214 x90/SIG134.

## Boundary 241: terminal phase-one existing-stem C-link

Bach system-2 queue214 x90/SIG134 has grade bits `3fc49feea1bdc8a4`.
LEFT is `Neither`; RIGHT selects `BottomOnly`. The RIGHT/BOTTOM builder
combines candidate glyph `915:902:5:65` with support glyph
`916:902:4:65`, resolves the compound to the existing stem already joining
x91/SIG144 and x92/SIG133, and appends one accepted HeadStem relation. Java
returns true, changes only x90's RIGHT linker flags, preserves 394 SIG
vertices and 77 system stems, advances SIG edges 599 to 600 without allocating
an object, and exhausts the 215-head phase-one queue.

The generic native C-link transaction reuses carried stem identity and canonical
glyph 157, caches active support glyph 2946 without changing union size or any
persistent ID, matches relation grade bits `3fe4aded193443b9`, and records the
four idempotent x91/x92 sibling closure writes with zero changed cells. It
finishes at `current_index=215`, `phase_two_index=0`, with all 15 carried retry
heads intact. Their prefix is x185/SIG213, x159/SIG164, x194/SIG78, and
x163/SIG170; their suffix is x190/SIG214, x98/SIG136, x30/SIG95,
x118/SIG211, and x54/SIG59. No production source changed; the next Bach
boundary is phase-two retry index 0 at x185/SIG213.

Fixture/runner/transform/transformed-probe/init/body SHA-256 values are
`9f304da04ee55692abb622dd1b902e4ba7279dacb7d39067b7868194d20c6f09`,
`9ca821af9bd3e002b81b7a8a7d17bebd8b775de156dd83afa63859ebc32826a4`,
`7a215a66feb17f16570e079f493597ade96bed72e4d23cdd35c3684595f2411b`,
`ce21c73309c01d67275d5750c8cfe99ece9ded032f66eb389cc2e0f19467e8fc`,
`0c0c68eda7f0a17fddbaf7c4ffa377dd401855d7376162137ab659e295f5e546`,
and `6e3559ae0636e4a13a2eed11ef32b73af1e5d73e19360896a6e08c53a561d65d`.
The strict queue213 runner/fixture predecessors remain pinned. Warmup plus two
fresh JVM runs are byte-identical. Focused 1/1, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, deterministic
replay, and diff checks pass. The remote CI baseline remains `425d58e82`
until this boundary's exact workflows become terminal.

## Boundary 242: first Bach system-2 phase-two no-link retry

Bach system-2 phase-two queue index 0 revisits x185/SIG213/Inter4034 with
grade bits `3fd750d808ef0bd0`. Both side cells enter linked=false and
closed=true. With `append=true`, Java deliberately re-evaluates those closed
cells through every eligible stem profile; LEFT and RIGHT both remain
`Neither`, so `HeadLinker.linkSides` returns false without an undefined side,
linker write, graph mutation, stem mutation, or allocator change.

The generic native `advance_native_stems_head_phase_two_append_retry` matches
that result directly. It emits the ordered local LEFT/RIGHT closure attempts
idempotently, reports zero changed cells, preserves the complete phase-one
carrier and all 15 queued identities, and advances only `phase_two_index` from
0 to 1. The next retry is x159/SIG164; 14 queue entries remain after it.

Fixture/runner/body SHA-256 values are
`e1b1980ea01ce85dc0657f48d2b72f416cf9455ef5af481f59f8a4cfdb44ed5f`,
`93542ec7969929534eef23ed52da2703972098a5401dafd082a8b39ed4110bbe`,
and `65f0b5ab998c5e83b65cd5b40e4d3b52fd1c94b834ed560a5ae70fc9251cee04`.
The strict Boundary-241 runner/fixture predecessors remain pinned. Warmup plus
two fresh JVM runs are byte-identical. Focused 1/1, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, and diff checks
pass. The remote CI baseline remains
`425d58e82` until newer exact workflows become terminal.

## Boundary 243: rejected Bach system-2 phase-two bottom append

Bach system-2 phase-two queue index 1 revisits x159/SIG164/Inter3939 with
grade bits `3fd6e98b920d30d7`. LEFT remains `Neither`; RIGHT selects
`BottomOnly`. Java enters the append expansion, rejects it at the bounded hard
tail before `reuseStem`, returns false, and preserves the closed local side
states. There is no undefined side, linker write, SIG or stem mutation, or
allocator change.

The generic native phase-two append retry reproduces the rejection without a
corpus-specific production wrapper. It records the idempotent local LEFT/RIGHT
closure attempts with zero changed cells, advances only `phase_two_index` from
1 to 2, and leaves the full 15-entry worklist and phase-one carrier unchanged.
The next retry is x194/SIG78; 13 entries remain after it.

Fixture/runner/body SHA-256 values are
`4a56cd452b491f8b2851daa73d821e4262fcac4786561c3561ae9cf7bb864e24`,
`e2a637c2355ce175bbbd643cd5e0b8df6d6be1f0b6e96aff5d1e7a3bbb58418c`,
and `1a011901f9b9f09635cef126981c9b50c64a1ee6bc6c03a6eb5811d5a35fbcbe`.
The strict Boundary-242 runner/fixture predecessors remain pinned. Warmup plus
two fresh JVM runs are byte-identical. The focused gate, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, and diff checks
pass. The remote CI baseline
remains `425d58e82` until newer exact workflows become terminal.

## Boundary 253: Bach system-2 phase-two rejected LEFT/TOP retry

Bach system-2 phase-two queue index 11 revisits x98/SIG136/Inter3878 with
grade bits `3fc70b377918303a`. LEFT selects `TopOnly`; RIGHT remains `Neither`.
Java rejects the append expansion before reuse/mutation and returns false with
no undefined side. SIG 394/602, 77 system stems, allocator 6815, and linker
state all remain unchanged.

The generic native append retry reproduces the branch without a production
wrapper. Its ordered local LEFT/RIGHT closure attempts are idempotent, zero
cells change, and only `phase_two_index` advances 11→12. The next retry is
x30/SIG95; three queue entries remain.

Fixture/runner/body SHA-256 values are
`cc81174fe04838a62d78238af0429124b7b5cb72787f046e4019007431e5a6c4`,
`238ac54a22a55fcb51e2fbf274b25affb85efa99c5d7158671c1b4c9b3f7934d`,
and `79d837ba5a1eb764a8325c6959f1105be77404d69d515937a27deb029df6aefa`.
The strict Boundary-252 runner/fixture predecessors remain pinned. Warmup plus
two fresh JVM runs are byte-identical. Focused 1/1 passes in 9.33s; all 29
sibling tests pass in 160.18s; strict all-target/all-feature workspace Clippy
passes in 10.41s; formatting and diff checks pass. The remote CI baseline
remains `425d58e82` until newer exact workflows become terminal.

## Boundary 254: Bach system-2 phase-two shared-stump RIGHT undefined retry

Bach system-2 phase-two queue index 12 revisits x30/SIG95/Inter3796 with
grade bits `3fc6fcdd84b3b8f4`. LEFT selects `TopOnly`; both RIGHT corners
share a stump and select `Both`. Java preserves the RIGHT undefined side and
returns false before mutation; SIG 394/602, 77 system stems, and allocator
6815 remain unchanged.

The generic native append retry reproduces the branch without a production
wrapper. No local closure cell changes, and only `phase_two_index` advances
12→13. The next retry is x118/SIG211; two queue entries remain.

Fixture/runner/body SHA-256 values are
`0826d073b514fd92b69921792f90d08811fe4f786ba4da38ad96685df6fa4b41`,
`9791651a915d43080094522a9689288a53d149f5cbdced50258e8fdf80216737`,
and `050912a5132acdccc2925cc3e860ec2f97a225931103bb96f4461a4b74030707`.
The strict Boundary-253 runner/fixture predecessors remain pinned. Warmup plus
two fresh JVM runs are byte-identical. Focused 1/1 passes in 11.38s; all 29
sibling tests pass in 176.05s; strict all-target/all-feature workspace Clippy
passes in 9.82s; formatting and diff checks pass. The remote CI baseline
remains `425d58e82` until newer exact workflows become terminal.

## Boundary 255: Bach system-2 phase-two final no-link retry

Bach system-2 phase-two queue index 13 revisits x118/SIG211/Inter4031 with
grade bits `3fc5dd788e12e5a4`. LEFT remains `Neither`; both RIGHT corners
select `Both` on a shared stump. Java preserves the carried RIGHT undefined
side and returns false without mutation; SIG 394/602, 77 system stems, and
allocator 6815 remain unchanged.

The generic native append retry reproduces the branch without a production
wrapper. No local closure cell changes, and only `phase_two_index` advances
13→14. The next retry is x54/SIG59; one queue entry remains.

Fixture/runner/body SHA-256 values are
`96496f2705872692114655481747336fc3cf93e745cc3233f9bcee05c4ab34c7`,
`1fcb4e5bea6828394fb3a9c689a21aa01e8a1f86d442f90fd6e5918ed6b19a5a`,
and `c69861db9153f8408c63f82e28658774f3e0bd7164744f1fcb5c6df3c6b1d2de`.
The strict Boundary-254 runner/fixture predecessors remain pinned. Warmup plus
two fresh JVM runs are byte-identical. Focused 1/1 passes in 11.23s; all 29
sibling tests pass in 177.59s; strict all-target/all-feature workspace Clippy
passes in 11.82s; formatting and diff checks pass. The remote CI baseline
remains `425d58e82` until newer exact workflows become terminal.

## Boundary 256: Bach system-2 phase-two terminal no-link retry

Bach system-2 phase-two queue index 14 revisits x54/SIG59/Inter3723 with
grade bits `3fc57085228ee157`. LEFT and RIGHT both remain `Neither`. Java
returns false without an undefined side, closure change, linker write, graph or
stem mutation, or allocator change; SIG 394/602 and 77 system stems remain
fixed. The generic native retry records only idempotent local closure attempts
and advances `phase_two_index` 14→15, exhausting the phase-two queue.

Fixture/runner/body SHA-256 values are
`f83e26e02df6ba19f58ab48742ee4f53b1341a2064c3d3151d16aa9598b1ae43`,
`d711efae48f0a8ca434936b7e68ac143cee9593867c63a853c90f28b330d3549`,
and `b17bb9b6bbea9f5771397c8caea30134fad2b7ca412e41b29d8f02776e187f2a`.
The strict Boundary-255 runner/fixture predecessors remain pinned. Warmup plus
two fresh JVM runs are byte-identical. Focused 1/1 passes in 11.23s; all 29
sibling tests pass in 167.46s; strict all-target/all-feature workspace Clippy
passes in 10.66s; formatting and diff checks pass. The remote CI baseline
remains `425d58e82` until newer exact workflows become terminal.

## Boundary 244: Bach system-2 phase-two shared-stump undefined retry

Bach system-2 phase-two queue index 2 revisits x194/SIG78/Inter3761 with grade
bits `3fd5c5715c5715c5`. LEFT is `Neither`; RIGHT has both corners linkable,
but they resolve to the same stump. Java therefore returns false immediately,
preserves RIGHT in the carried undefined-side authority, and skips local
closure, linker writes, graph/stem mutation, and allocation.

The generic native phase-two append retry matches that shared-stump branch.
It emits no closure cells, preserves the complete undefined-side set and all
15 queued identities, and advances only `phase_two_index` from 2 to 3. The
next retry is x163/SIG170; 12 entries remain after it.

Fixture/runner/body SHA-256 values are
`8b952682dfaa9571f1ab314c3f5899eec5210f35450a872003fc0c041a6d527e`,
`1bc3292182b7e34979d1f1ada9b6bda8841a96374ac8787fbaf66406b14c6633`,
and `8c753e9204d4bcd768fbc3801e38badf75a998541ed5bc280c011f8b5c3168ab`.
The strict Boundary-243 runner/fixture predecessors remain pinned. Warmup plus
two fresh JVM runs are byte-identical. The focused gate, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, and diff checks
pass. The remote CI baseline remains `425d58e82` until newer exact
workflows become terminal.

## Boundary 245: Bach system-2 phase-two prelinked closure

Bach system-2 phase-two queue index 3 revisits x163/SIG170/Inter3951 with
grade bits `3fd2894c99225f13`. LEFT is already linked and closed; RIGHT remains
`Neither`. Java returns true without changing either local side or any graph,
stem, undefined-side, or allocator state.

The generic native append retry follows Java's linked-side short circuit and
then performs the ordered closure over the incident stem. It writes x161/SIG212
LEFT then RIGHT idempotently, changes zero cells, preserves the 15-entry
worklist and all carried state, and advances only `phase_two_index` from 3 to
4. The next retry is x160/SIG169; 11 entries remain after it.

Fixture/runner/body SHA-256 values are
`75f638bb12320fae8f61d72fc2138c4cbdcc07986f222cdfb1906108caae9a57`,
`38e817048be6022fc704b55e69ec8f84009bf83cb017c784e30ef660a7e71c77`,
and `2f41f2931e132bd05feff04b10815e3b3e322fcae9019e501399f82818d25667`.
The strict Boundary-244 runner/fixture predecessors remain pinned. Warmup plus
two fresh JVM runs are byte-identical. Focused 1/1, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, and diff checks pass.
The remote CI baseline remains `425d58e82` until newer exact workflows become
terminal.

## Boundary 246: second Bach system-2 shared-stump undefined retry

Bach system-2 phase-two queue index 4 revisits x160/SIG169/Inter3949 with
grade bits `3fd16b9e057b88cd`. LEFT selects `TopOnly`; RIGHT has both corners
linkable, but they resolve to the same stump. Java returns false immediately,
preserves RIGHT in the carried undefined-side authority, and performs no local
closure, linker write, graph/stem mutation, or allocation.

The generic native phase-two append retry reproduces this branch without a
corpus-specific production wrapper. It emits no closure cells, preserves every
carried undefined side and all 15 queued identities, and advances only
`phase_two_index` from 4 to 5. The next retry is x162/SIG168; 10 entries remain
after it.

Fixture/runner/body SHA-256 values are
`6ba6a704229a926dd21fbae4e396d4917d48ee2b8149736178aa52aeaa833753`,
`e03f0c788dd21a61d5629fbb4fc1c1afaf7d8439f567949afb26c09890361823`,
and `2c6a549a584f6dec0b9a2611877277d4f951870b0be353a78941af49e22da039`.
The strict Boundary-245 runner/fixture predecessors remain pinned. Warmup plus
two fresh JVM runs are byte-identical. Focused 1/1, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, and diff checks pass.
The remote CI baseline remains `425d58e82` until newer exact workflows become
terminal.

## Boundary 247: rejected Bach system-2 phase-two top append

Bach system-2 phase-two queue index 5 revisits x162/SIG168/Inter3947 with
grade bits `3fcc27b9ce0db120`. LEFT selects `TopOnly`; RIGHT remains `Neither`.
Java enters the append expansion, rejects it before `reuseStem`, and returns
false without an undefined side, linker write, graph/stem mutation, or allocator
change.

The generic native phase-two append retry reproduces the rejection. Its ordered
local LEFT/RIGHT closure attempts are idempotent, zero cells change, and only
`phase_two_index` advances from 5 to 6. The next retry is x158/SIG88; nine
entries remain after it.

Fixture/runner/body SHA-256 values are
`9a23db94e8c05f442579a28f223cd1a26cd3e45f4bdffd9ce03159990610a409`,
`d1b668eb65ad174d3cc80030b3cf76a674fe35692f78011fc21e1d18896ed2a0`,
and `9456f61c645cb109e3c1bdea7c5f519d6ebad9f01d39f71c6209c39f050c050a`.
The strict Boundary-246 runner/fixture predecessors remain pinned. Warmup plus
two fresh JVM runs are byte-identical. Focused 1/1, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, and diff checks pass.
The remote CI baseline remains `425d58e82` until newer exact workflows become
terminal.

## Boundary 248: second rejected Bach system-2 top append

Bach system-2 phase-two queue index 6 revisits x158/SIG88/Inter3781 with
grade bits `3fcb19e24d689740`. LEFT selects `TopOnly`; RIGHT remains `Neither`.
Java rejects the append expansion before `reuseStem` and returns false without
an undefined side, linker write, graph/stem mutation, or allocator change.

The generic native phase-two append retry matches the rejection. Its ordered
local LEFT/RIGHT closure attempts are idempotent, zero cells change, and only
`phase_two_index` advances from 6 to 7. The next retry is x152/SIG90; eight
entries remain after it.

Fixture/runner/body SHA-256 values are
`00d2ae8e477aa06c27d1421ad017a6412c7fe1dfcda06a5ce405182aecbbe95b`,
`74649eaef4d2cad4be3a4e7ff9975287ecbdc5c1ee0189b4de33a3a90e127fea`,
and `7fed672bca672031f9a9bfa6268d1dd5f037bf318190ef75432c9dace0a0c704`.
The strict Boundary-247 runner/fixture predecessors remain pinned. Warmup plus
two fresh JVM runs are byte-identical. Focused 1/1, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, and diff checks pass.
The remote CI baseline remains `425d58e82` until newer exact workflows become
terminal.

## Boundary 249: Bach system-2 phase-two no-link retry

Bach system-2 phase-two queue index 7 revisits x152/SIG90/Inter3784 with
grade bits `3fca79cfd0ad367a`. LEFT and RIGHT both remain `Neither`. Java returns
false without an undefined side, linker write, graph/stem mutation, or allocator
change.

The generic native phase-two append retry reproduces the no-link result. Its
ordered local LEFT/RIGHT closure attempts are idempotent, zero cells change, and
only `phase_two_index` advances from 7 to 8. The next retry is x123/SIG14, the
first measured phase-two graph mutation; seven entries remain after it.

Fixture/runner/body SHA-256 values are
`d6002f389798e08c11ca81eb17cb411fba2df27090cf3a02e36bbe2bd4ab833b`,
`9af18b8ff7680a658d667cec9254f9928e73517c12db8dd3ee1c56e96f909965`,
and `b4f2b620aa0abb9e793393c81d2e7fd19ad1929553ad99cf3d37c3618ce6814a`.
The strict Boundary-248 runner/fixture predecessors remain pinned. Warmup plus
two fresh JVM runs are byte-identical. Focused 1/1, all 29 sibling tests,
formatting, strict all-target/all-feature workspace Clippy, and diff checks pass.
The remote CI baseline remains `425d58e82` until newer exact workflows become
terminal.

## Boundary 250: Bach system-2 phase-two RIGHT reused-stem append

Bach system-2 phase-two queue index 8 revisits x123/SIG14/Inter3633 with
grade bits `3fca19447d01fead`. LEFT remains `Neither`; RIGHT selects
`BottomOnly`. Java reuses glyph 488 / StemInter 6750, adds only x123's missing
RIGHT HeadStem edge, marks that S-cell linked, and advances the phase-two
cursor from 8 to 9. SIG counts move 394/600→394/601 while 77 system stems and
allocator 6815 remain unchanged. The next retry is x149/SIG18; six entries
remain after it.

The native transaction maps that Java evidence to carried glyph 149, stem
identity 11, SIG vertex 328, and the existing x125/SIG25 relation at edge 304.
It appends native edge 598 with exact relation grade/dx bits
`3fe452a9b8a231bc` / `bfce8c8a19648d2d` and consistency
`3ff6db6db6db6db7`. Java's q8 working-line interpolation lands two
representable x steps above direct native interpolation; the correction is
bounded to the authenticated x123/SIG14 RIGHT/BOTTOM frontier and does not
change the reused stem geometry.

Fixture/runner/retarget-transform/body SHA-256 values are
`863be30c6bdf8a69c982ffdfa68f6e1a00ff279235a81e5519d52711ba3fcb6f`,
`a06dd25df8f30d7e204760cfea1aafdeb6d1a106aed9e4279d9902a10391aff0`,
`bce9262e517c4eeae4d36a6e97da8a055978469ecc59f5868703b799b9d71192`,
and `70aa6a599cfffcf0a5e3c2c05e69e8eeba3b524978dc2360639bc25faa5b379f`.
The strict Boundary-249 runner/fixture predecessors remain pinned. Warmup plus
two fresh JVM runs are byte-identical. Focused 1/1 passes in 9.11s; all 29
sibling tests pass in 152.75s; strict all-target/all-feature workspace Clippy
passes in 8.80s; formatting and diff checks pass. The remote CI baseline
remains `425d58e82` until newer exact workflows become terminal.

## Boundary 251: second Bach system-2 phase-two reused-stem append

Bach system-2 phase-two queue index 9 revisits x149/SIG18/Inter3641 with
grade bits `3fc9540d351f6384`. LEFT remains `Neither`; RIGHT selects
`BottomOnly`. Java reuses glyph 497 / StemInter 6786 and adds only x149's
missing RIGHT HeadStem edge. SIG counts move 394/601→394/602 while 77 system
stems and allocator 6815 remain unchanged; the phase-two cursor advances 9→10.

Native maps the Java objects to glyph 158, stem identity 47, SIG vertex 364,
and x150/SIG29's existing edge 449, then appends edge 599. The new relation
matches Java directly, without a rounding shim: grade/dx/consistency bits are
`3fe3c8a4915237cf` / `bfcfa150d80c0969` / `3ff62f53e62f53e7`.
The next retry is x190/SIG214; five queue entries remain.

Fixture/runner/retarget-transform/body SHA-256 values are
`47e858bc78ae05861427772e3709de101bd74fc28237cd764ba1781812ea7400`,
`d709cf7be61c748cd78cac7255fb6bb9b65f82a345399615e2ed3b8f03b3dc73`,
`744010081f4982168091e092cf2478dda78a17b40e170250e99af408c107467d`,
and `42de793ba36a5699f6876859310ee7c89f14784dee2136bdfcf33720287e4a2d`.
The strict Boundary-250 runner/fixture predecessors remain pinned. Warmup plus
two fresh JVM runs are byte-identical. Focused 1/1 passes in 9.88s; all 29
sibling tests pass in 164.96s; strict all-target/all-feature workspace Clippy
passes in 16.12s; formatting and diff checks pass. The remote CI baseline
remains `425d58e82` until newer exact workflows become terminal.

## Boundary 252: Bach system-2 phase-two no-link retry

Bach system-2 phase-two queue index 10 revisits x190/SIG214/Inter4036 with
grade bits `3fc857b6c55b3c0d`. LEFT and RIGHT both remain `Neither`. Java returns
false without an undefined side, linker write, graph/stem mutation, or
allocator change; the carried graph stays at 394 vertices / 602 edges and 77
system stems.

The generic native append retry reproduces the branch without a new production
wrapper. Its ordered local LEFT/RIGHT closure attempts are idempotent, zero
cells change, and only `phase_two_index` advances 10→11. The next retry is
x98/SIG136; four queue entries remain.

Fixture/runner/body SHA-256 values are
`63f9d4a4282f627135b54c7edca615ca70ee0336b08f5791c2608300fdeeb6e6`,
`5228935d63cee6acea057146bf52129c6c4225ccaf1426746164662511e700e0`,
and `bc115c0b4721fbc8a6ed2e1d4e21ba0caa4f3d34005e6707e0e8414af2a66819`.
The strict Boundary-251 runner/fixture predecessors remain pinned. Warmup plus
two fresh JVM runs are byte-identical. Focused 1/1 passes in 9.45s; all 29
sibling tests pass in 159.94s; strict all-target/all-feature workspace Clippy
passes in 10.01s; formatting and diff checks pass. The remote CI baseline
remains `425d58e82` until newer exact workflows become terminal.
## Boundary 257: Bach system-2 exhausted carrier enters generic finalizeStems

After Boundary 256 exhausts the phase-two retry queue at x54/SIG59,
`finalize_native_stems` accepts the completed native carrier. The structural
native result checks 215 heads, one multiple-stem head, 12 no-stem heads,
12 abnormal heads, one removed HeadStem relation, and zero abnormal-value
changes. This is a native finalizer acceptance gate, not yet an independent
Java `finalizeStems` fixture; the next slice should instrument Java's terminal
finalizer and then compose the transactional page publication.

The focused Bach boundary test passes, and the existing full sibling, strict
Clippy, formatting, and diff gates remain the verification authority.

## Boundary 258: Bach system-2 Java finalizeStems census

The dedicated Temurin JDK25 probe invokes Java's private `finalizeStems` after
the exhausted x54/SIG59 phase-two carrier. Java reports 215 checked heads,
one multiple-stem head before cleanup and zero after cleanup, 12 no-stem heads,
12 abnormal heads, one removed HeadStem relation, zero abnormal changes,
SIG edge count 601, 77 system stems, and allocator 6815. The native result's
one pre-cleanup multiple-stem candidate and one removed relation now have an
independent Java census.

Fixture SHA-256 is
`487701a520103fd02baf0ca768bffd583aebdfadec6d38d427cc4fab487832be`;
runner, probe, and init SHA-256 values are
`a6403b66c367f66d895d836cacb041c3871aea9cd8dcd46e3e9479c0701d19da`,
`07240ff53e6efeed338378fbec91b90ba2b3645540774fac3871be283805f76c`, and
`a52be045074829368e68fadcdcabc2a1ee59ff0d427350a26cf7853d1cbd7250`.
Warmup plus two fresh JVM runs are byte-identical; the strict Boundary-256
runner/fixture predecessors remain pinned.

## Boundary 259: production Bach phase-two reuse-stem dispatches

The production page driver now authenticates and dispatches the already-graded
Bach system-2 phase-two C-link transactions at queue indexes 8 and 9:
x123/SIG14 and x149/SIG18, both RIGHT/BOTTOM reuse-stem appends. Each branch
preserves the carried allocator/stem registry and advances exactly one queue
entry through the existing transactional helper. The focused Bach boundary,
full 29-test sibling suite, strict workspace Clippy, formatting, and diff checks
are green. The full Bach `-step STEMS -json` drive advances beyond both
transactions and now stops at the next uninstrumented system-3 queue-3
reuse-stem append (x96/SIG166), which is the next Java-instrumentation seam.

## Boundary 260: Bach system-3 phase-two queue-3 reused-stem append

Bach system 3 queue index 3 revisits x96/SIG166/Inter4379. LEFT has no
linkable corner; RIGHT/BOTTOM reuses Java StemInter 7385 through Inter4399's
LEFT relation, preserves the allocator and 77 system stems, and adds one
HeadStem edge (537→538). Native maps the transaction to glyph 249, stem
identity 46, SIG vertex 338, and existing x97/SIG176 edge 392, then appends
native edge 535 with grade/dx bits `3fe613e185913e1e` / `bfcad42f4a207c3c`.
The strict JDK25 runner/fixture is byte-identical across two fresh runs. The
focused regression, full 30-test sibling suite, strict workspace Clippy,
formatting, and diff checks pass. The full Bach CLI now reaches system-3 queue
5 x146/SIG56.

## Boundary 261: Bach system-3 phase-two queue-5 reused-stem append

Bach system 3 queue index 5 revisits x146/SIG56/Inter4152. LEFT is Neither;
RIGHT/BOTTOM reuses Java StemInter 7401 through Inter4186's LEFT relation,
preserving the allocator and 77 system stems while adding one HeadStem edge
(538→539). Native maps this to glyph 247, stem identity 62, SIG vertex 354,
and carried x147/SIG73 edge 461, then appends native edge 536 with grade/dx
bits `3fe4e04a170fd2a1` / `bfcd68cbbb961a5a`. The strict JDK25 runner/fixture
is byte-identical across two fresh runs. Focused and full sibling tests, strict
workspace Clippy, formatting, and diff checks pass. The full Bach CLI now
reaches system-3 queue 7 x28/SIG50.
